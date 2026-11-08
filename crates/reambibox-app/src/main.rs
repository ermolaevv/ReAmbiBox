#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod hotkey_parser;
mod single_instance;
mod tray;
mod zone_overlay;

use reambibox_core::api::lightpack::LightpackServer;
use reambibox_core::api::state::create_shared_state;
use reambibox_core::capture::dxgi::DxgiCapture;
use reambibox_core::color::smoothing::TemporalSmoother;
use reambibox_core::config::{AppConfig, HotkeysConfig};
use reambibox_core::device::adalight::AdalightDriver;
use reambibox_core::device::traits::DeviceDriver;
use reambibox_core::device::types::{DeviceFrame, RgbColor};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tray::TrayManager;

#[derive(Debug, Deserialize)]
struct ConfigUpdate {
    brightness: Option<f32>,
    smoothing: Option<f32>,
    gamma: Option<f32>,
    letterbox: Option<bool>,
    is_active: Option<bool>,
    mode: Option<String>,
    color_order: Option<String>,
    static_color: Option<[u8; 3]>,
    selected_monitor: Option<usize>,
    autostart: Option<bool>,
    target_fps: Option<u32>,
    baud_rate: Option<u32>,
    serial_port: Option<String>,
    reconnect: Option<bool>,
    audio_effect: Option<String>,
    audio_sensitivity: Option<f32>,
    hotkeys: Option<HotkeysConfig>,
}

#[derive(Debug, Serialize)]
struct ApiMonitorInfo {
    index: usize,
    name: String,
    width: u32,
    height: u32,
    is_primary: bool,
}

#[derive(Debug, Serialize)]
struct ApiStatusResponse {
    is_active: bool,
    capture_fps: f32,
    serial_fps: f32,
    target_fps: u32,
    baud_rate: u32,
    brightness: f32,
    smoothing: f32,
    gamma: f32,
    letterbox: bool,
    autostart: bool,
    mode: String,
    color_order: String,
    static_color: [u8; 3],
    audio_effect: String,
    audio_sensitivity: f32,
    selected_monitor: usize,
    monitors: Vec<ApiMonitorInfo>,
    led_count: usize,
    serial_port: String,
    serial_status: String,
    available_ports: Vec<String>,
    is_overlay_visible: bool,
    hotkeys: HotkeysConfig,
    zones: Vec<reambibox_core::color::zone::Zone>,
}

unsafe fn init_console_support() {
    let args: Vec<String> = std::env::args().collect();
    let has_console_flag = args.iter().any(|a| a == "--console" || a == "-c");

    if cfg!(debug_assertions) || has_console_flag {
        use windows::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }
}

unsafe fn setup_shutdown_handlers(port: String, baud: u32, leds: usize) {
    // 1. Аварийный хук паники
    let panic_port = port.clone();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[Emergency] Аварийная остановка! Экстренное гашение светодиодов...");
        AdalightDriver::emergency_blank(&panic_port, baud, leds);
        default_hook(info);
    }));

    // 2. Сигнал закрытия консоли / Ctrl+C
    unsafe extern "system" fn console_ctrl_handler(dw_ctrl_type: u32) -> windows::Win32::Foundation::BOOL {
        println!("[Завершение] Получен системный сигнал {} — гасим подсветку", dw_ctrl_type);
        let cfg = AppConfig::load();
        AdalightDriver::emergency_blank(&cfg.serial_port, cfg.baud_rate, cfg.zones.len());
        windows::Win32::Foundation::BOOL(0)
    }

    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Автоматическое замещение предыдущего запущенного экземпляра (Single Instance Takeover)
    single_instance::ensure_single_instance_takeover();

    // Парсинг аргументов командной строки
    let args: Vec<String> = std::env::args().collect();
    let is_minimized = args.iter().any(|arg| arg == "--minimized" || arg == "--silent");

    // Включение Per-Monitor DPI Awareness v2 для устранения виртуализации разрешений и масштабирования Windows
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
        if !is_minimized {
            init_console_support();
        }
    }

    println!("============================================================");
    println!("        ReAmbiBox v0.1.0 — High-Performance Ambilight      ");
    println!("============================================================");

    // 1. Загрузка конфигурации
    let initial_cfg = AppConfig::load();
    println!("Загружена конфигурация:");
    println!("  - COM-порт: {} ({} бод, порядок: {})", initial_cfg.serial_port, initial_cfg.baud_rate, initial_cfg.color_order);
    println!("  - Активных светодиодных зон: {}", initial_cfg.zones.len());
    println!("  - Режим: {}, Яркость: {:.0}%, Сглаживание: {:.2}, Гамма: {:.1}",
             initial_cfg.mode, initial_cfg.brightness * 100.0, initial_cfg.smoothing, initial_cfg.gamma);

    // Установка обработчиков экстренного выключения подсветки
    unsafe {
        setup_shutdown_handlers(initial_cfg.serial_port.clone(), initial_cfg.baud_rate, initial_cfg.zones.len());
    }

    let shared_config = Arc::new(parking_lot::RwLock::new(initial_cfg.clone()));
    let shared_state = create_shared_state();
    {
        let mut s = shared_state.write().await;
        s.led_count = initial_cfg.zones.len();
        s.brightness = initial_cfg.brightness;
        s.smooth = initial_cfg.smoothing;
        s.gamma = initial_cfg.gamma;
    }

    // Разделяемые структуры между потоками захвата, вывода и веб-сервера
    let output_colors = Arc::new(parking_lot::RwLock::new(vec![RgbColor::BLACK; initial_cfg.zones.len()]));
    let serial_status = Arc::new(parking_lot::RwLock::new(String::from("Поиск устройства...")));
    let serial_fps = Arc::new(parking_lot::RwLock::new(0.0f32));
    let capture_fps = Arc::new(parking_lot::RwLock::new(0.0f32));
    let is_running = Arc::new(AtomicBool::new(true));
    let needs_reconnect = Arc::new(AtomicBool::new(false));

    // 2. Инициализация экранного оверлея калибровки зон (AmbiBox Overlay)
    let overlay_manager = Arc::new(zone_overlay::ZoneOverlayManager::new());
    overlay_manager.run_overlay_thread(Arc::clone(&shared_config));

    // 3. Инициализация системного трея и мониторинга питания Windows
    let tray_manager = Arc::new(TrayManager::new());
    tray_manager.run_tray_thread(
        Arc::clone(&shared_config),
        Arc::clone(&is_running),
        Arc::clone(&overlay_manager),
    );
    println!(">>> [OK] Системный трей Windows и мониторинг событий питания активны.");

    // 4. Запуск Lightpack API сервера (порт 3636)
    let lp_server = LightpackServer::new(shared_state.clone(), LightpackServer::DEFAULT_PORT);
    tokio::spawn(async move {
        let _ = lp_server.run().await;
    });
    println!(">>> [OK] Lightpack API сервер активен на 127.0.0.1:3636");

    // 4. Выделенный независимый аппаратный поток вывода в COM-порт (Adalight)
    {
        let colors_ref = Arc::clone(&output_colors);
        let status_ref = Arc::clone(&serial_status);
        let fps_ref = Arc::clone(&serial_fps);
        let run_ref = Arc::clone(&is_running);
        let cfg_ref = Arc::clone(&shared_config);
        let reconnect_ref = Arc::clone(&needs_reconnect);

        std::thread::spawn(move || {
            let (mut port_name, mut baud_rate, mut color_order) = {
                let cfg = cfg_ref.read();
                (cfg.serial_port.clone(), cfg.baud_rate, cfg.color_order.clone())
            };

            let mut driver = AdalightDriver::new(&port_name, baud_rate);
            driver.set_color_order(&color_order);

            let mut last_reconnect = Instant::now() - Duration::from_secs(5);
            let mut sent_frames: u64 = 0;
            let mut last_fps_time = Instant::now();

            while run_ref.load(Ordering::Relaxed) {
                let start = Instant::now();

                // Обработка ручного запроса переподключения/сброса контроллера
                if reconnect_ref.swap(false, Ordering::Relaxed) {
                    println!("[Serial] Запрошен принудительный сброс и переподключение COM-порта...");
                    let _ = driver.disconnect();
                    last_reconnect = Instant::now() - Duration::from_secs(5);
                }

                // Периодическая проверка смены настроек порта / скорости / порядка цветов
                let (target_fps, led_count, baud) = {
                    let cfg = cfg_ref.read();
                    if cfg.serial_port != port_name || cfg.baud_rate != baud_rate {
                        port_name = cfg.serial_port.clone();
                        baud_rate = cfg.baud_rate;
                        let _ = driver.disconnect();
                        driver = AdalightDriver::new(&port_name, baud_rate);
                    }
                    if cfg.color_order != color_order {
                        color_order = cfg.color_order.clone();
                        driver.set_color_order(&color_order);
                    }
                    (cfg.target_fps, cfg.zones.len(), cfg.baud_rate)
                };

                // Защита микроконтроллера от переполнения UART буфера:
                // Время физической передачи пакета (10 бит на байт: 1 старт + 8 данных + 1 стоп)
                let packet_bytes = (6 + led_count * 3) as u64;
                let tx_time_ms = (packet_bytes * 10 * 1000) / (baud.max(9600) as u64);
                // Микроконтроллеру требуется минимум 6-8 мс тишины между кадрами для вызова FastLED.show()
                let min_safe_interval_ms = tx_time_ms + 7;
                let requested_interval_ms = if target_fps > 0 {
                    1000 / (target_fps as u64)
                } else {
                    40
                };
                let frame_interval = Duration::from_millis(requested_interval_ms.max(min_safe_interval_ms));

                // Автопереподключение к порту при необходимости
                if !driver.is_connected() {
                    if last_reconnect.elapsed() >= Duration::from_millis(1500) {
                        last_reconnect = Instant::now();
                        match driver.connect() {
                            Ok(_) => {
                                println!(">>> [OK] Успешно подключено к Adalight на порту {} (порядок: {})", port_name, color_order);
                                *status_ref.write() = format!("Подключено ({})", port_name);
                            }
                            Err(e) => {
                                *status_ref.write() = format!("Ожидание порта {} ({})", port_name, e);
                            }
                        }
                    }
                }

                if driver.is_connected() {
                    let colors = {
                        let lock = colors_ref.read();
                        lock.clone()
                    };

                    let frame = DeviceFrame::new(colors);
                    match driver.send_frame(&frame) {
                        Ok(_) => {
                            sent_frames += 1;
                        }
                        Err(e) => {
                            eprintln!("[Serial] Ошибка записи в {}: {}", port_name, e);
                            *status_ref.write() = format!("Ошибка: {}", e);
                            // send_frame автоматически сбрасывает порт в None при обрыве
                        }
                    }
                }

                // Подсчет реального Serial FPS
                if last_fps_time.elapsed() >= Duration::from_secs(1) {
                    let fps = (sent_frames as f64) / last_fps_time.elapsed().as_secs_f64();
                    *fps_ref.write() = fps as f32;
                    sent_frames = 0;
                    last_fps_time = Instant::now();
                }

                let elapsed = start.elapsed();
                if elapsed < frame_interval {
                    std::thread::sleep(frame_interval - elapsed);
                }
            }

            // При выходе из цикла гарантированно гасим ленту
            let _ = driver.blank_leds();
            let _ = driver.disconnect();
        });
    }

    // 5. Запуск фонового захвата системного звука (WASAPI Loopback)
    let wasapi_bands = Arc::new(parking_lot::RwLock::new(reambibox_core::audio::AudioBands::default()));
    let audio_capture = Arc::new(reambibox_core::audio::WasapiLoopbackCapture::new());
    {
        let bands_store = Arc::clone(&wasapi_bands);
        audio_capture.start_capture(move |bands| {
            *bands_store.write() = bands;
        });
    }

    // 6. Выделенный поток захвата экрана и цветового процессинга
    {
        let colors_ref = Arc::clone(&output_colors);
        let cfg_ref = Arc::clone(&shared_config);
        let fps_ref = Arc::clone(&capture_fps);
        let run_ref = Arc::clone(&is_running);
        let tray_ref = Arc::clone(&tray_manager);
        let bands_ref = Arc::clone(&wasapi_bands);

        std::thread::spawn(move || {
            let mut current_mon: usize = 99999;
            let mut capture: Option<DxgiCapture> = None;
            let mut smoother = TemporalSmoother::new();
            let mut music_vis = reambibox_core::audio::MusicVisualizer::new();
            let mut last_sampled: Vec<RgbColor> = Vec::new();
            let mut frames_count: u64 = 0;
            let mut last_fps_time = Instant::now();

            while run_ref.load(Ordering::Relaxed) {
                let start = Instant::now();

                // Проверяем активность дисплея (сон экрана или блокировка ПК)
                let is_display_active = tray_ref.is_display_active();

                // Читаем актуальные параметры синхронно из parking_lot::RwLock
                let (mon_idx, is_active, brightness, smoothing, gamma, mode, static_color, zones) = {
                    let cfg = cfg_ref.read();
                    (
                        cfg.selected_monitor,
                        cfg.is_active,
                        cfg.brightness,
                        cfg.smoothing,
                        cfg.gamma,
                        cfg.mode.clone(),
                        cfg.static_color,
                        cfg.zones.clone(),
                    )
                };

                let led_count = zones.len();
                if last_sampled.len() != led_count {
                    last_sampled = vec![RgbColor::BLACK; led_count];
                }

                // Обработка состояния: Выключено или Дисплей спит / Сессия заблокирована
                if !is_active || !is_display_active {
                    {
                        let mut out = colors_ref.write();
                        *out = vec![RgbColor::BLACK; led_count];
                    }
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                // Обработка режима: Статический тестовый цвет (White / Test)
                if mode == "static" {
                    let base = RgbColor::new(static_color[0], static_color[1], static_color[2]);
                    let dimmed = base.with_brightness_gamma(brightness, gamma);
                    {
                        let mut out = colors_ref.write();
                        *out = vec![dimmed; led_count];
                    }
                    frames_count += 1;
                    if last_fps_time.elapsed() >= Duration::from_secs(1) {
                        *fps_ref.write() = (frames_count as f64 / last_fps_time.elapsed().as_secs_f64()) as f32;
                        frames_count = 0;
                        last_fps_time = Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }

                // Обработка режима: Светомузыка (Audio Reactive / Music Visualizer)
                if mode == "music" {
                    let bands = *bands_ref.read();
                    let (effect, sens) = {
                        let c = cfg_ref.read();
                        (c.audio_effect.clone(), c.audio_sensitivity)
                    };
                    let raw_leds = music_vis.render(bands, led_count, &effect, sens);
                    let processed: Vec<RgbColor> = raw_leds
                        .into_iter()
                        .map(|c| c.with_brightness_gamma(brightness, gamma))
                        .collect();
                    {
                        let mut out = colors_ref.write();
                        *out = processed;
                    }
                    frames_count += 1;
                    if last_fps_time.elapsed() >= Duration::from_secs(1) {
                        *fps_ref.write() = (frames_count as f64 / last_fps_time.elapsed().as_secs_f64()) as f32;
                        frames_count = 0;
                        last_fps_time = Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(16));
                    continue;
                }

                // Обработка режима: Захват экрана (Screen Capture)
                if mon_idx != current_mon || capture.is_none() {
                    match DxgiCapture::new(mon_idx) {
                        Ok(c) => {
                            println!(">>> [OK] DXGI захват переключен на монитор {}", mon_idx);
                            capture = Some(c);
                            current_mon = mon_idx;
                        }
                        Err(e) => {
                            eprintln!("Ошибка инициализации DXGI для монитора {}: {}", mon_idx, e);
                            std::thread::sleep(Duration::from_millis(500));
                            continue;
                        }
                    }
                }

                if let Some(ref mut cap) = capture {
                    match cap.capture_frame(Duration::from_millis(33)) {
                        Ok(frame) => {
                            last_sampled = zones.iter().map(|z| z.sample_color(&frame, 4)).collect();
                            frames_count += 1;
                        }
                        Err(reambibox_core::capture::CaptureError::Timeout) => {
                            // Экран не изменился, используем предыдущие цвета
                        }
                        Err(reambibox_core::capture::CaptureError::AccessLost) => {
                            std::thread::sleep(Duration::from_millis(200));
                            let _ = cap.reconnect();
                        }
                        Err(_) => {
                            std::thread::sleep(Duration::from_millis(15));
                        }
                    }

                    // Временное сглаживание
                    let smoothed = smoother.smooth(&last_sampled, smoothing);

                    // Коррекция яркости и гаммы в реальном времени
                    let final_colors: Vec<_> = smoothed
                        .into_iter()
                        .map(|c| c.with_brightness_gamma(brightness, gamma))
                        .collect();

                    {
                        let mut out = colors_ref.write();
                        *out = final_colors;
                    }
                }

                if last_fps_time.elapsed() >= Duration::from_secs(1) {
                    *fps_ref.write() = (frames_count as f64 / last_fps_time.elapsed().as_secs_f64()) as f32;
                    frames_count = 0;
                    last_fps_time = Instant::now();
                }

                let elapsed = start.elapsed();
                if elapsed < Duration::from_millis(16) {
                    std::thread::sleep(Duration::from_millis(16) - elapsed);
                }
            }

            // Гасим ленту при остановке потока захвата
            {
                let mut out = colors_ref.write();
                let count = out.len();
                *out = vec![RgbColor::BLACK; count];
            }
        });
    }

    println!(">>> [OK] Аппаратный поток DXGI захвата активен.");
    println!(">>> [OK] Выделенный поток COM-порта активен.");

    // 6. Запуск WebUI + REST API сервера (порт 3637)
    let web_config = Arc::clone(&shared_config);
    let web_state = Arc::clone(&shared_state);
    let web_serial_status = Arc::clone(&serial_status);
    let web_serial_fps = Arc::clone(&serial_fps);
    let web_capture_fps = Arc::clone(&capture_fps);
    let web_reconnect = Arc::clone(&needs_reconnect);
    let web_overlay = Arc::clone(&overlay_manager);
    let web_tray = Arc::clone(&tray_manager);

    tokio::spawn(async move {
        let listener = match TcpListener::bind("127.0.0.1:3637").await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Не удалось запустить WebUI сервер на порту 3637: {}", e);
                return;
            }
        };

        while let Ok((mut stream, _)) = listener.accept().await {
            let config_ref = Arc::clone(&web_config);
            let state_ref = Arc::clone(&web_state);
            let status_ref = Arc::clone(&web_serial_status);
            let s_fps_ref = Arc::clone(&web_serial_fps);
            let c_fps_ref = Arc::clone(&web_capture_fps);
            let recon_ref = Arc::clone(&web_reconnect);
            let overlay_ref = Arc::clone(&web_overlay);
            let tray_ref = Arc::clone(&web_tray);

            tokio::spawn(async move {
                let mut req_bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                let mut content_length: Option<usize> = None;
                let mut header_len = 0;

                loop {
                    let n = match stream.read(&mut chunk).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    req_bytes.extend_from_slice(&chunk[..n]);

                    if header_len == 0 {
                        if let Some(pos) = req_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            header_len = pos + 4;
                            let header_str = String::from_utf8_lossy(&req_bytes[..pos]);
                            for line in header_str.lines() {
                                if line.to_ascii_lowercase().starts_with("content-length:") {
                                    if let Some(val) = line.split(':').nth(1) {
                                        content_length = val.trim().parse().ok();
                                    }
                                }
                            }
                        }
                    }

                    if let Some(len) = content_length {
                        if req_bytes.len() >= header_len + len {
                            break;
                        }
                    } else if header_len > 0 {
                        break;
                    }
                }

                if req_bytes.is_empty() {
                    return;
                }

                let req_text = String::from_utf8_lossy(&req_bytes);

                // CORS Preflight OPTIONS
                if req_text.starts_with("OPTIONS") {
                    let response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // API: GET /api/config
                if req_text.starts_with("GET /api/config") {
                    let cfg = config_ref.read().clone();
                    let c_fps = *c_fps_ref.read();
                    let s_fps = *s_fps_ref.read();
                    let port_status = status_ref.read().clone();

                    let monitors_raw = DxgiCapture::list_monitors();
                    let monitors: Vec<_> = monitors_raw
                        .into_iter()
                        .map(|m| ApiMonitorInfo {
                            index: m.index,
                            name: format!("{}: {} ({}x{})", m.index, m.output_name, m.width, m.height),
                            width: m.width,
                            height: m.height,
                            is_primary: m.is_primary,
                        })
                        .collect();

                    let res_obj = ApiStatusResponse {
                        is_active: cfg.is_active,
                        capture_fps: c_fps,
                        serial_fps: s_fps,
                        target_fps: cfg.target_fps,
                        baud_rate: cfg.baud_rate,
                        brightness: cfg.brightness,
                        smoothing: cfg.smoothing,
                        gamma: cfg.gamma,
                        letterbox: cfg.letterbox_detection,
                        autostart: autostart::is_autostart_enabled(),
                        mode: cfg.mode.clone(),
                        color_order: cfg.color_order.clone(),
                        static_color: cfg.static_color,
                        audio_effect: cfg.audio_effect.clone(),
                        audio_sensitivity: cfg.audio_sensitivity,
                        selected_monitor: cfg.selected_monitor,
                        monitors,
                        led_count: cfg.zones.len(),
                        serial_port: cfg.serial_port.clone(),
                        serial_status: port_status,
                        available_ports: AdalightDriver::available_ports(),
                        is_overlay_visible: overlay_ref.is_visible(),
                        hotkeys: cfg.hotkeys.clone(),
                        zones: cfg.zones.clone(),
                    };

                    let json = serde_json::to_string(&res_obj).unwrap_or_default();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        json.len(),
                        json
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // API: POST /api/overlay/toggle
                if req_text.starts_with("POST /api/overlay/toggle") {
                    overlay_ref.toggle();
                    let is_vis = overlay_ref.is_visible();
                    let res = format!("{{\"status\":\"ok\",\"is_visible\":{}}}", is_vis);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        res.len(),
                        res
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // API: POST /api/zones/wizard
                if req_text.starts_with("POST /api/zones/wizard") {
                    if let Some(pos) = req_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body_slice = &req_bytes[pos + 4..];
                        if let Ok(params) = serde_json::from_slice::<reambibox_core::config::wizard::PerimeterParams>(body_slice) {
                            let new_zones = reambibox_core::config::wizard::ZoneWizard::generate_custom_perimeter(&params);
                            let count = new_zones.len();
                            {
                                let mut cfg = config_ref.write();
                                cfg.zones = new_zones;
                                cfg.led_count = count;
                                let _ = cfg.save();
                            }
                            if let Ok(mut st) = state_ref.try_write() {
                                st.led_count = count;
                            }
                            println!("[API] Мастер периметра зон применен: сгенерировано {} зон", count);
                        }
                    }

                    let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // API: POST /api/config
                if req_text.starts_with("POST /api/config") {
                    if let Some(pos) = req_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body_slice = &req_bytes[pos + 4..];
                        if let Ok(update) = serde_json::from_slice::<ConfigUpdate>(body_slice) {
                            let mut cfg = config_ref.write();

                            if let Some(b) = update.brightness {
                                cfg.brightness = b;
                                println!("[API] Яркость изменена: {:.0}%", b * 100.0);
                                if let Ok(mut st) = state_ref.try_write() {
                                    st.brightness = b;
                                }
                            }
                            if let Some(s) = update.smoothing {
                                cfg.smoothing = s;
                                if let Ok(mut st) = state_ref.try_write() {
                                    st.smooth = s;
                                }
                            }
                            if let Some(g) = update.gamma {
                                cfg.gamma = g;
                                if let Ok(mut st) = state_ref.try_write() {
                                    st.gamma = g;
                                }
                            }
                            if let Some(lb) = update.letterbox {
                                cfg.letterbox_detection = lb;
                            }
                            if let Some(act) = update.is_active {
                                cfg.is_active = act;
                                println!("[API] Питание: {}", if act { "ВКЛ" } else { "ВЫКЛ" });
                                if let Ok(mut st) = state_ref.try_write() {
                                    st.is_active = act;
                                }
                            }
                            if let Some(m) = update.mode {
                                println!("[API] Режим изменен на: {}", m);
                                cfg.mode = m;
                            }
                            if let Some(eff) = update.audio_effect {
                                println!("[API] Эффект светомузыки: {}", eff);
                                cfg.audio_effect = eff;
                            }
                            if let Some(sens) = update.audio_sensitivity {
                                println!("[API] Чувствительность светомузыки: {:.2}", sens);
                                cfg.audio_sensitivity = sens;
                            }
                            if let Some(order) = update.color_order {
                                println!("[API] Порядок цветов: {}", order);
                                cfg.color_order = order;
                            }
                            if let Some(sc) = update.static_color {
                                cfg.static_color = sc;
                            }
                            if let Some(mon) = update.selected_monitor {
                                println!("[API] Выбран монитор: {}", mon);
                                cfg.selected_monitor = mon;
                            }
                            if let Some(auto) = update.autostart {
                                let _ = autostart::set_autostart(auto);
                                println!("[API] Автозагрузка Windows: {}", if auto { "ВКЛ" } else { "ВЫКЛ" });
                            }
                            if let Some(fps) = update.target_fps {
                                println!("[API] Целевой FPS изменен на: {}", fps);
                                cfg.target_fps = fps.clamp(10, 120);
                            }
                            if let Some(baud) = update.baud_rate {
                                println!("[API] Скорость порта изменена на: {}", baud);
                                cfg.baud_rate = baud;
                            }
                            if let Some(port) = update.serial_port {
                                println!("[API] Смена COM-порта на: {}", port);
                                cfg.serial_port = port;
                                recon_ref.store(true, Ordering::Relaxed);
                            }
                            if let Some(true) = update.reconnect {
                                println!("[API] Запрос принудительного аппаратного сброса/переподключения контроллера");
                                recon_ref.store(true, Ordering::Relaxed);
                            }
                            if let Some(h) = update.hotkeys {
                                println!("[API] Обновление настроек горячих клавиш: питание ({}), оверлей ({})", h.toggle_power, h.toggle_overlay);
                                cfg.hotkeys = h;
                                tray_ref.reload_hotkeys();
                            }

                            let _ = cfg.save();
                        }
                    }

                    let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                if req_text.starts_with("GET /favicon.png") {
                    let bytes = include_bytes!("../../../ui/favicon.png");
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    );
                    let _ = stream.write_all(header.as_bytes()).await;
                    let _ = stream.write_all(bytes).await;
                    return;
                }

                if req_text.starts_with("GET /icon.png") {
                    let bytes = include_bytes!("../../../ui/icon.png");
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    );
                    let _ = stream.write_all(header.as_bytes()).await;
                    let _ = stream.write_all(bytes).await;
                    return;
                }

                // Статика: CSS, JS, HTML
                let (content_type, body) = if req_text.starts_with("GET /style.css") {
                    ("text/css", include_str!("../../../ui/style.css"))
                } else if req_text.starts_with("GET /app.js") {
                    ("application/javascript", include_str!("../../../ui/app.js"))
                } else {
                    ("text/html; charset=utf-8", include_str!("../../../ui/index.html"))
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    content_type,
                    body.as_bytes().len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });

    println!(">>> [OK] Web Панель управления доступна по адресу: http://localhost:3637\n");

    // Главный цикл держит процесс живым и периодически выводит статус
    let mut stats_timer = Instant::now();
    while is_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if stats_timer.elapsed() >= Duration::from_secs(3) {
            let cfg = shared_config.read().clone();
            let c_fps = *capture_fps.read();
            let s_fps = *serial_fps.read();
            let status = serial_status.read().clone();

            println!(
                "[Статус] Экран: {:.1} FPS | Порт: {:.1} FPS | Яркость: {:.0}% | Статус: {}",
                c_fps, s_fps, cfg.brightness * 100.0, status
            );
            stats_timer = Instant::now();
        }
    }

    println!("[Выход] Приложение ReAmbiBox завершает работу...");
    let cfg = shared_config.read().clone();
    AdalightDriver::emergency_blank(&cfg.serial_port, cfg.baud_rate, cfg.zones.len());

    Ok(())
}

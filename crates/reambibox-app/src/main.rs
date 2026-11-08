#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod hotkey_parser;
mod single_instance;
mod tray;
mod zone_overlay;

use reambibox_core::api::lightpack::LightpackServer;
use reambibox_core::api::state::create_shared_state;
use reambibox_core::capture::dxgi::DxgiCapture;
use reambibox_core::color::lut::GammaLut;
use reambibox_core::color::smoothing::TemporalSmoother;
use reambibox_core::config::{AppConfig, HotkeysConfig};
use reambibox_core::device::adalight::AdalightDriver;
use reambibox_core::device::wled::{WledDriver, WledProtocol};
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
    output_type: Option<String>,
    wled_host: Option<String>,
    wled_port: Option<u16>,
    wled_protocol: Option<String>,
    white_balance_r: Option<f32>,
    white_balance_g: Option<f32>,
    white_balance_b: Option<f32>,
    black_level_threshold: Option<u8>,
    saturation_boost: Option<f32>,
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
    output_type: String,
    wled_host: String,
    wled_port: u16,
    wled_protocol: String,
    white_balance_r: f32,
    white_balance_g: f32,
    white_balance_b: f32,
    black_level_threshold: u8,
    saturation_boost: f32,
    selected_monitor: usize,
    monitors: Vec<ApiMonitorInfo>,
    led_count: usize,
    serial_port: String,
    serial_status: String,
    available_ports: Vec<String>,
    is_overlay_visible: bool,
    hotkeys: HotkeysConfig,
    presets: Vec<reambibox_core::config::Preset>,
    active_preset: Option<String>,
    zones: Vec<reambibox_core::color::zone::Zone>,
}

#[derive(Debug, Deserialize)]
struct PresetApplyRequest {
    id: String,
}

#[derive(Debug, Deserialize)]
struct PresetSaveRequest {
    name: String,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PresetDeleteRequest {
    id: String,
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

unsafe fn setup_shutdown_handlers(port: String, baud: u32, wled_host: String, wled_port: u16, leds: usize) {
    // 1. Аварийный хук паники
    let panic_port = port.clone();
    let panic_wled_host = wled_host.clone();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[Emergency] Аварийная остановка! Экстренное гашение светодиодов...");
        AdalightDriver::emergency_blank(&panic_port, baud, leds);
        WledDriver::emergency_blank(&panic_wled_host, wled_port, leds);
        default_hook(info);
    }));

    // 2. Сигнал закрытия консоли / Ctrl+C
    unsafe extern "system" fn console_ctrl_handler(dw_ctrl_type: u32) -> windows::Win32::Foundation::BOOL {
        println!("[Завершение] Получен системный сигнал {} — гасим подсветку", dw_ctrl_type);
        let cfg = AppConfig::load();
        AdalightDriver::emergency_blank(&cfg.serial_port, cfg.baud_rate, cfg.zones.len());
        WledDriver::emergency_blank(&cfg.wled_host, cfg.wled_port, cfg.zones.len());
        windows::Win32::Foundation::BOOL(0)
    }

    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Автоматическое замещение предыдущего запущенного экземпляра (Single Instance Takeover)
    single_instance::ensure_single_instance_takeover();

    // Автоматическая актуализация пути автозапуска в реестре Windows (если был включен)
    autostart::sync_autostart_if_needed();

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
        setup_shutdown_handlers(
            initial_cfg.serial_port.clone(),
            initial_cfg.baud_rate,
            initial_cfg.wled_host.clone(),
            initial_cfg.wled_port,
            initial_cfg.zones.len(),
        );
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

    // 4. Выделенный независимый аппаратный поток вывода (Adalight COM-порт / WLED Wi-Fi)
    {
        let colors_ref = Arc::clone(&output_colors);
        let status_ref = Arc::clone(&serial_status);
        let fps_ref = Arc::clone(&serial_fps);
        let run_ref = Arc::clone(&is_running);
        let cfg_ref = Arc::clone(&shared_config);
        let reconnect_ref = Arc::clone(&needs_reconnect);

        std::thread::spawn(move || {
            let (mut current_cfg, mut cached_ver, mut target_fps, mut led_count, mut baud, mut is_wled) = {
                let cfg = cfg_ref.read();
                (
                    cfg.clone(),
                    cfg.version(),
                    cfg.target_fps,
                    cfg.zones.len(),
                    cfg.baud_rate,
                    cfg.output_type.to_lowercase() == "wled",
                )
            };

            fn create_driver(cfg: &AppConfig) -> Box<dyn DeviceDriver> {
                if cfg.output_type.to_lowercase() == "wled" {
                    let proto = WledProtocol::from_str_loose(&cfg.wled_protocol);
                    let mut d = WledDriver::new(&cfg.wled_host, cfg.wled_port, proto);
                    d.set_color_order(&cfg.color_order);
                    d.set_led_count(cfg.zones.len());
                    Box::new(d)
                } else {
                    let mut d = AdalightDriver::new(&cfg.serial_port, cfg.baud_rate);
                    d.set_color_order(&cfg.color_order);
                    d.set_led_count(cfg.zones.len());
                    Box::new(d)
                }
            }

            let mut driver = create_driver(&current_cfg);
            let mut last_reconnect = Instant::now() - Duration::from_secs(5);
            let mut sent_frames: u64 = 0;
            let mut last_fps_time = Instant::now();

            while run_ref.load(Ordering::Relaxed) {
                let start = Instant::now();

                // Обработка ручного запроса переподключения/сброса контроллера
                let force_recon = reconnect_ref.swap(false, Ordering::Relaxed);
                let cur_ver = cfg_ref.read().version();

                let mut should_recreate = force_recon;
                if cur_ver != cached_ver {
                    let cfg = cfg_ref.read();
                    let new_is_wled = cfg.output_type.to_lowercase() == "wled";
                    let hardware_changed = cfg.output_type != current_cfg.output_type
                        || cfg.serial_port != current_cfg.serial_port
                        || cfg.baud_rate != current_cfg.baud_rate
                        || cfg.wled_host != current_cfg.wled_host
                        || cfg.wled_port != current_cfg.wled_port
                        || cfg.wled_protocol != current_cfg.wled_protocol
                        || cfg.color_order != current_cfg.color_order;

                    if hardware_changed || force_recon {
                        should_recreate = true;
                        current_cfg = cfg.clone();
                    }
                    cached_ver = cur_ver;
                    target_fps = cfg.target_fps;
                    led_count = cfg.zones.len();
                    baud = cfg.baud_rate;
                    is_wled = new_is_wled;
                }

                if should_recreate {
                    println!("[Output] Переинициализация драйвера вывода...");
                    let _ = driver.disconnect();
                    driver = create_driver(&current_cfg);
                    last_reconnect = Instant::now() - Duration::from_secs(5);
                }

                // Расчет интервала между кадрами
                let frame_interval = if is_wled {
                    let ms = if target_fps > 0 { 1000 / (target_fps as u64) } else { 33 };
                    Duration::from_millis(ms.max(10))
                } else {
                    // Защита микроконтроллера от переполнения UART буфера:
                    let packet_bytes = (6 + led_count * 3) as u64;
                    let tx_time_ms = (packet_bytes * 10 * 1000) / (baud.max(9600) as u64);
                    let min_safe_interval_ms = tx_time_ms + 7;
                    let requested_interval_ms = if target_fps > 0 {
                        1000 / (target_fps as u64)
                    } else {
                        40
                    };
                    Duration::from_millis(requested_interval_ms.max(min_safe_interval_ms))
                };

                // Автопереподключение к порту / сокету при необходимости
                if !driver.is_connected() {
                    if last_reconnect.elapsed() >= Duration::from_millis(1500) {
                        last_reconnect = Instant::now();
                        match driver.connect() {
                            Ok(_) => {
                                println!(">>> [OK] Успешно подключено к {}", driver.name());
                                let st = if is_wled {
                                    format!("WLED ({}:{})", current_cfg.wled_host, current_cfg.wled_port)
                                } else {
                                    format!("Подключено ({})", current_cfg.serial_port)
                                };
                                *status_ref.write() = st;
                            }
                            Err(e) => {
                                let st = if is_wled {
                                    format!("WLED ({}:{}) недоступен ({})", current_cfg.wled_host, current_cfg.wled_port, e)
                                } else {
                                    format!("Ожидание порта {} ({})", current_cfg.serial_port, e)
                                };
                                *status_ref.write() = st;
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
                            eprintln!("[Output] Ошибка записи в {}: {}", driver.name(), e);
                            *status_ref.write() = format!("Ошибка: {}", e);
                        }
                    }
                }

                // Подсчет реального Output FPS
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
        let audio_ref = Arc::clone(&audio_capture);

        std::thread::spawn(move || {
            let mut current_mon: usize = 99999;
            let mut capture: Option<DxgiCapture> = None;
            let mut smoother = TemporalSmoother::new();
            let mut music_vis = reambibox_core::audio::MusicVisualizer::new();
            let mut letterbox_detector = reambibox_core::color::letterbox::LetterboxDetector::new(15);
            let mut last_letterbox_check = Instant::now() - Duration::from_secs(10);
            let mut active_viewport = reambibox_core::color::zone::NormalizedRect::new(0.0, 0.0, 1.0, 1.0);
            let mut last_sampled: Vec<RgbColor> = Vec::new();
            let mut frames_count: u64 = 0;
            let mut last_fps_time = Instant::now();

            // Локальный кэш параметров конфигурации для полного устранения оверхеда на клонирование строк и зон
            let mut cached_ver: u64 = 0;
            let mut cached_mon_idx: usize = 0;
            let mut cached_is_active: bool = true;
            let mut cached_smoothing: f32 = 0.5;
            let mut cached_mode: String = "screen".to_string();
            let mut cached_static_color: [u8; 3] = [255, 255, 255];
            let mut cached_zones: Vec<reambibox_core::color::zone::Zone> = Vec::new();
            let mut cached_target_fps: u32 = 30;
            let mut cached_target_interval: Duration = Duration::from_millis(33);
            let mut cached_letterbox: bool = true;
            let mut cached_saturation: f32 = 1.0;
            let mut cached_lut = GammaLut::new(1.0, 2.2);
            let mut final_colors_buf: Vec<RgbColor> = Vec::new();

            while run_ref.load(Ordering::Relaxed) {
                let start = Instant::now();

                // Проверяем активность дисплея (сон экрана или блокировка ПК)
                let is_display_active = tray_ref.is_display_active();

                // Считываем обновленные параметры ТОЛЬКО при реальной смене версии конфигурации
                let cur_ver = cfg_ref.read().version();
                if cur_ver != cached_ver {
                    cached_ver = cur_ver;
                    let cfg = cfg_ref.read();
                    cached_mon_idx = cfg.selected_monitor;
                    cached_is_active = cfg.is_active;
                    cached_smoothing = cfg.smoothing;
                    cached_mode = cfg.mode.clone();
                    cached_static_color = cfg.static_color;
                    cached_zones = cfg.zones.clone();
                    cached_target_fps = cfg.target_fps;
                    cached_target_interval = Duration::from_millis(1000 / (cfg.target_fps.clamp(10, 120) as u64));
                    cached_letterbox = cfg.letterbox_detection;
                    cached_saturation = cfg.saturation_boost;
                    cached_lut = GammaLut::new_calibrated(
                        cfg.brightness,
                        cfg.gamma,
                        cfg.white_balance_r,
                        cfg.white_balance_g,
                        cfg.white_balance_b,
                        cfg.black_level_threshold,
                    );

                    // Энергосбережение WASAPI: опрос звука активен только в режиме светомузыки
                    audio_ref.set_active(cached_mode == "music" && cached_is_active);
                }

                let led_count = cached_zones.len();
                if last_sampled.len() != led_count {
                    last_sampled = vec![RgbColor::BLACK; led_count];
                }

                // Обработка состояния: Выключено или Дисплей спит / Сессия заблокирована
                if !cached_is_active || !is_display_active {
                    {
                        let mut out = colors_ref.write();
                        out.clear();
                        out.resize(led_count, RgbColor::BLACK);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                // Обработка режима: Статический тестовый цвет (White / Test)
                if cached_mode == "static" {
                    let base = RgbColor::new(cached_static_color[0], cached_static_color[1], cached_static_color[2]);
                    let dimmed = cached_lut.correct_color(base);
                    {
                        let mut out = colors_ref.write();
                        out.clear();
                        out.resize(led_count, dimmed);
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
                if cached_mode == "music" {
                    let bands = *bands_ref.read();
                    let (effect, sens) = {
                        let c = cfg_ref.read();
                        (c.audio_effect.clone(), c.audio_sensitivity)
                    };
                    let raw_leds = music_vis.render(bands, led_count, &effect, sens);
                    final_colors_buf.clear();
                    for c in raw_leds {
                        final_colors_buf.push(cached_lut.correct_color(c));
                    }
                    {
                        let mut out = colors_ref.write();
                        out.clone_from(&final_colors_buf);
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
                if cached_mon_idx != current_mon || capture.is_none() {
                    match DxgiCapture::new(cached_mon_idx) {
                        Ok(c) => {
                            println!(">>> [OK] DXGI захват переключен на монитор {}", cached_mon_idx);
                            capture = Some(c);
                            current_mon = cached_mon_idx;
                        }
                        Err(e) => {
                            eprintln!("Ошибка инициализации DXGI для монитора {}: {}", cached_mon_idx, e);
                            std::thread::sleep(Duration::from_millis(500));
                            continue;
                        }
                    }
                }

                if let Some(ref mut cap) = capture {
                    let mut frame_captured = false;
                    let timeout = Duration::from_millis((1000 / (cached_target_fps.clamp(10, 120) as u64)).max(15));
                    let res = cap.capture_frame_with(timeout, |view| {
                        // Если включен letterbox, обновляем геометрию раз в секунду
                        if cached_letterbox && last_letterbox_check.elapsed() >= Duration::from_secs(1) {
                            last_letterbox_check = Instant::now();
                            active_viewport = letterbox_detector.detect(view);
                        }

                        // Высокопроизводительная адаптивная выборка цветов зон без аллокаций памяти
                        if cached_letterbox && (active_viewport.height < 0.95 || active_viewport.width < 0.95) {
                            last_sampled = cached_zones.iter().map(|z| {
                                let mut adjusted = z.clone();
                                adjusted.rect.x = active_viewport.x + z.rect.x * active_viewport.width;
                                adjusted.rect.y = active_viewport.y + z.rect.y * active_viewport.height;
                                adjusted.rect.width = z.rect.width * active_viewport.width;
                                adjusted.rect.height = z.rect.height * active_viewport.height;
                                adjusted.sample_color_adaptive(view)
                            }).collect();
                        } else {
                            last_sampled = cached_zones.iter().map(|z| z.sample_color_adaptive(view)).collect();
                        }

                        frames_count += 1;
                        frame_captured = true;
                    });

                    match res {
                        Ok(()) => {}
                        Err(reambibox_core::capture::CaptureError::Timeout) => {
                            // Содержимое экрана не изменилось — пропускаем повторный расчет
                        }
                        Err(reambibox_core::capture::CaptureError::AccessLost) => {
                            std::thread::sleep(Duration::from_millis(200));
                            let _ = cap.reconnect();
                        }
                        Err(_) => {
                            std::thread::sleep(Duration::from_millis(15));
                        }
                    }

                    if frame_captured {
                        // Высокопроизводительное сглаживание по месту, коррекция насыщенности и LUT без промежуточных аллокаций
                        smoother.smooth_into(
                            &last_sampled,
                            cached_smoothing,
                            &cached_lut,
                            cached_saturation,
                            &mut final_colors_buf,
                        );

                        {
                            let mut out = colors_ref.write();
                            out.clone_from(&final_colors_buf);
                        }
                    }
                }

                if last_fps_time.elapsed() >= Duration::from_secs(1) {
                    *fps_ref.write() = (frames_count as f64 / last_fps_time.elapsed().as_secs_f64()) as f32;
                    frames_count = 0;
                    last_fps_time = Instant::now();
                }

                let elapsed = start.elapsed();
                if elapsed < cached_target_interval {
                    std::thread::sleep(cached_target_interval - elapsed);
                }
            }

            // Гасим ленту при остановке потока захвата
            {
                let mut out = colors_ref.write();
                let count = out.len();
                out.clear();
                out.resize(count, RgbColor::BLACK);
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
                        output_type: cfg.output_type.clone(),
                        wled_host: cfg.wled_host.clone(),
                        wled_port: cfg.wled_port,
                        wled_protocol: cfg.wled_protocol.clone(),
                        white_balance_r: cfg.white_balance_r,
                        white_balance_g: cfg.white_balance_g,
                        white_balance_b: cfg.white_balance_b,
                        black_level_threshold: cfg.black_level_threshold,
                        saturation_boost: cfg.saturation_boost,
                        selected_monitor: cfg.selected_monitor,
                        monitors,
                        led_count: cfg.zones.len(),
                        serial_port: cfg.serial_port.clone(),
                        serial_status: port_status,
                        available_ports: AdalightDriver::available_ports(),
                        is_overlay_visible: overlay_ref.is_visible(),
                        hotkeys: cfg.hotkeys.clone(),
                        presets: cfg.presets.clone(),
                        active_preset: cfg.active_preset.clone(),
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
                            if let Some(ot) = update.output_type {
                                println!("[API] Смена типа вывода: {}", ot);
                                cfg.output_type = ot;
                                recon_ref.store(true, Ordering::Relaxed);
                            }
                            if let Some(wh) = update.wled_host {
                                println!("[API] Смена WLED хоста: {}", wh);
                                cfg.wled_host = wh;
                                recon_ref.store(true, Ordering::Relaxed);
                            }
                            if let Some(wp) = update.wled_port {
                                println!("[API] Смена WLED порта: {}", wp);
                                cfg.wled_port = wp;
                                recon_ref.store(true, Ordering::Relaxed);
                            }
                            if let Some(wpr) = update.wled_protocol {
                                println!("[API] Смена WLED протокола: {}", wpr);
                                cfg.wled_protocol = wpr;
                                recon_ref.store(true, Ordering::Relaxed);
                            }
                            if let Some(wr) = update.white_balance_r {
                                cfg.white_balance_r = wr.clamp(0.0, 2.0);
                            }
                            if let Some(wg) = update.white_balance_g {
                                cfg.white_balance_g = wg.clamp(0.0, 2.0);
                            }
                            if let Some(wb) = update.white_balance_b {
                                cfg.white_balance_b = wb.clamp(0.0, 2.0);
                            }
                            if let Some(bt) = update.black_level_threshold {
                                cfg.black_level_threshold = bt.min(100);
                            }
                            if let Some(sb) = update.saturation_boost {
                                cfg.saturation_boost = sb.clamp(0.0, 2.0);
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

                            // При ручной правке параметров сбрасываем привязку к активному пресету
                            cfg.active_preset = None;

                            let _ = cfg.save();
                        }
                    }

                    let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                    let _ = stream.write_all(response.as_bytes()).await;
                    return;
                }

                // API: POST /api/presets/apply
                if req_text.starts_with("POST /api/presets/apply") {
                    let mut applied = false;
                    if let Some(body_start) = req_text.find("\r\n\r\n") {
                        let body = &req_text[body_start + 4..];
                        if let Ok(req) = serde_json::from_str::<PresetApplyRequest>(body) {
                            let mut cfg = config_ref.write();
                            if cfg.apply_preset(&req.id) {
                                if let Ok(mut st) = state_ref.try_write() {
                                    st.brightness = cfg.brightness;
                                    st.smooth = cfg.smoothing;
                                    st.gamma = cfg.gamma;
                                    st.is_active = cfg.is_active;
                                }
                                let _ = cfg.save();
                                println!("[API] Применён пресет: {}", req.id);
                                applied = true;
                            }
                        }
                    }
                    if applied {
                        let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    } else {
                        let response = "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 26\r\nConnection: close\r\n\r\n{\"error\":\"Пресет не найден\"}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                    return;
                }

                // API: POST /api/presets/save
                if req_text.starts_with("POST /api/presets/save") {
                    let mut saved_id: Option<String> = None;
                    if let Some(body_start) = req_text.find("\r\n\r\n") {
                        let body = &req_text[body_start + 4..];
                        if let Ok(req) = serde_json::from_str::<PresetSaveRequest>(body) {
                            let mut cfg = config_ref.write();
                            let preset_id = cfg.save_preset(&req.name, req.id.as_deref());
                            let _ = cfg.save();
                            println!("[API] Сохранён пресет: {} ({})", req.name, preset_id);
                            saved_id = Some(preset_id);
                        }
                    }
                    if let Some(id) = saved_id {
                        let json = format!("{{\"status\":\"ok\",\"id\":\"{}\"}}", id);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            json.len(),
                            json
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                    } else {
                        let response = "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 32\r\nConnection: close\r\n\r\n{\"error\":\"Неверные данные пресета\"}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                    return;
                }

                // API: POST /api/presets/delete
                if req_text.starts_with("POST /api/presets/delete") {
                    let mut deleted = false;
                    if let Some(body_start) = req_text.find("\r\n\r\n") {
                        let body = &req_text[body_start + 4..];
                        if let Ok(req) = serde_json::from_str::<PresetDeleteRequest>(body) {
                            let mut cfg = config_ref.write();
                            if cfg.delete_preset(&req.id) {
                                let _ = cfg.save();
                                println!("[API] Удалён пресет: {}", req.id);
                                deleted = true;
                            }
                        }
                    }
                    if deleted {
                        let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    } else {
                        let response = "HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: 26\r\nConnection: close\r\n\r\n{\"error\":\"Пресет не найден\"}";
                        let _ = stream.write_all(response.as_bytes()).await;
                    }
                    return;
                }

                // API: POST /api/wled/test
                if req_text.starts_with("POST /api/wled/test") {
                    let cfg = config_ref.read().clone();
                    let host = cfg.wled_host.clone();
                    let port = cfg.wled_port;
                    let proto = WledProtocol::from_str_loose(&cfg.wled_protocol);
                    let mut test_driver = WledDriver::new(&host, port, proto);
                    test_driver.set_color_order(&cfg.color_order);
                    let (res_status, res_msg) = match test_driver.connect() {
                        Ok(_) => {
                            let frame = DeviceFrame::new(vec![RgbColor::new(0, 255, 0); cfg.zones.len().max(10)]);
                            let _ = test_driver.send_frame(&frame);
                            std::thread::sleep(Duration::from_millis(300));
                            let _ = test_driver.blank_leds();
                            ("ok", "Тестовый кадр (зеленый) отправлен на контроллер WLED")
                        }
                        Err(e) => {
                            eprintln!("[API] Ошибка теста WLED: {}", e);
                            ("error", "Не удалось отправить сетевой UDP пакет на WLED")
                        }
                    };
                    let json = format!("{{\"status\":\"{}\",\"message\":\"{}\"}}", res_status, res_msg);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        json.len(),
                        json
                    );
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
    WledDriver::emergency_blank(&cfg.wled_host, cfg.wled_port, cfg.zones.len());

    Ok(())
}

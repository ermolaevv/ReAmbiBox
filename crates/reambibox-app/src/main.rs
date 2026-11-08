use reambibox_core::api::lightpack::LightpackServer;
use reambibox_core::api::state::create_shared_state;
use reambibox_core::capture::dxgi::DxgiCapture;
use reambibox_core::color::smoothing::TemporalSmoother;
use reambibox_core::config::AppConfig;
use reambibox_core::device::adalight::AdalightDriver;
use reambibox_core::device::traits::DeviceDriver;
use reambibox_core::device::types::{DeviceFrame, RgbColor};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct ConfigUpdate {
    brightness: Option<f32>,
    smoothing: Option<f32>,
    gamma: Option<f32>,
    letterbox: Option<bool>,
    is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ApiStatusResponse {
    is_active: bool,
    fps: f32,
    brightness: f32,
    smoothing: f32,
    gamma: f32,
    letterbox: bool,
    led_count: usize,
    serial_port: String,
    zones: Vec<reambibox_core::color::zone::Zone>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("        ReAmbiBox v0.1.0 — High-Performance Ambilight      ");
    println!("============================================================");

    // 1. Загрузка конфигурации (98 зон из AmbiBox)
    let initial_cfg = AppConfig::load();
    println!("Загружена конфигурация:");
    println!("  - COM-порт: {} ({} бод)", initial_cfg.serial_port, initial_cfg.baud_rate);
    println!("  - Активных светодиодных зон: {}", initial_cfg.zones.len());
    println!("  - Сглаживание: {:.2}, Яркость: {:.2}", initial_cfg.smoothing, initial_cfg.brightness);

    let shared_config = Arc::new(RwLock::new(initial_cfg.clone()));
    let shared_state = create_shared_state();
    {
        let mut s = shared_state.write().await;
        s.led_count = initial_cfg.zones.len();
        s.brightness = initial_cfg.brightness;
        s.smooth = initial_cfg.smoothing;
        s.gamma = initial_cfg.gamma;
    }

    // 2. Запуск Lightpack API сервера (порт 3636)
    let lp_server = LightpackServer::new(shared_state.clone(), LightpackServer::DEFAULT_PORT);
    tokio::spawn(async move {
        let _ = lp_server.run().await;
    });
    println!(">>> [OK] Lightpack API сервер активен на 127.0.0.1:3636");

    // 3. Запуск WebUI + REST API сервера (порт 3637)
    let web_config = Arc::clone(&shared_config);
    let web_state = Arc::clone(&shared_state);
    let live_fps = Arc::new(parking_lot::RwLock::new(0.0f32));
    let web_fps = Arc::clone(&live_fps);

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
            let fps_ref = Arc::clone(&web_fps);

            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                if let Ok(n) = stream.read(&mut buf).await {
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);

                    // API: GET /api/config
                    if req.starts_with("GET /api/config") {
                        let cfg = config_ref.read().await;
                        let state = state_ref.read().await;
                        let fps = *fps_ref.read();

                        let res_obj = ApiStatusResponse {
                            is_active: state.is_active,
                            fps,
                            brightness: cfg.brightness,
                            smoothing: cfg.smoothing,
                            gamma: cfg.gamma,
                            letterbox: cfg.letterbox_detection,
                            led_count: cfg.zones.len(),
                            serial_port: cfg.serial_port.clone(),
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

                    // API: POST /api/config
                    if req.starts_with("POST /api/config") {
                        if let Some(body_start) = req.find("\r\n\r\n") {
                            let body = &req[body_start + 4..];
                            if let Ok(update) = serde_json::from_str::<ConfigUpdate>(body) {
                                let mut cfg = config_ref.write().await;
                                let mut state = state_ref.write().await;

                                if let Some(b) = update.brightness {
                                    cfg.brightness = b;
                                    state.brightness = b;
                                }
                                if let Some(s) = update.smoothing {
                                    cfg.smoothing = s;
                                    state.smooth = s;
                                }
                                if let Some(g) = update.gamma {
                                    cfg.gamma = g;
                                    state.gamma = g;
                                }
                                if let Some(lb) = update.letterbox {
                                    cfg.letterbox_detection = lb;
                                }
                                if let Some(act) = update.is_active {
                                    state.is_active = act;
                                }

                                let _ = cfg.save();
                            }
                        }

                        let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                        let _ = stream.write_all(response.as_bytes()).await;
                        return;
                    }

                    // Статика: CSS, JS, HTML
                    let (content_type, body) = if req.starts_with("GET /style.css") {
                        ("text/css", include_str!("../../../ui/style.css"))
                    } else if req.starts_with("GET /app.js") {
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
                }
            });
        }
    });
    println!(">>> [OK] Web Панель управления доступна по адресу: http://localhost:3637");

    // 4. Подключение к Adalight (COM10)
    let mut driver = AdalightDriver::new(&initial_cfg.serial_port, initial_cfg.baud_rate);
    match driver.connect() {
        Ok(_) => println!(">>> [OK] Успешно открыт порт {} для {} зон", initial_cfg.serial_port, initial_cfg.zones.len()),
        Err(e) => eprintln!("Предупреждение по порту {}: {}", initial_cfg.serial_port, e),
    }

    // 5. Разделяемый буфер кадра DXGI для многопоточной развязки
    let latest_frame = Arc::new(parking_lot::RwLock::new(None));
    let is_running = Arc::new(AtomicBool::new(true));

    let capture_frame_ref = Arc::clone(&latest_frame);
    let capture_running = Arc::clone(&is_running);
    let selected_mon = initial_cfg.selected_monitor;

    std::thread::spawn(move || {
        let mut capture = match DxgiCapture::new(selected_mon) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Ошибка инициализации DXGI: {}", e);
                return;
            }
        };

        while capture_running.load(Ordering::Relaxed) {
            match capture.capture_frame(Duration::from_millis(33)) {
                Ok(frame) => {
                    let mut lock = capture_frame_ref.write();
                    *lock = Some(frame);
                }
                Err(reambibox_core::capture::CaptureError::Timeout) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(reambibox_core::capture::CaptureError::AccessLost) => {
                    std::thread::sleep(Duration::from_millis(300));
                    let _ = capture.reconnect();
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    });

    println!(">>> [OK] Аппаратный поток DXGI захвата активен.");
    println!(">>> [OK] Сквозной конвейер стриминга 98 светодиодов в COM10 запущен!\n");

    let mut smoother = TemporalSmoother::new();
    let mut last_colors: Vec<RgbColor> = vec![RgbColor::BLACK; initial_cfg.zones.len()];
    let target_interval = Duration::from_millis(16); // 60 FPS таргет

    let mut last_stats = Instant::now();
    let mut sent_frames = 0;

    loop {
        let loop_start = Instant::now();

        // 1. Читаем актуальное динамическое состояние (яркость, активность, сглаживание)
        let (is_active, brightness, smoothing, zones) = {
            let state = shared_state.read().await;
            let cfg = shared_config.read().await;
            (state.is_active, cfg.brightness, cfg.smoothing, cfg.zones.clone())
        };

        if is_active {
            // 2. Расчет цветов зон по последнему кадру экрана
            let new_colors = {
                let lock = latest_frame.read();
                if let Some(ref frame) = *lock {
                    let sampled: Vec<_> = zones.iter().map(|z| z.sample_color(frame, 4)).collect();
                    Some(sampled)
                } else {
                    None
                }
            };

            if let Some(sampled) = new_colors {
                last_colors = sampled;
            }

            // 3. Временное сглаживание
            let smoothed = smoother.smooth(&last_colors, smoothing);

            // 4. Коррекция яркости в реальном времени!
            let final_colors: Vec<_> = smoothed
                .into_iter()
                .map(|c| c.with_brightness(brightness))
                .collect();

            // 5. Передача пакета в COM10
            if driver.is_connected() {
                let frame = DeviceFrame::new(final_colors);
                let _ = driver.send_frame(&frame);
                sent_frames += 1;
            }
        } else {
            // Если подсветка выключена, гасим диоды
            if driver.is_connected() {
                let black_frame = DeviceFrame::new(vec![RgbColor::BLACK; zones.len()]);
                let _ = driver.send_frame(&black_frame);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }

        // Подсчет и публикация реального FPS
        if last_stats.elapsed() >= Duration::from_secs(2) {
            let fps = (sent_frames as f64) / last_stats.elapsed().as_secs_f64();
            *live_fps.write() = fps as f32;

            println!(
                "[Статистика] Скорость: {:.1} FPS | Зон: {} | Яркость: {:.0}% | Порт: {}",
                fps, zones.len(), brightness * 100.0, initial_cfg.serial_port
            );
            sent_frames = 0;
            last_stats = Instant::now();
        }

        let elapsed = loop_start.elapsed();
        if elapsed < target_interval {
            tokio::time::sleep(target_interval - elapsed).await;
        }
    }
}

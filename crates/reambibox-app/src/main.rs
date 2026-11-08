use reambibox_core::api::lightpack::LightpackServer;
use reambibox_core::api::state::create_shared_state;
use reambibox_core::capture::dxgi::DxgiCapture;
use reambibox_core::color::smoothing::TemporalSmoother;
use reambibox_core::config::AppConfig;
use reambibox_core::device::adalight::AdalightDriver;
use reambibox_core::device::traits::DeviceDriver;
use reambibox_core::device::types::{DeviceFrame, RgbColor};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("        ReAmbiBox v0.1.0 — High-Performance Ambilight      ");
    println!("============================================================");

    // 1. Загрузка конфигурации (включая 97 зон из старого AmbiBox)
    let config = Arc::new(RwLock::new(AppConfig::load()));
    let cfg = config.read().await.clone();

    println!("Загружена конфигурация:");
    println!("  - COM-порт: {} ({} бод)", cfg.serial_port, cfg.baud_rate);
    println!("  - Активных светодиодных зон: {}", cfg.zones.len());
    println!("  - Сглаживание: {:.2}, Яркость: {:.2}", cfg.smoothing, cfg.brightness);

    // 2. Инициализация разделяемого состояния и Lightpack API сервера (порт 3636)
    let shared_state = create_shared_state();
    {
        let mut s = shared_state.write().await;
        s.led_count = cfg.zones.len();
        s.brightness = cfg.brightness;
        s.smooth = cfg.smoothing;
    }
    let lp_server = LightpackServer::new(shared_state.clone(), LightpackServer::DEFAULT_PORT);
    tokio::spawn(async move {
        let _ = lp_server.run().await;
    });
    println!(">>> [OK] Lightpack API сервер активен на 127.0.0.1:3636");

    // 3. Запуск локального WebUI сервера на порту 3637
    tokio::spawn(async move {
        let listener = match TcpListener::bind("127.0.0.1:3637").await {
            Ok(l) => l,
            Err(_) => return,
        };
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                if let Ok(n) = stream.read(&mut buf).await {
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let (content_type, body) = if req.contains("GET /style.css") {
                        ("text/css", include_str!("../../../ui/style.css"))
                    } else if req.contains("GET /app.js") {
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
    let mut driver = AdalightDriver::new(&cfg.serial_port, cfg.baud_rate);
    match driver.connect() {
        Ok(_) => println!(">>> [OK] Успешно открыт порт {} для {} зон", cfg.serial_port, cfg.zones.len()),
        Err(e) => eprintln!("Предупреждение по порту {}: {}", cfg.serial_port, e),
    }

    // 5. Разделяемый буфер для развязки потоков Захвата и Вывода
    let latest_frame = Arc::new(parking_lot::RwLock::new(None));
    let is_running = Arc::new(AtomicBool::new(true));

    // Поток аппаратного захвата экрана (DXGI)
    let capture_frame_ref = Arc::clone(&latest_frame);
    let capture_running = Arc::clone(&is_running);
    let selected_mon = cfg.selected_monitor;

    std::thread::spawn(move || {
        let mut capture = match DxgiCapture::new(selected_mon) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Ошибка DXGI захвата: {}", e);
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
                    // Экран не изменился — спим немного
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

    println!(">>> [OK] Поток DXGI захвата запущен.");

    // Основной высокоскоростной поток передачи в COM10 (гарантированные 60 FPS)
    let mut smoother = TemporalSmoother::new();
    let mut last_colors: Vec<RgbColor> = vec![RgbColor::BLACK; cfg.zones.len()];
    let target_interval = Duration::from_millis(16); // ~60 FPS

    println!(">>> [OK] Сквозной конвейер активен. Стриминг 97 светодиодов запущен!\n");

    let mut last_stats = Instant::now();
    let mut sent_frames = 0;

    loop {
        let loop_start = Instant::now();

        // 1. Проверяем активность
        let is_active = {
            let s = shared_state.read().await;
            s.is_active
        };

        if is_active {
            // 2. Берем свежий кадр экрана (если есть) и рассчитываем зоны
            let new_colors = {
                let lock = latest_frame.read();
                if let Some(ref frame) = *lock {
                    let sampled: Vec<_> = cfg.zones.iter().map(|z| z.sample_color(frame, 4)).collect();
                    Some(sampled)
                } else {
                    None
                }
            };

            if let Some(sampled) = new_colors {
                last_colors = sampled;
            }

            // 3. Временное сглаживание
            let smoothed = smoother.smooth(&last_colors, cfg.smoothing);

            // 4. Коррекция яркости
            let final_colors: Vec<_> = smoothed
                .into_iter()
                .map(|c| c.with_brightness(cfg.brightness))
                .collect();

            // 5. Передача в COM-порт
            if driver.is_connected() {
                let frame = DeviceFrame::new(final_colors);
                let _ = driver.send_frame(&frame);
                sent_frames += 1;
            }
        }

        // Вывод статистики в консоль каждые 2 секунды
        if last_stats.elapsed() >= Duration::from_secs(2) {
            let fps = (sent_frames as f64) / last_stats.elapsed().as_secs_f64();
            println!(
                "[Статистика] Поток подсветки: {:.1} FPS ({} кадров/сек) | Светодиодов: {} шт | Порт: {}",
                fps, sent_frames / 2, cfg.zones.len(), cfg.serial_port
            );
            sent_frames = 0;
            last_stats = Instant::now();
        }

        // Выдерживаем стабильный интервал 60 FPS
        let elapsed = loop_start.elapsed();
        if elapsed < target_interval {
            tokio::time::sleep(target_interval - elapsed).await;
        }
    }
}

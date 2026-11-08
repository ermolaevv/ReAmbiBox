use reambibox_core::api::lightpack::LightpackServer;
use reambibox_core::api::state::create_shared_state;
use reambibox_core::capture::dxgi::DxgiCapture;
use reambibox_core::color::smoothing::TemporalSmoother;
use reambibox_core::config::AppConfig;
use reambibox_core::device::adalight::AdalightDriver;
use reambibox_core::device::traits::DeviceDriver;
use reambibox_core::device::types::DeviceFrame;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("        ReAmbiBox v0.1.0 — High-Performance Ambilight      ");
    println!("============================================================");

    // 1. Загрузка конфигурации
    let config = AppConfig::load();
    println!("Загружена конфигурация:");
    println!("  - COM-порт: {} ({} бод)", config.serial_port, config.baud_rate);
    println!("  - Количество зон: {}", config.zones.len());
    println!("  - Сглаживание: {:.2}, Яркость: {:.2}", config.smoothing, config.brightness);

    // 2. Инициализация и запуск фонового Lightpack API TCP сервера на порту 3636
    let shared_state = create_shared_state();
    {
        let mut s = shared_state.write().await;
        s.led_count = config.zones.len();
        s.brightness = config.brightness;
        s.smooth = config.smoothing;
    }
    let server = LightpackServer::new(shared_state.clone(), LightpackServer::DEFAULT_PORT);
    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Ошибка Lightpack TCP сервера: {}", e);
        }
    });
    println!(">>> [OK] Lightpack API сервер активен на 127.0.0.1:3636");

    // 3. Подключение к Adalight устройству (COM10)
    let mut driver = AdalightDriver::new(&config.serial_port, config.baud_rate);
    match driver.connect() {
        Ok(_) => println!(">>> [OK] Успешно подключено к Adalight на порту {}", config.serial_port),
        Err(e) => {
            eprintln!("Предупреждение: Не удалось открыть порт {}: {}. Продолжаем в режиме симуляции.", config.serial_port, e);
        }
    }

    // 4. Инициализация захвата экрана через DXGI
    let monitors = DxgiCapture::list_monitors();
    let primary_idx = monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0);
    println!("Инициализация захвата экрана (монитор #{})...", primary_idx);

    let mut capture = match DxgiCapture::new(primary_idx) {
        Ok(c) => {
            let (w, h) = c.dimensions();
            println!(">>> [OK] DXGI Capture готов ({}x{})", w, h);
            c
        }
        Err(e) => {
            eprintln!("Критическая ошибка инициализации DXGI: {}", e);
            return Err(e.into());
        }
    };

    let mut smoother = TemporalSmoother::new();
    println!("\n>>> Сквозной конвейер запущен! Нажмите Ctrl+C для выхода.");

    let mut frame_count: u64 = 0;
    let mut last_stats = Instant::now();
    let mut last_latency = Duration::default();

    // Запуск цикла захвата и передачи
    loop {
        let frame_start = Instant::now();

        // 1. Проверяем флаг активности из состояния (можно выключать через API)
        let is_active = {
            let s = shared_state.read().await;
            s.is_active
        };

        if !is_active {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }

        // 2. Аппаратный захват кадра
        match capture.capture_frame(Duration::from_millis(50)) {
            Ok(captured) => {
                // 3. Расчет цветов зон (downsampling step 4)
                let raw_colors: Vec<_> = config
                    .zones
                    .iter()
                    .map(|z| z.sample_color(&captured, 4))
                    .collect();

                // 4. Временное сглаживание
                let smoothed = smoother.smooth(&raw_colors, config.smoothing);

                // 5. Коррекция яркости
                let final_colors: Vec<_> = smoothed
                    .into_iter()
                    .map(|c| c.with_brightness(config.brightness))
                    .collect();

                // 6. Отправка в COM-порт
                if driver.is_connected() {
                    let frame = DeviceFrame::new(final_colors);
                    let _ = driver.send_frame(&frame);
                }

                frame_count += 1;
                last_latency = frame_start.elapsed();
            }
            Err(reambibox_core::capture::CaptureError::Timeout) => {
                // Экран не изменился, задержка для экономии ресурсов
                tokio::time::sleep(Duration::from_millis(8)).await;
            }
            Err(reambibox_core::capture::CaptureError::AccessLost) => {
                eprintln!("Сессия дисплея потеряна, переподключение...");
                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = capture.reconnect();
            }
            Err(e) => {
                eprintln!("Ошибка захвата: {}", e);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        // Вывод статистики в консоль раз в 2 секунды
        if last_stats.elapsed() >= Duration::from_secs(2) {
            let fps = (frame_count as f64) / last_stats.elapsed().as_secs_f64();
            println!(
                "[Статистика] Скорость: {:.1} FPS | Задержка цикла кадра: {:.2?} | Передано кадров: {}",
                fps, last_latency, frame_count
            );
            frame_count = 0;
            last_stats = Instant::now();
        }
    }
}

use reambibox_core::capture::dxgi::DxgiCapture;
use std::time::{Duration, Instant};

fn main() {
    println!("=== ReAmbiBox DXGI Capture Benchmark ===");

    let monitors = DxgiCapture::list_monitors();
    let primary = monitors.iter().find(|m| m.is_primary).unwrap_or(&monitors[0]);
    println!(
        "Тестирование захвата на основном мониторе: [{}] {} ({}x{})...",
        primary.index, primary.output_name, primary.width, primary.height
    );

    let mut capture = match DxgiCapture::new(primary.index) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Ошибка инициализации: {}", e);
            return;
        }
    };

    println!("Захват 60 кадров...");
    let start = Instant::now();
    let mut captured_frames = 0;
    let mut timeouts = 0;

    while captured_frames < 60 && start.elapsed() < Duration::from_secs(3) {
        match capture.capture_frame(Duration::from_millis(50)) {
            Ok(frame) => {
                captured_frames += 1;
                if captured_frames == 1 {
                    println!(
                        "Первый кадр: {}x{}, row_pitch={}, буфер={} МБ",
                        frame.width,
                        frame.height,
                        frame.row_pitch,
                        frame.data.len() / (1024 * 1024)
                    );
                }
            }
            Err(reambibox_core::capture::CaptureError::Timeout) => {
                timeouts += 1;
            }
            Err(e) => {
                eprintln!("Ошибка в цикле захвата: {}", e);
                break;
            }
        }
    }

    let elapsed = start.elapsed();
    let fps = (captured_frames as f64) / elapsed.as_secs_f64();
    println!(
        ">>> [OK] Захвачено {} кадров (таймаутов: {}) за {:.2?} (эффективная скорость: {:.1} FPS)",
        captured_frames, timeouts, elapsed, fps
    );
}

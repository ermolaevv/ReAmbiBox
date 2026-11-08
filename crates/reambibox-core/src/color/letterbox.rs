use crate::capture::CapturedFrame;
use super::zone::NormalizedRect;

/// Детектор черных полос (Letterbox / Pillarbox)
pub struct LetterboxDetector {
    /// Порог яркости, ниже которого пиксель считается черным (0..255)
    pub black_threshold: u8,
    /// Текущие обнаруженные границы полезного содержимого
    pub active_viewport: NormalizedRect,
}

impl LetterboxDetector {
    pub fn new(black_threshold: u8) -> Self {
        Self {
            black_threshold,
            active_viewport: NormalizedRect::new(0.0, 0.0, 1.0, 1.0),
        }
    }

    /// Анализ кадра и определение границ полезного изображения (без черных полос)
    pub fn detect(&mut self, frame: &CapturedFrame) -> NormalizedRect {
        let width = frame.width;
        let height = frame.height;
        let row_pitch = frame.row_pitch as usize;
        let data = &frame.data;

        let sample_cols = [width / 4, width / 2, (width * 3) / 4];

        // 1. Поиск верхней границы (Top Letterbox)
        let mut top_bound = 0;
        'top_loop: for y in (0..height / 3).step_by(4) {
            for &x in &sample_cols {
                let offset = (y as usize) * row_pitch + (x as usize) * 4;
                if offset + 3 <= data.len() {
                    let b = data[offset];
                    let g = data[offset + 1];
                    let r = data[offset + 2];
                    let luma = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                    if luma > self.black_threshold {
                        top_bound = y;
                        break 'top_loop;
                    }
                }
            }
        }

        // 2. Поиск нижней границы (Bottom Letterbox)
        let mut bottom_bound = height;
        'bottom_loop: for y in ((height * 2 / 3)..height).rev().step_by(4) {
            for &x in &sample_cols {
                let offset = (y as usize) * row_pitch + (x as usize) * 4;
                if offset + 3 <= data.len() {
                    let b = data[offset];
                    let g = data[offset + 1];
                    let r = data[offset + 2];
                    let luma = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                    if luma > self.black_threshold {
                        bottom_bound = y;
                        break 'bottom_loop;
                    }
                }
            }
        }

        let y_norm = (top_bound as f32) / (height as f32);
        let h_norm = ((bottom_bound - top_bound) as f32) / (height as f32);

        self.active_viewport = NormalizedRect::new(0.0, y_norm, 1.0, h_norm);
        self.active_viewport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_letterbox_detection_on_21_9() {
        let width = 100;
        let height = 100;
        let row_pitch = width * 4;
        let mut data = vec![0u8; (row_pitch * height) as usize];

        // Заполняем середину (y от 20 до 80) белым цветом, а верх и низ оставляем черными
        for y in 20..80 {
            for x in 0..width {
                let offset = (y * row_pitch + x * 4) as usize;
                data[offset] = 255;
                data[offset + 1] = 255;
                data[offset + 2] = 255;
                data[offset + 3] = 255;
            }
        }

        let frame = CapturedFrame {
            width,
            height,
            row_pitch,
            data,
        };

        let mut detector = LetterboxDetector::new(10);
        let vp = detector.detect(&frame);

        // Верхняя граница должна быть обнаружена около 0.20
        assert!((vp.y - 0.20).abs() < 0.05);
        // Высота должна быть около 0.60
        assert!((vp.height - 0.60).abs() < 0.05);
    }
}

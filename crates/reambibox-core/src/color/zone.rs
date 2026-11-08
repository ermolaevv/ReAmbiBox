use crate::capture::CapturedFrame;
use crate::device::types::RgbColor;
use serde::{Deserialize, Serialize};

/// Нормализованный прямоугольник области экрана (координаты от 0.0 до 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl NormalizedRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
            width: width.clamp(0.0, 1.0),
            height: height.clamp(0.0, 1.0),
        }
    }

    /// Преобразование в абсолютные пиксельные координаты для заданного разрешения кадра
    #[inline]
    pub fn to_pixel_rect(&self, frame_width: u32, frame_height: u32) -> (u32, u32, u32, u32) {
        let px = ((self.x * frame_width as f32).round() as u32).min(frame_width.saturating_sub(1));
        let py = ((self.y * frame_height as f32).round() as u32).min(frame_height.saturating_sub(1));
        let pw = ((self.width * frame_width as f32).round() as u32).max(1);
        let ph = ((self.height * frame_height as f32).round() as u32).max(1);

        let final_w = pw.min(frame_width - px);
        let final_h = ph.min(frame_height - py);

        (px, py, final_w, final_h)
    }
}

/// Светодиодная зона захвата
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub id: usize,
    pub rect: NormalizedRect,
    pub name: String,
}

impl Zone {
    pub fn new(id: usize, rect: NormalizedRect) -> Self {
        Self {
            id,
            name: format!("Zone {}", id),
            rect,
        }
    }

    /// Быстрый расчет среднего цвета RGB зоны по BGRA кадру с шагом выборки (downsampling)
    pub fn sample_color(&self, frame: &CapturedFrame, step: u32) -> RgbColor {
        let (px, py, pw, ph) = self.rect.to_pixel_rect(frame.width, frame.height);
        let step = step.max(1);

        let mut sum_r: u64 = 0;
        let mut sum_g: u64 = 0;
        let mut sum_b: u64 = 0;
        let mut count: u64 = 0;

        let row_pitch = frame.row_pitch as usize;
        let data = &frame.data;

        let mut y = py;
        while y < py + ph {
            let row_offset = (y as usize) * row_pitch;
            let mut x = px;
            while x < px + pw {
                let pixel_offset = row_offset + (x as usize) * 4;
                if pixel_offset + 3 <= data.len() {
                    // Формат DXGI BGRA8: data[0]=B, data[1]=G, data[2]=R
                    let b = data[pixel_offset] as u64;
                    let g = data[pixel_offset + 1] as u64;
                    let r = data[pixel_offset + 2] as u64;

                    sum_b += b;
                    sum_g += g;
                    sum_r += r;
                    count += 1;
                }
                x += step;
            }
            y += step;
        }

        if count == 0 {
            return RgbColor::BLACK;
        }

        RgbColor::new(
            (sum_r / count) as u8,
            (sum_g / count) as u8,
            (sum_b / count) as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_rect_conversion() {
        let rect = NormalizedRect::new(0.0, 0.0, 0.5, 0.5);
        let (x, y, w, h) = rect.to_pixel_rect(1920, 1080);
        assert_eq!((x, y, w, h), (0, 0, 960, 540));
    }

    #[test]
    fn test_zone_color_sampling() {
        // Создаем синтетический кадр 100x100, залитый красным цветом (BGRA: B=0, G=0, R=255, A=255)
        let width = 100;
        let height = 100;
        let row_pitch = width * 4;
        let mut data = vec![0u8; (row_pitch * height) as usize];

        for i in 0..(width * height) as usize {
            data[i * 4] = 0;       // B
            data[i * 4 + 1] = 0;   // G
            data[i * 4 + 2] = 255; // R
            data[i * 4 + 3] = 255; // A
        }

        let frame = CapturedFrame {
            width,
            height,
            row_pitch,
            data,
        };

        let zone = Zone::new(0, NormalizedRect::new(0.0, 0.0, 1.0, 1.0));
        let color = zone.sample_color(&frame, 2);
        assert_eq!(color, RgbColor::new(255, 0, 0));
    }
}

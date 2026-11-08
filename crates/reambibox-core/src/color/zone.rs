use crate::capture::{AsFrameView, FrameView};
#[cfg(test)]
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

    /// Расчет среднего цвета RGB зоны с заданным фиксированным шагом выборки
    pub fn sample_color(&self, frame: &impl AsFrameView, step: u32) -> RgbColor {
        self.sample_color_view(&frame.as_frame_view(), Some(step))
    }

    /// Адаптивный расчет цвета зоны: оптимальная сетка точек (~144..256 замеров) для максимальной производительности
    pub fn sample_color_adaptive(&self, frame: &impl AsFrameView) -> RgbColor {
        self.sample_color_view(&frame.as_frame_view(), None)
    }

    /// Высокопроизводительная выборка среднего цвета зоны напрямую из FrameView
    pub fn sample_color_view(&self, frame: &FrameView<'_>, step_override: Option<u32>) -> RgbColor {
        let (px, py, pw, ph) = self.rect.to_pixel_rect(frame.width, frame.height);
        if pw == 0 || ph == 0 {
            return RgbColor::BLACK;
        }

        // Адаптивный шаг выборки: для Ambilight достаточно сетки ~12x12..16x16 точек (144..256 замеров)
        let step = match step_override {
            Some(s) if s > 0 => s,
            _ => {
                let target_samples_per_axis = 16u32;
                let step_x = (pw / target_samples_per_axis).max(1);
                let step_y = (ph / target_samples_per_axis).max(1);
                step_x.max(step_y).max(2)
            }
        };

        let mut sum_r: u64 = 0;
        let mut sum_g: u64 = 0;
        let mut sum_b: u64 = 0;
        let mut count: u64 = 0;

        let row_pitch = frame.row_pitch as usize;
        let data = frame.data;
        let data_len = data.len();

        let mut y = py;
        while y < py + ph {
            let row_offset = (y as usize) * row_pitch;
            let mut x = px;
            while x < px + pw {
                let pixel_offset = row_offset + (x as usize) * 4;
                if pixel_offset + 4 <= data_len {
                    // Быстрое 32-битное чтение пикселя (BGRA8) одной инструкцией
                    let pixel_val = unsafe {
                        std::ptr::read_unaligned(data.as_ptr().add(pixel_offset) as *const u32)
                    };
                    let b = (pixel_val & 0xFF) as u64;
                    let g = ((pixel_val >> 8) & 0xFF) as u64;
                    let r = ((pixel_val >> 16) & 0xFF) as u64;

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

        // Проверка адаптивной выборки
        let adaptive_color = zone.sample_color_adaptive(&frame);
        assert_eq!(adaptive_color, RgbColor::new(255, 0, 0));

        // Проверка заимствованного FrameView (Zero-Allocation)
        let view = frame.as_view();
        let view_color = zone.sample_color_adaptive(&view);
        assert_eq!(view_color, RgbColor::new(255, 0, 0));
    }
}


pub mod zone;
pub mod smoothing;
pub mod letterbox;
pub mod lut;

pub use zone::{NormalizedRect, Zone};
pub use smoothing::TemporalSmoother;
pub use letterbox::LetterboxDetector;
pub use lut::GammaLut;

use crate::device::types::RgbColor;

/// Коррекция насыщенности цвета без сдвига цветового тона (Rec. 601 Luma)
#[inline(always)]
pub fn apply_saturation(color: RgbColor, saturation: f32) -> RgbColor {
    if (saturation - 1.0).abs() < 0.005 {
        return color;
    }

    let r = color.r as f32;
    let g = color.g as f32;
    let b = color.b as f32;

    // Монохромная яркость Luma по стандарту ITU-R BT.601
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;

    let sat = saturation.clamp(0.0, 2.0);
    let new_r = (luma + (r - luma) * sat).clamp(0.0, 255.0).round() as u8;
    let new_g = (luma + (g - luma) * sat).clamp(0.0, 255.0).round() as u8;
    let new_b = (luma + (b - luma) * sat).clamp(0.0, 255.0).round() as u8;

    RgbColor::new(new_r, new_g, new_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_saturation_identity() {
        let col = RgbColor::new(100, 150, 200);
        assert_eq!(apply_saturation(col, 1.0), col);
    }

    #[test]
    fn test_apply_saturation_grayscale() {
        let col = RgbColor::new(255, 0, 0); // Чисто красный
        let gray = apply_saturation(col, 0.0);
        // Luma для красного (255 * 0.299) ~ 76
        assert_eq!(gray.r, gray.g);
        assert_eq!(gray.g, gray.b);
        assert_eq!(gray.r, 76);
    }

    #[test]
    fn test_apply_saturation_boost() {
        let col = RgbColor::new(180, 120, 100);
        let boosted = apply_saturation(col, 1.5);
        // Доминирующий канал R должен вырасти, меньшие уменьшиться
        assert!(boosted.r > col.r);
        assert!(boosted.b < col.b);
    }
}

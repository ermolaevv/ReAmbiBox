use crate::device::types::RgbColor;

/// Предрассчитанная 256-байтная таблица поиска для гамма- и яркостной коррекции (Zero-Cost Gamma LUT)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GammaLut {
    pub table: [u8; 256],
}

impl Default for GammaLut {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

impl GammaLut {
    /// Построение таблицы поиска для заданной яркости (0.0..1.0) и показателя гаммы (например, 2.2)
    pub fn new(brightness: f32, gamma: f32) -> Self {
        let b = brightness.clamp(0.0, 1.0);
        let g = if gamma > 0.05 { gamma } else { 1.0 };
        let mut table = [0u8; 256];

        for i in 0..=255 {
            let normalized = (i as f32 / 255.0) * b;
            let corrected = normalized.powf(g).clamp(0.0, 1.0);
            table[i] = (corrected * 255.0).round() as u8;
        }

        Self { table }
    }

    /// Коррекция одиночного цветового канала за 1 такт процессора O(1)
    #[inline(always)]
    pub fn correct_channel(&self, val: u8) -> u8 {
        self.table[val as usize]
    }

    /// Коррекция цвета RGB за 3 такта процессора через L1-кэш без вызовов powf
    #[inline(always)]
    pub fn correct_color(&self, color: RgbColor) -> RgbColor {
        RgbColor {
            r: self.table[color.r as usize],
            g: self.table[color.g as usize],
            b: self.table[color.b as usize],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamma_lut_identity() {
        let lut = GammaLut::new(1.0, 1.0);
        assert_eq!(lut.correct_channel(0), 0);
        assert_eq!(lut.correct_channel(128), 128);
        assert_eq!(lut.correct_channel(255), 255);
    }

    #[test]
    fn test_gamma_lut_half_brightness() {
        let lut = GammaLut::new(0.5, 1.0);
        assert_eq!(lut.correct_channel(0), 0);
        assert_eq!(lut.correct_channel(255), 128);
    }

    #[test]
    fn test_gamma_lut_correct_color() {
        let lut = GammaLut::new(1.0, 2.2);
        let color = RgbColor::new(0, 128, 255);
        let corrected = lut.correct_color(color);
        assert_eq!(corrected.r, 0);
        assert_eq!(corrected.b, 255);
        // При гамме 2.2 средний оттенок 128 становится темнее (~55)
        assert!(corrected.g < 70 && corrected.g > 40);
    }
}


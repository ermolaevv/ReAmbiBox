use crate::device::types::RgbColor;

/// Предрассчитанная 768-байтная таблица поиска для гамма-, яркостной и поканальной калибровки (Zero-Cost Calibrated Gamma LUT)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GammaLut {
    pub lut_r: [u8; 256],
    pub lut_g: [u8; 256],
    pub lut_b: [u8; 256],
    pub black_threshold: u8,
}

impl Default for GammaLut {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}

impl GammaLut {
    /// Построение таблицы поиска по умолчанию (баланс белого 1.0:1.0:1.0, порог черного 0)
    pub fn new(brightness: f32, gamma: f32) -> Self {
        Self::new_calibrated(brightness, gamma, 1.0, 1.0, 1.0, 0)
    }

    /// Построение таблицы калибровки с балансом белого R/G/B и порогом отсечки чёрного
    pub fn new_calibrated(
        brightness: f32,
        gamma: f32,
        gain_r: f32,
        gain_g: f32,
        gain_b: f32,
        black_threshold: u8,
    ) -> Self {
        let b = brightness.clamp(0.0, 1.0);
        let g = if gamma > 0.05 { gamma } else { 1.0 };
        let r_gain = gain_r.clamp(0.0, 2.0);
        let g_gain = gain_g.clamp(0.0, 2.0);
        let b_gain = gain_b.clamp(0.0, 2.0);

        let mut lut_r = [0u8; 256];
        let mut lut_g = [0u8; 256];
        let mut lut_b = [0u8; 256];

        for i in 0..=255 {
            let norm = i as f32 / 255.0;
            let base = (norm * b).powf(g);

            lut_r[i] = ((base * r_gain).clamp(0.0, 1.0) * 255.0).round() as u8;
            lut_g[i] = ((base * g_gain).clamp(0.0, 1.0) * 255.0).round() as u8;
            lut_b[i] = ((base * b_gain).clamp(0.0, 1.0) * 255.0).round() as u8;
        }

        Self {
            lut_r,
            lut_g,
            lut_b,
            black_threshold,
        }
    }

    /// Коррекция одиночного цветового канала за 1 такт процессора O(1)
    #[inline(always)]
    pub fn correct_channel(&self, val: u8) -> u8 {
        self.lut_r[val as usize]
    }

    /// Коррекция цвета RGB за 3 такта процессора через L1-кэш с аппаратной отсечкой черного
    #[inline(always)]
    pub fn correct_color(&self, color: RgbColor) -> RgbColor {
        if self.black_threshold > 0
            && color.r <= self.black_threshold
            && color.g <= self.black_threshold
            && color.b <= self.black_threshold
        {
            return RgbColor::BLACK;
        }

        RgbColor {
            r: self.lut_r[color.r as usize],
            g: self.lut_g[color.g as usize],
            b: self.lut_b[color.b as usize],
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
        assert!(corrected.g < 70 && corrected.g > 40);
    }

    #[test]
    fn test_gamma_lut_white_balance() {
        // Уменьшаем зеленый на 50%, синий на 20%
        let lut = GammaLut::new_calibrated(1.0, 1.0, 1.0, 0.5, 0.8, 0);
        let white = RgbColor::new(200, 200, 200);
        let balanced = lut.correct_color(white);
        assert_eq!(balanced.r, 200);
        assert_eq!(balanced.g, 100);
        assert_eq!(balanced.b, 160);
    }

    #[test]
    fn test_gamma_lut_black_threshold() {
        let lut = GammaLut::new_calibrated(1.0, 1.0, 1.0, 1.0, 1.0, 15);
        // Ниже порога -> черный
        assert_eq!(lut.correct_color(RgbColor::new(10, 15, 8)), RgbColor::BLACK);
        // Выше порога хотя бы по одному каналу -> не черный
        assert_ne!(lut.correct_color(RgbColor::new(16, 5, 5)), RgbColor::BLACK);
    }
}


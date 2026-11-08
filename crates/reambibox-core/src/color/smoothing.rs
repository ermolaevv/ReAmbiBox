use super::lut::GammaLut;
use crate::device::types::RgbColor;

/// Экспоненциальный фильтр временного сглаживания (Temporal Smoothing)
pub struct TemporalSmoother {
    current_colors: Vec<[f32; 3]>,
}

impl TemporalSmoother {
    pub fn new() -> Self {
        Self {
            current_colors: Vec::new(),
        }
    }

    /// Сглаживание массива целевых цветов
    /// factor: от 0.0 (без сглаживания, мгновенная реакция) до 0.99 (максимально плавные переходы)
    pub fn smooth(&mut self, targets: &[RgbColor], factor: f32) -> Vec<RgbColor> {
        let factor = factor.clamp(0.0, 0.99);

        if self.current_colors.len() != targets.len() {
            // Инициализация при смене количества зон
            self.current_colors = targets
                .iter()
                .map(|c| [c.r as f32, c.g as f32, c.b as f32])
                .collect();
            return targets.to_vec();
        }

        let mut result = Vec::with_capacity(targets.len());

        for (curr, target) in self.current_colors.iter_mut().zip(targets.iter()) {
            let tr = target.r as f32;
            let tg = target.g as f32;
            let tb = target.b as f32;

            curr[0] = curr[0] * factor + tr * (1.0 - factor);
            curr[1] = curr[1] * factor + tg * (1.0 - factor);
            curr[2] = curr[2] * factor + tb * (1.0 - factor);

            result.push(RgbColor::new(
                curr[0].round() as u8,
                curr[1].round() as u8,
                curr[2].round() as u8,
            ));
        }

        result
    }

    /// Высокопроизводительное сглаживание цветов по месту с одновременным применением насыщенности и Gamma LUT
    /// Не производит аллокаций памяти при неизменном размере вектора out
    pub fn smooth_into(
        &mut self,
        targets: &[RgbColor],
        factor: f32,
        lut: &GammaLut,
        saturation: f32,
        out: &mut Vec<RgbColor>,
    ) {
        let factor = factor.clamp(0.0, 0.99);
        out.clear();
        out.reserve(targets.len());

        let need_sat = (saturation - 1.0).abs() >= 0.005;

        if self.current_colors.len() != targets.len() {
            self.current_colors = targets
                .iter()
                .map(|c| [c.r as f32, c.g as f32, c.b as f32])
                .collect();
            for t in targets {
                let col = if need_sat { super::apply_saturation(*t, saturation) } else { *t };
                out.push(lut.correct_color(col));
            }
            return;
        }

        for (curr, target) in self.current_colors.iter_mut().zip(targets.iter()) {
            let tr = target.r as f32;
            let tg = target.g as f32;
            let tb = target.b as f32;

            curr[0] = curr[0] * factor + tr * (1.0 - factor);
            curr[1] = curr[1] * factor + tg * (1.0 - factor);
            curr[2] = curr[2] * factor + tb * (1.0 - factor);

            let raw = RgbColor::new(
                curr[0].round() as u8,
                curr[1].round() as u8,
                curr[2].round() as u8,
            );
            let col = if need_sat { super::apply_saturation(raw, saturation) } else { raw };
            out.push(lut.correct_color(col));
        }
    }

    pub fn reset(&mut self) {
        self.current_colors.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_smoothing_step_response() {
        let mut smoother = TemporalSmoother::new();

        // 1. Старт с черного цвета
        let black = vec![RgbColor::BLACK];
        let out1 = smoother.smooth(&black, 0.5);
        assert_eq!(out1[0], RgbColor::BLACK);

        // 2. Резкий скачок на белый цвет (255)
        let white = vec![RgbColor::WHITE];
        let out2 = smoother.smooth(&white, 0.5);
        // curr = 0 * 0.5 + 255 * 0.5 = 127.5 -> 128
        assert_eq!(out2[0], RgbColor::new(128, 128, 128));

        // 3. Следующий шаг
        let out3 = smoother.smooth(&white, 0.5);
        // curr = 127.5 * 0.5 + 255 * 0.5 = 63.75 + 127.5 = 191.25 -> 191
        assert_eq!(out3[0], RgbColor::new(191, 191, 191));
    }

    #[test]
    fn test_temporal_smoothing_smooth_into() {
        let mut smoother = TemporalSmoother::new();
        let lut = GammaLut::new(1.0, 1.0);
        let mut out = Vec::new();

        let black = vec![RgbColor::BLACK];
        smoother.smooth_into(&black, 0.5, &lut, 1.0, &mut out);
        assert_eq!(out[0], RgbColor::BLACK);

        let white = vec![RgbColor::WHITE];
        smoother.smooth_into(&white, 0.5, &lut, 1.0, &mut out);
        assert_eq!(out[0], RgbColor::new(128, 128, 128));

        // Проверка с насыщенностью 0.0 (ч/б)
        let mut out_bw = Vec::new();
        let mut bw_smoother = TemporalSmoother::new();
        let red = vec![RgbColor::new(255, 0, 0)];
        bw_smoother.smooth_into(&red, 0.0, &lut, 0.0, &mut out_bw);
        assert_eq!(out_bw[0].r, out_bw[0].g);
        assert_eq!(out_bw[0].g, out_bw[0].b);
    }
}


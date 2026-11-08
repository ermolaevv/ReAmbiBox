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
}

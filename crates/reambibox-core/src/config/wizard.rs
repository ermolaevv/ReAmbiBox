use crate::color::zone::{NormalizedRect, Zone};

/// Мастер автоматической генерации зон подсветки (Zone Wizard)
pub struct ZoneWizard;

impl ZoneWizard {
    /// Генерация зон по периметру экрана (по часовой стрелке, начиная с нижнего левого угла)
    /// depth: глубина зоны захвата (0.05..0.20 от экрана)
    pub fn generate_perimeter(
        top_count: usize,
        bottom_count: usize,
        left_count: usize,
        right_count: usize,
    ) -> Vec<Zone> {
        let depth = 0.10f32; // 10% от края экрана
        let mut zones = Vec::with_capacity(top_count + bottom_count + left_count + right_count);
        let mut id = 0;

        // 1. Нижняя сторона (слева направо)
        if bottom_count > 0 {
            let step = 1.0 / (bottom_count as f32);
            for i in 0..bottom_count {
                let x = (i as f32) * step;
                let rect = NormalizedRect::new(x, 1.0 - depth, step, depth);
                zones.push(Zone::new(id, rect));
                id += 1;
            }
        }

        // 2. Правая сторона (снизу вверх)
        if right_count > 0 {
            let step = 1.0 / (right_count as f32);
            for i in (0..right_count).rev() {
                let y = (i as f32) * step;
                let rect = NormalizedRect::new(1.0 - depth, y, depth, step);
                zones.push(Zone::new(id, rect));
                id += 1;
            }
        }

        // 3. Верхняя сторона (справа налево)
        if top_count > 0 {
            let step = 1.0 / (top_count as f32);
            for i in (0..top_count).rev() {
                let x = (i as f32) * step;
                let rect = NormalizedRect::new(x, 0.0, step, depth);
                zones.push(Zone::new(id, rect));
                id += 1;
            }
        }

        // 4. Левая сторона (сверху вниз)
        if left_count > 0 {
            let step = 1.0 / (left_count as f32);
            for i in 0..left_count {
                let y = (i as f32) * step;
                let rect = NormalizedRect::new(0.0, y, depth, step);
                zones.push(Zone::new(id, rect));
                id += 1;
            }
        }

        zones
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perimeter_generation() {
        let zones = ZoneWizard::generate_perimeter(20, 20, 10, 10);
        assert_eq!(zones.len(), 60);

        // Первая зона внизу слева
        assert_eq!(zones[0].id, 0);
        assert_eq!(zones[0].rect.y, 0.9);

        // Зоны для 30 диодов
        let zones_30 = ZoneWizard::generate_perimeter(10, 10, 5, 5);
        assert_eq!(zones_30.len(), 30);

        // Зоны для 120 диодов
        let zones_120 = ZoneWizard::generate_perimeter(40, 40, 20, 20);
        assert_eq!(zones_120.len(), 120);
    }
}

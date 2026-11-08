use crate::color::zone::{NormalizedRect, Zone};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartCorner {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

impl Default for StartCorner {
    fn default() -> Self {
        StartCorner::BottomLeft
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Clockwise
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerimeterParams {
    pub top_count: usize,
    pub bottom_count: usize,
    pub left_count: usize,
    pub right_count: usize,
    #[serde(default)]
    pub start_corner: StartCorner,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default = "default_depth")]
    pub depth: f32,
}

fn default_depth() -> f32 {
    0.08f32
}

impl Default for PerimeterParams {
    fn default() -> Self {
        Self {
            top_count: 26,
            bottom_count: 25,
            left_count: 24,
            right_count: 23,
            start_corner: StartCorner::BottomLeft,
            direction: Direction::Clockwise,
            depth: 0.08,
        }
    }
}

/// Мастер автоматической генерации зон подсветки (Zone Wizard)
pub struct ZoneWizard;

impl ZoneWizard {
    /// Гибкая генерация периметра с произвольными сторонами, углом старта и направлением
    pub fn generate_custom_perimeter(params: &PerimeterParams) -> Vec<Zone> {
        let depth = params.depth.clamp(0.02, 0.30);
        let total = params.top_count + params.bottom_count + params.left_count + params.right_count;
        let mut zones = Vec::with_capacity(total);
        let mut id = 0;

        // Порядок обхода сторон в зависимости от стартового угла и направления
        // Side: 0=Left, 1=Top, 2=Right, 3=Bottom
        let sides_order: Vec<(usize, bool)> = match (params.start_corner, params.direction) {
            // Нижний левый:
            // Clockwise: Left (вверх), Top (вправо), Right (вниз), Bottom (влево)
            (StartCorner::BottomLeft, Direction::Clockwise) => {
                vec![(0, true), (1, false), (2, false), (3, true)]
            }
            // CounterClockwise: Bottom (вправо), Right (вверх), Top (влево), Left (вниз)
            (StartCorner::BottomLeft, Direction::CounterClockwise) => {
                vec![(3, false), (2, true), (1, true), (0, false)]
            }

            // Нижний правый:
            // Clockwise: Bottom (влево), Left (вверх), Top (вправо), Right (вниз)
            (StartCorner::BottomRight, Direction::Clockwise) => {
                vec![(3, true), (0, true), (1, false), (2, false)]
            }
            // CounterClockwise: Right (вверх), Top (влево), Left (вниз), Bottom (вправо)
            (StartCorner::BottomRight, Direction::CounterClockwise) => {
                vec![(2, true), (1, true), (0, false), (3, false)]
            }

            // Верхний левый:
            // Clockwise: Top (вправо), Right (вниз), Bottom (влево), Left (вверх)
            (StartCorner::TopLeft, Direction::Clockwise) => {
                vec![(1, false), (2, false), (3, true), (0, true)]
            }
            // CounterClockwise: Left (вниз), Bottom (вправо), Right (вверх), Top (влево)
            (StartCorner::TopLeft, Direction::CounterClockwise) => {
                vec![(0, false), (3, false), (2, true), (1, true)]
            }

            // Верхний правый:
            // Clockwise: Right (вниз), Bottom (влево), Left (вверх), Top (вправо)
            (StartCorner::TopRight, Direction::Clockwise) => {
                vec![(2, false), (3, true), (0, true), (1, false)]
            }
            // CounterClockwise: Top (влево), Left (вниз), Bottom (вправо), Right (вверх)
            (StartCorner::TopRight, Direction::CounterClockwise) => {
                vec![(1, true), (0, false), (3, false), (2, true)]
            }
        };

        for (side, reverse) in sides_order {
            match side {
                // Левая сторона (x = 0.0)
                0 if params.left_count > 0 => {
                    let step = 1.0 / (params.left_count as f32);
                    let range: Box<dyn Iterator<Item = usize>> = if reverse {
                        // Снизу вверх (1.0 -> 0.0)
                        Box::new((0..params.left_count).rev())
                    } else {
                        // Сверху вниз (0.0 -> 1.0)
                        Box::new(0..params.left_count)
                    };

                    for i in range {
                        let y = (i as f32) * step;
                        let rect = NormalizedRect::new(0.0, y, depth, step);
                        zones.push(Zone::new(id, rect));
                        id += 1;
                    }
                }

                // Верхняя сторона (y = 0.0)
                1 if params.top_count > 0 => {
                    let step = 1.0 / (params.top_count as f32);
                    let range: Box<dyn Iterator<Item = usize>> = if reverse {
                        // Справа налево (1.0 -> 0.0)
                        Box::new((0..params.top_count).rev())
                    } else {
                        // Слева направо (0.0 -> 1.0)
                        Box::new(0..params.top_count)
                    };

                    for i in range {
                        let x = (i as f32) * step;
                        let rect = NormalizedRect::new(x, 0.0, step, depth);
                        zones.push(Zone::new(id, rect));
                        id += 1;
                    }
                }

                // Правая сторона (x = 1.0 - depth)
                2 if params.right_count > 0 => {
                    let step = 1.0 / (params.right_count as f32);
                    let range: Box<dyn Iterator<Item = usize>> = if reverse {
                        // Снизу вверх (1.0 -> 0.0)
                        Box::new((0..params.right_count).rev())
                    } else {
                        // Сверху вниз (0.0 -> 1.0)
                        Box::new(0..params.right_count)
                    };

                    for i in range {
                        let y = (i as f32) * step;
                        let rect = NormalizedRect::new(1.0 - depth, y, depth, step);
                        zones.push(Zone::new(id, rect));
                        id += 1;
                    }
                }

                // Нижняя сторона (y = 1.0 - depth)
                3 if params.bottom_count > 0 => {
                    let step = 1.0 / (params.bottom_count as f32);
                    let range: Box<dyn Iterator<Item = usize>> = if reverse {
                        // Справа налево (1.0 -> 0.0)
                        Box::new((0..params.bottom_count).rev())
                    } else {
                        // Слева направо (0.0 -> 1.0)
                        Box::new(0..params.bottom_count)
                    };

                    for i in range {
                        let x = (i as f32) * step;
                        let rect = NormalizedRect::new(x, 1.0 - depth, step, depth);
                        zones.push(Zone::new(id, rect));
                        id += 1;
                    }
                }

                _ => {}
            }
        }

        zones
    }

    /// Обратная совместимость со старым вызовом генерации
    pub fn generate_perimeter(
        top_count: usize,
        bottom_count: usize,
        left_count: usize,
        right_count: usize,
    ) -> Vec<Zone> {
        let params = PerimeterParams {
            top_count,
            bottom_count,
            left_count,
            right_count,
            start_corner: StartCorner::BottomLeft,
            direction: Direction::CounterClockwise,
            depth: 0.10,
        };
        Self::generate_custom_perimeter(&params)
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

    #[test]
    fn test_ambibox_98_zone_clockwise_generation() {
        // Настройка пользователя: 98 LED (26 верх, 25 низ, 24 лево, 23 право)
        // Старт: нижний левый угол, по часовой стрелке (вверх по левой грани)
        let params = PerimeterParams {
            top_count: 26,
            bottom_count: 25,
            left_count: 24,
            right_count: 23,
            start_corner: StartCorner::BottomLeft,
            direction: Direction::Clockwise,
            depth: 0.08,
        };

        let zones = ZoneWizard::generate_custom_perimeter(&params);
        assert_eq!(zones.len(), 98);

        // Зона 0 (id 0) - низ левой стороны
        assert_eq!(zones[0].id, 0);
        assert_eq!(zones[0].rect.x, 0.0);
        // Последняя зона левой стороны (зона 23) - верх левой стороны (y около 0.0)
        assert_eq!(zones[23].id, 23);
        assert_eq!(zones[23].rect.x, 0.0);
        assert!(zones[23].rect.y < zones[0].rect.y);

        // Зона 24 (первая зона верха) - левый верх
        assert_eq!(zones[24].id, 24);
        assert_eq!(zones[24].rect.y, 0.0);

        // Зона 49 (последняя зона верха) - правый верх
        assert_eq!(zones[49].id, 49);
        assert_eq!(zones[49].rect.y, 0.0);
        assert!(zones[49].rect.x > zones[24].rect.x);

        // Зона 50 (первая зона правой стороны) - правый верх
        assert_eq!(zones[50].id, 50);
        assert_eq!(zones[50].rect.x, 1.0 - 0.08);

        // Зона 72 (последняя зона правой стороны) - правый низ
        assert_eq!(zones[72].id, 72);
        assert_eq!(zones[72].rect.x, 1.0 - 0.08);
        assert!(zones[72].rect.y > zones[50].rect.y);

        // Зона 73 (первая зона низа) - правый низ
        assert_eq!(zones[73].id, 73);
        assert_eq!(zones[73].rect.y, 1.0 - 0.08);

        // Зона 97 (последняя зона) - левый низ
        assert_eq!(zones[97].id, 97);
        assert_eq!(zones[97].rect.y, 1.0 - 0.08);
        assert!(zones[97].rect.x < zones[73].rect.x);
    }
}


use serde::{Deserialize, Serialize};

/// 24-битный цвет RGB одного светодиода
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255 };

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Применение масштабирования яркости (0.0..1.0)
    #[inline]
    pub fn with_brightness(self, brightness: f32) -> Self {
        let factor = brightness.clamp(0.0, 1.0);
        Self {
            r: (self.r as f32 * factor).round() as u8,
            g: (self.g as f32 * factor).round() as u8,
            b: (self.b as f32 * factor).round() as u8,
        }
    }
}

impl From<(u8, u8, u8)> for RgbColor {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self { r, g, b }
    }
}

impl From<[u8; 3]> for RgbColor {
    fn from([r, g, b]: [u8; 3]) -> Self {
        Self { r, g, b }
    }
}

/// Кадр данных для передачи на светодиодное устройство
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceFrame {
    pub colors: Vec<RgbColor>,
    pub timestamp_ms: u64,
}

impl DeviceFrame {
    pub fn new(colors: Vec<RgbColor>) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            colors,
            timestamp_ms,
        }
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_creation_and_conversion() {
        let color = RgbColor::new(255, 128, 0);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);

        let from_tuple: RgbColor = (10, 20, 30).into();
        assert_eq!(from_tuple, RgbColor::new(10, 20, 30));

        let from_array: RgbColor = [1, 2, 3].into();
        assert_eq!(from_array, RgbColor::new(1, 2, 3));
    }

    #[test]
    fn test_brightness_scaling() {
        let color = RgbColor::new(200, 100, 50);
        let dimmed = color.with_brightness(0.5);
        assert_eq!(dimmed, RgbColor::new(100, 50, 25));

        let black = color.with_brightness(0.0);
        assert_eq!(black, RgbColor::BLACK);
    }

    #[test]
    fn test_device_frame() {
        let colors = vec![RgbColor::new(255, 0, 0), RgbColor::new(0, 255, 0)];
        let frame = DeviceFrame::new(colors.clone());
        assert_eq!(frame.len(), 2);
        assert!(!frame.is_empty());
        assert_eq!(frame.colors, colors);
    }
}


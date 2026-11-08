pub mod wizard;

use crate::color::zone::Zone;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(skip, default = "default_version_counter")]
    pub version_counter: Arc<AtomicU64>,
    pub selected_monitor: usize,
    pub serial_port: String,
    pub baud_rate: u32,
    pub led_count: usize,
    pub brightness: f32,
    pub smoothing: f32,
    pub gamma: f32,
    pub letterbox_detection: bool,
    pub target_fps: u32,
    #[serde(default = "default_is_active")]
    pub is_active: bool,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_color_order")]
    pub color_order: String,
    #[serde(default = "default_static_color")]
    pub static_color: [u8; 3],
    #[serde(default = "default_audio_effect")]
    pub audio_effect: String,
    #[serde(default = "default_audio_sensitivity")]
    pub audio_sensitivity: f32,
    #[serde(default = "default_output_type")]
    pub output_type: String,
    #[serde(default = "default_wled_host")]
    pub wled_host: String,
    #[serde(default = "default_wled_port")]
    pub wled_port: u16,
    #[serde(default = "default_wled_protocol")]
    pub wled_protocol: String,
    #[serde(default = "default_one_f32")]
    pub white_balance_r: f32,
    #[serde(default = "default_one_f32")]
    pub white_balance_g: f32,
    #[serde(default = "default_one_f32")]
    pub white_balance_b: f32,
    #[serde(default)]
    pub black_level_threshold: u8,
    #[serde(default = "default_one_f32")]
    pub saturation_boost: f32,
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub active_preset: Option<String>,
    #[serde(default)]
    pub hotkeys: HotkeysConfig,
    pub zones: Vec<Zone>,
}

/// Пользовательский профиль настроек подсветки (Preset)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub brightness: f32,
    pub smoothing: f32,
    pub gamma: f32,
    pub saturation_boost: f32,
    pub white_balance_r: f32,
    pub white_balance_g: f32,
    pub white_balance_b: f32,
    pub black_level_threshold: u8,
    pub letterbox_detection: bool,
    pub target_fps: u32,
    pub static_color: [u8; 3],
    pub audio_effect: String,
    pub audio_sensitivity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotkeysConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_toggle_power")]
    pub toggle_power: String,
    #[serde(default = "default_brightness_up")]
    pub brightness_up: String,
    #[serde(default = "default_brightness_down")]
    pub brightness_down: String,
    #[serde(default = "default_toggle_overlay")]
    pub toggle_overlay: String,
}

fn default_true() -> bool {
    true
}

fn default_toggle_power() -> String {
    "Ctrl+Shift+A".to_string()
}

fn default_brightness_up() -> String {
    "Ctrl+Shift+Up".to_string()
}

fn default_brightness_down() -> String {
    "Ctrl+Shift+Down".to_string()
}

fn default_toggle_overlay() -> String {
    "Ctrl+Shift+O".to_string()
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            toggle_power: default_toggle_power(),
            brightness_up: default_brightness_up(),
            brightness_down: default_brightness_down(),
            toggle_overlay: default_toggle_overlay(),
        }
    }
}

fn default_is_active() -> bool {
    true
}

fn default_mode() -> String {
    "screen".to_string()
}

fn default_color_order() -> String {
    "RGB".to_string()
}

fn default_static_color() -> [u8; 3] {
    [255, 255, 255]
}

fn default_audio_effect() -> String {
    "pulse".to_string()
}

fn default_audio_sensitivity() -> f32 {
    1.0
}

fn default_output_type() -> String {
    "serial".to_string()
}

fn default_wled_host() -> String {
    "192.168.1.100".to_string()
}

fn default_wled_port() -> u16 {
    4048
}

fn default_wled_protocol() -> String {
    "ddp".to_string()
}

fn default_one_f32() -> f32 {
    1.0
}

fn default_version_counter() -> Arc<AtomicU64> {
    Arc::new(AtomicU64::new(1))
}

impl PartialEq for AppConfig {
    fn eq(&self, other: &Self) -> bool {
        self.selected_monitor == other.selected_monitor
            && self.serial_port == other.serial_port
            && self.baud_rate == other.baud_rate
            && self.led_count == other.led_count
            && self.brightness == other.brightness
            && self.smoothing == other.smoothing
            && self.gamma == other.gamma
            && self.letterbox_detection == other.letterbox_detection
            && self.target_fps == other.target_fps
            && self.is_active == other.is_active
            && self.mode == other.mode
            && self.color_order == other.color_order
            && self.static_color == other.static_color
            && self.audio_effect == other.audio_effect
            && self.audio_sensitivity == other.audio_sensitivity
            && self.output_type == other.output_type
            && self.wled_host == other.wled_host
            && self.wled_port == other.wled_port
            && self.wled_protocol == other.wled_protocol
            && self.white_balance_r == other.white_balance_r
            && self.white_balance_g == other.white_balance_g
            && self.white_balance_b == other.white_balance_b
            && self.black_level_threshold == other.black_level_threshold
            && self.saturation_boost == other.saturation_boost
            && self.presets == other.presets
            && self.active_preset == other.active_preset
            && self.hotkeys == other.hotkeys
            && self.zones == other.zones
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let led_count = 60;
        let zones = wizard::ZoneWizard::generate_perimeter(20, 20, 10, 10);
        Self {
            version_counter: default_version_counter(),
            selected_monitor: 0,
            serial_port: "COM10".to_string(),
            baud_rate: 115200,
            led_count,
            brightness: 1.0,
            smoothing: 0.5,
            gamma: 2.2,
            letterbox_detection: true,
            target_fps: 30,
            is_active: true,
            mode: default_mode(),
            color_order: default_color_order(),
            static_color: default_static_color(),
            audio_effect: default_audio_effect(),
            audio_sensitivity: default_audio_sensitivity(),
            output_type: default_output_type(),
            wled_host: default_wled_host(),
            wled_port: default_wled_port(),
            wled_protocol: default_wled_protocol(),
            white_balance_r: 1.0,
            white_balance_g: 1.0,
            white_balance_b: 1.0,
            black_level_threshold: 0,
            saturation_boost: 1.0,
            presets: Vec::new(),
            active_preset: None,
            hotkeys: HotkeysConfig::default(),
            zones,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(appdata);
        path.push("ReAmbiBox");
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str(&content) {
                    return cfg;
                }
            }
        }
        let default = Self::default();
        let _ = default.save();
        default
    }

    pub fn version(&self) -> u64 {
        self.version_counter.load(Ordering::Acquire)
    }

    pub fn bump_version(&self) {
        self.version_counter.fetch_add(1, Ordering::Release);
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        self.bump_version();
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, json)
    }

    /// Применение параметров из сохраненного пресета по его id
    pub fn apply_preset(&mut self, preset_id: &str) -> bool {
        if let Some(p) = self.presets.iter().find(|p| p.id == preset_id).cloned() {
            self.mode = p.mode;
            self.brightness = p.brightness;
            self.smoothing = p.smoothing;
            self.gamma = p.gamma;
            self.saturation_boost = p.saturation_boost;
            self.white_balance_r = p.white_balance_r;
            self.white_balance_g = p.white_balance_g;
            self.white_balance_b = p.white_balance_b;
            self.black_level_threshold = p.black_level_threshold;
            self.letterbox_detection = p.letterbox_detection;
            self.target_fps = p.target_fps;
            self.static_color = p.static_color;
            self.audio_effect = p.audio_effect;
            self.audio_sensitivity = p.audio_sensitivity;
            self.active_preset = Some(preset_id.to_string());
            self.bump_version();
            true
        } else {
            false
        }
    }

    /// Сохранение текущих настроек в новый или существующий пресет
    pub fn save_preset(&mut self, name: &str, existing_id: Option<&str>) -> String {
        let id = match existing_id {
            Some(eid) if self.presets.iter().any(|p| p.id == eid) => eid.to_string(),
            _ => {
                let clean_name: String = name
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect();
                let base = if clean_name.is_empty() { "preset" } else { &clean_name };
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                format!("{}_{}", base.to_lowercase(), ts % 100000)
            }
        };

        let new_preset = Preset {
            id: id.clone(),
            name: name.trim().to_string(),
            mode: self.mode.clone(),
            brightness: self.brightness,
            smoothing: self.smoothing,
            gamma: self.gamma,
            saturation_boost: self.saturation_boost,
            white_balance_r: self.white_balance_r,
            white_balance_g: self.white_balance_g,
            white_balance_b: self.white_balance_b,
            black_level_threshold: self.black_level_threshold,
            letterbox_detection: self.letterbox_detection,
            target_fps: self.target_fps,
            static_color: self.static_color,
            audio_effect: self.audio_effect.clone(),
            audio_sensitivity: self.audio_sensitivity,
        };

        if let Some(pos) = self.presets.iter().position(|p| p.id == id) {
            self.presets[pos] = new_preset;
        } else {
            self.presets.push(new_preset);
        }

        self.active_preset = Some(id.clone());
        self.bump_version();
        id
    }

    /// Удаление пресета по его id
    pub fn delete_preset(&mut self, preset_id: &str) -> bool {
        let initial_len = self.presets.len();
        self.presets.retain(|p| p.id != preset_id);
        let removed = self.presets.len() < initial_len;
        if removed {
            if self.active_preset.as_deref() == Some(preset_id) {
                self.active_preset = None;
            }
            self.bump_version();
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presets_lifecycle() {
        let mut cfg = AppConfig::default();
        assert!(cfg.presets.is_empty());
        assert_eq!(cfg.active_preset, None);

        // Настраиваем уникальные параметры
        cfg.brightness = 0.77;
        cfg.gamma = 1.9;
        cfg.black_level_threshold = 12;

        // Сохраняем в пресет
        let id = cfg.save_preset("Мой ночной пресет", None);
        assert_eq!(cfg.presets.len(), 1);
        assert_eq!(cfg.active_preset.as_deref(), Some(id.as_str()));
        assert_eq!(cfg.presets[0].name, "Мой ночной пресет");
        assert_eq!(cfg.presets[0].brightness, 0.77);
        assert_eq!(cfg.presets[0].black_level_threshold, 12);

        // Меняем текущие настройки
        cfg.brightness = 1.0;
        cfg.gamma = 2.2;
        cfg.black_level_threshold = 0;

        // Применяем сохраненный пресет обратно
        let applied = cfg.apply_preset(&id);
        assert!(applied);
        assert_eq!(cfg.brightness, 0.77);
        assert_eq!(cfg.gamma, 1.9);
        assert_eq!(cfg.black_level_threshold, 12);

        // Удаляем пресет
        let deleted = cfg.delete_preset(&id);
        assert!(deleted);
        assert!(cfg.presets.is_empty());
        assert_eq!(cfg.active_preset, None);
    }
}

pub mod wizard;

use crate::color::zone::Zone;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
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
    #[serde(default)]
    pub hotkeys: HotkeysConfig,
    pub zones: Vec<Zone>,
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

impl Default for AppConfig {
    fn default() -> Self {
        let led_count = 60;
        let zones = wizard::ZoneWizard::generate_perimeter(20, 20, 10, 10);
        Self {
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

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(path, json)
    }
}

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
}

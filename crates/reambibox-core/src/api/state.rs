use crate::device::types::RgbColor;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Глобальное разделяемое состояние приложения
#[derive(Debug, Clone)]
pub struct AppState {
    pub is_active: bool,
    pub current_profile: String,
    pub profiles: Vec<String>,
    pub led_count: usize,
    pub brightness: f32,
    pub smooth: f32,
    pub gamma: f32,
    pub manual_colors: Vec<RgbColor>,
}

impl Default for AppState {
    fn default() -> Self {
        let led_count = 60;
        Self {
            is_active: true,
            current_profile: "Default".to_string(),
            profiles: vec!["Default".to_string(), "Movie".to_string(), "Game".to_string()],
            led_count,
            brightness: 1.0,
            smooth: 0.5,
            gamma: 2.2,
            manual_colors: vec![RgbColor::BLACK; led_count],
        }
    }
}

pub type SharedAppState = Arc<RwLock<AppState>>;

pub fn create_shared_state() -> SharedAppState {
    Arc::new(RwLock::new(AppState::default()))
}


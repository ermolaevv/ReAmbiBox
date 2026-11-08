use super::state::SharedAppState;
use crate::device::types::RgbColor;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

pub struct LightpackServer {
    state: SharedAppState,
    bind_addr: SocketAddr,
}

impl LightpackServer {
    pub const DEFAULT_PORT: u16 = 3636;

    pub fn new(state: SharedAppState, port: u16) -> Self {
        let bind_addr = SocketAddr::from(([127, 0, 0, 1], port));
        Self { state, bind_addr }
    }

    pub async fn run(self) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("Lightpack API сервер запущен на {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let state = Arc::clone(&self.state);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, state).await {
                            warn!("Клиент {} отключился с ошибкой: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Ошибка accept соединения: {}", e);
                }
            }
        }
    }

    async fn handle_client(stream: TcpStream, state: SharedAppState) -> Result<(), std::io::Error> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            let cmd = line.trim();
            if cmd.is_empty() {
                continue;
            }

            let response = Self::process_command(cmd, &state).await;
            writer.write_all(response.as_bytes()).await?;
            writer.flush().await?;
        }

        Ok(())
    }

    pub async fn process_command(cmd: &str, state: &SharedAppState) -> String {
        let lower = cmd.to_ascii_lowercase();

        if lower == "getprofiles" {
            let s = state.read().await;
            return format!("profiles:{}\n", s.profiles.join(";"));
        }

        if lower == "getprofile" {
            let s = state.read().await;
            return format!("profile:{}\n", s.current_profile);
        }

        if lower == "getstatus" {
            let s = state.read().await;
            return format!("status:{}\n", if s.is_active { "on" } else { "off" });
        }

        if lower == "getcountleds" {
            let s = state.read().await;
            return format!("countleds:{}\n", s.led_count);
        }

        if lower == "getstatusapi" {
            return "statusapi:idle\n".to_string();
        }

        if lower.starts_with("apikey:") {
            return "ok\n".to_string();
        }

        if lower.starts_with("setstatus:") {
            let val = lower.strip_prefix("setstatus:").unwrap_or("").trim();
            let mut s = state.write().await;
            if val == "on" {
                s.is_active = true;
                return "ok\n".to_string();
            } else if val == "off" {
                s.is_active = false;
                return "ok\n".to_string();
            } else {
                return "error:invalid_status\n".to_string();
            }
        }

        if lower.starts_with("setprofile:") {
            let profile_name = cmd.strip_prefix("setprofile:").unwrap_or("").trim();
            let mut s = state.write().await;
            s.current_profile = profile_name.to_string();
            return "ok\n".to_string();
        }

        if lower.starts_with("setbrightness:") {
            if let Ok(b) = lower.strip_prefix("setbrightness:").unwrap().trim().parse::<u8>() {
                let mut s = state.write().await;
                s.brightness = (b as f32) / 255.0;
                return "ok\n".to_string();
            }
            return "error:invalid_brightness\n".to_string();
        }

        if lower.starts_with("setsmooth:") {
            if let Ok(sm) = lower.strip_prefix("setsmooth:").unwrap().trim().parse::<u8>() {
                let mut s = state.write().await;
                s.smooth = (sm as f32) / 255.0;
                return "ok\n".to_string();
            }
            return "error:invalid_smooth\n".to_string();
        }

        if lower.starts_with("setgamma:") {
            if let Ok(g) = lower.strip_prefix("setgamma:").unwrap().trim().parse::<f32>() {
                let mut s = state.write().await;
                s.gamma = g;
                return "ok\n".to_string();
            }
            return "error:invalid_gamma\n".to_string();
        }

        if lower.starts_with("setcolor:") {
            // Формат: setcolor:0-255,0,0;1-0,255,0\n
            let payload = lower.strip_prefix("setcolor:").unwrap().trim();
            let mut s = state.write().await;
            let parts = payload.split(';');

            for part in parts {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if let Some((idx_str, rgb_str)) = part.split_once('-') {
                    if let Ok(idx) = idx_str.trim().parse::<usize>() {
                        let channels: Vec<&str> = rgb_str.split(',').collect();
                        if channels.len() == 3 {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                channels[0].trim().parse::<u8>(),
                                channels[1].trim().parse::<u8>(),
                                channels[2].trim().parse::<u8>(),
                            ) {
                                if idx < s.manual_colors.len() {
                                    s.manual_colors[idx] = RgbColor::new(r, g, b);
                                }
                            }
                        }
                    }
                }
            }
            return "ok\n".to_string();
        }

        "error:unknown_command\n".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::state::create_shared_state;

    #[tokio::test]
    async fn test_lightpack_commands() {
        let state = create_shared_state();

        assert_eq!(
            LightpackServer::process_command("getprofiles", &state).await,
            "profiles:Default;Movie;Game\n"
        );

        assert_eq!(
            LightpackServer::process_command("getstatus", &state).await,
            "status:on\n"
        );

        assert_eq!(
            LightpackServer::process_command("setstatus:off", &state).await,
            "ok\n"
        );

        assert_eq!(
            LightpackServer::process_command("getstatus", &state).await,
            "status:off\n"
        );

        assert_eq!(
            LightpackServer::process_command("setcolor:0-255,128,64", &state).await,
            "ok\n"
        );

        let s = state.read().await;
        assert_eq!(s.manual_colors[0], RgbColor::new(255, 128, 64));
    }
}


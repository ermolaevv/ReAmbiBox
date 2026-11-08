use super::traits::{DeviceDriver, DeviceError};
use super::types::{DeviceFrame, RgbColor};
use serialport::SerialPort;
use std::io::Write;
use std::time::Duration;
use tracing::{error, info, warn};

/// Драйвер протокола Adalight для управления светодиодами через COM-порт
pub struct AdalightDriver {
    port_name: String,
    baud_rate: u32,
    color_order: String,
    last_led_count: usize,
    port: Option<Box<dyn SerialPort>>,
}

impl AdalightDriver {
    pub const DEFAULT_BAUD_RATE: u32 = 115200;
    pub const MAGIC_HEADER: [u8; 3] = [b'A', b'd', b'a'];

    pub fn new(port_name: impl Into<String>, baud_rate: u32) -> Self {
        Self {
            port_name: port_name.into(),
            baud_rate,
            color_order: "RGB".to_string(),
            last_led_count: 98,
            port: None,
        }
    }

    pub fn set_color_order(&mut self, order: impl Into<String>) {
        self.color_order = order.into();
    }

    pub fn set_led_count(&mut self, count: usize) {
        if count > 0 {
            self.last_led_count = count;
        }
    }

    /// Принудительное гашение всех светодиодов (отправка чёрного кадра)
    pub fn blank_leds(&mut self) -> Result<(), DeviceError> {
        let count = self.last_led_count.max(1);
        let black_packet = Self::build_packet_with_order(&vec![RgbColor::BLACK; count], &self.color_order)?;
        if let Some(port) = self.port.as_mut() {
            let _ = port.write_all(&black_packet);
            let _ = port.flush();
        }
        Ok(())
    }

    /// Аварийное гашение ленты при завершении процесса или панике
    pub fn emergency_blank(port_name: &str, baud_rate: u32, led_count: usize) {
        if let Ok(mut port) = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(200))
            .open()
        {
            if let Ok(packet) = Self::build_packet(&vec![RgbColor::BLACK; led_count.max(1)]) {
                let _ = port.write_all(&packet);
                let _ = port.flush();
            }
        }
    }

    /// Формирование полного бинарного пакета протокола Adalight с учетом порядка цветов
    pub fn build_packet_with_order(colors: &[RgbColor], order: &str) -> Result<Vec<u8>, DeviceError> {
        let count = colors.len();
        if count == 0 {
            return Err(DeviceError::InvalidLedCount(0));
        }
        if count > 65536 {
            return Err(DeviceError::InvalidLedCount(count));
        }

        let hi = (((count - 1) >> 8) & 0xFF) as u8;
        let lo = ((count - 1) & 0xFF) as u8;
        let checksum = hi ^ lo ^ 0x55;

        let mut packet = Vec::with_capacity(6 + count * 3);
        packet.extend_from_slice(&Self::MAGIC_HEADER);
        packet.push(hi);
        packet.push(lo);
        packet.push(checksum);

        let is_grb = order.eq_ignore_ascii_case("GRB");
        let is_bgr = order.eq_ignore_ascii_case("BGR");

        for color in colors {
            if is_grb {
                packet.push(color.g);
                packet.push(color.r);
                packet.push(color.b);
            } else if is_bgr {
                packet.push(color.b);
                packet.push(color.g);
                packet.push(color.r);
            } else {
                packet.push(color.r);
                packet.push(color.g);
                packet.push(color.b);
            }
        }

        Ok(packet)
    }

    /// Формирование стандартного RGB бинарного пакета протокола Adalight
    pub fn build_packet(colors: &[RgbColor]) -> Result<Vec<u8>, DeviceError> {
        Self::build_packet_with_order(colors, "RGB")
    }

    /// Получение списка всех доступных COM-портов в системе
    pub fn available_ports() -> Vec<String> {
        match serialport::available_ports() {
            Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
            Err(e) => {
                warn!("Не удалось получить список COM-портов: {}", e);
                Vec::new()
            }
        }
    }

    /// Попытка автоматического переподключения к порту
    pub fn ensure_connected(&mut self) -> Result<(), DeviceError> {
        if self.is_connected() {
            return Ok(());
        }
        self.connect()
    }

    /// Отправка кадра с автоматической попыткой переподключения
    pub fn send_frame_with_retry(&mut self, frame: &DeviceFrame) -> Result<(), DeviceError> {
        if !self.is_connected() {
            let _ = self.connect();
        }
        self.send_frame(frame)
    }
}

impl DeviceDriver for AdalightDriver {
    fn name(&self) -> &str {
        "Adalight"
    }

    fn connect(&mut self) -> Result<(), DeviceError> {
        if self.is_connected() {
            return Ok(());
        }

        info!("Подключение к Adalight на порту {} ({} бод)...", self.port_name, self.baud_rate);
        let mut port = serialport::new(&self.port_name, self.baud_rate)
            .timeout(Duration::from_millis(500))
            .open()?;

        // Очищаем входные и выходные буферы COM-порта от зависших байтов предыдущих сессий
        let _ = port.clear(serialport::ClearBuffer::All);

        // Аппаратный DTR сброс микроконтроллера Arduino (эквивалент физического переподключения USB)
        let _ = port.write_data_terminal_ready(false);
        std::thread::sleep(Duration::from_millis(50));
        let _ = port.write_data_terminal_ready(true);
        let _ = port.write_request_to_send(true);
        std::thread::sleep(Duration::from_millis(150));
        let _ = port.clear(serialport::ClearBuffer::All);

        self.port = Some(port);
        info!("Успешно подключено к Adalight на {}", self.port_name);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), DeviceError> {
        let _ = self.blank_leds();
        if let Some(mut port) = self.port.take() {
            let _ = port.flush();
            info!("Отключено от Adalight на {}", self.port_name);
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.port.is_some()
    }

    fn send_frame(&mut self, frame: &DeviceFrame) -> Result<(), DeviceError> {
        self.last_led_count = frame.colors.len();
        let packet = Self::build_packet_with_order(&frame.colors, &self.color_order)?;

        let port = match self.port.as_mut() {
            Some(p) => p,
            None => return Err(DeviceError::NotConnected),
        };

        if let Err(e) = port.write_all(&packet) {
            error!("Ошибка записи кадра в порт {}: {}", self.port_name, e);
            self.port = None; // Сбрасываем порт при обрыве связи для автопереподключения
            return Err(DeviceError::IoError(e));
        }

        let _ = port.flush();
        Ok(())
    }
}

impl Drop for AdalightDriver {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adalight_packet_builder() {
        let colors = vec![
            RgbColor::new(255, 0, 0),
            RgbColor::new(0, 255, 0),
            RgbColor::new(0, 0, 255),
        ];

        let packet = AdalightDriver::build_packet(&colors).unwrap();
        // 3 диода: count = 3, count - 1 = 2
        // hi = 0, lo = 2, crc = 0 ^ 2 ^ 0x55 = 0x57 (87)
        assert_eq!(&packet[0..3], b"Ada");
        assert_eq!(packet[3], 0);
        assert_eq!(packet[4], 2);
        assert_eq!(packet[5], 0 ^ 2 ^ 0x55);

        // Цвета
        assert_eq!(&packet[6..9], &[255, 0, 0]);
        assert_eq!(&packet[9..12], &[0, 255, 0]);
        assert_eq!(&packet[12..15], &[0, 0, 255]);
        assert_eq!(packet.len(), 15);
    }

    #[test]
    fn test_adalight_empty_fails() {
        let empty: Vec<RgbColor> = Vec::new();
        assert!(matches!(
            AdalightDriver::build_packet(&empty),
            Err(DeviceError::InvalidLedCount(0))
        ));
    }

    #[test]
    fn test_connect_to_nonexistent_port_fails_gracefully() {
        let mut driver = AdalightDriver::new("COM9999", 115200);
        assert!(!driver.is_connected());
        let res = driver.connect();
        assert!(res.is_err());
        assert!(!driver.is_connected());

        let frame = DeviceFrame::new(vec![RgbColor::new(255, 0, 0)]);
        let send_res = driver.send_frame(&frame);
        assert!(matches!(send_res, Err(DeviceError::NotConnected)));
    }
}

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
    port: Option<Box<dyn SerialPort>>,
}

impl AdalightDriver {
    pub const DEFAULT_BAUD_RATE: u32 = 115200;
    pub const MAGIC_HEADER: [u8; 3] = [b'A', b'd', b'a'];

    pub fn new(port_name: impl Into<String>, baud_rate: u32) -> Self {
        Self {
            port_name: port_name.into(),
            baud_rate,
            port: None,
        }
    }

    /// Формирование полного бинарного пакета протокола Adalight
    pub fn build_packet(colors: &[RgbColor]) -> Result<Vec<u8>, DeviceError> {
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

        for color in colors {
            packet.push(color.r);
            packet.push(color.g);
            packet.push(color.b);
        }

        Ok(packet)
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
        let port = serialport::new(&self.port_name, self.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()?;

        self.port = Some(port);
        info!("Успешно подключено к Adalight на {}", self.port_name);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), DeviceError> {
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
        let packet = Self::build_packet(&frame.colors)?;

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
}

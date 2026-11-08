use super::traits::{DeviceDriver, DeviceError};
use super::types::{DeviceFrame, RgbColor};
use serde::{Deserialize, Serialize};
use std::net::UdpSocket;
use std::time::Duration;
use tracing::{error, info};

/// Поддерживаемые сетевые протоколы WLED
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WledProtocol {
    /// Distributed Display Protocol (DDP) — современный, быстрый, порт 4048
    Ddp,
    /// WLED Realtime UDP (WARLS) — классический протокол WLED, порт 21324
    Warls,
}

impl Default for WledProtocol {
    fn default() -> Self {
        Self::Ddp
    }
}

impl WledProtocol {
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Ddp => 4048,
            Self::Warls => 21324,
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "warls" | "udp" => Self::Warls,
            _ => Self::Ddp,
        }
    }
}

/// Драйвер для управления контроллерами WLED (ESP8266/ESP32) по сети Wi-Fi/Ethernet
pub struct WledDriver {
    host: String,
    port: u16,
    protocol: WledProtocol,
    color_order: String,
    last_led_count: usize,
    sequence: u8,
    socket: Option<UdpSocket>,
    packet_buffer: Vec<u8>,
}

impl WledDriver {
    pub const DEFAULT_PORT: u16 = 4048;
    pub const MAX_LEDS_PER_DDP_PACKET: usize = 480; // 480 * 3 = 1440 байт полезной нагрузки (< 1500 MTU)

    pub fn new(host: impl Into<String>, port: u16, protocol: WledProtocol) -> Self {
        Self {
            host: host.into(),
            port,
            protocol,
            color_order: "RGB".to_string(),
            last_led_count: 98,
            sequence: 1,
            socket: None,
            packet_buffer: Vec::with_capacity(10 + 98 * 3),
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

    pub fn set_protocol(&mut self, protocol: WledProtocol) {
        self.protocol = protocol;
    }

    /// Преобразование порядка цветовых компонент
    #[inline]
    pub fn reorder_color(color: &RgbColor, order: &str) -> (u8, u8, u8) {
        match order.to_uppercase().as_str() {
            "GRB" => (color.g, color.r, color.b),
            "BGR" => (color.b, color.g, color.r),
            "BRG" => (color.b, color.r, color.g),
            "RBG" => (color.r, color.b, color.g),
            "GBR" => (color.g, color.b, color.r),
            _ => (color.r, color.g, color.b), // "RGB" по умолчанию
        }
    }

    /// Запись одиночного DDP пакета в переиспользуемый буфер без выделения памяти
    pub fn write_single_ddp_packet(
        colors: &[RgbColor],
        color_order: &str,
        sequence: u8,
        out: &mut Vec<u8>,
    ) -> Result<(), DeviceError> {
        let count = colors.len();
        if count == 0 {
            return Err(DeviceError::InvalidLedCount(0));
        }

        out.clear();
        let data_len = (count * 3) as u16;
        let flags: u8 = 0x40 | 0x01; // Push immediately

        out.reserve(10 + count * 3);
        out.push(flags);
        out.push(sequence & 0x0F);
        out.push(0x01); // RGB 24-bit
        out.push(0x01); // Destination ID: 1
        out.extend_from_slice(&0u32.to_be_bytes()); // offset = 0
        out.extend_from_slice(&data_len.to_be_bytes());

        for color in colors {
            let (c1, c2, c3) = Self::reorder_color(color, color_order);
            out.push(c1);
            out.push(c2);
            out.push(c3);
        }

        Ok(())
    }

    /// Формирование одного или нескольких DDP UDP пакетов
    /// Поддерживает автоматическое разбиение на чанки (по 480 светодиодов)
    pub fn build_ddp_packets(
        colors: &[RgbColor],
        color_order: &str,
        sequence: u8,
    ) -> Result<Vec<Vec<u8>>, DeviceError> {
        let count = colors.len();
        if count == 0 {
            return Err(DeviceError::InvalidLedCount(0));
        }

        let mut packets = Vec::new();
        let chunks: Vec<&[RgbColor]> = colors.chunks(Self::MAX_LEDS_PER_DDP_PACKET).collect();
        let total_chunks = chunks.len();

        for (chunk_idx, chunk) in chunks.into_iter().enumerate() {
            let is_last = chunk_idx == total_chunks - 1;
            // Флаги: 0x40 (версия 1) | 0x01 (push/display сразу, выставляем на последнем пакете кадра)
            let flags: u8 = if is_last { 0x40 | 0x01 } else { 0x40 };

            let led_offset = chunk_idx * Self::MAX_LEDS_PER_DDP_PACKET;
            let byte_offset = (led_offset * 3) as u32;
            let data_len = (chunk.len() * 3) as u16;

            // DDP заголовок = 10 байт
            let mut packet = Vec::with_capacity(10 + chunk.len() * 3);
            packet.push(flags);
            packet.push(sequence & 0x0F); // 4-битный sequence number
            packet.push(0x01);            // Data type: RGB 24-bit
            packet.push(0x01);            // Destination ID: 1
            // 4 байта смещения (Big-Endian)
            packet.extend_from_slice(&byte_offset.to_be_bytes());
            // 2 байта длины данных (Big-Endian)
            packet.extend_from_slice(&data_len.to_be_bytes());

            // Полезная нагрузка RGB
            for color in chunk {
                let (c1, c2, c3) = Self::reorder_color(color, color_order);
                packet.push(c1);
                packet.push(c2);
                packet.push(c3);
            }

            packets.push(packet);
        }

        Ok(packets)
    }

    /// Запись пакета по протоколу WLED WARLS в переиспользуемый буфер (до 255 диодов)
    pub fn write_warls_packet(
        colors: &[RgbColor],
        color_order: &str,
        timeout_seconds: u8,
        out: &mut Vec<u8>,
    ) -> Result<(), DeviceError> {
        let count = colors.len();
        if count == 0 {
            return Err(DeviceError::InvalidLedCount(0));
        }
        if count > 255 {
            return Err(DeviceError::ProtocolError(
                "Протокол WARLS поддерживает максимум 255 светодиодов, используйте DDP".to_string(),
            ));
        }

        out.clear();
        out.reserve(2 + count * 4);
        out.push(0x02);
        out.push(timeout_seconds.max(1));

        for (i, color) in colors.iter().enumerate() {
            let (c1, c2, c3) = Self::reorder_color(color, color_order);
            out.push(i as u8);
            out.push(c1);
            out.push(c2);
            out.push(c3);
        }

        Ok(())
    }

    /// Формирование UDP пакета по протоколу WLED WARLS (до 255 диодов)
    pub fn build_warls_packet(
        colors: &[RgbColor],
        color_order: &str,
        timeout_seconds: u8,
    ) -> Result<Vec<u8>, DeviceError> {
        let mut buf = Vec::new();
        Self::write_warls_packet(colors, color_order, timeout_seconds, &mut buf)?;
        Ok(buf)
    }

    /// Экстренное выключение подсветки WLED по сети (без необходимости сохранять драйвер)
    pub fn emergency_blank(host: &str, port: u16, led_count: usize) {
        let count = led_count.max(1);
        let black = vec![RgbColor::BLACK; count];
        if let Ok(packets) = Self::build_ddp_packets(&black, "RGB", 0) {
            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
                let _ = socket.set_write_timeout(Some(Duration::from_millis(200)));
                let target = format!("{}:{}", host, port);
                for pkt in packets {
                    let _ = socket.send_to(&pkt, &target);
                }
            }
        }
    }
}

impl DeviceDriver for WledDriver {
    fn name(&self) -> &str {
        match self.protocol {
            WledProtocol::Ddp => "WLED (DDP / UDP)",
            WledProtocol::Warls => "WLED (WARLS / UDP)",
        }
    }

    fn connect(&mut self) -> Result<(), DeviceError> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| DeviceError::IoError(e))?;

        socket
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| DeviceError::IoError(e))?;

        let remote_addr = format!("{}:{}", self.host, self.port);
        socket
            .connect(&remote_addr)
            .map_err(|e| DeviceError::IoError(e))?;

        info!(
            "Подключен сетевой вывод WLED на {}:{} (протокол: {:?})",
            self.host, self.port, self.protocol
        );
        self.socket = Some(socket);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), DeviceError> {
        if let Some(_socket) = self.socket.take() {
            info!("Сетевой вывод WLED {}:{} отключен", self.host, self.port);
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.socket.is_some()
    }

    fn send_frame(&mut self, frame: &DeviceFrame) -> Result<(), DeviceError> {
        let socket = self.socket.as_mut().ok_or(DeviceError::NotConnected)?;
        self.last_led_count = frame.colors.len();

        match self.protocol {
            WledProtocol::Ddp => {
                if frame.colors.is_empty() {
                    return Err(DeviceError::InvalidLedCount(0));
                }
                if frame.colors.len() <= Self::MAX_LEDS_PER_DDP_PACKET {
                    Self::write_single_ddp_packet(
                        &frame.colors,
                        &self.color_order,
                        self.sequence,
                        &mut self.packet_buffer,
                    )?;
                    socket.send(&self.packet_buffer).map_err(|e| {
                        error!("Ошибка отправки DDP пакета WLED: {}", e);
                        DeviceError::IoError(e)
                    })?;
                } else {
                    let packets = Self::build_ddp_packets(
                        &frame.colors,
                        &self.color_order,
                        self.sequence,
                    )?;

                    for packet in packets {
                        socket.send(&packet).map_err(|e| {
                            error!("Ошибка отправки DDP пакета WLED: {}", e);
                            DeviceError::IoError(e)
                        })?;
                    }
                }
                self.sequence = self.sequence.wrapping_add(1);
            }
            WledProtocol::Warls => {
                Self::write_warls_packet(
                    &frame.colors,
                    &self.color_order,
                    2, // 2 секунды удержания кадра в WLED
                    &mut self.packet_buffer,
                )?;

                socket.send(&self.packet_buffer).map_err(|e| {
                    error!("Ошибка отправки WARLS пакета WLED: {}", e);
                    DeviceError::IoError(e)
                })?;
            }
        }

        Ok(())
    }

    fn blank_leds(&mut self) -> Result<(), DeviceError> {
        let count = self.last_led_count.max(1);
        let black_frame = DeviceFrame::new(vec![RgbColor::BLACK; count]);
        self.send_frame(&black_frame)
    }

    fn set_color_order(&mut self, order: &str) {
        self.set_color_order(order);
    }

    fn set_led_count(&mut self, count: usize) {
        self.set_led_count(count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ddp_packet_structure_single() {
        let colors = vec![
            RgbColor::new(255, 0, 0),
            RgbColor::new(0, 255, 0),
            RgbColor::new(0, 0, 255),
        ];

        let packets = WledDriver::build_ddp_packets(&colors, "RGB", 5).unwrap();
        assert_eq!(packets.len(), 1);

        let pkt = &packets[0];
        assert_eq!(pkt.len(), 10 + 9); // 10 байт заголовок + 3 диода * 3

        // Flags: 0x40 (V1) | 0x01 (Push) = 0x41
        assert_eq!(pkt[0], 0x41);
        // Sequence: 5 & 0x0F = 5
        assert_eq!(pkt[1], 5);
        // Data type: 0x01 (RGB)
        assert_eq!(pkt[2], 0x01);
        // Destination: 0x01
        assert_eq!(pkt[3], 0x01);
        // Offset: 0 (4 байта)
        assert_eq!(&pkt[4..8], &[0, 0, 0, 0]);
        // Length: 9 байт (2 байта Big-Endian)
        assert_eq!(&pkt[8..10], &[0, 9]);

        // Цвета RGB:
        assert_eq!(&pkt[10..13], &[255, 0, 0]);
        assert_eq!(&pkt[13..16], &[0, 255, 0]);
        assert_eq!(&pkt[16..19], &[0, 0, 255]);
    }

    #[test]
    fn test_ddp_packet_color_order() {
        let colors = vec![RgbColor::new(10, 20, 30)];
        let packets = WledDriver::build_ddp_packets(&colors, "GRB", 1).unwrap();
        let pkt = &packets[0];
        // GRB порядок: Green (20), Red (10), Blue (30)
        assert_eq!(&pkt[10..13], &[20, 10, 30]);
    }

    #[test]
    fn test_ddp_multi_packet_fragmentation() {
        // Создадим 500 диодов (480 в первом пакете + 20 во втором)
        let colors = vec![RgbColor::new(1, 2, 3); 500];
        let packets = WledDriver::build_ddp_packets(&colors, "RGB", 2).unwrap();
        assert_eq!(packets.len(), 2);

        // Первый пакет: 480 диодов = 1440 байт данных + 10 байт заголовок
        let pkt1 = &packets[0];
        assert_eq!(pkt1.len(), 10 + 1440);
        assert_eq!(pkt1[0], 0x40); // Без флага Push (0x01)
        assert_eq!(&pkt1[4..8], &[0, 0, 0, 0]); // Смещение 0
        assert_eq!(&pkt1[8..10], &[0x05, 0xA0]); // 1440 байт (0x05A0)

        // Второй пакет: 20 диодов = 60 байт данных + 10 байт заголовок
        let pkt2 = &packets[1];
        assert_eq!(pkt2.len(), 10 + 60);
        assert_eq!(pkt2[0], 0x41); // С флагом Push (0x01)
        let _offset = 480 * 3; // 1440 = 0x000005A0
        assert_eq!(&pkt2[4..8], &[0, 0, 0x05, 0xA0]); // Смещение 1440
        assert_eq!(&pkt2[8..10], &[0, 60]); // 60 байт
    }

    #[test]
    fn test_warls_packet_structure() {
        let colors = vec![
            RgbColor::new(100, 150, 200),
            RgbColor::new(50, 60, 70),
        ];

        let pkt = WledDriver::build_warls_packet(&colors, "RGB", 2).unwrap();
        assert_eq!(pkt.len(), 2 + 2 * 4);
        assert_eq!(pkt[0], 0x02); // WARLS protocol ID
        assert_eq!(pkt[1], 0x02); // Timeout 2s

        // Диод 0: [index=0, 100, 150, 200]
        assert_eq!(&pkt[2..6], &[0, 100, 150, 200]);
        // Диод 1: [index=1, 50, 60, 70]
        assert_eq!(&pkt[6..10], &[1, 50, 60, 70]);
    }

    #[test]
    fn test_write_single_ddp_packet_matches_build() {
        let colors = vec![
            RgbColor::new(10, 20, 30),
            RgbColor::new(40, 50, 60),
            RgbColor::new(70, 80, 90),
        ];

        let built_packets = WledDriver::build_ddp_packets(&colors, "RGB", 3).unwrap();
        let mut buf = Vec::new();
        WledDriver::write_single_ddp_packet(&colors, "RGB", 3, &mut buf).unwrap();

        assert_eq!(built_packets.len(), 1);
        assert_eq!(&buf, &built_packets[0]);
    }
}

use super::types::DeviceFrame;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("Ошибка последовательного порта: {0}")]
    SerialError(#[from] serialport::Error),

    #[error("Устройство не подключено")]
    NotConnected,

    #[error("Ошибка ввода-вывода (IO): {0}")]
    IoError(#[from] std::io::Error),

    #[error("Недопустимое количество светодиодов: {0}")]
    InvalidLedCount(usize),

    #[error("Сбой протокола: {0}")]
    ProtocolError(String),
}

/// Абстрактный интерфейс драйвера светодиодного устройства
pub trait DeviceDriver: Send {
    /// Человекочитаемое имя драйвера
    fn name(&self) -> &str;

    /// Подключение к аппаратному устройству
    fn connect(&mut self) -> Result<(), DeviceError>;

    /// Отключение от аппаратного устройства
    fn disconnect(&mut self) -> Result<(), DeviceError>;

    /// Проверка активного соединения
    fn is_connected(&self) -> bool;

    /// Отправка очередного кадра цветов
    fn send_frame(&mut self, frame: &DeviceFrame) -> Result<(), DeviceError>;
}

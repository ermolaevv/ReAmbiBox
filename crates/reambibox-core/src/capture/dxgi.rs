use std::time::Duration;
use thiserror::Error;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
    IDXGIOutputDuplication, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
    DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SetThreadDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
};

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("Ошибка Windows API: {0}")]
    WindowsError(#[from] windows::core::Error),

    #[error("Сессия рабочего стола потеряна (UAC / Winlogon / сброс дисплея)")]
    AccessLost,

    #[error("Таймаут ожидания кадра (кадр не изменился)")]
    Timeout,

    #[error("Монитор с индексом {0} не найден")]
    MonitorNotFound(usize),

    #[error("Недопустимый формат кадра или пустая текстура")]
    InvalidFrame,
}

/// Захваченный кадр экрана с метаданными (владеющий буфером)
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    /// Буфер пикселей в формате BGRA
    pub data: Vec<u8>,
}

impl CapturedFrame {
    pub fn as_view(&self) -> FrameView<'_> {
        FrameView {
            width: self.width,
            height: self.height,
            row_pitch: self.row_pitch,
            data: &self.data,
        }
    }
}

/// Легковесное zero-allocation представление захваченного кадра (заимствующее память)
#[derive(Debug, Clone, Copy)]
pub struct FrameView<'a> {
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub data: &'a [u8],
}

/// Трейт для полиморфной выборки пикселей из владеющего или заимствованного кадра
pub trait AsFrameView {
    fn as_frame_view(&self) -> FrameView<'_>;
}

impl<'a> AsFrameView for FrameView<'a> {
    #[inline]
    fn as_frame_view(&self) -> FrameView<'_> {
        *self
    }
}

impl AsFrameView for CapturedFrame {
    #[inline]
    fn as_frame_view(&self) -> FrameView<'_> {
        self.as_view()
    }
}

/// Информация об обнаруженном мониторе
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub adapter_name: String,
    pub output_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Диспетчер захвата экрана через DXGI Desktop Duplication API
pub struct DxgiCapture {
    monitor_index: usize,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
}

impl DxgiCapture {
    /// Получение списка всех физических мониторов на всех видеоадаптерах
    pub fn list_monitors() -> Vec<MonitorInfo> {
        let mut list = Vec::new();
        unsafe {
            let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
                Ok(f) => f,
                Err(_) => return list,
            };

            let mut monitor_idx = 0;
            let mut adapter_idx = 0;

            while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                let adapter_desc = match adapter.GetDesc1() {
                    Ok(d) => d,
                    Err(_) => {
                        adapter_idx += 1;
                        continue;
                    }
                };

                let adapter_name = String::from_utf16_lossy(&adapter_desc.Description)
                    .trim_matches(char::from(0))
                    .to_string();

                let mut output_idx = 0;
                while let Ok(output) = adapter.EnumOutputs(output_idx) {
                    if let Ok(desc) = output.GetDesc() {
                        let output_name = String::from_utf16_lossy(&desc.DeviceName)
                            .trim_matches(char::from(0))
                            .to_string();

                        // Считываем физическое аппаратное разрешение монитора из Windows (без искажений DPI-масштабирования)
                        let mut devmode = DEVMODEW::default();
                        devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
                        let (width, height) = if EnumDisplaySettingsW(
                            PCWSTR(desc.DeviceName.as_ptr()),
                            ENUM_CURRENT_SETTINGS,
                            &mut devmode,
                        ).as_bool() {
                            (devmode.dmPelsWidth, devmode.dmPelsHeight)
                        } else {
                            (
                                (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32,
                                (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32,
                            )
                        };

                        let is_primary = desc.DesktopCoordinates.left == 0 && desc.DesktopCoordinates.top == 0;

                        list.push(MonitorInfo {
                            index: monitor_idx,
                            adapter_name: adapter_name.clone(),
                            output_name,
                            x: desc.DesktopCoordinates.left,
                            y: desc.DesktopCoordinates.top,
                            width,
                            height,
                            is_primary,
                        });
                        monitor_idx += 1;
                    }
                    output_idx += 1;
                }
                adapter_idx += 1;
            }
        }
        list
    }

    /// Привязка текущего потока к интерактивному рабочему столу
    pub fn attach_to_input_desktop() {
        unsafe {
            // Запрашиваем стандартные права доступа к рабочему столу (0x01FF),
            // не требующие административных привилегий, либо GENERIC_ALL
            let desktop = OpenInputDesktop(
                DESKTOP_CONTROL_FLAGS(0),
                false,
                DESKTOP_ACCESS_FLAGS(0x01FF),
            ).or_else(|_| {
                OpenInputDesktop(
                    DESKTOP_CONTROL_FLAGS(0),
                    false,
                    DESKTOP_ACCESS_FLAGS(0x10000000),
                )
            });

            if let Ok(desktop) = desktop {
                let _ = SetThreadDesktop(desktop);
                let _ = CloseDesktop(desktop);
            }
        }
    }

    /// Инициализация захвата для указанного индекса монитора
    pub fn new(target_monitor: usize) -> Result<Self, CaptureError> {
        Self::attach_to_input_desktop();
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let mut current_idx = 0;
            let mut adapter_idx = 0;
            let mut total_monitors = 0;

            while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                let mut output_idx = 0;
                while let Ok(output) = adapter.EnumOutputs(output_idx) {
                    if current_idx == target_monitor {
                        return Self::create_for_output(&adapter, &output, target_monitor);
                    }
                    current_idx += 1;
                    output_idx += 1;
                    total_monitors += 1;
                }
                adapter_idx += 1;
            }

            // Fallback: если запрошенный монитор временно не найден при загрузке системы,
            // но есть монитор 0, переключаемся на него
            if total_monitors > 0 && target_monitor > 0 {
                return Self::new(0);
            }

            Err(CaptureError::MonitorNotFound(target_monitor))
        }
    }

    fn create_for_output(
        adapter: &IDXGIAdapter1,
        output: &IDXGIOutput,
        monitor_index: usize,
    ) -> Result<Self, CaptureError> {
        unsafe {
            // Создание D3D11 устройства на ТОМ ЖЕ адаптере, к которому подключен монитор!
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let feature_levels = [D3D_FEATURE_LEVEL_11_0];

            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                7, // D3D11_SDK_VERSION
                Some(&mut device),
                None,
                Some(&mut context),
            )?;

            let device = device.ok_or(CaptureError::InvalidFrame)?;
            let context = context.ok_or(CaptureError::InvalidFrame)?;

            let output1: IDXGIOutput1 = output.cast()?;
            let duplication = output1.DuplicateOutput(&device)?;

            let desc = output.GetDesc()?;
            let dupl_desc = duplication.GetDesc();
            let (width, height) = if dupl_desc.ModeDesc.Width > 0 && dupl_desc.ModeDesc.Height > 0 {
                (dupl_desc.ModeDesc.Width, dupl_desc.ModeDesc.Height)
            } else {
                let mut devmode = DEVMODEW::default();
                devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
                if EnumDisplaySettingsW(
                    PCWSTR(desc.DeviceName.as_ptr()),
                    ENUM_CURRENT_SETTINGS,
                    &mut devmode,
                ).as_bool() {
                    (devmode.dmPelsWidth, devmode.dmPelsHeight)
                } else {
                    (
                        (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32,
                        (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32,
                    )
                }
            };

            Ok(Self {
                monitor_index,
                device,
                context,
                duplication,
                staging_texture: None,
                width,
                height,
            })
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn ensure_staging_texture(&mut self, width: u32, height: u32) -> Result<ID3D11Texture2D, CaptureError> {
        if self.staging_texture.is_none() || self.width != width || self.height != height {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut texture: Option<ID3D11Texture2D> = None;
            unsafe {
                self.device.CreateTexture2D(&desc, None, Some(&mut texture))?;
            }
            self.width = width;
            self.height = height;
            self.staging_texture = texture;
        }

        Ok(self.staging_texture.as_ref().unwrap().clone())
    }

    /// Zero-Allocation захват кадра с прямой передачей слайса отображенной видеопамяти
    pub fn capture_frame_with<R>(
        &mut self,
        timeout: Duration,
        f: impl FnOnce(&FrameView<'_>) -> R,
    ) -> Result<R, CaptureError> {
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;

        let acquire_res = unsafe {
            self.duplication
                .AcquireNextFrame(timeout.as_millis() as u32, &mut frame_info, &mut resource)
        };

        if let Err(e) = acquire_res {
            if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Err(CaptureError::Timeout);
            }
            if e.code() == DXGI_ERROR_ACCESS_LOST {
                return Err(CaptureError::AccessLost);
            }
            return Err(CaptureError::WindowsError(e));
        }

        let resource = resource.ok_or(CaptureError::InvalidFrame)?;
        let desktop_texture: ID3D11Texture2D = resource.cast()?;

        let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            desktop_texture.GetDesc(&mut texture_desc);
        }

        let width = texture_desc.Width;
        let height = texture_desc.Height;

        let staging = self.ensure_staging_texture(width, height)?;

        unsafe {
            self.context.CopyResource(&staging, &desktop_texture);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;

            let row_pitch = mapped.RowPitch;
            let total_bytes = (row_pitch * height) as usize;
            let slice = std::slice::from_raw_parts(mapped.pData as *const u8, total_bytes);

            let view = FrameView {
                width,
                height,
                row_pitch,
                data: slice,
            };

            let result = f(&view);

            self.context.Unmap(&staging, 0);
            let _ = self.duplication.ReleaseFrame();

            Ok(result)
        }
    }

    /// Захват кадра с копированием данных во владеющий буфер (для совместимости)
    pub fn capture_frame(&mut self, timeout: Duration) -> Result<CapturedFrame, CaptureError> {
        self.capture_frame_with(timeout, |view| CapturedFrame {
            width: view.width,
            height: view.height,
            row_pitch: view.row_pitch,
            data: view.data.to_vec(),
        })
    }

    pub fn reconnect(&mut self) -> Result<(), CaptureError> {
        self.staging_texture = None;
        let reloaded = Self::new(self.monitor_index)?;
        *self = reloaded;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_monitors() {
        let monitors = DxgiCapture::list_monitors();
        println!("\n=== ОБНАРУЖЕННЫЕ МОНИТОРЫ (всего: {}) ===", monitors.len());
        for m in &monitors {
            println!(
                "  [{}] {} | {} | pos: ({}, {}) | {}x{} | Primary: {}",
                m.index, m.adapter_name, m.output_name, m.x, m.y, m.width, m.height, m.is_primary
            );
        }
    }

    #[test]
    fn test_frame_view_conversion() {
        let dummy = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let frame = CapturedFrame {
            width: 2,
            height: 1,
            row_pitch: 8,
            data: dummy,
        };

        let view = frame.as_view();
        assert_eq!(view.width, 2);
        assert_eq!(view.height, 1);
        assert_eq!(view.row_pitch, 8);
        assert_eq!(view.data.len(), 8);

        let view_from_trait = frame.as_frame_view();
        assert_eq!(view_from_trait.width, 2);
    }
}

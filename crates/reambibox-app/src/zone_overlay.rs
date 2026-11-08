use parking_lot::RwLock;
use reambibox_core::config::AppConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
    InvalidateRect, Rectangle, ScreenToClient, SelectObject, SetBkMode, SetTextColor,
    DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD,
    PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
    LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SetCursor, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, HTCLIENT, HWND_TOPMOST, IDC_ARROW,
    IDC_SIZEALL, IDC_SIZENWSE, LWA_ALPHA, LWA_COLORKEY, MSG, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, SW_SHOW, WM_CREATE, WM_DESTROY, WM_ERASEBKGND,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_SETCURSOR,
    WM_USER, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const WM_TOGGLE_OVERLAY: u32 = WM_USER + 201;

struct OverlayState {
    config: Arc<RwLock<AppConfig>>,
    is_visible: Arc<AtomicBool>,
    is_dragging: bool,
    is_resizing: bool,
    active_zone_id: Option<usize>,
    drag_offset_x: f32,
    drag_offset_y: f32,
    #[allow(dead_code)]
    hwnd: HWND,
    screen_width: i32,
    screen_height: i32,
}

unsafe impl Send for OverlayState {}
unsafe impl Sync for OverlayState {}

static GLOBAL_OVERLAY: parking_lot::Mutex<Option<OverlayState>> = parking_lot::Mutex::new(None);

pub struct ZoneOverlayManager {
    is_visible: Arc<AtomicBool>,
    hwnd: parking_lot::Mutex<Option<HWND>>,
}

unsafe impl Send for ZoneOverlayManager {}
unsafe impl Sync for ZoneOverlayManager {}

impl ZoneOverlayManager {
    pub fn new() -> Self {
        Self {
            is_visible: Arc::new(AtomicBool::new(false)),
            hwnd: parking_lot::Mutex::new(None),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible.load(Ordering::Relaxed)
    }

    pub fn toggle(&self) {
        if let Some(hwnd) = *self.hwnd.lock() {
            unsafe {
                let _ = PostMessageW(hwnd, WM_TOGGLE_OVERLAY, WPARAM(0), LPARAM(0));
            }
        }
    }

    #[allow(dead_code)]
    pub fn show(&self) {
        if !self.is_visible() {
            self.toggle();
        }
    }

    #[allow(dead_code)]
    pub fn hide(&self) {
        if self.is_visible() {
            self.toggle();
        }
    }

    pub fn run_overlay_thread(&self, config: Arc<RwLock<AppConfig>>) {
        let is_visible_ref = Arc::clone(&self.is_visible);
        let (tx, rx) = std::sync::mpsc::channel::<Option<usize>>();

        std::thread::spawn(move || unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = w!("ReAmbiBoxZoneOverlayClass");

            let wnd_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(overlay_wnd_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };

            RegisterClassW(&wnd_class);

            // Получаем размеры экрана
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                class_name,
                w!("ReAmbiBoxZoneOverlay"),
                WS_POPUP,
                0,
                0,
                screen_w,
                screen_h,
                HWND(std::ptr::null_mut()),
                windows::Win32::UI::WindowsAndMessaging::HMENU(std::ptr::null_mut()),
                HINSTANCE(instance.0),
                None,
            ).unwrap_or_default();

            if hwnd.0.is_null() {
                eprintln!("[Overlay] Не удалось создать окно экранного оверлея зон");
                let _ = tx.send(None);
                return;
            }

            // Устанавливаем ключевой цвет прозрачности (черный RGB(0,0,0) полностью прозрачен)
            // И глобальную прозрачность альфа 235 (для видимости зон поверх окон)
            let _ = SetLayeredWindowAttributes(
                hwnd,
                COLORREF(0x00000000), // Черный цвет = прозрачный фон
                235,
                LWA_COLORKEY | LWA_ALPHA,
            );

            *GLOBAL_OVERLAY.lock() = Some(OverlayState {
                config,
                is_visible: is_visible_ref,
                is_dragging: false,
                is_resizing: false,
                active_zone_id: None,
                drag_offset_x: 0.0,
                drag_offset_y: 0.0,
                hwnd,
                screen_width: screen_w,
                screen_height: screen_h,
            });

            let _ = tx.send(Some(hwnd.0 as usize));

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });

        if let Ok(Some(raw)) = rx.recv() {
            *self.hwnd.lock() = Some(HWND(raw as *mut _));
        }
    }
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => LRESULT(0),

        WM_TOGGLE_OVERLAY => {
            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                let current = state.is_visible.load(Ordering::Relaxed);
                let next = !current;
                state.is_visible.store(next, Ordering::Relaxed);

                if next {
                    // Вычисляем физические координаты и разрешение выбранного монитора
                    let target_mon_idx = state.config.read().selected_monitor;
                    let monitors = reambibox_core::capture::dxgi::DxgiCapture::list_monitors();
                    let (mon_x, mon_y, mon_w, mon_h) = if let Some(m) = monitors.iter().find(|m| m.index == target_mon_idx) {
                        (m.x, m.y, m.width as i32, m.height as i32)
                    } else if let Some(m) = monitors.first() {
                        (m.x, m.y, m.width as i32, m.height as i32)
                    } else {
                        (0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
                    };

                    state.screen_width = mon_w;
                    state.screen_height = mon_h;

                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        mon_x,
                        mon_y,
                        mon_w,
                        mon_h,
                        windows::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW,
                    );
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
                    let _ = InvalidateRect(hwnd, None, true);
                    println!(
                        "[Overlay] Нативный экранный оверлей зон: АКТИВЕН на мониторе {} ({}x{} в {},{})",
                        target_mon_idx, mon_w, mon_h, mon_x, mon_y
                    );
                } else {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    println!("[Overlay] Нативный экранный оверлей зон: СКРЫТ (зоны сохранены)");
                }
            }
            LRESULT(0)
        }

        WM_ERASEBKGND => {
            // При двойной буферизации фон очищается в памяти, предотвращая мерцание
            LRESULT(1)
        }

        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Some(ref state) = *GLOBAL_OVERLAY.lock() {
                let screen_w = state.screen_width;
                let screen_h = state.screen_height;
                let (zones, sw_f32, sh_f32) = {
                    let cfg = state.config.read();
                    (cfg.zones.clone(), screen_w as f32, screen_h as f32)
                };

                // Двойная буферизация в памяти для исключения шлейфов и мерцания
                let mem_dc = CreateCompatibleDC(hdc);
                let mem_bmp = CreateCompatibleBitmap(hdc, screen_w, screen_h);
                let old_bmp = SelectObject(mem_dc, mem_bmp);

                // Очищаем весь буфер цветом прозрачности RGB(0,0,0)
                let rc_full = RECT {
                    left: 0,
                    top: 0,
                    right: screen_w,
                    bottom: screen_h,
                };
                let bg_brush = CreateSolidBrush(COLORREF(0x00000000));
                FillRect(mem_dc, &rc_full, bg_brush);
                let _ = DeleteObject(bg_brush);

                // Создаем шрифты для отображения номеров зон и плашки
                let h_font = CreateFontW(
                    14, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                );
                let old_font = SelectObject(mem_dc, h_font);
                SetBkMode(mem_dc, TRANSPARENT);

                // Отрисовка плашки подсказки вверху по центру
                {
                    let banner_w = 640;
                    let banner_h = 36;
                    let banner_x = (screen_w - banner_w) / 2;
                    let banner_y = 16;
                    let rc_banner = RECT {
                        left: banner_x,
                        top: banner_y,
                        right: banner_x + banner_w,
                        bottom: banner_y + banner_h,
                    };
                    let brush = CreateSolidBrush(COLORREF(0x00151210)); // Темно-серый
                    FillRect(mem_dc, &rc_banner, brush);
                    let _ = DeleteObject(brush);

                    let pen = CreatePen(PS_SOLID, 1, COLORREF(0x00EB8030)); // Оранжевая рамка
                    let old_pen = SelectObject(mem_dc, pen);
                    let _ = Rectangle(mem_dc, banner_x, banner_y, banner_x + banner_w, banner_y + banner_h);
                    SelectObject(mem_dc, old_pen);
                    let _ = DeleteObject(pen);

                    SetTextColor(mem_dc, COLORREF(0x00FFFFFF));
                    let mut text: Vec<u16> = "ReAmbiBox — Настройка зон | Перетаскивайте мышкой | Esc: Сохранить и закрыть\0"
                        .encode_utf16()
                        .collect();
                    let mut rc_text = rc_banner;
                    DrawTextW(mem_dc, &mut text, &mut rc_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                }

                // Отрисовка всех зон экрана в буфер памяти
                for zone in &zones {
                    let is_active = state.active_zone_id == Some(zone.id);

                    let zx = (zone.rect.x * sw_f32).round() as i32;
                    let zy = (zone.rect.y * sh_f32).round() as i32;
                    let zw = (zone.rect.width * sw_f32).round().max(16.0) as i32;
                    let zh = (zone.rect.height * sh_f32).round().max(16.0) as i32;

                    let rc_zone = RECT {
                        left: zx,
                        top: zy,
                        right: zx + zw,
                        bottom: zy + zh,
                    };

                    // Полупрозрачный цвет заливки зоны (синий/пурпурный для обычных, неоновый циан для активной)
                    let fill_color = if is_active {
                        COLORREF(0x004A3B10) // Подсвеченный сине-зеленый
                    } else {
                        COLORREF(0x003A2018) // Полупрозрачный темно-синий
                    };
                    let brush = CreateSolidBrush(fill_color);
                    FillRect(mem_dc, &rc_zone, brush);
                    let _ = DeleteObject(brush);

                    // Рамка зоны (активная: толстая ярко-голубая, обычные: розовая/фиолетовая)
                    let border_color = if is_active {
                        COLORREF(0x00FFD700) // Яркий циан (BGR: 00FFD700)
                    } else {
                        COLORREF(0x00C832D0) // Фиолетово-розовая
                    };
                    let pen = CreatePen(PS_SOLID, if is_active { 2 } else { 1 }, border_color);
                    let old_pen = SelectObject(mem_dc, pen);

                    let old_brush = SelectObject(mem_dc, windows::Win32::Graphics::Gdi::GetStockObject(windows::Win32::Graphics::Gdi::NULL_BRUSH));
                    let _ = Rectangle(mem_dc, zx, zy, zx + zw, zy + zh);
                    SelectObject(mem_dc, old_brush);
                    SelectObject(mem_dc, old_pen);
                    let _ = DeleteObject(pen);

                    // Маркер ресайза в правом нижнем углу
                    if is_active {
                        let handle_size = 6;
                        let handle_brush = CreateSolidBrush(COLORREF(0x0000FFFF));
                        let rc_handle = RECT {
                            left: zx + zw - handle_size,
                            top: zy + zh - handle_size,
                            right: zx + zw,
                            bottom: zy + zh,
                        };
                        FillRect(mem_dc, &rc_handle, handle_brush);
                        let _ = DeleteObject(handle_brush);
                    }

                    // Номер зоны по центру с контрастной обводкой
                    let mut num_text: Vec<u16> = format!("{}\0", zone.id + 1).encode_utf16().collect();
                    SetTextColor(mem_dc, COLORREF(0x00000000)); // Черная тень/обводка
                    for (dx, dy) in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let mut rc_shadow = RECT {
                            left: rc_zone.left + dx,
                            top: rc_zone.top + dy,
                            right: rc_zone.right + dx,
                            bottom: rc_zone.bottom + dy,
                        };
                        let mut num_text_shadow = num_text.clone();
                        DrawTextW(mem_dc, &mut num_text_shadow, &mut rc_shadow, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                    }

                    SetTextColor(mem_dc, if is_active { COLORREF(0x0000FFFF) } else { COLORREF(0x00FFFFFF) });
                    let mut rc_num = rc_zone;
                    DrawTextW(mem_dc, &mut num_text, &mut rc_num, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                }

                SelectObject(mem_dc, old_font);
                let _ = DeleteObject(h_font);

                // Мгновенный аппаратный перенос кадра на экран (без шлейфов и артефактов)
                let _ = BitBlt(hdc, 0, 0, screen_w, screen_h, mem_dc, 0, 0, SRCCOPY);

                SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(mem_bmp);
                let _ = DeleteDC(mem_dc);
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let mouse_x = (lparam.0 & 0xFFFF) as i16 as f32;
            let mouse_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;

            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                let screen_w = state.screen_width as f32;
                let screen_h = state.screen_height as f32;

                let cfg = state.config.read();
                let mut clicked_zone = None;
                let mut is_resize = false;

                // Ищем зону под курсором (с конца к началу, чтобы верхние брались первыми)
                for zone in cfg.zones.iter().rev() {
                    let zx = zone.rect.x * screen_w;
                    let zy = zone.rect.y * screen_h;
                    let zw = zone.rect.width * screen_w;
                    let zh = zone.rect.height * screen_h;

                    if mouse_x >= zx && mouse_x <= zx + zw && mouse_y >= zy && mouse_y <= zy + zh {
                        clicked_zone = Some(zone.id);
                        // Проверяем попадание в правый нижний угол (область 14x14px для ресайза)
                        if mouse_x >= zx + zw - 14.0 && mouse_y >= zy + zh - 14.0 {
                            is_resize = true;
                        }
                        state.drag_offset_x = (mouse_x - zx) / screen_w;
                        state.drag_offset_y = (mouse_y - zy) / screen_h;
                        break;
                    }
                }

                if let Some(id) = clicked_zone {
                    state.active_zone_id = Some(id);
                    if is_resize {
                        state.is_resizing = true;
                    } else {
                        state.is_dragging = true;
                    }
                    SetCapture(hwnd);
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let mouse_x = (lparam.0 & 0xFFFF) as i16 as f32;
            let mouse_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;

            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                if (state.is_dragging || state.is_resizing) && state.active_zone_id.is_some() {
                    let zone_id = state.active_zone_id.unwrap();
                    let screen_w = state.screen_width as f32;
                    let screen_h = state.screen_height as f32;

                    let mut cfg = state.config.write();
                    if let Some(zone) = cfg.zones.iter_mut().find(|z| z.id == zone_id) {
                        if state.is_dragging {
                            // Перемещение зоны
                            let new_x = ((mouse_x / screen_w) - state.drag_offset_x).clamp(0.0, 1.0 - zone.rect.width);
                            let new_y = ((mouse_y / screen_h) - state.drag_offset_y).clamp(0.0, 1.0 - zone.rect.height);
                            zone.rect.x = new_x;
                            zone.rect.y = new_y;
                        } else if state.is_resizing {
                            // Изменение размера
                            let zx = zone.rect.x * screen_w;
                            let zy = zone.rect.y * screen_h;
                            let new_w = ((mouse_x - zx) / screen_w).clamp(0.01, 1.0 - zone.rect.x);
                            let new_h = ((mouse_y - zy) / screen_h).clamp(0.01, 1.0 - zone.rect.y);
                            zone.rect.width = new_w;
                            zone.rect.height = new_h;
                        }
                    }

                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                if state.is_dragging || state.is_resizing {
                    state.is_dragging = false;
                    state.is_resizing = false;
                    // Сохраняем перемещенные зоны в config.json
                    let _ = state.config.read().save();
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }

        WM_SETCURSOR => {
            let hit_test = (lparam.0 & 0xFFFF) as u32;
            if hit_test == HTCLIENT {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                let _ = ScreenToClient(hwnd, &mut pt);

                let mut cursor = IDC_ARROW;
                if let Some(ref state) = *GLOBAL_OVERLAY.lock() {
                    let screen_w = state.screen_width as f32;
                    let screen_h = state.screen_height as f32;
                    let px = pt.x as f32;
                    let py = pt.y as f32;

                    let cfg = state.config.read();
                    for zone in cfg.zones.iter().rev() {
                        let zx = zone.rect.x * screen_w;
                        let zy = zone.rect.y * screen_h;
                        let zw = zone.rect.width * screen_w;
                        let zh = zone.rect.height * screen_h;

                        if px >= zx && px <= zx + zw && py >= zy && py <= zy + zh {
                            if px >= zx + zw - 14.0 && py >= zy + zh - 14.0 {
                                cursor = IDC_SIZENWSE;
                            } else {
                                cursor = IDC_SIZEALL;
                            }
                            break;
                        }
                    }
                }

                let _ = SetCursor(LoadCursorW(None, cursor).unwrap_or_default());
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
                // Закрытие оверлея по клавише Esc
                if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                    state.is_visible.store(false, Ordering::Relaxed);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let _ = state.config.read().save();
                    println!("[Overlay] Закрытие оверлея по клавише Esc (настройки сохранены)");
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

use parking_lot::RwLock;
use reambibox_core::config::AppConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
    InvalidateRect, Rectangle, ScreenToClient, SelectObject, SetBkMode, SetTextColor, UpdateWindow,
    DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD,
    HBITMAP, HDC, HGDIOBJ, PAINTSTRUCT, PS_SOLID,
    SRCCOPY, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetCursorPos, GetMessageW,
    GetSystemMetrics, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SetCursor,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, HTCLIENT,
    HWND_TOPMOST, IDC_ARROW, IDC_SIZENWSE, LWA_ALPHA, LWA_COLORKEY, MSG, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, SW_SHOW, WM_CAPTURECHANGED, WM_CREATE, WM_DESTROY, WM_ERASEBKGND,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT,
    WM_RBUTTONUP, WM_SETCURSOR, WM_USER, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

const WM_TOGGLE_OVERLAY: u32 = WM_USER + 201;
// Ключевой цвет прозрачности DWM — чистый Magenta RGB(255, 0, 255)
// Устраняет артефакты, шлейфы и ложные прозрачные дыры от теней текста
const OVERLAY_COLOR_KEY: COLORREF = COLORREF(0x00FF00FF);

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
    mem_dc: Option<HDC>,
    mem_bmp: Option<HBITMAP>,
    old_bmp: Option<HGDIOBJ>,
    buf_w: i32,
    buf_h: i32,
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
            // Включаем Per-Monitor DPI Awareness v2 для потока оверлея
            let _ = windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
                windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            );

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

            // Устанавливаем ключевой цвет прозрачности Magenta и общую альфу для плавного наложения зон
            let _ = SetLayeredWindowAttributes(
                hwnd,
                OVERLAY_COLOR_KEY,
                225,
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
                mem_dc: None,
                mem_bmp: None,
                old_bmp: None,
                buf_w: 0,
                buf_h: 0,
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
            let hdc = HDC(wparam.0 as *mut _);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let bg_brush = CreateSolidBrush(OVERLAY_COLOR_KEY);
            FillRect(hdc, &rc, bg_brush);
            let _ = DeleteObject(bg_brush);
            LRESULT(1)
        }

        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                let screen_w = state.screen_width;
                let screen_h = state.screen_height;
                let (zones, sw_f32, sh_f32) = {
                    let cfg = state.config.read();
                    (cfg.zones.clone(), screen_w as f32, screen_h as f32)
                };

                // Выделяем или переиспользуем буфер двойной буферизации
                if state.mem_dc.is_none() || state.buf_w != screen_w || state.buf_h != screen_h {
                    if let Some(old_dc) = state.mem_dc {
                        if let Some(old_b) = state.old_bmp {
                            SelectObject(old_dc, old_b);
                        }
                        if let Some(bmp) = state.mem_bmp {
                            let _ = DeleteObject(bmp);
                        }
                        let _ = DeleteDC(old_dc);
                    }

                    let new_dc = CreateCompatibleDC(hdc);
                    let new_bmp = CreateCompatibleBitmap(hdc, screen_w, screen_h);
                    let prev_bmp = SelectObject(new_dc, new_bmp);

                    state.mem_dc = Some(new_dc);
                    state.mem_bmp = Some(new_bmp);
                    state.old_bmp = Some(prev_bmp);
                    state.buf_w = screen_w;
                    state.buf_h = screen_h;
                }

                if let Some(mem_dc) = state.mem_dc {
                    // 1. Очищаем весь буфер прозрачным ключевым цветом Magenta
                    let rc_full = RECT {
                        left: 0,
                        top: 0,
                        right: screen_w,
                        bottom: screen_h,
                    };
                    let bg_brush = CreateSolidBrush(OVERLAY_COLOR_KEY);
                    FillRect(mem_dc, &rc_full, bg_brush);
                    let _ = DeleteObject(bg_brush);

                    // 2. Создаем шрифты
                    let h_font = CreateFontW(
                        14, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
                    );
                    let old_font = SelectObject(mem_dc, h_font);
                    SetBkMode(mem_dc, TRANSPARENT);

                    // 3. Плашка подсказки (размещаем ниже верхних зон периметра, чтобы не перекрывать их)
                    {
                        let banner_w = 720;
                        let banner_h = 40;
                        let banner_x = (screen_w - banner_w) / 2;
                        // Смещаем ниже верхнего ряда светодиодов (обычно верхняя полоса занимает первые 60-90px)
                        let banner_y = (screen_h as f32 * 0.12).round().max(90.0) as i32;
                        let rc_banner = RECT {
                            left: banner_x,
                            top: banner_y,
                            right: banner_x + banner_w,
                            bottom: banner_y + banner_h,
                        };

                        // Тёмно-графитовый фон плашки
                        let bg_banner = CreateSolidBrush(COLORREF(0x001A1816));
                        FillRect(mem_dc, &rc_banner, bg_banner);
                        let _ = DeleteObject(bg_banner);

                        // Оранжевая рамка AmbiBox (BGR: R=0xFA, G=0x8C, B=0x16 -> 0x00168CFA)
                        let pen = CreatePen(PS_SOLID, 1, COLORREF(0x00168CFA));
                        let old_pen = SelectObject(mem_dc, pen);
                        let null_brush = windows::Win32::Graphics::Gdi::GetStockObject(windows::Win32::Graphics::Gdi::NULL_BRUSH);
                        let old_brush = SelectObject(mem_dc, null_brush);
                        let _ = Rectangle(mem_dc, banner_x, banner_y, banner_x + banner_w, banner_y + banner_h);
                        SelectObject(mem_dc, old_brush);
                        SelectObject(mem_dc, old_pen);
                        let _ = DeleteObject(pen);

                        // Мультиязычный читаемый текст подсказки
                        SetTextColor(mem_dc, COLORREF(0x00FFFFFF));
                        let is_ru = state.config.read().language.to_lowercase() == "ru";
                        let text_str = if is_ru {
                            "ReAmbiBox — Настройка зон | ЛКМ: Выбор и перемещение | ПКМ / Esc: Закрыть [✕]\0"
                        } else {
                            "ReAmbiBox — Zone Setup | LMB: Select & Move | RMB / Esc: Close [✕]\0"
                        };
                        let mut text: Vec<u16> = text_str.encode_utf16().collect();
                        let mut rc_text = rc_banner;
                        DrawTextW(mem_dc, &mut text, &mut rc_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                    }

                    // 4. Поэтапная отрисовка всех зон:
                    // Пасс 1 & 2: Заливка фона всех зон (мягкий глубокий синий тон)
                    // Пасс 3: Рамки всех обычных зон (тонкий #60a5fa 1px поверх всех заливок)
                    // Пасс 4: Рамка и маркер активной зоны (яркий циан #22d3ee 2px + янтарь #f59e0b)
                    // Пасс 5: Текст номеров всех зон с контрастными тенями
                    // Благодаря разделению на пассы заливка ОДНОЙ зоны НИКОГДА не перекрывает рамку соседней!

                    let zone_rects: Vec<(usize, RECT, bool)> = zones
                        .iter()
                        .map(|z| {
                            let zx = (z.rect.x * sw_f32).round() as i32;
                            let zy = (z.rect.y * sh_f32).round() as i32;
                            let zw = (z.rect.width * sw_f32).round().max(16.0) as i32;
                            let zh = (z.rect.height * sh_f32).round().max(16.0) as i32;
                            let is_active = state.active_zone_id == Some(z.id);
                            (
                                z.id,
                                RECT {
                                    left: zx,
                                    top: zy,
                                    right: zx + zw,
                                    bottom: zy + zh,
                                },
                                is_active,
                            )
                        })
                        .collect();

                    // Пасс 1 & 2: Заливка фона (гарантирует захват мыши DWM по всей площади зоны)
                    let inactive_fill_brush = CreateSolidBrush(COLORREF(0x003A2214)); // Мягкий полупрозрачный синий
                    let active_fill_brush = CreateSolidBrush(COLORREF(0x004A3B10));   // Подсвеченный циан
                    for (_, rc, is_active) in &zone_rects {
                        FillRect(mem_dc, rc, if *is_active { active_fill_brush } else { inactive_fill_brush });
                    }
                    let _ = DeleteObject(inactive_fill_brush);
                    let _ = DeleteObject(active_fill_brush);

                    // Пасс 3: Отрисовка рамок обычных зон поверх ВСЕХ фонов (небесно-голубой #60a5fa, 1px)
                    let pen_inactive = CreatePen(PS_SOLID, 1, COLORREF(0x00FAA560));
                    let null_brush = windows::Win32::Graphics::Gdi::GetStockObject(windows::Win32::Graphics::Gdi::NULL_BRUSH);
                    let old_brush = SelectObject(mem_dc, null_brush);
                    let old_pen = SelectObject(mem_dc, pen_inactive);

                    for (_, rc, is_active) in &zone_rects {
                        if !*is_active {
                            let _ = Rectangle(mem_dc, rc.left, rc.top, rc.right, rc.bottom);
                        }
                    }

                    // Пасс 4: Отрисовка рамки и маркера активной зоны поверх всего (неоновый циан #22d3ee, 2px)
                    let pen_active = CreatePen(PS_SOLID, 2, COLORREF(0x00EED322));
                    SelectObject(mem_dc, pen_active);

                    for (_, rc, is_active) in &zone_rects {
                        if *is_active {
                            let _ = Rectangle(mem_dc, rc.left, rc.top, rc.right, rc.bottom);

                            // Маркер изменения размера в правом нижнем углу (янтарный #f59e0b)
                            let handle_size = 12;
                            let handle_brush = CreateSolidBrush(COLORREF(0x000B9EF5));
                            let rc_handle = RECT {
                                left: rc.right - handle_size,
                                top: rc.bottom - handle_size,
                                right: rc.right,
                                bottom: rc.bottom,
                            };
                            FillRect(mem_dc, &rc_handle, handle_brush);
                            let _ = DeleteObject(handle_brush);
                        }
                    }

                    SelectObject(mem_dc, old_brush);
                    SelectObject(mem_dc, old_pen);
                    let _ = DeleteObject(pen_inactive);
                    let _ = DeleteObject(pen_active);

                    // Пасс 5: Отрисовка номеров всех зон с четкой черной тенью (100% читаемость)
                    for (id, rc, is_active) in &zone_rects {
                        let mut num_text: Vec<u16> = format!("{}\0", id + 1).encode_utf16().collect();
                        SetTextColor(mem_dc, COLORREF(0x00000000));
                        for (dx, dy) in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            let mut rc_shadow = RECT {
                                left: rc.left + dx,
                                top: rc.top + dy,
                                right: rc.right + dx,
                                bottom: rc.bottom + dy,
                            };
                            let mut num_text_shadow = num_text.clone();
                            DrawTextW(mem_dc, &mut num_text_shadow, &mut rc_shadow, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                        }

                        SetTextColor(mem_dc, if *is_active { COLORREF(0x00EED322) } else { COLORREF(0x00FFFFFF) });
                        let mut rc_num = *rc;
                        DrawTextW(mem_dc, &mut num_text, &mut rc_num, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                    }

                    SelectObject(mem_dc, old_font);
                    let _ = DeleteObject(h_font);

                    // Мгновенный аппаратный перенос кадра на экран (BitBlt без шлейфов)
                    let _ = BitBlt(hdc, 0, 0, screen_w, screen_h, mem_dc, 0, 0, SRCCOPY);
                }
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_NCHITTEST => {
            // Перехватываем мышь по всей площади оверлея, чтобы клики не улетали
            // в фоновые окна и оверлей не терял фокус
            LRESULT(HTCLIENT as isize)
        }

        WM_LBUTTONDOWN => {
            // Получаем точные клиентские координаты курсора мыши
            let (mouse_x, mouse_y) = {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                let _ = ScreenToClient(hwnd, &mut pt);
                (pt.x as f32, pt.y as f32)
            };

            let mut should_close = false;
            let mut captured = false;

            {
                if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                    let screen_w = state.screen_width as f32;
                    let screen_h = state.screen_height as f32;

                    // 1. Проверяем клик по правой части баннера (кнопка [✕])
                    let banner_w = 720.0;
                    let banner_h = 40.0;
                    let banner_x = (screen_w - banner_w) / 2.0;
                    let banner_y = (screen_h * 0.12).round().max(90.0);
                    if mouse_x >= banner_x + banner_w - 70.0 && mouse_x <= banner_x + banner_w
                        && mouse_y >= banner_y && mouse_y <= banner_y + banner_h
                    {
                        should_close = true;
                    }

                    if !should_close {
                        let cfg = state.config.read();
                        let mut clicked_zone = None;
                        let mut is_resize = false;

                        // Если зона уже была выбрана, сначала проверяем её (особенно маркер ресайза)
                        let hit_margin = 4.0;
                        if let Some(active_id) = state.active_zone_id {
                            if let Some(zone) = cfg.zones.iter().find(|z| z.id == active_id) {
                                let zx = zone.rect.x * screen_w;
                                let zy = zone.rect.y * screen_h;
                                let zw = (zone.rect.width * screen_w).max(16.0);
                                let zh = (zone.rect.height * screen_h).max(16.0);

                                if mouse_x >= zx - hit_margin && mouse_x <= zx + zw + hit_margin
                                    && mouse_y >= zy - hit_margin && mouse_y <= zy + zh + hit_margin
                                {
                                    clicked_zone = Some(active_id);
                                    if mouse_x >= zx + zw - 18.0 && mouse_y >= zy + zh - 18.0 {
                                        is_resize = true;
                                    }
                                    state.drag_offset_x = (mouse_x - zx) / screen_w;
                                    state.drag_offset_y = (mouse_y - zy) / screen_h;
                                }
                            }
                        }

                        // Если клик не в текущую активную зону, ищем среди остальных зон (с конца к началу)
                        if clicked_zone.is_none() {
                            for zone in cfg.zones.iter().rev() {
                                let zx = zone.rect.x * screen_w;
                                let zy = zone.rect.y * screen_h;
                                let zw = (zone.rect.width * screen_w).max(16.0);
                                let zh = (zone.rect.height * screen_h).max(16.0);

                                if mouse_x >= zx - hit_margin && mouse_x <= zx + zw + hit_margin
                                    && mouse_y >= zy - hit_margin && mouse_y <= zy + zh + hit_margin
                                {
                                    clicked_zone = Some(zone.id);
                                    // Проверяем попадание в правый нижний угол (область 18x18px для ресайза)
                                    if mouse_x >= zx + zw - 18.0 && mouse_y >= zy + zh - 18.0 {
                                        is_resize = true;
                                    }
                                    state.drag_offset_x = (mouse_x - zx) / screen_w;
                                    state.drag_offset_y = (mouse_y - zy) / screen_h;
                                    break;
                                }
                            }
                        }

                        if let Some(id) = clicked_zone {
                            println!("[Overlay] Захват зоны ID {} (позиция: {:.0}, {:.0}, resize: {})", id, mouse_x, mouse_y, is_resize);
                            state.active_zone_id = Some(id);
                            if is_resize {
                                state.is_resizing = true;
                            } else {
                                state.is_dragging = true;
                            }
                            captured = true;
                        } else {
                            // Клик в пустое место экрана — снимаем выделение активной зоны
                            state.active_zone_id = None;
                            state.is_dragging = false;
                            state.is_resizing = false;
                        }
                    }
                }
            } // МЬЮТЕКС GLOBAL_OVERLAY ПОЛНОСТЬЮ ОСВОБОЖДЕН ПЕРЕД ЛЮБЫМИ ВЫЗОВАМИ GDI!

            if should_close {
                if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                    state.is_visible.store(false, Ordering::Relaxed);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let _ = state.config.read().save();
                    println!("[Overlay] Закрытие оверлея по клику на [✕] (настройки сохранены)");
                }
                return LRESULT(0);
            }

            if captured {
                SetCapture(hwnd);
            }

            // Мгновенная перерисовка без дедлока:
            let _ = InvalidateRect(hwnd, None, false);
            let _ = UpdateWindow(hwnd);
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let (mouse_x, mouse_y) = {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                let _ = ScreenToClient(hwnd, &mut pt);
                (pt.x as f32, pt.y as f32)
            };

            let mut needs_redraw = false;

            {
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
                        needs_redraw = true;
                    }
                }
            } // МЬЮТЕКС GLOBAL_OVERLAY И config.write() ПОЛНОСТЬЮ ОСВОБОЖДЕНЫ!

            if needs_redraw {
                let _ = InvalidateRect(hwnd, None, false);
                let _ = UpdateWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            let mut needs_redraw = false;

            {
                if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                    if state.is_dragging || state.is_resizing {
                        println!("[Overlay] Положение зоны зафиксировано и сохранено");
                        state.is_dragging = false;
                        state.is_resizing = false;
                        // Сохраняем перемещенные зоны в config.json
                        let _ = state.config.read().save();
                        needs_redraw = true;
                    }
                }
            } // МЬЮТЕКС GLOBAL_OVERLAY ОСВОБОЖДЕН!

            if needs_redraw {
                let _ = InvalidateRect(hwnd, None, false);
                let _ = UpdateWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_CAPTURECHANGED => {
            let gaining_hwnd = HWND(lparam.0 as *mut _);
            if gaining_hwnd != hwnd {
                if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                    state.is_dragging = false;
                    state.is_resizing = false;
                }
            }
            LRESULT(0)
        }

        WM_RBUTTONUP => {
            // Закрытие оверлея по клику правой кнопкой мыши
            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                state.is_visible.store(false, Ordering::Relaxed);
                state.is_dragging = false;
                state.is_resizing = false;
                state.active_zone_id = None;
                let _ = ShowWindow(hwnd, SW_HIDE);
                let _ = state.config.read().save();
                println!("[Overlay] Закрытие оверлея по клику правой кнопки мыши (настройки сохранены)");
            }
            LRESULT(0)
        }

        WM_SETCURSOR => {
            let hit_test = (lparam.0 & 0xFFFF) as u32;
            if hit_test == HTCLIENT {
                let mut cursor_type = IDC_ARROW;
                if let Some(ref state) = *GLOBAL_OVERLAY.lock() {
                    if state.is_resizing {
                        cursor_type = IDC_SIZENWSE;
                    }
                }
                let h_cursor = LoadCursorW(None, cursor_type).unwrap_or_default();
                if !h_cursor.0.is_null() {
                    let _ = SetCursor(h_cursor);
                    return LRESULT(1);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
                // Закрытие оверлея по клавише Esc
                if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                    state.is_visible.store(false, Ordering::Relaxed);
                    state.is_dragging = false;
                    state.is_resizing = false;
                    state.active_zone_id = None;
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let _ = state.config.read().save();
                    println!("[Overlay] Закрытие оверлея по клавише Esc (настройки сохранены)");
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            if let Some(ref mut state) = *GLOBAL_OVERLAY.lock() {
                if let Some(mem_dc) = state.mem_dc {
                    if let Some(old_b) = state.old_bmp {
                        SelectObject(mem_dc, old_b);
                    }
                    if let Some(bmp) = state.mem_bmp {
                        let _ = DeleteObject(bmp);
                    }
                    let _ = DeleteDC(mem_dc);
                }
                state.mem_dc = None;
                state.mem_bmp = None;
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

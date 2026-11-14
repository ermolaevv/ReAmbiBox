use parking_lot::RwLock;
use reambibox_core::config::AppConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    RegisterPowerSettingNotification, UnregisterPowerSettingNotification, HPOWERNOTIFY,
};
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, TrackPopupMenuEx, TranslateMessage, HICON, HMENU, IDI_APPLICATION,
    MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG, REGISTER_NOTIFICATION_FLAGS,
    SW_SHOWNORMAL, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_COMMAND, WM_DESTROY,
    WM_ENDSESSION, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_POWERBROADCAST,
    WM_QUERYENDSESSION, WM_RBUTTONUP, WM_USER, WM_WTSSESSION_CHANGE, WNDCLASSW,
};

// Константы оконных сообщений и идентификаторов меню
const WM_TRAYICON: u32 = WM_USER + 101;
const WM_RELOAD_HOTKEYS: u32 = WM_USER + 102;
const ID_MENU_TOGGLE: usize = 2001;
const ID_MENU_OVERLAY: usize = 2005;
const ID_MENU_WEBUI: usize = 2002;
const ID_MENU_AUTOSTART: usize = 2003;
const ID_MENU_EXIT: usize = 2004;
const ID_MENU_LANG_RU: usize = 2006;
const ID_MENU_LANG_EN: usize = 2007;
const ID_MENU_PRESET_BASE: usize = 3000;

// Идентификаторы глобальных горячих клавиш Windows
const HOTKEY_ID_TOGGLE_POWER: i32 = 101;
const HOTKEY_ID_BRIGHTNESS_UP: i32 = 102;
const HOTKEY_ID_BRIGHTNESS_DOWN: i32 = 103;
const HOTKEY_ID_TOGGLE_OVERLAY: i32 = 104;

// GUID_MONITOR_POWER_ON: {0255fa76-4ae3-492d-9e84-a37f42d2dd77}
const GUID_MONITOR_POWER_ON: windows::core::GUID = windows::core::GUID::from_u128(0x0255fa76_4ae3_492d_9e84_a37f42d2dd77);
const PBT_POWERSETTINGCHANGE: usize = 0x8013;
const WTS_SESSION_LOCK: usize = 0x7;
const WTS_SESSION_UNLOCK: usize = 0x8;

struct TrayContext {
    config: Arc<RwLock<AppConfig>>,
    is_running: Arc<AtomicBool>,
    is_display_on: Arc<AtomicBool>,
    is_session_unlocked: Arc<AtomicBool>,
    overlay_manager: Arc<crate::zone_overlay::ZoneOverlayManager>,
    h_power_notify: Option<HPOWERNOTIFY>,
    hwnd: HWND,
}

unsafe impl Send for TrayContext {}
unsafe impl Sync for TrayContext {}

static GLOBAL_TRAY_CTX: parking_lot::Mutex<Option<TrayContext>> = parking_lot::Mutex::new(None);

pub struct TrayManager {
    is_display_on: Arc<AtomicBool>,
    is_session_unlocked: Arc<AtomicBool>,
    hwnd: parking_lot::Mutex<Option<HWND>>,
    last_lock_check: Arc<parking_lot::Mutex<std::time::Instant>>,
}

unsafe impl Send for TrayManager {}
unsafe impl Sync for TrayManager {}

impl TrayManager {
    pub fn new() -> Self {
        let initial_locked = is_workstation_locked();
        if initial_locked {
            println!("[Сессия] Начальное состояние: экран заблокирован (Lock Screen / вход в систему) — подсветка отключена");
        }
        Self {
            is_display_on: Arc::new(AtomicBool::new(true)),
            is_session_unlocked: Arc::new(AtomicBool::new(!initial_locked)),
            hwnd: parking_lot::Mutex::new(None),
            last_lock_check: Arc::new(parking_lot::Mutex::new(std::time::Instant::now())),
        }
    }

    pub fn is_display_active(&self) -> bool {
        if !self.is_display_on.load(Ordering::Relaxed) {
            return false;
        }

        // Периодическая проверка статуса блокировки (раз в 1 сек).
        // Защищает от ситуаций, когда процесс стартовал до входа в систему (ARSO),
        // либо когда экран заблокирован/разблокирован без доставки WTS-сообщения.
        let mut last_check = self.last_lock_check.lock();
        if last_check.elapsed() >= std::time::Duration::from_millis(1000) {
            *last_check = std::time::Instant::now();
            let locked = is_workstation_locked();
            let current = self.is_session_unlocked.load(Ordering::Relaxed);
            if locked && current {
                println!("[Сессия] Обнаружена блокировка экрана — гасим подсветку");
                self.is_session_unlocked.store(false, Ordering::Relaxed);
                return false;
            } else if !locked && !current {
                println!("[Сессия] Обнаружена разблокировка экрана — возобновляем подсветку");
                self.is_session_unlocked.store(true, Ordering::Relaxed);
                return true;
            }
        }

        self.is_session_unlocked.load(Ordering::Relaxed)
    }

    /// Принудительная проверка статуса блокировки (например, при потере сессии DXGI)
    pub fn force_check_lock_state(&self) -> bool {
        let locked = is_workstation_locked();
        let current = self.is_session_unlocked.load(Ordering::Relaxed);
        if locked && current {
            println!("[Сессия] Подтверждена блокировка экрана — гасим подсветку");
            self.is_session_unlocked.store(false, Ordering::Relaxed);
        } else if !locked && !current {
            println!("[Сессия] Подтверждена разблокировка экрана — возобновляем подсветку");
            self.is_session_unlocked.store(true, Ordering::Relaxed);
        }
        *self.last_lock_check.lock() = std::time::Instant::now();
        locked
    }

    pub fn reload_hotkeys(&self) {
        if let Some(hwnd) = *self.hwnd.lock() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    hwnd,
                    WM_RELOAD_HOTKEYS,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }

    pub fn run_tray_thread(
        &self,
        config: Arc<RwLock<AppConfig>>,
        is_running: Arc<AtomicBool>,
        overlay_manager: Arc<crate::zone_overlay::ZoneOverlayManager>,
    ) {
        let display_on_ref = Arc::clone(&self.is_display_on);
        let session_unlocked_ref = Arc::clone(&self.is_session_unlocked);
        let (tx_hwnd, rx_hwnd) = std::sync::mpsc::channel::<usize>();

        std::thread::spawn(move || unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = w!("ReAmbiBoxTrayClass");

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(tray_wnd_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };

            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                class_name,
                w!("ReAmbiBoxTrayWindow"),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                HWND(std::ptr::null_mut()),
                HMENU(std::ptr::null_mut()),
                HINSTANCE(instance.0),
                None,
            ).unwrap_or_default();

            if hwnd.0.is_null() {
                eprintln!("[Tray] Не удалось создать окно сообщений трея");
                return;
            }

            let _ = tx_hwnd.send(hwnd.0 as usize);

            // Добавляем иконку в системный трей (используем встроенную иконку ReAmbiBox, либо системную как fallback)
            let icon: HICON = LoadIconW(HINSTANCE(instance.0), PCWSTR(1 as _))
                .or_else(|_| LoadIconW(None, IDI_APPLICATION))
                .unwrap_or_default();
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: icon,
                ..Default::default()
            };

            let tip = "ReAmbiBox — Фоновый Ambilight сервис\0".encode_utf16().collect::<Vec<u16>>();
            let copy_len = tip.len().min(nid.szTip.len());
            nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            // Регистрация на события питания (включение/выключение монитора)
            let h_power = RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_MONITOR_POWER_ON,
                REGISTER_NOTIFICATION_FLAGS(0), // DEVICE_NOTIFY_WINDOW_HANDLE
            ).ok();

            // Регистрация на события блокировки сессии (Win + L)
            let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);

            // Регистрация глобальных горячих клавиш Windows из конфигурации
            let hotkeys_cfg = config.read().hotkeys.clone();
            register_configured_hotkeys(hwnd, &hotkeys_cfg);

            *GLOBAL_TRAY_CTX.lock() = Some(TrayContext {
                config,
                is_running: Arc::clone(&is_running),
                is_display_on: display_on_ref,
                is_session_unlocked: session_unlocked_ref,
                overlay_manager,
                h_power_notify: h_power,
                hwnd,
            });

            // Стандартный цикл обработки сообщений Windows
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Очистка перед выходом
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE_POWER);
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_BRIGHTNESS_UP);
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_BRIGHTNESS_DOWN);
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE_OVERLAY);
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            if let Some(ctx) = GLOBAL_TRAY_CTX.lock().take() {
                if let Some(h) = ctx.h_power_notify {
                    let _ = UnregisterPowerSettingNotification(h);
                }
                let _ = WTSUnRegisterSessionNotification(hwnd);
            }
            is_running.store(false, Ordering::Relaxed);
        });

        if let Ok(raw_hwnd) = rx_hwnd.recv() {
            *self.hwnd.lock() = Some(HWND(raw_hwnd as *mut _));
        }
    }
}

/// Проверка, заблокирован ли в данный момент рабочий стол Windows (Lock screen / Winlogon / UAC).
/// Возвращает `true`, если экран заблокирован или сессия не активна.
pub fn is_workstation_locked() -> bool {
    unsafe {
        use windows::Win32::System::RemoteDesktop::{
            WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION, WTSINFOEXW,
        };

        // 1. Пробуем через официальный WTS API сессии
        let mut buffer: windows::core::PWSTR = windows::core::PWSTR(std::ptr::null_mut());
        let mut bytes_returned: u32 = 0;
        if WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes_returned,
        ).is_ok() && !buffer.0.is_null() {
            let info = &*(buffer.0 as *const WTSINFOEXW);
            let locked_opt = if info.Level == 1 {
                let flags = info.Data.WTSInfoExLevel1.SessionFlags;
                // WTS_SESSIONSTATE_LOCK = 0, WTS_SESSIONSTATE_UNLOCK = 1, WTS_SESSIONSTATE_UNKNOWN = 0xFFFFFFFF
                if flags == 0 {
                    Some(true)
                } else if flags == 1 {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            };
            WTSFreeMemory(buffer.0 as *mut _);

            if let Some(locked) = locked_opt {
                return locked;
            }
        }

        // 2. Fallback через OpenInputDesktop:
        // Если станция заблокирована, активным рабочим столом является Winlogon / LogonUI,
        // к которому у обычного процесса нет прав доступа, и вызов возвращает ошибку (ERROR_ACCESS_DENIED).
        use windows::Win32::System::StationsAndDesktops::{
            CloseDesktop, OpenInputDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
        };
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x0100)) {
            Ok(desktop) => {
                let _ = CloseDesktop(desktop);
                false
            }
            Err(_) => true,
        }
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                // Открытие веб-панели управления в браузере по умолчанию
                open_web_ui();
            } else if event == WM_RBUTTONUP {
                // Контекстное меню по клику правой кнопкой мыши
                show_context_menu(hwnd);
            }
            LRESULT(0)
        }

        WM_COMMAND => {
            let cmd_id = wparam.0 as usize;
            handle_menu_command(cmd_id);
            LRESULT(0)
        }

        WM_POWERBROADCAST => {
            if wparam.0 == PBT_POWERSETTINGCHANGE {
                // Смена состояния питания монитора
                if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                    // Читаем значение из POWERBROADCAST_SETTING: 0 = выключен, 1 = включен, 2 = dim
                    let setting_ptr = lparam.0 as *const windows::Win32::System::Power::POWERBROADCAST_SETTING;
                    if !setting_ptr.is_null() {
                        let setting = &*setting_ptr;
                        if setting.PowerSetting == GUID_MONITOR_POWER_ON && setting.DataLength >= 1 {
                            let state_byte = *setting.Data.as_ptr();
                            let is_on = state_byte != 0;
                            ctx.is_display_on.store(is_on, Ordering::Relaxed);
                            println!("[Питание] Состояние дисплея: {}", if is_on { "ВКЛЮЧЕН" } else { "ВЫКЛЮЧЕН (Сон)" });
                        }
                    }
                }
            }
            LRESULT(1)
        }

        WM_WTSSESSION_CHANGE => {
            let event_type = wparam.0;
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                if event_type == WTS_SESSION_LOCK {
                    ctx.is_session_unlocked.store(false, Ordering::Relaxed);
                    println!("[Сессия] Получено уведомление Windows: экран заблокирован (Win + L) — гасим подсветку");
                } else if event_type == WTS_SESSION_UNLOCK {
                    ctx.is_session_unlocked.store(true, Ordering::Relaxed);
                    println!("[Сессия] Получено уведомление Windows: экран разблокирован — возобновляем подсветку");
                }
            }
            LRESULT(0)
        }

        WM_HOTKEY => {
            let hotkey_id = wparam.0 as i32;
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                match hotkey_id {
                    HOTKEY_ID_TOGGLE_POWER => {
                        let mut cfg = ctx.config.write();
                        cfg.is_active = !cfg.is_active;
                        let active = cfg.is_active;
                        let _ = cfg.save();
                        println!("[Hotkey] Питание подсветки: {}", if active { "ВКЛ" } else { "ВЫКЛ" });
                    }
                    HOTKEY_ID_BRIGHTNESS_UP => {
                        let mut cfg = ctx.config.write();
                        cfg.brightness = (cfg.brightness + 0.1).clamp(0.0, 1.0);
                        let b = cfg.brightness;
                        let _ = cfg.save();
                        println!("[Hotkey] Яркость увеличена: {:.0}%", b * 100.0);
                    }
                    HOTKEY_ID_BRIGHTNESS_DOWN => {
                        let mut cfg = ctx.config.write();
                        cfg.brightness = (cfg.brightness - 0.1).clamp(0.0, 1.0);
                        let b = cfg.brightness;
                        let _ = cfg.save();
                        println!("[Hotkey] Яркость уменьшена: {:.0}%", b * 100.0);
                    }
                    HOTKEY_ID_TOGGLE_OVERLAY => {
                        ctx.overlay_manager.toggle();
                        println!("[Hotkey] Переключение экранного оверлея зон");
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }

        WM_RELOAD_HOTKEYS => {
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                let hotkeys_cfg = ctx.config.read().hotkeys.clone();
                register_configured_hotkeys(hwnd, &hotkeys_cfg);
            }
            LRESULT(0)
        }

        WM_QUERYENDSESSION | WM_ENDSESSION => {
            println!("[Завершение Windows] Получен сигнал завершения сеанса/системы — тушим подсветку");
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                ctx.is_running.store(false, Ordering::Relaxed);
                let (port, baud, leds) = {
                    let cfg = ctx.config.read();
                    (cfg.serial_port.clone(), cfg.baud_rate, cfg.zones.len())
                };
                reambibox_core::device::adalight::AdalightDriver::emergency_blank(&port, baud, leds);
            }
            LRESULT(1)
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn format_hotkey_label(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("disabled")
        || trimmed == "Отключено"
        || trimmed == "отключено"
    {
        "Отключено"
    } else {
        trimmed
    }
}

unsafe fn register_configured_hotkeys(hwnd: HWND, cfg: &reambibox_core::config::HotkeysConfig) {
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE_POWER);
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID_BRIGHTNESS_UP);
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID_BRIGHTNESS_DOWN);
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE_OVERLAY);

    if !cfg.enabled {
        println!("[Hotkeys] Глобальные горячие клавиши отключены в настройках.");
        return;
    }

    if let Some((mods, vk)) = crate::hotkey_parser::parse_hotkey(&cfg.toggle_power) {
        let _ = RegisterHotKey(hwnd, HOTKEY_ID_TOGGLE_POWER, mods, vk);
    }
    if let Some((mods, vk)) = crate::hotkey_parser::parse_hotkey(&cfg.brightness_up) {
        let _ = RegisterHotKey(hwnd, HOTKEY_ID_BRIGHTNESS_UP, mods, vk);
    }
    if let Some((mods, vk)) = crate::hotkey_parser::parse_hotkey(&cfg.brightness_down) {
        let _ = RegisterHotKey(hwnd, HOTKEY_ID_BRIGHTNESS_DOWN, mods, vk);
    }
    if let Some((mods, vk)) = crate::hotkey_parser::parse_hotkey(&cfg.toggle_overlay) {
        let _ = RegisterHotKey(hwnd, HOTKEY_ID_TOGGLE_OVERLAY, mods, vk);
    }

    println!(
        "[Hotkeys] Комбинации: Питание ({}), Яркость +10% ({}), Яркость -10% ({}), Оверлей ({})",
        format_hotkey_label(&cfg.toggle_power),
        format_hotkey_label(&cfg.brightness_up),
        format_hotkey_label(&cfg.brightness_down),
        format_hotkey_label(&cfg.toggle_overlay)
    );
}

unsafe fn open_web_ui() {
    let _ = ShellExecuteW(
        None,
        w!("open"),
        w!("http://localhost:3637"),
        None,
        None,
        SW_SHOWNORMAL,
    );
}

unsafe fn show_context_menu(hwnd: HWND) {
    let menu = match CreatePopupMenu() {
        Ok(m) => m,
        Err(_) => return,
    };

    let (is_active, is_en) = if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
        let cfg = ctx.config.read();
        (cfg.is_active, cfg.language.eq_ignore_ascii_case("en"))
    } else {
        (true, false)
    };

    let is_autostart = crate::autostart::is_autostart_enabled();

    let toggle_text = if is_en {
        if is_active {
            w!("Ambient Light: On\tCtrl+Shift+A")
        } else {
            w!("Ambient Light: Off\tCtrl+Shift+A")
        }
    } else if is_active {
        w!("Подсветка: Включена\tCtrl+Shift+A")
    } else {
        w!("Подсветка: Выключена\tCtrl+Shift+A")
    };

    let autostart_flags = if is_autostart { MF_CHECKED } else { MF_UNCHECKED } | MF_STRING;

    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_TOGGLE, toggle_text);

    let (presets, active_preset) = if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
        let cfg = ctx.config.read();
        (cfg.presets.clone(), cfg.active_preset.clone())
    } else {
        (Vec::new(), None)
    };

    if !presets.is_empty() {
        if let Ok(preset_menu) = CreatePopupMenu() {
            for (idx, p) in presets.iter().enumerate() {
                let flags = if active_preset.as_deref() == Some(&p.id) {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                } | MF_STRING;

                let wide_name = windows::core::HSTRING::from(&p.name);
                let _ = AppendMenuW(
                    preset_menu,
                    flags,
                    ID_MENU_PRESET_BASE + idx,
                    PCWSTR(wide_name.as_ptr()),
                );
            }
            let _ = AppendMenuW(
                menu,
                windows::Win32::UI::WindowsAndMessaging::MF_POPUP,
                preset_menu.0 as usize,
                if is_en { w!("Presets") } else { w!("Пресеты") },
            );
        }
    }

    let overlay_label = if is_en {
        w!("Show Screen Zones\tCtrl+Shift+O")
    } else {
        w!("Показать зоны на экране\tCtrl+Shift+O")
    };
    let webui_label = if is_en {
        w!("Control Panel (Web UI)...")
    } else {
        w!("Панель управления (Web UI)...")
    };
    let autostart_label = if is_en {
        w!("Start with Windows")
    } else {
        w!("Запуск вместе с Windows")
    };
    let exit_label = if is_en {
        w!("Exit")
    } else {
        w!("Выход")
    };

    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_OVERLAY, overlay_label);
    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_WEBUI, webui_label);
    let _ = AppendMenuW(menu, autostart_flags, ID_MENU_AUTOSTART, autostart_label);

    // Подменю выбора языка (Language)
    if let Ok(lang_menu) = CreatePopupMenu() {
        let ru_flags = if is_en { MF_UNCHECKED } else { MF_CHECKED } | MF_STRING;
        let en_flags = if is_en { MF_CHECKED } else { MF_UNCHECKED } | MF_STRING;
        let _ = AppendMenuW(lang_menu, ru_flags, ID_MENU_LANG_RU, w!("Русский (RU)"));
        let _ = AppendMenuW(lang_menu, en_flags, ID_MENU_LANG_EN, w!("English (EN)"));
        let _ = AppendMenuW(
            menu,
            windows::Win32::UI::WindowsAndMessaging::MF_POPUP,
            lang_menu.0 as usize,
            if is_en { w!("Language") } else { w!("Язык (Language)") },
        );
    }

    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, exit_label);

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenuEx(
        menu,
        (TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON).0,
        pt.x,
        pt.y,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
}

unsafe fn handle_menu_command(cmd_id: usize) {
    match cmd_id {
        ID_MENU_TOGGLE => {
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                let mut cfg = ctx.config.write();
                cfg.is_active = !cfg.is_active;
                let _ = cfg.save();
                println!("[Трей] Подсветка переключена: {}", if cfg.is_active { "ВКЛ" } else { "ВЫКЛ" });
            }
        }
        ID_MENU_LANG_RU => {
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                let mut cfg = ctx.config.write();
                cfg.language = "ru".to_string();
                let _ = cfg.save();
                println!("[Трей] Язык переключен: Русский (RU)");
            }
        }
        ID_MENU_LANG_EN => {
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                let mut cfg = ctx.config.write();
                cfg.language = "en".to_string();
                let _ = cfg.save();
                println!("[Tray] Language changed: English (EN)");
            }
        }
        ID_MENU_OVERLAY => {
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                ctx.overlay_manager.toggle();
            }
        }
        ID_MENU_WEBUI => {
            open_web_ui();
        }
        ID_MENU_AUTOSTART => {
            let current = crate::autostart::is_autostart_enabled();
            let new_state = !current;
            let _ = crate::autostart::set_autostart(new_state);
            println!("[Трей] Автозапуск изменен: {}", if new_state { "ВКЛ" } else { "ВЫКЛ" });
        }
        cmd if cmd >= ID_MENU_PRESET_BASE && cmd < ID_MENU_PRESET_BASE + 200 => {
            let index = cmd - ID_MENU_PRESET_BASE;
            if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
                let mut cfg = ctx.config.write();
                if let Some(target) = cfg.presets.get(index).cloned() {
                    cfg.apply_preset(&target.id);
                    let _ = cfg.save();
                    println!("[Трей] Активирован пресет: {}", target.name);
                }
            }
        }
        ID_MENU_EXIT => {
            println!("[Трей] Запрос выхода из приложения...");
            let ctx_data = {
                let guard = GLOBAL_TRAY_CTX.lock();
                guard.as_ref().map(|ctx| {
                    ctx.is_running.store(false, Ordering::Relaxed);
                    let cfg = ctx.config.read();
                    (cfg.serial_port.clone(), cfg.baud_rate, cfg.zones.len(), ctx.hwnd)
                })
            };

            if let Some((port, baud, leds, hwnd)) = ctx_data {
                reambibox_core::device::adalight::AdalightDriver::emergency_blank(&port, baud, leds);
                let _ = DestroyWindow(hwnd);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_workstation_locked() {
        unsafe {
            use windows::Win32::System::RemoteDesktop::{
                WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx,
                WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTSINFOEXW,
            };
            let mut buffer: windows::core::PWSTR = windows::core::PWSTR(std::ptr::null_mut());
            let mut bytes_returned: u32 = 0;
            if WTSQuerySessionInformationW(
                WTS_CURRENT_SERVER_HANDLE,
                WTS_CURRENT_SESSION,
                WTSSessionInfoEx,
                &mut buffer,
                &mut bytes_returned,
            ).is_ok() && !buffer.0.is_null() {
                let info = &*(buffer.0 as *const WTSINFOEXW);
                println!("WTS level: {}, SessionFlags: {}", info.Level, info.Data.WTSInfoExLevel1.SessionFlags);
                WTSFreeMemory(buffer.0 as *mut _);
            } else {
                println!("WTS query returned Err or null");
            }

            use windows::Win32::System::StationsAndDesktops::{
                CloseDesktop, OpenInputDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
            };
            match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x0100)) {
                Ok(desktop) => {
                    println!("OpenInputDesktop succeeded");
                    let _ = CloseDesktop(desktop);
                }
                Err(e) => {
                    println!("OpenInputDesktop failed: {:?}", e);
                }
            }
        }

        let locked = is_workstation_locked();
        println!("Combined is_workstation_locked(): {}", locked);
    }
}

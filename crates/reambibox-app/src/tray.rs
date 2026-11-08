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
    WM_ENDSESSION, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_POWERBROADCAST, WM_QUERYENDSESSION,
    WM_RBUTTONUP, WM_USER, WM_WTSSESSION_CHANGE, WNDCLASSW,
};

// Константы оконных сообщений и идентификаторов меню
const WM_TRAYICON: u32 = WM_USER + 101;
const ID_MENU_TOGGLE: usize = 2001;
const ID_MENU_WEBUI: usize = 2002;
const ID_MENU_AUTOSTART: usize = 2003;
const ID_MENU_EXIT: usize = 2004;

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
    h_power_notify: Option<HPOWERNOTIFY>,
    hwnd: HWND,
}

unsafe impl Send for TrayContext {}
unsafe impl Sync for TrayContext {}

static GLOBAL_TRAY_CTX: parking_lot::Mutex<Option<TrayContext>> = parking_lot::Mutex::new(None);

pub struct TrayManager {
    is_display_on: Arc<AtomicBool>,
    is_session_unlocked: Arc<AtomicBool>,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            is_display_on: Arc::new(AtomicBool::new(true)),
            is_session_unlocked: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn is_display_active(&self) -> bool {
        self.is_display_on.load(Ordering::Relaxed) && self.is_session_unlocked.load(Ordering::Relaxed)
    }

    pub fn run_tray_thread(
        &self,
        config: Arc<RwLock<AppConfig>>,
        is_running: Arc<AtomicBool>,
    ) {
        let display_on_ref = Arc::clone(&self.is_display_on);
        let session_unlocked_ref = Arc::clone(&self.is_session_unlocked);

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

            *GLOBAL_TRAY_CTX.lock() = Some(TrayContext {
                config,
                is_running: Arc::clone(&is_running),
                is_display_on: display_on_ref,
                is_session_unlocked: session_unlocked_ref,
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
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            if let Some(ctx) = GLOBAL_TRAY_CTX.lock().take() {
                if let Some(h) = ctx.h_power_notify {
                    let _ = UnregisterPowerSettingNotification(h);
                }
                let _ = WTSUnRegisterSessionNotification(hwnd);
            }
            is_running.store(false, Ordering::Relaxed);
        });
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
                    println!("[Сессия] Рабочая станция заблокирована (Win + L) — гасим подсветку");
                } else if event_type == WTS_SESSION_UNLOCK {
                    ctx.is_session_unlocked.store(true, Ordering::Relaxed);
                    println!("[Сессия] Рабочая станция разблокирована — возобновляем подсветку");
                }
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

    let is_active = if let Some(ref ctx) = *GLOBAL_TRAY_CTX.lock() {
        ctx.config.read().is_active
    } else {
        true
    };

    let is_autostart = crate::autostart::is_autostart_enabled();

    let toggle_text = if is_active {
        w!("Подсветка: Включена (Нажмите для выключения)")
    } else {
        w!("Подсветка: Выключена (Нажмите для включения)")
    };

    let autostart_flags = if is_autostart { MF_CHECKED } else { MF_UNCHECKED } | MF_STRING;

    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_TOGGLE, toggle_text);
    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_WEBUI, w!("Открыть панель управления (Web UI)..."));
    let _ = AppendMenuW(menu, autostart_flags, ID_MENU_AUTOSTART, w!("Запуск вместе с Windows"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, w!("Выход"));

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
        ID_MENU_WEBUI => {
            open_web_ui();
        }
        ID_MENU_AUTOSTART => {
            let current = crate::autostart::is_autostart_enabled();
            let new_state = !current;
            let _ = crate::autostart::set_autostart(new_state);
            println!("[Трей] Автозапуск изменен: {}", if new_state { "ВКЛ" } else { "ВЫКЛ" });
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

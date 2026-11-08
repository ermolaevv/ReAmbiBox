use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};

/// Завершает любые ранее запущенные экземпляры reambibox-app.exe, гарантируя уникальность процесса в системе
pub fn ensure_single_instance_takeover() {
    let current_pid = std::process::id();
    let current_exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_else(|| "reambibox-app.exe".to_string());

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let process_id = entry.th32ProcessID;
                if process_id != current_pid && process_id != 0 {
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches(char::from(0))
                        .to_lowercase();

                    if exe_name == current_exe_name || exe_name == "reambibox-app.exe" {
                        println!("[SingleInstance] Обнаружен предыдущий экземпляр (PID {}). Завершаем...", process_id);
                        if let Ok(h_proc) = OpenProcess(
                            PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                            false,
                            process_id,
                        ) {
                            let _ = TerminateProcess(h_proc, 0);
                            let _ = WaitForSingleObject(h_proc, 2000);
                            let _ = CloseHandle(h_proc);
                            println!("[SingleInstance] Предыдущий экземпляр (PID {}) успешно завершён.", process_id);
                            // Пауза 250 мс для полного освобождения ресурсов (дескрипторов COM-порта и TCP сокетов ОС)
                            std::thread::sleep(std::time::Duration::from_millis(250));
                        }
                    }
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }
}

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "ReAmbiBox";

/// Проверка, включен ли автозапуск ReAmbiBox в реестре Windows
pub fn is_autostart_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_READ) {
        if let Ok(path) = run_key.get_value::<String, _>(APP_NAME) {
            // Проверяем, что значение существует и не пустое
            return !path.is_empty();
        }
    }
    false
}

/// Включение или отключение автозапуска в реестре Windows
pub fn set_autostart(enabled: bool) -> Result<(), std::io::Error> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE)?;

    if enabled {
        let exe_path = std::env::current_exe()?;
        let exe_str = exe_path.to_string_lossy().to_string();
        // Записываем путь в кавычках с флагом --minimized для фонового тихого запуска в трей
        let command = format!("\"{}\" --minimized", exe_str);
        run_key.set_value(APP_NAME, &command)?;
        tracing::info!("Автозапуск ReAmbiBox включен: {}", command);
    } else {
        match run_key.delete_value(APP_NAME) {
            Ok(_) => tracing::info!("Автозапуск ReAmbiBox отключен"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Если ключа уже не было, это не ошибка
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_toggle() {
        // Проверяем, что вызов функции не паникует
        let initial = is_autostart_enabled();
        
        // Включаем
        let res_enable = set_autostart(true);
        assert!(res_enable.is_ok());
        assert!(is_autostart_enabled());

        // Проверяем наличие ключа --minimized
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_READ) {
            if let Ok(val) = run_key.get_value::<String, _>(APP_NAME) {
                assert!(val.contains("--minimized"), "Команда автозапуска должна содержать --minimized");
            }
        }

        // Выключаем
        let res_disable = set_autostart(false);
        assert!(res_disable.is_ok());
        assert!(!is_autostart_enabled());

        // Возвращаем в исходное состояние
        let _ = set_autostart(initial);
    }
}

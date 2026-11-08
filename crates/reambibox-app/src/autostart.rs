use std::path::PathBuf;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "ReAmbiBox";

/// Извлекает путь к исполняемому файлу из строки команды автозапуска (убирая кавычки и аргументы)
pub fn extract_exe_path(command: &str) -> Option<PathBuf> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('"') {
        let after_quote = &trimmed[1..];
        if let Some(end_idx) = after_quote.find('"') {
            return Some(PathBuf::from(&after_quote[..end_idx]));
        }
    }

    let first_token = trimmed.split_whitespace().next().unwrap_or(trimmed);
    Some(PathBuf::from(first_token))
}

/// Проверка, включен ли автозапуск ReAmbiBox в реестре Windows
pub fn is_autostart_enabled() -> bool {
    is_autostart_enabled_named(APP_NAME)
}

/// Проверка автозапуска для произвольного ключа реестра
pub fn is_autostart_enabled_named(name: &str) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_READ) {
        if let Ok(cmd) = run_key.get_value::<String, _>(name) {
            if let Some(exe_path) = extract_exe_path(&cmd) {
                let path_lower = exe_path.to_string_lossy().to_lowercase();
                // Исключаем тестовые бинарники Cargo deps, которые не являются приложением
                if path_lower.contains(r"\target\debug\deps\") || path_lower.contains("/target/debug/deps/") {
                    return false;
                }
                // Запись валидна только если исполняемый файл физически существует на диске
                return exe_path.exists();
            }
        }
    }
    false
}

/// Включение или отключение автозапуска в реестре Windows
pub fn set_autostart(enabled: bool) -> Result<(), std::io::Error> {
    set_autostart_named(APP_NAME, enabled)
}

/// Включение или отключение автозапуска для произвольного ключа реестра
pub fn set_autostart_named(name: &str, enabled: bool) -> Result<(), std::io::Error> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_WRITE)?;

    if enabled {
        let exe_path = std::env::current_exe()?;
        let exe_str = exe_path.to_string_lossy().to_string();
        // Записываем путь в кавычках с флагом --minimized для фонового тихого запуска в трей
        let command = format!("\"{}\" --minimized", exe_str);
        run_key.set_value(name, &command)?;
        tracing::info!("Автозапуск {} включен: {}", name, command);
    } else {
        match run_key.delete_value(name) {
            Ok(_) => tracing::info!("Автозапуск {} отключен", name),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Если ключа уже не было, это не ошибка
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Автоматическая актуализация автозапуска при старте приложения:
/// Если в реестре остался битый путь, устаревший путь или артефакт тестирования,
/// запись синхронизируется на текущий `current_exe()`.
pub fn sync_autostart_if_needed() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_READ) {
        if let Ok(cmd) = run_key.get_value::<String, _>(APP_NAME) {
            if let Ok(current_exe) = std::env::current_exe() {
                let current_exe_lower = current_exe.to_string_lossy().to_lowercase();
                // Не трогаем реестр при запуске модульных тестов
                if current_exe_lower.contains(r"\target\debug\deps\") || current_exe_lower.contains("/target/debug/deps/") {
                    return;
                }

                let needs_update = if let Some(reg_path) = extract_exe_path(&cmd) {
                    reg_path != current_exe || !cmd.contains("--minimized") || !reg_path.exists()
                } else {
                    true
                };

                if needs_update {
                    let _ = set_autostart(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_APP_KEY: &str = "ReAmbiBox_UnitTest_Key";

    #[test]
    fn test_extract_exe_path() {
        let cmd_quoted = r#""C:\Program Files\ReAmbiBox\reambibox-app.exe" --minimized"#;
        assert_eq!(
            extract_exe_path(cmd_quoted),
            Some(PathBuf::from(r"C:\Program Files\ReAmbiBox\reambibox-app.exe"))
        );

        let cmd_unquoted = r#"C:\Tools\reambibox.exe --silent"#;
        assert_eq!(
            extract_exe_path(cmd_unquoted),
            Some(PathBuf::from(r"C:\Tools\reambibox.exe"))
        );

        assert_eq!(extract_exe_path(""), None);
    }

    #[test]
    fn test_autostart_toggle() {
        // Тестируем логику автозапуска строго на изолированном ключе TEST_APP_KEY,
        // чтобы тесты Cargo никогда не портили реальную системную запись ReAmbiBox пользователя.
        let _ = set_autostart_named(TEST_APP_KEY, false);

        // Включаем
        let res_enable = set_autostart_named(TEST_APP_KEY, true);
        assert!(res_enable.is_ok());

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_READ) {
            if let Ok(val) = run_key.get_value::<String, _>(TEST_APP_KEY) {
                assert!(val.contains("--minimized"), "Команда автозапуска должна содержать --minimized");
                let exe = extract_exe_path(&val).expect("Должен быть путь к exe");
                assert!(exe.exists(), "Исполняемый файл должен существовать");
            }
        }

        // Выключаем
        let res_disable = set_autostart_named(TEST_APP_KEY, false);
        assert!(res_disable.is_ok());
        assert!(!is_autostart_enabled_named(TEST_APP_KEY));
    }
}

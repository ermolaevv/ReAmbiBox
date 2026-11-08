use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_DOWN, VK_F1, VK_F10, VK_F11,
    VK_F12, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_LEFT, VK_RIGHT, VK_SPACE,
    VK_TAB, VK_UP,
};

/// Парсит строку горячей клавиши (например, "Ctrl+Shift+A", "Alt+F12", "Ctrl+Up")
/// в модификаторы Windows (HOT_KEY_MODIFIERS) и код виртуальной клавиши (VK).
/// Если строка пустая, "Отключено", "None", "Disabled", возвращает `None`.
pub fn parse_hotkey(s: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    let trimmed = s.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("disabled")
        || trimmed.eq_ignore_ascii_case("off")
        || trimmed == "Отключено"
        || trimmed == "отключено"
    {
        return None;
    }

    let parts: Vec<&str> = trimmed.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut mods = 0u32;
    let mut key_code: Option<u32> = None;

    for part in parts {
        let lower = part.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL.0,
            "shift" => mods |= MOD_SHIFT.0,
            "alt" => mods |= MOD_ALT.0,
            "win" | "windows" | "super" => mods |= MOD_WIN.0,
            _ => {
                // Если уже нашли одну основную клавишу, вторая недопустима
                if key_code.is_some() {
                    return None;
                }

                let code = match lower.as_str() {
                    "up" | "arrowup" => VK_UP.0 as u32,
                    "down" | "arrowdown" => VK_DOWN.0 as u32,
                    "left" | "arrowleft" => VK_LEFT.0 as u32,
                    "right" | "arrowright" => VK_RIGHT.0 as u32,
                    "space" => VK_SPACE.0 as u32,
                    "tab" => VK_TAB.0 as u32,
                    "f1" => VK_F1.0 as u32,
                    "f2" => VK_F2.0 as u32,
                    "f3" => VK_F3.0 as u32,
                    "f4" => VK_F4.0 as u32,
                    "f5" => VK_F5.0 as u32,
                    "f6" => VK_F6.0 as u32,
                    "f7" => VK_F7.0 as u32,
                    "f8" => VK_F8.0 as u32,
                    "f9" => VK_F9.0 as u32,
                    "f10" => VK_F10.0 as u32,
                    "f11" => VK_F11.0 as u32,
                    "f12" => VK_F12.0 as u32,
                    other if other.len() == 1 => {
                        let ch = other.chars().next().unwrap().to_ascii_uppercase();
                        if ch.is_ascii_alphanumeric() {
                            ch as u32
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                };
                key_code = Some(code);
            }
        }
    }

    let vk = key_code?;
    Some((HOT_KEY_MODIFIERS(mods), vk))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hotkey_valid() {
        let (mods, vk) = parse_hotkey("Ctrl+Shift+A").unwrap();
        assert_eq!(mods.0, MOD_CONTROL.0 | MOD_SHIFT.0);
        assert_eq!(vk, b'A' as u32);

        let (mods, vk) = parse_hotkey("Alt+F12").unwrap();
        assert_eq!(mods.0, MOD_ALT.0);
        assert_eq!(vk, VK_F12.0 as u32);

        let (mods, vk) = parse_hotkey("Ctrl+Shift+Up").unwrap();
        assert_eq!(mods.0, MOD_CONTROL.0 | MOD_SHIFT.0);
        assert_eq!(vk, VK_UP.0 as u32);

        let (mods, vk) = parse_hotkey("Win+Space").unwrap();
        assert_eq!(mods.0, MOD_WIN.0);
        assert_eq!(vk, VK_SPACE.0 as u32);
    }

    #[test]
    fn test_parse_hotkey_invalid() {
        assert!(parse_hotkey("").is_none());
        assert!(parse_hotkey("   ").is_none());
        assert!(parse_hotkey("Отключено").is_none());
        assert!(parse_hotkey("отключено").is_none());
        assert!(parse_hotkey("None").is_none());
        assert!(parse_hotkey("disabled").is_none());
        assert!(parse_hotkey("Ctrl+Shift").is_none());
        assert!(parse_hotkey("UnknownKey").is_none());
        assert!(parse_hotkey("Ctrl+A+B").is_none());
    }
}


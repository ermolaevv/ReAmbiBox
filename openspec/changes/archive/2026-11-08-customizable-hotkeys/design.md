# Technical Design: Настройка и кастомизация горячих клавиш

## Context

См. `proposal.md` — Мотивация и цели изменения.
Ранее были добавлены фиксированные комбинации клавиш (`Ctrl+Shift+A`, `Ctrl+Shift+Up/Down`, `Ctrl+Shift+O`), обрабатываемые через Win32 API `RegisterHotKeyW` в потоке `tray.rs`.

## Goals / Non-Goals

**Goals:**
- Сохранение настроек хоткеев в файле конфигурации `config.json`.
- Поддержка произвольных комбинаций модификаторов (`Ctrl`, `Shift`, `Alt`, `Win`) и функциональных/буквенно-цифровых клавиш (`A-Z`, `0-9`, `F1-F12`, `Up`, `Down` и др.).
- Динамическая перерегистрация хоткеев на лету через `PostMessageW` без перезапуска приложения.
- Удобный веб-интерфейс захвата клавиш с клавиатуры в панели управления Web UI.

**Non-Goals:**
- Глобальный кейлоггер или перехват последовательностей клавиш (макросов).
- Регистрация клавиш мыши как глобальных клавиатурных хоткеев Windows.

## Decisions

### 1. Модель данных `HotkeysConfig` в `reambibox-core`
- В `AppConfig` добавляется поле `hotkeys: HotkeysConfig`:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct HotkeysConfig {
      pub enabled: bool,
      pub toggle_power: String,    // Default: "Ctrl+Shift+A"
      pub brightness_up: String,   // Default: "Ctrl+Shift+Up"
      pub brightness_down: String, // Default: "Ctrl+Shift+Down"
      pub toggle_overlay: String,  // Default: "Ctrl+Shift+O"
  }
  ```
- Обеспечивается обратная совместимость: если в старом `config.json` этой секции нет, `#[serde(default)]` заполняет дефолтными значениями.

### 2. Парсер строковых сочетаний клавиш (`parse_hotkey`)
- Принимает строку вида `"Ctrl+Shift+A"`, `"Alt+F12"`, `"Ctrl+Up"`.
- Извлекает битовую маску модификаторов `HOT_KEY_MODIFIERS`:
  - `Ctrl` / `Control` -> `MOD_CONTROL`
  - `Shift` -> `MOD_SHIFT`
  - `Alt` -> `MOD_ALT`
  - `Win` / `Windows` -> `MOD_WIN`
- Извлекает виртуальный код клавиши `vk_code`:
  - Буквы `'A'..='Z'` (0x41..0x5A)
  - Цифры `'0'..='9'` (0x30..0x39)
  - `F1`..`F12` (0x70..0x7B)
  - Стрелки: `Up` (0x26), `Down` (0x28), `Left` (0x25), `Right` (0x27)
  - Специальные: `Space` (0x20), `Tab` (0x09), `Home` (0x24), `End` (0x23)

### 3. Динамическая смена без перезапуска (`WM_RELOAD_HOTKEYS`)
- В окне трея объявляется сообщение `WM_RELOAD_HOTKEYS = WM_USER + 102`.
- `TrayManager::reload_hotkeys(&self)` отправляет `PostMessageW(hwnd, WM_RELOAD_HOTKEYS, 0, 0)`.
- При получении `WM_RELOAD_HOTKEYS` в `tray_wnd_proc`:
  1. Вызывается `UnregisterHotKey` для всех 4 идентификаторов.
  2. Если `cfg.hotkeys.enabled == true`, парсятся и регистрируются новые комбинации через `RegisterHotKey`.
  3. Логируется результат регистрации.

### 4. Веб-интерфейс захвата клавиш
- В Web UI поля ввода хоткеев имеют атрибут `readonly`.
- При фокусе на поле слушатель `keydown` перехватывает нажатия (`e.preventDefault()`), формирует строку (например, `Ctrl+Shift+P`) и записывает её в поле.
- Кнопка «Сохранить» отправляет изменения в `POST /api/config`.

## Risks / Trade-offs

- **[Конфликт с системными хоткеями Windows (например, Win+L или Ctrl+Alt+Del)]** → Ограничиваем неперехватываемые системные клавиши; при ошибке `RegisterHotKey` выводится понятное предупреждение в лог, приложение продолжает работать.
- **[Ошибочно введённая некорректная строка]** → Парсер возвращает `None`, в этом случае регистрация пропускается с предупреждением, а старая регистрация аккуратно снимается.


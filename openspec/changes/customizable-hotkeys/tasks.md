# Tasks: Настройка и кастомизация горячих клавиш

## 1. Модель данных и парсер комбинаций клавиш

- [ ] 1.1 Добавить структуру `HotkeysConfig` в `crates/reambibox-core/src/config/mod.rs` с полями `enabled`, `toggle_power`, `brightness_up`, `brightness_down`, `toggle_overlay`, дефолтными значениями и интеграцией в `AppConfig`.
- [ ] 1.2 Создать модуль `crates/reambibox-app/src/hotkey_parser.rs` с функцией `parse_hotkey(s: &str) -> Option<(HOT_KEY_MODIFIERS, u32)>` и модульными тестами на разбор различных комбинаций (`Ctrl+Shift+A`, `Alt+F12`, `Ctrl+Shift+Up`). Проверить через `cargo test -p reambibox-app`.

## 2. Динамическая перерегистрация хоткеев в трее и API

- [ ] 2.1 Обновить `crates/reambibox-app/src/tray.rs`: добавить метод `TrayManager::reload_hotkeys()`, оконное сообщение `WM_RELOAD_HOTKEYS` и процедуру повторной регистрации комбинаций из актуального `cfg.hotkeys`.
- [ ] 2.2 В `crates/reambibox-app/src/main.rs` обновить обработчики `GET /api/config` и `POST /api/config` для чтения и записи секции `hotkeys`, а также вызова `tray_manager.reload_hotkeys()` при обновлении.

## 3. Интерфейс настройки хоткеев в Web UI

- [ ] 3.1 Добавить карточку настройки горячих клавиш на вкладку «Настройки» в `ui/index.html` с полями для каждого действия и кнопкой «Сбросить по умолчанию».
- [ ] 3.2 Добавить интерактивный перехватчик `keydown` в `ui/app.js` для удобного назначения комбинаций кликом и нажатием клавиш, и передачу в `POST /api/config`.
- [ ] 3.3 Проверить работу: изменить хоткей через веб-панель (например, назначить `Ctrl+Shift+P` вместо `Ctrl+Shift+A`) и убедиться, что новая комбинация переключает подсветку без перезапуска приложения.

# Tasks: presets-management

## 1. Модель данных пресетов (`reambibox-core::config`)

- [ ] 1.1 Создать структуру `Preset` со списком профильных полей в `crates/reambibox-core/src/config/mod.rs`
- [ ] 1.2 Добавить поля `presets: Vec<Preset>` (пустой список по умолчанию) и `active_preset: Option<String>` в `AppConfig`
- [ ] 1.3 Добавить модульные тесты для сериализации, десериализации и применения пресетов. Проверить через `cargo test -p reambibox-core`

## 2. API и синхронизация бэкенда (`reambibox-app`)

- [ ] 2.1 Реализовать API маршруты `POST /api/presets/apply`, `POST /api/presets/save`, `POST /api/presets/delete` в `crates/reambibox-app/src/main.rs`
- [ ] 2.2 Добавить поля `presets` и `active_preset` в структуру ответа `ApiStatusResponse` (`GET /api/config`)
- [ ] 2.3 Интегрировать подменю пресетов в системный трей Windows (`crates/reambibox-app/src/tray.rs`)

## 3. Пользовательский веб-интерфейс (`ui`)

- [ ] 3.1 Добавить блок выбора/сохранения пользовательских пресетов в `ui/index.html`
- [ ] 3.2 Добавить стили кнопок пресетов, бейджей и модального диалога сохранения в `ui/style.css`
- [ ] 3.3 В `ui/app.js` реализовать переключение, создание, обновление и удаление пресетов

## 4. Верификация и сборка

- [ ] 4.1 Прогнать все модульные тесты через `cargo test`
- [ ] 4.2 Собрать релизный дистрибутив через `scripts/package.ps1`
- [ ] 4.3 Зафиксировать коммит в Git и заархивировать изменение OpenSpec

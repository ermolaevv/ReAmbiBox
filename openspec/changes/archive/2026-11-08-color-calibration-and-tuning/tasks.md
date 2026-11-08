# Tasks: color-calibration-and-tuning

## 1. Ядро калибровки цвета (`reambibox-core::color`)

- [x] 1.1 Обновить структуру `GammaLut` в `crates/reambibox-core/src/color/lut.rs`: добавить 3 независимые таблицы `lut_r`, `lut_g`, `lut_b` и параметр `black_threshold`
- [x] 1.2 Реализовать алгоритм коррекции насыщенности `apply_saturation(color: RgbColor, factor: f32) -> RgbColor` на базе Rec. 601 Luma
- [x] 1.3 Добавить модульные тесты для `GammaLut` с индивидуальными коэффициентами усиления R/G/B, отсечкой черного и тесты для `apply_saturation`. Проверить через `cargo test -p reambibox-core`

## 2. Конфигурация и API сервера (`reambibox-app`)

- [x] 2.1 Добавить поля `white_balance_r`, `white_balance_g`, `white_balance_b`, `black_level_threshold`, `saturation_boost` в `AppConfig` (`crates/reambibox-core/src/config/mod.rs`) со значениями по умолчанию
- [x] 2.2 Добавить поля в структуры `ApiStatusResponse` и `ConfigUpdate` в `crates/reambibox-app/src/main.rs`, поддержать их сохранение в `config.json` и инкремент версии конфига
- [x] 2.3 Обновить инициализацию `GammaLut` и конвейера обработки кадров в потоке захвата `main.rs` с передачей параметров калибровки

## 3. Пользовательский веб-интерфейс (`ui`)

- [x] 3.1 Добавить в `ui/index.html` секцию «Калибровка цвета» с ползунками баланса белого (Красный, Зелёный, Синий), порога чёрного и насыщенности, а также кнопкой сброса
- [x] 3.2 Добавить стили слайдеров калибровки (цветовые акценты R, G, B каналов) в `ui/style.css`
- [x] 3.3 В `ui/app.js` реализовать чтение статуса, отображение значений бейджей, отправку изменений в `POST /api/config` и сброс к значениям по умолчанию

## 4. Верификация и сборка

- [x] 4.1 Выполнить полный прогон тестов `cargo test` и убедиться в успешном прохождении всех тестов
- [x] 4.2 Собрать релизный дистрибутив через `scripts/package.ps1`
- [x] 4.3 Зафиксировать коммит в Git и заархивировать изменение OpenSpec


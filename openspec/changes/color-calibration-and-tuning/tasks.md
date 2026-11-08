# Tasks: color-calibration-and-tuning

## 1. Ядро калибровки цвета (`reambibox-core::color`)

- [ ] 1.1 Обновить структуру `GammaLut` в `crates/reambibox-core/src/color/lut.rs`: добавить 3 независимые таблицы `lut_r`, `lut_g`, `lut_b` и параметр `black_threshold`
- [ ] 1.2 Реализовать алгоритм коррекции насыщенности `apply_saturation(color: RgbColor, factor: f32) -> RgbColor` на базе Rec. 601 Luma
- [ ] 1.3 Добавить модульные тесты для `GammaLut` с индивидуальными коэффициентами усиления R/G/B, отсечкой черного и тесты для `apply_saturation`. Проверить через `cargo test -p reambibox-core`

## 2. Конфигурация и API сервера (`reambibox-app`)

- [ ] 2.1 Добавить поля `white_balance_r`, `white_balance_g`, `white_balance_b`, `black_level_threshold`, `saturation_boost` в `AppConfig` (`crates/reambibox-core/src/config/mod.rs`) со значениями по умолчанию
- [ ] 2.2 Добавить поля в структуры `ApiStatusResponse` и `ConfigUpdate` в `crates/reambibox-app/src/main.rs`, поддержать их сохранение в `config.json` и инкремент версии конфига
- [ ] 2.3 Обновить инициализацию `GammaLut` и конвейера обработки кадров в потоке захвата `main.rs` с передачей параметров калибровки

## 3. Пользовательский веб-интерфейс (`ui`)

- [ ] 3.1 Добавить в `ui/index.html` секцию «Калибровка цвета» с ползунками баланса белого (Красный, Зелёный, Синий), порога чёрного и насыщенности, а также кнопкой сброса
- [ ] 3.2 Добавить стили слайдеров калибровки (цветовые акценты R, G, B каналов) в `ui/style.css`
- [ ] 3.3 В `ui/app.js` реализовать чтение статуса, отображение значений бейджей, отправку изменений в `POST /api/config` и сброс к значениям по умолчанию

## 4. Верификация и сборка

- [ ] 4.1 Выполнить полный прогон тестов `cargo test` и убедиться в успешном прохождении всех тестов
- [ ] 4.2 Собрать релизный дистрибутив через `scripts/package.ps1`
- [ ] 4.3 Зафиксировать коммит в Git и заархивировать изменение OpenSpec

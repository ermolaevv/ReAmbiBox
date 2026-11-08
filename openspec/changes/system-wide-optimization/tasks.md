# Задачи реализации: system-wide-optimization

## 1. Структура `GammaLut` и оптимизация сглаживания (`reambibox-core::color`)
- [ ] 1.1 Создать `GammaLut` в `reambibox-core::color` с методами `new`, `correct_channel`, `correct_color`
- [ ] 1.2 Добавить метод `smooth_into` в `TemporalSmoother` для in-place сглаживания с применением `GammaLut`
- [ ] 1.3 Написать модульные тесты для `GammaLut` и `smooth_into`

## 2. Энергосбережение WASAPI аудио (`reambibox-core::audio`)
- [ ] 2.1 Добавить атомарный флаг `is_active_mode` / `enable_processing` в `WasapiLoopbackCapture`
- [ ] 2.2 Добавить метод `set_active(&self, active: bool)` для переключения между активным опросом (15 мс) и сном (150 мс)

## 3. Версионирование конфига и оптимизация потоков (`reambibox-app`)
- [ ] 3.1 Ввести `config_version: Arc<AtomicU64>` и инкрементировать при вызовах `POST /api/config` и хоткеях
- [ ] 3.2 Кэшировать `zones`, `lut`, `target_fps`, `mode` в потоке захвата на основе проверки версии
- [ ] 3.3 Управлять активностью `audio_capture.set_active(mode == "music")`
- [ ] 3.4 Использовать `smooth_into` с предварительно выделенным вектором `frame_output_buffer`

## 4. Конфигурация компилятора (`Cargo.toml`)
- [ ] 4.1 Добавить `[profile.release]` с `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`

## 5. Верификация и сборка
- [ ] 5.1 Прогнать `cargo test` по всем тестам
- [ ] 5.2 Собрать релиз через `pwsh -File scripts/package.ps1`
- [ ] 5.3 Архивировать изменение в OpenSpec и зафиксировать в Git

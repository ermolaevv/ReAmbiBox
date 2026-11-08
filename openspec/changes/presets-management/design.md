# Design: presets-management

## Context
В настоящее время `AppConfig` хранит плоский список параметров. При переходе от просмотра кино к играм или музыке пользователю приходится вручную двигать ползунки сглаживания, частоты кадров, насыщенности и гаммы. Настоящий дизайн вводит концепцию пресетов (`Preset`), разделяя настройки на глобальные аппаратные (COM-порт, геометрия зон, автозапуск) и профильные параметры отображения.

## Goals / Non-Goals

**Goals:**
- Введение структуры `Preset` со всеми ключевыми параметрами рендеринга.
- Автоматическая инициализация предустановленных заводских пресетов («Кино», «Игры», «Фоновый свет», «Музыка»).
- Возможность сохранять, перезаписывать и удалять пресеты через WebUI.
- Быстрое переключение через WebUI (кнопки/селектор в шапке панели) и меню трея Windows.
- Атомарное применение пресета с обновлением `config_version` без задержек и перезапуска.

**Non-Goals:**
- Привязка пресетов к запущенным .exe файлам процессов (автопрофилирование игр) — это задача отдельного будущего расширения.
- Дублирование геометрии зон внутри пресетов (зоны откалиброваны физически под экран и остаются общими).

## Decisions

### 1. Модель данных `Preset` в `reambibox-core`
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub is_builtin: bool,
    pub mode: String,
    pub brightness: f32,
    pub smoothing: f32,
    pub gamma: f32,
    pub saturation_boost: f32,
    pub white_balance_r: f32,
    pub white_balance_g: f32,
    pub white_balance_b: f32,
    pub black_level_threshold: u8,
    pub letterbox_detection: bool,
    pub target_fps: u32,
    pub static_color: [u8; 3],
    pub audio_effect: String,
    pub audio_sensitivity: f32,
    pub hotkey: Option<String>,
}
```

### 2. Заводские пресеты по умолчанию
- **Кино (Cinema)**: `mode: screen`, `gamma: 2.2`, `smoothing: 0.70`, `target_fps: 30`, `letterbox: true`, `black_level: 10`, `saturation: 1.1`.
- **Игры (Gaming)**: `mode: screen`, `gamma: 1.8`, `smoothing: 0.25`, `target_fps: 60`, `letterbox: false`, `black_level: 0`, `saturation: 1.25`.
- **Фоновый свет (Bias / Cozy)**: `mode: static`, `brightness: 0.6`, `static_color: [255, 190, 120]` (тёплый уютный свет).
- **Музыка (Party Music)**: `mode: music`, `audio_effect: pulse`, `audio_sensitivity: 1.2`, `brightness: 1.0`.

### 3. REST API маршруты
- `POST /api/presets/apply` с телом `{"id": "gaming"}` — находит пресет, копирует его поля в `AppConfig`, инкрементирует `config_version`, сохраняет `active_preset = Some("gaming")` и сохраняет конфиг.
- `POST /api/presets/save` с телом `{"name": "Мой пресет", "id": "optional_id"}` — сериализует текущие параметры конфига в пресет.
- `POST /api/presets/delete` с телом `{"id": "custom_1"}` — удаляет пользовательский пресет, блокируя удаление `is_builtin`.

### 4. Интеграция с UI
- В шапке WebUI появляется селектор пресетов в виде кнопок быстрого выбора («Кино», «Игры», «Фон», «Музыка») и кнопка «+ Сохранить как пресет».
- В трее Windows создается подменю `Пресеты` -> с перечнем всех пресетов и галочкой `MF_CHECKED` на активном.

## Risks / Trade-offs

- **[Risk]** Пользователь может случайно перезаписать встроенные пресеты.
  → *Mitigation*: Флаг `is_builtin: true` запрещает удаление, а в UI отображается бейдж «Встроенный».

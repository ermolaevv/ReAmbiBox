<!-- RULE: Capability-First Execution Check -->
# ПРАВИЛО: ПРЕДВАРИТЕЛЬНЫЙ ВЫБОР ИНСТРУМЕНТОВ, НАВЫКОВ И MCP (CAPABILITY-FIRST DISPATCH)

> **ГЛАВНЫЙ ИМПЕРАТИВ**: Перед тем как выполнять любое действие (поиск в коде, архитектурные выводы, написание кода, рефакторинг, отладка ошибок компилятора, сборка или запуск тестов), агент ОБЯЗАН сначала свериться с данным реестром и активировать профильный навык или инструмент. Запрещено использовать сырые эвристики и grep, если для задачи существует специализированный инструмент.

---

## 1. RUST-СПЕЦИАЛИЗАЦИЯ И ЭКСПЕРТИЗА (RUST MASTER SERIES)

Используются при любых вопросах, архитектуре или ошибках на Rust:

### Диспетчер и аудит качества:
* `rust-router`: **НАИВЫСШИЙ ПРИОРИТЕТ** для любых задач на Rust. Анализ ошибок компилятора, архитектурные сравнения (tokio vs async-std), выбор паттернов.
* `coding-guidelines`: Стандарты оформления, Rust style guide, соглашения об именовании (P.NAM, G.FMT), правила для Clippy и rustfmt.
* `unsafe-checker`: **КРИТИЧЕСКИЙ АУДИТ**: проверка любого блока `unsafe`, FFI, сырых указателей (`*const`, `*mut`), `repr(C)`, mem layout, предотвращение Undefined Behavior (UB).
* `meta-cognition-parallel`: Трёхуровневый параллельный метакогнитивный анализ сложных проблем (`/meta-parallel`).

### Экспертные модули компилятора и семантики:
* `m01-ownership`: Ошибки владения, заимствования и времен жизни (E0382, E0597, E0506, E0507, E0515, 'static, Copy vs Clone).
* `m02-resource`: Умные указатели и управление памятью в куче (Box, Rc, Arc, Weak, RefCell, Cell, RAII, Drop).
* `m03-mutability`: Конфликты заимствования и внутренняя мутабельность (E0596, E0499, E0502, Mutex, RwLock).
* `m04-zero-cost`: Дженерики, трейты, мономорфизация, dynamic vs static dispatch (E0277, E0308, dyn, impl Trait).
* `m05-type-driven`: Type state pattern, PhantomData, newtype, compile-time validation, sealed traits, ZST.
* `m06-error-handling`: Стратегии обработки ошибок (Result, Option, ?, anyhow, thiserror, panic vs Result).
* `m07-concurrency`: Асинхронность и многопоточность (Send, Sync, Tokio, каналы mpsc/crossbeam, data race, deadlocks).
* `m09-domain`: Предметное моделирование на Rust (DDD, Entity, Value Object, инварианты бизнес-логики).
* `m10-performance`: Оптимизация быстродействия, бенчмарки (criterion, flamegraph, SIMD, аллокации, кэш).
* `m11-ecosystem`: Выбор крейтов из crates.io, настройка Cargo.toml, feature flags, workspace.
* `m12-lifecycle`: Жизненный цикл соединений и ресурсов, RAII guard, OnceLock, Lazy инициализация.
* `m13-domain-error`: Доменные и транзиентные ошибки, стратегии Retry, Circuit Breaker, Graceful Degradation.
* `m14-mental-model`: Ментальные модели и визуализация концепций владения и работы памяти для интуиции.
* `m15-anti-pattern`: Код-ревью на антипаттерны, неоправданный `.clone()`, `.unwrap()` в проде, запахи кода.

### LSP-инструменты анализа и навигации Rust:
* `rust-code-navigator`: Навигация по коду (Go to definition, Find references).
* `rust-call-graph`: Визуализация графов вызовов функций (`/call-graph`).
* `rust-symbol-analyzer`: Анализ структуры модулей, перечисление структур, трейтов и методов (`/symbols`).
* `rust-trait-explorer`: Исследование реализаций трейтов (`/trait-impl`).
* `rust-refactor-helper`: Безопасный рефакторинг с LSP (переименование, извлечение функций).
* `rust-deps-visualizer`: ASCII-визуализация дерева зависимостей Cargo (`/deps-viz`).
* `rust-learner`: Информация о версиях Rust, редакциях (2021/2024), changelog и документация API.
* `rust-daily`: Дайджесты и новости экосистемы (TWIR, Rust blog, релизы).
* `rust-skill-creator`: Генерация динамических навыков из документации новых крейтов.

---

## 2. ДОМЕННЫЕ НАВЫКИ (DOMAIN CRATES & ARCHITECTURES)

* `domain-cli`: Разработка консольных утилит (clap, ratatui, crossterm, indicatif, shell completions).
* `domain-embedded`: Встроенные системы и no_std (HAL, PAC, RTIC, embassy, GPIO, SPI, I2C, UART).
* `domain-iot`: Интернет вещей и телеметрия (MQTT, сенсоры, шлюзы, бинарные протоколы).
* `domain-web`: Веб-серверы и API (axum, actix, tower, hyper, reqwest, WebSocket, JWT).
* `domain-cloud-native`: Облачные сервисы (Docker, Kubernetes, gRPC Tonic, Prometheus metrics, tracing).
* `domain-ml`: Машинное обучение и тензоры (burn, candle, ndarray, tch-rs).
* `domain-fintech`: Финтех, высокоточные десятичные расчеты (decimal), транзакции, реестры.

---

## 3. WINDOWS И WINUI РАЗРАБОТКА (WINAPP)

* `winapp-find-api`: Поиск настоящих API Windows SDK / WinRT / WinApp SDK без гаданий.
* `winapp-find-ui`: Подбор контролов WinUI 3 и компилируемых примеров из WinUI Gallery.
* `winapp-frameworks`: Интеграция Windows-фич в Rust, Tauri, C++, Electron, Flutter, .NET.
* `winapp-identity`: Назначение Package Identity обычным exe для системных Windows API.
* `winapp-manifest`: Редактирование Package.appxmanifest и манифестов запуска.
* `winapp-maui`: Сборка и упаковка .NET MAUI.
* `winapp-package`: Сборка MSIX инсталлятора из любого проекта (.NET, C++, Rust, Tauri).
* `winapp-signing`: Управление сертификатами разработчика и подпись бинарников (`winapp sign`).
* `winapp-troubleshoot`: Устранение ошибок 0x80073CFx, сбоев манифестов и сертификатов.
* `winapp-ui-automation`: Инспекция, клики, ввод текста и скриншоты живого UI через UI Automation.
* `winui-devtools`: Исследование XAML-дерева в рантайме (стили, байндинги, live-правки).
* `winapp-performance`: Анализ лагов интерфейса через ETW-трейсы и задержки GC.
* `winapp-sandbox`: Запуск, изоляция и тестирование софта в чистом Windows Sandbox.
* `winapp-setup`: Инициализация Windows SDK / Windows App SDK (`winapp init`, `winapp restore`).

---

## 4. OPENSPEC (СПЕЦИФИКАЦИЯ И УПРАВЛЕНИЕ ИЗМЕНЕНИЯМИ)

* `openspec-explore`: Исследование идей и анализ архитектуры (режим мышления).
* `openspec-propose`: Создание полного пакета спецификаций, дизайна и задач в один шаг.
* `openspec-apply-change`: Пошаговая реализация задач из утвержденного изменения.
* `openspec-update-change`: Обновление артефактов планирования при изменении вводных.
* `openspec-sync-specs`: Синхронизация дельта-спецификаций в основной реестр спецификаций.
* `openspec-archive-change`: Архивирование выполненных изменений.

---

## 5. АНАЛИЗ КОДОВОЙ БАЗЫ И РЕВЕРС-ИНЖИНИРИНГ

### MCP: `codebase-memory-mcp`
Инструменты графа проекта: `search_graph`, `trace_path`, `get_code_snippet`, `check_index_coverage`, `query_graph`, `get_architecture`, `get_file_outline`, `compare_graphs`, `search_code`, `list_projects`, `index_status`, `index_repository`, `detect_changes`, `manage_adr`, `ingest_traces`.

### Навык и MCP: `reverse-engineer-anything` (REA)
Глубокий статический и динамический анализ бинарников:
* **Статика**: `binary_overview`, `procedure_assembly`, `procedure_pseudo_code`, `batch_decompile`, `search_bytes`, `search_strings`, `xrefs`, `get_call_graph`.
* **Форматы**: `inspect_pe_imports`, `inspect_pe_resources`, `inspect_macho`, `inspect_rust_binary`, `inspect_go_binary`, `inspect_wasm_artifact`.
* **Динамика & Отладка**: `start_gdb_session`, `start_rizin_debug_session`, `instrument_with_frida`, `observe_native_calls`.
* **Android/Web/Сеть**: `inspect_adb_device`, `inspect_android_package`, `inspect_pcap_capture`, `inspect_electron_page`.

### Специализированные платформы:
* `yandex-cucumber-hardware`: Регистры, ALSA, I2C, Amlogic A5 Yandex Station Midi.
* `station-web-ui`: Веб-интерфейс и WebSocket протоколы Yandex Station Midi.

---

## 6. КАСТОМИЗАЦИЯ И ИНТЕРФЕЙС ANTIGRAVITY

* `agy-customizations`: Создание skills, rules, plugins, hooks и MCP-серверов.
* `antigravity-guide`: Документация по AGY CLI, IDE, SDK и slash-командам.
* `plugin`: Управление плагинами (установка, включение, сборка).
* `generative_ui`: Рендеринг интерактивных HTML-виджетов и дашбордов в чате.
* `automation`: Проектирование периодических и фоновых автоматизаций.
* `migrate-agent-settings`: Перенос конфигов из Cursor, Claude Code, Codex.
* `migrate-workflows`: Конвертация устаревших workflow-файлов в скиллы.
* `ui-plugin-navigation`: Вызов навигационных плашек UI-плагинов.
* `core-actionbook`, `core-agent-browser`, `core-dynamic-skills`, `core-fix-skill-docs`: Служебные навыки агента.

---

## 7. АЛГОРИТМ ПРИНЯТИЯ РЕШЕНИЙ (DECISION TREE)

```
[Поступила задача или ошибка]
               │
               ▼
 1. Ошибка компилятора Rust, лайфтаймы, трейты или unsafe?
    ├── ДА  ➔ rust-router -> профильный m01..m15 / unsafe-checker / coding-guidelines
    └── НЕТ ➔ Перейти к п. 2
               │
               ▼
 2. Навигация по коду, вызовы функций, зависимости в Rust?
    ├── ДА  ➔ rust-code-navigator / rust-call-graph / rust-deps-visualizer
    └── НЕТ ➔ Перейти к п. 3
               │
               ▼
 3. Исследование глобальной архитектуры кодовой базы?
    ├── ДА  ➔ codebase-memory-mcp (search_graph, trace_path)
    └── НЕТ ➔ Перейти к п. 4
               │
               ▼
 4. Windows, WinUI, MSIX, Sandbox или автоматизация GUI?
    ├── ДА  ➔ Навыки winapp-* и winui-*
    └── НЕТ ➔ Перейти к п. 5
               │
               ▼
 5. Реверс стороннего софта, PE-дампы, сетевые пакеты?
    ├── ДА  ➔ reverse-engineer-anything + MCP rea
    └── НЕТ ➔ Перейти к п. 6
               │
               ▼
 6. Проектирование архитектуры или реализация OpenSpec?
    ├── ДА  ➔ openspec-explore / openspec-propose / openspec-apply-change
    └── НЕТ ➔ Базовые инструменты окружения
```


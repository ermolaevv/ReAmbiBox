# Архитектурный дизайн: Оптимизация захвата экрана и выборки цветов

## 1. Zero-Allocation конвейер захвата (`FrameView`)

### Текущее состояние:
```rust
// Каждый кадр (60 раз в секунду):
let total_bytes = (row_pitch * height) as usize; // 33 МБ на 4K
let mut buffer = vec![0u8; total_bytes]; // Аллокация и зануление 33 МБ!
std::ptr::copy_nonoverlapping(mapped.pData as *const u8, buffer.as_mut_ptr(), total_bytes); // memcpy 33 МБ!
// drop(buffer) - освобождение 33 МБ!
```

### Новая архитектура:
```rust
pub struct FrameView<'a> {
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub data: &'a [u8],
}

impl DxgiCapture {
    pub fn capture_frame_with<R>(
        &mut self,
        timeout: Duration,
        f: impl FnOnce(&FrameView) -> R,
    ) -> Result<R, CaptureError> {
        // 1. AcquireNextFrame
        // 2. CopyResource(&staging, &desktop_texture)
        // 3. Map(&staging, 0, D3D11_MAP_READ, ...)
        // 4. let slice = unsafe { std::slice::from_raw_parts(mapped.pData as *const u8, total_bytes) };
        // 5. let res = f(&FrameView { width, height, row_pitch, data: slice });
        // 6. Unmap & ReleaseFrame
        // 7. Ok(res)
    }
}
```
**Результат**: 0 аллокаций памяти, 0 байт промежуточного копирования полного экрана в RAM, 0 системных вызовов mmap/VirtualAlloc.

## 2. Адаптивный расчет шага выборки (Adaptive Subsampling)
Вместо жестко закодированного `step = 4`:
Для прямоугольника зоны размером `(pw, ph)` в пикселях:
```rust
// Оптимальное количество точек по каждой оси: от 8 до 16
let step_x = (pw / 16).max(1);
let step_y = (ph / 16).max(1);
let step = step_x.max(step_y).max(2);
```
Кроме того, чтение пикселей оптимизируется чтением 32-битного значения `u32`:
```rust
let px = unsafe { *(data.as_ptr().add(pixel_offset) as *const u32) };
let b = (px & 0xFF) as u64;
let g = ((px >> 8) & 0xFF) as u64;
let r = ((px >> 16) & 0xFF) as u64;
```
Это дает векторную загрузку одной машинной инструкцией и устраняет множественные проверки границ в цикле.

## 3. Точный тайминг частоты кадров (Target FPS Throttling)
Интервал между кадрами захвата рассчитывается динамически:
```rust
let target_fps = cfg.target_fps.clamp(10, 120);
let target_interval = Duration::from_millis(1000 / (target_fps as u64));
```
Если захват и обработка заняли 3 мс, а `target_fps = 30`, поток спит оставшиеся 30 мс, не потребляя ресурсы CPU и видеокарты.

## 4. Детектор Letterbox с интервальным кэшированием
Проверка наличия черных полос киноформата (21:9) выполняется 1 раз в 30 кадров (~1 Гц) и только в случае включенного флага `letterbox_detection`. Полученный `viewport` кэшируется и применяется к координатам зон при выборке.

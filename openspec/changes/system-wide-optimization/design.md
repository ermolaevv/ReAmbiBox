# Архитектурный дизайн: Системная оптимизация конвейера

## 1. Таблица гамма-коррекции `GammaLut`
```rust
#[derive(Debug, Clone, Copy)]
pub struct GammaLut {
    table: [u8; 256],
}

impl GammaLut {
    pub fn new(brightness: f32, gamma: f32) -> Self {
        let b = brightness.clamp(0.0, 1.0);
        let g = if gamma > 0.1 { gamma } else { 1.0 };
        let mut table = [0u8; 256];
        for i in 0..=255 {
            let normalized = (i as f32 / 255.0) * b;
            let corrected = normalized.powf(g).clamp(0.0, 1.0);
            table[i] = (corrected * 255.0).round() as u8;
        }
        Self { table }
    }

    #[inline(always)]
    pub fn correct_channel(&self, val: u8) -> u8 {
        self.table[val as usize]
    }

    #[inline(always)]
    pub fn correct_color(&self, color: RgbColor) -> RgbColor {
        RgbColor::new(
            self.table[color.r as usize],
            self.table[color.g as usize],
            self.table[color.b as usize],
        )
    }
}
```
256 байт целиком помещаются в 4 строки кэша L1 процессора (64 байта каждая).
Время выборки: ~1 такт процессора вместо ~30-40 тактов на вызов `powf`.

## 2. In-Place сглаживание в `TemporalSmoother`
```rust
impl TemporalSmoother {
    pub fn smooth_into(
        &mut self,
        targets: &[RgbColor],
        factor: f32,
        lut: &GammaLut,
        out: &mut Vec<RgbColor>,
    ) {
        let factor = factor.clamp(0.0, 0.99);
        out.clear();
        out.reserve(targets.len());

        if self.current_colors.len() != targets.len() {
            self.current_colors = targets.iter().map(|c| [c.r as f32, c.g as f32, c.b as f32]).collect();
            for t in targets {
                out.push(lut.correct_color(*t));
            }
            return;
        }

        for (curr, target) in self.current_colors.iter_mut().zip(targets.iter()) {
            let tr = target.r as f32;
            let tg = target.g as f32;
            let tb = target.b as f32;

            curr[0] = curr[0] * factor + tr * (1.0 - factor);
            curr[1] = curr[1] * factor + tg * (1.0 - factor);
            curr[2] = curr[2] * factor + tb * (1.0 - factor);

            let raw = RgbColor::new(
                curr[0].round() as u8,
                curr[1].round() as u8,
                curr[2].round() as u8,
            );
            out.push(lut.correct_color(raw));
        }
    }
}
```
Объединяет временное сглаживание и гамма-коррекцию в один проход, исключая 2 аллокации векторов на кадр.

## 3. Атомарное версионирование конфигурации (`AtomicU64`)
```rust
let config_version = Arc::new(AtomicU64::new(1));
```
При каждом изменении конфигурации через API или хоткеи:
```rust
config_version.fetch_add(1, Ordering::Release);
```
В потоках захвата и вывода:
Проверяем `if local_version != current_version`. Только при несовпадении обновляются локальные клоны `zones`, `mode`, `lut`.
При совпадении — 0 блокировок чтения, 0 аллокаций строк и векторов!

## 4. Сон аудио-захвата WASAPI при `mode != "music"`
В цикле `WasapiLoopbackCapture`:
Проверяем атомарный флаг потребности аудио (`needs_audio: Arc<AtomicBool>`).
Если `false`, поток делает `sleep(Duration::from_millis(150))`, высвобождая ресурсы CPU.

## 5. Профиль релиза `Cargo.toml`
```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
strip = true
```

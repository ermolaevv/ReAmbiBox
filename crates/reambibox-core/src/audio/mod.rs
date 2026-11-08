use crate::device::types::RgbColor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};

/// Анализ частотных полос звука
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioBands {
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub energy: f32,
}

impl AudioBands {
    /// Анализ буфера аудиосэмплов (32-bit float PCM)
    pub fn from_samples(samples: &[f32], channels: usize) -> Self {
        if samples.is_empty() || channels == 0 {
            return Self::default();
        }

        // Преобразуем многоканальный звук в моно
        let mono: Vec<f32> = samples
            .chunks_exact(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect();

        if mono.is_empty() {
            return Self::default();
        }

        // 1. Общая энергия (RMS)
        let sum_sq: f32 = mono.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / mono.len() as f32).sqrt().min(1.0);

        // 2. Разделение частот (эмуляция полосовых фильтров через дифференциальные переходы)
        let mut bass_acc = 0.0f32;
        let mut mids_acc = 0.0f32;
        let mut treble_acc = 0.0f32;

        let mut low_prev = 0.0f32;
        let mut mid_prev = 0.0f32;

        for &s in &mono {
            // Low-pass фильтр (~250 Гц)
            let low = low_prev + 0.05 * (s - low_prev);
            low_prev = low;
            bass_acc += low * low;

            // Band-pass фильтр (~250-2500 Гц)
            let mid = mid_prev + 0.25 * ((s - low) - mid_prev);
            mid_prev = mid;
            mids_acc += mid * mid;

            // High-pass фильтр (>2500 Гц)
            let high = s - low - mid;
            treble_acc += high * high;
        }

        let n = mono.len() as f32;
        let bass = (bass_acc / n).sqrt().min(1.0);
        let mids = (mids_acc / n).sqrt().min(1.0);
        let treble = (treble_acc / n).sqrt().min(1.0);

        Self {
            bass,
            mids,
            treble,
            energy: rms,
        }
    }
}

/// Генератор светомузыкальных кадров
pub struct MusicVisualizer {
    smoothed_bands: AudioBands,
    phase: f32,
}

impl MusicVisualizer {
    pub fn new() -> Self {
        Self {
            smoothed_bands: AudioBands::default(),
            phase: 0.0,
        }
    }

    /// Обновление состояния и генерация кадра диодов
    pub fn render(
        &mut self,
        raw_bands: AudioBands,
        led_count: usize,
        effect: &str,
        sensitivity: f32,
    ) -> Vec<RgbColor> {
        let sens = sensitivity.clamp(0.2, 5.0);

        // Экспоненциальное сглаживание
        let alpha = 0.35f32;
        self.smoothed_bands.bass = self.smoothed_bands.bass * (1.0 - alpha) + (raw_bands.bass * sens).min(1.0) * alpha;
        self.smoothed_bands.mids = self.smoothed_bands.mids * (1.0 - alpha) + (raw_bands.mids * sens).min(1.0) * alpha;
        self.smoothed_bands.treble = self.smoothed_bands.treble * (1.0 - alpha) + (raw_bands.treble * sens).min(1.0) * alpha;
        self.smoothed_bands.energy = self.smoothed_bands.energy * (1.0 - alpha) + (raw_bands.energy * sens).min(1.0) * alpha;

        self.phase = (self.phase + 0.05 + self.smoothed_bands.bass * 0.1) % (2.0 * std::f32::consts::PI);

        match effect {
            "spectrum" => self.render_spectrum(led_count),
            "wave" => self.render_wave(led_count),
            _ => self.render_pulse(led_count), // По умолчанию "pulse"
        }
    }

    /// Эффект 1: Пульсация под бит (Beat Pulse)
    fn render_pulse(&self, count: usize) -> Vec<RgbColor> {
        let b = self.smoothed_bands;
        let hue = (self.phase / (2.0 * std::f32::consts::PI)) * 360.0;
        let base_rgb = hsv_to_rgb(hue, 0.9, (b.energy * 1.5).min(1.0));

        // Басовый акцент: добавляет вспышку
        let bass_boost = (b.bass * 2.2).min(1.0);
        let r = ((base_rgb.r as f32 * (1.0 - bass_boost * 0.4) + 255.0 * bass_boost).min(255.0)) as u8;
        let g = ((base_rgb.g as f32 * (1.0 - bass_boost * 0.7)).min(255.0)) as u8;
        let b_val = ((base_rgb.b as f32 + 200.0 * b.treble).min(255.0)) as u8;

        let col = RgbColor::new(r, g, b_val);
        vec![col; count]
    }

    /// Эффект 2: Спектральный эквалайзер (VU-Meter по периметру)
    fn render_spectrum(&self, count: usize) -> Vec<RgbColor> {
        let half = (count / 2).max(1);
        let mut leds = vec![RgbColor::BLACK; count];

        let b = self.smoothed_bands;
        let fill_level = (b.energy * half as f32 * 1.4).round() as usize;

        for i in 0..half {
            if i < fill_level {
                let frac = i as f32 / half as f32;
                // Градиент от зеленого (низ) через желтый к красному (верх)
                let color = if frac < 0.6 {
                    RgbColor::new(0, (255.0 * (1.0 - frac * 0.5)) as u8, (255.0 * frac) as u8)
                } else if frac < 0.85 {
                    RgbColor::new(255, 200, 0)
                } else {
                    RgbColor::new(255, 30, 30)
                };

                leds[i] = color;
                if count > 1 + i {
                    leds[count - 1 - i] = color;
                }
            }
        }
        leds
    }

    /// Эффект 3: Бегущая звуковая волна (Color Wave)
    fn render_wave(&self, count: usize) -> Vec<RgbColor> {
        let mut leds = Vec::with_capacity(count);
        let b = self.smoothed_bands;

        for i in 0..count {
            let offset = (i as f32 / count as f32) * 2.0 * std::f32::consts::PI;
            let val = (self.phase * 2.0 + offset).sin() * 0.5 + 0.5;
            let hue = ((self.phase * 50.0 + i as f32 * 3.0) % 360.0).abs();
            let brightness = (val * (0.3 + b.energy * 0.7)).min(1.0);
            leds.push(hsv_to_rgb(hue, 0.95, brightness));
        }
        leds
    }
}

/// Преобразование HSV в RGB
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> RgbColor {
    let c = v * s;
    let h_prime = (h / 60.0) % 6.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if (0.0..1.0).contains(&h_prime) {
        (c, x, 0.0)
    } else if (1.0..2.0).contains(&h_prime) {
        (x, c, 0.0)
    } else if (2.0..3.0).contains(&h_prime) {
        (0.0, c, x)
    } else if (3.0..4.0).contains(&h_prime) {
        (0.0, x, c)
    } else if (4.0..5.0).contains(&h_prime) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    RgbColor::new(
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Высокопроизводительный захват звука Windows через WASAPI Loopback
pub struct WasapiLoopbackCapture {
    is_running: Arc<AtomicBool>,
}

impl WasapiLoopbackCapture {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Запуск фонового потока захвата системного звука
    pub fn start_capture<F>(&self, callback: F)
    where
        F: Fn(AudioBands) + Send + 'static,
    {
        let is_running = Arc::clone(&self.is_running);
        is_running.store(true, Ordering::Relaxed);

        std::thread::spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("[Audio] Ошибка создания MMDeviceEnumerator: {:?}", err);
                    is_running.store(false, Ordering::Relaxed);
                    CoUninitialize();
                    return;
                }
            };

            let device = match enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                Ok(d) => d,
                Err(err) => {
                    eprintln!("[Audio] Устройство воспроизведения по умолчанию не найдено: {:?}", err);
                    is_running.store(false, Ordering::Relaxed);
                    CoUninitialize();
                    return;
                }
            };

            let audio_client: IAudioClient = match device.Activate(CLSCTX_ALL, None) {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("[Audio] Ошибка активации IAudioClient: {:?}", err);
                    is_running.store(false, Ordering::Relaxed);
                    CoUninitialize();
                    return;
                }
            };

            let pwfx = match audio_client.GetMixFormat() {
                Ok(f) => f,
                Err(err) => {
                    eprintln!("[Audio] Не удалось получить MixFormat: {:?}", err);
                    is_running.store(false, Ordering::Relaxed);
                    CoUninitialize();
                    return;
                }
            };

            let wave_format = &*pwfx;
            let channels = wave_format.nChannels as usize;
            let bits_per_sample = wave_format.wBitsPerSample;

            let is_float = if wave_format.wFormatTag == 0xFFFE {
                let ext = &*(pwfx as *const WAVEFORMATEXTENSIBLE);
                ext.SubFormat.data1 == 3 // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                wave_format.wFormatTag == 3 // WAVE_FORMAT_IEEE_FLOAT
            };

            if let Err(err) = audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                10_000_000, // 1 секунда буфера
                0,
                pwfx,
                None,
            ) {
                eprintln!("[Audio] Ошибка инициализации WASAPI Loopback: {:?}", err);
                is_running.store(false, Ordering::Relaxed);
                CoUninitialize();
                return;
            }

            let capture_client: IAudioCaptureClient = match audio_client.GetService() {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("[Audio] Ошибка получения IAudioCaptureClient: {:?}", err);
                    is_running.store(false, Ordering::Relaxed);
                    CoUninitialize();
                    return;
                }
            };

            if let Err(err) = audio_client.Start() {
                eprintln!("[Audio] Ошибка запуска аудиопотока: {:?}", err);
                is_running.store(false, Ordering::Relaxed);
                CoUninitialize();
                return;
            }

            println!("[Audio] Захват системного звука WASAPI Loopback активен ({} каналов, {} бит, float={})", channels, bits_per_sample, is_float);

            let mut sample_buf = Vec::<f32>::with_capacity(2048);

            while is_running.load(Ordering::Relaxed) {
                let mut p_data = std::ptr::null_mut();
                let mut num_frames = 0u32;
                let mut flags = 0u32;

                while let Ok(()) = capture_client.GetBuffer(&mut p_data, &mut num_frames, &mut flags, None, None) {
                    if num_frames == 0 {
                        break;
                    }

                    if flags & 1 != 0 {
                        // Флаг тишины (AUDCLNT_BUFFERFLAGS_SILENT)
                        sample_buf.clear();
                    } else if is_float && bits_per_sample == 32 && !p_data.is_null() {
                        let total_samples = (num_frames as usize) * channels;
                        let slice = std::slice::from_raw_parts(p_data as *const f32, total_samples);
                        sample_buf.extend_from_slice(slice);
                    } else if bits_per_sample == 16 && !p_data.is_null() {
                        let total_samples = (num_frames as usize) * channels;
                        let slice = std::slice::from_raw_parts(p_data as *const i16, total_samples);
                        sample_buf.extend(slice.iter().map(|&s| s as f32 / 32768.0));
                    }

                    let _ = capture_client.ReleaseBuffer(num_frames);

                    if sample_buf.len() >= 512 * channels {
                        let bands = AudioBands::from_samples(&sample_buf, channels);
                        callback(bands);
                        sample_buf.clear();
                    }
                }

                std::thread::sleep(Duration::from_millis(15));
            }

            let _ = audio_client.Stop();
            println!("[Audio] Захват системного звука WASAPI Loopback остановлен");
            CoUninitialize();
        });
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_bands_from_silence() {
        let samples = vec![0.0f32; 1024];
        let bands = AudioBands::from_samples(&samples, 2);
        assert_eq!(bands.energy, 0.0);
        assert_eq!(bands.bass, 0.0);
    }

    #[test]
    fn test_audio_bands_from_sine() {
        let mut samples = Vec::new();
        for i in 0..1024 {
            let t = i as f32 / 44100.0;
            // 60 Hz басовый синус
            let s = (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.8;
            samples.push(s);
            samples.push(s); // 2 канала
        }
        let bands = AudioBands::from_samples(&samples, 2);
        assert!(bands.energy > 0.3);
        assert!(bands.bass > 0.2);
    }

    #[test]
    fn test_music_visualizer_renders_correct_length() {
        let mut vis = MusicVisualizer::new();
        let bands = AudioBands {
            bass: 0.8,
            mids: 0.5,
            treble: 0.2,
            energy: 0.7,
        };
        let frame_pulse = vis.render(bands, 98, "pulse", 1.0);
        assert_eq!(frame_pulse.len(), 98);

        let frame_spectrum = vis.render(bands, 98, "spectrum", 1.0);
        assert_eq!(frame_spectrum.len(), 98);

        let frame_wave = vis.render(bands, 98, "wave", 1.0);
        assert_eq!(frame_wave.len(), 98);
    }
}

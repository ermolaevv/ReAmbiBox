# ReAmbiBox

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6.svg?logo=windows)](https://github.com/Deus/ReAmbiBox)
[![Language](https://img.shields.io/badge/Language-Rust%202021-DEA584.svg?logo=rust)](https://www.rust-lang.org/)
[![Tested on](https://img.shields.io/badge/Tested%20on-Arduino%20(Adalight)%20%7C%2098%20LEDs-brightgreen.svg)]()

**A lightweight, blazing-fast ambient lighting (Ambilight) engine for modern Windows.**  
Rewritten from scratch in Rust, inspired by the legendary **AmbiBox**.

[English](README.md) • [Русский](README.ru.md)

</div>

---

## 💡 The Story & Inspiration

Back in the day, **AmbiBox** (version 2.1.7, originally created by Konstantin Granitsa) was the gold standard for DIY ambient PC lighting. It was simple, reliable, and brought desktop setups to life.

However, AmbiBox received its last update in **2015**. Over the last decade, Windows evolved: modern WDDM display drivers, HDR, high refresh rates, and Windows 10/11 broke older screen capture hooks, leading to micro-stutters, CPU lag, and connection drops.

**ReAmbiBox** was born as a modern, spiritual successor to AmbiBox. We took the beloved core concepts — perimeter zone capture, Adalight protocol compatibility, and Lightpack API integration — and reimagined them on modern technology:
- **Written in Rust** with memory safety, sub-millisecond execution, and zero garbage collection pauses.
- **DirectX 11 DXGI Desktop Duplication**: grabs frames straight from the GPU with zero-allocation buffers, keeping CPU usage near 0% even at 60+ FPS.
- **Clean Web Control Center**: no clunky WinForms or outdated panels — configure everything in a modern, responsive browser UI from any device.

---

## ⚠️ Hardware Notice (Tested Setup)

> [!IMPORTANT]
> **ReAmbiBox was developed and tested exclusively on the author's personal hardware:**
> - **OS**: Windows 11 Pro (x64)
> - **Controller**: Arduino Nano / Uno compatible board running the standard **Adalight** firmware
> - **Connection**: USB Virtual COM-port (`COM10`, `115200` baud)
> - **LED Strip**: 98 addressable WS2812B LEDs placed along the monitor perimeter (26 top, 25 bottom, 24 left, 23 right, clockwise from bottom-left)
>
> Support for **WLED** (network UDP via DDP & WARLS), higher baud rates (up to 2,000,000 baud), and alternative zone counts was engineered strictly according to protocol specifications. If you have different hardware, your feedback, bug reports, and contributions are very welcome!

---

## ✨ What ReAmbiBox Does

### 🎮 Smooth, Real-Time Ambilight (60+ FPS)
Instead of slow GDI screenshots, ReAmbiBox hooks directly into the DirectX 11 Desktop Duplication subsystem. It samples edge regions in hardware and updates your LEDs with imperceptible latency — perfect for fast-paced shooters, movies, and desktop work.

### 🎬 Smart Movie Watching (21:9 Letterbox Detection)
Watching widescreen cinema on a 16:9 monitor? ReAmbiBox automatically detects black bars and shifts its color sampling directly into the active movie area, preventing dark gaps or false colors.

### 🛋️ Room & Eye Comfort (Color Calibration)
- **Gamma Correction (sRGB 2.2 LUT)**: Natural color graduation matched to human eye perception.
- **White Balance**: Tune Red, Green, and Blue gains so the light matches your wall's paint tone.
- **Black Level Threshold**: Intelligently dims or shuts off LEDs during dark movie scenes or horror games, eliminating blinding glare when the screen is dark.
- **Saturation Boost**: Pops vivid, rich colors on your wall even when desktop elements look pale.
- **Temporal Smoothing**: Smooths transitions so colors flow like liquid light without distracting strobe effects.

### 🎶 Sound to Light (WASAPI Audio Reactive)
Turn your desk into a music visualizer. Using low-latency Windows audio loopback capture (no virtual cables required), ReAmbiBox analyzes live sound frequencies into:
- **Beat Pulse**: Rhythmic energy pulses with bass hits.
- **VU Spectrum**: Classic dynamic frequency equalizer.
- **Color Wave**: Traveling wave of colors synced to your music tempo.

### 🌐 Clean Browser Control Center (`http://localhost:3637`)
Everything is managed through a lightweight web interface:
- **Perimeter Wizard**: Set up your LED count per edge in seconds.
- **Interactive Drag-and-Drop Editor**: Visually fine-tune, resize, and snap zone boxes directly over your monitor preview.
- **Presets**: Save separate profiles for Gaming, Cinema, Evening Reading, or Party.
- **Instant Language Toggle**: Switch between **English** and **Русский** with one click.

### ⌨️ Global Windows Hotkeys
Adjust lighting without alt-tabbing out of a full-screen game:
- `Ctrl + Shift + A` — Toggle lights On / Off
- `Ctrl + Shift + Up / Down` — Brightness ±10%
- `Ctrl + Shift + O` — Show on-screen zone overlay

### 💡 Lightpack & Smart Home Friendly
Includes a built-in TCP server on port `3636` implementing the standard Lightpack protocol. Works out of the box with **Kodi**, **Home Assistant**, and legacy **AmbiBox** automation scripts.

---

## 🚀 Quick Start Guide

1. **Download** the latest release from the [Releases](https://github.com/Deus/ReAmbiBox/releases) page:
   - `ReAmbiBox-Portable-v0.1.0.zip` (portable, no install needed)
   - Or `ReAmbiBox-Setup-v0.1.0.exe` (installer with autostart)
2. **Run `reambibox.exe`**. The app silently starts and docks into your System Tray (near the clock).
3. **Open the Control Panel**: In your browser, open `http://localhost:3637`.
4. **Select Your Hardware**:
   - In the **Hardware** tab, pick your Arduino COM-port and baud rate.
5. **Generate Zones**:
   - In the **Zone Editor** tab, click **Perimeter Zone Wizard...**, enter your LED counts (e.g., 26 top, 25 bottom, 24 left, 23 right), and click **Apply to Screen**.
6. That's it! Your ambient lighting is active.

---

## 🛠️ Building from Source

If you prefer building ReAmbiBox yourself:

```powershell
# Prerequisites: Rust 1.75+ (MSVC toolchain) and Git on Windows 10/11 x64
git clone https://github.com/Deus/ReAmbiBox.git
cd ReAmbiBox

# Run automated tests
cargo test --workspace

# Build optimized release binary
cargo build --release --bin reambibox

# Package standalone zip distribution (PowerShell)
.\scripts\package.ps1
```

The resulting standalone executable will be at `target\release\reambibox.exe`. All web UI files, tray icons, and localization dictionaries are embedded directly into the binary at compile time.

---

## 🤝 Tribute & Acknowledgments

- **AmbiBox**: Heartfelt appreciation to **Konstantin Granitsa** and the original AmbiBox team (2004–2015) for inspiring an entire generation of DIY ambient lighting projects.
- **Adalight**: Special thanks to **Adafruit** and the open-source community for the classic serial LED streaming protocol.

---

## 📄 License

Distributed under the **MIT License**. See [LICENSE](LICENSE) for details.

Copyright (c) 2026 Vladislav Ermolaev

# ReAmbiBox

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-0078D6.svg?logo=windows)](https://github.com/ermolaevv/ReAmbiBox)
[![Language](https://img.shields.io/badge/Language-Rust%202021-DEA584.svg?logo=rust)](https://www.rust-lang.org/)
[![Tested on](https://img.shields.io/badge/Tested%20on-Arduino%20(Adalight)%20%7C%2098%20LED-brightgreen.svg)]()

**A lightweight, responsive, and fast ambient lighting service (Ambilight) for modern Windows.**  
Written from scratch in Rust, spiritually inspired by the legendary **AmbiBox**.

[English](README.md) • [Русский](README.ru.md)

</div>

---

## 💡 Background & Inspiration

For many years, **AmbiBox** was the go-to tool for DIY ambient lighting behind a monitor.  
However, the last AmbiBox update came out in **2015**. Since then, Windows has moved on significantly: new WDDM graphics drivers, HDR, high-refresh-rate monitors — and the old screen capture methods built for Windows 7/8 began causing micro-stutters, high CPU usage, and random freezes.

**ReAmbiBox** is a modern spiritual successor to AmbiBox. The familiar principles have been preserved — perimeter capture, the classic Adalight protocol, and the open Lightpack API server — but everything has been rebuilt on a completely new technology stack:
- **Written in Rust**: guaranteed memory safety, instant response, and no garbage-collector pauses.
- **DirectX 11 Direct Capture (DXGI Desktop Duplication)**: frames are read directly from GPU memory with no intermediate copies in RAM. Thanks to the *zero-allocation* architecture, CPU load approaches 0% even at a full 60+ FPS.
- **Web-based control panel**: no more cluttered legacy UIs — everything is configured right in the browser through a clean, modern web dashboard.

---

## ⚠️ Hardware Notice (What It Has Been Tested On)

> [!IMPORTANT]
> **ReAmbiBox was developed and tested exclusively on the author's personal hardware:**
> - **OS**: Windows 11 Pro (x64)
> - **Controller**: Arduino Nano / Uno with the classic **Adalight** firmware
> - **Connection**: USB COM port (`COM10`, baud rate `115200`)
> - **LED strip**: 98 addressable WS2812B LEDs around the monitor perimeter (26 top, 25 bottom, 24 left, 23 right, clockwise from the bottom-left corner)
>
> Support for **WLED** network controllers (Wi-Fi UDP via DDP and WARLS protocols), high baud rates (up to 2 000 000), and other LED configurations is implemented strictly according to official protocol specifications. If you have different hardware — your feedback, bug reports, and pull requests are very welcome!

---

## ✨ What ReAmbiBox Can Do

### 🎮 Smooth Real-Time Lighting (60+ FPS)
Instead of slow GDI screenshots, the program uses the DirectX 11 Desktop Duplication API. Screen-edge sampling takes fractions of a millisecond, giving the LEDs a nearly instant reaction in fast-paced games and movies.

### 🎬 Smart Movie Mode (21:9 Letterbox Detection)
Watching a widescreen film on a standard 16:9 monitor? ReAmbiBox detects the black bars at the top and bottom on the fly and shifts the capture zones to the actual image, eliminating dark voids.

### 🛋️ Eye Comfort & Room Calibration
- **Gamma Correction (sRGB 2.2 LUT)**: Correct non-linear tone distribution for realistic colour reproduction.
- **White Balance**: Independent per-channel adjustment of red, green, and blue to match the wall behind your monitor.
- **Black Level Clipping**: Gently dims the LEDs in dark scenes (0–50), preventing harsh white flashes during night scenes and horror films.
- **Saturation Boost**: Makes the ambient light vivid and punchy even when the on-screen image is muted.
- **Temporal Smoothing (EMA)**: Smooth colour transitions with no stroboscopic effect.

### 🌐 Browser Control Panel (`http://localhost:3637`)
All controls are gathered in a sleek web interface:
- **Perimeter Wizard**: Automatic LED placement in just a few clicks.
- **Interactive Zone Editor**: Drag zones with the mouse, resize them by their corners, and use magnetic edge snapping.
- **Settings Presets**: Create and save profiles.

### ⌨️ Global Windows Hotkeys
Control the lighting without leaving fullscreen games:
- `Ctrl + Shift + A` — Toggle lighting on / off
- `Ctrl + Shift + Up / Down` — Adjust brightness by ±10%
- `Ctrl + Shift + O` — Toggle the zone overlay on top of the screen

### 💡 Open Lightpack API
A built-in TCP server on port `3636` is fully compatible with the Lightpack protocol. Supports **Kodi** plugins, **Home Assistant** integrations, and legacy Python automation scripts from AmbiBox.

---

## 🚀 Quick Start

1. **Download** the latest release from the [Releases](https://github.com/ermolaevv/ReAmbiBox/releases) page:
   - `ReAmbiBox-Portable-v1.0.1.zip` (extract anywhere, no installation required)
   - Or `ReAmbiBox-Setup-v1.0.1.exe` (classic installer)
2. **Run `reambibox.exe`**. The app will minimise to the system tray near the clock.
3. **Open the control panel**: Navigate to `http://localhost:3637` in any browser.
4. **Set up your connection**:
   - In the **Hardware** tab, select your Arduino's COM port and baud rate.
5. **Create zones**:
   - In the **Zone Editor** tab, click **Screen Perimeter Wizard…**, enter the LED counts per side (e.g. 26 top, 25 bottom, 24 left, 23 right), and click **Apply to Screen**.
6. Done! Your ambient lighting is live.

---

## 🛠️ Build from Source

If you'd like to build the binary yourself:

```powershell
# Requirements: Rust 1.75+ (MSVC toolchain) and Git on Windows 10/11 x64
git clone https://github.com/ermolaevv/ReAmbiBox.git
cd ReAmbiBox

# Run tests
cargo test --workspace

# Build the release binary
cargo build --release --bin reambibox

# Package the portable distribution as a zip (PowerShell)
.\scripts\package.ps1
```

The final monolithic executable will be at `target\release\reambibox.exe`. All web interface files, the icon, and translation dictionaries are embedded directly into the `.exe` at compile time.

---

## 🤝 Acknowledgements

- **AmbiBox**: Huge thanks to the entire original AmbiBox team (2004–2015) for creating the project that inspired thousands of enthusiasts around the world to build their own ambient lighting rigs.
- **Adalight**: Thanks to the **Adafruit** team and the maker community for the timeless COM-port communication protocol.

---

## 📄 Licence

This project is distributed under the open and free **MIT** licence. See [LICENSE](LICENSE) for details.

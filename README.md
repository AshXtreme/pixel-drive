# 🕹️ PixelDrive

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Target](https://img.shields.io/badge/Target-60%20FPS-green.svg)

A modern, high-performance Game Boy (GB / GBC) and Game Boy Advance (GBA) emulator built in **Rust**, powered by **WGPU** for hardware-accelerated rendering and a dynamic **Libretro Core Bridge**.

---

## ✨ Features

- **Multi-System Architecture:**
  - **Game Boy / Game Boy Color:** Native pure-Rust cycle-accurate emulation core with full HDMA, GBC palette RAM, and MBC1/3/5 support.
  - **Game Boy Advance:** High-performance dynamic Libretro core bridge (mGBA) via `libloading` with automatic fallback.
- **Modern Hardware Rendering:** Built on `wgpu` with native support for **Metal (macOS / Apple Silicon)**, **Vulkan (Linux / Windows)**, and **DirectX 12**.
- **Archive & ROM Support:** Drag-and-drop `.gb`, `.gbc`, `.gba` files, or compressed `.zip` archives directly into the window.
- **Low Latency & High FPS:** Smooth 60 FPS frame synchronization with zero tearing.
- **Cross-Platform:** Designed for macOS, Linux, and Windows.

---

## 🎮 Controls

| GBA / GBC Button | Primary Mapping | Secondary Mapping (WASD Layout) |
| :--- | :--- | :--- |
| **D-Pad Up** | `Up Arrow` | `W` |
| **D-Pad Down** | `Down Arrow` | `S` |
| **D-Pad Left** | `Left Arrow` | `A` |
| **D-Pad Right** | `Right Arrow` | `D` |
| **A Button** | `Z` | `J` |
| **B Button** | `X` | `K` |
| **L Shoulder** | `Q` | `U` |
| **R Shoulder** | `E` | `I` |
| **Start** | `Enter` | `Space` |
| **Select** | `Right Shift` | `Left Shift` / `Backspace` |

### ⚡ Hotkeys & Save States

| Action | Keybinding |
| :--- | :--- |
| **Save State** | `F1` |
| **Quick Load State** | `F5` or `F2` |
| **Select Save State Slot (1–9)** | Number keys `1` .. `9` |

---

## 🚀 Getting Started

### 1. Prerequisites

- [Rust & Cargo](https://www.rust-lang.org/tools/install) (1.75+ recommended)

### 2. Setting Up GBA Cores

PixelDrive uses dynamic Libretro cores for GBA emulation. Place the compiled core library for your platform inside the `cores/` directory in the project root:

```bash
mkdir -p cores
# macOS (Apple Silicon / Intel):
# Place mgba_libretro.dylib in cores/

# Linux:
# Place mgba_libretro.so in cores/

# Windows:
# Place mgba_libretro.dll in cores/
```

### 3. Build & Run

```bash
# Build release binary
cargo build --release

# Run emulator
cargo run --release

# Or launch directly with a ROM:
cargo run --release -- path/to/game.gba
# or
cargo run --release -- path/to/game.gbc
```

---

## 📜 License

This project is licensed under the [MIT License](LICENSE).

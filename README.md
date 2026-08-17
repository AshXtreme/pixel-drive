# 🕹️ PixelDrive

A modern, high-performance Game Boy (GB / GBC) and Game Boy Advance (GBA) emulator built in **Rust**, powered by **WGPU** for hardware-accelerated rendering, real-time **WGSL post-processing shaders**, **cpal** low-latency audio, and a dynamic **Libretro Core Bridge**.

---

## ✨ Features

- **Multi-System Architecture:**
  - **Game Boy / Game Boy Color:** Native cycle-accurate pure-Rust emulation core with full 4-channel APU audio synthesis.
  - **Game Boy Advance:** High-performance dynamic Libretro core bridge (`libloading`) supporting official `mgba_libretro` cores.
- **Hardware-Accelerated Rendering:**
  - Built on **WGPU** supporting Metal (macOS), Vulkan (Linux/Windows), and DirectX 12.
  - **Real-Time WGSL Shaders:** Nearest-neighbor sharp scaling, LCD subpixel grid lines, and GBA color correction tone-mapping.
- **Low-Latency Audio Engine:**
  - Real-time stereo audio pipeline powered by **cpal** with a lock-free ring buffer (`ringbuf`).
  - High-precision Catmull-Rom cubic Hermite spline resampler and 2nd-order Butterworth lowpass filter.
- **Save Management & Persistence:**
  - **Battery Saves (`.sav`):** In-game cartridge RAM automatically flushes to `./saves/<rom_name>.sav`.
  - **Real-Time Save States:** Persistent multi-slot state snapshots saved to disk (`./saves/<rom_name>.state<slot>`).
- **On-Screen Display (OSD) & Menu Bar:**
  - Built with **egui** featuring native file picker dialogs (`rfd`), slot selector, volume controls, and real-time FPS HUD.
- **Speed Controls:**
  - Fast-forward acceleration (uncapped / 2x speed) with audio overflow protection.

---

## 🎮 Controls & Hotkeys

### Gameplay Controls
| GBA / GBC Input | Primary Keyboard Mapping | Secondary Mapping (WASD Layout) |
| :--- | :--- | :--- |
| **D-Pad Up** | `Up Arrow` | `W` |
| **D-Pad Down** | `Down Arrow` | `S` |
| **D-Pad Left** | `Left Arrow` | `A` |
| **D-Pad Right** | `Right Arrow` | `D` |
| **A Button** | `Z` | `J` |
| **B Button** | `X` | `K` |
| **L Shoulder** | `Q` | `U` |
| **R Shoulder** | `E` | `I` |
| **Start** | `Enter` | — |
| **Select** | `Right Shift` | `Left Shift` / `Backspace` |

### Emulation Hotkeys
| Action | Key / Shortcut |
| :--- | :--- |
| **Quick Save State** | `F1` (Saves to active slot) |
| **Quick Load State** | `F5` / `F2` (Loads from active slot) |
| **Cycle Display Shaders** | `F4` (Nearest $\rightarrow$ LCD Grid $\rightarrow$ Color Correction $\rightarrow$ LCD+Color) |
| **Toggle Mute Audio** | `M` |
| **Fast-Forward (2x Toggle)** | `Tab` |
| **Slot Selection** | `1`–`9` |

---

## 🚀 Getting Started

### 1. Prerequisites
- [Rust & Cargo](https://www.rust-lang.org/tools/install) (1.75+ recommended)

### 2. GBA Dynamic Core Setup
PixelDrive loads dynamic Libretro cores for GBA emulation. Place your platform's core binary inside the `./cores/` directory:

```bash
mkdir -p cores
# macOS: cores/mgba_libretro.dylib
# Linux: cores/mgba_libretro.so
# Windows: cores/mgba_libretro.dll
```

### 3. Running PixelDrive

```bash
# Launch with native file dialog & egui OSD
cargo run

# Launch directly with a ROM (supports .gb, .gbc, .gba, and .zip)
cargo run -- /path/to/game.gba
```

---

## 🏗️ Architecture & Project Structure

```
PixelDrive/
├── cores/                  # Dynamic Libretro core libraries (mgba_libretro.dylib)
├── saves/                  # Persistent cartridge saves (.sav) & save states (.state1..9)
├── src/
│   ├── main.rs             # Winit event loop, WGPU/Pixels setup, keyboard & hotkey router
│   ├── core/               # Shared EmulatorCore trait & Input Button mappings
│   ├── gbc/                # Native Game Boy / Game Boy Color emulator core
│   │   ├── cpu.rs          # Sharp SM83 (Z80-like) cycle-accurate CPU
│   │   ├── ppu.rs          # Pixel Processing Unit (Mode 0-3, Scanline FIFO, CGB palettes)
│   │   ├── mmu.rs          # Memory bus, HDMA/GDMA, banking, and I/O registers
│   │   ├── mbc.rs          # Memory Bank Controllers (ROM Only, MBC1, MBC2, MBC3, MBC5)
│   │   ├── apu.rs          # 4-Channel APU Audio Synthesizer (Square 1/2, Wave, Noise)
│   │   └── joypad.rs       # Active-low directional & button matrix
│   ├── gba/                # Game Boy Advance emulation layer
│   │   ├── libretro.rs     # FFI Libretro dynamic bridge with audio/video/input callbacks
│   │   ├── cpu.rs          # ARM7TDMI 32-bit CPU core & mode registers
│   │   ├── arm.rs          # ARM instruction decoder & barrel shifter
│   │   ├── thumb.rs        # 16-bit THUMB instruction decoder
│   │   ├── mmu.rs          # 32-bit GBA Memory Map, DMA controller & Flash/SRAM
│   │   ├── ppu.rs          # GBA PPU with Modes 0-5 bitmap and affine background rendering
│   │   ├── bios.rs         # SWI BIOS routines & HLE fallback
│   │   └── keypad.rs       # GBA KEYINPUT 10-button active-low matrix
│   ├── render/             # Hardware-accelerated rendering & video shaders
│   │   ├── mod.rs          # WGPU ShaderPipeline controller & render pass
│   │   └── shaders.rs      # WGSL shaders (Nearest, LCD Grid, Color Correction)
│   ├── audio/              # Low-latency CPAL stereo audio engine & lock-free ring buffer
│   ├── save.rs             # Battery save (.sav) & state snapshot manager (.state1..9)
│   ├── ui/                 # egui OSD overlay, top menu bar, and toast notification system
│   └── error.rs            # Unified PixelDriveError enum with thiserror
└── Cargo.toml
```

---

## 🧪 Testing & Verification

Run the full suite of 67 unit and integration tests:

```bash
cargo test -- --test-threads=1
```

Run clippy linter for zero warnings:

```bash
cargo clippy --all-targets -- -D warnings
```

---

## 📄 License

MIT License. See [LICENSE](LICENSE) for details.

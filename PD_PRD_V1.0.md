# PixelDrive — Product Requirement Document (PRD) v1.0

**Target OS:** macOS (Apple Silicon M1/M2/M3/M4 & Intel)  
**Language:** 100% Rust (`cargo` toolchain)  
**Supported Hardware Cores:** Game Boy / Game Boy Color (8-bit) & Game Boy Advance (32-bit)  
**Development Workflow:** Agentic Development Platform  

---

## 1. Executive Summary & Core Vision

**PixelDrive** is a high-performance, single-window macOS handheld emulator built entirely in safe Rust. Designed specifically for Agentic Platform, PixelDrive unifies Game Boy Color (8-bit) and Game Boy Advance (32-bit) emulation into a single, cohesive desktop application.

### Key Architectural Pillars
- **Unified Window Shell:** One native macOS app window handles game selection, rendering, audio, and inputs.
- **Trait-Based Modular Cores:** Hardware emulation engines (GBC & GBA) are strictly isolated from the UI layer via a shared `EmulatorCore` trait.
- **Native Rust Speed:** Zero garbage collection pauses, low CPU overhead, and hardware-accelerated Metal surface rendering targeting a rock-solid 60 FPS.

---

## 2. System Architecture

The high-level architecture separates the frontend windowing system from the emulation cores:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        PixelDrive macOS Frontend                       │
│            (winit event loop + pixels framebuffer + cpal)              │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                     ┌──────────────┴──────────────┐
                     │  EmulatorCore (Rust Trait) │
                     └──────────────┬──────────────┘
                                    │
                   ┌────────────────┴────────────────┐
                   ▼                                 ▼
        ┌─────────────────────┐           ┌─────────────────────┐
        │     GbcCore         │           │     GbaCore         │
        │  - Sharp LR35902    │           │  - ARM7TDMI Core    │
        │  - 160x144 Frame    │           │  - 240x160 Frame    │
        │  - 4-Channel APU    │           │  - DirectSound APU  │
        └─────────────────────┘           └─────────────────────┘
```

---

## 3. Detailed Subsystem Specifications

### 3.1 Frontend & macOS App Shell (`src/frontend/`)
- **Window Management (`winit`):** Native macOS window event loop with Retina resolution support and smooth drag-and-drop file ingestion.
- **Graphics Renderer (`pixels` / `wgpu`):** Metal-backed hardware-accelerated 2D surface renderer with integer pixel-perfect scaling.
- **Audio Engine (`cpal`):** Low-latency stereo audio stream buffer piping core samples directly to macOS default audio outputs.
- **ROM Ingestion:** Drag-and-drop `.gb`, `.gbc`, or `.gba` files into the running window to hot-swap cores automatically.

### 3.2 Unified Core Interface (`src/core/`)
All emulator cores implement a unified interface that decouples hardware timing from screen rendering:

```rust
pub trait EmulatorCore {
    /// Advances core simulation by 1 frame (~16.6ms)
    fn step_frame(&mut self);
    
    /// Returns raw RGBA pixel buffer to draw
    fn framebuffer(&self) -> &[u8];
    
    /// Returns native display resolution (width, height)
    fn display_dimensions(&self) -> (u32, u32);
    
    /// Handles controller button state updates
    fn handle_input(&mut self, button: Button, pressed: bool);
    
    /// Returns queued stereo audio samples
    fn audio_buffer(&mut self) -> Vec<f32>;
}
```

### 3.3 Game Boy / GBC Core (`src/gbc/`)
- **CPU:** Sharp LR35902 interpreter (Z80 variant operating at 4.19 / 8.38 MHz).
- **Memory Map:** 64 KB address space routing with support for MBC1, MBC3, and MBC5 Bank Switching.
- **PPU:** Scanline-based graphics renderer (Tilemaps, Background, Window, and 8x8 / 8x16 Sprites).
- **APU:** 4 retro audio channels (2 Pulse channels, 1 Programmable Wave channel, 1 Noise channel).

### 3.4 Game Boy Advance Core (`src/gba/`)
- **CPU:** 32-bit ARM7TDMI processor featuring dual ARM (32-bit) and THUMB (16-bit) instruction sets at 16.78 MHz.
- **Memory Map:** 32-bit bus layout routing EWRAM (256 KB), IWRAM (96 KB), VRAM (96 KB), and Cartridge ROM space.
- **PPU:** Support for Background Modes 0–2 (Tilemaps) and Modes 3–5 (Bitmap/Affine Transformation modes).
- **APU:** Legacy 4-channel retro APU combined with dual DirectSound DMA FIFO channels.

---

## 4. Feature Matrix for v1.0 Release

| Category | Feature Description | Status |
| :--- | :--- | :--- |
| **Core** | Dual Engine GBC + GBA Support in 1 Executable | ✅ Included |
| **Core** | Automatic ROM Header Inspection (`.gb`, `.gbc`, `.gba`) | ✅ Included |
| **UI** | Drag & Drop ROM File Loading | ✅ Included |
| **Graphics** | Integer Aspect Ratio Scaler (4x for GBC / 3x for GBA) | ✅ Included |
| **Graphics** | Metal-Accelerated Framebuffer via `pixels` | ✅ Included |
| **Audio** | Real-time Stereo Audio Resampling (44.1 kHz output) | ✅ Included |
| **Controls** | Configurable Keyboard Mappings (DPad + A/B/Start/Select) | ✅ Included |
| **Saves** | Persistent SRAM Battery Saves (`.sav` files) | ✅ Included |
| **States** | Quick Save & Quick Load States (F1 / F5) | 🟡 Post-v1.0 (v1.1) |

---

## 5. Dependency Configuration (`Cargo.toml`)

```toml
[package]
name = "pixel-drive"
version = "1.0.0"
edition = "2021"
authors = ["PixelDrive Team"]
description = "Unified macOS Handheld Emulator in Rust"

[dependencies]
winit = "0.29"         # Native macOS windowing & event loop
pixels = "0.13"        # Hardware-accelerated 2D framebuffer surface
cpal = "0.15"          # Low-latency cross-platform audio I/O
log = "0.4"            # System logging abstractions
env_logger = "0.11"    # Terminal & stdout log formatting
```

---

## 6. Phased Roadmap

### Phase 1: Project Setup & Window Shell
Initialize Cargo project, setup `winit` + `pixels`, create the `EmulatorCore` trait, and render an animated 60 FPS test grid pattern.

### Phase 2: Game Boy / GBC Core Engine
Implement Sharp LR35902 CPU opcode interpreter, memory bus, PPU scanline renderer, keyboard bindings, and ROM loading.

### Phase 3: Game Boy Advance Core Engine
Implement ARM7TDMI execution pipeline (ARM and THUMB mode decoders), GBA memory map, VRAM modes, and DirectSound.

### Phase 4: Audio, Persistence & App Packaging
Connect CPAL audio streams, enable `.sav` SRAM persistent battery files, and package as a native macOS `PixelDrive.app` bundle.

---

*PixelDrive PRD v1.0 — Prepared for Agentic Platform & Rust macOS Toolchain*

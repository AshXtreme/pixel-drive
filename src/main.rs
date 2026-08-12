mod core;
mod gba;
mod gbc;

use core::{Button, EmulatorCore};
use gba::GbaCore;
use gbc::GbcCore;
use log::{info, warn};
use pixels::{Pixels, SurfaceTexture};
use std::time::Instant;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting PixelDrive Handheld Emulator...");

    let event_loop = EventLoop::new()?;

    // Active emulator core defaults to GBC Core (160x144)
    let mut active_core: Box<dyn EmulatorCore> = Box::new(GbcCore::new());
    let (mut core_width, mut core_height) = active_core.display_dimensions();

    // Default window size: 4x scale for GBC (640x576)
    let window_width = core_width * 4;
    let window_height = core_height * 4;

    let window = WindowBuilder::new()
        .with_title("PixelDrive - Game Boy Color / Game Boy Advance Emulator")
        .with_inner_size(LogicalSize::new(window_width, window_height))
        .with_min_inner_size(LogicalSize::new(core_width, core_height))
        .build(&event_loop)?;

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(core_width, core_height, surface_texture)?
    };

    let mut last_frame_time = Instant::now();
    let frame_duration = std::time::Duration::from_nanos(1_000_000_000 / 60);

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    info!("Closing PixelDrive.");
                    elwt.exit();
                }

                WindowEvent::Resized(new_size) => {
                    if new_size.width > 0 && new_size.height > 0 {
                        if let Err(err) = pixels.resize_surface(new_size.width, new_size.height) {
                            warn!("Pixels surface resize error: {:?}", err);
                        }
                    }
                }

                WindowEvent::DroppedFile(path) => {
                    info!("ROM Drag & Drop detected: {:?}", path);
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        match ext.to_lowercase().as_str() {
                            "gb" | "gbc" => {
                                info!("Ingesting Game Boy / GBC ROM: {}", path.display());
                                let mut gbc = GbcCore::new();
                                match gbc.load_rom_file(&path) {
                                    Ok(_) => {
                                        active_core = Box::new(gbc);
                                        let (w, h) = active_core.display_dimensions();
                                        if w != core_width || h != core_height {
                                            core_width = w;
                                            core_height = h;
                                            if let Err(err) = pixels.resize_buffer(w, h) {
                                                warn!("Failed to resize pixel buffer: {:?}", err);
                                            }
                                        }
                                        window.set_title("PixelDrive - Game Boy Color Engine");
                                    }
                                    Err(err) => warn!("Failed to load GBC ROM: {}", err),
                                }
                            }
                            "gba" => {
                                info!("Ingesting Game Boy Advance ROM: {}", path.display());
                                let mut gba = GbaCore::new();
                                match gba.load_rom_file(&path) {
                                    Ok(header) => {
                                        let title = format!(
                                            "PixelDrive - GBA: {} [{}]",
                                            header.title, header.game_code
                                        );
                                        active_core = Box::new(gba);
                                        let (w, h) = active_core.display_dimensions();
                                        if w != core_width || h != core_height {
                                            core_width = w;
                                            core_height = h;
                                            if let Err(err) = pixels.resize_buffer(w, h) {
                                                warn!("Failed to resize pixel buffer: {:?}", err);
                                            }
                                        }
                                        window.set_title(&title);
                                    }
                                    Err(err) => warn!("Failed to load GBA ROM: {}", err),
                                }
                            }
                            "zip" => {
                                info!("Ingesting ZIP ROM archive: {}", path.display());
                                let mut gba = GbaCore::new();
                                if let Ok(header) = gba.load_rom_file(&path) {
                                    let title = format!(
                                        "PixelDrive - GBA: {} [{}]",
                                        header.title, header.game_code
                                    );
                                    active_core = Box::new(gba);
                                    let (w, h) = active_core.display_dimensions();
                                    core_width = w;
                                    core_height = h;
                                    let _ = pixels.resize_buffer(w, h);
                                    window.set_title(&title);
                                } else {
                                    let mut gbc = GbcCore::new();
                                    if let Ok(_) = gbc.load_rom_file(&path) {
                                        active_core = Box::new(gbc);
                                        let (w, h) = active_core.display_dimensions();
                                        core_width = w;
                                        core_height = h;
                                        let _ = pixels.resize_buffer(w, h);
                                        window.set_title("PixelDrive - Game Boy Color Engine");
                                    } else {
                                        warn!("No valid GBC or GBA ROM found inside ZIP: {}", path.display());
                                    }
                                }
                            }
                            _ => {
                                warn!("Unsupported file extension '.{}' for ROM: {}", ext, path.display());
                            }
                        }
                    }
                }

                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(key_code),
                            state,
                            ..
                        },
                    ..
                } => {
                    let pressed = state == ElementState::Pressed;
                    let button = match key_code {
                        KeyCode::KeyZ | KeyCode::KeyJ => Some(Button::A),
                        KeyCode::KeyX | KeyCode::KeyK => Some(Button::B),
                        KeyCode::KeyA | KeyCode::KeyQ => Some(Button::L),
                        KeyCode::KeyS | KeyCode::KeyE => Some(Button::R),
                        KeyCode::Backspace | KeyCode::ShiftRight => Some(Button::Select),
                        KeyCode::Enter => Some(Button::Start),
                        KeyCode::ArrowUp | KeyCode::KeyW => Some(Button::Up),
                        KeyCode::ArrowDown => Some(Button::Down),
                        KeyCode::ArrowLeft => Some(Button::Left),
                        KeyCode::ArrowRight | KeyCode::KeyD => Some(Button::Right),
                        _ => None,
                    };

                    if let Some(btn) = button {
                        active_core.handle_input(btn, pressed);
                    }
                }

                WindowEvent::RedrawRequested => {
                    active_core.step_frame();

                    let frame = pixels.frame_mut();
                    frame.copy_from_slice(active_core.framebuffer());

                    if let Err(err) = pixels.render() {
                        warn!("Pixels render error: {:?}", err);
                        elwt.exit();
                    }
                }

                _ => {}
            },

            Event::AboutToWait => {
                let now = Instant::now();
                if now.duration_since(last_frame_time) >= frame_duration {
                    last_frame_time = now;
                    window.request_redraw();
                }
            }

            _ => {}
        }
    })?;

    Ok(())
}

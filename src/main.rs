mod core;
mod gba;
mod gbc;

use core::{Button, EmulatorCore};
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
    let mut gbc_core = GbcCore::new();
    let (core_width, core_height) = gbc_core.display_dimensions();

    // Default window size: 4x scale for GBC (640x576)
    let window_width = core_width * 4;
    let window_height = core_height * 4;

    let window = WindowBuilder::new()
        .with_title("PixelDrive - Game Boy Color / GBA Emulator")
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
                                if let Err(err) = gbc_core.load_rom_file(&path) {
                                    warn!("Failed to load ROM file {}: {}", path.display(), err);
                                }
                            }
                            "gba" => {
                                info!("Ingesting Game Boy Advance ROM: {}", path.display());
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
                        KeyCode::Backspace | KeyCode::ShiftRight => Some(Button::Select),
                        KeyCode::Enter => Some(Button::Start),
                        KeyCode::ArrowUp | KeyCode::KeyW => Some(Button::Up),
                        KeyCode::ArrowDown | KeyCode::KeyS => Some(Button::Down),
                        KeyCode::ArrowLeft | KeyCode::KeyA => Some(Button::Left),
                        KeyCode::ArrowRight | KeyCode::KeyD => Some(Button::Right),
                        _ => None,
                    };

                    if let Some(btn) = button {
                        gbc_core.handle_input(btn, pressed);
                    }
                }

                WindowEvent::RedrawRequested => {
                    gbc_core.step_frame();

                    let frame = pixels.frame_mut();
                    frame.copy_from_slice(gbc_core.framebuffer());

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

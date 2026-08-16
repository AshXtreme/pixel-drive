use egui::{epaint::Shadow, Align2, Color32, Context, FontId, Rounding, Stroke, Vec2};
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::State;
use std::time::{Duration, Instant};
use winit::{event::WindowEvent, window::Window};

/// User action requested from the OSD / menu overlay.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GuiAction {
    OpenRomPicker,
    LoadRom(std::path::PathBuf),
    UnloadRom,
    Exit,
    TogglePause,
    Reset,
    ToggleFastForward,
    QuickSave(usize),
    QuickLoad(usize),
    SelectSlot(usize),
    SetVolume(f32),
    ToggleMute,
    ToggleFpsHud,
}

/// GuiRenderer manages egui state, overlay drawing, top menu bar, and on-screen HUD.
pub struct GuiRenderer {
    pub context: Context,
    pub state: State,
    pub renderer: Option<Renderer>,
    pub screen_descriptor: ScreenDescriptor,

    // UI state flags
    pub show_menu_bar: bool,
    pub show_fps_hud: bool,
    pub show_about_dialog: bool,
    pub show_controls_dialog: bool,

    // Emulation state reflection
    pub fps: f32,
    pub frame_time_ms: f32,
    pub is_paused: bool,
    pub is_fast_forward: bool,
    pub is_muted: bool,
    pub master_volume: f32,
    pub active_save_slot: usize,
    pub loaded_rom_name: Option<String>,
    pub active_core_name: String,

    // Ephemeral Toast notification
    pub toast: Option<(String, Instant)>,
}

impl GuiRenderer {
    pub fn new(window: &Window, width: u32, height: u32) -> Self {
        let context = Context::default();
        let viewport_id = context.viewport_id();
        let state = State::new(context.clone(), viewport_id, window, None, None);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width.max(1), height.max(1)],
            pixels_per_point: window.scale_factor() as f32,
        };

        Self {
            context,
            state,
            renderer: None,
            screen_descriptor,
            show_menu_bar: true,
            show_fps_hud: true,
            show_about_dialog: false,
            show_controls_dialog: false,
            fps: 60.0,
            frame_time_ms: 16.6,
            is_paused: false,
            is_fast_forward: false,
            is_muted: false,
            master_volume: 1.0,
            active_save_slot: 1,
            loaded_rom_name: None,
            active_core_name: "GBC".to_string(),
            toast: None,
        }
    }

    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    /// Forward window events to egui. Returns true if consumed by egui.
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        self.screen_descriptor.size_in_pixels = [width.max(1), height.max(1)];
        self.screen_descriptor.pixels_per_point = scale_factor;
    }

    /// Build the OSD UI layout and collect user actions.
    pub fn prepare_ui(&mut self, window: &Window) -> Vec<GuiAction> {
        let mut actions = Vec::new();
        let raw_input = self.state.take_egui_input(window);

        self.context.begin_frame(raw_input);

        // Styling: Sleek Dark Retro Arcade Theme
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgba_premultiplied(18, 20, 26, 230);
        visuals.window_fill = Color32::from_rgba_premultiplied(22, 25, 33, 240);
        visuals.window_rounding = Rounding::same(8.0);
        visuals.window_shadow = Shadow {
            offset: [0.0, 4.0].into(),
            blur: 16.0,
            spread: 4.0,
            color: Color32::from_black_alpha(140),
        };
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 75, 110);
        visuals.widgets.active.bg_fill = Color32::from_rgb(80, 110, 180);
        visuals.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(35, 40, 52, 200);
        self.context.set_visuals(visuals);

        // 1. Top Menu Bar
        if self.show_menu_bar {
            egui::TopBottomPanel::top("top_menu_bar")
                .min_height(28.0)
                .show(&self.context, |ui| {
                    ui.horizontal(|ui| {
                        ui.visuals_mut().button_frame = true;

                        // Logo / Brand
                        ui.label(
                            egui::RichText::new("🕹️ PixelDrive")
                                .strong()
                                .color(Color32::from_rgb(255, 180, 50)),
                        );
                        ui.separator();

                        // File Menu
                        ui.menu_button("File", |ui| {
                            if ui.button("📂 Open ROM...").clicked() {
                                actions.push(GuiAction::OpenRomPicker);
                                ui.close_menu();
                            }
                            if self.loaded_rom_name.is_some() {
                                if ui.button("⏹ Unload ROM").clicked() {
                                    actions.push(GuiAction::UnloadRom);
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button("❌ Exit").clicked() {
                                actions.push(GuiAction::Exit);
                                ui.close_menu();
                            }
                        });

                        // Emulation Menu
                        ui.menu_button("Emulation", |ui| {
                            let pause_label = if self.is_paused { "▶ Resume" } else { "⏸ Pause" };
                            if ui.button(pause_label).clicked() {
                                actions.push(GuiAction::TogglePause);
                                ui.close_menu();
                            }
                            if ui.button("🔄 Reset Core").clicked() {
                                actions.push(GuiAction::Reset);
                                ui.close_menu();
                            }
                            ui.separator();
                            let ff_label = if self.is_fast_forward {
                                "⚡ Fast-Forward (2x) [Active]"
                            } else {
                                "⚡ Fast-Forward (2x) (Tab)"
                            };
                            if ui.button(ff_label).clicked() {
                                actions.push(GuiAction::ToggleFastForward);
                                ui.close_menu();
                            }
                        });

                        // State Menu
                        ui.menu_button("Save States", |ui| {
                            let current_stem = self.loaded_rom_name.as_ref().map(|n| {
                                std::path::Path::new(n)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(n.as_str())
                                    .to_string()
                            });

                            let active_exists = if let Some(ref stem) = current_stem {
                                crate::save::SaveManager::state_exists_on_disk(stem, self.active_save_slot)
                            } else {
                                false
                            };

                            let active_status = if active_exists { " [Saved]" } else { " [Empty]" };
                            if ui.button(format!("💾 Quick Save Slot {}{} (F1)", self.active_save_slot, active_status)).clicked() {
                                actions.push(GuiAction::QuickSave(self.active_save_slot));
                                ui.close_menu();
                            }

                            let load_label = format!("📂 Quick Load Slot {}{} (F5)", self.active_save_slot, active_status);
                            if ui.add_enabled(active_exists, egui::Button::new(load_label)).clicked() {
                                actions.push(GuiAction::QuickLoad(self.active_save_slot));
                                ui.close_menu();
                            }

                            ui.separator();
                            ui.label("Slots (1–9):");
                            for slot in 1..=9 {
                                let exists = if let Some(ref stem) = current_stem {
                                    crate::save::SaveManager::state_exists_on_disk(stem, slot)
                                } else {
                                    false
                                };

                                let is_active = slot == self.active_save_slot;
                                let status_badge = if exists { " [Saved]" } else { " [Empty]" };
                                let slot_label = format!("Slot {}{}", slot, status_badge);

                                ui.horizontal(|ui| {
                                    let mut label = egui::RichText::new(slot_label);
                                    if is_active {
                                        label = label.strong().color(Color32::from_rgb(255, 200, 50));
                                    } else if exists {
                                        label = label.color(Color32::from_rgb(100, 220, 140));
                                    } else {
                                        label = label.color(Color32::from_rgb(150, 160, 175));
                                    }

                                    if ui.selectable_label(is_active, label).clicked() {
                                        actions.push(GuiAction::SelectSlot(slot));
                                    }

                                    if exists {
                                        if ui.small_button("Load").clicked() {
                                            actions.push(GuiAction::QuickLoad(slot));
                                            ui.close_menu();
                                        }
                                    }
                                    if ui.small_button("Save").clicked() {
                                        actions.push(GuiAction::QuickSave(slot));
                                        ui.close_menu();
                                    }
                                });
                            }
                        });

                        // Audio Menu
                        ui.menu_button("Audio", |ui| {
                            let mute_label = if self.is_muted { "🔊 Unmute Audio (M)" } else { "🔇 Mute Audio (M)" };
                            if ui.button(mute_label).clicked() {
                                actions.push(GuiAction::ToggleMute);
                                ui.close_menu();
                            }
                            ui.separator();
                            ui.label("Master Volume:");
                            let mut vol = self.master_volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).text("Volume")).changed() {
                                actions.push(GuiAction::SetVolume(vol));
                            }
                        });

                        // View Menu
                        ui.menu_button("View", |ui| {
                            if ui.checkbox(&mut self.show_fps_hud, "Show HUD / Stats Overlay").changed() {
                                actions.push(GuiAction::ToggleFpsHud);
                            }
                        });

                        // Help Menu
                        ui.menu_button("Help", |ui| {
                            if ui.button("🎮 Controls & Hotkeys").clicked() {
                                self.show_controls_dialog = true;
                                ui.close_menu();
                            }
                            if ui.button("ℹ️ About PixelDrive").clicked() {
                                self.show_about_dialog = true;
                                ui.close_menu();
                            }
                        });

                        // Status Info on the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(ref rom_name) = self.loaded_rom_name {
                                ui.label(
                                    egui::RichText::new(format!("🎮 [{}] {}", self.active_core_name, rom_name))
                                        .color(Color32::from_rgb(180, 220, 255))
                                        .size(12.0),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("Drag & drop a ROM or press Open ROM")
                                        .italics()
                                        .color(Color32::from_rgb(140, 150, 165))
                                        .size(11.0),
                                );
                            }
                        });
                    });
                });
        }

        // Controls Modal Dialog
        if self.show_controls_dialog {
            egui::Window::new("🎮 Controls & Hotkeys")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&self.context, |ui| {
                    ui.heading("Game Controls");
                    egui::Grid::new("controls_grid").striped(true).show(ui, |ui| {
                        ui.label(egui::RichText::new("D-Pad").strong());
                        ui.label("Arrow Keys / W, A, S, D");
                        ui.end_row();
                        ui.label(egui::RichText::new("A Button").strong());
                        ui.label("Z / J");
                        ui.end_row();
                        ui.label(egui::RichText::new("B Button").strong());
                        ui.label("X / K");
                        ui.end_row();
                        ui.label(egui::RichText::new("L Shoulder").strong());
                        ui.label("Q / U");
                        ui.end_row();
                        ui.label(egui::RichText::new("R Shoulder").strong());
                        ui.label("E / I");
                        ui.end_row();
                        ui.label(egui::RichText::new("Start").strong());
                        ui.label("Enter");
                        ui.end_row();
                        ui.label(egui::RichText::new("Select").strong());
                        ui.label("Shift / Backspace");
                        ui.end_row();
                    });

                    ui.add_space(8.0);
                    ui.heading("Hotkeys");
                    egui::Grid::new("hotkeys_grid").striped(true).show(ui, |ui| {
                        ui.label(egui::RichText::new("Tab").strong());
                        ui.label("Toggle 2x Fast-Forward");
                        ui.end_row();
                        ui.label(egui::RichText::new("M").strong());
                        ui.label("Toggle Audio Mute");
                        ui.end_row();
                        ui.label(egui::RichText::new("1 - 9").strong());
                        ui.label("Select Save State Slot 1-9");
                        ui.end_row();
                        ui.label(egui::RichText::new("F1").strong());
                        ui.label("Quick Save State to Active Slot");
                        ui.end_row();
                        ui.label(egui::RichText::new("F5 / F2").strong());
                        ui.label("Quick Load State from Active Slot");
                        ui.end_row();
                    });

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        self.show_controls_dialog = false;
                    }
                });
        }

        // About Modal Dialog
        if self.show_about_dialog {
            egui::Window::new("ℹ️ About PixelDrive")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&self.context, |ui| {
                    ui.heading("🕹️ PixelDrive Handheld Emulator");
                    ui.label("A high-performance Game Boy Color & Game Boy Advance emulator built with Rust, WGPU, Pixels, and egui.");
                    ui.add_space(6.0);
                    ui.label("• Dual-Core Architecture (Native GBC Core + Libretro GBA Core)");
                    ui.label("• Real-time Save States (Slots 1-9)");
                    ui.label("• Persistent Battery Saves (.sav sync)");
                    ui.label("• Dynamic 2x Fast-Forward & Audio Sync Throttle");
                    ui.label("• On-Screen Display (OSD) & HUD");
                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        self.show_about_dialog = false;
                    }
                });
        }

        // 2. On-Screen Stats HUD (Top-Right)
        if self.show_fps_hud {
            let margin = 10.0;
            let top_offset = if self.show_menu_bar { 36.0 } else { 10.0 };

            egui::Window::new("hud_stats")
                .anchor(Align2::RIGHT_TOP, Vec2::new(-margin, top_offset))
                .title_bar(false)
                .resizable(false)
                .collapsible(false)
                .frame(egui::Frame {
                    inner_margin: egui::Margin::symmetric(8.0, 6.0),
                    rounding: Rounding::same(6.0),
                    fill: Color32::from_rgba_premultiplied(12, 14, 20, 190),
                    stroke: Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 30)),
                    ..Default::default()
                })
                .show(&self.context, |ui| {
                    ui.horizontal(|ui| {
                        let fps_color = if self.fps >= 58.0 {
                            Color32::from_rgb(80, 240, 120)
                        } else if self.fps >= 40.0 {
                            Color32::from_rgb(255, 200, 50)
                        } else {
                            Color32::from_rgb(255, 90, 90)
                        };

                        ui.label(
                            egui::RichText::new(format!("{:.1} FPS", self.fps))
                                .color(fps_color)
                                .font(FontId::monospace(12.0))
                                .strong(),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("{:.1}ms", self.frame_time_ms))
                                .color(Color32::from_rgb(170, 185, 200))
                                .font(FontId::monospace(11.0)),
                        );

                        if self.is_fast_forward {
                            ui.separator();
                            ui.label(
                                egui::RichText::new("⚡ 2X")
                                    .color(Color32::from_rgb(255, 215, 0))
                                    .strong()
                                    .font(FontId::monospace(11.0)),
                            );
                        }

                        if self.is_paused {
                            ui.separator();
                            ui.label(
                                egui::RichText::new("⏸ PAUSED")
                                    .color(Color32::from_rgb(255, 120, 120))
                                    .strong()
                                    .font(FontId::monospace(11.0)),
                            );
                        }

                        if self.is_muted {
                            ui.separator();
                            ui.label(
                                egui::RichText::new("🔇 MUTED")
                                    .color(Color32::from_rgb(200, 100, 100))
                                    .font(FontId::monospace(11.0)),
                            );
                        }
                    });
                });
        }

        // 3. Toast Notifications (Floating centered pill at bottom)
        if let Some((msg, timestamp)) = self.toast.clone() {
            let elapsed = timestamp.elapsed();
            if elapsed < Duration::from_millis(2500) {
                let alpha = if elapsed > Duration::from_millis(2000) {
                    let fade_progress = (2500 - elapsed.as_millis()) as f32 / 500.0;
                    (fade_progress * 255.0) as u8
                } else {
                    240
                };

                egui::Window::new("toast_notification")
                    .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -25.0))
                    .title_bar(false)
                    .resizable(false)
                    .collapsible(false)
                    .frame(egui::Frame {
                        inner_margin: egui::Margin::symmetric(14.0, 8.0),
                        outer_margin: egui::Margin::default(),
                        rounding: Rounding::same(20.0),
                        fill: Color32::from_rgba_premultiplied(20, 24, 34, alpha),
                        stroke: Stroke::new(1.5_f32, Color32::from_rgba_premultiplied(100, 160, 255, alpha)),
                        shadow: Shadow {
                            offset: [0.0, 2.0].into(),
                            blur: 8.0,
                            spread: 2.0,
                            color: Color32::from_black_alpha(alpha.min(100)),
                        },
                    })
                    .show(&self.context, |ui| {
                        ui.label(
                            egui::RichText::new(&msg)
                                .color(Color32::from_rgba_premultiplied(255, 255, 255, alpha))
                                .strong()
                                .size(13.0),
                        );
                    });
            } else {
                self.toast = None;
            }
        }

        actions
    }

    /// Render egui primitives directly to the WGPU surface texture view on top of pixels.
    pub fn render(
        &mut self,
        encoder: &mut pixels::wgpu::CommandEncoder,
        render_target: &pixels::wgpu::TextureView,
        context: &pixels::PixelsContext,
        window: &Window,
    ) {
        let full_output = self.context.end_frame();
        self.state.handle_platform_output(window, full_output.platform_output);

        let clipped_primitives = self.context.tessellate(full_output.shapes, self.screen_descriptor.pixels_per_point);

        if self.renderer.is_none() {
            self.renderer = Some(Renderer::new(
                &context.device,
                pixels::wgpu::TextureFormat::Bgra8UnormSrgb,
                None,
                1,
            ));
        }

        if let Some(ref mut renderer) = self.renderer {
            for (id, image_delta) in &full_output.textures_delta.set {
                renderer.update_texture(&context.device, &context.queue, *id, image_delta);
            }

            renderer.update_buffers(
                &context.device,
                &context.queue,
                encoder,
                &clipped_primitives,
                &self.screen_descriptor,
            );

            {
                let mut render_pass = encoder.begin_render_pass(&pixels::wgpu::RenderPassDescriptor {
                    label: Some("egui_render_pass"),
                    color_attachments: &[Some(pixels::wgpu::RenderPassColorAttachment {
                        view: render_target,
                        resolve_target: None,
                        ops: pixels::wgpu::Operations {
                            load: pixels::wgpu::LoadOp::Load,
                            store: pixels::wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                renderer.render(&mut render_pass, &clipped_primitives, &self.screen_descriptor);
            }

            for id in &full_output.textures_delta.free {
                renderer.free_texture(id);
            }
        }
    }
}


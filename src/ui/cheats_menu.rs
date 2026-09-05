//! In-Game Cheats Menu Modal UI for PixelDrive.
//!
//! Provides an interactive egui modal window for viewing, toggling, adding,
//! validating, editing, and deleting per-game cheat codes.

use crate::cheats::{CheatEngine, CheatType};
use egui::{Align2, Color32, Context, FontId, Rounding, Stroke, Vec2};

/// State for the in-progress Add/Edit cheat form.
#[derive(Debug, Clone)]
pub struct CheatFormState {
    pub desc: String,
    pub code: String,
    pub cheat_type: CheatType,
    pub editing_id: Option<String>,
    pub error_msg: Option<String>,
}

impl Default for CheatFormState {
    fn default() -> Self {
        Self {
            desc: String::new(),
            code: String::new(),
            cheat_type: CheatType::Raw,
            editing_id: None,
            error_msg: None,
        }
    }
}

/// In-game Cheats Menu Modal component.
#[derive(Debug, Clone, Default)]
pub struct CheatsMenu {
    pub is_open: bool,
    pub form: CheatFormState,
    pub show_add_form: bool,
}

impl CheatsMenu {
    /// Creates a new `CheatsMenu`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the cheat codes modal.
    pub fn open(&mut self, is_gba: bool) {
        self.is_open = true;
        self.show_add_form = false;
        self.form = CheatFormState {
            desc: String::new(),
            code: String::new(),
            cheat_type: if is_gba {
                CheatType::GameSharkGba
            } else {
                CheatType::GameSharkGbc
            },
            editing_id: None,
            error_msg: None,
        };
    }

    /// Closes the cheat codes modal and auto-saves the active list to disk.
    pub fn close(&mut self, engine: &mut CheatEngine) {
        self.is_open = false;
        self.show_add_form = false;
        engine.recompile_patches();
        let _ = engine.save();
    }

    /// Renders the egui cheats menu modal window.
    pub fn show(&mut self, ctx: &Context, engine: &mut CheatEngine) {
        if !self.is_open {
            return;
        }

        let mut close_requested = false;
        let game_title = engine
            .rom_identifier
            .as_ref()
            .map_or_else(|| "No ROM Loaded".to_string(), |id| id.title.clone());
        let crc_hex = engine
            .rom_identifier
            .as_ref()
            .map_or_else(|| "00000000".to_string(), |id| id.crc32_hex());

        egui::Window::new("👾 Cheat Codes Engine")
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .min_width(400.0)
            .min_height(350.0)
            .frame(egui::Frame {
                inner_margin: egui::Margin::symmetric(16.0, 14.0),
                rounding: Rounding::same(12.0),
                fill: Color32::from_rgba_premultiplied(16, 18, 26, 245),
                stroke: Stroke::new(1.5_f32, Color32::from_rgb(80, 130, 220)),
                shadow: egui::epaint::Shadow {
                    offset: [0.0, 6.0].into(),
                    blur: 16.0,
                    spread: 2.0,
                    color: Color32::from_black_alpha(180),
                },
                ..Default::default()
            })
            .show(ctx, |ui| {
                // Header with title and ROM CRC32 badge
                ui.horizontal(|ui| {
                    ui.heading("👾 Cheat Codes Manager");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕ Close").clicked() {
                            close_requested = true;
                        }
                        ui.label(
                            egui::RichText::new(format!("CRC32: {}", crc_hex))
                                .font(FontId::monospace(11.0))
                                .color(Color32::from_rgb(160, 200, 255)),
                        );
                    });
                });

                ui.label(
                    egui::RichText::new(format!("Active Game: {}", game_title))
                        .color(Color32::from_rgb(200, 210, 230))
                        .size(13.0),
                );
                ui.separator();

                // Top Toolbar / Summary Actions
                ui.horizontal(|ui| {
                    let enabled_count = engine.cheats.enabled_count();
                    let total_count = engine.cheats.len();
                    ui.label(
                        egui::RichText::new(format!("Active: {} / {}", enabled_count, total_count))
                            .strong()
                            .color(if enabled_count > 0 {
                                Color32::from_rgb(80, 240, 140)
                            } else {
                                Color32::from_rgb(170, 180, 200)
                            }),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(if self.show_add_form { "📋 View Cheats" } else { "➕ Add Cheat" }).clicked() {
                            self.show_add_form = !self.show_add_form;
                            self.form.error_msg = None;
                        }

                        if !self.show_add_form && total_count > 0 {
                            if ui.button("🗑️ Clear All").clicked() {
                                engine.clear_all();
                            }
                            if ui.button("⚪ Disable All").clicked() {
                                engine.toggle_all(false);
                            }
                            if ui.button("⚡ Enable All").clicked() {
                                engine.toggle_all(true);
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                if self.show_add_form {
                    // ========================================================
                    // Add / Edit Cheat Form
                    // ========================================================
                    ui.group(|ui| {
                        ui.heading(if self.form.editing_id.is_some() { "✏️ Edit Cheat Code" } else { "➕ Add New Cheat Code" });
                        ui.add_space(4.0);

                        ui.label("Description / Name:");
                        ui.text_edit_singleline(&mut self.form.desc);

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Cheat Type:");
                            egui::ComboBox::from_id_source("cheat_type_select")
                                .selected_text(self.form.cheat_type.label())
                                .show_ui(ui, |ui| {
                                    if engine.is_gba {
                                        ui.selectable_value(&mut self.form.cheat_type, CheatType::GameSharkGba, CheatType::GameSharkGba.label());
                                        ui.selectable_value(&mut self.form.cheat_type, CheatType::ActionReplayMax, CheatType::ActionReplayMax.label());
                                        ui.selectable_value(&mut self.form.cheat_type, CheatType::Raw, CheatType::Raw.label());
                                    } else {
                                        ui.selectable_value(&mut self.form.cheat_type, CheatType::GameSharkGbc, CheatType::GameSharkGbc.label());
                                        ui.selectable_value(&mut self.form.cheat_type, CheatType::Raw, CheatType::Raw.label());
                                    }
                                });
                        });

                        ui.add_space(6.0);
                        ui.label("Cheat Code (Hex strings, multiline supported):");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.form.code)
                                .font(FontId::monospace(12.0))
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        );

                        // Real-time validation feedback
                        let validation = CheatEngine::validate_code(&self.form.code, self.form.cheat_type, engine.is_gba);
                        match validation {
                            crate::cheats::engine::ValidationResult::Valid { lines_count, .. } => {
                                ui.label(
                                    egui::RichText::new(format!("✓ Valid code format ({} line{})", lines_count, if lines_count == 1 { "" } else { "s" }))
                                        .color(Color32::from_rgb(80, 240, 140))
                                        .size(12.0),
                                );
                            }
                            crate::cheats::engine::ValidationResult::Invalid(ref err) => {
                                ui.label(
                                    egui::RichText::new(format!("⚠ {}", err))
                                        .color(Color32::from_rgb(255, 120, 120))
                                        .size(12.0),
                                );
                            }
                        }

                        if let Some(ref err) = self.form.error_msg {
                            ui.label(
                                egui::RichText::new(format!("Error: {}", err))
                                    .color(Color32::from_rgb(255, 80, 80))
                                    .strong(),
                            );
                        }

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let is_valid = matches!(validation, crate::cheats::engine::ValidationResult::Valid { .. }) && !self.form.desc.trim().is_empty();

                            let save_btn = ui.add_enabled(
                                is_valid,
                                egui::Button::new(if self.form.editing_id.is_some() { "💾 Update Cheat" } else { "➕ Save Cheat Code" }),
                            );

                            if save_btn.clicked() {
                                if let Some(ref edit_id) = self.form.editing_id.clone() {
                                    // Update existing
                                    if let Some(entry) = engine.cheats.entries.iter_mut().find(|e| &e.id == edit_id) {
                                        entry.desc = self.form.desc.trim().to_string();
                                        entry.code = self.form.code.trim().to_string();
                                        entry.cheat_type = self.form.cheat_type;
                                        engine.recompile_patches();
                                        let _ = engine.save();
                                        self.show_add_form = false;
                                        self.form = CheatFormState::default();
                                    }
                                } else {
                                    // Add new
                                    match engine.add_cheat(
                                        self.form.desc.trim().to_string(),
                                        self.form.code.trim().to_string(),
                                        self.form.cheat_type,
                                    ) {
                                        Ok(()) => {
                                            self.show_add_form = false;
                                            self.form = CheatFormState::default();
                                        }
                                        Err(err) => {
                                            self.form.error_msg = Some(err);
                                        }
                                    }
                                }
                            }

                            if ui.button("Cancel").clicked() {
                                self.show_add_form = false;
                                self.form = CheatFormState::default();
                            }
                        });
                    });
                } else {
                    // ========================================================
                    // List View of Registered Cheats
                    // ========================================================
                    if engine.cheats.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("No cheat codes added for this game.")
                                    .color(Color32::from_rgb(160, 170, 190))
                                    .size(14.0),
                            );
                            ui.add_space(8.0);
                            if ui.button("➕ Add Your First Cheat Code").clicked() {
                                self.show_add_form = true;
                            }
                            ui.add_space(20.0);
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(340.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut toggle_idx = None;
                                let mut delete_idx = None;
                                let mut edit_entry = None;

                                for (idx, entry) in engine.cheats.entries.iter().enumerate() {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            // Toggle Checkbox / Switch
                                            let mut enabled = entry.enabled;
                                            if ui.checkbox(&mut enabled, "").changed() {
                                                toggle_idx = Some(idx);
                                            }

                                            // Description & Type Badge
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(&entry.desc)
                                                            .strong()
                                                            .size(13.5)
                                                            .color(if entry.enabled {
                                                                Color32::WHITE
                                                            } else {
                                                                Color32::from_rgb(150, 160, 175)
                                                            }),
                                                    );

                                                    // Badge
                                                    let badge_bg = match entry.cheat_type {
                                                        CheatType::GameSharkGba => Color32::from_rgb(60, 100, 180),
                                                        CheatType::ActionReplayMax => Color32::from_rgb(160, 80, 180),
                                                        CheatType::GameSharkGbc => Color32::from_rgb(40, 150, 120),
                                                        CheatType::Raw => Color32::from_rgb(140, 110, 40),
                                                    };
                                                    ui.label(
                                                        egui::RichText::new(format!(" {} ", entry.cheat_type.badge()))
                                                            .font(FontId::monospace(10.0))
                                                            .background_color(badge_bg)
                                                            .color(Color32::WHITE),
                                                    );
                                                });

                                                // Monospace formatted code snippet
                                                let preview = entry.code.lines().next().unwrap_or("").trim();
                                                let more_lines = entry.code.lines().count().saturating_sub(1);
                                                let code_text = if more_lines > 0 {
                                                    format!("{} (+{} lines)", preview, more_lines)
                                                } else {
                                                    preview.to_string()
                                                };

                                                ui.label(
                                                    egui::RichText::new(code_text)
                                                        .font(FontId::monospace(11.0))
                                                        .color(Color32::from_rgb(180, 195, 215)),
                                                );
                                            });

                                            // Action Buttons (Edit / Delete)
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.button("🗑️").on_hover_text("Delete Cheat").clicked() {
                                                    delete_idx = Some(idx);
                                                }
                                                if ui.button("✏️").on_hover_text("Edit Cheat").clicked() {
                                                    edit_entry = Some(entry.clone());
                                                }
                                            });
                                        });
                                    });
                                    ui.add_space(2.0);
                                }

                                if let Some(idx) = toggle_idx {
                                    engine.toggle_cheat(idx);
                                }
                                if let Some(idx) = delete_idx {
                                    engine.remove_cheat(idx);
                                }
                                if let Some(entry) = edit_entry {
                                    self.form = CheatFormState {
                                        desc: entry.desc,
                                        code: entry.code,
                                        cheat_type: entry.cheat_type,
                                        editing_id: Some(entry.id),
                                        error_msg: None,
                                    };
                                    self.show_add_form = true;
                                }
                            });
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("← Back / Save").clicked() {
                        close_requested = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Auto-saves .cht on change")
                                .color(Color32::from_rgb(130, 140, 160))
                                .size(11.0),
                        );
                    });
                });
            });

        if close_requested {
            self.close(engine);
        }
    }
}

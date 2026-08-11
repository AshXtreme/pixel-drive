use super::mmu::MemoryBus;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

/// Pixel Processing Unit (PPU) handling Game Boy and Game Boy Color scanline graphics rendering.
pub struct Ppu {
    framebuffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
    scanline_counter: u16,
    pub ly: u8,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            framebuffer: [255; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            scanline_counter: 0,
            ly: 0,
        }
    }

    /// Returns reference to raw RGBA framebuffer slice.
    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// Advances PPU simulation by `cycles` CPU dots and renders scanlines when due.
    pub fn step(&mut self, cycles: u8, bus: &mut MemoryBus) {
        let lcdc = bus.read_byte(0xFF40);
        let lcd_enable = (lcdc & 0x80) != 0;

        if !lcd_enable {
            self.scanline_counter = 0;
            self.ly = 0;
            bus.set_ly_direct(0);
            return;
        }

        self.scanline_counter += cycles as u16;

        // 456 CPU dots/cycles per scanline
        if self.scanline_counter >= 456 {
            self.scanline_counter -= 456;
            self.ly = (self.ly + 1) % 154;

            // Sync LY register (0xFF44) to MMU memory space directly
            bus.set_ly_direct(self.ly);

            // Trigger VBlank interrupt (Bit 0 of IF 0xFF0F) on scanline 144
            if self.ly == 144 {
                let if_reg = bus.read_byte(0xFF0F);
                bus.write_byte(0xFF0F, if_reg | 0x01);
            }

            // Scanlines 0..=143 are active draw lines
            if self.ly < 144 {
                self.render_scanline(bus);
            }
        }

        // Update STAT register (0xFF41) mode & LYC coincidence flags
        let mode = if self.ly >= 144 {
            1 // Mode 1 VBlank
        } else if self.scanline_counter < 80 {
            2 // Mode 2 OAM Search
        } else if self.scanline_counter < 248 {
            3 // Mode 3 Transfer
        } else {
            0 // Mode 0 HBlank
        };

        let stat = bus.read_byte(0xFF41);
        let lyc = bus.read_byte(0xFF45);
        let lyc_flag = if self.ly == lyc { 0x04 } else { 0x00 };
        bus.set_stat_direct((stat & 0xF8) | lyc_flag | mode);
    }

    /// Renders background, window, and sprite scanlines into RGBA framebuffer for current LY.
    fn render_scanline(&mut self, bus: &MemoryBus) {
        let lcdc = bus.read_byte(0xFF40);
        let scy = bus.read_byte(0xFF42);
        let scx = bus.read_byte(0xFF43);
        let wy = bus.read_byte(0xFF4A);
        let wx = bus.read_byte(0xFF4B);
        let bgp = bus.read_byte(0xFF47);

        let window_enable = (lcdc & 0x20) != 0 && self.ly >= wy && wx <= 166;
        let bg_tile_map: u16 = if (lcdc & 0x08) != 0 { 0x9C00 } else { 0x9800 };
        let win_tile_map: u16 = if (lcdc & 0x40) != 0 { 0x9C00 } else { 0x9800 };
        let unsigned_tile_data = (lcdc & 0x10) != 0;

        let y = self.ly;
        let bg_y = y.wrapping_add(scy);
        let is_gbc = bus.is_gbc;
        let use_palette_ram = is_gbc && bus.palette_ram_written;

        // Render Background & Window Scanline
        for x in 0..SCREEN_WIDTH {
            let is_window = window_enable && (x as u8 + 7 >= wx);

            let (tile_map_base, render_x, render_y) = if is_window {
                let win_x = (x as u8 + 7).wrapping_sub(wx);
                let win_y = y.wrapping_sub(wy);
                (win_tile_map, win_x, win_y)
            } else {
                let bg_x = (x as u8).wrapping_add(scx);
                (bg_tile_map, bg_x, bg_y)
            };

            let tile_row = (render_y / 8) as u16;
            let tile_col = (render_x / 8) as u16;
            let tile_map_addr = tile_map_base + tile_row * 32 + tile_col;

            // Bank 0 contains Tile Index
            let tile_index = bus.read_vram_bank(tile_map_addr, 0);

            let (pal_num, tile_bank, x_flip, y_flip) = if is_gbc {
                // Bank 1 contains GBC Tile Attributes
                let attr = bus.read_vram_bank(tile_map_addr, 1);
                (
                    (attr & 0x07) as usize,
                    (attr >> 3) & 0x01,
                    (attr & 0x20) != 0,
                    (attr & 0x40) != 0,
                )
            } else {
                (0, 0, false, false)
            };

            let mut tile_y = (render_y % 8) as u16;
            if y_flip {
                tile_y = 7 - tile_y;
            }

            let tile_data_addr: u16 = if unsigned_tile_data {
                0x8000 + (tile_index as u16) * 16
            } else {
                let signed_index = tile_index as i8 as i16;
                (0x9000i32 + (signed_index as i32 * 16)) as u16
            };

            let byte1 = bus.read_vram_bank(tile_data_addr + tile_y * 2, tile_bank);
            let byte2 = bus.read_vram_bank(tile_data_addr + tile_y * 2 + 1, tile_bank);

            let bit_idx = if x_flip {
                render_x % 8
            } else {
                7 - (render_x % 8)
            };

            let bit_low = (byte1 >> bit_idx) & 1;
            let bit_high = (byte2 >> bit_idx) & 1;
            let color_id = (bit_high << 1) | bit_low;

            let shade = (bgp >> (color_id * 2)) & 0x03;
            let (r, g, b) = if use_palette_ram {
                bus.get_bg_palette_color(pal_num, shade as usize)
            } else if is_gbc {
                // Game Boy Color dual-compatibility default palette (Pokémon Yellow warm theme)
                match shade {
                    0 => (255, 240, 160), // Warm Yellow
                    1 => (224, 144, 48),  // Vibrant Orange
                    2 => (160, 64, 32),   // Dark Rust
                    _ => (40, 24, 16),    // Deep Espresso
                }
            } else {
                // Classic DMG Pea-Soup Green
                match shade {
                    0 => (224, 248, 208),
                    1 => (136, 192, 112),
                    2 => (52, 104, 86),
                    _ => (8, 24, 32),
                }
            };

            let idx = (y as usize * SCREEN_WIDTH + x) * 4;
            self.framebuffer[idx] = r;
            self.framebuffer[idx + 1] = g;
            self.framebuffer[idx + 2] = b;
            self.framebuffer[idx + 3] = 255;
        }

        // Render Sprites (OBJ)
        let obj_enable = (lcdc & 0x02) != 0;
        if obj_enable {
            let sprite_height: i16 = if (lcdc & 0x04) != 0 { 16 } else { 8 };
            let obp0 = bus.read_byte(0xFF48);
            let obp1 = bus.read_byte(0xFF49);

            for i in (0..40).rev() {
                let oam_addr = 0xFE00 + i * 4;
                let sprite_y = bus.read_byte(oam_addr) as i16 - 16;
                let sprite_x = bus.read_byte(oam_addr + 1) as i16 - 8;
                let tile_idx = bus.read_byte(oam_addr + 2);
                let flags = bus.read_byte(oam_addr + 3);

                let cur_y = y as i16;
                if cur_y >= sprite_y && cur_y < sprite_y + sprite_height {
                    let y_flip = (flags & 0x40) != 0;
                    let x_flip = (flags & 0x20) != 0;
                    let pal_num = (flags & 0x07) as usize;
                    let tile_bank = (flags >> 3) & 0x01;

                    let mut line = cur_y - sprite_y;
                    if y_flip {
                        line = sprite_height - 1 - line;
                    }

                    let tile_addr = 0x8000 + (tile_idx as u16) * 16 + (line as u16) * 2;
                    let byte1 = bus.read_vram_bank(tile_addr, tile_bank);
                    let byte2 = bus.read_vram_bank(tile_addr + 1, tile_bank);

                    for px in 0..8 {
                        let target_x = sprite_x + px;
                        if target_x >= 0 && target_x < SCREEN_WIDTH as i16 {
                            let bit_idx = if x_flip { px } else { 7 - px };
                            let bit_low = (byte1 >> bit_idx) & 1;
                            let bit_high = (byte2 >> bit_idx) & 1;
                            let color_id = (bit_high << 1) | bit_low;

                            // Color 0 is transparent for Sprites
                            if color_id != 0 {
                                let palette = if (flags & 0x10) != 0 { obp1 } else { obp0 };
                                let shade = (palette >> (color_id * 2)) & 0x03;

                                let (r, g, b) = if use_palette_ram {
                                    let (pr, pg, pb) = bus.get_obj_palette_color(pal_num, shade as usize);
                                    log::trace!("OBJ Palette Read: R={}, G={}, B={}", pr, pg, pb);
                                    (pr, pg, pb)
                                } else if is_gbc {
                                    match shade {
                                        0 => (255, 240, 160),
                                        1 => (224, 144, 48),
                                        2 => (160, 64, 32),
                                        _ => (40, 24, 16),
                                    }
                                } else {
                                    match shade {
                                        0 => (224, 248, 208),
                                        1 => (136, 192, 112),
                                        2 => (52, 104, 86),
                                        _ => (8, 24, 32),
                                    }
                                };

                                let idx = (y as usize * SCREEN_WIDTH + target_x as usize) * 4;
                                self.framebuffer[idx] = r;
                                self.framebuffer[idx + 1] = g;
                                self.framebuffer[idx + 2] = b;
                                self.framebuffer[idx + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

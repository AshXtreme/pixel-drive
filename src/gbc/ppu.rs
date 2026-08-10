use super::mmu::MemoryBus;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

/// Pixel Processing Unit (PPU) handling Game Boy scanline graphics rendering.
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
            let tile_index = bus.read_byte(tile_map_addr);

            let tile_data_addr: u16 = if unsigned_tile_data {
                0x8000 + (tile_index as u16) * 16
            } else {
                let signed_index = tile_index as i8 as i16;
                (0x9000i32 + (signed_index as i32 * 16)) as u16
            };

            let tile_y = (render_y % 8) as u16;
            let byte1 = bus.read_byte(tile_data_addr + tile_y * 2);
            let byte2 = bus.read_byte(tile_data_addr + tile_y * 2 + 1);

            let bit_idx = 7 - (render_x % 8);
            let bit_low = (byte1 >> bit_idx) & 1;
            let bit_high = (byte2 >> bit_idx) & 1;
            let color_id = (bit_high << 1) | bit_low;

            let shade = (bgp >> (color_id * 2)) & 0x03;
            let (r, g, b) = match shade {
                0 => (255, 255, 255),
                1 => (192, 192, 192),
                2 => (96, 96, 96),
                _ => (0, 0, 0),
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
                    let palette = if (flags & 0x10) != 0 { obp1 } else { obp0 };
                    let y_flip = (flags & 0x40) != 0;
                    let x_flip = (flags & 0x20) != 0;

                    let mut line = cur_y - sprite_y;
                    if y_flip {
                        line = sprite_height - 1 - line;
                    }

                    let tile_addr = 0x8000 + (tile_idx as u16) * 16 + (line as u16) * 2;
                    let byte1 = bus.read_byte(tile_addr);
                    let byte2 = bus.read_byte(tile_addr + 1);

                    for px in 0..8 {
                        let target_x = sprite_x + px;
                        if target_x >= 0 && target_x < SCREEN_WIDTH as i16 {
                            let bit_idx = if x_flip { px } else { 7 - px };
                            let bit_low = (byte1 >> bit_idx) & 1;
                            let bit_high = (byte2 >> bit_idx) & 1;
                            let color_id = (bit_high << 1) | bit_low;

                            // Color 0 is transparent for Sprites
                            if color_id != 0 {
                                let shade = (palette >> (color_id * 2)) & 0x03;
                                let (r, g, b) = match shade {
                                    0 => (255, 255, 255),
                                    1 => (192, 192, 192),
                                    2 => (96, 96, 96),
                                    _ => (0, 0, 0),
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

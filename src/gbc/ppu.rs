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
            bus.write_byte(0xFF44, 0);
            return;
        }

        self.scanline_counter += cycles as u16;

        // 456 CPU dots/cycles per scanline
        if self.scanline_counter >= 456 {
            self.scanline_counter -= 456;
            self.ly = (self.ly + 1) % 154;

            // Sync LY register (0xFF44) to MMU memory space
            bus.write_byte(0xFF44, self.ly);

            // Scanlines 0..=143 are active draw lines
            if self.ly < 144 {
                self.render_scanline(bus);
            }
        }
    }

    /// Renders a single background tile scanline into the RGBA framebuffer for current LY.
    fn render_scanline(&mut self, bus: &MemoryBus) {
        let lcdc = bus.read_byte(0xFF40);

        // Bit 0 of LCDC: BG & Window Display Enable
        let bg_enable = (lcdc & 0x01) != 0;
        if !bg_enable {
            let y = self.ly as usize;
            for x in 0..SCREEN_WIDTH {
                let idx = (y * SCREEN_WIDTH + x) * 4;
                self.framebuffer[idx] = 255;
                self.framebuffer[idx + 1] = 255;
                self.framebuffer[idx + 2] = 255;
                self.framebuffer[idx + 3] = 255;
            }
            return;
        }

        let scy = bus.read_byte(0xFF42);
        let scx = bus.read_byte(0xFF43);
        let bgp = bus.read_byte(0xFF47);

        // Bit 3 of LCDC: BG Tile Map Display Select (0 = 0x9800, 1 = 0x9C00)
        let tile_map_base: u16 = if (lcdc & 0x08) != 0 { 0x9C00 } else { 0x9800 };

        // Bit 4 of LCDC: BG & Window Tile Data Select (0 = 0x8800, 1 = 0x8000)
        let unsigned_tile_data = (lcdc & 0x10) != 0;

        let y = self.ly;
        let bg_y = y.wrapping_add(scy);
        let tile_row = (bg_y / 8) as u16;

        for x in 0..SCREEN_WIDTH {
            let bg_x = (x as u8).wrapping_add(scx);
            let tile_col = (bg_x / 8) as u16;

            let tile_map_addr = tile_map_base + tile_row * 32 + tile_col;
            let tile_index = bus.read_byte(tile_map_addr);

            let tile_data_addr: u16 = if unsigned_tile_data {
                0x8000 + (tile_index as u16) * 16
            } else {
                let signed_index = tile_index as i8 as i16;
                (0x9000i32 + (signed_index as i32 * 16)) as u16
            };

            let tile_y = (bg_y % 8) as u16;
            let byte1 = bus.read_byte(tile_data_addr + tile_y * 2);
            let byte2 = bus.read_byte(tile_data_addr + tile_y * 2 + 1);

            let bit_idx = 7 - (bg_x % 8);
            let bit_low = (byte1 >> bit_idx) & 1;
            let bit_high = (byte2 >> bit_idx) & 1;
            let color_id = (bit_high << 1) | bit_low;

            // Map color_id to 2-bit shade using BGP register (0xFF47)
            let shade = (bgp >> (color_id * 2)) & 0x03;

            // Standard Game Boy monochrome shade mapping to RGBA
            let (r, g, b) = match shade {
                0 => (255, 255, 255), // White
                1 => (192, 192, 192), // Light Gray
                2 => (96, 96, 96),   // Dark Gray
                _ => (0, 0, 0),       // Black
            };

            let idx = (y as usize * SCREEN_WIDTH + x) * 4;
            self.framebuffer[idx] = r;
            self.framebuffer[idx + 1] = g;
            self.framebuffer[idx + 2] = b;
            self.framebuffer[idx + 3] = 255;
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

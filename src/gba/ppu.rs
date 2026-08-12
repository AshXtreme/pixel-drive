#![allow(dead_code)]

pub const GBA_SCREEN_WIDTH: usize = 240;
pub const GBA_SCREEN_HEIGHT: usize = 160;
pub const CYCLES_PER_SCANLINE: usize = 1232;
pub const VISIBLE_SCANLINES: usize = 160;
pub const TOTAL_SCANLINES: usize = 228;

pub const PALETTE_SIZE: usize = 1024;  // 1 KB
pub const VRAM_SIZE: usize = 96 * 1024; // 96 KB
pub const OAM_SIZE: usize = 1024;       // 1 KB

/// Convert 16-bit BGR555 color (0x0BBBBBGGGGGRRRRR) to 32-bit RGBA byte tuple (R, G, B, A)
pub fn bgr555_to_rgba(color: u16) -> (u8, u8, u8, u8) {
    let r_5 = (color & 0x001F) as u8;
    let g_5 = ((color >> 5) & 0x001F) as u8;
    let b_5 = ((color >> 10) & 0x001F) as u8;

    let r_8 = (r_5 << 3) | (r_5 >> 2);
    let g_8 = (g_5 << 3) | (g_5 >> 2);
    let b_8 = (b_5 << 3) | (b_5 >> 2);

    (r_8, g_8, b_8, 255)
}

/// GBA 240x160 Picture Processing Unit (PPU) Engine
pub struct GbaPpu {
    pub framebuffer: Vec<u8>,
    pub palette: Vec<u8>,
    pub vram: Vec<u8>,
    pub oam: Vec<u8>,

    // LCD Control Registers
    pub dispcnt: u16,  // 0x04000000
    pub dispstat: u16, // 0x04000004
    pub vcount: u16,   // 0x04000006

    // BG Control Registers
    pub bg0cnt: u16, // 0x04000008
    pub bg1cnt: u16, // 0x0400000A
    pub bg2cnt: u16, // 0x0400000C
    pub bg3cnt: u16, // 0x0400000E

    // BG Scroll Offsets
    pub bg0hofs: u16,
    pub bg0vofs: u16,
    pub bg1hofs: u16,
    pub bg1vofs: u16,
    pub bg2hofs: u16,
    pub bg2vofs: u16,
    pub bg3hofs: u16,
    pub bg3vofs: u16,

    pub scanline_cycles: usize,
    pub vblank_irq_requested: bool,
}

impl Default for GbaPpu {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaPpu {
    pub fn new() -> Self {
        let fb_size = GBA_SCREEN_WIDTH * GBA_SCREEN_HEIGHT * 4;
        Self {
            framebuffer: vec![0; fb_size],
            palette: vec![0; PALETTE_SIZE],
            vram: vec![0; VRAM_SIZE],
            oam: vec![0; OAM_SIZE],

            dispcnt: 0,
            dispstat: 0,
            vcount: 0,
            vblank_irq_requested: false,

            bg0cnt: 0,
            bg1cnt: 0,
            bg2cnt: 0,
            bg3cnt: 0,

            bg0hofs: 0,
            bg0vofs: 0,
            bg1hofs: 0,
            bg1vofs: 0,
            bg2hofs: 0,
            bg2vofs: 0,
            bg3hofs: 0,
            bg3vofs: 0,

            scanline_cycles: 0,
        }
    }

    /// Reset PPU state and clear framebuffer
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advance PPU timing by specified CPU cycles
    pub fn step(&mut self, cycles: usize) {
        self.scanline_cycles += cycles;

        if self.scanline_cycles >= CYCLES_PER_SCANLINE {
            self.scanline_cycles -= CYCLES_PER_SCANLINE;

            // Render line if in visible range (0..159)
            if (self.vcount as usize) < VISIBLE_SCANLINES {
                self.render_scanline(self.vcount as usize);
            }

            // Advance VCOUNT line counter
            self.vcount = (self.vcount + 1) % TOTAL_SCANLINES as u16;

            // Update DISPSTAT VBlank and VMatch flags
            let is_vblank = (self.vcount as usize) >= VISIBLE_SCANLINES;
            let vmatch_setting = (self.dispstat >> 8) & 0xFF;
            let is_vmatch = self.vcount == vmatch_setting;

            self.dispstat = (self.dispstat & !0x05)
                | (if is_vblank { 1 } else { 0 })
                | (if is_vmatch { 4 } else { 0 });

            if self.vcount == 160 {
                self.vblank_irq_requested = true;
            }
        }
    }

    /// Return reference to raw RGBA pixel framebuffer
    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// Render single scanline based on DISPCNT BG Mode (0-5)
    pub fn render_scanline(&mut self, line: usize) {
        let mode = self.dispcnt & 0x07;

        match mode {
            0 | 1 | 2 => self.render_tile_mode(line, mode),
            3 => self.render_mode3(line),
            4 => self.render_mode4(line),
            5 => self.render_mode5(line),
            _ => self.render_fallback_backdrop(line),
        }
    }

    /// Mode 3: 240x160 16-bit BGR555 Direct Bitmap
    fn render_mode3(&mut self, line: usize) {
        let frame_start = line * GBA_SCREEN_WIDTH * 2;
        for x in 0..GBA_SCREEN_WIDTH {
            let vram_idx = frame_start + x * 2;
            if vram_idx + 1 < self.vram.len() {
                let color = self.vram[vram_idx] as u16 | ((self.vram[vram_idx + 1] as u16) << 8);
                let (r, g, b, a) = bgr555_to_rgba(color);
                let pixel_idx = (line * GBA_SCREEN_WIDTH + x) * 4;
                self.framebuffer[pixel_idx] = r;
                self.framebuffer[pixel_idx + 1] = g;
                self.framebuffer[pixel_idx + 2] = b;
                self.framebuffer[pixel_idx + 3] = a;
            }
        }
    }

    /// Mode 4: 240x160 8-bit Indexed Color Bitmap with Page Flipping
    fn render_mode4(&mut self, line: usize) {
        let frame_select = (self.dispcnt & (1 << 4)) != 0;
        let page_offset = if frame_select { 0xA000 } else { 0x0000 };
        let line_start = page_offset + line * GBA_SCREEN_WIDTH;

        for x in 0..GBA_SCREEN_WIDTH {
            let vram_idx = line_start + x;
            if vram_idx < self.vram.len() {
                let palette_idx = self.vram[vram_idx] as usize;
                let pal_byte_idx = palette_idx * 2;
                let color = if pal_byte_idx + 1 < self.palette.len() {
                    self.palette[pal_byte_idx] as u16 | ((self.palette[pal_byte_idx + 1] as u16) << 8)
                } else {
                    0
                };
                let (r, g, b, a) = bgr555_to_rgba(color);
                let pixel_idx = (line * GBA_SCREEN_WIDTH + x) * 4;
                self.framebuffer[pixel_idx] = r;
                self.framebuffer[pixel_idx + 1] = g;
                self.framebuffer[pixel_idx + 2] = b;
                self.framebuffer[pixel_idx + 3] = a;
            }
        }
    }

    /// Mode 5: 160x128 16-bit BGR555 Double Buffered Bitmap (centered)
    fn render_mode5(&mut self, line: usize) {
        let frame_select = (self.dispcnt & (1 << 4)) != 0;
        let page_offset = if frame_select { 0xA000 } else { 0x0000 };

        // Fill scanline with backdrop color first
        self.render_fallback_backdrop(line);

        if line >= 16 && line < 144 {
            let src_y = line - 16;
            let line_start = page_offset + src_y * 160 * 2;
            for x in 0..160 {
                let vram_idx = line_start + x * 2;
                if vram_idx + 1 < self.vram.len() {
                    let color = self.vram[vram_idx] as u16 | ((self.vram[vram_idx + 1] as u16) << 8);
                    let (r, g, b, a) = bgr555_to_rgba(color);
                    let dst_x = x + 40;
                    let pixel_idx = (line * GBA_SCREEN_WIDTH + dst_x) * 4;
                    self.framebuffer[pixel_idx] = r;
                    self.framebuffer[pixel_idx + 1] = g;
                    self.framebuffer[pixel_idx + 2] = b;
                    self.framebuffer[pixel_idx + 3] = a;
                }
            }
        }
    }

    /// Modes 0-2: Tile/Text & Affine Background rendering using VRAM & Palette RAM
    fn render_tile_mode(&mut self, line: usize, _mode: u16) {
        // Start with background palette backdrop color (palette index 0)
        self.render_fallback_backdrop(line);

        // Check enabled background layers in DISPCNT (bits 8-11)
        for bg in (0..4).rev() {
            let bg_enable = (self.dispcnt & (1 << (8 + bg))) != 0;
            if !bg_enable {
                continue;
            }

            let bgcnt = match bg {
                0 => self.bg0cnt,
                1 => self.bg1cnt,
                2 => self.bg2cnt,
                _ => self.bg3cnt,
            };

            let char_block = ((bgcnt >> 2) & 3) as usize * 0x4000;
            let screen_block = ((bgcnt >> 8) & 0x1F) as usize * 0x800;
            let is_8bpp = (bgcnt & (1 << 7)) != 0;

            let (hofs, vofs) = match bg {
                0 => (self.bg0hofs as usize, self.bg0vofs as usize),
                1 => (self.bg1hofs as usize, self.bg1vofs as usize),
                2 => (self.bg2hofs as usize, self.bg2vofs as usize),
                _ => (self.bg3hofs as usize, self.bg3vofs as usize),
            };

            let curr_y = (line + vofs) & 0xFF;
            let tile_y = curr_y / 8;
            let ty = curr_y % 8;

            for x in 0..GBA_SCREEN_WIDTH {
                let curr_x = (x + hofs) & 0xFF;
                let tile_x = curr_x / 8;
                let tx = curr_x % 8;

                let map_idx = screen_block + (tile_y * 32 + tile_x) * 2;
                if map_idx + 1 >= self.vram.len() {
                    continue;
                }

                let tile_info = self.vram[map_idx] as u16 | ((self.vram[map_idx + 1] as u16) << 8);
                let tile_num = (tile_info & 0x03FF) as usize;
                let h_flip = (tile_info & (1 << 10)) != 0;
                let v_flip = (tile_info & (1 << 11)) != 0;
                let pal_bank = ((tile_info >> 12) & 0x0F) as usize;

                let pixel_x = if h_flip { 7 - tx } else { tx };
                let pixel_y = if v_flip { 7 - ty } else { ty };

                let (color_idx, is_transparent) = if is_8bpp {
                    let tile_addr = char_block + tile_num * 64 + pixel_y * 8 + pixel_x;
                    if tile_addr < self.vram.len() {
                        let idx = self.vram[tile_addr] as usize;
                        (idx, idx == 0)
                    } else {
                        (0, true)
                    }
                } else {
                    let tile_addr = char_block + tile_num * 32 + pixel_y * 4 + pixel_x / 2;
                    if tile_addr < self.vram.len() {
                        let byte = self.vram[tile_addr];
                        let idx = if (pixel_x % 2) == 0 {
                            (byte & 0x0F) as usize
                        } else {
                            ((byte >> 4) & 0x0F) as usize
                        };
                        (pal_bank * 16 + idx, idx == 0)
                    } else {
                        (0, true)
                    }
                };

                if !is_transparent {
                    let pal_byte_idx = color_idx * 2;
                    if pal_byte_idx + 1 < self.palette.len() {
                        let color = self.palette[pal_byte_idx] as u16
                            | ((self.palette[pal_byte_idx + 1] as u16) << 8);
                        let (r, g, b, a) = bgr555_to_rgba(color);
                        let pixel_idx = (line * GBA_SCREEN_WIDTH + x) * 4;
                        self.framebuffer[pixel_idx] = r;
                        self.framebuffer[pixel_idx + 1] = g;
                        self.framebuffer[pixel_idx + 2] = b;
                        self.framebuffer[pixel_idx + 3] = a;
                    }
                }
            }
        }
    }

    /// Render default backdrop color from Palette RAM index 0
    fn render_fallback_backdrop(&mut self, line: usize) {
        let backdrop_color = if self.palette.len() >= 2 {
            self.palette[0] as u16 | ((self.palette[1] as u16) << 8)
        } else {
            0
        };
        let (r, g, b, a) = bgr555_to_rgba(backdrop_color);

        let line_start = line * GBA_SCREEN_WIDTH * 4;
        for x in 0..GBA_SCREEN_WIDTH {
            let pixel_idx = line_start + x * 4;
            self.framebuffer[pixel_idx] = r;
            self.framebuffer[pixel_idx + 1] = g;
            self.framebuffer[pixel_idx + 2] = b;
            self.framebuffer[pixel_idx + 3] = a;
        }
    }

    /// Read PPU I/O register byte
    pub fn read_io(&self, addr: u32) -> u8 {
        match addr {
            0x04000000 => self.dispcnt as u8,
            0x04000001 => (self.dispcnt >> 8) as u8,
            0x04000004 => self.dispstat as u8,
            0x04000005 => (self.dispstat >> 8) as u8,
            0x04000006 => self.vcount as u8,
            0x04000007 => (self.vcount >> 8) as u8,
            0x04000008 => self.bg0cnt as u8,
            0x04000009 => (self.bg0cnt >> 8) as u8,
            0x0400000A => self.bg1cnt as u8,
            0x0400000B => (self.bg1cnt >> 8) as u8,
            0x0400000C => self.bg2cnt as u8,
            0x0400000D => (self.bg2cnt >> 8) as u8,
            0x0400000E => self.bg3cnt as u8,
            0x0400000F => (self.bg3cnt >> 8) as u8,
            _ => 0,
        }
    }

    /// Write PPU I/O register byte
    pub fn write_io(&mut self, addr: u32, val: u8) {
        match addr {
            0x04000000 => self.dispcnt = (self.dispcnt & 0xFF00) | val as u16,
            0x04000001 => self.dispcnt = (self.dispcnt & 0x00FF) | ((val as u16) << 8),
            0x04000004 => self.dispstat = (self.dispstat & 0xFF07) | ((val as u16) & 0x38),
            0x04000005 => self.dispstat = (self.dispstat & 0x00FF) | ((val as u16) << 8),
            0x04000008 => self.bg0cnt = (self.bg0cnt & 0xFF00) | val as u16,
            0x04000009 => self.bg0cnt = (self.bg0cnt & 0x00FF) | ((val as u16) << 8),
            0x0400000A => self.bg1cnt = (self.bg1cnt & 0xFF00) | val as u16,
            0x0400000B => self.bg1cnt = (self.bg1cnt & 0x00FF) | ((val as u16) << 8),
            0x0400000C => self.bg2cnt = (self.bg2cnt & 0xFF00) | val as u16,
            0x0400000D => self.bg2cnt = (self.bg2cnt & 0x00FF) | ((val as u16) << 8),
            0x0400000E => self.bg3cnt = (self.bg3cnt & 0xFF00) | val as u16,
            0x0400000F => self.bg3cnt = (self.bg3cnt & 0x00FF) | ((val as u16) << 8),

            0x04000010 => self.bg0hofs = (self.bg0hofs & 0xFF00) | val as u16,
            0x04000011 => self.bg0hofs = (self.bg0hofs & 0x00FF) | ((val as u16) << 8),
            0x04000012 => self.bg0vofs = (self.bg0vofs & 0xFF00) | val as u16,
            0x04000013 => self.bg0vofs = (self.bg0vofs & 0x00FF) | ((val as u16) << 8),
            0x04000014 => self.bg1hofs = (self.bg1hofs & 0xFF00) | val as u16,
            0x04000015 => self.bg1hofs = (self.bg1hofs & 0x00FF) | ((val as u16) << 8),
            0x04000016 => self.bg1vofs = (self.bg1vofs & 0xFF00) | val as u16,
            0x04000017 => self.bg1vofs = (self.bg1vofs & 0x00FF) | ((val as u16) << 8),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgr555_conversion() {
        // Pure Red (0x001F)
        let (r, g, b, a) = bgr555_to_rgba(0x001F);
        assert_eq!((r, g, b, a), (255, 0, 0, 255));

        // Pure Green (0x03E0)
        let (r, g, b, a) = bgr555_to_rgba(0x03E0);
        assert_eq!((r, g, b, a), (0, 255, 0, 255));

        // Pure Blue (0x7C00)
        let (r, g, b, a) = bgr555_to_rgba(0x7C00);
        assert_eq!((r, g, b, a), (0, 0, 255, 255));
    }

    #[test]
    fn test_mode3_bitmap_rendering() {
        let mut ppu = GbaPpu::new();
        ppu.dispcnt = 3; // Mode 3

        // Set pixel at (10, 20) in VRAM to Pure Red (0x001F)
        let pixel_offset = (20 * 240 + 10) * 2;
        ppu.vram[pixel_offset] = 0x1F;
        ppu.vram[pixel_offset + 1] = 0x00;

        ppu.render_scanline(20);

        let fb_idx = (20 * 240 + 10) * 4;
        assert_eq!(ppu.framebuffer[fb_idx], 255); // Red
        assert_eq!(ppu.framebuffer[fb_idx + 1], 0); // Green
        assert_eq!(ppu.framebuffer[fb_idx + 2], 0); // Blue
        assert_eq!(ppu.framebuffer[fb_idx + 3], 255); // Alpha
    }

    #[test]
    fn test_mode4_indexed_bitmap_rendering() {
        let mut ppu = GbaPpu::new();
        ppu.dispcnt = 4; // Mode 4

        // Set Palette Index 1 to Pure Green (0x03E0)
        ppu.palette[2] = 0xE0;
        ppu.palette[3] = 0x03;

        // Set pixel at (50, 50) to Palette Index 1
        let pixel_offset = 50 * 240 + 50;
        ppu.vram[pixel_offset] = 1;

        ppu.render_scanline(50);

        let fb_idx = (50 * 240 + 50) * 4;
        assert_eq!(ppu.framebuffer[fb_idx], 0);   // Red
        assert_eq!(ppu.framebuffer[fb_idx + 1], 255); // Green
        assert_eq!(ppu.framebuffer[fb_idx + 2], 0);   // Blue
    }
}

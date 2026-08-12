use super::joypad::Joypad;
use super::mbc::Mbc;

/// MemoryBus handles the 64 KB Game Boy memory map routing.
///
/// Memory Map Layout:
/// - 0x0000 - 0x7FFF: ROM Bank 0 & Switchable ROM Bank (via MBC)
/// - 0x8000 - 0x9FFF: VRAM (Video RAM, 8 KB DMG / 16 KB GBC across 2 banks)
/// - 0xA000 - 0xBFFF: External RAM / Cartridge RAM (via MBC)
/// - 0xC000 - 0xDFFF: WRAM (Work RAM, 8 KB)
/// - 0xE000 - 0xFDFF: Echo RAM (Mirror of 0xC000 - 0xDDFF)
/// - 0xFE00 - 0xFE9F: OAM (Object Attribute Memory, 160 bytes)
/// - 0xFEA0 - 0xFEFF: Not Usable
/// - 0xFF00 - 0xFF7F: I/O Registers (128 bytes)
/// - 0xFF80 - 0xFFFE: HRAM (High RAM, 127 bytes)
/// - 0xFFFF: Interrupt Enable Register (IE)
#[allow(dead_code)]
pub struct MemoryBus {
    pub mbc: Mbc,
    pub is_gbc: bool,
    vram: [u8; 0x4000], // 2 banks of 8 KB (0x2000 bytes each)
    vbk: u8,            // 0xFF4F VRAM Bank Select (0 or 1)
    wram: [u8; 0x8000], // 32 KB GBC WRAM (Bank 0 at 0xC000..0xCFFF, Banks 1-7 at 0xD000..0xDFFF)
    svbk: u8,           // 0xFF70 WRAM Bank Select (1 to 7)
    oam: [u8; 0xA0],
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
    pub joypad: Joypad,

    // GBC Palette RAM (64 bytes each: 8 palettes * 4 colors * 2 bytes)
    bg_palette_ram: [u8; 64],
    bgpi: u8, // 0xFF68 BG Palette Index
    obj_palette_ram: [u8; 64],
    obpi: u8, // 0xFF6A OBJ Palette Index
    pub palette_ram_written: bool,
}

#[allow(dead_code)]
impl MemoryBus {
    /// Creates a new `MemoryBus` with DMG hardware register defaults.
    pub fn new() -> Self {
        let mut bus = Self {
            mbc: Mbc::RomOnly { rom: Vec::new() },
            is_gbc: false,
            vram: [0; 0x4000],
            vbk: 0,
            wram: [0; 0x8000],
            svbk: 1,
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            joypad: Joypad::new(),
            bg_palette_ram: [0xFF; 64],
            bgpi: 0,
            obj_palette_ram: [0xFF; 64],
            obpi: 0,
            palette_ram_written: false,
        };

        // DMG default hardware register values post-boot
        bus.io[0x0F] = 0xE1; // IF: Interrupt Flag default
        bus.io[0x40] = 0x91; // LCDC: LCD on, BG on, Tile map 0x9800, Tile data 0x8000
        bus.io[0x47] = 0xE4; // BGP: Standard shade palette (11 10 01 00)
        bus
    }

    /// Loads a ROM buffer into the MBC handler and inspects GBC flag header at 0x0143.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        if rom_bytes.len() > 0x0143 {
            let cgb_flag = rom_bytes[0x0143];
            self.is_gbc = cgb_flag == 0x80 || cgb_flag == 0xC0;
        } else {
            self.is_gbc = false;
        }

        self.mbc = Mbc::from_bytes(rom_bytes);
    }

    /// Reads a single byte from the 64 KB memory bus.
    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.mbc.read_rom(addr),
            0x8000..=0x9FFF => {
                let bank_offset = (self.vbk & 1) as usize * 0x2000;
                self.vram[bank_offset + (addr - 0x8000) as usize]
            }
            0xA000..=0xBFFF => self.mbc.read_ram(addr),
            0xC000..=0xCFFF => self.wram[(addr - 0xC000) as usize],
            0xD000..=0xDFFF => {
                let bank = if self.is_gbc {
                    let b = self.svbk & 0x07;
                    if b == 0 { 1 } else { b as usize }
                } else {
                    1
                };
                self.wram[bank * 0x1000 + (addr - 0xD000) as usize]
            }
            0xE000..=0xFDFF => {
                let norm_addr = addr - 0x2000;
                self.read_byte(norm_addr)
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0x00,
            0xFF00 => self.joypad.read_joyp(),
            0xFF4F => self.vbk | 0xFE,
            0xFF68 => self.bgpi,
            0xFF69 => {
                let idx = (self.bgpi & 0x3F) as usize;
                self.bg_palette_ram[idx]
            }
            0xFF6A => self.obpi,
            0xFF6B => {
                let idx = (self.obpi & 0x3F) as usize;
                self.obj_palette_ram[idx]
            }
            0xFF70 => self.svbk | 0xF8,
            0xFF01..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
        }
    }

    /// Writes a single byte to the 64 KB memory bus.
    pub fn write_byte(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.mbc.write_rom(addr, val),
            0x8000..=0x9FFF => {
                let bank_offset = (self.vbk & 1) as usize * 0x2000;
                self.vram[bank_offset + (addr - 0x8000) as usize] = val;
            }
            0xA000..=0xBFFF => self.mbc.write_ram(addr, val),
            0xC000..=0xCFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xD000..=0xDFFF => {
                let bank = if self.is_gbc {
                    let b = self.svbk & 0x07;
                    if b == 0 { 1 } else { b as usize }
                } else {
                    1
                };
                self.wram[bank * 0x1000 + (addr - 0xD000) as usize] = val;
            }
            0xE000..=0xFDFF => {
                let norm_addr = addr - 0x2000;
                self.write_byte(norm_addr, val);
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = val,
            0xFEA0..=0xFEFF => {} // Reserved / Unusable
            0xFF00 => self.joypad.write_joyp(val),
            0xFF04 => {
                // Writing any value to DIV (0xFF04) resets it to 0
                self.io[0x04] = 0;
            }
            0xFF44 => {
                // Any write to LY (0xFF44) resets it to 0
                self.io[0x44] = 0;
            }
            0xFF46 => {
                // OAM DMA Transfer (0xFF46): Copy 160 bytes from (val << 8) to OAM memory
                self.io[0x46] = val;
                let src_base = (val as u16) << 8;
                for i in 0..160 {
                    let byte = self.read_byte(src_base + i);
                    self.oam[i as usize] = byte;
                }
            }
            0xFF4F => {
                self.vbk = val & 0x01;
                self.io[0x4F] = val;
            }
            0xFF68 => self.bgpi = val,
            0xFF69 => {
                self.palette_ram_written = true;
                let idx = (self.bgpi & 0x3F) as usize;
                self.bg_palette_ram[idx] = val;
                if (self.bgpi & 0x80) != 0 {
                    let next_idx = (self.bgpi & 0x3F).wrapping_add(1) & 0x3F;
                    self.bgpi = 0x80 | next_idx;
                }
            }
            0xFF6A => self.obpi = val,
            0xFF6B => {
                self.palette_ram_written = true;
                let idx = (self.obpi & 0x3F) as usize;
                self.obj_palette_ram[idx] = val;
                if (self.obpi & 0x80) != 0 {
                    let next_idx = (self.obpi & 0x3F).wrapping_add(1) & 0x3F;
                    self.obpi = 0x80 | next_idx;
                }
            }
            0xFF70 => {
                self.svbk = val & 0x07;
                self.io[0x70] = val;
            }
            0xFF01..=0xFF7F => self.io[(addr - 0xFF00) as usize] = val,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF => self.ie = val,
        }
    }

    /// Directly sets STAT register without triggering write logic.
    pub fn set_stat_direct(&mut self, val: u8) {
        self.io[0x41] = val;
    }

    /// Directly sets LY register without triggering reset logic.
    pub fn set_ly_direct(&mut self, val: u8) {
        self.io[0x44] = val;
    }

    /// Directly sets DIV register without triggering reset logic.
    pub fn set_div_direct(&mut self, val: u8) {
        self.io[0x04] = val;
    }

    /// Directly sets TIMA register without triggering write logic.
    pub fn set_tima_direct(&mut self, val: u8) {
        self.io[0x05] = val;
    }

    /// Reads VRAM from specific bank (0 or 1) regardless of active VBK.
    pub fn read_vram_bank(&self, addr: u16, bank: u8) -> u8 {
        let bank_offset = (bank & 1) as usize * 0x2000;
        self.vram[bank_offset + (addr - 0x8000) as usize]
    }

    /// Reads RGB555 palette color converted to 24-bit (R, G, B) from BG Palette RAM.
    pub fn get_bg_palette_color(&self, palette_idx: usize, color_idx: usize) -> (u8, u8, u8) {
        let offset = (palette_idx & 7) * 8 + (color_idx & 3) * 2;
        let byte_low = self.bg_palette_ram[offset] as u16;
        let byte_high = self.bg_palette_ram[offset + 1] as u16;
        let raw_color = (byte_high << 8) | byte_low;

        let r = ((raw_color & 0x001F) as u8) * 8;
        let g = (((raw_color >> 5) & 0x001F) as u8) * 8;
        let b = (((raw_color >> 10) & 0x001F) as u8) * 8;

        (r, g, b)
    }

    /// Reads RGB555 palette color converted to 24-bit (R, G, B) from OBJ Palette RAM.
    pub fn get_obj_palette_color(&self, palette_idx: usize, color_idx: usize) -> (u8, u8, u8) {
        let offset = (palette_idx & 7) * 8 + (color_idx & 3) * 2;
        let byte_low = self.obj_palette_ram[offset] as u16;
        let byte_high = self.obj_palette_ram[offset + 1] as u16;
        let raw_color = (byte_high << 8) | byte_low;

        let r = ((raw_color & 0x001F) as u8) * 8;
        let g = (((raw_color >> 5) & 0x001F) as u8) * 8;
        let b = (((raw_color >> 10) & 0x001F) as u8) * 8;

        (r, g, b)
    }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_bus_read_write() {
        let mut bus = MemoryBus::new();
        bus.write_byte(0xC000, 0x42);
        assert_eq!(bus.read_byte(0xC000), 0x42);

        bus.write_byte(0xFF80, 0x99);
        assert_eq!(bus.read_byte(0xFF80), 0x99);
    }

    #[test]
    fn test_rom_loading() {
        let mut bus = MemoryBus::new();
        let dummy_rom = vec![0x00; 0x8000];
        bus.load_rom(&dummy_rom);
        assert_eq!(bus.read_byte(0x0000), 0x00);
    }

    #[test]
    fn test_gbc_vram_and_palette_banking() {
        let mut bus = MemoryBus::new();
        bus.is_gbc = true;

        // VRAM Bank 0 vs Bank 1
        bus.write_byte(0xFF4F, 0);
        bus.write_byte(0x8000, 0x11);
        bus.write_byte(0xFF4F, 1);
        bus.write_byte(0x8000, 0x22);

        assert_eq!(bus.read_vram_bank(0x8000, 0), 0x11);
        assert_eq!(bus.read_vram_bank(0x8000, 1), 0x22);

        // BG Palette Write with auto-increment (0x80)
        bus.write_byte(0xFF68, 0x80);
        bus.write_byte(0xFF69, 0x1F); // Low byte of RGB555 (red)
        bus.write_byte(0xFF69, 0x00); // High byte

        let (r, g, b) = bus.get_bg_palette_color(0, 0);
        assert_eq!(r, 248);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }
}

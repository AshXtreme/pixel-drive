use super::joypad::Joypad;
use super::mbc::Mbc;

/// MemoryBus handles the 64 KB Game Boy memory map routing.
///
/// Memory Map Layout:
/// - 0x0000 - 0x7FFF: ROM Bank 0 & Switchable ROM Bank (via MBC)
/// - 0x8000 - 0x9FFF: VRAM (Video RAM, 8 KB)
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
    vram: [u8; 0x2000],
    wram: [u8; 0x2000],
    oam: [u8; 0xA0],
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
    pub joypad: Joypad,
}

#[allow(dead_code)]
impl MemoryBus {
    /// Creates a new `MemoryBus` with DMG hardware register defaults.
    pub fn new() -> Self {
        let mut bus = Self {
            mbc: Mbc::RomOnly { rom: Vec::new() },
            vram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            joypad: Joypad::new(),
        };

        // DMG default hardware register values post-boot
        bus.io[0x0F] = 0xE1; // IF: Interrupt Flag default
        bus.io[0x40] = 0x91; // LCDC: LCD on, BG on, Tile map 0x9800, Tile data 0x8000
        bus.io[0x47] = 0xE4; // BGP: Standard shade palette (11 10 01 00)
        bus
    }

    /// Instantiates the appropriate MBC handler from ROM header bytes.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        self.mbc = Mbc::from_bytes(rom_bytes);
    }

    /// Reads a single byte from the 64 KB memory space based on address mapping.
    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.mbc.read_rom(addr),
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.mbc.read_ram(addr),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize], // Echo RAM
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0x00, // Reserved / Unusable
            0xFF00 => self.joypad.read_joyp(),
            0xFF01..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
        }
    }

    /// Writes a single byte to the 64 KB memory space based on address mapping.
    pub fn write_byte(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.mbc.write_rom(addr, val),
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
            0xA000..=0xBFFF => self.mbc.write_ram(addr, val),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val, // Echo RAM
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
            0xFF01..=0xFF7F => self.io[(addr - 0xFF00) as usize] = val,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF => self.ie = val,
        }
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
        // Echo RAM mirror
        assert_eq!(bus.read_byte(0xE000), 0x42);

        // LY reset on write
        bus.write_byte(0xFF44, 0x12);
        assert_eq!(bus.read_byte(0xFF44), 0);
    }

    #[test]
    fn test_rom_loading() {
        let mut bus = MemoryBus::new();
        let rom = vec![0x00, 0xC3, 0x50, 0x01];
        bus.load_rom(&rom);
        assert_eq!(bus.read_byte(0x0000), 0x00);
        assert_eq!(bus.read_byte(0x0001), 0xC3);
    }
}


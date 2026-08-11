#![allow(dead_code)]

use super::ppu::GbaPpu;

pub const BIOS_SIZE: usize = 16 * 1024;   // 16 KB (0x00000000 - 0x00003FFF)
pub const EWRAM_SIZE: usize = 256 * 1024; // 256 KB (0x02000000 - 0x0203FFFF)
pub const IWRAM_SIZE: usize = 32 * 1024;  // 32 KB (0x03000000 - 0x03007FFF)
pub const IO_SIZE: usize = 1024;          // 1 KB (0x04000000 - 0x040003FE)
pub const SRAM_SIZE: usize = 64 * 1024;   // 64 KB (0x0E000000 - 0x0E00FFFF)

/// GBA 32-bit Memory Bus (MMU) handling mapping, byte/halfword/word access,
/// PPU registers/VRAM integration, and Game Pak ROM loading.
pub struct GbaMemoryBus {
    pub ppu: GbaPpu,
    pub bios: Vec<u8>,
    pub ewram: Vec<u8>,
    pub iwram: Vec<u8>,
    pub io: Vec<u8>,
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
}

impl Default for GbaMemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaMemoryBus {
    pub fn new() -> Self {
        Self {
            ppu: GbaPpu::new(),
            bios: vec![0; BIOS_SIZE],
            ewram: vec![0; EWRAM_SIZE],
            iwram: vec![0; IWRAM_SIZE],
            io: vec![0; IO_SIZE],
            rom: Vec::new(),
            sram: vec![0; SRAM_SIZE],
        }
    }

    /// Load raw ROM bytes into the Game Pak ROM space (mapped at 0x08000000).
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        self.rom = rom_bytes.to_vec();
    }

    /// Load BIOS bytes into the System BIOS ROM space (mapped at 0x00000000).
    pub fn load_bios(&mut self, bios_bytes: &[u8]) {
        let len = bios_bytes.len().min(BIOS_SIZE);
        self.bios[..len].copy_from_slice(&bios_bytes[..len]);
    }

    /// Read an 8-bit byte from the 32-bit GBA memory address space.
    pub fn read_u8(&self, addr: u32) -> u8 {
        match addr >> 24 {
            0x00 => {
                let offset = (addr & 0x3FFF) as usize;
                if offset < self.bios.len() {
                    self.bios[offset]
                } else {
                    0
                }
            }
            0x02 => {
                let offset = (addr & 0x3FFFF) as usize;
                self.ewram[offset]
            }
            0x03 => {
                let offset = (addr & 0x7FFF) as usize;
                self.iwram[offset]
            }
            0x04 => {
                let offset = (addr & 0x3FF) as usize;
                if offset <= 0x56 {
                    self.ppu.read_io(addr)
                } else if offset < self.io.len() {
                    self.io[offset]
                } else {
                    0
                }
            }
            0x05 => {
                let offset = (addr & 0x3FF) as usize;
                if offset < self.ppu.palette.len() {
                    self.ppu.palette[offset]
                } else {
                    0
                }
            }
            0x06 => {
                let mut offset = (addr & 0x1FFFF) as usize;
                if offset >= self.ppu.vram.len() {
                    offset -= 0x8000;
                }
                if offset < self.ppu.vram.len() {
                    self.ppu.vram[offset]
                } else {
                    0
                }
            }
            0x07 => {
                let offset = (addr & 0x3FF) as usize;
                if offset < self.ppu.oam.len() {
                    self.ppu.oam[offset]
                } else {
                    0
                }
            }
            0x08..=0x0D => {
                let offset = (addr & 0x01FFFFFF) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0
                }
            }
            0x0E | 0x0F => {
                let offset = (addr & 0xFFFF) as usize;
                if offset < self.sram.len() {
                    self.sram[offset]
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// Write an 8-bit byte to the 32-bit GBA memory address space.
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        match addr >> 24 {
            0x00 => {
                // BIOS System ROM is read-only
            }
            0x02 => {
                let offset = (addr & 0x3FFFF) as usize;
                self.ewram[offset] = val;
            }
            0x03 => {
                let offset = (addr & 0x7FFF) as usize;
                self.iwram[offset] = val;
            }
            0x04 => {
                let offset = (addr & 0x3FF) as usize;
                if offset <= 0x56 {
                    self.ppu.write_io(addr, val);
                } else if offset < self.io.len() {
                    self.io[offset] = val;
                }
            }
            0x05 => {
                let offset = (addr & 0x3FF) as usize;
                if offset < self.ppu.palette.len() {
                    self.ppu.palette[offset] = val;
                }
            }
            0x06 => {
                let mut offset = (addr & 0x1FFFF) as usize;
                if offset >= self.ppu.vram.len() {
                    offset -= 0x8000;
                }
                if offset < self.ppu.vram.len() {
                    self.ppu.vram[offset] = val;
                }
            }
            0x07 => {
                let offset = (addr & 0x3FF) as usize;
                if offset < self.ppu.oam.len() {
                    self.ppu.oam[offset] = val;
                }
            }
            0x08..=0x0D => {
                // Game Pak ROM is read-only
            }
            0x0E | 0x0F => {
                let offset = (addr & 0xFFFF) as usize;
                if offset < self.sram.len() {
                    self.sram[offset] = val;
                }
            }
            _ => {}
        }
    }

    /// Read a 16-bit halfword (Little-Endian) from memory.
    pub fn read_u16(&self, addr: u32) -> u16 {
        let offset = (addr & 0x3FFFF) as usize;
        match addr >> 24 {
            0x02 if offset + 1 < self.ewram.len() => {
                u16::from_le_bytes([self.ewram[offset], self.ewram[offset + 1]])
            }
            0x03 => {
                let iwram_off = (addr & 0x7FFF) as usize;
                if iwram_off + 1 < self.iwram.len() {
                    u16::from_le_bytes([self.iwram[iwram_off], self.iwram[iwram_off + 1]])
                } else {
                    self.fallback_read_u16(addr)
                }
            }
            0x08..=0x0D => {
                let rom_off = (addr & 0x01FFFFFF) as usize;
                if rom_off + 1 < self.rom.len() {
                    u16::from_le_bytes([self.rom[rom_off], self.rom[rom_off + 1]])
                } else {
                    self.fallback_read_u16(addr)
                }
            }
            _ => self.fallback_read_u16(addr),
        }
    }

    fn fallback_read_u16(&self, addr: u32) -> u16 {
        let b0 = self.read_u8(addr) as u16;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u16;
        b0 | (b1 << 8)
    }

    /// Write a 16-bit halfword (Little-Endian) to memory.
    pub fn write_u16(&mut self, addr: u32, val: u16) {
        let bytes = val.to_le_bytes();
        let offset = (addr & 0x3FFFF) as usize;
        match addr >> 24 {
            0x02 if offset + 1 < self.ewram.len() => {
                self.ewram[offset] = bytes[0];
                self.ewram[offset + 1] = bytes[1];
            }
            0x03 => {
                let iwram_off = (addr & 0x7FFF) as usize;
                if iwram_off + 1 < self.iwram.len() {
                    self.iwram[iwram_off] = bytes[0];
                    self.iwram[iwram_off + 1] = bytes[1];
                } else {
                    self.fallback_write_u16(addr, val);
                }
            }
            _ => self.fallback_write_u16(addr, val),
        }
    }

    fn fallback_write_u16(&mut self, addr: u32, val: u16) {
        self.write_u8(addr, val as u8);
        self.write_u8(addr.wrapping_add(1), (val >> 8) as u8);
    }

    /// Read a 32-bit word (Little-Endian) from memory.
    pub fn read_u32(&self, addr: u32) -> u32 {
        let offset = (addr & 0x3FFFF) as usize;
        match addr >> 24 {
            0x02 if offset + 3 < self.ewram.len() => {
                u32::from_le_bytes(self.ewram[offset..offset + 4].try_into().unwrap())
            }
            0x03 => {
                let iwram_off = (addr & 0x7FFF) as usize;
                if iwram_off + 3 < self.iwram.len() {
                    u32::from_le_bytes(self.iwram[iwram_off..iwram_off + 4].try_into().unwrap())
                } else {
                    self.fallback_read_u32(addr)
                }
            }
            0x08..=0x0D => {
                let rom_off = (addr & 0x01FFFFFF) as usize;
                if rom_off + 3 < self.rom.len() {
                    u32::from_le_bytes(self.rom[rom_off..rom_off + 4].try_into().unwrap())
                } else {
                    self.fallback_read_u32(addr)
                }
            }
            _ => self.fallback_read_u32(addr),
        }
    }

    fn fallback_read_u32(&self, addr: u32) -> u32 {
        let b0 = self.read_u8(addr) as u32;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u32;
        let b2 = self.read_u8(addr.wrapping_add(2)) as u32;
        let b3 = self.read_u8(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Write a 32-bit word (Little-Endian) to memory.
    pub fn write_u32(&mut self, addr: u32, val: u32) {
        let bytes = val.to_le_bytes();
        let offset = (addr & 0x3FFFF) as usize;
        match addr >> 24 {
            0x02 if offset + 3 < self.ewram.len() => {
                self.ewram[offset..offset + 4].copy_from_slice(&bytes);
            }
            0x03 => {
                let iwram_off = (addr & 0x7FFF) as usize;
                if iwram_off + 3 < self.iwram.len() {
                    self.iwram[iwram_off..iwram_off + 4].copy_from_slice(&bytes);
                } else {
                    self.fallback_write_u32(addr, val);
                }
            }
            _ => self.fallback_write_u32(addr, val),
        }
    }

    fn fallback_write_u32(&mut self, addr: u32, val: u32) {
        self.write_u8(addr, val as u8);
        self.write_u8(addr.wrapping_add(1), (val >> 8) as u8);
        self.write_u8(addr.wrapping_add(2), (val >> 16) as u8);
        self.write_u8(addr.wrapping_add(3), (val >> 24) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewram_rw() {
        let mut bus = GbaMemoryBus::new();
        bus.write_u8(0x02000000, 0xAB);
        assert_eq!(bus.read_u8(0x02000000), 0xAB);

        bus.write_u16(0x02000010, 0x1234);
        assert_eq!(bus.read_u16(0x02000010), 0x1234);
        assert_eq!(bus.read_u8(0x02000010), 0x34);
        assert_eq!(bus.read_u8(0x02000011), 0x12);

        bus.write_u32(0x02000020, 0xDEADBEEF);
        assert_eq!(bus.read_u32(0x02000020), 0xDEADBEEF);
    }

    #[test]
    fn test_iwram_rw() {
        let mut bus = GbaMemoryBus::new();
        bus.write_u32(0x03007FFC, 0xCAFEBABE);
        assert_eq!(bus.read_u32(0x03007FFC), 0xCAFEBABE);
    }

    #[test]
    fn test_vram_and_palette() {
        let mut bus = GbaMemoryBus::new();
        bus.write_u16(0x05000000, 0x7C00); // Red palette entry
        assert_eq!(bus.read_u16(0x05000000), 0x7C00);

        bus.write_u16(0x06000000, 0x03E0); // Green pixel
        assert_eq!(bus.read_u16(0x06000000), 0x03E0);
    }

    #[test]
    fn test_ppu_io_registers() {
        let mut bus = GbaMemoryBus::new();
        bus.write_u16(0x04000000, 0x0003); // DISPCNT Mode 3
        assert_eq!(bus.read_u16(0x04000000), 3);
    }

    #[test]
    fn test_rom_waitstates_mirroring() {
        let mut bus = GbaMemoryBus::new();
        let test_rom = vec![0xEA, 0x00, 0x00, 0x2E, 0x24, 0xFF, 0xAE, 0x51];
        bus.load_rom(&test_rom);

        // Waitstate 0 (0x08000000)
        assert_eq!(bus.read_u32(0x08000000), 0x2E0000EA);
        // Waitstate 1 (0x0A000000)
        assert_eq!(bus.read_u32(0x0A000000), 0x2E0000EA);
        // Waitstate 2 (0x0C000000)
        assert_eq!(bus.read_u32(0x0C000000), 0x2E0000EA);
    }

    #[test]
    fn test_sram_rw() {
        let mut bus = GbaMemoryBus::new();
        bus.write_u8(0x0E000000, 0x55);
        assert_eq!(bus.read_u8(0x0E000000), 0x55);
    }
}

#![allow(dead_code)]

use super::dma::GbaDma;
use super::keypad::GbaKeypad;
use super::ppu::GbaPpu;

pub const BIOS_SIZE: usize = 16 * 1024;   // 16 KB (0x00000000 - 0x00003FFF)
pub const EWRAM_SIZE: usize = 256 * 1024; // 256 KB (0x02000000 - 0x0203FFFF)
pub const IWRAM_SIZE: usize = 32 * 1024;  // 32 KB (0x03000000 - 0x03007FFF)
pub const IO_SIZE: usize = 1024;          // 1 KB (0x04000000 - 0x040003FE)
pub const SRAM_SIZE: usize = 64 * 1024;   // 64 KB (0x0E000000 - 0x0E00FFFF)

/// GBA 32-bit Memory Bus (MMU) handling mapping, byte/halfword/word access,
/// PPU, Keypad, DMA integration, and Game Pak ROM loading.
pub struct GbaMemoryBus {
    pub ppu: GbaPpu,
    pub keypad: GbaKeypad,
    pub dma: GbaDma,
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
        let mut bus = Self {
            ppu: GbaPpu::new(),
            keypad: GbaKeypad::new(),
            dma: GbaDma::new(),
            bios: vec![0; BIOS_SIZE],
            ewram: vec![0; EWRAM_SIZE],
            iwram: vec![0; IWRAM_SIZE],
            io: vec![0; IO_SIZE],
            rom: Vec::new(),
            sram: vec![0; SRAM_SIZE],
        };
        bus.init_hle_bios();
        bus
    }

    /// Initialize default HLE BIOS ARM IRQ vector at 0x00000018
    pub fn init_hle_bios(&mut self) {
        let irq_vector_code: [u32; 17] = [
            0xE92D400F, // 0x18: STMFD sp!, {r0-r3, r12, lr}
            0xE3A00604, // 0x1C: MOV r0, #0x04000000
            0xE5901200, // 0x20: LDR r1, [r0, #0x200]
            0xE0012821, // 0x24: AND r2, r1, r1, LSR #16
            0xE1C020B2, // 0x28: STRH r2, [r0, #0x202]
            0xE59F0024, // 0x2C: LDR r0, [pc, #36] -> 0x03007FF8
            0xE1D010B0, // 0x30: LDRH r1, [r0]
            0xE1811002, // 0x34: ORR r1, r1, r2
            0xE1C010B0, // 0x38: STRH r1, [r0]
            0xE5903004, // 0x3C: LDR r3, [r0, #4] -> 0x03007FFC
            0xE3530000, // 0x40: CMP r3, #0
            0x11A0E00F, // 0x44: MOVNE lr, pc
            0x112FFF13, // 0x48: BXNE r3
            0xE8BD400F, // 0x4C: LDMFD sp!, {r0-r3, r12, lr}
            0xE25EF004, // 0x50: SUBS pc, lr, #4
            0x00000000, // 0x54: NOP
            0x03007FF8, // 0x58: Literal pointer to BIOS flags 0x03007FF8
        ];

        for (i, &instr) in irq_vector_code.iter().enumerate() {
            let offset = 0x18 + i * 4;
            let bytes = instr.to_le_bytes();
            self.bios[offset..offset + 4].copy_from_slice(&bytes);
        }
    }

    /// Reset MMU peripherals and memory state
    pub fn reset(&mut self) {
        self.ppu.reset();
        self.keypad.reset();
        self.dma.reset();
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

    /// Execute transfer logic for a specified DMA channel
    pub fn execute_dma(&mut self, ch: usize) {
        let is_32bit = (self.dma.channels[ch].cnt_h & (1 << 10)) != 0;
        let dst_cnt = (self.dma.channels[ch].cnt_h >> 5) & 3;
        let src_cnt = (self.dma.channels[ch].cnt_h >> 7) & 3;
        let repeat = (self.dma.channels[ch].cnt_h & (1 << 9)) != 0;

        let raw_count = self.dma.channels[ch].cnt_l as usize;
        let count = if raw_count == 0 {
            if ch == 3 { 65536 } else { 16384 }
        } else {
            raw_count
        };

        let unit_bytes: u32 = if is_32bit { 4 } else { 2 };
        let mut src_addr = self.dma.channels[ch].internal_sad;
        let mut dst_addr = self.dma.channels[ch].internal_dad;

        for _ in 0..count {
            if is_32bit {
                let val = self.read_u32(src_addr);
                self.write_u32(dst_addr, val);
            } else {
                let val = self.read_u16(src_addr);
                self.write_u16(dst_addr, val);
            }

            match src_cnt {
                0 => src_addr = src_addr.wrapping_add(unit_bytes), // Increment
                1 => src_addr = src_addr.wrapping_sub(unit_bytes), // Decrement
                2 => {}                                           // Fixed
                _ => src_addr = src_addr.wrapping_add(unit_bytes),
            }

            match dst_cnt {
                0 | 3 => dst_addr = dst_addr.wrapping_add(unit_bytes), // Increment / Increment-Reload
                1 => dst_addr = dst_addr.wrapping_sub(unit_bytes),     // Decrement
                2 => {}                                               // Fixed
                _ => {}
            }
        }

        self.dma.channels[ch].internal_sad = src_addr;
        if dst_cnt == 3 {
            self.dma.channels[ch].internal_dad = self.dma.channels[ch].dad; // Reload initial destination
        } else {
            self.dma.channels[ch].internal_dad = dst_addr;
        }

        if !repeat {
            self.dma.channels[ch].cnt_h &= !(1 << 15); // Clear Enable bit upon completion
        }
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
                match addr {
                    0x04000000..=0x04000056 => self.ppu.read_io(addr),
                    0x040000B0..=0x040000DF => self.dma.read_u8(addr),
                    0x04000130..=0x04000133 => self.keypad.read_u8(addr),
                    _ => {
                        if offset < self.io.len() {
                            self.io[offset]
                        } else {
                            0
                        }
                    }
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
                if offset >= 0x18000 {
                    offset &= 0x17FFF;
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
                match addr {
                    0x04000000..=0x04000056 => self.ppu.write_io(addr, val),
                    0x040000B0..=0x040000DF => {
                        if let Some(ch) = self.dma.write_u8(addr, val) {
                            self.execute_dma(ch);
                        }
                    }
                    0x04000130..=0x04000133 => self.keypad.write_u8(addr, val),
                    0x04000202 => {
                        let old = self.io[0x202];
                        self.io[0x202] = old & !val;
                    }
                    0x04000203 => {
                        let old = self.io[0x203];
                        self.io[0x203] = old & !val;
                    }
                    _ => {
                        if offset < self.io.len() {
                            self.io[offset] = val;
                        }
                    }
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
                if offset >= 0x18000 {
                    offset &= 0x17FFF;
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
        match addr >> 24 {
            0x02 => {
                let off = (addr & 0x3FFFF) as usize;
                let b0 = self.ewram[off];
                let b1 = self.ewram[(off + 1) & 0x3FFFF];
                u16::from_le_bytes([b0, b1])
            }
            0x03 => {
                let off = (addr & 0x7FFF) as usize;
                let b0 = self.iwram[off];
                let b1 = self.iwram[(off + 1) & 0x7FFF];
                u16::from_le_bytes([b0, b1])
            }
            0x08..=0x0D => {
                let rom_off = (addr & 0x01FFFFFF) as usize;
                if rom_off + 1 < self.rom.len() {
                    let low = self.rom[rom_off] as u16;
                    let high = self.rom[rom_off + 1] as u16;
                    (high << 8) | low
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
        match addr >> 24 {
            0x02 => {
                let off = (addr & 0x3FFFF) as usize;
                self.ewram[off] = bytes[0];
                self.ewram[(off + 1) & 0x3FFFF] = bytes[1];
            }
            0x03 => {
                let off = (addr & 0x7FFF) as usize;
                self.iwram[off] = bytes[0];
                self.iwram[(off + 1) & 0x7FFF] = bytes[1];
            }
            0x04 => {
                if addr == 0x04000202 {
                    let old_if = self.read_u16(0x04000202);
                    let new_if = old_if & !val;
                    self.io[0x202] = new_if as u8;
                    self.io[0x203] = (new_if >> 8) as u8;
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

    /// Read a 32-bit word (Little-Endian) from memory with ARM unaligned ROR rotation.
    pub fn read_u32(&self, addr: u32) -> u32 {
        let unaligned_shift = addr & 3;
        if unaligned_shift != 0 {
            let aligned_val = self.read_u32(addr & !3);
            return aligned_val.rotate_right(unaligned_shift * 8);
        }

        match addr >> 24 {
            0x02 => {
                let off = (addr & 0x3FFFF) as usize;
                let b0 = self.ewram[off];
                let b1 = self.ewram[(off + 1) & 0x3FFFF];
                let b2 = self.ewram[(off + 2) & 0x3FFFF];
                let b3 = self.ewram[(off + 3) & 0x3FFFF];
                u32::from_le_bytes([b0, b1, b2, b3])
            }
            0x03 => {
                let off = (addr & 0x7FFF) as usize;
                let b0 = self.iwram[off];
                let b1 = self.iwram[(off + 1) & 0x7FFF];
                let b2 = self.iwram[(off + 2) & 0x7FFF];
                let b3 = self.iwram[(off + 3) & 0x7FFF];
                u32::from_le_bytes([b0, b1, b2, b3])
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
        match addr >> 24 {
            0x02 => {
                let off = (addr & 0x3FFFF) as usize;
                self.ewram[off] = bytes[0];
                self.ewram[(off + 1) & 0x3FFFF] = bytes[1];
                self.ewram[(off + 2) & 0x3FFFF] = bytes[2];
                self.ewram[(off + 3) & 0x3FFFF] = bytes[3];
            }
            0x03 => {
                let off = (addr & 0x7FFF) as usize;
                self.iwram[off] = bytes[0];
                self.iwram[(off + 1) & 0x7FFF] = bytes[1];
                self.iwram[(off + 2) & 0x7FFF] = bytes[2];
                self.iwram[(off + 3) & 0x7FFF] = bytes[3];
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
    fn test_dma_immediate_transfer() {
        let mut bus = GbaMemoryBus::new();

        // Fill EWRAM 0x02000000 with pattern
        bus.write_u32(0x02000000, 0x11223344);
        bus.write_u32(0x02000004, 0x55667788);

        // Setup DMA3: Source 0x02000000, Dest 0x03000000 (IWRAM), Count 2 words
        bus.write_u32(0x040000D4, 0x02000000); // SAD
        bus.write_u32(0x040000D8, 0x03000000); // DAD
        bus.write_u16(0x040000DC, 2);          // CNT_L = 2
        // CNT_H: Enable (bit 15), 32-bit (bit 10), Immediate (bits 12-13 = 0) => 0x8400
        bus.write_u16(0x040000DE, 0x8400);

        // Verify DMA transferred the 2 words into IWRAM
        assert_eq!(bus.read_u32(0x03000000), 0x11223344);
        assert_eq!(bus.read_u32(0x03000004), 0x55667788);
    }

    #[test]
    fn test_keypad_io_mapping() {
        let mut bus = GbaMemoryBus::new();
        assert_eq!(bus.read_u16(0x04000130), 0x03FF); // KEYINPUT active-low default
        bus.keypad.handle_input(crate::core::Button::A, true);
        assert_eq!(bus.read_u16(0x04000130), 0x03FE);
    }
}

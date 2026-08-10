use super::mmu::MemoryBus;
use log::warn;

/// 8-bit registers and 16-bit register pairs for Sharp LR35902 CPU.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

#[allow(dead_code)]
impl Registers {
    /// Initializes default registers to DMG post-boot values.
    pub fn new() -> Self {
        Self {
            a: 0x01,
            f: 0xB0, // Z=1, N=0, H=1, C=1 (lower 4 bits always 0)
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100, // Standard entry point after boot ROM
        }
    }

    // 16-bit register pair getters and setters
    pub fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }

    pub fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = (val & 0xF0) as u8; // Lower 4 bits of F are always 0
    }

    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = (val & 0xFF) as u8;
    }

    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = (val & 0xFF) as u8;
    }

    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = (val & 0xFF) as u8;
    }

    // Flag Getters (F register bit mapping)
    pub fn flag_z(&self) -> bool {
        (self.f & 0x80) != 0
    }

    pub fn flag_n(&self) -> bool {
        (self.f & 0x40) != 0
    }

    pub fn flag_h(&self) -> bool {
        (self.f & 0x20) != 0
    }

    pub fn flag_c(&self) -> bool {
        (self.f & 0x10) != 0
    }

    // Flag Setters
    pub fn set_flag_z(&mut self, val: bool) {
        if val {
            self.f |= 0x80;
        } else {
            self.f &= !0x80;
        }
    }

    pub fn set_flag_n(&mut self, val: bool) {
        if val {
            self.f |= 0x40;
        } else {
            self.f &= !0x40;
        }
    }

    pub fn set_flag_h(&mut self, val: bool) {
        if val {
            self.f |= 0x20;
        } else {
            self.f &= !0x20;
        }
    }

    pub fn set_flag_c(&mut self, val: bool) {
        if val {
            self.f |= 0x10;
        } else {
            self.f &= !0x10;
        }
    }

    pub fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        self.set_flag_z(z);
        self.set_flag_n(n);
        self.set_flag_h(h);
        self.set_flag_c(c);
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

/// Sharp LR35902 CPU emulator core.
pub struct Cpu {
    pub registers: Registers,
    pub halted: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            halted: false,
        }
    }

    /// Fetches the next 8-bit immediate byte at PC and advances PC.
    fn fetch_u8(&mut self, bus: &MemoryBus) -> u8 {
        let val = bus.read_byte(self.registers.pc);
        self.registers.pc = self.registers.pc.wrapping_add(1);
        val
    }

    /// Fetches the next 16-bit immediate word at PC (little-endian) and advances PC by 2.
    fn fetch_u16(&mut self, bus: &MemoryBus) -> u16 {
        let low = self.fetch_u8(bus) as u16;
        let high = self.fetch_u8(bus) as u16;
        (high << 8) | low
    }

    /// Executes a single CPU instruction step and returns elapsed T-cycle count.
    pub fn step(&mut self, bus: &mut MemoryBus) -> u8 {
        if self.halted {
            return 4;
        }

        let opcode = self.fetch_u8(bus);

        match opcode {
            // NOP (4 T-cycles)
            0x00 => 4,

            // LD BC, nn (12 T-cycles)
            0x01 => {
                let nn = self.fetch_u16(bus);
                self.registers.set_bc(nn);
                12
            }

            // LD B, n (8 T-cycles)
            0x06 => {
                self.registers.b = self.fetch_u8(bus);
                8
            }

            // LD C, n (8 T-cycles)
            0x0E => {
                self.registers.c = self.fetch_u8(bus);
                8
            }

            // LD DE, nn (12 T-cycles)
            0x11 => {
                let nn = self.fetch_u16(bus);
                self.registers.set_de(nn);
                12
            }

            // LD D, n (8 T-cycles)
            0x16 => {
                self.registers.d = self.fetch_u8(bus);
                8
            }

            // JR n (12 T-cycles)
            0x18 => {
                let offset = self.fetch_u8(bus) as i8;
                self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                12
            }

            // LD E, n (8 T-cycles)
            0x1E => {
                self.registers.e = self.fetch_u8(bus);
                8
            }

            // LD HL, nn (12 T-cycles)
            0x21 => {
                let nn = self.fetch_u16(bus);
                self.registers.set_hl(nn);
                12
            }

            // LD H, n (8 T-cycles)
            0x26 => {
                self.registers.h = self.fetch_u8(bus);
                8
            }

            // JR NZ, n (12 T-cycles if branch taken, 8 if not)
            0x20 => {
                let offset = self.fetch_u8(bus) as i8;
                if !self.registers.flag_z() {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                    12
                } else {
                    8
                }
            }

            // JR Z, n (12 T-cycles if branch taken, 8 if not)
            0x28 => {
                let offset = self.fetch_u8(bus) as i8;
                if self.registers.flag_z() {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                    12
                } else {
                    8
                }
            }

            // LD L, n (8 T-cycles)
            0x2E => {
                self.registers.l = self.fetch_u8(bus);
                8
            }

            // LD SP, nn (12 T-cycles)
            0x31 => {
                self.registers.sp = self.fetch_u16(bus);
                12
            }

            // LD A, n (8 T-cycles)
            0x3E => {
                self.registers.a = self.fetch_u8(bus);
                8
            }

            // HALT (4 T-cycles)
            0x76 => {
                self.halted = true;
                4
            }

            // XOR A (4 T-cycles)
            0xAF => {
                self.registers.a ^= self.registers.a;
                self.registers.set_flags(true, false, false, false);
                4
            }

            // JP nn (16 T-cycles)
            0xC3 => {
                let nn = self.fetch_u16(bus);
                self.registers.pc = nn;
                16
            }

            // Unimplemented / fallback opcode
            _ => {
                warn!(
                    "Unimplemented Opcode 0x{:02X} at PC: 0x{:04X}",
                    opcode,
                    self.registers.pc.wrapping_sub(1)
                );
                4
            }
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

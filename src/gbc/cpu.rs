use super::mmu::MemoryBus;
use log::warn;

/// 8-bit registers and 16-bit register pairs for Sharp LR35902 CPU.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

    /// Initializes CGB hardware post-boot register values (A = 0x11 indicates CGB hardware).
    pub fn init_gbc_defaults(&mut self) {
        self.a = 0x11;
        self.b = 0x00;
        self.c = 0x00;
        self.d = 0x00;
        self.e = 0x08;
        self.h = 0x00;
        self.l = 0x7C;
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cpu {
    pub registers: Registers,
    pub halted: bool,
    pub ime: bool, // Interrupt Master Enable
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            halted: false,
            ime: false,
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

    /// Pushes a 16-bit word onto the stack.
    fn push_u16(&mut self, bus: &mut MemoryBus, val: u16) {
        let high = (val >> 8) as u8;
        let low = (val & 0xFF) as u8;
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        bus.write_byte(self.registers.sp, high);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        bus.write_byte(self.registers.sp, low);
    }

    /// Pops a 16-bit word off the stack.
    fn pop_u16(&mut self, bus: &MemoryBus) -> u16 {
        let low = bus.read_byte(self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);
        let high = bus.read_byte(self.registers.sp) as u16;
        self.registers.sp = self.registers.sp.wrapping_add(1);
        (high << 8) | low
    }

    /// Reads an 8-bit operand specified by register index 0..7.
    fn read_r8(&self, bus: &MemoryBus, reg_idx: u8) -> u8 {
        match reg_idx & 7 {
            0 => self.registers.b,
            1 => self.registers.c,
            2 => self.registers.d,
            3 => self.registers.e,
            4 => self.registers.h,
            5 => self.registers.l,
            6 => bus.read_byte(self.registers.hl()),
            7 => self.registers.a,
            _ => unreachable!(),
        }
    }

    /// Writes an 8-bit operand specified by register index 0..7.
    fn write_r8(&mut self, bus: &mut MemoryBus, reg_idx: u8, val: u8) {
        match reg_idx & 7 {
            0 => self.registers.b = val,
            1 => self.registers.c = val,
            2 => self.registers.d = val,
            3 => self.registers.e = val,
            4 => self.registers.h = val,
            5 => self.registers.l = val,
            6 => bus.write_byte(self.registers.hl(), val),
            7 => self.registers.a = val,
            _ => unreachable!(),
        }
    }

    // ALU Operations
    fn add_a(&mut self, val: u8) {
        let a = self.registers.a;
        let (res, overflow) = a.overflowing_add(val);
        let h = ((a & 0x0F) + (val & 0x0F)) > 0x0F;
        self.registers.a = res;
        self.registers.set_flags(res == 0, false, h, overflow);
    }

    fn adc_a(&mut self, val: u8) {
        let a = self.registers.a;
        let c = if self.registers.flag_c() { 1 } else { 0 };
        let sum = (a as u16) + (val as u16) + (c as u16);
        let res = sum as u8;
        let h = ((a & 0x0F) + (val & 0x0F) + c) > 0x0F;
        self.registers.a = res;
        self.registers.set_flags(res == 0, false, h, sum > 0xFF);
    }

    fn sub_a(&mut self, val: u8) {
        let a = self.registers.a;
        let (res, overflow) = a.overflowing_sub(val);
        let h = (a & 0x0F) < (val & 0x0F);
        self.registers.a = res;
        self.registers.set_flags(res == 0, true, h, overflow);
    }

    fn sbc_a(&mut self, val: u8) {
        let a = self.registers.a;
        let c = if self.registers.flag_c() { 1 } else { 0 };
        let sub = (a as i16) - (val as i16) - (c as i16);
        let res = sub as u8;
        let h = ((a & 0x0F) as i16 - (val & 0x0F) as i16 - c as i16) < 0;
        self.registers.a = res;
        self.registers.set_flags(res == 0, true, h, sub < 0);
    }

    fn and_a(&mut self, val: u8) {
        self.registers.a &= val;
        let a = self.registers.a;
        self.registers.set_flags(a == 0, false, true, false);
    }

    fn xor_a(&mut self, val: u8) {
        self.registers.a ^= val;
        let a = self.registers.a;
        self.registers.set_flags(a == 0, false, false, false);
    }

    fn or_a(&mut self, val: u8) {
        self.registers.a |= val;
        let a = self.registers.a;
        self.registers.set_flags(a == 0, false, false, false);
    }

    fn cp_a(&mut self, val: u8) {
        let a = self.registers.a;
        let (res, overflow) = a.overflowing_sub(val);
        let h = (a & 0x0F) < (val & 0x0F);
        self.registers.set_flags(res == 0, true, h, overflow);
    }

    fn daa(&mut self) {
        let mut a = self.registers.a;
        let mut adjust = 0;
        let mut carry = self.registers.flag_c();

        if self.registers.flag_h() || (!self.registers.flag_n() && (a & 0x0F) > 0x09) {
            adjust |= 0x06;
        }
        if self.registers.flag_c() || (!self.registers.flag_n() && a > 0x99) {
            adjust |= 0x60;
            carry = true;
        }

        if self.registers.flag_n() {
            a = a.wrapping_sub(adjust);
        } else {
            a = a.wrapping_add(adjust);
        }

        self.registers.a = a;
        self.registers.set_flag_z(a == 0);
        self.registers.set_flag_h(false);
        self.registers.set_flag_c(carry);
    }

    /// Executes 0xCB prefix bitwise opcodes.
    fn execute_cb(&mut self, bus: &mut MemoryBus) -> u8 {
        let cb_op = self.fetch_u8(bus);
        let reg_idx = cb_op & 0x07;
        let bit = (cb_op >> 3) & 0x07;
        let mut cycles = if reg_idx == 6 { 16 } else { 8 };

        let val = self.read_r8(bus, reg_idx);

        match cb_op {
            // RLC r (0x00..=0x07)
            0x00..=0x07 => {
                let carry = (val & 0x80) != 0;
                let res = (val << 1) | (if carry { 1 } else { 0 });
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, carry);
            }
            // RRC r (0x08..=0x0F)
            0x08..=0x0F => {
                let carry = (val & 0x01) != 0;
                let res = (val >> 1) | (if carry { 0x80 } else { 0 });
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, carry);
            }
            // RL r (0x10..=0x17)
            0x10..=0x17 => {
                let old_carry = self.registers.flag_c();
                let new_carry = (val & 0x80) != 0;
                let res = (val << 1) | (if old_carry { 1 } else { 0 });
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, new_carry);
            }
            // RR r (0x18..=0x1F)
            0x18..=0x1F => {
                let old_carry = self.registers.flag_c();
                let new_carry = (val & 0x01) != 0;
                let res = (val >> 1) | (if old_carry { 0x80 } else { 0 });
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, new_carry);
            }
            // SLA r (0x20..=0x27)
            0x20..=0x27 => {
                let carry = (val & 0x80) != 0;
                let res = val << 1;
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, carry);
            }
            // SRA r (0x28..=0x2F)
            0x28..=0x2F => {
                let carry = (val & 0x01) != 0;
                let res = (val >> 1) | (val & 0x80);
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, carry);
            }
            // SWAP r (0x30..=0x37)
            0x30..=0x37 => {
                let res = val.rotate_left(4);
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, false);
            }
            // SRL r (0x38..=0x3F)
            0x38..=0x3F => {
                let carry = (val & 0x01) != 0;
                let res = val >> 1;
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flags(res == 0, false, false, carry);
            }
            // BIT b, r (0x40..=0x7F)
            0x40..=0x7F => {
                if reg_idx == 6 {
                    cycles = 12;
                }
                let is_zero = (val & (1 << bit)) == 0;
                self.registers.set_flag_z(is_zero);
                self.registers.set_flag_n(false);
                self.registers.set_flag_h(true);
            }
            // RES b, r (0x80..=0xBF)
            0x80..=0xBF => {
                let res = val & !(1 << bit);
                self.write_r8(bus, reg_idx, res);
            }
            // SET b, r (0xC0..=0xFF)
            0xC0..=0xFF => {
                let res = val | (1 << bit);
                self.write_r8(bus, reg_idx, res);
            }
        }

        cycles
    }

    /// Executes a single CPU instruction step and returns elapsed T-cycle count.
    pub fn step(&mut self, bus: &mut MemoryBus) -> u8 {
        // Check for pending hardware interrupts (IE: 0xFFFF, IF: 0xFF0F)
        let ie = bus.read_byte(0xFFFF);
        let if_reg = bus.read_byte(0xFF0F);
        let pending = ie & if_reg & 0x1F;

        if pending != 0 {
            self.halted = false; // Pending interrupt un-halts CPU

            if self.ime {
                self.ime = false;
                let bit = pending.trailing_zeros() as u8;
                bus.write_byte(0xFF0F, if_reg & !(1 << bit));
                self.push_u16(bus, self.registers.pc);
                self.registers.pc = 0x0040 + (bit as u16 * 8);
                return 20; // Interrupt service routine entry takes 20 T-cycles
            }
        }

        if self.halted {
            return 4;
        }

        let opcode = self.fetch_u8(bus);

        match opcode {
            // NOP (4 T-cycles)
            0x00 => 4,

            // STOP (2-byte instruction)
            0x10 => {
                let _ = self.fetch_u8(bus);
                bus.switch_speed_if_prepared();
                4
            }

            // 16-bit Immediate Loads (LD rr, nn)
            0x01 => {
                let nn = self.fetch_u16(bus);
                self.registers.set_bc(nn);
                12
            }
            0x11 => {
                let nn = self.fetch_u16(bus);
                self.registers.set_de(nn);
                12
            }
            0x21 => {
                let nn = self.fetch_u16(bus);
                self.registers.set_hl(nn);
                12
            }
            0x31 => {
                self.registers.sp = self.fetch_u16(bus);
                12
            }

            // 8-bit Immediate Loads (LD r, n)
            0x06 => {
                self.registers.b = self.fetch_u8(bus);
                8
            }
            0x0E => {
                self.registers.c = self.fetch_u8(bus);
                8
            }
            0x16 => {
                self.registers.d = self.fetch_u8(bus);
                8
            }
            0x1E => {
                self.registers.e = self.fetch_u8(bus);
                8
            }
            0x26 => {
                self.registers.h = self.fetch_u8(bus);
                8
            }
            0x2E => {
                self.registers.l = self.fetch_u8(bus);
                8
            }
            0x36 => {
                let n = self.fetch_u8(bus);
                bus.write_byte(self.registers.hl(), n);
                12
            }
            0x3E => {
                self.registers.a = self.fetch_u8(bus);
                8
            }

            // 8-bit Register-to-Register Loads (LD r, r')
            0x40..=0x75 | 0x77..=0x7F => {
                let src = opcode & 7;
                let dest = (opcode >> 3) & 7;
                let val = self.read_r8(bus, src);
                self.write_r8(bus, dest, val);
                if src == 6 || dest == 6 {
                    8
                } else {
                    4
                }
            }

            // 8-bit Increment / Decrement
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let reg_idx = (opcode >> 3) & 7;
                let val = self.read_r8(bus, reg_idx);
                let res = val.wrapping_add(1);
                let h = (val & 0x0F) == 0x0F;
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flag_z(res == 0);
                self.registers.set_flag_n(false);
                self.registers.set_flag_h(h);
                if reg_idx == 6 {
                    12
                } else {
                    4
                }
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let reg_idx = (opcode >> 3) & 7;
                let val = self.read_r8(bus, reg_idx);
                let res = val.wrapping_sub(1);
                let h = (val & 0x0F) == 0x00;
                self.write_r8(bus, reg_idx, res);
                self.registers.set_flag_z(res == 0);
                self.registers.set_flag_n(true);
                self.registers.set_flag_h(h);
                if reg_idx == 6 {
                    12
                } else {
                    4
                }
            }

            // 16-bit Increment / Decrement
            0x03 => {
                self.registers.set_bc(self.registers.bc().wrapping_add(1));
                8
            }
            0x13 => {
                self.registers.set_de(self.registers.de().wrapping_add(1));
                8
            }
            0x23 => {
                self.registers.set_hl(self.registers.hl().wrapping_add(1));
                8
            }
            0x33 => {
                self.registers.sp = self.registers.sp.wrapping_add(1);
                8
            }
            0x0B => {
                self.registers.set_bc(self.registers.bc().wrapping_sub(1));
                8
            }
            0x1B => {
                self.registers.set_de(self.registers.de().wrapping_sub(1));
                8
            }
            0x2B => {
                self.registers.set_hl(self.registers.hl().wrapping_sub(1));
                8
            }
            0x3B => {
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                8
            }

            // 16-bit ADD HL, rr
            0x09 | 0x19 | 0x29 | 0x39 => {
                let val = match opcode {
                    0x09 => self.registers.bc(),
                    0x19 => self.registers.de(),
                    0x29 => self.registers.hl(),
                    0x39 => self.registers.sp,
                    _ => unreachable!(),
                };
                let hl = self.registers.hl();
                let sum = (hl as u32) + (val as u32);
                let h = ((hl & 0x0FFF) + (val & 0x0FFF)) > 0x0FFF;
                self.registers.set_hl(sum as u16);
                self.registers.set_flag_n(false);
                self.registers.set_flag_h(h);
                self.registers.set_flag_c(sum > 0xFFFF);
                8
            }

            // Indirect Register Loads
            0x02 => {
                bus.write_byte(self.registers.bc(), self.registers.a);
                8
            }
            0x12 => {
                bus.write_byte(self.registers.de(), self.registers.a);
                8
            }
            0x0A => {
                self.registers.a = bus.read_byte(self.registers.bc());
                8
            }
            0x1A => {
                self.registers.a = bus.read_byte(self.registers.de());
                8
            }

            // Rotate Accumulator & Special Ops
            0x07 => {
                let a = self.registers.a;
                let carry = (a & 0x80) != 0;
                let res = (a << 1) | (if carry { 1 } else { 0 });
                self.registers.a = res;
                self.registers.set_flags(false, false, false, carry);
                4
            }
            0x0F => {
                let a = self.registers.a;
                let carry = (a & 0x01) != 0;
                let res = (a >> 1) | (if carry { 0x80 } else { 0 });
                self.registers.a = res;
                self.registers.set_flags(false, false, false, carry);
                4
            }
            0x17 => {
                let a = self.registers.a;
                let old_carry = self.registers.flag_c();
                let new_carry = (a & 0x80) != 0;
                let res = (a << 1) | (if old_carry { 1 } else { 0 });
                self.registers.a = res;
                self.registers.set_flags(false, false, false, new_carry);
                4
            }
            0x1F => {
                let a = self.registers.a;
                let old_carry = self.registers.flag_c();
                let new_carry = (a & 0x01) != 0;
                let res = (a >> 1) | (if old_carry { 0x80 } else { 0 });
                self.registers.a = res;
                self.registers.set_flags(false, false, false, new_carry);
                4
            }
            0x2F => {
                self.registers.a = !self.registers.a;
                self.registers.set_flag_n(true);
                self.registers.set_flag_h(true);
                4
            }
            0x37 => {
                self.registers.set_flag_n(false);
                self.registers.set_flag_h(false);
                self.registers.set_flag_c(true);
                4
            }
            0x3F => {
                let c = self.registers.flag_c();
                self.registers.set_flag_n(false);
                self.registers.set_flag_h(false);
                self.registers.set_flag_c(!c);
                4
            }

            // Relative Jumps
            0x18 => {
                let offset = self.fetch_u8(bus) as i8;
                self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                12
            }
            0x20 => {
                let offset = self.fetch_u8(bus) as i8;
                if !self.registers.flag_z() {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                    12
                } else {
                    8
                }
            }
            0x28 => {
                let offset = self.fetch_u8(bus) as i8;
                if self.registers.flag_z() {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                    12
                } else {
                    8
                }
            }
            0x30 => {
                let offset = self.fetch_u8(bus) as i8;
                if !self.registers.flag_c() {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                    12
                } else {
                    8
                }
            }
            0x38 => {
                let offset = self.fetch_u8(bus) as i8;
                if self.registers.flag_c() {
                    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
                    12
                } else {
                    8
                }
            }

            // Absolute Jumps (JP cc, nn)
            0xC3 => {
                let nn = self.fetch_u16(bus);
                self.registers.pc = nn;
                16
            }
            0xC2 => {
                let nn = self.fetch_u16(bus);
                if !self.registers.flag_z() {
                    self.registers.pc = nn;
                    16
                } else {
                    12
                }
            }
            0xCA => {
                let nn = self.fetch_u16(bus);
                if self.registers.flag_z() {
                    self.registers.pc = nn;
                    16
                } else {
                    12
                }
            }
            0xD2 => {
                let nn = self.fetch_u16(bus);
                if !self.registers.flag_c() {
                    self.registers.pc = nn;
                    16
                } else {
                    12
                }
            }
            0xDA => {
                let nn = self.fetch_u16(bus);
                if self.registers.flag_c() {
                    self.registers.pc = nn;
                    16
                } else {
                    12
                }
            }

            // Stack PUSH / POP
            0xC5 => {
                self.push_u16(bus, self.registers.bc());
                16
            }
            0xD5 => {
                self.push_u16(bus, self.registers.de());
                16
            }
            0xE5 => {
                self.push_u16(bus, self.registers.hl());
                16
            }
            0xF5 => {
                self.push_u16(bus, self.registers.af());
                16
            }
            0xC1 => {
                let val = self.pop_u16(bus);
                self.registers.set_bc(val);
                12
            }
            0xD1 => {
                let val = self.pop_u16(bus);
                self.registers.set_de(val);
                12
            }
            0xE1 => {
                let val = self.pop_u16(bus);
                self.registers.set_hl(val);
                12
            }
            0xF1 => {
                let val = self.pop_u16(bus);
                self.registers.set_af(val);
                12
            }

            // CALL / RET / RST
            0xCD => {
                let nn = self.fetch_u16(bus);
                self.push_u16(bus, self.registers.pc);
                self.registers.pc = nn;
                24
            }
            0xC4 => {
                let nn = self.fetch_u16(bus);
                if !self.registers.flag_z() {
                    self.push_u16(bus, self.registers.pc);
                    self.registers.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xCC => {
                let nn = self.fetch_u16(bus);
                if self.registers.flag_z() {
                    self.push_u16(bus, self.registers.pc);
                    self.registers.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xD4 => {
                let nn = self.fetch_u16(bus);
                if !self.registers.flag_c() {
                    self.push_u16(bus, self.registers.pc);
                    self.registers.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xDC => {
                let nn = self.fetch_u16(bus);
                if self.registers.flag_c() {
                    self.push_u16(bus, self.registers.pc);
                    self.registers.pc = nn;
                    24
                } else {
                    12
                }
            }
            0xC9 => {
                self.registers.pc = self.pop_u16(bus);
                16
            }
            0xC0 => {
                if !self.registers.flag_z() {
                    self.registers.pc = self.pop_u16(bus);
                    20
                } else {
                    8
                }
            }
            0xC8 => {
                if self.registers.flag_z() {
                    self.registers.pc = self.pop_u16(bus);
                    20
                } else {
                    8
                }
            }
            0xD0 => {
                if !self.registers.flag_c() {
                    self.registers.pc = self.pop_u16(bus);
                    20
                } else {
                    8
                }
            }
            0xD8 => {
                if self.registers.flag_c() {
                    self.registers.pc = self.pop_u16(bus);
                    20
                } else {
                    8
                }
            }
            0xD9 => {
                self.registers.pc = self.pop_u16(bus);
                self.ime = true;
                16
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let target = (opcode & 0x38) as u16;
                self.push_u16(bus, self.registers.pc);
                self.registers.pc = target;
                16
            }

            // Memory Loads & High RAM
            0x08 => {
                let nn = self.fetch_u16(bus);
                let sp = self.registers.sp;
                bus.write_byte(nn, (sp & 0xFF) as u8);
                bus.write_byte(nn.wrapping_add(1), (sp >> 8) as u8);
                20
            }
            0xEA => {
                let nn = self.fetch_u16(bus);
                bus.write_byte(nn, self.registers.a);
                16
            }
            0xFA => {
                let nn = self.fetch_u16(bus);
                self.registers.a = bus.read_byte(nn);
                16
            }
            0xE0 => {
                let n = self.fetch_u8(bus) as u16;
                bus.write_byte(0xFF00 + n, self.registers.a);
                12
            }
            0xF0 => {
                let n = self.fetch_u8(bus) as u16;
                self.registers.a = bus.read_byte(0xFF00 + n);
                12
            }
            0xE2 => {
                let c = self.registers.c as u16;
                bus.write_byte(0xFF00 + c, self.registers.a);
                8
            }
            0xF2 => {
                let c = self.registers.c as u16;
                self.registers.a = bus.read_byte(0xFF00 + c);
                8
            }
            0x22 => {
                let hl = self.registers.hl();
                bus.write_byte(hl, self.registers.a);
                self.registers.set_hl(hl.wrapping_add(1));
                8
            }
            0x32 => {
                let hl = self.registers.hl();
                bus.write_byte(hl, self.registers.a);
                self.registers.set_hl(hl.wrapping_sub(1));
                8
            }
            0x2A => {
                let hl = self.registers.hl();
                self.registers.a = bus.read_byte(hl);
                self.registers.set_hl(hl.wrapping_add(1));
                8
            }
            0x3A => {
                let hl = self.registers.hl();
                self.registers.a = bus.read_byte(hl);
                self.registers.set_hl(hl.wrapping_sub(1));
                8
            }
            0xE8 => {
                let n = self.fetch_u8(bus) as i8 as i16 as u16;
                let sp = self.registers.sp;
                let res = sp.wrapping_add(n);
                let h = ((sp & 0x0F) + (n & 0x0F)) > 0x0F;
                let c = ((sp & 0xFF) + (n & 0xFF)) > 0xFF;
                self.registers.sp = res;
                self.registers.set_flags(false, false, h, c);
                16
            }
            0xF8 => {
                let n = self.fetch_u8(bus) as i8 as i16 as u16;
                let sp = self.registers.sp;
                let res = sp.wrapping_add(n);
                let h = ((sp & 0x0F) + (n & 0x0F)) > 0x0F;
                let c = ((sp & 0xFF) + (n & 0xFF)) > 0xFF;
                self.registers.set_hl(res);
                self.registers.set_flags(false, false, h, c);
                12
            }
            0xF9 => {
                self.registers.sp = self.registers.hl();
                8
            }
            0xE9 => {
                self.registers.pc = self.registers.hl();
                4
            }

            // ALU Operations (Accumulator)
            0x80..=0x87 => {
                let val = self.read_r8(bus, opcode & 7);
                self.add_a(val);
                4
            }
            0x88..=0x8F => {
                let val = self.read_r8(bus, opcode & 7);
                self.adc_a(val);
                4
            }
            0x90..=0x97 => {
                let val = self.read_r8(bus, opcode & 7);
                self.sub_a(val);
                4
            }
            0x98..=0x9F => {
                let val = self.read_r8(bus, opcode & 7);
                self.sbc_a(val);
                4
            }
            0xA0..=0xA7 => {
                let val = self.read_r8(bus, opcode & 7);
                self.and_a(val);
                4
            }
            0xA8..=0xAF => {
                let val = self.read_r8(bus, opcode & 7);
                self.xor_a(val);
                4
            }
            0xB0..=0xB7 => {
                let val = self.read_r8(bus, opcode & 7);
                self.or_a(val);
                4
            }
            0xB8..=0xBF => {
                let val = self.read_r8(bus, opcode & 7);
                self.cp_a(val);
                4
            }

            // Immediate ALU
            0xC6 => {
                let n = self.fetch_u8(bus);
                self.add_a(n);
                8
            }
            0xCE => {
                let n = self.fetch_u8(bus);
                self.adc_a(n);
                8
            }
            0xD6 => {
                let n = self.fetch_u8(bus);
                self.sub_a(n);
                8
            }
            0xDE => {
                let n = self.fetch_u8(bus);
                self.sbc_a(n);
                8
            }
            0xE6 => {
                let n = self.fetch_u8(bus);
                self.and_a(n);
                8
            }
            0xEE => {
                let n = self.fetch_u8(bus);
                self.xor_a(n);
                8
            }
            0xF6 => {
                let n = self.fetch_u8(bus);
                self.or_a(n);
                8
            }
            0xFE => {
                let n = self.fetch_u8(bus);
                self.cp_a(n);
                8
            }

            // DAA, DI, EI, HALT
            0x27 => {
                self.daa();
                4
            }
            0xF3 => {
                self.ime = false;
                4
            }
            0xFB => {
                self.ime = true;
                4
            }
            0x76 => {
                self.halted = true;
                4
            }

            // 0xCB Prefix Decoder
            0xCB => self.execute_cb(bus),

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_pairs() {
        let mut reg = Registers::new();
        reg.set_bc(0x1234);
        assert_eq!(reg.b, 0x12);
        assert_eq!(reg.c, 0x34);
        assert_eq!(reg.bc(), 0x1234);

        reg.set_af(0xFFF0);
        assert_eq!(reg.a, 0xFF);
        assert_eq!(reg.f, 0xF0);
        assert_eq!(reg.af(), 0xFFF0);
    }

    #[test]
    fn test_cpu_xor_a() {
        let mut cpu = Cpu::new();
        let mut bus = MemoryBus::new();
        bus.load_rom(&[0xAF]); // XOR A opcode
        cpu.registers.pc = 0x0000;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.registers.a, 0x00);
        assert!(cpu.registers.flag_z());
    }

    #[test]
    fn test_cb_bit_and_set() {
        let mut cpu = Cpu::new();
        let mut bus = MemoryBus::new();
        // 0xCB 0xC7 (SET 0, A)
        bus.load_rom(&[0xCB, 0xC7]);
        cpu.registers.pc = 0x0000;
        cpu.registers.a = 0xFE;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.registers.a, 0xFF);
    }

    #[test]
    fn test_ld_r_r() {
        let mut cpu = Cpu::new();
        let mut bus = MemoryBus::new();
        // 0x47 is LD B, A
        bus.load_rom(&[0x47]);
        cpu.registers.pc = 0x0000;
        cpu.registers.a = 0x42;

        let cycles = cpu.step(&mut bus);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.registers.b, 0x42);
    }
}

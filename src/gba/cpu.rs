#![allow(dead_code)]

use super::mmu::GbaMemoryBus;

/// ARM7TDMI Operating Modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum CpuMode {
    User = 0x10,
    FIQ = 0x11,
    IRQ = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
    System = 0x1F,
}

impl CpuMode {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0x1F {
            0x10 => CpuMode::User,
            0x11 => CpuMode::FIQ,
            0x12 => CpuMode::IRQ,
            0x13 => CpuMode::Supervisor,
            0x17 => CpuMode::Abort,
            0x1B => CpuMode::Undefined,
            _ => CpuMode::System,
        }
    }

    pub fn bits(self) -> u32 {
        self as u32
    }
}

/// ARM7TDMI General Registers & CPSR Status Flags with Banked Registers
#[derive(Debug, Clone)]
pub struct Registers {
    /// General Purpose Registers r0-r15 (r13=SP, r14=LR, r15=PC)
    pub r: [u32; 16],

    /// Current Program Status Register (CPSR)
    pub cpsr: u32,

    // Banked Registers for Modes
    pub r13_sys: u32,
    pub r14_sys: u32,

    pub r8_fiq: u32,
    pub r9_fiq: u32,
    pub r10_fiq: u32,
    pub r11_fiq: u32,
    pub r12_fiq: u32,
    pub r13_fiq: u32,
    pub r14_fiq: u32,
    pub spsr_fiq: u32,

    pub r13_svc: u32,
    pub r14_svc: u32,
    pub spsr_svc: u32,

    pub r13_abt: u32,
    pub r14_abt: u32,
    pub spsr_abt: u32,

    pub r13_irq: u32,
    pub r14_irq: u32,
    pub spsr_irq: u32,

    pub r13_und: u32,
    pub r14_und: u32,
    pub spsr_und: u32,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    pub fn new() -> Self {
        let mut regs = Self {
            r: [0; 16],
            cpsr: 0x0000001F, // System mode by default, ARM mode, IRQ/FIQ enabled

            r13_sys: 0x03007F00,
            r14_sys: 0,

            r8_fiq: 0,
            r9_fiq: 0,
            r10_fiq: 0,
            r11_fiq: 0,
            r12_fiq: 0,
            r13_fiq: 0,
            r14_fiq: 0,
            spsr_fiq: 0x1F,

            r13_svc: 0x03007FE0,
            r14_svc: 0,
            spsr_svc: 0x1F,

            r13_abt: 0,
            r14_abt: 0,
            spsr_abt: 0x1F,

            r13_irq: 0x03007FA0,
            r14_irq: 0,
            spsr_irq: 0x1F,

            r13_und: 0,
            r14_und: 0,
            spsr_und: 0x1F,
        };
        regs.r[13] = 0x03007F00; // Default IWRAM Stack Pointer
        regs.r[15] = 0x08000000; // GBA Game Pak Entry Point (0x08000000)
        regs
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // --- Register Accessors ---
    pub fn pc(&self) -> u32 {
        self.r[15]
    }

    pub fn set_pc(&mut self, val: u32) {
        self.r[15] = val;
    }

    /// Set PC with automatic ARM/THUMB interworking (Bit 0 selects THUMB vs ARM mode)
    pub fn set_pc_interworking(&mut self, target_addr: u32) {
        self.set_thumb_mode((target_addr & 1) != 0);
        self.r[15] = target_addr & !1;
    }

    pub fn sp(&self) -> u32 {
        self.r[13]
    }

    pub fn set_sp(&mut self, val: u32) {
        self.r[13] = val;
    }

    pub fn lr(&self) -> u32 {
        self.r[14]
    }

    pub fn set_lr(&mut self, val: u32) {
        self.r[14] = val;
    }

    // --- CPSR Flag Helpers ---

    /// Negative Flag (N - Bit 31)
    pub fn n_flag(&self) -> bool {
        (self.cpsr & (1 << 31)) != 0
    }

    pub fn set_n_flag(&mut self, flag: bool) {
        if flag {
            self.cpsr |= 1 << 31;
        } else {
            self.cpsr &= !(1 << 31);
        }
    }

    /// Zero Flag (Z - Bit 30)
    pub fn z_flag(&self) -> bool {
        (self.cpsr & (1 << 30)) != 0
    }

    pub fn set_z_flag(&mut self, flag: bool) {
        if flag {
            self.cpsr |= 1 << 30;
        } else {
            self.cpsr &= !(1 << 30);
        }
    }

    /// Carry Flag (C - Bit 29)
    pub fn c_flag(&self) -> bool {
        (self.cpsr & (1 << 29)) != 0
    }

    pub fn set_c_flag(&mut self, flag: bool) {
        if flag {
            self.cpsr |= 1 << 29;
        } else {
            self.cpsr &= !(1 << 29);
        }
    }

    /// Overflow Flag (V - Bit 28)
    pub fn v_flag(&self) -> bool {
        (self.cpsr & (1 << 28)) != 0
    }

    pub fn set_v_flag(&mut self, flag: bool) {
        if flag {
            self.cpsr |= 1 << 28;
        } else {
            self.cpsr &= !(1 << 28);
        }
    }

    /// IRQ Disable (I - Bit 7)
    pub fn irq_disabled(&self) -> bool {
        (self.cpsr & (1 << 7)) != 0
    }

    pub fn set_irq_disabled(&mut self, disabled: bool) {
        if disabled {
            self.cpsr |= 1 << 7;
        } else {
            self.cpsr &= !(1 << 7);
        }
    }

    /// FIQ Disable (F - Bit 6)
    pub fn fiq_disabled(&self) -> bool {
        (self.cpsr & (1 << 6)) != 0
    }

    pub fn set_fiq_disabled(&mut self, disabled: bool) {
        if disabled {
            self.cpsr |= 1 << 6;
        } else {
            self.cpsr &= !(1 << 6);
        }
    }

    /// Execution State Bit (T - Bit 5): 0 = 32-bit ARM mode, 1 = 16-bit THUMB mode
    pub fn thumb_mode(&self) -> bool {
        (self.cpsr & (1 << 5)) != 0
    }

    pub fn set_thumb_mode(&mut self, thumb: bool) {
        if thumb {
            self.cpsr |= 1 << 5;
        } else {
            self.cpsr &= !(1 << 5);
        }
    }

    /// Current Operating Mode (M[4:0] - Bits 0-4)
    pub fn mode(&self) -> CpuMode {
        CpuMode::from_bits(self.cpsr & 0x1F)
    }

    /// Change CPU operating mode, automatically swapping banked registers.
    pub fn set_mode(&mut self, new_mode: CpuMode) {
        let current_mode = self.mode();
        if current_mode == new_mode {
            return;
        }

        // Save active registers to current mode's bank
        match current_mode {
            CpuMode::User | CpuMode::System => {
                self.r13_sys = self.r[13];
                self.r14_sys = self.r[14];
            }
            CpuMode::FIQ => {
                self.r8_fiq = self.r[8];
                self.r9_fiq = self.r[9];
                self.r10_fiq = self.r[10];
                self.r11_fiq = self.r[11];
                self.r12_fiq = self.r[12];
                self.r13_fiq = self.r[13];
                self.r14_fiq = self.r[14];
            }
            CpuMode::Supervisor => {
                self.r13_svc = self.r[13];
                self.r14_svc = self.r[14];
            }
            CpuMode::Abort => {
                self.r13_abt = self.r[13];
                self.r14_abt = self.r[14];
            }
            CpuMode::IRQ => {
                self.r13_irq = self.r[13];
                self.r14_irq = self.r[14];
            }
            CpuMode::Undefined => {
                self.r13_und = self.r[13];
                self.r14_und = self.r[14];
            }
        }

        // Update CPSR mode bits
        self.cpsr = (self.cpsr & !0x1F) | new_mode.bits();

        // Restore registers from new mode's bank
        match new_mode {
            CpuMode::User | CpuMode::System => {
                self.r[13] = self.r13_sys;
                self.r[14] = self.r14_sys;
            }
            CpuMode::FIQ => {
                self.r[8] = self.r8_fiq;
                self.r[9] = self.r9_fiq;
                self.r[10] = self.r10_fiq;
                self.r[11] = self.r11_fiq;
                self.r[12] = self.r12_fiq;
                self.r[13] = self.r13_fiq;
                self.r[14] = self.r14_fiq;
            }
            CpuMode::Supervisor => {
                self.r[13] = self.r13_svc;
                self.r[14] = self.r14_svc;
            }
            CpuMode::Abort => {
                self.r[13] = self.r13_abt;
                self.r[14] = self.r14_abt;
            }
            CpuMode::IRQ => {
                self.r[13] = self.r13_irq;
                self.r[14] = self.r14_irq;
            }
            CpuMode::Undefined => {
                self.r[13] = self.r13_und;
                self.r[14] = self.r14_und;
            }
        }
    }

    /// Read Saved Program Status Register (SPSR) for current mode.
    pub fn spsr(&self) -> u32 {
        match self.mode() {
            CpuMode::FIQ => self.spsr_fiq,
            CpuMode::Supervisor => self.spsr_svc,
            CpuMode::Abort => self.spsr_abt,
            CpuMode::IRQ => self.spsr_irq,
            CpuMode::Undefined => self.spsr_und,
            _ => self.cpsr,
        }
    }

    /// Write Saved Program Status Register (SPSR) for current mode.
    pub fn set_spsr(&mut self, val: u32) {
        match self.mode() {
            CpuMode::FIQ => self.spsr_fiq = val,
            CpuMode::Supervisor => self.spsr_svc = val,
            CpuMode::Abort => self.spsr_abt = val,
            CpuMode::IRQ => self.spsr_irq = val,
            CpuMode::Undefined => self.spsr_und = val,
            _ => {}
        }
    }
}

/// ARM7TDMI Processor Core
pub struct Cpu {
    pub regs: Registers,
    pub cycle_count: usize,
    pub last_pc: u32,
    pub pc_repeat_count: usize,
    pub halted: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            cycle_count: 0,
            last_pc: 0,
            pc_repeat_count: 0,
            halted: false,
        }
    }

    /// Perform Branch and Exchange (BX) to target address, dynamically switching ARM/THUMB mode.
    pub fn branch_and_exchange(&mut self, target: u32) {
        let thumb = (target & 1) != 0;
        self.regs.set_thumb_mode(thumb);
        if thumb {
            self.regs.r[15] = target & !1;
        } else {
            self.regs.r[15] = target & !3;
        }
    }

    /// Raise CPU Hardware SWI Exception (vector 0x00000008).
    pub fn trigger_swi(&mut self) {
        let old_cpsr = self.regs.cpsr;
        let is_thumb = self.regs.thumb_mode();
        let return_pc = if is_thumb {
            self.regs.r[15].wrapping_sub(2) // LR_svc = next instruction after THUMB SWI
        } else {
            self.regs.r[15].wrapping_sub(4) // LR_svc = next instruction after ARM SWI
        };

        self.regs.set_mode(CpuMode::Supervisor);
        self.regs.set_spsr(old_cpsr);
        self.regs.set_irq_disabled(true);
        self.regs.set_thumb_mode(false); // ARM mode
        self.regs.r[14] = return_pc; // LR_svc
        self.regs.r[15] = 0x00000008; // ARM SWI Exception Vector
    }

    /// Raise CPU Hardware IRQ Exception (vector 0x00000018).
    pub fn trigger_irq(&mut self) {
        if self.regs.irq_disabled() {
            return;
        }

        let is_thumb = self.regs.thumb_mode();
        let old_cpsr = self.regs.cpsr;
        let current_pc = if is_thumb {
            self.regs.r[15].wrapping_sub(4)
        } else {
            self.regs.r[15].wrapping_sub(8)
        };
        let return_pc = current_pc.wrapping_add(4);

        self.regs.set_mode(CpuMode::IRQ);
        self.regs.set_spsr(old_cpsr);
        self.regs.set_irq_disabled(true);
        self.regs.set_thumb_mode(false); // ARM mode
        self.regs.r[14] = return_pc; // LR_irq
        self.regs.r[15] = 0x00000018; // ARM IRQ Exception Vector
    }

    /// Execute single CPU instruction step (ARM 32-bit or THUMB 16-bit).
    pub fn step(&mut self, bus: &mut GbaMemoryBus) -> usize {
        if self.halted {
            let ie = bus.read_u16(0x04000200);
            let if_flags = bus.read_u16(0x04000202);
            if (ie & if_flags) != 0 {
                self.halted = false;
            } else {
                self.cycle_count += 2;
                return 2;
            }
        }

        let pc = if self.regs.thumb_mode() {
            self.regs.r[15] & !1
        } else {
            self.regs.r[15] & !3
        };

        if self.regs.thumb_mode() {
            // THUMB Mode: 16-bit instruction, 2-byte aligned (R15 reads as pc + 4)
            let instr = bus.read_u16(pc);
            let expected_pc = pc.wrapping_add(4);
            self.regs.r[15] = expected_pc;
            let cycles = super::thumb::execute_thumb(&mut self.regs, bus, instr);
            if self.regs.r[15] == expected_pc {
                self.regs.r[15] = pc.wrapping_add(2);
            }
            self.cycle_count += cycles;
            cycles
        } else {
            // ARM Mode: 32-bit instruction, 4-byte aligned (R15 reads as pc + 8)
            let instr = bus.read_u32(pc);
            let expected_pc = pc.wrapping_add(8);
            self.regs.r[15] = expected_pc;
            let cycles = super::arm::execute_arm(&mut self.regs, bus, instr);
            if self.regs.r[15] == expected_pc {
                self.regs.r[15] = pc.wrapping_add(4);
            }
            self.cycle_count += cycles;
            cycles
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gba::mmu::GbaMemoryBus;

    #[test]
    fn test_cpsr_flags() {
        let mut regs = Registers::new();
        regs.set_n_flag(true);
        regs.set_z_flag(true);
        regs.set_c_flag(false);
        regs.set_v_flag(true);

        assert!(regs.n_flag());
        assert!(regs.z_flag());
        assert!(!regs.c_flag());
        assert!(regs.v_flag());
    }

    #[test]
    fn test_mode_switching_and_banked_registers() {
        let mut regs = Registers::new();
        regs.set_mode(CpuMode::System);
        regs.set_sp(0x03007F00); // System SP
        regs.set_lr(0x08000100); // System LR

        // Switch to IRQ mode
        regs.set_mode(CpuMode::IRQ);
        assert_eq!(regs.mode(), CpuMode::IRQ);
        assert_eq!(regs.sp(), 0x03007FA0); // IRQ SP default
        regs.set_sp(0x03007F80);
        regs.set_lr(0x08000200);

        // Switch back to System mode
        regs.set_mode(CpuMode::System);
        assert_eq!(regs.mode(), CpuMode::System);
        assert_eq!(regs.sp(), 0x03007F00);
        assert_eq!(regs.lr(), 0x08000100);

        // Switch back to IRQ mode to ensure banked IRQ values persisted
        regs.set_mode(CpuMode::IRQ);
        assert_eq!(regs.sp(), 0x03007F80);
        assert_eq!(regs.lr(), 0x08000200);
    }

    #[test]
    fn test_branch_and_exchange_arm_to_thumb() {
        let mut cpu = Cpu::new();

        assert!(!cpu.regs.thumb_mode()); // Initially ARM mode

        // BX to odd address (0x08000005) -> THUMB mode at 0x08000004
        cpu.branch_and_exchange(0x08000005);
        assert!(cpu.regs.thumb_mode());
        assert_eq!(cpu.regs.pc(), 0x08000004);

        // BX to even address (0x08000010) -> ARM mode at 0x08000010
        cpu.branch_and_exchange(0x08000010);
        assert!(!cpu.regs.thumb_mode());
        assert_eq!(cpu.regs.pc(), 0x08000010);
    }

    #[test]
    fn test_cpu_step_arm_and_thumb_fetch() {
        let mut cpu = Cpu::new();
        let mut bus = GbaMemoryBus::new();

        cpu.regs.set_pc(0x08000000);
        // Target THUMB address: 0x08000101
        cpu.regs.r[0] = 0x08000101;
        // Load ARM opcode for BX r0 (0xE12FFF10) into ROM at 0x08000000
        let mut rom = vec![0u8; 0x1000];
        rom[0..4].copy_from_slice(&0xE12FFF10u32.to_le_bytes());
        bus.load_rom(&rom);

        cpu.step(&mut bus);

        // CPU should now be in THUMB mode at PC 0x08000100
        assert!(cpu.regs.thumb_mode());
        assert_eq!(cpu.regs.pc(), 0x08000100);
    }
}

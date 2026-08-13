#![allow(dead_code)]

use super::cpu::{CpuMode, Registers};
use super::mmu::GbaMemoryBus;

/// Check ARM Condition Field [31:28] against CPSR condition flags
pub fn check_condition(cond: u32, regs: &Registers) -> bool {
    match cond {
        0x0 => regs.z_flag(),                        // EQ (Equal)
        0x1 => !regs.z_flag(),                       // NE (Not Equal)
        0x2 => regs.c_flag(),                        // CS / HS (Carry Set / Unsigned Higher or Same)
        0x3 => !regs.c_flag(),                       // CC / LO (Carry Clear / Unsigned Lower)
        0x4 => regs.n_flag(),                        // MI (Minus / Negative)
        0x5 => !regs.n_flag(),                       // PL (Plus / Positive)
        0x6 => regs.v_flag(),                        // VS (Overflow Set)
        0x7 => !regs.v_flag(),                       // VC (Overflow Clear)
        0x8 => regs.c_flag() && !regs.z_flag(),      // HI (Unsigned Higher)
        0x9 => !regs.c_flag() || regs.z_flag(),      // LS (Unsigned Lower or Same)
        0xA => regs.n_flag() == regs.v_flag(),       // GE (Signed Greater or Equal)
        0xB => regs.n_flag() != regs.v_flag(),       // LT (Signed Less Than)
        0xC => !regs.z_flag() && (regs.n_flag() == regs.v_flag()), // GT (Signed Greater Than)
        0xD => regs.z_flag() || (regs.n_flag() != regs.v_flag()),  // LE (Signed Less Than or Equal)
        0xE => true,                                 // AL (Always)
        0xF => false,                                // NV (Never / Reserved)
        _ => true,
    }
}

/// Shift types for ARM Barrel Shifter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftType {
    LSL = 0,
    LSR = 1,
    ASR = 2,
    ROR = 3,
}

/// ARM Barrel Shifter calculation
pub fn shift_operand(
    shift_type: ShiftType,
    amount: u32,
    val: u32,
    carry_in: bool,
    is_reg_shift: bool,
) -> (u32, bool) {
    match shift_type {
        ShiftType::LSL => {
            if amount == 0 {
                (val, carry_in)
            } else if amount < 32 {
                let carry = (val & (1 << (32 - amount))) != 0;
                (val << amount, carry)
            } else if amount == 32 {
                let carry = (val & 1) != 0;
                (0, carry)
            } else {
                (0, false)
            }
        }
        ShiftType::LSR => {
            if amount == 0 {
                if is_reg_shift {
                    (val, carry_in)
                } else {
                    // LSR #0 is interpreted as LSR #32
                    let carry = (val & (1 << 31)) != 0;
                    (0, carry)
                }
            } else if amount < 32 {
                let carry = (val & (1 << (amount - 1))) != 0;
                (val >> amount, carry)
            } else if amount == 32 {
                let carry = (val & (1 << 31)) != 0;
                (0, carry)
            } else {
                (0, false)
            }
        }
        ShiftType::ASR => {
            if amount == 0 {
                if is_reg_shift {
                    (val, carry_in)
                } else {
                    // ASR #0 is interpreted as ASR #32
                    let carry = (val & (1 << 31)) != 0;
                    let res = if (val as i32) < 0 { 0xFFFFFFFF } else { 0 };
                    (res, carry)
                }
            } else if amount < 32 {
                let carry = (val & (1 << (amount - 1))) != 0;
                let res = ((val as i32) >> amount) as u32;
                (res, carry)
            } else {
                let carry = (val & (1 << 31)) != 0;
                let res = if (val as i32) < 0 { 0xFFFFFFFF } else { 0 };
                (res, carry)
            }
        }
        ShiftType::ROR => {
            if amount == 0 {
                if is_reg_shift {
                    (val, carry_in)
                } else {
                    // RRX (Rotate Right Extended by 1 through Carry)
                    let carry = (val & 1) != 0;
                    let c_bit = if carry_in { 1 << 31 } else { 0 };
                    ((val >> 1) | c_bit, carry)
                }
            } else {
                let shift_amt = amount % 32;
                if shift_amt == 0 {
                    let carry = (val & (1 << 31)) != 0;
                    (val, carry)
                } else {
                    let carry = (val & (1 << (shift_amt - 1))) != 0;
                    (val.rotate_right(shift_amt), carry)
                }
            }
        }
    }
}

/// Execute 32-bit ARM instruction
pub fn execute_arm(regs: &mut Registers, bus: &mut GbaMemoryBus, instr: u32) -> usize {
    let cond = (instr >> 28) & 0x0F;
    if !check_condition(cond, regs) {
        return 1;
    }

    // 1. Branch Exchange (BX / BLX reg): cond 0001 0001 0010 1111 1111 1111 001x Rm
    if (instr & 0x0FFFFF90) == 0x012FFF10 {
        let is_blx = (instr & 0x20) != 0;
        let rm = (instr & 0x0F) as usize;
        let target = regs.r[rm];
        if is_blx {
            regs.r[14] = regs.r[15].wrapping_sub(4);
        }
        regs.set_pc_interworking(target);
        return 3;
    }

    // 1b. Branch with Link and Exchange Immediate (BLX label): 1111 101H offset24
    if (instr & 0xFE000000) == 0xFA000000 {
        let h = ((instr >> 24) & 1) << 1;
        let offset24 = instr & 0x00FFFFFF;
        let sign_extended = if (offset24 & 0x00800000) != 0 {
            (offset24 | 0xFF000000) as i32
        } else {
            offset24 as i32
        };
        let branch_offset = ((sign_extended << 2) as u32) | h;
        regs.r[14] = regs.r[15].wrapping_sub(4);
        let target = regs.r[15].wrapping_add(branch_offset) | 1; // Bit 0 set for THUMB
        regs.set_pc_interworking(target);
        return 3;
    }

    // 2. Status Register Access (MRS / MSR)
    if (instr & 0x0FBF0FFF) == 0x010F0000 {
        // MRS Rd, CPSR/SPSR
        let rd = ((instr >> 12) & 0x0F) as usize;
        let use_spsr = (instr & (1 << 22)) != 0;
        regs.r[rd] = if use_spsr { regs.spsr() } else { regs.cpsr };
        return 1;
    }

    if (instr & 0x0DBF0000) == 0x01200000 {
        // MSR CPSR/SPSR_<fields>, Rm/imm
        let use_spsr = (instr & (1 << 22)) != 0;
        let is_imm = (instr & (1 << 25)) != 0;
        let mask_bits = (instr >> 16) & 0x0F;

        let mut mask: u32 = 0;
        if (mask_bits & 1) != 0 { mask |= 0x000000FF; } // c (control)
        if (mask_bits & 2) != 0 { mask |= 0x0000FF00; } // x (extension)
        if (mask_bits & 4) != 0 { mask |= 0x00FF0000; } // s (status)
        if (mask_bits & 8) != 0 { mask |= 0xFF000000; } // f (flags)

        let val = if is_imm {
            let imm = instr & 0xFF;
            let rotate = ((instr >> 8) & 0x0F) * 2;
            imm.rotate_right(rotate)
        } else {
            let rm = (instr & 0x0F) as usize;
            regs.r[rm]
        };

        if use_spsr {
            let cur_spsr = regs.spsr();
            let new_spsr = (cur_spsr & !mask) | (val & mask);
            regs.set_spsr(new_spsr);
        } else {
            let old_mode = regs.mode();
            let cur_cpsr = regs.cpsr;
            let new_cpsr = (cur_cpsr & !mask) | (val & mask);
            let new_mode = CpuMode::from_bits(new_cpsr & 0x1F);

            if old_mode != new_mode {
                regs.set_mode(new_mode);
            }
            regs.cpsr = (regs.cpsr & !mask) | (val & mask);
        }
        return 1;
    }

    // 3. Multiply / Multiply Accumulate (MUL, MLA)
    if (instr & 0x0FC000F0) == 0x00000090 || (instr & 0x0FC000F0) == 0x00200090 {
        let is_mla = (instr & (1 << 21)) != 0;
        let set_flags = (instr & (1 << 20)) != 0;
        let rd = ((instr >> 16) & 0x0F) as usize;
        let rn = ((instr >> 12) & 0x0F) as usize; // accumulator for MLA
        let rs = ((instr >> 8) & 0x0F) as usize;
        let rm = (instr & 0x0F) as usize;

        let mut res = regs.r[rm].wrapping_mul(regs.r[rs]);
        if is_mla {
            res = res.wrapping_add(regs.r[rn]);
        }
        regs.r[rd] = res;

        if set_flags {
            regs.set_n_flag((res & (1 << 31)) != 0);
            regs.set_z_flag(res == 0);
        }
        return 3;
    }

    // 4. Halfword / Signed Data Transfer (LDRH, STRH, LDRSB, LDRSH)
    if (instr & 0x0E000090) == 0x00000090 {
        let p = (instr & (1 << 24)) != 0;
        let u = (instr & (1 << 23)) != 0;
        let i = (instr & (1 << 22)) != 0; // 1 = immediate, 0 = register
        let w = (instr & (1 << 21)) != 0;
        let l = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;
        let s = (instr & (1 << 6)) != 0;
        let h = (instr & (1 << 5)) != 0;

        let offset = if i {
            ((instr >> 4) & 0xF0) | (instr & 0x0F)
        } else {
            let rm = (instr & 0x0F) as usize;
            regs.r[rm]
        };

        let base = regs.r[rn];
        let addr = if p {
            if u { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
        } else {
            base
        };

        if l {
            let val = match (s, h) {
                (false, true) => bus.read_u16(addr) as u32,                        // LDRH
                (true, false) => (bus.read_u8(addr) as i8) as i32 as u32,           // LDRSB
                (true, true) => (bus.read_u16(addr) as i16) as i32 as u32,          // LDRSH
                _ => 0,
            };
            regs.r[rd] = val;
        } else {
            if !s && h {
                bus.write_u16(addr, regs.r[rd] as u16);                            // STRH
            }
        }

        if !p {
            let writeback_addr = if u { base.wrapping_add(offset) } else { base.wrapping_sub(offset) };
            regs.r[rn] = writeback_addr;
        } else if w {
            regs.r[rn] = addr;
        }
        return 3;
    }

    // 5. Single Data Transfer (LDR, STR, LDRB, STRB)
    if (instr & 0x0C000000) == 0x04000000 {
        let is_reg_offset = (instr & (1 << 25)) != 0;
        let p = (instr & (1 << 24)) != 0;
        let u = (instr & (1 << 23)) != 0;
        let b = (instr & (1 << 22)) != 0;
        let w = (instr & (1 << 21)) != 0;
        let l = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;

        let offset = if is_reg_offset {
            let rm = (instr & 0x0F) as usize;
            let shift_type = match (instr >> 5) & 3 {
                0 => ShiftType::LSL,
                1 => ShiftType::LSR,
                2 => ShiftType::ASR,
                _ => ShiftType::ROR,
            };
            let shift_amt = (instr >> 7) & 0x1F;
            let (shifted, _) = shift_operand(shift_type, shift_amt, regs.r[rm], regs.c_flag(), false);
            shifted
        } else {
            instr & 0xFFF
        };

        let base = regs.r[rn];
        let addr = if p {
            if u { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
        } else {
            base
        };

        if l {
            let val = if b {
                bus.read_u8(addr) as u32
            } else {
                bus.read_u32(addr)
            };
            regs.r[rd] = val;
        } else {
            if b {
                bus.write_u8(addr, regs.r[rd] as u8);
            } else {
                bus.write_u32(addr, regs.r[rd]);
            }
        }

        if !p {
            let writeback_addr = if u { base.wrapping_add(offset) } else { base.wrapping_sub(offset) };
            regs.r[rn] = writeback_addr;
        } else if w {
            regs.r[rn] = addr;
        }
        return 3;
    }

    // 6. Block Data Transfer (LDM, STM)
    if (instr & 0x0E000000) == 0x08000000 {
        let p = (instr & (1 << 24)) != 0;
        let u = (instr & (1 << 23)) != 0;
        let w = (instr & (1 << 21)) != 0;
        let l = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let reg_list = instr & 0xFFFF;

        let reg_count = reg_list.count_ones();
        let curr_addr = regs.r[rn];

        let start_addr = if u {
            if p { curr_addr.wrapping_add(4) } else { curr_addr }
        } else {
            let total_bytes = reg_count * 4;
            if p { curr_addr.wrapping_sub(total_bytes) } else { curr_addr.wrapping_sub(total_bytes).wrapping_add(4) }
        };

        let mut addr_iter = start_addr;
        for reg in 0..16 {
            if (reg_list & (1 << reg)) != 0 {
                if l {
                    regs.r[reg] = bus.read_u32(addr_iter);
                } else {
                    bus.write_u32(addr_iter, regs.r[reg]);
                }
                addr_iter = addr_iter.wrapping_add(4);
            }
        }

        if w {
            let final_addr = if u {
                curr_addr.wrapping_add(reg_count * 4)
            } else {
                curr_addr.wrapping_sub(reg_count * 4)
            };
            regs.r[rn] = final_addr;
        }
        return 3 + reg_count as usize;
    }

    // 7. Branch & Branch with Link (B, BL)
    if (instr & 0x0E000000) == 0x0A000000 {
        let is_link = (instr & 0x01000000) != 0;
        let offset24 = instr & 0x00FFFFFF;
        let sign_extended = if (offset24 & 0x00800000) != 0 {
            (offset24 | 0xFF000000) as i32
        } else {
            offset24 as i32
        };
        let branch_offset = (sign_extended << 2) as u32;

        if is_link {
            regs.r[14] = regs.r[15].wrapping_sub(4);
        }

        regs.r[15] = regs.r[15].wrapping_add(branch_offset);
        return 3;
    }

    // 8. Software Interrupt (SWI): cond 1111 xxxx xxxx xxxx xxxx xxxx xxxx
    if (instr & 0x0F000000) == 0x0F000000 {
        let swi_num = ((instr >> 16) & 0xFF) as u8;
        super::bios::handle_swi(swi_num, regs, bus);
        return 3;
    }

    // 9. Data Processing Operations
    if (instr & 0x0C000000) == 0x00000000 {
        let is_imm = (instr & (1 << 25)) != 0;
        let opcode = (instr >> 21) & 0x0F;
        let set_flags = (instr & (1 << 20)) != 0;
        let rn = ((instr >> 16) & 0x0F) as usize;
        let rd = ((instr >> 12) & 0x0F) as usize;

        let (op2, shifter_carry) = if is_imm {
            let imm = instr & 0xFF;
            let rotate = ((instr >> 8) & 0x0F) * 2;
            let val = imm.rotate_right(rotate);
            let carry = if rotate == 0 { regs.c_flag() } else { (val & (1 << 31)) != 0 };
            (val, carry)
        } else {
            let rm = (instr & 0x0F) as usize;
            let shift_type = match (instr >> 5) & 3 {
                0 => ShiftType::LSL,
                1 => ShiftType::LSR,
                2 => ShiftType::ASR,
                _ => ShiftType::ROR,
            };
            let is_reg_shift = (instr & (1 << 4)) != 0;
            let shift_amt = if is_reg_shift {
                let rs = ((instr >> 8) & 0x0F) as usize;
                regs.r[rs] & 0xFF
            } else {
                (instr >> 7) & 0x1F
            };
            shift_operand(shift_type, shift_amt, regs.r[rm], regs.c_flag(), is_reg_shift)
        };

        let op1 = regs.r[rn];
        let carry_in = regs.c_flag() as u32;

        let mut write_rd = true;
        let (res, carry_out, overflow_out) = match opcode {
            0x0 => (op1 & op2, shifter_carry, regs.v_flag()),                     // AND
            0x1 => (op1 ^ op2, shifter_carry, regs.v_flag()),                     // EOR
            0x2 => {                                                              // SUB
                let diff = (op1 as u64).wrapping_sub(op2 as u64);
                let res = diff as u32;
                let c = op1 >= op2;
                let v = ((op1 ^ op2) & (op1 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0x3 => {                                                              // RSB
                let diff = (op2 as u64).wrapping_sub(op1 as u64);
                let res = diff as u32;
                let c = op2 >= op1;
                let v = ((op2 ^ op1) & (op2 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0x4 => {                                                              // ADD
                let sum = (op1 as u64) + (op2 as u64);
                let res = sum as u32;
                let c = sum > 0xFFFFFFFF;
                let v = (!((op1 ^ op2)) & (op1 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0x5 => {                                                              // ADC
                let sum = (op1 as u64) + (op2 as u64) + (carry_in as u64);
                let res = sum as u32;
                let c = sum > 0xFFFFFFFF;
                let v = (!((op1 ^ op2)) & (op1 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0x6 => {                                                              // SBC
                let borrow = 1 - carry_in as u64;
                let diff = (op1 as u64).wrapping_sub(op2 as u64).wrapping_sub(borrow);
                let res = diff as u32;
                let c = (op1 as u64) >= ((op2 as u64) + borrow);
                let v = ((op1 ^ op2) & (op1 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0x7 => {                                                              // RSC
                let borrow = 1 - carry_in as u64;
                let diff = (op2 as u64).wrapping_sub(op1 as u64).wrapping_sub(borrow);
                let res = diff as u32;
                let c = (op2 as u64) >= ((op1 as u64) + borrow);
                let v = ((op2 ^ op1) & (op2 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0x8 => { write_rd = false; (op1 & op2, shifter_carry, regs.v_flag()) } // TST
            0x9 => { write_rd = false; (op1 ^ op2, shifter_carry, regs.v_flag()) } // TEQ
            0xA => {                                                              // CMP
                write_rd = false;
                let diff = (op1 as u64).wrapping_sub(op2 as u64);
                let res = diff as u32;
                let c = op1 >= op2;
                let v = ((op1 ^ op2) & (op1 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0xB => {                                                              // CMN
                write_rd = false;
                let sum = (op1 as u64) + (op2 as u64);
                let res = sum as u32;
                let c = sum > 0xFFFFFFFF;
                let v = (!((op1 ^ op2)) & (op1 ^ res) & 0x80000000) != 0;
                (res, c, v)
            }
            0xC => (op1 | op2, shifter_carry, regs.v_flag()),                     // ORR
            0xD => (op2, shifter_carry, regs.v_flag()),                           // MOV
            0xE => (op1 & !op2, shifter_carry, regs.v_flag()),                    // BIC
            0xF => (!op2, shifter_carry, regs.v_flag()),                          // MVN
            _ => (0, false, false),
        };

        if write_rd {
            if rd == 15 {
                if set_flags {
                    let old_mode = regs.mode();
                    let cur_spsr = regs.spsr();
                    regs.cpsr = cur_spsr;
                    let new_mode = CpuMode::from_bits(cur_spsr & 0x1F);
                    if old_mode != new_mode {
                        regs.set_mode(new_mode);
                    }
                    if regs.thumb_mode() {
                        regs.r[15] = res & !1;
                    } else {
                        regs.r[15] = res & !3;
                    }
                } else {
                    regs.r[15] = res & !3;
                }
            } else {
                regs.r[rd] = res;
            }
        }

        if set_flags && rd != 15 {
            regs.set_n_flag((res & (1 << 31)) != 0);
            regs.set_z_flag(res == 0);
            regs.set_c_flag(carry_out);
            regs.set_v_flag(overflow_out);
        }
        return 1;
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm_add_and_cmp() {
        let mut regs = Registers::new();
        let mut bus = GbaMemoryBus::new();

        // ADD r0, r1, #5  (r1 = 10 -> r0 = 15)
        regs.r[1] = 10;
        let add_instr = 0xE2810005; // ADD r0, r1, #5
        execute_arm(&mut regs, &mut bus, add_instr);
        assert_eq!(regs.r[0], 15);

        // CMP r0, #15
        let cmp_instr = 0xE350000F; // CMP r0, #15
        execute_arm(&mut regs, &mut bus, cmp_instr);
        assert!(regs.z_flag());
        assert!(regs.c_flag());
        assert!(!regs.n_flag());
    }
}

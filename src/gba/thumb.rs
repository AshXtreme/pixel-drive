#![allow(dead_code)]

use super::arm::{check_condition, shift_operand, ShiftType};
use super::cpu::Registers;
use super::mmu::GbaMemoryBus;

/// Execute 16-bit THUMB instruction
pub fn execute_thumb(regs: &mut Registers, bus: &mut GbaMemoryBus, instr: u16) -> usize {
    let top_bits = instr >> 12;

    match top_bits {
        // Formats 1-3: Move Shifted Register & Add/Subtract
        0x0 | 0x1 => {
            if (instr & 0xF800) == 0x1800 {
                // Format 2: ADD/SUB register/3-bit immediate
                let is_sub = (instr & (1 << 9)) != 0;
                let is_imm = (instr & (1 << 10)) != 0;
                let rn_imm = ((instr >> 6) & 7) as u32;
                let rs = ((instr >> 3) & 7) as usize;
                let rd = (instr & 7) as usize;

                let val2 = if is_imm { rn_imm } else { regs.r[rn_imm as usize] };
                let val1 = regs.r[rs];

                let (res, c, v) = if is_sub {
                    let diff = (val1 as u64).wrapping_sub(val2 as u64);
                    let res = diff as u32;
                    let c = val1 >= val2;
                    let v = ((val1 ^ val2) & (val1 ^ res) & 0x80000000) != 0;
                    (res, c, v)
                } else {
                    let sum = (val1 as u64) + (val2 as u64);
                    let res = sum as u32;
                    let c = sum > 0xFFFFFFFF;
                    let v = (!((val1 ^ val2)) & (val1 ^ res) & 0x80000000) != 0;
                    (res, c, v)
                };

                regs.r[rd] = res;
                regs.set_n_flag((res & (1 << 31)) != 0);
                regs.set_z_flag(res == 0);
                regs.set_c_flag(c);
                regs.set_v_flag(v);
                1
            } else {
                // Format 1: Shift by Immediate (LSL, LSR, ASR)
                let op = (instr >> 11) & 3;
                let offset5 = ((instr >> 6) & 0x1F) as u32;
                let rs = ((instr >> 3) & 7) as usize;
                let rd = (instr & 7) as usize;

                let shift_type = match op {
                    0 => ShiftType::LSL,
                    1 => ShiftType::LSR,
                    _ => ShiftType::ASR,
                };

                let (res, carry) = shift_operand(shift_type, offset5, regs.r[rs], regs.c_flag(), false);
                regs.r[rd] = res;
                regs.set_n_flag((res & (1 << 31)) != 0);
                regs.set_z_flag(res == 0);
                regs.set_c_flag(carry);
                1
            }
        }

        // Format 3: Move/Compare/Add/Subtract Immediate
        0x2 | 0x3 => {
            let op = (instr >> 11) & 3;
            let rd = ((instr >> 8) & 7) as usize;
            let imm8 = (instr & 0xFF) as u32;
            let val1 = regs.r[rd];

            match op {
                0 => {
                    // MOV Rd, #imm8
                    regs.r[rd] = imm8;
                    regs.set_n_flag((imm8 & (1 << 31)) != 0);
                    regs.set_z_flag(imm8 == 0);
                }
                1 => {
                    // CMP Rd, #imm8
                    let diff = (val1 as u64).wrapping_sub(imm8 as u64);
                    let res = diff as u32;
                    regs.set_n_flag((res & (1 << 31)) != 0);
                    regs.set_z_flag(res == 0);
                    regs.set_c_flag(val1 >= imm8);
                    regs.set_v_flag(((val1 ^ imm8) & (val1 ^ res) & 0x80000000) != 0);
                }
                2 => {
                    // ADD Rd, #imm8
                    let sum = (val1 as u64) + (imm8 as u64);
                    let res = sum as u32;
                    regs.r[rd] = res;
                    regs.set_n_flag((res & (1 << 31)) != 0);
                    regs.set_z_flag(res == 0);
                    regs.set_c_flag(sum > 0xFFFFFFFF);
                    regs.set_v_flag(!((val1 ^ imm8)) & (val1 ^ res) & 0x80000000 != 0);
                }
                3 => {
                    // SUB Rd, #imm8
                    let diff = (val1 as u64).wrapping_sub(imm8 as u64);
                    let res = diff as u32;
                    regs.r[rd] = res;
                    regs.set_n_flag((res & (1 << 31)) != 0);
                    regs.set_z_flag(res == 0);
                    regs.set_c_flag(val1 >= imm8);
                    regs.set_v_flag(((val1 ^ imm8) & (val1 ^ res) & 0x80000000) != 0);
                }
                _ => {}
            }
            1
        }

        // Formats 4 & 5: ALU Operations & Hi Register Operations / BX
        0x4 => {
            if (instr & 0xFC00) == 0x4000 {
                // Format 4: ALU Operations
                let op = (instr >> 6) & 0x0F;
                let rs = ((instr >> 3) & 7) as usize;
                let rd = (instr & 7) as usize;

                let val1 = regs.r[rd];
                let val2 = regs.r[rs];

                match op {
                    0x0 => { // AND
                        let res = val1 & val2;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    0x1 => { // EOR
                        let res = val1 ^ val2;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    0x2 => { // LSL
                        let (res, c) = shift_operand(ShiftType::LSL, val2 & 0xFF, val1, regs.c_flag(), true);
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(c);
                    }
                    0x3 => { // LSR
                        let (res, c) = shift_operand(ShiftType::LSR, val2 & 0xFF, val1, regs.c_flag(), true);
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(c);
                    }
                    0x4 => { // ASR
                        let (res, c) = shift_operand(ShiftType::ASR, val2 & 0xFF, val1, regs.c_flag(), true);
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(c);
                    }
                    0x5 => { // ADC
                        let c_in = regs.c_flag() as u64;
                        let sum = (val1 as u64) + (val2 as u64) + c_in;
                        let res = sum as u32;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(sum > 0xFFFFFFFF);
                        regs.set_v_flag(!((val1 ^ val2)) & (val1 ^ res) & 0x80000000 != 0);
                    }
                    0x6 => { // SBC
                        let c_in = regs.c_flag() as u64;
                        let borrow = 1 - c_in;
                        let diff = (val1 as u64).wrapping_sub(val2 as u64).wrapping_sub(borrow);
                        let res = diff as u32;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag((val1 as u64) >= ((val2 as u64) + borrow));
                        regs.set_v_flag(((val1 ^ val2) & (val1 ^ res) & 0x80000000) != 0);
                    }
                    0x7 => { // ROR
                        let (res, c) = shift_operand(ShiftType::ROR, val2 & 0xFF, val1, regs.c_flag(), true);
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(c);
                    }
                    0x8 => { // TST
                        let res = val1 & val2;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    0x9 => { // NEG
                        let diff = 0u64.wrapping_sub(val2 as u64);
                        let res = diff as u32;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(0 >= val2);
                        regs.set_v_flag((val2 & res & 0x80000000) != 0);
                    }
                    0xA => { // CMP
                        let diff = (val1 as u64).wrapping_sub(val2 as u64);
                        let res = diff as u32;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(val1 >= val2);
                        regs.set_v_flag(((val1 ^ val2) & (val1 ^ res) & 0x80000000) != 0);
                    }
                    0xB => { // CMN
                        let sum = (val1 as u64) + (val2 as u64);
                        let res = sum as u32;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(sum > 0xFFFFFFFF);
                        regs.set_v_flag(!((val1 ^ val2)) & (val1 ^ res) & 0x80000000 != 0);
                    }
                    0xC => { // ORR
                        let res = val1 | val2;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    0xD => { // MUL
                        let res = val1.wrapping_mul(val2);
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    0xE => { // BIC
                        let res = val1 & !val2;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    0xF => { // MVN
                        let res = !val2;
                        regs.r[rd] = res;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                    }
                    _ => {}
                }
                1
            } else if (instr & 0xFC00) == 0x4400 {
                // Format 5: Hi Register Operations / BX
                let op = (instr >> 8) & 3;
                let h1 = (instr & (1 << 7)) != 0;
                let h2 = (instr & (1 << 6)) != 0;
                let rs = (((h2 as usize) << 3) | (((instr >> 3) & 7) as usize)) as usize;
                let rd = (((h1 as usize) << 3) | ((instr & 7) as usize)) as usize;

                let val2 = regs.r[rs];

                match op {
                    0 => { // ADD
                        let res = regs.r[rd].wrapping_add(val2);
                        if rd == 15 {
                            regs.r[15] = regs.r[15].wrapping_add(val2) & !1;
                            return 3;
                        } else {
                            regs.r[rd] = res;
                        }
                    }
                    1 => { // CMP
                        let val1 = regs.r[rd];
                        let diff = (val1 as u64).wrapping_sub(val2 as u64);
                        let res = diff as u32;
                        regs.set_n_flag((res & (1 << 31)) != 0);
                        regs.set_z_flag(res == 0);
                        regs.set_c_flag(val1 >= val2);
                        regs.set_v_flag(((val1 ^ val2) & (val1 ^ res) & 0x80000000) != 0);
                    }
                    2 => { // MOV
                        if rd == 15 {
                            regs.r[15] = val2 & !1;
                            return 3;
                        } else {
                            regs.r[rd] = val2;
                        }
                    }
                    3 => { // BX / BLX
                        if h1 {
                            regs.r[14] = (regs.r[15].wrapping_sub(2)) | 1;
                        }
                        let target = val2;
                        regs.set_pc_interworking(target);
                        return 3;
                    }
                    _ => {}
                }
                1
            } else {
                // Format 6: PC-Relative Load (LDR Rd, [PC, #imm8])
                let rd = ((instr >> 8) & 7) as usize;
                let word8 = ((instr & 0xFF) as u32) * 4;
                let aligned_pc = (regs.r[15].wrapping_sub(2).wrapping_add(2)) & !2; // Force 4-byte alignment
                let addr = aligned_pc.wrapping_add(word8);
                let val = bus.read_u32(addr);
                regs.r[rd] = val;
                2
            }
        }

        // Format 7 & 8: Load/Store with Register Offset & Sign-extended Byte/Halfword
        0x5 => {
            let is_load = (instr & (1 << 11)) != 0;
            let is_byte_or_sign = (instr & (1 << 10)) != 0;
            let ro = ((instr >> 6) & 7) as usize;
            let rb = ((instr >> 3) & 7) as usize;
            let rd = (instr & 7) as usize;

            let addr = regs.r[rb].wrapping_add(regs.r[ro]);

            if (instr & (1 << 9)) == 0 {
                // Format 7: Register offset LDR/STR/LDRB/STRB
                if is_load {
                    if is_byte_or_sign {
                        regs.r[rd] = bus.read_u8(addr) as u32;
                    } else {
                        regs.r[rd] = bus.read_u32(addr);
                    }
                } else {
                    if is_byte_or_sign {
                        bus.write_u8(addr, regs.r[rd] as u8);
                    } else {
                        bus.write_u32(addr, regs.r[rd]);
                    }
                }
            } else {
                // Format 8: Sign-extended LDRH/STRH/LDRSB/LDRSH
                match (is_byte_or_sign, is_load) {
                    (false, false) => bus.write_u16(addr, regs.r[rd] as u16),             // STRH
                    (false, true) => regs.r[rd] = (bus.read_u8(addr) as i8) as i32 as u32,  // LDRSB
                    (true, false) => regs.r[rd] = bus.read_u16(addr) as u32,              // LDRH
                    (true, true) => regs.r[rd] = (bus.read_u16(addr) as i16) as i32 as u32, // LDRSH
                }
            }
            2
        }

        // Formats 9 & 10: Load/Store Immediate Offset & Halfword
        0x6 | 0x7 | 0x8 => {
            if top_bits == 0x8 {
                // Format 10: Load/Store Halfword (LDRH / STRH)
                let is_load = (instr & (1 << 11)) != 0;
                let offset5 = (((instr >> 6) & 0x1F) as u32) * 2;
                let rb = ((instr >> 3) & 7) as usize;
                let rd = (instr & 7) as usize;
                let addr = regs.r[rb].wrapping_add(offset5);

                if is_load {
                    regs.r[rd] = bus.read_u16(addr) as u32;
                } else {
                    bus.write_u16(addr, regs.r[rd] as u16);
                }
            } else {
                // Format 9: Load/Store Immediate Offset (LDR, STR, LDRB, STRB)
                let is_byte = (instr & (1 << 12)) != 0;
                let is_load = (instr & (1 << 11)) != 0;
                let offset5 = ((instr >> 6) & 0x1F) as u32;
                let rb = ((instr >> 3) & 7) as usize;
                let rd = (instr & 7) as usize;

                let multiplier = if is_byte { 1 } else { 4 };
                let addr = regs.r[rb].wrapping_add(offset5 * multiplier);

                if is_load {
                    if is_byte {
                        regs.r[rd] = bus.read_u8(addr) as u32;
                    } else {
                        regs.r[rd] = bus.read_u32(addr);
                    }
                } else {
                    if is_byte {
                        bus.write_u8(addr, regs.r[rd] as u8);
                    } else {
                        bus.write_u32(addr, regs.r[rd]);
                    }
                }
            }
            2
        }

        // Formats 11, 12, 13, 14: SP-relative, Load Address, SP add/sub, PUSH/POP
        0x9 | 0xA | 0xB => {
            if top_bits == 0x9 {
                // Format 11: SP-Relative Load/Store (LDR/STR Rd, [SP, #imm8])
                let is_load = (instr & (1 << 11)) != 0;
                let rd = ((instr >> 8) & 7) as usize;
                let word8 = ((instr & 0xFF) as u32) * 4;
                let addr = regs.r[13].wrapping_add(word8);

                if is_load {
                    regs.r[rd] = bus.read_u32(addr);
                } else {
                    bus.write_u32(addr, regs.r[rd]);
                }
                2
            } else if top_bits == 0xA {
                // Format 12: Load Address (ADD Rd, PC/SP, #imm8)
                let use_sp = (instr & (1 << 11)) != 0;
                let rd = ((instr >> 8) & 7) as usize;
                let word8 = ((instr & 0xFF) as u32) * 4;
                let base = if use_sp {
                    regs.r[13]
                } else {
                    regs.r[15] & !2
                };
                regs.r[rd] = base.wrapping_add(word8);
                1
            } else {
                // Format 13 & 14
                if (instr & 0xFF00) == 0xB000 {
                    // Format 13: Add/Subtract Offset to SP
                    let is_sub = (instr & (1 << 7)) != 0;
                    let sword7 = ((instr & 0x7F) as u32) * 4;
                    if is_sub {
                        regs.r[13] = regs.r[13].wrapping_sub(sword7);
                    } else {
                        regs.r[13] = regs.r[13].wrapping_add(sword7);
                    }
                    1
                } else if (instr & 0xF600) == 0xB400 {
                    // Format 14: PUSH / POP
                    let is_pop = (instr & (1 << 11)) != 0;
                    let store_lr_pc = (instr & (1 << 8)) != 0;
                    let reg_list = instr & 0xFF;

                    let count = reg_list.count_ones() + if store_lr_pc { 1 } else { 0 };

                    if is_pop {
                        let mut addr = regs.r[13];
                        for r in 0..8 {
                            if (reg_list & (1 << r)) != 0 {
                                regs.r[r] = bus.read_u32(addr);
                                addr = addr.wrapping_add(4);
                            }
                        }
                        if store_lr_pc {
                            regs.r[15] = bus.read_u32(addr) & !1;
                            addr = addr.wrapping_add(4);
                        }
                        regs.r[13] = addr;
                    } else {
                        let mut addr = regs.r[13].wrapping_sub(count * 4);
                        let start_sp = addr;
                        for r in 0..8 {
                            if (reg_list & (1 << r)) != 0 {
                                bus.write_u32(addr, regs.r[r]);
                                addr = addr.wrapping_add(4);
                            }
                        }
                        if store_lr_pc {
                            bus.write_u32(addr, regs.r[14]);
                        }
                        regs.r[13] = start_sp;
                    }
                    2 + count as usize
                } else {
                    1
                }
            }
        }

        // Formats 15-19: LDMIA/STMIA, Conditional Branch, SWI, Unconditional Branch, BL
        0xC | 0xD | 0xE | 0xF => {
            if top_bits == 0xC {
                // Format 15: Multiple Load/Store (LDMIA / STMIA)
                let is_load = (instr & (1 << 11)) != 0;
                let rb = ((instr >> 8) & 7) as usize;
                let reg_list = instr & 0xFF;
                let count = reg_list.count_ones();

                let mut addr = regs.r[rb];
                for r in 0..8 {
                    if (reg_list & (1 << r)) != 0 {
                        if is_load {
                            regs.r[r] = bus.read_u32(addr);
                        } else {
                            bus.write_u32(addr, regs.r[r]);
                        }
                        addr = addr.wrapping_add(4);
                    }
                }
                regs.r[rb] = addr;
                2 + count as usize
            } else if top_bits == 0xD {
                let cond = (instr >> 8) & 0x0F;
                if cond == 0x0F {
                    // Format 17: Software Interrupt (SWI)
                    let swi_num = (instr & 0xFF) as u8;
                    super::bios::handle_swi(swi_num, regs, bus);
                    3
                } else {
                    // Format 16: Conditional Branch (B<cond>)
                    if check_condition(cond as u32, regs) {
                        let s8 = (instr & 0xFF) as i8 as i32;
                        let offset = (s8 << 1) as u32;
                        regs.r[15] = regs.r[15].wrapping_add(offset);
                        3
                    } else {
                        1
                    }
                }
            } else if top_bits == 0xE {
                // Format 18: Unconditional Branch (B label)
                let offset11 = instr & 0x07FF;
                let sign_extended = if (offset11 & 0x0400) != 0 {
                    (offset11 | 0xF800) as i16 as i32
                } else {
                    offset11 as i32
                };
                let branch_offset = (sign_extended << 1) as u32;
                regs.r[15] = regs.r[15].wrapping_add(branch_offset);
                3
            } else {
                // Format 19: Long Branch with Link (BL / BLX)
                let is_second_half = (instr & (1 << 11)) != 0;
                let offset11 = (instr & 0x07FF) as u32;

                if !is_second_half {
                    let pc = regs.r[15].wrapping_sub(4);
                    let next_instr = bus.read_u16(pc.wrapping_add(2));
                    if (next_instr & 0xF000) == 0xF800 || (next_instr & 0xF000) == 0xE800 {
                        let sign_extended = if (offset11 & 0x0400) != 0 {
                            (offset11 | 0xFFFFF800) as i32
                        } else {
                            offset11 as i32
                        };
                        let tmp_lr = (regs.r[15] as i32).wrapping_add(sign_extended << 12) as u32;
                        let offset2 = (next_instr & 0x07FF) as u32;
                        let is_blx = (next_instr & (1 << 12)) == 0;

                        let target = tmp_lr.wrapping_add(offset2 << 1);
                        regs.r[14] = (pc.wrapping_add(4)) | 1;
                        if is_blx {
                            regs.set_thumb_mode(false);
                            regs.r[15] = target & !3;
                        } else {
                            regs.r[15] = target & !1;
                        }
                        return 4;
                    }

                    // Fallback single half
                    let sign_extended = if (offset11 & 0x0400) != 0 {
                        (offset11 | 0xFFFFF800) as i32
                    } else {
                        offset11 as i32
                    };
                    regs.r[14] = (regs.r[15] as i32).wrapping_add(sign_extended << 12) as u32;
                } else {
                    // Second 16-bit instruction standalone fallback
                    let target = regs.r[14].wrapping_add(offset11 << 1);
                    regs.r[14] = (regs.r[15].wrapping_sub(2)) | 1;
                    regs.r[15] = target;
                }
                2
            }
        }

        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumb_add_sub() {
        let mut regs = Registers::new();
        let mut bus = GbaMemoryBus::new();
        regs.set_thumb_mode(true);

        // Format 3: ADD r1, #8 => 0x3108
        let add_imm8 = 0x3108;
        execute_thumb(&mut regs, &mut bus, add_imm8);
        assert_eq!(regs.r[1], 8);
    }
}

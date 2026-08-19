#![allow(dead_code)]

use super::cpu::Registers;
use super::mmu::GbaMemoryBus;

/// High-Level Emulation (HLE) for GBA BIOS Software Interrupts (SWI).
pub fn handle_swi(comment: u8, regs: &mut Registers, bus: &mut GbaMemoryBus) {
    if bus.has_real_bios {
        let old_cpsr = regs.cpsr;
        let is_thumb = regs.thumb_mode();
        let return_pc = if is_thumb {
            regs.r[15].wrapping_sub(2)
        } else {
            regs.r[15].wrapping_sub(4)
        };

        regs.set_mode(super::cpu::CpuMode::Supervisor);
        regs.set_spsr(old_cpsr);
        regs.set_irq_disabled(true);
        regs.set_thumb_mode(false); // Switch to ARM mode
        regs.r[14] = return_pc; // LR_svc
        regs.r[15] = 0x00000008; // Hardware SWI Exception Vector
        return;
    }

    let is_thumb = regs.thumb_mode();
    let swi_pc = if is_thumb {
        regs.r[15].wrapping_sub(4)
    } else {
        regs.r[15].wrapping_sub(8)
    };

    // Advance PC past the SWI instruction (2 bytes in THUMB mode, 4 bytes in ARM mode)
    regs.r[15] = if is_thumb {
        swi_pc.wrapping_add(2)
    } else {
        swi_pc.wrapping_add(4)
    };

    // Only warn for SWI numbers above 0x2B (documented GBA BIOS limit)
    if comment > 0x2B {
        let sp = regs.sp();
        let val0 = bus.read_u32(sp);
        let val1 = bus.read_u32(sp.wrapping_add(4));
        let val2 = bus.read_u32(sp.wrapping_add(8));
        let val3 = bus.read_u32(sp.wrapping_add(12));
        log::debug!(
            "Invalid SWI 0x{:02X} requested at PC=0x{:08X}! Stack: [SP+0=0x{:08X}, SP+4=0x{:08X}, SP+8=0x{:08X}, SP+12=0x{:08X}]",
            comment, swi_pc, val0, val1, val2, val3
        );
        return;
    }

    log::debug!(
        "GBA BIOS SWI 0x{:02X} invoked at PC=0x{:08X}",
        comment,
        swi_pc
    );

    match comment {
        0x01 => {
            // SWI 0x01: RegisterRamReset
            // Input: R0 = Bitmask of RAM/IO areas to reset
            // Output: R0 = 0
            let flags = regs.r[0] as u8;

            if (flags & (1 << 0)) != 0 {
                // Reset EWRAM (256 KB at 0x02000000)
                bus.ewram.fill(0);
            }
            if (flags & (1 << 1)) != 0 {
                // Reset IWRAM (32 KB at 0x03000000 except 0x7E00-0x7FFF for IRQ vector & SP)
                let clear_end = 0x7E00.min(bus.iwram.len());
                bus.iwram[..clear_end].fill(0);
            }
            if (flags & (1 << 2)) != 0 {
                // Reset Palette RAM (1 KB at 0x05000000)
                bus.ppu.palette.fill(0);
            }
            if (flags & (1 << 3)) != 0 {
                // Reset VRAM (96 KB at 0x06000000)
                bus.ppu.vram.fill(0);
            }
            if (flags & (1 << 4)) != 0 {
                // Reset OAM (1 KB at 0x07000000)
                bus.ppu.oam.fill(0);
            }
            if (flags & (1 << 5)) != 0 {
                // Reset SIO Registers (0x04000120 - 0x0400012C)
                for a in 0x04000120..=0x0400012C {
                    bus.write_u8(a, 0);
                }
            }
            if (flags & (1 << 6)) != 0 {
                // Reset Sound Registers (0x04000060 - 0x040000A8)
                for a in 0x04000060..=0x040000A8 {
                    bus.write_u8(a, 0);
                }
            }
            if (flags & (1 << 7)) != 0 {
                // Reset Display and Other I/O Registers (0x04000000 - 0x04000056)
                bus.ppu.dispcnt = 0;
                bus.ppu.dispstat = 0;
                bus.ppu.bg0cnt = 0;
                bus.ppu.bg1cnt = 0;
                bus.ppu.bg2cnt = 0;
                bus.ppu.bg3cnt = 0;
            }

            // GBA BIOS spec: RegisterRamReset clears R0 to 0 upon completion
            regs.r[0] = 0;
        }

        0x02 => {
            // SWI 0x02: Halt — HLE: immediately satisfy by signaling VBlank IRQ
            // In HLE mode we cannot truly halt the CPU; instead we pre-satisfy the
            // VBlank interrupt so any subsequent IntrWait/VBlankIntrWait returns instantly.
            hle_signal_vblank(bus);
        }

        0x03 => {
            // SWI 0x03: Stop — treated same as Halt in HLE
            hle_signal_vblank(bus);
        }

        0x04 => {
            // SWI 0x04: IntrWait
            // R0 = Clear Flags (1 = clear old flags), R1 = IRQ Mask
            handle_intr_wait(regs, bus);
        }

        0x05 => {
            // SWI 0x05: VBlankIntrWait — wait for VBlank IRQ (bit 0)
            regs.r[0] = 1;
            regs.r[1] = 1;
            handle_intr_wait(regs, bus);
        }

        0x06 | 0x07 => {
            // SWI 0x06: Div / SWI 0x07: DivArm
            // Input: R0 = Num (signed 32-bit), R1 = Denom (signed 32-bit)
            // Output: R0 = Num / Denom, R1 = Num % Denom, R3 = ABS(Num / Denom)
            let num = regs.r[0] as i32;
            let denom = regs.r[1] as i32;

            if denom != 0 {
                let (div, rem) = if num == i32::MIN && denom == -1 {
                    (i32::MIN, 0)
                } else {
                    (num / denom, num % denom)
                };
                let abs_div = div.unsigned_abs();

                regs.r[0] = div as u32;
                regs.r[1] = rem as u32;
                regs.r[3] = abs_div;
            }
        }

        0x0B | 0x0E => {
            // SWI 0x0B / 0x0E: CpuSet (16-bit / 32-bit block copy / fill)
            let src_addr = regs.r[0];
            let dst_addr = regs.r[1];
            let control = regs.r[2];
            let count = control & 0x001F_FFFF;
            let fill_mode = (control & (1 << 24)) != 0;
            let is_32bit = (control & (1 << 26)) != 0;

            let mut curr_src = src_addr;
            let mut curr_dst = dst_addr;

            if is_32bit {
                let fill_val = if fill_mode { bus.read_u32(src_addr) } else { 0 };
                for _ in 0..count {
                    let val = if fill_mode {
                        fill_val
                    } else {
                        bus.read_u32(curr_src)
                    };
                    bus.write_u32(curr_dst, val);
                    if !fill_mode {
                        curr_src = curr_src.wrapping_add(4);
                    }
                    curr_dst = curr_dst.wrapping_add(4);
                }
            } else {
                let fill_val = if fill_mode { bus.read_u16(src_addr) } else { 0 };
                for _ in 0..count {
                    let val = if fill_mode {
                        fill_val
                    } else {
                        bus.read_u16(curr_src)
                    };
                    bus.write_u16(curr_dst, val);
                    if !fill_mode {
                        curr_src = curr_src.wrapping_add(2);
                    }
                    curr_dst = curr_dst.wrapping_add(2);
                }
            }
        }

        0x0C | 0x0F => {
            // SWI 0x0C / 0x0F: CpuFastSet (32-bit word block copy / fill)
            let src = regs.r[0];
            let dst = regs.r[1];
            let cnt_h = regs.r[2];
            let count = cnt_h & 0x001F_FFFF;
            let is_fixed = (cnt_h & (1 << 24)) != 0;

            let mut curr_src = src;
            let mut curr_dst = dst;

            for _ in 0..count {
                let val = bus.read_u32(curr_src);
                bus.write_u32(curr_dst, val);
                if !is_fixed {
                    curr_src = curr_src.wrapping_add(4);
                }
                curr_dst = curr_dst.wrapping_add(4);
            }
        }

        0x08 => {
            // SWI 0x08: Sqrt — R0 = sqrt(R0), result in R0
            let val = regs.r[0];
            regs.r[0] = (val as f64).sqrt() as u32;
        }

        0x09 => {
            // SWI 0x09: ArcTan — R0 = ArcTan(R0), result in R0
            // Input is 1.14 fixed-point; output is 0.14 fixed-point (range: -0x4000..0x4000)
            let input = regs.r[0] as i32;
            let angle = (input as f64 / 16384.0).atan();
            regs.r[0] = ((angle / std::f64::consts::PI * 16384.0) as i32) as u32;
        }

        0x0A => {
            // SWI 0x0A: ArcTan2 — R0 = ArcTan2(R1, R0), result in R0
            let x = regs.r[0] as i32 as f64 / 16384.0;
            let y = regs.r[1] as i32 as f64 / 16384.0;
            let angle = y.atan2(x);
            regs.r[0] = ((angle / (2.0 * std::f64::consts::PI) * 65536.0) as u32) & 0xFFFF;
        }

        0x11 | 0x12 => {
            // SWI 0x11 / 0x12: LZ77UncompWram / LZ77UncompVram
            let src = regs.r[0];
            let dst = regs.r[1];
            decompress_lz77(src, dst, bus);
        }

        0x1D => {
            // SWI 0x1D: CustomHalt / SoundDriverMain (Pokemon-specific extension)
            // In HLE mode: no-op — sound driver is not emulated; just return cleanly.
            // This prevents the CPU from spinning in an invalid-SWI loop.
        }

        _ => {
            // All other recognised-range SWIs (0x00..=0x2B) not explicitly handled:
            // silently ignore so the game can continue rather than spinning.
        }
    }
}

/// HLE helper: pre-satisfy the VBlank interrupt so IntrWait loops exit immediately.
/// In a real GBA the CPU would halt; in HLE mode we cannot do that, so we signal
/// the interrupt as already fired so the game's wait loop sees it satisfied.
fn hle_signal_vblank(bus: &mut GbaMemoryBus) {
    // Set IF bit 0 (VBlank) in the hardware Interrupt Flag register
    let cur_if = bus.read_u16(0x04000202);
    bus.io[0x202] = (cur_if | 0x0001) as u8;
    bus.io[0x203] = ((cur_if | 0x0001) >> 8) as u8;

    // Set bit 0 in the BIOS interrupt check word used by IntrWait / VBlankIntrWait
    let intr_check = bus.read_u16(0x03007FF8);
    let lo = (intr_check | 0x0001) as u8;
    let hi = ((intr_check | 0x0001) >> 8) as u8;
    bus.iwram[0x7FF8] = lo;
    bus.iwram[0x7FF9] = hi;
}

/// Helper function for IntrWait (SWI 0x04) and VBlankIntrWait (SWI 0x05).
/// In HLE mode, pre-satisfy the requested interrupt mask so the game's wait loop
/// exits on the very next check rather than spinning forever.
fn handle_intr_wait(regs: &mut Registers, bus: &mut GbaMemoryBus) {
    let irq_mask = regs.r[1] as u16;

    // Pre-satisfy: write the requested IRQ bits into IF so the game sees them fired
    let cur_if = bus.read_u16(0x04000202);
    let new_if = cur_if | irq_mask;
    bus.io[0x202] = new_if as u8;
    bus.io[0x203] = (new_if >> 8) as u8;

    // Also set the BIOS interrupt check word at 0x03007FF8
    let intr_check = bus.read_u16(0x03007FF8);
    let new_check = intr_check | irq_mask;
    bus.iwram[0x7FF8] = new_check as u8;
    bus.iwram[0x7FF9] = (new_check >> 8) as u8;
}

/// GBA BIOS LZ77 Decompressor (used by GBA games for graphics & WRAM loading)
fn decompress_lz77(src: u32, dst: u32, bus: &mut GbaMemoryBus) {
    let header = bus.read_u32(src);
    let comp_type = (header >> 4) & 0x0F;
    if comp_type != 1 {
        return; // Only LZ77 (type 1) supported
    }

    let decomp_size = (header >> 8) as usize;
    let mut src_curr = src + 4;
    let mut dst_curr = dst;
    let dst_end = dst + decomp_size as u32;

    while dst_curr < dst_end {
        let flags = bus.read_u8(src_curr);
        src_curr = src_curr.wrapping_add(1);

        for bit in (0..8).rev() {
            if dst_curr >= dst_end {
                break;
            }

            if (flags & (1 << bit)) != 0 {
                // Compressed block (2 bytes)
                let b0 = bus.read_u8(src_curr) as u16;
                let b1 = bus.read_u8(src_curr.wrapping_add(1)) as u16;
                src_curr = src_curr.wrapping_add(2);

                let length = ((b0 >> 4) + 3) as u32;
                let disp = (((b0 & 0x0F) << 8) | b1) as u32 + 1;

                let mut copy_src = dst_curr.wrapping_sub(disp);
                for _ in 0..length {
                    if dst_curr >= dst_end {
                        break;
                    }
                    let val = bus.read_u8(copy_src);
                    bus.write_u8(dst_curr, val);
                    copy_src = copy_src.wrapping_add(1);
                    dst_curr = dst_curr.wrapping_add(1);
                }
            } else {
                // Uncompressed literal byte
                let val = bus.read_u8(src_curr);
                src_curr = src_curr.wrapping_add(1);
                bus.write_u8(dst_curr, val);
                dst_curr = dst_curr.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swi_cpuset_copy_and_fill() {
        let mut regs = Registers::new();
        let mut bus = GbaMemoryBus::new();

        // 1. 32-bit Fill Mode: fill 4 words at 0x03000000 with 0xDEADBEEF
        bus.write_u32(0x02000000, 0xDEADBEEF);
        regs.r[0] = 0x02000000; // src
        regs.r[1] = 0x03000000; // dst
        regs.r[2] = 4 | (1 << 24) | (1 << 26); // count=4, fill=1, 32bit=1
        handle_swi(0x0B, &mut regs, &mut bus);

        assert_eq!(bus.read_u32(0x03000000), 0xDEADBEEF);
        assert_eq!(bus.read_u32(0x03000004), 0xDEADBEEF);
        assert_eq!(bus.read_u32(0x03000008), 0xDEADBEEF);
        assert_eq!(bus.read_u32(0x0300000C), 0xDEADBEEF);

        // 2. 16-bit Copy Mode: copy 2 halfwords from 0x02000010 to 0x03000020
        bus.write_u16(0x02000010, 0x1234);
        bus.write_u16(0x02000012, 0x5678);
        regs.r[0] = 0x02000010; // src
        regs.r[1] = 0x03000020; // dst
        regs.r[2] = 2; // count=2, fill=0, 16bit=0
        handle_swi(0x0B, &mut regs, &mut bus);

        assert_eq!(bus.read_u16(0x03000020), 0x1234);
        assert_eq!(bus.read_u16(0x03000022), 0x5678);
    }

    #[test]
    fn test_swi_div_overflow_boundary() {
        let mut regs = Registers::new();
        let mut bus = GbaMemoryBus::new();

        // 1. Normal division: 100 / 7 = 14 rem 2, abs 14
        regs.r[0] = 100;
        regs.r[1] = 7;
        handle_swi(0x06, &mut regs, &mut bus);
        assert_eq!(regs.r[0], 14);
        assert_eq!(regs.r[1], 2);
        assert_eq!(regs.r[3], 14);

        // 2. Negative division: -50 / 8 = -6 rem -2, abs 6
        regs.r[0] = (-50i32) as u32;
        regs.r[1] = 8;
        handle_swi(0x06, &mut regs, &mut bus);
        assert_eq!(regs.r[0] as i32, -6);
        assert_eq!(regs.r[1] as i32, -2);
        assert_eq!(regs.r[3], 6);

        // 3. Overflow boundary: i32::MIN / -1 must not panic
        regs.r[0] = 0x80000000; // -2147483648
        regs.r[1] = 0xFFFFFFFF; // -1
        handle_swi(0x06, &mut regs, &mut bus);
        assert_eq!(regs.r[0], 0x80000000);
        assert_eq!(regs.r[1], 0);
        assert_eq!(regs.r[3], 0x80000000);
    }
}

use super::mmu::MemoryBus;

/// Game Boy Hardware Timer and Divider Subsystem (0xFF04 - 0xFF07).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Timer {
    div_counter: u16,
    tima_counter: u16,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            div_counter: 0,
            tima_counter: 0,
        }
    }

    /// Advances Timer and Divider registers by `cycles` CPU T-cycles.
    pub fn step(&mut self, cycles: u8, bus: &mut MemoryBus) {
        // DIV register (0xFF04) increments at 16384 Hz (every 256 T-cycles)
        self.div_counter = self.div_counter.wrapping_add(cycles as u16);
        let div_val = (self.div_counter >> 8) as u8;
        bus.set_div_direct(div_val);

        let tac = bus.read_byte(0xFF07);
        let timer_enabled = (tac & 0x04) != 0;

        if timer_enabled {
            let clock_select = tac & 0x03;
            let threshold: u16 = match clock_select {
                0 => 1024, // 4096 Hz
                1 => 16,   // 262144 Hz
                2 => 64,   // 65536 Hz
                _ => 256,  // 16384 Hz
            };

            self.tima_counter += cycles as u16;
            while self.tima_counter >= threshold {
                self.tima_counter -= threshold;
                let tima = bus.read_byte(0xFF05);
                if tima == 0xFF {
                    let tma = bus.read_byte(0xFF06);
                    bus.set_tima_direct(tma);
                    // Request Timer Interrupt (bit 2 of IF 0xFF0F)
                    let if_reg = bus.read_byte(0xFF0F);
                    bus.write_byte(0xFF0F, if_reg | 0x04);
                } else {
                    bus.set_tima_direct(tima.wrapping_add(1));
                }
            }
        }
    }

    /// Resets internal DIV counter when CPU writes to 0xFF04.
    #[allow(dead_code)]
    pub fn reset_div(&mut self) {
        self.div_counter = 0;
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

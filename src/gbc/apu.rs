use crate::audio::AudioProducer;

const CPU_CLOCK_HZ: f64 = 4_194_304.0;
const FRAME_SEQUENCER_PERIOD: u32 = 8192; // 512 Hz at 4.194304 MHz

const DUTY_CYCLES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [1, 0, 0, 0, 0, 0, 0, 1], // 25.0%
    [1, 0, 0, 0, 0, 1, 1, 1], // 50.0%
    [0, 1, 1, 1, 1, 1, 1, 0], // 75.0%
];

// ============================================================================
// Channel 1: Square Wave with Sweep and Envelope
// ============================================================================

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Channel1 {
    pub enabled: bool,
    pub dac_enabled: bool,

    // NR10: Sweep
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,
    pub sweep_timer: u8,
    pub sweep_shadow_freq: u16,
    pub sweep_enabled: bool,

    // NR11: Duty & Length
    pub duty: u8,
    pub duty_step: u8,
    pub length_counter: u16,

    // NR12: Envelope
    pub initial_volume: u8,
    pub envelope_volume: u8,
    pub envelope_increase: bool,
    pub envelope_period: u8,
    pub envelope_timer: u8,

    // NR13 & NR14: Frequency & Control
    pub frequency: u16,
    pub timer: u32,
    pub length_enable: bool,
}

impl Channel1 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.sweep_period = (val >> 4) & 0x07;
        self.sweep_negate = (val & 0x08) != 0;
        self.sweep_shift = val & 0x07;
    }

    pub fn read_nr10(&self) -> u8 {
        0x80 | (self.sweep_period << 4)
            | (if self.sweep_negate { 0x08 } else { 0 })
            | self.sweep_shift
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.duty = (val >> 6) & 0x03;
        self.length_counter = 64 - (val & 0x3F) as u16;
    }

    pub fn read_nr11(&self) -> u8 {
        0x3F | (self.duty << 6)
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.initial_volume = (val >> 4) & 0x0F;
        self.envelope_increase = (val & 0x08) != 0;
        self.envelope_period = val & 0x07;
        self.dac_enabled = (val & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub fn read_nr12(&self) -> u8 {
        (self.initial_volume << 4)
            | (if self.envelope_increase { 0x08 } else { 0 })
            | self.envelope_period
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.frequency = (self.frequency & 0x0700) | val as u16;
    }

    pub fn write_nr14(&mut self, val: u8) {
        self.frequency = (self.frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
        self.length_enable = (val & 0x40) != 0;

        if (val & 0x80) != 0 {
            self.trigger();
        }
    }

    pub fn read_nr14(&self) -> u8 {
        0xBF | (if self.length_enable { 0x40 } else { 0 })
    }

    pub fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.timer = (2048 - self.frequency as u32) * 4;
        self.envelope_volume = self.initial_volume;
        self.envelope_timer = if self.envelope_period > 0 {
            self.envelope_period
        } else {
            8
        };

        // Sweep initialization
        self.sweep_shadow_freq = self.frequency;
        self.sweep_timer = if self.sweep_period > 0 {
            self.sweep_period
        } else {
            8
        };
        self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;

        if self.sweep_shift > 0 && self.calculate_sweep_freq() > 2047 {
            self.enabled = false;
        }
    }

    fn calculate_sweep_freq(&self) -> u16 {
        let delta = self.sweep_shadow_freq >> self.sweep_shift;
        if self.sweep_negate {
            self.sweep_shadow_freq.saturating_sub(delta)
        } else {
            self.sweep_shadow_freq + delta
        }
    }

    pub fn clock_sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period > 0 {
                self.sweep_period
            } else {
                8
            };
            if self.sweep_enabled && self.sweep_period > 0 {
                let new_freq = self.calculate_sweep_freq();
                if new_freq <= 2047 && self.sweep_shift > 0 {
                    self.sweep_shadow_freq = new_freq;
                    self.frequency = new_freq;
                    if self.calculate_sweep_freq() > 2047 {
                        self.enabled = false;
                    }
                } else if new_freq > 2047 {
                    self.enabled = false;
                }
            }
        }
    }

    pub fn clock_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    pub fn clock_envelope(&mut self) {
        if self.envelope_period > 0 {
            if self.envelope_timer > 0 {
                self.envelope_timer -= 1;
            }
            if self.envelope_timer == 0 {
                self.envelope_timer = self.envelope_period;
                if self.envelope_increase && self.envelope_volume < 15 {
                    self.envelope_volume += 1;
                } else if !self.envelope_increase && self.envelope_volume > 0 {
                    self.envelope_volume -= 1;
                }
            }
        }
    }

    pub fn step(&mut self, mut cycles: u32) {
        let period = ((2048 - self.frequency as u32) * 4).max(1);
        while self.timer <= cycles {
            cycles -= self.timer;
            self.timer = period;
            self.duty_step = (self.duty_step + 1) & 7;
        }
        self.timer -= cycles;
    }

    pub fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let duty_val = DUTY_CYCLES[self.duty as usize][self.duty_step as usize];
        let digital_val = if duty_val == 1 {
            self.envelope_volume
        } else {
            0
        };
        (digital_val as f32 - 7.5) / 7.5
    }
}

// ============================================================================
// Channel 2: Square Wave with Envelope
// ============================================================================

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Channel2 {
    pub enabled: bool,
    pub dac_enabled: bool,

    // NR21: Duty & Length
    pub duty: u8,
    pub duty_step: u8,
    pub length_counter: u16,

    // NR22: Envelope
    pub initial_volume: u8,
    pub envelope_volume: u8,
    pub envelope_increase: bool,
    pub envelope_period: u8,
    pub envelope_timer: u8,

    // NR23 & NR24: Frequency & Control
    pub frequency: u16,
    pub timer: u32,
    pub length_enable: bool,
}

impl Channel2 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.duty = (val >> 6) & 0x03;
        self.length_counter = 64 - (val & 0x3F) as u16;
    }

    pub fn read_nr21(&self) -> u8 {
        0x3F | (self.duty << 6)
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.initial_volume = (val >> 4) & 0x0F;
        self.envelope_increase = (val & 0x08) != 0;
        self.envelope_period = val & 0x07;
        self.dac_enabled = (val & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub fn read_nr22(&self) -> u8 {
        (self.initial_volume << 4)
            | (if self.envelope_increase { 0x08 } else { 0 })
            | self.envelope_period
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.frequency = (self.frequency & 0x0700) | val as u16;
    }

    pub fn write_nr24(&mut self, val: u8) {
        self.frequency = (self.frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
        self.length_enable = (val & 0x40) != 0;

        if (val & 0x80) != 0 {
            self.trigger();
        }
    }

    pub fn read_nr24(&self) -> u8 {
        0xBF | (if self.length_enable { 0x40 } else { 0 })
    }

    pub fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.timer = (2048 - self.frequency as u32) * 4;
        self.envelope_volume = self.initial_volume;
        self.envelope_timer = if self.envelope_period > 0 {
            self.envelope_period
        } else {
            8
        };
    }

    pub fn clock_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    pub fn clock_envelope(&mut self) {
        if self.envelope_period > 0 {
            if self.envelope_timer > 0 {
                self.envelope_timer -= 1;
            }
            if self.envelope_timer == 0 {
                self.envelope_timer = self.envelope_period;
                if self.envelope_increase && self.envelope_volume < 15 {
                    self.envelope_volume += 1;
                } else if !self.envelope_increase && self.envelope_volume > 0 {
                    self.envelope_volume -= 1;
                }
            }
        }
    }

    pub fn step(&mut self, mut cycles: u32) {
        let period = ((2048 - self.frequency as u32) * 4).max(1);
        while self.timer <= cycles {
            cycles -= self.timer;
            self.timer = period;
            self.duty_step = (self.duty_step + 1) & 7;
        }
        self.timer -= cycles;
    }

    pub fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let duty_val = DUTY_CYCLES[self.duty as usize][self.duty_step as usize];
        let digital_val = if duty_val == 1 {
            self.envelope_volume
        } else {
            0
        };
        (digital_val as f32 - 7.5) / 7.5
    }
}

// ============================================================================
// Channel 3: Custom Wave Pattern RAM
// ============================================================================

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Channel3 {
    pub enabled: bool,
    pub dac_enabled: bool,

    // NR30: DAC
    // NR31: Length
    pub length_counter: u16,

    // NR32: Volume
    pub volume_code: u8,

    // NR33 & NR34: Frequency & Control
    pub frequency: u16,
    pub timer: u32,
    pub length_enable: bool,

    // Wave RAM: 16 bytes = 32 4-bit samples
    pub wave_ram: [u8; 16],
    pub sample_pos: u8,
}

impl Channel3 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_nr30(&mut self, val: u8) {
        self.dac_enabled = (val & 0x80) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub fn read_nr30(&self) -> u8 {
        0x7F | (if self.dac_enabled { 0x80 } else { 0 })
    }

    pub fn write_nr31(&mut self, val: u8) {
        self.length_counter = 256 - val as u16;
    }

    pub fn read_nr31(&self) -> u8 {
        0xFF
    }

    pub fn write_nr32(&mut self, val: u8) {
        self.volume_code = (val >> 5) & 0x03;
    }

    pub fn read_nr32(&self) -> u8 {
        0x9F | (self.volume_code << 5)
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.frequency = (self.frequency & 0x0700) | val as u16;
    }

    pub fn write_nr34(&mut self, val: u8) {
        self.frequency = (self.frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
        self.length_enable = (val & 0x40) != 0;

        if (val & 0x80) != 0 {
            self.trigger();
        }
    }

    pub fn read_nr34(&self) -> u8 {
        0xBF | (if self.length_enable { 0x40 } else { 0 })
    }

    pub fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 256;
        }
        self.timer = (2048 - self.frequency as u32) * 2;
        self.sample_pos = 0;
    }

    pub fn clock_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    pub fn step(&mut self, mut cycles: u32) {
        let period = ((2048 - self.frequency as u32) * 2).max(1);
        while self.timer <= cycles {
            cycles -= self.timer;
            self.timer = period;
            self.sample_pos = (self.sample_pos + 1) & 31;
        }
        self.timer -= cycles;
    }

    pub fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled || self.volume_code == 0 {
            return 0.0;
        }

        let byte = self.wave_ram[(self.sample_pos / 2) as usize];
        let raw_4bit = if (self.sample_pos & 1) == 0 {
            (byte >> 4) & 0x0F
        } else {
            byte & 0x0F
        };

        let shifted = match self.volume_code {
            1 => raw_4bit,      // 100%
            2 => raw_4bit >> 1, // 50%
            3 => raw_4bit >> 2, // 25%
            _ => 0,             // Mute
        };

        (shifted as f32 - 7.5) / 7.5
    }
}

// ============================================================================
// Channel 4: Noise with LFSR
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Channel4 {
    pub enabled: bool,
    pub dac_enabled: bool,

    // NR41: Length
    pub length_counter: u16,

    // NR42: Envelope
    pub initial_volume: u8,
    pub envelope_volume: u8,
    pub envelope_increase: bool,
    pub envelope_period: u8,
    pub envelope_timer: u8,

    // NR43: Polynomial counter
    pub clock_shift: u8,
    pub width_mode_7bit: bool,
    pub divisor_code: u8,

    // NR44: Control
    pub length_enable: bool,

    pub lfsr: u16,
    pub timer: u32,
}

impl Default for Channel4 {
    fn default() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            length_counter: 0,
            initial_volume: 0,
            envelope_volume: 0,
            envelope_increase: false,
            envelope_period: 0,
            envelope_timer: 0,
            clock_shift: 0,
            width_mode_7bit: false,
            divisor_code: 0,
            length_enable: false,
            lfsr: 0x7FFF,
            timer: 0,
        }
    }
}

impl Channel4 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_nr41(&mut self, val: u8) {
        self.length_counter = 64 - (val & 0x3F) as u16;
    }

    pub fn read_nr41(&self) -> u8 {
        0xFF
    }

    pub fn write_nr42(&mut self, val: u8) {
        self.initial_volume = (val >> 4) & 0x0F;
        self.envelope_increase = (val & 0x08) != 0;
        self.envelope_period = val & 0x07;
        self.dac_enabled = (val & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    pub fn read_nr42(&self) -> u8 {
        (self.initial_volume << 4)
            | (if self.envelope_increase { 0x08 } else { 0 })
            | self.envelope_period
    }

    pub fn write_nr43(&mut self, val: u8) {
        self.clock_shift = (val >> 4) & 0x0F;
        self.width_mode_7bit = (val & 0x08) != 0;
        self.divisor_code = val & 0x07;
    }

    pub fn read_nr43(&self) -> u8 {
        (self.clock_shift << 4) | (if self.width_mode_7bit { 0x08 } else { 0 }) | self.divisor_code
    }

    pub fn write_nr44(&mut self, val: u8) {
        self.length_enable = (val & 0x40) != 0;
        if (val & 0x80) != 0 {
            self.trigger();
        }
    }

    pub fn read_nr44(&self) -> u8 {
        0xBF | (if self.length_enable { 0x40 } else { 0 })
    }

    fn calc_period(&self) -> u32 {
        let divisor: u32 = match self.divisor_code {
            0 => 8,
            1 => 16,
            2 => 32,
            3 => 48,
            4 => 64,
            5 => 80,
            6 => 96,
            _ => 112,
        };
        divisor << self.clock_shift
    }

    pub fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.timer = self.calc_period();
        self.lfsr = 0x7FFF;
        self.envelope_volume = self.initial_volume;
        self.envelope_timer = if self.envelope_period > 0 {
            self.envelope_period
        } else {
            8
        };
    }

    pub fn clock_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    pub fn clock_envelope(&mut self) {
        if self.envelope_period > 0 {
            if self.envelope_timer > 0 {
                self.envelope_timer -= 1;
            }
            if self.envelope_timer == 0 {
                self.envelope_timer = self.envelope_period;
                if self.envelope_increase && self.envelope_volume < 15 {
                    self.envelope_volume += 1;
                } else if !self.envelope_increase && self.envelope_volume > 0 {
                    self.envelope_volume -= 1;
                }
            }
        }
    }

    pub fn step(&mut self, mut cycles: u32) {
        let period = self.calc_period().max(1);
        while self.timer <= cycles {
            cycles -= self.timer;
            self.timer = period;

            let xor_result = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr = (self.lfsr >> 1) | (xor_result << 14);
            if self.width_mode_7bit {
                self.lfsr = (self.lfsr & !(1 << 6)) | (xor_result << 6);
            }
        }
        self.timer -= cycles;
    }

    pub fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let digital_val = if (self.lfsr & 1) == 0 {
            self.envelope_volume
        } else {
            0
        };
        (digital_val as f32 - 7.5) / 7.5
    }
}

// ============================================================================
// Main 4-Channel APU
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Apu {
    pub ch1: Channel1,
    pub ch2: Channel2,
    pub ch3: Channel3,
    pub ch4: Channel4,

    // NR50: Master Volume
    pub nr50: u8,
    // NR51: Panning
    pub nr51: u8,
    // NR52: Master Power / Status
    pub power_on: bool,

    // Frame Sequencer
    frame_sequencer_timer: u32,
    frame_sequencer_step: u8,

    // Sample Clock & Output Buffer
    sample_timer: f64,
    sample_rate: f64,
    pub sample_buffer: Vec<f32>,
    #[serde(skip)]
    audio_producer: Option<AudioProducer>,
}

impl Default for Apu {
    fn default() -> Self {
        Self {
            ch1: Channel1::new(),
            ch2: Channel2::new(),
            ch3: Channel3::new(),
            ch4: Channel4::new(),
            nr50: 0x77,
            nr51: 0xF3,
            power_on: true,
            frame_sequencer_timer: FRAME_SEQUENCER_PERIOD,
            frame_sequencer_step: 0,
            sample_timer: 0.0,
            sample_rate: 48000.0,
            sample_buffer: Vec::with_capacity(2048),
            audio_producer: None,
        }
    }
}

impl Apu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_audio_producer(&mut self, producer: Option<AudioProducer>) {
        if let Some(ref prod) = producer {
            prod.set_input_sample_rate(self.sample_rate);
        }
        self.audio_producer = producer;
    }

    /// Read APU IO registers (0xFF10..=0xFF3F)
    pub fn read_register(&self, addr: u16) -> u8 {
        if !self.power_on && addr < 0xFF30 {
            return match addr {
                0xFF26 => 0x70,
                _ => 0xFF,
            };
        }

        match addr {
            0xFF10 => self.ch1.read_nr10(),
            0xFF11 => self.ch1.read_nr11(),
            0xFF12 => self.ch1.read_nr12(),
            0xFF13 => 0xFF,
            0xFF14 => self.ch1.read_nr14(),

            0xFF15 => 0xFF,
            0xFF16 => self.ch2.read_nr21(),
            0xFF17 => self.ch2.read_nr22(),
            0xFF18 => 0xFF,
            0xFF19 => self.ch2.read_nr24(),

            0xFF1A => self.ch3.read_nr30(),
            0xFF1B => self.ch3.read_nr31(),
            0xFF1C => self.ch3.read_nr32(),
            0xFF1D => 0xFF,
            0xFF1E => self.ch3.read_nr34(),

            0xFF1F => 0xFF,
            0xFF20 => self.ch4.read_nr41(),
            0xFF21 => self.ch4.read_nr42(),
            0xFF22 => self.ch4.read_nr43(),
            0xFF23 => self.ch4.read_nr44(),

            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                let bit7 = if self.power_on { 0x80 } else { 0x00 };
                let bit3 = if self.ch4.enabled { 0x08 } else { 0x00 };
                let bit2 = if self.ch3.enabled { 0x04 } else { 0x00 };
                let bit1 = if self.ch2.enabled { 0x02 } else { 0x00 };
                let bit0 = if self.ch1.enabled { 0x01 } else { 0x00 };
                0x70 | bit7 | bit3 | bit2 | bit1 | bit0
            }

            0xFF27..=0xFF2F => 0xFF,

            0xFF30..=0xFF3F => {
                let idx = (addr - 0xFF30) as usize;
                self.ch3.wave_ram[idx]
            }

            _ => 0xFF,
        }
    }

    /// Write APU IO registers (0xFF10..=0xFF3F)
    pub fn write_register(&mut self, addr: u16, val: u8) {
        if !self.power_on && addr < 0xFF30 && addr != 0xFF26 {
            return;
        }

        match addr {
            0xFF10 => self.ch1.write_nr10(val),
            0xFF11 => self.ch1.write_nr11(val),
            0xFF12 => self.ch1.write_nr12(val),
            0xFF13 => self.ch1.write_nr13(val),
            0xFF14 => self.ch1.write_nr14(val),

            0xFF16 => self.ch2.write_nr21(val),
            0xFF17 => self.ch2.write_nr22(val),
            0xFF18 => self.ch2.write_nr23(val),
            0xFF19 => self.ch2.write_nr24(val),

            0xFF1A => self.ch3.write_nr30(val),
            0xFF1B => self.ch3.write_nr31(val),
            0xFF1C => self.ch3.write_nr32(val),
            0xFF1D => self.ch3.write_nr33(val),
            0xFF1E => self.ch3.write_nr34(val),

            0xFF20 => self.ch4.write_nr41(val),
            0xFF21 => self.ch4.write_nr42(val),
            0xFF22 => self.ch4.write_nr43(val),
            0xFF23 => self.ch4.write_nr44(val),

            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
            0xFF26 => {
                let new_power = (val & 0x80) != 0;
                if !new_power && self.power_on {
                    // Turn APU off: clear all registers (except Wave RAM)
                    self.ch1 = Channel1::new();
                    self.ch2 = Channel2::new();
                    let wave_ram = self.ch3.wave_ram;
                    self.ch3 = Channel3::new();
                    self.ch3.wave_ram = wave_ram;
                    self.ch4 = Channel4::new();
                    self.nr50 = 0;
                    self.nr51 = 0;
                    self.power_on = false;
                } else if new_power && !self.power_on {
                    // Turn APU on
                    self.power_on = true;
                    self.frame_sequencer_step = 0;
                    self.frame_sequencer_timer = FRAME_SEQUENCER_PERIOD;
                }
            }

            0xFF30..=0xFF3F => {
                let idx = (addr - 0xFF30) as usize;
                self.ch3.wave_ram[idx] = val;
            }

            _ => {}
        }
    }

    /// Advance APU state by the specified number of 4.194304 MHz APU cycles.
    pub fn step(&mut self, cycles: u8) {
        let c = cycles as u32;

        if self.power_on {
            // Step Channel Timers
            self.ch1.step(c);
            self.ch2.step(c);
            self.ch3.step(c);
            self.ch4.step(c);

            // Step Frame Sequencer (512 Hz)
            if self.frame_sequencer_timer <= c {
                let overflow = c - self.frame_sequencer_timer;
                self.frame_sequencer_timer = FRAME_SEQUENCER_PERIOD;
                if self.frame_sequencer_timer > overflow {
                    self.frame_sequencer_timer -= overflow;
                }
                self.clock_frame_sequencer();
            } else {
                self.frame_sequencer_timer -= c;
            }
        }

        // Clock Audio Sampling
        let sample_step = CPU_CLOCK_HZ / self.sample_rate;
        self.sample_timer += c as f64;
        while self.sample_timer >= sample_step {
            self.sample_timer -= sample_step;
            let (left, right) = self.mix();

            self.sample_buffer.push(left);
            self.sample_buffer.push(right);
        }
    }

    fn clock_frame_sequencer(&mut self) {
        match self.frame_sequencer_step {
            0 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
            }
            2 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
                self.ch1.clock_sweep();
            }
            4 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
            }
            6 => {
                self.ch1.clock_length();
                self.ch2.clock_length();
                self.ch3.clock_length();
                self.ch4.clock_length();
                self.ch1.clock_sweep();
            }
            7 => {
                self.ch1.clock_envelope();
                self.ch2.clock_envelope();
                self.ch4.clock_envelope();
            }
            _ => {}
        }
        self.frame_sequencer_step = (self.frame_sequencer_step + 1) % 8;
    }

    /// Mix active sound channels according to NR50 (Volume) and NR51 (Panning).
    pub fn mix(&self) -> (f32, f32) {
        if !self.power_on {
            return (0.0, 0.0);
        }

        let s1 = self.ch1.sample();
        let s2 = self.ch2.sample();
        let s3 = self.ch3.sample();
        let s4 = self.ch4.sample();

        let mut left = 0.0_f32;
        let mut right = 0.0_f32;

        // Panning (NR51)
        if (self.nr51 & 0x10) != 0 {
            left += s1;
        }
        if (self.nr51 & 0x01) != 0 {
            right += s1;
        }

        if (self.nr51 & 0x20) != 0 {
            left += s2;
        }
        if (self.nr51 & 0x02) != 0 {
            right += s2;
        }

        if (self.nr51 & 0x40) != 0 {
            left += s3;
        }
        if (self.nr51 & 0x04) != 0 {
            right += s3;
        }

        if (self.nr51 & 0x80) != 0 {
            left += s4;
        }
        if (self.nr51 & 0x08) != 0 {
            right += s4;
        }

        // Master Volume (NR50: bits 4..6 Left, bits 0..2 Right)
        let vol_l = (((self.nr50 >> 4) & 0x07) + 1) as f32 / 8.0;
        let vol_r = ((self.nr50 & 0x07) + 1) as f32 / 8.0;

        // Normalize across 4 channels with master headroom scale
        let out_l = (left / 4.0) * vol_l * 0.60;
        let out_r = (right / 4.0) * vol_r * 0.60;

        (out_l, out_r)
    }

    /// Drain queued audio samples (for EmulatorCore trait).
    pub fn drain_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.sample_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apu_register_rw() {
        let mut apu = Apu::new();

        // Ch1 Duty & Length
        apu.write_register(0xFF11, 0x80);
        assert_eq!(apu.read_register(0xFF11), 0xBF);

        // Ch1 Envelope
        apu.write_register(0xFF12, 0xF3);
        assert_eq!(apu.read_register(0xFF12), 0xF3);

        // Master Volume & Panning
        apu.write_register(0xFF24, 0x77);
        assert_eq!(apu.read_register(0xFF24), 0x77);
        apu.write_register(0xFF25, 0xF0);
        assert_eq!(apu.read_register(0xFF25), 0xF0);
    }

    #[test]
    fn test_apu_power_off_clears_registers() {
        let mut apu = Apu::new();
        apu.write_register(0xFF12, 0xF3);
        apu.write_register(0xFF24, 0x77);

        // Turn power off (bit 7 = 0)
        apu.write_register(0xFF26, 0x00);
        assert_eq!(apu.read_register(0xFF26), 0x70);
        assert_eq!(apu.read_register(0xFF12), 0xFF);
        assert_eq!(apu.read_register(0xFF24), 0xFF);
    }

    #[test]
    fn test_wave_ram_rw() {
        let mut apu = Apu::new();
        for i in 0..16 {
            apu.write_register(0xFF30 + i, (i * 0x11) as u8);
        }
        for i in 0..16 {
            assert_eq!(apu.read_register(0xFF30 + i), (i * 0x11) as u8);
        }
    }

    #[test]
    fn test_apu_step_and_sample_generation() {
        let mut apu = Apu::new();
        // Enable Ch1 with volume
        apu.write_register(0xFF11, 0x80); // 50% duty
        apu.write_register(0xFF12, 0xF0); // max volume
        apu.write_register(0xFF13, 0x00); // freq low
        apu.write_register(0xFF14, 0x87); // trigger + freq high

        // Step 10000 cycles (~2.3ms)
        for _ in 0..1000 {
            apu.step(10);
        }

        let samples = apu.drain_audio();
        assert!(!samples.is_empty(), "APU should generate audio samples");
    }
}

/// Memory Bank Controller (MBC) interface and implementation for Game Boy cartridges.
#[derive(Debug, Clone)]
pub enum Mbc {
    RomOnly {
        rom: Vec<u8>,
    },
    Mbc1 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
        ram_enabled: bool,
        mode: bool, // false = ROM mode, true = RAM mode
    },
    Mbc3 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
        ram_enabled: bool,
        rtc_registers: [u8; 5],
    },
    Mbc5 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
        ram_enabled: bool,
    },
}

impl Mbc {
    /// Instantiates appropriate MBC handler based on cartridge header inspection.
    pub fn from_bytes(rom_bytes: &[u8]) -> Self {
        if rom_bytes.len() < 0x0150 {
            return Mbc::RomOnly {
                rom: rom_bytes.to_vec(),
            };
        }

        let cart_type = rom_bytes[0x0147];
        let ram_size_code = rom_bytes[0x0149];

        let ram_size = match ram_size_code {
            0x02 => 8 * 1024,
            0x03 => 32 * 1024,
            0x04 => 128 * 1024,
            0x05 => 64 * 1024,
            _ => 32 * 1024,
        };

        match cart_type {
            0x01..=0x03 => Mbc::Mbc1 {
                rom: rom_bytes.to_vec(),
                ram: vec![0; ram_size],
                rom_bank: 1,
                ram_bank: 0,
                ram_enabled: false,
                mode: false,
            },
            0x0F..=0x13 => Mbc::Mbc3 {
                rom: rom_bytes.to_vec(),
                ram: vec![0; ram_size],
                rom_bank: 1,
                ram_bank: 0,
                ram_enabled: false,
                rtc_registers: [0; 5],
            },
            0x19..=0x1E => Mbc::Mbc5 {
                rom: rom_bytes.to_vec(),
                ram: vec![0; ram_size],
                rom_bank: 1,
                ram_bank: 0,
                ram_enabled: false,
            },
            _ => Mbc::RomOnly {
                rom: rom_bytes.to_vec(),
            },
        }
    }

    /// Reads a byte from ROM space (0x0000 - 0x7FFF).
    pub fn read_rom(&self, addr: u16) -> u8 {
        match self {
            Mbc::RomOnly { rom } => {
                let idx = addr as usize;
                if idx < rom.len() {
                    rom[idx]
                } else {
                    0xFF
                }
            }

            Mbc::Mbc1 {
                rom,
                rom_bank,
                mode,
                ..
            } => {
                let idx = if addr < 0x4000 {
                    let bank = if *mode { (*rom_bank) & 0x60 } else { 0 };
                    (bank * 0x4000) + (addr as usize)
                } else {
                    let bank = if *rom_bank == 0 { 1 } else { *rom_bank };
                    (bank * 0x4000) + ((addr - 0x4000) as usize)
                };
                if idx < rom.len() {
                    rom[idx]
                } else {
                    0xFF
                }
            }

            Mbc::Mbc3 { rom, rom_bank, .. } => {
                let idx = if addr < 0x4000 {
                    addr as usize
                } else {
                    let bank = if *rom_bank == 0 { 1 } else { *rom_bank };
                    (bank * 0x4000) + ((addr - 0x4000) as usize)
                };
                if idx < rom.len() {
                    rom[idx]
                } else {
                    0xFF
                }
            }

            Mbc::Mbc5 { rom, rom_bank, .. } => {
                let idx = if addr < 0x4000 {
                    addr as usize
                } else {
                    (rom_bank * 0x4000) + ((addr - 0x4000) as usize)
                };
                if idx < rom.len() {
                    rom[idx]
                } else {
                    0xFF
                }
            }
        }
    }

    /// Writes a control byte to ROM space (0x0000 - 0x7FFF) for bank switching.
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match self {
            Mbc::RomOnly { .. } => {}

            Mbc::Mbc1 {
                rom_bank,
                ram_bank,
                ram_enabled,
                mode,
                ..
            } => match addr {
                0x0000..=0x1FFF => *ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    let bank = (val & 0x1F) as usize;
                    let bank = if bank == 0 { 1 } else { bank };
                    *rom_bank = ((*rom_bank) & 0x60) | bank;
                }
                0x4000..=0x5FFF => {
                    let bits = (val & 0x03) as usize;
                    if *mode {
                        *ram_bank = bits;
                    } else {
                        *rom_bank = ((*rom_bank) & 0x1F) | (bits << 5);
                    }
                }
                0x6000..=0x7FFF => *mode = (val & 0x01) != 0,
                _ => {}
            },

            Mbc::Mbc3 {
                rom_bank,
                ram_bank,
                ram_enabled,
                ..
            } => match addr {
                0x0000..=0x1FFF => *ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    let bank = (val & 0x7F) as usize;
                    *rom_bank = if bank == 0 { 1 } else { bank };
                }
                0x4000..=0x5FFF => *ram_bank = (val & 0x0F) as usize,
                _ => {}
            },

            Mbc::Mbc5 {
                rom_bank,
                ram_bank,
                ram_enabled,
                ..
            } => match addr {
                0x0000..=0x1FFF => *ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x2FFF => *rom_bank = ((*rom_bank) & 0x0100) | (val as usize),
                0x3000..=0x3FFF => *rom_bank = ((*rom_bank) & 0x00FF) | (((val & 0x01) as usize) << 8),
                0x4000..=0x5FFF => *ram_bank = (val & 0x0F) as usize,
                _ => {}
            },
        }
    }

    /// Reads a byte from Cartridge RAM space (0xA000 - 0xBFFF).
    pub fn read_ram(&self, addr: u16) -> u8 {
        match self {
            Mbc::RomOnly { .. } => 0xFF,

            Mbc::Mbc1 {
                ram,
                ram_bank,
                ram_enabled,
                mode,
                ..
            } => {
                if !*ram_enabled || ram.is_empty() {
                    return 0xFF;
                }
                let bank = if *mode { *ram_bank } else { 0 };
                let idx = (bank * 0x2000) + ((addr - 0xA000) as usize);
                if idx < ram.len() {
                    ram[idx]
                } else {
                    0xFF
                }
            }

            Mbc::Mbc3 {
                ram,
                ram_bank,
                ram_enabled,
                rtc_registers,
                ..
            } => {
                if !*ram_enabled {
                    return 0xFF;
                }
                if *ram_bank >= 0x08 && *ram_bank <= 0x0C {
                    return rtc_registers[*ram_bank - 0x08];
                }
                let idx = (*ram_bank * 0x2000) + ((addr - 0xA000) as usize);
                if idx < ram.len() {
                    ram[idx]
                } else {
                    0xFF
                }
            }

            Mbc::Mbc5 {
                ram,
                ram_bank,
                ram_enabled,
                ..
            } => {
                if !*ram_enabled || ram.is_empty() {
                    return 0xFF;
                }
                let idx = (*ram_bank * 0x2000) + ((addr - 0xA000) as usize);
                if idx < ram.len() {
                    ram[idx]
                } else {
                    0xFF
                }
            }
        }
    }

    /// Writes a byte to Cartridge RAM space (0xA000 - 0xBFFF).
    pub fn write_ram(&mut self, addr: u16, val: u8) {
        match self {
            Mbc::RomOnly { .. } => {}

            Mbc::Mbc1 {
                ram,
                ram_bank,
                ram_enabled,
                mode,
                ..
            } => {
                if !*ram_enabled || ram.is_empty() {
                    return;
                }
                let bank = if *mode { *ram_bank } else { 0 };
                let idx = (bank * 0x2000) + ((addr - 0xA000) as usize);
                if idx < ram.len() {
                    ram[idx] = val;
                }
            }

            Mbc::Mbc3 {
                ram,
                ram_bank,
                ram_enabled,
                rtc_registers,
                ..
            } => {
                if !*ram_enabled {
                    return;
                }
                if *ram_bank >= 0x08 && *ram_bank <= 0x0C {
                    rtc_registers[*ram_bank - 0x08] = val;
                    return;
                }
                let idx = (*ram_bank * 0x2000) + ((addr - 0xA000) as usize);
                if idx < ram.len() {
                    ram[idx] = val;
                }
            }

            Mbc::Mbc5 {
                ram,
                ram_bank,
                ram_enabled,
                ..
            } => {
                if !*ram_enabled || ram.is_empty() {
                    return;
                }
                let idx = (*ram_bank * 0x2000) + ((addr - 0xA000) as usize);
                if idx < ram.len() {
                    ram[idx] = val;
                }
            }
        }
    }
}

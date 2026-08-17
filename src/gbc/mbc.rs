/// Memory Bank Controller (MBC) interface and implementation for Game Boy cartridges.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        has_battery: bool,
    },
    Mbc2 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_enabled: bool,
        has_battery: bool,
    },
    Mbc3 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
        ram_enabled: bool,
        rtc_registers: [u8; 5],
        has_battery: bool,
    },
    Mbc5 {
        rom: Vec<u8>,
        ram: Vec<u8>,
        rom_bank: usize,
        ram_bank: usize,
        ram_enabled: bool,
        has_battery: bool,
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
                has_battery: cart_type == 0x03,
            },
            0x05 | 0x06 => Mbc::Mbc2 {
                rom: rom_bytes.to_vec(),
                ram: vec![0; 512],
                rom_bank: 1,
                ram_enabled: false,
                has_battery: cart_type == 0x06,
            },
            0x0F..=0x13 => Mbc::Mbc3 {
                rom: rom_bytes.to_vec(),
                ram: vec![0; ram_size],
                rom_bank: 1,
                ram_bank: 0,
                ram_enabled: false,
                rtc_registers: [0; 5],
                has_battery: matches!(cart_type, 0x0F | 0x10 | 0x13),
            },
            0x19..=0x1E => Mbc::Mbc5 {
                rom: rom_bytes.to_vec(),
                ram: vec![0; ram_size],
                rom_bank: 1,
                ram_bank: 0,
                ram_enabled: false,
                has_battery: matches!(cart_type, 0x1B | 0x1E),
            },
            _ => Mbc::RomOnly {
                rom: rom_bytes.to_vec(),
            },
        }
    }

    /// Checks if cartridge has battery-backed persistent RAM.
    #[allow(dead_code)]
    pub fn has_battery(&self) -> bool {
        match self {
            Mbc::RomOnly { .. } => false,
            Mbc::Mbc1 { has_battery, .. } => *has_battery,
            Mbc::Mbc2 { has_battery, .. } => *has_battery,
            Mbc::Mbc3 { has_battery, .. } => *has_battery,
            Mbc::Mbc5 { has_battery, .. } => *has_battery,
        }
    }

    /// Extract a slice of cartridge battery RAM for saving to disk.
    pub fn get_ram(&self) -> Option<&[u8]> {
        match self {
            Mbc::RomOnly { .. } => None,
            Mbc::Mbc1 { ram, .. } => Some(ram.as_slice()),
            Mbc::Mbc2 { ram, .. } => Some(ram.as_slice()),
            Mbc::Mbc3 { ram, .. } => Some(ram.as_slice()),
            Mbc::Mbc5 { ram, .. } => Some(ram.as_slice()),
        }
    }

    /// Ingest persistent save data into cartridge RAM.
    pub fn load_ram(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        match self {
            Mbc::RomOnly { .. } => {}
            Mbc::Mbc1 { ram, .. } => {
                let copy_len = ram.len().min(data.len());
                ram[..copy_len].copy_from_slice(&data[..copy_len]);
            }
            Mbc::Mbc2 { ram, .. } => {
                let copy_len = ram.len().min(data.len());
                ram[..copy_len].copy_from_slice(&data[..copy_len]);
            }
            Mbc::Mbc3 { ram, .. } => {
                let copy_len = ram.len().min(data.len());
                ram[..copy_len].copy_from_slice(&data[..copy_len]);
            }
            Mbc::Mbc5 { ram, .. } => {
                let copy_len = ram.len().min(data.len());
                ram[..copy_len].copy_from_slice(&data[..copy_len]);
            }
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

            Mbc::Mbc2 { rom, rom_bank, .. } => {
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

            Mbc::Mbc2 {
                rom_bank,
                ram_enabled,
                ..
            } => {
                if (0x0000..=0x3FFF).contains(&addr) {
                    if (addr & 0x0100) == 0 {
                        // Bit 8 is 0: RAM Enable
                        *ram_enabled = (val & 0x0F) == 0x0A;
                    } else {
                        // Bit 8 is 1: ROM Bank (lower 4 bits)
                        let bank = (val & 0x0F) as usize;
                        *rom_bank = if bank == 0 { 1 } else { bank };
                    }
                }
            }

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

            Mbc::Mbc2 {
                ram, ram_enabled, ..
            } => {
                if !*ram_enabled {
                    return 0xFF;
                }
                let idx = (addr & 0x01FF) as usize;
                ram[idx] | 0xF0 // Upper 4 bits always read 1s
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

            Mbc::Mbc2 {
                ram, ram_enabled, ..
            } => {
                if !*ram_enabled {
                    return;
                }
                let idx = (addr & 0x01FF) as usize;
                ram[idx] = val & 0x0F; // Only lower 4 bits are stored
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbc_battery_and_save_persistence() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x13; // MBC3 + RAM + BATTERY
        rom[0x0149] = 0x03; // 32KB RAM

        let mut mbc = Mbc::from_bytes(&rom);
        assert!(mbc.has_battery());

        let save_bytes = vec![0xAB; 32768];
        mbc.load_ram(&save_bytes);

        let ram = mbc.get_ram().expect("RAM should exist");
        assert_eq!(ram.len(), 32768);
        assert_eq!(ram[0], 0xAB);
    }
}

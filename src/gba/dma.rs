#![allow(dead_code)]

/// DMA Channel state for Channels 0-3
#[derive(Debug, Clone, Default)]
pub struct DmaChannel {
    pub sad: u32,
    pub dad: u32,
    pub cnt_l: u16,
    pub cnt_h: u16,

    pub internal_sad: u32,
    pub internal_dad: u32,
}

/// GBA Direct Memory Access (DMA) Controller for Channels 0-3
pub struct GbaDma {
    pub channels: [DmaChannel; 4],
}

impl Default for GbaDma {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaDma {
    pub fn new() -> Self {
        Self {
            channels: [
                DmaChannel::default(),
                DmaChannel::default(),
                DmaChannel::default(),
                DmaChannel::default(),
            ],
        }
    }

    /// Reset DMA controller channels
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read byte from DMA I/O space (0x040000B0 - 0x040000DF)
    pub fn read_u8(&self, addr: u32) -> u8 {
        let ch = match addr {
            0x040000B0..=0x040000BB => 0,
            0x040000BC..=0x040000C7 => 1,
            0x040000C8..=0x040000D3 => 2,
            0x040000D4..=0x040000DF => 3,
            _ => return 0,
        };

        let offset = (addr - (0x040000B0 + (ch as u32) * 12)) as usize;
        let dma = &self.channels[ch];

        match offset {
            0 => dma.sad as u8,
            1 => (dma.sad >> 8) as u8,
            2 => (dma.sad >> 16) as u8,
            3 => (dma.sad >> 24) as u8,

            4 => dma.dad as u8,
            5 => (dma.dad >> 8) as u8,
            6 => (dma.dad >> 16) as u8,
            7 => (dma.dad >> 24) as u8,

            8 => dma.cnt_l as u8,
            9 => (dma.cnt_l >> 8) as u8,

            10 => dma.cnt_h as u8,
            11 => (dma.cnt_h >> 8) as u8,

            _ => 0,
        }
    }

    /// Write byte to DMA I/O space (0x040000B0 - 0x040000DF).
    /// Returns `Some(ch)` if a channel becomes newly enabled and requests immediate execution.
    pub fn write_u8(&mut self, addr: u32, val: u8) -> Option<usize> {
        let ch = match addr {
            0x040000B0..=0x040000BB => 0,
            0x040000BC..=0x040000C7 => 1,
            0x040000C8..=0x040000D3 => 2,
            0x040000D4..=0x040000DF => 3,
            _ => return None,
        };

        let offset = (addr - (0x040000B0 + (ch as u32) * 12)) as usize;
        let dma = &mut self.channels[ch];

        match offset {
            0 => dma.sad = (dma.sad & 0xFFFFFF00) | val as u32,
            1 => dma.sad = (dma.sad & 0xFFFF00FF) | ((val as u32) << 8),
            2 => dma.sad = (dma.sad & 0xFF00FFFF) | ((val as u32) << 16),
            3 => dma.sad = (dma.sad & 0x00FFFFFF) | ((val as u32) << 24),

            4 => dma.dad = (dma.dad & 0xFFFFFF00) | val as u32,
            5 => dma.dad = (dma.dad & 0xFFFF00FF) | ((val as u32) << 8),
            6 => dma.dad = (dma.dad & 0xFF00FFFF) | ((val as u32) << 16),
            7 => dma.dad = (dma.dad & 0x00FFFFFF) | ((val as u32) << 24),

            8 => dma.cnt_l = (dma.cnt_l & 0xFF00) | val as u16,
            9 => dma.cnt_l = (dma.cnt_l & 0x00FF) | ((val as u16) << 8),

            10 => dma.cnt_h = (dma.cnt_h & 0xFF00) | val as u16,
            11 => {
                let old_cnt_h = dma.cnt_h;
                dma.cnt_h = (dma.cnt_h & 0x00FF) | ((val as u16) << 8);

                let newly_enabled = (old_cnt_h & (1 << 15)) == 0 && (dma.cnt_h & (1 << 15)) != 0;
                if newly_enabled {
                    dma.internal_sad = dma.sad;
                    dma.internal_dad = dma.dad;
                    return Some(ch);
                }
            }
            _ => {}
        }
        None
    }
}

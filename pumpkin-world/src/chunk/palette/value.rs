use std::hash::Hash;

pub trait PaletteValue: Default + Hash + Eq + Copy {
    const MAX_BITS: u8;
    const MAX_UNIQUE: usize;

    fn get_palette_config(unique_count: usize) -> PaletteConfig;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteConfig {
    Uniform,
    Indirect { bits_per_entry: u8 },
    Direct,
}

impl PaletteValue for u16 {
    const MAX_BITS: u8 = 16;
    const MAX_UNIQUE: usize = 256;

    fn get_palette_config(unique_count: usize) -> PaletteConfig {
        match unique_count {
            0 | 1 => PaletteConfig::Uniform,
            // Indirect: 4-8 bits (protocol specifies no 1-3 bit values for blocks)
            2..=16 => PaletteConfig::Indirect { bits_per_entry: 4 },
            17..=32 => PaletteConfig::Indirect { bits_per_entry: 5 },
            33..=64 => PaletteConfig::Indirect { bits_per_entry: 6 },
            65..=128 => PaletteConfig::Indirect { bits_per_entry: 7 },
            129..=Self::MAX_UNIQUE => PaletteConfig::Indirect { bits_per_entry: 8 },
            // Direct: use full 16 bits
            _ => PaletteConfig::Direct,
        }
    }
}

impl PaletteValue for u8 {
    const MAX_BITS: u8 = 8;
    const MAX_UNIQUE: usize = 8;

    fn get_palette_config(unique_count: usize) -> PaletteConfig {
        match unique_count {
            0 | 1 => PaletteConfig::Uniform,
            // Indirect: 1-3 bits for biomes
            2 => PaletteConfig::Indirect { bits_per_entry: 1 },
            3..=4 => PaletteConfig::Indirect { bits_per_entry: 2 },
            5..=Self::MAX_UNIQUE => PaletteConfig::Indirect { bits_per_entry: 3 },
            // Direct: use full 8 bits
            _ => PaletteConfig::Direct,
        }
    }
}

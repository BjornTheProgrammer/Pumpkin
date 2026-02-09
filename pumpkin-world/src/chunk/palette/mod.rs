pub mod value;
use std::hash::Hash;

use pumpkin_data::{Block, block_properties::is_air, chunk::Biome};
use pumpkin_util::math::vector3::Vector3;

use crate::{
    block::BlockStateCodec,
    chunk::{
        format::{ChunkSectionBiomes, ChunkSectionBlockStates, PaletteBiomeEntry},
        palette::value::{PaletteConfig, PaletteValue},
    },
};

#[derive(Clone)]
pub enum PalettedContainer<V: Hash + Eq + Copy, const SIZE: usize> {
    /// This makes the container all a uniform block, useful for air
    Uniform(V),
    /// This is for when the container has a number of unique blocks
    Indirect {
        palette: Box<[V]>,
        /// The wiki specification states that the data is stored in a padded 64-bit integer, with each block occupying a certain number of bits.
        data: Box<[u64]>,
        bit_width: u8,
        /// Reference counts for each palette entry
        ref_counts: Box<[u16]>,
    },
    /// This is for when the container has too many unique blocks to fit in a palette.
    Direct {
        /// This should be the global minecraft id
        data: Box<[V; SIZE]>,
    },
}

// The reason it is a u16 is that minecraft support a max of a u16 block count, the 4096 is the max number of blocks in a chunk section 16 * 16 * 16
pub type BlockSectionData = PalettedContainer<u16, 4096>;
// Same thing here but for biomes
pub type BiomeSectionData = PalettedContainer<u8, 64>;

impl<V: PaletteValue, const SIZE: usize> Default for PalettedContainer<V, SIZE> {
    fn default() -> Self {
        Self::Uniform(V::default())
    }
}

impl<V: PaletteValue, const SIZE: usize> PalettedContainer<V, SIZE> {
    pub const SIZE: usize = SIZE;

    #[inline]
    pub const fn dimension() -> usize {
        match SIZE {
            4096 => 16, // 16 * 16 * 16 for blocks
            64 => 4,    // 4 * 4 * 4 for biomes
            _ => {
                // This is a really dumb way to do it, but it works in const context.
                let mut dim = 1;
                while dim * dim * dim < SIZE {
                    dim += 1;
                }
                dim
            }
        }
    }

    #[inline]
    fn position_to_index(position: Vector3<u8>) -> usize {
        let x = position.x as usize;
        let y = position.y as usize;
        let z = position.z as usize;
        let dim = Self::dimension();

        (y * (dim * dim)) + (z * dim) + x
    }

    /// Gets the value at the given position
    pub fn get_value(&self, position: Vector3<u8>) -> V {
        match self {
            PalettedContainer::Uniform(value) => *value,
            PalettedContainer::Indirect {
                palette,
                data,
                bit_width,
                ..
            } => {
                let i = Self::position_to_index(position);
                let palette_index = Self::unpack_value(data, i, *bit_width);
                palette[palette_index as usize]
            }
            PalettedContainer::Direct { data, .. } => data[Self::position_to_index(position)],
        }
    }

    /// Sets the value at the given position
    pub fn set_value(&mut self, position: Vector3<u8>, value: V) -> V {
        match self {
            PalettedContainer::Uniform(uniform_value) => {
                let old_value = *uniform_value;

                if *uniform_value == value {
                    return old_value;
                }

                let bits_per_entry = match V::get_palette_config(2) {
                    PaletteConfig::Indirect { bits_per_entry } => bits_per_entry,
                    _ => unreachable!(),
                };
                let entries_per_long = 64 / bits_per_entry as usize;
                let num_longs = (SIZE + (entries_per_long - 1)) / entries_per_long;

                let palette = vec![*uniform_value, value].into_boxed_slice();
                let mut data = vec![0; num_longs].into_boxed_slice();

                let i = Self::position_to_index(position);
                Self::pack_value(&mut data, i, bits_per_entry, 1);

                let ref_counts = vec![SIZE as u16 - 1, 1].into_boxed_slice();

                *self = PalettedContainer::Indirect {
                    palette,
                    data,
                    bit_width: bits_per_entry,
                    ref_counts,
                };

                return old_value;
            }
            PalettedContainer::Indirect {
                palette,
                data,
                bit_width,
                ref_counts,
            } => {
                let current_position = Self::position_to_index(position);
                let current_palette_index =
                    Self::unpack_value(data, current_position, *bit_width) as usize;
                let old_value = palette[current_palette_index];

                for (palette_index, palette_item) in palette.iter().enumerate() {
                    if value == *palette_item {
                        if palette_index == current_palette_index {
                            return old_value;
                        }

                        ref_counts[current_palette_index] -= 1;
                        ref_counts[palette_index] += 1;

                        Self::pack_value(data, current_position, *bit_width, palette_index as u64);

                        if ref_counts[current_palette_index] == 0 {
                            // We shrink the Indirect now that a value has been removed
                            let config = V::get_palette_config(palette.len() - 1);
                            match config {
                                PaletteConfig::Uniform => *self = PalettedContainer::Uniform(value),
                                PaletteConfig::Indirect { bits_per_entry } => {
                                    let mut new_ref_counts = ref_counts.to_vec();
                                    new_ref_counts.retain(|x| *x != 0);

                                    let deleted_value = palette[current_palette_index];
                                    let mut new_palette = palette.to_vec();
                                    new_palette.retain(|x| *x != deleted_value);

                                    let entries_per_long = 64 / bits_per_entry as usize;
                                    let num_longs =
                                        (Self::SIZE + (entries_per_long - 1)) / entries_per_long;
                                    let mut new_data = vec![0u64; num_longs].into_boxed_slice();
                                    for i in 0..Self::SIZE {
                                        let mut old_index = Self::unpack_value(data, i, *bit_width);
                                        if old_index as usize > current_palette_index {
                                            old_index -= 1;
                                        }
                                        Self::pack_value(
                                            &mut new_data,
                                            i,
                                            bits_per_entry,
                                            old_index,
                                        );
                                    }

                                    *self = PalettedContainer::Indirect {
                                        data: new_data,
                                        bit_width: bits_per_entry,
                                        ref_counts: new_ref_counts.into_boxed_slice(),
                                        palette: new_palette.into_boxed_slice(),
                                    };
                                }
                                PaletteConfig::Direct => unreachable!(),
                            }
                        }

                        return old_value;
                    }
                }

                let config = V::get_palette_config(palette.len());

                match config {
                    PaletteConfig::Uniform => *self = PalettedContainer::Uniform(value),
                    PaletteConfig::Indirect { bits_per_entry } => {
                        ref_counts[current_palette_index] -= 1;

                        if ref_counts[current_palette_index] == 0 {
                            // We do not need to allocate a new palette, we can just restructure the existing one
                            palette[current_palette_index] = value;
                            ref_counts[current_palette_index] += 1;
                            Self::pack_value(
                                data,
                                current_position,
                                *bit_width,
                                current_palette_index as u64,
                            );
                        } else if bits_per_entry == *bit_width {
                            // Bit width stays the same, no need to reallocate data array
                            let mut new_palette = palette.to_vec();
                            new_palette.push(value);
                            let new_palette = new_palette.into_boxed_slice();

                            let mut new_ref_counts = ref_counts.to_vec();
                            new_ref_counts.push(1);
                            let new_ref_counts = new_ref_counts.into_boxed_slice();

                            Self::pack_value(
                                data,
                                current_position,
                                *bit_width,
                                (new_palette.len() - 1) as u64,
                            );

                            *self = PalettedContainer::Indirect {
                                data: std::mem::take(data),
                                bit_width: *bit_width,
                                palette: new_palette,
                                ref_counts: new_ref_counts,
                            };
                        } else {
                            let entries_per_long = 64 / bits_per_entry as usize;
                            let num_longs =
                                (Self::SIZE + (entries_per_long - 1)) / entries_per_long;
                            let mut new_data = vec![0u64; num_longs].into_boxed_slice();
                            for i in 0..Self::SIZE {
                                let value = Self::unpack_value(data, i, *bit_width);
                                Self::pack_value(&mut new_data, i, bits_per_entry, value);
                            }

                            // Doing this is fine since we always have at least one new entry.
                            let mut new_palette = palette.to_vec();
                            new_palette.push(value);
                            let new_palette = new_palette.into_boxed_slice();

                            let mut new_ref_counts = ref_counts.to_vec();
                            new_ref_counts.push(1);
                            let new_ref_counts = new_ref_counts.into_boxed_slice();

                            Self::pack_value(
                                &mut new_data,
                                current_position,
                                bits_per_entry,
                                (new_palette.len() - 1) as u64,
                            );

                            *self = PalettedContainer::Indirect {
                                data: new_data,
                                bit_width: bits_per_entry,
                                palette: new_palette,
                                ref_counts: new_ref_counts,
                            };
                        }
                    }
                    PaletteConfig::Direct => {
                        let mut direct_data = Box::new(std::array::from_fn(|i| {
                            let palette_index = Self::unpack_value(&data, i, *bit_width);
                            palette[palette_index as usize]
                        }));
                        direct_data[current_position] = value;
                        *self = PalettedContainer::Direct { data: direct_data };
                    }
                }

                old_value
            }
            PalettedContainer::Direct { data } => {
                let current_position = Self::position_to_index(position);
                let old_value = data[current_position];
                data[current_position] = value;

                let mut unique_values: std::collections::HashSet<V> =
                    std::collections::HashSet::new();
                for v in data.iter() {
                    unique_values.insert(*v);
                    // Early exit if too many unique values for indirect
                    if unique_values.len() > V::MAX_UNIQUE {
                        return old_value;
                    }
                }

                let config = V::get_palette_config(unique_values.len());
                match config {
                    PaletteConfig::Indirect { bits_per_entry } => {
                        let palette: Vec<V> = unique_values.into_iter().collect();
                        let palette_map: std::collections::HashMap<V, usize> =
                            palette.iter().enumerate().map(|(i, &v)| (v, i)).collect();

                        let entries_per_long = 64 / bits_per_entry as usize;
                        let num_longs = (Self::SIZE + (entries_per_long - 1)) / entries_per_long;
                        let mut indirect_data = vec![0u64; num_longs].into_boxed_slice();

                        let mut ref_counts = vec![0u16; palette.len()];
                        for i in 0..Self::SIZE {
                            let palette_index = palette_map[&data[i]];
                            ref_counts[palette_index] += 1;
                            Self::pack_value(
                                &mut indirect_data,
                                i,
                                bits_per_entry,
                                palette_index as u64,
                            );
                        }

                        *self = PalettedContainer::Indirect {
                            palette: palette.into_boxed_slice(),
                            data: indirect_data,
                            bit_width: bits_per_entry,
                            ref_counts: ref_counts.into_boxed_slice(),
                        };

                        old_value
                    }
                    PaletteConfig::Direct => old_value,
                    _ => unreachable!(),
                }
            }
        }
    }

    #[inline]
    fn unpack_value(buffer: &[u64], entry_index: usize, bit_width: u8) -> u64 {
        let entries_per_long = 64 / bit_width as usize;
        let long_index = entry_index / entries_per_long;
        let bit_index = (entry_index % entries_per_long) * bit_width as usize;
        let entry_mask = (1u64 << bit_width) - 1;

        (buffer[long_index] >> bit_index) & entry_mask
    }

    #[inline]
    fn pack_value(buffer: &mut [u64], entry_index: usize, bit_width: u8, value: u64) {
        let bits_per_entry = bit_width as usize;

        let entries_per_long = 64 / bits_per_entry;
        let long_index = entry_index / entries_per_long;
        let bit_index = (entry_index % entries_per_long) * bits_per_entry;
        let entry_mask = (1u64 << bits_per_entry) - 1;
        buffer[long_index] &= !(entry_mask << bit_index);

        buffer[long_index] |= (value & entry_mask) << bit_index;
    }

    #[must_use]
    pub fn from_palette_and_packed_data(palette: Vec<V>, packed_data: &[i64]) -> Self {
        if palette.is_empty() {
            log::warn!("Empty palette data! Defaulting...");
            return Self::default();
        }

        if palette.len() == 1 {
            return Self::Uniform(palette[0]);
        }

        let config = V::get_palette_config(palette.len());
        match config {
            PaletteConfig::Uniform => unreachable!(),
            PaletteConfig::Indirect { bits_per_entry } => {
                let data: Box<[u64]> = packed_data.iter().map(|&x| x as u64).collect();

                let mut ref_counts = vec![0u16; palette.len()];
                for i in 0..SIZE {
                    let palette_index = Self::unpack_value(&data, i, bits_per_entry) as usize;
                    if palette_index < palette.len() {
                        ref_counts[palette_index] += 1;
                    } else {
                        log::warn!(
                            "Invalid palette index {} for palette of size {} at position {}",
                            palette_index,
                            palette.len(),
                            i
                        );
                    }
                }

                Self::Indirect {
                    palette: palette.into_boxed_slice(),
                    data,
                    bit_width: bits_per_entry,
                    ref_counts: ref_counts.into_boxed_slice(),
                }
            }
            PaletteConfig::Direct => {
                let bit_width = (usize::BITS - (palette.len() - 1).leading_zeros()) as u8;

                let data: Box<[V; SIZE]> = Box::new(std::array::from_fn(|i| {
                    let palette_index = Self::unpack_value_raw(packed_data, i, bit_width) as usize;
                    if palette_index < palette.len() {
                        palette[palette_index]
                    } else {
                        V::default()
                    }
                }));

                Self::Direct { data }
            }
        }
    }

    /// Helper function to unpack a value from i64 array (used for disk format)
    #[inline]
    fn unpack_value_raw(buffer: &[i64], entry_index: usize, bit_width: u8) -> u64 {
        let entries_per_long = 64 / bit_width as usize;
        let long_index = entry_index / entries_per_long;
        let bit_index = (entry_index % entries_per_long) * bit_width as usize;
        let entry_mask = (1u64 << bit_width) - 1;

        if long_index < buffer.len() {
            ((buffer[long_index] as u64) >> bit_index) & entry_mask
        } else {
            0
        }
    }

    fn to_palette_and_packed_data_disk(&self) -> (Vec<V>, Option<Box<[i64]>>) {
        match self {
            PalettedContainer::Uniform(value) => {
                // Return single-entry palette with no data
                (vec![*value], None)
            }
            PalettedContainer::Indirect { palette, data, .. } => {
                // Convert u64 data to i64 for NBT
                let packed_data: Box<[i64]> = data.iter().map(|&x| x as i64).collect();
                (palette.to_vec(), Some(packed_data))
            }
            PalettedContainer::Direct { data } => {
                // Build palette from unique values
                let mut palette_vec = Vec::new();
                let mut value_to_index = std::collections::HashMap::new();

                for &value in data.iter() {
                    if !value_to_index.contains_key(&value) {
                        value_to_index.insert(value, palette_vec.len());
                        palette_vec.push(value);
                    }
                }

                // Calculate bits per entry
                let bits_per_entry = if palette_vec.len() <= 1 {
                    0
                } else {
                    (usize::BITS - (palette_vec.len() - 1).leading_zeros()) as u8
                };

                if bits_per_entry == 0 {
                    // All same value
                    return (palette_vec, None);
                }

                // Pack the data
                let entries_per_long = 64 / bits_per_entry as usize;
                let num_longs = (Self::SIZE + (entries_per_long - 1)) / entries_per_long;
                let mut packed_data = vec![0i64; num_longs];

                for (i, &value) in data.iter().enumerate() {
                    let palette_index = value_to_index[&value];
                    let long_index = i / entries_per_long;
                    let bit_index = (i % entries_per_long) * bits_per_entry as usize;
                    let entry_mask = (1u64 << bits_per_entry) - 1;

                    packed_data[long_index] |=
                        ((palette_index as u64 & entry_mask) << bit_index) as i64;
                }

                (palette_vec, Some(packed_data.into_boxed_slice()))
            }
        }
    }
}

impl BlockSectionData {
    pub fn from_disk_nbt(nbt: ChunkSectionBlockStates) -> Self {
        use crate::block::BlockStateCodec;

        // Convert palette entries to state IDs
        let palette = nbt
            .palette
            .into_iter()
            .map(|entry: BlockStateCodec| entry.get_state_id())
            .collect::<Vec<_>>();

        // Use the helper method with disk minimum of 4 bits per entry
        Self::from_palette_and_packed_data(palette, nbt.data.as_ref().unwrap_or(&Box::default()))
    }

    #[must_use]
    pub fn to_disk_nbt(&self) -> ChunkSectionBlockStates {
        let (palette, packed_data) = self.to_palette_and_packed_data_disk();

        ChunkSectionBlockStates {
            data: packed_data,
            palette: palette
                .into_iter()
                .map(|state_id| {
                    let block = Block::from_state_id(state_id);
                    BlockStateCodec {
                        name: block,
                        properties: block.properties(state_id).map(|p| {
                            p.to_props()
                                .into_iter()
                                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                                .collect()
                        }),
                    }
                })
                .collect(),
        }
    }

    pub fn count_non_air(&self) -> u16 {
        match self {
            PalettedContainer::Uniform(value) => {
                if is_air(*value) {
                    0
                } else {
                    Self::SIZE as u16
                }
            }
            PalettedContainer::Indirect {
                ref_counts,
                palette,
                ..
            } => palette
                .iter()
                .zip(ref_counts.iter())
                .filter(|(block, _)| !is_air(**block))
                .map(|(_, count)| *count)
                .sum(),
            PalettedContainer::Direct { data } => {
                data.iter().filter(|block| !is_air(**block)).count() as u16
            }
        }
    }
}

impl BiomeSectionData {
    pub fn from_disk_nbt(nbt: ChunkSectionBiomes) -> Self {
        let palette = nbt
            .palette
            .into_iter()
            .map(|entry| Biome::from_name(&entry.name).unwrap_or(&Biome::PLAINS).id)
            .collect::<Vec<_>>();

        Self::from_palette_and_packed_data(palette, nbt.data.as_ref().unwrap_or(&Box::default()))
    }

    #[must_use]
    pub fn to_disk_nbt(&self) -> ChunkSectionBiomes {
        let (palette, packed_data) = self.to_palette_and_packed_data_disk();

        ChunkSectionBiomes {
            data: packed_data,
            palette: palette
                .iter()
                .map(|&biome_id| PaletteBiomeEntry {
                    name: Biome::from_id(biome_id).unwrap().registry_id.into(),
                })
                .collect(),
        }
    }
}

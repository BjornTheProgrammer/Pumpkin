use pumpkin_nbt::nbt_long_array;
use pumpkin_util::math::position::BlockPos;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::ops::{BitAnd, BitOr};

#[derive(Debug, Clone, Copy)]
pub enum ChunkHeightmapType {
    WorldSurface = 0,
    MotionBlocking = 1,
    MotionBlockingNoLeaves = 2,
}
impl TryFrom<usize> for ChunkHeightmapType {
    type Error = &'static str;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::WorldSurface),
            1 => Ok(Self::MotionBlocking),
            2 => Ok(Self::MotionBlockingNoLeaves),
            _ => Err("Invalid usize value for ChunkHeightmapType. The value should be 0~2."),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub struct ChunkHeightmaps {
    #[serde(serialize_with = "nbt_long_array")]
    pub world_surface: Box<[i64]>,
    #[serde(serialize_with = "nbt_long_array")]
    pub motion_blocking: Box<[i64]>,
    #[serde(serialize_with = "nbt_long_array")]
    pub motion_blocking_no_leaves: Box<[i64]>,
}

impl ChunkHeightmaps {
    pub fn set(&mut self, heightmap: ChunkHeightmapType, pos: BlockPos, min_y: i32) {
        let data = match heightmap {
            ChunkHeightmapType::WorldSurface => &mut self.world_surface,
            ChunkHeightmapType::MotionBlocking => &mut self.motion_blocking,
            ChunkHeightmapType::MotionBlockingNoLeaves => &mut self.motion_blocking_no_leaves,
        };

        let local_x = (pos.0.x & 15) as usize;
        let local_z = (pos.0.z & 15) as usize;

        let adjust_height = (pos.0.y + min_y.abs()) as usize;

        assert!(adjust_height <= 2 << 9);

        //chunk column index in 16*16 chunk.
        let column_idx = local_z * 16 + local_x;

        // Each height value uses 9 bits, calculate starting bit position
        let bit_start_idx = column_idx * 9;

        // Find where these 9 bits start within a 64-bit packed array element
        // We use bit_start_index % 63, which means the last bit of i64 won't be used,
        // but this avoids the hassle of bit concatenation.
        let packed_array_bit_start_idx = bit_start_idx as u32 % 63;

        let mask = {
            if packed_array_bit_start_idx == 0 {
                //0b0000_0000_0111_1111_...
                !(0x1FF << (64 - 9))
            } else {
                !(0x1FF << (64 - packed_array_bit_start_idx - 9))
            }
        };

        let height_bit_bytes = adjust_height
            .wrapping_shl(64 - 9 - packed_array_bit_start_idx)
            .to_ne_bytes();
        let height = i64::from_ne_bytes(height_bit_bytes);

        let packed_array_idx = column_idx / 7;

        data[packed_array_idx] = data[packed_array_idx].bitand(mask).bitor(height);
    }

    #[must_use]
    pub fn get(&self, heightmap: ChunkHeightmapType, x: i32, z: i32, min_y: i32) -> i32 {
        let local_x = (x & 15) as usize;
        let local_z = (z & 15) as usize;

        let column_idx = local_z * 16 + local_x;
        let bit_start_idx = column_idx * 9;

        let packed_array_bit_start_idx = bit_start_idx as u32 % 63;

        let mask = {
            if packed_array_bit_start_idx == 0 {
                //0b1111_1111_1000_0000_...
                0x1ff << (64 - 9)
            } else {
                0x1ff << (64 - packed_array_bit_start_idx - 9)
            }
        };

        let packed_array_idx = column_idx / 7;

        let data = match heightmap {
            ChunkHeightmapType::WorldSurface => &self.world_surface,
            ChunkHeightmapType::MotionBlocking => &self.motion_blocking,
            ChunkHeightmapType::MotionBlockingNoLeaves => &self.motion_blocking_no_leaves,
        };
        let height_bit_bytes_i64 = data[packed_array_idx].bitand(mask).to_ne_bytes();

        (u64::from_ne_bytes(height_bit_bytes_i64)
            .wrapping_shr(64 - (packed_array_bit_start_idx + 9)) as i32)
            - min_y.abs()
    }

    pub fn log_heightmap(&self, _type: ChunkHeightmapType, min_y: i32) {
        let mut header = "Z/X".to_string();
        for x in 0..16 {
            let _ = write!(header, "{x:4}");
        }

        let grid: String = (0..16)
            .map(|z| {
                let mut row = format!("{z:3}");
                row.push_str(
                    &(0..16)
                        .map(|x| format!("{:4}", self.get(_type, x, z, min_y)))
                        .collect::<String>(),
                );
                row
            })
            .collect::<Vec<_>>()
            .join("\n");

        log::info!("\nHeightMap:\n{header}\n{grid}");
    }
}

/// The Heightmap for a completely empty chunk
impl Default for ChunkHeightmaps {
    fn default() -> Self {
        Self {
            // 9 bits per entry
            // 0 packed into an i64 7 times.
            motion_blocking: vec![0; 37].into_boxed_slice(),
            motion_blocking_no_leaves: vec![0; 37].into_boxed_slice(),
            world_surface: vec![0; 37].into_boxed_slice(),
        }
    }
}

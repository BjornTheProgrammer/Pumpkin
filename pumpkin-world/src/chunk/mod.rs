use std::sync::{Arc, atomic::AtomicBool};

use pumpkin_data::{Block, chunk::ChunkStatus, dimension::Dimension, fluid::Fluid};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{
    position::{BlockPos, ChunkPos},
    vector3::Vector3,
};
use rustc_hash::FxHashMap;
use tokio::sync::Mutex;

use crate::{
    block::entities::BlockEntity,
    chunk::{
        heightmap::ChunkHeightmaps,
        light::ChunkLight,
        palette::{BiomeSectionData, BlockSectionData},
        section::ChunkSection,
    },
    registry::BlockStateId,
    tick::scheduler::ChunkTickScheduler,
};

pub mod format;
pub mod heightmap;
pub mod io;
pub mod light;
pub mod palette;
pub mod section;

pub struct Chunk {
    pub sections: Box<[ChunkSection]>,
    pub heightmaps: std::sync::Mutex<ChunkHeightmaps>,
    pub light: ChunkLight,
    pub dirty: AtomicBool,
    pub position: ChunkPos,
    pub min_y: i32,
    pub block_entities: std::sync::Mutex<FxHashMap<BlockPos, Arc<dyn BlockEntity>>>,
    pub block_ticks: ChunkTickScheduler<&'static Block>,
    pub fluid_ticks: ChunkTickScheduler<&'static Fluid>,
    pub status: ChunkStatus,
}

pub struct ChunkEntityData {
    /// Chunk X
    pub x: i32,
    /// Chunk Z
    pub z: i32,
    pub data: Mutex<FxHashMap<uuid::Uuid, NbtCompound>>,

    pub dirty: AtomicBool,
}

impl Chunk {
    pub fn new_empty(dimension: &Dimension, position: ChunkPos, num_sections: usize) -> Self {
        let block_data = BlockSectionData::default();
        let biome_data = BiomeSectionData::default();
        Self {
            sections: vec![
                ChunkSection {
                    block_data,
                    biome_data,
                    block_count: 0,
                };
                num_sections
            ]
            .into_boxed_slice(),
            dirty: AtomicBool::new(false),
            heightmaps: std::sync::Mutex::new(ChunkHeightmaps::default()),
            position: position,
            min_y: dimension.min_y,
            light: ChunkLight::default(),
            block_entities: std::sync::Mutex::new(FxHashMap::default()),
            block_ticks: ChunkTickScheduler::default(),
            fluid_ticks: ChunkTickScheduler::default(),
            status: ChunkStatus::Empty,
        }
    }

    /// Gets a block from the chunk with a relative position
    pub fn get_block(&self, position: Vector3<u16>) -> BlockStateId {
        let dimensions = BlockSectionData::dimension() as u16;
        assert!(position.x <= dimensions && position.z <= dimensions);
        let section_y = position.y / dimensions;
        let local_pos = Vector3 {
            x: (position.x % dimensions) as u8,
            y: (position.y % dimensions) as u8,
            z: (position.z % dimensions) as u8,
        };

        self.sections[section_y as usize]
            .block_data
            .get_value(local_pos)
    }

    /// Sets the block at the given position to the new block.
    /// Returns the previous block state.
    pub fn set_block(&mut self, position: Vector3<u16>, new_block: BlockStateId) -> BlockStateId {
        let dimensions = BlockSectionData::dimension() as u16;
        assert!(position.x <= dimensions && position.z <= dimensions);
        let section_y = position.y / dimensions;
        let local_pos = Vector3 {
            x: (position.x % dimensions) as u8,
            y: (position.y % dimensions) as u8,
            z: (position.z % dimensions) as u8,
        };

        self.sections[section_y as usize]
            .block_data
            .set_value(local_pos, new_block)
    }
}

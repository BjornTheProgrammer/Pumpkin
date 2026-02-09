use pumpkin_data::block_properties::is_air;
use pumpkin_util::math::vector3::Vector3;

use crate::{
    chunk::palette::{BiomeSectionData, BlockSectionData},
    registry::BlockStateId,
};

#[derive(Clone)]
pub struct ChunkSection {
    pub block_data: BlockSectionData,
    pub biome_data: BiomeSectionData,

    /// This is the number of blocks in the section that are not air.
    /// It is important to keep track of this value because if the block count reaches zero,
    /// the client does not render the chunk.
    pub block_count: u16,
}

impl ChunkSection {
    pub fn get_block(&self, position: Vector3<u8>) -> BlockStateId {
        self.block_data.get_value(position)
    }

    pub fn set_block(&mut self, position: Vector3<u8>, new_block: BlockStateId) {
        let old_block = self.block_data.set_value(position, new_block);

        let old_is_air = is_air(old_block);
        let new_is_air = is_air(new_block);

        match (old_is_air, new_is_air) {
            (true, false) => self.block_count += 1,
            (false, true) => self.block_count -= 1,
            _ => {}
        }
    }
}

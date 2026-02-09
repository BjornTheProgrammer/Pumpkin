use pumpkin_data::{Block, BlockState};
use pumpkin_util::math::position::BlockPos;
use std::pin::Pin;

pub type BlockStateId = u16;

pub trait BlockRegistryExt: Send + Sync {
    fn can_place_at(
        &self,
        block: &Block,
        state: &BlockState,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
    ) -> bool;
}

pub trait BlockAccessor: Send + Sync {
    fn get_block<'a>(
        &'a self,
        position: &'a BlockPos,
    ) -> Pin<Box<dyn Future<Output = &'static Block> + Send + 'a>>;

    fn get_block_state<'a>(
        &'a self,
        position: &'a BlockPos,
    ) -> Pin<Box<dyn Future<Output = &'static BlockState> + Send + 'a>>;

    fn get_block_state_id<'a>(
        &'a self,
        position: &'a BlockPos,
    ) -> Pin<Box<dyn Future<Output = BlockStateId> + Send + 'a>>;

    fn get_block_and_state<'a>(
        &'a self,
        position: &'a BlockPos,
    ) -> Pin<Box<dyn Future<Output = (&'static Block, &'static BlockState)> + Send + 'a>>;
}

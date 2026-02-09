use std::sync::Arc;

use dashmap::DashMap;
use pumpkin_config::{chunk::ChunkConfig, world::LevelConfig};
use pumpkin_data::{chunk_gen_settings::GenerationSettings, dimension::Dimension};
use pumpkin_util::{math::position::ChunkPos, world_seed::Seed};

use crate::{
    chunk::{
        Chunk,
        format::{anvil::AnvilChunkFile, linear::LinearFile},
        io::{FileIO, file_manager::ChunkFileManager},
    },
    level::folder::LevelFolder,
    registry::BlockRegistryExt,
};

pub mod folder;

pub struct Level {
    pub dimension: Dimension,
    pub seed: Seed,
    pub block_registry: Arc<dyn BlockRegistryExt>,
    pub level_folder: LevelFolder,
    pub loaded_chunks: Arc<DashMap<ChunkPos, Arc<Chunk>>>,
    pub generation_settings: &'static GenerationSettings,
    pub chunk_saver: Arc<dyn FileIO<Data = Arc<Chunk>>>,
    // pub loaded_chunks: ...
}

impl Level {
    pub fn new(
        level_config: &LevelConfig,
        level_folder: LevelFolder,
        dimension: Dimension,
        seed: Seed,
        block_registry: Arc<dyn BlockRegistryExt>,
    ) -> Self {
        let chunk_saver: Arc<dyn FileIO<Data = Arc<Chunk>>> = match &level_config.chunk {
            ChunkConfig::Linear(config) => {
                Arc::new(ChunkFileManager::<LinearFile<Chunk>>::new(config.clone()))
            }
            ChunkConfig::Anvil(config) => Arc::new(ChunkFileManager::<AnvilChunkFile<Chunk>>::new(
                config.clone(),
            )),
        };

        Self {
            dimension,
            seed,
            block_registry,
            level_folder,
            loaded_chunks: Arc::new(DashMap::new()),
            generation_settings: GenerationSettings::from_dimension(&dimension),
            chunk_saver,
        }
    }

    pub async fn load_chunks(&mut self, chunks: &[(i32, i32)]) {
        let loaded_chunks = Vec::new();

        self.chunk_saver
            .fetch_chunks(&self.level_folder, &[pos], t_send.clone())
            .await;
        // self.generation_settings.
        //
    }
}

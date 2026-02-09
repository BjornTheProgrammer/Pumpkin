use pumpkin_data::dimension::Dimension;
use pumpkin_util::world_seed::Seed;
use pumpkin_world_new::level::{Level, folder::LevelFolder};

fn main() {
    let dimension = Dimension::OVERWORLD;
    let seed = Seed(0);
    let block_registry = pumpkin::block::registry::default_registry();
    let temp_dir = std::env::temp_dir();
    let level_folder = LevelFolder::new(temp_dir);

    let level = Level::new(dimension, seed, block_registry, level_folder);
}

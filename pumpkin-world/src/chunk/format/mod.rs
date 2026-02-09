use std::{
    io::Cursor,
    path::PathBuf,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};

use bytes::Bytes;
use futures::future::join_all;
use pumpkin_data::{Block, chunk::ChunkStatus, fluid::Fluid};
use pumpkin_nbt::{compound::NbtCompound, from_bytes, nbt_long_array};
use rustc_hash::FxHashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

use pumpkin_util::math::{position::ChunkPos, vector2::Vector2};
use serde::{Deserialize, Serialize};

use crate::{
    block::{BlockStateCodec, entities::block_entity_from_nbt},
    chunk::{
        Chunk, ChunkEntityData,
        format::{
            anvil::{SingleChunkDataSerializer, WORLD_DATA_VERSION},
            errors::{ChunkParsingError, ChunkReadingError, ChunkSerializingError},
        },
        heightmap::ChunkHeightmaps,
        io::{Dirtiable, file_manager::PathFromLevelFolder},
        light::{ChunkLight, LightContainer},
        palette::{BiomeSectionData, BlockSectionData},
        section::ChunkSection,
    },
    cords,
    level::folder::LevelFolder,
    tick::{ScheduledTick, scheduler::ChunkTickScheduler},
};

pub mod anvil;
pub mod errors;
pub mod linear;

impl SingleChunkDataSerializer for Chunk {
    #[inline]
    fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
        Self::internal_from_bytes(bytes, pos).map_err(ChunkReadingError::ParsingError)
    }

    #[inline]
    fn to_bytes(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, ChunkSerializingError>> + Send + '_>> {
        Box::pin(async move { self.internal_to_bytes().await })
    }

    #[inline]
    fn position(&self) -> (i32, i32) {
        // This is really x and z
        (self.position.0.x, self.position.0.y)
    }
}

impl PathFromLevelFolder for Chunk {
    #[inline]
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf {
        folder.region_folder.join(file_name)
    }
}

impl Dirtiable for Chunk {
    #[inline]
    fn mark_dirty(&self, flag: bool) {
        self.dirty.store(flag, Ordering::Relaxed);
    }

    #[inline]
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
}

impl Chunk {
    pub fn internal_from_bytes(
        chunk_data: &[u8],
        position: Vector2<i32>,
    ) -> Result<Self, ChunkParsingError> {
        let chunk_data = from_bytes::<ChunkNbt>(Cursor::new(chunk_data))
            .map_err(|e| ChunkParsingError::ErrorDeserializingChunk(e.to_string()))?;

        if chunk_data.light_correct {
            for section in &chunk_data.sections {
                let mut block = false;
                let mut sky = false;
                let mut block_sum = 0;
                let mut sky_sum = 0;
                if let Some(block_light) = &section.block_light {
                    block = !block_light.is_empty();
                    block_sum = block_light
                        .iter()
                        .map(|b| ((*b >> 4) + (*b & 0x0F)) as usize)
                        .sum();
                }
                if let Some(sky_light) = &section.sky_light {
                    sky = !sky_light.is_empty();
                    sky_sum = sky_light
                        .iter()
                        .map(|b| ((*b >> 4) + (*b & 0x0F)) as usize)
                        .sum();
                }
                if (block || sky) && section.y == -5 {
                    log::trace!(
                        "section {},{},{}: block_light={}/{}, sky_light={}/{}",
                        chunk_data.x_pos,
                        section.y,
                        chunk_data.z_pos,
                        block,
                        block_sum,
                        sky,
                        sky_sum,
                    );
                }
            }
        }

        if chunk_data.x_pos != position.x || chunk_data.z_pos != position.y {
            return Err(ChunkParsingError::ErrorDeserializingChunk(format!(
                "Expected data for chunk {},{} but got it for {},{}!",
                position.x, position.y, chunk_data.x_pos, chunk_data.z_pos,
            )));
        }
        let (block_lights, sky_lights, chunk_sections) = chunk_data
            .sections
            .into_iter()
            .map(|section| {
                // Map light data to the LightContainer enum
                let block_light = section
                    .block_light
                    .map_or(LightContainer::Empty(0), LightContainer::Full); // Standard default
                let sky_light = section
                    .sky_light
                    .map_or(LightContainer::Empty(15), LightContainer::Full); // Sky is usually bright

                // Convert NBT to Palettes
                let block_palette = section
                    .block_states
                    .map(BlockSectionData::from_disk_nbt)
                    .unwrap_or_default();
                let biome_palette = section
                    .biomes
                    .map(BiomeSectionData::from_disk_nbt)
                    .unwrap_or_default();

                let block_count = block_palette.count_non_air();

                (
                    block_light,
                    sky_light,
                    ChunkSection {
                        block_data: block_palette,
                        biome_data: biome_palette,
                        block_count: block_count,
                    },
                )
            })
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |(mut bl, mut sl, mut bp), (b_l, s_l, b_p)| {
                    bl.push(b_l);
                    sl.push(s_l);
                    bp.push(b_p);
                    (bl, sl, bp)
                },
            );

        // 2. Assemble the LightEngine
        let light_engine = ChunkLight {
            block_light: block_lights.into_boxed_slice(),
            sky_light: sky_lights.into_boxed_slice(),
        };

        // 3. Assemble the ChunkSections using your specific struct fields
        let min_y = cords::section::section_to_block(chunk_data.min_y_section);

        Ok(Self {
            sections: chunk_sections.into_boxed_slice(),
            heightmaps: std::sync::Mutex::new(chunk_data.heightmaps),
            light: light_engine,
            dirty: AtomicBool::new(false),
            position: ChunkPos(position),
            min_y,
            block_entities: {
                let mut block_entities = FxHashMap::default();
                for nbt in chunk_data.block_entities {
                    let block_entity = block_entity_from_nbt(&nbt);
                    if let Some(block_entity) = block_entity {
                        block_entities.insert(block_entity.get_position(), block_entity);
                    }
                }
                std::sync::Mutex::new(block_entities)
            },
            block_ticks: ChunkTickScheduler::from_iter(chunk_data.block_ticks),
            fluid_ticks: ChunkTickScheduler::from_iter(chunk_data.fluid_ticks),
            status: chunk_data.status,
        })
    }

    async fn internal_to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
        let sections: Vec<ChunkSectionNBT> = {
            let min_section_y = (self.min_y >> 4) as i8;

            (0..self.sections.len())
                .map(|i| {
                    ChunkSectionNBT {
                        y: i as i8 + min_section_y,
                        // Convert the palettes to their NBT disk representation
                        block_states: Some(self.sections[i].block_data.to_disk_nbt()),
                        biomes: Some(self.sections[i].biome_data.to_disk_nbt()),
                        block_light: match self.light.block_light.get(i) {
                            Some(LightContainer::Full(data)) => Some(data.clone()),
                            _ => None,
                        },
                        sky_light: match self.light.sky_light.get(i) {
                            Some(LightContainer::Full(data)) => Some(data.clone()),
                            _ => None,
                        },
                    }
                })
                .collect()
        };

        let heightmaps = self.heightmaps.lock().unwrap().clone();

        let entities_to_serialize = {
            let entities_guard = self.block_entities.lock().unwrap();
            entities_guard.values().cloned().collect::<Vec<_>>()
        };

        let block_entities_nbt = join_all(entities_to_serialize.into_iter().map(
            |block_entity| async move {
                let mut nbt = NbtCompound::new();
                block_entity.write_internal(&mut nbt).await;
                nbt
            },
        ))
        .await;

        let nbt = ChunkNbt {
            data_version: WORLD_DATA_VERSION,
            x_pos: self.position.0.x,
            z_pos: self.position.0.y,
            min_y_section: cords::section::block_to_section(self.min_y),
            status: self.status,
            heightmaps,
            sections,
            block_ticks: self.block_ticks.to_vec(),
            fluid_ticks: self.fluid_ticks.to_vec(),
            block_entities: block_entities_nbt,
            light_correct: false,
        };

        let mut result = Vec::new();
        pumpkin_nbt::to_bytes(&nbt, &mut result)
            .map_err(ChunkSerializingError::ErrorSerializingChunk)?;
        Ok(result.into())
    }
}

impl PathFromLevelFolder for ChunkEntityData {
    #[inline]
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf {
        folder.entities_folder.join(file_name)
    }
}

impl Dirtiable for ChunkEntityData {
    #[inline]
    fn mark_dirty(&self, flag: bool) {
        self.dirty.store(flag, Ordering::Relaxed);
    }

    #[inline]
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
}

impl SingleChunkDataSerializer for ChunkEntityData {
    #[inline]
    fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
        Self::internal_from_bytes(bytes, pos).map_err(ChunkReadingError::ParsingError)
    }

    #[inline]
    fn to_bytes(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, ChunkSerializingError>> + Send + '_>> {
        Box::pin(async move { self.internal_to_bytes().await })
    }

    #[inline]
    fn position(&self) -> (i32, i32) {
        (self.x, self.z)
    }
}

impl ChunkEntityData {
    fn internal_from_bytes(
        chunk_data: &[u8],
        position: Vector2<i32>,
    ) -> Result<Self, ChunkParsingError> {
        let chunk_entity_data = pumpkin_nbt::from_bytes::<EntityNbt>(Cursor::new(chunk_data))
            .map_err(|e| ChunkParsingError::ErrorDeserializingChunk(e.to_string()))?;

        if chunk_entity_data.position[0] != position.x
            || chunk_entity_data.position[1] != position.y
        {
            return Err(ChunkParsingError::ErrorDeserializingChunk(format!(
                "Expected data for entity chunk {},{} but got it for {},{}!",
                position.x,
                position.y,
                chunk_entity_data.position[0],
                chunk_entity_data.position[1],
            )));
        }
        let mut map = FxHashMap::default();
        for entity_nbt in chunk_entity_data.entities {
            let uuid = if let Some(uuid) = entity_nbt.get_int_array("UUID") {
                Uuid::from_u128(
                    (uuid[0] as u128) << 96
                        | (uuid[1] as u128) << 64
                        | (uuid[2] as u128) << 32
                        | (uuid[3] as u128),
                )
            } else {
                log::debug!(
                    "Entity in chunk {},{} is missing UUID: {:?}",
                    position.x,
                    position.y,
                    entity_nbt
                );
                continue;
            };

            map.insert(uuid, entity_nbt);
        }

        Ok(Self {
            x: position.x,
            z: position.y,
            data: Mutex::new(map),
            dirty: AtomicBool::new(false),
        })
    }

    async fn internal_to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
        let nbt = EntityNbt {
            data_version: WORLD_DATA_VERSION,
            position: [self.x, self.z],
            entities: self.data.lock().await.values().cloned().collect(),
        };

        let mut result = Vec::new();
        pumpkin_nbt::to_bytes(&nbt, &mut result)
            .map_err(ChunkSerializingError::ErrorSerializingChunk)?;
        Ok(result.into())
    }
}

#[derive(Serialize, Deserialize)]
struct ChunkSectionNBT {
    #[serde(skip_serializing_if = "Option::is_none")]
    block_states: Option<ChunkSectionBlockStates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    biomes: Option<ChunkSectionBiomes>,
    #[serde(rename = "BlockLight", skip_serializing_if = "Option::is_none")]
    block_light: Option<Box<[u8]>>,
    #[serde(rename = "SkyLight", skip_serializing_if = "Option::is_none")]
    sky_light: Option<Box<[u8]>>,
    #[serde(rename = "Y")]
    y: i8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChunkSectionBiomes {
    #[serde(
        serialize_with = "nbt_long_array",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) data: Option<Box<[i64]>>,
    pub(crate) palette: Vec<PaletteBiomeEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
// NOTE: Change not documented in the wiki; biome palettes are directly just the name now
#[serde(rename_all = "PascalCase", transparent)]
pub struct PaletteBiomeEntry {
    /// Biome name
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChunkSectionBlockStates {
    #[serde(
        serialize_with = "nbt_long_array",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) data: Option<Box<[i64]>>,
    pub(crate) palette: Vec<BlockStateCodec>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ChunkNbt {
    data_version: i32,
    #[serde(rename = "xPos")]
    x_pos: i32,
    #[serde(rename = "zPos")]
    z_pos: i32,
    #[serde(rename = "yPos")]
    min_y_section: i32,
    status: ChunkStatus,
    #[serde(rename = "sections")]
    sections: Vec<ChunkSectionNBT>,
    heightmaps: ChunkHeightmaps,
    #[serde(rename = "block_ticks")]
    block_ticks: Vec<ScheduledTick<&'static Block>>,
    #[serde(rename = "fluid_ticks")]
    fluid_ticks: Vec<ScheduledTick<&'static Fluid>>,
    #[serde(rename = "block_entities")]
    block_entities: Vec<NbtCompound>,
    #[serde(rename = "isLightOn")]
    light_correct: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct EntityNbt {
    data_version: i32,
    position: [i32; 2],
    entities: Vec<NbtCompound>,
}

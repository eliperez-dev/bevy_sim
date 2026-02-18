
use bevy::prelude::Color;

pub const OCEAN_THRESHOLD: f32 = 0.45;

pub const CHUNK_SIZE: f32 = 1000.0; 
pub const MAP_HEIGHT_SCALE: f32 = 230.0;
pub const TERRAIN_SMOOTHNESS: f32 = 0.0;

pub const LIGHTING_BOUNDS: f32 = 5000.0;
pub const COMPUTE_SMOOTH_NORMALS: bool = false;

pub const RENDER_DISTANCE: i32 = 4;
pub const MAX_CHUNKS_PER_FRAME: usize = 200;

pub const MAX_ILLUMANENCE: f32 = 7_300.0;

pub const LOD_QUALITY_MULTIPLIER: u32 = 1;
pub const LOD_DISTANCE_MULTILIER: f32 = 2.0;

pub const LOD_LEVELS: [(f32, u32); 4] = [
    (5.0 * LOD_DISTANCE_MULTILIER , 8 * LOD_QUALITY_MULTIPLIER),
    (10.0 * LOD_DISTANCE_MULTILIER, 4* LOD_QUALITY_MULTIPLIER),
    (20.0 * LOD_DISTANCE_MULTILIER, 2 * LOD_QUALITY_MULTIPLIER),
    (40.0 * LOD_DISTANCE_MULTILIER, 1 * LOD_QUALITY_MULTIPLIER),
];





pub struct TerrainStop {
    pub height: f32,
    pub color: Color,
}

pub const GRASSLANDS_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -1.0, color: Color::srgb(0.3, 0.2, 0.1) }, // Dirt
    TerrainStop { height: -0.5,  color: Color::srgb(0.8, 0.7, 0.5) }, // Sand
    TerrainStop { height: 0.2,  color: Color::srgb(0.2, 0.5, 0.2) }, // Grass
    TerrainStop { height: 2.5,  color: Color::srgb(0.5, 0.5, 0.5) }, // Rock
];

pub const DESERT_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -1.0, color: Color::srgb(0.6, 0.4, 0.2) }, // Hard dirt
    TerrainStop { height: -0.5,  color: Color::srgb(0.9, 0.8, 0.5) }, // Sand
    TerrainStop { height: 0.7,  color: Color::srgb(0.8, 0.6, 0.3) }, // Orange dunes
    TerrainStop { height: 1.5,  color: Color::srgb(0.7, 0.4, 0.2) }, // Red Rock
    TerrainStop { height: 2.5,  color: Color::srgb(0.6, 0.3, 0.1) }, // Dark Mesa peak
];

pub const TAIGA_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -1.0, color: Color::srgb(0.2, 0.2, 0.2) }, // Dark Dirt
    TerrainStop { height: -0.5,  color: Color::srgb(0.4, 0.4, 0.4) }, // Gravel
    TerrainStop { height: 0.3,  color: Color::srgb(0.1, 0.3, 0.2) }, // Dark Pine Grass
    TerrainStop { height: 1.0,  color: Color::srgb(0.5, 0.5, 0.5) }, // Rock
    TerrainStop { height: 1.5,  color: Color::WHITE },               // Heavy Snow
];

pub const FOREST_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -1.0, color: Color::srgb(0.3, 0.2, 0.1) }, // Dirt
    TerrainStop { height: -0.5,  color: Color::srgb(0.2, 0.4, 0.1) }, // Deep Grass
    TerrainStop { height: 0.3,  color: Color::srgb(0.1, 0.5, 0.1) }, // Lush Canopy
    TerrainStop { height: 2.0,  color: Color::srgb(0.4, 0.4, 0.4) }, // Rock
    TerrainStop { height: 3.0,  color: Color::WHITE },               // Snow
];
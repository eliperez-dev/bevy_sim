use bevy::prelude::Color;

pub const OCEAN_HUMIDITY_THRESHOLD: f32 = 0.70;
pub const OCEAN_HUMIDITY_OFFSET: f32 = 0.1;
pub const OCEAN_HOT_TEMP_THRESHOLD: f32 = 0.9;
pub const OCEAN_COLD_TEMP_THRESHOLD: f32 = 0.0;
pub const OCEAN_TRANSITION_WIDTH: f32 = 0.3;

pub const CHUNK_SIZE: f32 = 1000.0; 
pub const MAP_HEIGHT_SCALE: f32 = 500.0;

pub const MAX_ILLUMANENCE: f32 = 5_300.0;
pub const TERRAIN_HORIZONTAL_SCALE: f32 = 1.0;

pub fn world_units_to_meters(world_units: f32) -> f32 {
    world_units * 0.41967669172
}

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
    TerrainStop { height: 0.8,  color: Color::srgb(0.5, 0.5, 0.5) }, // Rock
    TerrainStop { height: 1.0,  color: Color::WHITE },               // Heavy Snow
];

pub const FOREST_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -1.0, color: Color::srgb(0.3, 0.2, 0.1) }, // Dirt
    TerrainStop { height: -0.5,  color: Color::srgb(0.2, 0.4, 0.1) }, // Deep Grass
    TerrainStop { height: 0.3,  color: Color::srgb(0.1, 0.8, 0.1) }, // Lush Canopy
    TerrainStop { height: 2.7,  color: Color::srgb(0.4, 0.4, 0.4) }, // Rock
    TerrainStop { height: 3.0,  color: Color::WHITE },               // Snow
];


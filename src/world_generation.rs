use bevy::color::Mix;
use noise::{NoiseFn, Perlin};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;

use bevy::{
    mesh::VertexAttributeValues,
    platform::collections::HashSet,
    prelude::*
};

use crate::consts::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Biome {
    Desert,
    Grasslands,
    Taiga,
    Forest, 
}

#[derive(Resource, Clone)]
pub struct WorldGenerator {
    terrain_layers: Vec<PerlinLayer>,
    temperature_layer: PerlinLayer,
    humidity_layer: PerlinLayer,
}

impl WorldGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            terrain_layers: vec![
                // Base layer (Broad continents)
                PerlinLayer::new(seed, 0.25, 3.5),      
                // Detail 1 (Hills - double the frequency, half the amplitude)
                PerlinLayer::new(seed + 100, 0.5, 1.75), 
                // Detail 2 (Ridges)
                PerlinLayer::new(seed + 200, 1.0, 0.8),  
                // Detail 3 (Bumps/Rocks)
                PerlinLayer::new(seed + 300, 2.0, 0.4),  
            ],
            // Note: Temperature and humidity need to be broad, so keep scales low!
            temperature_layer: PerlinLayer::new(seed + 400, 0.05, 1.0),
            humidity_layer: PerlinLayer::new(seed + 500, 0.05, 1.0),
        }
    }

    pub fn get_climate(&self, pos: &[f32; 3]) -> (f32, f32) {
        let raw_temp = self.temperature_layer.get_level(pos);
        let raw_hum = self.humidity_layer.get_level(pos);

        // Normalize to 0.0 -> 1.0 and clamp it
        let temp_normalized = (((raw_temp / self.temperature_layer.vertical_scale) + 1.0) * 0.5).clamp(0.0, 1.0);
        let hum_normalized = (((raw_hum / self.humidity_layer.vertical_scale) + 1.0) * 0.5).clamp(0.0, 1.0);

        (temp_normalized, hum_normalized)
    }

    pub fn get_biome(&self, pos: &[f32; 3]) -> Biome {
        // 1. Get raw noise (typically -1.0 to 1.0)
        let raw_temp = self.temperature_layer.get_level(pos);
        let raw_hum = self.humidity_layer.get_level(pos);

        // 2. Normalize to roughly 0.0 -> 1.0
        // We divide by vertical_scale to reverse the multiplier you applied in get_level
        let temp_normalized = ((raw_temp / self.temperature_layer.vertical_scale) + 1.0) * 0.5;
        let hum_normalized = ((raw_hum / self.humidity_layer.vertical_scale) + 1.0) * 0.5;

        // 3. Simple 2x2 Biome Matrix
        if temp_normalized > 0.5 { 
            // Hot climates
            if hum_normalized > 0.45 { 
                Biome::Forest // Hot & Wet
            } else { 
                Biome::Desert // Hot & Dry
            }
        } else { 
            // Cold climates
            if hum_normalized > 0.5 { 
                Biome::Taiga // Cold & Wet
            } else { 
                Biome::Grasslands // Cold & Dry
            }
        }
    }
}

#[derive(Resource, Clone)]
struct PerlinLayer {
    perlin: Perlin,
    horizontal_scale: f32,
    vertical_scale: f32,
    offset: f64, 
}

impl PerlinLayer {
    pub fn new(seed: u32, horizontal_scale: f32, vertical_scale: f32) -> Self {
        Self {
            perlin: Perlin::new(seed),
            horizontal_scale,
            vertical_scale,
            offset: (seed as f64 * 1337.42) % 100000.0, 
        }
    }

    pub fn get_level(&self, pos: &[f32; 3]) -> f32 {
        let height = self.perlin
            .get([
                (pos[0] * self.horizontal_scale / 1000.0) as f64 + self.offset,
                (pos[2] * self.horizontal_scale / 1000.0) as f64 + (self.offset.sqrt() + 202994.0)
            ]
        ) as f32;
        height * self.vertical_scale
    }
}

fn get_biome_elevation_offset(temp: f32, humidity: f32) -> f32 {
    let desert_elev = -0.1;    
    let grass_elev = 0.1;     // Slightly above sea level
    let forest_elev = 0.2;    // Higher plateau
    let taiga_elev = 0.8;     // High mountain base

    // Use the same bilinear interpolation as the multiplier
    let cold_blend = grass_elev + (taiga_elev - grass_elev) * humidity;
    let hot_blend = desert_elev + (forest_elev - desert_elev) * humidity;

    cold_blend + (hot_blend - cold_blend) * temp
}


fn get_biome_height_multiplier(temp: f32, humidity: f32) -> f32 {
    // Define the extreme bounds for our biomes
    let desert_mult = 0.05;  // Very flat dunes
    let grass_mult = 0.20;    // Gentle rolling hills
    let forest_mult = 0.65;   // Steeper, uneven terrain
    let taiga_mult = 1.33;    // High, jagged mountains

    // Blend along the humidity axis (dry -> wet)
    // Lerp formula: start + (end - start) * percent
    let cold_blend = grass_mult + (taiga_mult - grass_mult) * humidity;
    let hot_blend = desert_mult + (forest_mult - desert_mult) * humidity;

    // Blend along the temperature axis (cold -> hot)

    cold_blend + (hot_blend - cold_blend) * temp
}

#[derive(Component)]
pub struct ChunkTask(Task<Mesh>);

pub fn modify_plane(
    mut commands: Commands,
    query: Query<(Entity, &Mesh3d, &Transform), Added<Chunk>>,
    world_generator: Res<WorldGenerator>,
    meshes: Res<Assets<Mesh>>,
) {
    let thread_pool = AsyncComputeTaskPool::get();
    for (entity, mesh_handle, transform) in &query {
        if let Some(mesh) = meshes.get(mesh_handle) {
            let mut mesh_clone = mesh.clone();
            let world_gen = world_generator.clone();
            let transform_clone = *transform;

            let task = thread_pool.spawn(async move {
                let mut colors: Vec<[f32; 4]> = Vec::new();

                if let Some(VertexAttributeValues::Float32x3(positions)) = 
                    mesh_clone.attribute_mut(Mesh::ATTRIBUTE_POSITION) 
                {
                    colors.reserve(positions.len());

                    for pos in positions.iter_mut() {
                        let world_pos = [
                            pos[0] + transform_clone.translation.x,
                            pos[1] + transform_clone.translation.y,
                            pos[2] + transform_clone.translation.z,
                        ];

                        let mut base_height = 0.0;
                        let (temp, humidity) = world_gen.get_climate(&world_pos);

                        for layer in &world_gen.terrain_layers {
                            base_height += layer.get_level(&world_pos);
                        }

                        let height_multiplier = get_biome_height_multiplier(temp, humidity);
                        let elevation_offset = get_biome_elevation_offset(temp, humidity);

                        let final_height = base_height * height_multiplier + elevation_offset;
                        colors.push(get_terrain_color(final_height, temp, humidity));
                        pos[1] = final_height * MAP_HEIGHT_SCALE;
                    }
                }
                
                mesh_clone.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

                if COMPUTE_SMOOTH_NORMALS {
                    mesh_clone.compute_smooth_normals();
                } else {
                    mesh_clone.duplicate_vertices();
                    mesh_clone.compute_flat_normals()
                }
                mesh_clone
            });

            commands.entity(entity).insert(ChunkTask(task));
        }
    }
}

pub fn handle_compute_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &Mesh3d, &mut ChunkTask)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mesh_handle, mut task) in &mut tasks {
        if let Some(new_mesh) = future::block_on(future::poll_once(&mut task.0)) {
            // Update the mesh asset
            if let Some(mesh) = meshes.get_mut(mesh_handle) {
                *mesh = new_mesh;
            }

            // Remove the task component
            commands.entity(entity).remove::<ChunkTask>();
        }
    }
}

#[derive(Component)]
pub struct Chunk {
    x: i32,
    z: i32,
}

#[derive(Resource, Default)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<(i32, i32)>,
}

#[derive(Resource)]
pub struct SharedChunkMaterials {
    pub terrain_material: Handle<StandardMaterial>,
    pub water_material: Handle<StandardMaterial>,
}
pub fn generate_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    shared_materials: Res<SharedChunkMaterials>,
    mut chunk_manager: ResMut<ChunkManager>,
    camera: Query<&Transform, With<Camera>>,
) {
    let cam_transform = camera.single().unwrap().translation;
    
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    let render_distance_sq = (RENDER_DISTANCE as f32).powi(2);
    
    let mut chunks_to_spawn = Vec::new();

    for x in (cam_x - RENDER_DISTANCE)..=(cam_x + RENDER_DISTANCE) {
        for z in (cam_z - RENDER_DISTANCE)..=(cam_z + RENDER_DISTANCE) {
            let dx = (x - cam_x) as f32;
            let dz = (z - cam_z) as f32;
            let distance_sq = dx * dx + dz * dz;

            if distance_sq <= render_distance_sq
                && !chunk_manager.spawned_chunks.contains(&(x, z)) {
                    chunks_to_spawn.push((x, z, distance_sq));
                }
        }
    }

    chunks_to_spawn.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    for (x, z, _) in chunks_to_spawn.iter().take(MAX_CHUNKS_PER_FRAME) {
        chunk_manager.spawned_chunks.insert((*x, *z));
        
        let x_pos = *x as f32 * CHUNK_SIZE;
        let z_pos = *z as f32 * CHUNK_SIZE;
        
        commands.spawn((
            Mesh3d(meshes.add(
                Plane3d::default().mesh()
                .size(CHUNK_SIZE, CHUNK_SIZE)
                .subdivisions((TERRAIN_QUALITY) as u32)
            )),
            MeshMaterial3d(shared_materials.terrain_material.clone()),
            Transform::from_xyz(x_pos, 0.0, z_pos),
            Chunk { x: *x, z: *z },
        )).with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Plane3d::default().mesh().size(CHUNK_SIZE, CHUNK_SIZE))),
                MeshMaterial3d(shared_materials.water_material.clone()),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        });
    }
}

pub fn despawn_out_of_bounds_chunks(
    mut commands: Commands,
    camera: Query<&Transform, With<Camera>>,
    chunks: Query<(Entity, &Chunk)>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    let cam_transform = camera.single().unwrap().translation;
    
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    let despawn_distance_sq = (DESPAWN_DISTANCE as f32).powi(2);
    
    let mut chunks_to_despawn = Vec::new();

    for (entity, chunk) in &chunks {
        let dx = (chunk.x - cam_x) as f32;
        let dz = (chunk.z - cam_z) as f32;
        let distance_sq = dx * dx + dz * dz;

        if distance_sq > despawn_distance_sq {
            chunks_to_despawn.push((entity, chunk.x, chunk.z, distance_sq));
        }
    }

    chunks_to_despawn.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());

    for (entity, x, z, _) in chunks_to_despawn.iter().take(MAX_CHUNKS_PER_FRAME) {
        commands.entity(*entity).despawn_recursive();
        chunk_manager.spawned_chunks.remove(&(*x, *z));
    }
}




fn get_color_from_palette(height: f32, palette: &[TerrainStop]) -> Color {
    if height.is_nan() { return palette[0].color; }

    let mut upper_idx = 0;
    while upper_idx < palette.len() && height > palette[upper_idx].height {
        upper_idx += 1;
    }

    if upper_idx == 0 { return palette[0].color; }
    if upper_idx >= palette.len() { return palette.last().unwrap().color; }

    let lower = &palette[upper_idx - 1];
    let upper = &palette[upper_idx];

    let range = upper.height - lower.height;
    let t = ((height - lower.height) / range).clamp(0.0, 1.0);
    
    let blend_start = 1.0 - TERRAIN_SMOOTHNESS.clamp(0.0, 1.0);
    let base_color = lower.color.to_linear();
    let next_color = upper.color.to_linear();

    if t > blend_start && TERRAIN_SMOOTHNESS > 0.0 {
        let blend_t = (t - blend_start) / TERRAIN_SMOOTHNESS;
        base_color.mix(&next_color, blend_t).into()
    } else {
        base_color.into()
    }
}

// Notice the signature changed to accept temp and humidity
fn get_terrain_color(height: f32, temp: f32, humidity: f32) -> [f32; 4] {
    // 1. Get what the color *would* be if the world was 100% this biome
    let forest_color = get_color_from_palette(height, FOREST_TERRAIN_LEVELS).to_linear();
    let desert_color = get_color_from_palette(height, DESERT_TERRAIN_LEVELS).to_linear();
    let taiga_color = get_color_from_palette(height, TAIGA_TERRAIN_LEVELS).to_linear();
    let grass_color = get_color_from_palette(height, GRASSLANDS_TERRAIN_LEVELS).to_linear();

    // 2. Bilinear Interpolation
    // First, blend the humidity axis (dry -> wet) for both hot and cold extremes
    let cold_blend = grass_color.mix(&taiga_color, humidity); 
    let hot_blend = desert_color.mix(&forest_color, humidity);  

    // Finally, blend between those two results along the temperature axis (cold -> hot)
    let final_color = cold_blend.mix(&hot_blend, temp);

    final_color.to_f32_array()
}
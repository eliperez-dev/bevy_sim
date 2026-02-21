use bevy::color::Mix;
use bevy::ecs::relationship::Relationship;
use bevy::light::CascadeShadowConfig;
use noise::{NoiseFn, Perlin};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;

use bevy::{
    mesh::VertexAttributeValues,
    platform::collections::HashSet,
    prelude::*
};

use crate::{RenderSettings, consts::*};
use crate::controls::MainCamera;

#[derive(Component)]
pub struct WaterChunk;
pub fn _animate_water_cpu(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    // 1. Add `&Parent` so the water knows which chunk it belongs to
    water_query: Query<(&Mesh3d, &GlobalTransform, &ChildOf), With<WaterChunk>>, 
    // 2. Query to read the terrain chunk's data
    chunk_query: Query<&Chunk>, 
    chunk_manager: Res<ChunkManager>,
) {
    let t = time.elapsed_secs();
    
    let amplitude = 5.0;
    let frequency = 0.015;
    let speed = 1.5;

    // Grab the subdivision count of your highest fidelity LOD.
    // Assuming chunk_manager.lod_levels is sorted nearest-to-farthest, index 0 is your highest LOD.
    // (e.g., if your highest LOD is 64 subdivisions, this evaluates to 64)
    let highest_lod = chunk_manager.lod_levels[0].1; 

    for (mesh_handle, global_transform, parent) in water_query.iter() {
        
        // 3. Look up the parent terrain chunk. If it doesn't exist, skip.
        let Ok(chunk) = chunk_query.get(parent.get()) else { continue; };
        
        // Is this the highest LOD chunk?
        let is_high_fidelity = chunk.current_lod == highest_lod;

        if let Some(mesh) = meshes.get_mut(mesh_handle)
            && let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
                
                let chunk_world_pos = global_transform.translation();

                for pos in positions.iter_mut() { 
                    if is_high_fidelity {
                        // Do the expensive trig math for nearby chunks
                        let world_x = pos[0] + chunk_world_pos.x;
                        let world_z = pos[2] + chunk_world_pos.z;

                        let wave1 = (world_x * frequency + t * speed).sin() * amplitude;
                        let wave2 = (world_z * frequency * 0.8 + t * speed * 1.2).cos() * amplitude;
                        
                        pos[1] = wave1 + wave2; 
                    } else {
                        // 4. Force lower LOD chunks to remain perfectly flat.
                        // By bypassing the sine/cosine math entirely, we save CPU cycles.
                        pos[1] = 0.0;
                    }
                }
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Biome {
    Desert,
    Grasslands,
    Taiga,
    Forest,
    Ocean,
}

#[derive(Resource, Clone)]
pub struct WorldGenerator {
    pub seed: u32,
    terrain_layers: Vec<PerlinLayer>,
    temperature_layer: PerlinLayer,
    humidity_layer: PerlinLayer,
}

impl WorldGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            terrain_layers: vec![
                PerlinLayer::new(seed,       0.08 * TERRAIN_HORIZONTAL_SCALE, 4.5),    
                PerlinLayer::new(seed,       0.20 * TERRAIN_HORIZONTAL_SCALE, 3.5),      
                PerlinLayer::new(seed + 100, 0.5 * TERRAIN_HORIZONTAL_SCALE, 1.75), 
                PerlinLayer::new(seed + 200, 1.0 * TERRAIN_HORIZONTAL_SCALE, 0.5),  
                PerlinLayer::new(seed + 300, 2.0 * TERRAIN_HORIZONTAL_SCALE, 0.4),  
            ],
            // Note: Temperature and humidity need to be broad, so keep scales low!
            temperature_layer: PerlinLayer::new(seed + 400, 0.06 * TERRAIN_HORIZONTAL_SCALE, 1.0),
            humidity_layer: PerlinLayer::new(seed + 500, 0.06 * TERRAIN_HORIZONTAL_SCALE, 1.0),
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

        // Ocean appears in wet areas OR extreme temperatures
        if hum_normalized > OCEAN_HUMIDITY_THRESHOLD + OCEAN_HUMIDITY_OFFSET
            || temp_normalized > OCEAN_HOT_TEMP_THRESHOLD
            || temp_normalized < OCEAN_COLD_TEMP_THRESHOLD {
            return Biome::Ocean;
        }

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
            if hum_normalized < 0.45 { 
                Biome::Grasslands
            } else { 
                Biome::Taiga
            }
        }
    }

    pub fn get_terrain_height(&self, pos: &[f32; 3]) -> f32 {
        let mut base_height = 0.0;
        let (temp, humidity) = self.get_climate(pos);

        for layer in &self.terrain_layers {
            base_height += layer.get_level(pos);
        }

        let height_multiplier = get_biome_height_multiplier(temp, humidity);
        let elevation_offset = get_biome_elevation_offset(temp, humidity);

        let final_height = base_height * height_multiplier + elevation_offset;
        final_height * MAP_HEIGHT_SCALE
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
    let desert_elev = 0.0;    
    let grass_elev = 0.04;     
    let forest_elev = 0.5;    
    let taiga_elev = 8.0;     
    let ocean_elev = -2.5; // Deep negative offset

    // 1. Calculate land elevation
    let cold_blend = grass_elev + (taiga_elev - grass_elev) * humidity;
    let hot_blend = desert_elev + (forest_elev - desert_elev) * humidity;
    let land_elev = cold_blend + (hot_blend - cold_blend) * temp;

    // 2. Calculate how much this should blend toward ocean (0.0 = land, 1.0 = ocean)
    let hum_blend = if humidity > OCEAN_HUMIDITY_THRESHOLD {
        ((humidity - OCEAN_HUMIDITY_THRESHOLD) / OCEAN_TRANSITION_WIDTH).clamp(0.0, 1.0)
    } else { 0.0 };
    
    let hot_blend = if temp > OCEAN_HOT_TEMP_THRESHOLD - OCEAN_TRANSITION_WIDTH {
        ((temp - (OCEAN_HOT_TEMP_THRESHOLD - OCEAN_TRANSITION_WIDTH)) / OCEAN_TRANSITION_WIDTH).clamp(0.0, 1.0)
    } else { 0.0 };
    
    let cold_blend = if temp < OCEAN_COLD_TEMP_THRESHOLD + OCEAN_TRANSITION_WIDTH {
        ((OCEAN_COLD_TEMP_THRESHOLD + OCEAN_TRANSITION_WIDTH - temp) / OCEAN_TRANSITION_WIDTH).clamp(0.0, 1.0)
    } else { 0.0 };
    
    let ocean_factor = hum_blend.max(hot_blend).max(cold_blend);
    
    // 3. Blend between land and ocean
    land_elev + (ocean_elev - land_elev) * ocean_factor
}

fn get_biome_height_multiplier(temp: f32, humidity: f32) -> f32 {
    let desert_mult = 0.01;
    let grass_mult = 0.02;
    let forest_mult = 0.05;
    let taiga_mult = 1.5;
    let ocean_mult = 0.01; // Oceans are flat

    // 1. Calculate land multiplier
    let cold_blend = grass_mult + (taiga_mult - grass_mult) * humidity;
    let hot_blend = desert_mult + (forest_mult - desert_mult) * humidity;
    let land_mult = cold_blend + (hot_blend - cold_blend) * temp;

    // 2. Calculate how much this should blend toward ocean (0.0 = land, 1.0 = ocean)
    let hum_blend = if humidity > OCEAN_HUMIDITY_THRESHOLD {
        ((humidity - OCEAN_HUMIDITY_THRESHOLD) / OCEAN_TRANSITION_WIDTH).clamp(0.0, 1.0)
    } else { 0.0 };
    
    let hot_blend = if temp > OCEAN_HOT_TEMP_THRESHOLD - OCEAN_TRANSITION_WIDTH {
        ((temp - (OCEAN_HOT_TEMP_THRESHOLD - OCEAN_TRANSITION_WIDTH)) / OCEAN_TRANSITION_WIDTH).clamp(0.0, 1.0)
    } else { 0.0 };
    
    let cold_blend = if temp < OCEAN_COLD_TEMP_THRESHOLD + OCEAN_TRANSITION_WIDTH {
        ((OCEAN_COLD_TEMP_THRESHOLD + OCEAN_TRANSITION_WIDTH - temp) / OCEAN_TRANSITION_WIDTH).clamp(0.0, 1.0)
    } else { 0.0 };
    
    let ocean_factor = hum_blend.max(hot_blend).max(cold_blend);
    
    // 3. Blend between land and ocean
    land_mult + (ocean_mult - land_mult) * ocean_factor
}

#[derive(Component)]
pub struct ChunkTask {
    pub task: Task<Mesh>,
    pub new_handle: Option<Handle<Mesh>>,
}

pub fn modify_plane(
    mut commands: Commands,
    query: Query<(Entity, &Mesh3d, &Transform), Added<Chunk>>,
    world_generator: Res<WorldGenerator>,
    meshes: Res<Assets<Mesh>>,
    render_settings: Res<RenderSettings>,
) {
    let thread_pool = AsyncComputeTaskPool::get();
    for (entity, mesh_handle, transform) in &query {
        if let Some(mesh) = meshes.get(mesh_handle) {
            let mut mesh_clone = mesh.clone();
            let world_gen = world_generator.clone();
            let transform_clone = *transform;

            let smoothness = render_settings.terrain_smoothness;
            let compute_smooth_normals = render_settings.compute_smooth_normals;

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
                        colors.push(get_terrain_color(final_height, temp, humidity, smoothness));
                        pos[1] = final_height * MAP_HEIGHT_SCALE;
                    }
                }
                
                mesh_clone.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

                if compute_smooth_normals {
                    mesh_clone.compute_smooth_normals();
                } else {
                    mesh_clone.duplicate_vertices();
                    mesh_clone.compute_flat_normals()
                }
                mesh_clone
            });

            commands.queue(move |world: &mut World| {
                if let Ok(mut entity) = world.get_entity_mut(entity) {
                    entity.insert(ChunkTask { task, new_handle: None });
                }
            });
        }
    }
}

pub fn handle_compute_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &Mesh3d, &mut ChunkTask, &Chunk)>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&Transform, With<MainCamera>>,
    settings: Res<WorldGenerationSettings>,
) {
    let cam_transform = camera.single().unwrap().translation;
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    let mut task_priorities: Vec<(Entity, f32)> = tasks
        .iter()
        .map(|(entity, _, _, chunk)| {
            let dx = (chunk.x - cam_x) as f32;
            let dz = (chunk.z - cam_z) as f32;
            let distance_sq = dx * dx + dz * dz;
            (entity, distance_sq)
        })
        .collect();

    task_priorities.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut processed_count = 0;
    
    for (entity, _distance) in task_priorities {
        if processed_count >= settings.max_chunks_per_frame {
            break;
        }

        if let Ok((entity, mesh_handle, mut task, _chunk)) = tasks.get_mut(entity)
            && let Some(new_mesh) = future::block_on(future::poll_once(&mut task.task)) {
                if let Some(new_handle) = task.new_handle.take() {
                    if let Some(mesh) = meshes.get_mut(&new_handle) {
                        *mesh = new_mesh;
                    }
                    commands.entity(entity).try_insert(Mesh3d(new_handle));
                    meshes.remove(mesh_handle);
                } else if let Some(mesh) = meshes.get_mut(mesh_handle) {
                    *mesh = new_mesh;
                }

                commands.entity(entity).remove::<ChunkTask>();
                commands.entity(entity).try_insert(Visibility::Visible);
                processed_count += 1;
            }
    }
}

#[derive(Component)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub current_lod: u32,
}

#[derive(Resource, Default)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<(i32, i32)>,
    pub last_camera_chunk: Option<(i32, i32)>,
    pub to_spawn: Vec<(i32, i32)>,
    pub lod_to_update: Vec<Entity>,
    pub render_distance: i32,
    pub lod_levels: [(f32, u32); 5],
    pub lod_quality_multiplier: u32,
    pub lod_distance_multiplier: f32,
}

#[derive(Resource)]
pub struct SharedChunkMaterials {
    pub terrain_material: Handle<StandardMaterial>,
    pub water_material: Handle<StandardMaterial>,
}

fn get_lod_subdivisions(distance_sq: f32, chunk_manager: &ChunkManager) -> u32 {
    let distance = distance_sq.sqrt();
    
    for (max_distance, subdivisions) in chunk_manager.lod_levels.iter() {
        if distance <= *max_distance * chunk_manager.lod_distance_multiplier{
            return *subdivisions * chunk_manager.lod_quality_multiplier;
        }
    }
    
    chunk_manager.lod_levels.last().unwrap().1 * chunk_manager.lod_quality_multiplier
}

#[derive(Resource)]
pub struct WorldGenerationSettings {
    pub max_chunks_per_frame: usize,
}

impl Default for WorldGenerationSettings {
    fn default() -> Self {
        Self {
            max_chunks_per_frame: 100,
        }
    }
}

pub fn generate_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    shared_materials: Res<SharedChunkMaterials>,
    mut chunk_manager: ResMut<ChunkManager>,
    camera: Query<&Transform, With<MainCamera>>,
    mut last_render_distance: Local<Option<i32>>,
    settings: Res<WorldGenerationSettings>,
    mut sun_query: Query<&mut CascadeShadowConfig, (With<crate::day_cycle::Sun>, Without<MainCamera>)>,
    mut render_settings: ResMut<crate::RenderSettings>,
) {
    let mut cascade = sun_query.single_mut().unwrap();

    let cam_transform = camera.single().unwrap().translation;
    
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    // Only re-scan if the camera has moved to a new chunk or render distance changed
    if chunk_manager.last_camera_chunk != Some((cam_x, cam_z)) ||
    *last_render_distance != Some(chunk_manager.render_distance) ||
    render_settings.just_updated
    {
        chunk_manager.last_camera_chunk = Some((cam_x, cam_z));
        *last_render_distance = Some(chunk_manager.render_distance);
        render_settings.just_updated = false;
        
        let render_distance_sq = (chunk_manager.render_distance as f32).powi(2);
        chunk_manager.to_spawn.clear();

        for x in (cam_x - chunk_manager.render_distance)..=(cam_x + chunk_manager.render_distance) {
            for z in (cam_z - chunk_manager.render_distance)..=(cam_z + chunk_manager.render_distance) {
                let dx = (x - cam_x) as f32;
                let dz = (z - cam_z) as f32;
                let distance_sq = dx * dx + dz * dz;

                if distance_sq <= render_distance_sq
                    && !chunk_manager.spawned_chunks.contains(&(x, z)) {
                        chunk_manager.to_spawn.push((x, z));
                    }
            }
        }
        
        // Sort by distance to camera so we spawn closest chunks first
        chunk_manager.to_spawn.sort_by(|a, b| {
            let da = ((a.0 - cam_x).pow(2) + (a.1 - cam_z).pow(2)) as f32;
            let db = ((b.0 - cam_x).pow(2) + (b.1 - cam_z).pow(2)) as f32;
            da.partial_cmp(&db).unwrap()
        });

        // Update cascades
        *cascade = bevy::light::CascadeShadowConfigBuilder {
        first_cascade_far_bound: chunk_manager.render_distance as f32 * CHUNK_SIZE / 10.0,
        maximum_distance: if render_settings.cascades > 0 {
            chunk_manager.render_distance as f32 * CHUNK_SIZE
        } else {
            0.01
        },
        minimum_distance: 0.0,
        num_cascades: render_settings.cascades.max(1),
        ..default()
        }
        .build();
    }

    // Spawn a limited number of chunks from the queue
    let mut spawned_count = 0;
    while spawned_count < settings.max_chunks_per_frame && !chunk_manager.to_spawn.is_empty() {
        let (x, z) = chunk_manager.to_spawn.remove(0);
        
        // Final check: Is it still within range and not already spawned?
        let dx = (x - cam_x) as f32;
        let dz = (z - cam_z) as f32;
        let distance_sq = dx * dx + dz * dz;
        let render_distance_sq = (chunk_manager.render_distance as f32).powi(2);

        if distance_sq <= render_distance_sq && !chunk_manager.spawned_chunks.contains(&(x, z)) {
            chunk_manager.spawned_chunks.insert((x, z));
            
            let x_pos = x as f32 * CHUNK_SIZE;
            let z_pos = z as f32 * CHUNK_SIZE;
            let lod = get_lod_subdivisions(distance_sq, &chunk_manager);
            
            commands.spawn((
                Mesh3d(meshes.add(
                    Plane3d::default().mesh()
                    .size(CHUNK_SIZE, CHUNK_SIZE)
                    .subdivisions(lod)
                )),
                MeshMaterial3d(shared_materials.terrain_material.clone()),
                Transform::from_xyz(x_pos, 0.0, z_pos),
                Chunk { x, z, current_lod: lod },
                Visibility::Hidden,
            )).with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(Plane3d::default().mesh().size(CHUNK_SIZE, CHUNK_SIZE).subdivisions(1))),
                    MeshMaterial3d(shared_materials.water_material.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    WaterChunk,
                    bevy::light::NotShadowCaster
                ));
            });
            spawned_count += 1;
        }
    }
}

pub fn update_chunk_lod(
    mut commands: Commands,
    camera: Query<&Transform, With<MainCamera>>,
    mut chunks: Query<(Entity, &mut Chunk, &Mesh3d, &Transform, Option<&Children>), Without<ChunkTask>>,
    mut meshes: ResMut<Assets<Mesh>>,
    world_generator: Res<WorldGenerator>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut last_cam_pos: Local<Option<(i32, i32)>>,
    settings: Res<WorldGenerationSettings>,
    render_settings: ResMut<RenderSettings>,
) {
    let cam_transform = camera.single().unwrap().translation;
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    // Only re-scan all chunks for LOD changes when the camera moves to a new chunk
    if *last_cam_pos != Some((cam_x, cam_z)) ||
    render_settings.just_updated
     {
        *last_cam_pos = Some((cam_x, cam_z));
        
        let mut candidates = Vec::new();
        for (entity, chunk, _, _, _children) in &chunks {
            let dx = (chunk.x - cam_x) as f32;
            let dz = (chunk.z - cam_z) as f32;
            let distance_sq = dx * dx + dz * dz;
            let desired_lod = get_lod_subdivisions(distance_sq, &chunk_manager);
            
            if desired_lod != chunk.current_lod {
                candidates.push((entity, distance_sq));
            }
        }

        // Sort candidates by distance so we prioritize updating closer chunks
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        chunk_manager.lod_to_update = candidates.into_iter().map(|(e, _)| e).collect();
    }

    let thread_pool = AsyncComputeTaskPool::get();
    let mut processed_count = 0;

    // Process a limited number of LOD updates from the queue
    while processed_count < settings.max_chunks_per_frame && !chunk_manager.lod_to_update.is_empty() {
        let entity = chunk_manager.lod_to_update.remove(0);

        if let Ok((entity, chunk, _mesh_handle, transform, _children)) = chunks.get_mut(entity) {
            let dx = (chunk.x - cam_x) as f32;
            let dz = (chunk.z - cam_z) as f32;
            let distance_sq = dx * dx + dz * dz; 
            let desired_lod = get_lod_subdivisions(distance_sq, &chunk_manager);


            // Double check if it still needs an update (might have been updated by another frame's scan)
            if desired_lod == chunk.current_lod {
                continue;
            }


            let new_mesh_handle = meshes.add(
                Plane3d::default().mesh()
                .size(CHUNK_SIZE, CHUNK_SIZE)
                .subdivisions(desired_lod)
            );
            
            if let Some(mesh) = meshes.get(&new_mesh_handle) {
                let mut mesh_clone = mesh.clone();
                let world_gen = world_generator.clone();
                let transform_clone = *transform;

                let smoothness = render_settings.terrain_smoothness;
                let compute_smooth_normals = render_settings.compute_smooth_normals;

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
                            colors.push(get_terrain_color(final_height, temp, humidity, smoothness));
                            pos[1] = final_height * MAP_HEIGHT_SCALE;
                        }
                    }
                    mesh_clone.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
                    if compute_smooth_normals {
                        mesh_clone.compute_smooth_normals();
                    } else {
                        mesh_clone.duplicate_vertices();
                        mesh_clone.compute_flat_normals()
                    }
                    mesh_clone
                });

                commands.queue(move |world: &mut World| {
                    if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                        entity_mut.insert(ChunkTask { task, new_handle: Some(new_mesh_handle) });
                    }
                });
                
                // Finalize the chunk's LOD state
                if let Ok((_, mut chunk, _, _, _children)) = chunks.get_mut(entity) {
                    chunk.current_lod = desired_lod;
                }
                processed_count += 1;
            }
        }
    }
}

pub fn despawn_out_of_bounds_chunks(
    mut commands: Commands,
    camera: Query<&Transform, With<MainCamera>>,
    chunks: Query<(Entity, &Chunk)>,
    mut chunk_manager: ResMut<ChunkManager>,
    settings: Res<WorldGenerationSettings>,
) {
    let cam_transform = camera.single().unwrap().translation;
    
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    let despawn_distance_sq = ((chunk_manager.render_distance + 1) as f32).powi(2);
    
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

    for (entity, x, z, _) in chunks_to_despawn.iter().take(settings.max_chunks_per_frame * 2) {
        if let Ok(mut entity_commands) = commands.get_entity(*entity) {
            entity_commands.despawn();
            chunk_manager.spawned_chunks.remove(&(*x, *z));
        }
    }
}

fn get_color_from_palette(height: f32, palette: &[TerrainStop], smoothness: f32) -> Color {
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
    
    let blend_start = 1.0 - smoothness.clamp(0.0, 1.0);
    let base_color = lower.color.to_linear();
    let next_color = upper.color.to_linear();

    if t > blend_start && smoothness > 0.0 {
        let blend_t = (t - blend_start) / smoothness;
        base_color.mix(&next_color, blend_t).into()
    } else {
        base_color.into()
    }
}

// Notice the signature changed to accept temp and humidity
fn get_terrain_color(height: f32, temp: f32, humidity: f32, smoothness: f32) -> [f32; 4] {
    // 1. Get what the color *would* be if the world was 100% this biome
    let forest_color = get_color_from_palette(height, FOREST_TERRAIN_LEVELS, smoothness).to_linear();
    let desert_color = get_color_from_palette(height, DESERT_TERRAIN_LEVELS, smoothness).to_linear();
    let taiga_color = get_color_from_palette(height, TAIGA_TERRAIN_LEVELS, smoothness).to_linear();
    let grass_color = get_color_from_palette(height, GRASSLANDS_TERRAIN_LEVELS, smoothness).to_linear();

    // 2. Bilinear Interpolation
    // First, blend the humidity axis (dry -> wet) for both hot and cold extremes
    let cold_blend = grass_color.mix(&taiga_color, humidity); 
    let hot_blend = desert_color.mix(&forest_color, humidity);  

    // Finally, blend between those two results along the temperature axis (cold -> hot)
    let final_color = cold_blend.mix(&hot_blend, temp);

    final_color.to_f32_array()
}
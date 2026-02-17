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
    Ocean,
}

#[derive(Component)]
pub struct Cloud {
    pub origin: Vec3,
}

#[derive(Component)]
pub struct CloudTask {
    pub task: Task<Mesh>,
}

#[derive(Resource, Clone)]
pub struct WorldGenerator {
    pub terrain_layers: Vec<PerlinLayer>,
    pub temperature_layer: PerlinLayer,
    pub humidity_layer: PerlinLayer,
    pub cloud_layer: PerlinLayer,
}

impl WorldGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            terrain_layers: vec![
                PerlinLayer::new(seed,       0.15, 4.0),    
                PerlinLayer::new(seed,       0.20, 3.5),      
                PerlinLayer::new(seed + 100, 0.5, 1.75), 
                PerlinLayer::new(seed + 200, 1.0, 0.5),  
                PerlinLayer::new(seed + 300, 2.0, 0.4),  
            ],
            // Note: Temperature and humidity need to be broad, so keep scales low!
            temperature_layer: PerlinLayer::new(seed + 400, 0.03, 1.0),
            humidity_layer: PerlinLayer::new(seed + 500, 0.03, 1.0),
            cloud_layer: PerlinLayer::new(seed + 600, CLOUD_SCALE * 1000.0, 1.0),
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

        // If it's extremely wet, call it Ocean regardless of temp
        if hum_normalized > 0.75 {
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

    pub fn get_3d_level(&self, pos: &[f32; 3]) -> f32 {
        let val = self.perlin
            .get([
                (pos[0] * self.horizontal_scale / 1000.0) as f64 + self.offset,
                (pos[1] * self.horizontal_scale / 1000.0) as f64 + self.offset,
                (pos[2] * self.horizontal_scale / 1000.0) as f64 + (self.offset.sqrt() + 202994.0)
            ]
        ) as f32;
        val * self.vertical_scale
    }
}
fn get_biome_elevation_offset(temp: f32, humidity: f32) -> f32 {
    let desert_elev = 0.0;    
    let grass_elev = 0.1;     
    let forest_elev = 0.3;    
    let taiga_elev = 3.0;     
    let ocean_elev = -2.5; // Deep negative offset

    // 1. Blend humidity
    let cold_blend = grass_elev + (taiga_elev - grass_elev) * humidity;
    let hot_blend = desert_elev + (forest_elev - desert_elev) * humidity;
    let base_land_elev = cold_blend + (hot_blend - cold_blend) * temp;

    // 2. Blend toward ocean if humidity is high (0.7 -> 1.0 range)
    if humidity > 0.7 {
        let t = ((humidity - 0.7) / 0.3).clamp(0.0, 1.0);
        base_land_elev + (ocean_elev - base_land_elev) * t
    } else {
        base_land_elev
    }
}

fn get_biome_height_multiplier(temp: f32, humidity: f32) -> f32 {
    let desert_mult = 0.025;
    let grass_mult = 0.2;
    let forest_mult = 0.25;
    let taiga_mult = 1.5;
    let ocean_mult = 0.05; // Oceans are relatively flat at the bottom

    let cold_blend = grass_mult + (taiga_mult - grass_mult) * humidity;
    let hot_blend = desert_mult + (forest_mult - desert_mult) * humidity;
    let base_land_mult = cold_blend + (hot_blend - cold_blend) * temp;

    if humidity > 0.7 {
        let t = ((humidity - 0.7) / 0.3).clamp(0.0, 1.0);
        base_land_mult + (ocean_mult - base_land_mult) * t
    } else {
        base_land_mult
    }
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

            commands.entity(entity).insert(ChunkTask { task, new_handle: None });
        }
    }
}

// Helper for deterministic random numbers
fn pseudo_random(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393);
    h = h.wrapping_add((z as u32).wrapping_mul(668265263));
    h = h.wrapping_add(seed);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h as f32) / (u32::MAX as f32)
}

pub fn modify_clouds(
    mut commands: Commands,
    query: Query<(Entity, &Mesh3d, &Cloud), Added<Cloud>>,
    world_generator: Res<WorldGenerator>,
    meshes: Res<Assets<Mesh>>,
) {
    let thread_pool = AsyncComputeTaskPool::get();
    for (entity, mesh_handle, cloud) in &query {
        if let Some(mesh) = meshes.get(mesh_handle) {
            let mut mesh_clone = mesh.clone();
            let world_gen = world_generator.clone();
            let cloud_origin = cloud.origin;

            let task = thread_pool.spawn(async move {
                // 1. Extract normals first (immutable borrow)
                let normals: Vec<[f32; 3]> = if let Some(VertexAttributeValues::Float32x3(normals)) = 
                    mesh_clone.attribute(Mesh::ATTRIBUTE_NORMAL) 
                {
                    normals.clone()
                } else {
                    Vec::new()
                };

                // 2. Modify positions (mutable borrow)
                if !normals.is_empty() {
                    if let Some(VertexAttributeValues::Float32x3(positions)) = 
                        mesh_clone.attribute_mut(Mesh::ATTRIBUTE_POSITION) 
                    {
                        for (i, pos) in positions.iter_mut().enumerate() {
                            if i >= normals.len() { break; }
                            
                            let world_pos = [
                                pos[0] + cloud_origin.x,
                                pos[1] + cloud_origin.y,
                                pos[2] + cloud_origin.z,
                            ];

                            // 3D Noise for deformation
                            let noise_val = world_gen.cloud_layer.get_3d_level(&world_pos);
                            
                            // Displace along normal
                            let displacement = noise_val * CLOUD_DISPLACEMENT;
                            
                            let normal = normals[i];
                            pos[0] += normal[0] * displacement;
                            pos[1] += normal[1] * displacement;
                            pos[2] += normal[2] * displacement;
                        }
                    }
                }
                
                mesh_clone.duplicate_vertices();
                mesh_clone.compute_flat_normals();
                bevy::camera::primitives::MeshAabb::compute_aabb(&mesh_clone);
                mesh_clone
            });

            commands.entity(entity).insert(CloudTask { task });
        }
    }
}

pub fn handle_cloud_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &Mesh3d, &mut CloudTask)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mesh_handle, mut task) in &mut tasks {
        if let Some(new_mesh) = future::block_on(future::poll_once(&mut task.task)) {
            // Update the mesh in-place
            if let Some(mesh) = meshes.get_mut(mesh_handle) {
                *mesh = new_mesh;
            }
            commands.entity(entity).remove::<CloudTask>();
        }
    }
}

pub fn handle_compute_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &Mesh3d, &mut ChunkTask)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mesh_handle, mut task) in &mut tasks {
        if let Some(new_mesh) = future::block_on(future::poll_once(&mut task.task)) {
            if let Some(new_handle) = task.new_handle.take() {
                // LOD update case: Update the NEW mesh and then swap it onto the entity
                if let Some(mesh) = meshes.get_mut(&new_handle) {
                    *mesh = new_mesh;
                }
                commands.entity(entity).insert(Mesh3d(new_handle));
                
                // Now we can safely remove the old mesh
                meshes.remove(mesh_handle);
            } else {
                // Initial generation case: Update the current mesh in-place
                if let Some(mesh) = meshes.get_mut(mesh_handle) {
                    *mesh = new_mesh;
                }
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
    current_lod: u32,
}

#[derive(Resource, Default)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<(i32, i32)>,
    pub last_camera_chunk: Option<(i32, i32)>,
    pub to_spawn: Vec<(i32, i32)>,
    pub lod_to_update: Vec<Entity>,
}

#[derive(Resource)]
pub struct SharedChunkMaterials {
    pub terrain_material: Handle<StandardMaterial>,
    pub water_material: Handle<StandardMaterial>,
    pub cloud_material: Handle<StandardMaterial>,
}

fn get_lod_subdivisions(distance_sq: f32) -> u32 {
    let distance = distance_sq.sqrt();
    
    for (max_distance, subdivisions) in LOD_LEVELS.iter() {
        if distance <= *max_distance {
            return *subdivisions;
        }
    }
    
    LOD_LEVELS.last().unwrap().1
}

pub fn generate_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    shared_materials: Res<SharedChunkMaterials>,
    mut chunk_manager: ResMut<ChunkManager>,
    camera: Query<&Transform, With<Camera>>,
    world_generator: Res<WorldGenerator>,
) {
    let cam_transform = camera.single().unwrap().translation;
    
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    // Only re-scan if the camera has moved to a new chunk or we haven't scanned yet
    if chunk_manager.last_camera_chunk != Some((cam_x, cam_z)) {
        chunk_manager.last_camera_chunk = Some((cam_x, cam_z));
        
        let render_distance_sq = (RENDER_DISTANCE as f32).powi(2);
        chunk_manager.to_spawn.clear();

        for x in (cam_x - RENDER_DISTANCE)..=(cam_x + RENDER_DISTANCE) {
            for z in (cam_z - RENDER_DISTANCE)..=(cam_z + RENDER_DISTANCE) {
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
    }

    // Spawn a limited number of chunks from the queue
    let mut spawned_count = 0;
    while spawned_count < MAX_CHUNKS_PER_FRAME && !chunk_manager.to_spawn.is_empty() {
        let (x, z) = chunk_manager.to_spawn.remove(0);
        
        // Final check: Is it still within range and not already spawned?
        let dx = (x - cam_x) as f32;
        let dz = (z - cam_z) as f32;
        let distance_sq = dx * dx + dz * dz;
        let render_distance_sq = (RENDER_DISTANCE as f32).powi(2);

        if distance_sq <= render_distance_sq && !chunk_manager.spawned_chunks.contains(&(x, z)) {
            chunk_manager.spawned_chunks.insert((x, z));
            
            let x_pos = x as f32 * CHUNK_SIZE;
            let z_pos = z as f32 * CHUNK_SIZE;
            let lod = get_lod_subdivisions(distance_sq);
            
            commands.spawn((
                Mesh3d(meshes.add(
                    Plane3d::default().mesh()
                    .size(CHUNK_SIZE, CHUNK_SIZE)
                    .subdivisions(lod)
                )),
                MeshMaterial3d(shared_materials.terrain_material.clone()),
                Transform::from_xyz(x_pos, 0.0, z_pos),
                Chunk { x, z, current_lod: lod },
            )).with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(Plane3d::default().mesh().size(CHUNK_SIZE, CHUNK_SIZE))),
                    MeshMaterial3d(shared_materials.water_material.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                ));

                // Cloud Spawning Logic
                for i in 0..CLOUDS_PER_CHUNK {
                    let seed = (x as u32).wrapping_mul(1000).wrapping_add((z as u32).wrapping_mul(100)).wrapping_add(i);
                    let rx = pseudo_random(x, z, seed);
                    let rz = pseudo_random(x, z, seed + 1);
                    
                    // Random position within chunk
                    let cloud_x = (rx - 0.5) * CHUNK_SIZE + x_pos;
                    let cloud_z = (rz - 0.5) * CHUNK_SIZE + z_pos;
                    
                    // Use humidity for cloud placement!
                    let humidity_val = world_generator.humidity_layer.get_level(&[cloud_x, 0.0, cloud_z]);
                    
                    // Normalize humidity roughly (it's -1 to 1)
                    // If humidity > 0.0 (50% coverage), spawn clouds.
                    // Added jitter to seed to avoid grid artifacts
                    if humidity_val > -0.2 && pseudo_random(x + i as i32, z - i as i32, seed + 2) > 0.3 {
                        let ry = pseudo_random(x, z, seed + 3);
                        let cloud_y = CLOUD_HEIGHT_RANGE.0 + ry * (CLOUD_HEIGHT_RANGE.1 - CLOUD_HEIGHT_RANGE.0);
                        
                        let size_factor = pseudo_random(x, z, seed + 4);
                        let cloud_size = CLOUD_SIZE_RANGE.0 + size_factor * (CLOUD_SIZE_RANGE.1 - CLOUD_SIZE_RANGE.0);

                        // Random rotation and scale
                        let rot_x = pseudo_random(x, z, seed + 5) * 3.14;
                        let rot_y = pseudo_random(x, z, seed + 6) * 3.14;
                        let scale_x = 0.8 + pseudo_random(x, z, seed + 7) * 0.4;
                        let scale_y = 0.5 + pseudo_random(x, z, seed + 8) * 0.5; // Flatter clouds
                        let scale_z = 0.8 + pseudo_random(x, z, seed + 9) * 0.4;

                        // Note: Transform is relative to parent (Chunk), so we need relative coordinates
                        let rel_x = cloud_x - x_pos;
                        let rel_z = cloud_z - z_pos;

                        parent.spawn((
                            Mesh3d(meshes.add(Sphere::new(cloud_size).mesh().ico(2).unwrap())), 
                            MeshMaterial3d(shared_materials.cloud_material.clone()),
                            Transform::from_xyz(rel_x, cloud_y, rel_z)
                                .with_rotation(Quat::from_euler(EulerRot::XYZ, rot_x, rot_y, 0.0))
                                .with_scale(Vec3::new(scale_x, scale_y, scale_z)),
                            Cloud { origin: Vec3::new(cloud_x, cloud_y, cloud_z) },
                        ));
                    }
                }
            });
            spawned_count += 1;
        }
    }
}

pub fn update_chunk_lod(
    mut commands: Commands,
    camera: Query<&Transform, With<Camera>>,
    mut chunks: Query<(Entity, &mut Chunk, &Mesh3d, &Transform), Without<ChunkTask>>,
    mut meshes: ResMut<Assets<Mesh>>,
    world_generator: Res<WorldGenerator>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut last_cam_pos: Local<Option<(i32, i32)>>,
) {
    let cam_transform = camera.single().unwrap().translation;
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;

    // Only re-scan all chunks for LOD changes when the camera moves to a new chunk
    if *last_cam_pos != Some((cam_x, cam_z)) {
        *last_cam_pos = Some((cam_x, cam_z));
        
        let mut candidates = Vec::new();
        for (entity, chunk, _, _) in &chunks {
            let dx = (chunk.x - cam_x) as f32;
            let dz = (chunk.z - cam_z) as f32;
            let distance_sq = dx * dx + dz * dz;
            let desired_lod = get_lod_subdivisions(distance_sq);
            
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
    while processed_count < MAX_CHUNKS_PER_FRAME && !chunk_manager.lod_to_update.is_empty() {
        let entity = chunk_manager.lod_to_update.remove(0);

        if let Ok((entity, chunk, _mesh_handle, transform)) = chunks.get_mut(entity) {
            let dx = (chunk.x - cam_x) as f32;
            let dz = (chunk.z - cam_z) as f32;
            let distance_sq = dx * dx + dz * dz;
            let desired_lod = get_lod_subdivisions(distance_sq);

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

                commands.entity(entity).insert(ChunkTask { task, new_handle: Some(new_mesh_handle) });
                
                // Finalize the chunk's LOD state
                if let Ok((_, mut chunk, _, _)) = chunks.get_mut(entity) {
                    chunk.current_lod = desired_lod;
                }
                processed_count += 1;
            }
        }
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

    for (entity, x, z, _) in chunks_to_despawn.iter().take(MAX_CHUNKS_PER_FRAME * 2) {
        commands.entity(*entity).despawn();
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
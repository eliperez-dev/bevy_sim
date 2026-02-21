use bevy::prelude::*;
use noise::{NoiseFn, Perlin};

use crate::world_generation::{WorldGenerator, Chunk, ChunkTask, Biome};
use crate::controls::MainCamera;
use crate::consts::CHUNK_SIZE;

#[derive(Component)]
pub struct VegetationSpawner;

#[derive(Component)]
pub struct Tree;

const TREE_DENSITY: f32 = 0.5;
const TREE_SPACING_GRID_SIZE: f32 = 270.0;

pub fn spawn_vegetation_for_chunk(
    mut commands: Commands,
    chunks: Query<(Entity, &Chunk, &Transform), (Without<VegetationSpawner>, Without<ChunkTask>)>,
    world_generator: Res<WorldGenerator>,
    chunk_manager: Res<crate::world_generation::ChunkManager>,
    asset_server: Res<AssetServer>,
    camera: Query<&Transform, With<MainCamera>>,
) {
    let tree_noise = Perlin::new(world_generator.seed + 9999);
    let density_noise = Perlin::new(world_generator.seed + 7777);
    let cam_transform = camera.single().unwrap().translation;
    let cam_x = (cam_transform.x / CHUNK_SIZE).round() as i32;
    let cam_z = (cam_transform.z / CHUNK_SIZE).round() as i32;
    
    for (chunk_entity, chunk, chunk_transform) in chunks.iter() {
        let dx = (chunk.x - cam_x) as f32;
        let dz = (chunk.z - cam_z) as f32;
        let distance = (dx * dx + dz * dz).sqrt();
        
        if distance > chunk_manager.tree_render_distance {
            continue;
        }
        let chunk_world_pos = chunk_transform.translation;
        let mut tree_spawns = Vec::new();
        
        let chunk_min_x = chunk_world_pos.x - CHUNK_SIZE / 2.0;
        let chunk_max_x = chunk_world_pos.x + CHUNK_SIZE / 2.0;
        let chunk_min_z = chunk_world_pos.z - CHUNK_SIZE / 2.0;
        let chunk_max_z = chunk_world_pos.z + CHUNK_SIZE / 2.0;
        
        let grid_min_x = (chunk_min_x / TREE_SPACING_GRID_SIZE).floor() as i32;
        let grid_max_x = (chunk_max_x / TREE_SPACING_GRID_SIZE).ceil() as i32;
        let grid_min_z = (chunk_min_z / TREE_SPACING_GRID_SIZE).floor() as i32;
        let grid_max_z = (chunk_max_z / TREE_SPACING_GRID_SIZE).ceil() as i32;
        
        for grid_x in grid_min_x..grid_max_x {
            for grid_z in grid_min_z..grid_max_z {
                let grid_world_x = grid_x as f64 * TREE_SPACING_GRID_SIZE as f64;
                let grid_world_z = grid_z as f64 * TREE_SPACING_GRID_SIZE as f64;
                
                let offset_x = (tree_noise.get([
                    grid_world_x * 0.01,
                    grid_world_z * 0.01,
                    137.5
                ]) as f32 * 0.5 + 0.5) * TREE_SPACING_GRID_SIZE;
                
                let offset_z = (tree_noise.get([
                    grid_world_x * 0.01,
                    grid_world_z * 0.01,
                    742.3
                ]) as f32 * 0.5 + 0.5) * TREE_SPACING_GRID_SIZE;
                
                let world_x = grid_world_x as f32 + offset_x;
                let world_z = grid_world_z as f32 + offset_z;
                
                if world_x < chunk_min_x || world_x > chunk_max_x ||
                   world_z < chunk_min_z || world_z > chunk_max_z {
                    continue;
                }
                
                let world_pos = [world_x, 0.0, world_z];
                let biome = world_generator.get_biome(&world_pos);
                
                let (tree_model, biome_scale_multiplier, biome_density) = match biome {
                    Biome::Forest => (Some("pine.glb#Scene0"), 0.7, TREE_DENSITY * 1.2),
                    Biome::Taiga => (Some("pine.glb#Scene0"), 1.2, TREE_DENSITY),
                    Biome::Grasslands => (Some("oak.glb#Scene0"), 0.8, TREE_DENSITY * 1.4),
                    _ => (None, 1.0, TREE_DENSITY),
                };
                
                let density_sample = density_noise.get([
                    grid_world_x * 0.01,
                    grid_world_z * 0.01,
                ]) as f32;
                
                if density_sample < (biome_density * 2.0 - 1.0) {
                    continue;
                }
                
                if let Some(mut model_path) = tree_model {
                    let terrain_height = world_generator.get_terrain_height(&world_pos);
                    
                    if terrain_height > 0.0 {
                        let dead_tree_chance = tree_noise.get([
                            world_x as f64 * 0.19,
                            world_z as f64 * 0.19,
                            555.0
                        ]) as f32;

                        let is_dead_tree = dead_tree_chance.abs() < 0.04;
                        
                        if is_dead_tree {
                            model_path = "dead_tree.glb#Scene0";
                        }
                        
                        let rotation_y = (tree_noise.get([
                            world_x as f64 * 0.33,
                            world_z as f64 * 0.33,
                            999.0
                        ]) as f32 * 0.5 + 0.5) * std::f32::consts::TAU;
                        
                        let base_scale = 0.4 + (tree_noise.get([
                            world_x as f64 * 0.27,
                            world_z as f64 * 0.27,
                            123.0
                        ]) as f32 * 0.5 + 0.5) * 1.2;
                        
                        let scale = base_scale * biome_scale_multiplier * if is_dead_tree && 
                        (biome == Biome::Taiga || biome == Biome::Forest) { 1.3 } else { 1.0 };
                        
                        let local_x = world_x - chunk_world_pos.x;
                        let local_z = world_z - chunk_world_pos.z;
                        
                        tree_spawns.push((
                            model_path,
                            Vec3::new(local_x, terrain_height, local_z),
                            rotation_y,
                            scale,
                        ));
                    }
                }
            }
        }
   
        commands.entity(chunk_entity).with_children(|parent| {
            for (model_path, position, rotation_y, scale) in tree_spawns {
                parent.spawn((
                    SceneRoot(asset_server.load(model_path)),
                    Transform::from_translation(position)
                        .with_rotation(Quat::from_rotation_y(rotation_y))
                        .with_scale(Vec3::splat(scale * 40.0)),
                    Tree,
                    Visibility::Hidden,
                ));
            }
        });
        
        commands.entity(chunk_entity).insert(VegetationSpawner);
    }
}

pub fn update_tree_lod(
    chunk_manager: Res<crate::world_generation::ChunkManager>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut trees: Query<(&GlobalTransform, &mut Visibility), With<Tree>>,
) {
    let Ok(cam_transform) = camera.single() else { return };
    let cam_pos = cam_transform.translation();
    
    for (tree_transform, mut visibility) in trees.iter_mut() {
        let tree_pos = tree_transform.translation();
        let distance = cam_pos.distance(tree_pos);
        
        if distance > chunk_manager.tree_render_distance * CHUNK_SIZE {
            *visibility = Visibility::Hidden;
        } else {
            *visibility = Visibility::Inherited;
        }
    }
}

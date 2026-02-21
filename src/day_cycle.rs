use bevy::prelude::*;
use crate::{consts::*, world_generation::ChunkManager, controls::MainCamera};

#[derive(Resource)]
pub struct DayNightCycle {
    pub time_of_day: f32,
    pub speed: f32, 
    pub inclination: f32,
}

#[derive(Component)]
pub struct Sun;

#[derive(Component)]
pub struct Star {
    pub offset: Vec3,
    pub brightness: f32,
    pub phase: f32,         // Random starting point for the sine wave
    pub twinkle_speed: f32, // How fast this specific star flickers
}

pub fn update_daylight_cycle(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
    mut clear_color: ResMut<ClearColor>,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight), (With<Sun>, Without<MainCamera>)>,
    mut env_query: Query<(&mut DistanceFog, &mut AmbientLight)>, 
    camera_query: Query<&Transform, (With<MainCamera>, Without<Sun>)>,
    chunk_manager: Res<ChunkManager>,
    mut star_query: Query<(&Star, &mut Transform, &MeshMaterial3d<StandardMaterial>), (Without<MainCamera>, Without<Sun>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    cycle.time_of_day = (cycle.time_of_day + cycle.speed * time.delta_secs()) % 1.0;

    let angle = cycle.time_of_day * std::f32::consts::TAU;
    let orbit_rotation = Quat::from_rotation_x(angle);
    let tilt_rotation = Quat::from_rotation_z(cycle.inclination);
    
    let final_rotation = tilt_rotation * orbit_rotation;
    let sun_dir = final_rotation.mul_vec3(Vec3::NEG_Z);
    let up_dot = sun_dir.dot(Vec3::NEG_Y);
    
    let daylight = ((up_dot + 0.1) * 5.0).clamp(0.0, 1.0);
        
    if let Ok((mut transform, mut light)) = sun_query.single_mut() {
        transform.rotation = final_rotation; 
        
        let sunset_horizon_factor = (1.0 - (up_dot.abs() / 0.34)).clamp(0.0, 1.0);
        //let star_horizon_factor = (1.0 - (up_dot.abs() / 0.24)).clamp(0.0, 1.0);
        light.illuminance = daylight * MAX_ILLUMANENCE;

        if let Ok((mut fog, mut ambient)) = env_query.single_mut() {
            let min_ambient = 70.0;
            let max_ambient = 180.0;
            ambient.brightness = min_ambient + (max_ambient - min_ambient) * daylight; 

            let night_fog = Vec3::new(0.1, 0.1, 0.2);
            let day_fog = Vec3::new(0.35, 0.48, 0.66);
            let sunset_fog = Vec3::new(0.90, 0.45, 0.2);
            
            let base_fog = night_fog.lerp(day_fog, daylight);
            let current_fog = base_fog.lerp(sunset_fog, sunset_horizon_factor);
            
            let final_color = Color::srgb(current_fog.x, current_fog.y, current_fog.z);
            fog.color = final_color;
            clear_color.0 = final_color; 
            fog.directional_light_color = Color::NONE; 
        }

        if let Ok(camera_transform) = camera_query.single() {
            transform.translation = camera_transform.translation - sun_dir * CHUNK_SIZE * chunk_manager.render_distance as f32;
            let scale_factor = 0.1 * chunk_manager.render_distance as f32;
            transform.scale = Vec3::splat(scale_factor);
        }
    }

    if let Ok(camera_transform) = camera_query.single() {
        let global_star_visibility = ((-up_dot - 0.2) / 0.6).clamp(0.0, 1.0);
        
        let star_distance = CHUNK_SIZE * chunk_manager.render_distance as f32;
        let scale_factor = 0.2 * chunk_manager.render_distance as f32;
        let base_star_brightness = 10.0; 

        if global_star_visibility > 0.0 {
            for (star, mut star_transform, material_handle) in star_query.iter_mut() {
                let star_rotation = final_rotation * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let current_star_dir = star_rotation.mul_vec3(-star.offset).normalize(); 
                
                let star_elevation = current_star_dir.dot(Vec3::Y);
                let horizon_fade = ((star_elevation + 0.05) / 0.35).clamp(0.0, 1.0);
                let final_alpha = global_star_visibility * horizon_fade;

                if final_alpha > 0.001 {
                    star_transform.translation = camera_transform.translation + current_star_dir * star_distance;

                    let t = time.elapsed_secs() * star.twinkle_speed;
                    let wave1 = (t + star.phase).sin();
                    let wave2 = (t * 2.7 + star.phase * 1.5).sin();
                    let noise = (wave1 * 0.7) + (wave2 * 0.3);
                    let twinkle = noise * 0.4 + 1.0;

                    star_transform.scale = Vec3::splat(scale_factor * star.brightness * (twinkle * 0.2 + 0.8));

                    if let Some(material) = materials.get_mut(material_handle) {
                        let final_brightness = base_star_brightness * star.brightness * twinkle;
                        
                        material.base_color = Color::srgba(1.0, 1.0, 1.0, final_alpha);
                        material.emissive = LinearRgba::rgb(
                            final_brightness * final_alpha, 
                            final_brightness * final_alpha, 
                            final_brightness * final_alpha
                        );
                    }
                } else {
                    star_transform.scale = Vec3::ZERO;
                }
            }
        } else {
            for (_, mut star_transform, _) in star_query.iter_mut() {
                star_transform.scale = Vec3::ZERO;
            }
        }
    }
}

pub fn spawn_stars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let num_stars = 1000;

    for _ in 0..num_stars {
        let phi = rand::random::<f32>() * std::f32::consts::TAU;
        let theta = rand::random::<f32>() * std::f32::consts::PI;
        
        let x = theta.sin() * phi.cos();
        let y = theta.cos();
        let z = theta.sin() * phi.sin();

        let offset = Vec3::new(x, y, z).normalize();
        let brightness = 0.7 + rand::random::<f32>() * 0.5;
        
        // Randomize the twinkle parameters per-star
        let phase = rand::random::<f32>() * std::f32::consts::TAU;
        let twinkle_speed = 3.0 + rand::random::<f32>() * 4.0;

        let star_mesh = meshes.add(Sphere::new(4.0 + brightness * 2.0));

        let star_material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.0),
            emissive: LinearRgba::rgb(10.1, 10.1, 10.1),
            alpha_mode: AlphaMode::Blend,
            fog_enabled: false,
            ..default()
        });

        commands.spawn((
            Mesh3d(star_mesh),
            MeshMaterial3d(star_material),
            Transform::default(),
            Star { 
                offset, 
                brightness, 
                phase, 
                twinkle_speed 
            },
        ));
    }
}
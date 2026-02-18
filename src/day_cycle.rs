
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

pub fn update_daylight_cycle(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
    mut clear_color: ResMut<ClearColor>,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight), (With<Sun>, Without<MainCamera>)>,
    mut env_query: Query<(&mut DistanceFog, &mut AmbientLight)>, 
    camera_query: Query<&Transform, (With<MainCamera>, Without<Sun>)>,
    chunk_manager: Res<ChunkManager>,
) {
    cycle.time_of_day = (cycle.time_of_day + cycle.speed * time.delta_secs()) % 1.0;


    // Convert 0.0-1.0 time to radians
    let angle = cycle.time_of_day * core::f32::consts::TAU;

    // 1. Time of day rotation (The sun's progress through the sky)
    let orbit_rotation = Quat::from_rotation_x(angle);

    // 2. Inclination rotation (The tilt based on latitude)
    // Rotating around Z tips the "noon" position away from the vertical (Y) axis
    let tilt_rotation = Quat::from_rotation_z(cycle.inclination);
        

    if let Ok((mut transform, mut light)) = sun_query.single_mut() {

        transform.rotation = tilt_rotation * orbit_rotation;

        let sun_dir = transform.forward().as_vec3();
        
        // Dot product with straight down. 1.0 = noon, 0.0 = horizon, -1.0 = midnight
        let up_dot = sun_dir.dot(Vec3::NEG_Y); 

        // We add 0.1 so dawn starts just below the horizon, and multiply by 5.0 to make the fade faster.
        let daylight = ((up_dot + 0.1) * 5.0).clamp(0.0, 1.0); 
        
        // NEW: Horizon factor. Peaks at 1.0 when up_dot is 0.0, fades to 0.0 by +/- 0.25
        // Tweak the 0.25 to make the sunset phase last longer or shorter.
        let horizon_factor = (1.0 - (up_dot.abs() / 0.40)).clamp(0.0, 1.0);

        // Apply lighting
        light.illuminance = daylight * MAX_ILLUMANENCE;

        // Update Environment
        if let Ok((mut fog, mut ambient)) = env_query.single_mut() {
            ambient.brightness = 10.0 + (daylight * 9.5); 

            // Simple Fog Color Lerp (using Vec3 for universally safe math)
            let night_fog = Vec3::new(0.1, 0.1, 0.2);
            let day_fog = Vec3::new(0.35, 0.48, 0.66);
            let sunset_fog = Vec3::new(0.90, 0.45, 0.2); // An orange/pinkish hue
            
            // 1. Interpolate between night and day like normal
            let base_fog = night_fog.lerp(day_fog, daylight);
            // 2. Blend in the sunset color based on the horizon factor
            let current_fog = base_fog.lerp(sunset_fog, horizon_factor);
            
            let final_color = Color::srgb(current_fog.x, current_fog.y, current_fog.z);
        
            fog.color = final_color;
            clear_color.0 = final_color; 
            
            fog.directional_light_color = Color::NONE; 
        }

        // Keep sun mesh far away in the background
        if let Ok(camera_transform) = camera_query.single() {
            transform.translation = camera_transform.translation - sun_dir * CHUNK_SIZE * chunk_manager.render_distance as f32;
            let scale_factor = 0.1 * chunk_manager.render_distance as f32;
            transform.scale = Vec3::splat(scale_factor);
        }
    }
}
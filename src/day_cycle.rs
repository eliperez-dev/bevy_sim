
use bevy::prelude::*;

use crate::consts::*;

#[derive(Resource)]
pub struct DayNightCycle {
    pub time_of_day: f32,
    pub speed: f32, 
}

#[derive(Component)]
pub struct Sun;


pub fn update_daylight_cycle(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
    mut clear_color: ResMut<ClearColor>,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut env_query: Query<(&mut DistanceFog, &mut AmbientLight)>, 
    camera_query: Query<&Transform, (With<Camera>, Without<Sun>)>,
) {
    cycle.time_of_day = (cycle.time_of_day + cycle.speed * time.delta_secs()) % 1.0;
    
    // Convert 0.0-1.0 time to radians
    let angle = cycle.time_of_day * core::f32::consts::TAU - core::f32::consts::FRAC_PI_2;

    if let Ok((mut transform, mut light)) = sun_query.single_mut() {
        transform.rotation = Quat::from_rotation_x(angle) * Quat::from_rotation_y(0.5);
        let sun_dir = transform.forward().as_vec3();
        
        // Dot product with straight down. 1.0 = noon, 0.0 = horizon, -1.0 = midnight
        let up_dot = sun_dir.dot(Vec3::NEG_Y); 

        // We add 0.1 so dawn starts just below the horizon, and multiply by 5.0 to make the fade faster.
        let daylight = ((up_dot + 0.1) * 5.0).clamp(0.0, 1.0); 

        // Apply lighting
        light.illuminance = daylight * MAX_ILLUMANENCE;

        // 3. Update Environment
        if let Ok((mut fog, mut ambient)) = env_query.single_mut() {
            // Ambient light interpolates between 0.2 (Night) and 10.0 (Day)
            ambient.brightness = 5.5 + (daylight * 9.8); 

            // Simple Fog Color Lerp (using Vec3 for universally safe math)
            let night_fog = Vec3::new(0.1, 0.1, 0.2);
            let day_fog = Vec3::new(0.35, 0.48, 0.66);
            let current_fog = night_fog.lerp(day_fog, daylight);
            
            let final_color = Color::srgb(current_fog.x, current_fog.y, current_fog.z);
        
            fog.color = final_color;
            clear_color.0 = final_color; 
            
            // FIX THE MOUNTAIN CLIPPING: 
            // Setting this to entirely transparent turns off the fake screen-space glare.
            fog.directional_light_color = Color::NONE; 
        }

        // 4. Keep sun mesh far away in the background
        if let Ok(camera_transform) = camera_query.single() {
            transform.translation = camera_transform.translation - sun_dir * CHUNK_SIZE * RENDER_DISTANCE as f32;
        }
    }
}
use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*
};

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct Aircraft {
    pub speed: f32
}

#[derive(Resource)]
pub struct ControlMode {
    pub mode: FlightMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlightMode {
    FreeFlight,
    Aircraft,
}

impl Default for ControlMode {
    fn default() -> Self {
        Self {
            mode: FlightMode::FreeFlight,
        }
    }
}

pub fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut wire_frame: ResMut<WireframeConfig>,
    mut control_mode: ResMut<ControlMode>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    mut aircraft_query: Query<(&mut Transform, &mut Aircraft), (With<Aircraft>, Without<MainCamera>)>,
) {
    // Toggle control mode and wireframe
    if keyboard.just_pressed(KeyCode::KeyF) {
        control_mode.mode = match control_mode.mode {
            FlightMode::FreeFlight => FlightMode::Aircraft,
            FlightMode::Aircraft => FlightMode::FreeFlight,
        };
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        wire_frame.global = !wire_frame.global;
    }

    let rotation_speed = 0.5;
    let freeflight_pan_speed = match keyboard.pressed(KeyCode::ShiftLeft) {
        true => 2000.0,
        false => 300.0,
    };
    let panning_delta = rotation_speed * time.delta_secs();
    
    let Ok((mut plane_transform, mut aircraft)) = aircraft_query.single_mut() else { return };
            
    if keyboard.pressed(KeyCode::Equal) {
        aircraft.speed = (aircraft.speed + 1000.0 * time.delta_secs()).min(250.0);
    } 

    if keyboard.pressed(KeyCode::Minus) {
        aircraft.speed =  (aircraft.speed - 1000.0 * time.delta_secs()).max(80.0);
    }

    let forward = plane_transform.forward().as_vec3();
    plane_transform.translation += forward * aircraft.speed * time.delta_secs();

    // Split logic based on the mode we are in
    match control_mode.mode {
        FlightMode::FreeFlight => {
            // --- FREE FLIGHT (FPS Style Strafing) ---
            let Ok(mut camera_transform) = camera_query.single_mut() else { return };
            
            let forward = camera_transform.forward().as_vec3();
            let right = camera_transform.right().as_vec3();
            let up = camera_transform.up().as_vec3();
            let mut pan_direction = Vec3::ZERO;

            if keyboard.pressed(KeyCode::KeyW) { pan_direction += forward; }
            if keyboard.pressed(KeyCode::KeyS) { pan_direction -= forward; }
            if keyboard.pressed(KeyCode::KeyA) { pan_direction -= right; }
            if keyboard.pressed(KeyCode::KeyD) { pan_direction += right; }
            if keyboard.pressed(KeyCode::KeyE) { pan_direction += up; }
            if keyboard.pressed(KeyCode::KeyQ) { pan_direction -= up; }

            if keyboard.pressed(KeyCode::ArrowLeft) { camera_transform.rotate_y(panning_delta); }
            if keyboard.pressed(KeyCode::ArrowRight) { camera_transform.rotate_y(-panning_delta); }
            if keyboard.pressed(KeyCode::ArrowUp) { camera_transform.rotate_local_x(panning_delta); }
            if keyboard.pressed(KeyCode::ArrowDown) { camera_transform.rotate_local_x(-panning_delta); }

            camera_transform.translation += pan_direction.normalize_or_zero() * freeflight_pan_speed* time.delta_secs();
        }
        FlightMode::Aircraft => {
            // --- AIRCRAFT MODE (Flight Sim Style) ---

            if keyboard.pressed(KeyCode::KeyQ) {
                plane_transform.rotate_y(panning_delta); // Yaw Left
            }
            if keyboard.pressed(KeyCode::KeyE) {
                plane_transform.rotate_y(-panning_delta); // Yaw Right
            }
            if  keyboard.pressed(KeyCode::KeyW) {
                plane_transform.rotate_local_x(-panning_delta); // Pitch Down
            }
            if keyboard.pressed(KeyCode::KeyS) {
                plane_transform.rotate_local_x(panning_delta); // Pitch Up
            }
            if  keyboard.pressed(KeyCode::KeyA) {
                plane_transform.rotate_local_z(panning_delta); // Roll Left
            }
            if keyboard.pressed(KeyCode::KeyD) {
                plane_transform.rotate_local_z(-panning_delta); // Roll Right
            }
        }
    }
}


pub fn camera_follow_aircraft(
    time: Res<Time>,
    control_mode: Res<ControlMode>,
    aircraft_query: Query<(&Transform, &Aircraft), (With<Aircraft>, Without<MainCamera>)>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    if control_mode.mode != FlightMode::Aircraft {
        return;
    }

    let Ok((plane_transform, aircraft)) = aircraft_query.single() else { return; };
    let Ok(mut camera_transform) = camera_query.single_mut() else { return; };

    // --- 1. VELOCITY COMPENSATION (The Fix) ---
    // Calculate how much the plane moved this frame.
    // We use the same math as the movement system: direction * speed * delta_time
    let plane_movement = plane_transform.forward().as_vec3() * aircraft.speed * time.delta_secs();
    
    // meaningful change: Move the camera by the plane's speed *before* smoothing.
    // This removes the "lag" caused by high speeds.
    camera_transform.translation += plane_movement;


    // --- 2. CONFIGURATION ---
    let base_distance = 18.0;
    let max_extra_dist = 4.0;
    let speed_threshold = 100.0;
    
    let height = 9.0;
    let look_ahead_distance = 15.0;
    let smoothness = 5.0; 

    // --- 3. CURVED DISTANCE CALCULATION ---
    let speed_ratio = (aircraft.speed / speed_threshold).clamp(0.0, 2.0);
    let dynamic_distance = base_distance + (max_extra_dist * speed_ratio.powf(0.5));

    let dynamic_smoothness = smoothness + (speed_ratio * 2.0);
    let t = (time.delta_secs() * dynamic_smoothness).min(1.0);

    // --- 4. CALCULATE TARGET POSITION ---
    let target_position = plane_transform.translation 
        + (plane_transform.back() * dynamic_distance) 
        + (plane_transform.up() * height);
    
    // Now we lerp. Since we already added 'plane_movement' above, 
    // this lerp only handles the offset/drift, not the 2000 mph speed.
    camera_transform.translation = camera_transform.translation.lerp(target_position, t);

    // --- 5. SMOOTH ROTATION ---
    let look_target = plane_transform.translation + (plane_transform.forward() * look_ahead_distance);
    let target_rotation = camera_transform.looking_at(look_target, plane_transform.up()).rotation;
    camera_transform.rotation = camera_transform.rotation.slerp(target_rotation, t);
}
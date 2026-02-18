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
    let panning_delta = rotation_speed * time.delta_secs();
    
    let Ok((mut plane_transform, mut aircraft)) = aircraft_query.single_mut() else { return };
            
    if keyboard.pressed(KeyCode::Equal) {
        aircraft.speed = (aircraft.speed + 1000.0 * time.delta_secs()).min(2000.0);
    } 

    if keyboard.pressed(KeyCode::Minus) {
        aircraft.speed =  (aircraft.speed - 1000.0 * time.delta_secs()).max(40.0);
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

            camera_transform.translation += pan_direction.normalize_or_zero() * aircraft.speed * time.delta_secs();
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

    let Ok((transform, aircraft)) = aircraft_query.single() else { return; };
    let Ok(mut camera) = camera_query.single_mut() else { return; };

    // --- CONFIGURATION ---
    let base_distance = 80.0;    // Minimum distance at low speed
    let max_extra_dist = 60.0;   // The most the camera can pull back
    let speed_threshold = 200.0; // The "anchor" speed for the curve
    
    let height = 80.0;
    let look_ahead_distance = 150.0;
    let smoothness = 5.0; 

    // --- CURVED DISTANCE CALCULATION ---
    // We calculate a ratio of current speed vs threshold.
    // Using a power of 0.5 (square root) creates a curve that flattens out.
    let speed_ratio = (aircraft.speed / speed_threshold).clamp(0.0, 2.0);
    let dynamic_distance = base_distance + (max_extra_dist * speed_ratio.powf(0.5));

    // Calculate dynamic smoothness: make the camera "snappier" at high speeds
    // to prevent it from feeling like it's dragging on a rubber band.
    let dynamic_smoothness = smoothness + (speed_ratio * 2.0);
    let t = (time.delta_secs() * dynamic_smoothness).min(1.0);

    // 1. CALCULATE TARGET POSITION
    let target_position = transform.translation 
        + (transform.back() * dynamic_distance) 
        + (transform.up() * height);
    
    camera.translation = camera.translation.lerp(target_position, t);

    // 2. CALCULATE "LOOK AHEAD" TARGET
    let look_target = transform.translation + (transform.forward() * look_ahead_distance);

    // 3. SMOOTH ROTATION
    let target_rotation = camera.looking_at(look_target, transform.up()).rotation;
    camera.rotation = camera.rotation.slerp(target_rotation, t);
}
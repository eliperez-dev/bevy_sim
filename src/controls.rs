use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*
};

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct Aircraft;

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
    mut aircraft_query: Query<&mut Transform, (With<Aircraft>, Without<MainCamera>)>,
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

    let mut pan_speed = 1230.0;
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::Tab) {
        pan_speed *= 15.0;
    }

    let rotation_speed = 1.0;
    let panning_delta = rotation_speed * time.delta_secs();

    let Ok(mut plane_transform) = aircraft_query.single_mut() else { return };
            
    // 1. Airplanes don't strafe; they always fly forward based on their speed!
    let forward = plane_transform.forward().as_vec3();
    plane_transform.translation += forward * pan_speed * time.delta_secs();

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

            camera_transform.translation += pan_direction.normalize_or_zero() * pan_speed * time.delta_secs();
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
    aircraft_query: Query<&Transform, (With<Aircraft>, Without<MainCamera>)>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    if control_mode.mode != FlightMode::Aircraft {
        return;
    }

    let Ok(aircraft) = aircraft_query.single() else { return; };
    let Ok(mut camera) = camera_query.single_mut() else { return; };

    // --- CONFIGURATION ---
    let distance_behind = 120.0; 
    let height = 80.0;
    let look_ahead_distance = 150.0; // How far in front of the nose to look
    let smoothness = 5.0; 
    let t = (time.delta_secs() * smoothness).min(1.0);

    // 1. CALCULATE TARGET POSITION (Remains the same)
    let target_position = aircraft.translation 
        + (aircraft.back() * distance_behind) 
        + (aircraft.up() * height);
    camera.translation = camera.translation.lerp(target_position, t);

    // 2. CALCULATE "LOOK AHEAD" TARGET
    // We take the plane's position and add its forward vector multiplied by distance
    let look_target = aircraft.translation + (aircraft.forward() * look_ahead_distance);

    // 3. SMOOTH ROTATION
    // Now we look at the 'look_target' instead of 'aircraft.translation'
    let target_rotation = camera.looking_at(look_target, aircraft.up()).rotation;
    camera.rotation = camera.rotation.slerp(target_rotation, t);
}
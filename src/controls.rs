use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*
};

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct Aircraft {
    /// Current actual speed (units per second)
    pub speed: f32,
    /// Target engine power (0.0 to 1.0)
    pub throttle: f32,
    
    // Rotational Velocity (radians per second)
    // We store these to create "momentum" in turns
    pub pitch_velocity: f32,
    pub roll_velocity: f32,
    pub yaw_velocity: f32,
}

impl Default for Aircraft {
    fn default() -> Self {
        Self {
            speed: 0.0,
            throttle: 0.0,
            pitch_velocity: 0.0,
            roll_velocity: 0.0,
            yaw_velocity: 0.0,
        }
    }
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
            mode: FlightMode::Aircraft,
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
    let dt = time.delta_secs();

    // --- Toggles ---
    if keyboard.just_pressed(KeyCode::KeyF) {
        control_mode.mode = match control_mode.mode {
            FlightMode::FreeFlight => FlightMode::Aircraft,
            FlightMode::Aircraft => FlightMode::FreeFlight,
        };
        info!("Switched to {:?}", control_mode.mode);
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        wire_frame.global = !wire_frame.global;
    }

    match control_mode.mode {
        FlightMode::FreeFlight => {
            // --- FREE FLIGHT (Standard FPS/Debug Cam) ---
            let Ok(mut camera_transform) = camera_query.single_mut() else { return };
            
            let rotation_speed = 0.5;
            let pan_speed = if keyboard.pressed(KeyCode::ShiftLeft) { 50.0 } else { 10.0 };
            
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

            // Camera Rotation
            let panning_delta = rotation_speed * dt;
            if keyboard.pressed(KeyCode::ArrowLeft) { camera_transform.rotate_y(panning_delta); }
            if keyboard.pressed(KeyCode::ArrowRight) { camera_transform.rotate_y(-panning_delta); }
            if keyboard.pressed(KeyCode::ArrowUp) { camera_transform.rotate_local_x(panning_delta); }
            if keyboard.pressed(KeyCode::ArrowDown) { camera_transform.rotate_local_x(-panning_delta); }

            camera_transform.translation += pan_direction.normalize_or_zero() * pan_speed * dt;
        }
        FlightMode::Aircraft => {
            // --- AIRCRAFT PHYSICS MODE ---
            let Ok((mut plane_transform, mut aircraft)) = aircraft_query.single_mut() else { return };

            // 1. ENGINE & DRAG
            // Controls adjust throttle, not speed directly
            if keyboard.pressed(KeyCode::Equal) {
                aircraft.throttle = (aircraft.throttle + 0.5 * dt).min(1.0);
            }
            if keyboard.pressed(KeyCode::Minus) {
                aircraft.throttle = (aircraft.throttle - 0.5 * dt).max(0.0);
            }

            // Physics Constants
            let max_speed = 250.0;
            let drag_factor = 0.5; // Higher = accelerates/decelerates faster
            
            // Calculate Target Speed
            let target_speed = aircraft.throttle * max_speed;
            
            // Lerp current speed to target (Simulates Inertia)
            aircraft.speed = aircraft.speed + (target_speed - aircraft.speed) * drag_factor * dt;

            // 2. AERODYNAMICS (Control Surfaces)
            // Surfaces are less effective at low speeds
            let airspeed_ratio = (aircraft.speed / max_speed).clamp(0.0, 1.0);
            
            // Responsiveness settings
            let pitch_strength = 2.0 * airspeed_ratio;
            let roll_strength = 3.0 * airspeed_ratio;
            let yaw_strength = 1.0 * airspeed_ratio;
            let rotational_damping = 2.0; // Air resistance stopping the rotation

            // Input adds Torque (changes velocity, doesn't rotate directly)
            if keyboard.pressed(KeyCode::KeyW) { aircraft.pitch_velocity -= pitch_strength * dt; }
            if keyboard.pressed(KeyCode::KeyS) { aircraft.pitch_velocity += pitch_strength * dt; }
            
            if keyboard.pressed(KeyCode::KeyA) { aircraft.roll_velocity += roll_strength * dt; }
            if keyboard.pressed(KeyCode::KeyD) { aircraft.roll_velocity -= roll_strength * dt; }
            
            if keyboard.pressed(KeyCode::KeyQ) { aircraft.yaw_velocity += yaw_strength * dt; }
            if keyboard.pressed(KeyCode::KeyE) { aircraft.yaw_velocity -= yaw_strength * dt; }

            // Apply Damping (Slow down rotation if no input)
            aircraft.pitch_velocity -= aircraft.pitch_velocity * rotational_damping * dt;
            aircraft.roll_velocity -= aircraft.roll_velocity * rotational_damping * dt;
            aircraft.yaw_velocity -= aircraft.yaw_velocity * rotational_damping * dt;

            // 3. APPLY TO TRANSFORM
            plane_transform.rotate_local_x(aircraft.pitch_velocity * dt);
            plane_transform.rotate_local_z(aircraft.roll_velocity * dt);
            plane_transform.rotate_local_y(aircraft.yaw_velocity * dt);

            // Move forward based on updated rotation
            let forward = plane_transform.forward().as_vec3();
            plane_transform.translation += forward * aircraft.speed * dt;
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

    // --- 1. VELOCITY COMPENSATION ---
    // Move the camera by the plane's exact velocity frame-by-frame 
    // BEFORE calculating the smooth drift. This eliminates "lag" at high speeds.
    let plane_movement = plane_transform.forward().as_vec3() * aircraft.speed * time.delta_secs();
    camera_transform.translation += plane_movement;

    // --- 2. SMOOTH FOLLOW LOGIC ---
    let base_distance = 18.0;
    let max_extra_dist = 4.0;
    let speed_threshold = 100.0;
    
    let height = 6.0;
    let look_ahead_distance = 20.0;
    
    // Calculate dynamic distance based on speed
    let speed_ratio = (aircraft.speed / speed_threshold).clamp(0.0, 2.0);
    let dynamic_distance = base_distance + (max_extra_dist * speed_ratio.powf(0.5));

    // Calculate smoothness (less smooth at high speeds to feel "tight")
    let smoothness = 5.0 + (speed_ratio * 2.0);
    let t = (time.delta_secs() * smoothness).min(1.0);

    // Calculate Ideal Target Position
    let target_position = plane_transform.translation 
        + (plane_transform.back() * dynamic_distance) 
        + (plane_transform.up() * height);
    
    // Lerp towards target (handles the sway/drift, but not the raw speed)
    camera_transform.translation = camera_transform.translation.lerp(target_position, t);

    // --- 3. LOOK AT LOGIC ---
    // Look slightly ahead of the plane
    let look_target = plane_transform.translation + (plane_transform.forward() * look_ahead_distance);
    let target_rotation = camera_transform.looking_at(look_target, plane_transform.up()).rotation;
    camera_transform.rotation = camera_transform.rotation.slerp(target_rotation, t);
}
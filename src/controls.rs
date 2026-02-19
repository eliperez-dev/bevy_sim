use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*,
};
use noise::{NoiseFn, Perlin};

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct Aircraft {
    // --- STATE ---
    pub speed: f32,
    pub throttle: f32,
    pub pitch_velocity: f32,
    pub roll_velocity: f32,
    pub yaw_velocity: f32,

    // --- TUNING ---
    pub max_speed: f32,
    pub max_throttle: f32,       // Maximum throttle (1.0 = 100%, 1.3 = 130%)
    pub drag_factor: f32,        // How fast engine reaches target speed
    pub gravity: f32,            // How much climbing slows you / diving speeds you up
    pub g_force_drag: f32,       // Speed loss when turning hard
    
    pub pitch_strength: f32,
    pub roll_strength: f32,
    pub yaw_strength: f32,
    
    pub bank_turn_strength: f32, // Auto-yaw when banking
    pub auto_level_strength: f32,// Stability
}

impl Default for Aircraft {
    fn default() -> Self {
        Self {
            speed: 150.0,
            throttle: 0.80,
            pitch_velocity: 0.0,
            roll_velocity: 0.0,
            yaw_velocity: 0.0,

            // Default Physics Values
            max_speed: 350.0,
            max_throttle: 1.5,
            drag_factor: 0.5,
            gravity: 150.0,       
            g_force_drag: 5.0,
            pitch_strength: 2.0,
            roll_strength: 3.0,
            yaw_strength: 1.0,
            bank_turn_strength: 0.85,
            auto_level_strength: 0.60,
        }
    }
}

#[derive(Resource)]
pub struct ControlMode {
    pub mode: FlightMode,
    pub physics_paused: bool,
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
            physics_paused: false,
        }
    }
}

#[derive(Resource)]
pub struct Wind {
    pub base_wind: Vec3,
    pub turbulence_intensity: f32,
    pub turbulence_frequency: f32,
    pub perlin: Perlin,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            base_wind: Vec3::new(10.0, 0.0, 5.0),
            turbulence_intensity: 0.003,
            turbulence_frequency: 4.0,
            perlin: Perlin::new(42),
        }
    }
}

pub fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut wire_frame: ResMut<WireframeConfig>,
    mut control_mode: ResMut<ControlMode>,
    wind: Res<Wind>,
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
    if keyboard.just_pressed(KeyCode::KeyP) {
        control_mode.physics_paused = !control_mode.physics_paused;
        info!("Physics {}", if control_mode.physics_paused { "paused" } else { "resumed" });
    }

    // --- AIRCRAFT PHYSICS (Always runs unless paused) ---
    if !control_mode.physics_paused {
        if let Ok((mut plane_transform, mut aircraft)) = aircraft_query.single_mut() {
        // 1. ENGINE & DRAG
        if keyboard.pressed(KeyCode::Equal) {
            aircraft.throttle = (aircraft.throttle + 0.5 * dt).min(aircraft.max_throttle);
        }
        if keyboard.pressed(KeyCode::Minus) {
            aircraft.throttle = (aircraft.throttle - 0.5 * dt).max(0.0);
        }

        // A. Engine Acceleration
        let target_speed = aircraft.throttle * aircraft.max_speed;
        let engine_acceleration = (target_speed - aircraft.speed) * aircraft.drag_factor;
        
        // B. Climb/Dive Acceleration (Gravity)
        let climb_angle = plane_transform.forward().y;
        let gravity_acceleration = -climb_angle * aircraft.gravity;

        // C. Induced Drag (G-force based)
        let centripetal_accel = aircraft.speed * aircraft.pitch_velocity.abs();
        let g_force = centripetal_accel / 9.8;
        let turn_drag = g_force * aircraft.g_force_drag;

        // D. Wind Force (headwind/tailwind effect)
        let forward = plane_transform.forward().as_vec3();
        let wind_dot = forward.dot(wind.base_wind.normalize_or_zero());
        let wind_acceleration = wind_dot * wind.base_wind.length() * 0.3;

        // E. Quadratic drag (increases with speed²)
        let speed_drag = (aircraft.speed / aircraft.max_speed).powi(2) * 20.0;

        // Apply all accelerations
        aircraft.speed += (engine_acceleration + gravity_acceleration - turn_drag - speed_drag + wind_acceleration) * dt;
        aircraft.speed = aircraft.speed.max(0.0);

        // 2. AERODYNAMICS
        let airspeed_ratio = aircraft.speed / aircraft.max_speed;
        
        let control_effectiveness = if airspeed_ratio > 1.0 {
            dbg!((1.0 / (airspeed_ratio.powf(3.0))).clamp(0.2, 1.0))
        } else {
            airspeed_ratio.clamp(0.0, 1.0)
        };
        
        let pitch_strength = aircraft.pitch_strength * control_effectiveness;
        let roll_strength = aircraft.roll_strength * control_effectiveness;
        let yaw_strength = aircraft.yaw_strength * control_effectiveness;
        let rotational_damping = 2.0;

        // --- INPUTS (Only in Aircraft mode) ---
        if control_mode.mode == FlightMode::Aircraft {
            let mut is_rolling = false;
            let mut is_pitching = false;

            if keyboard.pressed(KeyCode::KeyW) { 
                aircraft.pitch_velocity -= pitch_strength * dt;
                is_pitching = true;
            }
            if keyboard.pressed(KeyCode::KeyS) { 
                aircraft.pitch_velocity += pitch_strength * dt;
                is_pitching = true;
            }
            
            if keyboard.pressed(KeyCode::KeyA) { 
                aircraft.roll_velocity += roll_strength * dt; 
                is_rolling = true;
            }
            if keyboard.pressed(KeyCode::KeyD) { 
                aircraft.roll_velocity -= roll_strength * dt; 
                is_rolling = true;
            }
            
            if keyboard.pressed(KeyCode::KeyQ) { aircraft.yaw_velocity += yaw_strength * dt; }
            if keyboard.pressed(KeyCode::KeyE) { aircraft.yaw_velocity -= yaw_strength * dt; }

            // --- STABILITY & AUTO-CORRECTION ---
            let bank_angle = plane_transform.right().y;
            let pitch_angle = plane_transform.forward().y;
            
            aircraft.yaw_velocity += bank_angle * aircraft.bank_turn_strength * control_effectiveness * dt;

            if !is_rolling {
                let auto_level_force = -bank_angle * aircraft.auto_level_strength * control_effectiveness;
                aircraft.roll_velocity += auto_level_force * dt;
            }

            if !is_pitching {
                let auto_level_force = -pitch_angle * aircraft.auto_level_strength * control_effectiveness;
                aircraft.pitch_velocity += auto_level_force / 1.25 * dt;
            }
        }

        // --- STALL BEHAVIOR (Always applies) ---
        let stall_strength = (1.0 - airspeed_ratio).max(0.0);
        let stall_pitch_down = stall_strength * match plane_transform.up().y.is_sign_negative() {
            true => -1.0,
            false => 1.0,
        };
        if aircraft.speed < aircraft.max_speed * 0.33 {
            aircraft.pitch_velocity -= stall_pitch_down * dt;
        }

        // --- WIND TURBULENCE (Perlin noise-based) ---
        let pos = plane_transform.translation;
        let t = time.elapsed_secs_f64() as f64;
        let freq = wind.turbulence_frequency as f64;
        
        let turbulence_pitch = wind.perlin.get([pos.x as f64 * freq, pos.z as f64 * freq, t * freq]) as f32;
        let turbulence_roll = wind.perlin.get([pos.z as f64 * freq, pos.y as f64 * freq, t * freq + 100.0]) as f32;
        let turbulence_yaw = wind.perlin.get([pos.y as f64 * freq, pos.x as f64 * freq, t * freq + 200.0]) as f32;
        
        let turbulence_scale = wind.turbulence_intensity * (airspeed_ratio + 1.0).powf(8.0);
        aircraft.pitch_velocity += turbulence_pitch * turbulence_scale * dt;
        aircraft.roll_velocity += turbulence_roll * turbulence_scale * dt;
        aircraft.yaw_velocity += turbulence_yaw * turbulence_scale * dt;

        // --- DAMPING & MOVEMENT (Always applies) ---
        aircraft.pitch_velocity -= aircraft.pitch_velocity * rotational_damping * dt;
        aircraft.roll_velocity -= aircraft.roll_velocity * rotational_damping * dt;
        aircraft.yaw_velocity -= aircraft.yaw_velocity * rotational_damping * dt;

        // Apply Rotation
        plane_transform.rotate_local_x(aircraft.pitch_velocity * dt);
        plane_transform.rotate_local_z(aircraft.roll_velocity * dt);
        plane_transform.rotate_local_y(aircraft.yaw_velocity * dt);

        // Apply Lift vs Gravity (Vertical position)
        let lift_threshold = 150.0;
        let gravity_strength = 30.0;
        let gravity_factor = (1.0 - (aircraft.speed / lift_threshold)).max(0.0);
        
        let mut movement = forward * aircraft.speed;
        movement.y -= gravity_strength * gravity_factor;
        
        movement += wind.base_wind * 0.5;

        plane_transform.translation += movement * dt;
        }
    }

    // --- CAMERA CONTROLS (Only in FreeFlight mode) ---
    if control_mode.mode == FlightMode::FreeFlight {
        if let Ok(mut camera_transform) = camera_query.single_mut() {
            let rotation_speed = 0.7;
            let pan_speed = if keyboard.pressed(KeyCode::ShiftLeft) { 1000.0 } else { 200.0 };
            
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
            if keyboard.pressed(KeyCode::KeyZ) { camera_transform.rotate_local_z(panning_delta); }
            if keyboard.pressed(KeyCode::KeyX) { camera_transform.rotate_local_z(-panning_delta); }

            camera_transform.translation += pan_direction.normalize_or_zero() * pan_speed * dt;
        }
    }
}


pub fn camera_follow_aircraft(
    time: Res<Time>,
    control_mode: Res<ControlMode>,
    aircraft_query: Query<(&Transform, &Aircraft), (With<Aircraft>, Without<MainCamera>)>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    if control_mode.mode != FlightMode::Aircraft || control_mode.physics_paused {
        return;
    }

    let Ok((plane_transform, aircraft)) = aircraft_query.single() else { return; };
    let Ok(mut camera_transform) = camera_query.single_mut() else { return; };

    let plane_movement = plane_transform.forward().as_vec3() * aircraft.speed * time.delta_secs();
    camera_transform.translation += plane_movement;

    let base_distance = 18.0;
    let max_extra_dist = 4.0;
    let speed_threshold = 100.0;
    let height = 8.0;
    let look_ahead_distance = 0.2 * aircraft.speed;
    
    let speed_ratio = (aircraft.speed / speed_threshold).clamp(0.0, 2.0);
    let dynamic_distance = base_distance + (max_extra_dist * speed_ratio.powf(0.5));

    let smoothness = 2.0 + (speed_ratio * 1.5);
    let t = (time.delta_secs() * smoothness).min(1.0);

    let target_position = plane_transform.translation 
        + (plane_transform.back() * dynamic_distance) 
        + (plane_transform.up() * height);
    
    camera_transform.translation = camera_transform.translation.lerp(target_position, t);

    let look_target = plane_transform.translation + (plane_transform.forward() * look_ahead_distance);
    let target_rotation = camera_transform.looking_at(look_target, plane_transform.up()).rotation;
    camera_transform.rotation = camera_transform.rotation.slerp(target_rotation, t);
}

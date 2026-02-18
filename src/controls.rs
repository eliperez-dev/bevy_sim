use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*
};

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

    // --- TUNING (The sliders we want to tweak) ---
    pub max_speed: f32,
    pub drag_factor: f32,        // Air resistance
    pub g_force_drag: f32,       // Speed loss when turning
    
    pub pitch_strength: f32,
    pub roll_strength: f32,
    pub yaw_strength: f32,
    
    pub bank_turn_strength: f32, // How much rolling automatically turns the plane
    pub auto_level_strength: f32,// How fast it levels out when letting go
}

impl Default for Aircraft {
    fn default() -> Self {
        Self {
            speed: 0.0,
            throttle: 0.0,
            pitch_velocity: 0.0,
            roll_velocity: 0.0,
            yaw_velocity: 0.0,

            // Default Physics Values
            max_speed: 250.0,
            drag_factor: 0.5,
            g_force_drag: 100.0,
            pitch_strength: 2.0,
            roll_strength: 3.0,
            yaw_strength: 1.0,
            bank_turn_strength: 0.5,
            auto_level_strength: 1.0,
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
            if keyboard.pressed(KeyCode::Equal) {
                aircraft.throttle = (aircraft.throttle + 0.5 * dt).min(1.0);
            }
            if keyboard.pressed(KeyCode::Minus) {
                aircraft.throttle = (aircraft.throttle - 0.5 * dt).max(0.0);
            }

            let max_speed = aircraft.max_speed;
            let drag_factor = aircraft.drag_factor; 
            let g_force_drag = aircraft.g_force_drag;
            
            let target_speed = aircraft.throttle * max_speed;
            
            // Drag from turning hard
            let cornering_penalty = aircraft.pitch_velocity.abs() * g_force_drag * dt;
            aircraft.speed = (aircraft.speed - cornering_penalty).max(0.0);

            // Engine power vs Air Resistance
            aircraft.speed = aircraft.speed + (target_speed - aircraft.speed) * drag_factor * dt;

            // 2. AERODYNAMICS
            let airspeed_ratio = (aircraft.speed / max_speed).clamp(0.0, 1.0);
            
            let pitch_strength = aircraft.pitch_strength * airspeed_ratio;
            let roll_strength = aircraft.roll_strength * airspeed_ratio;
            let yaw_strength = aircraft.yaw_strength * airspeed_ratio;
            let rotational_damping = 2.0; 

            // --- INPUTS ---
            let mut is_rolling = false;

            if keyboard.pressed(KeyCode::KeyW) { aircraft.pitch_velocity -= pitch_strength * dt; }
            if keyboard.pressed(KeyCode::KeyS) { aircraft.pitch_velocity += pitch_strength * dt; }
            
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
            
            // Calculate Bank Angle (Right vector Y component)
            // If > 0, we are banked Left. If < 0, we are banked Right.
            let bank_angle = plane_transform.right().y; 

            // A. Auto-Yaw (Coordinated Turn)
            // Reduced multiplier from 1.5 to 0.5 for gentler turns
            aircraft.yaw_velocity += bank_angle * aircraft.bank_turn_strength * airspeed_ratio * dt;

            if !is_rolling {
                let auto_level_force = -bank_angle * aircraft.auto_level_strength * airspeed_ratio;
                aircraft.roll_velocity += auto_level_force * dt;
            }

            // --- DAMPING & MOVEMENT ---

            // Apply Damping 
            aircraft.pitch_velocity -= aircraft.pitch_velocity * rotational_damping * dt;
            aircraft.roll_velocity -= aircraft.roll_velocity * rotational_damping * dt;
            aircraft.yaw_velocity -= aircraft.yaw_velocity * rotational_damping * dt;

            // Apply Rotation
            plane_transform.rotate_local_x(aircraft.pitch_velocity * dt);
            plane_transform.rotate_local_z(aircraft.roll_velocity * dt);
            plane_transform.rotate_local_y(aircraft.yaw_velocity * dt);

            // Apply Lift vs Gravity
            let forward = plane_transform.forward().as_vec3();
            let lift_threshold = 150.0;
            let gravity_strength = 30.0;
            let gravity_factor = (1.0 - (aircraft.speed / lift_threshold)).max(0.0);
            
            let mut movement = forward * aircraft.speed;
            movement.y -= gravity_strength * gravity_factor;

            plane_transform.translation += movement * dt;
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

    let plane_movement = plane_transform.forward().as_vec3() * aircraft.speed * time.delta_secs();
    camera_transform.translation += plane_movement;

    let base_distance = 18.0;
    let max_extra_dist = 4.0;
    let speed_threshold = 100.0;
    let height = 6.0;
    let look_ahead_distance = 20.0;
    
    let speed_ratio = (aircraft.speed / speed_threshold).clamp(0.0, 2.0);
    let dynamic_distance = base_distance + (max_extra_dist * speed_ratio.powf(0.5));

    let smoothness = 5.0 + (speed_ratio * 2.0);
    let t = (time.delta_secs() * smoothness).min(1.0);

    let target_position = plane_transform.translation 
        + (plane_transform.back() * dynamic_distance) 
        + (plane_transform.up() * height);
    
    camera_transform.translation = camera_transform.translation.lerp(target_position, t);

    let look_target = plane_transform.translation + (plane_transform.forward() * look_ahead_distance);
    let target_rotation = camera_transform.looking_at(look_target, plane_transform.up()).rotation;
    camera_transform.rotation = camera_transform.rotation.slerp(target_rotation, t);
}
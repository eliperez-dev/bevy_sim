use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*,
};
use noise::{NoiseFn, Perlin};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlightMode {
    Aircraft,
    Orbit,
    FreeFlight,
}

impl Default for ControlMode {
    fn default() -> Self {
        Self {
            mode: FlightMode::Aircraft, 
            physics_paused: false,
        }
    }
}

#[derive(Component)]
pub struct MainCamera {
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_distance: f32, 
}

// Manually implement default to establish our starting values
impl Default for MainCamera {
    fn default() -> Self {
        Self {
            orbit_yaw: 0.0,
            orbit_pitch: 0.0,
            orbit_distance: 18.0, // Replaces your hardcoded `base_distance = 18.0`
        }
    }
}


#[derive(Component)]
pub struct Aircraft {
    // --- STATE ---
    pub velocity: Vec3,
    pub speed: f32,
    pub throttle: f32,
    pub pitch_velocity: f32,
    pub roll_velocity: f32,
    pub yaw_velocity: f32,

    // --- TUNING ---
    pub max_speed: f32,
    pub max_throttle: f32,       
    pub thrust: f32,        // Now acts as the core engine power multiplier
    pub gravity: f32,            // How much climbing slows you / diving speeds you up
    pub g_force_drag: f32,       // Speed loss when turning hard
    pub lift_coefficient: f32,   // How much lift is generated at optimal AoA
    pub lift_reduction_factor: f32, // How much lift reduces gravity effect
    pub parasitic_drag_coef: f32, // Parasitic drag coefficient
    
    pub pitch_strength: f32,
    pub roll_strength: f32,
    pub yaw_strength: f32,
    
    pub bank_turn_strength: f32, // Auto-yaw when banking
    pub auto_level_strength: f32,// Stability
}

impl Default for Aircraft {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            speed: 150.0,
            throttle: 0.80,
            pitch_velocity: 0.0,
            roll_velocity: 0.0,
            yaw_velocity: 0.0,

            // Default Physics Values
            max_speed: 450.0,
            max_throttle: 1.5,
            thrust: 1.5,
            gravity: 80.0,       
            g_force_drag: 1.2,
            lift_coefficient: 2.5,
            lift_reduction_factor: 30.0,
            parasitic_drag_coef: 5.0,
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


#[derive(Resource)]
pub struct Wind {
    pub wind_direction: Vec3, // Keep this normalized!
    pub wind_speed: f32,      // Pure scalar speed
    
    // --- Base Wind Evolution (Time-based changes) ---
    pub wind_evolution_speed: f64,
    pub min_wind_speed: f32,
    pub max_wind_speed: f32,
    
    // --- Macro Wind (Large sweeping weather patterns) ---
    pub macro_wind_freq: f64,
    pub weather_evolution_rate: f64,
    pub max_angle_shift: f32,
    
    // --- Micro Wind (Turbulence & Gusts) ---
    pub turbulence_intensity: f32,
    pub turbulence_frequency: f32,
    pub gust_frequency_multiplier: f64,
    
    pub perlin: Perlin,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            // We normalize this so it strictly represents a direction/heading
            wind_direction: Vec3::new(2.0, 0.0, 1.0).normalize(), 
            
            // 11.18 is roughly the speed (length) of your old Vec3(10.0, 0.0, 5.0)
            wind_speed: 11.18, 
            
            wind_evolution_speed: 0.05,
            min_wind_speed: 5.0,
            max_wind_speed: 35.0,
            
            macro_wind_freq: 0.001,
            weather_evolution_rate: 0.85,
            max_angle_shift: std::f32::consts::FRAC_PI_2,
            
            turbulence_intensity: 0.008,
            turbulence_frequency: 6.0,
            gust_frequency_multiplier: 0.00075,
            
            perlin: Perlin::new(42),
        }
    }
}

pub fn get_control_effectiveness(airspeed_ratio: f32) -> f32 {
    if airspeed_ratio > 1.0 {
        1.0
    } else {
        (1.0 - ( 1.0 - airspeed_ratio).powf(3.0)).clamp(0.0, 1.0)
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
            FlightMode::Aircraft => FlightMode::Orbit,
            FlightMode::Orbit => FlightMode::FreeFlight,
            FlightMode::FreeFlight => FlightMode::Aircraft,
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
            // Get position and time early so we can use them for all noise calculations
            let pos = plane_transform.translation;
            let t = time.elapsed_secs_f64() as f64;

            // 1. ENGINE & DRAG
            if keyboard.pressed(KeyCode::Equal) {
                aircraft.throttle = (aircraft.throttle + 0.5 * dt).min(aircraft.max_throttle);
            }
            if keyboard.pressed(KeyCode::Minus) {
                aircraft.throttle = (aircraft.throttle - 0.5 * dt).max(0.0);
            }

            let forward = plane_transform.forward().as_vec3();
            let climb_angle = forward.y;
            
            let airspeed_ratio = aircraft.speed / aircraft.max_speed;
            let dynamic_pressure = airspeed_ratio.powi(2);

            // A. Engine Acceleration (Thrust Falloff)
            let base_thrust = 50.0 * aircraft.thrust; 
            let max_effective_ratio = aircraft.throttle + 0.2; 
            let high_speed_falloff = (max_effective_ratio - airspeed_ratio).clamp(0.0, 1.0); 
            let engine_acceleration = aircraft.throttle * base_thrust * high_speed_falloff;
            
            // B. Lift & Gravity Physics
            let gravity_acceleration = -climb_angle * aircraft.gravity;
            
            let lift_efficiency = (1.0 - climb_angle.abs()).max(0.3);
            let lift_force = aircraft.lift_coefficient * dynamic_pressure * lift_efficiency * aircraft.lift_reduction_factor;
            
            let final_gravity_accel = if climb_angle > 0.0 {
                let lift_reduction = lift_force * 0.5;
                let effective_gravity_accel = gravity_acceleration.abs() - lift_reduction;
                -effective_gravity_accel.max(0.0)
            } else {
                let lift_reduction = lift_force * 0.2;
                let effective_gravity_accel = gravity_acceleration.abs() - lift_reduction;
                effective_gravity_accel.max(0.0)
            };
            let gravity_acceleration = final_gravity_accel;

            // C. Turn Drag (G-forces from maneuvers)
            let centripetal_accel = aircraft.speed * aircraft.pitch_velocity.abs();
            let g_force = centripetal_accel / 9.8;
            let turn_drag = g_force * aircraft.g_force_drag * dynamic_pressure;

            // D. Wind Force (Macro Wind Intensity & Direction via Perlin Noise)
            
            // Recombine direction and speed to get the base velocity for drift calculation
            let base_wind_velocity = wind.wind_direction * wind.wind_speed;
            let wind_drift = base_wind_velocity * t as f32;
            
            let sample_x = (pos.x - wind_drift.x) as f64 * wind.macro_wind_freq;
            let sample_z = (pos.z - wind_drift.z) as f64 * wind.macro_wind_freq;
            let weather_evolution = t * (wind.macro_wind_freq * wind.weather_evolution_rate); 
            
            // 1. Intensity Noise (Unchanged)
            let wind_intensity_noise = wind.perlin.get([
                sample_x, 
                weather_evolution, 
                sample_z
            ]) as f32;
            let wind_multiplier = (wind_intensity_noise * 0.8) + 1.0; 

            // 2. Direction Noise (Unchanged)
            let wind_dir_noise = wind.perlin.get([
                sample_x + 1000.0, 
                weather_evolution + 1000.0, 
                sample_z + 1000.0
            ]) as f32;
            let angle_shift = wind_dir_noise * wind.max_angle_shift;
            
            // 3. Apply changes cleanly
            // Rotate the normalized direction, NOT the speed
            let wind_rotation = Quat::from_rotation_y(angle_shift);
            let current_wind_dir = wind_rotation * wind.wind_direction;
            
            // Current wind speed is base speed * noise multiplier
            let current_speed = wind.wind_speed * wind_multiplier;

            // Final Wind Vector to push the plane sideways
            let current_wind = current_wind_dir * current_speed;

            // Apply forward acceleration
            // Because current_wind_dir is already normalized, we skip the costly .normalize_or_zero() call!
            let wind_dot = forward.dot(current_wind_dir);
            
            // We can also skip .length() and just use current_speed directly
            let wind_acceleration = wind_dot * current_speed * 0.3;

            // E. Parasitic drag
            let high_speed_multiplier = 1.0 + (airspeed_ratio.max(1.0) - 1.0) * 0.5;
            let parasitic_drag = dynamic_pressure * aircraft.parasitic_drag_coef * high_speed_multiplier;

            // Apply all accelerations
            aircraft.speed += (engine_acceleration + gravity_acceleration - turn_drag - parasitic_drag + wind_acceleration) * dt;
            aircraft.speed = aircraft.speed.max(0.0);

            // 2. AERODYNAMICS
            let control_effectiveness = get_control_effectiveness(airspeed_ratio);

            let pitch_strength = aircraft.pitch_strength * control_effectiveness;
            let roll_strength = aircraft.roll_strength * control_effectiveness;
            let yaw_strength = aircraft.yaw_strength * control_effectiveness;
            let rotational_damping = 2.0;

            // --- INPUTS (Only in Aircraft mode) ---
            if control_mode.mode == FlightMode::Aircraft
            || control_mode.mode == FlightMode::Orbit {
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
            let freq = wind.turbulence_frequency as f64;
            
            // Add wind drift to turbulence sampling as well so gusts travel with the wind!
            let turb_sample_x = pos.x as f64 - wind_drift.x as f64;
            let turb_sample_z = pos.z as f64 - wind_drift.z as f64;

            let gust_freq = freq * wind.gust_frequency_multiplier;
            let turbulence_velocity_x = wind.perlin.get([turb_sample_x * gust_freq, t * gust_freq, turb_sample_z * gust_freq + 300.0]) as f32;
            let turbulence_velocity_y = wind.perlin.get([pos.y as f64 * gust_freq, t * gust_freq, turb_sample_x * gust_freq + 400.0]) as f32;
            let turbulence_velocity_z = wind.perlin.get([turb_sample_z * gust_freq, t * gust_freq, pos.y as f64 * gust_freq + 500.0]) as f32;
            
            let turbulence_force = Vec3::new(turbulence_velocity_x, turbulence_velocity_y, turbulence_velocity_z);
            let turbulance_pow = 5.5;
            let turbulence_velocity_scale = wind.turbulence_intensity * 85.0 * (airspeed_ratio + 0.5).powf(turbulance_pow);
            
            // Couple translational turbulence to rotational effects
            let coupling_strength = 0.7;
            let turbulence_pitch = turbulence_velocity_y * coupling_strength;
            let turbulence_roll = -turbulence_velocity_x * coupling_strength * 1.25;
            let turbulence_yaw = turbulence_velocity_x * coupling_strength / 1.5;
            
            let turbulence_scale = wind.turbulence_intensity * (airspeed_ratio + 1.0).powf(turbulance_pow);
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
            
            // Apply the noisy current_wind instead of the static base_wind
            movement += current_wind * 0.5; 
            movement += turbulence_force * turbulence_velocity_scale;

            aircraft.velocity = movement;

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


pub fn evolve_wind(
    mut wind: ResMut<Wind>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs_f64();
    let evolution_speed = wind.wind_evolution_speed;
    
    let dir_x = wind.perlin.get([t * evolution_speed, 0.0, 0.0]) as f32;
    let dir_y = wind.perlin.get([0.0, t * evolution_speed, 1000.0]) as f32;
    let dir_z = wind.perlin.get([1000.0, 0.0, t * evolution_speed]) as f32;
    
    let noise_direction = Vec3::new(dir_x, dir_y, dir_z);
    
    if noise_direction.length_squared() > 0.001 {
        wind.wind_direction = noise_direction.normalize();
    }
    
    let speed_noise = wind.perlin.get([t * evolution_speed + 5000.0, t * evolution_speed + 5000.0, 0.0]) as f32;
    let speed_range = wind.max_wind_speed - wind.min_wind_speed;
    wind.wind_speed = wind.min_wind_speed + (speed_noise + 1.0) * 0.5 * speed_range;
}

pub fn camera_follow_aircraft(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    control_mode: Res<ControlMode>,
    aircraft_query: Query<(&Transform, &Aircraft), (With<Aircraft>, Without<MainCamera>)>,
    mut camera_query: Query<(&mut Transform, &mut MainCamera)>,
) {
    // Abort if we are in FreeFlight or paused
    if control_mode.mode == FlightMode::FreeFlight || control_mode.physics_paused {
        return;
    }

    let Ok((plane_transform, aircraft)) = aircraft_query.single() else { return; };
    let Ok((mut camera_transform, mut main_camera)) = camera_query.single_mut() else { return; };

    // --- CAMERA ZOOM CONTROLS (Only in Orbit Mode) ---
    if control_mode.mode == FlightMode::Orbit 
    || control_mode.mode == FlightMode::Aircraft {
        let zoom_speed = 25.0; // Adjust for faster/slower zooming
        
        if keyboard.pressed(KeyCode::KeyX) { main_camera.orbit_distance -= zoom_speed * time.delta_secs(); }
        if keyboard.pressed(KeyCode::KeyZ) { main_camera.orbit_distance += zoom_speed * time.delta_secs(); }
        
        // Clamp it so you can't clip through the plane or zoom into the stratosphere
        main_camera.orbit_distance = main_camera.orbit_distance.clamp(5.0, 150.0);
    }

    // --- BASE CAMERA MATH ---
    let mut actual_direction = aircraft.velocity.normalize_or_zero();
    if actual_direction == Vec3::ZERO {
        actual_direction = plane_transform.forward().into();
    }

    let plane_movement = aircraft.velocity * time.delta_secs();
    camera_transform.translation += plane_movement;

    let max_extra_dist = 4.0;
    let speed_threshold = 100.0;
    let height = 7.0;
    
    let speed_ratio = (aircraft.speed / speed_threshold).clamp(0.0, 2.0);
    
    // We replace 'base_distance' with our new controllable variable!
    let dynamic_distance = main_camera.orbit_distance + (max_extra_dist * speed_ratio.powf(0.5));

    let smoothness = 2.0 + (speed_ratio * 1.5);
    let t = (time.delta_secs() * smoothness).min(1.0);

    // --- APPLY BEHAVIOR BASED ON FLIGHT MODE ---
    match control_mode.mode {
        FlightMode::Orbit => {
            let orbit_speed = 2.0;

            if keyboard.pressed(KeyCode::ArrowLeft) { main_camera.orbit_yaw -= orbit_speed * time.delta_secs(); }
            if keyboard.pressed(KeyCode::ArrowRight) { main_camera.orbit_yaw += orbit_speed * time.delta_secs(); }
            if keyboard.pressed(KeyCode::ArrowDown) { main_camera.orbit_pitch -= orbit_speed * time.delta_secs(); }
            if keyboard.pressed(KeyCode::ArrowUp) { main_camera.orbit_pitch += orbit_speed * time.delta_secs(); }

            // Clamp to prevent flipping upside down
            main_camera.orbit_pitch = main_camera.orbit_pitch.clamp(-0.4, 1.2);

            // Calculate Orbit Position using world-space axes (not affected by plane rotation)
            let world_up = Vec3::Y;
            let world_right = Vec3::X;
            
            let orbit_rotation = Quat::from_axis_angle(world_up, main_camera.orbit_yaw) 
                               * Quat::from_axis_angle(world_right, main_camera.orbit_pitch);

            let base_offset = Vec3::new(0.0, height * 1.5, -dynamic_distance * 1.5);
            let rotated_offset = orbit_rotation * base_offset;

            let target_position = plane_transform.translation + rotated_offset;
            camera_transform.translation = camera_transform.translation.lerp(target_position, t);

            // Look directly at the plane
            let target_rotation = camera_transform.looking_at(plane_transform.translation, world_up).rotation;
            camera_transform.rotation = target_rotation;
        }
        FlightMode::Aircraft => {
            // Standard Chase Position
            let target_position = plane_transform.translation 
                + (-actual_direction * dynamic_distance) 
                + (plane_transform.up() * height);
            camera_transform.translation = camera_transform.translation.lerp(target_position, t);

            // Look Ahead of the plane
            let look_ahead_distance = 0.2 * aircraft.speed;
            let look_target = plane_transform.translation + (actual_direction * look_ahead_distance);
            
            let target_rotation = camera_transform.looking_at(look_target, plane_transform.up()).rotation;
            camera_transform.rotation = camera_transform.rotation.slerp(target_rotation, t);
        }
        FlightMode::FreeFlight => unreachable!(), // Handled by early return
    }
}
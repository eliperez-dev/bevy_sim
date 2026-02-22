use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*,
};
use noise::{NoiseFn, Perlin};

use crate::world_generation::WorldGenerator;

// Constants for physics calculations
const BASE_THRUST_MULTIPLIER: f32 = 50.0;
const THRUST_HEADROOM: f32 = 0.2;
const G_FORCE_CONSTANT: f32 = 9.8;
const LIFT_EFFICIENCY_MIN: f32 = 0.3;
const LIFT_REDUCTION_CLIMBING: f32 = 0.5;
const LIFT_REDUCTION_DIVING: f32 = 0.2;
const STALL_THRESHOLD_RATIO: f32 = 0.33;
const ROTATIONAL_DAMPING: f32 = 2.0;
const LIFT_THRESHOLD_SPEED: f32 = 150.0;
const GRAVITY_STRENGTH: f32 = 30.0;
const WIND_FORWARD_COUPLING: f32 = 0.3;
const WIND_LATERAL_COUPLING: f32 = 0.5;
const WIND_NOISE_OFFSET_DIRECTION: f32 = 1000.0;
const TURBULENCE_NOISE_OFFSET_BASE: f64 = 300.0;
const TURBULENCE_POWER: f32 = 5.0;
const TURBULENCE_VELOCITY_MULTIPLIER: f32 = 85.0;
const TURBULENCE_COUPLING_STRENGTH: f32 = 0.7;
const AUTO_LEVEL_PITCH_DIVISOR: f32 = 1.25;

// Camera control constants
const FREE_FLIGHT_ROTATION_SPEED: f32 = 0.7;
const FREE_FLIGHT_PAN_SPEED_NORMAL: f32 = 200.0;
const FREE_FLIGHT_PAN_SPEED_FAST: f32 = 1000.0;
const THROTTLE_CHANGE_RATE: f32 = 0.5;
const CAMERA_ZOOM_SPEED: f32 = 45.0;
const CAMERA_ZOOM_MIN: f32 = 5.0;
const CAMERA_ZOOM_MAX: f32 = 250.0;
const ORBIT_ROTATION_SPEED: f32 = 2.0;
const ORBIT_PITCH_MIN: f32 = -0.4;
const ORBIT_PITCH_MAX: f32 = 1.2;
const CAMERA_MAX_EXTRA_DISTANCE: f32 = 15.0;
const CAMERA_SPEED_THRESHOLD: f32 = 100.0;
const CAMERA_HEIGHT: f32 = 12.0;
const CAMERA_SMOOTHNESS_BASE: f32 = 2.0;
const CAMERA_SMOOTHNESS_MULTIPLIER: f32 = 1.5;
const CAMERA_LOOK_AHEAD_MULTIPLIER: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlightMode {
    Aircraft,
    Orbit,
    FreeFlight,
}

#[derive(Resource)]
pub struct ControlMode {
    pub mode: FlightMode,
    pub physics_paused: bool,
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

impl Default for MainCamera {
    fn default() -> Self {
        Self {
            orbit_yaw: 0.0,
            orbit_pitch: 0.0,
            orbit_distance: 15.0,
        }
    }
}

#[derive(Component)]
pub struct Aircraft {
    // State
    pub velocity: Vec3,
    pub speed: f32,
    pub throttle: f32,
    pub pitch_velocity: f32,
    pub roll_velocity: f32,
    pub yaw_velocity: f32,
    pub crashed: bool,

    // Physics tuning parameters
    pub max_speed: f32,
    pub max_throttle: f32,       
    pub thrust: f32,
    pub gravity: f32,
    pub g_force_drag: f32,
    pub lift_coefficient: f32,
    pub lift_reduction_factor: f32,
    pub parasitic_drag_coef: f32,
    
    // Control responsiveness
    pub pitch_strength: f32,
    pub roll_strength: f32,
    pub yaw_strength: f32,

    // Simulated banking
    pub bank_turn_strength: f32,
    
    // Flight assists
    pub auto_level_strength: f32,
    
    // Respawn settings
    pub respawn_height: f32,
    pub respawn_speed: f32,
}


impl Aircraft {
    pub fn light() -> Self {
        Self {
            velocity: Vec3::ZERO,
            speed: 150.0,
            throttle: 0.80,
            pitch_velocity: 0.0,
            roll_velocity: 0.0,
            yaw_velocity: 0.0,
            crashed: false,
            max_speed: 350.0,
            max_throttle: 2.0,
            thrust: 1.5,
            gravity: 80.0,       
            g_force_drag: 2.5,
            lift_coefficient: 2.5,
            lift_reduction_factor: 30.0,
            parasitic_drag_coef: 8.0,
            pitch_strength: 2.0,
            roll_strength: 3.0,
            yaw_strength: 1.0,
            bank_turn_strength: 0.85,
            auto_level_strength: 1.00,
            respawn_height: 500.0,
            respawn_speed: 150.0,
        }
    }

    pub fn jet() -> Self {
        Self {
            velocity: Vec3::ZERO,
            speed: 1000.0,
            throttle: 0.80,
            pitch_velocity: 0.0,
            roll_velocity: 0.0,
            yaw_velocity: 0.0,
            crashed: false,
            max_speed: 1500.0,
            max_throttle: 2.5,
            thrust: 8.0,
            gravity: 80.0,       
            g_force_drag: 2.5,
            lift_coefficient: 2.5,
            lift_reduction_factor: 30.0,
            parasitic_drag_coef: 100.0,
            pitch_strength: 3.0,
            roll_strength: 12.5,
            yaw_strength: 0.35,
            bank_turn_strength: 0.1,
            auto_level_strength: 0.1,
            respawn_height: 1000.0,
            respawn_speed: 1300.0,
        }
    }
}

#[derive(Resource)]
pub struct Wind {
    pub wind_direction: Vec3,
    pub wind_speed: f32,
    
    // Base wind evolution
    pub wind_evolution_speed: f64,
    pub min_wind_speed: f32,
    pub max_wind_speed: f32,
    
    // Macro wind (weather patterns)
    pub macro_wind_freq: f64,
    pub weather_evolution_rate: f64,
    pub max_angle_shift: f32,
    
    // Micro wind (turbulence)
    pub turbulence_intensity: f32,
    pub turbulence_frequency: f32,
    pub gust_frequency_multiplier: f64,
    
    pub perlin: Perlin,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            wind_direction: Vec3::new(2.0, 0.0, 1.0).normalize(), 
            wind_speed: 0.0, 
            wind_evolution_speed: 0.01,
            min_wind_speed: 0.0,
            max_wind_speed: 5.0,
            macro_wind_freq: 0.001,
            weather_evolution_rate: 0.85,
            max_angle_shift: std::f32::consts::FRAC_PI_4,
            turbulence_intensity: 0.005,
            turbulence_frequency: 6.0,
            gust_frequency_multiplier: 0.00075,
            perlin: Perlin::new(42),
        }
    }
}

/// Calculate how effective flight controls are based on airspeed
pub fn get_control_effectiveness(airspeed_ratio: f32) -> f32 {
    if airspeed_ratio > 1.0 {
        1.0
    } else {
        (1.0 - (1.0 - airspeed_ratio).powf(3.0)).clamp(0.0, 1.0)
    }
}

/// Handle input toggles for flight mode, wireframe, and physics pause
fn handle_input_toggles(
    keyboard: &ButtonInput<KeyCode>,
    wire_frame: &mut WireframeConfig,
    control_mode: &mut ControlMode,
    aircraft: Option<&mut Aircraft>,
    plane_transform: Option<&mut Transform>,
    world_gen: Option<&WorldGenerator>,
) {
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
    if keyboard.just_pressed(KeyCode::KeyR) {
        if let Some(aircraft) = aircraft {
            if aircraft.crashed {
                if let (Some(transform), Some(world_gen)) = (plane_transform, world_gen) {
                    let current_pos = transform.translation;
                    let terrain_height = world_gen.get_terrain_height(&[current_pos.x, current_pos.y, current_pos.z]);
                    
                    aircraft.crashed = false;
                    aircraft.speed = aircraft.respawn_speed;
                    aircraft.throttle = 0.8;
                    aircraft.velocity = Vec3::ZERO;
                    aircraft.pitch_velocity = 0.0;
                    aircraft.roll_velocity = 0.0;
                    aircraft.yaw_velocity = 0.0;
                    control_mode.physics_paused = false;
                    
                    transform.translation.y = (terrain_height + aircraft.respawn_height).max(aircraft.respawn_height);
                    transform.rotation = Quat::IDENTITY;
                    
                    info!("Aircraft respawned");
                }
            }
        }
    }
}

/// Update throttle based on keyboard input
fn update_throttle(keyboard: &ButtonInput<KeyCode>, aircraft: &mut Aircraft, dt: f32) {
    if keyboard.pressed(KeyCode::Equal) {
        aircraft.throttle = (aircraft.throttle + THROTTLE_CHANGE_RATE * dt).min(aircraft.max_throttle);
    }
    if keyboard.pressed(KeyCode::Minus) {
        aircraft.throttle = (aircraft.throttle - THROTTLE_CHANGE_RATE * dt).max(0.0);
    }
}

struct PhysicsForces {
    engine_acceleration: f32,
    gravity_acceleration: f32,
    turn_drag: f32,
    parasitic_drag: f32,
    wind_acceleration: f32,
}

/// Calculate engine and aerodynamic forces
fn calculate_engine_and_drag(
    aircraft: &Aircraft,
    climb_angle: f32,
    airspeed_ratio: f32,
    dynamic_pressure: f32,
) -> PhysicsForces {
    // Engine thrust with falloff at high speeds
    let base_thrust = BASE_THRUST_MULTIPLIER * aircraft.thrust; 
    let max_effective_ratio = aircraft.throttle + THRUST_HEADROOM; 
    let high_speed_falloff = (max_effective_ratio - airspeed_ratio).clamp(0.0, 1.0); 
    let engine_acceleration = aircraft.throttle * base_thrust * high_speed_falloff;
    
    // Lift and gravity interaction
    let gravity_acceleration_base = -climb_angle * aircraft.gravity;
    let lift_efficiency = (1.0 - climb_angle.abs()).max(LIFT_EFFICIENCY_MIN);
    let lift_force = aircraft.lift_coefficient * dynamic_pressure * lift_efficiency * aircraft.lift_reduction_factor;
    
    let gravity_acceleration = if climb_angle > 0.0 {
        let lift_reduction = lift_force * LIFT_REDUCTION_CLIMBING;
        let effective_gravity_accel = gravity_acceleration_base.abs() - lift_reduction;
        -effective_gravity_accel.max(0.0)
    } else {
        let lift_reduction = lift_force * LIFT_REDUCTION_DIVING;
        let effective_gravity_accel = gravity_acceleration_base.abs() - lift_reduction;
        effective_gravity_accel.max(0.0)
    };

    // Turn drag from G-forces
    let centripetal_accel = aircraft.speed * aircraft.pitch_velocity.abs();
    let g_force = centripetal_accel / G_FORCE_CONSTANT;
    let turn_drag = g_force * aircraft.g_force_drag * dynamic_pressure;

    // Parasitic drag
    let high_speed_multiplier = 1.0 + (airspeed_ratio.max(1.0) - 1.0) * 0.5;
    let parasitic_drag = dynamic_pressure * aircraft.parasitic_drag_coef * high_speed_multiplier;

    PhysicsForces {
        engine_acceleration,
        gravity_acceleration,
        turn_drag,
        parasitic_drag,
        wind_acceleration: 0.0,
    }
}

struct WindEffects {
    current_wind: Vec3,
    wind_acceleration: f32,
    macro_wind_pitch: f32,
    macro_wind_roll: f32,
    macro_wind_yaw: f32,
}

/// Calculate wind effects on the aircraft
fn calculate_wind_effects(
    wind: &Wind,
    pos: Vec3,
    time: f64,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
) -> WindEffects {
    let base_wind_velocity = wind.wind_direction * wind.wind_speed;
    let wind_drift = base_wind_velocity * time as f32;
    
    let sample_x = (pos.x - wind_drift.x) as f64 * wind.macro_wind_freq;
    let sample_z = (pos.z - wind_drift.z) as f64 * wind.macro_wind_freq;
    let weather_evolution = time * (wind.macro_wind_freq * wind.weather_evolution_rate); 
    
    // Wind intensity variation
    let wind_intensity_noise = wind.perlin.get([
        sample_x, 
        weather_evolution, 
        sample_z
    ]) as f32;
    let wind_multiplier = (wind_intensity_noise * 0.8) + 1.0; 

    // Wind direction variation
    let wind_dir_noise = wind.perlin.get([
        sample_x + WIND_NOISE_OFFSET_DIRECTION as f64, 
        weather_evolution + WIND_NOISE_OFFSET_DIRECTION as f64, 
        sample_z + WIND_NOISE_OFFSET_DIRECTION as f64
    ]) as f32;
    let angle_shift = wind_dir_noise * wind.max_angle_shift;
    
    let wind_rotation = Quat::from_rotation_y(angle_shift);
    let current_wind_dir = wind_rotation * wind.wind_direction;
    let current_speed = wind.wind_speed * wind_multiplier;
    let current_wind = current_wind_dir * current_speed;

    // Wind acceleration on forward movement
    let wind_dot = forward.dot(current_wind_dir);
    let wind_acceleration = wind_dot * current_speed * WIND_FORWARD_COUPLING;
    
    // Rotational effects from crosswinds and updrafts
    let wind_lateral = current_wind.dot(right);
    let wind_vertical = current_wind.dot(up);
    
    let wind_force_scale = (current_speed / 20.0).min(2.0);
    let macro_wind_coupling = 0.003 * wind_force_scale;
    
    let macro_wind_roll = -wind_lateral * macro_wind_coupling * 2.0;
    let macro_wind_yaw = wind_lateral * macro_wind_coupling * 0.2;
    let macro_wind_pitch = wind_vertical * macro_wind_coupling * 0.4;

    WindEffects {
        current_wind,
        wind_acceleration,
        macro_wind_pitch,
        macro_wind_roll,
        macro_wind_yaw,
    }
}

struct TurbulenceEffects {
    turbulence_force: Vec3,
    turbulence_velocity_scale: f32,
    turbulence_pitch: f32,
    turbulence_roll: f32,
    turbulence_yaw: f32,
    turbulence_scale: f32,
}

/// Calculate turbulence effects
fn calculate_turbulence(
    wind: &Wind,
    pos: Vec3,
    wind_drift: Vec3,
    time: f64,
    airspeed_ratio: f32,
) -> TurbulenceEffects {
    let freq = wind.turbulence_frequency as f64;
    let turb_sample_x = pos.x as f64 - wind_drift.x as f64;
    let turb_sample_z = pos.z as f64 - wind_drift.z as f64;
    let gust_freq = freq * wind.gust_frequency_multiplier;

    let turbulence_velocity_x = wind.perlin.get([
        turb_sample_x * gust_freq, 
        time * gust_freq, 
        turb_sample_z * gust_freq + TURBULENCE_NOISE_OFFSET_BASE
    ]) as f32;
    let turbulence_velocity_y = wind.perlin.get([
        pos.y as f64 * gust_freq, 
        time * gust_freq, 
        turb_sample_x * gust_freq + TURBULENCE_NOISE_OFFSET_BASE + 100.0
    ]) as f32;
    let turbulence_velocity_z = wind.perlin.get([
        turb_sample_z * gust_freq, 
        time * gust_freq, 
        pos.y as f64 * gust_freq + TURBULENCE_NOISE_OFFSET_BASE + 200.0
    ]) as f32;
    
    let turbulence_force = Vec3::new(turbulence_velocity_x, turbulence_velocity_y, turbulence_velocity_z);
    let turbulence_velocity_scale = wind.turbulence_intensity * TURBULENCE_VELOCITY_MULTIPLIER 
        * (airspeed_ratio + 0.5).powf(TURBULENCE_POWER);
    
    // Rotational coupling
    let turbulence_pitch = turbulence_velocity_y * TURBULENCE_COUPLING_STRENGTH * 0.5;
    let turbulence_roll = -turbulence_velocity_x * TURBULENCE_COUPLING_STRENGTH * 2.0;
    let turbulence_yaw = turbulence_velocity_x * TURBULENCE_COUPLING_STRENGTH * 0.75;
    
    let turbulence_scale = wind.turbulence_intensity * (airspeed_ratio + 1.0).powf(TURBULENCE_POWER);

    TurbulenceEffects {
        turbulence_force,
        turbulence_velocity_scale,
        turbulence_pitch,
        turbulence_roll,
        turbulence_yaw,
        turbulence_scale,
    }
}

/// Handle player flight control inputs
fn handle_flight_controls(
    keyboard: &ButtonInput<KeyCode>,
    aircraft: &mut Aircraft,
    transform: &Transform,
    pitch_strength: f32,
    roll_strength: f32,
    yaw_strength: f32,
    control_effectiveness: f32,
    dt: f32,
) {
    let mut is_rolling = false;
    let mut is_pitching = false;

    // Pitch controls
    if keyboard.pressed(KeyCode::KeyW) { 
        aircraft.pitch_velocity -= pitch_strength * dt;
        is_pitching = true;
    }
    if keyboard.pressed(KeyCode::KeyS) { 
        aircraft.pitch_velocity += pitch_strength * dt;
        is_pitching = true;
    }
    
    // Roll controls
    if keyboard.pressed(KeyCode::KeyA) { 
        aircraft.roll_velocity += roll_strength * dt; 
        is_rolling = true;
    }
    if keyboard.pressed(KeyCode::KeyD) { 
        aircraft.roll_velocity -= roll_strength * dt; 
        is_rolling = true;
    }
    
    // Yaw controls
    if keyboard.pressed(KeyCode::KeyQ) { aircraft.yaw_velocity += yaw_strength * dt; }
    if keyboard.pressed(KeyCode::KeyE) { aircraft.yaw_velocity -= yaw_strength * dt; }

    // Auto-stabilization
    apply_stability_assists(aircraft, transform, control_effectiveness, is_rolling, is_pitching, dt);
}

/// Apply stability and flight assists
fn apply_stability_assists(
    aircraft: &mut Aircraft,
    transform: &Transform,
    control_effectiveness: f32,
    is_rolling: bool,
    is_pitching: bool,
    dt: f32,
) {
    let bank_angle = transform.right().y;
    let pitch_angle = transform.forward().y;
    
    // Bank-to-turn coordination
    aircraft.yaw_velocity += bank_angle * aircraft.bank_turn_strength * control_effectiveness * dt;

    // Auto-level when not actively rolling
    if !is_rolling {
        let auto_level_force = -bank_angle * aircraft.auto_level_strength * control_effectiveness;
        aircraft.roll_velocity += auto_level_force * dt;
    }

    // Auto-level when not actively pitching
    if !is_pitching {
        let auto_level_force = -pitch_angle * aircraft.auto_level_strength * control_effectiveness;
        aircraft.pitch_velocity += auto_level_force / AUTO_LEVEL_PITCH_DIVISOR * dt;
    }
}

/// Apply stall behavior at low speeds
fn apply_stall_behavior(aircraft: &mut Aircraft, transform: &Transform, airspeed_ratio: f32, dt: f32) {
    if aircraft.speed < aircraft.max_speed * STALL_THRESHOLD_RATIO {
        let stall_strength = (1.0 - airspeed_ratio).max(0.0);
        let stall_pitch_down = stall_strength * if transform.up().y.is_sign_negative() {
            -1.0
        } else {
            1.0
        };
        aircraft.pitch_velocity -= stall_pitch_down * dt;
    }
}

/// Apply all movement and rotation to the aircraft
fn apply_aircraft_movement(
    aircraft: &mut Aircraft,
    transform: &mut Transform,
    forward: Vec3,
    current_wind: Vec3,
    turbulence_force: Vec3,
    turbulence_velocity_scale: f32,
    dt: f32,
) {
    // Apply rotational damping
    aircraft.pitch_velocity -= aircraft.pitch_velocity * ROTATIONAL_DAMPING * dt;
    aircraft.roll_velocity -= aircraft.roll_velocity * ROTATIONAL_DAMPING * dt;
    aircraft.yaw_velocity -= aircraft.yaw_velocity * ROTATIONAL_DAMPING * dt;

    // Apply rotation
    transform.rotate_local_x(aircraft.pitch_velocity * dt);
    transform.rotate_local_z(aircraft.roll_velocity * dt);
    transform.rotate_local_y(aircraft.yaw_velocity * dt);

    // Calculate final movement vector
    let gravity_factor = (1.0 - (aircraft.speed / LIFT_THRESHOLD_SPEED)).max(0.0);
    let mut movement = forward * aircraft.speed;
    movement.y -= GRAVITY_STRENGTH * gravity_factor;
    movement += current_wind * WIND_LATERAL_COUPLING; 
    movement += turbulence_force * turbulence_velocity_scale;

    aircraft.velocity = movement;
    transform.translation += movement * dt;
}

/// Handle free flight camera controls
fn handle_free_flight_camera(
    keyboard: &ButtonInput<KeyCode>,
    camera_transform: &mut Transform,
    dt: f32,
) {
    let pan_speed = if keyboard.pressed(KeyCode::ShiftLeft) { 
        FREE_FLIGHT_PAN_SPEED_FAST 
    } else { 
        FREE_FLIGHT_PAN_SPEED_NORMAL 
    };
    
    let forward = camera_transform.forward().as_vec3();
    let right = camera_transform.right().as_vec3();
    let up = camera_transform.up().as_vec3();
    let mut pan_direction = Vec3::ZERO;

    // Movement controls
    if keyboard.pressed(KeyCode::KeyW) { pan_direction += forward; }
    if keyboard.pressed(KeyCode::KeyS) { pan_direction -= forward; }
    if keyboard.pressed(KeyCode::KeyA) { pan_direction -= right; }
    if keyboard.pressed(KeyCode::KeyD) { pan_direction += right; }
    if keyboard.pressed(KeyCode::KeyE) { pan_direction += up; }
    if keyboard.pressed(KeyCode::KeyQ) { pan_direction -= up; }

    // Rotation controls
    let panning_delta = FREE_FLIGHT_ROTATION_SPEED * dt;
    if keyboard.pressed(KeyCode::ArrowLeft) { camera_transform.rotate_y(panning_delta); }
    if keyboard.pressed(KeyCode::ArrowRight) { camera_transform.rotate_y(-panning_delta); }
    if keyboard.pressed(KeyCode::ArrowUp) { camera_transform.rotate_local_x(panning_delta); }
    if keyboard.pressed(KeyCode::ArrowDown) { camera_transform.rotate_local_x(-panning_delta); }
    if keyboard.pressed(KeyCode::KeyZ) { camera_transform.rotate_local_z(panning_delta); }
    if keyboard.pressed(KeyCode::KeyX) { camera_transform.rotate_local_z(-panning_delta); }

    camera_transform.translation += pan_direction.normalize_or_zero() * pan_speed * dt;
}

/// Main camera and aircraft control system
pub fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut wire_frame: ResMut<WireframeConfig>,
    mut control_mode: ResMut<ControlMode>,
    wind: Res<Wind>,
    world_gen: Res<WorldGenerator>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    mut aircraft_query: Query<(&mut Transform, &mut Aircraft), (With<Aircraft>, Without<MainCamera>)>,
) {
    let dt = time.delta_secs();

    // Handle input toggles first - need special handling for respawn
    if let Ok((mut plane_transform, mut aircraft)) = aircraft_query.single_mut() {
        handle_input_toggles(&keyboard, &mut wire_frame, &mut control_mode, Some(&mut aircraft), Some(&mut plane_transform), Some(&world_gen));
    } else {
        handle_input_toggles(&keyboard, &mut wire_frame, &mut control_mode, None, None, None);
    }

    // Aircraft physics
    if !control_mode.physics_paused {
        if let Ok((mut plane_transform, mut aircraft)) = aircraft_query.single_mut() {
            let pos = plane_transform.translation;
            let time_elapsed = time.elapsed_secs_f64();

            update_throttle(&keyboard, &mut aircraft, dt);

            let forward = plane_transform.forward().as_vec3();
            let right = plane_transform.right().as_vec3();
            let up = plane_transform.up().as_vec3();
            let climb_angle = forward.y;
            
            let airspeed_ratio = aircraft.speed / aircraft.max_speed;
            let dynamic_pressure = airspeed_ratio.powi(2);

            // Calculate forces
            let mut forces = calculate_engine_and_drag(&aircraft, climb_angle, airspeed_ratio, dynamic_pressure);
            let wind_effects = calculate_wind_effects(&wind, pos, time_elapsed, forward, right, up);
            forces.wind_acceleration = wind_effects.wind_acceleration;

            let wind_drift = wind.wind_direction * wind.wind_speed * time_elapsed as f32;
            let turbulence = calculate_turbulence(&wind, pos, wind_drift, time_elapsed, airspeed_ratio);

            // Apply speed changes
            aircraft.speed += (
                forces.engine_acceleration + 
                forces.gravity_acceleration - 
                forces.turn_drag - 
                forces.parasitic_drag + 
                forces.wind_acceleration
            ) * dt;
            aircraft.speed = aircraft.speed.max(0.0);

            // Handle player input and stabilization
            let control_effectiveness = get_control_effectiveness(airspeed_ratio);
            let pitch_strength = aircraft.pitch_strength * control_effectiveness;
            let roll_strength = aircraft.roll_strength * control_effectiveness;
            let yaw_strength = aircraft.yaw_strength * control_effectiveness;

            if control_mode.mode == FlightMode::Aircraft || control_mode.mode == FlightMode::Orbit {
                handle_flight_controls(
                    &keyboard, 
                    &mut aircraft, 
                    &plane_transform, 
                    pitch_strength, 
                    roll_strength, 
                    yaw_strength, 
                    control_effectiveness, 
                    dt
                );
            }

            // Apply environmental effects
            aircraft.pitch_velocity += (wind_effects.macro_wind_pitch + turbulence.turbulence_pitch * turbulence.turbulence_scale) * dt;
            aircraft.roll_velocity += (wind_effects.macro_wind_roll + turbulence.turbulence_roll * turbulence.turbulence_scale) * dt;
            aircraft.yaw_velocity += (wind_effects.macro_wind_yaw + turbulence.turbulence_yaw * turbulence.turbulence_scale) * dt;

            apply_stall_behavior(&mut aircraft, &plane_transform, airspeed_ratio, dt);
            apply_aircraft_movement(
                &mut aircraft, 
                &mut plane_transform, 
                forward, 
                wind_effects.current_wind, 
                turbulence.turbulence_force, 
                turbulence.turbulence_velocity_scale, 
                dt
            );

            // Terrain and water collision detection
            let aircraft_pos = plane_transform.translation;
            let terrain_height = world_gen.get_terrain_height(&[aircraft_pos.x, aircraft_pos.y, aircraft_pos.z]);
            
            if (aircraft_pos.y <= terrain_height || aircraft_pos.y <= 0.0) && !aircraft.crashed {
                aircraft.crashed = true;
                aircraft.speed = 0.0;
                aircraft.velocity = Vec3::ZERO;
                aircraft.pitch_velocity = 0.0;
                aircraft.roll_velocity = 0.0;
                aircraft.yaw_velocity = 0.0;
                plane_transform.translation.y = terrain_height.max(0.0);
                control_mode.physics_paused = true;
                
                if aircraft_pos.y <= 0.0 {
                    info!("Aircraft crashed into water at position: [{:.1}, {:.1}, {:.1}]", aircraft_pos.x, aircraft_pos.y, aircraft_pos.z);
                } else {
                    info!("Aircraft crashed into terrain at position: [{:.1}, {:.1}, {:.1}]", aircraft_pos.x, aircraft_pos.y, aircraft_pos.z);
                }
            }

            // Prevent further movement if crashed
            if aircraft.crashed {
                aircraft.speed = 0.0;
                aircraft.velocity = Vec3::ZERO;
            }
        }
    }

    // Free flight camera controls
    if control_mode.mode == FlightMode::FreeFlight {
        if let Ok(mut camera_transform) = camera_query.single_mut() {
            handle_free_flight_camera(&keyboard, &mut camera_transform, dt);
        }
    }
}

/// Evolve wind direction and speed over time
pub fn evolve_wind(
    mut wind: ResMut<Wind>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let evolution_speed = wind.wind_evolution_speed;
    
    // Direction evolution
    let yaw_noise = wind.perlin.get([t * evolution_speed, 0.0, 0.0]) as f32;
    let pitch_noise = wind.perlin.get([0.0, t * evolution_speed, 1000.0]) as f32;
    
    let yaw_rotation_speed = yaw_noise * 0.5;
    let pitch_rotation_speed = pitch_noise * 0.1;
    
    let yaw_rotation = Quat::from_rotation_y(yaw_rotation_speed * dt);
    let right_axis = Vec3::new(-wind.wind_direction.z, 0.0, wind.wind_direction.x).normalize_or_zero();
    let pitch_rotation = if right_axis.length_squared() > 0.001 {
        Quat::from_axis_angle(right_axis, pitch_rotation_speed * dt)
    } else {
        Quat::IDENTITY
    };
    
    wind.wind_direction = (yaw_rotation * pitch_rotation * wind.wind_direction).normalize();
    
    // Speed evolution
    let speed_noise = wind.perlin.get([t * evolution_speed + 5000.0, t * evolution_speed + 5000.0, 0.0]) as f32;
    let speed_range = wind.max_wind_speed - wind.min_wind_speed;
    wind.wind_speed = wind.min_wind_speed + (speed_noise + 1.0) * 0.5 * speed_range;
}

/// Camera follow system for orbit and chase modes
pub fn camera_follow_aircraft(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    control_mode: Res<ControlMode>,
    aircraft_query: Query<(&Transform, &Aircraft), (With<Aircraft>, Without<MainCamera>)>,
    mut camera_query: Query<(&mut Transform, &mut MainCamera)>,
) {
    if control_mode.mode == FlightMode::FreeFlight || control_mode.physics_paused {
        return;
    }

    let Ok((plane_transform, aircraft)) = aircraft_query.single() else { return; };
    let Ok((mut camera_transform, mut main_camera)) = camera_query.single_mut() else { return; };

    // Zoom controls
    if control_mode.mode == FlightMode::Orbit || control_mode.mode == FlightMode::Aircraft {
        if keyboard.pressed(KeyCode::KeyX) { 
            main_camera.orbit_distance -= CAMERA_ZOOM_SPEED * time.delta_secs(); 
        }
        if keyboard.pressed(KeyCode::KeyZ) { 
            main_camera.orbit_distance += CAMERA_ZOOM_SPEED * time.delta_secs(); 
        }
        main_camera.orbit_distance = main_camera.orbit_distance.clamp(CAMERA_ZOOM_MIN, CAMERA_ZOOM_MAX);
    }

    // Calculate camera positioning
    let mut actual_direction = aircraft.velocity.normalize_or_zero();
    if actual_direction == Vec3::ZERO {
        actual_direction = plane_transform.forward().into();
    }

    let plane_movement = aircraft.velocity * time.delta_secs();
    camera_transform.translation += plane_movement;

    let speed_ratio = (aircraft.speed / CAMERA_SPEED_THRESHOLD).clamp(0.0, 2.0);
    let dynamic_distance = main_camera.orbit_distance + (CAMERA_MAX_EXTRA_DISTANCE * speed_ratio.powf(0.5));
    let smoothness = CAMERA_SMOOTHNESS_BASE + (speed_ratio * CAMERA_SMOOTHNESS_MULTIPLIER);
    let t = (time.delta_secs() * smoothness).min(1.0);

    // Apply camera behavior based on mode
    match control_mode.mode {
        FlightMode::Orbit => {
            if keyboard.pressed(KeyCode::ArrowLeft) { 
                main_camera.orbit_yaw -= ORBIT_ROTATION_SPEED * time.delta_secs(); 
            }
            if keyboard.pressed(KeyCode::ArrowRight) { 
                main_camera.orbit_yaw += ORBIT_ROTATION_SPEED * time.delta_secs(); 
            }
            if keyboard.pressed(KeyCode::ArrowDown) { 
                main_camera.orbit_pitch -= ORBIT_ROTATION_SPEED * time.delta_secs(); 
            }
            if keyboard.pressed(KeyCode::ArrowUp) { 
                main_camera.orbit_pitch += ORBIT_ROTATION_SPEED * time.delta_secs(); 
            }

            main_camera.orbit_pitch = main_camera.orbit_pitch.clamp(ORBIT_PITCH_MIN, ORBIT_PITCH_MAX);

            let world_up = Vec3::Y;
            let world_right = Vec3::X;
            let orbit_rotation = Quat::from_axis_angle(world_up, main_camera.orbit_yaw) 
                               * Quat::from_axis_angle(world_right, main_camera.orbit_pitch);

            let base_offset = Vec3::new(0.0, CAMERA_HEIGHT * 1.5, -dynamic_distance * 1.5);
            let rotated_offset = orbit_rotation * base_offset;

            let target_position = plane_transform.translation + rotated_offset;
            camera_transform.translation = camera_transform.translation.lerp(target_position, t);

            let target_rotation = camera_transform.looking_at(plane_transform.translation, world_up).rotation;
            camera_transform.rotation = target_rotation;
        }
        FlightMode::Aircraft => {
            let target_position = plane_transform.translation 
                + (-actual_direction * dynamic_distance) 
                + (plane_transform.up() * CAMERA_HEIGHT);
            camera_transform.translation = camera_transform.translation.lerp(target_position, t);

            let look_ahead_distance = CAMERA_LOOK_AHEAD_MULTIPLIER * aircraft.speed;
            let look_target = plane_transform.translation + (actual_direction * look_ahead_distance);
            
            let target_rotation = camera_transform.looking_at(look_target, plane_transform.up()).rotation;
            camera_transform.rotation = camera_transform.rotation.slerp(target_rotation, t);
        }
        FlightMode::FreeFlight => unreachable!(),
    }
}

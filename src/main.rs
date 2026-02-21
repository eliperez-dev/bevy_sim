use bevy::{
    color::palettes::css::WHITE,
    light::CascadeShadowConfigBuilder,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    platform::collections::HashSet,
    prelude::*, 
    render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}},
    camera::ClearColorConfig,
    window::{PresentMode, WindowPlugin},
    diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore},
};

use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

use world_generation::*;
use consts::*;
use day_cycle::*;
use controls::*;
use hud::*;

mod world_generation;
mod consts;
mod day_cycle;
mod controls;
mod hud;
mod network;

// Temperature conversion constants
const TEMP_SCALE: f32 = 100.0;
const TEMP_OFFSET: f32 = 10.0;
const FAHRENHEIT_TO_CELSIUS_OFFSET: f32 = 32.0;
const FAHRENHEIT_TO_CELSIUS_RATIO: f32 = 5.0 / 9.0;

// Time format constants
const HOURS_PER_DAY: f32 = 24.0;
const TIME_OFFSET_HOURS: f32 = 4.0;
const MINUTES_PER_HOUR: f32 = 60.0;

// FPS update interval
const FPS_UPDATE_INTERVAL: f32 = 0.5;

// UI precision
const TEMP_PRECISION: f32 = 10.0;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
            WireframePlugin::default(),
        ))
        .insert_resource(WireframeConfig {
            global: false,
            default_color: WHITE.into(),
        })
        .insert_resource(ChunkManager {
            spawned_chunks: HashSet::new(),
            last_camera_chunk: None,
            to_spawn: Vec::new(),
            lod_to_update: Vec::new(),
            render_distance: 40,
            lod_levels: [
                (0.70, 20),
                (1.25, 15),
                (2.0, 8),
                (3.0, 3),
                (4.0, 1),
            ],
            lod_quality_multiplier: 1,
            lod_distance_multiplier: 10.0,
        })
        .insert_resource(RenderSettings {
            cascades: 0,
            just_updated: false,
            terrain_smoothness: 0.0,
            compute_smooth_normals: false,
        })
        .init_resource::<WorldGenerationSettings>()
        .insert_resource(DayNightCycle {
            time_of_day: 0.50,
            speed: 0.01,  
            inclination: -1.0,     
        })
        .init_resource::<ControlMode>()
        .init_resource::<Wind>()
        .init_resource::<hud::MultiplayerMenu>()
        .add_plugins(EguiPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_observer(network::spawn_remote_player)
        .add_observer(network::update_remote_player)
        .add_observer(network::despawn_remote_player)
        .add_observer(network::teleport_to_player)
        .add_systems(Startup, setup_camera_system)
        .add_systems(EguiPrimaryContextPass, (debugger_ui, flight_hud_system))
        .add_systems(Startup, (setup, setup_camera_fog, spawn_stars, hud::auto_connect_on_startup).chain())
        .add_systems(Update, (
            evolve_wind,
            camera_controls, 
            update_debugger, 
            generate_chunks, 
            modify_plane, 
            handle_compute_tasks, 
            update_chunk_lod, 
            update_daylight_cycle,
            draw_lod_rings,
            network::send_player_updates,
            network::receive_server_messages,
            network::lerp_remote_players,
            network::update_player_labels,
            hud::process_connection_results,
        ))
        .add_systems(PostUpdate, (
            despawn_out_of_bounds_chunks,
            camera_follow_aircraft,
        ))
        .run();
}

#[derive(Resource)]
pub struct RenderSettings {
    cascades: usize,
    just_updated: bool,
    terrain_smoothness: f32,
    compute_smooth_normals: bool,
}

fn setup_camera_system(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cascade_shadow_config = CascadeShadowConfigBuilder::default().build();

    let world_gen = WorldGenerator::new(3);
    let spawn_pos = [0.0, 0.0, 0.0];
    let terrain_height = world_gen.get_terrain_height(&spawn_pos);
    let spawn_height = (terrain_height + RESPAWN_HEIGHT).max(RESPAWN_HEIGHT);
    
    commands.insert_resource(world_gen);
    
    commands.insert_resource(SharedChunkMaterials {
        terrain_material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            ..default()
        }),
        water_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.15, 0.35, 0.7, 0.90),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            metallic: 0.1,
            ..default()
        }),
    });

    let sun_mesh = meshes.add(Sphere::new(300.0));
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.8),
        emissive: LinearRgba::rgb(100.0, 80.0, 20.0),
        fog_enabled: false,
        ..default()
    });

    commands.spawn((
        Mesh3d(sun_mesh),
        MeshMaterial3d(sun_material),
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            illuminance: MAX_ILLUMANENCE, 
            ..default()
        },
        Transform::default(), 
        cascade_shadow_config,
        bevy::camera::visibility::NoFrustumCulling,
        Sun, 
    ));

    let plane_entity = commands.spawn((
        Aircraft::default(),
        Transform::from_xyz(0.0, spawn_height, 0.0).with_scale(Vec3::splat(0.1)),
        Visibility::default(),
        InheritedVisibility::default(),
    )).id();

    let model_correction = commands.spawn(SceneRoot(
        asset_server.load("low-poly_airplane/scene.gltf#Scene0")
    )).insert(Transform::from_rotation(
        Quat::from_rotation_y((180.0f32).to_radians()) 
    )).id();

    commands.entity(plane_entity).add_child(model_correction);
    
    commands.spawn((
        Text::new("Pos: N/A"),
        Node {
            position_type: PositionType::Absolute,
            top: px(12.0),
            left: px(12.0),
            ..default()
        },
        Debugger
    ));
}

fn setup_camera_fog(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            far: 50000.0,
            ..default()
        }),
        MainCamera::default(),
        Transform::from_xyz(0.0, 4000.0, 0.0).looking_at(Vec3::new(5000.0, 3000.0, 5000.0), Vec3::Y),
        DistanceFog {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0), 
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.1), 
            directional_light_exponent: 1000.0, 
            falloff: FogFalloff::ExponentialSquared{ 
                density: 0.000045, 
            },
        }, 
        AmbientLight {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            brightness: 2000.0,
            affects_lightmapped_meshes: false,
        },
    ));
}

/// Convert normalized temperature (0-1) to Fahrenheit and Celsius
fn map_temperature(t: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0);
    let fahrenheit = t * TEMP_SCALE + TEMP_OFFSET;
    let celsius = (fahrenheit - FAHRENHEIT_TO_CELSIUS_OFFSET) * FAHRENHEIT_TO_CELSIUS_RATIO;
    ((fahrenheit * TEMP_PRECISION).round() / TEMP_PRECISION, (celsius * TEMP_PRECISION).round() / TEMP_PRECISION)
}

/// Format time of day (0-1) as HH:MM
fn format_game_time(t: f32) -> String {
    let total_hours = (t * HOURS_PER_DAY - TIME_OFFSET_HOURS) % HOURS_PER_DAY;
    let hours = total_hours.floor() as u32;
    let minutes = ((total_hours - hours as f32) * MINUTES_PER_HOUR).round() as u32;

    if minutes == 60 {
        format!("{:02}:00", (hours + 1) % 24)
    } else {
        format!("{:02}:{:02}", hours, minutes)
    }
}

#[derive(Component)]
struct Debugger;

fn update_debugger(
    camera: Query<&Transform, With<MainCamera>>,
    world: Res<WorldGenerator>,
    chunks: Res<ChunkManager>,
    cycle: Res<DayNightCycle>,
    control_mode: Res<ControlMode>,
    mut debugger: Query<&mut Text, With<Debugger>>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut last_update: Local<f32>,
    mut cached_fps: Local<f32>,
) {
    if time.elapsed_secs() - *last_update >= FPS_UPDATE_INTERVAL {
        *last_update = time.elapsed_secs();
        *cached_fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
            .unwrap_or(0.0) as f32;
    }

    let Ok(camera_transform) = camera.single() else { return };
    let cam_trans = camera_transform.translation;
    let camera_pos_arr = [cam_trans.x, cam_trans.y, cam_trans.z];
    
    let biome = world.get_biome(&camera_pos_arr);
    let climate = world.get_climate(&camera_pos_arr);

    let Ok(mut text_component) = debugger.single_mut() else { return };
    let message = &mut text_component.0;

    message.clear();
    message.push_str(&format!("FPS: {:.0}\n", *cached_fps));
    message.push_str(&format!("Position: [{:.0}, {:.0}, {:.0}]\n", cam_trans.x.round(), cam_trans.y.round(), cam_trans.z.round()));
    message.push_str(&format!("Biome: {:?} | Tempature: {:?}F / {:?}C\n", biome, map_temperature(climate.0).0, map_temperature(climate.0).1));
    message.push_str(&format!("Chunks: {} | Time: {}\n", chunks.spawned_chunks.len(), format_game_time(cycle.time_of_day)));

    message.push_str("\n--- CONTROLS ---\n");
    message.push_str(&format!("Camera Mode: {:?} (Press F to toggle)\n", control_mode.mode));
    message.push_str("T: Toggle Wireframe\n");
    message.push_str("P: Pause Plane Physics\n");
    
    match control_mode.mode {
        FlightMode::FreeFlight => {
            message.push_str("WASD: Move Horizontal\n");
            message.push_str("E / Q: Move Up / Down\n");
            message.push_str("Shift: Fast Move Speed\n");
            message.push_str("Arrows: Look Around (Pitch/Yaw)\n");
            message.push_str("Z / X: Roll Camera\n");
        },
        FlightMode::Orbit => {
            message.push_str("W / S: Pitch Down/Up\n");
            message.push_str("A / D: Roll (Turn)\n");
            message.push_str("Q / E: Yaw (Rudder)\n");
            message.push_str("= / -: Throttle Up/Down\n");
            message.push_str("Arrows: Orbit Camera\n");
            message.push_str("Z / X: Zoom Camera In/Out\n");
        },
        FlightMode::Aircraft => {
            message.push_str("W / S: Pitch Down/Up\n");
            message.push_str("A / D: Roll (Turn)\n");
            message.push_str("Q / E: Yaw (Rudder)\n");
            message.push_str("= / -: Throttle Up/Down\n");
        }
    }
}

/// Display aircraft physics telemetry and tuning controls
fn ui_aircraft_physics(ui: &mut egui::Ui, aircraft: &mut Aircraft) {
    ui.group(|ui| {
        ui.label(egui::RichText::new("Live Telemetry").strong());
        let airspeed_ratio = aircraft.speed / aircraft.max_speed;
        let centripetal_accel = aircraft.speed * aircraft.pitch_velocity.abs();
        let g_force = centripetal_accel / 9.8;
        let turn_drag = g_force * aircraft.g_force_drag;
        let speed_drag = airspeed_ratio.powi(2) * 20.0;
        
        ui.label(format!("Speed: {:.1} ({:.0}%)", aircraft.speed, airspeed_ratio * 100.0));
        ui.label(format!("Control Effect: {:.0}%", get_control_effectiveness(airspeed_ratio) * 100.0));
        ui.label(format!("G-Force: {:.2}G", g_force));
        ui.label(format!("Turn Drag / Speed Drag: {:.2} / {:.2}", turn_drag, speed_drag));
    });
    
    ui.label(egui::RichText::new("Flight Model Tuning").strong());
    
    ui.label("Engine & Drag");
    ui.add(egui::Slider::new(&mut aircraft.max_speed, 50.0..=1000.0).text("Max Speed"));
    ui.add(egui::Slider::new(&mut aircraft.thrust, 0.1..=2.0).text("Engine Response"));
    ui.add(egui::Slider::new(&mut aircraft.parasitic_drag_coef, 0.0..=50.0).text("Parasitic Drag"));
    ui.add(egui::Slider::new(&mut aircraft.g_force_drag, 0.0..=10.0).text("G-Force Drag"));
    
    ui.separator();
    ui.label("Lift & Gravity");
    ui.add(egui::Slider::new(&mut aircraft.gravity, 0.0..=300.0).text("Gravity Force"));
    ui.add(egui::Slider::new(&mut aircraft.lift_coefficient, 0.0..=10.0).text("Lift Coefficient"));
    ui.add(egui::Slider::new(&mut aircraft.lift_reduction_factor, 0.0..=100.0).text("Lift Reduction Factor"));
    
    ui.separator();
    ui.label("Responsiveness & Assists");
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut aircraft.pitch_strength, 0.5..=10.0).text("Pitch"));
        ui.add(egui::Slider::new(&mut aircraft.roll_strength, 0.5..=10.0).text("Roll"));
        ui.add(egui::Slider::new(&mut aircraft.yaw_strength, 0.5..=10.0).text("Yaw"));
    });
    ui.add(egui::Slider::new(&mut aircraft.bank_turn_strength, 0.0..=5.0).text("Auto-Turn (Bank)"));
    ui.add(egui::Slider::new(&mut aircraft.auto_level_strength, 0.0..=5.0).text("Auto-Level (Stability)"));
}

/// Display wind and weather controls
fn ui_wind_weather(ui: &mut egui::Ui, wind: &mut Wind) {
    ui.label(egui::RichText::new("Base Wind Evolution (Time-based)").strong());
    ui.add(egui::Slider::new(&mut wind.wind_evolution_speed, 0.0..=0.5).text("Evolution Speed"));
    ui.add(egui::Slider::new(&mut wind.min_wind_speed, 0.0..=100.0).text("Min Wind Speed"));
    ui.add(egui::Slider::new(&mut wind.max_wind_speed, 0.0..=200.0).text("Max Wind Speed"));
    
    ui.separator();
    
    ui.label(egui::RichText::new("Current Base Wind (Live)").strong());
    ui.horizontal(|ui| {
        ui.label(format!("Direction: [{:.2}, {:.2}, {:.2}]", 
            wind.wind_direction.x, wind.wind_direction.y, wind.wind_direction.z));
    });
    ui.label(format!("Speed: {:.1}", wind.wind_speed));

    ui.separator();

    ui.label(egui::RichText::new("Macro Weather (Large Fronts)").strong());
    ui.add(egui::Slider::new(&mut wind.macro_wind_freq, 0.001..=0.05).text("Pattern Size (Freq)"));
    ui.add(egui::Slider::new(&mut wind.weather_evolution_rate, 0.0..=1.0).text("Evolution Rate"));
    
    let mut angle_deg = wind.max_angle_shift.to_degrees();
    if ui.add(egui::Slider::new(&mut angle_deg, 0.0..=180.0).text("Max Direction Shift (Deg)")).changed() {
        wind.max_angle_shift = angle_deg.to_radians();
    }

    ui.separator();

    ui.label(egui::RichText::new("Micro Turbulence (Gusts)").strong());
    ui.add(egui::Slider::new(&mut wind.turbulence_intensity, 0.0..=0.10).text("Intensity").logarithmic(true));
    ui.add(egui::Slider::new(&mut wind.turbulence_frequency, 0.1..=10.0).text("Frequency"));
    ui.add(egui::Slider::new(&mut wind.gust_frequency_multiplier, 0.0001..=0.01).text("Gust Multiplier"));
}

/// Display world and time controls
fn ui_world_time(ui: &mut egui::Ui, day_cycle: &mut DayNightCycle) {
    ui.add(egui::Slider::new(&mut day_cycle.time_of_day, 0.0..=1.0).text("Time of Day"));
    ui.add(egui::Slider::new(&mut day_cycle.speed, 0.0..=0.1).text("Time Speed"));
    ui.add(egui::Slider::new(&mut day_cycle.inclination, -1.0..=1.0).text("Inclination"));
}

/// Display render settings controls
fn ui_render_settings(
    ui: &mut egui::Ui,
    wireframe_config: &mut WireframeConfig,
    chunk_manager: &mut ChunkManager,
    world_settings: &mut WorldGenerationSettings,
    render_settings: &mut RenderSettings,
    fog_query: &mut Query<&mut DistanceFog, With<MainCamera>>,
) {
    ui.checkbox(&mut wireframe_config.global, "Global Wireframe");

    if ui.add(egui::Slider::new(&mut chunk_manager.render_distance, 2..=150).text("Render Distance")).changed() {
        render_settings.just_updated = true;
    }
    ui.add(egui::Slider::new(&mut world_settings.max_chunks_per_frame, 1..=500).text("Max Gen / Frame"));

    if ui.add(egui::Slider::new(&mut render_settings.cascades, 0..=4).text("Cascades")).changed() {
        render_settings.just_updated = true;
    }
    if ui.add(egui::Slider::new(&mut render_settings.terrain_smoothness, 0.0..=1.0).text("Terrain Smoothness")).changed() {
        render_settings.just_updated = true;
    }
    if ui.checkbox(&mut render_settings.compute_smooth_normals, "Smooth Normals").changed() {
        render_settings.just_updated = true;
    }
    if ui.add(egui::Slider::new(&mut chunk_manager.lod_quality_multiplier, 1..=4).text("LOD Quality")).changed() {
        render_settings.just_updated = true;
    }
    if ui.add(egui::Slider::new(&mut chunk_manager.lod_distance_multiplier, 1.0..=25.0).text("LOD Distance")).changed() {
        render_settings.just_updated = true;
    }

    if let Ok(mut fog) = fog_query.single_mut() {
         if let FogFalloff::ExponentialSquared { density } = &mut fog.falloff {
            ui.add(egui::Slider::new(density, 0.000005..=0.001).text("Fog Density").logarithmic(true));
        }
    }
}

/// Main debugger UI system
pub fn debugger_ui(
    mut contexts: EguiContexts,
    mut day_cycle: ResMut<DayNightCycle>,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut world_settings: ResMut<WorldGenerationSettings>,
    mut fog_query: Query<&mut DistanceFog, With<MainCamera>>,
    mut render_settings: ResMut<RenderSettings>,
    mut aircraft_query: Query<&mut Aircraft, Without<MainCamera>>,
    mut wind: ResMut<Wind>,
    client: Option<Res<network::NetworkClient>>,
    remote_players: Query<(&network::RemotePlayer, &GlobalTransform)>,
    mut commands: Commands,
    mut menu: ResMut<hud::MultiplayerMenu>,
) -> Result<(), > { 
    egui::Window::new("Simulation Debugger").show(contexts.ctx_mut()?, |ui| {

        ui.collapsing("✈ Aircraft Physics", |ui| {
            if let Ok(mut aircraft) = aircraft_query.single_mut() {
                ui_aircraft_physics(ui, &mut aircraft);
            } else {
                ui.label(egui::RichText::new("No Aircraft found in scene.").color(egui::Color32::RED));
            }
        });

        ui.collapsing("🌪 Wind & Weather", |ui| {
            ui_wind_weather(ui, &mut wind);
        });

        ui.collapsing("🌍 World & Time", |ui| {
            ui_world_time(ui, &mut day_cycle);
        });

        ui.collapsing("📷 Render Settings", |ui| {
            ui_render_settings(
                ui, 
                &mut wireframe_config, 
                &mut chunk_manager, 
                &mut world_settings, 
                &mut render_settings, 
                &mut fog_query
            );
        });

        ui.collapsing("🌐 Multiplayer", |ui| {
            if let Some(client) = &client {
                if client.connected {
                    ui.label(format!("🟢 Connected (Player ID: {})", client.player_id.unwrap_or(0)));
                    
                    if let Some(seed) = client.world_seed {
                        ui.label(format!("World Seed: {}", seed));
                    }
                    
                    ui.separator();
                    ui.label(egui::RichText::new("Remote Players").strong());
                    
                    if remote_players.is_empty() {
                        ui.label("No other players connected");
                    } else {
                        for (remote_player, transform) in remote_players.iter() {
                            ui.horizontal(|ui| {
                                ui.label(format!("Player {}", remote_player.player_id));
                                if ui.button("Teleport").clicked() {
                                    commands.trigger(network::TeleportToPlayer {
                                        player_id: remote_player.player_id,
                                        position: transform.translation().into(),
                                        rotation: transform.to_scale_rotation_translation().1.into(),
                                    });
                                }
                            });
                        }
                    }
                    
                    ui.separator();
                    
                    if ui.button("Disconnect").clicked() {
                        commands.remove_resource::<network::NetworkClient>();
                        menu.connection_status = "Disconnected".to_string();
                    }
                } else {
                    ui.label("🔴 Not Connected");
                    ui.separator();
                    
                    ui.label("Server Address:");
                    ui.text_edit_singleline(&mut menu.server_address);
                    
                    ui.add_space(5.0);
                    
                    if menu.connecting {
                        ui.label("Connecting...");
                    } else if ui.button("Connect").clicked() {
                        let address = menu.server_address.clone();
                        menu.connecting = true;
                        menu.connection_status.clear();
                        
                        let (tx, rx) = crossbeam_channel::unbounded();
                        menu.connection_receiver = Some(rx);
                        
                        std::thread::spawn(move || {
                            let result = network::TOKIO_RUNTIME.block_on(network::connect_to_server(&address));
                            let _ = tx.send(result);
                        });
                    }
                    
                    if !menu.connection_status.is_empty() {
                        ui.separator();
                        ui.colored_label(egui::Color32::RED, &menu.connection_status);
                    }
                }
            } else {
                ui.label("🔴 Not Connected");
                ui.separator();
                
                ui.label("Server Address:");
                ui.text_edit_singleline(&mut menu.server_address);
                
                ui.add_space(5.0);
                
                if menu.connecting {
                    ui.label("Connecting...");
                } else if ui.button("Connect").clicked() {
                    let address = menu.server_address.clone();
                    menu.connecting = true;
                    menu.connection_status.clear();
                    
                    let (tx, rx) = crossbeam_channel::unbounded();
                    menu.connection_receiver = Some(rx);
                    
                    std::thread::spawn(move || {
                        let result = network::TOKIO_RUNTIME.block_on(network::connect_to_server(&address));
                        let _ = tx.send(result);
                    });
                }
                
                if !menu.connection_status.is_empty() {
                    ui.separator();
                    ui.colored_label(egui::Color32::RED, &menu.connection_status);
                }
            }
        });
        
        ui.separator();
        if ui.button("Reset Simulation State").clicked() {
            day_cycle.time_of_day = 0.5;
            if let Ok(mut aircraft) = aircraft_query.single_mut() {
                aircraft.speed = 250.0;
                aircraft.throttle = 0.8;
            }
        }
    });
    
    Ok(())
}

fn draw_lod_rings(
    mut gizmos: Gizmos,
    query: Query<&GlobalTransform, With<MainCamera>>,
    wire_frame: Res<WireframeConfig>,
    chunk_manager: Res<ChunkManager>,
) {
    if !wire_frame.global {
        return;
    }
    let Ok(transform) = query.single() else { return };
    let translation = transform.translation();

    for (distance, _) in chunk_manager.lod_levels {
        gizmos.circle(
            Isometry3d::new(
                Vec3::new(translation.x, MAP_HEIGHT_SCALE * 10.0 / 3.0, translation.z), 
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            distance * CHUNK_SIZE * chunk_manager.lod_distance_multiplier,
            Color::srgb(1.0, 0.0, 0.0),
        );
    }
}

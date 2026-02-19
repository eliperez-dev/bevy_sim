use bevy::{
    color::palettes::css::WHITE,
    light::CascadeShadowConfigBuilder,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    platform::collections::HashSet,
    prelude::*, 
    render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
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


fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        // WARN this is a native only feature. It will not work with webgl or webgpu
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
            // You need to add this plugin to enable wireframe rendering
            WireframePlugin::default(),
        ))
        // Wireframes can be configured with this resource. This can be changed at runtime.
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
                (1.0 , 4),
                (2.0, 2),
                (3.0, 1),
            ],
            lod_quality_multiplier: 2,
            lod_distance_multiplier: 10.0,
        })
        .insert_resource(RenderSettings {
            cascades: 0,
            just_updated: false,
            terrain_smoothness: 0.0,
            compute_smooth_normals: false,
        })
        .init_resource::<WorldGenerationSettings>()
        // Initialize the daylight cycle
        .insert_resource(DayNightCycle {
            time_of_day: 0.50, // Start at sunrise
            speed: 0.01,  
            inclination: -1.0,     
        })
        .init_resource::<ControlMode>()
        .init_resource::<Wind>()
        .add_plugins(EguiPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, setup_camera_system)
        .add_systems(EguiPrimaryContextPass, (debugger_ui, flight_hud_system))
        .add_systems(Startup, (setup, setup_camera_fog, spawn_stars).chain())
        .add_systems(Update, (
            camera_controls, 
            update_debugger, 
            generate_chunks, 
            modify_plane, 
            handle_compute_tasks, 
            update_chunk_lod, 
            update_daylight_cycle,
            draw_lod_rings,
            //animate_water_cpu,
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
    // Shadows
    let cascade_shadow_config = CascadeShadowConfigBuilder::default().build();

    // World Generator
    commands.insert_resource(WorldGenerator::new(rand::random::<u32>()));
    
    // Shared materials for chunks
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

    // Sun Visuals
    let sun_mesh = meshes.add(Sphere::new(300.0)); // Adjust size to taste
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.8),
        emissive: LinearRgba::rgb(100.0, 80.0, 20.0), // High values make it bloom/glow
        fog_enabled: false,
        ..default()
    });

    // Sun Entity
    commands.spawn((
        Mesh3d(sun_mesh),
        MeshMaterial3d(sun_material),
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            illuminance: MAX_ILLUMANENCE, 
            ..default()
        },
        // We will move the translation dynamically in the update system
        Transform::default(), 
        cascade_shadow_config,
        bevy::camera::visibility::NoFrustumCulling,
        Sun, 
    ));

    // --- Replace this block in setup() ---

    // 1. Create a "parent" entity for the physics/logic
    let plane_entity = commands.spawn((
        Aircraft::default(),
        Transform::from_xyz(0.0, SPAWN_HEIGHT, 0.0).with_scale(Vec3::splat(0.1)),
        Visibility::default(),
        InheritedVisibility::default(),
    )).id();

    // 2. Spawn the GLTF model as a child with a corrective rotation
    let model_correction = commands.spawn(SceneRoot(
        asset_server.load("low-poly_airplane/scene.gltf#Scene0")
    )).insert(Transform::from_rotation(
        // Rotate 180 degrees on Y to face the right way (-Z)
        // Tweak the 2.0 degrees to fix your "slightly to the left" issue
        Quat::from_rotation_y((180.0f32).to_radians()) 
    )).id();

    commands.entity(plane_entity).add_child(model_correction);
    // UI
    commands.spawn((Text::new("Pos: N/A" ),
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
        MainCamera,
        Transform::from_xyz(0.0, 4000.0, 0.0).looking_at(Vec3::new(5000.0, 3000.0, 5000.0), Vec3::Y),
        DistanceFog {
            // The base "thick" color of the fog
            color: Color::srgba(0.35, 0.48, 0.66, 1.0), 
            
            // Reduced alpha (0.2-0.4) prevents the "blinding" effect
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.1), 
            
            directional_light_exponent: 1000.0, 
            
            falloff: FogFalloff::ExponentialSquared{ 
                // Tweak this number to make the fog thicker/thinner globally
                // Higher number = thicker fog closer to the camera
                density: 0.000040, 
            },
        }, 
        AmbientLight {
        color: Color::srgba(0.35, 0.48, 0.66, 1.0), // Blue-grey for sky color
        brightness: 2000.0, // Crank this up to match your Sun's high lux
        affects_lightmapped_meshes: false,
    },
    ));
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
    if time.elapsed_secs() - *last_update >= 0.5 {
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
    message.push_str(&format!("Mode: {:?} (Press F to toggle)\n", control_mode.mode));
    message.push_str(&format!("Position: [{:.0}, {:.0}, {:.0}]\n", cam_trans.x, cam_trans.y, cam_trans.z));
    message.push_str(&format!("Biome: {:?} | Climate: {:?}\n", biome, climate));
    message.push_str(&format!("Chunks: {} | Time: {:.2}\n", chunks.spawned_chunks.len(), cycle.time_of_day));


    // --- CONTROLS ---
    message.push_str("\n--- CONTROLS ---\n");
    message.push_str("T: Toggle Wireframe\n");
    
    match control_mode.mode {
        FlightMode::FreeFlight => {
            message.push_str("WASD: Move Horizontal\n");
            message.push_str("Q / E: Up / Down\n");
            message.push_str("Shift: Turbo Speed\n");
            message.push_str("Arrows: Look Around\n");
            message.push_str("Z / X: Roll Camera\n");
        },
        FlightMode::Aircraft => {
            message.push_str("W / S: Pitch Down/Up\n");
            message.push_str("A / D: Roll (Turn)\n");
            message.push_str("Q / E: Yaw (Rudder)\n");
            message.push_str("+ / -: Throttle\n");
            message.push_str("(Physics Enabled: Stalls & Gravity)\n");
        }
    }
}





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
) -> Result<(), > { 
    
    egui::Window::new("Debugger").show(contexts.ctx_mut()?, |ui| {
        render_settings.just_updated = false;

        // --- AIRCRAFT SETTINGS ---
        ui.heading("Aircraft Physics");
        if let Ok(mut aircraft) = aircraft_query.single_mut() {
            ui.collapsing("Current Values", |ui| {
                let airspeed_ratio = aircraft.speed / aircraft.max_speed;
                let control_effectiveness = if airspeed_ratio > 1.0 {
                    (1.0 / airspeed_ratio).clamp(0.3, 1.0)
                } else {
                    airspeed_ratio.clamp(0.0, 1.0)
                };
                let centripetal_accel = aircraft.speed * aircraft.pitch_velocity.abs();
                let g_force = centripetal_accel / 9.8;
                let turn_drag = g_force * aircraft.g_force_drag;
                let speed_drag = (airspeed_ratio).powi(2) * 20.0;
                
                ui.label(format!("Speed: {:.1} ({:.0}%)", aircraft.speed, airspeed_ratio * 100.0));
                ui.label(format!("Control Effectiveness: {:.0}%", control_effectiveness * 100.0));
                ui.label(format!("Pitch Velocity: {:.3}", aircraft.pitch_velocity));
                ui.label(format!("G-Force: {:.2}G", g_force));
                ui.label(format!("Turn Drag: {:.2}", turn_drag));
                ui.label(format!("Speed Drag: {:.2}", speed_drag));
            });
            
            ui.collapsing("Flight Model Tuning", |ui| {
                ui.label("Forces");
                ui.add(egui::Slider::new(&mut aircraft.max_speed, 50.0..=1000.0).text("Max Speed (Level)"));
                ui.add(egui::Slider::new(&mut aircraft.drag_factor, 0.1..=2.0).text("Drag (Acceleration)"));
                ui.add(egui::Slider::new(&mut aircraft.gravity, 0.0..=100.0).text("Gravity (Climb/Dive Effect)"));
                ui.add(egui::Slider::new(&mut aircraft.g_force_drag, 0.0..=500.0).text("Turn Drag (Energy Bleed)"));

                ui.separator();
                ui.label("Responsiveness");
                ui.add(egui::Slider::new(&mut aircraft.pitch_strength, 0.5..=10.0).text("Pitch Strength"));
                ui.add(egui::Slider::new(&mut aircraft.roll_strength, 0.5..=10.0).text("Roll Strength"));
                ui.add(egui::Slider::new(&mut aircraft.yaw_strength, 0.5..=10.0).text("Yaw Strength"));

                ui.separator();
                ui.label("Assists");
                ui.add(egui::Slider::new(&mut aircraft.bank_turn_strength, 0.0..=5.0).text("Auto-Turn (Bank)"));
                ui.add(egui::Slider::new(&mut aircraft.auto_level_strength, 0.0..=5.0).text("Auto-Level (Stability)"));
            });
        } else {
            ui.label("No Aircraft found.");
        }
        ui.separator();

        // --- WIND SETTINGS ---
        ui.heading("Wind & Turbulence");
        ui.collapsing("Wind Settings", |ui| {
            let mut wind_speed = wind.base_wind.length();
            let wind_direction = if wind_speed > 0.01 {
                wind.base_wind / wind_speed
            } else {
                Vec3::new(1.0, 0.0, 0.0)
            };
            
            ui.label("Wind Speed");
            if ui.add(egui::Slider::new(&mut wind_speed, 0.0..=100.0).text("Speed")).changed() {
                wind.base_wind = wind_direction * wind_speed;
            }
            
            ui.separator();
            ui.label("Wind Direction (Vector)");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(&mut wind.base_wind.x).speed(0.5));
                ui.label("Y:");
                ui.add(egui::DragValue::new(&mut wind.base_wind.y).speed(0.5));
                ui.label("Z:");
                ui.add(egui::DragValue::new(&mut wind.base_wind.z).speed(0.5));
            });
            
            ui.separator();
            ui.label("Turbulence");
            ui.add(egui::Slider::new(&mut wind.turbulence_intensity, 0.0..=3.0).text("Intensity").logarithmic(true));
            ui.add(egui::Slider::new(&mut wind.turbulence_frequency, 0.01..=10.0).text("Frequency"));
        });
        ui.separator();

        // --- WORLD GEN SETTINGS ---
        ui.heading("Time Settings");
        ui.add(egui::Slider::new(&mut day_cycle.time_of_day, 0.0..=1.0).text("Time of Day"));
        ui.add(egui::Slider::new(&mut day_cycle.speed, 0.0..=0.1).text("Time Speed"));
        ui.add(egui::Slider::new(&mut day_cycle.inclination, -1.0..=1.0).text("Inclination"));

        ui.separator();

        ui.collapsing("Render Settings", |ui| {
        ui.heading("Render Settings");
        ui.checkbox(&mut wireframe_config.global, "Global Wireframe");

        if ui.add(egui::Slider::new(&mut chunk_manager.render_distance, 2..=150).text("Render Distance")).changed() {
            render_settings.just_updated = true;
        }

        ui.add(egui::Slider::new(&mut world_settings.max_chunks_per_frame, 1..=500).text("Max Chunks / Frame"));

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

        if ui.add(egui::Slider::new(&mut chunk_manager.lod_distance_multiplier, 1.0..=15.0).text("LOD Distance")).changed() {
            render_settings.just_updated = true;
        }

        if let Ok(mut fog) = fog_query.single_mut() {
             if let FogFalloff::ExponentialSquared { density } = &mut fog.falloff {
                ui.add(egui::Slider::new(density, 0.000005..=0.001).text("Fog Density").logarithmic(true));
            }
        }
        });
        
        if ui.button("Reset Simulation").clicked() {
            day_cycle.time_of_day = 0.5;
            if let Ok(mut aircraft) = aircraft_query.single_mut() {
                aircraft.speed = 100.0;
                aircraft.throttle = 0.5;
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

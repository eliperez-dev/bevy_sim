use bevy::{
    color::palettes::css::WHITE,
    light::CascadeShadowConfigBuilder,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    platform::collections::HashSet,
    prelude::*, 
    render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    camera::ClearColorConfig,
};


use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

use world_generation::*;
use consts::*;
use day_cycle::*;
use controls::*;

mod world_generation;
mod consts;
mod day_cycle;
mod controls;


fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(RenderPlugin {
                render_creation: WgpuSettings {
                    // WARN this is a native only feature. It will not work with webgl or webgpu
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }
                .into(),
                ..default()
            }),
            // You need to add this plugin to enable wireframe rendering
            WireframePlugin::default(),
            FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    text_config: TextFont {
                        // Here we define size of our overlay
                        font_size: 42.0,
                        // If we want, we can use a custom font
                        font: default(),
                        // We could also disable font smoothing,
                        font_smoothing: bevy::text::FontSmoothing::default(),

                        ..default()
                    },
                    // We can also change color of the overlay
                    text_color: Color::srgb(0.0, 1.0, 0.0),
                    // We can also set the refresh interval for the FPS counter
                    refresh_interval: core::time::Duration::from_millis(100),
                    enabled: true,
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: true,
                        // The minimum acceptable fps
                        min_fps: 30.0,
                        // The target fps
                        target_fps: 144.0,
                    },
                },
            },
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
            render_distance: RENDER_DISTANCE,
        })
        .init_resource::<WorldGenerationSettings>()
        // Initialize the daylight cycle
        .insert_resource(DayNightCycle {
            time_of_day: 0.50, // Start at sunrise
            speed: 0.025,  
            inclination: 0.8,     
        })
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup_camera_system)
        .add_systems(EguiPrimaryContextPass, ui_example_system)
        .add_systems(Startup, (setup, setup_camera_fog).chain())
        .add_systems(Update, (
            camera_controls, 
            update_debugger, 
            generate_chunks, 
            modify_plane, 
            handle_compute_tasks, 
            update_chunk_lod, 
            update_daylight_cycle.after(camera_controls)
        ))
        .add_systems(PostUpdate, despawn_out_of_bounds_chunks)
        .run();
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
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        first_cascade_far_bound: LIGHTING_BOUNDS/100.0,
        maximum_distance: LIGHTING_BOUNDS,
        ..default()
    }
    .build();

    // World Generator
    commands.insert_resource(WorldGenerator::new(0));
    
    // Shared materials for chunks
    commands.insert_resource(SharedChunkMaterials {
        terrain_material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            ..default()
        }),
        water_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.6),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.5,
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

    // Tree
    commands.spawn((
        // The "#Scene0" points to the default scene inside the glTF file
        SceneRoot(asset_server.load("tree.gltf#Scene0")), 
        Transform::from_xyz(0.0, 55.0, 0.0), // Position it where you want
    ));
    
    // UI
    commands.spawn((Text::new("Pos: N/A" ),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12.0),
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
        Transform::from_xyz(-50.0, 100.0, 50.0).looking_at(Vec3::new(0.0, 80.0, 0.0), Vec3::Y),
        DistanceFog {
            // The base "thick" color of the fog
            color: Color::srgba(0.35, 0.48, 0.66, 1.0), 
            
            // Reduced alpha (0.2-0.4) prevents the "blinding" effect
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.1), 
            
            directional_light_exponent: 1000.0, 
            
            falloff: FogFalloff::ExponentialSquared{ 
                // Tweak this number to make the fog thicker/thinner globally
                // Higher number = thicker fog closer to the camera
                density: 0.00002, 
            },
        }, 
        AmbientLight {
            color: Color::WHITE,
            brightness: 10.0,
            affects_lightmapped_meshes: false, // Standard daytime ambient brightness
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
    mut debugger: Query<&mut Text, With<Debugger>>,
) {
    let cam_trans = camera.single().unwrap().translation;
    let camera_pos_arr = &[cam_trans.x, cam_trans.y, cam_trans.z];
    let biome = world.get_biome(camera_pos_arr);
    let message = &mut debugger.single_mut().unwrap().0;

    let climate = world.get_climate(camera_pos_arr);

    message.clear();

    message.push_str(&format!("Position: [{:.2}, {:.2}, {:.2}]\n", cam_trans.x, cam_trans.y, cam_trans.z));
    message.push_str(&format!("Biome: {:?} Climate: {:?}\n", biome, climate));
    message.push_str(&format!("Chunks: {:?}\n", chunks.spawned_chunks.len())); // Added newline here
    message.push_str(&format!("Time of Day: {:.2}\n", cycle.time_of_day)); // <-- Added daylight info
}

fn ui_example_system(
    mut contexts: EguiContexts,
    mut day_cycle: ResMut<DayNightCycle>,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut world_settings: ResMut<WorldGenerationSettings>,
    mut fog_query: Query<&mut DistanceFog, With<MainCamera>>,
) -> Result {
    egui::Window::new("Debugger").show(contexts.ctx_mut()?, |ui| {
        ui.heading("Time Settings");
        
        // Sun Time Slider
        ui.add(egui::Slider::new(&mut day_cycle.time_of_day, 0.0..=1.0).text("Time of Day"));
        
        // Sun Speed
        ui.add(egui::Slider::new(&mut day_cycle.speed, 0.0..=0.1).text("Time Speed"));
        
        // Sun Inclination
        ui.add(egui::Slider::new(&mut day_cycle.inclination, -1.0..=1.0).text("Inclination"));

        ui.separator();

        ui.heading("Render Settings");
        
        // Global Wireframe Toggle
        ui.checkbox(&mut wireframe_config.global, "Global Wireframe");

        ui.add(egui::Slider::new(&mut chunk_manager.render_distance, 2..=150).text("Render Distance"));
        ui.add(egui::Slider::new(&mut world_settings.max_chunks_per_frame, 1..=250).text("Max Chunks / Frame"));

        if let Ok(mut fog) = fog_query.single_mut() {
            if let FogFalloff::ExponentialSquared { density } = &mut fog.falloff {
                ui.add(egui::Slider::new(density, 0.00001..=0.00025).text("Fog Density"));
            }
        }

        if ui.button("Reset Simulation").clicked() {
            day_cycle.time_of_day = 0.5;
        }
    });
    Ok(())
}

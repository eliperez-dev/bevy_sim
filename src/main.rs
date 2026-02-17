use bevy::{
    color::palettes::css::WHITE,
    light::CascadeShadowConfigBuilder,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    platform::collections::HashSet,
    prelude::*, 
    render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig}
};

use world_generation::*;
use consts::*;

mod world_generation;
mod consts;

#[derive(Resource)]
pub struct DayNightCycle {
    pub time_of_day: f32,
    pub speed: f32, 
}

#[derive(Component)]
pub struct Sun;

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
        .insert_resource(ChunkManager {spawned_chunks: HashSet::new(), last_camera_chunk: None, to_spawn: Vec::new(), lod_to_update: Vec::new()})
        // Initialize the daylight cycle
        .insert_resource(DayNightCycle {
            time_of_day: 0.75, // Start at sunrise
            speed: 0.05,       
        })
        .add_systems(Startup, (setup, setup_camera_fog).chain())
        .add_systems(Update, (
            camera_controls, 
            update_debugger, 
            generate_chunks, 
            modify_plane, 
            handle_compute_tasks, 
            update_chunk_lod, 
            despawn_out_of_bounds_chunks,
            update_daylight_cycle // Register the new daylight system
        ))
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
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
            ..default()
        }),
        water_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.6),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });

    // Tree
    commands.spawn((
        // The "#Scene0" points to the default scene inside the glTF file
        SceneRoot(asset_server.load("tree.gltf#Scene0")), 
        Transform::from_xyz(0.0, 55.0, 0.0), // Position it where you want
    ));
    
    // Sun
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            illuminance: MAX_ILLUMANENCE, // Set standard baseline illuminance
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(-0.15, -0.05, 0.25), Vec3::Y),
        cascade_shadow_config,
        Sun, // Tag the sun
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

fn setup_camera_fog(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-50.0, 100.0, 50.0).looking_at(Vec3::new(0.0, 80.0, 0.0), Vec3::Y),
        DistanceFog {
            // The base "thick" color of the fog
            color: Color::srgba(0.35, 0.48, 0.66, 1.0), 
            
            // Reduced alpha (0.2-0.4) prevents the "blinding" effect
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.1), 
            
            directional_light_exponent: 100.0, 
            
            falloff: FogFalloff::Exponential{ 
                // Tweak this number to make the fog thicker/thinner globally
                // Higher number = thicker fog closer to the camera
                density: 0.00015, 
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
    camera: Query<&Transform, With<Camera>>,
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


fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut wire_frame: ResMut<WireframeConfig>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
) {

    let mut transform = camera_query.single_mut().unwrap();
    let mut pan_speed = 200.0; 
    let rotation_speed = 1.0; 
    let mut pan_direction = Vec3::ZERO;
    let panning_delta = rotation_speed * time.delta_secs();

    if keyboard.pressed(KeyCode::ShiftLeft) {
        pan_speed *= 15.00;
    }
    if keyboard.pressed(KeyCode::Tab) {
        pan_speed *= 15.00;
    }

    let forward = transform.forward().as_vec3();
    let right = transform.right().as_vec3();
    let up = transform.up().as_vec3();

    // Wireframe Enable
    if keyboard.just_pressed(KeyCode::KeyT) {
        wire_frame.global = !wire_frame.global;
    }

    // Pan Forward/Backward 
    if keyboard.pressed(KeyCode::KeyW) {
        pan_direction += forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        pan_direction -= forward;
    }

    // Pan Left/Right
    if keyboard.pressed(KeyCode::KeyA) {
        pan_direction -= right;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        pan_direction += right;
    }

    // Pan Up/Down 
    if keyboard.pressed(KeyCode::KeyE) {
        pan_direction += up;
    }
    if keyboard.pressed(KeyCode::KeyQ) {
        pan_direction -= up;
    }

    // Handle Yaw 
    if keyboard.pressed(KeyCode::ArrowLeft) {
        transform.rotate_y(panning_delta);
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        transform.rotate_y(-panning_delta);
    }

    // Handle Pitch 
    if keyboard.pressed(KeyCode::ArrowUp) {
        transform.rotate_local_x(panning_delta);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        transform.rotate_local_x(-panning_delta);
    }

    // Apply transform translation
    transform.translation += pan_direction.normalize_or_zero() * pan_speed * time.delta_secs();
}


fn update_daylight_cycle(
    time: Res<Time>,
    mut cycle: ResMut<DayNightCycle>,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    // Query both the Fog AND the AmbientLight component (they are on the same camera entity)
    mut env_query: Query<(&mut DistanceFog, &mut AmbientLight)>, 
) {
    cycle.time_of_day = (cycle.time_of_day + cycle.speed * time.delta_secs()) % 1.0;
    let angle = cycle.time_of_day * core::f32::consts::TAU - core::f32::consts::FRAC_PI_2;

    let mut current_intensity = 0.0;

    // 1. Update the Sun
    if let Ok((mut transform, mut light)) = sun_query.single_mut() {
        transform.rotation = Quat::from_rotation_x(angle) * Quat::from_rotation_y(0.5);

        let sun_dir = transform.forward().as_vec3();
        let up_dot = sun_dir.dot(Vec3::NEG_Y);
        
        current_intensity = up_dot.max(0.0);
        light.illuminance = (MAX_ILLUMANENCE * current_intensity).max(MIN_ILLUMANENCE); 
    }

    // 2. Update the Fog and AmbientLight
    if let Ok((mut fog, mut ambient)) = env_query.single_mut() {
        
        // --- Ambient Light Fade ---
        let max_ambient = 10.0; // Daytime brightness
        let min_ambient = 0.2;  // Nighttime brightness (tweak this if it's too dark!)
        ambient.brightness = min_ambient + (max_ambient - min_ambient) * current_intensity;

        let day_r = 0.35; let day_g = 0.48; let day_b = 0.66;
        let night_r = 0.02; let night_g = 0.02; let night_b = 0.05;

        let r = night_r + (day_r - night_r) * current_intensity;
        let g = night_g + (day_g - night_g) * current_intensity;
        let b = night_b + (day_b - night_b) * current_intensity;
        
        // Update the base color (thickness remains exactly the same!)
        fog.color = Color::srgb(r, g, b);

        // Sunset glow
        let sun_r = 1.0;
        let sun_g = 0.4 + (0.55 * current_intensity);
        let sun_b = 0.1 + (0.75 * current_intensity);
        fog.directional_light_color = Color::srgba(sun_r, sun_g, sun_b, 0.15 * current_intensity);
    }
}
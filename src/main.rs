use bevy::{
    anti_alias::fxaa::Fxaa,
    camera::Exposure,
    color::palettes::css::{BLACK, WHITE},
    core_pipeline::tonemapping::Tonemapping,
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    input::keyboard::KeyCode,
    light::{
        light_consts::lux, AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder, FogVolume,
        VolumetricFog, VolumetricLight,
    },
    mesh::CuboidMeshBuilder,
    pbr::{
        Atmosphere, AtmosphereMode, AtmosphereSettings, DefaultOpaqueRendererMethod,
        ScatteringMedium, ScreenSpaceReflections,
        wireframe::{WireframeConfig, WireframePlugin},
    },
    platform::collections::HashSet,
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        render_resource::{AsBindGroup, ShaderType},
        settings::{WgpuFeatures, WgpuSettings},
        RenderPlugin,
    },
    shader::ShaderRef,
};

use world_generation::*;
use consts::*;

mod world_generation;
mod consts;


fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight::NONE)
        .insert_resource(GameState::default())
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
        .insert_resource(ChunkManager {spawned_chunks: HashSet::new()})
        .add_systems(Startup, (setup, setup_camera_fog).chain())
        .add_systems(Update, (camera_controls, update_debugger, generate_chunks, modify_plane, despawn_out_of_bounds_chunks, atmosphere_controls, dynamic_scene))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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

    // testbox
    commands.spawn( (
        Mesh3d(meshes.add(dbg!(CuboidMeshBuilder::default()))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.5))),
        Transform::from_xyz(0.0, 35.0, 0.0),
        TestBox,
    ));
    
    // Sun
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            illuminance: lux::RAW_SUNLIGHT,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(0.0, -0.1, -1.0), Vec3::Y),
        VolumetricLight,
        cascade_shadow_config,
    ));

    // Fog Volume
    commands.spawn((
        FogVolume::default(),
        Transform::from_scale(Vec3::new(10.0, 1.0, 10.0)).with_translation(Vec3::Y * 0.5),
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

fn setup_camera_fog(
    mut commands: Commands,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-50.0, 100.0, 50.0).looking_at(Vec3::new(0.0, 80.0, 0.0), Vec3::Y),
        Atmosphere::earthlike(scattering_mediums.add(ScatteringMedium::default())),
        AtmosphereSettings::default(),
        Exposure { ev100: 15.0 },
        Tonemapping::AcesFitted,
        Bloom::NATURAL,
        AtmosphereEnvironmentMapLight::default(),
        VolumetricFog {
            ambient_intensity: 0.0,
            ..default()
        },
        Msaa::Off,
        Fxaa::default(),
    ));
}

#[derive(Component)]
struct Debugger;

fn update_debugger(
    camera: Query<&Transform, With<Camera>>,
    world: Res<WorldGenerator>,
    chunks: Res<ChunkManager>,
    mut debugger: Query<&mut Text, With<Debugger>>,
) {
    let camera_pos = camera.single().unwrap().translation;
    let biome = world.get_biome(&[camera_pos[0], camera_pos[1], camera_pos[1]]);
    let message = &mut debugger.single_mut().unwrap().0;

    message.clear();

    message.push_str(&format!("Position: {:?}\n", camera_pos));
    message.push_str(&format!("Biome: {:?}\n", biome));
    message.push_str(&format!("Chunks: {:?}", chunks.spawned_chunks.len()));

}


#[derive(Component)]
struct TestBox;

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
    if keyboard.just_pressed(KeyCode::KeyI) {
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

#[derive(Resource)]
struct GameState {
    paused: bool,
    high_fidelity: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            paused: false,
            high_fidelity: true,
        }
    }
}

fn atmosphere_controls(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut atmosphere_settings: Query<&mut AtmosphereSettings>,
    mut game_state: ResMut<GameState>,
    mut camera_exposure: Query<&mut Exposure, With<Camera3d>>,
    mut suns: Query<&mut Transform, With<DirectionalLight>>,
    mut sun_lights: Query<&mut DirectionalLight>,
    camera_query: Query<Entity, With<Camera3d>>,
    sun_query: Query<Entity, With<DirectionalLight>>,
    time: Res<Time>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        game_state.high_fidelity = !game_state.high_fidelity;
        println!("High Fidelity: {}", game_state.high_fidelity);

        let camera_entity = camera_query.single().unwrap();
        let sun_entity = sun_query.single().unwrap();

        if game_state.high_fidelity {
            commands.entity(camera_entity)
                .insert(VolumetricFog {
                    ambient_intensity: 0.0,
                    ..default()
                })
                .insert(Bloom::NATURAL)
                .insert(Fxaa::default());
            
            commands.entity(sun_entity).insert(VolumetricLight);

            for mut l in &mut sun_lights {
                l.shadows_enabled = true;
            }
        } else {
            commands.entity(camera_entity)
                .remove::<VolumetricFog>()
                .remove::<Bloom>()
                .remove::<Fxaa>();
            
            commands.entity(sun_entity).remove::<VolumetricLight>();

            for mut l in &mut sun_lights {
                l.shadows_enabled = false;
            }
        }
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) {
        for mut sun_transform in &mut suns {
            *sun_transform = Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(0.0, -0.1, 1.0), Vec3::Y);
        }
        println!("Reset to sunrise");
    }

    if keyboard_input.just_pressed(KeyCode::KeyT) {
        game_state.paused = !game_state.paused;
        println!("Paused Time");
    }

    if keyboard_input.just_pressed(KeyCode::Digit1) {
        for mut settings in &mut atmosphere_settings {
            settings.rendering_method = AtmosphereMode::LookupTexture;
            println!("Switched to lookup texture rendering method");
        }
    }

    if keyboard_input.just_pressed(KeyCode::Digit2) {
        for mut settings in &mut atmosphere_settings {
            settings.rendering_method = AtmosphereMode::Raymarched;
            println!("Switched to raymarched rendering method");
        }
    }

    if keyboard_input.just_pressed(KeyCode::Enter) {
        game_state.paused = !game_state.paused;
    }

    if keyboard_input.pressed(KeyCode::Minus) {
        for mut exposure in &mut camera_exposure {
            exposure.ev100 -= time.delta_secs() * 2.0;
        }
    }

    if keyboard_input.pressed(KeyCode::Equal) {
        for mut exposure in &mut camera_exposure {
            exposure.ev100 += time.delta_secs() * 2.0;
        }
    }
}

fn dynamic_scene(
    mut suns: Query<&mut Transform, With<DirectionalLight>>,
    time: Res<Time>,
    sun_motion_state: Res<GameState>,
) {
    // Only rotate the sun if motion is not paused
    if !sun_motion_state.paused {
        suns.iter_mut()
            .for_each(|mut tf| tf.rotate_x(time.delta_secs() * std::f32::consts::PI / 10.0));
    }
}

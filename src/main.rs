use bevy::{
    color::palettes::css::WHITE, light::CascadeShadowConfigBuilder, mesh::{CuboidMeshBuilder, VertexAttributeValues}, pbr::wireframe::{NoWireframe, Wireframe, WireframeConfig, WireframePlugin}, prelude::*, render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}}
};

use bevy::color::Mix;
use noise::{NoiseFn, Perlin};

const MAP_SIZE: f32 = 10000.0; 
const TERRAIN_QUALITY: f32 = 0.025;
const MAP_HEIGHT_SCALE: f32 = 100.0;
const TERRAIN_SMOOTHNESS: f32 = 0.0;

const LIGHTING_BOUNDS: f32 = 5000.0;
const FOG_DISTANCE: f32 = 800.0;


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
        ))
        // Wireframes can be configured with this resource. This can be changed at runtime.
        .insert_resource(WireframeConfig {
            global: false,
            default_color: WHITE.into(),
        })
        .add_systems(Startup, (setup, modify_plane, setup_camera_fog, update_debugger).chain())
        .add_systems(Update, (camera_controls, update_debugger))
        .run();
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Biome {
    Desert,
    Grasslands,
    Taiga,
    Forest, // New Biome!
    None,
}

#[derive(Resource)]
struct WorldGenerator {
    terrain_layers: Vec<PerlinLayer>,
    temperature_layer: PerlinLayer,
    humidity_layer: PerlinLayer,
}

impl WorldGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            terrain_layers: vec![
            PerlinLayer::new(seed, 2.0, 0.25),
            PerlinLayer::new(seed + 1, 1.5, 0.5),
            PerlinLayer::new(seed + 2, 1.0, 1.0),
            PerlinLayer::new(seed + 3, 0.3, 3.5),
        ],
            temperature_layer: PerlinLayer::new(seed + 4, 0.1, 1.0),
            humidity_layer: PerlinLayer::new(seed + 5, 0.1, 1.0),
        }
    }

    pub fn get_climate(&self, pos: &[f32; 3]) -> (f32, f32) {
        let raw_temp = self.temperature_layer.get_level(pos, Biome::Grasslands);
        let raw_hum = self.humidity_layer.get_level(pos, Biome::Grasslands);

        // Normalize to 0.0 -> 1.0 and clamp it
        let temp_normalized = (((raw_temp / self.temperature_layer.vertical_scale) + 1.0) * 0.5).clamp(0.0, 1.0);
        let hum_normalized = (((raw_hum / self.humidity_layer.vertical_scale) + 1.0) * 0.5).clamp(0.0, 1.0);

        (temp_normalized, hum_normalized)
    }

    pub fn get_biome(&self, pos: &[f32; 3]) -> Biome {
        // 1. Get raw noise (typically -1.0 to 1.0)
        let raw_temp = self.temperature_layer.get_level(pos, Biome::Grasslands);
        let raw_hum = self.humidity_layer.get_level(pos, Biome::Grasslands);

        // 2. Normalize to roughly 0.0 -> 1.0
        // We divide by vertical_scale to reverse the multiplier you applied in get_level
        let temp_normalized = ((raw_temp / self.temperature_layer.vertical_scale) + 1.0) * 0.5;
        let hum_normalized = ((raw_hum / self.humidity_layer.vertical_scale) + 1.0) * 0.5;

        // 3. Simple 2x2 Biome Matrix
        if temp_normalized > 0.5 { 
            // Hot climates
            if hum_normalized > 0.45 { 
                Biome::Forest // Hot & Wet
            } else { 
                Biome::Desert // Hot & Dry
            }
        } else { 
            // Cold climates
            if hum_normalized > 0.5 { 
                Biome::Taiga // Cold & Wet
            } else { 
                Biome::Grasslands // Cold & Dry
            }
        }
    }

    pub fn add_layer(&mut self, layer: PerlinLayer) {
        self.terrain_layers.push(layer);
    }
}

#[derive(Resource)]
struct PerlinLayer {
    perlin: Perlin,
    horizontal_scale: f32,
    vertical_scale: f32,
}

impl PerlinLayer {
    pub fn new(seed: u32, horizontal_scale: f32, vertical_scale: f32) -> Self {
        Self {
            perlin: Perlin::new(seed),
            horizontal_scale,
            vertical_scale,
        }
    }

    pub fn get_level(&self, pos: &[f32; 3], biome: Biome) -> f32 {
        let height = self.perlin
            .get([
                (pos[0] * self.horizontal_scale / 1000.0) as f64,
                (pos[2] * self.horizontal_scale / 1000.0) as f64
            ]
        ) as f32;
        height * self.vertical_scale
    }
}

fn modify_plane(
    query: Query<&Mesh3d, With<Ground>>,
    world_generator: Res<WorldGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
) {

    for mesh_handle in &query {
        // Get a mutable reference to the mesh asset
        if let Some(mesh) = meshes.get_mut(mesh_handle) {
            
            let mut colors: Vec<[f32; 4]> = Vec::new();

            // Access the position attribute mutably
            if let Some(VertexAttributeValues::Float32x3(positions)) = 
                mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) 
            {
                // Pre-allocate for performance
                colors.reserve(positions.len());

                // Inside modify_plane, replace your inner `for pos in positions.iter_mut()` loop with this:

                for pos in positions.iter_mut() {
                    let mut height = 0.0;
                    
                    // Grab the exact climate for this vertex
                    let (temp, humidity) = world_generator.get_climate(pos);

                    // We pass Biome::None here because your noise layer currently doesn't use the biome 
                    // parameter to calculate height anyway (we can change this later!)
                    for layer in &world_generator.terrain_layers {
                        height += layer.get_level(pos, Biome::None)
                    }

                    // Pass the exact temp and humidity into the color generator
                    colors.push(get_terrain_color(height, temp, humidity));

                    pos[1] = height * MAP_HEIGHT_SCALE;
                }
            }
            
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.compute_smooth_normals();
        }
    }
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


    // ground
    commands.spawn((
        Mesh3d(meshes.add(
            Plane3d::default().mesh()
            .size(MAP_SIZE, MAP_SIZE)
            .subdivisions((MAP_SIZE * TERRAIN_QUALITY) as u32)
        )),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE, // Set to white to allow vertex colors to show
            perceptual_roughness: 1.0, // 1.0 is fully matte, 0.0 is a mirror
            reflectance: 0.1,          // Lowering this also reduces "specular" highlights
            ..default()
        })),
        Ground,
    ));

    // Ocean
    commands.spawn((
        Mesh3d(meshes.add(
            Plane3d::default().mesh()
            .size(MAP_SIZE, MAP_SIZE)
        )),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.5))),
        NoWireframe,
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

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
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(-0.15, -0.05, 0.25), Vec3::Y),
        cascade_shadow_config,
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
        Transform::from_xyz(-50.0, 100.0, 50.0).looking_at(Vec3::new(0.0, 80.0, 0.0), Vec3::Y),
        DistanceFog {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
            directional_light_exponent: 10.0,
            falloff: FogFalloff::from_visibility_colors(
                FOG_DISTANCE * 50.0, // distance in world units up to which objects retain visibility
                Color::srgb(0.35, 0.5, 0.66), // atmospheric extinction color
                Color::srgb(0.8, 0.844, 1.0), // atmospheric inscattering color
            ),
        },
    ));
}

#[derive(Component)]
struct Debugger;

fn update_debugger(
    camera: Query<&Transform, With<Camera>>,
    world: Res<WorldGenerator>,
    mut debugger: Query<&mut Text, With<Debugger>>,
) {
    let camera_pos = camera.single().unwrap().translation;
    let biome = world.get_biome(&[camera_pos[0], camera_pos[1], camera_pos[1]]);
    let mut message = debugger.single_mut().unwrap();

    message.0 = format!("Position: {:?}\nBiome: {:?}", camera_pos, biome);
}


#[derive(Component)]
struct Ground;

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

struct TerrainStop {
    height: f32,
    color: Color,
}


const GRASSLANDS_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -0.5, color: Color::srgb(0.3, 0.2, 0.1) }, // Dirt
    TerrainStop { height: 0.0,  color: Color::srgb(0.8, 0.7, 0.5) }, // Sand
    TerrainStop { height: 0.3,  color: Color::srgb(0.2, 0.5, 0.2) }, // Grass
    TerrainStop { height: 1.5,  color: Color::srgb(0.5, 0.5, 0.5) }, // Rock
    TerrainStop { height: 2.0,  color: Color::WHITE },               // Snow
];

const DESERT_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -0.5, color: Color::srgb(0.6, 0.4, 0.2) }, // Hard dirt
    TerrainStop { height: 0.0,  color: Color::srgb(0.9, 0.8, 0.5) }, // Sand
    TerrainStop { height: 0.3,  color: Color::srgb(0.8, 0.6, 0.3) }, // Orange dunes
    TerrainStop { height: 1.5,  color: Color::srgb(0.7, 0.4, 0.2) }, // Red Rock
    TerrainStop { height: 2.0,  color: Color::srgb(0.6, 0.3, 0.1) }, // Dark Mesa peak
];

const TAIGA_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -0.5, color: Color::srgb(0.2, 0.2, 0.2) }, // Dark Dirt
    TerrainStop { height: 0.0,  color: Color::srgb(0.4, 0.4, 0.4) }, // Gravel
    TerrainStop { height: 0.3,  color: Color::srgb(0.1, 0.3, 0.2) }, // Dark Pine Grass
    TerrainStop { height: 1.0,  color: Color::srgb(0.5, 0.5, 0.5) }, // Rock
    TerrainStop { height: 1.2,  color: Color::WHITE },               // Heavy Snow
];

const FOREST_TERRAIN_LEVELS: &[TerrainStop] = &[
    TerrainStop { height: -0.5, color: Color::srgb(0.3, 0.2, 0.1) }, // Dirt
    TerrainStop { height: 0.0,  color: Color::srgb(0.2, 0.4, 0.1) }, // Deep Grass
    TerrainStop { height: 0.3,  color: Color::srgb(0.1, 0.5, 0.1) }, // Lush Canopy
    TerrainStop { height: 1.5,  color: Color::srgb(0.4, 0.4, 0.4) }, // Rock
    TerrainStop { height: 2.0,  color: Color::WHITE },               // Snow
];

fn get_color_from_palette(height: f32, palette: &[TerrainStop]) -> Color {
    if height.is_nan() { return palette[0].color; }

    let mut upper_idx = 0;
    while upper_idx < palette.len() && height > palette[upper_idx].height {
        upper_idx += 1;
    }

    if upper_idx == 0 { return palette[0].color; }
    if upper_idx >= palette.len() { return palette.last().unwrap().color; }

    let lower = &palette[upper_idx - 1];
    let upper = &palette[upper_idx];

    let range = upper.height - lower.height;
    let t = ((height - lower.height) / range).clamp(0.0, 1.0);
    
    let blend_start = 1.0 - TERRAIN_SMOOTHNESS.clamp(0.0, 1.0);
    let base_color = lower.color.to_linear();
    let next_color = upper.color.to_linear();

    // Notice we return Color here, not an f32 array yet
    if t > blend_start && TERRAIN_SMOOTHNESS > 0.0 {
        let blend_t = (t - blend_start) / TERRAIN_SMOOTHNESS;
        base_color.mix(&next_color, blend_t).into()
    } else {
        base_color.into()
    }
}

// Notice the signature changed to accept temp and humidity
fn get_terrain_color(height: f32, temp: f32, humidity: f32) -> [f32; 4] {
    // 1. Get what the color *would* be if the world was 100% this biome
    let forest_color = get_color_from_palette(height, FOREST_TERRAIN_LEVELS).to_linear();
    let desert_color = get_color_from_palette(height, DESERT_TERRAIN_LEVELS).to_linear();
    let taiga_color = get_color_from_palette(height, TAIGA_TERRAIN_LEVELS).to_linear();
    let grass_color = get_color_from_palette(height, GRASSLANDS_TERRAIN_LEVELS).to_linear();

    // 2. Bilinear Interpolation
    // First, blend the humidity axis (dry -> wet) for both hot and cold extremes
    let cold_blend = grass_color.mix(&taiga_color, humidity); 
    let hot_blend = desert_color.mix(&forest_color, humidity);  

    // Finally, blend between those two results along the temperature axis (cold -> hot)
    let final_color = cold_blend.mix(&hot_blend, temp);

    final_color.to_f32_array()
}
use bevy::{
    color::palettes::css::WHITE, light::CascadeShadowConfigBuilder, mesh::{CuboidMeshBuilder, VertexAttributeValues}, pbr::wireframe::{NoWireframe, Wireframe, WireframeConfig, WireframePlugin}, prelude::*, render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}}
};

use noise::{NoiseFn, Perlin};



const MAP_SIZE: f32 = 1000.0;
const MAP_HEIGHT_SCALE: f32 = 100.0;

const LIGHTING_BOUNDS: f32 = 1000.0;
const FOG_DISTANCE: f32 = 50.0;


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
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .add_systems(Startup, (setup, modify_plane, setup_camera_fog, update_debugger).chain())
        .add_systems(Update, (camera_controls, update_debugger))
        .run();
}

struct WorldGenerator {
    perlin_layers: Vec<PerlinLayer>,
}

impl WorldGenerator {
    pub fn new() -> Self {
        Self {
            perlin_layers: vec![],
        }
    }

    pub fn add_layer(&mut self, layer: PerlinLayer) {
        self.perlin_layers.push(layer);
    }
}

impl Default for WorldGenerator {
    fn default() -> Self {
        Self::new()
    }
}

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
            vertical_scale: vertical_scale.max(-1.0).min(1.0),
        }
    }

    pub fn get_level(&self, pos: &mut [f32;3]) -> f32{
        let height = self.perlin
            .get([
                (pos[0] * self.horizontal_scale / MAP_SIZE) as f64,
                (pos[2] * self.horizontal_scale / MAP_SIZE) as f64
            ]
        ) as f32;
        height * self.vertical_scale
    }


}

fn modify_plane(
    query: Query<&Mesh3d, With<Ground>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {

    let mut world_generator = WorldGenerator::new();

    world_generator.add_layer(PerlinLayer::new(0, 1.0, 1.0));
    //world_generator.add_layer(PerlinLayer::new(1, 1.0, 1.0));

    for mesh_handle in &query {
        // Get a mutable reference to the mesh asset
        if let Some(mesh) = meshes.get_mut(mesh_handle) {
            // Access the position attribute mutably
            if let Some(VertexAttributeValues::Float32x3(positions)) = 
                mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) 
            {
                for pos in positions.iter_mut() {
                    // // Waves
                    // pos[1] = pos[0].sin() + pos[2].cos();

                    let mut height = 0.0;

                    for layer in &world_generator.perlin_layers {
                        height += layer.get_level(pos)
                    }

                    pos[1] = height * MAP_HEIGHT_SCALE;
                }
            }
            mesh.compute_smooth_normals();
        }
    }
}


fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        first_cascade_far_bound: MAP_SIZE/50.0,
        maximum_distance: LIGHTING_BOUNDS,
        ..default()
    }
    .build();

    // plane
    commands.spawn((
        Mesh3d(meshes.add(
        Plane3d::default().mesh()
        .size(MAP_SIZE, MAP_SIZE)
        .subdivisions(MAP_SIZE as u32 / 10)
    )),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.5, 0.3),
            perceptual_roughness: 1.0, // 1.0 is fully matte, 0.0 is a mirror
            reflectance: 0.1,          // Lowering this also reduces "specular" highlights
            ..default()
        })),
        Ground,
        //Wireframe,
    ));

    // Ocean
    commands.spawn((
        Mesh3d(meshes.add(
        Plane3d::default().mesh()
        .size(MAP_SIZE, MAP_SIZE)
    )),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.5))),
        NoWireframe,
        Transform::from_xyz(0.0, -2.0, 0.0),
    ));

    // testbox
    commands.spawn( (
        Mesh3d(meshes.add(dbg!(CuboidMeshBuilder::default()))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.5))),
        Transform::from_xyz(0.0, 35.0, 0.0),
        TestBox,
        Wireframe,
        )
    );
    
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


    commands.spawn((Text::new("Pos: N/A" ),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
        Debugger
    ));


}

fn setup_camera_fog(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-50.0, 50.0, 50.0).looking_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y),
        DistanceFog {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
            directional_light_exponent: 10.0,
            falloff: FogFalloff::from_visibility_colors(
                FOG_DISTANCE * 50.0, // distance in world units up to which objects retain visibility (>= 5% contrast)
                Color::srgb(0.35, 0.5, 0.66), // atmospheric extinction color (after light is lost due to absorption by atmospheric particles)
                Color::srgb(0.8, 0.844, 1.0), // atmospheric inscattering color (light gained due to scattering from the sun)
            ),
        },
    ));
}

#[derive(Component)]
struct Debugger;

fn update_debugger(
    camera: Query<&Transform, With<Camera>>,
    mut debugger: Query<&mut Text, With<Debugger>>,

) {
    let mut message = debugger.single_mut().unwrap();
    message.0 = format!("Position: {:?}", camera.single().unwrap().translation);
}


#[derive(Component)]
struct Ground;

#[derive(Component)]
struct TestBox;

fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
) {
    let mut transform = camera_query.single_mut().unwrap();
    let mut pan_speed = 50.0; 
    let rotation_speed = 1.0; 
    let mut pan_direction = Vec3::ZERO;
    let panning_delta = rotation_speed * time.delta_secs();

    if keyboard.pressed(KeyCode::ShiftLeft) {
        pan_speed *= 5.00;
    }

    let forward = transform.forward().as_vec3();
    let right = transform.right().as_vec3();
    let up = transform.up().as_vec3();

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
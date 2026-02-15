use bevy::{
    color::palettes::css::WHITE,
    mesh::{CuboidMeshBuilder, VertexAttributeValues},
    pbr::wireframe::{NoWireframe, Wireframe, WireframeConfig, WireframePlugin},
    prelude::*,
    render::{RenderPlugin, settings::{WgpuFeatures, WgpuSettings}}
};

use noise::{NoiseFn, Perlin};


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
            global: true,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .add_systems(Startup, (setup, modify_plane).chain())
        .add_systems(Update, camera_controls)
        .run();
}


fn modify_plane(
    query: Query<&Mesh3d, With<Ground>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
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

                    let base_layer = Perlin::new(0);
                
                    let horizontal_scale = 0.008;
                    let vertical_scale = 25.0;

                    let height = base_layer
                        .get([
                            (pos[0] * horizontal_scale) as f64,
                            (pos[2] * horizontal_scale) as f64
                        ]) as f32
                    ;
                    pos[1] = height * vertical_scale;
                }
            }
        }
    }
}


fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {

    // plane
    commands.spawn((
        Mesh3d(meshes.add(
        Plane3d::default().mesh()
        .size(500., 500.)
        .subdivisions(100)
    )),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Ground,
        NoWireframe,
    ));

    // Ocean
    commands.spawn((
        Mesh3d(meshes.add(
        Plane3d::default().mesh()
        .size(500., 500.)
    )),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.5))),
        NoWireframe,
        Transform::from_xyz(0.0, -2.0, 0.0),
    ));

    // testbox
    commands.spawn( (
        Mesh3d(meshes.add(CuboidMeshBuilder::default())),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.5))),
        Transform::from_xyz(0.0, 3.0, 0.0),
        TestBox,
        Wireframe,
        )
    );
    
    // light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_translation(Vec3::ONE).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(15.0, 5.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
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
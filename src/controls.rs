use bevy::{
    pbr::wireframe::WireframeConfig,
    prelude::*
};

#[derive(Component)]
pub struct MainCamera;

pub fn camera_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut wire_frame: ResMut<WireframeConfig>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
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


use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use std::sync::Arc;
use once_cell::sync::Lazy;

use crate::consts::world_units_to_meters;

pub static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime")
});

pub const _DEFAULT_SERVER_PORT: u16 = 7878;
pub const DEFAULT_SERVER_ADDR: &str = "192.168.0.184:7878";
const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Join { name: String },
    UpdatePosition { position: [f32; 3], rotation: [f32; 4] },
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        your_id: u32,
        seed: u32,
        existing_players: Vec<PlayerState>,
        time_of_day: f32,
        speed: f32,
    },
    PlayerJoined {
        player: PlayerState,
    },
    PlayerUpdate {
        id: u32,
        position: [f32; 3],
        rotation: [f32; 4],
    },
    PlayerLeft {
        id: u32,
    },
    Error {
        message: String,
    },
}

#[derive(Resource)]
pub struct NetworkClient {
    pub player_id: Option<u32>,
    pub connected: bool,
    pub world_seed: Option<u32>,
    pub original_seed: u32,
    send_tx: mpsc::UnboundedSender<ClientMessage>,
    recv_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<ServerMessage>>>,
    disconnect_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<()>>>,
}

impl NetworkClient {
    pub fn send(&self, message: ClientMessage) {
        let _ = self.send_tx.send(message);
    }

    pub async fn try_recv(&self) -> Option<ServerMessage> {
        let mut rx = self.recv_rx.lock().await;
        rx.try_recv().ok()
    }

    pub fn disconnect(&mut self) {
        if self.connected {
            self.send(ClientMessage::Disconnect);
            self.connected = false;
        }
    }
}

pub async fn connect_to_server(address: &str) -> Result<NetworkClient, String> {
    let stream = TcpStream::connect(address)
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let (read_half, write_half) = stream.into_split();

    let (send_tx, mut send_rx) = mpsc::unbounded_channel::<ClientMessage>();
    let (recv_tx, recv_rx) = mpsc::unbounded_channel::<ServerMessage>();
    let (disconnect_tx, disconnect_rx) = mpsc::unbounded_channel::<()>();

    let disconnect_tx_write = disconnect_tx.clone();
    TOKIO_RUNTIME.spawn(async move {
        let mut write_half = write_half;
        while let Some(message) = send_rx.recv().await {
            //println!("Client sending message: {:?}", message);
            if let Err(e) = send_message(&mut write_half, &message).await {
                eprintln!("Failed to send message: {}", e);
                let _ = disconnect_tx_write.send(());
                break;
            }
        }
        println!("Client write task ended");
    });

    let recv_tx_clone = recv_tx.clone();
    TOKIO_RUNTIME.spawn(async move {
        let mut read_half = read_half;
        println!("Client read task started");
        loop {
            match receive_message(&mut read_half).await {
                Ok(Some(message)) => {
                    //println!("Client received message: {:?}", message);
                    if recv_tx_clone.send(message).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    println!("Server disconnected");
                    let _ = disconnect_tx.send(());
                    break;
                }
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                    let _ = disconnect_tx.send(());
                    break;
                }
            }
        }
        println!("Client read task ended");
    });

    Ok(NetworkClient {
        player_id: None,
        connected: true,
        world_seed: None,
        original_seed: 0,
        send_tx,
        recv_rx: Arc::new(tokio::sync::Mutex::new(recv_rx)),
        disconnect_rx: Arc::new(tokio::sync::Mutex::new(disconnect_rx)),
    })
}

async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    message: &ClientMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = bincode::serialize(message)?;
    let len = data.len() as u32;
    
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    
    Ok(())
}

async fn receive_message(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
) -> Result<Option<ServerMessage>, Box<dyn std::error::Error>> {
    let mut len_bytes = [0u8; 4];
    
    match reader.read_exact(&mut len_bytes).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Ok(None);
        }
        Err(e) => return Err(Box::new(e)),
    }
    
    let len = u32::from_le_bytes(len_bytes) as usize;
    
    if len > MAX_MESSAGE_SIZE {
        return Err("Message too large".into());
    }
    
    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer).await?;
    
    let message = bincode::deserialize(&buffer)?;
    Ok(Some(message))
}

pub fn send_player_updates(
    client: Option<ResMut<NetworkClient>>,
    aircraft_query: Query<&Transform, With<crate::controls::Aircraft>>,
    time: Res<Time>,
    mut last_send: Local<f32>,
) {
    let Some(client) = client else { return };
    if !client.connected {
        return;
    }

    const SEND_INTERVAL: f32 = 1.0 / 120.0;
    if time.elapsed_secs() - *last_send < SEND_INTERVAL {
        return;
    }
    *last_send = time.elapsed_secs();

    if let Some(transform) = aircraft_query.iter().next() {
        let position = transform.translation;
        let rotation = transform.rotation;

        client.send(ClientMessage::UpdatePosition {
            position: [position.x, position.y, position.z],
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
        });
    }
}

pub fn check_connection_status(
    client: Option<ResMut<NetworkClient>>,
    mut commands: Commands,
    mut world_generator: ResMut<crate::world_generation::WorldGenerator>,
    mut chunk_manager: ResMut<crate::world_generation::ChunkManager>,
    chunks: Query<(Entity, &crate::world_generation::Chunk, Option<&Children>)>,
    mut render_settings: ResMut<crate::RenderSettings>,
) {
    let Some(mut client) = client else { return };
    if !client.connected {
        return;
    }

    let disconnected = TOKIO_RUNTIME.block_on(async {
        let mut disconnect_rx = client.disconnect_rx.lock().await;
        disconnect_rx.try_recv().is_ok()
    });

    if disconnected {
        println!("🔌 Server connection lost, triggering cleanup");
        client.connected = false;
        client.player_id = None;
        client.world_seed = None;
        
        let original_seed = client.original_seed;
        println!("🔄 Restoring original world seed {}", original_seed);
        
        *world_generator = crate::world_generation::WorldGenerator::new(original_seed);
        
        for (entity, chunk, children) in chunks.iter() {
            chunk_manager.spawned_chunks.remove(&(chunk.x, chunk.z));
            if let Some(children) = children {
                for child in children.iter() {
                    commands.entity(child).despawn();
                }
            }
            commands.entity(entity).despawn();
        }
        
        chunk_manager.last_camera_chunk = None;
        chunk_manager.to_spawn.clear();
        chunk_manager.lod_to_update.clear();
        render_settings.just_updated = true;
        
        commands.trigger(DisconnectCleanup);
        commands.trigger(RespawnAircraft);
    }
}

pub fn receive_server_messages(
    client: Option<ResMut<NetworkClient>>,
    mut commands: Commands,
    mut world_generator: ResMut<crate::world_generation::WorldGenerator>,
    mut chunk_manager: ResMut<crate::world_generation::ChunkManager>,
    chunks: Query<(Entity, &crate::world_generation::Chunk, Option<&Children>)>,
    mut render_settings: ResMut<crate::RenderSettings>,
    mut day_cycle: ResMut<crate::day_cycle::DayNightCycle>,
) {
    let Some(mut client) = client else { return };
    if !client.connected {
        return; 
    }

    TOKIO_RUNTIME.block_on(async {
        while let Some(message) = client.try_recv().await {
                match message {
                    ServerMessage::Welcome { your_id, seed, existing_players, time_of_day, speed } => {
                        println!("✅ Connected to server! Player ID: {}, Seed: {}", your_id, seed);
                        client.player_id = Some(your_id);
                        client.world_seed = Some(seed);
                        
                        day_cycle.time_of_day = time_of_day;
                        day_cycle.speed = speed;
                        
                        // Update world generator with server seed
                        *world_generator = crate::world_generation::WorldGenerator::new(seed);
                        
                        // Despawn all existing chunks and their vegetation
                        for (entity, chunk, children) in chunks.iter() {
                            chunk_manager.spawned_chunks.remove(&(chunk.x, chunk.z));
                            if let Some(children) = children {
                                for child in children.iter() {
                                    commands.entity(child).despawn();
                                }
                            }
                            commands.entity(entity).despawn();
                        }
                        
                        println!("🔄 Regenerating world with seed {}", seed);
                        
                        // Reset chunk manager state to trigger regeneration
                        chunk_manager.last_camera_chunk = None;
                        chunk_manager.to_spawn.clear();
                        chunk_manager.lod_to_update.clear();
                        
                        // Force chunk regeneration
                        render_settings.just_updated = true;
                        
                        // Respawn aircraft at spawn position
                        commands.trigger(RespawnAircraft);
                        
                        for player in existing_players {
                            println!("Player {} already in game", player.id);
                            commands.trigger(SpawnRemotePlayer(player));
                        }
                    }
                    ServerMessage::PlayerJoined { player } => {
                        println!("Player {} joined", player.id);
                        commands.trigger(SpawnRemotePlayer(player));
                    }
                    ServerMessage::PlayerUpdate { id, position, rotation } => {
                        commands.trigger(UpdateRemotePlayer { id, position, rotation });
                    }
                    ServerMessage::PlayerLeft { id } => {
                        println!("Player {} left", id);
                        commands.trigger(DespawnRemotePlayer(id));
                    }
                    ServerMessage::Error { message } => {
                        eprintln!("Server error: {}", message);
                    }
                }
            }
    });
}

#[derive(Event)]
pub struct SpawnRemotePlayer(pub PlayerState);

#[derive(Event)]
pub struct UpdateRemotePlayer {
    pub id: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Event)]
pub struct DespawnRemotePlayer(pub u32);

#[derive(Event)]
pub struct DisconnectCleanup;

#[derive(Event)]
pub struct RespawnAircraft;

#[derive(Event)]
pub struct TeleportToPlayer {
    pub player_id: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Component)]
pub struct RemotePlayer {
    pub player_id: u32,
}

#[derive(Component)]
pub struct LerpTarget {
    pub position: Vec3,
    pub last_position: Vec3,
    pub rotation: Quat,
}

#[derive(Resource)]
pub struct NetworkSmoothingSettings {
    pub half_life: f32,
    pub forward_offset: f32,
}

#[derive(Component)]
pub struct PlayerLabel;

#[derive(Component)]
pub struct PlayerLabelText {
    pub player_id: u32,
}

#[derive(Component)]
pub struct PlayerDistanceLabel {
    pub player_id: u32,
}

pub fn spawn_remote_player(
    trigger: On<SpawnRemotePlayer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let player_state = &trigger.0;
    
    let position = Vec3::from(player_state.position);
    let rotation = Quat::from_array(player_state.rotation);

    let plane_entity = commands.spawn((
        RemotePlayer { player_id: player_state.id },
        Transform::from_translation(position)
            .with_rotation(rotation)
            .with_scale(Vec3::splat(0.2)),
        LerpTarget {
            position,
            last_position: position,
            rotation,
        },
        Visibility::default(),
        InheritedVisibility::default(),
    )).id();

    let model_correction = commands.spawn(SceneRoot(
        asset_server.load("low-poly_airplane/scene.gltf#Scene0")
    )).insert(Transform::from_rotation(
        Quat::from_rotation_y((180.0f32).to_radians())
    )).id();

    let label = commands.spawn((
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.0, 0.0),
            emissive: LinearRgba::rgb(10.0, 0.0, 0.0),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 150.0, 0.0),
        PlayerLabel,
    )).id();

    commands.spawn((
        Text::new(format!("Player {}", player_state.id)),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        PlayerLabelText {
            player_id: player_state.id,
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
    ));

    commands.spawn((
        Text::new("0m"),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
        PlayerDistanceLabel {
            player_id: player_state.id,
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
    ));

    commands.entity(plane_entity).add_children(&[model_correction, label]);
}

pub fn update_remote_player(
    trigger: On<UpdateRemotePlayer>,
    mut query: Query<&mut LerpTarget, With<RemotePlayer>>,
    remote_players: Query<(Entity, &RemotePlayer)>,
) {
    let event = &trigger;
    
    for (entity, remote_player) in remote_players.iter() {
        if remote_player.player_id == event.id {
            if let Ok(mut lerp_target) = query.get_mut(entity) {
                lerp_target.last_position = lerp_target.position;
                lerp_target.position = Vec3::from(event.position);
                lerp_target.rotation = Quat::from_array(event.rotation);
            }
            break;
        }
    }
}

pub fn lerp_remote_players(
    mut query: Query<(&mut Transform, &LerpTarget), With<RemotePlayer>>,
    time: Res<Time>,
    settings: Res<NetworkSmoothingSettings>,
) {
    for (mut transform, lerp_target) in query.iter_mut() {
        let dt = time.delta_secs();
        let t = 1.0 - 0.5_f32.powf(dt / settings.half_life);
        
        let forward = lerp_target.rotation * Vec3::NEG_Z;
        let predicted_position = lerp_target.position + (lerp_target.position - lerp_target.last_position) + forward * settings.forward_offset;
        
        transform.translation = transform.translation.lerp(predicted_position, t);
        transform.rotation = transform.rotation.slerp(lerp_target.rotation, t);
    }
}

pub fn despawn_remote_player(
    trigger: On<DespawnRemotePlayer>,
    mut commands: Commands,
    query: Query<(Entity, &RemotePlayer)>,
    label_query: Query<(Entity, &PlayerLabelText)>,
    distance_label_query: Query<(Entity, &PlayerDistanceLabel)>,
) {
    let player_id = trigger.0;
    
    for (entity, remote_player) in query.iter() {
        if remote_player.player_id == player_id {
            commands.entity(entity).despawn();
            break;
        }
    }
    
    for (entity, label_text) in label_query.iter() {
        if label_text.player_id == player_id {
            commands.entity(entity).despawn();
            break;
        }
    }
    
    for (entity, distance_label) in distance_label_query.iter() {
        if distance_label.player_id == player_id {
            commands.entity(entity).despawn();
            break;
        }
    }
}


pub fn cleanup_on_disconnect(
    _trigger: On<DisconnectCleanup>,
    mut commands: Commands,
    query: Query<Entity, With<RemotePlayer>>,
    label_query: Query<Entity, With<PlayerLabelText>>,
    distance_label_query: Query<Entity, With<PlayerDistanceLabel>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    
    for entity in label_query.iter() {
        commands.entity(entity).despawn();
    }
    
    for entity in distance_label_query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn update_player_labels(
    camera_query: Query<(&GlobalTransform, &Camera), With<crate::controls::MainCamera>>,
    remote_players: Query<(&GlobalTransform, &RemotePlayer)>,
    aircraft_query: Query<&Transform, With<crate::controls::Aircraft>>,
    mut label_text_query: Query<(&mut Node, &PlayerLabelText)>,
    mut distance_label_query: Query<(&mut Node, &mut Text, &PlayerDistanceLabel), Without<PlayerLabelText>>,
) {
    let Ok((camera_transform, camera)) = camera_query.single() else { return };
    let Ok(aircraft_transform) = aircraft_query.single() else { return };
    
    for (mut style, label_text) in label_text_query.iter_mut() {
        for (player_transform, remote_player) in remote_players.iter() {
            if remote_player.player_id == label_text.player_id {
                let player_pos = player_transform.translation() + Vec3::new(0.0, 40.0, 0.0);
                
                if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, player_pos) {
                    style.left = Val::Px(screen_pos.x);
                    style.top = Val::Px(screen_pos.y);
                } else {
                    style.left = Val::Px(-1000.0);
                    style.top = Val::Px(-1000.0);
                }
                break;
            }
        }
    }
    
    for (mut style, mut text, distance_label) in distance_label_query.iter_mut() {
        for (player_transform, remote_player) in remote_players.iter() {
            if remote_player.player_id == distance_label.player_id {
                let distance = aircraft_transform.translation.distance(player_transform.translation());
                let meters = world_units_to_meters(distance);
                **text =  if distance < 1000.0 {
                    format!("{:.0}m", meters)
                } else {
                    format!("{}km", (meters / 100.0).round() / 10.0)
                };
                
                let player_pos = player_transform.translation() + Vec3::new(0.0, 40.0, 0.0);
                
                if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, player_pos) {
                    style.left = Val::Px(screen_pos.x);
                    style.top = Val::Px(screen_pos.y + 24.0);
                } else {
                    style.left = Val::Px(-1000.0);
                    style.top = Val::Px(-1000.0);
                }
                break;
            }
        }
    }
}

pub fn teleport_to_player(
    trigger: On<TeleportToPlayer>,
    mut aircraft_query: Query<&mut Transform, With<crate::controls::Aircraft>>,
    mut camera_query: Query<&mut Transform, (With<crate::controls::MainCamera>, Without<crate::controls::Aircraft>)>,
) {
    let event = &trigger;
    
    if let Ok(mut transform) = aircraft_query.single_mut() {
        transform.translation = Vec3::from(event.position);
        transform.rotation = Quat::from_array(event.rotation);
        
        if let Ok(mut camera_transform) = camera_query.single_mut() {
            camera_transform.translation = transform.translation + Vec3::new(0.0, 9.0, 0.0);
            camera_transform.rotation = camera_transform.looking_at(transform.translation, Vec3::Y).rotation;
        }
        
        println!("Teleported to Player {}", event.player_id);
    }
}

pub fn respawn_aircraft(
    _trigger: On<RespawnAircraft>,
    mut aircraft_query: Query<(&mut Transform, &mut crate::controls::Aircraft)>,
    mut camera_query: Query<(&mut Transform, &mut crate::controls::MainCamera), Without<crate::controls::Aircraft>>,
    world_gen: Res<crate::world_generation::WorldGenerator>,
) {
    if let Ok((mut transform, mut aircraft)) = aircraft_query.single_mut() {
        let spawn_pos = [0.0, 0.0, 0.0];
        let terrain_height = world_gen.get_terrain_height(&spawn_pos);
        let spawn_height = (terrain_height + aircraft.respawn_height).max(aircraft.respawn_height * 2.0);
        
        transform.translation = Vec3::new(0.0, spawn_height, 0.0);
        transform.rotation = Quat::IDENTITY;
        
        aircraft.crashed = false;
        aircraft.speed = aircraft.respawn_speed;
        aircraft.throttle = 0.8;
        aircraft.velocity = Vec3::ZERO;
        aircraft.pitch_velocity = 0.0;
        aircraft.roll_velocity = 0.0;
        aircraft.yaw_velocity = 0.0;
        
        if let Ok((mut camera_transform, mut main_camera)) = camera_query.single_mut() {
            main_camera.orbit_yaw = 0.0;
            main_camera.orbit_pitch = 0.0;
            main_camera.orbit_distance = aircraft.camera_distance;
            
            let camera_offset = Vec3::new(0.0, 9.0, main_camera.orbit_distance * 5.0);
            camera_transform.translation = transform.translation + camera_offset;
            camera_transform.rotation = camera_transform.looking_at(transform.translation, Vec3::Y).rotation;
        }
        
        println!("Aircraft respawned at spawn position");
    }
}

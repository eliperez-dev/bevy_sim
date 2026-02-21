mod protocol;

use protocol::{ClientMessage, PlayerState, ServerMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};

const SERVER_ADDR: &str = "0.0.0.0:7878";
const MAX_MESSAGE_SIZE: usize = 4096; 

type PlayerId = u32;
type PlayerMap = Arc<RwLock<HashMap<PlayerId, PlayerState>>>;
type ClientSender = mpsc::UnboundedSender<ServerMessage>;
type ClientSenders = Arc<RwLock<HashMap<PlayerId, ClientSender>>>;

struct GameServer {
    seed: u32,
    players: PlayerMap,
    senders: ClientSenders,
    next_player_id: Arc<RwLock<u32>>,
    time_of_day: Arc<RwLock<f32>>,
    speed: f32,
}

impl GameServer {
    fn new() -> Self {
        let seed = rand::random::<u32>();
        println!("🌍 Generated world seed: {}", seed);
        
        Self {
            seed,
            players: Arc::new(RwLock::new(HashMap::new())),
            senders: Arc::new(RwLock::new(HashMap::new())),
            next_player_id: Arc::new(RwLock::new(1)),
            time_of_day: Arc::new(RwLock::new(0.50)),
            speed: 0.003,
        }
    }

    async fn get_next_id(&self) -> PlayerId {
        let mut id = self.next_player_id.write().await;
        let current = *id;
        *id += 1;
        current
    }

    async fn broadcast(&self, message: ServerMessage, exclude: Option<PlayerId>) {
        let senders = self.senders.read().await;
        for (id, sender) in senders.iter() {
            if let Some(excluded_id) = exclude {
                if *id == excluded_id {
                    continue;
                }
            }
            let _ = sender.send(message.clone());
        }
    }

    async fn send_to(&self, player_id: PlayerId, message: ServerMessage) {
        let senders = self.senders.read().await;
        if let Some(sender) = senders.get(&player_id) {
            let _ = sender.send(message);
        }
    }
}

#[tokio::main]
async fn main() {
    println!("🚀 Flight Sim Server starting...");
    
    let server = Arc::new(GameServer::new());
    let listener = TcpListener::bind(SERVER_ADDR)
        .await
        .expect("Failed to bind server");
    
    println!("✅ Server listening on {}", SERVER_ADDR);
    println!("Waiting for players...\n");

    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        let mut last_update = std::time::Instant::now();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
            let now = std::time::Instant::now();
            let delta_secs = (now - last_update).as_secs_f32();
            last_update = now;

            let mut time = server_clone.time_of_day.write().await;
            *time = (*time + server_clone.speed * delta_secs) % 1.0;
        }
    });

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                println!("🔌 New connection from: {}", addr);
                let server = Arc::clone(&server);
                tokio::spawn(async move {
                    handle_client(server, socket).await;
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_client(server: Arc<GameServer>, socket: TcpStream) {
    let player_id = server.get_next_id().await;
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    
    server.senders.write().await.insert(player_id, tx);

    let (mut read_half, mut write_half) = socket.into_split();

    let server_clone = Arc::clone(&server);
    let write_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = send_message(&mut write_half, &message).await {
                eprintln!("❌ Failed to send to player {}: {}", player_id, e);
                break;
            }
        }
    });

    let existing_players: Vec<PlayerState> = server.players.read().await.values().cloned().collect();
    
    let welcome = ServerMessage::Welcome {
        your_id: player_id,
        seed: server.seed,
        existing_players,
        time_of_day: *server.time_of_day.read().await,
        speed: server.speed,
    };
    
    server.send_to(player_id, welcome).await;

    println!("✨ Player {} joined (total: {})", player_id, server.players.read().await.len() + 1);

    loop {
        match receive_message(&mut read_half).await {
            Ok(Some(msg)) => {
                match msg {
                    ClientMessage::Join { name: _ } => {
                    }
                    ClientMessage::UpdatePosition { position, rotation } => {
                        let player_state = PlayerState {
                            id: player_id,
                            position,
                            rotation,
                        };

                        let mut players = server.players.write().await;
                        let is_new = !players.contains_key(&player_id);
                        players.insert(player_id, player_state.clone());
                        drop(players);

                        if is_new {
                            server.broadcast(
                                ServerMessage::PlayerJoined {
                                    player: player_state,
                                },
                                Some(player_id),
                            ).await;
                        } else {
                            server.broadcast(
                                ServerMessage::PlayerUpdate {
                                    id: player_id,
                                    position,
                                    rotation,
                                },
                                Some(player_id),
                            ).await;
                        }
                    }
                    ClientMessage::Disconnect => {
                        println!("👋 Player {} disconnected gracefully", player_id);
                        break;
                    }
                }
            }
            Ok(None) => {
                println!("👋 Player {} disconnected", player_id);
                break;
            }
            Err(e) => {
                eprintln!("❌ Error reading from player {}: {}", player_id, e);
                break;
            }
        }
    }

    cleanup_player(&server_clone, player_id).await;
    write_task.abort();
}

async fn cleanup_player(server: &GameServer, player_id: PlayerId) {
    server.players.write().await.remove(&player_id);
    server.senders.write().await.remove(&player_id);
    
    server.broadcast(
        ServerMessage::PlayerLeft { id: player_id },
        None,
    ).await;
    
    println!("🧹 Player {} cleaned up (remaining: {})", player_id, server.players.read().await.len());
}

async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    message: &ServerMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = bincode::serialize(message)?;
    let len = data.len() as u32;
    
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&data).await?;
    writer.flush().await?;
    
    Ok(())
}

async fn receive_message(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
) -> Result<Option<ClientMessage>, Box<dyn std::error::Error + Send + Sync>> {
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

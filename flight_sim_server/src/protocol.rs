use serde::{Deserialize, Serialize};

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

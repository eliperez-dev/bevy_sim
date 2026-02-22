use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaneType {
    Light,
    Jet,
}

impl Default for PlaneType {
    fn default() -> Self {
        PlaneType::Light
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: u32,
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub plane_type: PlaneType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Join { name: String },
    UpdatePosition { name: String, position: [f32; 3], rotation: [f32; 4], plane_type: PlaneType },
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
        name: String,
        position: [f32; 3],
        rotation: [f32; 4],
        plane_type: PlaneType,
    },
    PlayerLeft {
        id: u32,
    },
    Error {
        message: String,
    },
}

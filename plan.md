# Multiplayer Implementation Plan

## Overview
Add local online multiplayer to the flight simulator using a Tokio-based server. The server stores the world seed and player positions/rotations, while clients sync their aircraft state and render other players' planes.

## Architecture
- **Server**: Tokio TCP server storing seed + player states
- **Client**: Bevy game client sending/receiving position/rotation data
- **Protocol**: Simple binary or JSON messages over TCP
- **UI**: EGUI menu for server connection

---

## Phase 1: Network Protocol & Shared Types
**Goal**: Define the message format and shared data structures

### Tasks:
1. Create `src/network.rs` module
2. Define message types:
   - `ClientToServer`: Join, Update (position/rotation), Disconnect
   - `ServerToClient`: Welcome (seed), PlayerJoined, PlayerUpdate, PlayerLeft
3. Define `PlayerState` struct (id, position, rotation)
4. Add serialization (serde + bincode or JSON)
5. Add dependencies to main Cargo.toml: `tokio`, `serde`, `bincode`

### Files to create/modify:
- `src/network.rs` (new)
- `Cargo.toml` (modify)

---

## Phase 2: Server Implementation
**Goal**: Build the Tokio server that manages game state

### Tasks:
1. Add dependencies to `flight_sim_server/Cargo.toml`:
   - `tokio` with full features
   - `serde`, `bincode`
2. Implement server in `flight_sim_server/src/main.rs`:
   - Generate and store world seed on startup
   - Accept TCP connections on localhost:7878
   - Maintain HashMap of connected players
   - Broadcast player updates to all clients
   - Handle disconnections
3. Simple message handling loop per client
4. Test with telnet/nc to verify connectivity

### Files to modify:
- `flight_sim_server/Cargo.toml`
- `flight_sim_server/src/main.rs`

---

## Phase 3: Client Network System
**Goal**: Connect game client to server and sync local player

### Tasks:
1. Add `NetworkClient` resource to hold TCP connection
2. Create `network::connect_to_server()` function
3. Add Bevy system to send player position/rotation updates
4. Add Bevy system to receive server messages
5. Store world seed from server and regenerate world
6. Handle connection errors gracefully

### Files to modify:
- `src/network.rs`
- `src/main.rs` (add network systems)

---

## Phase 4: Remote Player Rendering
**Goal**: Spawn and update plane models for other players

### Tasks:
1. Create `RemotePlayer` component with player_id
2. Add system to spawn plane model when receiving PlayerJoined
3. Add system to update remote player transforms from PlayerUpdate messages
4. Add system to despawn planes when receiving PlayerLeft
5. Use same plane model as local player (low-poly_airplane)

### Files to modify:
- `src/network.rs` (add remote player logic)
- `src/main.rs` (add remote player systems)

---

## Phase 5: Connection UI Menu
**Goal**: Add EGUI menu to join servers

### Tasks:
1. Add `MultiplayerMenu` resource/state
2. Create `multiplayer_menu_ui()` system in `src/hud.rs`
3. UI elements:
   - Text input for server IP/port
   - "Connect" button
   - "Disconnect" button (when connected)
   - Connection status display
   - Error messages
4. Toggle menu with key (e.g., 'M')
5. Show menu on startup or on demand

### Files to modify:
- `src/hud.rs`
- `src/main.rs` (add menu system, keybinds)

---

## Phase 6: Testing & Polish
**Goal**: Ensure stability and usability

### Tasks:
1. Test with 2+ clients connecting to local server
2. Handle edge cases:
   - Server shutdown while clients connected
   - Client timeout/disconnect
   - Invalid server addresses
3. Add basic logging for debugging
4. Verify seed synchronization creates identical worlds
5. Smooth interpolation for remote player movement (optional)
6. Test crash/respawn behavior in multiplayer

### Files to modify:
- Various (bug fixes)

---

## Technical Notes

### Network Message Flow
```
Client -> Server:  Join(name) 
Server -> Client:  Welcome(seed, your_id, existing_players)
Client -> Server:  Update(pos, rot) [periodic]
Server -> All:     PlayerUpdate(id, pos, rot)
Client -> Server:  Disconnect()
Server -> All:     PlayerLeft(id)
```

### Dependencies to Add
Main client (`Cargo.toml`):
- `tokio = { version = "1", features = ["rt", "net", "io-util"] }`
- `serde = { version = "1", features = ["derive"] }`
- `bincode = "1"`

Server (`flight_sim_server/Cargo.toml`):
- `tokio = { version = "1", features = ["full"] }`
- `serde = { version = "1", features = ["derive"] }`
- `bincode = "1"`

### Default Server Address
- `localhost:7878`

### Update Frequency
- Send position updates every ~50ms (20Hz) or every frame with throttling
- Server broadcasts immediately upon receiving updates

---

## Implementation Order
1. Phase 1: Protocol definitions
2. Phase 2: Server (can test independently)
3. Phase 3: Client networking
4. Phase 4: Remote player rendering
5. Phase 5: UI menu
6. Phase 6: Testing

---

## Future Enhancements (Post-MVP)
- Interpolation/extrapolation for smooth remote player movement
- Latency compensation
- UDP for position updates (TCP for control messages)
- Server browser / lobby system
- Chat functionality
- Dedicated server mode (headless)
- NAT traversal / public server support

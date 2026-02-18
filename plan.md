# Flight Simulator Development Plan

## Current State
- ✅ Procedural terrain generation with LOD system
- ✅ Day/night cycle with dynamic lighting
- ✅ Low poly Cessna aircraft model loaded
- ✅ Free camera controls (WASD + arrows)
- ✅ Multi-biome world (grasslands, desert, taiga, forest)
- ✅ Fog and atmospheric effects
- ✅ Debug UI with EGUI

## Ideas for Flight Simulator Features

### Phase 1: Basic Flight Mechanics (Single Player)

1. **Dual Control Mode System** ⚠️ STARTING HERE
   - **Free Flight Mode (Spectator)**:
     - Keep existing camera controls (WASD + arrows for movement/rotation)
     - Hide plane model when in this mode
     - Useful for exploring world and debugging
   - **Aircraft Mode**:
     - Same control scheme as free flight (WASD + arrows)
     - Controls now move the plane entity instead of a free camera
     - Third-person camera follows/tracks the plane
     - Plane model visible
     - Toggle between modes with a key (e.g., F or V)
   - **Implementation Notes**:
     - Reuse existing `camera_controls` logic
     - Apply transforms to plane entity in aircraft mode
     - Apply transforms to camera entity in free flight mode
     - Component to track current mode: `ControlMode { Free, Aircraft }`
     - NO physics simulation yet - direct arcade control movement

2. **Aircraft Physics Component** (DEFERRED - Phase 2)
   - Create `Aircraft` component with properties:
     - velocity, acceleration
     - pitch, roll, yaw angles
     - thrust, drag, lift forces
     - mass, wing area
   - Simple arcade-style physics (not full simulation)
   - Stall behavior at low speeds
   - Speed limits (min/max airspeed)

3. **Flight Controls Refinement** (After physics)
   - Map controls to realistic flight:
     - W/S: Throttle up/down
     - A/D: Roll left/right
     - Arrow Up/Down: Pitch
     - Arrow Left/Right: Yaw (rudder)
     - Q/E: Camera orbit/zoom
     - C: Cycle camera views (cockpit, chase, free)
   - Gamepad support for better flight feel

3. **HUD (Heads-Up Display)**
   - Altitude indicator
   - Airspeed indicator
   - Heading compass
   - Artificial horizon
   - Throttle percentage
   - Optional: Simple radar/minimap

4. **Aircraft Model Integration**
   - Attach camera to plane entity
   - Add animated control surfaces (ailerons, elevator, rudder) if models support it
   - Engine sound effects (pitch varies with throttle)
   - Wind sound effects (volume varies with speed)

### Phase 2: Enhanced Gameplay
5. **Airport/Landing Strips**
   - Generate flat areas with runway markers in certain biomes
   - Spawn points at airports
   - Landing detection and scoring (smooth vs rough landing)

6. **Weather System**
   - Wind (affects flight physics)
   - Turbulence zones
   - Rain/snow particles (visual only or affects visibility)
   - Cloud layer (could use fog/particle effects)

7. **Additional Aircraft**
   - Multiple aircraft models with different characteristics:
     - Cessna (already have): Light, slow, easy to fly
     - Jet: Fast, harder to control
     - Glider: No engine, relies on thermals
   - Selection menu at spawn

### Phase 3: Multiplayer Foundation
8. **Networking Library Selection**
   - Options to consider:
     - `bevy_quinnet` - UDP/QUIC networking, good for real-time
     - `bevy_replicon` - ECS replication framework
     - `matchbox` - WebRTC-based, works with WebAssembly
     - `lightyear` - Full netcode solution with client prediction
   - Recommendation: **lightyear** (most complete, handles interpolation/prediction)

9. **Client-Server Architecture**
   - Server authoritative model:
     - Server simulates all aircraft physics
     - Clients send input commands
     - Server broadcasts state updates
   - Client prediction for local player (reduce input lag)
   - Interpolation for remote players (smooth movement)

10. **Player Entity System**
    - Each player controls one aircraft
    - Player ID mapping to aircraft entity
    - Player spawn/despawn handling
    - Reconnection handling

11. **State Synchronization**
    - Replicate aircraft transforms (position, rotation)
    - Replicate physics state (velocity, angular velocity)
    - Replicate control inputs (for animations)
    - Optimize: Only sync entities within visibility range
    - World generation seed shared across clients

### Phase 4: Multiplayer Features
12. **Lobby/Connection System**
    - Server browser or direct connect by IP
    - Player limit (start with 4-8 players)
    - Player list UI
    - Chat system (text-based)

13. **Multiplayer Gameplay**
    - Free flight mode (explore together)
    - Race checkpoints (fly through gates)
    - Formation flying challenges
    - Tag/chase games
    - Optional: Combat mode (simple projectiles)

14. **Player Identification**
    - Player name tags (visible at distance)
    - Different aircraft colors per player
    - Position indicators on HUD for other players

### Phase 5: Polish & Optimization
15. **Performance Optimization**
    - Network bandwidth optimization (delta compression)
    - LOD for remote aircraft
    - Culling players outside render distance
    - Chunk loading synchronized across clients

16. **Audio**
    - Engine sounds (varies with throttle)
    - Wind sounds (varies with speed)
    - Stall warning audio cue
    - Doppler effect for other players

17. **UI/UX Improvements**
    - Main menu (single player, multiplayer, settings)
    - Control remapping screen
    - Graphics settings (render distance, quality)
    - Tutorial/control hints overlay

## Technical Considerations

### Networking Challenges
- **Terrain Sync**: Share world generation seed instead of syncing terrain data
- **Latency**: Use client-side prediction for local aircraft, interpolation for others
- **Bandwidth**: Limit update rate (20-60 Hz), use compression
- **Cheating**: Server-authoritative collision and scoring

### Architecture Recommendations
```
src/
├── main.rs
├── aircraft/           # NEW
│   ├── mod.rs
│   ├── components.rs  (Aircraft, FlightPhysics)
│   ├── physics.rs     (flight simulation)
│   └── controls.rs    (input → physics)
├── camera/            # NEW (refactor from controls.rs)
│   ├── mod.rs
│   └── flight_camera.rs
├── hud/               # NEW
│   └── flight_hud.rs
├── multiplayer/       # NEW
│   ├── mod.rs
│   ├── network.rs     (connection handling)
│   ├── replication.rs (state sync)
│   └── lobby.rs       (player management)
├── world_generation/
│   └── (keep existing)
├── day_cycle/
│   └── (keep existing)
└── consts.rs
```

### Dependency Additions
```toml
# For Phase 1-2
bevy_kira_audio = "0.21"     # Audio
leafwing-input-manager = "*" # Better input handling

# For Phase 3-4 (pick one networking solution)
lightyear = "0.18"           # Recommended: full netcode
# OR
bevy_quinnet = "*"           # Alternative: lower-level
bevy_replicon = "*"          # Pairs with quinnet

# Optional
serde = { version = "*", features = ["derive"] }
bincode = "*"                # For serialization
```

### Low Poly Art Style
- Maintain the existing low-poly aesthetic
- Keep chunk LOD system (works great for flight sim scale)
- Simple geometric shapes for runways/checkpoints
- Flat-shaded or minimal lighting for performance

### Multiplayer Scalability
- **Target**: 4-16 players initially
- **Server Requirements**: 
  - Host server separately OR
  - Player-hosted (one client acts as server)
- **Cross-platform**: Consider WebAssembly support for browser play (use matchbox)

## Progressive Implementation Path

### Step 0: Dual Control Modes (Day 1-2) ⚠️ CURRENT TASK
1. Implement Free Flight / Aircraft mode toggle
2. In Free Flight: hide plane, control camera directly (current behavior)
3. In Aircraft: show plane, move plane with same controls, camera follows
4. Add mode indicator to debug UI

### Minimum Viable Flight Sim (Week 1-2)
1. Basic aircraft physics (pitch, roll, yaw, thrust)
2. Refine third-person camera (smooth follow, banking)
3. Simple HUD (altitude, speed, mode indicator)
4. Tune flight controls for physics model

### Single Player Fun (Week 3-4)
5. Landing strips on terrain
6. Multiple aircraft selection
7. Basic sound effects
8. Improved camera modes (cockpit view)

### Multiplayer Prototype (Week 5-8)
9. Integrate networking library
10. Client-server connection
11. See other players flying
12. Basic synchronization

### Full Multiplayer (Week 9-12)
13. Lobby system
14. Player identification
15. Multiplayer game modes
16. Polish and optimization

## Success Criteria
- ✈️ Smooth, intuitive flight controls
- 🌍 Infinite procedural world exploration
- 👥 Stable multiplayer with 8+ players
- 📡 Low latency (<100ms perceived delay)
- 🎮 Fun arcade-style physics (not overly realistic)
- 🎨 Consistent low-poly aesthetic

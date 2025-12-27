# HYPERION Architecture

This document provides a high-level overview of the HYPERION server architecture.

## System Overview

HYPERION is a spaceship bridge simulation game server built in Rust. The server exposes a REST/GraphQL API and WebSocket streaming service that game clients connect to.

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Game Clients                               │
│              (Web UI, Desktop Apps, Custom Frontends)               │
└──────────────────┬─────────────────────────┬────────────────────────┘
                   │                         │
            REST/GraphQL              WebSocket (real-time)
                   │                         │
┌──────────────────▼─────────────────────────▼────────────────────────┐
│                        Rocket Web Server                            │
│                         (src/server.rs)                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │  REST API   │  │  WebSocket Mgr  │  │   Event Broadcaster     │  │
│  │ (src/api/)  │  │(src/websocket.rs│  │(src/event_broadcaster.rs│  │
│  └──────┬──────┘  └────────┬────────┘  └────────────┬────────────┘  │
│         │                  │                        │               │
├─────────▼──────────────────▼────────────────────────▼───────────────┤
│                         Game State                                  │
│                       (src/state.rs)                                │
│                                                                     │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │                      GameWorld                              │   │
│   │  - Players, Teams, Blueprints, Ships, Stations              │   │
│   │  - Event Queue                                              │   │
│   │  - AI Manager                                               │   │
│   │  - Bevy ECS World (entity management)                       │   │
│   └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│                        Simulation Layer                             │
│                      (src/simulation/)                              │
│                                                                     │
│   ┌───────────────┐  ┌───────────────┐  ┌───────────────────────┐   │
│   │   Physics     │  │  Components   │  │      Systems          │   │
│   │ (physics.rs)  │  │(components.rs)│  │    (systems.rs)       │   │
│   └───────────────┘  └───────────────┘  └───────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Configuration Layer                            │
│                        (src/config/)                                │
│                                                                     │
│  Ship Classes │ Modules │ Weapons │ AI │ Map │ Factions │ Settings  │
│                         (data/*.yaml)                               │
└─────────────────────────────────────────────────────────────────────┘
```

## Core Modules

### Entry Point ([src/main.rs](../src/main.rs))

The CLI entry point using `clap`. Parses command-line arguments, initializes logging, loads configuration from the data directory, and launches the Rocket server.

### Server ([src/server.rs](../src/server.rs))

Configures and launches the Rocket web server:
- Initializes shared game state (`GameWorld`)
- Sets up WebSocket manager for real-time connections
- Configures CORS for cross-origin requests
- Mounts REST and GraphQL route handlers
- Starts the `EventBroadcaster` background task

### Game State ([src/state.rs](../src/state.rs))

The `GameWorld` struct is the central state container, wrapped in `Arc<RwLock<>>` for thread-safe access:

| Registry | Description |
|----------|-------------|
| `players` | Registered player accounts |
| `teams` | Player teams with credit balances |
| `blueprints` | Ship designs awaiting compilation |
| `ships` | Active ships in the simulation |
| `stations` | Space stations and docking facilities |
| `ai_manager` | AI behavior management |
| `event_queue` | Pending events for WebSocket broadcast |

Key methods include player/team management, blueprint creation, ship registration, and event queuing.

### API Layer ([src/api/](../src/api/))

REST endpoints organized by domain:

| Module | Routes | Description |
|--------|--------|-------------|
| `players` | `/players/*` | Player registration and lookup |
| `teams` | `/teams/*` | Team creation, membership, credits |
| `blueprints` | `/blueprints/*` | Ship blueprint CRUD |
| `ships` | `/ships/*` | Active ship management |
| `ship_classes` | `/ship-classes/*` | Available ship class catalog |
| `modules` | `/modules/*` | Module definitions and variants |
| `catalog` | `/catalog/*` | Full game data catalog |
| `stations` | `/stations/*` | Space station data |
| `factions` | `/factions/*` | Faction information |
| `generation` | `/generation/*` | Procedural universe generation |
| `ai` | `/ai/*` | AI behavior configuration |

#### Bridge Position APIs ([src/api/positions/](../src/api/positions/))

Specialized endpoints for each bridge station:

| Position | Responsibilities |
|----------|------------------|
| `captain` | Ship-wide commands, crew management, captain's log |
| `helm` | Navigation, thrust, warp/jump drives |
| `engineering` | Power allocation, cooling, repairs |
| `science` | Sensors, scanning, contact analysis |
| `comms` | Hailing, docking requests, messaging |
| `energy_weapons` | Directed energy weapons targeting and firing |
| `kinetic_weapons` | Ballistic weapons, ammunition management |
| `missile_weapons` | Missile/torpedo targeting and launch |
| `countermeasures` | Defensive systems, point defense, decoys |

### WebSocket System

#### WebSocket Manager ([src/websocket.rs](../src/websocket.rs))

Handles real-time client connections:
- Client registration and subscription management
- Event filtering based on subscriptions (ship, player, simulation)
- Broadcast channel for event distribution

#### Event Broadcaster ([src/event_broadcaster.rs](../src/event_broadcaster.rs))

Background service that:
- Periodically drains events from `GameWorld.event_queue`
- Broadcasts events through `WebSocketManager` to subscribed clients
- Runs at ~60fps (16ms intervals) by default

### Simulation ([src/simulation/](../src/simulation/))

Entity-Component-System (ECS) architecture using Bevy ECS:

| File | Purpose |
|------|---------|
| `components.rs` | ECS components for ships, modules, weapons |
| `systems.rs` | ECS systems that process component updates |
| `physics.rs` | 3D physics calculations |
| `module_state.rs` | Runtime module state tracking |
| `loop.rs` | Main simulation loop |

### Configuration ([src/config/](../src/config/))

YAML-based configuration loaded at startup:

| Module | Config Files | Description |
|--------|--------------|-------------|
| `ship_class.rs` | `data/ship-classes/*.yaml` | Ship hull types and stats |
| `module.rs` | `data/modules/**/*.yaml` | Module definitions and variants |
| `weapon.rs` | `data/modules/*-weapons/*.yaml` | Weapon systems |
| `ai.rs` | `data/ai.yaml` | AI behavior parameters |
| `map.rs` | `data/map.yaml`, `data/procedural_generation.yaml` | Galaxy generation |
| `simulation.rs` | `data/simulation.yaml` | Physics and simulation tuning |
| `faction_gen.rs` | `data/faction_generation.yaml` | Faction generation rules |
| `game_settings.rs` | `data/game.yaml` | Economy and game rules |

Configuration is validated on load and accessible via `GameConfig`.

### Ship Blueprint System

#### Blueprint Model ([src/models/blueprint.rs](../src/models/blueprint.rs))

Defines the structure for ship designs before they become active:
- Ship class selection
- Module slot configuration with variant selection
- Weapon loadout
- Player role assignments
- Ready status tracking

#### Blueprint Validation ([src/blueprint.rs](../src/blueprint.rs))

Validates blueprints before compilation:
- Ship class exists and is valid
- Weight within limits
- Module count within limits
- Required modules present
- Module variants properly configured
- All players assigned roles and marked ready

#### Ship Compiler ([src/compiler.rs](../src/compiler.rs))

Converts validated blueprints into active ships:
1. Validates blueprint completeness
2. Calculates credit cost (ship class + modules + variants)
3. Verifies team has sufficient credits
4. Compiles modules by resolving variant stats
5. Initializes ship systems (hull, shields, power, cooling)
6. Deducts credits from team
7. Spawns ship into game world

### AI System ([src/ai/](../src/ai/))

Behavior tree-based AI for NPC ships:

| File | Purpose |
|------|---------|
| `behavior_tree.rs` | Core behavior tree implementation |
| `ships.rs` | Ship-specific AI contexts and commands |
| `system.rs` | AI manager and update systems |

AI personalities include combat, patrol, and trading behaviors.

### Procedural Generation ([src/generation/](../src/generation/))

Generates dynamic game content:

| File | Generates |
|------|-----------|
| `galaxy.rs` | Galaxy structure, sectors, star placement |
| `systems.rs` | Star systems, planets, asteroid belts |
| `factions.rs` | Alien factions with traits and relationships |
| `languages.rs` | Procedural alien languages |
| `history.rs` | Historical events between factions |

A `ProceduralUniverse` combines all generated content with a seed for reproducibility.

## Data Flow

### Ship Creation Flow

```
1. Client creates blueprint via POST /blueprints
2. Client configures modules via POST /blueprints/{id}/modules
3. Client assigns players/roles via POST /blueprints/{id}/players
4. Players mark ready via POST /blueprints/{id}/ready
5. Client compiles ship via POST /blueprints/{id}/compile
   └── BlueprintValidator checks constraints
   └── ShipCompiler resolves module stats
   └── Credits deducted from team
   └── Ship spawned into GameWorld
6. Ship available via GET /ships/{id}
```

### Real-Time Event Flow

```
1. Simulation tick updates game state
2. State changes push GameEvent to GameWorld.event_queue
3. EventBroadcaster drains queue (every 16ms)
4. WebSocketManager broadcasts events
5. Clients receive filtered events based on subscriptions
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `rocket` | Web framework (REST API) |
| `rocket_ws` | WebSocket support |
| `juniper` | GraphQL implementation |
| `bevy_ecs` | Entity Component System |
| `serde` / `serde_yaml` | Configuration parsing |
| `nalgebra` | Linear algebra for physics |
| `tokio` | Async runtime |
| `uuid` | Entity identifiers |

## Thread Safety

The `GameWorld` is wrapped in `Arc<RwLock<GameWorld>>` (aliased as `SharedGameWorld`):
- Multiple readers can access state concurrently
- Write operations acquire exclusive locks
- Background tasks (simulation, broadcasting) coordinate via the lock

## Extension Points

### Adding New Bridge Positions

1. Create module in `src/api/positions/`
2. Define position-specific endpoints
3. Add routes to `src/api.rs`
4. Add role to `ShipRole` enum in `src/models/role.rs`

### Adding New Module Types

1. Create YAML files in `data/modules/{type}/`
2. Add slot definition in `data/module-slots/`
3. Module variants automatically loaded and validated

### Adding New Ship Classes

1. Create YAML in `data/ship-classes/`
2. ID derived from filename
3. Available via `/ship-classes` API

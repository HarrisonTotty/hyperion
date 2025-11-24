# HYPERION API Reference

**Version**: 0.1.0  
**Base URL**: `http://localhost:8000`  
**API Prefix**: `/v1`

---

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [REST API](#rest-api)
   - [Players](#players)
   - [Teams](#teams)
   - [Factions](#factions)
   - [Blueprints](#blueprints)
   - [Ships](#ships)
   - [Stations](#stations)
   - [AI Ships](#ai-ships)
   - [Procedural Generation](#procedural-generation)
   - [Ship Positions](#ship-positions)
     - [Captain](#captain)
     - [Helm](#helm)
     - [Engineering](#engineering)
     - [Communications](#communications)
     - [Science](#science)
     - [Energy Weapons](#energy-weapons)
     - [Kinetic Weapons](#kinetic-weapons)
     - [Missile Weapons](#missile-weapons)
     - [Countermeasures](#countermeasures)
4. [WebSocket API](#websocket-api)
5. [GraphQL API](#graphql-api)
6. [Error Codes](#error-codes)
7. [Examples](#examples)

---

## Overview

The HYPERION API provides a comprehensive REST interface for managing a spaceship bridge simulation game. The API follows RESTful principles and returns JSON responses.

### Key Features

- **Player Management**: Register players and create teams
- **Ship Design**: Create and customize ship blueprints
- **Ship Operations**: Control all aspects of ship operation across 9 bridge positions
- **Procedural Generation**: Generate and explore galaxies, star systems, and factions
- **AI Ships**: Create and control AI-driven ships
- **Real-time Updates**: WebSocket streaming for live game state updates

### Response Format

All API responses follow this general format:

**Success Response**:
```json
{
  "id": "unique-id",
  "field1": "value1",
  "field2": "value2"
}
```

**Error Response**:
```json
{
  "error": "Error description"
}
```

### HTTP Status Codes

- `200 OK` - Request succeeded
- `201 Created` - Resource created successfully
- `400 Bad Request` - Invalid request data
- `404 Not Found` - Resource not found
- `422 Unprocessable Entity` - Validation failed
- `500 Internal Server Error` - Server error

---

## Authentication

**Current Status**: No authentication required (development mode)

**Future Implementation**: 
- Token-based authentication
- Player session management
- Team-based authorization

---

## REST API

### Players

Manage player registration and information.

#### List Players

```
GET /v1/players
```

**Response**: Array of Player objects

```json
[
  {
    "id": "player-uuid",
    "name": "Alice",
    "team_id": "team-uuid"
  }
]
```

#### Create Player

```
POST /v1/players
Content-Type: application/json
```

**Request Body**:
```json
{
  "name": "Alice"
}
```

**Response**: Player object (201 Created)

#### Get Player

```
GET /v1/players/{id}
```

**Response**: Player object

#### Delete Player

```
DELETE /v1/players/{id}
```

**Response**: 200 OK

---

### Teams

Manage teams and team membership.

#### List Teams

```
GET /v1/teams
```

**Response**: Array of Team objects

```json
[
  {
    "id": "team-uuid",
    "name": "Alpha Squad",
    "faction": "federation",
    "members": ["player-uuid-1", "player-uuid-2"]
  }
]
```

#### Create Team

```
POST /v1/teams
Content-Type: application/json
```

**Request Body**:
```json
{
  "name": "Alpha Squad",
  "faction": "federation"
}
```

**Response**: Team object (201 Created)

#### Get Team

```
GET /v1/teams/{id}
```

**Response**: Team object

#### Add Player to Team

```
PATCH /v1/teams/{id}
Content-Type: application/json
```

**Request Body**:
```json
{
  "player_id": "player-uuid"
}
```

**Response**: 200 OK

#### Remove Player from Team

```
DELETE /v1/teams/{team_id}/players/{player_id}
```

**Response**: 200 OK

---

### Factions

Retrieve faction information.

#### List Factions

```
GET /v1/factions
```

**Response**: Array of Faction objects

```json
[
  {
    "id": "federation",
    "name": "United Federation",
    "description": "A democratic alliance...",
    "traits": ["diplomatic", "scientific"]
  }
]
```

---

### Blueprints

Design and configure ship blueprints before launching.

#### List Blueprints

```
GET /v1/blueprints
```

**Response**: Array of Blueprint objects

#### Create Blueprint

```
POST /v1/blueprints
Content-Type: application/json
```

**Request Body**:
```json
{
  "name": "USS Enterprise",
  "ship_class": "battleship",
  "faction": "federation"
}
```

**Response**: Blueprint object (201 Created)

#### Get Blueprint

```
GET /v1/blueprints/{id}
```

**Response**: Blueprint object with modules and crew assignments

#### Join Blueprint

```
POST /v1/blueprints/{id}/join
Content-Type: application/json
```

**Request Body**:
```json
{
  "player_id": "player-uuid"
}
```

**Response**: 200 OK

#### Update Crew Roles

```
PATCH /v1/blueprints/{id}/roles
Content-Type: application/json
```

**Request Body**:
```json
{
  "assignments": {
    "player-uuid-1": "captain",
    "player-uuid-2": "helm",
    "player-uuid-3": "engineering"
  }
}
```

**Response**: 200 OK

**Available Roles**:
- `captain` - Overall command
- `helm` - Navigation and movement
- `engineering` - Power and repairs
- `comms` - Communications
- `science` - Sensors and analysis
- `energy_weapons` - Energy weapon systems
- `kinetic_weapons` - Kinetic weapon systems
- `missile_weapons` - Missile systems
- `countermeasures` - Shields and countermeasures

#### Add Module

```
POST /v1/blueprints/{id}/modules
Content-Type: application/json
```

**Request Body**:
```json
{
  "module_id": "phaser-array-mk2",
  "position": {"x": 10, "y": 5, "z": 0}
}
```

**Response**: 200 OK

#### Remove Module

```
DELETE /v1/blueprints/{id}/modules/{module_id}
```

**Response**: 200 OK

#### Configure Module

```
PATCH /v1/blueprints/{id}/modules/{module_id}
Content-Type: application/json
```

**Request Body**:
```json
{
  "power_allocation": 75,
  "mode": "auto"
}
```

**Response**: 200 OK

#### Mark Player Ready

```
POST /v1/blueprints/{id}/ready
Content-Type: application/json
```

**Request Body**:
```json
{
  "player_id": "player-uuid"
}
```

**Response**: 200 OK

#### Unmark Player Ready

```
DELETE /v1/blueprints/{id}/ready/{player_id}
```

**Response**: 200 OK

#### Validate Blueprint

```
GET /v1/blueprints/{id}/validate
```

**Response**: Validation results

```json
{
  "valid": true,
  "issues": [],
  "warnings": ["Low shield coverage in sector 3"]
}
```

---

### Ships

Manage active ships in the game world.

#### Compile Ship from Blueprint

```
POST /v1/ships/compile
Content-Type: application/json
```

**Request Body**:
```json
{
  "blueprint_id": "blueprint-uuid"
}
```

**Response**: Ship object (201 Created)

**Note**: All players must be marked ready before compilation.

#### List Ships

```
GET /v1/ships
```

**Response**: Array of Ship objects

#### Get Ship

```
GET /v1/ships/{id}
```

**Response**: Ship object with full state

```json
{
  "id": "ship-uuid",
  "name": "USS Enterprise",
  "class": "battleship",
  "position": {"x": 1000.0, "y": 2000.0, "z": 500.0},
  "velocity": {"x": 10.0, "y": 0.0, "z": 5.0},
  "rotation": {"w": 1.0, "x": 0.0, "y": 0.0, "z": 0.0},
  "hull": 1000.0,
  "max_hull": 1000.0,
  "shields": 500.0,
  "max_shields": 500.0,
  "power": 750.0,
  "max_power": 1000.0,
  "crew": [...],
  "modules": [...]
}
```

---

### Stations

Manage space stations and docking operations.

#### List Stations

```
GET /v1/stations
```

**Response**: Array of Station objects

#### Get Station

```
GET /v1/stations/{id}
```

**Response**: Station object

#### Create Station

```
POST /v1/stations
Content-Type: application/json
```

**Request Body**:
```json
{
  "name": "Deep Space 9",
  "position": {"x": 5000.0, "y": 3000.0, "z": 1000.0},
  "faction": "federation"
}
```

**Response**: Station object (201 Created)

#### Delete Station

```
DELETE /v1/stations/{id}
```

**Response**: 200 OK

#### Request Docking

```
POST /v1/stations/{station_id}/dock
Content-Type: application/json
```

**Request Body**:
```json
{
  "ship_id": "ship-uuid"
}
```

**Response**: Docking bay assignment

#### Complete Docking

```
POST /v1/stations/{station_id}/dock/{ship_id}/complete
```

**Response**: 200 OK

#### Undock Ship

```
POST /v1/stations/{station_id}/undock/{ship_id}
```

**Response**: 200 OK

#### Request Station Services

```
POST /v1/stations/{station_id}/services/{ship_id}
Content-Type: application/json
```

**Request Body**:
```json
{
  "services": ["repair", "resupply", "refuel"]
}
```

**Response**: Service quote and timing

#### Get Docking Status

```
GET /v1/stations/{station_id}/dock/{ship_id}
```

**Response**: Docking status information

---

### AI Ships

Create and manage AI-controlled ships.

#### List AI Ships

```
GET /v1/ai/ships
```

**Response**: Array of AI ship objects

#### Get AI Ship

```
GET /v1/ai/ships/{ship_id}
```

**Response**: AI ship details with behavior info

#### Create AI Ship

```
POST /v1/ai/ships
Content-Type: application/json
```

**Request Body**:
```json
{
  "name": "Romulan Warbird",
  "class": "cruiser",
  "faction": "romulan",
  "personality": "aggressive",
  "position": {"x": 10000.0, "y": 5000.0, "z": 2000.0}
}
```

**Personality Types**:
- `aggressive` - Engages enemies on sight
- `defensive` - Protects territory
- `cautious` - Avoids unnecessary conflict
- `neutral` - Minimal engagement unless provoked

**Response**: AI ship object (201 Created)

#### Delete AI Ship

```
DELETE /v1/ai/ships/{ship_id}
```

**Response**: 200 OK

#### Set Patrol Route

```
POST /v1/ai/ships/{ship_id}/patrol
Content-Type: application/json
```

**Request Body**:
```json
{
  "waypoints": [
    {"x": 1000.0, "y": 2000.0, "z": 500.0},
    {"x": 1500.0, "y": 2500.0, "z": 600.0}
  ]
}
```

**Response**: 200 OK

#### Add Hostile Faction

```
POST /v1/ai/ships/{ship_id}/hostile
Content-Type: application/json
```

**Request Body**:
```json
{
  "faction": "klingon"
}
```

**Response**: 200 OK

---

### Procedural Generation

Generate and query procedurally-generated universe content.

#### Generate Universe

```
POST /v1/generation/universe
Content-Type: application/json
```

**Request Body**:
```json
{
  "seed": 12345,
  "size": "medium",
  "faction_count": 5
}
```

**Sizes**: `small` (50 stars), `medium` (100 stars), `large` (200 stars)

**Response**: Universe generation summary (201 Created)

#### Get Universe

```
GET /v1/generation/universe
```

**Response**: Current universe state

#### Get Galaxy

```
GET /v1/generation/galaxy
```

**Response**: Galaxy structure with sectors

#### List Star Systems

```
GET /v1/generation/systems
```

**Response**: Array of star system summaries

#### Get Star System

```
GET /v1/generation/systems/{star_id}
```

**Response**: Detailed star system with planets, asteroids, etc.

```json
{
  "star": {
    "id": "star-uuid",
    "name": "Alpha Centauri",
    "type": "G2V",
    "mass": 1.1,
    "radius": 696000.0
  },
  "planets": [
    {
      "id": "planet-uuid",
      "name": "Proxima b",
      "type": "terrestrial",
      "radius": 6371.0,
      "mass": 5.972e24,
      "orbital_radius": 1.496e11,
      "habitable": true
    }
  ],
  "asteroids": [...],
  "stations": [...]
}
```

#### List Factions

```
GET /v1/generation/factions
```

**Response**: Array of procedurally-generated factions

#### Get Faction

```
GET /v1/generation/factions/{faction_id}
```

**Response**: Detailed faction information

```json
{
  "id": "faction-uuid",
  "name": "Xelari Collective",
  "government_type": "hive_mind",
  "economy_type": "planned",
  "military_strength": 8.5,
  "technology_level": 7.2,
  "territory": ["system-1", "system-2"],
  "relationships": {
    "faction-2": "hostile",
    "faction-3": "neutral"
  }
}
```

#### Get Faction Language

```
GET /v1/generation/languages/{faction_id}
```

**Response**: Language structure and phonemes

#### Translate Text

```
POST /v1/generation/languages/{faction_id}/translate
Content-Type: application/json
```

**Request Body**:
```json
{
  "text": "Hello, how are you?"
}
```

**Response**: Translated text in faction's language

#### Get Historical Events

```
GET /v1/generation/history
```

**Response**: Array of historical events across all factions

#### Get Faction History

```
GET /v1/generation/history/{faction_id}
```

**Response**: Historical events for specific faction

#### Get Timeline

```
GET /v1/generation/timeline
```

**Response**: Chronological timeline of universe events

---

## Ship Positions

Position-specific endpoints for ship control. All require an active ship and assigned crew member.

### Captain

Overall command and crew management.

#### Reassign Crew

```
POST /ships/{ship_id}/reassign
Content-Type: application/json
```

**Request Body**:
```json
{
  "player_id": "player-uuid",
  "new_position": "science"
}
```

**Response**: 200 OK

#### Add Log Entry

```
POST /ships/{ship_id}/log
Content-Type: application/json
```

**Request Body**:
```json
{
  "entry": "Encountered unknown vessel at sector 7",
  "classification": "encounter"
}
```

**Classifications**: `encounter`, `combat`, `discovery`, `malfunction`, `other`

**Response**: 200 OK

#### Get Ship Log

```
GET /ships/{ship_id}/log
```

**Response**: Array of log entries

```json
[
  {
    "timestamp": "2024-12-15T14:30:00Z",
    "entry": "Encountered unknown vessel at sector 7",
    "classification": "encounter",
    "author": "Captain Alice"
  }
]
```

---

### Helm

Navigation and movement control.

#### Set Thrust

```
POST /v1/ships/{ship_id}/helm/thrust
Content-Type: application/json
```

**Request Body**:
```json
{
  "thrust": 0.75
}
```

**Thrust Range**: 0.0 (idle) to 1.0 (maximum)

**Response**: 200 OK

#### Set Rotation

```
POST /v1/ships/{ship_id}/helm/rotate
Content-Type: application/json
```

**Request Body**:
```json
{
  "pitch": 0.1,
  "yaw": -0.2,
  "roll": 0.0
}
```

**Rotation Range**: -1.0 to 1.0 for each axis

**Response**: 200 OK

#### Full Stop

```
POST /v1/ships/{ship_id}/helm/stop
```

**Response**: 200 OK

**Note**: Engages reverse thrust to bring ship to a complete stop.

#### Engage Warp Drive

```
POST /v1/ships/{ship_id}/helm/warp
Content-Type: application/json
```

**Request Body**:
```json
{
  "warp_factor": 5.0,
  "heading": {"x": 0.0, "y": 0.0, "z": 1.0}
}
```

**Warp Factor Range**: 1.0 to 9.0

**Response**: 200 OK or 422 if FTL is blocked (Tachyon effect)

**Note**: Warp drive provides gradual FTL acceleration.

#### Engage Jump Drive

```
POST /v1/ships/{ship_id}/helm/jump
Content-Type: application/json
```

**Request Body**:
```json
{
  "destination": {"x": 10000.0, "y": 5000.0, "z": 2000.0}
}
```

**Response**: 200 OK or 422 if FTL is blocked (Tachyon effect)

**Note**: Jump drive provides instant teleportation.

#### Initiate Docking

```
POST /v1/ships/{ship_id}/helm/dock
Content-Type: application/json
```

**Request Body**:
```json
{
  "station_id": "station-uuid"
}
```

**Response**: 200 OK

#### Get Helm Status

```
GET /v1/ships/{ship_id}/helm/status
```

**Response**: Current helm configuration

```json
{
  "thrust": 0.75,
  "rotation_rate": {"pitch": 0.1, "yaw": -0.2, "roll": 0.0},
  "warp_active": false,
  "docking_mode": false
}
```

---

### Engineering

Power management, cooling, and repairs.

#### Allocate Power

```
PATCH /v1/ships/{ship_id}/power/allocate
Content-Type: application/json
```

**Request Body**:
```json
{
  "allocations": {
    "weapons": 40,
    "shields": 30,
    "engines": 20,
    "sensors": 10
  }
}
```

**Note**: Total must equal 100%

**Response**: 200 OK

#### Allocate Cooling

```
PATCH /v1/ships/{ship_id}/cooling/allocate
Content-Type: application/json
```

**Request Body**:
```json
{
  "allocations": {
    "reactor": 50,
    "weapons": 30,
    "engines": 20
  }
}
```

**Response**: 200 OK

#### Repair Module

```
POST /v1/ships/{ship_id}/repair
Content-Type: application/json
```

**Request Body**:
```json
{
  "module_id": "shield-generator-1",
  "crew_assigned": 3
}
```

**Response**: Repair estimate

```json
{
  "estimated_time": 120.0,
  "success_probability": 0.85
}
```

#### Get Ship Status

```
GET /v1/ships/{ship_id}/status
```

**Response**: Overall ship status

```json
{
  "hull_integrity": 85.0,
  "shield_strength": 60.0,
  "power_level": 90.0,
  "heat_level": 45.0,
  "damaged_modules": ["sensor-array-2"],
  "status_effects": ["ion_jam"]
}
```

#### Get Module Status

```
GET /v1/ships/{ship_id}/modules/status
```

**Response**: Detailed status of all modules

---

### Communications

Inter-ship communication and jamming.

#### Request Docking

```
POST /v1/ships/{ship_id}/dock-request
Content-Type: application/json
```

**Request Body**:
```json
{
  "station_id": "station-uuid"
}
```

**Response**: Docking clearance or denial

#### Undock

```
POST /v1/ships/{ship_id}/undock
```

**Response**: 200 OK

#### Hail Ship

```
POST /v1/ships/{ship_id}/hail
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "target-ship-uuid",
  "message": "This is the USS Enterprise. State your intentions."
}
```

**Response**: 200 OK or 422 if communications are jammed

#### Respond to Hail

```
POST /v1/ships/{ship_id}/respond
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "hailing-ship-uuid",
  "message": "We come in peace.",
  "tone": "friendly"
}
```

**Tones**: `friendly`, `neutral`, `hostile`, `distress`

**Response**: 200 OK

#### Jam Communications

```
POST /v1/ships/{ship_id}/jam
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "target-ship-uuid"
}
```

**Response**: 200 OK

**Note**: Requires Ion weapons to be equipped and enabled.

#### Command Fighters

```
POST /v1/ships/{ship_id}/fighters/command
Content-Type: application/json
```

**Request Body**:
```json
{
  "command": "launch",
  "target_id": "enemy-ship-uuid"
}
```

**Commands**: `launch`, `recall`, `attack`, `defend`, `patrol`

**Response**: 200 OK

---

### Science

Sensors, scanning, and tactical analysis.

#### Scan Target

```
POST /v1/ships/{ship_id}/scan
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "target-uuid",
  "scan_type": "detailed"
}
```

**Scan Types**: `quick`, `detailed`, `deep`

**Response**: Scan results

```json
{
  "target_id": "target-uuid",
  "type": "ship",
  "class": "cruiser",
  "hull_integrity": 75.0,
  "shield_strength": 50.0,
  "weapon_systems": ["phaser-array", "photon-torpedoes"],
  "threat_level": "moderate"
}
```

**Note**: Scan quality reduced if Ion jammed.

#### Get Contacts

```
GET /v1/ships/{ship_id}/contacts
```

**Response**: Array of nearby entities

```json
[
  {
    "id": "entity-uuid",
    "type": "ship",
    "distance": 5000.0,
    "bearing": {"azimuth": 45.0, "elevation": 10.0},
    "velocity": {"x": 10.0, "y": 0.0, "z": 5.0}
  }
]
```

#### Get Threats

```
GET /v1/ships/{ship_id}/threats
```

**Response**: Array of hostile entities

#### Get Navigation Data

```
GET /v1/ships/{ship_id}/navigation/{target_id}
```

**Response**: Navigation data to target

```json
{
  "target_id": "target-uuid",
  "distance": 10000.0,
  "heading": {"x": 0.707, "y": 0.0, "z": 0.707},
  "eta": 120.0,
  "intercept_course": {"pitch": 0.0, "yaw": 45.0}
}
```

#### Analyze Target

```
POST /v1/ships/{ship_id}/analyze
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "target-uuid",
  "analysis_type": "tactical"
}
```

**Analysis Types**: `tactical`, `technical`, `biological`

**Response**: Detailed analysis results

---

### Energy Weapons

Energy-based weapon systems (phasers, lasers, plasma).

#### Set Target

```
POST /v1/ships/{ship_id}/energy-weapons/target
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "enemy-ship-uuid"
}
```

**Response**: 200 OK

#### Fire Weapon

```
POST /v1/ships/{ship_id}/energy-weapons/fire
Content-Type: application/json
```

**Request Body**:
```json
{
  "weapon_id": "phaser-array-1"
}
```

**Response**: Fire result

```json
{
  "hit": true,
  "hull_damage": 45.0,
  "shield_damage": 120.0,
  "critical_hit": false,
  "effects": ["ion_jam"]
}
```

#### Toggle Auto-Fire

```
POST /v1/ships/{ship_id}/energy-weapons/auto
Content-Type: application/json
```

**Request Body**:
```json
{
  "enabled": true
}
```

**Response**: 200 OK

**Note**: Auto-fire engages when target in range.

#### Activate Radial Weapon

```
POST /v1/ships/{ship_id}/radial-weapons/activate
Content-Type: application/json
```

**Request Body**:
```json
{
  "weapon_id": "nova-cannon"
}
```

**Response**: 200 OK

**Note**: Radial weapons damage all nearby entities.

#### Get Weapon Status

```
GET /v1/ships/{ship_id}/energy-weapons/status
```

**Response**: Weapon system status

```json
{
  "weapons": [
    {
      "id": "phaser-array-1",
      "type": "phaser",
      "power": 100.0,
      "heat": 35.0,
      "ready": true,
      "auto_fire": false,
      "tags": ["ion", "plasma"]
    }
  ],
  "current_target": "enemy-ship-uuid"
}
```

---

### Kinetic Weapons

Projectile-based weapons (railguns, mass drivers).

#### Set Target

```
POST /v1/ships/{ship_id}/kinetic-weapons/target
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "enemy-ship-uuid"
}
```

**Response**: 200 OK

#### Configure Weapon

```
POST /v1/ships/{ship_id}/kinetic-weapons/{weapon_id}/configure
Content-Type: application/json
```

**Request Body**:
```json
{
  "kind": "armor-piercing",
  "velocity": 5000.0
}
```

**Kinds**: Defined in `data/modules.yaml` - `armor-piercing`, `explosive`, `scatter`, etc.

**Response**: 200 OK

#### Load Ammunition

```
POST /v1/ships/{ship_id}/kinetic-weapons/{weapon_id}/load
Content-Type: application/json
```

**Request Body**:
```json
{
  "ammo_type": "depleted-uranium"
}
```

**Response**: 200 OK

#### Fire Weapon

```
POST /v1/ships/{ship_id}/kinetic-weapons/{weapon_id}/fire
```

**Response**: Fire result with damage and hit status

#### Toggle Auto-Fire

```
POST /v1/ships/{ship_id}/kinetic-weapons/{weapon_id}/auto
Content-Type: application/json
```

**Request Body**:
```json
{
  "enabled": true
}
```

**Response**: 200 OK

#### Get Weapon Status

```
GET /v1/ships/{ship_id}/kinetic-weapons/status
```

**Response**: Kinetic weapon system status

---

### Missile Weapons

Guided missile systems (photon torpedoes, missiles).

#### Set Target

```
POST /v1/ships/{ship_id}/missile-weapons/target
Content-Type: application/json
```

**Request Body**:
```json
{
  "target_id": "enemy-ship-uuid"
}
```

**Response**: 200 OK

#### Load Ordnance

```
POST /v1/ships/{ship_id}/missile-weapons/{weapon_id}/load
Content-Type: application/json
```

**Request Body**:
```json
{
  "ordnance_type": "photon-torpedo"
}
```

**Response**: 200 OK

#### Fire Weapon

```
POST /v1/ships/{ship_id}/missile-weapons/{weapon_id}/fire
```

**Response**: Launch confirmation

**Note**: Missiles can be intercepted by countermeasures.

#### Toggle Auto-Fire

```
POST /v1/ships/{ship_id}/missile-weapons/{weapon_id}/auto
Content-Type: application/json
```

**Request Body**:
```json
{
  "enabled": true
}
```

**Response**: 200 OK

#### Get Weapon Status

```
GET /v1/ships/{ship_id}/missile-weapons/status
```

**Response**: Missile weapon system status

```json
{
  "weapons": [
    {
      "id": "torpedo-tube-1",
      "type": "photon-torpedo",
      "loaded": true,
      "ammo_count": 12,
      "ready": true,
      "lock_quality": 0.95
    }
  ]
}
```

---

### Countermeasures

Defensive systems (shields, point defense, countermeasures).

#### Raise Shields

```
POST /v1/ships/{ship_id}/shields/raise
```

**Response**: 200 OK

#### Lower Shields

```
POST /v1/ships/{ship_id}/shields/lower
```

**Response**: 200 OK

**Note**: Lowering shields increases sensor range and reduces power drain.

#### Get Shield Status

```
GET /v1/ships/{ship_id}/shields/status
```

**Response**: Shield status

```json
{
  "active": true,
  "strength": 750.0,
  "max_strength": 1000.0,
  "recharge_rate": 50.0,
  "coverage": {
    "forward": 90.0,
    "aft": 70.0,
    "port": 85.0,
    "starboard": 85.0
  }
}
```

#### Load Countermeasures

```
POST /v1/ships/{ship_id}/countermeasures/load
Content-Type: application/json
```

**Request Body**:
```json
{
  "type": "chaff",
  "count": 20
}
```

**Types**: `chaff`, `antimissile`, `decoy`

**Response**: 200 OK

#### Activate Countermeasures

```
POST /v1/ships/{ship_id}/countermeasures/activate
Content-Type: application/json
```

**Request Body**:
```json
{
  "type": "chaff"
}
```

**Response**: 200 OK

**Effects**:
- `chaff` - Confuses sensors and missiles
- `antimissile` - Destroys incoming missiles
- `decoy` - Creates false target

#### Toggle Point Defense

```
POST /v1/ships/{ship_id}/point-defense/toggle
Content-Type: application/json
```

**Request Body**:
```json
{
  "enabled": true
}
```

**Response**: 200 OK

**Note**: Point defense automatically engages incoming missiles.

---

## WebSocket API

Real-time game state updates via WebSocket connection.

### Connection

```
ws://localhost:8000/ws
```

### Message Format

All WebSocket messages use JSON format:

```json
{
  "type": "event_type",
  "data": { ... }
}
```

### Event Types

#### Ship Position Update

```json
{
  "type": "ship_position",
  "data": {
    "ship_id": "ship-uuid",
    "position": {"x": 1000.0, "y": 2000.0, "z": 500.0},
    "velocity": {"x": 10.0, "y": 0.0, "z": 5.0},
    "rotation": {"w": 1.0, "x": 0.0, "y": 0.0, "z": 0.0}
  }
}
```

#### Combat Event

```json
{
  "type": "combat",
  "data": {
    "attacker_id": "ship-1",
    "target_id": "ship-2",
    "weapon_type": "phaser",
    "damage": {
      "hull": 45.0,
      "shields": 120.0
    },
    "critical": false
  }
}
```

#### Ship Status Change

```json
{
  "type": "ship_status",
  "data": {
    "ship_id": "ship-uuid",
    "hull": 850.0,
    "shields": 600.0,
    "power": 750.0,
    "status_effects": ["ion_jam", "graviton_weight"]
  }
}
```

#### Communication Event

```json
{
  "type": "communication",
  "data": {
    "from_ship": "ship-1",
    "to_ship": "ship-2",
    "message": "We come in peace",
    "tone": "friendly"
  }
}
```

#### Docking Event

```json
{
  "type": "docking",
  "data": {
    "ship_id": "ship-uuid",
    "station_id": "station-uuid",
    "status": "docked"
  }
}
```

### Subscribing to Events

Send subscription request:

```json
{
  "type": "subscribe",
  "data": {
    "events": ["ship_position", "combat", "communication"],
    "filter": {
      "ship_id": "my-ship-uuid"
    }
  }
}
```

---

## GraphQL API

GraphQL endpoint for flexible querying.

### Endpoint

```
POST /graphql
Content-Type: application/json
```

### Example Query

```graphql
query {
  ship(id: "ship-uuid") {
    id
    name
    class
    position {
      x
      y
      z
    }
    hull
    shields
    modules {
      id
      type
      power
      health
    }
    crew {
      player_id
      position
      name
    }
  }
}
```

### Example Mutation

```graphql
mutation {
  fireWeapon(shipId: "ship-uuid", weaponId: "phaser-1") {
    success
    damage {
      hull
      shields
    }
    effects
  }
}
```

### Available Queries

- `player(id: ID!)` - Get player details
- `team(id: ID!)` - Get team details
- `ship(id: ID!)` - Get ship details
- `station(id: ID!)` - Get station details
- `blueprint(id: ID!)` - Get blueprint details
- `ships` - List all ships
- `players` - List all players
- `teams` - List all teams
- `stations` - List all stations

### Available Mutations

- `createPlayer(name: String!)` - Create new player
- `createTeam(name: String!, faction: String!)` - Create new team
- `fireWeapon(shipId: ID!, weaponId: ID!)` - Fire weapon
- `setThrust(shipId: ID!, thrust: Float!)` - Set ship thrust
- `raiseShields(shipId: ID!)` - Raise shields

---

## Error Codes

### HTTP Status Codes

| Code | Meaning | Description |
|------|---------|-------------|
| 200 | OK | Request succeeded |
| 201 | Created | Resource created successfully |
| 400 | Bad Request | Invalid request format or parameters |
| 404 | Not Found | Requested resource doesn't exist |
| 422 | Unprocessable Entity | Request valid but action cannot be performed |
| 500 | Internal Server Error | Server encountered an error |

### Game-Specific Errors

```json
{
  "error": "InsufficientPower",
  "message": "Cannot fire weapon: insufficient power allocated to weapons",
  "required_power": 100,
  "available_power": 75
}
```

Common game errors:
- `InsufficientPower` - Not enough power allocated
- `WeaponNotReady` - Weapon cooling down or damaged
- `InvalidTarget` - Target out of range or not targetable
- `CommunicationsJammed` - Ion effect blocking communications
- `FTLBlocked` - Tachyon effect preventing FTL
- `ShipNotDocked` - Action requires ship to be docked
- `ModuleDamaged` - Required module is damaged
- `CrewNotAssigned` - Position has no crew member

---

## Examples

### Complete Ship Creation Workflow

```bash
# 1. Create players
curl -X POST http://localhost:8000/v1/players \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice"}'

curl -X POST http://localhost:8000/v1/players \
  -H "Content-Type: application/json" \
  -d '{"name": "Bob"}'

# 2. Create team
curl -X POST http://localhost:8000/v1/teams \
  -H "Content-Type: application/json" \
  -d '{"name": "Alpha Squad", "faction": "federation"}'

# 3. Add players to team
curl -X PATCH http://localhost:8000/v1/teams/TEAM_ID \
  -H "Content-Type: application/json" \
  -d '{"player_id": "ALICE_ID"}'

# 4. Create blueprint
curl -X POST http://localhost:8000/v1/blueprints \
  -H "Content-Type: application/json" \
  -d '{"name": "USS Enterprise", "ship_class": "battleship", "faction": "federation"}'

# 5. Join blueprint
curl -X POST http://localhost:8000/v1/blueprints/BLUEPRINT_ID/join \
  -H "Content-Type: application/json" \
  -d '{"player_id": "ALICE_ID"}'

# 6. Assign roles
curl -X PATCH http://localhost:8000/v1/blueprints/BLUEPRINT_ID/roles \
  -H "Content-Type: application/json" \
  -d '{"assignments": {"ALICE_ID": "captain", "BOB_ID": "helm"}}'

# 7. Mark ready
curl -X POST http://localhost:8000/v1/blueprints/BLUEPRINT_ID/ready \
  -H "Content-Type: application/json" \
  -d '{"player_id": "ALICE_ID"}'

# 8. Compile ship
curl -X POST http://localhost:8000/v1/ships/compile \
  -H "Content-Type: application/json" \
  -d '{"blueprint_id": "BLUEPRINT_ID"}'
```

### Combat Scenario

```bash
# Set target
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/energy-weapons/target \
  -H "Content-Type: application/json" \
  -d '{"target_id": "ENEMY_ID"}'

# Raise shields
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/shields/raise

# Fire weapons
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/energy-weapons/fire \
  -H "Content-Type: application/json" \
  -d '{"weapon_id": "phaser-array-1"}'

# Activate countermeasures if under attack
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/countermeasures/activate \
  -H "Content-Type: application/json" \
  -d '{"type": "chaff"}'
```

### Navigation and Movement

```bash
# Set thrust
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/helm/thrust \
  -H "Content-Type: application/json" \
  -d '{"thrust": 0.8}'

# Rotate ship
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/helm/rotate \
  -H "Content-Type: application/json" \
  -d '{"pitch": 0.0, "yaw": 0.5, "roll": 0.0}'

# Engage warp
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/helm/warp \
  -H "Content-Type: application/json" \
  -d '{"warp_factor": 5.0, "heading": {"x": 0.0, "y": 0.0, "z": 1.0}}'

# Jump to coordinates
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/helm/jump \
  -H "Content-Type: application/json" \
  -d '{"destination": {"x": 10000.0, "y": 5000.0, "z": 2000.0}}'
```

### Station Docking

```bash
# Request docking
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/dock-request \
  -H "Content-Type: application/json" \
  -d '{"station_id": "STATION_ID"}'

# Navigate to docking bay (helm)
curl -X POST http://localhost:8000/v1/ships/SHIP_ID/helm/dock \
  -H "Content-Type: application/json" \
  -d '{"station_id": "STATION_ID"}'

# Complete docking
curl -X POST http://localhost:8000/v1/stations/STATION_ID/dock/SHIP_ID/complete

# Request services
curl -X POST http://localhost:8000/v1/stations/STATION_ID/services/SHIP_ID \
  -H "Content-Type: application/json" \
  -d '{"services": ["repair", "resupply"]}'

# Undock
curl -X POST http://localhost:8000/v1/stations/STATION_ID/undock/SHIP_ID
```

---

## Rate Limiting

**Current Status**: No rate limiting implemented

**Future Implementation**:
- Rate limits per player/team
- WebSocket message throttling
- API request quotas

## Versioning

API version is specified in the URL path: `/v1/`

Breaking changes will increment the version number. The current version (v1) will be supported during the v2 transition period.

## Support

For issues, questions, or contributions:
- GitHub: [HYPERION Repository]
- Documentation: `/doc/`
- Examples: `/examples/`

---

**Last Updated**: December 2024  
**API Version**: 1.0.0

---

## Ship Classes

### Overview

The Ship Classes API provides detailed information about available ship classes, including technical specifications, bonuses, and faction-specific manufacturers.

**Base Path**: `/ship-classes`

### List Ship Classes

Get a list of all available ship classes with optional faction filtering.

**Endpoint**: `GET /ship-classes`

**Query Parameters**:
- `faction` (optional): Filter by faction ID to show only ships with that faction's manufacturer

**Example Requests**:
```bash
# Get all ship classes
curl http://localhost:8000/ship-classes

# Get ship classes available to Terran Federation
curl http://localhost:8000/ship-classes?faction=terran-federation
```

**Response**:
```json
[
  {
    "id": "frigate",
    "name": "Frigate",
    "description": "A medium ship built to defend other ships in combat...",
    "size": "Medium",
    "role": "Defense",
    "max_weight": 270.0,
    "max_modules": 15,
    "build_points": 640.0
  }
]
```

**Response Fields**:
- `id`: Unique ship class identifier
- `name`: Display name
- `description`: Overview of the ship class
- `size`: Ship size category (Small, Medium, Large)
- `role`: Ship role category (Combat, Defense, Support, etc.)
- `max_weight`: Maximum weight in kg the ship can support
- `max_modules`: Maximum number of modules that can be equipped
- `build_points`: Build points required to construct this ship class

---

### Get Ship Class Details

Get comprehensive information about a specific ship class.

**Endpoint**: `GET /ship-classes/<id>`

**Path Parameters**:
- `id`: Ship class identifier (e.g., "frigate", "cruiser")

**Example Request**:
```bash
curl http://localhost:8000/ship-classes/frigate
```

**Response**:
```json
{
  "id": "frigate",
  "name": "Frigate",
  "description": "A medium ship built to defend other ships in combat...",
  "size": "Medium",
  "role": "Defense",
  "max_weight": 270.0,
  "max_modules": 15,
  "base_hull": 420.0,
  "base_shields": 0.0,
  "build_points": 640.0,
  
  "bonuses": {
    "defense": [
      {
        "id": "module_hp",
        "name": "Module Durability",
        "description": "Increases hit points of all modules",
        "value": 0.1,
        "formatted_value": "+10%",
        "applies_to": ["all_modules"]
      },
      {
        "id": "module_hp_defense",
        "name": "Defensive Module Durability",
        "description": "Increases hit points of defensive modules",
        "value": 0.25,
        "formatted_value": "+25%",
        "applies_to": ["defensive_modules"]
      }
    ]
  },
  
  "technical_specs": {
    "Length": "150.0 m",
    "Width": "45.0 m",
    "Height": "30.0 m",
    "Mass": "50000 tonnes",
    "Crew": "25-40",
    "Cargo": "500 m³",
    "Max Acceleration": "35.0 m/s²",
    "Turn Rate": "25.0°/s",
    "Max Warp": "5.0c",
    "Sensor Range": "25000 km",
    "Range": "15.0 AU"
  },
  
  "manufacturers": {
    "terran-federation": {
      "manufacturer": "United Shipyards",
      "variant": "Constitution-class",
      "lore": "The Terran Federation's frigates emphasize balanced performance..."
    },
    "mars-coalition": {
      "manufacturer": "Olympus Mons Defense Industries",
      "variant": "Ares-class",
      "lore": "Martian frigates are optimized for extended independent operations..."
    }
  },
  
  "lore": "The frigate represents centuries of naval engineering tradition...",
  "year_introduced": 2285,
  "notable_ships": [
    "TFS Valiant",
    "MSS Red Storm",
    "BAV Resilience"
  ]
}
```

**Response Fields**:

*Basic Information*:
- `id`, `name`, `description`, `size`, `role`: Same as list endpoint
- `max_weight`, `max_modules`, `build_points`: Build constraints
- `base_hull`: Base hull integrity points
- `base_shields`: Base shield capacity

*Bonuses* (grouped by category):
- Each bonus includes:
  - `id`: Bonus identifier
  - `name`: Human-readable bonus name
  - `description`: What the bonus does
  - `value`: Raw numeric value
  - `formatted_value`: Formatted for display (e.g., "+25%")
  - `applies_to`: What modules/systems this bonus affects

*Technical Specifications*:
- Dynamic key-value pairs with formatted values
- Common specs include: Length, Width, Height, Mass, Crew, Cargo, Max Acceleration, Turn Rate, Max Warp, Sensor Range, Range

*Manufacturers*:
- Faction-specific manufacturer information
- `manufacturer`: Company/organization name
- `variant`: Faction-specific variant name
- `lore`: Faction-specific design philosophy and history

*Lore and Flavor*:
- `lore`: Detailed background and historical information
- `year_introduced`: Year the ship class was first deployed
- `notable_ships`: Famous ships of this class

---

### Bonus Categories

Bonuses are automatically grouped into these categories:

- **combat**: Bonuses affecting weapons and offensive capabilities
- **defense**: Bonuses affecting shields, armor, and survivability
- **mobility**: Bonuses affecting movement and maneuverability
- **utility**: Bonuses affecting sensors, power, and ship systems
- **efficiency**: Bonuses affecting costs and operational efficiency

### Size Categories

- **Small**: Interceptors, scouts, escorts, corvettes
- **Medium**: Frigates, destroyers, cruisers
- **Large**: Battleships, dreadnoughts, carriers

### Role Categories

- **Versatile**: Multi-role ships adaptable to various missions
- **Combat**: Offensive-focused warships
- **Defense**: Defensive and escort vessels
- **Support**: Utility and support craft
- **Transport**: Cargo and personnel transport
- **Exploration**: Long-range exploration vessels
- **Offense**: Pure offensive platforms

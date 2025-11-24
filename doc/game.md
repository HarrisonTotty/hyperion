# HYPERION Game Design

## Overview

When the `hyperion` game server is started, a new simulated world generated and started. The exposed REST/GraphQL API can then be used to interact with objects in the game world.

### Player Registration

A player's client starts registering a new `Player` into the game. Each player has an associated _name_.

Relevant endpoints:
```
GET /v1/players
POST /v1/players
```

### Team Registration

From there, the client may choose to _create a new Team_ or _join an existing team_. Doing so will either create a new `Team` and add that `Player` to its list of `members`, or add the `Player` to the `members` of an existing `Team`.

Relevant endpoints:
```
GET /v1/factions
GET /v1/teams
POST /v1/teams
PATCH /v1/teams/<team>
```

Example of team creation data:
```json
{
    "name": "Red Team",
    "faction": "Alliance"
}
```

### Ship Blueprint Registration

Next, the client may choose to _create a new ship_ or _join an existing ship_. The former will create a `ShipBlueprint`, which represents a spaceship in the design phase.

Example POST data for ship creation:

```json
// POST /v1/blueprints
{
    "name": "Pillar of Autumn",
    "class": "dreadnaught",
    "team": "Red Team",
    "player": "Adam", // The creating player.
    "roles": ["captain", "helm"] // Role(s) requested by the creating player.
}
```

This will return a _Ship Identifier Code_, which is just a number that will be used to reference the ship in the API.

To join an existing ship (or update the desired roles):

```json
// POST /v1/blueprints/<id>/join
{
    "player": "Sarah",
    "roles": ["engineering"]
}
```

### Ready For Launch

Each player client will send over a "ready" indicator when they are satisfied with the current design of the ship. They may "unsend" this indicator at any time during the design phase if they forgot to make changes.

Once all players have indicated that they are ready and all required ship modules have been specified, the `ShipBlueprint` is compiled into a `Ship` instance and placed into the game world at a random position.

## Weapon Tags

Weapons in HYPERION have modifiers based on a simple _tag_ system. A weapon may have more than one _tag_.

### Beam

Weapon fires its projectiles in a continuous stream, dealing 1x weapon damage per second.

### Burst

Weapon fires its projectiles in bursts of 3 rounds, each dealing the weapon's damage.

### Decoy

Weapon contains empty warhead and false scan signal to trick enemy ships into wasting anti-missile countermeasures.

### Graviton

Weapon is graviton-based. Enemies hit with this weapon experience 30% additonal temporary effective weight (non-stacking).

### Ion

Weapon is ion-based. Enemies hit with this weapon have their communications and science stations jammed, and cannot lock-on to targets.

### Missile

Weapon is guided with a high velocity but small warhead.

### Photon

Weapon is photon-based and deals 1/2 damage to shields.

### Plasma

Weapon is plasma-based and deals 2x damage to shields.

### Positron

Weapon is positron-based and allows 25% of its damage to ignore shields.

### Pulse

Weapon fires its projectiles in bursts of 2 rounds, each dealing the weapon's damage.

### Single-Fire

Weapon fires one projectile at a time.

### Tachyon

Weapon is tachyon-based and enemies hit with this weapon cannot engage warp or jump.

### Torpedo

Weapon is unguided with a large warhead but relatively slow velocity.

### Antimissile

Missile targets other missiles or torpedos.

### Antitorpedo

Torpedo targets other missiles or torpedos.

### Chaff

Antitorpedo that doesn't detonate and jammes missiles within some distance for some time.

### Manual

Weapon must be manually fired at a targeted enemy when ready.

### Automatic

Weapon is capable of automatically firing on a targeted enemy when ready.

### Toggle

Weapon continues to deal damage to the target until toggled off.

## General Terms & Definitions

### Warp/Jump Drive

_Warp_ drives in HYPERION accelerate a ship to extreme FTL speeds. _Jump_ drives on the other hand instantly teleport a ship some distance away but at the cost of a longer startup time.


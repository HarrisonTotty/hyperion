# HYPERION Game Design

This document details HYPERION's game design, covering the player experience, core gameplay loop, and feature overview.

## Vision

HYPERION is a cooperative spaceship bridge simulation where players work together to operate a customizable starship. Inspired by Star Trek bridge operations, Artemis Spaceship Bridge Simulator, and the depth of Dwarf Fortress, HYPERION emphasizes teamwork, realistic ship operations, and emergent gameplay in a procedurally generated galaxy.

The game is intentionally complex. Each bridge station presents players with detailed displays, technical readouts, and systems that require learning and mastery. This complexity creates immersion—players should feel like actual crew members operating sophisticated spacecraft.

### Core Design Decisions

| Aspect | Choice | Rationale |
|--------|--------|-----------|
| **Session length** | Long (3+ hours) | Deep immersion, meaningful journeys across star systems |
| **Death penalty** | Permadeath | High stakes make every decision matter |
| **Mission structure** | Sandbox/emergent | Players create their own narratives |
| **AI crew** | None | Coordination is mandatory; empty stations require multitasking |
| **Drop-in/out** | Hot-swap roles | Players can join/leave mid-flight seamlessly |
| **Faction reputation** | Central mechanic | Gates access to stations, trade, and safe passage |
| **Carrier gameplay** | Full support | Large ships deploy AI fighters controlled by Communications |
| **Universe persistence** | Fresh each game | Each session generates a new galaxy from seed |

## The Player Experience

### Getting Started

1. **Team Formation** — Players form a team with a shared credit balance
2. **Ship Design** — The team creates a ship blueprint by selecting:
   - A ship class (Scout, Cruiser, Destroyer, Battleship, etc.)
   - Module loadout (power cores, engines, weapons, shields, sensors)
   - Ship inventory (ammunition)
   - Role assignments for each player
3. **Launch** — Once all players are ready, the blueprint is compiled into an active ship

### Bridge Stations

Each player operates one or more bridge positions. Ships support 1-9 crew members, with larger ships requiring more specialization.

| Station | Primary Responsibilities |
|---------|-------------------------|
| **Captain** | Command authority, crew reassignment, mission objectives, captain's log |
| **Helm** | Ship piloting, navigation waypoints, docking procedures, FTL drives |
| **Engineering** | Power/cooling allocation, damage control, module repairs, system efficiency |
| **Science** | Long-range scanning, contact analysis, threat assessment, navigation support |
| **Communications** | Ship-to-ship hailing, docking requests, distress signals, fighter commands |
| **Countermeasures** | Shield management, point defense, chaff/decoys, anti-missile systems |
| **Energy Weapons** | Directed-energy targeting (lasers, phasers, particle beams) |
| **Kinetic Weapons** | Ballistic weapons (railguns, cannons, coilguns), ammunition management |
| **Missile Weapons** | Guided munitions (missiles, torpedoes), lock-on targeting |

Players can hold multiple roles on smaller ships. For example, a 2-player crew might have:
- Player 1: Captain, Helm, Engineering
- Player 2: Science, Communications, all Weapons

### The Bridge Interface

Each station presents a specialized interface:

- **Data-dense displays** — Status readouts, system diagrams, tactical overlays
- **Realistic controls** — Power sliders, targeting systems, navigation inputs
- **Real-time updates** — All stations receive live updates via WebSocket
- **Inter-station communication** — Chat, alerts, and shared tactical information

The interfaces deliberately avoid gamification—no health bars or simplified indicators. Instead, players read actual values: shield capacity in megajoules, power draw in kilowatts, distances in kilometers.

## Core Gameplay Loop

```
┌─────────────────────────────────────────────────────────────────────┐
│                         PREPARATION                                 │
│  • Design ship blueprint                                           │
│  • Assign crew roles                                               │
│  • Purchase and outfit ship                                        │
└────────────────────────────┬────────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          DEPLOYMENT                                 │
│  • Launch from station                                             │
│  • Set mission objectives                                          │
│  • Plot initial course                                             │
└────────────────────────────┬────────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          OPERATIONS                                 │
│  • Navigate the galaxy                                             │
│  • Detect and analyze contacts                                     │
│  • Engage hostiles or diplomatic encounters                        │
│  • Manage ship systems under stress                                │
│  • Complete objectives                                             │
└────────────────────────────┬────────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                           RESUPPLY                                  │
│  • Dock at friendly stations                                       │
│  • Repair damaged modules                                          │
│  • Rearm weapons                                                   │
│  • Trade cargo                                                     │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             └──────────────► (Return to Operations)
```

### Moment-to-Moment Gameplay

A typical engagement might unfold like this:

1. **Science** detects unknown contacts at long range
2. **Helm** adjusts course for intercept or evasion
3. **Science** identifies contacts as hostile patrol ships
4. **Captain** decides to engage; issues combat alert
5. **Engineering** reroutes power to weapons and shields
6. **Countermeasures** raises shields
7. **Communications** attempts to hail (no response)
8. **Energy Weapons** locks on lead target
9. **Helm** maneuvers for optimal firing solution
10. **All weapons stations** fire in coordinated volley
11. **Countermeasures** deploys decoys against incoming missiles
12. **Engineering** manages heat buildup and repairs damage
13. **Science** tracks enemy reinforcements

This creates emergent storytelling through coordinated action.

## Ship Systems

### Power Management

Ships generate power from reactors and store it in capacitors. Every module draws power:

| System | Typical Draw |
|--------|--------------|
| Life Support | Always on (minimal) |
| Sensors | Continuous |
| Shields | High when raised |
| Weapons | Spike during firing |
| FTL Drives | Massive during use |

Engineering allocates power priority. When generation exceeds demand, excess charges capacitors. When demand exceeds generation, modules operate at reduced efficiency or shut down.

### Thermal Management

Weapons and reactors generate heat. Cooling systems dissipate heat through radiators. If heat exceeds capacity:

- Module efficiency degrades
- Components take damage
- Critical overheating can destroy modules

Players must balance sustained fire with cooling cycles.

### Module Health

Each module tracks health independently. Damage comes from:
- Enemy weapons fire
- Internal system failures
- Overheating
- Collision damage

Damaged modules operate at reduced efficiency. Destroyed modules go offline entirely. Engineering can repair modules, but repairs take time and the ship remains vulnerable.

### Status Effects

Special weapons apply debilitating effects:

| Effect | Impact | Source |
|--------|--------|--------|
| **Ion** | Jams communications and sensors, disables targeting | Ion weapons |
| **Graviton** | Increases effective mass by 30%, slowing maneuvers | Graviton projectors |
| **Tachyon** | Disables warp and jump drives | Tachyon pulse emitters |

Effects have duration and can be stacked from multiple sources.

## Ship Classes

Ships range from small scouts to massive dreadnoughts:

| Class | Size | Crew | Role |
|-------|------|------|------|
| Scout | Tiny | 1-2 | Reconnaissance |
| Interceptor | Tiny | 1-2 | Fast attack |
| Corvette | Small | 2-4 | Patrol |
| Courier | Small | 1-3 | Fast transport |
| Cruiser | Small | 2-4 | Versatile |
| Escort | Small | 2-4 | Fleet support |
| Frigate | Medium | 3-5 | Combat |
| Destroyer | Medium | 4-6 | Heavy combat |
| Transport | Medium | 2-4 | Cargo hauling |
| Freighter | Large | 3-5 | Bulk cargo |
| Tanker | Large | 2-4 | Fuel transport |
| Defender | Large | 5-7 | System defense |
| Warship | Large | 5-8 | Fleet combat |
| Battleship | Large | 6-9 | Line combat |
| Dreadnought | Huge | 8-12 | Capital ship |
| Hyperion | Huge | 6-9 | Advanced exploration |

Each class defines:
- Base hull and shield capacity
- Maximum weight (limits module loadout)
- Maximum module count
- Build points for module installation
- Credit cost
- Performance characteristics (acceleration, turn rate, sensor range)
- Faction-specific variants with unique lore

## Module Categories

### Power & Propulsion
- **Power Cores** — Reactors and batteries (fusion, fission, plasma, antimatter)
- **Impulse Engines** — Sublight propulsion (ion, plasma, neutrino)
- **Maneuvering Thrusters** — Attitude control and fine maneuvering
- **Warp Cores** — FTL drives (warp sustained travel, jump instant teleport)

### Weapons
- **Directed Energy** — Lasers, phasers, particle beams (no ammo, heat-limited)
- **Kinetic** — Railguns, cannons, coilguns (require ammunition)
- **Missile Launchers** — Guided missiles and torpedoes (limited payload)

### Defense
- **Shield Generators** — Absorb incoming damage
- **Countermeasures** — Chaff, decoys, flak (defeat missiles)
- **Stealth Systems** — Reduce detection signature

### Support
- **Sensor Arrays** — Detection range and resolution
- **Communications** — Hailing and coordination
- **Cooling Systems** — Heat dissipation
- **Auxiliary Systems** — Emergency power, repair nanobots, boosters

### Special
- **Radial Emission Systems** — EMP pulses, sensor jammers, tachyon emitters

## The Galaxy

### Procedural Generation

Each game generates a unique galaxy with:

- **Star systems** — Stars, planets, asteroid belts, stations
- **Factions** — AI civilizations with governments, traits, relationships
- **Alien languages** — Procedural phonology and vocabulary
- **History** — Past wars, alliances, and events between factions

The generation uses seeds for reproducibility—share a seed to play the same galaxy.

### Fresh Universe Each Session

The universe does not persist between sessions:

- Each game starts with a new procedurally generated galaxy
- No carryover of credits, ships, or reputation
- Players begin fresh at a neutral station with starting credits
- The seed can be shared to replay the same galaxy with different choices

This design choice emphasizes:
- Each session is a complete, self-contained voyage
- No grinding or long-term progression requirements
- New players start on equal footing with veterans
- Every session feels like a fresh adventure

### Stations

Space stations provide services to docked ships:

| Service | Function |
|---------|----------|
| **Repair** | Restore module health |
| **Refuel** | Replenish power reserves |
| **Rearm** | Reload ammunition |
| **Trade** | Buy/sell cargo |

Stations belong to factions. Hostile factions deny docking requests.

### Faction Reputation

Reputation is a central mechanic that shapes the entire play experience:

**How Reputation Works**
- Each faction tracks standing with the player's team (-100 to +100)
- Actions shift reputation: combat, trade, completing requests, helping/harming allies
- Reputation spreads—attacking a faction's ally damages standing with both

**Reputation Effects**

| Standing | Access Level |
|----------|--------------|
| Allied (+75 to +100) | Full station access, best trade prices, military contracts |
| Friendly (+25 to +74) | Station access, favorable trade, information sharing |
| Neutral (-24 to +24) | Basic station access, standard prices |
| Unfriendly (-25 to -74) | Denied docking, may be warned to leave territory |
| Hostile (-75 to -100) | Attacked on sight, bounty placed on ship |

**Strategic Implications**
- Players must choose allegiances carefully
- Crossing hostile territory requires stealth or overwhelming force
- Reputation recovery is slow—burning bridges has lasting consequences
- Different factions control different regions, resources, and technology

### Navigation

Ships navigate via:
- **Sublight** — Normal space travel within systems
- **Warp** — Sustained faster-than-light travel between nearby systems
- **Jump** — Instantaneous teleportation to distant locations (high energy cost)

Tachyon fields can disable FTL, trapping ships in combat zones.

## Combat

### Weapon Types

**Directed Energy**
- Instant hit (light speed)
- No ammunition
- Limited by power and heat
- Best vs shields

**Kinetic**
- Projectile travel time
- Requires ammunition
- Penetrates shields partially
- Best vs hull

**Missiles**
- Guided, can be intercepted
- High damage per hit
- Limited payload
- Can carry special warheads

### Defensive Systems

**Shields**
- Absorb damage while raised
- Drain power continuously
- Must be lowered to dock
- Regenerate when not taking fire

**Point Defense**
- Automated interception of missiles
- Effectiveness depends on tracking speed and coverage

**Countermeasures**
- Chaff clouds confuse missile seekers
- Decoys draw fire away from the ship
- Flak torpedoes create defensive zones

### Damage Model

Incoming damage is processed:

1. Shields absorb damage (if raised)
2. Remaining damage hits hull
3. Critical hits may damage specific modules
4. Hull breach at 0% destroys the ship

Module damage cascades—a destroyed reactor cuts power, disabling dependent systems.

### Permadeath

Ship destruction is permanent:

- Hull breach at 0% ends the ship
- All modules, cargo, and ammunition are lost
- Team credits remain (stored at faction banks)
- The crew must design and build a new ship to continue

This creates meaningful tension in every encounter. Retreat is a valid tactical option. Reckless aggression has permanent consequences.

### Carrier Operations

Large ships equipped with fighter bays can deploy AI-controlled fighters:

**Fighter Commands** (issued by Communications)
- **Launch** — Deploy fighters from hangar
- **Recall** — Return fighters to ship
- **Attack** — Engage specified target
- **Defend** — Protect specified ship
- **Patrol** — Fly waypoint pattern

**Fighter Characteristics**
- AI-piloted, no player control of individual fighters
- Vulnerable to point defense and flak
- Effective for overwhelming enemy defenses
- Must return to carrier for rearming and repairs
- Carrier destruction strands deployed fighters

## Economy

### Credits

Teams share a credit pool used for:
- Purchasing ships (blueprint compilation)
- Buying modules and upgrades
- Station services (repair, rearm, refuel)
- Trade transactions

Ships can be sold back (scrapped) for full credit refund.

### Cost Factors

Ship cost = Base class cost + Module slot costs + Variant costs

Heavier modules cost more. Advanced variants of basic modules command premium prices.

## Multiplayer

### Local Play

Multiple players connect from the same network, each controlling their station on separate devices (tablets, laptops, phones).

### Online Play

Players connect to a hosted server over the internet. The server maintains authoritative game state; clients receive updates via WebSocket.

### Asymmetric Roles

Not all stations require equal skill or attention. New players can start with simpler roles (Communications) while experienced players handle complex stations (Engineering, Weapons).

### Hot-Swap Crew Changes

Players can join or leave mid-session without interrupting gameplay:

- New players connect and claim available stations
- Departing players release their stations for others
- The Captain can reassign roles at any time
- No AI fills empty stations—the crew must adapt or accept reduced capabilities

This enables long sessions where real-life interruptions don't end the game.

### No AI Crew

Empty bridge stations remain unmanned. This is intentional:

- Solo play requires rapid station-switching
- Small crews must prioritize which systems to actively manage
- Coordination between human players is the core experience
- Larger ships genuinely require larger crews to operate effectively

## Extensibility

### BYOF (Bring Your Own Frontend)

The HYPERION server exposes a REST/GraphQL API. Anyone can build custom frontends:
- Web interfaces
- Desktop applications
- Mobile apps
- VR experiences
- Physical bridge setups with hardware controls

### Modding

All game data loads from YAML files at runtime:
- Ship classes
- Module definitions
- Weapon statistics
- AI behaviors
- Universe generation parameters

Players can create custom scenarios, ship configurations, and balance changes without modifying code.

## Design Principles

1. **Teamwork over individual heroics** — Success requires coordination
2. **Complexity creates depth** — Systems interact in meaningful ways
3. **Information, not abstraction** — Real values, not simplified indicators
4. **Emergent narrative** — Stories arise from gameplay, not scripts
5. **Accessible entry, deep mastery** — Easy to start, rewarding to master
6. **Modularity** — Every system can be customized or replaced

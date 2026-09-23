# Single-Player Experience

Brainstorm for HYPERION's first playable experience: one player flying one small ship through the
generated galaxy, in the manner of Elite Dangerous, before the multi-position bridge exists. This is
a design exploration, not a plan. Decisions are marked **Lean** where there is a recommendation,
and the open ones are collected under [Open questions](#open-questions). What has been settled is
under [Decisions](#decisions), from two rounds with the project owner.

It builds on [the galaxy generation brainstorm](galaxy-generation.md) and assumes all of its plans
are built. Every system, star, body and event is a pure function of seed, ID and time. The range
query and the `GALAXY` and `SYSTEM` displays exist, and sensors see the past through a minimal
Knowledge overlay.

## Goal and scope

A player opens a universe and is given a small ship somewhere in it. They can fly it among bodies
that really orbit, go anywhere in a system including close to its star, jump between systems, find
out what is there, and keep the ship and themselves alive while doing it. One client, one seat, and
every system of the ship within reach.

Building this before the bridge has two purposes. It tests movement and the ship's core systems
with one person in the loop, where a flight model that feels wrong is found quickly, before a crew
divides the controls among themselves. And what it produces is kept: the same craft definitions fly
later as fighters and other small craft beside the main ship.

In scope:

- Sessions: a universe, a craft, a player, and the persistence of play.
- The simulation loop: a server-owned clock that advances, and time compression.
- The craft model: a data-driven definition of hull and modules, shared by every class of craft.
- Flight: rigid-body dynamics in six degrees of freedom under real gravity, flight control laws,
  engines and thrusters, the navigation computer and the autopilot.
- Jump drives as a family of types, and the first of them.
- Ship systems: power, heat, propellant, life support, damage and failures.
- Sensors and exploration, over the Knowledge overlay.
- The hazards the galaxy already generates: starlight and heat, flares, dust and gas at speed,
  tides, collisions.
- The cockpit: the single-seat console, its displays, controls and input devices, and a wireframe
  view of the surroundings.
- The choice of a 3D rendering engine.

Out of scope, each with hooks defined here:

- The multi-position bridge. The ship's systems are divided so that the bridge is a regrouping of
  the same commands and displays; see [The seat and the bridge](#the-seat-and-the-bridge).
- Full 3D planets and landing, which come in a later phase in the manner of No Man's Sky. Nothing
  here may rule them out; see [Planets](#planets).
- Other actors: civilisations, stations, other ships, trade, communications and combat. The module
  and damage models leave room for weapons without designing them.
- LLM enrichment.
- Several players in one universe. The session design must not rule it out.

## What to take from the references

| Reference             | Take                                                                                                                                                                                                                                                                                                                                                                                                                                       | Leave                                                                                                                                                                                                                            |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No Man's Sky          | Seamless flight through a system and down to a planet's surface, with no loading screens. The whole of a system is open to the player, its star included. Everything is generated from the seed on arrival.                                                                                                                                                                                                                                | Systems and planets far smaller than real ones. HYPERION keeps real scale, so crossing a system takes a jump, or a drive type of its own (an open question).                                                                     |
| Elite Dangerous       | One pilot runs a whole ship. Modules with power and heat budgets, and heat as a resource to manage. Jump range that depends on the ship's mass, so loadout is a navigation decision. Scooping fuel as a survival loop. Exploration as discover, scan and map, with a log. Flight assist, which decides whether a keyboard can fly the ship at all. Fighters launched from a larger ship. Landings on real-scale planets, since _Horizons_. | A speed cap on a Newtonian ship. Supercruise as a second, separate invented drive; continuous flight within a system, if it comes, is a type of jump drive (an open question). Fighters flown by telepresence at any range.      |
| Orbiter               | Honest Newtonian flight among real orbits. Multi-function displays as the interface to orbital mechanics: transfers, plane changes, synchronised orbits. Time acceleration up to 100,000×, which makes real distances playable.                                                                                                                                                                                                            | Shallow systems: no power, heat or life support without add-ons. A rendering-first presentation.                                                                                                                                 |
| Kerbal Space Program  | Manoeuvre nodes: a planned burn that the player edits against a predicted trajectory. Delta-v budgets as readouts. Time warp that drops to real time when something needs attention.                                                                                                                                                                                                                                                       | Patched conics and their jumps at sphere-of-influence boundaries. A solar system at a tenth of real scale.                                                                                                                       |
| DCS World, Falcon BMS | Depth of systems: failure modes, checklists, start-up procedures, full HOTAS binding. A cockpit that rewards knowing the aircraft.                                                                                                                                                                                                                                                                                                         | A steep entry. Depth must be available, never required on the first flight.                                                                                                                                                      |
| Artemis, EmptyEpsilon | Artemis 2.4 added a fighter pilot position: a player flies a single-seat craft launched from the crew's ship. EmptyEpsilon's Single Pilot station combines Helm, Weapons and short-range scanning into one console, so one person can fly a ship built for a crew. That regrouping is the model for this whole brainstorm.                                                                                                                 | Arcade flight models.                                                                                                                                                                                                            |
| _The Expanse_         | High-g flight as a limit on the crew, not the ship. Flip-and-burn. A ship that shows up in infrared because it has to shed heat.                                                                                                                                                                                                                                                                                                           | The Epstein drive's free lunch: gees of thrust at an exhaust velocity that implies terawatts, with no waste heat to show for it. If HYPERION invents a drive, it says so; see [Travel within a system](#travel-within-a-system). |
| Traveller             | For a later drive type, the 100-diameter limit: a jump drive that cannot work deep in a gravity well, so every arrival and departure has a sublight leg.                                                                                                                                                                                                                                                                                   | A rule with no reason behind it. [Later drive types](#for-later-drive-types) derive one.                                                                                                                                         |

The common lesson: **a single seat is the whole ship at one console.** Design the ship as though a
crew will fly it, then give all of its stations to one person.

## The seat and the bridge

The single seat is not a separate game mode. It is the bridge with one person holding every
station.

- A ship is a set of systems. Each publishes telemetry, accepts commands, and belongs to a station
  in the ship-wide nomenclature: the reactor to Engineering, the flight control system to Helm.
- A station is a set of displays and the authority to command a set of systems. The single seat
  holds all the authority. Every command carries the station it came from, and the ship arbitrates
  control, which the UX guide already requires every system to show (`AUTO`, `MAN`, and who is in
  control).
- The bridge is then a redivision of commands and displays that already exist, not new ones. A
  fighter is the same seat in a smaller hull.
- Displays are the unit of reuse. The single seat's displays are the stations' displays, plus at
  most one primary display that gathers what a pilot needs at a glance, as EmptyEpsilon's Single
  Pilot screen does.
- **Lean:** one client can open several windows, each hosting one display, so a player can put the
  view on one monitor and the navigation display on another, as home-cockpit builders do with
  multi-function displays. The same mechanism later puts a station, or the bridge's main screen, on
  another machine.

## Sessions and the loop

### Sessions

A session is a universe, the craft in it and the players flying them. Single-player is one craft
and one player. The server holds the session and is its only authority.

**Lean:** single-player runs the same server, locally. The Electron main process starts
`hyperion-server` on the loopback interface as a child process and connects to it, and connecting
to a remote server stays possible. One code path means nothing written for one player has to be
rewritten for several.

A session's state is play state, not generated data: the craft, the clock, the Knowledge gathered
and the deltas that play has made. The galaxy brainstorm's overlays already anticipate it.
**Lean:** it is stored in the universe's directory under the names plan 04 reserved there:
`session.json` for the clock and the craft, `knowledge/`, which plan 12 already writes, and
`deltas/`. A universe therefore holds one session. A second playthrough of the same seed is a second
universe, as plan 04 already provides, since a universe is a save with an ID of its own. The session
is saved on a timer and on request.

### The clock

The session holds the universe's "now". **Lean:** a session starts at the epoch, UT 0, and time
only moves forward. Mission elapsed time, `MET`, counts from the session's start, in the format
the UX guide already sets.

**Lean: the simulation steps at 64 Hz.** `UniverseTime` counts whole nanoseconds, and 1/64 s is
exactly 15,625,000 ns, so every tick is exact and the clock never drifts. 1/60 s is not a whole
number of nanoseconds.

**Time compression** lets real orbits and real engines be played: waiting for a periapsis, or a
cruise burn of days (see [Propulsion and distance](#propulsion-and-distance)). The rates are powers
of ten from 1× to 100,000×, the ceiling of both Orbiter and KSP. Each tick then advances
10ᵏ × 15.625 ms, which is still exact. **Lean**, for the rules:

- The flight computer drops the rate on its own when the pilot needs to act: an alert, a change of
  frame, the start of a planned burn, closing within a set distance of another object or the top of
  an atmosphere, and arming the jump drive, which drops it to 1×.
- Above 10×, manual inputs are ignored and attitude is held by the flight computer. Burns under
  compression are the flight computer's, flown to a plan.
- Compressing time never saves anything but the player's time. A 30-day transfer at 100,000× lasts
  26 seconds and consumes 30 days of air, water and food; see [The pilot](#the-pilot).

The galaxy's clock window is H = 1,000 years either side of the epoch. At 100,000× the window
forward takes 3.7 days of continuous play, so a session will not reach it in practice, but the
server enforces it.

**Lean:** pausing is a rate of zero, available in single-player only. The rate sits beside `UT` in
the header strip. The UX guide asks for a persistent banner in simulation, training and replay
modes, and a pause should carry one too, which needs an edit to the guide.

A session has one clock. When fighters share a session with the main ship, compression becomes an
agreement across the session. That is out of scope here, and noted so that nothing assumes a clock
per craft.

### Determinism of play

The galaxy is deterministic by construction. Play need not be, but it should be. **Lean:** the
flight and systems models are pure functions in `hyperion-sim`, from state, inputs and a step to
the next state, computed through its `math` module. A session then replays bit for bit from its
start state and a log of its inputs. That gives replays, golden tests of the flight model on the
three architectures CI already checks, and a path to client-side prediction later. Randomness in
play, such as failures, draws from a session seed through the same `Stream` machinery, under
domain tags of its own.

The server owns the loop: one task per session drives the ticks, and generation queries run on the
existing CPU pool with the existing caches. The loop never waits on generation. What the craft will
need next, such as the bodies of a destination system or the cells ahead of it, is requested in
advance, for a jump when the drive is armed.

A local server makes latency negligible. A remote one, for a bridge on a network, will want
client-side prediction and interpolation of the craft, which determinism keeps possible and which
is deferred. The view interpolates between telemetry updates even locally, since frames and ticks
do not line up and many monitors refresh faster than 64 Hz.

## Where the ship is

The galaxy brainstorm's frames carry the ship: galactic (integer light-year cell plus a metre
offset), system (metres from the barycentre) and body (metres from the body's centre). A ship is in
exactly one frame, and the rule for choosing among overlapping systems (the smallest distance ÷
tidal radius, with hysteresis) is already set.

- **Body frames.** The galaxy brainstorm says a ship enters a body's frame on entering its sphere of
  influence, without choosing the sphere. Frames here serve numerical precision, not a patched-conic
  approximation (see [Gravity](#gravity)). **Lean:** the body's Hill sphere, and the smallest one
  that contains the ship. A body's frame moves on an analytic orbit, so the frame's acceleration is
  known exactly and enters the equations as the indirect term.
- **Body-fixed frames**, rotating with the body, are what flight near a surface needs. The planetary
  stage already provides a body's rotation as a function of time. Nothing in this phase flies in
  one, but [Planets](#planets) will.
- **Between the stars** the galaxy model moves systems in straight lines, so a ship there does the
  same and nothing drifts relative to anything else. Within the central black hole's sphere of
  influence the ship feels the black hole and the enclosed mass, as the systems there do.
- A position is always reported with its frame, as the UX guide requires of spatial displays.

## Flight dynamics

### The body

The craft is a rigid body in six degrees of freedom. Its mass, centre of mass and inertia change
as propellant drains, which shifts the thrust line and makes the flight control system trim. Every
engine and thruster sits at a position on the hull with a direction, so a failed thruster quad
produces exactly the torques and translations it would.

### Gravity

Two approaches:

- **Patched conics**, as KSP: one body at a time, switched at sphere-of-influence boundaries. Cheap,
  but it has no Lagrange points, no captures, no third-body perturbations, and trajectories that
  jump at every boundary.
- **Restricted n-body**, as Orbiter: the ship feels every significant body, and the bodies ignore
  the ship.

**Lean: restricted n-body.** Bodies are on rails, so each gravity term is one evaluation of a Kepler
orbit the planetary stage already provides. The ship feels its host star or stars and every body
whose pull at its position exceeds a small threshold. Low orbits also need the dominant body's
oblateness, J₂, which the planetary stage does not provide yet. It follows from the rotation and the
interior, and is a hook to add. The
navigation computer still shows osculating elements about a chosen body for planning, as Orbiter's
displays do.

For now atmospheres matter only at the top: for aerobraking and for skimming gas giants (see
[Refuelling](#refuelling)). Drag and heating need the density against height, which follows from
quantities the planetary stage already has: the scale height is kT ÷ (μ m_u g). For Jupiter's 165 K
at 1 bar, mean molecular mass 2.22 and effective equatorial gravity 23.12 m/s² it gives about 27 km,
as NASA's fact sheet does. The gravity must be the effective one at the skimming latitude, rotation
included. The model should extend downward to the surface, with lift and aerodynamic control, when
[Planets](#planets) needs it.

### Integration

Technical choices, stated for review rather than for a ruling:

- **Coasting** uses Encke's method: the osculating Kepler orbit about the frame's body, propagated
  exactly, plus integrated perturbations. It is exact for a two-body coast and cheap at 100,000×.
- **Under thrust or strong perturbation**, a fixed-substep high-order integrator, with the number of
  substeps per tick set by the local dynamical time. Step selection is a function of the state
  computed through `math`, so it stays deterministic.
- **Attitude** is a quaternion under Euler's equations, on a faster substep. Rotation about the
  intermediate axis is unstable, as it is in reality, and a test says so. With energy dissipated by
  propellant slosh or flexing radiators, only the axis of greatest inertia is stable, as Explorer 1
  showed. Whether slosh is modelled is left to the plan.

### Flight control

The pilot flies through a fly-by-wire flight control system, whose modes are always shown:

- **Rotation.** `DIRECT` fires thrusters in proportion to the stick. `RATE` commands an angular rate
  and holds attitude when the stick is centred; it is the default. Attitude holds point the ship
  `PROGRADE`, `RETROGRADE`, `NORMAL`, `ANTINORMAL`, `RADIAL OUT`, `RADIAL IN`, at a `TARGET` or away
  from it, or at a fixed inertial attitude.
- **Translation.** `DIRECT`, or `VEL`, in which the stick commands a change of velocity against a
  stated reference and a centred stick holds it. The references are the frame body's orbit, where
  holding means coasting; the frame body itself, where holding means thrusting against gravity, as a
  hover over a surface will; and `TARGET`, for station-keeping.
- Real spacecraft autopilots offer the same choices: rate command with attitude hold, direct
  control, and minimum-impulse pulses. Elite's flight assist is `RATE` with `VEL` against the frame
  body, and it is what lets a keyboard fly a ship.

There is no speed limit. Speed is relative to a frame and says little on its own, so the flight
display shows velocity against the chosen reference and the delta-v remaining.

The autopilot flies planned burns (attitude, ignition at the planned time, cutoff on delta-v
achieved), orbit insertion, and rendezvous and approach to a target. Main-engine ignition by hand
is a hazardous command, `ARM` then `EXECUTE`. Thrusters are not.

### The pilot

The pilot is a person in a seat.

- **Acceleration.** Tolerance depends on the axis and the duration. It is far higher chest to back
  (+Gx) than head to foot (+Gz), which is why launch couches recline. NASA-STD-3001 Volume 2 sets
  sustained limits by axis and duration, and a fighter pilot in a g-suit, straining, holds 9 g in
  +Gz for about 15 seconds. **Lean:** model the load on the body's axes, accumulated over time
  against the standard's curves, with effects from grey-out to loss of consciousness. The flight
  control system enforces a g limit that the pilot can override with `ARM` and `EXECUTE`. That limit
  is the realistic answer to "why can't I pull 20 g": the ship could, and the pilot could not.
- **Consumables.** Oxygen, water, food, carbon dioxide scrubbing, and the power that runs them.
  **Lean:** modelled from the start, with an endurance of weeks for a small craft, set per craft
  definition. Because compression does not save them, endurance is what makes a fast route worth
  more than a cheap one.

## Propulsion and distance

### The problem

Real distances are the central difficulty. Accelerating to the midpoint and decelerating after it,
the fastest trip over a distance d at a constant acceleration a takes 2√(d ÷ a) and costs a
delta-v of 2√(a d):

| Distance               | 0.001 g         | 0.01 g            | 0.1 g              | 1 g                 |
| ---------------------- | --------------- | ----------------- | ------------------ | ------------------- |
| Earth–Moon, 384,400 km | 4.6 d, 4 km/s   | 34.8 h, 12 km/s   | 11.0 h, 39 km/s    | 3.5 h, 123 km/s     |
| 1 AU                   | 90 d, 77 km/s   | 28.6 d, 242 km/s  | 9.0 d, 766 km/s    | 2.9 d, 2,422 km/s   |
| 30 AU                  | 495 d, 420 km/s | 157 d, 1,327 km/s | 49.5 d, 4,196 km/s | 15.7 d, 13,268 km/s |

The power in the exhaust is m a vₑ ÷ 2. For each tonne of ship at 1 g that is 49 GW at an exhaust
velocity of 10,000 km/s, which is Project Daedalus's pulsed fusion, and 490 MW at 100 km/s, where
the delta-v of the astronomical-unit rows is out of reach. Even if 99% of a terawatt left in the
exhaust, the rest would need hectares of radiator (see [Ship systems](#ship-systems)). A ship that
crosses an astronomical unit in days at 1 g is therefore not known physics. It is exactly the part
of _The Expanse_ that is invented. What known physics offers:

- **High thrust, low exhaust velocity.** Chemical rockets at about 4.4 km/s and nuclear thermal at
  about 9 km/s. Tenths of a g to several g, but only a few km/s of delta-v at any sensible mass
  ratio: 10 km/s costs a mass ratio of 3 even with nuclear thermal.
- **Low thrust, high exhaust velocity.** Fusion, extrapolated from studies such as Daedalus, gives
  hundreds of km/s of delta-v at modest mass ratios, at accelerations of the order of a thousandth
  of a g for a small craft. Fission-electric propulsion is short of that by one or two orders of
  magnitude: at the 20 kg per kilowatt that the National Academies call a significant challenge,
  it reaches 10⁻⁵ to 10⁻⁴ g with tens of km/s of delta-v.

Every craft definition is held to the same constraint: the jet power per kilogram of ship is
a vₑ ÷ 2, and the reactor and radiators must supply and shed it.

### Travel within a system

The owner has ruled that travel within a system works as in No Man's Sky: the whole of a system's
space is open to the player, its star included. The first jump drive works within a system as well
as between systems (see [Jump drives](#jump-drives)), so the pieces are:

- **Jumps for distance.** Crossing a system, or reaching a body, is a jump to a point near it.
- **Real engines for everything near.** Approach, orbit, rendezvous and skimming are flown under
  known physics: a manoeuvring engine for anything quick, and a cruise drive for longer legs.
- **Time compression for long coasts and slow burns**, flown by the flight computer.
- **Nothing is fenced off.** The limit on approaching an ordinary star is the one physics sets: the
  heat of its light on the hull, 136 kW/m² at 0.1 AU from the Sun (see
  [Ship systems](#ship-systems)). Near a compact remnant, tides and radiation can set it first.

**Lean: jump drives are the only invented technology.** Everything else is known physics, or
engineering extrapolated from it, the register the UX guide already sets for the consoles. A torch
drive, as _The Expanse_ has, would make the heat, power and propellant models decorative, and those
are the systems this phase exists to test. Time compression is an honest concession to play: it
changes how fast the player lives through time, not what happens in it.

The first drive strains this lean while it is fitted. A free jump into any orbit leaves the engines,
propellant and time compression little to do, so until a drive type with costs exists they are
exercised by scenarios and by craft fitted without a drive, as fighters will be.

What this leaves out is No Man's Sky's pulse engine: continuous flight between planets in a few
minutes, watching the destination grow. Real engines cannot do it, and a jump skips it. If flying
the first version shows it is missed, it can come back as a type of jump drive that moves the ship
continuously rather than in one step, so that jump drives stay the only invented technology. It is
listed under [Open questions](#open-questions).

### Engines

**Lean:** a craft carries two classes of engine, both real.

- A **manoeuvring engine**, high thrust and low exhaust velocity, chemical or nuclear thermal: for
  orbit insertion, rendezvous, docking and anything quick. Fighters are mostly this.
- A **cruise drive**, low thrust and high exhaust velocity, fusion: for longer legs, flown by the
  autopilot under compression.
- **Reaction control thrusters** for attitude and fine translation.

The reaction mass is hydrogen wherever it can be, which makes every tank a cryogenic one that boils
off, and makes gas giants a source of it. Reactor fuel lasts far longer than reaction mass, so
reaction mass is the recurring constraint.

### Refuelling

With no civilisations there is nowhere to buy propellant. Elite scoops it from stars, but a corona
is far too tenuous to fill a tank from. **Lean: skimming gas giants.** The craft makes a grazing
pass through the upper atmosphere and scoops hydrogen, trading drag, heating and the risk of going
too deep against the mass it gains. It is the classic hard science-fiction answer, and Traveller's
wilderness refuelling. It needs only the upper atmosphere of [Gravity](#gravity). Ice from comets,
rings and small moons, taken at a rendezvous rather than a landing, can follow.

## Jump drives

The galaxy brainstorm settled that a drive can jump to any system within range, charted or not. The
owner has ruled that HYPERION will have several types of jump drive with different gameplay
properties, and that the first is the simplest: it teleports the ship to its target, within a
system as well as between systems.

### Drive types

A drive type is data, like the rest of a craft definition. Each type fixes:

- **Range**, and what it depends on: a constant, or the ship's mass and the energy available.
- **Where it can start and end**: anywhere clear of a body, or only outside limits such as the
  tidal limit under [For later drive types](#for-later-drive-types).
- **The state on arrival**: position and velocity relative to the target, and what happens to the
  ship's energy.
- **Charge and recharge**: how long, and at what cost in power and heat.
- **Duration** in universe time.
- **What it costs** to run: energy, and anything else.

### The first drive: a teleport

**Lean**, for each property, since the owner has asked for the simplest drive and left the detail:

- **Range: 1,000 ly**, a parameter of the type, set large for testing. At the Sun's radius, with
  Milky Way-like parameters, that sphere holds several million systems, so targets are not listed.
  They are chosen where they are already shown: on the `GALAXY` display's map or local chart, on
  the `SYSTEM` display, or by designation or ID. Any system in range is a target, charted or not,
  but a body is one only once the ship knows it. Longer journeys chain jumps.
- **Any point in range, within a system too, the star included.** The galaxy brainstorm's rule names
  systems, and this drive widens it to points. It refuses only an arrival inside a body, or below
  the top of its atmosphere or the star's photosphere, with a margin. The navigation computer states
  the heat load at the arrival point before `EXECUTE`, and arriving where that load exceeds the
  radiators is left to the pilot.
- **Arrival matched to the target.** The pilot chooses the arrival state from the target: a circular
  orbit about a body at a chosen altitude, or rest relative to the target at a chosen offset. A
  point in empty space means rest relative to the frame there: the body's within its Hill sphere,
  the system's barycentre elsewhere in a system, and between the stars the galaxy's circular
  velocity at that point, about the mean motion of the systems around it. So the drive is also a
  free change of velocity, which bypasses the propellant economy while it is fitted. That suits
  testing, and later types restore the cost.
- **No universe time in transit, and no cost.** `ARM` drops the time rate to 1× and charges the
  drive for a few seconds of universe time, and `EXECUTE` then jumps.
- **What the sensors then see.** On arrival the sensors see the destination's past light cone, which
  the galaxy already models: after a 10 ly jump the pilot sees their point of departure as it was
  ten years before, and a jump outruns any news. There is no faster-than-light communication.
- **Gas and dust.** The galaxy brainstorm left "what a faster-than-light drive does in gas" to this
  document. The drive ignores the medium in transit, and the ship at each end meets what physics
  says is there: extinction, and the erosion and radiation load of gas at speed.

### For later drive types

The rules worked out in this document's first draft are kept here as candidates for drive types with
real costs. None of them applies to the first drive.

#### Range from energy

A jump draws on a charge that the reactor builds up in the drive. The energy grows with the ship's
mass and the distance, so range falls as the ship is loaded (Elite's lesson: loadout is a navigation
decision), and the recharge time ties the drive to power and heat. There is no invented consumable.

The galaxy says what a range means. With Milky Way-like parameters the generated density at the
Sun's radius is about 0.002 systems per cubic light-year, as measured (Reylé et al. 2021;
Kirkpatrick et al. 2024), and other seeds give from 0.0008 to 0.008. At 0.002 the nearest neighbour
is about 4.4 ly away, a 10 ly sphere holds about 8 systems, and a 20 ly sphere about 67. Density
falls with height above the disc, so a short range makes the space above the disc and the thin
stretches between arms real obstacles, as the galaxy brainstorm intends.

#### The tidal limit

A drive that cannot work where the tidal field of any body exceeds a threshold τ needs
G M ÷ r³ < τ. The tidal acceleration across a length L is about 2 G M L ÷ r³, so the criterion says
that the drive fails where space is too strongly curved across it. Because
G M ÷ r³ = (4π ÷ 3) G ρ (R ÷ r)³, the limit is a fixed number of radii for bodies of the same
density, which is Traveller's 100-diameter rule with a reason. Calibrated to 100 diameters at Earth,
τ ≈ 1.9 × 10⁻¹³ s⁻², and:

| Body                             | Limit           | In diameters |
| -------------------------------- | --------------- | ------------ |
| Moon                             | 294,000 km      | 85           |
| Mars                             | 606,000 km      | 89           |
| Earth                            | 1.27 million km | 100          |
| Jupiter                          | 8.7 million km  | 62           |
| 0.2 M☉ red dwarf                 | 0.35 AU         | 169          |
| 0.6 M☉ white dwarf               | 0.50 AU         | 4,300        |
| Sun                              | 0.59 AU         | 63           |
| 1.4 M☉ neutron star              | 0.66 AU         | 4 million    |
| Central black hole, 4.3 × 10⁶ M☉ | 96 AU           | 570          |

Diameters are the body's own, and the black hole's are of its event horizon. The red dwarf's radius
is the observed 0.22 R☉ at that mass (Boyajian et al. 2012); code would take radii from the stellar
model. The limits are for each body alone.

Consequences:

- The limit grows as the cube root of the mass and ignores density at a distance, so a neutron star
  is no worse than the Sun. The central black hole's is about a hundred astronomical units.
- Around the Sun, Mercury lies inside the Sun's limit and Venus outside it.
- A red dwarf's temperate planets orbit at under a tenth of an astronomical unit, well inside its
  limit. The commonest temperate worlds in the galaxy would therefore always need a sublight leg of
  over a quarter of an astronomical unit, which takes weeks.
- A moon inside its planet's limit is reached only by a sublight leg from the planet's limit. The
  Moon, at 384,400 km, lies inside Earth's 1.27 million km, so its own limit never binds.
- Limits are checked against the whole tidal field, summed over bodies, at the departure point now
  and the arrival point at the arrival time. Summed, the fields push a limit out: the Sun's field at
  1 AU is a fifth of τ, and moves Earth's limit out by about 8% along the line to the Sun.
- A drive refuses to jump inside a limit, as an interlock its display shows, rather than misjumping.

#### Velocity and energy on arrival

A jump moves the ship to a different place in a gravitational field, so it changes the ship's
potential energy by m ΔΦ, where ΔΦ is the difference in potential between the two ends. What
happens to the velocity decides what happens to that energy. Four rules:

1. **Velocity kept, energy free.** The change in potential energy comes from nowhere. That is a
   perpetual-motion machine: fall from 10 AU to the Sun's limit, jump back out, and keep 51 km/s of
   excess speed, again and again. Not acceptable for any drive type.
2. **Velocity kept, energy paid.** The drive's charge supplies m ΔΦ for a jump uphill and absorbs
   it for a jump downhill. A jump whose energy the charge cannot supply or hold is refused. Energy
   is conserved, and because the velocity is untouched in every frame, the rule does not depend on
   the frame it is stated in.
3. **Velocity matched to the target, free.** The first drive's rule. Simple and generous: every
   jump is also a free change of velocity, and the galaxy's kinematics play no part in travel.
4. **Velocity chosen at an energy cost**, growing with the square of the change.

Rule 2 is the realistic one, and its consequences are what would make a drive type built on it play
differently:

- After a jump to another system the ship still has its old velocity, so it carries the difference
  between the two stars' velocities: tens of km/s between neighbours in the disc, and more for stars
  of the thick disc and the halo (Barnard's Star moves at about 140 km/s relative to the Sun). The
  galaxy's kinematics, generated per population, become part of route planning.
- Between two similar stars the wells nearly cancel. The potential at a tidal limit is
  (G M)^⅔ τ^⅓, 1.5 GJ/kg for the Sun, and the charge pays only the net difference.
- Within a system the kept velocity is a different orbit at the new radius. A jump from Earth's
  orbit to Mars's distance keeps Earth's 29.8 km/s where circular speed is 24.1 km/s, an orbit
  reaching 4.9 AU, and costs 0.31 GJ/kg, 6 TJ for a 20 t craft.
- The galaxy's own potential matters over long journeys. With a flat rotation curve of 230 km/s
  the potential changes by v_c² ln(R₂ ÷ R₁): about 20 MJ/kg per 10 ly of radius near the solar
  radius, small against a star's well, but about 10¹¹ J/kg from there to a kiloparsec from the
  centre, petajoules for a small craft. A craft heading coreward must bleed that energy off through
  its radiators between jumps, and one heading rimward must pay it from its reactor.
- Energy can never be released as heat at the moment of a jump. At hundreds of megajoules per
  kilogram it would vaporise the ship, so the charge's capacity is the limit.
- Jumps can still turn the charge's energy into delta-v. A short outward jump followed by a coast
  reaches Mars for 1.8 km/s, against 5.6 km/s for a Hohmann transfer, and a fall inward followed by
  a jump out turns the reactor's energy into speed without reaction mass. Both run at the reactor's
  power, and the second at the pace of a fall through a well, years at the Sun from 10 AU, so this
  is a slow propellant-free drive and not a free one.

## Ship systems

Every system publishes telemetry against limits, raises alerts from the server, and has procedures,
as the UX guide requires.

- **Power.** Reactor, batteries and capacitors, and solar power near a star. Electrical buses with
  priorities and load shedding, in the guide's own example:
  `EPS-2 BUS B UNDERVOLT: shed load or start APU`.
- **Heat.** Everything a ship does ends as heat, which leaves only by radiation. A radiator at 1,000
  K sheds 57 kW per square metre per face, and at 300 K only 0.46. Radiators are large and fragile,
  deployed and stowed. Heat sinks absorb bursts, and coolant loops move heat between them. Starlight
  adds to the load: 1,361 W/m² at 1 AU from the Sun and 136 kW/m² at 0.1 AU, so a close pass by a
  star is a thermal problem, and for an ordinary star the limit on how close a ship can go. A ship's
  radiators are also its brightest feature in the infrared, which is how other sensors will find it.
- **Propellant.** Tanks with mass, position, pressure and boil-off. Delta-v remaining is computed
  from them.
- **Life support.** Oxygen, carbon dioxide scrubbing, water, cabin temperature and pressure, leaks,
  and the consumables of [The pilot](#the-pilot).
- **Avionics.** The flight control system, the navigation computer, the sensors' processors, with
  redundancy.
- **Structure.** Hull integrity and structural load limits.
- **Damage and failures.** Each component has a health and failure modes. Wear-out failures draw
  from the session's streams, and hazards damage what they reach: collisions, radiation from flares,
  heat, dust and gas at speed, and tides near compact remnants. **Lean:** a manual cold start is
  available, in the manner of DCS, and so is an automatic one, since depth must never be required on
  the first flight.

## Sensors and exploration

The consoles see only what the sensors have resolved; the server holds the truth. The galaxy phase
provides detection by flux limits, bearings and resolved hosts, and body records that degrade by
level of detail. This phase adds sensor models and the exploration loop on top.

- **Passive telescopes** by band, optical and infrared, with aperture, detector and integration
  time, and limiting magnitudes that read the galaxy's extinction. **Radio**, for the 21 cm line and
  pulsars. **Active radar and lidar**, whose return falls as r⁻⁴, so they are short-ranged and
  announce the ship that uses them.
- **Measurements, not scans.** Astrometry over time gives orbits. Photometry and spectroscopy give
  temperature, composition and radial velocity.
- **A system is explored before it is visited.** From outside, planets are found by the real
  methods, with their real biases: transits need the orbit edge-on, radial velocity favours massive
  close-in planets, direct imaging favours young giants on wide orbits. These are the ship's
  equivalent of Elite's discovery scanner.
- **The ladder of knowledge** runs from point source, to orbit determined, to bulk properties, to
  surface and atmosphere, to mapped from orbit. Each rung has a method and an instrument.
- **The log** records each discovery with its time and position, under the catalogue designation
  derived from its ID.
- **Mapping** from orbit needs the orbital-scale map generator. The galaxy brainstorm includes
  orbital-scale maps in its scope, but plan 14 builds only their inputs, the surface seed and global
  figures, and leaves the generator to a later consumer. It is the first layer of the planets to
  come, so it belongs to their brainstorm; see [Planets](#planets).

## The cockpit

### Displays

New displays, each to join the nomenclature list:

- `VIEW`: the view outside; see [The view outside](#the-view-outside).
- `FLIGHT`: the primary flight display. Attitude against the chosen reference on an attitude ball,
  as Apollo's flight director attitude indicator, with rates, g-load, velocity in the reference
  frame, the flight control modes, thrust and delta-v remaining.
- `NAV`: the trajectory. The current path and its prediction dashed, planned burns in `--target`,
  osculating elements, and the times to periapsis, apoapsis and the next change of frame.
- `JUMP`: the selected target with its distance, the arrival states it allows, the heat load at the
  arrival point, the drive's charge, and `ARM` and `EXECUTE`. Targets are picked on the `GALAXY` and
  `SYSTEM` displays or entered by designation or ID.
- `SENSORS`: contacts, measurements in progress and the discovery log.
- Systems displays, as schematics: power, thermal, propellant and life support.

`LINK` stays as it is. `GALAXY` and `SYSTEM` gain one command, to make the selection the jump
target.

### The view outside

The owner has ruled that the view starts as a basic wireframe of the surroundings, and that a 3D
game rendering engine is needed for what follows (see
[The rendering engine](#the-rendering-engine)). A wireframe also suits the UX guide, whose spatial
displays are thin vector lines on `--surface-0`. **Lean**, for what `VIEW` draws:

- **From the pilot's seat.** A perspective camera fixed to the ship's axes, with look-around and a
  choice of field of view. The reference frame is named on the display.
- **Bodies true to scale**, as spheres drawn by a graticule of latitude and longitude that turns
  with the body's rotation, so that rotation and approach can be seen. Rings as their ellipses, the
  star as a sphere at its radius, and other craft, such as the home base, as wireframe hulls from
  their definitions.
- **Orbits of bodies**, on request, as the `NAV` display draws them.
- **Stars** as points, from the range query, brighter by apparent magnitude down to a limit.
- **Symbology in the guide's grammar**: a flight path marker for the velocity against the chosen
  reference, target brackets with range and closure rate, and the destination reticle in
  `--target`.
- **What the ship knows.** The server sends the scene: the bodies, craft and stars near enough to
  draw, with positions at the retarded time, as sensors see them. Within a system the light-time is
  seconds to hours and the difference is small, but the rule is the same one, and the client never
  holds truth that the ship has not seen.

The guide's rules for spatial displays were written for charts: orthographic projection, and
redrawing only on demand. A view is neither, so the guide gains a class of display of its own,
perspective and redrawn every frame, always labelled as a view.

### Controls and input

- Keyboard and mouse, gamepads, and HOTAS through the browser's Gamepad API, which Chromium extends
  to most joysticks, with WebHID for devices it does not cover. Bindings are configured as a ship
  system, since the guide puts settings inside the fiction.
- Stick axes are a continuous stream at the tick rate, not commands. The guide's closed-loop
  commanding (`PENDING`, then the server's answer) applies to discrete commands. For continuous
  inputs the displays show what the flight control system commands in `--target` beside what the
  ship does.
- Hazardous commands take `ARM` then `EXECUTE`: jump, manual main-engine ignition, jettison,
  reactor scram, and overriding the g limit.

### Sound

The guide allows short, dry, functional sounds and no interface music. A cockpit adds sounds carried
through the structure: thrusters firing, pumps, the hull under load. They are information, not
atmosphere, because a pilot hears a stuck thruster before any display shows it. **Lean:** allow
structure-borne sounds that correspond to real events in the simulation, which needs an edit to the
guide.

## The rendering engine

The wireframe needs very little. The engine is chosen for what comes after it: a textured view of
real-scale systems, and then planets that can be flown down to, as in No Man's Sky. What it must do:

- **Real scale.** A GPU works in 32-bit floats, which cannot hold a system, let alone a galaxy. The
  view is drawn relative to the camera: positions are differenced in 64 bits, in the frames the
  galaxy already defines, and only small offsets reach the GPU. This is the floating origin. Depth
  from centimetres to astronomical units needs a reversed-Z floating-point depth buffer. A
  logarithmic one also works, but it writes depth from the fragment shader and loses early depth
  testing.
- **Planets later.** Terrain on a subdivided cube-sphere, streamed at increasing detail as the ship
  descends, atmospheric scattering, clouds, oceans and instanced vegetation. GPU compute is close to
  essential.
- **The same terrain on both sides.** Anything the ship can collide with must be identical in the
  client and the server. `hyperion-sim` already compiles to WebAssembly, and CI checks it bit for
  bit there. So the terrain generator can be sim code that the server runs natively and the client
  runs in workers, if the browser target joins those checks. Detail finer than collision needs can
  be generated on the GPU, where it need not match. The client then holds the body's surface seed,
  the top level of its record, which reveals the whole surface at once. The planets brainstorm must
  square that with the rule that the client holds only what the ship has seen.
- **Fit with the consoles.** Text that must be read stays in the DOM, set in B612, where the UX
  guide's tooling checks it. A canvas is paired with a DOM list of its marks. Displays open in
  windows of their own.
- **Linux first.** The owner develops on it.
- **Stability.** The project is long-lived, and an engine that breaks its API every release is a
  standing cost.

The options:

| Option                          | Runs                                   | For                                                                                                                                                                                                                                  | Against                                                                                                                                                          |
| ------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Babylon.js 9                    | In the Electron renderer, as a display | TypeScript-first. WebGPU and compute shaders; its WebGL2 fallback goes unused, since WebGPU is required. Large-world rendering with a floating origin, new in 9.0. A stated commitment to backward compatibility. A complete engine. | Heavier than three.js, with a smaller community. Browser limits (below).                                                                                         |
| three.js                        | In the Electron renderer, as a display | The largest ecosystem and the most examples of procedural planets. React bindings (react-three-fiber). WebGPU renderer with a reversed depth buffer.                                                                                 | Breaking changes in most releases. The floating origin is ours to write, though our frames make that small. Browser limits.                                      |
| Bevy                            | Native, as a separate display process  | Rust, sharing the sim without WebAssembly. Native Vulkan, multithreaded. `big_space` gives nested integer grids with a floating origin, which map onto the galaxy's frames. Built-in atmosphere.                                     | A second UI technology, with text outside the DOM and the guide's tooling. Breaking releases before 1.0. A second binary, and input split between two processes. |
| Godot 4, double-precision build | Native, as a separate display process  | A mature engine and editor, 64-bit positions when built for them, Rust through GDExtension.                                                                                                                                          | The same split as Bevy, plus a custom engine build.                                                                                                              |
| Unreal, Unity                   | Native, replacing or beside the client | Top-end rendering.                                                                                                                                                                                                                   | Heavy, closed or licensed, and built to own the whole application, which the consoles already are.                                                               |

The browser limit that matters is WebGPU on Linux. Chromium enables it by default there only for
Intel Gen12 and later (from Chrome 144) and NVIDIA under Wayland (from 147). Other GPUs need
command-line switches. Electron can set them from its main process, at the cost of bypassing
Chromium's GPU blocklist. WebGL2 works everywhere but has no compute shaders. A native engine talks
to Vulkan directly and has no such gate.

**Lean: Babylon.js, in the renderer, as one more display.** It keeps one client, one UI technology
and the UX guide's tooling, and its large-world rendering and backward compatibility answer the
first and last requirements directly. The wireframe `VIEW` is built on it from the start, so that
precision, depth and window handling are proven at real scale before anything depends on them.
Before the planets phase, a spike settles whether the browser can carry them: an Earth-sized planet
from orbit to a metre above the ground, terrain from the sim in WebAssembly workers, at 60 frames a
second at 1080p on a discrete GPU, and at 30 at 720p on the low setting on the owner's Linux
machine, whose integrated GPU is the performance floor; see
[Performance budget](rendering-and-planets.md#performance-budget). If it cannot, the fallback is
Bevy as a native display host. The protocol is already the boundary, so the host is just another
client of the session, which also suits the bridge's main screen on a machine of its own.

## Starting, failing and purpose

- **Where the player starts.** With no civilisations there are no stations. The options are a lone
  craft in a chosen system, or a home base as pinned content: a depot, or the future main ship
  parked, where the craft docks to refuel, repair and save. **Lean:** a home base. It is the first
  docking target and exactly the carrier relationship that fighters need later. The starting system
  is chosen by a rule (a Sun-like star in the thin disc, with a gas giant for fuel) or by the player
  from the `GALAXY` display.
- **What happens on failure** is open: a restart from the last save, or permanent loss.
- **What the player does.** For the first milestone, open-ended exploration. **Lean:** scenarios as
  data, a start state and objectives ("rendezvous with the depot from a 400 km orbit"), under the
  guide's training banner. They serve as tutorials and as integration tests of the flight model.

## Later phases

### Fighters and the bridge

- **Craft definitions are data**: a hull (mass, inertia, dimensions, structural limits) and modules
  placed on it (engines, thrusters, reactor, radiators, tanks, sensors, jump drive, docking port).
  One flight model and one systems model fly all of them.
- **A fighter** is a craft with high-thrust engines, short endurance and no jump drive, launched and
  recovered by a carrier. The docking built for the home base is the same mechanism.
- **Remote piloting is limited by light.** The control loop's delay is twice the distance ÷ c, two
  seconds at one light-second, so telepresence works only close to the carrier. A crewed fighter is
  a player at another client, as in Artemis.
- **The bridge** is the same module types at larger scale, and its stations are the single seat's
  displays and authority divided among a crew.

### Planets

The owner has ruled that full 3D planets will follow, as in No Man's Sky: a seamless descent from
orbit to the surface, and landing. Real scale makes that harder than No Man's Sky has it, but it is
done: Elite Dangerous lands on real-scale planets, and SpaceEngine and Outerra draw whole planets
from orbit to the ground. So it passes the realism ruling's test of feasibility. The galaxy
brainstorm's "surfaces stop at orbital scale for now" stands for this phase.

This phase must leave room for:

- body-fixed rotating frames, in which flight near a surface is flown;
- an atmosphere model that extends from the top to the ground, with lift and aerodynamic control;
- collision with terrain, from height queries that the client and server answer identically;
- a renderer that stays precise to centimetres at the surface and streams terrain as it descends;
- the galaxy's surface seed and global figures as the planet generator's inputs.

**Lean:** one brainstorm for planets, covering maps from orbit and surfaces together, written
before that phase.

## Server and protocol

- Session messages: create, open, list and save sessions; set the time rate; subscribe to
  telemetry at chosen rates; subscribe to the view's scene; a control input stream; commands with
  closed-loop results; alerts through the galaxy phase's notification path.
- **Lean:** JSON first, as now. Binary telemetry only if measurement shows JSON is the bottleneck.
- The server enforces the separation of truth and Knowledge even for one player, so the bridge needs
  no retrofit.

## Testing

- **Flight model.** On a two-body coast, energy and angular momentum are conserved and the orbit
  closes after many periods, in agreement with Kepler propagation. In a circular restricted
  three-body test system the Jacobi constant is conserved, and tadpole orbits about L4 persist when
  the smaller body's share of the mass is under 0.0385 (Routh's criterion). A burn's delta-v equals
  vₑ ln(m₀ ÷ m₁). Torque-free rotation of a rigid body is unstable about the intermediate axis and
  stable about the others, with angular momentum conserved. Position and velocity are continuous
  across every change of frame. A coast agrees at 1× and 100,000× within a stated tolerance.
- **Determinism.** A session replays bit for bit from its start state and input log on all three
  architectures.
- **The first drive.** Every arrival state is what was asked for: a circular orbit at the chosen
  altitude, or rest relative to the target at the chosen offset. Arrivals inside a body or below
  its atmosphere or photosphere are refused, and range is enforced.
- **The view.** Hull detail a metre away and a planet an astronomical unit away in one frame without
  depth fighting or jitter, checked by eye and by a test scene.
- **Scenarios** as integration tests.
- **By feel.** The point of this phase is that a person flies the ship. The flight control gains and
  the displays will be tuned by hand, and the plan should budget for it.

## Decisions

Settled with the project owner on 2026-09-21, in the first round:

- **The first playable experience is single-seat.** One player flies one small ship, in the manner
  of Elite Dangerous. The multi-position bridge comes after it.
- **The single-seat craft is reused.** The same definitions fly later as fighters and other small
  craft beside the main ship.
- **Its purpose is to test movement and the core systems** with one person before a crew divides
  them.

Settled on the same day, in the second round:

- **The realism ruling applies**, as it does to the galaxy: where a choice is open, the most
  realistic answer wins, unless it is technically infeasible.
- **Travel within a system works as in No Man's Sky.** The whole of a system's space is open to the
  player, its star included.
- **Full 3D planets follow later**, as in No Man's Sky. See [Planets](#planets).
- **Jump drives are a family of types** with different gameplay properties. The first teleports the
  ship to its target, and works within a system as well as between systems. See
  [Jump drives](#jump-drives).
- **The first drive's range is large, for testing.** 1,000 ly is this document's choice.
- **The view starts as a basic wireframe** of the surroundings, and a 3D game rendering engine is to
  be chosen for what follows. See [The view outside](#the-view-outside) and
  [The rendering engine](#the-rendering-engine).

## Open questions

1. **A continuous mode within a system.** Whether No Man's Sky's pulse engine, continuous flight
   between planets in minutes, should come back as a drive type of its own. **Lean:** not at first;
   decide after flying the teleport.
2. **The pilot's body.** Whether acceleration limits and loss of consciousness are modelled.
   **Lean:** yes, with an overridable g limit.
3. **Consumables and endurance.** **Lean:** modelled, weeks for a small craft.
4. **Where the player starts.** **Lean:** a home base as pinned content, in a system chosen by rule
   or by the player.
5. **Failure.** Restart from a save, or permanent loss.
6. **Refuelling.** **Lean:** skimming gas giants first; ice at a rendezvous later.
7. **The planets brainstorm.** When it is written. **Lean:** before the planets phase, with maps
   from orbit and surfaces in one document.
8. **Structure-borne sound.** **Lean:** allowed for real events, with an edit to the UX guide.
9. **Naming discoveries.** Whether the player can give proper names, as an overlay on catalogue
   designations.
10. **Pausing.** **Lean:** allowed in single-player, as a rate of zero under a banner.
11. **Views in the UX guide.** **Lean:** a class of display of its own, perspective and redrawn
    every frame, always labelled as a view, with symbology in the guide's grammar.

The engine, the integrators, the tick rate and the wire format are technical choices, made above as
leans and open to review rather than waiting on a ruling.

## Suggested order of attack

Not a plan, only the dependency order a plan would follow:

1. Sessions, the loop at 64 Hz, persistence, and the local server started by the client, with a
   craft as a point mass on a trivial display. A thin vertical slice.
2. The rendering engine, and `VIEW` as a wireframe at real scale.
3. The first jump drive and the `JUMP` display, arriving at rest, so that testing can reach
   anywhere.
4. Gravity and propagation among the planetary stage's bodies, frames, time compression, arrival in
   orbit, and the `NAV` display with a predicted trajectory.
5. Attitude dynamics, thrusters, the flight control modes, the `FLIGHT` display and input devices.
6. Engines, propellant and delta-v, planned burns and the autopilot.
7. Power, heat and life support, their displays, the heat load on `JUMP`, and alerts with
   procedures.
8. Sensors over the Knowledge overlay, and the exploration loop.
9. Hazards, damage and failures.
10. Refuelling, the home base and docking.
11. Craft definitions for fighters, and the hooks a carrier will need.

**Lean, for the first milestone:** steps 1 to 5. A player jumps to any planet within 1,000 ly,
arrives in orbit, and flies there on thrusters with time compression, seen through the wireframe
view. It is the smallest thing that exercises the loop, the flight model, the drive and the cockpit,
and it answers the question this phase was chosen to answer: does flying feel right?

Sensors come at step 8, so until then the ship knows everything: the server grants every level of a
body's record, as plan 14's does until the Knowledge overlay exists.

## Sources

Figures above are rounded and should be re-checked against these when they become code.

- Artemis Spaceship Bridge Simulator 2.4.0 release notes (the fighter pilot position).
  <https://store.steampowered.com/news/app/247350/view/2906466828267337599>
- EmptyEpsilon wiki (the Single Pilot station). <https://github.com/daid/EmptyEpsilon/wiki>
- Orbiter 2010 User Manual, keyboard reference (time acceleration up to 100,000×).
  <https://img1.wsimg.com/blobby/go/bdeed67b-0796-4663-94e5-3744dd90d9fd/downloads/1buj8jvnq_883095.pdf?ver=1604452105702>
- Kerbal Space Program Wiki, _Time warp_. <https://wiki.kerbalspaceprogram.com/wiki/Time_warp>
- Traveller Wiki, _Jump drive_, and Freelance Traveller, _The 100-Diameter Limit_.
  <https://wiki.travellerrpg.com/Jump_drive>
  <https://www.freelancetraveller.com/features/rules/navigation/hundreddiam.html>
- NASA-STD-3001 Volume 2, Revision C, _Human Factors, Habitability, and Environmental Health_
  (sustained acceleration limits by axis and duration).
  <https://www.nasa.gov/wp-content/uploads/2020/10/2022-04-08_nasa-std-3001_vol_2_rev_c_final.pdf>
- NASA/TM-20205008196, _Artemis Sustained Translational Acceleration Limits: Human Tolerance
  Evidence from Apollo to International Space Station_.
  <https://ntrs.nasa.gov/api/citations/20205008196/downloads/TM-20205008196.pdf>
- G tolerance in a human centrifuge, Scientific Reports 10 (2020) (9 g for 15 s with an anti-g suit
  and straining manoeuvre). <https://www.nature.com/articles/s41598-020-78687-3>
- Bond, Martin et al. 1978, _Project Daedalus: The Final Report on the BIS Starship Study_, JBIS
  Supplement, S1–S192 (exhaust velocities of about 10,600 and 9,200 km/s for the two stages, seen
  here only through secondary sources).
- National Academies 2021, _Space Nuclear Propulsion for Human Mars Exploration_,
  doi:10.17226/25977 (specific mass and specific impulse of nuclear electric propulsion; nuclear
  thermal specific impulse).
- Winchell Chung, _Atomic Rockets_ (engine classes, radiators, torch drives).
  <https://www.projectrho.com/public_html/rocket/>
- Boyajian et al. 2012, _Stellar diameters and temperatures II_, ApJ 757, 112 (radii of M dwarfs).
- Reylé et al. 2021, _The 10 parsec sample in the Gaia era_, A&A 650 (local density of systems).
- Kirkpatrick et al. 2024, _A full-sky 20pc census of stars and brown dwarfs_, ApJS 271, 55 (local
  density of systems).
- NASA Jupiter fact sheet (scale height, gravity, temperature at 1 bar).
  <https://nssdc.gsfc.nasa.gov/planetary/factsheet/jupiterfact.html>
- CODATA 2018 (the Stefan–Boltzmann constant); Kopp and Lean 2011, GRL 38 (total solar irradiance,
  1,361 W/m²).
- WebGPU implementation status, by browser and platform.
  <https://github.com/gpuweb/gpuweb/wiki/Implementation-Status>
- Babylon.js, _Floating Origin_ (large-world rendering).
  <https://doc.babylonjs.com/features/featuresDeepDive/scene/floating_origin>
- three.js, _WebGPURenderer_ (reversed depth buffer). <https://threejs.org/docs/pages/WebGPURenderer.html>
- Bevy, _Bevy + WebGPU_, and `big_space`. <https://bevy.org/news/bevy-webgpu/>
  <https://docs.rs/big_space/latest/big_space/>

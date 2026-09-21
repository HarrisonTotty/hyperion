# Single-Player Experience

Brainstorm for HYPERION's first playable experience: one player flying one small ship through the
generated galaxy, in the manner of Elite Dangerous, before the multi-position bridge exists. This is
a design exploration, not a plan. Decisions are marked **Lean** where there is a recommendation,
and the open ones are collected under [Open questions](#open-questions). What has been settled is
under [Decisions](#decisions).

It builds on [the galaxy generation brainstorm](galaxy-generation.md) and assumes all of it is
built. Every system, star, body and event is a pure function of seed, ID and time. The range query
and the `GALAXY` and `SYSTEM` displays exist, and sensors see the past through a minimal Knowledge
overlay.

## Goal and scope

A player opens a universe and is given a small ship somewhere in it. They can fly it among bodies
that really orbit, jump between systems, find out what is there, and keep the ship and themselves
alive while doing it. One client, one seat, and every system of the ship within reach.

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
- The jump drive's rules, which the galaxy brainstorm left for later.
- Ship systems: power, heat, propellant, life support, damage and failures.
- Sensors and exploration, over the Knowledge overlay.
- The hazards the galaxy already generates: starlight and heat, flares, dust and gas at speed,
  tides, collisions.
- The cockpit: the single-seat console, its displays, controls and input devices.

Out of scope, each with hooks defined here:

- The multi-position bridge. The ship's systems are divided so that the bridge is a regrouping of
  the same commands and displays; see [The seat and the bridge](#the-seat-and-the-bridge).
- Other actors: civilisations, stations, other ships, trade, communications and combat. The module
  and damage models leave room for weapons without designing them.
- Landing and flight in an atmosphere below skimming altitude. Surfaces stop at orbital scale.
- LLM enrichment.
- Several players in one universe. The session design must not rule it out.

## What to take from the references

| Reference             | Take                                                                                                                                                                                                                                                                                                                                                                                     | Leave                                                                                                                                                                                |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Elite Dangerous       | One pilot runs a whole ship. Modules with power and heat budgets, and heat as a resource to manage. Jump range that depends on the ship's mass, so loadout is a navigation decision. Scooping fuel as a survival loop. Exploration as discover, scan and map, with a log. Flight assist, which decides whether a keyboard can fly the ship at all. Fighters launched from a larger ship. | A speed cap on a Newtonian ship. Supercruise, a second invented drive for travel within a system. Fighters flown by telepresence at any range.                                       |
| Orbiter               | Honest Newtonian flight among real orbits. Multi-function displays as the interface to orbital mechanics: transfers, plane changes, synchronised orbits. Time acceleration from 10× to 100,000×, which makes real distances playable.                                                                                                                                                    | Shallow systems: no power, heat or life support without add-ons. A rendering-first presentation.                                                                                     |
| Kerbal Space Program  | Manoeuvre nodes: a planned burn that the player edits against a predicted trajectory. Delta-v budgets as readouts. Time warp that drops to real time when something needs attention.                                                                                                                                                                                                     | Patched conics and their jumps at sphere-of-influence boundaries. A solar system at a tenth of real scale.                                                                           |
| DCS World, Falcon BMS | Depth of systems: failure modes, checklists, start-up procedures, full HOTAS binding. A cockpit that rewards knowing the aircraft.                                                                                                                                                                                                                                                       | A steep entry. Depth must be available, never required on the first flight.                                                                                                          |
| Artemis, EmptyEpsilon | Artemis 2.4 added a fighter pilot position: a player flies a single-seat craft launched from the crew's ship. EmptyEpsilon's Single Pilot station combines Helm, Weapons and short-range scanning into one console, so one person can fly a ship built for a crew. That regrouping is the model for this whole brainstorm.                                                               | Arcade flight models.                                                                                                                                                                |
| _The Expanse_         | High-g flight as a limit on the crew, not the ship. Flip-and-burn. A ship that shows up in infrared because it has to shed heat.                                                                                                                                                                                                                                                         | The Epstein drive's free lunch: gees of thrust at an exhaust velocity that implies terawatts, with no waste heat to show for it. If HYPERION invents a drive, it says so; see below. |
| Traveller             | The 100-diameter limit: a jump drive that cannot work deep in a gravity well, so every arrival and departure has a sublight leg.                                                                                                                                                                                                                                                         | A rule with no reason behind it. [The jump drive](#the-jump-drive) derives one.                                                                                                      |

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
  navigation display on a second monitor as home-cockpit builders do with multi-function displays.
  The same mechanism later puts a station on another machine.

## Sessions and the loop

### Sessions

A session is a universe, the craft in it and the players flying them. Single-player is one craft
and one player. The server holds the session, and the client never simulates.

**Lean:** single-player runs the same server, locally. The Electron main process starts
`hyperion-server` on the loopback interface as a child process and connects to it, and connecting
to a remote server stays possible. One code path means nothing written for one player has to be
rewritten for several.

A session's state is play state, not generated data: the craft, the clock, the Knowledge gathered
and the deltas that play has made. The galaxy brainstorm's overlays already anticipate it. It is
stored beside the universe, under `universes/<id>/sessions/<id>/`, and saved on a timer and on
request.

### The clock

The session holds the universe's "now". **Lean:** a session starts at the epoch, UT 0, and time
only moves forward. Mission elapsed time, `MET`, counts from the session's start, as the UX guide
already defines.

**Lean: the simulation steps at 64 Hz.** `UniverseTime` counts whole nanoseconds, and 1/64 s is
exactly 15,625,000 ns, so every tick is exact and the clock never drifts. 1/60 s is not a whole
number of nanoseconds.

**Time compression** is what makes real distances playable (see
[Propulsion and distance](#propulsion-and-distance)). The rates are powers of ten from 1× to
100,000×, the ceiling of both Orbiter and KSP. Each tick then advances 10ᵏ × 15.625 ms, which is
still exact. **Lean**, for the rules:

- The flight computer drops the rate on its own when the pilot needs to act: an alert, a change of
  frame, the start of a planned burn, arrival at a jump limit, and closing within a set distance of
  another object or the top of an atmosphere.
- Above 10×, manual inputs are ignored and attitude is held by the flight computer. Burns under
  compression are the flight computer's, flown to a plan.
- Compressing time never saves anything but the player's time. A 30-day transfer at 100,000× lasts
  26 seconds and consumes 30 days of air, water and food; see [The pilot](#the-pilot).

The galaxy's clock window is H = 1,000 years either side of the epoch. At 100,000× the window
forward takes 3.7 days of continuous play, so a session will not reach it in practice, but the
server enforces it.

Pausing is a rate of zero, available in single-player only. The UX guide asks for a persistent
banner for any mode that is not live operation, and the rate sits beside `UT` in the header strip.

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
is deferred.

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
oblateness, J₂, if the planetary stage provides a flattening; that is a hook to check. The
navigation computer still shows osculating elements about a chosen body for planning, as Orbiter's
displays do.

Atmospheres matter only at the top: for aerobraking and for skimming gas giants (see
[Refuelling](#refuelling)). Drag and heating need the density against height, which follows from
quantities the planetary stage already has: the scale height is kT ÷ (μ m_u g). For Jupiter's 165 K
at 1 bar, mean molecular mass 2.22 and effective equatorial gravity 23.12 m/s² it gives about 27 km,
as NASA's fact sheet does. The gravity must be the effective one at the skimming latitude, rotation
included. There is no lift and no aerodynamic control.

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
  holding means coasting, and `TARGET`, for station-keeping.
- Real spacecraft autopilots offer the same choices: rate command with attitude hold, direct
  control, and minimum-impulse pulses. Elite's flight assist is `RATE` with `VEL` against the local
  frame, and it is what lets a keyboard fly a ship.

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
exhaust, the rest would need hectares of radiator (see [Heat](#heat)). A ship that crosses an
astronomical unit in days at 1 g is therefore not known physics. It is exactly the part of _The
Expanse_ that is invented. What known physics offers:

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

### The options

1. **Known physics and time compression.** Transfers take days to months at milli-g and pass in
   seconds to minutes of play. A 28-day transfer lasts 24 seconds at 100,000×.
2. **An invented torch drive**, as _The Expanse_: gees of thrust at a high exhaust velocity, with
   the power and heat waved away.
3. **The jump drive crosses systems too.** It is already invented. If it can jump within a system,
   crossing one takes a jump, and the sublight legs are the approach and departure.
4. **Supercruise**, as Elite: a second faster-than-light mode that works only inside a system.

**Lean: options 1 and 3 together, under one principle: the jump drive is the only invented
technology.** Everything else is known physics, or engineering extrapolated from it, the register
the UX guide already sets for the consoles. Time compression is the concession to play, and it is
an honest one: it changes how fast the player lives through time, not what happens in it. A torch
drive would make the heat, power and propellant models decorative, and those are the systems this
phase exists to test.

### Engines

**Lean:** a craft carries two classes of engine, both real.

- A **manoeuvring engine**, high thrust and low exhaust velocity, chemical or nuclear thermal: for
  orbit insertion, rendezvous, docking and anything quick. Fighters are mostly this.
- A **cruise drive**, low thrust and high exhaust velocity, fusion: for transfers, flown by the
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

## The jump drive

The galaxy brainstorm settled that a drive can jump to any point within range, charted or not, and
that travel is free in three dimensions. Its rules are settled here.

### What limits range

**Lean: energy.** A jump draws on a charge that the reactor builds up in the drive. The energy
grows with the ship's mass and the distance, so range falls as the ship is loaded (Elite's lesson:
loadout is a navigation decision), and the recharge time ties the drive to power and heat. There is
no invented consumable.

Range per craft is a tuning number, but the galaxy says what it means. At a Sun-like radius the
generated density matches the local one, about 0.0023 systems per cubic light-year (Reylé et al.
2021), so the nearest neighbour is about 4 ly away, a 10 ly sphere holds about 10 systems, and a
20 ly sphere about 80. Density falls with height above the disc, so a short range makes the space
above the disc and the thin stretches between arms real obstacles, as the galaxy brainstorm
intends.

### Where a jump can start and end

**Lean: a tidal limit.** The drive cannot work where the tidal field of any body exceeds a
threshold τ: it needs G M ÷ r³ < τ. The tidal acceleration across a length L is about 2 G M L ÷ r³,
so the criterion says that the drive fails where space is too strongly curved across it. Because
G M ÷ r³ = (4π ÷ 3) G ρ (R ÷ r)³, the limit is a fixed number of radii for bodies of the same
density, which is Traveller's 100-diameter rule with a reason. Calibrated to 100 diameters at Earth,
τ ≈ 1.9 × 10⁻¹³ s⁻², and:

| Body                             | Limit           | In diameters |
| -------------------------------- | --------------- | ------------ |
| Moon                             | 294,000 km      | 85           |
| Mars                             | 606,000 km      | 89           |
| Earth                            | 1.27 million km | 100          |
| Jupiter                          | 8.7 million km  | 62           |
| 0.2 M☉ red dwarf                 | 0.35 AU         | 165          |
| 0.6 M☉ white dwarf               | 0.50 AU         | 4,300        |
| Sun                              | 0.59 AU         | 63           |
| 1.4 M☉ neutron star              | 0.66 AU         | 4 million    |
| Central black hole, 4.3 × 10⁶ M☉ | 96 AU           | 570          |

Diameters are the body's own, and the black hole's are of its event horizon. The red dwarf's radius
is the observed 0.22 R☉ at that mass (Boyajian et al. 2012); code takes radii from the stellar
model. The limits are for each body alone.

Consequences:

- The limit grows as the cube root of the mass and ignores density at a distance, so a neutron star
  is no worse than the Sun. The central black hole's is about a hundred astronomical units.
- Around the Sun, Mercury lies inside the Sun's limit and Venus outside it.
- A red dwarf's temperate planets orbit at under a tenth of an astronomical unit, well inside its
  limit. The commonest temperate worlds in the galaxy therefore always need a sublight leg of over a
  quarter of an astronomical unit, which takes weeks.
- A moon inside its planet's limit is reached only by a sublight leg from the planet's limit. The
  Moon, at 384,400 km, lies inside Earth's 1.27 million km, so its own limit never binds.
- The limits are checked against the whole tidal field, summed over bodies, at the departure point
  now and the arrival point at the arrival time. Summed, the fields push a limit out: the Sun's
  field at 1 AU is a fifth of τ, and moves Earth's limit out by about 8% along the line to the Sun.
  Bodies move, and the navigation computer knows where they will be.

τ is a free parameter and a candidate for a ruling. **Lean:** the drive refuses to jump inside a
limit, as an interlock the `JUMP` display shows, rather than misjumping.

### Velocity on arrival

A jump moves the ship to a different place in a gravitational field, so it changes the ship's
potential energy by m ΔΦ, where ΔΦ is the difference in potential between the two ends. What
happens to the velocity decides what happens to that energy. Four options:

1. **Velocity kept, energy free.** The ship leaves with the velocity it had, and the change in
   potential energy comes from nowhere. That is a perpetual-motion machine: fall from 10 AU to the
   Sun's limit, jump back out, and keep 51 km/s of excess speed, again and again. Not acceptable as
   it stands.
2. **Velocity kept, energy paid.** The drive's charge supplies m ΔΦ for a jump uphill and absorbs
   it for a jump downhill. A jump whose energy the charge cannot supply or hold is refused. Energy
   is conserved, and because the velocity is untouched in every frame, the rule does not depend on
   the frame it is stated in.
3. **Velocity matched to the destination, free.** The ship arrives at rest relative to the body it
   aimed for. Simple and generous: every jump is also a free change of velocity, and the galaxy's
   kinematics play no part in travel.
4. **Velocity chosen at an energy cost**, growing with the square of the change.

**Lean: option 2.** Its consequences:

- After a jump to another system the ship still has its old velocity, so it carries the difference
  between the two stars' velocities: tens of km/s between neighbours in the disc, and more for stars
  of the thick disc and the halo (Barnard's Star moves at about 140 km/s relative to the Sun). The
  galaxy's kinematics, generated per population, become part of route planning, and the navigation
  computer shows the relative velocity at arrival for every candidate.
- Between two similar stars the wells nearly cancel. The potential at a limit is
  (G M)^⅔ τ^⅓, 1.5 GJ/kg for the Sun, and the charge pays only the net difference.
- Within a system the kept velocity is a different orbit at the new radius, and the navigation
  computer plans jumps as it plans burns. A jump from Earth's orbit to Mars's distance keeps Earth's
  29.8 km/s where circular speed is 24.1 km/s, an orbit reaching 4.9 AU, and costs 0.31 GJ/kg, 6 TJ
  for a 20 t craft.
- The galaxy's own potential matters over long journeys. With a flat rotation curve of 230 km/s
  the potential changes by v_c² ln(R₂ ÷ R₁): about 20 MJ/kg per 10 ly of radius near the solar
  radius, small against a star's well, but about 10¹¹ J/kg from there to a kiloparsec from the
  centre, petajoules for a small craft. A craft heading coreward must bleed that energy off through
  its radiators between jumps, and one heading rimward must pay it from its reactor. The galactic
  centre is deep in a well, and reaching it and returning are real undertakings.
- Energy can never be released as heat at the moment of a jump. At hundreds of megajoules per
  kilogram it would vaporise the ship, so the charge's capacity is the limit.
- One loophole remains: jumps can turn the charge's energy into delta-v. A short outward jump
  followed by a coast reaches Mars for 1.8 km/s, against 5.6 km/s for a Hohmann transfer. A fall
  inward followed by a jump out turns the reactor's energy into speed without reaction mass. Both
  run at the reactor's power, and the second at the pace of a fall through a well, years at the Sun
  from 10 AU, so this is a slow propellant-free drive and not a free one. It may be acceptable as
  gameplay, or a reason to prefer option 3 or 4.

The risk of option 2 is that small craft cannot afford it. Option 3 is the simple fallback, at the
cost of the galaxy's kinematics.

### Duration, and what the sensors then see

**Lean:** a jump takes no universe time. The charge and the spool-up take real time, and the drive
must recharge afterwards. On arrival, the sensors see the destination's past light cone, which the
galaxy already models: after a 10 ly jump the pilot sees their point of departure as it was ten
years before, and a jump outruns any news. There is no faster-than-light communication.

### Gas and dust

The galaxy brainstorm left "what a faster-than-light drive does in gas" to this document.
**Lean:** nothing. The drive ignores the medium in transit, and the ship at each end meets what
physics says is there: extinction, and the erosion and radiation load of gas at speed.

## Ship systems

Every system publishes telemetry against limits, raises alerts from the server, and has procedures,
as the UX guide requires.

- **Power.** Reactor, batteries and capacitors, and solar power near a star. Electrical buses with
  priorities and load shedding, in the guide's own example: `EPS-2 BUS B UNDERVOLT: shed load or
start APU`.
- **Heat.** Everything a ship does ends as heat, which leaves only by radiation. A radiator at 1,000
  K sheds 57 kW per square metre per face, and at 300 K only 0.46. Radiators are large and fragile,
  deployed and stowed. Heat sinks absorb bursts, and coolant loops move heat between them. Starlight
  adds to the load: 1,361 W/m² at 1 AU from the Sun and 136 kW/m² at 0.1 AU, so a close pass by a
  star is a thermal problem. A ship's radiators are also its brightest feature in the infrared,
  which is how other sensors will find it.
- **Propellant.** Tanks with mass, position, pressure and boil-off. Delta-v remaining is computed
  from them.
- **Life support.** Oxygen, carbon dioxide scrubbing, water, cabin temperature and pressure, leaks,
  and the consumables of [The pilot](#the-pilot).
- **Avionics.** The flight control system, the navigation computer, the sensors' processors, with
  redundancy.
- **Structure.** Hull integrity and structural load limits.
- **Damage and failures.** Each component has a health and failure modes. Wear-out failures draw
  from the session's streams, and hazards damage what they reach: collisions, radiation from flares,
  heat, and gas at speed. **Lean:** a manual cold start is available, in the manner of DCS, and so
  is an automatic one, since depth must never be required on the first flight.

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
- **Mapping** from orbit needs the orbital-scale map generator that the galaxy brainstorm leaves to
  a later consumer. **Lean:** give it a brainstorm of its own, scheduled inside this phase, because
  maps are the payoff of exploration.

## The cockpit

### Displays

New displays, each to join the nomenclature list:

- `FLIGHT`: the primary flight display. Attitude against the chosen reference on an attitude ball,
  as Apollo's flight director attitude indicator, with rates, g-load, velocity in the reference
  frame, the flight control modes, thrust and delta-v remaining.
- `NAV`: the trajectory. The current path and its prediction dashed, planned burns in `--target`,
  osculating elements, and the times to periapsis, apoapsis, the next change of frame and the next
  jump limit.
- `JUMP`: the local chart of the `GALAXY` display with the range sphere, reachable systems in
  `--accent`, the relative velocity at arrival for each, the limits, the charge, and `ARM` and
  `EXECUTE`.
- `SENSORS`: contacts, measurements in progress and the discovery log.
- Systems displays, as schematics: power, thermal, propellant and life support.

`GALAXY`, `SYSTEM` and `LINK` stay as they are.

### The view outside

The galaxy brainstorm says HYPERION shows the galaxy through instruments, not a free camera, and the
UX guide forbids decoration. Elite is flown by looking out of the canopy. Four options:

1. **Instruments only.** Fly entirely on displays, as in instrument flight or a real rendezvous by
   radar and crosshairs. The purest option and the hardest to learn.
2. **The camera as an instrument.** A `CAMERA` display showing the ship's cameras, drawn from
   physics: apparent magnitudes and colours, extinction and reddening, positions at retarded time,
   and real exposure and field of view. Flight symbology such as a flight path marker and target
   brackets sits over it in the guide's grammar. Most of the time it shows points of light, and a
   planet becomes a disc only when the ship is near it, which is the truth. A camera is a sensor, so
   its image is a measurement and enters the Knowledge. The server sends the camera's detections and
   the client draws them, so the client never holds truth that the ship has not seen.
3. **Synthetic vision.** A 3D view drawn from the Knowledge, as in modern avionics: everything
   known, whether or not light would show it.
4. **A full out-of-the-window 3D cockpit**, as Elite. It needs a 3D renderer and contradicts the
   instruments principle.

**Lean: option 2.** It keeps the instruments principle and honest data, and it still gives the
pilot something to look at.

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

## Fighters and the path to the bridge

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

## Server and protocol

- Session messages: create, open, list and save sessions; set the time rate; subscribe to
  telemetry at chosen rates; a control input stream; commands with closed-loop results; alerts
  through the galaxy phase's notification path.
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
- **The jump drive.** Limits for isolated bodies against the table above, energy against mass and
  distance, velocity kept across a jump, and m ΔΦ exchanged with the charge, so that no cycle of
  falls and jumps gains energy the reactor did not supply.
- **Scenarios** as integration tests.
- **By feel.** The point of this phase is that a person flies the ship. The flight control gains and
  the displays will be tuned by hand, and the plan should budget for it.

## Decisions

Settled with the project owner on 2026-09-21:

- **The first playable experience is single-seat.** One player flies one small ship, in the manner
  of Elite Dangerous. The multi-position bridge comes after it.
- **The single-seat craft is reused.** The same definitions fly later as fighters and other small
  craft beside the main ship.
- **Its purpose is to test movement and the core systems** with one person before a crew divides
  them.

## Open questions

The first five shape everything else; the rest are detail.

1. **Does the realism ruling apply?** The galaxy was settled under "where a choice is open, the most
   realistic answer wins". **Lean:** yes, with the jump drive as the one invented technology and
   time compression as the concession to play.
2. **Travel within a system.** Known physics with time compression, an invented torch drive, jumps
   within a system, supercruise, or a combination. **Lean:** known physics with compression, plus
   jumps within a system.
3. **The jump drive.** What limits range (**Lean:** energy, growing with mass and distance); the
   well limit and its threshold τ (**Lean:** the tidal limit, calibrated to 100 diameters at Earth);
   velocity and energy on arrival (**Lean:** velocity kept, the charge paying or absorbing m ΔΦ,
   which makes the galactic centre expensive and leaves a slow propellant-free loophole); duration
   (**Lean:** none in universe time); gas in transit (**Lean:** ignored); refusal or misjump inside
   a limit (**Lean:** refusal).
4. **Range per craft.** A small craft's jump range sets how the galaxy feels: about 10 systems
   within 10 ly, about 80 within 20 ly.
5. **The view outside.** Instruments only, the camera as an instrument, synthetic vision, or a full
   3D cockpit. **Lean:** the camera as an instrument.
6. **The pilot's body.** Whether acceleration limits and loss of consciousness are modelled.
   **Lean:** yes, with an overridable g limit.
7. **Consumables and endurance.** **Lean:** modelled, weeks for a small craft.
8. **Where the player starts.** **Lean:** a home base as pinned content, in a system chosen by rule
   or by the player.
9. **Failure.** Restart from a save, or permanent loss.
10. **Refuelling.** **Lean:** skimming gas giants first; ice at a rendezvous later.
11. **Orbital maps.** In this brainstorm, or one of their own. **Lean:** their own, scheduled in
    this phase.
12. **Structure-borne sound.** **Lean:** allowed for real events, with an edit to the UX guide.
13. **Naming discoveries.** Whether the player can give proper names, as an overlay on catalogue
    designations.
14. **Pausing.** **Lean:** allowed in single-player, as a rate of zero under a banner.

## Suggested order of attack

Not a plan, only the dependency order a plan would follow:

1. Sessions, the loop at 64 Hz, persistence, and the local server started by the client, with a
   craft as a point mass on a trivial display. A thin vertical slice.
2. Gravity and propagation among the planetary stage's bodies, frames, time compression, and the
   `NAV` display with a predicted trajectory.
3. Attitude dynamics, thrusters, the flight control modes, the `FLIGHT` display and input devices.
4. Engines, propellant and delta-v, planned burns and the autopilot.
5. The jump drive: range, limits, velocity on arrival, and the `JUMP` display.
6. Power, heat and life support, their displays, and alerts with procedures.
7. Sensors over the Knowledge overlay, the exploration loop, and the camera.
8. Hazards, damage and failures.
9. Refuelling, the home base and docking.
10. Craft definitions for fighters, and the hooks a carrier will need.

**Lean, for the first milestone:** steps 1 to 3. A player flies a craft in orbit about a generated
planet, with time compression. It is the smallest thing that exercises the loop, the flight model
and the cockpit, and it answers the question this phase was chosen to answer: does flying feel
right?

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
- Bond, Martin et al. 1978, _Project Daedalus: The Final Report on the BIS Starship Study_, JBIS
  Supplement, S1–S192 (exhaust velocities of about 10,600 and 9,200 km/s for the two stages, seen
  here only through secondary sources).
- National Academies 2021, _Space Nuclear Propulsion for Human Mars Exploration_,
  doi:10.17226/25977 (specific mass and specific impulse of nuclear electric propulsion; nuclear
  thermal specific impulse).
- G tolerance in a human centrifuge, Scientific Reports 10 (2020) (9 g for 15 s with an anti-g suit
  and straining manoeuvre). <https://www.nature.com/articles/s41598-020-78687-3>
- Boyajian et al. 2012, _Stellar diameters and temperatures II_, ApJ 757, 112 (radii of M dwarfs).
- Winchell Chung, _Atomic Rockets_ (engine classes, radiators, torch drives).
  <https://www.projectrho.com/public_html/rocket/>
- Reylé et al. 2021, _The 10 parsec sample in the Gaia era_, A&A 650 (local density of systems).
- NASA Jupiter fact sheet (scale height, gravity, temperature at 1 bar).
  <https://nssdc.gsfc.nasa.gov/planetary/factsheet/jupiterfact.html>
- CODATA 2018 (the Stefan–Boltzmann constant); Kopp and Lean 2011, GRL 38 (total solar irradiance,
  1,361 W/m²).

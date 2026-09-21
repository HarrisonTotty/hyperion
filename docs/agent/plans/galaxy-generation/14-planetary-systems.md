# Plan 14: Planetary systems and the `SYSTEM` view

- **Milestone:** M5.
- **Depends on:** 06 (stars), 11 (multiplicity and binaries), 13 (substellar layers). Through them
  it also reads what plans 01–05, 08, 09 and 12 provide; see [Consumes](#consumes).
- **Brainstorm sections covered:** "Planetary systems"; "Hooks for the layers above"; the first
  paragraph of "Orbits and time" (Keplerian elements, position as a pure function of time, no
  N-body); the sentence of "Coordinates" on planetary systems stripped to the tidal radius; the
  sentence of "Planetary systems" and the bullets of "Between the stars" on brown dwarfs and rogue
  planets reusing this stage; "A system's own events" under "Events in time", applied to bodies; the
  body ID of "Identifiers"; "Knowledge" under "Overlays and persistence"; the planetary lines of
  "Testing" and "Runtime and code shape"; step 12 of "Suggested order of attack"; the "Reuse"
  paragraph of "The local chart in 3D" (its `⊕` remark is plan 13's).

## Goal

When this plan is done, every system in the galaxy, whatever its host, answers "what orbits this
star?" and "where is this moon at T+3 h?" as a pure function of seed, ID and time. A system's
planets come from a disc derived from its star, an architecture class whose frequency depends on
stellar mass and metallicity, and placement under dynamical constraints. Every other property of a
body is computed from those: radius, composition, temperature, atmosphere, rotation, moons, rings,
belts, a cometary halo, and the hooks the surface, life and civilisation generators will read.
Planets around binaries sit in the stable zones, planets around giants and remnants are what
survived, free-floating brown dwarfs and rogue planets have moons and bulk properties, and bodies
have events. The server answers system and body queries with records that degrade cleanly for the
Knowledge overlay, and the bridge client has a `SYSTEM` display: an orbit map on plan 05's spatial
view, a body list with readouts, and a time control.

## Scope and non-goals

In scope:

- `hyperion_sim::planetary`: disc, architecture classes, placement, derivation, small bodies, hooks,
  special hosts, events on bodies, the assembled generator and its queries.
- `hyperion_sim::orbit`: the Kepler element type and propagator, if plan 11 has not already put one
  in a shared place (see [Design notes](#design-notes), D2).
- The written definition of the architecture classes and their frequencies, with sources.
- Wire types and server handlers for system bodies, body detail and body events.
- The `SYSTEM` display and the edits to `docs/frontend/ux-guidelines.md` it needs. The drawn `⊕` is
  plan 13's, consumed here.
- Property, statistical and golden tests, and the benchmark of a full system.

Not in scope:

- Kilometre-scale global maps, terrain, local biomes. This plan provides the surface seed and the
  global figures only; the map generator is a later consumer.
- Life, species, civilisations. The habitability assessment is a physical statement about liquid
  water and nothing more. The coarse "habitable-world density per cell" summary that the
  civilisation brainstorm will want is not built here; D17 says what would serve it.
- The Knowledge overlay itself. This plan provides the detail levels and the degradation function;
  who has detected what is a later plan.
- Sensor models. The display shows the navigation computer's present state, as charts do. Retarded
  views of bodies are plan 12's mechanism applied by a later sensors plan.
- N-body dynamics, secular perturbation theory, resonant libration. Orbits are on rails.
- Interstellar comets and asteroids: a statistical density, left to the feature that needs them.
- Deltas (mined asteroids, stations) and pinned systems: overlays, stored by the `BodyId` this plan
  defines.

## Provides

### Additions to plan 11's `hyperion_sim::orbit`

Plan 11 provides `orbit::{KeplerElements, Eccentricity, solve_kepler}` for bound orbits and says
plan 14 reuses them. This plan adds, without touching plan 11's output:

```rust
// `units::GravitationalParameter` (m³ s⁻²) is new in this plan, beside plan 01's `GM_*` constants.
impl KeplerElements {
    pub fn from_semi_major_axis(a: Metres, mu: GravitationalParameter, ..) -> Self;
    pub fn scaled(&self, factor: f64, mu: GravitationalParameter) -> Self;   // D11 expansion
}
pub struct OpenOrbit { /* pericentre: Metres, eccentricity >= 1 - 1e-4, angles,
    time_of_pericentre: UniverseTime, mu */ }
impl OpenOrbit {
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity);
}
pub fn solve_kepler_hyperbolic(mean_anomaly: f64, e: f64) -> f64;
pub fn solve_barker(mean_anomaly: f64) -> f64;                           // parabolic
pub fn elements_from_state(r: SystemVector, v: SystemVelocity, mu: GravitationalParameter,
    t: UniverseTime) -> Result<KeplerElements, OpenOrbit>;               // D11 supernovae
```

### `hyperion_sim::planetary`

```rust
// index.rs — layout of the u16 inside plan 01's BodyId (D3)
pub struct BodyIndex(u16);                       // (slot: u8) << 8 | (sub: u8)
pub enum BodySlot { Stellar, Planet(u8), Belt(u8), SecondGeneration(u8) }
pub enum BodySub { Primary, Component(u8), Moon(u8), Ring(u8), Member(u8) }
impl BodyIndex { pub fn new(slot: BodySlot, sub: BodySub) -> Result<Self, EncodeBodyIndexError>;
                 pub fn decode(raw: u16) -> Result<(BodySlot, BodySub), DecodeBodyIndexError>; }

// context.rs — everything the stage reads from the stages above
pub struct SystemContext { /* id, host kind, stars with tracks, hierarchy, [Fe/H], [alpha/Fe],
                              age at epoch, tidal radius, encounter environment */ }
impl SystemContext {
    pub fn for_system(galaxy: &Galaxy, id: SystemId) -> Result<Self, ResolveSystemError>;
    pub fn builder() -> SystemContextBuilder;    // synthetic hosts for tests and tools
}
pub enum HostKind { Stellar, BrownDwarf, RoguePlanet }

// system.rs — the generator
pub fn generate(seed: Seed, ctx: &SystemContext) -> PlanetarySystem;       // plan 01's `Seed`
pub fn generate_planets(seed: Seed, ctx: &SystemContext) -> PlanetarySystem; // no moons
// satellites.rs
pub fn generate_satellites(seed: Seed, ctx: &SystemContext, planet: &Body) -> Satellites;
pub struct PlanetarySystem { /* per host: disc, class, zone; bodies sorted by BodyIndex; belts;
    halo. Primordial state only (D1) */ }
impl PlanetarySystem {
    pub fn disc(&self, host: OrbitHost) -> Option<&Disc>;
    pub fn architecture(&self, host: OrbitHost) -> ArchitectureClass;
    pub fn zones(&self) -> &[OrbitZone];
    pub fn bodies(&self) -> &[Body];
    pub fn body(&self, index: BodyIndex) -> Option<&Body>;
    pub fn children(&self, index: BodyIndex) -> impl Iterator<Item = &Body>;
    pub fn belts(&self) -> &[Belt];
    pub fn halo(&self) -> Option<&CometaryHalo>;
    pub fn snapshot_at(&self, ctx: &SystemContext, t: UniverseTime) -> SystemSnapshot;
    pub fn body_at(&self, ctx: &SystemContext, index: BodyIndex, t: UniverseTime)
        -> Result<BodyRecord, ResolveBodyError>;
    pub fn position_at(&self, ctx: &SystemContext, index: BodyIndex, t: UniverseTime)
        -> Result<SystemPosition, ResolveBodyError>;       // system frame, plan 01 `coords`
    pub fn events_between(&self, ctx: &SystemContext, from: UniverseTime, to: UniverseTime)
        -> Vec<BodyEvent>;
    pub fn habitable_zone_at(&self, ctx: &SystemContext, host: OrbitHost, t: UniverseTime)
        -> Option<HabitableZone>;
}

// the pieces, each usable alone
pub mod disc { pub struct Disc; pub fn derive(..) -> Disc;
    pub fn snow_line(l: Luminosity) -> Metres; }
pub mod architecture { pub enum ArchitectureClass { Barren, TerrestrialOnly, CompactMulti,
    CompactWithColdGiant, SolarLike, EccentricGiant, WarmGiant, HotJupiter, SubstellarCompact };
    pub struct ClassWeights; pub fn class_weights(m: SolarMasses, feh: FeH) -> ClassWeights;
    pub const ARCHITECTURE_TABLE: &[ClassRow]; }
pub mod placement { pub fn mutual_hill_radius(..) -> Metres; pub fn spacing_floor(..) -> f64;
    pub struct OrbitZone; pub enum OrbitHost { Star(u8), Pair(u8), Barycentre, Body(BodyIndex) }
    pub fn stable_zones(h: &SystemHierarchy) -> Vec<OrbitZone>;
    pub fn holman_wiegert_s_type(mu: f64, e: f64) -> f64;
    pub fn holman_wiegert_p_type(mu: f64, e: f64) -> f64; }
pub mod derive { pub fn radius_chen_kipping(..); pub fn radius_zeng(..); pub fn composition(..);
    pub fn equilibrium_temperature(..) -> Kelvin; pub fn habitable_zone(l, teff) -> HabitableZone;
    pub fn jeans_parameter(..) -> f64; pub fn tidal_locking_time(..) -> Seconds;
    pub fn roche_limit_fluid(..) -> Metres; pub fn roche_limit_rigid(..) -> Metres;
    pub fn hill_radius(..) -> Metres; pub fn satellite_stability_limit(..) -> Metres; }
pub mod hooks { pub struct BodyHooks { /* surface_seed, bulk, surface, habitability, resources,
    figures */ } pub struct BulkComposition; pub struct SurfaceConditions;
    pub struct HabitabilityAssessment; pub struct ResourceAbundances; pub struct GlobalFigures; }
pub mod fate { pub enum BodyState { NotYetFormed, Present, Destroyed { cause: DestructionCause,
    at: UniverseTime }, Unbound { at: UniverseTime } } }

// record.rs — what a query returns, and its degradation
pub struct BodyRecord { /* id, label, kind, parent, state, orbit, bulk, surface, hooks */ }
pub enum DetailLevel { Contact, MassAndOrbit, Bulk, Surface, Full }   // ordered
impl BodyRecord { pub fn degrade(&self, level: DetailLevel) -> BodyRecord; }
pub enum BodyKind { Planet(PlanetClass), DwarfPlanet, Moon(MoonOrigin), Ring, Belt(BeltKind),
    CometaryHalo, ProtoplanetaryDisc, DebrisDisc,
    Unresolved }                                  // only ever produced by `degrade(Contact)`
```

Domain tags, as entries of plan 01's single `domain_tags!` registry in `rng/tags.rs` under a "Plan
14" heading, never renamed. Plan 01 requires at least one full stop in a name and one scope per tag,
so the first draft's `ring`, `belt` and `halo` are `ring.system`, `belt.population` and
`cometary.population` (not `halo.`, which is the galactic halo's prefix in plans 02 and 08).

- Scope `System` (opened with `ObjectKey::from(SystemId)`, the host and slot in the draw number,
  D4): `planet.disc`, `planet.plane`, `planet.class`, `planet.count`, `planet.spacing`,
  `planet.mass`, `planet.secondgen`, `belt.population`, `cometary.population`.
- Scope `Body` (opened with `ObjectKey::from(BodyId)`): `planet.orbit`, `planet.radius`,
  `planet.volatiles`, `planet.spin`, `planet.origin`, `moon.count`, `moon.mass`, `moon.orbit`,
  `moon.impact`, `moon.capture`, `ring.system`, `belt.member`, `body.surface`, `body.resources`.
- Scope `Event`, each also an entry of plan 01's `event_tags!` in the block 0x0400–0x04FF that plan
  06 sets aside for this plan: `0x0400 BODY_IMPACT` (`body.impact`), `0x0401 BODY_ERUPTION`
  (`body.eruption`), `0x0402 BODY_STORM` (`body.storm`), `0x0403 BODY_DUSTSTORM` (`body.duststorm`),
  `0x0404 SYSTEM_COMET` (`system.comet`).

Every stream is `Stream::open(seed, tag, key)`. A `u16` body index fills plan 01's 16-bit `sub`
field of the second counter word exactly (`sub << 48 | block`), leaving 48 bits of draw number.

Test helpers:
`planetary::testing::{synthetic_star, synthetic_binary, sample_contexts, solar_system_bodies}`
behind the `testing` feature and `cfg(test)`, in the way plan 06 exposes `stellar::testing`.

### Protocol (`hyperion-protocol`, mirrored in `@hyperion/protocol`)

In plan 04's envelope (`ClientMessage::Request { id, body }`), with the `Dto` naming of plans 06 and
11:

- `RequestBody::SystemBodies(SystemBodiesRequest { universe, system, time, detail })` and
  `ResponseBody::SystemBodies(SystemBodiesDto)`, kind `system_bodies` (reserved by plan 04).
- `RequestBody::BodyDetail(BodyDetailRequest { universe, body, time, detail })` and
  `ResponseBody::BodyDetail(BodyDetailDto)`, kind `body_detail` (reserved by plan 04).
- `RequestBody::BodyEvents(BodyEventsRequest { universe, system, from, to })` and
  `ResponseBody::BodyEvents(BodyEventsDto)`, kind `body_events` (reserved by plan 04 with the other
  two); it is added by the procedure of plan 04's "Extending the convention" (a variant of each body
  with the same `kind`, the string in `REQUEST_KINDS`, a wire-form test for each, a handler,
  `just gen-protocol`), which needs no change to plan 04's code or to the TypeScript client.
- `BodyIdHex`, a string in plan 01's form for a `BodyId`: 16 hex digits, a full stop, 4 hex digits.
  `DetailLevelDto`, `BodyOrbitDto` (plan 11's `OrbitDto` plus `parent`, `mu_m3_s2` and
  `valid_until`), `BodySummaryDto`, `BeltDto`, `ZoneDto`, `HabitableZoneDto`, `BodyHooksDto`,
  `BodyEventDto`, `BodyStateDto`, `BodyKindDto`.
- `ErrorCode::UnknownBody` (`unknown_body`), beside plan 06's `ErrorCode::UnknownSystem`.
- In `@hyperion/protocol`: `parseBodyId` and `formatBodyId`. Plan 04's generic `request` needs no
  change.

### Client (`apps/hyperion`)

- `"system"` in plan 05's `DisplayId` union (reserved there for this plan) and
  `displays/system/SystemDisplay.tsx`.
- `lib/orbit.ts`: `solveKepler`, `positionAt`, `orbitPolyline`, `composePosition`, pure functions.
- In plan 05's `spatial/`, all additive: in `marks.ts` the mark kinds `PathMark` and `AnnulusMark`
  and two optional members of `SpatialScene`, `paths` and `annuli`; in `symbols.ts` the
  `SymbolShape` values `pentagon` and `hexagon`; in `frame.ts`
  `planeFrame(normal, reference): LocalFrame`, which builds the tilted frame of D21 (plan 05's
  plane, grid, stalks, presets and fill rule all follow `scene.frame`, so `PlaneSpec` itself needs
  no change); in `AxisTriad.tsx` an optional `axes` prop, three labelled unit vectors, which
  defaults to the scene frame's.
- In `lib/format.ts`: `formatPeriod`, `formatPressure`, `formatGravity`, `formatTemperatureK`,
  `formatBodyDistance`, `formatUniverseTimeDhms`. Planetary masses use plan 13's `formatMassMearth`
  and `EarthMassUnit`, and a brown dwarf's plan 13's `formatSubstellarMass`.

## Consumes

Names are those of the neighbouring plans' "Provides" as written. P14.T1.a reconciles them with the
code that exists when this plan's turn comes.

- **Plan 01, determinism foundation.** `math`;
  `rng::{Seed, Stream, DomainTag, TagScope, ObjectKey, domain_tags!, EventKey}` with
  `Stream::open(Seed, DomainTag, ObjectKey)`, `impl From<BodyId> for ObjectKey` (counter word 1 is
  `sub << 48 | block`, with the `body_index` as `sub`), `seek` and `word_at` for draws addressed by
  slot, the samplers and integer-threshold decisions; `units`, including `EarthMasses`,
  `JupiterMasses` and `units::consts`; `time::{UniverseTime, CLOCK_WINDOW_H}`; `coords` (system and
  body frames, which are translations with galactic axes; rotating body-fixed frames are left to
  this plan); `id::{SystemId, BodyId, EventId, EventSubject, Designation}` and the `event_tags!`
  registry (every `u16` is a valid body index there, and its meaning is this plan's);
  `GENERATOR_VERSION`; `hyperion-testkit` (the golden harness, `stats`, order independence),
  slow-test marking, `just test-slow`, `just bench`.
- **Plan 02, galaxy model.** `galaxy::Galaxy`; `PotentialTables::tidal_radius(m, &PointLy)`.
- **Plan 03, placement.** `placement::{SystemRecord, resolve}`, `ResolveSystemError`.
- **Plan 04, server and protocol.** The request envelope, `RequestBody`, `ResponseBody`,
  `REQUEST_KINDS`, `ErrorCode`, `SystemIdHex`, the wire `UniverseTime`;
  `compute::{CpuPool, SingleFlight}`; `cache::{ByteLru, SharedByteLru, HeapBytes}`; the TypeScript
  `request`.
- **Plan 05, `GALAXY` display.** `lib/displays.ts` (`DISPLAYS`, `DisplayId`, which it reserves for
  this plan to extend) and the navigation bar; `useServerRequest`, `RequestState` and
  `RequestStatus`; `spatial/` (`SpatialView` with its `scene`, `fitRadius`, `formatLength` and
  `frameName` props, `marks.ts` with `SpatialScene`, `PointMark`, `PlaneSpec` and `SymbolShape`,
  `frame.ts` with `LocalFrame`, `camera.ts`, `symbols.ts`, `drawList.ts`, `pick.ts`, `scale.ts`,
  `redraw.ts`, `AxisTriad.tsx`, whose lengths are in the scene's unit with a formatter);
  `lib/format.ts`; `UnitLabel`; the list-plus-canvas selection pattern, `formatUniverseTimeYr` and
  `TIME_SYSTEM_LABEL`; the guide's 3D spatial display conventions (P05.T2.e) and its units `yr`,
  `Myr` and `Gyr`.
- **Plan 06, stars.** `stellar::system::{SystemStars, StarModel, draw_metallicity}`; `Composition`;
  `Track::{state_at, lifetime, death, max_radius_until, max_luminosity_until}`; `StarState` (phase,
  mass, luminosity, radius, effective temperature); `remnant::{Death, DeathKind, NatalKick}` and
  `SystemStars::natal_kick`; `rotation::ActivityLevel`; `stellar::substellar::cooling`; the generic
  `events::{PoissonBins, MonotonePhase, RateModel, PhaseClock, LinearClock, TimeWindow}`;
  `math::normal_quantile`; `SystemSummaryDto` for the hosts on the wire; the `starSymbols.ts`
  registry. The zero-age main-sequence luminosity and radius of D6 are its `zams::luminosity` and
  `zams::radius` (P06.T4.c), and `ErrorCode::UnknownSystem` is its P06.T33's. Still needed and
  settled in T1.a: an [α/Fe], and an X-ray and ultraviolet history from `ActivityLevel`.
- **Plan 08, velocities and kicks.** Nothing beyond the `NatalKick` plan 06 exposes.
- **Plan 09, features.** A system's sphere of influence (the smaller of the galactic tidal radius
  and the feature's, and the pericentre rule of P09.T28.c for the Kepler regime); for a feature
  member, the local number density, velocity dispersion and mean member mass from the feature's
  class profiles.
- **Plan 11, multiplicity.** `orbit::{KeplerElements, Eccentricity, solve_kepler}`; in
  `stellar::multiplicity`: `SystemHierarchy`, `HierarchyNode`, `StarSlot`, `star_positions_at`,
  `STAR_BODY_INDEX_END`, and `StarSlot`'s kind, which tells a brown-dwarf companion from a star;
  `coords::{SystemVector, SystemVelocity}`; `BinaryState` for evolved pairs; `OrbitDto`,
  `HierarchyDto`. Every component of the hierarchy is plan 11's body, brown-dwarf companions
  included: this plan generates no body in slot `0x00` of a system that has a star, and reads such a
  companion as one more component that bounds stable zones and may host one (D3, D10).
- **Plan 12, retarded observation.** Nothing at build time. Its note that degraded body records
  arrive with this plan is met by T34.
- **Plan 13, substellar layers.** The free-floating brown dwarf and rogue planet records,
  `SystemRecord::kind() -> SystemKind`, from which `HostKind` converts, and the body-0 convention
  (the object is body `0x0000`, its moons `0x0100` upward, its rings `0x0080`–`0x008F`); brown dwarf
  state through plan 06's cooling fit; `stellar::substellar::giant_cooling`, returning a
  `CoolingState`, for 0.3–13 M_Jup, which plan 13 builds and this plan only calls; for a rogue
  planet, the record and metallicity with no derived state, which this plan supplies; `EarthGlyph`,
  `EarthMassUnit`, `formatMassMearth`, `formatSubstellarMass`, the `triangle-down` symbol, and the
  guide's entry for M⊕ (every planetary mass is in M⊕; there is no Jupiter-mass unit on the
  consoles).

## Design notes

None of these contradicts the brainstorm; where one reads a sentence of it narrowly (D6, D7, D14)
the note says so and the reading is reported to the plan's commissioner. The literature named here
and in the tasks was cited from memory, without access to the papers. Each note that rests on it
ends with what to re-check, and no constant is committed before that check.

**D1. Draws are primordial; time enters through a fate transform.** The brainstorm requires that
draws never depend on time, and is silent on planets of evolved stars. So a system is always
generated as it was born, from its hosts' zero-age properties, and a closed-form transform of the
host's track and the clock gives each body's state at the queried time: not yet formed, present
(with evolved orbit and surface conditions), destroyed, or unbound. Body indices are permanent. A
destroyed planet resolves at every time, to "destroyed, cause, when", exactly as an unborn system
resolves to "no system yet". An observer at retarded time still sees it alive. Details in D11.

**D2. One Kepler module.** Plan 11 owns `orbit` and its bound-orbit solver, and this plan uses them
unchanged for planets and moons, so a planet and a companion star are propagated by the same code.
Comets need eccentricities of 1 and above, which plan 11's `Eccentricity` rightly refuses, so open
orbits are a separate type added beside it (T2).

**D3. Layout of `body_index`.** Plan 01 owns `BodyId = (SystemId, u16)`. This plan fixes the `u16`
as `slot << 8 | sub`, decodable without generating anything:

| Slot          | Meaning                                 | Sub                                                                                                                                                                                                                                               |
| ------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0x00`        | The stellar level                       | `0x00`–`0x0F` plan 11's components (plan 06's primary at `0x00`, companions, brown-dwarf companions included, from `0x01`); in a free-floating brown dwarf's or rogue planet's system `0x00` is the object itself and `0x80`–`0x8F` are its rings |
| `0x01`–`0xBF` | Primordial planets, in generation order | `0x00` the planet, `0x01`–`0x7F` moons, `0x80`–`0x8F` rings, rest reserved                                                                                                                                                                        |
| `0xC0`–`0xCF` | Second-generation planets (D11)         | as for planets                                                                                                                                                                                                                                    |
| `0xE0`–`0xEF` | Belts, discs and the cometary halo      | `0x00` the population, `0x01`–`0xFF` its named members                                                                                                                                                                                            |
| other         | Reserved                                | zero                                                                                                                                                                                                                                              |

Generation order is host by host in hierarchy order, then inside out, so an index never depends on a
later stage. Slot `0x00` is the raw values 0–15, which is plan 06's "index 0 is the primary", plan
11's "stars take body indices 0 to 15 in hierarchy order" and its `STAR_BODY_INDEX_END` = 16: this
plan numbers nothing there and generates no stellar-level body. Plan 13 states the same convention
for free-floating objects: the object is body `0x0000`, a rogue planet's moons are the planet-level
slots `0x01` upward of its system (`0x0100`, `0x0200`, …), since the object is that system's stellar
level, and its rings are slot `0x00`, sub `0x80` upward. A brown dwarf's planets take slots `0x01`
upward like a star's. Unused values are rejected by `BodyIndex::decode`, although plan 01's `BodyId`
accepts any `u16`. The whole `u16` is the `sub` field of plan 01's second counter word, so every
body has streams of its own and none can collide with its system's.

**D4. System draws and body draws are separate streams.** Architecture, counts, spacings and masses
are drawn on system-level streams with the planet's slot in the draw counter, because they are joint
properties. Everything that belongs to one body (radius quantile, volatiles, spin, moons, rings,
surface seed, resources) is drawn on that body's own streams keyed by `BodyId`. Adding a property to
moons can then never move a planet, and `generate_satellites` for one planet equals the same
planet's satellites inside `generate`.

**D5. The disc is a budget and a ruler, the class table is the frequency.** The brainstorm's
approach C takes class frequencies from observation and everything else from the star. The disc (gas
mass, solid mass, outer radius, lifetime, snow line) therefore never decides the class. It scales
what the class places: the characteristic planet mass, the reach of the system, the mass of belts
and the halo, and whether the class's giants can exist at all. A class whose needs the disc cannot
meet (a giant where the stable zone ends inside the snow line, or where the solid budget is under 10
M⊕) falls back to its giant-free sibling, and the class frequencies quoted in tests are measured
after the fallback.

**D6. The snow line uses the host's zero-age main-sequence luminosity.** The brainstorm gives 2.7 au
× √(L ÷ L☉) and does not say which L. It cannot be the present one: the snow line decides what
formed where, which is primordial, draws may not depend on time (D1), and present luminosity would
put rocky planets at 30 au around a red giant. The zero-age main-sequence value is the one fixed
luminosity every star has. For a substellar host, which has no main sequence, L is the cooling fit's
luminosity at 10 Myr. The coefficient is Hayashi's (1981), set with the present Sun, so a 1 M☉ host,
0.7 L☉ at zero age, gets 2.3 au, inside the 15% by which published normalisations differ. Recorded
under risks, because pre-main-sequence stars are brighter. Re-check: Hayashi (1981) for the
coefficient; Kennedy and Kenyon (2008) for how far the pre-main-sequence snow line moves.

**D7. Spacing floors.** The brainstorm centres spacing "on the observed 14–20" and rejects it "below
the long-term stability floor of about 10–12". Both figures are measurements of Kepler's small
planets (Weiss et al. 2018; Pu and Wu 2015), and the floor is kept exactly as written for the
population it was measured on. Applied to pairs that include a giant it would forbid the
brainstorm's own Solar-like class, since Jupiter and Saturn sit 8.0 apart, so for those pairs the
floor is the corresponding stability result for giants. So: for a pair of planets both under 0.1 M_J
the floor on (a₂ − a₁) ÷ R_H is 10 for circular orbits rising to 12 with eccentricity (10 + 100 ×
mean e, capped at 12); for a pair with a giant it is 7 (Chambers et al. 1996; Marzari and
Weidenschilling 2002); and for every pair the gap between the inner apocentre and the outer
pericentre is at least 2√3 R_H, Gladman's (1993) two-planet Hill stability applied at closest
approach. The last rule is what makes "no overlapping orbits" a theorem of the generator. Re-check:
the 10 and 12 against Pu and Wu (2015); the 7 against Chambers et al. (1996), Marzari and
Weidenschilling (2002) and Chatterjee et al. (2008), which is the least certain number here; 2√3
against Gladman (1993).

**D8. Radius: one quantile, then composition, then escape.** "Radius from mass, refined by
composition" is read as follows. A body draws one uniform quantile on its `planet.radius` stream,
which places it within Chen and Kipping's scatter at its mass. That quantile is the one number in
the derivation that is drawn, and it stands for the composition that the class does not fix; the
brainstorm's "computed, not rolled" holds in that radius, composition and envelope are never drawn
independently of each other. Zeng et al.'s curves (pure iron, Earth-like, pure rock, 50% and 100%
water, and hydrogen envelopes) then turn that (mass, radius) point into a composition, constrained
by where the body formed: a body formed inside the snow line cannot be a water world, so its radius
is clamped to the rock curve or given an envelope. The escape step may then strip the envelope, and
a stripped body takes the Zeng radius of its core. The radius valley near 1.8 R⊕ is a result of this
and is tested, not imposed: the initial radii are unimodal, and the gap opens because close-in
envelopes are lost whole (Owen and Wu 2017). Re-check: Chen and Kipping (2017), table 1, for the
breaks, exponents and scatters; Zeng et al. (2019) for the curves; Fulton et al. (2017) for where
the valley sits.

**D9. Secular change is closed-form and non-crossing.** Tidal circularisation, tidal recession of
giant-impact moons, despinning, envelope loss and adiabatic orbit expansion are all evaluated in
closed form at age plus clock time. Each is monotone and keeps a body inside the interval its
spacing was checked on (circularisation at constant angular momentum ends at a(1 − e²), which lies
between the original pericentre and apocentre; expansion under stellar mass loss scales every orbit
of a host by the same factor), so the property tests hold at all times.

**D10. Planets in multiple systems.** Stable zones come from Holman and Wiegert's fits, applied at
every level of plan 11's hierarchy: around a single component out to the S-type limit set by its
nearest companion at that level, and around a pair from the P-type limit outward, itself bounded by
the next level up. Each zone is an independent orbit host with its own disc share and class draw. A
disc truncated inside its snow line cannot make giants (D5), and hosts in binaries closer than about
50 au draw `Barren` with an added probability so that the occurrence is about a third of a single
star's (Kraus et al. 2016). For a binary that plan 11 has evolved, the zone is the intersection of
the zones at birth and at the evaluated state. A brown-dwarf companion is one of plan 11's
components: it bounds zones through the same fits (clamped at μ = 0.1), and its own zone, if wide
enough, is a host on the `SubstellarCompact` row of D13. Re-check: Holman and Wiegert (1999) for the
coefficients; Kraus et al. (2016) for the 50 au and the factor of three.

**D11. Evolved stars and remnants: what survives.** The most realistic treatment that stays
closed-form:

- _Expansion._ Stellar mass loss is slow against any planetary period inside about 1,000 au, so
  orbits expand adiabatically: a(t) = a₀ × M₀ ÷ M(t), eccentricity unchanged. The cometary halo is
  not adiabatic and loses a mass-loss-dependent share (Veras et al. 2011).
- _Engulfment._ A planet is destroyed at the first age at which a(t) < f × R★(t), with f = 2 for a
  rocky planet rising to 3 for a Jovian one, because tides drag in planets from beyond the
  photosphere (Mustill and Villaver 2012). That needs the largest stellar radius before a given age,
  a monotone helper asked of plan 06.
- _Scorching._ Surface and atmosphere derivations read the largest luminosity before the age as well
  as the present one, so a survivor at 3 au of a white dwarf has lost its volatiles.
- _Supernovae._ Mass loss is instantaneous. The planet's orbital phase at the death time is known in
  closed form, the star's velocity changes by the kick, and the new elements follow from the
  two-body energy and angular momentum. Nearly every planet of a neutron star is unbound, since the
  star loses more than half its mass. A black hole formed by complete fallback, with no kick and
  little loss, keeps its outer planets, those beyond the supergiant's reach.
- _White dwarfs._ Survivors sit at a₀ × M₀ ÷ M_wd, typically beyond 5 au. A white dwarf with a
  surviving belt and at least one planet is polluted with a probability fitted to the observed
  25–50% (Koester et al. 2014), and carries a dusty disc inside its own Roche limit in 1–4% of
  cases, falling with cooling age (Farihi 2016). The disc is a body in a belt slot.
- _Second-generation planets._ A neutron star draws a fallback-disc system with probability 0.005, a
  parameter of the generator version (Niţu et al. 2022 bound pulsar planets to under about half a
  per cent, so 0.005 is that upper bound and may only come down): one to three terrestrial-mass
  bodies at 0.2–0.5 au, as PSR B1257+12 has, appearing 10 Myr after the death, in the
  second-generation slots. The draw is fixed by the ID like any other; time enters only as the
  threshold "10 Myr after the death".
- _Unbound planets_ are not tracked. The rogue planet layer already counts them statistically, as
  escapers need no population of their own.

Re-check: Veras et al. (2011) for the adiabatic limit and the halo's loss; Mustill and Villaver
(2012) for f; Koester et al. (2014) and Farihi (2016) for the white dwarf fractions; Niţu et al.
(2022) for the pulsar-planet bound.

**D12. Young hosts.** A disc lives for a drawn time of a few million years. Before it, the system
holds a protoplanetary disc in a belt slot. Giants exist from a formation age drawn below the disc
lifetime, small planets from the disc lifetime, and terrestrial planets carry a magma-ocean surface
state until 10–100 Myr. Debris belts are bright when young and fade as 1 ÷ age (Wyatt 2008).

**D13. Substellar hosts.** A brown dwarf runs the stellar path with the class table's
`SubstellarCompact` row: no giants, a compact chain or nothing, the disc scaled to its mass. A rogue
planet runs the satellite path only, as a planet without a star. One that is under a Jupiter mass is
taken to have been ejected and keeps only moons inside a tenth of the Hill radius it had on a drawn
birth orbit (Hong et al. 2018, to re-check); heavier ones formed alone and keep everything. Only
free-floating objects come here: a brown dwarf bound to a star is plan 11's component, and D10
treats it as one. With no star, temperatures come from internal heat: plan 13's cooling fits for
giants, radiogenic and residual heat in closed form for rocky bodies.

**D14. Tidal stripping uses the same stability limit as moons.** The brainstorm says a system near
the central black hole is stripped to its tidal radius at pericentre. That is an outer bound, and it
is kept. Nothing prograde is stable out to a full Hill radius, and the brainstorm's tidal radius
reduces to the Hill radius R × (m ÷ 3M)^⅓ about a point mass, so bodies are generated only with
apocentres inside `SATELLITE_STABILITY_FRACTION` = 0.49 of it (Domingos et al. 2006; re-check the
0.4895 and its eccentricity terms), and the property tests assert the weaker statement the
brainstorm makes: nothing beyond the radius itself. The same rule cuts the cometary halo of every
system at 0.49 of its sphere of influence, and the same function bounds moons inside Hill spheres.
In a dense feature a second cut applies: the semi-major axis at which the time to a disrupting
encounter, 1 ÷ (n σ_cs v) with gravitational focusing, equals the system's age.

**D15. Events on bodies.** Five tags, all on the two constructions of plan 06: impacts and eruptions
as Poisson bins with rates from the small-body populations and from tidal heat, giant storms and
global dust storms as monotone phases with the orbital period as P, and comet apparitions as Poisson
bins on the system's key. A comet is identified by its `EventId`, not a body index, and its
near-parabolic orbit is a function of time like any other. No event leaves a mark: a crater made
during play is below the resolution of the global figures.

**D16. Records degrade by dropping sections.** `BodyRecord` is nested so that each `DetailLevel` is
a prefix: `Contact` (identity, kind unknown, position), `MassAndOrbit`, `Bulk` (radius, density,
class, equilibrium temperature), `Surface` (atmosphere, surface conditions, rotation, figures),
`Full` (hooks: seed, composition, habitability, resources). `degrade` clears sections and never
blurs a number; measurement noise is the sensor plan's business. Until the Knowledge overlay exists
the server grants whatever level a request asks for, and says which level it granted.

**D17. No analytic habitability summary yet.** The civilisation brainstorm will want habitable
worlds per large cell from the fields. The class table and the habitable-zone formulae make that a
two-dimensional quadrature over stellar mass and metallicity, which can be tabulated offline later
without touching this plan's output.

**D18. The client propagates orbits for drawing.** The server sends elements, and the display
evaluates Kepler's equation itself to animate. That is drawing, not generation: every number in a
readout comes from the server, and the display re-requests the system when its time has moved more
than a year or past a body's `valid_until`. The TypeScript solver need not match `libm` bit for bit.

**D19. Units on the display follow plan 13.** Every planetary mass, from a moon to a 13 M_Jup giant
(`4,131 M⊕`), is in M⊕ with plan 13's formatter and drawn glyph, because the guide wants one unit
per quantity everywhere on the ship; a brown dwarf's is in M☉, as on the chart. There is no
Jupiter-mass unit. Radii are in km, which needs no new unit.

**D20. Property tests without a new dependency.** "For any seed and ID" is a loop over a fixed
sample of seeds and contexts drawn with plan 01's own generator, several thousand in the ordinary
suite and a million under `just test-slow`. No `proptest`.

**D21. The orbit map's reference plane is the system's, its triad the galaxy's.** Plan 01's system
frame is a translation of the galactic axes, so planetary planes are tilted at random to it. An
orbit map drawn on the galactic plane would show every system as a fan of stalks. The map's
reference plane, grid and `TOP` preset therefore use the primary host's planetary plane (for a close
binary, the binary's), the frame label says so (`SYSTEM PLANE`), and the axis triad still points
galactic north, coreward and spinward so that the operator can relate the map to the chart. Plan
05's view needs no new concept for this. Its plane, grid, stalks, fill rule and presets all follow
`scene.frame`, a `LocalFrame` of three orthonormal vectors, so the orbit map passes a frame whose
`north` is the plane's normal and whose `coreward` is galactic coreward projected onto the plane
(`planeFrame`, T40), and hands the true galactic directions to `AxisTriad` separately.

**D22. Designations and labels.** Plan 01's designation of a body, the system's designation plus
` /<body index>`, stays the designation of record, because it parses back to the ID. This plan adds
a `BodyLabel` for people: host letter, planets lettered from `b` by semi-major axis, moons in Roman
numerals, belts numbered. Labels are derived, not stored, and proper names are an overlay.

**D23. Body-fixed frames.** Plan 01 leaves rotating frames to this plan. A body's fixed frame is its
pole (from obliquity and a drawn azimuth) and a prime-meridian angle W(t) from T14's rotation law;
for a locked body the prime meridian faces the primary at pericentre. That is enough for a later
surface plan to place a map on the sphere.

**D24. Time on the display.** The guide allows motion only to show a change of state. On the orbit
map the state is the display time: bodies move when it changes and never otherwise. The display
opens held at the time it was given. `RUN` is an operator's command to advance the display time, it
is never the default, it stops at the clock window's edge, and the mode (`HOLD`, or `RUN` with its
rate) is always shown. The display time is labelled as the display's (`DISPLAY TIME UT …`) and is
not the ship's clock, which does not exist yet; when it does, a display time that differs from it
falls under the guide's rule for simulation modes and takes that banner. Step sizes reach down to an
hour, which plan 05's `UT +12.50 yr` cannot show, so the time reads as years, then days, hours,
minutes and seconds in the guide's `MET` form (`UT +12 yr 183/14:08:33`). This format needs the
owner's confirmation, as plan 05's `UT` does.

## Tasks

Nine phases. Every task and subtask ends with `just ci` green, and subtasks land in the order given.
Unless a task says otherwise its unit tests live in the file under test, and every figure that
becomes a constant is re-checked against the source named and cited in the doc comment, as the
roadmap requires. Where a task lists its tests once, each test names its subtask in brackets, and a
subtask's acceptance is that its own tests pass under the task's `_Accept_` command. A domain tag is
added to `rng/tags.rs`, under the "Plan 14" heading, by the task that first opens a stream under it,
as plan 01 requires.

Order and parallelism:

- Phase 0 first. Then phases A→B and T11–T15 of phase C are independent of each other and can run in
  parallel; within C, T13 needs T11 and T12. T16 needs T11–T15 and, for its sampled tests, T8.
  T10.b's radius-dependent assertions need T11.
- T3 and T4 take the limits of a stable zone and of the strip radius as plain arguments. T9 and T29
  supply real values when they land, and until then callers pass none.
- Phase D needs B, T15 and T16. Phase E needs C and D. Phase F needs B–E. Phase G needs F.
- Phase H's wire types (T35) can be drafted once T34 has fixed `BodyRecord`, in parallel with G.
- Phase I's T38, T39 and T40 need only plan 05 and can start at any time; T41–T44 need H.

### Phase 0: groundwork

#### P14.T1 Scaffold, interfaces and the body index

- **P14.T1.a Re-validate interfaces.** Read the "Provides" of plans 01, 06, 09, 11 and 13 and the
  code they produced. For every item under [Consumes](#consumes) record its real path in the module
  docs of `planetary/mod.rs`. Where something is missing, add it as a small additive function with
  tests, in this subtask and without a new draw. Expected: [α/Fe] as a closed form of [Fe/H] and
  population (the thin-disc and thick-disc sequences; cite the source used), in
  `planetary/context.rs`; an X-ray and ultraviolet history as a closed form of mass, age and plan
  06's `ActivityLevel` (saturation time by spectral type as the fallback), in the same file; and, in
  plan 09's modules, an accessor for a system's sphere-of-influence radius by its rule and for a
  feature member's environment.
  - _Files:_ `crates/hyperion-sim/src/planetary/mod.rs`, owning modules as needed.
  - _Accept:_ `cargo doc -p hyperion-sim` builds with intra-doc links from `planetary` to every
    consumed item; no generated output changes (the goldens of plans 01–13 pass untouched).
- **P14.T1.b Module tree and errors.** Create
  `planetary/{mod, error, index, context, params, system, record}.rs` with `//!` docs. Error enums:
  `EncodeBodyIndexError`, `DecodeBodyIndexError`,
  `ResolveBodyError { NoSuchSystem, NoSuchBody, MalformedIndex }`. Add the "Plan 14" heading to plan
  01's `rng/tags.rs`, with a comment listing the names and scopes under Provides so that no later
  task picks another; the entries themselves arrive with the tasks that use them.
  - _Tests:_ each error's `Display` text is lower case without a full stop.
  - _Accept:_ `cargo test -p hyperion-sim planetary::error` passes; plan 01's tag-collision
    assertion still compiles.
- **P14.T1.c `BodyIndex`.** The layout of D3 with `new`, `decode`, `slot()`, `sub()`, `parent()` (a
  moon's parent is its planet, a planet's is resolved by the system), and `TryFrom<u16>`. Reserved
  values are rejected.
  - _Tests:_ round trip over all 65,536 values: `decode(raw)` succeeds exactly on the canonical set,
    and re-encoding returns `raw`.
  - _Accept:_ `cargo test -p hyperion-sim planetary::index`.
- **P14.T1.d `SystemContext`.** Fields as in Provides. `for_system` resolves the ID through plan 03,
  builds the stars through plans 06 and 11 (or the substellar record through plan 13), reads [Fe/H]
  and [α/Fe], the sphere-of-influence radius and the encounter environment. The builder makes
  synthetic hosts: `synthetic_star(mass, feh, age)`, `synthetic_binary(m1, m2, a, e, ..)`, with a
  caller-supplied `SystemId` so that streams differ between samples.
  `sample_contexts(n, seed, filter)` draws real systems from a Milky-Way-parameter galaxy for the
  statistical tests.
  - _Tests:_ `for_system` on an unresolvable ID returns `NoSuchSystem`; the builder rejects a
    negative mass or an age beyond the universe's.
  - _Accept:_ `cargo test -p hyperion-sim planetary::context`.

#### P14.T2 Orbit additions

In plan 11's `crates/hyperion-sim/src/orbit/`, per D2. Nothing here may change a binary's state:
plan 11's goldens pass untouched.

- **P14.T2.a Bound orbits at planetary precision.** Check, and fix if needed, that
  `KeplerElements::relative_state_at` reduces the mean anomaly from `UniverseTime`'s integer seconds
  modulo the period before converting to `f64`, so that a one-day orbit keeps its phase a thousand
  years out, and that `solve_kepler` exits after a fixed number of iterations and not on a
  tolerance, so that every platform agrees. Add `from_semi_major_axis` and `scaled`.
  - _Tests:_ residual |E − e sin E − M| < 10⁻¹³ for e up to 0.999; position at t and t + 10⁵ P agree
    to 1 m for a 1-day orbit at 0.02 au, and to 1 mm after one period at 50 au; energy and angular
    momentum of the state match the elements.
- **P14.T2.b Open orbits.** `orbit/open.rs`: `OpenOrbit`, Barker's equation in closed form, the
  hyperbolic equation by the same fixed-iteration scheme, and the near-parabolic series inside |e −
  1| < 10⁻⁶.
  - _Tests:_ continuity of position across the switch to 1 m at 1 au; hyperbolic residual as above.
- **P14.T2.c Elements from a state.** `elements_from_state`, the inverse of `relative_state_at`, for
  T28.c.
  - _Tests:_ round trip of 10⁴ random bound states to 10⁻¹² relative; an unbound state returns the
    `OpenOrbit`.
  - _Accept (all of T2):_ `cargo test -p hyperion-sim orbit`.

### Phase A: disc and architecture

#### P14.T3 The disc

`planetary/disc.rs`. Inputs: host mass, [Fe/H], zero-age luminosity and radius (D6), and optional
inner and outer truncation radii in metres. All draws on `planet.disc`, keyed by `SystemId` with the
host number in the draw number.

- **P14.T3.a Masses and lifetime.** Gas mass M_d = f × M★ with log₁₀ f normal about −2.0, σ = 0.5,
  capped at −1.0 (gravitational instability). Solid mass M_s = M_d × Z☉ × 10^[Fe/H] with Z☉ = 0.0149
  (Lodders 2003), times an ice enhancement beyond the snow line (factor 2; Lodders 2003). Lifetime
  log-normal about 2.5 Myr × (M★ ÷ M☉)^−0.5, σ = 0.3 dex (Mamajek 2009; Ribas et al. 2015). For a
  host over 3 M☉ the lifetime's mass scaling is what starves planet formation; no separate switch.
  - _Tests:_ medians and widths of 10⁵ draws within 2% of the parameters; solid mass scales as
    10^[Fe/H] exactly.
- **P14.T3.b Geometry.** `snow_line(L)` = 2.7 au × √(L ÷ L☉). Inner edge: the larger of 2.5 zero-age
  stellar radii, the star's fluid Roche limit for a 1,000 kg/m³ body, and a magnetospheric
  truncation radius at a drawn corotation period, log-normal about 8 days, σ = 0.25 dex (the
  observed inner edge of Kepler systems near 10 days; Mulders et al. 2018). Outer radius r_c = 30 au
  × (M★ ÷ M☉)^0.5 with 0.3 dex of scatter; surface density Σ ∝ r⁻¹ exp(−r ÷ r_c), normalised to M_s.
  Then truncate to the radii passed in, renormalising nothing: a truncated disc has lost that mass.
  The orbit zone (T9.c) and the strip radius of D14 (T29) are what callers pass.
  - _Tests:_ `snow_line` of 1 L☉ is 2.7 au; the integrated surface density returns M_s to 10⁻⁹ for
    an untruncated disc; inner edge < outer edge or the disc is `Disc::None`.
- **P14.T3.c `Disc` type** with getters `gas_mass`, `solid_mass`, `solid_mass_between(a, b)` (closed
  form of the Σ above), `lifetime`, `snow_line`, `inner_edge`, `outer_edge`, and `isolation_mass(a)`
  = the mass a body can sweep from its feeding zone of 10 Hill radii, solved in closed form from Σ
  (Lissauer 1987).
  - _Tests:_ `solid_mass_between` over the whole disc equals `solid_mass`, and is additive over
    adjoining intervals to 10⁻¹²; a minimum-mass solar disc gives an isolation mass of 0.05–0.2 M⊕
    at 1 au and 3–15 M⊕ at 5 au.
  - _Accept (all of T3):_ `cargo test -p hyperion-sim planetary::disc`.

#### P14.T4 Architecture classes and their frequencies, written down

This is the task the brainstorm's "ours to define and defend" asks for. Its output is the module
documentation of `planetary/architecture.rs` and the constant `ARCHITECTURE_TABLE`, which are the
single written definition; tests read the same table.

- **P14.T4.a The classes.** Document each class with what it is, what observation motivates it and
  what it contains. Starting content:

  | Class                  | Contents                                                                                                            | Motivation                                                             |
  | ---------------------- | ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
  | `Barren`               | Nothing above 0.02 M⊕; belts allowed                                                                                | Stars with no detected planets; disc failures                          |
  | `TerrestrialOnly`      | 2–6 rocky planets of 0.05–2 M⊕ inside the snow line, 0–3 ice-rich bodies beyond                                     | The population below survey limits; Buchhave et al. 2012               |
  | `CompactMulti`         | 1–7 planets of 1–20 M⊕ starting at 1–50 days; 40% are the dynamically hot variant with 1–2 planets and larger e, i  | Kepler multis; Weiss et al. 2018; Pu and Wu 2015; the Kepler dichotomy |
  | `CompactWithColdGiant` | `CompactMulti` plus 1–2 giants beyond the snow line                                                                 | Zhu and Wu 2018; Bryan et al. 2019                                     |
  | `SolarLike`            | Terrestrials, 1–3 low-eccentricity giants from 1–2 snow-line radii, 0–2 ice giants, both belts                      | Jupiter analogues, 3–6% (Wittenmyer et al. 2016)                       |
  | `EccentricGiant`       | 1–2 giants at 0.5–5 au × √L with eccentricities from Kipping's (2013) Beta(0.867, 3.03); 0–1 small survivor         | The radial-velocity giants; scattering                                 |
  | `WarmGiant`            | A giant at 0.1–1 au × √L, moderate e, small companions in half of cases (Huang et al. 2016)                         | Disc migration                                                         |
  | `HotJupiter`           | A giant at 1–10 days, piled up near 3–4; nothing else inside 100 days; an outer giant in 50–70% (Bryan et al. 2016) | Wright et al. 2012; Howard et al. 2012                                 |
  | `SubstellarCompact`    | For hosts under 0.08 M☉: a compact chain of 0.01–2 M⊕ bodies                                                        | D13; TRAPPIST-1 as the limiting case                                   |

  The three migration classes are `WarmGiant`, `HotJupiter` and the inner planets of the two compact
  classes; a migrated body keeps the composition of where it formed (flag
  `formed_beyond_snow_line`).

- **P14.T4.b The frequency model.** Each class has a weight w(M★, [Fe/H]) and the probability is its
  weight over the sum. Starting weights at 1 M☉ and [Fe/H] = 0, and their scalings:

  | Class                  | w₀    | Mass scaling  | Metallicity scaling |
  | ---------------------- | ----- | ------------- | ------------------- |
  | `Barren`               | 0.24  | 1             | 1                   |
  | `TerrestrialOnly`      | 0.35  | 1             | s([Fe/H])           |
  | `CompactMulti`         | 0.24  | (M ÷ M☉)^−0.9 | s([Fe/H])           |
  | `CompactWithColdGiant` | 0.07  | g(M)          | 10^(2[Fe/H])        |
  | `SolarLike`            | 0.03  | g(M)          | 10^(2[Fe/H])        |
  | `EccentricGiant`       | 0.04  | g(M)          | 10^(2[Fe/H])        |
  | `WarmGiant`            | 0.02  | g(M)          | 10^(2[Fe/H])        |
  | `HotJupiter`           | 0.008 | g(M)          | 10^(2[Fe/H])        |

  with g(M) = M ÷ M☉ up to 1.9 M☉ (Johnson et al. 2010) and a Gaussian fall beyond it of width 0.8
  M☉ (Reffert et al. 2015), and s = 1 ÷ (1 + 10^(−2([Fe/H] + 1.5))), which is flat through the discs
  and thins small planets only in the halo, as the brainstorm asks after Buchhave et al. [Fe/H] is
  clamped to ±0.5 inside the giant factor, the range Fischer and Valenti fitted. The ratio form
  keeps every giant class proportional to 10^(2[Fe/H]) while rare and saturates the total near two
  thirds at the metal-rich end. The anchors the weights were chosen to meet, each to be re-checked:
  giants of 0.3–10 M_J inside 2,000 days around 10.5% of FGK stars (Cumming et al. 2008); hot
  Jupiters around 0.4–1.2%; Kepler-like inner systems around 30% of Sun-like stars (Zhu et al.
  2018); a cold giant in about a third of those (Zhu and Wu 2018); 2.5 ± 0.2 small planets inside
  200 days per M dwarf (Dressing and Charbonneau 2015); giants around about 3% of M dwarfs.

- **P14.T4.c Code.** `class_weights`, `ClassWeights::probabilities`, and `draw_class`, which maps
  one `planet.class` draw through integer thresholds (plan 01's `Thresholds::from_weights` and
  `pick`). D5's fallback and D10's binary suppression are applied here, from plain arguments: the
  disc, the zone's outer limit if any, and whether the host is in a binary closer than 50 au.
  - _Tests, which pin the table:_ probabilities sum to 1 for a grid of masses 0.08–150 M☉ and [Fe/H]
    −2.5 to +0.5; the summed weight of the giant classes at fixed mass, before normalisation, is
    10^(2Δ[Fe/H]) to 10⁻¹² for [Fe/H] from −0.5 to +0.5 and constant outside it, and the log-slope
    of the giant share is 1.85–2.0 between −0.5 and −0.2 (the share itself bends as it saturates);
    at 0.3 M☉ and solar metallicity the giant classes sum to under 0.05 and the two compact classes
    to over 0.45, and at 1 M☉ to 0.14–0.20 and 0.27–0.35; the weights of the small-planet classes at
    [Fe/H] = −0.8 are within 5% of solar and under a quarter of it at −2; a golden file of all class
    probabilities on a 6 × 6 grid of mass and [Fe/H], so that any change to a weight or a scaling is
    a visible diff; chi-square of 10⁵ class draws against the probabilities.
  - _Files:_ `crates/hyperion-sim/src/planetary/architecture.rs`.
  - _Accept:_ T4.a and T4.b: `cargo doc -p hyperion-sim` renders both tables in the module
    documentation of `planetary::architecture`, every row of each naming its source in full
    (authors, year, journal, the figure or table used), with a line per anchor saying whether it was
    re-checked against the paper. T4.c: `cargo test -p hyperion-sim planetary::architecture`.

#### P14.T5 Class templates

`planetary/architecture.rs`, `ClassTemplate`: for each class the ordered list of planet groups it
places, each with a count distribution, a mass range and law, a starting location and a spacing law,
as data. P14.T8 interprets it. Starting values are the "Contents" column above, with counts from
zero-truncated Poisson laws (mean 3.5 for a cold compact chain), giant masses from dN ÷ d ln M ∝
M^−0.31 over 0.3–10 M_J (Cumming et al. 2008) and the hot-Jupiter period log-normal about 3.5 days.

- _Tests:_ every template's ranges are ordered and non-empty; a template never asks for a giant
  inside the snow line unless its group is marked migrated.
- _Accept:_ `cargo test -p hyperion-sim planetary::architecture::template`.

### Phase B: placement

#### P14.T6 Hill spacing and the stability floor

`planetary/placement/spacing.rs`.

- **P14.T6.a Primitives.** `mutual_hill_radius(m1, m2, m_host, a1, a2)` = ((m₁ + m₂) ÷ 3M★)^⅓ ×
  (a₁ + a₂) ÷ 2. `next_semi_major_axis(a1, delta, chi)` = a₁(1 + Δχ) ÷ (1 − Δχ) with χ = ½((m₁ + m₂)
  ÷ 3M★)^⅓, returning `None` when Δχ ≥ 0.9 (no room for another planet).
  `spacing_floor(m1, m2, e1, e2)` per D7. `satisfies_floor(inner, outer, m_host)` checks both of
  D7's conditions.
- **P14.T6.b The spacing draw.** A system draws a mean spacing μ on `planet.spacing`: normal about
  17 with σ = 2.5 held to 13–24 for small planets (the brainstorm's 14–20; Weiss et al. 2018; Pu and
  Wu 2015), about 30 with σ = 8 for the terrestrial groups of `SolarLike` and `TerrestrialOnly`, and
  about 9 with σ = 2 for giant pairs. Each pair draws Δ = μ + N(0, 3). A draw under the floor is
  rejected and redrawn on the next draw number, at most 16 times, after which the floor itself is
  used, so the loop is bounded and deterministic.
  - _Tests:_ (a) the Solar System's eight planets pass `satisfies_floor` pairwise (Jupiter and
    Saturn at 8.0 against a floor of 7); two Jupiters at 5.2 and 6.5 au fail; Kepler-11 b and c, at
    9.4, fail narrowly, which is the known cost of the floor (see risks); `next_semi_major_axis`
    inverts `mutual_hill_radius` to 10⁻¹². (b) The distribution of accepted Δ for small pairs has no
    mass below 10, which is the brainstorm's floor, and a median in 14–20, which is its centre; two
    runs of the redraw loop agree.
  - _Accept:_ `cargo test -p hyperion-sim planetary::placement::spacing`.

#### P14.T7 Masses and the peas-in-a-pod correlation

`planetary/placement/masses.rs`.

- **P14.T7.a Characteristic mass.** Per host and group, ln m_c is normal about a median that scales
  with the disc's solid mass inside the group's zone (median 4 M⊕ for a compact group in a solar
  disc, exponent 1 on solid mass, held to the template's range) with between-system scatter σ_b.
- **P14.T7.b Members.** ln mᵢ = ln m_c + σ_w εᵢ + 0.1 dex per step outward, with σ_b and σ_w chosen
  so that the correlation of adjacent log radii after P14.T11 is 0.65 (Weiss et al. 2018) and the
  outer planet of a pair is the larger in about 65% of pairs. Each mass is capped by
  `Disc::isolation_mass` times 10 (pebble and merger growth) and the group's total by the solids
  available. Draw numbers are the planet's slot, so inserting a group never shifts another.
- **P14.T7.c Giants.** Mass from the template's law, capped by the gas mass of the disc. A giant
  needs 10 M⊕ of solids beyond the snow line within the disc lifetime, else D5's fallback.
  - _Tests:_ (a) the median characteristic mass of 10⁴ compact groups in a solar disc is 3.6–4.4 M⊕
    and doubles with the solid mass. (b) A fixed-seed sample of 2,000 compact systems gives an
    adjacent log-mass correlation of 0.5–0.8, and the outer planet of a pair is the more massive in
    0.55–0.75 of pairs; no group exceeds its disc's solids; changing the template of one group
    leaves the masses of the others bit-identical. (c) No giant exceeds its disc's gas mass, and a
    disc with under 10 M⊕ of solids beyond the snow line yields the fallback.
  - _Accept:_ `cargo test -p hyperion-sim planetary::placement::masses`.

#### P14.T8 Class placers

`planetary/placement/classes.rs` and `planetary/placement/mod.rs`.
`place(ctx, host, zone, disc, class) -> Vec<PlacedPlanet>` interprets the template inside the zone.

- **P14.T8.a Compact groups.** First period from the template, held outside the disc's inner edge.
  Walk outward with T6 and T7 until the count is reached, the zone ends or `next_semi_major_axis`
  returns `None`. The dynamically hot variant has 1–2 planets. A cold chain of three or more is
  marked resonant with probability 0.1 (0.3 for hosts under 0.3 M☉): each period ratio then snaps to
  the nearest of 4:3, 3:2, 5:3 and 2:1, widened by 0.5–2%, but only where the floor still holds.
- **P14.T8.b Terrestrial and Solar-like groups.** Rocky planets from about 0.3 au × √L to the snow
  line; giants from 1–2 snow-line radii outward; ice giants beyond them, thinning with the disc's
  taper. A gap of at least 2 × 1.3 (m ÷ M★)^(2⁄7) a either side of a giant is kept free of small
  planets (the chaotic zone of Wisdom 1980).
- **P14.T8.c Giant and migration classes.** `EccentricGiant`, `WarmGiant`, `HotJupiter`, and the
  cold giants of `CompactWithColdGiant`. A migrated giant's final orbit is drawn from the template,
  and no small planet is placed between it and its formation zone except where the template says.
  The hot Jupiter's orbit must lie outside twice the star's Roche limit for the planet's density.
- **P14.T8.d Eccentricity, inclination and angles.** On `planet.orbit`, per body. Eccentricity:
  Rayleigh with σ = 0.04 for cold compact groups, 0.3 (half-normal) for the hot variant (Xie et al.
  2016; Van Eylen et al. 2019), 0.05 for Solar-like groups, Beta(0.867, 3.03) for eccentric giants.
  Mutual inclination: Rayleigh with σ = 1.5° for cold groups (Fabrycky et al. 2014), otherwise σ_i =
  σ_e ÷ 2 radians, about a system plane whose orientation is one isotropic draw per host on
  `planet.plane` (for a close binary, the binary's plane, with no draw). Node, argument of
  pericentre and mean anomaly at the epoch are uniform. After drawing e, spacing is re-checked
  against D7 and the outer body's e is scaled down until it holds, which is deterministic and needs
  no redraw.
- **P14.T8.e Tidal circularisation.** e(t) = e₀ exp(−(age + t) ÷ τ_c) and a(t) from constant angular
  momentum, with τ_c from the standard constant-Q form (Goldreich and Soter 1966; Q′ = 10⁶ for
  giants, 10² for rocky bodies). Evaluated in the fate transform (T28), defined here.
  - _Tests:_ (a, b, c, each for its own classes) every placed planet lies inside the limits passed
    in and outside the disc's inner edge, and every adjacent pair satisfies D7's semi-major-axis
    floor; (a) resonant pairs sit 0.5–2% wide of commensurability; (b) no small planet lies in a
    giant's chaotic zone; (c) every hot Jupiter lies outside twice its Roche limit and has no
    companion inside 100 days; (d) every adjacent pair satisfies both conditions of D7 at the epoch,
    and the eccentricities of 10⁴ cold compact planets pass a Kolmogorov–Smirnov test against the
    Rayleigh law where no rescaling happened; (e) hot Jupiters within 5 days have e < 0.01 at 5 Gyr,
    a(t) stays between the original pericentre and semi-major axis, and every adjacent pair still
    satisfies D7 at ±H.
  - _Accept:_ `cargo test -p hyperion-sim planetary::placement`.

#### P14.T9 Stable zones in multiple systems

`planetary/placement/zones.rs`, per D10.

- **P14.T9.a The fits.** `holman_wiegert_s_type(μ, e)` = 0.464 − 0.380μ − 0.631e + 0.586μe + 0.150e²
  − 0.198μe² and `holman_wiegert_p_type(μ, e)` = 1.60 + 5.10e − 2.22e² + 4.12μ − 4.27eμ − 5.09μ² +
  4.61e²μ², both in units of the binary's semi-major axis with μ = m₂ ÷ (m₁ + m₂) (Holman and
  Wiegert 1999, equations 1 and 3; re-check every coefficient). Outside the fitted range (e ≤ 0.8
  for S-type, e ≤ 0.7 for P-type, 0.1 ≤ μ ≤ 0.9) the arguments are clamped, which errs towards
  smaller zones.
- **P14.T9.b Zones from the hierarchy.** `stable_zones(&SystemHierarchy) -> Vec<OrbitZone>`: one
  zone per component and per pair, each with its host, host mass, inner and outer limits. A single
  star has one zone bounded only by its disc and D14. A zone narrower than a factor of 1.5 in radius
  is dropped.
- **P14.T9.c Host assignment.** The disc of T3 is derived per zone: a circumstellar disc from its
  star, a circumbinary one from the pair's total mass and summed luminosity. Apply D10's suppression
  for binaries inside 50 au (Kraus et al. 2016; Moe and Kratter 2021) by moving class weight to
  `Barren`.
  - _Tests:_ (a) for μ = 0.5, e = 0: S-type limit 0.274 and P-type 2.39 binary separations; α
    Centauri AB (23.5 au, e = 0.52) gives about 2.8 au around A. (b) Zones of a hierarchical triple
    never overlap; a single star has exactly one zone; a brown-dwarf companion of plan 11 bounds
    zones like any other component. (c) No planet of 10⁴ sampled binaries lies outside its zone, and
    hosts in binaries inside 50 au have planets a quarter to a half as often as single stars of the
    same mass.
  - _Accept:_ `cargo test -p hyperion-sim planetary::placement::zones`.

#### P14.T10 Placement tests

`crates/hyperion-sim/tests/planetary_placement.rs`, the statistical part marked slow.

- **P14.T10.a Property: no overlapping orbits.** For every sampled host (T8's `place` over
  `sample_contexts` and the synthetic hosts, several thousand in the ordinary suite and a million
  when slow, D20) and every pair of bodies sharing a host: the inner apocentre is below the outer
  pericentre by at least 2√3 mutual Hill radii, at the epoch and at ±H.
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_placement no_overlapping_orbits`.
- **P14.T10.b Statistics of architecture,** each against the source's window and not against our own
  table:
  - small planets (1–4 R⊕, under 100 days) per FGK star in 0.5–1.2 (Fressin et al. 2013; Petigura et
    al. 2018), and per M dwarf under 200 days in 1.8–3.2 (Dressing and Charbonneau 2015);
  - hot Jupiters around 0.4–1.2% of FGK stars; giants of 0.3–10 M_J inside 2,000 days around 7–14%;
  - the slope of log occurrence against [Fe/H] for giants in Fischer and Valenti's window (period
    under 4 years, velocity semi-amplitude over 30 m/s, FGK hosts, −0.5 to +0.3) is 2.0 ± 0.3;
  - among hosts of 0.1–0.5 M☉, at least 40% have two or more planets inside 200 days, and under 5%
    have a giant;
  - small planets per star at [Fe/H] = −0.8 within 20% of solar, and under a quarter of solar at −2.

  The assertions that need a radius use T11 and are added here when it lands, so that the
  architecture has one report. Until then the same counts are asserted on mass (1–20 M⊕).
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_placement` and `just test-slow`. If an
    assertion fails, the weights of T4.b are tuned inside their sources' uncertainty, the change is
    recorded in the table's documentation, and the windows here are not widened.

### Phase C: derivation

Pure functions in `planetary/derive/`, each taking plain quantities so that it can be tested on
Solar System values without a generator.

#### P14.T11 Mass, radius and composition

- **P14.T11.a Chen and Kipping.** `radius_chen_kipping(mass, quantile)`: the broken power law with
  exponents 0.279, 0.589, −0.044 and 0.881 and breaks at 2.04 M⊕, 0.414 M_J and 0.080 M☉, with the
  paper's intrinsic scatter per segment applied through the quantile (plan 06's
  `math::normal_quantile`). Re-check the constants against Chen and Kipping 2017, table 1.
- **P14.T11.b Zeng curves.** `radius_zeng(mass, composition)` for iron, Earth-like (32.5% iron),
  pure rock, 50% water and 100% water as power laws R = c × M^(1⁄3.7) with the published
  coefficients (Zeng et al. 2019, and Zeng, Sasselov and Jacobsen 2016 for (1.07 − 0.21 × core mass
  fraction)), interpolated linearly in composition between curves, plus the hydrogen-envelope curves
  at 0.1–5% by mass as a small table in envelope fraction and equilibrium temperature.
- **P14.T11.c Composition solve.** Per D8: `composition(mass, radius, origin, t_eq)`, where `origin`
  says which side of the snow line the body formed on, returns a `BulkComposition` of iron, rock,
  water and envelope mass fractions summing to 1. Below the iron curve the radius is raised to it.
  Above the rock curve a body formed inside the snow line takes an envelope if its core is over 1.5
  M⊕ and otherwise is clamped to rock with up to 0.1% water.
- **P14.T11.d Giants.** Above 0.414 M_J: the radius of plan 13's `giant_cooling` at the body's age
  (it covers 0.3–13 M_J, so every giant this plan places), blended into Chen and Kipping's over
  0.3–0.414 M_J; inflated for equilibrium temperatures over 1,000 K by the fitted heating efficiency
  of Thorngren and Fortney (2018), capped at 2 R_J.
  - _Tests:_ (a) radius is continuous across the segment breaks at the median quantile and monotone
    in the quantile; the median gives 1.0 R⊕ at 1 M⊕ and 1.0 R_J at 1 M_J to 10%. (b) Earth, Venus,
    Mars and Mercury radii from mass and known composition within 5%. (c) Fractions sum to 1; the
    composition solve is the inverse of `radius_zeng` on its curves; a body formed inside the snow
    line never comes out as a water world. (d) Uranus, Neptune, Saturn and Jupiter radii within 10%;
    radius is continuous in mass across 0.414 M_J and across the lower end of plan 13's
    `giant_cooling` at 0.3 M_J to 5%.
  - _Files:_ `planetary/derive/{radius, composition}.rs`.
  - _Accept:_ `cargo test -p hyperion-sim planetary::derive::radius planetary::derive::composition`.

#### P14.T12 Irradiation, equilibrium temperature and the habitable zone

- **P14.T12.a Equilibrium temperature.** T_eq = T★ √(R★ ÷ 2a) (1 − A)^¼ (1 − e²)^(−⅛), the
  orbit-averaged form, with the host's state at age + t. In a multiple system fluxes add: a
  circumstellar planet sees its companion at the binary's time-averaged separation, a circumbinary
  one sees both at its own distance. Bond albedo is derived from the surface and cloud state in T13,
  with 0.3 as the value used before that loop closes. Moons take their planet's orbit. A body with
  internal luminosity adds it: T⁴ = T_eq⁴ + L_int ÷ (4πR²σ).
- **P14.T12.b Habitable zone.** `habitable_zone(L, T_eff)` returns a `HabitableZone` with the five
  limits `recent_venus`, `runaway_greenhouse`, `moist_greenhouse`, `maximum_greenhouse` and
  `early_mars`, as distances d = √(L ÷ S_eff) au with S_eff = S☉ + aT + bT² + cT³ + dT⁴, T = T_eff −
  5,780 K, from the corrected coefficients of Kopparapu et al. (2013, with the 2013 erratum, ApJ
  770, 82). The fit holds for 2,600–7,200 K; outside it T_eff is clamped and the result flagged
  `extrapolated`. In a multiple system the limit is where Σ Lᵢ ÷ (dᵢ² S_eff,ᵢ) = 1, solved on the
  orbit-averaged distances.
  - _Tests:_ (a) Earth 255 K, Venus 229 K (A = 0.76), Mars 210 K, Jupiter 110 K with their albedos
    to 2 K; a circumbinary planet of two equal stars is 2^¼ hotter than with one. (b) The Sun's
    conservative zone is 0.99–1.69 au and its optimistic one 0.75–1.77 au to 0.02; the zone moves
    outward monotonically along a 1 M☉ track until the tip of the giant branch; a 2,300 K host is
    flagged `extrapolated`.
  - _Files:_ `planetary/derive/{irradiation, habitable_zone}.rs`.
  - _Accept:_
    `cargo test -p hyperion-sim planetary::derive::irradiation planetary::derive::habitable_zone`.

#### P14.T13 Atmospheres: inventory, escape, greenhouse

`planetary/derive/atmosphere.rs`.

- **P14.T13.a Inventory.** The initial envelope fraction is not drawn here: it is the one T11.c
  solved from the body's radius quantile (D8), so that radius and envelope can never disagree. Its
  distribution over cores above 1.5 M⊕ is checked against a log-normal about 3% × (M_core ÷ 5
  M⊕)^0.6 with σ = 0.5 dex, and the scatter T11.a applies is the dial if it is far off. Drawn here,
  on `planet.volatiles`: a volatile inventory (water, carbon, nitrogen) as a log-normal multiple of
  Earth's per unit mass, raised beyond the snow line and for migrated bodies.
- **P14.T13.b Thermal escape.** Jeans parameter λ = G M m ÷ (k T_exo R) per species (H₂, He, H₂O,
  CH₄, NH₃, N₂, O₂, CO₂, Ar), with the exobase temperature a stated multiple of T_eq that rises with
  the host's activity; a species is retained over the system's age when λ exceeds a threshold near
  25, fixed by the Solar System table below. Hydrogen envelopes are also subject to energy-limited
  escape: mass lost = ε π R³ E_XUV ÷ (G M K_tide) with ε = 0.1, where E_XUV is the closed-form time
  integral of the host's X-ray and ultraviolet output, saturated at 10^−3.5 of bolometric for 100
  Myr (1 Gyr for M dwarfs) and falling as t^−1.5 after (Ribas et al. 2005; Owen and Wu 2017). The
  envelope fraction at age + t is the initial one less the loss, floored at zero, and the radius
  follows D8. The largest past luminosity (D11) enters the exobase temperature, so a giant-branch
  survivor is judged on the worst it has seen.
- **P14.T13.c Greenhouse and surface state.** Surface pressure from the retained inventory and
  gravity. Grey-atmosphere warming T_s = T_eq' (1 + ¾τ)^¼ with optical depth a sum over retained
  greenhouse gases of k × (partial pressure)^½, the constants fixed by Venus, Earth, Mars and Titan.
  States: runaway greenhouse inside the runaway limit of T12 when water was present (oceans lost, a
  thick CO₂ atmosphere), temperate, snowball beyond the maximum-greenhouse limit, airless, magma
  ocean (D12 or T_s over the silicate solidus), gas envelope. Irradiation alone can never lift T_s
  above the hottest host's effective temperature, which is thermodynamics, so the function saturates
  there. Albedo and cloud fraction follow from the state, and T12 and T13 iterate a fixed three
  times.
  - _Tests (Solar System table, `solar_system_bodies`):_ (a) the median initial envelope fraction of
    5 M⊕ cores is 1–6%, and the inventory scales with the side of the snow line. (b) Earth, Venus,
    Mars and Titan keep atmospheres, Mercury, the Moon, Ganymede and Ceres do not; a 5 M⊕ core with
    a 2% envelope at 0.05 au of a Sun is stripped by 1 Gyr and the same body at 0.5 au is not; the
    envelope fraction is monotone and continuous in time. (c) Surface temperatures of Venus 735 K,
    Earth 288 K, Mars 215 K, Titan 94 K within 8%; no surface is hotter than the hottest host.
  - _Accept:_ `cargo test -p hyperion-sim planetary::derive::atmosphere`.

#### P14.T14 Rotation and tides

`planetary/derive/rotation.rs`.

- **P14.T14.a Spin.** On `planet.spin`: primordial rotation period (log-normal about 10 h for
  giants, 15 h for rocky bodies), obliquity (isotropic for bodies that had giant impacts, Rayleigh
  about 10° otherwise), rotation phase at the epoch.
- **P14.T14.b Locking.** τ_lock = ω a⁶ I Q ÷ (3 G M★² k₂ R⁵) with I = 0.33–0.4 M R² by class, Q =
  100 and k₂ = 0.3 for rocky bodies, Q = 10⁵ and k₂ = 0.4 for giants (Gladman et al. 1996). The spin
  rate at age + t falls linearly to the synchronous rate over τ_lock. A locked body with e over
  about 0.1 is in the 3:2 state. Moons use their planet as the primary. Rotation angle is a closed
  form of time in each regime, continuous where regimes meet.
- **P14.T14.c Body-fixed frame.** `planetary/frames.rs`, per D23:
  `BodyFixedFrame { pole, w0, rate }` and `body_fixed_at(body, t)`, the rotation from the body's
  inertial frame (plan 01) to its fixed frame at a time.
  - _Tests:_ (a) 10⁵ periods and obliquities pass Kolmogorov–Smirnov tests against their laws. (b)
    The Moon, Io and Titan lock in under 100 Myr; Earth and Mars do not lock in 10 Gyr; Mercury
    locks and lands in 3:2; a planet in the habitable zone of a 0.2 M☉ star locks in under 1 Gyr;
    rotation angle is continuous across the locking time. (c) A locked body's prime meridian faces
    its primary at every pericentre; `body_fixed_at` is a rotation (orthonormal to 10⁻¹²) at every
    time.
  - _Accept:_ `cargo test -p hyperion-sim planetary::derive::rotation planetary::frames`.

#### P14.T15 Roche limits, Hill spheres and satellite survival

`planetary/derive/limits.rs`: `roche_limit_fluid` = 2.456 R (ρ_primary ÷ ρ_satellite)^⅓,
`roche_limit_rigid` = 1.26 R (ρ_primary ÷ ρ_satellite)^⅓, `hill_radius` = a(1 − e)(m ÷ 3M)^⅓,
`satellite_stability_limit(hill, e_planet, e_satellite, sense)` = 0.4895 R_H (1 − 1.0305 e_p −
0.2738 e_s) prograde and 0.9309 R_H (1 − 1.0764 e_p − 0.9812 e_s) retrograde (Domingos, Winter and
Yokoyama 2006), and `maximum_surviving_moon_mass(planet, host, age)` from Barnes and O'Brien (2002),
which removes the moons of close-in planets. `SATELLITE_STABILITY_FRACTION` of D14 is the prograde
constant.

- _Tests:_ Saturn's fluid Roche limit for porous ice of 600 kg/m³ is 2.5–2.7 Saturn radii and
  contains its main rings, which end at 2.27; Earth's Hill radius is 1.5 × 10⁹ m; every Solar System
  moon lies inside its stability limit; a planet at 0.05 au of a Sun can keep no moon over 10⁻⁶ M⊕
  for 5 Gyr.
- _Accept:_ `cargo test -p hyperion-sim planetary::derive::limits`.

#### P14.T16 Derivation assembly and its property test

- **P14.T16.a Assembly.** `planetary/derive/mod.rs`:
  `derive_body(placed, host_state, disc, age, t) -> DerivedBody` runs T11–T15 in a fixed order and
  is the only entry point the generator uses.
  - _Tests:_ the Solar System table through `derive_body` reproduces the figures of T11–T15; two
    calls agree bit for bit.
  - _Accept:_ `cargo test -p hyperion-sim planetary::derive`.
- **P14.T16.b Properties** (needs T8), in `crates/hyperion-sim/tests/planetary_properties.rs`. **No
  planet hotter than its star**: for every sampled body and time in ±H, surface and effective
  temperatures are below the hottest host's effective temperature; hosts that are black holes are
  excluded, and bodies with internal heat are checked against their coeval host, which the cooling
  fits keep hotter at equal age. Radius, temperature and envelope fraction are continuous in time
  across ±H (step of 1 year, relative jump under 10⁻³ except at a recorded state change).
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_properties no_planet_hotter`.
- **P14.T16.c The radius valley** (needs T8; slow). The radius distribution of sampled small planets
  inside 100 days of FGK hosts of 1–10 Gyr is bimodal with a minimum between 1.5 and 2.0 R⊕ at under
  two thirds of either peak (Fulton et al. 2017), and the same sample at 10 Myr is not. If it fails,
  the dials are the scatter of T11.a and the escape efficiency of T13.b, inside their sources'
  ranges; a gap is never imposed (D8).
  - _Accept:_ `just test-slow` runs `radius_valley_emerges` and it passes.

### Phase D: small bodies

All satellite draws are on the parent's own streams (D4), so a planet's moons can be generated
alone. Every satellite orbit is given in the parent's body frame, referred to the parent's equator
for regular moons and rings and to its orbital plane for the rest.

#### P14.T17 Regular moons

`planetary/moons.rs`.

- **P14.T17.a Count, masses, orbits.** For a parent over 10 M⊕ with an envelope: total satellite
  mass = M_p × 10^N(−3.8, 0.3), the 1–2.5 × 10⁻⁴ of Canup and Ward (2006), split among 1–6 major
  moons (zero-truncated Poisson, mean 3.5) by the peas-in-a-pod law of T7 with the planet as host.
  The innermost sits at 3–8 planetary radii and outside the fluid Roche limit; the rest follow by
  T6's spacing about the planet (mean 15 mutual Hill radii), adjacent pairs snapping to 2:1 with
  probability 0.5, as the Galilean moons and those of TRAPPIST-1's analogues do. Everything must lie
  inside a twentieth of the Hill radius and inside `maximum_surviving_moon_mass`; what does not fit
  is dropped from the outside in. Eccentricity Rayleigh about 0.005, raised to a forced value of
  0.004–0.04 in a resonance; inclination Rayleigh about 0.5° to the planet's equator. Small inner
  moonlets are a count only.
- **P14.T17.b Derivation for moons.** T16's `derive_body` with the planet as the tidal primary and
  the star as the source of light. Composition from a circumplanetary ice line at T = 170 K in the
  planet's own early luminosity. Tidal heating H = (21⁄2)(k₂ ÷ Q)(G M_p² R⁵ n e² ÷ a⁶) (Peale,
  Cassen and Reynolds 1979) gives a surface heat flux, a volcanism level and the subsurface-ocean
  flag for icy moons whose flux exceeds what keeps a water layer liquid under their ice.
  - _Tests:_ (a) Jupiter's and Saturn's masses give total satellite masses within a factor of 3 of
    the real ones in the median; every moon lies inside a twentieth of the Hill radius and outside
    the fluid Roche limit. (b) Io's elements give a heat flux of 1–4 W/m²; Europa's give the
    subsurface-ocean flag; all regular moons lock.
  - _Accept:_ `cargo test -p hyperion-sim planetary::moons::regular`.

#### P14.T18 Giant-impact moons

`planetary/moons.rs`, on `moon.impact`. A rocky or icy planet or dwarf planet of 0.001–5 M⊕ has a
giant-impact moon with probability 0.15 (between the 1 in 12 and 1 in 4 of Elser et al. 2011), with
mass ratio log-uniform over 0.002–0.05, and 0.01–0.15 for dwarf planets (Charon). It forms just
outside the fluid Roche limit and recedes by tides: a(t)^(13⁄2) = a₀^(13⁄2) + (13⁄2) × 3(k₂ ÷ Q) √(G
÷ M_p) R_p⁵ m × (age + t), a closed form that is continuous in time. A moon whose orbit passes the
prograde stability limit is lost at that age (`BodyState::Unbound`). The impact also sets the
planet's obliquity to the isotropic branch of T14 and its surface age clock.

- _Tests:_ Earth–Moon values give 3–5 × 10⁸ m at 4.5 Gyr for an effective Q of 30–40; the orbit is
  monotone in time; a moon of a planet at 0.1 au is lost within 1 Gyr.
- _Accept:_ `cargo test -p hyperion-sim planetary::moons::impact`.

#### P14.T19 Irregular moons

`planetary/moons.rs`, on `moon.capture`. Giants capture a population: a number above 1 km from a
power law scaled to the Hill sphere's cross-section and to the mass of the nearest belt, recorded as
a count with a size slope, and its largest members as bodies (up to four), 10–250 km, on orbits of
0.1–0.45 Hill radii, eccentricity 0.1–0.6, inclinations avoiding 60–120° (the Kozai gap), retrograde
in about two thirds of cases, always inside the matching limit of T15. An ice giant has a
Triton-like large capture with probability 0.2: mass 10⁻⁴–10⁻³ of the planet, retrograde,
circularised close in, and it removes that planet's regular moons beyond its orbit. A rocky planet
next to a belt has one or two kilometre-scale captured moons with probability 0.2.

- _Tests:_ every irregular lies inside its stability limit at pericentre and apocentre; no
  inclination falls in the gap; a Triton-like capture leaves no regular moon outside it.
- _Accept:_ `cargo test -p hyperion-sim planetary::moons::irregular`.

#### P14.T20 Rings

`planetary/rings.rs`, on `ring.system`. Every giant has tenuous dusty rings (a record with optical
depth under 10⁻³). A massive icy ring system like Saturn's exists with probability 0.15 per giant
colder than 170 K at the cloud tops, and a rocky one with probability 0.03 for hotter giants; the
probabilities are parameters of the generator version, because ring lifetimes are disputed. A ring
runs from 1.1 planetary radii to a drawn fraction, 0.6–1.0, of the fluid Roche limit for its
material (600 kg/m³ for porous ice, 2,500 for rock), with gaps at the 2:1 and 3:2 resonances of the
innermost regular moons, a mass of 10⁻⁹–10⁻⁷ of the planet, an optical depth, and the ring plane on
the planet's equator. No moon over 10 km is placed inside a massive ring.

- _Tests:_ **rings inside Roche limits**: outer edge ≤ fluid Roche limit for the ring's density,
  inner edge ≥ the planet's radius, for every sampled ring; about 15% of cold giants have massive
  rings (Poisson interval).
- _Accept:_ `cargo test -p hyperion-sim planetary::rings`.

#### P14.T21 Belts, their largest members and the cometary halo

`planetary/belts.rs` and `planetary/halo.rs`, on `belt.population`, `belt.member` and
`cometary.population`.

- **P14.T21.a Asteroid belts at resonances.** Inside the innermost giant beyond the snow line: a
  belt between its 4:1 and 2:1 resonances (0.397–0.630 of the giant's semi-major axis), provided no
  planet lies within its own chaotic zone of the band, with gaps recorded at 3:1, 5:2 and 7:3. In a
  system without giants, a belt occupies any gap between planets wider than 40 mutual Hill radii,
  between the two chaotic zones. Mass: the disc's solids in the band times a depletion factor of
  10⁻³–10⁻¹ with giants and 10⁻²–1 without, then worn down collisionally as 1 ÷ (1 + age ÷ t_c)
  (Wyatt et al. 2007).
- **P14.T21.b Kuiper-like belt and debris brightness.** Outside the outermost planet, from its 3:2
  to its 2:1 resonance (1.31–1.59 of its semi-major axis) plus a scattered component to the disc's
  outer edge; without planets, the disc's outer third. Mass from the solids there, depleted a
  hundredfold if a giant lies within a factor of 3 in radius. Every belt gets a fractional
  luminosity f = L_dust ÷ L★ from its mass, radius and age (Wyatt 2008), which is what an infrared
  sensor will see.
- **P14.T21.c Largest members.** Sizes follow N(> D) ∝ D^−q with q = 2.5–3.5 normalised to the
  belt's mass. The largest members over 400 km, at most eight per belt, become bodies in the belt's
  slot with sub-indices 1 upward: orbits drawn inside the belt (eccentricity Rayleigh 0.1,
  inclination Rayleigh 8°, more for the scattered component), derived by T16 as dwarf planets, and
  eligible for T18's moon. The rest of the belt stays a population: mass, size slope, bounds, mean
  eccentricity and inclination, composition class by side of the snow line.
- **P14.T21.d Cometary halo.** A statistical population and nothing else: inner radius 2,000 au ×
  (M★ ÷ M☉)^⅓, outer radius the smaller of 50,000 au × (M★ ÷ M☉)^⅓, 0.49 of the sphere of influence
  (D14), and a third of the distance to any wide companion; a number of comets over 1 km of
  10¹¹–10¹² scaled by disc mass and present only if the system has a planet over 10 M⊕ beyond the
  snow line to scatter them; and a rate of new comets reaching the inner system, which T31 turns
  into events. Evolved hosts lose part of it (D11). A halo whose outer radius falls below its inner
  one does not exist, which is the case throughout the nuclear cluster.
  - _Tests:_ (a, b) belts never overlap a planet's chaotic zone, and a Solar System input gives
    belts at 2.1–3.3 au and 39–48 au; (b) the share of FGK hosts of 1–10 Gyr with a cold belt of f >
    10⁻⁶ is 0.15–0.30 (Eiroa et al. 2013; Montesinos et al. 2016; slow); (c) members are inside
    their belt's bounds, at most eight, and their indices decode to the belt's slot; (d) no halo
    extends beyond 0.49 of the sphere of influence, and a system with no planet over 10 M⊕ beyond
    the snow line has none.
  - _Accept:_ `cargo test -p hyperion-sim planetary::belts planetary::halo`.

#### P14.T22 Satellite assembly and small-body property tests

- **P14.T22.a `generate_satellites`.** `planetary/satellites.rs`:
  `generate_satellites(seed, ctx, planet) -> Satellites` runs T17–T20 for one planet in a fixed
  order (regular moons, giant-impact moon, captures and their removals, rings), assigns sub-indices
  by D3 and reads nothing but the planet, its host and the belts' masses passed in. It is what T30
  calls for every planet and what T27.b calls for a rogue planet.
  - _Tests:_ two calls agree bit for bit; sub-indices are unique and decode; a planet with no moons
    returns an empty set, not an error.
  - _Accept:_ `cargo test -p hyperion-sim planetary::satellites`.
- **P14.T22.b Properties.** `crates/hyperion-sim/tests/planetary_properties.rs`, over planets placed
  by T8 for the sampled hosts of T10.a. **Moons inside Hill spheres**: for every sampled moon and
  time in ±H, apocentre < the parent's Hill radius at the parent's pericentre, and also < the T15
  limit for its sense; pericentre > the parent's fluid Roche limit for the moon's density and > the
  parent's radius. **Rings inside Roche limits** (from T20, over the full sample). Moons of one
  planet satisfy T10.a's non-overlap rule with the planet as host. The order-independence test of
  satellites against the whole system belongs to T30.a, where `generate` exists.
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_properties moons rings`, and the
    million-system run under `just test-slow`.

### Phase E: hooks

`planetary/hooks/`. The brainstorm's list, and no more. Nothing here reads a sibling body except
through quantities already in the parent's or host's record.

#### P14.T23 Surface seed and bulk composition

`hooks/mod.rs`. `surface_seed` is one block output of (universe seed, `body.surface`; `BodyId`) and
depends on nothing else, so no later change to derivation can alter a map's seed. `BulkComposition`
is T11's fractions plus the atmosphere's inventory from T13 and the host's [Fe/H] and [α/Fe], which
the resource model and later generators need.

- _Tests:_ the seed of a body is unchanged when any other property of the system changes (generate
  with two different template tables); seeds of 10⁶ bodies have no duplicates.
- _Accept:_ `cargo test -p hyperion-sim planetary::hooks::seed`.

#### P14.T24 Surface conditions and global figures

`hooks/{surface, figures}.rs`. All derived, no draws except where stated, all functions of age + t.

- **P14.T24.a `SurfaceConditions`.** Mean surface temperature from T13 with day–night and
  equator–pole contrasts from rotation state, obliquity and atmospheric column (a thick atmosphere
  or an ocean flattens them); surface pressure and gravity; atmosphere as ordered gas fractions;
  stellar flux; the host's activity level as a radiation class; liquid-water flag from pressure and
  temperature range against water's phase diagram.
- **P14.T24.b `GlobalFigures`.** Ocean fraction: a logistic function of the water inventory over the
  basin capacity, where capacity ∝ surface area × relief, reaching 1 (an ocean world) above it; ice
  fraction from the latitude at which the zonal temperature crosses freezing; cloud fraction by
  surface state; relief: greatest relief 20 km × (g⊕ ÷ g) scaled by a lithosphere factor from heat
  flow; heat flow from radiogenic heating (∝ rock mass × host [Fe/H] × e^(−age ÷ τ) summed over U,
  Th and K), residual formation heat and tidal heating; tectonic regime and volcanism level from
  heat flow and mass; magnetic field class from rotation, core fraction and heat flow; surface age:
  the smaller of the system's age and a resurfacing time that falls with heat flow, and the time
  since a giant impact; crater density N(> 1 km) per km² from surface age by the lunar chronology
  curve, 5.44 × 10⁻¹⁴ (e^(6.93T) − 1) + 8.38 × 10⁻⁴ T with T in Gyr (Neukum, Ivanov and Hartmann
  2001), scaled by the system's belt masses and zeroed under a thick atmosphere for small craters.
  - _Tests:_ Solar System table: (a) Earth has the liquid-water flag and Mars and Venus do not; a
    locked airless body has a day–night contrast over 300 K and Venus one under 10 K. (b) Earth
    ocean fraction 0.6–0.8, Mars and Venus 0, Europa flagged as subsurface ocean; the Moon's crater
    density exceeds Earth's by over 100; relief of Mars exceeds Earth's; every fraction is within
    0–1 and continuous in time.
  - _Accept:_ `cargo test -p hyperion-sim planetary::hooks::surface planetary::hooks::figures`.

#### P14.T25 Habitability assessment

`hooks/habitability.rs`. `HabitabilityAssessment { class, limits }`: class one of `Hostile`,
`SubsurfaceOcean`, `Marginal`, `Temperate`; `limits` a set of reasons from a closed enum (outside
the optimistic zone, outside the conservative zone, no atmosphere, pressure under water's triple
point, runaway greenhouse, snowball, gas envelope, tidally locked, high eccentricity, active host,
host too young, host leaving the main sequence within 1 Gyr, extrapolated zone). `Temperate`
requires a rocky body of 0.1–5 M⊕ in the conservative zone with liquid water at the surface. It is a
function of time. It says nothing about life.

- _Tests:_ Earth is `Temperate`, Mars `Marginal`, Venus `Hostile` with the runaway reason, Europa
  `SubsurfaceOcean`; the share of FGK hosts with a `Temperate` body is reported by the slow test and
  asserted only to lie in 0.01–0.3, since η⊕ is that uncertain.
- _Accept:_ `cargo test -p hyperion-sim planetary::hooks::habitability`.

#### P14.T26 Resource abundances

`hooks/resources.rs`, on `body.resources`. `ResourceAbundances`: a mass fraction of the accessible
outer layers for each of a closed list: iron-group metals, light lithophile metals, rare earths,
platinum group, radioactives, silicates, water ice, other ices (CO₂, NH₃, CH₄), hydrocarbons,
helium-3, deuterium. Each is a deterministic function of bulk composition, differentiation
(platinum-group elements sink into a core, so they are rich on undifferentiated small bodies and
poor in crusts), formation side of the snow line, atmosphere (helium-3 and deuterium for giants),
host [Fe/H] and [α/Fe] (α elements track [α/Fe], iron-group [Fe/H]), and age for radioactives, times
one log-normal draw per resource of 0.3 dex for what the model leaves out.

- _Tests:_ fractions are within 0–1 and sum to at most 1; iron-group abundance scales with 10^[Fe/H]
  at fixed composition; radioactives halve over 4.5 Gyr for the U-238 share; a metal-poor halo
  planet is poorer in every metal than its disc twin.
- _Accept:_ `cargo test -p hyperion-sim planetary::hooks::resources`.

### Phase F: special hosts

#### P14.T27 Substellar hosts

`planetary/hosts/substellar.rs`, per D13.

- **P14.T27.a Brown dwarfs.** `SystemContext` with `HostKind::BrownDwarf` runs T3–T8 with the
  `SubstellarCompact` row: weights `Barren` 0.5 and chain 0.5 at 0.05 M☉, the chain weight falling
  linearly to 0.2 at 13 M_J. Luminosity, radius and temperature for T12 come from the brown dwarf's
  state at age + t (plan 06's `stellar::substellar::cooling`, as plan 13 routes it), so such planets
  cool with their host and the habitable zone sweeps inward across them.
- **P14.T27.b Rogue planets.** `HostKind::RoguePlanet`: the object is body `0x0000`, derived by T16
  with no star (T_eq term zero, internal heat only: the cooling fit above 0.3 M_J, radiogenic and
  residual heat from T24 below it, and a hydrogen envelope's insulation where one survives).
  Satellites by T17–T20 with the ejection cut of D13 on `planet.origin`.
  - _Tests:_ (a) 10⁴ free-floating brown dwarfs place chains that satisfy T10.a, none with a giant,
    about half with nothing; the habitable zone of a 0.05 M☉ host moves inward with age. (b) 10⁴
    rogue planets of 0.3 M⊕–13 M_J derive without a star, and their satellites from T22.a satisfy
    T22.b's properties; a 1 M_J rogue of 1 Gyr has an effective temperature within 20% of plan 13's
    `giant_cooling`; moons of ejected planets lie inside a tenth of the birth Hill radius; body
    `0x0000` is the object, its moons are `0x0100` upward and its rings `0x0080` upward.
  - _Accept:_ `cargo test -p hyperion-sim planetary::hosts::substellar`.

#### P14.T28 Young and evolved hosts: the fate transform

`planetary/fate.rs` and `planetary/hosts/{young, evolved}.rs`, per D1, D11 and D12.
`fate::state_at(body, ctx, t) -> (BodyState, Elements)` is the only place that turns primordial
elements into elements at a time.

- **P14.T28.a Young hosts.** Formation ages on `planet.origin`: giants uniform in log between 0.5
  Myr and the disc lifetime, small planets at the disc lifetime, terrestrial planets molten to
  10–100 Myr (drawn). Before the disc lifetime the system has a `ProtoplanetaryDisc` body in belt
  slot `0xE0` carrying the disc's masses and radii and gaps at formed giants; it is
  `Destroyed { cause: Dispersed }` afterwards. `BodyState::NotYetFormed` before a body's formation
  age.
- **P14.T28.b Expansion and engulfment.** Adiabatic expansion and the engulfment test of D11, using
  the host's mass at age + t and its largest radius before that age. The destruction time is found
  by bisection on the monotone function a(t) − f R_max(t) with a fixed number of steps. Moons go
  with their planet. Circularisation (T8.e) is applied first.
- **P14.T28.c Supernovae.** At the host's death time: the planet's state vector from its elements,
  the host's velocity change from the death record's kick, the remnant's mass, then new elements
  from `elements_from_state` (T2.c) or `Unbound`. A body whose new pericentre is inside the
  remnant's Roche limit is destroyed. For a star in a binary the companion's planets see the same
  mass loss through plan 11's post-explosion orbit; circumbinary planets are treated as orbiting the
  pair's total mass.
- **P14.T28.d White dwarfs.** The pollution mark and the dusty disc of D11 on `belt.population`, the
  disc as a `DebrisDisc` body inside the white dwarf's Roche limit of about 1 R☉, present from the
  start of the white dwarf phase with a lifetime set by the cooling age.
- **P14.T28.e Second-generation planets** of neutron stars, on `planet.secondgen`, in slots `0xC0`
  upward, placed by T6–T8 with a fixed small template and derived by T16 with the pulsar's spin-down
  luminosity from plan 06 as the only irradiation.
  - _Tests, on synthetic hosts with planets placed by T8:_ (all) a body's state sequence in time is
    always a prefix of NotYetFormed → Present → Destroyed or Unbound, and elements are continuous in
    time except at a supernova. (a) No giant forms after its disc has gone; the disc body is
    `Destroyed { Dispersed }` from its lifetime on. (b) Along a 1 M☉ track, planets inside about 1
    au at birth are destroyed by the tip of the asymptotic giant branch and survivors end at a₀ × M₀
    ÷ M_wd to 10⁻⁹. (c) A planet of a star that loses over half its mass at once with no kick is
    unbound from a circular orbit; black holes from complete fallback keep bodies and kicked ones do
    not. (d) 25–50% of white dwarfs of 1–3 Gyr cooling age with a belt and a planet are polluted.
    (e) Under 1% of neutron stars have second-generation planets, none earlier than 10 Myr after the
    death, and the share is 0.005 to a Poisson interval.
  - _Accept:_ `cargo test -p hyperion-sim planetary::fate planetary::hosts`.

#### P14.T29 Tidal and encounter stripping

`planetary/hosts/strip.rs`, per D14. `strip_radius(ctx)` = the smaller of 0.49 × the system's sphere
of influence (plan 09's rule: the tidal radius at pericentre for a system orbiting the central black
hole, the feature rule inside a feature) and, for a feature member, the encounter radius: the a that
solves 1 ÷ (n π a² (1 + 2GM ÷ (a σ²)) σ) = age, a quadratic in a (Adams and Laughlin 2001; Spurzem
et al. 2009 for the cross-section's calibration). The radius enters T3.b as a truncation of the disc
and T21.d as the halo's bound, and is asserted again after placement.

- _Tests:_ a system with a semi-major axis of 0.01 ly about a Milky-Way-mass black hole has nothing
  beyond 2.7 au, the brainstorm's figure, and nothing generated beyond 0.49 of it, 1.32 au; a member
  of a globular core of 10⁴ systems per cubic light-year keeps nothing beyond a few au; a field star
  at 26,000 ly is unaffected inside 10⁴ au.
- _Accept:_ `cargo test -p hyperion-sim planetary::hosts::strip`.

### Phase G: events and assembly

#### P14.T30 Assemble the generator

- **P14.T30.a `generate`.** `planetary/system.rs`. `generate` = zones (T9) → per zone: disc (T3),
  class (T4), placement (T8) → slots assigned (D3) → per planet: `generate_satellites` (T22.a) →
  belts and halo (T21) → second generation (T28.e); for a free-floating host, T27's path.
  `PlanetarySystem` holds only primordial state, which is what the server may cache ("state at the
  epoch, never positions at some time"). `generate_planets` stops before the satellites.
  - _Tests:_ determinism (two runs equal); order independence through plan 01's
    `assert_order_independent` (A then B equals B then A equals B alone); `generate_planets` equals
    `generate` with satellites removed; `generate_satellites` for a planet equals that planet's
    children in `generate`, for 1,000 systems; every index in a system decodes and is unique; a
    system of plan 11 with a brown-dwarf companion gains no body in slot `0x00`.
  - _Accept:_ `cargo test -p hyperion-sim planetary::system::generate`.
- **P14.T30.b Queries at a time.** Derivation and hooks are evaluated lazily by `body_at` and
  `snapshot_at`, since they depend on time: fate (T28), then `derive_body` (T16), then hooks
  (T23–T26). `position_at` composes the host's position from plan 11's `star_positions_at` with the
  body's and its parent's Kepler states. `habitable_zone_at` per T12.b on the hosts' states at the
  time.
  - _Tests:_ `body_at` for an unused index returns `NoSuchBody`; in a system not yet born every body
    resolves, as `NotYetFormed`; `snapshot_at` equals `body_at` body by body; a moon's position is
    its planet's plus its own offset; T10.a, T16.b and T22.b rerun on whole generated systems, and
    every body lies inside its stable zone and the strip radius at ±H.
  - _Accept:_ `cargo test -p hyperion-sim planetary::system`.
- **P14.T30.c Labels.** `BodyLabel` per D22 in `planetary/label.rs`.
  - _Tests:_ the golden Solar-like layout labels `A b` to `A i`, a moon `A d II`, a belt `BELT 1`;
    labels are unique within a system and stable under `degrade` down to `MassAndOrbit`.
  - _Accept:_ `cargo test -p hyperion-sim planetary::label`.

#### P14.T31 Events on bodies

`planetary/events.rs`, per D15, using plan 06's two constructions with `BodyId` (or `SystemId` for
comets) as the subject of plan 01's `EventKey::derive`. T31.a and T31.b register the five event tags
of Provides, each in `rng/tags.rs` (scope `Event`) and in `id/event_tags.rs` with its number, as it
is first used.

- **P14.T31.a Poisson bins.** `body.impact`: rate above a stated energy = the belts' number flux at
  the body × its gravitationally focused cross-section, each event with an energy from the size law
  and a location on the body. `body.eruption`: rate rising with T24's heat flow, zero below a
  threshold, each with a duration found by looking back a fixed number of bins. `system.comet`: rate
  from T21.d, each event a comet with perihelion time, perihelion distance (uniform to 5 au × √L),
  isotropic orientation, eccentricity 1 − 10⁻⁴ to 1, and nucleus size; its position at any time is
  T2.b's, and it is listed while inside 30 au × √L.
- **P14.T31.b Monotone phases.** `body.storm` for giants with P = the orbital period (Saturn's great
  white spots) and `body.duststorm` for desert planets with thin atmospheres with P = the orbital
  period, a skip mark of two in three and events held to the perihelion season (Mars).
- **P14.T31.c `events_between`** merges them, sorted by time then `EventId`.
  - _Tests:_ (a, b) counts over 10⁴ years match the rates (Poisson interval); (a) a Solar System
    input gives a Shoemaker–Levy-class impact on Jupiter every 50–500 years and 5–20 comets a year
    inside 5 au; (b) dust storms fall only in the perihelion season and about one Mars year in three
    has one; (c) the same events come back for a window asked whole, in halves, and backwards (plan
    06's `events::testing::assert_order_independent`); `EventId`s round-trip through plan 01's text
    form; no event changes any `body_at` result.
  - _Accept:_ `cargo test -p hyperion-sim planetary::events`.

#### P14.T32 Golden systems

- **P14.T32.a The search helper and the pinned IDs.** `crates/hyperion-sim/tests/common/mod.rs`
  gains `find_system(galaxy, predicate, budget)`, which walks cells in a fixed order and returns the
  first ID whose generated system satisfies a predicate. `tests/planetary_golden.rs` holds one
  predicate and one pinned ID constant for each of the twenty-four descriptions below, found once
  with the helper. Hosts that the grid holds too thinly to find in the budget (the nuclear-cluster
  member, the globular member, the layer-E death inside the clock window) come from plan 09's member
  and catalogue sources by the same helper.
  - _Tests:_ every pinned ID resolves and its system satisfies its own predicate, so that after a
    version bump a golden still is what its name says; the search itself is a slow test
    (`#[ignore = "slow: searches for the golden systems"]`) that reproduces the pinned IDs.
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_golden pinned` and `just test-slow`.
- **P14.T32.b The goldens.** `tests/golden/planetary/`, through plan 01's `golden!`. Each golden
  holds the full `snapshot_at` at the epoch and at +H and the events of one century. Bump
  `GENERATOR_VERSION` here.
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_golden`; changing any constant in
    `planetary` fails it.

The twenty-four, at Milky Way parameters: an M dwarf with a resonant chain, a metal-rich G dwarf
with a hot Jupiter, a Solar-like system, an eccentric giant, a halo star, a close binary with a
circumbinary planet, a wide binary with planets around both stars, a hierarchical triple, a T Tauri
star with its disc, a subgiant, a red giant mid engulfment, a white dwarf with a polluting belt, a
neutron star with second-generation planets, a fallback black hole with survivors, a free-floating
brown dwarf with a chain, a star with a brown-dwarf companion of plan 11 and planets inside its
stable zone, a rogue Jupiter with moons, an ejected rogue Earth, a nuclear-cluster member stripped
to a few au, a globular member, a layer-E system due to die inside the clock window (evaluated
before and after), and three ordinary fillers.

#### P14.T33 Benchmark and slow statistics

- **P14.T33.a Benchmark.** `crates/hyperion-sim/benches/planetary.rs` (Criterion, under
  `just bench`): `generate` plus `snapshot_at` for (i) the golden Solar-like system, (ii) a mixed
  sample of 1,000 field systems, (iii) 1,000 rogue planets, (iv) `position_at` for one moon. Target
  from the brainstorm: a full system with its bodies in under a millisecond, measured as the mean of
  (ii) per system with plan 06's and plan 11's stars already built, since the target is this
  stage's; recorded in the bench's doc comment with the measured figure. `position_at` should be
  well under a microsecond. A miss is a finding, not a CI failure.
- **P14.T33.b Planets per star, end to end.** The slow test of T10.b rerun on `sample_contexts`
  (real systems of a Milky-Way-parameter galaxy, binaries and evolved hosts included) and reported
  by population: planets per system, share of systems with any planet, giants by [Fe/H] bin. The
  asserted figures are those of T10.b restricted to main-sequence single FGK and M hosts; the rest
  is printed for review.
  - _Accept:_ `just bench` prints the four figures; `just test-slow` passes.

### Phase H: queries and protocol

#### P14.T34 Detail levels and degradation

`planetary/record.rs`, per D16. `BodyRecord` with nested sections `identity`, `orbit`, `bulk`,
`surface`, `hooks`, each `Option` except identity; `DetailLevel` ordered; `degrade(level)`;
`SystemSnapshot::degrade(level)` applies it to every body and drops belts' member lists below
`Bulk`. `Contact`, the brainstorm's "unresolved contact", keeps the ID and the position: the kind is
replaced by `BodyKind::Unresolved` and the label is withheld. `MassAndOrbit` is its "mass and orbit
only".

- _Tests:_ `degrade` is idempotent and monotone (`degrade(a).degrade(b)` = `degrade(min(a, b))`); a
  `MassAndOrbit` record serialised to JSON contains no radius, temperature or composition key.
- _Accept:_ `cargo test -p hyperion-sim planetary::record`.

#### P14.T35 Wire types

`crates/hyperion-protocol/src/` in plan 04's module layout, following its "Extending the
convention": a variant of `RequestBody` and of `ResponseBody` per kind, the kind's string in
`REQUEST_KINDS`, a wire-form test each, then `just gen-protocol`.

- **P14.T35.a IDs and orbits.** `BodyIdHex` beside `SystemIdHex`, in plan 01's string form, with
  `ParseBodyIdHexError`;
  `BodyOrbitDto { parent: BodyIdHex, orbit: OrbitDto, mu_m3_s2, valid_until }`, angles in the system
  frame for planets and in the parent's frame for moons, as the sim has them; `DetailLevelDto` as
  snake-case strings.
- **P14.T35.b Records.** `BodySummaryDto` (identity, label, kind, state, orbit, and the `bulk`
  section when granted), `BodyDetailDto` (all granted sections, hooks included; the surface seed as
  16 hex digits), `BeltDto`, `ZoneDto` (stable zones, snow line, system plane), `HabitableZoneDto`,
  `BodyEventDto` (a comet event carries its elements and a track of positions sampled by the server,
  since the client does not propagate open orbits). Every quantity's field name carries its SI unit.
- **P14.T35.c Requests and responses.** The three pairs under Provides. `SystemBodiesDto` carries
  `granted: DetailLevelDto`, the hosts as plan 06's `SystemSummaryDto` with plan 11's
  `HierarchyDto`, the zones, and a flat body list in index order (the tree is rebuilt from
  `parent`). `ErrorCode::UnknownBody`. `body_events` is reserved by plan 04 like the other two: its
  string goes into `REQUEST_KINDS` with the other two, and plan 04's
  `request_kinds_lists_every_variant` test then covers it.
  - _Tests:_ one wire-form test per request, response and record type, as `rust-dev.md` requires,
    each in the subtask that adds the type; (a) `BodyIdHex` round trip and rejection of malformed
    strings; (c) `REQUEST_KINDS` holds the three new strings.
  - _Accept:_ `cargo test -p hyperion-protocol`; `just gen-protocol-check` passes.

#### P14.T36 Server handlers

`crates/hyperion-server/src/`, beside plan 06's `system_summary` handler.

- **P14.T36.a System cache.** A `SharedByteLru` keyed by (galaxy key, `SystemId`) holding
  `Arc<(SystemContext, PlanetarySystem)>`, with `HeapBytes` implemented from body counts, filled
  under `SingleFlight` so that two consoles opening one system generate it once.
- **P14.T36.b Handlers.** `system_bodies`, `body_detail`, `body_events`: every inbound ID goes
  through the sim's `resolve` and `BodyIndex::decode`; generation runs on the `CpuPool` at
  interactive priority; the result is evaluated at the request's time, degraded to the requested
  level and returned. `body_events` refuses windows longer than 1,000 years with `bad_request`
  naming the field. A full queue answers with plan 04's `queue_full`.
  - _Tests:_ (a) inserting systems past the byte bound evicts and never exceeds it; two concurrent
    requests for one system generate it once (a counter, not timing). (b) Integration tests over a
    real socket (bound to `127.0.0.1:0`, every wait under a timeout): a request for a golden system
    returns its golden summary; an unknown body returns `unknown_body`; a malformed body index
    returns `bad_request` naming the field; a `detail` of `mass_and_orbit` returns no bulk section;
    the same request twice is served from the cache (the counter again).
  - _Accept:_ `cargo test -p hyperion-server system_bodies`.

#### P14.T37 Client protocol helpers

`packages/protocol/src/`: `parseBodyId`, `formatBodyId`, and the decoding guards for the three new
response kinds if plan 04's decoder needs them listed. The generic `request` is unchanged.

- _Tests:_ Vitest: fixtures copied from the Rust wire-form tests decode; a malformed body ID is
  rejected.
- _Accept:_ `pnpm --filter @hyperion/protocol test`, `pnpm typecheck`.

### Phase I: the `SYSTEM` display

Everything follows `docs/frontend/ux-guidelines.md` and the conventions plan 05 set for the local
chart: orthographic projection, reference plane, stalks, filled above the plane and open below, no
idle motion, DOM text, keyboard for everything.

#### P14.T38 Style guide edits and formatters

- **P14.T38.a Guide.** In `docs/frontend/ux-guidelines.md`: add `d` as a unit for orbital and
  rotation periods beside plan 05's `yr` (M⊕ with its drawn `⊕` is plan 13's entry, and there is no
  Jupiter-mass unit, D19); the display-time format of D24, marked as needing the owner's
  confirmation; under spatial displays, the orbit map's conventions, which extend what the guide
  already says of it (thin vector lines, a faint `--line` grid or rings, scale, orientation and
  frame always shown, dashes for predicted paths, `--target` for commanded ones, shape for type):
  orbits are solid `--line` ellipses, reference marks like range rings and not predictions, so never
  dashed, and the selected body's orbit is a solid `--text` hairline because it carries meaning;
  zones and belts are labelled annuli drawn as their two edges in `--line`, a belt's edges joined by
  short radial ticks every 10°, with no fill, hatch or dots, since hazard striping is the guide's
  only pattern fill; the mandatory `BODIES NOT TO SCALE` label; the reference plane of D21 and its
  label; the body symbols of T42, added to the one ship-wide symbol set; and D24's rule that bodies
  move only when the display time does.
  - _Accept:_
    `grep -n "BODIES NOT TO SCALE\|SYSTEM PLANE\|DISPLAY TIME" docs/frontend/ux-guidelines.md` finds
    each edit; Prettier passes on the guide.
- **P14.T38.b Formatters.** In plan 05's `lib/format.ts`: `formatPeriod` (`d` under 1,000 days, `yr`
  above), `formatBodyDistance` (the guide's scaled units from km to AU with hysteresis),
  `formatTemperatureK`, `formatPressure` (Pa, kPa, MPa), `formatGravity` (m/s²),
  `formatUniverseTimeDhms` (D24; years, then `days/hh:mm:ss`, from a `UniverseTime`'s integer
  seconds, never through a float of seconds). Radii in km with digit grouping. Masses through plan
  13's `formatMassMearth`.
  - _Tests:_ Vitest for each formatter's boundaries and hysteresis; `formatUniverseTimeDhms` at the
    epoch, one second before it, and at ±H.
  - _Accept:_ `pnpm test`, `pnpm lint`.

#### P14.T39 Orbit mathematics in TypeScript

`apps/hyperion/src/renderer/src/lib/orbit.ts`, pure functions per D18: `solveKepler`,
`positionAt(orbit, timeS)` in the parent's frame, `orbitPolyline(orbit, segments)` with segments
spaced evenly in eccentric anomaly so that pericentre stays smooth, and
`composePosition(bodies, id, timeS)` walking the parent chain. Near-parabolic comets use the
server-supplied sampled track instead and are not propagated client-side.

- _Tests:_ Vitest against a fixture of 32 states exported by a test of T2.a, agreeing to 10⁻⁹
  relative; the polyline closes; a moon's composed position equals planet plus offset.
- _Accept:_ `pnpm --filter hyperion exec vitest run src/renderer/src/lib/orbit.test.ts`.

#### P14.T40 Spatial-view marks for paths and annuli

In plan 05's `spatial/`, all additive, so that the `GALAXY` display's draw lists are unchanged:

- **P14.T40.a Marks.** Two mark kinds in `marks.ts`: `PathMark` (a polyline in 3D, a `role` of
  `"reference"` or `"selected"` that picks `--line` or `--text`, and a label anchor) and
  `AnnulusMark` (inner and outer radius on the reference plane, a `ticks` flag for belts, and a
  label; T38.a's drawing rule). `SpatialScene` gains optional `paths` and `annuli`, absent meaning
  none. `buildDrawList` handles both with the existing projection and D12's far-to-near order (a
  path is split where it crosses the plane, an annulus is drawn with the plane), and `pick` ignores
  them. `symbols.ts` gains the closed outlines `pentagon` and `hexagon` for T42.
  - _Tests:_ Vitest on the projection of a circular path tilted 60° (an ellipse with axes 1 and
    0.5), on draw order either side of the reference plane, on an annulus's two edges and ticks, and
    on the new outlines (closed, centred, inside the unit circle, open and filled differing only by
    fill); plan 05's tests pass untouched.
- **P14.T40.b The tilted frame.** `frame.ts` gains `planeFrame(normal, reference): LocalFrame`:
  `north` is the unit normal, `coreward` the reference direction projected onto the plane and
  normalised (falling back to any perpendicular when the two are within 10⁻⁶ of parallel),
  `spinward` their cross product, right-handed as `localFrameAt`'s is. `AxisTriad` gains the
  optional `axes` prop of Provides. `PlaneSpec` is unchanged (D21).
  - _Tests:_ the frame is orthonormal for 1,000 random normals; with the galactic north as normal it
    equals `localFrameAt`'s; a scene built on a frame tilted 30° draws its grid as plan 05 draws the
    galactic one in that frame's own coordinates; with `axes` given the triad's `NORTH` follows the
    galactic vector and not the plane's normal.
  - _Accept (both):_ `pnpm test`.

#### P14.T41 The display shell

`apps/hyperion/src/renderer/src/displays/system/`, beside plan 05's `displays/galaxy/`.

- **P14.T41.a Navigation and requests.** Add `"system"` to `DisplayId` and its case to the
  navigation bar. The `GALAXY` display's selected-system readout gains an `OPEN SYSTEM` button that
  switches display with the system's ID and the chart's time. A hook
  `useSystemBodies(systemId, timeS, detail)` owns the request, cancels on change, and re-requests
  per D18.
- **P14.T41.b Data states.** No system selected (`NO SYSTEM SELECTED`), and every non-`ok`
  `RequestState` through plan 05's shared `RequestStatus` (pending, rejected with the typed reason,
  timed out, link down), which keeps the guide's ban on spinners and "Loading…"; "no such system";
  `NO SYSTEM YET` for an unborn host; and an empty system (`NO BODIES`), each as text, never as an
  empty canvas. On loss of the link the last data stay, marked stale as the guide requires. The
  granted detail level is always shown (`DETAIL: MASS AND ORBIT ONLY`).
  - _Tests:_ Testing Library with `FakeWebSocket`: (a) opening from the galaxy display issues one
    `system_bodies` request with the right ID and time, and `F`-key navigation reaches the display;
    (b) each state renders its text.
  - _Accept:_ `pnpm test`.

#### P14.T42 The orbit map

`displays/system/OrbitMap.tsx` on the spatial view.

- **P14.T42.a Marks.** Hosts and bodies as point marks at `composePosition`, orbits as `path` marks,
  stable-zone limits, the snow line and the habitable zone as `annulus` marks that can be switched
  off, belts as ticked annuli (T38.a), the cometary halo as a labelled outer ring only when it is
  inside the view. The selected body's orbit is the one `"selected"` path. Symbols, where shape
  encodes type and one shape means one thing on every display: hosts keep their symbols from the
  registry of plans 06 and 13 (circle, ringed circle, diamond for a white dwarf, triangle for a
  neutron star, square for a black hole). A planet, bound or free-floating, is plan 13's
  `triangle-down`, with plan 05's size class telling giant (3) from smaller planet (1) from dwarf
  planet (0), under a `SYMBOLS NOT TO SCALE` legend; a moon is a `pentagon`; an unresolved contact
  is a `hexagon`. Both new outlines are closed, because plan 05's filled-above, open-below rule
  needs a shape that can be filled, which rules out a plain cross. The list names every kind in
  words, so shape is never the only signal. Destroyed and unbound bodies are not drawn but stay in
  the list. Colour stays free: only the selection reticle and, later, status use it.
- **P14.T42.b Frames and scale.** Two frames: `SYSTEM BARYCENTRIC`, drawn on the `SYSTEM PLANE` of
  D21, and, when a planet is focused with `FOCUS BODY`, `BODY <designation>`, in which its moons and
  rings are drawn and distances switch to Mm and km. Distances are true to scale with a 1-2-5 scale
  bar; symbols are not, and the display says `BODIES NOT TO SCALE`. Zoom presets `INNER` (fits the
  outer habitable-zone limit or the fifth body), `ALL` (fits the outermost planet) and `BELTS` (fits
  the outermost belt), plus plan 05's `TOP`, `SIDE`, `FRONT` and oblique views relative to the
  system plane. The frame name, time, azimuth, elevation and triad are always shown.
- **P14.T42.c Interaction.** Plan 05's controls unchanged. Picking selects the nearest body symbol
  within the guide's tolerance; orbits are not pickable.
  - _Tests:_ (a) pure mark-building functions unit-tested (a golden system's summary in, marks out;
    no destroyed body among them; one selected path at most); Testing Library: toggling
    `HABITABLE ZONE` removes its annulus from the mark list. (b) Focus switches the frame label and
    the scale bar's unit; `BODIES NOT TO SCALE` and the legend are present. (c) A click within
    tolerance of a body selects it and one on an orbit selects nothing.
  - _Accept:_ `pnpm test`; by eye against the golden Solar-like system and the circumbinary one.

#### P14.T43 Body list and readouts

`displays/system/{BodyList, BodyReadout}.tsx`.

- **P14.T43.a List.** A tree in the DOM: hosts, their planets by semi-major axis, moons and rings
  under their planet, belts and their members, with position and total as the guide asks of lists.
  Arrow keys move, Enter selects, and selection is shared with the map. Each row: designation, kind
  in words, semi-major axis, and state in words when not present (`DESTROYED`, `NOT YET FORMED`,
  `UNBOUND`).
- **P14.T43.b Readout.** An `output` element, filled by a `body_detail` request: designation and ID;
  label; kind and class; state; mass and radius; density and surface gravity; semi-major axis,
  period, eccentricity, inclination; distance from its primary now; equilibrium and surface
  temperature; pressure and atmosphere; rotation period and locking state; composition; the global
  figures; the habitability class with its reasons in words; resource abundances as a small table;
  for the system as a whole, the architecture class and the zones. A section the granted level
  withholds shows `NOT RESOLVED`, never a blank or a zero, and a single value missing inside a
  granted section is the guide's em dash in `--text-muted`.
- **P14.T43.c Events.** A list of the body events of the century around the display time, from a
  `body_events` request, each with its time in the chart's time system and a countdown in the
  guide's `T-` form.
  - _Tests:_ Testing Library: (a) arrow keys and Enter select, the list shows position and total,
    and a destroyed body's row says `DESTROYED`; (b) keyboard selection updates the readout, a
    `mass_and_orbit` fixture shows `NOT RESOLVED` for the surface section, and units appear on every
    value; (c) an event before the display time counts `T+` and one after it `T-`, each with its
    time-system label.
  - _Accept:_ `pnpm test`.

#### P14.T44 Time control

`displays/system/TimeControl.tsx`, per D24. These controls change what the console displays and
nothing on the ship, and are styled as display controls, as the guide requires.

- **P14.T44.a Stepping.** The display time starts at the time the display was opened with and is
  held. Buttons step it by a selectable amount (1 h, 1 d, 10 d, 1 yr, 10 yr, 100 yr) backwards and
  forwards, and `RESET` returns to the opening time. The time is shown as
  `DISPLAY TIME UT +12 yr 183/14:08:33` (`formatUniverseTimeDhms`, with plan 05's
  `TIME_SYSTEM_LABEL`), and is held to the clock window ±H with the limit stated when reached
  (`CLOCK WINDOW LIMIT`). A step redraws once, through plan 05's redraw scheduler, without a
  transition. Every control has a key binding shown on it.
  - _Tests:_ fake timers: stepping changes positions by the expected mean anomaly; the time never
    leaves ±H and the limit text appears; crossing a year or a body's `valid_until` triggers exactly
    one re-request (D18); no frame is requested after the redraw that follows a step.
- **P14.T44.b `RUN` and `HOLD`.** `RUN` advances the display time at a selectable rate (1 h, 1 d, 10
  d or 1 yr per second) and `HOLD` stops it; they are a congruent pair in that order, and the
  current mode and rate are always shown (`HOLD`, `RUN 1 d/s`). The display opens in `HOLD`, never
  starts `RUN` on its own, and drops to `HOLD` at ±H, on leaving the display and on loss of the
  link. While running, redraws are coalesced through `requestAnimationFrame`, numeric readouts
  update at the guide's 4 Hz without tweening, and nothing moves when held, so the display has no
  idle motion. Under `prefers-reduced-motion`, `RUN` steps at 4 Hz without interpolation.
  - _Tests:_ fake timers: `RUN` then `HOLD` leaves no timer or frame callback alive; unmounting
    while running does the same; at +H the mode reads `HOLD`; with reduced motion there are four
    paints a second and no frames between them; the readout changes at most four times a second.
  - _Accept (both):_ `pnpm test`; by eye: a hot Jupiter circles in seconds at 1 d/s while the cold
    giant barely moves, and nothing moves in `HOLD`.

## Verification

The plan is done when all of the following hold.

- **Property tests**, the brainstorm's planetary four and this plan's own (ordinary suite on
  thousands of systems, `just test-slow` on a million), for any seed and ID and any time in ±H: no
  overlapping orbits (T10.a); moons inside Hill spheres and outside Roche limits (T22.b); rings
  inside Roche limits (T20, T22.b); no planet hotter than its star (T16.b); all four again on whole
  generated systems (T30.b); every body inside its stable zone (T9) and inside the strip radius
  (T29, T30.b); body state sequences are prefixes of formed → present → gone (T28); indices unique
  and decodable (T30.a). State is continuous in time (T16.b, T28), and both event constructions
  return the same events in any order of asking (T31).
- **Statistical tests** under `just test-slow`, each against a published figure in the window where
  the survey was complete: planets per star for FGK and M hosts, hot-Jupiter and giant occurrence,
  the giant–metallicity slope of 2.0 ± 0.3, M-dwarf compact chains common and giants rare, small
  planets flat in metallicity down to −0.8 (T10.b, T33.b); adjacent-size correlation of 0.5–0.8
  (T7); the radius valley (T16.c); debris-belt incidence (T21); white dwarf pollution and the rarity
  of pulsar planets (T28); ring incidence (T20).
- **Solar System table.** The derivation functions fed with real masses and orbits reproduce radii,
  temperatures, atmosphere retention, locking, Roche and Hill radii, belts and event rates within
  the stated tolerances (T11–T15, T17, T21, T24, T25, T31). This is the quickest guard against a
  unit error.
- **Golden systems** (T32): twenty-four pinned IDs covering every host kind, at the epoch and at +H,
  with a century of events.
- **The architecture table** is written, sourced and pinned: the rendered module documentation and
  the tests of T4.c, which assert the brainstorm's two statements (giant occurrence as 10^(2[Fe/H]);
  M dwarfs rarely host giants and often host compact chains) on the table itself, before any planet
  is placed, and T10.b, which asserts them again on generated systems.
- **Knowledge-friendly records** (T34): `degrade` to "unresolved contact" and "mass and orbit only"
  and the levels between, idempotent and monotone, with nothing of a withheld section on the wire
  (T35, T36).
- **Order independence and determinism** (T22.a, T30.a): a planet's satellites alone equal the same
  inside the system; A then B equals B then A equals B alone.
- **Benchmark** (T33.a): a full system with bodies under a millisecond, the brainstorm's target; a
  regression is a finding.
- **The guide** (T38.a, T40–T44): the `SYSTEM` display shows scale, orientation, frame and time at
  all times, says `BODIES NOT TO SCALE`, uses no dashes, fills or patterns for orbits, zones and
  belts, moves nothing while held, and opens held.
- **Wire forms** pinned for every new message and record (T35), bindings fresh
  (`just gen-protocol-check`).
- **By eye**, in the `SYSTEM` display: the golden Solar-like system reads as one (terrestrials,
  belt, giants, outer belt, zone annuli in the right places); the circumbinary system shows both
  stars moving inside the inner stable limit; the red giant's inner planets are listed as destroyed
  after the right time when the time control is run forward through the clock window for the system
  due to change inside it; a rogue planet opens with its moons and no star; a nuclear-cluster member
  shows a system cut off at a few au; `MASS AND ORBIT ONLY` hides exactly what it should.

## Generator version

Adding bodies moves no star: every draw here is on a new domain tag, and nothing upstream reads this
stage. The version is bumped once, in T32, when the first planetary goldens are committed, and again
by any later task that changes a body. What this plan reserves so that later work need not disturb
it:

- The `body_index` layout of D3, with slots `0x01`–`0xBF`, `0xC0`–`0xCF` and `0xE0`–`0xEF` in use,
  `0xD0`–`0xDF` and `0xF0`–`0xFF` reserved (pinned bodies and stations are overlays and need none),
  sub-indices `0x90`–`0xFF` of a planet reserved.
- The domain tags listed under Provides and the event tags `0x0400`–`0x0404`, the rest of plan 06's
  block `0x0400`–`0x04FF` staying free for later body events. Surface seeds depend on `body.surface`
  and the `BodyId` alone, so the map generator's output survives any change to derivation.
- `DetailLevel` as an ordered enum with room between levels on the wire (string names, not numbers).
- Parameters that belong to the generator version and are named constants in one place
  (`planetary/params.rs`): the class weight table, the spacing floors, ring probabilities, the
  pulsar-planet probability, `SATELLITE_STABILITY_FRACTION`, the white dwarf pollution fit.

## Risks and open points

- **The class table is a model of incomplete surveys.** `TerrestrialOnly` and `Barren` cannot be
  told apart by any current survey, so their split (0.35 to 0.24) is a judgement. The tests
  deliberately assert only inside survey windows. If the owner wants more or fewer empty systems,
  that split is the dial, and turning it changes nothing that is tested.
- **The floor of 10 forbids a few real systems.** Kepler-11 b and c sit at 9.4 mutual Hill radii,
  and TRAPPIST-1's tightest pairs are lower still, protected by resonance. D7 keeps the brainstorm's
  floor and lets resonant snapping happen only where the floor holds, so the tightest observed
  chains are slightly under-produced. Lowering the floor to 7 for resonant pairs would fix it and is
  a one-constant change with a version bump.
- **The brainstorm's floor against the Solar System.** Read literally, "rejected below about 10–12"
  excludes Jupiter and Saturn at 8. D7 applies it to small planets only. Reported to the plan's
  commissioner as an interpretation.
- **"Stripped to the tidal radius" against dynamics.** D14 generates inside 0.49 of the radius and
  asserts the brainstorm's weaker bound. Also reported.
- **The snow line's luminosity.** D6 uses the zero-age main-sequence value. Pre-main-sequence stars
  are several times brighter while planetesimals form, which would move M dwarfs' snow lines outward
  by a factor of 2–3. If plan 06's contraction tracks are good enough at 1 Myr, the alternative is
  one line; it needs the 2.7 au coefficient recalibrated, which is why it is not the default.
- **What the stellar stage must expose.** Plan 06 provides `max_radius_until`,
  `max_luminosity_until` and the zero-age main-sequence fits for this plan. It has no [α/Fe] and no
  X-ray and ultraviolet history. T1.a supplies both as closed forms with no draw, so no star moves;
  if plan 06 later draws an [α/Fe] of its own, the closed form becomes its mean and resource
  abundances change with a version bump.
- **Interacting binaries.** D10 intersects the stable zones at birth and at the evaluated state.
  Planets of systems that went through a common envelope are probably over-retained. They are rare,
  and the golden set includes none on purpose; revisit with plan 11's author.
- **Plan 05's spatial view** is unit-agnostic, which the orbit map needs, and its plane follows
  `scene.frame`, so a tilted plane is a frame and not a change to `PlaneSpec` (D21, T40.b). Its
  `LocalFrame` names its axes coreward, spinward and north; on the orbit map those names hold only
  in projection, which the separate triad makes plain. If plan 05's code turns out to read the
  galactic frame anywhere but through `scene.frame`, T40.b grows by about a day.
- **Body indices across four plans.** Plan 01 leaves the meaning of the `u16` to this plan, plan 06
  fixes index 0 as the primary, plan 11 numbers the other components 1–15 in hierarchy order with
  `STAR_BODY_INDEX_END` = 16, and plan 13 states the free-floating convention. D3 is consistent with
  all four as written. What plan 11 must keep: every stellar-level body, brown-dwarf companions
  included, inside raw values 0–15, and nothing of its own at 16 or above.
- **The radius valley may not emerge.** D8 lets it fall out of escape and does not impose it. If
  T16.c fails after its two dials are exhausted, the finding goes to the owner; a drawn gap would
  contradict the brainstorm's "computed, not rolled".
- **`body_events` is not among the kinds plan 04 reserved.** Adding a kind is plan 04's ordinary
  extension path, followed step by step in T35.c, and plan 04's reserved list should name it. The
  alternative, folding events into `body_detail`, was rejected because the events panel asks for a
  window of time and the detail request for an instant.
- **Symbols.** Plan 06 uses circle, ringed circle, diamond, triangle and square, and plan 13 adds
  `triangle-down` for a free-floating planet. T42 makes that shape mean "planet" everywhere and adds
  only `pentagon` and `hexagon`. Three sizes of one shape must stay legible at plan 05's sizes; if
  they do not by eye, a fourth outline for giants is the fallback, with a guide edit.
- **Time control and the guide.** `RUN` is continuous motion at the operator's command, which the
  guide permits only as the showing of a changing state (D24). If the owner reads the guide more
  strictly, T44.b is dropped and stepping alone remains; nothing else depends on it.
- **Performance.** The millisecond target includes moons, and a Solar-like system has tens of them.
  Derivation is lazy (T30), but `snapshot_at` evaluates every body. If it misses the target, the
  first lever is caching the host state per snapshot, the second is deriving hooks only on
  `body_at`.
- **Dependencies in the roadmap.** The roadmap lists 06, 11 and 13. This plan also reads plan 09
  (sphere of influence, a member's environment), which precedes it transitively through plan 11. No
  roadmap change is needed.
- **Linear scale on the orbit map.** A compact chain with a cold giant spans three decades of
  distance, which the zoom presets handle but a single view does not. A labelled logarithmic radial
  scale is the obvious addition and was left out to keep the first display true to scale.
- **Sources beyond the brainstorm's list.** The occurrence-rate and small-body literature named in
  the tasks (Cumming et al. 2008; Johnson et al. 2010; Dressing and Charbonneau 2015; Zhu and Wu
  2018; Kraus et al. 2016; Canup and Ward 2006; Domingos et al. 2006; Mustill and Villaver 2012;
  Wyatt 2008; Owen and Wu 2017; and the others) was cited from memory without web access. Every
  figure taken from them is marked for re-checking in its task, and T4's written table is where the
  checked values and full references end up.

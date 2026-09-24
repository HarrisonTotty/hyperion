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
plan 14 reuses them. By ruling 33 of 2026-09-22, P11.T3.a builds them to this plan's precision from
the start (T2.a) and adds `units::GravitationalParameter` (m³ s⁻²), beside plan 01's bare
`units::consts::GM_*` constants. This plan adds, without touching plan 11's output:

```rust
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
pub struct SystemContext { /* id, host kind, stars as plan 06's `StarModel`s (ruling 34),
                              hierarchy, [Fe/H], [alpha/Fe], age at epoch, tidal radius,
                              encounter environment */ }
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
pub mod disc { pub struct Disc; pub fn derive(.., lifetime: Years, ..) -> Disc; // T3; ruling 33
    pub fn snow_line(l: SolarLuminosities) -> Metres; }
pub mod architecture { pub enum ArchitectureClass { Barren, TerrestrialOnly, CompactMulti,
    CompactWithColdGiant, SolarLike, EccentricGiant, WarmGiant, HotJupiter, SubstellarCompact };
    pub struct ClassWeights; pub fn class_weights(m: SolarMasses, feh: Dex) -> ClassWeights;
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
pub struct BodyRecord { /* id, label, kind, parent, state;
                          orbit, bulk, surface, hooks: each a Section<_> */ }
pub enum Section<T> { Ok(T), NotResolved, NotModelled, NotApplicable }   // ruling 34, T34
pub enum DetailLevel { Contact, MassAndOrbit, Bulk, Surface, Full }   // ordered
impl BodyRecord { pub fn degrade(&self, level: DetailLevel) -> BodyRecord; }
pub enum BodyKind { Planet(PlanetClass), DwarfPlanet, Moon(MoonOrigin), Ring, Belt(BeltKind),
    CometaryHalo, ProtoplanetaryDisc, DebrisDisc,
    Unresolved }                                  // only ever produced by `degrade(Contact)`
```

Domain tags, as entries of plan 01's single `domain_tags!` registry in `rng/tags.rs` under a "Plan
14" heading, never renamed. The heading goes at the end of the list, whatever the plan number,
because the macro's order fixes `tags::ALL`, which `tests/golden/rng/tags.golden` pins; the task
that adds a tag regenerates that golden with `domain_tags_are_pinned`. Plan 01 requires at least one
full stop in a name and one scope per tag, so the first draft's `ring`, `belt` and `halo` are
`ring.system`, `belt.population` and `cometary.population` (not `halo.`, which is the galactic
halo's prefix in plans 02 and 08).

- Scope `System` (opened with `ObjectKey::from(SystemId)`, the host and slot in the draw number,
  D4): `planet.disc` (a disc's masses, radii and corotation period, and a lifetime only for a
  circumbinary disc: a circumstellar disc's lifetime is its star's `star.disc_lifetime`, ruling 33),
  `planet.plane`, `planet.class`, `planet.count`, `planet.spacing`, `planet.mass`,
  `planet.secondgen`, `belt.population`, `cometary.population`.
- Scope `Body` (opened with `ObjectKey::from(BodyId)`): `planet.orbit`, `planet.radius`,
  `planet.volatiles`, `planet.spin`, `planet.origin`, `moon.count`, `moon.mass`, `moon.orbit`,
  `moon.impact`, `moon.capture`, `ring.system`, `belt.member`, `body.surface`, `body.resources`.
- Scope `Event`, each also an entry of plan 01's `event_tags!` in the block 0x0400–0x04FF that plan
  06 sets aside for this plan: `0x0400 BODY_IMPACT` (`body.impact`), `0x0401 BODY_ERUPTION`
  (`body.eruption`), `0x0402 BODY_STORM` (`body.storm`), `0x0403 BODY_DUSTSTORM` (`body.duststorm`),
  `0x0404 SYSTEM_COMET` (`system.comet`).

Every stream is `Stream::open(seed, tag, key)`. A `u16` body index fills plan 01's 16-bit `sub`
field of the second counter word exactly (`sub << 48 | block`), leaving 48 bits of block counter,
`Stream::WORDS` = 2⁴⁹ draw numbers per body.

Test helpers:
`planetary::testing::{synthetic_star, synthetic_binary, sample_contexts, solar_system_bodies}`
behind `cfg(any(test, feature = "testing"))`, as plan 06's `events::testing` is (the crate's
`testing` feature exists).

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
- `BodyIdHex`, a string in plan 01's form for a `BodyId` (`BodyId`'s `Display`): 16 lower-case hex
  digits, a full stop, 4 lower-case hex digits.
  `DetailLevelDto`, `BodyOrbitDto` (plan 11's `OrbitDto` plus `parent` and `valid_until`; by ruling
  33 of 2026-09-22 `OrbitDto` itself carries the whole element set and μ, `mu_m3_s2`, so this plan
  adds no element of its own), `BodySummaryDto`, `BeltDto`, `ZoneDto`, `HabitableZoneDto`,
  `BodyHooksDto`, `BodyEventDto`, `BodyStateDto`, `BodyKindDto`.
- `ErrorCode::UnknownBody` (`unknown_body`), beside plan 06's `ErrorCode::UnknownSystem` (P06.T33;
  neither exists at `9d8e775`). The client's exhaustive `switch` over error codes, `settledState` in
  `apps/hyperion/src/renderer/src/lib/useServerRequest.ts`, gains its case in the same task, so that
  `just ci` stays green after `just gen-protocol`.
- In `@hyperion/protocol`: `parseBodyId` and `formatBodyId`, in `packages/protocol/src/hex.ts`
  beside `hexToU64` and `u64ToHex`. Plan 04's generic `request` (`RequestClient::request`) needs no
  change, nor does `decodeServerMessage`, which trusts the generated types.

### Client (`apps/hyperion`)

All paths under `apps/hyperion/src/renderer/src/`.

- `"system"` in plan 05's `DisplayId` union (`lib/displays.ts`, reserved there for this plan), its
  entry in `DISPLAYS` (`{ id: "system", title: "System", key: "F3" }`), its case in `App.tsx`'s
  exhaustive `switch` over displays, and `displays/system/SystemDisplay.tsx`.
- `lib/orbit.ts`: `solveKepler`, `positionAt`, `orbitPolyline`, `composePosition`, pure functions.
- In plan 05's `spatial/`, all additive: in `marks.ts` the mark kinds `PathMark` and `AnnulusMark`,
  two optional members of `SpatialScene`, `paths` and `annuli`, and the `SymbolShape` values
  `pentagon` and `hexagon`, whose outlines go in `symbols.ts`'s `OUTLINES`; in `frame.ts`
  `planeFrame(normal, reference): LocalFrame`, which builds the tilted frame of D21 (plan 05's
  plane, grid, stalks, presets and fill rule all follow `scene.frame`, so `PlaneSpec` itself needs
  no change); on `SpatialView` an optional `axes` prop, three labelled unit vectors, which defaults
  to the scene frame's and which it passes both to `AxisTriad` and to the core arrow
  (`coreArrowLayout`, `CoreArrow`), since it draws both from `scene.frame` today (ruling 33); and a
  `ScaleUnit` ladder in km, Mm, Gm and AU for `SpatialView`'s `scaleUnits` (plan 05's `scale.ts`
  has only `SCENE_UNIT_ONLY` and the chart's private ly-to-AU ladder).
- In `lib/format.ts`: `formatPeriod`, `formatPressure`, `formatGravity`, `formatTemperatureK`,
  `formatBodyDistance`, `formatUniverseTimeDhms`. Planetary masses use plan 13's `formatMassMearth`
  and `EarthMassUnit`, and a brown dwarf's plan 13's `formatSubstellarMass`.

## Consumes

Names are those of the neighbouring plans' "Provides" as written. P14.T1.a reconciles them with the
code that exists when this plan's turn comes. The plan-text half of that was done at `9d8e775` for
the vertical slice (see Risks): where an item is built, its real path or signature is given here;
where it is not, the task that builds it is named.

- **Plan 01, determinism foundation.** `math`;
  `rng::{Seed, Stream, DomainTag, TagScope, ObjectKey, domain_tags!, EventKey}` with
  `Stream::open(Seed, DomainTag, ObjectKey)`, `impl From<BodyId> for ObjectKey` (counter word 1 is
  `sub << 48 | block`, with the `body_index` as `sub`), `seek` and `word_at` for draws addressed by
  slot, the samplers (`Stream` methods; `PowerLaw::new` and `PiecewiseLinear::new` return `Result`s)
  and integer-threshold decisions (`Thresholds::from_weights(weights, bound)`,
  `Mark::{pick, pick_weighted}`); `units`, including `EarthMasses`, `JupiterMasses`,
  `SolarLuminosities`, `SolarRadii`, `Kelvin`, `Dex` and `units::consts` (`METRES_PER_AU`,
  `SOLAR_RADIUS_M`, `SOLAR_LUMINOSITY_W`, the `GM_*` values); `time::{UniverseTime, ClockWindow}`
  and `CLOCK_WINDOW_H`, where `UniverseTime` is an `i64` of seconds and a `u32` of nanoseconds;
  `coords` (system and body frames, `coords/frames.rs`'s `SystemPosition` and `BodyPosition`, which
  are translations with galactic axes; rotating body-fixed frames are left to this plan);
  `id::{SystemId, BodyId, EventId, EventSubject, Designation}` (`BodyId::new(system, body_index)`,
  `body_index()`; a body's designation appends ` /<body index>`) and the `event_tags!` registry
  (every `u16` is a valid body index there, and its meaning is this plan's); `GENERATOR_VERSION`;
  `hyperion-testkit` (the `golden!` harness with `GoldenWriter`, `stats`,
  `order::assert_order_independent`), slow tests marked `#[ignore = "slow: …"]`, `just test-slow`,
  `just bench`.
- **Plan 02, galaxy model.** `galaxy::Galaxy` (`potential()`, `shares()`, `mass_function()`);
  `PotentialTables::tidal_radius(&self, m: SolarMasses, p: &PointLy) -> Metres`, at a record's
  `epoch_position()` converted with `PointLy::from`.
- **Plan 03, placement.** `placement::{SystemRecord, resolve}` (`resolve(galaxy, id)`; the record's
  `epoch_position()`, `primary_initial_mass()`, `age_at_epoch()`, `age_at(t)`, `existence_at(t)`),
  `ResolveSystemError` (`NoSuchSystem`, `LayerNotGenerated`, `KindNotGenerated`).
- **Plan 04, server and protocol.** The request envelope, `RequestBody`, `ResponseBody`,
  `REQUEST_KINDS`, `ErrorCode`, `SystemIdHex`, the wire `UniverseTime` (`{ seconds, nanos }`, the
  seconds a JSON number); in `hyperion-server`, `compute::{CpuPool, SingleFlight, GalaxyKey}`,
  `cache::{ByteLru, SharedByteLru, HeapBytes}`, and the dispatch in `requests/mod.rs` (`kind`,
  `is_large`, the `Handlers` match, and the `every_body` test walk), with `TestServer` and
  `TestClient` in `crates/hyperion-server/tests/common/mod.rs`; the TypeScript
  `RequestClient::request`.
- **Plan 05, `GALAXY` display**, under `apps/hyperion/src/renderer/src/`. `lib/displays.ts`
  (`DISPLAYS`, `DisplayId = "link" | "galaxy"`, which it reserves for this plan to extend) and the
  navigation bar (`components/ConsoleFrame.tsx`, `lib/useDisplayKeys.ts`, `App.tsx`);
  `lib/useServerRequest.ts` (`useServerRequest`, `RequestState`) and
  `components/RequestStatus.tsx`; `spatial/`: `SpatialView`, whose props are `scene`, `fitRadius`,
  `formatLength`, `scaleUnits?`, `frameName`, `centre` (`SpatialReading`s), `time` (a
  `SpatialReading`), `coreDistance` (a `SpatialQuantity`), `accessibleName`, `stale`, `onSelect`
  and `children?`, all but two of them required; `marks.ts` with `SpatialScene`, `PointMark`
  (`sizeClass` 0–4), `PlaneSpec` and `SymbolShape`; `frame.ts` with `LocalFrame` and
  `localFrameAt`; `camera.ts` with `PRESETS` (`top`, `side`, `front`, `oblique`); `symbols.ts` with
  `symbolOutline` and `SIZE_CLASS_REM`; `drawList.ts` (`buildDrawList`), `pick.ts`, `scale.ts`
  (`ScaleUnit { perSceneUnit, minSceneLength }`, `scaleBar`, `RADIUS_STEPS_LY`), `redraw.ts`
  (`createRedrawScheduler`), `AxisTriad.tsx` (props `frame`, `angles`, `boxRem`), `CoreArrow.tsx`,
  `Reading.tsx`, `ScaleBar.tsx`, `furniture.ts`; `lib/format.ts`; `components/UnitLabel.tsx`; the
  list-plus-canvas selection pattern, `formatUniverseTimeYr` and `TIME_SYSTEM_LABEL` (`"UT"`); the
  local chart's time, which is `timeYr: number` in `displays/galaxy/useLocalChart.ts`, not a
  `UniverseTime`, and which `@hyperion/protocol`'s `universeTimeFromYears` converts; the guide's 3D
  spatial display conventions (P05.T2.e) and its units `yr`, `Myr` and `Gyr`.
- **Plan 06, stars.** Built at `9d8e775`: `Composition` (`fe_h()`, `z_fit()`); `StarState` (phase,
  mass, luminosity, radius, effective temperature, by getters); `ObjectKind` (in `stellar::state`,
  re-exported from `stellar`); `StarDraws` with `disc_lifetime()`, the `UnitUniform` rank on
  `star.disc_lifetime` that T3.a's lifetime reads (ruling 33); `stellar::sse::{zams, ZCoeffs}`, the
  zero-age main-sequence luminosity and radius of D6,
  `zams::luminosity(m: SolarMasses, &ZCoeffs) -> SolarLuminosities` and
  `zams::radius(m, &ZCoeffs) -> SolarRadii` (P06.T4.c), with `ZCoeffs::new(composition.z_fit())`;
  `stellar::remnant::{CompactRemnant, RemnantKind}` (`CompactRemnant::new` is `pub(crate)`); the
  generic `events` module, `events::{EventSeries, TimeWindow}` with `PoissonBins`, `MonotonePhase`,
  `RateModel`, `PhaseClock` and `LinearClock`, in which every construction takes an `&EventSeries`,
  built by `EventSeries::new(seed, tag, subject)`, and `RateModel::bound` takes the bin's
  `TimeWindow` and returns `EventsPerSecond`; `events::testing::assert_partition_independent`;
  `math::normal_quantile`. Not built yet, by the task that builds it: `StarModel` (P06.T29.a), which
  by ruling 34 is how this plan reads every star, never `Track`: it exposes `state_at`, `lifetime`,
  `death`, `max_radius_until` and `max_luminosity_until` for every star, the cooling-fit stars below
  0.1 M☉ included (whose radius falls monotonically with age), over P06.T10.c–e's `Track` for the
  Hurley, Pols and Tout range and `substellar::cooling` below it (ruling 33); `SystemStars` and
  `draw_metallicity` of `stellar::system` (P06.T3, T29); the disc-lifetime law of P06.T15.c in
  `stellar/premain.rs` (built first, for this plan, in round 7); `remnant::{Death, DeathKind}`
  (P06.T10.e, T18) and `NatalKick` with `SystemStars::natal_kick` (P06.T19);
  `rotation::ActivityLevel` (P06.T25); `stellar::substellar::cooling`, which returns a `StarState`
  (P06.T13); `SystemSummaryDto` for the hosts on the wire and `ErrorCode::UnknownSystem` (P06.T33);
  the `lib/galaxy/starSymbols.ts` registry (P06.T35.b). Still needed and settled in T1.a: an [α/Fe],
  and an X-ray and ultraviolet history from `ActivityLevel`.
- **Plan 08, velocities and kicks.** Nothing beyond the `NatalKick` plan 06 exposes.
- **Plan 09, features** (not built at `9d8e775`; T1.d's interim rule stands in, see there). A
  system's sphere of influence (the smaller of the galactic tidal radius and the feature's, and the
  pericentre rule of P09.T28.c for the Kepler regime); for a feature member, the local number
  density, velocity dispersion and mean member mass from the feature's class profiles.
- **Plan 11, multiplicity** (none of it built at `9d8e775`; P11.T3.a and T1.a–c are built in round
  7). `orbit::{KeplerElements, Eccentricity, solve_kepler}` and `units::GravitationalParameter`
  (P11.T3.a, which takes this plan's T2.a requirements); in `stellar::multiplicity`:
  `SystemHierarchy`, `HierarchyNode`, `StarSlot`, `star_positions_at`, `STAR_BODY_INDEX_END`, and
  `StarSlot`'s kind, which tells a brown-dwarf companion from a star (P11.T2.a, T3.b, T2.d);
  `coords::{SystemVector, SystemVelocity}`, in `coords/frames.rs`; `BinaryState` for evolved pairs
  (P11.T4, after the slice); `OrbitDto`, which carries the whole element set and μ, and
  `HierarchyDto`, which carries the components' masses (P11.T13, ruling 33). Every component of the
  hierarchy is plan 11's body, brown-dwarf companions included: this plan generates no body in slot
  `0x00` of a system that has a star, and reads such a companion as one more component that bounds
  stable zones and may host one (D3, D10).
- **Plan 12, retarded observation.** Nothing at build time. Its note that degraded body records
  arrive with this plan is met by T34.
- **Plan 13, substellar layers.** The free-floating brown dwarf and rogue planet records,
  `SystemRecord::kind() -> SystemKind`, from which `HostKind` converts, and the body-0 convention
  (the object is body `0x0000`, its moons `0x0100` upward, its rings `0x0080`–`0x008F`); brown dwarf
  state through plan 06's cooling fit; `stellar::substellar::giant_cooling`, returning a
  `CoolingState`, for 0.3–13 M_Jup, which plan 13 builds and this plan only calls; by ruling 34 a
  host (a star or a brown dwarf) is always read as a `StarState` and only a giant planet's interior
  as a `CoolingState`, with no conversion between them; for a rogue planet, the record and
  metallicity with no derived state, which this plan supplies; `EarthGlyph`, `EarthMassUnit`,
  `formatMassMearth`, `formatSubstellarMass`, the `triangle-down` symbol, and the guide's entry for
  M⊕ (every planetary mass is in M⊕; there is no Jupiter-mass unit on the consoles). None of it is
  built at `9d8e775`; the slice takes P13.T5.b–c, T8.a (as an owner's draft), T8.b and the
  `triangle-down` outline first.

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
the floor on (a₂ − a₁) ÷ R_H is 10 for circular orbits rising to 12 with eccentricity (10 + 80 ×
mean e, capped at 12: Pu and Wu's 100 per unit of the Rayleigh scale σₑ, which is 0.8 of the mean,
ruling 38); for a pair with a giant it is 7 (Chambers et al. 1996; Marzari and
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
- _Engulfment._ A planet is destroyed at the first age at which a(t) < f × R★(t), with f = (1 +
  M_p ÷ 3.1 M⊕)^⅛, 1.04 for an Earth rising to 1.79 for a Jupiter, because tides drag in planets
  from beyond the photosphere (Mustill and Villaver 2012; ruling 62 fits f to their critical
  axes). That needs the largest stellar radius before a given age, a monotone helper asked of plan 06.
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

**D12. Young hosts.** A disc lives for a drawn time of a few million years. By ruling 33 of
2026-09-22 there is one lifetime per circumstellar disc, so that a star's T Tauri class (P06.T24)
and its planets' formation see the same disc: P06.T15.c's law (exponential, mean 2.5 Myr × (M ÷
M☉)^−½, held to 0.3–15 Myr; Mamajek 2009) applied to the star's own `star.disc_lifetime` rank. Only
a circumbinary disc draws a rank of its own, on `planet.disc`, and takes the same law at the pair's
total mass. Before it, the system holds a protoplanetary disc in a belt slot. Giants exist from a
formation age drawn below the disc lifetime, small planets from the disc lifetime, and terrestrial
planets carry a magma-ocean surface state until 10–100 Myr. Debris belts are bright when young and
fade as 1 ÷ age (Wyatt 2008).

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
`Full` (hooks: seed, composition, habitability, resources). `degrade` marks sections `NotResolved`
and never blurs a number (each section carries one of four states, T34); measurement noise is the
sensor plan's business. Until the Knowledge overlay exists the server grants whatever level a
request asks for, and says which level it granted.

**D17. No analytic habitability summary yet.** The civilisation brainstorm will want habitable
worlds per large cell from the fields. The class table and the habitable-zone formulae make that a
two-dimensional quadrature over stellar mass and metallicity, which can be tabulated offline later
without touching this plan's output.

**D18. The client propagates orbits for drawing.** The server sends elements, and the display
evaluates Kepler's equation itself to animate. That is drawing, not generation: every number in a
readout comes from the server, and the display re-requests the system when its time has moved more
than a year or past a body's `valid_until`. The TypeScript solver need not match `libm` bit for bit.

**D19. Units on the display follow plan 13.** Every planetary mass, from a moon to a 13 M_Jup giant
(`4131 M⊕`), is in M⊕ with plan 13's formatter and drawn glyph, because the guide wants one unit
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
(`planeFrame`, T40), and hands the true galactic directions separately, as `SpatialView`'s `axes`
prop. `SpatialView` draws both the triad and the core arrow from `scene.frame` itself, so the prop
is `SpatialView`'s and it passes it to both (ruling 33).

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
owner's confirmation, as plan 05's `UT` does. By ruling 33 of 2026-09-22 it is drafted for the owner
with the rest of T38.a's guide entries, and the client is built to the draft and marked for the
owner's confirmation.

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
- T3, T4 and T8 take the limits of a stable zone and of the strip radius as plain arguments. T9 and
  T29 supply real values when they land, and until then callers pass none.
- Phase D needs B, T15 and T16. Phase E needs C and D. Phase F needs B–E. Phase G needs F.
- Phase H's wire types (T35) can be drafted once T34 has fixed `BodyRecord`, in parallel with G.
- Phase I's T38, T39 and T40 need only plan 05 and can start at any time; T41–T44 need H.
- **The vertical slice** (README, "The vertical slice to the `SYSTEM` display", ruling 33 of
  2026-09-22) builds the first working `SYSTEM` display before plans 09, 11 and 13 are complete.
  Its tasks from this plan are T1.a–d, T2.a–c, T3–T9, T10.a, T11.a–d, T12, T15, T16.a–b, T28.a–c,
  T30.a–c, subsets of T32, T35 and T36, T34, T37–T41, T42.a–c, T43.a–b and T44.a. Each is built as
  written here. What a deferred plan or task would supply is a plain argument, the interim rule
  that the task names, or a documented `None`, and each task says so where it applies (_Slice:_).
  Nothing built for the slice is torn out later; a later task that fills a seam bumps the version
  where it moves output.

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
  - _Slice:_ the plan-text half was done at `9d8e775` (Consumes and Risks). The module docs of
    `planetary/mod.rs` record each consumed item's real path as the task that uses it lands. The
    [α/Fe] and the X-ray and ultraviolet history wait with their consumers (T13.b and phase E), and
    plan 09's accessors are stood in for by T1.d's interim rule.
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
  - _Slice:_ the builder and the synthetic hosts land first, `for_system` once P06.T29.b's
    `SystemStars` exists (with a single-star fallback until P11.T2.c has merged). Until plan 09:
    - the sphere of influence is the galactic tidal radius alone,
      `galaxy.potential().tidal_radius(system mass, &PointLy::from(record.epoch_position()))`, in
      metres. Deferring plan 09's pericentre rule (P09.T28.c) changes output only for systems within
      about 10 ly of the centre;
    - the encounter environment is `None`;
    - the strip radius of T29 is `SATELLITE_STABILITY_FRACTION` (0.49) × that tidal radius.

    The [α/Fe] is `None` until T1.a's closed form lands with its consumers.

#### P14.T2 Orbit additions

In plan 11's `crates/hyperion-sim/src/orbit/`, per D2. Nothing here may change a binary's state:
plan 11's goldens pass untouched.

- **P14.T2.a Bound orbits at planetary precision.** Check, and fix if needed, that
  `KeplerElements::relative_state_at` reduces the mean anomaly from `UniverseTime`'s integer seconds
  modulo the period before converting to `f64`, so that a one-day orbit keeps its phase a thousand
  years out, and that `solve_kepler` exits after a fixed number of iterations and not on a
  tolerance, so that every platform agrees. Add `from_semi_major_axis` and `scaled`. By ruling 33 of
  2026-09-22 this subtask is folded into P11.T3.a: the reduction, the fixed iteration count and
  `units::GravitationalParameter` are P11.T3.a's requirements from the start, and the two
  constructors and every test below are built with it (the `orbit` lane, round 7), so that there is
  nothing left to check here once it has merged.
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

`planetary/disc.rs`. Inputs, all plain arguments: host mass, [Fe/H], zero-age luminosity and
radius (D6; plan 06's `sse::zams::{luminosity, radius}(m, &ZCoeffs)`), the disc's lifetime (T3.a),
and optional inner and outer truncation radii in metres. All draws on `planet.disc`, keyed by
`SystemId` with the host number in the draw number. Registering `planet.disc` regenerates
`tags.golden` (Provides).

- **P14.T3.a Masses and lifetime.** Gas mass M_d = f × M★ with log₁₀ f normal about −2.0, σ = 0.5,
  capped at −1.0 (gravitational instability). Solids are Lodders' (2003, Table 11) condensate shares
  of the gas, each × 10^[Fe/H]: rock, 0.489% of the gas, inside the snow line, and rock plus water
  ice, 1.06%, beyond it, a step of 2.17 (ruling 38; the protosolar Z☉ = 0.0149 bounds both, where
  the literal "Z☉ × 10^[Fe/H], times 2" would have put twice the heavy elements into solids). The
  lifetime is an argument of `disc::derive`, not a draw here (ruling 33 of 2026-09-22, D12):
  P06.T15.c's law, exponential with a mean of 2.5 Myr at 1 M☉ scaled by m^−0.1 below it and m^−1.06
  above, held to 0.3–15 Myr (Mamajek 2009, AIP Conf. Proc. 1158, 3; Luhman et al. 2005; Ribas et al.
  2015; ruling 38), in `stellar/premain.rs`. For a circumstellar disc the caller applies it to the
  star's own rank, `StarDraws::disc_lifetime()` on `star.disc_lifetime`, so that the star's T Tauri
  class (P06.T24) and its planets see the same disc. Only for a circumbinary disc does the caller
  (T9.c) draw a rank on `planet.disc`, and apply the same law at the pair's total mass. For a host
  over 3 M☉ the lifetime's mass scaling is what starves planet formation; no separate switch.
  - _Tests:_ medians and widths of 10⁵ draws within 2% of the parameters; solid mass scales as
    10^[Fe/H] exactly; the lifetime is the argument. The law's own tests (its median, its clamps,
    monotonicity in the rank) go with the law in `stellar/premain.rs`.
- **P14.T3.b Geometry.** `snow_line(L)` = 2.7 au × √(L ÷ L☉). Inner edge: the larger of 2.5 zero-age
  stellar radii, the star's fluid Roche limit for a 1,000 kg/m³ body, and a magnetospheric
  truncation radius at a drawn corotation period, log-normal about 8 days, σ = 0.25 dex (the
  observed inner edge of Kepler systems near 10 days; Mulders et al. 2018). Characteristic radius
  r_c = 30 au × (M★ ÷ M☉)^0.5 with 0.3 dex of scatter; surface density Σ ∝ r⁻¹ exp(−r ÷ r_c), the
  self-similar profile, normalised to M_s from the inner edge to infinity, with the outer edge at
  3 r_c, inside which about 95% of the mass lies (ruling 38: r_c is not the edge, so a Kuiper-like
  belt has solids beyond it). Then truncate to the radii passed in, renormalising nothing: a
  truncated disc has lost that mass.
  The orbit zone (T9.c) and the strip radius of D14 (T29) are what callers pass.
  - _Tests:_ `snow_line` of 1 L☉ is 2.7 au; the integrated surface density returns M_s to 10⁻⁹ for
    an untruncated disc; inner edge < outer edge or the disc is `Disc::None`.
- **P14.T3.c `Disc` type** with getters `gas_mass`, `solid_mass`, `solid_mass_between(a, b)` (closed
  form of the Σ above), `lifetime`, `snow_line`, `inner_edge`, `outer_edge`, and `isolation_mass(a)`
  = the mass a body can sweep from its feeding zone of 2√3 Hill radii either side, solved in closed
  form from Σ (Lissauer 1987; ruling 38).
  - _Tests:_ `solid_mass_between` over the whole disc equals `solid_mass`, and is additive over
    adjoining intervals to 10⁻¹²; Hayashi's (1981) minimum-mass nebula (solids of 7.1 g cm⁻² ×
    (r ÷ 1 au)^−3/2 inside 2.7 au and 30 g cm⁻² × (r ÷ 1 au)^−3/2 beyond) gives an isolation mass
    of about 1 M⊕ at 5 au (Kennedy and Kenyon 2008, §2), and a disc enhanced to Σ = 10 g cm⁻² gives
    0.05–0.2 M⊕ at 1 au and 3–15 M⊕ at 5 au (Armitage 2007, eqs. 202–203: 0.07 and 9 M⊕).
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
  one `planet.class` draw through integer thresholds: plan 01's `Thresholds::from_weights` with the
  weights' own total as its `bound`, so that no mark is rejected, and `Mark::pick` (or
  `Mark::pick_weighted(weights, bound)`, which gives the same answer without allocating), both
  returning an `Option` that is `Some` whenever the bound is the total. D5's fallback and D10's
  binary suppression are applied here, from plain arguments: the disc, the zone's outer limit if
  any, and whether the host is in a binary closer than 50 au.
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
  outer planet of a pair is the larger in about 65% of pairs. The group's total is held to a solid
  budget drawn from the disc's whole solid mass, not the local annulus, by one factor for every
  member (ruling 38, point 4): close-in planets are made of solids carried inward, which the local
  isolation mass cannot give (2.3 × 10⁻⁴ M⊕ at 0.1 au in the median solar disc, where the compact
  classes place 1–20 M⊕), as Chiang and Laughlin's (2013) minimum-mass extrasolar nebula shows,
  and Mulders et al. (2021) find the solid masses of observed systems matching those of discs only
  at an efficiency near 100%. The local isolation mass stays for what it describes, a giant's core
  beyond the snow line (T7.c). Draw numbers are the planet's slot, so inserting a group never
  shifts another.
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
`place(host, limits, disc, class) -> Vec<PlacedPlanet>` interprets the template inside the zone.
Its inputs are plain arguments, as the order note allows: the host's parameters (mass, zero-age
luminosity and radius, [Fe/H], and whether it is in a binary closer than 50 au) and the zone's
inner and outer limits in metres, not a `SystemContext` or an `OrbitZone`, which do not exist when
it lands. T30.a adapts T9's zones and T1.d's context to them.

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
  star, with the star's own disc lifetime (P06.T15.c's law on `StarDraws::disc_lifetime()`), a
  circumbinary one from the pair's total mass and summed luminosity, with a lifetime from a rank
  drawn on `planet.disc` under the same law at the total mass (ruling 33, D12). Apply D10's
  suppression for binaries inside 50 au (Kraus et al. 2016; Moe and Kratter 2021) by moving class
  weight to `Barren`.
  - _Tests:_ (a) for μ = 0.5, e = 0: S-type limit 0.274 and P-type 2.39 binary separations; α
    Centauri AB (23.5 au, e = 0.52) gives about 2.8 au around A. (b) Zones of a hierarchical triple
    never overlap; a single star has exactly one zone; a brown-dwarf companion of plan 11 bounds
    zones like any other component. (c) No planet of 10⁴ sampled binaries lies outside its zone, and
    hosts in binaries inside 50 au have planets a quarter to a half as often as single stars of the
    same mass.
  - _Accept:_ `cargo test -p hyperion-sim planetary::placement::zones`.
  - _Slice:_ plan 11's engine (P11.T4–T11) is deferred, so every pair is two single stars on an
    orbit and D10's zones are the zones at birth only; the intersection with the evaluated state
    comes with `BinaryState`. No brown-dwarf companion is drawn until P11.T2.d, so (b)'s test of one
    runs on a hand-built hierarchy with a `StarSlot` of the brown-dwarf kind.

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
  (it covers 0.3–13 M_J, so every giant this plan places), taken as its `CoolingState`, the one
  cooling type this subtask accepts (ruling 34), blended into Chen and Kipping's over
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
  orbit-averaged form, with the host's state at age + t, taken as plain values of L, T_eff and R,
  which the caller reads from each host's `StarState` (ruling 34: a host is always a `StarState`,
  and this subtask accepts no `CoolingState`). In a multiple system fluxes add: a
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
  - _Slice:_ `derive_body` runs T11, T12 and T15 only, in the order it will keep; T13 and T14 are
    deferred, so the Bond albedo is fixed at 0.3, the value T12.a names before T13 closes the loop,
    and the surface section is tagged `NotModelled` (T34).
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
  age. _Slice:_ built first without the `ProtoplanetaryDisc` body, whose belt slot `0xE0` no other
  slice task fills; it is added under this subtask, with a bump, alongside phase D's belts (T21),
  and the second half of test (a) waits with it. `BodyKindDto` has the variant from the start (T35).
- **P14.T28.b Expansion and engulfment.** Adiabatic expansion and the engulfment test of D11, using
  the host's mass at age + t and its largest radius before that age, with the reach f = (1 + M_p ÷
  3.1 M⊕)^⅛ fitted to Mustill and Villaver (2012) (ruling 62). The clearance a(t) − f R_max(t) is
  not monotone, since after the red-giant tip the winds widen the orbit while R_max holds, so the
  destruction time is its first crossing, found by a scan of fixed points and then a bisection
  with a fixed number of steps (ruling 62). Moons go with their planet. Circularisation (T8.e) is
  applied first.
- **P14.T28.c Supernovae.** At the host's death time: the planet's state vector from its elements,
  the host's velocity change from the death record's kick, the remnant's mass, then new elements
  from `elements_from_state` (T2.c) or `Unbound`. A body whose new pericentre is inside the
  remnant's Roche limit is destroyed. For a star in a binary the companion's planets see the same
  mass loss through plan 11's post-explosion orbit; circumbinary planets are treated as orbiting the
  pair's total mass. _Slice:_ until P06.T19 there is no kick law, so `SystemStars::natal_kick()` is
  `None` and T28.c applies a zero kick; the kick changes output later, with P06.T19's bump. Until
  P11.T4 the companion's planets see the mass loss as a single star's.
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
  _Slice:_ satellites, belts, the halo, second-generation planets and T27's hosts are deferred, so
  `generate` equals `generate_planets` until T22.a. Each deferred stage has its own streams (D4)
  and slots (D3), so adding it moves no planet.
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
comets) as the subject of an `events::EventSeries::new(seed, tag, subject)`, which calls plan 01's
`EventKey::derive`; every construction takes the `&EventSeries`, a series goes to one construction
only, and a `RateModel`'s `bound` takes the bin's `TimeWindow` (P06.T27 as built). T31.a and T31.b
register the five event tags of Provides, each in `rng/tags.rs` (scope `Event`) and in
`id/event_tags.rs` with its number, as it is first used.

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
    has one; (c) the same events come back for a window asked whole and in halves (plan 06's
    `events::testing::assert_partition_independent`) and in any order of asking (plan 01's
    `hyperion_testkit::order::assert_order_independent`); `EventId`s round-trip through plan 01's
    text form; no event changes any `body_at` result.
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
- **P14.T32.b The goldens.** `tests/golden/planetary/`, through plan 01's `golden!`, each written
  with `GoldenWriter` so that a bump can re-bless it. Each golden holds the full `snapshot_at` at
  the epoch and at +H and the events of one century. Bump `GENERATOR_VERSION` here.
  - _Accept:_ `cargo test -p hyperion-sim --test planetary_golden`; changing any constant in
    `planetary` fails it.
- _Slice:_ T32.a and T32.b pin the fifteen of the twenty-four whose hosts and bodies exist without
  plans 09 and 13, phase D and the rest of phase F: the M dwarf, the hot Jupiter, the Solar-like
  system, the eccentric giant, the halo star, the close and the wide binary and the triple (their
  pairs as two single stars on an orbit until P11.T4), the T Tauri star (without its disc body until
  T28.a has it), the subgiant, the red giant, the fallback black hole (with a zero kick) and the
  three fillers, and hold no events until T31. The other nine (the white dwarf with a polluting
  belt, the neutron star with second-generation planets, the free-floating brown dwarf, the star
  with a brown-dwarf companion, the two rogue planets, the nuclear-cluster and globular members and
  the layer-E death inside the clock window) are added by the tasks that make them possible, each
  with its bump.

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
`surface`, `hooks`, each a `Section<T>` except identity (below); `DetailLevel` ordered;
`degrade(level)`; `SystemSnapshot::degrade(level)` applies it to every body and drops belts' member
lists below `Bulk`. `Contact`, the brainstorm's "unresolved contact", keeps the ID and the position:
the kind is replaced by `BodyKind::Unresolved` and the label is withheld. `MassAndOrbit` is its
"mass and orbit only".

Every optional section of a body or system record is tagged with its state (ruling 34 of
2026-09-22, which settles ruling 33's `NOT YET MODELLED` against `NOT RESOLVED`). The type is
`Section<T> { Ok(T), NotResolved, NotModelled, NotApplicable }`:

- `Ok(T)`: the section, with its value. "None" is data, not a state: an airless world's atmosphere
  is `Ok` with no gas in it.
- `NotResolved`: the granted detail level withholds it. `degrade` is the only thing that produces
  it.
- `NotModelled`: this generator version does not compute it, and it must never be taken for
  "none". In the slice that is every body's `surface` and `hooks` (T13, T14 and T23–T26 fill them),
  each planet's `moons` and `rings`, and the system's `belts` and `halo` (phase D).
- `NotApplicable`: the section has no meaning for the body's kind, such as a gas giant's surface.

The server sets every tag, because it is the authority on what its generator version computes; the
client never infers one. A single value that the generator does not compute inside a section it
otherwise models is not a section state: the section is `Ok` and the value is absent, which the
display shows as the guide's "Missing" state, the em dash (the host star's variability, rotation
and spins in the first viewer). `SystemSnapshot` carries `belts` and `halo` as sections, and each
planet's record `moons` and `rings`, so that T41.b composes its system note from the tags.

- _Tests:_ `degrade` is idempotent and monotone (`degrade(a).degrade(b)` = `degrade(min(a, b))`); a
  `MassAndOrbit` record serialised to JSON contains no radius, temperature or composition key;
  `degrade` turns `Ok`, `NotModelled` and `NotApplicable` sections above the level into
  `NotResolved` and leaves those at or below it untouched; in the slice, a planet's `surface` and
  `hooks` are `NotModelled` at a granted level of `Full`, a gas giant's `surface` is
  `NotApplicable`, and the system's `belts` and `halo` are `NotModelled`.
- _Accept:_ `cargo test -p hyperion-sim planetary::record`.

#### P14.T35 Wire types

`crates/hyperion-protocol/src/` in plan 04's module layout, following its "Extending the
convention": a variant of `RequestBody` and of `ResponseBody` per kind, the kind's string in
`REQUEST_KINDS`, a wire-form test each, then `just gen-protocol`.

- **P14.T35.a IDs and orbits.** `BodyIdHex` beside `SystemIdHex`, in plan 01's string form, with
  `ParseBodyIdHexError`;
  `BodyOrbitDto { parent: BodyIdHex, orbit: OrbitDto, valid_until }`, with plan 11's `OrbitDto`,
  which by ruling 33 of 2026-09-22 carries the whole element set (Ω, ω and the mean anomaly at the
  epoch beside the period, a, e and i, in radians) and μ in m³ s⁻² (`mu_m3_s2`), so that D18's
  client-side propagation needs nothing else; angles in the system frame for planets and in the
  parent's frame for moons, as the sim has them; `DetailLevelDto` as snake-case strings;
  `SectionDto<T>`, T34's `Section<T>` on the wire, tagged by a `state` of `ok` (with the value),
  `not_resolved`, `not_modelled` or `not_applicable` (ruling 34), which every optional section of
  every record below uses. Its wire form is pinned with one test per state.
- **P14.T35.b Records.** `BodySummaryDto` (identity, label, kind, state, and the `orbit`, `bulk`,
  `moons` and `rings` sections), `BodyDetailDto` (every section, hooks included, each a
  `SectionDto`; the surface seed as 16 hex digits), `BeltDto`, `ZoneDto` (stable zones, snow line,
  system plane), `HabitableZoneDto`, `BodyEventDto` (a comet event carries its elements and a track
  of positions sampled by the server, since the client does not propagate open orbits). Every
  quantity's field name carries its SI unit. _Slice:_ `BodyKindDto` and `BodyStateDto` get every
  variant from the start, so that no later task changes their shape; `BeltDto` and `BodyEventDto`
  wait for T21 and T31; the surface and hooks sections, each planet's moons and rings, and the
  system's belts and halo are tagged `not_modelled` (T34).
- **P14.T35.c Requests and responses.** The three pairs under Provides. `SystemBodiesDto` carries
  `granted: DetailLevelDto`, the hosts as plan 06's `SystemSummaryDto` with plan 11's
  `HierarchyDto`, the zones, the `belts` and `halo` sections, and a flat body list in index order
  (the tree is rebuilt from `parent`). `ErrorCode::UnknownBody`, whose case the client's exhaustive
  `settledState` switch in `apps/hyperion/src/renderer/src/lib/useServerRequest.ts` gains in this
  subtask, so that `just ci` stays green after `just gen-protocol`, as P06.T33 does for
  `unknown_system`. The server's exhaustive matches over `RequestBody` and `ResponseBody`
  (`requests::{kind, is_large}`, the `Handlers` match and the `every_body` test walk in
  `crates/hyperion-server/src/requests/mod.rs`) gain the new kinds, which answer `unsupported` until
  T36. `body_events` is reserved by plan 04 like the other two: its string goes into `REQUEST_KINDS`
  with the other two, and plan 04's `request_kinds_lists_every_variant` test then covers it.
  _Slice:_ `body_events` waits for T31, so the slice adds `system_bodies` and `body_detail` only,
  and `body_events` follows by the same procedure.
  - _Tests:_ one wire-form test per request, response and record type, as `rust-dev.md` requires,
    each in the subtask that adds the type; (a) `BodyIdHex` round trip and rejection of malformed
    strings; (c) `REQUEST_KINDS` holds the three new strings (two in the slice).
  - _Accept:_ `cargo test -p hyperion-protocol`; `just gen-protocol-check` passes.

#### P14.T36 Server handlers

`crates/hyperion-server/src/`, beside plan 06's `system_summary` handler.

- **P14.T36.a System cache.** A `SharedByteLru` keyed by `(GalaxyKey, SystemId)` holding
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
- _Slice:_ `system_bodies` and `body_detail` only; `body_events` and its window test come with
  T31. The hosts' `SystemSummaryDto` is P06.T33–T34's, whose `system_summary` handler and
  `SystemStars` cache the slice builds first.

#### P14.T37 Client protocol helpers

`packages/protocol/src/hex.ts`: `parseBodyId` and `formatBodyId`, beside `hexToU64` and
`u64ToHex`, exported from `index.ts`. Plan 04's decoder needs no list of kinds:
`decodeServerMessage` trusts the generated types. The generic `request` is unchanged.

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
  orbits are solid `--text-muted` ellipses (7.22:1 on `--surface-0`), reference marks like range
  rings and not predictions, so never dashed, and the selected body's orbit is a solid `--text`
  hairline because it carries the selection; zones and belts are labelled annuli drawn as their two
  edges in `--text-muted`, a belt's edges joined by short radial ticks every 10°, with no fill,
  hatch or dots, since hazard striping is the guide's only pattern fill; `--line` (1.38:1) stays for
  the grid and the scale rings on the reference plane only, because orbits and zone and belt edges
  say where something lies, so the guide's 6:1 rule for the parts of a symbol that carry meaning
  binds them as it binds the range sphere's outline (ruling 35.6 of 2026-09-22); the mandatory
  `BODIES NOT TO SCALE` label; the reference plane of D21 and its label; the body symbols of T42,
  added to the one ship-wide symbol set; and D24's rule that bodies move only when the display time
  does.
  - _Accept:_
    `grep -n "BODIES NOT TO SCALE\|SYSTEM PLANE\|DISPLAY TIME" docs/frontend/ux-guidelines.md` finds
    each edit; Prettier passes on the guide.
  - _Slice:_ by ruling 33 of 2026-09-22 the guide is the owner's to edit, so these entries,
    P13.T8.a's M⊕, the phrases `NOT YET MODELLED` and `NOT RESOLVED` and the parts of P06.T35.a the
    display shows are drafted for the owner (the orchestration notes' `ux-draft-system-display.md`).
    The client is built to the draft and marked for the owner's confirmation, as ruling 15's
    `DRIVE RANGE` row was, and the acceptance above holds once the owner has made the edits.
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

- _Tests:_ Vitest against the fixture of 32 orbits and states that P11.T3.a writes with T2.a
  (a golden under `crates/hyperion-sim/tests/golden/orbit/`, written through `GoldenWriter`, its
  line format in its header), agreeing to 10⁻⁹ relative; the polyline closes; a moon's composed
  position equals planet plus offset. The fixture is read from the golden, so a re-blessed golden
  moves the client's test with it.
- _Accept:_ `pnpm --filter hyperion exec vitest run src/renderer/src/lib/orbit.test.ts`.

#### P14.T40 Spatial-view marks for paths and annuli

In plan 05's `spatial/`, all additive, so that the `GALAXY` display's draw lists are unchanged:

- **P14.T40.a Marks.** Two mark kinds in `marks.ts`: `PathMark` (a polyline in 3D, a `role` of
  `"reference"` or `"selected"` that picks `--text-muted` or `--text`, T38.a's rule, and a label
  anchor) and
  `AnnulusMark` (inner and outer radius on the reference plane, a `ticks` flag for belts, and a
  label; T38.a's drawing rule). `SpatialScene` gains optional `paths` and `annuli`, absent meaning
  none. `buildDrawList` handles both with the existing projection and D12's far-to-near order (a
  path is split where it crosses the plane, an annulus is drawn with the plane), and `pick` ignores
  them. `marks.ts`'s `SymbolShape` gains `pentagon` and `hexagon` for T42, `symbols.ts`'s
  `OUTLINES` their closed outlines, and every exhaustive `switch` over `SymbolShape` its case. The
  `ui` lane builds them in round 7 with the `ringed-circle` outline of P06.T35.b and the
  `triangle-down` outline of P13.T8.c, so the four arrive together.
  - _Tests:_ Vitest on the projection of a circular path tilted 60° (an ellipse with axes 1 and
    0.5), on draw order either side of the reference plane, on an annulus's two edges and ticks, and
    on the new outlines (closed, centred, inside the unit circle, open and filled differing only by
    fill); plan 05's tests pass untouched.
- **P14.T40.b The tilted frame.** `frame.ts` gains `planeFrame(normal, reference): LocalFrame`:
  `north` is the unit normal, `coreward` the reference direction projected onto the plane and
  normalised (falling back to any perpendicular when the two are within 10⁻⁶ of parallel),
  `spinward` their cross product, right-handed as `localFrameAt`'s is. `SpatialView` gains the
  optional `axes` prop of Provides (ruling 33 of 2026-09-22): it draws the axis triad
  (`triadLayout`, `AxisTriad`, whose own props stay `frame`, `angles` and `boxRem`) and the core
  arrow (`coreArrowLayout`, `CoreArrow`) from `scene.frame` today (`SpatialView.tsx`, about lines
  296, 302 and 498), so it passes `axes`, or `scene.frame` when it is absent, to both. `PlaneSpec`
  is unchanged (D21).
  - _Tests:_ the frame is orthonormal for 1,000 random normals; with the galactic north as normal it
    equals `localFrameAt`'s; a scene built on a frame tilted 30° draws its grid as plan 05 draws the
    galactic one in that frame's own coordinates; with `axes` given the triad's `NORTH` follows the
    galactic vector and not the plane's normal, and the core arrow follows the given coreward; with
    `axes` absent the `GALAXY` display's triad and arrow are unchanged.
  - _Accept (both):_ `pnpm test`.

#### P14.T41 The display shell

`apps/hyperion/src/renderer/src/displays/system/`, beside plan 05's `displays/galaxy/`.

- **P14.T41.a Navigation and requests.** Add `"system"` to `DisplayId`, its entry to `DISPLAYS`
  (`SYSTEM`, `F3`), which the navigation bar and `useDisplayKeys` read, and its case to `App.tsx`'s
  exhaustive `switch` over displays. Beside the `GALAXY` display's selected-system readout, in
  `SystemsPanel.tsx` and not inside the readout's `output` or any `role="status"` region (rulings 13
  and 14 keep controls out of live regions), an `OPEN SYSTEM` button switches display with the
  system's ID and the chart's time. The chart's time is `timeYr: number` in `useLocalChart`, so it
  crosses as `universeTimeFromYears(timeYr)`, a `UniverseTime`. A hook
  `useSystemBodies(systemId, time: UniverseTime, detail)` over plan 05's `useServerRequest` owns the
  request, cancels on change, and re-requests per D18 (when the display time has moved more than a
  year or past a body's `valid_until`).
- **P14.T41.b Data states.** No system selected (`NO SYSTEM SELECTED`), and every non-`ok`
  `RequestState` through plan 05's shared `RequestStatus` (pending, rejected with the typed reason,
  timed out, link down), which keeps the guide's ban on spinners and "Loading…"; "no such system";
  `NOT YET FORMED` for an unborn host (ruling 34: one phrase for anything not yet born, system,
  star or planet); and an empty system (`NO BODIES`), each as text, never as an
  empty canvas. On loss of the link the last data stay, marked stale as the guide requires. The
  granted detail level is always shown (`DETAIL: MASS AND ORBIT ONLY`). What this generator version
  does not model is said, never left to look empty (ruling 33 of 2026-09-22): the display carries a
  system note, `MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED` in the slice, so that the
  empty space beyond the planets is not read as empty. The note is composed from T34's section tags
  (ruling 34), never from a list in the client: belts and halo from the system's sections, moons
  and rings from each planet's, each named when its tag is `not_modelled`, so each part drops out
  as the server starts to model it.
  - _Tests:_ Testing Library with `FakeWebSocket`: (a) opening from the galaxy display issues one
    `system_bodies` request with the right ID and time, and `F`-key navigation reaches the display;
    (b) each state renders its text, including `NOT YET FORMED` for an unborn host, and the system
    note names exactly the kinds whose tags are `not_modelled` in a fixture (all four in the
    slice's, none when a fixture tags them `ok`).
  - _Accept:_ `pnpm test`.

#### P14.T42 The orbit map

`displays/system/OrbitMap.tsx` on the spatial view.

- **P14.T42.a Marks.** Hosts and bodies as point marks at `composePosition`, orbits as `path` marks,
  stable-zone limits, the snow line and the habitable zone as `annulus` marks that can be switched
  off, belts as ticked annuli (T38.a), the cometary halo as a labelled outer ring only when it is
  inside the view. Orbits and every annulus edge are solid `--text-muted`, and the selected body's
  orbit, the one `"selected"` path, a solid `--text` hairline; `--line` is for the plane's grid and
  scale rings only (ruling 35.6, T38.a). Symbols, where shape
  encodes type and one shape means one thing on every display: hosts keep their symbols from the
  registry of plans 06 and 13 (`lib/galaxy/starSymbols.ts`, P06.T35.b: circle, ringed circle,
  diamond for a white dwarf, triangle for a neutron star, square for a black hole). A planet, bound
  or free-floating, is plan 13's `triangle-down`, with plan 05's size class telling giant (3) from
  smaller planet (1) from dwarf planet (0), under a `SYMBOLS NOT TO SCALE` legend; a moon is a
  `pentagon`; an unresolved contact is a `hexagon`. Both new outlines are closed, because plan 05's
  filled-above, open-below rule needs a shape that can be filled, which rules out a plain cross. The
  list names every kind in words, so shape is never the only signal. Destroyed and unbound bodies
  are not drawn but stay in the list. Colour stays free: only the selection reticle and, later,
  status use it. _Slice:_ no belts, halo, moons or unresolved contacts exist yet, so their marks are
  built and tested on fixtures but draw nothing from a real system.
- **P14.T42.b Frames and scale.** Two frames: `SYSTEM BARYCENTRIC`, drawn on the `SYSTEM PLANE` of
  D21, and, when a planet is focused with `FOCUS BODY`, `BODY <designation>`, in which its moons and
  rings are drawn and distances switch to Mm and km. Distances are true to scale with a 1-2-5 scale
  bar; symbols are not, and the display says `BODIES NOT TO SCALE`. The bar's ladder is a new
  `ScaleUnit` list in km, Mm, Gm and AU (`ScaleUnit { perSceneUnit, minSceneLength }`), passed as
  `SpatialView`'s `scaleUnits`. Zoom presets `INNER` (fits the outer habitable-zone limit or the
  fifth body), `ALL` (fits the outermost planet) and `BELTS` (fits the outermost belt), which set
  `fitRadius`, plus plan 05's camera `PRESETS` (`top`, `side`, `front`, `oblique`, shown as `TOP`,
  `SIDE`, `FRONT` and `OBLIQUE`) relative to the system plane. The frame name, time, azimuth,
  elevation and triad are always shown: `SpatialView` requires `frameName`, `centre`, `time`,
  `coreDistance`, `accessibleName` and `stale`, and takes the galactic directions for the triad and
  the core arrow as its `axes` (T40.b). By ruling 34 the `centre` readings are the system's galactic
  position, which is where its barycentre is (`RADIUS`, `ANGLE`, `HEIGHT`, as the chart reads its
  centre), named in words as the barycentre, and `coreDistance` is its galactocentric distance, so
  that the core arrow shows the true direction to the galactic centre in the galactic axes the frame
  keeps. _Slice:_ `BELTS` and `FOCUS BODY` wait for phase D, since there are no belts and no moons.
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
  `UNBOUND`). _Slice:_ hosts and planets only, the hosts first, then the planets by semi-major
  axis.
- **P14.T43.b Readout.** An `output` element, filled by a `body_detail` request: designation and ID;
  label; kind and class; state; mass and radius; density and surface gravity; semi-major axis,
  period, eccentricity, inclination; distance from its primary now; equilibrium and surface
  temperature; pressure and atmosphere; rotation period and locking state; composition; the global
  figures; the habitability class with its reasons in words; resource abundances as a small table;
  for the system as a whole, the architecture class and the zones. Each section renders from its T34
  tag (ruling 34), never from a guess: `ok` shows the section; `not_resolved` reads `NOT RESOLVED`;
  `not_modelled` reads `NOT YET MODELLED`, once per section, and is never read as "none";
  `not_applicable` omits the section's rows, so a gas giant shows no surface section, not an em
  dash. A single value missing inside an `ok` section is the guide's "Missing" state, the em dash in
  `--text-muted`, and "none" is data (an airless world's atmosphere is `ok` with no gas). Nothing is
  ever a blank or a zero. _Slice:_ the surface, atmosphere, rotation, habitability and resources
  sections arrive tagged `not_modelled` and read `NOT YET MODELLED`; the hosts' rows show the
  stellar summary of P06.T33 (kind, phase, MK class without peculiar suffixes, initial mass and mass
  now, L, R, T_eff, and for a remnant its kind, mass and cooling age), with the guide's em dash for
  what plan 06 does not model yet (variability, rotation, activity, spins, kicks, binary class).
- **P14.T43.c Events.** A list of the body events of the century around the display time, from a
  `body_events` request, each with its time in the chart's time system and a countdown in the
  guide's `T-` form.
  - _Tests:_ Testing Library: (a) arrow keys and Enter select, the list shows position and total,
    and a destroyed body's row says `DESTROYED`; (b) keyboard selection updates the readout, a
    `mass_and_orbit` fixture shows `NOT RESOLVED` for the surface section, a surface tagged
    `not_modelled` shows `NOT YET MODELLED` once, a gas giant's `not_applicable` surface shows no
    row, and units appear on every value; (c) an event
    before the display time counts `T+` and one after it `T-`, each with its time-system label.
  - _Accept:_ `pnpm test`.
  - _Slice:_ T43.c waits for T31 and `body_events`; there is no events panel in the first display.

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
- _Slice:_ T44.a only. The first display opens held and steps; `RUN` and `HOLD` (T44.b) follow.

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
- **Deviations in T40 and T38.b, as built (round 7, `ui`).** _T40.a._ `PathMark` is `{ id, points, role: PathRole, label, labelAt }` and `AnnulusMark` `{ id, centre?, innerRadius, outerRadius, ticks, label }`: each has an `id` for its label's key, and an annulus an optional `centre`, the view centre when absent, taken at its foot on the plane, since a wide binary's zones are about one star and not the barycentre. A path is cut where it meets the plane (heights within 10⁻⁹ of the path's size count as on it, and so above, so that an orbit drawn in a tilted plane is not cut at every rounding error), a closed path's first and last runs on one side are joined, and each piece is a 1 px polyline, `--line` or `--text`, that opens its own half before the half's marks, so that no line crosses a symbol, with the selected path after the reference ones; an annulus is drawn with the plane after its rings, each edge a `--line` polyline (equal radii draw one, a radius of 0 none) and a belt's ticks one `ticks` op of radial segments from the inner edge to the outer every 10° from coreward, the reading taken of "edges joined by short radial ticks". Labels follow the rings' in the curve labels, placed as a ring's: an annulus's at its outer edge's rimward point, away from the rings' at their coreward points, a path's at `labelAt`. **For the orchestrator to rule:** those orders and places, and T38.a's `--line` for zones, belts and orbits: it is 1.38:1 on `--surface-0`, and a zone's or belt's edge says where something lies, as a range sphere's outline does, for which the guide asks 6:1; `--text-muted` (7.22:1) is the token that passes, and the selected orbit's `--text` already does. New outlines: `pentagon` point up and `hexagon` flat at top and bottom, so that they read apart; at size class 0 their holes are 3.8 and 4.1 px at 100%, but their flat sides lie only 0.76 and 0.54 px inside a circle of the same size, so whether a moon or a contact reads apart from a host's circle at class 0 is a by-eye point for T42, which may give them a smallest size class. _T40.b_, with ruling 33: `planeFrame(normal, reference)` falls back, for a reference within `PARALLEL_TOLERANCE` (10⁻⁶) of the normal or of no length, to the direction along the galactic axis least along the normal, laid onto the plane, and is never `onAxis`; `SpatialView`'s `axes?: LocalFrame` feeds `AxisTriad`'s new `axes` prop and the core arrow through a last optional `shown` parameter of `triadLayout` and `coreArrowLayout`, and the `DIRECTIONS UNDEFINED` note follows it, since it is about the galaxy's directions; the camera, plane, grid, stalks, fill rule and presets follow `scene.frame` as D21 says. _T38.b_: `formatPeriod(periodDays)`, `formatBodyDistance(distanceKm, previous)`, `formatPressure(pressurePa)` return `{ value, unit }`; `formatTemperatureK` and `formatGravity` the value alone. Periods, distances, pressures and gravities take three significant figures, with E notation outside each ladder (below 0.01 d, from 10⁴ yr; below 0.01 km, from 10⁶ AU; below 0.01 Pa, from 10⁴ MPa; below 0.01 m/s²). Distances go km, Mm, Gm, then AU from 0.1 AU (15.0 Gm), so that every planet but the closest-in reads in AU, keeping the last unit within 5% (`DISTANCE_HYSTERESIS`) of either edge of its band; the caller holds the last unit. `formatUniverseTimeDhms(time)` takes a `UniverseTime`'s floor seconds and splits them with integer arithmetic; the sign is the whole time's, as a countdown's, so one second before the epoch reads `-0 yr 000/00:00:01`, and the days are zero-padded to three digits so that the field keeps its width, where the guide's `MET 57/14:08:33` is not. It is built to D24 and says so in its TSDoc; those two choices go to the owner with D24. Masses use plan 13's `formatMassMearth`, recorded there.
- **Re-validated at `9d8e775` for the vertical slice** (round 7, the `doc` lane). Plans 01–05 and 07
  are built, plan 06 in part (T1, T2, T4–T9, T10.a–b, T11, T27), plans 08, 09, 11, 13 and 15 not at
  all; P11.T3.a, P11.T1.a–c, P14.T1.b–c and T3, and P14.T40 are being built in round 7. The plan
  text now follows the code in these places:
  - `units::GravitationalParameter` is P11.T3.a's, which takes T2.a's requirements (ruling 33).
  - `coords` is split, so the two vectors are in `coords/frames.rs`.
  - `snow_line` takes `SolarLuminosities`, and `class_weights` takes a `Dex`; there is no
    `Luminosity` or `FeH`.
  - `zams::{luminosity, radius}` take `(SolarMasses, &ZCoeffs)`; `ObjectKind` lives in
    `stellar::state`; `CompactRemnant::new` is `pub(crate)`.
  - `Death`, `DeathKind`, `NatalKick`, `ActivityLevel`, `Track`, `StarModel`, `SystemStars` and
    `substellar::cooling` are not built yet, and Consumes names the task for each.
  - The generic `events` module takes an `EventSeries` (`EventSeries::new(seed, tag, subject)`), its
    `RateModel::bound` a `TimeWindow`, and T31's order check is plan 01's
    `hyperion_testkit::order::assert_order_independent`, beside plan 06's
    `events::testing::assert_partition_independent`.
  - `Thresholds::from_weights` takes a bound, and `Mark::pick_weighted` exists (T4.c).
  - `SymbolShape` is in `marks.ts` and has four values, `starSymbols.ts` does not exist, and
    `DisplayId` is `"link" | "galaxy"`.
  - `SpatialView` draws the triad and the core arrow from `scene.frame`, so `axes` is its prop
    (ruling 33). It also requires `centre`, `time`, `coreDistance`, `accessibleName` and `stale`,
    and its scale ladder is a `ScaleUnit` list, which needs a km-to-AU one.
  - The chart's time is `timeYr: number`, which crosses to this display as a `UniverseTime` through
    `universeTimeFromYears`.
  - New error codes need the client's `settledState` case in the same task, and new kinds the
    server's dispatch matches.
  - The decoder needs no kind list, and the system cache key is `(GalaxyKey, SystemId)`.
  - Stream counters leave 2⁴⁹ words per body.
  - New tags go at the end of `domain_tags!`.
  - T3's lifetime is an argument, from P06.T15.c's law on the star's rank (ruling 33).
  - `OrbitDto` carries the whole element set and μ, so `BodyOrbitDto` adds only `parent` and
    `valid_until` (ruling 33).

  The risk above that `body_events` is not among plan 04's reserved kinds is settled: plan 04's
  "Extending the convention" table reserves it with the other two.

  Pending re-validation, because what they read is not built:
  - T1.d's `for_system` (P06.T29.b, P11.T2.c);
  - T11.d (P13.T5.c);
  - T12 and T27 for substellar hosts (P06.T13, plan 13);
  - T13 (P06.T25's `ActivityLevel`);
  - T28.b (P06.T10.d's `max_radius_until`) and T28.c (P06.T19, P11.T4);
  - T29 (plan 09);
  - T35.c and T36 (P06.T33–T34);
  - T42.a's host symbols (P06.T35.b).

- **For the orchestrator to rule: telling "not yet modelled" from "does not apply".** T34 and T43.b
  tell a withheld section (`NOT RESOLVED`) from one this version does not compute
  (`NOT YET MODELLED`) by the granted level alone. That leaves a section that does not apply to a
  body's kind (a belt's surface, a free-floating object's orbit), and it leaves the question of
  where T41.b's system note gets its list. The options:
  - (a) The client decides. From the granted level and the body's kind it knows which sections
    apply, and the note's list is a client constant that each modelling task shortens. This is what
    the analysis behind ruling 33 assumes.
  - (b) The server says. `SystemBodiesDto` and `BodyDetailDto` carry a `not_modelled` list of
    section and small-body names for the generator version, from which the client builds both the
    readout's words and the note. A server at a later version then needs no client change to stop
    saying it.
  - (c) Each optional section on the wire becomes a three-way value: present, withheld or not
    modelled.

  Ruled (ruling 34): option (c) with a fourth state, so that every optional section is tagged `ok`,
  `not_resolved`, `not_modelled` or `not_applicable` by the server and the system note is composed
  from the tags (T34, T35.a, T41.b, T43.b).

- **For the orchestrator to rule: hosts without a `Track`.** By ruling 33, `Track` covers the
  Hurley, Pols and Tout range, and `evolve` dispatches below 0.1 M☉ to `substellar::cooling`.
  Consumes still reads `Track::{state_at, max_radius_until, max_luminosity_until}` for every host,
  in T12, T13.b and T28.b. A host of 0.08–0.1 M☉ has no `Track`, and neither has a free-floating
  brown dwarf. The options:
  - (a) Plan 06's `StarModel` (P06.T29.a) exposes `state_at`, `max_radius_until` and
    `max_luminosity_until` for every star and dispatches itself, and this plan reads hosts only
    through `StarModel` and `SystemStars`.
  - (b) `SystemContext` holds a host enum, a `Track` or a cooling fit, and T12, T13.b and T28.b
    dispatch on it.

  Ruled (ruling 34): option (a), this plan reads every star through `StarModel`, which exposes
  `state_at`, `lifetime`, `death`, `max_radius_until` and `max_luminosity_until` for every star
  below 0.1 M☉ included, and `SystemContext` holds `StarModel`s with no host enum for stars.

- **For the orchestrator to rule: the orbit map's centre and core distance.** `SpatialView` requires
  `centre` readings and a `coreDistance`, which the chart fills with its centre's `RADIUS`, `ANGLE`
  and `HEIGHT`. The orbit map's centre is a barycentre. The options:
  - (a) The system's galactic position, read as the chart reads its centre, with the system's
    `RADIUS` as the core distance.
  - (b) The barycentre named in words (`SYSTEM BARYCENTRIC` already is the frame), with the galactic
    position left to the system readout and the core distance still the system's `RADIUS`.

  Ruled (ruling 34): option (a), the `centre` is the system's galactic position, named in words as
  the barycentre, and `coreDistance` its galactocentric distance (T42.b).

- **For the orchestrator to rule: one phrase for an unborn system.** T41.b writes `NO SYSTEM YET`
  for an unborn host (the brainstorm's "no system yet", and plan 03's `Existence::NoSystemYet`).
  P06.T36 writes `NOT YET FORMED` for an unborn system in the `GALAXY` readout, and T43.a writes it
  for an unformed body. The guide's "one name per thing" asks for one. The options:
  - (a) `NO SYSTEM YET` for a system everywhere, which changes P06.T36's text, and `NOT YET FORMED`
    for a body.
  - (b) `NOT YET FORMED` for both, which changes T41.b's.

  Ruled (ruling 34): option (b), `NOT YET FORMED` for a system, a star and a planet alike, which
  T41.b now uses.

- **The vertical slice** (README, "The vertical slice to the `SYSTEM` display (2026-09-23)"). This
  plan's tasks in it are T1.a–d, T2.a–c, T3–T9, T10.a, T11.a–d, T12, T15, T16.a–b, T28.a–c,
  T30.a–c, T32 (fifteen of the twenty-four goldens), T34, T35 and T36 (without `body_events`),
  T37–T41, T42.a–c, T43.a–b and T44.a. Each task's _Slice:_ note says what it takes as a plain
  argument or `None` until its supplier lands. The first display has no moons, rings, belts, halo,
  events panel or `RUN`, and cannot reach brown dwarfs or rogue planets.
- **Deviations in P14.T2, as built** (round 7, in P11.T3.a's `orbit` module; see plan 11's
  record). All three subtasks are built.
  - **T2.a.** The precision requirements were built in from the start rather than checked
    afterwards. The phase is reduced exactly from the integer seconds and centred.
    `solve_kepler` has a fixed starter and a fixed count (plan 11's record gives both, and the
    residuals). `units::GravitationalParameter` is plan 11's addition.
  - **T2.a constructors.** `from_semi_major_axis(a, μ, e, Orientation, M₀)` fills the sketch's
    `..` with plan 11's `Orientation`. `scaled(factor, μ)` returns
    `Result<Self, BuildOrbitError>` rather than `Self`. It keeps e, the orientation and M₀. With
    `factor` = μ₀ ÷ μ(t), μ being the pair's total G(M₁ + M₂) (Veras et al. 2011), the specific
    angular momentum is conserved; for a planet of negligible mass that is M₀ ÷ M(t).
  - **T2.a tests.** All pass: 1 m after 10⁵ periods at 0.02 au, 1 mm after one period at 50 au
    at four times up to 8 × 10¹² s, energy and angular momentum to 10⁻¹² over 5,000 random
    orbits, and a one-day orbit bit for bit a thousand years out.
  - **T2.b.** `OpenOrbit::new(q, e, Orientation, time_of_pericentre, μ)` returns `Result`, with
    e ≥ `OpenOrbit::MIN_ECCENTRICITY` = 0.9999, so it also carries bound near-parabolic orbits.
    It refuses an orbit whose mean motion √(μ ÷ |a|³), with |a| = q ÷ |1 − e|, overflows or
    underflows (`BuildOrbitError::MeanMotionNotPositive`), so propagation never produces NaN.
  - **T2.b regimes.** A bound orbit's interval is always reduced exactly modulo its period
    first. Inside `NEAR_PARABOLIC_BAND` = 10⁻⁶ of e = 1, the universal-variable equation is
    solved from pericentre with the Stumpff series, by two Halley iterations. They start from the
    conic's own solution, E ÷ √α or H ÷ √−α, which the stable forms of the two Kepler solvers
    give accurately there, and from `solve_barker`'s parabola at e = 1 exactly. Above the band,
    `solve_kepler_hyperbolic` uses Mikkola's hyperbolic starter and two Halley iterations
    (relative residual 2 × 10⁻¹⁵ up to |M| = 10¹²), and it returns NaN for e ≤ 1. Below the band
    it is the ellipse.
  - **T2.b fix from review.** The first build started the band's iteration from the parabola on
    the unreduced interval. The science review found that this fails beyond about one period for
    a bound orbit in the band, so both changes above were made.
  - **T2.b conventions.** `solve_barker(M)` solves D + D³ ÷ 3 = M, with D = tan(ν ÷ 2) and
    M = √(μ ÷ 2q³)(t − τ), in a closed form that does not cancel for small M. The positions
    either side of each band edge agree to 1 m at q = 1 au over ±100 years. A bound orbit in the
    band matches the ellipse to 10⁻⁹ and repeats after a period, up to ten periods out.
  - **T2.c.** The signature is `elements_from_state(r, v, μ, t) -> Result<Orbit, InvertStateError>`,
    with `Orbit::{Bound(KeplerElements), Open(OpenOrbit)}`. The sketch's
    `Result<KeplerElements, OpenOrbit>` treated an open orbit as an error and had no way to report
    a state with no angular momentum (`Radial`), a component that is not finite, a pericentre
    time off the clock, or a valid state whose orbit overflows (`Unrepresentable`, which wraps
    the `BuildOrbitError`).
  - **T2.c conventions.** An eccentricity below 0.9999 is `Bound`, and anything else is `Open`.
    Where an angle is undefined, the node is set to 0 at i = 0 or π and the periapsis to the node
    for e = 0, and the anomaly is measured from that choice. Inside the band, the universal
    variable comes in closed form from both in-plane components, E = atan2(√α y′, 1 − α(q − x)),
    so a state beyond r = a keeps its branch.
  - **T2.c tests.** 10⁴ random bound states, including i = 0, π and e = 0, round-trip to 10⁻¹².
    3,000 open states, and bound states in the band beyond r = a, round-trip to 10⁻⁹. Far out on
    a hyperbola r ∥ v, so the state's own r × v is good only to about 10⁻¹⁶ ÷ sin(r, v).
  - **The fixture for T39** is `crates/hyperion-sim/tests/golden/orbit/states.golden`, written by
    `tests/orbit_states.rs` and blessed at 11. It holds 32 orbits, one per line:
    `state[NN] = a e i node peri m0 period mu t_s t_ns x y z vx vy vz`, in SI units and radians,
    with the format documented in the file's header. The floats are Rust's shortest round-trip
    `{:e}`, which `Number()` reads back exactly.
  - **The fixture's floats.** Writing them as decimals departs from the sim-determinism rule
    "never pin a float by its decimal form alone", because the TypeScript side must parse the
    file. The departure is safe: shortest round-trip decimal maps each finite double, −0
    included, to its own string, and the test asserts that every value written is finite.
  - **A second new golden**, `orbit/functions`, pins by bits what the fixture does not: both
    Kepler solvers, Barker's equation, the Roche lobe, Peters's time over all its panel counts
    and its inverse, open orbits in their three regimes, and the inverse from a state.
  - **`math::fmod`.** A wrapper of `libm::fmod` was added to plan 01's `math` so that the phase
    reduction does not lower `%` to the platform's `fmod`. Every correct `fmod` is exact, so the
    bits are the same either way.
  - **What the fixture asks of `orbit.ts`.** Two lines, a hot Jupiter and a white-dwarf binary
    each a thousand years out, agree to 10⁻⁹ only if the client reduces `t_s % period` before
    adding `t_ns`, as the server does. JavaScript's `%` is exact, and a float of seconds times
    the mean motion is not enough.
- **Deviations in P14.T1.b, T1.c and T3, as built (`planet`, round 7).**
  - _T1.b._ `planetary/{mod, error, index, context, params, system, record}.rs`, with `context`,
    `system` and `record` documentation only. `ResolveBodyError`'s variants carry their causes,
    `NoSuchSystem(ResolveSystemError)` and `MalformedIndex(DecodeBodyIndexError)`, with `From` for
    both and `source()`; `EncodeBodyIndexError` is `SlotOutOfRange`, `SubOutOfRange` and
    `SubNotInSlot { slot, sub }`, and `DecodeBodyIndexError` is `ReservedSlot { raw }` and
    `ReservedSub { raw }`. The "Plan 14" heading of `rng/tags.rs` lists every name and scope of
    Provides; `planet.disc` (`System`) is its one entry so far, the only line `tags.golden` gains.
    Plan 07's test that `gas.noise` is the registry's last tag now checks that it follows
    `gas.params`. `planetary/mod.rs` records the path of each consumed item the built pieces use;
    the full table of T1.a is left to the `doc` lane's reconciliation.
  - _T1.c._ `BodyIndex` has `new`, `decode`, `get`, `slot`, `sub`, `parent`, `body_id`, `PRIMARY`,
    `TryFrom<u16>` and `From<BodyIndex> for u16`, and orders by its raw value; `BodySlot` declares
    `SecondGeneration` before `Belt`, so that it orders as its slots do. `Planet(n)` is slot n
    (1–191) and `Component(n)`, `Moon(n)` and `Member(n)` sub-index n (from 1); `Ring(n)`,
    `Belt(n)` and `SecondGeneration(n)` count from 0 in their blocks, whose bounds are public
    constants (`STELLAR_SUB_END`, `LAST_PLANET_SLOT`, `SECOND_GENERATION_SLOT_START`,
    `BELT_SLOT_START`, `BLOCK_LEN`, `LAST_MOON_SUB`, `RING_SUB_START`). The stellar level's ring
    sub-indices decode in every system, because the index alone cannot tell a free-floating
    object's system from a star's. 33,936 of the 65,536 values are bodies. `parent()` gives a
    moon's or ring's planet, a stellar-level ring's object (`0x0000`) and a member's belt, and
    `None` for a primary or a component, whose parent the system resolves. `STELLAR_SUB_END` = 16
    stands in for plan 11's `STAR_BODY_INDEX_END`, which is not in the code yet; the merge should
    tie the two with a `const` assertion.
  - _T3, shape._ Plain arguments (ruling 33): `disc::derive` takes a `&DiscHost`, the lifetime in
    `Megayears`, the `&DiscDraws` and a `Truncation`, and returns a `Disc`. `DiscHost::new` takes
    the mass, [Fe/H], zero-age luminosity and zero-age radius, validated once
    (`BuildDiscHostError`); a truncation is `Truncation::NONE` with `with_inner` and `with_outer`.
    The draws are `DiscDraws::for_host(seed, system, host: u8)`, words 16h onwards of
    `planet.disc`: three standard normals and, at word 6, the rank of a circumbinary disc's
    lifetime, which `DiscDraws::circumbinary_lifetime(pair_mass)` turns into P06.T15.c's law; T9
    numbers the hosts (a single star is host 0). `DiscDraws`' fields are public variates, as plan
    06's `StarDrawsParts` are, and `DiscDraws::MEDIAN` sets each to its median. `Disc` is
    `None | Present(DiscProfile)`, `None` also when a far corotation radius meets a near `r_c`;
    T3.c's getters are `DiscProfile`'s, with `surface_density`, `gas_surface_density`,
    `characteristic_radius`, `corotation_period`, `host_mass` and `drawn_gas_mass` (`M_d`, the
    whole profile's) besides, and
    `Disc::{gas_mass, solid_mass}` are zero for `None`. `snow_line(SolarLuminosities)` and
    `isolation_mass(Σ, a, M★)` are free functions. `units` gains `KilogramsPerSquareMetre` and
    `KilogramsPerCubicMetre`. New goldens, blessed at version 11 with no bump: `planetary/disc`
    (draws and discs), `planetary/limits` (T6.a and T15) and `stellar/disc_lifetime`.
  - _T3, the solid share (for the orchestrator to rule)._ Lodders (2003) Table 11 has 0.489% of a
    solar-composition gas condensing as rock and 0.571% as water ice; Z☉ = 0.0149 is all heavy
    elements, so T3.a's "M_d × Z☉ × 10^[Fe/H], times 2 beyond the snow line" read literally makes
    the solids beyond the snow line 3% of the gas, twice every heavy element. As built the solids
    are 0.489% × 10^[Fe/H] of the gas inside the snow line and 1.060% × 10^[Fe/H] beyond (a step of
    2.17, the plan's factor 2). The median solar-mass disc has 33.2 M⊕ of solids (1.8 M⊕ of rock);
    the literal reading gives about 96. Left out: ammonia hydrate, a further 9% beyond 131 K
    (about 4.6 au × √L). Ruled (ruling 38, point 1): stands as built; with the outer edge of point 2
    the median solar-mass disc has 32.2 M⊕ of solids.
  - _T3, the outer edge (for the orchestrator to rule)._ "Outer radius r_c" was first built as the
    disc's edge, with M_d normalised between the inner edge and r_c. The physics review argued
    against it: Andrews et al. (2010, eq. 1) normalise M_d from 0 to ∞, so for γ = 1 a fraction e⁻¹
    (37%) lies beyond r_c, and a Kuiper-like belt needs solids there. Ruled (ruling 38, point 2):
    r_c is the characteristic radius. Σ is now normalised from the inner edge to infinity (∫ is
    r_c e^(−r_in ÷ r_c)), the untruncated outer edge is 3 r_c (`OUTER_EDGE_CHARACTERISTIC_RADII`),
    inside which 95% of the mass lies, and truncation still renormalises nothing. The median
    solar-mass disc keeps 0.0095 of its 0.01 M☉ of gas inside 90 au, with Σ of solids 2.2 g cm⁻² at
    1 au and more than 5 M⊕ of solids at 30–50 au. T3.b's text still says "outer radius r_c", for
    the `doc` lane.
  - _T3, the isolation mass (for the orchestrator to rule)._ Lissauer's feeding zone is 2√3 Hill
    radii either side of the orbit, 6.9 wide, not "10 Hill radii" (40% less mass than a 10-wide
    zone). Ruled (ruling 38, point 3): the zone stands as built, and the test's disc is Hayashi's
    (1981) minimum-mass nebula, solids of 7.1 g cm⁻² × (r ÷ 1 au)^−3/2 inside 2.7 au and 30 g cm⁻²
    × (r ÷ 1 au)^−3/2 beyond, which gives 1.15 M⊕ at 5 au against Kennedy and Kenyon's (2008, §2)
    "M_iso ≈ 0.1 (1) M⊕ at 1 (5) AU" (asserted 0.5–2 M⊕), and 0.039 M⊕ at 1 au, Armitage's
    (2007, eq. 202) 0.07 M⊕ at 10 g cm⁻² scaled to 7.1. The plan's 0.05–0.2 and 3–15 M⊕ are kept
    for a disc enhanced to Σ = 10 g cm⁻² (0.066 and 8.2 M⊕), as P14.T3.c's text now says. For
    T7.b, which caps each planet at `Disc::isolation_mass` × 10: the median solar disc's isolation
    mass is 0.0070 M⊕ at 1 au and 0.0012 M⊕ at 0.3 au, so that cap would forbid the compact class's
    1–20 M⊕ inside 0.3 au by three orders of magnitude. Ruled (ruling 38, point 4): direction for
    the T7 lane, which replaces the local cap with a cited solid budget from the disc's whole solid
    mass; the local isolation mass stays for giants' cores beyond the snow line.
  - _T3, measured._ Over 10⁵ hosts: gas fraction median 0.00998 of the star, σ 0.4995 dex (2,237
    at the cap); rotation period 7.99 d, σ 0.2503 dex; r_c 30.10 au at 1 M☉ and 15.05 au at
    0.25 M☉, σ 0.3005 dex. The integrated surface density returns M_s to 1.2 × 10⁻¹⁴, adjoining
    intervals add to 3.7 × 10⁻¹⁶, and the solid mass scaled as 10^[Fe/H] to the last bit at the
    five abundances measured (the test allows 4 ε). The gas-fraction test reads `drawn_gas_mass`,
    since `gas_mass` is now the 95% inside the edge. Unchanged by ruling 38: the draws, the
    medians and widths above, and the integrals, which were re-run with the edge at 3 r_c.
  - _T3, sources re-checked._ Recorded in `planetary/disc.rs`: Hayashi's 2.7 au (via Kennedy and
    Kenyon 2008, 170 K); the 8-day, 0.25 dex rotation period against Lee and Chiang (2017, Fig. 2)
    and Mulders et al. (2018, Table 2: innermost planets at 12 +3/−2 days, 0.22 dex wide); r_c
    against Andrews et al. (2010, Tables 4 and 5: 14–198 au, median 39 au, 0.36 dex; γ = 0.9 ±
    0.2) and Andrews et al. (2018, R_eff ∝ M★^0.58±0.10, 0.30 dex); the gas fraction against
    Andrews et al. (2013, §3.2.2: M_d ∝ M★, 0.2–0.6%, ±0.7 dex, so the plan's median is 0.3–0.7
    dex above their Class II discs); the cap against Kratter and Lodato (2016). Not in the model:
    dust sublimation, which puts a luminous host's inner edge far outside its corotation radius
    (about 0.07 au × √L), and the faster spin of A stars (Lee and Chiang's 1-day break).
- **Deviations in P14.T6.a and T15, as built (`planet`, round 7).**
  - _T6.a._ `placement/spacing.rs`: `mutual_hill_factor(m1, m2, host)` returns a `HillFactor` (χ),
    a type of its own so that it cannot be swapped with a spacing; then
    `mutual_hill_radius(m1, m2, host, a1, a2)`, `next_semi_major_axis(a1, Δ, χ)` (`None` from Δχ =
    `MAX_SPACING_STEP`, 0.9), `spacing_floor(m1, m2, e1, e2)`, `Neighbour::new(mass, a, e)` and
    `satisfies_floor(&inner, &outer, host)`, which is false when `outer` is not outside `inner`.
    Planet masses are M⊕ and the host's M☉; the floors, the giant threshold and 2√3 are in
    `params.rs`. The 10 and 12 are Pu and Wu's (2015, abstract and eqs. 12 and 14). The 7 is
    conservative: Chatterjee et al.'s (2008) three-giant systems, spaced in the same mutual Hill
    radius (eq. B2), are mostly stable for 10⁹ years at 5.5 (Fig. 29). For the orchestrator: Pu
    and Wu's slope is 100 per unit of σₑ, the Rayleigh scale, which is 80 per unit of mean
    eccentricity, so design note 7's 100 × mean e was a quarter steeper. Ruled (ruling 38, point
    7): `SPACING_FLOOR_ECCENTRICITY_SLOPE` is 80, the floor reaches 12 at a mean eccentricity of
    0.025, and design note 7 says so. Measured: Jupiter and
    Saturn 7.89 apart; Kepler-11 b and c 9.45 (Lissauer et al. 2013, Tables 3 and 4); two Jupiters
    at 5.2 and 6.5 au, 2.58.
  - _T15._ `derive/limits.rs`, masses in kilograms so that one function serves star and planet or
    planet and moon. `TidalPlanet::new(mass, radius, k₂, Q)` is validated
    (`BuildTidalPlanetError`); `maximum_surviving_moon_mass(&planet, primary_mass, a, e, age)`
    rests on `moon_mass_limit(outermost, &planet, age)`, Barnes and O'Brien's eq. 7 solved exactly
    (their eq. 8 drops the planet's radius, 6% for HD 209458 b: 6.5 × 10⁻⁷ against their
    7.0 × 10⁻⁷, reproduced when dropped). `satellite_stability_limit` takes the circular Hill
    radius: the fit carries the planet's eccentricity itself, and T15's `hill_radius`,
    a (1 − e) (m ÷ 3M)^⅓, would count it twice. All six of Domingos et al.'s coefficients are
    confirmed by their abstract. "Every Solar System moon": all 187 moons of JPL's mean-element
    table lie inside the fit, the nearest Aoede at 0.93 of its limit; the test carries 45 of them,
    the regular moons, Pluto's and the irregulars nearest their limits. Saturn's fluid limit for
    600 kg m⁻³ is 149,600 km, 2.57 mean radii and 2.48 equatorial; the bracket of 2.5–2.7 holds
    only in mean radii, which the test uses (for the orchestrator), though the A ring's edge, at
    2.27 equatorial radii, is inside the limit either way. Ruled (ruling 38): the bracket holds in
    mean radii, and the test says mean radii. Earth's Hill radius is 1.47 × 10⁹ m
    (1.50 circular).
  - _T15, for the orchestrator to rule._ (a) The moon-survival bound starts a moon at the
    generator's own 0.4895 R_H, not Barnes and O'Brien's 0.36, so it is 7.4 times theirs (7.8 for
    HD 209458 b, whose radius matters); T15's "no moon over 10⁻⁶ M⊕ at 0.05 au for 5 Gyr" then
    holds for rocky planets (Earth 7.6 × 10⁻⁹) but not for a Neptune (4.0 × 10⁻⁶) or a Jupiter
    (1.2 × 10⁻⁴), and the test asserts it for rocky planets. Ruled (ruling 38, point 5): stands
    as built, so that the moons the generator calls stable are the ones that can survive; the test
    says so and why. (b) Rosario-Franco et al. (2020, §3.1.1) find 0.40 R_H, not 0.49, when all
    twenty starting phases must survive 10⁵ years. Ruled (ruling 38, point 5): Domingos et al.'s
    0.4895 stands.
- **P14.T39 and ruling 35.6, as built (round 7b, `ui`).**
  - **Exports.** `lib/orbit.ts` exports `solveKepler(meanAnomalyRad, eccentricity)`, `stateAt(orbit, time)`, `positionAt(orbit, time)`, `orbitPolyline(orbit, segments)` and `composePosition(bodies, id, time)`; the types `KeplerOrbit`, `OrbitState` and `BodyPlacement`; and `KEPLER_HALLEY_ITERATIONS` (2) and `NEAR_PARABOLIC_ECCENTRICITY` (0.9999). `stateAt` is an addition that gives the velocity, which the fixture checks. The time is the wire's `UniverseTime`, not a float `timeS`.
  - **The orbit.** `KeplerOrbit` holds a, e, i, Ω, ω, M₀ and P, each with its unit in its name. It leaves out μ, since the period gives the mean motion, as it does on the server. T41 builds it from `OrbitDto` when P11.T13 lands.
  - **The method.** It follows `phase.rs`, `kepler.rs` and `orientation.rs` step for step. The whole seconds are reduced by `%`, which is exact as `fmod` is, and the nanoseconds are taken as a signed remainder about the nearer second. Then come the centring, Mikkola's starter, two Halley iterations, and the stable forms near periapsis.
  - **Refusals.** `RangeError` for e ≥ 0.9999, an a or P that is not positive, an inclination outside [0, π], an angle that is not finite, or a malformed time. The server sends near-parabolic orbits as open ones, and never a malformed time, so each is a bug upstream.
  - **The polyline and the composition.** `orbitPolyline` gives `segments` + 1 points from periapsis, evenly in E, with the first repeated last. `composePosition` takes a `ReadonlyMap<string, BodyPlacement>`, each body at the `origin` or on an `orbit` about a `parentId`. It sums from the origin down, so a moon is its planet plus its offset exactly, and it throws, naming the body, for a missing one or a chain that never reaches the origin. Hosts that orbit a barycentre wait for P11.T13's `HierarchyDto`, and until then a host is at the origin.
  - **The fixture.** The test reads the golden through Vite's `?raw` import, since the renderer may not import `node:*`, and checks all 32 lines. The worst relative disagreement is 3.0 × 10⁻¹⁶ in position (`state[26]`) and 2.1 × 10⁻¹⁶ in velocity (`state[13]`), and 30 of 32 positions and 29 of 32 velocities are the server's bits.
  - **Which lines need the reduction.** A float of seconds times the mean motion fails two lines: `state[22]` by 1.1 × 10⁻⁷ and `state[23]` by 1.5 × 10⁻⁸, the white-dwarf pairs a thousand years after and before the epoch. The hot Jupiter (`state[07]`) is off by only 3.8 × 10⁻¹¹ that way, as it is with the seconds divided by the period or reduced as a float. So the bullet above should read "the two white-dwarf binaries" for "a hot Jupiter and a white-dwarf binary", as should the fixture's header.
  - **Ruling 35.6 in T40.a.** A `reference` path and every annulus edge and belt tick are drawn in `--text-muted`, through a new `ColourToken`, `textMuted`, and `--line` is left to the grid and the plane's rings. A new test holds that: the `--line` ops of a scene with paths and annuli are those of the same plane without them. This supersedes the `--line` of the T40.a bullet above. A stale view draws both roles in `--text-muted`, as it draws every mark. **For the orchestrator to rule:** `--text` against `--text-muted` is only 1.88:1, both 1 px hairlines, so the selected orbit now stands out far less than it did against `--line`, and not at all when stale; the reticle still marks the selection by shape. T42 could draw the selected orbit wider (2 px), which T38.a's "hairline" does not allow as written. The UX review also notes the guide's reference table asks for "faint … orbit lines", which 6:1 is not.
- **Deviations in T11.a–c and T12, as built (`derive`, round 7).**
  - _Files and API._ `planetary/derive/{radius, envelope, composition, irradiation, habitable_zone}.rs`;
    `envelope.rs` is added beside T11's two files for the envelope model and its tables.
    `radius_chen_kipping(EarthMasses, UnitUniform) -> EarthRadii`, with `MassRadiusClass`;
    `radius_zeng(EarthMasses, CoreComposition) -> EarthRadii`, a `CoreComposition` being a core
    mass fraction and a water fraction (`IRON`, `EARTH_LIKE`, `ROCK`, `HALF_WATER`, `WATER`);
    `envelope::radius_with_envelope(mass, core, fraction, EarthFluxes, Gigayears)`;
    `composition(mass, radius, SnowLineSide, EarthFluxes) -> Result<SolvedComposition,
SolveCompositionError>`, whose `fractions()` are a `MassFractions` of iron, rock, water and
    envelope (the name `BulkComposition` is left to T23's hook, which holds these), with `core()`,
    `envelope_fraction()`, `radius()` and `adjustment()`; `HostLight::new(L, T_eff, R)` (ruling
    34), `Illumination::new(host, a, e)` with its `flux()`, `total_flux`, `BondAlbedo`,
    `equilibrium_temperature(EarthFluxes, BondAlbedo)`, `with_internal_heat`;
    `habitable_zone(L, T_eff)` and `habitable_zone_of(orbited, companions)`, a `HabitableZone`
    with the five limits, `conservative()`, `optimistic()` and `extrapolated()`. `units` gains
    `EarthRadii` (the mean 6,371 km, Zeng et al.'s unit), `JupiterRadii`, `WattsPerSquareMetre`
    and `EarthFluxes` (L☉ ÷ 4π au²), and `consts` σ and those radii; `params.rs` gains
    `ENVELOPE_CORE_FLOOR`, `INNER_WATER_CAP`, `OUTER_WATER_CAP`, `COMPOSITION_REFERENCE_AGE` and
    `BOND_ALBEDO_BEFORE_ATMOSPHERES`. New goldens `planetary/derive_radius` and
    `planetary/derive_irradiation`, blessed at version 11; no existing golden moved.
  - _T11.a._ Chen and Kipping's Table 1 is confirmed as the plan gives it (C = 1.008 R⊕; S =
    0.2790, 0.589, −0.044, 0.881; transitions 2.04 M⊕, 0.414 M_J, 0.0800 M☉), with σ = 4.03,
    14.6, 7.37 and 4.43%, which their eq. 3 makes dex of log₁₀ R. The median at 1 M_J is 13.77 R⊕,
    1.23 R_J, not the plan's 1.0 R_J to 10%: their Jovian segment is fitted mostly to irradiated
    hot Jupiters. The test asserts the paper's figure, and Jupiter's radius is T11.d's. Away from
    the median the radius steps at the transitions, since each segment has its own σ, as in the
    paper.
  - _T11.b._ Zeng et al.'s (1.07 − 0.21 CMF) M^(1/3.7) holds only for 1–8 M⊕ and CMF 0–0.4, has
    no iron curve, and misses Mars by 6% and Mercury by 10%, so their Table 2 (0.125–32 M⊕; 100,
    50, 30, 25, 20% iron, rock, 25, 50, 100% water) is used, log–log in mass and linear in CMF; it
    meets the power law to 0.025 R⊕ inside its range. Water is Zeng et al.'s (2019) f = 1 + 0.55x
    − 0.14x², applied as a blend towards the pure-water curve, on an Earth-like core. The envelope
    table is Lopez and Fortney's (2014) Tables 2–4 (solar metallicity; 1–20 M⊕, 0.01–20%, 0.1,
    10 and 1,000 F⊕, 0.1, 1 and 10 Gyr), with R = R_Zeng(core) + (R_LF − M_core^¼), their own
    decomposition (it reproduces their table to 3.2%). It is read in flux, their axis, not T_eq,
    which would bring in an albedo their models already contain, and it keeps their age axis,
    which the plan's small table lacked. Beyond 20% the radius bridges, log–log in the envelope
    fraction, to Fortney, Marley and Barnes's (2007) coreless radius (Tables 2–4): this module's
    construction, within 10% of their cored models. On an icy core the thickness is laid on
    that core's own radius; their models have Earth-like cores, and they put the error of varying
    the core's iron alone at about 10% (their §3.1). Where two printed entries are equal the
    radius can fall by up to 1.1 × 10⁻⁵ of itself as the envelope grows; the solve's bisection
    needs only continuity.
  - _T11.c._ `composition` takes the flux, not `t_eq`, and returns the radius it keeps: iron
    raises a radius, the rock and ice clamps and the envelope limit lower one
    (`RadiusAdjustment`). The envelope is solved at 5 Gyr, Lopez and Fortney's representative age,
    since Chen and Kipping describe mature planets. Inside the snow line the rules are the plan's,
    the envelope on an Earth-like core (theirs), so the composition steps from pure rock to 32.5%
    iron at the rock curve while the radius stays continuous. Beyond it, where the plan is silent:
    above the Earth-like curve, water on an Earth-like core up to 53.9%, the ice share of the
    disc's solids (Lodders 2003, Table 11, as in `disc`), then an envelope on that core. The 1.5 M⊕
    floor applies on both sides, and as a floor on the core (f ≤ 1 − 1.5 M⊕ ÷ M). From 0.414 M_J
    `composition` returns `SolveCompositionError::GiantPlanet`, the seam for T11.d. Measured:
    Mercury CMF 0.708, Venus 0.288, Earth 0.323, Mars 0.216; envelopes of 8.2% for Uranus, 6.4%
    for Neptune and 71% for Saturn (about 74% by Fortney et al.'s 25 M⊕ of heavy elements).
  - _T12._ T_eq is computed from the flux, so that fluxes add, and equals the plan's
    T★ √(R★ ÷ 2a) form to 10⁻⁴ for the nominal Sun. Measured: Earth 255.1 K (NASA's current Bond
    albedo 0.294), Venus 229.1 K at the plan's A = 0.76 (226.6 K at NASA's 0.77), Mars 210.1 K,
    Jupiter 109.9 K. Kopparapu et al.'s coefficients are the erratum's Table 3, checked on the
    erratum (ApJ 770, 82): the Sun's zone is 0.993–1.690 au conservative (moist to maximum
    greenhouse, the paper's §5) and 0.751–1.767 au optimistic. The erratum's own Table 1 prints
    1.67 and 0.97 au for the maximum and runaway greenhouse, where its coefficients give 1.690
    and 0.982. A limit is +∞ where companions alone give more than its flux, and zero about a
    dark host. The fit is for main-sequence stars, and about a giant it is applied through T_eff
    alone, which is all it reads; `extrapolated` flags only a temperature outside it. The track test is written against a documented sequence of (L, T_eff) at 25 ages
    from this crate's `stellar::sse` phases (1 M☉, Z = 0.02, the mass held fixed), since the
    integrator is not merged; it should be rerun on the integrator's track when P06.T10.c–e lands.
  - _For the orchestrator to rule._ (1) Chen and Kipping's scatter meets the solve's clamps
    (measured over 999 quantiles at 10 F⊕): inside the snow line 1.4% of 1 M⊕ bodies are raised
    to pure iron, 30% come out over 50% iron and 27% are clamped to rock; just above 2.04 M⊕,
    where σ jumps from 0.040 to 0.146 dex, 25% are raised to pure iron (15% at 3 M⊕, 6% at 5
    M⊕); at 100 M⊕ 56% are lowered to the envelope limit. Super-Mercuries so common contradict
    the CMF of 0.26 ± 0.07 Zeng et al. (2016) find. A remedy that changes no draw: T16 maps the
    quantile onto the part of Chen and Kipping's distribution that the curves can hold (a
    truncated normal), or T11.a narrows σ for dry bodies inside their curves. (2) The envelope
    bridge above 20%, and the 5 Gyr reference age. (3) The outer water cap at the disc's ice share,
    and the 1.5 M⊕ floor beyond the snow line. (4) An Earth-like core under an inner envelope (a
    step in composition at the rock curve) rather than pure rock (continuous, but iron-free).
- **Deviations in P14.T6.b and T9, as built (`zones`, round 7).**
  - _T6.b, shape._ In `placement/spacing.rs`, `SpacingKind` is `SmallPlanets`, `TerrestrialGroup`
    or `GiantPair`, each with `law() -> MeanSpacingLaw` (`centre`, `sigma`, `min`, `max` and
    `mean(z)`). `SpacingDraws::for_host(seed, system, host)` holds three public `StandardNormal`s,
    with `MEDIAN`, `normal(kind)` and `mean_spacing(kind)`. The pair's draw is
    `draw_pair_spacing(seed, system, outer: BodyIndex, mean, floor) -> PairSpacing`, whose
    `outcome()` is `SpacingOutcome::Drawn { redraws }` or `Floor`. A template's spacing law (T5)
    can name a `SpacingKind`. The law's figures live in `spacing.rs` beside their sources, as the
    disc's do, and not in `params.rs`.
  - _T6.b, one mean per host and kind._ "A system draws a mean spacing μ" is read per orbit host
    and per kind: each zone is its own host (D10), and a `SolarLike` host has both a terrestrial and
    a giant μ.
  - _T6.b, the floor._ It is an argument, `spacing_floor` of the pair's masses and eccentricities.
    T8.d draws eccentricities after the spacing, so at draw time a placer can pass only the
    circular floor, or the floor of an eccentricity it assumes.
  - _T6.b, draw numbers._ All on `planet.spacing` (`System`), the one line `tags.golden` gains. Host
    h's three normals are words 8h, 8h + 2 and 8h + 4 of the eight it owns
    (`SPACING_WORDS_PER_HOST`, so 256 hosts fill words 0–2,047). The pair whose outer planet is in
    slot s reads words 2,048 + 64s onwards, two per attempt, so a placer assigns a planet its slot
    before drawing its spacing and never renumbers it. "Redrawn … at most 16 times" is read
    literally: the draw and 16 redraws, 17 attempts in all, then the floor.
  - _T6.b, holds (for the orchestrator to rule)._ The plan holds only the small planets' mean
    (13–24). A terrestrial mean of 30 ± 8 or a giant mean of 9 ± 2 unheld can fall under its floor,
    which would put every pair of the host at the floor. As built they are held to 14–46 (two σ,
    clear of the small planets' highest floor of 12) and 7–13 (the giants' floor, and two σ above).
    Ruled (ruling 52, 1): stands as built.
  - _T6.b, sources._ Weiss et al. (2018, AJ 155, 48, §5.2 and Fig. 14) find 93% of CKS pairs at
    least 10 apart and a peak near 20, and note that a wide pair may hide a planet. Pu and Wu (2015,
    abstract) find the pairs of systems with four or more transiting planets "tightly clustered
    around 12 mutual Hill radii" once transit geometry and sensitivity are accounted for, below the
    plan's centre of 17. The plan's 30 and 9 have no source. Computed from JPL's masses, the Solar
    System's Venus and Earth are 26.3 apart, Earth and Mars 40.1, Jupiter and Saturn 7.9 and Saturn
    and Uranus 14.0.
  - _T6.b, measured._ Over 10⁴ small pairs of 2,000 systems at the circular floor of 10, the least
    accepted Δ is 10.003 and the median 17.15, with 12.6 and 22.1 at the 10th and 90th percentiles,
    and no pair at the floor. The accepted spacings about a fixed mean are the normal truncated at
    the floor (Kolmogorov–Smirnov p = 0.078). Over 4 × 10⁵ pairs about a mean at the floor, the
    redraw counts are geometric out to 16. The means' medians and the shares held at each end
    match their laws at α = 10⁻³. T6.a's tests stand as they were: the Solar System passes
    pairwise, two Jupiters at 5.2 and 6.5 au fail, and so do Kepler-11 b and c at 9.45.
  - _T6.b, an M dwarf's chain._ Δ in mutual Hill radii is the same for every host, so the period
    ratio per step grows as the host's mass falls. At 0.3 M☉ and the median 17.15 it is 1.63 for a
    pair of 1 M⊕ planets, 2.04 for 3 M⊕ and 2.34 for 5 M⊕. A chain of 3 M⊕ planets from 5 days
    reaches 175 days at its sixth planet and 358 at its seventh; one of 1 M⊕ planets from 10 days
    stays inside 200 days to its seventh. How much of a chain lies beyond 200 days is therefore
    T7's masses and T5's first period as much as T6.b's spacing (ruling 48(e)).
  - _T9.a._ Every coefficient of equations 1 and 3 was checked against Holman and Wiegert (1999)
    itself (arXiv astro-ph/9809315), and all thirteen are as the plan gives them. Their μ is m₂ ÷
    (m₁ + m₂), where m₁ is the star the planet orbits.
    - The S-type fit covers 0.1 ≤ μ ≤ 0.9 and e ≤ 0.8 (their Table 3), to 4% typically and 11% at
      worst.
    - The P-type fit covers 0.1 ≤ μ ≤ 0.5 and e ≤ 0.7 (their Table 7), to 3% and 6%. The paper's
      text says 0.9, but its integrations stop at 0.5, and its §2 states the μ ↔ 1 − μ symmetry.
    - The built fits meet Tables 3 and 7 to 11.4% and 6% at worst. The 11.4% is at μ = 0.6,
      e = 0.7, where Table 3's 0.05 has one significant figure.
    - Measured: 0.274 and 2.3875 at μ = 0.5, e = 0. With Holman and Wiegert's own α Centauri (Table
      4: 23.57 au, e = 0.516, 1.12 and 0.95 M☉) the limits are 2.794 au around A, 2.54 around B and
      87.4 around both, against their table's 2.79, 2.54 and 87. At the plan's 23.5 au and
      e = 0.52, A's limit is 2.76 au.
  - _T9.a, outside the fitted ranges (ruled, ruling 52, 2: stands as built)._ "Clamped, which errs towards
    smaller zones" holds at two edges only, S-type μ < 0.1 and P-type μ < 0.1, which are clamped. At
    the other edges a clamp errs larger, so each fit is continued instead:
    - S-type μ > 0.9 (a light host) takes the Hill scaling ((1 − μ) ÷ 0.1)^⅓ that the paper finds
      there (its §3.1 and Fig. 1). It was measured at e = 0 only, so using it at every e is an
      assumption.
    - S-type e > 0.8 scales by ((1 − e) ÷ 0.2)^1.2 (`S_TYPE_ECCENTRICITY_EXPONENT`), the law of
      Jaime, Aguilar and Pichardo (2014, MNRAS 443, 260, eq. 11), R = R_Egg 0.733 (1 − e)^1.2
      q^0.07, after the invariant loops of Pichardo et al. (2005). For equal masses it gives Table
      3's 0.04 at e = 0.8, and it closes faster than the companion's pericentre. A linear (1 − e),
      first built, errs larger than both this law and the fit's own slope there, (1 − e)^1.4, as
      the science review found. For γ Vir (e = 0.881) Holman and Wiegert extrapolate the polynomial
      to 0.61 au, and this gives 0.74.
    - P-type μ is folded to min(μ, 1 − μ). Unfolded, μ = 0.9 on a circular orbit would give 1.19
      separations against the 1.96 of its mirror image, μ = 0.1.
    - P-type e > 0.7 scales by (1 + e) ÷ 1.7, the same multiple of the apocentre, which Table 7
      holds at 2.3–2.6 from e = 0.5 to 0.7. Jaime et al.'s circumbinary law (eq. 12) grows more
      slowly with e, so this errs towards the smaller zone.

    Each continuation is continuous at its edge, and inside the ranges both functions are the
    published polynomials to the bit.

  - _T9.b, the hierarchy._ Plan 11's `SystemHierarchy` is not built, so `stable_zones` takes a
    `ZoneHierarchy`, built from `ZoneNode::component(index, mass, kind)`, where `ComponentKind` is
    `Star` or `BrownDwarf`, and `ZoneNode::pair(inner, outer, a, Eccentricity)`.
    - It is checked once (`BuildZoneHierarchyError`): indices 0 to n − 1, unique and under 16,
      masses and semi-major axes positive, and pair keys unique.
    - It does not check plan 11's numbering (depth first, inner before outer). The zones' order by
      body index and the range 1–15 of `Pair(k)` rely on it.
    - At the merge, a walk of `SystemHierarchy` from its root builds one, and
      `stable_zones(&SystemHierarchy)` goes through that adapter. _Second pass:_ built as
      `ZoneHierarchy::from(&SystemHierarchy)`, with `stable_zones` still taking a `ZoneHierarchy`
      (see the second pass's bullet below).
  - _T9.b, zones._ `OrbitZone` has `host()`, `host_number()`, `host_mass()`, `component_kind()`,
    `members()`, `inner()`, `outer()`, `truncation()` and `in_close_binary()` (the second pass
    replaces the last with `host_multiplicity()`). A limit is `None`
    where the hierarchy sets none: a component has no inner limit, and the top of the hierarchy no
    outer one (D14's strip radius is T29's).
    - `OrbitHost::Pair(k)` names a pair by the lowest-indexed component of its outer member,
      plan 11's key star (its D5), so the name survives any node layout. `Barycentre` and `Body`
      are defined, but no zone has either. **For the orchestrator to rule:** should the root
      pair's zone be `Barycentre`? Ruled (ruling 52, 4): yes, keeping its host number; built in
      the second pass.
    - Host numbers, for `DiscDraws::for_host` and `SpacingDraws::for_host`, are a star's body
      index n and 16 + k for a pair. A single star is host 0, and a star's disc draws do not change
      when it gains a companion.
    - Zones come depth first, inner member before outer, each pair after its members. A pair's
      mass is its members' summed up the tree, inner first, and a golden pins that order.
  - _T9.b, added beyond the plan._ Every zone is also bounded by the zone of the pair above it,
    less its own greatest distance from that pair's barycentre, a (1 + e) μ. This makes zones
    disjoint for any hierarchy, and it binds only in hierarchies too tight to be stable. A
    component's zone with no room left is dropped, as is a pair's zone narrower than 1.5.
    _Superseded (ruling 53):_ it binds in drawn, stable hierarchies too, and a component is now
    bounded at its closest approach to each outer companion instead (the second pass's bullet
    below). Pairs' zones keep this bound.
  - _T9.c._ `ZoneDiscInputs::for_zone(seed, system, zone, stars: &[ZoneStar], fe_h)` has `host()`,
    `lifetime()`, `draws()`, `truncation()` and `derive()`. A `ZoneStar` is a component's zero-age
    L and R and its `star.disc_lifetime` rank, plain arguments until T1.d's context; an invalid one
    is `ResolveZoneDiscError::InvalidStar` with its index. A circumbinary host sums its members'
    luminosities in index order and takes their largest radius. The radius enters only the inner
    edge, which the P-type limit overrides, except where a close pair's corotation radius lies
    beyond it.
  - _T9.c, the close-binary flag (ruled, ruling 52, 3: stands as built)._ The cut is Kraus et al.'s (2016,
    abstract) measured a_cut = 47 (+59/−23) au, not the plan's rounded 50. A zone is flagged when
    its host is a member of such a pair at any level above it, and never for its own circumbinary
    zone, since that disc is cleared from inside and not truncated from outside. The weight moved
    to `Barren` is `arch`'s, to give S_bin = 0.34 (+0.14/−0.15).
  - _T9 tests._ (a) and (b) run on hand-built hierarchies. Overlap is checked at the worst phases on
    3,000 random hierarchies of three to six components. Of (c), "every zone's disc lies inside its
    zone" runs on 10⁴ random binaries and triples. "No planet outside its zone" waits for T8's
    placer, the planet-occurrence ratio for T4's weights with the flag, and both for plan 11's
    sampled binaries. The zones are those at birth (the slice). _Second pass:_ the zones' part of
    (c) now runs on 10⁴ drawn multiples; the planets' part and the ratio wait for T8.
  - _Goldens._ New at version 11: `planetary/spacing` and `planetary/zones`, written by
    `tests/planetary_placement_golden.rs`. Nothing existing moved.
- **Deviations in T4 and T5, as built (`arch`, round 7).**
  - _Shape_ (ruled, ruling 48, h: T5's own file stands). `planetary/architecture.rs` holds T4 (the classes, the frequency model, the anchors
    and the draw in its module documentation, `ARCHITECTURE_TABLE` of `ClassRow`s) and
    `planetary/architecture/template.rs` holds T5, a file of its own so that T5's accept filter
    `planetary::architecture::template` names a module. `ArchitectureClass` has `ALL`, `index`,
    `name`, `has_giants` and `giant_free_sibling`; `class_weights(SolarMasses, Dex) -> ClassWeights`
    (`get`, `total`, `giant_total`, `probabilities`, `constrained`); `ClassProbabilities`
    (`get`, `giant_share`, `compact_share`). D5 and D10 enter as
    `ClassConstraints::new(&Disc, ZoneLimit, HostMultiplicity)`, with
    `ZoneLimit::{Unbounded, Outer(Metres)}`, `HostMultiplicity::{SingleOrWide, CloseBinary}`,
    `ClassConstraints::NONE` and `capacity() -> DiscCapacity::{None, SmallPlanets, Giants}`;
    `ClassWeights::constrained` applies them. The weight table and its constants live in
    `architecture.rs`, not `params.rs` (whose documentation points there), so that the module
    documentation is their single written definition. `ClassDraw::for_host(seed, system, host)` reads word
    4h of `planet.class` (`CLASS_WORDS_PER_HOST`, words 4h + 1 to 4h + 3 reserved) and
    `ClassDraw::class` picks with `Mark::pick_weighted` against the weights' own total, which is
    `Thresholds::from_weights` with `Mark::pick` without the allocation (tested equal);
    `draw_class(seed, system, host, &weights, &constraints)` does both. The scaling laws are
    public: `giant_host_mass_scaling`, `giant_metallicity_scaling`,
    `small_planet_metallicity_scaling`. `planet.class` (`System`) is the one new tag.
  - _Anchors re-checked_ (papers under the lane's `target/scratch/papers/arch/`): Cumming et al.
    (2008) 10.5% and α = −0.31; Wright et al. (2012) 1.2 ± 0.38%; Howard et al. (2012) 0.4–0.5%;
    Zhu et al. (2018) 30 ± 3% and 3.0 ± 0.3 per system; Zhu and Wu (2018) 32 ± 8%; Dressing and
    Charbonneau (2015) 2.5 ± 0.2; M-dwarf giants (Johnson et al. 2010's 3% at 0.5 M☉, Cumming's
    1.0%, Bonfils et al. 2013, Montet et al. 2014's 6.5 ± 3.0%); Johnson et al.'s α = 1.0 ± 0.3
    (their β is 1.2 ± 0.2, not 2); Reffert et al. (2015) µ = 1.9, σ = 0.5 M☉. Fischer and Valenti
    (2005) and Buchhave et al. (2012) against their abstracts only (no preprint; the full texts
    were not reachable), so the ±0.5 of the giant law's range is plan 14's reading.
  - _Weights and scalings changed, each for its source._ `CompactWithColdGiant` 0.07 → 0.10 and
    `CompactMulti` 0.24 → 0.21 (Zhu and Wu's third; the draft gave 23%); `SolarLike` 0.03 → 0.01
    (Zhu and Wu's ∼1% of cold-Jupiter systems without super-Earths; Wittenmyer et al.'s Jupiter
    analogues come from `CompactWithColdGiant`; with the eccentric and outer giants the table gives
    4.1% of stars a cold Jupiter without super-Earths and P(SE ∣ CJ) ≈ 73%, inside Zhu and Wu's
    90 ± 20%); `CompactMulti`'s exponent −0.9 → −1.5, with the chain count rising to M dwarfs
    (below), so that a 0.48 M☉ host has 2.4 small planets if every chain planet lay inside 200
    days, about 1.9 inside them at T6.b's spacing (the science review's estimate), against
    Dressing and Charbonneau's 2.5 ± 0.2 (the draft gave 1.4 counting every planet); Reffert et al.'s width 0.8 → 0.5 M☉. Then, by ruling 48 (c), so that the anchors hold after D5's fallback: the giant classes' w₀ × 1.373 (`GIANT_WEIGHT_SCALE`: 0.1373, 0.01373, 0.05492, 0.02746, keeping their ratios) and `HotJupiter` 0.008 → 0.0103 (`HOT_JUPITER_WEIGHT`), solved together over the solar-disc sample (20,000 discs of a zero-age 1 M☉, \[Fe/H\] = 0 host, 83.4% of which can form a giant). Below \[Fe/H\] = −0.5 the giant weight is thinned by
    s(\[Fe/H\]) ÷ s(−0.5), not only held, since held it gave 2.2% of hosts at −2.5 a giant against
    0.7% small planets alone, against every survey's order and Mortier et al.'s (2012, A&A 543,
    A45) fall "even in the low-metallicity tail"; Sozzetti et al.'s (2009) fp < 0.67% is met
    either way. T4.c's "constant outside it" holds above +0.5 only. What s and z take goes to `Barren`. `SubstellarCompact` has the two
    small-planet classes' weight at 0.08 M☉ (`galaxy::imf::MASS_LIMIT_LO`), the giants none.
  - _T4.c as measured._ Probabilities sum to 1 to 10⁻¹² over 0.01–150 M☉ and −2.5 to +0.5; the giants' summed weight is 10^(2Δ\[Fe/H\]) to 10⁻¹² on −0.5 to +0.5; the giant share's log-slope between −0.5 and −0.2 is 1.88 at 1 M☉ and 1.98 at 0.3 M☉; in the table, before the fallback, at 0.3 M☉ giants 0.038 and compact 0.679, at 1 M☉ giants 0.234, compact 0.333, hot Jupiters 0.0099 and cold giants in 0.396 of compact systems; the small-planet weights are 0.963 of solar at −0.8 and 0.091 at −2. After the fallback, over the solar-disc sample at 1 M☉ and \[Fe/H\] = 0 (`the_anchors_hold_after_the_disc_fallback`): giants of 0.3–10 M♃ at 2–2,000 days 10.50% (Cumming's 10.5%, with the templates' orbits), hot Jupiters 0.823%, a cold giant in 32.4% of compact systems, giants in all 19.5% (the plan's 0.14–0.20 now holds here, not in the table), compact 33.9%. The
    6 × 6 golden (`planetary/architecture`, with two constrained rows, four draws and every
    template's figures, which nothing reads until T8) is new and blessed at 11; the chi-square of
    2 × 10⁵ draws (two cases) runs in the ordinary suite.
  - _D5's fallback._ No disc gives `Barren`. Giants need the snow line inside the nearer of the
    disc's edge and the zone's outer limit, and 10 M⊕ (`GIANT_SOLID_BUDGET`) of solids between
    them; otherwise `SolarLike` and `EccentricGiant` fall to `TerrestrialOnly`, and
    `CompactWithColdGiant`, `WarmGiant` and `HotJupiter` to `CompactMulti`. T7.c's "within the
    disc lifetime" is left to T7.c, which has the growth law. With T3's draws, 83.4% of solar discs can form giants, 67% at 0.5 M☉, 50% at 0.3 M☉ and 48% at \[Fe/H\] = −0.5; the rescaling above makes the anchors hold after it. Ruled (ruling 48, c): fixed as described.
  - _D10._ A close binary keeps 0.34 of every planet-bearing class (Kraus et al. 2016's
    `S_bin`); the cut-off is theirs, 47 (+59 −23) au, as `CLOSE_BINARY_CUTOFF_AU`, which the
    caller (T9.c) compares, where plan 14 says "about 50 au". Ruled (ruling 48, g): Kraus's 47 au stands.
  - _T5._ `ClassTemplate` (groups inside out, `quiet_inside`, the two `BeltRule`s) of
    `PlanetGroup`s (role, presence, `CountLaw`, `MassRange` with `MassLaw`, `Location`, `Reach`,
    `SpacingFamily`, `EccentricityLaw`, `Origin`, an optional `HotVariant`), `TEMPLATES` and
    `template(class)`. Beyond the plan's four fields it carries the eccentricity law (T8.d's
    values and Kipping 2013's short- and long-period Betas), the origin that sets
    `formed_beyond_snow_line`, and the presence of optional groups. The chain's count is a
    zero-truncated Poisson with λ = 3.38 (max(M, 0.48 M☉) ÷ M☉)^−0.82, at most 10: mean 3.5 at
    1 M☉, 6.1 at and below 0.48 M☉ (Ballard and Johnson 2016's 6.1 ± 1.9 for M dwarfs), 2.9 at
    1.3 M☉; the plan's cap of 7 is 10 (Mulders et al. 2018), since a mean of 6.1 cannot live
    under 7. The first period is Mulders et al.'s (2018) broken power law, break 12 days, indices
    1.6 and −0.9, truncated to 1–50 days. From the science review: the warm giant is at 10–200
    days, Huang et al.'s definition, not 0.1–1 au × √L, which around a 0.5 M☉ host is 1.6–49 days
    and so a hot Jupiter; it is `Origin::Migrated`, since Huang et al. propose in-situ formation
    for those with companions; every giant but the eccentric ones takes Kipping's (2013, Table 2)
    two Betas by the drawn period (split at 382.3 days, `EccentricityLaw::BetaByPeriod`), not by
    group; the hot Jupiter's outer giant is 1–10 M♃, so that 56% of hot-Jupiter systems have one
    in Bryan et al.'s (2016) 1–20 M♃ at 5–20 au (their 52 ± 5% overall, hot giants more often);
    `quiet_inside` of 100 days is plan 14's, Huang et al. having 50. Choices of this lane where
    the plan gave no figure: the hot Jupiter's σ of 0.15 dex, its outer giant at 2–8 snow-line
    radii in 60% of systems, the ice-rich bodies' 0.02–5 M⊕, the survivor's 0.05–10 M⊕, the ice
    giants' 10–30 M⊕, the rocky groups' start at 0.2–0.5 au × √L and the warm giant's companions
    at 1–2.
  - _Put to the orchestrator, and ruled by ruling 48._ (a) `SubstellarCompact`'s weight: continuous with the stars'
    at 0.08 M☉ makes 97.6% of brown dwarfs host a chain, which no survey measures; the alternative is a lower figure with no source. Ruled (ruling 48, a): deferred to P14.T27, which sets `SubstellarCompact`'s weight from a cited occurrence estimate for ultracool dwarfs; the slice never reaches a brown-dwarf host. (b) −1.5 extrapolates below the Kepler M dwarfs:
    a 0.1 M☉ host is 92% compact. Ruled (ruling 48, b): stands, for T10.b to check against a cited mid-to-late M-dwarf occurrence (Hardegree-Ullman et al. 2019 the candidate); a figure outside is a finding against the exponent. (c) Whether the giant w₀ should be raised so that the anchors hold after D5's fallback rather than before it. Ruled (ruling 48, c): yes, fixed as above (× 1.373, and hot Jupiters 0.0103). (d) `Barren` taking the halo's loss, and the giants thinned below −0.5. Ruled (ruling 48, d): stands as built. (e) Dressing and Charbonneau's 2.5 ± 0.2 is not met
    inside 200 days (about 1.9, inside T10.b's 1.8–3.2 window) because T6.b's spacing of about
    17 mutual Hill radii carries an M dwarf's chain past 200 days; meeting it needs a steeper
    compact exponent (about −2.8, over 99% compact at 0.1 M☉) or tighter M-dwarf spacing, which T6.b and T10.b should settle together. Ruled (ruling 48, e): settled jointly by T6.b's spacing and T10.b; the `zones` lane is told. (f) Counting `TerrestrialOnly`'s Earth-mass planets, η at
    1 M☉ is roughly 40–50% against Zhu et al.'s 30 ± 3% (Yang et al. 2020 find 73 ± 13%): the
    `Barren`–`TerrestrialOnly` split, or T7's rocky masses, is the dial. Ruled (ruling 48, f): stands; Zhu's 30% counts Kepler-like systems, and T10.b checks η⊕ against Bryson et al. (2021), 0.37–0.60 in the conservative zone. (g) and (h) are recorded under _D10_ and _Shape_.
- **Deviations in T35.a, as built (round 7, `wire`),** with P06.T33 and P11.T13's slice.
  - `BodyIdHex` is in `primitives.rs`: `from_parts(system: u64, body_index: u16)`, `to_parts()`,
    `as_str()`, `TryFrom<String>` and `Display`. `ParseBodyIdHexError` has `WrongLength`,
    `MissingSeparator` and `InvalidDigit`, with one `Display` text, as `ParseHex64Error` has. Tests
    round-trip every body index and refuse 17 malformed forms.
  - `DetailLevelDto` (`contact` to `full`) derives `Ord` in the plan's order.
  - `SectionDto<T>` is adjacently tagged: `{"state": "ok", "value": …}`,
    `{"state": "not_resolved"}`, `{"state": "not_modelled"}`, `{"state": "not_applicable"}`.
    Internal tagging would put the value's fields beside `state` and cannot carry a list, such as
    a planet's moons. **For the orchestrator to rule.**
  - `BodyOrbitDto.parent` is an `OrbitHostDto`, not a `BodyIdHex` (ruling 53), mirroring the sim's
    `planetary::placement::OrbitHost` and tagged by `type`: `{"type": "star", "body_index": n}`,
    `{"type": "pair", "key_body_index": k}`, `{"type": "barycentre"}` and
    `{"type": "body", "id": …}` for a moon's or a ring's planet, with one wire-form pin each. `k`
    names the pair as the sim does, by its outer member's first star, so a client finds it in the
    system's `HierarchyDto` as the pair node whose outer child begins with star k.
  - `valid_until` is an `Option<UniverseTime>`, `null` when no change falls inside the clock
    window, as the `record` lane's `BodyOrbit::valid_until` is.
  - `ErrorCode::UnknownBody` lands here rather than in T35.c, with its `settledState` case, as the
    brief asked.
  - In `@hyperion/protocol`, T37's `parseBodyId` and `formatBodyId` land here too, in `hex.ts`,
    with `isBodyId` and a `BodyIdParts { system, bodyIndex }` interface. They throw `SyntaxError`
    for a malformed string or system, and `RangeError` for an index that is not an integer from 0
    to 65,535. T37's test that the Rust wire-form fixtures decode waits for T35.b–c's records.
- **Deviations in P14.T9.b–c, as built (`zones`, second pass, round 7).**
  - _The adapter._ `impl From<&SystemHierarchy> for ZoneHierarchy` walks plan 11's hierarchy
    from its root: each star's slot becomes a `ZoneNode::component` of its index, initial mass
    and `SlotKind`, and each pair a `ZoneNode::pair` of its orbit's semi-major axis and
    eccentricity. Its pair masses are plan 11's `node_mass` bit for bit (tested over 2,000 drawn
    hierarchies). `stable_zones(&ZoneHierarchy)` stays the one function that makes zones, so a
    generated system's zones are `stable_zones(&ZoneHierarchy::from(&hierarchy))`: that is how
    the Provides' `stable_zones(&SystemHierarchy)` is reached.
    - `ZoneHierarchy` stays because the zones' tests and golden need hierarchies that no draw
      gives: named systems, hierarchies too tight to be stable, and brown dwarfs before P11.T2.d.
      `SystemHierarchy` is stable by construction and has no public constructor, and giving it
      one would weaken that guarantee.
    - `ComponentKind` is gone. `ZoneNode::component` and `OrbitZone::component_kind` use plan 11's
      `SlotKind`.
    - For tests only, `stellar::multiplicity::hand_built` (a `cfg(test)` module appended to
      `hierarchy.rs`) builds a `SystemHierarchy` from a tree, with orbits taken from their
      semi-major axes. Plan 11's test fixtures `galaxy`, `sunlike`, `imf_records` and `SAMPLE`
      are now `pub(crate)`.
  - _Ruling 52.4._ The root pair's zone is `OrbitHost::Barycentre`, with host number 16 + k as
    before. Pairs below the root stay `Pair(k)`. Every multiple has exactly one barycentre zone:
    the last, with no outer limit, around every component, and never flagged as a close binary.
    In the `planetary/zones` golden, 59 of 382 lines change from `pair k` to `barycentre`, and
    every value, host number, member list and flag is unchanged (checked line by line).
    `golden_diff.py` counts the relabelled lines as moved values.
  - _D10's flag into the class draw._ `OrbitZone::host_multiplicity() -> HostMultiplicity`
    replaces `in_close_binary()`. The zone also gives `zone_limit() -> ZoneLimit` and
    `class_constraints(&Disc) -> ClassConstraints`. `CLOSE_BINARY_SEMI_MAJOR_AXIS` is defined as
    `CLOSE_BINARY_CUTOFF_AU`, so 47 au is written in one place. The chain that T8 and T30.a use,
    per zone, is:
    - `ZoneDiscInputs::for_zone(..).derive()` for the disc;
    - `draw_class(seed, system, zone.host_number(), &class_weights(zone.host_mass(), fe_h), &zone.class_constraints(&disc))`
      for the class;
    - `zone.inner()` and `zone.outer()` for T8's limits.
  - _T9's tests on drawn hierarchies._ The sample is 10⁴ multiples drawn by plan 11
    (`ForcedMultiple`, over the mass function at the Sun-like point).
    - The zones: 2,094 of the multiples have three or more stars, with 22,872 stars in all and
      34,131 zones (after ruling 53, below). No two zones overlap at the worst phases. At eight
      times per system, measured against plan 11's own `star_positions_at`, the least clearance
      between a zone's limit and a star on either side of it was a factor of 2.07: a star outside
      a zone that is not its own, or a pair's own star inside the pair's inner limit. The width
      cut dropped 1,613 inner pairs' zones.
    - The flag, on a second 10⁴ sample: 18,411 zones are flagged and 15,798 are not. Each is
      flagged exactly when a pair above its host is under 47 au. Every flagged zone keeps 0.34 of
      each planet-bearing class's weight, bit for bit. Planet-bearing classes were drawn in 4,524
      flagged zones against 4,591 expected (p above α = 10⁻³). Every zone's disc lies inside its
      zone.
    - The brown-dwarf `StarSlot`: a binary and a triple built by hand give the zones of their
      star-slot twins, differing only in the kind, and match the `ZoneNode` build bit for bit.
    - The occurrence ratio of (c) waits for T8's placer.
  - _A finding against the first pass. Ruled (ruling 53): a component is bounded at its closest
    approach to each outer companion._ The first pass bounded a component's zone below the root by
    its pair's zone less its own swing. That bound does not bind "only in hierarchies too tight to
    be stable". Of the 5,250 stars below the root pair in the sample, it left 78 without a zone and
    cut 34 more short, all in hierarchies that pass Mardling and Aarseth's test.
    - An example: a 0.107 M☉ star at 11.4 au (e = 0.53) from a 0.659 M☉ primary, with a
      0.639 M☉ third star at 53.3 au (e = 0.25, pericentre 40 au). The pair's S-type zone
      against the third star is 10.9 au about its barycentre, and the light star swings 15.0 au
      from it, so it lost its own 0.73 au zone, although its Hill sphere against the third star
      is about 9.5 au even at the closest approach.
    - The triple passes Mardling and Aarseth only through their inclination factor (108°, R_p ÷
      a_in = 3.52 against 4.13 coplanar), as do the hierarchies of 61 of the 78 zoneless stars
      and 28 of the 34 cut ones. Holman and Wiegert's fits are for coplanar, prograde orbits and
      apply to inclined hierarchies loosely at best.
    - _As built after the ruling._ A component's outer limit is the least, over every pair L
      above it, of `a_L × holman_wiegert_s_type(μ, e_L) × (1 − ρ ÷ (a_L (1 − e_L)))`, with
      μ = `m_C ÷ (m + m_C)` for the component's mass m and the mass `m_C` of L's other member
      (taken at its barycentre), and ρ the component's greatest distance from the barycentre of
      its own member of L. That is the S-type limit of a binary of L's eccentricity whose
      pericentre is their closest approach. At the component's own pair ρ = 0 and it is the
      plain S-type limit. A pair's circumbinary zone is bounded as before.
    - _Outcomes._ On the same sample, all 78 zoneless and all 34 cut stars now keep their own
      S-type limit exactly: the closest-approach bound binds for none of the 5,250 stars below
      the root pair, and every star has a zone. The zones stay disjoint at the worst phases, and
      the least clearance against plan 11's star positions is still a factor of 2.07. There are
      34,131 zones, up from 34,053 under the first pass's bound.
    - _Disjointness_ now holds for stable hierarchies by test, not for every hierarchy by
      construction. The random-hierarchy overlap test checks it only for the 1,688 of its 3,000
      hierarchies that pass Mardling and Aarseth (coplanar, prograde). A hand-built unstable
      triple where the new bound binds is tested against the formula.
    - _Goldens._ `planetary/zones` does not move under the ruling: no zone of its hierarchies is
      set by either bound below the root. Against HEAD it changes only in ruling 52.4's labels,
      which `golden_diff.py` reports as changed values (a false positive of its label matching).
- **Deviations in T7, as built (`masses`, round 7).**
  - _Shape._ `placement/masses.rs`. `MassDraws::for_planet(seed, system, planet)` reads words 8s
    to 8s + 7 of `planet.mass` (`System`, the one line `tags.golden` gains) for the planet in slot
    s: its own scatter εᵢ at words 0–1, its group's between-system normal at 2–3 (read at the
    group's first member, so that every draw number is a slot and none a group number), a law's
    rank at 4. T8 calls `group_masses(seed, system, group, disc, members)`, with the template's
    `&PlanetGroup`, the host's `&DiscProfile` and the members' `BodyIndex`es inside out for the
    count it drew, and gets a `GroupMasses`
    (`masses()`, `characteristic()`, `cap()`: `GroupCap::{AsDrawn, SolidBudget, GasMass}`, and
    `total()`); `group_masses_from` is its pure core, with `characteristic_mass`, `reference_mass`,
    `solid_budget` and `gas_budget`. The 0.1 dex step is centred on the group,
    0.1 (i − (n − 1) ÷ 2), so that m_c stays the group's typical mass at any count (for the
    orchestrator). Members are held to the template's range, then the group to its limit, which
    outranks the range's floor.
  - _The solid budget (ruling 38, point 4)._ The disc's whole solid mass between its edges, already
    cut to the zone, at an efficiency of 1 (`SOLID_BUDGET_EFFICIENCY`), per group, so that changing
    one group moves no other. Sources: Chiang and Laughlin's (2013, eq. 4) nebula holds 12.7 M⊕
    at 0.05–0.5 au, where the median solar disc has 0.23 M⊕ (its isolation mass is 2.3 × 10⁻⁴ M⊕
    at 0.1 au, not the ruling's 5 × 10⁻⁴); Mulders et al. (2021, ApJ 920, 66) find observed
    systems and Class II discs both peaking near 10 M⊕, "a discrepancy only when the planet
    formation efficiency is below 100%"; the pebble models' own efficiencies, 50% (Lambrechts and
    Johansen, 2014) and 15–20% into one core, would cap half of Sun-like chains, since the plan's
    4 M⊕ × 3.5 is 43% of the median disc's 32.2 M⊕. Measured: the budget holds 9.9% of solar
    chains, 47% at 0.5 M☉ and 59% at 0.3 M☉, where arch's count rises to 6.1 as the solids fall;
    the median chain planet is then 3.75, 1.69 and 1.02 M⊕, 6.5%, 29% and 44% of them under
    1 M⊕. **For the orchestrator:** that thins T10.b's count of 1–4 R⊕ planets per M dwarf
    (ruling 48, e).
  - _σ_b and σ_w (for the orchestrator to rule)._ The disc's 0.5 dex gas scatter already enters
    m_c at the power 1, more between-system scatter than the correlation needs, so σ_b = 0 (the
    normal is drawn, so a later σ_b moves no draw) and σ_w = 0.2 dex. Over 2,000 solar-disc chains
    (5,001 pairs) the adjacent log radii after T11 correlate at 0.66, the log masses at 0.87, and
    the outer planet is the more massive in 0.58 and the larger in 0.58 of pairs. Weiss et al.'s
    65% is out of reach with the plan's 0.1 dex step once Chen and Kipping's independent scatter
    (0.146 dex) enters, and σ_w ≈ 0.13 for 65% in mass would put the radius correlation near 0.71.
    So test (b)'s mass window 0.5–0.8, which assumed radii tracking masses, is corrected to
    0.80–0.90, and the calibration's radius correlation is asserted at 0.60–0.70.
  - _Re-checked._ Weiss et al. (2018): r = 0.65 (§3; Fig. 2's caption 0.62; 0.53 above 1 R⊕),
    outer larger in 65.4 ± 0.4% (§5.3, in radius, tied to photo-evaporation). The 4 M⊕ median:
    Pascucci et al. (2018) find the typical mass inside the snow line linear in the host's mass,
    and their G-star law's median is 5.1 M⊕ at 1 M☉ extended to the smallest masses, 6.5 M⊕
    inside their fitted range; Mulders et al.'s de-biased median is about 5 M⊕. The plan's 4 M⊕,
    20–40% under, at the edge of their errors, is kept (for the owner). The rocky groups' 0.5 M⊕
    (`ROCKY_CHARACTERISTIC_MASS`: the Solar System's mean; Kokubo and Genda 2010) is this lane's
    choice, where the plan gives none, and the dial of ruling 48 (f)'s η⊕.
  - _T7.c._ Giants take Cumming et al.'s law by inversion (KS against it) and are held together to
    the disc's gas mass. "Within the disc lifetime" is Lambrechts and Johansen's (2014, eq. 35)
    pebble growth at the snow line, seeded with the local isolation mass there (ruling 38's use for
    it): the median solar disc's core reaches 10 M⊕ by 0.32 Myr (0.50 Myr at 5 au, their "about
    1 Myr"). As a 10 M⊕ gate the isolation mass alone would forbid almost every giant (0.20 M⊕ at
    5 au, under 1 M⊕ anywhere). `giant_core(&Disc, ZoneLimit)` returns a `GiantCore`: `NoDisc`,
    `TooFewSolids`, `TooSlow { by }` or `Forms { by }`. Of discs living their star's lifetime, solids
    alone admit 83% at 1 M☉ and the core in time 71%; 49% and 29% at \[Fe/H\] = −0.5; 94% and 89%
    at +0.3; 66% and 64% at 0.5 M☉; 89% and 62% at 1.5 M☉. **For the orchestrator to rule:** it
    is not wired into `ClassConstraints` (arch's file), which would move `planetary/architecture`
    and ruling 48 (c)'s anchors (`GIANT_WEIGHT_SCALE` re-solved) and steepen the giants' fall
    with metallicity (eq. 35 goes as Z^(25/6)) against T10.b's 2.0 ± 0.3; T8 may apply it as a
    second fallback instead, or it may stay informative.
  - _Goldens._ New `planetary/masses` (draws, every template group's masses in seven discs, and
    their cores), blessed at 11; `tags.golden` gains `planet.mass`. Nothing else moved.
- **Deviations in T16.a and T34, as built (`record`, round 7).**
  - _T16.a, API._ `derive::derive_body(&PlacedBody, &BodyHosts, &DiscProfile, age: Years, t:
UniverseTime) -> Result<DerivedBody, DeriveBodyError>`. `PlacedBody::new(mass, orbit:
KeplerElements, formation_distance, radius_rank: UnitUniform)` stands in for T8's
    `PlacedPlanet`, which converts into it; its orbit is the primordial one, and `with_orbit_now`
    sets T28's orbit at `t`, which only the steps at the time read. The rank is a
    plain argument: `planet.radius` is not registered here, and T30 opens it (for the
    orchestrator). `BodyHosts::new(primary_mass: Kilograms, orbited: &[HostLight], companions:
&[Illumination])` takes each host as plain L, T_eff and R (ruling 34.2), and the mass the
    body orbits at `t` for T15. The order kept: T11 (fixed at formation), T12 (T13 will iterate
    there), the radius at the time (T13.b will strip there), T14's place, T15. Bond albedo 0.3.
    `DerivedBody` holds the mass, the confined rank, the side of the snow line, the
    `SolvedComposition`, the flux, T_eq, the radius at `age + t`, density, surface gravity
    (`units::MetresPerSecondSquared`, new), `PlanetClass`, the circular Hill radius, both
    satellite limits (e_s = 0) and `maximum_surviving_moon_mass` at `age + t`, with
    `roche_limit_fluid` and `roche_limit_rigid` for a satellite density. `NotYetFormed` for a
    system age at `t` that is not positive. Giants (T11.d, wired at the merge, ruling 56): from
    0.3 M_J, where `giant_share` rises from zero, `giant_cooling(m, age + t, composition)` and
    `radius_giant(m, &interior, flux)` give the radius, blended into the envelope model's
    radius and `giant_composition`'s fractions into the solve's up to 0.414 M_J and alone above
    it, where `composition()` is `None`; the giant's internal luminosity enters T_eq through
    `with_internal_heat`. `BodyHosts::new` gains the system's `Composition`, which the cooling
    reads. Above 13 M_J, `DeriveBodyError::Giant` or `GiantCooling`. Saturn, at 0.2994 M_J, stays
    below the blend.
  - _T16.a, the formation flux (for the orchestrator)._ Composition is fixed at formation (ruling
    47.2), so the solve reads the flux of the disc host's zero-age luminosity at the body's
    primordial orbit, never at the orbit at `t` (tested with a widened orbit),
    companions left out as D6 leaves them out of the snow line; `DiscProfile` gains
    `host_luminosity()`. The envelope's radius at `t` reads the present flux and `age + t`.
  - _T16.a, ruling 47.1._ `composition::radius_window(mass, side, flux) -> RadiusWindow` gives the
    radii the solve keeps, with the solve's own expressions (`largest_envelope` is shared with
    `enveloped`; no output moved): from the iron curve to the envelope limit where a body may take
    an envelope, or its dry curve when that is larger, and to the capped rock or ice curve for
    cores under 1.5 M⊕, whose clamps are the same kind of boundary as the envelope limit the
    ruling names. `radius::chen_kipping_rank(mass, radius)` is Chen and Kipping's CDF, and
    `radius_rank_in_window(rank, &window)` rescales the rank (the window carries its mass). Over
    999 ranks at 10 F⊕, before → after: 1 M⊕
    inside, 1.4% raised to iron and 26.8% clamped to rock → 0 and 0; 2.1 M⊕, 25.4% iron → 0; 3
    M⊕, 15.3% → 0; 5 M⊕, 6.0% → 0; 100 M⊕, 56.4% at the envelope limit → 0; 131 M⊕, 71.6% → 0;
    beyond the snow line 0.8–1.7% clamped to ice → 0. After it no rank of 13 masses (10⁻⁴–131
    M⊕), both sides and three fluxes lands on a boundary. Where the upper edge is over 8.3 σ above
    the median (1.6 M⊕ inside at 10⁻³ F⊕) Φ rounds to 1 and the rank is held below 1, still
    inside; a rank within 10⁻¹² of 0 or 1 can meet an edge by rounding.
  - _T16.a, what the mapping did to iron._ Inside the snow line below 1.5 M⊕ the 27% once clamped
    to rock were spread over the window, so the share over half iron rose from 30% to 40% and the
    median core mass fraction of dry bodies from 0.29 to 0.42. Ruled (ruling 53, amending 47.1):
    rocky compositions come from the observed spread, below.
  - _T16.a, rocky outcomes (ruling 53)._ `derive/rocky.rs` holds Plotnykov and Valencia's (2020,
    MNRAS 499, 932, abstract; re-checked on arXiv:2010.06480) population of 33 rocky exoplanets'
    core mass fractions, 0.24 +0.33 −0.18, read as the median and the 16th and 84th percentiles.
    The paper's distribution is a kernel density estimate with no functional form, so it is built
    as a two-piece logit-normal through those three points (scales 1.599 below and 1.435 above
    in logit): inside (0, 1), continuous, and exact at the three; 21% of it is over half iron,
    and Earth sits at its 62nd percentile. `rocky_core_mass_fraction(rank, &window)`: a confined
    rank between the iron and rock curves' is at a share s of the rocky part, and its core mass
    fraction is the distribution's at 1 − s, with the radius Zeng's at it (`dry_composition`);
    above the rock curve the rank keeps Chen and Kipping's radius and the solve. One quantile per
    body still. `RadiusWindow` gains `mass()` and `rock()`, and for a core under 1.5 M⊕ inside the
    snow line now ends at the rock curve, below the 0.1% water sliver the solve keeps, so that the
    window is continuous in mass at the floor (it stepped the radius by 8 × 10⁻⁵ there).
    Measured over 999 ranks of nine masses of 0.1–1.9 M⊕ at 1 F⊕, inside the snow line, as built
    (47.1) → now: 16th percentile 0.145 → 0.060, median 0.410 → 0.240, 84th 0.681 → 0.569, over
    half iron 38.7% → 21.1% (2.1 M⊕: median 0.563 → 0.239; 5 M⊕: 0.489 → 0.241). The radius is
    continuous in the rank across the rock curve (to the envelope model's 1.1 × 10⁻⁵ where an
    envelope begins) and in mass everywhere but at Chen and Kipping's transitions, where their
    radii already step (T11.a); tested.
  - _T16.a, rocky outcomes beyond the snow line (for the orchestrator)._ The ruling's "between the
    iron and rock curves" is applied on both sides. Beyond the snow line the solve reads a radius
    between the Earth-like and rock curves as water, up to about 12% at 1 M⊕, on an Earth-like
    core; those ranks are now dry rock of the observed spread instead. Of the bodies of 0.1–1.9 M⊕
    beyond it, 2,596 of 8,991 are watery against 5,071 before, and the dry ones' median core
    fraction is 0.24 instead of 0.56. The alternative is to remap only the dry part below the
    Earth-like curve there, which keeps every water world but leaves dry bodies beyond the snow
    line at 0.325 and over.
  - _For the owner: design note 8's reading._ The brainstorm's "radius from mass (Chen and Kipping
    2017), refined by composition with Zeng et al." is read, for rocky bodies, as composition
    refining the radius: the one drawn quantile picks the composition from the observed spread,
    and the radius follows from Zeng's curves, since Chen and Kipping's 0.040 dex at low mass
    mixes measurement error and sub-Neptunes into a spread wider than all of iron to rock. For
    everything above the rock curve the quantile still places the body within Chen and Kipping's
    scatter and the composition is solved from that radius. Radius, composition and envelope are
    never drawn apart (ruling 53).
  - _T16.a, classes and tides (for the orchestrator)._ `PlanetClass { Rocky, Icy, SubNeptune,
IceGiant, GasGiant }`, in `derive`, with its thresholds in `params.rs`: an envelope from 0.1% of
    the mass
    (`THIN_ENVELOPE_FRACTION`; Venus's air is 10⁻⁴) makes a sub-Neptune, an ice giant from 10 M⊕
    (`ICE_GIANT_MASS`, the critical core mass), a gas giant from half the mass
    (`GAS_GIANT_ENVELOPE_FRACTION`); without one, water from 10% (`ICY_WATER_FRACTION`) is icy.
    `has_surface()` is false for the two giants only, so a sub-Neptune's surface is T13.c's "gas
    envelope" state. T15's moon limit takes T14.b's k₂ and Q (0.3 and 100 for classes with a
    surface, 0.4 and 10⁵ for giants), also in `params.rs`.
  - _T16.a, the Solar System_, each planet at the rank that keeps its radius, about the present
    Sun (1 L☉) in the zero-age Sun's disc (snow line 2.26 au) at 4.57 Gyr: core mass fractions
    Mercury 0.708, Venus 0.288, Earth 0.323, Mars 0.216; envelopes Saturn 71.4% (9.145 R⊕ against
    9.140), Uranus 8.2% (3.992 against 3.981), Neptune 6.4% (3.874 against 3.865); classes Rocky,
    GasGiant and IceGiant; T_eq at A = 0.3 Earth 254.6 K, Mars 206.5 K, Venus 299.3 K, each T12's
    to the bit (T12's figures at each planet's own albedo, Venus 229 K, Mars 210 K, Jupiter 110
    K, stay T12's tests, since the slice fixes A = 0.3); Earth 9.82 m s⁻² and 5,513 kg m⁻³;
    Earth's Hill radius 1.497 × 10⁹ m; the Moon, Phobos, Deimos, Titan, Iapetus, Phoebe
    (retrograde) and Triton (retrograde) inside their limits; Saturn's fluid Roche limit for 600
    kg m⁻³ within 2.5–2.7 of its radius and outside the A ring's edge at 136,775 km; an Earth at
    0.05 au
    keeps no moon over 10⁻⁶ M⊕ for 5 Gyr. Jupiter, through T11.d: 11.22 R⊕ (1.001 R_J of 71,492
    km; its volumetric mean is 10.97), 57.9 M⊕ of heavy elements (an envelope of 81.8%), T_eq
    128.9 K with its internal luminosity of 4.4 × 10¹⁷ W (111.7 K from sunlight alone at
    A = 0.3; measured, about 3.3 × 10¹⁷ W and an effective temperature of 124 K). New golden
    `planetary/derive_body` (the eight planets and five synthetic bodies, two of them giants) at
    11; the `giants` lane's `planetary/derive_giants` is unchanged.
  - _T34, API._ `record::{DetailLevel, Section<T>, SectionState, RecordSection, BodyRecord,
BodyRecordBuilder, BodyIdentity, BodyLabel, BodyOrbit, BulkProperties, Surface, Hooks,
BodyKind, MoonOrigin, BeltKind, SystemSnapshot, SystemSection}` and their build errors;
    `fate::{BodyState, DestructionCause}` (the states only; T28's transform produces them).
    `BodyRecord::builder(identity)` starts every section `NotModelled`, `.derived(&DerivedBody)`
    sets the slice's mass, bulk and surface tags, and `build()` rejects `NotResolved` and
    `BodyKind::Unresolved`, so `degrade` is their only producer. `degrade(level)` keeps the ID,
    parent, state and position, withholds the kind and label at `Contact`, and turns every section
    above the level `NotResolved`; `degrade(a).degrade(b) == degrade(min(a, b))`.
    `SystemSnapshot::new(system, time, bodies)` takes full records in index order, with the belts
    and halo `NotModelled`; `with_populations(belts, halo)`, T21's, checks that each index names a
    body of its kind; and `degrade` drops belt members' records below `Bulk`.
  - _T34, shape (for the orchestrator; T35 mirrors it)._ (1) The mass is a section of its own at
    `MassAndOrbit`, not the bulk section's, since that level shows it and a rogue planet or a
    belt has a mass without an orbit. (2) `BodyKind::Planet` carries no class: D16 puts the class
    at `Bulk`, and the kind shows from `MassAndOrbit`, so the class is `BulkProperties::class`.
    (3) The label is a `Section<BodyLabel>`, `NotModelled` until T30.c and `NotResolved` at
    `Contact`. (4) `parent` is `Option<OrbitHost>` (ruling 53), T9's type: `Star(n)` for a planet,
    `Pair(n)` for a circumbinary one, `Body(i)` for a moon, ring or member, `None` only for a
    system's root host, a free-floating object; T35's `BodyOrbitDto.parent` is an
    `OrbitHostDto`.
    (5) Belts, the halo and belt members are bodies (D3), so the snapshot's `belts` is
    `Section<Vec<BodyIndex>>` and `halo` `Section<Option<BodyIndex>>` (`Ok(None)`: no halo), and
    T21 puts a belt's population in its own record. (6) `Surface` and `Hooks` have no variants
    until T13, T14, T24 and T23–T26 give them contents, so no record can claim either. (7) Moons,
    rings, belts and halo belong to `MassAndOrbit`. (8) The JSON test of a `MassAndOrbit` record
    moves to T35; here its equivalent asserts that the bulk, surface and hooks sections are
    withheld. (9) `BodyOrbit::valid_until` is `None` until T28. (10) The mass fractions sit in the
    bulk section at `Bulk`, as the brief asks, where D16 lists "composition" among `Full`'s
    hooks; T23's `BulkComposition`, with the volatile inventory and the host's abundances, stays
    at `Full`. (11) `BodyLabel` is a checked `String` in `record.rs`; T30.c may move it to
    `label.rs` with a re-export. (12) `degrade` takes `&self`, as Provides has it, so it clones
    what it keeps.
  - _T16.b._ `tests/planetary_rocky_properties.rs` (moved from `planetary_properties.rs` at the merge, which `place`'s T16.b tests took) holds the core-fraction test (ruling 53):
    `rocky_core_mass_fractions_follow_plotnykov_and_valencia`, over 35,964 hand-placed bodies of
    0.1–1.9 M⊕ at 0.1–1.5 au of the present Sun (T8's placer is not built), whose rocky outcomes'
    16th, 50th and 84th percentiles are within 0.01 of 0.06, 0.24 and 0.57, with 21% ± 1% over
    half iron and a largest gap to the source's cumulative distribution under 0.01. "No planet
    hotter" and the continuity in time wait for `StarModel` (P06.T29.a), which has not merged.
- **Deviations in T11.d, as built (`giants`, round 7).**
  - _Files and API._ `planetary/derive/{radius, composition}.rs`; `params.rs` gains
    `GIANT_RADIUS_CAP` (2 R_J), `GIANT_INFLATION_ONSET` (1,000 K) and
    `GIANT_INFLATION_FADE_START` (500 K). `radius_giant(EarthMasses, &CoolingState, EarthFluxes)
-> Result<GiantRadius, DeriveGiantError>` takes plan 13's `giant_cooling` at the body's age
    (ruling 34) and the total orbit-averaged flux, with `radius()`, `cooling_radius()`,
    `inflated_radius()`, `is_inflated()`, `heating_efficiency()`, `share()`,
    `internal_luminosity()` and `blended(EarthRadii)`; `giant_share(EarthMasses)` is the blend's
    weight and `heating_efficiency(EarthFluxes)` Thorngren and Fortney's eq. 34.
    `giant_composition(EarthMasses, SnowLineSide) -> Result<GiantComposition, DeriveGiantError>`
    has `fractions()`, `core()`, `heavy_elements()` and `blended(MassFractions)`. `composition`
    still refuses 0.414 M_J and above with `GiantPlanet`, the seam at which `derive_body` takes
    these; from 0.3 M_J (`giant_share` above zero) it blends them into the solve's. New golden
    `planetary/derive_giants`, blessed at 11; no existing golden moved.
  - _The fit, re-checked on the paper_ (arXiv:1709.04539v2, eq. 34): ε = 2.37 (+1.3 −0.26)% ×
    exp[−(log₁₀ F − 0.14 (+0.060 −0.069))² ÷ (2 × 0.37 (+0.038 −0.059)²)], F in 10⁹ erg s⁻¹
    cm⁻², with T_eq = (F ÷ 4σ)^¼. It peaks at 1,570 K and is 0.22% at 2,500 K (their abstract's
    0.2%). At equilibrium the interior radiates ε π R² F (their eq. 35, T_int = ε^¼ T_eq), which
    is the internal heat given to T12.
  - _The inflated radius is their models', not the cooling fit's._ The direct reading, the
    cooling track's own state at ε^¼ T_eq, was built first. It puts HD 209458 b at 1.47 R_J
    against 1.359. Since εF peaks at 1,880 K, it shrinks planets beyond that as the flux rises.
    Against 650 transiting giants of 0.5–10 M_J (NASA Exoplanet Archive) its medians are +13,
    +11, +6.5, −5, −11 and −23% in the bands 1,000–1,250, 1,250–1,500, 1,500–1,750,
    1,750–2,000, 2,000–2,500 and beyond 2,500 K. The fit is isolated and coreless, and their
    models have irradiated atmospheres and heavy elements. As built, a giant takes the larger of
    its cooling radius and their Fig. 2 model radius at its mass and T_eq (5 Gyr, mean
    composition). The lines at 500, 1,000, 1,250, 1,500 and 2,000 K were read from the figure's
    vector paths on their own 50-mass log grid, and 34 masses from 0.299 to 12 M_J are kept. Where
    the 2,000 K line leaves the plot, below 0.586 M_J, the next value down is recovered from where
    its segment is cut and the lighter ones carry that segment on. The lines are interpolated
    linearly in T_eq and carried on beyond 2,000 K, an extrapolation: εF falls beyond 1,880 K, but
    their Gaussian model's radius in their Fig. 12 still rises to about 2,500 K in every mass bin, and at
    2,500 K the carried-on line is 0.2% above it at 0.9–1.2 M_J and 6–8% above it at 1.2–10 M_J.
    The figure uses their Gaussian-process ε, which their DIC does not tell from eq. 34's. The
    medians are then −1.0 to
    +5.9% in every band below 2,500 K and +9.9% beyond (11 planets). HD 209458 b comes out at
    1.336 R_J (−1.7%) and HD 189733 b at 1.126 (−1.1%), against 0.984 and 0.997 from cooling
    alone.
  - _The threshold._ Below 1,000 K the model radius fades linearly to nothing at 500 K. Their
    5 Gyr radii there belong to planets still cooling, and holding one at every later age would
    keep old giants up, by up to 4.3% at 13 M_J and 13.8 Gyr. No giant cooler than 900 K is held
    at any age to 13.8 Gyr.
  - _The blend and the cap._ Between the giant's radius and the body's radius without T11.d
    (Chen and Kipping's through the solve and the envelope model at its age), ln R is linear in
    log mass, weighted by `giant_share`: 0 at 0.3 M_J and 1 at 0.414 M_J. The internal
    luminosity takes the same weight and the fractions blend linearly. Both ends are continuous
    to 10⁻⁶ over ages, fluxes and quantiles (the plan asks 5%). Their models inflate planets below
    0.5 M_J, outside their sample, far beyond what is observed (none has a surface gravity under
    about 3 m s⁻², their §2). Below 0.414 M_J the blend holds these back; from 0.414 to 0.5 M_J
    only the 2 R_J cap does, and a 0.42 M_J giant at the cap has 2.6 m s⁻².
  - _Composition._ The heavy elements follow Thorngren et al.'s (2016) mean relation,
    M_z = 57.9 M⊕ (M ÷ M_J)^0.61. They take the solve's core on the body's side of the snow line
    (53.9% water on Earth-like rock beyond it, Earth-like inside), and the rest is envelope.
    Jupiter comes out 82% envelope (57.9 M⊕, against their model's 37), 0.3 M_J 71% and 13 M_J
    93%. Their 1.82× scatter is not drawn: the fit has no heavy elements for a draw to move the
    radius by, and design note 8 keeps radius and composition from being drawn apart. The radius
    quantile is unused above 0.414 M_J.
  - _Test (d), bracket by bracket._
    - Jupiter rests on the cooling fit: 71,478 km, within 0.02% of 71,492.
    - Uranus rests on Chen and Kipping: its median is 1.8% small.
    - Neptune's median is 4.31 R⊕, 11.5% large, with Neptune 0.32σ below it. So the plan's 10%
      at the median does not hold, and the test asserts the paper's median and Neptune within
      1σ, as T11.a did for Jupiter.
    - Saturn (0.2994 M_J) has no giant share. Its median is 29% large (−0.77σ) and the coreless
      fit 12%. From Thorngren et al.'s 27 M⊕ of heavy elements, the envelope model gives +0.2%.
  - _For the orchestrator to rule._
    1. Their model radii, digitised from a figure, over the direct reading.
    2. The 500–1,000 K fade.
    3. Neptune's and Saturn's brackets. By this lane's estimate, ruling 47.1's mapping moves the
       median body to +10.7% for Neptune and −5.5% for Saturn.
    4. Giant radii carry no compositional scatter, which is what they credit with the observed
       one.
    5. The seam stays at 0.414 M_J, and `derive_body` calls T11.d from 0.3.
    6. The mass-only relation, rather than their Z_p ÷ Z★ = 9.7 (M ÷ M_J)^−0.45 with the host's
       metallicity.
    7. The 1,500–2,000 K line carried on beyond 2,000 K, rather than held or given a second
       slope from their Fig. 12.
    8. The 0.414–0.5 M_J giants that only the cap holds, below TF's sample and their 3 m s⁻²
       floor of observed gravities.
- **P14.T41–T42, T43.a–b for hosts and T44.a, as built (round 7c, `ui`).** Against `FakeWebSocket`
  fixtures copied from the protocol's wire-form pins (`test/systemFixtures.ts`); the server still
  answers `system_summary` with `unsupported`, which reads `REJECTED: …` as a fault with `RETRY`.
  - _Navigation (T41.a)._ `"system"` in `DisplayId` and `DISPLAYS` (`System`, `F3`) and its case in
    `App`. `OPEN SYSTEM` follows `SystemReadout` in `SystemsPanel.tsx`, outside its `role="status"`,
    held back with `NO SYSTEM SELECTED` while nothing is selected. It carries a `SystemTarget`: the ID
    and `universeTimeFromYears` of the chart answer's time, and also the designation, the galactic
    position and the census's `LayerBand`s, since `system_summary` carries none of them and the
    display needs them for body designations, ruling 34.5's `centre` and ruling 36's size classes.
    `App` keeps a `SystemOpening { target, sequence }`, and each opening is a new `key`, so opening a
    system again starts it afresh; another universe opened reads `NO SYSTEM SELECTED`.
    `GalaxyDisplay` takes a stable `onOpenSystem`, so its three tests pass a no-op.
  - _Requests._ `useSystemSummary(target, time, generation)` over `useServerRequest` keeps the last
    answer for its system through a newer request, converted once by `lib/system/wire.ts`'s
    `toSystemModel`, which reports an answer it cannot place as a fault in words
    (`SYSTEM DATA INVALID: hierarchy malformed`, with `RETRY`) rather than throwing in a render.
    `useSystemData(target, displayTime, generation)` holds D18's request time and moves it to the
    display time once that is more than a Julian year away, decided on whole seconds and then
    nanoseconds, or past a star's `death_time` or, unborn, the birth its age gives
    (`requestTime.ts`); `useSystemBodies` joins it there with `valid_until`.
  - _Data states (T41.b)._ The request's states through `RequestStatus`, in the map's head once there
    is an answer, so the map does not move; `NOT YET FORMED`; `NO BODIES` when nothing is drawable
    (a star with no remnant); stale with the `S` on link loss, and when the newer request a step
    asked for is refused or times out, which leaves an answer for a time the display has left.
    `systemNote(BodiesKnown)` composes the
    note from `not_modelled` tags (`MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED` for the
    slice's, `null` for `ok`). **For the orchestrator to rule:** with no bodies kind in this
    protocol there are no tags, so the display passes `{ kind: "unserved" }` and the note reads
    `PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED`, the client's statement that
    it cannot ask for bodies, lest the space round the stars read as empty; the UX review notes that
    rulings 55–56 have the sim computing planets, so "not computed by this generator version" is
    arguable, and the alternatives are to drop `PLANETS` from the note or to give "not served" a
    phrase of its own. `DETAIL:` waits for `system_bodies`, which carries the granted level.
  - _Orbit map (T42 for hosts)._ Scene unit the AU. `composePosition` gains a `member` placement
    (`share` times the pair's relative orbit: −M₂ ÷ M inside, +M₁ ÷ M outside), and
    `lib/system/hierarchy.ts` lays out `HierarchyDto`: placements, each pair's members, each star's
    reach, the orbit each star keys and the plane. Each star's path about its pair's barycentre, and
    an inner pair's barycentre's, is a `reference` path about where the barycentre is now; the
    selected star's is `selected`, 2 px (`drawList`'s per-role width, ruling 44.2). The frame reads
    `SYSTEM BARYCENTRIC`, the centre the system's `RADIUS`, `ANGLE`, `HEIGHT`, `coreDistance` its
    `RADIUS`, `axes` its `localFrameAt`. `ORBIT_SCALE_UNITS` puts the bar on `formatBodyDistance`'s
    bands (AU from 0.1 AU, Gm, Mm, km); a 1-2-5 bar steps, so it keeps no unit from before.
    `SpatialView` gains an optional `fitRequest`, which `INNER`/`ALL` (`I`, `A`) bump so a preset
    fits after a zoom; absent on the chart. The legend: `BODIES NOT TO SCALE` alone (ruling 36), each
    kind drawn with its symbol, the census's `INIT MASS` sizes, the fill words, orbit and selected
    orbit samples, the bracket. **For the orchestrator to rule:** (1) the plane, until planets, is
    the innermost pair holding the primary's orbit, normal along its angular momentum, for a wide
    binary too; a single star is drawn on the galactic plane, named `GALACTIC PLANE`; (2) D21's
    "frame label says `SYSTEM PLANE`" is met by the legend's `FILLED ABOVE SYSTEM PLANE` /
    `OPEN BELOW SYSTEM PLANE`, the `FRAME` reading keeping the draft's name alone; (3) `INNER` fits the
    nearest non-zero reach (the sum of each orbit's share of its apoapsis up the chain), `ALL` the
    farthest, a lone star 1 AU (`LONE_STAR_FIT_AU`); (4) orbits are unlabelled, their stars are.
    Picking is plan 05's; a click on an orbit selects nothing (tested). A zoom preset stays pressed
    through a manual zoom, since the grid still covers its radius and `Z` returns to it (the camera
    presets release; **for the orchestrator to rule**). A zoom key repeating in the frame a preset is
    pressed applies against the old fit, a known edge left as it is so that the chart's camera is
    untouched.
  - _Bodies (T43.a–b for hosts)._ `BodyList` is a `tree` of `treeitem`s (`aria-level`, set position
    and size per parent): arrows move the active row, `Enter`/`Space` select, a click selects, and
    `1-2 of 2` shows position and total. Rows read designation (`<system> /<index>`), kind in words,
    and for a companion the SMA of the orbit it keys; the primary's cell is empty (**for the
    orchestrator to rule**). The primary is selected until another is chosen. `BodyReadout` (a
    `div role="status"`, ruling 14) reads DESIG, ID, KIND, PHASE, CLASS, INIT MASS, MASS, LUM, RADIUS
    (km for neutron stars and black holes), T EFF, REMNANT, COOLING AGE, PULSAR PERIOD, SPIN, KICK,
    ROTATION, ACTIVITY, VARIABILITY, with the em dash where not modelled and `NONE` where modelled as
    none; no light reads `—` with `NO LIGHT` for LUM and T EFF, and a star with no remnant omits its
    physical rows as not applicable. Binary class has no field and no row. The panel names the system
    with its `AGE` at the answer's time and `[Fe/H]` in dex. `LUM`, `T EFF` and `SMA` are the
    draft's proposed abbreviations.
  - _Time (T44.a)._ `useDisplayTime` holds the time and step (`1 h` … `100 yr`, keys `1`–`6`,
    default `1 d`); `[` / `]` step, `R` resets, all through plan 05's redraw scheduler, so steps before
    a frame land in one state update and one paint, and no frame is asked for after it (tested with
    fake frames). `stepTime` adds whole seconds and stops at ±H exactly; `CLOCK WINDOW LIMIT` stands
    in an always-present `output` and holds back the step towards the edge. The time reads
    `DISPLAY TIME UT +12 yr 183/14:08:33` in the panel, an `output` so that each step is announced
    once (ruling 16's precedent), and silently in the view's furniture.
  - _Also._ `formatMassMearth` keeps three significant figures in E notation (`1.57E-4`, ruling
    44.1). `GALAXY`'s draw lists are byte for byte unchanged: the round-7 probe's 1,152 chart scenes
    written to JSON before any edit and after give SHA-256 `9cf26949…` both times.
- **Deviations in T8 and T10, as built (`place`, round 7),** with ruling 55.3 and T16.b's
  property test.
  - _Shape._ `planetary/placement/classes.rs` with `classes/orbits.rs` (T8.d) and
    `classes/tides.rs` (T8.e). `place(seed, system, &PlacementHost, limits: Truncation, &Disc,
class, first_slot) -> HostPlacement`: `PlacementHost::new(number, DiscHost, HostPlane)` carries
    the host's zero-age parameters as the disc's host does (T9.c's `ZoneDiscInputs::host`), and
    the limits are the zone's `truncation()`. `HostPlacement` has `drawn_class()`, `class()`,
    `core()`, `planets()` in slot order and `next_slot()`, which the next host takes as its
    `first_slot`. `PlacedPlanet` has `index`, `group`, `role`, `mass`, `orbit` (a
    `KeplerElements` with μ = G (M★ + m)), `formation_distance`, `formed()`
    (`SnowLineSide`), `origin`, `hot`, `resonance`, `drawn_eccentricity` and `rescaled`; T30.a
    builds T16.a's `PlacedBody` from its mass, orbit and formation distance. The close-binary
    flag is not an argument: it enters the class draw (T9.c); the placer takes `HostPlane`
    instead, `Aligned` with a close pair's orbital plane for its circumbinary zone.
  - _Tags._ `planet.orbit` (`Body`) and `planet.plane` (`System`), as the brief asked, and
    `planet.count` (`System`) for the group-level draws: presence, count, first location, the hot
    variant, resonance, a flanking side, and per pair a resonance offset. **For the orchestrator
    to rule:** the brief named two tags; the counts need a system-level stream (design note 4) and
    Provides lists `planet.count`, so `tags.golden` gains three lines.
  - _Slots._ Every group's drawn count is reserved in consecutive slots before anything is placed,
    so that masses and spacings, keyed by slot, are fixed; a member that cannot be placed leaves
    its slot unused, and generation order is template order, a flanking companion after its giant.
  - _Where a group starts._ Its location law is drawn inside the range its bounds allow, the shape
    kept (a range wholly inside the inner bound is held at it), rather than clamped, and a small
    group's range stops at the chaotic-zone gap of a giant anchored beyond it. Later groups'
    first bodies are barriers to earlier walks: D7's floor with the barrier's eccentricity and the
    walker's largest, and the gap. Spacings are drawn against `spacing_floor` at the
    eccentricities assumed (the pair's drawn and the outer's largest), so few are rescaled (under
    10% of cold chain planets).
  - _Hot and warm Jupiters_ (for the orchestrator to rule). T8's test says every planet lies
    outside the disc's inner edge, but that edge sits near an 8-day corotation period (T3.b, over
    10 days in 35% of discs), and so held 30% of hot Jupiters beyond 10 days and the rest near 8.
    A migrated giant drawn by a period law is placed inside the disc's cavity, bounded by twice its
    Roche limit (Ford and Rasio 2006) and the zone; placed hot Jupiters around single Suns then
    meet the class's 0.82%.
  - _Roche limit._ Ford and Rasio's (2006, §1) a_R = 2.16 R_p (M★ ÷ M_p)^⅓, their own definition of
    the "twice the Roche limit" edge, with R_p Chen and Kipping's median radius at the mass, not
    T15's fluid 2.456. Every planet's periapsis stays outside twice it, and inside the zone.
  - _Resonance offsets_ (for the orchestrator to rule). Fabrycky et al. (2014, §4, eq. 11) find the
    excess at −0.2 < ζ₁ < −0.1, which is 0.28–0.56% wide of 4:3, 0.56–1.11% of 3:2 and
    1.67–3.33% of 2:1, not the plan's 0.5–2% for all; ζ is drawn uniform in 0.1–0.2 and 5:3 takes
    the same ζ (they find no second-order excess). A snap is kept only where admitted; otherwise
    the pair keeps its spacing.
  - _Eccentricity and inclination._ Laws are truncated at each planet's limit by drawing the rank
    in the truncated range, with a numerical cap of 0.99. Beta laws by Press et al.'s `invbetai`
    (fixed steps). σ_i is 1.5° for cold chains (Fabrycky et al.'s range 1.0–2.2°, best fit 1.8°),
    and otherwise half the Rayleigh scale with the law's mean square eccentricity. Re-checked: Xie
    et al. (2016) give means (multis ē = 0.04, σ ≈ 0.032) and Van Eylen et al. (2019) Rayleigh
    σ = 0.061 for multis and half-Gaussian 0.32 ± 0.06 for singles; the templates' 0.04 and 0.3
    stand inside those. Kipping's Betas are as the templates have them. Wisdom's 1.3 is derived
    from his eq. 56 (s ≃ 0.51 μ^(−2⁄7)), as Chiang et al. (2009) print it.
  - _T8.e._ τ = (4 ÷ 63) Q′ (m ÷ M★) (a ÷ R)⁵ ÷ n (Rasio et al. 1996, eq. 9; Jackson et al. 2008,
    eq. 1), with Q′ 10⁶ and 10² (Ogilvie 2014: Jupiter's 1.2 × 10⁶; Jackson et al.'s 10^6.5).
    Goldreich and Soter (1966) were not reachable. **Finding:** hot Jupiters within 5 days are
    circular to 0.01 at 5 Gyr only up to 1.5 Jupiter masses; 95% of those of 1.5–2, 85% of 2–4 and
    65% of heavier ones are, as massive eccentric hot Jupiters are observed (HAT-P-2 b, XO-3 b), so
    test (e) asserts it up to 1.5.
  - _Ruling 55.3._ `core_fallback` applies T7.c's `giant_core` after the class draw. Re-fitted by
    `arch`'s method, extended to both fallbacks, over the solar-disc sample with each disc living
    its star's drawn lifetime (83.4% have the solids, 71.1% grow the core in time):
    `GIANT_WEIGHT_SCALE` 1.373 → 1.701 (+24%) and `HOT_JUPITER_WEIGHT` 0.0103 → 0.0127 (+23%).
    After both: Cumming's window 10.50%, hot Jupiters 0.82%, a cold giant in 30.7% of compact
    systems, giants in all 19.5%, compact 35.7%; on the placed sample of single Suns 10.51%, 0.82%
    and 30.5%. T4.c's brackets still hold, some narrowly: the log-slope is 1.851 (1.85–2.0), the
    compact share at 1 M☉ 0.345 (0.27–0.35), the giants at 0.3 M☉ 0.046 (under 0.05).
    `planetary/architecture` moves (251 values), without a bump as ruled.
  - _T10._ `tests/planetary_placement.rs`, with a shared `tests/planetary_support/mod.rs` that builds
    systems as T30.a will (records at the Sun-like point, plan 11's hierarchies,
    `ZoneHierarchy::from`, `zams` values, each star's disc-lifetime rank); `SystemContext` does
    not exist. T10.a runs on 4,000 systems and, slow, on a million; it also checks T9.c's "no planet
    outside its zone". T10.b asserts on mass (1–20 M⊕ for 1–4 R⊕), the radius rank being
    `planet.radius`'s (T30). As built: small planets 0.70 per FGK star (0.57 by radius at drawn
    ranks), hot Jupiters 0.73%, Cumming's giants 10.4%, the giants' slope 2.01, 61% of 0.1–0.5 M☉
    hosts with two or more inside 200 days and 3.7% with a giant, at −2 0.003 of solar.
    **Findings, pinned as built, for the orchestrator:** η⊕ 0.09 against Bryson et al.'s 0.37–0.60
    (rocky groups end short of the zone; all of `Barren` in `TerrestrialOnly` gives about 0.16);
    adjacent log radii's correlation 0.78 on these hosts (0.65 on single Suns) and the outer the
    larger in 0.58; 0.98 small planets per M dwarf inside 200 days (0.86 by radius) against 2.5 ±
    0.2; 0.14 planets per M3–M5.5 dwarf and 4% compact multiples inside 10 days against 1.19 and
    0.44 (the disc's inner edge near 8 days); small planets at −0.8 0.45 of solar (T7's masses
    scale with the solids); close-binary hosts' planets 0.23 of single stars' (inside Kraus's 1σ,
    under the plan's quarter); no small pair under 10 mutual Hill radii (Weiss 7%), median 17.2.
  - _T16.b._ `tests/planetary_properties.rs`, built against this tree's derivation pieces, not
    `derive_body` (not merged here): T12.a's flux of the zone's stars, the 0.3 albedo, the
    composition at formation flux with a drawn rank, and T11.d's giant radius and internal heat;
    stars from HPT's track at 0.1, 1, 5 and 12 Gyr. Both tests pass: no body hotter than its
    hottest host at ±H, and no jump of 10⁻³ per year in temperature, radius or envelope.
- **Deviations in T1.a and T1.d, as built (`context`, round 7).**
  - _Shape._ `planetary/context.rs`: `SystemContext` (re-exported from `planetary` with `HostKind`)
    has these getters:
    - `id`, `host_kind`, `stars() -> &[StarModel]` (by body index) and `hierarchy() ->
&SystemHierarchy`;
    - `composition`, `fe_h` and `alpha_fe() -> Option<Dex>`;
    - `age_at_epoch`, `age_at` and `existence_at` (plan 03's `Existence`, in `SystemRecord`'s
      arithmetic);
    - `tidal_radius`, `strip_radius` and `encounter_environment() ->
Option<&EncounterEnvironment>`.

    Two conveniences serve T30.a. `zones()` is P14.T9's `stable_zones(&ZoneHierarchy::from(..))`,
    the zones at birth. `zone_stars()` gives the `ZoneStar`s that `ZoneDiscInputs::for_zone` reads.
    `for_system(galaxy, id)` is `resolve` followed by the public `from_record(galaxy, &record)`, for
    callers that already hold a record. `builder()` is as Provides has it. The ID (`system`), the
    stars (`star` or `binary`) and `age_at_epoch` are required, or the build fails with
    `BuildSystemContextError::Missing*`; `fe_h`, `star_draws`, `tidal_radius` and
    `encounter_environment` have defaults. New beside them:
    - `UNIVERSE_AGE`, 13.787 Gyr (Planck 2020, Table 2, TT,TE,EE+lowE+lensing+BAO);
    - `solar_neighbourhood_tidal_radius`, `SyntheticDraws` and `EncounterEnvironment`;
    - the two build errors.

    The re-validation of `for_system` that the `doc` lane's bullet above left pending on P06.T29.b
    is done here. P11.T2.c remains.

  - _The companions are not in._ P11.T2.c had not merged, so a real system is its primary alone.
    That is `SystemStars::generate`'s one `StarModel`, with the single-star hierarchy that
    `draw_hierarchy(.., ForcedSingle, RedrawAttempt::FIRST)` gives, which draws nothing. The sphere
    of influence is `tidal_radius(hierarchy.system_mass(), &PointLy::from(epoch_position))`, so it
    is taken at the primary's initial mass until then.

    A test checks that each hierarchy slot is its model's star. It fails once `SystemStars` holds
    companions and the hierarchy does not. At that merge `from_record` takes `SystemStars`'
    hierarchy instead. The golden then gains the companions, and its tidal-radius, strip-radius and
    zone lines move under that task's bump.

  - _As the slice says._
    - \[α/Fe\] is `None`, documented as not modelled rather than solar.
    - The encounter environment is `None`.
    - The strip radius is `SATELLITE_STABILITY_FRACTION` (0.4895) times the tidal radius. T29 folds
      the encounter cut into the same `strip_radius()`, so the stage has one strip radius; until
      then a synthetic host's environment is carried but not applied.
    - Every context is `HostKind::Stellar`.

    `EncounterEnvironment` holds what T29 reads of a feature member: the number density (per ly³),
    the one-dimensional velocity dispersion (km s⁻¹) and the mean member mass (M☉), validated.
    **For the orchestrator to rule:** its shape is set here, ahead of plan 09. The relative speed
    T29's rate needs is T29's to form from the dispersion. `hierarchy()` returns a
    `&SystemHierarchy`, which a rogue planet's context (T27.b) cannot fill; T27.b decides.

  - _Synthetic hosts._ The builder accepts:
    - a primary of 0.08–150 M☉;
    - a companion of 13 Jupiter masses up to the primary's mass, in a brown-dwarf slot below
      `MIN_COMPANION_MASS`, which plan 11's draw makes only from P11.T2.d;
    - a period of plan 11's 0.1–10¹¹ days;
    - an eccentricity inside Moe and Di Stefano's envelope at that period (circular below 12 days)
      and under 0.9999;
    - an apocentre inside `TIDAL_CUT_SHARE` of the sphere of influence;
    - an age from −H to `UNIVERSE_AGE`.

    The orbit lies in the reference plane at periapsis at the epoch. Plan 11's `SystemHierarchy`
    has no public constructor, so `stellar/multiplicity/hierarchy.rs` gains crate-private `single`
    and `binary` beside the test-only `hand_built` (recorded in plan 11's Risks). The type stays
    stable by construction. **For the orchestrator to rule:**
    - Stars take the median star's draws unless `star_draws(SyntheticDraws::OfUniverse(seed))`
      asks for plan 06's own draws of each body. So `synthetic_star` gives every sample the median
      disc lifetime (design note 12), and only its planetary streams differ.
    - With no `tidal_radius`, a synthetic host's sphere of influence is King's (1962, eq. 24) r_J =
      (G m ÷ 4A(A − B))^⅓. It uses Bovy's (2017, eqs. 5–6) Oort constants, 15.3 and −11.9 km s⁻¹
      kpc⁻¹, and gives 1.372 pc for 1 M☉. Plan 02's Milky-Way fixture gives 1.296 pc at the Sun-like
      point, 5.5% less. A synthetic host has no place from which to read the potential tables.

  - _Zero-age states (`zone_stars`)._ A star takes Tout et al.'s zero-age main sequence at its
    initial mass and the system's composition (design note 6). A brown-dwarf companion takes its
    cooling fit at 10 Myr. The disc-lifetime rank is each model's own. **For the orchestrator to
    rule:** Tout et al. fit from 0.1 M☉ and call extrapolation in mass "inaccurate but still
    reasonable" (their §2), but grid primaries start at 0.08 M☉. At 0.08 M☉ the fit gives 4.69 ×
    10⁻⁴ L☉ and 0.102 R☉. The cooling fits' main sequence (their 5 Gyr state) gives 2.52 × 10⁻⁴ L☉
    and 0.098 R☉, so the snow line comes out 1.37 times as far. At 0.1 M☉ the two agree to 4 ×
    10⁻⁶. The alternative is the cooling fits' main-sequence state below 0.1 M☉, where `StarModel`
    already evolves stars on those fits.
  - _`planetary::testing`_ (`cfg(any(test, feature = "testing"))`).
    - `synthetic_star(id, mass, fe_h, age)` and `synthetic_binary(id, m1, m2, a, e, fe_h, age)`
      return the builder's `Result`.
    - `sample_contexts(n, seed, filter)` builds the Milky-Way-parameter galaxy of `seed` and
      returns `Result<Vec<SystemContext>, SampleContextsError>`, each `from_record` of
      `sample_records(&galaxy, n, filter)`.
    - `sample_records` lists the `n` systems nearest `SAMPLE_CENTRE_LY`, (0, 26,000, 0) ly, at the
      epoch that pass the filter, nearest first with ties broken by ID. Over all masses the sample
      is volume-limited, and the sample of k is the first k of any larger one. The search starts
      at 16 ly and doubles the radius until a sphere holds `n`. A sphere may cover at most
      `MAX_SAMPLE_CELLS` = 2¹⁸ cells; beyond that the answer is `TooFewSystems`.
    - `SampleFilter` is a band of primary mass, which also skips the layers it cannot reach, and a
      `fn(&SystemRecord) -> bool`. Filters on \[Fe/H\] or on the stars' states apply to the
      contexts afterwards.
    - `solar_system_bodies` is not T1.d's and is not built.

    **For the orchestrator to rule:** this definition of the sample. The crate's `Cargo.toml`
    comment on the `testing` feature names the module.

  - _T1.a._ `planetary/mod.rs`'s table gains the items T1.d consumes:
    - plan 01's time, clock window and units;
    - plan 02's `tidal_radius` and the galaxy and cells the sample reads;
    - plan 03's `resolve`, `ResolveSystemError`, `SystemRecord` and `Existence`;
    - plan 06's `SystemStars`, `StarModel`, `MAX_STAR_MASS`, `StarDraws`, `draw_metallicity`,
      `Composition::from_fe_h` and `substellar::cooling`;
    - plan 11's `draw_hierarchy`, `ForcedSingle`, `TIDAL_CUT_SHARE`, the companion floors, the
      period range and the eccentricity envelope.

    Plans 09 and 13 are listed as not built. The \[α/Fe\] and the X-ray and ultraviolet closed
    forms still wait with their consumers, as T1.a's _Slice_ note says.

  - _Tests and golden._ Accept with `cargo test -p hyperion-sim --lib -- planetary::context
planetary::testing` (17 and 7 tests) and `cargo test -p hyperion-sim --test
planetary_context_golden`. The tests cover:
    - `NoSuchSystem`;
    - the builder refusing a negative mass (primary or companion), an age beyond the universe's or
      before −H, an orbit plan 11 would not draw, and every other invalid input;
    - each field of a real context against the stage it comes from, bit for bit;
    - two galaxies built apart giving equal contexts, and `assert_order_independent` over
      `for_system` and over builders;
    - the synthetic binary's zones against the same pair built for T9, and the zero-age states;
    - the sample's nearest-first order, its completeness inside its farthest member, its refusals,
      and `sample_contexts` equalling `for_system`.

    The new golden, `planetary/context` at 11, holds six real systems of layers A, C and E and
    three synthetic hosts. Nothing existing moved.
- **Deviations in T28.a–c, as built (`fate`, round 7). Ruled (ruling 62).**
  - _API._ `fate::state_at(&FateBody, &FateHost, t) -> FateAt`, and `BodyFate::resolve(body, host)` with `at(t)`, `formed_at()` and `ending()`: a body's history is fixed once, from the body and its host alone, and read at any time, so the prefix property holds by construction. `FateAt` has `state()`, `orbit()` (the `KeplerElements` at `t`, only while `Present`), `valid_until()` (the next formation, ending or supernova inside the clock window) and `body_orbit()`, T34's `BodyOrbit`. It is the plan's `(BodyState, Elements)`: a body not present has no elements. `BodyState::ended_at()` is added, and `record`'s types are otherwise untouched.
  - _The seam T30.b fills._ Plain inputs, since `SystemContext` and the placer are being built beside this. `FateHost::star(&StarModel)`, or `FateHost::stars(..)` for a circumbinary body, every star below its pair (non-empty and coeval), summed in the order given. `FateBody::new(Formation, orbit, mass, density)`, with the primordial `KeplerElements` about the host's initial mass and the bulk density for the Roche test, then `.with_circularisation(Circularisation)`. The formation distance is not read: nothing in T28.a–c depends on it. The disc lifetime enters through `hosts::young::Formation::draw(seed, BodyId, mass, lifetime)`, which is primordial and which T30.a draws with the body. T8.e's τ_c enters as `Circularisation::new(Years)`, since its constant-Q form is the `place` lane's. This lane applies e(t) = e₀ exp(−(age + t) ÷ τ_c) at constant a(1 − e²) (`hosts::evolved::circularised`). **Ruled (ruling 62):** T8.e (`place`'s `classes/tides.rs`) defines τ_c, the transform applies it, and at the merge the transform calls `place`'s τ_c rather than a copy.
  - _T28.a._ `planet.origin` (`Body`) is registered after `planet.mass`, and `tags.golden` gains its one line. Every planet reads word 0 (the giant's rank) and word 1 (the magma ocean's), and words 2–7 are reserved. A giant is a planet from 0.1 M_J, design note 7's `SPACING_GIANT_MASS`. It forms at 0.5^(1−u) L^u Myr, held to 0.5 Myr–L, or at L itself for a disc shorter than 0.5 Myr (`EARLIEST_GIANT_FORMATION`); a small planet forms at L. The magma ocean ends at a host age log-uniform over 10–100 Myr, no earlier than the formation (`MAGMA_OCEAN_END_EARLIEST`, `_LATEST`), which nothing reads until T13 and T24. The log-uniform law is this lane's reading of "(drawn)" (**ruled, ruling 62**: it stands). There is no `ProtoplanetaryDisc` body (the _Slice_ note). A body whose formation age falls at or after a host star's death never forms.
  - _T28.b._ The reach follows Mustill and Villaver (2012), not the plan's f = 2–3 (**ruled, ruling 62**). `params.rs` gains `ENGULFMENT_TIDAL_MASS`, M_c = 3.1 M⊕, and `engulfment_reach` is f = (1 + M_p ÷ M_c)^⅛: Zahn's (1977) equilibrium tide, which they integrate, drags a planet in at ȧ ∝ M_p (R★ ÷ a)⁸. M_c is fitted to their Figure 7, read from its vector paths. The figure gives the initial axes at the start of the thermally pulsing AGB of the outermost circular Terrestrial, Neptunian and Jovian planets engulfed about 1–5 M☉ stars, beside each star's largest AGB radius. Each ratio of axis to radius is f times the share of the star's mass left at its largest radius, one factor a star, which the transform's own expansion supplies. The least-squares fit gives M_c = 3.10 M⊕ at Zahn's ⅛, and an exponent of 0.129 when it is free. It reproduces all eighteen axes to 1.8% rms and 4% at worst. f is 1.036 for the Earth, 1.264 for Neptune and 1.786 for Jupiter, with no dependence on the host's mass: their Jovian-to-Terrestrial ratio is 1.65–1.82 with no trend from 1 to 5 M☉. a(t) = a_c(t) M₀ ÷ M(t), with M from `StarModel::state_at` of each host star. The radius is the largest `max_radius_until` among the host's stars not dead by the segment's start. **The clearance a − f R_max is not monotone**, as T28.b took it to be (**ruled, ruling 62**, and T28.b corrected). After the red-giant tip R_max holds while the winds widen the orbit. So along the 1 M☉ track an Earth born at 0.55–0.68 au is inside the tip's reach there, and outside the final radius's reach by the end (1.25 au against 1.05 for one at 0.65 au). A bisection on the whole interval would find no crossing for it. So the first crossing is found by a scan of fixed points, then a bisection. The scan evaluates the start, then end − span × 0.75ᵏ for k = 1–64, then the end. The bisection works on the clock's nanoseconds to 1 s, in at most 64 halvings. The points depend on the segment alone, from the formation to the end of the window or the last death. A dip shorter than a quarter of the time then left to the end would be missed, and none is on the tracks tried: a tip's engulfment lasts through core helium burning. A body that no host can reach costs two evaluations. One that it can costs about 124: 163 µs a body at a load of 12.6 and 2.3 GHz, where `math::exp` took 8 ns, so about 20,000 `exp`s.
  - _The limits against theirs (for the orchestrator)._ On plan 06's tracks the limits at birth are 0.680, 0.830 and 1.173 au (Earth, Neptune, Jupiter) about 1 M☉, where the red-giant tip (0.862 au at 0.759 M☉) sets them, so the Earth survives the Sun at 1.924 au; 1.048, 1.279 and 1.807 au about 1.5 M☉; and 1.330, 1.623 and 2.294 au about 2 M☉. At the start of the AGB, whose mass is what their axis reads, the 1.5 and 2 M☉ tracks' limits are 1.115, 1.361 and 1.923 au at 1.41 M☉ against their 1.80, 2.17 and 3.10 au there, and 1.347, 1.644 and 2.323 au at 1.975 M☉ against 2.13, 2.48 and 3.51. The tracks' limits are 62–66% of theirs because SSE's largest AGB radius is 61–63% of Vassiliadis and Wood's (1993) there: 1.42 au against 2.26 at 1.41 M☉, and 1.82 against 2.97 at 1.975 M☉. The pattern is theirs: Jovian over Terrestrial is 1.72 against 1.71 and 1.65. Their §5 names the stellar model as the larger uncertainty. Matching their astronomical units would mean larger AGB radii in plan 06, not a larger f.
  - _T28.c._ At a sudden death (`DeathKind::is_sudden`), the orbit just before is taken about the progenitor's helium core plus envelope (plus the other stars), with `relative_state_at(t_d)`. The kick is subtracted from the velocity, and a circumbinary body takes the remnant's share m_rem ÷ M_after of it. μ afterwards is μ₀ M_after ÷ M₀, and `elements_from_state` gives the new orbit. A bound orbit starts a new segment, with no further circularisation. An open orbit is `Unbound`, including a bound one from e = 0.9999, whose apocentre is over 2 × 10⁴ pericentres out. A pericentre inside the remnant's fluid Roche limit, 2.456 (3M ÷ 4πρ)^⅓, is `Destroyed { TidallyDisrupted }` at the next pericentre, or at once for a plunge. The kick is `StarModel::natal_kick`, `None` until P06.T19, so it is zero (ruling 33). The transform applies one as soon as the model returns it, which moves output with P06.T19's bump. A white dwarf's own kick, at an envelope-loss death, is not applied, and e ≥ 0.9999 after a supernova counts as unbound (**ruled, ruling 62**). A companion's planets take their own star as host, so they see only its mass loss (P11.T4 waits).
  - _Envelope-loss deaths step for 7.7–8.1 M☉ (ruled, ruling 62: adiabatic, the step the cap's artefact)._ A white dwarf's birth is adiabatic and no event. Under ruling 57.1 these tracks end the AGB with 5.4–5.9 M☉ of envelope still on, so the mass steps from about 7.6 to 1.37 M☉ at the death, and the orbit widens by 5.6 at once. The end state is right, since a super-AGB superwind of 10⁴–10⁵ years is adiabatic. But the elements step, and the continuity test exempts those deaths. Plan 06 could give the track a finite superwind.
  - _Tests_, in `planetary/fate/tests.rs` and `planetary/hosts/`, all on synthetic hosts, with bodies placed by hand.
    - The prefix property and continuity over 10 hosts of 0.08–25 M☉ × 44 bodies × about 110 times through each life. Continuity is a relative 10⁻³ in a and 10⁻³ in e over a year, and angles bit for bit.
    - (a) 10⁴ draws: no giant after its disc, and a Kolmogorov–Smirnov test of the log-uniform law.
    - (b) Along the 1 M☉ track every Jupiter of 0.02–1.1 au and every Earth of 0.02–0.6 au is engulfed by the death: "about 1 au" holds for the Jovian limit, 1.17 au, and the rocky one is 0.68 au (ruling 62). Survivors of 1.5–100 au end at a₀ × 1.9244 (M_wd = 0.51965 M☉) to 10⁻⁹. The limits follow f times the red-giant tip's 0.862 au times 0.759 to 1%, and at 1.5 and 2 M☉ keep Mustill and Villaver's pattern. The Earth at 0.65 au is a regression test for the scan. The fit itself is tested against the eighteen critical axes read from their Figure 7.
    - (c) In the kernel, e = ΔM ÷ M_after, and a circular orbit is unbound from half. A 12 M☉ neutron star unbinds Jupiters at 60–1,000 au. A 25 M☉ complete-fallback black hole, which loses 10⁻⁸ of its mass, keeps them with e < 10⁻⁶, and engulfs one born at 3 au, inside its 4.2 au reach. A 20 M☉ black hole (8.30 → 6.81 M☉) leaves e = 0.219. A circumbinary planet of 20 + 3 M☉ sees the pair's loss. Kicked bodies are unbound (in the kernel).
    - A scalar axis stands in for the orbit's in the search, and is tested to agree with it bit for bit.
  - New golden `planetary/fate` (formation draws, the reach, and histories on the 1 M☉, 12 M☉, 25 M☉ and 20 + 3 M☉ hosts), blessed at 11.
- **Deviations in T35.b–c and T37, as built (round 7, `wire`, second pass).**
  - _Records (ruling 53)._ `BodySummaryDto { id, kind, label, parent, state, position_m, mass_kg,
orbit, moons, rings, bulk }`, and `BodyRecordDto`, the same with `surface` and `hooks`, mirror
    `record::BodyRecord`: the mass is its own `mass_and_orbit` section, `mass_kg`;
    `BodyKindDto::Planet` carries no class, which is `BulkPropertiesDto::class`; the label is a
    `SectionDto<String>`; the mass fractions are the bulk's `mass_fractions`; moons, rings, belts
    and halo are lists of bodies (the halo `SectionDto<Option<BodyIdHex>>`), by `BodyIdHex` rather
    than the bare index, as `OrbitHostDto::Body` names a body and `parseBodyId` reads one. The plan's summary
    list has no mass or position; both are added. The ID, parent, state and position are plain
    fields at every level, as `degrade` keeps them: `parent: Option<OrbitHostDto>`, `null` only for
    a root body, which `BodyOrbitDto.parent` repeats, since the tree is rebuilt at `contact`, when
    the orbit is withheld; `position_m: Option<[f64; 3]>` in the system frame, `null` for a body not
    present or a population. `BodyKindDto` and `BodyStateDto` are tagged by `type`, with every
    variant: `{"type": "moon", "origin": "giant_impact"}`, `{"type": "belt", "belt_kind":
"kuiper"}`, `{"type": "destroyed", "cause": "engulfed", "at": …}`.
  - _`body_detail` answers with a wrapper._ `ResponseBody` is tagged by `kind`, and a record has a
    `kind` of its own, so the two cannot share an object: `BodyDetailDto { universe, time, granted,
record: BodyRecordDto }`. `ResponseBody::SystemBodies` and `BodyDetail` hold their DTOs boxed
    (Clippy's `large_enum_variant`); the wire and TypeScript are unchanged by it. **For the
    orchestrator to rule.**
  - _SI units_, as the task says: `mass_kg`, `radius_m`, `density_kg_m3`, `surface_gravity_m_s2`,
    `equilibrium_temperature_k`, and metres for every zone. The display's M⊕ (D19) divides by the
    sim's `EARTH_MASS_KG`, GM⊕ ÷ G = 5.972 17 × 10²⁴ kg, and a client constant that differs moves
    the last digits. **For the orchestrator:** the alternative is `mass_mearth`, the sim's own unit.
  - _No value yet._ `BodySurfaceDto` and `BodyEventDto` are uninhabited enums, TypeScript's
    `never`, as the sim's `Surface` and `Hooks` are, so no `ok` surface and no event parses.
    `BodyHooksDto { surface_seed: SurfaceSeedHex }` holds the one hook whose form the plan fixes,
    in a new 16-hex-digit newtype; T23–T26 add the others, as `Modelled` fields where a hook lands
    before its section's others. **For the orchestrator:** an uninhabited `BodyHooksDto` would
    mirror the sim exactly.
  - _Zones._ `ZoneDto { host, inner_m, outer_m, snow_line_m, plane: SystemPlaneDto, architecture:
ArchitectureClassDto, habitable_zone: Option<HabitableZoneDto> }`, one per `stable_zones`
    entry, in its order. `HabitableZoneDto` has Kopparapu's five limits in metres and
    `extrapolated`; a limit the sim puts at +∞ is `null`, since JSON has no infinity. Beyond the
    plan's list, for the orchestrator: `architecture`, since T43.b's readout names the class and
    nothing else carries it; and `SystemBodiesDto.system_plane`, D21's reference plane, chosen by
    the server, which knows which host is a close binary's. Zones are not sections and do not
    degrade; the Knowledge overlay may want the plane and the class withheld below
    `mass_and_orbit`.
  - _`body_events`._ Its string is in `REQUEST_KINDS` with the other two, as T35.c says, and so it
    has its variants now (`BodyEventsRequest { universe, system, from, to }`, `BodyEventsDto { …,
events }`): plan 04's `request_kinds_lists_every_variant` and the server's
    `kind_names_every_body_as_the_wire_does` need a variant per string, and a string without one is
    answered `bad_request`, not `unsupported`. All three kinds answer `unsupported` until T36 and
    T31. The window's ends, `from` inclusive and `to` exclusive, and `to` as the field a refusal
    names, are provisional until then. **For the orchestrator:** the slice note's "two in the slice" is three.
  - _Server._ `is_large` is true for `system_bodies` (up to 255 members a belt) and `body_events`
    (comet tracks), false for `body_detail`; a test checks the three kinds answer `unsupported`.
  - _T37._ `packages/protocol/fixtures/planetary.json` holds ten messages: the slice's
    `system_bodies` answer; a populated one with every kind, state and section state, shapes rather
    than a generator's output; `body_detail` at `full`, `mass_and_orbit` and `contact`;
    `unknown_body`; a `body_events` answer, with no events; and the three requests. `hyperion-protocol` requires each to round-trip exactly
    and the slice's to equal its wire-form builders, and `planetary.test.ts` decodes them through
    `decodeServerMessage`, rebuilds the tree from `parent` and writes the requests. Its numbers
    have twelve significant figures, because serde_json's default parser (no `float_roundtrip`)
    misread a 17-digit position by an ulp; the server only writes floats, so the wire is exact. No
    `ErrorCode` is new, so `settledState` is unchanged.
- **P14.T41–T43 for bodies, and ruling 59's fixes, as built (round 7d, `ui`).** Against
  `packages/protocol/fixtures/planetary.json`, imported by `test/planetaryFixture.ts` and decoded
  through `decodeServerMessage`; the server still answers `system_bodies` and `body_detail` with
  `unsupported`.
  - _Ruling 59._ The primary's SMA cell reads the em dash (`readout__missing`). `SpatialView` gains
    an optional `onFitChange(fitting)`, called from the key, wheel and pinch handlers (never an
    effect): a zoom by hand releases the pressed zoom preset, and `Z` presses it again, since the
    view then fits its radius once more. The `useOrbitCamera.ts:137` lint error was the heuristic
    reading the parameter annotation `transition: Transition` as a CSS declaration; the parameter is
    renamed `turn`. The camera's turn already honours reduced motion (`turnTo(…, instantly)`).
  - _Requests (T41.a)._ `useSystemBodies` beside `useSystemSummary` inside `useSystemData`, both
    asked at the same request time with `detail: "full"`; D18's re-request also fires one
    nanosecond past each body orbit's `valid_until`, the last instant its elements hold. While a
    bodies answer is on show the hosts are drawn from its own `hosts`, so that bodies and stars are
    one answer; otherwise from the summary. `unsupported` from `system_bodies` is ruling 59.1's
    transitional state: no fault, the hosts alone, and the note keeps `PLANETS, …` until bodies
    arrive. `useBodyDetail` asks `body_detail` for a selected body (not a host) at the request time;
    the list's entry is read until the record comes, and the record's own `RequestStatus` stands
    under the readout, outside its live region (ruling 13). `lib/system/bodiesWire.ts` converts and
    checks both answers (IDs of the system and in index order, hosts that exist, no circular parent
    chain, propagatable orbits, positive masses and bulk values) into a fault in words:
    `BODY DATA INVALID: …` with `RETRY`, the stars drawn alone. Masses are held in M⊕ by
    `EARTH_MASS_KG`, the sim's GM⊕ ÷ G to the bit.
  - _Orbit map (T42.a–b)._ `bodyMap.ts`: bodies placed by `composePosition` on their orbit about
    their `OrbitHostDto` parent (a `pair` through `HierarchyLayout.pairKeyedBy`, `barycentre` the
    root); a population's member (a belt's dwarf planet) orbits what the population orbits. A body
    not present is not drawn. A contact, whose orbit is withheld, stands at the server's
    `position_m`. Symbols: planet `triangle-down`, giant (gas or ice giant by the bulk class) 3,
    planet 1, dwarf planet 0 raised to ruling 35.5's floor of 1; moon `pentagon` 1, contact
    `hexagon` 2, each the smallest class whose flattest side lies at least half the 1.5 px outline
    inside a same-size circle (tested); populations take none. A moon's orbit is not drawn in the
    system frame (its elements are about its planet's equator, which the wire lacks; FOCUS BODY).
    The plane is `system_plane` once bodies arrive (ruling 59.2's fallback before). Zones: the
    stable zone where companions bound it, the snow line, and the conservative habitable zone
    (moist to maximum greenhouse), in the system plane about their host, each switchable
    (`STABLE ZONE`, `SNOW LINE`, `HABITABLE ZONE` toggles). `INNER` fits the nearer of the primary
    zone's habitable-zone outer limit and the fifth body's reach; `ALL` the farthest of bodies,
    stars and `INNER`. Belts, rings, discs and the halo have no geometry on the wire yet and are
    listed, not drawn. The legend names each body kind drawn at its size (`GIANT PLANET`,
    `PLANET`, …).
  - _List and readout (T43.a–b)._ `systemRows`: hosts, each body under its star or the body it
    orbits, by SMA, then those with no orbit by index; what orbits a pair or the barycentre
    follows the hosts at the top. A body not present reads its state in words in the SMA column
    (widened to 15ch). The readout renders every section from its tag: `NOT RESOLVED` or
    `NOT YET MODELLED` once in place of the section, `not_applicable` no row. Rows: DESIG, ID,
    LABEL, KIND, ORIGIN (moon), STATE, CAUSE and SINCE (`UT …`), PARENT, DETAIL, MASS (M⊕), SMA,
    PERIOD, ECC, INC, DIST, CLASS, RADIUS (km), DENSITY, GRAVITY, T EQ, IRON/ROCK/WATER/ENVELOPE
    (%), MOONS, RINGS, and from the whole record SURFACE and HOOKS (SURFACE SEED). The bodies
    panel reads `DETAIL` (granted level) and `ARCH` (the primary's zone's class); a star's readout
    reads each zone that holds it: ZONE, ARCH, STABLE ZONE (left out about a single star),
    SNOW LINE, HABITABLE ZONE (`~` when extrapolated, `FROM` with an outer limit beyond every
    orbit, `NONE` without one).
  - _System note (T41.b)._ `smallBodySections`: belts and halo from the system, moons and rings
    from each planet and dwarf planet.
  - **For the orchestrator to rule:** (1) a dwarf planet at the floor is a smaller planet's size,
    so size no longer tells them apart on the map (the list and legend do); (2) DIST is derived on
    the client, from the server's elements at the display time (agreeing to 10⁻⁹, T39), against
    D18's "every number in a readout comes from the server", since the answer is up to a year old;
    INC of a planet is to the system plane (the angle between two server normals), a moon's the
    wire's, to its planet's equator; (3) the state words stand in the SMA column; (4) only the
    conservative habitable zone is drawn and read; (5) level words `CONTACT ONLY`,
    `MASS AND ORBIT ONLY`, `TO BULK`, `TO SURFACE`, `FULL`, and `ARCH`, `DETAIL`, `DIST`, `SINCE`,
    `PARENT` as labels, none yet on the nomenclature list; (6) moon and contact sizes 1 and 2 by the
    half-outline test; (7) a pair reads `PAIR /0 /1`. The by-eye check at 1920×1080 and 1280×720 is
    still to do: the bodies panel does not scroll, and a planet's readout is about sixteen lines.
    `GALAXY`'s draw lists are byte for byte unchanged (the 1,152-scene probe, SHA-256 `9cf26949…`
    before and after).

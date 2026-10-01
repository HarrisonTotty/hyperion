# Plan 12: Retarded-Time Observation and Alerts

- **Milestone:** M4.
- **Depends on:** [09 Large features and catalogue classes](09-features-and-catalogue-classes.md),
  [11 Multiplicity and interacting binaries](11-multiplicity-and-binaries.md). Through them it reads
  plans 01, 03, 04, 05, 06, 07, 08 and 10.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/galaxy-generation.md)): "What a sensor sees is the past" and
  the microlensing sentences that close "Orbits and time"; "Alerts" and the "evaluated at, which for
  a sensor is the retarded time" reading of the supernova table under "Events in time"; the
  "Knowledge" bullet of "Overlays and persistence", as far as alerts need it; the alert-scan cache
  named under "Runtime and code shape"; the source horizon of "Time"; "Sensors see the past light
  cone" under "Decisions"; the items of "Testing" that concern time and events seen from a distance;
  step 10 of "Suggested order of attack".

## Goal

When this plan is done, every query can be asked in one of two modes. In `now` the answer is the
state of the galaxy at the query's time. In `observed from x_o` the spatial search is unchanged, on
present positions, and each object found also carries what a sensor at x_o would see: its state at
the retarded time t − d ÷ c, its apparent position, the age of the light, and a stated bound on the
error from neglected curvature, defined back to the start of the source horizon. An observed
position and velocity, extrapolated to the present, land exactly on the star. "What went off?" is a
pure function in the sim: it walks the catalogue classes within a sensor's horizon, evaluates each
host over the retarded interval, and returns a complete census per class or nothing. The server
wraps it in a subscription that is re-evaluated when the clock advances or the observer jumps, runs
the galaxy-wide scan of the accreting white dwarfs in the background and caches it, and delivers
detections through a minimal Knowledge overlay: first a bearing and a flux, later a resolved host.
The `GALAXY` display gains the mode, an alert list and chart marks that follow the UX guide and show
retarded and present positions honestly. Microlensing exists as a query-time ray walk along one line
of sight. Light echoes are noted and deferred.

## Scope and non-goals

In scope:

- `hyperion_sim::observe`: retarded time with one fixed-point step, apparent position, light age,
  the curvature error bound, observed state of a system, extrapolation to the present.
- `QueryMode` on the range query, the system summary and ID resolution, in the sim and on the wire.
- `hyperion_sim::alerts`: reach per class, the catalogue walk, retarded intervals, durations, the
  census per class, features and the global list, local events with the range query, bearing and
  flux with extinction.
- `hyperion_sim::lensing`: the ray walk and point-lens magnification. API and tests only.
- Server: the alert service (subscription, schedule, background scan, cache), the Knowledge store in
  its minimal form with persistence, protocol messages and notifications.
- Client: mode control and observer mark, observed-mode readout, the alert list, chart marks, and
  the UX guide edits they need.

Non-goals:

- Sensor models. A sensor here is two flux limits per band. Apertures, integration, sky background
  and the detectability of ordinary stars in observed mode belong to the sensor consoles.
- A ship. The observer is a position and a time that the client sets. When a session with a ship
  exists, the server feeds the same functions from it (Design note 9).
- Knowledge beyond alerts: degraded body records ("mass and orbit only") arrive with plan 14.
  Knowledge per crew arrives with sessions.
- Light echoes. Deferred: an echo is a query-time appearance of dust lit by a past event, it needs
  the dust field along paths that are not lines of sight, and nothing else depends on it. The
  central black hole's past luminosity is already a function of time (plan 09), which is what an
  echo would read.
- A lensing survey, which the brainstorm rules out, binary lenses, and any lensing display.
- New generated content. This plan changes no star.

## Provides

Rust paths are under `hyperion_sim` unless a crate is named. Signatures are sketches.

### `observe`

```rust
pub enum QueryMode { Now, ObservedFrom(GalacticPosition) }
pub struct Observer { /* position: GalacticPosition, time: UniverseTime (within ±H) */ }
impl Observer { pub fn new(position: GalacticPosition, time: UniverseTime)
    -> Result<Self, BuildObserverError>; }               // outside the cube or the clock window

/// Anything with a position that is a pure function of time.
pub trait Trajectory {
    fn position_at(&self, t: UniverseTime) -> GalacticPosition;
    fn velocity_at(&self, t: UniverseTime) -> GalacticVelocity;
}
pub struct Retardation { /* emitted: UniverseTime, light_age: Seconds, distance_now: LightYears,
    apparent_position: GalacticPosition, velocity_then: GalacticVelocity */ }
pub fn retarded(observer: &Observer, source: &impl Trajectory) -> Retardation;
pub fn retarded_exact_linear(observer: &Observer, epoch_position: &GalacticPosition,
    velocity: &GalacticVelocity) -> Retardation;          // closed form; test oracle
pub struct CurvatureError { /* length: LightYears, angle: Arcseconds */ }   // Design note 3
pub fn curvature_error(galaxy: &Galaxy, at: &GalacticPosition, r: &Retardation) -> CurvatureError;
pub fn extrapolate_to_present(r: &Retardation, now: UniverseTime) -> GalacticPosition;

pub struct ObservedSystem { /* hit: SystemHit (present), retardation: Retardation,
    existence_then: SystemExistence, brief_then: Option<StellarBrief>, error: CurvatureError */ }
pub fn observe_hit(galaxy: &Galaxy, hit: &SystemHit, stars: &SystemStars, observer: &Observer)
    -> ObservedSystem;
pub fn summary_observed(galaxy: &Galaxy, record: &SystemRecord, stars: &SystemStars,
    observer: &Observer) -> (SystemSummary, Retardation);
pub fn bearing(galaxy: &Galaxy, from: &GalacticPosition, to: &GalacticPosition) -> Bearing;
pub struct Bearing { /* azimuth: Degrees in [0, 360), from coreward towards spinward;
    elevation: Degrees in [−90, 90], positive north */ }
```

### `alerts`

```rust
pub enum AlertBand { Photometric(Band), XRay }          // Band is plan 07's `galaxy::gas::Band`
pub struct SensorHorizon { /* per AlertBand: detect, resolve: WattsPerSquareMetre */ }
pub struct AlertQuery { /* observer_position, window: TimeWindow (observer time, within ±H),
    sensor: SensorHorizon, classes: ClassSet, host_limit: NonZeroU32 */ }
pub struct Detection { /* event: EventId, host: SystemId, class: Option<ClassId> (None for a local
    event of an ordinary system), kind: TransientKind,
    emitted: UniverseTime, arrival: UniverseTime, phase: DetectionPhase, band: AlertBand,
    flux: WattsPerSquareMetre, bearing: Bearing, retardation: Retardation, resolvable: bool */ }
pub enum DetectionPhase { Onset, InProgress }
pub enum TransientKind { CoreCollapse, TypeIa, LuminousRedNova, Kilonova, Nova, DwarfNova,
    XrayTransient, XrayBurst, BeXOutburst, GiantEruption, TidalDisruption, CentralFlare,
    Flare, Glitch, MagnetarBurst, FuOrionisOutburst }
pub struct ClassCensus { /* class: ClassId, reach: LightYears, expected_hosts: f64,
    status: ClassStatus */ }
pub enum ClassStatus { Complete, OverLimit, NeedsBackgroundScan }
pub struct AlertResult { /* detections: Vec<Detection> (by arrival, then EventId),
    census: Vec<ClassCensus> */ }

/// The caller's caches and sources, since the sim holds none: plan 07's `NoiseCache`, plan 09's
/// `FeatureGas` as the `GasModifierSource`, its `CatalogueClassSource` and `FeatureCatalogue` with
/// their cell caches, and plan 10's `GlobalList`.
pub struct AlertContext<'a> { /* .. */ }
pub fn class_reach(class: ClassId, sensor: &SensorHorizon) -> LightYears;
pub fn alerts_in(galaxy: &Galaxy, ctx: &mut AlertContext<'_>, query: &AlertQuery)
    -> AlertResult;                                                         // interactive classes
pub fn scan_plan(galaxy: &Galaxy, class: ClassId, query: &AlertQuery) -> Vec<CatalogueCellKey>;
pub fn scan_cell(galaxy: &Galaxy, ctx: &mut AlertContext<'_>, class: ClassId,
    cell: CatalogueCellKey, query: &AlertQuery, out: &mut Vec<Detection>);  // background chunks
pub fn local_events(galaxy: &Galaxy, ctx: &mut AlertContext<'_>, observed: &[ObservedSystem],
    stars: &[&SystemStars], sensor: &SensorHorizon, window: TimeWindow,
    out: &mut Vec<Detection>);
pub fn xray_optical_depth(sightline: &Sightline) -> f64;                    // Design note 14
pub const BACKGROUND_THRESHOLD_HOSTS: f64;     // above this a class is NeedsBackgroundScan
```

### `lensing` and `galaxy::query`

```rust
// galaxy::query
pub fn cells_along_segment(layer: Layer, from: &GalacticPosition, to: &GalacticPosition,
    tube_radius: LightYears) -> impl Iterator<Item = CellKey>;
// lensing
pub struct LensSightline { /* observer: GalacticPosition, source: SystemId */ }
pub struct LensQuery { /* sightline, window: TimeWindow, max_impact: f64 (Einstein radii),
    substellar: SubstellarRequest, cell_budget: NonZeroU32 */ }
pub struct LensEvent { /* lens: SystemId, peak: UniverseTime (observer time), impact: f64,
    einstein_radius: Metres, einstein_time: Seconds, peak_magnification: f64 */ }
pub struct LensResult { /* events: Vec<LensEvent>, walked: LensCensus */ }
pub fn lenses_along<C: CellCache>(galaxy: &Galaxy, cache: &mut C, sources: &[&dyn SystemSource],
    query: &LensQuery) -> Result<LensResult, LensQueryError>;
pub fn magnification_at(galaxy: &Galaxy, event: &LensEvent, sightline: &LensSightline,
    t: UniverseTime) -> f64;
pub fn point_lens_magnification(u: f64) -> f64;        // (u² + 2) ÷ (u √(u² + 4))
```

### Server (`hyperion_server`)

```rust
pub mod alerts { AlertService, Subscription, SubscriptionId, AlertSchedule, ScanKey,
                 ALERT_LOOKAHEAD, SubscribeError }
pub mod knowledge { KnowledgeStore, ContactId, ContactRecord, KnowledgeLevel, Sighting,
                    LoadKnowledgeError, SaveKnowledgeError }
```

Environment: `HYPERION_ALERT_CACHE_MB` (default 32).

### Protocol (`hyperion_protocol`, mirrored in `@hyperion/protocol`)

Everything rides plan 04's request envelope and its rules for extending it: each request kind below
is a variant of `RequestBody` and of `ResponseBody` with the same `kind` string, an entry in
`REQUEST_KINDS` and a wire-form test. Plan 04 reserved `resolve_system`, `subscribe`, `unsubscribe`,
`alerts_observer` and `alerts_acknowledge`, and the `notification` server message, for this plan.

- `QueryModeDto` (`{"type":"now"}` or `{"type":"observed","observer":GalacticPosition}`), as a
  defaulted field `mode` on plan 04's `SystemsInRangeRequest`, on plan 06's `SystemSummaryRequest`
  and on the new `ResolveSystemRequest`.
- `ObservedDto` (`emitted`, `light_age_yr`, `apparent_position`, `curvature_error_ly`,
  `curvature_error_arcsec`) as an optional field `observed` on each range row and on the summary.
- `resolve_system`: `ResolveSystemRequest { universe, system: SystemIdHex, time, mode }` answered by
  one range row. `subscribe`: `SubscribeRequest { universe, topic }` with
  `SubscriptionTopic::Alerts(AlertsSubscribeRequest)`, answered by
  `Subscribed { subscription: u32, state }` with `SubscriptionState::Alerts(AlertsSubscribed)`, as
  plan 04's reservation describes. `unsubscribe`, `alerts_observer` and `alerts_acknowledge` each
  name the `subscription` and answer with an empty body. `SensorDto` is the limits per band.
- `ServerMessage::Notification { subscription: u32, body: NotificationBody }`, its first use, with
  `NotificationBody::Alerts(AlertsNotification { contacts, census })`, and `ContactDto`,
  `KnowledgeLevelDto`, `ClassCensusDto`, `TransientKindDto`, `AlertBandDto`, `BearingDto`.

### Client (`apps/hyperion`)

`ModeControl`, `ObserverMark`, `AlertList`, `useAlerts(requests, observer)`, the pure helpers
`bearingRay`, `apparentOffset` and `formatBearing` (over plan 05's `formatBearingDeg` and
`formatSignedDeg`), and two additions to plan 05's general spatial view: `SegmentMark` in
`spatial/marks.ts` (a line between two points, solid or dashed, which `buildDrawList` clips to the
viewport) and the `transient` value of `SymbolShape`.

### Test helpers

`tests/common/observe.rs`: `brute_force_detections(galaxy, class, query)` (every host of a class by
walking the whole catalogue, each event evaluated directly), `observer_at(galaxy, ly)`.

## Consumes

Names are those of the owning plans' Provides as they stand; the owning plan is authoritative, and
where a name has changed by the time this plan runs only the call sites here change.

- **Plan 01:**
  `time::{UniverseTime, Span, CLOCK_WINDOW_H, LIGHT_CROSSING_L, SourceHorizon, ClockWindow}`;
  `units` with `consts::SPEED_OF_LIGHT` (this plan adds `WattsPerSquareMetre` and `Arcseconds`;
  `Degrees` and `Watts` exist);
  `coords::{GalacticPosition, GalacticDisplacement, GalacticVelocity, Directions}` with
  `GalacticPosition::directions()`, which is `None` on the axis; `id::{SystemId, EventId}`; from
  `hyperion-testkit`, the `golden!` harness and `order::assert_order_independent`.
- **Plan 03:** from `query`, `RangeQuery`, `RangeQueryBuilder`, `RangeResult`, `range_query`,
  `SystemHit`, `QuerySphere`, `position_at`, `epoch_velocity`, `PAD_SPEED`, `pad_for`,
  `cells_in_sphere` and `SystemSource`; from `placement`, `SystemRecord`, `CellKey`, `CellCache` and
  `resolve`.
- **Plan 04:** the request envelope (`RequestBody`, `ResponseBody`, `REQUEST_KINDS`, `RequestError`
  with `ErrorCode::BadRequest` and its `field`), the reserved kinds `resolve_system`, `subscribe`
  and `unsubscribe` and the reserved `notification` message with its `subscription: u32`,
  `compute::{CpuPool, Priority, CancelToken, SingleFlight}`, `cache::{ByteLru, HeapBytes}`,
  `universe::UniverseStore` and the reserved directory name `knowledge/` under a universe's
  directory, the ±H check on query times, `Simulation::now()`, `TestClient`; in
  `@hyperion/protocol`, `RequestClient`.
- **Plan 05:** the general spatial view (`spatial/marks.ts`, `drawList.ts`, `paint.ts`,
  `symbols.ts`), `LocalChartPanel`, `ChartControls`, `SystemList`, `SystemReadout`, `lib/format.ts`,
  `FakeWebSocket`; its guide edits for `yr`, E notation, the direction names and the 3D spatial
  conventions.
- **Plan 06:** `SystemStars::{summary_at, brief_at, death_time}`, `StellarBrief`, `SystemSummary`,
  `SystemExistence`, `events::{TimeWindow, PoissonBins, MonotonePhase}`,
  `stellar::events::{events_in, active_at}`, `stellar::photometry`, the `system_summary` request
  kind with `SystemSummaryRequest`, the server's byte-bounded `SystemStars` cache.
- **Plan 07:** from `galaxy::gas`: `Band`, `sightline`, `Sightline` (`in_band`, `a_v`), `NoiseMode`,
  `Quality`, `NoiseCache`, `GasModifier`, `GasModifierSource`, `HYDROGEN_COLUMN_PER_MAG`. Its `Band`
  has no X-ray value; Design note 14 resolves that here.
- **Plan 08:** real velocities behind `epoch_velocity`; `query::pad_speed(Layer)` and
  `UNBOUND_PAD_SPEED`.
- **Plan 09:** `catalogue_classes::{ClassId, CatalogueCellKey, CatalogueClassSource}` with
  `cells_in_sphere(class, sphere)`, `expected_hosts_in_sphere(class, sphere)` and entries as
  `SystemRecord`s that carry their death time;
  `catalogue_classes::supernova::{SupernovaEntry, LightCurve}`;
  `catalogue_classes::lbv::LbvProcess`; `features::catalogue::FeatureCatalogue::near` and
  `features::members::FeatureLevelList`; `features::gas_overlay::FeatureGas`;
  `motion::{regime_of, position_velocity_at}` for the centre's Kepler members;
  `features::centre::events::{flares, tidal_disruptions, luminosity_at}`.
- **Plan 10:** `global_list::GlobalList::{entries_touching, entries_with_class_members}`.
- **Plan 11:** the class values `STELLAR_MERGER`, `NEUTRON_STAR_MERGER`, `XRAY_BINARY`,
  `ACCRETING_WHITE_DWARF` (fast hosts) and `ACCRETING_WHITE_DWARF_SLOW`;
  `stellar::binary::scan::{AwdScanMarks, XrbScanMarks, scan_marks, engine_calls}`;
  `stellar::binary::events::{events_in, light_curve, xray_luminosity}`;
  `SystemStars::system_mass_at`.

## Design notes

1. **One fixed-point step, for every kind of motion.** The brainstorm says a reading at the retarded
   time "costs one evaluation", and this is that evaluation. With Δ(τ) = x_s(τ) − x_o: s₀ = |Δ(t)| ÷
   c from the present position the search already has, then s₁ = |Δ(t − s₀)| ÷ c, and the retarded
   time is t − s₁. The step corrects by about d v_r ÷ c², 37 years at 50,000 ly, and what it leaves
   is of order d (v ÷ c)², under seven months at plan 03's padding speed and about ten days for a
   disc star; for a bound orbit about the black hole the residual is bounded by the orbit's
   light-crossing time times v ÷ c, hours for an S2-like star. The same code serves drift and the
   centre's Kepler orbits. For drift the light-cone equation is a quadratic with a closed-form root,
   kept as `retarded_exact_linear` and used as the test oracle, not as the implementation, because
   one path for every kind of motion is easier to keep reproducible. The residual never breaks the
   brainstorm's promise that an extrapolated jump lands on the star: for straight-line motion,
   position and velocity at any emitted time extrapolate to the same place.
2. **Light time in seconds from metres.** d ÷ c is computed from the displacement in metres (integer
   cells first, as plan 01 requires) and rounded to nanoseconds. At 50,000 ly an `f64` of seconds
   resolves 0.2 ms, far below anything observable.
3. **The stated error.** `curvature_error` returns min(½ a Δ², |v| Δ + 2 r_apo), with Δ =
   |t_emitted − epoch|, a = |∇Φ| ≈ v_c²(R) ÷ R and R taken at the epoch position, v the source's
   speed and r_apo its apocentre from the energy in the spherical potential, as a length and as an
   angle at the observer. The quadratic bounds the gap while the orbit's phase over Δ is short,
   and the linear term bounds it after, because the orbit stays within r_apo of the centre while
   the line leaves at |v| (ruling 143.2, which replaced the cap of 2R; where it is only an
   estimate is recorded under Risks). It is zero for a member in plan 09's Kepler regime, whose orbit is
   followed and not neglected. It must reproduce the brainstorm's figures at Milky Way values: under
   0.5″ for a disc star from across the cube (0.53 ly at 227,000 years), up to 13″ for the nuclear
   disc (its inner edge near 100 ly, at 100 km/s, seen from a corner of the cube at 113,500 ly). It
   rides with every observed record.
4. **Observed mode changes what is reported, never what is found.** Cells, padding, distance tests,
   the census and the unborn filter all use the present, exactly as in `now`. A found system whose
   age at the retarded time is negative is reported as `NotYetBorn` then; the wire drops its
   `stellar` brief and keeps the row, because the chart is the navigation computer's view of the
   present.
5. **Reach is an upper bound, flux decides.** `class_reach` is the distance at which the class's
   brightest event in its best band falls to the sensor's detection limit with no extinction, capped
   at the cube's diagonal. Hosts are walked on present positions, as every spatial search is, within
   that sphere widened by `UNBOUND_PAD_SPEED` ÷ c of its radius (1%), because a host was where its
   light left it and may since have moved out of reach, and by plan 03's padding. Each event's flux
   at arrival is then computed from the distance at emission with plan 07's extinction, and an event
   under the limit is dropped. So the census is complete per class for what the sensor could see.
6. **An event belongs to the window its light arrives in.** An event emitted at T from a host at
   x_s(T) arrives at t_a = T + |x_s(T) − x_o| ÷ c, which is a closed form with no iteration, and it
   is reported in the window [t₁, t₂) that holds t_a. That makes a split window return exactly the
   detections of the whole. To find the candidates, a host at present light time s is evaluated over
   the emission interval [t₁ − s − D − ε, t₂ − s + ε], where D is the class's longest event duration
   (plan 06's look-back idea) and ε = s × pad speed ÷ c covers the change in light time over the
   interval. Events whose arrival precedes t₁ and which are still above the limit at t₁ are reported
   as `InProgress`. One-shot entries compare the arrival of their T directly.
7. **Complete per class or nothing**, decided before walking from plan 09's expected host count in
   the reach sphere. Over `host_limit` the class reports `OverLimit` and no detections. A class
   whose expected count passes `BACKGROUND_THRESHOLD_HOSTS` (10⁵) reports `NeedsBackgroundScan` from
   `alerts_in`; the caller then drives `scan_plan` and `scan_cell`. A background scan that is
   cancelled or fails yields nothing for its class.
8. **What is cached is a schedule.** The server evaluates a subscription over [t, t +
   `ALERT_LOOKAHEAD`] (one year of universe time) at its observer's position and keeps the
   detections sorted by arrival. Advancing the clock pops from the schedule. Past the half-way point
   the next year is scanned in the background. A jump, or a clock that moves backwards, drops the
   schedule and rescans: interactive classes at once, the accreting white dwarfs in the background.
   Schedules live in a `ByteLru` keyed by `ScanKey` (universe, class set, sensor, observer position,
   window) under `SingleFlight`, "like the density map". Scans run only in the server, on plan 04's
   one bounded `CpuPool`: the sim's `scan_cell` is a pure function of one catalogue cell, and
   nothing here spawns a thread of its own. Nothing is backfilled after a jump: light that passed
   the new position before arrival is gone, which is what lets a crew outrun an event and watch it
   again.
9. **The observer is client-set for now.** Plan 04 has no session. A subscription therefore carries
   an observer that the client moves with `alerts_observer`. The service's core is
   `advance(previous, next) -> notifications`, which a session clock will call unchanged.
10. **Knowledge, minimal.** A `ContactRecord` is keyed by `EventId` inside the server and by an
    opaque `ContactId` on the wire, so an unresolved contact cannot leak its host through its
    identifier. It holds sightings (observer position and time, emitted time, bearing, flux, band)
    and the highest `KnowledgeLevel` reached: `Bearing` or `Resolved`. It becomes `Resolved` when a
    sighting's flux reaches the sensor's `resolve` limit, as the light curve rises or the observer
    closes in. Only then does the wire carry the host's ID, light age, apparent position and
    extrapolated present position. The store is per universe until sessions exist, and is saved as
    versioned JSON lines in the `knowledge/` directory that plan 04 reserved under the universe's
    own. Nothing else of the brainstorm's Knowledge overlay is built: no degraded body records, no
    per-console views, no sensor model.
11. **"Alert" has two meanings.** The UX guide's alerts are ship conditions in four classes, raised
    by the server from simulation state. A detection is awareness only and is raised by the server,
    so it fits the guide's Advisory class and needs no fifth: coloured text in the alert list, no
    sound, no flash, acknowledgeable, never hidden by acknowledgement. The guide's header strip
    counts emergencies, warnings and cautions only, so detections never inflate it. "The condition
    clears", in the guide's words, when the event's flux at the observer falls back under the
    sensor's detection limit; the contact then leaves the alert list and stays in Knowledge. The
    list component is the guide's alert list, built here because no display has needed one yet.
12. **Bearings are local.** Azimuth runs from coreward through spinward, elevation is positive
    north, at the observer. Within a light-year of the axis, where coreward is undefined, azimuth
    runs from +x instead and the wire says so (`frame: "galactic_x"`).
13. **A lens is a point mass** of the system's mass at the time the light passes it, and the lens's
    position is taken at that time, t − D_l ÷ c. The tube's radius is `max_impact` times the largest
    Einstein radius possible on the sightline, plus `PAD_SPEED` times the window. _As built
    (P12.T4.b and ruling 143.3):_ a cone, padded by the local escape speed; see Risks.
14. **X-rays are absorbed by metals, so the X-ray band reads A_V.** Plan 07's `Band` runs from U to
    radio through the Cardelli law, which stops at 0.1 µm, and plan 07 is right to leave X-rays out
    of it. X-ray transients and bursts still need a horizon. Photoelectric absorption at a few keV
    is by the same metals that make the dust, so `xray_optical_depth` is σ_X × A_V ×
    `HYDROGEN_COLUMN_PER_MAG`: the solar-equivalent hydrogen column, which follows metallicity as
    plan 07's dust does, and not `Sightline::hydrogen_column`, which counts the hot corona's ionised
    gas that absorbs almost nothing. σ_X is one cross-section per hydrogen atom at 2 keV, a constant
    of the generator version whose source T5.a re-checks and cites (candidates: Morrison and
    McCammon 1983; Wilms, Allen and McCray 2000). `AlertBand::XRay` exists only in this plan;
    nothing is added to plan 07's `Band`.

## Tasks

T0 comes first. T1–T3 are a chain in the sim. T4 (lensing) needs only T1 and can run beside
everything. T5 follows T2; its subtasks run in order. T6 (protocol for modes) follows T3. T7
(Knowledge) is independent of T5 and can start after T0. T8 follows T5 and T7. T9 follows T8. T10
(guide) can be written at any time and must land before T11 and T12, which follow T6 and T9 and are
independent of each other. T13 closes.

Rust files are under `crates/hyperion-sim/src/` unless a path says otherwise.

### P12.T0 Sweep the source horizon

Plans 03–11 were written for ±H with retardation promised, and the brainstorm defines retarded
evaluation "back to the start of the source horizon". Add a slow test that evaluates one object of
every source (grid system, displaced remnant, cluster member, centre member, stream member, dwarf
core member, an entry of each catalogue class) at −(H + L), −H, 0 and +H: no panic, finite positions
inside the cube padded by `UNBOUND_PAD_SPEED` × (H + L), state defined, and both event constructions
answer for a window at the horizon's start. Any function that refuses or clamps times before −H is
widened in its owning module, without changing its output inside ±H (the goldens prove it). There is
no interface-reconciliation task: the names under Consumes were checked against the owning plans
when this plan was validated, and plans this late are re-validated against the code when their turn
comes (README).

Files: `crates/hyperion-sim/tests/source_horizon.rs`, and whatever the sweep finds. Acceptance:
`just test-slow` runs `source_horizon_sweep`; every golden is unchanged.

### P12.T1 Retarded time

Build `units::{WattsPerSquareMetre, Arcseconds}`, `Observer`, `Trajectory` (implemented for a
`SystemRecord` with its galaxy through plan 03's `position_at` and `epoch_velocity`, which plan 09's
`motion::position_velocity_at` already serves for the centre's Kepler members), `Retardation`,
`retarded` (Design notes 1–2), `retarded_exact_linear`, `curvature_error` (Design note 3),
`extrapolate_to_present`.

Files: `units.rs`, `observe/{mod,retarded,error}.rs`.

Tests: the step agrees with the closed form to d (v ÷ c)² for 10⁴ random sources at up to 1,000
km/s; the correction is 37 ± 1 years for v_r = 220 km/s at 50,000 ly; light age never exceeds L for
any pair of points in the cube; `extrapolate_to_present` of a drifting system equals `position_at`
to 1 m; the curvature figures of Design note 3 at Milky Way parameters, within 25% because the
brainstorm's figures are rounded and the potential is the model's own: 0.53 ly and under 0.5″ for a
disc star at 26,000 ly after 227,000 years; 0.026 ly for the same star after 50,000 years; the
maximum over the nuclear disc from a corner of the cube within 10–16″; zero for a Kepler member; and
the bound, which is never above min(½ a Δ², |v| Δ + 2 r_apo) and never below the exact error of a
circular orbit (ruling 143.2). Acceptance:
`cargo test -p hyperion-sim observe::retarded`, and the `retarded` bench reports at most 0.7 µs
for drift at low load, 0.5 µs from a known present position (ruling 143.4).

### P12.T2 Observed state

Build `ObservedSystem`, `observe_hit` (retardation, existence and `brief_at` at the emitted time),
`summary_observed`, `bearing` (Design note 12). For a centre member the observed elements are the
orbit's, so extrapolation uses the orbit.

Files: `observe/{system,bearing}.rs`.

Tests: a layer-E system with death at T seen from distance d is a living supergiant for t < T + d ÷
c and a supernova of age t − d ÷ c − T after it, with one ID throughout; an observer 26,000 ly out
in the plane at Milky Way parameters sees living supergiants that are dead now, several hundred of
them (slow; counted from the catalogue's supernova class); a system born 100 years ago is
`NotYetBorn` from 5,000 ly; an extrapolated jump lands on the star: observe from x_o, extrapolate,
place a second observer there at the same t, and the distance to the star is under 1 m for drift and
under the stated curvature error for a centre member; bearings of the six named directions.
Acceptance: `cargo test -p hyperion-sim observe::system`.

### P12.T3 The query mode in the sim

`RangeQueryBuilder::mode(QueryMode)`; `RangeResult` gains `observed: Vec<ObservedSystem>` parallel
to `systems` in observed mode, filled through a new caller-supplied trait in the manner of plan 03's
`CellCache`, since the sim holds no cache: `observe::StarsCache` with
`with_stars<R>(&mut self, galaxy, record, f: impl FnOnce(&SystemStars) -> R) -> R`, and
`NoStarsCache` which regenerates. `range_query_observed(galaxy, cells, stars, sources, query)` wraps
plan 03's `range_query`, whose signature does not change. Spatial logic is untouched (Design note
4), which a test pins: the set of IDs and the census are identical in both modes for 100 random
queries.

Files: `galaxy/query/*.rs`, `observe/mod.rs`.

Tests: as stated; observed-mode overhead recorded by the `range_observed_50ly` bench. Acceptance:
`cargo test -p hyperion-sim query::mode`.

### P12.T4 Microlensing

- **P12.T4.a The segment walk.** `cells_along_segment`: a 3D grid traversal (Amanatides and Woo)
  widened by the tube radius, exact integer cell arithmetic. Tests: against brute force over a
  bounding box for 10³ random segments; no cell twice; order from the observer outward.
- **P12.T4.b Lenses.** `lenses_along`, `magnification_at`, `point_lens_magnification`, θ_E² = 4GM ÷
  c² × D_ls ÷ (D_l D_s) (Design note 13); grid layers by mass floor, substellar layers on request,
  `SystemSource`s through their sphere method on a chain of spheres along the segment; a cell budget
  with `LensQueryError::OverCellBudget`. Tests: a pinned lens placed by hand through a test
  `SystemSource` gives the textbook light curve (peak 1.34 at u = 1, symmetric, Einstein time from
  relative transverse speed); the optical depth towards the bulge from 10⁴ random sightlines is of
  order 10⁻⁶ (slow); order independence. Bench `lens_walk_26kly`.

Files: `galaxy/query/segment.rs`, `lensing/{mod,walk,point_lens}.rs`. Acceptance:
`cargo test -p hyperion-sim lensing`.

### P12.T5 Alerts in the sim

- **P12.T5.a Types, reach, flux.** The types under Provides; a registry row per `TransientKind`:
  class, peak luminosity bound per band, longest duration D, light-curve function (plans 06, 09,
  11); `class_reach` (Design note 5); flux = L ÷ 4πd² × 10^(−0.4 A_band), with A_band from plan 07's
  `sightline` (`NoiseMode::Realised`, `Quality::Budget`, the modifiers of plan 09's `FeatureGas`
  along the segment, the caller's `NoiseCache` from `AlertContext`) from the apparent position to
  the observer, and `Sightline::in_band`. `AlertBand::XRay` takes `xray_optical_depth` and plan 11's
  `xray_luminosity` (Design note 14). Tests: reach grows monotonically as the limit falls and caps
  at the diagonal; flux of a pinned nova at three distances; the X-ray depth is unity at an A_V the
  doc comment states from σ_X, and is unchanged by adding hot coronal gas to a test sightline.
- **P12.T5.b One-shot classes.** Core collapse, Type Ia, stellar mergers, neutron-star mergers: walk
  the class's cells in the widened reach sphere (`CatalogueClassSource::cells_in_sphere`), take each
  entry's T, compute its arrival, test the window of Design note 6, emit a `Detection` with its
  `EventId`. Tests: completeness against `brute_force_detections` with exact set equality, for three
  observers and windows of a year and a century.
- **P12.T5.c Recurrent classes.** Luminous blue variables (plan 09's class, plan 06's giant
  eruptions), X-ray binaries, and the two accreting white dwarf classes through `scan_cell` using
  plan 11's scan marks only: slow hosts test the eruption times their marks carry, fast hosts ask
  the monotone phase for events in the emission interval of Design note 6. Dwarf novae and X-ray
  bursts are excluded from distant scans by reach and come through T5.e. Tests: completeness as in
  T5.b on a 2,000 ly sphere; `scan_cell` over all cells equals one call over the whole sphere; plan
  11's `scan::engine_calls()` stays at zero. Bench `awd_scan_marks_cell`.
- **P12.T5.d Census, features, the global list.** `alerts_in` with Design note 7; feature-level
  lists within reach through plan 09's `FeatureCatalogue::near`; the global list's entries through
  plan 10's `GlobalList::entries_with_class_members` (the centre's tidal disruptions and flares;
  dwarf cores and streams where they hold class members). Tests: a class over the limit returns no
  detections and says so; halving the reach brings it back; a window split in two gives the same
  detections as the whole; any order of classes gives the same result.
- **P12.T5.e Local events.** `local_events`: for the systems of an observed-mode range result,
  flares, glitches, magnetar bursts, FU Orionis outbursts, dwarf novae and bursts over each system's
  retarded interval, through plans 06 and 11. Tests: equals direct evaluation per system; adds under
  1 ms to a 50 ly query at the reference density (bench, a finding).

Files: `alerts/{mod,registry,reach,oneshot,recurrent,census,local}.rs`.

Shared test: re-observation. A nova detected from 1,000 ly at arrival t_a is detected from 1,500 ly
on the same line through its place of emission at t_a + 500 years to a millisecond (arrival is a
closed form, Design note 6), with the same `EventId`, emitted time and peak luminosity, and the
host's previous eruption is found from 10⁴ ly further still. Acceptance:
`cargo test -p hyperion-sim alerts` and the slow completeness tests.

### P12.T6 Protocol and server for the modes

Add `QueryModeDto` and `ObservedDto`; `mode` is `#[serde(default)]` and defaults to `now`, so
existing clients are unaffected; add the `resolve_system` kind under plan 04's extension rules
(`RequestBody::ResolveSystem`, `ResponseBody::ResolveSystem`, its string in `REQUEST_KINDS`): ID,
time, mode → one row, or a `RequestError` of `BadRequest` naming `system` when the sim's `resolve`
says `NoSuchSystem`. The server builds the observer, runs the query on the pool at
`Priority::Interactive`, and wraps the byte-bounded `SystemStars` cache that plan 06 instantiated as
the `StarsCache`. Plan 04's ±H check applies to the query time only, never to the emitted time. Run
`just gen-protocol`.

Files: `crates/hyperion-protocol/src/{galaxy,observe}.rs`, `crates/hyperion-server/src/` handlers,
`packages/protocol/src/generated/*`, `packages/protocol/src/index.ts`.

Tests: wire forms; an integration test asks the same sphere in both modes and gets the same IDs,
with `observed` present only in one; an observer outside the cube is `BadRequest` naming the field.
Acceptance: `just ci`.

### P12.T7 The Knowledge store

- **P12.T7.a Records and levels.** `ContactRecord`, `Sighting`, `KnowledgeLevel`, `ContactId`
  (sequential per universe), `KnowledgeStore::{record_sighting, acknowledge, contacts, view}` where
  `view` is the only way out and returns the degraded form for the record's level (Design note 10).
  Tests: a `Bearing` view has no host, distance, light age or position; two sightings of one
  `EventId` from different places share one contact; the level never falls.
- **P12.T7.b Persistence.** `<data_dir>/universes/<id>/knowledge/contacts.v1.jsonl`, in the
  directory plan 04 reserved, through its `UniverseStore`: append on change, load on open, an
  unknown version is `LoadKnowledgeError::UnsupportedFormat`, a torn last line is dropped with a
  `tracing::warn!`. Writes go through `spawn_blocking`. Tests: round trip; reopening a universe
  restores contacts and acknowledgements.

Files: `crates/hyperion-server/src/knowledge/{mod,record,store,persist}.rs`. Acceptance:
`cargo test -p hyperion-server knowledge`.

### P12.T8 The alert service

- **P12.T8.a Schedule and advance.** `AlertSchedule`, `Subscription`, and the pure core
  `advance(previous: Observer, next: Observer) -> Advance { due, rescan }` (Design notes 8–9).
  Tests: advancing in one step or in ten pushes the same detections once each; a backwards clock or
  a moved observer requests a rescan; nothing from the old position survives a jump.
- **P12.T8.b Scans on the pool.** Interactive classes as one `Priority::Interactive` job; background
  classes as `Priority::Bulk` jobs of a few hundred catalogue cells each under a `CancelToken`,
  merged only when all have finished (Design note 7); look-ahead renewal; the `ByteLru` of schedules
  under `SingleFlight`. Tests: a cancelled scan leaves the class `Pending` and delivers nothing; two
  subscriptions with one key share one scan; the cache is bounded.
- **P12.T8.c Into Knowledge.** Each due detection becomes a sighting; the notification carries the
  views of the contacts that changed, and the census per class (`complete`, `over_limit`,
  `pending`). A resolved contact appears first as a bearing if its flux crossed `detect` before
  `resolve`.

Files: `crates/hyperion-server/src/alerts/{mod,schedule,service,scan}.rs`.

Bench (server, Criterion): `awd_scan_cold` at Milky Way parameters with the default workers. The
brainstorm's "seconds" is the target: record the figure, and treat more than 10 s as a finding for
plan 11's scan marks. Acceptance: `cargo test -p hyperion-server alerts`.

### P12.T9 Protocol for alerts

Under plan 04's extension rules, as laid out under Provides: `subscribe` with
`SubscriptionTopic::Alerts` (observer, time, `SensorDto` of limits per band, class list, host
limit), answered by
`Subscribed { subscription, state: Alerts(AlertsSubscribed { census, contacts }) }` (contacts
already known in this universe); `alerts_observer`; `alerts_acknowledge`; `unsubscribe`; and
`ServerMessage::Notification`, which `RequestClient::handleServerMessage` must leave unconsumed so
that the subscription helper sees it. A request naming an unknown subscription is `BadRequest` with
`field: "subscription"`. `ContactDto`: `contact`, `level`, `kind` (only what the light curve's band
and shape justify at `bearing` level: `unclassified` until resolved, except the kinds a single band
identifies), `first_seen`, `last_seen`, `bearing`, `flux_w_m2`, `band`, `phase`, `acknowledged`,
and, only when resolved, `host`, `designation`, `light_age_yr`, `apparent_position`,
`present_position`, `curvature_error_ly`. Subscriptions end with the socket. Run
`just gen-protocol`; add `TestClient::next_notification()`.

Files: `crates/hyperion-protocol/src/alerts.rs`, server handlers, generated bindings,
`packages/protocol/src/index.ts` (`NotificationOf`, a subscription helper on `RequestClient`).

Note (drafted by rendering plan R03's R03.T1 for this plan's owner): the scene subscription needs
the envelope first, so R03.T5 builds `subscribe`, `unsubscribe`, `Subscribed`,
`SubscriptionTopic`, `SubscriptionState`, `ServerMessage::Notification`, `NotificationBody` and the
unknown-subscription refusal to this task's design, in `crates/hyperion-protocol/src/envelope.rs`
beside `ServerMessage`, with `Scene` as the only topic, and `TestClient::next_notification()` (in
`crates/hyperion-server/tests/common/mod.rs`), `NotificationOf<T>` and the subscription helper on
`RequestClient` (R03.T5.c).
This task then adds only the `Alerts` topic, state and notification body, `alerts_observer` and
`alerts_acknowledge`, and `alerts.rs` holds only the `Alerts` payloads. One departure (R03 Design
note 1): the subscription helper lives on `RequestClient` itself
(`packages/protocol/src/subscriptions.ts`), so `handleServerMessage` routes a `notification` to its
subscription and consumes it (returns `true`), dropping one for an unknown subscription, rather than
leaving it unconsumed for a helper outside. As built (R03.T5.a): the envelope's three enums,
`SubscriptionTopic`, `SubscriptionState` and `NotificationBody`, are tagged by `topic` in snake
case, and `ResponseBody::Subscribe` holds a `Box<Subscribed>`; the `Alerts` variants follow both. As built
(R03.T5.b): a topic implements `Handler::subscribe` in the server and merges its changes into its
`Pusher` as a `PendingPush` variant with its own `Merge`; requests that name a subscription
(`alerts_observer`, `alerts_acknowledge`) are routed by the connection, on the path R03.T8 builds
for `scene_cameras`.

Tests: wire forms; an integration test subscribes near a pinned recurrent nova, advances the
observer's time past an arrival and receives one contact at `bearing`, then moves the observer close
and receives the same `contact` at `resolved` with the pinned host; a JSON-level assertion that no
`bearing` contact contains a key named `host`. Acceptance: `just ci`.

### P12.T10 UX guide edits

Edit `docs/frontend/ux-guidelines.md`, on top of plan 05's edits (which add `yr`, E notation, the
direction names and the 3D conventions): under "Data states" add **Observed**: a value that is as
old as its light is shown with its light age available beside it (`LIGHT AGE 4210 yr`), and a
position extrapolated from it is Estimated (`~`); under "Numbers, units and time" add `kyr` and
`W/m²`, with small fluxes in plan 05's E notation (`3.20E-12 W/m²`, since B612 has no superscript
minus), and define a direction as azimuth and signed elevation, `047° +12°`, the azimuth following
the guide's existing bearing rule; under "Alerts" state that sensor detections are Advisory alerts
raised by the server, that they clear when the event fades below the sensor's limit, and name their
text form (`SENSORS TRANSIENT 047° +12°: unresolved`); add `OBS`, `LIGHT AGE` and `TRANSIENT` to
plan 05's nomenclature list; under "Graphs, schematics and spatial displays" add the bearing ray
(solid, ends at the display's edge with its label), the apparent-position tick joined to the present
position by a dashed line (dashes already mean prediction), and the rule that the mode (`NOW` or
`OBSERVED FROM`) is shown with the frame and time. Acceptance: `pnpm format:check` passes, `grep`
finds `LIGHT AGE`, `kyr`, `W/m²` and `OBSERVED FROM` in the guide, and the edits are one commit for
the owner to read, as plan 05 does for its own.

### P12.T11 Client: mode and observed readout

`SegmentMark` in plan 05's `spatial/marks.ts`, `drawList.ts` and `paint.ts` (solid or dashed,
clipped to the viewport, not pickable). `ModeControl` (`NOW` / `OBSERVED`, a display control and not
a ship command, single-key binding shown), `ObserverMark` on the chart with a "set observer to chart
centre" control and its coordinates in the readout. In observed mode the chart still draws present
positions; the selected system, and any system whose apparent offset exceeds four pixels, gets the
apparent tick and dashed joiner from the pure helper `apparentOffset`. The readout shows state
`AS OBSERVED`, light age, emitted time, the present position with `~` and the stated error, and an
em dash for present state, which the crew cannot know. The mode label sits beside the frame and the
time.

Files: under `apps/hyperion/src/renderer/src/`: `spatial/{marks,drawList,paint}.ts`,
`displays/galaxy/{LocalChartPanel,ChartControls,SystemReadout,useRangeQuery}.ts(x)`,
`components/ModeControl.tsx`, `lib/galaxy/{model,wire}.ts`, tests.

Tests: Vitest for the helpers and the readout in both modes; keyboard operation; no colour-only
signal. Acceptance: `pnpm test`, `just ci`, and by eye: a supergiant known dead is shown alive from
afar with its light age.

### P12.T12 Client: alert list and chart marks

`useAlerts` (subscribe on mount, move the observer with the display's time and observer, handle
notifications through `@hyperion/protocol`, mark everything stale on link loss), `AlertList`
(sortable by priority, time and system; position and total; acknowledge from the keyboard; Advisory
presentation; a census line per class in words: `NOVAE COMPLETE`, `ACCRETING WD PENDING`,
`MERGERS OVER LIMIT`), and chart marks through the general spatial view's `SegmentMark` and the new
`transient` `SymbolShape`: a bearing ray for an unresolved contact, the `transient` mark on a
resolved host inside the fetched sphere, a ray with a range label for one outside it. Selecting a
list row selects the host when it is resolved.

Files: `components/AlertList.tsx`, `lib/useAlerts.ts`, `displays/galaxy/alertMarks.ts`,
`spatial/{marks,symbols}.ts`, `test/FakeWebSocket.ts` (gains `serverNotifies(subscription, body)`),
tests.

Tests: list sorting and acknowledgement; a `bearing` contact renders no host text; `bearingRay`
geometry at the six named directions; reduced motion has nothing to reduce. Acceptance: `pnpm test`,
`just ci`, by eye against the pinned nova.

### P12.T13 Verification pass

Run every slow test and bench below, record the figures in the doc comments that own them, and add
the golden files: retarded observations of six pinned systems from two observers, and the detections
of a pinned observer over a pinned century. Acceptance: `just ci`, `just test-slow` and `just bench`
complete.

## Verification

- **Re-observation:** the same `EventId`, emitted time and properties from any distance, and one
  Knowledge contact for it (T5, T7).
- **Linearity:** an extrapolated jump lands on the star (T2).
- **Completeness per class:** exact set equality with brute force for every class, with split
  windows and any order (T5); a class over its limit returns nothing and says so.
- **Honesty:** curvature error figures match the brainstorm's; an unresolved contact leaks no host
  at the JSON level; present state is never shown in observed mode.
- **Time:** the source-horizon sweep; dead supergiants seen alive.
- **Benches:** `retarded` (≤ 0.7 µs; `retarded_from` ≤ 0.5 µs; ruling 143.4), `range_observed_50ly`, `alerts_interactive`
  (galaxy-wide, all interactive classes; target under 200 ms), `awd_scan_cold` (seconds),
  `lens_walk_26kly` (cold, ≤ 15 s on one thread and ≤ 5 s wall on the pool), `lens_renew_26kly`
  (≤ 10 ms) and `lens_walk_disc_5kly` (≤ 1 s) (ruling 143.3).
- **By eye:** the pinned nova on the `GALAXY` display, as a bearing and then as a host.

## Generator version

No change to generated output and no bump: observation and alerts only read. The goldens added here
pin that reading. The plan reserves nothing in the generator. On the wire everything is additive
under plan 04's convention: a defaulted `mode`, new request kinds, and the first use of
`notification`.

## Risks and open points

- **The observer stands in for a ship** (Design note 9), and Knowledge is per universe where the
  brainstorm means per crew. Both move to the session when it exists; the pure cores are written so
  that they do not change.
- **Resolution rule.** The brainstorm says "first as a bearing and a flux and only later as a
  resolved host" without saying what resolves it. A second flux limit is the smallest honest rule;
  sensor consoles will replace it.
- **"Alert" against the UX guide's alert classes** (Design note 11). Read as Advisory. If the owner
  wants detections outside the ship's alert system, T12's list becomes a contact list with the same
  columns and T10's wording changes.
- **Reach without extinction** can make a class `OverLimit` towards a dusty direction in which the
  sensor would in fact see few hosts. The rule stays deterministic and errs towards saying less.
- **The scan's cost rests on plan 11's scan marks** being drawn without the rest of the system. If
  the cold scan takes minutes, the fallback is a per-cell host cache in the same `ByteLru`.
- **Times before −H.** Plans 03–11 were written for ±H with retardation promised. T0's sweep is
  where any function that assumed |t| ≤ H is found.
- **`SystemSource`s have no segment method**, so lensing walks them by a chain of spheres. Dense
  features on a sightline are costly; the cell budget bounds it and the result says what was walked.
- **The stated error between 10 and a few hundred light-years of the centre.** The brainstorm states
  the curvature error for a disc star and for the nuclear disc. A nuclear-cluster member outside
  plan 09's Kepler regime drifts on a straight line although its orbit is far shorter than the light
  time from the disc, so its stated error reaches the cap of Design note 3, 2R: tens of light-years,
  about an arcminute from 26,000 ly. The model stays consistent, since an extrapolated jump still
  lands on the star, and the figure is honest; whether the Kepler regime should reach further out
  for retarded evaluation is a question for the brainstorm.
- **The X-ray band** (Design note 14) is one cross-section at one energy. A sensor console that
  wants soft and hard bands replaces the constant with a function of energy; nothing else changes.
- **FU Orionis outbursts** are treated as local events. The research notes route them through
  star-forming-region features; if plan 09 lists them at feature level they join T5.d instead.
- **T0, T1, T2 and T4 as built (lane `obs12a`, 2026-09-29, at `GENERATOR_VERSION` 14).** Done;
  T3 is held until the v15 commit. No generated output moves and no golden changes: observation
  and lensing only read. Names and shapes that differ from the sketches:
  - _T0._ `tests/source_horizon.rs::source_horizon_sweep` (slow, 11 s) sweeps what exists: the
    grid's seven layers (three systems of a solar-circle cell and of a bulge cell each), three
    layer-E remnants, a catalogue feature's members in bands A, C and E, the supernova entry as a
    value, and both event constructions over the horizon's first year. Nothing refuses or clamps
    a time before −H, so no owning module was widened. The displaced remnants (P08.T12), the
    centre's members (P09.T24–T28), the streams' and dwarf cores' members (plan 10) and the
    catalogue classes' grids (P09.T32–T35) are not placed yet: **the task that places each adds
    its row to the sweep.** Two things found on the way, neither a time limit: plan 03's
    `query::position_at` panics for a feature member in a galaxy with kinematic tables
    (`kinematics::draw` needs a density component), so observation moves every record through
    `Drift::of_record`, which resolves a member's own velocity; and plan 06's range brief
    (`BriefModel`, P06.T38.e) chooses its route for [−H, +H] only. It agreed with
    `SystemStars::brief_at` at −(H + L) on every system of the sweep, but nothing guarantees it
    there, so observed mode reads `SystemStars::brief_at` (T3 and T6 must too).
  - _T1._ `units::Arcseconds` and `consts::RADIANS_PER_ARCSECOND`; `WattsPerSquareMetre` already
    existed. `observe::{Observer, BuildObserverError, Trajectory, Motion, Drift, Retardation,
retarded, retarded_exact_linear, light_time, CurvatureError, curvature_error,
extrapolate_to_present}`. `Trajectory` gains `motion() -> Motion` (`Drift` by default,
    `Followed` for an orbit followed in full, whose stated error is zero: the plan's "zero for a
    Kepler member" until P09.T28 builds one) and `position_with_residual_at`. The trajectory of a
    `SystemRecord` is `Drift::of_record(galaxy, record)`, with `Drift::of_member` and
    `Drift::through(position, at, velocity)` besides. `Drift` carries v × Δt as an exact pair and
    rounds the offset once, so its error does not grow with the span (plan 03's `position_at`
    rounds the product first: a few metres inside ±H, tens of metres at light times of 10⁴
    years); `Retardation` keeps what the apparent position's rounding dropped, so an
    extrapolation lands on the line to one rounding. `Retardation::light_age()` is a `Span`, not
    `Seconds`, and `observed()` and `motion()` are added. `curvature_error(galaxy, observer,
r)`: the position is the observer's, and the angle is the length over the distance to the
    apparent position; `CurvatureError` keeps the plan's name although it is a stated bound, not
    an error type; R is the apparent position's **spherical** galactocentric radius with the
    in-plane `v_c(R)`, so a halo star above the axis is not given zero (provisional). Design
    note 1's residual: the bound (v ÷ c)² × light time is exact and a radial source reaches it,
    ten days at 220 km/s from 50,000 ly, under seven months at 1,000 km/s within 50,000 ly, and
    2.5 years at 1,000 km/s across the whole cube (the note's "under seven months" is at 50,000
    ly). "`extrapolate_to_present` equals `position_at` to 1 m" is tested against the line
    (`Drift::position_at`); plan 03's `position_at` lies within 5 m of it inside ±H. Acceptance:
    `cargo test -p hyperion-sim observe::` (the curvature tests are in `observe::error`). Bench
    `observe/retarded`: **1.12 µs** (2026-09-29, load average 12), against the plan's "tens of
    nanoseconds": a finding. The cost is the exact-pair drift, whose fused multiply-adds are
    `libm`'s software `fma` (six an evaluation, three evaluations a step); the intermediate
    evaluation at t − s₀ only feeds a distance and could take plan 03's single rounding, which
    would cut about a third. Left for P12.T3's `range_observed_50ly`, which measures what it
    costs a query.
  - _T1, provisional (finding)._ **Design note 3's cap of 2R is not a bound** (science check):
    the line leaves while the orbit stays within R of the centre, so past an orbital phase of
    about 2.5 radians over the light's age the gap grows as v s + 2R (at 10 ly after 10⁵ years
    about 36 ly against the stated 20). It touches only sources within a few tens of light-years
    of the centre outside the Kepler regime. Built as the note says;
    min(½ a s², `v_c` s + 2R) would bound it, for the owner's ruling. _Resolved by ruling 143.2
    (lane `obs12b`, below)._
  - _T1, provisional (finding)._ **The nuclear-disc curvature figure misses its window**: the
    model's potential gives `v_c` = 76 km/s at 100 ly (94 at 30 ly, 93 at 200 ly), against the
    brainstorm's 100 km/s, so the maximum from a corner of the cube is 7.5″, not 10–16″. The
    test pins 7.5″ ± 10% and checks that the formula gives the brainstorm's 13″ at 100 km/s.
    Ruling 143.1: the acceptance is "the formula at the model's `v_c`"; plan 02's nuclear
    potential is raised in its next revision (its own bump), which moves the pin. _Closed by the
    joint revision (lane `pot02`, for version 16; plan 02, R26):_ the nuclear cluster normalised
    inside the centre's reach and the nuclear disc's inner part bring `v_c` at 100 ly to
    100.1 km/s, and the maximum to 13.05″; the test now holds the plan's 10–16″
    (`curvature_error_of_the_nuclear_disc_follows_the_models_circular_speed`) and still checks the
    brainstorm's 13″ at 100 km/s.
  - _T2._ `observe::{ObservedSystem, observe_hit, summary_observed, bearing, Bearing,
BearingFrame, AXIS_FRAME_RADIUS_LY}`. `bearing(from, to) -> Option<Bearing>` takes no galaxy
    (the directions are geometry) and is `None` for coincident points; `Bearing::frame()` says
    `GalacticX` within a light-year of the axis. `observe_hit` takes the present from the
    system's own line, not from the hit (plan 03's drift rounds it differently), so that it and
    `retarded` give the same bits; it resolves a feature member again for its velocity, which
    costs an interior build: T3 should hand velocities or a member cache to the observed query.
    After the death's light arrives the test checks for a neutron star or black hole whose age
    since T is t − d ÷ c − T: the brief has no supernova state, which arrives as a transient with
    the catalogue class and T5. Acceptance: `cargo test -p hyperion-sim observe::`, since the
    bearing tests are in `observe::bearing`. The supergiant test re-ages a real layer-E record so that it dies 20,000 years before
    the epoch (a search of 320 layer-E cells of the solar circle found no death in 40,000 years:
    about one such cell in 3,000 has one); the "born a century ago" test re-ages a layer-B
    record. The extrapolated jump lands under a metre from the star's line, and plan 03's
    `position_at` lies within 5 m of it. **Deferred:** "several hundred dead supergiants seen
    alive from 26,000 ly (slow; counted from the catalogue's supernova class)" waits for the
    supernova class's grid (P09.T32–T35); no stand-in was built. A centre member's extrapolation
    "uses the orbit" once P09.T28 exists: `Retardation` will need the orbit's elements then.
  - _T4.a._ `galaxy::query::cells_along_segment` in `query/segment.rs` steps through the slabs
    of cells across the segment's dominant axis and takes the rectangle the capsule reaches in
    each, Amanatides and Woo's traversal widened to a tube, so a wide tube costs its
    cross-section and not a cube per step; each cell is kept on the exact segment-to-box distance.
    The result is collected and sorted (by the projection of the cell's centre, then by cell).
    The brute-force test uses the walk's own distance test and so checks the candidate set; a
    second test checks the distance test against dense sampling. Acceptance:
    `cargo test -p hyperion-sim segment_walk lensing`, since the walk's tests are in
    `galaxy::query::segment`.
  - _T4.b._ `lensing::{LensSightline, LensQuery, LensQueryBuilder, BuildLensQueryError,
LensEvent, LensResult, LensCensus, FindLensesError, lenses_along, magnification_at,
point_lens_magnification, einstein_angle, lens_mass_at, DEFAULT_MAX_IMPACT,
DEFAULT_LENS_CELL_BUDGET}`. The query is built (`LensQuery::builder`) and takes a
    `mass_floor`; its window must lie in ±H. An event is a lens whose least impact within the
    window is under the reach; `peak` is the closest approach clamped to the window, and the
    event keeps the unclamped one, so `magnification_at(event, t)` needs no galaxy or sightline.
    `LensQueryError` is `FindLensesError` (the rules' verb-object naming). A lens's mass is the
    sum of its stars' `state_at(t).mass()` at the retarded time, or a free-floating object's
    mark; plan 11's `SystemStars::system_mass_at` waits for P11.T4 and the call switches to it
    then. Sources are asked on a chain of 64 spheres, twice a year apart, to read each hit's
    velocity, since `SystemSource` has none, and a grid system a source `suppresses` at the
    lens's retarded time is left out, as the range query leaves it out. The budget is checked
    against the tube's volume in cells before any is listed. The observer is at rest, so its own
    motion adds nothing to the relative proper motion. "Of order 10⁻⁶" is tested as τ within a
    factor of three of it. Bench `lensing/lens_walk_26kly` (the Sun to Baade's window, a year): **48 s** in a galaxy whose
    systems move (the cone below) and **0.55 s** in one whose systems keep their epoch positions
    (the thin tube), 2026-09-29 at load average 12. A finding: retarded lensing to the bulge is a
    background job, not an interactive one.
  - _T4.b, provisional (finding)._ **Design note 13's tube misses the lenses.** Cells hold epoch
    positions and a lens is taken at its retarded time, up to the source's light time before the
    window, so a tube of the reach's Einstein radius plus `PAD_SPEED` × window holds lenses of
    epoch positions that have since moved about 9 ly away, and misses those on the line then. As
    built, for a galaxy whose systems move, the tube adds the layer's padding speed times the
    time from the epoch to the lens's retarded time: a cone, 87 ly across at 26,000 ly for layers
    A–D and 260 ly for layer E, about 4.6 × 10⁵ cells to the bulge, which the cell budget bounds.
    The optical-depth test runs in a galaxy without kinematic tables, where nothing moves and the
    thin tube is exact: **τ = 8.1 × 10⁻⁷** (81 lenses, ±11%) towards 3–5° south of the centre from
    10⁴ sightlines (U = 100), inside the window and at the low end of Mróz et al. 2019's 0.6–1.4 ×
    10⁻⁶ for that field. The test walks 6.9 × 10⁷ cells in 24 minutes on four threads at load 12. The
    owner's ruling is needed on whether retarded lensing is worth the cone's cost. _Ruled by 143.3:
    the cone is kept (lane `obs12b`, below)._
  - _Centre members (after the rebase onto `d5330c7`, which added P09.T24–T27)._
    `Drift::of_record` returns `Result<Drift, TraceMotionError>` and refuses a member of the
    galactic centre (`SystemOrigin::CentreMember`) with `TraceMotionError::CentreOrbitNotBuilt`:
    inside the sphere of influence it follows its Kepler orbit, and `regime_of` and the orbit are
    P09.T28's, so no line is given for any centre member until then (provisional).
    `observe_hit` and `summary_observed` return that error; `lenses_along` returns
    `FindLensesError::SourceMotionNotTraced` for a centre member as the source and leaves a
    source's centre members out as lenses (`lens_mass_at` is `None` for one). Tests:
    `observed_centre_member_is_refused_until_its_orbit_is_built`,
    `lensing_refuses_a_centre_member_as_its_source`. P09.T28 replaces the refusal with the orbit
    (`Motion::Followed`) and adds the centre members' row to the T0 sweep.
  - _For P12.T13._ Its goldens should also pin one sightline's lens events (peak, impact, `t_E`,
    closest approach), one segment walk's keys and a few bearings, whose angles pass through
    `math::atan2` (determinism audit).
- **Ruling 143's points 2–4 as built (lane `obs12b`, 2026-09-29, at `GENERATOR_VERSION` 15).** No
  generated output moves and no golden changes: the curvature bound is text and a stated figure,
  the lens walk finds the same lenses faster, and the drift's product error is the same bits.
  Timings are at load averages of 5–12 on the 8-core development machine, so they read high.
  - _143.2, the curvature bound._ `curvature_error` states min(½ a Δ², |v| Δ + 2 `r_apo`) with
    Δ = |t_emitted − epoch|, R and a = `v_c²` ÷ R at the epoch position (`Retardation`'s line
    carried to the epoch, `line_at_epoch`, crate-private), |v| the source's own speed, and
    `r_apo` the largest r with Φ(r, 0) ≤ E by bisection, found only where the linear term can win
    (the quadratic is compared with |v| Δ + 2R first). E is Φ(R, z) + ½ |v|² from the (R, z) grid,
    or Φ(|x|, 0) + ½ |v|² without it; E ≥ 0 states the quadratic alone. Where it is an estimate,
    as the doc comment records: the quadratic takes a at the anchor, not the orbit's largest pull
    (an eccentric orbit's pericentre pulls harder); a is the in-plane `v_c²` ÷ R at the spherical
    radius, not |∇Φ|; Φ(r, 0) as the least potential on each sphere holds for an oblate galaxy,
    and without the grid the energy above the plane is low. Tests: never above the bound (with
    `r_apo` from an independent outward scan) and never below the exact circular gap, for ten
    radii from 0.5 to 60,000 ly and seven spans to the source horizon; the disc and nuclear-disc
    figures are unchanged (the quadratic governs); an unbound source states the quadratic; a star
    4 ly away seen at +1,000 years is stated over Δ = 996 years. The science check's case, 10 ly
    after 10⁵ years, is 49.6 ly of true gap at the model's `v_c` (it took 150 km/s and 61 ly) and
    73.4 ly stated, against the old cap's 20.
  - _143.3, lensing._ `lensing::{LensWalkPlan, LensCandidates, LensCandidateChunk,
lens_candidates, lenses_among}` and `FindLensesError::NotCoveredByCandidates`. `lenses_along` is
    `lens_candidates` for its own window then `lenses_among`, with the same events and census as
    before. The cold walk is planned (`LensWalkPlan::new`: source, geometry, cells, budget), walked
    in chunks of cells (`walk`, a pure function; the sim spawns no thread) and merged (`finish`:
    candidates by ID, each once), so the result does not depend on the split or the order. The
    candidates of one (observer position, source) serve any window inside the query they were
    walked for: a grid system is kept if its line passes within `max_impact` times the largest
    Einstein radius on the sightline plus 0.05 ly and a twentieth of the lens's and the source's
    motion over the span; sources (features) are asked again for each window, since their lines
    are read from each window's spheres. The pad of layers A–D and the substellar layers is a
    bound on the escape speed beyond each spherical radius, from a table of the potential's
    escape speed at 177 radii (eight an octave, 2⁻⁴ to 2¹⁸ ly) by 13 polar angles, maximised
    outward and raised by 2%, and capped at `PAD_SPEED`; a piece takes it at the least radius its
    lenses' epoch positions can have, and layer E keeps `UNBOUND_PAD_SPEED`. The cone's radius
    also now counts the lens's distance growing with its motion (δ ≤ β (|t| + far + reach) ÷
    (1 − β)) and the source's shift over the window, both of which it had left out, by parts in a
    thousand. Prefilter: the epoch position against the cone at its own distance with its own
    speed bound, then a two-step `f64` estimate of the retarded position against the kept reach
    with a margin of 0.01 ly plus the estimate's error, then the exact step. Figures (Sun to
    Baade's window, a year): **300,746 cells** against 4.6 × 10⁵ at 1,000 km/s, 3.3 × 10⁶
    systems examined; cold walk **15.2 s** on one thread (load 6–11), of which generating the
    cells alone is 9.3 s (`lens_cells_26kly`); **3.2 s** wall on seven threads (the pool's
    default, cores less one); renewal of a year from two centuries' candidates (5 of them)
    **3.4 µs**; the thin tube in a still galaxy 0.47 s; a cold disc sightline of 5,000 ly
    **31.5 ms**. The cone's floor is cell generation, which is plan 03's; the remaining 6 s are the
    examination of 3.3 × 10⁶ systems, about 1.8 µs each, most of it presumably the velocity draws
    of those inside the cone at their own distance (not profiled: no profiler on the machine). Tests: the renewal
    equals a fresh walk for random windows (still galaxy, pinned moving lenses); chunks in any
    split and order merge to one walk; the kept reach holds every lens that any window inside the
    span finds, for 3,000 lenses at up to 1,000 km/s past a moving source; and, slow
    (`lensing_cone_equals_a_brute_force_walk_in_a_moving_galaxy`, 53 s), the cone's 73 lenses of
    a 1,500 ly sightline at +500 years with a reach of 3 × 10⁵ equal a brute-force walk of every
    system in a box about it (44,642), the escape-speed cone walks 2,551 cells against 3,113 at
    1,000 km/s, and renewals from two centuries' candidates equal fresh walks and the brute force;
    slow (`lensing_escape_envelope_bounds_the_draws_cut`, 7 s), the table bounds the draw's cut at
    10⁵ random points, by at least 2.4 × 10⁻⁵ of it. "Interactive" is dropped for the bulge.
    **Not built: the background subscription.** Plan 12 has no server task for lensing (it is
    "API and tests only" in the sim); the subscription that walks a monitored sightline on the
    `CpuPool` in chunks, keeps its candidates in a `ByteLru` keyed by (universe, observer
    position, source, reach, layers) and renews them each window, rescanning after a jump, belongs
    to the task that brings lensing to the server (the sensor consoles), which should take this
    API as its core.
  - _143.4, the retarded step._ `math::two_product(a, b) -> (product, error)`, Dekker's product
    with Veltkamp's split, bit for bit `(a × b, mul_add(a, b, −(a × b)))` for every input: Dekker
    where it is exact (normal factors under 2⁹⁹⁵, the product in [2⁻⁹⁶⁸, 2¹⁰²¹)), +0 for a zero
    factor, and `libm::fma` for the rest. Pinned against `libm::fma` over the drift's ranges (8 ×
    10⁵ pairs) and the whole exponent range with subnormals, the range's edges, ±0, infinities
    and NaN (4 × 10⁵ pairs and every pair of 17 special values), on x86-64 and under wasmtime on
    `wasm32-wasip1` (those tests and `observe::retarded`, `observe::error`). The drift takes it
    for both of its products. `observe::retarded_from(observer, source, present)` is public, bit
    for bit `retarded` given the present. Plan 03's single rounding is not taken (P12.T3 decides).
    Bench (load 9–12, noisy): a drift evaluation 188 ns against 295 ns with `libm::fma`;
    `observe/retarded` at the epoch 0.62 µs, at +300 years 0.93 µs, `retarded_from` 0.56 µs,
    against 1.12 µs before at load 12. At low load the ruling's 0.7 and 0.5 µs are likely met but
    not measured here.
- **T3 and T7.a–b as built (lane `obs12c`, 2026-09-30, at `GENERATOR_VERSION` 15).** No generated
  output moves and no golden changes: the observed query only reads, and Knowledge is the server's.
  - _T3._ `QueryMode` lives in `galaxy/query/mode.rs` beside `range_query_observed` (the builder
    takes it) and is re-exported as `observe::QueryMode`; `RangeQuery::mode()` and
    `RangeQueryBuilder::mode(QueryMode)`, default `Now`. `build` refuses an observer outside the
    root cube with a new `BuildRangeQueryError::ObserverOutsideRootCube`, so the query carries a
    valid observer; the server's `refused_query` maps it to the field `mode` until T6 names the
    wire's field. `RangeResult::observed() -> &[ObservedSystem]`, empty in `Now` and from
    `range_query`. `observe::{StarsCache, NoStarsCache, stars_of}` in the new
    `observe/stars_cache.rs`: a cache must lend what `stars_of(galaxy, interiors, record)` builds,
    which for a feature member is `MemberRecord::stars` at its cluster's composition, not plan 06's
    `SystemStars::generate` (that reads the record's density component, which a member lacks: a
    debug assertion, and a wrong composition in release). A rogue planet has no stars, so its row is
    observed without them and has no brief then; the cache is never asked for it.
    `range_query_observed` returns `Result<RangeResult, TraceMotionError>`: a centre member found
    refuses the whole answer with obs12a's `CentreOrbitNotBuilt` until P09.T28, before any stars are
    asked for (none is found yet, since no centre `SystemSource` exists; the test hands the black
    hole in through a test source). It reads each row through `SystemStars::brief_at`, as T0
    required. The hit carries no velocity, so a feature member's line is traced with
    `Drift::of_record_in` (crate-private, over plan 09's `FeatureInteriorCache`) and one
    `KeepInteriors` per call: each feature's interior is built once more per query, not once a
    member (the source that found the members built it too). A server wanting its long-lived
    interior cache there needs a variant that takes it (T6). Tests: `query::mode` (100 random grid
    queries over the disc, radii 1–8 ly, times over ±H, observers anywhere in the cube: the same
    systems, census and statistics as `range_query`, and every row its own `observe_hit`; one in ten
    with the substellar layers; 2 s in a debug build) and, slow,
    `tests/observe_query.rs::observed_query_agrees_with_now_among_feature_members` (20 queries
    inside the first feature with members within 3,000 ly of the Sun, in a full-potential galaxy
    with plan 09's member source).
  - _T3, bench (finding for ruling 143.4; provisional; ruling deferred)._
    `query/range_observed_50ly` (the 50 ly Sun-like query observed from its centre, cells and stars
    warm; 917 systems) **4.19–4.79 ms** in two runs against `range_50ly_warm` 1.13 ms and
    `range_50ly_cold` 9.45 ms (2026-09-30, load 9–11): the observation costs 3.1–3.7 ms, about 4 µs
    a system, which is **over 20%** of the plain query however it is compared (32–39% of the cold
    query, three to four times the warm one). Measured apart over the same 917 systems (a scratch
    bench, not kept): `brief_at` at the emitted time 1.62 ms (44%), `curvature_error` 0.98 ms (27%),
    `retarded` 0.65 ms (18%), the line and the existence test under 0.02 ms. Plan 03's single
    rounding for the step's intermediate evaluation, cutting about a third of `retarded`, would save
    about 0.2 ms, some 5% of the observed query; the brief and the stated error are the larger
    costs. `range_observed_50ly_cold` (no cell or stars cache) is **240 ms**, nearly all of it
    building every system's `SystemStars`: without a stars cache observed mode is 25 times the plain
    query, so T6's server must hand it its `SystemStars` cache. Ruling 143.4's condition (observed
    mode more than 20% over the plain 50 ly query) is met, so its single-rounding decision falls due
    here, before T13's goldens; the lane's brief was to report it and leave the step alone, which it
    does. The measurement argues against taking it: the step is 18% of the observation, the saving
    about 5% of the query, and the brief and the stated error are the costs worth a lever.
  - _T5.a, ahead of it._ Only `alerts::AlertBand { Photometric(Band), XRay }` exists, in the new
    `sim/alerts/mod.rs` (`Band` is `galaxy::gas::ccm::Band`), because `Sighting` records its band;
    nothing else of T5.a is built.
  - _T7.a._ `knowledge::{KnowledgeStore, ContactId, ContactRecord, KnowledgeLevel, Sighting}`, the
    views `ContactView`, `BearingContact` and `ResolvedContact`, `Acknowledgement` and
    `AcknowledgeContactError`. `ContactId` wraps a `NonZeroU32` from 1.
    `KnowledgeStore::record_sighting(event, sighting) -> ContactId`,
    `acknowledge(contact) -> Result<bool, _>` (whether it changed; acknowledgement is sticky and
    never hides the contact), `contacts()` in number order and
    `view(contact) -> Option<ContactView>`. The host is not stored apart: it is the `EventId`'s
    subject, so `ContactRecord` holds the event, the sightings in the order recorded, the level and
    the acknowledgement, and exposes nothing outside the crate.
    `Sighting::new(observer, retardation, band, flux, level)`: the sighting says which level it
    supports (`Resolved` when its flux reaches the sensor's resolve limit, which T8.c decides from
    T5.a's `SensorHorizon`), and it keeps the apparent position and the position extrapolated to the
    observer's time (`extrapolate_to_present`), which only a resolved view shows; its bearing is
    `bearing(observer, apparent position)` and is `None` only for coincident points. A view's
    bearing, flux, band, light age and positions are the latest sighting's (the greatest observer
    time, the last recorded on a tie); `first_seen` and `last_seen` are observer times. `kind`,
    `phase` and `designation` of T9's `ContactDto` are not in the view yet: they need T5.a's types.
    `ContactRecord` is `pub(crate)`, not public as Provides lists it: nothing outside the store
    takes one, and its `Debug` form (and `KnowledgeStore`'s) leaves the event out so that a log line
    cannot name an unresolved host. The tests' fixtures are in `knowledge/testing.rs`.
  - _T7.b._
    `knowledge::{PersistedKnowledge, LoadKnowledgeError, SaveKnowledgeError, KNOWLEDGE_FORMAT}` and
    `UniverseStore::knowledge_dir(id)`. `PersistedKnowledge::open(store, universe)` loads on a
    blocking task; `record_sighting` and `acknowledge` run on one that takes the file's lock,
    appends and syncs the line, and only then applies the change, so memory never runs ahead of the
    disk and a dropped future leaves both in step; `read(|store| ..)` lends the store for its views.
    The file's first line is `{"format":1}`, then one line per change (`{"kind":"sighting",..}` with
    the event's text form, exact positions and times, `flux_w_m2`, `band`, `level`;
    `{"kind":"acknowledged","contact":n}`), replayed in order and checked to fit (a new contact must
    be the next number). A header of a later format, or a `contacts.vN.jsonl` with N > 1 beside the
    file, is `UnsupportedFormat` (a header of format 0 is `MalformedLine`); an unterminated last
    line is dropped with a `tracing::warn!` and the next append truncates it; any other bad line is
    `MalformedLine`. The `knowledge/` directory is created on the first change and never the
    universe's own, so a universe with no save gets no directory. A change that panics holding the
    file's lock poisons it, and later changes are `SaveKnowledgeError::Poisoned` until the
    universe's Knowledge is opened again, since the file may then hold a change the store does not.
    A universe's file has one writer: two `PersistedKnowledge`s opened on it would cut each other's
    lines, so the owner (T8) keeps one per universe and clones it. Nothing wires the store into
    `AppState` yet. Every sighting is kept in memory and on disk; T8 should decide whether a
    long-lived recurrent contact needs its sightings thinned.

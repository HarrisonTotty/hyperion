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
everything. T5 follows T2; its subtasks run in order. T6 (protocol for modes) follows T3, and T6.b
(the summary's mode) follows T6 and lands before T11. T7 (Knowledge) is independent of T5 and can
start after T0. T8 follows T5 and T7. T9 follows T8. T10 (guide) can be written at any time and must
land before T11 and T12, which follow T6 and T9 and are independent of each other. T13 closes.

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
  11); the central black hole's flare row reads plan 09's
  `flares::{BIN_SECONDS, LOOK_BACK_BINS, MAX_DURATION, MAX_PEAK_LUMINOSITY_WATTS}` and
  `CentreFlare::luminosity_watts_at` (P09.T30.a as built: plan 09's Risks, "P09.T30.a as built"),
  and its tidal disruptions' bound, duration and light curve are P09.T30.b's; `class_reach` (Design
  note 5); flux = L ÷ 4πd² × 10^(−0.4 A_band), with A_band from plan 07's
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

- **P12.T6.b The summary's mode** (added 2026-10-15 by `decision-gg-port-d0.md` §6.2, from T6's
  record below; built in `lane/70-p12-t6b-summary-mode`, recorded at the end of Risks). T6 as
  built is the range query's mode and `resolve_system`; this
  subtask puts the mode on plan 06's `system_summary`, in the shape the Protocol section above
  already gives it, so it is additive and `PROTOCOL_VERSION` stays 2. A defaulted
  `mode: QueryModeDto` on `SystemSummaryRequest` (`#[serde(default)]`, written only when observed,
  as on `SystemsInRangeRequest`), and an optional `observed: ObservedDto` on `SystemSummaryDto`,
  written only in observed mode. In observed mode the server reads the system through
  `observe::summary_observed` over the stars its `SharedSystemCache` already builds for the
  summary: the stars, existence and age are `summary_at` the emitted time, `time` still echoes the
  request, and `observed` carries the retardation with its stated error (`curvature_error`), as a
  range row's does (`convert::observed_dto`). The mode's observer is checked by
  `convert::observe::query_mode` (`bad_request` naming `mode`), after the summary's own checks; a
  centre member stays refused as `system_summary` refuses it now. Two consequences to carry:
  `SystemSummaryRequest` derives `Eq` and `Hash`, which `QueryModeDto` cannot (its position holds
  `f64`s), so those derives go (nothing hashes the request); and in observed mode the summary's
  `age_myr` and `existence` are the emitted time's, unlike a range row's, so `SystemSummaryDto`'s
  docs ("at `time`") change and T11 subtracts no light age from them. Files:
  `crates/hyperion-protocol/src/stellar.rs`, the server's `system_summary` handler and
  `convert/stellar.rs`, `packages/protocol/src/generated/*`. Tests: the wire forms in both modes and
  a summary request of plan 06's form read as `now`; over a socket, one system's summary asked in
  both modes has the same `system` and `time`, `observed` only in the observed answer, and its
  stars are `summary_observed`'s; an observer outside the cube is `bad_request` naming `mode`.
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
  restores contacts and acknowledgements. How durable the directory and the appends are on each
  platform is plan 04's P04.T17.b.

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

Note (drafted by rendering plan R03's R03.T1 for this plan's owner; accepted 2026-09-30 by a
delegated decision, amended to name `subscription_ended`): the scene subscription needs
the envelope first, so R03.T5 builds `subscribe`, `unsubscribe`, `Subscribed`,
`SubscriptionTopic`, `SubscriptionState`, `ServerMessage::Notification`, `NotificationBody`,
`ServerMessage::SubscriptionEnded` (`subscription_ended { subscription, error }`: a subscription
also ends when its topic fails, the place freed and no retry, not only with `unsubscribe` or the
socket) and the unknown-subscription refusal to this task's design, in
`crates/hyperion-protocol/src/envelope.rs` beside `ServerMessage`, with `Scene` as the only topic,
and `TestClient::next_notification()` (in `crates/hyperion-server/tests/common/mod.rs`),
`NotificationOf<T>` and the subscription helper on `RequestClient` (R03.T5.c).
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
clipped to the viewport, not pickable) (its width and dash in CSS px, drawn by `paint.ts` at
R07.T16.f's line scale; decision-thin-line-contrast). `ModeControl` (`NOW` / `OBSERVED`, a display
control and not a ship command, single-key binding shown), `ObserverMark` on the chart with a "set
observer to chart centre" control and its coordinates in the readout. In observed mode the chart
still draws present positions; the selected system, and any system whose apparent offset exceeds
four pixels, gets the apparent tick and dashed joiner from the pure helper `apparentOffset`. The
readout shows state `AS OBSERVED`, light age, emitted time, the present position with `~` and the
stated error, and an em dash for present state, which the crew cannot know. The mode label sits
beside the frame and the time.

Files: under `apps/hyperion/src/renderer/src/`: `spatial/{marks,drawList,paint}.ts`,
`displays/galaxy/{LocalChartPanel,ChartControls,SystemReadout,useRangeQuery}.ts(x)`,
`components/ModeControl.tsx`, `lib/galaxy/{model,wire}.ts`, tests.

Tests: Vitest for the helpers and the readout in both modes; keyboard operation; no colour-only
signal. Acceptance: `pnpm test`, `just ci`, and by eye: a supergiant known dead is shown alive from
afar with its light age.

_Amended 2026-10-15 (`decision-gg-port-d0.md` §6.1–§6.2, from T6 as built):_ an observed row's
`age_myr` and `velocity_km_s` are the present's, as its `position` is (Design note 4), so in
observed mode the readout shows the age then, `age_myr − light_age_yr ÷ 10⁶`, as an Observed value,
and `NOT YET FORMED` where that is below zero; and `—` for the velocity, which is present state the
crew cannot know, until a task puts the velocity at emission on the wire. A server built before T6
ignores `mode` and answers `now`, so the readout never labels a row `AS OBSERVED` unless the row
carries `observed`, whatever was asked. The one system's stars as observed come from T6.b's
summary.

- **P12.T11.b The HR page's mode** (added 2026-10-21 by main's scoping, from T11's deferred
  finding (2); built in `lane/73-p12-t11b-hr-mode`, recorded at the end of Risks). The `HR DIAGRAM`
  page plots the briefs of the chart on show, which in observed mode are the primaries when the
  light left, and did not say so. Under its plot, before the counts, where the chart's furniture
  stands, it now reads the answer's time and mode as the furniture does, stale with the answer:
  `UT` and `MODE NOW`, or `FRAME GALACTIC`, `UT` and `MODE OBSERVED FROM` with the observer's
  `RADIUS`, `ANGLE` and `HEIGHT`, from `answeredMode` and the result's `timeYr` through the
  readings the two pages now share (`chartReadings.ts`), and never from the mode being chosen.
  Files: under `apps/hyperion/src/renderer/src/`: `displays/galaxy/`'s `chartReadings.ts`,
  `HrDiagram.tsx`, `GalaxyPages.tsx` and `LocalChartPanel.tsx`, `styles.css`'s `.hr-diagram`
  block, tests. Tests: Vitest for the shared readings, for the page's line in both modes, on the
  galactic axis and stale, and for the display's page across `OBSERVED` and `NOW` and for an
  answer asked observed whose rows came without their observation. Acceptance: `pnpm test`,
  `just ci`, and main's look at 1920 × 1080 and 1280 × 720.
- **P12.T11.c The observed chart's look follow-ups** (added 2026-10-21 by main's UX decision on
  T11's landing look; built in `lane/74-p12-t11c-compact-refusals`, recorded at the end of Risks).
  The look found a refused entry on the compact page pushing the mode row onto the furniture in
  `OBSERVED`, and smaller findings: `OBSERVER TO CENTRE` sharing `CENTRE` with `CENTRE CHART`, a
  0.25 rem tick no wider than the selected joiner, a legend sample too short for two dashes, a
  system's label over the observer's eye, `AGE`'s `AT UT` wrapping at 1920 × 1080, and the
  `0.01-500 ly` hint. On the compact page a refusal now stands in its field's hint's place on the
  field's row, its name hidden from sight alone, and never wraps; the command is
  `OBSERVER TO CHART CENTRE`, on one row with the mode while a chart is on show; the tick is
  0.5 rem and the sample 2.75 rem; marks' labels keep off the eye; `AGE` reads
  `· AT UT <time> yr`, wide as observed with its time in a fixed slot; and the hint reads
  `0.01 to 500 ly`; with the guide's sentences on angles, grouping, the observer's mark, `AT` and
  the command. Files: under `apps/hyperion/src/renderer/src/`: `displays/galaxy/`'s
  `ChartControls.tsx`, `SymbolLegend.tsx` and `SystemReadout.tsx`, `spatial/`'s `drawList.ts`,
  `labels.ts` and `SpatialView.tsx`, `styles.css`, tests; the guide. Tests: Vitest for both
  refusals at once on a compact and a stacked page, the stylesheet's compact rules, the command,
  the tick, the sample's dashes, the labels about the eye, `AGE` in both modes and the hint.
  Acceptance: `pnpm test`, `just ci`, and main's look at 1920 × 1080 and 1280 × 720.
- **P12.T11.d One stale S per value on the spatial furniture** (added 2026-10-22 by main's UX
  decision on T11.b's landing look; built in `lane/75-p12-t11d-stale-per-value`, recorded at the
  end of Risks). The look found the chart's furniture marking a stale answer with one `S` after the
  scale bar, read `2 ly S` as if the camera's live scale were stale, while the `HR DIAGRAM` page's
  line, in its own copies of the furniture's readings, marked each value, and the page's counts
  moved as their `S` came. `Reading` now keeps a 2 ch slot for its `S` where its value can go
  stale, shared `FRAME` and `MODE` readings serve the furniture and the HR line, the furniture marks
  each value it reads from the answer and not the frame's or the mode's name, the scale bar or the
  orbit map's own display time, the scale bar stands between the time and the mode so that the
  compact chart keeps its lines, and the counts keep a slot too; the guide's `MODE` row names the
  graph's line. Files: under `apps/hyperion/src/renderer/src/`: `spatial/`'s `Reading.tsx`, the new
  `FrameReading.tsx` and `ModeReading.tsx`, and `SpatialView.tsx`; `displays/galaxy/`'s
  `HrDiagram.tsx` and `chartReadings.ts`; `displays/system/OrbitMapPanel.tsx`; `styles.css`; tests;
  the guide; plan 05's ruling (8). Tests: Vitest for the reading's slot and mark, the shared
  readings, the furniture's marks per value, its order and its own time, the HR line and counts,
  the chart's and the orbit map's stale furniture, both pages' stale text, and the compact scale
  bar's rule. Acceptance: `pnpm test`, `just ci`, and main's look at 1920 × 1080, 1280 × 720 and
  150%, live and stale.
- **P12.T11.e No label held back across its own mark, and the age on its own row in both modes**
  (added 2026-10-22 by main's UX decision on the series' remaining follow-ups; built in
  `lane/80-p12-t11e-followups`, recorded at the end of Risks). Main's looks at T11.c's and T11.d's
  landings found that the compact chart's 256 px stage has no room on either side of a centred
  selection's mark for its label, which ruling 149.1's hold then set back across its own mark and
  bracket, and that the readout's `AGE … · AT UT +0.00 yr` fills its half row at 1920 × 1080, so
  that a chart time of 10 yr or more, or a five-character age, wraps it after `AT`. A label whose
  mark is in view is now drawn only beside its mark, else not at all, the selection's too, and the
  hold stands for a pinned mark out of the view alone; `AGE` takes the readout's row in both modes,
  its chart time in `NOW` right-aligned in `TIME_WIDTH_CH`'s slot; and the guide's `DISPLAY TIME`
  cell says the display time is the console's own and never stale (main's edit). Files: under
  `apps/hyperion/src/renderer/src/`: `spatial/labels.ts` and `displays/galaxy/SystemReadout.tsx`,
  tests; the guide's `DISPLAY TIME` row; plan 05's note on labels beside reticles. Tests: Vitest
  for no label where neither side has room, the selection's and the rest's, on a 256 px stage and
  in a view, the top edge's hold, the destination's label as with no selection, and the age's row
  and slot in both modes and at the clock window's edges. Acceptance: `pnpm test`, `just ci`, and
  main's look at 1920 × 1080, 1280 × 720 and 150%.

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
- **The alert service's appends on macOS (P12.T8; plan 04, P04.T17.b).** Each Knowledge change is
  appended and synced before it is applied (T7.b). std's `sync_data` on Apple is
  `fcntl(F_FULLFSYNC)`, which flushes the drive's cache, so each line costs one full flush. If the
  service's sighting bursts make that visible, batch several lines per sync, still synced before
  any of them is applied. Measure on the Mac when T8 lands.
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
- **`observe::in_system`, added by rendering plan R03 (R03.T2 and R03.T3, by agreement, `e91fd75`
  and `6db8630`; recorded in RM1 validation, 2026-10-02).** Beside this plan's galactic
  `observe::retarded`, `crates/hyperion-sim/src/observe/in_system.rs` evaluates what an observer
  inside a system sees of its bodies and stars: `SystemObserver`, `retarded_in_system` returning an
  `InSystemRetardation`, `IN_SYSTEM_LIGHT_TIME_TOLERANCE` (1 ns) and `IN_SYSTEM_MAX_CORRECTIONS`
  (10). The light time is iterated to a fixed point (SPICE's converged "CN"), the observer enters
  only at reception, and the apparent point is the emission event Lorentz-boosted into the
  observer's frame (R03 Design note 7). `observe/in_system/tracks.rs` holds the sources: a system's
  bodies and stars, placed by plan 14's `PlanetarySystem::position_at` and plan 11's
  `star_positions_at` and moving by `state_at` and `star_states_at`.
  `tests/observe_in_system_golden.rs` pins `solar_like` and `close_binary` in
  `observe/in_system.golden`. This plan's galactic functions are unchanged. The subscription
  envelope that R03.T5.a built to P12.T9's design is recorded under P12.T9 above.
- **T6 as built (lane `obs12d`, 2026-09-30, at `GENERATOR_VERSION` 16; ported onto
  `rendering-and-planets` on 2026-10-15 at 22, the last item below).** No generated output moves
  and no golden changes: the modes are new optional wire fields and a new kind, and a request or row
  without them is byte for byte the one before. `PROTOCOL_VERSION` stays 2 (plan 04, design note
  15).
  - _Protocol._ The new `hyperion_protocol` module `observe.rs` holds `QueryModeDto`
    (`{"type":"now"}` or `{"type":"observed","observer":…}`), `ObservedDto` (`emitted`,
    `light_age_yr`, `apparent_position`, `curvature_error_ly`, `curvature_error_arcsec`),
    `ResolveSystemRequest { universe, system, time, mode }` and
    `ResolvedSystem { universe, time, record }`, whose `record` is a range row. `mode` is
    `#[serde(default)]` on `SystemsInRangeRequest` and `ResolveSystemRequest` and written only
    when observed; `SystemRecord.observed` is written only in observed mode.
    `ResponseBody::ResolveSystem` holds a `Box<ResolvedSystem>`, as `SystemBodies` is boxed, since
    a row would otherwise make every message a third larger (`clippy::large_enum_variant`).
    `resolve_system` joins `REQUEST_KINDS` and the server's exhaustive matches (`kind`,
    `is_large`: not large). No error code is new, so the client's `settledState` switch is
    unchanged.
  - _Deviation: the refusal of an ID._ An ID `resolve` refuses, or whose bits are no system ID, is
    `unknown_system` naming `system`, not `bad_request` as the task's text says: plan 06 added
    `ErrorCode::UnknownSystem` after this plan was written, for exactly this case, and
    `system_summary`, `system_bodies` and `body_detail` answer it so. Accepted
    (`decision-gg-port-d0.md` §6.2).
  - _Deviation: the answer's shape._ `resolve_system` answers
    `ResolvedSystem { universe, time, record }`, `record` a range row, not the bare row Provides
    describes: it echoes the request's universe and time, as `system_summary` does, and is boxed
    in `ResponseBody` for `clippy::large_enum_variant`. Accepted (`decision-gg-port-d0.md` §6.2).
  - _Deviation: the summary's mode is not built._ Provides puts `mode` on `SystemSummaryRequest`
    and `observed` on the summary; the task's text and files do not, so neither is on the wire yet
    (`observe::summary_observed` is ready for it). A readout of one system as observed has
    `resolve_system`'s row, and the brief then from a range query with `include_stellar`, but not
    the stars as observed. Ruled (`decision-gg-port-d0.md` §6.2): it is the new subtask P12.T6.b,
    before T11, whose text is under T6 above; T6 as built is the range query's mode and
    `resolve_system`. _Built by P12.T6.b (`lane/70-p12-t6b-summary-mode`, below)._
  - _Server._ `RangeRequest` checks `mode` after `limit`: an observer that is not a canonical
    position inside the root cube is `bad_request` naming `mode`, as `refused_query` names the
    sim's `ObserverOutsideRootCube`. Only the query's time is held to ±H; emitted times before −H
    are reported as they are. The handler runs plan 03's `range_query` unchanged and then reads
    each row through `observe_hit` over the system cache, lent as the sim's `StarsCache` by
    `SharedSystemCache::handle(key)` (`compute::SystemStarsHandle`; a miss builds
    `observe::stars_of` and stores it under the ID, `SharedSystemCache::get_or_build`, so it shares
    every entry with `system_summary`). A rogue planet, and a centre member, which has no stars
    generated, are read on their line alone, as `range_query_observed` reads a rogue planet. The
    rows are built in the query's job up to `BRIEF_CHUNK_ROWS` and in chunks of that many beyond
    it, as the briefs are; `range_query_observed` is not called because it observes every row in
    one call, and a 20,000-row answer whose stars are cold (0.26 ms a system, T3's bench) would
    hold one worker for about 5 s. The rows are `range_query_observed`'s, bit for bit (unit test
    `observed_rows_do_not_depend_on_chunks_or_the_cache`, whole and chunked, over an empty, a warm
    and no cache). With `include_stellar` in observed mode a row's `stellar` is its primary's
    brief at the emitted time (`ObservedSystem::brief_then`, from the full system), absent when it
    was not yet born then. `resolve_system` is one interactive job: `resolve` (refusals as above);
    a grid system placed by `position_at` at `epoch_velocity`, bit for bit the range row, and any
    other on its own `Drift::of_record` line (plan 08's draw, which `position_at` takes, panics for
    a record without a density component); a hit at distance zero; and in observed mode the same
    reading as a range row. Its row has no brief, since the request has no `include_stellar`.
  - _Centre members._ `centre09c` had not landed in this tree. The central black hole is refused
    in both modes as `unknown_system` naming `system` ("cannot be placed yet", with
    `TraceMotionError`'s message), as
    `system_summary` refuses it, never a panic (`now` would otherwise have panicked in plan 08's
    draw); the server's range query places no centre member, and should a source ever hand it one
    observed, the answer is `bad_request` naming `mode`. When P09.T28 traces the centre, the
    member is placed and read on its orbit with no brief, and
    `the_central_black_hole_is_refused_until_its_orbit_is_built` changes with it.
  - _Tests._ Wire forms in `hyperion-protocol` (both modes, the observed row, the request and
    response of `resolve_system`, a plan 04 request read as `now`); `convert::observe` unit tests
    (the observer's checks and order, the DTO's exactness, the refusal);
    `compute::systems::the_stars_handle_shares_the_entries_of_the_systems_ids`; and
    `tests/observed.rs` over a socket: one 12 ly sphere asked in both modes has the same IDs,
    census and present rows, `observed` only in the observed answer, each the sim's
    `range_query_observed` row with its brief then, and a repeat builds no star; an observer
    outside the cube or off the grid is `bad_request` naming `mode` for both kinds; `resolve_system`
    gives the range row in either mode; an ID naming no system is `unknown_system`; the black hole
    as above; and, slow, `resolve_system_places_a_feature_member_on_its_own_line` (a band-C member
    of the first feature within 3,000 ly of the Sun, placed on its line and observed as
    `observe_hit` reads it with its member record's stars).
  - _Port (batch B, 2026-10-15, at `GENERATOR_VERSION` 22; `decision-gg-port-d0.md`)._ Ported from
    galaxy-generation 9d3a4944 by `git cherry-pick -n` onto the integration tip after A1's P07.T10
    (277b464c), then brought to 9fbc5ea2; the two branches share no history. The content is
    unchanged except as follows. The conflicts were the integration branch's appended entries in
    `compute/mod.rs`, `convert.rs` and `requests/mod.rs`, kept both, and plan 12's record, kept
    after R03's `observe::in_system`; the envelope's walks merged as
    `… → Extinction → ResolveSystem → None`, with `resolve_system` last in `REQUEST_KINDS`. The
    bindings were regenerated by `just gen-protocol`: `RequestBody.ts` and `ResponseBody.ts` gain
    the kind, and `ObservedDto.ts` and `SystemRecord.ts` the ruled doc sentences. By hand: the
    `resolve_system` arm meets R03.T10.a's `Answer` seam through `answered`;
    `mode: QueryModeDto::Now` in the range requests added since the squash (`tests/scene.rs`,
    `tests/sky.rs`, `tests/websocket.rs`); `resolve_system` among the kinds the fake server of
    `apps/hyperion/src/tools/placeShip.test.ts` leaves unfaked, since its switch is exhaustive;
    `every_body`'s `resolve_system` names a universe of its own, the shared one being moved into
    `scene_ship`; `tests/observed.rs`'s brief comparison carries R02.T5's `absolute_v_mag`, which
    the wire's brief gained after the squash, and checks it against the brief then as
    `tests/systems_in_range.rs` checks a present one; and
    `the_central_black_hole_is_refused_until_its_orbit_is_built` pins version 22, at which its
    refusal was re-checked, in place of 16. `requests_naming` in `tests/universes.rs` gains a
    `resolve_system` whose observer is outside the cube, so the new kind is refused for its universe
    before its `mode` is read, as every other kind is. `PROTOCOL_VERSION`'s doc gains the defaulted
    `mode`'s sentence (§6.1), and `ObservedDto`'s and `SystemRecord`'s docs the ruled present-state
    sentences (§6.2), `stellar` excepted, which in observed mode is the brief then. From review:
    `convert::systems_in_range` takes each row's brief and observation from their iterators instead
    of collecting two row-long vectors on every range answer, and
    `observed_rows_do_not_depend_on_chunks_or_the_cache` also asks a 3 ly sphere with the substellar
    layers (a rogue planet read on its line, a brown dwarf through
    `SharedSystemCache::get_or_build`), and also runs over a tight cache and one filled through
    `get_or_generate` as `system_summary` fills it. Re-checked at the port: the centre is still not
    traced (`TraceMotionError::CentreOrbitNotBuilt` stands, P09.T28 unbuilt) and the server's range
    query still passes no sources, so the refusals above hold. No golden or generated output moves,
    and nothing is blessed.
  - _Deferred findings (2026-10-13 rule), from the port's review._ (1) For a feature member
    `resolve_system` places the system by `Drift::position_at`, while plan 09's
    `FeatureMemberSource::hit_at` moves a member by `translated(displacement_over(elapsed))`; the
    sim's `observe_hit` docs put the two a few metres apart. The server's range query passes no
    sources, so no answer differs today; when feature sources are wired into it, `resolve_system`
    should take `hit_at`'s arithmetic so that its row stays the range row bit for bit. (2) The
    chunk test's galaxy has no kinematic tables (`Galaxy::new`), so its exact comparison with the
    sim covers systems at rest; `tests/observed.rs` covers moving ones, to 1e-15 relative.
- **P12.T6: an observed row's `age_myr` and `velocity_km_s` are the present's**, as its `position`
  is (Design note 4 keeps the row). Ruled (`decision-gg-port-d0.md` §6.2): the wire stays as it is,
  and `ObservedDto`'s and `SystemRecord::observed`'s docs say so; T11's readout in observed mode
  shows the age then (`age_myr − light_age_yr ÷ 10⁶`, an Observed value, or `NOT YET FORMED` below
  zero) and `—` for the velocity, until a task puts the velocity at emission on the wire. T11's
  text carries the amendment.
- **P12.T6.b as built (`lane/70-p12-t6b-summary-mode`, 2026-10-21, at `GENERATOR_VERSION` 22).**
  No golden or generated output moves: the mode and the observation are optional wire fields, and a
  summary request or answer in `now` is byte for byte plan 06's. `PROTOCOL_VERSION` stays 2; its
  doc's defaulted-field paragraph now names `system_summary` (`decision-gg-port-d0.md` §6.1).
  - _Protocol._ `SystemSummaryRequest` gains `mode: QueryModeDto`, last, with
    `SystemsInRangeRequest.mode`'s attributes, and loses its `Eq` and `Hash` derives;
    `SystemSummaryDto` gains `observed: Option<ObservedDto>`, last, with `SystemRecord.observed`'s.
    The summary's docs give its `existence`, `age_myr`, `stars` and `hierarchy` at the described
    time, `time` in `now` and `observed.emitted` in observed mode, with `time` echoing the request
    in both, so T11 subtracts no light age from a summary; `ObservedDto`'s doc adds the summary and
    keeps the row's sentences. The nested types' "the summary's time" (`StarSummaryDto`,
    `PulsarDto`, `VariabilityDto`, `StarEventDto`, `HierarchyDto`) is left as it is, so that only
    three bindings move: `SystemSummaryDto`'s doc says that it means the described time. The
    bindings `SystemSummaryRequest.ts`, `SystemSummaryDto.ts` and
    `ObservedDto.ts` change, and nothing else.
  - _Server._ `SummaryRequest` gains the accessor `mode() -> QueryMode` and loses `Eq` and `Hash`;
    its `TryFrom` checks `time`, decodes `system`, then runs `convert::observe::query_mode`
    (`bad_request` naming `mode`). `convert::system_summary(request, stars, time)` keeps its
    signature and answers `observed: None`, so `bodies`, `SceneWorld::hosts` and the scene fixture
    are unchanged. The observed answer is
    `convert::system_summary_observed(request, stars, &summary, &retardation, error)`; both are
    built by the private `summary_dto`, which takes the age at the summary's own time
    (`SystemSummary::time`), so the two cannot drift. The handler keeps one interactive job:
    `get_or_generate` first, so a centre member and a rogue planet are `unknown_system` naming
    `system` in either mode; then, observed, `Observer::new`, `summary_observed` over
    `stars.record()` and the cached stars, a `TraceMotionError` mapped by `untraced` (unreached:
    the cache has refused the only systems that raise it), and `curvature_error`. The emitted time
    is never held to the clock window.
  - _Tests._ Wire forms in `stellar::tests`:
    `a_system_summary_request_of_plan_06s_form_reads_as_now`,
    `system_summary_request_wire_form_observed` and `system_summary_response_wire_form_observed`,
    the `now` forms unchanged; `observe::tests`'s `observer`, `observed` and `observed_json` are
    `pub(crate)` for them. In `convert::stellar::tests`:
    `a_summary_request_checks_time_then_system_then_mode` and
    `an_observed_summary_is_the_summary_when_the_light_left` (a system born after the epoch, seen
    from 3,000 ly before its birth; the answer is `system_summary` at the emitted time with
    `observed` beside it). In `tests/system_summary.rs`, two more properties of the shared-server
    test `pinned_summaries_are_the_sims_and_refusals_name_their_field`, each with its own universe:
    `a_summary_asked_in_both_modes_is_the_sims_now_and_when_its_light_left` (a pinned binary a
    century after the epoch from 3,000 ly coreward, its light emitted before −1,000 years; the
    observed state is `summary_observed`'s bit for bit and its `observed` the sim's retardation and
    `curvature_error` to 1e-15 relative, over a second `OnceLock` galaxy with the full potential,
    `moving_galaxy`) and
    `an_observed_summary_names_the_mode_for_a_bad_observer_and_the_system_for_the_centre`.
    `assert_summary_is_the_sims` takes the time it holds the summary at. The literals of the two
    structs gain `mode: QueryModeDto::Now` or `observed: None`; the hosts of `system_bodies` and the
    scene's hosts stay `now`.
  - _The order of the refusals._ "After the summary's own checks" is read as the request's own
    checks, as the brief orders them and as `resolve_system` does (T6): `SummaryRequest::try_from`
    checks `time`, decodes the ID's bits, then the observer, and the pool job resolves the ID after
    that. So an ID that `resolve` refuses, a centre member or a rogue planet asked with a bad
    observer is `bad_request` naming `mode`, and `unknown_system` naming `system` with a valid
    observer or in `now`; no pool job is taken for a request that cannot be answered. The wire's
    `mode` doc and the handler's `# Errors` say so. No other deviation.
  - _Deferred findings (2026-10-13 rule), from review._ (1) An observed summary of a feature member
    rebuilds the member's feature interior on every request, cached stars or not: the sim's
    `summary_observed` traces the record through `Drift::of_record`, which resolves the member over
    `NoInteriorCache` (`resolve_system`'s observed path does the same). It costs only a summary
    asked of a member by its ID, since the server's range query passes no feature sources yet; a
    sim entry point that takes the member's `Drift`, or lends an interior cache, would remove it.
    (2) `tests/system_summary.rs` keeps its own `close` and `assert_observed_is_the_sims`, near copies of `tests/observed.rs`'s; moving both
    into `tests/common/` touches files outside this lane.
- **P12.T11 as built (`lane/72-p12-t11-observed-mode`, 2026-10-21, at `GENERATOR_VERSION` 22).**
  Client only: no Rust, protocol, binding, server, sim, GPU or golden change, and nothing generated
  moves. The wire is used as P12.T6 and T6.b left it; a request in `now` carries no `mode` and is
  byte for byte the one before (`decision-gg-port-d0.md` §6.1).
  - _Names._ Under `apps/hyperion/src/renderer/src/`: `lib/galaxy/model.ts` gains
    `QueryMode` (`{ kind: "now" }` or `{ kind: "observed", observerLy }`), `NOW_MODE`,
    `sameQueryMode`, `Observation` (`emittedYr`, `lightAgeYr`, `apparentRelLy` from the chart
    centre by `galacticDeltaLy`, `curvatureBoundLy`, `curvatureBoundArcsec`),
    `ChartSystem.observed: Observation | null`, `ChartResult.mode`, `observedAgeMyr` (the age then,
    `ageMyr − lightAgeYr ÷ 10⁶`) and `shownAgeMyr`. `lib/galaxy/wire.ts`: `toQueryModeDto`,
    `toRangeRequest(…, mode = NOW_MODE)` and `toChartResult(response, mode = NOW_MODE)`, the mode
    passed from the request the answer is `ok` for, so that an answer on show keeps its label.
    `lib/galaxy/observation.ts`: `formatLightAgeYr`, `formatCurvatureBound` (`BoundUnit`),
    `formatCurvatureBoundArcsec`, `LIGHT_AGE_WIDTH_CH` (10) and `EMITTED_WIDTH_CH` (11).
    `spatial/marks.ts`: `SegmentMark` (`id`, `from`, `to`, `line: SegmentLine` `solid` or `dashed`,
    `stroke: ColourToken`, `widthPx`, and optional `fromMark`, `endTick`, `minLengthPx`),
    `ObserverMark` (`position`, `label`) and `SpatialScene.segments` and `.observer`, none when
    absent. `spatial/apparent.ts`: `apparentOffset(present, apparent, frame, camera): ScreenOffset`
    (`dxPx`, `dyPx`) and `APPARENT_OFFSET_MIN_PX` (4). `spatial/drawList.ts`: `LineOp.dashPx`
    (`DashPx`), `PolylineOp.weight` (`line` or `mark`), `SEGMENT_DASH_PX` (6 and 4 CSS px, restated
    from `view/wireframe/drawList.ts`'s `PREDICTED_DASH_PX` with a pointer) and `OBSERVER_MARK_ID`.
    `spatial/SpatialView.tsx`: `mode?: SpatialMode` (`name`, `from`). `components/ModeControl.tsx`:
    `ModeControl({ mode, onMode, heldBackId, observedHeldBack })`, `DisplayMode` and `MODE_KEY`
    (`M`). `ChartControls`: `modes?: ChartModeControls` and `MOVE_OBSERVER_LABEL`.
    `useLocalChart`: `displayMode`, `observerLy`, `mode`, `observedHeldBack`,
    `observerMoveHeldBack`, `chooseMode`, `moveObserverToCentre` and `NO_CHART_TO_OBSERVE`.
    `useRangeQuery`'s input gains `mode?`. `useSystemSummary` takes a fourth argument, the mode,
    `NOW_MODE` when absent, and returns `shownMode` and `shownEmitted` too. `SystemReadout` gains
    `starsAt?`, `AS_OBSERVED`, `CURVATURE_BOUND_LABEL` and `EMITTED_LABEL`; `SystemReading` gains
    `widthCh?`; `SymbolLegend` gains `observed?` and `APPARENT_LEGEND`; `chartModel.ts` gains
    `OBSERVER_LABEL` and `answeredMode`; `drawList.ts`'s `PolylineOp` gains `fill?` and its
    `RingLabel` `clearOfPointPx?`, which `furniture.ts`'s ring candidates keep, and `markInksPx`
    counts a mark-weight polyline as ink. Provides' `ObserverMark` is a scene mark, not a
    component, and the task's "its coordinates in the readout" is the view's furniture, as the
    lane's brief directed.
  - _Deviation: no new draw op._ The existing paint test's `leadingCall` switches over every
    `DrawOp` kind with no default, so a new kind would fail its type check. A segment and its tick
    are `line` ops with an optional dash, and the observer's eye a closed `polyline` at a mark's
    weight with an optional fill, which `paint` draws at `markStrokeDevicePx` and does not move
    out. Every existing scene's draw list and paint are unchanged.
  - _Deviation: where the 4 px rule lives._ The view owns its camera and hands it to nobody, so
    `toScene` cannot measure an offset on the screen. It puts the rule on each segment as data
    (`minLengthPx` 4, or 0 for the selected system's; `fromMark`, the mark it leaves), and
    `buildDrawList` measures each segment with `apparentOffset` and cuts it at the mark's outline
    there. A chart in `now`, and a row without `observed`, have no segment.
  - _§5 a, the bound._ The present position's readings (`DIST`, `NORTH`, `COREWARD`, `SPINWARD`,
    `RADIUS`, `ANGLE`, `HEIGHT`) take `~` and no `±`; `CURVATURE BOUND` follows `HEIGHT` under
    `GALACTIC`, in the distance ladder (`km`, `Mm`, `Gm` and `AU` through `formatBodyDistance`
    below 0.01 ly, `ly` from there, each switch with `DISTANCE_HYSTERESIS`'s 5%, held across
    readings as the habitable zones' rows hold theirs), then after a middle dot, as a value of its
    own in `--text`, in `″` to three significant figures, E notation below 0.001″, set against its
    digits (`0.632 AU · 0.00150″`), on a row of its own. A bound of zero reads `0 ly · 0″`.
  - _§5 b, the joiner and the tick._ As leaned: 1 px `--text-muted` at 6 and 4 px, the selected
    system's `--text` 2 px, the tick 0.25 rem across the joiner's end, solid, as wide; never picked.
    Both stand only outside the mark's outline, the tick only where the apparent position does, so
    that the amendment's rule wins over "the selected system whatever its offset" inside the
    outline. They are drawn before every mark, as no path crosses a symbol, the selected one last
    among them.
  - _§5 c, the observer._ An eye, two quadratic arcs 1 rem across and half as tall, filled north
    of the plane and open south of it as every mark is, with a stalk to its foot in `--text`,
    labelled `OBSERVER` as a curve label whose text stands 0.25 rem outside the place of a
    selection's bracket about it, as a mark's label does, counted as ink by the other labels,
    drawn only inside the view after every mark, and never picked. It is none of the symbol
    shapes, the bracket, the chevrons, or the galaxy map's upright cross with a gap (its cursor)
    and diagonal cross (its chart centre). The furniture writes the mode after the time: `MODE NOW`,
    or `MODE OBSERVED FROM` and the observer's `RADIUS`, `ANGLE` and `HEIGHT`. The furniture, the
    legend's `OBSERVER` and `TICK APPARENT POSITION, DASHED TO PRESENT` and the eye follow the
    answer on show's `answeredMode`: the mode asked, but `now` for an answer asked observed whose
    rows all came without their observation, as from a server that ignores the mode.
  - _§5 d, words._ As leaned; the rows proposed are in the report and below.
  - _§5 e, the first observer._ As leaned: `OBSERVED` takes the centre of the chart on show and is
    held back with `NO CHART: centre one to observe from it` while there is neither an observer nor
    a chart on show, and with the link's reason, as both modes are. `OBSERVER TO CENTRE` stands
    beside the mode in both modes and moves the observer at any time, in `NOW` setting where the
    next `OBSERVED` observes from, so that `M` never takes it from under the focus; it is held back
    with the link and without a chart on show. The mode and the observer are kept across a new
    centre and dropped for good with the universe: going back to it does not bring them back.
  - _`AS OBSERVED`._ A line above the readout's scrolling region, so that it stays on show however
    far the readings are scrolled, with a copy in the live region, so that a selection is announced
    with it.
  - _§5 f, the key._ `M`, gated as `K` is (no modifier, no repeat, not in a text field), stepping
    to the other mode, and live while the controls are hidden on a compact page.
  - _§5 g._ `AGE` prints `AT UT <emitted>` in observed mode; the list's `DIST ly` takes `~` for an
    observed row, its column 7 ch for `~500.00`; `LIGHT AGE` and `EMITTED UT` stand in 10 and 11 ch
    slots for `228,000.00` and `-228,000.00`, to universe time's 0.01 yr, so that the three times
    read side by side add up.
  - _The summary._ `SystemsPanel` asks `system_summary` in the mode of the chart on show; the
    `SYSTEM` display's `useSystemData` is unchanged and asks in `now`. `toSummaryRequest` takes the
    wire's mode (`RequestOf<"system_summary">["mode"]`) so that `lib/system/wire.ts` changes in
    that function alone; `useSystemSummary` converts with `toQueryModeDto` and reads only the
    answer's `observed` presence and `emitted`, so `toSystemModel` is unchanged. An answer is in the
    mode asked where it carries `observed` and in `now` where it does not, and is stale where that
    differs from the selection's: the chart's for a row that carries `observed`, `now` for one that
    does not. The primary's `DIES IN`/`DIED` counts from the emitted time, which the stars describe.
  - _Observed rows elsewhere._ A free-floating planet is drawn and listed by its age then
    (`isFormedPlanet`, `toMark`), and every symbol and class is the brief the row carries, the
    primary when the light left.
  - _Deviation: systems unborn when their light left._ A star system that exists now but whose
    light shows it unborn has no brief in observed mode (Design note 4: the wire drops it and keeps
    the row), so it is listed as `NOT YET FORMED` and not drawn, as in `now` (the legend's
    `NOT YET FORMED: LIST ONLY`), although the chart otherwise draws every system at its present
    position. Whether it should be drawn there, with a mark that claims no type, is the owner's.
  - _Tests (123 new, as run)._ `lib/galaxy/wire.observed.test.ts` (12),
    `lib/galaxy/observation.test.ts` (7), `spatial/apparent.test.ts` (6),
    `spatial/drawList.segments.test.ts` (21), `spatial/paint.test.ts`'s new `paint's segments`
    block (10), `displays/galaxy/chartModel.observed.test.ts` (20),
    `components/ModeControl.test.tsx` (9), `displays/galaxy/ChartControls.modes.test.tsx` (6),
    `displays/galaxy/useRangeQuery.observed.test.tsx` (5),
    `displays/system/useSystemSummary.test.tsx` (6),
    `displays/galaxy/SystemReadout.observed.test.tsx` (9),
    `displays/galaxy/SymbolLegend.observed.test.tsx` (2),
    `displays/galaxy/GalaxyDisplay.observed.test.tsx` (2: `OBSERVED` held back until a chart is on
    show, and the mode and observer dropped for good with the universe), and in
    `displays/galaxy/GalaxyDisplay.integration.test.tsx` two steps of the script (`observed`,
    `unobserved`) with four tests and the by-eye case's own block of four (a neutron star now 20,002
    ly from an observer kept at the first centre, read `AS OBSERVED` as the supergiant it was, with
    `LIGHT AGE 20,002.00 yr`). Every existing test passes unchanged.
  - _Proposed nomenclature rows (main adds them to the guide)._ `OBSERVED`, control: the mode
    control's word for `OBSERVED FROM`. `MODE`, label: a display's mode in its furniture and on its
    control (`MODE NOW`). `EMITTED`, label: when the light a reading shows left the thing, in
    universe time (`EMITTED UT -4197.50 yr`). `CURVATURE BOUND`, label: the stated bound on an
    apparent position's error from the neglected curvature of the source's orbit, a bound and not a
    `±`, in the distance ladder and in `″` to three significant figures (E notation below 0.001″).
    `LIGHT AGE`'s precision: 0.01 yr, as universe time. `OBSERVER`, legend and label: the eye, two
    arcs meeting at points, with a stalk, that marks the observer in observed mode.
    `TICK APPARENT POSITION, DASHED TO PRESENT`, legend: the apparent position's marks.
    `OBSERVER TO CENTRE`, command: moves the observer to the centre of the chart on show.
    `NO CHART: centre one to observe from it`, held-back reason.
  - _Deferred findings (2026-10-13 rule)._ (1) `FROM CHART CENTRE` stays the present's dust from
    the chart's centre at the chart's time; with the observer moved off the centre it is not the
    dust the observed light crossed, which would be the observer to the apparent position at the
    emitted time. (2) The `HR DIAGRAM` page plots the briefs of the chart on show, which in
    observed mode are the primaries when the light left, and does not name the mode; it could carry
    the chart's `MODE` reading. _Closed by P12.T11.b._ (3) The legend's dashed sample relies on
    `stroke-dasharray` at `--line-scale` in a non-scaling-stroke SVG, which only the look at
    landing can check. _Closed by P12.T11.c._ (4) The legend names no selected joiner (`--text`,
    2 px), as the orbit map's does its selected orbit. (5) `OBSERVER TO CENTRE` shares `CENTRE`
    with `CENTRE CHART`; `OBSERVE FROM CENTRE` was weighed and left, since it reads as a choice of
    the mode; the nomenclature row decides. _Closed by P12.T11.c._
- **The observed brief reads the summary's primary (P12.T2 fix, lane `observed-class`,
  2026-10-21, at `GENERATOR_VERSION` 22).** P12.T11's landing look (look-72) found one system at
  one emitted instant reading two classes on one screen: GDG 0E0V00 E-141 of LOOK72
  (`820E6EE00000008D`, 10.96 + 6.38 M☉ at P = 326 d), seen from 20,000 ly, was
  `M6.5Ib SUPERGIANT` in its observed range row and `sdO3 HOT SUBDWARF, 2.25 M☉` as the observed
  summary's STAR A. The row read `SystemStars::brief_at`, the primary's single-star track (T0's
  note above: "observed mode reads `SystemStars::brief_at`"; plan 11's "Deviations in P11.T11,
  as built", _The brief_), and the summary read the pair's timeline. The pair's Roche-lobe
  overflow in the Hertzsprung gap at 20.1946 Myr ran into a common envelope at 20.19485 Myr that
  left a 2.37 M☉ helium star in a 1.04 d orbit. The light left 2.5 Myr later, so the summary was
  right, and the row's red supergiant, which the stripped star never becomes, was wrong (science
  check: Götberg et al. 2018, A&A 615, A78; Laplace et al. 2020, A&A 637, A6).
  - _The fix._ `SystemStars::summary_brief_at(t)` is the brief of `summary_at(t)`'s primary: its
    kind, class, log L and `T_eff` from the one private helper that chooses between a star's own
    model and its pair's state (`summarised`, which `star_summary_with` now reads too), and the
    kept attempt's star count (`SystemStars::star_count`). `observe_on_line`, and so
    `observe_hit`, `range_query_observed` and the server's observed rows and `resolve_system`,
    report it as `brief_then`. `SystemStars::brief_at` and the range brief (`BriefModel`) are
    unchanged, so now mode is plan 06's row for row. _Since P11.T11.a_ (plan 11; ruling
    `decision-observed-class-findings.md` §A) `SystemStars::brief_at` has this body and
    `summary_brief_at` is gone, so one brief remains, and now mode's rows read the pair too.
  - _What moves._ An observed row of a primary its pair has changed by the emitted time (a
    stripped star, an Algol's donor or gainer, a merger product), and the star count of a
    redrawn system's observed row (P11.T7, about 5 × 10⁻⁴ of the grid). No golden pins an
    observed row, so none moves; the record is not touched, so no bump.
  - _Tests._ `tests/observe_brief.rs`: look-72's system at look-72's instant (the stripped helium
    star of 2–4 M☉ in both the row and the summary, no supergiant, where its single-star track is
    EAGB), and six layer-E systems near it whose primary its pair strips, re-aged so that the
    stripping starts 40,000 years before the epoch and observed 20,000 years before, during and
    20,000 years after it: the row's kind, class, log L, `T_eff` and count are the summary's
    primary's, bit for bit. `stellar::system`'s `the_summary_brief_counts_the_kept_stars` held
    the kept count against a first attempt's (removed by P11.T11.a with `first_star_count`, since
    the one brief counts the kept stars by construction). T2's dying supergiant is now searched by the
    summary's brief, as observed mode reads it.
  - _Deferred findings (2026-10-20 rule)._ (1) Now mode keeps the divergence: a range row
    (`BriefModel`, single-star) and the now summary disagree for a primary its pair has changed
    at the query's time (P11.T11's open ruling; with a decision agent through main). **Ruled
    2026-10-21 (`decision-observed-class-findings.md` §A), built by plan 11's P11.T11.a.** (2)
    `822C72E00000008C`, a case-A Algol, core-collapses its donor while it is still on the main
    sequence (6.34 M☉) at its pinned single-star death age, 24.6187 Myr (`binary/supernova.rs`,
    `pinned_collapse`; with a decision agent through main). (3) The stripped mass, 2.37 M☉ for a
    10.96 M☉ primary, is some 30% under the detailed grids' 3.32 M☉ (Götberg et al. 2018, Table 1,
    Z = 0.014) and 3.43 M☉ (Laplace et al. 2020, Table A.2), likely the Hertzsprung-gap core mass
    of Hurley et al.'s fits. (4) **Closed** by galaxy plan 06's P06.T24.c (lane `stripped-class`,
    2026-10-21; `decision-stripped-star-class.md`): a 2.25 M☉ stripped star was classified
    `HOT SUBDWARF`, but Götberg et al. (2018) find subdwarf-like spectra only up to about 1.7 M☉,
    and Drout et al. (2023, Science 382, 1287) put the 2–8 M☉ stripped stars between the subdwarfs
    and the Wolf-Rayet stars. Hot subdwarfs now end at 1.7 M☉, and the heavier helium stars below
    the Wolf-Rayet floor are `STRIPPED HELIUM STAR`s (`He-sdO3` here, in the observed and the now
    summary alike). Plan 11's P11.T11.b (the same lane) closes the other half: a paired helium
    star now carries its helium-star entry mass, so its surface follows the single star's rule and
    a paired Wolf-Rayet star turns WC as a single one does.
    (5) The pin collapses the helium star at about 0.7 of its helium main sequence (Hurley et al.
    2000, eq. 79, gives 3.71 and 3.38 Myr at 2.25 and 2.37 M☉, against the 2.53 Myr to the pin),
    not after core-helium depletion; ruled not a blocker (`decision-observed-class-findings.md`
    §B.1).
- **P12.T11.b as built (`lane/73-p12-t11b-hr-mode`, 2026-10-21, at `GENERATOR_VERSION` 22).**
  Client only: no Rust, protocol, binding, server, sim, GPU or golden change, nothing generated
  moves, and the guide is not edited. The `HR DIAGRAM` page names the mode and time of the answer
  it plots, as the chart's furniture does, and closes T11's deferred finding (2).
  - _Names._ Under `apps/hyperion/src/renderer/src/displays/galaxy/`: the new `chartReadings.ts`,
    the one writer of the chart's readings, exports `CHART_FRAME_NAME` (`GALACTIC`, the
    furniture's `frameName` literal before), `RADIUS_WIDTH_CH` (9), `ANGLE_WIDTH_CH` (6),
    `HEIGHT_WIDTH_CH` (9), `TIME_WIDTH_CH` (9), `CENTRE_DECIMALS` (1), `radiusReading`,
    `placeReadings`, `timeReading(timeYr)` and `modeReading(mode)`, each with TSDoc. The constants
    and the three functions are moved from `LocalChartPanel.tsx`, and the time reading's
    construction is made `timeReading`; the panel imports them and its furniture reads exactly as
    before. `HrDiagramProps` gains `mode: QueryMode` and `timeYr`; `HrDiagram.tsx` gains the
    private `ModeLine` and `LineReading`. `GalaxyPages` passes `answeredMode(chart.result)` and
    `chart.result.timeYr`; its empty state is unchanged. In `styles.css`'s `.hr-diagram` block:
    `.hr-diagram__readings`, `.hr-diagram__stale-slot`, and the line added to
    `.hr-diagram--stale`'s muted rule.
  - _Deviation: the line's own reading._ The brief named the chart's `Reading`
    (`spatial/Reading.tsx`), but it has no room for a stale mark, a mark beside its `div` would
    stand in the `dl` outside any term's group, and `spatial/` is outside the lane's files.
    `LineReading` writes `Reading`'s markup in `Reading`'s classes (`field spatial-reading`,
    `field__value`, `spatial-reading__value` in its `ch` slot, `spatial-reading__unit`,
    `readout__missing`) and adds after the unit a slot, `hr-diagram__stale-slot`, 2 ch wide in the
    value's monospaced figures. The labels, slots and precisions are the shared module's, as the
    chart's are. Posted to the lane's issue.
  - _§5 a, where the line stands._ As leaned: a `dl` under the plot, after the temperature axis's
    label and before the counts. It is in the furniture's own classes as well
    (`spatial-view__readings`, and `spatial-view__mode` and `spatial-view__observer` within it), so
    that it wraps between readings and never inside one as the furniture does, with no selector
    added outside the `.hr-diagram` block. `.hr-diagram__readings` adds only `flex: none`.
  - _§5 b, the time and the frame._ As leaned: `UT` at `TIME_WIDTH_CH`, then `MODE`. In observed
    mode `FRAME GALACTIC` stands before `UT` and is the heading the observer's `RADIUS`, `ANGLE`
    and `HEIGHT` stand under, as on the chart's furniture; no second `GALACTIC` is written. In
    `NOW` there is no `FRAME`, since the diagram plots no positions.
  - _§5 c, read from the answer._ As leaned: the page reads `answeredMode(chart.result)` and the
    result's `timeYr`, so a choice not yet answered leaves it as its answer reads, and an answer
    asked observed whose rows all came without their observation reads `MODE NOW`.
  - _§5 d, stale._ As leaned: the line joins `.hr-diagram--stale`'s muted rule, and the time and
    each coordinate trail `StaleMark`'s `S` in their slot; `NOW`, `OBSERVED FROM`, `GALACTIC` and
    an em dash take none. The slot is kept while the answer is live, so that the mark's coming
    moves nothing on the line. The chart's own single furniture `S` is unchanged.
  - _§5 e–g._ As leaned: the canvas's accessible name stays `Hertzsprung-Russell diagram` (and
    `…, stale`), the line being DOM text in the figure after the plot; no new words; no
    `AS OBSERVED` line.
  - _Tests (22 new, as run)._ `displays/galaxy/chartReadings.test.ts` (7: the frame's name, a
    place's readings at 0.1 ly in the chart's 9, 6 and 9 ch slots, the axis's missing `ANGLE`, the
    radius, the time in its 9 ch slot, `NOW`, and `OBSERVED FROM` with the observer as a place);
    `displays/galaxy/HrDiagram.test.tsx`'s new block (10: `NOW` with `UT` and no `FRAME` or
    observer; the line's place in the figure, between the axis's label and the counts; observed,
    `FRAME GALACTIC` and the observer at the chart's precisions nested in `MODE`; the axis's
    `ANGLE —`; the accessible name unchanged; four stale marks, on the time and each coordinate and
    on no word; the time stale but not `NOW`; no mark on the em dash; the slots' room; and the
    line in the stale rule); `displays/galaxy/GalaxyDisplay.observed.test.tsx`'s new block (5,
    with an off-axis centre typed into `CURSOR` and observed: the `HR DIAGRAM` page reads
    `OBSERVED FROM` and the observer; it reads the chart furniture's `MODE` and `UT` text; it keeps
    `OBSERVED FROM` while `NOW` is chosen and not yet answered; it reads `MODE NOW` once answered;
    and an answer asked observed whose rows came without `observed` reads `MODE NOW`). The existing
    `HrDiagram` tests pass with only the new props added, and every chart furniture test passes
    unchanged.
  - _Deferred findings (2026-10-13 rule)._ (1) The two pages mark a stale answer differently: the
    chart's furniture trails one `S` for the whole block (plan 05's ruling (8), applied in its T11
    validation), the HR page's line one after each value, as its counts do, so the same answer's
    `UT` reads `+0.00 yr` beside the block's one `S` on one page and `+0.00 yr S` on the other;
    main decides. _Closed by P12.T11.d._ (2) A `stale` prop on `Reading`, and a shared `FRAME` and
    `MODE` reading, in plan 05's `spatial/`, would let the HR page's line use the chart's own markup
    rather than write it again in `LineReading` and `ModeLine`; outside this lane. _Closed by
    P12.T11.d._ (3) The guide's `MODE` row places the reading in a spatial view's furniture only;
    the `HR DIAGRAM` page, a graph, now carries it under its plot, which the row could name (UX
    review; the guide is main's). _Closed by P12.T11.d._ (4) The page's existing counts keep no
    room for their `S` (P06.T37), so a stale answer moves them while the new line keeps its place:
    the page now behaves two ways (UX review). _Closed by P12.T11.d._
- **P12.T11.c as built (`lane/74-p12-t11c-compact-refusals`, 2026-10-22, at `GENERATOR_VERSION`
  22).** Client and guide only: no Rust, protocol, binding, server, sim, GPU or golden change, and
  nothing generated moves. P06.T24.c was not on `origin/rendering-and-planets` at the lane's last
  merge, so the compact comment's figures are look-72's and `LocalChartPanel.tsx`'s doc is
  unchanged. Closes T11's deferred findings (3) and (5).
  - _Names._ Under `apps/hyperion/src/renderer/src/`: `spatial/drawList.ts` exports
    `SEGMENT_TICK_REM` (0.5, private before) and
    `observerBoxPx(scene, camera, viewport): ScreenBoxPx | null`, the eye's 1 rem square where it
    is drawn, else `null`, which shares the private `observerCentre` with `observerOps`.
    `spatial/labels.ts`: `LabelSurroundings.observer` (`BoxPx | null`, optional), which
    `SpatialView` fills. `ChartControls.tsx`: `MOVE_OBSERVER_LABEL` is `OBSERVER TO CHART CENTRE`;
    the private `refusalPart`, through `StatusText`'s `renderPart`; the classes
    `chart-controls__field--refused`, `chart-controls__error-name` and
    `chart-controls__modes--reason`. `SystemReadout.tsx`: the private `AtTime`. `styles.css`: the
    compact rules for the three classes, after the choices' bottom margin, and
    `.system-readout__beside .system-readout__value`, its stale form and `.system-readout__whole`.
  - _A2.1, the refusals._ As briefed. The field is marked `--refused` while its refusal stands,
    and the compact page hides its hint then; the refusal takes `flex-basis: auto`, no margin and
    `nowrap`, and its name, with its trailing space, the `.visually-hidden` pattern, whose
    declarations a guard test holds the rule to. "Never wraps" rests on the row's room, since the
    field keeps `flex-wrap: wrap`: reckoned, the refusals end near 775 and 801 px of the page,
    inside the column that ends at 831 (the UX review reckons 461 and 499 px of their rows, the
    same). The stacked page is unchanged.
  - _A2.2, the command._ The 0.5 rem gap, and `nowrap` with `flex: none` on the row's parts while
    the row is not marked `--reason`, which it is only with the link up and a `NO CHART` reason on
    show. Reckoned, the row ends some 5–8 px inside the column at 1280 × 720 (the lane 6–8, the
    UX review 5), room for the 3 px focus ring, so the 0.25 rem padding fallback is not built and
    `.control` is untouched. Unshrunk, a row the look finds short runs past the column rather than
    wraps onto the furniture.
  - _A2.3 and A2.4, the tick and the sample._ As briefed: the sample is 2.75 rem, 44 units, the
    joiner `M 42 4 H 0` drawn from the tick, (6 + 4 + 6) px × 2.56 = 40.96 of its 42; the tick 8
    units, 0.5 rem. The compact legend keeps six lines in both modes by the UX review's reckoning,
    the observed last line ending near 768 px.
  - _Deviation: the eye is its own surrounding._ The eye's box is `LabelSurroundings.observer`
    rather than one of `obstacles`, and `placeLabels` adds it to the obstacles for the destination's
    label and the rest, since furniture never moves the selection's label and the eye must (its
    other side, or none, as where it yields to the destination's).
  - _Deviation: the eye at a system's point._ A label beside the mark the eye stands on starts
    `labelOffsetPx` out, the mark's radius, 0.5 rem and 0.75 px + 5δ, outside the eye's 0.5 rem
    half-width, so it never covers the eye and is kept, the selection's too: the brief's tests
    "the observer at an unselected system's point drops that system's label" and "at the selected
    system's point puts its label on its other side, or none" contradict its rule that a label that
    would cover the eye is dropped. The tests put the eye under the label instead, look-72's case
    (a system's label at the centre), and one pins that the label of the mark the eye stands on is
    kept. Only a label held inside the view back across its own mark covers the eye there.
  - _A2.6, `AGE`._ `· AT UT <time> yr` in both modes, the time's figures a value's, in `--text`
    and untracked, muted with a stale readout, as `EMITTED UT`'s are and as the terrain readout's
    `· AT 243 K` sets its figure (UX review); its words and unit stay muted. `.readout dd` already
    set the figures in B612 Mono, so only their tracking and colour changed. As observed the
    reading is wide and the time stands in `EMITTED_WIDTH_CH`'s 11 ch slot. `UT <time> yr` is kept
    whole (`nowrap`) in both modes and `· AT` is not, so that the readings' one column at 1280 × 720
    breaks the reading after `AT` rather than run it 19–34 px past its edge (the UX review's
    must-fix, against a first build that kept `· AT UT <time> yr` whole); in the wide row at
    1920 × 1080 it stands on one line.
  - _A2.7 and A2.8._ As briefed, `CHART TIME`'s `±1000 yr` kept. The guide's sentences are written
    as given but one: the angle bullet's last sentence names the picoarcsecond too, "The
    milliarcsecond, microarcsecond and picoarcsecond, which Table 8's note gives for astronomy, are
    not used", since Table 8's note gives all three (SI Brochure, 9th edition, V4.01 (2026), note
    (a), p. 141; note (b) in V1.08 to V3.02). Table 8's degree, minute and second, and §5.4.3's
    "no space is left between the numerical value and the unit symbol" for `°`, `′` and `″`, held
    as written (science check).
  - _Left as written._ T11's record keeps `OBSERVER TO CENTRE` and plan 05's records `0.01-500 ly`:
    they say what was built then.
  - _Tests (44 new, as run)._ `displays/galaxy/ChartControls.test.tsx` (7: the hint in words; each
    field's refusal in its own field while both stand, described by its own; each name, with its
    trailing space, in its element; the same refusals and descriptions inside a compact and a
    stacked page, which pins that the markup does not depend on the page; the refused mark dropped
    with the refusal); `ChartControls.modes.test.tsx` (4: the row's `--reason` mark with both
    reasons, with the command's alone, with a chart on show and with the link down);
    `LocalChartPanel.compact.test.tsx` (8 stylesheet guards: the refusal's row, the hint giving
    way, the hidden name, the stacked rules; the row's gap, its `nowrap` and `flex: none`, its
    wrapping reasons, `.control` untouched); `SymbolLegend.observed.test.tsx` (4: the dash, two
    dashes and the gap at 2.56, a dash meeting the tick, the tick's 0.5 rem);
    `spatial/drawList.segments.test.ts` (3: `observerBoxPx` inside the view, in `now` and outside
    it); `spatial/labels.test.ts` (6: a label over the eye dropped, the label beside the mark the
    eye stands on kept, the selection's other side, the selection's none, the destination's next
    place, and no eye placing every label as before); `spatial/SpatialView.test.tsx` (4: dropped,
    the selection's other side, `now`, the observer outside the view);
    `displays/galaxy/SystemReadout.observed.test.tsx` (8: wide as observed, the 11 ch slot, the
    time whole with its label and unit in both modes, the stylesheet's `nowrap`, `--text` and
    untracked, stale muted, `now`'s slot). Updated for the words: the tick's ends (0.5 rem), the
    command's name in the mode and integration tests, and `· AT UT` in four readout tests and the
    integration test, which now reads `AGE` by its term. Every other existing test passes
    unchanged.
  - _Deferred findings (2026-10-13 rule)._ (1) `CHART TIME`'s hint `±1000 yr` sets a limit with
    the sign the guide's `±` row keeps for an Estimated value's uncertainty; `-1000 to 1000 yr`,
    its refusal's words, would follow § Numbers as `DRIVE RANGE`'s now does (UX review; the brief
    kept it). (2) In `NOW` the time after `AT` has no fixed slot, so `yr` moves as `CHART TIME`
    changes; an 8 ch slot would make the reading as long as its longest, which the UX review
    reckons wraps after `AT` in its half row at 1920 × 1080; main's look measures. _Closed by
    P12.T11.e._ (3) The guide's
    grouping bullet credits the thin space to the SI Brochure's §5.4.4, which says "a space"; the
    thin, fixed space is NIST SP 811's (§10.5.3; science check). (4) The guide's observer bullet
    says marks' labels keep off the eye "as they keep off the core arrow", but the selection's
    label, which the core arrow never moves, takes its other side or none (plan-conformance
    review). (5) The beside's words, `AT UT`, `yr` and `AGO`, inherit `.readout dd`'s B612 Mono,
    where § Typography gives labels B612 (UX review; before this task). (6) The guide's
    `1/8,000 s` (its exposure row) groups a four-digit whole part, which the clearer grouping rule
    now rules out (UX review; before this task). (7) The eye standing on a centred system's mark
    is left as both true positions, the eye drawn after every mark; main's look judges whether the
    two read as one. (8) Putting the eye's box on the `DrawList` would project the observer once
    (TypeScript review); left, since the smoke harness's and the HR projection's draw lists, outside
    this lane, build the type, and both paths already share `observerCentre`.
- **P12.T11.d as built (`lane/75-p12-t11d-stale-per-value`, 2026-10-22, at `GENERATOR_VERSION`
  22).** Client, guide and plan only: no Rust, protocol, binding, server, sim, GPU or golden
  change, and nothing generated moves. Neither P06.T24.c nor the `GENERATOR_VERSION` 23 bump was on
  `origin/rendering-and-planets` at the lane's last merge (`a324a05a`), so the reckoning is at the
  base's figures. Main's ruling on the fit came before Part A was committed, so Parts A and B are
  one commit. Closes T11.b's deferred findings (1) to (4).
  - _Names._ `spatial/Reading.tsx`: `ReadingProps.stale?: boolean | undefined`, the brief's shape,
    and `ReadingProps.own?: boolean | undefined`; the slot `span.spatial-reading__stale-slot`,
    whose rule (`.hr-diagram__stale-slot`'s: `inline-block`, `min-width: 2ch`, right-aligned) now
    stands beside `.spatial-reading__unit`; the modifier `spatial-reading--own`. The new
    `spatial/FrameReading.tsx` (`FrameReading`, `FrameReadingProps.frameName`) and
    `spatial/ModeReading.tsx` (`ModeReading`, `ModeReadingProps.mode` and `.stale?`, and
    `SpatialMode`, moved from `SpatialView.tsx`, which `chartReadings.ts` now imports from there).
    `SpatialViewProps.timeIsOwn?: boolean | undefined`. In `HrDiagram.tsx` `LineReading` and
    `ModeLine` are gone: the line is a `dl` of `FrameReading`, `Reading` and `ModeReading` written
    in place, and each count's `dd` ends in `span.hr-diagram__count-slot`. In `styles.css`:
    `.hr-diagram__count-slot`, `.spatial-view--stale .spatial-reading--own` (`color: var(--text)`)
    after the muting rule, and the compact scale bar's `margin-right: 0.75rem`.
  - _§3, the fit._ The lane's reckoning from look-73's boxes agrees with main's. With a 2 ch slot
    (18.2 px) after `RADIUS`, `ANGLE`, `HEIGHT` and `UT` (191.4, 136.2, 192.1 and 152.9 px) and
    after the observer's three (the mode 728.9 px), line 1's five readings come to 802.4 px with no
    gap, more than the 798 px line, so in today's order `UT` wraps (it would end at 883.4) and
    compact `OBSERVED` takes a fourth line, its stage 222 px against its 240 px floor. Main's ruling
    of 2026-10-22, on the lane's issue, replaced the brief's stop: the furniture holds a first `dl`
    (`FRAME`, the centre and the time), the scale bar, then, only where `mode` is given, a second
    `dl.spatial-view__readings` holding the mode alone; the compact scale bar takes the readings'
    0.75 rem as `margin-right`; nothing else in the layout changes; and the orbit map's
    `DISPLAY TIME UT`, the console's own clock, takes no mark and is not muted. As built, reckoned
    at 1280 × 720 and 100% (at 150% every edge within 0.5 px, the furniture from 32.7 px):
    - `NOW`: line 1 ends after `HEIGHT` at 718.5; line 2 holds `UT` (33–185.9), the bar
      (197.9–384.9) and `MODE NOW` (396.9–488.2). Two lines, as before; the stage 258 px (264.6 at
      150%) over its 240.
    - `OBSERVED`: line 1 the same; line 2 `UT` and the bar, to 384.9, the mode not fitting after
      them (it would end at 1125.8); line 3 the mode, 33–761.9. Three lines, as before; the stage
      240 px (248 at 150%), at its floor. Line 3 is the tightest: the mode's box ends 69 px inside
      the page's 831, and with its 0.75 rem margin, which the line holds as the base's `UT` held
      its own (823 of 831), 57.1 px inside (the UX review's figure; main's ruling gave 69); every
      wrap is certain, `UT` by 52.4 px past line 1's end and the mode by 294.8 past line 2's.
    - At 150%, main's figures: line 1 ends at 718.1; line 2 at 487.9 in `NOW` and 384.6 observed;
      line 3 32.7–761.5. With P06.T24.c, not in this tree, main's figures: the same edges at 100%
      (about 718, 488, 385 and 762 at 150%), and the stages 262 px in `NOW` and 228 observed at
      100%, 266 and 232 at 150%, against its 224 px floor.
    - Live and stale alike, since every slot is kept and the own time has none; no reading wraps
      inside itself, each being `nowrap` and the longest, the mode, 728.9 px of the line's 798.
    - The stacked page (1920 × 1080, the furniture to 1068.7, its readings 1.25 rem apart and its
      parts 1.5 rem): line 1 the first `dl`, to 915.4; line 2 the bar (33–220) and the mode
      (244–335.3 in `NOW`; 244–996.9 observed, the observer 1.25 rem apart). Two lines in both
      modes, where `OBSERVED` had three, so its stage gains a line, about 22 px.
    - The orbit map has no mode, so one `dl` and the bar, as before: its centre's three readings
      grow 54.6 px and the display time not at all. By estimate only (B612's tracked capitals at
      about 10.7 px, from look-73's boxes, and the `SYSTEM` grid's 2fr column): at 1280 × 720
      `FRAME SYSTEM BARYCENTRIC` to `HEIGHT` comes to about 794 px of a map about 785 wide, so
      `HEIGHT` may now open the display time's line (the two about 564 px), three lines either way;
      at 1920 × 1080 the five readings, about 1166 px, still share one line of about 1210, and the
      bar takes the second.
    - The HR counts each grow 18.2 px live, so stale moves none: `OFF SCALE` stands at 167.6 in
      both states, look-73's stale place. `NOT PLOTTED`'s row, from 49 with one-digit counts:
      `NEUTRON STAR` to 215.2, `BLACK HOLE` 231.2–372.2, and by estimate `NO REMNANT` to about 526
      and `NOT YET FORMED` to about 718, inside 831; a `PLANET` count would end near 833 and a
      `DATA INVALID` count alone near 893, so either may now wrap the row where before only both
      together did, and more digits wrap it sooner. The first row under a `STARS` filter
      (`SYSTEMS 1,630 OF 1,630 SHOWN: REMNANTS`, `PLOTTED 1,630`, `OFF SCALE 0`) ends near 713.
      A wrap there is a watch item: the HR stage has 204 px over its 128 px floor at 1280 × 720.
  - _Choices._ (a) The own time is a second optional prop on `Reading`, `own`, rather than a third
    state of `stale`, so that `stale` keeps the brief's shape; `SpatialView` gives the own time
    `own` and no `stale`, and `own` overrides any `stale` given with it, so that an own reading has
    no slot and no mark whatever `stale` says (deferred finding (4)). (b) The shared readings are
    components in files of their own, since the TypeScript rules give each exported component its
    own file, and `SpatialMode` moved with `ModeReading`, which reads it. (c) The counts' slot has
    a rule of its own at the reading's 2 ch, not a second selector on the reading's, so that
    `stylesheetRule` holds each by its selector. (d) The HR line keeps no private wrapper. (e)
    `LocalChartPanel.tsx`'s stale sentence, "drawn and read as stale, in `--text-muted` with a
    trailing `S`", still holds and is unchanged.
  - _Deviations, by main's ruling._ On a display with a mode the furniture's order is now the
    frame, the centre, the time, the scale bar, then the mode in a second
    `dl.spatial-view__readings`; with none, the one `dl` and the order are as before. `SpatialView`
    gains `timeIsOwn` and the stylesheet the own time's rule, and `OrbitMapPanel` sets `timeIsOwn`,
    its one code change. The guide's `MODE` cell is the brief's, with "after the time and the scale
    bar" for "after the time". The "furniture tests that count one `S`" were none: no test counted
    the row's one `S`, and `LocalChartPanel.test.tsx`'s stale test, which counted the page's
    `stale` marks, now reads each value's.
  - _Tests (38 new, 3 changed, as run)._ `spatial/Reading.test.tsx` (9: no slot without `stale`,
    an empty slot live, the mark after the unit, after a value with no unit, the em dash's empty
    slot, the slot's rule, the own reading's class and no slot, no mark on an own reading given
    `stale`, no other reading's class); `spatial/FrameReading.test.tsx` (2);
    `spatial/ModeReading.test.tsx` (7: `NOW`, the observer nested, marks on the observer's values
    only, the axis's em dash, `NOW` never marked, empty slots live, none without `stale`);
    `spatial/SpatialView.test.tsx`, in a block of its own after the furniture's, "SpatialView's
    stale furniture, one S per value" (15: the centre's and the time's marks, the observer's, none
    on the frame's or the mode's name, the em dash's empty slot, none in the bar, no mark outside a
    slot, empty slots live, `AZM` and `ELV` bare live and stale (two cases), the order by
    `compareDocumentPosition`, one `dl` with no mode, the own time, its colour rule, the canvas's
    stale name, the muting rule); `HrDiagram.test.tsx` (2: an empty slot after each count live,
    `SYSTEMS` among them, and each count's mark in its slot); `GalaxyDisplay.observed.test.tsx` (1:
    stale, both pages read `+0.00 yr S` and the same observer with its marks);
    `SystemView.test.tsx` (1, beside the unchanged stale test: the centre's values marked,
    `DISPLAY TIME UT` not); `LocalChartPanel.compact.test.tsx` (1: the compact scale bar's margin).
    Changed: the HR line's slot test names the shared slot and its rule; the chart's stale test
    reads each furniture value's `S` and `GALACTIC` bare; the compact guard that said `UT` ends the
    first line is renamed. Every other existing test passes unchanged.
  - _For main's look._ The order (the time, the bar, then the mode); compact `OBSERVED`'s line 3,
    57.1 px inside the page with its margin (69 to its box), and its stage at the 240 px floor (228
    against 224 with P06.T24.c); line 2's right edge (384.9 observed, 488.2 in `NOW`); the stacked
    page's `OBSERVED` furniture on two lines; the orbit map's first line at 1280 × 720, where
    `HEIGHT` may wrap, and its `DISPLAY TIME UT` in `--text` with no `S` while stale; the HR counts'
    second row with a `PLANET` or `DATA INVALID` count; `OFF SCALE` at its live x; and `AZM` and
    `ELV` unchanged.
  - _Deferred findings (2026-10-13 rule)._ (1) The console now marks a stale answer two ways: each
    value in the spatial furniture and on the HR page, and once a block in the census line, the
    list, the readout and the panels' titles (`Systems`, `Orbit map`, `Bodies`), where the guide's
    "Data states" says each state "looks the same everywhere"; the blocks are outside this task.
    (2) The guide says the console's own values are never stale only of `BASELINE` and
    `PAD WIDTH`; its `DISPLAY TIME` row could say so of the display time, which main's ruling now
    leaves unmarked and unmuted (UX review; the brief allows no other guide edit). _Closed by
    P12.T11.e._ (3) Under a
    `STARS` filter a stale `SYSTEMS` count reads `… SHOWN: REMNANTS S`, its `S` after the filter's
    name, the console's own live choice, as look-73 found `2 ly S` after the scale; the marks could
    follow the figures instead (UX review; the place is P06.T37's and the brief's §5 A4 kept it).
    (4) `Reading`'s `stale` and `own` are two optional props, so the type admits both at once,
    which renders as `own` alone; one union prop (a source that is the answer's, stale or not, or
    the console's own) would rule the pair out, but renames the brief's `stale` (TypeScript review;
    main's call).
- **P12.T11.e as built (`lane/80-p12-t11e-followups`, 2026-10-22, at `GENERATOR_VERSION` 23).**
  Client, guide and plan only: no Rust, protocol, binding, server, sim, GPU or golden change, and
  nothing generated moves. P06.T24.c and the 23 bump are on the base, and the lane's last merge of
  `origin/rendering-and-planets` is `174d583c` (P14.T52.a, nothing under `apps/`). Main's UX
  decision of 2026-10-22 found none of the series' twelve remaining items a blocker: it had the
  two built here, the hold and the `NOW` slot, made the guide's `DISPLAY TIME` cell its own edit,
  made here as main wrote it, and recorded the other nine itself. Closes T11.c's and T11.d's
  deferred findings (2).
  - _Names._ No export is added or renamed. In `spatial/labels.ts`, inside `placeLabels`, the
    private `inPlace(label, anchor)`: whether a label on its mark's line stands beside its mark
    (`besideItsMark`) or its mark is out of the view; `lineLabels` now holds only the labels in
    place. The docs of `lineLabel`, `besideItsMark`, `LabelSurroundings.pinnedIds` and
    `placeLabels`' remarks say what is now true. In `displays/galaxy/SystemReadout.tsx`, `AtTime`
    sets its time in a `system-readout__value--fixed` slot in both modes, `TIME_WIDTH_CH` (from
    `chartReadings.ts`) in `NOW` and `EMITTED_WIDTH_CH` as observed, and the age's `Reading` is
    always `wide`, so the private `ageReading` no longer returns the `formed` it was read for. The
    docs of `destinationLabel`'s `others` and `placeLabels`' remarks name the labels the
    destination's is placed against. The guide's `DISPLAY TIME` cell is main's, word for word;
    Prettier re-padded no other line.
  - _A, the hold._ As briefed. A label whose mark is in view is drawn only where `besideItsMark`
    holds: the rest are dropped; the selection's, held back on its first side, tries its other
    side under the conditions it takes it under where it yields (clear of the destination's label
    and chevrons and of the eye, and beside its mark), else is not drawn; the destination's own
    rule is unchanged. A pinned mark out of the view keeps the 0.25 rem hold, and the hold at the
    top and bottom edges, which leaves a label beside its mark, stands. For a mark in view the
    first side is held back only where both are, since `lineLabel` flips to the left only where
    the right would run past the right edge, and the right never runs past the left edge for a
    mark at x ≥ 0; so the other side never rescues it, and the code tries it anyway, as briefed.
  - _Choice: the destination's label avoids no label held back._ A chosen label that would be held
    back across its own mark in view is left out of the labels the destination's is placed
    against, as a label whose mark is out of view already was (R07.T16.j), since with no selection
    it is never drawn (addendum C, C1: placed "as though no mark were selected"). So the
    destination's label may now take a place that such a label's held box covered. Its own places
    and rule are unchanged, and selecting still never moves it.
  - _A's reckoning_ (estimated boxes, as `placeLabels` reckons them; the stage 256 px wide, its
    edge clearance 4 px). A 16-character designation is 138.88 px wide (16 × 0.62 em at
    0.875 rem). Its near edge stands the mark's radius, 0.5 rem and 2.00 px out at a ratio of 1,
    and 0.75 px at 1.5, where δ is 0 and the outline 1.5 px. About a class-4 mark (radius 8 px:
    18 px out at 1, 16.75 at 1.5) the right side fits to x 95.12 and the left from x 160.88 at 1,
    and to 96.37 and from 159.63 at 1.5: so marks from 95.12 to 160.88 px across (65.76 px, about
    the stage's middle quarter) now have no label at 1, and from 96.37 to 159.63 (63.26 px) at 1.5.
    About a class-0 mark (radius 4 px) the band is 99.12–156.88 (57.76 px) at 1 and 100.37–155.63
    (55.26 px) at 1.5. A label of n characters whose box starts d px out has no side over a band
    2d + 17.36n − 248 px wide, so about a class-4 mark at 1 no label of 12 characters or fewer
    loses its side. A stage wider than 2 × (18 + 138.88 + 4) = 321.76 px always has a side for the
    16-character label, so the stacked page's 1036 px stage and the orbit map's 787 px are
    unchanged. The tests pin both ratios' edges (95 and 96, 160 and 161 px at 1; 96 and 97, 159
    and 160 at 1.5).
  - _B, the age._ As briefed. The age's reading is `wide` in both modes, so at 1920 × 1080 `MASS`
    stands alone on its row in `NOW` as it does as observed, and the two modes set the readout's
    rows alike but for the rows `AS OBSERVED` adds; with nothing selected the age's em dash takes
    the row too. Its time stands right-aligned in a fixed slot in both modes: in `NOW`
    `TIME_WIDTH_CH`, 9 ch, the furniture's `UT` slot, so that the chart time takes one width
    wherever it stands (`-1000.00` and `+1000.00` are 8 characters, the furniture's 1 ch to spare
    kept); as observed `EMITTED_WIDTH_CH`, 11 ch, as before. `UT <time> yr` stays whole and `· AT`
    does not, in both modes. No comment in `styles.css` goes false: those of
    `.system-readout__value--fixed` and `.system-readout__beside .system-readout__value` hold in
    both modes, and the file is unchanged.
  - _B's reckoning_ (from look-74's figures; a `ch` of the readout's figures is 9.1 px). At
    1920 × 1080 the age's reading in `NOW` takes the row from observed's left edge, the wide `dd`
    starting in the first value column, with a slot 2 ch (18.2 px) shorter: for the same age text
    it ends at 1541 − 18.2 = 1522.8 px, 345.2 px inside the readout's edge at 1868, on one line.
    Look-74's two figures agree on the left edges: with `+0.00`'s 5 ch the half row's reading ends
    at 1867.2 and observed's ends at 1541 with 11 ch, so the half row starts 380.8 px right of the
    row, about half the readout, as its two-column grid sets it. The slot is fixed, so
    `UT -1000.00 yr` (8 of its 9 ch) ends where `UT +0.00 yr` does, and the longest `NOW` reading
    is set by the age alone: a five-character age, `0.150 Myr` at `UT -1000.00 yr`, ends at 1531.9
    (336.1 inside). The formatter writes a younger age with more figures (`0.0150 Myr`, six, 1541.0;
    `0.00150 Myr`, seven, 1550.1), and the row holds 37 characters more than a four-character age
    (345.2 ÷ 9.1), so every `NOW` reading stands on one line. At 1280 × 720 and 150%, one reading
    to a line, the reading breaks after `AT` as before: its first line is unchanged, and its time
    line is 4 ch (36.4 px) longer than the unslotted `UT +0.00 yr` and 2 ch shorter than
    observed's in its 11 ch slot, which look-74 found inside the column (53.8 and 59.4 px to
    spare), so it fits with at least 72.0 and 77.6 px, at one width for every chart time.
  - _Tests (21 new, 2 changed, as run)._ `spatial/labels.test.ts` (543 to 558): "keeps a label
    whole and 0.25 rem inside a view too narrow for it on either side of its mark", which pinned
    the hold for a mark in view, is now two: in its 160 px view the mark at x 80 draws no label,
    and a label at the top edge is held 0.25 rem inside it beside its mark; and a new block,
    "placeLabels on the compact chart's 256 px stage" (14): the centred selection with no label
    and none over its bracket, an unselected mark at the middle, the side or none at nine places
    on both ratios' edges, the selection 2 rem from the left edge on the right as before, the
    selection held back on both sides beside a destination whose label stands as with no
    selection (C1), and the destination's label taking no account of a label that would be held
    back. `spatial/SpatialView.test.tsx` (2, in a block of their own): a selection at the view's
    centre has no label element in a 256 px stage and its label in a 1036 px one.
    `displays/galaxy/SystemReadout.observed.test.tsx` (17 to 21): "keeps the age's slot in now,
    its time after a middle dot in no fixed slot" becomes "takes the readout's row for the age in
    now, its time in the chart time's slot", its text unchanged; new: the chart time at the clock
    window's edges, `-1000.00` and `+1000.00`, within `TIME_WIDTH_CH` (2), `NOT YET FORMED` taking
    the row, and a row without its observation, asked observed, in `NOW`'s slot. Every other
    existing test passes unchanged, among them "draws no label for a selection at the view's right
    edge whose other side would be held back across its own mark", "starts a pinned label held
    inside at the left 0.25 rem in", "draws no label for the selection where its other side is not
    clear either" and "keeps the age's time whole with its system's label and its unit" in both
    modes; no other test pinned the hold for a mark in view. Against the base's code the 11
    refusals (10 in `labels.test.ts`, the 256 px case in `SpatialView.test.tsx`) and the 4 slot
    tests fail, and the rest pass. The `spatial` and `displays/galaxy` suites: 56 files, 2391
    tests; the gate's other planned files (`App`, `displays/system`, `lib/serverLink`,
    `view/engine/engineBoundary`, `wasm/surfaceImports`): 20 files, 348 tests.
  - _Review._ TypeScript: a test asserted a label present with `queryAllBy*`, now `getBy*` through
    `overlayLabel` in a test of its own; `ageReading`'s unread `formed` removed. UX: the top and
    bottom edges, deferred finding (1), asked on the issue; two guide sentences proposed, deferred
    findings (2) and (3); and, before this task, (4) and (5); the `DISPLAY TIME` cell checked true
    against the code. Plan conformance: the galactic-axis case carried into the look below; the
    docs of the labels the destination's is placed against, and two phrases here, reworded.
  - _Deferred findings (2026-10-13 rule)._ (1) A selection whose centre lies just past the top or
    bottom edge, up to about 14 px for a class-4 mark at a ratio of 1, inside the band where
    neither side has room, keeps the hold, since `inView` reads the centre; and the canvas still
    draws the part of its symbol and bracket inside the view, which the held label runs across: at
    (128, -2) on the 256 × 262 px stage the label stands at x 4–142.88, y 4–21.5, the bracket's
    box at x 114–142, y -16–12, and the label cuts in and out as the mark crosses the edge. The
    left and right edges are safe, where a mark whose bracket reaches into the view always has a
    side. The brief keeps the hold for a mark outside the view by `inView`, so it is asked on the
    issue; the lane's lean is to hold only a mark whose bracket lies wholly outside the view (UX
    review's must-fix). (2) The guide's spatial-display bullets do not say that a label with room
    on neither side is not drawn, as its Views paragraph says of a view's; the UX review proposes
    "A mark's label that has room on neither side of its mark is not drawn, the selection's too,
    and the bracket, the list and the readout still name the mark; only the label of a selection
    whose bracket lies wholly outside the view is held `0.25rem` inside its edge" (main decides).
    (3) § Data states says "Loss of the server link makes every live value on the console stale at
    once", and only the rows of `DISPLAY TIME`, `BASELINE` and `PAD WIDTH` except the console's own
    values; the UX review proposes "A value the console keeps itself, such as the `DISPLAY TIME`,
    `BASELINE` or `PAD WIDTH`, is not a live value: it never goes stale, and on loss of the link
    stays in `--text` with no `S`" (main decides). (4) The age's value has no fixed slot, from 3 to
    7 characters (`123`, `4.57`, `0.150`, `0.00150`), so the time's slot still moves between
    selections, and in `NOW` as a young system's age crosses 0.1 or 1 Myr while the chart time
    steps (§ Numbers: "A field has a fixed width sized for its longest possible value"; before
    this task; UX review). (5) `formatAge` writes ages below 1 Myr in `Myr` (`0.150 Myr`), where
    § Numbers says "ages are in `kyr`, `Myr` and `Gyr`" (before this task; UX review). (6) A
    selection centred on the 256 px stage, as after centring the chart on it, now has no label on
    the chart; a place above or below its bracket, as the destination's has, would keep it named
    there (plan-conformance review; main's ruling chose not drawn). (7) The selection's other side
    never rescues a first side held back for a mark in view (A above); kept as briefed, it could be
    dropped from that path (TypeScript and plan-conformance reviews).
  - _For main's look._ The compact `LOCAL CHART` at 1280 × 720 and 150%, in `NOW` and `OBSERVED`:
    the chart centred on its selection, its bracket drawn and no label over its own mark or
    bracket; a label coming back as a cut once its mark leaves the band (95.12–160.88 px across at
    1 for a 16-character designation about a class-4 mark); every label of a mark with a side as
    in look-75; a selection in the band crossing the top or bottom edge (deferred finding (1)); the
    observer moved to the centre on the selection, the eye in the bracket with no label beside it
    (item 7); a destination beside a mark whose label has no side, which may now take a place that
    label's held box covered; the stage's heights, unchanged, against P06.T24.c's figures (262 and
    228 px at 100%, 266 and 232 at 150%), and its galactic-axis case (the chart centred on
    [0, 0, 3000] ly, 212 px in `OBSERVED` at 100% and 216 at 150%, from plan 06's P06.T24.c
    record); and B612's real widths against the 0.62 em estimate the band's edges rest on. The
    stacked page at 1920 × 1080: every label as in look-75. The readout at 1920 × 1080: `AGE` on
    its own row in `NOW` at `CHART TIME` +0.00 and -1000, one line, its time in its 9 ch slot,
    reckoned to end near 1523 for a four-character age, and a young system's longer age; `MASS`
    alone on its row, with nothing selected too; as observed, unchanged. The readout at
    1280 × 720 and 150%: the break after `AT` as before, the time's line one width, and no
    sideways scroll.

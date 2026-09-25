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
3. **The stated error.** `curvature_error` returns min(½ a s², 2R), with a = v_c² ÷ R from the
   potential tables at the source's galactocentric radius R, as a length and as an angle at the
   observer; the cap is the orbit's own size, which the quadratic passes where an orbit is short
   against the light time. It is zero for a member in plan 09's Kepler regime, whose orbit is
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
    Einstein radius possible on the sightline, plus `PAD_SPEED` times the window.
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
the cap, which never lets the error pass 2R. Acceptance:
`cargo test -p hyperion-sim observe::retarded`, and the `retarded` bench reports tens of nanoseconds
for drift.

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
- **Benches:** `retarded` (tens of nanoseconds), `range_observed_50ly`, `alerts_interactive`
  (galaxy-wide, all interactive classes; target under 200 ms), `awd_scan_cold` (seconds),
  `lens_walk_26kly`.
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

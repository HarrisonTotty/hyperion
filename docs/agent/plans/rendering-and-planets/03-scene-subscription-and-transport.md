# Plan R03: The Scene Subscription and Bulk Transport

- **Milestone:** Rendering milestone RM1 (a wireframe view at real scale, and the determinism
  checks), beside R01, R02 and R04.
- **Depends on:** galaxy plans [04](../galaxy-generation/04-server-and-protocol.md) (built: the
  request envelope, the outbound queue and its limits, the CPU pool),
  [12](../galaxy-generation/12-retarded-observation-alerts.md) (P12.T0–T2 and T4 built:
  `hyperion_sim::observe`; its subscription envelope, P12.T9, is not built and is built here if it
  still is not, Design note 1) and [14](../galaxy-generation/14-planetary-systems.md) (in progress:
  bodies, orbits, `system_bodies`, `body_detail` and the body cache are built), and the built galaxy
  plans 01, 03 and 11 (coordinates, frame selection, the star hierarchy). Of the R-plans, only
  R03.T13 waits, on R02.T8.a's `selectCameraFrame`, the one rule for which body is local (Design
  note 7); everything else needs no R-plan. R02 consumes this plan for generated scenes, and R06
  and R09 put their payloads on its binary frames.
- **Brainstorm sections covered** (in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)): the retarded-position
  paragraphs and leans of
  [The floating origin is already in the simulation](../../brainstorming/rendering-and-planets.md#the-floating-origin-is-already-in-the-simulation)
  (drawn positions as light-time plus aberration, the ship's local body geometric,
  `SystemTrajectory` and
  `retarded_in_system`); the one-scene lean of
  [Two deployments, one scene](../../brainstorming/rendering-and-planets.md#two-deployments-one-scene)
  (time and time rate on every push, client extrapolation); the local camera's report to the scene
  subscription in [The free camera](../../brainstorming/rendering-and-planets.md#the-free-camera);
  the bullets "The scene arrives as a subscription" and "Bulk payloads travel as binary frames" of
  [Runtime and code shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape);
  the tests "The scene is bounded by Knowledge" and "Two clients agree" of
  [Testing](../../brainstorming/rendering-and-planets.md#testing); the Decisions entry "The scene";
  [open questions](../../brainstorming/rendering-and-planets.md#open-questions) 10 (closed; built
  here) and 21; the scene-subscription sentences of step 1 of
  [Suggested order of attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack).
  From [the single-player brainstorm](../../brainstorming/single-player-experience.md): the tick of
  [The clock](../../brainstorming/single-player-experience.md#the-clock) (64 Hz, rates in powers of
  ten, pause as a rate of zero), "What the ship knows" under
  [The view outside](../../brainstorming/single-player-experience.md#the-view-outside), and the
  scene and JSON bullets of
  [Server and protocol](../../brainstorming/single-player-experience.md#server-and-protocol).

## Goal

When this plan is done a client can subscribe to the scene of a universe and receive, in JSON, the
bodies of the system its ship stand-in is in, as plan 14's records with their orbital elements, each
at the detail level granted for it, with the system's stars and the time and time rate on every
push; the
server re-sends a body when its `valid_until` passes or what the ship knows of it changes, sends a
whole system on arrival, a heartbeat each second, and craft at the 64 Hz tick once anything supplies
craft. Each client renders at the pushed time extrapolated by its own elapsed time times the rate,
reports each of its views' cameras to its subscription, and computes every body's drawn position as
the apparent position the ship sees, light time and aberration together, from the elements, checked
against the sim's own `retarded_in_system` by golden vectors; the ship's local body is drawn
geometrically at the present. The server can also answer a request with bulk binary frames of 256
KiB, within the brainstorm's 1 MB, correlated to the request and flowing under the outbound byte
budget without holding scene pushes behind them, and the client reassembles them. Two tests hold the
design to the brainstorm: no scene leaks what the ship does not know, wherever the cameras are, and
two clients of one scene place every body at the same position to within one push. The protocol
stays at version 2.

## Scope and non-goals

In scope:

- The subscription envelope (`subscribe`, `unsubscribe`, `ServerMessage::Notification`) to plan 12's
  design, if P12.T9 has not built it, with `SubscriptionTopic::Scene` as its first topic.
- The scene topic: its state and notifications, their cadence, conflation for a slow reader, and
  per-view camera reports.
- A provisional ship stand-in and scene clock per universe, since no session exists (Design
  note 2), set by a request and replaced by the session when there is one.
- `hyperion_sim::observe`'s in-system retarded evaluation and the trajectories of a system's bodies
  and stars; golden vectors of apparent positions.
- The client's scene store, clock extrapolation and apparent positions, and a `useScene` hook that
  R02's view consumes.
- Outbound binary frames: their header, chunking, the handler seam that returns bulk, streaming
  under `limits.rs`, cancellation, and the client's reassembly.
- Open question 21's ruling, pinned by tests; the entries this plan adds to plan 04's table of
  reserved kinds.
- The Knowledge-bound scene test and the two-clients-agree test.

Non-goals:

- The sky request (R06) and the coarse field's request and chunk contents (R09). Both ride this
  plan's binary frames; neither is built here. R09 extends the Knowledge-bound test to surveyed
  cells.
- Drawing anything. R02 owns the camera, its frame selection (body frames included), the
  projection, `BodyFixedPosition` and the `VIEW` display; this plan hands it a timed scene.
- The main screen: the client role in `Hello`, camera-selection commands and the scene's camera
  field (R07). Design note 4 leaves room for the field.
- Sessions, ship state, the 64 Hz loop and craft. The craft list is empty until a session supplies
  it; the push path for it is built and tested with a test source.
- The Knowledge overlay beyond a seam: until the sensors plan, the server grants the level asked,
  as plan 14's handlers do. The seam grants a level per body, and the wire carries it (Design
  note 13).
- Flight prediction. A craft's planned path is a draft field that the sessions plan and the flight
  model fill; this plan carries it and extrapolates in a straight line when it is absent (Design
  note 4). Until a flight model fills it, R11 sweeps along this plan's `predictedPath` and widens
  the sweep along a straight-line extrapolation by ½ g t² (its `extrapolationMargin`, R11 Design
  note 7), which bounds an unpowered arc.
- Inbound binary frames, which stay refused (`crates/hyperion-server/src/ws.rs`).
- Body rotation and orientation for drawing (plan 14's P14.T14, consumed by R02 and R07).
- Latency compensation beyond what the heartbeat gives, and client-side prediction of craft.

## Provides

Signatures are sketches, named precisely enough to be grepped.

### `hyperion_sim::observe` (in-system)

```rust
// observe/in_system.rs
pub struct SystemObserver { /* position: SystemPosition, velocity: SystemVelocity (m/s, galactic
    axes, relative to the barycentre), time: UniverseTime (within ±H) */ }
impl SystemObserver { pub fn new(position: SystemPosition, velocity: SystemVelocity,
    time: UniverseTime) -> Result<Self, BuildSystemObserverError>; }   // not finite, outside ±H

/// A position in a system's frame that is a pure function of time (brainstorm's proposal).
pub trait SystemTrajectory {
    fn position_at(&self, t: UniverseTime) -> Option<SystemPosition>;  // None: not present then
    fn velocity_at(&self, t: UniverseTime) -> Option<SystemVelocity>;
}
pub struct InSystemRetardation { /* observed, emitted: UniverseTime, light_time: Span,
    corrections: u8, geometric_then: SystemPosition, apparent: SystemPosition,
    residual: Span */ }
pub enum TraceInSystemError { NotPresentThen { emitted: UniverseTime },
    NotConverged { corrections: u8, last_change: Span } }
pub fn retarded_in_system(observer: &SystemObserver, source: &impl SystemTrajectory)
    -> Result<InSystemRetardation, TraceInSystemError>;
pub const IN_SYSTEM_LIGHT_TIME_TOLERANCE: Span;      // 1 ns, inclusive
pub const IN_SYSTEM_MAX_CORRECTIONS: u8;             // 10; typically 2–4, 7 at a system's reach

// observe/in_system/tracks.rs
pub struct BodyTrack<'a> { /* system: &PlanetarySystem, ctx: &SystemContext, index: BodyIndex */ }
pub struct StarTrack<'a> { /* hierarchy: &SystemHierarchy, star: BodyId */ }
impl SystemTrajectory for BodyTrack<'_> { .. }
impl SystemTrajectory for StarTrack<'_> { .. }
```

### Protocol (`hyperion_protocol`, mirrored in `@hyperion/protocol`)

Only if P12.T9 has not landed (Design note 1), the envelope, to plan 12's Provides:

```rust
RequestBody::Subscribe(SubscribeRequest { universe, topic: SubscriptionTopic })
RequestBody::Unsubscribe(UnsubscribeRequest { subscription: u32 })
ResponseBody::Subscribe(Subscribed { subscription: u32, state: SubscriptionState })
ResponseBody::Unsubscribe                                   // empty body
pub enum SubscriptionTopic { Scene(SceneSubscribeRequest) }      // P12.T9 adds Alerts(..)
pub enum SubscriptionState { Scene(SceneStateDto) }              // P12.T9 adds Alerts(..)
ServerMessage::Notification { subscription: u32, body: NotificationBody }
pub enum NotificationBody { Scene(SceneNotificationDto) }        // P12.T9 adds Alerts(..)
```

This plan's own (`crates/hyperion-protocol/src/scene.rs`):

```rust
pub enum FramePositionDto {                 // tagged by "frame"
    Galactic { position: GalacticPosition },
    System { system: SystemIdHex, offset_m: [f64; 3] },   // from the barycentre, galactic axes
    Body { body: BodyIdHex, offset_m: [f64; 3] },         // from the centre, galactic axes
}
pub struct KinematicsDto { position: FramePositionDto, velocity_m_s: [f64; 3], time: UniverseTime }
pub struct SceneClockDto { time: UniverseTime, time_rate: u32, state: SceneClockStateDto }
pub enum SceneClockStateDto { Running, Paused, WindowLimit }
pub struct CameraReportDto { view: u8, pose: KinematicsDto }
pub struct SceneSubscribeRequest { detail: DetailLevelDto, cameras: Vec<CameraReportDto> }
pub struct SceneShipRequest { universe, ship: KinematicsDto, time_rate: u32 }   // `scene_ship`
pub struct SceneCamerasRequest { subscription: u32, cameras: Vec<CameraReportDto> } // `scene_cameras`
pub struct SceneShipSet { clock: SceneClockDto }                    // `scene_ship`'s answer
// `scene_cameras` answers with an empty body, as `unsubscribe` does
pub struct SeenPositionDto { apparent_m: [f64; 3], emitted: UniverseTime }  // a body with no orbit
pub struct BodyGrantDto { body: BodyIdHex, level: DetailLevelDto,
    seen: Option<SeenPositionDto> }                                   // Design note 13
pub struct SceneSystemDto { system: SystemBodiesDto,   // `granted`: the level asked; each record
    grants: Vec<BodyGrantDto> }                        // degraded to its own grant, index order
pub struct SceneBodyDto { level: DetailLevelDto, record: BodySummaryDto,
    seen: Option<SeenPositionDto> }                                   // a re-sent body
pub struct SceneStateDto { sequence: u64, clock: SceneClockDto, ship: KinematicsDto,
    system: Option<SceneSystemDto>, craft: Vec<SceneCraftDto> }
pub struct SceneNotificationDto { sequence: u64, clock: SceneClockDto,
    ship: Option<KinematicsDto>, arrival: Option<SceneArrivalDto>,
    bodies: Vec<SceneBodyDto>, craft: Option<Vec<SceneCraftDto>> }
// later optional fields: R07's `main_screen`, R09's `surface_revisions` (Design note 4)
pub enum SceneArrivalDto { System { system: Box<SceneSystemDto>, tidal_radius_m: f64 }, NoSystem }
// tidal_radius_m: the system's FrameCandidate::tidal_radius at the arrival time (R02's ask)
pub struct SceneCraftDto { craft: String, hull: String, state: KinematicsDto,
    attitude: [f64; 4], angular_velocity_rad_s: [f64; 3],
    planned_path: Option<Vec<KinematicsDto>> }                        // a draft; Design note 4
```

In `crates/hyperion-protocol/src/bulk.rs`, built by R03.T10.a:

```rust
pub struct BulkManifestDto { chunks: u32, bytes: u64 }               // for R06's and R09's kinds
```

Binary frames: the header of Design note 10, documented in the protocol crate's module docs, with
no type (the crate holds wire types, and a binary frame is not JSON).

### Server (`hyperion_server`)

```rust
pub(crate) mod subscriptions { Subscriptions, SubscriptionId, PendingPush, Merge }
pub(crate) mod scene { SceneService, SceneClock, ShipStandIn, SceneCore, SceneDelta,
    SceneKnowledge, GrantAsked, CraftSource, NoCraft, CraftState,
    Clock, Ship,                                // the traits sessions implement (Design note 2)
    is_large_notification }                     // Design note 14
pub(crate) const SCENE_FRAME_ENTRY: f64;       // 0.9: enter a system's frame at ratio ≤ 0.9
pub(crate) mod bulk { encode_header, BinaryFrameHeader, chunk, Answer, BulkPayload }
pub fn tap_socket(tcp: &tokio::net::TcpStream); // the low-water mark, for every serve site
// limits.rs
pub const MAX_BINARY_FRAME_BYTES: usize;        // 262_144, header included (Design note 11)
pub const BULK_QUEUED_BYTES: usize;             // 262_144: one chunk in the queue at a time
pub const TCP_NOTSENT_LOWAT_BYTES: u32;         // 65_536, set on Linux (Design note 11)
pub const MAX_SUBSCRIPTIONS: usize;             // 4 a connection
pub const MAX_SCENE_CAMERAS: usize;             // 8 a subscription
pub const SCENE_HEARTBEAT: Duration;            // 1 s
pub const CRAFT_PUSH_INTERVAL: Duration;        // 15.625 ms, the 64 Hz tick
```

`TestClient::next_notification()` (R03.T5.b) and `TestClient::next_binary()` (R03.T10.b) in
`crates/hyperion-server/tests/common/mod.rs`.

In the sim, added by agreement in plan 03's file (R03.T7):
`galaxy::frame::candidate_at(galaxy, record: &SystemRecord, ship: &GalacticPosition, t) ->
Option<FrameCandidate>`, the candidate `frame_at` forms for one system, so that a caller can read
its ratio.

### Client

In `@hyperion/protocol` (`packages/protocol/src/`): `subscriptions.ts`
(`RequestClient::subscribe(topic) -> Subscription<T>` with `state`, `onNotification`,
`unsubscribe()`; `NotificationOf<T>`), `bulk.ts` (`parseBinaryFrameHeader`, `BulkAssembler`,
`RequestClient::handleBinaryFrame(data: ArrayBuffer)` and a `requestBulk` outcome carrying the
chunks).

In `apps/hyperion/src/renderer/src/lib/scene/`: `sceneWire.ts` (`toSceneModel`,
`applySceneNotification`, over `lib/system/bodiesWire.ts`'s `toSystemBodiesModel`), `sceneClock.ts`
(`renderTime(clock, receivedMs, nowMs): UniverseTime`), `craft.ts`
(`predictedPath(craft, untilS) -> ReadonlyArray<CraftPose>`: the planned path when the craft has
one, else its pose extrapolated in a straight line), `apparent.ts` (`apparentPosition`,
`sceneAt(model, observer, time, previousLocal?) -> SceneFrame` with each body's and star's
`geometricM`, `apparentM` and `emitted`, and for bodies `level` (its `DetailLevelDto`), `seen`
(true for a body placed by the server's `SeenPositionDto`) and `hillRadiusM` (`null` below
`mass_and_orbit`), and the ship's local body named), `cameraReports.ts` (`CameraReporter`, at most
4 Hz and on a frame change), `useScene.ts` (`useScene(requests, universe, cameras) -> SceneView`,
whose `frameAt(nowMs)` calls `sceneAt` at the render time; stale on link loss).

## Consumes

- **Galaxy plan 04:** the request envelope (`RequestBody`, `ResponseBody`, `REQUEST_KINDS`,
  `RequestError`, `ErrorCode::BadRequest` with `field`), its reservation of `notification`, of
  `response_part`'s rule that parts precede the terminal `response`, and of binary frames for bulk;
  its table of reserved kinds, which a new kind enters first; `requests::{Handler, Handlers,
kind, is_large}` (`is_large` is private today and becomes `pub(crate)` in R03.T5.a) and the
  connection task in `ws.rs`; `outbound::{Outbound, Held, Writer}` and
  `limits::{OUTBOUND_BYTES, OUTBOUND_QUEUE_FRAMES, WRITE_TIMEOUT, MAX_IN_FLIGHT_REQUESTS,
MAX_INBOUND_FRAME_BYTES}`; `compute::{CpuPool, Priority, CancelToken}`; `testing::Harness`;
  `TestClient`; `RequestClient` in `@hyperion/protocol`; `PROTOCOL_VERSION` and its rule
  (`crates/hyperion-protocol/src/lib.rs`, design note 15).
- **Galaxy plan 12:** `observe::{Observer, Trajectory, Retardation, light_time}` as the model for
  the in-system functions, and P12.T9's design of `subscribe`, `unsubscribe`, `Subscribed`,
  `SubscriptionTopic`, `SubscriptionState`, `ServerMessage::Notification`, the unknown-subscription
  refusal and `TestClient::next_notification()`; the precedent of Design note 9 (a client-set
  observer until sessions).
- **Galaxy plan 14:** `PlanetarySystem::{position_at, body_at, snapshot_at}`, `SystemContext`,
  `BodyIndex`, `BodyOrbit::valid_until`; on the wire `SystemBodiesDto`, `BodySummaryDto`,
  `BodyOrbitDto`, `DetailLevelDto`, `SectionDto`; the server's body cache (`AppState::bodies`,
  `requests::system::bodies_of`, private today and made `pub(crate)` by R03.T8.a) and converter
  (`convert::planetary::system_bodies`), with `BodyRecord::degrade(level)` for a body's own grant;
  `hill_radius` (`planetary/derive/limits.rs`); the client's `toSystemBodiesModel`
  (`lib/system/bodiesWire.ts`), `layoutHierarchy` (`lib/system/hierarchy.ts`) and
  `composePosition`, `positionAt` and `stateAt` (`lib/orbit.ts`). **Asked of plan 14** (Design
  note 8): `PlanetarySystem::state_at(ctx, index, t) -> Result<Option<(SystemPosition,
SystemVelocity)>, ResolveBodyError>`, a body's velocity
  beside the position `position_at` already gives, sharing its arithmetic.
- **Galaxy plan 11:** `stellar::multiplicity::{SystemHierarchy, star_positions_at}`. **Asked of
  plan 11**: `star_states_at(h, t, out)`, the same walk with each star's velocity.
- **Galaxy plans 01 and 03:** `coords::{Frame, SystemPosition, SystemVector, SystemVelocity,
BodyPosition, GalacticPosition}`, `time::{UniverseTime, Span, ClockWindow}`,
  `galaxy::frame::{frame_at, FrameCandidate, FRAME_HYSTERESIS}`, the galactic centre's
  `galaxy.potential().tidal_radius(m, p)`, and `units::consts::GRAVITATIONAL_CONSTANT`. **Asked of
  plan 03**: `galaxy::frame::candidate_at` (under Provides), added by R03.T7 in `frame.rs` by
  agreement, with the existing rule untouched. `observe::retarded`'s private `before` becomes
  `pub(super)` for the in-system sibling (R03.T2).
- **R02:** `selectCameraFrame` (R02.T8.a), the TypeScript twin of `select_body_frame`, by which
  `sceneAt` names the ship's local body (R03.T13 only). R02 consumes `useScene` and `sceneAt`, and
  owns the camera whose poses `CameraReporter` sends. Asked of R02, which applies it in its Consumes
  of this plan (and of R07 and R08, which read the same rule): only the ship's local body is drawn
  geometrically; a free camera's other local body is drawn at its apparent position like every other
  body (Design note 7). R02 still chooses the camera's frame, for which `sceneAt` gives each body's
  Hill radius, as R02 asks. Asked by R02 and provided here: `tidal_radius_m` on
  `SceneArrivalDto::System`, which R02.T17 reads for the free camera's clamp.
- **R08 and R11** read two things `sceneAt` and the scene model give for them: each body's granted
  `DetailLevelDto` (R08's rebuilds on a level change) and `predictedPath` of a craft (R11's
  `couldTouch`).

## Design notes

1. **The envelope is built here if plan 12 has not built it, to plan 12's design.** P12.T9 needs
   P12.T8's alert service, which needs T5 and T7, none of them built, so the scene is likely to
   need `subscribe` first, as the brainstorm foresaw ("the view either waits on plan 12's envelope
   work or builds it"). R03.T5 builds the envelope exactly as plan 12's Provides and P12.T9 describe
   it, with `Scene` as the only topic, and records in plan 12 that P12.T9 then adds only its
   `Alerts` topic, state and notification body, `alerts_observer` and `alerts_acknowledge`. If
   P12.T9 has landed first, R03.T5 shrinks to adding the `Scene` variants. Either way the kind
   strings stay plan 12's in plan 04's table; this plan adds its own two (R03.T1). The Rust types
   go in `crates/hyperion-protocol/src/envelope.rs` beside `ServerMessage`, not in plan 12's
   `alerts.rs`, which then holds only the `Alerts` payloads.

   One departure from P12.T9, recorded in R03.T1's note there: P12.T9 says
   `RequestClient::handleServerMessage` must leave a `notification` unconsumed so that a
   subscription helper outside it sees it. Here the helper lives on `RequestClient` itself
   (`packages/protocol/src/subscriptions.ts`, re-exported from `index.ts`), so
   `handleServerMessage` routes the notification to its subscription and returns `true`, and the
   link's own switch never sees one. One owner for routing is simpler than two, and a notification
   for an unknown subscription is consumed and dropped in one place.

2. **A ship stand-in and a scene clock, per universe, until sessions exist.** The brainstorm's scene
   is about the ship, and its time is the session's; neither exists. Plan 12's Design note 9 met the
   same gap with an observer the client sets. Here the stand-in must be shared, because two clients
   of one scene can agree only on one clock and one observer, so it is per open universe, held by
   the server: a ship pose (position, velocity) at a time, moving in a straight line at that
   velocity in its frame, and a clock at that time running at a rate of 0 or a power of ten from 1×
   to 100,000× (the single-player brainstorm's [The clock](../../brainstorming/single-player-experience.md#the-clock)).
   One request, `scene_ship`, sets both; the last one the server accepts stands, and every
   subscription of the universe is pushed the change. The clock runs on the server's monotonic
   clock and stops at the clock window's edge in state `window_limit`. None of this is saved. When
   sessions exist the session's clock and ship replace it, `scene_ship` answers `bad_request` for a
   universe with a session, and the service's core, which takes a `Clock` and a `Ship` by trait
   (`scene::{Clock, Ship}`, which `SceneClock` and `ShipStandIn` implement), does not change.
3. **What the scene holds.** The scene's system is the ship's frame by plan 03's `frame_at`, with
   the previous scene system as `current`. That rule's `FRAME_HYSTERESIS` acts only between rival
   systems: a ship that leaves its current system's sphere (distance ÷ tidal radius above 1) is
   re-ranked as if it had no frame, so a stand-in drifting across one sphere's own boundary would
   flicker in and out (`crates/hyperion-sim/src/galaxy/frame.rs`, `select_frame`; the README's
   brainstorm correction for "The frame-change scene"). The scene core adds a Schmitt band at the
   boundary, as R02's Design note 6 does for body frames: the scene enters a system that `frame_at`
   names only once the ship's ratio to that system's sphere is at most `SCENE_FRAME_ENTRY` = 0.9,
   R02's `BODY_FRAME_ENTRY`, and leaves it only when `frame_at` no longer names it (a ratio above
   1). The ratio comes from `galaxy::frame::candidate_at`, the candidate `frame_at` forms for one
   system, which R03.T7 exposes in plan 03's file; the arrival states that candidate's tidal
   radius at the arrival time as `tidal_radius_m`, to which R02's free camera is clamped (R02
   Design note 7). In a system the scene holds plan 14's
   `SystemBodiesDto` for it at the scene time, built at the level asked, and each body's record
   degraded to its own grant (Design note 13): every body, with its orbit as `BodyOrbitDto`
   (parent, elements, `valid_until`) where its grant reaches `mass_and_orbit`, and the system's
   stars and hierarchy in `hosts`, which the local star's disc needs (R06). A system's bodies are
   some tens to about a hundred, so the whole system is sent, and the cameras do not thin it. In the
   galactic frame the scene holds no system. The star field
   is not in it: the sky is R06's request. Craft are the ship's contacts in the scene's reach,
   none until sessions and sensors.
4. **The state and its pushes.** A subscription is answered with the whole state; each notification
   carries only what changed, and always the clock (time, rate, state) and a `sequence`. The
   sequence is numbered when a notification is sent, not when a change is merged, so it grows by
   exactly one per notification sent, conflated or not (Design note 5); since the socket loses
   nothing, a gap or a step back is a server bug, which the client reports as an error and then
   resubscribes. What changes, and when: the system, on arrival and on leaving (`arrival`); a body,
   when the scene time passes its `valid_until` or what the ship knows of it changes, and a body
   placed by a seen position at every heartbeat (Design note 13) (`bodies`, whole `SceneBodyDto`s,
   latest wins); the ship stand-in and the clock, when `scene_ship` changes them;
   craft, at every tick while any are in the scene (`craft`, the whole list); and a heartbeat of the
   clock alone every `SCENE_HEARTBEAT`, 1 s. The brainstorm's figures, 250–300 bytes a body and 400
   a craft, about 0.25 MB/s for 100 bodies and 10 craft (Runtime and code shape), are re-measured by
   R03.T15: plan 14's `BodySummaryDto` with its `bulk` section is larger than a bare `BodyOrbitDto`,
   perhaps 700–900 bytes, which changes the arrival's size and not the rate, since bodies are pushed
   only on change. The state leaves room for R07's camera field (`main_screen`) and R09's
   `surface_revisions` (a revision per body), each an optional field added by its plan, which moves
   no version. `SceneCraftDto` is a draft, the least a renderer needs of a craft (a hull
   definition's key, a pose and its rates), and belongs to the sessions plan, which may reshape it
   freely while nothing sends it. Its optional `planned_path` is the flight computer's predicted
   path as poses at stated times, which the sessions plan and the flight model fill; R11's
   `couldTouch` sweeps a descending craft's footprint along it. The client's
   `predictedPath(craft, untilS)` returns it where present and otherwise extrapolates the pose in a
   straight line at its velocity, which is exact for nothing but is the only honest guess without
   the flight model; R11 widens its sweep for that case.
5. **A slow reader gets the latest scene, not a backlog.** Pushes at 64 Hz to a client that has
   stopped reading would fill plan 04's queue and then its byte budget. Each subscription therefore
   keeps one pending push, into which every new change is merged before it is queued: the clock, the
   ship and the craft list are replaced by the latest, bodies are merged by ID with the latest
   winning, and an `arrival` replaces everything before it, bodies included. The connection queues
   the pending push only when the outbound queue has room for it (`Outbound::has_room_for`), never
   holds it as it holds a finished request, and otherwise leaves it pending, so a slow reader
   receives fewer, fuller notifications and the server holds one per subscription. A merge never
   drops a body change, which is what makes it safe: a body's latest record supersedes its earlier
   ones. The write timeout still closes a reader that stops altogether.
6. **Cameras bound the scene; they are never ship state.** Each view reports its camera, one
   subscription carrying up to `MAX_SCENE_CAMERAS` (8), through `scene_cameras` (and in the
   `subscribe` request), as plan 12's `alerts_observer` reports its observer. The client sends a
   report at most at 4 Hz and at once on a change of frame; each is a pose in a frame, its velocity
   and its time. The server keeps the latest per view, refuses a camera outside the scene's reach
   (`bad_request` naming `cameras`: in a system, a position outside the system's sphere of influence
   or in a frame of another system or of a body not in the scene; in the galactic frame, one in any
   system's frame, since the ship is in none), and will use them to bound what cannot all be sent,
   which will be the contacts drawn as hulls; R09 sends every surveyed cell (at most about 12 MB)
   and does not bound them by camera (R09 Design note 19). Poses are
   neither saved nor echoed to other subscriptions.
7. **Drawn positions are the ship's apparent positions, computed on the client from the elements**
   (researched 2026-09-29). The observer enters only at reception: τ solves |x_B(t − τ) − x_o(t)| =
   c τ, and aberration uses the observer's velocity at reception, v_o(t). The brainstorm's
   x_B(t − τ) − x_cam(t − τ) equals this only for an observer moving uniformly; for one that
   accelerates, as a ship in orbit does, the two differ by ½ a τ², an angle of a τ ÷ 2c, 1.5 × 10⁻⁵
   rad for a ship in low orbit seeing a body 2 au off: a thirtieth of a pixel at 1080p across 60°,
   15 pixels in a telescopic view of 10⁻⁶ rad a pixel. SPICE's "LT+S" and "CN+S" take the observer's
   state at the observation epoch for both the light time and the stellar aberration (NAIF
   `spkezr_c`, `spkaps_c`, `stelab_c`, and the Aberration Corrections Required Reading's eq. 1), and
   NOVAS does the same (Bangert et al. 2011, _User's Guide to NOVAS C3.1_, pp. C-16–C-17); the
   Explanatory Supplement's structure is the same, from memory, its section numbers to be re-checked
   before a doc comment cites them. The aberration is the exact special-relativistic one, a Lorentz
   boost of the emission event, which costs one square root: with r = x_B(t − τ) − x_o(t), β = v_o ÷
   c and γ = 1 ÷ √(1 − β²), r′ = r + γ v_o τ + [γ² ÷ (c² (γ + 1))] (r · v_o) v_o, and the apparent
   point is A_B = x_o(t) + r′. It points along the relativistically aberrated direction and equals the
   first-order r + v_o τ to O(β²); the first-order form's direction error, β² ÷ 4 at most, would
   reach a telescopic pixel near 600 km/s, and a ship may reach several hundred km/s, and some
   2,000–2,800 km/s (about 0.01 c) orbiting a white dwarf. The light-time solve stays Newtonian in the
   system frame, as SPICE's and NOVAS's do. The observer is the ship (its stand-in until sessions),
   never the camera, as the free camera's label says; each view then differences A_B against its own
   camera, the camera's own velocity and light time never entering.

   The iteration (researched 2026-09-29) starts from τ₀ = `light_time`(|x_B(t) − x_o(t)|), the
   present distance. Correction k ≥ 1 evaluates the source again, τ_k = `light_time`(|x_B(t −
   τ_{k−1}) − x_o(t)|), and δ_k = |τ_k − τ_{k−1}| on the rounded values. It stops at the first k
   with δ_k ≤ 1 ns (inclusive: τ is rounded to the nanosecond by plan 12's `light_time`, and a
   strict test could cycle between two adjacent nanoseconds), or, from k = 2, at the first δ_k ≥
   δ_{k−1}, and returns τ_k with `corrections` = k, the confirming correction counted; past a cap of
   10 it returns `NotConverged`. The second rule is needed far out: beyond about 10⁴ au an `f64` τ
   steps by 15–30 ns (its ulp is 2.2 × 10⁻¹⁶ τ; the distance's ulp at 2.7 × 10⁵ au is 8 m, 27 ns),
   and for an approaching source the rounded map has, with a probability of about β_r, no fixed
   point and alternates between two values; a true contraction strictly shrinks, so a change that
   stops shrinking is rounding noise. For uniform radial motion the map is linear and δ_k = τ₀ β_r^k,
   β_r the source's radial speed over c; the observer's velocity does not enter. Purely transverse
   motion contracts by about β_t² and needs about two. The worst, radial, cases:

   | Source                 | τ₀           | Last change above 1 ns | Corrections   |
   | ---------------------- | ------------ | ---------------------- | ------------- |
   | 10 au, 30 km/s         | 4,990 s      | δ₃ = 5 ns              | 4             |
   | 100 au, 250 km/s       | 4.99 × 10⁴ s | δ₄ = 24 ns             | 5             |
   | 10⁴ au, 300 km/s       | 4.99 × 10⁶ s | δ₅ = 5.0 ns            | 6             |
   | 10⁴ au, 800 km/s       | 4.99 × 10⁶ s | δ₆ = 1.8 ns            | 7, at times 6 |
   | 2.7 × 10⁵ au, 800 km/s | 1.35 × 10⁸ s | δ₆ = 49 ns             | 7 (or 8)      |

   So two to four corrections are typical within a system's planets, a hot Jupiter at 100 au needs
   five, and a system's reach seven, all within the cap. The brainstorm's "at most three, as SPICE's
   CN does" fails in the game's own systems (SPICE's three iterations are for Solar System speeds,
   `spkezr_c`). A fixed point is kept rather than Newton's method, which fails when τ spans many
   orbits of a close pair. Sources: Banach's fixed-point analysis of τ ↦ |x_B(t − τ) − x_o| ÷ c, whose
   Lipschitz constant is |v_B · r̂| ÷ c; `light_time` in `crates/hyperion-sim/src/observe/retarded.rs`,
   which adds 0.5 ns and floors.

   For a source and an observer moving together at a common velocity u, the Newtonian light-time
   solve and the Lorentz aberration combine exactly: with r = r₀ − u τ and κ u² = γ − 1 for
   κ = γ² ÷ (c² (γ + 1)), the τ terms cancel and r′ = r₀ + (γ − 1)(r₀ · û) û, the rest-frame
   separation, independent of τ and so of its rounding (the Lorentz boost of Jackson, _Classical
   Electrodynamics_, 3rd ed., eq. 11.19). Its angle to the present vector is at most (γ − 1) ÷ 2 ≈
   β² ÷ 4: 2.5 × 10⁻⁹ rad at 30 km/s, 2.5 × 10⁻⁵ rad at 3,000 km/s. The brainstorm's "equals the
   present relative position" for a comoving camera holds to that.

   Only the ship's local body, where τ is milliseconds, is drawn geometrically at the present, in its
   own frame, so that terrain and collision agree; its time-varying states are still drawn at their
   emitted times. Which body that is follows one rule for the camera and the ship alike, R02's
   (its Design note 6): the innermost body whose Hill sphere, at pericentre, holds the ship, entered
   at a ratio of at most `BODY_FRAME_ENTRY` = 0.9 and left above 1, with the galaxy's rank among
   overlapping siblings. `sceneAt` applies it through R02.T8.a's `selectCameraFrame` to the ship
   stand-in's geometric present position, taking the previous frame's local body as current, so a
   body whose terrain is streamed for a landing ship is its local body by the same rule, and the
   scene and the view can never name different bodies.
   A free camera's local body, if it is another, is drawn apparent like every other body, with its
   orientation at the emitted time: drawn geometrically among apparent neighbours it would sit, for a
   camera at Jupiter with the ship at Earth, some 80,000 km off its moons' frame, about a fifth of
   Io's orbit. The brainstorm's "the camera's local body" is read as the ship's. The system's stars
   are retarded by the same code (`StarTrack`): a companion at 1 au about a 2 M☉ pair, seen from 5 au,
   moves 5 × 10⁴ km in the light time, 70 telescopic pixels. `sceneAt` returns, for every body and
   star, both positions and the emitted time, and names the ship's local body.

   The brainstorm's 10 m figure is the cost of drawing the ship's local body apparent, not of light
   time alone: for a planet at rest and a ship 400 km above at 7.7 km/s across the line of sight,
   light time alone moves nothing and light time with aberration displaces the planet by
   Δv × 1.334 ms ≈ 10.27 m, whatever frame the velocities are taken in; light time alone is
   frame-dependent (in the system frame, with the planet at 30 km/s, about 40 m). Its tenth of a
   pixel holds: 16 km/s ÷ c = 5.34 × 10⁻⁵ rad, 0.098 of a 5.45 × 10⁻⁴ rad pixel.

   The evaluation runs on the client every frame, because the light time changes continuously and
   the elements arrive rarely; that is drawing, as plan 14's D18 already has the client propagate
   orbits. A warm start from the previous frame's τ brings it to about one correction, and a few
   hundred bodies cost a few thousand Kepler solves, well under a millisecond. The client mirrors the
   sim exactly where it matters: τ rounded to the nanosecond, the emitted time formed in
   `UniverseTime` as t less a `Span`, the same stop rules and cap, norms as a sum of squares and a
   square root (never `Math.hypot`). The sim's `retarded_in_system` is the reference, and its golden
   vectors pin the client (researched 2026-09-29). The two sides agree far better than the orbit
   golden's 10⁻⁹ (`lib/orbit.test.ts`): the mean anomaly is reduced by an exact remainder on both
   (`fractionOfPeriod` and `reduceToHalfTurn` in `lib/orbit.ts`, bit for bit with the sim's), the
   basic operations and the square root are correctly rounded on both (IEEE 754-2019; ECMAScript
   §6.1.6.1), and V8's `Math.sin`, `Math.cos` and `Math.cbrt` and the `libm` crate are ports of the
   same fdlibm, at worst about 1 ulp apart, which Kepler's equation amplifies by 1 ÷ (1 − e cos E).
   Some 10⁻¹⁶ to 10⁻¹⁵ of the magnitudes is expected, perhaps 10⁻¹⁴ near e = 0.99. With
   Σ = |x_B(t − τ)| + |x_o(t)| in system-frame magnitudes, the bound is

   |A_client − A_sim| ≤ 1 mm + 10⁻¹⁴ Σ + (|v_B| + γ |v_o|) |Δemitted|,

   about 0.3 m at 100 au (Σ ≈ 3 × 10¹³ m), 3 × 10⁻⁹ rad seen from 10⁵ km, a three-hundredth of a
   telescopic pixel; the 1 mm floor covers an `f64`'s step, about 1 mm at 30–60 au and 2 mm at
   60–120 au, so a bound on the apparent vector alone could not be met far out. The emitted times
   are not required equal: a difference δ in |r| moves τ by δ ÷ c, and the sides may round to
   adjacent nanoseconds, so |Δemitted| ≤ 1 ns + ⌈(1 mm + 10⁻¹⁴ Σ) ÷ c⌉ ns, which also covers τ's
   own `f64` step far out. Absolute system-frame coordinates step by 8 m at a system's reach, so an
   observer within 10⁵ km of a source there sees about 10⁻⁷ rad of noise in the sim itself, which
   no client bound can beat. R03.T13 first measures the agreement and pins at the larger of this
   bound and ten times what it measures, and fails if the measurement exceeds 10⁻¹² Σ, which would
   be a bug to fix rather than a bound to widen. The coefficient 10⁻¹⁴ is of medium confidence and
   that measurement settles it.

8. **The in-system functions are built here, in plan 12's module; velocities are asked of plans 14
   and 11.** Plan 12 has no task for `SystemTrajectory` or `retarded_in_system`, and plan 14 none
   for a body's velocity, so this plan builds the first two in `observe/in_system.rs` beside plan
   12's galactic ones. The light-time iteration needs positions only, which plan 14's `position_at`
   and plan 11's `star_positions_at` already give, so `BodyTrack` and `StarTrack` take their
   positions from those, bit for bit. The velocity, which the brainstorm's `SystemTrajectory`
   includes and later readouts need, is asked of plan 14 (`PlanetarySystem::state_at`) and plan 11
   (`star_states_at`); if either has not landed when R03.T3 runs, R03.T3 adds it in the owning file
   by agreement, with the position half left as the existing code path, and records it in that
   plan's as-built notes, and the owning plan's goldens prove its output unchanged. A body not
   present at the emitted time (not yet formed, or destroyed within the light time) is
   `NotPresentThen`. Physically, a body destroyed less than a light time ago is still seen, since
   its light is still arriving; the scene does not model that. It sends each body's current record
   only, so the client leaves such a body out as soon as its record says it is gone, and draws a
   body whose elements changed at a `valid_until` with its new elements at the emitted time. The
   error lasts at most one light time and is accepted under Risks, with the fallback there.
9. **Time on the client.** Every push states the scene time and rate. The client keeps the latest
   with the `performance.now()` at which it arrived and renders at t + (now − received) × rate, as a
   `UniverseTime` of whole seconds and nanoseconds so that a thousand years of seconds keep their
   nanoseconds (an `f64` of seconds from the epoch resolves only about 4 µs at 10³ years). Two clients
   of one scene then differ by the delivery of the latest push, the brainstorm's bound; the 1 s
   heartbeat keeps a local clock's drift from accumulating. A paused clock or one at the window's
   limit does not advance.
10. **Binary frames: a fixed header, chunks before the terminal response.** Each outbound binary
    frame is a 24-byte header then a payload, little-endian: bytes 0–3 the magic `HYPB`, 4 a format
    of 1, 5 reserved as 0, 6–7 the header's length (24), 8–11 the request ID, 12–15 the chunk's
    index from 0, 16–19 the chunk count, 20–23 the payload's length; frames are at most
    `MAX_BINARY_FRAME_BYTES`, 262,144 bytes with the header, within the brainstorm's "at most 1 MB"
    (Design note 11). A request answered in bulk sends every chunk in order and then its terminal
    JSON `response`, as plan 04 has parts precede the terminal response; the response body carries a
    `BulkManifestDto` of the chunk count and total bytes, which the client checks against what
    arrived. There is no checksum: TCP and the WebSocket framing already guard the bytes, and the
    count and length checks catch the server's own bugs. A cancelled or failed request stops its
    chunks and ends in `cancelled` or its `request_error`, and the client drops what it had. The
    request's in-flight slot is held until its terminal frame is sent. Researched 2026-09-29: axum
    0.8.9's WebSocket, on tungstenite 0.29.0 (the versions in `Cargo.lock`), has size limits on
    incoming messages only, which `ws.rs` already lowers, and writes each outbound message as one
    final frame, with no fragmentation and no outbound limit (tungstenite `src/protocol/mod.rs`);
    Chromium's WebSocket refuses a received message only above `wtf_size_t`'s maximum, about 4 GiB
    (`third_party/blink/renderer/modules/websockets/websocket_channel_impl.cc`), and delivers one
    socket's messages in order, text and binary together, so chunks stay before their response. The
    client sets `binaryType = "arraybuffer"` (WHATWG WebSockets Standard) at once on creating the
    socket, before any message can arrive; the default `blob` would need an asynchronous read per
    chunk.
11. **Bulk never holds more than one small chunk ahead of a scene push, and the kernel holds little
    more** (researched 2026-09-29). Plan 04's queue is one FIFO under a 16 MiB budget, and a frame
    larger than the budget is sent once the queue is empty (`crates/hyperion-server/src/limits.rs`),
    so a 15 MB field in one frame would hold every push behind it, as the brainstorm warns. A
    WebSocket message cannot be interleaved with another (RFC 6455 §5.4), so a push waits behind the
    rest of the chunk being written, the unsent bytes of the kernel's send buffer, which Linux
    autotunes up to `tcp_wmem`'s maximum of up to 4 MB, and the bytes in flight. Three bounds follow.
    Chunks are `MAX_BINARY_FRAME_BYTES`, 262,144 bytes with the header, well within the brainstorm's
    "at most 1 MB". Each carries 262,120 payload bytes after the 24-byte header, so a payload of
    L bytes is ⌈L ÷ 262,120⌉ chunks: 58 for a field of 15 × 10⁶ bytes, 61 for one of 15 MiB
    (15,728,640 bytes). They are queued one at a time, the next only once
    the bulk bytes queued fall under `BULK_QUEUED_BYTES`, one chunk, while text frames queue as
    before. And on Linux every accepted socket gets `TCP_NOTSENT_LOWAT` of 65,536 bytes
    (`TCP_NOTSENT_LOWAT_BYTES`), under which the kernel adds no new buffers once that much is unsent
    (kernel `ip-sysctl` documentation; Cloudflare found 16 KiB kept connections fully used, P. Meenan
    2018, "HTTP/2 Prioritization with NGINX"). It is set without `unsafe` in this workspace: socket2
    0.6.5, already in the tree through tokio and added as a direct dependency with its `all` feature,
    has `Socket::set_tcp_notsent_lowat` for Linux and Android, reached through
    `socket2::SockRef::from(&tcp)` in axum 0.8.9's `ListenerExt::tap_io`. One library function,
    `hyperion_server::tap_socket`, does it, and all three places that call `axum::serve` use it:
    `main.rs`, the unit-test `Harness` in `src/testing.rs` and the integration harness in
    `tests/common/mod.rs`, so every scene and bulk test runs with the option set. A failure is
    logged at `warn` and the connection goes on. socket2 does not expose it on
    macOS, and Windows has none, so there the chunk bound alone holds. At 40 Mbit/s of Wi-Fi a push
    then waits about 52 ms behind a chunk and 13 ms behind the kernel, plus the bytes in flight, under
    the main screen's 100 ms loop; at 1 Gbit/s about 2 ms. With 1 MB chunks it would be over 200 ms
    however the kernel is tuned. A chunk takes about 2 s at 1 Mbit/s, inside the 10 s write timeout.
    Loopback drains a megabyte in microseconds, so R03.T10.b measures on an emulated slow link: a test
    reader with a fixed 64 KiB receive buffer (socket2's `set_recv_buffer_size`, which turns off
    receive autotuning) reading through a 5 MB/s token bucket, with the low-water mark on and off.
    The renderer must not stall its own receive path either, since Chromium's backpressure reaches the
    TCP window: chunks are kept as `ArrayBuffer`s and parsed when complete, by R06 and R09 off the
    main thread.
12. **Open question 21: no version bump.** `lib.rs`'s rule bumps `PROTOCOL_VERSION` for a removed or
    changed message and not for an added kind or optional field, because an older peer refuses what
    it does not know. The first `notification` and the first binary frames are additions of the same
    kind: the server sends a notification only on a subscription the client opened, and binary frames
    only in answer to a request whose kind asks for bulk, so a version 2 client that never sends
    those requests never receives either; a newer client asking an older server gets `unsupported`.
    Today's client would in any case drop both harmlessly: `connection.ts` ignores non-string data,
    and `RequestClient::handleServerMessage` has no case for an unknown `type`. The version stays 2,
    and R03.T1 pins it with a test that a connection which subscribes to nothing and asks for no bulk
    receives only text frames of the existing types. This is the plan's ruling on a question the
    brainstorm left open; it reverses if either message is ever sent unasked. The ruling is a
    brainstorm revision like the README's corrections, drafted for the owner's revision pass by
    R03.T1.
13. **Knowledge as a seam, a level per body.** The scene is built through a `SceneKnowledge`
    trait: the level granted for each body and the craft that are contacts. Production uses
    `GrantAsked`, the level asked for every body and no craft, as plan 14's handlers grant today;
    the sensors plan replaces it. Plan 14's `SystemBodiesDto` has one `granted` for the whole list,
    and its converter degrades the whole snapshot to it
    (`crates/hyperion-server/src/convert/planetary.rs`, `system_bodies`), so a mixed grant cannot
    ride that field. The scene therefore builds the system at the level asked, degrades each body's
    record to its own grant with plan 14's `BodyRecord::degrade`, which never adds detail, and
    carries the grants beside it: `SceneSystemDto { system, grants }`, one `BodyGrantDto` per body
    in index order, and every re-sent body as a `SceneBodyDto` with its `level`. The system's own
    `granted` then states the level asked, the most any record holds, and nothing in plan 14's DTO
    changes.

    A body granted only `contact` has no orbit and no mass on the wire, only its position at the
    record's time (plan 14's `BodySummaryDto`), so the client can neither propagate nor retard it.
    For such a body the server evaluates it itself: the core runs `retarded_in_system` from the
    ship stand-in at the push's time over the body's `BodyTrack` and sends the result as
    `seen: SeenPositionDto { apparent_m, emitted }`, which is what a sensor contact is, a direction
    and range as seen. It re-sends such bodies at every heartbeat, so a contact is at most 1 s of
    scene time stale, which at 100,000× is a day of motion and is labelled as a contact by the
    view. The client draws a seen body at `apparent_m` without extrapolation, gives it no Hill
    radius (`null`), and never computes a geometric position for it. Hill radii need the body's mass
    and its orbit, so they are `null` below `mass_and_orbit` as well. The Knowledge-bound test
    drives the scene with a restrictive fake and places cameras anywhere, so that the rule is tested
    before anything restricts it.

14. **Size classes and serialisation.** A scene state or an arrival holds a whole `SystemBodiesDto`,
    so `Subscribed` for the scene is a large body (`requests::is_large`, which inspects the
    subscription's state) and is serialised on the pool. A notification is not a `ResponseBody`, so
    it has its own rule, `scene::is_large_notification`: one carrying an `arrival` is serialised on
    the pool, the rest on the runtime. `scene_ship`, `scene_cameras` and `unsubscribe` are small.

## Tasks

T1 comes first, and T5.a waits for the owner's sign-off that T1 asks for. T2 and T3 are a chain in
the sim and can run beside everything on the protocol. T4 precedes T5, whose subtasks run in
order; T6 follows T5.a; T7 follows T3 and T6, since it evaluates contacts with `BodyTrack`; T8
follows T5.b and T7; T9 follows T8. T10.a (binary frames) can start after T1 and runs beside
T4–T9; T10.b follows T5.b and T10.a, and its heartbeat measurement follows T8.a; T11 follows
T10.a. T12 follows T5.c; T13 follows T3, T12 and R02.T8.a; T14 follows T8, T11 and T13. T15
closes.

### R03.T1 Reconcile, reserve the kinds, rule question 21

Check what exists: whether P12.T9 has landed (`SubscribeRequest`, `ServerMessage::Notification` in
`crates/hyperion-protocol/src/envelope.rs`), whether plan 14's `state_at` and plan 11's
`star_states_at` exist, and the names under Consumes against the tree; record the findings in this
plan. Edit plan 04's table of reserved kinds
(`docs/agent/plans/galaxy-generation/04-server-and-protocol.md`,
"Extending the convention") with a row for this plan, `scene_ship` and `scene_cameras`, and a note
that `subscribe` and `unsubscribe` stay plan 12's and may be built by R03 to plan 12's design; add
the matching note to plan 12's P12.T9, which also records Design note 1's departure (the helper on
`RequestClient` consumes notifications). Both edits are amendments of galaxy plans, drafted for
their owner: the README lists them under "Awaiting the owner", and R03.T5.a, which adds the two
kinds, does not start until they are accepted. Draft for the brainstorm's revision pass the
closure of open question 21 by Design note 12. Add the test of Design note 12 to
`crates/hyperion-server/tests/websocket.rs`: a client that sends `hello`, a range query and a ping
receives only text frames that parse as today's `ServerMessage`. Extend the doc comment of
`PROTOCOL_VERSION` with the ruling and its condition.

Files: the two plan documents, `crates/hyperion-server/tests/websocket.rs`,
`crates/hyperion-protocol/src/lib.rs`. Acceptance: `npx prettier --check` on both plans;
`grep -n scene_ship docs/agent/plans/galaxy-generation/04-server-and-protocol.md`;
`cargo test -p hyperion-server --test websocket`; `cargo test -p hyperion-protocol
protocol_version_is_two`; the owner signs off the two galaxy-plan amendments and the question 21
draft.

### R03.T2 In-system retarded evaluation

Build `SystemObserver`, `BuildSystemObserverError`, `SystemTrajectory`, `InSystemRetardation`,
`TraceInSystemError` (with `NotConverged`), `retarded_in_system` and the two constants (Design note
7), in the manner of plan 12's `retarded` (`light_time` for metres to a `Span` rounded to the
nanosecond; `before` for the subtraction, made `pub(super)` in `observe/retarded.rs`): the light
time by the fixed point, stopping when a correction changes it by at most 1 ns, or from the second
correction on when a change is no smaller than the one before, and refusing after 10; the apparent
point by the Lorentz form of Design note 7, in its stable arrangement with no division by |v_o|. The
doc comments cite Design
note 7's sources: NAIF's `spkezr_c`, `spkaps_c` and `stelab_c` and the Aberration Corrections
Required Reading, the NOVAS C3.1 guide, and the Explanatory Supplement, whose section numbers are
re-checked against the book first, as the Figures rule requires.

Files: `crates/hyperion-sim/src/observe/{mod.rs,in_system.rs,retarded.rs}`.

Tests:

- A source at rest is seen at its own position, with τ = d ÷ c to the nanosecond; a source in
  uniform motion against the closed-form root of the light-cone quadratic in the system frame, to
  1 ns.
- Counts, on uniform radial tracks (an orbit bends within τ and gives no exact count): a source
  receding radially at 100 km/s from a stationary observer 100 au away (τ₀ = 49,900.478 s,
  β = 3.33564 × 10⁻⁴; δ₁ = 16.645 s, δ₂ = 5.552 ms, δ₃ = 1,852 ns, δ₄ = 0.62 ns) stops after
  exactly 4 corrections, with δ_k ÷ δ_{k−1} = β to 10⁻⁶ relative for k ≤ 3; the same at 5 km/s
  stops after 3 (δ₃ = 0.23 ns); one 2.7 × 10⁵ au away approaching at 800 km/s stops within the cap,
  at 7 or 8 (Design note 7's table); a source rigged so that the rounded map has no fixed point,
  alternating between two adjacent values, stops by the non-shrinking rule rather than running to
  the cap; a source faster than the cap allows (β_r near 1) returns `NotConverged`.
- An observer at β = 0.01 sees a fixed source at an angle θ from its velocity at the relativistic θ′,
  cos θ′ = (cos θ + β) ÷ (1 + β cos θ), to 10⁻¹²; at 30 km/s the result equals the first-order
  r + v_o τ to within β² ÷ 4 in direction.
- An observer on a circular orbit (a ≈ 8.7 m/s²) seeing a body about 2 au away differs from the
  brainstorm's x_B(t − τ) − x_o(t − τ) by an angle of a τ ÷ 2c to 10%.
- The brainstorm's figure, reworded (Design note 7): a body at rest and an observer 400 km away moving
  at 7.7 km/s across the line of sight: the light-time point is not displaced and the apparent point
  is displaced 10.27 ± 0.01 m along the observer's velocity; adding the same 30 km/s to both leaves
  the apparent displacement unchanged to 1 mm while the light-time point moves about 40 m.
- An observer and source moving together at u on uniform tracks: the apparent vector is the
  rest-frame separation r₀ + (γ − 1)(r₀ · û) û of the present one r₀ (Design note 7), to 1 mm +
  10⁻¹⁴ (|x_B| + |x_o| + |u| τ), and its angle to r₀ is at most β² ÷ 4, at 30 km/s and at
  3,000 km/s; for uniform tracks with a relative velocity Δv the angle to the present vector is at
  most |Δv⊥| ÷ c + β_o² ÷ 4 plus |Δv| × 0.5 ns ÷ (c τ); a source absent at the emitted time is
  `NotPresentThen`; determinism across two runs.

Acceptance: `cargo test -p hyperion-sim observe::in_system`.

### R03.T3 Tracks of a system's bodies and stars, and the golden vectors

Build `BodyTrack` and `StarTrack` (Design note 8): positions from `PlanetarySystem::position_at` and
`star_positions_at`, velocities from plan 14's `state_at` and plan 11's `star_states_at`, each added
in its owning file by agreement if absent (the position path untouched), with a test there that the
velocity is the time derivative of the position to 10⁻⁶ relative by central differences over a
second. Add the golden `crates/hyperion-sim/tests/golden/observe/in_system.golden` through the
testkit's `golden!` harness: for three pinned systems of the planetary goldens (one with moons, one
with a wide binary), two observers each (one in low orbit about a planet, one at 30 au), at the
epoch and at +100 years, every present body's and star's emitted time, light time, geometric
position then and apparent position, and every body's Hill radius from plan 14's `hill_radius` (for
R02's frame selection). The systems are chosen so that the bound of Design note 7 is exercised where
an `f64`'s step is largest and the iteration works hardest: at least one body beyond 60 au, and a
close fast stellar pair, whose stars are in the golden too. R03.T13 reads it.

Files: `crates/hyperion-sim/src/observe/in_system/tracks.rs`,
`crates/hyperion-sim/tests/observe_in_system_golden.rs`, the golden, and, if absent,
`crates/hyperion-sim/src/planetary/system.rs` and
`crates/hyperion-sim/src/stellar/multiplicity/positions.rs`.

Tests: `BodyTrack::position_at` equals `position_at` bit for bit at 1,000 random times in the clock
window; a moon's track is its planet's plus its offset; every existing planetary golden is unchanged.
Acceptance: `cargo test -p hyperion-sim --lib observe::in_system`;
`cargo test -p hyperion-sim --lib planetary::system`;
`cargo test -p hyperion-sim --test observe_in_system_golden`; `just ci`.

### R03.T4 Wire types of the scene

Add `crates/hyperion-protocol/src/scene.rs` with every type of the `scene.rs` block under Provides,
each with a wire-form test as `rust-dev.md` requires: `FramePositionDto` tagged by `frame` in snake
case; `time_rate` as a `u32`; `SceneClockStateDto` in snake case; `BodyGrantDto`, `SceneBodyDto`
and `SeenPositionDto` with `seen` omitted when `None`; `SceneCraftDto` documented as a draft with
`planned_path` omitted when `None` (Design note 4). `BulkManifestDto` is R03.T10.a's. Export them
from `lib.rs`, run `just gen-protocol`, re-export from `packages/protocol/src/index.ts`. No kind is
added yet.

Files: `crates/hyperion-protocol/src/{scene.rs,lib.rs}`, `packages/protocol/src/generated/*`,
`packages/protocol/src/index.ts`. Acceptance: `cargo test -p hyperion-protocol scene`;
`just gen-protocol-check`; `just ci`.

### R03.T5 The subscription envelope

- **R03.T5.a Protocol.** Waits on the owner's acceptance of R03.T1's plan 04 rows. In
  `crates/hyperion-protocol/src/envelope.rs` (Design note 1), if P12.T9 has not landed:
  `SubscribeRequest`, `UnsubscribeRequest`, `Subscribed`, `SubscriptionTopic::Scene`,
  `SubscriptionState::Scene`,
  `ServerMessage::Notification`, `NotificationBody::Scene`, the kinds `subscribe` and `unsubscribe`,
  and this plan's `scene_ship` (`SceneShipRequest`) and `scene_cameras` (`SceneCamerasRequest`) in
  `RequestBody`, `ResponseBody` and `REQUEST_KINDS`, as plan 04's "Extending the convention" says;
  if it has, the `Scene` variants and the two kinds only; `ResponseBody::SceneShip(SceneShipSet)`
  and an empty `ResponseBody::SceneCameras`. The server's exhaustive matches
  (`requests::{kind, is_large}`, `Handlers`, the `every_body` walk) gain them, answering
  `unsupported` until R03.T6 and R03.T8, and `is_large` becomes `pub(crate)`. Run
  `just gen-protocol`. Tests: wire forms of every new type and of `notification`; `REQUEST_KINDS`
  holds the new strings; `is_large` marks the scene's `Subscribed` large (Design note 14). Files:
  `crates/hyperion-protocol/src/{envelope.rs,lib.rs}`, `crates/hyperion-server/src/requests/mod.rs`,
  the generated bindings. Acceptance: `cargo test -p hyperion-protocol`;
  `cargo test -p hyperion-server requests::`; `just ci`.
- **R03.T5.b Subscriptions in the connection.** `crates/hyperion-server/src/subscriptions.rs`: a
  table owned by the connection task, at most `MAX_SUBSCRIPTIONS` (4) a connection, numbered from
  1 per connection and never reused on that connection; `subscribe` registers one and answers
  `Subscribed`, `unsubscribe` ends it, a request naming an unknown subscription is `bad_request`
  with `field: "subscription"`, and every subscription ends with its socket. Each subscription has
  a `PendingPush` whose `Merge` implementation follows Design note 5, and the connection's loop
  queues it when `Outbound::has_room_for` allows and otherwise leaves it pending, numbering it with
  the subscription's next `sequence` only then (Design note 4). Add `MAX_SUBSCRIPTIONS` with its
  figure and reason to `limits.rs` and its test: 4, room for the scene, plan 12's alerts and two
  more topics on one connection, where one scene subscription already carries every view's camera.
  `TestClient::next_notification()` in `crates/hyperion-server/tests/common/mod.rs`. Tests (unit, in
  `subscriptions.rs` and `outbound.rs`, with an injected topic that pushes on demand and a stuck
  writer from `testing.rs`): ten pushes into a stuck writer leave one pending push holding all ten
  changes merged; bodies merged by ID with the latest winning; an arrival clears earlier bodies;
  the fifth subscription is refused; unknown subscription; nothing pending survives the socket;
  sequences rise by one per notification sent, however many changes each merged.
  Acceptance: `cargo test -p hyperion-server --lib subscriptions::`;
  `cargo test -p hyperion-server --lib outbound::`; `just ci`.
- **R03.T5.c The client's helper.** `packages/protocol/src/subscriptions.ts`:
  `RequestClient::subscribe(topic)` returning a `Subscription` with its initial state,
  `onNotification(listener)` and `unsubscribe()`; `handleServerMessage` routes `notification` to
  its subscription and returns `true`, drops one for an unknown subscription, and ends every
  subscription on `linkLost()`; `NotificationOf<T>`; re-exported from `index.ts`. Tests (Vitest,
  driving `RequestClient` directly, since the app's `FakeWebSocket` is not importable from the
  package): routing by subscription; a
  notification after `unsubscribe` is dropped; link loss ends subscriptions; a notification for an
  unknown subscription is consumed and ignored. Acceptance:
  `pnpm --filter @hyperion/protocol test`; `just ci`.

### R03.T6 The scene clock and the ship stand-in

`crates/hyperion-server/src/scene/{mod.rs,clock.rs}`: `SceneClock` (anchor time, anchor instant of
`tokio::time::Instant`, rate, state; `now()`, `at_instant()`, and `instant_of(time)` for a deadline,
`None` when paused), `ShipStandIn` (a pose in a frame moving in a straight line), both per open
universe in a `SceneService` held by `AppState`, behind a `watch` channel so that subscriptions see
changes (Design note 2). The `scene_ship` handler validates the rate (0 or a power of ten to
100,000), the time (±H) and the pose (finite; a system or body frame resolved through the sim's
`resolve`, as plan 04 requires of IDs read from a client), and answers `SceneShipSet` with the
clock it set. The `Clock` and `Ship` traits of Design note 2, which `SceneClock` and `ShipStandIn`
implement. The clock reaches the window's edge in state `window_limit` and holds there.

Tests (paused tokio clock): the clock advances at its rate and not while paused; `instant_of` inverts
`at_instant` to the nanosecond at 1× and 100,000×; the window's edge; a bad rate, time or frame is
`bad_request` naming its field; a second `scene_ship` supersedes the first, seen by two receivers of
the universe's `watch` channel (the push to subscriptions is R03.T8.a's test). Acceptance:
`cargo test -p hyperion-server scene::clock`; `just ci`.

### R03.T7 The scene's core

`crates/hyperion-server/src/scene/core.rs`, pure and synchronous: `SceneCore::build(inputs) ->
SceneStateDto` and `SceneCore::advance(&mut self, inputs) -> SceneDelta`, where the inputs are the
clock reading, the stand-in's pose, the cameras, a `SceneKnowledge` and a `CraftSource`, and the
system comes from plan 14's body cache through a caller-supplied closure (the core does no I/O).

- **R03.T7.a The system and its grants.** Add `galaxy::frame::candidate_at` in plan 03's
  `crates/hyperion-sim/src/galaxy/frame.rs` by agreement, the candidate `frame_at` already forms
  for one system, with the rule untouched and a test that `frame_at`'s answers are unchanged over
  its existing cases; record it in plan 03's as-built notes. The core selects the system by
  `frame_at` with the previous system as current and the Schmitt band of Design note 3
  (`SCENE_FRAME_ENTRY` = 0.9), builds `SystemBodiesDto` by plan 14's converter at the level asked,
  degrades each record to its own grant (`BodyRecord::degrade`) and writes the `grants` (Design
  note 13). A body granted `contact` gets its `seen` position from `retarded_in_system` over its
  `BodyTrack`, observed from the stand-in at the scene time. `GrantAsked`, `NoCraft` and the
  `CraftState` it maps to `SceneCraftDto`. Tests: arrival and leaving across a sphere of
  influence, a stand-in moved back and forth across the boundary between ratios 0.95 and 1.05
  arriving once, at 0.9, and leaving once, above 1; the galactic frame has no system; the scene
  holds the system's stars in `hosts`; the arrival's `tidal_radius_m` equals `candidate_at`'s
  `tidal_radius()` at the arrival time; under a fake granting `contact` to half the bodies, each
  record equals `system_bodies` at its own level for that body and every contact carries `seen`
  equal to `retarded_in_system`'s apparent point. Files: `scene/core.rs`, `galaxy/frame.rs`.
  Acceptance: `cargo test -p hyperion-sim galaxy::frame`; `cargo test -p hyperion-server
scene::core`; `just ci`.
- **R03.T7.b Advancing.** Find the next `valid_until` among the scene's bodies (a scene time at
  which `advance` must be called), re-evaluate each body whose `valid_until` has passed, compare
  what `SceneKnowledge` grants with what was sent, and refresh every `seen` body at each heartbeat.
  Camera validation as Design note 6, with `MAX_SCENE_CAMERAS` (8, one per view of the largest
  layout R07 draws on one client, with room) and `scene::is_large_notification` (Design note 14).
  Tests: a body whose `valid_until` passes is re-sent once with its new elements; a knowledge
  change re-sends exactly the bodies it touched, with their new `level`; a heartbeat re-sends every
  `seen` body and only those; advancing in one step or ten gives the same merged delta; a camera in
  another system's frame, and a ninth camera, are refused. Acceptance: `cargo test -p
hyperion-server scene::core`; `just ci`.

### R03.T8 The scene subscription, live

- **R03.T8.a Subscribe, report, push on change.** The `subscribe` handler for `Scene` builds the
  core on the pool at `Priority::Interactive` and answers the whole state; a task per subscription,
  owned by the connection's subscription table and ended with it, waits on the clock's `watch`, the
  next `valid_until` deadline mapped to an instant by `instant_of`, camera reports and the heartbeat,
  and merges each delta into the pending push. `scene_cameras` replaces a subscription's cameras.
  `SCENE_HEARTBEAT` (1 s) joins `limits.rs`, and `requests::system::bodies_of` becomes
  `pub(crate)` for the core's closure. Tests (integration, `crates/hyperion-server/tests/scene.rs`):
  subscribe in a pinned system and receive every body of `system_bodies` for the same time and
  level, in the same form, with every grant at that level; a heartbeat
  each second with `sequence` rising by one (paused clock); a `scene_ship` rate change is pushed at
  once to two subscriptions; a body's `valid_until` passing at 100,000× is pushed; moving the
  stand-in out of the system pushes `no_system`; an unknown subscription and a camera out of reach
  are refused naming their fields. Acceptance: `cargo test -p hyperion-server --test scene`.
- **R03.T8.b Craft at the tick, and a slow reader.** While the scene holds craft, the task pushes
  them every `CRAFT_PUSH_INTERVAL`, 15.625 ms (64 Hz, which is exactly 15,625,000 ns: the
  single-player brainstorm's The clock, re-checked there), with missed ticks skipped;
  `CRAFT_PUSH_INTERVAL` joins `limits.rs` as `Duration::from_nanos(15_625_000)`. A test
  `CraftSource` injected through `AppState` supplies ten craft in the tests. Tests: 64 pushes a
  second of scene time at 1×, each stating its time; with the writer stuck for a second the client
  then receives one notification holding the latest craft and every body change of that second,
  and the server's queued bytes never exceed one pending push a subscription; the push rate of ten
  craft is about 0.25 MB/s (recorded, a finding if over 0.5). Acceptance:
  `cargo test -p hyperion-server --test scene craft`; `cargo test -p hyperion-server --lib
subscriptions::`; `just ci`.

### R03.T9 The Knowledge-bound and two-clients tests

- **Bounded by Knowledge.** A restrictive `SceneKnowledge` injected through `AppState` grants
  `contact` for half of a pinned system's bodies and `full` for the rest, and names three of ten test
  craft as contacts. For 200 camera placements drawn from a fixed seed, in the system frame and in
  body frames, beside hidden craft and inside hidden bodies' Hill spheres, and through a sequence of
  `scene_cameras` and `scene_ship` moves, no state or notification contains a craft that is not a
  contact or a section above its body's granted level, every body's `level` on the wire equals the
  fake's grant, and no `contact` body carries an orbit, a mass or a Hill radius's inputs: asserted
  on the JSON, as plan 12 asserts that no bearing contact has a `host` key. R09 extends this test
  to surveyed cells.
- **Two clients agree.** Two `TestClient`s subscribed to one universe's scene, one of them joining a
  second later, receive notifications whose bodies, clock and ship are equal apart from
  `sequence` after the second's arrival, for a minute of scene time at 10× with a `valid_until`
  crossing and a stand-in move; both states equal `system_bodies` answered for the same time.

Files: `crates/hyperion-server/tests/scene_knowledge.rs`,
`crates/hyperion-server/tests/scene_agree.rs`.
Acceptance: `cargo test -p hyperion-server --test scene_knowledge`;
`cargo test -p hyperion-server --test scene_agree`; `just ci`.

### R03.T10 Outbound binary frames

- **R03.T10.a Header, chunks and the handler's answer.** `crates/hyperion-server/src/bulk.rs`:
  `BinaryFrameHeader`, `encode_header`, `chunk(payload, request) -> impl Iterator<Item = Bytes>` at
  `MAX_BINARY_FRAME_BYTES`, header included (Design note 10). The handler seam's output becomes
  `Answer { body: ResponseBody, bulk: Option<BulkPayload> }` with `From<ResponseBody>`, so every
  existing handler changes by one `.map(Answer::from)`; `BulkManifestDto` in the protocol with its
  wire-form test, in `crates/hyperion-protocol/src/bulk.rs`. Document the header in
  `crates/hyperion-protocol/src/lib.rs`'s module docs. Add the three limits with their figures and
  citations to `limits.rs` (Design note 11), and set `TCP_NOTSENT_LOWAT_BYTES` on every accepted
  socket on Linux through `socket2::SockRef` in `hyperion_server::tap_socket`, which axum's
  `ListenerExt::tap_io` calls at all three serve sites, with socket2 (feature `all`) added to
  `[workspace.dependencies]`; a failure is logged at `warn`. Files: `crates/hyperion-server/src/
{bulk.rs,lib.rs,limits.rs,main.rs,testing.rs,requests/*.rs}`,
  `crates/hyperion-server/tests/common/mod.rs`, `crates/hyperion-protocol/src/{bulk.rs,lib.rs}`,
  `Cargo.toml`, `crates/hyperion-server/Cargo.toml`. Tests: the header's bytes for a pinned
  request, byte for byte, which R03.T11 repeats in TypeScript; payloads of 0, 1, 262,120 (exactly
  one frame's), 262,121, 15 × 10⁶ and 15,728,640 bytes split into ⌈L ÷ 262,120⌉ chunks (none for
  an empty payload, whose manifest says 0; 1, 1, 2, 58 and 61 for the rest), every frame at
  most 262,144 bytes and the payload rejoined equal; on Linux, `SockRef::tcp_notsent_lowat()` of a
  connection accepted by the unit and the integration harnesses reads back 65,536. Acceptance:
  `cargo test -p hyperion-server --lib bulk::`; `cargo test -p hyperion-server --lib limits::`;
  `cargo test -p hyperion-protocol bulk`;
  `just ci`.
- **R03.T10.b Streaming under the budget.** The connection sends a bulk answer's chunks in order,
  each queued only once the bulk bytes queued are under `BULK_QUEUED_BYTES`, then the terminal
  response (Design note 11); the request stays in flight until then; `cancel` stops the chunks and
  answers `cancelled`; a write timeout closes as plan 04's T15 does. Tests (unit over real sockets,
  with an injected handler that answers 15 MB in bulk, in `outbound.rs` or a new `bulk` test
  module): every chunk precedes the terminal response, in order; the server's queued bulk bytes
  never exceed one chunk; a scene push issued mid-transfer arrives before the transfer ends;
  `cancel` after the third chunk gets `cancelled` and no further chunk; a stuck reader is closed by
  the write timeout with nothing held. The push comes from R03.T5.b's injected topic, so these tests
  need no scene. `TestClient::next_binary()` in `crates/hyperion-server/tests/common/mod.rs`,
  returning the header parsed and the payload, with an integration test in
  `crates/hyperion-server/tests/bulk.rs` that an injected bulk handler's chunks arrive through it
  in order before the response. Once R03.T8.a has landed, measure the added latency of a heartbeat
  push during a 15 MB transfer, from the server's send to the client's receipt, over an emulated
  40 Mbit/s link (Design note 11: a reader with a fixed 64 KiB `SO_RCVBUF` reading through a 5 MB/s
  token bucket), with the low-water mark on and off, and record both in Design note 11 (a finding
  over 100 ms with it on). Acceptance: `cargo test -p hyperion-server --lib bulk::`;
  `cargo test -p hyperion-server --lib outbound::`;
  `cargo test -p hyperion-server --test bulk`; `just ci`.

### R03.T11 The client receives bulk

Set `binaryType = "arraybuffer"` in `lib/connection.ts` immediately after the socket is constructed,
before `open`, and turn today's `typeof event.data !== "string"` guard into an
`instanceof ArrayBuffer` branch that routes to `RequestClient::handleBinaryFrame`; chunks are kept
as `ArrayBuffer`s and handed over whole, never parsed on arrival (Design note 11);
`packages/protocol/src/bulk.ts`: `parseBinaryFrameHeader` (refusing a bad magic, format or length,
which the link reports as `error` and does not close on), `BulkAssembler` keyed by request ID, and a
request outcome that resolves with the response and its chunks once the terminal response's manifest
matches what arrived (a mismatch fails the request as `internal`). Tests (Vitest: the protocol
package's own suite drives `RequestClient::handleBinaryFrame` directly; the app's `connection`
suite uses its `FakeWebSocket`, which gains `serverSendsBinary`): the pinned header of R03.T10.a
parses; chunks out of order or missing
fail the request; `cancel` discards partial chunks; link loss discards everything. Acceptance:
`pnpm --filter @hyperion/protocol test`;
`pnpm --filter hyperion exec vitest run src/renderer/src/lib/connection`; `just ci`.

### R03.T12 The client's scene store and clock

`lib/scene/sceneWire.ts`: `toSceneModel(state)` over `toSystemBodiesModel`, and
`applySceneNotification(model, notification)` (arrival replaces, keeping its `tidal_radius_m` in
the model, bodies merge by ID with their
`level` and `seen`, craft and clock replace; the `sequence` must be the previous plus one, and a gap
or a step back is an error that makes `useScene` resubscribe, Design note 4).
`lib/scene/sceneClock.ts`: `renderTime(clock, receivedMs, nowMs)` in `UniverseTime` of whole
seconds and nanoseconds (Design note 9), still when paused or at the limit. `lib/scene/craft.ts`:
`predictedPath(craft, untilS)` (Design note 4). Tests: applying every notification of a
hand-built fixture sequence (an arrival, a body re-sent at a new level, a heartbeat, a leaving)
equals the state built directly for its end; a gap and a step back are errors; `renderTime` at 1×
and 100,000× from a time 999 years out keeps nanoseconds; paused holds; `predictedPath` returns a
planned path unchanged and otherwise the straight line at the pose's velocity.
Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/lib/scene`; `just ci`.

### R03.T13 Apparent positions on the client

`lib/scene/apparent.ts`: `apparentPosition(track, observer, time, previousTau?)` mirroring
`retarded_in_system` (Design note 7): τ rounded to the nanosecond, the emitted time formed as a
`UniverseTime` less a span, the same stop rules (a change of at most 1 ns, or one no smaller than
the one before) with a cap of 10, a warm start from the previous frame's τ allowed, and the same
Lorentz form for the apparent point, with norms as a sum of squares and `Math.sqrt`, never
`Math.hypot`; a body's track from `composePosition` and `stateAt`, a star's from `layoutHierarchy`
over the hosts. `sceneAt(model, observer, time, previousLocal?)` returns each present body's and
star's `geometricM`, `apparentM` and `emitted`, each body's `level` and `seen`, and its
`hillRadiusM`: plan 14's a(1 − e)(m ÷ 3M)^⅓ with m the body's `mass_kg` and M = μ ÷ G − m, from
the orbit's μ = G (M + m) (`OrbitDto.mu_m3_s2`) and the sim's G, 6.674 30 × 10⁻¹¹ m³ kg⁻¹ s⁻²
(`units::consts::GRAVITATIONAL_CONSTANT`, CODATA 2018), `null` below `mass_and_orbit`. A body with
`seen` is placed at its `apparent_m` and gets no geometric position. The ship's local body is named
by R02.T8.a's `selectCameraFrame` over the stand-in's geometric present position with
`previousLocal` as current (Design note 7). The observer is the scene's ship stand-in at the render
time.

Tests: first log the largest discrepancy against R03.T3's golden (read with `?raw` as
`lib/orbit.test.ts` reads plan 14's), then pin every vector within the larger of Design note 7's
bound and ten times that measured value, failing outright if the measurement exceeds 10⁻¹²
(|x_B(t − τ)| + |x_o(t)|); every emitted time within Design note 7's allowance of the golden's;
Hill radii to 10⁻¹² relative of the golden's; two frames 16 ms apart with a warm start agree with a
cold start within the same bound; a camera placed at a far body sees that body and its moons
shifted together, their relative vectors within |Δv_moon| τ of the geometric ones; the local body
named equals `selectCameraFrame`'s answer, and a stand-in held at a ratio of 0.95 of a moon's
sphere keeps whichever body it had; a body not present at its emitted time is left out; a seen
body sits at its `apparent_m`; an observer whose velocity equals a body's at t sees it within
β² ÷ 4 + |a_B| τ ÷ (2c) + 10⁻¹⁴ relative of the present direction. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/lib/scene/apparent`; `just ci`.

### R03.T14 `useScene` and the camera reports

`lib/scene/useScene.ts`: subscribes on mount through R03.T5.c's helper, keeps the model and the
receipt time, exposes `frameAt(nowMs)` for R02 (`sceneAt` at the render time, carrying the previous
local body), reports cameras through `CameraReporter` in `lib/scene/cameraReports.ts` (at most
4 Hz, which keeps a report within a quarter-second of a camera's pose while costing a few hundred
bytes a second, and at once when a camera's frame changes), resubscribes after a reconnection and
after a sequence error (R03.T12), and marks the
scene stale while the link is down, as plan 12's `useAlerts` is to. Tests (Vitest with
`FakeWebSocket` gaining `serverNotifies(subscription, body)` if plan 12 has not added it): subscribe,
receive, render at an extrapolated time; the reporter never exceeds 4 Hz under 60 pose changes a
second and sends a frame change at once; stale on link loss; and the client half of "two clients
agree": two hooks fed the same pushes with receipt times 20 ms apart place every body within
v × 20 ms × rate of each other and craft within one push of each other. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/lib/scene`; `just ci`.

### R03.T15 Verification pass

Record in this plan's as-built notes: the JSON size of a scene state and an arrival for the three
pinned systems, of a `BodySummaryDto` and of a craft push, against the brainstorm's 250–300 and 400
bytes (Design note 4); the push rate with ten test craft; the added latency of a push during a 15 MB
bulk transfer on loopback and, by hand, between two machines on gigabit Ethernet and on Wi-Fi
(Design note 11), with the link rates; a smoke check in the real Electron renderer that one bulk
transfer of 15 MiB (61 chunks) and its terminal response arrives whole and in order, since every
unit test
uses `FakeWebSocket` (run by hand, or in R01's headless harness if it can open a socket, and
recorded); the envelope's state against P12.T9 for plan 12's writer. Acceptance: `just ci`; the
figures are in the plan.

## Verification

- **Honest scene:** the Knowledge-bound test over 200 camera placements, with a level per body
  (T9); the unknown and out-of-reach refusals (T8); contacts placed only by the server's seen
  positions (T7.a, T13).
- **One scene:** the two-clients tests on the server (T9) and the client (T14); every push states
  its time and rate (T8).
- **Apparent positions:** the in-system tests and figures (T2), the tracks against plan 14's
  positions bit for bit (T3), and the client within the bound T13 pins, Design note 7's or ten times
  the measured agreement, and never looser than 10⁻¹² of the barycentric distances (T13).
- **Transport:** chunks in order before the terminal response, one chunk queued at most, a push
  overtaking a transfer, cancel mid-stream (T10.b); the client's reassembly (T11); no unasked
  notification or binary frame (T1).
- **Figures:** the recorded sizes, rates and latencies of T8.b, T10.b and T15.
- **By eye:** R02's wireframe drawing a generated system from `useScene`, with a moon's position
  steady as the stand-in's clock runs at 10⁵×, is R02's check and the first real use of this plan.

## Generator version

No change to generated output and no bump. The in-system functions only read plan 14's and plan
11's positions, and the velocity additions of R03.T3 leave every position bit for bit, which the
planetary goldens prove. The golden added in R03.T3 pins the reading. Nothing is reserved in the
generator: no domain tag, stream or ID prefix. On the wire everything is additive under plan 04's
convention, and `PROTOCOL_VERSION` stays 2 (Design note 12).

## Risks and open points

- **The research of 2026-09-29 came after the first draft.** Its rulings are in Design notes 7, 10
  and 11. Two points in it stay of medium confidence and are settled by measurement: the agreement
  the client reaches with the sim's goldens, whose 10⁻¹⁴ coefficient R03.T13 measures before
  pinning (a second research pass the same day corrected the first's figure, which had put
  10⁻¹² Σ at 30 mm at 100 au where it is some 30 m), and the low-water mark
  and in-flight window on real Wi-Fi (R03.T10.b, T15). The Explanatory Supplement was cited from
  memory and is re-read before R03.T2's doc comments cite it.
- **Head-of-line blocking below the server.** The chunk size, the one-chunk queue and
  `TCP_NOTSENT_LOWAT` bound what the server and the kernel hold, not the router's or access point's
  queue, which a bufferbloated access point can hold for hundreds of milliseconds, nor macOS and
  Windows, where the option is not set. If a wired LAN still holds pushes past the main screen's
  100 ms loop, the fallbacks are, in order: pacing bulk to a fraction of the measured goodput, then a
  second WebSocket for bulk, which escapes the send buffer but still shares the bottleneck queue.
- **Gravitational deflection is omitted**, as SPICE omits it: 1.75″ at the Sun's limb, some 8.5
  telescopic pixels, milliarcseconds in the anti-solar hemisphere, and about 3.6 × 10⁻⁴ rad at 10⁴ km
  from a white dwarf. No task; a telescopic view near a compact remnant is where it would first show.
- **Lighting takes a star at the lit body's emitted time**, not retarded again by the star-to-body
  light time: an error in the terminator's direction of about v★ ÷ c, 10⁻⁴ typically and at most
  3 × 10⁻³ rad for the fastest pairs, invisible on a disc. For R07.

- **The ship stand-in** is shared per universe, set by whichever client asks last, and not saved.
  It is a development and testing seam that sessions retire (Design note 2), and its request is
  refused once a universe has a session. Until then any client can move every client's scene, as any
  client can set plan 12's observer.
- **Elements across an event within the light time.** A body whose elements change at a
  `valid_until` is seen with its old elements for up to a light time afterwards, and a body
  destroyed less than a light time ago is still seen. The scene sends each body's current record
  only, so for that interval the client draws the new elements at the emitted time. Events are
  formations, engulfments and supernova unbindings, rare against light times of hours; if a
  telescopic view ever shows the error, the server keeps the previous record alongside until the
  scene's farthest observer has seen the change. The same interval is a Knowledge gap: the client
  holds the new record before the ship could have seen the event, against the single-player
  brainstorm's "the client never holds truth that the ship has not seen" (The view outside). The
  elements on the wire already carry present truth by the rendering brainstorm's design, and the
  display draws only what is retarded, so nothing shown leaks; the sensors plan, which owns what
  the ship has seen, should delay a record's change to its light time when it replaces
  `GrantAsked`.
- **Contacts are placed by the server once a second.** A body granted only `contact` is drawn at
  the server's seen position, re-sent at each heartbeat (Design note 13), so at high rates it
  steps: at 100,000× by a day of scene time between pushes. That is honest for a contact, which
  the view labels as one; a finer cadence for contacts in view is the sensors plan's to set.
- **Plan 12 and plan 14 coordination.** If P12.T9 and this plan run in parallel lanes, one builds the
  envelope and the other rebases onto it; R03.T1 records which. The velocity asks of plans 14 and 11
  are small, but land in their files; their writers should see them in R03.T1's note.
- **The craft DTO is a draft** owned by the sessions plan (Design note 4). No production code sends
  it, so reshaping it moves no version.
- **Cameras bound little.** Within a system the whole system is sent, so a camera report does not
  yet change what a client receives beyond refusals, and R09 sends every surveyed cell (at most
  about 12 MB) without bounding them by camera (R09 Design note 19); it is built now so that the
  sensors plan's contacts drawn as hulls have their input, and so that the Knowledge-bound test
  covers it.
- **Client clock and latency.** A client renders at the push's time plus its own elapsed time, so it
  lags the server by one delivery, which at 100,000× on a 10 ms link is 1,000 s of scene time. That
  is the brainstorm's bound ("differ only by the delivery of the latest push") and is not
  compensated; half the ping's round trip could be added later if a bridge needs it.
- **Body frames on the wire.** `FramePositionDto::Body` names a non-rotating body frame, plan 01's
  `Frame::Body`. R02's `BodyFixedPosition`, which rotates, is a position type and does not go on the
  wire here; if R02 needs it there, it adds a variant.
- **Re-validated at `899db5e` (R03.T1, 2026-09-30).** No brainstorm change since the plan was
  written. Found in the tree: P12.T9 has not landed (no `SubscribeRequest`, `Subscribed`,
  `SubscriptionTopic` or `ServerMessage::Notification` anywhere), so R03.T5.a builds the whole
  envelope; `PlanetarySystem::state_at`, `star_states_at` and `galaxy::frame::candidate_at` are
  absent, so R03.T3 and R03.T7.a add them by agreement. The velocities they need exist and are
  discarded today: `KeplerElements::relative_state_at` and `Orbit::relative_state_at` return
  `(SystemVector, SystemVelocity)`, and `position_at` (through the private `Epoch::position`) and
  `star_positions_at` keep only the first. Every other Consumes name matches, with these
  particulars for later tasks: `star_positions_at(h, t, out: &mut Vec<(BodyId, SystemPosition)>)`
  fills an out-parameter; `SystemPosition` has no `Sub` (use `displacement_to`, which gives other
  − self, and `SystemVector::length`); `Span::as_seconds_f64` and `from_seconds_f64` are the names;
  the window is `ClockWindow::contains` (±`CLOCK_WINDOW_H`, 1,000 Julian years); `hill_radius(a,
e, mass, primary_mass)` is the pericentre form R02 wants; `tidal_radius` takes a `&PointLy`;
  `convert::planetary::system_bodies(wanted: BodiesRequest, hosts, ctx, planets, seed)`;
  `requests::is_large` and `requests::system::bodies_of` are private, as the plan says;
  `Handler::handle` returns `HandlerFuture`, a boxed `Result<ResponseBody, RequestError>`, which
  R03.T10.a's `Answer` replaces; `Outbound::has_room_for(bytes)` exists; the three `axum::serve`
  sites are `main.rs`, `src/testing.rs` (`Harness::start_with_limits`) and `tests/common/mod.rs`
  (`TestServer::start_with`); socket2 0.6.5 is in `Cargo.lock` through tokio only; the lock holds
  tungstenite 0.29.0 (axum's) and 0.30.0 (the tests' `tokio-tungstenite`). On the client,
  `fractionOfPeriod` and `reduceToHalfTurn` are module-private in `lib/orbit.ts`, so R03.T13
  exports them (or mirrors through `positionAt`) to share the exact reduction. The skills'
  `plan_task.py` does not yet parse `R` IDs (R04.T7.c), so these tasks are read from the plan.
  No task pending re-validation among T1–T10.a; T13 still waits on R02.T8.a.
- **Deviations in T1, as built.** The plan 04 row and the P12.T9 note are drafted in the galaxy
  plans and, per the RM1 lane rules, treated as provisionally accepted so that R03.T5.a proceeds;
  the owner still signs them off (README, Awaiting the owner). The question 21 test is
  `a_client_that_asks_for_no_push_and_no_bulk_receives_only_known_text_frames` in
  `crates/hyperion-server/tests/websocket.rs`: hello, `create_universe`, a 5 ly range query and two
  pings, each answered by the very next text frame, which `TestClient` parses as `ServerMessage`
  and would panic on as binary.
- **Draft for the brainstorm's revision pass (R03.T1): open question 21 closed.** To replace the
  entry's "**Open.**" text: "**Closed: no bump.** The first `notification` and the first binary
  frames are additions like a new kind: the server sends a notification only on a subscription the
  client opened and binary frames only in answer to a request whose kind asks for bulk, so a
  version 2 client that sends neither receives neither, and a newer client asking an older server
  gets `unsupported`. `PROTOCOL_VERSION` stays 2 while neither is ever sent unasked; a test pins it
  (plan R03, Design note 12)." The owner signs off.
- **Deviations in T2, as built.** `BuildSystemObserverError` gains `NotSlowerThanLight`: the
  Lorentz factor needs |v_o| < c. `InSystemRetardation::residual` is the last correction's change
  |τₖ − τₖ₋₁|: at most 1 ns when converged, above it only where the noise rule stopped the
  iteration. A source absent at the observer's present is `NotPresentThen { emitted }` with the
  observed time, since τ₀ needs the present distance. In the 100 au, 100 km/s test δ₁ ÷ τ₀ and
  δ₂ ÷ δ₁ are held to β within 10⁻⁶ relative, but δ₃ only to β δ₂ ± 1 ns: δ₃ = 1,852 ns is formed
  from light times each rounded to the nanosecond, so 10⁻⁶ cannot hold at k = 3. The
  no-fixed-point test alternates 5 ns apart (1 ns would meet the inclusive tolerance). The
  accelerated-observer test also holds the angle to 1% of the exact circular-orbit offset, since
  at ω τ ≈ 1.13 rad a τ ÷ 2c is only the small-angle limit (0.965 of it here). The Explanatory
  Supplement is cited as §7.2.3 "Aberration", pp. 263–269, from its printed contents (a research
  agent's check; the book's text not seen), for the aberration only; where it treats the light
  time is not confirmed. For R03.T13, from the determinism audit: `light_time` floors through
  `Span::from_seconds_f64`, whose `floor_nanos` uses `math::mul_add`, so the client must port that
  floor exactly (an exact two-product or `BigInt`, never a naive `Math.floor(x * 1e9)`), or a 1 ns
  difference can change `corrections` and the emitted time; `Span` and `UniverseTime` stay as whole
  seconds and nanoseconds; and `aberrated`'s grouping, documented on it, is copied operation for
  operation.
- **Deviations in T3, as built.** `BodyTrack::new(system, ctx, index)` returns
  `Result<_, ResolveBodyError>` and `StarTrack::new(hierarchy, star)` an `Option`, so a track is
  resolved once and its `SystemTrajectory` methods need no error path; a belt or the halo resolves
  but has no position. `StarTrack` walks the whole hierarchy per call (a few stars; fine for now).
  Both velocity tests (plan 14's and plan 11's) hold the central difference over ±1 s to 10⁻⁶ of
  the speed plus 4 ε X, X the widest pair apocentre, the body's own apocentre or its distance: a
  Kepler state is rounded at a few ε of its orbit, more near pericentre of an eccentric orbit,
  which over a 2 s difference exceeds 10⁻⁶ v for wide eccentric pairs (measured: 0.008 m/s against
  1,179 m/s on a 430 au pair). A third test pins `Epoch::centre_velocity` to `star_states_at` bit
  for bit. The golden also records each source's velocity now, the observer's velocity,
  `corrections` and `residual`; a primary's mass for the Hill radius is μ ÷ G − m from the orbit,
  as a client has it, and Hill radii are written for bodies with a mass and a bound orbit. The
  golden's farthest body is 92.9 au out (asserted beyond 60 au, bodies only); its most corrections
  is 4, since its observers see planets, and Design note 7's five and seven are held by R03.T2's
  unit tests instead. Plans 14 and 11 carry as-built notes of the two additions. The star tests
  run under `cargo test -p hyperion-sim --lib stellar::multiplicity::positions`, beside the task's
  acceptance commands.
- **Deviations in T4, as built.** `SceneArrivalDto` is tagged by `type` in snake case (`system`,
  `no_system`). `SceneNotificationDto`'s `ship`, `arrival` and `craft` are omitted when `None`, so
  a heartbeat is `{sequence, clock, bodies: []}`; `SceneStateDto.system` is an explicit `null` in
  the galactic frame, since the state is always whole. `sequence` is a JSON number (`number` in
  TypeScript), not the crate's hexadecimal for a `u64`: at 64 Hz it passes 2⁵³ only after some
  4 × 10⁶ years, and the client checks it arithmetically. `Copy` is derived only on types holding
  no `String` or `Vec`. Plan 14's protocol test fixtures are re-exported under `#[cfg(test)]` from
  `planetary.rs` (`record_fixtures`, `requests_fixtures`) for the scene's wire-form tests.
- **Deviations in T5.a, as built.** The owner's acceptance of R03.T1's plan 04 rows is treated as
  provisionally given (the RM1 lane rule). `ResponseBody::Subscribe` holds a `Box<Subscribed>`, as
  `BodyDetail` does, since a scene's state holds a whole `SystemBodiesDto`; the wire form is
  unchanged. `SubscriptionTopic`, `SubscriptionState` and `NotificationBody` are internally tagged
  by `topic` in snake case (`"scene"`), so a request reads `"topic": {"topic": "scene", …}` and
  P12.T9's variants take `alerts` (noted in P12.T9); `type` would match the other tagged DTOs and
  is cheap to switch to before R03.T5.c's client reads it. The four kinds answer `unsupported`
  through `not_served_yet` until R03.T5.b (`subscribe`, `unsubscribe`), R03.T6 (`scene_ship`) and
  R03.T8 (the scene topic, `scene_cameras`). `RequestClient::handleServerMessage`
  (`packages/protocol/src/requests.ts`) and the link's switch
  (`apps/hyperion/src/renderer/src/lib/connection.ts`) gain a `notification` case, which the
  type-aware exhaustiveness lint requires; until R03.T5.c routes it, the client consumes and drops
  a notification.
- **Deviations in T5.b, as built.** A topic is served through a new
  `Handler::subscribe(state, SubscribeRequest, Pusher, token) -> SubscribeFuture`, `unsupported` by
  default. The connection intercepts `subscribe`, reserves the subscription (openings count toward
  `MAX_SUBSCRIPTIONS`) and runs the opening as an ordinary request under the same admission and
  cancellation rules; the subscription goes live, and its pushes are sent, only once its
  `subscribed` answer is queued, so notifications never precede it (tested, the answer held for
  want of room included); a failed or cancelled opening ends it and its number is not reused; the
  state's `sequence` is set to 0 whatever the topic wrote. A topic hands its task to
  `Pusher::attach`, aborted when the subscription ends, under one lock with the end
  (`TaskSlot`). `unsubscribe` is answered at once by the connection, after the same admission
  checks; unsubscribing a subscription still opening is `bad_request` naming `subscription`. The
  fifth `subscribe` is `bad_request` naming `topic`. Across an arrival a pending `ship` or `craft`
  survives unless the arrival brings its own, since no change is dropped (Design note 5's
  "everything before it" read as the system and its bodies). The connection's `select!` takes
  pushes after reading frames, so a topic pushing fast cannot keep `ping`, `cancel` or the close
  from being read. All the tests are in `subscriptions.rs`, end to end through the unit `Harness`
  with `Scripted::with_openings` as the injected topic, none in `outbound.rs`; the stuck-writer
  tests use an outbound budget of 1 MiB, below the 8 MiB clogging frame, so that nothing fits once
  it is queued. `Pusher`, `ScenePush::heartbeat` and `Shared::merge` carry
  `cfg_attr(not(test), expect(dead_code))` until R03.T8 pushes. For R03.T8: requests that name a
  subscription (`scene_cameras`, and P12.T9's `alerts_observer` and `alerts_acknowledge`) cannot
  reach it through `Handler::handle`; the connection must route them, as it does `unsubscribe`
  (an inbox per subscription is the likely shape). A held answer near the budget can be crowded by
  pushes that each fit; if R03.T8.b sees it, pushes should wait while a held frame has no room.
- **Deviations in T5.c, as built.** `RequestClient.subscribe(universe, topic)` takes the universe
  as well, since `SubscribeRequest` carries both, and returns a `PendingSubscription<T>` (an
  `outcome` of `SubscribeOutcome<T>` that never rejects, and `cancel()`), in `request`'s shape.
  `Subscription` also has `id`, `ended` and `onEnd(listener)` (`"unsubscribed"` or `"link_lost"`,
  heard at once by a listener added after the end); `onNotification` and `onEnd` return removers.
  The subscription is registered within the answer's own `handleServerMessage` call, through an
  internal `onAnswer` hook on `startRequest`, before the outcome settles. A `subscribe` answered
  as it was cancelled, or opened for another topic (`protocol_violation`), is ended with an
  `unsubscribe` at once, so that the server holds no subscription nobody reads (`#cancelled` now
  keeps a late-answer hook per ID). The fire-and-forget `unsubscribe` discards its outcome with
  `void` and no `.catch`: an outcome never rejects, and the package has no console to report on.
  `SubscriptionTable` is internal; the types `Subscription`, `SubscriptionEnd`, `NotificationOf`,
  `StateOf` and `TopicName` and `PendingSubscription`, `SubscribeOutcome` are re-exported. The
  two single-topic type guards (`isStateOf`, `isNotificationOf`) each disable
  `typescript/no-unnecessary-condition` for one line until plan 12's `alerts` topic makes the
  comparison real.
- **Deviations in T10.a, as built.** `chunk` takes `axum::body::Bytes` (the `bytes` crate's type,
  re-exported), so no `bytes` dependency is added. `BulkPayload::new(bytes)` returns
  `Result<_, BuildBulkPayloadError>`, refusing a payload whose chunk count does not fit the
  header's `u32` (about a petabyte); `chunk` documents the same panic. `BulkPayload::manifest()`
  gives the `BulkManifestDto` and `frames(request)` the chunks. `BulkManifestDto.bytes` is a JSON
  number (`number` in TypeScript), a payload being megabytes. The handlers meet the seam through
  one private `answered(result) = result.map(Answer::from)` applied with `FutureExt::map`, and the
  unit harness's private `Answer` enum is renamed `Reply`. Until R03.T10.b streams a bulk answer,
  `run` answers one `internal` and logs at `error`; `bulk.rs` carries
  `cfg_attr(not(test), expect(dead_code))` until then. `tap_socket` sets the mark on Android as
  well as Linux, where socket2 exposes it. Both harnesses read the mark back inside their `tap_io`
  closure (`accepted_lowat()`); the integration test, `an_accepted_socket_has_the_low_water_mark`,
  is in `tests/websocket.rs`, which only `just ci` runs among the acceptance commands.
- **Deviations in T6, as built.** The stand-in is in `scene/ship.rs` (`ShipStandIn`,
  `ShipPosition`) beside `scene/clock.rs` (`SceneClock`, `TimeRate`, `ClockReading`,
  `ClockState`); `scene/mod.rs` holds the `Clock` and `Ship` traits, `SceneSetting` (the clock and
  the stand-in) and `SceneService` (`watch(universe)`, `set(universe, setting)`, a `watch` sender per
  `UniverseId`). A universe nobody has set reads as the stand-in at rest at the galactic centre at
  the epoch, in the galactic frame, clock paused: the centre is in no system's frame, so a scene
  subscribed before any `scene_ship` holds no system. `instant_of` rounds up (the first instant at
  which the clock reads the time or later) and answers the anchor for a time already passed; the
  test inverts `instant_of(at_instant(i)) = i` at 1× and 100,000×. The checks are
  `convert/scene.rs` (`ShipRequest`) and the handler `requests/scene.rs`; fields are named
  `time_rate`, `ship.time`, `ship.velocity_m_s` (also refused at or above c, which
  `SystemObserver` needs) and `ship.position`. A frame whose system `resolve` refuses is
  `unknown_system` and a body its system lacks, or that is absent at the pose's time,
  `unknown_body`, both naming `ship.position`, as plan 04's codes for unknown IDs, rather than
  `bad_request`; a system offset that leaves the galactic range is `bad_request`.
  `requests::system::bodies_of` became `pub(super)` here for the body frame (R03.T8.a's
  `pub(crate)` follows). The pose's velocity is relative to its own frame's origin, as
  `KinematicsDto` documents, and the stand-in moves in a straight line in that frame. A body index
  outside plan 14's layout is `bad_request` naming `ship.position`; a body-frame offset is checked,
  as a system-frame one is, for a place in the galactic range at the pose's time. `SceneClock`
  stores no state: `ClockState` is worked out from the rate and the time read (`ClockReading`
  has getters, and `SceneClockDto: From<ClockReading>`); the `Clock` trait is
  `reading_at(Instant)` and `instant_of(time)`; `SceneClock::new` refuses a time outside the
  window (`BuildSceneClockError`); a stand-in is built from a checked `ShipRequest`
  (`From<ShipRequest>`). The acceptance filter `scene::clock` runs the clock's tests only; the
  handler's, the checks', the stand-in's and the service's run under
  `cargo test -p hyperion-server --lib scene` and `just ci`.
- **Deviations in T7, as built.** T7.a and T7.b are one commit. `candidate_at` is in plan 03's
  `frame.rs`, recorded in plan 03's Risks; its test, `candidate_at_gives_the_candidate_frame_at_weighs`
  (bit for bit the candidate the search forms from its hit), and the existing `frame_at` cases are
  in `tests/frame.rs`, so they run under `cargo test -p hyperion-sim --test frame` and `just ci`;
  the filter `galaxy::frame` runs the unit tests and the doctest only. The core's signatures:
  `SceneCore::build(asked, inputs, craft: Vec<CraftState>, world) -> Result<(SceneCore,
SceneStateDto), FetchSystemError>`, `advance(inputs, world, beat: Beat) -> Result<SceneDelta,
FetchSystemError>`, `set_cameras(cameras, t, world) -> Result<(), RequestError>` (the cameras
  are not an input; checked against the tidal radius at the scene time and replaced whole, the
  latest per view; a camera's own time is not checked), `cameras()`, `next_due()` (the next
  `valid_until`), `craft(knowledge, craft) -> Option<Vec<SceneCraftDto>>` (the contacts while
  there are any, an empty list once when the last leaves) and `has_craft()`. The core never reads
  a `CraftSource`: the caller calls `craft_at` and passes the list. `SceneInputs` holds the clock
  reading, the `Ship` and the `SceneKnowledge`; `Beat::{Heartbeat, Change}`. The galaxy comes
  through a `SceneWorld` struct (the universe, the galaxy, its key, the cell and stars caches, and
  a map of the planetary systems the caller fetched) rather than a closure: a system the map lacks
  is `FetchSystemError(id)`, with the core unchanged, and R03.T8.a fetches it from the body cache
  and calls again. `SceneKnowledge` (whose `grant` takes a `BodyId` and the level asked),
  `GrantAsked`, `CraftSource`, `NoCraft` and `CraftState` (a wrapper of the draft
  `SceneCraftDto`) are in `scene/sensing.rs`, `pub(crate)` until R03.T8.b's injection. The
  converter gains `scene_system`, `scene_body` and `ListedBody` (`convert/planetary.rs`, sharing
  `system_bodies`' assembly) and `BodiesRequest::new`, which asserts the clock window. A body
  re-sent because its `valid_until` passed is evaluated at that time, each change in turn, so
  that one advance or ten send the same record; a contact is evaluated at the scene time. A
  contact with no single position (a population) or absent then carries no `seen`; a heartbeat
  re-sends the contacts the ship sees, and one it saw at the last push and no longer does. A grant
  that stops resolving a belt's member sends nothing, the wire having no withdrawal; the client
  then keeps a record above the ship's new grant until the scene is rebuilt (for the sensors plan,
  which first lowers a grant). The ship is placed in the scene's system directly when it is in
  that system's or one of its bodies' frames, otherwise through the galactic frame with the
  systems' drifts (`epoch_velocity`); a body frame whose body is absent at a time falls back to
  the barycentre. A handover between overlapping spheres cannot leave the scene in no system while
  the ship is still inside its current one: `frame_at` takes the ship from a sphere that holds it
  only for a rival whose ratio is at most 0.9 of the current one's, so at most 0.9. No body of the
  pinned systems changes inside the clock window (plan 14's goldens hold no `valid_until`), so the
  `valid_until` and one-or-ten tests plant one on the core's record of what it sent. The
  frame-crossing test finds a direction from the system into interstellar space and steps the
  stand-in through ratios 1.0, 0.95, 0.9(1 − 10⁻⁹), 0.95, 0.999, 1.05, 0.95, 1.05.
  `MAX_SCENE_CAMERAS` is in `limits.rs`.
- **Deviations in T11, as built.** No kind is answered in bulk yet, and R06's and R09's carry the
  manifest differently (R09's only in one variant of an enum), so the request is
  `RequestClient.requestBulk(body, manifestOf)`, the caller reading the manifest from the response
  (`null` for a response with no bulk, which must then have had no chunks); its outcome,
  `BulkOutcome<K>`, is `{ ok, response, chunks }`. Each chunk is a `Uint8Array` view of its
  frame's payload over the frame's own `ArrayBuffer`, neither copied nor parsed.
  `requestBulk` and `handleBinaryFrame` are `RequestClient`'s methods, in `requests.ts`; `bulk.ts`
  holds the parser, its constants and `BulkAssembler`, all exported, so R06.T12 can drive the
  assembler directly or through `requestBulk` and `FakeWebSocket.serverSendsBinary`.
  `parseBinaryFrameHeader` also refuses a frame over 262,144 bytes, a reserved byte other than 0, a
  payload length that disagrees with the frame's, and an index not below the count; a refused frame is reported by the
  link through `console.error` and closes nothing (`handleBinaryFrame` returns
  `BinaryFrameReceipt`). A chunk out of order, or stating another count than the request's earlier
  ones, fails its request as `internal` at once and sends `cancel`, so that the server stops
  streaming; the manifest is checked at the terminal response for the count of chunks, the count
  the chunks stated, and the bytes. A chunk for a request that asked for no bulk, has ended or was
  cancelled is dropped silently. A `manifestOf` that throws fails the request as `internal` rather
  than leave it unsettled. The internal `startRequest` now takes an options
  object (`onAnswer`, `onLateAnswer`, `bulk`). The app's `FakeWebSocket` gains `binaryType` and
  `serverSendsBinary`, which delivers a `Blob` unless the client asked for `arraybuffer`, so the
  connection test proves the setting; `test/binaryFrames.ts` builds frames as the server's
  `encode_header` and `chunk` do.
- **Deviations in T12, as built.** The scene's messages carry no designation, which plan 14's
  `toSystemBodiesModel` needs for its labels, so `toSceneModel(state, designate)` and
  `applySceneNotification(model, notification, designate)` take a `designate(system)` callback,
  which `useScene` (R03.T14) supplies from the chart's answers. Both return results rather than
  throw: `{ kind: "ok", model }` or `{ kind: "fault", fault }`, and the update also
  `{ kind: "sequence", expected, received }` for a gap or a step back, the model unchanged. The
  model's types are in `lib/scene/model.ts` (`SceneModel`, `SceneSystem`, `SceneClock`,
  `SceneKinematics`, `ScenePosition`, `SceneCraft`, `BodyGrant`, `SeenPosition`); the model keeps
  the scene as the wire states it with every notification merged in (`wire`) and is rebuilt from
  it, so that applying a sequence equals building its end. A re-sent body absent from the list is
  inserted in ID order; the grants must name the bodies in order, or the scene is a fault; a body
  re-sent with no system is a fault. The adapter also refuses, as a fault, a clock rate other than 0
  or a power of ten to 100,000, a time that is not whole seconds and nanoseconds in `[0, 10⁹)`, a
  `sequence` that is not a whole number, and a galactic position whose offsets leave `[0, 1 ly)`. A
  fault, like a sequence error, leaves the model as it was; `useScene` (R03.T14) resubscribes on
  either. **Open, pending the owner (no task owns it yet):** `tidal_radius_m` comes only with an
  arrival, and `SceneStateDto` has none, so a client that subscribes while the scene is already in
  a system has `tidalRadiusM: null` until the next arrival, and R02.T17's clamp has nothing to read;
  the README's row for R02's ask reads "met" but is only partly met. The likely fix is an optional
  `tidal_radius_m` on `SceneStateDto` (the README puts it "not on `SceneSystemDto`"), added on the
  server with R03.T8.a. **Open, likewise:** the scene's messages carry no system designation, so
  `useScene` needs a designation source for any system the scene arrives in, which its planned
  signature `useScene(requests, universe, cameras)` lacks; the alternatives are a `designate`
  parameter on `useScene` (asking the server for an unknown system's designation) or the
  designation on `SceneSystemDto`, an additive server change like the tidal radius. `renderTime` holds the time at the
  clock window's edge, ±H, as the server's clock stops there, and never runs back for a frame
  stamped before its push. `predictedPath(craft, untilS)` takes `untilS` as scene seconds after the
  pose's time (a `RangeError` for one negative or not finite) and returns the straight line as its
  two ends, a galactic pose carried across its light-year cells with its offset kept below 1 ly;
  `CraftPose` is `SceneKinematics`, and R11 tells an extrapolated path by `plannedPath === null`; a craft's wire attitude (x, y, z, w) becomes R02's `Quaternion`. Hand-built
  fixtures are in `src/renderer/src/test/sceneFixture.ts`, on plan 14's shared wire fixture.
- **Deviations in T13, as built.** `apparentPosition(track, observer, time, previousTau)` takes a
  `SystemTrack` (`positionAt(time)`, `null` when absent) and `previousTau` as a `Span` or `null`,
  and returns `seen` (emitted, light time, corrections, residual, geometric position then, apparent
  position), `not_present_then` or `not_converged`. The sim's rounding is ported in
  `lib/scene/lightTime.ts`: JavaScript has no fused multiply-add, so `floor_nanos` reads the exact
  rounding error of the product by 10⁹ with Dekker's two-product, and the test pins a distance where
  a naive floor gives a nanosecond more; spans and times stay whole seconds and nanoseconds.
  `fractionOfPeriod` and `reduceToHalfTurn` are not exported: the tracks go through `positionAt`
  and `composePosition`, which already reduce by them. `sceneAt(model, observer, time, previous)`
  takes the previous `SceneFrame` (its local body is the current one, its light times start the
  iteration warm) rather than `previousLocal`, and returns `null` when the scene has no system;
  each body's and star's `geometricM` is its position at the frame's time (what R02 draws for the
  local body), beside `apparentM`, `emitted` and `lightTime`. `shipObserver(model, time)` gives the
  stand-in as the observer, in the system frame or carried with its body, `null` in the galactic
  frame. Hill radii are computed in the wire adapter (`SceneSystem.hillRadiiM`) from the wire's
  kilograms and μ, as plan 14's `hill_radius` does with `Math.cbrt` for `libm`'s. Bodies are placed
  by the `SYSTEM` display's `layoutBodies` (`displays/system/bodyMap.ts`), so `lib/scene` imports
  from `displays/`. The local body's candidates are the planets, dwarf planets and moons with a Hill
  radius, each with its parent body or star (`null` for a pair or the barycentre). **The fixture.**
  R03.T3's golden holds what is seen, not the elements a client propagates, and its galaxy, the
  Milky Way fixture at its seed, is not one a server universe builds (a universe draws its
  parameters from its seed). A server unit test, `crates/hyperion-server/src/convert/scene_fixture.rs`,
  therefore writes `crates/hyperion-server/tests/golden/scene_systems.golden`: the `system_bodies`
  frames of the three systems at both times, at `mass_and_orbit`, through the handler's own
  converters on that galaxy. A belt's members, not yet on the wire, and rings, which have no single
  position, are not compared: 140 vectors are, stars among them (at least 20, asserted). Two unbinding times in the frames
  lie some 6 Gyr before the epoch, beyond 2⁵³ s, which plan 14's adapter refuses; the test holds
  them to a safe integer and the issue is reported as galaxy work. **The measurement**
  (2026-09-30): the largest discrepancy is 2.0 × 10⁻¹³ Σ (1.03 m, a moon of `close_binary` a
  century on); it and the next (6.8 × 10⁻¹⁵ Σ, a moon of `solar_like`) come from moons whose orbits
  evolve tidally with age while their `valid_until` is `None`, so the sim evaluates the elements at
  the emitted time and the wire states them at the record's (about 8 m of semi-major axis a
  century; a finding for plan 14). Every other vector agrees within 5 × 10⁻¹⁵ Σ, which bears out
  Design note 7's 10⁻¹⁴. Emitted times agree within 2 ns, Hill radii within 3 × 10⁻¹⁶. The test
  finds the drifting moons from the fixture itself (elements that differ between its two times with
  no `valid_until`) and pins them apart: every other vector at the larger of Design note 7's bound
  and 10 × 5 × 10⁻¹⁵ Σ, the drifting moons at min(10 × 2.0 × 10⁻¹³, 10⁻¹²) Σ, and fails on any
  measurement above the Verification's ceiling of 10⁻¹² Σ. **Awaiting the owner:** whether plan 14
  should give a tidally evolving orbit a `valid_until` (or the client bound those moons so), and the
  unbinding times beyond 2⁵³ s; both are reported to the orchestrator for plan 14. **Tests, as
  built.** The far body and its moons are held to 1.01 |Δv_moon| τ, plus the giant's and the
  observer's motion across the difference of the two light times, plus 1 m: without the second term
  a moon's measured offset exceeds the plan's bound by 0.3%; the factor and the metre cover a 1 s
  chord's estimate of the moon's speed and the positions' rounding. The warm start is held to Design
  note 7's bound with each source's own speed from the golden for `apparentPosition`, and, for
  `sceneAt` given the frame before, with 10⁵ m/s bounding every source's speed in these systems. The local body is checked twice: the planet the golden's
  low-orbit observer circles, and `selectCameraFrame`'s own answer over candidates built
  independently. The client's placed tracks never report absence: a body is in the scene by its
  record's state at the scene's time, so `not_present_then` is tested on a synthetic track and the
  record's `destroyed` through `sceneAt` (the Risks' "Elements across an event within the light
  time"). **Names, for R02, R07 and R08.** `previous` is required (`null` for none), so that a
  caller cannot drop the frame rule's hysteresis; R08's sketch `sceneAt(model, observer, time)`
  passes `null`. A body's entry is `SceneBodyFrame`, a union of `placed` (with `geometricM`,
  `lightTime` and `hillRadiusM`) and `contact` (the server's `apparentM` and `emitted` only), which
  R07's text calls `SceneFrameBody`. The candidates for the local body are formed from the present
  geometry before the light time is solved, so a source whose light time does not converge is still
  in the frame rule. The wire adapter keeps a scene's `SceneSystem` across a notification that does
  not change the system (a heartbeat, a craft push), so `sceneAt`'s placements, kept per system
  model, are laid out once per change and not once per push. The acceptance command should read
  `pnpm --filter hyperion exec vitest run src/renderer/src/lib/scene`, which also runs
  `lightTime.test.ts`.
- **Deviations in T8.a, as built.** The topic is `scene/topic.rs`: `open` (reached through
  `Handlers::subscribe`) builds the core on the pool at `Priority::Interactive`, re-running the
  job after fetching each system it asks for (`FetchSystemError`) from the body cache, and spawns
  the task, handed to the `Pusher` (`attach`) so that it ends with the subscription; its pool jobs
  carry a token a `CancelOnDrop` cancels when the task ends. Cameras sent with `subscribe` out of
  reach refuse the subscription (`bad_request` naming `cameras`). `scene_cameras` is routed by the
  connection, as R03.T5.b foresaw: each subscription has a command channel of 8
  (`SubscriptionCommand`, `Subscriptions::command`, `Pusher::take_commands`), and
  `Requests::scene_cameras` starts a request that awaits the topic's answer; an unknown
  subscription is `bad_request` naming `subscription`, a full channel `queue_full`. A notification
  carrying an arrival is serialised on the pool (Design note 14): `Subscriptions::next_ready`
  became `next_unsent`, whose `Unsent` is serialised here or by `serialise_on(pool)` in
  `flush_pushes`. The frame is re-selected at each heartbeat, setting change and due
  `valid_until`, so a stand-in drifting across a sphere is noticed within a second of real time.
  A heartbeat is pushed each second whether or not anything changed. Requested by the
  orchestrator for R02.T17 (lane D3): `SceneStateDto` gains an optional `tidal_radius_m`, present
  whenever `system` is, so a client subscribing inside a system has the sphere; bindings
  regenerated. `scene` is a public module for its seams only (`SceneKnowledge`, `GrantAsked`,
  `CraftSource`, `NoCraft`, `CraftState`), injected through `ServerConfigBuilder::scene_knowledge`
  and `craft_source` (as `entropy` is) and held by `SceneService`; everything else stays
  `pub(crate)`. `requests::{bodies_of, openable_universe}` are re-exported `pub(crate)`.
  `TestClient::request_among_notifications` answers a request on a connection with subscriptions
  open. The heartbeat test runs in real time with the scene clock paused (the task's "paused
  clock" read as the scene's). No generated body changes inside the clock window, so "a body's
  `valid_until` passing at 100,000× is pushed" is a unit test in `scene/topic.rs` on a planted
  `valid_until` (`SceneCore::plant_valid_until`, test-only).
- **Deviations in T8.b, as built.** The craft tick runs in the topic's task (`MissedTickBehavior::Skip`)
  while the core holds craft; the craft source is asked at each tick and at each heartbeat (which
  starts the tick once craft appear), on the runtime, so a source must be cheap. The core is held
  as an `Option` and lent to each pool job rather than cloned; the task keeps only the planetary
  systems of the scene's system and the ship's frame. A large notification is serialised with
  `try_submit` and on the runtime when the pool's queue is full, so that the connection never
  waits for room and goes on reading (from the T8.a review). The slow-reader test is a unit test
  (`scene/topic.rs`, through `Harness::start_configured` and a handler that answers requests by
  script and subscriptions by the server's topic): with the writer stuck past a heartbeat, the
  queue's bytes do not grow, and the next notification after the clogging response is numbered
  one past the last and carries the latest ten craft, stated at its own time, and the
  heartbeat's contacts. Recorded on this shared machine (provisional): 64 craft pushes in a second
  of scene time at 1× with ten craft, 0.191 MB/s of JSON (the brainstorm's 0.25 MB/s, under the
  0.5 finding threshold), `craft_are_pushed_at_64_hz_each_push_stating_its_time` in
  `tests/scene.rs`, which allows 32 to 66 for a loaded machine. Not observed, so not built: a held
  answer crowded by pushes that each fit (R03.T5.b's note). A topic whose pool job fails ends its
  task with a `warn` and leaves its subscription silent until unsubscribed; the client's sequence
  check does not see it (only a server shutting down reaches it today).
- **Deviations in T9, as built.** The scene's integration helpers moved to
  `tests/common/scene.rs`. `scene_knowledge.rs` drives 200 camera placements from a fixed seed (in
  the system's frame within 40 au, beside the seven craft that are not contacts, inside a hidden
  body's Hill sphere in its frame, and near any body in its frame, in turn) and, every fiftieth,
  moves the stand-in out of the system and back to a drawn point in it, so that the whole system
  arrives four times more; every state, notification and arrival is checked on its JSON (craft
  only contacts; each grant and re-sent body's `level` the fake's; no section above a body's
  level `ok`; a contact's `mass_kg` and `orbit` withheld and its kind `unresolved`). Camera time
  is fixed at the stand-in's. `scene_agree.rs` cannot compare the two subscriptions' pushes byte
  for byte: each task reads the clock when it pushes and their heartbeats are a second apart. It
  holds instead that both are told the same ships in the same order and one departure each,
  that every clock either is told lies on its setting's line (scene time less the rate times the
  real time since the setting was answered) to within a second of real time at 10×, and that every
  system either is given, the states and the return's arrival, equals `system_bodies` answered
  for that push's own time. No generated body changes inside the clock window, so the minute has
  no `valid_until` crossing (R03.T8.a's unit test covers it). That last check found that a push
  stated the clock read when it was pushed rather than the reading the core evaluated at: the
  task now pushes the reading `delta` advanced to (`push_of`), so a push's clock is the time its
  records hold.
- **T8 and T9 after review.** The tests of T8.a's `valid_until` and T8.b's slow reader are unit
  tests in `scene/topic.rs`, so both tasks' acceptance gains
  `cargo test -p hyperion-server --lib scene::topic`. The craft tick landed in T8.a's commit.
  `CRAFT_PUSH_INTERVAL` is `Duration::from_micros(15_625)`, the plan's `from_nanos(15_625_000)`
  (Clippy prefers the larger unit). A large notification the pool refuses is serialised by
  `spawn_blocking`, never on the runtime. The craft test asserts each push states a later time and
  at most 66 pushes a second; the measured rate is recorded in T8.b's note, not asserted. The
  slow-reader test asserts that the merged notification carries every contact the ship sees and
  craft stated within a second before its clock (a heartbeat merged after a craft push states a
  later clock than the craft, which carry their own time). `keep_only` has its own test. The
  knowledge test counts arrivals by reading until a system arrives. Its "inside a hidden body's
  Hill sphere" placements are offsets within 10⁸ m on each axis of the body, not scaled to the
  sphere (the wire hides the Hill radius of a contact). The two-clients test injects ten craft and
  a knowledge with contacts: each craft list either client is told is the source's at its stated
  time, the same contacts are placed by sight for both, and each later setting continues the
  clock, so the minute is one run of scene time; records are compared by position, each degraded
  to its own grant. Open for the owner: a slow reader's merged push states the latest clock while
  an arrival merged into it holds records evaluated at an earlier time (Design note 5 keeps the
  latest clock; the records' orbits are time-independent, a contact's `seen` is not); and a topic
  whose pool job fails stops pushing with only a `warn`, which the client's sequence check cannot
  see (an error notification would need a protocol addition).

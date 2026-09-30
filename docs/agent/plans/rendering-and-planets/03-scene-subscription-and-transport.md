# Plan R03: The Scene Subscription and Bulk Transport

- **Milestone:** Rendering milestone RM1 (a wireframe view at real scale, and the determinism
  checks), beside R01, R02 and R04.
- **Depends on:** galaxy plans [04](../galaxy-generation/04-server-and-protocol.md) (built: the
  request envelope, the outbound queue and its limits, the CPU pool),
  [12](../galaxy-generation/12-retarded-observation-alerts.md) (P12.T0–T2 and T4 built:
  `hyperion_sim::observe`; its subscription envelope, P12.T9, is not built and is built here if it
  still is not, Design note 1) and [14](../galaxy-generation/14-planetary-systems.md) (in progress:
  bodies, orbits, `system_bodies`, `body_detail` and the body cache are built). No R-plan: R02
  consumes this plan for generated scenes, and R06 and R09 put their payloads on its binary frames.
- **Brainstorm sections covered** (in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)): the retarded-position
  paragraphs and leans of
  [The floating origin is already in the simulation](../../brainstorming/rendering-and-planets.md#the-floating-origin-is-already-in-the-simulation)
  (drawn positions as light-time plus aberration, the ship's local body geometric, `SystemTrajectory` and
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
bodies of the system its ship stand-in is in, as plan 14's records with their orbital elements, at
the detail level granted, with the system's stars and the time and time rate on every push; the
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
  as plan 14's handlers do.
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
pub const IN_SYSTEM_MAX_CORRECTIONS: u8;             // 10; typically 2–3 (Design note 7)

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
pub struct SceneStateDto { sequence: u64, clock: SceneClockDto, ship: KinematicsDto,
    system: Option<SystemBodiesDto>, craft: Vec<SceneCraftDto> }
pub struct SceneNotificationDto { sequence: u64, clock: SceneClockDto,
    ship: Option<KinematicsDto>, arrival: Option<SceneArrivalDto>,
    bodies: Vec<BodySummaryDto>, craft: Option<Vec<SceneCraftDto>> }
// later optional fields: R07's `main_screen`, R09's `surface_revisions` (Design note 4)
pub enum SceneArrivalDto { System(Box<SystemBodiesDto>), NoSystem }
pub struct SceneCraftDto { craft: String, hull: String, state: KinematicsDto,
    attitude: [f64; 4], angular_velocity_rad_s: [f64; 3] }            // a draft; Design note 4
pub struct BulkManifestDto { chunks: u32, bytes: u64 }               // for R06's and R09's kinds
```

Binary frames: the header of Design note 10, documented in the protocol crate's module docs, with
no type (the crate holds wire types, and a binary frame is not JSON).

### Server (`hyperion_server`)

```rust
pub(crate) mod subscriptions { Subscriptions, SubscriptionId, PendingPush, Merge }
pub(crate) mod scene { SceneService, SceneClock, ShipStandIn, SceneCore, SceneDelta,
    SceneKnowledge, GrantAsked, CraftSource, NoCraft, CraftState }
pub(crate) mod bulk { encode_header, BinaryFrameHeader, chunk, Answer, BulkPayload }
// limits.rs
pub const MAX_BINARY_FRAME_BYTES: usize;        // 262_144, header included (Design note 11)
pub const BULK_QUEUED_BYTES: usize;             // 262_144: one chunk in the queue at a time
pub const TCP_NOTSENT_LOWAT_BYTES: u32;         // 65_536, set on Linux (Design note 11)
pub const MAX_SUBSCRIPTIONS: usize;             // 4 a connection
pub const MAX_SCENE_CAMERAS: usize;             // 8 a subscription
pub const SCENE_HEARTBEAT: Duration;            // 1 s
pub const CRAFT_PUSH_INTERVAL: Duration;        // 15.625 ms, the 64 Hz tick
```

`TestClient::next_notification()` and `TestClient::next_binary()` in
`crates/hyperion-server/tests/common/mod.rs`.

### Client

In `@hyperion/protocol` (`packages/protocol/src/`): `subscriptions.ts`
(`RequestClient::subscribe(topic) -> Subscription<T>` with `state`, `onNotification`,
`unsubscribe()`; `NotificationOf<T>`), `bulk.ts` (`parseBinaryFrameHeader`, `BulkAssembler`,
`RequestClient::handleBinaryFrame(data: ArrayBuffer)` and a `requestBulk` outcome carrying the
chunks).

In `apps/hyperion/src/renderer/src/lib/scene/`: `sceneWire.ts` (`toSceneModel`,
`applySceneNotification`, over `lib/system/bodiesWire.ts`'s `toSystemBodiesModel`), `sceneClock.ts`
(`renderTime(clock, receivedMs, nowMs): UniverseTime`), `apparent.ts` (`apparentPosition`,
`sceneAt(model, observer, time) -> SceneFrame` with each body's and star's `geometricM`,
`apparentM`, `emitted` and, for bodies, `hillRadiusM`, and the ship's local body named),
`cameraReports.ts` (`CameraReporter`, at most 4 Hz and on a frame change), `useScene.ts`
(`useScene(requests, universe, cameras) -> SceneView`, stale on link loss).

## Consumes

- **Galaxy plan 04:** the request envelope (`RequestBody`, `ResponseBody`, `REQUEST_KINDS`,
  `RequestError`, `ErrorCode::BadRequest` with `field`), its reservation of `notification`, of
  `response_part`'s rule that parts precede the terminal `response`, and of binary frames for bulk;
  its table of reserved kinds, which a new kind enters first; `requests::{Handler, Handlers,
kind, is_large}` and the connection task in `ws.rs`; `outbound::{Outbound, Held, Writer}` and
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
  `requests::system::bodies_of`) and converter (`convert::planetary::system_bodies`); the client's
  `toSystemBodiesModel` (`lib/system/bodiesWire.ts`) and `composePosition` and `positionAt`
  (`lib/orbit.ts`). **Asked of plan 14** (Design note 8): `PlanetarySystem::state_at(ctx, index,
t) -> Result<Option<(SystemPosition, SystemVelocity)>, ResolveBodyError>`, a body's velocity
  beside the position `position_at` already gives, sharing its arithmetic.
- **Galaxy plan 11:** `stellar::multiplicity::{SystemHierarchy, star_positions_at}`. **Asked of
  plan 11**: `star_states_at(h, t, out)`, the same walk with each star's velocity.
- **Galaxy plans 01 and 03:** `coords::{Frame, SystemPosition, SystemVector, SystemVelocity,
BodyPosition, GalacticPosition}`, `time::{UniverseTime, Span, ClockWindow}`,
  `galaxy::frame::{frame_at, FRAME_HYSTERESIS}`.
- **R02:** nothing at build time. R02 consumes `useScene` and `sceneAt`, and owns the camera whose
  poses `CameraReporter` sends. Asked of R02 (and of R07 and R08, which read the same rule): only
  the ship's local body is drawn geometrically; a free camera's other local body is drawn at its
  apparent position like every other body (Design note 7). R02 still chooses the camera's frame,
  for which `sceneAt` gives each body's Hill radius, as R02 asks.

## Design notes

1. **The envelope is built here if plan 12 has not built it, to plan 12's design.** P12.T9 needs
   P12.T8's alert service, which needs T5 and T7, none of them built, so the scene is likely to
   need `subscribe` first, as the brainstorm foresaw ("the view either waits on plan 12's envelope
   work or builds it"). R03.T5 builds the envelope exactly as plan 12's Provides and P12.T9 describe
   it, with `Scene` as the only topic, and records in plan 12 that P12.T9 then adds only its
   `Alerts` topic, state and notification body, `alerts_observer` and `alerts_acknowledge`. If
   P12.T9 has landed first, R03.T5 shrinks to adding the `Scene` variants. Either way the kind
   strings stay plan 12's in plan 04's table; this plan adds its own two (R03.T1).
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
   universe with a session, and the service's core, which takes a `Clock` and a `Ship` by trait,
   does not change.
3. **What the scene holds.** The scene's system is the ship's frame by plan 03's `frame_at`, with
   the previous scene system as `current`, so that plan 03's `FRAME_HYSTERESIS` stops it flickering
   at a sphere of influence. In a system the scene holds plan 14's `SystemBodiesDto` for it at the
   scene time and the granted level: every body, with its orbit as `BodyOrbitDto` (parent, elements,
   `valid_until`), and the system's stars and hierarchy in `hosts`, which the local star's disc
   needs (R06). A system's bodies are some tens to about a hundred, so the whole system is sent,
   and the cameras do not thin it. In the galactic frame the scene holds no system. The star field
   is not in it: the sky is R06's request. Craft are the ship's contacts in the scene's reach,
   none until sessions and sensors.
4. **The state and its pushes.** A subscription is answered with the whole state; each notification
   carries only what changed, and always the clock (time, rate, state) and a `sequence` that grows
   by one per notification, so that the client can tell a lost frame from a conflated one (Design
   note 5). What changes, and when: the system, on arrival and on leaving (`arrival`); a body, when
   the scene time passes its `valid_until` or what the ship knows of it changes (`bodies`, whole
   `BodySummaryDto`s, latest wins); the ship stand-in and the clock, when `scene_ship` changes them;
   craft, at every tick while any are in the scene (`craft`, the whole list); and a heartbeat of the
   clock alone every `SCENE_HEARTBEAT`, 1 s. The brainstorm's figures, 250–300 bytes a body and 400
   a craft, about 0.25 MB/s for 100 bodies and 10 craft (Runtime and code shape), are re-measured by
   R03.T15: plan 14's `BodySummaryDto` with its `bulk` section is larger than a bare `BodyOrbitDto`,
   perhaps 700–900 bytes, which changes the arrival's size and not the rate, since bodies are pushed
   only on change. The state leaves room for R07's camera field (`main_screen`) and R09's
   `surface_revisions` (a revision per body), each an optional field added by its plan, which moves
   no version. `SceneCraftDto` is a draft, the least a renderer needs of a craft (a hull
   definition's key, a pose and its rates), and belongs to the sessions plan, which may reshape it
   freely while nothing sends it.
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
   which from R09 on is the coarse field's cells and later the contacts drawn as hulls. Poses are
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

   The iteration starts from the present distance and corrects until the light time changes by at
   most 1 ns (inclusive: τ is rounded to the nanosecond by plan 12's `light_time`, and a strict test
   could cycle between two adjacent nanoseconds), with a cap of 10 corrections past which it returns
   `NotConverged`. The contraction factor is the source's radial speed over c, and the observer's
   velocity does not enter. Typically two or three corrections are needed, but not always: at 100 au
   and 250 km/s (a hot Jupiter) four; at 10⁴ au and 300–800 km/s (a wide companion, a close compact
   pair) five or six; at a system's reach, some 2.7 × 10⁵ au, at 800 km/s six. The brainstorm's "at
   most three, as SPICE's CN does" fails in the game's own systems (SPICE's three iterations are for
   Solar System speeds, `spkezr_c`). A fixed point is kept rather than Newton's method, which fails
   when τ spans many orbits of a close pair.

   Only the ship's local body, the body in whose Hill sphere the ship is or whose terrain is streamed
   for the ship, where τ is milliseconds, is drawn geometrically at the present, in its own frame, so
   that terrain and collision agree; its time-varying states are still drawn at their emitted times.
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
   `UniverseTime` as t less a `Span`, the same stop rule and cap. The sim's `retarded_in_system` is
   the reference, and its golden vectors pin the client to |A_client − A_sim| ≤ 1 mm + 10⁻¹²
   (|x_B(t − τ)| + |x_o(t)|) in system-frame magnitudes, with the emitted times equal as integer
   nanoseconds: an `f64`'s step is about 1 mm at 30–60 au and 2 mm at 60–120 au, so a bound on the
   apparent vector alone could not be met far out, and 30 mm at 100 au seen from 10⁵ km is a
   three-thousandth of a telescopic pixel. R03.T13 first measures the agreement and pins at the larger
   of this bound and ten times what it measures.

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
   `NotPresentThen`; one destroyed less than a light time ago is still seen, since its light is
   still arriving, and the scene keeps its last elements until then (Risks).
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
    "at most 1 MB" (a 15 MB field is 60 of them). They are queued one at a time, the next only once
    the bulk bytes queued fall under `BULK_QUEUED_BYTES`, one chunk, while text frames queue as
    before. And on Linux every accepted socket gets `TCP_NOTSENT_LOWAT` of 65,536 bytes
    (`TCP_NOTSENT_LOWAT_BYTES`), under which the kernel adds no new buffers once that much is unsent
    (kernel `ip-sysctl` documentation; Cloudflare found 16 KiB kept connections fully used, P. Meenan
    2018, "HTTP/2 Prioritization with NGINX"). It is set without `unsafe` in this workspace: socket2
    0.6.5, already in the tree through tokio and added as a direct dependency with its `all` feature,
    has `Socket::set_tcp_notsent_lowat` for Linux and Android, reached through
    `socket2::SockRef::from(&tcp)` in axum 0.8.9's `ListenerExt::tap_io`, in `main.rs` and in the test
    server; a failure is logged at `warn` and the connection goes on. socket2 does not expose it on
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
    brainstorm left open; it reverses if either message is ever sent unasked.
13. **Knowledge as a seam.** The scene is built through a `SceneKnowledge` trait: the level granted
    for each body and the craft that are contacts. Production uses `GrantAsked`, the level asked and
    no craft, as plan 14's handlers grant today; the sensors plan replaces it. The Knowledge-bound
    test drives the scene with a restrictive fake and places cameras anywhere, so that the rule is
    tested before anything restricts it.
14. **Size classes and serialisation.** A scene state or an arrival holds a whole `SystemBodiesDto`,
    so `Subscribed` for the scene is a large body (`requests::is_large`) and is serialised on the
    pool, and so is a notification carrying an arrival; the rest are small and serialised on the
    runtime. `scene_ship`, `scene_cameras` and `unsubscribe` are small.

## Tasks

T1 comes first. T2 and T3 are a chain in the sim and can run beside everything on the protocol.
T4 precedes T5, whose subtasks run in order; T6 follows T5.a; T7 follows T6; T8 follows T5.b and
T7; T9 follows T8. T10 (binary frames) can start after T1 and runs beside T4–T9; T11 follows T10.a.
T12 follows T5.c; T13 follows T3 and T12; T14 follows T8, T11 and T13. T15 closes.

### R03.T1 Reconcile, reserve the kinds, rule question 21

Check what exists: whether P12.T9 has landed (`SubscribeRequest`, `ServerMessage::Notification` in
`crates/hyperion-protocol/src/envelope.rs`), whether plan 14's `state_at` and plan 11's
`star_states_at` exist, and the names under Consumes against the tree; record the findings in this
plan. Edit plan 04's table of reserved kinds (`docs/agent/plans/galaxy-generation/04-server-and-protocol.md`,
"Extending the convention") with a row for this plan, `scene_ship` and `scene_cameras`, and a note
that `subscribe` and `unsubscribe` stay plan 12's and may be built by R03 to plan 12's design; add
the matching note to plan 12's P12.T9. Add the test of Design note 12 to
`crates/hyperion-server/tests/websocket.rs`: a client that sends `hello`, a range query and a ping
receives only text frames that parse as today's `ServerMessage`. Extend the doc comment of
`PROTOCOL_VERSION` with the ruling and its condition.

Files: the two plan documents, `crates/hyperion-server/tests/websocket.rs`,
`crates/hyperion-protocol/src/lib.rs`. Acceptance: `npx prettier --check` on both plans;
`grep -n scene_ship docs/agent/plans/galaxy-generation/04-server-and-protocol.md`;
`cargo test -p hyperion-server --test websocket`; `cargo test -p hyperion-protocol
protocol_version_is_two`.

### R03.T2 In-system retarded evaluation

Build `SystemObserver`, `BuildSystemObserverError`, `SystemTrajectory`, `InSystemRetardation`,
`TraceInSystemError` (with `NotConverged`), `retarded_in_system` and the two constants (Design note
7), in the manner of plan 12's `retarded` (`light_time` for metres to a `Span` rounded to the
nanosecond; `before` for the subtraction): the light time by the fixed point, stopping when a
correction changes it by at most 1 ns and refusing after 10; the apparent point by the Lorentz form
of Design note 7, in its stable arrangement with no division by |v_o|. The doc comments cite Design
note 7's sources: NAIF's `spkezr_c`, `spkaps_c` and `stelab_c` and the Aberration Corrections
Required Reading, the NOVAS C3.1 guide, and the Explanatory Supplement, whose section numbers are
re-checked against the book first, as the Figures rule requires.

Files: `crates/hyperion-sim/src/observe/{mod.rs,in_system.rs}`.

Tests:

- A source at rest is seen at its own position, with τ = d ÷ c to the nanosecond; a source in
  uniform motion against the closed-form root of the light-cone quadratic in the system frame, to
  1 ns.
- A source 100 au from the observer on a 100 km/s orbit converges to at most 1 ns in three
  corrections, and one 2.7 × 10⁵ au away at 800 km/s within the cap; each correction's change is at
  most β_r times the one before, β_r the source's radial speed over c; a source rigged to sit on a
  rounding boundary between two nanoseconds stops rather than cycling; a source faster than the cap
  allows returns `NotConverged`.
- An observer at β = 0.01 sees a fixed source at an angle θ from its velocity at the relativistic θ′,
  cos θ′ = (cos θ + β) ÷ (1 + β cos θ), to 10⁻¹²; at 30 km/s the result equals the first-order
  r + v_o τ to within β² ÷ 4 in direction.
- An observer on a circular orbit (a ≈ 8.7 m/s²) seeing a body about 2 au away differs from the
  brainstorm's x_B(t − τ) − x_o(t − τ) by an angle of a τ ÷ 2c to 10%.
- The brainstorm's figure, reworded (Design note 7): a body at rest and an observer 400 km away moving
  at 7.7 km/s across the line of sight: the light-time point is not displaced and the apparent point
  is displaced 10.27 ± 0.01 m along the observer's velocity; adding the same 30 km/s to both leaves
  the apparent displacement unchanged to 1 mm while the light-time point moves about 40 m.
- For an observer and source moving together the apparent vector equals the present one to
  |Δv⊥| ÷ c; a source absent at the emitted time is `NotPresentThen`; determinism across two runs.

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
Acceptance: `cargo test -p hyperion-sim observe::in_system planetary::system`;
`cargo test -p hyperion-sim --test observe_in_system_golden`; `just ci`.

### R03.T4 Wire types of the scene

Add `crates/hyperion-protocol/src/scene.rs` with every type of this plan's own under Provides, each
with a wire-form test as `rust-dev.md` requires: `FramePositionDto` tagged by `frame` in snake case;
`time_rate` as a `u32`; `SceneClockStateDto` in snake case; `SceneCraftDto` documented as a draft
(Design note 4). Export them from `lib.rs`, run `just gen-protocol`, re-export from
`packages/protocol/src/index.ts`. No kind is added yet.

Files: `crates/hyperion-protocol/src/{scene.rs,lib.rs}`, `packages/protocol/src/generated/*`,
`packages/protocol/src/index.ts`. Acceptance: `cargo test -p hyperion-protocol scene`;
`just gen-protocol-check`; `just ci`.

### R03.T5 The subscription envelope

- **R03.T5.a Protocol.** If P12.T9 has not landed: `SubscribeRequest`, `UnsubscribeRequest`,
  `Subscribed`, `SubscriptionTopic::Scene`, `SubscriptionState::Scene`,
  `ServerMessage::Notification`, `NotificationBody::Scene`, the kinds `subscribe` and `unsubscribe`,
  and this plan's `scene_ship` (`SceneShipRequest`) and `scene_cameras` (`SceneCamerasRequest`) in
  `RequestBody`, `ResponseBody` and `REQUEST_KINDS`, as plan 04's "Extending the convention" says;
  if it has, the `Scene` variants and the two kinds only. The server's exhaustive matches
  (`requests::{kind, is_large}`, `Handlers`, the `every_body` walk) gain them, answering
  `unsupported` until R03.T6 and R03.T8. Run `just gen-protocol`. Tests: wire forms of every new
  type and of `notification`; `REQUEST_KINDS` holds the new strings; `is_large` marks the scene's
  `Subscribed` large (Design note 14). Acceptance: `cargo test -p hyperion-protocol`;
  `cargo test -p hyperion-server requests::`; `just ci`.
- **R03.T5.b Subscriptions in the connection.** `crates/hyperion-server/src/subscriptions.rs`: a
  table owned by the connection task, at most `MAX_SUBSCRIPTIONS` (4) a connection, numbered from
  1 per connection and never reused on that connection; `subscribe` registers one and answers
  `Subscribed`, `unsubscribe` ends it, a request naming an unknown subscription is `bad_request`
  with `field: "subscription"`, and every subscription ends with its socket. Each subscription has
  a `PendingPush` whose `Merge` implementation follows Design note 5, and the connection's loop
  queues it when `Outbound::has_room_for` allows and otherwise leaves it pending. Add the limits
  and their figures to `limits.rs` and its test. `TestClient::next_notification()`. Tests (unit, in
  `subscriptions.rs` and `outbound.rs`, with an injected topic that pushes on demand and a stuck
  writer from `testing.rs`): ten pushes into a stuck writer leave one pending push holding all ten
  changes merged; bodies merged by ID with the latest winning; an arrival clears earlier bodies;
  the fifth subscription is refused; unknown subscription; nothing pending survives the socket.
  Acceptance: `cargo test -p hyperion-server subscriptions:: outbound::`; `just ci`.
- **R03.T5.c The client's helper.** `packages/protocol/src/subscriptions.ts`:
  `RequestClient::subscribe(topic)` returning a `Subscription` with its initial state,
  `onNotification(listener)` and `unsubscribe()`; `handleServerMessage` routes `notification` to
  its subscription and returns `true`, drops one for an unknown subscription, and ends every
  subscription on `linkLost()`; `NotificationOf<T>`. Tests (Vitest): routing by subscription; a
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
`resolve`, as plan 04 requires of IDs read from a client), and answers with the clock it set. The
clock reaches the window's edge in state `window_limit` and holds there.

Tests (paused tokio clock): the clock advances at its rate and not while paused; `instant_of` inverts
`at_instant` to the nanosecond at 1× and 100,000×; the window's edge; a bad rate, time or frame is
`bad_request` naming its field; a second `scene_ship` supersedes the first for every subscription of
the universe. Acceptance: `cargo test -p hyperion-server scene::clock`; `just ci`.

### R03.T7 The scene's core

`crates/hyperion-server/src/scene/core.rs`, pure and synchronous: `SceneCore::build(inputs) ->
SceneStateDto` and `SceneCore::advance(&mut self, inputs) -> SceneDelta`, where the inputs are the
clock reading, the stand-in's pose, the cameras, a `SceneKnowledge` and a `CraftSource`, and the
system comes from plan 14's body cache through a caller-supplied closure (the core does no I/O).
It selects the system by `frame_at` with hysteresis, builds `SystemBodiesDto` by plan 14's
converter at the granted level, finds the next `valid_until` among the scene's bodies (a scene time
at which `advance` must be called), re-evaluates each body whose `valid_until` has passed, and
compares what `SceneKnowledge` grants with what was sent. `GrantAsked`, `NoCraft` and the
`CraftState` it maps to `SceneCraftDto`. Camera validation as Design note 6.

Tests: arrival and leaving across a sphere of influence, with no flicker inside the hysteresis band
(a stand-in moved back and forth across the boundary by less than it); a body whose `valid_until`
passes is re-sent once with its new elements; a knowledge change re-sends exactly the bodies it
touched; advancing in one step or ten gives the same merged delta; the galactic frame has no system;
a camera in another system's frame is refused; the scene holds the system's stars in `hosts`.
Acceptance: `cargo test -p hyperion-server scene::core`; `just ci`.

### R03.T8 The scene subscription, live

- **R03.T8.a Subscribe, report, push on change.** The `subscribe` handler for `Scene` builds the
  core on the pool at `Priority::Interactive` and answers the whole state; a task per subscription,
  owned by the connection's subscription table and ended with it, waits on the clock's `watch`, the
  next `valid_until` deadline mapped to an instant by `instant_of`, camera reports and the heartbeat,
  and merges each delta into the pending push. `scene_cameras` replaces a subscription's cameras.
  Tests (integration, `crates/hyperion-server/tests/scene.rs`): subscribe in a pinned system and
  receive every body of `system_bodies` for the same time and level, in the same form; a heartbeat
  each second with `sequence` rising by one (paused clock); a `scene_ship` rate change is pushed at
  once to two subscriptions; a body's `valid_until` passing at 100,000× is pushed; moving the
  stand-in out of the system pushes `no_system`; an unknown subscription and a camera out of reach
  are refused naming their fields. Acceptance: `cargo test -p hyperion-server --test scene`.
- **R03.T8.b Craft at the tick, and a slow reader.** While the scene holds craft, the task pushes
  them every `CRAFT_PUSH_INTERVAL`, 15.625 ms (64 Hz, which is exactly 15,625,000 ns: the
  single-player brainstorm's The clock, re-checked there), with missed ticks skipped. A test
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
  contact or a section above its body's granted level: asserted on the JSON, as plan 12 asserts that
  no bearing contact has a `host` key. R09 extends this test to surveyed cells.
- **Two clients agree.** Two `TestClient`s subscribed to one universe's scene, one of them joining a
  second later, receive notifications whose bodies, clock and ship are equal apart from
  `sequence` after the second's arrival, for a minute of scene time at 10× with a `valid_until`
  crossing and a stand-in move; both states equal `system_bodies` answered for the same time.

Files: `crates/hyperion-server/tests/scene_knowledge.rs`, `crates/hyperion-server/tests/scene_agree.rs`.
Acceptance: `cargo test -p hyperion-server --test scene_knowledge --test scene_agree`; `just ci`.

### R03.T10 Outbound binary frames

- **R03.T10.a Header, chunks and the handler's answer.** `crates/hyperion-server/src/bulk.rs`:
  `BinaryFrameHeader`, `encode_header`, `chunk(payload, request) -> impl Iterator<Item = Bytes>` at
  `MAX_BINARY_FRAME_BYTES`, header included (Design note 10). The handler seam's output becomes
  `Answer { body: ResponseBody, bulk: Option<BulkPayload> }` with `From<ResponseBody>`, so every
  existing handler changes by one `.map(Answer::from)`; `BulkManifestDto` in the protocol with its
  wire-form test. Document the header in `crates/hyperion-protocol/src/lib.rs`'s module docs. Add
  the three limits with their figures and citations to `limits.rs` (Design note 11), and set
  `TCP_NOTSENT_LOWAT_BYTES` on every accepted socket on Linux through `socket2::SockRef` in axum's
  `ListenerExt::tap_io`, in `main.rs` and `testing.rs`, with socket2 (feature `all`) added to
  `[workspace.dependencies]`; a failure is logged at `warn`. Tests: the header's bytes for a pinned
  request, byte for byte, which R03.T11 repeats in TypeScript; a payload of 0, 1, exactly one
  frame's and 15 MB splits into the right count (60 for 15 MB) with every frame at most 262,144
  bytes and the payload rejoined equal; on Linux, `SockRef::tcp_notsent_lowat()` of an accepted
  connection reads back 65,536. Acceptance: `cargo test -p hyperion-server bulk:: limits::`;
  `cargo test -p hyperion-protocol bulk`; `just ci`.
- **R03.T10.b Streaming under the budget.** The connection sends a bulk answer's chunks in order,
  each queued only once the bulk bytes queued are under `BULK_QUEUED_BYTES`, then the terminal
  response (Design note 11); the request stays in flight until then; `cancel` stops the chunks and
  answers `cancelled`; a write timeout closes as plan 04's T15 does. Tests (unit over real sockets,
  with an injected handler that answers 15 MB in bulk, in `outbound.rs` or a new `bulk` test
  module): every chunk precedes the terminal response, in order; the server's queued bulk bytes
  never exceed one chunk; a scene push issued mid-transfer arrives before the transfer ends;
  `cancel` after the third chunk gets `cancelled` and no further chunk; a stuck reader is closed by
  the write timeout with nothing held. Measure the added latency of a heartbeat push during a 15 MB
  transfer, from the server's send to the client's receipt, over an emulated 40 Mbit/s link (Design
  note 11: a reader with a fixed 64 KiB `SO_RCVBUF` reading through a 5 MB/s token bucket), with the
  low-water mark on and off, and record both in Design note 11 (a finding over 100 ms with it on).
  Acceptance: `cargo test -p hyperion-server --lib bulk:: outbound::`; `just ci`.

### R03.T11 The client receives bulk

Set `binaryType = "arraybuffer"` in `lib/connection.ts` immediately after the socket is constructed,
before `open`, and turn today's `typeof event.data !== "string"` guard into an
`instanceof ArrayBuffer` branch that routes to `RequestClient::handleBinaryFrame`; chunks are kept
as `ArrayBuffer`s and handed over whole, never parsed on arrival (Design note 11);
`packages/protocol/src/bulk.ts`: `parseBinaryFrameHeader` (refusing a bad magic, format or length,
which the link reports as `error` and does not close on), `BulkAssembler` keyed by request ID, and a
request outcome that resolves with the response and its chunks once the terminal response's manifest
matches what arrived (a mismatch fails the request as `internal`). Tests (Vitest, `FakeWebSocket`
gaining `serverSendsBinary`): the pinned header of R03.T10.a parses; chunks out of order or missing
fail the request; `cancel` discards partial chunks; link loss discards everything. Acceptance:
`pnpm --filter @hyperion/protocol test`;
`pnpm --filter hyperion exec vitest run src/renderer/src/lib/connection`; `just ci`.

### R03.T12 The client's scene store and clock

`lib/scene/sceneWire.ts`: `toSceneModel(state)` over `toSystemBodiesModel`, and
`applySceneNotification(model, notification)` (arrival replaces, bodies merge by ID, craft and clock
replace; a `sequence` gap is accepted, since the server conflates, and a sequence that goes back is
an error). `lib/scene/sceneClock.ts`: `renderTime(clock, receivedMs, nowMs)` in `UniverseTime` of
whole seconds and nanoseconds (Design note 9), still when paused or at the limit. Tests: applying
every notification of a recorded sequence equals the state of a fresh subscription at its end;
`renderTime` at 1× and 100,000× from a time 999 years out keeps nanoseconds; paused holds.
Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/lib/scene`; `just ci`.

### R03.T13 Apparent positions on the client

`lib/scene/apparent.ts`: `apparentPosition(track, observer, time, previousTau?)` mirroring
`retarded_in_system` (Design note 7): τ rounded to the nanosecond, the emitted time formed as a
`UniverseTime` less a span, the same stop rule of at most 1 ns inclusive with a cap of 10, a warm
start from the previous frame's τ allowed, and the same Lorentz form for the apparent point; a
body's track from `composePosition`, a star's from the hosts' hierarchy. `sceneAt(model, observer,
time)` returns each present body's and star's `geometricM`, `apparentM` and `emitted`, each body's
`hillRadiusM` (plan 14's a(1 − e)(m ÷ 3M)^⅓, M from the orbit's μ), and the ship's local body, the
one whose Hill sphere holds the ship stand-in. The observer is the scene's ship stand-in at the
render time.

Tests: first log the largest discrepancy against R03.T3's golden (read with `?raw` as
`lib/orbit.test.ts` reads plan 14's), then pin every vector within the larger of 1 mm + 10⁻¹²
(|x_B(t − τ)| + |x_o(t)|) and ten times that measured value, and every emitted time equal to the
nanosecond; Hill radii to 10⁻¹² relative of the golden's; two frames 16 ms apart with a warm start
agree with a cold start within the same bound; a camera placed at a far body sees that body and its
moons shifted together, their relative vectors within |Δv_moon| τ of the geometric ones; only the
ship's local body is named local; a body not present at its emitted time is left out; a comoving
observer's apparent vectors equal the present ones to |Δv⊥| ÷ c. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/lib/scene/apparent`; `just ci`.

### R03.T14 `useScene` and the camera reports

`lib/scene/useScene.ts`: subscribes on mount through R03.T5.c's helper, keeps the model and the
receipt time, exposes `sceneAt(nowMs)` for R02, reports cameras through `CameraReporter` (at most
4 Hz, and at once when a camera's frame changes), resubscribes after a reconnection, and marks the
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
transfer of 60 chunks and its terminal response arrives whole and in order, since every unit test
uses `FakeWebSocket` (run by hand, or in R01's headless harness if it can open a socket, and
recorded); the envelope's state against P12.T9 for plan 12's writer. Acceptance: `just ci`; the
figures are in the plan.

## Verification

- **Honest scene:** the Knowledge-bound test over 200 camera placements (T9); the unknown and
  out-of-reach refusals (T8).
- **One scene:** the two-clients tests on the server (T9) and the client (T14); every push states
  its time and rate (T8).
- **Apparent positions:** the in-system tests and figures (T2), the tracks against plan 14's
  positions bit for bit (T3), and the client within 1 mm + 10⁻¹² of the barycentric distances of the
  golden (T13).
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
  the client reaches with the sim's goldens (R03.T13 measures before pinning), and the low-water mark
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
  scene's farthest observer has seen the change.
- **Plan 12 and plan 14 coordination.** If P12.T9 and this plan run in parallel lanes, one builds the
  envelope and the other rebases onto it; R03.T1 records which. The velocity asks of plans 14 and 11
  are small, but land in their files; their writers should see them in R03.T1's note.
- **The craft DTO is a draft** owned by the sessions plan (Design note 4). No production code sends
  it, so reshaping it moves no version.
- **Cameras bound little until R09.** Within a system the whole system is sent, so a camera report
  does not yet change what a client receives beyond refusals; it is built now so that R09's cells
  and the sensors plan's contacts have their input, and so that the Knowledge-bound test covers it.
- **Client clock and latency.** A client renders at the push's time plus its own elapsed time, so it
  lags the server by one delivery, which at 100,000× on a 10 ms link is 1,000 s of scene time. That
  is the brainstorm's bound ("differ only by the delivery of the latest push") and is not
  compensated; half the ping's round trip could be added later if a bridge needs it.
- **Body frames on the wire.** `FramePositionDto::Body` names a non-rotating body frame, plan 01's
  `Frame::Body`. R02's `BodyFixedPosition`, which rotates, is a position type and does not go on the
  wire here; if R02 needs it there, it adds a variant.

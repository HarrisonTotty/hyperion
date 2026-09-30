# Plan R02: Real-Scale Foundations and the Wireframe `VIEW`

- **Milestone:** Rendering milestone RM1 (R01–R04: a wireframe view at real scale, and the
  determinism checks).
- **Depends on:** [R01 Graphics platform and the engine adapter](01-graphics-platform-and-engine.md)
  for everything drawn; [R03 The scene subscription and bulk
  transport](03-scene-subscription-and-transport.md) for generated scenes (R02.T17 only). Built
  galaxy plans read: 01 (`coords`, `time`), 04 (the request envelope), 05 (the general spatial view
  and its guide edits), 06 (the stellar brief on range rows, P06.T33–T34), 14 (bodies, orbits,
  `hill_radius`, `system_bodies`). Not yet built and asked for: galaxy plan 14's rotation and
  body-fixed frame ([P14.T14.c](../galaxy-generation/14-planetary-systems.md)).
- **Brainstorm sections covered** (by heading, in [the rendering
  brainstorm](../../brainstorming/rendering-and-planets.md)): [Real-scale
  foundations](../../brainstorming/rendering-and-planets.md#real-scale-foundations) in full — "The
  floating origin is already in the simulation" (the position types, differencing, per-patch
  origins, the rotation-only view matrix and frame changes, but not the retarded evaluation, which
  is R03's), "Depth: reversed-Z, and no logarithmic depth", and "Luminance in physical units, and an
  exposure model" up to the full-screen pass; [The view before the
  planets](../../brainstorming/rendering-and-planets.md#the-view-before-the-planets); the wireframe
  half of [Two styles of one
  renderer](../../brainstorming/rendering-and-planets.md#two-styles-of-one-renderer) (not terrain,
  which is R10's); [The free camera](../../brainstorming/rendering-and-planets.md#the-free-camera)
  for local views (the main screen's commanded camera is R07's); [What the guide must
  gain](../../brainstorming/rendering-and-planets.md#what-the-guide-must-gain), all nine items as
  drafts for the owner; [Accessibility, which a canvas
  threatens](../../brainstorming/rendering-and-planets.md#accessibility-which-a-canvas-threatens);
  the `view/` bullet of [Runtime and code
  shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape); the "Projection and
  camera", "Precision, as arithmetic", "Exposure and photometry", "The precision scene" and "The
  frame-change scene" items of [Testing](../../brainstorming/rendering-and-planets.md#testing); the
  second half of step 1 of [Suggested order of
  attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack). From the
  single-player brainstorm, [The view
  outside](../../brainstorming/single-player-experience.md#the-view-outside) and the "The view" item
  of its [Testing](../../brainstorming/single-player-experience.md#testing).

## Goal

When this plan is done, the bridge client has a `VIEW` display that draws the surroundings as a
wireframe, in perspective, at real scale, from a free camera with seat, chase and free presets.
Bodies are graticule spheres true to scale, rings are their ellipses, orbits are drawn on request,
craft are vector hulls with their hidden lines removed, and stars are sprites at their true apparent
magnitudes, exposed through a photometric exposure model shown as an instrument. Every position
reaches the GPU as an `f64` difference from the camera narrowed to `f32`, through a rotation-only
view matrix and a reversed-Z, `depth32float`, infinite-far projection; frame changes, the camera's
own body frames included, rebase in one operation and are continuous to a millimetre and a pixel.
Two kept scenes, the precision scene and the frame-change scene, prove this by test and by hand. The
simulation has a body-fixed position type and a body-frame selection rule that later plans reuse,
the shared vector and frame code has left `spatial/`, and the nine guide items are drafted for the
owner. Stars come from the range query's rows until R06's sky; bodies come from kept scenes, and
from R03's scene once it lands.

## Scope and non-goals

In scope:

- The simulation's `coords::BodyFixedPosition`, `BodyFixedVector` and `BodyFixedRotation`, and a
  body-frame selection rule beside `galaxy::frame`.
- An optional absolute V magnitude on the stellar brief of range rows, as a named ask of galaxy plan
  06, built here if it has not landed.
- The client's `view/` directory: position types mirroring the simulation's, camera-relative
  differencing and narrowing, the per-patch origin convention, the camera, its projection and
  rotation-only view matrix, the depth policy and transparent-layer ordering, frame selection and
  rebasing, the camera presets and free flight, photometry, exposure and the per-sprite tone curve,
  the engine-agnostic view scene, the wireframe style's geometry and draw list, and its WGSL shaders
  submitted through R01's adapter.
- The `VIEW` display: canvas, DOM list, label block, exposure instrument, camera controls,
  `prefers-reduced-motion`.
- The interim star field from range rows, labelled as such.
- The kept precision and frame-change scenes, their automatic tests and their by-hand records.
- Drafts of all nine items of "What the guide must gain" and their nomenclature entries.
- The move of `vec3`, `frame` and the direction conventions out of `spatial/`.
- The wireframe's own low setting and its benchmark at 1080p on the UHD 620.

Non-goals:

- The platform switches, the engine adapter, one canvas context per view, lazy loading, the
  WGSL-only guard and the SwiftShader harness: all R01's. This plan submits through them.
- The scene subscription, extrapolation by time and rate, camera reports, retarded and apparent
  positions within a system, and binary frames: R03's. This plan draws what it is given.
- The photorealistic style, the style switch, the full-screen AgX pass, histogram metering, bloom, a
  second view's budget and the main screen's commanded camera: R07's. The view's style type has only
  `wireframe` here and R07 adds its variant.
- Terrain of any kind, the wireframe's depth-only terrain and contours (R10), the `TERRAIN:`
  annunciations (R05), survey coverage (R09, R10).
- The sky request, the star cubemap, the galactic band and the local star as a limb-darkened disc
  (R06). Here the local star is a graticule sphere at its radius, as the single-player brainstorm's
  wireframe has it.
- A ship, sessions, ship state and the flight model. The seat and chase presets are defined against
  the scene's own ship: a kept scene's test craft, or R03's ship stand-in (`scene_ship`) until
  sessions give a real one.
- Hull definitions as ship data. This plan defines the outline format the wireframe draws and one
  hand-made hull for the kept scenes; the definitions themselves belong to the single-player plan
  that builds craft.
- Any change to the Content Security Policy: nothing here loads WebAssembly or a new origin.

## Provides

Client paths are under `apps/hyperion/src/renderer/src/` unless they start with `crates/` or
`docs/`. Signatures are sketches.

### Simulation (`hyperion_sim`)

```rust
// coords (new file coords/body_fixed.rs, re-exported from coords)
pub struct BodyFixedPosition([f64; 3]);   // m from the body's centre, along its rotating axes
pub struct BodyFixedVector([f64; 3]);     // a displacement along those axes
pub struct BodyFixedRotation { /* rows: [[f64; 3]; 3], orthonormal to 1e-12 */ }
impl BodyFixedRotation {
    pub fn from_rows(rows: [[f64; 3]; 3]) -> Result<Self, BuildRotationError>;
    pub const IDENTITY: Self;
    pub fn to_body(&self, p: &BodyFixedPosition) -> BodyPosition;        // R · p, fixed order
    pub fn to_body_fixed(&self, p: &BodyPosition) -> BodyFixedPosition;  // Rᵀ · p
    pub fn vector_to_body(&self, v: &BodyFixedVector) -> BodyVector;
}
pub struct BodyVector([f64; 3]);          // a displacement in the non-rotating body frame (new)

// planetary::frame (new): the body-frame rule, Design note 6
pub const BODY_FRAME_ENTRY: f64;          // 0.9: enter at ratio ≤ 0.9 of the Hill radius
                                          // (siblings: galaxy::frame::FRAME_HYSTERESIS)
pub struct BodyFrameCandidate { /* id: BodyId, parent: Option<BodyId>, distance: Metres,
    hill_radius: Metres */ }
pub fn select_body_frame(candidates: &[BodyFrameCandidate], current: Option<BodyId>)
    -> Option<BodyId>;
```

Galaxy plan 14 is asked to return `BodyFixedRotation` from `body_fixed_at(body, t)` (P14.T14.c).

### Protocol

- `StellarBriefDto.absolute_v_mag?: number | null` (Rust `Option<Option<f32>>`-style optional field,
  written only when the brief is asked for; `null` where plan 06's photometry has no value), filled
  by the server's `brief_dto` (`crates/hyperion-server/src/convert/stellar.rs`).

### Shared geometry (moved out of `spatial/`)

- `geometry/vec3.ts` (`Vec3`, `vec3`, `add`, `sub`, `scale`, `dot`, `cross`, `norm`, `normalise`)
  and `geometry/frame.ts` (`LocalFrame`, `localFrameAt`, `planeFrame`, `toLocal`, `fromLocal`,
  `cylindrical`, `AXIS_TOLERANCE_LY`, `PARALLEL_TOLERANCE`), unchanged in content.

### `view/coords/`

```ts
type ViewPosition =
  | { readonly kind: "galactic"; readonly position: GalacticPosition }
  | { readonly kind: "system"; readonly system: SystemIdHex; readonly m: Vec3 }
  | { readonly kind: "body"; readonly body: BodyIdHex; readonly m: Vec3 }
  | { readonly kind: "body_fixed"; readonly body: BodyIdHex; readonly m: Vec3 };
interface FrameOrigins {
  // per frame time: where each frame's origin is
  systemBarycentre(system): GalacticPosition;
  bodyCentre(body): Vec3; // system-frame metres
  bodyFixedRotation(body): Rotation3 | null; // null: rotation not modelled
}
function galacticDeltaM(from, to): Vec3; // cells first, then offsets
function relativeToCamera(p: ViewPosition, camera: CameraPose, o: FrameOrigins): Vec3; // f64
function narrow(v: Vec3): Float32Array; // the only f64 → f32 step
interface OriginRelative {
  readonly origin: ViewPosition;
  readonly offsetsF32: Float32Array;
}
function originMinusCamera(o: OriginRelative, camera, origins): Float32Array;
```

### `view/camera/`

`CameraPose { frame: CameraFrame; positionM: Vec3; orientation: Quaternion }`, `CameraFrame`
(`system`, `body`, or `craft` for a pose held as an offset from a craft, Design note 22; `galactic`
when the scene is), `Quaternion` and its operations, `viewRotation(orientation): Mat3F32` (no
translation), `perspectiveReversedInfinite(fovXRad, aspect, nearM): Mat4F32`,
`project(v, camera, viewport)`, `pixelSolidAngle(dir, camera, viewport)`, `NEAR_PLANE_M`,
`FOV_STEPS_DEG`, `selectCameraFrame` (the TS twin of `select_body_frame`),
`rebase(pose, next, origins): { pose; change: FrameChange }`, `CameraPreset` (`seat`, `chase`,
`free`), `RenderStyle` (`"wireframe"`; R07 adds `"photorealistic"`), `ViewRole` (`"eye"` |
`"camera"`, which R06's star limits read), `ViewId`,
`CameraState { preset; target; style; role; pose }`, `cutTo`, `EASED_MOVE_S = 0.4`,
`stepFreeCamera(state, input, dtS, reducedMotion)`.

### `view/depth/`

`DEPTH_FORMAT = "depth32float"`, `DEPTH_CLEAR = 0`, `DEPTH_COMPARE = "greater-equal"`,
`ORDERING_RELATIVE_SEPARATION = 1e-6`, `separable(aM, bM, distanceM): boolean`,
`occluderRadius(radiusM, distanceM)`, `transparentLayerOrder(layers, camera): layers`.

### `view/photometry/`

`V0_ILLUMINANCE_LX = 2.54e-6`, `illuminanceLx(apparentV)`, `apparentV(absoluteV, distanceM)`,
`pixelLuminance(illuminanceLx, psfWeight, pixelSolidAngleSr)`, `psfPixelWeights(subpixel, sigmaPx)`,
`ev100FromTriple({ aperture, shutterS, iso })`, `ev100FromAverageLuminance(cdPerM2)`,
`exposureScale(ev100)`, `ExposureControl` (`auto` | `manual` | `inhibited`, Design note 11),
`preExpose(cdPerM2, previousExposure)` with its clamp at 65,504, `HDR_COLOUR_FORMAT = "rgba16float"`
(its alpha channel left to R07's meter weight), `toneCurve(rgbLinear): rgbLinear` (the full AgX
function) and its WGSL twin `agx` in `toneCurve.wgsl`, `DEFAULT_MAN_EV100 = -1`.

### `view/scene/`, `view/scenes/`, `view/wireframe/`, `view/shaders/`

- `ViewScene` (bodies, rings, orbits, craft, stars, own ship, time and time rate, provenance),
  `HullOutline { vertices; edges; faces }`, `TEST_HULL`.
- `precisionScene()`, `frameChangeScene()`: the kept scenes.
- `graticule(body, camera, viewport)`, `bodyRegime(angularDiameterPx)`, `orbitPath`, `ringEllipse`,
  `hullEdges`, `symbologyMarks`, `buildWireframeDrawList(scene, camera, viewport, tokens)` returning
  engine-agnostic `WireframeDrawList` (line batches with width, casing, colour token and dash;
  depth-only occluders; star sprites).
- `lines.wgsl`, `occluder.wgsl`, `starSprite.wgsl`, `toneCurve.wgsl`.

### Display

`displays/view/ViewDisplay.tsx` with `ViewCanvas`, `ViewMarkList`, `ViewLabelBlock`,
`ExposureControl` panel, `CameraControls`; `DisplayId` gains `"view"` on `F4`;
`useInterimStars(requests, universe, centre, time)`.

### Test helpers

`test/viewFixtures.ts` (`aViewScene`, `aCameraPose`, `aBody`, `aStarRow`), and the sim golden
`crates/hyperion-sim/tests/golden/frame/body_frames.golden` read by the TS twin.

### Documentation

`docs/frontend/ux-guidelines.md`: drafts of the nine items and their nomenclature entries, each
awaiting the owner (R02.T2).

## Consumes

Names are the owning plans' as they stand; where a name has changed by the time this plan runs, only
the call sites here change.

- **R01:** `loadRenderEngine` (the dynamic import and manual chunk) and its `RenderEngine`:
  `createView` (one `GPUCanvasContext` per canvas on the shared device), `createMesh`,
  `createMaterial` with `WgslMaterialSpec` (the GLSL guard), `RenderView.render(FrameSubmission)`
  whose `viewRotation` and `projection` this plan fills and whose `DrawItem.offsetFromCameraM` is
  this plan's `originMinusCamera`, `resize`, `onFault` with `GraphicsFault` and its annunciation
  wording (R01 design note 10), which the view's label block carries; `DepthPolicy`
  `"reversed-z-float"`; `WGSL_CATALOGUE`, where this plan registers its materials; the SwiftShader
  harness (`just test-render`). Asked of R01: that the frozen projection is passed through
  unchanged, with no half-Z conversion and no Y flip on top of it, and that the scene is
  right-handed (`scene.useRightHandedSystem = true`) or every R02 pipeline two-sided, since this
  plan's matrix is right-handed (Design note 4); that the engine does not cull by its own frustum
  (Design note 8); that `WgslMaterialSpec` gains `depthBiasAway?: { constant; slopeScale }`
  (positive meaning away from the camera, mapped to Babylon's positive `zOffsetUnits` and `zOffset`,
  which Babylon negates under reversed depth) and `cullMode: "none" | "back"` (Design note 5); that
  sprites blend in linear light through an sRGB view of the canvas (`viewFormats`); and R01's
  nomenclature drafts (its T5.c), which R02.T2.f absorbs into the single pass over the guide.
- **R03:** `useScene` and its scene model (`lib/scene/`): bodies from `SystemBodiesDto` and
  `BodySummaryDto` as orbital elements with their `DetailLevelDto`, craft (`SceneCraftDto`), the
  ship stand-in's `KinematicsDto`, the clock (`SceneClockDto`) and `renderTime`; `sceneAt`, which
  gives every body's and star's `geometricM`, `apparentM` and `emitted`, each body's `hillRadiusM`
  (plan 14's a(1 − e)(m ÷ 3M)^⅓, which the camera's frame selection reads) and names the ship's
  local body, from which this plan draws the ship's local body geometrically at the present and
  every other body at its apparent position, a free camera's own local body included (R03's design
  note 7: drawn at the present, a free camera's body would sit tens of thousands of kilometres off
  its moons); `FramePositionDto`, which `ViewPosition` mirrors; and `CameraReporter`, to which each
  local view hands its pose. R03's apparent positions (its T13) land before `useScene` (its T14), so
  the view never draws present state as if seen. Only R02.T17 waits on R03.
- **R06:** replaces the interim stars (R02.T16) with the sky request's sprites; it reuses this
  plan's photometry, sprite shader and `ViewRole`.
- **R07:** the photorealistic style's `ExposureReading`, which a wireframe view accompanying it
  takes as `AUTO`; the `photorealistic` variant of `RenderStyle`; the full-screen AgX pass, which
  includes `toneCurve.wgsl`'s `agx` so that an isolated star on black is identical in both styles
  (R07's design note 9), and which says whether it compensates the 0.79 stops by which the metered
  average sits below AgX's middle grey (Design note 11).
- **Galaxy plan 01:**
  `coords::{Frame, SystemPosition, BodyPosition, SystemVector, GalacticPosition}` and
  `coords/vec3.rs`; `hyperion-testkit`'s `golden!` and `float::assert_same_bits`.
- **Galaxy plan 03:** `galaxy::frame::{select_frame, FRAME_HYSTERESIS}`, the precedent the body rule
  follows.
- **Galaxy plan 04 and 05:** the request envelope, `useServerRequest`, `RequestStatus`,
  `lib/format.ts`, `lib/displays.ts`, `usePrefersReducedMotion`, `FakeWebSocket`, the guide's
  spatial display conventions, the symbol set (`spatial/symbols.ts`) and the casing rule over
  rasters.
- **Galaxy plan 06:** `systems_in_range` with `include_stellar` and `StellarBriefDto` (P06.T33–T34),
  `stellar::photometry::{absolute_bolometric_magnitude, bolometric_correction_v}`. Asked: the
  optional `absolute_v_mag` on the brief (R02.T5).
- **Galaxy plan 14:** `planetary::derive::limits::hill_radius`, `lib/orbit.ts` (the client's orbit
  propagation, P14.T39), `system_bodies` and `BodyOrbitDto`. Asked, not built:
  `body_fixed_at(body, t)` returning `BodyFixedRotation` (P14.T14.c). Until it lands, graticules do
  not turn and the view says `ROTATION NOT YET MODELLED` (Design note 14).

## Design notes

1. **One rule for where differencing happens.** Every position the view draws is a `ViewPosition`,
   tagged with its frame, and one function, `relativeToCamera`, turns it into an `f64` vector from
   the camera. `narrow` is the only place an `f64` becomes an `f32`, so a grep proves that nothing
   reaches the GPU in world coordinates (brainstorm, "The floating origin is already in the
   simulation"). JavaScript numbers are `f64`, so the client's differencing is the server's
   arithmetic; the cells-first `galacticDeltaM` is `galacticDeltaLy` of
   `packages/protocol/src/position.ts` in metres. The difference is not required to be small: an
   `f32`'s relative error, 6 × 10⁻⁸, is below the 5 × 10⁻⁴ rad of a pixel at every distance.
2. **Per-patch origins are the one convention for anything large.** A mesh whose vertices lie far
   from its owner's centre is an `OriginRelative`: an `f64` origin (for terrain, a `body_fixed`
   position) with `f32` offsets no larger than the mesh, and the shader receives `originMinusCamera`
   as one `f32` vector per draw. This plan uses it for graticule sphere meshes and hulls; R05 and
   R10 use it for patches. The origin is rotated into the body frame in `f64` by the body's
   `BodyFixedRotation` before it is differenced, one matrix per body per frame.
3. **The view matrix is rotation only.** `viewRotation` builds a 3 × 3 rotation from the camera's
   unit quaternion, and the 4 × 4 handed to the engine has a zero translation column. A test
   composes a 1 au translation into a naive matrix and shows the 16 km spacing, then shows the
   rotation-only path's error at the same point is below 0.1 px (brainstorm, second bullet of "The
   floating origin").
4. **Reversed-Z, infinite far, one near plane.** In WebGPU's `[0, 1]` depth range, for a
   right-handed view space looking down −z, `perspectiveReversedInfinite` is

   ```text
   | s   0    0   0 |      s = 1 ÷ tan(fovX ÷ 2), a = width ÷ height,
   | 0   s·a  0   0 |      n = the near plane,
   | 0   0    0   n |      depth = n ÷ (−z_view): 1 at the near plane, → 0 at infinity.
   | 0   0   −1   0 |
   ```

   The field of view is stated horizontally, the brainstorm's convention (60° across). `n` is
   `NEAR_PLANE_M` = 0.1 m, which keeps a hull plate at 1 m well inside the frustum and costs nothing
   far away: at 1 au the depth is 6.7 × 10⁻¹³, a normal `f32`, spaced at about 1.2 × 10⁻⁷ of itself
   (Reed 2015). The depth buffer is `depth32float`, cleared to 0, compared `greater-equal`
   (Babylon's `GEQUAL`, R01's `DepthPolicy`). Babylon's own `PerspectiveFovReverseLHToRef` builds
   the same infinite reversed form (checked in `packages/dev/core/src/Maths/math.vector.pure.ts` on
   the Babylon.js main branch, 2026-09-29), but the matrix is ours, tested without a GPU, and handed
   to the engine frozen.

5. **Ordering without a global bias** (researched 2026-09-29). Two surfaces order reliably while
   their separation along the view exceeds `ORDERING_RELATIVE_SEPARATION` = 10⁻⁶ of the distance
   (brainstorm, "Depth"); the depth itself errs by one `f32` division, 6 × 10⁻⁸ relative, so the
   bound is conservative. The wireframe relies on it twice. A body is drawn as a depth-only occluder
   sphere of radius `occluderRadius` = r − 4 × 10⁻⁶ × d (never below r ÷ 2), so that the front
   hemisphere of its own graticule always lies in front of it by four times the bound, while other
   bodies, orbits and hulls behind it are hidden. Its own graticule's back half is removed
   analytically: a point is drawn only where its outward normal faces the camera, which is exact for
   a sphere and needs no depth. Shrinking is ill-defined for a hull (a 1 m plate has no interior),
   so a hull's hidden lines use its faces as a depth-only occluder in a pipeline of its own with a
   depth bias pushing the faces away: under reversed-Z away is smaller depth, so the raw WebGPU
   values are negative, `depthBias` = −128 and `depthBiasSlopeScale` = −(w_max ÷ 2 + 1), −2.0 for
   strokes up to 2 px, with no `depthBiasClamp` (it must be 0 in compatibility mode). For
   `depth32float` the constant's unit is 2^(e − 23) of the primitive's largest depth, so −128 moves
   a face by 7.6 × 10⁻⁶ to 1.5 × 10⁻⁵ of its distance, twice the 67 the 4 × 10⁻⁶ margin needs, since
   backends may differ by 2×; the slope term covers a stroke's half-width and its one-pixel
   antialiasing fringe. Bias is valid only on triangle topologies and is never set on a line pass.
   The adapter carries it as `depthBiasAway { constant: 128, slopeScale: 2 }`, positive meaning
   away, in one place, because Babylon's `zOffset` and `zOffsetUnits` are already negated under
   `useReverseDepthBuffer` and a double negation is the likely bug. The occluder is drawn two-sided,
   so that a winding flip between our right-handed matrix and a left-handed engine cannot unhide
   every hidden line. The bias is local to the occluder pass, not the global bias the brainstorm
   rejects. Sources: W3C WebGPU, "GPUDepthStencilState" and "biased fragment depth"; Babylon.js
   main, `Engines/WebGPU/webgpuCacheRenderPipeline.ts` and `Engines/abstractEngine.pure.ts`
   (`setZOffset`, `setZOffsetUnits`).
6. **The body-frame rule** (researched 2026-09-29; a physics ruling). The frame boundary is a
   precision device, not a dynamical one: with every body's gravity integrated, the choice of frame
   changes only rounding, so the larger, already-computed Hill sphere is kept rather than the
   Laplace sphere of influence a(m ÷ M)^(2/5) of patched conics (Earth 1.47 × 10⁹ m against 9.2 ×
   10⁸ m; the Moon 5.8 × 10⁷ against 6.6 × 10⁷; Bate, Mueller and White 1971, its section number
   recalled). The radius is plan 14's pericentre form, a(1 − e)(m ÷ 3M)^⅓
   (`planetary/derive/limits.rs`), constant in time, so the boundary does not breathe with the
   orbit. The camera, and later a ship, is in the non-rotating frame of the **innermost** body whose
   sphere contains it, the system frame otherwise: smallest ratio across depths would keep a camera
   at 0.8 of the Moon's sphere, 0.3 of Earth's, in Earth's frame. Among **siblings** at the same
   depth, whose spheres can overlap (co-orbitals, Trojans, compact generated systems), the galaxy's
   own rank decides: smallest ratio, a rival taking over only at (1 − `FRAME_HYSTERESIS`) of the
   current ratio, the lower ID only on an exact tie. On each sphere's own boundary a Schmitt band:
   enter at a ratio of at most `BODY_FRAME_ENTRY` = 0.9, leave above 1, then re-run the whole rule
   with no current frame, as `select_frame` does; the band is 5.8 × 10⁶ m for the Moon and 1.5 × 10⁸
   m for Earth. Distances are geometric and present, from system-frame positions at the frame time,
   never apparent ones, identically in Rust and TypeScript. Only planets and moons are candidates; a
   wide multiple's stellar components are a note for the craft plan. The rule lives in the sim, so
   the flight model adopts it unchanged, and the client's camera runs a TypeScript twin tested
   against a golden the sim writes, as `lib/orbit.ts` is against `orbit/states.golden`. A body frame
   is non-rotating and **free-falling**, not inertial: it has no Coriolis or centrifugal terms, but
   a flight model that adopts the rule must integrate the other bodies' tidal residual (the indirect
   term −a_body) in every body frame, as Cowell and Encke propagation do.
7. **The camera's reach is the scene's system.** A free camera is clamped to the current system's
   sphere of influence, its tidal radius, which the scene states; it cannot cross into the galactic
   frame except where the scene itself is galactic, and when the scene's system changes (a jump) the
   camera returns to the chase preset about the own ship, or to the scene's default pose where there
   is none (brainstorm, "The free camera").
8. **Culling is ours.** An infinite reversed matrix gives a degenerate far plane, and an engine's
   frustum extraction from it is not something to trust at 10¹² m. The draw list is culled by this
   plan's own predicates in `f64` (a sphere against the frustum's side planes, a horizon test for
   marks behind a body's limb), and the adapter is asked to submit every mesh it is given, as
   Babylon's `alwaysSelectAsActiveMesh` allows.
9. **Lines are our own instanced quads.** WebGPU draws only one-pixel lines with no width or smooth
   antialiasing. Every stroke is a segment instance expanded to a screen-space quad in `lines.wgsl`,
   with coverage computed analytically in the fragment shader over the stroke's width plus a pixel,
   depth-tested against the occluders and never writing depth. A cased mark is two strokes of one
   instance: the casing at the guide's width in `--surface-0`, then the coloured stroke, so that the
   casing is a solid outline, never a blur (guide, "Graphs, schematics and spatial displays").
   Babylon's GreasedLine has WGSL shaders (checked on its main branch, 2026-09-29), so it would not
   trigger the GLSL fetch, but its widths, casings and dashes would then be the engine's; ours keeps
   them engine-agnostic and testable on the CPU through the draw list. Dashes are reserved for
   predicted paths and are computed in screen space so that they do not crawl.
10. **Photometry, from magnitude to pixel.** Apparent V from the brief's absolute V and the
    distance, with no extinction until R06; illuminance E = 2.54 µlx × 10^(−0.4 V) (Cox 2000;
    Crumey 2014); the star's light spread by a pixel-integrated Gaussian point-spread function of σ
    = 0.64 px (a full width at half maximum of 1.5 px), whose per-pixel weights are differences of
    the error function so that their sum is 1 at any sub-pixel position and a turning camera cannot
    make a star flash; each pixel's luminance is E × weight ÷ Ω, with Ω the pixel's own solid angle,
    Ω_centre × cos³θ off the axis, so point sources brighten with resolution while discs do not
    (brainstorm, "Luminance in physical units"). The centre pixel of a 1920 px, 60° view subtends (2
    tan 30° ÷ 1920)² = 3.62 × 10⁻⁷ sr (the brainstorm's 3 × 10⁻⁷ is the angular mean, (60°
    ÷ 1920)²), and a 16:9 corner pixel, at θ = 33.5°, 0.580 of that; a mag 6.5 star wholly in the
    centre pixel is then 1.76 × 10⁻² cd/m², which a test reproduces. Researched 2026-09-29: at σ
    = 0.64 px the centred peak weight is 0.32 and the corner-placed one 0.19, and the displayed
    total after the tone curve varies only 0.98–1.14 across sub-pixel positions for every star from
    V 6.5 to −1.46 at EV100 −1 to +2, so turning never flashes a star and a wider PSF would only
    soften them.
11. **Exposure is a camera state with three automation levels** (researched 2026-09-29). EV100 =
    log₂(N² ÷ t) − log₂(S ÷ 100) for a triple under `MAN`; EV100 = log₂(L̄ × S ÷ K) with K = 12.5
    for a metered average under `AUTO`; the saturation-based maximum is L_max = (N² ÷ t) × 78 ÷ (q
    S) = 1.2 × 2^EV100 with q = 0.65 (78 is ISO 12232's saturation constant), and the scale that
    multiplies luminance is 1 ÷ L_max, which puts the white point at 9.6 × L̄ exactly (Filament,
    "Physically based camera": "Exposure value", "Exposure settings", "Exposure"; bruop.github.io
    /exposure; after Lagarde and de Rousiers, _Moving Frostbite to PBR_, SIGGRAPH 2014). Under this
    scale the metered average lands at 1 ÷ 9.6 = 0.104, 0.79 stops below AgX's 0.18 middle grey; a
    calibration, which R07 accepts or compensates by +0.79 EV and says which. `INHIBITED` means the
    automatic function is prevented from acting: the exposure is held at its last metered value, and
    the display says who inhibited it and why, `INHIBITED · OPERATOR` or
    `INHIBITED · NO IMAGE TO METER` when the source view closed or faulted. A system-set inhibit
    returns to `AUTO` by itself when the source returns; an operator-set one does not. The commands
    are the guide's congruent pair `INHIBIT` and `ENABLE`, not `AUTO`. A wireframe view has no image
    to meter, so `AUTO` is offered only when a photorealistic view it accompanies exists (R07);
    until then the control offers `MAN`, and `AUTO` is shown unavailable with `NO IMAGE TO METER`.
    The default `MAN` value, `DEFAULT_MAN_EV100`, is **−1**, computed through the full AgX of Design
    note 12 and the PSF above at 1080p and 60°: Sirius's peak pixel reaches display-linear 0.985
    (+6.3 of AgX's +6.5 stops) and a mag 6.5 star's 5.4 × 10⁻³, visible, where +1 would leave the
    mag 6.5 star at 4 × 10⁻⁴, invisible in a lit room. At 4K the pixel is a quarter the size and
    point sources two stops brighter, so the default is stated for 1080p. The recorded run confirms
    it.
12. **The tone curve is the full AgX, applied per sprite** (researched 2026-09-29). A star sprite's
    fragment computes its pre-exposed linear-sRGB colour, from T_eff and normalised to its V
    luminance, applies `agx` and writes a display value, so the wireframe needs no HDR target and no
    full-screen pass (brainstorm, "Two styles of one renderer"). A luminance-only sigmoid would not
    do: AgX's inset matrix does not map grey to grey (its rows sum to 1.106, 0.933 and 0.961), so a
    white star would come out a different brightness and cast from R07's pass. `agx` is the whole
    per-pixel function: Rec. 709 to 2020, the inset, log₂ clamped between log₂ 0.18 − 10 and
    log₂ 0.18 + 6.5 (−12.47393 and 4.026069), Wrensch's sixth-order sigmoid
    `15.5x⁶ − 40.14x⁵ + 31.96x⁴ − 6.868x³ + 0.4298x² + 0.1191x − 0.00232` clamped at 0, the outset,
    a power of 2.2, 2020 to 709 and a clamp (three.js `tonemapping_pars_fragment.glsl.js`, after
    Filament pull request 7236 and Blender's AgX). R07 includes the same file, so an isolated star
    on black is identical in both styles; the differences left are overlapping sprites and any
    non-black background, where tone(sky + star) is not tone(sky) + tone(star), which is R07's
    reason to draw its stars before its full-screen pass. Sprites blend additively in linear light,
    through an sRGB view of the canvas (R01's to confirm), never on encoded values.
    `HDR_COLOUR_FORMAT`, `preExpose` and its clamp to 65,504 are defined and tested here for R07's
    targets, since the policy is this plan's; the target's alpha is left to R07's meter weight, as
    R07 asks.
13. **What a body looks like in the wireframe, by size.** At least 8 px across: its limb circle, its
    graticule at 15° (30° below 64 px), its equator and prime meridian a step heavier; 3 to 8 px:
    its limb circle only; below 3 px: its symbol from the ship-wide set (planet, moon or star),
    labelled as a mark. The 3 px boundary is the brainstorm's point regime; the 8 px one is where 12
    graticule lines stop being lines. Graticule and limb are generated in `f64`, camera-relative,
    subdivided until each chord's sagitta is under 0.25 px, so a body 400 km below the camera has a
    true horizon rather than a polygon's. Rings are their ellipses, inner and outer edge with radial
    ticks every 10°, as the orbit map draws them.
14. **Rotation waits on plan 14, and says so.** A graticule turns with its body through
    `bodyFixedRotation`. Until P14.T14.c provides it, `FrameOrigins.bodyFixedRotation` is `null`,
    the graticule's pole is the orbit normal (the assumption `displays/system/bodyFrame.ts` already
    makes), it does not turn, and the view's list and label say `ROTATION NOT YET MODELLED`. Kept
    scenes carry hand-set rotations so that the rotating path and the grounded-craft test run now.
15. **Hulls are outlines from definitions, and there are none yet.** `HullOutline` holds vertices in
    the hull's own frame in metres, edges as index pairs, and triangulated faces for the occluder.
    `TEST_HULL` is a hand-made 20 m craft with a 1 m square hull plate at the seat's eye point,
    labelled `TEST HULL` wherever it appears, and it also stands for R03's ship stand-in and for any
    `SceneCraftDto` whose `hull` names no known outline. The real definitions are the flight model's
    data, and the plan that builds craft is asked to emit this outline from them.
16. **The label block states what the picture is.** Always: the display class `VIEW`, the frame
    (`SYSTEM BARYCENTRIC`, `BODY <designation>`, `GALACTIC`), the time with its time system, the
    style `WIREFRAME`, the camera preset, the field of view in degrees (item 5's substitute for a
    scale bar), the exposure with its automation level, and the star source
    (`STARS: RANGE QUERY · VOLUME-LIMITED · NO EXTINCTION` until R06). While the camera is off the
    hull: `POSITIONS AS SEEN FROM SHIP`. In a kept scene: the scene's name, under the guide's
    training banner. It is DOM, B612 Mono for values, on a `--surface-0` plate, updated at 4 Hz.
17. **The DOM list is the canvas's partner.** Bodies, craft and the selection, windowed as plan 05's
    lists are, an ARIA `listbox` from which a mark is selected by keyboard, and the canvas focusable
    with an accessible name (`VIEW, WIREFRAME, SEAT`). Stars are not listed: they are not targets.
    Every target carries a range readout, from the own ship where there is one, else labelled
    `FROM CAMERA`.
18. **Motion.** The view redraws every frame while it is visible and stops when it is hidden (plan
    05's `Activity`). A preset change and a slew to a target are cuts. The 0.4 s eased move is a
    setting, `EASED CAMERA MOVES`, off by default, and not applied under `prefers-reduced-motion`,
    which also removes the free camera's acceleration ramp and damping: the camera then moves at the
    commanded rate from the first frame and stops when input stops (brainstorm, item 8). Readouts
    stay at 4 Hz.
19. **The interim stars** (researched 2026-09-29, measured on the Milky Way fixture at generator
    version 15, seed `0x0311_1000_0000_0000`, at the Sun-like point [0, 26,000, 0] ly). The census
    adds each layer's expected count from E down and drops a layer that would pass the limit of
    20,000 together with every finer one (`crates/hyperion-sim/src/galaxy/query/census.rs`), so a
    `d` floor includes E, and D with E exceed the limit beyond about 400 ly. The largest complete
    radii by bisection are E 683 ly (19,992 systems, 264 ms native), D 401 ly (190 ms), C 235 ly
    (106 ms), B 202 ly and A 139 ly (353 ms), with the cell budget never binding near the Sun. The
    counts are exact for the seed; the times are provisional, measured while other agents' tests
    shared the machine, and R02.T16.a re-measures them on a quiet one. The view sends four
    `systems_in_range` requests with `include_stellar` about the camera's system position at the
    scene time — `min_layer` `e` to 620 ly, `d` to 360 ly, `c` to 210 ly and `a` to 60 ly, about a
    tenth under the measured limits, the last reaching layer B's K dwarfs (V 6.5 to about 41 ly) and
    young T Tauri stars (about 70 ly) — merged by ID. Density varies by orders of magnitude
    elsewhere, so a request answered `over_limit` at its floor is retried once at r × (0.9 × 20,000
    ÷ Σ expected)^⅓, from the census's `LayerCensus.expected`, and the label states the radii used.
    A row whose brief has no `absolute_v_mag` (white dwarfs and remnants, most of layer E) is not
    drawn, a companion's light is not added, and positions are present, not retarded; the label says
    the set is volume-limited with no extinction. Expect a thin sky: the bright giants and B stars
    beyond 400–700 ly are missing, and R02.T16.a counts the rows at V ≤ 6.5. The requests are
    re-sent only on arrival in a new system, not as the camera moves: the parallax across a system
    is under a tenth of a pixel for every star farther than about 9 ly (brainstorm, "Baked once per
    arrival").
20. **The move out of `spatial/`.** `vec3.ts` and `frame.ts` move whole to `geometry/`, because both
    the orthographic spatial view and the perspective view use them and neither owns them; the
    orthographic `Camera` stays in `spatial/`, which is not generalised (brainstorm, "Runtime and
    code shape"). The move is a separate commit with no behaviour change, so that review sees only
    paths.
21. **The wireframe's low setting.** The station target is 60 fps at 1080p on the UHD 620 with 4 to
    9 ms of GPU time once terrain arrives (brainstorm, "Two deployments, one scene"); without
    terrain this plan's wireframe must stay under 4 ms there. Its low setting caps star sprites at
    2,000 by flux, draws graticules at 30° only, and analytic line coverage stays on, since MSAA
    would cost more than the lines. Both settings are measured and recorded (R02.T18).
22. **Near views are differenced craft-relative** (researched 2026-09-29). An `f64` at distance D
    from its frame's origin is quantised at ulp(D), and the own hull is a metre from the seat: at 1
    au the error is 3 × 10⁻⁵ m, 0.06 px at 1 m, but at 50 au it is 1 mm, about 2 px, and at a
    system's tidal radius, about 1 ly, 2 m, which makes the hull garbage; a chase camera 30 m from a
    craft 1,000 au out jitters by about 2 px. So the seat and chase poses, and a free camera about a
    target craft, are held in a `craft` `CameraFrame` as an offset from that craft, and
    `relativeToCamera` computes (p − craft) − offset, so that the hull's own offset is exact and
    only far objects carry ulp(D), where it is sub-pixel. The camera's frame selection (Design
    note 6) then applies to the craft's position, and to the camera's own only in `free`.

## Tasks

Order and parallelism: T1 first (a pure move). T2 (the guide drafts) needs nothing and can run at
any time. T3 → T4 are Rust; T5 is Rust and protocol. T6 → T7 → T8 → T9 are pure TypeScript, T8 needs
T4's golden, and T10 and T11 need only T6. T12 and T13 need T6–T11. T14 needs R01's adapter. T15
needs T12–T14. T16 needs T5 and T15. T17 needs R03. T18 is last. Every task ends with `just ci`
green; TypeScript tasks with `pnpm typecheck`, `pnpm lint` and `pnpm test`, every export with TSDoc,
and component tests by role and name with `userEvent.setup()` and no snapshot. Every figure turned
into a constant is re-checked against the brainstorm's citation and cited in its doc comment.

### R02.T1 Move `vec3`, `frame` and the direction conventions out of `spatial/`

Move `spatial/vec3.ts` and `spatial/frame.ts` (with their tests) to `geometry/`, rewrite every
import (28 files outside `spatial/` today, among them `lib/orbit.ts`, `lib/system/*`, `lib/galaxy/*`
and `displays/*`, and 19 inside it), and leave no re-export behind.

- Files: `geometry/vec3.ts`, `geometry/frame.ts`, `geometry/frame.test.ts`, and the importers.
- Tests: the moved tests, unchanged.
- Acceptance:
  `grep -rn "spatial/vec3\|spatial/frame\|from \"\./vec3\"\|from \"\./frame\"" apps/hyperion/src/renderer/src`
  finds nothing; `pnpm test` green with the same test count as before.

### R02.T2 The guide's nine items, drafted for the owner

Docs only, one commit whose message lists the nine items and the nomenclature entries, so that the
owner can accept or revert each; the code does not wait, since each is confined to a label, a
constant or a component. Every item ends in "the owner signs off". Each subtask is one section edit;
acceptance for each is `pnpm format:check` and the quoted strings found by `grep` in
`docs/frontend/ux-guidelines.md`.

- **R02.T2.a Items 1 and 5: the view as a class of display.** In "Graphs, schematics and spatial
  displays", a new "Views" entry: perspective, redrawn every frame, always labelled `VIEW`, **not a
  spatial display**, the list of what it keeps (the contact symbol set, the bracket reticle,
  `--target`, dashed predictions, the frame name, the time, the style and the camera mode, the
  light-time statement off the hull, the canvas paired with a DOM list), that it covers both styles,
  and that the wireframe style follows the ordinary rules for colour, stroke and contrast. Item 5:
  the field of view in degrees and the frame, with a range readout on every target, in place of the
  1-2-5 scale bar. Grep: `not a spatial display`, `FIELD OF VIEW`.
- **R02.T2.b Items 2, 3 and 4: the image is data, outlines, glare.** In "Colour", the photometric
  image's exception with its honesty rule (decoration labelled beside coverage and setting) and the
  flash threshold binding the image; the casing rule extended to every mark over the image, never a
  blur or a glow, and the `--surface-0` plate for DOM text over it; glare and bloom allowed on the
  image, never on symbology or chrome. Grep: `rendered image is data`, `plate`.
- **R02.T2.c Items 6 and 7: exposure and limited detail.** In "Data states" and "Controls and
  commanding": the exposure as a value with its unit (`EV100`) and automation level (`AUTO`, `MAN`,
  `INHIBITED`, with Design note 11's meanings: who inhibited it and why, `INHIBIT` and `ENABLE` as
  the commands, a system inhibit resuming by itself and an operator's not); `TERRAIN: STREAMING` and
  `TERRAIN: DETAIL LIMITED` as steady annunciations in `--text` on the label block's plate, neither
  a data state nor an alert, with no status colour and never the word "degraded". Grep: `EV100`,
  `TERRAIN: DETAIL LIMITED`.
- **R02.T2.d Item 8: motion for a display that never stops.** In "Motion and sound": a view redraws
  continuously; preset changes and slews are cuts; single-player's optional 0.4 s eased move, off by
  default; the main screen always cuts; under `prefers-reduced-motion` every non-physical motion
  stops (eased moves, idle drift, camera smoothing and automatic camera motion) while camera flight
  itself is never suppressed; readouts stay at 4 Hz. Grep: `eased move`.
- **R02.T2.e Item 9: the main screen.** In "Layout": the main screen has no header strip, work area,
  navigation bar or input, and always shows the status line, the mode banner, the alert annunciator
  and the view's label block with who commands the camera; `NO CARRIER` and the stale `S` on loss of
  link. Its text is sized in minutes of arc at the furthest stated viewing distance, h = 2 d tan(θ
  ÷ 2), from MIL-STD-1472H (15 September 2020, the current revision; researched 2026-09-29): at
  least 20′ (5.8 mrad), because the main screen's text is colour-coded (§5.17.25.14, "when accurate
  color perception is required"), and never below the general 15′ (§5.17.18.2, which sets 10′ as the
  minimum and 15′ as the preferred value); the alert annunciator's warning and caution text 30′ to
  60′, the larger in adverse conditions (§5.7.3.6). The viewing distance is between 3 and 6 screen
  diagonals where it can be chosen (§5.2.2.12.3), and the guide's dark theme stands against
  §5.2.2.12.8.1's preference for dark-on-light, for a night-adapted bridge, which the draft says.
  Grep: `main screen`, `status line`, `MIL-STD-1472H`.
- **R02.T2.f The nomenclature list.** `VIEW` (display), `WIREFRAME` and `PHOTOREALISTIC` (styles),
  `SEAT`, `CHASE`, `FREE` (camera presets), `FOV` (abbreviation), `EV100` (unit),
  `NO IMAGE TO METER`, `POSITIONS AS SEEN FROM SHIP`, `ROTATION NOT YET MODELLED`,
  `EASED CAMERA MOVES` (setting), `TEST HULL`, `TERRAIN: STREAMING`, `TERRAIN: DETAIL LIMITED`,
  `DECORATION ON`, and the star-source reading of Design note 16. R01's drafts (its T5.c: the
  `GRAPHICS` fault and mode wording) are absorbed into the same pass and re-checked against this
  list. Grep: `SEAT`, `EV100`, `GRAPHICS DEVICE LOST`.
- Files: `docs/frontend/ux-guidelines.md`.

### R02.T3 Body-fixed position types in `coords`

Add `coords/body_fixed.rs` with `BodyFixedPosition`, `BodyFixedVector`, `BodyVector` and
`BodyFixedRotation`, re-exported from `coords`. `from_rows` refuses a matrix that is not orthonormal
to 10⁻¹² or has determinant below zero (`BuildRotationError`). Products are written out in a fixed
order, `r[0][0]*x + r[0][1]*y + r[0][2]*z`, with no `mul_add`. The module documentation says why it
is a position type and not a fourth frame: the camera and every craft stay in the non-rotating body
frame, which has no rotational fictitious forces (it is free-falling, not inertial, so the other
bodies' tidal term remains; Design note 6). `compile_fail` doctests show that a `BodyFixedPosition`
cannot be added to a `BodyPosition` nor converted without a rotation.

- Files: `crates/hyperion-sim/src/coords/body_fixed.rs`, `coords/mod.rs`,
  `crates/hyperion-sim/tests/golden/coords/body_fixed.golden`, a test in
  `crates/hyperion-sim/tests/` for the golden.
- Tests: identity round trip bit for bit; a 90° rotation about z maps x̂ to ŷ exactly; a rotation
  built from Earth's pole and a prime-meridian angle of 2.2 × 10⁵ rad (the brainstorm's century)
  reduced by the caller round-trips a point at 6,371 km to 10⁻⁹ m; a non-orthonormal matrix and a
  reflection are refused; the golden pins ten conversions.
- Acceptance: `cargo test -p hyperion-sim coords::body_fixed` and the golden test pass; `just ci`
  green. `GENERATOR_VERSION` unchanged.

### R02.T4 The body-frame selection rule

**R02.T4.a The rule.** `planetary/frame.rs`:
`BodyFrameCandidate::new(id, parent, distance, hill_radius)` (refusing non-finite or non-positive
values, as `FrameCandidate::new` does), `BODY_FRAME_ENTRY` = 0.9 and `select_body_frame` per Design
note 6: of the candidates whose ratio is within entry, or within 1 for the current frame and its
ancestors, the deepest in the parent chain wins; at equal depth, the smallest ratio, a rival taking
over only at (1 − `FRAME_HYSTERESIS`) of the current one; the lower ID on an exact tie; after
leaving a sphere the rule re-runs with no current frame. Distances are geometric and present. The
answer does not depend on the candidates' order. The doc comment cites the brainstorm, the galaxy
rule it follows, and says the boundary is a precision device and a body frame free-falling, not
inertial.

- Tests: Earth–Moon numbers (Earth's Hill radius 1.5 × 10⁹ m, the Moon's about 5.8 × 10⁷ m, both
  recomputed by `hill_radius`): a camera arriving from Earth's frame takes the Moon's at 0.9 of the
  Moon's Hill radius and not at 0.95; once in the Moon's frame it stays at 0.95 and returns to
  Earth's at 1.01; a camera outside every sphere is in the system frame; two co-orbital siblings
  with overlapping spheres hand over only at a tenth's advantage, and by ID only on an exact tie; a
  massive moon, Charon-like (sphere 6.7 × 10⁶ m against its planet's), nests as the Moon does; the
  answer does not depend on the candidates' order (`order::assert_order_independent`).
- Acceptance: `cargo test -p hyperion-sim planetary::frame`.

**R02.T4.b The golden for the TypeScript twin.** `tests/body_frame_golden.rs` writes
`tests/golden/frame/body_frames.golden`: 200 seeded cases (candidate sets with nesting, the current
frame, the answer), one per line in a documented text form.

- Acceptance: the golden test passes natively; `just test-wasm` passes it on wasip1 by hand.

### R02.T5 The absolute V magnitude on the stellar brief (a named ask of galaxy plan 06)

If plan 06 has not added it, add `absolute_v_mag: Option<f32>` to `StellarBriefDto` as an optional
field written only when a brief is asked for, `null` where `absolute_bolometric_magnitude` or
`bolometric_correction_v` has no value (white dwarfs, remnants, below 1,710 K). `brief_dto` computes
it from the brief's log L and T_eff, as plan 06's `absolute_magnitude_v` does from a state. Run
`just gen-protocol`. The field is optional, so `PROTOCOL_VERSION` stays at 2
(`crates/hyperion-protocol/src/lib.rs`).

- Files: `crates/hyperion-protocol/src/stellar.rs`, `crates/hyperion-server/src/convert/stellar.rs`,
  `packages/protocol/src/generated/StellarBriefDto.ts`.
- Tests: the wire form with and without the field; the Sun-like brief gives 4.83 ± 0.01 (the
  photometry doctest's figure); a white dwarf's is `null`; a range request with `include_stellar`
  carries it on every row with a brief.
- Acceptance: `cargo test -p hyperion-protocol stellar` and `cargo test -p hyperion-server convert`
  pass; `just gen-protocol-check` clean.

### R02.T6 Positions, differencing and narrowing (`view/coords/`)

**R02.T6.a Position types and the galactic delta.** `ViewPosition`, `FrameOrigins`, `Rotation3` (a
row-major `f64` triple of rows, built only from an orthonormal matrix), `galacticDeltaM` (cells
subtracted before offsets, `METRES_PER_LIGHT_YEAR` from `@hyperion/protocol`), and the conversions
between the four kinds given `FrameOrigins`.

- Tests: a point 60,000 ly from the centre and 1 m from the camera differences to 1 m within 2 m
  (the galactic frame's resolution); system ↔ body round trips to 10⁻⁶ m at 30 au; body-fixed ↔ body
  through a rotation matches the sim's `body_fixed.golden` to 10⁻⁹ relative.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/coords`.

**R02.T6.b Camera-relative differencing and the origin convention.** `relativeToCamera`, `narrow`,
`OriginRelative` and `originMinusCamera`, per Design notes 1 and 2, with the `craft` camera frame of
Design note 22 differenced as (p − craft) − offset.

- Tests (the brainstorm's "Precision, as arithmetic"): a camera at 6,371 km from a body's centre
  keeps two features 1 cm apart distinct after narrowing, and the naive path (narrow both, then
  subtract) merges them, since `f32` spacing there is 0.5 m; at 1 km from the camera the narrowed
  spacing is 0.06 mm and at 1,000 km 6 cm (IEEE `f32`, `Math.fround`); an `OriginRelative` patch 1
  km across at a planetary radius reproduces its vertices to 1 mm; with the ship 1 ly from the
  barycentre in the system frame, the own hull at 1 m from a seat camera in the `craft` frame stays
  within 0.1 px, while the same pose held in the system frame errs by metres.
- Acceptance: the vitest run above;
  `grep -rn "Math.fround\|new Float32Array" apps/hyperion/src/renderer/src/view` finds the narrowing
  only in `view/coords/narrow.ts` and in GPU buffer packing under `view/wireframe/`, whose inputs
  are already narrowed.

### R02.T7 Projection, view rotation and the depth policy

**R02.T7.a Camera maths.** `Quaternion` (normalised on construction), `viewRotation`,
`perspectiveReversedInfinite`, `project`, `pixelSolidAngle`, `NEAR_PLANE_M` = 0.1, `FOV_STEPS_DEG` =
10, 20, 30, 45, 60, 90, 120 with 60 the default.

- Tests: the matrix of Design note 4 against hand values at 60° and 16:9; a point at the near plane
  projects to depth 1, one at 1 au to 6.7 × 10⁻¹³ within 10⁻⁶ relative; the rotation-only path keeps
  a point 1 au away within 0.1 px while a naive translated matrix errs by more than 1 px (Design
  note 3); the centre pixel of a 1920 px, 60° view subtends 3.62 × 10⁻⁷ sr within 0.5% and a 16:9
  corner pixel, at θ = 33.5°, 0.580 of that (cos³θ); the view basis is right-handed.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/camera`.

**R02.T7.b Depth constants, ordering and transparent layers.** `view/depth/`: the constants,
`separable`, `occluderRadius` (Design note 5), and `transparentLayerOrder`: opaque first; then per
body, back to front by the distance of the body's centre; within a body, the shells below the camera
by ascending altitude, then the planes (rings), then the shells above by descending altitude; every
transparent layer tests depth and writes none (brainstorm, "Depth"). R08 and R11 consume it.

- Tests: surfaces 1 m apart are separable to 10⁶ m and 1 km apart to 10⁹ m (Reed 2015 through the
  brainstorm); `occluderRadius` keeps a graticule point on the front hemisphere at least 4 × 10⁻⁶ d
  in front of the occluder at 400 km, 10⁸ m and 1 au; a camera between two cloud shells orders them
  as stated; the order is a function of the layers and the camera alone.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/depth`.

### R02.T8 The camera's frame selection and rebasing

**R02.T8.a The twin.** `selectCameraFrame`, the TypeScript twin of `select_body_frame`, and the
tidal-radius clamp of Design note 7.

- Tests: every line of `body_frames.golden` (read with `?raw`, as `lib/orbit.test.ts` reads its
  golden) gives the sim's answer.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/camera/frames.test.ts`.

**R02.T8.b Rebasing as one event.** `rebase(pose, nextFrame, origins)` re-expresses the camera's
position in the new frame in `f64` and returns a `FrameChange` that invalidates every cached
camera-relative quantity at once; nothing is shifted incrementally in `f32`.

- Tests (the brainstorm's frame-change test): along a scripted path from the system frame into a
  planet's Hill sphere and out, with hysteresis, the `f64` vector from the camera to a fixed
  landmark on the body is continuous to 1 mm across each change and its projection to 1 px; the same
  for a free camera whose frame changes apart from the own ship's; a grounded craft's `body_fixed`
  position is constant while its body-frame position moves at ω × r, with ω from the kept scene's
  hand-set rotation.
- Acceptance: the vitest run above.

### R02.T9 Presets, cuts and the free camera

**R02.T9.a Camera state.** `CameraState { preset, target, style, role, pose }` with `style` a
`RenderStyle` of `"wireframe"` only and `role` a `ViewRole`, `"eye"` for the single-player cockpit
view and `"camera"` otherwise; `seat` (fixed to the own ship's hull at its eye point, looking
forward, aft or at the target), `chase` (an offset in the own ship's frame), both in the `craft`
camera frame (Design note 22), and `free` (detached, a pose in the scene's frames, or in the `craft`
frame while it orbits a target craft); `cutTo(state, preset | target)`; the `EASED CAMERA MOVES`
setting (0.4 s, off by default, never under reduced motion). With no own ship only `free` is
offered.

- Tests: each preset's pose from a scene; a cut changes the pose in one step; an eased move
  lasts 0.4 s and is a cut under reduced motion; `seat` and `chase` are refused with no own ship.

**R02.T9.b Free flight.** `stepFreeCamera(state, input, dtS, reducedMotion)`, integrated at display
rate: translation rate commanded in metres a second on a logarithmic scale (so one control spans a
metre a second to a tenth of the system across in seconds), rotation in degrees a second, an
acceleration ramp and damping that are removed under reduced motion (Design note 18); clamped to the
system's tidal radius; re-selecting the camera's frame each step.

- Tests: pose after a fixed input sequence at 60 Hz and 144 Hz agrees within 10⁻⁶ relative; with
  reduced motion, motion starts and stops on the frame input starts and stops; the clamp holds.

**R02.T9.c Keyboard bindings.** A pure map from keys to camera inputs, active only while the view's
canvas has focus and ignoring modified events and text inputs (plan 05's design note D3): translate,
rotate, roll, rate up and down, preset keys, next and previous target, field of view steps.

- Files (all of T9): `view/camera/state.ts`, `view/camera/freeCamera.ts`, `view/camera/keys.ts` and
  their tests.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/camera`.

### R02.T10 Photometry, exposure and the tone curve

**R02.T10.a Magnitude to pixel.** `V0_ILLUMINANCE_LX`, `apparentV`, `illuminanceLx`,
`psfPixelWeights`, `pixelLuminance` (Design note 10). Re-check the 2.54 µlx zero point against the
brainstorm's citations (Cox 2000 §15; Crumey 2014; the Sun's V = −26.76 of Willmer 2018 giving
about 1.28 × 10⁵ lx) and cite them.

- Tests: V = 0 gives 2.54 × 10⁻⁶ lx; V = −26.76 gives 1.28 × 10⁵ lx within 1%; a mag 6.5 star wholly
  in the centre pixel of a 1080p 60° view is 1.76 × 10⁻² cd/m² within 1%; PSF weights sum to 1
  within 10⁻⁹ at 100 sub-pixel positions, and the brightest pixel's weight varies by less than a
  factor of 2.5 across them; and the stronger test, that the display value summed after the tone
  curve stays within ±15% across those 100 positions for stars from V 6.5 to −1.46 at EV100 −1 to
  +2.

**R02.T10.b Exposure.** `ev100FromTriple`, `ev100FromAverageLuminance`, `exposureScale`, and
`ExposureControl` with its transitions (Design note 11): `MAN` from any state by the operator,
`AUTO` only with a metering source, `INHIBITED` by the operator's `INHIBIT` or on the loss of the
source, each carrying who set it and why, and `ENABLE` clearing it; `DEFAULT_MAN_EV100` = −1. Cite K
= 12.5, q = 0.65, the 78 and the 9.6 × average white point to Filament's "Physically based camera"
sections and bruop.github.io/exposure.

- Tests: f/1, 1 s, ISO 100 is EV100 0; f/16, 1/100 s, ISO 100 is about 14.6; a metered average
  of 12.5 cd/m² is EV100 6.64; the white point is 9.6 × the average within 1%; the state machine's
  transitions, including `AUTO` refused with `NO IMAGE TO METER`; a system inhibit resumes `AUTO`
  when the source returns and an operator inhibit does not; the reason is shown with the level.

**R02.T10.c Tone curve and pre-exposure.** `toneCurve` and `toneCurve.wgsl`'s `agx`, the full AgX
colour function of Design note 12 with its constants cited to three.js's
`tonemapping_pars_fragment.glsl.js` (after Filament pull request 7236 and Blender),
`HDR_COLOUR_FORMAT` with its alpha left to R07, `preExpose` with its clamp to 65,504 (the
`rgba16float` maximum).

- Tests: grey inputs are monotone, 0 maps to 0 (the sigmoid's −0.00232 clamped) and the output
  saturates at or below 1; the sigmoid alone maps the log-encoded 0.18 to 0.497 and
  display-linear 0.215, and the full function's value for 0.18 grey is computed once the matrices
  are in and pinned with its derivation; the inset rows sum to 1.106, 0.933 and 0.961 as cited; the
  WGSL function and its twin agree at 64 points when the shader is run on SwiftShader in R02.T14.c;
  a Sun's disc at 2 × 10⁹ cd/m² under a dark-sky exposure is clamped to 65,504 before storage.
- Files (all of T10): `view/photometry/*.ts` and tests, `view/shaders/toneCurve.wgsl`.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/photometry`.

### R02.T11 The view scene, hull outlines and the kept scenes

**R02.T11.a The model.** `ViewScene` (bodies with radius, Hill radius, parent, rotation or `null`;
rings; orbits; craft with `HullOutline` and pose; stars as direction, distance and absolute V; the
own ship; the time, time rate and provenance `kept` or `server`), `HullOutline`, `TEST_HULL`, and
`test/viewFixtures.ts`.

**R02.T11.b The precision scene.** `precisionScene()`: the test hull's plate 1 m from the seat's eye
point, a moon at 10⁸ m and a planet at 1 au in one frame, with a scripted camera path that
translates and rotates. Tests: along the path every mark's projection moves smoothly, with no step
larger than 0.1 px beyond the path's own motion between consecutive frames; the plate, the moon and
the planet are pairwise separable in depth by Design note 5; the plate's hidden edges are hidden by
its faces.

**R02.T11.c The frame-change scene.** `frameChangeScene()`: a planet with a hand-set rotation, a
moon inside its Hill sphere, a landmark on the planet and a grounded test craft, with a camera path
that crosses the moon's and the planet's Hill spheres both ways. Its tests are R02.T8.b's, run on
this scene.

- Files (all of T11): `view/scene/*.ts`, `view/scenes/precision.ts`, `view/scenes/frameChange.ts`
  and tests, `test/viewFixtures.ts`.
- Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/scene src/renderer/src/view/scenes`.

### R02.T12 The wireframe's geometry

**R02.T12.a Bodies.** `bodyRegime`, `graticule` (Design note 13: adaptive subdivision to a 0.25 px
sagitta in `f64`, analytic hemisphere visibility, the limb), `ringEllipse`.

- Tests: a body 400 km below the camera has a limb within 0.25 px of the true horizon circle; no
  graticule point on the far hemisphere is emitted; the regime changes at 3 and 8 px; a rotated
  body's prime meridian follows its rotation.

**R02.T12.b Orbits, hulls and culling.** `orbitPath` from `lib/orbit.ts`'s propagation, sampled by
screen-space error rather than a fixed count; `hullEdges`; the frustum and limb culling predicates
of Design note 8, including a mark larger than the frustum and a camera inside a body's Hill sphere.

**R02.T12.c Symbology.** The bracket reticle for the selection, the destination reticle in
`--target`, target brackets with range and closure rate, the own ship's flight path marker for its
velocity against the frame's reference where an own ship exists, and a mark per body below 3 px from
`spatial/symbols.ts`.

- Files (all of T12): `view/wireframe/{bodies,orbits,hulls,cull,symbology}.ts` and tests.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/wireframe`.

### R02.T13 The draw list

`buildWireframeDrawList(scene, camera, viewport, tokens)`: line batches (width, casing width, colour
token, dash pattern for predicted paths only), depth-only occluders (spheres by `occluderRadius`,
hull faces with the occluder pass's `depthBiasAway`, two-sided), star sprites (pre-exposed linear
colour and sub-pixel position), in the order opaque occluders, lines, sprites, all `f32` and
camera-relative. It reads colours through plan 05's `readTokens` names, never literals.

- Tests: the list is a function of the scene, camera, viewport and tokens alone; orbits are
  `--text-muted` solid, the selected orbit `--text` 2 px; a mark over a star sprite is cased; no
  batch holds a coordinate larger than the scene's largest camera-relative distance narrowed; a
  predicted path is the only dashed batch; the low setting caps sprites at 2,000 by flux.
- Files: `view/wireframe/drawList.ts` and its test.
- Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/wireframe/drawList.test.ts`.

### R02.T14 WGSL shaders and their submission through R01's adapter

**R02.T14.a Lines.** `lines.wgsl` per Design note 9, registered through R01's WGSL pipeline API,
with the depth state of Design note 4 (`greater-equal`, no write).

**R02.T14.b Occluders and sprites.** `occluder.wgsl` (depth-only, colour writes off, two-sided; the
hull variant with `depthBiasAway { constant: 128, slopeScale: 2 }` of Design note 5, never on a line
pass) and `starSprite.wgsl` (PSF weights and `agx`, additive in linear light through the sRGB view
R01 provides, depth-tested, no write).

**R02.T14.c Smoke on SwiftShader.** In R01's headless harness, render the precision scene's first
frame and read it back: every texel finite; the depth buffer holds 0 where nothing was drawn; the
casing texels around a line are `--surface-0`; the tone curve's WGSL output at 64 luminances equals
its twin within 10⁻⁵; a texel at the near plane reads depth 1 and +y is up on screen (no half-Z
conversion or Y flip was added over the frozen projection); a face with its own edge and a second
edge 10⁻⁵ of the distance behind it, at 1 m and at 10⁸ m, shows the first edge and hides the second.
Properties only, never a stored image.

- Files: `view/shaders/*.wgsl`, `view/wireframe/submit.ts` (the only file that calls R01's adapter),
  the smoke test in R01's harness directory.
- Acceptance: the smoke recipe R01 names passes, and
  `grep -rn "@babylonjs" apps/hyperion/src/renderer/src/view` finds nothing: no engine type crosses
  into `view/`.

### R02.T15 The `VIEW` display

**R02.T15.a Shell and wiring.** `DisplayId` gains `"view"` (`View`, `F4`); `ViewDisplay` loads the
engine lazily through R01, shows R01's fault state or `PENDING` in its place, and renders a kept
scene chosen from a `SCENE` selector (`PRECISION TEST`, `FRAME CHANGE TEST`) until R02.T17 adds the
server's, under the guide's training banner.

**R02.T15.b The canvas and its list.** `ViewCanvas` (focusable, named per Design note 17),
`ViewMarkList` (windowed `listbox` of bodies and craft with keyboard selection that moves the
bracket reticle, range readouts), and selection from the canvas by pointer through a pure pick over
the draw list's anchors, as plan 05's `pick` does.

**R02.T15.c The label block and the exposure instrument.** `ViewLabelBlock` with Design note 16's
lines at 4 Hz through `useThrottledValue`; the exposure panel showing `EV100 −1.0 MAN` with its
triple, `AUTO` unavailable with its reason, `INHIBIT` and `ENABLE` as its commands and the
`INHIBITED` reading with who set it; camera controls for preset, target and field of view, each a
button reachable by keyboard with its key shown.

**R02.T15.d Motion and settings.** The redraw loop runs only while visible; `EASED CAMERA MOVES` as
a setting; `prefers-reduced-motion` honoured per Design note 18, tested through `stubMatchMedia`.

- Files: `displays/view/*.tsx`, `lib/displays.ts`, `App.tsx`, and tests.
- Tests: the display switches on `F4`; the list and the canvas select the same mark; the label block
  shows each line of Design note 16 in its condition; keyboard-only operation reaches every camera
  control; reduced motion turns eased moves into cuts; no text is drawn into the canvas.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/displays/view`; the
  console-ux skill's lint, contrast and glyph scripts pass on the new components.

### R02.T16 The interim star field

**R02.T16.a The queries.** `useInterimStars`: Design note 19's four `systems_in_range` requests (`e`
620, `d` 360, `c` 210 and `a` 60 ly) with `include_stellar`, merged by ID, re-sent on a change of
system; a request answered `over_limit` at its floor retried once at r × (0.9 × 20,000 ÷ Σ
expected)^⅓; rows without `absolute_v_mag` dropped and counted. Re-measure the four queries' census
counts and server times on the Milky Way fixture near the Sun, the times on a quiet machine (Design
note 19's are provisional, taken under shared load), the brief-building time of the remnant-heavy
`e` query included (`limits.rs` states about a second per 1,024 dead rows), and count the rows at V
≤ 6.5; record the figures in this plan's Verification.

**R02.T16.b Sprites and label.** Stars into the scene as direction and apparent V, drawn by the
sprite shader through the exposure; the label's star-source line; the count line
`STARS n DRAWN · m WITHOUT V · RADII e/d/c/a` with the radii used.

- Files: `view/stars/interim.ts`, `displays/view/useInterimStars.ts`, tests with `FakeWebSocket`.
- Tests: the four requests' shapes; merging keeps one row per ID; a `FakeWebSocket` census answer of
  `over_limit` triggers one retry at the shrunk radius and no second; a row with `null` magnitude is
  not drawn and is counted; the label reads as stated, with the radii used.
- Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/stars src/renderer/src/displays/view`;
  the measured counts are in Verification.

### R02.T17 Generated scenes from R03's subscription

Once R03's scene topic lands: `view/scene/fromServer.ts` turns R03's client scene into a `ViewScene`
(bodies from their orbital elements through `lib/orbit.ts` at R03's render time, radii, Hill radii,
rings; `sceneAt`'s `hillRadiusM`; craft when they exist; the ship stand-in as the own ship), the
ship's local body drawn from `sceneAt`'s `geometricM` and every other body, a free camera's own
local body included, from its `apparentM`, the `SCENE` selector gains the server's scene for the
open system, and each local view's pose is handed to `CameraReporter`. Graticules stay still under
`ROTATION NOT YET MODELLED` until P14.T14.c.

- Tests: a scene fixture from R03's builders yields geometric body positions equal to the `SYSTEM`
  display's orbit map at the same time to 10⁻⁹ relative; the ship's local body is drawn at its
  `geometricM` and a free camera's own local body, when it is another, at its `apparentM`; the
  camera report carries the pose on each frame change and at R03's stated rate.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/scene`.

### R02.T18 Recorded runs, by hand

On the UHD 620 (and on a discrete GPU when one is available), with the settings recorded:

- **The precision scene.** The camera translates and rotates along its path: no depth fighting
  between plate, moon and planet, no visible jitter at 1 m, 10⁸ m and 1 au. Recorded as pass or fail
  with the machine, driver and build.
- **The frame-change scene.** The landmark's projected position is watched across each boundary
  crossing: no visible jump.
- **The exposure.** The default `MAN` EV100 against the interim star field near the Sun: faint stars
  visible, Sirius-class stars unclipped; if the default is wrong, change it and record why.
- **Performance.** GPU time per frame of the precision scene and of a generated system with its star
  field, at 1080p, high and low settings, and frame intervals at the 50th, 95th and 99th
  percentiles, on a quiet machine with no other test run sharing it, against Design note 21's 4 ms.
- Files: this plan's Verification, "Recorded runs" table. R12 consolidates.
- Acceptance: the table has one row per run, and every failure is filed as a finding against the
  task that owns it.

## Verification

- `just ci` green after every task; the TypeScript and Rust tests above are the automatic proof of
  the foundations: precision as arithmetic, projection, rotation-only view, depth ordering,
  frame-change continuity to 1 mm and 1 px, body-fixed positions constant under rotation, photometry
  and exposure against hand values with citations.
- The TypeScript twin of the body-frame rule agrees with every line of the sim's golden, and the
  golden passes on wasip1 by hand (`just test-wasm`).
- The SwiftShader smoke test (R02.T14.c) completes a frame of each kept scene with only finite
  texels.
- The recorded runs of R02.T18, below. No golden image is stored anywhere.
- The owner has answered each of R02.T2's drafts; until then they stand as drafts and the client is
  built to them.

### Recorded runs

| Date | Machine and driver | Run | Setting | Result |
| ---- | ------------------ | --- | ------- | ------ |

## Generator version

This plan changes no generated output. `BodyFixedPosition` and its siblings are types; the
body-frame rule reads plan 14's Hill radius and generates nothing; `absolute_v_mag` is computed from
the brief's existing figures on the server and carried on the wire. `GENERATOR_VERSION` is not
bumped. It reserves no stream or tag. It adds the golden files `coords/body_fixed.golden` and
`frame/body_frames.golden`, which move only if the conversion or the rule changes.

## Risks and open points

- **Figures settled by research that the recorded run must still confirm** (researched 2026-09-29):
  the default `MAN` EV100 of −1 depends on the display and the room; the PSF's perceptual adequacy;
  the full AgX function's value for grey, to be pinned once the matrices are in; and the depth
  bias's unit, which backends may implement to within a factor of 2, which the bias's doubled margin
  and R02.T14.c's smoke test cover. The satellite stability fractions behind Design note 6's nesting
  argument are recalled and not load-bearing.
- **R01's adapter must pass our projection through, with no half-Z conversion, no Y flip and a
  right-handed scene or two-sided pipelines, and must not cull.** If Babylon's frustum or its
  world-matrix path cannot be kept out of the way behind the adapter, R01 must say so before
  R02.T14; the fallback is raw WebGPU pipelines for the wireframe's three shaders, which the adapter
  already gives access to through the shared device.
- **No real ship exists.** Seat and chase run against a kept scene's craft or R03's ship stand-in,
  whose hull is `TEST_HULL`, until the single-player sessions exist.
- **No hull definitions exist.** `TEST_HULL` stands in and says so; the plan that builds craft owns
  the definitions and is asked to emit `HullOutline`.
- **Rotation is not modelled** until P14.T14.c, and plan 14 is asked to return this plan's
  `BodyFixedRotation` so that there is one rotation type.
- **The interim sky is poor.** Measured: at the radii that fit one census the bright giants and B
  stars beyond 400–700 ly are missing. Volume-limited, without extinction, companions or retarded
  time; it is labelled, and R06 replaces it.
- **Per-sprite tone mapping differs from a full-screen pass** where sprites overlap or the
  background is not black (the galactic band); with R07 including the same `agx`, an isolated star
  on black is identical in both styles, and R07 draws its stars before its full-screen pass.
- **Asked by R07, not yet a task here.** An oblate graticule for the giants R07 draws oblate from
  plan 14's flattening (R07 Design note 19, whose non-goals name it R02's). Until it exists a
  graticule is a sphere of the equatorial radius. The roadmap's asks table carries it.

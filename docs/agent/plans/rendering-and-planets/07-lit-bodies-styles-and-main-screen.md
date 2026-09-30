# Plan R07: Lit bodies, the photorealistic style, several views and the main screen

- **Milestone:** Rendering milestone RM3 (R06–R07).
- **Depends on:**
  [R02 Real-scale foundations and the wireframe `VIEW`](02-real-scale-view-and-wireframe.md),
  [R03 The scene subscription and bulk transport](03-scene-subscription-and-transport.md),
  [R06 The sky](06-the-sky.md); through them
  [R01 Graphics platform and the engine adapter](01-graphics-platform-and-engine.md). It reads
  [R05 Terrain geometry and the descent spike](05-terrain-geometry-and-descent-spike.md) for one
  thing, the quadtree drawn as a smooth figure (Design note 3). Galaxy plan 14
  ([Planetary systems](../galaxy-generation/14-planetary-systems.md)) for radii, rotation (P14.T14),
  and a photometric section and a flattening that it is asked to add (Design notes 5 and 19). Phase
  C also depends on **the single-player sessions, ship state and closed-loop commands, for which no
  plan exists yet** (Design note 21). Phase C does not start until that plan is written and built.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)): step 5 of "Suggested order of
  attack"; the three regimes of "The scales the view spans"; the shading sentences of "Luminance in
  physical units, and an exposure model"; "Two styles of one renderer" for the photorealistic style
  and the switch; "Two deployments, one scene" for the main screen; the second half of "The free
  camera" (main-screen control, server integration, presentation track); "Several views in one
  client" except the per-view canvas context itself, which is R01's; "Exposure and tone mapping";
  the exposure-histogram, bloom and tone-mapping rows of "Performance budget" and its per-view
  paragraphs; the eclipse and terminator sentences of "The local star as a disc"; items 2, 3, 4, 6
  and 9 of "What the guide must gain" as this plan builds to R02's drafts of them; "The contrast
  problem, which is the real one"; the main-screen sentence of "Accessibility, which a canvas
  threatens"; the "Several views and stills" entry of "Testing", less stills; the Decisions entries
  "Light", "Two styles", "The main screen" and "Several views and stills".

## Goal

When this plan is done, any view can be switched between the wireframe and a photorealistic style
without rebuilding its scene. In the photorealistic style every body is lit at real scale by the
stars of its system in absolute photometric units: a smooth figure, oblate where plan 14 says so,
shaded by a lunar-Lambert disc law times a phase function fitted to measured planetary phase
curves, which reproduces the body's visual geometric albedo per channel and phase integral, with a
terminator in closed form from the star's angular size, eclipses by other bodies through
limb-darkened annuli, and planetshine from the brightest lit neighbours. A body is a point below
3 px, an analytic disc on one quad above, and a smooth quadtree mesh where depth against other
geometry requires it, with its flux continuous across every switch. The image is metered as the
arithmetic mean luminance of a GPU histogram with the star's disc excluded and its veil included,
exposed under `AUTO`, `MAN` or `INHIBITED`, given veiling glare for the light the display cannot
show, and tone-mapped by AgX in one full-screen pass, with a low setting built beside the high one.
Symbology over the image is cased. The single-player `VIEW` display holds a full-window
photorealistic view and up to two wireframe instrument views as canvases of one document on one
device, each with its own budget, and only one photorealistic view on the low setting. Once the
sessions exist (Phase C), a bridge has a main screen: a client role, a camera that is ship state
set by granted stations through closed-loop commands and flown from one station's input by the
server, within 100 ms from input to photon, carried on the scene topic so that a helm can slave its
wireframe to it, and a chrome of status line, mode banner, alert annunciator and label block sized
for the room.

## Scope and non-goals

In scope:

- The regime selector for lit bodies and its painter order (Design notes 1–3).
- Illuminance at a body from each star, per display channel; the closed-form horizon term and the
  annulus eclipse term; planetshine (Design notes 4, 6, 7).
- The body BRDF and its disc-integrated phase function, as a TypeScript reference and a WGSL
  function that R08, R10 and R11 call (Design note 5).
- Oblate discs and points from plan 14's flattening (Design note 19).
- A photometric section on the wire, asked of galaxy plan 14 and carried by R03's scene.
- The photorealistic style as a variant of R02's `RenderStyle`, its pass list and the style switch.
- The exposure histogram, the metering controller, bloom as veiling glare with analytic glare
  sources, the AgX pass with upscale, encoding and dither.
- The casing of symbology over the image and the `--surface-0` plates of DOM text over it.
- The per-view budget policy and the dynamic-resolution controller; instrument views inside
  `VIEW`; exposure shared with them; the second-monitor check of a child window.
- The low setting of each of the above, and its recorded benchmarks on both GPUs.
- Phase C, gated: the client role in `Hello`, the main-screen camera as ship state, its commands and
  grants, the server's free-camera integrator and presentation track, the scene's camera field, the
  station control panel with the flyer's local view, the main-screen client and its chrome, and the
  latency measurement.

Non-goals:

- The per-view canvas context, the engine adapter, the WGSL-only guard, kernel selection, the
  smoke harness and the child-window prototype itself (R01, R01.T13). This plan submits through
  them.
- The camera model, reversed-Z, the HDR target and pre-exposure, the exposure triple and its three
  automation levels, the AgX function `toneCurve` and its WGSL twin `agx`, the wireframe style,
  `VIEW`'s label block and DOM list, and the nine guide drafts (R02). This plan adds to them.
- The scene subscription, retarded and apparent positions, extrapolation and camera reports (R03).
- The sky, the star cubemap and sprites, the host stars' limb-darkened discs and the spectral
  colour table (R06). This plan lights bodies with R06's `HostDisc`s and meters around them.
- Atmospheres, and the baked disc reflectance that replaces this plan's albedo-only shading of an
  atmosphere-bearing body (R08). Terrain, and the hand-over from disc to terrain (R10). Rings,
  their far annulus, clouds, oceans, glint, decoration and still images (R11). Until R11, rings
  are R02's wireframe ellipses drawn as cased symbology over the image (Design note 17).
- Craft as lit models. No hull art exists; craft stay R02's hull outlines, cased over the image.
- Gas giants' cloud bands. No plan generates a band structure yet (Risks).
- Thermal emission of magma oceans in the visible, which is not reflection and which no plan owns
  yet (Risks).
- An oblate graticule and oblate collision (R02, R05 and the flight model).
- The consolidated performance runs and the settings ladder's adjustment (R12).
- Sessions, ship state, stations, control arbitration and alerts themselves (the unwritten sessions
  plan). Phase C consumes them.
- HDR display output. The canvas is standard dynamic range; an extended-range canvas is an open
  point.

## Provides

TypeScript paths are under `apps/hyperion/src/renderer/src/view/` unless they start otherwise.
Signatures are sketches, named precisely enough to be grepped.

### Lighting (`lighting/`)

```ts
export type Rgb = readonly [number, number, number]; // R08's `Rgb`, one type
/** Illuminance at a point face-on to a host star, per display channel, lux (Design note 4). */
export function starIlluminance(disc: HostDisc, distanceM: number): Rgb;
/** Irradiance factor from a uniform sphere of H = d ÷ R at angle φ to the normal (Howell). */
export function sphereIrradianceFactor(h: number, phiRad: number): number;
export const DISC_ANNULI_HIGH = 4; // limb-darkened annuli, edges uniform in μ
export const DISC_ANNULI_LOW = 2;
export function annulusEdges(law: PowerTwo, k: number): Float64Array;
export function circleOverlapArea(r: number, k: number, z: number): number; // exact
export function eclipseVisible(
  disc: HostDisc,
  from: Vec3,
  occluders: ReadonlyArray<Occluder>,
  k: number,
): number; // fraction of flux visible
export interface Occluder {
  readonly centreM: Vec3;
  readonly radiusM: number;
}
export function occludersFor(
  body: SceneFrameBody,
  discs: ReadonlyArray<HostDisc>,
  bodies: ReadonlyArray<SceneFrameBody>,
): Occluder[]; // shadow-cone pre-test, f64
export interface SecondarySource {
  readonly direction: Vec3;
  readonly angularRadiusRad: number;
  readonly illuminance: Rgb;
}
export function planetshineSources(
  body: SceneFrameBody,
  lit: ReadonlyArray<SceneFrameBody>,
  max: number,
): SecondarySource[]; // Design note 7
```

`lighting/oracle.ts`: the `f64` oracles (dense annulus sums, brute-force sphere irradiance, the
disc integral of a law).

### Appearance and BRDF (`appearance/`)

```ts
export interface PhotometricLaw {
  // Design note 5
  readonly a: Rgb; // albedo scale per channel
  readonly lommelSeeligerShare: number; // L, 0 (Lambert) to 1
  readonly phaseTable: PhaseTableId; // f(α), tabulated, clamped at 4
  readonly phaseExponent: number; // s in Φ_t(α)^s
}
export interface BodyPhotometry {
  readonly geometricAlbedo: Rgb;
  readonly phaseIntegral: Rgb;
  readonly law: PhotometricLaw;
  readonly provenance: "modelled" | "provisional";
}
export const PROVISIONAL_PHOTOMETRY: BodyPhotometry; // until plan 14's section, labelled
export const PHASE_F_CLAMP = 4;
export function brdf(law: PhotometricLaw, mu0: number, mu: number, phaseRad: number): Rgb; // I/F
export function discIntegratedPhase(law: PhotometricLaw, phaseRad: number): Rgb; // Φ(α), Φ(0) = 1
export function lawFor(p: Rgb, q: number, template: PhaseTemplate): PhotometricLaw;
// Mallama and Hilton 2018
export const PHASE_TEMPLATES: Readonly<Record<PhaseTemplateId, PhaseTemplate>>;
export interface BodyFigure {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
  readonly pole: Vec3 | null;
} // Design note 19
export interface BodyAppearance {
  // what R08 and R10 read per body
  readonly body: BodyIdHex;
  readonly figure: BodyFigure;
  readonly photometry: BodyPhotometry;
  readonly regime: LitRegime;
  readonly labels: ReadonlyArray<AppearanceLabel>;
}
```

`shaders/litBody.wgsl` exports `body_brdf(law, mu0, mu, alpha) -> vec3f`,
`sphere_irradiance(h, phi) -> f32` and `eclipse_visible(...) -> f32`, called by R08's surface
lighting, R10's `terrainLit.wgsl` with per-class parameters, and R11's ring and ocean passes; all
registered in R01's `WGSL_CATALOGUE`.

### Regimes (`bodies/`)

```ts
export type LitRegime = "point" | "disc" | "mesh";
export const POINT_BELOW_PX = 3; // brainstorm, "The scales the view spans"
export const GAS_GIANT_FULL_PASS_BOUNDARY_M = 1e9; // R08's full passes inside it
export const REGIME_HYSTERESIS = 0.1;
export function litRegimes(
  bodies: ReadonlyArray<SceneFrameBody>,
  camera: CameraPose,
  viewport: Viewport,
  previous: ReadonlyMap<BodyIdHex, LitRegime>,
): Map<BodyIdHex, LitRegime>;
export function painterOrder(
  bodies: ReadonlyArray<SceneFrameBody>,
  camera: CameraPose,
): BodyIdHex[]; // by tangent power, Design note 2
```

`shaders/bodyDisc.wgsl` (the analytic disc, spheroid by axis scaling), `shaders/bodyPoint.wgsl`
(through R02's `starSprite.wgsl` and `psfPixelWeights`), `bodies/smoothMesh.ts` (R05's quadtree on
the reference spheroid at zero height).

### The photorealistic style and its passes (`photoreal/`, `post/`)

```ts
// R02's `RenderStyle` gains its variant: "wireframe" | "photorealistic"
export function photorealisticPasses(setting: QualitySetting): PassList; // Design note 8
export type MeterMode = "average" | "lit" | "dark"; // Design note 10
export interface Histogram {
  readonly bins: Uint32Array;
  readonly preExposure: number;
}
export const HISTOGRAM_BINS = 256;
export const HISTOGRAM_MIN_LOG2 = -14; // pre-exposed, rgba16float
export const HISTOGRAM_MAX_LOG2 = 16;
export interface PercentileWindow {
  readonly low: number;
  readonly high: number;
} // default [0, 1]
export function meteredLuminance(h: Histogram, window: PercentileWindow, veil: number): number;
export interface AutoExposure {
  step(h: Histogram | undefined, veil: number, dtS: number): ExposureReading;
}
export interface ExposureReading {
  readonly ev100: number;
  readonly control: ExposureControl;
  readonly meter: MeterMode;
  readonly source: ViewId;
} // R02 takes it as `AUTO`
export interface GlareSource {
  readonly direction: Vec3;
  readonly angularRadiusRad: number;
  readonly excessLuminance: Rgb;
} // cd/m² above 65,504
export function glareSpread(role: ViewRole, thetaRad: number): number; // sr⁻¹, Σ = 1
export function frameVeil(
  sources: ReadonlyArray<GlareSource>,
  role: ViewRole,
  camera: CameraPose,
  viewport: Viewport,
): number; // mean cd/m², Design note 12
// NNLS weights, Σ = 1
export function bloomKernel(setting: QualitySetting, role: ViewRole, radPerPx: number): BloomKernel;
```

WGSL: `post/histogram.wgsl` (shared-memory atomics; a `KernelPair` with `readback: "bit-exact"`
and no subgroup twin unless R01's catalogue asks for one), `post/bloomDown.wgsl`,
`post/bloomUp.wgsl`, `post/tonemap.wgsl` (R02's `agx`, the analytic glare of `GlareSource`s, the
upscale, the encoding and the dither in one pass).

### Several views (`budget/`, `displays/view/`)

```ts
export interface ViewBudget {
  readonly renderScale: number;
  readonly rateHz: 60 | 30;
  readonly style: RenderStyle;
  readonly streamPriority: "primary" | "secondary";
}
export const PER_CANVAS_OVERHEAD_MS = 0.3; // provisional, Design note 14
export function viewBudgets(
  views: ReadonlyArray<ViewSpec>,
  setting: QualitySetting,
  instrumentsOpen: boolean,
): Map<ViewId, ViewBudget>; // pure
export function photorealisticAllowed(
  views,
  setting,
  candidate: ViewId,
): { allowed: true } | { allowed: false; reason: "ONE PHOTOREALISTIC VIEW ON LOW SETTING" };
export class ResolutionController {
  update(gpuFrameMs: number | undefined, intervalMs: number): number;
} // renderScale
```

`displays/view/InstrumentView.tsx`, `displays/view/StyleControl.tsx`,
`displays/view/MeterControl.tsx`; R02's `ViewDisplay.tsx` gains the instrument slots.

### Main screen (Phase C)

Protocol (`hyperion_protocol`, mirrored in `@hyperion/protocol`), all additive:

```rust
// envelope.rs
ClientMessage::Hello { client_version: String, #[serde(default)] role: ClientRoleDto }
pub enum ClientRoleDto { #[default] Console, MainScreen }      // "console" | "main_screen"
// main_screen.rs
pub struct MainScreenCameraDto { preset: CameraPresetDto, target: Option<TargetDto>,
    style: RenderStyleDto, pose: Option<KinematicsDto>, attitude: [f64; 4],
    angular_velocity_rad_s: [f64; 3], selected_by: StationDto, flown_by: Option<StationDto>,
    sequence: u64 }
pub struct MainScreenGrantsDto { stations: Vec<StationDto> }
// commands, in the sessions plan's command envelope (Design note 21)
MainScreenCommand::{ SelectPreset(CameraPresetDto), SelectTarget(Option<TargetDto>),
    SelectStyle(RenderStyleDto), TakeFreeCamera, ReleaseFreeCamera }
// R03's reserved optional field on SceneStateDto and SceneNotificationDto
main_screen: Option<MainScreenCameraDto>
```

Server (`hyperion_server`): `main_screen::{MainScreenCamera, Grants, FreeCameraIntegrator,
PresentationTrack}`.

Client: `apps/hyperion/src/renderer/src/mainScreen/{MainScreenApp, MainScreenStatusLine,
ModeBanner, AlertAnnunciator, roomTextSize, DisplaySetup}.tsx|ts`,
`displays/view/MainScreenControl.tsx`, the CLI option `--role main-screen` in
`apps/hyperion/src/main/cli.ts`, and `apps/hyperion/src/main/displayEdid.ts` (the EDID pre-fill).

### Test helpers

`test/litFixtures.ts` (`aHostDisc`, `aLitBody`, `SOLAR_SYSTEM_PHOTOMETRY`, the table of Design note
5), `lighting/oracle.ts`, and the kept scenes `view/scenes/eclipseScene.ts` and
`view/scenes/phaseScene.ts`.

## Consumes

Names are the owning plans' Provides as written on 2026-09-29; the owner is authoritative, and
where a name has changed by the time this plan runs, only the call sites here change.

- **R01:** `RenderEngine` (`createView` per canvas, `createMaterial`, `createPostProcess`,
  `createCompute`), `KernelPair` and `selectKernel`, `GpuCapabilities` (with `shaderF16`,
  `timestampQuery`, `rg11b10Renderable`), `styleAvailability` (the refusal of the photorealistic
  style on a fallback adapter), `GraphicsStatus`, `WGSL_CATALOGUE`, the smoke harness
  (`just test-render`) with its withheld-capability runs, `GPU_TIMING_SWITCH`
  (`--hyperion-gpu-timing`) for per-pass benchmarks, and R01.T13's child-window record. Asked of
  R01: a probe of each adapter's render-target rounding mode in `GraphicsStatus` (Design note 12).
- **R02:** `view/coords` (`relativeToCamera`, `narrow`, `originMinusCamera`), `view/camera`
  (`CameraPose`, `CameraState`, `RenderStyle`, `ViewRole`, `ViewId`, `project`,
  `pixelSolidAngle`, `perspectiveReversedInfinite`), `view/depth` (`DEPTH_COMPARE`,
  `transparentLayerOrder`), `view/photometry` (`V0_ILLUMINANCE_LX`, `illuminanceLx`, `apparentV`,
  `pixelLuminance`, `psfPixelWeights`, `ev100FromTriple`, `ev100FromAverageLuminance`,
  `exposureScale`, `ExposureControl`, `preExpose`, `HDR_COLOUR_FORMAT` with its alpha left to this
  plan, `toneCurve` and `agx` in `toneCurve.wgsl`, `DEFAULT_MAN_EV100`), `ViewScene`,
  `buildWireframeDrawList` with its casing width, `bodyRegime`, `ringEllipse`, `hullEdges`,
  `starSprite.wgsl`, `ViewDisplay`, `ViewCanvas`, `ViewMarkList`, `ViewLabelBlock`, the
  `ExposureControl` panel, `CameraControls`, and the nine guide drafts (R02.T2), items 2, 3, 4, 6
  and 9 above all.
- **R03:** `useScene` and `sceneAt` (`SceneFrame`, with each body's `geometricM`, `apparentM`,
  `emitted` and `hillRadiusM`), bodies as plan 14's `BodySummaryDto`, `SceneClockDto`,
  `KinematicsDto`, `CameraReporter`, the two-clients-agree test, and the optional `main_screen`
  field its Design note 4 reserves.
- **R05:** `selectPatches`, `PlanetGeometry`, `PatchCache` and the patch geometry with per-patch
  `f64` origins; `QualitySetting`, whose entries this plan's settings join. Asked of R05: a
  `PlanetGeometry` of a reference spheroid (equatorial and polar radii and a pole) drawn at zero
  height with no height worker (Design notes 3 and 19).
- **R06:** `HostDisc` (radius, mean luminance per channel at the surface, power-2 coefficients in B,
  V and R), `angular_radius`, `HostDiscLayer`, `StarColour`, `cameraLimitV` (which reads this plan's
  `ExposureReading`), and the disc's energy above the clamp "handed to R07's glare pass as one value
  per disc" (R06 Design note 16). Asked of R06: the disc pass writes meter weight 0 in the HDR
  target's alpha; for eye views, a `GlareSource` also for bright sources up to about 45° outside the
  frame (Design note 12).
- **R08:** `DiscReflectanceTable` for a body with an atmosphere, which replaces the albedo-only
  disc shading; it reads `BodyAppearance` and `starIlluminance`.
- **R10:** the disc-to-terrain hand-over by relief (its Design note 13), for which this plan's
  `mesh` regime is the smooth figure.
- **R11:** the far ring annulus and slab, which replace the cased ellipse; glint energy as a
  `GlareSource`.
- **Galaxy plan 14:** `BodySummaryDto` and its sections (`crates/hyperion-protocol/src/planetary/`);
  radius; the `SurfaceState` and Bond albedo of P14.T13.c once carried; `body_fixed_at` (P14.T14.c)
  through R02's `BodyFixedRotation`; the photometric section and flattening asked for in R07.T1.
- **Galaxy plan 06:** through R06's `host_discs`, each host's radius, effective temperature and
  surface gravity; `stellar::photometry::absolute_magnitude_v`
  (`crates/hyperion-sim/src/stellar/photometry.rs:137`) as the check of Design note 4.
- **The sessions plan (not written).** Phase C needs, by name to be fixed when it exists: a session
  with a ship, its name and clock; station identity per client (`StationDto`, with at least
  `captain` and `helm`); the ship's control arbitration (take, release, holder, override); ship
  commands with closed-loop results (`PENDING`, accepted, rejected with a reason, timed out); an
  input stream sent on change and at the tick; the 64 Hz loop; `session.json` persistence; the
  replay log; the simulation, training, replay and pause modes; and ship alerts in the guide's four
  classes. None of it exists: `ClientMessage` has `Hello { client_version }`, `Ping`, `Request` and
  `Cancel` only (`crates/hyperion-protocol/src/envelope.rs:34`), and nothing identifies a station.

## Design notes

What exists today, checked against the tree at `4abab50`: no `view/` directory, no WebGPU and no
perspective anywhere in the client; `lib/displays.ts` has `link`, `galaxy` and `system`; the
client's command line takes `--address` and `--port` only (`apps/hyperion/src/main/cli.ts`);
`PROTOCOL_VERSION` is 2 (`crates/hyperion-protocol/src/lib.rs:63`); plan 14's Bond albedo exists as
`BondAlbedo` (`planetary/derive/irradiation.rs:263`) with a provisional 0.3 until atmospheres close
(`planetary/params.rs:102`), and is carried by no field; the surface section is the empty enum
`Surface` (`planetary/record.rs:543`), `BodySurfaceDto {}` on the wire; no
`planetary/derive/rotation.rs` or `planetary/frames.rs`; no geometric albedo, phase integral or
flattening anywhere; `usePrefersReducedMotion.ts` and the annunciation component
`components/StatusLine.tsx` exist, the latter unrelated to the main screen's status line.

1. **Three regimes, chosen per view per frame on the CPU.** A body is a `point` while its angular
   diameter is under 3 px of the view's own pixels (brainstorm, "The scales the view spans"): its
   flux goes through R02's sprite path. Otherwise it is a `disc`: one quad, shaded per pixel. It is
   a `mesh` only when Design note 2 needs depth, or when R10 hands it to terrain. The
   `GAS_GIANT_FULL_PASS_BOUNDARY_M` of 10⁹ m is kept as the stated boundary inside which R08 runs a
   giant's full atmosphere passes; the brainstorm derives it from ten scale heights spanning 2 px,
   which gives 2.5 × 10⁸ m for Jupiter and 5.5 × 10⁸ m for Saturn at 1080p across 60°, and R07.T5
   re-derives both. Each threshold has a 10% hysteresis, so a body at the boundary does not flip
   every frame, and the set is a function of the camera, viewport, scene and previous set only,
   tested for order independence.
2. **The disc quad has no depth of its own, so bodies are ordered analytically.** The brainstorm
   keeps `frag_depth` writes unneeded (Depth), and a quad at one depth cannot order a moon against
   a planet's limb. Between two spheres that do not intersect, the one on the camera's side of their
   radical plane is in front along every ray that meets both, and the camera is on sphere A's side
   when its power, |c_A|² − r_A² (the squared tangent length), is the smaller. So analytic bodies
   are drawn back to front by power, computed in `f64`, which is exact for disjoint spheres, with no
   depth write; an oblate body is ordered on its equatorial bounding sphere. A disc whose screen
   footprint overlaps any geometry that writes depth — a mesh body, R10's terrain, a craft — is
   promoted to `mesh` for that frame, so that the depth test decides there. A point is a point
   source at its centre's depth, whose error is below its sub-pixel radius.
3. **The mesh regime is R05's quadtree on the reference spheroid, with no height.** A smooth figure
   is R05's cube-sphere at zero height, its unit directions scaled by (a, a, c) in the body frame,
   with its per-patch `f64` origins, so that nothing reaches the GPU in world coordinates
   (brainstorm, "The floating origin"). The zero source needs no worker. This is the reading closest
   to the step order: R05 precedes this plan, and R10 later hands the same geometry real heights.
   A lone sphere mesh differenced at the body's centre would break the per-patch rule.
4. **Illuminance per display channel, from R06's host discs.** A host of angular radius ρ = asin(R
   ÷ d) and mean surface luminance L̄_c gives E_c = π L̄_c sin²ρ face-on; the V-weighted sum is
   checked against 2.54 µlx × 10^(−0.4 V) from the star's absolute V through R02's `apparentV`,
   with no extinction inside a system. The star is taken at the body's own emitted time; neglecting
   the star-to-body light time moves the light's direction by the star's reflex speed over c, some
   10⁻⁸ rad for a Jupiter's pull on a Sun, which a test states. Stars of a multiple system each
   light the body; a star whose illuminance at the body is under 10⁻⁴ of the brightest is dropped,
   R08's cut, so that both plans agree on which suns shine.
5. **The BRDF is a lunar-Lambert disc times a fitted phase function** (researched 2026-09-29;
   Mallama, Krobusek and Pavlov 2017, Icarus 282, 19, Table 7; Mallama and Hilton 2018, Astronomy
   and Computing 25, 10, arXiv:1808.01973, eqs. 2–17; McEwen 1991, Icarus 92, 298, from memory).
   Every measured phase integral in V lies between 0.48 (Mercury) and 1.36 (Saturn), below
   Lambert's 1.5 and Lommel–Seeliger's 1.64, so no law of fixed disc-integrated shape reaches p and
   q together; the law needs a free phase function. It is I/F = A · f(α) · [L · 2μ₀ ÷ (μ₀ + μ) +
   (1 − L) μ₀] with f(0) = 1, whose geometric albedo is p = A [L + ⅔(1 − L)] in closed form, and
   whose disc-integrated phase is Φ(α) = f(α) Φ_shape(α; L), with Lambert's and Lommel–Seeliger's
   Φ in closed form inside Φ_shape. Per surface state, f is fitted to a Solar System analogue's V
   phase curve Φ_t, so f = Φ_t ÷ Φ_shape reproduces the measured curve; another q is reached by
   Φ_t^s, s solved by bisection since q falls monotonically in s. L is 1 for airless states (a flat
   full Moon), 0 for cloudy and gas states, 0.5 for thin atmospheres. f is clamped at 4 and held
   past the template's valid range (a Lambert crescent vanishes faster than a cloudy one: Venus's f
   would reach 61 at 170°), and the point regime integrates the same clamped law, so flux is
   continuous at 3 px by construction. In the shader it is one division, one multiply-add and a 1D
   texture fetch; Hapke has parameters p and q do not constrain and stays R11's for rings. The
   disc-integrated flux is F = E★ p (R ÷ Δ)² Φ(α). The values come from plan 14 (R07.T1), which is
   asked for p in B, V and R, a phase template and s, L, and the stated ratio p_V q_V ÷ A_Bond,
   never p derived from the Bond albedo through a fixed q: the ratio is physical, about 1 for grey
   regolith, 0.69 for red Mars and about 2 for the giants, dark in the near infrared. Until the
   section is on the wire a body takes `PROVISIONAL_PHOTOMETRY`, a Lambert sphere of spherical
   albedo 0.3 (p = 0.2, q = 1.5, brighter at large phase than any real body), and the label block
   says `BODY ALBEDO: NOT YET MODELLED` (a phrase for the owner, R07.T16).
6. **The horizon in closed form, eclipses by annuli** (researched 2026-09-29; Howell's catalogue of
   radiation view factors, configurations B-41 and B-42,
   <https://www.thermalradiation.net/tablecon.html>, formula transcribed from memory and verified by
   brute force to 2 × 10⁻³; Kreidberg 2015, PASP 127, 1161). The brainstorm's "sampled at a handful
   of points" is 13–26% wrong for small occluders at 4 to 8 samples and bands by 1 ÷ N, and needs
   about 256 samples for 1%. Instead a lit point's visibility is the product of two closed forms.
   The horizon term is the irradiance factor from a uniform sphere of H = d ÷ R★ at angle φ between
   the normal and the star's centre, which is exact for a uniform disc and within 0.45% of the
   limb-darkened one for a star 19.5° in radius; it softens the terminator and lights a close-in
   planet beyond its hemisphere (to 109.5° at 3 stellar radii). The eclipse term splits the disc
   into K annuli of uniform intensity with edges uniform in μ, and takes each annulus's eclipsed
   area as the difference of two exact circle–circle overlaps: continuous, so it never bands; 0.62%
   worst absolute error at K = 4 (high) and 1.5% at K = 2 (low). The product errs only where an
   eclipse's penumbra crosses the terminator band. Occluder lists are built on the CPU in `f64` from
   shadow cones and are usually empty. At 1 au the soft terminator is 59.3 km wide on an Earth-sized
   body and lies below 0.5% of peak irradiance, black under a day-side exposure. Mandel and Agol
   2002 and Agol, Luger and Foreman-Mackey 2020 are the `f64` oracles' cross-checks; Maxted and Gill
   2019's qpower2 is validated only below a radius ratio of 0.2 and is not used.
7. **Planetshine is included** (researched 2026-09-29; figures computed from Design note 5's
   albedos). The brainstorm does not mention it; the realism ruling decides. It is the dominant
   night-side light of every moon: earthshine on the Moon at full Earth is about 15 lx, 13 stops
   below sunlight but 10³–10⁵ times the integrated starlight; Jupiter-shine on Io about 70 lx, 6
   stops below. Each body is lit by at most two neighbours whose reflected illuminance
   E★ p (R ÷ Δ)² Φ(α) is largest (one on the low setting), each as a uniform sphere of its angular
   radius through the same `sphere_irradiance`, never shadow-tested against third bodies. Its
   stated errors are the lit crescent's offset from the neighbour's centre (up to 0.4 R, about 4°
   for Jupiter seen from Io) and a finite-distance correction of order (R ÷ Δ)², about 3% at Io.
8. **The photorealistic style is a pass list over R02's scene, camera and HDR target.** In order:
   R06's sky and host discs; mesh bodies (opaque, depth); analytic bodies in painter order; R08's,
   R10's and R11's passes in their places when they exist; the histogram; bloom of the light above
   the display's range; the tone-mapping pass with the analytic glare, upscale, encoding and
   dither; symbology cased over the result; DOM readouts on plates. A style owns no scene, camera or
   projection, so switching is a change of pass list, and a body projects to the same pixel in both
   styles, which R02's style-switch test extends to cover. On a fallback adapter R01's
   `styleAvailability` refuses the photorealistic style, and the control says so.
9. **One AgX function in both styles** (researched 2026-09-29; Filament's `ToneMapper.cpp`,
   Apache-2.0, <https://github.com/google/filament/blob/main/filament/src/ToneMapper.cpp>). R02's
   `toneCurve` is the full AgX function with its WGSL twin `agx`, applied per star sprite in the
   wireframe. The tone-mapping pass calls the same `agx`, so an isolated star on black is identical
   in both styles by construction; what differs is tone(sky + star) against tone(sky) + tone(star),
   which is why this style draws its stars into the HDR target before the pass. The function is
   Filament's port: Blender's inset and outset matrices in Rec. 2020 with conversion from and to
   linear Rec. 709, `AgxMinEv` −12.47393 and `AgxMaxEv` 4.026069, the seventh-order polynomial fit
   after Wrensch's minimal AgX, no look, its header kept and a NOTICE entry added. EaryChow's,
   Sobotka's and iolite's versions carry no licence and are not ported; Blender's shipped LUT is an
   offline test oracle only. If R02 built another fit, R07.T15 replaces `toneCurve` and `agx` with
   the port in one change. The metered average lands at qK ÷ 78 = 0.104 of AgX's input, 0.79 stop
   below its 0.18 pivot, which is realistic for a K of 12.5 (about 12% reflectance); no hidden bias
   is added, and exposure compensation stays the operator's, which answers R02's question.
10. **Metering reads a weight from the HDR target's alpha** (researched 2026-09-29; bruop.github.io
    /exposure; probes on the UHD 620, provisional under load). Colour has no alpha use, so each
    pass writes a meter weight there: 0 for a host's disc (R06's ask), so that the exposure does not
    step when the star enters frame (brainstorm, Exposure and tone mapping); and for bodies, under
    the operator's `LIT` or `DARK` meter, 1 on the lit or unlit side and 0 elsewhere. `AVG` weighs
    everything but host discs. The histogram has 256 bins over log₂ of the pre-exposed value from
    −14 to +16, the range `rgba16float` can hold; bin 0 takes everything below 2⁻¹⁴, zeros included,
    since a black pixel is real (no) light to a meter; the others are 0.118 stop wide. Weights are
    integers, round(alpha × 16). The kernel accumulates in workgroup memory with atomics and adds
    each non-empty bin once to the global histogram; per-pixel global atomics are forbidden, being
    20–60 times slower under a dark sky, where every pixel lands in one bin (16.6 ms against 0.3–0.8
    ms at 640 × 360). Integer adds make any reduction order bit-exact, as anything read back must be
    (brainstorm, "The graphics API, and the Intel problem"); a subgroup pre-reduction saved only
    10–15% on uniform input and is not built unless R01's catalogue requires the pair. The histogram
    is read back asynchronously, one to three frames late, and exposure is computed on the CPU in
    TypeScript, so the displayed value is exactly the applied one and the controller is unit-tested
    without a GPU. The next frame's pre-exposure uses the latest applied exposure.
11. **Exposure is the photographic model, on an arithmetic mean** (researched 2026-09-29; Lagarde
    and de Rousiers 2014, _Moving Frostbite to Physically Based Rendering 3.0_, §5.1, eqs. 67–75,
    Listing 28, pp. 83–85, read; ISO 2720 and ISO 12232 via their summaries). EV100 = log₂(L̄ × 100
    ÷ 12.5) through R02's `ev100FromAverageLuminance`, and the saturation-based white L_max = 1.2 ×
    2^EV100 = 9.6 L̄ holds as written. A reflected-light meter integrates flux, so L̄ is the
    weighted arithmetic mean of luminance, Σ nᵢ Lᵢ ÷ Σ nᵢ with Lᵢ = 2^(bin centre) ÷ pre-exposure,
    not the log-average games use: over a dark sky it exposes a lit planet 4–5 stops above the
    average, inside AgX's 7.3 stops above it, where a log-average would expose for the sky and burn
    the planet. `AVG`'s window is therefore [0, 1], no percentile clip; a clip of the top 1% would
    delete a small planet. Temporal smoothing is in EV and frame-rate-independent, after Unreal's
    documented structure: linear at 3 EV/s brightening and 1 EV/s darkening while |ΔEV| exceeds 1.5,
    exponential inside that band (the speeds from memory, to be settled by eye). No dark adaptation
    of the eye is modelled. `MAN` takes R02's triple. `INHIBITED` freezes the last value. Exposure
    adaptation is not motion, so `prefers-reduced-motion` leaves it alone. A wireframe instrument
    view takes the `ExposureReading` of the photorealistic view it accompanies and shows which.
12. **Glare is the CIE glare spread function, applied to the light the display cannot show**
    (researched 2026-09-29; Vos and van den Berg 1999, CIE 135/1999, as reproduced in McCann and
    Vonikakis 2018, Front. Psychol. 8:2079, eq. 2). For an eye view (R02's `ViewRole` `"eye"`) the
    PSF is the CIE function at age 30 and pigmentation 0.5, renormalised to one; its first angular
    constant is 0.0046°, not the 0.046° the open copies print, which integrates to about 37 instead
    of 1. It puts 58% of its energy beyond one pixel at 1080p across 60°, and the player's eye
    already supplies glare for what the display reproduces, so only L_high = L − min(L, T) is
    bloomed, T being AgX's top of range, about 157 L̄: energy is still conserved. A camera view's
    PSF is a small core and a Harvey-type tail holding 2–4% of the energy beyond a few pixels (from
    memory, after ISO 9358's veiling glare index; low confidence). The mip chain is Jimenez 2014's
    13-tap down and tent up without the Karis average, which does not conserve energy; its level
    weights are a non-negative least-squares fit of the chain's impulse response to the PSF's
    encircled energy at 2^k px, computed per view from its angular pixel scale. A clamped disc's
    excess cannot live in any `rgba16float` level (an O star's is some 10⁹ times the format's
    maximum), so each `GlareSource` is evaluated in closed form in the tone-mapping pass in `f32`;
    R06's discs and R11's glint use the same entry. Veiling glare is real light on the retina or
    sensor, so the meter adds each source's analytic frame-mean veil, and for eye views includes
    sources to about 45° outside the frame: exposure then changes continuously as a star nears and
    enters the frame. Colour-attachment writes on Gen9 round toward zero (probed), a −0.78% to
    −1.56% bias per write in `rg11b10ufloat` and −0.05% in `rgba16float`, so the bloom chain stays
    in `rgba16float` unless R01's probe reports round-to-nearest, and the CPU twin models
    truncation. The chain uses no random sampling, so the guide's flash limit holds.
13. **The tone-mapping pass encodes and dithers** (researched 2026-09-29; probes on the UHD 620).
    AgX's formed image spans 16.5 stops, −10 to +6.5 about 0.18, about 9.2 below and 7.3 above the
    metered average; the brainstorm's "roughly 25 stops" is AgX Log's encoding. The canvas is
    `rgba8unorm`, `getPreferredCanvasFormat()` on this machine, written through a non-sRGB view with
    the encoding in the pass, so that the dither is applied in the encoded domain: triangular
    (TPDF) noise of ±1 LSB from a static blue-noise tile, never animated. Whether Filament's
    `pow(v, 2.2)` then the sRGB curve or the sigmoid's output written directly is right near black
    is settled by a dark ramp against Blender's AgX Base sRGB to one code (R07.T15). The pass also
    upscales from the view's internal resolution.
14. **Per-view budgets are a pure policy** (researched 2026-09-29; probes on the UHD 620,
    provisional under load). The primary view renders at its internal scale; each secondary view at
    a lower scale or 30 Hz; on the low setting at most one view is photorealistic, and the style
    control of any other view is disabled with the reason (brainstorm, "Several views in one
    client"). While instruments are open the photorealistic view's internal scale drops, and a
    controller holds it between 0.5 and 1.0 from the GPU frame time, from `timestamp-query` where
    the adapter has it and the frame interval otherwise. Under the forced switches timestamps are
    quantised to 65,536 ns, ±0.4% of a 16 ms frame, which the controller bears; per-pass benchmarks
    run under R01's `--hyperion-gpu-timing`, never `--enable-unsafe-webgpu`, which also lifts the
    quantisation but exposes experimental features. Presenting two extra small canvases cost below
    the probe's resolution, so `PER_CANVAS_OVERHEAD_MS` starts at 0.3 ms, flagged provisional and
    measured in R07.T20. A 4K main screen renders below native through the same scale. Secondary
    views stream terrain at lower priority (R05). The policy's outputs are data that R12's runs
    record.
15. **Instrument views are panels in `VIEW`; child windows are proved** (researched 2026-09-29;
    probe of a same-origin child window, passed to R01.T13). The guide's frame keeps its header
    strip, work area and navigation bar; the primary view fills the work area and up to two
    instrument views sit in fixed slots over its right edge, each a canvas with its own context,
    camera, style and target, focusable, named and paired with its own DOM list (R02). Two is the
    brainstorm's tested case. A same-origin child window's canvas configured with the opener's
    device renders and presents, paced at 16.7 ms from either window's animation frames; closing it
    is not a device loss, but the next submit raises a validation error, so the client drops the
    child's context on `pagehide` and expects that error. Pacing on a second monitor is untested,
    for want of one (R07.T21).
16. **Symbology over the image is cased; text sits on plates.** Every mark over a photorealistic
    image is stroked twice, a `--surface-0` casing at the guide's widths beneath the coloured
    stroke, never a blur or glow (guide item 3); a DOM readout over the canvas sits on its own
    `--surface-0` plate. The wireframe style's marks follow the guide's ordinary rules. The image is
    never dimmed under symbology. Status colours reserved for symbology cannot be reserved from
    photons, so marks are told from the image by shape and outline (brainstorm, The contrast
    problem).
17. **Nothing in the scene goes undrawn in the photorealistic style.** Until R11 draws rings they
    are R02's ring ellipses, and craft are R02's hull outlines, both cased over the image. A body
    with no photometric section is drawn with the provisional photometry and labelled (Design note
    5), never omitted.
18. **Low settings are built with the high ones.** Histogram over a quarter-resolution input;
    bloom with fewer levels at quarter resolution; `DISC_ANNULI_LOW` annuli; planetshine from one
    neighbour; 720p presented upscaled; one photorealistic view. The budget rows are the targets:
    histogram 0.3–0.5 ms on the discrete part and about 1 ms on the UHD 620, bloom and tone mapping
    under 1 ms and 2–3 ms (brainstorm, Performance budget). The histogram's probe at 640 × 360,
    0.25–0.8 ms, is inside its row, provisionally, since it was measured under shared load. Each
    target is a finding if missed, not a failure.
19. **Giants are drawn oblate** (researched 2026-09-29; flattenings from NASA's fact sheets; the
    Darwin–Radau relation, from memory after Murray and Dermott 1999, checked against six planets).
    Flattening f shows at one pixel once the disc is 1 ÷ f px across: 10 px for Saturn (f = 0.098),
    15 px for Jupiter, 44 and 59 px for Uranus and Neptune, 300 px for Earth. So every close view of
    a giant is visibly oblate, and Saturn seen from its equator is about 10% dimmer than a sphere of
    the same albedo. Plan 14 is asked (R07.T1) for f by Darwin–Radau, f = (5 ÷ 2) q_r ÷ [1 + (25 ÷
    4)(1 − (3 ÷ 2) C ÷ MR²)²] with q_r = ω² a³ ÷ GM, from its rotation and a moment of inertia per
    composition class (rocky about 0.33, gas giants 0.25, Saturn-like 0.21, ice giants 0.23), not
    from its elastic tidal k₂; capped at 0.2, skipped below about 250 km radius, with a = R_vol (1 −
    f)^(−⅓) and c = a (1 − f), and p defined against the projected area π a c equator-on, as Mallama
    does for Saturn. The formula gives Earth, Jupiter and Saturn within 1.3%; Mars and Uranus are
    15% off (Tharsis, and Uranus's uncertain period). The disc regime intersects a spheroid as a
    sphere after scaling the body frame's polar axis by a ÷ c, the normal by the inverse transpose,
    with the pole from R02's `BodyFixedRotation`; the point's area is π a b′ with b′ = √(a² sin²β +
    c² cos²β) at sub-observer latitude β; the mesh is R05's spheroid (Design note 3). Until plan 14
    sends f, bodies are spheres. R02's graticule and collision stay spherical.
20. **The main screen's camera is ship state; a local camera is not.** The server holds
    `MainScreenCamera` (preset, target, style, pose, who selected, who flies) in the session and
    saves it in `session.json`. Selections are ship commands, closed-loop, from any granted
    station; the last the server accepts stands and every client shows who set it. Grants default
    to the Captain and Helm, and to Helm alone on a bridge without a Captain (brainstorm, after
    EmptyEpsilon and Artemis; MIL-STD-1472H §5.2.2.12.7.1 has group displays controlled by
    designated users). Flying the free camera is exclusive, taken and released through the ship's
    control arbitration, and the Captain can always take it. The server integrates the pose at each
    tick from the holder's input, in `hyperion-server`, outside `hyperion-sim` and its determinism
    rules, since the camera changes nothing in the simulation; the input is logged on a
    presentation track that a replay shows and its bit-for-bit check skips. The main screen always
    cuts between presets. A local view's camera stays a display control reported to R03's
    `CameraReporter`.
21. **The main screen waits on sessions, and says so.** Phase C needs what the sessions plan will
    own (Consumes). Nothing in Phase C is built as a stub before it exists, so as not to guess the
    shape of stations, commands and arbitration. Phases A and B need no session: a view without a
    ship runs on R03's ship stand-in.
22. **The loop is 100 ms from input to photon, display included** (researched 2026-09-29;
    MIL-STD-1472H §5.12.1.4.1.1, read; RTINGS's input-lag method). The standard counts 100 ms (20–50
    ms preferred) round trip to the display of the result, so the brainstorm's "excluding the
    display device" is too generous. The breakdown, keyboard input, typical and worst: USB poll 4
    and 8 ms; LAN 1 and 2; the server's wait for the tick 7.8 and 15.6; integrate and push 1 and 2;
    the main screen's wait for its animation frame 8.3 and 16.7; render, composite and present 25
    and 33.4; scanout to the screen's centre 8.3; a game-mode television 1–7 and 15. Interpolating a
    tick behind would add 15.6 ms and sending input only at the tick another 7.8–15.6. So the
    station sends input on change as well as at the tick, and the main screen extrapolates the pose
    from the last pose and its angular and linear rates, blending corrections over one tick: about
    60 ms typical and 102 ms worst with a game-mode display, the worst 16.7 ms more for a gamepad,
    which the Gamepad API polls only on animation frames. This departs from the brainstorm's "the
    main screen interpolates between poses" (notes), is safe because the camera is presentation
    only, and keeps interpolation as the fallback R07.T28 compares. Only the flyer has a control
    loop, so the flying station also draws a local predicted view of the camera, integrated from its
    own input at display rate, whose loop is some 30–50 ms. A discrete command shows `PENDING`
    within 100 ms (MIL-STD-1472H §5.1.2.1.6.4, Table V) and completes within 250 ms.
23. **Main-screen text is sized for the room** (researched 2026-09-29; MIL-STD-1472H, read). Its
    text is colour-coded, so it follows §5.17.25.14: at least 20′ at the longest anticipated
    viewing distance, above §5.17.18.2's 10′ "shall" and 15′ "should", measured from the top of the
    capitals to the bottom of the descenders (§3.2.28). The alert annunciator's text is at least 30′
    and the newest emergency up to 60′ (§5.7.3.6, warning and caution signals). The height in
    device pixels is 2 D tan(θ ÷ 2) × (width in pixels ÷ W), divided by `devicePixelRatio` for CSS
    (this machine runs at 0.78125); at 4 m on a 55″ 1080p television that is 36.7 px at 20′ and
    55.0 px at 30′. No web API gives a display's physical size, so the main-screen machine holds two
    settings, the screen diagonal and the furthest viewing distance, the diagonal pre-filled from
    EDID where it is plausible (non-zero and within 5% of the pixel aspect; Electron's main process
    reads `/sys/class/drm/card*-*/edid` on Linux), with an on-screen 100 mm bar to check it. EDID is
    unreliable for televisions and zero for projectors. The setup warns when distance ÷ diagonal
    falls outside 2–10 (§5.2.2.12.3) and states that a television must be in game or PC mode,
    since outside it input lag reaches 40–120 ms. The guide's rem scale does not apply.

## Tasks

Phase A builds the photorealistic style for one view, Phase B several views, Phase C the main
screen. T1 and T2 can run beside T3–T6. T7 needs T3–T5; T8–T11 follow T7; T12–T15 follow T7 and
are independent of T8–T11; T16 follows T8; T17 closes Phase A. Phase B follows T7 and T13. Phase C
waits on the sessions plan. TypeScript paths are under `apps/hyperion/src/renderer/src/view/`
unless they start otherwise. Every timing below that comes from the research probes was measured
under shared load and is provisional until re-measured on a quiet machine.

### Phase A: lit bodies and the photorealistic style

#### R07.T1 The photometry and figure asks of galaxy plan 14

Draft, in `docs/agent/plans/galaxy-generation/14-planetary-systems.md`, a subtask beside P14.T24 for
a `photometry` section of `BodySummaryDto` at `Bulk` detail (Design note 5): `geometric_albedo` in
B, V and R, a phase-curve template per surface state and its exponent s, the lunar-Lambert share
L, and the stated ratio p_V q_V ÷ A_Bond; the templates of the research (gas envelope Jupiter, L 0;
runaway Venus, L 0; temperate Earth, L 0; airless rock the Moon's p_V 0.12 on Mercury's curve, L 1;
airless ice Ganymede or Europa, snowball and magma provisional and labelled), p scaled by the
state's Bond albedo over the template's and capped so that p q ≤ 1; its tests reproduce Mallama et
al. 2017's Table 7 and the computed q to 0.5% and state each analogue's ratio. Add two checks for
plan 14's owner: airless rock's Bond albedo of 0.11 against the Moon's p_V 0.12 and a Mercury-like q
of 0.48, which give 0.06 (Lane and Irvine 1973 to be read); and Earth's p_V 0.434 with Tinetti's
curve, which gives A_V 0.57 against a Bond albedo of 0.294. Draft beside it the flattening of Design
note 19 (f, or equatorial and polar radii) with its per-class moment of inertia and the six-planet
check. Both subtasks are plan 14's to build; the edit ends in plan 14's owner accepting them.
Acceptance: the drafted subtasks cite the sources above; `npx prettier --check` on both plan files.

#### R07.T2 The photometric section on the scene

Once plan 14's section exists on the wire, R03's scene carries it inside `BodySummaryDto` with no
change of its own; before then the client parses its absence as `not_modelled`. Build the client
side: `lib/system/bodiesWire.ts` and `model.ts` parse `photometry` and the figure as `SectionDto`s;
`appearance/fromWire.ts` maps them, or their absence, to `BodyPhotometry` with `provenance` and to
`BodyFigure`. Tests: fixtures in each section state; a body without the section maps to
`PROVISIONAL_PHOTOMETRY` and carries the `BODY ALBEDO: NOT YET MODELLED` label; a body without a
flattening is a sphere. Acceptance: `pnpm test`, `just ci`.

#### R07.T3 Illuminance at a body

`lighting/illuminance.ts`: `starIlluminance` from R06's `HostDisc` (Design note 4); the
multiple-star cut at 10⁻⁴. Tests against hand values, re-checking the brainstorm's citations: the
Sun at 1 au gives 1.28 × 10⁵ lx from V = −26.76 (Willmer 2018, ApJS 236, 47) with V = 0 at 2.54 µlx
(Cox 2000, _Allen's Astrophysical Quantities_ §15; Crumey 2014), and the disc form agrees with the
magnitude form to 1%; the inverse square to Mars; the channel split's V-weighted sum is E_V; the
direction error from neglecting the star-to-body light time is under 10⁻⁷ rad for a Sun with a
Jupiter. Acceptance: `pnpm --filter hyperion test view/lighting`.

#### R07.T4 The BRDF and its phase function

`appearance/{law,brdf,phase,templates}.ts` and `body_brdf` in `shaders/litBody.wgsl` (Design note
5). `PHASE_TEMPLATES` from Mallama and Hilton 2018's eqs. 2–17 inside their valid ranges (Mercury's
zeroth-order term −0.613, as that paper corrects the 2017 table), f tabulated at 0.5°, clamped at
`PHASE_F_CLAMP` and held past each range; `lawFor`; `discIntegratedPhase` by the closed forms inside
Φ_shape. `test/litFixtures.ts` gains `SOLAR_SYSTEM_PHOTOMETRY`: p in B, V and R, B−V and V−R, V(1,0)
and the computed q_V for Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus and Neptune. Tests:
the closed forms of p and Φ for Lambert (p = 2A ÷ 3, q = 3 ÷ 2) and Lommel–Seeliger (p = ϖ ÷ 8, q =
16 ÷ 3 × (1 − ln 2)) against the `f64` disc integral to 10⁻⁴; `lawFor` recovers each template's p
and q to 0.5%, with the departure the clamp causes stated per template; each planet's V at stated
geometries within the paper's tolerance (about 0.01–0.03 mag for Mercury, Venus and Jupiter; Mars
within its 0.035 mag of longitude variation; Earth within 30%); the TypeScript and WGSL functions
agree at five pinned geometries to 10⁻⁵ relative in the smoke harness. Acceptance: `pnpm test`,
`just test-render`.

#### R07.T5 Regimes and painter order

`bodies/{regime,painter}.ts` (Design notes 1, 2 and 19). Tests: the 3 px threshold at 720p, 1080p
and 4K; the brainstorm's figures re-derived: a Jupiter disc at least 3 px to about 9 × 10¹⁰ m, ten
scale heights at 2 px to 2.5 × 10⁸ m for Jupiter and 5.5 × 10⁸ m for Saturn (scale heights from
NASA's planetary fact sheets, cited in the test); hysteresis holds a body at the boundary; the
regime map is independent of input order; painter order by power equals a ray-marched truth for 10⁴
random disjoint sphere pairs from random cameras; an oblate body is ordered on its equatorial
sphere; a disc overlapping a mesh body is promoted. Acceptance:
`pnpm --filter hyperion test view/bodies`.

#### R07.T6 The horizon and eclipse terms

`lighting/{sphereIrradiance,annuli,occluders,oracle}.ts` and `sphere_irradiance` and
`eclipse_visible` in `litBody.wgsl` (Design note 6). Tests: Howell's factor against brute force to
2 × 10⁻³ at H = 3, 11.5 and 215, and its limb-darkening error at most 0.45% at 19.5°; the eclipse
term's worst absolute error at most 0.62% at K = 4 and 1.5% at K = 2 over a grid of radius ratios
0.1–30 and 41 separations, against 400 annuli; the terminator 59.3 km wide on an airless body of
6,371 km at 1 au, and E ÷ E_zenith at the geometric terminator 9.87 × 10⁻⁴ for a uniform disc and
9.27 × 10⁻⁴ for the brainstorm's polynomial law; a concentric occultation against the closed form
[(1 − c) μ_k² + 2c μ_k^(α+2) ÷ (α + 2)] ÷ [(1 − c) + 2c ÷ (α + 2)], μ_k = √(1 − k²), and a total
eclipse exactly 0; umbra and penumbra radii r_u = R_o − x (R★ − R_o) ÷ d and r_p = R_o + x (R★ +
R_o) ÷ d at pinned distances; a planet at 3 stellar radii lit to 109.5°; occluder lists empty for a
body in no shadow cone. Acceptance: `pnpm test`; the WGSL equals the oracle at five pinned points in
`just test-render`.

#### R07.T7 The photorealistic style

`photoreal/{style,passes}.ts` (Design note 8): the `photorealistic` variant of R02's `RenderStyle`,
the pass list with empty slots for R08, R10 and R11, the style switch per view in
`displays/view/StyleControl.tsx` (a display control, single-key binding shown), and the refusal on a
fallback adapter through R01's `styleAvailability`. The HDR target is R02's. Tests: switching style
leaves the camera, projection and every body's projected position identical (R02's test extended);
the pass list for each setting; the control disabled with its reason on a fallback adapter; the
smoke harness renders an empty photorealistic frame with finite texels. Acceptance: `just ci`,
`just test-render`.

#### R07.T8 Point and disc bodies

`shaders/{bodyDisc,bodyPoint}.wgsl`, `bodies/draw.ts`. The disc: a quad over the body's projected
bound, ray against the spheroid (Design note 19) in the camera-relative direction normalised by
distance so that no large numbers meet in `f32`, `body_brdf` per channel under the horizon and
eclipse terms, meter weight per Design note 10. The point: F = E × p × (R ÷ Δ)² × Φ(α) per channel,
over the oblate area where there is one, into R02's sprite. Tests: at the 3 px switch the disc's
summed pixel flux equals the point's to 1% (a CPU rasteriser of the same shader arithmetic in
TypeScript, and in the smoke harness); a phase scene (`view/scenes/phaseScene.ts`: a body at 0°, 90°
and 150° phase) whose limb and terminator match the oracle to half a pixel; a Saturn-like f = 0.098
disc's polar and equatorial extents to half a pixel at 100 px. By hand, recorded: the phase scene
and a Jupiter from 10¹⁰ m. Acceptance: `just ci`, `just test-render`.

#### R07.T9 Mesh bodies

`bodies/smoothMesh.ts` (Design note 3): R05's selection and geometry on the reference spheroid at
zero height, with the same shading functions; the promotion of Design note 2. Tests: a promoted disc
and its mesh agree in projected silhouette to half a pixel, oblate giants included, and in total
flux to 1% at the switch; a moon passing behind a planet's limb is hidden where the oracle says,
over a scripted occultation. By hand, recorded: the occultation. Acceptance: `just ci`.

#### R07.T10 Eclipses in the image

`view/scenes/eclipseScene.ts`, the kept scene: a camera crossing a moon's shadow on a planet, and a
ship in a moon's penumbra looking at the star. Tests: the shadow's umbra and penumbra on the planet
from the oracle to a pixel; the flux on a probe point over the crossing against the oracle to the
setting's error. By hand, recorded: a partial eclipse seen from the penumbra, the star's disc partly
covered by the moon's disc through the painter order. Acceptance: `just ci`.

#### R07.T11 Planetshine

`lighting/planetshine.ts` and its term in `litBody.wgsl` (Design note 7): `planetshineSources`
returning direction, angular radius and illuminance per channel, through `sphere_irradiance`. Tests:
earthshine on the Moon at full Earth 15.3 lx ± 20% (the spread of Earth's p); the full Moon on Earth
0.32 lx from V = −12.74 to 5%; Jupiter-shine on Io at inferior conjunction about 70 lx ± 10%; a
neighbour at new phase contributes about nothing; a body's sources are the two largest, one on the
low setting. Acceptance: `pnpm test`.

#### R07.T12 The exposure histogram

`post/histogram.wgsl` and `post/histogram.ts` (Design note 10): 256 bins over log₂ −14 to +16 of the
pre-exposed value, integer weights, workgroup-memory atomics with one global add per non-empty bin,
read back through a ring of three buffers; a `KernelPair` with `readback: "bit-exact"`. Tests: a CPU
histogram of a synthetic target equals the GPU's bin for bin in the smoke harness, with and without
subgroups; host-disc pixels of weight 0 are not counted; zeros land in bin 0; the readback never
maps a buffer in use. Bench, recorded under `--hyperion-gpu-timing` with the flag noted, with a
uniform dark-sky input as the worst case: 0.3–0.5 ms on the discrete target, about 1 ms at 640 × 360
on the UHD 620 (brainstorm, Performance budget; the probe's 0.25–0.8 ms is provisional). Acceptance:
`just ci`, `just test-render`.

#### R07.T13 Auto exposure and metering

`post/autoExposure.ts`, `displays/view/MeterControl.tsx` (Design notes 10–12): `meteredLuminance`
as the weighted arithmetic mean with window [0, 1] by default, plus the frame's veil;
`AutoExposure.step` with the smoothing of Design note 11; `AVG`, `LIT` and `DARK` meters; `AUTO`,
`MAN` and `INHIBITED` through R02's `ExposureControl`, entering `INHIBITED` when no histogram
arrives; the `ExposureReading` for R06's `cameraLimitV` and for wireframe views. Tests, against hand
values with the citations re-checked: EV100 from a uniform 100 cd/m² field is log₂(800) = 9.644
(Lagarde and de Rousiers 2014, eq. 69); the displayed EV100 equals log₂(mean × 8) for a two-level
histogram; the white point is 9.6 × L̄ under a linear clip (eq. 75, Listing 28); a 5% disc 25 stops
above a black frame is exposed 4–5 stops above the average, not clipped; exposure changes by no
more than 0.05 EV a frame as a star crosses the frame's edge, the disc carrying weight 0; 30 Hz and
60 Hz step sequences reach the same EV at the same time within 0.05 EV; `LIT` and `DARK` differ as
stated on a half-lit body; `INHIBITED` holds; the control shows EV100 with its automation level,
keyboard operable. By eye, recorded: a lit planet on black, a star entering frame, the cockpit
turning to a planet, which settle the smoothing speeds. Acceptance: `pnpm test`.

#### R07.T14 Bloom as veiling glare

`post/{bloomDown,bloomUp}.wgsl`, `post/{bloom,glare}.ts` (Design note 12): `glareSpread` (the CIE
function with 0.0046°, age 30, pigmentation 0.5, renormalised; the lens tail for camera views);
`bloomKernel` by non-negative least squares per angular pixel scale; the threshold at AgX's top of
range; `rgba16float` intermediates unless R01's probe reports round-to-nearest; `frameVeil`; each
`GlareSource` evaluated in the tone-mapping pass. Tests: the CIE function integrates to 1.047 at age
25 and 1.010 at pigmentation 0 before renormalisation; the chain's impulse response matches its
encircled energy to 10% at 1, 4, 16 and 64 px; weights sum to 1 at both settings; with truncation
modelled in the CPU twin, stored plus injected energy equals the unclamped energy to 1%, and on
hardware in the smoke harness to 1.5%; the low setting uses fewer levels at quarter resolution.
Bench, recorded under `--hyperion-gpu-timing`: under 1 ms and 2–3 ms with tone mapping. Acceptance:
`just ci`, `just test-render`.

#### R07.T15 Tone mapping, upscale and output

`post/tonemap.wgsl`, `post/tonemap.ts` (Design notes 9 and 13): R02's `agx`, replaced by Filament's
port with its header and a NOTICE entry if R02 built another fit, in the same change as `toneCurve`;
the canvas configured `rgba8unorm` with the encoding in the pass; static blue-noise TPDF dither of
±1 LSB in the encoded domain; upscale from the internal resolution. Tests: `agx` against values
computed once in `f64` from Filament's formula and pinned with their citation; monotone in
luminance; an isolated star identical in both styles; the WGSL matches the TypeScript at pinned
inputs in the smoke harness; offline and recorded, a grey ramp within one code of Blender's AgX
Base sRGB LUT, which settles the encoding near black. By hand, recorded: a Sun-like star in frame
with a lit planet, hues holding in the highlight. Acceptance: `just ci`, `just test-render`.

#### R07.T16 Symbology over the image, and the phrases

`photoreal/overlay.ts` (Design notes 16–17): R02's draw list with casing on in the photorealistic
style; rings and hulls as cased marks until R11; DOM readouts on `--surface-0` plates; the label
block gains the meter, the style and `BODY ALBEDO: NOT YET MODELLED`. Draft, for the owner, the
nomenclature entries this plan adds beyond R02's nine items (`PHOTOREALISTIC`, `METER AVG`,
`METER LIT`, `METER DARK`, `ONE PHOTOREALISTIC VIEW ON LOW SETTING`, the albedo phrase), as one
edit of `docs/frontend/ux-guidelines.md` that ends in the owner's sign-off. Tests: every overlay
mark over the image has a casing stroke; plates are present for every readout; the console-ux
skill's lint and contrast scripts pass. Acceptance: `just ci`; the guide edit is one commit for the
owner.

#### R07.T17 The low setting and benchmarks

Add every setting of Design note 18 to R05's `QualitySetting`; record the benchmarks of T12, T14
and T15 and the whole style's frame time on the UHD 620 at 720p on a quiet machine, and on a
discrete part of the RTX 4060 class when one is borrowed or rented (the brainstorm's Testing
section), under `--hyperion-gpu-timing`, in this plan as "as built" figures replacing the probes'
provisional ones; R12 consolidates them. Acceptance: `just ci`; the figures recorded with their
settings, flags, load and dates.

### Phase B: several views

#### R07.T18 The view budget and resolution controller

`budget/{viewBudget,resolutionController}.ts` (Design note 14). Tests: one photorealistic view on
the low setting and the reason for the refusal; secondary views at lower scale or 30 Hz; the
primary's scale drops while instruments are open; `PER_CANVAS_OVERHEAD_MS` enters the budget; the
controller converges on a synthetic load, holds within [0.5, 1.0], and does not oscillate under a
step (hysteresis); it is stable with inputs quantised to 65,536 ns; the frame interval is used when
timestamps are unavailable. Acceptance: `pnpm test`.

#### R07.T19 Instrument views in `VIEW`

`displays/view/InstrumentView.tsx`, `ViewDisplay.tsx` (Design note 15): two slots, each with its own
camera, style and target, R01's `createView`, R02's DOM list and label block, and the exposure
reading of the primary view. Tests (Vitest): each view focusable and named; keyboard reaches every
camera control in every view; the style control of a second view is disabled on low with its
reason; a wireframe instrument shows the source of its exposure. Acceptance: `pnpm test`,
`just ci`.

#### R07.T20 Several views, by hand

With the real styles on the UHD 620 on a quiet machine: a full-window photorealistic view and two
wireframe instruments, each the right way up, no GPU time in copies, a resize of one leaving the
others' attachments alone, the frame time with instruments open against the low setting's 33 ms
(brainstorm, Testing), and the per-canvas overhead that replaces `PER_CANVAS_OVERHEAD_MS`'s
provisional 0.3 ms. Recorded in this plan. Acceptance: the record.

#### R07.T21 A child window on a second monitor

R01.T13 records the child-window prototype, with the research probe as its first data. This task
adds what that probe could not: an instrument view in a same-origin child window on a second
monitor, recording whether Chromium on X11 paces it from that display's vsync, and its frame times
beside the main view's; and the client's release of the child's context on `pagehide`. Done when a
second display is available. Acceptance: the record.

### Phase C: the main screen (gated on the sessions plan)

Each task begins by fitting its names to the sessions plan as built.

#### R07.T22 The client role

`ClientRoleDto` and `role` on `Hello`, defaulted to `console`, so that `PROTOCOL_VERSION` stays 2
(brainstorm, Runtime and code shape); the server records the role per connection;
`--role main-screen` on the client's command line. Run `just gen-protocol`. Tests: wire forms; a
`Hello` without `role` is a console; the role reaches the connection. Acceptance: `just ci`.

#### R07.T23 The camera as ship state

`crates/hyperion-server/src/main_screen/{camera,grants}.rs` (Design note 20): state, grants with
their defaults, the selection commands through the sessions plan's command path with closed-loop
results, `selected_by`, persistence in `session.json`. Tests: default grants with and without a
Captain; an ungranted station is rejected with a reason; the last accepted selection stands; a
session saved and reopened restores the camera. Acceptance:
`cargo test -p hyperion-server main_screen`.

#### R07.T24 The free camera on the server

`main_screen/{integrator,track}.rs`: take and release through control arbitration, the Captain's
override, the integrator from the holder's input (sent on change and at the tick) at each tick, the
presentation track in the replay log. Tests: one holder at a time; the Captain takes from Helm;
releasing on disconnect; the integrator's pose for a pinned input log; a replay's bit-for-bit check
passes with the track present and ignores it. Acceptance: `cargo test -p hyperion-server
main_screen`.

#### R07.T25 The camera on the scene topic

R03's reserved `main_screen` field on `SceneStateDto` and `SceneNotificationDto`, with the pose's
rates, pushed on change and with every craft push while the camera is flown; the main screen's
extrapolation with its one-tick blend, and interpolation as the fallback; a wireframe camera mode
`SLAVED` on a station that follows it. Run `just gen-protocol`. Tests: R03's two-clients-agree test
extended to the camera; a slaved helm's projected positions equal the main screen's to a pixel at
one push; extrapolation and interpolation agree on a constant-rate flight and the blend is
continuous on a correction. Acceptance: `just ci`.

#### R07.T26 The station's control panel

`displays/view/MainScreenControl.tsx`: preset, target and style as ship commands, visually distinct
from display controls, showing `PENDING` within 100 ms and then the result; `TAKE` and `RELEASE` for
the free camera; the flyer's local predicted view of the camera (Design note 22); who selected and
who flies, always shown. Tests (Vitest, `FakeWebSocket`): the closed loop in each outcome;
`PENDING` shown on the same frame as the command; ungranted stations see the state and no commands;
keyboard for everything. Acceptance: `pnpm test`.

#### R07.T27 The main-screen client

`mainScreen/*` (Design notes 22–23, guide item 9): full screen, no console chrome, the view
photorealistic by default; a status line with the ship's name, labelled ship time and link state;
the mode banner; the alert annunciator (counts of active and unacknowledged emergency, warning and
caution alerts and the newest unacknowledged emergency or warning in full, under the guide's flash
and reverse-video rules, never a border or overlay) with text at least 30′; the label block with
who commands the camera; `roomTextSize` in device pixels; `DisplaySetup` with the diagonal
pre-filled by `main/displayEdid.ts`, the distance, the 100 mm check bar, the 2–10 ratio warning and
the game-mode statement; the DOM list kept; on link loss the last frame held, the time stale with
`S`, `NO CARRIER`. Tests: `roomTextSize` gives 36.7 px at 20′ and 55.0 px at 30′ for a 55″ 1080p
screen at 4 m, and the CSS size at a scale factor of 0.78125; EDID parsing of a pinned blob, and a
zero size rejected; the annunciator's counts exclude advisories; link loss; `prefers-reduced-motion`
has only flashing to reduce. Acceptance: `pnpm test`, `just ci`.

#### R07.T28 The loop, measured

An integration test from a station's input frame to the scene push that carries its pose, on
loopback, recording p50 and p95 against the server's share of Design note 22 (about 10 ms typical);
by hand, recorded, the time from a station key press to the main screen's photon on a wired LAN,
filmed at 240 frames a second, with extrapolation and with interpolation, the main screen's
animation-frame-to-photon time recorded separately and the display's own lag measured apart; the
flyer's local view's loop; a discrete selection's `PENDING` within 100 ms and completion within
250 ms. Acceptance: the test passes; the record.

## Verification

- **Photometry:** illuminance, magnitudes and phase against the Solar System table (T3, T4);
  planetshine against earthshine and the full Moon (T11); flux continuous across point, disc and
  mesh (T8, T9); a star identical in both styles (T15).
- **Geometry:** projected positions identical in both styles (T7); painter order against a
  ray-marched truth and occultations at the limb (T5, T9); oblate extents (T8); terminator and
  eclipse against the `f64` oracles (T6, T10).
- **Exposure:** metering continuous as a star enters frame; histograms equal bin for bin with and
  without subgroups (T12, T13); bloom conserves energy with truncation modelled (T14).
- **Views:** one photorealistic view on low; three canvases on one device with no copies (T18–T20).
- **Main screen:** grants, exclusivity and the Captain's override; saved with the session; replay
  ignores the track; two clients agree on the camera; the loop within 100 ms to the photon with a
  game-mode display (T22–T28).
- **By eye, recorded:** the phase, occultation and eclipse scenes, a lit planet on black, a star and
  planet in frame, the cockpit with instruments on the UHD 620, and the main screen across a room.
- **Benchmarks:** T12, T14, T15 and T17's timings on both GPUs on a quiet machine, handed to R12.

## Generator version

No change to generated output and no bump: the renderer reads. The photometric section and the
flattening are galaxy plan 14's and move its version when they land. On the wire everything is
additive: optional sections parsed by the client, a defaulted `role` on `Hello`, R03's reserved
optional field and new commands in the sessions plan's envelope, none of which moves
`PROTOCOL_VERSION` by the brainstorm's rule. The plan reserves nothing in the generator.

## Risks and open points

- **Provisional timings.** Every timing from the research probes (histogram, present overhead,
  quantisation's effect) was measured while other work loaded the machine; T17 and T20 re-measure
  on a quiet one before any figure becomes "as built".
- **Templates without an analogue** (airless ice, snowball, magma) are provisional and labelled;
  the q values of Jupiter and Neptune rest on phase curves extrapolated past their data (Mayorga et
  al. 2016 would settle Jupiter); and plan 14's airless-rock Bond albedo and Earth's albedo are
  checks for its owner (T1).
- **The lens PSF** of camera views rests on recalled veiling-glare figures (low confidence); the
  eye's CIE function is solid. The glare threshold at AgX's top of range and the smoothing speeds
  are settled by eye (T13, T14).
- **Planetshine's uniform-disc approximation** shifts its terminator on the receiver by the
  neighbour's crescent offset, up to about 4° at Io; stated, not corrected.
- **Gas giants' cloud bands** are left by R11 to R08 and this plan; this plan draws a uniform,
  oblate giant and no plan generates bands. An ask for a band structure (zonal winds, belt and zone
  albedos) belongs with plan 14 or R08 and is not yet made.
- **Magma oceans glow** above about 1,394 K by thermal emission, which is not reflection; no plan
  owns it (R08 or R10).
- **Oblateness beyond the image.** R02's graticule on a 10% oblate Saturn is off by up to 6,000 km,
  and collision is spherical; both are R02's and the flight model's to take up once plan 14 sends f.
- **Ordering against craft.** Promotion to a mesh settles depth for bodies against depth-writing
  geometry; craft are outlines until hull art exists, and when lit hulls arrive they join the
  promotion rule as depth-writing geometry.
- **The main screen is gated** on a plan that does not exist. Its task list is written against the
  brainstorm and will be re-fitted (T22–T28 each begin so); the sessions plan may choose a command
  envelope that reshapes `MainScreenCommand`, and must accept input sent on change.
- **Extrapolating the main screen's camera** departs from the brainstorm's interpolation (Design
  note 22); T28 compares both, and the owner may keep interpolation at the cost of about 16 ms.
- **A child window on a second monitor** is unproved until a second display is at hand (T21).
- **HDR output.** An `rgba16float` canvas with extended tone mapping configures on this machine, but
  the panel is not HDR; it would suit a main screen on an HDR television and is left open.
- **Asked by later plans, not yet designed here** (the roadmap's asks table carries each). R10:
  `body_brdf` fed per-texel lunar-Lambert parameters (A_N, L(α), f(α), with L(0) from A_N); the
  analytic disc sampling R10's coarse class-weights map (`classMap.ts`) over surveyed cells and the
  uniform `lawFor(p, q)` elsewhere; and the horizon term (`sphere_irradiance`) taking its local
  horizon from R10's horizon map on terrain (R10 Design notes 8 and 10). R12: the instrument
  panels' sizes in the cockpit layout, stable `PassList` labels for its `PASS_ROWS`, and the
  resolution controller's bounds as a setting value that its T11 may move.

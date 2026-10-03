# Plan R07: Lit bodies, the photorealistic style, several views and the main screen

- **Milestone:** Rendering milestone RM3 (R06–R07).
- **Depends on:**
  [R02 Real-scale foundations and the wireframe `VIEW`](02-real-scale-view-and-wireframe.md),
  [R03 The scene subscription and bulk transport](03-scene-subscription-and-transport.md),
  [R06 The sky](06-the-sky.md); through them
  [R01 Graphics platform and the engine adapter](01-graphics-platform-and-engine.md). It reads
  [R05 Terrain geometry and the descent spike](05-terrain-geometry-and-descent-spike.md) for the
  quadtree drawn as a smooth figure (Design note 3), its `QualitySetting` and its streaming
  priority (Design note 14). Galaxy plan 14
  ([Planetary systems](../galaxy-generation/14-planetary-systems.md)) for radii, rotation (P14.T14),
  and a photometric section and a flattening that it is asked to add (Design notes 5 and 19); galaxy
  plan 06, through R06, for each host's radius and absolute V. Phase
  C also depends on **the single-player sessions, ship state and closed-loop commands, for which no
  plan exists yet** (Design note 21). Phase C does not start until that plan is written and built.
  Re-validated at `ce7aeb3` (2026-10-02): still no sessions plan (only the brainstorm
  `single-player-experience.md`), so **Phase C (T22–T28) is out of scope for RM3**; RM3 is Phases A
  and B.
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
arithmetic mean luminance of a GPU histogram with the star's disc and its veil excluded, exposed
under `AUTO`, `MAN` or `INHIBITED`, given veiling glare for the light the display cannot
show, and tone-mapped by AgX in one full-screen pass, with a low setting built beside the high one.
Symbology over the image is cased. The single-player `VIEW` display holds a full-window
photorealistic view and up to two wireframe instrument views as canvases of one document on one
device, each with its own budget, and only one photorealistic view on the low setting. Once the
sessions exist (Phase C), a bridge has a main screen: a client role, a camera that is ship state
set by granted stations through closed-loop commands and flown from one station's input by the
server, about 60 ms from input to photon typically against a 100 ms requirement whose worst case
T28 measures, carried on the scene topic so that a helm can slave its
wireframe to it, and a chrome of status line, mode banner, alert annunciator and label block sized
for the room.

## Scope and non-goals

In scope:

- The regime selector for lit bodies and its painter order (Design notes 1–3).
- Illuminance at a body from each star, per display channel; the closed-form horizon term and the
  annulus eclipse term; planetshine (Design notes 4, 6, 7).
- The body BRDF and its disc-integrated phase function, as a TypeScript reference and a WGSL
  function that R10 and R11 call (Design note 5).
- The hooks later plans fill, each built with a default and tested on a synthetic input: a law per
  texel, a class-weights map on the disc and a local horizon (R10), a ring-shadow term (R11), and
  each sun's transmittance and the sky's irradiance through an atmosphere (R08) (Design note 24).
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

- The per-view canvas context, the engine adapter, WGSL compile-error reporting, kernel selection,
  the smoke harness and the child-window prototype itself (R01, R01.T13). This plan submits through
  them.
- The camera model, reversed-Z, the HDR target and pre-exposure, the exposure triple and its three
  automation levels, the AgX function `toneCurve` and its WGSL twin `agx`, the wireframe style,
  `VIEW`'s label block and DOM list, and the nine guide drafts (R02). This plan adds to them.
- The scene subscription, retarded and apparent positions, extrapolation and camera reports (R03).
- The sky, the star cubemap and sprites, the host stars' limb-darkened discs and the spectral
  colour table (R06). This plan lights bodies with R06's host discs and meters around them.
- Atmospheres, and the baked disc reflectance that replaces this plan's albedo-only shading of an
  atmosphere-bearing body (R08). Terrain, and the hand-over from disc to terrain (R10). Rings,
  their far annulus and their shadow on the body, clouds, oceans, glint, decoration and still
  images (R11). Until R11, rings are R02's wireframe ellipses drawn as cased symbology over the
  image (Design note 17), and the ring-shadow term is 1.
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

TypeScript paths are relative to `apps/hyperion/src/renderer/src/view/`, except those that begin
with `displays/`, `lib/`, `mainScreen/`, `test/` or `view/`, which are relative to
`apps/hyperion/src/renderer/src/`, and those that begin with `apps/`. Signatures are sketches,
named precisely enough to be grepped.

Names used below and defined elsewhere: `HostDiscDto` is R06's wire form of `sky::disc::HostDisc`
(its `SkyResponse.hosts`, with radius, per-channel mean luminance and power-2 coefficients, in the
channel order B, V, R for the display's b, g, r; R06.T10 fixes the field names);
`SceneFrameBody` is one body's entry of R03's `SceneFrame`, built as `SceneBodyFrame`
(`lib/scene/apparent.ts`), a union of `placed` (`geometricM`, `apparentM`, `emitted`, `lightTime`,
`level`, `hillRadiusM: number | null`) and `contact` (`apparentM`, `emitted` and `level` only);
`Viewport` is R02's (`view/camera/projection.ts`, `{ widthPx, heightPx }`), the field of view
being `ProjectionCamera.fovXRad`, so a sketch below that takes `camera: CameraPose` and a viewport
takes R02's `DrawCamera { pose, fovXRad }` with it; `ViewSpec` is one view's `ViewId`,
`CameraState` and canvas size; `ExposureTriple` is R02's, from `view/photometry/exposure.ts`, and
`Rgb` R02's from `view/photometry/toneCurve.ts` (not `lib/galaxy/ramp.ts`'s), the one per-channel
triple that R05, R07 and R08 share. `Vec3` is `renderer/src/geometry/vec3.ts`'s. Shaders follow R01
Design note 23: `@group(0)` `Frame` (`view/shaders/frame.wgsl`), `@group(1)` the draw's `Draw`
(`offsetFromCameraM`, then the spec's uniforms), `@group(2)` resources at declared bindings (a
post-process's input colour holds bindings 0 and 1, `POST_PROCESS_BINDINGS`), `vertexMain` and
`fragmentMain`; every material and post-process registered in `WGSL_CATALOGUE` carries a
`displayName` (upper case, at most three words, what it draws; compute kernels have none).

### Lighting (`lighting/`)

```ts
/** Illuminance at a point face-on to a host star, per display channel, lux (Design note 4). */
export function starIlluminance(disc: HostDiscDto, distanceM: number): Rgb;
/** A star under this fraction of the brightest's illuminance at a body does not light it. */
export const STAR_CUT_RELATIVE = 1e-4; // Design note 4; R08 applies the same constant
/** Irradiance factor from a uniform sphere of H = d ÷ R at angle φ to the normal (Howell),
 *  above a local horizon of elevation `horizonRad` (0 on the smooth figure; R10's on terrain). */
export function sphereIrradianceFactor(h: number, phiRad: number, horizonRad?: number): number;
export const DISC_ANNULI_HIGH = 4; // limb-darkened annuli, edges uniform in μ
export const DISC_ANNULI_LOW = 2;
/** Edges of K annuli uniform in μ for one channel's power-2 law I(μ)/I(1) = 1 − c(1 − μ^α),
 *  from `HostDiscDto`'s per-channel coefficients. */
export function annulusEdges(c: number, alpha: number, k: number): Float64Array;
export function circleOverlapArea(r: number, k: number, z: number): number; // exact
export function eclipseVisible(
  disc: HostDiscDto,
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
  discs: ReadonlyArray<HostDiscDto>,
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
  readonly template: PhaseTemplateId; // whose V curve f(α) is tabulated, clamped at 4
  readonly phaseExponent: Rgb; // s per channel in Φ_t(α)^s
}
export type PhaseTemplateId =
  | "moon"
  | "mercury"
  | "mars"
  | "venus"
  | "earth"
  | "jupiter"
  | "saturn"
  | "uranus"
  | "neptune"
  | "airless-ice"
  | "snowball"
  | "magma"; // the last three provisional and labelled
export interface PhaseTemplate {
  readonly phaseV: (alphaRad: number) => number; // Φ_t, Φ_t(0) = 1
  readonly validToRad: number; // held beyond, Design note 5
  readonly source: string; // the citation
}
export interface BodyPhotometry {
  readonly geometricAlbedo: Rgb;
  readonly phaseIntegral: Rgb; // q per channel, from the law
  readonly law: PhotometricLaw;
  readonly bondRatioCheck: number | null; // p_V q_V ÷ A_Bond as plan 14 states it; a check only
  readonly provenance: "modelled" | "provisional";
}
export const PROVISIONAL_PHOTOMETRY: BodyPhotometry; // until plan 14's section, labelled
export const PHASE_F_CLAMP = 4;
export function brdf(law: PhotometricLaw, mu0: number, mu: number, phaseRad: number): Rgb; // I/F
export function discIntegratedPhase(law: PhotometricLaw, phaseRad: number): Rgb; // Φ(α), Φ(0) = 1
export function lawFor(p: Rgb, q: Rgb, template: PhaseTemplateId): PhotometricLaw;
// Mallama and Hilton 2018
export const PHASE_TEMPLATES: Readonly<Record<PhaseTemplateId, PhaseTemplate>>;
/**
 * Design note 19: the reference spheroid, the datum every plan measures heights from. R05 defines
 * it first, in `view/terrain/planet.ts` (R05.T7.a), with this shape; this plan re-exports it and
 * defines no second one.
 */
export type { BodyFigure } from "../terrain/planet"; // { equatorialRadiusM; polarRadiusM; pole: Vec3 | null }
export type AppearanceLabel = "BODY ALBEDO: NOT YET MODELLED";
export interface BodyAppearance {
  // what R08 and R10 read per body; built by R07.T5
  readonly body: BodyIdHex;
  readonly figure: BodyFigure;
  readonly photometry: BodyPhotometry;
  readonly regime: LitRegime;
  readonly labels: ReadonlyArray<AppearanceLabel>;
}
/** What the disc shades with (Design note 24): the uniform law until R10 supplies a map. */
export type DiscSurface =
  | { readonly kind: "uniform"; readonly law: PhotometricLaw }
  | {
      readonly kind: "class-map";
      readonly weights: TextureHandle; // R10's coarse class weights, surveyed cells only
      readonly laws: ReadonlyArray<PhotometricLaw>; // one per class
      readonly elsewhere: PhotometricLaw; // unsurveyed texels: the uniform `lawFor(p, q)`
    };
```

`shaders/litBody.wgsl` exports `struct LunarLambert { a: vec3f, l: f32, s: vec3f, template: u32 }`
(a law that a caller may build per texel), `body_brdf(law, mu0, mu, alpha) -> vec3f`,
`sphere_irradiance(h, phi, horizon) -> f32` (horizon 0 on the smooth figure),
`eclipse_visible(...) -> f32`, and the lighting of a lit point, which calls
`ring_shadow_on_body(p, sun) -> vec3f` (a stub returning 1 until R11's `ringShadow.wgsl` replaces
it, and not called where R11's `ringShadowOnBody` setting is off) and R08's `surfaceLighting.wgsl`
(`atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad) -> vec3f` on
each sun's illuminance and `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad) -> vec3f`
added through the same law; stubs of 1 and 0 until R08.T9.b). The lit point passes its geodetic
height, its geodetic latitude (from the spheroid normal against `BodyFigure`'s pole) and each
sun's azimuth from local north, which R08 Design note 17 needs on an oblate body. The functions
are called by R10's `terrainLit.wgsl` with per-class parameters and R11's ring and ocean passes,
and all are registered in R01's `WGSL_CATALOGUE`.

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
export type PainterEntry =
  | { readonly kind: "body"; readonly body: BodyIdHex } // a disc or a point
  | { readonly kind: "host"; readonly star: number }; // R06's disc, as a sphere
export function painterOrder(
  bodies: ReadonlyArray<SceneFrameBody>,
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  hosts: ReadonlyArray<HostDiscDto>,
  camera: CameraPose,
): PainterEntry[]; // back to front by tangent power, meshes excluded, Design note 2
```

`shaders/bodyDisc.wgsl` (the analytic disc, spheroid by axis scaling), `shaders/bodyPoint.wgsl`
(through R02's `starSprite.wgsl` and `psfPixelWeights`), `bodies/smoothMesh.ts` (R05's quadtree on
the reference spheroid at zero height).

### The photorealistic style and its passes (`photoreal/`, `post/`)

```ts
// R02's `RenderStyle` gains its variant: "wireframe" | "photorealistic"
export function photorealisticPasses(setting: QualitySetting): PassList; // Design note 8
/**
 * This plan's pass labels, fixed across frames and releases, so that R12's `PASS_ROWS` keys on
 * them. Later plans' passes in their slots carry their own plans' labels. As R01 built
 * `onPassTimes`, a submission's main pass is timed under its `FrameSubmission.label`, a compute
 * dispatch under its `pass` argument (default `"compute"`), and each post-process of a submission
 * under `` `${frame.label} ${spec.name}` ``; each label below is therefore given as the
 * `FrameSubmission.label` or `dispatch` pass of its own submission, or the post-process `spec.name`
 * chosen so that the derived label is the one recorded in `PassList`.
 */
export const PHOTOREAL_PASS_LABELS = {
  bodies: "bodies", // mesh bodies, opaque with depth
  discs: "discs", // host discs, disc bodies and point bodies in painter order
  histogram: "histogram",
  bloom: "bloom", // one label for the whole chain
  tonemap: "tonemap",
  symbology: "symbology",
} as const;
export interface PassList {
  readonly passes: ReadonlyArray<{
    readonly label: string;
    readonly owner: "R06" | "R07" | "R08" | "R10" | "R11";
  }>; // in order
}
export type MeterMode = "average" | "lit" | "dark"; // Design note 10
/** The class each pass writes in the HDR target's alpha, an exact small integer (note 10).
 *  In `post/meter.ts` with `meterWeights`, the file R06.T13.e creates under this name if it lands
 *  first (then extended here, never redeclared); `GlareSource` likewise in `post/glare.ts`. */
export const METER_CLASS = { hostDisc: 0, other: 1, litBody: 2, unlitBody: 3 } as const;
export function meterWeights(mode: MeterMode): readonly [number, number, number, number];
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
export function meteredLuminance(h: Histogram, window: PercentileWindow): number; // no veil
export interface AutoExposure {
  step(h: Histogram | undefined, dtS: number): ExposureReading;
}
export interface ExposureReading {
  readonly ev100: number;
  /** R02's triple; under `AUTO` from Design note 11's program. R06's `cameraLimitV` reads it. */
  readonly triple: ExposureTriple; // R02's `{ aperture, shutterS, iso }`
  readonly control: ExposureControl;
  readonly meter: MeterMode;
  readonly source: ViewId;
} // R02 takes it as `AUTO`
/** R06's `HostDiscLayer.glareSources` returns these, one per disc; R11's glint is another. */
export interface GlareSource {
  readonly direction: Vec3;
  readonly angularRadiusRad: number;
  readonly excessLuminance: Rgb;
} // cd/m² above 65,504
/** sr⁻¹, Σ = 1; an eye view passes R06's `DEFAULT_EYE_OBSERVER` (age 25, pigmentation 0.5) as
 *  `eye`, so that T14.a builds and tests before R06.T13.e lands. */
export function glareSpread(
  role: ViewRole,
  thetaRad: number,
  eye: { readonly ageYears: number; readonly pigmentation: number },
): number;
export interface BloomKernel {
  readonly levels: number;
  readonly weights: Float32Array; // NNLS, non-negative, Σ = 1
}
export function bloomKernel(setting: QualitySetting, role: ViewRole, radPerPx: number): BloomKernel;
```

WGSL: `post/histogram.wgsl` (shared-memory atomics; a `KernelPair` with `readback: "bit-exact"`
and no subgroup twin unless R01's catalogue asks for one; run through `createComputeAsync` and
`dispatch(kernel, bindings, workgroups, "histogram")`, read back through R01's
`readBuffer(buffer, "cpu")`), `post/bloomDown.wgsl`,
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
  views: ReadonlyArray<ViewSpec>,
  setting: QualitySetting,
  candidate: ViewId,
): { allowed: true } | { allowed: false; reason: "ONE PHOTOREALISTIC VIEW ON LOW SETTING" };
export class ResolutionController {
  /** `bounds` is `ViewSettings.internalScaleBounds`, [0.5, 1.0] on both settings (note 14). */
  constructor(bounds: readonly [min: number, max: number]);
  update(gpuFrameMs: number | undefined, intervalMs: number): number;
} // renderScale
```

`displays/view/InstrumentView.tsx`, `displays/view/StyleControl.tsx`,
`displays/view/MeterControl.tsx` (beside R02's `ExposurePanel.tsx`, the `ExposureControl` panel as
built); R02's `ViewDisplay.tsx` gains the instrument slots. In Phase C
R02's `CameraPreset` gains `slaved`, shown `SLAVED` (R07.T25).

### Main screen (Phase C)

Protocol (`hyperion_protocol`, mirrored in `@hyperion/protocol`), all additive:

```rust
// envelope.rs
ClientMessage::Hello { client_version: String, #[serde(default)] role: ClientRoleDto,
    #[serde(default)] reduced_motion: bool }                     // Design note 20
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

The DTOs of `main_screen.rs` and `MainScreenCommand` are built by R07.T23, with `CameraPresetDto`,
`RenderStyleDto` and `TargetDto` as the wire forms of R02's `CameraPreset`, `RenderStyle` and
target, and `StationDto` the sessions plan's.

Server (`hyperion_server`): `main_screen::{MainScreenCamera, Grants, FreeCameraIntegrator,
PresentationTrack}`.

Client: `apps/hyperion/src/renderer/src/mainScreen/{MainScreenApp, MainScreenStatusLine,
ModeBanner, AlertAnnunciator, roomTextSize, DisplaySetup}.tsx|ts`,
`displays/view/MainScreenControl.tsx`, the CLI options `--role main-screen`,
`--screen-diagonal-in` and `--viewing-distance-m` in `apps/hyperion/src/main/cli.ts`, and
`apps/hyperion/src/main/displayEdid.ts` (the EDID pre-fill).

### Test helpers

`test/litFixtures.ts` (`aHostDisc` from R07.T3, `aLitBody` from R07.T5, `SOLAR_SYSTEM_PHOTOMETRY`
from R07.T4.a, the table of Design note 5), `lighting/oracle.ts`, and the kept scenes
`view/scenes/eclipseScene.ts` and `view/scenes/phaseScene.ts`.

## Consumes

Names are the owning plans' Provides as written on 2026-09-29; the owner is authoritative, and
where a name has changed by the time this plan runs, only the call sites here change.

- **R01:** `RenderEngine` (`createView` per canvas, `createMaterial`, `createPostProcess`,
  `createCompute`), `KernelPair` and `selectKernel`, `GpuCapabilities` (with `shaderF16`,
  `timestampQuery`, `rg11b10Renderable`), `styleAvailability` (the refusal of the photorealistic
  style on a fallback adapter), `GraphicsStatus`, `WGSL_CATALOGUE`, the smoke harness
  (`just test-render`) with its withheld-capability runs, `GPU_TIMING_SWITCH`
  (`--hyperion-gpu-timing`) for per-pass benchmarks, and R01.T13's child-window record; the HDR
  target and each bloom level from `createRenderTarget`, one target per level; pipelines through
  `createMaterialAsync`; per-pass GPU time from `onPassTimes`, keyed by `FrameSubmission.label`,
  which are this plan's `PassList` labels; and the timer's state, `GraphicsStatus.timer`, for the
  resolution controller (R01 Design notes 19 and 20). Asked of R01 and met there: each adapter's
  render-target rounding, `GraphicsStatus.targetRounding` (R01 Design note 22, R01.T8.j), which
  Design note 12 reads; and blend modes that all keep the destination alpha, `"additive"` and
  `"premultiplied"` (R01 Design note 21, R01.T8.i), so that every blended pass of R06, R08 and R11
  leaves `METER_CLASS` intact (Design note 10). _As built (`view/engine/types.ts`, re-checked at
  `ce7aeb3`):_ `createView(canvas, name)` takes a name; `createMaterialAsync(spec, targets,
meshes?)`; `createComputeAsync(pair)`; `dispatch(kernel, bindings, workgroups, pass?)`;
  `readBuffer(buffer, access?)` makes a new staging buffer per call (no ring, no partial range);
  `createRenderTarget({ name, size, format, mips, depth, category })` with `RenderTarget.colour`
  sampled by later passes (sampling a target while rendering into it throws `ColourSelfSample`);
  post-processes are full-screen draws, `PostProcessItem { postProcess, uniforms, textures? }`,
  chained through two `rgba16float` intermediates, input colour at `POST_PROCESS_BINDINGS` 0 and 1,
  and the last written to the canvas's sRGB view; `blend: "none" | "additive" | "premultiplied"`,
  `"none"` writing alpha and the other two keeping it (source zero, destination one);
  `GraphicsStatus.targetRounding` is per format (`rgba16float`, `rg11b10ufloat`); `timer` is
  `"quantized" | "full" | "absent"`; `styleAvailability(summary: AdapterSummary)`;
  `GPU_TIMING_SWITCH` lives in `apps/hyperion/src/preload/graphicsLaunch.ts` and is honoured only in
  the `vulkan` launch mode; the smoke harness's variants are `default` and `no-subgroups`
  (`smoke/page.ts`'s `VARIANTS`), and no override withholds `timestamp-query`; every material,
  target and kernel is re-created by its owner in `onRestored` after a device loss.
- **R02:** `view/coords` (`relativeToCamera`, `narrow`, `originMinusCamera`), `view/camera`
  (`CameraPose`, `CameraState`, `RenderStyle`, `ViewRole`, `ViewId`, `project`,
  `pixelSolidAngle`, `perspectiveReversedInfinite`), `view/depth` (`DEPTH_COMPARE`,
  `transparentLayerOrder`), `view/photometry` (`V0_ILLUMINANCE_LX`, `illuminanceLx`, `apparentV`,
  `pixelLuminance`, `psfPixelWeights`, `ev100FromTriple`, `ev100FromAverageLuminance`,
  `exposureScale`, `ExposureControl`, `ExposureTriple`, `Rgb`, `preExpose`, `HDR_COLOUR_FORMAT`
  with its alpha left to this plan, `toneCurve` and `agx` in `toneCurve.wgsl`, Filament's AgX
  port built once by R02.T10.c with its `NOTICE` entry, `DEFAULT_MAN_EV100`), `ViewScene`,
  `buildWireframeDrawList` with its casing width, `bodyRegime`, `ringEllipse`, `hullEdges`,
  `starSprite.wgsl`, `ViewDisplay`, `ViewCanvas`, `ViewMarkList`, `ViewLabelBlock`, the
  `ExposureControl` panel, `CameraControls`, and the nine guide drafts (R02.T2), items 2, 3, 4, 6
  and 9 above all; `CameraPreset`, a union left open to this plan's Phase C member `slaved`
  (R07.T25). _As built (re-checked at `ce7aeb3`):_ `Viewport` is `{ widthPx, heightPx }` and the
  field of view `ProjectionCamera.fovXRad` (`CameraState.fovDeg` in degrees); `graticule` is in
  `view/wireframe/bodies.ts`; `ExposureControl` is `manual { triple }` | `auto { ev100 }` |
  `inhibited { ev100, reason: "operator" | "no_image_to_meter" }`, driven by `setManual`, `setAuto`
  (refused with `no_image_to_meter` until a photorealistic view meters), `inhibit`, `enable` and
  `onMetering(control, ev100 | null)`, which this plan's meter feeds; the panel is `ExposurePanel`;
  `buildWireframeDrawList(scene, camera: DrawCamera, viewport, tokens, options: DrawOptions)` with
  `CASING_PX` = 1, lines `premultiplied`, screen symbology at depth 1; `agx`, `agxSigmoid` and
  `agxSprite` in `toneCurve.wgsl`; there is no client `BodyFixedRotation` type: a body's rotation is
  `CameraOrigins.bodyFixedRotation(body): Rotation3 | null` (`view/coords/position.ts`), `null`
  today since plan 14 sends no rotation; the sphere occluder's `SLOPE_SCALE` is 3 (a WGSL constant
  in `occluderSphere.wgsl`) while the hull faces' `occluder.wgsl` bias keeps `slopeScale: 2`.
- **R03:** `useScene` and `sceneAt` (`SceneFrame`, with each body's `geometricM`, `apparentM`,
  `emitted` and `hillRadiusM`), bodies as plan 14's `BodySummaryDto` inside R03's
  `SceneBodyDto` and `SceneSystemDto`, each with its granted `level`, `SceneClockDto`,
  `KinematicsDto`, `CameraReporter`, the two-clients-agree test, and the optional `main_screen`
  field its Design note 4 reserves. _As built (re-checked at `ce7aeb3`):_ `sceneAt(model, observer,
time, previous)` with `previous` required; a body's entry is `SceneBodyFrame`, and a `contact`
  entry has no `geometricM` or `hillRadiusM`; levels come per body through `SceneSystemDto.grants`
  (`BodyGrantDto { body, level, seen }`), not on each body; `main_screen` is room left by R03's
  Design note 4, not a field (Phase C adds it); the two-clients test is
  `crates/hyperion-server/tests/scene_agree.rs`.
- **R05:** `selectPatches`, `PlanetGeometry`, `PatchCache` and the patch geometry with per-patch
  `f64` origins; `QualitySetting` (`"high" | "low"`), `ViewSettings` and `SETTINGS`, to which this
  plan adds its settings as fields with their high and low values; the streaming
  priority of secondary views; `BodyFigure`, which this plan re-exports; and, asked of R05 and met
  there, a `PlanetGeometry` of a reference spheroid drawn at zero height with no height worker,
  `planetGeometry(figure, null)` (R05.T7.a; Design notes 3 and 19), which R07.T9 uses. _Not built
  at `ce7aeb3`_ (no `view/terrain/` or `view/quality/`); R05's Provides still match these names:
  `BodyFigure` and `planetGeometry` from R05.T7.a (`view/terrain/planet.ts`, `pole: Vec3 | null`),
  `QualitySetting`, `ViewSettings` and `SETTINGS` from R05.T7.b (`view/quality/`), the 0.25
  secondary-view weight from R05.T7.d, `PatchCache` from R05.T8, the patch's `originM` from
  R05.T10 and the terrain pass from R05.T11.
- **R06:** `HostDiscDto` on `SkyResponse.hosts`: its radius, `mean_luminance_cd_m2` per channel
  (the disc mean, which E = π L̄ sin²ρ uses, never `central_luminance_cd_m2`), the power-2
  coefficients per channel, and the host's `teff_k`, `log_g`, `chroma` and `lux_per_v0`, since the
  client holds no colour table; `angular_radius`, `HostDiscLayer`, `cameraLimitV(sensor, exposure:
ExposureTriple, fovDeg, backgroundCdM2)`, which this plan calls with its `ExposureReading.triple`;
  the default eye observer, `DEFAULT_EYE_OBSERVER` (R06 Design note 4: age 25, pigmentation 0.5).
  Met by R06.T13.e: the disc pass writes `METER_CLASS.hostDisc` (0) in the HDR target's alpha; the
  sprite and band passes blend alpha with source factor zero and destination one, so the meter
  class beneath them survives (Design note 10); and `HostDiscLayer.glareSources(camera, viewport):
GlareSource[]` returns one `GlareSource` per disc, `excessLuminance` per channel in cd/m² above
  65,504, in eye views also for a disc up to 45° outside the frame. Where this plan's `post/`
  module is absent, R06.T13.e declares `METER_CLASS` and `GlareSource` there under this plan's
  names, and this plan extends that file, so that R06 depends on nothing of R07 at build time.
  _Not built at `ce7aeb3`_ (no `view/sky/`, `view/post/` or `sky` protocol module). Providers:
  `HostDiscDto` R06.T10 (filled by R06.T11.c; its radius and coefficient field names are fixed
  there, not yet in R06's text), `cameraLimitV` and `DEFAULT_VIEW_CAMERA` (N = 1.4, t = 1/30 s,
  R06 Design note 18) R06.T13.a, `HostDiscLayer`, `glareSources`, `DEFAULT_EYE_OBSERVER` and the
  meter class R06.T13.e (`view/post/{meter,glare}.ts` if this plan's are absent). `angular_radius`
  is Rust only (`sky::disc`); the client computes asin(R ÷ d) itself (T3).
- **R08:** `DiscReflectanceTable`, baked by R08's `bakeDiscReflectance(medium, appearance:
BodyAppearance)`, for a body with an atmosphere, which replaces the albedo-only disc shading;
  R08.T16.b adds its read path to this plan's `shaders/bodyDisc.wgsl`. It reads `BodyAppearance`
  and `starIlluminance`. `surfaceLighting.wgsl`
  (R08.T9.b): `atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad)
-> vec3f` and `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad) -> vec3f`, which
  `litBody.wgsl`'s lighting applies to each sun and adds as sky light (Design note 24), at the lit
  point's geodetic latitude and each sun's azimuth from north (R08 Design note 17).
  `DiscReflectanceTable` is a 2D array texture, one layer a latitude band of the medium's slicing,
  read at each disc pixel's geodetic latitude (R08 Design note 17).
- **R10:** the disc-to-terrain hand-over by relief (its Design note 13), for which this plan's
  `mesh` regime is the smooth figure; the coarse class-weights map (`classMap.ts`) and per-class
  laws that fill `DiscSurface`'s `class-map` case, per-texel laws through `LunarLambert`, and its
  horizon map feeding `sphere_irradiance`'s horizon on terrain (Design note 24). R10 measures its
  heights above the reference spheroid of `BodyFigure` (Design note 19; the coordinator's ruling),
  as its `GroundPoint` and Design notes 12 and 14 state.
- **R11:** the far ring annulus and slab, which replace the cased ellipse; glint energy as a
  `GlareSource`; `ringShadow.wgsl`'s `ring_shadow_on_body`, which `litBody.wgsl` calls for a ringed
  body's disc and mesh regimes where R11's `ringShadowOnBody` is on (R11 Design note 14). R11's
  passes write `METER_CLASS` and keep the destination alpha where they blend (Design note 10).
- **Galaxy plan 14:** `BodySummaryDto` and its sections (`crates/hyperion-protocol/src/planetary/`);
  radius; the `SurfaceState` and Bond albedo of P14.T13.c once carried; `body_fixed_at` (P14.T14.c)
  through R02's `BodyFixedRotation`; the surface pressure and cloud fraction of `Atmosphere`
  (`planetary/derive/atmosphere.rs`) that choose the law (Design note 5); the photometric section
  and flattening asked for in R07.T1. _Re-checked on `main` at `ce7aeb3` (`GENERATOR_VERSION`
  19):_ none of R07.T1's asks is drafted in plan 14 or built — no photometry section, no
  flattening, no spheroid datum; `radius_m` on `BulkPropertiesDto` is the mean radius.
  `SurfaceState` (`atmosphere.rs:699`: `GasEnvelope`, `MagmaOcean`, `Airless`,
  `RunawayGreenhouse`, `Temperate`, `Snowball`), `Atmosphere::surface_pressure` and
  `cloud_fraction` exist in the sim only and are read by plan 14's section, never by the client;
  the Bond albedo is iterated from a 0.3 seed through `SurfaceState::albedo` (a provisional table)
  and `Atmosphere::albedo` and reaches the wire only inside `equilibrium_temperature_k`;
  `BodySurfaceDto` is still empty. Spin, locking and body-fixed frames are built sim-side
  (`planetary/derive/rotation.rs`, `planetary/frames.rs::body_fixed_at`,
  `PlanetarySystem::rotation_of`) but reach no DTO, so the client's `bodyFixedRotation` is `null`.
  Locking's moments of inertia are per class already (`params.rs`: rocky 0.33, icy 0.34,
  sub-Neptune and ice giant 0.23, gas giant 0.25, provisional; `PlanetClass` has five classes and no
  Saturn-like split).
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
  Still so at `ce7aeb3`: `Hello { client_version }` (`envelope.rs:41`), `PROTOCOL_VERSION` 2
  (`lib.rs:108`), one ship stand-in per open universe, and no sessions plan in
  `docs/agent/plans/`.

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
Re-checked at `ce7aeb3` (RM1 merged): R01's engine, R02's `view/` and wireframe `VIEW` and R03's
scene exist as the Consumes' "as built" notes give them; `lib/displays.ts` gains `view`; R05's and
R06's modules and everything of plan 14 named above as absent are still absent, save rotation,
which exists sim-side only; `PROTOCOL_VERSION` is still 2 and the command line still `--address`
and `--port`.

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
   when its power, |c_A|² − r_A² (the squared tangent length), is the smaller. So every analytic
   sphere, a disc body, a point body and each host star's disc (R06's, a sphere of the star's
   radius), is drawn back to front by power in one sequence, computed in `f64`, which is exact for
   disjoint spheres, with no depth write; an oblate body is ordered on its equatorial bounding
   sphere. A planet behind its star, as at superior conjunction seen from another planet, is then
   covered by the star's disc, and a moon in front of the star covers it. A disc whose screen
   footprint overlaps any geometry that writes depth — a mesh body, R10's terrain, a craft — is
   promoted to `mesh` for that frame, so that the depth test decides there. A point is drawn at its
   place in the sequence, so a point behind a disc is covered by it; its depth error is below its
   sub-pixel radius.
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
   light the body; a star whose illuminance at the body is under 10⁻⁴ of the brightest is dropped
   (`STAR_CUT_RELATIVE`, owned here and applied by R08 to its suns), so that both plans agree on
   which suns shine.
5. **The BRDF is a lunar-Lambert disc times a fitted phase function** (researched 2026-09-29;
   Mallama, Krobusek and Pavlov 2017, Icarus 282, 19, Table 7; Mallama and Hilton 2018, Astronomy
   and Computing 25, 10, arXiv:1808.01973, eqs. 2–17; McEwen 1991, Icarus 92, 298, from memory;
   added by decision-phase-curves, 2026-10-02: Krisciunas and Schaefer 1991, PASP 103, 1033, eq.
   9, after Allen 1973, _Astrophysical Quantities_, 3rd ed., p. 143, for the Moon's curve; Squyres
   and Veverka 1981, Icarus 46, 137, and 1982, Icarus 52, and Buratti 1991, Icarus 92, 312, for
   Ganymede's q; Grundy et al. 2007, Science 318, 234, for Europa's; Buratti 1984, Icarus 59, 392,
   for L at high albedo).
   Every measured phase integral in V lies between 0.48 (Mercury) and 1.36 (Saturn), below
   Lambert's 1.5 and Lommel–Seeliger's 1.64, so no law of fixed disc-integrated shape reaches p and
   q together; the law needs a free phase function. It is I/F = A · f(α) · [L · 2μ₀ ÷ (μ₀ + μ) +
   (1 − L) μ₀] with f(0) = 1, whose geometric albedo is p = A [L + ⅔(1 − L)] in closed form, and
   whose disc-integrated phase is Φ(α) = f(α) Φ_shape(α; L), with Lambert's and Lommel–Seeliger's
   Φ in closed form inside Φ_shape. Per surface state, f is fitted to a Solar System analogue's V
   phase curve Φ_t, so f = Φ_t ÷ Φ_shape reproduces the measured curve; another q is reached by
   Φ_t^s, s solved by bisection since q falls monotonically in s, per channel: s_B, s_V and s_R
   apply to the one V template, so that q differs by channel and a crescent reddens as Mercury's,
   the Moon's and Mars's do (B − V grows by some 0.1–0.2 mag from opposition to about 100°); where
   the analogue has only a V curve the three are equal. L and the template are chosen from
   plan 14's surface state, and within a state from its surface pressure P (decision-phase-curves,
   2026-10-02, replacing the rule researched 2026-09-29 on Mallama et al. 2017's Mars and Earth
   curves and McEwen 1991 for Mars's L; the Rayleigh optical depth at 550 nm scales with the column
   P ÷ g, Earth's about 0.097, so below about 30 kPa the gas cannot hide the ground), in this
   order: a gas envelope, the giant of its class, L = 0; a runaway greenhouse, Venus's, L = 0; a
   magma ocean, `magma` (Mercury's curve, L = 1, provisional) under 30 kPa and Venus's at or above
   it; a snowball, `snowball` (the Moon's curve, L = 1, provisional, q solved to Europa's 1.01);
   airless rock, Mercury's, L = 1 (the Moon's kept as a test analogue); airless ice, `airless-ice` (the Moon's curve,
   L = 1, provisional, q solved to Ganymede's 0.80); a temperate world under 30 kPa the Mars
   template with L = 0.5, otherwise Earth's, L = 0. The cloud condition (c under 0.3) is suspended
   while plan 14's cloud fraction is a constant of the state, and restored when it depends on the
   condensables. q does not depend on L (Φ = Φ_t^s unless the clamp acts), so L sets only the
   resolved disc's limb darkening. If steps between neighbouring worlds look wrong in the
   population, L = (1 − c) L_surf(P), log-interpolated through 1 at 100 Pa, 0.5 at 30 kPa and 0 at
   100 kPa, replaces them. The Mars curve covers
   phases to about 50° only, beyond which the clamp and hold carry it. f is clamped at 4 and held
   past the template's valid range (a Lambert crescent vanishes faster than a cloudy one: Venus's f
   would reach 61 at 170°), and the point regime integrates the same clamped law, so flux is
   continuous at 3 px by construction. In the shader it is one division, one multiply-add and a 1D
   texture fetch; Hapke has parameters p and q do not constrain and stays R11's for rings. The
   disc-integrated flux is F = E★ p (R ÷ Δ)² Φ(α). The values come from plan 14 (R07.T1), which is
   asked for p in B, V and R, a phase template and s per channel, L, and the stated ratio p_V q_V ÷
   A_Bond, never p derived from the Bond albedo through a fixed q: the ratio is physical, about 1
   for grey regolith, 0.69 for red Mars and about 2 for the giants, dark in the near infrared. The
   template and s fix q; the ratio is carried as a check (`bondRatioCheck`), and a body whose q_V
   from its law differs from the ratio's by more than 5% is a finding for plan 14's owner, not an
   input the client reconciles. Until the
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
   R06's sky; mesh bodies (opaque, depth); host discs, disc bodies and point bodies in one painter
   order (Design note 2); R08's,
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
   after Wrensch's minimal AgX (iolite-engine), no look, its header kept and a `NOTICE` entry, all
   built once by R02.T10.c (R02 Design note 12). EaryChow's and Sobotka's versions and Wrensch's
   own code carry no licence and are not ported; Blender's shipped LUT is an offline test oracle
   only. The curve's ceiling is 0.961 and `agx(0)` is 2.53 × 10⁻⁶; R02's sprites subtract `agx(0)`
   and this plan's full-screen pass need not, a difference below one code. The metered average lands
   at qK ÷ 78 = 0.104 of AgX's input, 0.79 stop below its 0.18 pivot, which is realistic for a K of
   12.5 (about 12% reflectance); no hidden bias is added, and exposure compensation stays the
   operator's, which answers R02's question.
10. **Metering reads a class from the HDR target's alpha** (researched 2026-09-29; bruop.github.io
    /exposure; probes on the UHD 620, provisional under load). Colour has no alpha use, so each
    opaque pass writes a meter class there as an exact small integer: `METER_CLASS.hostDisc` (0)
    for a host's disc (R06.T13.e's meter weight 0), so that the exposure does not step when the star
    enters frame (brainstorm, Exposure and tone mapping); `litBody` (2) and `unlitBody` (3) for a
    body's pixels on its lit and unlit side; `other` (1) for everything else. Translucent passes
    that blend into the target (R08's aerial perspective, R11's clouds) use an alpha blend of source
    factor zero and destination factor one, so they keep the class beneath them. The histogram
    kernel maps class to an integer weight through `meterWeights(mode)`: `AVG` weighs every class
    but the host disc, `LIT` only lit bodies and `DARK` only unlit ones, so no other plan's pass
    needs to know the operator's meter. The histogram has 256 bins over log₂ of the pre-exposed
    value from −14 to +16, the range `rgba16float` can hold; bin 0 takes everything below 2⁻¹⁴,
    zeros included, since a black pixel is real (no) light to a meter, and counts as luminance 0 in
    the mean; the others are 0.118 stop wide. The kernel accumulates in workgroup memory with
    atomics and adds each non-empty bin once to the global histogram; per-pixel global atomics are
    forbidden, being 20–60 times slower under a dark sky, where every pixel lands in one bin (16.6
    ms against 0.3–0.8 ms at 640 × 360). Integer adds make any reduction order bit-exact, as
    anything read back must be (brainstorm, "The graphics API, and the Intel problem"); a subgroup
    pre-reduction saved only 10–15% on uniform input and is not built unless R01's catalogue
    requires the pair. The histogram is read back asynchronously, one to three frames late, and
    exposure is computed on the CPU in TypeScript, so the displayed value is exactly the applied one
    and the controller is unit-tested without a GPU. The next frame's pre-exposure uses the latest
    applied exposure. With the window [0, 1] of Design note 11 the metered value is a quantised
    weighted mean: the extreme value the brainstorm's histogram guards against, the star's disc, is
    removed by its class, not by the histogram, which is kept for the operator's meters and any
    later clip.
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
    documented structure: linear at 3 EV/s when the scene brightens and 1 EV/s when it darkens while
    |ΔEV| exceeds 1.5, exponential inside that band (the speeds from memory, to be settled by eye).
    No dark adaptation of the eye is modelled. `MAN` takes R02's triple. Under `AUTO` the reading
    also carries a triple, for R06's `cameraLimitV`, by a sensitivity-priority program: the aperture
    and shutter stay at R06's `DEFAULT_VIEW_CAMERA` (N = 1.4, t = 1/30 s) and the sensitivity is
    solved from R02's EV100 = log₂(N² ÷ t) − log₂(S ÷ 100). `INHIBITED` freezes the last value.
    Exposure adaptation is not motion, so `prefers-reduced-motion` leaves it alone. A wireframe
    instrument view takes the `ExposureReading` of the photorealistic view it accompanies and shows
    which.
12. **Glare is the CIE glare spread function, applied to the light the display cannot show**
    (researched 2026-09-29; Vos and van den Berg 1999, CIE 135/1999, as reproduced in McCann and
    Vonikakis 2018, Front. Psychol. 8:2079, eq. 2). For an eye view (R02's `ViewRole` `"eye"`) the
    PSF is the CIE function for R06's default eye observer, age 25 and pigmentation 0.5, one
    observer for the sky's glare and the image's (age enters only the wide-angle term, through (A ÷
    62.5)⁴, so the choice moves the renormalised core by well under 1%), renormalised to one; its
    first angular constant is 0.0046°, not the 0.046° the open copies print, which integrates to
    about 37 instead of 1. It puts 58% of its energy beyond one pixel at 1080p across 60°, and the
    player's eye already supplies glare for what the display reproduces, so only L_high = L − min(L,
    T) is bloomed, T being AgX's top of range, about 157 L̄: energy is still conserved. A camera
    view's PSF is a small core and a Harvey-type tail holding 2–4% of the energy beyond a few pixels
    (from memory, after ISO 9358's veiling glare index; low confidence). The mip chain is Jimenez
    2014's 13-tap down and tent up without the Karis average, which does not conserve energy; its
    level weights are a non-negative least-squares fit of the chain's impulse response to the PSF's
    encircled energy at 2^k px, computed per view from its angular pixel scale. A clamped disc's
    excess cannot live in any `rgba16float` level (an O star's is some 10⁹ times the format's
    maximum), so each `GlareSource` is evaluated in closed form in the tone-mapping pass in `f32`;
    R06's discs and R11's glint use the same entry; for eye views R06 also supplies sources up to
    about 45° outside the frame, whose glare reaches into it. The veil is drawn but not metered
    (researched 2026-09-29, by numerical integration of the CIE function above): with the Sun at 1
    au in a 60° × 34° frame its frame-mean veil is about 3.6 × 10⁴ cd/m², and 15° outside the frame
    still about 750, against 50–3,000 cd/m² that a sunlit planet covering 5–30% of the frame adds to
    the mean. Metering the veil would drop the exposure 4–7 stops and put the planet 2–5 stops below
    the average, the "everything else goes black" the brainstorm's exclusion of the disc prevents;
    for an eye view it would also count glare twice, since the display shows the veil and the
    player's eye adapts to what it shows, which is this note's own reason for blooming only L_high.
    Colour-attachment writes on Gen9 round toward zero (probed), a −0.78% to −1.56% bias per write
    in `rg11b10ufloat` and −0.05% in `rgba16float`, so the bloom chain stays in `rgba16float` unless
    R01's probe, `GraphicsStatus.targetRounding`, reports `nearest` for `rg11b10ufloat`, and the CPU
    twin models truncation. As R01 built and ran the probe, the RTX 3080 also reads `toward-zero`
    for both formats (SwiftShader `nearest` for `rgba16float`), so truncation is not Gen9's alone
    and the chain stays `rgba16float` on the recommended machine too. The chain uses no random
    sampling, so the guide's flash limit holds.
13. **The tone-mapping pass encodes and dithers** (researched 2026-09-29; probes on the UHD 620).
    AgX's formed image spans 16.5 stops, −10 to +6.5 about 0.18, about 9.2 below and 7.3 above the
    metered average; the brainstorm's "roughly 25 stops" is AgX Log's encoding. The canvas is
    `rgba8unorm`, `getPreferredCanvasFormat()` on the probed UHD 620, written through a non-sRGB
    view with the encoding in the pass, so that the dither is applied in the encoded domain:
    triangular (TPDF) noise of ±1 LSB from a static blue-noise tile, never animated. Whether
    Filament's `pow(v, 2.2)` then the sRGB curve or the sigmoid's output written directly is right
    near black is settled by a dark ramp against Blender's AgX Base sRGB to one code (R07.T15), and
    whichever is chosen applies to R02's wireframe sprites too, so that an isolated star on black is
    identical in both styles before the dither and within one code after it. The pass also upscales
    from the view's internal resolution. As R01 built the view, the canvas takes
    `getPreferredCanvasFormat()` (`rgba8unorm` or `bgra8unorm`) with only its `-srgb` view format,
    and a submission's last post-process writes through that sRGB view; so T15 adds to R01's
    engine the choice of writing the last post-process through the canvas's own non-sRGB view
    (`view/engine/`, run under `just test-render`), the wireframe keeping the sRGB view.
14. **Per-view budgets are a pure policy** (researched 2026-09-29; probes on the UHD 620,
    provisional under load). The primary view renders at its internal scale; each secondary view at
    a lower scale or 30 Hz; on the low setting at most one view is photorealistic, and the style
    control of any other view is disabled with the reason (brainstorm, "Several views in one
    client"). While instruments are open the photorealistic view's internal scale drops, and a
    controller holds it between 0.5 and 1.0, the setting value `ViewSettings.internalScaleBounds`
    that R12's T11 may move, from the GPU frame time, from R01's `onPassTimes`
    where `GraphicsStatus.timer` is not `"absent"` and the frame interval otherwise. Under the
    forced switches timestamps are quantised to 65,536 ns, ±0.4% of a 16 ms frame, which the
    controller bears; per-pass benchmarks run under R01's `--hyperion-gpu-timing`, never
    `--enable-unsafe-webgpu`, which also lifts the quantisation but exposes experimental features.
    Presenting two extra small canvases cost below the probe's resolution, so
    `PER_CANVAS_OVERHEAD_MS` starts at 0.3 ms, flagged provisional and measured in R07.T20. A 4K
    main screen renders below native through the same scale. Secondary views stream terrain at lower
    priority (R05). The policy's outputs are data that R12's runs record.
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
    0.3–0.8 ms, is inside its row, provisionally, since it was measured under shared load. Each
    target is a finding if missed, not a failure.
19. **Bodies are drawn oblate, on one datum** (researched 2026-09-29; flattenings from NASA's fact
    sheets; the Darwin–Radau relation, from memory after Murray and Dermott 1999, checked against
    six planets).
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
    sends f, bodies are spheres. The spheroid is the one datum for the body (researched 2026-09-29;
    WGS 84's a − c = 21.385 km against a geoid within about ±106 m of the ellipsoid, EGM2008;
    confidence high): R10's terrain heights and its unsurveyed ground are asked to stand on it,
    measured along its normal, rather than on a sphere of the volumetric radius, which would put an
    Earth's sea level 7 km high at the equator and 14 km low at the poles and open a seam of up to
    21 km between terrain and this plan's smooth figure. R02's graticule stays spherical, labelled
    as geocentric latitude, and collision stays R02's and the flight model's (Risks).
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
    cuts between presets. Under the main-screen machine's `prefers-reduced-motion`, sent as
    `reduced_motion` in its `Hello`, the non-physical motion the guide's item 8 removes goes: the
    integrator maps the holder's input to rates with no acceleration ramps or damping, and the main
    screen applies a correction at once rather than blending it over a tick. A local view's camera
    stays a display control reported to R03's `CameraReporter`.
21. **The main screen waits on sessions, and says so.** Phase C needs what the sessions plan will
    own (Consumes). Nothing in Phase C is built as a stub before it exists, so as not to guess the
    shape of stations, commands and arbitration. Phases A and B need no session: a view without a
    ship runs on R03's ship stand-in.
22. **The loop is budgeted at 100 ms from input to photon, display included** (researched
    2026-09-29; MIL-STD-1472H §5.12.1.4.1.1, read; RTINGS's input-lag method). The standard counts
    100 ms (20–50 ms preferred) round trip to the display of the result, so the brainstorm's
    "excluding the display device" is too generous. The breakdown, keyboard input, typical and
    worst: USB poll 4 and 8 ms; LAN 1 and 2; the server's wait for the tick 7.8 and 15.6; integrate
    and push 1 and 2; the main screen's wait for its animation frame 8.3 and 16.7; render, composite
    and present 25 and 33.4; scanout to the screen's centre 8.3; a game-mode television 1–7 and 15.
    Interpolating a tick behind would add 15.6 ms and sending input only at the tick another
    7.8–15.6. So the station sends input on change as well as at the tick, and the main screen
    extrapolates the pose from the last pose and its angular and linear rates, blending corrections
    over one tick: about 60 ms typical and 102 ms worst with a game-mode display, the worst 16.7 ms
    more for a gamepad (about 119 ms), which the Gamepad API polls only on animation frames. The
    requirement is therefore met typically and at the 95th percentile T28 records, not in the worst
    case, which T28 records as a finding against 100 ms (Risks). This departs from the brainstorm's
    "the main screen interpolates between poses" (notes), is safe because the camera is presentation
    only, and keeps interpolation as the fallback R07.T28 compares. Only the flyer has a control
    loop, so the flying station also draws a local predicted view of the camera, integrated from its
    own input at display rate, whose loop is some 30–50 ms. Since the camera is ship state and the
    guide forbids showing it as if the ship had obeyed, that view is labelled `PREDICTED` in its
    label block, a phrase drafted for the owner in R07.T26. A discrete command shows `PENDING`
    within 100 ms (MIL-STD-1472H §5.1.2.1.6.4, Table V) and completes within 250 ms.
23. **Main-screen text is sized for the room** (researched 2026-09-29; MIL-STD-1472H, read). Its
    text is colour-coded, so it follows §5.17.25.14: at least 20′ at the longest anticipated viewing
    distance, above §5.17.18.2's 10′ "shall" and 15′ "should", measured from the top of the capitals
    to the bottom of the descenders (§3.2.28). The alert annunciator's text is at least 30′ and the
    newest emergency up to 60′ (§5.7.3.6, warning and caution signals). The height in device pixels
    is 2 D tan(θ ÷ 2) × (width in pixels ÷ W), divided by `devicePixelRatio` for CSS (the probed UHD
    620 laptop runs at 0.78125, and so does the development machine, at `Xft.dpi` 75); at 4 m on a
    55″ 1080p television that is 36.7 px at 20′ and 55.0 px at 30′. No web API gives a display's
    physical size, so the main-screen machine holds two settings, the screen diagonal and the
    furthest viewing distance, the diagonal pre-filled from EDID where it is plausible (non-zero and
    within 5% of the pixel aspect; Electron's main process reads `/sys/class/drm/card*-*/edid` on
    Linux, matching a connector to Electron's display by the EDID's monitor name against
    `Display.label` and its native mode against the display's size, with no pre-fill when the match
    is ambiguous), with an on-screen 100 mm bar to check it. EDID is unreliable for televisions and
    zero for projectors. The main screen takes no input in use (guide item 9), so the two settings
    are given at installation, as the command-line options `--screen-diagonal-in` and
    `--viewing-distance-m` or through `DisplaySetup` with a keyboard attached for the purpose, and
    saved on that machine. The setup warns when distance ÷ diagonal falls outside 2–10 (§5.2.2.12.3)
    and states that a television must be in game or PC mode, since outside it input lag reaches
    40–120 ms. The guide's rem scale does not apply.
24. **Later plans' inputs have hooks here, each with a default.** R10 and R11 run after this plan,
    so the shading takes their inputs through interfaces built and tested now on synthetic data.
    `body_brdf` takes a `LunarLambert` struct rather than a per-body uniform, so R10 can build one
    per texel (A_N, L(α) and f(α), with L(0) from A_N, its Design note 8). The disc shades from a
    `DiscSurface`: the uniform law until R10 supplies its coarse class-weights map over surveyed
    cells, with per-class laws, and the uniform `lawFor(p, q)` elsewhere. `sphere_irradiance` takes
    a local horizon elevation, 0 on the smooth figure and R10's horizon map on terrain (its Design
    note 10). `ring_shadow_on_body` is a stub returning 1 until R11's `ringShadow.wgsl` supplies
    the ring's shadow on the body (R11 Design note 14), and R08's `atmosphere_sun_transmittance`
    and `atmosphere_sky_irradiance` are stubs of 1 and 0 until R08.T9.b supplies them. Each default
    is the behaviour without the later plan, so nothing is drawn differently until it lands. R07
    creates the signatures (T4.c, T6.a, T6.c, T8.b); R10.T10.b, R10.T10.d and R10.T8.b complete the
    per-texel law, the class map and the local horizon inside this plan's files, keeping them.

## Tasks

Phase A builds the photorealistic style for one view, Phase B several views, Phase C the main
screen. T1 and T3 can start at once; T2.a follows T4.b (`lawFor`), and T2.b waits on plan 14's section. T4 and
T6 can run beside T3. T5 needs T2.a and T4.a. T7 needs T3–T5. T8.a needs T6 and T7; T8.b, T9, T10
and T11 follow T8.a. T12–T15 follow T7 and are independent of T8–T11, with T14.a before T13.a
(T13 reads no veil but tests against T14.a's glare sources) and T13.a before T16. T16 follows T8.a
and T13.a; T17 closes Phase A. Phase B follows T7 and T13. Phase C waits on the sessions plan.
TypeScript paths follow the rule at the head of Provides. Every task that adds or changes a shader
registered in `WGSL_CATALOGUE`, or anything under `view/engine/`, runs `just test-render` in its
acceptance, and gives each material and post-process a `displayName`.

Waits on R05 and R06 (re-validated at `ce7aeb3`, neither built): T2.a on R05.T7.a (`BodyFigure`);
T3 on R06.T10 (`HostDiscDto`'s field names); T5 on T2.a and R06.T10; T7 on R05.T7.b
(`QualitySetting`); T8.a on T3, T7 and R06.T13.e (the host-disc pass it is ordered with); T9 on
R05.T7.a–b, T8 and T11; T10 on T9 and R06.T13.e; T11 on T8.a; T14.b's injected sources on
R06.T13.e's `glareSources` (synthetic sources in its tests until then); T17 on R05.T7.b; T18 on
R05.T7.b; T19 on R05.T7.d. Free of both, and startable now: T1, T4.a–c, T6.a–c, T12, T13.a (the
`AUTO` program's aperture and shutter a constructor argument, set from R06's `DEFAULT_VIEW_CAMERA`
where the controller is made, in T13.b), T13.b, T14.a (the eye observer an argument), T15 and T16's
guide draft. T2.b waits on plan 14 (T1). T20 and T21 are by hand for the owner. Every timing below that
comes from the research probes was measured under shared load and is provisional until re-measured
on a quiet machine.

### Phase A: lit bodies and the photorealistic style

#### R07.T1 The photometry and figure asks of galaxy plan 14

Draft, in `docs/agent/plans/galaxy-generation/14-planetary-systems.md`, a subtask beside P14.T24 for
a `photometry` section of `BodySummaryDto` at `Bulk` detail (Design note 5): `geometric_albedo` in
B, V and R, a phase-curve template and its exponents s_B, s_V and s_R, the lunar-Lambert share L,
and the stated ratio p_V q_V ÷ A_Bond; the template and L chosen from surface pressure and cloud
fraction by Design note 5's rule as decision-phase-curves (2026-10-02) restates it (the surface
state first; 30 kPa for the magma and Mars branches; airless ice and snowball on the Moon's curve
with q solved to Ganymede's 0.80 and Europa's 1.01; magma the thin branch only), p = the
analogue's p × A_Bond ÷ the generator's albedo for the analogue (Europa's measured 0.68 for the snowball; decision-p14-phase-j, 9), capped so that p q ≤ 1; its tests reproduce Mallama et al. 2017's
Table 7 and the computed q to 0.5% and state each analogue's ratio. Add two checks for plan 14's
owner: airless rock's Bond albedo of 0.11 against the Moon's p_V 0.12 and a Mercury-like q of 0.48,
which give 0.06 (Lane and Irvine 1973 to be read); and Earth's p_V 0.434 with Tinetti's curve, which
gives A_V 0.57 against a Bond albedo of 0.294. Draft beside it the flattening of Design note 19 (f,
or equatorial and polar radii) with its per-class moment of inertia, which also replaces P14.T14.b's
"I = 0.33–0.4 M R² by class" so that one moment of inertia serves locking and flattening, with the
classes (rocky, Jupiter-like, Saturn-like, ice giant) defined by a criterion on plan 14's
composition classes for its owner to set, and the six-planet check; and the spheroid as the
reference figure that heights are measured from (Design note 19). Both subtasks are plan 14's to
build; the edit ends in plan 14's owner accepting them. Draft them against plan 14 as built
(re-validated at `ce7aeb3`, `GENERATOR_VERSION` 19): the law's inputs are the sim's `SurfaceState`
and `Atmosphere::{surface_pressure, cloud_fraction}` (`planetary/derive/atmosphere.rs`) and its
iterated Bond albedo, none of them on the wire, so the section is computed server-side and is
the only photometric thing the client reads; P14.T14.b's moments of inertia are already per
class as built (`params.rs`: rocky 0.33, icy 0.34, sub-Neptune and ice giant 0.23, gas giant 0.25,
provisional), so the flattening draft reuses those constants and asks only for what they lack
(a Saturn-like 0.21 inside `GasGiant`, by a criterion for the owner, and the six-planet check);
and, since rotation reaches no DTO yet, the drafts name the pole and rotation on the wire (the
roadmap's existing ask of P14.T14 through P14.T35) as the flattening's companion, without which an
oblate body has no axis. Both are a `GENERATOR_VERSION` bump of plan 14 and an additive protocol
change (`just gen-protocol`), plan 14's to make. Acceptance: the drafted subtasks cite the sources
above; `pnpm exec prettier --check` on both plan files. Decided 2026-10-02 (item 5): the body-fixed
frame (pole, W₀, rate and the locking time and post-lock rate, as `body_fixed_at` evaluates them)
goes on the wire in the same plan 14 subtask set as the flattening, under one `GENERATOR_VERSION`
bump (driven by the moment-of-inertia change) and one additive protocol change; the draft states
the detail level that grants it (`bulk`, the level that grants `bulk.radius_m`, since a disc needs
both). The client twin of `body_fixed_at` matches a Rust fixture to 10⁻⁹ rad at five times either
side of the locking time. _Drafted (2026-10-02):_ plan 14's Phase J, P14.T46 (moment of inertia,
rotation, flattening, datum, wire, one bump) and P14.T47 (photometry), marked "drafted for the
owner (delegated decision pending)". Accepted with amendments 2026-10-02 (decision-p14-phase-j).

#### R07.T2 The photometric section on the scene

- **R07.T2.a Absence, now.** `appearance/fromWire.ts` maps a body with no photometric section to
  `PROVISIONAL_PHOTOMETRY` (built here: Design note 5's Lambert sphere, p = 0.2, q = 1.5, through
  T4.b's `lawFor`) with the `BODY ALBEDO: NOT YET MODELLED` label, and a body with no flattening to
  a sphere of its `bulk.radius_m` as a `BodyFigure` (`pole` from `bodyFixedRotation`, `null` while
  plan 14 sends no rotation). A `contact` entry of R03's frame has an apparent position only: it
  is lit from its apparent direction with no eclipse or planetshine term, and with no resolved
  radius it stays R02's mark (a question for the owner, Risks). Tests: a body from today's
  `BodySummaryDto` maps to the provisional law and label; its figure is a sphere with a `null`
  pole. Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/appearance`,
  `just ci`.
- **R07.T2.b The section, once plan 14 has built it.** After plan 14's subtask lands and
  `just gen-protocol` has run, `lib/system/bodiesWire.ts` and `lib/system/model.ts` parse
  `photometry` and the figure as `SectionDto`s, and `fromWire.ts` maps them to `BodyPhotometry` with
  `provenance: "modelled"`, `bondRatioCheck` and `BodyFigure`. Tests: fixtures in each section
  state; a stated ratio that disagrees with the law's q_V by more than 5% is logged for plan 14's
  owner. Acceptance: `pnpm test`, `just ci`.

#### R07.T3 Illuminance at a body

`lighting/illuminance.ts`: `starIlluminance` from R06's `HostDiscDto` (Design note 4) and
`STAR_CUT_RELATIVE`; `aHostDisc` in `test/litFixtures.ts`. Tests against hand values, re-checking
the brainstorm's citations: the Sun at 1 au gives 1.28 × 10⁵ lx from V = −26.76 (Willmer 2018, ApJS
236, 47) with V = 0 at 2.54 µlx (Cox 2000, _Allen's Astrophysical Quantities_ §15; Crumey 2014), and
the disc form agrees with the magnitude form to 1%; the inverse square to Mars; the channel split's
V-weighted sum is E_V; the direction error from neglecting the star-to-body light time is under
10⁻⁷ rad for a Sun with a Jupiter; a companion under 10⁻⁴ of the brightest is dropped. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/lighting`.

#### R07.T4 The BRDF and its phase function

- **R07.T4.a Templates and the law.** `appearance/{law,templates}.ts` (Design note 5):
  `PHASE_TEMPLATES` from Mallama and Hilton 2018's eqs. 2–17 inside their valid ranges (Mercury's
  zeroth-order term −0.613, as that paper corrects the 2017 table), f tabulated at 0.5°, clamped at
  `PHASE_F_CLAMP` and held past each range; `PhotometricLaw` with its per-channel exponents.
  `test/litFixtures.ts` gains `SOLAR_SYSTEM_PHOTOMETRY`: p in B, V and R, B−V and V−R, V(1,0) and
  the computed q_V for Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus and Neptune. Tests: each
  template's Φ_t(0) = 1 and continuity at its range's end; the table's values against the paper.
  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/appearance`.
- **R07.T4.b Closed forms and `lawFor`.** `appearance/{brdf,phase}.ts`: `brdf`,
  `discIntegratedPhase` by the closed forms inside Φ_shape, `lawFor` solving s per channel by
  bisection. Tests: the closed forms of p and Φ for Lambert (p = 2A ÷ 3, q = 3 ÷ 2) and
  Lommel–Seeliger (p = ϖ ÷ 8, q = 16 ÷ 3 × (1 − ln 2)) against the `f64` disc integral to 10⁻⁴;
  `lawFor` recovers each template's p and q to 0.5%, with the departure the clamp causes stated per
  template; each planet's V at stated geometries within the paper's tolerance (about 0.01–0.03 mag
  for Mercury, Venus and Jupiter; Mars within its 0.035 mag of longitude variation; Earth within
  30%); a crescent of the Mercury template is redder in B − V than at opposition where the fixture
  gives it. Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/appearance`.
- **R07.T4.c The WGSL twin.** `body_brdf` and `struct LunarLambert` in `shaders/litBody.wgsl`, f as
  an RGB 1D texture, registered in `WGSL_CATALOGUE`. Tests: the TypeScript and WGSL functions agree
  at five pinned geometries to 10⁻⁵ relative in the smoke harness, one of them with a law built per
  texel from a synthetic A_N and L(α). Acceptance: `pnpm test`, `just test-render`.

#### R07.T5 Regimes, painter order and the appearance

`bodies/{regime,painter}.ts` and `appearance/bodyAppearance.ts` (Design notes 1, 2 and 19), and
`aLitBody` in `test/litFixtures.ts`. `BodyAppearance` gathers T2.a's figure and photometry, the
regime and the labels. Tests: the 3 px threshold at 720p, 1080p and 4K; the brainstorm's figures
re-derived: a Jupiter disc at least 3 px to about 9 × 10¹⁰ m, ten scale heights at 2 px to 2.5 × 10⁸
m for Jupiter and 5.5 × 10⁸ m for Saturn (scale heights from NASA's planetary fact sheets, cited in
the test); hysteresis holds a body at the boundary; the regime map is independent of input order;
painter order by power equals a ray-marched truth for 10⁴ random disjoint sphere pairs from random
cameras, host stars and points included; a planet behind its star is ordered behind it; an oblate
body is ordered on its equatorial sphere; a disc overlapping a mesh body is promoted. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies`.

#### R07.T6 The horizon and eclipse terms

- **R07.T6.a The horizon.** `lighting/{sphereIrradiance,oracle}.ts` (Design note 6), with the local
  horizon argument. Tests: Howell's factor against brute force to 2 × 10⁻³ at H = 3, 11.5 and 215,
  and its limb-darkening error at most 0.45% at 19.5°; the terminator 59.3 km wide on an airless
  body of 6,371 km at 1 au, and E ÷ E_zenith at the geometric terminator 9.87 × 10⁻⁴ for a uniform
  disc and 9.27 × 10⁻⁴ for the brainstorm's polynomial law; a planet at 3 stellar radii lit to
  109.5°; a local horizon of 5° removes the light of a star 4° up. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/lighting`.
- **R07.T6.b Annuli and overlaps.** `lighting/annuli.ts`: `annulusEdges`, `circleOverlapArea`,
  `eclipseVisible`. Tests: the eclipse term's worst absolute error at most 0.62% at K = 4 and 1.5%
  at K = 2 over a grid of radius ratios 0.1–30 and 41 separations, against 400 annuli; a concentric
  occultation against the closed form [(1 − c) μ_k² + 2c μ_k^(α+2) ÷ (α + 2)] ÷ [(1 − c) + 2c ÷ (α +
  2)], μ_k = √(1 − k²); a total eclipse exactly 0. Acceptance: `pnpm --filter hyperion exec vitest
run src/renderer/src/view/lighting`.
- **R07.T6.c Occluders and the WGSL.** `lighting/occluders.ts`, and `sphere_irradiance`,
  `eclipse_visible` and the stubs `ring_shadow_on_body`, `atmosphere_sun_transmittance` and
  `atmosphere_sky_irradiance` in `litBody.wgsl`. Tests: umbra and penumbra radii
  r_u = R_o − x (R★ − R_o) ÷ d and r_p = R_o + x (R★ + R_o) ÷ d at pinned distances; occluder lists
  empty for a body in no shadow cone; the WGSL equals the oracle at five pinned points and
  the stubs, which take R08.T9.b's full signatures (latitude and sun azimuth included, R08 Design
  note 17) and ignore them, return 1, 1 and 0 in the smoke harness. Acceptance: `pnpm test`,
  `just test-render`.

#### R07.T7 The photorealistic style

`photoreal/{style,passes}.ts` (Design note 8): the `photorealistic` variant of R02's `RenderStyle`
(`view/camera/state.ts`),
the pass list with empty slots for R08, R10 and R11, the style switch per view in
`displays/view/StyleControl.tsx` (a display control, single-key binding shown), and the refusal on a
fallback adapter through R01's `styleAvailability`. The HDR target is R02's. Tests: switching style
leaves the camera, projection and every body's projected position identical (R02's test extended);
the pass list for each setting, each entry labelled from `PHOTOREAL_PASS_LABELS` or by the owning
plan, with no label repeated; the control disabled with its reason on a fallback adapter; the
smoke harness renders an empty photorealistic frame with finite texels. Acceptance: `just ci`,
`just test-render`.

#### R07.T8 Point and disc bodies

- **R07.T8.a Point and disc.** `shaders/{bodyDisc,bodyPoint}.wgsl`, `bodies/draw.ts`. The disc: a
  quad over the body's projected bound, ray against the spheroid (Design note 19) in the
  camera-relative direction normalised by distance so that no large numbers meet in `f32`,
  `body_brdf` per channel from a uniform `DiscSurface` under the horizon, eclipse and ring-shadow
  terms, the meter class of Design note 10. The point: F = E × p × (R ÷ Δ)² × Φ(α) per channel, over
  the oblate area where there is one, into R02's sprite, at its place in the painter order. Tests:
  at the 3 px switch the disc's summed pixel flux equals the point's to 1% (a CPU rasteriser of the
  same shader arithmetic in TypeScript, and in the smoke harness); a phase scene
  (`view/scenes/phaseScene.ts`: a body at 0°, 90° and 150° phase) whose limb and terminator match
  the oracle to half a pixel; a Saturn-like f = 0.098 disc's polar and equatorial extents to half a
  pixel at 100 px; lit and unlit pixels carry their classes. By hand, recorded: the phase scene and
  a Jupiter from 10¹⁰ m. Acceptance: `just ci`, `just test-render`.
- **R07.T8.b The class-map hook.** `DiscSurface`'s `class-map` case in `bodyDisc.wgsl` and
  `bodies/discSurface.ts` (Design note 24), fed in tests by a synthetic two-class weights map with
  half the cells surveyed. Tests: surveyed texels shade by their classes' laws and the rest by the
  uniform law; a map of one class equal to the uniform law gives the uniform disc to 10⁻⁵; the
  disc's integrated p equals the area-weighted p of the classes. Acceptance: `just ci`,
  `just test-render`.

#### R07.T9 Mesh bodies

`bodies/smoothMesh.ts` (Design note 3): R05's selection and geometry on the reference spheroid at
zero height, built here over R05's public geometry if R05 has not built the spheroid, with the same
shading functions; the promotion of Design note 2. Tests: a promoted disc and its mesh agree in
projected silhouette to half a pixel, oblate giants included, and in total flux to 1% at the switch;
a moon passing behind a planet's limb is hidden where the oracle says, over a scripted occultation.
By hand, recorded: the occultation. Acceptance: `just ci`, `just test-render`.

#### R07.T10 Eclipses in the image

`view/scenes/eclipseScene.ts`, the kept scene: a camera crossing a moon's shadow on a planet, a ship
in a moon's penumbra looking at the star, and a planet passing behind its star. Tests: the shadow's
umbra and penumbra on the planet from the oracle to a pixel; the flux on a probe point over the
crossing against the oracle to the setting's error; the planet behind the star is covered by the
star's disc. By hand, recorded: a partial eclipse seen from the penumbra, the star's disc partly
covered by the moon's disc through the painter order. Acceptance: `just ci`, `just test-render`.

#### R07.T11 Planetshine

`lighting/planetshine.ts` and its term in `litBody.wgsl` (Design note 7): `planetshineSources`
returning direction, angular radius and illuminance per channel, through `sphere_irradiance`. Tests:
earthshine on the Moon at full Earth 15.3 lx ± 20% (the spread of Earth's p); the full Moon on Earth
0.32 lx from V = −12.74 to 5%; Jupiter-shine on Io at inferior conjunction about 70 lx ± 10%; a
neighbour at new phase contributes about nothing; a body's sources are the two largest, one on the
low setting. Acceptance: `pnpm test`, `just test-render`.

#### R07.T12 The exposure histogram

`post/histogram.wgsl` and `post/histogram.ts` (Design note 10): 256 bins over log₂ −14 to +16 of the
pre-exposed value, the meter class mapped to integer weights by `meterWeights`, workgroup-memory
atomics with one global add per non-empty bin, read back with at most three reads in flight over
a ring of three histogram buffers (R01's `readBuffer` makes its own staging buffer per call, so no
mapped buffer is ever reused; if T12's bench shows that per-call staging costs, a staging ring is
added to R01's readback in `view/engine/` under this task); a `KernelPair` with
`readback: "bit-exact"` and no subgroup twin. Tests: a CPU histogram of a
synthetic target equals the GPU's bin for bin in the smoke harness, on its `default` and
`no-subgroups` variants; host-disc pixels are not counted under any meter; zeros land in
bin 0; each meter's weights select their classes; the readback never maps a buffer in use. Bench,
recorded under `--hyperion-gpu-timing` with the flag noted, with a uniform dark-sky input as the
worst case: 0.3–0.5 ms on the discrete target, about 1 ms at 640 × 360 on the UHD 620 (brainstorm,
Performance budget; the probe's 0.3–0.8 ms is provisional). Acceptance: `just ci`,
`just test-render`.

#### R07.T13 Auto exposure and metering

- **R07.T13.a The meter and the controller.** `post/autoExposure.ts` (Design notes 10–12):
  `meteredLuminance` as the weighted arithmetic mean with window [0, 1] by default, bin 0 at
  luminance 0 and no veil; `AutoExposure.step` with the smoothing of Design note 11; `AVG`, `LIT`
  and `DARK` meters; `AUTO`, `MAN` and `INHIBITED` through R02's `ExposureControl` as built, the
  metered EV100 reported through `onMetering(control, ev100 | null)`, `null` when no histogram
  arrives (R02's system inhibit `no_image_to_meter`), so that R02's `setAuto` is accepted once a
  photorealistic view meters; the `ExposureReading` with its triple for R06's `cameraLimitV` and
  for wireframe views, the `AUTO` program's aperture and shutter a constructor argument (R06's
  `DEFAULT_VIEW_CAMERA`, N = 1.4 and t = 1/30 s, passed where the controller is made). Tests, against hand values with the citations re-checked:
  EV100 from a uniform 100 cd/m² field is log₂(800) = 9.644 (Lagarde and de Rousiers 2014, eq. 69);
  the displayed EV100 equals log₂(mean × 8) for a two-level histogram; the white point is 9.6 × L̄
  under a linear clip (eq. 75, Listing 28); a 5% disc 25 stops above a black frame is exposed 4–5
  stops above the average, not clipped; a Sun-like star entering a frame with a sunlit planet, its
  disc of class 0 and its glare from T14.a's sources drawn, changes the metered value by under 1%;
  30 Hz and 60 Hz step sequences reach the same EV at the same time within 0.05 EV; `LIT` and
  `DARK` differ as stated on a half-lit body; `INHIBITED` holds; under `AUTO` the triple's EV100
  equals the reading's to 10⁻⁹. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/post`.
- **R07.T13.b The control.** `displays/view/MeterControl.tsx`, beside R02's `ExposurePanel`: EV100
  with its automation level and the meter, keyboard operable. Tests (Vitest): each meter selectable by keyboard; the reading and
  its source view shown. By eye, recorded: a lit planet on black, a star entering frame, the
  cockpit turning to a planet, which settle the smoothing speeds. Acceptance: `pnpm test`.

#### R07.T14 Bloom as veiling glare

- **R07.T14.a The spread functions and the kernel.** `post/{bloom,glare}.ts` (Design note 12):
  `glareSpread` (the CIE function with 0.0046°, for an eye observer passed in, R06's default of
  age 25 and pigmentation 0.5 at the call sites, renormalised; the lens tail for camera views); `bloomKernel` by non-negative
  least squares per angular pixel scale; the threshold at AgX's top of range; the CPU twin of the
  chain with truncation modelled; each `GlareSource`'s closed form. Tests: the CIE function
  integrates to 1.047 at age 25 and 1.010 at pigmentation 0 before renormalisation; the chain's
  impulse response matches its encircled energy to 10% at 1, 4, 16 and 64 px; weights sum to 1 at
  both settings; stored plus injected energy equals the unclamped energy to 1%; the low setting uses
  fewer levels at quarter resolution. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/post`.
- **R07.T14.b The chain on the GPU.** `post/{bloomDown,bloomUp}.wgsl`, `rgba16float` intermediates
  unless R01's probe reports round-to-nearest, each `GlareSource` evaluated in the tone-mapping
  pass. Tests: on hardware in the smoke harness, stored plus injected energy equals the unclamped
  energy to 1.5%. Bench, recorded under `--hyperion-gpu-timing`: under 1 ms and 2–3 ms with tone
  mapping. Acceptance: `just ci`, `just test-render`.

#### R07.T15 Tone mapping, upscale and output

`post/tonemap.wgsl`, `post/tonemap.ts` (Design notes 9 and 13): R02's `agx`, Filament's port as
built by R02.T10.c with its header and `NOTICE` entry, included unchanged; the canvas's preferred
format written through its non-sRGB view with the encoding in the pass (the option Design note 13
adds to R01's engine, in `view/engine/`); static blue-noise TPDF dither of ±1 LSB in the encoded
domain; upscale from the internal resolution. Tests: `agx` against values computed once in `f64`
from Filament's formula and pinned with their citation; monotone in luminance; an isolated star
identical in both styles before the dither and within one code after it; the WGSL matches the
TypeScript at pinned inputs in the smoke harness; offline and recorded, a grey ramp within one code
of Blender's AgX Base sRGB LUT, which settles the encoding near black for both styles. By hand,
recorded: a Sun-like star in frame with a lit planet, hues holding in the highlight. Acceptance:
`just ci`, `just test-render`.

#### R07.T16 Symbology over the image, and the phrases

`photoreal/overlay.ts` (Design notes 16–17): R02's draw list with casing on in the photorealistic
style; rings and hulls as cased marks until R11; DOM readouts on `--surface-0` plates; the label
block gains the meter, the style and `BODY ALBEDO: NOT YET MODELLED`; hull edges cased over the
image, with the hull faces' occluder bias (`occluder.wgsl`, `slopeScale` 2 as built) raised to 3 so
that the casing is covered (the UX decision, item 12; the sphere occluder's `SLOPE_SCALE` is 3
already). Draft, for the owner, the nomenclature entries this plan adds beyond R02's nine items
(`PHOTOREALISTIC` is already drafted by R02, beside `WIREFRAME`; `METER AVG`,
`METER LIT`, `METER DARK`, `ONE PHOTOREALISTIC VIEW ON LOW SETTING`, the albedo phrase), as one
edit of `docs/frontend/ux-guidelines.md` that ends in the owner's sign-off. Tests: every overlay
mark over the image has a casing stroke; plates are present for every readout; the console-ux
skill's lint and contrast scripts pass. Acceptance: `just ci`; the guide edit is one commit for the
owner.

#### R07.T17 The low setting and benchmarks

Add Design note 18's settings as fields of R05's `ViewSettings`, with their high and low values in
`SETTINGS`, and Design note 14's `internalScaleBounds`, [0.5, 1.0] on both; record the benchmarks of
T12, T14 and T15 and the whole style's frame time on the development machine's RTX 3080, which
exceeds the RTX 4060 class of the brainstorm's Testing section, at 1080p, and, by the owner, on the
UHD 620 at 720p, each on a quiet machine, under `--hyperion-gpu-timing`, in this plan as "as built"
figures replacing the probes' provisional ones; R12 consolidates them. Acceptance: `just ci`; the
figures recorded with their settings, flags, load and dates.

### Phase B: several views

#### R07.T18 The view budget and resolution controller

`budget/{viewBudget,resolutionController}.ts` (Design note 14). Tests: one photorealistic view on
the low setting and the reason for the refusal; secondary views at lower scale or 30 Hz; the
primary's scale drops while instruments are open; `PER_CANVAS_OVERHEAD_MS` enters the budget; the
controller converges on a synthetic load, holds within the bounds it is given ([0.5, 1.0] from
`SETTINGS`, and a narrower pair in a second case), and does not oscillate under a
step (hysteresis); it is stable with inputs quantised to 65,536 ns; the frame interval is used when
timestamps are unavailable. Acceptance: `pnpm test`.

#### R07.T19 Instrument views in `VIEW`

`displays/view/InstrumentView.tsx`, `ViewDisplay.tsx` (Design note 15): two slots, each with its own
camera, style and target, R01's `createView(canvas, name)`, R02's DOM list and label block, and the exposure
reading of the primary view. Tests (Vitest): each view focusable and named; keyboard reaches every
camera control in every view; the style control of a second view is disabled on low with its
reason; a wireframe instrument shows the source of its exposure. Acceptance: `pnpm test`,
`just ci`.

#### R07.T20 Several views, by hand

With the real styles on the development machine (RTX 3080) and, by the owner, on the UHD 620, each
on a quiet machine: a full-window photorealistic view and two wireframe instruments, each the right
way up, no GPU time in copies, a resize of one leaving the others' attachments alone, the frame time
with instruments open against the low setting's 33 ms on the UHD 620 (brainstorm, Testing), and the
per-canvas overhead that replaces `PER_CANVAS_OVERHEAD_MS`'s provisional 0.3 ms. Recorded in this
plan. Acceptance: the record.

#### R07.T21 A child window on a second monitor

R01.T13 records the child-window prototype, with the research probe as its first data. This task
adds what that probe could not: an instrument view in a same-origin child window on a second
monitor, on R01.T13's prototype branch, recording whether Chromium on X11 paces it from that
display's vsync, and its frame times beside the main view's; and that the prototype's release of the
child's context on `pagehide` holds there. Done when a second display is available. Acceptance: the
record.

### Phase C: the main screen (gated on the sessions plan)

Each task begins by fitting its names to the sessions plan as built. **Out of scope for RM3**
(re-validated at `ce7aeb3`: no sessions plan exists); these tasks were not re-validated against
the code and are re-validated when that plan is built.

#### R07.T22 The client role

`ClientRoleDto` and `role` on `Hello`, defaulted to `console`, and `reduced_motion`, defaulted to
false, so that `PROTOCOL_VERSION` stays 2 (brainstorm, Runtime and code shape); the server records
both per connection; `--role main-screen` on the client's command line, and the main screen sends
its `prefers-reduced-motion`. Run `just gen-protocol`. Tests: wire forms; a `Hello` without `role`
is a console and without `reduced_motion` is false; both reach the connection. Acceptance:
`just ci`.

#### R07.T23 The camera as ship state

`crates/hyperion-protocol/src/main_screen.rs` (`MainScreenCameraDto`, `MainScreenGrantsDto`,
`CameraPresetDto`, `RenderStyleDto`, `TargetDto`), `MainScreenCommand` in the sessions plan's
command envelope, and `crates/hyperion-server/src/main_screen/{camera,grants}.rs` (Design note 20):
state, grants with their defaults, the selection commands through the sessions plan's command path
with closed-loop results, `selected_by`, persistence in `session.json`. Run `just gen-protocol`.
Tests: wire forms; default grants with and without a Captain; an ungranted station is rejected with
a reason; the last accepted selection stands; a session saved and reopened restores the camera.
Acceptance: `cargo test -p hyperion-server main_screen`, `just ci`.

#### R07.T24 The free camera on the server

`main_screen/{integrator,track}.rs`: take and release through control arbitration, the Captain's
override, the integrator from the holder's input (sent on change and at the tick) at each tick,
without acceleration ramps or damping while the main screen reports reduced motion, the presentation
track in the replay log. Tests: one holder at a time; the Captain takes from Helm; releasing on
disconnect; the integrator's pose for a pinned input log, with and without reduced motion; a
replay's bit-for-bit check passes with the track present and ignores it. Acceptance:
`cargo test -p hyperion-server main_screen`.

#### R07.T25 The camera on the scene topic

R03's reserved `main_screen` field on `SceneStateDto` and `SceneNotificationDto`, with the pose's
rates, pushed on change and at every tick (`CRAFT_PUSH_INTERVAL`) while the camera is flown, whether
or not craft are in the scene; the main screen's extrapolation with its one-tick blend, applied at
once under reduced motion, and interpolation as the fallback; a wireframe camera preset `slaved`,
added to R02's `CameraPreset` and shown `SLAVED`, on a station that follows it. Run
`just gen-protocol`. Tests: R03's two-clients-agree test extended to the camera; a flown camera in
a scene without craft is pushed every tick; a slaved helm's projected positions equal the main
screen's to a pixel at one push; extrapolation and interpolation agree on a constant-rate flight
and the blend is continuous on a correction. Acceptance: `just ci`.

#### R07.T26 The station's control panel

`displays/view/MainScreenControl.tsx`: preset, target and style as ship commands, visually distinct
from display controls, showing `PENDING` within 100 ms and then the result; `TAKE` and `RELEASE` for
the free camera; the flyer's local predicted view of the camera (Design note 22), labelled
`PREDICTED`; who selected and who flies, always shown. Draft, for the owner, the nomenclature
entries of Phase C (`TAKE` and `RELEASE` as a congruent pair, `SLAVED`, `PREDICTED`, the main
screen's setup phrases and its game-mode statement) as one edit of `docs/frontend/ux-guidelines.md`
that ends in the owner's sign-off. Tests (Vitest, `FakeWebSocket`): the closed loop in each outcome;
`PENDING` shown on the same frame as the command; ungranted stations see the state and no commands;
the predicted view carries its label; keyboard for everything. Acceptance: `pnpm test`; the guide
edit is one commit for the owner.

#### R07.T27 The main-screen client

- **R07.T27.a The chrome.** `mainScreen/{MainScreenApp,MainScreenStatusLine,ModeBanner,
AlertAnnunciator}.tsx` (Design notes 22–23, guide item 9): full screen, no console chrome, the
  view photorealistic by default; a status line with the ship's name, labelled ship time and link
  state; the mode banner; the alert annunciator (counts of active and unacknowledged emergency,
  warning and caution alerts and the newest unacknowledged emergency or warning in full, under the
  guide's flash and reverse-video rules, never a border or overlay) with text at least 30′; the
  label block with who commands the camera; the DOM list kept; on link loss the last frame held, the
  time stale with `S`, `NO CARRIER`. Tests: the annunciator's counts exclude advisories; link loss;
  under `prefers-reduced-motion` flashing is reduced and the camera's corrections are applied at
  once (T25). Acceptance: `pnpm test`, `just ci`.
- **R07.T27.b Text sized for the room.** `mainScreen/{roomTextSize.ts,DisplaySetup.tsx}` and the
  command-line options `--screen-diagonal-in` and `--viewing-distance-m`: the diagonal, the
  distance, the 100 mm check bar, the 2–10 ratio warning and the game-mode statement, saved on the
  machine. Tests: `roomTextSize` gives 36.7 px at 20′ and 55.0 px at 30′ for a 55″ 1080p screen at
  4 m, and the CSS size at a scale factor of 0.78125; the ratio warning at 1.9 and 10.1; the
  options override the saved values. Acceptance: `pnpm test`.
- **R07.T27.c EDID, and the room.** `apps/hyperion/src/main/displayEdid.ts`: the EDID read and its
  match to Electron's display (Design note 23). Tests: EDID parsing of a pinned blob; a zero size
  rejected; an ambiguous match gives no pre-fill; no pre-fill when the EDID's two reported sizes
  disagree by more than 5%, with the development machine's projector EDID as the fixture (decided
  2026-09-30 by a delegated decision; Risks). By hand, recorded: the main screen across a room
  at the set distance, the text legible and the 100 mm bar measured. Acceptance: `pnpm test`; the
  record.

#### R07.T28 The loop, measured

An integration test from a station's input frame to the scene push that carries its pose, on
loopback, recording p50 and p95 against the server's share of Design note 22 (about 10 ms typical);
by hand, recorded, the time from a station key press to the main screen's photon on a wired LAN,
filmed at 240 frames a second, with extrapolation and with interpolation, the p95 against the 100 ms
requirement and the worst case recorded beside it as a finding, the main screen's
animation-frame-to-photon time recorded separately and the display's own lag measured apart; the
flyer's local view's loop; a discrete selection's `PENDING` within 100 ms and completion within 250
ms. Acceptance: the test passes; the record.

## Verification

- **Photometry:** illuminance, magnitudes and phase against the Solar System table (T3, T4);
  planetshine against earthshine and the full Moon (T11); flux continuous across point, disc and
  mesh (T8, T9); the class-map hook against the uniform law (T8.b); a star identical in both styles
  before the dither (T15).
- **Geometry:** projected positions identical in both styles (T7); painter order against a
  ray-marched truth, host stars and points included, and occultations at the limb (T5, T9, T10);
  oblate extents (T8); terminator and eclipse against the `f64` oracles (T6, T10).
- **Exposure:** the metered value unmoved as a star enters frame; histograms equal bin for bin on
  devices with and without the `subgroups` feature (T12, T13); bloom conserves energy with
  truncation modelled (T14).
- **Views:** one photorealistic view on low; three canvases on one device with no copies (T18–T20).
- **Main screen:** grants, exclusivity and the Captain's override; saved with the session; replay
  ignores the track; two clients agree on the camera; reduced motion removes the camera's
  smoothing; the loop about 60 ms typical and within 100 ms at the 95th percentile to the photon
  with a game-mode display, its worst case recorded (T22–T28).
- **By eye, recorded:** the phase, occultation and eclipse scenes, a lit planet on black, a star and
  planet in frame, the cockpit with instruments on the development machine and on the UHD 620
  (T8–T10, T13.b, T15, T20), and the
  main screen across a room (T27.c).
- **Benchmarks:** T12, T14, T15 and T17's timings on both GPUs on a quiet machine, handed to R12.

## Generator version

No change to generated output and no bump: the renderer reads. The photometric section and the
flattening are galaxy plan 14's and move its version when they land. On the wire everything is
additive: optional sections parsed by the client, a defaulted `role` and `reduced_motion` on
`Hello`, R03's reserved
optional field and new commands in the sessions plan's envelope, none of which moves
`PROTOCOL_VERSION` by the brainstorm's rule. The plan reserves nothing in the generator.
Re-validated at `ce7aeb3` (`GENERATOR_VERSION` 19): Phases A and B change no protocol type and
generate nothing; T2.b only parses bindings that plan 14's subtasks generate, and those subtasks
(T1's drafts) carry plan 14's own bump, coordinated through the orchestrator, and its
`just gen-protocol`.

## Risks and open points

- **Provisional timings.** Every timing from the research probes (histogram, present overhead,
  quantisation's effect) was measured while other work loaded the machine; T17 and T20 re-measure
  on a quiet one before any figure becomes "as built".
- **Templates with a borrowed shape** (airless ice and snowball, the Moon's curve with q solved to
  Ganymede's and Europa's; magma, Mercury's) are provisional and labelled. Thick magma oceans take
  Venus's curve unlabelled;
  the q values of Jupiter and Neptune rest on phase curves extrapolated past their data (Mayorga et
  al. 2016 would settle Jupiter); and plan 14's airless-rock Bond albedo and Earth's albedo are
  checks for its owner (T1).
- **The lens PSF** of camera views rests on recalled veiling-glare figures (low confidence); the
  eye's CIE function is solid. The glare threshold at AgX's top of range and the smoothing speeds
  are settled by eye (T13, T14).
- **Glare in the meter.** The veil is drawn and not metered (Design note 12). A camera meter does
  see flare, so a realistic camera mode that meters it could be offered as an operator-selected
  meter (the research lean, 2026-09-29), not as a default and not with a cap, which would be an
  arbitrary number; it would need a guide entry for the owner. Not built.
- **The law's thresholds** (Design note 5: 100 Pa and 30 kPa, raised from 10 kPa so that the
  simulated Mars, 11 kPa, reaches the Mars template; the cloud term suspended until plan 14's cloud
  fraction depends on the condensables, the README's open finding) are
  judgement, of medium confidence, and the Mars template holds past about 50° of phase by the
  clamp; the smooth blend of Design note 5 replaces the steps if the population shows jumps.
- **Planetshine's uniform-disc approximation** shifts its terminator on the receiver by the
  neighbour's crescent offset, up to about 4° at Io; stated, not corrected.
- **Gas giants' cloud bands** are left by R11 to neither R08 nor itself, and R11 advises this plan;
  this plan draws a uniform,
  oblate giant and no plan generates bands. The roadmap's open item records a research lean that
  this plan own a band texture driven by zonal jets over R08's deck photometry, with the structure
  asked of plan 14; this plan has not taken it up, and it waits on the owner's assignment and on
  plan 14's structure.
- **Magma oceans glow** above about 1,394 K by thermal emission, which is not reflection; no plan
  owns it (R08 or R10).
- **Oblateness beyond the image.** R02's graticule on a 10% oblate Saturn is off by up to 6,000 km,
  and collision is spherical; both are R02's and the flight model's to take up once plan 14 sends f.
  For an Earth-like rotator, collision against a sphere is up to 21 km wrong at low altitude, so the
  flight model needs the spheroid (or R10's terrain on it) before landings on such worlds.
- **The datum across plans.** The spheroid of Design note 19 is the one height datum for R05's
  vertex formation, R09's coarse elevations and R10 (the coordinator's ruling of 2026-09-29, on
  this plan's research; not an owner decision). Until each plan's text is built to it, a height
  measured from a sphere would meet the smooth figure with a seam of up to 21 km on an Earth.
- **Ordering against craft.** Promotion to a mesh settles depth for bodies against depth-writing
  geometry; craft are outlines until hull art exists, and when lit hulls arrive they join the
  promotion rule as depth-writing geometry.
- **The main screen is gated** on a plan that does not exist. Its task list is written against the
  brainstorm and will be re-fitted (T22–T28 each begin so); the sessions plan may choose a command
  envelope that reshapes `MainScreenCommand`, and must accept input sent on change.
- **Extrapolating the main screen's camera** departs from the brainstorm's interpolation (Design
  note 22); T28 compares both, and the owner may keep interpolation at the cost of about 16 ms.
- **The loop's worst case** is about 102 ms with a keyboard and 119 ms with a gamepad against the
  100 ms requirement (Design note 22). If T28's record confirms it, the cuts in order are the render
  and present term (25–33 ms, through the main screen's internal scale) and the animation-frame
  wait.
- **A child window on a second monitor** is unproved until a second display is at hand (T21).
- **HDR output.** An `rgba16float` canvas with extended tone mapping configures on the probed UHD
  620, but its panel is not HDR; it would suit a main screen on an HDR television and is left open.
  _Decided 2026-09-30 by a delegated decision (hardware item 6):_ HDR stays open. The development
  machine's display, an Optoma UHD projector, declares no HDR, and Xorg has no HDR path. What is
  checked on that machine: the device-pixel ratio (0.78125 at `Xft.dpi` 75), that an
  extended-range canvas configures, and the projector's declared 72 ms of lag, which T28 records
  as the display's own lag.
- **EDID on the development machine, decided 2026-09-30 by a delegated decision** (hardware item
  6). The projector's EDID reports two sizes that disagree, so Design note 23's pre-fill also
  requires them to agree within 5%, and that EDID is T27.c's test fixture for the no-pre-fill case.
- **Hull edges over the image, decided 2026-09-30 by a delegated decision** (the UX decisions,
  item 12). R02 draws hull edges uncased in the wireframe, since a casing would widen a 1.5 px
  edge past the 2 px that its occluder's slope bias covers. This plan's overlay (T16) must case them
  over the photorealistic image and raise the occluder's slope scale to 3 so that the casing is
  covered (R02's Risks, T13 as built). As built at `ce7aeb3` the sphere occluder's `SLOPE_SCALE`
  is already 3 (RM1 validation, for graticule strokes); the one to raise is the hull faces'
  `occluder.wgsl` depth bias, `slopeScale` 2.
- **The camera's local state, decided 2026-09-30 by a delegated decision** (the UX decisions,
  item 14). R02's `CameraState` keeps `free` (`FreeFlight`) and `move` (`EasedMove`), a local
  view's integration state. This plan moves them into a local wrapper, so that the server-held
  main-screen camera carries only the shared shape (the brainstorm's "one shape in both
  deployments"; R02's Risks, T9 as built).
- **A kept scene on the main screen says `TRAINING`** (decided 2026-10-01 by a delegated decision,
  R02's depth decisions, item 3). R02 shows the `TRAINING` banner only while `VIEW` draws a kept
  scene; the main screen, when it shows a kept scene, must say `TRAINING` too.
- **Hidden-window resizes restart the GPU process** under the Vulkan surface (R01's Risks, "The
  forced path is the only Linux path"; T12 and T13 as built). On the RTX 3080 every resize or
  creation of a hidden window gave `vkAcquireNextImageKHR` OUT_OF_DATE and a GPU-process
  restart, and three restarts remove WebGPU. `--disable-vulkan-surface` avoids it, but whether it
  costs Vulkan presentation on screen is unchecked. This plan's child windows (T21) and any
  minimised or hidden view must be checked on screen, by hand, before the choice of switch is
  made; the visible-window check is pending by hand for the owner.
- **Asked by later plans.** R10's asks are built here as signatures with defaults (Design note 24)
  and completed by R10's own tasks in this plan's files, with the signatures unchanged: `body_brdf`
  with per-texel lunar-Lambert parameters (T4.c, completed by R10.T10.b); the disc sampling
  `classMap.ts` over surveyed texels through `DiscSurface`, with `lawFor(p, q, template)` elsewhere
  (T8.b, completed by R10.T10.d); and `sphere_irradiance` with a per-sample local horizon whose
  absence is the closed form (T6.a, completed by R10.T8.b). R11's ring shadow on the body is the
  `ring_shadow_on_body` stub (T6.c). R12's stable `PassList` labels are `PHOTOREAL_PASS_LABELS`
  (T7), and the resolution controller's bounds are the setting value
  `ViewSettings.internalScaleBounds` (Design note 14, T17, T18). Not yet designed here: the
  instrument panels' sizes in the cockpit layout, which R12 needs for its runs.
- **Re-validated at `ce7aeb3`** (2026-10-02, RM3; RM1 merged, R05 and R06 not built, `main`
  equal to the integration branch). Consumes swept against R01–R04 as built and their Risks: the
  "as built" notes in Consumes (R01's engine calls, `createView`'s name, per-call readback,
  post-process labels and bindings, the canvas's sRGB view, per-format rounding, the timing
  switch's home; R02's `Viewport` without `fovXRad`, `ExposureControl`'s union and
  `onMetering`, `ExposurePanel`, `graticule`'s module, no client `BodyFixedRotation`; R03's
  `SceneBodyFrame` union and per-body grants). Edits: Provides' names and the WGSL convention
  with `displayName` (R01 Design notes 23–24; Babylon dropped); `METER_CLASS` in `post/meter.ts`
  to meet R06.T13.e's file; pass labels as `onPassTimes` derives them; `glareSpread` and the
  `AUTO` program take the eye observer and the default camera as arguments so that T13.a and
  T14.a do not wait on R06; T12's readback over R01's per-call staging; T15 adds a non-sRGB
  canvas write to R01's engine (Design note 13); the RTX 3080's `toward-zero` rounding in Design
  note 12; T16's hull-occluder bias; T2.a after T4.b, where `lawFor` is; T1's draft fitted to plan
  14 as built; `pnpm exec prettier`; Phase C out of scope for RM3. RM1's decisions and follow-ups
  for this plan (UX items 12 and 14, the kept scene's `TRAINING`, hardware item 6's HDR and EDID,
  the hidden-window OUT_OF_DATE restart, Babylon dropped) were already folded in by the RM1-close
  pass (`ca9b2ae`) and are kept. Brainstorm drift since `899db5e`: only the CSP ruling and
  Babylon wording, neither touching this plan. **Pending re-validation:** T2.a waits on
  R05.T7.a; T3 on R06.T10 (`HostDiscDto`'s field names); T5 on R06.T10; T7, T17 and T18 on
  R05.T7.b; T8.a and T10 on R06.T13.e; T9 on R05.T7.a–b, T8 and T11; T19 on R05.T7.d; T2.b on
  plan 14's subtasks from T1; T22–T28 on the sessions plan. **Missing galaxy work** (plan 14 on
  `main`): the photometry section (T2.b, T8.a's modelled albedos, T11's neighbours' p); the
  flattening and the Saturn-like moment of inertia (T8.a's and T9's oblate figures); rotation and
  pole on the wire (P14.T14 is sim-only; the oblate axis in T8.a and T9, and R02's
  `bodyFixedRotation`); the spheroid datum (Design note 19, for R05, R09 and R10). Until then every
  body is a sphere of its mean radius with the provisional photometry, labelled. **For the owner**
  (smallest choice made, reversible): a `contact` body (apparent position only) is lit from its
  apparent direction without eclipse or planetshine and, with no resolved radius, stays R02's
  mark (T2.a); the lean is to keep it so until a contact carries a radius.
- **Deviations in T14.a, as built** (2026-10-02). `post/glare.ts`, `post/bloom.ts` and an added
  `post/nnls.ts` (Lawson and Hanson's non-negative least squares), each with tests.
  `bloomKernel(setting: BloomSetting, role, radPerPx, eye: EyeObserver)`: `BloomSetting` is the
  literal `"high" | "low"`, structurally R05's unbuilt `QualitySetting`, and becomes that import
  when R05.T7.b lands; `eye` is an argument as for `glareSpread`, so that it builds before R06
  (both approved by the orchestrator). `BloomKernel` extends `BloomLevels { firstLevel, levels }`:
  `weights[k]` multiplies mip level `firstLevel + k`, the low setting starting at level 1, quarter
  resolution; the level counts are constants (`BLOOM_LEVELS`: 7 from level 0 high, 5 from level 1
  low) until T17 makes them settings. **Design note 12's age factor is corrected** (science check,
  2026-10-02): the CIE 135/1999 complete equation has age in both terms, [1 − 0.08 (A/70)⁴] on the
  core and [1 + 1.6 (A/70)⁴] on the wide-angle term (McCann and Vonikakis 2018, eq. 2; Vos and van
  den Berg 1997); "(A ÷ 62.5)⁴, wide term only" is CIE 146:2002's simpler equation. The code uses
  the complete form, which gives T14.a's 1.047 and 1.010; the equation is valid to 100° and is
  extrapolated beyond (1.2% of its energy). The threshold 2^`AGX_MAX_EV` = 16.29 is 156.4 L̄, not 157. The camera PSF is a Gaussian core of σ = 0.25′ holding 97% and a Harvey-type 1 ÷ (1 +
  (θ/0.1°)²) tail holding 3%, within the 1–10% veiling glare index reported for commercial lenses;
  the shape and knee are assumed (low confidence, as before). The fit matches encircled energy at
  16 radii from 0 to 128 px with Σ w = 1; at 1080p across 60° the eye kernel is within 8.4% at
  1 px, 4.4% at 4 px and 1% at 16 and 64 px; the CIE energy beyond the chain's reach (about 6%
  beyond 128 px) is gathered into its widest levels, conserving energy but not the far veil. The
  chain's last step, w₀ D₀ + up(U₁), runs inside the tone-mapping pass in `f32`, unrounded; every
  other level is a rounded `rgba16float` write, `unknown` rounding modelled as toward zero.
  `GlareSource.excessLuminance` is cd/m² (Provides and R06's contract): the mean luminance less
  65,504 ÷ the pre-exposure scale. `glareSourceVeil` is the point form, L_ex Ω PSF(θ), exact in
  energy but far too faint just outside a resolved disc's limb; ruled 2026-10-02 (orchestrator):
  T14.b adds a near-limb term per CIE core term, the half-plane closed form 2ac²(π/2 − atan(d/c))
  at distance d beyond the limb, as max(point, half-plane), tested against a brute-force disc
  quadrature, with a science check. `bloomKernel` costs some 35 ms per fit after a one-off
  250–700 ms for the level responses (provisional, under load): its caller refits only when the
  angular pixel scale changes materially, not each frame.
- **Deviations in T12, as built** (2026-10-02). `post/histogram.{wgsl,ts}` and `post/meter.ts`
  (`MeterMode`, `METER_CLASS`, `MeterClass`, `meterWeights`; R06.T13.e extends this file, never
  redeclares). The kernel, `HISTOGRAM_KERNEL` (`exposure histogram`, reference only,
  `bit-exact`, entry `main`), takes the HDR colour as a sampled `TextureHandle` (`hdr`; decision
  2026-10-02, item 1: T7 wires the view's scene target), its `Params` uniform (`histogramParams`:
  the four class weights, the size, the stride) and a 256-word `bins` buffer, and bins the Rec.
  709 luminance (`METER_LUMA`, BT.709) of the pre-exposed value: bin 0 below 2⁻¹⁴ (and NaN in the
  CPU twin; WGSL leaves the kernel's NaN case unspecified), bin 255 from 2¹⁶ and for +∞ (a pass
  writing above 65,504; review fix, since a saturating `u32` of +∞ wrapped to bin 0), else 1 +
  ⌊(log₂ L + 14) × 8.5⌋; the class is `round(alpha)`, half to even, clamped to [0, 3]. The low
  setting's quarter-resolution input is a stride of 2 on each axis (`HistogramRequest.stride`),
  not a separate downsample. Exports beyond Provides: `HistogramReader`, `HistogramRequest`,
  `HistogramEngine` (the four engine calls the reader makes), `HISTOGRAM_RING` (3),
  `HISTOGRAM_PASS` (`"histogram"`, `PHOTOREAL_PASS_LABELS.histogram` once T7 builds it),
  `HISTOGRAM_WORKGROUP` (16 × 16, one bin per invocation), `HISTOGRAM_BINS_PER_STOP` (8.5),
  `histogramWorkgroups`, `METER_LUMA`, the CPU twin `cpuHistogram` with `histogramBin`,
  `binCentreLuminance` and `meterClassOf`. `HistogramReader` owns the ring of three storage
  buffers: a frame finding all three still being read takes no histogram, results older than one
  delivered are dropped, and the owner re-creates the reader in `onRestored`. Since R01's
  `readBuffer` stages per call, no mapped buffer is ever reused; the test of "never maps a buffer
  in use" checks instead, on a fake engine, that no slot is zeroed or dispatched into while its
  read is pending. Registered in `WGSL_CATALOGUE` (`POST_ENTRIES`); the smoke check
  (`smoke/histogram.ts`, on a 70 × 45 `rgba16float` texture of its own, luminances at bin
  centres, with black and infinite texels) passes bin for bin under each meter at strides 1 and 2,
  and through the reader's ring, on both `default` and `no-subgroups` (`just test-render`,
  SwiftShader, 2026-10-02). **The bench is pending, and no staging ring was added to R01's
  readback**: the RTX 3080 at 1920 × 1080 on a quiet machine, and the owner's UHD 620 at 1280 × 720
  with stride 2 (640 × 360), both under `--hyperion-gpu-timing` with a uniform dark-sky input;
  T17 builds the by-hand harness that drives `HistogramReader` and records the figures with the
  other post-processing benches (the probe's 0.3–0.8 ms stays provisional), and if per-call
  staging shows a cost T17 adds the staging ring through R01's guarded readback only (decision
  2026-10-02, item 6).
- **Deviations in T13.a, as built** (2026-10-02). `post/autoExposure.ts`: `meteredLuminance(h,
window = FULL_WINDOW)` (0 when the window holds no counts), `smoothEv`, `programTriple`,
  `ExposureProgram` (the `AUTO` program's N and t, a constructor argument: R06's
  `DEFAULT_VIEW_CAMERA` where the controller is made), `AutoExposureOptions`, and `AutoExposure`
  as a class (Provides sketched an interface with `step`), which also holds the operator's meter
  (`setMeter`), takes R02's command results (`apply(ExposureCommandResult)`), and exposes
  `meteredEv100` for R02's `setAuto` and `enable`. `step(h, dtS)` takes `undefined` on frames
  with no new histogram: the meter holds its last value, and only after `METER_TIMEOUT_S` (0.5 s)
  without one, or on a histogram with nothing to meter (a mean of 0, as under `LIT` with no lit
  body), does it report `null` to R02's `onMetering`, the system inhibit `NO IMAGE TO METER`;
  a system inhibit resumes `AUTO` from its held value and smooths from there. The smoothing is in
  closed form over each step (linear to the band's edge, then exponential at rate speed ÷ 1.5),
  so 30 Hz and 60 Hz agree to rounding; its speeds and band are Unreal's documented defaults
  (Speed Up 3, Speed Down 1 f-stops/s, `ExponentialTransitionDistance` 1.5; science check
  2026-10-02), no longer "from memory", still to be settled by eye. The star-entering test uses a
  480 × 270 frame at 60° (the Sun at 1 au 2.1 px in radius, a planet 15% of the frame): the
  metered value moves 0.01%, where in a 64 × 64 frame the disc's own 5% of the pixels, removed
  from the count, moved it 6%; Design note 11's "4–5 stops above the average" holds for a body
  covering about 3–6% of the metered pixels, and one under 0.64% reaches AgX's ceiling, as a
  real averaging meter would. **For the owner** (science check): under `AUTO` the program's
  sensitivity S = 5880 × 2^−EV100 at f/1.4 and 1/30 s spans ISO 0.18 (a sunlit planet, EV100 15) to 6 × 10⁶ (a dark sky, EV100 −10), far outside a real sensor; the lean is to record the
  triple as nominal until R06's `cameraLimitV` models noise from S, then clamp S and let the
  shutter take over.
- **Deviations in T13.b, as built** (2026-10-02). `displays/view/MeterControl.tsx` (with
  `meterLabel`) shows `EV100 9.6 AUTO` through R02's `exposureReading`, `METER AVG`, the source
  view as `SOURCE VIEW`, and the meters `AVG`, `LIT` and `DARK` as pressed-state buttons in the
  guide's order, reachable by Tab and pressed by Enter or Space; with no reading it says `NO
IMAGE TO METER` and holds the meters back (`aria-disabled`, focusable). It takes an
  `ExposureReading | null` and `onMeter(mode)`. **Not yet mounted in `ViewDisplay`**: no view
  meters an image until T7 makes the photorealistic view and its `AutoExposure`; T7 mounts it
  beside `ExposurePanel` (a few lines of `ViewDisplay`), so that no control stands on screen with
  nothing behind it. The labels `METER AVG`, `METER LIT`, `METER DARK` are T16's guide draft. The
  by-eye checks (a lit planet on black, a star entering the frame, the cockpit turning to a
  planet), which settle the smoothing speeds, wait on T7 and are pending by hand for the owner:
  `just client` with a photorealistic `VIEW` on the development machine.

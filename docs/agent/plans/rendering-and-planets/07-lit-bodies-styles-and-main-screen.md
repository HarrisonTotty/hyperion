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
export const DISC_ANNULI_HIGH = 4; // limb-darkened annuli, edges by equal concentric error
export const DISC_ANNULI_LOW = 3; // the orchestrator's ruling, 2026-10-03 (was 2)
/** Edges of K flux-exact annuli by equal concentric error for one channel's power-2 law
 *  I(μ)/I(1) = 1 − c(1 − μ^α), from `HostDiscDto`'s per-channel coefficients. */
export function annulusEdges(c: number, alpha: number, k: number): AnnulusSet; // as built
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
  readonly body: BodyIdHex; // as built (T11): the neighbour
  readonly direction: Vec3;
  readonly angularRadiusRad: number;
  readonly distanceM: number; // as built (T11)
  readonly radiusM: number; // as built (T11)
  readonly illuminance: Rgb;
}
export function litNeighbours(
  bodies: ReadonlyArray<ReflectingBody>,
  hosts: ReadonlyArray<PlacedLight>,
  annuli: number,
): LitNeighbour[]; // as built (T11): each neighbour's starlight, once a frame
export function planetshineSources(
  body: ReflectingBody,
  lit: ReadonlyArray<LitNeighbour>,
  max: number,
): SecondarySource[]; // Design note 7; as built (T11)
```

`lighting/oracle.ts`: the `f64` oracles (dense annulus sums, brute-force sphere irradiance, the
disc integral of a law).

### Appearance and BRDF (`appearance/`)

```ts
export interface PhotometricLaw {
  // Design note 5
  readonly a: Rgb; // albedo scale per channel
  // L(0), 0 (Lambert) to 1; R10.T10.b adds an optional L(α) curve in the phase row's alpha
  // (decision-r07-t8b)
  readonly lommelSeeligerShare: number;
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
export type AppearanceLabel = "BODY PHOTOMETRY: NOT YET MODELLED";
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

`shaders/bodyDisc.wgsl` (the analytic disc, spheroid by axis scaling), point bodies as sprite
records through R06's `SKY_SPRITE_HDR_MATERIAL` (`POINT SPRITES HDR`), R02's `starSprite.wgsl` with
the identity tone step, the sprite row's `z` carrying depth (decision-r07-t8a, item 2),
`bodies/smoothMesh.ts` (R05's quadtree on the reference spheroid at zero height).

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
export interface ViewSpec {
  readonly id: ViewId;
  readonly slot: "primary" | "instrument"; // `ViewSlot`
  readonly style: RenderStyle; // the style the view's camera asks for
}
export interface ScaleControl {
  readonly bounds: ScaleBounds;
  readonly target: ResolutionTarget;
}
export interface ViewBudget {
  readonly renderScale: number;
  readonly rateHz: 60 | 30;
  readonly style: RenderStyle; // drawn
  readonly streamPriority: "primary" | "secondary";
  readonly control: ScaleControl | null; // a photorealistic primary while instruments are open
}
export const PER_CANVAS_OVERHEAD_MS = 0.3; // provisional, Design note 14
export function viewBudgets(
  views: ReadonlyArray<ViewSpec>, // one primary and the open instruments, in slot order
  setting: QualitySetting,
): ReadonlyMap<ViewId, ViewBudget>; // pure
export function photorealisticAllowed(
  views: ReadonlyArray<ViewSpec>,
  setting: QualitySetting,
  candidate: ViewId,
):
  | { allowed: true }
  | { allowed: false; reason: "NOT AVAILABLE: QUALITY LOW allows one photorealistic view" };
/** `ViewSettings.internalScaleBounds`, [0.5, 1.0] on both settings (note 14). */
export type ScaleBounds = readonly [min: number, max: number];
export interface ResolutionTarget {
  readonly periodMs: number;
  readonly gpuBudgetMs: number;
}
export class ResolutionController {
  constructor(bounds: ScaleBounds, target: ResolutionTarget);
  get scale(): number;
  retarget(target: ResolutionTarget): void; // keeps the scale
  /** Each frame's `gpuTimeMs` once, or `undefined` where `GraphicsStatus.timer` is `"absent"`. */
  update(gpuFramesMs: ReadonlyArray<number> | undefined, intervalMs: number): number; // renderScale
}
export function gpuTimeMs(times: ReadonlyArray<PassTimes>): number;
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
   which gives 2.2 × 10⁸ m for Jupiter and 4.9 × 10⁸ m for Saturn at 1080p across 60°, and R07.T5
   re-derives both (recomputed with R02's centre-pixel scale, 1,663 px/rad; the brainstorm used
   width ÷ field and gives 2.5 and 5.5 × 10⁸ m: the orchestrator's ruling, 2026-10-03). Each threshold has a 10% hysteresis, so a body at the boundary does not flip
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
   for L at high albedo; added by decision-r07-earth-albedo, 2026-10-04: Robinson 2026, PSJ 7, 12,
   arXiv:2507.22258, §5 and eq. 14, for Earth's curve and p, replacing Mallama et al. 2017's Earth
   row and Mallama and Hilton 2018's eq. 5, Tinetti et al. 2006's model, whose Sun–observer azimuth
   is turned by 180° (Robinson et al. 2011, Astrobiology 11, 393)).
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
   for grey regolith, 0.9 for Earth, 0.69 for red Mars and about 2 for the giants, dark in the near
   infrared. The template and s fix q; the ratio is carried as a check (`bondRatioCheck`), and a
   body whose q_V from its law differs from the ratio's by more than 5% is a finding for plan 14's
   owner, not an input the client reconciles. Until the
   section is on the wire a body takes `PROVISIONAL_PHOTOMETRY`, a Lambert sphere of spherical
   albedo 0.3 (p = 0.2, q = 1.5, brighter at large phase than any real body), and the label block
   says `BODY PHOTOMETRY: NOT YET MODELLED` (a phrase for the owner, R07.T16).
6. **The horizon in closed form, eclipses by annuli** (researched 2026-09-29; Howell's catalogue of
   radiation view factors, configuration B-43 (Cunningham 1961; Hauptmann 1968),
   <https://www.thermalradiation.net/tablecon.html>, formula transcribed from memory and verified by
   brute force to 2 × 10⁻³; Kreidberg 2015, PASP 127, 1161). The brainstorm's "sampled at a handful
   of points" is 13–26% wrong for small occluders at 4 to 8 samples and bands by 1 ÷ N, and needs
   about 256 samples for 1%. Instead a lit point's visibility is the product of two closed forms.
   The horizon term is the irradiance factor from a uniform sphere of H = d ÷ R★ at angle φ between
   the normal and the star's centre, which is exact for a uniform disc and within 0.47% of the
   limb-darkened one for a star 19.5° in radius (polynomial law; 0.61% in B for the Sun's power-2
   law); it softens the terminator and lights a close-in
   planet beyond its hemisphere (to 109.5° at 3 stellar radii). The eclipse term splits the disc
   into K annuli of uniform intensity, each with its exact flux and edges by equal concentric
   error, and takes each annulus's eclipsed area as the difference of two exact circle–circle
   overlaps: continuous, so it never bands; for the Sun 0.56% (V), 0.70% (B) worst absolute error
   at K = 4 (high) and 0.97% (V), 1.23% (B) at K = 3 (low; 2.1% and 2.7% at K = 2, before the
   orchestrator's ruling of 2026-10-03 raised the low setting to 3) (decision-r07-dn6, 2026-10-03; the law
   I(μ)/I(1) = 1 − c(1 − μ^α) after Hestroffer 1997, A&A 327, 199, eq. 4, its coefficients from
   Maxted 2018, A&A 616, A39, Table 2). The product errs only where an
   eclipse's penumbra crosses the terminator band. Occluder lists are built on the CPU in `f64` from
   shadow cones and are usually empty. At 1 au the soft terminator is 59.3 km wide on an Earth-sized
   body and lies below 0.5% of peak irradiance, black under a day-side exposure. Mandel and Agol
   2002 and Agol, Luger and Foreman-Mackey 2020 are the `f64` oracles' cross-checks; Maxted and Gill
   2019's qpower2 is validated only below a radius ratio of 0.2 and is not used.
7. **Planetshine is included** (researched 2026-09-29; figures computed from Design note 5's
   albedos). The brainstorm does not mention it; the realism ruling decides. It is the dominant
   night-side light of every moon: earthshine on the Moon at full Earth is about 8 lx (7.7 lx with
   the `earth` template, Robinson 2026's fit; 8.1 lx at his physical model's p;
   decision-r07-earth-albedo), 14 stops below sunlight but some 3 × 10⁴ times the integrated
   starlight (2.8 × 10⁻⁴ lx on a face-on element: Seares et al. 1925's 1,092 stars of V = 1.0 over
   the sky, quoted by Roach and Megill 1961, ApJ 133, 228, ÷ 4); Jupiter-shine on Io about 70 lx, 6
   stops below. Each body is lit by at most two neighbours whose reflected illuminance
   E★ p (R ÷ Δ)² Φ(α) is largest (one on the low setting), each as a uniform sphere of its angular
   radius through the same `sphere_irradiance`, never shadow-tested against third bodies on its way
   (the neighbour's own starlight is eclipsed: averaged over its lit disc as the body it lights sees
   it, R07.T10.b; until then from its centre, of the bodies larger than it only, as T11 built it,
   which leaves out a smaller body's shadow, up to 11% of earthshine in a central solar eclipse, and
   fades a moon wider than the penumbra too fast, in 44 s rather than 254 s for Io). Its stated
   errors (corrected by T11's science check, 2026-10-04, as is the starlight figure, first given as
   10³–10⁵ times): the lit crescent's light centroid lies off the neighbour's centre, by 0.4 R at
   60° of phase and 3π ÷ 16 ≈ 0.59 R at quarter phase for a Lambert sphere (5.7° for Jupiter seen
   from Io), towards 0.9 R in a thin crescent (first given as "up to 0.4 R, about 4°"); and the
   far-field E errs at first order in R ÷ Δ for a disc brighter at its centre: the exact illuminance
   is 12% above it at full phase (76.8 lx against 68.7 lx in V) and 10% below it at quarter phase
   for Jupiter seen from Io, and 1.2% above it for Earth seen from the Moon (first given as of order
   (R ÷ Δ)², about 3% at Io). Beyond quarter phase the body sees less than the neighbour's
   hemisphere, which hides the limb crescent: the exact is 34% below at 120°, 71% below at 150° and
   nothing from 170.6° for Jupiter seen from Io, 10% below at 150° for Earth seen from the Moon.
8. **The photorealistic style is a pass list over R02's scene and camera and a per-view HDR target
   that R07.T7 creates with R01's `createRenderTarget` in R02's `HDR_COLOUR_FORMAT` (R02 DN12: the
   wireframe has none; decisions-r06-r07, item 1).** In order:
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
R05.T7.a–b, T8 and T11; T10 on T9 and R06.T13.e, except T10.b, on T6.b, T8.a and T11; T11 on T8.a;
T14.b's injected sources on R06.T13.e's `glareSources` (synthetic sources in its tests until then);
T17 on R05.T7.b; T18 on R05.T7.b; T19 on R05.T7.d. Free of both, and startable now: T1, T4.a–c,
T6.a–c, T12, T13.a (the
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
gives A_V 0.57 against a Bond albedo of 0.294. _Resolved 2026-10-04 (decision-r07-earth-albedo):_
Earth takes Robinson 2026's curve and p (R07.T4.d, P14.T47.e), p_V q_V 0.282 against CERES's 0.2915.
Draft beside it the flattening of Design note 19 (f, or equatorial and polar radii) with its
per-class moment of inertia, which also replaces P14.T14.b's "I = 0.33–0.4 M R² by class" so that
one moment of inertia serves locking and flattening, with the classes (rocky, Jupiter-like,
Saturn-like, ice giant) defined by a criterion on plan 14's
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
  T4.b's `lawFor`) with the `BODY PHOTOMETRY: NOT YET MODELLED` label, and a body with no flattening to
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
  `provenance: "modelled"`, `bondRatioCheck` and `BodyFigure`; and `view/scene/fromServer.ts` reads
  P14.T46.f's rotation section into the scene body's `rotation` through `rotation3FromRows`
  (decision-p14-phase-j), which orients R10's class maps. Tests: fixtures in each section
  state; a stated ratio that disagrees with the law's q_V by more than 5% is logged for plan 14's
  owner. It waits for P14.T47.e and R07.T4.d, as does any client test against `templates.golden`:
  before them every temperate world above 30 kPa would be drawn 1.9× too bright
  (decision-r07-earth-albedo). Acceptance: `pnpm test`, `just ci`.

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
  `PHASE_TEMPLATES` from Mallama and Hilton 2018's eqs. 2–17 inside their valid ranges and the
  Moon's from Krisciunas and Schaefer 1991 eq. 9 (Allen 1973) to 150°, which `airless-ice` and
  `snowball` borrow at L = 1 (decision-phase-curves, 2026-10-02) (Mercury's
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
- **R07.T4.d Earth after Robinson 2026** (decision-r07-earth-albedo, 2026-10-04).
  `appearance/templates.ts`' `earth` becomes Robinson 2026's eq. 14 (PSJ 7, 12,
  arXiv:2507.22258): Δm(α) = 3.75 log₁₀[(1 + g² + 2g cos α) ÷ (1 + g)²], g = −0.33, to 144° (the
  curated data's 5–144°), held beyond, L = 0, s = 1, not provisional. Its source string cites eq.
  14, and Robinson et al. 2011 (Astrobiology 11, 393) for dropping eq. 5.
  `SOLAR_SYSTEM_PHOTOMETRY`'s Earth row:
  - p in B, V and R 0.263, 0.215 and 0.210: the fit's f = 0.23 in Model 07's band ratios
    0.277 : 0.226 : 0.221, whose 0.4–0.5, 0.5–0.6 and 0.6–0.7 µm bands stand for Johnson's;
  - B − V 0.43, V − R 0.52;
  - V(1, 0) −3.23, with `templateV10Mag` the same;
  - q_V 1.312.

  `planetshine.test.ts` takes `planetPhotometry("Earth")` again. Tests:
  - Φ_t(0) = 1, and q_V 1.3116 to 0.5% (the clamp acts from 139.0°; eq. 14 unclamped gives
    1.350);
  - the law's V equals eq. 14 at f = 0.23 to 0.5% from 0° to 135°, in place of eq. 5's "within
    30%";
  - the clamp-departure row recomputed;
  - 0.23 q within 3% of Robinson's visual spherical albedo of 0.294.

  It integrates with P14.T47.e, whose `templates.golden` the client's `earth` must equal.
  R07.T2.b waits for both. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/appearance src/renderer/src/view/lighting`.

#### R07.T5 Regimes, painter order and the appearance

`bodies/{regime,painter}.ts` and `appearance/bodyAppearance.ts` (Design notes 1, 2 and 19), and
`aLitBody` in `test/litFixtures.ts`. `BodyAppearance` gathers T2.a's figure and photometry, the
regime and the labels. Tests: the 3 px threshold at 720p, 1080p and 4K; the brainstorm's figures
re-derived: a Jupiter disc at least 3 px to about 7.9 × 10¹⁰ m, ten scale heights at 2 px to
2.2 × 10⁸ m for Jupiter and 4.9 × 10⁸ m for Saturn (recomputed with R02's centre-pixel scale; the
brainstorm used width ÷ field and gives 9 × 10¹⁰, 2.5 × 10⁸ and 5.5 × 10⁸ m; scale heights from NASA's planetary fact sheets, cited in
the test); hysteresis holds a body at the boundary; the regime map is independent of input order;
painter order by power equals a ray-marched truth for 10⁴ random disjoint sphere pairs from random
cameras, host stars and points included; a planet behind its star is ordered behind it; an oblate
body is ordered on its equatorial sphere; a disc overlapping a mesh body is promoted. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies`.

#### R07.T6 The horizon and eclipse terms

- **R07.T6.a The horizon.** `lighting/{sphereIrradiance,oracle}.ts` (Design note 6), with the local
  horizon argument. Tests: Howell's factor against brute force to 2 × 10⁻³ at H = 3, 11.5 and 215,
  and its limb-darkening error at most 0.48% at 19.5° for the polynomial law and 0.62%, 0.49% and
  0.40% for the Sun's B, V and R power-2 laws (decision-r07-dn6); the terminator 59.3 km wide on an airless
  body of 6,371 km at 1 au, and E ÷ E_zenith at the geometric terminator 9.87 × 10⁻⁴ for a uniform
  disc and 9.27 × 10⁻⁴ for the brainstorm's polynomial law; a planet at 3 stellar radii lit to
  109.5°; a local horizon of 5° removes the light of a star 4° up. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/lighting`.
- **R07.T6.b Annuli and overlaps.** `lighting/annuli.ts`: `annulusEdges`, `circleOverlapArea`,
  `eclipseVisible`. Tests: the eclipse term's worst absolute error, over a grid of radius ratios
  0.1–30 and 41 separations against the exact integral oracle, at most 0.73%, 0.58% and 0.48% for
  the Sun's B, V and R at K = 4, 1.30%, 1.02% and 0.85% at K = 3 (the low setting, the
  orchestrator's ruling) and 2.8%, 2.2% and 1.85% at K = 2 (decision-r07-dn6); the equal
  dip predicting the grid's worst to 3 × 10⁻⁴; a concentric
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
fallback adapter through R01's `styleAvailability`. T7 creates each photorealistic view's HDR scene
target, `photoreal/sceneTarget.ts` (`sceneTargetSpec`, `createSceneTarget`): `createRenderTarget` in
`HDR_COLOUR_FORMAT` (`rgba16float`) at the view's internal resolution, with its own `depth32float`,
one mip, category `render-targets`, named `<view>:hdr`, resized with the internal scale and disposed
when the view leaves the style; no other task creates one (decisions-r06-r07, item 1); tests: the
cleared alpha equals `METER_CLASS.other`. Tests: switching style
leaves the camera, projection and every body's projected position identical (R02's test extended);
the pass list for each setting, each entry labelled from `PHOTOREAL_PASS_LABELS` or by the owning
plan, with no label repeated; the control disabled with its reason on a fallback adapter; the
smoke harness renders an empty photorealistic frame with finite texels. Acceptance: `just ci`,
`just test-render`.

#### R07.T8 Point and disc bodies

- **R07.T8.a Point and disc.** `shaders/bodyDisc.wgsl`, `bodies/draw.ts`, point bodies through R06's
  `POINT SPRITES HDR` (decision-r07-t8a, item 2). The disc: a
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

- **R07.T10.a Retarded lighting geometry** (decision-r07-t8a, follow-up (a), whose interface and
  tests it builds). Drawing keeps the apparent places. Lighting takes retarded geometric positions,
  in the system frame and without aberration: the lit body at its drawn time t_B, each star,
  occluder or planetshine neighbour X at t_B − |r_X − r_B| ÷ c, and a neighbour's own stars at its
  own retarded time, translated to the body's drawn centre. It adds fields only, with no protocol
  change, to `lib/scene/apparent.ts` (`emittedM`, `emittedVelocityMPerS`) and
  `view/scene/{model,fromServer}.ts` (`RetardedCentre`, `ViewBody.retarded`), and creates
  `lighting/retarded.ts` (`retardedFrom`, `lightingFrameOf`). It removes T8.a's known limit ("Light
  positions") and T11's ("Neighbours at drawn centres") before T10.c's scene. Acceptance:
  `pnpm test`.
- **R07.T10.b A body's eclipse over its disc** (decision-r07-earth-albedo, 2026-10-04).
  `lighting/discEclipse.ts`: `discEclipseVisible(star, body, occluders, towards, share, k)`,
  per channel. It is the fraction of a body's reflected light towards a far point (the camera
  for a point body, the lit body for a planetshine neighbour) that an eclipse leaves:
  - V̄ = 1 − ∫ (1 − V) w dA ÷ ∫ w dA, over the body's disc projected along the star's
    direction;
  - V is `eclipseVisible` at the surface point there;
  - w = [L · 2 ÷ (μ₀ + μ) + (1 − L)] μ for μ > 0, the lunar-Lambert term per unit projected
    area; f(α) cancels;
  - it is taken on the equivalent sphere √(a c).

  The quadrature: V depends only on the distance ρ from the shadow axis in that plane; the
  cone's spread over the body's depth, below R★ R ÷ d, is neglected. So the deficit is a
  product quadrature in (ρ, θ) about the axis:
  - ρ runs over the penumbra's overlap with the disc, split at |r_u|, |b − R| and the annulus
    contacts, with one `eclipseVisible` per node and channel;
  - θ runs over each circle's arc inside the disc;
  - ∫ w dA is the closed form R² π [L + ⅔ (1 − L)] Φ_shape(α).

  `pointFlux` and planetshine's neighbour use it for every occluder in `occludersFor`'s list,
  and the "larger than the neighbour" filter goes. The neighbour's factor is per (neighbour,
  lit body) pair. It is computed only where the list is non-empty, and the ranking's bound stays
  uneclipsed. Tests:
  - against a brute-force f64 surface integral (10⁶ points) to 10⁻³ absolute, over radius
    ratios 0.02–30, separations across the penumbra, phases 0°, 60° and 120°, and L 0 and 1;
  - a body wholly in the umbra gives exactly 0, and one clear of every penumbra exactly 1;
  - Earth's light towards the Moon in a central solar eclipse is the oracle's 0.893 of clear
    (uniform Sun, Lambert);
  - a point Jupiter in Io's shadow transit keeps 99.9–100% of its clear flux (the measured
    5.16 × 10⁻⁵ lx case, not 0);
  - a point Io entering Jupiter's shadow fades from 1 to 0 over about 254 s (the centre: 44 s);
  - at the 3 px switch during an Io-like ingress, the disc's summed pixel flux equals the
    point's to 1%.

  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/lighting
src/renderer/src/view/bodies`, `just test-render`.

- **R07.T10.c The eclipse scene.** `view/scenes/eclipseScene.ts`, the kept scene: a camera crossing
  a moon's shadow on a planet, a ship in a moon's penumbra looking at the star, and a planet passing
  behind its star. Tests: the shadow's umbra and penumbra on the planet from the oracle to a pixel;
  the flux on a probe point over the crossing against the oracle to the setting's error; the planet
  behind the star is covered by the star's disc. By hand, recorded: a partial eclipse seen from the
  penumbra, the star's disc partly covered by the moon's disc through the painter order. Acceptance:
  `just ci`, `just test-render`.

#### R07.T11 Planetshine

`lighting/planetshine.ts` and its term in `litBody.wgsl` (Design note 7): `planetshineSources`
returning direction, angular radius and illuminance per channel, through `sphere_irradiance`. Tests:
earthshine on the Moon at full Earth 7.7 lx ± 15% (Robinson 2026's fit, f = 0.23 in Model 07's
colours, as R07.T4.d's `earth` row; his physical model's 8.1 lx lies inside; the band covers
p_V 0.23 ± 0.02 and the weather; decision-r07-earth-albedo); the full Moon on Earth 0.32 lx from
V = −12.74 to 5%; Jupiter-shine on Io at inferior conjunction about 70 lx ± 10%; a neighbour at new
phase contributes about nothing; a body's sources are the two largest, one on the low setting.
Acceptance: `pnpm test`, `just test-render`.

#### R07.T12 The exposure histogram

`post/histogram.wgsl` and `post/histogram.ts` (Design note 10): 256 bins over log₂ −14 to +16 of the
pre-exposed value, the meter class mapped to integer weights by `meterWeights`, workgroup-memory
atomics with one global add per non-empty bin, read back with at most three reads in flight over
a ring of three histogram buffers (R01's `readBuffer` makes its own staging buffer per call, so no
mapped buffer is ever reused; if T12's bench shows that per-call staging costs, a staging ring is
added to R01's readback in `view/engine/` under this task (approved 2026-10-02, item 6, through
R01's guarded readback only)); a `KernelPair` with
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
adds to R01's engine, in `view/engine/`; approved 2026-10-02, item 6: an opt-in per submission,
default the sRGB view, every existing smoke check unchanged; tested by `just test-render` and the
engine's Vitest suite; the overlay pass of T16 follows it); static blue-noise TPDF dither of ±1 LSB in the encoded
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
block gains the meter, the style and `BODY PHOTOMETRY: NOT YET MODELLED`; hull edges cased over the
image, with the hull faces' occluder bias (`occluder.wgsl`, `slopeScale` 2 as built) raised to 3 so
that the casing is covered (the UX decision, item 12; the sphere occluder's `SLOPE_SCALE` is 3
already). Draft, for the owner, the nomenclature entries this plan adds beyond R02's nine items
(`PHOTOREALISTIC` is already drafted by R02, beside `WIREFRAME`; `METER AVG`,
`METER LIT`, `METER DARK`, the meter's statuses `NO LIT SIDE`, `NO DARK SIDE` and
`STAR DISC ONLY` with their remedy clauses (decision-r07-t8a-meter),
the albedo phrase; the several views' refusal,
`NOT AVAILABLE: QUALITY LOW allows one photorealistic view`, is the guide's `NOT AVAILABLE` form
and adds no entry, decision-r07-t18 item 6), as one edit of `docs/frontend/ux-guidelines.md`
that ends in the owner's sign-off. T16 also builds them:
`AutoExposure` keeps why it has no metered value (`no-image`, a histogram timeout;
`nothing-weighed`, histograms that weigh no pixel under the meter in force; `acquiring`, under
`METER_TIMEOUT_S` since the image was first drawn or the meter changed with no value held). R02's
`InhibitReason` gains `"nothing_weighed"` with its meter. `ExposurePanel` and `MeterControl` take
the cause beside the metered value; while it is `acquiring`, neither shows a meter status or
`NO IMAGE TO METER`, and `ENABLE` is held back with `NOT AVAILABLE: not yet metered`. Tests: under
`LIT` with no lit body, a drawn image reads `NO LIT SIDE`, never `NO IMAGE TO METER`, after 0.5 s
and not before; a meter change clears it at once; `AUTO` resumes when a lit body is metered. Its
other tests: every overlay mark over the image has a casing stroke; plates are present for every
readout; the console-ux skill's lint and contrast scripts pass. Symbology over the tone-mapped
image is a following canvas pass with `FrameSubmission.colourLoad` `"load"` through the sRGB view,
in the same task as T15's pass (built by T15 under decision 2026-10-02, item 6). Acceptance:
`just ci`; the guide edit is one commit for the owner.

#### R07.T17 The low setting and benchmarks

Add the rest of Design note 18's settings as fields of R05's `ViewSettings`, with their high and
low values in `SETTINGS`. T18 added Design note 14's `internalScaleBounds`, [0.5, 1.0] on both,
and `budget { photorealisticRateHz, photorealisticViews }` (high 60 Hz and no limit; low 30 Hz and
Design note 18's one photorealistic view), which T17 keeps as built (decision-r07-t18, item 2).
Make `VIEW`'s quality setting selectable in the running client (the orchestrator's assignment,
2026-10-04): `VIEW` hard-codes `high` today (`ViewDisplay.tsx` and `useViewSky.ts`), and the
UHD 620 runs of this task and of T20 need `low`. The setting is the one `VIEW` input that its sky,
its photorealistic frame and its budgets (T19) read; how the operator chooses it is the smallest
reversible choice, recorded in Risks. Record the benchmarks of T12, T14 and T15 and the whole
style's frame time on the development machine's RTX 3080, which exceeds the RTX 4060 class of the
brainstorm's Testing section, at 1080p, and, by the owner, on the UHD 620 at 720p, each on a quiet
machine, under `--hyperion-gpu-timing`, in this plan as "as built" figures replacing the probes'
provisional ones; R12 consolidates them. Tests (Vitest): `VIEW` given `low` draws its sky and its
photorealistic frame at the low setting's values. Acceptance: `pnpm test`, `just ci`; the figures
recorded with their settings, flags, load and dates.

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

`displays/view/InstrumentView.tsx`, `ViewDisplay.tsx` (Design notes 14 and 15): two slots, each
with its own camera, style and target, R01's `createView(canvas, name)`, R02's DOM list and label
block, and the exposure reading of the primary view; and T18's budgets wired into `VIEW`
(decision-r07-t18, item 1). `VIEW` builds a `ViewSpec` for the primary and for each open
instrument in slot order, and takes `viewBudgets(views, setting)` again whenever a view opens or
closes, a camera's style changes or the setting changes, the setting coming from one `VIEW` input
(`high` until the client offers `low`). Each view draws its budget's `style`, its label block's
`STYLE` naming the style drawn, and is paced at its `rateHz`: a 60 Hz view every animation frame,
a 30 Hz view every second one, a 30 Hz primary on every second vsync as R05 Design note 21 paces
the low setting, and an instrument only in frames the primary draws. Each view's terrain demand
carries its `streamPriority` (R05). A photorealistic view's scene target is made at its render
resolution times its scale: the budget's `renderScale`, or, while its `control` is set, the
`scale` of a `ResolutionController` made from `control` when it first appears, `retarget`ed when
its target changes (an instrument opened or closed, the setting changed) and dropped when
`control` returns to `null`. The controller is updated once a primary frame: `gpuFramesMs` holds
`gpuTimeMs` of every view's `PassTimes` submitted from one primary frame to the next, one entry
for each primary frame once all its resolves are in (none on some updates), or is `undefined`
while `GraphicsStatus.timer` is `absent`; `intervalMs` is the interval between the primary's
frames. The style control and the key `4` ask `photorealisticAllowed` before a switch to the
photorealistic style; a refusal holds the button back with its reason, the adapter's refusal
(`styleRefusal`) first where both hold. Tests (Vitest): each view focusable and named; keyboard
reaches every camera control in every view; the style control of a second view is disabled on low
with `NOT AVAILABLE: QUALITY LOW allows one photorealistic view`, and the key `4` refused there;
a wireframe instrument shows the source of its exposure; against a fake engine, opening an
instrument beside a photorealistic primary gives it a controller whose scale sizes the scene
target, closing the instruments returns it to the bounds' max, a 30 Hz view draws in every second
frame, an instrument's pass times count in the primary's frame, and an absent timer feeds
`undefined`. Acceptance: `pnpm test`, `just ci`.

#### R07.T20 Several views, by hand

With the real styles on the development machine (RTX 3080) and, by the owner, on the UHD 620, each
on a quiet machine: a full-window photorealistic view and two wireframe instruments, each the right
way up, no GPU time in copies, a resize of one leaving the others' attachments alone, the frame time
with instruments open against the low setting's 33 ms on the UHD 620 (brainstorm, Testing), and the
per-canvas overhead that replaces `PER_CANVAS_OVERHEAD_MS`'s provisional 0.3 ms. On the UHD 620's
low setting, also a wireframe primary with two wireframe instruments, and one with a
photorealistic and a wireframe instrument, against R05 Design note 21's criteria at 60 Hz (T the
display's measured vsync period). A miss of the first moves the low setting's wireframe primary to
30 Hz as a `budget` field; a miss of the second alone puts the photorealistic instrument's scale
under the controller, against the primary's period, rather than lowering the primary's rate
(decision-r07-t18, items 4 and 5). Recorded in this plan. Acceptance: the record.

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
- **`VIEW`'s quality setting is `high` throughout** (T17, the orchestrator's assignment,
  2026-10-04). `ViewDisplay.tsx` hard-codes `"high"` for the photorealistic frame (line 510) and
  bakes the sky at the high setting's face size wherever the device blends `float32` (433), and
  `useViewSky.ts` takes `SETTINGS.high.sky` (92), so the running client cannot draw at `low`,
  which T17's and T20's runs on the UHD 620 need. T17 makes the setting one `VIEW` input. How the
  operator chooses it is open: the lean is a launch option, as the descent spike's
  `--setting <high|low>` (R05.T13.c), since each measured run keeps one setting; a `QUALITY`
  control in `VIEW` (the guide's draft label) would change it mid-run, which T18's `retarget`
  bears, and would add its control to the guide (for the orchestrator).
- **Templates with a borrowed shape** (airless ice and snowball, the Moon's curve with q solved to
  Ganymede's and Europa's; magma, Mercury's) are provisional and labelled. Thick magma oceans take
  Venus's curve unlabelled; the q values of Jupiter and Neptune rest on phase curves extrapolated past their data (Mayorga et
  al. 2016 would settle Jupiter); and plan 14's airless-rock Bond albedo is a check for its owner
  (T1). Earth's albedo is resolved (decision-r07-earth-albedo). Its curve, Robinson 2026's eq. 14,
  is cut by the clamp from 139°, up to 29% at 144°, inside that phase's 38% weather spread. It is
  equal in every channel, since Robinson's per-band curves are not tabulated.
- **The lens PSF** of camera views rests on recalled veiling-glare figures (low confidence); the
  eye's CIE function is solid. The glare threshold at AgX's top of range and the smoothing speeds
  are settled by eye (T13, T14).
- **Glare in the meter.** The veil is drawn and not metered (Design note 12). A camera meter does
  see flare, so a realistic camera mode that meters it could be offered as an operator-selected
  meter (the research lean, 2026-09-29), not as a default and not with a cap, which would be an
  arbitrary number; it would need a guide entry for the owner. Not built.
- **No way back to `MAN` (open, for the owner; observed in decision-r07-t8a-meter, not ruled).**
  No control calls `setManual`, so `MAN` cannot be re-entered once it is left, and `INHIBIT` is
  the operator's only hold. A `MAN` entry (the triple, or an EV100 set point, under the guide's
  data-entry rules) is the owner's to decide; not built.
- **The law's thresholds** (Design note 5: 100 Pa and 30 kPa, raised from 10 kPa so that the
  simulated Mars, 11 kPa, reaches the Mars template; the cloud term suspended until plan 14's cloud
  fraction depends on the condensables, the README's open finding; to be built in T5 and T1's
  draft) are judgement, of medium confidence, and the Mars template holds past about 50° of phase by the
  clamp; the smooth blend of Design note 5 replaces the steps if the population shows jumps.
- **Constant L on resolved discs** (decision-r07-t8b). The L = 1 templates' resolved discs depart
  from the Moon's measured L(α) (McEwen 1996, LPSC XXVII, 841) at large phase. At equal flux the
  flux-weighted RMS difference is 0.13 stop at 30° and 0.25 stop at 90°, and the cusps are up to
  1.5 stop too bright. q, p and the point's flux are unaffected. R10.T10.b adds the L(α) channel
  and moves these templates to McEwen's curve.
- **Planetshine's uniform-disc approximation** shifts its terminator on the receiver by the
  neighbour's crescent offset, 5.7° at Io at quarter phase (0.59 R; T11's science check,
  2026-10-04, first given as about 4°); stated, not corrected. Its far-field illuminance errs by
  about 0.72 R ÷ Δ near full phase (at Io the exact is 12% above it at full phase, 10% below at
  quarter), and past quarter phase by the limb crescent the body cannot see (at Io 34% below at
  120°, 71% at 150°, all of it from 170.6°; 10% at 150° for Earth seen from the Moon). A
  correction table in (α, sin ρ, L) would remove both; not built.
- **The disc-averaged eclipse** (R07.T10.b, decision-r07-earth-albedo) is taken on the equivalent
  sphere √(a c); an oblate body shaded by a larger one errs at the second order in f.
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
  (T8.b, completed by R10.T10.d), which reads the map by a survey-masked bilinear reconstruction,
  with the point integrating it (R10.T10.f) and the L(α) channel (R10.T10.b) (decision-r07-t8b); and
  `sphere_irradiance` with a per-sample local horizon whose absence is the closed form (T6.a,
  completed by R10.T8.b). R11's ring shadow on the body is the `ring_shadow_on_body` stub (T6.c).
  R12's stable `PassList` labels are `PHOTOREAL_PASS_LABELS` (T7), and the resolution controller's
  bounds are the setting value `ViewSettings.internalScaleBounds` (Design note 14, T17, T18). Not
  yet designed here: the instrument panels' sizes in the cockpit layout, which R12 needs for its
  runs.
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
- **Deviations in T4.a, as built.**
  - **Files.** `appearance/law.ts` holds `PhaseTemplateId`, `PhotometricLaw`, `PHASE_F_CLAMP` and
    the table: `phaseFactor(law, α)` (exact f, clamped, held past the range), `phaseFactorTable`
    (361 texels at `PHASE_TABLE_STEP_RAD` 0.5°, r, g, b interleaved in a `Float32Array`) and
    `phaseFactorFromTable` (linear, as the shader reads it). `appearance/shapes.ts`, a file the
    plan does not name, holds the shapes' closed forms (`lambertPhase`, `lommelSeeligerPhase`,
    `shapePhase`, `shapeGeometricAlbedo`) and `phaseIntegral` (Simpson, 7,200 intervals), since
    the f table needs Φ_shape and `phase.ts` (T4.b) imports the table; T4.b's `phase.ts` keeps
    `discIntegratedPhase` and `lawFor`. `Rgb` is in display order (r, g, b): index 0 is R, 2 is B.
  - **`PhaseTemplate` gains two fields:** `provisional: boolean`, for T5 and T16's labels, and
    `lommelSeeligerShare`, the L that goes with the curve by Design note 5 (1 for the Moon,
    Mercury, airless ice, the snowball and magma, 0.5 for Mars, 0 otherwise), so that the
    templates' keys and laws live in one place (`templates.ts`) and change cheaply (the
    coordinator's request 2026-10-02).
  - **Moon:** Mallama and Hilton 2018 has no Moon; `moon` is Krisciunas and Schaefer 1991's eq. 9,
    Δm = 0.026 α + 4 × 10⁻⁹ α⁴, a fit to Allen 1973's table (p. 143, to 160°), held past 150°,
    now in Design note 5's sources. q = 0.626 at L = 1.
  - **Ranges and branches.** Mercury's eq. 2 is used below its observed 2.1°; Uranus's eq. 15 and
    Neptune's eq. 17 from opposition, in place of the flat eqs. 14 and 16 (which drops the
    paper's 0.021 and 0.015 mag steps at 3.1° and 1.9°); Mars is eq. 6 without L(λe) and L(Ls),
    held past 50° (eq. 7 unused, the coordinator's approval 2026-10-02); Saturn is its globe
    (eqs. 11–12, joined at 6°), the rings being R11's. Earth first ran eq. 5 to 180° (MH2018
    §4.3: Tinetti's curve approaches zero there; Mallama et al. 2017's Table A-3.1 tabulates a
    steeper fit of the same curve, 2.07 mag at 90° against eq. 5's 1.57); R07.T4.d replaced it
    with Robinson 2026's eq. 14 to 144° (decision-r07-earth-albedo).
  - **Borrowed shapes (decision-phase-curves, 2026-10-02, after the phase-curve check's four
    mismatches with galaxy's classes):** airless ice and the snowball take the Moon's curve at L = 1,
    their q reached through s (Ganymede's 0.80 at s ≈ 0.82, ratio 0.98; Europa's 1.01 at s ≈ 0.67,
    ratio 0.99, both inside T2.b's 5%); magma takes Mercury's, for the thin branch below 30 kPa
    only. All three are `provisional` and labelled. The Moon's constant is `MOON_KS91`. q does not
    depend on L inside a template's range; past it, where f is held while the shape varies with L,
    it does (see T4.b).
    The selection rule (30 kPa, the cloud term suspended) is T5's and T1's draft's, and no template
    is wired to a body class yet. A sourced icy curve exists only as Hapke fits (Domingue and
    Verbiscer 1997, Icarus 128, 49).
  - **Fixture.** `SOLAR_SYSTEM_PHOTOMETRY` adds `templateV10Mag` (MH2018's zeroth-order terms, e.g.
    Mercury's −0.613 beside Table 3's −0.69), `radiusKm` (the disc-equivalent √(a c) each Table 7
    p_V implies: Jupiter 69,134 km, Uranus 25,264, Neptune 24,552, Mars 3,386, Saturn the paper's
    57,240) and `template`; magnitude fields end in `Mag`; `SUN_JOHNSON_MAG` (Table 6) and
    `JohnsonBvr` are exported beside it. q_V is computed at s = 1 with the clamp and hold: Mercury
    0.480, Venus 1.344, Earth 1.312 (Robinson 2026's eq. 14, R07.T4.d; 1.311 on eq. 5), Mars
    1.085, Jupiter 1.312, Saturn 1.357, Uranus 1.302, Neptune 1.242 (reproduced independently by
    the science check). The table's tests live in `appearance/solarSystemPhotometry.test.ts`,
    inside the acceptance filter. Earth's row is Robinson 2026's since R07.T4.d (see its
    deviations).
  - **For T4.c.** WebGPU has no three-channel float format and `rgba32float` filters only with
    `float32-filterable`, so the shader reads the table as RGBA texels by two `textureLoad`s and
    interpolates itself; the table's 0.5° interpolation errs by up to 4 × 10⁻⁴ of a steep
    crescent's f and 2 × 10⁻³ where Venus's f meets the clamp (the phase integral by 10⁻⁴), an
    error the reference and the shader share.
- **Deviations in T4.b, as built.**
  - **f from the table.** `brdf` and `discIntegratedPhase` read f from the law's 0.5° table by
    linear interpolation, as the shader will, so that the disc and the point integrate one f
    (T4.c's 10⁻⁵ agreement needs it). They read it through `phaseFactorTableOf` (`law.ts`), which
    tabulates each law once and caches it in a `WeakMap` keyed by the law object.
  - **`lawFor(p, q, template)` takes L from the template**
    (`PHASE_TEMPLATES[id].lommelSeeligerShare`), A = p ÷ [L + ⅔(1 − L)], and solves each
    channel's s by 60 bisection steps (geometric midpoints) over s ∈ [1/16, 16], on the exact,
    clamped and held f over a 0.1° Simpson grid sampled once per call. A q no exponent reaches takes the bracket's nearer end, the
    closest law the template allows.
  - **Extra exports.** `geometricAlbedo(law)` (`phase.ts`); `lighting/oracle.ts` is created here,
    ahead of T6.a, with `discIntegral(reflectance, α, nodes = 200)` (Gauss–Legendre over
    photometric longitude and latitude, on any `Reflectance`), `Reflectance`, `gaussLegendre` and
    `GaussLegendreRule`.
  - **The clamp's departure in q at s = 1**, exact f on both sides: Venus −0.317%, Earth −0.656%
    (R07.T4.d's eq. 14, clamped from 139.0°; −0.011% on eq. 5), Uranus −0.008%, every other
    template 0.
  - **q against L (the coordinator's correction of the ruling, 2026-10-02, from the Phase J lane;
    plan 14's T47 makes the same change).** q is independent of L only inside a template's range,
    where the clamp does not act; past it the law holds f while the shape still varies with L (the
    spread of q over L = 0, 0.5 and 1 at s = 1: Mars 0.10, Jupiter 6.7 × 10⁻³, Neptune
    4.8 × 10⁻³, Venus 3.6 × 10⁻³, the Moon 1.1 × 10⁻⁴; airless ice at s = 0.82, 3 × 10⁻⁴). The
    ruling's "q equal at L = 0 and 1 to 10⁻⁴" is replaced by two tests: inside each range,
    unclamped, the disc-integrated phase equals Φ_t^s for any L to 10⁻¹²; and each template's q is
    solved at its own L.
  - **The planets' V** is checked as MH2018's zeroth-order term plus the law's dimming against the
    paper's equations written out in the test, for all eight planets, at 0.01 mag (tighter than
    the plan's 0.01–0.03 and 0.035, since the law reproduces its template up to the table, s and
    the clamp); Earth in flux, within 30% at 10–150° against eq. 5 until R07.T4.d, then within
    0.5% at 0–135° against Robinson 2026's eq. 14. p is checked by T4.a against V(1, 0) and the
    radius, not here.
  - **The crescent.** The fixture gives q in V only, so Mercury's test splits q by ±5% (q_R 1.05
    q_V, q_B 0.95 q_V) on the fixture's p; B − V then grows by about 0.11 mag from opposition to
    100°, inside Design note 5's 0.1–0.2.
- **Deviations in T14.a, as built** (2026-10-02). `post/glare.ts`, `post/bloom.ts` and an added
  `post/nnls.ts` (Lawson and Hanson's non-negative least squares), each with tests.
  `bloomKernel(setting: QualitySetting, role, radPerPx, eye: EyeObserver)`: first built on a
  literal `"high" | "low"` stand-in, switched to R05.T7.b's `QualitySetting`
  (`view/quality/qualitySetting.ts`) once it landed; `eye` is an argument as for `glareSpread`,
  so that it builds before R06 (approved by the orchestrator). `BloomKernel` extends `BloomLevels { firstLevel, levels }`:
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
  at distance d beyond the limb, as max(point, half-plane) (withdrawn 2026-10-02, its energy unbounded; replaced by the
  equal-area rectangle, see T14.b as built), tested against a brute-force disc
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
- **Deviations in T13.a, as built** (2026-10-02). `post/autoExposure.ts`: `meteredLuminance(h, window)`
  (window [0, 1] by default; 0 when the window holds no counts), `smoothEv`, `programTriple`,
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
  view as `SOURCE VIEW`, and the meters `AVG`, `LIT` and `DARK` as pressed-state buttons in Design note 10's order (the guide has none yet; T16 drafts it),
  reachable by Tab and pressed by Enter or Space; with no reading it says `NO IMAGE TO METER`
  and holds the meters back (`aria-disabled`, focusable). It takes `meter`, an
  `ExposureReading | null` and `onMeter(mode)`. **Not yet mounted in `ViewDisplay`**: no view
  meters an image until T7 makes the photorealistic view and its `AutoExposure`; T7 mounts it
  beside `ExposurePanel` (a few lines of `ViewDisplay`), so that no control stands on screen with
  nothing behind it. The labels `METER AVG`, `METER LIT`, `METER DARK` are T16's guide draft. The
  by-eye checks (a lit planet on black, a star entering the frame, the cockpit turning to a
  planet), which settle the smoothing speeds, wait on T7 and are pending by hand for the owner:
  `just client` with a photorealistic `VIEW` on the development machine.
- **T13 after review, as built** (2026-10-02). `meteredAverage(h, window)` is what the controller
  meters: `null` when no pixel counts, and for a frame whose counted pixels all fall below the
  histogram's range (bin 0, as after a cut from a sunlit planet to a dark sky) the range's floor
  2⁻¹⁴ ÷ the pre-exposure, an upper bound, so that the exposure steps darker and the frame comes
  into range; metering its mean of 0 as nothing to meter locked `AUTO` in a system inhibit it
  could not leave (a test drives the cut and the recovery). A histogram with nothing the meter
  weighs (`LIT` with no lit body) is treated as no histogram: `AUTO` holds until
  `METER_TIMEOUT_S`, then reads `INHIBITED · NO IMAGE TO METER`, which goes beyond the guide's
  definition of that status. T16 replaces it with the meter's own status, `NO LIT SIDE` and its
  twins (decision-r07-t8a-meter). Under `AUTO` `onMetering` receives the smoothed, applied EV100,
  so R02's `auto.ev100` is the applied value and `meteredEv100` the metered one; a system inhibit
  resumes and smooths in the same step; the operator's `setAuto` and `enable` take
  `meteredEv100` and set the exposure there at once, unsmoothed (R02's commands as built).
  `setMeter` changes the reading's meter at once, and histograms under its weights arrive one to
  three frames later. `MeterControl` takes the
  operator's `meter` as its own prop, so the chosen meter shows (`METER LIT`, the button
  underlined, as the time control's step is) while nothing is metered; held-back buttons are
  described by `NO IMAGE TO METER` (superseded: T8.a, parts 3 and 4, holds none back and describes
  the pressed one only); the panel is titled `Exposure meter`, its buttons grouped
  under the legend `SELECT`. **Left to T7**: mount `MeterControl` beside `ExposurePanel`,
  subscribed to the view's `AutoExposure` through `useSyncExternalStore` and throttled to about
  4 Hz at 0.1 EV, not passed down from `ViewDisplay`'s state; make the `AutoExposure` with R06's
  `DEFAULT_VIEW_CAMERA`; pass `exposure.meter` and the stride into each `HistogramRequest`; and
  set the next frame's pre-exposure from `reading().ev100`. **For T16's draft and the owner**:
  the words `SOURCE`, `SELECT` and the panel title, beside `METER AVG`, `METER LIT` and `METER
DARK`; the source shows the raw view id upper-cased until T7 names views as the label block
  does.
- **Deviations in T14.b, as built** (2026-10-02). `post/bloomDown.wgsl` and `post/bloomUp.wgsl`
  are materials (`BLOOM_DOWN_MATERIAL` `GLARE DOWNSAMPLE`, `BLOOM_UP_MATERIAL` `GLARE UPSAMPLE`,
  in `post/bloomChain.ts`) drawn on a full-screen triangle (`fullScreenTriangle`) into one
  `rgba16float` target per level, not post-processes: a post-process's input is its own frame's
  draws, and a chain level reads another target. Every tap is a bilinear sample of four
  `textureLoad`s, clamped, so that the first pass thresholds per texel before filtering (the
  threshold is not linear) and the passes match `bloomDown` and `bloomUpTent` sample for sample.
  `BloomChain` (`create`, `run(hdrColour, thresholdPreExposed)`, `resize`, `setKernel`,
  `levelOne`, `levelOneWeight`, `dispose`) takes the HDR colour as an argument (decision
  2026-10-02, item 1) and times every pass under `BLOOM_PASS` (`"bloom"`, several submissions a
  frame). The coarsest level is not rewritten at its weight: the first up pass weights it
  (`coarseWeight`), and the CPU twin `bloomChain` was changed to match. `rgba16float` throughout,
  since R01's probe reads `toward-zero` for `rg11b10ufloat` on both GPUs. The last step and the
  glare sources live in `post/glare.wgsl`, a library the tone-mapping pass concatenates
  (`bloom_excess`, `bloom_tent`, `glare_pixel_direction`, `glare_angle` by atan2 of cross and dot,
  since acos loses small angles in `f32`, `glare_veil`); the sources are a storage buffer
  (`packGlareSources(sources, preExposure, terms)`, 48 bytes each: direction in the scene's
  camera-relative frame, which the pass turns by `frame.viewRotation`, radius, excess in the
  target's units, solid angle, and each narrow term's inside level), the spread function five
  `vec4f` uniforms (`packGlareTerms`). `glare.ts` gains `GlareSpreadTerms`,
  `glareSpreadTerms(role, eye)` and `evaluateSpread`, one term form both twins evaluate (the
  narrow `(1 + (θ/c)²)^−1.5` terms, Lorentz, root, quadratic, constant, Gaussian), and
  `glareSourceSolidAngleSr` is written 4π sin²(ρ ÷ 2), which keeps a star's digits. **The
  near-limb veil** (orchestrator's rulings, 2026-10-02: the half-plane max was withdrawn, its
  energy being unbounded): each narrow term is integrated exactly over an equal-area rectangle
  facing the pixel, x ∈ [θ − ρ, θ + ρ], |y| ≤ πρ ÷ 4 (`poissonOverRectangle`, the solid angle of
  a rectangle from height c, Mathar 2005), in a cancellation-free form (science check: the plain
  difference of two atans lost every `f32` digit far from small sources, up to 4,500× wrong);
  inside the disc each such term takes the level that keeps the source's energy
  (`rectangleInsideLevel`), where the clamped disc is white through AgX anyway; the broad terms
  stay point-form. Against a brute-force quadrature over the disc, the Sun at 1 au at 1080p across
  60°: +7% to +13% from a quarter of a pixel to 8 px beyond the limb, 0.2% at 64 px, the point
  form alone 0.19–0.54 there; the veil integrates to L_ex Ω within 0.2% (the step at the limb).
  Limits recorded: the broad terms as a point fall to −18% near the limb of a 10° source and −29%
  at 19.5°; a camera view, with no narrow term, is the point form, 0.75 of the truth at the Sun's
  limb (a Lorentz rectangle would fix both; for the owner with the camera PSF); and a body in
  front of the disc (a transit) receives the inside level on its pixels. Smoke checks
  (`smoke/bloom.ts`, 512 × 512, a clamped disc of 10 px): the device's chain equals its CPU twin
  to 0.5%, stored plus injected energy equals the unclamped to 1.5%, and the WGSL veil equals its
  twin to 1% at five pixels beyond the limb; a vitest emulates the WGSL rectangle in `f32`. **The
  bench is pending** (under 1 ms, and 2–3 ms with tone mapping): T17's harness, on a quiet
  machine under `--hyperion-gpu-timing`, the RTX 3080 at 1080p and the owner's UHD 620 at 720p.
- **Deviations in T15, as built** (2026-10-02). `post/tonemap.wgsl` and `post/tonemap.ts`
  (`TONEMAP_MATERIAL` `IMAGE`, `tonemapDraw`, the twin `tonemapTexel`, `srgbEncode`, `tpdf`) and
  `post/blueNoise.ts` (`blueNoiseTile`, 64 × 64 by Ulichney's void-and-cluster from a fixed hash,
  uploaded as `r16float`). The pass is a material on the full-screen triangle drawn onto the
  canvas: it samples the HDR colour bilinearly at the canvas's resolution (the upscale), adds
  w₀ excess(L) and the tent of `BloomChain.levelOne`, each glare source's veil, multiplies by the
  exposure over the pre-exposure, applies `agxSprite` (AgX less its floor, as the wireframe's
  sprites write it, so an isolated star on black is identical in both styles before the dither:
  Design note 9 allowed the floor to stay, and taking it off makes the identity exact), encodes
  by the sRGB curve and adds the TPDF dither, `tpdf` of a blue-noise threshold read at three
  offsets for the three channels, ±1 LSB, static. **Engine extension** (decision 2026-10-02, item
  6; R01's Risks carry the pointer): `FrameSubmission.encoding?: "srgb-view" | "in-pass"` (a
  view writes through its canvas's own format under `in-pass`), `FrameSubmission.colourLoad?:
"clear" | "load"` (a frame without post-processes keeps the colour and depth an earlier
  submission drew, for T16's symbology), and `RenderTargetFormat` `"canvas-in-pass"` for
  `createMaterialAsync`; defaults unchanged, every earlier smoke check unchanged (`view.ts`,
  `drawing.ts`, `engine.ts`, `types.ts`, appended). Smoke checks (`smoke/tonemap.ts`): the pass
  in WGSL equals `tonemapTexel` within one code at 512 pinned texels (a 20-stop grey ramp and
  three colours), the dither moves no texel by more than one code and is unbiased over a flat
  field, a half-resolution flat field upscales evenly, and a following `load` pass keeps the
  image. **The encoding near black, settled** (orchestrator, 2026-10-02): Filament's `pow(v,
2.2)` stays in R02's `agx`, both styles, R02 untouched. Against Blender's AgX Base sRGB on the
  grey diagonal (its `AgX_Base_sRGB.cube`, GPL, read once and not committed; sixteen derived
  codes are pinned in `tonemap.test.ts` with attribution), the pass is within 6 codes from −3 stops up and up to 16 in the toe, worst at −5 stops (7 against 23; 14 at −6, 13 at −4), shadow tones of a lit body as well as near black. Writing the sigmoid's output directly
  as the encoded value halved the toe's gap (8 codes) but was rejected: its near-linear toe made
  a faint star's displayed total vary with its sub-pixel position from 0.83 to 1.37 of the
  centred star's (R02's constancy test, bounds 0.90–1.05), stars that would twinkle as the camera
  moves. **Finding for R12's look audit**: the toe's gap is the seventh-order polynomial's; an
  analytic AgX sigmoid or a LUT of our own could close it, provided star totals stay constant
  across sub-pixel positions. By hand, pending for the owner (needs T7's photorealistic view): a
  Sun-like star in frame with a lit planet, hues holding in the highlight. The bench is T17's.
- **T14.b and T15 after review, as built** (2026-10-02). The tone-mapping pass's uniforms are typed
  (`TonemapUniforms`, `tonemapUniforms`); `rectangleInsideLevel` keeps its outer integral as c⁴
  times a function of ρ ÷ c at 0.05% steps, so a source whose radius changes every frame costs a
  lookup and the store stays bounded; `packGlareSources` packs −1 as the level of a term a source
  is a point for, so both twins take the same branch; `packGlareTerms` refuses more than one
  Lorentz or root term; the dither leaves black at code 0 (dithering 0 scattered code-1 texels
  over empty space, an eighth of them); the engine's choices are pure functions with tests
  (`canvasPassFormat`, `sceneLoadOp`, `pipelineOutputs`). The bloom smoke check also compares the
  encircled energy about the disc, device against twin, to 1% at six radii; its 1.5% energy check
  adds the injected veil in closed form (L_ex Ω), the device's veil being checked against its twin
  at five pixels. Every bloom submission is timed under `"bloom"`, several a frame: R12 sums them.
  The CPU twin of the last step holds where the pass draws at the internal resolution; upscaled,
  the pass tents U₁ to the canvas's pixels and thresholds the interpolated colour. `just
test-render` (SwiftShader, `default` and `no-subgroups`, 2026-10-02, merged with origin at
  `7cb5d33`): every R07.T12, T14.b and T15 check passed but the T15 twin check, which failed on the
  harness's half-float encoder for subnormal inputs; fixed in `3e51ef3`, to be re-run with the
  owner's integrated `just ci` and `just test-render`.
- **Deviations in T4.c, as built.**
  - **`LunarLambert`'s last field is `table_row`, not `template`:** `template` is a reserved word in
    WGSL. It is the row of `phase_factor_table` that holds the law's f, one row per tabulated law
    (template, L and s), so a caller building laws per texel (R10) points each at a row it made.
    A row's f is fixed when it is tabulated with its law's L and s; a law built per texel with
    another A or L changes only the disc term, so R10 builds a row per (template, L, s) it needs,
    or accepts f from a neighbouring L. `brdfFromTable(law, table, …)` is the reference for that
    case (`brdf` reads the law's own table). R10's Consumes and R10.T10.b still name the field
    `template`; they read `table_row` (passed to the orchestrator for R10's owner).
  - **The table** is an `rgba32float` 2D texture, 361 texels per row (0° to 180° every 0.5°, f in
    r, g, b, alpha 0), read by two `textureLoad`s and interpolated in the shader, as
    `phaseFactorFromTable` does: `rgba32float` filters only with `float32-filterable`, and
    `rgba16float` would err by about 10⁻³. `litBody.wgsl` declares no binding; its includer
    declares `phase_factor_table : texture_2d<f32>`.
  - **The probe.** A library of functions is no material, so `WGSL_CATALOGUE` holds it inside a
    compute kernel, `LIT_BODY_PROBE` (`appearance/litBodyProbe.ts`, `readback: "bit-exact"`), that
    calls `body_brdf` at pinned cases; `smoke/litBody.ts` compares its results with `brdf` to 10⁻⁵
    relative at five geometries (four laws, off-sample phases, Venus's clamped crescent, and one
    law built per texel from a synthetic A_N = 0.23 and L(α) = 1 − α ÷ 2π).
  - **`just test-render`.** The first run (2026-10-03) refused the shader on `template`. After the
    rename, review found the per-texel case's reference re-tabulating f with the per-texel L while
    the kernel reads the borrowed row; `expectedBrdf` now reads the borrowed row (fixed with
    T6.c). `just test-render` (SwiftShader, `default` and `no-subgroups`, 2026-10-03, with T6.c's
    working tree over e641995): all five `body_brdf` checks pass, worst 1.3 × 10⁻⁷ relative; T6.c's
    horizon factors within 9 × 10⁻⁵, eclipse terms within 2.3 × 10⁻⁵, and the stubs exact.
- **Deviations in T6.a, as built.**
  - **`sphereIrradianceFactor(h, phiRad, horizonRad = 0)`** returns H² F, Howell's view factor
    over that of the sphere face-on, so it is cos φ wherever the whole disc is up; `howellViewFactor`
    is exported beside it. A local horizon η > 0 is taken as a plane tilted by η towards the star:
    the disc is cut at φ + η in Howell's form and the light through it weighed by the element's own
    normal (a tangential term with the disc's directions taken as one, an error of order ρ² sin η);
    a horizon below the tangent plane counts as 0. R10.T8.b replaces it with its horizon map.
  - **The oracle** (`lighting/oracle.ts`): `sphereIrradianceBruteForce`, Gauss–Legendre in the
    angle from the disc's centre and midpoint in azimuth, over any `LimbProfile`
    (`UNIFORM_DISC`); it meets the uniform closed form to 10⁻⁵. Its horizon is a cone of
    elevation η all round, which agrees with the tilted-plane model where the disc sits in the
    star's azimuth, the only case tested.
  - **Figures.** H = 3, 11.5 and 215 agree with brute force to 2 × 10⁻³; the terminator is
    59.3 km wide at 1 au on 6,371 km; E ÷ E_zenith at the geometric terminator is 9.87 × 10⁻⁴
    (uniform) and 9.27 × 10⁻⁴ (the brainstorm's polynomial law); a planet at 3 stellar radii is lit
    to 109.5°; a 5° horizon hides a star 4° up. **The limb-darkening error at 19.5°** is 0.469% of
    the face-on value at φ = 90° with the brainstorm's polynomial (Design note 6's 0.45% is a
    linear law's, u = 0.6), and 0.607%, 0.474% and 0.390% with the Sun's B, V and R power-2 laws
    (Maxted 2018, Table 2); the tests bound them at 0.48%, 0.62%, 0.49% and 0.40%
    (decision-r07-dn6, 2026-10-03). The error falls about as 1 ÷ H (Sun V: 1.28% at H = 1.5,
    0.13% at H = 10). The terminator's 2 ÷ (3π H) of face-on is the H ≫ 1 limit, 3.5% low at H = 3.
- **Deviations in T6.b, as built (decision-r07-dn6, 2026-10-03).**
  - **`annulusEdges(c, alpha, k)`** returns an `AnnulusSet`: the K + 1 edges by equal concentric
    error ("equal dip", the minimax flux-exact partition: a level bisected so that every annulus's
    worst concentric error is equal, each edge the furthest at that level, per star and channel on
    the CPU in `f64`), each annulus's exact share of the power-2 flux, and the level `dip`, the
    predicted worst error; `annulusVisibleFraction(annuli, ratio, separation)`
    is the eclipse term in stellar radii, and `eclipseVisible(disc, from, occluders, k)` takes a
    `LimbDarkenedDisc` (centre, radius and one channel's c and α), not `HostDiscDto`, which
    carries no position. The annulus construction is the one function a scheme change touches.
  - **The error bounds**, against the exact integral oracle `eclipseIntegralVisibleFraction`
    ((1 − c) A(1) + c ∫₀¹ A(√(1 − t^(2/α))) dt, which the 400-annulus `denseAnnulusVisibleFraction`
    meets to 10⁻⁵) over 41 ratios log-spaced over 0.1–30 by 41 separations over 0 to 1 + ratio:
    the Sun's B, V and R (Maxted 2018, Table 2: 0.846/0.830, 0.771/0.707, 0.712/0.625) at most
    0.73%, 0.58% and 0.48% at K = 4 and 2.8%, 2.2% and 1.85% at K = 2; c 0.71, α 0.6 (the R band,
    not V as first labelled) 0.47% and 1.78%; c 0.5, α 0.5 0.28% and 1.05%. The equal dip predicts
    the grid's worst to 3 × 10⁻⁴. The edges replace the first build's uniform-in-μ ones (0.73% and
    2.6% for c 0.71, α 0.6). The WGSL `eclipse_visible(star_radius, annuli: DiscAnnuli,
occluder_radius, separation)` takes the CPU's edges and fluxes (`DiscAnnuli { outer, flux,
count }`, at most four), not c and α.
  - **Still short of Design note 6 (for the owner).** K = 4 meets 0.62% for the Sun in V and R,
    not B (0.70%); the worst in Maxted's grid is the B channel of a 4,500 K dwarf, 0.87% at K = 4 and
    3.4% at K = 2. No flux-exact scheme reaches 1.5% at K = 2 for solar laws; freeing each
    annulus's flux (the total exact) with optimised intensities reaches 1.49% (V), 1.24% (R) and
    1.88% (B) at K = 2, but needs per-star numerical optimisation. **K = 3 with equal dip gives
    0.8–1.2% for the Sun: `DISC_ANNULI_LOW` is raised to 3 by the orchestrator's ruling under
    the owner's delegation (2026-10-03)**, for one more overlap term per lit texel while an
    eclipse is on; tested at the Sun's B 1.30%, V 1.02% and R 0.85% (about 5% over the measured
    1.23, 0.97 and 0.81%), K = 2 kept as the scheme's own check. The term treats the disc as flat in angle, an error of order ρ² ÷ 12, about 1% at
    ρ = 19.5° (estimated, not measured), for close-in hosts.
  - **Exports and assumptions.** `eclipseVisible` adds the fractions each occluder hides, which
    assumes the occluders do not overlap one another, as holds for the shadow cones `occludersFor`
    keeps; it measures the separation by atan2 of the cross and dot products.
    `annulusVisibleFraction`, `AnnulusSet`, `LimbDarkenedDisc` and the oracles
    `eclipseIntegralVisibleFraction`, `denseAnnulusVisibleFraction` and `power2Profile` are
    exports beyond Provides.
- **Deviations in T6.c, as built.**
  - **`occludersFor(body, stars, bodies)`** takes `LightingBody` and `LightingSphere` (centre and
    radius, m), not `SceneFrameBody` and `HostDiscDto`: neither carries both, the frame having no
    radius and the disc no position. `lightingBodyOf(frame, radiusM)` makes a lighting body of a
    `placed` entry at its geometric centre, and returns `null` for a `contact`, which never
    occludes and is never eclipsed (decisions-r06-r07, item 4), or for a body of unknown radius.
    `umbraRadius` and `penumbraRadius` are exported for the tests: Earth's umbra 4,600 km and
    penumbra 8,175 km across at the Moon's distance; the Moon's umbra ends short of Earth.
  - **The WGSL** (`litBody.wgsl`): `sphere_irradiance(h, phi, horizon)` and `howell_view_factor`;
    `eclipse_visible(star_radius, annuli: DiscAnnuli, occluder_radius, separation)` for one
    occluder in angular terms (the CPU builds the list, the angles and the annuli, T6.b); `circle_overlap_area`;
    and the stubs `ring_shadow_on_body(p, sun) -> vec3f` (1), `atmosphere_sun_transmittance(
altitude_m, mu_sun, latitude_rad, sun_azimuth_rad) -> vec3f` (1) and
    `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad) -> vec3f` (0), with R08.T9.b's
    and R11's full signatures. The lit point's lighting, which calls them, is T8.a's.
  - **The probe.** `LIT_BODY_PROBE` gains a second case array, `LightingCase` (kind, annuli, six
    values and the annuli's edges and fluxes, 64 bytes), so one kernel holds the whole library (an `auto` layout binds only what the entry
    point uses, and the engine binds every declared binding). The smoke harness checks five
    horizon factors and five eclipse terms against their TypeScript twins to 2 × 10⁻⁴ absolute
    (WGSL's `f32` `acos` and `atan` are allowed some 10⁻⁴), and the stubs' 1, 1 and 0 exactly.
- **Deviations in T3, as built.**
  - **`starIlluminance(disc, distanceM)`** returns E_c = π L̄_c sin²ρ in display order (r, g, b)
    from `HostDiscDto`'s B, V, R; inside the star sin ρ is held at 1. R06 builds the channels as
    the photopic mean times the star's linear Rec. 709 colour at unit luminance, so the "V-weighted
    sum" is the channels' Rec. 709 luminance, `photopicIlluminance`, with R06's own row
    (`CHANNEL_LUMINANCE`, `star_colour.rs`'s `LUMINANCE_RGB`, which differs from Filament's row in
    `starColour.ts` in the fourth decimal). `shiningStars(illuminances)` applies
    `STAR_CUT_RELATIVE` to photopic illuminance and returns the kept indices.
  - **`aHostDisc(overrides)`** (`test/litFixtures.ts`) builds a disc as R06's `host_discs` does,
    from an absolute V (the Sun's 4.81, Willmer 2018) with `lux_per_v0` 1, the Sun's V-band limb
    law in every channel (c 0.7837, α 0.6893, Claret and Southworth 2022 as R06 pins it) and an
    illustrative warm white of unit luminance. The Sun at 1 au gives 1.287 × 10⁵ lx (the
    brainstorm's 1.28 to 0.6%), and the disc form agrees with the magnitude form. The light-time
    direction error is v ÷ c for the Sun's reflex speed about the Sun–Jupiter barycentre,
    4.16 × 10⁻⁸ rad.
- **Deviations in T2.a, as built.**
  - **A thirteenth template, `lambert`** (approved by the coordinator, 2026-10-03): Φ_t is Lambert's
    closed form to 179° (f held at 1 beyond, where Φ_L reaches 0), L = 0, provisional and labelled.
    `PROVISIONAL_PHOTOMETRY` is `lawFor(0.2, 1.5, "lambert")`, an exact Lambert sphere (s = 1,
    q = 1.5); no body class maps to the key.
  - **`appearanceFromWire(body, rotation)`** (`appearance/fromWire.ts`) returns the photometry,
    the figure and the labels: every body takes `PROVISIONAL_PHOTOMETRY` with
    `BODY PHOTOMETRY: NOT YET MODELLED`, and a sphere of `bulk.radius_m` whose pole is the body-fixed z
    axis through `bodyFixedRotation` (`null` today); a body without a granted radius has no figure
    and stays R02's mark. `BodyPhotometry`, `AppearanceLabel` and the `BodyFigure` re-export live
    there. A `contact` frame entry has no summary here; it never occludes and is never eclipsed
    (`lightingBodyOf`, T6.c; decisions-r06-r07, item 4).
- **Deviations in T5, as built.**
  - **Inputs.** `litRegimes(bodies, camera, viewport, previous)` and `painterOrder(bodies, regimes,
hosts)` take `LitSphere`s (identifier, centre from the camera in `f64`, equatorial radius) and
    `HostSphere`s (star index, centre, radius) instead of `SceneFrameBody` and `HostDiscDto`, which
    carry no radius and no position respectively; `camera` is R02's `ProjectionCamera`. Sizes use
    R02's `angularDiameterPx` (the centre pixel's tan-based scale). A body new to the view starts as
    a point; a `mesh` keeps the disc side of the hysteresis.
  - **Promotion** is its own function, `promoteOverlapping(regimes, footprints, depthWriters)`,
    over screen circles (`ScreenCircle`): a disc overlapping a mesh body or other depth-writing
    geometry is promoted, and promotion spreads through chains of overlaps until nothing changes.
    Its callers (T8.a, T9) supply the footprints.
  - **The painter's ties** keep bodies before hosts and then identifiers' order, so the sequence
    does not depend on input order. The truth for the 10⁴ random disjoint pairs is each shared ray's
    first analytic hit (no marching is needed for spheres): no disagreement over some 5 × 10⁴ rays.
  - **The brainstorm's figures** scale by width ÷ field (1,833 px/rad at 1080p across 60°): a
    Jupiter of 3 px to 8.7 × 10¹⁰ m and ten scale heights over 2 px to 2.5 × 10⁸ m (Jupiter, 27 km)
    and 5.5 × 10⁸ m (Saturn, 59.5 km), each reproduced to 2%. R02's projection uses the centre
    pixel's 1,663 px/rad, which brings each distance in by 9.3% (Jupiter's disc to 7.9 × 10¹⁰ m);
    `GAS_GIANT_FULL_PASS_BOUNDARY_M` keeps 10⁹ m, above both.
  - **`BodyAppearance`** lives in `appearance/bodyAppearance.ts` with `bodyAppearance(body, wire,
regime)` (`null` for a body without a figure, which stays R02's mark) and `discSurfaceOf`;
    `DiscSurface` has its `uniform` case only, T8.b adding `class-map`. `aLitBody` is in
    `test/litFixtures.ts`. No template is mapped to a body class here: the photometry is T2.a's
    provisional law until plan 14's section (T2.b).
  - **`painterOrder` takes no camera**: centres are already relative to it in `f64`.
    `spherePower` and `litSphereOf(id, centreM, figure)` (the equatorial bounding sphere, which
    callers, R08's included, use to build each `LitSphere`) are exported beside it; ties rank
    bodies before hosts, then identifier or star index numerically.
  - **Hysteresis** is applied to the 3 px threshold only (up at 3.3 px, down at 2.7 px);
    `GAS_GIANT_FULL_PASS_BOUNDARY_M` is a constant whose hysteresis R08's caller applies, and the
    tests hold it above both giants' derived distances at both scales. `litRegimes` never returns
    `mesh`: a mesh comes from `promoteOverlapping` or, later, R10's hand-over. Footprints are
    bounding circles, so promotion may over-promote an oblate body but never under-promote it.
  - **Tests.** The pairs test fixes the camera at the origin and places the spheres at random
    (only relative geometry matters), each a disc, a point or a host at random; the oblate test
    places a moon whose power lies between Saturn's equatorial and mean spheres'; the
    order-independence test starts from an empty `previous` map. The acceptance also runs
    `src/renderer/src/view/appearance/bodyAppearance`, outside the `view/bodies` filter. **For the
    owner**: the brainstorm's 9 × 10¹⁰, 2.5 × 10⁸ and 5.5 × 10⁸ m are at width ÷ field; at R02's
    centre-pixel scale they are 9.3% nearer (the 10⁹ m boundary holds either way). _Decided
    2026-10-03 by the orchestrator under the owner's delegation:_ R02's centre-pixel scale is the
    reference; Design note 1 and T5's text now give 7.9 × 10¹⁰, 2.2 × 10⁸ and 4.9 × 10⁸ m, with a
    note of the brainstorm's scale; the brainstorm is unchanged; the tests assert the centre-scale
    values.
- **Deviations in T7, as built.**
  - **Files.** `photoreal/passes.ts` (`PHOTOREAL_PASS_LABELS`, which take the post passes' own
    `HISTOGRAM_PASS`, `BLOOM_PASS` and `TONEMAP_PASS`; `PassList` of `PassEntry { label, owner,
built }`; `photorealisticPasses(setting)`), `photoreal/style.ts` (`RENDER_STYLES`, `styleName`,
    `styleRefusal`, `otherStyle`, `withStyle`, `STYLE_TOGGLE_KEY`) and `photoreal/sceneTarget.ts`,
    in place of the plan's `photoreal/style.ts` alone. The pass list's other-plan slots carry R10's
    built `terrain` label (R05's terrain pass, owner R10) and the lane's provisional names for the
    unbuilt ones, `sky` (R06), `atmosphere` (R08), and `rings`, `clouds` and `ocean` (R11), each
    `built: false`, which those plans rename when they fill them; both settings share one order.
  - **The switch is built and unmounted** (the orchestrator's ruling, 2026-10-03): `StyleControl`
    (`displays/view/StyleControl.tsx`, `renderStyle` prop) and the toggle key are not mounted in
    `ViewDisplay` until R07.T8.a draws lit bodies, so that no view switches to an empty image; the
    label block's `STYLE` line and the canvas's accessible name read the camera's style. A refused
    style holds the button back with the guide's `GRAPHICS SOFTWARE ADAPTER: photorealistic style
not available`. **The key `4` is a provisional ruling** (the orchestrator, 2026-10-03; digits
    and brackets are the view's single keys, letters the flight keys): before T8.a mounts it, the
    guide's key-assignment rules and the view and console keymaps are checked for a collision and
    the ux-reviewer passes the control. The ux-reviewer's check (2026-10-03) found no collision
    (VIEW's `1`–`3`, brackets and `+`/`-`; the flight letters; the console's `F1`–`F4`; SYSTEM's
    `4`, a one-year time step, lives on another display, never live at once) and no key-assignment
    rule in the guide beyond "single-key bindings shown on the control"; the legend is `4 STYLE`
    on the buttons' group (`aria-keyshortcuts` there), as GALAXY's `K STARS`. For T8.a's mounting:
    `ViewKeyAction` gains a style kind and `VIEW_SINGLE_KEYS` its `"4"` (and the comment above it),
    `ViewDisplay`'s key legend is re-checked against the guide, and the panel's layout is checked
    at 1920 × 1080 and 1280 × 720.
  - **The style-switch test** checks that `withStyle` changes only the style (pose, field of view
    and every other field equal), from which the projection, built from pose and field of view
    alone, follows; R02's draw-list test needs no change. The smoke check renders an empty frame
    into a scene target made by `createSceneTarget` (finite, black, alpha `METER_CLASS.other`) and
    resizes it.
  - **The pixel-scale ruling** (the orchestrator, 2026-10-03) is folded into this commit: Design
    note 1 and T5's text give the centre-pixel distances (see T5's entry).
- **Deviations in T18, as built** (2026-10-04).
  - **Files.** `budget/viewBudget.ts` (`ViewBudget`, `PER_CANVAS_OVERHEAD_MS`, `viewBudgets`,
    `photorealisticAllowed`) and `budget/resolutionController.ts` (`ResolutionController`), each
    with tests. Beyond Provides: `ViewSpec { id, slot, style }`, `ViewSlot` (`"primary" |
"instrument"`), `ScaleControl`, `PhotorealisticPermission`, `ONE_PHOTOREALISTIC_VIEW`,
    `GPU_FRAME_SHARE`, `INSTRUMENT_RATE_HZ`, `WIREFRAME_PRIMARY_RATE_HZ` and `frameGpuBudgetMs`;
    `ScaleBounds`, `ResolutionTarget`, `CONTROLLER_TUNING` and `gpuTimeMs`.
  - **`viewBudgets(views, setting)`** takes no `instrumentsOpen` and returns a `ReadonlyMap`:
    `views` are the views shown, one primary and the open instruments in slot order, whether or
    not a 30 Hz instrument draws in a given frame. Instruments are open exactly when some are
    listed; a closed one has no budget and holds no photorealistic slot. R11's
    `viewBudgets(views, setting, …)` reads as two arguments.
  - **`ViewBudget.control: ScaleControl | null`** (the bounds and a `ResolutionTarget { periodMs,
gpuBudgetMs }`) is set only for a photorealistic primary while instruments are open, Design
    note 14 tying the drop to them; its `renderScale` is then the bounds' max, where the controller
    starts. A photorealistic primary alone renders at the bounds' max, uncontrolled; a wireframe
    view at 1, having no internal target (R02 Design note 12). The scale drops only as far as the
    measured GPU time needs. A photorealistic instrument (on low beside a wireframe primary, or
    any on high) is never controlled, a stated limit (decision-r07-t18, item 5): beside a
    photorealistic primary its passes count in the primary's measured frame, and the case left, a
    wireframe primary with a photorealistic instrument, is measured on the UHD 620 in T20.
  - **Rates.** A photorealistic primary at `budget.photorealisticRateHz`: 60 Hz, and 30 Hz on low
    (the low setting's 30 fps at 720p, paced to every second vsync, R05 Design note 21). A
    wireframe primary at 60 Hz on both settings, since the brainstorm's budget gives a wireframe
    view its own 16.7 ms frame on the UHD 620. Every instrument at 30 Hz and full scale, so that a
    wireframe's strokes stay sharp, at R05's secondary streaming priority. **Decided**
    (decision-r07-t18, item 4): on low beside a photorealistic primary at 30 Hz the instruments
    share its rate and are below it by being wireframes on smaller canvases, which meets Design
    note 14's "a lower scale or 30 Hz"; 15 Hz is not taken, and the wireframe primary's 60 Hz on
    low is checked in T20.
  - **The budget.** `gpuBudgetMs` is `GPU_FRAME_SHARE` 0.8 of the nominal period (R05 Design note
    21's headroom row) less `PER_CANVAS_OVERHEAD_MS` per instrument canvas (`frameGpuBudgetMs`).
    The controller measures the frame's whole GPU time, every view's passes (`gpuTimeMs`), so the
    instruments' cost comes out of the primary's margin (brainstorm, "Several views in one
    client") without an estimate of it. A caller may `retarget` from the measured vsync period
    (16.68 ms on the projector).
  - **The low setting's one photorealistic view** goes to the views asking for it in slot order,
    the primary first. `photorealisticAllowed` refuses every other view while one holds it, the
    primary too while an instrument holds it, with
    `NOT AVAILABLE: QUALITY LOW allows one photorealistic view`, the guide's `NOT AVAILABLE` form
    (decision-r07-t18, item 6), which needs no row of its own. The policy keeps no state, so the
    style control and the key `4` ask it before a switch (T19).
  - **The controller.** `new ResolutionController(bounds, target)` (the sketch has bounds only),
    with `retarget(target)` (keeping the scale, restarting the windows and waits, ignoring an
    equal target) and `scale`. `update(gpuFramesMs, intervalMs)` takes the GPU times of the frames
    resolved since the previous update, each frame once (none on some updates, two on others), or
    `undefined` where `GraphicsStatus.timer` is `absent`, in place of the sketch's single
    `number | undefined`, which could not tell a frame not yet resolved from no timer (TypeScript
    review). The caller groups a frame's resolves for `gpuTimeMs`: `PassTimes.frame` counts
    resolves, not animation frames. Kept, and Provides shows them (decision-r07-t18, item 3).
    - From GPU time, cost goes as the scale squared. Two of the last four frames over the budget
      drop the scale to where the worst would take 0.9 of it; a run of 30 under 0.75 raises it to
      where the run's worst would take 0.9, landing short of the top where a fixed part is large.
      The 25% band is far wider than the 65,536 ns quantum: with twelve quantised passes the scale
      settles within 0.02 of the exact run's and stays.
    - From the interval, a frame over 1.5 periods is missed (R05 Design note 21); two misses in
      four drop the scale by 0.85; 120 frames without one probe up by 1.1; a probe that misses
      returns at once. On the synthetic vsync load, failed probes cost 0.16% of frames.
    - After a change 6 updates settle (2 from the interval). A drop within eight first waits of a
      rise fails it and doubles the next wait, up to 16 times; a rise that holds resets it.
      `CONTROLLER_TUNING` is judgement on synthetic loads, to be checked on hardware with T20.
  - **Settings, ahead of T17.** `ViewSettings` gains `internalScaleBounds`, [0.5, 1] on both, which
    T17's text names, since T18's test takes it from `SETTINGS` and T17 is not built; and
    `budget: ViewBudgetSettings { photorealisticRateHz: 60 | 30, photorealisticViews: 1 | null }`
    (high 60 Hz and no limit, low 30 Hz and one): Design note 18's "one photorealistic view" and
    the low rate, placed by R05 Design note 26's rule. T17 adds the rest of Design note 18's.
    T17's text says so (decision-r07-t18, item 2).
  - **Not wired.** Nothing in `ViewDisplay`, `photorealFrame` or `PhotorealRenderer` reads the
    budgets yet; the photorealistic frame still renders at scale 1 (T8.a's entry). T19 builds
    `ViewSpec`s from its slots, paces each view at its `rateHz`, sizes the scene target from the
    controller's scale, feeds it each frame's `gpuTimeMs` (or `undefined` under an absent timer),
    and sends the style control and the key `4` through `photorealisticAllowed`; T19's text says
    so (decision-r07-t18, item 1).
  - **The setting in `VIEW`** is `high` throughout (`ViewDisplay.tsx` 510, `useViewSky.ts` 92):
    T19 reads it from one input so that its tests give `low`, but no task built yet lets the
    running client choose `low`, which T17's and T20's UHD 620 runs need. T17 makes it selectable
    (the orchestrator's assignment, 2026-10-04; see "`VIEW`'s quality setting" above).
- **Deviations in T8.a, as built (part 1: the disc regime, the lights and the phase scene).**
  - **Files.** `bodies/draw.ts` (`planLitBodies`, `pointFlux`, `hostAnnuli`, `LitBodyRenderer`,
    `BODY_DISC_MATERIALS`), with the record, its packer and the shader's `f64` twin in
    `bodies/discShading.ts` (`DiscRecord`, `packDiscRecords`, `rasteriseDisc`,
    `compositeDiscPixels`), the oblate integrals in `bodies/oblate.ts`, the lights in
    `lighting/hostLights.ts` and `lighting/hostDisc.ts`, and `shaders/bodyDisc.wgsl`, registered
    in `WGSL_CATALOGUE` as `BODY DISCS` and `BODY DISC LIMBS`. Point bodies through R06's HDR
    sprites, the sprite row's depth, the photorealistic frame's wiring and the mounting of
    `StyleControl` and key `4` are part 2, after R06.T13 reached `rendering-and-planets`.
  - **Lights (decision-r07-t8a, item 1).** `hostLights(scene, discs)` joins R06's discs to the
    scene's stars by `HostDiscDto.star` as body index; `sceneHostDiscs` takes a kept scene's
    `ViewScene.hostDiscs` and a server scene's held sky's `hosts`; `placeLights` places each at its
    star's centre in the frame (never R06's aberrated placement). `planLitBodies` takes the placed
    lights (`PlacedLight`): per body the brightest two past `STAR_CUT_RELATIVE`, and its
    occluders, the two largest seen from it. A body no star lights draws colour 0 with class
    `other`, and a point no light. The label lines `LIGHTING: PENDING` and
    `LIGHTING: STAR DISCS NOT RECEIVED` (`lightingStatement`) are the orchestrator's wording under the
    owner's delegation, drafted for the guide beside `TERRAIN: STREAMING`, and are shown from
    part 2. `sunLikeHostDisc` (moved out of `test/`, `aHostDisc` calls it) has an illustrative
    colour and the Sun's V limb law (Claret and Southworth 2022 at log g 4.5, α 0.6893; 0.6884 at
    4.438) in every channel, so a kept scene's eclipse is achromatic until a fixture from R06's
    `host_discs` exists.
  - **The disc's two draws (decision-r07-t8a, item 3).** One source, two materials: the interior
    (blend none) where all four corner rays meet the body (the silhouette is convex), writing the
    pure-pixel class (`litBody` where every shaded point is lit directly above
    `LIT_IRRADIANCE` = 10⁻⁵ of face-on, `unlitBody` where none is, `other` where they are mixed or
    no star shines); the limb (premultiplied) where any corner lies outside or within
    `LIMB_OVERLAP_PX` = 10⁻³ px inside the limb, keeping the class beneath. A pixel at the
    threshold is drawn by both, never by neither.
  - **Sampling.** 8 × 8 cells a pixel below 32 px, one inside and 4 × 4 on the limb above. Plain
    point sampling missed the 1% at the 3 px switch for a crescent (15% at 150°, the lit sliver
    0.2 px deep), and the ruling's fallback did worse (a 64-point R2 sequence 23–44%, a grid
    rotated by atan ½ 32–39%). So near the limb each cell's profile across the limb's local normal
    (a trapezoid, the convolution of boxes of widths h|n_x| and h|n_y|) weights its depth
    integral, by three Gauss points in u = √δ per piece, exact where the light goes as
    A + B√δ + Cδ, as μ and a crescent's μ₀ do at the limb; the count is not raised. The sweep
    (centres on a 4 × 4 sub-pixel grid; phases 0°, 90° and 150°; f = 0 and 0.098; 3, 3.3 and 6 px)
    is within 0.8% everywhere but 6 px at 150°, 1.6%, of which 1.1% is the point's own far-field
    error (below), so that the 6 px rows meet the exact near-field integral at 1% instead (part 2,
    follow-up (b)). A lunar law and a spheroid seen from 45° latitude
    are within 1% at 3 px.
  - **The ray in `f32`.** The hit is taken by cross products in the scaled space,
    q′ = −(r̂ × u′) × r̂ − √((a ÷ D)² − |r̂ × u′|²) r̂, never b² − rr · power, which cancels to 7% for
    a body 10⁻³ rad across (the first GPU run read 1.2–60% at 3 px against the twin's 0.4%).
  - **The horizon term stands for μ₀** in `body_brdf`'s disc term (h = μ₀ wherever the whole star
    is up), lighting the soft band; the Lommel–Seeliger term's μ₀ + μ is floored at the star's
    angular radius so that it stays bounded at the limb in that band.
  - **Oblate bodies.** p is defined against π a c equator-on (Mallama et al. 2017's Saturn), so a
    spheroid's disc takes A′ = A [L + ⅔(1 − L)] ÷ [L + (1 − L) m(0)], m(β) = ∫ μ² dS ÷ ∫ μ dS at
    zero phase (1.022 at f = 0.098, L = 0). A spheroid's point integrates the same law over its
    figure, F = (E ÷ π)(a ÷ Δ)² A′ f(α) K (`spheroidGeometricIntegral`, 96 × 192 midpoint,
    remembered by direction to 10⁻⁴), in place of the plan's π a b′ Φ(α), which errs by 4.2% at 90°
    and 10.9% at 150° for f = 0.098 seen from 45° latitude. A sphere keeps the closed form.
  - **The point is the far-field limit.** From a finite D the visible cap stops asin(a ÷ D) short
    of the hemisphere and each element's weight μ ÷ r² changes by (a ÷ D)(3μ² − 1), which dims a
    crescent: an `f64` surface integral puts the disc 0.05% (90°) and 0.55% (150°) below the
    point at 3 px, 0.11% and 1.10% at 6 px, 0.53% and 5.4% at 30 px (science-checker, 2026-10-03).
    Only the switch at 3 px matters to continuity.
  - **Eclipses.** A disc's samples take the eclipse term of each of its (at most two) occluders,
    their hidden fractions added; a point takes it from its centre. In a penumbra the two agree to
    2% for a Moon-sized body 3 px across. An occluder beyond the star hides nothing; a point inside
    one sees none of it.
  - **Tests.** `view/bodies` (the sweep, the extents of a Saturn-like f = 0.098 disc at 100 px to
    half a pixel, classes, eclipses, the plan's order, the records' layout, the renderer's draws,
    table and buffers against R05's counting engine), `view/scenes/phaseScene.test.ts` (the limbs
    and the geometric terminator, μ₀ = 0, of the 0°, 90° and 150° planets against an `f64` oracle to
    half a pixel; the soft band's 0.25 px is not the terminator), `view/lighting/hostLights.test.ts`.
    `just test-render` (SwiftShader, `default` and `no-subgroups`, 2026-10-03): a 20 px disc's 368
    texels within 0.23 of their tolerance (0.4% relative + 10⁻⁴) of the twin, and within 0.15 in a
    moon's shadow (89% of the light kept); classes 1, 2 and 3 as the twin's; at 3 px the summed
    flux within 0.42% of the point's at 0°, 90° and 150° over four placements; the Saturn-like
    extents 107.38 and 96.85 px against 107.38 and 96.86.
  - **Risks (decision-r07-t8a).** Until R06.T11 serves `sky`, a live photorealistic view draws
    its bodies unlit (colour 0, class `other`) under R02's cased marks with a `LIGHTING:` line. A
    body under about 5 px offers `LIT` and `DARK` few pixels and a point none, so they report
    `NO IMAGE TO METER`. R06's host disc stays hard-edged with class 0 on every touched pixel;
    anti-aliasing it would need an alpha-writing blend mode in R01 (an owner option). Overlapping
    limbs of two discs in one pixel carry the usual coverage-over error, under a pixel.
- **Deviations in T8.a, as built (part 2: points, the photorealistic view and its mounting).**
  - **One PSF (decision-r07-t8a, item 2).** Point bodies are sprite records through R06's
    `POINT SPRITES HDR` (R02's `starSprite.wgsl`, the identity tone step); no `bodyPoint.wgsl` and
    no `psf.wgsl`. The sprite row's `z` now carries reversed-Z depth: stars and small host discs
    write 0 and point bodies near ÷ distance, checked through both sprite materials (a star behind
    a plane 1 m ahead hidden, a point 0.5 m ahead drawn). `spriteRecord` and
    `exposedSpriteRecord` are exported from `wireframe/drawList.ts` as a pure refactor; R02's stars
    and R07's points are packed by them. One instanced draw per run of consecutive points, each run
    with its own small buffer.
  - **The photorealistic frame** (`photoreal/renderer.ts`, `PhotorealRenderer`, with
    `displays/view/photorealFrame.ts`), in Design note 8's order of what is built: R06's band, its
    baked cube's HDR draw and the star sprites (the sky's, or R02's interim field until it arrives,
    and the host discs under three pixels) into the view's scene target as R06's `sky` pass
    (`SKY_PASS_LABEL`, its slot now built); then, loading it, the painter's sequence as `discs`
    (R06's host discs at their `host` entries, keyed by `HostDiscDto.star`, disc bodies and point
    bodies); then the bloom chain, the tone-mapping pass onto the canvas in-pass with R06's glare
    sources, and the wireframe's marks as the `symbology` pass loading it. The pipelines are
    compiled asynchronously before the first frame and again after a device loss (a making a
    restore overtakes is abandoned); until then the view draws its wireframe, its `STYLE` line and
    canvas name stating the style drawn, with `PHOTOREALISTIC: PREPARING` under the label block. Where
    the pipelines cannot be made the view returns to the wireframe and the control holds the style
    back with the fault `GRAPHICS STYLE REFUSED: photorealistic style not created, relaunch to retry`,
    in `--status-caution` while it lasts. The control's
    reasons follow the graphics' condition (`styleRefusals`): the adapter's styles, the software
    adapter's refusal, `GRAPHICS ACQUIRING ADAPTER` before the answer; it is shown only beside a
    drawn view. The internal scale is 1 and the setting `high` until T17 and T18; the pre-exposure
    is the frame's own exposure (the tone-mapping pass's exposure over it is 1).
  - **Small host discs** (R06's sprites for discs under 3 px) are drawn with the star sprites at
    depth 0 before the painter's sequence, not at their `host` entries (decision-r07-t8a, item 2
    (d)): a disc body behind such a host would cover it, which needs the body several au beyond a
    star some pixels across, far below a pixel.
  - **Light positions: a known limit until R07.T10.a** (decision-r07-t8a, follow-up (a)). The view
    places each light, and builds each occluder, at the scene's drawn centre: a server scene's
    `apparentM`, with the camera's light time and aberration. The ruling: drawing keeps the apparent
    places, lighting takes retarded geometric positions (the lit body at its drawn time t_B, each
    star or occluder at t_B − |r_X − r_B| ÷ c, without aberration, translated to the body's drawn
    centre), built by R07.T10.a "Retarded lighting geometry", the first subtask of T10, with no
    protocol change. The error until then: the light direction and terminator by at most about
    10⁻⁴ rad (under 0.1 px for a disc under 1,000 px in radius); an eclipse's shadow by about v·δ,
    δ between 0 and twice the occluder-to-body light time, up to some 77 km for the Moon's on Earth
    (3–6 px on a 1,000 px Earth, against an umbra of at most 270 km), about 50 km for Io's on Jupiter
    against an umbra of 3,600 km. The frame's `geometricM` (the time T, not t_B) is no substitute:
    from 1 au it would put the Moon's shadow some 500 km off. Latent until R06.T11 serves `sky`;
    kept scenes are static, and `PHASE TEST` is exact.
  - **The disc's law.** `DiscRecord` takes `BodyPhotometry.law` directly, `DiscSurface`'s
    `uniform` case without the type; T8.b threads `DiscSurface` through the record and the shader.
    `bodyDisc.wgsl` writes `body_brdf`'s lunar-Lambert expression inline, with the horizon factor
    for μ₀ and the Lommel–Seeliger floor, so a change inside `body_brdf` must be mirrored there.
    The view builds `LitBodyInput`, not `BodyAppearance`, until T2.b.
  - **Sampling, as built (correcting part 1's "the count is not raised").** The grid stays 8 × 8
    below 32 px and 4 × 4 on limb pixels above; each cell within two cell widths of the limb is
    shaded at up to nine points (three Gauss points in each of its profile's three pieces) after
    three limb-angle evaluations, and above 32 px the interior pixels within about 2 px of the limb
    are integrated the same way and classed by those points. The cost goes to T17's bench.
  - **Promotion** (`promoteOverlapping`) is not called: the frame has no depth-writing geometry
    yet (no mesh bodies, terrain or lit hulls); T9 calls it with footprints.
  - **The atmosphere stubs' arguments.** The disc passes altitude 0, latitude 0 and sun azimuth 0
    to `atmosphere_sun_transmittance` and `atmosphere_sky_irradiance`; R08.T9.b's caller supplies
    the geodetic latitude from the spheroid normal and the azimuth from local north.
  - **Mounting.** `StyleControl` sits after `CameraControls` in the side panel and takes the
    refusals. The key `4` is `ViewKeyAction`'s `style` kind (`toggle`), in `VIEW_SINGLE_KEYS`
    through `STYLE_TOGGLE_KEY`, and `commandRun` takes the adapter's `StyleAvailability`. The
    canvas's key legend lists the flight keys only, so `4` is shown on its control, as the
    presets' digits are (ux-reviewer, 2026-10-03). The label block's photorealistic statements are
    `LIGHTING: PENDING` or `LIGHTING: STAR DISCS NOT RECEIVED` (`lightingState`, from `useViewSky`'s
    `pending`) and the lit bodies' labels (`BODY PHOTOMETRY: NOT YET MODELLED` while they take the
    provisional photometry), shown only while the scene has a planet, dwarf planet or moon. The
    guide has draft rows for `LIGHTING:`, `PHOTOREALISTIC: PREPARING`, `BODY PHOTOMETRY: NOT YET MODELLED`, `PHASE TEST`, its bodies' labels and the failure's wording, for the owner. The kept
    scene `PHASE TEST` is in the `SCENE` selector, with a Jupiter-sized `TEST GIANT` at 10¹⁰ m for
    the by-hand Jupiter. `DrawnSky` gains `bandIlluminanceLx`, the cull's, for R06's band layer.
    R06's `BandLayer` and `HostDiscLayer` are made again after a device loss, and the frame's path
    in the loop catches a creation refused between a loss and its restore, drawing the wireframe
    that frame. `test/fakeViewEngine.ts` answers the photorealistic renderer's creations.
  - **Lit bodies in the view** are the scene's planets, dwarf planets and moons: spheres of
    `ViewBody.radiusM`, the pole from the body-fixed rotation where there is one, with the
    provisional photometry. The figure and photometry from the wire (`appearanceFromWire`) reach
    the view with T2.b.
  - **Tests.** `view/photoreal/renderer.test.ts` (nothing drawn before the pipelines; the sky pass,
    then the bodies loading it; the tone map in-pass; the overlay loading; a restore remaking the
    resources and a making it overtakes abandoned; dispose releasing buffers and textures),
    `displays/view/{photorealFrame,styleRefusals}.test.ts`, the style command and statements in
    `viewRun.test.ts`, `4` in `keys.test.ts`, and in `ViewDisplay.test.tsx` the mounted control,
    the key, the control's button and the statements. `just test-render` (2026-10-03, `default`
    and `no-subgroups`): the sprite-depth check through both materials, and a frame end to end on a
    canvas (a planet 20 px across at 60° of phase: lit side green 200, sky 0). The smoke harness's
    Saturn-like extents (part 1) are the TypeScript rasteriser's coverage, the GPU's texels checked
    against it; its "100 px" is at the centre pixel's scale on a 128 px, 60° view, where the disc
    spans about 52° and projects to 107.38 × 96.85 px.
  - **The 6 px rows** (decision-r07-t8a, follow-up (b)): 1% everywhere. At 3 and 3.3 px the disc
    meets the point it switches with; at 6 px, where no point is drawn, the disc meets an exact
    `f64` near-field surface integral of its own law (`nearFieldFlux` in `draw.test.ts`, 400 × 800
    midpoint), and a test pins the far-field point's own error there, 1 − F_exact ÷ F_point =
    6.1 a ÷ D at 150° and 0.59 a ÷ D at 90°, to 0.2%.
  - **By hand, for the owner**: `PHASE TEST` with `4` (the target keys stepping through the 0°,
    90° and 150° planets and the `TEST GIANT`, a Jupiter from 10¹⁰ m) on the development machine
    with `just client`, recorded in the as-built notes; `StyleControl`'s layout at 1920 × 1080 and
    1280 × 720, and the marks' legibility over a bright disc.
- **Deviations in T8.a, as built (part 3: the metering, and the rulings' wording).**
  - **The photorealistic view meters its image**, closing T13.b's note ("T7 mounts
    `MeterControl`", which T7 left to T8.a). `PhotorealRenderer` makes a `HistogramReader` with
    its pipelines (`HISTOGRAM_KERNEL`, compiled with them, made again after a device loss) and takes
    the scene target's histogram each frame under the operator's meter (`PhotorealFrame.meter`;
    stride 2 on the low setting), after the scene target's passes and before bloom;
    `takeHistogram()` hands the latest read-back to the view. The stage's loop holds an
    `AutoExposure` (`VIEW_AUTO_PROGRAM`, Design note 11's f/1.4 and 1/30 s, added to
    `post/autoExposure.ts`): each frame it steps on the histogram that arrived (none while the
    wireframe is drawn, which times the meter out, and a system inhibit holds `AUTO`), the frame is
    pre-exposed and the wireframe's sprites exposed at its applied EV100, an operator's command
    reaches it when the display's control changes to one it did not publish, and at the readout's
    4 Hz the control it moved to reaches the display's state, with its reading. `MeterControl`
    stands beside `ExposurePanel` while the photorealistic image is drawn, and `ExposurePanel`'s
    `meteredEv100` is the reading's, so `AUTO` is offered once an image is metered and
    `NO IMAGE TO METER` is true when it is not. The meter's choice is held above the stage beside
    the exposure (`AVG` by default), so that a new scene keeps both. The loop tells an operator's
    command from its own by what it last read of the display's control against what it last gave
    it, and gives a control only when its readout would change (its level, or its EV100 to 0.1),
    so that the display is not re-rendered at every step; `useViewSky` reads a manual triple only,
    so that `AUTO`'s steps do not cull the sky again. `ExposurePanel` takes the meter's own value
    (`AutoExposure.meteredEv100`), which `ENABLE` resumes at. `HistogramReader.dispose` releases its
    ring's three buffers.
  - **`ENABLE` from `MAN` (ruled, decision-r07-t8a-meter: kept; the guide's `ENABLE` rows
    follow).** R02's `enable` refused anything but `INHIBITED`, and `INHIBIT` refuses `MAN`, so no
    command led from the default `MAN` to `AUTO` (ux-reviewer). As the smallest reversible choice,
    `ENABLE` now hands `MAN` to `AUTO` at the metered value, held back with `NO IMAGE TO METER`
    while nothing is metered; the guide named no command for it.
  - **`MeterControl`'s buttons are never held back** now that it stands only beside a drawn image:
    with `LIT` chosen and no lit body (no star discs, a body under about 5 px, a point) its reading
    is `NO IMAGE TO METER`, and the way out is to choose another meter. `NO IMAGE TO METER` then
    shows beside a drawn image, for the chosen meter finding nothing to weigh and for the first
    readout before a histogram arrives. Ruled (decision-r07-t8a-meter): the case gets statuses of
    its own, `NO LIT SIDE`, `NO DARK SIDE` and `STAR DISC ONLY`, built in T16. The transient before
    the first histogram gets no word. Until T16, `NO IMAGE TO METER` stands for both, beyond the
    guide's row. Focus on a meter button is lost when the panel unmounts on a style change (a
    consider for T16).
  - **After review.** The loop tells an operator's command from its own publication by keeping
    what it last read of the display's control apart from what it last gave it, so a frame between
    a readout and React's commit cannot revert the smoothing; it gives the display a control only
    when the readout would change (its level, or EV100 at one decimal), and `useViewSky` reads a
    camera's limit only from a `MAN` triple, so `AUTO` does not re-cull the sky. `ExposurePanel`
    takes the meter's own EV100 (`AutoExposure.meteredEv100`), not the applied one, so `ENABLE`
    after an operator's `INHIBIT` resumes at the metered value. `HistogramReader.dispose` releases
    its ring's buffers. The meter's choice is held above the stage beside the exposure, so a new
    scene keeps it. **`ENABLE` now also hands `MAN` to `AUTO`** at the metered value (R02's
    `enable`, the smallest reversible choice: the guide named no way out of `MAN`, and the
    ux-reviewer found `AUTO` unreachable; ruled, decision-r07-t8a-meter: kept; the guide's `ENABLE`
    rows follow). **`MeterControl`'s buttons are no longer held back** while nothing is metered (it
    stands only beside a drawn image, and `LIT` with no lit body must be left by choosing another);
    the reason still shows. Ruled (decision-r07-t8a-meter): the case gets statuses of its own,
    `NO LIT SIDE`, `NO DARK SIDE` and `STAR DISC ONLY`, built in T16. The transient before the
    first histogram gets no word. Until T16, `NO IMAGE TO METER` stands for both, beyond the
    guide's row. Focus on a meter button is lost when the panel unmounts on a style change (noted,
    not handled).
  - **Wording (decision-r07-t8a, follow-up (c), under the owner's delegation).**
    `GRAPHICS STYLE REFUSED: photorealistic style not created, relaunch to retry`
    (`PHOTOREAL_NOT_CREATED`), `PHOTOREALISTIC: PREPARING` (`PHOTOREAL_PREPARING`),
    `BODY PHOTOMETRY: NOT YET MODELLED` (the `AppearanceLabel`; Design note 5 and T2.a's text
    follow) and `LIGHTING: STAR DISCS NOT RECEIVED`; `LIGHTING: PENDING` unchanged. The guide's
    draft rows take the ruling's text.
  - **Tests.** The renderer's histogram (dispatched once a frame under the frame's meter, read back
    once, `takeHistogram` clearing it), the meter mounted only beside a drawn photorealistic image
    in `ViewDisplay.test.tsx`, and in `just test-render` a frame's histogram under `LIT` counting
    the lit side of a planet 20 px across (226 weighted counts). `test/fakeViewEngine.ts` answers
    the kernel, its dispatch and its read-back (an empty histogram).
  - **For the owner (ux-reviewer)**: `MeterControl`'s layout under the Style panel at 1920 × 1080
    and 1280 × 720, and the smoothing speeds by eye (T13.b's by-eye checks: a lit planet on black,
    a star entering the frame, the cockpit turning to a planet) with `just client` on `PHASE TEST`.
- **Deviations in T8.a, as built (part 4: the meter follow-up, decision-r07-t8a-meter).**
  - **The guide's `ENABLE` rows** follow the kept choice: the exposure bullet of "Data states"
    (`ENABLE` also hands `MAN` to `AUTO`, at the metered value), the commanding bullet (from `MAN`
    or from either inhibit) and the nomenclature row of `ENABLE`, `INHIBIT` (from `MAN` or
    `INHIBITED`, drafted also by this plan's T8.a), in the ruling's words.
  - **`ENABLE`'s refusal under `AUTO`** reads `NOT AVAILABLE: the exposure is AUTO`, the twin of
    `INHIBIT`'s `NOT AVAILABLE: the exposure is MAN`. R02's reason `not_inhibited` is renamed
    `already_auto`, the ruling's optional rename, since `MAN` is no longer refused.
  - **`MeterControl`'s reason** describes the pressed meter button only (`aria-describedby`), not
    all three: choosing another meter is the remedy, not a refused command.
  - **Left to T16, by the ruling.** The statuses `NO LIT SIDE`, `NO DARK SIDE` and
    `STAR DISC ONLY`, the `acquiring` window and `ENABLE`'s `NOT AVAILABLE: not yet metered` in
    it all need `AutoExposure`'s cause, which T16 builds with its guide edit. Until then `ENABLE`
    is held back with `NO IMAGE TO METER` before the first histogram, and both panels show it.
  - **Tests.** `MeterControl.test.tsx`: with nothing metered, only the pressed button is described.
    `ExposurePanel.test.tsx` (new): under `AUTO`, `ENABLE` is held back and described as
    `NOT AVAILABLE: the exposure is AUTO`; under `MAN`, `INHIBIT` as
    `NOT AVAILABLE: the exposure is MAN`. `exposure.test.ts` takes the renamed reason.
- **Deviations in T8.b, as built (the class-map hook).**
  - **Files.** `bodies/discSurface.ts` (`MAX_DISC_CLASSES` 16, `CLASS_MAP_FORMAT` `rgba8unorm`,
    `CLASSES_PER_LAYER`, `classMapLayers`, `ClassMapTexels`, `ClassMapTexel`, `classMapTexelOf`,
    `packClassMap`, `classWeightsAt`, `surfaceShares`, `discSurfaceLaws`, `classMapTextureSpec`
    and `classMapSurface`, the construction R10.T10.d calls), with `ClassMapDiscSurface` beside
    `UniformDiscSurface` in `appearance/bodyAppearance.ts`, Provides' shape unchanged (`weights`,
    `laws`, `elsewhere`).
  - **The map's layout**, which the plan left open: a 2D array of `rgba8unorm`, N × N texels a face
    of R05's cube sphere. Texel (i, j) of face f is the cell s ∈ [i ÷ N, (i + 1) ÷ N),
    t ∈ [j ÷ N, (j + 1) ÷ N) of `xyzToFaceUv`'s (u, v) under `uvToSt`, so with N = 2^L it is the
    quadtree's cell (f, L, i, j), tested against `vertexDir` at level 2. Class k's weight is
    channel k mod 4 of layer f + 6 ⌊k ÷ 4⌋, and the shader reads N by `textureDimensions`. At
    most 16 laws per body (R10.T1's fifteen `MaterialClass`es; a body carrying all three
    `FrostSpecies` would need 17), 24 layers.
  - **Weights and shares.** An unsurveyed texel holds no weight. Each shaded sample takes each
    class's weight as its share and gives `elsewhere` 1 − Σ w; weights summing past 1 are scaled
    down to 1. `packClassMap` rounds a texel's weights to bytes that keep their sum (largest
    remainder), so a surveyed texel leaves nothing to `elsewhere`, as R10.T2's bytes summing to
    255 do.
  - **Unfiltered.** The shader reads the texel the hit falls in (`textureLoad`), so no surveyed
    pattern shows past the survey's own cells. On a large disc a coarse map's cells show as
    blocks. Filtering would need a one-texel gutter at the face edges, and would carry surveyed
    weight half a texel into unsurveyed ground. That choice is left to R10.T10.d and T10.e at the
    hand-over. _Ruled 2026-10-04 (decision-r07-t8b):_ R10.T10.d replaces the nearest read with a
    survey-masked bilinear one with face gutters.
  - **Where a hit falls.** R05's spheroid point of the unit direction d is M d (`spheroidPoint`),
    so d is the hit stretched along the pole by a ÷ c. It is read along the body-fixed axes of
    `LitBodyInput.rotation` (R02's `Rotation3`, body-fixed to galactic): x and y are packed, and z
    is their cross product. Under a class map the disc takes its pole from the rotation's z axis,
    not from `figure.pole`, so the figure and the map cannot disagree.
  - **Inputs.** `LitBodyInput` gains two optional fields:
    - `surface`; where it is absent, the photometry's uniform law.
    - `rotation`. A class map without one cannot be oriented, so its disc shades with `elsewhere`.
  - **The record.** `DiscRecord` takes `surface` and `tableRows`, in place of `law`,
    `albedoScale` and `tableRow`.
    - `surface` is a `DrawnDiscSurface`: the `uniform` case, or an `OrientedClassMap` (the class
      map with the body's `rotation`), so a record cannot hold a map it cannot orient.
    - `tableRows` has one row per `discSurfaceLaws` entry, the uniform law or `elsewhere` first.
    - Each law's A takes the `oblateAlbedoScale` of its own L.
    - `rasteriseDisc(record, camera, viewport, classMap?)` reads each law's table itself, so it no
      longer takes a `table` argument. It takes the map's texels, which the surface holds only as a
      texture.
    - `DISC_ROWS` goes from 24 to 46 (a record of 736 bytes, 384 before): rows 24–25 hold the
      axes, 26–41 the classes' A and L, 42–45 their table rows, and row 5's w the class count.
    - The disc materials gain `classWeights` at binding 2 (`2d-array`). A uniform disc binds the
      renderer's one-texel `bodies:no class map`, which is never read.
  - **The shader** takes the per-light terms (horizon, eclipse, phase angle) once per light. It
    evaluates `body_brdf`'s inlined expression for each law with a share above 0: three at most on
    the synthetic maps, up to nine under R10.T2's eight-class palette on a partly surveyed texel,
    17 by the layout. Each costs one law evaluation per light per sample, which goes to T17's
    bench. R08's sky term takes the shares' mean A. On a uniform surface the arithmetic is the
    part-1 shader's (share 1, the mean A the law's).
  - **Not wired into the view.** `litBodiesOf` passes no surface or rotation, so nothing is drawn
    differently until R10 lands (Design note 24).
  - **For R10 (passed to the orchestrator for R10's owner).**
    - R10.T10.d also sets `LitBodyInput.rotation`, and passes `surface` and `rotation` through
      `litBodiesOf` (`displays/view/photorealFrame.ts`), a file its text does not list.
    - `packClassMap` and `classMapSurface` are built here, so T10.d supplies each texel's weights
      (`weights_at` at the cell's centre, byte ÷ 255, `null` where unsurveyed) rather than writing
      the construction. The caller owns the texture: it remakes it after a device loss and
      releases it when the map is dropped.
    - A live body's class map shades with `elsewhere` until plan 14 sends the body-fixed rotation
      (`bodyFixedRotation` is `null` today).
    - At most 16 laws per body.
  - **Open: L(α) against a constant L (for the orchestrator and R10's owner).** R10's Design note 8
    gives every class McEwen's phase-dependent L(α). The `class-map` case carries
    `PhotometricLaw`s, whose `lommelSeeligerShare` is a constant, as Provides wrote it. So the disc
    and the terrain agree at every phase only where L(α) equals that constant, which bears on
    R10.T10.e's 1/3-stop check at 30° and 90°. T8.b built Provides' shape, the smallest reversible
    choice. The lean is a per-law L(α) row beside f's in the phase table, read by the disc and the
    terrain alike, added by R10.T10.d. _Ruled 2026-10-04 (decision-r07-t8b):_ The L(α) curve goes in
    the alpha channel of each law's own f row, built by R10.T10.b.
  - **The point keeps the photometry's law (for R10.T10.d and the owner).** At the 3 px switch, a
    patterned body's disc differs from its point by how far its visible hemisphere departs from the
    mean, up to the contrast of its faces.
    - Iapetus is an example. Its dark leading terrain is about a tenth as bright as its trailing
      terrain (Squyres and Sagan 1983, Nature 303, 782; Spencer and Denk 2010, Science 327, 432).
      Its faces seen whole differ about fivefold (mean geometric albedos 0.07 and 0.35; Morrison et
      al. 1975, Icarus 24, 157; to verify: the albedos are not in its abstract, decision-r07-t8b).
    - Against the mean of the two, the disc at the switch would be up to 1.7 times as bright as
      the point when it faces the trailing side, some 0.6 mag. Facing the leading side, it would be
      about a third as bright, some 1.2 mag fainter. These are estimates from those albedos, not
      measured in the renderer, and they assume the point's law has the map's mean albedo.
    - Integrating the map for the point (each frame, or tabulated by direction) is left to
      R10.T10.d or a follow-up.
    - _Ruled 2026-10-04 (decision-r07-t8b):_ The point integrates the class map in a new R10.T10.f,
      to T8.a's 1%. The Iapetus figures are about 1.4× and 0.28× against the area mean (calibrated
      to Iapetus's V range of 10.2–11.9, from a secondary source: to verify).
  - **Tests** (`bodies/discSurface.test.ts`, on a synthetic two-class map with half its cells
    surveyed, i + j even, the pole tilted 55° and turned 30°):
    - At 40° of phase, each pixel of a 64 px, 10% oblate disc within 70% of its polar radius (one
      sample each) equals the share-weighted sum of the uniform discs of its texel's laws, bound
      10⁻¹² (worst 4 × 10⁻¹⁶). The texel is found by an independent `f64` ray–spheroid hit in
      body-fixed axes, and each of the three laws is seen on more than 100 pixels.
    - A half-surveyed map of one class whose law equals the uniform law (a distinct object) draws
      the uniform disc to 10⁻⁵ at 20 and 64 px, 10% oblate, at 70° of phase.
    - On a sphere 64 px across at zero phase, per channel: a 1 : 3 mix in every texel gives the
      mixed flux to 10⁻⁹. The half-surveyed map at 16 texels a face is within 0.10% of the
      area-weighted fluxes of the uniform law and the two classes (bound 0.3%), the areas taken
      over 2 × 10⁵ Fibonacci directions.
    - Layout, quantisation, shares, each refusal, the fall-back to `elsewhere` without a rotation,
      the record's packing and the renderer's binding.
  - **Smoke checks** (`smoke/bodies.ts`, `checkClassMap`):
    - A 40 px, 10% oblate disc at 60° of phase under the half-surveyed two-class map (4 texels a
      face) equals the CPU rasteriser within T8.a's texel tolerance, with its classes.
    - The one-class map draws the uniform disc to 10⁻⁵ at 20 and 40 px.
    - `just test-render` (SwiftShader, `default` and `no-subgroups`, 2026-10-04): every check
      passes (446, none failing), T8.a's figures unchanged. The two-class disc's 1,357 pixels are
      within 0.926 of the tolerance. In the twin, turning the body by 10⁻⁵ rad moves no pixel by more
      than 0.094 of it, so no sample lies near a texel boundary. The margin is `f32` shading at a
      limb sliver, not a texel flip. The one-class map's texels equal the uniform disc's exactly
      (0 at 20 and 40 px).
- **Deviations in T11, as built (planetshine).**
  - **Files and names.** The functions take `LitBodyInput`-shaped bodies and placed lights, not
    `SceneFrameBody`, which carries no radius or photometry. `lighting/planetshine.ts` holds:
    - `PLANETSHINE_SOURCES_HIGH` (2) and `PLANETSHINE_SOURCES_LOW` (1);
    - `SecondarySource`, which gains `body` (the neighbour) and `distanceM`, so that each lit point
      takes its own direction and inverse square;
    - `ReflectingBody`;
    - `LitNeighbour` and `litNeighbours(bodies, hosts, annuli)`, built once a frame: each
      neighbour's starlight, its eclipse (below), its equivalent sphere and a bound;
    - `planetshineSources(body, lit, max)`, where `lit` is the `LitNeighbour`s;
    - `phaseMaximum(law)` and `planetshineIrradiance`, the TypeScript twin.
    - `SecondarySource` also gains `radiusM`, so the disc record takes R directly.
  - **Elsewhere.**
    - `bodies/oblate.ts` gains `bodyReflection` (p Φ(α) for a sphere, the figure's integral for a
      spheroid) and `figurePole`. The point and planetshine reflect through this one function, a
      pure refactor of `pointFlux`.
    - `lighting/hostLights.ts` gains `lightsAt`, `LightAtPoint` and `MAX_BODY_LIGHTS`, moved from
      `draw.ts`' private `lightsOf`. `MAX_DISC_LIGHTS` is now its alias.
    - `BodyFrameOptions` gains `planetshine`, the count. The renderer sets it from the setting.
      `MAX_DISC_SECONDARIES` is `PLANETSHINE_SOURCES_HIGH`, so the disc cannot drop a source the
      point keeps.
    - `pointFlux(body, hosts, occluders, k, secondaries = [])` takes the body's sources as a fifth,
      defaulted argument.
    - `test/litFixtures.ts` gains `photometryFor(p, q, template)` (provenance `modelled`),
      `planetPhotometry(name)` (a planet's p in B, V and R, its q_V in every channel) and
      `MOON_GEOMETRIC_ALBEDO` (0.12).
  - **The term in the shaders.**
    - `litBody.wgsl` gains `planetshine_irradiance` (from the point to the source, the source's
      radius, its distance from the centre, the normal, the horizon): `sphere_irradiance` at the
      point's own H = d ÷ R, times (Δ ÷ d)².
    - `litBody.wgsl` also gains `lit_disc_term(l, h, mu0, mu, source_radius)`, the lunar-Lambert
      disc term under an extended source with the Lommel–Seeliger floor. The stars' inline
      expression now calls it, with unchanged arithmetic.
    - `bodyDisc.wgsl` gains `surface_reflectance`, shared by stars and planetshine, and a loop
      over the sources.
    - `DISC_ROWS` goes from 46 to 51. Row 46 holds the count, and rows 47–50 two sources (the
      direction and distance ÷ a; the illuminance and radius ÷ a). `DiscRecord` gains
      `secondaries` (`DiscSecondary`), and `MAX_DISC_SECONDARIES` is 2.
  - **Model, as built.**
    - Planetshine never sets a sample's `lit`, so a night side lit only by a neighbour keeps
      `unlitBody` for `DARK`.
    - It takes neither the eclipse term on its way nor R11's ring shadow. It does pass through
      R08's `atmosphere_sun_transmittance` along its own direction: a stub of 1 today, for R08's
      owner to keep or refuse.
    - The point adds each source as a point source along its centre's direction. At the 3 px
      switch the disc meets the point to 0.35% (Earth from the Moon, Lambert), 0.07% (the same,
      lunar law), 0.10% (Jupiter from Io, Lambert) and 0.61% (the same, lunar law), at 90° and
      150° of solar phase. The science check gives two terms:
      - The point source against the extended one: −ρ²/4 at zero phase (−0.66% at Io), but the
        true flux exceeds the point's by 1.9%, 10% and 103% at 120°, 150° and 170° at Io for a
        Lambert receiver (0.9%, 4.6% and 44% for a lunar law).
      - The receiver's own size, its lit hemisphere nearer the source: +3ε ÷ 4 at zero phase for
        Lambert, ε = r ÷ Δ (0.34% for the Moon, 0.32% for Io).
  - **The neighbour's starlight is eclipsed (a deviation; confirmed as an interim, replaced by
    R07.T10.b, decision-r07-earth-albedo).** It takes the eclipse term from its centre, counting
    only the bodies larger than the neighbour. This fixes two cases:
    - In a total lunar eclipse, Design note 7 as first written left 0.31 lx of moonlight on
      Earth's night side. The truth is about 10–50 µlx, light refracted by Earth's air, which no
      plan draws yet (Hernitschek, Schmidt and Vollmer 2008, Applied Optics 47, H62, Table 2: 9.6
      and 11.15 mag below full; 12–54 µlx from the abstract's −3.32 and −1.7 mag at
      V = 0 = 2.54 µlx).
    - It also fixes every full phase of a giant's moons seen from the giant.
    - Its two errors, removed by R07.T10.b's disc-averaged eclipse:
      - A smaller body's shadow is left out. From the neighbour's centre it would hide the whole
        star where it hides a spot: about 0.1% of Jupiter-shine for Io's shadow, and up to 11% of
        earthshine in a central solar eclipse (10.7% by brute force).
      - Partial phases are taken at the neighbour's centre, so a moon wider than the planet's
        penumbra fades too fast: in 44 s rather than 254 s for Io entering Jupiter's shadow. The
        "larger than" test also never shadows a pair of equal moons in a mutual eclipse.
    - The test of a moon in the umbra leaves no source.
  - **Selection.**
    - The candidates are ranked by the photopic light of their equivalent sphere, √(a c), with
      ties broken by identifier, so the order of the inputs does not matter.
    - A candidate whose bound (`phaseMaximum`, the law's largest p Φ over the 0.5° table, plus 1%)
      cannot reach the `max`-th best so far is skipped. A test against brute force over 40 random
      neighbours confirms the choice.
    - A neighbour that reflects nothing is not a source.
    - Only drawn bodies look for sources. The cost is about 2 ms a frame for 100 drawn bodies
      under shared load: provisional, and for T17's bench.
  - **Figures tested** (Sun-like disc, `sunLikeHostDisc`):
    - Earthshine at full Earth is 7.668 lx at the fixture's p to three places, (r, g, b) 0.210,
      0.215 and 0.263, and q 1.312; the ruling's 7.66 (7.664) is at the unrounded split, under the
      same warm white of `sunLikeHostDisc` (T4.d's science check). It is the closed form
      E★ p (R ÷ Δ)² to 10⁻⁹, inside the task's 7.7 lx ± 15%. T11 built it with a local copy of
      decision-r07-earth-albedo's photometry; R07.T4.d moved the fixture's row to the same values
      and pointed the test back at `planetPhotometry("Earth")`.
    - The full Moon is 0.3140 lx against 0.3168 lx from V = −12.74 (−0.9%), with p_V 0.12 (NASA's
      fact sheet, without the opposition surge; Krisciunas and Schaefer 1991, p. 1035). Earth is
      left out as an occluder there, because at zero phase the Moon is in its shadow.
    - Jupiter-shine on Io at inferior conjunction is 66.7 lx, in Io's own shadow transit, with
      Jupiter 6.5% oblate. That is within 10⁻³ of the closed form over √(a c) and 6.16 stops below
      the sunlight. The plan's 70 lx ± 10% is kept.
    - A neighbour at new phase gives under 10⁻¹² of full.
    - The `view/bodies` tests: a lunar disc's night-side pixel equals E (Δ ÷ d)² × exposure ÷ π ×
      `brdf` to 10⁻⁶, and the night side keeps `unlitBody`.
    - The renderer takes two sources on `high` and one on `low`.
  - **Earth's albedo: resolved (decision-r07-earth-albedo).** Mallama et al. 2017's p_V of 0.434,
    which the fixture and Design note 5's `earth` row used until R07.T4.d, is superseded by
    measurement. Robinson
    2026 (PSJ 7, 12, arXiv:2507.22258) gives a visual p of 0.242 (0.277, 0.226 and 0.221 in
    0.1 µm bands) and q of 1.22. Mallama's 0.434 came from extrapolating EPOXI data through
    Tinetti et al. 2006's model, whose Sun–observer azimuth is turned by 180°. The ruling takes
    Robinson's eq. 14 fit, f = 0.23 in his band ratios, as the `earth` template. Earthshine at
    full Earth is then 7.66 lx photopic, 8.06 lx at his physical model's p. T11 changed only its
    own earthshine test. R07.T4.d moved the client's `earth` and the fixture's row; P14.T47.e
    moves the generator's, in the 20 → 21 bump. The ruling's other plan text (Design note
    5's sources and ratio, T1, T4.d, T10.b, the Risks line on Earth's albedo, and plan 14) is
    applied by the docs pass of 2026-10-04.
  - **Existing tests.** T8.a's umbra test now runs with planetshine off, since it tests the
    starlight's eclipse term. Its occluder exactly at new phase gives about 4.5 × 10⁻²³ lx, from the
    rounding of sin π, and the occluder beyond the star some 10⁻¹⁰ of the sunlight.
  - **Smoke checks.** The probe's `LightingCase` gains kind 4: `planetshine_irradiance` against
    its twin at four points (wholly up, cut by the horizon, at Earth-from-the-Moon scale in the
    soft band, and behind a 3° horizon), to 2 × 10⁻⁴. `smoke/bodies.ts`' `checkPlanetshine` draws a
    Moon 20 px across at 150°, pre-exposed at 0.05, its night side lit by a full Earth, against
    the CPU rasteriser. `just test-render` (SwiftShader, `default` and `no-subgroups`, 2026-10-04):
    every check passes (456, none failing), T8.a's and T8.b's figures unchanged. The four
    planetshine factors are within 9.6 × 10⁻⁵ of their twins. The moon's 368 pixels are within
    0.258 of T8.a's texel tolerance, and all 280 unlit ones hold earthshine with the rasteriser's
    classes.
  - **Neighbours at drawn centres.** Neighbours are placed at the scene's drawn centres, as lights
    and occluders are (T8.a's known limit, "Light positions"). T10.a's retarded geometry must also
    place each neighbour at t_B − |r_N − r_B| ÷ c, and the neighbour's stars at its own retarded
    time.
  - **For R10 and T9.** A lit point's planetshine is `planetshine_irradiance` with the horizon
    argument (0 on the smooth figure) and `lit_disc_term`, over the `SecondarySource`s of its body.
  - **Possible follow-up (not built).** A table in (α, sin ρ, L) would correct the far-field
    illuminance's errors: first order in R ÷ Δ near full phase, and past quarter phase the hidden
    limb crescent (at Io, 34% below at 120°, 71% below at 150°, nothing from 170.6°).
  - **Found, for T10 (not fixed here) → R07.T10.b (decision-r07-earth-albedo).** A point body
    takes its eclipse from its centre, so a point Jupiter goes fully black during Io's shadow
    transit: `pointFlux` gives 0 against 5.16 × 10⁻⁵ lx clear, measured 2026-10-04. R07.T10.b's
    disc-averaged eclipse, shared by `pointFlux` and planetshine's neighbour, fixes it.
- **Deviations in T4.d, as built (Earth after Robinson 2026, decision-r07-earth-albedo).**
  - **The template.** `templates.ts`' `earth` is eq. 14 in its magnitude form, with
    `EARTH_HG_ASYMMETRY` = −0.33, to 144°, L = 0, not provisional. Its source string cites eq. 14
    (g, f = 0.23, the data's 5°–144°) and Robinson et al. 2011 for dropping eq. 5.
  - **The fixture's row.** p 0.263, 0.215 and 0.21; B − V 0.43 and V − R 0.52; V(1, 0) and
    `templateV10Mag` −3.23; radius 6,371 km; q_V 1.312.
    - The colours come from Model 07's unrounded band ratios and `SUN_JOHNSON_MAG`: 0.429 and
      0.516. The rounded p give a V − R of 0.514.
    - T4.a's checks of p_V against V(1, 0) and the radius (0.5%) and of the colours against the
      band ratios (3%) pass for the new row unchanged.
  - **Figures, computed by `phaseIntegral` and reproduced by a Python port.**
    - q_V at s = 1, clamped and held, is 1.31157. The clamp acts from 139.006°. Eq. 14 alone over
      0°–180° gives 1.3501.
    - The clamp-departure test's reference holds f unclamped past 144° (q 1.3202), not eq. 14 to
      180°, so Earth's row there is −0.656%.
    - The law solved to the fixture's q_V 1.312 has s = 0.99961. Its V departs from eq. 14 by at
      most 0.073% over 0°–135°, at 135°.
    - 0.23 q = 0.302, 2.6% above Robinson's 0.294; p_V q_V = 0.282.
  - **Tests.**
    - `templates.test.ts`: eq. 14's dimming at 30°, 90°, 120° and 144° by hand to 10⁻⁴ mag, and
      the range, L, the flag and the source.
    - `phase.test.ts`:
      - Φ_t(0) = 1 and q_V 1.3116 to 0.5%;
      - the clamp from 139.0° (f under 4 at 138.9°, at 4 by 139.1°), and eq. 14 alone giving 1.350;
      - 0.23 q within 3% of 0.294;
      - the clamp-departure row at −0.656%;
      - Earth's V against eq. 14 to 0.5% every 5° from 0° to 135°. Eq. 14 is written there in its
        Henyey–Greenstein form, independently of the template's magnitude form. This replaces
        eq. 5's 30%.
    - `planetshine.test.ts`' earthshine test reads `planetPhotometry("Earth")` again: 7.668 lx,
      unchanged.
  - **The golden.** No client code or test reads `photometry/templates.golden`. Its `earth` block
    stays on eq. 5 (q 1.3105694) until P14.T47.e's 20 → 21 bump, after which the two must agree.
    R07.T2.b waits for both.
  - **Other users of the `earth` key.** `discSurface.test.ts` and `smoke/bodies.ts` keep their own
    "bright terrestrial" class law (p 0.5, 0.45 and 0.4; q 1.3), now on eq. 14. T11's 3 px switch
    test with Earth from the Moon (`draw.test.ts`) and the ranking's bound now run on the new row.
    All pass unchanged.
  - **Reviewed.** The science check (2026-10-04) re-derived every figure above, eq. 14's form from
    Robinson's eqs. 5 and 14, and the citations, read from arXiv:2507.22258v2 (eq. 14's number in
    the typeset PSJ article not seen). Its wording notes are applied: Robinson's bands are 0.1 µm
    wide and solar-weighted, and stand for Johnson's R only in part (Johnson's R reaches past
    0.8 µm, recalled from Bessell 2005, moderate confidence); the flipped azimuth is in Robinson
    et al. 2011's Fig. 2 caption, in §3.4. The typescript review's two points (the departure
    comment, one reason per test) are fixed.

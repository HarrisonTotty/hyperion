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
`level`, `hillRadiusM: number | null`; _R07.T10.a adds `emittedM` and `emittedVelocityMPerS`_) and
`contact` (`apparentM`, `emitted` and `level` only);
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
// lighting/retarded.ts, as built (T10.a; decision-r07-t8a, follow-up (a))
export const RETARDATION_STEPS = 2;
export function retardedFrom(lit: RetardedCentre, source: RetardedCentre): Vec3;
export interface LightingFrame {
  readonly lights: ReadonlyArray<PlacedLight>;
  readonly occluders: ReadonlyArray<LightingBody>; // every other lit body, retarded
}
export function isLitBody(body: ViewBody): boolean;
export function lightingFrameOf(
  scene: ViewScene,
  lit: BodyIdHex,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): LightingFrame | null;
export interface LitBodyLighting {
  readonly body: ViewBody;
  readonly centreM: Vec3; // drawn, from the camera
  readonly frame: LightingFrame;
}
export function lightingFramesOf(
  scene: ViewScene,
  pose: CameraPose,
  discs: ReadonlyArray<HostDiscDto>,
): ReadonlyArray<LitBodyLighting>; // every lit body, in the scene's order
// lighting/discEclipse.ts, as built (T10.b; decision-r07-earth-albedo, Q3)
export interface EclipsedBody {
  readonly centreM: Vec3;
  readonly figure: BodyFigure; // taken as its equivalent sphere √(a c)
}
export function discEclipseVisible(
  star: PlacedLight, // its B, V and R limb laws: the result is per display channel
  body: EclipsedBody,
  occluders: ReadonlyArray<Occluder>,
  towards: Vec3, // unit, from the body to the far point
  share: number, // L
  k: number,
): Rgb;
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
  /** R02's triple: the view camera's program at every level, or `MAN`'s own (T13.c). R06's
   *  `cameraLimitV` reads it (T13.e). */
  readonly triple: ExposureTriple; // R02's `{ aperture, shutterS, iso, ndEv? }`
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

### Strokes (`lib/`, R07.T16.d)

```ts
// lib/strokes.ts (decision-thin-line-contrast, item 2): device widths at a device-pixel ratio.
export const MIN_STROKE_DEVICE_PX = 2;
export function lineScale(devicePixelRatio: number): number; // max(2, ratio), px per CSS px
export function markStrokeDevicePx(devicePixelRatio: number): number; // max(2, 1.5 × ratio)
export function markShiftDevicePx(devicePixelRatio: number): number; // δ, an outline's move out
// T16.f adds strokeProperties, watchStrokeProperties and useStrokeMetrics.
```

R02's `view/wireframe/drawList.ts` gains `ViewStrokes { strokeScale, markStrokePx, markShiftPx }`,
`viewStrokesAt(devicePixelRatio)`, `occluderSlopePxAt(strokeScale)`, `emptyDrawList(strokes)` and
`HULL_OCCLUDER_DEPTH_FRACTION`; `displays/view/ViewMarkLabels.tsx` gains
`markLabelShiftPx(devicePixelRatio)`.

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
  `onMetering(control, ev100 | null)`, which this plan's meter feeds (_R07.T16.b: `inhibited` also
  takes `nothing_weighed` with its meter, `setAuto` refuses with `not_metered`, and `onMetering`
  takes `number | SystemInhibitCause`; see "Deviations in T16.b, as built"_); the panel is
  `ExposurePanel`;
  `buildWireframeDrawList(scene, camera: DrawCamera, viewport, tokens, options: DrawOptions)` with
  `CASING_PX` = 1, lines `premultiplied`, screen symbology at depth 1; `agx`, `agxSigmoid` and
  `agxSprite` in `toneCurve.wgsl`; there is no client `BodyFixedRotation` type: a body's rotation is
  `CameraOrigins.bodyFixedRotation(body): Rotation3 | null` (`view/coords/position.ts`), `null`
  today since plan 14 sends no rotation; the sphere occluder's `SLOPE_SCALE` is 3 (a WGSL constant
  in `occluderSphere.wgsl`) while the hull faces' `occluder.wgsl` bias keeps `slopeScale: 2`
  (raised to 3 by T16.a). _R07.T16.d: both occluders take `occluderSlopePx` in the fragment, the
  hull faces with no hardware bias, and `DrawOptions` extends `ViewStrokes`; see "Deviations in
  T16.d, as built"._
- **R03:** `useScene` and `sceneAt` (`SceneFrame`, with each body's `geometricM`, `apparentM`,
  `emitted` and `hillRadiusM`), bodies as plan 14's `BodySummaryDto` inside R03's
  `SceneBodyDto` and `SceneSystemDto`, each with its granted `level`, `SceneClockDto`,
  `KinematicsDto`, `CameraReporter`, the two-clients-agree test, and the optional `main_screen`
  field its Design note 4 reserves. _As built (re-checked at `ce7aeb3`):_ `sceneAt(model, observer,
time, previous)` with `previous` required; a body's entry is `SceneBodyFrame`, and a `contact`
  entry has no `geometricM` or `hillRadiusM`; levels come per body through `SceneSystemDto.grants`
  (`BodyGrantDto { body, level, seen }`), not on each body; `main_screen` is room left by R03's
  Design note 4, not a field (Phase C adds it); the two-clients test is
  `crates/hyperion-server/tests/scene_agree.rs`. _R07.T10.a adds `emittedM` and
  `emittedVelocityMPerS` to `placed` entries and to `SceneStarFrame`, the retarded centres lighting
  takes._
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
  R06 Design note 18, the N and t of whose Design note this plan's `VIEW_CAMERA` holds with the
  camera's ranges, T13.c) R06.T13.a, `HostDiscLayer`, `glareSources`, `DEFAULT_EYE_OBSERVER` and the
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
   10⁻⁸ rad for a Jupiter's pull on a Sun, which a test states. _As built (R07.T10.a): each star is
   taken at the lit body's retarded time less the star-to-body light time (`retardedFrom`). A
   lone star does not move in the placements, so the reflex error stands; a multiple system's
   stars are retarded along their orbits._ Stars of a multiple system each
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
    No dark adaptation of the eye is modelled. Every level reads one camera, the view camera
    (decision-r07-exposure-camera; T13.c): R06's sensor behind a fixed f/1.4 lens, a shutter of
    1/8,000 to 1/30 s, ISO 100 to 409,600 and a variable neutral-density filter (ND), its triple a
    function of EV100 alone, EV100 = log₂(N² ÷ t) − log₂(S ÷ 100) + ND. From dark to bright it holds
    1/30 s, the live frame, and sets S = 5,880 × 2^−EV100 from 409,600 (EV100 −6.12) to 100 (5.88),
    R06 Design note 18's 12 stops of gain; then holds ISO 100 and sets t = 1.96 × 2^−EV100 to
    1/8,000 s (13.94); then holds 1/8,000 s and sets ND = EV100 − 13.94 EV, 28.1 EV at `MAN`'s
    top, 42. Darker than −6.12 the sensitivity runs on as a digital push, which brightens the
    picture without adding signal and reads `409,600 ↑`. S never falls below base, where R06's
    gain model does not hold. `MAN`'s entry takes the same triple; the reading carries it at every
    level for R06's `cameraLimitV`. `INHIBITED` freezes the last value.
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
    child's context on `pagehide` and expects that error. _Amended by T21 (2026-10-06): the
    `pagehide` can come after the opener's next frame, so a child the opener closes itself has its
    context dropped at that `close()`, and the `pagehide` drops it for every other close (R01.T13's
    rule as amended, in R01's T13 as-built notes)._ Pacing on a second monitor is untested, for
    want of one (R07.T21).
16. **Symbology over the image is cased; text sits on plates.** Every mark over a photorealistic
    image is stroked twice, a `--surface-0` casing at the guide's widths beneath the coloured
    stroke, never a blur or glow (guide item 3); a DOM readout over the canvas sits on its own
    `--surface-0` plate. The wireframe style's marks follow the guide's ordinary rules. The image is
    never dimmed under symbology. The one exception is a craft's silhouette, under Design note 17
    (decision-r07-t16a, item 3). Status colours reserved for symbology cannot be reserved from
    photons, so marks are told from the image by shape and outline (brainstorm, The contrast
    problem).
17. **Nothing in the scene goes undrawn in the photorealistic style.** Until R11 draws rings they
    are R02's ring ellipses, cased over the image. Until hull art exists craft are R02's hull
    outlines, cased over the image on a `--surface-0` silhouette of their opaque faces, so that
    nothing behind those faces shows through, and labelled `CRAFT PHOTOMETRY: NOT YET MODELLED`;
    a window is glass and hides nothing (decision-r07-t16a, item 3). A body with no photometric
    section is drawn with the provisional photometry and labelled (Design note 5), never omitted.
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
(T13 reads no veil but tests against T14.a's glare sources) and T13.a before T16. T13.c, T13.d
and T13.e follow T13.b in that order, before T16 (decision-r07-exposure-camera). T16 follows T8.a
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
- **R07.T8.c Small discs sampled by size** (decision-r07-small-disc-cost).
  - **What it does.** `discSamples(diameterPx)` in `bodies/discShading.ts` sets a disc record's
    cells: 8 × 8 in every pixel below `FINE_DISC_PX` (4 px); 4 × 4 in every pixel from 4 px to
    under `SMALL_DISC_PX` (32 px); one inside and 4 × 4 on the limb above.
    - The constants are `FINE_DISC_PX = 4`, `FINE_DISC_SAMPLES = 8` and `SMALL_DISC_SAMPLES = 4`;
      `LIMB_SAMPLES` is unchanged.
    - `discRecordOf` (`bodies/draw.ts`) takes its counts from `discSamples`.
    - There is no hysteresis, and no change to `bodyDisc.wgsl`. The twin follows from the record.
    - It replaces T8.a's "8 × 8 below 32 px" (decision-r07-t8a, item 3, refinement 4).
  - **Files.** `bodies/discShading.ts`, `bodies/draw.ts`, `bodies/draw.test.ts`, the smoke
    harness's disc checks, and this plan. _As built, also `bodies/discShading.test.ts`._
  - **Tests** (decision-r07-small-disc-cost §2):
    - `discSamples` at 3.99, 4, 31.99 and 32 px, and the records' row 3 for a 3.5 px and a 13 px
      disc.
    - G1 at 2.75, 3 and 3.3 px.
    - G2 and G2′.
    - G4's steps at 4 and 32 px.
    - G5, with G5′ anchoring its reference.
    - G6 through T10.b's `ingress` at 4, 6, 12 and 24 px.
    - T8.a's, T9's, T10.b's and T10.c's tests unchanged.
  - **Acceptance.**
    - `pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies
src/renderer/src/view/scenes src/renderer/src/view/lighting`.
    - `just test-render`, both variants: G10 re-measured; the captures holding a disc of 4–31 px
      listed as changed and every other capture byte-identical to the base.
    - `just ci`.
    - By hand, hidden, on the RTX 3080, with the lane's cost harness
      (`.git/rm23-scratch/r07-shading/cost-t19/`): `PHASE TEST` all photorealistic, each
      instrument's `discs` pass at most 0.6 M cycles at the logged clock. The 4 × 4 build measured
      0.54 M. A 3.5 px disc's view is recorded beside it, about 2.0 M, unchanged until T8.d.
  - _As built (2026-10-06, the shading lane): see Risks, "Deviations in T8.c, as built"._
- **R07.T8.d The cells in parallel** (decision-r07-small-disc-cost, §1.2).
  - **What it does.** A compute pass `disc cells` shades every cell of every disc under 32 px,
    one invocation a cell. Each pixel's cells are summed by one invocation in `pixel_sum`'s order.
    - The disc's interior and limb draws, and a promoted small body's mesh interior, read the
      pixel's sum (record row 51) in place of running `pixel_sum`.
    - Their tests, classes and coverage are unchanged.
    - A disc of 32 px or more keeps the in-fragment path, as does a record whose row 51 says so.
    - One dispatch per photorealistic view per frame, between `sky` and `discs`, and none without
      a small disc.
    - The twin is unchanged: no arithmetic changes.
  - **Files.**
    - New: `shaders/bodyDiscCells.wgsl`, registered in `WGSL_CATALOGUE` as `BODY DISC CELLS`.
    - Shaders: `shaders/bodyDisc.wgsl`, `shaders/bodyDiscDraw.wgsl` and `shaders/smoothMesh.wgsl`
      (the `cell_sums` binding, at one free `@group(2)` number in both includers).
    - TypeScript: `bodies/discShading.ts` (row 51, `DISC_ROWS` 52, the jobs), `bodies/draw.ts`
      (`LitBodyRenderer`'s buffers and dispatch), `photoreal/renderer.ts`, `photoreal/passes.ts`
      (`discCells`), `test/fakeViewEngine.ts` and `test/countingRenderEngine.ts` as needed.
    - The smoke harness's disc checks.
  - **Tests.**
    - Against the fake or counting engine:
      - A frame with a 3.5 px and a 13 px disc dispatches `disc cells` once, before `discs`, with
        one job per pixel of their rectangles, and writes their row 51.
      - A frame whose discs are all 32 px or more dispatches nothing and writes −1 there.
      - Three photorealistic views dispatch once each.
      - The pass list puts `disc cells` between `sky` and `discs`, no label repeated.
      - A device loss remakes the kernel and buffers.
      - `TIMING_FRAMES_IN_FLIGHT` still covers three frames' resolves of a photorealistic primary
        with two photorealistic instruments.
    - `just test-render`, both variants:
      - G11 for a 3.5 px disc (8 × 8), a 13 px and a 20 px disc (4 × 4), and a small body
        promoted to the mesh regime, each drawn through the pass and in-fragment in the same run.
      - G10 against the twin.
      - The kernel in the catalogue check.
      - No uncaptured GPU error.
  - **Acceptance.**
    - `pnpm test`, `just test-render` and `just ci`.
    - By hand, hidden, on the RTX 3080, with the cost harness:
      - In `PHASE TEST` all photorealistic, an instrument's `disc cells` and `discs` together at
        most 0.2 M cycles, from 0.54 M after T8.c.
      - A view whose only disc is 3.5 px at most 0.2 M cycles, from about 2.0 M.
      - The primary recorded as limited by its discs of 32 px or more (about 0.47–0.54 M).
    - A bound missed is recorded and goes to the orchestrator for a ruling. It does not fail
      silently.

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
  - Earth's light towards the Moon in a central solar eclipse is the oracle's 0.892 at 1 au
    (0.893 in parallel light) of clear (uniform Sun, Lambert);
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
  `just ci`, `just test-render`. _As built (R07.T10.c): `ECLIPSE TEST`, one kept scene whose clock
  runs a hundred times the script's, with a free camera in the planet's frame in place of a ship's
  seat; its bodies move and are lit at their retarded places. See "Deviations in T10.c, as
  built"._

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
- **R07.T13.c One camera** (decision-r07-exposure-camera). R02's `exposure.ts` takes the program
  from `post/autoExposure.ts` and widens it:
  `ExposureProgram { aperture, frameShutterS, minShutterS, baseIso, maxIso, maxNdEv }`,
  `VIEW_CAMERA` (f/1.4; 1/30 s and 1/8,000 s; ISO 100 and 409,600; 28.1 EV), and
  `programTriple(program, ev100)`: 1/30 s with S = 100 × (N² ÷ t) × 2^−EV100 while S ≥ 100, beyond
  `maxIso` included; then ISO 100 with t = N² × 2^−EV100 down to 1/8,000 s; then 1/8,000 s with
  `ndEv` = EV100 − log₂(N² ÷ t_min). `ExposureTriple` gains `ndEv?` (EV; absent while clear), which
  `ev100FromTriple` adds, `setManual` refuses when negative or not finite, and R06's
  `cameraLimitParts` applies as a transmission 2^−ndEv; R06's `ViewCameraSensor` gains `maxIso`
  (409,600), and the read-noise term takes S within [`baseIso`, `maxIso`], since below base the
  analogue gain stays at base and above the top a push adds none. `DEFAULT_MAN_TRIPLE` becomes
  `programTriple(VIEW_CAMERA, -1)`, f/1.4, 1/30 s, ISO 11,760 (EV100 −1 unchanged).
  `VIEW_AUTO_PROGRAM` retires; `AutoExposure` takes `VIEW_CAMERA`. R02's `ExposurePanel` shows
  `APERTURE`, `SHUTTER`, `ND` and `ISO` at every level from the shown control (`MAN`'s triple, else
  the program's): `f/1.4`; s to three figures; `CLEAR` or `6.1 EV`; ISO whole; a member beyond the
  camera's range pegged with the guide's off-scale `↑` (`ISO 409,600 ↑` below EV100 −6.12,
  `ND 28.1 EV ↑` above 42). The guide's rows follow, drafted for the owner. Tests: the program's
  EV100 round-trips to 10⁻¹² from −14 to 42 and is continuous at its joins, S ≥ 100 and t in range
  throughout; 5,880 against K N² ÷ (t L̄); the limit at 60° and μ 24 is 10.06 up to EV100 1, 9.40 at
  5.88, 2.56 at 15; an ND equals the same shortening of the shutter in the limit; the panel's rows
  under `AUTO`, at the push and with the ND.
  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/photometry
src/renderer/src/view/post src/renderer/src/view/sky src/renderer/src/displays/view`, the
  console-ux skill's scripts, `just ci`.
- **R07.T13.d The way back to `MAN`** (decision-r07-man-exposure, as amended by
  decision-r07-exposure-camera): decision-r07-man-exposure's subtask text, with `setManualEv100`
  setting `MAN` at `programTriple(VIEW_CAMERA, ev100)` in place of "the `MAN` camera's aperture and
  ISO (the triple in force, else `DEFAULT_MAN_TRIPLE`'s), the shutter solved"; its tests:
  `setManualEv100` round-trips `DEFAULT_MAN_TRIPLE` at −1 and gives the program's triple to 10⁻¹²
  across the span, from `AUTO` at 9.6 f/1.4, 1.96 × 2^−9.6 s, ISO 100. The `MAN` field sits after
  T13.c's four rows. Its follow-up (decision-r07-t13d): every field that enters on `Enter` or when
  left (the `MAN` field, `CURSOR`'s `X`, `Y`, `Z`, the chart's `DRIVE RANGE` and `CHART TIME`)
  takes `Escape`, which drops what was typed and any refusal and shows its value again, the `MAN`
  field's fill, selected, while it holds focus, and does the same for empty text, which is never
  refused; the `MAN` field's fill stays the reading's digits outside the span, where its entry is
  refused, and leaving the field drops a refused fill; `INHIBIT` states
  `Then AUTO resumes only on ENABLE` beside the button, as its description, under `AUTO` and a
  system inhibit, not under the operator's. Tests: `Escape` and empty text clear a refusal with no
  command in each field kind; an emptied `MAN` field under `MAN` keeps its value; a fill at −15.3
  is refused and dropped on leaving, and one at −14.04 enters −14.0; `INHIBIT`'s description at
  each level. Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/displays/view
src/renderer/src/displays/galaxy`, the console-ux skill's scripts.
- **R07.T13.e The sky's limit follows the camera** (decision-r07-exposure-camera). R06's
  `viewSky.ts`: the request's `camera_limit_v` and the cull take the view camera's deepest triple
  (f/1.4, 1/30 s, ISO 409,600: V 10.06 at 60°, 11.72 at 30°, 13.58 at 13° under μ 24) at every
  level, so that no `AUTO` step or `MAN` entry asks, culls or bakes the sky again; the label's
  `STARS V … mag CAM` takes the shown exposure's triple at every level (the guide's `CAM`: "from
  its exposure"), at the readout's 4 Hz and 0.1 mag. Where that limit is 0.05 mag or more shallower
  than the cull's (from EV100 3.5) a star at it reaches the tone curve 3.4–5.0 EV below AgX's floor
  at 1080p and 60° (1.4–3.0 EV at 4K), so the deeper cull changes nothing visible. `useViewSky`'s
  cull no longer depends on the exposure. Tests: the request's limit is the same under `MAN` and
  `AUTO` at any EV100; the drawn sky keeps its identity across `AUTO` steps; the label at −1 and
  15; a star at the label's limit below AgX's floor wherever that limit is shallower than the
  cull's (1080p and 4K, 60°). Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky src/renderer/src/displays/view`, `just ci`.

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

After T19.d, which is the last to edit `ExposurePanel.tsx` and R02's `exposure.ts` before it
(decision-r07-owner-ux-signoff). `photoreal/overlay.ts` (Design notes 16–17): R02's draw list with
casing on in the photorealistic style; rings and hulls as cased marks until R11; DOM readouts on
`--surface-0` plates; the label
block gains the style and `BODY PHOTOMETRY: NOT YET MODELLED` (its `METER` line is built by T19.b,
decision-r07-t19-layout, and its `METER` and `AVG` rows are drafted by T19.b's follow-up,
decision-r07-t19b-exposure-fit); hull edges cased over the
image, with the hull faces' occluder bias (`occluder.wgsl`, `slopeScale` 2 as built) raised to 3 so
that the casing is covered (the UX decision, item 12; the sphere occluder's `SLOPE_SCALE` is 3
already). Draft, for the owner, the nomenclature entries this plan adds beyond R02's nine items
(`PHOTOREALISTIC` is already drafted by R02, beside `WIREFRAME`; the meter's statuses
`NO LIT SIDE`, `NO DARK SIDE` and `STAR DISC ONLY` with their remedy clauses (decision-r07-t8a-meter)
in their own Status row, to which the `METER` row then points,
the albedo phrase; the several views' refusal,
`NOT AVAILABLE: QUALITY LOW allows one photorealistic view`, is the guide's `NOT AVAILABLE` form
and adds no entry, decision-r07-t18 item 6), as one edit of `docs/frontend/ux-guidelines.md`
that ends in the owner's sign-off. T16 also builds them:
`AutoExposure` keeps why it has no metered value (`no-image`, a histogram timeout;
`nothing-weighed`, histograms that weigh no pixel under the meter in force; `acquiring`, under
`METER_TIMEOUT_S` since the image was first drawn or the meter changed with no value held). R02's
`InhibitReason` gains `"nothing_weighed"` with its meter. `ExposurePanel` and `MeterControl` take
the cause beside the metered value; while it is `acquiring`, neither shows a meter status or
`NO IMAGE TO METER`, and `ENABLE` is held back with `NOT AVAILABLE: not yet metered`; the `MAN`
field (T13.d) is never held back, `acquiring` included, and `INHIBIT` states
`Then AUTO resumes only on ENABLE` under each new system inhibit as under `NO IMAGE TO METER`
(decision-r07-t13d), and stays held back under the operator's own (decision-r07-owner-ux-signoff).
Tests: under `LIT` with no lit body, a drawn image reads `NO LIT SIDE`, never `NO IMAGE TO METER`,
after 0.5 s and not before; a meter change clears it at once; `AUTO` resumes when a lit body is
metered. Its other tests: every overlay mark over the image has a casing stroke; plates are present
for every readout; the console-ux skill's lint and contrast scripts pass. Symbology over the
tone-mapped image is a following canvas pass with `FrameSubmission.colourLoad` `"load"` through the
sRGB view, in the same task as T15's pass (built by T15 under decision 2026-10-02, item 6).
Acceptance: `just ci`; the guide edit is one commit for the owner.

T16 is built as six subtasks, in the order T16.a, T16.b, T16.d, T16.e, T16.f, T16.c (a ruled split,
under the orchestrator's pre-authorisation of 2026-10-06, when T16 moved to the views lane; T16.d
and T16.e were added after T16.a by decision-r07-t16a, and T16.f by decision-thin-line-contrast,
before the guide's draft, so that the draft states what they build). The paragraph above stays the
task's whole specification: each subtask builds its share of it, and each runs the console-ux
skill's scripts.

- **R07.T16.a The overlay, cased, and the hull faces' bias.** `photoreal/overlay.ts` (new),
  `displays/view/viewFrameDrawer.ts`, R02's `wireframe/drawList.ts` and `shaders/occluder.wgsl`,
  and `smoke/wireframe.ts` (Design notes 16–17; the UX decisions, item 12). The symbology's canvas
  pass over the tone-mapped image is R02's draw list with every line batch cased in `--surface-0`
  at `CASING_PX`, the hull edges included, which the wireframe style leaves uncased, and with no
  star sprites, which the image holds. It is labelled `symbology` and loads the tone-mapped image
  (`colourLoad: "load"`, through the sRGB view). Rings and hulls stay R02's cased marks until R11.
  `HULL_OCCLUDER_BIAS.slopeScale` goes from 2 to 3, the sphere occluder's `SLOPE_SCALE`, so that a
  cased hull edge's coverage stays in front of its own faces; the bias is one for both styles. The
  DOM readouts over the canvas, the label block and the marks' labels, stand on their
  `--surface-0` plates as built, and the label block's `STYLE` line and
  `BODY PHOTOMETRY: NOT YET MODELLED` are built (T7, T8.a, T19). Tests: in a scene with a hull, a
  ring, an orbit, a predicted path, bodies and marks, every batch of the overlay has a
  `--surface-0` casing of `CASING_PX` and the overlay has no sprite, while the wireframe's own list
  keeps its hull edges uncased; against the fake engine, the drawer's overlay submits each batch's
  casing before its stroke, under `symbology`; the hull material's bias is
  `{ constant: 128, slopeScale: 3 }`; in the photorealistic style every text over the primary's
  image is on a plate, and each plate's rule in `styles.css` paints `var(--surface-0)`; in
  `just test-render`, a cased hull edge on its own receding face draws every texel it draws with
  no face. Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/photoreal
src/renderer/src/view/wireframe src/renderer/src/displays/view`, `just test-render`, the console-ux
  skill's scripts, `just ci`.
- **R07.T16.b The meter's causes, and their words on the panels.** R07's `post/autoExposure.ts`,
  R02's `photometry/exposure.ts`, `displays/view/ExposurePanel.tsx`, `MeterControl.tsx` and
  `ViewDisplay.tsx` (decision-r07-t8a-meter, item 1; decision-r07-owner-ux-signoff, item 1). Every
  part of T16's text from "`AutoExposure` keeps why it has no metered value" to "stays held back
  under the operator's own": the cause (`no-image`, `nothing-weighed`, `acquiring`); R02's
  `InhibitReason` `"nothing_weighed"` with the meter in force, read as `INHIBITED · NO LIT SIDE`
  and its twins; the statuses `NO LIT SIDE`, `NO DARK SIDE` and `STAR DISC ONLY`, each with its
  remedy clause on the meter's control and bare in `ENABLE`'s reason and in
  `AUTO NOT AVAILABLE: …`, in the panel and under the compact layout's row
  (decision-r07-t8a-meter: the remedy is the meter control's); the `acquiring` window, with no
  status and `ENABLE` held back with `NOT AVAILABLE: not yet metered`; the `MAN` field never held
  back; and `INHIBIT`'s consequence under each new system inhibit. It also settles T8.a's consider,
  focus on a meter button lost when the panel unmounts on a style change, or records why not.
  Tests: T16's three (`NO LIT SIDE` after
  0.5 s and not before, never `NO IMAGE TO METER`; a meter change clearing it at once; `AUTO`
  resuming when a lit body is metered); `NO DARK SIDE` under `DARK` and `STAR DISC ONLY` under
  `AVG`; `ENABLE` and the `MAN` field in the window; the inhibit's reading and `INHIBIT`'s note at
  each new level. By hand, hidden: T19.b's follow-up's captures re-taken with `EXPOSURE` and
  `EXPOSURE METER` open and each status standing, the 0.5rem probe passing and the least height at
  most 52.5rem, or a ruling asked for. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/post src/renderer/src/view/photometry src/renderer/src/displays/view`, the
  console-ux skill's scripts, `just ci`.
- **R07.T16.d Strokes of at least 2 device pixels, and one slope term for both occluders.** R02's
  `wireframe/drawList.ts`, `wireframe/symbology.ts`, `wireframe/submit.ts`, `shaders/occluder.wgsl`,
  `shaders/occluderSphere.wgsl` and `scenes/precision.test.ts`, `photoreal/overlay.ts`,
  `displays/view/viewFrameDrawer.ts`, R05's `spike/spikeRun.ts` and `spike/DescentSpike.tsx`, and
  `smoke/wireframe.ts`, and `lib/strokes.ts` (new) (decision-r07-t16a, items 1 and 2;
  decision-thin-line-contrast, items 2 and 4). `STROKE_PX`, `CASING_PX`, `PREDICTED_DASH_PX` and the
  view's `SYMBOL_STROKE_PX` are the guide's CSS pixels. `lib/strokes.ts` holds
  `MIN_STROKE_DEVICE_PX` (2), `lineScale(devicePixelRatio)`, the larger of the ratio and 2,
  `markStrokeDevicePx(devicePixelRatio)`, the larger of 1.5 × the ratio and 2, and
  `markShiftDevicePx(devicePixelRatio)`, half the second less 0.75 × the ratio.
  `buildWireframeDrawList` takes a required `strokeScale`, the device pixels drawn for each,
  `lineScale` of the ratio from its callers, and multiplies every batch's width, casing and dash by
  it. It also takes a required `markStrokePx`, `markStrokeDevicePx` of the ratio, at which every
  symbology outline is drawn. Each outline widens outward, so that what lies inside it stays: a
  symbol's line moves out by `markShiftDevicePx`, a ringed circle's disc by that and its ring by
  three times that, and every reticle by four times that. The list carries `strokeScale`,
  `markStrokePx` and `occluderSlopePx`,
  ⌈(`STROKE_PX.heavy` + 2 × `CASING_PX`) × `strokeScale` ÷ 2 + 1⌉ px (Design note 5's
  w_max ÷ 2 + 1, rounded up: 3 at a scale of 1, 5 at 2). `overlayDrawList` cases every batch to at
  least `CASING_PX` × `strokeScale`. Both occluders take `occluderSlopePx`
  as a uniform and push their depth away in the fragment by that many pixels of the depth's screen
  slope, its magnitude: the sphere as built, in place of its `SLOPE_SCALE`; the hull faces from the
  derivatives of their rasterised depth, exact on a plane, less a constant of 2⁻¹⁶ of the depth
  (Design note 5's 128 units at the larger unit, now the same on every backend), written as
  `frag_depth`. The hull material sets no hardware bias, and `HULL_OCCLUDER_BIAS` and
  `OccluderMesh.depthBiasAway` go. No line, casing or symbology outline is narrower than 2 device
  pixels at any ratio; the antialiasing fringe and the stars' point-spread function stay in device
  pixels. Tests: at `strokeScale` 2 every batch's width, casing and dash are twice those at 1, and
  `occluderSlopePx` is 5; at ratios 0.78125, 1, 2 and 3, `lineScale` is 2, 2, 2 and 3,
  `markStrokeDevicePx` 2, 2, 3 and 4.5, and `markShiftDevicePx` 0.41, 0.25, 0 and 0; every symbology
  outline is drawn at `markStrokePx`; at those ratios, outside their as-built radii: a class-0 body
  symbol's line lies 0.41, 0.25, 0 and 0 px out; a ringed circle's disc lies as much out and its
  ring three times as much; a reticle's half-size four times as much; through the drawer, the hull's
  line draws under `symbology` are `[7, 3]` at ratios of 2, 1 and 0.78125 and `[10.5, 4.5]` at 3;
  the overlay cases to 2 at ratios of 0.78125, 1 and 2; both occluder materials take
  `occluderSlopePx`, and the hull material has no `depthBiasAway`; the precision scene's hidden-line
  tests take the constant 2⁻¹⁶ in place of their two ends. In `just test-render`:
  `checkCasedHullEdge` on a face whose depth gradient runs at 45° to the screen's axes, at scales 1
  and 2, draws every texel of the cased edge that it draws with no face, the casing's outer texel
  included; a hull face's depth at a diagonal texel equals the push by the slope's magnitude
  computed in `f64`, within a tenth of its difference from the larger component's;
  `checkSphereSlope` reads the uniform; R02.T14.c's show-through check passes unchanged.
  `checkStrokeContrast` draws on a `--surface-0` target: a 1 px `--text-muted` circle; 1 px
  `--text-muted` lines at 0°, 3° and 45°; a 1.5 px `--text-muted` line; a dashed `--text` predicted
  path; and an open body symbol in `--text` with its `--accent` reticle and a `--target` reticle.
  They are built by `buildWireframeDrawList` at the `strokeScale` and `markStrokePx` of ratios
  0.78125, 1 and 2, and again through the overlay over a loaded `--text`. At every texel of length
  along each stroke, the brightest texel across it reaches 6.0:1 against `--surface-0` by WCAG's
  formula on the 8-bit texel. A control at a `strokeScale` of 1 reads under 6 for the 1 px circle
  (decision-thin-line-contrast, item 4). By hand, not committed: with the hardware's slope term put
  back, the diagonal check fails on a backend that takes the larger component. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/wireframe
src/renderer/src/view/scenes src/renderer/src/view/photoreal src/renderer/src/view/spike
src/renderer/src/displays/view src/renderer/src/lib`,
  `just test-render`, the console-ux skill's scripts, `just ci`.
- **R07.T16.e Craft over the image: windows of glass, opaque faces as silhouettes, and their
  note.** R02's `scene/hull.ts`, `scenes/precision.test.ts`, `wireframe/drawList.ts`,
  `wireframe/submit.ts` and `shaders/occluder.wgsl`, `photoreal/overlay.ts`,
  `displays/view/viewRun.ts` and what passes its statements, and `smoke/wireframe.ts`
  (decision-r07-t16a, item 3; Design notes 16 and 17). `HullOutline` gains `windows`, the
  indices of its glazed faces, which `hullOutline()` refuses out of range or repeated;
  `TEST_HULL`'s plate, its last two faces, is its one window. In both styles a hull's
  `OccluderMesh` holds its opaque faces alone, so that a window hides nothing, and its edges stay
  drawn. `OccluderMesh` gains `fill`, a colour or `null`: the wireframe fills none; the overlay
  fills every mesh in the list's `--surface-0`, so that over the image a craft is its cased
  outline on a `--surface-0` silhouette. The `symbology` pass draws the bodies' occluder spheres,
  then the filled hull meshes, depth-written with T16.d's push, then the lines; the fill is
  opaque, its colour a uniform, drawn after tone mapping, so the meter is unchanged.
  `photorealStatements` adds `CRAFT PHOTOMETRY: NOT YET MODELLED` while the photorealistic frame
  is drawn and the scene has a craft, the own ship included, outside the early return for a scene
  with no lit body; with `BODY PHOTOMETRY: NOT YET MODELLED` it composes into
  `BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED` in that note's place. Tests: `hullOutline()`
  refuses a bad window index; `TEST_HULL`'s occluder mesh has its 14 opaque faces and none of the
  plate's; the overlay's meshes are filled in `--surface-0` and the wireframe's are not; against
  the fake engine, the drawer's `symbology` pass draws the filled meshes after the spheres and
  before every line; the precision scene's plate tests become the plate hiding nothing from the
  seat and the hull's own edges in front of its opaque faces; the note in the photorealistic
  style with a craft and none in the wireframe or while preparing, composed with the body note,
  and shown with craft but no lit body. In `just test-render`, over a loaded colour: a filled
  face reads `--surface-0` at its interior texels and leaves the colour beyond its edge and
  behind a sphere occluder in front of it; a cased hull edge behind a window draws as with no
  window; and a hull's own cased edge draws as in T16.d's check. By hand, hidden: captures at
  1920 × 1080 and 1280 × 720 from `SEAT` and from `CHASE` on a photorealistic primary over a lit
  body with a photorealistic `CHASE` instrument open, the composed note standing, the 0.5rem
  probe passing and the least height at most 52.5rem, or a ruling asked for. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/scene src/renderer/src/view/scenes
src/renderer/src/view/wireframe src/renderer/src/view/photoreal src/renderer/src/displays/view`,
  `just test-render`, the console-ux skill's scripts, `just ci`.
- **R07.T16.f Strokes of 2 device pixels on the spatial displays and the galaxy map.**
  **Files:** P05's `spatial/paint.ts` and `spatial/LegendSymbol.tsx`, `styles.css`,
  `displays/galaxy/DensityLegend.tsx`, `lib/strokes.ts`, `main.tsx`, a new `smoke/spatial.ts`
  (registered in `smoke/page.ts`), and the console-ux skill's `scripts/contrast.py`
  (decision-thin-line-contrast, items 2 to 4).
  **The canvas.**
  - `paint` draws every op but a symbol and a reticle with `lineWidth` set to its `widthPx` ×
    `lineScale(pixelRatio)` ÷ `pixelRatio`.
  - It draws a symbol's and a reticle's outline at `markStrokeDevicePx(pixelRatio)` ÷
    `pixelRatio`.
  - With δ = `markShiftDevicePx(pixelRatio)` ÷ `pixelRatio`, it moves outlines out: a symbol's
    path radius by δ, a ringed circle's disc by δ and its ring by 3δ, and a reticle's half-size by
    4δ.

  So a 1 px line is 2 device px below a ratio of 2, and an outline widens outward, keeping every
  hole and the ringed circle's gap. Nothing in `spatial/drawList.ts` or
  `lib/galaxy/hrProjection.ts` changes.
  **The DOM's SVG strokes.** `lib/strokes.ts` gains `strokeProperties(devicePixelRatio)`, with two
  properties:
  - `--line-scale`: `lineScale` ÷ the ratio;
  - `--mark-stroke`: `markStrokeDevicePx` ÷ the ratio, in px.

  `main.tsx` sets them on the root before the first render, and again whenever the ratio changes
  (`watchStrokeProperties`, a `matchMedia` resolution listener). `:root` holds their values at a
  ratio of 2, `1` and `1.5px`. The stylesheet's SVG strokes take them:
  - the galaxy map's cursor and centre marks, `var(--mark-stroke)` over a casing of
    `calc(var(--mark-stroke) + 2px * var(--line-scale))`;
  - the axis triad, the core arrow and every `.symbol-legend__mark`, `var(--mark-stroke)`;
  - the orbit legend's paths, `calc(1px * var(--line-scale))` and
    `calc(2px * var(--line-scale))`;
  - `DensityLegend`'s ticks, `calc(1px * var(--line-scale))`, through a class in place of
    `strokeWidth={1}`;
  - the disclosure chevron (`.glyph--disclosure polyline`), `vector-effect: non-scaling-stroke`
    and `stroke-width: max(var(--mark-stroke), 0.105em)`.

  `LegendSymbol` widens its outline outward as `paint` does: δ, and for a ringed circle δ and 3δ,
  converted to its box's units with the ratio and the root's rem from `useStrokeMetrics` (new in
  `lib/strokes.ts`, updated on a change of either). `SunGlyph` and `EarthGlyph` are text and stay
  as they are.
  **`contrast.py`** gains `--coverage C` (0 < C ≤ 1), which blends the foreground over the
  background at that coverage before scoring, and `--blend srgb|linear`: sRGB-encoded values by
  default (Canvas 2D, SVG and the DOM), or linear light (the view). Its docstring says a stroke
  scores its pair's ratio, to within 1%, only at 2 device pixels or more.
  **Tests.**
  - Through the recording context, at ratios 0.78125, 1, 2 and 3:
    - a line op's `lineWidth` is 2.56, 2, 1 and 1 times its `widthPx`;
    - a symbol's and a reticle's `lineWidth` is 2.56, 2, 1.5 and 1.5;
    - an open circle's arc radius is its as-built radius plus 0.53, 0.25, 0 and 0;
    - a ringed circle's disc arc is its as-built radius plus the same, and its ring plus 1.59,
      0.75, 0 and 0;
    - a reticle's half-size is its as-built size plus 2.12, 1, 0 and 0.
  - A class-2 open ringed circle at 0.78125 keeps the hole and the gap round its disc that it has
    as built: 1.56 device px at 100% and 0.94 at 80%.
  - `LegendSymbol`'s outline radius moves out by the same shifts in its box's units.
  - `strokeProperties` gives `2.56`/`2.56px`, `2`/`2px`, `1`/`1.5px` and `1`/`1.5px`.
  - `watchStrokeProperties` sets the root on a change of ratio and stops when disposed.
  - Each SVG rule named above, read as text from `styles.css`, takes its property.
  - `DensityLegend`'s ticks carry the class.
  - None of the strokes changed here sits on `--surface-2`, the one surface where a 2 px stroke in
    `--text-muted` comes within 1% of 6:1.
  - By hand, recorded in the as-built entry:
    `contrast.py text-muted surface-0 --coverage 0.5 --blend linear` prints 4.11:1 FAIL, and
    `--coverage 0.39` prints 1.97:1 FAIL.

  **In `just test-render`,** `checkSpatialStrokeContrast` (group "R07.T16.f spatial strokes")
  uses `paint` to draw a draw list on a 2D canvas whose backing store stands for the device, at
  ratios 0.78125, 1 and 2. The list holds: a 1 px `--text-muted` circle; 1 px lines at 0°, 3° and
  45°; a 1 px `--text` circle with its ticks; a 2 px `--text` polyline; an open class-0 circle
  and an open class-2 ringed circle in `--accent`; an `--accent` reticle and a `--target`
  reticle; and all of these again in the stale tokens. The check asserts:
  - at every device pixel of length along each stroke, the brightest pixel across it reaches
    6.0:1 against `--surface-0` by WCAG's formula;
  - the open circle's centre pixel and the ringed circle's disc centre read `--surface-0`;
  - a control, a 1 CSS px circle stroked directly at 0.78125, reads under 6.

  **By hand, hidden:** `GALAXY`'s local chart and the orbit map, captured at 1920 × 1080 at a
  forced device scale factor of 0.78125, for the owner's look at the guide's draft.
  **Acceptance:** `pnpm --filter hyperion exec vitest run src/renderer/src/spatial
src/renderer/src/displays/galaxy src/renderer/src/displays/system src/renderer/src/components
src/renderer/src/lib`,
  `just test-render`, the console-ux skill's scripts, `just ci`.

- **R07.T16.c The guide's draft, for the owner.** `docs/frontend/ux-guidelines.md` alone, in one
  `docs(guide)` commit, after T16.f, so that it states what is built: decision-r07-t8a-meter's "In
  T16" edits, fitted to the guide as T19.d signed it off (the data-state bullet's inhibit sentence
  and its last sentence; the `AUTO`, `MAN`, `INHIBITED` row; the `NO IMAGE TO METER` row's added
  sentence; the new Status row of the three statuses with their remedy clauses), and a pointer to
  that row from the `METER` row (decision-r07-t19b-exposure-fit, item 4), each tagged as T16's
  draft, ending in "the owner signs off". It also takes the two points left for the guide: the
  meter's `SELECT` legend, left "for T16's draft" by T13 after review (a `Label` row, or the reason
  a group's legend needs none), and the data-state bullet's "offers `MAN` only", which reads against
  E5 (T19.d's open points, for the owner's next guide edit). It also drafts decision-r07-t16a's
  guide text, as T16.d, T16.e and T16.f built it: the unit of a width and the floor of 2 device
  pixels, a Layout bullet; a stroke's contrast as drawn, a Colour bullet (both
  decision-thin-line-contrast); a view's strokes as Layout gives them, in the Views bullet's
  paragraph on both styles; the craft's silhouette, as the one exception in "Outlines for
  symbology"; the craft note's Label row; and the added clauses of the `WIREFRAME`, `PHOTOREALISTIC`
  row and the `TEST HULL` row. Nothing else is new: `BODY PHOTOMETRY: NOT YET MODELLED` is signed
  off (T19.d), and the several views' refusal adds no entry (decision-r07-t18, item 6). Tests: each
  status the draft adds is a string in the code, in the words T16.b built; each note it adds is a
  string in the code, in the words T16.e built; and each width and colour the text gives is the
  code's, `lineScale` and `markStrokeDevicePx` included (decision-r07-t16a;
  decision-thin-line-contrast). Acceptance:
  `pnpm exec prettier --check docs/frontend/ux-guidelines.md`, the console-ux skill's scripts.

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
provisional ones; R12 consolidates them.

T8.c, T8.d and T19.e land before these benchmarks (decision-r07-small-disc-cost). The
benchmarks include the `disc cells` and `discs` passes of a view holding:

- a disc of 3.3–4 px (8 × 8);
- one of 4–32 px (4 × 4);
- one of 32 px or more.

Each size is checked against the logged disc plan. Every pass time is recorded with the GPU's
clock beside it, and in cycles at that clock:

- `nvidia-smi` on the RTX 3080;
- on the UHD 620, `gt_act_freq_mhz` (the actual) beside `gt_cur_freq_mhz` (the requested).
  - These are read under `/sys/class/drm/cardN/`, the card whose `device/driver` is i915. It is
    not always `card0`.
  - They are sampled during the timed passes, since the actual frequency reads 0 while the GPU
    idles.

A permitted reorder (decision-r07-small-disc-cost): if the orchestrator needs `low` selectable for
the owner's UHD 620 runs before T8.d lands, T17 may go first. T8.d then re-takes T17's discs-pass
rows on the 3080, and the owner's UHD 620 runs wait for T8.d.

Tests (Vitest): `VIEW` given `low` draws its sky and its
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

`displays/view/InstrumentView.tsx`, `ViewDisplay.tsx` (Design notes 14 and 15): two slots, each with
its own camera, style and target, R01's `createView(canvas, name)`, R02's DOM list and label block,
and the exposure reading of the primary view; and T18's budgets wired into `VIEW` (decision-r07-t18,
item 1). `VIEW` builds a `ViewSpec` for the primary and for each open instrument in slot order, and
takes `viewBudgets(views, setting)` again whenever a view opens or closes, a camera's style changes
or the setting changes, the setting coming from one `VIEW` input (`high` until the client offers
`low`). Each view draws its budget's `style`, its label block's `STYLE` naming the style drawn, and
is paced at its `rateHz`: a 60 Hz view every animation frame, a 30 Hz view every second one, a 30 Hz
primary on every second vsync as R05 Design note 21 paces the low setting, and an instrument only in
frames the primary draws. Each view's terrain demand carries its `streamPriority` (R05). A
photorealistic view's scene target is made at its render resolution times its scale: the budget's
`renderScale`, or, while its `control` is set, the `scale` of a `ResolutionController` made from
`control` when it first appears, `retarget`ed when its target changes (an instrument opened or
closed, the setting changed) and dropped when `control` returns to `null`. The controller is updated
once a primary frame: `gpuFramesMs` holds `gpuTimeMs` of every view's `PassTimes` submitted from one
primary frame to the next, one entry for each primary frame once all its resolves are in (none on
some updates), or is `undefined` while `GraphicsStatus.timer` is `absent`; `intervalMs` is the
interval between the primary's frames. The style control and the key `4` ask `photorealisticAllowed`
before a switch to the photorealistic style; a refusal holds the button back with its reason, the
adapter's refusal (`styleRefusal`) first where both hold. The engine numbers its timing frames
through R01's `RenderEngine.passTimesFrame`, added here (decision-r07-t19, item 1): the timer's
latest resolve number, 0 before any and while `ResilientEngine` has no engine, restarting from 0 at
a restore; a resolve dropped while every buffer is in flight still takes its number, and
`TIMING_FRAMES_IN_FLIGHT` is raised to 64, or to three frames' resolves of a photorealistic primary
with two instruments if more; tested in the engine's Vitest suite and run under `just test-render`.
`VIEW` reads it at the start of each primary frame; the previous frame's group is the numbers since
the previous read, an empty range giving no entry, a complete group the sum of its reports'
`gpuTimeMs`, a group still incomplete when a later one completes discarded, and every pending group
discarded at `onRestored`. Layout (decision-r07-t19, item 2): the slots `INSTRUMENT 1` above
`INSTRUMENT 2` stand over the stage's right edge, inset `0.5rem`, each a panel holding its label
block (`VIEW`, `FRAME`, `TIME`, `STYLE`, `CAMERA`, `FOV`, `EXPOSURE`, `SOURCE`, `STARS` and the
view's statements; `SCENE` on the primary's only) beside a 4:3 canvas of `15rem × 11.25rem`; no slot
covers the primary's label block or annunciations, which take the width left of the open slots, and
a slot with no room has its `OPEN` held back. A side-column panel `Instruments`, first in the
column, holds an `OPEN`/`CLOSE` pair for each slot (both `CLOSE` when the display mounts) and the
selector `CONTROLS` (`PRIMARY`, `INSTRUMENT 1`, `INSTRUMENT 2`, a closed instrument's held back with
`NOT AVAILABLE: INSTRUMENT 1 is not open`), which points the `Targets`, `Camera` and `Style` panels,
each designated with that view, and the single keys pressed off a canvas; a pointer press on a
canvas or a view key pressed on a focused canvas sets `CONTROLS` to that view first, and closing the
controlled instrument returns it to `PRIMARY`. The exposure is the primary's alone: the `Exposure`
and meter panels are designated `PRIMARY`, and an instrument shows the primary's reading with
`SOURCE PRIMARY`, `MeterControl`'s source taking the same view names. Canvases are named `VIEW,
<style drawn>, <slot name>, <preset>`. Tests (Vitest): each view focusable and named; keyboard
reaches every camera control in every view; the style control of a second view is disabled on low
with `NOT AVAILABLE: QUALITY LOW allows one photorealistic view`, and the key `4` refused there; a
wireframe instrument shows the source of its exposure; against a fake engine, opening an instrument
beside a photorealistic primary gives it a controller whose scale sizes the scene target, closing
the instruments returns it to the bounds' max, a 30 Hz view draws in every second frame, an
instrument's pass times count in the primary's frame, and an absent timer feeds `undefined`;
`passTimesFrame` grouping with a hole, a restore and an absent timer; Tab reaches each open
instrument; a key on a focused instrument sets `CONTROLS` and acts on it, Tab past the canvases
leaves `CONTROLS` alone; the designators follow `CONTROLS`; hidden screenshots at 1920×1080 and
1280×720 with both instruments open, nothing overlapping a reading or clipped. Acceptance: `pnpm
test`, `just ci`.

T19's paragraph above is R07.T19.a, built as R07.T19 (7e80d5d, guide draft 887fa4a); its layout
and frame follow-ups are two subtasks (decision-r07-t19-layout).

- **R07.T19.b `VIEW` at both sizes.** `ViewDisplay.tsx`, `styles.css`, `ViewLabelBlock.tsx`,
  `InstrumentView.tsx`, `ViewMarkList.tsx`, `viewRun.ts` (decision-r07-t19-layout, items 1–3 and
  5). Two layouts, chosen from the `.view` box's size alone and never from what it shows:
  **full** where the box is at least 98.5rem wide (a 50.5rem stage, two slots' room beside the
  primary's least label block, then 26rem and 20rem columns and their gaps) and as tall as
  column A's tallest state with the list at two rows (about 52rem; the measured figure recorded),
  which 1920×1080 at 100% is; **compact** otherwise, as 1280×720 and 1920×1080 at 125% and 150%
  are. Full: column A, 26rem, holds `Instruments`, `Targets` (flex), `Camera` and `Style`; column
  B, 20rem, `Exposure` and `Exposure meter`; focus runs through A before B. Compact: one 26rem
  column in the same order, its panels at `0.5rem` block padding: `Instruments` folds to its
  title row, a 2rem disclosure button, folded when the display mounts; `Targets` is always on
  show, at least its head, one row and its position; under it one row of disclosure buttons,
  `CAMERA`, `STYLE`, `EXPOSURE` and, while the meter panel would stand, `EXPOSURE METER` last;
  then the one open panel. Exactly one of `INSTRUMENTS`, `CAMERA`, `STYLE`, `EXPOSURE` and
  `EXPOSURE METER` is open, `CAMERA` by default; opening another folds the one open, folding it
  opens `CAMERA` again, and focus in a panel that folds goes to its button. While its panel is
  folded, `NO OWN SHIP: SEAT and CHASE need one`, `GRAPHICS STYLE REFUSED: …` and
  `AUTO NOT AVAILABLE: NO IMAGE TO METER` stand under the row; the limit reasons fold with their
  controls; single keys stay live. Every side panel takes `contain: inline-size`; the column never
  scrolls, the list alone does. So that every setting a folded panel holds is on show: every
  view's `CAMERA` value in `FREE` reads `FREE · RATE 1.00 km/s`, and `PAGE UP` and `PAGE DOWN`
  step the rate only in `FREE`, as the flight keys move only the free camera; the primary's block
  gains `METER AVG` (`LIT`, `DARK`) while the meter control stands, and the statement
  `EASED CAMERA MOVES` while that setting is on and applied. Label lines break at `·` first, then
  at a space, never inside a number with its sign, unit, band or prefix (`EV100 -1.0`, `60°`,
  `1.00 km/s`), between a star limit and its kind (`V 9.5 mag CAM`), after `UT` or inside a clock
  reading; `EV100 -1.0 MAN` stays whole where it fits; continuation lines hang under the value.
  `INSTRUMENT 1` stands at the stage's top right and `INSTRUMENT 2` at its bottom right, each
  inset `0.5rem`, so that neither moves when the other opens or closes; where the stage cannot
  hold both corners `INSTRUMENT 2` stands directly under `INSTRUMENT 1`, never over it. An
  instrument's block carries each photorealistic statement (`LIGHTING: …`, `BODY PHOTOMETRY: NOT
  YET MODELLED`) that holds for its picture unless the primary's block shows the same line, its
  statements running the slot's width under its label block and canvas. With no own ship the
  list's `RANGE` head reads `RANGE FROM CAMERA` and its rows the bare range, each option's
  accessible name keeping "from camera". T13.c–e and T16, which add to the exposure panels, each
  re-take the compact capture with `EXPOSURE` and `EXPOSURE METER` open. Tests (Vitest): the
  layout at each threshold; full's six panels in two columns in the ruled focus order; compact's
  disclosures (one open, `CAMERA` by default and again on folding, `INSTRUMENTS` folding the open
  panel, the meter's button only beside a drawn image, focus moving to the button), its tab order
  that of full among the parts both show, and its standing lines; the rate keys refused outside
  `FREE` and the rate on the label block in `FREE`; `METER` and `EASED CAMERA MOVES` on the
  primary's block; an instrument's photorealistic statements beside a wireframe primary and not
  beside a photorealistic one; the slots' anchors; the list's head; `.console__work`'s `scrollTop`
  0 after Tab through every control. By hand, hidden and never on `:0`: captures at 1920×1080
  (full), and at 1280×720 and 1920×1080 at 125% (compact), with both instruments open over a kept
  scene and over `PHASE TEST` all photorealistic, and in compact with each of its five panels open
  and with the camera's limit reasons standing; measured, the column's `scrollHeight` equal to
  its `clientHeight`, no text truncated (`DESIG` included), no slot over a reading or the other
  slot, and at 1920×1080 no value line broken but at `·`. Acceptance: `pnpm test`, `just ci`.

  Its follow-up (decision-r07-t19b-exposure-fit, items 1–5), after T19.c and T13.d's follow-up: in
  the full layout `Style` (the `CONTROLS` view's) heads column B, so that column A holds
  `Instruments`, `Targets` and `Camera` and column B `Style`, `Exposure` and `Exposure meter`, the
  order, the tab order and the compact column unchanged, and `Style` stands in every engine state,
  its buttons held back with `NOT AVAILABLE: no view is drawn` while no view can be drawn (in
  compact its button stands too); the least height is the taller column's tallest state with the
  list at two rows, measured and rounded up to 0.25rem, never above 52.5rem, so that a maximised
  1920 × 1080 window (a box of about 53.5rem) is full. The side column sets `line-height: 1.25`,
  so that its heights hold on every platform and scale. In the compact layout its parts stand
  0.25rem apart, and the exposure's camera setting stands in two rows of two members, `APERTURE`
  and `SHUTTER`, then `ND` and `ISO`, each member a `dt` and `dd` wrapped together, its label
  0.75rem from its value and the members 1.5rem apart. The `MAN` field's consequence reads `An
  entry sets MAN: AUTO resumes only on ENABLE`; the exposure panel's and the meter's readings break
  as the label block's do, at `·` first; `ENABLE` and `INHIBIT` keep a row each, their reason or
  consequence beside them with no bottom margin, so that it centres on the button's label, the rows
  0.75rem apart in the full layout's column B, where both can wrap. The
  guide drafts the `METER` and `AVG` rows. Tests: the threshold and its 52.5rem bound; `Style`
  first in column B, for `PRIMARY` and an instrument, and standing in every engine state; the
  order unchanged; the consequence's words; the readings' parts. By hand, hidden: with `EXPOSURE`
  open at 1280 × 720 and 1920 × 1080 at 125% and 150%, photorealistic under `AUTO`,
  `INHIBITED · OPERATOR` and `MAN` and wireframe trapped, each with and without a refused entry,
  and with an own ship trapped, refused or not; in full at 1920 × 1080, at a 53.5rem box and at the
  least height, with `CONTROLS` on an instrument too; measured, the column's `scrollHeight` equal
  to its `clientHeight`, no control below its foot, nothing truncated, each setting member's label
  0.75rem from its value and 1.5rem from the value before it, and in compact and at the 53.5rem box
  the same with a 0.5rem probe after the open panel. T16, and any later task that adds to a side
  panel, re-takes these captures, keeping the probe passing and the least height at most 52.5rem,
  or asks for a ruling. Acceptance: `pnpm test`, `just ci`.
- **R07.T19.c One frame path.** `viewFrameDrawer.ts`, `ViewDisplay.tsx` (decision-r07-t19-layout,
  follow-ups): the primary draws through `ViewFrameDrawer` as the instruments do, with T8.a's
  metering taken only by the exposure's source; a photorealistic instrument's renderer takes no
  histogram, since nothing reads it (Design note 11). After T19.b, ordered by the orchestrator
  against the shading lane's tasks that change the frame (T9, T10.a, T10, T13.e), and before T20,
  whose figures it changes. Until it lands, a task that changes what the primary's frame draws
  changes `ViewFrameDrawer` alike. Tests: against the fake engine, the primary and an instrument
  with the same camera, style and size submit the same passes but the histogram's, and an
  instrument submits no histogram; T8.a's metering tests pass unchanged. Acceptance: `pnpm test`,
  `just test-render`, `just ci`.
- **R07.T19.d The owner's sign-off of `VIEW`'s wording** (decision-r07-owner-ux-signoff). After
  T19.b's follow-up, and before T16, which starts from it. `INHIBIT` is held back under
  `INHIBITED · OPERATOR` with `NOT AVAILABLE: the exposure is INHIBITED · OPERATOR` (R02's
  `exposure.ts` refuses it with `already_inhibited`), the level phrase unbroken, as `ENABLE` is
  under `AUTO`. `LIGHTING: STAR DISCS NOT RECEIVED` becomes `LIGHTING: NOT RECEIVED`,
  `ROTATION NOT YET MODELLED` becomes `ROTATION: NOT YET MODELLED`, and `CAMERA REPORT REFUSED`
  and `CAMERA REPORT UNANSWERED` become `CAMERA REPORT REJECTED` and `CAMERA REPORT TIMED OUT`.
  The guide's drafts from R02.T2.f, R02.T15, R02.T17, R05.T13.b, the decision of 2026-10-02 and
  R07 are signed off with the ruling's amendments in one `docs(guide)` commit, all but the
  `INSTRUMENT 1`, `INSTRUMENT 2` row, which waits for the owner's look at the slots. Tests:
  `INHIBIT` refused and described under the operator's inhibit, still offered under `AUTO` and a
  system inhibit; the renamed strings; no draft tag left but that row's. By hand, hidden: the
  compact `EXPOSURE` captures E3 and E7 (the trapped wireframe under `INHIBIT`) pass
  decision-r07-t19b-exposure-fit's probe, `INHIBIT`'s row at most 36 px. Acceptance:
  `pnpm test`, the console-ux skill's scripts, `just ci`.
- **R07.T19.e Nothing drawn for what is off the view** (decision-r07-small-disc-cost, item 5;
  the open points of T19's off-view cut).
  - **What it does.** It changes no texel. It removes full-view draws that only discard:
    - R06's host disc (`view/sky/disc.ts`, `shaders/disc.wgsl`'s vertex stage):
      - No draw for a host whose disc lies wholly beyond the view widened by
        `OUTSIDE_VIEW_MARGIN_PX`. Its glare source is unchanged: the eye's 45° reach is its own.
      - Each drawn host's triangle becomes a quad over `sphereScreenRect` of its apparent centre
        and radius (the whole view where the silhouette reaches behind the near plane).
    - R02's occluder spheres (`packWireframe`, `wireframe/submit.ts`): none packed for a sphere
      wholly off the view.
    - T9's `sphereFootprint` (`bodies/regime.ts`): none for a body wholly off the view, so it
      promotes nothing.
    - `sphereOutsideView` moves to `wireframe/submit.ts` beside `sphereScreenRect`, R02's level,
      and `regime.ts` imports it.
  - **Tests.**
    - Each of the three gives no draw, record or footprint for a body or host beside, behind or
      across the camera's plane, and keeps one the edge cuts or whose limb is a pixel past it.
    - T19's mesh case, a body clear of both, no longer promotes.
    - A host's quad covers every pixel its full-view draw lit, against the old path on a sweep of
      disc sizes and positions.
  - **Acceptance.** `pnpm test`; `just test-render` with every capture byte-identical to the base
    run first; `just ci`.
  - **Lanes.** Its files are disjoint from T8.c's and T8.d's, so either lane may build it. It
    lands before T17. R06's lane is told of the change to its files.
  - _As built (2026-10-06, the shading lane, after R06's limb-depth follow-up): see Risks,
    "Deviations in T19.e, as built"._

#### R07.T20 Several views, by hand

After T19.b and T19.c (decision-r07-t19-layout), with the real styles on the development machine
(RTX 3080) and, by the owner, on the UHD 620, each
on a quiet machine: a full-window photorealistic view and two wireframe instruments, each the right
way up, no GPU time in copies, a resize of one leaving the others' attachments alone, the frame time
with instruments open against the low setting's 33 ms on the UHD 620 (brainstorm, Testing), and the
per-canvas overhead that replaces `PER_CANVAS_OVERHEAD_MS`'s provisional 0.3 ms. The pass timer's
drop warning never appears in these runs (decision-r07-t19, item 1). On the UHD 620's low setting,
also a wireframe primary with two wireframe instruments, and one with a photorealistic and a
wireframe instrument, against R05 Design note 21's criteria at 60 Hz (T the display's measured vsync
period). A miss of the first moves the low setting's wireframe primary to 30 Hz as a `budget` field;
a miss of the second alone puts the photorealistic instrument's scale under the controller, against
the primary's period, rather than lowering the primary's rate (decision-r07-t18, items 4 and 5).
Recorded in this plan. Acceptance: the record.

- **By hand, for the owner (harness prepared 2026-10-05).** On a quiet machine, the window shown:
  `just views-check` on the RTX 3080 (the high setting, 1920 × 1080) and `just views-check
  --setting low` on the UHD 620 (1280 × 720), each about four minutes, no server needed. It drives
  `VIEW` on `PHASE TEST` through five traced phases (the photorealistic primary alone; with two
  wireframe instruments, then a resize of the primary alone; a wireframe primary with two
  wireframe instruments; with a photorealistic and a wireframe instrument; the wireframe primary
  alone), asks at the end whether every view is the right way up, and writes
  `docs/measurements/several-views/<date>-<machine>-<setting>.json` and `.md`: each check above
  with its verdict, and the per-canvas overhead's upper bound beside the provisional 0.3 ms. The
  checklist is in that directory's README. The UHD 620 run's sky and photorealistic frame stay at
  `high` until T17 (see "Deviations in the T20 and T21 harnesses"). When the records land, each
  run's verdicts are entered here as one line. `PER_CANVAS_OVERHEAD_MS` stays at 0.3 ms until
  then (ruled 2026-10-05), and changes on the owner's ruling of which figure it takes.
- **The small disc's cost and the second criterion** (decision-r07-small-disc-cost). Once T8.d
  lands, a small disc's cost is throughput and grows with the view's pixels, so the remedy above
  for a miss of the second criterion (the instrument's scale under the controller) can act on
  it. Before T8.d it could not: lowering the scale does not shorten a chain, and can push a disc
  into the 8 × 8 band. A miss whose `discs` pass holds a disc of 32 px or more is the residual
  (a large disc's 4 × 4 limb, 0.47–0.54 M cycles) and is recorded as a finding.

#### R07.T21 A child window on a second monitor

R01.T13 records the child-window prototype, with the research probe as its first data. This task
adds what that probe could not: an instrument view in a same-origin child window on a second
monitor, on R01.T13's prototype branch, recording whether Chromium on X11 paces it from that
display's vsync, and its frame times beside the main view's; and that the prototype's release of the
child's context holds there: at the opener's `close()` of the child, with the child's `pagehide` for
every other close (R01.T13's rule as this task amended it; see below). Done when a second display
is available. Acceptance: the record.

- **By hand, for the owner (harness prepared 2026-10-05).** With a second display connected and the
  desktop extended onto it, at another refresh rate than the first where one is to hand: `just
  child-window-check --seconds 60`. The opener stands on the primary display and a same-origin
  child, a view of the opener's engine, on the second; the child is resized at 30 s and closed at
  60 s, its view dropped at the opener's `close()` of it, and the opener draws on for 2 s. It writes
  `docs/measurements/several-views/<date>-<machine>-child-window.md`: the displays, the child's
  frame intervals against its display's period beside the opener's, both views' GPU time, the
  release and the uncaptured errors after the close. With one display it refuses (exit 2) and
  opens nothing. The checklist is in that directory's README; the record's verdicts are entered
  here as one line when it lands. R01.T13's prototype branch no longer exists; this harness
  rebuilds it (see "Deviations in the T20 and T21 harnesses").
- **The release rule, amended (2026-10-06), for the next builder of child windows.** The hidden
  smoke found the child's `pagehide` one of the opener's frames after the opener's `close()`, which
  R01.T13's rule had assumed could not happen. The rule now: where the opener closes a child
  itself, it drops the child's view (its context and attachments) at its `close()`, before the
  call; the child's `pagehide` drops it for every other close, the user's among them; whichever
  comes first drops it, and the other does nothing. R01's T13 as-built notes hold the rule as it
  stands. It is built as `holdChildView` in this task's scene (`renderer/src/smoke/childWindow.ts`),
  the one place it lives while the client opens no window; R07's `InstrumentView` in a child window,
  when it is built, takes the rule from there. See "Deviations in T21's release at the close".

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
- **The way back to `MAN` (ruled, decision-r07-man-exposure; T13.d).** `MAN` is entered by an
  EV100 field in `ExposurePanel`, never a button, starting from the exposure as it stands, at the
  view camera's triple for that EV100 (T13.c). Editable `APERTURE`, `SHUTTER`, `ND` and `ISO`
  fields are not built; one that allowed a shutter beyond 1/30 s would have to raise the sky
  request's limit with it.
- **One camera (ruled, decision-r07-exposure-camera; T13.c, T13.e).** f/1.4, 1/8,000–1/30 s,
  ISO 100–409,600 and a variable ND, on R06's sensor, at every level. Three idealisations remain:
  the ND is continuous, where a real camera would step a filter wheel (an OD 5.0 solar filter, as
  MER's Pancam and MSL's Mastcam carry) and let the shutter fill between, which moves neither the
  image nor the limit; darker than EV100 −6.12 the picture is pushed beyond the sensor's top gain
  (`ISO 409,600 ↑`) without the noise a push brings, the camera's cut standing in for it; and the
  1/30 s frame is never smeared. A floor on `AUTO` at the camera's range, so that a dark sky is
  left underexposed as a live camera leaves it, would change the image and eye views share the
  controller: not built, the deep-space look judged in T13.b's by-eye checks. The cull takes the
  camera's deepest limit and the label the exposure's own (T13.e), a departure from the
  brainstorm's "each view thresholds its copy" only where the threshold would cut stars already
  invisible.
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
  sphere √(a c); an oblate body in another's shadow errs at first order in f: about f ÷ 5 of a
  small central shadow's share under Lambert (0.104% against 0.106% for Io's on Jupiter) and up to
  f ÷ 2 in an ingress's length. The code keeps the sphere; integrating on the spheroid itself is
  not built. _Reworded by
  the orchestrator's ruling on T10.b (2026-10-06), from "errs at the second order in f", after
  T10.b's science check (see "Deviations in T10.b, as built")._
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
  promotion rule as depth-writing geometry. Until then a craft over the photorealistic image is a
  `--surface-0` silhouette of its opaque faces under its cased outline, labelled
  `CRAFT PHOTOMETRY: NOT YET MODELLED` (T16.e). The glare and bloom of a source it hides still
  spread around it, the meter weighs the pixels it hides, and its shape is the stand-in's
  (decision-r07-t16a, item 3).
- **The main screen is gated** on a plan that does not exist. Its task list is written against the
  brainstorm and will be re-fitted (T22–T28 each begin so); the sessions plan may choose a command
  envelope that reshapes `MainScreenCommand`, and must accept input sent on change.
- **Extrapolating the main screen's camera** departs from the brainstorm's interpolation (Design
  note 22); T28 compares both, and the owner may keep interpolation at the cost of about 16 ms.
- **The loop's worst case** is about 102 ms with a keyboard and 119 ms with a gamepad against the
  100 ms requirement (Design note 22). If T28's record confirms it, the cuts in order are the render
  and present term (25–33 ms, through the main screen's internal scale) and the animation-frame
  wait.
- **A child window on a second monitor** is unproved until a second display is at hand (T21);
  its harness is ready (see "Deviations in the T20 and T21 harnesses").
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
  `occluder.wgsl` depth bias, `slopeScale` 2. _Built by T16.a (2026-10-06): cased over the image,
  `slopeScale` 3 in both styles; see "Deviations in T16.a, as built"._ _The slope term moves into
  the fragment and follows the strokes' scale in T16.d (decision-r07-t16a, items 1 and 2)._
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
    `GaussLegendreRule`. _R07.T10.b moved `gaussLegendre` and `GaussLegendreRule` to
    `lighting/quadrature.ts`._
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
  real averaging meter would. **Ruled** (decision-r07-exposure-camera): one view camera for every
  level, its sensitivity never below base: 1/30 s with ISO 409,600 to 100, then the shutter to
  1/8,000 s, then an ND; darker than EV100 −6.12 a digital push read `409,600 ↑`. Built in T13.c;
  the sky's limit reads it in T13.e.
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
  could not leave (a test drives the cut and the recovery). A frame of exact zeros never comes
  into range, so the floor is bounded below at EV100 −14, R02's `MAN_EV100_MIN`: max(2⁻¹⁴ ÷ the
  pre-exposure, 2⁻¹⁷ cd/m²) (decision-r07-t13d; T13.a's follow-up), which leaves to its light every
  frame with a counted pixel in range on its way to −14. A histogram with nothing the meter weighs
  (`LIT` with no lit body) is
  treated as no histogram: `AUTO` holds until
  `METER_TIMEOUT_S`, then reads `INHIBITED · NO IMAGE TO METER`, which goes beyond the guide's
  definition of that status. T16 replaces it with the meter's own status, `NO LIT SIDE` and its
  twins (decision-r07-t8a-meter; built by T16.b, see "Deviations in T16.b, as built"). Under
  `AUTO` `onMetering` receives the smoothed, applied EV100,
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
    _Removed by R07.T10.a: no view called it, and `lightingFrameOf` places each body retarded,
    a contact (`ViewBody.retarded` `null`) as no occluder._
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
    (`lightingBodyOf`, T6.c; decisions-r06-r07, item 4; _since R07.T10.a, `lightingFrameOf`_).
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
- **Several views, stated limits (decision-r07-t19).** A photorealistic instrument is exposed by the
  primary's reading, not metered from its own image (open). At 1280×720 two open instruments cover
  most of the primary's image, the operator's choice. The spike's `ResolveCounter.runFrame` drifts
  by one per dropped resolve now that a drop takes a number; the spike's own warning flags such
  runs.
- **Deviations in T19, as built** (2026-10-04).
  - **Files.** `displays/view/InstrumentView.tsx` (a slot), `InstrumentsPanel.tsx` (the
    `Instruments` panel), `InstrumentControls.tsx` (an instrument's `Targets`, `Camera` and `Style`
    panels), `useInstruments.ts` (the slots' state, commands, skies and loop), `viewFrameDrawer.ts`
    (`ViewFrameDrawer`, an instrument's frame in either style with R06's layers and cube, and
    `skySprites`, which the primary now imports (until T19.c)) and `viewNames.ts`
    (`PRIMARY_VIEW_ID`, `PRIMARY_NAME`, `INSTRUMENT_SLOTS`, `instrumentName`, `instrumentViewId`,
    `viewDisplayName`);
    `view/budget/framePacing.ts` (`drawsInFrame`, `PrimaryFrameTimes`, `PENDING_PRIMARY_FRAMES`,
    `BudgetedScale`); `view/photoreal/internalScale.ts` (`internalViewport`, `spritesAtScale`);
    `test/viewDisplayHarness.tsx` (`timedEngineSource`, `renderViewDisplay`, `nominalStore`,
    `openUniverse`, `sceneArrives`). `viewRun.ts` gains `followRun`, `startInstrumentRun` and
    `POSITIONS_FROM_SHIP`; `useViewSky.ts` `cullViewSky` (its cull, shared); `styleRefusals.ts`
    `withPermission`; `ViewLabelBlock` an `id` and readings that break at their `·` first;
    `CameraControls`, `StyleControl`, `ExposurePanel` and `MeterControl` an optional `designator`, a
    span in the title, so their regions are named `Camera PRIMARY` and so on. R01:
    `RenderEngine.passTimesFrame`, `PassTimer.frame`; the fakes' `passTimesFrame`, `reportPassTimes`
    and `raiseRestored`.
  - **The setting** is `ViewDisplay`'s prop `setting`, `high` by default, read by the budgets alone;
    the sky and the photorealistic frame stay at `high` until T17 (accepted by the orchestrator as
    the smallest reversible choice, 2026-10-04).
  - **Two frame paths.** The primary keeps its own loop, which T8.a's metering shares; the
    instruments draw through `ViewFrameDrawer`, whose frame code mirrors the primary's (the
    wireframe list, R06's band, discs and cube, the photorealistic frame), so that the two lanes'
    edits to `ViewDisplay.tsx` merged apart. One path for both is a follow-up. The primary's draw
    reads the merged availability (the adapter's, then the budget's), which equals its budget's
    `style`. **Made one by T19.c** (see "Deviations in T19.c, as built").
  - **Pacing.** The primary's `requestAnimationFrame` loop counts animation frames and returns from
    those `drawsInFrame` refuses its `rateHz`; the instruments are drawn from inside the primary's
    frame (`InstrumentsFrame`) at their 30 Hz, in the same phase, and publish their readouts in the
    primary's 4 Hz frame, so that React renders the stage once for all three.
  - **The scene target.** `photorealFrame` takes the internal viewport, `internalViewport(viewport,
    scale)` (the height at the width's factor), and the draw list's stars carried to it by
    `spritesAtScale` (positions about the centre by k, light per point-spread weight by k²).
    `PhotorealRenderer` is unchanged: its `#follow` resizes the target and the bloom chain, and
    refits the bloom kernel at each new scale, about 35 ms (provisional; T20 records it). The render
    resolution is the canvas's; the low setting's 720 rows are T17's. `BudgetedScale` also makes a
    new controller should the bounds change (equal on both settings today).
  - **Grouping.** `PrimaryFrameTimes.startFrame(engine.passTimesFrame)` at the start of each primary
    frame; reports that arrive before their group ends are held until it does; at most
    `PENDING_PRIMARY_FRAMES` (8) groups are awaited while no report arrives; a mark that goes back
    starts over, as `onRestored` does.
  - **`TIMING_FRAMES_IN_FLIGHT` is 135**: three frames of the heaviest the views allow, a
    photorealistic primary with two photorealistic instruments, 45 resolves in the fake engine (15
    each); with wireframe instruments 17, under 64 for three.
  - **Instruments.** A run of R02's `camera` role following the primary's scene each frame
    (`followRun`), opening at `CHASE` where there is an own ship, its camera reported to the
    server's scene while open. Its exposure is the display's control, under `AUTO` the primary's
    applied value as it reaches that state (4 Hz, 0.1 EV). Each culls the primary's sky for its own
    camera (`cullViewSky` at its role, its field of view and its canvas's width; R06 Design note
    20), so its `STARS` states its own limit and its cube is baked for its own selection. A
    photorealistic instrument passes `meter: "average"`, and its renderer still takes a histogram
    nothing reads (a cost for T20; T19.c removes it). Its canvas has no DOM mark labels (its list
    names the marks).
    Its statements are decision-r07-t19's list (`POSITIONS AS SEEN FROM SHIP`, `PHOTOREALISTIC:
    PREPARING`, its graphics fault); the scene's (its lighting, its bodies' labels, `ROTATION NOT
    YET MODELLED`) stay the primary's, and T19.b gives an instrument the photorealistic
    ones its primary's block does not show (decision-r07-t19-layout, item 5). An instrument opened
    during a device loss makes its view at once and its
    renderers at the restore. A new scene remounts the stage, so it closes the instruments. VIEW
    draws no terrain yet, so no demand carries `streamPriority`; the plan that adds terrain to
    `VIEW` takes each view's R05 weight from it.
  - **The `Instruments` panel.** Legends floated beside their buttons, as the form choices do. A
    reason is one line however many buttons it holds back (`NOT AVAILABLE: INSTRUMENT 1 and
    INSTRUMENT 2 are not open` while both are closed, the ruling's wording for one), so that the
    column keeps its height. While no view is drawn (the graphics' annunciation in the stage's
    place) every `OPEN` is held back with `NOT AVAILABLE: no view can be drawn` and `CONTROLS` stays
    `PRIMARY` (UX review). The room for one more slot is reckoned from the open slots' measured
    column, a first slot at 16 rem, beside the primary's label block at a least 14 rem (UX review).
    The focus return of the ruling's closing rule is not built: `CLOSE` takes the focus before the
    slot closes, so the closed canvas never holds it.
  - **Layout, as measured** (hidden window, never shown, Electron's `capturePage`, at 1920×1080 and
    1280×720 CSS px; both instruments closed, both open over a kept scene, and all three
    photorealistic in `PHASE TEST`). A slot is 555 px wide and 254 px tall over the kept scene (a
    288 px label block of twelve lines at line height 1.2, `STARS` over three, beside the 240 × 180
    px canvas), 233 px in `PHASE TEST`: two stand 33.25 rem, not the ruling's estimated 29.5 rem,
    since the label block, not the canvas, sets the height. On the stage nothing overlaps a reading
    or is clipped, at either size, in any of the three states. **Ruled in
    decision-r07-t19-layout (items 2–3, built by T19.b):** at 1280×720
    with both open the primary's label block is 230 px wide, and `FRAME`, `TIME`, `EXPOSURE` (`EV100
    -1.0` and `MAN`) and `ROTATION NOT YET MODELLED` break at spaces too, against decision-r07-t19
    2f's "no line breaks except at `·`"; and the slots pack upward, so `INSTRUMENT 2` opened alone
    stands at the top until `INSTRUMENT 1` opens.
  - **The side column (ruled in decision-r07-t19-layout, item 1; built by T19.b).** It fits at
    1920×1080 in the wireframe, both closed and
    both open, after the column's gap went to 0.5 rem, its panels' block padding to 0.75 rem (the
    `Instruments` panel's to 0.5 rem), their titles' margin to 0.25 rem and the targets list's least
    height to two rows. It overflows at 1280×720, as it did before T19 (the `Exposure` panel already
    stood below the stage's foot; `Instruments` pushes `Style` down too), and at 1920×1080 in the
    photorealistic style once the meter panel shows, as it has since T8.a's metering. T19's by-hand
    check ("nothing … clipped") is therefore unmet in the side column at those sizes; a rule for a
    short page (G 252–255's rearrangement) is needed. T13.c's camera setting, four rows at every
    level, makes the 1920 × 1080 wireframe overflow too (see "Deviations in T13.c, as built").
  - **The DOM list (confirmed in decision-r07-t19-layout, item 4).** An instrument's list is the
    side column's while `CONTROLS`
    points at it (decision-r07-t19, 2d), and focus alone does not move `CONTROLS`, so a focused
    instrument canvas shows the primary's list until a key or a press on it; whether that meets the
    brainstorm's "each view paired with its DOM list" is the owner's.
- **`VIEW`'s layout, ruled (decision-r07-t19-layout).** T19.b builds the full and compact layouts,
  the label breaks, the slots' fixed corners, the list's `RANGE FROM CAMERA` head and the label
  block's additions; T19.c builds one frame path, with no histogram on an instrument. Stated
  limits: in compact, the controls of a folded panel are one action away, its settings on the
  label block; below a 1280×720 CSS-px window the compact column may clip at its foot; at 1280×720
  the label lines also break at spaces, and two slots fit only while each holds at most one
  statement, so that a line appearing in an open slot without room (the moment of
  `LIGHTING: PENDING`) sets `INSTRUMENT 2` under `INSTRUMENT 1`, its foot past the stage's, until
  it clears.
- **Deviations in T19.b, as built** (2026-10-05).
  - **Files.** Beyond the task's list: `displays/view/viewLayout.ts` (`ViewLayout`, `viewLayout`,
    `FULL_MIN_WIDTH_REM` 98.5, `FULL_MIN_HEIGHT_REM` 55.25, `FoldPanel`, `DEFAULT_FOLD`,
    `toggledFold`, `FOLD_BUTTONS`, `SideFolds`); `InstrumentsPanel.tsx` (optional `id`, `fold` and
    `toggleRef`, its rows in a body under the `Instruments` disclosure); `CameraControls`,
    `StyleControl`, `ExposurePanel` and `MeterControl` (optional `id` and `hidden`; `NO_OWN_SHIP`
    and `AUTO_NOT_AVAILABLE` exported for the standing lines); `InstrumentControls` (`folds`);
    `useInstruments.ts` (each slot's `panelRef` and `panelSize`); `viewRun.ts` (`cameraReading`,
    `EASED_MOVES_STATEMENT`, `rangesFromCamera`, a `not_free` refusal on `CommandResult`);
    `ViewMarkList`'s required `fromCamera`, which `view/spike/DescentSpike.tsx` passes too; and
    `test/viewDisplayHarness.tsx` (`FULL_VIEW_PX`, `COMPACT_VIEW_PX`, `stubViewLayout`,
    `resizeView`, `VIEW` inside a `.console__work--view`).
  - **The full layout's least height is 55.25 rem**, not the ruling's estimated 52: column A's
    tallest state measures 882 px at 1920 × 1080 (`Instruments` 204 with both closed, `Targets` 174
    at two rows under the two-line `RANGE FROM CAMERA` head, `Camera` 339 with `NO OWN SHIP`, both
    limit reasons and reduced motion, `Style` 99 plus a two-line refusal such as the software
    adapter's, 141, and three gaps). 1920 × 1080 at 100% (a 57.5 rem box) is full. **Ruled
    (decision-r07-t19b-exposure-fit, item 2; T19.b's follow-up):** not accepted. `Style` heads
    column B and stands in every engine state, which leaves column A's tallest at 733 px
    (45.8 rem). The least height is measured anew, never above 52.5 rem, so that a maximised
    1920 × 1080 window is full. The layout is full before the box is measured, as the galaxy
    page's is.
  - **Room for the designations.** VIEW's side panels take 0.75 rem inline padding (1.25 rem
    before) and the list's columns stand 0.25 rem apart, so that `TEST PLANET 150°` (145 px at the
    list's 1 rem) keeps its width beside `KIND` at 12ch and `RANGE` at 9ch. With no own ship
    `FROM CAMERA` is the head's second line, under `RANGE` and running back under `KIND`'s empty
    head, so the head grows by a line (19 px). The compact list's floor is 4.5 rem.
  - **The disclosure row** is set at the guide's least letter spacing (0.1em), its chevrons
    0.25 rem from their names, its buttons 0.25 rem apart and without end padding, so that the four
    names stand on one line: 406 px of the 416 with `EXPOSURE METER` (at 0.15em and 0.75 rem it
    wrapped). Its buttons carry the ship's disclosure mark (`DisclosureGlyph`), the open one's
    pointing down; `CAMERA` pressed while open stays open. `EXPOSURE METER` stands only while its
    panel would, and `STYLE` now in every engine state (decision-r07-t19b-exposure-fit, item 2); a
    panel that goes gives its place to `CAMERA`, and its focus to its own button or, where that
    went too, to `CAMERA`'s.
  - **Focus.** The side column's focus events track the folding panel that holds the focus. It is
    forgotten when the focus leaves the column for another control, or for nothing by the
    operator's hand, so that a later switch to the compact layout opens `CAMERA`.
  - **Standing lines.** The style's stands only for the `CONTROLS` view's fault
    (`GRAPHICS STYLE REFUSED: …`); the software adapter's, `QUALITY LOW`'s and the graphics'
    condition's refusals fold with the panel, as the ruling's list has it.
  - **No scrolling.** `.console__work--view` and `.view__side` are `overflow: clip`, not
    `hidden`, so that no focused control can scroll them; the column's clip margin, 0.25 rem,
    keeps the focus rings at its edges.
  - **Breaks.** The unbreakable runs are `white-space: nowrap` spans, found by one pattern: `UT`
    with its first group, `V <m> mag EYE|CAM`, `EV100 <n>`, the clock `ddd/hh:mm:ss`, and a number
    with a unit of a fixed list (`km/s`, `m/s`, `kyr`, `Myr`, `Gyr`, `yr`, `mag`, `AU`, `Gm`, `Mm`,
    `km`, `m`, `ly`, `s`); a unit outside it is not held. Every reading is now set as inline-block
    parts at the line's top, so that a part broken within itself keeps its label beside its first
    line.
  - **An instrument's statements.** All of them, `POSITIONS AS SEEN FROM SHIP` and
    `PHOTOREALISTIC: PREPARING` too, run under its label block and canvas, outside `.view-label`
    but in the canvas's description. `PHOTOREALISTIC: PREPARING` is the view's own and is never
    dropped for the primary's. Ruled (decision-r07-t19b-exposure-fit, item 3): the guide's Views
    bullet now sets them as the label block's, under its lines and the canvas across the slot, a
    draft for the owner.
  - **The label block's additions.** `METER` stands after `EXPOSURE`; `EASED CAMERA MOVES` after
    `ROTATION NOT YET MODELLED`, before the photorealistic statements. The `METER` and `AVG` rows
    are drafted by T19.b's follow-up (decision-r07-t19b-exposure-fit, item 4), ahead of T16. The
    rate keys outside `FREE` are refused
    without a word; the camera panel's rate reasons still name the step limits (the UX review's
    suggestion of a reason for the preset is left: the ruling keeps the panel's `RATE` as built).
  - **Tests.** jsdom lays nothing out: the slots' corners, the breaks, the fit, the clipping and
    the work area's `scrollTop` are the captures'; the Vitest check of `scrollTop` guards only
    against code that scrolls. The anchor test checks the slot's class and the reading order;
    full's focus order is checked by document position and the shared tab order by Tab.
  - **By hand, measured** (hidden: the app's window made offscreen by a capture hook, as its own
    hidden spike and smoke runs are, never shown; `.git/rm23-scratch/r07-views/shots-t19b/`, run
    2026-10-05 on the RTX 3080). Pages of 1920 × 1078, 1280 × 718, 1536 × 862 (1920 × 1080 at
    125%) and 1279 × 719 (150%) CSS px; at each, both instruments over a kept scene and over
    `PHASE TEST` all photorealistic, compact with each of its five panels open, and the camera's
    worst (no own ship, `FOV` at its narrowest, the rate at its lowest, reduced motion, both
    instruments closed). In every state the column's `scrollHeight` equals its `clientHeight` and
    no control stands below its foot; no text is truncated (`DESIG` included); no slot overlaps a
    reading or the other; `INSTRUMENT 2` stands at the stage's foot; Tab through every control
    leaves the work area's and the column's `scrollTop` at 0. At 1920 × 1080 no value line breaks
    but at `·`; at 1280 × 720 and 150% with both slots open the primary's `FRAME`, `TIME` and
    `EXPOSURE` break at their spaces as ruled (`UT +0 yr` | `000/00:00:08`, `EV100 -1.0` | `MAN`).
    The camera's worst at 1280 × 720 leaves `Targets` 139 px, its one-row floor and a few px.
    T13.c's camera setting and T13.d's `MAN` field and `METERED` line (merged before the captures)
    fit in full, where column B holds `Exposure` at its tallest, 400 px, and the columns keep 416
    and 320 px under `INHIBITED · NO IMAGE TO METER`; and in compact at 125%.
  - **Ruled (decision-r07-t19b-exposure-fit, items 1 and 5; T19.b's follow-up): T13.d's panel in
    the compact layout.** The exposure panel stands
    352 px wireframe at `INHIBITED · NO IMAGE TO METER` and 374 px with a refused entry's line. With
    `EXPOSURE` open, measured: at 1280 × 720 with an own ship (`PRECISION TEST`) the first fits and
    the second runs 17 px past the column's foot; with no own ship (`PHASE TEST`, which adds the
    `NO OWN SHIP` standing line and the two-line list head) 40 and 62 px; at 150% 0 and 0 with an
    own ship, 21 and 42 px without. Where it runs past, `INHIBIT` (and at 62 px `ENABLE`) stands
    below the clip. The ruling budgeted the panel at about 290–300 px. Ruled:

    - The camera setting stands in two rows of two members (44 px).
    - The compact gaps are 0.25 rem (16 px with a standing line).
    - The `MAN` field's consequence is reworded to one line (18 px).
    - The side column has its own `line-height: 1.25`, so that the heights hold on every platform
      and scale.

    That leaves about 16 px spare in the worst state, with at least 8 px required (a 0.5 rem probe
    after the open panel).

    - `ENABLE` and `INHIBIT` side by side is rejected: their notes would leave their buttons and,
      under `AUTO` or a system inhibit, cost 16 px.
    - The notes beside them lose the reason's bottom margin, so that they centre on the label, and
      in column B the rows stand 0.75 rem apart.
    - The reading kept in parts saves nothing in either column, and is built for its breaks.
    - The reading stays 1rem: it is not one of `VIEW`'s primary readouts.
    - Widening the column is impossible: the stage would fall under the 50.5 rem two slots need.

    `INHIBIT`'s consequence (decision-r07-t13d) stands on its row in one line here, 250 px in 301.
    The guide's Views bullet, the commanding bullet and the `MAN` (field) row are reworded, and the
    `METER` and `AVG` rows added. All are drafts for the owner: the bullets carry no tag, by the
    guide's convention, and are recorded here. As built, see "Deviations in T19.b's follow-up, as
    built".
- **Deviations in T19.b's follow-up, as built** (2026-10-05; decision-r07-t19b-exposure-fit,
  items 1–5, with decision-r07-owner-ux-signoff's two amendments to its guide text).
  - **Files.** Beyond the ruling's list: `styleRefusals.ts` (`NO_VIEW_DRAWN`,
    `NO_VIEW_REFUSALS`) and `viewLayout.ts`'s `FULL_MIN_HEIGHT_BOUND_REM`. `SideFolds` loses
    `styleId` and `styleHidden`, and `InstrumentControls` loses `refusals` and `faulted` with its
    style panel, which `ViewDisplay` sets at the head of column B for the `CONTROLS` view.
  - **No view drawn.** While no view can be drawn the primary's style refusals are
    `NO_VIEW_REFUSALS` for every input, the style panel and the key `4` alike. The ruling took the
    key as refused there as built. It was not where the engine refused the stage's canvas under a
    nominal adapter: `4` switched the style with nothing drawn. A regression test covers it.
  - **The least height is 45.75 rem** (732 px), against the ruling's about 46 rem. Column A's
    tallest state is 729 px: `Instruments` 202.5 px with both closed, `Targets` 173 px at two rows
    under its two-line `RANGE FROM CAMERA` head, `Camera` 337.5 px at the camera's worst, and two
    gaps. Column B's is 728.5 px. That is `Style` 99.5 px plus 43 px for a two-line refusal this
    machine cannot raise (`QUALITY LOW`'s on an instrument under `CONTROLS`, or the software
    adapter's; each measured at two lines in the panel as laid out), the exposure at
    `INHIBITED · OPERATOR` with a refused entry, 362 px, the meter with `METERED`, 208 px, and two
    gaps. The parts are in `FULL_MIN_HEIGHT_REM`'s TSDoc. The 1600 × 900 opening window stays
    compact by its width (98 rem of 98.5).
  - **For T19.d, the next task to add to column B.** Its held-back `INHIBIT` note under
    `INHIBITED · OPERATOR` (at most about 20 px, decision-r07-owner-ux-signoff) falls in column B's
    tallest state, which then passes 732 px. T19.d re-measures and raises `FULL_MIN_HEIGHT_REM`,
    within 52.5 rem. Re-measured by T19.d: the note adds 3 px, and the least height stays
    45.75 rem (see "Deviations in T19.d, as built").
  - **Targets' floor in the full layout.** `.view-targets`' 10 rem floor is under two rows when
    the head takes two lines (173 px). From the least height up, the list keeps two rows by
    construction: at the least height, `Targets` stands at 176 px in the camera's worst. Below it
    the layout is compact. Left as built.
  - **The harness** (hidden, not committed; `.git/rm23-scratch/r07-views/shots-t19b/`, with
    `summarise.py`). `run.sh` takes `--slice=agents.slice`, `--disable-vulkan-surface` and
    `--force-device-scale-factor=1`, and the hook refuses a relaunch.
    - Each hidden-window resize restarted the GPU process under the Vulkan surface ("Hidden-window
      resizes restart the GPU process"). Three restarts made the app's crash loop, which
      relaunched it into safe mode outside the run.
    - The session's display scale, 0.78125, made the window's content size jitter by a few px on
      each resize. At a forced scale of 1 the page takes the size asked for exactly: 1280 × 720,
      a 562 px box, where T19.b's page was 1280 × 718, a 560 px box.
  - **By hand, measured** (hidden, offscreen windows, never shown; 2026-10-05, the RTX 3080 under
    load).
    - **The runs:**
      - `compact720`, 1280 × 720, a 562 px box;
      - `zoom150`, 1920 × 1080 at 150% (1280 × 720 CSS px), a 565 px box;
      - `zoom125`, 1920 × 1080 at 125% (1536 × 864 CSS px), a 708 px box;
      - `ownship720` and `ownship150`, E5 and E6 in `PRECISION TEST`;
      - `full`, 1920 × 1080, a 922 px box;
      - `max1080`, a 1920 × 1014 page and an 856 px box (53.5 rem);
      - `atfull`, a 732 px box, the least height;
      - `belowfull`, a 731 px box, which is compact.
    - **Every state of every run passes:**
      - the column's `scrollHeight` equals its `clientHeight`;
      - nothing passes its foot, no control stands below it, and `ENABLE` and `INHIBIT` are shown;
      - nothing is truncated, with `.visually-hidden` text left out;
      - the work area's and the column's `scrollTop` stay 0 after each Tab sweep;
      - the 0.5 rem probe passes in compact, and at the 53.5 rem box in column B and in `Camera`.
    - **Compact, `EXPOSURE` open: the panel, `Targets` and the spare**, in px. The spare is the
      largest probe, raised in 1 px steps, with which the column's `scrollHeight` still equals its
      `clientHeight`.

      | State | 1280 × 720 | 150% | 125% | Own ship, 720p | Own ship, 150% |
      | --- | --- | --- | --- | --- | --- |
      | E1 `AUTO` | 244 / 218.5 / 85 | 243.3 / 222.5 / 90 | 243.6 / 365.7 / 233 | — | — |
      | E2 `AUTO`, refused | 265.5 / 197 / 64 | 264.8 / 201 / 69 | 265.1 / 344.2 / 211 | — | — |
      | E3 `INHIBITED · OPERATOR`, refused | 265.5 / 197 / 64 | 264.8 / 201 / 69 | 265.1 / 344.2 / 211 | — | — |
      | E4 `MAN`, refused | 244 / 218.5 / 85 | 243.3 / 222.5 / 90 | 243.6 / 365.7 / 233 | — | — |
      | E5 trapped | 289.5 / 173 / 40 | 288.8 / 177 / 45 | 289.1 / 320.2 / 187 | 289.5 / 194.5 / 81 | 288.8 / 198.5 / 86 |
      | E6 trapped, refused | 311 / 151.5 / 18 | 310.3 / 155.5 / 23 | 310.6 / 298.7 / 166 | 311 / 173 / 60 | 310.3 / 177 / 65 |

      E6 is the worst at every size. At T19.b's 560 px box its 18 px is 16 px, as the ruling
      reckoned, against the 8 px required, and its panel stands at 311 px, the ruling's figure. Of
      T19.b's own states the least spare is the camera's worst: 21 px at 720p, 26 px at 150% and
      169 px at 125%.
    - **Compact, measured in E1–E6:**
      - the `MAN` field's consequence is one 17.5 px line;
      - `INHIBIT`'s row is 32 px with its statement on it;
      - the setting stands in two rows;
      - each `dd` stands 12 px after its own `dt`;
      - each row's second label stands 60.4 px and 123.4 px after the output before it (the `dd`'s
        9ch);
      - with an own ship, `ISO 409,600 ↑` is pegged, its `↑` drawn and nothing truncated;
      - the trapped reading breaks `EV100 11.7 INHIBITED ·` | `NO IMAGE TO METER`;
      - each note centres on its button's label, 0 px off.
    - **Full:**
      - column B is headed by `Style PRIMARY`, and by `Style INSTRUMENT 1` under
        `CONTROLS INSTRUMENT 1`;
      - in column B the exposure panel stands at 326.5, 348, 362, 306, 386.5 and 408 px in E1–E6,
        and the column at 612.5, 634, 685.5, 609.5, 494 and 515.5 px. Its tallest measured state
        is E3, 685.5 px; the ruling reckoned about 680 px beside a drawn image, taking the refused
        panel at about 320;
      - under `AUTO` (E1) both notes wrap in column B, in 35 px rows 12 px (0.75 rem) apart;
      - at the 53.5 rem box the spare is 140 px in column A (the camera's worst) and 170 px in
        column B (E3);
      - at the least height every state passes with no probe, with 16 px to column A's clip and 46
        px in column B.
  - **Left for T19.d, the next task to edit `ExposurePanel.tsx` (the UX review's consider).** The
    standing line `AUTO NOT AVAILABLE: NO IMAGE TO METER` still breaks inside its status in column
    B (`… NO IMAGE TO` | `METER`), where the reading above now keeps it whole. T19.d sets
    `INHIBITED · OPERATOR` unbroken in its new note, and can hold `NO IMAGE TO METER` the same way,
    at no height. Built by T19.d.
  - **Tests.**
    - `viewLayout.test.ts`: the threshold both ways; a maximised 1920 × 1080 box is full, and the
      1600 × 900 opening window compact; the least height within its 52.5 rem bound.
    - `ViewDisplayLayout.test.tsx`: the six panels in their order, `Style` in column B; column B
      headed by the `CONTROLS` view's style, for `PRIMARY` and an instrument; `STYLE` offered in
      compact while no view can be drawn; and no style fault shown once the graphics are ruled out
      after one.
    - `ExposurePanel.test.tsx`: the consequence's words; each label wrapped with its own value, in
      order; the trapped reading in two parts, the text unchanged.
    - `MeterControl.test.tsx`: the reading in parts under `INHIBITED · OPERATOR`; `NO IMAGE TO
      METER`, not a reading, while nothing is metered.
    - `ViewDisplay.test.tsx`: `Style` standing while the engine is made and where the views cannot
      be made, both buttons held back by `NOT AVAILABLE: no view is drawn`; the key `4` refused
      there (a regression test, which failed before); no panel moving when the engine is made.
  - **Guide** (`docs(guide)`, for the owner). The Views bullet sets an instrument's lines beside
    its canvas and its statements under the lines and the canvas across the slot, and adds `METER`
    to the primary's lines (decision-r07-owner-ux-signoff). The commanding bullet and the `MAN`
    (field) row read `An entry sets MAN: AUTO resumes only on ENABLE`, and the row's tag adds
    R07.T19.b. The `METER` row, with the sign-off's `EXPOSURE METER` panel name, and the `AVG` row
    follow `METERED`. Prettier re-padded the nomenclature table; that change is whitespace only.
- **The owner's sign-off (decision-r07-owner-ux-signoff, 2026-10-05, under the owner's
  delegation; built by T19.d).** The guide's drafts from R02, R05 and R07 are signed off, 15 rows
  with amendments, and `ELV`'s row is widened for `CAMERA ELV`. The exposure reading is not a
  primary readout and stays 1rem, and `G` 124 now defines one as a display's headline value.
  `ENABLE` stands, its verb "sets". `NO IMAGE TO METER` keeps its four words, as signed off.
  **Pending, for the owner's look:** the instrument slots (the `INSTRUMENT 1`, `INSTRUMENT 2`
  row, and the Views bullet's first sentence and its statements clause). The stated limits: at
  1280 × 720 two slots cover most of the primary's image; they fit there only while each holds
  one statement, beyond which the second `OPEN` is held back; and a statement appearing without
  room sets `INSTRUMENT 2` under `INSTRUMENT 1`. The ruling gives the steps.
- **Deviations in T19.d, as built** (2026-10-06; decision-r07-owner-ux-signoff).
  - **Files.** Within the ruling's list, `exposure.ts`'s `enable` TSDoc takes item 3's verb,
    "sets". Beyond it:
    - `ViewLabelBlock.tsx`: the reading's private `partRuns` becomes the exported
      `unbrokenRuns(text, runs)`, which `ExposurePanel.tsx`'s `exposureNote` also uses;
    - `ViewDisplay.tsx`: the compact standing line `AUTO NOT AVAILABLE: …` goes through
      `exposureNote` too;
    - `styles.css`: a command row's button keeps its width (below), and the run class's comment;
    - `viewLayout.ts`: `FULL_MIN_HEIGHT_REM`'s TSDoc parts;
    - `ViewDisplay.test.tsx` and `ViewDisplayLayout.test.tsx`: a line holding a phrase in a run of
      its own is found by its whole text;
    - `styleRefusals.ts`: `PHOTOREAL_NOT_CREATED`'s TSDoc no longer calls it drafted for the owner.
  - **Unbroken phrases.** Spans, not U+00A0: `exposureNote` sets `INHIBITED · OPERATOR` and
    `NO IMAGE TO METER` each in a `.view-label__run` (`white-space: nowrap`), so the text, the
    accessible descriptions and the captures' strings are unchanged. It covers every note of the
    exposure: the reasons and the statement beside `ENABLE` and `INHIBIT`, and the standing line in
    the panel and under the compact row. In column B the standing line now breaks
    `AUTO NOT AVAILABLE:` | `NO IMAGE TO METER`, at no height (T19.b's follow-up's UX consider).
  - **A command row's button keeps its width** (found by the captures). `.control`'s 2 rem least
    width let a button shrink beside a note wider than the room: beside the new note `INHIBIT`
    fell to about 55 px under its 81 px label, and in column B, as built by T19.b's follow-up,
    `ENABLE` under `AUTO` fell to 70 px, its label overflowing and its note standing 10 px left of
    `INHIBIT`'s. `.view-exposure__command > .control` now sets `flex: none`. No height changes.
    jsdom lays nothing out, so the captures carry the check (each button's width and overflow).
  - **`LightingState` keeps `"hosts-not-received"`.** The ruling's optional rename is not taken:
    the value is internal, and `view/lighting/hostLights.ts` is the shading lane's, so the change
    stays one string and a TSDoc line.
  - **The least height stays 45.75 rem.** The note adds 3 px, not the ruling's "at most about
    20 px" (its three-line case): beside the button there is 204.9 px, so
    `NOT AVAILABLE: the exposure` fits and the note takes two lines, a 35 px row, in column B as in
    compact. Column B's tallest is 731.5 px: `Style` 99.5 px plus the 43 px two-line refusal this
    machine cannot raise, the exposure at `INHIBITED · OPERATOR` with a refused entry 365 px, the
    meter with `METERED` 208 px, and two gaps. Column A's is 729 px. 731.5 px is 45.72 rem, which
    rounds up to 45.75 rem, so `FULL_MIN_HEIGHT_REM` is unchanged and its TSDoc gives the new
    parts. At the least height column B keeps 43 px in E3, the unraisable refusal's height: 0.5 px
    to spare, by construction. The other candidate is a drawn image whose meter weighs nothing at
    the operator's inhibit with a refused entry (E8 below): column B is 674 px there, `Style`
    99.5 px, the exposure 408 px with its standing line and both notes, the meter 150.5 px with
    `NO IMAGE TO METER` and no reading, and two gaps, so 717 px with the refusal. E3 stays the
    tallest.
  - **The harness** (hidden, not committed; `.git/rm23-scratch/r07-views/shots-t19d/`, from
    T19.b's follow-up's, with `table.py`).
    - `:1` is gone since the owner's re-login, and `:0` is the session. The orchestrator approved
      the offscreen proxy under `DISPLAY=:0`: no native window at all, `--disable-vulkan-surface`,
      `--force-device-scale-factor=1`, the relaunch refused, in a capped scope with
      `TasksMax=4096`.
    - Each window is made at its final size and never resized; the page and the view box are only
      checked, and every run's were exact. The least height's page is the box plus the 158 px of
      chrome.
    - `trappedExposure` gains E7: after E6, `INHIBIT`, then `99`. The own-ship runs gain E3 before
      E5, and the full runs gain E8 after E7: `PRECISION TEST` drawn photorealistic under
      `METER LIT`, whose bodies are black with no star disc, so that the meter weighs nothing,
      with `99` refused. The measures gain each note's runs with their line boxes, the standing
      line, and each button's width and overflow.
  - **By hand, measured** (hidden, never shown; 2026-10-06, the RTX 3080).
    - **The runs:** `full`, `max1080` (856 px box), `atfull` (732 px), `belowfull` (731 px,
      compact), `compact720` (562 px), `zoom150` (565 px), `zoom125` (708 px), `ownship720` and
      `ownship150`. All 123 states pass every check of decision-r07-t19b-exposure-fit item 1,
      the 0.5 rem probe included, and no relaunch, GPU-process restart or failure was logged.
    - **Compact, `EXPOSURE` open: the panel, `Targets` and the spare**, in px:

      | State | 1280 × 720 | 150% | 125% | Own ship, 720p | Own ship, 150% |
      | --- | --- | --- | --- | --- | --- |
      | E3 `INHIBITED · OPERATOR`, refused | 268.5 / 194 / 61 | 267.8 / 198 / 66 | 268.1 / 341.2 / 208 | 268.5 / 215.5 / 102 | 267.8 / 219.5 / 107 |
      | E7 trapped, `INHIBIT`, refused | 294 / 168.5 / 35 | 293.3 / 172.5 / 40 | 293.6 / 315.7 / 183 | 294 / 190 / 77 | 293.3 / 194 / 82 |

      E1, E2 and E4–E6 are as T19.b's follow-up measured them. E6 is still the worst, at 18 px
      (720p), 23 px (150%) and 166 px (125%), against the 8 px required.
    - **`INHIBIT`'s row** under `INHIBITED · OPERATOR` is 35 px (at most 36), its note two lines:
      `NOT AVAILABLE: the exposure is` | `INHIBITED · OPERATOR` in compact, and
      `NOT AVAILABLE: the exposure` | `is INHIBITED · OPERATOR` in column B. The phrase stands on
      one line box in every capture, so the note never breaks at the `·`. In E7 `ENABLE` is held
      back by `NO IMAGE TO METER` and `INHIBIT` by its note: the view offers `MAN` only.
    - **Full, column B:** the exposure panel in E1–E8 is 326.5, 348, 365, 306, 386.5, 408, 408 and
      408 px, and the column 612.5, 634, 688.5, 609.5, 494, 515.5, 515.5 and 674 px. In E8 the
      label block reads `ROTATION: NOT YET MODELLED` and `LIGHTING: NOT RECEIVED`. The least spare
      at the 53.5 rem box is 167 px in column B (E3) and 140 px in `Camera`; at the least height,
      43 px in column B (E3) and 16 px in column A.
  - **Tests.**
    - `exposure.test.ts`: `INHIBIT` refused with `already_inhibited` under the operator's inhibit,
      and still taken under `AUTO` and `INHIBITED · NO IMAGE TO METER`.
    - `ExposurePanel.test.tsx`: under `INHIBITED · OPERATOR`, `INHIBIT` held back and described by
      its reason in its consequence's place, and a press changing nothing; `ENABLE` offered beside
      a metered value and held back by `NO IMAGE TO METER` without one; the level phrase in one
      run; `NO IMAGE TO METER` in one run in the standing line and beside `ENABLE`. The test that
      said `INHIBIT` states nothing there is replaced.
    - The renamed strings, where the old ones were asserted.
    - `ViewDisplayLayout.test.tsx`: the compact layout's folded-exposure status under the row
      holds `NO IMAGE TO METER` in one run (it fails with the line set as plain text).
    - `rg -F` over `apps/` and `docs/frontend/` finds none of the four old strings, and finds
      `owner signs off` on the `INSTRUMENT 1`, `INSTRUMENT 2` row alone.
  - **Gate.** The acceptance command (453 tests), the app's vitest (319 files, 5,281 tests),
    `just check lint` from a clean tsc cache, Prettier and the console-ux skill's lint (0 errors,
    the 11 old checks), contrast and glyph scripts. No `glyphs.py --ranges`, since no U+00A0 is
    used. No `just test-render`: the smoke draws none of the changed statuses or rules, and no
    shader, `view/engine/` or `src/smoke/` file changed. No `just ci` (the Day 2 protocol).
  - **Reviewed** by the TypeScript, UX and plan-conformance reviewers, with no must-fix. Fixed: a
    test of the compact status line's run, the `ENABLE` test split in two, own-ship E3 and E8
    captured, T16's re-wrapped lines, and the stale remark in `styleRefusals.ts`.
  - **Open, for the orchestrator (the UX review's considers).**
    - The data-state bullet's "A view with nothing to meter offers `MAN` only" reads, taken
      literally, against E5, where `INHIBIT` stays offered to take a system inhibit over (the
      commanding bullet; the ruling, item 1, reason 3). A wording point for the owner's next guide
      edit, such as "offers no level but `MAN`"; the signed-off text is left as ruled.
    - In the B612 Mono readings the gap after a part's `·` looks wider than the one before it
      (`EV100 11.7 INHIBITED ·  OPERATOR`, compact E7). It predates this task (`readingParts`,
      T19.b); most likely the font's middle dot sits left of its cell's centre. Not measured.
- **Deviations in T19.c, as built** (2026-10-05).
  - **Files.** `viewFrameDrawer.ts`: `ViewFrameInputs.meter` (`MeterMode | null`),
    `ViewFrameDrawer.takeHistogram` and `makeViewFrameDrawer`, which makes a view's drawer at once
    or, where the engine has no device, at its restore. `ViewDisplay.tsx`: the primary's loop
    draws through a `ViewFrameDrawer` (`placeMarkLabels` moves the marks' labels, as before).
    Beyond the task's list: `useInstruments.ts` (a slot's drawer through `makeViewFrameDrawer`,
    `meter: null`); `view/photoreal/renderer.ts` (`PhotorealFrame.meter` is `MeterMode | null`, and
    a frame with `null` dispatches no histogram) and `photorealFrame.ts` (its input); and
    `test/viewDisplayHarness.tsx` (`Submission`, `submittedBy`, `TimedEngineSource.submissions`:
    every canvas pass, target pass and dispatch, a dispatch numbered as no resolve).
  - **What stays the primary's.** Its loop keeps what only the primary does: the pacing, the
    `PrimaryFrameTimes` grouping, the `BudgetedScale` controller, its `AutoExposure` (it takes the
    drawer's histogram, and the drawer draws at its applied control, `reading.control`), the
    return to the wireframe when its pipelines fail, the DOM mark labels, the 4 Hz publish and the
    instruments' frame. The drawer draws the budget's `style` for every view, as T19 says each view
    does: the primary's with the merged availability (the adapter's, then the budget's), an
    instrument's with its adapter's. An instrument still draws at the display's control, which
    under `AUTO` follows the primary's applied value at 4 Hz and 0.1 EV.
  - **No histogram on an instrument** per frame: its frames carry `meter: null`, so its renderer
    dispatches none and reads none back. Design note 8's histogram is the exposure source's pass
    alone: an instrument's photorealistic passes are the sky, the discs, bloom, the tone mapping and
    the symbology. The renderer still makes its `HistogramReader` (three 1 KiB buffers) and
    compiles the kernel while it is made, unused on an instrument; not making them would need a
    renderer option, left since their cost is memory alone.
  - **The primary during a device loss.** A stage mounted while the device is lost (a new scene)
    made its renderers in the effect, which threw `EngineUnavailable`; through
    `makeViewFrameDrawer` it now waits and draws from the restore, as an instrument opened then
    does. The drawer is made in a microtask after the restore's dispatch, not inside it:
    `onRestored`'s listeners run from a live set, so a drawer made inside would have its renderers'
    new listeners told of the same restore, and each would make its handles a second time, dropping
    the first (an instrument's did so since T19; plan-conformance review). A first making that
    fails but for a loss disposes of the view before it throws, and a release before the restore
    disposes of the view alone.
  - **The rule** "until it lands, a task that changes what the primary's frame draws changes
    `ViewFrameDrawer` alike" is retired: a change to the frame is made once, in `ViewFrameDrawer`
    (or `photorealFrame` and R07's renderer), and reaches every view.
  - **Tests.** `renderer.test.ts`: a frame with no meter dispatches no histogram and hands none
    over. `viewFrameDrawer.test.ts` (new, against the counting engine): a drawer asked for during a
    loss is made once after the restore, making each handle once (made inside the dispatch, it made
    every buffer, material, mesh and texture twice), and one released before the restore is never
    made and its view disposed of. `InstrumentView.test.tsx` (against the timed fake engine): the
    primary and `INSTRUMENT 1` at `CHASE`, both photorealistic, on stages of one size, submit the
    same passes in the same order with the same draws' materials but the primary's histogram (one
    preset, field of view and pose; their roles, the primary's `eye` and an instrument's `camera`,
    differ only in the passes' uniforms, the bloom kernel's weights and the glare terms); a
    photorealistic instrument dispatches no histogram; a stage mounted during a loss draws once
    restored, and one that goes before the restore disposes of its view. Each new test fails on the
    code before T19.c (8433e20, the merge of 49f029d). T8.a's metering tests pass unchanged.
  - **Cost per instrument frame, measured** (provisional: hidden, the app's window made offscreen by
    a capture hook, never shown; on the RTX 3080 under shared load, load average 9–16; Chromium's
    `--disable-dawn-features=timestamp_quantization`; a 1920 × 1078 page, `PHASE TEST`, all three
    views photorealistic in `FREE`, the primary's target 1120 × 898 px and each instrument's
    240 × 180; six alternating 8 s phases of one build, each instrument's meter `"average"`
    (before) or `null` (after), through temporary counters that were never committed; each view's
    GPU time the sum of the resolves its `draw` numbered, its CPU time `performance.now()` about its
    `draw`, at 0.1 ms resolution, without the read-back's callback). Before: GPU 1.870–1.873 ms per
    instrument frame, its histogram 0.0106–0.0108 ms; CPU 1.18–1.33 ms. After: GPU 1.866–1.886 ms,
    no histogram; CPU 1.10–1.33 ms. The histogram's 0.011 ms lies under the phases' spread, and the
    CPU's 0.05 ms or so under the load's. Of an instrument's 1.87 ms, the `discs` pass takes
    1.74 ms (the primary's 2.44 ms over 23 times the pixels): a cost per draw, not per pixel, of
    `PHASE TEST`'s bodies, for T20 and the shading lane. The primary, whose code path changed but
    not its passes: GPU 2.77–2.81 ms per frame (its histogram 0.024 ms) against 2.66–2.80 ms for
    the inline loop before, measured the same way; CPU 7.8–8.3 ms per draw against 7.1–10.9 ms,
    within the load's spread. T20 measures on a quiet machine. _Explained in "The photorealistic
    view's per-draw cost, investigated": a small disc's sampling, about 2 million cycles a view._
  - **Gate.** No `just ci` (the Day 2 protocol). `just test-render`, since R07's renderer changed.
- **Deviations in the T20 and T21 harnesses, as built** (2026-10-05; the harnesses only, the runs
  being the owner's).
  - **Files.** T20, a check launch of the client, on R05's descent-spike pattern: `--views-check`
    with the spike's `--setting`, `--smoke` and `--out` (`main/cli.ts`); the switch and the narrow
    API (`preload/viewsCheckLaunch.ts`, `preload/viewsCheckApi.ts`, the types in `preload/api.ts`);
    the main process's handlers, a trace window a phase, its scan and the session
    (`main/viewsCheck.ts`), the record's check (`main/viewsCheckRecord.ts`), and the results,
    their summary and their writer (`main/viewsCheckResults.ts`), reusing R05's `describeMachine`,
    `frameStats`, the frame rows (`frameRows`, `row`, `ofPeriod`, `overallOf`, exported from
    `results.ts` for it), its trace reducer and its Prettier estimate; in the renderer,
    `displays/view/check/`: `probedEngine.ts` (an engine that tells a probe which view's or
    target's `render` took each resolve), `viewsProbe.ts` (frames keyed by their animation frame's
    timestamp, through a wrapper of the page's `requestAnimationFrame`), `viewsCheckRun.ts` (the
    script, driving `VIEW` by its controls, and its ending) and `ViewsCheckRunner.tsx`, which
    `App.tsx` mounts on a check launch, opening on `VIEW` with the probe's engine source and the
    launch's setting. `VIEW` itself is unchanged. `scripts/viewsCheck.sh` and `just views-check`.
    T21, in the smoke harness: `--smoke-child` (`smoke/main.ts`, `smoke/childWindow.ts`; the scene
    `renderer/src/smoke/childWindow.ts`), `scripts/childWindowCheck.sh` and
    `just child-window-check`. Results go to `docs/measurements/several-views/` (its README holds
    the owner's checklist); each run's verdicts are entered as one line in T20's or T21's notes
    when the owner's records land, where the README's convention puts a plan's runs in its as-built
    notes.
  - **The primary fills `VIEW`'s stage, not the window.** `VIEW` as ruled (decision-r07-t19-layout)
    gives the primary the stage beside its side columns: 1126 × 906 px of a 1920 × 1080 window in
    the hidden smoke (about half a full window's pixels) and 822 × 544 px of 1280 × 720 in the
    compact layout. T20's "full-window photorealistic view" (and the brainstorm's and R12.T5.a's
    full-window canvas) is read as the primary at `VIEW`'s stage; the record gives every canvas's
    size, and the 33 ms comparison on the UHD 620 is the lighter for it (ruled 2026-10-05: the
    stage, as built; see "Ruled" below).
  - **The window is sized in device pixels.** A check's window asks for 1920 × 1080 or 1280 × 720
    device pixels, its DIP size the pixels over the display's scale (2458 × 1382 DIP at both
    machines' 0.78125), so that its canvases are Design note 21's 1080p and 720p; it keeps the
    console's least size, not the spike's, since the 1080p window can outgrow a 1080p display's
    work area. The descent spike sizes its window in DIP (1500 × 844 px at 0.78125; for the
    orchestrator). A tiling window manager such as the development machine's i3 sizes it to its
    tile instead, so the record reads the content size at the end and gives it in DIP and pixels.
  - **Windows.** Each phase settles 5 s once its configuration is reached, starts its trace, waits
    a 1 s guard (R05's `TRACE_BOUNDARY_GUARD_S`, so that the start's own pause is left out), is
    measured over 20 s (2 s in a smoke), marked by a `performance.measure` span, and stops its
    trace at the window's end; the pass timer's last reads arrive in a grace after it. Every trace
    figure (presentations, drops, the GPU process's busy time and slices) is clipped to that
    window, the page's clock set against the trace's by R05's reducer's `clockOffsetUs`, so the
    probe's frames and the trace's are the same span. This stands for R05 Design note 21's 10 s
    warm-up over a whole descent. Design note 21's five frame rows and two headroom rows are judged
    (the main thread's from the page's animation-frame callbacks, a lower bound); terrain,
    atmosphere and memory are not read, since `VIEW` draws no terrain. A phase that ends not
    holding its configuration is `not-measured`.
  - **The trace's categories** are R05's five for timed runs (decision-r05-trace-windows-2, item
    2), without `gpu` and V8's profiler, recorded through Electron's `contentTracing` as JSON in
    short windows; R05 moves its spike to a Perfetto stream over CDP (T14.g, T14.h), which the
    check may follow later (for the orchestrator).
  - **The resize** narrows the primary's stage in the page (to 80% and 65% of its width, then back),
    not the window: an offscreen window's resize restarts the GPU process under the Vulkan surface
    (R01's Risks), and a window's resize moves `VIEW` across its layouts' thresholds, which would
    move the slots too. The check is every texture made or destroyed meanwhile, by the view its
    name belongs to (a name no view's begins, such as the pass timer's buffers, is the engine's),
    and every view's canvas at each step. Each step also records its longest frame on the main
    thread, where the photorealistic primary's renderer refits its bloom at the new size: the
    figure T19's "about 35 ms (provisional; T20 records it)" asks for.
  - **No GPU time in copies** is read as R01.T11 read it, no pass of the views' own named as a copy,
    and the GPU process's slices of the traced categories named `copy` or `blit` are counted a
    frame by phase beside it: a count that grows with the instruments would be a copy a canvas
    outside the views' passes.
  - **The per-canvas overhead: an upper bound; 0.3 ms kept until the shown runs (ruled).** The pass
    timer sees only the views' passes; presenting a canvas is Chromium's work. The record's figure
    is the GPU process's main-thread time a primary draw, with the two instruments open less
    without, per canvas, for the photorealistic and the wireframe primary (the larger kept), with
    each instrument's own pass time beside it. It is CPU time, not GPU time, and more than the
    presenting cost: it holds the decoding of the instruments' own commands as well; and it is per
    primary draw, where on the high setting the 30 Hz instruments draw in every second one.
    Taken whole as `PER_CANVAS_OVERHEAD_MS` it would count the instruments' cost twice, since the
    controller's measured GPU time already holds their passes. A pair whose phase did not hold its
    configuration gives none. The hidden smokes read 0.6–2.4 ms a canvas, above the provisional
    0.3 ms. The record carries the client's constant, so it reads against whatever the constant
    is.
  - **The right way up, asked last.** `PHASE TEST`'s bodies lie on the horizon line, where a still
    picture cannot show a view turned upside down, so the question comes after the measured
    phases, with the cockpit set up again, in a dialog that is not modal: the owner turns each
    view's camera, and as it turns up the bodies must move down, in every view. Each phase's last
    frame is saved as a PNG under `target/views-check/`, never committed.
  - **The low setting before T17.** `--setting low` gives `VIEW`'s budgets the low setting (one
    photorealistic view, a 30 Hz photorealistic primary) and the 1280 × 720 window, but the sky and
    the photorealistic frame draw at `high` until T17 (see "`VIEW`'s quality setting is `high`
    throughout"). A low-setting record says so (`LOW_BEFORE_T17` in `viewsCheckResults.ts`, which
    T17 removes) and is provisional.
  - **No server, and both shown runs the owner's.** The runs draw the kept `PHASE TEST` with no
    server, so the UHD 620's low cases carry none of the local server's load that decision-r07-t18
    item 4's single-player case assumes (ruled 2026-10-05: no server; see "Ruled" below). T20 puts
    the RTX 3080 run on the
    development machine; a shown run takes a display, which the lanes never do, and a hidden run
    has no presentation times, so the RTX 3080's run is the owner's too.
  - **T21's prototype, rebuilt.** R01.T13 ran a scratch prototype that was never committed and no
    longer exists, so the harness rebuilds it in the smoke harness, outside the client, whose every
    window stays denied: an opener whose full-window canvas stands for the main view and a child
    whose canvas stands for an instrument, each R01.T11's marker scene with a post-process, the
    child on the second display (the first that is not the primary), on the client's own graphics
    switches with the timing lift. The opener allows that one child once and locks it down (it
    opens nothing and navigates nowhere). R07's `InstrumentView` in a child window is not built.
    The child's pacing is read from each window's animation frames, as R01.T13's probe read it, not
    from presentations: a median interval within 3% (`PACING_TOLERANCE`) of its display's period
    is paced by that display, and two displays whose periods lie within that band of each other
    cannot be told apart, which the record says. The release passes where the child's `pagehide`
    came; the opener's frames between the close and the `pagehide`, each still drawing the closing
    child, are counted beside it, and the uncaptured-error count judges whether any reached a
    closed context (until 2026-10-06; the release is now at the close, see "Deviations in T21's
    release at the close"). The resize passes only where the child drew at a size other than the
    one before it. The record counts the GPU process's exits, since the shown run
    is this plan's on-screen check of a child window under the Vulkan surface ("Hidden-window
    resizes restart the GPU process"), and describes the machine as R05's records do.
  - **Smoke runs on this machine** (hidden, never shown, the RTX 3080 under load; not T20's or
    T21's results and not committed). `just views-check --smoke`, high and low (load average 4.3
    and 4.5, 2 s windows), each about 33 s: every phase held, on low too (the compact layout's
    folded `Instruments` panel, the 30 Hz photorealistic primary at 33.3 ms between draws, the one
    photorealistic view freed for instrument 1); the resize passed (the primary's canvas
    1126 → 901 → 732 → 1126 px on high, 822 → 658 → 534 → 822 px on low, 42 of its textures made
    and destroyed, none of the instruments', and the pass timer's growing buffers the engine's); no
    pass named a copy; no drop warning, no dropped resolve, no fault; the longest frame at each
    resize step 20–54 ms on the main thread (the bloom's refit, against T19's provisional about
    35 ms); the cockpit's GPU frame 3.80
    and 3.88 ms (median, 95th percentile) on high, 8.31 and 9.34 ms on low; the GPU process's
    time a canvas 0.95 ms (high) and 0.61–2.41 ms (low). A photorealistic instrument at 240 × 180
    px took 4.8–5.8 ms of GPU time a draw, the `discs` pass's cost per draw that T19.c found
    (1.74 ms then), grown: a finding for the shading lane (_the same cycles at a lower clock; see
    "The photorealistic view's per-draw cost, investigated"_). Earlier smokes, before the windows
    were clipped and with `gpu` traced, read the GPU process's time a canvas at 1.4–2.4 ms.
    `just child-window-check --hidden --seconds 6` (load average 4.3, with
    `--disable-vulkan-surface`), run three times: the child opened (0 × 0, offscreen, as R01.T13
    found); its view was dropped on its `pagehide`, before the opener's next frame in two of them and
    one frame after the close in the other, with no uncaptured GPU error in any; the opener drew
    about 120 frames in the 2 s after the close with no loss; and the GPU process never exited. A
    `pagehide` a frame late is a finding for the client's rule (R01.T13's assumed it comes before
    the opener's next frame): where the opener closes a child itself, it can drop the view at the
    close (done 2026-10-06; see "Deviations in T21's release at the close"); its resize is the
    shown run's, since an offscreen child has no size of its own. The one-display refusal, run
    under headless Ozone so that no display is touched, exits 2 before any window opens.
  - **The development machine's display.** Electron on this session's X display (`:1`) reports one
    display, a `VX2450 SERIES` at 60.00 Hz, 2458 × 1383 DIP at a scale of 0.78125 (1920 × 1080
    px), not the projector at 59.94 Hz on `:0` that the README describes (for the orchestrator).
    Each run reads T from the display it is shown on and records it.
  - **Ruled** (2026-10-05; adopted by the orchestrator under the owner's delegation, from the
    lane's leans):
    - `PER_CANVAS_OVERHEAD_MS` stays at 0.3 ms until the owner's shown runs land. Which figure
      replaces it then stays open for the owner: presentation alone or the instruments' whole cost
      in the GPU process, per primary frame or per instrument draw, one value for both settings or
      one each. The lane's lean: keep it where the shown runs' upper bounds stay near it on both
      machines, and otherwise make it a setting's field, the UHD 620's figure for low and the
      RTX 3080's for high;
    - T20's cockpit and low cases measure the primary at `VIEW`'s stage, as built (as `VIEW` is
      built and played), with the canvas sizes recorded, not at the window's size;
    - the UHD 620's low cases run without the local server: T20's cases are the views' own cost,
      and the server's share is R12's budget run;
    - the UHD 620 run waits for T17, since a low-setting record before it draws the sky and the
      frame at high (`LOW_BEFORE_T17`).
  - **Tests.** `cli.test.ts` (the flag and its options, refused with the spike's own and with
    `--descent-spike`), `viewsCheckLaunch.test.ts`, `viewsCheckApi.test.ts` (the channels equal
    the main process's), `viewsCheckRecord.test.ts`, `viewsCheckResults.test.ts` (each finding's
    verdicts, the low setting's 30 Hz T and 35 ms row, a hidden run's missing figures, the
    engine's allocations, the writer's names), `viewsCheck.test.ts` (the trace's categories, the
    window's clipping of the presentations, the drops and the GPU process's busy time and slices,
    the session and the handlers' refusals), `viewsProbe.test.ts`, `viewsCheckRun.test.tsx` (the
    script against `VIEW` and the timed fake engine: every phase in order, each window after the
    guard and handed on as marked, the low setting's one photorealistic view freed first, the
    resize, the question last, and nothing more asked once stopped), `viewsCheckEnding.test.ts`
    (the ending, and the engine's names for the slots), `ViewsCheckRunner.test.tsx` (unmounted,
    the page's animation frames given back and nothing ended), `App.test.tsx` (a check launch
    opens on `VIEW`, and fails a run whose `VIEW` cannot draw), and `childWindow.test.ts` on both
    sides (the display, the refusal, the window-open handler, the record, the pacing reading).
  - **Gate.** No `just ci` (the Day 2 protocol): `pnpm --filter hyperion test`, `just check lint`,
    and `just test-render`, since `src/smoke/` changed.
- **Deviations in T21's release at the close, as built** (2026-10-06; the views lane's follow-up
  to the T21 harness's finding).
  - **The rule, amended.** R01.T13's rule dropped a child's view on its `pagehide` alone, and
    assumed that came before the opener's next frame. The hidden smoke found the `pagehide` a frame
    late in one run of three. So the opener now drops the view at its own `close()` of the child,
    before the call. The `pagehide` stays the release of every other close. Whichever comes first
    drops the view, and the other does nothing. R01's T13 as-built notes hold the rule as it
    stands, and T21's text points to it.
  - **Where it lives.** `holdChildView(child, view)` in `renderer/src/smoke/childWindow.ts`, T21's
    scene, gives:
    - `view()`, the view, or `null` once it is dropped;
    - `releasedBy()`, `"close"`, `"pagehide"` or `null`;
    - `close()`, which drops the view, then closes the window.

    Dropping the view also removes the hold's `pagehide` listener, for a client that drops a view
    while its window stays open. The scene draws `view()` only while it is held. The client opens no window (`main/index.ts`
    denies every one) and no shared code held the rule, so no production path changed. The first
    client view in a child window takes `holdChildView` from there, moved where both can import
    it.
  - **The record's check** is now "T21 the child's view was dropped at its close", read by
    `closeRelease` in place of `pagehideRelease`. It passes where the opener's close dropped the
    view and a `pagehide` came after it, and gives the opener's frames between. It fails where
    nothing dropped the view, where a `pagehide` dropped it first (the child closed before the
    opener closed it) or where no `pagehide` came. It reads `releasedBy` by an exhaustive
    `switch` that assigns its reading in each case, as `view/engine/status.ts` does, since oxlint's
    `consistent-return` refuses a function that returns from inside its cases.
  - **A hidden child is not resized.** Halfway, the scene used to resize the hidden child too,
    though an offscreen child has no size of its own and its resize check passed hidden anyway. A
    hidden run now resizes no window (`resizeDue`). So the smoke can borrow `:0` as an offscreen
    proxy, on the orchestrator's condition for T19.d's captures: every window made at its final
    size and never resized.
  - **Tests** (`childWindow.test.ts`, the renderer's) use a fake child window whose `pagehide`
    fires only when the test fires it:
    - the view is dropped at `close()`; the opener's next frame draws nothing in the child, and
      the late `pagehide` drops nothing;
    - a `pagehide` raised inside `close()` finds the view already dropped;
    - a `pagehide` from a close the opener did not make drops the view, and the opener's `close()`
      after it drops nothing;
    - in each case the view is disposed of once, and the hold stops listening once it drops it;
    - `closeRelease`'s five readings;
    - `resizeDue`: once, halfway through a shown child's time, and never for a hidden child.
  - **The hidden smoke, rerun** (2026-10-06, RTX 3080; not T21's record and not committed). The
    hidden scene ran as `just child-window-check --hidden --seconds 6` runs it, with
    `--force-device-scale-factor=1` added. It ran under `DISPLAY=:0` as the offscreen proxy, in a
    capped scope, both windows offscreen, never shown and never resized. There were nine runs in
    three sets of three: at load average 1.7, after the review's fixes at 16.3, and on the final
    code at 11.6.
    - All nine exit 0, with every check passing.
    - The view was dropped at the close in all nine. The `pagehide` came one of the opener's frames
      later in three runs (one of the first set, two of the second, none of the third), and before
      the next frame in the other six.
    - There were 0 uncaptured GPU errors, 0 device losses and no GPU-process exit in any run.
    - The opener drew 116–121 frames in the 2 s after the close in eight runs, and 71 in one of the
      loaded runs, whose frames' 95th percentile was 50.1 ms. Both windows' animation frames came
      at 16.70 ms (median) in every run.
    - Electron there reports one display, a `VX2450 SERIES` at 1920 × 1080 and 60.00 Hz, at the
      forced scale of 1.
  - **Gate.** No `just ci` (the Day 2 protocol): `pnpm --filter hyperion test` (319 files, 5,289
    tests), `just check lint` from a clean typecheck cache, and Prettier. There was no
    `just test-render`: the T21 scene is not among its checks, and the build and the smoke above
    load the changed page. The typescript-reviewer had no must-fix. Its two should-fix findings
    are fixed: the hidden run's skipped resize is now `resizeDue`, with tests, and `closeRelease`
    reads `releasedBy` exhaustively. Its consider is taken: the hold removes its `pagehide`
    listener.
- **Deviations in T16.a, as built** (2026-10-06; the views lane, T16 split as ruled under T16).
  - **Files.** `photoreal/overlay.ts` exports `overlayDrawList(list)` (every batch cased to at
    least `CASING_PX` in its own `casingColour`, `--surface-0` from `buildWireframeDrawList`; no
    sprites; the occluders and anchors kept as they are) and
    `overlaySubmission(renderer, list, camera, viewport)` (R02's `WireframeRenderer.frame` of it,
    labelled `symbology`, `colourLoad: "load"`, which `PhotorealRenderer` also still sets on any
    overlay, a harmless twin). `viewFrameDrawer.ts` takes its overlay from the latter at the
    canvas's viewport, not the scene target's internal one. `occluder.wgsl` changes only its
    header comment: the bias is `HULL_OCCLUDER_BIAS` in `drawList.ts`, which `submit.ts`'s hull
    material carries; R02's Design note 5, T14.b and T14's as-built note keep (128, 2) as R02
    built it, each with a pointer here. Beyond the subtask's list: `vitest.config.mts` (below),
    `test/viewFixtures.ts` (`OFF_PLANET_CAMERA`, `FIXTURE_MOON_ORBIT`, `poseOffPlanet`,
    `aMarkedViewScene`, moved out of R02's `drawList.test.ts`, which now imports them, so that the
    overlay's test shares its scene), `ViewDisplay.test.tsx`, and the README's line of what awaits
    the owner.
  - **The bias.** `HULL_OCCLUDER_BIAS` is `{ constant: 128, slopeScale: 3 }` in both styles:
    R02 Design note 5's w_max ÷ 2 + 1 for the cased edge's 3.5 px is 2.75 px, rounded up to the
    sphere occluder's `SLOPE_SCALE`. The constant is unchanged, so what shows through a face-on
    face is unchanged; behind a slanted face the slope term now lets a line within 3 px of the
    face's depth slope show through, where 2 px did, in both styles. R02.T14.c's show-through
    check draws a face-on square and does not exercise this. **Stated limit:** WebGPU scales the
    depth slope's larger screen component (the sphere occluder takes its magnitude in its own
    shader), so where a hull face's slope runs diagonally on the screen the term covers
    3 ÷ √2 = 2.12 px of the cased edge's 2.25 px of coverage, and the face may hide up to 0.13 of
    the casing's outer texel there. **For the orchestrator:** keep 3, as ruled (the UX decisions,
    item 12), or raise it to about 3.2 (2.25 × √2 = 3.18), which keeps the coverage in front on
    every slope and widens the show-through behind a slanted face by the same factor. _Ruled
    (decision-r07-t16a, item 1): neither. The hull faces take the sphere occluder's slope term,
    its magnitude in the fragment, at `occluderSlopePx`, with no hardware bias (T16.d)._
  - **Plates, as built.** Nothing changes: the label block and the marks' labels stand on their
    `--surface-0` plates (R02.T15). An instrument's slot over the primary's image is a `.panel` on
    `--surface-1`, opaque chrome with its label block on it (the guide's Views bullet: "each a
    panel"), not text on the image; the UX review agrees, and its look waits on the owner's look at
    the slots.
  - **A stylesheet in a test.** Vitest makes every CSS file empty, so a test cannot read the rule
    that paints a plate. `vitest.config.mts` sets `css.include` to `?raw` imports alone: an import
    for its effect stays empty, so jsdom still applies no rule, and `styles.css?raw` is its text.
  - **Tests.** `overlay.test.ts`: in `aMarkedViewScene` with a ring, every batch of the overlay
    (`body`, `hull`, `mark`, `orbit`, `predicted`, `ring`) is cased by `CASING_PX` in
    `--surface-0`, it has no sprite and keeps the occluders and anchors, and the wireframe's list
    keeps its hull edges uncased; the pass is `symbology`, loading, each batch's `--surface-0`
    casing two casings wider and before its stroke. `ViewDisplay.test.tsx`, through the drawer: on
    `PRECISION TEST` the hull's line draws are `[1.5]` in the wireframe and `[3.5, 1.5]` under
    `symbology`, where every line draw is a `--surface-0` casing followed by its stroke, as the
    wireframe's are not; with
    `INSTRUMENT 1` open over a photorealistic primary, every text over the image is on a
    `.view-label` or `.view-marks__label` plate, both are present, the slot holds text, and each
    plate's rule in `styles.css` paints `var(--surface-0)`. `drawList.test.ts` and
    `submit.test.ts` take the bias.
  - **The smoke check** (`smoke/wireframe.ts`, `checkCasedHullEdge`): a face receding from
    0.5 m to its far edge at 1 m, 24.35 px down the 64 px view; that edge as the wireframe's list
    has a hull's, uncased at `STROKE_PX.heavy`, cased through `overlayDrawList` (in `--text`,
    its casing in `--accent`, so that the casing's faint outer texel reads), against the same
    draw with no face, at 2 × 10⁻³ a channel in rows 20 to 29 of the middle column (the face's
    depth changes only down the view, so one column reads for all), and the casing's outer
    texel, 2.15 px below the edge, drawn. It tests the slope along an axis only; the diagonal is
    the stated limit above. `just test-render` passes it on both variants (that texel 0.0722 in
    green both ways). By hand, not committed: at `slopeScale` 2 it fails, row 26 covered.
  - **Open, for the orchestrator (the UX review's considers, not built).**
    - Stroke and casing widths are device pixels (`STROKE_PX`, `CASING_PX`, over a viewport
      scaled by `devicePixelRatio`), so at a ratio of 2, as on a Retina Mac, every stroke and
      casing is half a CSS pixel. It is R02's; the casing over the image now depends on it, and
      the slope bias is reasoned in device pixels too. Scale them by the ratio, or record device
      pixels as the unit. _Ruled (decision-r07-t16a, item 2; decision-thin-line-contrast, item
      2): CSS pixels. A line or casing, and a dash's length, is drawn at the larger of the ratio
      and 2 device pixels each, and a mark's outline at the larger of 1.5 × the ratio and 2,
      widened outward (T16.d, T16.f)._
    - The image has no hull in it, so the sky and its stars show inside a craft's cased outline,
      where the wireframe's faces hide them. Design note 17 keeps craft as outlines until R11;
      occluding the image under the hull's faces until lit craft exist would be a ruling. _Ruled
      (decision-r07-t16a, item 3): windows hide nothing; over the image a hull's opaque faces are
      a `--surface-0` silhouette under its cased outline, labelled
      `CRAFT PHOTOMETRY: NOT YET MODELLED` (T16.e)._
  - **Gate.** No `just ci` (the Day 2 protocol). The acceptance's vitest (32 files, 420 tests),
    the app's vitest (324 files, 5,433 tests), `just check lint` from a clean typecheck cache,
    Prettier, `just test-render` (both variants, 232 checks each, 0 uncaptured GPU errors) and the
    console-ux skill's lint (its two errors, `smoke/wireframe.ts`'s literal colours of R02.T14.c,
    predate this), contrast and glyph scripts.
  - **Reviewed** by the TypeScript, UX and plan-conformance reviewers, with no must-fix. Fixed:
    the smoke check's lengths named in metres, its texel comparison failing on a texel it cannot
    read, its widths from `STROKE_PX` and the casing from `overlayDrawList`; the shared scene moved
    to `test/viewFixtures.ts`; the plate test run with an instrument open; every line draw of the
    drawer's `symbology` pass checked as a casing and its stroke; the bias's show-through behind a
    slanted face stated; R02's two other `slopeScale: 2` lines and the README given pointers; and
    in the split, T16.b's bare statuses beside `ENABLE` and under `AUTO NOT AVAILABLE`, T8.a's
    focus consider given to T16.b, and `SELECT` and the "offers `MAN` only" wording given to
    T16.c.
- **Thin lines and the 6:1 (decision-thin-line-contrast).** The guide's 6:1 binds a line or an
  outline as drawn: the brightest device pixel across it, at its worst position on the grid. No
  line or outline that a canvas, an SVG or a view draws is under 2 device pixels (T16.d, T16.f).
  At that width it scores within 1% of its pair. Below a ratio of 2 every line is wider than its
  CSS width: 2.56 times at 0.78125, the development machine's and the UHD 620's ratio. A ringed
  circle grows up to 2δ more than other marks, so that the gap round its disc stays. Stated
  limits:
  - the DOM's own SVG strokes are tested by width, not captured;
  - the chrome's borders and outlines rest on Chromium's snapping of border widths to whole
    device pixels;
  - a projector's optics and scaler lie outside the pixels the checks read;
  - text, `☉` and `⊕` are scored on their pair.
- **Deviations in T16.b, as built** (2026-10-06; the views lane).
  - **Files.** The five the subtask lists. Beyond them: `styles.css` (the run class's comment);
    `test/fakeViewEngine.ts`, whose histogram read-back now weighs a `FakeMeteredImage` by the
    meter weights its dispatch carried (by default 1,000 `other` pixels in the middle bin, so that
    `AVG` meters it and `LIT` and `DARK` weigh nothing; a test may add a lit body); and
    `ViewDisplay.test.tsx` and `ViewDisplayLayout.test.tsx`. The guide is untouched (T16.c).
  - **The cause.** `AutoExposure.metering` is a `Metering`, `{ kind: "metered", ev100 }` or a
    `MeterCause` (`no-image`, `nothing-weighed` with its `meter`, `acquiring`); `meterStatus` gives
    the bare status, or `null` while metered or acquiring. `meteredEv100` is kept: R02's `setAuto`
    and `enable` take it. Two timers run: since the last histogram that weighed a pixel, and since
    the last of any kind. Once the first passes `METER_TIMEOUT_S` the value goes, as `no-image` if
    no histogram came in that time and `nothing-weighed` otherwise.
    - **`noteImage(drawn)`** (new). The loop calls it after each draw with the style drawn, so that
      the cause is true of the frame the readout shows: told at the step, before the draw, the
      controller would see the last frame's style and could stand `NO IMAGE TO METER` beside a
      newly drawn image for a readout. The controller, not the loop, now drops the histograms of an
      image no longer drawn.
    - **Beyond the text.** With no value held, `no-image` stands at once while no image is drawn
      (the wireframe), as `NO IMAGE TO METER` did before. The window opens whenever the image comes
      to be drawn with no value held, not only the first time, so that an image back from the
      wireframe is never met with `NO IMAGE TO METER`. A meter change restarts the window in both
      cases: with a value held it keeps that value for `METER_TIMEOUT_S` again (the old meter's
      histograms still in flight, one to three frames, may refresh it).
    - **`METER_STEP_MAX_S`** (0.1 s, after the UX review). The timers count at most that much of a
      step, so that a stall of the page, in which no read-back can be delivered, cannot time the
      meter out beside a drawn image. The first captures caught one: after a probe's synchronous
      measuring, the reading stood at `INHIBITED · NO IMAGE TO METER` beside `NO LIT SIDE` for a
      readout. The smoothing still takes the whole step.
  - **R02's `exposure.ts`.** `SystemInhibitCause` (`{ reason: "no_image_to_meter" }` or
    `{ reason: "nothing_weighed", meter }`), `NO_IMAGE_INHIBIT`, `NO_IMAGE_TO_METER` and
    `NOTHING_WEIGHED_STATUS` are new. `InhibitReason` gains `"nothing_weighed"`, and the
    `inhibited` variant is `{ kind, ev100 } & ({ reason: "operator" } | SystemInhibitCause)`.
    `onMetering(control, number | SystemInhibitCause)` replaces `ev100 | null`: a system inhibit
    takes each new cause, so that it always says why it holds, and returns itself where the cause
    is unchanged. The refusal `no_image_to_meter` is renamed `not_metered`, since one refusal now
    has three wordings, which `ExposurePanel` chooses by the cause. R07's Consumes, T13.a (1442),
    T13's as-built note and R02's T2.f note still name the old forms; Consumes and R02's note point
    here.
  - **The panels.** `ExposurePanel` and `MeterControl` take `metering: Metering` in place of
    `meteredEv100`. `AUTO_NOT_AVAILABLE` becomes `autoNotAvailable(metering)`, in the panel and
    under the compact layout's row. `MeterControl` takes the display's `exposure` and its `source`
    in place of the loop's `reading`, so that its reading is always the `Exposure` panel's, and
    `meterStatusLine(metering)` adds the remedy. Its status stands in the reading's place, as
    T8.a's `NO IMAGE TO METER` did, and `SOURCE` goes with it: the UX review's consider to keep it
    would add a row, about 20 px, to E10 and E11 below, which would then pass E3 and raise the least
    height by about 0.5 rem. In the window its reading stands as it is, with
    `METERED —`, the guide's missing value, under `MAN` and the operator's inhibit (UX review).
    `exposureNote` keeps the three statuses unbroken. The statuses, the meter's and
    `AUTO NOT AVAILABLE: …` in the panel and under the row, are now set in an `output`, a live
    region, since they come by themselves (UX review).
  - **One snapshot** (the UX review's must-fix). The metering went through the throttled
    `published`, and the exposure through the display's state, so a reading and its status could
    disagree for up to a readout. The loop now sets the metering in a state of its own in the frame
    it gives the exposure, not throttled again. `meteringFor(metering, meter)` shows a meter's own
    status as `acquiring` once another meter is chosen, so that the status clears in the choice's
    own render, before the controller takes it. `exposureShownChanged` compares the level
    readings, so that a new cause reaches the display.
  - **A standing system inhibit through the window: for a ruling.** The controller leaves the
    control as it stands while acquiring, so the reading keeps the standing inhibit's cause until
    the window ends, then reads the new meter's status or resumes `AUTO`: for up to 0.5 s
    `INHIBITED · NO LIT SIDE` under `METER DARK` after a change, and `INHIBITED · NO IMAGE TO METER`
    beside an image back from the wireframe. This follows decision-r07-t8a-meter's "shows its
    reading as it stands" and "The words follow the meter that found nothing", and the guide's
    "who inhibited it and why"; its "neither panel shows a meter status or `NO IMAGE TO METER`"
    reads against it. The UX review leans to keeping it, as the only honest reading; the
    plan-conformance review asks for a ruling. Other choices would be a bare `INHIBITED` in the
    window, or the new meter's status taken early; the guide has neither. T16.c's row must say
    what is ruled. _Ruled by the orchestrator (2026-10-06, the UX review's lean): as built, the
    reading keeping the standing cause until the window ends; T16.c's row says so._
  - **T8.a's consider, not settled: for a ruling.** In the compact layout the focus already goes to
    `CAMERA`'s disclosure (decision-r07-t19-layout). In the full layout, a meter button holding the
    focus when key `4`, a fault or the wireframe stand-in while the pipelines recompile unmounts
    the panel leaves the focus on the page's body; a press on the Style panel takes the focus there
    first. The full layout's target is a UX choice no ruling covers: (a) the Style panel's pressed
    button; (b) `CAMERA`, as in compact; (c) the Exposure panel's last control, `INHIBIT`, the
    meter's predecessor in column B (the UX review's lean); (d) leave it. _Ruled by the
    orchestrator (2026-10-06): (c). Built after T16.d: `ViewDisplay`'s focus effect gives the focus
    to `INHIBIT` (`ExposurePanel`'s `inhibitRef`) when the meter's panel goes in the full layout
    with the focus inside it; the compact layout keeps `CAMERA`. Tested in
    `ViewDisplayLayout.test.tsx`, with a twin that leaves a focus outside the panel where it is._
  - **For T16.c.** `NO IMAGE TO METER` can still stand beside a drawn image after a histogram
    timeout with the image drawn (a read-back fault), the plan's `no-image`. The ruling's sentence
    for the guide, "Never shown beside a drawn image", needs that exception. _Ruled by the
    orchestrator (2026-10-06): T16.c's sentence gains the exception._
  - **Tests.**
    - `autoExposure.test.ts`: under `LIT` with no lit body, acquiring until 0.45 s and
      `NO LIT SIDE` by 0.65 s, never `no-image`, and the inhibit raised with the status;
      `NO DARK SIDE` under `DARK`; `STAR DISC ONLY` under `AVG` over a field filled by a disc; a
      meter change acquiring at once, the window restarted and the reading kept until it ends; the
      control held while acquiring; `AUTO` resuming on a lit body; `no-image` while no image is
      drawn and acquiring when one comes; `NO IMAGE TO METER` when histograms stop beside a drawn
      image, and at once when the image goes; acquiring again when it comes back; `MAN` and the
      operator's inhibit left alone; one stalled frame not timing the meter out; `meteringFor`.
    - `exposure.test.ts`: the new inhibit's readings for each meter, `AUTO` resuming, each new cause
      taken, `INHIBIT` taking it over and `ENABLE` setting `AUTO` from it, `MAN` and the operator's
      inhibit left alone; `not_metered`.
    - `ExposurePanel.test.tsx`: each status bare beside `ENABLE` and in `AUTO NOT AVAILABLE`, the
      reading in its parts, never `NO IMAGE TO METER` nor the remedy, `INHIBIT`'s consequence under
      each new inhibit and its take-over, the statuses unbroken; in the window `ENABLE` held back
      with `NOT AVAILABLE: not yet metered`, no status, the reading as it stands, the `MAN` field
      taking an entry.
    - `MeterControl.test.tsx`: each status with its remedy in the reading's place, describing the
      chosen meter only, every meter choosable, unbroken, in a live region; the reading, `SOURCE`
      and `METERED —` in the window.
    - `ViewDisplay.test.tsx`, through the fake engine: `NO LIT SIDE` under `LIT` not by 0.3 s and by
      1 s, never `NO IMAGE TO METER`; a change to `DARK` clearing the status beside `ENABLE`, under
      `AUTO NOT AVAILABLE` and on the meter in the click's own render, then `NO DARK SIDE` after its
      window; `AUTO` resuming when a lit body is metered. `ViewDisplayLayout.test.tsx`: the folded
      exposure's `AUTO NOT AVAILABLE: NO LIT SIDE` under the row after the window, unbroken.
  - **By hand, measured** (hidden, never shown; 2026-10-06, the RTX 3080; the harness is
    `.git/rm23-scratch/r07-views/shots-t16b/`, from T19.d's, with `table16b.py`).
    - **The states.** After E7 (E8 in full), `PHASE TEST` drawn photorealistic at 10°, `ENABLE`
      under `AVG`, then `LIT` and `]` to the star, whose image holds no lit side: E9
      `INHIBITED · NO LIT SIDE`, E10 the same with `99` refused, E11 the operator's `INHIBIT` over
      it with `99` refused, E12 `MAN` with `99` refused, and E13 `INHIBITED · NO DARK SIDE` under
      `DARK`. The own-ship runs take them in `PRECISION TEST`, whose bodies are black. In compact
      each is captured with `EXPOSURE` open and with `EXPOSURE METER` open. `STAR DISC ONLY`, which
      needs a field filled by a star's disc, and the window's words, which last 0.5 s, are set in
      place in E9–E11 and measured with checks 1–3 and the probe, then put back.
    - **The runs:** `full`, `max1080` (856 px box), `atfull` (732 px), `belowfull` (731 px,
      compact), `compact720` (562 px), `zoom150` (565 px), `zoom125` (708 px), `ownship720` and
      `ownship150`. All 198 states pass every check of decision-r07-t19b-exposure-fit item 1, the
      0.5 rem probe included, and so does every variant set in place; no relaunch, GPU-process
      restart or failure was logged, and every page and box was exact. A second batch, after the
      reviews' fixes, measured the same to the 0.5 px.
    - **Compact, the panel, `Targets` and the spare**, in px:

      | State, the panel open | 1280 × 720 | 150% | 125% | Own ship, 720p | Own ship, 150% |
      | --- | --- | --- | --- | --- | --- |
      | E9 `INHIBITED · NO LIT SIDE`, `EXPOSURE` | 269.5 / 193 / 60 | 268.8 / 197 / 65 | 269.1 / 340.2 / 207 | 269.5 / 214.5 / 101 | 268.8 / 218.5 / 106 |
      | E10 E9 with `99` refused | 291 / 171.5 / 38 | 290.3 / 175.5 / 43 | 290.6 / 318.7 / 186 | 291 / 193 / 80 | 290.3 / 197 / 85 |
      | E11 the operator's inhibit over E9, refused | 294 / 168.5 / 35 | 293.3 / 172.5 / 40 | 293.6 / 315.7 / 183 | 294 / 190 / 77 | 293.3 / 194 / 82 |
      | E12 `MAN` beside `NO LIT SIDE`, refused | 269.5 / 193 / 60 | 268.8 / 197 / 65 | 269.1 / 340.2 / 207 | 269.5 / 214.5 / 101 | 268.8 / 218.5 / 106 |
      | E13 `INHIBITED · NO DARK SIDE` | 269.5 / 193 / 60 | 268.8 / 197 / 65 | 269.1 / 340.2 / 207 | 269.5 / 214.5 / 101 | 268.8 / 218.5 / 106 |
      | E9–E12, `EXPOSURE METER` | 160 / 281 / 148 | 141.8 / 302.5 / 170 | 142.1 / 445.7 / 313 | 160 / 302.5 / 189 | 141.8 / 324 / 212 |
      | E13, `EXPOSURE METER` | 160 / 281 / 148 | 159.3 / 285 / 153 | 159.6 / 428.2 / 295 | 160 / 302.5 / 189 | 159.3 / 306.5 / 194 |

      The tallest new state is E11, the operator's inhibit over `NO LIT SIDE` with a refused entry,
      at 35 px spare (720p) and 40 px (150%). With a five-character EV100, as the own-ship runs'
      `EV100 -14.0`, the reading `INHIBITED · STAR DISC ONLY` takes two lines, 20 px more (the
      own-ship E10 variant: 60 px spare against 80), so the arithmetic worst of the new states is
      E10 at that reading, 18 px at 720p, as T19.d's E6. The meter's panel is 160 px with its
      two-line status and keeps at least 148 px.
    - **Full, column B:** 652.5, 674, 674, 615 and 652.5 px in E9–E13, the meter 168 px with its
      two-line status in place of 150.5 px with `NO IMAGE TO METER`, while the exposure's
      `AUTO NOT AVAILABLE: NO LIT SIDE` takes one line where `… NO IMAGE TO METER` took two. With
      the 43 px refusal this machine cannot raise, at most 717 px, under E3's 731.5 px, so
      `FULL_MIN_HEIGHT_REM` stays 45.75 rem. At the least height (`atfull`) column B keeps 58 px in
      E10 and E11 and `Camera` 41 px; at the 53.5 rem box (`max1080`), 182 px and 165 px.
  - **Gate.** No `just ci` (the Day 2 protocol). The acceptance's vitest (32 files, 488 tests), the
    app's vitest (324 files, 5,476 tests; 325 files, 5,526 tests after merging
    `rendering-and-planets` with T10.c), `just check lint` from a clean typecheck cache, before and
    after that merge, Prettier, and the console-ux skill's lint (0 errors, the 11 old checks),
    contrast and glyph scripts. No `just test-render`: no shader, `view/engine/` or `src/smoke/`
    file changed.
  - **Reviewed** by the TypeScript, UX and plan-conformance reviewers. The UX review's must-fix,
    the two paths, is fixed (one snapshot, above). The plan-conformance review's must-fix, the
    reading through the window, waits on the ruling above, as does the focus. Fixed besides: a test
    of the folded row's own status, tests that query by what the operator sees, the loop's
    comment, the stall (`METER_STEP_MAX_S`), the live regions and `METERED —`. Not taken:
    removing `meteredEv100` and `InhibitReason` (R02's commands take the one, and the plan names
    the other), and `SOURCE` under a status (above).
- **Deviations in T16.d, as built** (2026-10-06; the views lane; decision-r07-t16a, items 1 and 2,
  as decision-thin-line-contrast, items 2 and 4, amends them).
  - **Files.** Those the subtask lists, and:
    - `lib/strokes.ts` and its test;
    - `smoke/strokeContrast.ts`, new, with a test of its arithmetic. It holds `checkStrokeContrast`,
      which the ruling names but places in no file, registered in `smoke/page.ts` as the group
      "R07.T16.d the view's strokes as drawn";
    - the options of the other callers and tests (`photorealFrame.test.ts`, `internalScale.test.ts`,
      `overlay.test.ts`, `ViewDisplay.test.tsx`, `spikeRun.test.ts`, `smoke/spike.ts`);
    - the eclipse scene's empty lists, now `emptyDrawList` (`smoke/eclipse.ts`,
      `scenes/eclipseScene.test.ts`);
    - `displays/view/ViewMarkLabels.tsx` and its test, and `reticleGrowthPx` in
      `wireframe/symbology.ts`, for the labels' clearance (below).

    `displays/view/useInstruments.ts`, which the ruling's table names, needs nothing: every
    instrument draws through `viewFrameDrawer.ts`.
  - **The strokes.** `DrawOptions` extends `ViewStrokes`: `strokeScale`, `markStrokePx`, and a third
    required field, `markShiftPx` (δ). δ depends on the ratio, which the other two do not give:
    0.78125 and 1 both draw at s = 2 and m = 2, but their δ is 0.41 and 0.25. `viewStrokesAt(ratio)`
    gives all three from `lib/strokes.ts`, and every caller spreads it: `viewFrameDrawer.ts` for the
    primary and each instrument, `DescentSpike.tsx` through `SpikeFrameInput.strokes`, and the smoke
    page at a ratio of 1. The list carries `strokeScale`, `markStrokePx` and `occluderSlopePx`. A
    mark's casing is a line's, `CASING_PX` × s. `lib/strokes.ts` takes a ratio that is not a
    positive number as 1.
  - **What the view draws, device px.**

    | Ratio | 1 px line | 1.5 px line | 2 px selected | Casing, each side | Dash on/off | Outline | δ | `occluderSlopePx` |
    | --- | --- | --- | --- | --- | --- | --- | --- | --- |
    | 0.78125 | 2 | 3 | 4 | 2 | 12/8 | 2 | 0.41 | 5 |
    | 1 | 2 | 3 | 4 | 2 | 12/8 | 2 | 0.25 | 5 |
    | 2 | 2 | 3 | 4 | 2 | 12/8 | 3 | 0 | 5 |
    | 3 | 3 | 4.5 | 6 | 3 | 18/12 | 4.5 | 0 | 7 |

    Before T16.d every width was the guide's figure in device px at every ratio: 1, 1.5, 2, casings
    of 1, outlines of 1.5, and a slope term of 3.
  - **The outward widening.** As ruled for most marks:
    - a body symbol's circle moves out by δ;
    - a ringed circle's disc moves out by δ and its ring by 3δ;
    - the bracket and destination reticles move out by 4δ.

    Where the ruling is silent:
    - **A polygon symbol.** Each side moves out by δ, and its corners by δ over the unit polygon's
      inradius (cos π/n), so that its inner edge stays where a 1.5 CSS px outline's would be.
      T16.f's "a symbol's path radius by δ" means the same for a circle, but would move a polygon's
      sides out by only δ cos(π/n): T16.f should match the view.
    - **Two marks the ruling does not list.** A target's ticks, the view's contact mark for a
      craft, move out by δ, as a symbol's line does. The flight path marker's circle moves out by
      δ, and its wings and fin start from it, so that both keep their open centres as built.
  - **The slope term.** As ruled, `occluderSlopePx` is 5 at every ratio up to 2 and 7 at 3, so 5
    device px of slope show through behind a slanted hull face, where decision-r07-t16a expected 3
    at a ratio of 1 or below (decision-thin-line-contrast).
    - The sphere's limb bound is 1.25 px.
    - `HULL_OCCLUDER_DEPTH_FRACTION` (2⁻¹⁶) is `drawList.ts`'s, and a test holds `occluder.wgsl`'s
      `DEPTH_FRACTION` equal to it.
    - Metal has no fine derivative, but on a plane coarse and fine derivatives agree, so the term
      is exact there too.
    - naga 30.0.1 validates both composed occluders and translates them to SPIR-V, MSL 2.1 and HLSL.
      At naga's default MSL version both fail on `instance_index`, as they did before T16.d.
  - **The smoke checks** (`smoke/wireframe.ts`).
    - T16.a's `checkCasedHullEdge` is kept on its axis, at s = 1.
    - `checkDiagonalCasedHullEdge` turns that face 45° and draws at s = 1 and 2. A 45° line meets
      the texel centres at one sub-pixel phase all along its length, so its far edge lies 24.153 px
      down the view before the turn. That puts texels in the band a larger-component push would
      lose: 2.190 px from the edge at s = 1 (the casing's coverage 0.06 there) and 3.604 px at 2
      (coverage 0.40). Six such texels at each scale draw as with no face, and no texel of the
      square differs.
    - `checkHullSlope` reads a hull face's depth at the turned face's middle texel. It is within
      1.3 × 10⁻⁴ and 1.5 × 10⁻⁴ of the f64 push by the slope's magnitude, at s = 1 and 2, against
      5.5 × 10⁻³ and 9.2 × 10⁻³ from the larger component's.
    - `checkSphereSlope` runs at s = 1 and 2, at 3 and 5 px, and matches to seven digits.
    - R02.T14.c's checks pass as before, the kept scenes drawn at `viewStrokesAt(1)`.
  - **`checkStrokeContrast`.** It draws three scenes on a 256 px square, at ratios 0.78125, 1 and 2,
    in the wireframe over `--surface-0` and through the overlay over `--text`:
    - a ring seen face-on, from cameras rolled 0°, 3° and 45°, whose ticks then lie at those angles
      and every 10° on;
    - a planet seen pole-on, its limb 110 px from the centre and rolled the same, for its limb,
      meridians and prime meridian;
    - two open circle symbols, the selection's and the destination's on different bodies, beside a
      dashed predicted path, and a third craft's target ticks.

    **The cross-section** at each pixel of length is the texels whose centres lie within half a
    texel's diagonal along the stroke and within its half-width and fringe across it. That is the
    cross-section the ruling's figures take: a stroke up to 2 px wide then peaks at w ÷ 2 of its
    colour at its worst position, and a 2 px one at all of it. With half a pixel along instead, a
    2 px line at 45° finds no texel within 0.71 px of its centreline at a corner of the grid
    (5.9:1).

    **Crossings are not read.** A cross-section that another batch's stroke or casing reaches is
    left out, because a later batch's casing covers an earlier stroke where they cross, by design
    (a stated limit).

    **The readings.** Every kind reads its pair's ratio at every ratio, in both styles:
    - the ring's edges and ticks, the limb and meridians (1 px), and the prime meridian (1.5 px),
      `--text-muted`: 7.22:1;
    - the symbols, the dashed path and the target ticks, `--text`: 13.57:1;
    - the `--accent` reticle: 10.37:1;
    - the `--target` reticle: 8.15:1.

    The control, the ring as built before T16.d, reads 4.20:1 on its 1 px edges at each ratio. Each
    kind must read at least 20 points over its frames, and the target's ticks, four of about 3 px,
    at least 8: they read 12 over the image at 0.78125.
  - **Captures.** decision-thin-line-contrast retires the brief's "captures at 0.78125 do not move":
    every line at a ratio under 2 is now at least 2 device px.
    - **`just test-render --captures`.** Before (at f57f2b1) and after, 98 of the 110 captures are
      byte-identical. The 12 that move are R05 spike's `craft` and `orbit` instruments, at its
      three shots on both variants. They are the only captures drawn through
      `buildWireframeDrawList`: their lines go from 1 to 2 device px, and their outlines from 1.5 to
      2.
    - **Hidden captures of `VIEW`'s canvas** (`.git/rm23-scratch/r07-views/t16d/shots/`): PRECISION
      TEST and FRAME CHANGE TEST from `SEAT` and from `CHASE`, in both styles, the frame clock
      frozen so that a kept scene holds at t = 0 (two runs of one build are byte-identical).
      - At a ratio of 0.78125, every frame moves at its strokes only: 0.31% to 1.84% of its pixels,
        nearly all brighter.
      - At 2 they move against f57f2b1 by 0.23% to 1.40%, since every width doubles there. Their
        widths are decision-r07-t16a's (s = 2, m = 3).
  - **The acceptance's vitest paths** leave out `smoke/strokeContrast.test.ts`, the test of the
    check's arithmetic, cross-section and dash phase. It runs in the app's whole suite and in
    `just ci`.
  - **Open, for the orchestrator** (the UX review's considers, not built):
    - **Graticule thresholds.** A body's graticule steps (drawn from 8 px across, and at 15° from
      64 px) are still device px, chosen for 1 px lines. With 2 px lines, a planet some 28 px across
      has its 30° cells nearly filled (`after-a-precision-chase-wireframe.png`). Scaling both
      thresholds by the line scale would keep the gaps as built.
    - **The view's clear colour.** `CLEAR_COLOUR` (`view/engine/webgpu/drawing.ts`) is black, not
      `--surface-0`. This predates T16.
  - **By hand, not committed** (`.git/rm23-scratch/r07-views/t16d/byhand/`). The hull's hardware
    slope term was put back (`depthBiasAway { constant: 128, slopeScale: 3 }`, the fragment writing
    no depth) and the default variant run on SwiftShader:
    - `checkHullSlope` reads 0.1226247 at both scales, which is the larger component's push
      (0.1227427, against the magnitude's 0.1171979 at s = 1): SwiftShader takes the larger
      component.
    - `checkDiagonalCasedHullEdge` fails at s = 1, where ten texels of the casing differ, the six
      at risk among them. It fails at s = 2 too, where 3 px cannot cover a 4 px reach.
    - T16.a's axis check passes.

    On both paths a constant 1.2 to 1.5 × 10⁻⁴ separates the depth SwiftShader writes from the
    `f64` model, a 0.02 px shift of the face, inside the check's tenth.
  - **Two clearances the ruling's widening closed, kept (after the UX review; for the
    orchestrator).** decision-thin-line-contrast lists "label positions" as unchanged, and gives the
    destination reticle no rule of its own. Both are kept as built at a ratio of 4/3 and above.
    - **The marks' labels.** A mark's DOM label stands 0.75 rem right of it, on an opaque
      `--surface-0` plate. Below a ratio of 1, the selection's bracket about a craft, moved out by
      4δ and widened by δ, ran under that plate: at 0.78125 the plate covered its right arms' outer
      half within the label's height (about 4.8:1 at the worst phase, and worse at 80%).
      `markLabelShiftPx` moves every label out by the bracket's growth, 5δ ÷ the ratio CSS px
      (2.65 px at 0.78125, 1.25 at 1, 0 from 4/3 up). Its clearance from the plate is then that of a
      1.5 CSS px bracket at its as-built place, at every interface scale (tested at 80%, 100% and
      150%, at ratios 0.78125, 1 and 2). A selected body of size class 3 or 4 has run under its
      label's plate since R02.T15 (its bracket's half-size is 0.69 to 0.75 rem); it is no worse.
    - **The destination's reticle on the selection.** It stood 0.25 rem outside the bracket: 3.1
      device px at 0.78125, and 2.5 at 80%. Its casing, drawn after the bracket, reaches 3.5 px, so
      it covered the bracket's full-coverage core (about 6.3:1 at 100% and 3.4:1 at 80%, from the
      ramp). The margin is now at least an outline and one casing, 4 device px below 4/3, so that
      the destination's casing never reaches the bracket's core. That is tested at 80%, 100% and
      150%, at ratios 0.78125, 1 and 2. No view draws a destination yet: `viewFrameDrawer.ts` and
      `spikeRun.ts` pass `null`.
    - **Seen, not changed.** About a craft, the destination reticle's right arms stand 0.875 rem
      out, and have run under the label's plate at every ratio since R02.T15. That too is latent
      until a destination is drawn.
  - **Gate.** No `just ci` (the Day 2 protocol).
    - The acceptance's vitest: 87 files, 1,520 tests.
    - The app's vitest: 327 files, 5,581 tests.
    - `just check lint` from a clean typecheck cache, and Prettier.
    - `just test-render`: both variants exit 0, 538 checks (269 each, 255 before), with no
      uncaptured GPU error.
    - naga, as above.
    - The console-ux skill's lint (no new error; its three, `smoke/wireframe.ts`'s test colours and
      `submit.ts`'s colour parser, predate T16.d), contrast and glyph scripts.
  - **Reviewed** by the TypeScript, UX and plan-conformance reviewers.
    - **The UX review's two must-fixes** are fixed, and the reviewer has confirmed it: the labels'
      clearance and the destination's margin, above.
    - **The TypeScript review's should-fix**, a reason above each new lint suppression, is fixed.
      So are its two considers: `INHIBIT`'s ref passed as an object, and the turned face's corners
      as a tuple.
    - **The plan-conformance review's three should-fixes** are fixed or recorded: the planet's limb
      placed at 110 px so that the check reads it; Consumes, the T16 consider and Provides given
      pointers; and the acceptance's vitest paths recorded.
    - **Considers taken:** a ring and a symbol in the scale tests, the target ticks read, and the
      sample floor kept at 20 but for the ticks.
- **Deviations in T8.a, as built (part 1: the disc regime, the lights and the phase scene).**
  - **Files.** `bodies/draw.ts` (`planLitBodies`, `pointFlux`, `hostAnnuli` (_moved to
    `lighting/hostLights.ts` by R07.T10.b_), `LitBodyRenderer`, `BODY_DISC_MATERIALS`), with the
    record, its packer and the shader's `f64` twin in
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
    are within 1% at 3 px. _Amended by R07.T8.c (decision-r07-small-disc-cost): 8 × 8 below 4 px,
    4 × 4 from 4 to 32 px; see part 2's "Sampling, as built"._
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
    2% for a Moon-sized body 3 px across. _Replaced by R07.T10.b: the point takes the eclipse over
    its disc, and the two agree to 2 × 10⁻⁵ there._ An occluder beyond the star hides nothing; a
    point inside one sees none of it.
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
    kept scenes are static, and `PHASE TEST` is exact. _Removed by R07.T10.a (see "Deviations in
    T10.a, as built")._
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
    _Amended by R07.T8.c (decision-r07-small-disc-cost): 8 × 8 below 4 px, 4 × 4 from 4 to
    32 px._
  - **Promotion** (`promoteOverlapping`) is not called: the frame has no depth-writing geometry
    yet (no mesh bodies, terrain or lit hulls); T9 calls it with footprints. _Called from T9's
    `planLitBodies` (see "Deviations in T9, as built")._
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
    6.1 a ÷ D at 150° and 0.59 a ÷ D at 90°, to 0.2%. _Since T8.c these rows run at 4 × 4, and
    the near field, taken once centred, is carried to each placement by the point's ratio (see
    "Deviations in T8.c, as built")._
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
    consider for T16). _Settled for the compact layout by T19.b and for the full one after T16.b
    (the orchestrator's ruling): see "Deviations in T16.b, as built"._
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
    _Built by T16.b (2026-10-06); see "Deviations in T16.b, as built"._
  - **Tests.** `MeterControl.test.tsx`: with nothing metered, only the pressed button is described.
    `ExposurePanel.test.tsx` (new): under `AUTO`, `ENABLE` is held back and described as
    `NOT AVAILABLE: the exposure is AUTO`; under `MAN`, `INHIBIT` as
    `NOT AVAILABLE: the exposure is MAN`. `exposure.test.ts` takes the renamed reason.
- **Sample counts by size** (T8.c, decision-r07-small-disc-cost).
  - Where the counts change: 8 × 8 below 4 px, 4 × 4 to 32 px, one inside and 4 × 4 on the limb
    above. The counts are a pure function of the frame, with no hysteresis.
  - The steps: a disc crossing 4 px changes its flux by up to 0.41%, and one crossing 32 px by up
    to 0.29%. That is under half an 8-bit step at white.
  - Pixel errors against brute force: at most 1.24% of the disc's brightest pixel, in a fully
    covered pixel by a 12 px crescent's terminator. The limb's worst, 1.16%, is a large disc's
    own limb error. The RMS is at most 0.20%.
  - The dominant flux error at high phase: the crescent's terminator lying in centre-sampled
    cells beyond the nine-point band, at most 0.8%. _As built, against a near-field oracle that
    follows each placement's phase: at most 0.52% (see "Deviations in T8.c, as built")._
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
    only the bodies larger than the neighbour. _Replaced by R07.T10.b: the eclipse over the
    neighbour's disc as the body it lights sees it, by bodies of every size (see "Deviations in
    T10.b, as built")._ This fixes two cases:
    - In a total lunar eclipse, Design note 7 as first written left 0.31 lx of moonlight on
      Earth's night side. The truth is about 10–50 µlx, light refracted by Earth's air, which no
      plan draws yet (Hernitschek, Schmidt and Vollmer 2008, Applied Optics 47, H62, Table 2: 9.6
      and 11.15 mag below full; 12–54 µlx from the abstract's −3.32 and −1.7 mag at
      V = 0 = 2.54 µlx).
    - It also fixes every full phase of a giant's moons seen from the giant.
    - Its two errors, removed by R07.T10.b's disc-averaged eclipse:
      - A smaller body's shadow is left out. From the neighbour's centre it would hide the whole
        star where it hides a spot: about 0.1% of Jupiter-shine for Io's shadow, and up to 11% of
        earthshine in a central solar eclipse (10.8% by brute force).
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
      the sunlight. The plan's 70 lx ± 10% is kept. _R07.T10.b: Io's own shadow now takes 0.104%
      of it, and the closed form is met by the clear light._
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
    time. _Removed by R07.T10.a (see "Deviations in T10.a, as built", "Neighbours")._
  - **For R10 and T9.** A lit point's planetshine is `planetshine_irradiance` with the horizon
    argument (0 on the smooth figure) and `lit_disc_term`, over the `SecondarySource`s of its body.
  - **Possible follow-up (not built).** A table in (α, sin ρ, L) would correct the far-field
    illuminance's errors: first order in R ÷ Δ near full phase, and past quarter phase the hidden
    limb crescent (at Io, 34% below at 120°, 71% below at 150°, nothing from 170.6°).
  - **Found, for T10 (not fixed here) → R07.T10.b (decision-r07-earth-albedo).** A point body
    takes its eclipse from its centre, so a point Jupiter goes fully black during Io's shadow
    transit: `pointFlux` gives 0 against 5.16 × 10⁻⁵ lx clear, measured 2026-10-04. R07.T10.b's
    disc-averaged eclipse, shared by `pointFlux` and planetshine's neighbour, fixes it. _Fixed by
    R07.T10.b: it keeps 99.896%._
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
- **Deviations in T13.c, as built (one camera, decision-r07-exposure-camera).**
  - **Files.** R02's `photometry/exposure.ts` takes `ExposureProgram`, `VIEW_CAMERA` and
    `programTriple` from `post/autoExposure.ts`, whose `VIEW_AUTO_PROGRAM` and local program are
    gone; `AutoExposureOptions.program` is R02's `ExposureProgram` and `ViewDisplay` passes
    `VIEW_CAMERA`. One private check, `isValidTriple`, serves `ev100FromTriple` (`RangeError`) and
    `setManual` (`invalid_triple`); R06's `cameraLimitParts` also throws on a negative or
    non-finite ND. Provides lists none of the new names, since they live in R02's file.
  - **The ND's densest is derived.** `maxNdEv` = 42 − log₂(1.4² × 8,000) = 28.0634 EV (OD 8.448),
    the ruling's "28.06 EV, OD 8.45", shown `28.1`, so that the ND reaches it exactly at EV100 42,
    `MAN`'s top, and pegs only above, as the ruling's readout says. The 42 is a private
    `VIEW_ND_TOP_EV100` until T13.d's `MAN_EV100_MAX`, which may replace it.
  - **`programTriple`** is written for any base ISO: t = N² × 100 ÷ (S_base × 2^EV100) and ND =
    EV100 − (log₂(N² ÷ t_min) − log₂(S_base ÷ 100)), the ruling's forms at base 100. It reads
    neither `maxIso` nor `maxNdEv`: the triple keeps the full pushed S and the full ND, so
    `ev100FromTriple` returns the reading (to 1.8 × 10⁻¹⁵ from −14 to 42), and the pegs are the
    panel's. `ndEv` is absent while the filter is clear, at the join itself included. It throws a
    `RangeError` on a non-finite EV100, which no reading carries.
  - **The ND against a shorter shutter.** The dark current builds over the whole shutter whatever
    the ND, so an ND of x EV equals a shutter of t × 2^−x only at no dark current, which
    `DEFAULT_VIEW_CAMERA` has; the 10⁻⁹ mag test runs there.
  - **The limit through the program** at 60° and μ 24: 10.058 from EV100 −14 to −1, 10.056 at 1,
    9.399 at 5.88, 6.317 at 10 and 2.555 at 15; the tests hold 9.40, 6.32 and 2.56 to the ±0.01
    the ruling gives 10.06. The science check (2026-10-04) re-derived every figure from the code
    and found no fault.
  - **The panel.** The rows are R02's shared `.readout` grid, one member per row as in the ruling's
    sample, in place of R02's wrapping row of `.field`s, at a 0.25 rem row gap; each value is at
    least 9ch wide (`1.25E-4 s`, `409,600 ↑`, `28.1 EV ↑`). The `↑` is `aria-hidden`, followed by
    a visually hidden "off scale high". The ND reads `CLEAR` while `ndEv` is absent, and its value
    at one decimal while the filter is in, so from EV100 13.94 to about 13.99 it reads `0.0 EV`
    (open, below). The ND test runs under `INHIBITED · OPERATOR` at 20; an added test pegs a `MAN`
    triple's 30 EV at `28.1 EV ↑`.
  - **The side column (open, for the orchestrator and the owner).** Measured in a hidden window
    (never shown) at 1920 × 1080 over a kept scene, under the default `MAN`: the four rows stand
    84 px plus an 8 px margin, where R02's single row of three stood about 26 px, so the side
    column's content runs 62 px past its foot with both instruments closed (`ENABLE` 13 px and
    `INHIBIT` 49 px below it, clipped) and 26 px with both open (`INHIBIT` clipped), where before it
    fitted by about 4 px. Two pairs a row (`APERTURE`·`SHUTTER`, `ND`·`ISO`) would leave 18 px with
    both closed and fit with both open, but widen the content-sized column from 387 to 441 px,
    narrowing the stage. At 1280 × 720, and photorealistic at 1080p, the column already overflowed
    (T19's entry above). Built as ruled; the short page's rule (T19's pending item) or a ruling on
    the setting's form settles it.
  - **Until T13.e**, `limitTriple` reads the new `DEFAULT_MAN_TRIPLE` under `AUTO`: the request and
    the cull ask V 10.06 at 60° (it was 13.7, cut at `MAX_CUT_V`'s 11), and the label reads
    `STARS V 10.1 mag CAM` at every `AUTO` exposure, not the exposure's own limit. No test pinned
    the old figure. The panel's private `shownTriple` is T13.e's `shownTriple(control)` in waiting;
    one shared helper in `exposure.ts` would keep the panel and the label from drifting.
  - **Plan and guide text.** decision-r07-man-exposure's own plan edits had never been applied, so
    this ruling's replacements went in against the text as it stood; its T16 insertion names T13.d.
    T13.d's entry amends decision-r07-man-exposure's subtask text, which is in that decision file
    only: T13.d's agent takes it from there. The ruling's `G` and `BS` are written out, and the
    Consumes insertion sits inside the parenthesis so that the item keeps R06.T13.a as its
    provider. R02's as-built lines (`02` 1411, 1664–1666) still say f/1, 2 s, ISO 100 and the
    triple under `MAN` only, left as R02's history; the README's cross-plan row for
    `ExposureReading.triple` (met by R07.T13.a) is the orchestrator's to update. In the guide, the
    `APERTURE`, `SHUTTER`, `ND`, `ISO` row's "An exposure darker than ISO 409,600 can make reads"
    is repaired to "Where the exposure is darker than ISO 409,600 can give, the setting reads"; the
    row names T13.d's `MAN` field as the ruling words it; and the `EV100` row's draft tag adds
    R07.T13.c beside R02.T2.f. All three are drafts for the owner.
  - **Open (for a decision agent):** whether an ND under 0.05 EV (EV100 13.94 to about 13.99)
    reads `CLEAR` or `0.0 EV`; the ruling's "`CLEAR`, or the attenuation in `EV` at one decimal"
    allows both, and `0.0 EV` is built.
- **Deviations in T13.d, as built (the way back to `MAN`, decision-r07-man-exposure as amended by
  decision-r07-exposure-camera).**
  - **Files.** R02's `photometry/exposure.ts` gains `MAN_EV100_MIN` (−14), `MAN_EV100_MAX` (42),
    `setManualEv100` and the refusal `invalid_ev100`; T13.c's private `VIEW_ND_TOP_EV100` gives way
    to `MAN_EV100_MAX`, `maxNdEv` unchanged (28.0634 EV). `ExposurePanel` gains a private
    `ManualEntry`, the `MAN` field; `MeterControl` gains `meteredEv100: number | null`, which
    `ViewDisplay` passes from `shown.meteredEv100`. Provides lists none of them, as in T13.c.
  - **`setManualEv100(ev100)` takes no control.** The ruling's `(control, ev100)` needed the control
    only for the superseded "keep the `MAN` camera's aperture and ISO": with one camera the triple
    is `programTriple(VIEW_CAMERA, ev100)` from every level. So "accepted from every level" is
    tested at the panel (`AUTO`, both inhibits and `MAN`), and the 9.6 test calls it with the EV100
    alone.
  - **Parsing is `CURSOR`'s.** Grouping commas are dropped, the sign may be `+`, `-` or `−`, and the
    value is rounded to one decimal before the span is checked, so 42.04 enters as 42.0 and −14.05
    as −14.0 (`Math.round` takes halves up). Text that is not a number is refused at the panel with
    the same `invalid_ev100` words.
  - **The field.** The fill is taken once, at focus (the ruling's "on focus"), and does not follow
    `AUTO` while the field holds focus untyped. It is also written to the input in the focus
    handler, so that its selection holds, and a press that focuses the field cancels its mouseup's
    default, which would collapse the selection in Chromium (tested by the event's
    `defaultPrevented`). A refused text stays, `aria-invalid`, until it is edited or `MAN`'s value
    changes elsewhere (an entry, or `ENABLE` from `MAN`), as `CURSOR` keeps its draft. The field is
    described by its unit, its hint, the consequence line and the refusal; its `—` is `--text-muted`
    on `--surface-2` (6.04:1). Text selected in any `.form-field__input` is now `--accent` under
    `--surface-0` (10.37:1), not the platform's highlight, since this field is the first to select
    on focus.
  - **`METERED`** stands after the meter's reading and before `SOURCE`, where the ruling names no
    place, its value 11ch wide for `EV100 -13.6`.
  - **The side column (adds to T13.c's open item, which R07.T19.b settles).** The consequence line
    and the refusal take `contain: inline-size`, so that they never widen the content-sized column.
    Measured in a hidden window (never shown) over a kept scene with both instruments closed, the
    panel heights depending on the column's width only (the same at 1280 × 720):
    - at the as-built 387 px column the exposure panel grows by 40 px under `MAN` (the field's 2rem
      row and its margin), by 80 px at another level (the consequence line takes two lines) and by
      22 px more with a refusal; the meter grows by 18 px with `METERED`;
    - at 1920 × 1080 the column's content now runs 102 px past its foot under the default `MAN`
      (62 px after T13.c);
    - in T19.b's 26rem compact column the exposure panel stands 301 px under `MAN` in the wireframe,
      315 px beside a drawn image at `AUTO`, 360 px trapped in the wireframe
      (`INHIBITED · NO IMAGE TO METER`) and 382 px with a refusal there, at today's 0.75rem block
      padding (T19.b's 0.5rem takes 8 px off), where decision-r07-t19-layout estimated 290–300 px;
    - in T19.b's 20rem column B the hint wraps under the field (the row needs 281 px of the 277 px
      there), so the field adds 58 px under `MAN` and 98 px at another level.
  - **Guide.** The new rows' draft tags name R07.T13.d where the ruling, written before the rename,
    said T13.c, and the `AUTO`, `MAN`, `INHIBITED` row's tag gains "and plan R07, R07.T13.d", as
    T13.c tagged the `EV100` row. The `MAN` field row has no "keeps the `MAN` camera's aperture and
    ISO" clause (decision-r07-exposure-camera): the `APERTURE`, `SHUTTER`, `ND`, `ISO` row's "takes
    the same setting" says it. The "Exposure is an instrument" bullet's tail is reflowed. All are
    drafts for the owner.
  - **README.** The cross-plan row for `ExposureReading.triple` (T13.c's entry left it to the
    orchestrator) now names `VIEW_CAMERA` and `programTriple` (T13.c) and T13.e.
  - **Gate.** No `just ci` (the Day 2 protocol) and no `just test-render`: no shader, `view/engine/`
    or `src/smoke/` file changed, and no rendered value.
  - **Reviewed** by the TypeScript, UX and plan-conformance reviewers, with no must-fix. Their
    should-fix points are fixed but the three open below: tests of a refusal that `ENABLE` drops, of
    `MAN` from the operator's inhibit and of the mouseup guard; `userEvent.setup()` before `render`;
    the selection's colour, `METERED`'s width and the field's `.form-field` wrapper.
  - **Ruled (decision-r07-t13d):**
    - every field that enters on `Enter` or when left takes `Escape`, which drops a typed or
      refused entry and shows its value again, and enters empty text as nothing (T13.d's
      follow-up, which also changes `CURSOR`'s and the chart's fields, the guide's entry rule
      being ship-wide); the refusal words are unchanged;
    - the `MAN` field's fill stays the exposure as it stands outside the span and is refused there
      as a typed value is, a refused fill dropped on leaving: a clamped fill would show a value the
      exposure does not have and step the image unasked, and `INHIBIT` holds any value; a frame of
      exact zeros, which drove `AUTO` without end, now meters no darker than EV100 −14 (T13.a's
      follow-up), so the fill keeps its width;
    - `INHIBIT` states `Then AUTO resumes only on ENABLE` beside the button under `AUTO` and a
      system inhibit (the guide's consequence rule), on the button's row: no height in T19.b's
      26rem column, at most about 10 px as built and in T19.b's 20rem column B (measured: none as
      built, 12 px in column B; T13.d's follow-up's deviations);
    - the entry and commanding bullets' new sentences are drafts for the owner, as the rows are,
      and so is the entry bullet's "selected" (decision-r07-t19b-exposure-fit, item 6; T13.d's
      second follow-up).
- **Deviations in T13.e, as built (the sky's limit follows the camera, decision-r07-exposure-camera
  (c)).**
  - **`shownTriple` is R02's.** `shownTriple(control)` lives in `photometry/exposure.ts` beside
    `programTriple`, where the ruling put it in `viewSky.ts`, and `ExposurePanel` imports it in
    place of its private copy, so that the panel's rows and the label's limit read one function.
    R06's `viewSky.ts` gains `deepestTriple(program)`, the program's triple at
    log₂(100 × N² ÷ (t_frame × S_max)), −6.1223 for `VIEW_CAMERA` (f/1.4, 1/30 s, ISO 409,600,
    clear). `limitTriple` is gone, and so is `viewSky.ts`'s `ViewSkyInput.exposure`, since the
    request no longer reads it; `useViewSky.ts`'s own `ViewSkyInput` keeps its `exposure`, for the
    label.
  - **Two functions for the two limits.** `viewSkyLimit(model, role, fovDeg)` gives the cull's
    `ViewStarLimit` and `viewSkyLabelV(model, role, exposure, fovDeg)` the label's V, in place of
    one `viewSkyLimit` returning both, so that the cull cannot be keyed on an exposure it does not
    read; `ViewSkyLimit` is gone. In `useViewSky.ts`, `cullViewSky(model, role, fovDeg, widthPx)`
    returns the `DrawnSky`, and `viewSkyLabel(model, role, exposure, fovDeg)` the `STARS` reading.
  - **The label reads the display's control**, through `shownTriple`, not
    `ExposureReading.triple`: the two are equal wherever a reading exists (`AutoExposure` builds
    its triple from `VIEW_CAMERA` the same way), and a wireframe view, which has no reading, keeps
    its label. So the Provides comment on `ExposureReading.triple` ("R06's `cameraLimitV` reads it
    (T13.e)"), the Consumes item on it and the README's row for it hold through `shownTriple`. The
    display's control reaches the hook at the readout's 4 Hz and only when its EV100 moves by 0.1,
    so the label moves at that rate. It is computed in render, not memoised: a few operations for a
    camera, and for an eye the deepest of a 64² band's 24,576 texels, which ran once per cull
    before and now runs on each render of the hook.
  - **Instruments.** R07.T19's `useSlotSky` follows: each slot culls at its camera's deepest limit
    and labels at the primary's shown exposure. The slot loops' inputs are now keyed on each drawn
    sky's identity rather than on the slot's `{ drawn, labelValue }`, so a new label does not remake
    them.
  - **Who asks a camera's limit.** The primary view's role is `eye`, so today only the instruments
    are cameras, and they cull the primary's eye sky; no running view sends a `camera_limit_v` yet.
    The hook's tests therefore drive `useViewSky` with an instrument's run directly.
  - **Tests.** `viewSky.test.ts`: `deepestTriple`, and no exposure in `MAN`'s span reaching deeper;
    the request at V 10.06 at 60° and cut at 11 at 30° and 13°; the cull at 10.06, 11.72 and 13.58;
    the label at the default `MAN` (10.06) and at `AUTO` 15 (2.56), and one label for one EV100 at
    `MAN` and both inhibits; the cull parting from the label by 0.05 mag or more at every tenth of
    an EV100 from 3.5 to 42 and at none below; a Sun-coloured star at the label's limit, centred in
    a pixel at the frame's centre, below 2^`AGX_MIN_EV` and drawn as `TONE_CURVE_BLACK`, at 1080p
    and at 4K (60°); and the corner bound below. A new `useViewSky.test.tsx`: the first request at
    V 10.06 under `MAN` −1, `MAN` 15 and `AUTO` −10, 0 and 15, each from a fresh view; once a sky
    asked at `MAN` 15 is held, no second request through `AUTO` from EV100 −10 to 15 by tenths and
    `MAN` −1, 15, −14 and 42; the drawn sky's identity across them; the label `V 10.1 mag CAM` at
    `MAN` −1 and `V 2.6 mag CAM` at `AUTO` 15. Four of these fail at 8e11907 (the request under
    `MAN` 15, the second request, the identity and the label); the request under `MAN` −1 and
    `AUTO` passed there too, at the default triple. `InstrumentView.test.tsx`: an instrument over
    the server scene's sky reads `STARS V 10.1 mag CAM` at the default `MAN` and `V 2.6 mag CAM`
    after a `MAN` entry of 15, the primary's exposure reaching its label. Its `AUTO` path is the
    same `viewSkyLabel` the hook's test drives under `AUTO`, since the fake meter's `AUTO` value
    (about EV100 3.3 after `ENABLE`) is an artefact of its fixed histogram, and its cull's identity
    is not observable through the fake engine. `InThreadSkyWorker` moves from
    `ViewDisplay.test.tsx` to `test/skyFixtures.ts`, and `autoAt` and `manualAt` are a new
    `test/exposureFixtures.ts`.
  - **Off the frame's centre (science check, 2026-10-05).** The ruling's 3.4–5.0 and 1.4–3.0 EV
    below AgX's floor are for a Sun-coloured star at the frame's centre, whose pixel has the largest
    solid angle. At a 16:9 corner at 60° (cos³θ = 0.579) a star is 0.79 EV brighter. As a bound,
    the bluest colour `starColour` gives (25,000 K) with the largest camera band term (A0V's
    +0.14, decision-camera-eta), admitted at V = limit − 0.14, stands 0.28 EV above the floor at
    4K and EV100 3.5 (0.24 EV with O5V's +0.11), inside AgX's toe: the sprite path gives at most
    2.4 × 10⁻⁷ display-linear and the full-screen pass moves at most 2.5 × 10⁻⁶ from black, under
    0.01 of an 8-bit sRGB code each, against the dither's ±1 code (tested). The science check's
    "`spriteToneCurve` still gives 0" holds everywhere but that 4K corner bound. At 1080p every
    case stays 1.7 EV or more below the floor. `viewSky.ts`'s header says so. `deepestTriple`'s
    limit is the deepest of the program's triples while the program's top gain is the sensor's; a
    `setManual` triple with a shutter beyond 1/30 s, which no console control sets, would label
    deeper than the cull, the ruling's "full manual mode" case.
  - **For the owner (from the plan-conformance review).** Away from the frame's centre at 4K, the
    stars between the two limits are hidden by AgX's toe, not by lying below its floor: so the
    deeper cull also departs, in that corner, from the brainstorm's "the camera's cut is an explicit
    noise-floor model rather than left to tone mapping", beside the "each view thresholds its copy"
    the ruling names. Built as ruled, since nothing visible changes; the alternative, culling at the
    exposure's own limit or with a margin, is what the ruling turned down for its cost (a re-cull of
    3 × 10⁵ stars and a re-bake at every exposure step).
  - **The limits, before and after** (`DEFAULT_VIEW_CAMERA`, μ 24):
    - before (T13.d, 8e11907), the request and the cull at `limitTriple`'s triple: under `AUTO`, an
      inhibit and `MAN` −1, `DEFAULT_MAN_TRIPLE` (ISO 11,760), V 10.0578 at 60°, 11.7248 at 30° and
      13.5817 at 13°, the request cut at 11 at 30° and 13°; under `MAN` 15, 2.555, 4.222 and 6.079,
      request and cull alike. The label was the cull's, so it read `V 10.1 mag CAM` under `AUTO` at
      every EV100, EV100 15 included, where the exposure's own limit is 2.6. Before T13.c, f/1, 2 s and ISO 100 gave V 13.7 at 60°,
      the request cut at 11.
    - after, the request and the cull at every level: V 10.0579 at 60°, 11.7249 at 30° and
      13.5818 at 13°, the request cut at 11 at 30° and 13°. The label at the shown exposure, 60°:
      10.1 at EV100 −1, 10.0 at 3.5, 9.4 at 5.88, 6.3 at 10, 2.6 at 15 and −1.2 at 20.
  - **Gate.** No `just ci` (the Day 2 protocol). No `just test-render`: no shader, `view/engine/` or
    `src/smoke/` file changed, and the smoke harness draws no culled sky, so no rendered output or
    star count of the harness changes.
- **Deviations in T13.d's follow-up, as built (decision-r07-t13d, items 1–3).**
  - **One empty-text test.** `lib/textEntry.ts` gains `isEmptyEntry(text)` (empty or spaces only),
    which the `MAN` field, `CURSOR` and `NumberField` share, where the ruling has each check
    `text.trim() === ""`, so that the ship-wide rule is written once.
  - **The statement's class.** `styles.css` gains `.view-exposure__reason--consequence`
    (`flex: 1 1 0` and `contain: inline-size`) beside the reason's class. A flex item under
    `contain: inline-size` has no content width and would shrink to nothing, so `flex: 1 1 0` gives
    it the row's room beside the button, where it wraps. The held-back reasons are unchanged (no
    `contain`, as built). `ExposureCommand`'s `consequence` is typed `string | undefined`, so that
    `ExposurePanel` can pass `undefined` under `exactOptionalPropertyTypes`.
  - **The `MAN` field.** A refusal leaves the draft as it is: a typed text stays the draft, and a
    refused fill, never made one, is marked only. Leaving the field with no draft clears the
    refusal, since the only untyped refusal is a fill's. `Escape` and an empty `Enter` take the fill
    again by the same path as focus, written to the input and selected.
  - **`CURSOR`.** `Escape` drops its own field's draft only; a refusal in another field stands, and
    so does `CENTRE CHART`'s hold for it. An emptied field keeps the precision of the coordinate it
    shows.
  - **The chart's fields.** `NumberField`'s empty check comes before the link's hold, so a field
    emptied before the hold (`NO CARRIER`) arrives still returns to its value when left; `Escape`
    drops a draft typed before the hold too. Neither sends anything. The ruling does not say.
  - **Tests.** As ruled, with the `INHIBIT` description under `AUTO` and under `NO IMAGE TO METER`
    as two tests, plus six: `Escape` leaves the focus in `CURSOR`'s field; `DRIVE RANGE` emptied
    with `Enter` enters nothing; text of spaces only enters nothing in each field kind (three); and
    a `DRIVE RANGE` emptied before the link's hold returns to its value when left. The `AUTO` test
    also asserts that `INHIBIT` is not held back. Of the 22 new tests, seventeen fail at 49f029d.
    The other five pin built behaviour: `Escape` with nothing typed, the fill at −14.04, the
    operator's inhibit, `MAN`'s reason alone and the `MAN` field's line.
  - **Measured, offscreen (never shown).** `.git/rm23-scratch/r07-shading/layout/run-t13d-fu.sh`
    and `hook-t13d-fu.js` run the app with an offscreen window, as the views lane's T19.b hook
    does, so no native window exists. T13.d's harness (`run-t13d.sh`) made its hidden window on
    `:0`, which the ruling's acceptance rules out. At 1920 × 1080:
    - as built, `INHIBIT`'s row stays 32 px with the statement on one line (18 px), under `AUTO`
      (262.5 px beside the button, the column 394 px) and trapped in the wireframe (316.5 px, the
      column 448 px); the panel stands 315 and 323 px, as at T13.d. The columns are T13.d's too,
      set under `AUTO` by `ENABLE`'s held-back reason and when trapped by the panel's reading and
      reasons (T13.d's open item), none of which has `contain`: the statement does not widen it;
    - in T19.b's 26rem column the statement takes one line in 284.9 px, the row 32 px, and the
      panel stands 315, 360 and 382 px (`AUTO`, trapped, trapped with a refusal), as at T13.d: no
      added height;
    - in T19.b's 20rem column B it takes two lines, the row 44 px, 12 px more (the panel 357 px
      under `AUTO`, 408 px trapped), where the ruling estimated about 10 px. Of the 12 px, 8 are
      the reason's 0.5rem bottom margin, which the row centres with the text;
    - in the running app, `Escape` on a refused `abc` while trapped takes the fill, `6.5`,
      selected and unrefused, and leaving then shows `—`. The offscreen page never has focus, so
      the harness sends the leave's `focusout` itself.
  - **Plan text.** The ruled Risks item says a frame of exact zeros "now meters no darker than
    EV100 −14"; it read "is to meter … (T13.a's follow-up, not yet built)" until that follow-up
    landed and restored the ruled words. The T13 entry's sentence on the floor (at "a test drives
    the cut and the recovery") is the T13.a follow-up's, and is not added here.
  - **Guide.** The `MAN` (field) row is now the nomenclature table's widest, so Prettier re-pads
    the table: besides the separator and the two rows, 238 rows change in whitespace only. The two
    bullets' new sentences and the two rows are drafts for the owner.
  - **From the UX review (decision-r07-t19b-exposure-fit).**
    - Ruled (item 5; built by T19.b's follow-up): the reason's 0.5rem bottom margin, which the
      statement shares, sits it about 4 px above the button label's centre line, as it does the
      held-back reasons beside `ENABLE` and `INHIBIT`; a note in a command row has none.
    - Ruled (decision-r07-t19b-exposure-fit, item 6): `Escape` selects the restored value in every
      such field. As built here, the `MAN` field selected its fill, so typing replaced it, but
      `CURSOR`'s and the chart's fields left the caret after the restored value, so typing appended
      to it. Built by T13.d's second follow-up.
  - **Ruled (decision-r07-owner-ux-signoff, item 1; built by T19.d).** `INHIBIT` under
    `INHIBITED · OPERATOR` is held back with `NOT AVAILABLE: the exposure is INHIBITED · OPERATOR`,
    as `ENABLE` is under `AUTO`. Offered, it advertised in `--accent` a command with no effect,
    whose press showed nothing. The note takes two lines in the compact column (3 px) and at most
    three in column B.
  - **Gate.** The acceptance command (920 tests), the app's vitest (4,832), `just check lint`,
    Prettier and the console-ux skill's scripts. No `just test-render`, since no shader, `view/engine/` or
    `src/smoke/` file changed, and no `just ci` (the Day 2 protocol).
  - **Reviewed** by the TypeScript, UX and plan-conformance reviewers, with no must-fix. Fixed:
    tests of spaces-only text and of the hold's order, the chart tests' role queries, the panel's
    summary sentence, `enter`'s redundant flag dropped, and this entry's wording. The UX review's
    two points are open above.
- **Deviations in T13.a's follow-up, as built (decision-r07-t13d, item 2's guard).**
  - **As ruled.** `post/autoExposure.ts` gains `EMPTY_FRAME_CD_M2`, 2⁻¹⁷ cd/m², computed from
    `MAN_EV100_MIN` and `METER_CALIBRATION_K` (2⁻¹⁴ × 12.5 ÷ 100, exact in binary). When the
    metered mean is 0, `meteredAverage` returns max(2⁻¹⁴ ÷ the pre-exposure,
    `EMPTY_FRAME_CD_M2`). Its remarks give the crossover, an applied EV100 of −3.26, and the
    range's floor at −14, 4.5 × 10⁻⁹ cd/m².
  - **Tests.** The ruling's three, the frame of zeros as one case per start, plus two:
    - the constant, exactly 2⁻¹⁷ cd/m² and EV100 −14;
    - `meteredAverage` itself: the range's floor at 9.6 and −3.2 (as built), the crossover, and
      `EMPTY_FRAME_CD_M2` at −3.3 and −20;
    - the frame of zeros runs the closed loop, each frame's pre-exposure taken from the applied
      EV100 as `ViewDisplay` sets it. From 9.6 and from −20 it stays within 0.05 EV of −14 over the
      120 s at 60 Hz that follow a 60 s settle, and from 9.6 it is never darker than −14. At
      6f7c15c it ends at −170.4 from 9.6;
    - the faint pixel is one at 10⁻⁸ cd/m² among 99 zeros, from 9.6. It comes into range near
      −12.8, above the floor, and settles within 0.1 EV of log₂(8 × 10⁻¹⁰), −30.2. It passes
      before and after the fix, pinning what the guard must keep;
    - "recovers from a frame entirely below the histogram's range" is unchanged and passes.
  - **Noted for the orchestrator: what the guard holds at −14.** It reads the metered mean, so it
    takes any frame whose counted pixels (those the meter weighs) all lie under the histogram's
    range, light or none. Such a frame is held at −14 when its counted pixels stay under the range
    on the way there:
    - from above, when every counted pixel is below 4.5 × 10⁻⁹ cd/m², the range's floor at −14.
      Before, the range's floor stepped it darker until it came into range and metered by its
      light: a uniform 4 × 10⁻⁹ cd/m² frame settled near −24.9, drawn at about 1 ÷ 9.6 of the
      white point;
    - from below −14, when every counted pixel is below 2⁻¹⁴ ÷ the pre-exposure,
      1.2 × 2^(EV100 − 14) cd/m², since brightening lowers the pre-exposure. Before, a uniform
      10⁻¹² cd/m² frame at −20 was walked down to −36.8; now it is brightened to −14;
    - a counted pixel that comes into range on the way meters by its light, below −14 included
      (the faint-pixel test).
  - **What a held frame shows.** Its counted pixels' luminance lies 1.5 stops or more under AgX's
    floor (2⁻¹²·⁴⁷), so they are drawn black. Pixels the meter does not weigh are drawn at −14:
    the host's disc (class 0) under any meter, and the lit side and the stars under `DARK` when
    the night side is exactly 0 (no planetshine). Before, the exposure ran on without end under
    them, toward the `f32` overflow.
  - **The ruled words overstated it (amended by the orchestrator, 2026-10-05).** The ruling's "the
    image changes only for a frame whose every pixel is below 4.5 × 10⁻⁹ cd/m²" holds only with
    "counted" added and with the case from below. Its "such a frame is black at either exposure"
    holds for exact zeros alone, and only of the counted pixels. The T13 entry's ruled sentence,
    inserted verbatim, overstated it in the same way: "which leaves every frame with light in it to
    its light". It now reads "which leaves to its light every frame with a counted pixel in range
    on its way to −14" (T13.d's second follow-up). The frames it holds at −14 are over 2,000 times
    fainter than the brainstorm's darkest scene, 10⁻⁵ cd/m², and meter far darker than the view
    camera's deepest setting, f/1.4, 1/30 s and ISO 409,600 at EV100 −6.12, beyond which the
    exposure is a digital push. The exception is the frame with uncounted light, which the guard
    now bounds.
  - **Gate.** The acceptance command (85 tests), the app's vitest (4,837), `just check lint` and
    Prettier. No `just test-render`: no shader, `view/engine/` or `src/smoke/` file changed, and
    the guard changes only a frame with no counted pixel in the histogram's range below EV100
    −3.26. No `just ci` (the Day 2 protocol).
  - **Reviewed** by the TypeScript reviewer and the science-checker, with one must-fix: these
    remarks and the note above said "reached from a brighter exposure" and "every pixel", where the
    guard also holds a frame from below and reads only the counted pixels. Fixed, as are a unit in
    a test's name, one case per start, the constant's own test, a comment's tolerance and "the
    counted pixels' luminance" for AgX's per-channel floor. Every figure checked out.
- **Deviations in T13.d's second follow-up, as built (decision-r07-t19b-exposure-fit, item 6).**
  - **One helper.** `lib/textEntry.ts` gains `showSelected(input, text)`: it writes the value to
    the input, then selects it, as the ruling asks ("as the `MAN` field's `takeFill` does"), so
    that the selection survives the render that shows the same value. `CURSOR`'s fields,
    `NumberField` and the `MAN` field's `takeFill` share it. The `MAN` field's behaviour and tests
    are unchanged.
  - **`CURSOR`.** `Escape` selects its field's coordinate whether or not anything was typed there.
    `Enter` selects it when that field's own text was empty or spaces only; it still enters the
    other fields' drafts, as built.
  - **The chart's fields.** The same in `NumberField`, whose empty check comes before the link's
    hold as built: an emptied field under `NO CARRIER` takes `Enter`, enters nothing and is
    selected, and `Escape` selects while held back too. The ruling does not say.
  - **Tests.** The ruling's three, as four tests (`X` refused then `Escape`, `Y` emptied then
    `Enter`, and `DRIVE RANGE` both ways), plus four: `Escape` with nothing typed selects the
    whole value wherever a click left the caret (`Z`, `CHART TIME`), and leaving after `Escape`
    enters what was typed then, not the value with it appended (`Z` 3, where ca91a18 entered
    12.03; `CHART TIME` 3, where `+12.503` entered 12.5). All eight fail at ca91a18.
  - **Plan 05's pointer.** Besides the ruled ", selected", its heading names this ruling and both
    of T13.d's follow-ups, so that it does not credit the selection to the first.
  - **Plan text.** The T13 entry's ruled sentence on the meter's floor is amended, the
    orchestrator's ruling on T13.a's follow-up's open point; that follow-up's deviations record it.
  - **Gate.** The acceptance command (928 tests), the app's vitest (4,845), `just check lint`,
    Prettier and the console-ux skill's scripts. No `just test-render`: no shader, `view/engine/`
    or `src/smoke/` file changed. No `just ci` (the Day 2 protocol).
  - **Reviewed** by the TypeScript and UX reviewers, with no must-fix or should-fix. The UX
    review's one point is open below.
  - **Ruled (decision-r07-owner-ux-signoff, item 5; built by T19.d).** The bullet reads "… has a
    way out that enters nothing and is never refused: `Escape` … shows the field's value again,
    selected so that typing replaces it; `Enter` in the emptied field does the same; left emptied,
    the field shows its value." It no longer requires a selection on leaving, and "never refused"
    covers all three acts.
  - **By hand, for the owner** (a focused, visible window; jsdom gives only the selection's
    offsets): `Escape` and an emptied `Enter` in each of the five fields, the value highlighted in
    `--accent` and replaced by the next key, also after a click that left the caret inside the
    value, at 1280 × 720 and 1920 × 1080; what a screen reader says when `Escape` restores and
    selects the value.
- **Deviations in T9, as built (mesh bodies).**
  - **Files.**
    - `bodies/smoothMesh.ts`: `SMOOTH_MESH_TAU_PX`, `smoothMeshTauPx`, `MAX_SMOOTH_MESH_PATCHES`,
      `SmoothPatch`, `SmoothMesh`, `poleAxes`, `smoothMeshOf`, `packSmoothMeshes`,
      `rotationColumns`, `LimbDepths`, `DISC_LIMB_DEPTHS`, `limbDepths`, `limbDepthAt`, and the
      `f64` twin's `smoothPatchVertices`, `MeshRaster` and `rasteriseSmoothMesh`.
    - `shaders/smoothMesh.wgsl`, as `SMOOTH_MESH_MATERIAL` (`BODY MESHES`) in `draw.ts`,
      registered in `WGSL_CATALOGUE`.
    - Not named by the plan: `bodies/frameTwin.ts` (`compositeBodyFrame`, the twin of the
      `bodies` and `discs` passes with each body's share of every pixel; `firstHitShares`, the
      first-hit oracle); `view/scenes/occultationScene.ts` (the scripted occultation, not in the
      `SCENE` selector); `smoke/meshBodies.ts`.
    - `regime.ts` gains `sphereFootprint`: the circle through the corners of the clamped
      `sphereScreenRect`, `null` for a sphere off the view or wholly behind the near plane, where
      the rectangle is the whole view and would promote every disc.
  - **The shaders split.**
    - `bodyDisc.wgsl` is now a library, ending in `disc_pixel(position, edge_pass)`; the disc's
      `Draw` and entry points are in `bodyDiscDraw.wgsl`, whose `Draw` gains `depths` (`vec4f`,
      the rectangle's corner depths, 0 for a disc body). R08's and R10's shading edits still go
      to `bodyDisc.wgsl`.
    - R05's `terrain.wgsl` gives `SlotRecord` and the `FaceDifferences` arithmetic, unchanged, to
      `terrain/shaders/patchVertex.wgsl`, which the terrain materials and the smooth figure both
      compose (R05's Risks, T11.b as built). The terrain's programs are the same.
  - **R05's geometry, over its public names** (R05 built `planetGeometry(figure, null)`, so no
    stand-in): `selectPatches` on it (one geometry per figure's radii, so that selection's bounds
    memo holds), `patchTerms(key, figure, 0)`, `writeSlotRecord`, `InstanceRecords`,
    `patchMeshData`, `chordSagittaM`, and `terrainPass.ts`' `morphRangeM` and `effectiveTauPx`.
    The zero-height morph target is written in `smoothMesh.wgsl` (R05's `morphOffset` reads
    heights). Skirts hang 2 × `chordSagittaM` of the parent level plus 2⁻²⁰ of the origin's
    distance (the science check: the gap is at most 1.94 × it, with a morphing coarser
    neighbour). No worker.
  - **Its own tolerance.** Selected at `SMOOTH_MESH_TAU_PX` = ¼ px at the view's corner on both
    settings (`smoothMeshTauPx`: ¼ ÷ sec²θ at the centre pixel's scale, Snyder 1987's radial
    scale of the gnomonic projection; 0.174 px at 1080p across 60°), not the terrain's τ, so that
    the figure covers every pixel the spheroid covers wholly (its centre ½ px or more inside the
    limb). At most 1,024 patches (`SmoothMesh.limited`, which no caller acts on); 50 patches,
    the deepest at level 14, for a camera 2 m above an Earth at 1080p.
  - **Axes about the pole.** The figure is built in `poleAxes(record.pole)`, not the body's
    rotation: it is symmetric about the pole, so its patches need not turn with the body, and
    selection does not change while it spins. R10, which hands over by patch with heights, builds
    its own in the body-fixed rotation.
  - **The figure gives coverage and depth; the light is the disc's** (the plan's "same shading
    functions"). Each fragment draws its pixel as the disc's first draw does (`disc_pixel`, edge
    pass 0), from the body's own disc record and the pixel's rays against the analytic spheroid,
    where the spheroid covers the pixel wholly, and is discarded elsewhere, writing no depth. The
    limb is the disc's second draw (`BodyStep` `limb`) at the body's place in the painter's
    sequence, depth-tested and writing none, its rectangle's corners on the limb's polar plane
    (`limbDepths`, the plane's reversed depth, affine on the view, unclamped so that the depth clip
    removes what sees the plane behind the camera). So a promoted disc and its mesh draw the same
    light, at every size from the 3.3 px switch up. A rasterised mesh shaded per vertex or facet
    would alias its limb by up to ±10% of a 3 px body's flux and sit inside the limb by its sag.
    A mesh body's time is split between `bodies` (its figure) and `discs` (its limb), not Design
    note 8's one pass.
    - A pixel both of the disc's draws take (a corner within `LIMB_OVERLAP_PX` of the limb) keeps
      the figure's opaque light: the limb, on its plane, lies behind the figure there. On the GPU
      that is within 0.122 of the texel tolerance of the disc's (a Saturn-like disc 64 px across).
  - **Promotion and the frame.**
    - `planLitBodies` calls `promoteOverlapping` with each body's `sphereFootprint` and the new
      optional `BodyFrameOptions.depthWriters`; the new required `BodyFrameOptions.setting` goes
      to selection. `BodyFramePlan` gains `meshes` (`MeshBodyPlan`: the record's index, the figure,
      the limb's depths); its `order` is the order the steps follow, a mesh body in it as a disc
      for its limb, while `painterOrder` still leaves meshes out.
    - `PhotorealFrame` gains `depthWriters`. The view passes none: nothing in a view writes depth
      yet (R10's terrain, lit craft), so the live client never promotes and draws as before. This
      replaces T8.a part 2's "Promotion … is not called".
    - The renderer submits `bodies` (loading the sky pass's colour and depth) between the sky and
      `discs`, only on a frame with a mesh body. `LitBodyRenderer.meshDraws(plan)` is one
      instanced draw a body (its `firstInstance` in its `Draw`); a plan's records are written once
      for both calls, and again after a device loss.
  - **Known limits.**
    - A mesh body behind a host star shows over the star's disc: R06's disc lies at depth 0, which
      the figure's depth hides (a disc body is ordered by the painter). It needs the body beyond a
      star some pixels across, over depth-writing geometry. Putting R06's disc on its sphere's
      polar plane, as the limb is, would fix it; that is R06's change (ruled below). _Resolved
      2026-10-06: R06's disc now lies on that plane; see "The star's disc at its limb's depth"
      below._
    - Where two limbs cross a pixel, the limb's blend over what is beneath keeps T8.a's
      coverage-over error: up to 0.115 of the pixel in the occultation.
    - Each mesh body runs `selectPatches` every frame, and its fragments discard under a depth
      write, which loses early-Z on many GPUs: both for T17's bench.
  - **Tests.**
    - `bodies/smoothMesh.test.ts`: the patches are R05's at h = 0 with origins on the datum; the
      vertices are R05's `f32` arithmetic at h = 0 to `f32`'s step; the outline at most 0.004 px
      inside the limb for an Earth 100 px across, 0.020 px for a Jupiter-like giant from 1.1
      radii (refined to level 3), 0.003 px in a 120° view's corner, and never outside, against
      ¼ px inside and ½ px outside; the twin's flux as a mesh against the disc's, mesh ÷ disc − 1
      at most 2 × 10⁻¹⁶ at 3.3, 6, 40 and 64 px (a Saturn-like giant among them), with no hole;
      the limb's depth on the plane for a Saturn-like giant and from 400 km up, where the horizon
      lies 2,290 km off and two corners see the plane behind; the budget from 2 m up; the records.
    - `view/scenes/occultationScene.test.ts`, the 24 scripted steps against a 16 × 16-ray oracle:
      the planet a mesh throughout, the moon promoted at step 3 and never back; no pixel a pixel
      or more from both limbs where the moon shows other than the oracle says; each body's share
      of a pixel on one limb within 1/16 (worst 0.023), on both within 0.25 (worst 0.115); the
      moon's visible area within 0.22 px²; the same light as the painter's frames with both
      bodies discs, to 10⁻⁹.
    - `draw.test.ts`, `renderer.test.ts` and `regime.test.ts`: promotion, records, limb order,
      the draws, the records written once, the `bodies` pass, and the footprint.
    - `just test-render` (SwiftShader, `default` and `no-subgroups`, 2026-10-05): every check
      passes (462 each), R05's terrain frames among them after the split. The promoted Earth 20 px
      across, the Saturn-like giant 64 px across and an Earth from 400 km draw the disc's texels
      and classes (within 0.122 of the tolerance) and its flux to 0.0000%, the twin's colours
      within 0.680 of it. Over the occultation every step is within 0.592 of the twin and 0.000 of
      the painter's frames, with their classes, and no pixel more than half the moon's where no
      oracle ray meets it.
  - **By hand, for the owner** (ruled below): `just test-render --variant=default --captures=DIR`
    writes `r07-t9-occultation-{mesh,disc}-NN-default.png`, ten steps of the occultation each at
    512 × 384 through the photorealistic frame, as meshes and as discs. On SwiftShader
    (2026-10-05) the two series are identical to the code at every step.
  - **Ruled (orchestrator, 2026-10-05), on T9's two open points.**
    - The by-hand occultation is recorded from the `just test-render --captures` frames: option
      (a), the captures as the record. The owner looks at the PNGs. (The live `VIEW` never
      promotes, since nothing writes depth, so `just client` cannot show the mesh path; (b), a
      kept occultation scene with a synthetic depth writer, and (c), waiting for R10, were not
      taken.)
    - A mesh body behind a star shows over the star's disc, because R06 draws the disc at
      infinite depth: a stated limit for RM3. No live view promotes a mesh until R10 brings depth
      writers, and a follow-up for R06 to draw star discs at their limb plane's depth is queued
      before R10. _Resolved 2026-10-06, before R10: the follow-up is built; see "The star's disc
      at its limb's depth" below._
- **Deviations in T10.a, as built (retarded lighting geometry, decision-r07-t8a, follow-up (a)).**
  - **Ruled: the local body is lit at its retarded time** (the orchestrator, 2026-10-05, under the
    owner's delegation, on the plan-conformance and science reviews). Decision-r07-t8a (a) gave
    the local body, drawn at the present, `lightTimeS` 0. Built instead: the local body is still
    drawn at its present `geometricM`, but is lit like every other body, from its `emittedM`, its
    velocity then and its real light time. The reasons:
    - The brainstorm's rule ("The floating origin is already in the simulation"): every
      time-varying state is drawn at its retarded time, the local body's included, eclipse
      contacts among them. Lit at the present, its contacts came τ = |r_cam − r_B| ÷ c early: up
      to 5 s on the Earth, 168 s inside Jupiter's Hill sphere (the frame rule's, at pericentre,
      5.05 × 10¹⁰ m).
    - It keeps δ in [0, 2 |r_X − r_B| ÷ c] for every lit body, so that the linear retardation's
      ½ a δ² stays about 3 m for Io. With τ_B = 0, δ fell to −r_Hill ÷ c. Io was then 9.5–9.9 km
      off with the camera 5 × 10¹⁰ m from Jupiter. A close moon of a giant on a wide orbit (a
      Jupiter mass at 30 au, the moon at 2.5 R_J) was some 2,000 km off, about 210 km of it across
      its shadow's axis at a central transit and up to about 1,000 km near the limb. All of it
      moved with the camera.
    - The local body's velocity is exact. Its emitted velocity no longer stands in for the present
      one.
  - **For R10 (a pointer, also in R10's Risks).** The local body's terrain is drawn at the present
    and lit at T − τ. Once R10 adds rotation, the terrain sits ωτ ahead of its lighting: about
    2,100 km at a Jupiter's equator from its Hill sphere's edge (τ 168 s, v_eq 12.6 km/s), about
    38 km at τ 3 s.
    That is the brainstorm's own split between geometry and time-varying state. R10 decides
    whether to draw rotation-dependent state at the retarded time too.
  - **Files and names.**
    - `lib/scene/apparent.ts`: `placed` entries and `SceneStarFrame` gain `emittedM`
      (`apparentPosition`'s `geometricThenM`) and `emittedVelocityMPerS`. The velocity is the
      composed orbits' own at `emitted` (`stateAt`'s, through the private `composedVelocity` the
      ship observer already used), not the ruling's track central difference over ±1 s: it is
      exact, and costs one chain of Kepler solves rather than two.
    - `view/scene/model.ts`: `RetardedCentre`, `staticRetarded(centreM)` and the required
      `ViewBody.retarded`. Every kept scene, and `aBody`, takes `staticRetarded` at the drawn centre
      (`aBody` at an overridden centre too). `isLitKind` moved here from `displays/view/viewRun.ts`,
      so that the view's "lit" has one definition.
    - `view/scene/fromServer.ts`: every star's and placed body's retarded centre, the local body's
      included, is its `emittedM`, its velocity then and `spanSeconds(lightTime)`; a contact's is
      `null`.
    - `lighting/retarded.ts`: `RETARDATION_STEPS`, `retardedFrom`, `LightingFrame`, `isLitBody`
      (`isLitKind` with a radius), `lightingFrameOf` (`null` for a body that is not lit),
      `LitBodyLighting` and `lightingFramesOf`. The last gives every lit body, in the scene's
      order, with its drawn centre and its frame, the drawn centres found once.
      `LightingFrame.occluders` are `LightingBody`s, which carry the identifier `occludersFor` and
      planetshine need: a subtype of the ruling's `LightingSphere`.
    - `lightingBodyOf` (T6.c) is removed. No view called it, and it would have lit the local body
      by a rule of its own.
    - `test/sceneFixture.ts` gains `SCENE_CHARTED_PLACE`, `sceneModelOf`, `systemSceneState`,
      `shipFrameOf`, `viewSceneOf` and `viewBodyOf`. `fromServer.test.ts` now uses them.
  - **Two fixed-point steps, not one.**
    - One step from the source's retarded centre leaves the light time wrong by (v ÷ c) δ, and the
      position by v² δ ÷ c. That is up to 8 m for the Moon, seen from far past it when the
      Earth–Moon line lies along the Earth's motion, against the ruling's 1 m test (tested at
      3,000 s, above 1 m; at the eclipse, where the line runs across the motion, 0.23 m).
    - Two steps leave v³ δ ÷ c², under a millimetre. The brainstorm's `retarded_in_system`
      iterates for the same reason.
  - **The translation.** The ruling's drawn(B) + (retardedFrom(B, X) − B's retarded centre) is
    computed as drawn(X) + ((retarded X − drawn X) − (retarded B − drawn B)). That is the same sum,
    the camera's offset being a translation, and it is drawn(X) exactly where nothing moves.
    `retardedFrom` returns a source at rest exactly where it is. So a kept scene lights bit for bit
    as before: tested on the phase, precision, frame-change and descent-spike scenes, and through
    `planLitBodies` on the phase scene.
  - **How the plan takes it.**
    - `LitBodyInput` and planetshine's `ReflectingBody` gain the required key
      `lighting: LightingFrame | undefined`, so that every builder states its choice (the
      TypeScript review).
    - A body with a frame takes its stars (`lightsOf`, so `pointFlux` and the disc record), its
      occluders (`occludersOf`) and its planetshine neighbours' places from it.
    - `undefined` states a static scene's geometry: the frame's hosts and the other bodies where
      they are drawn. The smoke harness, the occultation scene and the bodies' tests state it.
    - `placeLights` stays per scene, at the drawn centres, for the painter's order (`HostSphere`),
      and `PhotorealFrame.lights` is that. The ruling's "per lit body" is `lightingFrameOf`'s own
      `placeLights` call.
    - `litBodiesOf(scene, pose, discs)` maps `lightingFramesOf`'s entries, so every lit body has
      its frame by construction. `litLabelsOf` filters by `isLitBody`.
  - **Neighbours.** A planetshine neighbour N stands where the lit body's frame puts it, at
    t_B − |r_N − r_B| ÷ c. Its stars, and the larger bodies that shadow it, are those of its own
    frame, at its own retarded time, δ before the exact time. _R07.T10.b: bodies of every size
    shadow it, from the same frame._
    - That keeps each neighbour's starlight one evaluation a frame (T11's "once a frame").
    - It turns the neighbour's star by at most |v_N − v★| δ ÷ d★, with δ ≤ 2 |r_N − r_B| ÷ c:
      up to 6 × 10⁻⁷ rad for the Earth lighting the Moon.
    - It moves the neighbour's own eclipse by up to δ: 2.8 s of Io's 254 s ingress.
    - A neighbour the frame does not hold, a contact, lights nothing.
  - **The neglected terms, as held.**
    - The Moon's ½ a δ² takes its barycentric acceleration, 2.9–9.0 × 10⁻³ m/s² (the Sun's
      5.9 × 10⁻³, plus or minus the Earth's 2.5–3.1 × 10⁻³, apogee to perigee). That gives
      0.8–3.1 cm; the ruling's "1 cm" is the geocentric figure. Measured 1.2 cm at δ 2.63 s, and
      held under the bound and under 3 cm.
    - Io is held within ½ a δ², taking the track's own pull at Io's pericentre (μ = 4π² a³ ÷ P²,
      0.09% above GM_J, which P's J2 carries) plus the Sun's, and under 3 m, both with Jupiter the
      local body (5 × 10¹⁰ m off, measured 2.82 m at δ 2.81 s) and not.
    - The lit body's own aberration: the Earth's perihelion speed, 30.29 km/s, gives
      1.01 × 10⁻⁴ rad and 0.64 km at its limb (the ruling's "10⁻⁴ rad (0.6 km)", rounded).
    - The star's reflex motion stays `illuminance.test.ts`'s 4.16 × 10⁻⁸ rad: the placements hold
      a lone star at the barycentre (the test is renamed for it).
  - **Figures.** The Earth–Moon cases are at the fixture's partial eclipse (t 4,727,768 s, the
    axis 7,166 km from the Earth's centre, with the Moon's light time to the Earth). The suite's
    Io cases run at that time too, where Io's shadow is off Jupiter. The Jupiter–Io probes are at
    a transit (t 4,826,618 s, the axis 52 km from Jupiter's centre, 45 km without Io's light
    time), with Io put in Jupiter's orbital plane.
    - Tested (`retarded.test.ts`): `retardedFrom` within 1 m of the exact track from four cameras
      (beside the Earth, twice the Moon's distance past it, eight times past it, a tenth of an au
      across), measured 2 × 10⁻⁵ m to 1.2 cm. The Moon's shadow axis on the Earth does not move
      (0 m) when the camera's velocity changes by 30 km/s; from the drawn centres it moves 77 and
      35 km for the far cameras (asserted above 10 km). From a camera beside the Earth and one far
      past the Moon, at one t_B, it lies within 1 km of the exact tracks and of each other
      (measured 15 µm to 0.2 mm).
    - Probe, 2026-10-05, not in the suite: with the camera moving with the Earth (29.8 km/s), the
      sunlight's direction at the Earth and the Moon turns by 0.90–1.00 × 10⁻⁴ rad from T8.a's
      drawn placement, and the earthshine's at the Moon by 9.95 × 10⁻⁵ rad. The Moon's shadow axis
      in T8.a's placement is 0.62–3.27 km off the exact, and in T10.a's at most 0.2 mm off.
    - Probe, at rest: T8.a's placement put the Moon's shadow axis 0.83, 2.28, 77.3 and 38.2 km off
      for the four cameras. A 30 km/s change moved it 0.92, 1.77, 77.1 and 35.1 km. A camera at
      rest in the barycentric frame sees no aberration, so the sunlight does not turn.
    - Probe, Jupiter–Io, with the camera moving with Jupiter (12.4 km/s): the sunlight at Jupiter
      and Io turns by 3.5–4.2 × 10⁻⁵ rad and the Jupiter-shine at Io by 4.2 × 10⁻⁵ rad. Io's
      shadow axis on Jupiter moves 11.4 km for a camera beside Jupiter and 46 km for one
      6 × 10¹⁰ m past Io, from T8.a's placement.
    - Probe, Jupiter–Io, with a camera beside the Earth moving with it: the sunlight at Jupiter
      turns by 2.7 × 10⁻⁶ rad, the Jupiter-shine by 8.7 × 10⁻⁶ rad, and Io's shadow moves 13.8 km.
    - Probe, at rest beside Jupiter: 11.6 km, almost all from the ruled local-body change (Jupiter
      drawn at the present, lit 0.67 s back, Io moving at 17 km/s).
  - **Io's umbra.** T8.a's and the ruling's "an umbra of 3,600 km" is Io's diameter (3,643 km).
    At Jupiter's cloud tops the umbra is about 3,000 km across and the penumbra about 4,300 km
    (science check, 2026-10-05).
  - **Cost.** One retardation per pair of lit bodies a frame, O(n²) beside `occludersFor`'s, for
    T17's bench.
  - **Tests.** `lighting/retarded.test.ts` (24). New cases in `apparent.test.ts` (2),
    `fromServer.test.ts` (4: the local body, another body, a star, a contact), `model.test.ts`
    (2), `planetshine.test.ts` (4), `draw.test.ts` (6) and `photorealFrame.test.ts` (2).
    `occluders.test.ts` loses `lightingBodyOf`'s three.
  - **`just test-render`** (SwiftShader, `default` and `no-subgroups`, 2026-10-06): every check
    passes (249 and 247), and T9's 34 default captures are byte-identical. Nothing drawn moves:
    the smoke harness and the kept scenes light statically. Under load (averages of 10 to 40) the
    photorealistic frame's histogram check twice found no read-back within its 5 s wait; on a
    quieter machine it passes (226 weighted counts).
- **Deviations in T10.b, as built (a body's eclipse over its disc, decision-r07-earth-albedo, Q3).**
  - **Files and names.**
    - `lighting/discEclipse.ts`: `discEclipseVisible(star, body, occluders, towards, share, k)` and
      `EclipsedBody` (a centre and a figure, taken as the sphere √(a c)). `star` is the
      `PlacedLight`, whose B, V and R limb laws give the result per display channel (r, g, b): one
      call does all three, since the quadrature in θ is the same for every channel.
    - `lighting/annuli.ts`: `occultationFrom` and `Occultation`, `eclipseVisible`'s geometry for one
      occluder (bit-identical), so that each node's term is `eclipseVisible`'s arithmetic with the
      annuli made once.
    - `hostAnnuli` moved from `bodies/draw.ts` to `lighting/hostLights.ts`, since
      `discEclipse.ts`, which `draw.ts` imports, needs it.
    - `lighting/quadrature.ts`: `gaussLegendre` and `GaussLegendreRule`, moved out of the oracles,
      which the renderer never imports.
    - `lighting/oracle.ts`: `discEclipseBruteForce` and `FarView` (each view carries its whole,
      `totalM2`), the test's oracle.
  - **A uniform disc.** `annulusEdges` gave a law with c = 0 annuli of no area, 0 ÷ 0 on the CPU and
    in the shader. It now gives K annuli of equal area and flux there; nothing changes for c > 0.
  - **The point's occluders.** `pointFlux` takes the two that `occludersOf` keeps
    (`MAX_DISC_OCCLUDERS`, the largest as seen from the body), not all of `occludersFor`'s list.
    They are the disc record's two, so the point and the disc it becomes at 3 px leave out the same
    shadows: a third, as in a triple transit on Jupiter, about 0.1% each. Planetshine's `shadowing`
    is the whole list.
  - **The quadrature, as built.**
    - **Depth.** Each node's eclipse term is taken at its ring's w-weighted mean depth, not in the
      centre's plane. In the centre's plane alone the error reaches 1.6 × 10⁻³ against the oracle,
      over the task's 10⁻³, for a Moon-sized body 4 × 10⁸ m behind its occluder at 1 au. With the
      mean depth the cone's spread enters at second order. The task's "below R★ R ÷ d" for that
      spread is about (R★ + R_o) R ÷ d (science check, 2026-10-06).
    - **The view's edge.** ρ is also split where the rings meet the edge of the far point's view
      (μ = 0, a half-ellipse on the disc): at its two ends on the rim and at its points nearest and
      furthest from the axis (32 samples and bisection). Without these splits phases of 60° to 120°
      erred by up to 4 × 10⁻³. An arc is split where μ crosses 0 (24 samples and 30 bisections). A
      ring outside the edge's distances from the axis is wholly in view or wholly out, and is not
      sampled.
    - **Rules.** Gauss–Legendre under the substitution x = (1 − cos πt) ÷ 2, 8 nodes a piece of ρ
      and 16 an arc of θ.
    - **The exact 0 and 1.** These come from the exact tangent cones, (R_o + x sin γ) ÷ cos γ,
      with a margin for the axis's drift across the lit depth. The umbra's least is taken over the
      lit depths, since an occluder larger than its star has an umbra that widens behind it.
  - **The equivalent sphere errs at first order in f, not second.** The ruled Risks line (above,
    "The disc-averaged eclipse") says "at the second order in f". Under Lambert at zero phase a
    spheroid seen equator-on has m(0) = ⅔ (1 − ε ÷ 10 + …), ε = a² ÷ c² − 1 ≈ 2f. So a small central
    shadow's share is off by about f ÷ 5: 0.104% on the sphere against 0.106% for Io's on Jupiter.
    An ingress across a larger shadow is off by up to f ÷ 2 in its length (science check,
    2026-10-06). The code keeps the sphere; the doc comment says first order. The ruled line is
    left for the orchestrator. _Ruled 2026-10-06: the line is reworded to first order (applied
    with R07.T10.c); integrating on the spheroid is not built._
  - **Figures tested.**
    - **Against the oracle.** The oracle takes V at each of 10⁶ surface points, in (θ, φ) about the
      star's direction. A grid in the projected plane erred by 3.9 × 10⁻⁴ at the rim, where w jumps.
      Over radius ratios 0.02–30, three separations, 0°, 60° and 120° and L 0 and 1, the worst error
      is 4.9 × 10⁻⁵ in V. Wherever the eclipse takes over a thousandth of the light, the error is
      within 0.25% of what it takes (tested at 1%). Each channel is pinned to its own law to 10⁻⁶.
    - **The central solar eclipse.** A full Lambert Earth keeps 0.8922 of its light towards the
      Moon, under a uniform Sun at 1 au; the oracle agrees to 3 × 10⁻⁷, and so does the science
      check's own (0.89220). The ruling's 0.893 matches the same eclipse in parallel light, 0.89272
      with the Sun at infinity at its angular radius. The ruling's script is not kept, and its
      first-order 0.888 has no (1 + x ÷ d)². At 1 au the shadow cone widens over the 384,400 km by
      (1 + x ÷ d)², 1.005 in area. Both are tested. The ruling's 10.7% is thus 10.8% at 1 au (10.80%
      under `sunLikeHostDisc`, which planetshine loses there). From the Moon's own distance, each
      element of Earth weighted by its inverse square, 11.0% (science check): a stated error of
      planetshine's far point. _Ruled 2026-10-06 (applied with R07.T10.c): T10.b's test line now
      reads "0.892 at 1 au (0.893 in parallel light)", and the ruling's 10.7% in T11's
      deviations reads 10.8%._
    - **A point Jupiter in Io's central transit.** Seen from 5° of phase it keeps 99.896%, not 0: in
      the g channel 3.019 × 10⁻⁵ of 3.022 × 10⁻⁵ lx, Jupiter at 5.2 au seen from Earth at
      opposition. The share taken is 1.5 (R_Io ÷ √(a c))² (1 + x ÷ d)² = 0.104%. The ruling's "at
      most 0.10%" is its own (1,821.6 ÷ 69,134)² × 1.5 = 0.1041%, rounded
      (decision-r07-earth-albedo, Q2). The task's 99.9% floor comes from that rounding, so the test
      holds the point at or above 99.89% and the share to 1%. The 2026-10-04 probe's geometry,
      5.16 × 10⁻⁵ lx clear, is not recorded.
    - **Io entering Jupiter's shadow.** Io's radius is NASA's 1,821.5 km and its speed
      2π a ÷ P = 17.338 km/s, on a straight path across the axis. Seen from the Sun's side, the fade
      from 1 to 0 takes 252.5 s against (2 R_Io + w_p) ÷ v = 253.6 s (w_p 754 km). Its last second's
      light lies under the quadrature's 10⁻⁶ and rounds to 0. Its centre fades in 43.5 s.
      - From 17° of phase one contact's sliver lies on the far side, which one depending on which
        way the far point leans along the track: 248.9 s and 246.4 s are seen, 250.5 s with the
        lean out of the plane.
      - Io's real path meets the shadow 9.8° from opposition, more slowly across it, and takes about
        257 s (science check).
    - **The 3 px switch.** Through an Io-like ingress at 30° and 90° and three depths, the disc's
      summed pixels meet the point to 0.20% under the provisional Lambert law and 0.16% under a
      lunar law. T8.a's Moon in Earth's penumbra now agrees to 2 × 10⁻⁵, where it was allowed 2%;
      it is tested at 1%.
  - **Planetshine.**
    - `LitNeighbour` gains `shadowing` (its `occludersFor` list, of every size, in the neighbour's
      own frame) and `annuli`. Its `lights` are uneclipsed, and so is `boundLx`.
    - `planetshineSources` cuts each candidate's lights by `discEclipseVisible` towards the body
      (`eclipsedLights`). It does so only for a candidate past the bound that has shadowing bodies,
      and the cut light ranks it as well as lighting the body.
    - `towards` comes from the body's frame. The neighbour's stars and the bodies that shadow it
      come from its own frame, as T10.a placed them, with T10.a's stated δ (2.8 s of Io's 254 s).
    - The handoff's alternative, the pair's occluders from the lit body's frame, was not taken.
      That frame's occluders leave out the lit body itself, which is the occluder of a solar
      eclipse. They are also retarded to the light reaching the lit body rather than the neighbour,
      so they would not line up with the neighbour's own stars. It would also cost an
      `occludersFor` a pair.
    - The "larger than the neighbour" filter is gone, so a solar eclipse takes its share and equal
      moons shadow each other (tested).
  - **Tests.** `lighting/discEclipse.test.ts` (19), new cases in `annuli.test.ts` (the uniform
    disc), `planetshine.test.ts` (3) and `draw.test.ts` (3). Changed:
    - Earthshine at full Earth leaves the Moon out of the lit bodies, as its sibling test leaves
      Earth out at full Moon, since the Moon's own shadow falls there.
    - Jupiter-shine on Io takes Io's shadow off, 0.104%, before meeting the closed form to 10⁻³.
    - T8.a's disc and point in a penumbra are held to 1%, not 2%.
  - **`just test-render`** (SwiftShader, `default` and `no-subgroups`, 2026-10-06). Every check
    passes (249 and 247), and the 68 captures are byte-identical to T10.a's: no captured scene has
    a point in a shadow or a shadowed neighbour. T11's smoke check sets its neighbour on the
    anti-solar line, so its earthshine now carries the Moon's own shadow, on the GPU and in the twin
    alike. It still meets the twin within 0.258 of the tolerance.
  - **Cost.** A call (one light, one occluder, all three channels) took 0.15–0.2 ms at a load
    average of 36. It is made only where the occluder list is not empty. Provisional, for T17's
    bench.
  - **Unchanged.** The disc's starlight eclipse term, the records' layout and every shader. A
    disc's planetshine sources (`DiscRecord.secondaries`) now carry the neighbour's eclipse over its
    disc, as the point's do, and a uniform star's annuli are no longer empty.
  - **For R10 (a pointer, also at R10.T10.f).** R10.T10.f's "takes its eclipse term from the
    centre, as now" is stale. The point's eclipse is now `discEclipseVisible`, the average over the
    disc. T10.f keeps it, so that the point meets the disc at 3 px through an eclipse. Passed to
    the orchestrator for R10's owner.
- **Deviations in T10.c, as built (the eclipse scene).**
  - **Files and names.**
    - `view/scenes/eclipseScene.ts`: `ECLIPSE_SCENE_NAME` (`ECLIPSE TEST`); the bodies
      `ECLIPSE_STAR`, `ECLIPSE_PLANET`, `ECLIPSE_MOON` and `ECLIPSE_GIANT` (indices 0 to 3);
      `ECLIPSE_TIME_RATE` (100), `ECLIPSE_DURATION_S` (270), `ECLIPSE_MID_S` (10,100 s of the
      scene's clock) and `ECLIPSE_CONJUNCTION_S` (20,500); the radii and orbits;
      `ECLIPSE_CAMERA_OFFSET_M`, `ECLIPSE_CAMERA_POSE` and `eclipsePoseAtStar(sceneS)` (the camera
      turned to the star); `eclipsePlaceAt(body, sceneS)` and its `EclipsePlace`, the exact tracks
      the tests' oracles take; `eclipseSceneAt(sceneS)` and `eclipseScene()`.
    - `displays/view/viewRun.ts`: `SCENE_OPTIONS` gains it, after `PHASE TEST`.
    - `smoke/eclipse.ts`: `checkEclipse` (three checks) and `captureEclipse` (three series), run by
      `smoke/page.ts`; `scripts/testRender.sh`'s comment names the captures.
    - Doc comments only: `ViewBody.retarded` (`view/scene/model.ts`), `retardedFrom` and
      `placedFor` (`lighting/retarded.ts`) and the kept scenes' lighting test (`retarded.test.ts`)
      now say "a kept scene at rest", since this scene's bodies move.
    - Unchanged: `view/photoreal/renderer.ts` and `displays/view/photorealFrame.ts`, which the views
      lane's T16 also edits. The smoke file and the tests call `photorealFrame` as the view does.
  - **One scene, its clock at ×100.** The scene's clock runs `ECLIPSE_TIME_RATE` (100) times the
    script's (`timeRate` 100). A planet takes hours to pass behind its star (12,168 s for this
    giant), and a moon's penumbra about 2 h to cross a point (6,914 s). At ×100 the 270 s script
    holds the shadow's whole crossing of the planet's disc, the eclipse at the camera and the
    giant's passage, and starts and ends with every body clear. It is the first kept scene whose
    clock is not 1 and whose bodies move.
  - **The bodies.**
    - A Sun-like star at rest at the barycentre (`sunLikeHostDisc`).
    - An Earth-sized planet at 1 au.
    - A Moon-sized moon on a circle of 363,300 km about the planet, the Moon's perigee distance,
      so that its umbra reaches the planet: a total eclipse, the umbra 155 km and the penumbra
      6,811 km across the shadow's axis (25.07 d a turn, against the Moon's 27.32 d at its mean
      distance).
    - A Jupiter-sized giant on a circle of 0.05 au, a hot Jupiter's (P 4.08 d).
    - The orbits are circular, coplanar and prograde, each at √(G(M + m) ÷ r³), from IAU 2015
      Resolution B3's nominal GMs and DE430's lunar GM. So every new moon is an eclipse.
    - The planet does not turn, and its reflex about the planet–moon barycentre (4,414 km here,
      4,671 km for Earth's) is left out. Every body takes T2.a's provisional photometry, as every
      scene body does until T2.b.
  - **A free camera in the planet's frame stands for the ship.** The plan's "ship in a moon's
    penumbra" is the camera at the ship's place. The scene has no own ship: the test hull's
    windscreen plate, 1 m square and 1 m ahead of the seat's eye, would stand across a seat's view
    of the eclipse. The free camera starts 20,000 km above the point beneath the star at
    mid-eclipse, held in the planet's frame (it stays there through the script, tested), looking
    down. With the star as its target it turns to the star.
  - **"A camera crossing a moon's shadow", as built.** The camera hovers and the shadow crosses it.
    The shadow moves across the planet's disc from the view's left to its right at 0.985 km/s in
    the planet's frame, of the scene's clock (98.5 km/s of the script's). It is on the planet from
    about 1 s to 200 s of the script, on the part the camera sees from 3 s to 198 s. Its cone
    crosses the camera at 0.989 km/s: the camera is in the penumbra for 6,698 s (67 s of the
    script) and in the umbra for 345 s (3.5 s). The script starts with the penumbra 96 km clear of
    the planet.
  - **Lit at the retarded time.** Each moving body's retarded centre carries its velocity and no
    light time to the camera (`lightTimeS` 0, a kept scene's), so `retardedFrom` places each source
    where the light reaching the lit body left it.
    - The moon's shadow falls 34.8 km (the moon's 28.7 km/s over its 1.21 s light time) from where
      the moon's place at the scene's time would cast it: 3.2 px at 10° across 320 px.
    - The planet, the camera's local body, is lit at the scene's time, which is its drawn time
      here.
    - `retardedFrom` takes the light time from the lit body's centre, not from each surface point.
      The light reaching the point beneath the star passed the moon 0.021 s later, so its shadow
      there is 0.6 km (R_P v ÷ c) off, under a tenth of the tested pixel.
  - **Drawn at the scene's time, lit at the retarded time (a stated limit of a kept scene).** A
    kept scene draws its bodies where they are, with no light time to the camera, while its
    lighting is retarded. Seen from the penumbra, the moon drawn over the star is therefore ahead
    of the eclipse the camera stands in by the moon's light time to the camera, 1.12 s: 32 km
    (9.6 × 10⁻⁵ rad, 1.05 px at 10° at 1080p), or 33 s of the scene's clock (0.33 s of the
    script), a tenth of the 345 s totality. A server scene draws the apparent place and has no
    such offset. Nothing compares the two in the view, since a camera's own light is not drawn.
  - **The tests** (14, `eclipseScene.test.ts`), through the view's path: `photorealFrame`, then
    `planLitBodies`, then `rasteriseDisc`.
    - **The umbra and penumbra to a pixel.** Each wholly covered pixel of the planet is classed as
      drawn: umbra where its meter class is `unlitBody`, penumbra where its starlight is below the
      same pixel's unshadowed, else clear. The oracle classes it at its centre by the exact
      tangent cones, as angles from the surface point the pixel's ray meets, with the moon on its
      exact track at that point's own retarded time (fixed-point steps). A pixel may differ only
      beside the oracle's boundary.
      - At 45° across 480 px (34.5 km a pixel), entering and leaving: 18,052 and 24,488 penumbral
        pixels, 12 and 14 umbral; 8 and 44 differ, every one beside the boundary.
      - At 10° across 320 px (10.9 km a pixel), at mid-eclipse: the umbra's 160 pixels, none
        differing.
      - The same frame against the moon's place at the scene's time differs at 40 pixels more
        than a pixel from that boundary, so the test sees the retardation.
    - **The probe's flux over the crossing.** The probe is the point beneath the camera, the centre
      of a 3 × 3 view a milliradian across, over 75 frames 100 s apart (mid-eclipse ± 3,700 s).
      Its starlight's share of the same pixel unshadowed, planetshine left out, is held to the
      exact eclipse integral (`eclipseIntegralVisibleFraction`) per channel.
      - Worst 0.29% on the high setting (K = 4) and 0.48% on the low (K = 3).
      - "The setting's error" is read as T6.b's bound for the Sun's V, 0.58% and 1.02%. The scene's
        law (`sunLikeHostDisc`'s Claret and Southworth V-band law in every channel) errs at most
        0.56% and 0.98% on T6.b's grid, under them.
      - The probe is clear at both ends and total at two frames.
    - **The planet behind the star.**
      - At 2° across 320 px, a test field below the view's 10°, the giant is an 8 px disc. It is
        drawn before the star's disc, and every one of its 68 pixels lies under that disc.
      - 5,600 s earlier, at the star's limb, 38 of its 68 pixels lie outside it, uncovered.
      - At the view's 60° at 1080p it is a 1.5 px point. Its sprite is drawn before the star, and
        its 7 px quad lies inside the star's 15.5 px disc. At the view's 10° it is a 9.8 px disc.
    - **The partial eclipse through the painter's order** (beyond the plan's list, the by-hand
      item's CPU half). From the penumbra, 2,000 s before mid-eclipse, the moon's disc is drawn
      after the star's and covers 0.3192 of the star's 5,712 pixels. The two circles' overlap is
      0.3189.
    - **Planetshine in the eclipse** (beyond the list). The moon's planetshine keeps 0.892 of the
      planet's light in the central eclipse, by `discEclipseVisible` (T10.b).
    - Also: the selector offers the scene; `stepRun` keeps the free camera in the planet's frame
      through the script; the script starts and ends with the moon and the giant clear of the
      star and the moon's shadow off the planet.
  - **The planet behind the star holds while it is a disc or a point.** This is T9's host-disc
    limit, ruled a stated limit for RM3: as a mesh it would show over the star's disc. No view
    writes depth yet, so no body of the scene is promoted. _It now holds as a mesh too
    (2026-10-06): the star's disc lies on its limb's plane, tested with the giant promoted by a
    synthetic depth writer; see "The star's disc at its limb's depth" below._
  - **`just test-render`** (SwiftShader, `default` and `no-subgroups`, 2026-10-06). Both exit 0,
    with 255 and 253 checks, none failing. That is T10.b's 249 and 247, plus T10.c's three
    checks, its captures' one, and R02.T14.c's two for `ECLIPSE TEST` in `SCENE_OPTIONS`.
    - The shadow on the GPU: texels within 0.322 and 0.153 of the tolerance of the twin, no class
      differing (45° across 240 px entering, 10° across 160 px central; 2 and 40 umbral pixels).
    - The giant behind the star: no channel differs from the frame without it (68 of its texels
      show without the star's disc).
    - The moon in front: it takes 1,776 of the star's 5,712 texels, as on the CPU.
    - 21 new captures a variant, the same in both. The 68 earlier captures are byte-identical to
      T10.b's.
  - **By hand, recorded (for the owner).** The partial eclipse seen from the penumbra, the star's
    disc partly covered by the moon's through the painter's order, is the capture series
    `r07-t10c-eclipse-penumbra-t068` to `-t101` (the total at `t101`), at the view's narrowest 10°.
    The shadow's crossing is `r07-t10c-eclipse-shadow-t021` to `-t151` at 60°, and the giant's
    passage `r07-t10c-eclipse-behind-t135` to `-t205` at 10°. All are 768 × 432, in
    `/home/quantum/gh/hyperion/.git/rm23-scratch/r07-shading/t10c/captures-3/`.
    - They are the record as T9's occultation captures are (the orchestrator's ruling of
      2026-10-05, option (a)), extended here to T10.c pending the orchestrator.
    - The shadow is exposed at EV100 15, for the sunlit day side. The penumbra is exposed at
      EV100 30, as through a solar filter, so that the star's disc and its limb darkening lie
      inside AgX's range. The giant is exposed at EV100 27, the star's centre at the top of that
      range and the giant's full disc, about 10⁻³ of its luminance, some 3 stops below middle
      grey; at EV100 30 it is lost in black. Even so, its 3.9 px disc is a faint dot beside the
      star (`t135`), and lost against the star's edge once half behind it. At the day side's
      exposure the star's glare fills an eye view's whole frame, as the Sun's fills an eye, and
      hides both the moon's bite and the giant.
    - In the view, `ECLIPSE TEST` shows them live: the star as the target, the field at 10°, and
      `MAN` near EV100 30 for the penumbra (from 67 s of the script to 134 s) and 27 for the giant
      (from 144 s to 266 s).
  - **Test fields below the view's narrowest.** The probe's 0.06° and the giant's 2° are test
    fields below `FOV_STEPS_DEG`'s 10°; the giant is also tested at the view's own 60°, and is a
    9.8 px disc at its 10° at 1080p.
  - **Left for the orchestrator and the owner.**
    - The guide's `SCENE` row, and its row of the kept scenes' designations (`TEST STAR` …), do not
      name `ECLIPSE TEST`, as T10.c calls for no guide edit. Drafts, not entered (the views lane's
      T16 is editing that table):
      - `SCENE`: "… a kept test scene, `PRECISION TEST`, `FRAME CHANGE TEST`, `PHASE TEST` or
        `ECLIPSE TEST`, under the training banner, …";
      - a row `ECLIPSE TEST` | Scene | "The kept test scene of eclipses, its clock at a hundred
        times: a moon's shadow crossing an Earth-sized planet beneath a camera 20,000 km above it,
        the moon crossing a Sun-like star seen from inside its penumbra, and a giant passing
        behind the star";
      - the designations row: "… `ECLIPSE TEST`'s Sun-like star, Earth-sized planet, Moon-sized
        moon and Jupiter-sized giant …".
    - The label block does not state a kept scene's clock rate: `TIME` runs a hundred times fast
      here with nothing saying so.
    - Whether a free camera in the planet's frame stands for the plan's "ship in a moon's
      penumbra".
- **The photorealistic view's per-draw cost, investigated** (2026-10-06; the shading lane, from
  T19.c's 1.87 ms an instrument frame and the T20 smoke's 4.8–5.8 ms).
  - **The cause: a small disc's sampling, run in series in a few fragments.** A disc under 32 px
    (`SMALL_DISC_PX`) takes 8 × 8 cells in every pixel, and each cell within two cell widths of the
    limb takes up to nine points (T8.a's sampling). A fragment runs its 64 cells one after another,
    and a warp across the limb runs the nine-point branch at a cell for any lane that needs it:
    about 576 shades in series, each about 3,500 cycles, 2.0 million cycles. A disc of 4–31 px has
    at most about 1,200 fragments, so the GPU holds a handful of warps and waits on the longest one.
    That chain sets the pass's time, whatever the view's size. In `PHASE TEST` the half planet is
    13 px across in a 240 px instrument at 60° (62 px in the primary), and the giant a 14 px disc at
    the primary's edge. _Since T8.c a disc of 4–32 px takes 4 × 4 cells, and its chain a quarter
    of the shades (see "Deviations in T8.c, as built")._
  - **How it was measured.** Hidden runs on the RTX 3080, the window offscreen at 1920 × 1080 and
    never resized, `--hyperion-gpu-timing`, `PHASE TEST` with the primary (1120 × 900) and both
    instruments (240 × 180) photorealistic in `FREE`, 6 s phases. Each view's GPU time per pass was
    summed over the resolves its draw numbered, through temporary counters never committed. The
    GPU's clocks were logged by `nvidia-smi` every 250 ms; load averages 5–15, another lane's
    Electron sometimes on the GPU. The harness is
    `/home/quantum/gh/hyperion/.git/rm23-scratch/r07-shading/cost-t19/` (`run-cost.sh`,
    `hook-cost.js`, `instrumentation.patch`, `cycles.js`, the raw logs).
  - **The evidence.** Times are medians of the `discs` pass; cycles are the time at the phase's
    median graphics clock.
    - The cycles hold while the clocks move: an instrument at 60° takes 2.01–2.02 million cycles
      at 1,140–1,980 MHz (1.02 ms at 1,980 MHz, 1.77 ms at 1,140 MHz), and 2.18 million at
      510 MHz (4.28 ms). The memory clock, 810 to 9,501 MHz, changes nothing: the chain is
      arithmetic latency, not memory.
    - Not per pixel: at 30°, 45°, 90° and 120° (discs of 4–29 px) the instrument takes 1.87–2.05
      million cycles. At 20° (43 px, so one cell inside and 4 × 4 on the limb) it takes 0.47
      million, and at 10° (87 px) 0.54 million.
    - Per cell: a build with 4 × 4 cells below 32 px takes 0.54 million, the 16 cells' share of 64.
    - Planetshine's two sources are 41% of a shade: a build without them takes 1.18 million.
    - The phase table's fetches are off the chain: a build fetching fixed texels takes 1.99
      million.
    - Two small discs in one view overlap rather than add: with the giant also in the instrument,
      2.17 million.
    - The rest of an instrument's frame is small: bloom 0.056 ms (0.116 ms in the primary, 23 times
      larger), tone mapping 0.009 ms, sky 0.003 ms, symbology 0.008 ms, no histogram, all at
      1,980 MHz. Its scene target is its own size. Uploads, pipelines and bind groups cost no GPU
      time in a pass.
  - **Why the T20 smoke read 4.8–5.8 ms, and 9.2 ms in its second run.** Its phase was a
    wireframe primary with one photorealistic instrument. The GPU idles there, and the driver
    drops its clocks: in three of this harness's four runs of the phase to medians of
    510–780 MHz (P5), and in the fourth it held 1,800 MHz (1.21 ms). The same 2.2 million cycles
    read 4.28 ms at 510 MHz, and 9.2 ms is them at about 240 MHz, near the card's floor. T19.c's
    1.87 ms, with all three views photorealistic, was the chain at about 1,150 MHz.
  - **Cut, keeping the output: a disc wholly off the view draws nothing.**
    - `sphereScreenRect` gives the whole view to a sphere with any silhouette corner behind the
      near plane, which a body behind the camera, or beside it across the camera's plane, has.
      That body's two draws covered every pixel, each fragment rejecting itself. `PHASE TEST`'s
      full planet stands 90° off the axis.
    - `planLitBodies` now drops the disc record of a body wholly beyond a side plane widened by
      `OUTSIDE_VIEW_MARGIN_PX`, 8 px (`sphereOutsideView`, `bodies/regime.ts`; in
      `wireframe/submit.ts` since T19.e). Its regime is kept.
    - The primary's `discs` pass falls from 2.80 to 2.26–2.28 million cycles (−19%: 1.42 to
      1.15 ms at 1,980 MHz). The instruments' is unchanged: there the whole-view draws cost
      nothing measurable. The cut grows where the GPU is bound by throughput rather than latency:
      the UHD 620 at 1280 × 720, or a 4K view.
    - Tested (`draw.test.ts`, `regime.test.ts`):
      - Such a body, beside or behind the camera, keeps its regime and has no record, as a disc
        and, promoted, as a mesh with no figure and no limb. Both tests fail before the change.
        _Since T19.e such a body has no footprint and is never promoted, so the mesh case is now
        T19.e's test that it is not; see "Deviations in T19.e, as built"._
      - Spheres just beyond each widened plane (beside the view, level with a corner, across the
        camera's plane, behind) leave no pixel the disc's twin (`rasteriseDisc`) would draw. The
        sweep takes 10°, 60° and 120° across a 72 × 40 and a 40 × 72 view (whose height spans
        144°), 3 and 300 radii, and a sphere, f = 0.098 and the record's cap f = 0.2 with an
        oblique pole. With no margin 21 of its 36 cases draw pixels; from 1 px none do.
      - A sphere that the edge cuts, or whose limb is a pixel past it, is kept.
    - `just test-render` (SwiftShader, 2026-10-06): both variants exit 0, 255 checks each, none
      failing, no uncaptured GPU error. All 55 captures a variant are byte-identical to the merged
      base's, run first with the change stashed, and the two variants' to each other. The base's
      run failed only the histogram check that times out under load, as recorded for T10.b, and so
      stopped before `no-subgroups`. Against T10.c's captures only R05's three spike craft frames
      moved, in both variants, as they do in the base: T16.a's hull bias, merged in. The review's
      changes after the run rename two locals and touch only comments and tests.
  - **Options for the small disc's chain (a design change, for a ruling).**
    - (a) Fewer cells as the disc grows below 32 px, at 8 × 8 near the 3 px switch: for example
      n = min(8, max(4, ⌈48 ÷ d⌉)) for a disc d px across. The 4 × 4 build is its measure: −73% (an
      instrument's 2.01 to 0.54 million cycles, 1.02 to 0.27 ms at 1,980 MHz, 4.3 to about 1.1 ms
      at 510 MHz). It moves every capture with a disc of about 7–31 px, and T8.a's sweep (3, 3.3 and
      6 px within 1%), the phase scene's limb and terminator to half a pixel and the Saturn
      extents must hold again.
    - (b) The cells in parallel: a compute pass shades each small disc's cells one a thread and sums
      each pixel's in the cells' order, and the disc's draws read the sums. The chain falls to one
      cell's (three limb angles and up to nine shades, about 35,000 cycles), and the cost becomes
      throughput, under 0.1 ms an instrument here. The output stays, but for any difference in how
      the compiler fuses the two stages' arithmetic. It needs a new pass, its buffers, its twin and
      its tests: the larger change.
    - (c) Cheaper shades with the same output: the per-draw rows and directions hoisted out of
      `shade`. At most 10–20%, since the star's horizon term and planetshine are per point. Any
      change to the order of the shader's arithmetic risks a last-bit difference from the twin.
    - (d) Nine points only in the cells the limb crosses, not those within two cell widths of it.
      This changes the limb's integral, and a warp still takes the nine-point branch for any lane
      that needs it.
    - The lane's lean: (a), the smallest change for most of the gain; (b) if the output must stay.
  - **Projected on the UHD 620 (an estimate; the owner's T17 and T20 runs settle it).** The chain
    is about 576 dependent shades on any GPU. Taking Gen9 at 3,000–7,000 cycles a shade gives
    1.2–4 million cycles: about 1–4 ms at its 1.1 GHz, and 4–14 ms at its 300 MHz floor. Its
    throughput alone, for the same work in SIMD8 with the same divergence, would take about 2 ms.
    - On the low setting, one photorealistic view at most. A photorealistic primary with two
      wireframe instruments carries one chain where a small disc is in view, as `PHASE TEST`'s
      giant is: 1–4 ms of the 33 ms. A wireframe primary with a photorealistic and a wireframe
      instrument carries one chain on each frame the instrument draws, against 0.8 T (13.3 ms) at
      60 Hz. It passes at full clock and may miss at the floor, where an idle GPU's clocks may sit,
      as this machine's did in that phase.
    - On high, with both instruments and the primary photorealistic, up to three chains, about
      3–12 ms at full clock.
    - The figures scale with the clock, so the owner's runs should record the GPU's clock beside
      each pass time (on Intel, `gt_cur_freq_mhz` under `/sys/class/drm/card0/`). _T17 reads
      `gt_act_freq_mhz` beside it, on the i915 card, which is not always `card0`
      (decision-r07-small-disc-cost)._
  - **Left open, not measured apart.**
    - R06's host disc is a full-view triangle for every host of 3 px or more, on the view or not.
      _Built by T19.e: none off the view, and a quad over its rectangle on it._
    - R02's wireframe occluder spheres take the same whole-view rectangle behind the camera
      (`packWireframe`). _Built by T19.e: none packed off the view._
    - `sphereFootprint` (T9) gives such a body the whole view's circle, so once a view writes
      depth a body off the view is promoted with the writer and promotes every disc in turn: the
      mesh test above promotes a body clear of both. Nothing changes on screen today, since no
      view writes depth. Taking `sphereOutsideView` there too would end it, before R10. _Built by
      T19.e: no footprint off the view._ See "Deviations in T19.e, as built", the last entry.
  - _Ruled 2026-10-06 (decision-r07-small-disc-cost):_
    - **The sampling.** (a) is taken in two levels, not the lean's ramp: 8 × 8 below 4 px, the
      only cells that meet T8.a's 1% at the 3 px switch (6 × 6 reaches 1.15%, 4 × 4 1.38%), and
      4 × 4 from 4 px to 32 px.
      - 4 × 4 is within 0.77% of the exact near-field flux there, against 8 × 8's 0.79%. It is the
        better of the two on crescents from 6 to 11 px, where 8 × 8's narrower nine-point band
        leaves the terminator centre-sampled.
      - Built as T8.c. _As built, against a near-field oracle that follows each placement's
        phase: 0.52% against 8 × 8's 0.49% over 4–31.5 px. At 6–11 px and 150°, 4 × 4 reads
        0.005–0.31% against 8 × 8's 0.29–0.49%, better but for f = 0.098 at 9.6 px (0.31% against
        0.29%). See "Deviations in T8.c, as built"._
    - **The cell pass.** (b) is then taken for every disc under 32 px, as T8.d, so that a small
      disc's cost is throughput:
      - the 2.7–4 px disc's 2.0 M-cycle chain goes;
      - the resolution controller's scale acts on small discs again.
    - **Not taken.** (c) is not ordered. (d) is rejected: it narrows the band that carries
      crescents and changes the edge integral.
    - **Off-view draws.** R06's host disc and R02's occluder spheres need no change for the chain.
      T19.e removes their off-view draws, and bounds the host disc's draw, with no texel changed.
    - **Order and residual.** T8.c, T8.d and T19.e precede T17. The residual is a disc of 32 px or
      more's 4 × 4 limb, 0.47–0.54 M cycles.
- **The star's disc at its limb's depth** (2026-10-06; the shading lane: R06's follow-up to T9,
  queued before R10).
  - **What changed.** `disc.wgsl`'s vertex stage now puts R06's full-screen triangle on the
    camera's polar plane of the star's sphere, d cos²ρ along the axis, where T9 puts a mesh body's
    limb. Before, it lay at depth 0, at infinity. _Since T19.e the triangle is a quad over the
    star's screen rectangle, and `hostDiscRecord` takes that rectangle; see "Deviations in T19.e,
    as built"._
    - Its reversed depth is n (u · axis) ÷ (d cos²ρ) for the view ray u = (x_ndc ÷ s,
      y_ndc ÷ (s a), −1). That is affine on the view, so the triangle's corners carry it
      unclamped. The depth clip removes the part that sees the plane behind the camera, which
      holds no pixel of the disc.
    - The plane holds the limb and lies inside the star along every ray that meets the disc. So a
      body nearer than the star hides the disc, and a mesh body beyond it, whose figure writes
      depth, is hidden. The disc still writes no depth, and the painter's order still places disc
      bodies and limbs about it.
    - `HostDiscLayer` passes `inverseLimbDistance`, 1 ÷ (d cos²ρ) in m⁻¹, as an `f32` after
      `exposure`. Its other uniforms are bit-identical to before and the fragment stage is
      unchanged.
    - Where nothing writes depth, the depth buffer still holds 0 under every host disc, so no
      view's output changes. The live view writes none until R10.
    - `sky/disc.ts` gains `HostDiscRecord` and `hostDiscRecord(placement, exposureScale)`, what a
      draw carries, and `rasteriseHostDisc`, its `f64` twin, with each lit pixel's light and
      depth. `DiscDraw` gains `record`.
    - `compositeBodyFrame` (`bodies/frameTwin.ts`) takes the host records by star (`hosts`, none by
      default). It draws each at its step, opaque on its plane, so hidden where a nearer figure
      drew.
  - **The stated limit is resolved.** That is T9's "Known limits", the ruling of 2026-10-05 and
    T10.c's "The planet behind the star holds while it is a disc or a point". The test uses T9's
    synthetic depth writer over `ECLIPSE TEST`'s giant at conjunction: 2° across 320 px, the giant
    an 8 px disc promoted to a mesh.
    - On the CPU (`eclipseScene.test.ts`), the twin leaves the giant no share of any pixel. With
      the disc at infinity, as before, the giant keeps 32 pixels, its wholly covered ones.
    - On the GPU (`smoke/eclipse.ts`, "R06.T13.e a mesh giant behind the star leaves no texel of
      its own under the star's disc on its limb's plane"), no channel differs from the frame
      without the giant, and without the star's disc the giant shows in 68 texels. On the merged
      base, with the disc at infinity, the same check fails: 128 channels differ, the giant's 32
      wholly covered pixels.
    - `disc.test.ts`, at every pixel the disc lights: the plane's depth lies behind the star's near
      side and before its far side, and equals T9's `limbDepths` plane for the sphere to 10⁻⁹ (a
      turned camera, 30° across 128 px). The uniform carries 1 ÷ (d cos²ρ) to `f32`. The twin's
      light is held to the law (10⁻¹²), its flux to π L̄ sin²ρ (1%, the T13.e test, which now
      sums the twin) and its clamp to 65,504.
  - **`just test-render`** (SwiftShader, `default` and `no-subgroups`, 2026-10-06). Both exit 0,
    with 257 and 255 checks, none failing, and no uncaptured GPU error: the merged base's 256 and
    254 and the mesh giant's check. All 55 captures a variant are byte-identical to the merged
    base's, and the two variants' to each other. The base (a620deb with only the new check) was
    run first; it failed that check alone, and so stopped before `no-subgroups`.
  - **A stated limit: a camera within centimetres of a photosphere.** The plane lies d cos²ρ,
    about 2 (d − R), ahead. Within about 5 cm of the surface it is nearer than the near plane
    (0.1 m), and the depth clip removes the disc, which at depth 0 lit the forward hemisphere.
    `hostPlacements` places no host for a camera inside its star, and no view stands that close.
- **Deviations in T19.e, as built** (2026-10-06; the shading lane, after R06's limb-depth
  follow-up). Built as decision-r07-small-disc-cost rules it; no capture moved.
  - **The off-view test at R02's level.** `sphereOutsideView` and `OUTSIDE_VIEW_MARGIN_PX` (8 px,
    unchanged; the plan names only the function) moved from `bodies/regime.ts` into
    `wireframe/submit.ts`, beside `sphereScreenRect`. `regime.ts` and `bodies/draw.ts` import
    them.
    - The margin's TSDoc adds why its 8 px serves the two draws that light or write a pixel only
      where the ray through the pixel's centre meets the sphere: R06's host disc and R02's
      occluder sphere. The outermost centres lie half a pixel inside each side, so those need no
      margin but for their shaders' `f32`, under 10⁻⁶ rad, against 4.5 × 10⁻⁵ rad for a pixel at
      4K across 10°.
  - **R02's occluder spheres.** `packWireframe` packs none wholly off the view, which
    `sphereScreenRect` gave the whole view behind the camera or across its plane. It is one
    condition in the packing loop.
  - **T9's footprint.** `sphereFootprint` gives none for a body wholly off the view, so it is
    neither promoted nor promotes. `discRecordOf`'s own drop (T19) stays for R10's depth writers.
  - **R06's host disc.**
    - `HostDiscLayer.frame` gives a disc wholly off the view no draw. It keeps it among the discs
      whose glare sources it returns, so the eye's 45° reach is unchanged, and T13.e's 30° and
      50° glare test passes unchanged. A disc whose `sphereScreenRect` is empty has no draw
      either: it lit no pixel.
    - Each other draw is R02's quad (`WIREFRAME_MESHES.quad`, named `sky disc quad`) over
      `sphereScreenRect` of the star's apparent centre and radius, the whole view where the
      silhouette reaches behind the near plane. The rectangle is the uniform `rect` (px, a
      `vec4f` after `inverseLimbDistance`) and `HostDiscRecord.rect`. `hostDiscRecord` takes it,
      and the twin `rasteriseHostDisc` scans only it.
    - `disc.wgsl`'s vertex stage maps each corner through the rectangle to NDC, as
      `occluderSphere.wgsl` and `bodyDiscDraw.wgsl` do, and carries the limb plane's depth as
      before. The fragment stage is unchanged.
  - **"No texel changed" is measured, not guaranteed.** The fragment's NDC is now interpolated
    from the quad's corners, not the triangle's (−1, −1), (3, −1) and (−1, 3). The two agree to
    `f32`'s rounding, so another GPU could move a texel at the limb by a half float's step. On
    SwiftShader every capture is byte-identical (below).
  - **Files shared with T8.c.** The ruling calls T19.e's files disjoint from T8.c's, but moving
    `sphereOutsideView` changes `bodies/draw.ts`' import, and T19's mesh case is in
    `bodies/draw.test.ts`. Both are T8.c's files; expect a small merge there.
  - **Tests**, as ruled.
    - `disc.test.ts`:
      - Discs just beyond each widened side plane (beside the view, level with a corner, across
        the camera's plane and behind it), at 10°, 60° and 120° across a 72 × 40 and a 40 × 72
        view, from 3 radii: no draw and no sprite.
      - A disc the edge cuts keeps its draw, and so does one whose limb is a pixel past the edge.
      - An eye view keeps the glare source of a disc 30° past its top edge.
      - The twin over the whole view lights none of those discs' pixels, from 3 and 300 radii:
        the margin, against the old path. Each disc's nearest point stands 10⁻⁶ of its radius
        beyond the plane.
      - The quad lights exactly the full-view draw's pixels over a sweep: discs 3.5, 8, 30 and
        120 px across at the centre, on the right edge and over the top left corner, and one
        130° across whose rectangle is the whole view (60° across 160 × 90).
    - `submit.test.ts`: the same sweep of occluder spheres, from 3 and 300 radii, packs none, and
      an `f64` twin of `occluderSphere.wgsl`'s ray test writes none of their pixels. Their nearest
      points stand 10⁻³ of a radius beyond the plane, past their centres' rounding to `f32`
      (128 m at 300 radii). A sphere the edge cuts is packed, and so is one whose limb is a pixel
      past the edge. The test takes `OccluderSphere` from the list's type, leaving alone the
      `./drawList` import line that R07.T16.d changes.
    - `regime.test.ts`: no footprint for the sweep's spheres. Footprints for the sphere the edge
      cuts and for the one whose limb is a pixel past the edge.
    - `draw.test.ts`: T19's mesh case. Neither the body beside the camera nor the body clear of
      both is promoted. Before, the body's whole-view footprint promoted both. T19's check of a
      promoted mesh body off the view, with no figure and no limb, can no longer be set up.
    - `test/beyondView.ts`: `centresJustBeyond` and `limbPastRightEdge`, also used by T19's sweep
      in `regime.test.ts`. `eclipseScene.test.ts` takes the star's record from the layer.
    - With the three conditions taken out, 20 tests fail: the packing sweep (12), the host
      discs' sweep (6), the footprint test (1) and `draw.test.ts`' (1). The twins' sweeps check
      the margin, not the cull, and pass either way.
    - On the GPU, "R07.T19.e a host disc drawn over its screen rectangle lights its twin's
      texels, off the centre and cut by the view's edges" (`smoke/sky.ts`): a turned camera,
      60° across 96 × 64, a disc cut by the right edge, one over the top left corner and one off
      the centre. Each disc's texels are its twin's, with the twin's light. That puts the
      vertex stage's mapping through the rectangle under test, which a centred disc would not.
    - R06's `checkSkyDisc` probed "nothing lit outside" at texel (2, 2), which is outside the
      quad since T19.e. It now probes (22, 22), inside the quad and outside the disc, so the
      shader's own discard is what it tests.
  - **`just test-render`** (SwiftShader, `default` and `no-subgroups`, 2026-10-06). Both exit 0,
    with 258 and 256 checks, none failing, and no uncaptured GPU error: R06's limb-depth
    follow-up's 257 and 255 and the quad's new check.
    - The quad's check: 262, 159 and 100 texels, none not the twin's, colours within 0.118 of the
      tolerance.
    - All 55 captures a variant are byte-identical to the merged base's (a620deb), to R06's
      limb-depth follow-up's and to the other variant's.
    - The first `no-subgroups` run failed only T8.a's histogram check, which times out under load
      (load 10–13 then; recorded for T10.b). It passed alone on a rerun, with captures identical
      to the first run's.
  - **The cost saved, measured** (hidden, RTX 3080, the per-draw cost harness, 2026-10-06, load
    11–15). `PHASE TEST` with all three views photorealistic: the primary at 1120 × 900 stands on
    the half planet, the star 90° off its axis, so the merged base drew one full-view host
    triangle there each frame; the instruments' Sun is a 2 px sprite. T19.e draws none.
    - One such triangle costs about 9–11 thousand cycles in the `discs` pass (about 9 µs at the
      run's 1,155–1,245 MHz, 5 µs at 1,980 MHz). A build of the base drawing it 64 times read
      2.80–2.94 million cycles, against the base's 2.24 million.
    - That is 0.4% of the primary's pass, inside the runs' ±1% spread: T19.e read 2.24–2.26
      million cycles, the base 2.24. The instruments' passes are unchanged. A host disc on the
      view saves the same on every frame now: its triangle discarded outside its rectangle too.
    - The occluder spheres' cull in the wireframe pass is not resolved: the wireframe primary's
      whole GPU time is 0.025–0.029 ms in every build.
    - On the UHD 620, which is bound by throughput, the triangle's 0.9 million fragments at
      1280 × 720 are an estimated 0.1–0.2 ms a frame at 1.1 GHz (not measured; T17's owner runs
      settle it).
    - Harness: `/home/quantum/gh/hyperion/.git/rm23-scratch/r07-shading/star-depth/cost/`
      (`build-variants.sh`, logs, `cycles-sd1.txt`), with `cost-t19/`'s instrumentation.
- **Deviations in T8.c, as built** (2026-10-06; the shading lane, after T19.e). Built as
  decision-r07-small-disc-cost rules it: the constants, `discSamples` and `discRecordOf`'s counts,
  with no hysteresis and no shader change.
  - **`discSamples(diameterPx)`** returns `DiscSamples` (`{ interior, limb }`): 8 × 8 below
    `FINE_DISC_PX`, 4 × 4 below `SMALL_DISC_PX`, one inside and `LIMB_SAMPLES` on the limb above.
    NaN, and +∞ (which `angularDiameterPx` gives for a camera inside the body), take the large
    disc's counts, as before.
  - **Files.** The plan names `bodies/draw.test.ts`. The flux gates against the point and the
    near field (G1, G2, G2′), row 3 and G6 are there, beside T8.a's rows they extend. A new
    `bodies/discShading.test.ts`, beside `discSamples`, holds its table, G4, G5, G5′ and the brute
    force's own tests, so that the two files run in parallel (about 46 s and 27 s under load 19).
  - **`pointSampleDisc`** (new, `bodies/discShading.ts`): a record's pixels point-sampled m × m
    times each with the disc's own `shade`, in `f64`, the brute force of G5′. It sits beside the
    twin because it needs the twin's private ray and shade.
  - **The near-field oracle, corrected (science check, must-fix).** `worstOver` took the near
    field once, at the centred placement. A centre moved x px also turns the phase by up to
    x ÷ 1,663 rad, which moves a crescent's flux by up to 0.25% at 150° (0.07% at 90°). The near
    field is now carried to each placement by the point's ratio, which matches an integral at
    each placement to 3 × 10⁻⁵. T8.a's 6 px rows take the corrected oracle too, and pass. The bias
    had read the disc's error high, since the disc's own error is negative. The ruling's figures
    of 0.77% and 0.79% (and so "at most 0.8%") carry it. Corrected:
    - G2's worst is 0.52% under 4 × 4 (a sphere at 13 px, 150°) against 0.49% under 8 × 8 (6 px,
      150°). Over 4–31.5 px, 4 × 4 is within the 1% as 8 × 8 is, not more accurate, and
      `SMALL_DISC_SAMPLES`' TSDoc says so.
    - At 6–11 px and 150° the ruling's comparison stands: 4 × 4 reads 0.005–0.31% against
      8 × 8's 0.29–0.49%, better at every size but f = 0.098 at 9.6 px (0.31% against 0.29%).
    - G2′'s worst is 0.45% (lunar law) and 0.46% (the 45°-latitude spheroid), both at 13 px. The
      ruling's 0.57% and 0.63% came from sizes without a 13 px row; the biased oracle gives 0.62%
      and 0.71% there.
  - **4.01 px stands for the ruling's 4 px** in G2, G2′, G5 and G6. A placement off the centre
    stands a little farther off, which takes a disc 4 px across at the centre pixel's scale just
    under 4 px (3.9999992 px at 0.75 px off), and so back to 8 × 8. G6, which is centred, takes
    4.01 px too, for uniformity. G4 sets its counts itself, so it takes 4.0 px.
  - **G1's band starts at 2.7 px, not 2.75 px.** A shrinking disc turns to a point at 2.7 px
    (`POINT_BELOW_PX` × (1 − `REGIME_HYSTERESIS`)), so a row at 2.701 px joins the ruling's 2.75,
    3 and 3.3 px. 2.7 px itself puts the placements off the centre under it, where no disc is
    drawn.
    - G1's worst is 0.85% (f = 0.098, 2.75 px, 150°); 0.84% at 2.701 px. The ruling's 0.76% was
      measured at 2.8–3.3 px only (0.46% at 2.8 px), and the error is not smooth in size: 0.39%
      to 0.82% between 2.701 and 2.75 px for the sphere at 150°.
    - Its G1′ figure of 0.73% is the lunar law's 3.3 px row. At 3 px, the test's size, it is
      0.10%, and the 45°-latitude spheroid 0.81%.
    - The cells there are 8 × 8, unchanged by T8.c.
  - **G5's reference.** It is the 32 × 32 twin, as ruled, from 4 px. Below 4 px it is the
    64 × 64 twin.
    - A 3 px crescent at 150° is 0.2 px deep, beyond 32 × 32's near-limb band (about 0.08 px).
      Its terminator falls in centre-sampled cells, and the 32 × 32 twin stands 0.32% of the peak
      from a converged brute force there (1,024² samples a pixel), against 64 × 64's 0.11% and
      8 × 8's own 0.22%.
    - G5′ gains that case: the 64 × 64 twin at 3 px and 150° against 512² samples a pixel (256²
      is itself 0.20% off there, 512² 0.017%), within 0.3%. It takes about 5 s, under a 60 s
      limit. The ruled case at 4 px reads 0.18% against 256².
    - The errors are taken in each channel against that channel's peak, the RMS over the pixels
      either drawing covers (the night side included, as the ruling's probe takes it), the worst
      channel's.
  - **The figures** (`f64` twin, 16 placements and three channels unless stated):

    | Gate | Bound | As built | Ruling |
    |---|---|---|---|
    | G1, 2.701–3.3 px, against the point | 1% | 0.85% | 0.76% (2.8–3.3 px) |
    | G1′, 3 px: lunar law; 45° spheroid | 1% | 0.10%; 0.81% | 0.73%; 0.81% |
    | G2, 4.01–31.5 px, against the near field | 1% | 0.52% | 0.77% |
    | G2′, 4.01, 8, 13, 24 px at 150° | 1% | 0.45%; 0.46% | 0.57%; 0.63% |
    | G4, the steps at 4 px and 31.9 px | 0.5% | 0.41%; 0.29% | 0.41%; 0.29% |
    | G5, every pixel: max; RMS; coverage | 1.5%; 0.3%; 0.0025 | 1.22%; 0.18%; 0.0018 | 1.24%; 0.20%; 0.0017 |
    | G5′, the reference against brute force | 0.3% | 0.18% (4 px, 32 × 32, 256²); 0.11% (3 px, 64 × 64, 1,024²) | 0.09% (64 × 64 at 512²) |
    | G6, the eclipsed share against `discEclipseVisible` | 0.005 | 0.0010 | 0.0011 |

    G4's worst are the f = 0.098 spheroid at 150° (4 px) and the 45° spheroid at 120° (32 px).
    G5's worst is a 12 px crescent's pixel by its terminator, at 150°. G6 takes the provisional
    Lambert and a lunar law.
  - **The smoke harness.** A new check, "R07.T8.c a crescent's texels equal the CPU rasteriser's
    at each sampling level …", draws 3.5, 13 and 40 px crescents at 150° and holds each record's
    counts (8/8, 4/4, 1/4), texels and classes to the twin. Its texels are within 0.074, 0.126 and
    0.084 of the tolerance (0.4% relative + 10⁻⁴). `checkTexels`' 20 px disc now runs at 4 × 4:
    within 0.237 of the tolerance (G10 re-measured; the 3 px flux stays within 0.42% of the
    point).
  - **`just test-render`** (SwiftShader, 2026-10-06, after the machine's reboot).
    - `default` exit 0 with 259 checks, the base's 258 and the new one.
    - `no-subgroups` failed only T8.a's histogram check, "no histogram: 0 weighted counts", which
      times out under load (load 19–21; recorded for T10.b and T19.e). Rerun alone it exited 0
      with 257 checks (the base's 256 and the new one), the histogram's 228 counts among them, and
      the same captures.
  - **No capture moved, because none holds a disc of 4–31 px.**
    - All 55 captures a variant are byte-identical to the merged base's, run first with T8.c's
      production files stashed, and to each other across variants.
    - The ruling's "every capture with a disc of about 4 to 31 px moves" holds with no such
      capture. Planned as the captures draw them:
      - `ECLIPSE TEST`'s giant is 3.91 px at 10° across 768 px, so it stays 8 × 8.
      - Its moon is 45.3 px and its planet larger.
      - T9's occultation captures are drawn at 512 × 384, four times the scene's 128 px: the moon
        48.1 px and the planet 180 px, both on the large rule.
      - No other capture draws a lit body.
    - The 4 × 4 level is held on the GPU by the new smoke check instead.
  - **The cost, measured** (hidden and offscreen on the RTX 3080, 2026-10-06 19:56–20:06, after
    the reboot). The per-draw cost harness (`.git/rm23-scratch/r07-shading/cost-t19/`) ran under
    `just _locked`, with `nvidia-smi`'s clocks every 250 ms.
    - Two instrumented builds of the merged head, without and with T8.c's production edits.
    - Three rounds: the `var` plan, the `fov` plan, and `var` again in the other order.
    - Load 6–12 in the first two rounds and 34–70 in the third. The crash-telemetry sampler,
      `nvidia-smi` every 2 s, started at 20:02:57, during the third.
    - Times are medians of the `discs` pass, and cycles are the time at the phase's median
      graphics clock.
  - **The instruments.** `PHASE TEST` all photorealistic, each 240 × 180 at 60°, its only disc the
    13 px planet at 4 × 4. The `discs` pass took 2.00–2.04 million cycles before (1.65–1.70 ms
    at 1,185–1,215 MHz) and 0.48–0.55 million after: −73% to −76%, under the ruled 0.6 million in
    every round.
    - After: 1.18–1.20 ms at 450 MHz, 0.77–0.79 ms at 690 MHz, 2.28–2.32 ms at 210 MHz. At
      1,980 MHz, 0.54 million cycles is 0.27 ms.
    - The driver drops the clock under the lighter load, from 1,155–1,215 MHz to 210–780 MHz.
      So the time falls by less than the cycles: 1.70 ms to 1.20 ms in the first round.
    - The other fields, after: 30° 0.43–0.53 million, 45° 0.59, 90° 0.57, and 120° 0.63 (two
      4 × 4 discs in view). 10° and 20° hold discs of 32 px or more, at 0.39–0.53 million before
      and after.
  - **The primary** (1120 × 900): the 62 px half planet, and the 14 px giant, now 4 × 4. It took
    2.23–2.38 million cycles before (median 2.26) and 0.49–0.67 million after (median 0.60), the
    ruling's projected 0.54–0.6.
  - **A disc under 4 px.** In the `var` plan's last phase `PHASE TEST`'s giant, about 3 px and so
    8 × 8, enters each instrument beside the planet.
    - Those instruments took 2.17–2.19 million cycles after and 2.18 before: the 8 × 8 chain,
      unchanged until T8.d.
    - T20's phase (one photorealistic instrument, the giant in it) took 2.18–2.25 million either
      way.
    - This stands for the ruling's 3.5 px view: the harness's widest field, 120°, puts the
      planets at 4–8 px, so it has no 3.5 px case.
  - **The figures are provisional.** The machine was shared, and T17 retakes them.
  - **`just ci`** was not run, under the Day 2 protocol: the orchestrator runs it on the merge.
  - **Plan text.** The ruling's T8.c, T8.d, T17 and T20 text and its Risks bullets are inserted:
    - T8.c and T8.d after T8.b;
    - T17's paragraphs before "Tests (Vitest)", the permitted reorder as their last;
    - T20's note as the last bullet of its entry;
    - the "Ruled 2026-10-06" bullet at the end of the per-draw cost entry;
    - the amendment on T8.a's "Sampling, as built".
    - The ruling places "Sample counts by size" after a Risks bullet "Disc anti-aliasing (T8.a)".
      That is decision-r07-t8a's own Risks bullet, which the plan folded into part 1's "Sampling"
      and "Risks (decision-r07-t8a)" items. The new bullet follows T8.a's four as-built entries,
      before T8.b's.
    - decision-r07-t8a's item 3, refinement 4, carries the ruled amendment (an orchestration
      file, not tracked).
    - The inserted text keeps the ruling's figures. The corrections above are for the ruling's
      author, and so is "the only cells" of the "Ruled" bullet: of the counts measured (4, 5, 6
      and 8 per axis), 7 × 7 untried.

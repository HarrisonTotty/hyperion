# Plan R05: Terrain Geometry and the Descent Spike (the Gate)

- **Milestone:** Rendering milestone RM2 (the gate).
- **Depends on:** [R01](01-graphics-platform-and-engine.md) (the platform, the engine adapter, the
  per-view canvas contexts, the SwiftShader smoke harness),
  [R02](02-real-scale-view-and-wireframe.md) (the view's camera, reversed-Z, camera-relative
  differencing, `BodyFixedPosition`, the photometric pipeline, the wireframe style and the label
  block) and [R04](04-cross-target-determinism.md) (`hyperion-base`, the `hyperion-surface`
  skeleton, both wasm targets in `just ci`, the terrain hazards, the client's first WebAssembly and
  R04.T10.c's worker loader, built under the Content Security Policy unchanged, as R04.T10.a ruled
  on 2026-09-30). Nothing of galaxy plans 12 or 14
  and no session is needed: the spike runs on a scene built by hand.
- **Brainstorm sections covered** (by heading, in [the
  brainstorm](../../brainstorming/rendering-and-planets.md)): step 3 of [Suggested order of
  attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack), the descent
  spike, in full; [The geometry: a quadtree on a cube
  sphere](../../brainstorming/rendering-and-planets.md#the-geometry-a-quadtree-on-a-cube-sphere) in
  full; the level-selection, collision-level and bound rules of [Level-of-detail consistency, and
  why collision
  agrees](../../brainstorming/rendering-and-planets.md#level-of-detail-consistency-and-why-collision-agrees)
  as the geometry needs them; the 2 m band limit and 0.5 m spacing of [The line between truth and
  decoration](../../brainstorming/rendering-and-planets.md#the-line-between-truth-and-decoration);
  of [Performance budget](../../brainstorming/rendering-and-planets.md#performance-budget), the
  terrain and atmosphere rows, the patch-demand paragraph, _τ_, the height-worker budget, the
  height-texture cache row and the third rule (the grounded-body rule and `TERRAIN: DETAIL
LIMITED`); Earth's reference atmosphere from
  [Atmosphere](../../brainstorming/rendering-and-planets.md#atmosphere) (Hillaire 2020, four tables,
  Earth's reference values, the physical Rayleigh coefficients); the "Workers hand heights to the
  render thread" bullet of [Runtime and code
  shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape); item 7 of [What the
  guide must gain](../../brainstorming/rendering-and-planets.md#what-the-guide-must-gain) as the
  view shows it; of [Testing](../../brainstorming/rendering-and-planets.md#testing), the
  level-of-detail selection, grounded-body, culling and precision tests for patches, and the descent
  by hand; [open question 2](../../brainstorming/rendering-and-planets.md#open-questions) in full;
  and the spike paragraph of the single-player brainstorm's [The rendering
  engine](../../brainstorming/single-player-experience.md#the-rendering-engine).

## Goal

When this plan is done the renderer can draw a planet's terrain from orbit to a metre above the
ground without a seam, and the project knows, by measurement, whether the browser can carry the
planets. Terrain is a quadtree of 65 × 65-vertex patches on a cube sphere under S2's quadratic
warp, selected by a pure function of the cameras, the viewports, the quality setting and the
grounded bodies, at a screen-space error of 1 px on the high setting and 2 px on the low; patches
morph in height and position between levels, are culled by frustum and horizon, cached by patch
under a byte budget that never evicts what lies under a grounded body, and are baked by a pool of
height workers, each with its own WebAssembly instance of `hyperion-surface`, which transfer their
results to the render thread. The heights come from a hand-parameterised Earth-sized test planet in
`hyperion-surface`, marked provisional, under the same determinism checks as the sim and replaced by
R09's surface generator. Earth's reference atmosphere is drawn every frame by Hillaire's four
tables, which R08 generalises. A scripted, seeded descent, identical every run, records the
brainstorm's metrics into a results file, and the plan ends with the gate's verdict: the descent
passes or fails at 1080p on the recommended specification, the development machine's RTX 3080, at
its display's measured vsync period (Design note 21), and at 30 fps at 720p on the UHD 620's low
setting (run by the owner), and open question 2's rule, with a native wgpu replay and Dawn's safety
toggles priced, says whether a failure is the browser's. `TERRAIN: STREAMING` and `TERRAIN: DETAIL
LIMITED` annunciate on the view from here on.

## Scope and non-goals

In scope:

- In `hyperion-surface`: the cube sphere on the quadratic warp, patch keys and their neighbours
  across face edges, the finest level per body, the patch bake (heights, the parent-level morph
  channel, analytic normals, the vertex offsets, bounds, skirts), the collision interpolant
  through the finest level, the test planet's height function with analytic gradients, its
  per-level error bound, its goldens on native and both wasm targets, and a wasm entry point for
  the workers.
- In the client, under R02's `view/`: a TypeScript mirror of the cube-sphere mapping, pinned to the
  Rust goldens; patch selection, balance, culling, the grounded-body rule, the morph-hold function,
  streaming priority across views; the patch cache; the height-worker pool; the terrain pass
  through R01's engine adapter; Hillaire's atmosphere at Earth's reference parameters, high and low;
  the two terrain annunciations in R02's label block.
- The spike: the hand-built scene (test planet, its rotation, a Sun-like light), two wireframe
  instrument canvases and a console beside the view, the scripted seeded descent, the metrics
  harness, the results files, the capture and native wgpu replay, the Dawn toggle runs, the UHD 620
  run (the owner's), the discrete run (on the development machine) and the verdict of open
  question 2.
- The low setting of everything above, built alongside the high one.

Non-goals:

- Real terrain. The coarse global pass, the fine synthesis, Dendry channels, craters, the detail
  seed and Knowledge coverage are R09's; drawing generated worlds, materials, readouts, horizon-map
  shadows and the wireframe's depth-only terrain are R10's. The test planet realises none of plan
  14's figures and never reaches a universe.
- Any atmosphere but Earth's. Composition-driven terms, per-gas Rayleigh dispersion, thick
  atmospheres and their baked tables are R08's. This plan's atmosphere is a list of terms with
  Earth's values, in the shape R08 generalises.
- The photorealistic style as a style of `VIEW`, the style switch, AgX, the exposure histogram,
  bloom and glare (R07). The spike's lit view uses R02's tone curve as one full-screen pass and a
  manual exposure; it is a spike scene, not a render style.
- The sky beyond a Sun-like light and R02's stars (R06); clouds, oceans, rings, scatter and
  decoration (R11); still images (R11).
- Flight dynamics and collision response. The camera flies a scripted path; the collision
  interpolant is provided and tested, not used by a flight model, which does not exist.
- The consolidated performance runs and the recorded budget file (R12). This plan records its own
  runs, as every plan records its own benchmarks, in a form R12 folds in.

## Provides

Signatures are sketches. Rust paths are under `hyperion_surface` unless a crate is named;
TypeScript paths under `apps/hyperion/src/renderer/src/`.

### `hyperion_surface::cube` and `geometry`

```rust
pub enum Face { PosX, PosY, PosZ, NegX, NegY, NegZ }       // S2's face order and axes (DN 2)
pub struct FaceUv { face: Face, u: f64, v: f64 }            // u, v in [-1, 1]
pub fn st_to_uv(s: f64) -> f64;                             // S2's quadratic warp
pub fn uv_to_st(u: f64) -> f64;                             // its exact inverse (sqrt only)
pub fn face_uv_to_xyz(f: FaceUv) -> [f64; 3];               // unnormalised; `unit_dir` normalises
pub fn xyz_to_face_uv(p: [f64; 3]) -> FaceUv;               // largest-axis face, ties by DN 2
pub struct PatchKey { face: Face, level: u8, i: u32, j: u32 }
impl PatchKey {
    pub fn root(face: Face) -> Self;
    pub fn to_u64(self) -> u64;  pub fn from_u64(w: u64) -> Result<Self, DecodePatchKeyError>;
    pub fn parent(self) -> Option<Self>;  pub fn children(self) -> [Self; 4];
    pub fn edge_neighbour(self, edge: Edge) -> Self;         // across face edges, DN 2
    pub fn corner_neighbours(self) -> [Option<Self>; 4];     // `None` at a cube corner
    pub fn vertex_dir(self, x: u8, y: u8) -> [f64; 3];      // vertex (x, y) of 65 × 65, unit
}
pub const TEST_PLANET_VERSION: u32 = 1;                     // the surface goldens' header, DN 13
pub const MAX_LEVEL: u8 = 24;
pub const PATCH_QUADS: u32 = 64;                            // 65 × 65 vertices
pub const FINEST_SPACING_M: f64 = 0.5;                      // the brainstorm's 0.5 m
pub const BAND_LIMIT_M: f64 = 2.0;                          // the 2 m band limit
pub fn finest_level(radius_m: f64) -> u8;                   // DN 3: Earth → 19
pub fn vertex_spacing(radius_m: f64, level: u8) -> SpacingRange;   // min, mean, max, metres
```

### `hyperion_surface::test_planet` (provisional, replaced by R09)

```rust
pub struct Spheroid { pub equatorial_radius_m: f64, pub polar_radius_m: f64 }  // the datum, DN 5
pub struct TestPlanet { /* figure: Spheroid, seed, octave table (DN 12), rotation (DN 14) */ }
pub const TEST_PLANET: TestPlanet;                          // WGS 84's figure, hand-parameterised
// HeightSample and LatticeCache move to `hyperion_surface::height` in R09.T4, and `test_planet`
// re-exports them from there
pub struct HeightSample { pub height_m: f64, pub gradient: [f64; 3] }  // above the spheroid, DN 5
pub struct LatticeCache { /* per bake, keyed by u64 lattice keys, order-independent */ }
impl TestPlanet {
    pub fn height(&self, dir: [f64; 3], level: u8, cache: &mut LatticeCache) -> HeightSample;
    pub fn octaves_at(&self, level: u8) -> OctaveSet;        // fixed per level, faded, DN 12
    pub fn level_bound_m(&self, level: u8) -> f64;           // DN 15, derived and checked in T6
    pub fn height_range_m(&self, level: u8) -> (f64, f64);   // bounds for culling, DN 8
}
```

### `hyperion_surface::noise` and `num`

```rust
// noise: 3D improved Perlin gradient noise (DN 12) with its analytic gradient
pub fn gradient_noise(p: [f64; 3], octave: &Octave, cache: &mut LatticeCache)
    -> (f64, [f64; 3]);             // value and gradient; `Octave` holds k, offset, rotation, seed
pub const NOISE_BOUND: f64;          // B, the certified maximum, pinned (DN 15)
pub const NOISE_RMS: f64;            // σ_noise, measured and pinned (DN 15)
// num (src/num.rs): signed-zero-exact min and max (DN 13)
pub fn min(a: f64, b: f64) -> f64;  pub fn max(a: f64, b: f64) -> f64;
pub fn assert_finite(x: f64) -> f64;
```

### `hyperion_surface::patch`

```rust
pub enum VertexPath { BakedOffsets, FaceDifferences }       // DN 4
pub enum NormalScale { Mesh, Double }                       // 65² or 129² normals, DN 5
pub struct BakeOptions { pub vertex_path: VertexPath, pub normals: NormalScale, pub skirt_m: f64 }
pub struct PatchBake {
    pub key: PatchKey,
    pub origin: [f64; 3],            // body-fixed metres; the client's `BodyFixedVec3`
    pub heights: Vec<f32>,           // 65 × 65 × 2: own-level height, morph target (DN 5)
    pub offsets: Option<Vec<f32>>,   // BakedOffsets only: q0, q1 per vertex, from the origin
    pub normals: Vec<f32>,           // octahedral pairs, body-fixed; `rg16float` on the GPU (DN 5)
    pub height_range_m: (f32, f32),
    pub bounding_radius_m: f64,      // about `origin`
}
/// What the bake, the collision interpolant and selection's bound read. `TestPlanet` implements
/// it here; R10 implements it over R09's `Synthesiser` (R10 Design note 4), so nothing below
/// names the test planet.
pub trait HeightSource {
    type Cache;                      // LatticeCache here; R09's SynthCache
    type Error;                      // Infallible here; R09's QueryHeightError
    fn figure(&self) -> Spheroid;    // the datum, DN 5
    fn height(&self, cache: &mut Self::Cache, dir: [f64; 3], level: u8)
        -> Result<HeightSample, Self::Error>;
    fn level_bound_m(&self, level: u8) -> f64;               // DN 15
    fn height_range_m(&self, key: PatchKey) -> (f64, f64);   // culling, DN 8
}
pub fn bake_patch<S: HeightSource>(source: &S, key: PatchKey, opts: &BakeOptions,
    cache: &mut S::Cache) -> Result<PatchBake, S::Error>;
pub fn finest_surface_height<S: HeightSource>(source: &S, dir: [f64; 3],
    cache: &mut S::Cache) -> Result<f64, S::Error>;          // the collision interpolant, DN 6
```

The wasm entry point (T5, in R04's `src/wasm.rs`) exports `bake_patch` to the height workers as
owned typed arrays whose buffers the worker transfers (Design note 11), through the binding shape
R04's loader established, each export under a camel-case `js_name` as R04's `generatorVersion` is
(`bakePatch`, `bandLimitM`, `finestSpacingM`, `levelTable`). T6 adds `level_table()`, which returns, per level from 0 to `MAX_LEVEL`, `level_bound_m`, the two
ends of `height_range_m` and the largest vertex spacing as one `Float64Array`, so that the client's
selection reads the bound at run time rather than from a test fixture.

The testkit gains `hyperion_testkit::golden::f32_digest(values: &[f32]) -> u64` (T5): FNV-1a 64
over each value's little-endian bits, the one place float bits are hashed, as the ban's reason
requires.

### Client: `view/terrain/`

```ts
// cube.ts, patchKey.ts: the mirror, bit-exact against the Rust goldens (DN 1)
function stToUv(s: number): number;
function uvToSt(u: number): number;
function faceUvToDir(face: Face, u: number, v: number): Vec3;
type PatchKey = { face: Face; level: number; i: number; j: number };
function patchKeyString(k: PatchKey): string; // map keys; the u64 word does not fit a number

// view/quality/qualitySetting.ts: the settings, gathered in one place (DN 26); later plans add
// fields to `ViewSettings` and values to `SETTINGS`, never new setting names or second lists
type QualitySetting = "high" | "low";
type TerrainSettings = {
  tauPx: number; // 1 or 2
  renderHeightPx: number | null; // null for the canvas's own size, or 720
  normals: "double" | "mesh";
  vertexPath: "baked-offsets" | "face-differences";
  cacheBytes: number; // about 400 MB or 64 MiB until R10.T14.b measures (DN 10)
};
type ViewSettings = { terrain: TerrainSettings; atmosphere: TableSizes }; // later plans add fields
const SETTINGS: Readonly<Record<QualitySetting, ViewSettings>>; // the one list
const TERRAIN_SETTINGS: Readonly<Record<QualitySetting, TerrainSettings>>; // SETTINGS[s].terrain

// view/terrain/gpu/allocationTally.ts: the engine's allocation and upload tally, over R01's
// `onAllocation` (T11.a); R12 itemises it
interface AllocationTally {
  liveBytes(category: MemoryCategory): number; // bytes
  peakBytes(category: MemoryCategory): number; // bytes, since creation or `resetPeaks`
  uploadedBytesThisFrame(): number;
  resetPeaks(): void;
  subscribe(listener: () => void): () => void; // returns its unsubscribe
}
function allocationTally(engine: RenderEngine): AllocationTally;

// view/engine/memory.ts: members this plan adds to R01's `MemoryCategory` union, by the names
// R12.T3.a reports (R12 Design note 6)
//   "height-cache"       the patch cache's slot buffers and normals array (T11.a)
//   "atmosphere-tables"  the per-planet transmittance and multiple-scattering tables (T12.b)
//   "atmosphere-view"    the per-frame sky-view and aerial-perspective tables (T12.c)

// planet.ts; body-fixed vectors are geometry/vec3.ts's `Vec3` in the body's rotating axes,
// named `BodyFixedVec3` here as an alias; positions reach R02 as `ViewPosition` of kind
// `body_fixed`
type BodyFixedVec3 = Vec3;
// the reference spheroid, the datum (DN 5); the same shape as R07's `BodyFigure`, which R07
// re-exports from here, since this plan lands first
type BodyFigure = { equatorialRadiusM: number; polarRadiusM: number; pole: Vec3 | null };
type PlanetGeometry = {
  figure: BodyFigure;
  finestLevel: number;
  levels: Float64Array | null; // `level_table()`; null for R07's zero-height `mesh` regime
};
function planetGeometry(figure: BodyFigure, levelTable: Float64Array | null): PlanetGeometry;
function bandLimitM(): number; // the wasm module's `bandLimitM` (Rust `band_limit_m()`), for R11 (DN 5)

// bounds.ts, cull.ts
type PatchBounds = { centre: BodyFixedVec3; radiusM: number; minH: number; maxH: number };
function patchBounds(planet: PlanetGeometry, key: PatchKey): PatchBounds;
function inFrustum(b: CameraRelativeBounds, f: Frustum): boolean;
function aboveHorizon(b: CameraRelativeBounds, h: HorizonCone): boolean;

// select.ts, grounded.ts
type ViewSelectionInput = {
  // R02's `CameraPose` has no rotating frame: its position relative to the body's centre and its
  // orientation, rotated into the body-fixed axes with `rotateToBodyFixed` by the caller
  camera: { positionM: BodyFixedVec3; orientation: Quaternion };
  fovXRad: number;
  viewport: ViewSize; // the presented size in device pixels (DN 23)
  weight: number; // 1 primary, 0.25 secondary (DN 24)
  tauPx: number; // the setting's, or a style's own (the wireframe's 4 px)
};
type GroundContact = { positionM: BodyFixedVec3; radiusM: number }; // DN 9
// grounded.ts: callers pass contacts from any source; a swept path (R11.T4.d) is a chain of
// contacts spaced at most one radius apart. The residency time is provisional, T18 sets it.
const FORCED_REGION_RESIDENCY_S: number; // seconds, forced region requested → resident, p99
function isDescending(altitudeM: number, verticalSpeedMps: number): boolean; // DN 9
type SelectionInput = {
  planet: PlanetGeometry;
  views: readonly ViewSelectionInput[];
  setting: QualitySetting;
  grounded: readonly GroundContact[];
};
type SelectedPatch = { key: PatchKey; bounds: PatchBounds; forced: boolean };
type PatchRequest = { key: PatchKey; priority: number; forced: boolean }; // DN 24
type Selection = { patches: ReadonlyMap<string, SelectedPatch>; demand: readonly PatchRequest[] };
function selectPatches(input: SelectionInput): Selection; // pure, DN 7
function morphHold(v: BodyFixedVec3, grounded: readonly GroundContact[]): number; // DN 6
function screenSpaceErrorPx(boundM: number, distanceM: number, view: ViewSelectionInput): number;

// cache.ts
class PatchCache {
  constructor(layout: SlotLayout); /* fixed slots from R10's layout, LRU, pins, DN 10 */
}
function resolveDrawSet(sel: Selection, cache: PatchCache): DrawSet; // ancestor fallback

// workers/: pool.ts, height.worker.ts
type BakedPatch = {
  key: PatchKey;
  generation: number;
  originM: BodyFixedVec3;
  heights: Float32Array; // own level and morph target, interleaved (DN 5)
  offsets: Float32Array | null; // `BakedOffsets` only
  normals: Float16Array; // octahedral, packed in the worker
  heightRangeM: readonly [number, number];
  boundingRadiusM: number;
};
class HeightWorkerPool {
  // the factory makes `new Worker(new URL("./height.worker.ts", import.meta.url), { type: "module" })`;
  // the worker imports its own `.wasm` through `?url`, as R04's probe worker does
  constructor(opts: { workers: number; createWorker: () => WorkerLike });
  request(r: PatchRequest): void;
  reprioritise(demand: readonly PatchRequest[]): void; // each frame: re-score, drop the unwanted
  cancelStale(keep: ReadonlySet<string>): void;
  onBaked(cb: (bake: BakedPatch) => void): () => void;
  postField(bytes: ArrayBuffer): void;
}

// terrainPass.ts, shaders/terrain.wgsl: the pass through R01's adapter
// annunciation.ts; the line reaches the label block through `labelStatements` (displays/view/viewRun.ts)
function terrainAnnunciation(
  draw: DrawSet,
  sel: Selection,
  reference: Selection,
): "TERRAIN: STREAMING" | "TERRAIN: DETAIL LIMITED" | null; // DN 23
```

### Client: `view/atmosphere/`

```ts
// `Rgb` is R02's, imported from `view/photometry` (`readonly [number, number, number]`)
type MediumTerm = {
  name: string;
  density: DensityProfile;
  scattering: Rgb;
  absorption: Rgb;
  phase: PhaseFunction;
}; // the list R08 generalises (DN 16)
const EARTH_REFERENCE: AtmosphereMedium; // Rayleigh, aerosol, ozone; cited constants
class HillaireAtmosphere {
  constructor(engine: RenderEngine, medium: AtmosphereMedium, tables: TableSizes);
  setMedium(medium: AtmosphereMedium): void; // rebuilds the per-planet tables; the sun never does
  drawFrame(view: AtmosphereCamera, sun: SunState): void; // sky-view, aerial perspective, ray march
}
// not R02's `ViewFrame`, which names a coordinate frame: the camera in the body-fixed axes, as
// selection's `ViewSelectionInput.camera`, with its field of view and presented size
type AtmosphereCamera = {
  positionM: BodyFixedVec3;
  orientation: Quaternion;
  fovXRad: number;
  viewport: ViewSize;
};
const TABLE_SIZES: Record<QualitySetting, TableSizes>; // SETTINGS[s].atmosphere, for R08
/** DN 16's lookup inputs at the camera, in `hillaire.ts` (T12.c): r = √(MN) + h, h the geodetic
 * height, and the spheroid normal μ is measured against. R08.T6.f widens the result with the
 * camera's geodetic latitude and gravity scale s = g(φ) ÷ g_ref (R08 Design note 17). */
function atmosphereInputs(camera: AtmosphereCamera, figure: BodyFigure): AtmosphereInputs;
type AtmosphereInputs = { radiusM: number; heightM: number; normal: Vec3 }; // R08 adds two fields
```

### Client: `view/spike/` and the main process

`descentProfile.ts` (the scripted path, pure), `demand.ts` (the predicted demand), `spikeScene.ts`,
`metrics.ts` (frame intervals, pass timestamps, upload and pipeline tallies, memory tallies),
`percentiles.ts`, `pipelineShim.ts`, `capture.ts`, `DescentSpike.tsx`;
`apps/hyperion/src/main/spike.ts` (the switches, tracing, process memory, the results file),
`main/fdinfo.ts` (DRM fdinfo, summed over distinct client IDs) and `main/reduceTrace.ts` (the trace
reducer); the client flag `--descent-spike` with the options of T13.c; the recipes
`just descent-spike` and `just replay`; `tools/gpu-replay/`, a native wgpu replayer outside the
workspace; and `renderer/src/test/countingRenderEngine.ts`, a counting fake of R01's
`RenderEngine` (R01's own `test/fakeRenderEngine.ts`, the `ResilientEngine`'s fake, keeps its
name and role).

### Results and records

`docs/measurements/descent-spike/<date>-<machine>-<setting>.json` with a Markdown summary beside
it, one per recorded run (Design note 18), which R12 folds into its consolidated results; and the
gate's verdict, recorded in this plan's Risks and open points and in the brainstorm's open question
2 by an owner-approved edit.

## Consumes

Names are the owning plans' Provides as they stand when this plan was written, in parallel with
them; the owning plan is authoritative, and where a name differs when this plan runs only the call
sites here change. The plan is re-validated against the code (the `revalidate-plan` skill) before
its first task.

- **R01:** the engine-agnostic interface in `view/engine/types.ts` (`RenderEngine` with
  `createView`, `createMesh`, `createMaterial` and `createMaterialAsync` from a `WgslMaterialSpec`,
  `createPostProcess`, `createCompute` from a `KernelPair`, `createBuffer`, `createTexture`,
  `writeBuffer`, `dispatch`, `readBuffer` and `readTexture` (the CPU readback the smoke assertions
  and the Node tests use), `onPassTimes` delivering `PassTimes` keyed by `FrameSubmission.label`
  (the per-pass GPU time of Design note 18; `bracketed` is always `false`, every pass being the
  adapter's own, R01 Design note 24), `onAllocation` and
  `onFault`; `createRenderTarget` with a `RenderTargetSpec` whose `depth` gives the terrain pass a
  sampled colour and depth; `RenderView`; `FrameSubmission` with its `label`, and `DrawItem` with
  its `offsetFromCameraM`, `textures` and `indirect: IndirectArgs`, whose instance count the
  terrain pass writes with `writeBuffer` for its one instanced draw), `loadRenderEngine` behind its
  dynamic import with its optional `LoadEngineOptions` (the tests' injected `importEngine`),
  `GpuCapabilities`
  (`shaderF16`, `subgroups`, `timestampQuery`), standard WGSL with its compile errors reported by
  material name (R01 Design note 23), one canvas context per view on
  one device, the fault path (`GraphicsFault`, `GraphicsStatusStore`), the refusal of a fallback
  adapter; in the main process `graphicsSwitches` with its `gpuTiming` option and
  `GPU_TIMING_SWITCH`, which lift timestamp quantisation for measurement runs, and
  `mergeSwitchValue`, through which this plan adds Dawn's safety toggles to R01's
  `--enable-dawn-features` and `--disable-dawn-features` lists rather than replacing either (Design
  note 18); `WGSL_CATALOGUE`, in which every pass here is registered so that the offline render test
  and the SwiftShader smoke harness (`just test-render`) render it; `DrawItem.textures`, per-draw
  sampled textures, and `createTexture` and `dispatch` for compute kernels that write 2D and 3D
  storage textures, with their bytes on `onAllocation` (R01 carries both); and `FakeGpu` for tests.
  R01 Design notes 18–20 meet the instanced draw (through `DrawItem.indirect`), the buffer writes,
  the per-pass times and the readback. This plan's three further asks, which T11 and T12.c need,
  are met by R01 under the names asked (R01 Design note 21):
  - storage buffers on materials, read-only in the vertex and fragment stages:
    `WgslMaterialSpec.storageBuffers?: ReadonlyArray<StorageBufferSpec>`, with
    `StorageBufferSpec = { name: string; binding: number }`, and
    `DrawItem.storageBuffers?: Readonly<Record<string, BufferHandle>>` (R01.T8.a and T8.d, checked
    in R01.T9.i);
  - texture writes through the engine, tallied as R01's `uploaded` event, beside `writeBuffer`:
    `RenderEngine.writeTexture(texture, origin, size, data: ArrayBufferView)`, for the normals'
    texture array slot by slot (R01.T8.d);
  - a render target's `depth` accepted in `DrawItem.textures` as a `texture_depth_2d`, read with
    `textureLoad`, so that the deferred aerial-perspective pass, a full-screen draw into the view
    over the terrain's `createRenderTarget`, reads its reversed-Z depth beside its colour and the
    3D table (R01.T8.i, checked in R01.T9.i). R01 refuses a draw that samples the depth of the
    target it renders into (`DepthSelfSample`), so the pass draws into another target or the view.
- **R02:** `view/coords/` (`relativeToCamera`, `narrow`, `originMinusCamera`, `FrameOrigins` with
  `bodyFixedRotation`), `view/camera/` (`CameraPose`, `viewRotation`, `perspectiveReversedInfinite`,
  `pixelSolidAngle`, `rebase`, the presets), `view/depth/` (the reversed-Z policy,
  `transparentLayerOrder`), `view/photometry/` (`illuminanceLx`, `apparentV`, `ExposureControl` with
  `manual`, `preExpose` and its clamp, `HDR_COLOUR_FORMAT`, `toneCurve` and `toneCurve.wgsl`),
  `BodyFixedPosition`, `BodyFixedVector` and `BodyFixedRotation` in `hyperion_sim::coords` and their
  client form (`ViewPosition` of kind `body_fixed`), the wireframe style (`buildWireframeDrawList`,
  `graticule`, `hullEdges`, `TEST_HULL`) for the instrument canvases, `ViewDisplay` with
  `ViewCanvas`, `ViewMarkList` and `ViewLabelBlock`, the shared `geometry/vec3.ts`, and the drafted
  guide items 1, 6 and 7 with their nomenclature (`TERRAIN: STREAMING`, `TERRAIN: DETAIL LIMITED`).
- **R04:** `hyperion-base` (`math`, `units`, `version` with `GENERATOR_VERSION`, and `rng` with
  `Stream`, `Seed`, `ObjectKey`, `DomainTag`, `threefry2x64_20` and `domain_tags!`); the
  `hyperion-surface` skeleton with its `tags` registry, its `clippy.toml`, its relaxed-SIMD
  `compile_error!` and its crate-boundary entries (the surface crate depends on `hyperion-base`
  alone, and on `wasm-bindgen` on the wasm target, so nothing here adds a dependency);
  `wasm32-unknown-unknown` under `wasm-bindgen-test` through `tools/electron-node/node`, both wasm
  targets' fast goldens in `just ci` (`just test-wasm-fast`) and the slow wasip1 suite in
  `just ci-slow`; the testkit's embedded golden arm; the "Hazards across targets" section of the
  sim-determinism skill (R04.T6), which binds every Rust task here; `just gen-surface`, which builds
  the surface crate's module and glue for the client (R04.T10.b); the module loader and probe
  worker in `apps/hyperion/src/renderer/src/wasm/` and the renderer's `worker.format: "es"`
  (R04.T10.c), which this plan's height workers extend; the test conventions of R04 Design note 12,
  which bind every test in `hyperion-surface` here: each test module carries the
  `wasm_bindgen_test as test` import, a test that cannot run on `wasm32-unknown-unknown` (one that
  reads files, such as the golden writers' bless path) sits in a module named `native_only`, and
  every `should_panic` test states `expected` and lives in the crate's `tests/panics.rs`; and the
  CSP ruling (R04.T10.a, 2026-09-30: change nothing): workers are same-origin `*.worker.ts` module
  files, only workers and tests import `generated/surface/`, and the render thread never compiles
  WebAssembly.
- **Galaxy plan 14:** nothing built. `body_fixed_at` (P14.T14.c) does not exist; the test planet
  carries its own rotation in the shape R02's rotation interface takes (Design note 14).
- **Tree facts relied on** (checked 2026-09-29, re-checked 2026-10-02):
  `crates/hyperion-server/src/config.rs`'s `--num-workers` / `HYPERION_WORKERS`, which the spike's
  single-player run caps; the client's commander CLI in `apps/hyperion/src/main/cli.ts`
  (`buildCommand`, `parseClientArgs`, `ClientArgs` of `address` and `port` only, each option read
  by hand), which gains the spike flag; the justfile's `ci` recipe, which gains nothing of the
  spike's (Design note 20).
- **As built, re-validated 2026-10-02 at ce7aeb3** (the call sites below, and the tasks, use these
  names; the sketches above are otherwise unchanged):
  - _R01._ The engine is our own WebGPU adapter in `view/engine/webgpu/` (`createWebGpuEngine`),
    Babylon having been dropped (R01 Design notes 23–24); shaders are standard WGSL: `@group(0)`
    `Frame` from `view/shaders/frame.wgsl`, concatenated as a `?raw` import (as
    `view/wireframe/submit.ts` does), `@group(1)` `Draw` (`offsetFromCameraM : vec3f`, then the
    spec's `UniformSpec`s, checked against any `struct Draw` the source declares), `@group(2)` at
    the declared bindings, entry points `vertexMain`/`fragmentMain` (a kernel's is `main`), each
    registered in `WGSL_CATALOGUE` (`view/engine/catalogue.ts`, `CatalogueEntry` of kind
    `material`, `post-process` or `compute`) with a `displayName`. Textures bind through
    `TextureBindingSpec { name, binding, sampleType?, viewDimension? }`: a target's depth needs
    `sampleType: "depth"`, the normals `"2d"` or `"2d-array"`, the aerial-perspective table
    `"3d"`. `TextureSpec.dimension` is `"2d" | "3d" | "cube"`. `KernelPair` is
    `{ name, reference, subgroup, readback: "bit-exact" | "presentation-only" }`
    (`view/engine/kernels.ts`). Post-processes are `WgslPostProcessSpec` with
    `PostProcessItem { postProcess, uniforms, textures? }`; `PostProcessInput` is `"hdr-colour"`
    alone (the `depth` input was dropped, protocol decision 10), so a pass that reads depth binds
    the target's depth through `textures`. `LoadEngineOptions` (with `importEngine` resolving
    `{ createWebGpuEngine }`, and `gpu`, the entry point a rebuild asks) lives in `types.ts`;
    `loadRenderEngine` returns a `ResilientEngine`. `MemoryCategory` is
    `"render-targets" | "other"` today, and `AllocationEvent`'s `uploaded` kind carries no
    category. A fallback adapter is the `software-adapter` condition with
    `styleAvailability().photorealistic` false, not an error. The device is requested with
    WebGPU's default limits (128 MiB a storage-buffer binding, 256 texture-array layers, 8,192
    texels a 2D side), which bounds the slot layout (T11.a). No `GPUDevice` is reachable outside
    `view/engine/webgpu/` (`engineBoundary.test.ts`); the measurement shims of T14.a and T15.a reach
    it through `useViewEngine`'s `ViewEngineSource` parameter (`displays/view/useViewEngine.ts`),
    by a wrapped `GPU` passed to `requestAdapterOutcome` and as `LoadEngineOptions.gpu`. The pass
    timer times 64 passes a frame; `PassTimes.timer` is `full` only under
    `--hyperion-gpu-timing`. `test/fakeRenderEngine.ts` exists already (R01's fake for
    `ResilientEngine`, `FakeRenderEngine`, `FakeView`, `fakeEngineModule`) and counts nothing, as
    does `test/fakeViewEngine.ts`; this plan's counting fake is a new file (T11.a). The smoke
    harness's catalogue check (`renderer/src/smoke/catalogue.ts`) compiles each material and
    draws only post-processes; a frame of a material is a hand-written check group called from
    `renderer/src/smoke/page.ts`, as R02's `checkWireframe` is. In the main process,
    `graphicsSwitches(options: GraphicsLaunchOptions)`, `applyGraphicsSwitches` (which merges all
    four list switches) and `mergeSwitchValue` are in `main/graphics/switches.ts`;
    `GPU_TIMING_SWITCH` is `"hyperion-gpu-timing"`, declared in `preload/graphicsLaunch.ts`; both
    the timing toggle and `enable_subgroups_intel_gen9` apply only in R01's Linux `vulkan` mode.
  - _R02._ `relativeToCamera(p, camera, origins: CameraOrigins)` and
    `originMinusCamera(mesh: OriginRelative, camera, origins)`, whose `OriginRelative` is
    `{ origin: ViewPosition; offsetsF32 }` (`view/coords/relative.ts`); `Rotation3`,
    `rotation3FromRows`, `rotateToBody` and `rotateToBodyFixed` (`view/coords/rotation.ts`);
    `FrameOrigins.bodyFixedRotation(body): Rotation3 | null`. Every `CameraPose` frame is
    non-rotating (galactic, system, body or craft); there is no body-fixed camera frame, so
    selection rotates the pose into the body-fixed axes itself. R02's `ViewFrame` is a coordinate
    frame's kind, so this plan's atmosphere input is named `AtmosphereCamera` (Provides). A body's
    rotation is a `Rotation3` evaluated at the scene's time (`ViewBody.rotation`), and
    `frameChangeRotationAt` in `view/scenes/frameChange.ts` is the model for turning a pole and a
    period into one. `preExpose` and `HDR_COLOUR_FORMAT` are in `view/photometry/toneCurve.ts`,
    `agx` in `view/shaders/toneCurve.wgsl`; `ExposureControl`'s `manual` holds an
    `ExposureTriple`, and the label block reads `EV100 15.0 MAN` from it. `TEST_HULL` is in
    `view/scene/hull.ts`; a hand-built scene follows `KeptScene` (`view/scenes/kept.ts`) and
    `frameChange.ts`. The label block's steady statements come from `labelStatements(run)` in
    `displays/view/viewRun.ts` (as `ROTATION NOT YET MODELLED` does), not from its label lines.
    `Frustum`, `HorizonCone` and `CameraRelativeBounds` are not R02's: T7.a defines them, and may
    reuse `sphereInFrustum` from `view/wireframe/cull.ts`. The two `TERRAIN:` lines are in the
    guide's nomenclature row, signed off 2026-10-02 on the owner's delegation
    (decisions-r05.md item 5).
  - _R04._ `hyperion-surface` has `src/lib.rs`, `src/tags.rs` (an empty `domain_tags! {}`) and
    `src/wasm.rs` (a private `mod wasm` on the browser target, exporting `generatorVersion` by
    `js_name`); no `tests/`, no `benches/`, and no dev-dependency on `hyperion-testkit` or
    Criterion, which T1.a and T3.c add (dev-dependencies only, so the crate's runtime boundary,
    `hyperion-base` alone, holds). Exports follow the `js_name` convention: `bakePatch`,
    `levelTable`, `bandLimitM` and `finestSpacingM` in JavaScript for the Rust `bake_patch`,
    `level_table`, `band_limit_m` and `finest_spacing_m`. `Stream::open(seed, tag, object)` with
    `ObjectKey::galaxy_item(n)`; a draw number is a word index (`word_at`, `seek`). The testkit's
    `golden!` macro takes a literal name on the browser target; `f32_digest` does not exist yet
    (T5). `rng/tags.golden` is written by `domain_tags_are_pinned` in
    `crates/hyperion-sim/tests/foundation_golden.rs`. `just test-slow <filter>` takes nextest
    filters over `#[ignore = "slow: …"]` tests; `just bench -- <filter>` is a Criterion filter over
    the workspace. The Node-environment module test pattern is `wasm/handleRequest.test.ts`: the
    `.wasm` as a `?inline` import and `initSync({ module })`, since renderer code reads no files
    through `node:*`. `surfaceImports.test.ts` lets only `*.worker.ts` files and tests import
    `generated/surface/` and refuses any import of a `*.worker` module; `tsconfig.worker.json`
    lists the worker-side modules it types (`*.worker.ts`, `wasm/handleRequest.ts`). R04's
    loader is one-shot (it terminates its probe worker after the answer); the height workers are
    the pool's own, and reuse its fault vocabulary (`describeLoadFailure`, `checkGeneratorVersion`).
  - _The main process and the client's entry._ There is no `ipcMain` handler anywhere in
    `src/main/` and no sender-checking helper; the one precedent is the smoke harness's inline
    `event.senderFrame?.url` check in `src/smoke/main.ts`. Configuration reaches the preload only
    through `additionalArguments` read back from `process.argv` (`serverUrlSwitch`,
    `graphicsArguments`). `main.tsx` renders `App` unconditionally; no mode switch exists. Scripts
    are `.mjs` files that run TypeScript through Vite's `runnerImport` (`scripts/placeShip.mjs`);
    the repository's Node floor (22.12) does not strip types unflagged. `docs/measurements/` does
    not exist yet. The vitest `logic` project runs every `*.test.ts` under Node unless listed in
    `DOM_TESTS`; `*.test.tsx` run under jsdom.

## Design notes

"Researched 2026-09-29" marks a note settled by a research agent for this plan; its sources are
given, and the question and answer are in the plan's notes. Figures a task turns into code are
re-checked against the cited source in that task, as the galaxy README's Figures rule requires.

1. **What is Rust and what is TypeScript.** The height function, the cube-sphere mapping, the patch
   bake and the collision interpolant are authoritative and live in `hyperion-surface`, because both
   sides must agree on them bit for bit. Patch selection, culling, balance, the cache, streaming
   priority and the draw submission are presentation and live in the client under R02's
   `view/terrain/`, engine-agnostic and tested without a GPU, as [The engine is kept at arm's
   length](../../brainstorming/rendering-and-planets.md#the-engine-is-kept-at-arms-length) lists
   them. Selection needs the bounds of patches not yet baked, so the client carries a TypeScript
   mirror of the warp and the patch geometry. The warp uses only `+ − × ÷` and `sqrt`, which IEEE
   754 rounds exactly in both languages, so the mirror is pinned bit for bit to a golden file the
   Rust tests write (T2): a mismatch fails a vitest, not a picture. The mirror decides only what to
   draw; every height and vertex position reaching the GPU comes from the worker.
2. **Cube-sphere conventions.** S2's face order and axes (faces 0–5 = +x, +y, +z, −x, −y, −z,
   with S2's per-face (u, v) axes, so that S2's published cell statistics apply unchanged) and its
   quadratic warp, u = (4s² − 1)/3 for s ≥ ½ and (1 − 4(1 − s)²)/3 otherwise, whose inverse is
   s = ½√(1 + 3u) or 1 − ½√(1 − 3u). A point on a face edge or cube corner belongs to the face of
   largest |axis| with ties to the lowest face index, so every direction has one face. A
   `PatchKey` is (face, level, i, j) with `i, j < 2^level`, packed into a `u64` as face (3 bits),
   level (5 bits) and i, j (24 bits each) for cache keys in Rust; the client keys its maps by a
   string, since the word does not fit a JavaScript number. `MAX_LEVEL` is 24, a mean vertex
   spacing of about 9 mm (10 mm at most) on an Earth (0.277 m at level 19 ÷ 2⁵), above any body's
   finest level. Edge neighbours across a face edge rotate (i, j) by
   the face pair's fixed transform, tabulated from S2; a cube corner has three patches of its
   level about it, not four, so a corner patch has seven neighbours in all, as
   [The per-query evaluation](../../brainstorming/rendering-and-planets.md#the-per-query-evaluation)
   notes for craters. **A vertex shared across a face edge takes its direction from the canonical
   face.** Two faces compute the same edge point from different (u, v) and different axes, and the
   rounded results can differ in the last bit, which would break the bitwise agreement of edge
   heights, positions and collision there. So `vertex_dir` maps every vertex on a face edge to the
   canonical face of that edge (the lower face index, the same tie rule as `xyz_to_face_uv`) and
   evaluates it there, and each of the cube's eight corners is evaluated once, on the lowest-index
   of its three faces; both the Rust and the TypeScript mirror follow the rule (after R09's numerics
   design note).
3. **The finest level per body** (researched 2026-09-29). The brainstorm samples the terrain "at 0.5
   m at its finest level" so that the piecewise-linear interpolant keeps the 2 m band limit "to
   within about a third of its amplitude". On the quadratic warp a level's vertex spacing runs from
   0.65 to 1.17 times its mean (computed on a 2,048² face grid). The largest-to-smallest cell-area
   ratio (researched 2026-09-29) is 2.0917 at level 10, from exact cell areas: Van Oosterom–Strackee
   over two triangles a cell, and Girard's excess in 30-digit arithmetic at the extreme cells,
   which agree to about 12 digits. The ratio rises with level (2.076 at 8, 2.087 at 9, 2.094 at 11)
   towards 2.097, the ratio of `s2metrics.cc`'s `kMaxArea` 2.636 to `kMinArea` 8√2/9. The largest
   cell lies on the face diagonal near s = 0.694, and the smallest at an edge's midpoint, not at a
   corner. The 2.082 of `s2coords.h`'s projection table is a statistic of `s2cell_test`'s sampled
   cells, not a closed form, and T1.a's test does not use it. The mesh is triangles, whose diagonal
   sees √2 times the axis spacing. The worst pointwise error of a plane wave on the triangle mesh is
   0.56 of its amplitude at an axis spacing of λ/4 (0.5 m), and a third only at about 0.19 λ. So the
   finest level of a body of radius R is **the shallowest level whose largest vertex spacing is at
   most 0.375 m**, which keeps the brainstorm's "within a third" true in two dimensions and
   satisfies its 0.5 m. For an Earth (6,371 km) that is level 19: spacing 0.18–0.32 m, mean 0.277 m,
   patches of 17.7 m mean edge. Level 18, the reading nearest the brainstorm's "32 m patches", has a
   largest spacing of 0.65 m and fails the claim everywhere. The brainstorm's 160 m demand cap
   becomes about 89 m at level 19. `finest_level(R)` is one function of the radius and
   `FINEST_SPACING_M`, stated in the crate's documentation, as open question 6 requires of the band
   limit; R09 inherits both. Reported for a brainstorm revision in Risks.
4. **Two vertex paths, measured.** The vertex shader never forms the absolute position
   M·d + h·ν (Design note 5; on a sphere, (R + h)·d) in `f32`. The
   brainstorm offers two ways and leaves the choice to measurement, so both are built (T4.b, T11.b)
   and the spike chooses:
   - `BakedOffsets`: the worker computes, in `f64`, each vertex's own-level position q₀ and its
     morph target q₁, both relative to the patch origin, and narrows them to `f32` (|q| is at most
     the patch's size plus its sagitta, so the step is under 10⁻⁶ of the patch, 2 µm at level 19).
     The shader interpolates q₀ to q₁. Cost: 24 bytes a vertex, about 101 KB a patch.
   - `FaceDifferences`: a shared 65 × 65 grid of face offsets, with the height texture's two
     channels, and the direction difference formed from small quantities: with n = a + u e₁ + v e₂
     and n₀ at the patch origin, dir − dir₀ = Δ/|n| + n₀ (|n₀|² − |n|²)/(|n| |n₀| (|n| + |n₀|)),
     where Δ = n − n₀ and |n₀|² − |n|² = −(2 n₀·Δ + |Δ|²), so no difference of nearly equal
     numbers is formed. On the spheroid of Design note 5 the position difference is
     P − P₀ = M (d − d₀) + h (ν − ν₀) + (h − h₀) ν₀, with M linear, so M (d − d₀) keeps the
     identity's small quantities, and ν − ν₀ is formed by the same identity applied to M⁻¹ d.
     Cost: 8 bytes a vertex of height texture, about 34 KB a patch, and more vertex arithmetic.
     T4.b's precision test holds both to under 1 mm against the `f64` positions at level 19 on an
     Earth; the spike records upload bytes, memory and vertex time for each, and T18's defaults
     keep the cheaper. `BakedOffsets` is the default on the high setting until then, since its
     precision holds by construction; the low setting uses `FaceDifferences` from the start,
     because `BakedOffsets` does not fit its 64 MiB cache. By R10's Design note 15, a low-setting
     patch in R10's full layout (heights and parent heights 33.8 kB, mesh-resolution normals
     16.9 kB, class weights 33.8 kB, the horizon map 67.6 kB, the survey mask 4.2 kB) is about
     156 kB, and the sizing rule's about 430 slots fill 64 MiB with nothing to spare; with
     `BakedOffsets` a patch is about 257 kB, and the all-round set at 100 m, about 332 patches,
     needs about 85 MB. T18 may choose `BakedOffsets` for the high setting only.
5. **What a patch carries, on one datum.** Height is height above the body's rotational spheroid,
   R07's reference body, measured along the spheroid's normal (the coordinator's ruling of
   2026-09-29, resting on R07 Design note 19's research, high confidence; R09 and R10 measure from
   the same datum). The test planet's figure is WGS 84's ellipsoid, a = 6,378,137 m and f = 1 ÷
   298.257223563, so c = 6,356,752.314 m (NIMA TR8350.2), with its volumetric radius 6,371.0 km as
   the radius the octave table is scaled to. In the body-fixed frame with z along the pole, M =
   diag(a, a, c) maps a vertex's unit direction d (from `vertex_dir`) to the spheroid point M·d,
   whose outward normal is ν = M⁻¹d ÷ |M⁻¹d|, and a vertex sits at P = M·d + h·ν. A sphere is the
   case a = c. It is height above the datum, not radius, so its `f32` step at ±20 km is about 2 mm.
   `finest_level` takes the equatorial radius, where the spacing is largest (level 19 for the test
   planet, as for 6,371 km). The wasm module also exports `band_limit_m()` and `finest_spacing_m()`
   (`bandLimitM` and `finestSpacingM` in JavaScript), `BAND_LIMIT_M` and `FINEST_SPACING_M`, for
   the client (T5; R11 reads the band limit). The second
   channel is the morph target: at a vertex both of whose indices are even, the parent level's
   height there; at any other vertex, the parent mesh's own linear interpolation of its even
   neighbours, on the parent mesh's triangle diagonal (researched 2026-09-29; holding the parent
   level's height evaluated at an odd vertex would not close the crack, because the parent draws a
   chord there). Normals come from the height function's analytic gradient, taken in the spheroid's
   tangent plane and corrected by ρ ÷ (ρ + h), with ρ the spheroid's radius of curvature in the
   gradient's direction (R ÷ (R + h) on a sphere), stored in the body-fixed frame as octahedral
   pairs, returned by the bake as `f32` and packed by the worker into a `Float16Array` for an
   `rg16float` texture, which is core and filterable in WebGPU (angular error about 0.05°);
   `rg16snorm` would need the optional `texture-formats-tier1` feature (researched for R10,
   2026-09-29), and the surface crate's ban on reading float bits keeps the half-float packing out
   of Rust, where normals are presentation anyway, at the mesh's resolution on the low setting and
   twice it on the high, as the budget's memory table says. Each patch also carries its height range
   and a bounding radius about its origin, which selection's bounds replace once baked. Skirts hang
   from every edge of every patch, not only the cube-face seams, since T-junctions leave hairlines
   too; their depth is the level's error bound plus the `f32` step, and they never enter collision.
6. **Morph, and holding it at zero.** Morph follows CDLOD (Strugar 2009): each vertex blends from
   its own-level position and height to its parent's over the outer part of its level's range, by
   a factor computed from the vertex's unmorphed position, which both patches sharing an edge
   compute identically, so no crack opens. Near grounded bodies the factor is
   k(v) = min(k_CDLOD(v), ramp(d_g(v))), where d_g is the `f64` distance from the vertex to the
   nearest grounded body's footprint: 0 inside the held radius r_g, rising to 1 across one
   finest-level patch beyond it. Because it is still a function of position alone, shared vertices
   agree. The forced-finest region (Design note 9) extends at least one finest patch beyond the
   ramp's end, so that wherever a finest patch meets a coarser neighbour the morph is already 1, as
   CDLOD's crack-freedom requires (researched 2026-09-29; no published precedent for the hold was
   found, and it adds one term to CDLOD's own mechanism). Neighbouring patches differ by at most
   one level, enforced across face edges. `finest_surface_height` is the collision interpolant: the
   finest level's vertex heights, interpolated on the mesh's own triangle diagonal, which is
   exactly what is drawn wherever the morph is zero.
7. **Selection is a pure function.** A level is drawn where its stated error bound subtends at
   most τ pixels: ρ = ε_n · W_px ÷ (2 d tan(fov_h ÷ 2)), with ε_n the level's bound (Design note
   15), W_px the view's width in pixels and d the `f64` Euclidean distance from the camera to the
   nearest point of the patch's bounding volume, height range and curvature bulge included; a
   camera inside the volume refines, down to the finest level. Distance, not view-space depth, so
   turning the camera does not change the selection. τ is 1 px on the high setting and 2 px on the
   low (the brainstorm's lean; CesiumJS's default `maximumScreenSpaceError` is 2 px). The selected
   set is a function of each view's pose, field of view and viewport, the quality setting and the
   grounded bodies alone, and T7's tests hold it to that: no history, no arrival order, no
   hysteresis (the morph is what hides a level change). What is drawn is a second function,
   `resolveDrawSet`, which substitutes the nearest resident ancestor for a patch not yet baked;
   only it sees the cache. Researched 2026-09-29 (Ulrich 2002's chunked LOD; Cesium 3D Tiles'
   `geometricError`).
8. **Culling predicates.** Both run on the CPU in `f64` on camera-relative bounds. The frustum test
   uses the patch's bounding sphere and then its oriented box against the six planes (the far plane
   is infinite and omitted); a patch larger than the frustum and a camera inside a patch's volume
   both pass. The horizon test (Ring 2013, the Cesium horizon-culling method; Cozzi and Ring 2011)
   takes the occluder as the sphere of radius R_occ = c + h_min, the spheroid's polar radius plus
   the planet's lowest possible height, which lies inside the lowest possible surface and so is
   conservative, scales space by 1 ÷ R_occ, and with camera C and vt = P − C calls a point P
   occluded when −vt·C > |C|² − 1 and (vt·C)² ÷ |vt|² > |C|² − 1; a patch is culled only if all
   eight corners of its box, raised to its maximum height, are occluded, which is exact for a convex
   box. Exact tangency is visible, and a camera below R_occ disables the test. Researched
   2026-09-29.
9. **Grounded and descending bodies.** The brainstorm holds the finest level, morph at zero,
   "within a stated radius of every grounded or descending body in view". Until craft exist the
   input is a list, `GroundContact { positionM: BodyFixedVec3, radiusM }`, which the spike
   fills with its scripted craft and R10 and the sessions fill from the scene. A body is
   _descending_ while it is below 1 km above the spheroid under it and moving towards
   it, or while its time to contact at its present vertical speed is under 30 s (provisional
   figures; the spike records how long the forced patches take to become resident, and T18 sets
   them so that the region is resident before contact). That time is published as
   `FORCED_REGION_RESIDENCY_S`, the 99th percentile over the recorded runs from a region's first
   request to its last patch resident. It starts at 30 s, the time-to-contact threshold, as an
   upper bound, and T18 replaces it with the measured figure. `selectPatches` takes its contacts
   from any caller: the spike's craft here, and R10's, R11's and the sessions' later. A contact is
   a sphere, so a swept path, such as the one R11.T4.d derives from a descent, is passed as a
   chain of contacts spaced at most one radius apart, which the forced region covers without a
   gap. The held radius r_g is the body's
   bounding radius plus one finest patch, and the forced region adds the ramp and its one-patch
   margin (Design note 6). On an Earth at level 19 a 20 m craft forces a few tens of 17.7 m
   patches, the brainstorm's "handful". The rule binds every setting and every style: the low
   setting's τ, the patch cache and every view's own tolerance are all overridden inside the
   forced region.
10. **The patch cache, and who owns its layout.** Patches are cached by patch, not by view, in one
    cache per body. R10 owns the cache's sizes, formats and slot layout per setting (its Design note
    15 and T14); this plan owns the cache's behaviour, the eviction rule, the pins and the draw-set
    resolution, and builds the layout R10 specifies from the start, so that nothing is rebuilt when
    R10 lands (reconciled 2026-09-29 with R10's research). The layout: the cache is preallocated as
    fixed slots, the slot count derived from the setting's byte budget and the bytes a patch takes;
    heights and morph targets (and, from R10, the horizon map and the survey mask), unfiltered data,
    live in storage buffers indexed by slot and vertex in WGSL, tight and untiled, while normals,
    which want filtering, live in a 2D texture array or atlas with a one-texel gutter; and the
    terrain is one instanced draw over the shared 65 × 65 index buffer with a slot index per
    instance, which also keeps the CPU's per-draw cost down. R01's device has WebGPU's default
    limits (re-validated 2026-10-02): at most 256 array layers, so the normals take a 2D atlas
    (1,323 low slots or about 1,900 high ones exceed 256 layers), and at most 128 MiB a
    storage-buffer binding, which the heights (33.8 kB a slot, about 64 MB at 1,900 slots) meet
    and `BakedOffsets` on the high budget (101 kB a slot, about 193 MB) does not; the device requests
    up to 1 GiB a binding (decisions-r06-r07.md item 7), with `FaceDifferences` where it cannot
    (Risks, "Default device limits"). This plan's provisional budgets are 64
    MiB on the low setting and about 400 MB on the discrete target (R10 Design note 15: 1.3 × the
    all-round peak at 100 m is about 1,900 slots, 390 MB; the frustum term alone, about 800 slots
    and 165 MB, would fit 256 MiB, and R10.T14.b chooses between them); R10 replaces them with its
    sizing rule, max(1.3 × all-round peak, 3 × frustum peak) over the streaming views plus the
    forced region. Eviction is least recently used among unpinned slots. Two kinds of pin: a patch
    in the forced region is never evicted, whatever the budget, and a patch in the current draw set
    is evicted only after everything unpinned. If the pins alone exceed the slots, the cache reports
    it and the draw falls back to stand-ins outside the forced region, which annunciates `TERRAIN:
STREAMING`. The cache counts the GPU bytes it holds, which the metrics read (Design note 18).
    Horizon maps are R10's: sun-independent (so they are baked once with the patch and never rebaked
    as the sun moves, correcting the brainstorm's "rebaking only as the sun moves"), in 16-bit
    floats, R10's Design note 10; this plan leaves a slot field for them, declared with zero bytes
    until R10 sizes it, and builds none. So this plan's low layout is 33,800 B of heights and morph
    targets (65 × 65 × 2 `f32`) and 16,900 B of mesh-resolution normals (65 × 65 `rg16float`) a
    slot, and 64 MiB holds 1,323 slots; R10's full layout brings that to about 430 (Design note 4).
    For context only (the budgets above stand): the development machine's RTX 3080 has 10 GiB of
    VRAM beside 32 GB of system RAM, and the UHD 620 has no VRAM of its own, drawing on the
    laptop's shared system memory.
11. **The height-worker pool** (researched 2026-09-29). Each worker is a module worker bundled by
    electron-vite as a same-origin file, started with `new Worker(new URL(…, import.meta.url))` and
    `type: "module"`, with `worker.format: "es"` in the renderer's Vite config (set by R04.T10.c) so
    that the build matches the dev server; each loads its own instance of `hyperion-surface` built
    with wasm-bindgen's `--target web`, through `init` on a Vite `?url` import, so no `blob:`, no
    `worker-src` and no CDN enter the policy. There is no `SharedArrayBuffer` and no cross-origin
    isolation, as the brainstorm leans: each instance reads only its own linear memory, so shared
    memory would not remove the copies. The render thread keeps one priority queue and gives each
    worker at most two requests in flight: cancelling a queued request is removing it, priorities
    are re-scored every frame, and a result whose generation is stale is kept in the cache if its
    key is still wanted and dropped otherwise. Per-worker queues were rejected, because their
    cancellations race the bakes. A bake returns owned typed arrays, which wasm-bindgen copies out
    of linear memory once (a few hundred kilobytes, well under a millisecond), and the worker
    transfers their buffers; a WebAssembly memory's own buffer cannot be transferred. A coarse field
    is posted to each worker in turn, cloned, copied into that instance's memory and dropped from
    JavaScript, about 15 MB a worker at rest and twice that briefly. The spike posts a synthetic 15
    MB field to every worker of its pool. The timed runs use the default count below, two on the UHD
    620, and one separate memory run on each machine uses three workers, the brainstorm's "posted to
    three workers", for Design note 21's memory row only, so that the memory is measured before R09
    sends a real field and the timing is not taken with a worker the budget does not have. Default
    worker counts: single-player `clamp(floor(hardwareConcurrency ÷ 4), 1, 3)`, two on the
    i7-8665U's eight threads, matching the budget's two cores (three on the development machine's
    sixteen); a station
    `clamp(floor(hardwareConcurrency ÷ 2) − 1, 1, 4)`, three there; the spike's `--workers` option
    (T13.c) overrides both, and a later settings plan may expose it. A page cannot set a worker's
    thread priority, so the count is the only control. Two findings for R04 and the owner: a
    dedicated worker takes its policy from its own script's response, and a `file://` worker script
    has none, so compilation inside the worker probably does not need `'wasm-unsafe-eval'` while
    compilation on the render thread does; and no documented guarantee was found that module workers
    load from `file://` in Electron 44, so T10's first acceptance is a smoke test of the built app,
    with a privileged custom scheme served through `protocol.handle` as the fallback, which
    Electron's security checklist prefers anyway. _Settled by R04 (2026-09-30): R04.T10.a ruled
    the policy unchanged on these findings, and R04.T10.c's module worker loads from `file://` in
    the built app and from the dev server, checked by hand. Desktop timed runs pin three workers
    (decided 2026-09-30 by a delegated decision; T17)._
12. **The test planet** (researched 2026-09-29). An Earth-sized (WGS 84's figure, Design note 5;
    volumetric radius 6,371 km), dry world whose height
    is a sum of 3D improved Perlin gradient noise octaves (Perlin 2002: the reference
    implementation's 16-entry gradient table, the twelve cube-edge directions padded with (1, 1, 0),
    (0, −1, 1), (−1, 1, 0) and (0, −1, −1), indexed by four bits of one Threefry word, so that a
    uniform index gives an exactly zero-mean gradient, since both the twelve and the four padding
    vectors sum to zero; a multiply-high of a 64-bit word by 12 would not be exactly uniform, since
    2⁶⁴ is not a multiple of 12, and was dropped on R09's research of 2026-09-29; quintic fade)
    evaluated at the spheroid point M·d in metres. The octave of index k has lattice spacing λ_k =
    10,000 km ÷ 2^k and a target RMS σ_k following Earth's topographic spectrum: degree variances
    near ℓ⁻² (Balmino 1993; Rexer and Hirt 2015), which is Hurst exponent 0.5 and a per-octave gain
    of 2^−½, so σ_k = 1,732 m × 2^(−k÷2) for k ≤ 12, and above the ridge–valley scale, where
    landscape spectra steepen (Perron, Kirchner and Dietrich 2008), a gain of ½, σ_k = 27.1 m ×
    2^−(k−12) for k from 13 to 21. The total is σ_h ≈ 2.45 km, Earth's hypsometric standard
    deviation from its bimodal hypsometry (to be confirmed from Earth2014 in T3.b, as the
    brainstorm's own statistics were), and the spectrum puts 0.19% of the variance below 35 km, 106
    m RMS, inside the brainstorm's 0.1–0.2% and 35–125 m. The finest octave is k = 21 (4.77 m), so
    no lattice is finer than the 2 m band limit; Perlin noise is only roughly band-limited (Lagae et
    al. 2010) and its leakage past 2 m is measured and stated (T3.b), not assumed. Each octave has a
    seed-derived offset and a fixed rotation with rational entries, so that octaves do not share
    lattice points and no transcendental function is needed. The large octaves cover the sphere in a
    few cells, so their realised variance is measured over a fixed point set and octaves 0–8 are
    rescaled to σ_h, as the coarse pass rescales to plan 14's. The planet is a single-peaked
    Gaussian field, not Earth's continents and ocean floor, and has no ocean: stated on the spike's
    label block. A switch adds ridged terms, r = 1 − √(n² + ε²) on octaves 8–12, gated by a mask
    from octaves 2–3, with their pinned mean subtracted, because sharp crests are the worst case for
    pops; the spike runs with and without it. Level n evaluates the octaves whose λ_k is at least
    four times the level's largest vertex spacing, fixed per level, and fades the newest across the
    morph zone, as the level-of-detail rules require.
13. **Keys, streams and the golden header.** The lattice corners' words are keyed from integers
    only: a tag of scope `SelfTest`, `selftest.surface.test_planet`, in the surface crate's part of
    R04's split registry (a `SelfTest` tag accepts any key, is never opened by a universe's
    generator, and puts no permanent `surface.*` name in the registry for a planet that will be
    deleted), with `ObjectKey::galaxy_item` carrying the octave's packed (i, j) lattice indices and
    the draw number carrying the octave and k-axis index, as T3.a tabulates. Keys and cache indexes
    are `u64`, never `usize`. A per-bake `LatticeCache`, a dense array per octave over the patch's
    lattice box, hashes each corner once, which takes the per-point cost from about 6 µs to about 2
    µs (estimated from a Threefry block timed at 22.8 ns on the development machine under other
    agents' test load, so provisional; T3.c measures on a quiet machine); it is the caller's, and a
    test proves the bake independent of the order it fills. `num::min` and `num::max` fix the sign
    of zero (the brainstorm's hazard), and every height is asserted finite before it is compared or
    emitted. The test planet is outside every universe, so its goldens carry a `TEST_PLANET_VERSION`
    of their own rather than `GENERATOR_VERSION` (which R04 moves into `hyperion-base`, where R09's
    goldens will read it): a change to the test planet moves no universe. The testkit writes every
    header as `# generator_version = <n>` and every file as `tests/golden/<name>.golden`
    (`crates/hyperion-testkit/src/golden.rs`), so the surface crate's goldens write
    `TEST_PLANET_VERSION` into that header, and for them the bless hint's "bump GENERATOR_VERSION"
    reads "bump `TEST_PLANET_VERSION`", which the module documentation of each golden test says.
    Float bits are printed or hashed only through the testkit: the surface crate's `clippy.toml`
    bans `to_bits` ("float bits are never hashed; print bits through the testkit only"), so the
    bake golden hashes through `hyperion_testkit::golden::f32_digest` (T5).
14. **The test planet turns.** A grounded or hovering camera must see still ground, so the scripted
    path is defined in body-fixed coordinates and turned into the body frame each frame by the
    planet's rotation. Until P14.T14.c's `body_fixed_at` exists the test planet carries a fixed pole
    and a stellar period of 86,164.0989 s (IERS Conventions 2010, eq. 5.14; corrected in T13.a from
    the sidereal 86,164.0905 s, which is measured against the precessing equinox) in the shape R02's rotation
    interface takes, reducing the angle from the integer span since its epoch, as `body_fixed_at`
    will. That shape, as built, is a `Rotation3` evaluated at each frame's time (R02's
    `ViewBody.rotation`, read through `FrameOrigins.bodyFixedRotation`), built with
    `rotation3FromRows` as `frameChangeRotationAt` in `view/scenes/frameChange.ts` builds its own.
    Patch origins are body-fixed `f64` vectors (R02's `ViewPosition` of kind `body_fixed`)
    rotated into the body frame in `f64` once a frame and then differenced against the camera
    (R02's `originMinusCamera`).
15. **The level bound** (researched 2026-09-29). ε_n is a hard analytic bound on the distance
    between the level-n mesh and the finest one: the sum of the omitted octaves' certified maxima
    plus level n's linear-interpolation error, B ÷ σ_noise × Σσ_k over the omitted octaves plus
    h²÷8 × the included octaves' curvature bound. B, the certified maximum of the noise basis, is
    computed offline by grid search with a Lipschitz margin and pinned (expected 1.04–1.1; √(N÷4)
    is a heuristic, not a bound), and σ_noise, its RMS, is measured and pinned. The contract is the
    hard bound, as the brainstorm's test promises: T6's slow test asserts that no sampled
    difference ever exceeds it, and records the ratio of the 99.9th percentile to it per level, so
    that a loose bound cannot silently over-refine. A ratio below a quarter is a finding, not a
    failure. Whether selection then takes a calibrated bound, such as R10's min(hard, 4σ), is
    ruled (2026-10-02, decisions-r05.md item 6): R05 selects by the hard bound everywhere,
    the gate's runs included; R10.T4 applies min(hard, kσ) under the criterion recorded there. A
    sampled maximum never enters R05's contract. The
    brainstorm's patch-to-distance ratio of about five is provisional until this bound exists, and
    T6 records the ratio it implies per level: for fractal relief ε_n falls more slowly than the
    patch size, so the ratio grows towards the finest levels, and demand with it.
16. **Earth's atmosphere** (researched 2026-09-29). Hillaire 2020's four tables, from a list of
    medium terms (density profile, scattering and absorption per channel, phase function), the
    shape R08 generalises. The WGSL is ported from Bevy 0.19's atmosphere (MIT or Apache-2.0,
    already WGSL, already a list of terms, with a ray-marched mode for views from space), with
    sebh's UnrealEngineSkyAtmosphere (MIT) as the numerical reference, both licence notices kept in
    the ported files. Babylon's Hillaire atmosphere (its add-ons package) was not used even before
    R01 dropped Babylon (R01 Design note 24): it assumes forward depth with an infinite far plane,
    has exactly three media, rewrites the directional lights' colour and intensity every frame and
    is marked experimental. Earth's terms, each cited in T12.a:
    - **Rayleigh**, exponential with the US Standard Atmosphere's sea-level scale height, 8.43 km,
      so that the column is Earth's (decisions-r05.md item 3), 4.85, 11.5 and 28.7 × 10⁻⁶ m⁻¹ at 680,
      550 and 440 nm, from Peck and Reeder 1972's refractivity with Bates 1984's King factor at
      288.15 K and 101,325 Pa (the US Standard Atmosphere's sea level, N = 2.547 × 10²⁵ m⁻³);
      recomputed, they match Bucholtz 1995's 4.51 × 10⁻²⁷ cm² at 550 nm to 0.1%. The tutorials'
      5.8, 13.5, 33.1 × 10⁻⁶ are a pure λ⁻⁴ law with no King factor and are not used. The
      recomputation lives in a test; R08 builds the per-gas formula.
    - **Aerosol**, exponential with a 1.2 km scale height, Cornette–Shanks phase with g = 0.584,
      whose mean cosine, 0.65, is AERONET's continental asymmetry at 550 nm (Dubovik et al.
      2002, Table 1); Bruneton 2008's 0.76 has a mean cosine of 0.81 and is not used
      (decisions-r05.md item 1); an optical depth of 0.1 at 550 nm with Ångström
      exponent 1.3 and single-scattering albedo 0.92: Earth's measured continental aerosol
      (AERONET and MODIS climatologies), not Hillaire's reference, whose 5.3 × 10⁻³ at every
      wavelength is 20–40 times cleaner than a typical sky and whose flat spectrum is unphysical.
      Hillaire's values stay available as a comparison mode against his published images.
    - **Ozone**, absorption only, a tent from 10 to 40 km peaking at 25 km, 300 Dobson units, with
      Serdyuchenko et al. 2014's 233 K cross-sections binned at the three wavelengths (0.650, 1.881
      and 0.085 × 10⁻⁶ m⁻¹ at the peak), as Bruneton 2017 does.
    - Top of the atmosphere 100 km above the ground; ground albedo 0.1. (The tables are spherical;
      how they meet the spheroid datum of Design note 5 follows this list.)

    **The spherical tables on the spheroid** (researched 2026-09-29, medium-high confidence).
    Hillaire's tables, sebh's code and Bevy's are parameterised by (r, μ) on a sphere. Every lookup
    therefore takes r = R_osc + h, with h the geodetic height above the WGS 84 spheroid, and
    measures the view and sun zenith cosines against the spheroid normal. R_osc = √(MN) is the
    Gaussian mean radius of curvature at the camera, for the sky-view and aerial-perspective tables,
    and at each sample, for the ray march. The ray march clips against the spheroid shells a + 100
    km and c + 100 km, not a sphere. So Bevy's shaders keep their (r, μ) form, and only their inputs
    change. On Earth this removes the 21.4 km offset between a and c, and the residual is curvature
    mismatch:
    - under 1 m of height error over the 32 km of aerial perspective;
    - about 1 km at a ground horizon's 1,130 km grazing exit, where the density is negligible;
    - grazing optical depth within 0.5%, and sunset transmittance at 440 nm within about 4%.

    Two alternatives were rejected:
    - a geocentric sphere per view puts the limb and the terminator 14–21 km off from orbit, six
      times the Rayleigh density;
    - scaling the polar axis by a ÷ c shears the sun direction by up to 0.1°.

    Sources: Hillaire 2020 §4–5; sebh's UnrealEngineSkyAtmosphere and Bevy 0.19's atmosphere
    (spherical); WGS 84 (NIMA TR8350.2) for M and N. T12.b and T12.c build it.

    The transmittance and multiple-scattering tables depend on the atmosphere alone, per unit
    illuminance, so they are rebuilt when the atmosphere changes, not when the sun moves (a
    correction to the brainstorm's "whenever the atmosphere or the sun changes", harmless since they
    are cheap). The sky-view and aerial-perspective tables are rebuilt every frame. Views from above
    the atmosphere, and terrain beyond the aerial-perspective volume, take a per-pixel ray march.
    Table sizes, high (Hillaire's code): transmittance 256 × 64, multiple scattering 32 × 32,
    sky-view 192 × 108, aerial perspective 32³ reaching 32 km (the figure of Hillaire 2020, Table 2
    and §5.4, and of Bevy, where sebh's reference code's square-root slices reach 128 km). Low: the
    two per-planet tables at full size (128 KB, rarely built), sky-view 128 × 64 with at most 16
    samples, aerial perspective 32 × 32 × 16, the ray march at half resolution with a depth-aware
    upsample and about 16 samples, and aerial perspective applied in one deferred pass to terrain
    alone, which is what the budget's "aerial perspective on terrain only" means. Absolute luminance
    is the tables' value times the Sun's top-of-atmosphere illuminance per channel, scaled so that
    its luminance is the Sun's (from R02's V-magnitude photometry, which gives 1.28 × 10⁵ lx for V =
    −26.76), and the three spectral samples are turned into Rec. 709 by Bruneton's
    spectral-radiance-to-luminance factors rather than read as RGB. The tables stay in their
    per-unit-illuminance form, never in absolute units, and the sun's disc is clamped after
    pre-exposure.

    _Decided 2026-10-06 by a delegated decision (`decision-r05-high-atmosphere.md`):_ the sky view
    and the ray march place their steps quadratically toward each ray's lowest point (the camera,
    the ground or a limb's tangent point), not evenly. A ray from a camera inside the atmosphere
    that falls to its lowest point and climbs out again places its camera side toward the camera
    instead, since the view's own attenuation puts that side's light there (addendum A). The counts
    are those above, except the high setting's sky view, which takes 75 steps (addendum B: from
    60–100 km, a limb ray's narrow source at its tangent point needs them), and each term's density
    is evaluated once a sample (R05.T12.e). Even steps under-sample the aerosol's 1.2 km scale
    height at the dense end. A 100 km vertical ray at 30–32 even steps undercounts its column by
    about 24%, and an `f64` model puts the sky from the ground up to 28% off and the disc from orbit
    up to 7%.

17. **The spike's lit view is not a render style.** R07 owns the photorealistic style, its
    full-screen AgX pass, the histogram and bloom; R02 owns the AgX function itself, `toneCurve`
    and its WGSL twin `agx`. The spike draws its terrain lit by one directional light from a
    Sun-like star 1 au away, shaded Lambertian with a single visual albedo of 0.15 (a hand value,
    labelled), in `rgba16float` with pre-exposure, and maps it to the display by calling R02's
    `agx` in one full-screen pass of its own under a `MAN` exposure of EV100 15 by day, set per
    segment by the script. It is a spike scene inside R02's `VIEW`, and R07 replaces its shading,
    exposure and pass with its own.
    Because the spike's frame lacks the histogram, bloom, clouds, ocean, shadows and scatter, the
    pass criterion (Design note 21) reserves their budget rather than counting it as headroom.
18. **Metrics and the measurement switches** (researched 2026-09-29). A scripted run records, per
    frame and per altitude band, beside the per-level bound ε_n and the ratio k_n that selection's τ
    rests on (T6's `level_table()`), as the brainstorm's step 3 lists: frame intervals from
    presentation times in the trace, falling back to `requestAnimationFrame` timestamps, at the
    50th, 95th and 99th percentiles, with missed and hitching frames counted; GPU time per pass from
    each pass's `timestampWrites` at its start and end, read through R01's `onPassTimes` as
    `PassTimes` keyed by each `FrameSubmission.label` (every pass is the adapter's own, so none is
    bracketed, R01 Design note 24; timestamps inside passes need `--enable-unsafe-webgpu`, which is
    never set); the frames whose pass times the timer dropped, wholly or in part, while all its
    read-back buffers were in flight, counted and left out of every per-frame sum (T14.j); the GPU's
    graphics and memory clocks at 1 Hz beside the memory series, from `nvidia-smi` on NVIDIA and the
    kernel's sysfs on Intel and AMD under Linux, and null with the reason where no unprivileged
    reading exists, as on macOS (T14.k); main-thread time split into our code (`performance.measure`
    spans), the engine adapter (CPU-profiler self time in its lazily imported `engine-*.js` chunk,
    recorded in profiled runs only, below) and idle; patches a second requested, baked and made
    resident, against the predicted demand (Design note 19); upload bytes from the engine's
    `writeBuffer` and `writeTexture` tally (R01's `uploaded` events); pipeline creations after
    warm-up, from a shim on `createRenderPipeline`, `createComputePipeline` and their asynchronous
    forms (installed on the device through a wrapped `GPU` handed to the spike's `ViewEngineSource`,
    since no device is reachable outside R01's adapter), each late one logged with its label, cold
    and warm caches run separately; garbage-collection pauses per thread from V8's GC trace slices;
    and memory at 1 Hz: `app.getAppMetrics()` and the renderer's `process.getProcessMemoryInfo()`,
    the GPU process's DRM fdinfo (`drm-total-*`, `drm-resident-*`, summed over client IDs, since
    ANGLE and Dawn open their own), on NVIDIA `nvidia-smi -q -x`'s process list with the device's
    `memory.used` less a baseline (not `--query-compute-apps`, which lists compute processes only
    and can miss the GPU process; researched for R12, its Design note 6), and the adapter's own
    tally of buffers and textures, against the 1 GB ceiling. The trace is recorded through the
    Chrome DevTools Protocol's `Tracing` domain on the spike window's own `webContents.debugger`,
    which sends only `Tracing` and `IO` commands. It is returned as a stream in Perfetto's protobuf
    format, and decoded and reduced to the results file in the main process (T14.b, T14.h, T14.i).
    Its categories are `devtools.timeline`, `disabled-by-default-devtools.timeline`,
    `disabled-by-default-devtools.timeline.frame`, `disabled-by-default-v8.gc` and
    `blink.user_timing`. Electron's `contentTracing` writes JSON only. Chromium builds a JSON export
    in memory at 17.5–20 MB/s, 4.3–5.7 times the trace buffer's bytes, so a JSON stop took 16–44 s,
    while the protobuf stream (about 1.2 times the buffer) took 2.3–4.8 s (T14.f's measurements,
    2026-10-05). The trace is taken in windows of script time with a 768 MiB `record-until-full`
    buffer each. Windows are at most 120 s, apart from the last, which holds everything from 10 s
    before the approach and flare to the script's end unbroken (at most 300 s). No boundary lies
    within 10 s of a segment change. Chromium keeps one trace session at a time, so windows never
    overlap. The frames from a boundary's stop to 1 s after the next window starts are left out of
    every per-frame figure and counted: at most 5% of the descent after the warm-up and of any
    segment, and no frame excluded in the approach or any later segment. The last boundary's
    exclusion (its gap plus the guard) must end before the approach begins (T14.d, T14.e, T14.g;
    the clause clarified by the orchestrator under delegation, 2026-10-06, T14.f).
    `disabled-by-default-v8.cpu_profiler` and `gpu` are recorded only in a profiled run
    (`--trace-profile on`, with a 1.5 GiB buffer), a diagnostic that is never judged. V8 samples
    each isolate every 100 µs and keeps every sample in the renderer until the trace stops, about
    0.31 MB/s an isolate, so in a timed run it would inflate the renderer's memory in proportion to
    the worker count and interrupt every isolate 10,000 times a second. `gpu` is more than half the
    trace's bytes for two GPU-process slice names (decided 2026-10-04 and 2026-10-05 by delegated
    decisions). Chromium quantises WebGPU timestamps to 65.5 µs by default (Dawn's
    `timestamp_quantization` toggle, mask `0xFFFF0000` on the low word), which the brainstorm's "the
    forced switches make available, uncoarsened" gets wrong; measurement runs alone lift it through
    R01's `gpuTiming` option (`GPU_TIMING_SWITCH`), which the spike flag turns on before `ready`
    (R01 applies it, like `enable_subgroups_intel_gen9`, only in its Linux `vulkan` mode, so a run
    elsewhere records `PassTimes.timer` as `quantized`; accepted 2026-10-02, decisions-r06-r07.md
    item 8: frame intervals are the pass criterion; a quantized run is recorded as such). The safety
    toggles of Design note 22 are merged into both of R01's lists, `--enable-dawn-features` and
    `--disable-dawn-features`, with `mergeSwitchValue`, since appending a second switch would
    replace the first (R01 Design note 2). `gpuTiming` already puts `timestamp_quantization` in the
    disable list, so a run with timing on and safety off carries one
    `--disable-dawn-features=timestamp_quantization,lazy_clear_resource_on_first_use`.
19. **The scripted descent and its predicted demand** (researched 2026-09-29). The path is a pure
    function of script time in body-fixed coordinates over a landing site and approach azimuth
    drawn from the spike's seed, and the frames sample it at their own display times, so the
    trajectory is identical every run while the frame rate is free, which is what streaming must be
    measured against. A second, fixed-step mode samples it at 64 Hz for the CPU-only tests, whose
    selection sequence is then identical bit for bit. Its segments (provisional, T13.a):

    | Segment             | Altitude        | Ground speed      | Vertical speed    | Duration | Brainstorm's formula, high |
    | ------------------- | --------------- | ----------------- | ----------------- | -------- | -------------------------- |
    | Orbit coast         | 400 km          | 7.67 km/s         | 0                 | 60 s     | 4 a second                 |
    | Descent arc         | 400 km to 20 km | 7.67 to 1 km/s    | about −420 m/s    | 900 s    | 4 to 16 a second           |
    | Approach and flare  | 20 km to 300 m  | 1 km/s to 300 m/s | falling to 0      | 120 s    | 16 to 200 a second         |
    | Low fast pass       | 300 m           | 300 m/s           | 0                 | 30 s     | 200 a second               |
    | Slowdown            | 300 m to 200 m  | 300 m/s to 0      | about −2 m/s      | 60 s     | 200 to 3 a second          |
    | Vertical descent    | 200 m to 2 m    | 0                 | −20 m/s           | 10 s     | 29 to 65 a second          |
    | Hover and touchdown | 2 m to 1 m      | 0                 | −0.05 m/s, then 0 | 50 s     | near 0                     |

    The speeds are those between blends: the path joins consecutive segments by blending the
    velocity, a cubic Hermite in position, over the last 5 s of the earlier segment (1 s for a
    segment shorter than 20 s), or, after a level segment (the orbit coast, a lifted low pass),
    over the first 5 s of the later one, so that position and velocity are continuous at every
    boundary. T13.a re-fits the vertical speeds, not the durations, so that the boundary altitudes
    hold with the blends included. A `constant` segment whose ends agree, before a descending one,
    therefore climbs a little and its end blend takes the climb back. The unlifted low fast pass
    climbs 0.076 m/s, to 1.9 m above 300 m. Lifted, as at every real-terrain site recorded so far,
    it is flown level. The climb is kept (decision-r05-coast-speed.md): it is 0.04% of the pass's
    predicted demand. The path is a scripted camera, not a flight, so the blends' accelerations are
    not a craft's.

    The ground speed is the body-fixed speed of the track's point beneath the camera. It is measured
    on the sphere of radius (a + c) ÷ 2 (6,367.4 km on WGS 84) that the track's great circle is laid
    out on. The camera's own body-fixed speed is (R + h) ÷ R times it, about 8.15 km/s at 400 km and
    within 0.3% of it below 20 km. Demand reads the ground speed, since selection's rings sweep the
    ground at the track point's rate. The coast's 7.67 km/s is kept as an upper bound
    (decision-r05-coast-speed.md, 2026-10-04). A circular orbit 400 km up moves at 7.67 km/s
    inertial (√(GM ÷ r), GM = 3.986004418 × 10¹⁴ m³ s⁻²). On the rotating test planet its
    body-fixed ground speed lies between (n − ω) R ≈ 6.75 km/s, prograde equatorial, and
    (n + ω) R ≈ 7.68 km/s, retrograde equatorial, with n = √(GM ÷ r³) and ω the planet's rotation.
    7.22 km/s is a non-rotating planet's figure. The script flies the top of the band whatever the
    approach azimuth, so the path is not a Keplerian orbit. It is circular in the inertial frame
    only when heading west along the equator, and up to 13% above circular speed heading east. The
    arc starts from the coast's ground speed.

    The last column is the brainstorm's (200 · v + 290 · |ḣ|) ÷ h at the segments' ends, with h
    floored at the 89 m cap; the per-level prediction below differs from it and is recorded beside
    it. The low setting's figures are about a ninth.
    The low fast pass is meant to outrun the high setting's workers, as the budget expects, so that
    the moment streaming cannot keep up is part of every run.

    The prediction is the brainstorm's (200 · v + 290 · |ḣ|) ÷ h, re-derived: a level of patch size
    S drawn from k·S to 2k·S, with k about 5, exposes 4k patches along its leading edge, which
    summed over levels gives 8k² v ÷ h, 200 v ÷ h at k = 5 (300 if the parents exposed at a level's
    inner edge are no longer cached); each halving of altitude re-bakes the new finest level's nadir
    disc, 3πk² ≈ 236 patches, so the vertical constant is about 340 rather than 290, within the
    brainstorm's "about". The harness computes it per level from the measured bound, D = Σ_L 4 k_L v
    ÷ S_L + (3π k² ÷ ln 2) |ḣ| ÷ h with k_L = ε_L ÷ (S_L τ θ_px) and θ_px = 2 tan(fov ÷ 2) ÷ W_px,
    with h floored at the cap k S_finest (about 89 m at level 19, not the brainstorm's 160 m). The
    low setting's demand is about a ninth while k exceeds about 3, since k scales as 1 ÷ (τ θ_px)
    and demand as k², and its cap altitude a third; the brainstorm's "the cap as 1/τ" should read 1
    ÷ (τ θ_px). Below k ≈ 3 the quadtree's granularity sets a floor of about 36 patches a level ring
    (R10's count model, 2026-09-29), so the low setting's patch counts are about a quarter of the
    high setting's, not a ninth, and its demand is probably above a ninth too; T13.a's fixed-step
    run measures the ratio rather than assuming it. Two predictions are therefore kept apart: the
    closed form at a fixed k = 5, which must reproduce the brainstorm's worked figures, and the
    per-level D from T6's bounds, which is the one the runs are measured against, at the tolerance
    selection runs at, τ ÷ 1.1 in the terrain pass (T11.c). The measured demand,
    first-time-selected keys a second with the cache's semantics stated, must lie within a factor
    of two of the per-level D (T13.a), and the recorded figure is patches a second sustained
    against it, as the brainstorm's descent test asks.

20. **Where the spike lives.** In the client, behind a command-line flag, `--descent-spike`, that
    opens a `DescentSpike` view in place of the consoles (the flag is parsed by the existing
    commander CLI and reaches the renderer through the preload's existing configuration path), with
    a `just descent-spike` recipe that builds and runs it with the options of T13.c. Nothing of the
    spike runs in `just ci`, whose checks stay the CPU tests and the goldens on three targets. The
    SwiftShader smoke test (`just test-render`, R01's harness: every shader compiles and a frame
    completes with finite texels, with and without `shader-f16` and `subgroups`) gains the terrain
    and atmosphere passes: the catalogue check compiles them, and their frames are hand-written
    check groups in `renderer/src/smoke/`, called from `page.ts` as R02's `checkWireframe` is. It
    stays outside `just ci`, as the roadmap's conventions have it, and the
    tasks that add those passes run it in their own gates (T11.b, T12.b, T12.c). The native replayer
    is `tools/gpu-replay/`, a Rust binary with its own `[workspace]` table outside `crates/`. The
    root manifest's `members = ["crates/*"]` and the replayer's own `[workspace]` table already keep
    wgpu out of `just ci`; the root also gains `exclude = ["tools/*"]`, as a guard against a later
    glob. `just replay` runs it. The spike's single-player runs start the local server with its pool
    capped by `--num-workers 2` and, in a second run, a companion CPU load on two threads standing
    in for the server's arrival work (the sky's census and the coarse pass, neither of which exists
    yet).
21. **The pass criterion** (researched 2026-09-29). The brainstorm's "passes at 1080p60 on the
    discrete target and at 30 fps at 720p on the UHD 620's low setting" is made checkable as frame
    intervals from presentation times over the whole scripted descent after a declared 10 s
    warm-up, and again per segment, so that a failure near the ground cannot be averaged away. The
    UHD 620 run is paced to every second vsync of a 60 Hz display, since uncapped intervals
    alternate between 16.7 and 33.3 ms and their percentiles mean nothing.

    | Criterion             | 1080p, RTX 3080 (T = measured vsync period, 16.68 ms)                                                                        | 720p30, UHD 620 low (T = 33.3 ms)                                                               |
    | --------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
    | 50th percentile       | ≤ T + 0.5 ms                                                                                                                 | ≤ T + 0.5 ms                                                                                    |
    | 95th percentile       | ≤ T + 1 ms                                                                                                                   | ≤ 35 ms                                                                                         |
    | 99th percentile       | ≤ 2T                                                                                                                         | ≤ 2T                                                                                            |
    | Missed frames         | ≤ 1% of intervals above 1.5 T                                                                                                | ≤ 1% of intervals above 1.5 T                                                                   |
    | Hitches               | none above 3 T                                                                                                               | none above 3 T                                                                                  |
    | Headroom              | main thread and GPU pass sum, each ≤ 0.8 T at the 95th percentile                                                            | the same                                                                                        |
    | The rest of the frame | terrain and atmosphere GPU time, summed per frame, within their rows' summed upper ends at the 95th percentile: 6 ms (5 + 1) | 18 ms (14 + 4)                                                                                  |
    | Memory                | GPU resident ≤ 3 GB; above 2 GB a finding                                                                                    | GPU resident ≤ 1 GB, with the 15 MB synthetic field (R09's Earth: about 12 MB) in three workers |

    The memory row reads the brainstorm's "2 to 3 GB" as the roadmap's correction does: a finding
    above 2 GB and a failure above 3 GB. Its "three workers" is the separate memory run of Design
    note 11 on the UHD 620, whose timed runs keep the default count; on the desktop the timed runs
    pin three workers and the row is read from them (T17). The last two rows keep the gate honest
    about what the spike does not draw (Design note 17):
    the budget's other rows must still fit beside terrain and atmosphere.

    _Decided 2026-10-06 by a delegated decision (`decision-r05-high-atmosphere.md`):_ the row is
    judged on the two passes' per-frame sum against their summed upper ends. That sum is the share
    the brainstorm's budget leaves them beside the passes the spike does not draw. Each pass's own
    percentiles are recorded against its estimate as a finding for T19 and R12, never as a verdict:
    terrain 5 ms and atmosphere 1 ms on high, 14 and 4 ms on low. The brainstorm calls every figure
    of its table "an estimate, not a measurement" and expects the first measurements to decide which
    pass gives, and the roadmap's conventions make a missed budget target a finding for R12. At the
    clocks the driver chose, a 1 ms atmosphere could be met only with a march coarser than the low
    setting's (Risks, "The per-frame marches under-sample the dense air").

    Patches a second sustained are recorded against the predicted demand but are not a pass
    criterion: where demand outruns the workers, the view annunciates `TERRAIN: STREAMING`, as the
    budget intends, and the run records how long and where. Percentile and missed-frame conventions
    follow frame-time practice (PresentMon's `MsBetweenPresents`, Chromium's dropped-frame metric);
    the headroom row is the defined meaning of open question 2's "fit with headroom".

    _Decided 2026-10-06 by a delegated decision (`decision-r05-trace-windows-2.md`, addendum B):_ a
    GPU row's percentile is taken over the frames whose every resolve was timed. Frames with a
    dropped resolve are counted, and the verdict must hold whatever their times were. The row passes
    only if it passes with them all above its limit, and fails if it fails with them all below it.
    Otherwise it is not measured, with their count as its reason. A GPU row measures each pass at
    the clocks the driver chose for the spike's load, which is lighter than the full game's (Design
    note 17), so a compute-bound pass within its row is within it at any higher clock. Clocks are
    never pinned: that needs root, changes the machine for every process, has no macOS equivalent
    and is not the player's condition. They are recorded beside the rows, and a row measured at a
    median clock below 90% of the GPU's maximum carries a note saying so.

    _Decided 2026-09-30 by a delegated decision (the hardware decisions, items 1, 4 and 5):_ the
    discrete half runs on the recommended specification, the RTX 3080, not an RTX 4060-class part.
    The criteria are unchanged, with T the display's measured vsync period: 16.68 ms at the
    projector's 1080p 59.94 Hz mode, since it has no exact 60 Hz mode. No margin is scaled for the
    faster part. The memory ceilings are unchanged; the RTX 3080's 10 GiB is shared with the local
    LLM. The brainstorm's discrete column keeps its RTX 4060-class estimates until R12.T10 replaces
    it.

22. **Open question 2's rule, operationalised** (researched 2026-09-29). The verdict (T19) applies
    the brainstorm's rule in order. A failure on the UHD 620 alone redesigns the low setting and
    never triggers a native renderer. On the discrete machine:
    1. The descent runs twice, with Dawn's safety checks on and off. Off is
       `--enable-dawn-features=` R01's `enable_subgroups_intel_gen9` plus `disable_robustness`,
       `skip_validation`, `disable_workgroup_init`,
       `disable_lazy_clear_for_mapped_at_creation_buffer` and
       `disable_polyfills_on_integer_div_and_mod`, with
       `--disable-dawn-features=lazy_clear_resource_on_first_use`, confirmed in effect on
       `chrome://gpu`. The difference prices the browser's safety checks; it does not price
       Chromium's command transport and GPU-process hop, which only the replay does.
    2. The capture shim records, over a fixed span of the descent, every WGSL module, pipeline,
       bind-group and pass descriptor, every upload and each frame's command stream, as JSON with
       binary blobs; `tools/gpu-replay` recreates them in wgpu 30 on a window with FIFO
       presentation at the same resolution and records the same metrics. Every captured module is
       validated by naga first, and a module naga rejects is a finding, not a point for native. The
       replay has no compositor, so it is an upper bound on what native could do.
    3. The rule fires, and the owner is told, only if the replay meets the budget where the browser
       misses it by more than a fifth, or if our CPU time and the GPU's pass time both meet the
       headroom row while delivered frames miss the 95th-percentile row. Otherwise the failure is
       ours to fix, and the verdict names which pass or which thread.

    Existing tools were rejected: webgpu_recorder replays in a browser again, Dawn's wire traces
    need a Chromium build with tracing, and wgpu's own `player` records wgpu, not Dawn.

23. **The two annunciations.** The label block (R02) shows `TERRAIN: STREAMING` while any patch the
    selection asks for is drawn by a coarser resident ancestor, and `TERRAIN: DETAIL LIMITED` while
    the drawn selection is coarser than the reference selection: the same pure selection at τ = 1 px
    at the view's presented resolution (its canvas's size in device pixels, not the render
    resolution), with no depth cap. So the low setting annunciates the second whenever terrain is in
    view, which is honest: its 2 px tolerance, and its 720p render presented upscaled, draw the
    surface below what the camera's position warrants. Where both hold, `STREAMING` shows, because
    it clears by itself and the operator answers it differently. Each is steady text in `--text` on
    its plate, with no status colour and no flashing; a condition must hold for 250 ms before it
    shows and clear for 1 s before it goes, so that a patch arriving mid-frame cannot make the line
    flicker, which also keeps it within the guide's three-flashes-a-second limit. The wording is
    item 7 of the guide edits R02 drafts for the owner; this plan builds to the draft (in the
    guide's nomenclature row, signed off 2026-10-02 on the owner's delegation, decisions-r05.md
    item 5). The line is one of the label
    block's steady statements, from `labelStatements` in `displays/view/viewRun.ts`.
24. **Streaming priority across views.** Demand is the union of every streaming view's selection.
    Each request's priority is the largest over the views that want it of w_view × (ρ ÷ τ), the
    screen-space error excess of the ancestor drawn in its place, with w_view 1 for the primary view
    and 0.25 for a secondary, so that on the UHD 620 a second streaming camera shares the two
    workers at lower priority, as the brainstorm asks. Patches in a grounded body's forced region
    outrank everything. The cache is shared, so views whose cameras are near each other share
    patches.
25. **Normals at twice the mesh's resolution cost four times the gradients.** The brainstorm
    budgets a 65 × 65 patch of heights and gradients at about 40 ms, 10 µs a point, and asks for
    normals at twice the mesh's resolution on the discrete target, which is 129² gradients, about
    four times as many. On the low setting normals are at the mesh's resolution and the budget
    holds. On the high setting the spike measures the bake at both scales and records sustained
    patches a second for each, and T18's defaults keep the doubled normals only if the descent
    still streams within the budget; otherwise the finest levels fall back to mesh-resolution
    normals, a shading loss, never a geometric one, which the results file records. If the
    fallback becomes a default, T18 drafts its label for R02's nomenclature list, for the owner to
    sign off, and builds none until then. Reported as a brainstorm inconsistency.
26. **The low setting, in one place.** `QualitySetting` (`"high" | "low"`) and `SETTINGS`, one
    record from it to `ViewSettings`, in `view/quality/qualitySetting.ts`, hold this plan's values
    per setting (`terrain`, and `atmosphere`, which `TABLE_SIZES` reads); later plans (R07, R08,
    R12) add fields to `ViewSettings` and values to `SETTINGS`, not new setting names or second
    lists. The low setting: τ = 2 px at 720p presented upscaled; normals at the mesh's
    resolution; a 64 MiB patch cache of fixed slots, with the `FaceDifferences` vertex path; the
    atmosphere's low tables with aerial perspective on terrain only and the ray march at half
    resolution; one photorealistic view with the two wireframe instruments; `TERRAIN: DETAIL
LIMITED` whenever terrain is in view. Full depth under grounded bodies on every setting. Each is
    built in the task that builds its high form, as the budget's third rule requires.

27. **Timings need a quiet machine.** The development machine is shared with other agents' test
    runs, and every figure measured on it so far (the Threefry block at 22.8–41.7 ns against the
    repository's 10 ns, and any bench a task runs while other work is going) is provisional. Every
    benchmark and every recorded run in this plan (T3.c, T16, T17 and the replays) is
    taken on a quiet machine: no other test, build or agent running, the load average under 1
    before the run starts and recorded with it, and the CPU governor recorded. A figure taken
    otherwise is marked provisional in its results file and does not count towards the verdict.
    Windows keeps no load average, so a run there is always provisional, its quiet rule unchecked;
    only Linux has a cpufreq governor, which is null with its reason elsewhere (T20).

## Tasks

T1 to T6 are Rust in `crates/hyperion-surface/` and a chain: T1.a, then T1.b and T3.a side by side,
then T2 and T3.b (which reads T1.b's spacing), T3.c, T4.a, then T4.b and T4.c side by side, T5 and
T6. T7 to T9 are pure TypeScript and follow T2 and T6, whose level table they read. T10 needs T5,
T8 and R04's WebAssembly load. T11 needs T4, T7, T8 and T10, and R01's answers to this plan's asks
(Consumes); T12 needs only R01 and R02, and T12.c the depth-texture ask, and can run beside
everything from T1; T12.d, the owner's licence ruling, blocks nothing. T13 and T14 need T9 to T12.c.
T15 follows T14. T16 and T17, the runs, need everything before them; T18 follows both, and T19
closes. Every Rust task runs under the sim-determinism skill's "Hazards across targets" section
(R04.T6) and is audited by the determinism auditor; every renderer task that touches the label
block follows the `console-ux` skill.

Refined on re-validation (2026-10-02), from the code each task touches:

- T7.a's `null`-table path and T7.b's `qualitySetting.ts` are what R07 and R06 wait on; T7.a needs
  T6's golden only for its pinned-table test.
- T8 needs only T7.b's `Selection` type; T10.a only T7.d's `PatchRequest` and T8's cache. T9 needs
  T7 and T8.
- T11.a needs T8's `SlotLayout` and nothing of T10; T11.b needs T4.b's golden and T11.a; T11.c
  needs T7, T10 and T11.b.
- T12.a and T12.b need nothing of this plan. T12.c also needs T7.b (it fills
  `SETTINGS[s].atmosphere`) and T11.a (its tests use the counting fake).
- T13.a needs only T6, T7 and T8 (its fixed-step run is CPU-only). T13.b needs T7.c, T11.c and
  T12.c. T13.c needs T10.b and T13.b for its `--smoke` acceptance.
- T14.b and T14.c are main-process code that needs no renderer task and may land before T13.c,
  the first of them creating `main/spike.ts` (T13.c then adds its IPC handlers). T14.a needs T11.a
  and T13.b. T15.a needs T13.c (`--capture`) and T14.a's shim seam; T15.b needs only T15.a's
  capture format, and T15.c follows T15.b.
- T14.d and T14.e (the trace's windows, 2026-10-04) follow T14.c and T15's first capture. T14.g,
  T14.h and T14.i (the protobuf trace, 2026-10-05) follow them, in that order, and T14.f, the hidden
  runs, comes last. T14.c's visible run, T16 and T17 wait for T14.f. T14.j and T14.k (the GPU rows'
  incomplete frames and clocks, 2026-10-06) follow T14.f, in that order, and T14.c's visible run,
  T16 and T17 wait for them too. They need nothing of T13.a's demand records, which take no trace,
  and those need nothing of them. T12.e (the per-frame marches' steps) and T14.l (the rest of the
  frame as one row), both 2026-10-06, follow T14.k and may run side by side. T14.c's visible run,
  T16 and T17 wait for both to merge. They do not wait for T12.e's by-hand image check and timing.
- T20 (the machine's facts on every platform, 2026-10-06) follows T14.i, since lane D owns
  `results.ts` and the replayer's results until then. Nothing waits on it, but a run or a demand
  record on macOS or Windows reads its machine honestly only after it.
- Steps that need a visible window, a real vsync, a quiet machine or a display change are pending
  by hand for the owner, with their harness and exact commands prepared by the implementing
  lane, which never shows a window on the development machine's display (`:0`): T11.c's and
  T12.c's looks, T13.c's full descent, T14.c's visible run, T15.c's presented replay, T16 (the
  owner's laptop), T17, and every timing that needs a quiet machine (T3.c's bench and the runs of
  Design note 27). Hidden runs (`show: false` with offscreen rendering, or SwiftShader headless,
  under `setsid timeout --kill-after=10` with a fresh `--user-data-dir`, the process group killed
  after) are the lanes' own. T18 and T19 wait on T16 and T17.

Rust files are under `crates/hyperion-surface/` and TypeScript files under
`apps/hyperion/src/renderer/src/` unless a path says otherwise.

### R05.T1 The cube sphere

**R05.T1.a The warp and the faces.** `cube::{Face, FaceUv, st_to_uv, uv_to_st, face_uv_to_xyz,
xyz_to_face_uv}` per Design note 2, with the warp and its inverse cited to S2's `s2coords.h`
(re-check the quadratic forms and the area constants there), and `TEST_PLANET_VERSION = 1` at the
crate root, the header every surface golden writes (Design note 13). The crate gains
`hyperion-testkit` as a dev-dependency here (it has none; the testkit depends on `libm` alone), so
that later tasks do not each edit the manifest. Every test module carries R04's
`wasm_bindgen_test as test` import.

- Files: `src/cube.rs`, `src/lib.rs`, `Cargo.toml` (`[dev-dependencies]`).
- Tests: `uv_to_st(st_to_uv(s))` returns s to one ulp over 10⁴ values and exactly at 0, ¼, ½, ¾ and
  1; `xyz_to_face_uv(face_uv_to_xyz(f))` round-trips to 10⁻¹⁵ in u and v away from face edges;
  every direction has exactly one face, ties on edges and corners resolved by the lowest index;
  lines of constant u lie on planes through the centre (great circles); the ratio of largest to
  smallest exact cell area at level 10 is 2.0917 to a relative 10⁻⁴, which tells it apart from
  levels 9 (2.087) and 11 (2.094), and it is below 2.0968, `s2metrics.cc`'s
  `kMaxArea ÷ kMinArea` (Design note 3); the smallest cell is at an edge's midpoint.
- Acceptance: `cargo test -p hyperion-surface cube::` and
  `cargo clippy -p hyperion-surface --all-targets -- -D warnings` under the crate's own
  `clippy.toml`.

**R05.T1.b Patch keys, neighbours and spacing.** `PatchKey` with its `u64` packing, `parent`,
`children`, `edge_neighbour` across face edges from a tabulated transform per face pair,
`corner_neighbours`, `vertex_dir` (which evaluates a vertex on a shared face edge or cube corner
from its canonical face, the lowest index, per Design note 2, so that every patch sharing it gets
the same bits), `MAX_LEVEL`, `PATCH_QUADS`, `FINEST_SPACING_M`, `BAND_LIMIT_M`, `finest_level` and
`vertex_spacing` (Design note 3).

- Files: `src/cube.rs`, `src/geometry.rs`.
- Tests: `from_u64(to_u64(k)) == k` for random keys, and an out-of-range word is refused; a
  patch's neighbour's neighbour across the same edge is itself, on every edge of every face at
  levels 0, 1, 5 and 24; the 65 vertices of a shared edge are bitwise equal from both sides, across
  face edges included, and a shared face-edge vertex computed through either face's patch key gives
  bit-identical directions on all twelve cube edges at levels 0, 5, 19 and 24; each of the eight
  cube corners gives one bit-identical direction from all three of its faces; a check with the rule
  disabled (evaluating on the patch's own face) finds at least one differing bit, so the test can
  fail; a cube-corner patch has seven neighbours, and its `corner_neighbours` has exactly one
  `None`; `finest_level(6.371e6)` and `finest_level(6.378137e6)` are both 19, and
  `vertex_spacing` there gives a mean of 0.277 m ± 1% and a maximum of at most 0.375 m; the finest
  levels of the Moon (1,737.4 km) and Ceres (469.7 km) are pinned; for 100 radii from 100 km to
  70,000 km the finest level's largest spacing is at most 0.375 m and the level above's is not.
- Acceptance: `cargo test -p hyperion-surface cube::` and `cargo test -p hyperion-surface
geometry::`.

### R05.T2 The cube sphere's golden and the TypeScript mirror

A golden file of the warp at 1,000 values of s, the direction of every vertex of a fixed set of 50
patch keys at levels 0 to 24 on all six faces, the edge-neighbour table and `finest_level` for 20
radii, written as hexadecimal bits by the testkit's writer under `TEST_PLANET_VERSION`'s header
(Design note 13). R04's checks assert it on `wasm32-wasip1` and `wasm32-unknown-unknown`. The
client's `view/terrain/cube.ts` and `patchKey.ts` mirror the Rust functions line for line, and a
vitest reads the same golden file and asserts every value bit for bit (a `Float64Array` over the
parsed bits). The vitest imports the golden as a `?raw` import, as R04's `handleRequest.test.ts`
reads `version.rs`, since renderer code reads no files through `node:*`. The crate's first
integration test follows R04 Design note 12: the `golden!` macro with a literal name (which the
browser target embeds), the bless path in a `native_only` module.

- Files: `tests/cube_golden.rs`, `tests/golden/cube_sphere.golden`, `view/terrain/cube.ts`,
  `view/terrain/patchKey.ts`, `view/terrain/cube.test.ts`.
- Tests: as stated; `patchKeyString` round-trips; the mirror's `finestLevel` agrees for the 20
  radii.
- Acceptance: `just ci` passes with the golden on all three Rust targets and the vitest green;
  `pnpm --filter hyperion exec vitest run view/terrain/cube`; `just bless` leaves
  the file byte for byte unchanged.

### R05.T3 The test planet

**R05.T3.a The noise basis.** Improved Perlin gradient noise in 3D with its analytic gradient
(Design note 12), lattice corners keyed per Design note 13 under the new `SelfTest` tag
`selftest.surface.test_planet` in the surface crate's part of R04's registry, with a table in the
module documentation of which integers go into the object word and the draw number;
`LatticeCache`, a dense array per octave over a bake's lattice box; `num::min`, `num::max` and an
`assert_finite` helper. The certified maximum B and the RMS σ_noise are computed by a slow test
(grid search at 1/256 of a cell with a Lipschitz margin; 10⁶ random points for σ_noise) and pinned
as constants that the test recomputes.

- Files: `src/noise.rs`, `src/num.rs`, the surface crate's tag registry file (`src/tags.rs`, whose
  `domain_tags! {}` gains `TEST_PLANET: SelfTest = "selftest.surface.test_planet";`), and
  `crates/hyperion-sim/tests/golden/rng/tags.golden`, which prints the three registries (R04
  Design note 4; written by `domain_tags_are_pinned` in
  `crates/hyperion-sim/tests/foundation_golden.rs`) and is re-blessed with `just bless` for the
  new name with no `GENERATOR_VERSION` bump, as tag additions have been before. The lattice
  stream is `Stream::open(seed, TEST_PLANET, ObjectKey::galaxy_item(packed))`, its draw number a
  word index (`word_at`).
- Tests: the gradient agrees with a central difference to 10⁻⁶ relative at 10⁴ points; the
  ensemble mean over 10⁶ points is zero within three standard errors; no value exceeds B; noise
  values over a patch-sized lattice box are identical whether the cache is filled in raster,
  reverse or shuffled order (`hyperion_testkit::order::assert_order_independent`; the bake's own
  check is T4.a's); `num::min(0.0, -0.0)` and `num::min(-0.0, 0.0)` are both −0.0 whether or not
  the inputs are known at compile time (`black_box`), and `num::max` of the pair is +0.0 both
  ways; the tag registry's collision check still passes.
- Acceptance: `cargo test -p hyperion-surface noise::` and `cargo test -p hyperion-surface num::`;
  `just test-slow noise_bound_certified` runs it (an `#[ignore = "slow: …"]` test, selected by the
  nextest filter).

**R05.T3.b The octave stack.** `Spheroid` and the test planet's WGS 84 figure (Design note 5,
re-checked against NIMA TR8350.2), `TestPlanet`, `TEST_PLANET`, `height`, `octaves_at`,
`height_range_m`, the octave table of Design note 12 with each figure's citation re-checked
(Balmino 1993 and Rexer and Hirt 2015 for the ℓ⁻² degree variances; Perron, Kirchner and Dietrich
2008 for the roll-over; σ_h from Earth2014, Hirt and Rexer 2015, computed once with pyshtools by
the implementer and recorded with its script in the module documentation), the per-octave offsets
and rational rotations, the fade across the morph zone, the variance rescale of octaves 0–8 over a
fixed point set, and the ridged switch with its pinned mean.

- Files: `src/test_planet.rs`, `src/test_planet/octaves.rs`.
- Tests: the realised σ_h over the fixed point set is the pinned Earth figure within 1%; the
  variance in octaves finer than 35 km is between 0.1% and 0.2% of the total; each octave's sample
  mean is zero within three standard errors, ridged octaves after their mean is subtracted
  included; `octaves_at(n)` is the set of octaves with λ_k at least four times level n's largest
  spacing and never includes one finer than 2 m; heights and gradients are finite on a 10⁵-point
  sample; the spectral leakage above the 2 m band limit, measured by a transform along 1,000
  profiles sampled at 0.125 m, is recorded in the module documentation as an RMS in millimetres (a
  finding if it exceeds 1 cm).
- Acceptance: `cargo test -p hyperion-surface test_planet::`; the statistics under
  `just test-slow test_planet::`.

**R05.T3.c Goldens and cost.** A golden of `height` and its gradient at 500 fixed directions and
levels, ridged on and off, asserted on native and both wasm targets through R04's checks, and a
Criterion bench of a 65 × 65 bake's worth of points with and without the lattice cache.

- Files: `tests/test_planet_golden.rs`, `tests/golden/test_planet.golden`,
  `benches/test_planet.rs`, `Cargo.toml` (Criterion as a dev-dependency from the workspace, and
  `[[bench]] name = "test_planet" harness = false`; the `[lib]`'s `bench = false` stays).
- Tests: the golden; the bench.
- Acceptance: `just ci` green on all three targets; `just bench -- test_planet` reports the
  microseconds a point with its gradient, recorded in the module documentation with the machine and
  its load average, measured on a quiet machine (Design note 27); a figure taken while other work
  shares the machine is marked provisional and re-measured. About 2 µs cached and 6 µs uncached are
  the research estimate and 10 µs the budget; more than 10 µs is a finding for T16. While other
  lanes share the machine the lane records a provisional figure, and the quiet-machine run is
  taken in a quiet window the orchestrator schedules (decisions-r05.md item 7).

### R05.T4 The patch bake

**R05.T4.a Heights, morph targets, normals, bounds and skirts.** `patch::{HeightSource,
BakeOptions, PatchBake, bake_patch}`, with `HeightSource` implemented for `TestPlanet` (its
`height_range_m` per patch is its level's range) and read by the bake as its only height input, so
that R10 points the bake at R09's `Synthesiser` without editing it (R10 Design note 4): own-level
heights; the morph-target channel of Design note 5, on the mesh's diagonal
convention, which the module documentation fixes (each quad split from its (0, 0) corner to its
(1, 1) corner in patch index space); positions on the spheroid datum, P = M·d + h·ν (Design note
5); normals from the analytic gradient as octahedral `f32` pairs at `NormalScale::Mesh` or
`Double`; the height range, the bounding radius and the skirt depths.

- Files: `src/patch.rs`, `src/patch/normals.rs`.
- Tests: two patches sharing an edge have bitwise-equal heights along it, within a face and across
  face edges; at morph 1 every vertex of a child lies on its parent's mesh to the `f64` rounding;
  decoded normals agree with the gradient's to 10⁻⁴ rad; the height range contains every baked
  height; the bake is independent of the lattice cache's fill order; over a fixture
  `HeightSource` of constant height h on the WGS 84 figure every baked position lies on the shell
  at h to the `f64` rounding, and a source's `Err` is returned, not replaced.
- Acceptance: `cargo test -p hyperion-surface patch::`.

**R05.T4.b The two vertex paths.** `VertexPath::BakedOffsets` (q₀ and q₁ per vertex, narrowed from
`f64`) and the `FaceDifferences` formula of Design note 4, implemented in `f64` and as an `f32`
reference performing exactly the operations the WGSL will, in the same order.

A golden records the `f32` reference's inputs (face offsets, patch origin terms, heights) and
outputs, as bits through the testkit's writer, for twelve fixed patches at levels 0, 10 and 19,
two on each face and one at each of three cube corners, so that T11.b's TypeScript emulation is
checked against the same numbers.

- Files: `src/patch/vertex.rs`, `tests/vertex_golden.rs`, `tests/golden/vertex_f32.golden`.
- Tests: at level 19 on an Earth, over 100 patches spread over the six faces and their corners,
  both paths reproduce the `f64` spheroid positions of Design note 5 relative to the origin to
  under 1 mm, on the WGS 84 figure and on a sphere, and the
  naive M·d + h·ν in `f32` does not, which is asserted too, so that the test would have caught
  the naive form; at level 0 both stay within 1 m, where such a patch is seen from 10⁷ m; the
  golden on all three targets.
- Acceptance: `cargo test -p hyperion-surface patch::vertex`; `just ci`.

**R05.T4.c The collision interpolant.** `finest_surface_height`, over any `HeightSource`: the
finest level's vertex heights around the query direction, interpolated on the fixed diagonal.

- Files: `src/patch/collision.rs`.
- Tests: at every vertex of 20 finest-level patches it returns, bitwise, the `f64` height the bake
  computes there before narrowing, and the baked `f32` height to within that value's `f32` step
  (about 2 mm at ±20 km), which is the brainstorm's agreement of collision and picture; no `f32`
  enters the interpolant; inside a quad it is the barycentric interpolation on the stated
  diagonal; on a shared edge it is the same
  from either patch; it never reads a level other than the finest.
- Acceptance: `cargo test -p hyperion-surface patch::collision`.

### R05.T5 The workers' entry point

`bake_patch` exported to JavaScript through the binding shape R04 established, returning owned typed
arrays per Design note 11 (heights and morph targets as one `Float32Array`, offsets as another where
the path needs them, normals as a `Float32Array` that the worker packs into a `Float16Array`, and
the scalars), with a layout comment the TypeScript side mirrors; and a golden of three baked
patches, each array's `hyperion_testkit::golden::f32_digest` and the scalars' bits, the same on
every target. The testkit gains `f32_digest`: FNV-1a 64 over each value's little-endian IEEE 754
bits in order, documented so that a TypeScript twin over a `Float32Array`'s bytes reproduces it
(T10.b); float bits are hashed there and nowhere in the surface crate (Design note 13). The entry
also exports `band_limit_m()` and `finest_spacing_m()` (Design note 5). Each export takes a
camel-case `js_name` (`bakePatch`, `bandLimitM`, `finestSpacingM`), beside R04's
`generatorVersion`, and `just gen-surface` regenerates the client's glue.

- Files: `src/wasm.rs` (R04's private `mod wasm`, under
  `cfg(all(target_arch = "wasm32", target_os = "unknown"))`, extended), `tests/bake_golden.rs`,
  `tests/golden/bake.golden`, `crates/hyperion-testkit/src/golden.rs`.
- Tests: the golden on all three targets; `f32_digest` of a fixed slice against a hand-computed
  FNV-1a value, and a `-0.0` and `0.0` pair giving different digests; the two exported constants
  equal `BAND_LIMIT_M` and `FINEST_SPACING_M`.
- Acceptance: `just ci` green, with the `wasm32-unknown-unknown` run executing the golden under
  Electron's V8 through R04's `node` shim.

### R05.T6 The level bound

`TestPlanet::level_bound_m(n)`, its `HeightSource` bound: the analytic bound of Design note 15 per
level from the octave table, B, σ_noise and the interpolation term, with its derivation in the
documentation; the wasm entry's `level_table()` (Provides; `levelTable` in JavaScript), which hands the client the bound, the
height range and the largest spacing per level at run time; a level-table golden
(`tests/golden/level_table.golden`) that the client's test pins its parsed table against; and the
slow test that measures the mesh-to-mesh distance |S_n(p) − S_f(p)| between each level's mesh and
the finest's at 10⁴ sample points a level, the finest through `finest_surface_height` and level n
through its own vertices on the same diagonal rule.

- Files: `src/test_planet/bound.rs`, `src/wasm.rs`, `tests/level_bound.rs`,
  `tests/level_table_golden.rs`, `tests/golden/level_table.golden`.
- Tests: no sample exceeds the bound at any level; the ratio of the 99.9th percentile to the
  bound is recorded per level, and a ratio below a quarter is reported as a finding (Design note
  15), not failed; per level also recorded (decisions-r05.md item 6): σ_n, the RMS of the omitted
  octaves, √Σσ_k² with the fade weight of the newest included octave, the 99.9th percentile ÷
  4σ_n, and the per-patch maximum over 65 × 65 vertices ÷ 4σ_n for the 10⁴ samples grouped by
  patch; the implied
  patch-to-distance ratio k_n = ε_n ÷ (S_n τ θ_px) at τ = 1 px, 1080p and 60° is written out and
  recorded in the documentation for T13.a; the level-table golden on all three targets.
- Acceptance: `just test-slow level_bound_holds` runs it; `just ci`; the ratios are in the module
  documentation.

### R05.T7 Selection

**R05.T7.a Bounds and culling.** `BodyFigure`, `PlanetGeometry` and `planetGeometry`, built from
the figure and the wasm module's `level_table()` (T6), or with a null table for a zero-height
reference spheroid with no height worker, which R07's `mesh` regime asked for (R07 Design notes 3
and 19) and R07.T9 uses; `bandLimitM`; `patchBounds`, from the mirror and the table's per-level
height ranges; and `inFrustum` and `aboveHorizon` (Design note 8), all in `f64` on camera-relative
vectors from R02's differencing. `Frustum`, `HorizonCone` and `CameraRelativeBounds` are this
task's own types (R02 has none; its `sphereInFrustum` in `view/wireframe/cull.ts` may be reused for
the sphere stage). The pinned-table test reads `level_table.golden` as a `?raw` import.

- Files: `view/terrain/planet.ts`, `view/terrain/bounds.ts`, `view/terrain/cull.ts`, their tests.
- Tests: `planetGeometry` over the table parsed from T6's `level_table.golden` reproduces every
  value bit for bit; with a null table every patch's height range is zero and its bounds enclose
  the spheroid alone, and a sphere (a = c) and the WGS 84 figure both bound every vertex of 100
  sampled patches; a patch larger than the frustum passes; a camera inside a patch's volume
  passes; from 400 km, a patch just beyond the geometric horizon at its maximum height is visible
  and at its minimum height is culled; a 9 km peak beyond the horizon seen from 300 km is visible; a
  camera 2 m above the ground, and one in a valley below the mean radius, cull nothing they can see;
  a camera below the occluder's radius disables the test; exact tangency is visible; root and
  face-sized patches are handled; and a brute-force oracle, rays from the camera to 10⁴ points on
  each patch's surface tested against the occluder, finds no false cull over 1,000 random cameras.
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/cull view/terrain/bounds`.

**R05.T7.b The selection.** `QualitySetting`, `ViewSettings`, `SETTINGS` and `TERRAIN_SETTINGS`
(Design note 26), which selection first reads, with `atmosphere` typed and filled by T12.c;
`selectPatches` and `screenSpaceErrorPx` (Design note 7), with the restricted quadtree enforced
within and across faces. Each view's camera arrives in the body-fixed axes (Provides,
`ViewSelectionInput.camera`): the caller rotates R02's non-rotating `CameraPose` with
`rotateToBodyFixed` and the body's `Rotation3`.

- Files: `view/quality/qualitySetting.ts`, its test, `view/terrain/select.ts`,
  `view/terrain/select.test.ts`.
- Tests of the setting: both records carry every field; the low record's values are Design note
  26's (τ 2 px, 720p, mesh normals, `face-differences`, 64 MiB).
- Tests: every selected patch meets τ or is at the finest level; no two neighbours differ by more
  than one level, across face edges and at cube corners included; the result is identical for views
  and grounded bodies given in any order, called twice, or called after other calls (no module
  state); turning the camera about its own position changes the set only through culling; a
  smaller field of view refines; at the same pose τ = 2 px never selects a finer level than
  τ = 1 px; a camera inside a patch's bounds refines to the finest level.
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/select`.

**R05.T7.c The grounded-body rule.** `GroundContact`, `isDescending` (the descending test of
Design note 9), `FORCED_REGION_RESIDENCY_S`, the forced region and `morphHold`.

- Files: `view/terrain/grounded.ts`, `view/terrain/grounded.test.ts`, `view/terrain/select.ts`.
- Tests: on the high and the low setting, and at a view tolerance of 4 px (the wireframe's, which
  R10 uses), the selection contains the finest-level patches under every grounded body in view,
  with the ramp's margin; `morphHold` is 0 within r_g, 1 beyond the ramp, and continuous; two
  patches sharing an edge compute the same morph factor at every shared vertex near a grounded
  body; the morph is 1 wherever a finest patch meets a coarser one; a chain of contacts along a
  straight swept path, spaced one radius apart, forces a region with no gap between them; the
  selection with contacts given as one list or as the same contacts from two callers concatenated
  is identical; `FORCED_REGION_RESIDENCY_S` is exported and at most the 30 s threshold.
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/grounded view/terrain/select`.

**R05.T7.d Several views.** The union of the views' selections, per-request priorities (Design
note 24) and the `demand` list.

- Files: `view/terrain/select.ts`, `view/terrain/priority.ts`, their tests.
- Tests: two views at one pose request each patch once; a secondary view's requests rank below the
  primary's at equal error; forced-region patches outrank all; the demand list is ordered by
  priority with ties broken by `patchKeyString`.
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain`.

### R05.T8 The patch cache

`PatchCache` over fixed slots (Design note 10), taking a `SlotLayout` (slot count, bytes per slot
and per-slot fields) that R10 later supplies and this task defines with the heights, morph targets
and normals of this plan and an unused horizon-map field; and `resolveDrawSet`: the nearest
resident ancestor stands in for a patch not yet baked, and the draw set marks the stand-ins and
lists each drawn patch's slot index for the instanced draw.

- Files: `view/terrain/cache.ts`, `view/terrain/slotLayout.ts`, their tests.
- Tests: the slot count follows from the budget and the layout (64 MiB and the low layout of Design
  note 10, 50,700 B a slot with the horizon field at zero bytes, give 1,323 slots; R10's 156 kB
  layout gives about 430); no more slots are ever used than exist; a forced-region patch survives
  any eviction pressure; drawn patches are evicted only after unpinned ones; least recently used
  goes first; pins that exceed the slots are reported and fall back to stand-ins outside the forced
  region; an ancestor stands in for a missing patch and is marked; with nothing resident but the six
  roots, the draw set is the roots; a freed slot is reused.
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/cache`.

### R05.T9 The annunciations

`terrainAnnunciation` and its debounce (Design note 23), shown in R02's label block as one of
`labelStatements`'s steady statements, beside `ROTATION NOT YET MODELLED`. `ViewRun` carries no
terrain state today, so the debounced line reaches `labelStatements` as a field of the run or a
second argument, whichever keeps `labelStatements` pure.

- Files: `view/terrain/annunciation.ts`, its test, `displays/view/viewRun.ts` (`labelStatements`)
  and its test, and `displays/view/ViewLabelBlock.tsx`'s test.
- Tests: a stand-in in the draw set gives `TERRAIN: STREAMING` after 250 ms, which clears 1 s after
  the last stand-in goes; the low setting with terrain in view gives `TERRAIN: DETAIL LIMITED`; the
  high setting with everything resident gives neither; both at once show `STREAMING`; the label
  block renders the text in `--text` with no status colour; a condition toggling every frame never
  makes the line flicker (fake timers).
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/annunciation`; the `console-ux`
  skill's lint and contrast scripts pass on the changed files.

### R05.T10 The height-worker pool

**R05.T10.a The pool.** `HeightWorkerPool` over a `WorkerLike` interface (`postMessage`,
`onmessage`, `terminate`) given by a factory, per Design note 11: one priority queue on the render
thread, at most two requests in flight a worker, re-scoring every frame through `reprioritise`,
cancellation by removal, generation tags, results handed to the cache, and the field posted to one
worker at a time.

- Files: `view/terrain/workers/pool.ts`, `view/terrain/workers/messages.ts`, their tests with a
  scripted fake worker (after R04's `test/FakeSurfaceWorker.ts`).
- Tests: requests reach idle workers in priority order; `reprioritise` reorders the queued
  requests and drops those no longer in the demand, without touching those in flight; a cancelled
  request never reaches a worker;
  a result for a stale generation whose key is still wanted is cached, and one no longer wanted is
  dropped; no worker ever has more than two in flight; a field is posted to the second worker only
  after the first acknowledges; a worker that errors is replaced and its requests re-queued;
  `terminate` on unmount leaves no listener behind.
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/workers`.

**R05.T10.b The worker and its WebAssembly.** `height.worker.ts`, beside R04's probe worker in
`renderer/src/wasm/`, whose pattern it follows: it loads the module `just gen-surface` produces
(wasm-bindgen's web target) through `init({ module_or_path })` on a `?url` import of
`generated/surface/hyperion_surface_bg.wasm`, checks the module's `generatorVersion` with R04's
`checkGeneratorVersion`, reports a load failure through R04's `describeLoadFailure` (so a policy
refusal reads `csp-refused`), bakes, and copies and transfers the typed arrays. R04's one-shot
loader (`loadSurfaceModule`) and its probe worker stay as they are; the pool's factory starts the
height workers. It relies on the renderer's `worker.format: "es"`, which R04.T10.c sets. A
Node-environment vitest (the `logic` project) loads the same built `.wasm` as a `?inline` import
with `initSync({ module })`, as R04's `handleRequest.test.ts` does, and checks three bakes against
the native golden's digests (T5), through `f32Digest`, a TypeScript twin of the testkit's FNV-1a
over a `Float32Array`'s bytes, itself checked against the testkit's hand-computed value. In the
built app, started through `loadFile`, a worker bakes one patch and logs the module's content type
and any policy violation, extending R04's own check of its probe worker; T13.c's `--smoke` mode
later makes this a command that exits with a status. If module workers do not load from
`file://`, this task serves the renderer from a privileged custom scheme through `protocol.handle`
instead, with R01's and the owner's agreement, as Design note 11's fallback (R04's probe worker
does load from `file://`, R04.T10.c). The file is `*.worker.ts`, so `tsconfig.web.json` excludes
it, it compiles under `tsconfig.worker.json` and it is one of the files the source guard
(`surfaceImports.test.ts`) lets import `generated/surface/`; the worker-side modules it imports
(`messages.ts`, `f32Digest.ts`, the half-float packing) join `tsconfig.worker.json`'s `include`,
as R04's `wasm/handleRequest.ts` did. `Float16Array` is not in either config's `es2023` lib, so
both gain its declarations (the `esnext.float16` or `es2025` lib, whichever the pinned TypeScript
names).

- Files: `view/terrain/workers/height.worker.ts`, `view/terrain/workers/f32Digest.ts`,
  `view/terrain/workers/heightWasm.test.ts`, `apps/hyperion/tsconfig.worker.json`,
  `apps/hyperion/tsconfig.web.json`.
- Tests: the Node-environment wasm test; the built-app check.
- Acceptance: `just gen-surface && pnpm --filter hyperion exec vitest run view/terrain/workers`;
  the built-app check, recorded in the task's notes: the built app (`just build`, which runs
  `gen-surface`; then Electron on `out/`) bakes a patch in a worker with no policy violation,
  under the unchanged policy (R04.T10.a). The lane runs it hidden (a window never shown, a fresh
  `--user-data-dir`, read over the DevTools protocol, as R01.T14 checked its built client), never
  on the owner's display.

### R05.T11 The terrain pass

**R05.T11.a GPU resources.** Through R01's adapter, with its storage buffers and `writeTexture`
(R01.T8.a and T8.d; if they are not built when this task starts, the task stops and waits on them;
it does not reach past the adapter to the device): one shared index buffer for the 65 × 65 grid on
the fixed diagonal, with skirts; the indirect-arguments buffer of the one instanced draw
(`DrawItem.indirect`), whose instance count `writeBuffer` sets each frame; the slot layout's storage
buffers (heights and morph targets, and for `BakedOffsets` the offsets) and the normals' `rg16float`
texture array, for either normal scale, written slot by slot, all under the `MemoryCategory`
`height-cache`, which this task adds to R01's union in `view/engine/memory.ts` (R12's name); the
per-frame instance data of the one instanced draw, which a per-draw uniform cannot carry: a
storage buffer of one record per drawn patch, read by `instance_index`, holding its slot index,
the `f32` narrowing of the `f64` camera-relative patch origin (R02's `originMinusCamera`, after
rotating the body-fixed origin into the body frame) and its morph range, and a small storage
buffer of the grounded contacts for `morphHold`, both rewritten with `writeBuffer` each frame
(the `Draw` uniform's `offsetFromCameraM` is then zero); `AllocationTally` (Provides) over R01's
`onAllocation`, the live and peak bytes per memory category and the upload bytes a frame through
`writeBuffer` and `writeTexture`, which the metrics and R12 read; and
`renderer/src/test/countingRenderEngine.ts`, a fake of R01's `RenderEngine` over its `FakeGpu`
that counts creations, writes, draws and per-frame allocations, for this task's tests and T11.c's
and T12.c's (R01's `test/fakeRenderEngine.ts` is a different fake, of the `ResilientEngine`'s
inner engine, and is left alone; `test/fakeViewEngine.ts` may be its base).

Against R01's device as built (Consumes, "As built"), which has WebGPU's default limits: the normals
are one 2D `rg16float` atlas with a one-texel gutter (`TextureBindingSpec.viewDimension` `"2d"`),
since 256 array layers hold fewer slots than either budget; every storage buffer stays within
128 MiB a binding, which the `BakedOffsets` offsets at the high budget's about 1,900 slots exceed,
so this task settles that before it builds the high layout (Risks, "Default device limits"). The
tally starts from the engine's creation and ignores a release it never saw created (R01's
`frame uniforms` buffer is made before a caller can listen, R01's Risks, m5); `uploaded` events
carry no category and include the engine's own two uniform uploads a frame, which the tally
counts with the rest.

- Files: `view/terrain/gpu/resources.ts`, `view/terrain/gpu/uniforms.ts`,
  `view/terrain/gpu/allocationTally.ts`, their tests, `view/engine/memory.ts`,
  `renderer/src/test/countingRenderEngine.ts`; for the device limits (decisions-r06-r07.md item 7)
  `view/engine/webgpu/engine.ts`, `view/engine/platform.ts`, `view/engine/types.ts`,
  `test/fakeRenderEngine.ts`.
- Tests: the index buffer's triangles use the (0, 0)–(1, 1) diagonal; an instance record's origin
  is the `f64` difference narrowed once; no buffer exceeds the device's storage-binding limit; a freed slot is overwritten in place, with no new allocation after
  warm-up; the byte tally matches the formats' sizes; `liveBytes` falls on a destruction while
  `peakBytes` does not, and an unsubscribed listener hears nothing; the request never exceeds the
  adapter's limits and is capped at 2³⁰; the device's limits reach `GpuCapabilities`; with
  `defaultLimits` the high layout chooses `FaceDifferences` and no buffer exceeds the binding
  limit; a rebuild onto a FakeAdapter with 128 MiB re-derives the layout without a validation
  error (decisions-r06-r07.md item 7).
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain/gpu view/engine/webgpu/drawing
view/engine/webgpu/engine view/atmosphere`; `just test-render`.

**R05.T11.b The shaders.** `terrain.wgsl`: the vertex stage for both paths (Design note 4), the
morph and its hold (Design note 6), reversed-Z output (R02); the fragment stage's Lambertian shading
of Design note 17 from the decoded normal texture, writing pre-exposed `rgba16float`. A TypeScript
emulation of the `FaceDifferences` vertex arithmetic in `Math.fround`, operation for operation as
the WGSL has it, is checked against T4.b's Rust `f32` reference through T4.b's
`vertex_f32.golden`, which carries both the inputs and the outputs. The shader follows R01's
convention (Consumes, "As built"): `frame.wgsl` concatenated ahead of it, the `Draw` struct as the
spec's uniforms, the slot buffers, instance records, contacts and the normal atlas at `@group(2)`
bindings declared by `storageBuffers` and `textures`, `vertexMain` and `fragmentMain`; it is
registered in `WGSL_CATALOGUE` with a `displayName` in the guide's capitals (`TERRAIN`), whose
display the catalogue's test checks.

- Files: `view/terrain/shaders/terrain.wgsl`, `view/terrain/gpu/vertexEmulation.ts`, its test,
  `view/engine/catalogue.ts`, and a check group in `renderer/src/smoke/` (`terrain.ts`) called from
  `smoke/page.ts`.
- Tests: the emulation agrees with the golden's `f32` outputs bit for bit; the harness's catalogue
  check compiles the material with its external requests cancelled (R01's offline check; there is
  no vitest that compiles WGSL); the terrain check group renders a frame of the test planet from
  400 km and from 10 m with every texel finite, in the `default` and `no-subgroups` variants
  (SwiftShader has no `shader-f16`).
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain`; `just test-render` passes with
  the terrain pass registered in `WGSL_CATALOGUE`.

**R05.T11.c The pass in a view.** The terrain pass driven each frame by selection, the cache, the
pool and the draw set, submitted inside R02's `VIEW` as the spike scene of Design note 17, with
R02's `agx` in one full-screen pass and a `MAN` exposure.

- Files: `view/terrain/terrainPass.ts`, `view/spike/litView.ts`, their tests.
- Tests: with T11.a's counting engine, a frame submits one instanced draw whose instances are the
  drawn patches' slot indices, none for culled ones;
  stand-ins are drawn with their own morph range; the per-frame work allocates no new objects after
  warm-up (a counting fake); the exposure is shown as `EV100 15.0 MAN` in the label block (a
  `manual` `ExposureControl` whose triple gives EV100 15, through R02's `setManual`).
- Acceptance: `pnpm --filter hyperion exec vitest run view/terrain view/spike`; the lane captures
  the test planet from 400 km and from 2 m on the development machine hidden (a target read back
  with `readTexture`, or the view's `readBack`, written to PNG) and checks it is the right way up
  with no crack across a cube-face edge; the same look on screen, by a person, is pending by hand
  for the owner.

### R05.T12 Earth's atmosphere

**R05.T12.a The medium.** `MediumTerm`, `AtmosphereMedium` and `EARTH_REFERENCE` (Design note 16),
each constant with its citation re-checked: the three Rayleigh coefficients (Peck and Reeder 1972,
Bates 1984, the US Standard Atmosphere 1976 for N), the aerosol's optical depth, Ångström exponent,
single-scattering albedo, asymmetry and scale height (AERONET and MODIS climatologies; Dubovik
et al. 2002 for the asymmetry), the ozone column and cross-sections (Serdyuchenko et al. 2014, as Bruneton
2017 bins them), the top of the atmosphere and the ground albedo; Hillaire's reference medium as a
second constant for comparison; and the Sun's per-channel top-of-atmosphere illuminance and the
spectral-radiance-to-luminance factors (Bruneton 2017's method), computed by a script from the
ASTM E-490 solar spectrum and the CIE 1931 matching functions and committed as constants with a
header naming the script, its inputs and their sources. R08.T6.d later replaces `solar.ts`'s
spectral-to-luminance factors with R07's `starIlluminance` and keeps `solar.ts` as a 1% luminance
check.

- Files: `view/atmosphere/medium.ts`, `view/atmosphere/earth.ts`, `view/atmosphere/solar.ts`,
  their tests, and the script: `apps/hyperion/src/tools/solarFactors.ts`, run by
  `apps/hyperion/scripts/solarFactors.mjs` through Vite's `runnerImport`, as `placeShip.mjs` runs
  its tool (the repository's Node floor does not strip types unflagged). The
  script's header names each input's download URL, version and checksum. The input tables are not
  committed until T12.d's ruling; only the derived constants are.
- Tests: the Rayleigh coefficients recomputed in the test from the refractivity and King-factor
  formulas at the three wavelengths agree with the constants to 0.5%, and the 550 nm cross-section
  with Bucholtz 1995's 4.51 × 10⁻²⁷ cm² to 1%; the ozone coefficients recompute from 300 DU and the
  cross-sections; the aerosol's optical depth integrates back to 0.1 at 550 nm; the Sun's
  per-channel illuminance has the luminance R02's photometry gives for V = −26.76.
- Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere`.

**R05.T12.b The per-planet tables.** Transmittance and multiple scattering as compute passes through
R01's adapter, ported from Bevy 0.19's WGSL with its licence notice and checked against sebh's
reference code, rebuilt when the medium changes (not when the sun moves), at the sizes of Design
note 16, allocated under the `MemoryCategory` `atmosphere-tables` (R12's name, added to R01's union
here). An `f64` TypeScript integrator of optical depth along a ray is the oracle. Each kernel is a
`KernelPair` (`reference` WGSL with entry point `main`, `subgroup: null`,
`readback: "presentation-only"`) writing a 2D storage texture made by `createTexture`
(`dimension: "2d"`, storage and sampled usage), dispatched with `ComputeBindings.storage`, and
registered in `WGSL_CATALOGUE`.

- Files: `view/atmosphere/shaders/transmittance.wgsl`, `multiScattering.wgsl`,
  `view/atmosphere/tables.ts`, `view/atmosphere/opticalDepth.ts`, their tests,
  `view/engine/memory.ts`, `view/engine/catalogue.ts`, and a check group in `renderer/src/smoke/`
  (`atmosphere.ts`) called from `smoke/page.ts`.
- Tests: the oracle against closed forms for an exponential atmosphere at the zenith and the
  horizon; in R01's SwiftShader smoke harness, the transmittance table read back with R01's
  `readTexture` agrees with the oracle to 1% at 20 fixed (altitude, angle) texels, and every
  texel of both tables is finite and within [0, 1] for transmittance (the smoke page may read a
  presentation-only texture; nothing else may). (R08.T6.c later stores
  transmittance as optical depth and rewrites this assertion to "every stored optical depth finite
  and non-negative"; R08 keeps this task's file names.)
- Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere`; `just test-render` passes
  with the new property assertions.

**R05.T12.c The per-frame tables and the draw.** Sky-view and aerial perspective every frame, the
per-pixel ray march from above the atmosphere and for terrain beyond the aerial-perspective reach,
the deferred aerial-perspective pass over terrain that reads the reversed-Z depth, and the sky drawn
where depth is at the far plane; the high and low sizes of Design note 16, the per-frame tables
under the `MemoryCategory` `atmosphere-view` (R12's name, added to R01's union here); absolute
luminance from the per-channel solar illuminance, and the sun's disc clamped after pre-exposure.
This task also fills `SETTINGS[s].atmosphere` in T7.b's `view/quality/qualitySetting.ts`. R01's
post-process `depth` input no longer exists (`PostProcessInput` is `"hdr-colour"` alone), so the
deferred aerial-perspective composite reads the terrain target's reversed-Z depth through
`PostProcessItem.textures` (or a full-screen draw's `DrawItem.textures`), declared as a
`TextureBindingSpec` with `sampleType: "depth"` and read with `textureLoad`, beside the 3D table
(`viewDimension: "3d"`); it draws into the view or another target, since R01 refuses a sample of
the target being drawn into (`DepthSelfSample`, `ColourSelfSample`).

- Files: `view/atmosphere/shaders/skyView.wgsl`, `aerialPerspective.wgsl`, `rayMarch.wgsl`,
  `composite.wgsl`, `view/atmosphere/hillaire.ts`, their tests, `view/quality/qualitySetting.ts`,
  `view/engine/memory.ts`, `view/engine/catalogue.ts`, `renderer/src/smoke/atmosphere.ts`.
- Tests: with T11.a's counting engine, the per-planet tables are not rebuilt when only the sun moves and
  are when the medium changes; the low setting allocates the low sizes and applies aerial
  perspective to terrain alone; the smoke harness renders a frame from 400 km and from 2 m at noon
  and at the terminator with every texel finite and none above `rgba16float`'s maximum; the offline
  render test passes; `atmosphereInputs(camera, figure)`, the (r, μ) mapping of Design note 16,
  gives r = √(MN) + h and μ against the spheroid normal at the equator, at 45° and at the pole,
  checked against hand-computed WGS 84 values, and equals the plain sphere's when a = c; an `f64`
  ray march against the spheroid shells gives the grazing optical depth from the ground within
  0.5% of the table's. R08.T6.f later widens `atmosphereInputs` in this file with the camera's
  latitude and gravity scale, which need R08.T3.d's normal gravity; this task builds it as
  Design note 16 has it, and its tests stand.
- Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere`; `just test-render` passes;
  by hand, recorded: the comparison mode beside the published images of Hillaire 2020 from the
  ground at noon and sunset and from orbit, looked at by a person, as the brainstorm keeps image
  comparison. The lane renders those frames hidden and saves them as PNGs beside the references;
  the look is pending by hand for the owner.

**R05.T12.d The solar tables' licences, for the owner.** Draft for the owner, with the roadmap's
other data licences: whether the ASTM E-490 solar spectrum (ASTM's terms) and the CIE 1931
matching functions (CC BY-SA 4.0) may be committed beside `solarFactors.ts`, or only the derived
constants. Nothing is committed before the ruling, and the task ends when the owner signs off.

- Files: this plan's Risks and open points; the script's header, once ruled.
- Acceptance: the ruling is recorded with its date in this plan, and the tables are committed or
  not as it says.

**R05.T12.e The per-frame marches' steps** (added 2026-10-06 by a delegated decision,
`decision-r05-high-atmosphere.md`). In the sky view, the aerial perspective and the ray march, each
term's density is evaluated once a sample, at the sample's height above the datum. `sourceAt` takes
the sample's scattering, extinction and phased scattering in place of calling `mediumAt` again. The
output is not bit-identical; the largest difference is recorded.

The sky view and the ray march split each ray's segment at t\* = clamp(−o·d, t_start, t_end), the
point nearest the centre of the body (or of the sky view's own sphere). A segment with both sides
non-empty shares its steps in proportion to the square root of each side's length, with at least one
a side; a one-sided segment gives all its steps to its side. Each side is placed quadratically
toward t\*, t_k = t\* ± L_side·(k ÷ n_side)². The exception is the camera side of a two-sided
segment that starts at a camera inside the atmosphere, which is placed quadratically toward the
camera, t_k = t_start + L_side·(k ÷ n_side)² (lane C's V1, adopted in addendum A of
`decision-r05-high-atmosphere.md`). Each step is sampled at its midpoint. Its in-scattering is
throughput · S · Δt · g(x) per channel, with x = σ_t·Δt and g(x) = (1 − e^(−x)) ÷ x, computed as 1 −
x·(1/2 − x/6) below x = 0.01 and directly from 0.01 up. The helper is in `common.wgsl`, and the
step's transmittance stays e^(−x). One WGSL helper and one TypeScript twin hold the rule. Another
placement may replace it only if it passes the gate below at no more steps. High takes 75 sky-view
steps (addendum B of `decision-r05-high-atmosphere.md`) and keeps 32 march steps; low keeps 16 and
16, with the march at half resolution. A count rises only if a gate requires it, to the least that
passes, and the task refers back above 90 (sky view) or 48 (march).

- Files:
  - `view/atmosphere/marchSteps.ts` and its test;
  - `shaders/source.wgsl`, `skyView.wgsl`, `aerialPerspective.wgsl` and `rayMarch.wgsl`;
  - `hillaire.ts` (`TABLE_SIZES`'s TSDoc);
  - `renderer/src/smoke/atmosphere.ts`;
  - this plan.
- Tests:
  - **The quadrature gate,** with no GPU:
    - an `f64` twin of the two kernels' quadrature: Earth's terms on a sphere of WGS 84's a, single
      scattering, and a Lambertian ground of 0.15 lit through `opticalDepth.ts`'s sun transmittance;
    - against 4,096 placed steps, themselves within 0.05% of 8,192;
    - over the decision's march rays: from 400 km, the disc and R08's limb heights; to terrain
      beyond 32 km from 20 and 60 km; and, near level, from 0.5–5 km to terrain at 40–150 km. Over
      its sky rays: from 2 m, 1 km, 10 km and 50 km; and, from cameras at 0.5–50 km, the band
      between the local and the visible horizon (0.2, 0.5, 0.8 and 0.98 of the dip, and 0.01° either
      side of the horizontal) and rays 0.01°, 0.1°, 0.5° and 2° past the visible horizon; and, from
      cameras at 60, 80 and 99.999 km: limb rays at tangent heights of 0.84, 2.5, 8.4 and 25 km,
      with the sun at 30° and 90° at the tangent point and azimuths 0, 90 and 180°; and the band at
      0.2, 0.5, 0.8 and 0.98 of the dip, with the sun at 30, 80, 90 and 95° at the camera and
      azimuths 0 and 180°;
    - e = max over channels of |ΔL| ÷ max(L, 10⁻³ L_max), at most 2%;
    - at most 5% for twilight rays: at the segment's lowest point t\*, the sun more than 80° from
      the zenith and the ray within 10° of the horizon, each by more than 10⁻¹² rad;
    - the as-built even placement fails it;
    - low's counts are recorded per family and class beside even placement's at the same count.
      Low's worst e over the whole set is no worse than even's, and each family and class where low
      is worse is listed in Risks.
  - **On SwiftShader** (`just test-render`, both variants): the march and the sky view at their
    setting's counts (and at a frame from 80 km looking at the limb with the sun on its horizon)
    agree with the same kernels at 1,024 steps, in the same run, within the gate's tolerances plus
    1% on high. On low, within the twin's worst e at low's count for the same kernel and class, plus
    1%. The 1,024-step references take the stable step factor.
- Acceptance:
  - `pnpm --filter hyperion exec vitest run view/atmosphere`, `just check lint` and
    `just test-render` pass;
  - by hand, hidden on the RTX 3080 and recorded:
    - **the image check.** Over the 3 captures and 14 check frames, the shipped kernels against
      1,024 steps (2,048 confirming), through the composite: p99 of e ≤ 1% and max ≤ 5%. Even 32/30,
      even 16 and half resolution are recorded beside them;
    - **the timing.** Lane C's 150 s harness, high and low, before (28479b9) and after, in one
      window. The march at matched clocks is at most 5% slower. The sky view's p95 at matched clocks
      is at most 2.6 times its before. The rest of the frame is within 6 and 18 ms at p95. Low's
      atmosphere is no higher than its 3.00 / 3.60 ms before. There is no plateau.
- Suggested subject: `fix(atmosphere): R05.T12.e Step the marches toward the dense air`.

### R05.T13 The spike

**R05.T13.a The scripted descent.** `descentProfile.ts`, the path of Design note 19 as a pure
function of script time in body-fixed coordinates over a landing site and azimuth drawn from the
spike's seed, with its segments as data and its horizontal speeds Design note 19's ground speeds
(the track point's, body-fixed); the test planet's rotation of Design note 14 (the IERS sidereal
day re-checked); and `demand.ts`, the prediction of Design note 19 from T6's per-level bounds.

- Files: `view/spike/descentProfile.ts`, `view/spike/rotation.ts`, `view/spike/demand.ts`, their
  tests.
- Tests: position and velocity are continuous across every segment boundary, blends included; the
  path at a fixed seed is identical call to call, and two seeds give two sites; a hovering pose's
  body-fixed position is constant while its body-frame position moves at ω × r; the closed form at
  a fixed k = 5 reproduces the brainstorm's worked figures to 20% (about 4 a second in low orbit,
  13 at 100 m/s and 1.5 km, 200 at 300 m/s and 300 m, 29 descending at 20 m/s through 200 m, the
  last with the re-derived 340 in place of 290); the per-level D from T6's level table is computed
  for every segment and written out beside it, not asserted against the brainstorm; the fixed-step
  run of the whole descent through `selectPatches` and a simulated cache gives a selection
  sequence whose hash is pinned, and its measured demand, first-time-selected keys a second, lies
  within a factor of two of the per-level D in every segment on both settings; the same run
  records the patch counts and the predicted demand under both selection bounds, the hard ε_n and
  min(hard, 4σ_n), as a second pass of the pure selection with ε as its parameter
  (decisions-r05.md item 6).
- Acceptance: `pnpm --filter hyperion exec vitest run view/spike`.

**R05.T13.b The spike scene.** `spikeScene.ts` (the test planet, its rotation,
`SYNTHETIC_FIELD_BYTES` = 15 MB, the Sun-like light 1 au away with R02's photometry, the scripted
craft as a `GroundContact` once `isDescending` holds) and `DescentSpike.tsx` (the full-window view,
two small wireframe instrument canvases of R02's style drawing the planet's graticule, the craft's
hull and the orbit, and one console panel, all on one device through R01's per-view contexts, with
each canvas's DOM list). The scene is a `ViewScene` built by hand as `view/scenes/frameChange.ts`
builds its rotating body and `body_fixed` lander (provenance `kept`, after `KeptScene`'s
`sceneAt`/`cameraAt` shape), the craft's hull is R02's `TEST_HULL` (`view/scene/hull.ts`), and the
instruments draw through R02's `buildWireframeDrawList` and the `WireframeRenderer` that
`ViewDisplay` uses, each canvas made by `RenderEngine.createView(canvas, name)` on the engine
`useViewEngine` gives.

- Files: `view/spike/spikeScene.ts`, its test, `view/spike/DescentSpike.tsx`, its test.
- Tests: the craft becomes a contact exactly when `isDescending` first holds on the scripted path;
  `DescentSpike` renders its three canvases, each focusable, named and paired with its list, and
  its label block states the test planet as provisional, dry and hand-parameterised.
- Acceptance:
  `pnpm --filter hyperion exec vitest run view/spike/spikeScene view/spike/DescentSpike`.

**R05.T13.c The runner: flags, recipe and preload.** The client flag and every option the runs
need, in `cli.ts`:

```text
--descent-spike [--setting high|low] [--seed <u64>] [--smoke] [--out <dir>]
  [--workers <n>] [--vertex-path baked-offsets|face-differences] [--normals double|mesh]
  [--ridged on|off] [--dawn-safety on|off] [--capture <dir>]
```

`--workers` overrides Design note 11's count, and `--capture` turns on T15.a's shim. The other
options select the variants of T16 and T17. The `just descent-spike` recipe builds the client,
starts a local server with `--num-workers 2` and runs the flag with its arguments, and takes two
options of its own: `--companion-load <threads>`, the CPU load of Design note 20, and
`--cold-cache`, which empties the run's Chromium GPU shader cache first, so that cold and warm
pipeline caches are run separately. The preload gains narrow functions for the spike only, present
when the flag is given and each validated in its handler: start and stop the trace, sample memory
and write the results file (Design note 18). `--smoke` runs 10 s of the descent, writes no results
file and exits with a status, and it also bakes one patch in a worker, which is T10.b's built-app
check as a command.

As built around it (Consumes, "As built"): `ClientArgs` gains the spike's options, read by hand
in `parseClientArgs` as `address` and `port` are; the main process passes them to the window as one
`additionalArguments` switch (such as `--hyperion-descent-spike=<options>`), which the preload
reads back from `process.argv` as `serverUrlFromArgv` does and which decides whether
`HyperionApi` gains its `spike` member; `main.tsx` renders `DescentSpike` in place of `App` when
that member is present. The main process has no `ipcMain` handler yet: `spike.ts` adds the first,
each refusing a sender whose `event.senderFrame` is not the window's own page, after the smoke
harness's inline check in `src/smoke/main.ts`, factored into a helper with its test. `--smoke`
never shows its window (`show: false`, as R01.T14's hidden check of the built client ran; offscreen
rendering as `src/smoke/main.ts` uses it if a hidden window does not render), so the lanes may run
it. The recipe follows `client` and `test-render`: `just build` (which runs
`gen-surface`), the server started as `cargo run -p hyperion-server -- --num-workers 2` in the
background, Electron on `out/` under `setsid timeout --kill-after=10` with a fresh
`--user-data-dir`, and the process group killed at the end. `index.ts` calls T14.b's
`launchSwitches(options, spike)` from `spike.ts` in place of `graphicsSwitches`, with the parsed
`--dawn-safety`, before `ready`, so that a spike launch carries the measurement switches (orchestrator
ruling, 2026-10-03).

- Files: `apps/hyperion/src/main/cli.ts`, its test, `apps/hyperion/src/main/index.ts`,
  `apps/hyperion/src/main/spike.ts` (the IPC handlers and the sender check) and its test,
  `apps/hyperion/src/preload/api.ts`, `apps/hyperion/src/preload/index.ts`, a preload module for
  the spike switch beside `serverUrl.ts`, `apps/hyperion/src/renderer/src/main.tsx`,
  `apps/hyperion/src/renderer/src/test/stubHyperionApi.ts`, `justfile`.
- Tests: the CLI parses the flag and each option, refuses a bad value of each, and refuses every
  spike option given without `--descent-spike`; the spike's IPC handlers refuse a sender that is
  not the window's own frame and arguments that do not validate; the ordinary launch exposes no
  spike function on the preload.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/cli src/main/spike src/preload`;
  `just descent-spike --smoke` exits 0 (hidden); by hand, pending for the owner (a visible window
  on the development machine's display): one full descent on the development machine at the low
  setting, orbit to 1 m in one motion, with the exact command recorded by the lane.

### R05.T14 The metrics harness

**R05.T14.a In the renderer.** `metrics.ts`: frame intervals by `requestAnimationFrame` and
presentation time, per-pass GPU time from R01's `onPassTimes` (by `FrameSubmission.label`, with its
`bracketed` flag recorded) where `timestamp-query` is present, `performance.measure` spans around
our per-frame code, T11.a's `AllocationTally`, a shim over `createRenderPipeline`,
`createComputePipeline` and their asynchronous forms that counts and labels creations after warm-up,
and patches requested, baked and made resident a second, each tagged with the script time and
segment. The shim reaches the device the only way R01 leaves open (Consumes, "As built"): the
spike's `ViewEngineSource` for `useViewEngine` requests its adapter through a wrapped `GPU`
(`requestAdapterOutcome(wrapped)`), whose adapter's `requestDevice` wraps the device it returns,
and passes the same wrapped `GPU` as `LoadEngineOptions.gpu` so that a rebuild after a device loss
is shimmed too. R01's adapter itself is not edited. `PassTimes.timer` is recorded with the times
(`full`, `quantized` or `absent`), and passes beyond the timer's 64 a frame are counted as untimed.

- Files: `view/spike/metrics.ts`, `view/spike/pipelineShim.ts`, their tests. The percentiles,
  missed-frame and hitch counts are the main process's (`main/results.ts`, T14.c), so there is no
  `view/spike/percentiles.ts` (orchestrator ruling, 2026-10-03).
- Tests: the percentiles of fixed interval lists against hand-computed values, including the
  missed-frame and hitch counts of Design note 21 (in `main/results.test.ts`); the shim counts a
  creation after warm-up and passes the call through unchanged; per-segment figures do not mix
  segments.
- Acceptance: `pnpm --filter hyperion exec vitest run view/spike/metrics view/spike/pipelineShim`.

**R05.T14.b The switches, the trace and the reducer.** In `spike.ts`: the measurement switches of
Design note 18, set before `ready` when the flag is given. These are R01's `gpuTiming` and, when
`--dawn-safety off` is given, the safety-toggle set of Design note 22, merged into both
`--enable-dawn-features` and `--disable-dawn-features` with `mergeSwitchValue`. The task also
covers `contentTracing` over the descent, and the reducer, which parses the trace's JSON events in
the main process into frame, GPU-pass, main-thread and GC figures. The spike's switches are
`ChromiumSwitch` entries added to `graphicsSwitches`' list before `applyGraphicsSwitches`, which
already merges the four list switches with `mergeSwitchValue` (R01.T1 as built); the spike flag
sets R01's `gpuTiming` as `--hyperion-gpu-timing` does. This task may land before T13.c, creating
`main/spike.ts`, to which T13.c adds its IPC handlers.

- Files: `apps/hyperion/src/main/spike.ts`, `apps/hyperion/src/main/reduceTrace.ts`, their tests.
- Tests: the reducer against a small recorded trace, finding its GC slices per thread and its
  user-timing spans; the switch set is exactly the documented one for each option and never
  contains `--enable-unsafe-webgpu`; with timing on and safety off, one
  `--disable-dawn-features=timestamp_quantization,lazy_clear_resource_on_first_use` and one
  `--enable-dawn-features` holding R01's `enable_subgroups_intel_gen9` and the five safety toggles;
  the ordinary launch sets exactly R01's switches.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/spike src/main/reduceTrace`.

**R05.T14.c Memory and the results file.** `app.getAppMetrics()` and the renderer's memory at
1 Hz; a reader of the GPU process's DRM fdinfo; `nvidia-smi -q -x` where present, which is the
headline GPU memory on NVIDIA, whose driver gives no per-client figure through fdinfo; and the
results writer. The writer produces the file of Design note 18, with the machine, driver, Electron
and Chromium versions, setting, seed, options, switches, load average, governor and every figure,
including the per-level ε_n and k_n. It writes a Markdown summary beside the file. The memory
series is stored as columns (results schema version 2): the sample times in whole milliseconds
and one array of whole KiB per reading, which is lossless, since every source reports KiB or MiB.
A reading absent for the whole run is written once as null with its reason. A reading absent at
some samples is written as -1 there, with the sample ranges and the reason beside its column.
There is no delta encoding. Each results file stays under the repository's 500 KiB limit for
added files (`check-added-large-files`), which this form meets for runs of up to about two hours.
The file is never excluded from the hook, and the series is never thinned or moved to a side file
(decided 2026-10-04 by a delegated decision).

- Files: `apps/hyperion/src/main/fdinfo.ts`, `apps/hyperion/src/main/results.ts`, their tests,
  `docs/measurements/descent-spike/README.md`.
- Tests: the fdinfo parser against recorded fixtures from i915 and amdgpu, summing distinct
  client IDs; the results writer's output parses against its schema; each peak equals its
  column's maximum; a gap's samples hold -1 and no other sample does; a 3,600-sample run's file,
  formatted by Prettier, is under 512,000 bytes.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/fdinfo src/main/results`;
  `just descent-spike --setting low` on the development machine writes a results file with every
  figure present or null with a stated reason (on the RTX 3080 the fdinfo readings are null:
  decided 2026-09-30 by a delegated decision, hardware item 2). The lane proves the file with a
  hidden run, whose presentation-time figures are null with the reason "no window shown"; the
  visible run, the one that counts, is pending by hand for the owner. Its trace is windowed and
  unprofiled (T14.d, T14.e). The lane's proof of the windowed file is T14.f's hidden runs, and the
  visible run waits for T14.f.

**R05.T14.d The trace's windows: merge and results version 3.** This is pure main-process code and
the schema. It changes no runtime behaviour beyond the file's shape: until T14.e, a run is one
window with no boundaries.

- Each window's trace is reduced alone.
  - `reduceTrace.ts` adds the window's clock offset: the median of `ts − 1000 × args.startTime`
    over its `blink.user_timing` begin events, or null without one.
  - It keeps the end time of each dropped frame.
- `mergeTraceWindows` in a new `main/traceWindows.ts` pools the windows' figures under the rules of
  `decision-r05-trace-windows.md`:
  - presentation intervals only within a window, placed in script time by its offset;
  - the boundaries' excluded intervals, from the renderer's window times and the 1-s guard, left
    out of every per-frame figure;
  - times and counts summed and maxima maximal;
  - any failed window making every trace figure null with its reason.
- Results schema version 3:
  - `run.trace` with its windows, boundaries and traced time;
  - `frames.excludedFrames`, whole and per segment;
  - `mainThread.split` without the engine's figures, and `mainThread.engine` apart, null with its
    reason in an unprofiled run;
  - `validateResults` accepting version 3 only;
  - the summary;
  - the `-profiled` file name.
- The renderer's report gains `scriptStartMs` and its trace windows' times. The controller records
  the times of its one window.
- The replayer writes version 3.
- The committed version 2 files are converted to version 3 losslessly.

- Files:
  - `apps/hyperion/src/main/reduceTrace.ts`, `apps/hyperion/src/main/traceWindows.ts`,
    `apps/hyperion/src/main/results.ts`, `apps/hyperion/src/main/spikeSession.ts`,
    `apps/hyperion/src/main/spikeReport.ts`, their tests;
  - `apps/hyperion/src/main/fixtures/spikeReport.ts`, `apps/hyperion/src/preload/api.ts`;
  - `view/spike/spikeController.ts` and `view/spike/spikeRun.ts` (`scriptStartMs` only), their
    tests;
  - `tools/gpu-replay/src/results.rs`;
  - `docs/measurements/descent-spike/README.md`, and the committed results files and summaries.
- Tests:
  - from `src/main/fixtures/spike.trace.json`, the clock offset's events agree within 0.2 ms;
  - two windows with a gap give no interval across it, and their pooled percentiles and counts
    equal a hand computation;
  - a frame inside a boundary's excluded interval is absent from the rAF, presentation, pass and
    our-code figures, and is counted in its segment's `excludedFrames`;
  - a presentation is assigned to the segment of its script time;
  - busy times, GC and GPU-process slices are the windows' sums, and maxima their maximum;
  - each kind of failed window makes every trace figure null with "trace window k of n: …": a
    missing or empty file, no clock offset, a short span or `bufferPercent` ≥ 99, and another
    renderer;
  - `mainThread.engine` is null with the profiler-off reason in an unprofiled run, and
    `validateResults` refuses it present there;
  - `validateResults` refuses version 2, a boundary count other than the windows less one, and
    unordered or overlapping exclusions;
  - a 3,600-sample run with ten windows, formatted by Prettier, is under 512,000 bytes;
  - the replayer's unit test checks version 3 and its null `engine`.
- Acceptance:
  - `pnpm --filter hyperion exec vitest run src/main/reduceTrace src/main/traceWindows
src/main/results src/main/spikeReport src/main/spikeSession view/spike/spikeController`;
  - `just gpu-replay-check`;
  - every committed results file validates as version 3 and is under 512,000 B.
- Suggested subject: `feat(spike): R05.T14.d Merge trace windows into results version 3`.

**R05.T14.e Driving the windows.**

- `view/spike/traceWindows.ts` holds `traceBoundaries(segmentSpans)` and its constants:
  - `TRACE_WINDOW_MAX_S` 120, `TRACE_BUSY_WINDOW_MAX_S` 240, `TRACE_CHANGE_CLEARANCE_S` 10 and
    `TRACE_BOUNDARY_GUARD_S` 1;
  - the busy segments: "approach and flare", "low fast pass", "slowdown", "vertical descent".

  Boundaries are placed greedily from the start, each as late as allowed. None lies within the
  clearance of a segment change or inside the busy stretch widened by the clearance.

  _Changed by T14.g (2026-10-05, `decision-r05-trace-windows-2.md`):_ the window that holds the busy
  stretch runs from its widened start to the script's end, at most 300 s
  (`TRACE_BUSY_WINDOW_MAX_S`), so that no boundary follows it. Today's boundaries are 120, 240, 360,
  480, 600, 720, 840 and 950 s.

- The controller cycles the trace when the script first passes each boundary, without awaiting it
  in the frame, and records each window's `startedMs` and `stopRequestedMs`. A cycle that fails, or
  that is still pending at the next boundary, ends the trace (not the run): the remaining windows
  are failed with the reason.
- In the main process:
  - a `cycleTrace` channel and handler;
  - `SpikeTrace.cycle(path)`, which stops to `path` and then starts;
  - `SpikeTrace.bufferUsage()`, through `getTraceBufferUsage`, read before every stop;
  - `SPIKE_TRACE_CATEGORIES` without `disabled-by-default-v8.cpu_profiler`, and
    `SPIKE_PROFILER_CATEGORY` added by `spikeTraceConfig({ profiled })`;
  - `SPIKE_TRACE_BUFFER_KB` = 786,432 with `record-until-full`;
  - the session writing `spike-trace-<k>.json` per window under `userData`, then reducing and
    deleting each in order after the last stop.
- The client option `--trace-profile on|off` (default off), recorded in `run.options`.
- `descentSpike.sh` makes the run's profile under `target/descent-spike/` on disk, never under
  `TMPDIR`, and removes it as now. It and the `justfile` comment document `--trace-profile`.
- Nothing on screen changes: no label or console element marks a window, and the UX guide is
  untouched.

- Files:
  - `view/spike/traceWindows.ts`, `view/spike/spikeController.ts`, their tests;
  - `apps/hyperion/src/preload/api.ts`, `apps/hyperion/src/preload/spikeApi.ts`, its test;
  - `apps/hyperion/src/main/spike.ts`, `apps/hyperion/src/main/spikeSession.ts`,
    `apps/hyperion/src/main/cli.ts`, `apps/hyperion/src/main/index.ts`, their tests;
  - `apps/hyperion/src/renderer/src/test/stubHyperionApi.ts`,
    `apps/hyperion/scripts/descentSpike.sh`, `justfile`.
- Tests:
  - for today's profile, `traceBoundaries` gives exactly 120, 240, 360, 480, 600, 720, 840, 950
    and 1,190 s;
  - every window is ≤ 120 s except the one holding the busy segments, which is ≤ 240 s, and
    every boundary is ≥ 10 s from every segment change;
  - a profile whose busy stretch with its clearances exceeds 240 s throws;
  - the controller, on a fake clock, makes one cycle per boundary and reports every window's
    times;
  - a refused cycle ends the trace with its reason and the run goes on;
  - a pending cycle at the next boundary is refused;
  - `spikeTraceConfig()` excludes the profiler's category, and `spikeTraceConfig({ profiled:
true })` includes it, both with 786,432 KB and `record-until-full`;
  - `cycle` stops to its path before it starts, and a start while recording is still refused;
  - the session reduces three window files in order, deletes each, and fails only the window
    whose reduction throws;
  - the CLI parses `--trace-profile on|off`, refuses another value, and refuses it without
    `--descent-spike`;
  - the ordinary launch exposes no `cycleTrace`.
- Acceptance:
  - `pnpm --filter hyperion exec vitest run view/spike/traceWindows view/spike/spikeController
src/main/spike src/main/spikeSession src/main/cli src/preload`;
  - `just descent-spike --smoke` exits 0, hidden.
- Suggested subject: `feat(spike): R05.T14.e Take the trace in windows of script time`.

**R05.T14.f The windowed hidden runs** (re-defined 2026-10-05; after T14.g, T14.h and T14.i).

- **Conditions.** Every run is hidden, on the RTX 3080, under `just _locked` with its own capped
  scope:
  - no other spike run and no `descentDemand.mjs` (`pgrep`);
  - `free -g` ≥ 6 GB available;
  - the 1-min load recorded at each start.
- **First, the debugger's A/B:**
  - six untraced partial low runs, seed 7, to 255 s, in the order detached, attached, detached,
    attached, detached, attached. "Attached" is the CDP session attached and idle. Each starts
    only when the 1-min load is ≤ 4. An uncommitted instrument gives the untraced runs and the cap,
    as `experiment.patch` did.
  - Per run, over script time 10–255 s: `ourCodeMs` p50 and p95, the rAF interval's p95 and p99,
    and the renderer's private growth from 60 s.
  - **The session costs nothing** when, for each of the five figures, the median of the attached
    runs lies within the detached runs' range, widened by 2% of their median.
  - Otherwise stop, and report the figures to the orchestrator. The fallback is a browser-level CDP
    connection, re-ruled then.
- **Then three full runs:**
  - `--setting low --hidden` (T14.c's results run, re-taken);
  - `--setting high --hidden` (three workers, 4 isolates: the high setting's rates and last window
    before T17);
  - `--setting low --hidden --trace-profile on`.
- **Each must give:**
  - every window decoded and passing the frame-span cross-check, with no data loss and no new
    tracing-service dump in `coredumpctl`;
  - each window's protobuf bytes, `bufferPercent` and stop phases;
  - the largest `bufferPercent` ≤ 50%;
  - the tracing service's peak at most its largest working set plus 100 MB (no growth at the stop);
  - each boundary's gap, its excluded frames and its largest rAF interval in the gap;
  - in the timed runs, the **budget met:** ≤ 5% of the descent after the warm-up, ≤ 5% of each
    segment, and no frame excluded in the approach or any later segment. The last boundary's
    exclusion (its gap plus the guard) must end before the approach begins. _Clarified by the
    orchestrator under delegation (2026-10-06):_ the ruling's "none from 950 s" could be read as
    broken by construction, since its own last boundary is at 950 s;
  - in the timed runs, the renderer's private growth ≤ 0.1 MB/s from 60 s;
  - the post-run decoding's time (≤ 5 min) and the main process's peak during it;
  - the disk used;
  - where Chromium spools the stream (the browser's open files during a stop), which must be on
    disk;
  - every figure present or null with a stated reason:
    - `mainThread.engine` and the `WebGPU` and `VulkanQueueSubmitHook` slices only in the profiled
      run;
    - `GPUTask` in all.

  The profiled run's exclusions are recorded, not budgeted.

- **The guard is confirmed** when, pooled over every boundary of the two timed runs, at most 2% of
  rAF intervals in the 5 s after the guard exceed their window's 99th percentile. Otherwise it is
  raised by whole seconds until that holds, up to 5 s. Beyond that, report to the orchestrator.
- **If a timed run breaks the budget,** run one hidden low run with a single window (an uncommitted
  instrument: no boundaries) and report both to the orchestrator before committing.
- **The owner's first visible runs** are checked the same way before the next: T14.c's visible run,
  then T16's and T17's first. Every window must be decoded with no data loss, the largest
  `bufferPercent` must be ≤ 50%, and the budget must be met, or the windows are re-ruled.
- **Files:**
  - `docs/measurements/descent-spike/<date>-<machine>-low.json`, `-high.json` and
    `-low-profiled.json`, with their `.md`;
  - `TRACE_BOUNDARY_GUARD_S` in `view/spike/traceWindows.ts`, if raised;
  - this plan's T14.c as-built record and Risks (the A/B's figures included);
  - `docs/measurements/descent-spike/README.md`'s commands.
- **Acceptance:**
  - the three files validate as version 4, each under 512,000 B (the hook on, never skipped);
  - the A/B and the checks above hold, or their failure is reported with figures;
  - the visible-run commands in the README are current, for the owner.
- Suggested subject: `test(spike): R05.T14.f Record the windowed hidden runs`.

**R05.T14.g The frame span, the categories and results version 4.** This comes first. It fixes the
split's `ourCodeMs` and prepares the protobuf trace, with the JSON transport still in place.

- **The renderer.**
  - `SpikeRun.frame` reads `ended = performance.now()` once at the callback's end. It calls
    `performance.measure("spike.frame", { start: started, end: ended })` (`FRAME_MEASURE`), then
    `performance.clearMeasures("spike.frame")`, and reports `callbackMs = ended − started`, so that
    the span and `ourCodeMs` are the same two numbers.
  - `traceBoundaries`: the busy window runs to the script's end, with `TRACE_BUSY_WINDOW_MAX_S` 300.
- **The main process.**
  - `TraceReducer`'s `ourCodeMs` is the union of the main thread's `spike.frame` spans. It keeps
    each such span's start and duration for the cross-check.
  - `GPU_PROCESS_SLICES` is split by category, and the reducer summarises only the names the
    session says were recorded.
  - `SPIKE_TRACE_CATEGORIES` loses `gpu`, and `spikeTraceConfig({ profiled: true })` adds `gpu` and
    the profiler.
  - `mergeTraceWindows` gains the per-window frame-span cross-check.
  - Results version 4 (`format: "json"` for now), and `validateResults`. The replayer writes 4. The
    three committed files are converted. The README and the plan text of this ruling.
- The main process's literal `"spike.frame"` is pinned by a test on each side, as
  `SEGMENT_MEASURE_PREFIX`'s copies are, since the two tsconfig projects cannot share it.
- **Files:**
  - `view/spike/spikeRun.ts`, `view/spike/traceWindows.ts`, their tests;
  - `apps/hyperion/src/main/reduceTrace.ts`, `main/traceWindows.ts`, `main/spike.ts`,
    `main/spikeSession.ts`, `main/results.ts`, their tests, and `main/fixtures/traces.ts`;
  - `tools/gpu-replay/src/results.rs`;
  - the three committed results files;
  - `docs/measurements/descent-spike/README.md`;
  - this plan.
- **Tests:**
  - a window holding a 60-s `spike.segment:` span and per-frame `spike.frame` spans gives
    `ourCodeMs` equal to the `spike.frame` union alone;
  - `traceBoundaries` gives exactly 120, 240, 360, 480, 600, 720, 840 and 950 s for today's
    profile, the last window holds the busy segments to the end, and a profile whose last window
    would exceed 300 s throws;
  - `spikeTraceConfig()` holds the five categories and `spikeTraceConfig({ profiled: true })` adds
    `gpu` and the profiler;
  - without `gpu` the slices are `GPUTask` alone, and with it all three;
  - the cross-check passes on a matching window and fails it with its reason on each of: a missing
    span, an extra span, a duration off by 0.02 ms, and a start before its frame's rAF time less
    0.2 ms;
  - `validateResults` refuses version 3, a `TraceRun` without `format`, and a `WebGPU` slice
    without `gpu` among the categories;
  - every committed results file validates as version 4 within 512,000 B;
  - the replayer's unit test checks version 4;
  - `SpikeRun` reports `callbackMs` equal to its `spike.frame` span and keeps no `spike.frame`
    entry after the frame.
- **Acceptance:**
  - `pnpm --filter hyperion exec vitest run view/spike/spikeRun view/spike/traceWindows
src/main/reduceTrace src/main/traceWindows src/main/spike src/main/spikeSession
src/main/results`;
  - `just gpu-replay-check`;
  - the hidden `just descent-spike --smoke` exits 0.
- Suggested subject: `fix(spike): R05.T14.g Count our code by one frame span and record results
version 4`.

**R05.T14.h The protobuf trace's decoder.**

- `main/traceProto.ts` streams a Perfetto protobuf trace file and yields the JSON-shaped events
  `TraceReducer.add` reads. `reduceTraceFile` dispatches by the file's first byte: `{` or `[` for
  JSON, `0x0a` (`Trace.packet`) for protobuf.
- It is hand-written and has no dependency. It decodes only:
  - `TracePacket`: timestamp, `timestamp_clock_id`, `trusted_packet_sequence_id`,
    `sequence_flags` and `incremental_state_cleared`, `first_packet_on_sequence`, `interned_data`,
    `trace_packet_defaults`, `track_descriptor`, `track_event`, `clock_snapshot`, and the legacy
    process and thread descriptors;
  - interning per sequence: event categories, event names and debug-annotation names, reset when a
    sequence's incremental state is cleared;
  - clocks: each sequence's incremental clock from its defaults and clock snapshots, converted to
    one clock for the whole trace, so that the main thread's and the compositor's sequences agree;
  - track descriptors: their process and thread (pid, tid, names, and Chrome's process and thread
    types mapped to the JSON's `Renderer`, `GPU Process`, `Browser`, `CrRendererMain` and
    `CrGpuMain`), and each sequence's default track;
  - `TrackEvent`: type, track, categories, name, timestamps, `legacy_event` (phase, ids, duration,
    pid and tid overrides), debug annotations (scalars, nested dictionaries and legacy JSON values;
    `startTime` and ProfileChunk's `data` among them), and `chrome_frame_reporter` (state and
    `layer_tree_host_id`, mapped to the JSON's `STATE_*` names);
  - slice begins and ends paired per track into complete events; async user-timing pairs keep the
    reducer's (pid, id, name) matching.
- Unknown fields are skipped. An event kind the reducer reads that arrives in an encoding the
  decoder does not know fails the file with its name.
- Every field number is cited in the TSDoc from Perfetto's protos at the version Chromium 152 ships
  (`third_party/perfetto/protos/perfetto/trace/…`), and pinned by the fixtures.
- `trimProtoTrace(bytes, untilUs)` keeps each sequence's packets from its first up to `untilUs`. A
  sequence's incremental state is a prefix, so the result is a valid trace. It is used to cut the
  fixtures.
- The renderer's report gains `callbackStartsMs`, each frame's callback start: `SpikeRun`'s
  `started`, the start its `spike.frame` measure receives. `mergeTraceWindows`'s frame-span check
  matches spans to frames by identity.
  - A frame whose callback start lies at least 0.5 s inside its window has exactly one span whose
    `args.startTime` equals it within 10⁻⁶ ms.
  - That span's duration is within 0.25 ms of its `ourCodeMs` (`FRAME_SPAN_TOLERANCE_MS`): two of
    `performance.now()`'s 0.1 ms quanta, since Chromium may write the endpoints unclamped.
  - A span in that interior that matches no frame fails the window, unless it starts after the
    series' last callback (the page draws on after the run).
  - The last frame is checked. `FRAME_SPAN_EARLY_MS` goes.
  - Files beyond the decoder's: `view/spike/spikeRun.ts`, `view/spike/metrics.ts`,
    `preload/api.ts`, `main/spikeReport.ts`, `main/fixtures/spikeReport.ts`,
    `main/traceWindows.ts`, `main/fixtures/traces.ts`, and their tests.
- **Fixtures** (each ≤ 400 KB, the hook on), recorded from Electron 44.4.3 on the RTX 3080. They
  come from short hidden partial runs (low, seed 7, ended at 30 s by the handoff's
  `experiment.patch`, applied for the recording only and never committed: CDP proto, window files
  kept, report dumped), since a smoke hands the main process no report
  (`decision-r05-trace-windows-2.md`, addendum A):
  - `main/fixtures/spike.pftrace`: the timed partial's window, trimmed to about 0.5 s;
  - `main/fixtures/spike-profiled.pftrace`: the same from a `--trace-profile on` partial;
  - `main/fixtures/spike.pftrace.report.json`: the timed partial's report, cut to the fixture's
    frames (`scriptStartMs`, its window, and each frame's script time, `ourCodeMs` and
    `callbackStartsMs`).

  The timed fixture holds at least one GC slice on the renderer.

  The test file records the provenance (date, versions, commands).

- **Tests** (on the fixtures):
  - the reducer finds the renderer, its `CrRendererMain`, the GPU process and its `CrGpuMain`;
  - **against the renderer's own series:** one `spike.frame` span for each of the report's frames
    in the fixture, its `args.startTime` equal to the frame's `callbackStartsMs` within 10⁻⁶ ms and
    its duration within 0.25 ms of its `ourCodeMs`. The test file records whether the trace's
    endpoints are the values passed or unclamped, and the fixture's largest deviation. Any frame
    beyond 0.25 ms, or any identity that fails, stops the task for the orchestrator;
  - the window's clock offset agrees within 0.2 ms over its begins;
  - **across sequences:** each `spike.frame` span starts within 0–17 ms after a `PipelineReporter`
    begin on the busiest compositor;
  - every `spike.frame` span lies inside a `RunTask` on `CrRendererMain`, within 1 µs;
  - `PipelineReporter` states decode to the JSON's names;
  - GC and `GPUTask` slices are complete, with positive durations;
  - on the profiled fixture: `WebGPU` and `VulkanQueueSubmitHook` slices are present, and the main
    thread's ProfileChunk samples give `sampledMs` within 10% of the fixture's main-thread span;
  - `decode(trimProtoTrace(f, t))` equals `decode(f)` cut at t, per sequence;
  - decoding with an 8-KiB read size equals decoding with the default;
  - a truncated packet fails with its offset;
  - the JSON fixture still reduces through the JSON reader;
  - the identity check fails a window for a missing span, a duplicated span, a span whose duration
    is off by 0.3 ms, and an unmatched span inside the interior; it ignores a span that starts after
    the last callback; it passes a callback that began after the next frame's rAF time.
- **Recorded, not a test (as built):**
  - `trace_processor_shell` v58.2 (the handoff's sha256-checked copy) on the untrimmed window. Per
    thread, the counts and summed durations of `RunTask`, `GPUTask`, `PipelineReporter`, `MinorGC`
    and `MajorGC` must equal the decoder's. User timing is left out, being its known fault.
  - The decoder's speed and peak memory on the untrimmed window.
- **Acceptance:** `pnpm --filter hyperion exec vitest run src/main/traceProto src/main/reduceTrace`.
  The fixtures are committed with the hook on, and the oracle comparison is recorded.
- Suggested subject: `feat(spike): R05.T14.h Decode the protobuf trace`.

**R05.T14.i The trace over CDP.**

- `main/cdpTracing.ts`: `CdpTracing`, implementing `SpikeTracing` over the spike window's
  `webContents.debugger`.
  - It attaches (protocol `1.3`) once before the first window, and detaches after the last stop.
  - Each start is `Tracing.start` with:
    - `traceConfig` { `recordMode: "recordUntilFull"`, `traceBufferSizeInKb`,
      `includedCategories`, `excludedCategories: ["*"]` };
    - `transferMode: "ReturnAsStream"`, `streamFormat: "proto"`, `streamCompression: "none"`;
    - `bufferUsageReportingInterval: 1000`.
  - Each stop is `Tracing.end`, waits for `Tracing.tracingComplete`, then reads the stream with
    `IO.read` (base64, 8 MiB a read) into the window's file, then `IO.close`. It logs both phases'
    times.
  - `dataLossOccurred` fails the window ("lost data"). The last `Tracing.bufferUsage.percentFull` (a
    fraction) × 100 is its `bufferPercent`.
  - It sends no other command. An unexpected `detach` ends the trace, not the run, as a failed
    cycle does.
- `SpikeTrace` takes it in place of `contentTracing`. The buffer is 786,432 KiB, or 1,572,864 KiB
  profiled. The window files are `spike-trace-<k>.pftrace`, `format` is `"perfetto-proto"`, and
  `windowFileFailure` adds data loss.
- `descentSpike.sh` gives Electron a `TMPDIR` on disk (`target/descent-spike/tmp`), removed after
  the run, so that any spool of the stream stays out of a RAM-backed `/tmp`.
- `contentTracing` is no longer used by the spike.
- The smoke checks its trace (addendum A). It hands the main process its report through the same
  results call a run makes. For a smoke, the session merges the windows with the report, frame-span
  check included, and logs each window's line. It fails the smoke if any window failed, with that
  window's reason, and builds and writes no results file. Files beyond the list:
  `view/spike/spikeController.ts` and its test.
- **Files:**
  - `apps/hyperion/src/main/cdpTracing.ts`, `main/spike.ts`, `main/spikeSession.ts`,
    `main/traceWindows.ts` (data loss), `main/index.ts`, their tests;
  - `apps/hyperion/scripts/descentSpike.sh`, `justfile` (comment);
  - `docs/measurements/descent-spike/README.md`.
- **Tests,** on a fake debugger:
  - the only commands are `Tracing.start`, `Tracing.end`, `IO.read` and `IO.close`, with one attach
    and one detach;
  - `Tracing.start`'s parameters are exactly the above, with the profiled buffer when profiled;
  - a stop waits for `tracingComplete` and writes the stream's chunks in order until `eof`;
  - `dataLossOccurred` fails the window;
  - `percentFull` 0.095 gives 9.5 %;
  - a detach mid-run ends the trace with its reason, and the run goes on;
  - a start while recording is still refused;
  - a smoke whose window fails the frame-span check exits 1 with that reason; a smoke writes no
    results file.
- **Acceptance:**
  - `pnpm --filter hyperion exec vitest run src/main/cdpTracing src/main/spike
src/main/spikeSession src/main/traceWindows`;
  - the hidden `just descent-spike --smoke` exits 0 with three protobuf windows, each decoded by
    T14.h's decoder, passing the frame-span cross-check, and deleted. Its window log lines are
    recorded.
- Suggested subject: `feat(spike): R05.T14.i Take the trace over CDP as a protobuf stream`.

**R05.T14.j Incomplete frames and results version 5.**

- **The renderer.**
  - The spike's recorder knows each frame's resolve numbers (`ResolveCounter`, following
    `RenderEngine.passTimesFrame`) and which reported times. Its report gains a per-frame count of
    a frame's numbered resolves that never reported (0 for a complete frame).
  - The report reader checks the count's length and that it is whole.
- **The main process.**
  - `buildResults` takes every per-frame sum over complete frames, and counts incomplete frames
    after the warm-up, dropped and partial apart.
  - Each GPU percentile row gets its verdict by addendum B's bound (Design note 21).
- **Results version 5:**
  - `gpu.incompleteFrames`;
  - `gpu.clocks`, written null with "the GPU's clocks are not yet read (R05.T14.k)" until T14.k;
  - `validateResults`, and the summary's line;
  - the replayer's version 5;
  - the six committed files converted;
  - the README and addendum B's plan text.
- **Files:**
  - `view/spike/metrics.ts`, `view/spike/spikeHarness.ts`, their tests;
  - `preload/api.ts`, `main/spikeReport.ts`, `main/fixtures/spikeReport.ts`;
  - `main/results.ts`, its test;
  - `tools/gpu-replay/src/results.rs`;
  - the six committed results files;
  - `docs/measurements/descent-spike/README.md`;
  - this plan.
- **Tests:**
  - a frame with one of its five resolves missing is partial, and one with all missing is
    dropped; neither enters a sum, and both are counted after the warm-up only;
  - the bound:
    - a row whose complete frames pass, with k incomplete frames that could push it over, is not
      measured with the reason;
    - one that passes even with all k above the limit passes;
    - one that fails even with all k below fails;
    - k = 0 gives exactly version 4's verdict;
  - each pass's own percentiles keep every time that arrived;
  - `validateResults` refuses version 4, a count above the frames after the warm-up, a clock column
    of the wrong length, and a pass verdict that the count makes impossible;
  - every committed file validates as version 5 within 512,000 B;
  - the replayer's test checks version 5.
- **Acceptance:**
  - `pnpm --filter hyperion exec vitest run view/spike/metrics view/spike/spikeHarness
src/main/spikeReport src/main/results`;
  - `just gpu-replay-check`;
  - the hidden `just descent-spike --smoke` exits 0.
- Suggested subject: `fix(spike): R05.T14.j Count incomplete frames and bound the GPU rows by
them`.

**R05.T14.k The GPU's clocks.**

- **A reader in `main/gpuClocks.ts`,** run by the 1 Hz sampler beside the memory:
  - **NVIDIA:** from the `nvidia-smi -q -x` output the sampler already reads. `parseNvidiaSmi`
    gains `clocks/graphics_clock`, `clocks/mem_clock`, `performance_state` and
    `max_clocks/graphics_clock`, in MHz, `N/A` as null.
  - **i915 under Linux:** `gt_act_freq_mhz`, and `gt_RP0_freq_mhz` or `gt_boost_freq_mhz`, of the
    run's GPU's DRM card. Choose it by the adapter's PCI vendor and device under
    `/sys/class/drm/card<N>/device/`.
  - **amdgpu under Linux:** `pp_dpm_sclk` and `pp_dpm_mclk`.
  - **Otherwise** null with the platform's reason (`process.platform` injected for tests). A
    sysfs file that is absent or unreadable gives that column null with the path in its reason.
- **What reaches the results.**
  - `gpu.clocks` from the samples.
  - The GPU rows' note: the median graphics clock after the warm-up, against the maximum, when
    below 90%.
  - The summary's line: the median, p5 and p95 of the graphics clock, against its maximum, with
    the source.
- **The replayer** reads the same sources (`nvidia-smi`, sysfs) before its first frame, after its
  last, and once a second between. It writes `gpu.clocks` from them.
- **Files:**
  - `apps/hyperion/src/main/gpuClocks.ts`, `main/fdinfo.ts` (the parser), `main/results.ts`, their
    tests;
  - fixtures: an `nvidia-smi.xml` with clocks (re-recorded on the RTX 3080, unprivileged), and
    i915 and amdgpu sysfs trees written in the kernel's documented layout;
  - `tools/gpu-replay/src/` (the reading and `results.rs`);
  - `docs/measurements/descent-spike/README.md`.
- **Tests:**
  - the NVIDIA fixture parses its graphics, memory and maximum clocks and its P-state;
  - the i915 tree gives `gt_act_freq_mhz` against RP0, and falls back to boost when RP0 is absent;
  - the amdgpu tree gives the starred levels and the highest;
  - darwin and win32 (non-NVIDIA) give null with their reasons;
  - a missing file gives its column null with the path;
  - the column form holds `-1` for a missing sample, with its gap;
  - a run at a median 1,100 of 1,980 MHz puts the note on the three GPU rows, and one at 1,950
    puts none;
  - the replayer's unit test writes clocks from a stubbed reading.
- **Acceptance:**
  - `pnpm --filter hyperion exec vitest run src/main/gpuClocks src/main/fdinfo src/main/results`;
  - `just gpu-replay-check`;
  - the hidden `just descent-spike --smoke` on the RTX 3080 exits 0, with its sampled clocks
    logged and recorded in the as-built notes.
- Suggested subject: `feat(spike): R05.T14.k Record the GPU's clocks beside the rows`.

**R05.T14.l The rest of the frame as one row** (added 2026-10-06 by a delegated decision,
`decision-r05-high-atmosphere.md`).

- **The row.**
  - The criteria's `terrain` and `atmosphere` rows give way to one, `terrain-atmosphere`: the 95th
    percentile, over complete frames, of each frame's passes whose row is `terrain` or `atmosphere`,
    summed.
  - Its limit is the two estimates' sum, 6 ms on high and 18 ms on low.
  - It is built by `boundedGpuRow`, and the clock note applies.
- **The parts.**
  - `gpu.rows` holds `terrain` and `atmosphere`: each per-frame sum's p50, p95 and p99, its
    estimate, and `overEstimate`.
  - They enter no verdict.
  - The summary prints each against its estimate.
- **The rest:**
  - `GPU_ROW_IDS` is `headroom-gpu` and `terrain-atmosphere`;
  - results version 6, with `validateResults` refusing version 5 and refusing a joint limit that is
    not the estimates' sum;
  - the committed files converted (the joint row not measured, with the reason, where a file cannot
    give it);
  - the replayer's `results.rs`;
  - the README's `criteria` and `gpu` rows.
- **Files:**
  - `main/results.ts` and its test;
  - `tools/gpu-replay/src/results.rs`;
  - the committed results files;
  - `docs/measurements/descent-spike/README.md`;
  - this plan.
- **Tests:**
  - terrain 4.5 and atmosphere 1.4 ms p95 pass, with the atmosphere over its estimate as a finding;
  - 6.2 ms fails;
  - the percentile is of per-frame sums;
  - T14.j's bounds hold on the joint row;
  - no verdict reads `gpu.rows`;
  - version 5 is refused;
  - the replayer writes version 6.
- **Acceptance:**
  - `pnpm --filter hyperion exec vitest run src/main/results src/main/spikeReport`;
  - `just gpu-replay-check`;
  - the hidden `just descent-spike --smoke` exits 0.
- Suggested subject: `feat(spike): R05.T14.l Judge terrain and atmosphere as one row`.

### R05.T15 The capture and the native replay

**R05.T15.a The capture.** A measurement-only shim on `GPUDevice`, `GPUQueue` and the encoders,
after webgpu_recorder's interception design, that writes WGSL sources, descriptors, uploads and
each frame's commands over a fixed span of the descent as JSON with binary blobs, when
`--capture <dir>` (T13.c) is given. It wraps the device through T14.a's seam, the wrapped `GPU`.
Its forwarding wrappers of `createBuffer` and `createTexture` trip `engineBoundary.test.ts`'s rule
against allocating outside R01's adapter, so the test gains a named exemption for
`view/spike/capture.ts` alone, as it has one for the smoke page, rather than the rule being
loosened.

- Files: `view/spike/capture.ts`, its test against a fake device (`FakeGpu`),
  `view/engine/engineBoundary.test.ts`.
- Tests: a scripted sequence of device calls round-trips through the capture's reader to the same
  calls and bytes; the shim is absent unless the flag is given.
- Acceptance: `pnpm --filter hyperion exec vitest run view/spike/capture`.

**R05.T15.b The replayer's reader and validation.** `tools/gpu-replay/`, a Rust binary with its
own `[workspace]`, on wgpu 30: it loads a capture, validates every module with naga and reports
rejections. The root `Cargo.toml` gains `exclude = ["tools/*"]` (Design note 20), and the
justfile gains a `replay` recipe that `ci` does not call.

- Files: `tools/gpu-replay/Cargo.toml`, `tools/gpu-replay/src/{main,capture,validate}.rs`,
  `Cargo.toml`, `justfile`.
- Tests: in the tool, the capture reader against a small checked-in capture; the naga validation
  report lists a deliberately invalid module.
- Acceptance: `cargo test --manifest-path tools/gpu-replay/Cargo.toml`; `just ci` does not build
  wgpu (its build log names no `wgpu` crate).

**R05.T15.c The replay.** On winit: recreate the capture's objects, replay its frames with FIFO
presentation at the captured resolution, and write the same frame-interval and per-pass figures
as the results file, in its schema.

- Files: `tools/gpu-replay/src/{replay,results}.rs`.
- Tests: in the tool, a replay of the checked-in capture on the default adapter, into an
  offscreen texture with no window or presentation, writes a results file that parses against
  the schema.
- Acceptance: `just replay <capture>` replays the development machine's capture on its RTX 3080
  and writes a results file; the owner does the same for a UHD 620 capture on the UHD 620. The
  presented replay opens a window with FIFO presentation, so on the development machine it is
  pending by hand for the owner, with the lane's capture and exact command recorded.
- Results note (T20): the replay's machine facts are read at its start through `sysinfo`, on every
  platform alike, with the governor from sysfs on Linux. On Windows, which keeps no load average, a
  replay is always provisional.

### R05.T16 The UHD 620 runs

By hand on the owner's UHD 620 laptop, by the owner, recorded, each run on a quiet machine (Design
note 27) with its results file and summary under `docs/measurements/descent-spike/`. The low
setting's harness and results file are first proved on the development machine (T14), so that the
owner's runs need no debugging. The runs are one baseline and then one-factor changes from it, not a
matrix. Every run is windowed and unprofiled (T14.e), so that a variant differs from the baseline in
its one factor alone.

**R05.T16.a The baseline.** The low setting at 720p paced to 30 fps with the two instruments and
the console open, the local server running with `--num-workers 2`, the default two height
workers, `face-differences`, mesh normals, ridged terms off, safety checks on and a warm pipeline
cache, over three seeds: three runs of about 21 minutes (the sum of Design note 19's durations).
One high-setting run at 1080p is recorded for comparison, not judged.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: the four results files exist, each with every figure of Design note 18 and the
  machine's load average and governor; the summary states pass or fail against every row of
  Design note 21's table. Each results file records `PassTimes.timer`, the platform, the launch
  mode, the frames with incomplete pass times and the GPU's clocks (or why not); GPU-time rows on a
  `quantized` timer carry ±65.5 µs per pass and are marked marginal within that tolerance of their
  limit (decisions-r06-r07.md item 8).

**R05.T16.b The variants.** On one seed, each changing one factor from the baseline: a companion
load on two threads (Design note 20); a cold pipeline cache; ridged terms on; Dawn's safety checks
off; and the memory run with three workers and the synthetic 15 MB field (Design note 11). That
makes five runs. There is no vertex-path variant, since `BakedOffsets` is barred from the low
setting (Design note 4); T17 runs it. The field is `SYNTHETIC_FIELD_BYTES`, the brainstorm's 15 MB,
an upper bound on R09 Design note 17's 11.6 MB for an Earth (Risks, "The coarse field's size"). A
capture of one baseline span is replayed by `just replay` for the native comparison.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: the five results files and the replay's exist, each as T16.a's; the summary states
  each variant's difference from the baseline per row.

**R05.T16.c The low setting's redesign, if needed.** If a T16.a or T16.b run misses Design note
21's rows, record which pass or thread misses and by how much. Then redesign the low setting
within this plan before T18, in the order that costs the picture least:

1. normal and table sizes;
2. the ray march's resolution;
3. τ within the brainstorm's lean;
4. the render resolution under 720p, presented upscaled;
5. last, the instruments' rate.

Each change is one more recorded baseline run. If nothing misses, the task records that and ends.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`; the low values in
  `view/quality/qualitySetting.ts`.
- Acceptance: the last baseline run passes every row of the low column, or the summary names the
  row that cannot be met and why.

### R05.T17 The discrete runs

By hand on the development machine, by the owner, on a quiet machine (Design note 27); every run
needs a visible window, a real vsync and the display's mode changed, which no lane does on the
owner's display, so the implementing lane prepares the harness and the exact commands and the
runs are pending for the owner. Its RTX 3080 (10 GiB,
760 GB/s), the recommended specification, exceeds the RTX 4060 class (272 GB/s), and its one
display, an Optoma UHD projector (native 3840 × 2160 at 60 Hz), is set to its 1080p 59.94 Hz mode
for the runs, T being the measured vsync period (Design note 21; it has no exact 60 Hz mode; a
rented cloud GPU gives advisory GPU-time figures only, since its virtual display has no real
vertical blank). A pass there is a pass on a faster part than the class named, so the summary
records the GPU-time headroom per pass beside the frame intervals. The runs are:

- the high setting at 1080p over the same three seeds, with the same two instruments and console
  and the local server capped as in T16, which is three runs;
- on one seed, safety checks off, the other vertex path, and mesh normals, which is three runs;
- on one seed, the same run with `--workers 2`, the budget's two cores, for comparison;
- one capture replayed natively on the same machine by `just replay`.

Every timed run pins `--workers 3`, so the memory row is read from the timed runs and there is no
separate memory run (decided 2026-09-30 by a delegated decision, hardware item 4). Every timed run
is windowed and unprofiled (T14.e). The first T17 run's windows are checked before the others. Every
window is reduced, none lost data or is above half its buffer (`bufferPercent` ≤ 50), and the frames
left out are within T14.f's budget. Otherwise the runs stop, and the windows are re-ruled. If a
timed run misses a frame row or the main-thread headroom row, one more run on the same seed with
`--trace-profile on` gives T19 the engine adapter's share of the main thread and the GPU process's
`WebGPU` and `VulkanQueueSubmitHook` slices. It is a diagnostic, not judged, and the only profiled
run the owner makes (decided 2026-10-04 by a delegated decision).

The machine, driver and display are recorded with the results.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: as T16.a, on the discrete machine, with the replay's results file beside the
  browser's for the same span.

### R05.T18 Defaults from the measurements

The choices the plan left to measurement, made from T16's and T17's results and recorded with the
figures that decided them: the vertex path for the high setting (Design note 4; the low setting
keeps `FaceDifferences`), the high setting's normal scale (Design note 25), the descending
thresholds and `FORCED_REGION_RESIDENCY_S` (Design note 9), the cache budgets (Design note 10, until
R10), the worker counts (Design note 11) and, if T16 redesigned it, the low setting. The selection
bound is not among them: both settings select by the hard ε_n plus the sagitta
(`decision-r05-high-bound.md`). Nor are the per-frame marches' steps or the atmosphere's row.
R05.T12.e sets the steps' placement and counts by its quadrature gate, and T18 changes them only
through that gate. Design note 21 judges terrain and atmosphere together
(`decision-r05-high-atmosphere.md`). The high setting's vertex path is also read against T13.a's
effective tolerance. On the ridged planet, `FaceDifferences`' budget of 1,952 lowers τ′ from
2.1–3.0 px to 1.0–2.3 px (probe, 2026-10-04). With ridges off, `BakedOffsets`' 981 binds over the
late arc at τ ÷ 1.1 (decision-r05-record-tau.md, probe 2026-10-05; the re-run of 2026-10-05: 34%
of the arc's frames and 30% of the approach's, τ′ at most 0.96 px), so the high setting's vertex
path also decides whether `DETAIL LIMITED` shows on the default planet. ⌊slots ÷ 2⌋ is confirmed
or changed against the cache's eviction of coarse patches (Risks). The code's defaults change in
one commit; the plan's Design notes gain "as built" lines.

- Files: the constants' modules in `view/terrain/` and `view/atmosphere/`, this plan.
- Tests: the existing tests, updated only where a default is asserted.
- Acceptance: `just ci`; the plan's Risks and open points list what was chosen and why.

### R05.T19 The gate's verdict

Open question 2's rule applied to the recorded runs, in the order of Design note 22: whether the
spike passes on each machine; if the discrete run fails, whether Dawn's safety checks, the
browser's transport (from the replay) or our own code is responsible, by pass and thread; and so
whether the rule fires. The verdict, with its figures, is written in this plan's Risks and open
points and summarised in `docs/measurements/descent-spike/README.md`, and an edit to the
brainstorm's open question 2 (from **Lean** to **Closed**, or to what the runs show) is drafted
for the owner, as are the brainstorm findings this plan reports (Risks and open points). If the
rule fires, the owner is told before any later plan depends on the browser. If the discrete run
fails on streaming demand and the demand under min(hard, 4σ_n) (T13.a) would meet the budget, the
verdict names the selection bound as ours to fix; that is never a fired rule (decisions-r05.md item
6). The engine adapter's share of the main thread, and the GPU process's `WebGPU` and
`VulkanQueueSubmitHook` slices, come from a profiled run: T17's if one was needed, else T14.f's
hidden one, which is provisional. Timed runs carry none. Each GPU row is read with the clocks
recorded beside it (T14.k). The verdict is the row as measured. A row that fails while the run's
median clock was below 90% of the maximum says so, and may cite the pass's times in the samples at
the maximum clock as evidence, never in the row's place. The replay's and the browser's pass times
are compared with both runs' clocks stated, and a difference of more than 10% between their median
clocks is named beside the comparison. The rest of the frame is judged on terrain and
atmosphere together (Design note 21). Each pass's percentiles against its estimate are reported,
and a pass over its estimate is among the brainstorm findings drafted for the owner. If the joint
row fails, the verdict names the pass furthest over its estimate.

- Files: this plan, `docs/measurements/descent-spike/README.md`.
- Acceptance: the verdict names each criterion of Design note 21 with its measured value on each
  machine; the owner has the drafted brainstorm edit.

### R05.T20 Machine facts on every platform

Added 2026-10-06 by a delegated decision (`decision-cross-platform-server.md`, items 6 and 7). It
runs after T14.i has merged, since lane D owns `results.ts` and the replayer's results until then.

Off Linux the replayer's `machine()` recorded `cpu: "unknown"`, `name: "machine"`,
`memoryBytes: 0` and a `loadAverage` of `[0, 0, 0]` written as if it had been measured. The
provisional rule is `load_1 >= 1.0`, so every replay on the Mac or on Windows claimed a quiet
machine, a wrong verdict under Design note 27. The client has the same flaw on Windows, where
Node's `os.loadavg()` returns zeros, and `scripts/descentDemand.mjs` read `/proc/loadavg`, which
throws on macOS and Windows. `sysinfo` 0.39.6 (MSRV 1.95), with `default-features = false,
features = ["system"]`, gives the host name, the CPU's brand and the total memory on all three
platforms, and the load average on Linux and macOS ("currently not working on Windows"). The tool
is outside the workspace, so the dependency touches neither the workspace's dependency list nor
its lint. On Linux it reads the same `/proc` sources, so Linux results stay the same.

The ruling: use `sysinfo` in the tool. Windows has no load average, so a Windows run is always
provisional, with the note "Windows keeps no load average: the quiet-machine rule (Design note 27)
is unchecked". The field stays a number triple under results version 5; a later schema version,
bumped for other reasons, should make `loadAverage` a `Measured`. The same rule goes into the
client's results, and `descentDemand.mjs` stops reading `/proc/loadavg`.

- _Files:_
  - `tools/gpu-replay/Cargo.toml`:
    `sysinfo = { version = "0.39.6", default-features = false, features = ["system"] }`.
  - `tools/gpu-replay/src/results.rs`:
    - `machine()` reads through a small `MachineSources` seam: hostname, CPU brand, total memory,
      load or none, and governor or a reason. The production source is `sysinfo`, with `/sys` for
      the governor on Linux.
    - Elsewhere the governor's reason is "no cpufreq governor on macOS" or "… on Windows".
    - On Windows, `provisional: true` with the note above.
    - The hostname keeps its normalisation.
  - `apps/hyperion/src/main/results.ts`:
    - The provisional rule becomes `platform === "win32" || load >= 1`, with the same note.
    - The governor's reason depends on the platform.
    - `os.loadavg()` returns zeros on Windows (Node's documentation).
  - `apps/hyperion/scripts/descentDemand.mjs`: `os.loadavg()` instead of reading `/proc/loadavg`,
    which throws on macOS and Windows.
  - This plan: Design note 27's sentence on the platforms (no load average on Windows, so
    provisional; no governor off Linux), and T15.c's results note.
- _Tests:_
  - gpu-replay unit tests of `machine` over fixture sources, for Linux, macOS and Windows: the
    name's normalisation, a missing CPU giving `"unknown"`, Windows provisional with its note, a
    macOS load of 0.5 not provisional.
  - `results.test.ts`: a `win32` run is provisional with the note; a `darwin` run at load 0.5 is
    not.
  - The demand script's machine-facts path runs with `/proc` absent, through an injected reader
    (the orchestrator's addition).
- _Accept:_
  - `just gpu-replay-check`;
  - `pnpm --filter hyperion test results`;
  - `node --check apps/hyperion/scripts/descentDemand.mjs`;
  - `just cross-clippy`, which runs the replayer's Clippy for `aarch64-apple-darwin` and
    `x86_64-pc-windows-msvc` without an SDK;
  - `just ci`.
- The Electron main process's other platform branches (item 7) are not this task's. They are R12.T0's
  audit list: DRM fdinfo and `nvidia-smi` off Linux, `workingSetSize` across platforms, the launch
  mode that is always `default` off Linux and so the quantized timer, and R12's `/sys`-only
  samplers.
- Suggested subject: `fix(replay): R05.T20 Read the machine's facts on every platform`.

## Verification

- **Determinism:** the cube-sphere, test-planet, vertex, bake and level-table goldens pass on
  native, `wasm32-wasip1` and `wasm32-unknown-unknown` in `just ci` (T2, T3.c, T4.b, T5, T6); the
  TypeScript mirror, the vertex emulation and the level table match the Rust bits (T2, T11.b,
  T7.a); the noise and the bake are independent of their cache's fill order (T3.a, T4.a).
- **Geometry:** shared edges are bitwise equal across faces (T1.b, T4.a); both vertex paths are
  under 1 mm from `f64` at level 19 and the naive form is not (T4.b); the collision interpolant is
  the drawn finest mesh (T4.c).
- **Level of detail:** the hard bound holds, and its looseness per level, with σ_n and the
  ratios to 4σ_n, is recorded for R10.T4's mechanical rule (T6);
  selection meets τ, is balanced, is a pure function of its inputs and keeps the finest level under
  every grounded body on every setting and tolerance (T7); the morph is continuous at shared
  vertices and 1 at every level transition (T7.c).
- **Culling:** the predicates' named cases and the brute-force oracle's zero false culls (T7.a).
- **Streaming:** measured demand within a factor of two of the per-level prediction on the
  fixed-step descent (T13.a); patches a second sustained against demand in every recorded run
  (T16, T17).
- **Atmosphere:** the Rayleigh, ozone and aerosol figures recompute (T12.a); the transmittance table
  matches an `f64` oracle to 1% on SwiftShader (T12.b); the comparison against Hillaire's published
  images, by eye (T12.c).
- **The gate:** the recorded runs against every row of Design note 21 on both machines (T16, T17),
  the replay and toggle comparisons, and the verdict (T19).
- **By eye, recorded:** the descent from orbit to a metre above the ground in one motion, watching
  for pops, cracks between levels and at cube-face edges, and the moment `TERRAIN: STREAMING`
  appears (T13.c, T16.a), by the owner.

## Generator version

No change to generated output and no bump. The test planet belongs to no universe, and nothing a
universe generates reads `hyperion-surface` yet. Its goldens carry `TEST_PLANET_VERSION`, which
starts at 1 and moves with any change to the test planet's heights or the bake's bytes. The plan
reserves the `SelfTest` tag `selftest.surface.test_planet` in the surface crate's part of the
registry, and nothing under a `surface.*` or `body.surface.*` name, which R09 registers. The new
tag changes `crates/hyperion-sim/tests/golden/rng/tags.golden`, which is re-blessed with no bump,
as earlier tag additions were: the registry listing is not generated output. The
constants `FINEST_SPACING_M`, `BAND_LIMIT_M` and the rule of `finest_level` become part of the
generator version when R09's real height function reads them; R09 must bump `GENERATOR_VERSION`,
which R04 places in `hyperion-base` where the surface crate can see it, for any later change to
them. No wire type changes: the spike's IPC is the preload's, not the protocol's.

## Risks and open points

- **The discrete machine is the development machine.** Its RTX 3080 exceeds the RTX 4060 class
  the brainstorm names, so T17's pass is a pass on a faster part; its summary records the GPU-time
  headroom so that the class can be judged. A cloud GPU's figures remain advisory and cannot sign
  off the gate (researched 2026-09-29: virtual displays have no real vertical blank, and the
  compositor path and driver branch may differ). The UHD 620 half, T16, is the owner's by hand;
  until it runs, the gate answers for the discrete target only.
- **The hardware decisions, decided 2026-09-30 by a delegated decision** (the orchestration's
  hardware record), as they fall on this plan:
  - _The gate (item 1):_ retargeted to the recommended specification, the RTX 3080, with Design
    note 21's criteria unchanged and T the measured vsync period (16.68 ms at 59.94 Hz); no
    scaling margin. Written into Design note 21 and T17.
  - _Readings (item 2):_ `nvidia-smi` memory is the headline on NVIDIA; the fdinfo readings are
    null with a reason there. T14.c's acceptance is "present or null with reason".
  - _CPU budgets (item 3):_ the bake budget (about 40 ms a 65 × 65 patch, Design note 25) stays
    the laptop's figure. The desktop records its own and fails only if over the laptop budget.
  - _Workers (item 4):_ desktop timed runs pin `--workers 3`; T17's separate memory run is
    dropped, and one seed is compared at `--workers 2`.
  - _Memory (item 5):_ the ceilings are unchanged (2–3 GB discrete, 1 GB on the UHD 620); only
    the context changes: the 10 GiB is shared with the local LLM.
  - The display is an Optoma UHD projector (native 3840 × 2160 at 60 Hz; 1080p at 240, 120,
    59.94, 50 or 23.98 Hz, no exact 60), with `Xft.dpi` 75, so the device-pixel ratio is
    0.78125.
- **Findings for the brainstorm, for the owner** (T19 drafts the edits):
  - The finest level's spacing (Design note 3): "sampled at 0.5 m" and "about a third" hold in one
    dimension only; in two, the finest level's largest spacing must be at most 0.375 m, which is
    level 19 on an Earth, with 17.7 m patches rather than "32 m patches", and a demand cap near 89 m
    rather than 160 m. "Its 0.5 m spacing is a quarter of the 2 m band limit" is a mean.
  - Patch demand (Design note 19): the vertical constant re-derives as about 340, not 290; and "the
    cap as 1/τ" should read 1 ÷ (τ θ_px). Below k ≈ 3 the quadtree's granularity sets a floor of
    about 36 patches a level ring (R10's count model, 2026-09-29), so the low setting's patch counts
    are about a quarter of the high setting's, not a ninth, and its demand is probably above a ninth
    too; T13.a's fixed-step run measures the ratio rather than assuming it.
  - Timestamps (Design note 18): the forced switches do not give uncoarsened timestamps; Chromium
    quantises them to 65.5 µs unless `timestamp_quantization` is disabled, which the measurement
    runs do.
  - The atmosphere (Design note 16): the per-planet tables depend on the atmosphere alone and need
    no rebuild when the sun moves; the 32 km aerial-perspective reach is Hillaire 2020's and Bevy's
    figure, where sebh's reference code reaches 128 km; Hillaire's reference aerosol is 20–40 times
    cleaner than Earth's typical sky, so "Earth's reference atmosphere" is taken as Earth's measured
    aerosol, with Hillaire's as a comparison mode.
  - The aerosol asymmetry (decisions-r05.md item 1): Cornette–Shanks g = 0.76 is a mean cosine of
    0.81; Earth's measured 0.65 is g ≈ 0.58. Applied to the brainstorm on 2026-10-02.
  - The Rayleigh column (decisions-r05.md item 3): an 8 km scale height at the sea-level density
    leaves the column 5% short; the US Standard Atmosphere's 8.43 km carries it.
  - Horizon maps are sun-independent (R10's research), so the budget's "it needs rebaking only as
    the sun moves" is wrong: they are baked once with each patch.
  - The low setting's patch counts are about a quarter of the high setting's, not a ninth, because
    of the quadtree's per-ring floor (R10's research); demand's "a ninth" holds only while k exceeds
    about 3.
  - Normals at twice the mesh's resolution quadruple the gradients the bake budget counts (Design
    note 25).
  - The worker's policy (Design note 11): a `file://` worker has no policy of its own, so
    `'wasm-unsafe-eval'` is needed only for compilation on the render thread. Ruled 2026-09-30
    (R04.T10.a): the policy is unchanged and the render thread never compiles WebAssembly; the
    brainstorm already carries the ruling.
- **Timings measured so far are provisional.** The development machine is shared with other
  agents' tests; every figure in this plan measured today, including the research agents' Threefry
  timing and the estimates built on it, is re-measured on a quiet machine (Design note 27) before it
  decides anything.
- **The record's module runner** (2026-10-06, lane D, R05.T13.a's follow-up). The record's
  script (`just descent-demand`) runs its TypeScript through Vite's module runner (`runnerImport`),
  where each read of an imported binding is a getter call. The bundled game reads them directly, and
  R05.T7 perf (d) bound selection's hot imports once at module level for that reason. The record's
  selection times therefore need not be the game's, and its summary says so beside the timing
  columns (record version 4).
  - _Measured on today's code_ (`target/laneD/rcpu/ab.sh` and `rb.mjs`, scratch, not committed):
    - The 14 fixture windows (seed 7, ridges off, min(hard, 4σ_n), 64 Hz) ran through `runCell`
      on the exact CPU clock, five measured passes after one of warm-up.
    - Each form ran in its own process: the runner, and an esbuild bundle of `demandRecord.ts`, the
      game's form, as perf (d) bundled its variants.
    - Four rounds of each, interleaved ABBA, under `just _locked`, nice 0, schedutil, at loads
      6–11.
    - Both forms gave the 14 pinned hashes.

    | Selection CPU time (ms), range over rounds | Runner                    | Bundle                    |
    | ------------------------------------------ | ------------------------- | ------------------------- |
    | High: sum over 4,515 selections            | 988–1,104                 | 1,115–1,259               |
    | High: p50 / p95                            | 0.163–0.170 / 0.697–0.738 | 0.167–0.174 / 0.851–0.931 |
    | Low: sum over 4,515 selections             | 672–789                   | 774–833                   |
    | Low: p50 / p95                             | 0.082–0.088 / 0.532–0.610 | 0.086–0.088 / 0.644–0.734 |

    The bundle was not faster: about 14% more CPU time in all, p95 about 20% higher, p50 within
    5%. Why was not examined. So on these windows the record's selection times do not overstate a
    bundled build's.

  - _Not the runner against the bundle._ Lane B's 2.75 against 1.77 ms (perf (d), "In the game's
    form") is the old selection against the new, both bundled. On the record's path the same step
    went from 2.68 to 1.43 ms (CPU p95). No measurement yet sets the runner beside the bundle on the
    ridged high approach, where perf (d) found the getters costly before it bound them.
  - _Bundling the record's runner: described, not built._ The script would build `demandRecord.ts`,
    `machineLoad.ts` and `threadCpuClock.ts` into one ES module before importing it, by file URL
    (for Windows), from a cache directory. The tool would be Vite's own `build` (rolldown, SSR
    mode, which needs a config and a written output) or esbuild, which bundles it in one call (a
    scratch build did) but is only a dependency of the toolchain, not of the app. That is not
    trivial, and by the measurement above it would not bring the times nearer the game's. Neither
    Node form is the renderer's rolldown bundle running in Chromium's V8. The figures of record for
    selection time stay the perf A/Bs and the owner's runs of the built client (T16, T17).
- **The cache layout is R10's, built here.** R05 builds fixed slots, storage buffers and the
  instanced draw to R10's specification (Design note 10) so that nothing is rebuilt; if R10's
  plan changes the layout before T8 runs, T8 follows R10. `BakedOffsets` is barred from the low
  setting by its 64 MiB budget, whatever T18 finds.
- **Module workers from `file://`** are undocumented in Electron 44 (researched 2026-09-29, medium
  confidence). R04.T10.c's probe worker loads from `file://` in the built app (checked by hand,
  2026-09-30), so the risk is now small; T10.b's smoke run confirms it for the height worker. The
  fallback, a privileged custom scheme, touches R01's main process and the policy's origin and
  needs the owner.
- **The test planet is not the real function.** Its cost per point, its spectrum and its bound
  stand in for R09's, whose Dendry channels and crater octaves the brainstorm estimates at 1 to 3 µs
  a point on their own. A spike that passes on the test planet passes for a function of the test
  planet's cost; T3.c's measured cost and the headroom row are what R09 must stay within, and R09's
  own benchmarks re-run the descent (its scripted path and harness are kept for that). The
  researched σ_h, the break at about 2.4 km and the noise basis's certified bound are estimates
  until T3 measures them.
- **The hard bound may be loose.** If T6 finds it looser than four times the 99.9th percentile,
  selection over-refines and demand rises. Whether selection then takes a calibrated bound
  (R10's min(hard, 4σ)) is ruled (2026-10-02, decisions-r05.md item 6): R05 selects by the hard
  bound everywhere, gate runs included; R10.T4 applies min(hard, kσ) under the criterion recorded
  there, from T6's and its own recorded figures. Re-ruled for both settings on the whole-descent
  records (2026-10-04, `decision-r05-high-bound.md`): unchanged. With ridges off, at the pass's
  τ ÷ 1.1 the high setting meets the `BakedOffsets` budget over 34% of the arc's frames and 30%
  of the approach's, with τ′ at most 0.96 px ≤ τ (decision-r05-record-tau.md, the re-run of
  2026-10-05). The drawn bound stays within τ at each selection, but `DETAIL LIMITED` shows there.
  Low never meets its budget. With ridges on, min(hard, 4σ_n) is not a bound: at levels 4–8, p99.9
  reaches 1.23 × 4σ_n and the patch maxima 1.87 × 4σ_n, and σ_n leaves out the included crests'
  interpolation error, which ε_n carries at levels 9–12. The budgeted hard selection's τ′ of
  2–3 px corresponds to about 0.4–1.0 px of measured error. So the high setting shows
  `TERRAIN: DETAIL LIMITED` for most of the ridged coast, arc and approach: true of the stated
  bound, and conservative of the picture.
- **The budget and the morph.** A `limited` selection's leaves meet τ′ = τ × its refused split's
  excess, not τ. Morph bands at the setting's τ therefore leave coarse–fine edges unmorphed:
  T-junction steps under the skirts, and splits that appear without a morph. Ruled
  (`decision-r05-high-bound.md`): selection returns the excess (`Selection.limitExcess`), and the
  terrain pass sets its bands at τ × max(1, excess ÷ w). Until that lands, the ridged runs'
  limited segments may show seams or pops of up to τ′ in bound terms. **Landed (2026-10-04):**
  F1 and F2 (T7 and T11.c, as built). The bands now move with τ′ at each selection; T13.a's
  record gives τ′'s steps between selections (F4, below).
- **The cadence's 1%** (decision-r05-record-tau.md, "Noted for lane C"; **fixed 2026-10-05**).
  The pass selects at τ_sel = τ ÷ (1 + m), m = `SELECTION_MARGIN` = 0.1 (`RESELECT_FRACTION`
  until 2026-10-06, R05.T13.a's follow-up). It re-selected on a move of more than m × d_min, where
  d_min is the box distance to the nearest selected non-finest patch. That held the drawn error to
  τ ÷ (1 − m²), about 1.0101 τ, between selections, not to τ.
  A leaf at d ≥ d_min with ρ ≤ τ_sel is still (1 − m) d away after such a move, so ρ ≤ τ_sel ÷
  (1 − m).
  - _The fix._ The pass re-selects on a move of more than `RESELECT_MOVE_FRACTION` = m ÷ (1 + m),
    about 0.0909, × d_min (`selectionTolerance.ts`, derived from `SELECTION_MARGIN`). Then
    1 − m ÷ (1 + m) = 1 ÷ (1 + m) and ρ ≤ τ_sel × (1 + m) = τ exactly. Where the budget binds,
    the bound is (1 + m) τ′, the tolerance of the morph bands. So a coarse–fine edge now stays at
    morph 1 between selections too. Under the old move it could fall to 0.99 of its band's end,
    about morph 0.93 for a level whose bound halves, a step the skirts covered.
  - _Unchanged._ τ_sel stays, and so does T13.a's record, which selects every frame and never
    reads the move. Its 14 pinned window hashes reproduce. Its header's "at most 0.1 × d_min
    earlier" still holds, now at most 0.0909. Since R05.T13.a's follow-up (2026-10-06) the header
    says "at most m ÷ (1 + m) = 0.0909 × d_min earlier".
  - _Not covered, as before._ Unbaked leaves, which the streaming gate holds and
    `TERRAIN: STREAMING` reports, are outside the bound. The floor of one finest patch (20.7 m)
    on d_min loosens it for no leaf that meets τ_sel at the settings' views: a level-18 leaf meets
    τ_sel only beyond 45 m, on low at 640 px wide.
  - _Tests._ `selectionTolerance.test.ts` places the worst leaf at d_min with ρ = τ_sel. At
    m ÷ (1 + m) its ρ after the move is τ to rounding, and at m it is τ ÷ (1 − m²) > 1.01 τ, on
    both settings at level 18. `terrainPass.test.ts` streams a low camera at 1.5 km.
    Every leaf it sees stays within τ at the farthest pose the pass keeps the selection for. A
    move halfway between m ÷ (1 + m) and m of d_min re-selects; that test fails on the old rule.
  - _The cost, measured_ (a CPU-only replay of the pass's cadence over the record's descent).
    - Method: seed 7, ridges off, the hard bound, the record's ideal pool and views, at 64 Hz.
      Ten windows of 20 s (the vertical descent's 10 s), each after 3 s of warm-up from an empty
      cache. Each window ran at both moves.
    - The move rule's own selections rose by about 1 + m: 57 → 65 on high and 133 → 143 on low,
      190 → 208 together (+9.5%).
    - All selections rose 0.3% on high (3,656 → 3,666) and 1.2% on low (3,004 → 3,039), so the
      share of frames that select went from 0.300 to 0.301 and from 0.247 to 0.250.
    - Other triggers decide most selections:
      - stored bakes;
      - the camera's turn of more than a pixel, half or more of the selections in the coast and
        the early arc;
      - the contacts. While the craft descends (`isDescending`), its ground contact moves every
        frame, so the pass selects every frame whatever the move: in these windows, the
        approach's at t 1053–1073 s and the slowdown's.
- **Coarse patches evicted under load.** In the approach (probe, 2.4 km, both ridge settings),
  patches of levels 2–12 are evicted and re-baked within a second, while the descending contact's
  finest-level region turns over 100–260 bakes a second under an ideal pool. A horizon patch that
  returns is then a non-resident leaf, drawn by its parent at up to 6.8 px (ridges off) and about
  45 px (on) of bound, for a frame or more. It is not a matter of the selection bound. The cause
  and the fix are T8's, as built: see "T8, as built (lane B, with F3 of
  `decision-r05-high-bound.md`, 2026-10-04)". The patches had not left the view. Selection had
  culled them by their own baked ranges, so `retain` never touched them, and the cache now marks
  them as used (`Selection.hiddenBaked`). T13.a's F4 record (2026-10-05) measures what is left
  over the whole descent. Returns, drawn by a stand-in of level 12 or coarser, show on 8.5% of
  the ridged high coast's frames (at about 2 px of bound) and 5.2% of its low pass's (up to
  68 px). That is the cache's capacity, as T8 found, not thrash. The forced region's rate is also
  an input to T18's descending thresholds (Design note 9): the record gives 81–85 bakes a second
  over the ridged approach and 11–15 over the slowdown.
- **The descending thresholds** (Design note 9) are provisional: 1 km and 30 s, and
  `FORCED_REGION_RESIDENCY_S` 30 s, until T16 measures how long a forced region takes to become
  resident and T18 sets them.
- **The pass criterion's reserve** (Design note 21) counts only terrain and atmosphere against
  their rows' upper ends. If later plans' passes land above their own rows, the spike's pass does
  not carry over; R12's consolidated runs are where that shows.
- **The scripted path is not an orbit** (ruled 2026-10-04, decision-r05-coast-speed.md). The
  coast flies a ground speed of 7.67 km/s, the track point's, body-fixed. That is the top of the
  band, 6.75 to 7.68 km/s, that a 400 km circular orbit's ground track covers on the rotating
  test planet. Its demand, about 4 a second, is therefore an upper bound: at most 14% above a
  prograde equatorial orbit's and 6% above a non-rotating planet's. The camera itself moves at
  about 8.15 km/s body-fixed. In the inertial frame that is a circular orbit's speed only when
  the track heads west along the equator, and up to 13% above it heading east. Matching an orbit
  would tie the approach azimuth to an inclination and make the speed depend on the seed, and it
  would not change the gate: the low fast pass's 200 a second, not the coast's 4, loads the
  workers. The unlifted low pass's 0.076 m/s climb (Design note 19) is kept likewise.
- **Names from R01, R02 and R04** were written in parallel with those plans and were re-validated
  before T1 (2026-10-02; Consumes, "As built", and the record below). The asks in particular: that R04's loader and probe worker, and its `just gen-surface`
  output, suit module workers under `file://` and a Node-environment test (`initSync` on the
  module's bytes). R01 now provides everything this plan asked of it: the instanced draw,
  `writeBuffer`, the per-pass times and the readback (its Design notes 18–20), and storage buffers
  on materials, `writeTexture` and a render target's depth as a sampled `texture_depth_2d` (its
  Design note 21, R01.T8.a, T8.d and T8.i). T11 and T12.c wait on those R01 tasks being built.
- **The atmosphere on strongly oblate bodies** (R08's; researched 2026-09-29, resolved in R08
  Design note 17). Design note 16's r = √(MN) + h holds within R08's 5% up to f ≈ 0.005, the Earth
  class, which is all this plan draws. Past that, the leading error is not curvature (±f/2 in
  grazing optical depth: 0.17% on Earth, 3.5% on Jupiter, 5.4% on Saturn) but a medium built at one
  g. Somigliana's g_p/g_e − 1 is 16.7% on Jupiter and 32.6% on Saturn, ±7.7% and ±14.1% in
  vertical optical depth about a mid g, and a factor of 2–4 in the limb's density at 10 scale
  heights. With one table the grazing spread is ±0.38% on Earth, ±8.9% on Jupiter and ±14.8% on
  Saturn. An earlier "about 17% at Saturn" was the ratio of polar to equatorial meridional
  curvature, which √(MN) already removes (R08 Design note 17). R08 widens Design note 16 without
  changing its Earth result:
  - gravity-scaled height h·g(φ)/g_ref from Somigliana's normal gravity;
  - transmittance in curvature slices over κ = s·R_α ÷ R_ref, with 1/R_α = cos²α ÷ M + sin²α ÷ N;
  - multiple scattering in latitude bands.

  On Earth each is a single table and s ≈ 1 ± 0.3%, so T12.b and T12.c are built as written.

- **The coarse field's size** (researched 2026-09-29). The spike's synthetic field keeps the
  brainstorm's 15 MB, the top of its 2–15 MB range. R09 Design note 17's layout gives 11.6 MB for
  an Earth at level 8: 393,216 cells × 21 B = 8.26 MB, plus 98,304 groups of four × 34 B =
  3.34 MB. That is at most about 12.8 MB if the climate layer grows as R09 allows, and the field's
  level is capped at 8, so no larger body exceeds it. So 15 MB stays an upper bound, where 12 MB
  would stop being one once the climate layer grows. The gap is 1–2% of the 1 GB ceiling with
  three workers. The size is one named constant, `SYNTHETIC_FIELD_BYTES`, in `spikeScene.ts`,
  citing the brainstorm for its value and R09 Design note 17 for the current estimate, so that a
  larger R09 layout changes one line.
- **Measurement switches in a shipped binary.** The timestamp and Dawn-toggle switches are set only
  when the spike flag is given; T14.b's test holds that the ordinary launch sets exactly R01's
  switches.
- **Asked by later plans.** Met here: R07's `PlanetGeometry` of a reference spheroid at zero height
  with no height worker (`planetGeometry(figure, null)`, T7.a), R11's extra `GroundContact`s and
  `FORCED_REGION_RESIDENCY_S` (T7.c) and its `BAND_LIMIT_M` export (T5), R12's `QualitySetting`,
  `SETTINGS` and `AllocationTally` (T7.b, T11.a), and R10's height source as an argument:
  `bake_patch`, `finest_surface_height` and the level bound read a `HeightSource`, not `&TestPlanet`
  (Provides, T4.a, T4.c), which R10 implements over R09's `Synthesiser`. R08 changes this plan's
  atmosphere code in its own tasks: transmittance stored as optical depth, the ozone term through a
  curve of growth, and the channel wavelengths refitted (R08 Design notes 5 and 8).
- **Default device limits** (found on re-validation, 2026-10-02). Decided 2026-10-02
  (decisions-r06-r07.md item 7): option (a), `min(adapter, 1 GiB)` for
  `maxStorageBufferBindingSize` and `maxBufferSize`, with `FaceDifferences` on the high setting
  where the device's limit cannot hold `BakedOffsets`; not paging. Built in T11.a (its as-built
  record below). The finding as first written: R01's engine requests its
  device with WebGPU's default limits: 128 MiB a storage-buffer binding, 256 MiB a buffer, 256
  texture-array layers, 8,192 texels a 2D side. The low layout and the high layout's heights fit;
  the normals fit as a 2D atlas, not an array; `BakedOffsets` at the high budget's about 1,900
  slots (about 193 MB of offsets) does not fit one binding. T11.a settles it before building the
  high layout, by one of: (a) R01's `createWebGpuEngine` asks for the adapter's
  `maxStorageBufferBindingSize` and `maxBufferSize` (capped, say at 1 GiB), with a test, which
  touches R01's adapter and so goes through the orchestrator; (b) the offsets split over pages of
  at most 128 MiB, one instanced draw a page; (c) `BakedOffsets` capped on the high setting at the
  slots that fit, about 1,300. The lean is (a): one request, no change to the draw, and both target
  adapters offer far more. Until settled, T18's choice of vertex path for the high setting must
  count it.
- **Files shared with other lanes.** `view/engine/catalogue.ts` and `renderer/src/smoke/page.ts`
  (T11.b, T12.b, T12.c, and R06's and R07's passes), `view/engine/memory.ts` (T11.a, T12.b, T12.c,
  and R10's categories later), `crates/hyperion-sim/tests/golden/rng/tags.golden` (T3.a, and any
  lane adding a tag), `displays/view/viewRun.ts` (T9, and R07's main screen), `justfile` (T13.c,
  T15.b) and `view/quality/qualitySetting.ts` (T7.b, T12.c, then R06, R07, R08 and R12). Each is
  appended to, never reordered; a conflict keeps both sides, and `tags.golden` is re-blessed after
  a merge rather than merged by hand.
- **Re-validated at ce7aeb3** (2026-10-02, after RM1 merged). Swept every Consumes item against
  the code and folded in R01's, R02's and R04's as-built deviations and RM1's decision records
  (Babylon dropped and the WGSL binding convention, the hardware decisions, the protocol
  decision dropping the post-process `depth` input, the CSP ruled unchanged, `height.worker.ts`).
  The brainstorm has changed since the plan was written only in the CSP passages, which the plan
  already carried. What changed here: Consumes gained an "As built" entry with the real names;
  Provides' sketches of `ViewSelectionInput` (a body-fixed camera, since R02's poses never rotate),
  `HeightWorkerPool` (a worker factory), the atmosphere's camera (`AtmosphereCamera`, not R02's
  `ViewFrame`), the wasm exports' JavaScript names and the counting fake's file
  (`test/countingRenderEngine.ts`, R01's `fakeRenderEngine.ts` being another fake); Design notes
  10, 14, 18, 20 and 23 gained the as-built seams; the task-order notes were refined and the
  steps pending for the owner named; T1.a and T3.c add the surface crate's dev-dependencies; T3.a's
  tag and stream, T5's and T6's exports, T7.a's own culling types, T9's `labelStatements`, T10.b's
  worker against R04's loader as built (`?inline` and `initSync`, the worker tsconfig,
  `Float16Array`'s lib), T11.a's instance storage buffer, normal atlas and tally, T11.b's and
  T12.b's catalogue entries and smoke check groups, T12.a's script runner, T12.c's depth binding,
  T13.b's scene, T13.c's launch path and first IPC handlers, T14.a's and T15.a's device seam,
  T14.b's switch merge, and the hidden and by-hand halves of T10.b, T11.c, T12.c, T13.c, T14.c,
  T15.c and T17 were corrected. Nothing built changes. No task is pending re-validation.
- **Deviations in T1.a, as built.** The warp's round trip is exact at 0, ¼, ½, ¾ and 1, within
  one ulp of s for s ≥ ½, and within 2⁻⁵³ absolute below ½: there the warp works in 1 − s, whose
  ulp in [½, 1) is 2⁻⁵³, so s cannot be recovered more finely. `face_of` (crate-private) and
  `unit_dir` hold the face rule and the normalisation; `Face` also converts with `TryFrom<u8>`
  (`DecodeFaceError`). The module documentation lists the operations the TypeScript mirror must
  keep bit for bit (`(4s)s − 1`, a division by 3, `sqrt(x² + y² + z²)` not `Math.hypot`, unary
  negation's −0).
- **Deviations in T1.b, as built.** `edge_neighbour` folds the step over the cube in exact integer
  arithmetic (half-cell units on S2's axes) rather than reading a table per face pair; T2's
  golden pins all 24 directed face crossings in its `table` section. Added:
  `edge_neighbour_and_back` (the neighbour and the edge leading back, since two faces' axes
  differ), `PatchKey::new` with `NewPatchKeyError` (wrapped by `DecodePatchKeyError::Range`), the
  getters `face`, `level`, `i`, `j`, and `geometry::MAX_FINEST_SPACING_M` = 0.75 ×
  `FINEST_SPACING_M` (0.375 m), which `finest_level` tests against. `vertex_spacing` uses closed
  forms of the arc rate per unit s instead of a 2,048² grid: largest (4 ÷ 3)√(1 + 3u*) ÷ (1 + u*²)
  at u* = (√31 − 2) ÷ 9, 1.704 897; smallest 2√2 ÷ 3 at an edge's midpoint; mean 1.459 214, an
  integral a test recomputes; a 512² grid test checks all three. Pinned: the Moon (1,737.4 km)
  is level 17 (largest spacing 0.353 m), Ceres (469.7 km) level 16 (0.191 m; level 15 misses by
  1.8%, at 0.382 m). `MAX_LEVEL` and `PATCH_QUADS` live in `cube`, the spacing constants in
  `geometry`.
- **Deviations in T2's Rust half, as built.** For each of the 50 patches `cube_sphere.golden`
  prints 81 vertices (x, y ∈ {0, 1, 8, 16, 32, 48, 56, 63, 64}) and a `digest` of all 4,225
  directions, `hyperion_testkit::golden::f64_digest` (FNV-1a 64 over each `f64`'s little-endian
  bits, added here beside T5's `f32_digest`), since every vertex would make the file about 30 MB;
  as built it is 360 KB. The TypeScript mirror reproduces the digest over a `Float64Array`'s
  bytes (BigInt arithmetic). The record format is in `tests/cube_golden.rs`'s module
  documentation. A native-only test holds every surface golden to `TEST_PLANET_VERSION`, and the
  sim-determinism skill's `golden_diff.py` now checks the surface crate's goldens against
  `TEST_PLANET_VERSION` rather than `GENERATOR_VERSION`.
- **Deviations in T3.a, as built.** `NOISE_BOUND` B = 1.0681 (grid maximum 1.036 35 at 1/256 of a
  cell over the 1/48 its symmetry leaves, plus a Lipschitz margin of 0.0317) and `NOISE_RMS`
  σ_noise = 0.2701 (10⁶ points; 0.270 12), both recomputed by `noise_bound_certified` and
  `noise_rms_measured` under `just test-slow`. The gradient index is the corner word's top four
  bits; the object word packs i and j as 32-bit two's complements and the draw number is
  (octave << 32) | k; the octave offsets read words 2⁴⁰ + 4n + axis of object 0. `Octave` holds
  the index, spacing, rotation, offset and seed; `LatticeCache::cover(octave, centre, radius)`
  sets an octave's dense box (capped at 2²² corners). `num::max` and `min` refuse NaN.
- **Deviations in T3.b, as built.** `Spheroid` lives in `hyperion_surface::spheroid` (P14.T46.e's
  module), with R05's `point`, `normal`, `surface_point`, `curvature_radii_m`,
  `section_radius_m` and `WGS84`; `test_planet` re-exports it. σ_h is Earth2014's TBI layer,
  **2,508.2 m** (pyshtools 4.14.1, degrees 1–2160; Hirt and Rexer 2015 Table 2: 2,508.3 m), not
  the design's 2.45 km; octaves 0–8 are rescaled together by the pinned `COARSE_RESCALE` =
  1.055 66 over a 10⁴-point Fibonacci sphere, giving a realised 2,509.3 m. Octaves finer than 35
  km hold 0.179% of the variance. The fade of the newest octave across the morph zone is the
  morph itself (the morph target is the parent level's height, which lacks that octave), so the
  height function has no fade of its own. Ridges: r = 1 − √(n² + ε²) with ε = 0.05, centred and
  scaled by the pinned `RIDGE_MEAN` 0.7692 and `RIDGE_RMS` 0.1488, gated by a smoothstep of
  ½ + (n₂ + n₃) ÷ (4 σ_noise). Rotations are those of integer quaternions (k + 2, 1 + k mod 3,
  2 + k mod 5, 1 + k mod 7). Leakage above the 2 m band limit: **0.149 mm RMS** over 1,000
  profiles at 0.125 m. Each octave's sample mean is tested for octaves 6–21 only: octaves 0–5
  cover the sphere in a few cells, so their samples are not independent.
- **Deviations in T3.c, as built.** Provisional bench (2026-10-03, load average 5–10), with each
  planet's octave table built once per `LatticeCache` (`take_octaves`, `restore_octaves`;
  bit-identical, every golden unchanged) rather than once a point: **4.1 µs a point cached,
  4.6 µs uncached** (5.5 and 6.1 µs before the table; budget 10 µs); the quiet-machine run is taken in a quiet window the
  orchestrator schedules (decisions-r05.md item 7).
  `TestPlanet::cover_patch` prepares a bake's cache. Criterion is a dev-dependency off the
  browser target, where the bench is an empty program.
- **Deviations in T4.a, as built.** The patch origin is the surface point of vertex (32, 32) at
  its own height (`PatchBake::origin_height_m` added), so that the `FaceDifferences` path's
  (h − h₀) is small. `PatchBake` gains `skirt_depth_m` (ε_n plus the `f32` step of the largest
  |h| plus `BakeOptions::skirt_m`). `HeightSource` gains `prepare_cache(cache, key)`, default
  no-op. `PatchKey::sample_dir(x, y, per_patch)` (64 or 128) gives the double-resolution normals'
  samples by the canonical rule. The level bound (T6's `level_bound_m`) landed with T4.a, which
  the skirts and `HeightSource` need.
- **Deviations in T4.b, as built.** `patch::vertex::{PatchTerms, PatchTermsF32,
face_difference_position, face_difference_position_f32, face_difference_morph_f32,
naive_position_f32}`. The warp's difference is 4 δs (s + s₀) ÷ 3 (or (2 − s − s₀)), with a
  level-0 patch, which straddles s = ½, forming u(s) − u(s₀) directly; a morph target at an odd
  vertex is the mean of the formula at its two even neighbours. Worst errors at level 19 over 100
  patches: baked offsets 0.7 µm, face differences 0.48 mm (the `f32` height's own step), naive
  0.89 m. The golden prints, per vertex, the two neighbours' morph heights an odd vertex reads.
  `hyperion_testkit::float::bits_f32` added.
- **Deviations in T4.c, as built.** The interpolation weights are the query's (s, t) fractions
  within its quad, not barycentrics of the flat 3D triangle (a second-order difference); a query
  within 10⁻⁶ of a quad of a lattice line is snapped onto it, so that a vertex's own direction
  returns its height bit for bit. `collision::mesh_height(source, dir, level, cache)` exposes the
  same interpolant at any level, for T6's test.
- **Deviations in T5, as built.** `bakePatch(face, level, i, j, vertexPath, normals, ridged,
skirtM)` bakes the test planet (with the ridges switch) and returns a `BakedPatch` whose
  getters copy each array out; the layout is in `src/wasm.rs`'s module documentation.
  `levelTable(ridged)`. `f32_digest` beside T2's `f64_digest`.
- **Deviations in T6, as built.** ε_n = the omitted octaves' maxima + ½ ‖H‖ h_n² + ¼ |∇F| κ a Δ²
  for level n and for the finest (Waldron 1998's ½ M r² on the mesh's triangles, not Design note
  15's one-dimensional h² ÷ 8, plus the warp's term), with C₁ = 6.70 and C₂ = 32.82 the noise's
  certified gradient and Hessian bounds (grid at 1/512 plus Lipschitz margins) and κ = 3.67 the
  face's parametric curvature. No sample exceeded it; ridges off, the 99.9th percentile is 0.27–0.33
  of ε_n (level 18, which omits no octave, 0.04), 0.70–0.81 of 4σ_n, and every patch maximum is
  within 1.25 × 4σ_n; ridges on, the 99.9th percentile is under a quarter of ε_n at levels 2–15
  and 2–4% at levels 5–10 (findings; the crests' curvature grows as 1 ÷ ε). Both tables are in
  `src/test_planet/bound.rs`'s module documentation. k_n at 1080p, 60°: 1.3 at level 0 rising to about 15 at levels 10–14 (table in
  `src/test_planet/bound.rs`). The bound is checked at 16 patches × 625 random points per level;
  the per-patch maxima of decisions-r05.md item 6 are taken over all 65 × 65 vertices of 32
  further patches a level (where both meshes pass through their functions' values, so the
  distance is |F_n − F_f|), whose share within 1.25 × 4σ_n resolves about 3%, not R10's 99%: a
  recorded figure, not R10's test. σ_n is the omitted octaves' RMS at full weight (no fade
  weight: the morph is the fade, T3.b), its value at morph 0.
- **T6's bound tightened per octave** (2026-10-03, decision-r05-patch-demand.md section 4c).
  Each octave's interpolation error takes the least of three true bounds, since linear
  interpolation is linear: curvature (Waldron 1998's ½ M r², as before), slope (`G_k h_L`, from
  Jensen and the circumradius identity Σλᵢ|p − vᵢ|² = R² − |p − c|² on the right isosceles
  triangles, R = Δ ÷ √2) and range (the width of the octave's value interval; for a ridged octave
  σ_k (√(B² + ε²) − ε) ÷ r_rms). The derivation is in `src/test_planet/bound.rs`. Ridges off,
  nothing changes but one last bit (level 14, from the new summation order). Ridges on, ε_n falls at
  levels 5–12: k_9 from 230 to 51 (the decision's unmeasured estimate was about 40), and p99.9 ÷ ε
  rises to 8–17% there. `level_bound_holds` still finds every sample below the bound. The level
  table and the ridged bake's skirt depth changed, so `TEST_PLANET_VERSION` is 2 (every surface
  golden re-blessed; `EXPECTED_TEST_PLANET_VERSION` in `heightBake.ts` and `cube.test.ts`
  follow); `GENERATOR_VERSION` is unchanged.
- **The drawn surface's height exported** (2026-10-03, for the spike). The wasm module's
  `surfaceHeightM(x, y, z, ridges): number` is T4.c's `finest_surface_height` over the test
  planet at a body-fixed direction (not necessarily unit), so the client never ports the
  interpolant; it throws for a zero or non-finite direction. A new golden,
  `tests/golden/collision.golden` (27 directions, ridges off and on, a face-edge vertex, an
  interior vertex and a cube corner among them), pins it natively, and `heightWasm.test.ts`
  checks the export against it bit for bit. New values only: no `TEST_PLANET_VERSION` bump.
- **σ_n exported** (2026-10-03, for T13.a). `TestPlanet::omitted_sigma_m(level)` and the wasm
  module's `omittedSigmaM(level, ridges)` give σ_n, the RMS of the octaves level n omits: the
  octaves' variances summed in index order, a ridged octave counting at its mask's RMS
  (`RIDGE_MASK_RMS` = 0.6281, measured over 10⁶ points of the planet and pinned by
  `the_ridge_mask_rms_is_pinned`), 0 from level 18 on. It is not the same with ridges on and off:
  at levels 0–8, which omit a ridged octave, it is smaller with ridges on (98.6 m against 155.8 m
  at level 4), and the same from level 9. `level_table.golden` gains `sigma` lines (new values
  only, so no `TEST_PLANET_VERSION` bump), and `level_bound_holds` reads it. **A finding:** with
  the ridged octaves at their true RMS, the ridged planet's error is far from Gaussian at levels
  4–8 (p99.9 1.0–1.23 × 4σ_n, patch maxima up to 1.87 × 4σ_n, 59–88% of patches within
  1.25 × 4σ_n), so min(ε_n, 4σ_n) is not a safe bound there; R10.T4's criteria (i) and (ii) would
  both fail on it.
- **Decisions-r05.md item 6, the ridged planet** (ruled 2026-10-02 on this lane's question). With
  ridges on the hard bound is 10–50 times the 99.9th percentile at levels 5–12 (k_n up to 230,
  against about 15 with ridges off), so selection by it over-refines the ridged planet heavily.
  Ruled: T13.a also records the patch counts and demand under min(hard, 4σ_n) for the ridged
  planet, and T19 judges a ridged run's streaming failure caused only by that over-refinement as
  "ours to fix", never a fired rule.
- **Review fixes to T1–T6** (43c1613 and the commit after it). `LatticeCache` boxes serve only
  the identical octave (a cache reused across planets or seeds never returns another's
  gradients); every height is asserted finite in the normals and the collision interpolant too;
  `cube::SampleGrid` (`Mesh`, `Double`) replaces `sample_dir`'s raw quads; the wasm exports take
  `VertexPath`, `NormalScale` and `Ridges` enums, and `testPlanetVersion()` is exported so a stale
  module is caught when the test planet changes; `Octave::new` refuses a rotation that is not
  orthonormal; `Spheroid::sphere` refuses a non-positive radius; `HeightSource::Error` is bounded
  by `std::error::Error + Send + Sync`; `test_planet` re-exports `LatticeCache` (its home is
  `noise`), the path R09.T4 moves it from; `spheroid` also carries P14.T46.e's
  `from_volumetric`, `flattening`, `volumetric_radius_m` and `BuildSpheroidError`, written by this
  lane to the API the orchestrator gave (P14.T46.e adds no second copy). The `FaceDifferences`
  formula forms a face-edge vertex from its own face's (u, v), not the canonical face's (an ulp
  the skirts cover), and `vertex_f32.golden` prints vertices x, y ∈ {0, 1, 31, 32, 33, 63, 64} of
  each patch. T3.c's quiet-machine bench is re-measured in a quiet window the orchestrator
  schedules (decisions-r05.md item 7), not by the owner. `src/wasm.rs` has no tests of its own: `just test-wasm-browser`
  requires the browser target's test list to equal the native one, and the module exists only on
  the browser target. Its exports are one-line wrappers; the bake's array lengths are tested
  natively (`patch` tests), and the bake golden runs under Electron's V8. The test planet's octave
  table is now built once per `LatticeCache` (`take_octaves`, `restore_octaves`), bit-identical
  (every golden unchanged), and a cache box is matched to its octave by seed on each lookup (the
  whole octave in debug builds).
- **Deviations in T14.b, as built** (2026-10-02).
  - `main/spike.ts` exports `launchSwitches(options, spike)`: with `spike` undefined it returns
    R01's `graphicsSwitches(options)` unchanged; otherwise it turns `gpuTiming` on and, with
    `dawnSafety: "off"`, merges Design note 22's toggles (`DAWN_SAFETY_OFF_ENABLED`,
    `DAWN_SAFETY_OFF_DISABLED`) into one `--enable-dawn-features` and one
    `--disable-dawn-features` before `applyGraphicsSwitches`. R01's `LIST_SWITCHES` is now
    exported from `graphics/switches.ts` for it. `index.ts` still calls `graphicsSwitches`: **T13.c
    replaces that call with `launchSwitches(options, spike)`, with the parsed `--dawn-safety`,
    before `ready`**; until then no launch carries the spike's switches.
  - The safety toggles also go on `default`-mode and non-Linux launches, which have no R01 Vulkan
    set (there `--enable-dawn-features` holds the five toggles alone); `safe` mode gets nothing.
    `gpuTiming` still takes effect only in R01's Linux `vulkan` mode, so a run elsewhere records
    `PassTimes.timer` as `quantized` (decisions-r06-r07.md item 8).
  - The trace (`SpikeTrace`, `spikeTraceConfig`, `SPIKE_TRACE_CATEGORIES`) records Design note
    18's categories plus `disabled-by-default-devtools.timeline` (its `RunTask` and `GPUTask`
    slices) and `disabled-by-default-devtools.timeline.frame`, and leaves out `toplevel`, which
    repeats `RunTask` and was about half a recorded trace's bytes. It is `record-until-full` with a
    2 GiB buffer: about 1.1 MB/s was measured on a small WebGPU page, about 1.4 GB over the
    21-minute descent; a full buffer shows as a short `span`. Chromium's tracing service is its
    own utility process, so T14.c reports it apart from the app's memory. Superseded (2026-10-04,
    delegated decision `decision-r05-trace-windows.md`): a whole descent's trace crashed the tracing
    service at its stop (1.35–1.71 GB), so the trace is windowed with a 768 MiB buffer per window,
    and `disabled-by-default-v8.cpu_profiler` is recorded only with `--trace-profile on`
    (T14.d–T14.f). Superseded again (2026-10-05, `decision-r05-trace-windows-2.md`): the trace is
    recorded over CDP as a Perfetto protobuf stream and decoded in the main process (T14.h, T14.i),
    and `gpu` is recorded only in profiled runs, so a timed run's GPU-process slices are `GPUTask`
    alone. The split's "our code" is the union of the per-frame `spike.frame` spans (T14.g).
  - `main/reduceTrace.ts` (`TraceReducer`, `reduceTrace`, `reduceTraceFile`,
    `readTraceEvents`) streams Chromium's one-event-a-line layout, since a descent's trace is too
    large for one `JSON.parse`; `.prettierignore` keeps `src/main/fixtures/*.trace.json` in that
    layout. Frames are the compositor's `PipelineReporter` slices (end = presentation; state
    presented, dropped or not wanted), from the renderer with the most frames and its busiest
    `layer_tree_host_id`, one interval a distinct presentation; a hidden window's frames are all
    "not wanted", hence T14.c's null presentation figures. No trace category carries GPU time per
    pass (that is T14.a's `onPassTimes`), so the "GPU-pass" figures are the GPU process's
    `CrGpuMain` busy time and its `WebGPU`, `GPUTask` and `VulkanQueueSubmitHook` slices, the CPU
    side of the command transport. The engine's self time is V8 CPU-profile samples whose leaf is
    in the `engine-*.js` chunk (`ENGINE_CHUNK_PATTERN`; the built chunk is
    `engine-<hash>.js`). User-timing spans carry their starts, so that T14.c can split
    presentation intervals by segment marks.
  - The test's trace (`src/main/fixtures/spike.trace.json`) was recorded on 2026-10-02 from
    Electron 44.4.3 headless on SwiftShader and trimmed to 190 ms of the events read.
- **Deviations in T12.a, as built** (2026-10-02).
  - _The solar inputs._ NREL is now NLR: E-490-00a is fetched from
    `https://www.nlr.gov/media/docs/libraries/grid/e490_00a_amo.xls?sfvrsn=ce97914b_1`, an `.xls`.
    The script reads a two-column CSV export of its `NewAM0` sheet (the procedure, both SHA-256s and
    the CIE CSV's DOI and checksum are in `src/tools/solarFactors.ts`'s header) and refuses inputs
    whose checksums differ. Neither input is committed (T12.d). No `just` recipe was added; the
    script is `node apps/hyperion/scripts/solarFactors.mjs --e490 <csv> --cie <csv>`.
  - _`solar.ts`'s shape._ It commits E-490's point samples at the three wavelengths, Bruneton's sun
    (λ⁰) and sky (λ⁻³) factors in lm nm W⁻¹ and E-490's photopic illuminance (133,318 lx, 4% above
    R02's 1.28 × 10⁵ lx for V = −26.76). It exposes `sunIlluminanceRgb()` (scaled so that its
    Rec. 709 luminance is R02's) and `skyLuminanceScale()` (the factor from a table's radiance per
    unit spectral irradiance to cd m⁻²), plus `rec709Luminance`. The XYZ-to-Rec. 709 matrix is the
    seven-figure one `toneCurve.ts` carries, not Bruneton's four-figure IEC one.
  - _The medium's shape._ `DensityProfile` is `exponential` or `tent`; `PhaseFunction` is `rayleigh`,
    `cornette-shanks` or `none` (ozone). `AtmosphereMedium` is
    `{ name, topHeightM, groundAlbedo, terms }` with no radius, since the tables take the figure (Design note 16). Added helpers:
    `densityAt`, `columnLengthM`, `extinction`, `termNamed`, `CHANNEL_WAVELENGTHS_NM`.
    `HILLAIRE_REFERENCE` is sebh's `SetupEarthAtmosphere`, with a black ground. R08's sketch of
    these types (`cornetteShanks`, `topAltitudeM`, no `none` phase) follows R05 as built
    (`cornette-shanks`, `topHeightM`, `none` for ozone) when R08 is re-validated.
  - _The citations_ were checked by the science checker on 2026-10-02 and corrected: the aerosol
    climatology (Remer 2008; Levy 2013), the Ångström exponent (Ångström 1929; Dubovik 2002), the
    single-scattering albedo (Levy, Remer and Dubovik 2007), 300 DU and the Dobson unit (WMO/UNEP
    2018), the ground albedo (Bruneton and Neyret 2008, Fig. 6).
  - _Decided 2026-10-02 (delegated; the orchestration's `decisions-r05.md`)._
    - Item 1: Earth's aerosol takes Cornette–Shanks g = 0.584, whose mean cosine, 0.650, is
      AERONET's continental asymmetry at 550 nm (Dubovik et al. 2002, Table 1), in place of the
      brainstorm's 0.76 (mean cosine 0.81); `HILLAIRE_REFERENCE` keeps 0.8. The brainstorm's
      line is corrected. A test holds the mean cosine to 0.650 ± 0.002.
    - Item 2: the ozone values stay Bruneton's and sebh's upward bins, so that both media share
      Hillaire's ozone. Centred bins would be +10%, −6% and −19% at 680, 550 and 440 nm.
    - Item 3: Earth's Rayleigh scale height is the US Standard Atmosphere 1976's sea-level
      R*T₀ ÷ (M₀g₀) = 8,434.5 m, so that τ_R(550) = 0.0969 against Bodhaine et al. 1999's ≈ 0.097;
      `HILLAIRE_REFERENCE` keeps 8 km. Tests recompute the height and hold τ_R(550) to 1% of
      0.097. For R08: its tabulated profile is to reproduce Bodhaine's 0.097 to 1%.
    - Item 4 (T12.d, ruled; the task is closed): neither raw input is committed (the E-490 table
      is ASTM's; the CIE table is CC BY-SA 4.0 and not needed); the derived constants are, with
      the attributions in `solar.ts` and `NOTICE`'s Data section (DOE/NLR, CIE, Serdyuchenko and
      Gorshelev). `NOTICE` gains Bevy (MIT), Bruneton (BSD-3) and sebh (MIT); sebh's full MIT
      text is in `multiScattering.wgsl`. Hillaire 2020's published images stay local and
      untracked; only our own renders may be committed. Nothing packages `NOTICE` into the
      built app yet: an ask of whichever plan adds packaging (R12).
    - Item 5: the two `TERRAIN:` lines are signed off; the guide row loses its draft marker and
      gains "Where both hold, `STREAMING` is shown." (applied here, with Design note 23 and
      Consumes).
- **Deviations in T12.b, as built** (2026-10-02).
  - _The WGSL._ It is ported from Bevy 0.19.1 (tag `v0.19.1`; MIT notice and Bruneton's BSD notice
    in `shaders/common.wgsl`), with sebh's MIT notice in full in `multiScattering.wgsl`, whose
    structure is sebh's. It differs from Bevy as follows:
    - The terms are evaluated per sample from a `Medium` uniform of at most 8 terms (`MAX_TERMS`;
      `packMedium` refuses more), instead of Bevy 0.19's baked density and scattering tables.
    - Every step is sampled at its midpoint. Bevy and sebh sample at 0.3 of each step and drop the
      last step's remainder.
    - Where Bevy's multiple-scattering kernel departs from sebh's, sebh is followed:
      - the sun's direction;
      - the ground bounce with its 1 ÷ π and the sun's transmittance read at the ground;
      - sebh's 8 × 8 stratified directions.
  - _Sampling._ Transmittance takes 256 steps a ray: at 128, one shallow texel from the ground was
    1.2% off the oracle in blue on SwiftShader. Multiple scattering takes 32 steps a ray (sebh's
    reference takes 20).
  - _The sphere._ `AtmosphereTables(engine, medium, bottomRadiusM)` builds once per planet on a
    sphere of one reference radius R_b (the smoke check uses WGS 84's mean radius R₁ =
    6,371,008.8 m). A lookup in these tables reads height as r − R_b, so T12.c turns Design note
    16's r = √(MN) + h into the table's own radius, r_table = R_b + h, with μ measured against
    the spheroid normal; the curvature mismatch is what Design note 16 bounds. Which R_b (R₁ or
    the Gaussian radius at some latitude) is T12.c's to settle and record.
  - _The multiple-scattering table's mapping_ is sebh's (sub-texel remap, the sun's μ across, the
    height up from 10 m above the ground). `multiScatteringRMuToUv` is in `common.wgsl` for T12.c.
    (In `medium.wgsl` since 2026-10-06: "Deviations in T12.b and T12.c, as built: the medium read
    in place".)
  - _The engine seam._ `RenderEngine.readTexture` gained an optional fourth parameter,
    `access: "cpu" | "tolerance"`, mirroring `readBuffer`'s, so that the smoke page can read the
    `presentation-only` tables (in `types.ts`, `webgpu/engine.ts` and `resilientEngine.ts`;
    `engineBoundary.test.ts` already refuses `tolerance` outside `smoke/`). Approved by the
    orchestrator on 2026-10-02 and recorded in R01's Risks.
  - _The memory category._ `MemoryCategory` gained `atmosphere-tables`, appended.
  - _The rebuild rule._ `setMedium` returns whether it rebuilt (by value: the medium and the
    radius), keeps its key only once both dispatches are issued, and `builds` counts builds. On
    the engine's `onRestored` the tables make their textures and kernels again and rebuild, so
    `transmittance` and `multiScattering` are getters to read afresh; `dispose()` stops following
    restores. The multiple-scattering kernel indexes its workgroup arrays by
    `local_invocation_id`.
  - _The counting fake lands here._ `renderer/src/test/countingRenderEngine.ts`
    (`CountingRenderEngine`, `countingRenderEngine()`), named by T11.a, is built now for
    `tables.test.ts` (build, no rebuild, rebuild on change, a failed pack, the restore); T11.a
    extends it.
  - _No engine test of the tolerance read._ `engineBoundary.test.ts` refuses a `tolerance` read
    anywhere outside `smoke/`, tests included, so the smoke check is its test, as for
    `readBuffer`'s.
  - _The smoke check_ (`smoke/atmosphere.ts`, group "R05.T12.b the atmosphere's tables") holds
    each of the 20 texels to 1% of the oracle's transmittance per channel. Below rgba16float's
    least normal, 2⁻¹⁴, the bound is that absolute floor instead, since the stored value has lost
    its relative precision there. The check also asserts that an ordinary readback is refused, and
    passed on both variants on 2026-10-02.
- **Deviations in T7.b, as built (the settings, 2026-10-02).** `view/quality/qualitySetting.ts`
  landed first and alone, for R06.T13.f and R07; `selectPatches` and `screenSpaceErrorPx` follow
  once T2's golden and T6's level table are built. `ViewSettings` has `terrain` only: T12.c adds
  `atmosphere: TableSizes` with its values, as the task says. Added beside the Provides names:
  `QUALITY_SETTINGS` (both settings in order, for tests and menus), and `TerrainNormals` and
  `TerrainVertexPath`, the unions of `TerrainSettings.normals` and `.vertexPath`. The shapes are
  `interface`s (the TypeScript rules), not the sketch's `type`s. The high cache budget is
  400,000,000 B (Design note 10's "about 400 MB"); the low is 64 MiB.
- **Deviations in T8, as built (the patch cache, 2026-10-02).** T8 ran before T2 and T7.a, so it
  wrote the types it reads from them: `PatchBounds` (`bounds.ts`, with `minHeightM` and
  `maxHeightM` for the sketch's `minH` and `maxH`, the unit rule), `BodyFixedVec3` (`planet.ts`),
  `SelectedPatch`, `PatchRequest` and `Selection` (`select.ts`, types only), and the key half of
  T2's `patchKey.ts` (`Face`, `FACES`, `MAX_LEVEL`, `PatchKey`, `rootKey`, `patchKeyString`,
  `parsePatchKeyString`, `isValidPatchKey`, `parentKey`, `childKeys`, with `patchKey.test.ts`);
  T2 checks them against its golden. Added beside the Provides: `ResidentPatch`, `CachedPatch`,
  `CacheInsert` (`stored` with the evicted key, or `refused`), `CachePressure`,
  `PatchCache.insert`/`remove`/`retain`/`pressure`/`heldBytes`, `DrawnPatch`, and
  `DrawSet { patches, slots: Uint32Array, standingIn, missing }`; `slotLayout(fields, budget)`,
  `terrainSlotFields`, `terrainSlotLayout` and `MIN_SLOTS` (6) in `slotLayout.ts`. Choices: the six
  roots are never evicted, so every selected patch keeps a resident ancestor once they are baked;
  an ancestor standing in covers its resident descendants (drawn patches never overlap), and
  `standingIn` counts them; draw pins cover every resident selected patch as well as the drawn
  ones, so the siblings under a stand-in are kept until all arrive; a patch inserted between two
  `retain`s takes the last selection's pins at once, so a forced patch is pinned on arrival; the
  pins are reported as exceeding the slots when the forced patches (resident or not) and the other
  pinned ones outnumber the slots, or an insert was refused, and the report survives the frame's
  `retain`. **Design note 10 corrected (decided 2026-10-02 by the orchestrator): the slot counts
  normals as stored, with the atlas's one-texel gutter**, 67² × 4 = 17,956 B at the mesh's
  resolution and 131² × 4 = 68,644 B doubled, so the low slot is 51,756 B and 64 MiB holds 1,296
  slots, not 1,323 (Design note 10's and T8's figure). `SlotLayout` is the one source of the slot
  count; T11.a reads it and never recomputes it. The R10 test counts R10's
  class weights and survey mask inside the horizon-map field until R10 adds fields of its own.
  The acceptance filter `view/terrain/cache` leaves out `slotLayout.test.ts`; both run under
  `view/terrain`.
- **Deviations in T10.a, as built (the pool, 2026-10-02).** `WorkerLike` has `addEventListener`
  and `removeEventListener` for `message` and `error` (oxlint's `prefer-add-event-listener`), not
  `onmessage`. The constructor also takes `bake: BakeSettings` (the setting's vertex path and
  normals, sent with every bake). Added: `onFailed` (a key whose bake failed is reported and not
  requested again), `currentGeneration`, `queuedCount`, `inFlightCount`, `lostWorkers`,
  `MAX_IN_FLIGHT_PER_WORKER` and `MAX_CONSECUTIVE_WORKER_FAILURES` (3: a worker failing three times
  with no answer between is given up, not restarted forever), and `bakeTransferables` in
  `messages.ts`. Requests go one a worker a round, least loaded first. Once a field is posted, a
  worker is sent bakes only after acknowledging it, and a bake from an older field is baked again.
  The pool keeps the field's bytes (about 15 MB on the render thread, beyond Design note 11's
  count) to post them to a replacement worker. The scripted fake worker lives in `pool.test.ts`.
  `Float16Array`'s lib (`es2025.float16`, the name TypeScript 7.0.2 has) joined both
  `tsconfig.web.json` and `tsconfig.worker.json` here, ahead of T10.b, for `BakedPatch.normals`.
- **Deviations in T10.b, as built (the height worker, 2026-10-03).**
  - The worker's logic is `workers/heightBake.ts` (`answerRequest`, `bakeKey`, `packNormals`,
    `staleModuleMessage`, and the `WASM_*` maps to the module's enums, which the test pins to the
    generated glue); `height.worker.ts` only loads the module and forwards messages, as R04's
    probe worker and `handleRequest.ts` do, so the Node test drives the same code.
  - The worker checks the module's `testPlanetVersion()` against `EXPECTED_TEST_PLANET_VERSION`,
    which a test pins to the crate's `TEST_PLANET_VERSION`, rather than `generatorVersion` with
    `checkGeneratorVersion`: the test planet belongs to no universe, and the spike and the smoke
    page have no server version to compare against. On the client, R04's loader still checks the
    same module file's generator version against the server's.
  - A load failure (worded by `describeLoadFailure`, so a policy refusal reads `csp-refused`), a
    stale module and a trap (`WebAssembly.RuntimeError`) are raised as the worker's error with
    `reportError`: the pool replaces the worker and gives up its place after
    `MAX_CONSECUTIVE_WORKER_FAILURES`. A key the module refuses (a `JsError`) answers
    `bake-failed`. Bakes pass `skirtM` 0.
  - The coarse field is held as the worker's own copy in JavaScript (the structured clone it
    receives), replaced by the next, for the worker's life, not copied into the module's memory:
    the test planet does not read it, and the memory at rest is the same, which Design note 21's
    memory row measures. R09 adds the export that copies it in.
  - `BakeSettings` gains `ridges: TestPlanetRidges` (`"off" | "on"`, Design note 12); `BakedPatch`
    gains `originHeightM` and `skirtDepthM` for T11.a (approved by the orchestrator).
  - `TerrainNormals` and `TerrainVertexPath` moved to `view/quality/terrainKinds.ts`, a file with
    no imports that `qualitySetting.ts` re-exports, so that `tsconfig.worker.json` can include
    `messages.ts` without the render thread's quality modules; it changes lines inside the shared
    `qualitySetting.ts`. The worker config also includes `heightBake.ts`, `patchKey.ts`,
    `planet.ts`, `terrainKinds.ts` and `geometry/vec3.ts`.
  - The built-app check is a smoke-harness group, "R05.T10.b the height worker"
    (`smoke/heightWorker.ts`), run hidden by `just test-render` (SwiftShader, a fresh
    `--user-data-dir`, the page loaded with `loadFile` from `out/`), which exits with a status,
    rather than a one-off DevTools read. Its page, `smoke.html`, refuses WebAssembly on the render
    thread as the client's does (`script-src 'self'`, no `'wasm-unsafe-eval'`); a policy refusal
    fails the check as the worker's error. It does not log the module's content type (R04.T10.c saw
    `application/wasm` by hand). **Result, 2026-10-03: passed** — a level-12 patch baked in a
    module worker from `file://` (heights 8,450, offsets 25,350, normals 33,282, all finite). T13.c's
    `--smoke` still bakes one patch on the client page. `just test-render` does not depend on
    `gen-surface`, so a stale `generated/surface/` would run an old module; run `just gen-surface`
    first (a justfile change for the orchestrator).
  - `src/wasm.rs`'s removed browser-only tests are covered here: `heightWasm.test.ts` checks
    `bandLimitM`, `finestSpacingM` and `testPlanetVersion` against the crate's sources, the level
    table's length, the bake's layout, and the three golden bakes bit for bit, through the real
    module under Node.
- **GPU timing elsewhere than Linux `vulkan` mode is quantized** (decisions-r06-r07.md item 8,
  2026-10-02); frame intervals stay the criterion, and the follow-up (honouring
  `--hyperion-gpu-timing` in `default` mode) is taken only if a marginal row matters.
- **Deviations in T14.c, as built** (2026-10-02).
  - The memory sampler (`sampleMemory`, `MemorySampler`), the results builder (`buildResults`),
    the schema check (`validateResults`), the summary (`summaryMarkdown`), the machine's
    description (`describeMachine`) and the writer (`writeResults`) are in `main/results.ts`, not
    `main/spike.ts`; T13.c's IPC handlers in `spike.ts` call them. `main/fdinfo.ts` holds the DRM
    fdinfo reader (`parseFdinfo`, `sumDrmClients`, `readDrmMemory`) and `nvidia-smi`'s
    (`parseNvidiaSmi`, `readNvidiaSmi`). A total over DRM clients is given only when every client
    gives one.
  - The renderer's report, `DescentSpikeReport` with its `Spike*` types, is in `preload/api.ts`
    (a T13.c file), and fixes T14.a's output: raw per-frame series (script time, rAF interval, the
    frame callback's main-thread time, each pass's GPU time with its row, `terrain`,
    `atmosphere` or `other`), streaming per segment, uploads, late pipelines, the adapter's peak
    and the canvas size. The main process computes the percentiles (nearest rank), missed frames
    and hitches (`frameStats`), so **T14.a builds no `view/spike/percentiles.ts`**: its percentile,
    missed-frame and hitch tests are in `results.test.ts`, and T14.a's acceptance drops that
    filter. T14.a marks each segment with one `performance.measure("spike.segment:<name>")` span
    (`SEGMENT_MEASURE_PREFIX`), by which presentation intervals are split by segment. `bracketed`
    is not carried (always `false`, R01 Design note 24).
  - The headroom row's main-thread figure is each frame's whole `requestAnimationFrame` callback,
    engine submission included (one span a frame, T14.a's contract). It leaves out the browser's
    own work on the thread, so the row is a lower bound; the trace's `mainThread` split is
    recorded beside it.
  - T is `1000 / Display.displayFrequency`, doubled on `low`. A hidden run's T, presentation
    intervals and every row read against T are null with "no window shown"; its rAF intervals
    are still recorded. Frames come from presentation times when there are any, else from rAF.
  - The memory headline is `nvidia-smi`'s device memory used less a baseline taken before the
    launch (T13.c takes it), else the DRM fdinfo's resident peak, else the adapter's tally with a
    note; the GPU process's own `nvidia-smi` figure is recorded beside it, since the device
    figure counts the local LLM too. Only the first GPU `nvidia-smi` lists is read. The DRM
    reading's own reason reaches the file. Chromium's tracing service is reported apart. GB is
    10⁹ B.
  - Files are `<date>-<machine>-<setting>.json` and `.md`, numbered `-2`, `-3` and so on for
    repeats; `<machine>` is the host name, lower-cased, keeping `[a-z0-9-]`. The schema is
    `hyperion.descent-spike.results` version 1, checked by `validateResults`, a TypeScript check;
    T15.c's Rust writer has no schema file to test against, so its test checks the same required
    parts and figures (or T15.c ships a JSON Schema generated from the check).
  - The i915 and amdgpu fdinfo fixtures are written in the kernel's documented layout
    (`drm-usage-stats.rst`; amdgpu in both its `drm-memory-*` and `drm-total-*` forms), since
    neither device is on the development machine; the NVIDIA fdinfo and `nvidia-smi.xml` were
    recorded there (RTX 3080, driver 615.71.09). The owner's UHD 620 runs (T16) read i915 for real.
  - **Result, 2026-10-04: the hidden low-setting run** (lane D,
    `docs/measurements/descent-spike/2026-10-04-effect-low.md`).
    - _The run._
      - `bash target/laneD/smoke.sh --setting low --hidden`, the same as
        `just descent-spike --setting low --hidden` with `TMPDIR` in the worktree. It ran on
        af69a9e, whose level coast decision-r05-coast-speed.md cleared.
      - The server was on port 7893 (`HYPERION_SPIKE_PORT`), off the default 7879 in case
        another lane's smoke run started meanwhile. None did: no other spike process at 12:31,
        12:36, 12:45 or 12:54.
      - It exited 0 after 1,306 s (12:31:32 to 12:53:18 CDT) and sampled memory over 1,229 s.
      - RTX 3080 (PCI 10de:2206), driver 615.71.09, Electron 44.4.3, Chromium 152.0.7977.130,
        Linux `vulkan` mode, timer `full`, seed 7, ridges off, Dawn safety on, face differences,
        mesh normals, hidden, canvas 806 × 431 px.
      - **Timings provisional.** The load average was 10.4 at the start and about 19 at its
        peak, with the machine shared by about ten agents. The file marks the run provisional,
        and its overall verdict is `not-measured`.
    - _The file._ `validateResults` passes. Every figure is present, or null with its reason:
      - "no window shown": T, and the presentation figures whole and by segment;
      - "no DRM client in the process's fdinfo (NVIDIA's driver writes none)": the DRM reading;
      - "the trace has no timed event": the trace and every figure read from it (the finding
        below).
      - The rAF statistics' missed-frame and hitch counts are null inside their figure, since
        they need T; their criterion rows carry "no window shown".
    - _Figures._
      - **Frames (rAF, after the warm-up):** 65,518 intervals; p50 16.70 ms, p95 33.30 ms,
        p99 50.10 ms, max 133.3 ms. These are not read against T.
      - **GPU time, 95th percentile:** terrain 0.57 ms (limit 14 ms, pass); atmosphere 3.73 ms
        (limit 4 ms, pass at 93% of it; its view pass alone is 3.59 ms). The pass sum is
        4.26 ms and our code's main-thread time 7.10 ms, neither read against T. No pass was
        untimed, and no pipeline was created after the warm-up.
      - **Memory:** the headline is 0.252 GB (`nvidia-smi` less its baseline; limit 1 GB,
        pass). The GPU process alone held 192 MiB and the adapter's tally peaked at 75 MiB.
        The app's processes peaked at 3.98 GB, with Chromium's tracing service at 1.71 GB apart.
      - **Uploads:** 3.14 GB.
      - **Streaming, patches a second, requested and baked against the hard bound's D:**

        | Segment            | Requested | Baked |    D | `TERRAIN: STREAMING` (s) |
        | ------------------ | --------: | ----: | ---: | -----------------------: |
        | orbit coast        |       1.6 |   1.6 |  0.7 |                      1.1 |
        | descent arc        |       0.8 |   0.8 |  2.1 |                     14.8 |
        | approach and flare |      14.4 |  13.2 | 29.0 |                     35.7 |
        | low fast pass      |      43.0 |  41.0 | 57.4 |                     22.7 |
        | slowdown           |      48.5 |  40.6 | 34.8 |                     45.6 |
        | vertical descent   |       0.0 |   0.0 | 21.4 |                      0.0 |
        | hover              |       0.0 |   0.0 |  0.0 |                      0.0 |

        Below the cap altitude the vertical descent selects nothing new, as T13.a's windows
        found.

      - This run flew the level coast, so its coast row is current. T13.a's two demand records
        are `just descent-demand` runs and stay stale for the coast. Both were re-run on
        2026-10-05 (F4, T13.a as built).
    - _Fixed here: an empty trace's zeros._
      - The trace held no events. The file as written still gave `frames.dropped` 0 and
        `mainThread.gc` `[]` as if measured.
      - `buildResults` now takes a trace with no timed event as no trace: every figure read from
        it is null with `EMPTY_TRACE_REASON`.
      - `validateResults` now refuses a trace figure present while `run.trace` is null. The
        figures are the frames dropped, the GPU process, the main thread's split, its GC pauses
        and the presentation figures. A native replay is exempt for its presentations, which it
        times without a trace.
      - Three tests fail on the old writer: the empty trace, the refused figures and a client
        run's refused presentations. A fourth holds the native replay's exemption. Reviewed by
        typescript-reviewer: no must-fix, and its should-fix (the presentation test) is applied.
      - The run's results were regenerated by applying the rule to the saved file, since the raw
        report is not kept. The rule changes only the four figures the trace alone gives, so the
        result is what the fixed writer would have written. The summary shows none of them and
        is byte-identical.
    - _The JSON, committed in schema version 2._
      - In version 1 it was 563 KB, over the repository's 500 KB limit on added files
        (`check-added-large-files`). Its 1,228 memory samples took 477 KB.
      - decision-r05-results-size.md (2026-10-04) rules results schema version 2, with the
        memory series as columns of whole KiB (the deviation below). The committed
        `2026-10-04-effect-low.json` is the corrected file converted to it, losslessly, with the
        app's version corrected. Formatted, it is 109,109 B.
      - The corrected v1 file stays uncommitted at `target/laneD/t14c/2026-10-04-effect-low.json`,
        and formatted as `pretty.json`, the conversion's source. The file as written is beside it
        as `.as-written.json`.
    - _Findings._
      - The tracing service crashed writing the trace (the Risks bullet below).
      - The renderer's private memory grew roughly linearly, from 0.26 GB at 60 s to 1.57 GB at
        the end (about 1.1 MB a second). Found since (2026-10-04, the Risks bullet below on the
        renderer's memory): about 1.25 MB/s of it is the trace's own CPU-profiler category, and
        up to about 66 MB was the patch cache keeping each bake's arrays, now fixed.
      - `run.versions.app` recorded 44.4.3, Electron's own version, not the app's 0.1.0:
        `app.getVersion()` finds no `package.json` beside `out/main/index.js`. Fixed with
        schema version 2 (the deviation below).
      - Chromium's basic GPU information gives no description, so the GPU appears in the file
        as its PCI IDs.
    - **Pending:** the visible run stays with the owner, its command in
      `docs/measurements/descent-spike/README.md` (decisions-r05.md item 7). Its presentation
      figures need the trace's remedy first.
  - **Re-taken by T14.f, 2026-10-06** (`2026-10-06-effect-low.md`): the hidden low run with the
    windowed protobuf trace, started at load 0.36. All nine windows were decoded and matched frame
    by frame, at most 23% of a window's buffer. The trace's figures are present, and 2.16% of the
    descent was left out at the boundaries. rAF p50 / p95 / p99 were 16.70 / 16.80 / 33.30 ms,
    terrain 0.68 ms, atmosphere 3.58 ms and GPU memory 0.256 GB. The record is the Risks bullet
    "R05.T14.f, the windowed hidden runs, as built". The trace's remedy is proven, so the visible
    run now waits only for the owner, with the check of its windows in the README.
  - Results schema version 2 (2026-10-04, delegated decision): v1's memory samples, an object
    per second, made the T14.c hidden run's file 563,230 B, over the 500 KiB hook. Version 2
    stores them as columns of whole KiB, which brings that run to about 105 kB and an hour's run
    to about 257 kB. The T14.c file is a lossless conversion of the hidden run's output.
    - _The form._ `memory.series` holds `tMs` and the columns `appKiB`, `gpuProcessKiB`,
      `tracingKiB`, `rendererPrivateKiB`, `drmResidentKiB`, `drmTotalKiB`, `nvidiaDeviceKiB` and
      `nvidiaGpuProcessKiB`. Each column is a figure like any other:
      - present: `{ "value": { "samples": [...], "gaps": [...] }, "reason": null }`, with a gap
        as `{ "from", "to", "reason" }`;
      - missing all run: `{ "value": null, "reason": "..." }`. Where samples gave different
        reasons, the column's reason lists each once, joined by "; ".
    - _Built by_ `memorySeries` (`main/results.ts`). The in-memory `MemorySample` and the
      sampler are unchanged. The peaks are their columns' maxima × 1024, which are exact, since
      every source reports whole KiB. The peak less `nvidia-smi`'s baseline is the device
      column's maximum less the baseline. The file does not hold the baseline, so
      `validateResults` checks only that this peak is at most that maximum.
    - _Checked by_ `validateResults`, which accepts version 2 only:
      - times in whole ms;
      - every column as long as the times;
      - readings in whole KiB;
      - -1 at a gap's samples and at no other;
      - gaps in order, within the samples, each with a reason, and consecutive samples of one
        reason in one gap;
      - a column with no reading written as null, with a reason;
      - each peak equal to its column's maximum × 1024, and null for a column with none.
    - _The size warning._ The client does not ship Prettier, so `formatAsPrettier` reproduces
      what Prettier changes in `JSON.stringify`'s indented form, which is what the writer writes:
      - an array of primitives that fits goes on one line;
      - an array of numbers that does not fit is packed to 100 columns;
      - an exponent's `+` is dropped.

      The tests hold its output byte-identical to Prettier 3.9.8's, with the repository's own
      configuration, on three files: a long run, a run with gaps and a run with no samples.
      `apps/hyperion` lists Prettier among its dev dependencies for them. `writeResults` warns with `console.warn`,
      naming the formatted size, when it is over 512,000 B (`ADDED_FILE_LIMIT_BYTES`), and still
      writes the whole file.

    - _Sizes as built,_ formatted at a print width of 100:
      - the T14.c file is 109,109 B, against the ruling's 105,329 B, since each column nests
        its samples in `value`;
      - the tests' 3,600-sample run is 250,626 B.
    - _The replayer._ T15.c's replayer (`tools/gpu-replay/src/results.rs`) writes version 2 too,
      approved by the orchestrator on 2026-10-04. Its series has no times, and every column is
      null with "the native replay does not measure memory". Its unit test checks the series,
      and its GPU test checks `RESULTS_VERSION`.
    - _The app's version._ `run.versions.app` is now `__APP_VERSION__`, `package.json`'s
      version, which the main process's build defines as the renderer's does
      (`electron.vite.config.mts`, with a test in `main/appVersion.test.ts`).
      `run.versions.electron` keeps Electron's. The client's `--version` had the same fault and
      reads `__APP_VERSION__` too. In the T14.c file the app's version is
      corrected to 0.1.0. The renderer bundle that run loaded, built at its start (12:31:31),
      carries `__APP_VERSION__` as "0.1.0". The summary's versions line is regenerated, and
      nothing else in the summary changed.
    - _The conversion_ was a one-off script, not committed. It built the series with
      `memorySeries` from the corrected file's samples, and checked the result against the
      source:
      - each of the 7,367 readings back from its column, exactly;
      - each missing reading's reason;
      - each time to within 0.5 ms.

      It kept the peaks as they were. `validateResults` confirms that they equal the columns'
      maxima × 1024. The only gap is the renderer's first sample, before the renderer reported.
- **Finding: Chromium's tracing service crashes writing the whole descent's trace** (2026-10-04,
  T14.c's hidden run; for T14 and T16, and the owner's visible T14.c and T17 runs).
  - _What happened._
    - When the trace stopped, the tracing service (`tracing.mojom.TracingService`, a utility
      process) held 1.71 GB. That is under `SPIKE_TRACE_BUFFER_KB`'s 2 GiB, so the buffer had
      not filled.
    - It took a SIGTRAP in `posix_memalign` at 12:52:48 (`coredumpctl info 427995`). That is
      Chromium's crash on a failed allocation. The machine had about 23 GB of memory available
      at 12:45, so the system was not short of memory.
    - The file it left held no events. The run itself ended normally and wrote its results.
  - _What it cost._
    - None of the trace's figures were measured: presentation times, the frames Chromium
      dropped, the GPU process's time, the main thread's split and GC pauses.
    - A visible run would lose its presentation intervals, the criterion's source, and fall
      back to rAF timestamps.
  - _Size._ The trace grew at about 1.4 MB a second. `SPIKE_TRACE_BUFFER_KB`'s comment assumed
    1.1, measured on a small page. Where inside Chromium the allocation failed is not confirmed;
    the JSON export of a buffer this size is the likeliest place.
  - _Likely remedies, to choose before the visible runs:_
    - fewer categories (`disabled-by-default-v8.cpu_profiler` and
      `disabled-by-default-devtools.timeline.frame` are the likeliest bulk, not yet measured);
    - a smaller buffer, which records until full and so truncates rather than crashes, if the
      export's size is the cause;
    - a shorter traced span (one trace a segment, or a window, each reduced as it ends).

    A short hidden run that stops its trace at a few sizes would find the threshold.

  - _Already fixed._ The results writer takes such an empty trace as none (T14.c's record
    above).
  - _Measured since_ (2026-10-04, the renderer's memory finding below):
    - The CPU profiler is about a fifth of the trace. With the profiler's category alone, the
      tracing service grew about 0.27 MB/s; with the other six categories, about 1.1 MB/s.
    - Without the profiler, two full hidden descents (G and H) still lost their traces. At about
      1.35 GB, each tracing service took a SIGTRAP at the stop (pids 619777 and 662220,
      `coredumpctl`). Dropping that category alone is no remedy.
  - _Ruled_ (2026-10-04, delegated decision `decision-r05-trace-windows.md`): the cause is the
    stop's read-out. The service's working set roughly doubled at the stop (G: 1.41 to 2.76 GB, H:
    1.35 to 2.55 GB, then SIGTRAP), and a stop frees it (E: back to about 70 MB after each of its
    60-s windows). The trace is therefore windowed. Windows are at most 120 s, with one unbroken
    window of at most 240 s over the approach, the low fast pass, the slowdown and the vertical
    descent. Each has a 768 MiB `record-until-full` buffer, and the files are reduced after the run.
    At the measured 1.15 MB/s without the profiler, a window holds about 138 MB, or about 276 MB for
    the long one, against the 1.35 GB that crashed. Even a full buffer's stop (about 1.75 GB) stays
    under the 2.55–2.76 GB at which G's and H's services were still alive. Windows cannot overlap,
    since Chromium runs one trace session at a time. The frames of each gap and of 1 s after it are
    left out and counted (T14.d, T14.e, T14.f).
  - _Re-ruled_ (2026-10-05, delegated decision `decision-r05-trace-windows-2.md`), on T14.f's
    partial runs. The stop's cost is Chromium's JSON export, which the tracing service builds whole
    in memory before streaming it (192 to 883 MB in about 24 s). That is also the cause of the
    crash, since JSON is 4.3–5.7 times the buffer's bytes. The export runs at 17.5–20 MB/s whatever
    the load, so the JSON windows' gaps were 16–44 s, and a descent would leave out 13–29% of its
    frames and cut the busy stretch. A Perfetto protobuf stream over CDP is about 1.2 times the
    buffer, read in 2.3–4.8 s a window with no growth of the service and no stall in the gap.
    Without `gpu` it projects to about 2.4% of a descent left out, all in the descent arc. The trace
    is therefore protobuf, decoded by the spike's own decoder (T14.h) and checked against the
    renderer's per-frame series. `gpu` is recorded only in profiled runs, and the last window runs
    from 950 s to the script's end.
- **Finding: the renderer's memory growth is the trace's CPU profiler, and the cache kept each
  bake's arrays** (2026-10-04, T14.c's hidden run; for T14's trace remedy and T16).
  - _The question._ In T14.c's hidden run, the renderer's private memory grew about 1.1 MB/s, to
    1.57 GB, while the GPU's stayed at 0.252 GB. The growth did not follow the bakes. It was
    1.08 MB/s through the descent arc, at 0.84 bakes/s, and −0.39 MB/s in the low fast pass, at
    41 bakes/s.
  - _The probe._
    - The runs were hidden low-setting runs, seed 7, on the RTX 3080, with no other spike run
      on the GPU. Each was capped at 200–300 s from the descent's start, unless marked full.
    - A temporary probe in the main process (not committed) sampled every 5 s:
      - the renderer's anonymous memory (`RssAnon`, the figure behind
        `getProcessMemoryInfo().private`);
      - each isolate's JS heap and ArrayBuffer backing stores, over the DevTools protocol
        (`Runtime.getHeapUsage`, for the page and each height worker);
      - the renderer's `/proc/<pid>/smaps` at 60 s and at the end, summed by mapping name.
    - Between runs, only the trace's categories changed, and in F the worker count.
  - _Rates from 60 s to the end, MB/s._ The isolates are the page and its height workers. "JS
    heaps" and "ArrayBuffers" are summed over the isolates.

    | Run | Trace                                         | Isolates |  Renderer | JS heaps | ArrayBuffers |
    | --- | --------------------------------------------- | -------: | --------: | -------: | -----------: |
    | A   | Design note 18's seven categories, as T14.c   |        4 |      1.30 |    0.016 |        0.022 |
    | B   | none                                          |        4 |      0.07 |    0.020 |        0.036 |
    | C   | all but `disabled-by-default-v8.cpu_profiler` |        4 |      0.04 |    0.007 |        0.042 |
    | D   | the CPU profiler alone                        |        4 |      1.27 |    0.016 |        0.038 |
    | E   | D's, stopped and restarted every 60 s         |        4 | 1.2, 0.07 |    0.019 |        0.052 |
    | F   | D's, with `--workers 1`                       |        2 |      0.61 |    0.023 |        0.010 |

    E's two renderer figures are its growth within one trace and across restarts (its troughs).

  - _Cause 1: V8's CPU profiler, which is the measurement's own._
    - The category starts V8's sampling profiler in every isolate of the renderer, so in the
      page and in each height worker. Each isolate keeps its profile in native memory until the
      trace stops.
    - D's smaps put 198 MiB of its 217 MiB growth (60 s to 240 s) in PartitionAlloc, Chromium's
      `malloc`. Only 11.5 MiB was in V8's sandbox, which holds the JS heaps and ArrayBuffers.
    - It costs about 0.31 MB/s for each isolate (D against F). An idle worker costs as much as
      the page, so the cost follows the worker count, not the descent's work.
    - Stopping the trace frees it. In E, each restart dropped the renderer back to its baseline
      (about 185–198 MiB), and its troughs rose at 0.07 MB/s, B's rate.
  - _Cause 2: the patch cache kept each bake's arrays. Ours, now fixed._
    - `PatchCache.insert` takes a `ResidentPatch`, but the terrain pass hands it the worker's
      whole `BakedPatch`, and the entry was a spread of it. So every resident patch kept its
      heights and normals in the renderer (50.7 KB a patch on the low layout), although its slot
      already held them.
    - This was bounded by the slots. In vitest, 4,000 such bakes into a low cache held 65.6 MB
      of ArrayBuffers once its 1,296 slots were full, and stayed at that plateau. On high, about
      1,960 slots of heights, offsets and double normals would hold about 396 MB, nearly the
      cache's whole 400 MB budget again.
    - `insert` now keeps only the resident fields (`cachedPatch`). The same drive holds about
      0 MB. Two regression tests in `cache.test.ts` fail on the old code, one for each insert
      path. typescript-reviewer found no must-fix or should-fix. Its wording note on `insert`'s
      remarks is applied. The hidden smoke (`descentSpike.sh --hidden --smoke --setting low`)
      passes on the fix.
    - _Over a whole descent._ G (before the fix) and H (after) were full hidden runs on
      `rendering-and-planets` with F3, tracing C's six categories:

      | Figure                                     | G, before |   H, after |
      | ------------------------------------------ | --------: | ---------: |
      | Renderer `RssAnon`, peak                   |   349 MiB |    305 MiB |
      | Renderer `RssAnon`, at the end             |   335 MiB |    255 MiB |
      | Results file's renderer private peak       |   370 MiB |    305 MiB |
      | The page's ArrayBuffers, peak              |    113 MB |      56 MB |
      | The page's ArrayBuffers, mean from 1,000 s |     88 MB |      24 MB |
      | Renderer growth, 60 s to the end           | 0.13 MB/s | 0.065 MB/s |

      In G, the page's ArrayBuffers grew with the bakes (25 MB at 65 s, 103 MB by 1,147 s). In
      H, they stayed at 18–23 MB, apart from one 56 MB sample at 1,207 s that the next GC
      returned.
  - _Ruled out, with the share each had._
    - The recorder's per-frame series and the 1 Hz samples are in the page's JS heap. All the
      JS heaps together grew 0.024 MB/s in H, about 30 MB over a descent.
    - Worker messages and height-worker results are ArrayBuffers. With the cache's copies
      gone, all the ArrayBuffers together grew 0.009 MB/s in H.
    - The height workers' ArrayBuffers (their wasm memories and bake buffers) went from 45 MB
      to between 52 and 62 MB, where they stayed from 850 s on.
    - Uploads' staging is shared memory, which `private` does not count. The renderer's
      `RssShmem` held at about 9 MiB throughout.
    - Event listeners and the cache's eviction (F3) do not show: the growth followed neither
      the bakes nor the cache.
    - What is left after the fix and without the profiler is 0.065 MB/s, about 80 MB over a
      descent: the JS heaps, the workers' ArrayBuffers, and about 0.03 MB/s of native memory.
  - _What it costs on T16's laptop._
    - The laptop runs two height workers by default (`defaultSpikeWorkers` for 8 threads), so
      three isolates. That is about 0.93 MB/s, or about 1.15 GB by the descent's end, on top of
      the renderer's own 0.3 GB.
    - It sits in the system memory the UHD 620 shares, beside the tracing service's 1.3–1.7 GB.
    - The criterion's memory row (DRM resident) does not count it. The results file's
      renderer-private figure does.
  - _Remedies for the profiler, to choose with the trace's remedy above (for the orchestrator
    and the owner):_
    - Windowed traces, the trace finding's third remedy. A stop frees the profile (E), so
      windows of 120 s or less bound it at about 110 MB with three isolates. They bound the
      tracing service too. One trace for each segment would not: the descent arc alone is
      900 s.
    - Drop the category. The renderer then grows at about 0.065 MB/s, and the results lose only
      the main thread's engine split (`engineSelfMs` and `sampledMs`, Design note 18).
    - Keep it, and read the renderer's figure as including it.

    _Ruled_ (2026-10-04, delegated decision `decision-r05-trace-windows.md`): both. The trace is
    windowed, as the crash needs, and the profiler leaves every timed run. Its cost scales with the
    worker count that T16.b's memory run, T17's worker comparison and T18 vary, and a variant must
    differ from its baseline in one factor. The engine adapter's share comes from a profiled run
    (`--trace-profile on`), which is diagnostic and never judged. Memory measured in an unprofiled
    windowed run needs no correction and no separate untraced run: the renderer grew 0.04 MB/s with
    the other categories against 0.07 MB/s with no trace, and the tracing service is reported apart.

- **Deviations in T14.d, as built** (2026-10-05, the trace's windows and results schema version 3).
  - _Where things are._ `main/traceWindows.ts` holds `mergeTraceWindows`, the window and boundary
    types (`TraceRun`, `TraceWindowRecord`, `TraceBoundary`), what the session hands it
    (`TraceRecording`, `TraceWindowFile`, `TraceSettings`), `traceSettingsOf(config)`, and the
    constants `TRACE_BOUNDARY_GUARD_S` (1), `CPU_PROFILER_CATEGORY`, `PROFILER_OFF_REASON`,
    `NO_CLOCK_OFFSET_REASON`, `SHORT_SPAN_FRACTION` (0.95) and `FULL_BUFFER_PERCENT` (99).
    `EMPTY_TRACE_REASON` moved there from `results.ts`. `Measured`, `measured` and `missing` moved
    to a new `main/measured.ts`, which `results.ts` re-exports, so that `results.ts` and
    `traceWindows.ts` do not import each other. `results.ts` no longer exports
    `SEGMENT_MEASURE_PREFIX`: presentations are assigned by script time, and the renderer's
    `metrics.ts` keeps its own copy for the marks.
  - _For T14.e._ The main process's `TRACE_BOUNDARY_GUARD_S` is in `main/traceWindows.ts`, since
    the main process cannot import the renderer's. T14.e's copy in `view/spike/traceWindows.ts`
    must agree with it, or the report must carry the guard. T14.e's `SPIKE_PROFILER_CATEGORY` is
    `CPU_PROFILER_CATEGORY`'s value: `profiled` is read from the categories recorded. Until T14.e
    drops that category from `SPIKE_TRACE_CATEGORIES`, every run records the profiler and so is a
    profiled run, named `-profiled` (no run is made between T14.d and T14.e). `bufferPercent` is
    null until T14.e's `bufferUsage()`. The session (`SpikeSessionDeps.traceSettings`, wired in
    `index.ts` as `traceSettingsOf(spikeTraceConfig())`) records one window from the start to the
    stop, reading its file's size (`SpikeFiles.size`) before reducing it.
  - _The clock offset_ is the median over the begins on the renderer's main thread alone, since
    a worker's `performance.now()` has its own origin; for an even count it is the mean of the
    middle two. The fixture's eight begins agree within 0.2 ms.
  - _Exclusions._ A frame is left out when its own script time lies from a boundary's
    `stopRequestedS` up to its `excludedToS`, as the warm-up leaves out a frame by its own time. A
    boundary's `excludedFrames` and `maxRafIntervalMs` count the frames after the warm-up only, so
    `frames.excludedFrames` is the boundaries' sum, which `validateResults` checks. Presentation
    intervals keep the ruling's rule (both ends after the warm-up and outside every exclusion), so
    the warm-up now drops the interval that starts in it too: v2 kept the first interval ending
    after it. `frames.dropped` counts only drops after the warm-up and outside the exclusions; v2
    counted the whole trace.
  - _A window's figures_ are null exactly when it failed, with its reason. `validateResults`
    refuses a window with figures whose span is under 95 % of its length or whose buffer use is
    99 % or more, and any trace figure present when a window failed. A window with no GPU process
    leaves `gpu.gpuProcess` null with "trace window k of n: the trace has no GPU process" but fails
    nothing; a GPU process whose ID changes between windows (a restart) is summed under the first
    one's IDs. When the renderer reports no window, or another number than the main process wrote,
    `run.trace` is null with the reason and nothing is left out.
  - _The summary_ heads a profiled run with the ruling's PROFILED line, and its Trace line gives
    the windows, the traced time, whether profiled, the boundaries' frames and largest stall, the
    largest file and buffer use, and each failed window. The frames line adds the excluded frames
    when there is a trace.
  - _Files beyond the task's list:_ `main/measured.ts`, `main/fixtures/traces.ts` (the tests'
    window traces), `main/index.ts` (the settings), `view/spike/SpikeApp.tsx` (the controller's
    `nowMs`, `performance.now()`), and `view/spike/metrics.ts` and `spikeHarness.ts`, whose
    reports are now `SpikeMetricsReport`, the report less `scriptStartMs` and `traceWindows`, which
    the controller adds. `SpikeRun`'s frame sample carries `scriptStartMs`.
  - _Sizes,_ formatted by Prettier: a 3,600-sample run with ten windows is 255,015 B, against
    250,800 B with no trace, so the windows add about 4.2 kB.
  - _The committed files._ `2026-10-04-effect-low.json`, `2026-10-05-effect-low.json` and
    `2026-10-05-effect-low-replay.json` were converted by a one-off script, not committed. Each is
    lossless: removing the new parts (`frames.excludedFrames` and each segment's, 0, and
    `mainThread.engine`, null with the trace's own reason) gives the v2 file back. The replay file
    was edited as text, keeping serde_json's number text (`14.0`) and its sorted keys. Their
    sizes are 109,428, 109,429 and 8,938 B. The summaries are unchanged: regenerated and
    formatted, they are byte-identical, so no `.md` changed. A test in `results.test.ts` now
    validates every committed results file as the current version and within 512,000 B, so the
    next version bump must convert them again.
  - The three subtasks' text in this plan keeps the ruling's suggested subjects, T14.f's included.

- **Deviations in T14.e, as built** (2026-10-05, the trace driven in windows).
  - _Where things are._ `view/spike/traceWindows.ts` holds `traceBoundaries`, the four constants
    and `TRACE_BUSY_SEGMENTS`. `main/spike.ts` holds `SPIKE_PROFILER_CATEGORY`, now the one copy
    of the literal (`main/traceWindows.ts`'s `CPU_PROFILER_CATEGORY` is gone), `SpikeTraceOptions`,
    `spikeTraceConfig({ profiled })` and `traceSettingsOf`, moved there from `traceWindows.ts` so
    that the two do not import each other. `SpikeTrace(tracing, { profiled })` exposes `settings`,
    the configuration every window records with, in place of `index.ts`'s second
    `spikeTraceConfig()` call. Its `state` is `idle`, `recording` or `busy`, and it refuses any
    call while another is in flight. The session refuses a cycle or a stop while it is busy, before
    any window is begun. `bufferUsage()` keeps the plan's name and returns a percentage, as its
    TSDoc says, where Electron gives a fraction. The session takes
    `traceDir` (`userData`) in place of `tracePath` and `traceSettings`; `traceWindowFileName(k)`
    counts from 0, as `run.trace.windows[].index` does. It logs one line per window: its bytes
    and buffer use, or its failure. `SpikeLaunch.traceProfile` carries `--trace-profile` to
    `run.options`. The results schema is unchanged (version 3).
  - _The guard._ The two tsconfig projects can neither import each other's constants nor hold them
    equal in a test, so the renderer's `TRACE_BOUNDARY_GUARD_S` is the one source. The report
    carries it as `traceGuardS`, and `mergeTraceWindows(recording, report)` reads it from there;
    the main process's `TRACE_BOUNDARY_GUARD_S` is gone. T14.f raises it in the renderer only.
  - _A trace that ends early._ "The remaining windows are failed" is recorded as one failed last
    window (`SpikeTraceWindow.failure`), from when the trace ended to the run's end, not one per
    planned window. Each planned boundary the trace never reached would otherwise leave 1 s of
    frames out with no stall in it. The boundary before the failed window excludes the whole
    failed or pending cycle. The main process has no file for that window when the start failed,
    and one when a late cycle started a recording that the last stop ended. `mergeTraceWindows`
    accepts one file fewer than windows when the last window is failed, and ignores a failed
    window's file. `readDescentSpikeReport` refuses a failure on any window but the last. A start
    still pending at the first boundary ends the trace in the same way. `tracedS` now counts only
    windows that did not fail. A failing last stop fails its window (its file is missing), not
    the run.
  - _The smoke traces._ T13.c's `--smoke` took no trace, so it could not show the cycle. A smoke
    now cycles at 3 and 6 s (`SMOKE_TRACE_BOUNDARIES_S`). That exercises the IPC,
    `getTraceBufferUsage`, the files, and their reduction and deletion. The session fails the
    smoke's stop when a window's file fails (`windowFileFailure`: missing, unreadable, empty, no
    clock offset, or buffer at 99 % or more), and the controller fails a smoke whose trace ended
    early. A smoke still writes no results. A `--smoke --capture` run traces too, and its span
    from 5 s crosses the cycle at 6 s, which the capture does not record.
    - The hidden `just descent-spike --smoke` on the committed tree (2026-10-05, the high
      setting, RTX 3080, load about 18) exited 0 after 30 s. Its windows were 18,457,209 B
      (0.31 % of the buffer), 11,303,047 B (0.18 %) and 22,698,503 B (0.50 %), each reduced and
      deleted, and the profile under `target/descent-spike/` was removed. A run before the
      review's fixes gave 20,826,061, 12,255,570 and 22,194,612 B at 0.35, 0.19 and 0.49 %.
  - _The buffer's units._ Electron's `getTraceBufferUsage().percentage` is Chromium's
    `percent_full`, a fraction from 0 to 1 (CDP's `Tracing.bufferUsage.percentFull`), so
    `bufferUsage()` scales it by 100. The smoke agrees with that: 0.50 % of 805 MB is about
    4.0 MB in about 4 s, near the ruling's 1.15 MB/s. Read as a percentage, the buffer would have
    held 100 times that.
  - _Finding for T14.f: the window files are 5.6 to 7.8 times the buffer's use._ The ruling sized
    the files deferred on disk at about 1.5 GB a descent, from the buffer's rate. If a descent's
    windows keep the smoke's ratio, a 120 s window's JSON file is about 0.8 to 1.1 GB, and a run's
    files total about 10 GB on disk until the reduction. That is in `target/descent-spike/`, with
    310 GB free here. The smoke was the high setting's start, not a descent window. T14.f records
    each window's bytes, and the reduction's time after the run, which is unmeasured and counts
    against the 45-minute watchdog. Free disk is checked before each run, and for the owner's
    laptop.
  - _Files beyond the task's list:_
    - `main/traceWindows.ts` and its test: the guard, the early end, `windowFileFailure` and
      `tracedS`;
    - `main/spikeReport.ts` and its test, `main/results.test.ts` and `main/fixtures/spikeReport.ts`:
      `failure` and `traceGuardS`;
    - `preload/spikeLaunch.ts` and its test: `traceProfile`;
    - `view/spike/SpikeApp.test.tsx`: `stubHyperionApi.ts`'s new `TEST_SPIKE_LAUNCH`, which the
      controller's test shares.

- **Deviations in T14.g, as built** (2026-10-05, the frame span, the categories and results
  version 4; `decision-r05-trace-windows-2.md`).
  - _Where things are._ The renderer's `FRAME_MEASURE` is in `view/spike/spikeRun.ts`, the main
    process's in `main/reduceTrace.ts`, each pinned to `"spike.frame"` by its own side's test.
    `MainThreadFigures.frameSpans` (a `SpanList`, which `UserTimingFigures` now extends) keeps the
    spans for the check, and `MainThreadSplit` leaves it out of the results. `GPU_PROCESS_SLICES`
    is a list of `{ name, category }`, and `recordedGpuSlices(categories)` gives the names a run
    records. `ReduceOptions.categories` is required: the session hands `reduce(path, categories)`
    its trace's `settings.categories`, and `index.ts` wires `reduceTraceFile(path, { categories })`.
    `main/spike.ts` adds `SPIKE_GPU_CATEGORY` and `SPIKE_TRACE_FORMAT` (`"json"` until T14.i), and
    `traceSettingsOf(config, format)` takes the format. `main/traceWindows.ts` holds `TraceFormat`,
    `TraceSettings.format`, `frameSpanFailure` and its constants `FRAME_CHECK_MARGIN_MS` (500),
    `FRAME_SPAN_EARLY_MS` (0.2) and `FRAME_SPAN_TOLERANCE_MS` (0.01). The merged GPU-process slices
    are listed from the recorded categories, not from the first window's list.
  - _The check's stretches._ Each frame's spans are those starting from its rAF time less 0.2 ms to
    the next frame's rAF time less 0.2 ms, within the ruling's "[rAF time − 0.2 ms, the next frame's
    rAF time)". With the ruling's bounds, consecutive stretches overlap by 0.2 ms. A frame whose
    callback began within 0.2 ms of its own rAF time (less the offset's spread) would then count
    for the frame before it too, which would hold two spans and fail the window. A span moved to
    0.3 ms before its frame's rAF time therefore falls in the previous frame's stretch, and its test
    counts two frames that disagree. The series' last frame is not checked: it has no next frame,
    and the page draws on after the run ends, so the last window holds spans of frames the report
    does not have. Its script time is held at the script's end, so the stretch of the frame before
    it ends at that frame's rAF time plus the last rAF interval. The check runs after the span and
    buffer checks and before the renderer check, so a window's reason is the first that applies.
  - _The boundaries._ `traceBoundaries` places no boundary after the busy stretch's start less the
    clearance. It throws up front when that window would exceed 300 s, and when a change's
    clearance moves the last boundary so early that the last window would. Without a busy stretch,
    every window is at most 120 s, as before. The controller's test now expects eight cycles.
  - _Ahead of T14.i._ Design note 18 and the README's trace bullet, as the ruling words them,
    describe the protobuf trace over CDP, which T14.h and T14.i build. Until then the spike writes
    JSON through `contentTracing` (`format: "json"`), now without `gpu`, and no recorded run is
    made. Profiled runs keep the 768 MiB buffer until T14.i, whose task sets 1.5 GiB.
  - _Results version 4._ `validateResults` refuses a missing or unknown `format`, a GPU-process
    slice whose category was not recorded, a recorded name that is absent, and a name the reducer
    does not summarise. The summary's Trace line names the format ("2 json windows"). The three
    committed files changed in their version number alone (109,428, 109,429 and 8,938 B, as
    before), and their `.md` files did not change. The replayer's test also pins its null split and
    GPU process, the fields version 4 changes.
  - _The smoke._ The hidden `just descent-spike --smoke` on the change (2026-10-05, the high
    setting, RTX 3080, load about 14, port 7893, not locked) exited 0 after 27 s. Its windows were
    8,332,428 B (0.18 % of the buffer), 8,152,138 B (0.22 %) and 10,510,229 B (0.30 %), against
    T14.e's 18.5, 11.3 and 22.7 MB with `gpu`. On the final tree, after the review's fixes (load
    about 7), it exited 0 after 21 s, with 10,936,865 B (0.25 %), 8,286,071 B (0.24 %) and
    4,636,633 B (0.14 %). A smoke writes no results and hands the main process no report, so the
    frame-span check did not run on it. T14.i's acceptance asks its smoke to pass the check, which
    needs the smoke's frames and window times handed to the session: an open point for T14.i.
  - _For T14.h: the duration's tolerance._ The renderer's `performance.now()` is coarsened to
    0.1 ms: every `ourCodeMs` of T14.f's P3 report is a multiple of 0.1 ms. In T14.d's JSON fixture
    the `spike.frame` begins' `ts − 1000 × startTime` spread over 138 µs, and each end's `ts` is
    its `callTime`, the call's unclamped time. That page may have left the measure's end to "now".
    But if Chromium writes unclamped times for a numeric start and end too, the trace's durations
    differ from `callbackMs` by up to about 0.1–0.2 ms, and every window fails the 0.01 ms check.
    T14.h's fixture settles this. If they differ, the tolerance is to be re-ruled, for example to
    two clock quanta (0.2 ms).
  - _For T14.f: a late callback._ Under load, a frame's callback can begin after the next frame's
    rAF time, when Chromium dates the next frame by a vsync that passed while this one waited. Its
    span then lies outside its stretch and fails the window. T14.f's runs show whether this occurs.
    If it does, pairing spans with frames in order is one remedy, to be ruled.
  - _Files beyond the task's list:_ `main/index.ts` (the reducer's categories),
    `view/spike/spikeController.test.ts` (eight cycles, and the new error's text).
  - _Review._ typescript-reviewer found no must-fix. Its should-fix, that the series' last frame
    could collect the spans drawn after the run, and its three considers are applied. rust-reviewer
    found nothing to fix, and its two considers are applied.

- **Deviations in T14.h, as built** (2026-10-05, the protobuf trace's decoder;
  `decision-r05-trace-windows-2.md` and its addendum A).
  - _Where things are._ `main/traceProto.ts` holds `ProtoTraceDecoder` (one packet at a time,
    `DecodedPacket` with its sequence, its time on the trace's clock and its events),
    `tracePackets`, `decodeProtoPackets`, `decodeProtoTrace`, `readProtoTraceBatches` (one array of
    events a read, 4 MiB by default, `DEFAULT_PROTO_READ_BYTES`), `readProtoTraceEvents` and
    `trimProtoTrace`. Its field numbers are Perfetto's at
    `9c7ce42050379be8c24a270f395e00c7347965cb`, the revision Chromium 152.0.7977.130's `DEPS` pins
    (`protos/perfetto/trace/…` and `protos/third_party/chromium/chrome_track_event.proto`,
    `chrome_enums.proto`).
    `main/reduceTrace.ts` adds `readTraceFileEvents`, which reads a file by its first byte (`{` or
    `[` JSON, `0x0a` protobuf, none for an empty file, and any other byte fails naming the file),
    and `reduceTraceFile` reduces a protobuf trace a read at a time, since one asynchronous step an
    event cost more than reducing it (2.4 s against 1.0 s for the timed window).
    `MainThreadFigures.frameSpans` is a `FrameSpanList`, whose `startTimesMs` keeps each begin's
    `args.startTime`. `main/traceWindows.ts` has `FRAME_SPAN_IDENTITY_MS` (10⁻⁶),
    `FRAME_SPAN_TOLERANCE_MS` (0.25), `FrameSpanReport`, and
    `frameSpanFailure(trace, time, report)`, which takes no clock offset. A window's reason adds
    "; k spans match no frame" for strays, and a span without a `startTime` is one. The
    renderer's `SpikeFrameSample`, `ControllerFrame` and `FrameSample` carry `callbackStartMs`, and
    the report's `frames.callbackStartsMs`, which `readDescentSpikeReport` requires as long as the
    frames.
  - _What Chromium 152 writes._ Every slice is typed, with no `legacy_event`, except V8's `Profile`
    and `ProfileChunk` samples (phase `P`, unscoped IDs, `track_uuid` 0, `data` as a legacy JSON
    value) and some `C` counters. Each sequence's timestamps are on its incremental µs clock (64)
    and its absolute twin (65), rebased by its snapshots against `MONOTONIC`, the primary clock.
    The events written after the fact carry explicit `MONOTONIC` timestamps: the user-timing spans
    and the `PipelineReporter`s. Each `performance.measure` span has its own track, a child of the
    main thread's track. Each `PipelineReporter` has its own track, a child of the renderer's
    process track, with its state in `ChromeTrackEvent.frame_reporter` (extension 1075). The
    renderer's thread IDs are its sandbox's (1, 2, …). Busy sequences clear their incremental
    state every 0.5 s.
  - _Slices on other tracks than a thread's_ become async begins and ends, as `trace_processor`'s
    JSON conversion writes them. So the GC states V8 writes on child tracks of a thread
    (`Scavenge`, `Marking`, `ObservablePause`) are not GC pauses: only thread-track slices are.
  - _Addendum A's stop conditions did not fire._ On the untrimmed windows, the timed run's 1,753
    frames have 1,752 spans by identity (its first callback ran before the trace began) and the
    profiled run's 1,619 have 1,618, with no span left over. The largest |duration − `ourCodeMs`|
    is 0.0010 ms in both. **Chromium writes the endpoints as the values passed** (clamped to
    0.1 ms), within 0.0010 ms, not unclamped. The fixture's test pins both.
  - _The fixtures' cuts._ The window's first 0.3 s are the page's start-up: GC, slow frames, and
    reporters written after the cut. The ruling's tests fail there. A steady-state 0.5 s is about
    410 KB timed and 845 KB profiled. So `trimProtoTrace(bytes, untilUs, fromUs?)` gains an
    optional start. Each sequence is then kept from its first packet at or after it that clears
    its state, which re-sends its interning, clocks and tracks.
    - `spike.pftrace` is the timed window from 20.0 to 20.5 s after its first event: 388,210 B,
      27 frames, two `MinorGC` and one `MajorGC` on the main thread.
    - `spike-profiled.pftrace` is the profiled window's first 0.21 s: 357,362 B, a prefix, since
      only a window's start holds the CPU profiles' `Profile` events. 0.21 s is what 400 KB holds.
    - The windows were 24,584,692 B (2.48 % of the buffer, no data loss) and 50,582,974 B
      (5.19 %), from two hidden partial runs of 43 s and 49 s under an 8 GB cap, with no new
      coredump.
  - _Tests reworded to what the fixtures can show._
    - "Inside a `RunTask` within 1 µs": the end within 1 µs, but the start within 0.25 ms. The
      start the page passed is clamped, so it falls up to 127 µs before its task (p99 102 µs) in
      the timed window.
    - "`sampledMs` within 10 % of the fixture's main-thread span": of the span the main thread was
      sampled over, from the profile's start plus its first delta to its last chunk. V8 took the
      first sample 95 ms after the profile's start, itself 90 ms into the window, so the samples
      cover only the fixture's last 24 ms.
    - The cross-sequence check covers the spans at least 100 ms before the fixture's end, since a
      frame's reporter is written at its presentation.
    - The GC slices are complete with durations ≥ 0, since V8's phases are timed in whole µs (4 of
      372 on the renderer are 0 µs long). The pauses and `GPUTask`s are positive.
    - The trim test compares with `decodeProtoPackets`' packets cut per sequence.
    - Hand-made traces test the start cut, skipped fields, a known name in an unknown encoding,
      and a packet that is not one.
  - _The oracle_ (`trace_processor_shell` v58.2, development only), on both untrimmed windows.
    - Per process and thread, the counts and summed durations of `RunTask`, `GPUTask`, `MinorGC`,
      `MajorGC`, `WebGPU` and `VulkanQueueSubmitHook` equal the decoder's to the nanosecond, in
      all 23 and 25 groups.
    - The `PipelineReporter` counts are equal (3,549 and 3,273). But 2,353 of the timed window's
      reporters are 1–11 µs longer in it: it sorts a track's ends by time, so it ends a reporter at
      a child `Swap`'s end written 1 µs after the reporter's own. The decoder pairs ends in the
      writer's order, as the ruling's stack per track does.
  - _Speed and memory_ (provisional, under other lanes' load).
    - At a load of 4–12, the timed window (24.6 MB) frames in 0.15 s, decodes in 0.75 s and reduces
      in 1.0 s at a peak of 315 MB, about 25 MB/s.
    - At a load of 28, it reduces in 1.9 s, and the profiled window (50.6 MB) in 3.8 s, both at
      about 310 MB, so 13 MB/s.
    - That projects to 0.6–1.2 min for a timed run's windows (about 0.9 GB) and 1.3–2.6 min for a
      profiled run's (about 2 GB), inside T14.f's 5 min. The decoder holds one read and its
      events at a time; the reducer's intervals grow with the window, as with JSON.
  - _Files beyond the task's list:_ `view/spike/spikeController.ts` (`ControllerFrame`), and the
    tests of the harness, the controller, the results and the report.
  - _Beyond the ruling's decoder._ A packet marked `previous_packet_dropped` other than its
    sequence's first fails the file, since a lost end would pair its begin with another slice's.
    In both windows the mark is on each sequence's first packet alone. A framing error other than
    a short read fails at once, rather than waiting for bytes that cannot mend it.
  - _A crash after the recording._ The session crashed and the machine rebooted after both runs
    had finished (exit 0). The fixtures, cut again from the kept windows with the final decoder,
    are byte-identical, and the identity and oracle figures above are those of the final decoder.
  - _Review._ typescript-reviewer found no must-fix. Its five should-fix are applied:
    - `signed()` decodes negative `int64`s exactly (−1 had come out 0);
    - a framing error other than a short read fails at once;
    - the dispatch tests remove every temporary directory;
    - the fixture tests with several claims are split;
    - the exported decoders document their `@throws`.

    Its three considers are applied too: the lost-packet failure, a test pinning the decoder's
    copy of the reducer's names to `GPU_PROCESS_SLICES`, and a hand-made trace read a byte at a
    time.

- **Deviations in T14.i, as built** (2026-10-06, the trace over CDP; `decision-r05-trace-windows-2.md`,
  rulings 1, 5 and 6, and addendum A item 4).
  - _Where things are._ `main/cdpTracing.ts` holds `CdpTracing`; `TraceDebugger`, the part of
    Electron's `Debugger` it uses, which Electron's satisfies; `CDP_TRACE_COMMANDS`, the allowlist,
    which `#send` takes as a type (`CdpTraceCommand`); `cdpStartParams(config)`;
    `CDP_PROTOCOL_VERSION` ("1.3"); `CDP_READ_BYTES` (8 MiB); and `CDP_BUFFER_REPORT_MS` (1000).
    `main/spike.ts`'s `SpikeTracing` is now the spike's own interface (`start(config)`,
    `stop(path)` returning `TraceWindowStop { lostData, bufferPercent }`, and `close()`), not a
    `Pick` of `contentTracing`. `spike.ts` also has `SPIKE_PROFILED_TRACE_BUFFER_KB` (1,572,864) and
    `SPIKE_TRACE_FORMAT` (`"perfetto-proto"`, the ruling's and version 4's name; the brief's
    "proto"). `main/traceWindows.ts` adds `TraceWindowFile.lostData`, `LOST_DATA_REASON`,
    `checkFrameSpans` (the counts `frameSpanFailure` now reads) and `MergedTrace.checkedFrames`,
    which is for the log and not the results. The tests share a fake debugger,
    `main/fixtures/debugger.ts`.
  - _The buffer's reading._ `SpikeTrace.bufferUsage()` is gone. Each stop returns the window's
    last `percentFull` × 100. It is reset at each start, so a window with no report has `null`,
    never the previous window's reading.
  - _A cycle's stop._ `SpikeTrace.cycle(path, stopped)` hands the stop's outcome to `stopped`
    before it starts again. A window whose next start fails is then still known to be whole. A
    callback that throws leaves the trace idle.
  - _Reasons._ `SpikeTrace`'s errors carry their cause's message ("the spike's trace did not stop:
    the trace's debugger session detached: …"), since IPC gives the renderer the message alone. A
    window whose stop failed fails with that reason, and its file is not read, since it may be
    partial. T14.e read it and failed the window on ENOENT. The session holds each window's stop as
    `pending`, `stopped` or `failed`.
  - _Data loss_ is checked after the span and the buffer, so that a buffer that filled says so,
    and before the frame spans. Its reason is "it lost data (Chromium's dataLossOccurred)".
  - _The detach._ Electron's own `detach()` emits `detach` too, so the session is marked closed
    first. An unexpected detach is logged ("descent spike: the trace's debugger session detached:
    <reason>"), and it fails a stop that is waiting for `tracingComplete`. Every later command and
    start is refused with its reason, so the next cycle fails and the renderer ends the trace. An
    attach that fails because another debugger holds the page fails the first start. That ends the
    run, as any refused start does.
  - _Closing._ The session closes the transport in `#stop`'s `finally`, after the last stop or after
    a failed cycle has ended the trace. A session therefore measures once: a second start is
    refused, and nothing asks for one. Closing fails a stop still waiting for the trace's end.
    A failed `IO.close` is logged and does not fail the window. A failed read does fail it, with
    the file and the stream closed, and the read's error is kept over the file's.
  - _The smoke's results call._ `SpikeApi.writeResults` now answers a union
    (`SpikeResultsAnswer`): `{ kind: "written", paths }` for a run, and
    `{ kind: "smoke checked", failure }` for a smoke. The preload accepts each only on its own kind
    of launch. The controller ends a smoke with the answer's failure first, then with the trace's
    early end, then with the baked check. An answer, not a refused call, keeps Electron's "Error
    invoking remote method …" prefix out of the FAIL line (typescript-reviewer). A smoke's
    `stopTrace` no longer fails on a window, as T14.e's did; the merge checks it. Files beyond the
    list: `preload/api.ts`, `preload/spikeApi.ts` and its test, `view/spike/SpikeApp.test.tsx`,
    `main/results.test.ts`, and `main/fixtures/traces.ts`, whose settings are now protobuf with the
    profiled 1.5 GiB.
  - _The log._ Each stop logs "trace window k stopped: complete after X ms, B B read in Y ms",
    with ", data lost" when Chromium lost data. Each window logs its line as it is reduced, as
    before. A smoke, whose log is its only record, also logs each window's "the spans of m frames
    match the renderer's" and each boundary's "X ms from the stop to the next start".
  - _The spool._ `descentSpike.sh` gives Electron a per-run `TMPDIR`,
    `target/descent-spike/tmp.XXXXXX`, rather than the fixed `tmp`. Two runs of one checkout then
    cannot remove each other's. Chromium opens its spool of each window's stream there with
    `DELETE_ON_CLOSE`, which on POSIX unlinks the file at once, so the directory looks empty. The
    browser's open files show it instead. In the final smoke the browser held one
    `.org.chromium.Chromium.XXXXXX (deleted)` under the run's `TMPDIR` for each window, and none
    elsewhere. Only Linux's `/tmp` may be RAM-backed: the temporary directories of macOS and
    Windows are on disk, and Chromium reads `TMP` and `TEMP` on Windows.
  - _The hidden smoke_ (`just descent-spike --smoke`, 2026-10-06; the high setting, RTX 3080, port
    7893, an 8 GB scope with a no-relaunch guard, not locked, under other lanes' load of 18–91).
    It passed three times. Each run had three protobuf windows, each decoded, matched frame by
    frame, and deleted, with no new coredump (40 before and after).

    | Smoke                      | Exit, time | Window bytes (buffer %)                              | Stops: complete / read (ms) | Frames checked | Gaps (ms)  |
    | -------------------------- | ---------- | ---------------------------------------------------- | --------------------------- | -------------- | ---------- |
    | 1 (load 34–38)             | 0, 33 s    | 2,509,601 (0.11), 2,512,004 (0.13), 1,783,845 (0.13) | 110/44, 101/47, 237/48      | 86, 97, 61     | not logged |
    | 2 (load 34–91)             | 0, 52 s    | 2,222,318 (0.09), 2,087,971 (0.14), 2,536,603 (0.15) | 109/71, 129/43, 107/38      | 74, 72, 109    | not logged |
    | 3, final tree (load 18–23) | 0, 23 s    | 2,369,136 (0.22), 2,952,750 (0.19), 3,648,353 (0.27) | 88/53, 84/42, 61/41         | 61, 111, 172   | 156, 140   |

    The windows are about a quarter of T14.g's JSON windows (8–11 MB). The guard tripped once, in
    smoke 2, on its own fd sampler's last `sleep`, which outlived the run; nothing ran on the spike's
    profile, and the sampler now waits for it.

  - _Review._ typescript-reviewer found no must-fix. Its four should-fix are applied: the smoke's
    answer as a result union; the fake trace refusing a start after `close()`, as the transport
    does; the window's stop as a union; and a reason above an `oxlint-disable`. Its three
    considers are applied too: `close()` fails a pending stop, a throwing `stopped` leaves the trace
    idle, and the buffer's fraction is named a fraction. So is its note that a read's error should
    survive a failing file close. A stream named by a `tracingComplete` whose `Tracing.end` then
    fails is left open. The detach at the end and the removed `TMPDIR` make that harmless.
  - _Orchestrator rulings on T14.h's open questions_ (2026-10-06):
    1. T14.h's fixture cuts stand: a timed cut from the middle of the window, a profiled prefix,
       and `trimProtoTrace`'s optional start. So do its reworded tests: the start within 0.25 ms
       inside a `RunTask`, `sampledMs` against the sampled span, and the cross-sequence check on
       the spans at least 100 ms before the end.
    2. The decoder's `PipelineReporter` durations may differ from `trace_processor`'s by 1–11 µs,
       since it pairs ends in write order, as the ruling's per-track stack prescribes. That is
       accepted: microseconds against 16–33 ms frames. The ruling's "must equal" is read as equal
       counts and summed durations within 0.1%. T14.h's sums differ by 2.52 ms of 102.88 s
       (0.0024%) timed and 2.46 ms of 109.46 s (0.0022%) profiled.
    3. A packet marked as following lost packets fails its window, as `dataLossOccurred` does.
       T14.h built it so: the decoder fails the file ("the trace lost packets of sequence N before
       this one").

- **R05.T14.f, the windowed hidden runs, as built** (2026-10-06; `decision-r05-trace-windows-2.md`,
  ruling 5d and its T14.f; lane D). All runs hidden on the RTX 3080 (driver 615.71.09, Electron
  44.4.3, Chromium 152.0.7977.130, Linux `vulkan` mode, timer `full`), seed 7, port 7893. The
  orchestrator held other heavy work back for the window.
  - _Conditions._ Each run took `just _locked` for itself alone, with its own 8 GB scope
    (`--unit=t14f-<run>`, `TasksMax` 4096). It started only when a read-only gate passed, both
    before the lock and again inside it:
    - the 1-min load at most 4 (A/B) or 8 (full runs);
    - no `cargo`, `cargo-nextest`, `rustc` or `descentDemand.mjs`;
    - no other spike, views-check or `gpu-replay` run;
    - at the orchestrator's request, no Electron or Chrome process but the run's own;
    - at least 6 GB available.
      A no-relaunch guard followed every run. The scripts are uncommitted, in
      `target/laneD/t14f2/`: `gate.sh`, `go.sh`, `locked.sh`, `run.sh`, the 0.5 s sampler
      `sample.sh`, and the analyses `ab.py` and `full.py`. Every run's log, samples and report dump
      are kept there too.
  - _The instrument_ (uncommitted, `target/laneD/t14f2/ab.patch`):
    - `HYPERION_T14F_AB=detached|attached` swaps `CdpTracing` for a stand-in, `T14fAbTracing`. It
      never sends `Tracing.start`, and its stops write empty window files, so every trace figure
      is null.
    - In the attached arm it attaches the spike window's own `webContents.debugger` ("1.3") at the
      first window's start, with `CdpTracing`'s listeners, and detaches it after the last stop. Each
      attached run logged the attach, then the detach "after 0 protocol messages".
    - The controller's script ends at `Math.min(durationS, 255)`.
    - `HYPERION_T14F_REPORT=<file>` dumps the renderer's report at the results call.
      The full runs carry the dump alone (`full.patch`), which acts only after the run. It gives
      the guard's statistic, which needs every frame's rAF interval.
  - **The debugger's A/B: the idle session costs nothing.** Six untraced 255 s low runs, in the
    order detached, attached, detached, attached, detached, attached. Each figure is taken over
    script time 10–255 s; the growth is the least-squares slope of the renderer's private memory
    from 60 s, with 1 MB = 10⁶ B.

    | Run | Arm      | Load at start | Load during | Our code p50 / p95 (ms) | rAF p95 / p99 (ms) | Growth (MB/s) |
    | --- | -------- | ------------: | ----------- | ----------------------- | ------------------ | ------------: |
    | 1   | detached |          1.21 | 1.18–2.55   | 1.50 / 1.90             | 16.80 / 33.30      |        0.0526 |
    | 2   | attached |          0.87 | 0.87–2.18   | 1.50 / 1.80             | 16.80 / 33.30      |        0.0484 |
    | 3   | detached |          0.62 | 0.62–2.36   | 1.50 / 1.80             | 16.80 / 33.30      |        0.0474 |
    | 4   | attached |          0.65 | 0.65–2.86   | 1.50 / 1.90             | 16.80 / 33.30      |        0.0541 |
    | 5   | detached |          0.70 | 0.70–1.18   | 1.50 / 1.90             | 16.80 / 33.30      |        0.0467 |
    | 6   | attached |          0.36 | 0.36–2.40   | 1.50 / 1.80             | 16.80 / 33.30      |        0.0482 |

    | Figure            | Detached range | Allowed (± 2% of the median) | Attached median | Verdict |
    | ----------------- | -------------- | ---------------------------- | --------------: | ------- |
    | our code p50 (ms) | 1.50–1.50      | 1.47–1.53                    |            1.50 | within  |
    | our code p95 (ms) | 1.80–1.90      | 1.76–1.94                    |            1.80 | within  |
    | rAF p95 (ms)      | 16.80–16.80    | 16.46–17.14                  |           16.80 | within  |
    | rAF p99 (ms)      | 33.30–33.30    | 32.63–33.97                  |           33.30 | within  |
    | growth (MB/s)     | 0.0467–0.0526  | 0.0458–0.0535                |          0.0484 | within  |
    - Our code's times are on `performance.now()`'s 0.1 ms grid, and the rAF intervals are
      multiples of the vsync. Their percentiles are therefore coarse, and identical runs give
      identical values. The growth is the continuous figure. By endpoints the attached median is
      0.0425 MB/s against a detached range of 0.0299–0.0661 MB/s, also within.
    - _Run 1 was repeated._ Its first try started at load 1.85, but another lane's pre-commit
      hook compiled `hyperion-sim` through most of it (load 1.85–15.63, mean 8.94). That try gave
      our code 1.70 / 5.20 ms, rAF 16.80 / 33.50 ms and growth 0.0570 MB/s. It is kept as
      `ab1-d-loaded` and left out of the verdict, since the A/B is defined at low load. Its p95 is
      2.7 times the quiet runs', which shows how much load moves our code's tail.

  - **The three full runs: every check holds.** Each run exited 0, with every window decoded,
    matched frame by frame and written with no data loss. `coredumpctl` listed 40 dumps before and
    after each run. All three files validate as version 4 and are 117,718 / 118,703 / 119,073 B
    once formatted (high / low / low-profiled). Each started below a load of 1, so none is
    provisional; their verdicts are `not-measured`, being hidden.

    | Figure                              | Low (`--setting low --hidden`) | High (`--setting high --hidden`) | Profiled (`low --trace-profile on`) |
    | ----------------------------------- | ------------------------------ | -------------------------------- | ----------------------------------- |
    | Load at start (1/5/15), during      | 0.36 1.11 1.80; 0.36–3.16      | 0.18 1.30 1.64; 0.18–5.14        | 0.03 1.41 2.15; 0.03–4.34           |
    | Exit, time                          | 0, 1,282 s                     | 0, 1,278 s                       | 0, 1,325 s                          |
    | Windows 1–8: bytes                  | 84.4–94.3 MB                   | 66.4–81.3 MB                     | 183.7–208.9 MB                      |
    | Window 9 (950 s to the end): bytes  | 226.1 MB                       | 235.0 MB                         | 503.8 MB                            |
    | Buffer use, windows 1–8 / 9         | 8.57–9.61% / 23.20%            | 6.86–8.35% / 24.34%              | 9.49–10.79% / 26.34%                |
    | Stops 1–8: complete / read (ms)     | 1,374–1,565 / 714–837          | 973–1,262 / 467–602              | 3,337–3,878 / 1,492–1,752           |
    | Last stop: complete / read (ms)     | 4,465 / 1,726                  | 5,013 / 1,971                    | 14,430 / 3,857                      |
    | Frames checked, windows 1–8 / 9     | 6,362–7,061 / 16,344           | 4,118–5,400 / 16,045             | 6,175–7,038 / 16,104                |
    | Tracing service: peak / working set | 250 / 250 MB                   | 259 / 258 MB                     | 494 / 492 MB                        |
    | Gaps                                | 2.13–2.34 s                    | 1.44–1.79 s                      | 4.90–5.60 s                         |
    | Excluded frames a boundary          | 185–199                        | 29–162                           | 349–388                             |
    | Largest rAF in a gap                | 16.8–33.4 ms                   | 16.8–183.4 ms                    | 50.0–66.8 ms                        |
    | Left out: descent / worst segment   | 2.16% / 2.92% (descent arc)    | 1.36% / 2.03% (descent arc)      | 4.19% / 5.66% (recorded only)       |
    | Last exclusion's end / margin       | 953.14 s / 6.86 s              | 952.46 s / 7.54 s                | 955.92 s / 4.08 s                   |
    | Renderer growth from 60 s           | 0.082 MB/s (ends 0.076)        | 0.092 MB/s (ends 0.086)          | 0.172 MB/s (profiler; not judged)   |
    | Decoding / main process's peak      | 34.4 s / 1.00 GB               | 30.3 s / 1.42 GB                 | 66.2 s / 1.13 GB                    |
    | Disk peak: profile + spool          | 0.98 + 0.23 = 1.19 GB          | 0.85 + 0.24 = 1.00 GB            | 2.12 + 0.50 = 2.61 GB               |
    | Callbacks after the next rAF time   | 1 of 72,966                    | 4 of 55,119                      | 15 of 72,793                        |
    - _The budget_ is met in both timed runs: at most 5% of the descent and of each segment, and
      no frame excluded in the approach or any later segment. Every excluded frame is in the
      descent arc. The last boundary's exclusion ends 6.86 s (low) and 7.54 s (high) before the
      approach at 960 s. The ruling's "none from 950 s" could not be met as worded, since its
      own last boundary is at 950 s and that boundary's exclusion lies after it. The orchestrator
      ruled it met in intent (2026-10-06, a clarification under delegation): the clause protects
      the busy stretch from the approach on. Design note 18, T14.f and the README now say so.
    - _The profiled run's exclusions_ are recorded, not budgeted: 4.19% of the descent and 5.66%
      of the descent arc, from gaps of 4.9–5.6 s. Its windows are about 2.2 times the timed ones,
      from the `gpu` category and the CPU profiler.
    - _The service does not grow at a stop._ Within a stop it grows by at most 5 MB, and its peak
      is at most 2 MB above its largest working set, against the 100 MB allowed. That working set
      is the last window's buffer.
    - _The 950 s window_ is the largest, at 23–26% of its buffer. Its stop's time grows with its
      bytes: 35–51 MB/s to complete and 119–131 MB/s to read. No window approaches half its
      buffer.
    - _The gaps' stalls._ The low run's gaps hold no stall: their largest rAF interval, 33.4 ms,
      is under the run's own maximum of 50.1 ms. The profiled run's reach 66.8 ms, above its kept
      maximum of 50.1 ms: about four vsyncs, in stops of 5 s. The high run's reach 183.4 ms, the
      descent arc's own maximum on high, where that segment's p99 is 100 ms.
    - _Decoding_ is from the last stop's log line to the last window's reduction line. The main
      process's RSS swings by about 0.5 GB around each stop as the stream's base64 chunks are
      decoded, and it does not grow over a run.
    - _Disk_ is the run's profile, where the window files wait until the end, plus the browser's
      open spool, from the sizes of its open files (`/proc/<browser>/fd`, every 0.5 s).
    - _The spool_ was `target/descent-spike/tmp.*/.org.chromium.Chromium.* (deleted)` in every
      sample that caught a stop (36, 28 and 89 samples), on disk and nowhere else, apart from the
      browser's own `/dev/shm` segments. The sampler first read the Electron CLI's Node launcher as
      the browser: Chromium rewrites the browser's command line, so the browser is now found by its
      executable.
    - _Figures present or null:_ `validateResults` passes on all three, and every null has its
      reason. The nulls are "no window shown" (T, and the presentation figures whole and by
      segment) and NVIDIA's absent DRM fdinfo. `mainThread.engine` is null with the profiler-off
      reason in the timed runs and present in the profiled one. The GPU process's slices are
      `GPUTask` alone in the timed runs, and `WebGPU`, `GPUTask` and `VulkanQueueSubmitHook` in
      the profiled one. The high run adds "no timed pass" (the finding below).
    - _Callbacks after the next frame's rAF time_ (addendum A item 2, information only) are a
      callback start later than `scriptStartMs` + 1000 × the next frame's script time: 1, 4 and 15
      in the three runs.

  - **The guard is confirmed at 1 s.** Pooled over every boundary of the two timed runs, 66 of
    3,983 rAF intervals (1.66%) in the 5 s after the guard exceed their window's 99th percentile,
    within the 2% allowed. `TRACE_BOUNDARY_GUARD_S` stays 1.
    - By run: low 17 of 2,382 (0.71%), high 49 of 1,601 (3.06%).
    - Of the high run's 49, 39 are at the 950 s boundary. Its next window's p99 is the busy
      stretch's 33.4 ms, while the 5 s after the guard are descent-arc frames, whose own p99 on
      high is 100 ms. The excess there is the segments' difference, not the boundary.
    - For guards of 2, 3, 4 and 5 s the pooled shares are 1.18%, 0.88%, 0.67% and 0.83%. The
      profiled run gives 1.10% at 1 s, for information.
  - _Headline figures_ (hidden, so read against no T):
    - **Low:** rAF p50 / p95 / p99 16.70 / 16.80 / 33.30 ms, maximum 50.10 ms. GPU p95: terrain
      0.68 ms, atmosphere 3.58 ms. Our code's p95 is 2.10 ms; GPU memory 0.256 GB; 808 frames
      dropped.
    - Against T14.c's run of 2026-10-04 (load 10–19), rAF p95 falls from 33.30 to 16.80 ms and
      our code's p95 from 7.10 to 2.10 ms, the effect of a quiet machine.
    - **High:** rAF 16.70 / 83.30 / 83.40 ms, maximum 183.4 ms; our code's p95 4.60 ms; GPU
      memory 0.719 GB; 18,027 frames dropped.
    - **Profiled:** rAF as low's; terrain 0.67 ms, atmosphere 3.55 ms, our code's p95 2.50 ms.
  - **Finding, for T17 and T19: the high setting's atmosphere at altitude, and its pass timer.**
    - In the high run the pass timer gave times only for the first 9.75 s (399 of 55,120 frames),
      then none, so every GPU-pass figure of the high file is null with "no timed pass". The low
      runs timed every frame.
    - In those first 10 s the atmosphere's view pass took 6.42 ms at the median and 70.05 ms at
      the 95th percentile, against the high setting's 1 ms limit. The terrain pass took 0.13 ms.
    - The high run's frames are slow where the atmosphere fills the view. In the orbit coast and
      the descent arc the rAF p95 is 66.7 and 83.3 ms; from the approach on it is 16.8 ms.
    - The canvas was 1,398 × 793 px, hidden. Whether the timer stops because the GPU falls behind
      the read-backs is not separated here. It is open for the orchestrator, ahead of T17's
      visible high runs. (Diagnosed in the next record, "R05.T14, the high run's pass timer and
      atmosphere, diagnosed".)
  - _Other deviations._ The full runs wrote their files straight into this directory, as the
    owner's runs will. The A/B runs wrote theirs under `target/laneD/t14f2/` (`--out`). The
    README's T16 and T17 commands use the seeds 7, 0 and 1, which the clearance ruling checked by
    hand; the plan names no seeds.

- **R05.T14, the high run's pass timer and atmosphere, diagnosed** (2026-10-06, after T14.f).
  Lane D reproduced the first 150 s of T14.f's high run three times. Each was hidden, seed 7, on
  the RTX 3080, under `just _locked` in an 8 G scope, with load ≤ 5 at the start. The build
  carried temporary instruments, none committed (`target/laneD/t14diag/`): the renderer's console
  in the log, the timer's number against the spike's count every frame, the three "atmosphere
  view" dispatches labelled apart, the ray march's size, and `nvidia-smi` at 100 ms.
  - **The timer stopped because the spike miscounted, and that is fixed.** R07.T19 (7e80d5d, after
    T14.c's low run) made `PassTimer.resolve` number a resolve it drops when all 135 read-back
    pairs are in flight. Such a resolve never calls `resolveQuerySet`. The spike's
    `ResolveCounter` counted those calls, so after the first drop its numbers fell behind the
    timer's. Every later report then carried a number past every recorded frame and was thrown
    away.
    - Evidence (d1, the committed counter): the timer's one warning ("135 resolves' pass times
      are still being read; dropping some") came at 9.25 s. By 13.2 s the timer was 179 ahead of
      the count, and from then on every report (300 a second) was unmatched. Of 7,182 frames, 556
      were timed.
    - The ring ran out because a hidden window renders offscreen. Its rAF keeps 60 Hz whatever
      the GPU does, so in a 70 ms plateau (below) the GPU fell more than 27 frames behind (135
      pairs at 5 resolves a frame). The low runs never did, so their two counts agreed.
    - Ruled out: the query set (5–7 timed passes a frame of its 64); a `mapAsync` that never
      settles (the lag went back to 5 resolves after every backlog); a device error (none
      logged); the untimed fallback (`untimedPasses` counts only passes beyond 64, and was 0); and
      a resolution controller (the spike runs none, and the march's size stayed 1,398 × 793 at
      scale 1 with 32 samples).
    - **Fix (lane D):** `ResolveCounter` now follows the engine's own count,
      `RenderEngine.passTimesFrame` (`follow`, called in `spikeGpu`'s `load`), which counts
      dropped resolves too. `deviceMade()` carries the run's highest number across a rebuild, since
      `ResilientEngine` releases the lost engine before it asks for a device. Four tests: the
      engine's count with a dropped number, a rebuild through the real `ResilientEngine`, a frame
      after a dropped resolve getting its times, and `load` following the engine. The first three
      fail on the old counter, and the last fails without `follow`.
    - Run with the fix (d2): 6,403 of 6,937 frames were timed, to the end.
      - 402 (6.2%) were dropped whole during GPU backlogs, and so are the slow frames. A hidden run
        whose GPU falls behind therefore understates its GPU rows.
      - 110 frames (1.7%) are partial, which moves no figure (atmosphere p95 70.41 against 70.40 ms
        without them).
      - _Ruled_ (2026-10-06, addendum B of `decision-r05-trace-windows-2.md`): incomplete frames are
        left out of every per-frame sum and counted, dropped and partial apart (results version 5).
        A GPU row's verdict must hold for any times they had: pass only with them all above the
        limit, fail if failing with them all below it, else not measured (T14.j).
  - **The atmosphere's cost is real GPU work, and it comes from how the ray march's shader reads
    its medium.**
    - Of the three dispatches, the per-pixel ray march is the cost: p50 6.12 ms, p95 69.88 ms.
      The sky view takes 0.18 ms at p95 and the aerial perspective 0.38 ms. The per-planet tables
      are baked once, at frame 0 (90.6 ms), never again.
    - From 400 km every pixel whose ray meets the shell marches 32 samples. The camera's attitude
      is steady, so the work a frame is steady too.
    - Yet the march runs in a cycle of 9.0–11.4 s: 0.62–0.65 s at a median 24.6 ms, then
      1.63–1.68 s at about 6.1 ms, then 0.93–1.08 s at a median 70.0 ms, then 5.7–8.0 s at about
      6.1 ms. The cycle is not fixed in script time: d1's phase differed and stopped at 127 s.
    - The GPU stayed at P0 (1,965–1,980 MHz, memory 9,501 MHz) throughout. In the 70 ms plateaus
      it ran at 100% utilization and 276 W (p50), against 46% and 197 W between them: more work,
      not waiting.
    - The cause is `common.wgsl`'s and `view.wgsl`'s helpers. `sourceAt`, `mediumAt` and
      `phasedScatteringAt` take the 416 B `Medium` by value and index its `terms` with a loop
      variable, three loops a sample. A dynamically indexed array held by value goes into a
      per-invocation variable (Tint's handling), which is the likely mechanism.
    - Experiment (d3, temporary, `instrument3.py`): the march's helpers read the module-scope
      `medium` uniform in place, with the same arithmetic.
      - The march fell to p50 2.20, p95 2.84 and at most 10.5 ms, with no plateau.
      - rAF p95 / p99 / max fell to 16.7 / 16.8 / 33.4 ms, with no interval above 40 ms. All 8,976
        frames were timed.
    - At that load the driver ran the GPU at 900–1,320 MHz (P3/P5, SM about 27%). The march's time
      scaled with the clock: 2.22 ms at about 1,100 MHz and 1.31 ms in the 150 frames at P0.
    - So high's atmosphere would be about 1.6 ms at full clock, and 2.7–3.5 ms at the clocks the
      driver picks. That is still over the 1 ms row.
    - The 1 ms row is the brainstorm's estimate, "0.5–1 ms: sky-view and aerial-perspective
      tables, and a ray march from orbit", which the brainstorm calls "an estimate, not a
      measurement" for this plan to replace (T18).
    - T15's offscreen replay of low (wgpu's compiler, naga, gave 0.165 ms against the client's
      3.57 ms) is consistent with this, though not proven by it.
  - **The hitches are the march's plateaus.** Once the backlog fills the renderer, rAF falls to
    the GPU's 70 ms frames, 66.7–83.3 ms (4–5 vsyncs), for about 3 s of each cycle: about 45 long
    intervals every 10 s through the coast and the arc.
    - Inside the atmosphere the march skips the sky (`needed` in `rayMarch.wgsl`), consistent with
      T14.f's 16.8 ms p95 from the approach on.
    - Ruled out: a table bake (once only), the main thread (our code's p50 2.1 ms), and the GPU's
      clocks (P0 throughout).
  - **Options, for lane C (the shader is R05.T12's):**
    1. Read the medium in place, in every kernel that passes `Medium` by value, not only the
       march. Check it with the atmosphere's render tests.
    2. Toward 1 ms at full clock: evaluate the terms' densities once a sample, not three times;
       march fewer samples; or march at half resolution with low's depth-aware upsample.
    3. Have T18 re-rule the row from measured figures.

    The low setting runs the same shader, at 3.57–3.73 ms against its 4 ms limit here, so the fix
    bears on T16 (the UHD 620) too.

    **Option 1 landed (2026-10-06, R05.T12, lane C):** every kernel reads the medium in place, and
    every read-back is bit-identical. The march has no plateau now, and high's row is about
    1.6 ms at P0, still over 1 ms. Figures and the options left are in "Deviations in T12.b and
    T12.c, as built: the medium read in place".

    **Ruled** (2026-10-06, `decision-r05-high-atmosphere.md`): the densities once a sample are
    taken, with the steps placed toward the dense air, in R05.T12.e. Fewer steps and half resolution
    are not taken on high. Design note 21's rest of the frame is judged on terrain and atmosphere
    together (T14.l).

  - **A measurement caveat for T17 and T18:** a GPU row is read at the clocks the driver picks. At
    the spike's light load, this GPU runs a compute-bound pass at about 1.7 times its time at full
    clock. _Ruled_ (2026-10-06, addendum B): a row measures the pass at the clocks the driver chose,
    never pinned or normalised. The clocks are recorded at 1 Hz beside the rows, from `nvidia-smi`,
    or i915's or amdgpu's sysfs, else null with the reason (macOS), and noted on a row measured
    below 90% of the maximum clock (T14.k).
  - **T17:** with the fix merged, the timer no longer stops. On the committed shader, though,
    high's atmosphere misses its row by 6 times between plateaus and 70 times at p95, and the
    frame rows fail in the coast and the arc. Lane D's view is that T17's high runs wait for
    option 1; until then they would measure a known failure.

- **Deviations in T14.j, as built** (2026-10-06, incomplete frames and results version 5;
  `decision-r05-trace-windows-2.md`, addendum B).
  - _Where things are._ The report's `frames.missingResolves` (`preload/api.ts`) holds each
    frame's resolves that never reported. `SpikeMetrics` keeps a per-frame count of reports and
    takes the difference from the resolves the frame numbered, after the frame before it, in
    `report()`, floored at 0. `readDescentSpikeReport` requires the series, as long as the frames'
    and whole. `results.ts` adds `IncompleteFrames`, `GpuClocks`, `GpuClockSource`,
    `boundedGpuRow`, `incompleteFramesReason`, `GPU_ROW_IDS`, `GPU_CLOCKS_NOT_READ` and the private
    `rankIndex`, which `nearestRank` and the schema check now share. The three GPU rows are built
    by `boundedGpuRow`. `gpu.passes` keeps every time that arrived, and `gpu.sumP95Ms` and the rows
    read the complete frames only.
  - _Dropped or partial._ The report carries only the missing count, as ruled. The main process
    tells the two apart by the pass series: the timer numbers a resolve only when it timed a pass,
    and every reported resolve writes its passes. A frame with a missing resolve and no pass time is
    therefore dropped, and one with some is partial.
  - _The frames counted._ `gpu.incompleteFrames` is `{ dropped, partial, frames }`, one field more
    than the ruling's. `frames` is the frames counted, so that the check can bound the counts in a
    replay's file, which has no rAF figure. The counts cover the frames after the warm-up outside
    the trace's boundary exclusions, the frames every per-frame figure reads, so the bound's k is
    over the rows' own frames. In a client's file `frames` equals the rAF statistics' count, and
    the schema check holds it to that.
  - _The bound._ A row is judged twice, with its k incomplete frames placed at +∞ and at −∞. When
    both judgements agree, that is its verdict; otherwise it is not measured with the ruling's
    reason, "k frames' pass times were incomplete (the GPU was more than 27 frames behind)". When
    both placements are `marginal` (a quantized timer), the row stays marginal, since every
    placement between gives the same verdict. The ruling names only pass, fail and not measured.
    A row that holds with k > 0 carries the note "k frames with incomplete pass times left out; the
    verdict holds whatever their times". With k = 0 the verdict is `row`'s, as in version 4 (a
    test compares the two).
  - _The schema check._ Version 5 only. `gpu.incompleteFrames` must be whole counts with dropped
    plus partial at most `frames`. A GPU row's verdict must be one its count allows. From the
    counts alone, a pass (or marginal) needs some m ≤ `frames` whose 95th-percentile rank lies
    below m − k, about 5% of the frames, and a fail needs the rank of `frames` to reach k. The rank
    is `nearestRank`'s own arithmetic. `gpu.clocks`, when present, must name `nvidia-smi`,
    `i915-sysfs` or `amdgpu-sysfs`. Its maximum must be whole MHz, and each of its three columns
    must have the memory series' column form, as long as `tMs`. `checkColumn` names the column's
    unit (KiB, MHz or P-states).
  - _For T14.k._ `GpuClocks` is typed ahead of its reader: `maxGraphicsMHz` is a `Measured<number>`
    so that a missing maximum is null with its reason, and `MemoryColumn` documents its unit as
    the field's name gives it. T14.k may refine both.
  - _The summary._ A new line after the trace's: "**Incomplete pass times:** d dropped and p partial
    of N frames after the warm-up, left out of the GPU rows' sums, whose verdicts hold whatever
    their times", or "none of N frames after the warm-up", or the reason.
  - _Frames still in flight at the report._ The report is taken after the trace's last stop. A
    resolve whose read is still in flight then counts as missing: at the report it never reported.
    The count is then conservative. It can make a row not measured, never pass or fail it wrongly.
    The reason still names the 27-frame backlog. A trace that failed before its first window stops
    at once, so a run's last few frames could be counted so. Waiting for the reads before the
    report would be a controller change (open, for the orchestrator). _Ruled_ (2026-10-06, the
    orchestrator): the controller waits up to 1 s for the reads in flight before the report, and a
    read still outstanding then is missing with its own reason, "read in flight at the report", not
    the backlog's. Built in T14.k (its record, below).
  - _The replayer._ It resolves once a frame, so its frames are whole or dropped (`partial` 0). It
    counts a frame dropped when its mapping or its read failed. Before, such a frame vanished from
    the figures (a failed mapping) or counted as untimed (a failed read).
    - `Replayer::read_pass_times` returns `Result<_, wgpu::MapRangeError>`. `Frames::finish`
      returns `FrameTimes { read, unread, untimed, unread_cause }`.
    - The first cause becomes one of the replay's findings.
    - `ReplayFigures` has `unread_frames` and `untimed_frames`, and `frames()` sums them with the
      frames read, so the unread are never more than the frames.
    - Its rows are bounded the same way (`PassTiming::row`, sorting its own values, by index
      arithmetic). Its reason is its own: "k frames' pass times could not be read back". The
      ruling's names the browser timer's 27-frame ring, which the replay has none of.
    - `gpu.clocks` is null with the client's reason.
    - A replay file it writes validates under the client's `validateResults`.
  - _The committed files._ The six files are converted with both new figures null, "not recorded
    before results version 5", and every row as written. The converter checks losslessness: it
    removes the two figures, restores the version and compares the result with the version 4 file,
    values and key order alike.
    - The five client files were rewritten from JSON and formatted by Prettier, with the keys in
      `buildResults`' order.
    - The replay's file was edited as text, in serde's sorted order, so that its `14.0`-style
      numbers stay as written.
    - Sizes: 109,638, 109,639, 9,148, 117,928, 118,913 and 119,283 B.
    - The five summaries were regenerated by `summaryMarkdown` and Prettier, and differ from the
      committed ones by the new line alone.
  - _Files beyond the task's list._ `main/spikeSession.test.ts` (the report fixture's field),
    `tools/gpu-replay/src/run.rs`, `replay.rs`, `window.rs` and `tests/replay.rs` (the replayer's
    own count). `spikeHarness.ts` needed no change: `SpikeRecorder` passes the engine numbers that
    `SpikeMetrics` counts. The README gains a "How the figures are read" bullet on incomplete pass
    times, beside the `gpu` row's addendum text. In this plan, T14.j's text says "addendum B's
    bound (Design note 21)" for "ruling 1's bound", and Design note 18's paragraph is re-wrapped
    after the inserted clause.
  - _The smoke._ The hidden `just descent-spike --smoke` (high, RTX 3080, port 7893, an 8 G scope,
    `DISPLAY=:0`, offscreen) exited 0 twice. The first run took 22 s at load 2.8, with windows of
    3,307,956, 2,047,334 and 3,514,768 B (0.15, 0.12 and 0.22 %) and 117, 66 and 174 frames checked.
    The second, on the final tree, took 28 s at load 18, with 3,018,069, 2,597,484 and 3,346,684 B
    (0.13, 0.17 and 0.20 %) and 105, 91 and 160 frames checked. The report with its new series
    passed the main process's reader. A smoke writes no results, so the counts themselves are
    proven by the tests.
  - _Gate (all capped)._
    - The acceptance vitest: 4 files, 181 tests.
    - `pnpm test`: 325 files, 5,483 tests.
    - The typecheck from a clean cache, `just check lint` and Prettier.
    - `just gpu-replay-check`: 27 unit tests and clippy. The replayer's GPU test also passed,
      offscreen.
    - The commit hooks.
  - _Review._ typescript-reviewer found no must-fix. Its three should-fix items are applied:
    - tests for an impossible `marginal`, a replay's own count, and the boundary exclusion;
    - unit-neutral names in the column check;
    - unit-suffixed parameters on `boundedGpuRow`.

    Of its two considers, this record is one. The other, frames still in flight at the report, is
    recorded above as open.

    rust-reviewer found one must-fix, a float compared through `serde_json::Value` in a test, now
    compared with a tolerance. Its three should-fix items and three considers are all applied:
    - one row builder with one limit, which sorts its own values;
    - `#[must_use]`;
    - a test for a row with only unread frames;
    - the read-back's error kept as a finding;
    - frame counts that hold by construction;
    - no reallocated placements.

- **Deviations in T14.k, as built** (2026-10-06, the GPU's clocks; `decision-r05-trace-windows-2.md`,
  addendum B, ruling 2, and the orchestrator's ruling on T14.j's open question).
  - _Where things are._ `main/gpuClocks.ts` holds `GpuClockReader` (its `read(nvidia)` never
    rejects), `GpuClockSample`, `GpuClockReadings`, `GpuIdentity`, `SysfsFiles`, `nvidiaClocks`,
    `parseDpmLevels` and the reasons' constants. `GpuClockSource` moved there from `results.ts`,
    which re-exports it. `parseNvidiaSmi` (`main/fdinfo.ts`) gives each GPU's `clocks`, each block
    read within its own element, since `applications_clocks` and the others reuse the names.
    `results.ts` adds `MemorySample.clocks`, `MemorySources.clocks(nvidia)`, `gpuClocksOf`,
    `clockNote`, `CLOCK_NOTE_FRACTION`, `describeClocks`, `describeClockSamples` and an exported
    `activeGpu`, and drops `GPU_CLOCKS_NOT_READ` (the replayer's copy too). `index.ts` reads the run's
    GPU from `app.getGPUInfo("basic")` once, at the first sample.
  - _The source._ It is chosen once, from Chromium's active GPU. NVIDIA (vendor `0x10de`) reads
    `nvidia-smi` on every platform but macOS; Intel (`0x8086`) reads i915 and AMD (`0x1002`) amdgpu,
    under Linux alone, on the lowest-numbered `card<N>` whose `device/vendor` and `device/device`
    are the GPU's. A GPU Chromium does not report, another vendor, or no matching card gives
    `gpu.clocks` null with the reason. NVIDIA's clocks are the reading's first GPU's, as T14.c's
    memory figure is; a machine with two NVIDIA GPUs running on the second would read the wrong
    one (not handled).
  - _The readings._ i915 has no memory clock (it shares the system's memory) and no performance
    states, and amdgpu's levels are not performance states (their numbering runs the other way
    from NVIDIA's), so those columns are null with their reasons. The maximum is the largest any
    sample read, which is constant in practice. A run whose graphics clock is missing at every
    sample has `gpu.clocks` null with that reason, rather than a source with empty columns.
  - _After the warm-up._ The memory series' times count from the measuring's start, which the
    committed runs place 0.07 to 0.15 s after script time 0 (their first window's `fromS`). The
    note and the summary read "after the warm-up" as `tMs ≥ 1000 × warmupS`, so at 1 Hz at most one
    sample is misplaced. The summary computes it from the file alone.
  - _The note._ "measured at a median N of M MHz (the driver's choice at this load)", without
    thousands separators. It is added after a row's own note with "; ", and only to a GPU row with
    a value. The median is by nearest rank, the files' convention.
  - _The summary and the logs._ The summary's new line is "**GPU clocks:** from <source>;
    graphics median N MHz (P %), p5 …, p95 …, after the warm-up, of M MHz; memory median … MHz;
    performance states Pa to Pb". A smoke's 10 s are all warm-up and it writes no file, so its log
    lists every sample (`descent spike: GPU clocks sampled …`); a full run logs the summary's line.
  - _No schema change._ Version 5's shapes hold, so the version stays. The six committed files keep
    `gpu.clocks` null ("not recorded before results version 5"); the five summaries gain the clock
    line, "— (not recorded before results version 5)", and nothing else.
  - _Fixtures._ `nvidia-smi.xml` was re-recorded unprivileged on the RTX 3080 (2026-10-06, idle:
    P8, 210 and 405 MHz). It is trimmed to the elements read, with the clock blocks that reuse
    their names. Its maximum graphics clock is 2,115 MHz, above the 1,965–1,980 MHz the diagnosis
    saw at P0, so a run at P0 notes nothing. `fdinfo.test.ts` follows its new memory (123 MiB, two
    processes). The sysfs trees are `sysfs/i915` (a UHD 620, `0x8086:0x5917`, RP0 1,150 MHz),
    `sysfs/amdgpu` (an RX 6800 XT, `0x1002:0x73bf`, as card1 beside an Intel card0) and
    `sysfs/i915-no-rp0`, which the replayer's fallback test reads in place of a temporary
    directory. The kernel writes `pp_dpm_*` lines with a trailing space, which the repository's
    trailing-whitespace hook would strip from a file, so the trees omit it and the parsers' tests
    hold the kernel's exact form inline.
  - _The replayer._ `tools/gpu-replay/src/clocks.rs` reads the same sources, chosen at run time by
    `std::env::consts::OS` and the adapter's vendor and device, with no `cfg` gating, so every
    platform compiles every reader. `nvidia-smi` is bounded at 5 s, as the client's is. A
    `ClockSampler` takes its first reading before the first frame, then one a second on its own
    thread, and the last after the frames' GPU work has ended. `memory.series.tMs` holds the
    clock samples' times, and every memory column stays null. The GPU rows carry the client's note,
    a replay having no warm-up. Its file validates under the client's `validateResults` (the gate's
    replays, below).
  - _Reads in flight at the report_ (the orchestrator's ruling on T14.j's open question). After the
    trace's last stop the controller waits up to `PASS_READS_WAIT_MS` (1,000 ms) for the reads of
    the recorded frames' pass times still in flight, then takes its report. A read still
    outstanding then is missing with the reason "read in flight at the report", not the backlog's.
    - `view/spike/passReads.ts`: `PassReads` and `trackPassReads`. The spike's device wrapper
      replaces `createBuffer` on the device instance, and `mapAsync` on each staging buffer R01's
      timer labels `pass times readback <k>`. A read's number is `ResolveCounter.value` when its
      mapping begins, which is its resolve's, since the timer maps just after the resolve's own
      submission. It ends with its report, or with a failed mapping. A resolve the timer dropped
      makes no read, so it is never in flight. A test through the real engine checks the label.
    - `SpikeRecorder.readsSettled` waits for reads numbered after its first engine frame up to its
      last frame's, so reads begun before the numbering followed the engine, or after the run's
      last frame, are never waited for. A timed-out wait is logged.
    - The report's `frames.inFlightResolves` counts each frame's missing resolves still being read
      (at most its `missingResolves`), and the reader checks it. A hand-made report needs it.
    - `buildResults` counts a frame whose every missing read was in flight as incomplete with that
      reason (`READ_IN_FLIGHT_REASON`). A frame with a dropped resolve as well is the backlog's. The
      counts stay within `dropped` and `partial`, so the schema is unchanged. `IncompleteCount`
      (`{ frames, inFlight }`) is what `boundedGpuRow` and `incompleteFramesReason` take now. The
      not-measured reason names both causes with their counts, and the "verdict holds" note names
      the frames in flight.
    - A read whose mapping succeeded but whose report never came (the timer's
      `getMappedRange` threw) stays in flight, and so counts as in flight after the wait.
  - _The smoke._ The hidden `just descent-spike --smoke` (high, RTX 3080, port 7893, an 8 G scope,
    `DISPLAY=:0`, offscreen, no resize) exited 0 twice, each logging its ten clock samples from
    `nvidia-smi`, against a maximum of 2,115 MHz.
    - The first took 22 s at load 11.8. Its graphics clock was 210 MHz, then 1,965 MHz six times
      and 1,980 MHz three times; its memory clock 405, then 9,501 MHz; its state P8, then P0.
      Windows of 3,198,235, 2,781,053 and 3,460,885 B (0.13, 0.18 and 0.25 %), and 116, 104 and
      171 frames checked.
    - The second, on the final tree, took 24 s at load 27.5: 285 MHz, then 1,965 MHz three times
      and 1,980 MHz six times, with the same memory clocks and states. Windows of 2,351,844,
      3,038,634 and 3,321,068 B (0.12, 0.18 and 0.26 %), and 70, 115 and 166 frames checked.
    - So the GPU runs at P0 within a second of the smoke's start, at 93% of its maximum, which puts
      no note on a row. A smoke writes no results, so the file's clocks are proven by the tests and
      the replay.
  - _Gate (all capped)._
    - The acceptance vitest: 3 files, 183 tests.
    - `pnpm test`: 327 files, 5,542 tests.
    - The typecheck from a clean cache, `just check lint` and Prettier.
    - `just gpu-replay-check`: 40 unit tests and clippy.
    - `just cross-clippy`, for macOS and Windows. Its caches were seeded by a reflink copy of the
      primary checkout's `target/cross` and `target/tools-cross`. It ran in a 10 G scope, as one
      Clippy over the whole workspace for two targets.
    - The replayer's GPU test, offscreen. A replay of the fixture offscreen read the clocks
      twice. At idle that was 210 of 2,115 MHz at P8; an earlier one, before the review fixes,
      read 1,800 MHz at P0. Its file validates under the client's `validateResults`.
    - The commit hooks.
  - _Review._ typescript-reviewer found no must-fix. Its three should-fix items are applied: a
    regression test for the clock check without the series' times, the report reader's refusals of
    a bad `inFlightResolves`, and the tests that had more than one reason to fail split. rust-reviewer
    found no must-fix. Its three should-fix items and its consider are applied: the sampler's
    interval passed in (`start_every`), so that no test's count of samples rests on the wall clock;
    the clocks module
    private to the crate; `#[must_use]` on the new pure helpers; and a timed-out `nvidia-smi` killed
    and given up without the blocking wait, which a run stuck in the driver would hold (its
    not-found and timed-out paths now tested, the latter on Unix).
  - _Files beyond the task's list._ `preload/api.ts`, `main/spikeReport.ts`,
    `main/fixtures/spikeReport.ts`, `main/spikeSession.ts` and its test, `main/index.ts`;
    `view/spike/passReads.ts`, `metrics.ts`, `spikeHarness.ts`, `spikeController.ts` and their tests
    (the orchestrator's addition); the five summaries; and `tools/gpu-replay/src/lib.rs`,
    `run.rs` and `window.rs`.

- **Deviations in T20, as built** (2026-10-06, the machine's facts on every platform;
  `decision-cross-platform-server.md`, items 6 and 7).
  - _Where things are._
    - The replayer's `results.rs` holds `MachineSources`, whose fields are private and whose
      `read()` and `findings()` the crate calls, and the private `Platform` (`Linux`, `MacOs`,
      `Windows`, `Other(name)`). The per-platform rules are the pure functions
      `load_average_on(platform, read)` and `governor_on(platform, read)`, which `read()` calls
      and the tests run for every platform on any host. `machine(sources, adapter)` and
      `quiet(load)` write them.
      `ReplayFigures.machine` carries the facts, and `run.platform` is their platform's name.
    - The client's `main/machineLoad.ts` holds `LoadAverage`, `LoadSources`,
      `NO_WINDOWS_LOAD_AVERAGE`, `QUIET_RULE_UNCHECKED`, `keepsLoadAverage`, `readLoadAverage`,
      `recordedLoadAverage`, `Quiet` and `quietOf`. `results.ts` uses it, and `descentDemand.mjs`
      loads it through Vite's `runnerImport`, as it loads `demandRecord.ts`. `MachineSources`
      extends `LoadSources`, so it gains `platform`, which `nodeMachineSources` fills from
      `process.platform`. `run.quiet` is typed `Quiet`.
  - _Read at the start._ The replayer read the machine when it wrote its results, after the
    replay, so its "load average … at the start" was the load at the end, the replay's own work
    included. Both the offscreen and the presented replay now read the facts before that work. This
    goes beyond the ruling.
  - _No schema change._ Results version 5's shapes hold: `loadAverage` stays a number triple, zeros
    on Windows, and `run.quiet` says why. The committed files do not change. All six results files
    are Linux runs, the five summaries regenerate byte-identical, and a replay of the fixture on
    this machine records the committed replay's name, CPU, threads, memory and governor. _Since
    results version 6_ (2026-10-06, the orchestrator's ruling after T14.l), the results file
    records `loadAverage` as a `Measured`, none with its reason on Windows ("Deviations in T14.l,
    as built").
  - _Facts that could not be read._ The schema keeps the host name, CPU, threads and memory plain
    values, so an unread one is written as before (`"machine"`, `"unknown"`, 0 and 0), and a line in
    `replay.findings` says which, for example "the machine's memory could not be read:
    run.machine.memoryBytes is 0". This goes beyond the ruling. The client's Node readers always
    answer; a short `os.loadavg()` reading, which Node never gives, throws.
  - _The load average's limits._
    - On Windows `sysinfo` is never asked. It emulates a load average from the processor queue's
      length, sampled every 5 s from its first call, so at a replay's start it would read 0.
    - A platform `sysinfo` has no backend for (`IS_SUPPORTED_SYSTEM` false) has none either, "no
      load average on <platform>", so its replays are provisional too (rust-reviewer's consider).
    - Elsewhere `sysinfo` gives zeros where the platform's call fails (an unreadable
      `/proc/loadavg`, a failed `getloadavg`), which cannot be told from a reading. The replayer
      did the same on Linux before. This is stated, not handled.
  - _The CPU on Arm Linux._ `sysinfo` names an Arm CPU by its "CPU part" where cpuinfo has one,
    over the model name, so an Arm Linux replay's `cpu` differs from before. None of the three
    target platforms is affected.
  - _The demand record._
    - A Windows cell records `loadAverage: []`. The type allows it, so the record stays version 3.
      The summary then adds "k of the n cells recorded no load average: Windows keeps none, so
      whether the machine was quiet is unchecked."
    - The script now imports the surface module by a `file://` URL, since Node's loader refuses a
      Windows path (`C:\…`) as a specifier. typescript-reviewer found this beyond the rules: the
      script would otherwise still have failed on Windows before its first cell.
    - Vite's `runnerImport` takes the two TypeScript modules' plain paths. That it accepts a
      Windows path is unverified, since there is no Windows runner.
  - _The summary._ A Windows run's summary says "(provisional: Windows keeps no load average)" and
    "load average none (Windows keeps none)" in place of the zeros. `keepsLoadAverage` takes Node's
    `win32` and Rust's `windows`, a replay's `run.platform`.
  - _Tests._
    - The replayer, 14 new: the two rules on each platform; fixture sources for Linux, macOS and
      Windows; the name's normalisation; an unread CPU as `"unknown"`; the findings; this machine's
      facts; and, under `cfg(target_os = "linux")`, this machine's facts against `/proc`. Off Linux
      `this_machines_facts_are_read` runs alone, the stated limit.
    - `machineLoad.test.ts`, 12: the reading on each platform; the throw; the rule; the demand
      script's path with `/proc` absent, through an injected `os.loadavg`; and the script's own
      source, which records through `recordedLoadAverage` with Node's `loadavg` and names no
      `/proc` (the pattern has its own test).
    - `results.test.ts`, 10 new: a `win32` run is provisional with the note, with its two summary
      lines, and its zeros valid under version 5; a `darwin` run at load 0.5 is not provisional;
      `describeMachine` on macOS and Windows reads no file.
    - `demandRecord.test.ts`, 2: the summary's line, and its absence.
  - _Not done here._ Item 7's other platform branches are R12.T0's audit list, written into plan 12
    with the R12.T4.b launch contract. The views check (`viewsCheckResults.ts`) and the child-window
    check's summary (`smoke/childWindow.ts`) still judge `load >= 1` alone. They are another lane's
    files, so they are on that list and relayed to the orchestrator: each should take `quietOf`.
  - _Gate (all capped)._
    - The targeted vitest (`machineLoad`, `results`, `demandRecord`): 3 files, 211 tests.
    - `pnpm test`: 328 files, 5,650 tests.
    - The typecheck from a clean cache, `just check lint`, Prettier and rustfmt.
    - `just gpu-replay-check`: 54 unit tests and Clippy.
    - `just cross-clippy` for macOS and Windows, which checks `sysinfo` and the replayer for both.
      The ruling's own `--target aarch64-apple-darwin` Clippy is part of it, with no SDK.
    - `node --check apps/hyperion/scripts/descentDemand.mjs`.
    - The replayer's GPU test, offscreen. A replay of the fixture offscreen, whose file validates
      under the client's `validateResults`.
    - A short demand run through the new path, into scratch:
      `--rules calibrated --ridges off --settings low --cap-hours 0.0005 --wall-cap-hours 0.05`.
      It exited 0 in about 11 s, its cell's load average read from `os.loadavg()`.
    - The commit hooks.
  - _Review._ rust-reviewer found no must-fix. Its five should-fix items are applied: the
    per-platform rules made pure functions tested on any host; the fields and `Platform` private;
    the Windows choice an exhaustive match; `#[must_use]` on the pure writers; and a one-sentence
    summary line. So are its three considers: no load average where `sysinfo` has no backend; the
    Arm rule in the Linux test; and the getter's doc. typescript-reviewer found no must-fix. Its
    should-fix is applied, the tests split to one reason to fail each, and so are its three
    considers: the source test's patterns and their own test, the duplicate `loadavg` and `quiet`
    type removed, and the summary's wording built from the constant. Its finding beyond the rules,
    the Windows import, is fixed too.
  - _Files beyond the task's list._ `main/machineLoad.ts` and its test,
    `tools/gpu-replay/src/run.rs`, `window.rs` and `Cargo.lock`, `view/spike/demandRecord.ts` and
    its test, `docs/measurements/descent-spike/README.md`, and plan 12 (T0 and T4.b).

- **Deviations in T14.l, as built** (2026-10-06, the rest of the frame as one row and results
  version 6; `decision-r05-high-atmosphere.md`).
  - _The plan text._ Lane C's T12.e agent committed the ruling's plan text on its own branch
    (`docs(plans): R05 Add T12.e and T14.l, ruled by decision-r05-high-atmosphere`). It was not on
    `rendering-and-planets` when this task began. This task applies that commit's four T14.l hunks
    byte for byte, so that the two commits merge cleanly in either order: Design note 21's row and
    its decided paragraph, the order of work, T14.l's text and T19's sentence. The rest comes with
    lane C's commit: Design note 16's paragraph, T12.e, T12.c's sizes, T18's sentence and the Risks
    edits. Among those is the bullet that Design note 21's paragraph cites, "The per-frame marches
    under-sample the dense air", and the T17 sub-bullet's ruling, which replaces "The atmosphere
    row still fails at 1 ms".
  - _Where things are._
    - `results.ts` adds `TERRAIN_ATMOSPHERE_ROW`, `EstimatedRow`, `PassRowFigures` and
      `OVER_ESTIMATE_FINDING`. `GPU_ROW_IDS` is `headroom-gpu` and `terrain-atmosphere`, and
      `gpu.rows` is a `Record<EstimatedRow, PassRowFigures>`.
    - The private helpers are `passRowFigures`, `checkPassRow`, `checkRestOfFrame`,
      `describeRowFinding` and `msOrDash`. `gpuPercentile` takes its percentile, and `buildResults`'
      `sums` takes a set of pass rows. `LIMITS`' `terrainMs` and `atmosphereMs` are the two
      estimates.
    - `ESTIMATE_OF` names each estimated row's key among `LIMITS`. It `satisfies` a record over
      `EstimatedRow`, so a new pass row cannot be left out of the joint row. `ESTIMATED_ROWS` is
      taken from its keys (`isEstimatedRow`).
    - The replayer's `results.rs` adds `TERRAIN_ATMOSPHERE_ROW`, `PassTiming::percentile` and
      `PassTiming::pass_row`, and the per-frame `rest_of_frame` sums. The free `p95` is gone.
  - _The joint row._ `boundedGpuRow` reads each complete frame's sum of its passes whose row is
    terrain or atmosphere, in the frames with one, summed in the frame's pass order.
    - Its tolerance is the per-pass tolerance times the most such passes a frame has.
    - Its criterion is "terrain and atmosphere GPU time, summed per frame, ≤ 6 ms (5 + 1) at the
      95th percentile", or 18 ms (14 + 4) on low. Without a value its reason is "no timed terrain
      or atmosphere pass".
    - It is in `GPU_ROW_IDS`, so the clock note and T14.j's schema check of a verdict against the
      incomplete count apply to it.
  - _The parts' form._ The ruling names the fields, not their form. Each of `gpu.rows.terrain` and
    `gpu.rows.atmosphere` is `{ estimateMs, p50Ms, p95Ms, p99Ms, overEstimate }`:
    - The estimate is a plain number, always present, so that the schema check can hold the joint
      limit to the sum even where the percentiles are missing.
    - Each percentile is its own `Measured`, so that a converted file keeps a row's 95th
      percentile where it has no 50th or 99th.
    - `overEstimate` is the 95th percentile above the estimate, with the timer's tolerance aside
      since it is a finding, and `null` without a 95th percentile.
    - The parts read the complete frames, as the joint row does. When no complete frame has the
      row, the incomplete frames' count is the reason. They carry no clock note.
  - _The schema check._ Version 6 only. Beyond the ruling's refusal of a joint limit that is not
    the estimates' sum (to 10⁻⁹ ms):
    - `gpu.rows` must hold both rows, each with an estimate above 0 and percentiles that are
      numbers or null, in order where present.
    - `overEstimate` must be exactly whether the 95th percentile is above the estimate.
    - `criteria.whole` must hold one `terrain-atmosphere` row and neither of version 5's rows.
    - The estimates are not held to the setting's values: nothing else in the check holds a limit
      to its setting.
  - _The summary._ A section after the criterion, "Terrain and atmosphere against their
    estimates", gives one row each: the estimate, p50, p95, p99 and the finding. The finding is
    "over its estimate: a finding for T19 and R12" where it applies, then why any percentile is
    missing.
  - _The committed files_ (`target/laneD/t14l/tov6.mts`, not committed). All six are converted, and
    the replay's is edited as text, keeping serde's sorted keys and its `14.0` forms. A version 5
    file allows this much:
    - Each row's 95th percentile is its old row's value, or that row's reason.
    - A row of one pass (terrain, in the five files with pass times; the high run's has none) sums
      to that pass's time in each frame, over the same frames, so its 50th and 99th percentiles are
      the pass's. The converter checks that the pass's 95th equals the row's.
    - The atmosphere has two passes (three dispatches a frame in the replay), so its 50th and 99th
      are "not recorded before results version 6, whose file kept this row's 95th percentile
      alone".
    - The joint row is not measured. Its reason is "not recorded before results version 6: the file
      keeps each row's 95th percentile, not each frame's sum of the two" in the five files with
      values. In the high run's, which had no timed pass, it is "no timed terrain or atmosphere
      pass", as its version 5 rows gave it.

    The conversion is lossless for the old rows: rebuilt from the new figures, each equals its
    version 5 row. No overall verdict changed; all six were and are `not-measured`. Sizes are
    110,401, 110,402, 9,912, 118,497, 119,676 and 120,044 B. The five summaries are regenerated,
    and only the criterion table and the new section differ.

  - _Their figures_ (hidden, provisional). On low, the atmosphere's 95th percentiles are 3.73, 3.57,
    3.58 and 3.55 ms against 4 ms, terrain's 0.57–0.71 ms against 14 ms, and the replay's 0.17 and
    0.08 ms. None is over its estimate. The high run has none, since its timer stopped at 9.75 s.
  - _The smoke._ The acceptance's hidden `just descent-spike --smoke` exited 0 after 23 s, at a
    1-minute load of 12.3 (`target/laneD/t14l/smoke-1.log`).
    - The run: the high setting on the RTX 3080, port 7893, in an 8 G scope, under
      `hyperion-gpu.lock` and a build slot, with Electron's DBus address `/nonexistent`.
    - The trace: three windows of 3.2, 2.3 and 3.2 MB, at most 0.19% of the buffer, with the spans
      of 113, 77 and 144 frames matched.
    - The clocks: from `nvidia-smi`, 210 to 1,215 MHz in P8 to P3, of 2,115 MHz.
    - What it proves: a smoke writes no results file and returns before `buildResults`
      (`spikeSession.ts`), so it proves the harness, not this change.
    - The writer's own cross-check is an offscreen replay of the fixture
      (`gpu-replay replay tools/gpu-replay/tests/fixtures/small`, under the GPU lock). Its version
      6 file is accepted by the client's `validateResults`. Its joint row is 0.002 ms, with the
      clock note "measured at a median 1800 of 2115 MHz".
  - _Files beyond the task's list._ `tools/gpu-replay/tests/replay.rs`, the replayer's GPU test,
    which `just test-gpu-replay` runs and which is ignored by default. It looked up version 5's
    `terrain` and `atmosphere` rows by their ids. It now reads:
    - the joint row against low's 14 + 4 ms;
    - terrain's 95th percentile in `gpu.rows`;
    - the atmosphere's "no timed atmosphere pass", with `overEstimate` null.
  - _Tests._
    - `results.test.ts`, 13 new, in "a results file's rest of the frame":
      - 4.5 + 1.4 ms passes on high, and the parts' figures with the atmosphere over its estimate;
      - one criterion row and neither alone, and the summary's table;
      - a file over its estimate validates, and 6.2 ms fails;
      - the percentile is of the per-frame sums: 5.5 ms, where the two percentiles sum to 9;
      - no verdict reads the split: three runs at 5.75 ms, with neither, the atmosphere or terrain
        over its estimate, give equal criteria;
      - the parts read the complete frames;
      - the joint row's and a part's reasons without a timed pass, and the summary's;
      - the reasons of percentiles that differ.
    - `results.test.ts`, 9 new refusals in the schema check, and "refuses version 5".
    - The tests of the setting's limits, the quantized timer, the windowed trace, T14.j's
      incomplete frames and T14.k's clock notes now read the joint row, so T14.j's bounds are
      tested on it.
    - `results.rs`, 4 new:
      - the joint row against the estimates' sum, with the atmosphere over its estimate;
      - 6.2 ms fails;
      - the percentile of the per-frame sums, whose terrain part sits at its estimate and is not
        over it;
      - low's limit of 18 ms.

      Five updated tests now check `gpu.rows`, the untimed replay's among them.
  - _Gate_ (every run capped, cargo through a build slot):
    - the acceptance vitest, `src/main/results src/main/spikeReport`: 2 files, 212 tests, and
      with `spikeSession` 3 files, 234;
    - `pnpm test`: 331 and 9 files, 5,810 and 174 tests. One earlier run failed once in
      `SystemDisplay.test.tsx`, another lane's test, which passed three times alone and in the
      next full run;
    - the typecheck from a clean cache, `just check lint`, Prettier and rustfmt;
    - `just gpu-replay-check`: 58 unit tests and Clippy;
    - the replayer's GPU test, offscreen under the GPU lock;
    - the fixture replay, validated by the client's `validateResults`;
    - the six converted files, validated as version 6;
    - the hidden smoke;
    - the commit's hooks.
  - _Review._
    - typescript-reviewer found no must-fix. Its two should-fix items are applied: each test now
      has one reason to fail. So are its three considers: `ESTIMATE_OF`, the 95th percentile read
      by its name, and `GPU_ROW_IDS` built from the constant.
    - rust-reviewer had three must-fix items, all applied:
      - the GPU test still read version 5's rows;
      - an `.ok()` had no comment, and forced a clone;
      - three names had no unit.

      Its four should-fix items are applied too: the parts' percentiles and the equal-to-estimate
      boundary are pinned, the row ids are asserted with `assert_eq!`, the doc's summary line is
      split, and a stale comment is fixed. So is its consider, the untimed replay's parts.

    - plan-conformance-reviewer found that the change conforms. The four hunks are byte-identical
      to lane C's, and both merge orders are clean.
      - Its must-fix, to run the smoke, is done.
      - Its should-fix items are applied: a third run with neither row over its estimate, and this
        record's file list.
      - Of its considers, a stale comment and two wordings are fixed. Holding the estimates to the
        setting is not taken, for the reason under "The schema check".
  - _Version 6 carries T20's change too_ (ruled by the orchestrator, 2026-10-06, in a follow-up
    commit, `feat(spike): R05.T20 Record the load average as a value with its reason`). T20's
    ruling said that "a later schema version, bumped for other reasons, should make `loadAverage`
    a `Measured`", and version 6 had landed nowhere.
    - _The file._ `run.machine` is a `RecordedMachine`: the run's description with `loadAverage`
      a `Measured`. On Windows it is none, with `readLoadAverage`'s and `quietOf`'s reason,
      "Windows keeps no load average", in place of zeros. The replayer writes its reading's
      `Result` the same way, with "no load average on <platform>" where `sysinfo` has no backend.
    - _The description stays a triple._ `MachineDescription.loadAverage` keeps Node's zeros on
      Windows, since the views check's and the child window's records (R07) reuse it and keep their
      own schemas. `buildResults` takes the `Measured` from it by `keepsLoadAverage`, the one case
      in which `readLoadAverage` gives none.
    - _The summary._ A Windows run's line reads "load average none (Windows keeps no load
      average)", and its overall verdict's "(provisional: …)" takes the same reason.
    - _The schema check._ It refuses:
      - a bare triple, version 5's form ("… is a bare triple, version 5's form: version 6 records
        it as a value with its reason");
      - a value that is not three averages;
      - a value with a reason, or none without one;
      - a value on Windows;
      - a run with none that is not provisional.
    - _The committed files._ All six are Linux runs. Each keeps its averages as the value, edited
      as text (`target/laneD/t14l/load6.mts`) in its own key order, and their summaries regenerate
      unchanged.
    - _Tests._ `results.test.ts` gains a Windows run's none with its reason, its file's
      validation, a macOS run's measured value, and seven refusals. The Windows summary's wording
      is updated. `results.rs` gains an unsupported platform's none with its reason and its whole
      quiet note, and its Linux, macOS and Windows tests assert the `Measured` form.
      `tests/replay.rs` asserts the form its platform gives.
    - _Gate_ (capped; cargo through a build slot), on the branch with `rendering-and-planets`
      dcd99216 merged, so that the views check's and the child window's uses of the description
      are covered:
      - the targeted vitest (results, report, session, machine load, views check, child window):
        7 files, 316 tests;
      - `pnpm test`, the typecheck from a clean cache, `just check lint`, Prettier and rustfmt;
      - `just gpu-replay-check` (59 unit tests and Clippy), and the replayer's GPU test offscreen
        under the GPU lock;
      - a fixture replay's version 6 file, validated by the client's `validateResults`;
      - the six converted files, validated;
      - the commit's hooks.
    - _Review._ typescript-reviewer and rust-reviewer found no must-fix.
      - typescript-reviewer's should-fix is applied: the check now requires the reason.
      - rust-reviewer's should-fix is applied: `RESULTS_VERSION`'s doc names the change.
      - Their considers are applied too: the TSDoc is rewrapped, the GPU test asserts its
        platform's form, and the unsupported platform's whole quiet note is pinned.
      - rust-reviewer also asked whether a version 6 file written before this change could reach
        the check. None exists outside scratch, and the refusal now says "a bare triple, version
        5's form" rather than naming its version.

- **Deviations in T12.c, as built** (2026-10-02).
  - _The shape._ `hillaire.ts` holds `TableSizes`, `TABLE_SIZES`, `AtmosphereCamera`,
    `AtmosphereInputs`, `atmosphereInputs` and `HillaireAtmosphere`, as Provides sketches them,
    with these differences:
    - The constructor takes the figure as a fourth argument: `new HillaireAtmosphere(engine,
medium, sizes, figure)`.
    - `drawFrame(camera, sun, scene)` takes the terrain target as `AtmosphereScene` (colour,
      reversed-Z depth, near plane, pre-exposure scale). It dispatches the three per-frame kernels
      and returns the composite as a `DrawItem` for the caller to submit into another target or
      the view: the composite is a full-screen material draw, not a post-process, because a
      post-process cannot read a depth texture (R01 as built).
    - `SunState` is `{ directionBodyFixed, distanceAu, angularRadiusRad }`.
    - Added: `geodeticOf` (Bowring's iteration), `tableRadiusM`, `SpheroidFigure`,
      `aerialPerspectiveVolume()`, `tables`, and the kernel and material constants for the
      catalogue.
    - `atmosphereInputs` takes `SpheroidFigure` (`{ equatorialRadiusM, polarRadiusM }`), which
      T7.a's `BodyFigure` satisfies structurally. `planet.ts` is lane B's and did not exist yet.
  - _Which radius the tables use_ (the T12.b open point, settled here):
    - The per-planet tables are built on the figure's mean radius, R₁ = (2a + c) ÷ 3.
    - Every lookup reads them at the point's height above the datum (r_table = R₁ + h), with μ
      against the spheroid's normal.
    - The sky-view and aerial-perspective tables are built on the camera's own sphere, of radius
      √(MN) (Design note 16).
    - The ray march clips against the spheroid's shells: a + top and c + top for the atmosphere,
      a and c for the ground. Each sample's height is taken along its radius, which differs from
      the geodetic height by at most 2.1 m on WGS 84 up to 400 km. The sun's cosine is measured
      against the normal of the similar spheroid through the sample, within 0.011° of the true
      normal (science check, 2026-10-02).
    - A test holds the grazing optical depth from the ground on the WGS 84 spheroid to within 0.5%
      of the spherical oracle, at 0° and 45° and looking north and east, on both radii: √(MN)
      (worst 0.17%) and R₁ (worst 0.28%). It compares with the `f64` oracle rather than the GPU table,
      since a Node test has no GPU; T12.b's smoke check holds the table to the oracle to 1%.
    - `shaders/view.wgsl` (not in the task's file list) holds the shared `AtmosphereView`, the
      source term and the sky view's mapping, prepended to the three kernels and the composite.
      (The source term is in `source.wgsl` since 2026-10-06: "Deviations in T12.b and T12.c, as
      built: the medium read in place".)
    - From above the atmosphere, a sky pixel whose ray meets the ground stores no transmittance
      to space, so the sun's disc is never drawn through the planet.
    - `geodeticOf` is the classical fixed-point iteration on φ and h. On WGS 84 it converges to
      under 10⁻⁸ m. Above a flattening of about 0.04 it fails to converge, so R08 replaces it (with
      Bowring 1976, for instance).
  - _The phase functions_ are evaluated per term in the shaders. `packMedium` encodes each term's
    phase in `Term.scattering.w` (0 none, 1 Rayleigh, 2 Cornette–Shanks) and its g in
    `Term.absorption.w`; a test holds the packing.
  - _The sizes_ follow Design note 16. Two steps a slice on both settings, and the ray-march step
    counts, are this task's choices; T18 revisits them. (For the sky view and the march,
    superseded on 2026-10-06 by R05.T12.e: steps placed toward each ray's lowest point, with
    counts set by its gate.)

    | Table              | High                       | Low                                                    |
    | ------------------ | -------------------------- | ------------------------------------------------------ |
    | Sky view           | 192 × 108, 30 steps (sebh) | 128 × 64, 16 steps                                     |
    | Aerial perspective | 32³, to 32 km              | 32 × 32 × 16                                           |
    | Ray march          | full resolution, 32 steps  | half resolution, 16 steps, with a depth-aware upsample |

    "Aerial perspective on terrain only" is `aerialPerspectiveScope`. On high it is `scene`, which
    publishes the volume for later plans' passes (`aerialPerspectiveVolume()`). On low it is
    `terrain`, which publishes nothing, so the one deferred composite applies it to the terrain
    alone.

  - _The sky view's parameterisation_ is sebh's (`SkyViewLutParamsToUv`): the azimuth is measured
    from the sun's over [0, π], and the horizon split is compressed by a square root. Bevy's is
    world-fixed over 2π. As in sebh, the sky view has no ground bounce. The kernel and the
    composite both clamp the camera's height to 1 m inside the shell (`skyViewHeight`). They form
    r² − R² as h(2R + h) (`groundHitFromHeight`), which would otherwise cancel in `f32` near the
    ground.
  - _The aerial-perspective volume_ stores, per slice, the running in-scattered radiance (rgb) and
    the mean transmittance (a), linear. This is sebh's choice; Bevy stores the logarithm and takes
    transmittance from the transmittance table. The composite fades the first slice in from the
    camera.
  - _Luminance._
    - The sky is scaled by `skyLuminanceScale()` ÷ d².
    - Sunlight that the grey ground reflects into the ray march takes the sun's factors, not the
      sky's (Bruneton 2017, `GetSunAndSkyIlluminance`), through the view's `sunOverSky`.
    - The sun's disc is `solar.ts`'s per-channel illuminance ÷ the disc's solid angle ÷ d². It is
      attenuated by the transmittance to space (the ray march's mean from above the atmosphere)
      and clamped at 65,504 after pre-exposure.
    - `SunState.angularRadiusRad` must be asin(R★ ÷ d), kept consistent with the distance by the
      caller; 0 draws no disc. The smoke check's Sun is 0.0046505 rad (IAU 2015 B3 R⊙ᴺ at 1 au).
  - _The settings._ `SETTINGS[s].atmosphere` is `TABLE_SIZES[s]`, which `qualitySetting.ts`
    imports from `hillaire.ts` (appended).
  - _Memory._
    - `MemoryCategory` gains `atmosphere-view` (appended).
    - The ray-march target grows to the largest output so far and is never remade smaller. The
      engine has no public release of a texture, so a resize past the largest size leaves the
      previous target allocated until the engine is disposed. The kernel and the composite address
      the target by the current output's size.
    - The sky-view and aerial-perspective textures are made once. They are freed only with the
      engine.
  - _The smoke harness._
    - `smoke/atmosphere.ts` gains `checkAtmosphereFrames`. On both settings it renders from 2 m and
      from 400 km, at noon and at the terminator, with every texel finite and within rgba16float,
      and the noon sky from the ground bluer than red. A dark surface 500 m and 5 km away (the
      volume's path) and 60 km away (the ray march's) must take on haze, more at 5 km and at 60 km
      than at 500 m. At 5 km and beyond, the haze has nearly reached the horizon sky's brightness,
      so the two paths are held to agree within 10%; they are not ordered. On SwiftShader on
      2026-10-02, the high setting gave 0.140 at 5 km and 0.139 at 60 km in blue.
    - `just test-render --captures=DIR` renders `HILLAIRE_REFERENCE` on Hillaire's 6,360 km sphere,
      with no sun disc, as his comparison images have none: from the ground at noon and at sunset,
      and from 400 km. It saves the frames as 8-bit PNGs, exposed to a mean of 0.18 and passed
      through R02's AgX. The pieces that carry it:
      - `apps/hyperion/scripts/testRender.sh` takes `--captures=DIR` and passes it to the harness
        as the new `--smoke-captures` switch.
      - The page's report gains an `images` field.
      - `readSmokeImages` in `smoke/result.ts` checks each image's name, size and length. The main
        process logs any image it rejects or fails to save, without losing the judged checks.
  - _Left as found._ At half resolution, a far-surface pixel whose four nearest ray-march texels
    are all sky gets no haze: a one-pixel fringe on silhouettes, on the low setting only.
  - _By hand, for the owner._ The comparison with Hillaire 2020's published images (CGF 39(4), DOI
    10.1111/cgf.14050) confirms the work but blocks nothing (decisions-r05.md item 7). The images
    are kept local and untracked (decisions-r05.md item 4).
  - _The lane's pre-screen_ (2026-10-02). It ran
    `just test-render --captures=<scratchpad>/laneC-captures` on SwiftShader, headless, on both
    variants. The run saved `hillaire-ground-noon`, `hillaire-ground-sunset` and
    `hillaire-orbit` (320 × 180, one set a variant). The lane viewed them and saw:
    - noon: a blue sky, lighter towards the horizon, with an aureole about the sun;
    - sunset: an orange-to-rose band along the horizon, brightest under the sun, under a
      grey-violet sky;
    - orbit: the limb as a thin bright blue band over the lit disc, with black space above.

    Below the horizon the ground is black, as in Hillaire's comparison scene (black ground, no
    disc). Nothing looked wrong side up or discontinuous. The side-by-side with his figures
    remains the owner's.

  - _Aerial-perspective scope_ lives in `TableSizes` for now; R08.T9.a's `AERIAL_PERSPECTIVE_SCOPE`
    takes it over or reads it.
- **Deviations in T12.b and T12.c, as built: the medium read in place** (2026-10-06, lane C;
  option 1 of "R05.T14, the high run's pass timer and atmosphere, diagnosed").
  - _The change._ No WGSL helper takes a `Medium` by value any more. Each reads the `medium`
    uniform that every kernel declares at `@group(0) @binding(0)`. The arithmetic is the old
    one token for token; one line is split, which parses to the same sum.
    - The helpers are `mediumAt`, `distanceToTop`, `distanceToBottom`, `intersectsGround`,
      `maxDistance`, `transmittanceUvToRMu`, `transmittanceRMuToUv`, `multiScatteringUvToRMu`
      and `multiScatteringRMuToUv` (from `common.wgsl`), and `phasedScatteringAt`,
      `tableTransmittance`, `tableMultiScattering` and `sourceAt` (from `view.wgsl`).
    - The kernels' calls only lose their `medium` argument. The densities are still evaluated
      three times a sample (option 2's first item is not taken).
  - _Two chunks not in T12.b's and T12.c's Files lists._
    - `shaders/medium.wgsl` holds the nine helpers from `common.wgsl`.
    - `shaders/source.wgsl` holds `phaseOf` and the four source helpers from `view.wgsl`.
    - Two files are needed because the composite takes `common.wgsl` and `view.wgsl` but
      declares no medium, and WGSL resolves every name in a module, called or not.
  - _The order_ in which each module is joined:
    - transmittance and multiple scattering: `common`, `medium`, then the kernel (`tables.ts`);
    - sky view, aerial perspective and ray march: `common`, `medium`, `view`, `source`, then the
      kernel (`hillaire.ts`);
    - the composite: `common`, `view`, then `composite`, as before.
  - _Supersedes:_
    - T12.b's "`multiScatteringRMuToUv` is in `common.wgsl`": it is now in `medium.wgsl`;
      `shellMultiScatteringRMuToUv` stays in `common.wgsl`.
    - T12.c's "`shaders/view.wgsl` … holds the shared `AtmosphereView`, the source term and the
      sky view's mapping": the source term is now in `source.wgsl`. `view.wgsl` keeps the view's
      uniform, the camera's rays and the sky view's mapping, and reads no medium.
  - _Notices._ The new chunks point to the notices, as the other kernels do: Bevy's MIT and
    Bruneton's BSD-3 notices are in `common.wgsl`, which starts every module, and sebh's is in
    `multiScattering.wgsl`. `NOTICE` names `medium.wgsl` in its Bevy, Bruneton and sebh entries
    and `source.wgsl` in Bevy's.
  - _Test._ `hillaire.test.ts` checks each of the six assembled modules for a `: Medium`
    parameter and for a local copy of the uniform or its terms. It fails on the old shaders.
  - _Bit identity._ A temporary instrument, not committed, hashed raw read-backs in the smoke
    page. Per run it covered 72 buffers:
    - both per-planet tables, for Earth's medium and for Hillaire's;
    - the sky view, the aerial volume, the ray march and the composite of the 14 check frames (7
      cases on each setting) and of the 3 comparison captures.

    Before and after, all 72 hashes are identical on SwiftShader (both smoke variants) and on the
    RTX 3080 (a hidden smoke run on `:0`). All 84 capture PNGs of `just test-render --captures`
    are byte-identical, and the smoke's PASS lines are the same, 498 of them. naga 30.0.1
    (gpu-replay's version and settings) validates the six assembled modules. The composite drops
    from 32 functions to 18.

  - _Timing, high._ Lane D's harness, copied: `target/laneD/t14diag/go.sh` with
    `instrument.py 150` and `instrument2.py`'s dispatch labels. Hidden, seed 7, the first 150 s,
    under `just _locked` in an 8 G scope, `nvidia-smi` at 100 ms. The run (m1, 16:42) started at
    load 4.47, at most 8.51. Figures are after the 10 s warm-up, 8,378 frames, every one timed.
    - The driver held P3 (70% of samples) and P5 (27%): graphics 1,125 MHz p50 (855–1,245 p5–p95),
      memory 5,001 MHz, utilization 26%, 80 W.
    - Lane D's two runs are the comparison:
      - d2, the committed shader. Its load at start was 1.83, and it ran at P0 throughout, at
        1,965 MHz and 99%, since the slow march kept the GPU busy.
      - d3, the march's helpers alone read in place. It ran at P3/P5, the same clocks as m1.

    | ms, p50 / p95      | Before (d2, P0) | March only (d3, P3/P5) | All kernels (m1, P3/P5) |
    | ------------------ | --------------- | ---------------------- | ----------------------- |
    | sky view           | 0.17 / 0.18     | 0.30 / 0.40            | 0.11 / 0.17             |
    | aerial perspective | 0.17 / 0.38     | 0.29 / 0.39            | 0.18 / 0.24             |
    | ray march          | 6.12 / 69.88    | 2.20 / 2.84            | 2.27 / 3.02             |
    | composite          | 0.03 / 0.03     | 0.06 / 0.40            | 0.06 / 0.40             |
    | atmosphere row     | 6.54 / 70.41    | 2.85 / 4.03            | 2.63 / 3.82             |
    - At P0, m1's 220 frames at P0 (in the warm-up, from the same orbit view, after its first
      second): march 1.40 ms p50, row 1.62 ms p50. Their p95s (2.53, 3.39 ms) carry the
      warm-up's clock changes. d3's 150 frames at P0 gave a march of 1.31 ms.
    - rAF after the warm-up: p50 16.70, p95 16.80, p99 16.80 and max 83.4 ms. There is no plateau.
      - Four intervals exceed 40 ms (50–83 ms): two at 101.5 s, one at 137.4 s and one at 142.4 s.
        Each falls within 0.15 s of a P-state change (memory 810 ↔ 5,001 MHz). Their frames' GPU
        times are ordinary (march 2.3–2.8 ms), and our code took under 9 ms. They are the driver's
        power management, not the march.
      - The orbit coast has none. After the warm-up, d3 had none and d2 had 564.
    - The per-planet tables' bake, at frame 0, fell from 90.6 ms (d2) to 1.38 ms. Its two kernels
      took `Medium` by value too, so a medium change no longer costs a 90 ms hitch.
    - The results file's rows (m1): atmosphere 3.82 ms at p95, failing its 1 ms limit; GPU pass
      sum 4.93 ms at p95, within 0.8 T at 60 Hz (13.3 ms); terrain 1.05 ms, passing.

  - _Low, not re-run._ The same harness's low run waited 100 minutes for a quiet window, found
    none, and was stopped. It was `go.sh <name> 5 <minutes> 600 --setting low --hidden`, lane D's
    T14.f invocation. The pending re-run's scripts and build are in
    `.git/rm23-scratch/laneC/t14diag/`.
    - The like-for-like before is the first 150 s of lane D's T14.f full low run: the committed
      shader, load 0.36 at the start, clocks not sampled.
      - "atmosphere view": 2.88 / 3.48 ms p50 / p95. That covered the sky view, the aerial
        perspective and the march, which were then one label, so which one dominated is not known.
      - The composite was 0.11 / 0.15 ms, and the row 3.00 / 3.60 ms against low's 4 ms.
      - The tables' bake was 92.9 ms.
    - Low runs the same kernels on a quarter of the pixels and half the steps.
  - _High is still over its 1 ms row._ The row is 2.63 / 3.82 ms p50 / p95 at the driver's
    light-load clocks and about 1.6 ms p50 at P0 (lane D's estimate was 1.6 ms). The march is
    85% of it. As the task's brief directs, nothing more is optimised here. The options, with
    estimates against the march's 1.4 ms and the row's 1.6 ms at P0 (none measured):
    1. _Each term's density once a sample, not three times._ This drops `sourceAt`'s second
       `mediumAt` and `phasedScatteringAt`'s densities.
       - By an operation count it saves about a quarter of the march's transcendental and
         reciprocal work (12 of about 44 operations a sample), and none of its 8 texel reads.
       - Estimate: 0.3–0.4 ms. The row would be about 1.2–1.3 ms, not enough alone.
       - It is not bit-identical. `mediumAt` takes the density at (R_b + h) − R_b, which rounds h
         to f32's 0.5 m spacing at Earth's radius, while `phasedScatteringAt` takes it at h. A
         shared density moves the output by at most about 2 × 10⁻⁴ (0.25 m against the aerosol's
         1.2 km scale height), well within the tests' tolerances.
    2. _Fewer samples._ Past a small per-pixel setup, the march's cost is linear in its 32 steps.
       - 16 steps would save about 45% (0.6 ms), bringing the row to about 0.95 ms.
       - Quality: the rays from orbit along the limb are the long ones (up to about 2,300 km
         inside the shell). The 60 km haze check and the orbit capture would need re-checking.
       - T12.c's sizes already leave the step counts to T18.
    3. _Half resolution, as low does._ A quarter of the pixels.
       - Estimate: the march falls to about 0.35 ms, plus the composite's depth-aware upsample
         (low's composite is 0.11 ms p50). The row would be about 0.6–0.7 ms.
       - Quality: it softens the limb and the terminator seen from orbit, which high's
         full-resolution march exists to keep.
    4. _T18 re-rules the row from these figures._
       - The 1 ms is the brainstorm's estimate ("an estimate, not a measurement"). The brainstorm
         cites Hillaire's 0.5 ms on a GTX 1080 at 720p for his tables with the per-pixel march.
       - This canvas has 1.2 times 720p's pixels, and the march takes 32 steps.
       - The frame's GPU pass sum is 4.93 ms at p95, inside its 0.8 T row.
       - The caveat about the driver's clocks (above) applies to whichever row T18 sets.

    _Ruled_ (2026-10-06, `decision-r05-high-atmosphere.md`): option 1 is taken in R05.T12.e, with
    the steps placed toward each ray's lowest point. Options 2 and 3 are not taken on high: 16 even
    steps err by up to 27% on the disc and 80% at the limb, and low's upsample steps 18–21% every
    other pixel on the limb. Option 4 is decided now, not in T18: Design note 21 judges terrain and
    atmosphere together, against 6 ms.

  - _T17:_ the plateaus are gone, so its high runs no longer measure a known failure of the frame
    rows. Ruled (2026-10-06): T17's runs wait for R05.T12.e and T14.l, and judge terrain and
    atmosphere together against 6 ms.
  - R08.T6.b, which moves the terms into a storage buffer, edits these two chunks as well as the
    kernels it names.
- **The per-frame marches under-sample the dense air** (found and ruled 2026-10-06,
  `decision-r05-high-atmosphere.md`).
  - _Found._ The sky view and the ray march space their steps evenly: Bevy's linear placement, at
    sebh's sky-view count of 30 and the march's 32. sebh's own code places them quadratically from
    the ray's start. A ray's dense end (the camera, the ground or a limb's tangent point) is
    therefore under-sampled.
    - The aerosol's 1.2 km scale height is the finest structure. A 100 km vertical ray's 3.1–3.3 km
      steps undercount its column by about 24%: the midpoint rule's (Δh/H)² ÷ 24, as for
      `TRANSMITTANCE_SAMPLES`.
    - An `f64` model of the two kernels' quadrature (single scattering and a 0.15 ground, against
      4,096 placed steps, with e floored at 10⁻³ of the brightest pixel) puts the as-built error at:
      - the sky from the ground, up to 28% (19% at the zenith with the sun at 30°);
      - the disc from 400 km, up to 7%;
      - the limb, up to 17% at the terminator;
      - terrain beyond 32 km from inside, about 1%.
    - Sixteen even steps: 82%, 27%, 80% and 6%. sebh's 14 and Bevy's 16 per-pixel steps are
      real-time defaults, not accuracy references.
  - _Ruled._ R05.T12.e places the steps quadratically toward each ray's lowest point, at the same
    counts. In the model that gives:
    - at most 1.3% in the sky (2.4% at a sunset horizon);
    - 0.6% on the disc;
    - 1.6% on the daylit limb (even steps: 0.6%), 4.6% at the terminator limb;
    - 0.3% inside.

    Its gate is 2%, and 5% for grazing twilight rays. Low's 16 steps improve too: from 27/80/6/82%
    to 5/18/1/8%.

  - _The multiple-scattering kernel's_ 32 even steps have the same form. It is a second-order term
    built once per medium; R08.T6.a's twin, which gives each kernel in `f64`, measures it against a
    placed reference.
  - _Half resolution, not taken on high._ Low's upsample takes the nearest-depth texel, so on the
    limb from orbit every other pixel reads a texel 1.7 km of tangent height away, where the
    radiance e-folds in 9–10 km: a step of 18–21%.
  - _Addendum A (2026-10-06)._
    - _The band._ The gate first lacked the band between the local and the visible horizon. There a
      ray from a camera inside the atmosphere falls to a lowest point below the camera, then climbs
      out. Q gave the camera side 1–4 of 30 steps, its longest at the camera, where the view's
      attenuation puts the light. At 30 steps that cost 11–13% (ordinary) and about 80% (twilight),
      worse than even steps' 5% and 30%.
    - _V1._ Lane C's V1 places that side toward the camera and shares steps by the square root of
      each side's length: 1.3% and 4.1% at 30 steps (the decision's model; the lane's 300 rays,
      1.35% and 3.46%). One-sided rays and rays from orbit are unchanged.
    - _Two more families_ joined the gate: rays just past the visible horizon, and the march from
      0.5–5 km to terrain at 40–150 km. Q passes high on both (3.7% and 2.4% at twilight), though
      even steps do better there.
    - _Low's known trades._ At low's 16 steps, the placement is worse than even steps on three
      families:
      - the daylit limb, 4.8% against 1.6%;
      - near-level twilight march rays from low cameras, 9.1% against 2.5%;
      - twilight rays just past the visible horizon, 17.4% against 3.5%.

      Elsewhere it improves low's worst from 27–96% to 5–18%. No placement tried (Q, P, the
      exponents, or half the steps toward each end) is better on both sides.

    - _The f32 cancellation._ The 1,024-step reference also exposed an f32 cancellation in (S − S·T)
      ÷ σ_t at tiny optical depths, up to 15% on SwiftShader. The per-frame kernels take a stable
      step factor (T12.e), within 2 × 10⁻⁵, and the multiple-scattering kernel takes it in R08.T6.a.
  - _Addendum B (2026-10-06)._
    - _The gap._ The sky view serves cameras up to 100 km, and the gate first stopped at 50 km. From
      60–100 km, a limb ray falls far below a camera in thin air.
    - _V1 there._ V1 puts its long steps at the ray's narrow source at t\*: 8.1% at twilight at 30
      steps (lane C's 72 rays; the decision's model agrees). The band from 60–100 km cameras fails
      at twilight under every placement at 30: Q 24%, V1 9%.
    - _The fix._ The high sky view's count rises from 30 to 75 under V1, for a worst of 0.40%
      ordinary and 3.20% twilight over both families. The pass's p95 goes from about 0.17 to about
      0.43 ms at light-load clocks, inside the joint row.
    - _Not taken:_
      - **switches between V1 and Q.** With density alone they fail at twilight. With the sun's
        transmittance (Ws) they pass at 45, but a hard switch risks a seam in the twilight sky; Ws
        is kept as the lever if the sky view's cost must fall;
      - **importance by the view's opacity,** which fails twilight;
      - **a split at the shadow's edge,** left to R08.T8;
      - **counts by the camera's height,** which would save at most 0.26 ms.
    - _The fallback,_ if the sky view's p95 exceeds 2.6 times its before: the camera side split at
      its midpoint, with half its steps toward each end (E), at 60 steps (worst 2.71%).
    - _Open for R08.T8._ The 192 × 108 table's angular resolution across the limb, seen from 60–100
      km, is unmeasured.
- **The atmosphere's budget estimate is contradicted (a brainstorm finding, for T19 and R12).**
  - _The measurement._ The brainstorm's 0.5–1 ms for the discrete column, for an RTX 4060-class
    part, compares with:
    - about 1.6 ms p50 at full clock on the RTX 3080, about twice that class's arithmetic;
    - 2.6 / 3.8 ms p50 / p95 at the driver's light-load clocks.
  - _The conditions:_ the spike's 1,398 × 793 px view, one sun, three terms, 32 steps.
  - _It will grow._ A full 1080p view has 1.9 times those pixels, and R08 adds terms and suns.
  - _How it is handled._ Design note 21 judges terrain and atmosphere together, against the 6 ms the
    brainstorm leaves them (terrain 1.05 ms p95 at the same clocks). The atmosphere's figure against
    its estimate is a finding: T19 drafts it for the owner, R08.T11 records it, and R12.T10 replaces
    the estimate.
- **Deviations in T12.e, as built** (2026-10-06, lane C; `decision-r05-high-atmosphere.md` and its
  addenda A and B).
  - _The change._
    - `source.wgsl`'s `sampleMediumAt(heightM, cosTheta)` evaluates each term's density once a
      sample, at the sample's height above the datum, and returns `SampleMedium`: the scattering,
      the extinction and the phased scattering. `sourceAt` takes it as `local`.
      `phasedScatteringAt` is gone, and `mediumAt` stays for the per-planet tables. The helper list
      in "Deviations in T12.b and T12.c, as built: the medium read in place" predates this.
    - `source.wgsl`'s `marchSplit(tStartM, tEndM, nearestM, samples, fromCamera)` and
      `marchStep(split, i)` hold V1's placement, twinned by `marchSteps.ts`. A step's midpoint and
      length are in closed form, with no `pow` or `sqrt` a step and two `sqrt` a ray. The sky view
      passes `fromCamera` as true, since it serves only cameras inside the atmosphere. The march
      passes `shell.x <= 0`.
    - `common.wgsl`'s `stepFactor(x)` is the step factor g. The three per-frame kernels add
      `throughput · source · dt · stepFactor(σ_t dt)` and no longer divide by a floored σ_t.
    - `TABLE_SIZES.high.skyViewSamples` is 75 (addendum B). Low keeps 16 and 16, and the march 32
      on high.
    - The kernels take at least 2 steps (`max(…, 2u)`), since each non-empty side needs one.
      `marchSplit` refuses fewer. No shipped count is affected.
    - The aerial perspective takes the densities once and the step factor. Its slices keep their
      equal depths.
  - _Outside the Files list:_
    - `common.wgsl`, for the step factor (addendum A);
    - `HillaireAtmosphere.frameTables`, a read-only getter of the latest frame's sky-view table and
      ray-march target for the smoke page, with its test in `hillaire.test.ts`;
    - the check's group in `smoke/page.ts`.
  - _The twin_ (`marchSteps.test.ts`, about 10–17 s under the lanes' load).
    - _The sun's transmittance_ is `opticalDepth.ts`'s integrator at 64 Simpson intervals,
      tabulated once on a 512 × 128 grid of the transmittance table's (u, v) and read bilinearly.
      Every scheme and the reference read the same table. Integrating each sample's sun ray afresh
      instead, over the same 1,046 rays, passes every test as well. It moves no ray's e at high's
      counts by more than 0.23 percentage points, and at low's by more than 3.9. Both are twilight
      rays whose sun is on the horizon, where the grid's bilinear read is coarsest. It takes seven
      times as long (129 s against 17 s).
    - _L_max_ is each kernel's brightest reference pixel per channel, as in the decision's model.
    - _The reference_ is 4,096 placed steps, confirmed against 8,192 to 0.05%. Two twilight rays
      cross the ground's hard shadow edge, where the midpoint rule converges only as 1 ÷ n, and miss
      that by a little. Each takes 8,192 steps, confirmed against 16,384:
      - the band from 1 km at 0.98 of the dip, the sun at 90°/180°;
      - the band from 20 km at 0.98 of the dip, the sun at 95°/0°.

      The worst over the set is then 2.3 × 10⁻⁴.

    - _The rays:_ 1,046, all gated:
      - 40 disc, 30 limb, 20 inside and 108 near-level for the march;
      - 120 sky, 336 band, 224 past the visible horizon, 72 high limb and 96 high band for the sky
        view.

      The band, past and high-band rays take the addenda's suns, 30/80/90/95° × 0/180° at the
      camera. The near-level rays take the ruling's, 30/80/90/95° × 0/90/180°, at the terrain.

    - _Twilight_ is judged at t\* with the 10⁻¹² rad margin, applied to the angles (addendum A,
      4(c)). Against the ruling's list (every limb ray, sky rays beyond 80° of view zenith and disc
      rays beyond 80° of ground zenith, each with the sun more than 80° away):
      - 135 rays are twilight at t\* and not on the list, all with the sun at 90° or 95°;
      - the worst of them at high's counts is 2.43%, a near-level march ray from 2 km to terrain
        150 km away;
      - addendum A's own figures count that family's twilight rays so.
  - _The gate's figures._ Worst e, %, ordinary / twilight. The columns are:
    - High: V1, the march at 32 and the sky view at 75;
    - V1 at 30: the sky view at the ruling's count;
    - As built: even steps at 32 / 30;
    - Low: V1 at 16 / 16, against even steps at 16 / 16.

    | Family (rays)                  | High        | V1 at 30    | As built      | Low           | Even at 16    |
    | ------------------------------ | ----------- | ----------- | ------------- | ------------- | ------------- |
    | Disc (40)                      | 0.55 / 0.11 | 0.55 / 0.11 | 6.93 / 0.86   | 2.66 / 2.95   | 26.96 / 6.84  |
    | Limb (30)                      | 1.65 / 4.58 | 1.65 / 4.58 | 0.57 / 16.85  | 4.84 / 18.15  | 1.56 / 78.90  |
    | Inside (20)                    | 0.28 / 0.04 | 0.28 / 0.04 | 1.01 / 0.10   | 1.17 / 0.17   | 5.54 / 0.37   |
    | Near-level march (108)         | 0.42 / 2.43 | 0.42 / 2.43 | 0.12 / 0.66   | 1.63 / 9.27   | 0.48 / 2.64   |
    | Sky (120)                      | 0.22 / 0.38 | 1.35 / 2.34 | 27.84 / 15.75 | 4.70 / 8.44   | 81.68 / 71.63 |
    | Band (336)                     | 0.24 / 0.57 | 1.30 / 4.11 | 5.57 / 33.28  | 9.04 / 15.16  | 14.38 / 95.23 |
    | Past the visible horizon (224) | 0.04 / 0.59 | 0.28 / 3.70 | 1.48 / 0.99   | 0.81 / 17.50  | 5.14 / 3.52   |
    | High limb (72)                 | 0.40 / 3.19 | 2.10 / 8.19 | 0.77 / 18.31  | 11.54 / 49.48 | 3.40 / 74.33  |
    | High band (96)                 | 0.19 / 2.66 | 1.91 / 8.96 | 1.22 / 3.84   | 9.06 / 39.01  | 2.26 / 24.42  |
    - High passes every ray. The as-built even steps fail on the disc, at the limb and in the
      sky, and the test checks that they do.
    - Low's worst over the set is 49.48% (the high limb at its terminator), against even 16's
      95.23% (addendum A, 4(a), with addendum B's families).
    - _Low's known trades,_ where even steps do better:
      - the daylit limb, 4.84% against 1.56%;
      - the near-level march, 1.63% against 0.48% (ordinary) and 9.27% against 2.64% (twilight);
      - twilight rays just past the visible horizon, 17.50% against 3.52%;
      - the high limb, ordinary, 11.54% against 3.40%;
      - the high band, 9.06% against 2.26% (ordinary) and 39.01% against 24.42% (twilight).
    - _Low's bounds._ Low's worst per kernel, class and camera range, rounded up, are
      `LOW_TWIN_WORST`:
      - the march, 4.9% and 18.2%;
      - the sky view from cameras up to 50 km, 9.1% and 17.5%;
      - the sky view from 60–100 km, 11.6% and 49.5%.

      The twin holds itself within them, and low's GPU check holds low to them plus 1%. Splitting
      the sky view by camera range goes beyond addendum A's 4(b), which is per kernel and class: it
      keeps the high cameras' figures from loosening low's bound from below 50 km to about 50%. It
      only ever tightens.

  - _The GPU agreement check_ (`checkAtmosphereSteps`; `just test-render`, both variants, with the
    same figures on each).
    - _What it compares._ Each kernel at its setting's counts against the same kernel at 1,024
      steps, in the same run, texel by texel. The sky-view table is at a quarter of each dimension
      (48 × 27 on high, 32 × 16 on low), each texel still one ray of the shipped kernel. The ray
      march covers the 64 × 32 check frame (32 × 16 on low).
    - _The classes._ A CPU copy of each texel's ray sets its class: the sky view's own sphere for
      the sky view, and spheres of radius a for the march. Only rgb is compared.
    - _The frames._
      - The sky view is held from 2 m and 5 km, and from 80 km at the limb with the sun on its
        horizon (addendum B). The 5 km frame lets the sky-view table span the band.
      - The march is held from 400 km and over a surface 60 km ahead. The surface frame marches from
        inside, beyond the aerial-perspective volume.
      - An all-dark converged output fails.
    - _Results._ Each row is the texel nearest its tolerance (its e ÷ tolerance the largest), and
      its tolerance:

      | Frame                                        | High                 | Low                      |
      | -------------------------------------------- | -------------------- | ------------------------ |
      | Sky view, 2 m, noon                          | 0.28% (3%)           | 4.34% (10.1%)            |
      | Sky view, 2 m, sunset                        | 0.19% (3%)           | 6.92% (18.5%, twilight)  |
      | Sky view, 5 km, sunset                       | 0.21% (6%, twilight) | 3.26% (18.5%, twilight)  |
      | Sky view, 80 km, the limb with its sun's set | 1.69% (6%, twilight) | 28.10% (50.5%, twilight) |
      | Ray march, 400 km, noon                      | 2.00% (3%)           | 0.99% (5.9%)             |
      | Ray march, 400 km, terminator                | 2.03% (3%)           | 2.44% (5.9%)             |
      | Ray march, 2 m, a surface 60 km ahead        | 0.64% (3%)           | 2.33% (5.9%)             |

    - _The f32 cancellation it found_ (addendum A). Before the step factor, the 1,024-step
      reference disagreed by up to 15%: 6.6% on the 400 km disc and limb, and 14.8% and 25.9% on
      the sky view at the top. An f32 emulation gives −13% on a 90 km tangent ray at 1,024 steps,
      against 0.02% at 32.
  - _T12.c's comparison frames_ were re-rendered on SwiftShader at 75 sky-view steps, in both
    variants, for the owner's pending look. They are in
    `.git/rm23-orchestration/laneC-captures-t12e/`, beside the earlier `laneC-captures/`. Hillaire's
    medium is clean, so they change little.
  - _The image check_ (step 6; 2026-10-06, hidden on the RTX 3080, under the GPU lock).
    - _How._ A temporary instrument (`t12e/imagecheck.py`, never committed) renders each frame
      again under variants and logs e per atmosphere pixel as page warnings. A pixel counts where a
      channel of the 1,024-step composite exceeds 10⁻³ of the frame's brightest, and e is floored
      at the same.
    - _The frames:_ the 14 check frames (7 cases on each setting), addendum B's frame from 80 km at
      the limb's sunset on each setting, and the 3 comparison captures (high's sizes, Hillaire's
      medium).
    - _What is compared._ Each variant through the composite against the same kernels at 1,024
      steps. 2,048 steps confirm the reference: p99 0.00% and worst 0.45% on high.
    - _High passes._ Over its 11 frames (119,586 pixels), the shipped kernels give p50 0.08%, p99
      0.32% and max 2.09% (the orbit capture). The gate is p99 ≤ 1% and max ≤ 5%. Per frame the
      worst are the 400 km terminator (2.02%) and the 80 km limb (1.01%).
    - _For the record_ (high's frames):

      | Variant                                 | p99   | Max    |
      | --------------------------------------- | ----- | ------ |
      | Shipped (V1, 32 / 75)                   | 0.32% | 2.09%  |
      | As built (28479b9's kernels at 32 / 30) | 16.8% | 20.2%  |
      | Even 16 (28479b9's kernels at 16 / 16)  | 41.6% | 83.9%  |
      | Half resolution through low's upsample  | 79.2% | 3,569% |

      Half resolution's worst pixels are at silhouettes, where the nearest-depth texel belongs to
      another surface. That closes options 2 and 3 on measured images.

    - _Low_ is recorded, not gated, since low's only gate is the twin's. Shipped: p99 4.03% and max
      17.4% (the 80 km limb). As built: 51.5% and 83.7%.
    - _The run's other checks._ One FAIL: R07.T8.a's "a photorealistic frame tones a lit planet
      onto the canvas over a black sky" (`smoke/bodies.ts`). It touches no atmosphere code, and it
      failed the same way on lane C's RTX runs at 0c57be5, before and after.
  - _Option 1's largest difference_ (the same run).
    - _The variant._ 28479b9's even steps and step form, with the densities once, against 28479b9's
      kernels as built.
    - _The outputs:_ the sky view, the aerial perspective, the ray march and the composite of the 19
      frames.
    - _The result._ 37 of 4.7 million stored values differ, each by one rgba16float step. The
      largest e is 9.7 × 10⁻⁴, one half-float step. That fits the ruling's "at most about
      2 × 10⁻⁴" before the output's rounding.
  - _The timing_ (step 7; 2026-10-07, 00:46–00:57, in one window the orchestrator cleared).
    - _The runs._ Lane C's m1 protocol: hidden, seed 7, the first 150 s, under `just _locked` and,
      inside it, the NVIDIA-GPU lock, with `nvidia-smi` at 100 ms. Each run passed the quiet gate
      inside both locks, at a load of 2.0–2.7 (at most 3.2 during a run) with 28–29 GB free. The
      telemetry sampler (`crash-telemetry`, every 2 s) was active throughout.
    - _The builds._ Before is 28479b9's instrumented build (`laneC/t14diag/out-instrumented/`).
      After is the head's (12023f3), with lane D's two instruments (`t12e/out-after/`). The order
      was high and low before, then high and low after. Figures are after the 10 s warm-up, from
      `t12e/timing/analyse2.py`.

    | ms, p50 / p95        | High, before  | High, after   | Low, before   | Low, after    |
    | -------------------- | ------------- | ------------- | ------------- | ------------- |
    | Sky view             | 0.111 / 0.168 | 0.200 / 0.230 | 0.284 / 0.302 | 0.205 / 0.213 |
    | Aerial perspective   | 0.186 / 0.244 | 0.118 / 0.139 | 1.436 / 1.554 | 0.325 / 0.433 |
    | Ray march            | 2.299 / 3.072 | 1.900 / 2.582 | 0.628 / 1.921 | 0.514 / 1.162 |
    | Composite            | 0.061 / 0.402 | 0.398 / 0.402 | 0.122 / 0.208 | 0.124 / 0.226 |
    | Atmosphere row       | 2.657 / 3.865 | 2.619 / 3.370 | 2.475 / 2.879 | 1.179 / 2.205 |
    | Terrain              | 0.336 / 1.081 | 0.992 / 1.183 | 0.379 / 0.431 | 0.398 / 0.465 |
    | Terrain + atmosphere | 2.989 / 4.894 | 3.622 / 4.395 | 2.860 / 3.262 | 1.578 / 2.681 |
    | rAF p99, ms          | 16.80         | 16.80         | 33.30         | 33.30         |
    | Intervals over 40 ms | 0             | 0             | 3             | 0             |
    - _The clocks_ (graphics / memory, MHz, p50):
      - high before: P3 68%, P5 29%, 1,110 / 5,001;
      - high after: P3 26%, P5 71%, 1,065 / 810;
      - low: P8 93% before and 96% after, 255 / 405 before and 225 / 405 after.

      The driver chose the clocks, as Design note 21 has it. High's after ran mostly at P5's 810
      MHz memory clock. That, not T12.e, is the likely cause of its slower terrain (p50 0.34 → 0.99
      ms) and composite (0.06 → 0.40 ms), which this task did not touch. The after build also
      carries the lanes' work merged since 28479b9.

    - _The bounds._
      - **(a) The march at matched clocks: met, 18% faster.** In the 1,000–1,399 MHz bin, before
        is 2.294 ms p50 (7,245 frames, 1,110 MHz) and after 1.875 ms (6,598 frames, 1,080 MHz).
        Neither run has 100 frames at ≥ 1,800 MHz. On low every frame was below 1,000 MHz:
        0.628 → 0.514 ms at 255 → 225 MHz.
      - **The sky view at matched clocks: met.** Its p95 in the 1,000–1,399 MHz bin goes from 0.168
        to 0.232 ms, 1.38 times its before against addendum B's 2.6. Its p50 goes from 0.111 to
        0.198 ms, 1.78 times, for 2.5 times the steps. Addendum B's fallbacks are not needed.
      - **(b) Terrain + atmosphere: met.** 4.40 ms p95 on high (limit 6 ms) and 2.68 ms on low
        (limit 18 ms).
      - **(c) No plateau: met on high; low's p99 as before.**
        - High: rAF p99 16.80 ms before and after, and no interval over 40 ms.
        - Low: rAF p99 33.30 ms before and after. Its long intervals are single missed vsyncs (1.65%
          before and 1.84% after), spread evenly through the run at about 10 every 10 s. The GPU's
          pass sum in those frames is 1.8–3.1 ms p50.
        - Lane D's six hidden low runs of T14.f gave the same p99 of 33.30 ms. So this is how the
          hidden low run paces at P8, not a plateau and not this task's. Low's three intervals
          over 40 ms before, none near a P-state change, are gone after.
        - The literal "rAF p99 ≤ 16.8 ms" is therefore not met on low, before or after. It is
          recorded as a finding, not acted on.
      - **(d) Low's atmosphere row: met.** 1.18 / 2.21 ms p50 / p95, against its 3.00 / 3.60 before
        (2.48 / 2.88 in this window).
    - _High's atmosphere row against its 1 ms estimate_ (a finding for T19 and R12): 2.62 / 3.37
      ms p50 / p95 at the driver's clocks, against 2.66 / 3.87 before. The march's 18% saving pays
      for the sky view's 2.5 times the steps.
- **Deviations in T11.a, as built** (2026-10-02 and 2026-10-03).
  - _Device limits_ (decisions-r06-r07.md item 7). `createWebGpuEngine` requests
    `requiredLimits(adapter, overrides)` (`platform.ts`): the adapter's
    `maxStorageBufferBindingSize` and `maxBufferSize`, never above what it reports, capped at
    `MAX_REQUESTED_BUFFER_BYTES`, 1 GiB.
    - `GpuCapabilities` gains both limits. They are read from the device, as its features are,
      so a rebuild after a device loss reports the rebuilt device's.
    - `CapabilityOverrides.defaultLimits` raises nothing. It is how the harness runs the
      `FaceDifferences` fallback once T11.a's layout exists.
    - `FakeAdapter` takes both limits. Its devices get WebGPU's defaults unless more is required,
      and reject a request beyond the adapter's.
    - `GraphicsPanel` leaves the two limits out of its feature list.
    - The RTX 3080's adapter limits, read in a hidden run on 2026-10-03 (an offscreen window
      never shown, R01's Vulkan switches; NVIDIA, Ampere, driver 615.71.09):
      `maxStorageBufferBindingSize` 2,147,483,644, `maxBufferSize` 4,294,967,292,
      `maxTextureDimension2D` 16,384, `maxTextureArrayLayers` 2,048. The engine asks for 1 GiB of
      each buffer limit, so the high setting keeps `BakedOffsets` there. Headless Ozone could not
      be used: with a hardware adapter its GPU process exits (SIGSEGV), as R01 Design note 17 found
      for windows without offscreen rendering.
  - _`AllocationTally`_ (`view/terrain/gpu/allocationTally.ts`) adds `startFrame()`, which zeroes
    the frame's upload count; the interface has no other notion of a frame. It also adds
    `dispose()`, which stops listening.
  - _The counting fake_ (`test/countingRenderEngine.ts`) landed with T12.b. It stands alone,
    implementing `RenderEngine` and delegating views and faults to R01's `FakeRenderEngine`,
    because that class's members return `never` and cannot be overridden. It also records
    `textureSpecs`, `dispatched`, `writes` and `targetFrames`, and offers `restore()` and
    `destroy()`.
  - `MemoryCategory` gains `height-cache`, appended.
  - _The layout_ (`gpu/resources.ts`, `terrainLayout(terrain, limits)`, `TerrainLayout`) reads T8's
    `SlotLayout` and the device's limits from `engine.capabilities`. Where the `BakedOffsets`
    offsets at the slot count exceed min(`maxStorageBufferBindingSize`, `maxBufferSize`), it
    takes `FaceDifferences` over the same byte budget, and `TerrainLayout.fallback` is
    `binding-limit`, for T13 and T14 to put in the results file and the label. On the high setting
    under default limits that is 3,904 slots, against 1,962 with `BakedOffsets`; the heights then
    take 132.0 MB of the 128 MiB binding (98%). Item 7's "keeping the slot budget" is read as the
    byte budget (the orchestrator's approval, 2026-10-03; if the slot count was meant, the
    fallback holds 1,962 slots and one atlas layer). A layout whose buffers or atlas still do not
    fit throws.
  - _Per-slot records_ (`uniforms.ts`, `SLOT_RECORD_BYTES` 112, `writeSlotRecord`, `patchTerms`).
    A storage buffer of one record a slot holds `PatchTermsF32`'s fields, the skirt depth and
    `straddles`, written with the slot, in `height-cache`. It sits outside `SlotLayout`'s budget
    (437 kB at 3,904 slots; approved by the orchestrator) and the tally counts it. `patchTerms` is
    the client's twin of `PatchTerms::new`; a test holds its narrowed record to `vertex_f32.golden`'s
    terms bit for bit on all twelve patches. Its private `stToUv` yields to T2's mirror.
  - _The normals atlas_ (`normalsAtlasLayout`, `atlasTile`) is `rg16float`, each tile with a
    one-texel gutter repeating its edge sample, and has as many 2D array layers as
    `maxTextureDimension2D` requires (at most `MAX_TEXTURE_ARRAY_LAYERS`, WebGPU's 256, now in
    `platform.ts`), the tiles spread evenly over them. At 8,192 texels: low 1,296 tiles of 67² in
    8,174 × 737 × 1; high 1,962 of 131² in 8,122 × 4,192 × 1; the fallback's 3,904 in
    8,122 × 4,192 × 2. The engine does not raise `maxTextureDimension2D`, so the RTX 3080 has the
    same atlas. T11.b declares it `texture_2d_array`; R01's `drawing.ts` gains
    `viewDimensionBinds`, so that a `2d-array` binding also takes a single-layer 2D texture (a
    view WebGPU allows; approved by the orchestrator, pointer in R01's Risks). This replaces the
    task's `viewDimension` `"2d"`.
  - _The mesh_ (`patchMeshData`, `GRID_VERTICES`, `SKIRT_VERTICES`, `PATCH_INDICES`): `position`
    carries (x, y, skirt), grid vertex (x, y) at 65 y + x (the bake's order), then 4 × 65 skirt
    vertices, edge e anticlockwise from y = 0; quads split (0, 0)–(1, 1), anticlockwise seen from
    outside, as are the skirts' quads (p, p′, q′), (p, q′, q).
  - _The per-frame buffers_ (`InstanceRecords`, `ContactRecords`, `writeFrame`): instance records
    of 32 B (origin less camera, slot, morph start and end), the instance buffer sized to the slot
    count; a contacts buffer of a 16 B header (the count) and 32 B a contact (centre less camera,
    held radius r_g, ramp), `MAX_CONTACTS` 1,024, more throws; the indirect arguments written once
    with the index count, then only the instance count each frame. These three are category
    `other`. `bytes()` keeps one view a record count, so a frame allocates nothing once its count
    has been seen.
  - _The upload_ (`SlotUpload`, `upload` → `SlotUploadResult`): it takes `originHeightM` and
    `skirtDepthM` as its own fields until T10.b's `BakedPatch` carries them. Everything is checked
    before the first write. A bake that predates the layout (a slot past a rebuilt layout's count,
    or offsets the vertex path no longer takes) is `refused` with nothing written; an array of
    the wrong length throws.
  - _After a device loss_ `TerrainResources` remakes everything from the rebuilt device's limits
    and `onRebuilt(layout)` tells the cache that every slot is empty. The engine has no public
    release of a buffer or texture, so the handles live until the engine is disposed, as T12.c's
    textures do; a setting change (T11.c) makes new ones beside them.
  - _The counting fake_ refuses with `LimitExceeded` a buffer above `maxBufferSize`, a storage
    buffer above `maxStorageBufferBindingSize`, or a 2D texture above `maxTextureDimension2D` or
    256 layers, where a device would raise a validation error. It records `textureWritten`, takes
    buffer limits (`countingRenderEngine(limits)`, `fakeDevice(limits)`) and restores onto
    another device (`restore(device)`).
  - _The smoke check_ `smoke/terrain.ts` (group "R05.T11.a the terrain's resources", appended to
    `page.ts`) makes and writes both settings' resources on the harness's engine, then the high
    setting on a second engine with `defaultLimits`: `FaceDifferences`, 3,904 slots, 8,122 ×
    4,192 × 2. It passed on both variants on SwiftShader on 2026-10-03, with no uncaptured GPU
    error. T11.b adds its frames to the same file.
- **Deviations in T11.b, as built** (2026-10-03).
  - _Three sources, two materials_ (`gpu/material.ts`, `terrainMaterialSpec`, `TERRAIN_MATERIALS`,
    `TERRAIN_BINDINGS`, `TERRAIN_PASS_LABEL`). `shaders/terrain.wgsl` holds the shared stages, the
    `FaceDifferences` formula, the morph and hold, the skirts and the fragment stage;
    `terrainBakedOffsets.wgsl` and `terrainFaceDifferences.wgsl` each define `ownOffset` and
    `morphOffset`, the first also binding the offsets buffer. Each material is `frame.wgsl` +
    `terrain.wgsl` + its path, by concatenation: `TERRAIN` (`face-differences`) and
    `TERRAIN OFFSETS` (`baked-offsets`), both in `WGSL_CATALOGUE`. Bindings at `@group(2)`: heights
    0, slot records 1, instances 2, contacts 3, normals 4 (`texture_2d_array`), offsets 5. The
    `Draw` uniforms after `offsetFromCameraM` (zero): `bodyRotation` (the body-fixed axes into the
    instance origins' frame), `sunDirection` (body-fixed), `sunRadiance` (albedo ÷ π × the sun's
    illuminance per channel × the pre-exposure, which T11.c fills) and `atlas` (columns, tiles a
    layer, a tile's stored texels, samples a side).
  - _Split by R07.T9 (2026-10-05)._ `SlotRecord` and the `FaceDifferences` arithmetic moved,
    unchanged, from `terrain.wgsl` to `shaders/patchVertex.wgsl`, which each material composes
    after `frame.wgsl` and which R07's smooth figure (`view/shaders/smoothMesh.wgsl`) shares.
  - _The morph_ is CDLOD's factor on the unmorphed vertex's distance from the camera over the
    instance's morph range (0 where the range is empty), computed on the GPU in `f32` from the
    instance origin plus the rotated own offset; shared vertices agree to the rounding of two
    `f32` sums, not bit for bit, and the skirts cover the rest. It is then min'd with each contact's
    hold, clamp((|v − c| − r_g) ÷ ramp, 0, 1) (a step at r_g when the ramp is 0), which T7's
    `morphHold` and forced region must match. Contact centres are camera-relative in the instance
    origins' frame (the body's rotated axes), not body-fixed; T11.c writes them so. Every vertex
    loops over every contact, so T11.c should pass only the contacts near the drawn patches. A skirt vertex takes the morphed offset less the
    skirt depth along the spheroid's normal at its edge vertex, ν₀ + (ν − ν₀), on both paths.
  - _The normals are filtered by hand._ The fragment stage loads the four nearest samples
    (`textureLoad`), decodes each as the bake's `decode_octahedral` does and blends the vectors
    bilinearly. The pairs are discontinuous across the lower hemisphere's fold, along x = 0 and
    y = 0 for z < 0 (four southern half-meridians of the body-fixed frame), where hardware
    filtering of the pairs would decode to a wrong normal. So no sampler is bound, and the
    atlas's gutter is unused by this pass.
  - _Output._ Lambertian, `sunRadiance × max(n · s, 0)`, clamped at 65,504; alpha is
    `METER_CLASS.litBody` (2).
  - _The emulation_ (`gpu/vertexEmulation.ts`: `SlotTermsF32`, `faceDifferencePositionF32`,
    `faceDifferenceMorphF32`, `directionDifferenceF32`, `normalDifferenceF32`) rounds each
    operation with `Math.fround`. It agrees with `vertex_f32.golden`'s own and morph positions bit
    for bit at all 588 vertices of the 12 patches; a test with one reordered operation fails. A
    GPU may differ in the last bits: WGSL allows fused multiply-adds, 2.5 ULP in an `f32` division
    and `sqrt` at `inverseSqrt`'s accuracy. Every inexact quotient is a small quantity, far inside
    T4.b's 1 mm. The catalogue's display-name test is `view/engine/catalogue`, outside the
    acceptance filter `view/terrain`.
  - _The frames check_ (`smoke/terrain.ts`, `checkTerrainFrames`, group "R05.T11.b the terrain's
    frames", after T10.b landed). A height worker bakes 5 × 5 test-planet patches (ridges off)
    around the centre of face 2: level 5 for a camera 400 km above the centre patch's origin
    looking straight down, level 18 for one 10 m above it looking 30° below the horizon. They are
    drawn by both paths (high: `BakedOffsets`, double normals; low: `FaceDifferences`, mesh
    normals) over a 64 MiB cache, not the setting's, since the engine frees nothing before it is
    disposed. Each frame (64 × 48, R01's harness projection) must be finite everywhere, with the
    terrain's meter class covering at least 90% of the frame from 400 km and 25% from 10 m. On
    SwiftShader on 2026-10-03, both variants: 100% from 400 km and 68.8% from 10 m, every terrain
    pixel lit, no uncaptured GPU error. The terrain being drawn with `cullMode: "back"` also
    confirms the mesh's winding.
- **Deviations in T11.c, as built** (2026-10-03).
  - _The pass_ (`view/terrain/terrainPass.ts`).
    - `TerrainPass` holds one view's `TerrainResources`, `PatchCache` (over `layout.slots`) and
      pool. All three are made per device and remade with a new pool after a device loss. A late
      bake from the old pool is ignored.
    - The pool comes from a `TerrainPoolFactory(bake)`, the layout's vertex path decided first;
      `TerrainPool` is the subset the pass drives.
    - `ready()` compiles `terrainMaterialSpec(layout.vertexPath)`.
    - `frame(TerrainFrameInput)` returns `TerrainFrame`: the draw or `null`, the draw set, the
      selection, `reselected`, the conditions and the debounced line.
    - The camera comes as `TerrainView`: the body's `Rotation3`, the camera in the body's
      non-rotating axes, and the orientation there. The pass rotates it into body-fixed axes for
      selection (position by Rᵀ, orientation conj(q_R) · q).
    - Patch origins and contacts are written as R · p less the camera, from `f64`, narrowed once.
    - `bodyRotation` is R, column-major.
    - A setting change is a new `TerrainPass`; the engine frees nothing before it is disposed.
  - _The cadence_ (decision-r05-patch-demand.md 4d). Selection re-runs on any of:
    - a change of viewport, field of view, contacts (compared by value, from a copy) or baked
      ranges (any bake stored);
    - a rotation, roll included, of more than one pixel's angle, 2 acos |q · q′| > fov_x ÷ W;
    - a move of more than `RESELECT_MOVE_FRACTION` = m ÷ (1 + m), about 0.0909 for m =
      `SELECTION_MARGIN` (0.1), × d_min: the nearest selected non-finest patch's box distance
      (T13.c, as built: spheres of ±24.5 km height ranges contained a low camera), floored at one
      finest patch. It was m × d_min until 2026-10-05 (Risks, "The cadence's 1%").

    It selects at τ ÷ 1.1 with `maxPatches` = ⌊slots ÷ 2⌋ (981 high `BakedOffsets`, 1,952
    fallback, 648 low) and the cache as `heightRanges`. The draw set, `retain`, the conditions and
    the contacts near drawn patches are recomputed only when selection runs, so a frame at rest
    reuses its `DrawSet`. The demand (T7.c's breadth-first list, less resident keys) goes to the
    pool when selection runs or a bake lands.

  - _The morph bands_ (`morphRangeM`, `MORPH_START_FRACTION` 0.7, a hand value).
    - Level n's band runs from 0.7 of the way from d_n to d₍ₙ₋₁₎, ending at d₍ₙ₋₁₎, where d_k is
      the distance at which `selectionErrorM(k)` subtends the setting's τ (raised under the
      budget, below; not τ ÷ 1.1, so that a coarse–fine edge stays at morph 1 between selections).
    - Level 0 has none. The finest level's band is [0.7 d₍ₙ₋₁₎, d₍ₙ₋₁₎].
    - The bands are computed per level when selection runs. Stand-ins take their own level's.
    - When selection is `limited`, the bands are computed at τ × max(1, `limitExcess` ÷ w)
      (`decision-r05-high-bound.md`), so that a coarse–fine edge stays at morph 1 under the budget
      too. The skirts remain the guard against `f32` hairlines.
      - _As built (F2, 2026-10-04)._ `effectiveTauPx(τ, limitExcess, w)`, with w = 1 for the
        pass's one view. `limitExcess` is measured against the τ_sel = τ ÷ 1.1 that selection runs
        at, so every drawn baked leaf has ρ ≤ τ′ = τ_sel × max(1, `limitExcess` ÷ w), the τ′ of T7
        and of T13.a's record. The bands, at 1.1 τ′, keep the margin of 1.1 over the leaves that
        they have at τ without a budget. `limitExcess` is 0 when the budget does not bind (T7, as
        built), so the bands are then at exactly τ, bit-identical to before, with no test of
        `limited`. Unbaked leaves are held by the streaming gate, as before.
      - _Tests (`terrainPass.test.ts`)._ At 2,000 km, unbudgeted, the written bands equal
        `morphRangeM` at τ bit for bit. At 1.5 km, 69° from straight down at 1080p, the budget
        binds (981 patches, `limitExcess` about 1.65). There the bands equal `morphRangeM` at
        τ × `limitExcess`, and every vertex of the 148 coarse–fine edges has morph factor 1, the
        nearest at 1.107 × its band's end. With the bands at τ, all 9,620 of them are below 1,
        many at 0.
        - The vertices are the cube sphere's `f64` points at the fake bakes' height of 0, under the
          shader's `morphFactor` without contacts on the written `f32` bands, not
          `vertexEmulation.ts`'s positions: the fake bakes carry no offsets, and the margin is far
          beyond `f32` rounding.
        - Only the bands read the new tolerance, so the unchanged suite pins every other output.
          `just test-render --captures` (both variants, every shot unlimited) gives 28 PNGs
          byte-identical to a build with the bands at τ.
      - _What remains._
        - Every band moves when τ′ does, at each selection. A vertex inside its band then steps in
          morph factor by about (k + start ÷ (end − start)) × Δτ′ ÷ τ′, about 6 × Δτ′ ÷ τ′ for a
          level whose bound halves. Selection re-runs on every stored bake, so while streaming
          under the budget τ′ can change almost every frame.
        - Where τ′ falls between two selections, a patch the looser budget now splits can start
          partly morphed. Where τ′ rises, a merged patch's children may have been partly morphed.
          The bands' margin of 1.1 is shared with the camera's move since the last selection: the
          largest move the cadence allows, m ÷ (1 + m) of d_min, uses all of it, so any change
          of τ′ after such a move shows (Risks, "The cadence's 1%"). Either is a pop of up to
          about the larger τ′ in bound terms, as every split under the budget was before F2.
        - F4's record could add the change of τ′ between consecutive selections, which sets those
          steps. The ruling's by-hand look at T17's ridged high run is where any of it would show.
        - The coarse side of an edge is not F2's. Crack-freedom also needs the coarse leaf's shared
          vertices unmorphed, and near a low camera a coarse patch can span more depth than its
          band allows. Unbudgeted, at 1.5 km and 640 × 360, some reach morph factor 1 towards their
          own parent, and the skirts cover the step. In the budgeted 1080p case above all are at 0,
          the nearest at 0.983 × its coarse band's start.
  - _Contacts._ Only contacts whose held radius plus ramp reaches a drawn patch's bounding sphere
    are written. Each is written with T7.c's `heldRadiusM` and `morphRampM`. Both rules are 3-D, so a body above
    the ground is passed as the surface point beneath it (T13.b's to do). Selection also takes
    `skirtMarginM` = `WORKER_SKIRT_MARGIN_M` (0, as `heightBake.ts`'s `bakeKey` bakes).
  - _The annunciation._ The pass owns the view's `TerrainAnnunciationDebounce`. `STREAMING` and
    `DETAIL LIMITED` come from T9's `terrainConditions(draw, selection, selection)`, so `limited`
    alone sets the latter, and on the low setting any drawn terrain sets it (Design note 26).
    `limited` is measured against τ ÷ 1.1. T13.b passes the line to `labelStatements`.
  - _Allocation._ A frame writes the instance and contact records from scalars
    (`InstanceRecords.pushXyz`, `ContactRecords.pushXyz`, added) into buffers made once, and
    returns one draw item made with the material. The counting engine checks that no buffer,
    texture, mesh, material or target is made after warm-up, and a test checks that the
    `DrawSet` is reused at rest. Since lane B's perf (a) (2026-10-03), the pass keeps one
    `DrawSetResolver` per cache (made with the cache, remade after a device loss) and calls
    `resolve(selection)` when selection runs: the same `DrawSet` rewritten in place, its first
    `count` patches and slots drawn in selection order. An unseen forced patch
    (`SelectedPatch.seen` false) is requested and pinned by `retain` but not drawn; a test puts a
    contact on the far side of the planet and checks that none of its region is drawn while it is
    requested. Selection itself still allocates (lane B's perf (b)).
  - _The lit view_ (`view/spike/litView.ts`, `litAgx.wgsl`).
    - `LitView` makes the spike's own `<view>:spike-hdr` `rgba16float` target with depth
      (`render-targets`), at the render size `renderSizeOf(size, renderHeightPx)`: 720p at the
      view's aspect on low, the presented size on high.
    - It draws the terrain into it under `TERRAIN_PASS_LABEL`, then one full-screen triangle into
      any `LitOutput`, labelled `spike display`. The triangle samples the target bilinearly
      (upscaling low) and applies R02's `agx`.
    - Selection's viewport is the render size, so τ is in rendered pixels: on low, τ = 2 px at
      720p (Design note 26), about 3 px presented at 1080p. DN23's "the presented size" is
      superseded for the low setting (the orchestrator's ruling, 2026-10-03).
    - Design note 17 makes the spike its own scene. R07.T7, which owns the production HDR target
      (decisions-r06-r07.md item 1), is not built, and R07 replaces this view. Confirmed by the
      orchestrator (2026-10-03): item 1 covers the production photorealistic target, and this is
      measurement scratch behind `--descent-spike`. If the spike is kept beyond RM2, R07.T7's
      `createSceneTarget` replaces it.
    - `SPIKE_DAY_TRIPLE` is f/16, 1/128 s, ISO 100: EV100 = log₂(256 × 128) = 15. `setExposure`
      changes it per segment.
    - The display material `SPIKE DISPLAY` joins the catalogue's terrain entries.
    - "Inside R02's `VIEW`" is the `LitOutput` seam. T13.b wires `DescentSpike`'s view through it.
  - _The captures_ (`captureTerrain`, group "R05.T11.c the terrain captures", under
    `--smoke-captures`). The test planet, ridges off, is streamed through a three-worker
    `HeightWorkerPool` and drawn through `LitView`, 480 × 270, over the cube's +x/+z face edge.
    Two shots: from 400 km (high, 30° below the horizon) and from 2 m (low, 10° below, rendered at
    the 720p rule). Each waits until nothing stands in, or 3 s pass with no new resident patch, or
    120 s in all, and checks that it streamed everything. `srgb8` and `base64Of` are exported from
    `smoke/atmosphere.ts`.
  - **Result, 2026-10-03.**
    - On the RTX 3080, hidden: an offscreen window never shown, R01's Vulkan switches, a fresh
      `--user-data-dir`, the process group killed after. Headless Ozone's GPU process exits on a
      hardware adapter.
    - On SwiftShader under `just test-render --captures`, both variants.
    - Both shots streamed everything: 44 patches from 400 km in about 1 s, 133 from 2 m in about
      2 s, nothing standing in or missing, `limited` false, no uncaptured GPU error.
    - The lane viewed the PNGs (kept local in the scratch directory). Both are the right way up,
      grey terrain below and black sky above. From 400 km the limb curves across the upper quarter;
      from 2 m the horizon lies about a quarter of the way down and the relief shows as fine
      shading. No crack or gap shows along the +x/+z face edge in either.
    - The RTX 3080 and SwiftShader images look the same.
    - The on-screen look by a person stays pending by hand for the owner (decisions-r05.md item
      7).
- **Orchestrator rulings, 2026-10-03.** T13.c's text now says it wires T14.b's `launchSwitches` into
  `index.ts`; T14.a's files and acceptance drop `view/spike/percentiles.ts`, whose figures T14.c
  computes in the main process.
- **Deviations in T14.a, as built** (2026-10-03, partial).
  - `view/spike/pipelineShim.ts`: `wrapGpu(gpu, wrapDevice)` (a plain object forwarding to the
    browser's `GPU`, whose adapters' `requestDevice` is replaced on the instance), `shimPipelines`
    and `PipelineTally` (`endWarmup`, `creations`, `late`), each creation stamped with the descent's script time, as `DescentSpikeReport.latePipelines` takes it. The device keeps its identity: methods
    are replaced on the instance, not put behind a `Proxy`, since the browser's WebGPU calls
    brand-check their arguments. An asynchronous creation that calls the synchronous form through
    the instance is counted once.
  - `view/spike/metrics.ts`: `SpikeMetrics` keeps the raw series T14.c's `DescentSpikeReport` takes
    (`frame`, `passTimes` aligned by `PassTimes.frame`, `allocation` for uploads, `patches`,
    `report`), `SEGMENT_MEASURE_PREFIX` (`spike.segment:`, which `main/results.ts` repeats: the
    renderer and the main process share no module, so the two constants are kept equal by hand)
    and `TIMED_PASSES_A_FRAME`. The report's timer is the worst state seen over the run, and a streaming interval that crosses a segment boundary is shared between the segments. The adapter's peak comes from T11.a's `allocationTally` through
    `report`'s argument.
  - **Settled by T13.b (2026-10-03):** the spans are T13.b's. The wrapped `GPU` reaches the spike
    through `DescentSpikeProps.engineSource`, and `SpikeMetrics` is built over
    `SpikeListeners`, both by T13.c (T13.b's record, "The metrics seam").
- **Deviations in T15.a, as built** (2026-10-03).
  - `view/spike/capture.ts`: `GpuCapture` (`wrapDevice`, `startSpan`, `frame`, `endSpan`,
    `result`, `dispose`), `parseCapture`, `replayCapture` (the reference replayer the test uses) and the file
    types. The log is generic: each call is `{ target, op, args, result }` by object ID (0 the
    device, 1 its queue), arguments as JSON with `$ref`, `$blob` (+ `$type`), `$undefined` and
    `$bigint`, so the format needs no schema per call. On disk a capture is `capture.json` and
    `capture.bin` (blobs by offset and length) in the `--capture` directory; T13.c writes them.
    The file also carries `meta` (`setting`, `seed`, `passRows`), which the replayer reads for its
    results file.
  - Before the span only creations, views, bind-group layouts and destructions are logged. At the
    span's start every live buffer and texture is copied to staging buffers in one submission
    (buffers and textures are created with `COPY_SRC` added while the shim is installed; the log
    keeps the usage asked), and their bytes are placed in the log as writes at that point, so the
    engine's calls during the read-back are kept; `result()` refuses until they are all in, and a
    failed read-back destroys the staging buffers and rejects `startSpan`. Mappable buffers,
    multisampled textures, depth and stencil formats (never a copy destination) and formats without
    a texel size in the shim's table are listed in `skipped`; a buffer whose size is not a multiple
    of four loses its last bytes. `startSpan` is called between frames: a span call on an object
    whose creation the log lacks (an encoder open when the span started) is not logged but listed
    in the file's `problems`, as is an argument naming one, so a capture says when a replay cannot
    trust it. `writeBuffer` is logged as the bytes written,
    with an explicit data offset of 0 and size.
  - Canvases are surfaces: `GPUCanvasContext.prototype.getCurrentTexture` is wrapped from the
    span's start to `endSpan` or `dispose` (`CaptureOptions.contexts`), each canvas logged with
    its size and format; T13.c calls `dispose` when a run ends early.
  - It installs through T14.a's seam: `spikeDeviceWrapper(tally, capture)` in `pipelineShim.ts`
    applies the pipeline tally always and the capture only when given, so a run without
    `--capture` carries none (tested). `engineBoundary.test.ts` exempts `view/spike/capture.ts`
    and its test by name from the allocation rule (`isCaptureShim`; the plan named the shim alone,
    but its test drives a device directly).
- **Deviations in T15.b and T15.c, as built** (2026-10-03).
  - `tools/gpu-replay` (own `[workspace]`, wgpu and naga `=30.0.1`, winit 0.30, pollster) has
    `src/lib.rs` and, beyond the plan's files, `src/run.rs` (the offscreen and presented drivers)
    and `src/window.rs` (the winit window), so that `main.rs` stays thin. `gpu-replay validate
<capture>` reads and validates; `gpu-replay replay <capture> [--present] [--setting high|low]
[--out <dir>]` replays and writes `<date>-<machine>-<setting>-replay.json` (default
    `docs/measurements/descent-spike/`). `just replay` runs `replay` in release.
  - The replay recreates the capture's objects in wgpu from the WebGPU names in the log (wgpu's
    `serde` feature reads them), adds `COPY_DST` to buffers and textures for the snapshot's
    writes, and keeps passes open across calls (`forget_lifetime`). The capture's own query sets
    and `resolveQuerySet` are not replayed: the replay times every pass itself with
    `TIMESTAMP_QUERY` where the adapter has it, labelled as captured. A buffer created
    `mappedAtCreation` starts as zeros (its mapped writes are not in the log). Arguments that do
    not parse (a format, a vertex attribute, a binding type, a constant), and buffer ranges past a
    buffer's end, stop the replay with the call named, rather than being defaulted. wgpu's
    validation errors are collected and reported, not fatal; the capture's `problems`, capture
    features the adapter or the replayer lacks, and a canvas format the window cannot present are
    reported as `findings`.
  - Offscreen, every canvas is an offscreen texture; frame intervals are the gaps between the
    GPU timestamps at the end of successive frames' last passes, with at most two frames queued
    (the replay waits on the frame two before), so they say what the GPU sustains; without
    `TIMESTAMP_QUERY` they are null with that reason. Presented, the main (largest) canvas is the
    window's surface with FIFO presentation, intervals are taken as `present` returns, and an
    outdated or lost surface is configured again, up to eight failures in a row. A replay's
    results file follows `main/results.ts`'s schema (version 1; version 2 since 2026-10-04, see
    T14.c's deviations), whose types gained
    `frames.source: "gpu-completion"`, the optional `frames.gpuCompletion` and the launch mode
    `native-replay` (the backend is in `run.options.backend`); figures a native replay cannot have
    (trace, main thread, memory, rAF) are null with their reason. A GPU row counts only frames
    with a pass of that row, as the client's writer does.
  - The checked-in capture (`tests/fixtures/small`) is hand-written in the client's format (two
    frames, `meta` naming its setting, seed and pass rows), with one module invalid on purpose.
    Unit tests pin the argument mapping (extents, binding types, ranges, dynamic offsets, absent
    arguments) and the criterion's rows. Its replay test needs a GPU adapter (any wgpu backend) and runs
    with `cargo test --manifest-path tools/gpu-replay/Cargo.toml`, display variables unset; it
    passed on the RTX 3080 (Vulkan) on 2026-10-03. The root `Cargo.toml` has
    `exclude = ["tools/*"]`, so neither `just lint` nor `just ci` checks the tool: its fmt, clippy
    (`-D warnings`) and tests are run by hand with that manifest. Canvas textures of past frames
    stay in the replay's object table for the run (a span is short).
  - The capture of the real descent and its offscreen replay on the RTX 3080 are done (the record
    below). **Pending:** the presented replay, by hand for the owner.
- **T15's first capture of the descent and its offscreen replay** (2026-10-04 and 2026-10-05,
  lane D, at `rendering-and-planets` c629b01, with the replayer's fix 7fd98eb).
  - _The capture run._
    - It was `descentSpike.sh --setting low --hidden --capture <dir>`, with
      `HYPERION_SPIKE_PORT=7893`, under the heavy-test lock. It followed a `--smoke --capture`
      check: a span from 5 s on the high setting, which replays too.
    - No other spike or replay run overlapped it. The integration `just ci` in the primary
      checkout did, with its `gpu-replay-check` from 21:19 to 21:23 CDT. The load average was 21 at
      the start and rose to 165 at 21:27, in the descent arc. It had fallen to 5 by the span.
      The timings are provisional.
    - The run exited 0 after 1,280 s. The span is T13.c's: 120 frames from 1,085 s,
      5 s into the low fast pass.
    - `capture.json` is 172,679,574 B and `capture.bin` is 84,034,064 B. Neither is committed.
      Both stay in the lane D worktree's `target/laneD/capture` for the presented replay.
    - The capture holds 1,169,018 calls, 1,141,826 of them before the span. Its three canvases
      are all rgba8unorm: one of 806 × 431 and two of 80 × 140. The snapshot skipped 38
      resources: 34 pass-time read-back buffers and four depth textures. It lists no problems,
      and naga accepts all 19 of its modules.
  - _The run's own results:_ `2026-10-05-effect-low.json` (109,110 B once formatted) and its
    `.md`, under the writer's UTC date. They are T14.c-style, but the capture's costs are in them,
    so they are not T14.c's record.
    - `validateResults` passes. Every figure is present or null with a reason:
      - "no window shown" (nine);
      - "the trace has no timed event" (five);
      - NVIDIA's absent DRM fdinfo (three).
    - The headline figures:
      - terrain GPU p95 0.71 ms;
      - atmosphere 3.57 ms (limit 4 ms);
      - GPU memory 0.254 GB, from `nvidia-smi` less its baseline;
      - rAF p50/p95/p99 16.70/16.80/50.00 ms;
      - uploads 3.24 GB;
      - no late pipelines.
    - The trace came back empty again. The tracing service peaked at 1.76 GB (the tracing
      finding above).
    - What the capture itself costs:
      - The low fast pass's longest frame is 1,949.9 ms: the span's snapshot and its write.
      - The renderer's private memory rose about 1.7 MB a second, to 3.33 GB, against T14.c's
        1.1. The trace's CPU profiler accounts for about 1.25 MB a second (the renderer-memory
        finding above). The shim's log, which keeps every call from the start, is the likely
        rest.
    - The descent arc's longest frame is 5,883 ms, during the load peak. Its cause is not
      separated.
  - _Two replayer faults the capture found, fixed in 7fd98eb._
    - _Freed objects._ The replay made every call before the span. Those include 781,049 views,
      180,260 bind groups and 180,239 layouts read back from pipelines, which the engine had
      dropped and the browser had freed. Every submit then failed ("submission index 15 … last
      successful submission: 0").
      - `Capture::setup_calls` now leaves out the views, bind groups and read-back layouts that
        nothing replayed uses (`DROPPED_BEFORE_THE_SPAN`).
      - It keeps 303 of the real capture's 1,141,826 calls.
    - _The sRGB view._ The engine configures each canvas with its sRGB form as a view format
      (`srgbViewFormat`), which the capture does not log. So the replay's offscreen canvases
      refused the display pass's sRGB view:
      - the smoke capture's replay had 1,071 validation errors;
      - the `spike display` and `view:wireframe` submits failed every frame.

      `canvas_view_formats` now gives that view format to the offscreen canvases, and to the
      presented surface where the adapter has `SURFACE_VIEW_FORMATS`.

    - _The fixture_ gained a view that nothing uses and a clear through the canvas's sRGB view.
      Its GPU test fails without the view-format fix and passes with it.
  - _The offscreen replay_, recorded as `2026-10-05-effect-low-replay.json` (8,822 B once
    formatted).
    - The command was `env -u DISPLAY -u WAYLAND_DISPLAY just replay target/laneD/capture`. It
      ran under the heavy-test lock on the RTX 3080 (Vulkan), at a load average of 8.6. The GPU
      was idle (P8, 210 MHz) before it.
    - It reported no validation errors. Its two findings are features of the capture that the
      replay does not map: `core-features-and-limits` and `float32-blendable`.
    - `validateResults` passes. Every figure a native replay cannot have is null with its
      reason: the trace, rAF, the main thread, the GPU process and memory.
    - The figures:
      - GPU-completion intervals p50/p95/p99/max 0.64/1.29/2.18/2.26 ms, over 118 intervals;
      - terrain p95 0.085 ms;
      - atmosphere 0.165 ms (the view's three canvases plus the composite);
      - the pass sum's p95 0.271 ms;
      - span uploads 8.06 MB;
      - no untimed pass.
    - _Not comparable with the client's figures yet._ The replay's pass times are a fraction of
      the run's own (terrain 0.085 against 0.71 ms, atmosphere 0.165 against 3.57 ms). Three
      causes are not separated:
      - the client's percentiles cover the whole descent, the replay's only the span;
      - the replay's 120 frames run back to back from an idle GPU, about 0.08 s of GPU work in
        all, while the client's frames are paced by `requestAnimationFrame`;
      - Dawn's robustness transforms (`--dawn-safety on`) and its barriers differ from wgpu's.

      Matching the client's own timestamps over the same 120 frames would settle it. The
      results file has no per-frame series for that.
  - **Pending, by hand for the owner** (a visible window on `:0`): the presented replay, from
    the primary checkout once this lane is merged:

    ```sh
    just replay /home/quantum/gh/hyperion/.claude/worktrees/agent-a41f8f84c6e445909/target/laneD/capture --present
    ```
- **Deviations in T9, as built (the annunciations, 2026-10-03).** `annunciation.ts` adds, beside
  `terrainAnnunciation`: `TerrainAnnunciation` (the two strings), `TerrainConditions` and
  `terrainConditions` (the frame's two conditions before the debounce), `coarserThan` (some
  reference patch covered by an ancestor in the selection), `ANNUNCIATION_ONSET_MS` (250) and
  `ANNUNCIATION_CLEAR_MS` (1,000), and `TerrainAnnunciationDebounce`, one per view, whose `update`
  takes a monotonic `nowMs` (the tests feed times directly rather than fake timers). `STREAMING`
  also holds while a selected patch has no resident ancestor at all (`DrawSet.missing`), not only
  while an ancestor stands in. `labelStatements(run, terrain = null)` appends the debounced line
  after the existing statements and stays pure; `ViewDisplay` does not pass it yet, since no view
  draws terrain: T11.c (the pass in a view) creates the per-view debounce and passes its line. The
  tests build the low and reference selections by hand until `selectPatches` lands (T7.b). The
  guide row and Design note 23 already carried decisions-r05 item 5's wording.
- **Deviations in T2's TypeScript mirror, as built (2026-10-03).** `cube.ts` mirrors `cube.rs`
  and `geometry.rs`: `stToUv`, `uvToSt`, `faceUvToXyz` (the unnormalised tuple), `unitDir`,
  `faceUvToDir` (Provides' `Vec3` form), `faceOf`, `xyzToFaceUv`, `sampleDir` (64 or 128 a side),
  `vertexDir`, `vertexSpacing`, `finestLevel`, `PATCH_QUADS`, `BAND_LIMIT_M`, `FINEST_SPACING_M`
  and `MAX_FINEST_SPACING_M`, over an `Xyz` tuple. `patchKey.ts` gains Rust's integer cube
  geometry: `patchKeyWord` (the `to_u64` word as a `bigint`, for the golden), `Edge`, `EDGES`,
  `edgeNeighbour`, `edgeNeighbourAndBack`, `cornerNeighbours`, `sameKey`, `facePoint`,
  `faceCoords`, `faceOfAxis`, `canonicalFace`, `Axis` and `unreachable` (which closes the numeric
  switches, whose exhaustiveness oxlint's `consistent-return` cannot see). The golden is read
  whole: the 1,000 warp values, the 50 patches' words, printed vertices and full-patch digests,
  their edge and corner neighbours, the 24-crossing table and the 20 finest levels with their
  spacings, all bit for bit (19 tests, first run green). The digest needs the testkit's
  `f64_digest`, so `workers/f32Digest.ts` (T10.b's file) landed here with `fnv1a64`, `f64Digest`
  and `f32Digest`, checked against FNV's published vectors and the testkit's hand-computed value.
- **Deviations in T7.a, as built (bounds and culling, 2026-10-03).** `PatchBounds` gains
  `box: OrientedBox` (the centre's spheroid normal and two tangents, with half-extents) beside its
  sphere, whose centre is the box's; the bounds sample the patch's boundary at every fourth vertex
  and the centre vertex at both ends of the level's height range and pad by the largest chord
  between neighbouring samples (about a sixteenth of the patch). Added beside the Provides names:
  in `planet.ts` `LEVEL_TABLE_STRIDE`, `levelBoundM`, `levelHeightRangeM`, `lowestHeightM`,
  `spheroidPoint`, `spheroidNormal` and `surfacePoint`; in `bounds.ts` `OrientedBox`,
  `boxCorners`, `relativeBounds`, `distanceToBoxM` (selection's distance to the nearest point)
  and `CameraRelativeBounds` (there, not in `cull.ts`); in `cull.ts` `Plane`, `FrustumCamera`,
  `frustumOf` and `horizonCone`. `HorizonCone` holds the camera's position and R_occ = c + the
  lowest height of any level (0 with no table); the test is off at or below R_occ. The frustum's
  sphere stage is its own, not `sphereInFrustum`, which takes R02's `ProjectionCamera`.
  `bandLimitM()` returns the mirrored `BAND_LIMIT_M`, since the render thread loads no
  WebAssembly; `planet.wasm.test.ts` checks it, `FINEST_SPACING_M` and both level tables against
  the module itself. `planetGeometry` refuses a figure not 0 < c ≤ a and a table not 100 finite
  entries. Shared test fixtures: `src/test/terrainFixtures.ts` (`UNIT_BOUNDS`, `WGS84_FIGURE`,
  `goldenLevelTable`, `selectionOf`).
- **Deviations in T7.b's selection, as built (2026-10-03).** `selectPatches` charges a level
  `selectionErrorM` = ε_n + `chordSagittaM`, the flat triangles' sag below the datum, which ε_n, a
  bound on heights, leaves out; without it a zero-height spheroid (R07's `mesh` regime) never
  refines past the roots. Ruled by the science-checker (2026-10-03, relayed by the orchestrator):
  sag_n = K h_n² ÷ (4 c² ÷ a), K = `SAGITTA_FACTOR` = 1.03. Linear interpolation's error is at most
  r² ÷ (2 ρ_min) for r the smallest enclosing disc's radius (Waldron 1998, SIAM J. Numer. Anal.
  35(3) 1191–1200, Thm 4.1 eq. (4.5)), ρ_min = c² ÷ a the spheroid's smallest radius of curvature,
  and on the quadratic-warp cube sphere split on (0, 0)–(1, 1) r_n² ≤ 1.023 h_n² ÷ 2 (worst at
  (s, t) ≈ (0.234, 0.766); `select.test.ts` measures 1.020–1.025 over a face's 64² cells), K
  rounding that up to cover the height's stretch 1 + H ÷ ρ for |H| up to about 30 km; level 0
  sags about 1.17 km on WGS 84, under a sixth of ε_0, and under a tenth of ε_n from level 1.
  The distance is to the nearest point of the patch's oriented box. A patch is refined where any
  view that sees it finds ρ > τ; one no view sees is not selected. The restricted quadtree is
  enforced after the traversal by `restrictQuadtree` (exported, and tested on hand-built leaf
  sets across a face edge and at a cube corner): a leaf two or more levels coarser than an edge or
  corner neighbour is split, each child expanded as the traversal would. Bounds are memoised per
  `PlanetGeometry` (a `WeakMap`, at most 131,072 a planet, the older half dropped past it), a pure
  cache that changes no result. `GroundContact` sits in `select.ts` until T7.c moves it to
  `grounded.ts`; `demand` is filled by T7.d. Finding: on the test planet's hard bound
  (decisions-r05 item 6), a 1080p view at τ = 1 px from 1.5 km, tilted 69° from the nadir,
  selects about 12,700 patches; with the per-level error and the horizon's corners computed
  without allocation and the leaf walk stopped at the first interior node, a warm call takes
  about 80 ms on the loaded development machine (360 ms before): still more than a frame, so the
  per-frame cost is revisited when T11.c drives selection each frame.
- **Deviations in T7.c, as built, with the patch-demand ruling (2026-10-03,
  `decision-r05-patch-demand.md`, items 4a, 4b and 4d), which amend Design notes 7, 10, 23 and 24.**
  - _The grounded rule._ `grounded.ts` holds `GroundContact` (moved from `select.ts`),
    `FORCED_REGION_RESIDENCY_S` (30 s), `DESCENT_ALTITUDE_M` (1 km), `DESCENT_TIME_TO_CONTACT_S`
    (30 s), `isDescending(altitudeM, verticalSpeedMps)` (positive upward), `finestPatchSizeM`
    (64 of the finest level's largest spacings, 20.7 m on the test planet), `heldRadiusM`
    (r_g = radius + one finest patch), `morphRampM` (one finest patch), `forcedRadiusM` (r_g + the
    ramp + one more patch), `inForcedRegion` and `contactHold`. `morphHold(v, grounded,
patchSizeM)` takes the finest patch size as a third argument. The hold is term for term lane
    C's `terrain.wgsl` `morphFactor` (clamp((|v − c| − r_g) ÷ ramp, 0, 1), a step at r_g when the
    ramp is 0), pinned by a test against the shader's expression evaluated in `f32`; lane C's
    `vertexEmulation.ts` has no hold to pin against. A contact's forced region is measured from
    the patch's box, not its bounding sphere: with the level's ±24.5 km height range a sphere
    reaches tens of kilometres past the footprint and forced a 25 km disc to the finest level.
    Both rules are three-dimensional, so a body above the ground is passed as a contact at the
    surface beneath it (`GroundContact`'s documentation).
  - _Inherited height ranges (4a)._ `SelectionInput.heightRanges` (a `HeightRangeLookup`, which
    `PatchCache` implements through `heightRangeM(key)`) feeds `inheritedHeightRangeM`: the nearest
    baked ancestor's range, rounded outward from `f32`, widened by ε_m + ε_{n−1} and below by the
    skirt (ε_n and an `f32` step), within the level's range. `patchBounds` takes the range as an
    optional third argument; the bounds memo is per level by numeric key, with the range it was
    built for. `select.wasm.test.ts` bakes ancestors and descendants with the module and finds no
    baked height outside the inherited range. Design note 7's inputs now include the baked ranges.
  - _The budget (4b)._ `SelectionInput.maxPatches` (absent: no limit): selection refines from the
    roots by a max-heap on (forced, w × ρ ÷ τ, level, key), each split balanced at once by
    `PatchLeafSet.splitBalanced` (the restricted quadtree, now incremental, exported for its
    tests, with `begin`/`commit`/`rollback`), and a split that takes the leaves past the budget is
    undone and stops the run with `Selection.limited` true; forced splits are never refused. A run
    may pass through more leaves than it ends with (a split drops children no view sees), so a
    budget equal to the unbudgeted count can still stop it. `restrictQuadtree` is gone.
    `terrainConditions` reads `limited` as `DETAIL LIMITED` (Design note 23), so the high setting
    passes its selection as its own reference.
  - _Breadth-first demand (Design note 24)._ For each selected patch not baked, the shallowest
    unbaked patch on its way down whose parent is baked (or a root) is requested once, at the
    largest w × ρ ÷ τ of its parent, the patch drawn in its place; a forced patch is requested
    directly. With nothing baked the demand is the roots.
  - _Cost (4d)._ Leaves keyed numerically per level, the per-level error computed once a call, no
    per-view array a node, interior neighbours without the face fold. At the 1.5 km pose (ridges
    off, 1920 × 1080, fov_h 60°, τ = 1 px, tilted 69°), warm, on the development machine at load
    23: 12,731 patches unbudgeted in about 75 ms, 1,952 (budget 1,952) in about 11 ms, 981 (budget 981) in about 5 ms. The ruling's 2 ms at p95 is not yet met; the remaining costs are the
    per-node key string (for the baked-range lookup and the output map), the camera-relative
    bounds objects and the neighbour arrays. Recorded, not asserted (Design note 27).
- **T7.c's review, as built (2026-10-03).** `inheritedHeightRangeM` takes the bake's skirt margin
  (`SelectionInput.skirtMarginM`, the worker's `skirtM`, default 0) and its `f32` steps from the
  largest height the patch can reach, so that the range reaches the skirts' bottoms as the bake
  hangs them (`select.wasm.test.ts` checks every edge vertex's skirt bottom at margins of 0 and
  5 m). Selection floors a patch's distance at the near plane (0.1 m), so a camera inside a volume
  gives a finite excess that a secondary view's weight still scales. `maxPatches` counts forced
  patches but never refuses them, so a selection can exceed it by the forced region, its balance
  and the six roots.
- **Deviations in T7.d, as built (several views, 2026-10-03).** The union of the views is one
  traversal (T7.b), and the demand is built in `select.ts` (T7.c's breadth-first rule).
  `priority.ts` holds `compareRequests` (forced first, then the higher priority, then
  `patchKeyString`), `PRIMARY_VIEW_WEIGHT` (1) and `SECONDARY_VIEW_WEIGHT` (0.25). A request's
  priority is the largest w_view × ρ ÷ τ over the views of the patch drawn in its place, its
  parent (a root's own). `priority.test.ts` builds demand with everything to level 2 baked, so
  that it reaches level 3: two views at one pose request each patch once, a secondary view's
  priorities are the primary's × 0.25 (the near-plane floor keeps them finite), forced patches
  come first, and the order is `compareRequests`'s.
- **T7.d's review, as built (2026-10-03).** A request's priority is now Design note 24's: the
  largest w_view × ρ ÷ τ of the patch drawn in its place over the views that see the request and
  want that patch split (where none does, a split the 2:1 balance made, over the views that see
  both). Each traversal node keeps two view bitmasks (`seenBy`, `wantedBy`), not per-view arrays,
  so selection takes at most `MAX_SELECTION_VIEWS` (31) views and throws past it; a request's
  per-view excess is recomputed only for the demand. A forced region is selected, and requested,
  whether or not a view sees it, so that it is resident before contact (Design note 9); it is also
  drawn, the frustum culling nothing on the GPU. A forced request's priority is its own weighted
  excess, which orders forced requests among themselves only. The tests now check
  `compareRequests` directly, a second view at one pose adding nothing, a patch only a
  secondary view wants ranked at the secondary's weight (a test that fails under the earlier
  rule), a forced region no view sees, and the unforced demand's priorities never rising.
- **R05.T7 perf (a), as built (2026-10-03): the draw set and the pins without allocation.**
  `DrawSetResolver` (one per cache, `resolve(selection)`) returns the same `DrawSet` every call,
  its records, `patches` array and `slots` buffer rewritten in place: `DrawSet` gains `count`,
  `slots` is a buffer of the cache's slot count whose first `count` entries are the instances
  (lane C: read `slots.subarray(0, count)` or upload `count` entries), and the drawn patches come
  in the selection's order, each stand-in where it is first needed, with no sort.
  `resolveDrawSet(selection, cache)` stays, making a resolver for the call. Lookups go by level
  and `patchKeyIndex` (now in `patchKey.ts`, with `ancestorIndex`) through
  `PatchCache.residentAt(level, index)`, and `CachedPatch` carries its `keyString`, so no key or
  string is built per call. `retain` reuses its forced and selected sets and counts its pins into
  fields. Iterating the cache's maps still makes V8's iterator objects; nothing else is allocated
  per call after warm-up. Tests hold the set, its arrays and its records identical across calls.
  _Unseen forced patches (the orchestrator, 2026-10-03)._ `SelectedPatch.seen` is false for a
  forced patch no view sees: it is selected and requested, and `retain` pins it, but the resolver
  leaves it out of the draw set and `maxPatches` does not count it. Forced patches a view sees
  count against `maxPatches` but are never refused.
- **R05.T7 perf (b), as built (2026-10-03): selection toward 2 ms.** `viewGeometry.ts`
  (`ViewGeometry`, `viewGeometry`, `viewExcess`) holds each view's planes, horizon and error
  scale as plain numbers and tests a patch with no allocation; `viewGeometry.test.ts` holds it to
  `inFrustum`, `aboveHorizon` and `distanceToBoxM` over 300 random cameras of 20 patches each.
  No traversal node builds a key string (only the output map and nothing else); nodes carry a
  parent pointer, which the demand's breadth-first walk follows. The bounds memo answers a patch
  with nothing baked above it without computing its range. `PatchLeafSet` is now linked tree
  nodes from the six roots plus a map per level by `patchKeyIndex` (`addRoot` replaces `add`), so
  that the leaf over a neighbour's cell is usually one lookup at the parent's level rather than a
  walk; leaves come out depth first from face 0, in `childKeys`' order. Rollback deletes the
  children from the map. At the 1.5 km pose (ridges off, 1920 × 1080, fov_h 60°, τ = 1 px,
  tilted 69°), warm, under Node 26 at load 15–23 on the development machine: budget 981, p50
  2.8 ms and p95 5.4 ms (was about 11 ms and 15–28 ms); budget 1,952, 5.0 and 8.0 ms;
  unbudgeted (12,731 patches), 37 and 44 ms. The 2 ms p95 is not yet met under this load; the
  remaining per-node costs are the neighbour keys (8 objects a probe), `childKeys`' and the
  children's arrays a split, and `patchBounds` for new nodes, which the ruling's item 4d also
  names. A quiet-machine run is pending (Design note 27). Recorded, not asserted.
- **Selection's stability in motion, as built (2026-10-03, after R05.T13.a's probe).** Lane D's
  fixed-step probe (low fast pass, 300 m/s at 300 m, budget 981, 1,962 slots) saw about 830 new
  keys a frame and demand of 53,000 a second against D = 311. Reproduced in `motion.test.ts`'s
  harness: with a cache that keeps only the selected patches (as the probe's simulated cache
  does), about 650 of 980 patches change every frame. The cause: an unbaked patch's bounds take
  its level's whole ±24.5 km height range, so it looks far worse than its baked neighbours; the
  greedy budget is spent refining under those loose bounds, the refined patches tighten once
  baked, the budget moves to the next loose region, and the cache, keeping no ancestors, loses
  the tight ranges that held the last cut. Three changes, every bound still a true one:
  - _The streaming gate_ (`wantsRefining`): where `heightRanges` is given, a patch is split for
    its error only if it is baked itself, so selection reaches at most one level below what is
    baked and every split is decided on a baked range; it descends as bakes land, the
    breadth-first demand already ordering them, and the unbaked frontier is drawn by its baked
    parent (`TERRAIN: STREAMING`). Forced regions are not gated. Without `heightRanges` nothing
    changes. The gate only stops a refinement: it never claims a smaller error than the bound.
  - _Ancestors are kept_ (`PatchCache.retain`): every resident ancestor of a selected patch is
    draw-pinned and touched, since selection's bounds read its range and it stands in for its
    descendants; the walk runs before the draw set's touches, so a stand-in does not end it
    early (`cache.test.ts` pins the ancestors above a stand-in).
  - _Deepest first among equals_ (`PatchCache` eviction): among patches used as recently, the
    deepest is evicted first; with ancestors first, the selection collapsed to the six roots
    whenever the slots barely held it (`motion.test.ts`'s second test fails without it).
    With the real cache, 8 bakes a frame and the budget, the flight changes about 7–10 patches a
    frame (at most about 20) out of 980; `motion.test.ts` bounds the mean below 3% and any frame
    below 10%, and holds the selection above 90% of the budget at 1,100 slots. Hysteresis was not
    needed. The fixed-step run's simulated cache must keep the selection's ancestors as
    `PatchCache.retain` does (or use `PatchCache` and `DrawSetResolver` themselves), or the gate
    collapses it. Lane C's `terrainPass.test.ts` was adapted: the draw set's slots read through
    `count` (perf a), and two tests stream their demand in before asserting, since selection now
    starts at the roots; with the fake pool's flat bakes the 1.5 km pose fits its budget, so the
    budget test checks the budget holds and that `DETAIL LIMITED` follows `limited`
    (`select.test.ts` covers a binding budget). Selection in the flight, warm, under vitest at load
    46: about 15 ms p50 and 28 ms p95 (provisional).
- **Selection below the datum (2026-10-03, after R05.T13.a's collapse report).** Selection with
  the camera 1.6 m above ground 1.85 km below the WGS 84 datum works, with baked ranges and
  without, looking down or ahead, and a contact forces its region even with no view on it
  (`belowDatum.wasm.test.ts`, real bakes through the module). The probe's collapse to an empty
  selection was its camera hovering about 107 m underground: the descent's site height
  (−1,953.2 m, from `measureTerrain`) was taken along the geocentric direction p ÷ |p| of the
  ground point, where the collision interpolant and the bake take the direction d with
  p = M·d (Design note 5), so it read the terrain about 0.1–0.2° away; the bakes under the
  camera give −1,846.9 to −1,844.3 m. Looking down from below the baked surface, every patch's
  box lies behind the camera, so the frustum rightly culls them all, and the contact, 107 m
  below the ground, forces nothing. No selection change.
- **T7, as built (lane B, with F1 of `decision-r05-high-bound.md`, 2026-10-04).**
  `Selection.limitExcess`: the largest w_view × ρ ÷ τ_view of the split the budget refused, over
  the views that see it, or 0 when not limited. Every drawn leaf (baked, where `heightRanges` are
  given) meets τ × max(1, limitExcess ÷ w) in a view of weight w and tolerance τ.
  - _0, not the ruling's 1, when not limited._ τ × max(1, limitExcess ÷ w) is then τ in every
    view, so F2 needs no test of `limited`; 1 would give τ ÷ w (4τ) in a secondary view. For the
    primary view the two agree, and T13.a reads the value over limited frames only.
  - _τ × max(1, limitExcess ÷ w), not the ruling's τ × limitExcess._ The refused split's excess is
    below 1 when only a secondary view wants it, and a secondary view's own ratio is
    limitExcess ÷ w. T13.a's τ′ for the primary view is τ × max(1, limitExcess).
  - _What holds._ Forced splits are made first and never refused, and the rest go in order of
    weighted excess. So at the refusal, every baked leaf still wanting a split has a weighted
    excess of at most limitExcess, the refused one included. Every other leaf meets τ or is at the
    finest level. An unbaked leaf is held by the streaming gate, not the budget, and is drawn by
    its baked ancestor (`TERRAIN: STREAMING`).
  - _Tests (`select.test.ts`)._ Three cases:
    - one view with no baked ranges (budget 300);
    - a primary view plus a secondary at 0.25 and 4 px, over ranges baked to level 7 (budget 200);
    - a primary view from orbit and a secondary at 0.25 from 1.5 km (budget 2,000). There the
      refused split is one only the secondary wants, and limitExcess is about 0.91.

    Each case checks:
    - the excess is 0 with no budget, or with one not reached;
    - under the budget, every baked leaf meets τ × max(1, limitExcess ÷ w) in every view that sees
      it;
    - the largest weighted excess of a baked leaf still wanting a split equals limitExcess;
    - every split below it is touched by a split patch one level finer outside it, as a balance
      split is.

  - _Selection unchanged._ Digests of the selected keys and the demand, with no budget and under
    each case's budget, match those taken from selection before the field was added. They also
    guard the work on selection's cost (the ruling's item 4), and are re-recorded only when
    selection's output is meant to change.
- **T8, as built (lane B, with F3 of `decision-r05-high-bound.md`, 2026-10-04): the cache keeps
  the baked patches selection hides.**
  - _The cause, not the ruling's guess._ The coarse patches that thrashed had not left the view.
    Selection had reached them and culled them by their own baked ranges: a child a split left
    out, or a split patch with nothing visible beneath it. Neither selected nor an ancestor of a
    selected patch, such a patch was never touched by `retain`, so it aged from its bake and was
    among the first evicted. Evicted, it took its ancestor's looser range (the patch-demand
    ruling's 4a) and rose over the horizon. It was then selected and demanded again, and its
    parent stood in for it and its siblings until the bake landed. Baked, it was hidden again.
    The probe's 3 s warm-up from an empty cache put the cold start's hidden patches in phase, so
    its one second was a burst (45–51); in a warm cache they cycle all through the approach (the
    table below).
  - _The fix._ `Selection.hiddenBaked` (`select.ts`, from `PatchLeafSet.bareKeys`) lists the
    baked patches the traversal reached but left out, depth first from face 0. `PatchCache.retain`
    marks them as used each frame they are hidden. They are not pinned and not counted in
    `CachePressure.drawn`; among the unpinned patches used as recently, they go last. In Design
    note 10's terms the rule is still least recently used among unpinned slots, with the hidden
    patches counted as used and one more tie rule. Also, a patch the selection demanded is now
    draw-pinned as it arrives, as selected and forced ones already were. Before, an ancestor
    requested for its unbaked descendants stayed unpinned until the next retain, so once the
    unpinned patches were used up, a later insert of the same frame could evict it. Otherwise
    eviction is unchanged: deepest first among equals, forced patches and roots never. The
    slots, `exceeded`, refusals and the memory bound are unchanged. T7's digests of the selected
    keys and the demand still match: `hiddenBaked` adds an output and changes nothing selected.
  - _Unpinned, not draw pins._ A version that pinned them like the draw set made one run worse:
    608 coarse re-bakes against 39. That run is a scratch low pass like the test's, at 640 × 360
    and τ = 1 px, tilted 1.5 rad, with 500 slots, which its pins and hidden patches overflow. There
    the hidden patches pushed out the selection's own new leaves. Unpinned, that run is as before:
    39 re-bakes and 25 frames of stand-ins either way.
  - _The ruling's examples, not built._ Depth before recency among unpinned patches, measured
    the same way over the high approach, cut the ridges-off coarse re-bakes to 52. But it raised
    the level-14 evictions to 2,676 (off) and 6,880 (on), and the re-bakes of levels 13–18 within
    1 s to 1,622 and 13,405. Bakes rose to 160 and 460 a second, and the ridged coarse re-bakes
    to 4,848: freshly left fine patches went first and came straight back. Keeping levels ≤ L
    resident does not fit either: the thrash reached level 12, where patches are not few.
  - _Measured_ (CPU only; the ruling's probe loop of select, resolve, retain and bake every
    request at once, an ideal pool, at 16 Hz with the hard bound and seed 7; the high setting's
    981 patches and 1,962 slots unless noted). In the ruling's 2.4 km window
    (`run2.mjs on|off high 1040`, 16 frames after a 3 s warm-up from empty):

    | Before → after                                    | Ridges on      | Ridges off              |
    | ------------------------------------------------- | -------------- | ----------------------- |
    | Frames with a non-resident drawn leaf, level ≤ 12 | 12 → 0         | 6 → 1                   |
    | Levels 2–12 evicted; re-baked                     | 51; 45 → 0; 0  | 19; 13 → 0; 0           |
    | Level-14 evictions                                | 30 → 6         | 0 → 0                   |
    | τ′ at the window's last frame                     | 11.4 px → 1.18 | 6.84 px → 1 (unlimited) |

    The one frame left with ridges off is the first bake of a level-11 patch coming into view,
    which had never been resident. Over the whole approach (960–1,080 s, 1,921 frames after a
    20 s warm-up, with the frames counted over every seen selected leaf):

    | Before → after                                          | High, on      | High, off     | Low, on       | Low, off    |
    | ------------------------------------------------------- | ------------- | ------------- | ------------- | ----------- |
    | Levels 2–12 re-baked within 1 s of eviction             | 2,168 → 0     | 545 → 0       | 714 → 0       | 26 → 0      |
    | Frames drawing a once-resident level ≤ 12 by a stand-in | 967 → 7       | 387 → 5       | 527 → 6       | 34 → 0      |
    | Its largest ρ, px of bound                              | 211 → 19      | 68 → 8.3      | 141 → 12.7    | 33 → none   |
    | Level-14 evictions                                      | 2,406 → 2,178 | 2,187 → 2,166 | 1,077 → 1,070 | 352 → 357   |
    | Bakes a second                                          | 278 → 239     | 127 → 122     | 138 → 119     | 23.3 → 23.0 |

    The stand-ins left were of patches evicted 23–63 s before, out of the selection all that
    time: the cache's capacity, not thrash. The first bakes of patches coming into view (61–126
    frames a run) are unchanged. On low with ridges off, the level-14 evictions rose by 5 (1.4%),
    outside the ruling's window: the hidden patches hold slots that fine patches used before.
    Listing them costs about 0.1 ms of an 8.3 ms selection of 2,643 patches with 559 hidden
    (vitest under load; provisional).

  - _Headroom, for T18 and R10._ The hidden patches peaked at 530 (on) and 389 (off) of the high
    setting's 1,962 slots, and at 354 and 197 of the low setting's 1,296. The draw pins peaked at
    1,389 and 1,208 (high) and 929 and 250 (low), unchanged. Forced patches, draw pins and hidden
    patches together peaked on a single frame at 1,922 and 1,537 (high) and 1,229 and 425 (low).
    No run evicted a hidden patch or a draw pin, evicted a patch in the frame it was stored,
    refused a bake or set `exceeded`. So ⌊slots ÷ 2⌋ holds against the coarse eviction in these
    runs, but on the ridged high approach with only 40 slots to spare. The hidden patches are a
    resident population that neither T18's ⌊slots ÷ 2⌋ nor R10's sizing rule counts, and R10's
    layout of about 430 low slots was not measured.
  - _The acceptance, as read._ F3 asks that "no steady-state frame has a non-resident drawn leaf
    at level ≤ 12". It is read as no once-resident patch, evicted and drawn by a stand-in on its
    return. With ridges off one frame remains, the first bake of a patch never resident. No
    eviction rule can make such a patch resident: the ideal pool bakes after the frame's draw, so
    every patch's first appearance is drawn by its parent for one frame (61–126 frames a run,
    unchanged). `cache.test.ts`'s low pass counts once-resident patches for the same reason. Put
    to the orchestrator with the commit, for the ruling's author.
  - _How it was measured._ `run2.mjs` and `probe2.ts`, adapted from the ruling's scratch copies to
    read this worktree, count every seen selected leaf a frame. `thrash.ts` and `run.mjs` run the
    same loop over the whole approach, with the baked ranges memoised on disk. "Before" is the
    same code with the selection's `hiddenBaked` and `demand` withheld from `retain`, which is
    exactly the earlier rule. The depth-first variant replaces `PatchCache`'s victim choice in
    the probe. Copies of the scripts and their results are in `.git/rm23-scratch/r05-t8-f3/`;
    they run from a worktree's `target/f3-probe/`.
  - _Tests._ `cache.test.ts`:
    - a coarse ancestor of the drawn patches, and a hidden patch beside it, are kept while the
      drawn patches churn;
    - hidden patches go after every unpinned one and before the draw pins, age once no longer
      hidden, and are not counted as pins;
    - a demanded ancestor is pinned from the moment it arrives;
    - under 300 frames of random selections and hidden sets, no slot is used twice or beyond the
      count, and the bytes held follow the slots;
    - the same patches are evicted whatever order they were stored in;
    - a low pass of selection and the cache together (500 slots, an ideal pool, a sliding
      contact) bakes no level ≤ 12 patch twice after a 16-frame warm-up, draws none by a
      stand-in once it was resident, and evicts the same patches on a second run.

    Without the fix, the churn test, the test of the hidden patches' place in the order, the
    demanded-ancestor test and the low pass's re-bake test fail.
    `select.test.ts`: `hiddenBaked` lists only baked patches neither selected nor above a
    selected one, and every baked child of a split patch that is neither; the list is the same
    for the same input and empty with no baked ranges; and `bareKeys` lists the children a split
    left out and the splits with no leaf beneath.
- **R05.T7 perf (c), as built (lane B, 2026-10-04): the per-node costs perf (b) left.** This is the
  next step toward 4d's 2 ms p95 (decision-r05-high-bound.md, reason 4). Selection's output is
  unchanged bit for bit:
  - the selected keys and their order;
  - every number of every bound;
  - the demand with its priorities;
  - `limited`, `limitExcess` and F3's `hiddenBaked`.

  It is built on F3's `hiddenBaked` (the entry above), and `bareKeys` reads the new node layout.
  - _The neighbour probe._ It builds no keys.
    - `stepCellInto` (`patchKey.ts`) writes a folded cell into a reused `CellOut`. `stepCell`, and
      so `edgeNeighbour` and `cornerNeighbours`, now call it.
    - The balance asks of each neighbour only for the leaf over its grandparent cell. It walks up
      the leaf's own ancestors to the first that holds that cell (across a face edge, it starts
      from that face's root), then down. Where that ancestor is the leaf's own grandparent there
      is none, since the grandparent is split.
    - Away from a face's edge, three of the eight neighbours decide. They are (Δi, 0), (0, Δj) and
      (Δi, Δj) for the leaf's outward steps, tried in the order the eight-neighbour scan meets
      them. Each of the other five lies in the parent's own cell or in the parent cell of one of
      those three, so it answers the same.
  - _No `childKeys` tuple and no children array a split._
    - `PatchLeafSet`'s nodes hold their parent and four child fields. The per-level maps by
      `patchKeyIndex` are gone.
    - A split still makes its four keys and its child nodes, one at a time, and `splitBalanced`
      returns one `leaves` array.
    - The journal and the balance's work lists are reused.
    - `PatchLeafSet.leafValues()` (new) returns the leaves as an array, and `values()` yields
      from it. Selection reads it once, for the output map and the demand.
  - _`patchBounds` in scalars._
    - It runs over preallocated scratch, in the same `f64` operations and order, and allocates
      only the bounds it returns. Selection still passes a new bound its `[lowM, highM]` tuple.
    - `Math.hypot` is called only where the squared length is not below 1 − 10⁻¹² of the largest.
      That gives the same maximum for finite metres, and a NaN still comes out as NaN.
    - A call takes about 7.2 µs, down from 24.6 µs (load 3, provisional).
  - _Smaller costs._
    - The output map's key strings are kept in the bounds memo, so they are neither rebuilt nor
      rehashed.
    - The per-level error, ε_n and height ranges are computed once a planet.
    - `inForcedRegion` (`grounded.ts`) is now a loop over `distanceToBoxFromM` (`viewGeometry.ts`).
      It builds no `relativeBounds` objects and no closure. It is still the rule's one definition,
      and selection skips it when there are no contacts.
    - The demand's walk no longer stops at an ancestor already requested. Such an ancestor is
      always the top of its unbaked chain, so the walk ends there anyway and the targets are
      unchanged.
    - Hidden children get no traversal node.
  - _Tests._
    - `select.test.ts` adds whole-output digests (with `hiddenBaked`), recorded before this work.
      The first set is at the 1.5 km pose, at budgets of 981 and 1,952 and with none.
    - The second is a 48-frame approach: 20 km to 300 m, a secondary view and a contact below
      1 km, at budget 981, baking 128 requests a frame from the demand. Its digest folds every
      frame's output.
    - `select.test.ts` also adds a randomized check against the leaf set's former algorithm, kept
      as a string-keyed map. The check covers left-out children, face edges, cube corners and
      rollbacks.
    - `bounds.test.ts` holds `patchBounds` to its former vector form, bit for bit, over 1,350
      patches (every face and level, the corners and edges included), 3 planets and 3 ranges.
    - `patchKey.test.ts` holds `stepCellInto` to the former array form of `stepCell`.
      `cube.test.ts`'s Rust golden still holds the fold itself.
    - `viewGeometry.test.ts` holds `distanceToBoxFromM` to `distanceToBoxM` of `relativeBounds`.
    - F1's and F3's tests and the spike's pinned sequences pass unchanged.
  - _Measured (provisional, Design note 27)._ The harness is `bench.ts`, `ab.mjs` and `build.mjs`
    in `.git/rm23-scratch/r05-b/perfc/`. Its figures are taken at the perf (b) pose: 1.5 km, ridges
    off, 1920 × 1080, fov_h 60°, τ = 1 px, tilted 69°, warm, Node 26. Each is the median of ten
    interleaved runs of the old code (F3 merged) and the new, at loads 5.5–10.7, governor
    schedutil.

    | Run                        | p50 before → after     | p95 before → after     |
    | -------------------------- | ---------------------- | ---------------------- |
    | Budget 981                 | 2.28 → 0.82 ms (0.36×) | 3.67 → 1.54 ms (0.42×) |
    | Budget 1,952               | 4.39 → 1.46 ms (0.33×) | 6.92 → 2.33 ms (0.34×) |
    | No budget (12,731 patches) | 35.0 → 13.4 ms (0.38×) | 41.0 → 16.3 ms (0.40×) |
    | `bench.ts`'s flight        | 2.71 → 1.09 ms (0.40×) | 4.38 → 1.82 ms (0.42×) |

    `bench.ts`'s flight runs at budget 981 with baked ranges: 640 frames at 64 Hz from 6 km to
    300 m at 200 m/s, one view, 16 bakes a frame.

    T13.a's fixed-step windows on high were also run: the fixture's bakes, ridges off, the
    min(hard, 4σ_n) second pass, the real `PatchCache`. Each figure is the median of six
    interleaved runs, at loads 8.6–12.0. They about halve:
    - the approach and flare goes from 0.67 / 1.08 ms (p50 / p95) to 0.32 / 0.43 ms;
    - the low fast pass goes from 0.68 / 0.99 ms to 0.31 / 0.41 ms.

  - _Still open._ At the perf (b) pose under this load, the budget-981 p95 is below 2 ms. These are
    pending for the owner's quiet machine (T14/T17):
    - a `terrain.select` run;
    - high's ruled hard-bound windows with ridges off, which reason 4 measured at 2.3–6.2 ms p95;
    - the same windows ridged, at up to 13 ms (`just descent-demand`'s hard cells, with real
      bakes).

    The remaining costs of a selection are intrinsic or the interface's:
    - each view's excess test, about 15% of a call;
    - the output map and its records, about 8%;
    - the bounds memo's lookups by a non-Smi `patchKeyIndex`, about 6%;
    - the heap's comparisons, about 6%.

    The cache's own `heightRangeM` lookup costs about as much as the memo's, so selection fed by
    `PatchCache` pays it as well.

- **R05.T7 perf (d), as built (lane B, 2026-10-05): the ridged approach under 2 ms.** The record's
  ridged high approach (hard bound, 16 Hz) measured 2.8–3.2 ms p95 on the record's path at loads
  6–17 (lane B's probe, after F4's 29.2 ms was found to be CPU starvation). That is under DN21's
  13.3 ms but above 4d's 2 ms. This step brings it to 1.43 ms, with selection's output unchanged
  bit for bit, as perf (c) defined it.
  - _Profile first._ The approach's frames, ridges on (V8's sampler at 50 µs, from t = 960 s):
    - the excess test, 16% of a call;
    - bounds builds, 19%, two thirds of it the ring's directions;
    - the memo's `Map` lookups by a non-Smi index, 10%;
    - the cache's `heightRangeM`, 10%, by the same kind of `Map`;
    - the balance's walks, about 15%;
    - the heap, 5%;
    - `hiddenBaked`'s walk and second lookups, 4%;
    - the output map, 5%.

    About 60 bounds a contact frame are built: 34 new keys, and 27 whose range tightened as a bake
    landed, on the same key a frame or two later.

  - _What changed._
    - _The bounds memo is a tree._ Each patch has a cell, kept from call to call and reached from its
      parent's cell, so a child's bounds need no lookup. The memo's semantics (when the range must
      be found, when it matches) are the old one's. Past 131,072 cells a call drops the cells none
      of the last 64 calls read, then waits for twice the cells kept (`pruneSelectionMemo`,
      exported for its tests). The old memo dropped the older half of a level at 32,768.
    - _Bounds are packed._ A cell keeps its bounds as 18 numbers in a `Float64Array`, and the
      excess test, the forced-region rule and the requests' priorities read that form:
      `viewExcessPacked`, `inForcedRegionPacked` and `distanceToPackedBoxFromM`.
      `viewExcess`, `inForcedRegion` and `distanceToBoxFromM` pack a `PatchBounds` and call them, so
      each rule still has one form. The `PatchBounds` object is made only for a drawn patch, and
      kept while its range holds.
    - _A patch's geometry is kept._ `patchBounds` is now `patchGeometryInto` (the ring's spheroid
      points and normals, the centre's frame), then `packPatchBounds` (the heights on), then
      `unpackPatchBounds`. Selection keeps the last 512 geometries (1.7 MB), so a bounds rebuilt for
      a tightened range skips about two thirds of the build. On the approach 41% of builds reuse
      one.
    - _The ring's warps once a patch._ The 17 columns' s and the 17 rows' t are computed once a
      build, not twice a vertex. A vertex on a face's edge, whose canonical face may be another,
      still takes the whole path.
    - _The patch cache looks up by a typed table._ `PatchKeyTable` (new, `patchKeyTable.ts`)
      hashes a key's two 32-bit words into an open-addressed `Int32Array`, with linear probing and
      backward-shift deletion, in place of the per-level `Map`s by `patchKeyIndex`. The draw set's
      `residentAt(level, index)` is now `residentOver(key, level)`, a key's ancestor at a level, so
      no index is encoded and decoded again. `insert` refuses a key deeper than `MAX_LEVEL` with a
      `RangeError` before it changes anything, since such a key does not fit the table's words.
    - _No second walks._ A traversal node is marked split as its children are made, so the main
      loop's `isLeaf` walk is gone. The baked children a split leaves out are a bit mask on their
      parent, set when `nodeOf` looked them up, so `hiddenBaked` is `bareKeys` through a filter
      (`BareKeyFilter`, new), with no second lookup and no keys built for unbaked ones.
    - _Smaller costs._
      - The heap keeps each slot's forced flag and weighted excess in typed arrays beside it, with
        the same comparisons in the same order.
      - The leaf set's journal and scratch are count-indexed rather than truncated.
      - A drawn patch's `SelectedPatch` is kept on its cell while its bounds, `forced` and `seen`
        hold.
      - The horizon test tries the top corner nearest the camera first, and the frustum test
        takes a plane's box reach only where the centre is behind the plane. A box's reach is the
        centre's plus terms that are never negative, so neither changes an answer.
    - _The module runner's getters._ Under Vite's module runner, which the record's harness and
      `just descent-demand` run selection under, each read of an imported binding is a getter
      call. The game's renderer, bundled or native ESM, does not pay this. The hot modules
      therefore bind such imports once, at module level:
      - the packed layout's offsets and `NEAR_PLANE_M` in `viewGeometry.ts`;
      - `viewExcessPacked`, `inForcedRegionPacked`, `patchKeyIndex` and `MAX_LEVEL` in
        `select.ts`;
      - `distanceToPackedBoxFromM` in `grounded.ts`;
      - `stToUv` in `bounds.ts`.

      Read through the getters, the packed test was slower than the old one: 28% of a call against
      16%. The function aliases alone are worth 8–11% on the approach.
  - _Bit for bit._
    - `pab.mjs` runs runFixedStep's frame loop for the old code and the new side by side, each
      with its own module instance, memo and `PatchCache`. Every frame it selects with both, in
      alternating order, and compares the two outputs whole: every key in the map's order, every
      bound number by `Object.is` (with the sphere's centre being the box's), `forced` and `seen`,
      the demand's keys, priorities and flags in order, `limited`, `limitExcess` and `hiddenBaked`.
    - It found no difference in the three high hard-bound cells over the whole descent (0–1,230
      s): ridges on at 16 Hz (19,681 frames), and ridges off at 16 Hz (19,681) and 64 Hz (78,721).
      That is 118,083 frames in all. The ridged cell's key hash is the record's `7c53b786241b137a`,
      and the 64 Hz cell's is its `ec4300c0eecb583f`.
    - The unit tests' whole-output digests, recorded before perf (c), pass unchanged.
    - New tests:
      - `viewGeometry.test.ts` holds the excess test to perf (c)'s form bit for bit, over 10,000
        patches seen from 400 cameras near the ground, in orbit and inside patches;
      - `bounds.test.ts` builds bounds from one kept geometry for three ranges against the
        vector oracle;
      - `patchKeyTable.test.ts` checks the table against a `Map` over 40,000 random operations;
      - `select.test.ts` lists filtered bare keys against the filtered list, and checks the
        memo's prune: kept bounds stay the same objects, and the output after a prune is the
        same digest;
      - `grounded.test.ts` holds `inForcedRegion` to the box distance over 289 patches about a
        contact.
  - _Measured (provisional, Design note 27)._ The timed A/B is `ab.sh` in
    `.git/rm23-scratch/r05-b/perfd/`. It runs on the record's path: `rec.mjs` is Vite's runner,
    runFixedStep as runCell calls it, hard bound, high, the saved ranges and no bakes.
    - Old and new ran interleaved by round, each as its own process at nice 0.
    - All of it ran under `just _locked` (972 s), at loads 1.2–5.1. One old round of the
      ridges-off 16 Hz cell met a load of 11.7.
    - The governor was schedutil.
    - Each figure is the median of three rounds; at 64 Hz it is the mean of two.
    - CPU is the thread's own: `process.cpuUsage()` then `process.threadCpuUsage()` (see the
      finding below).
    - Wall time is within 0.04 ms of CPU time at p95 in every approach cell.

    `selectPatches`, CPU p50 / p95 ms, old → new:

    | Segment             | Ridges on, 16 Hz          | Ridges off, 16 Hz         | Ridges off, 64 Hz         |
    | ------------------- | ------------------------- | ------------------------- | ------------------------- |
    | orbit coast         | 1.14 / 1.44 → 0.66 / 0.80 | 0.53 / 0.77 → 0.31 / 0.41 | 0.50 / 0.61 → 0.30 / 0.36 |
    | descent arc         | 1.14 / 1.53 → 0.68 / 0.81 | 0.74 / 1.30 → 0.44 / 0.68 | 0.76 / 1.22 → 0.44 / 0.67 |
    | approach and flare  | 1.61 / 2.68 → 0.87 / 1.43 | 1.33 / 2.54 → 0.68 / 0.85 | 1.09 / 1.75 → 0.63 / 0.75 |
    | low fast pass       | 1.43 / 1.76 → 0.75 / 1.09 | 1.34 / 1.69 → 0.70 / 0.87 | 0.94 / 1.19 → 0.53 / 0.65 |
    | slowdown            | 0.64 / 1.11 → 0.33 / 0.58 | 0.77 / 1.43 → 0.40 / 0.70 | 0.58 / 1.02 → 0.32 / 0.57 |
    | vertical descent    | 0.22 / 0.30 → 0.12 / 0.17 | 0.22 / 0.31 → 0.12 / 0.17 | 0.23 / 0.31 → 0.12 / 0.17 |
    | hover and touchdown | 0.20 / 0.20 → 0.10 / 0.10 | 0.20 / 0.20 → 0.10 / 0.10 | 0.21 / 0.21 → 0.10 / 0.11 |
    - The ridged approach's rounds: old 2.61–2.74 ms p95, new 1.34–1.47 ms.
    - The old ridges-off approach at 16 Hz varied more, from 1.81 to 2.54 ms (5.58 under the load
      of 11.7). The new one stays between 0.85 and 0.87 ms.
    - `pab.mjs`'s paired ratios, measured on the same frames in one process at loads 2–16, agree
      with the A/B. On the approach's p95: 0.48 ridged, 0.47 and 0.54 ridges off.
    - In the game's form the gain is smaller, since the old code's getter overhead is gone too.
      With both variants bundled by esbuild and paired, the ridged approach goes from 2.75 to
      1.77 ms p95 (0.65×). Bundled and run alone at loads 13–21, it goes from 3.59–3.69 to
      1.51–1.71 ms wall.

  - _Finding, for the record's timings: `process.threadCpuUsage()` alone is tick-grained here._
    - The kernel runs tick-based CPU accounting: no `nohz_full`, so no vtime. Its
      `getrusage(RUSAGE_THREAD)` reads the thread's runtime as of the last scheduler update.
    - So a 0.37 ms loop reads 0 or 0.997 ms, and the record's frames came out in multiples of
      about 1 ms.
    - `process.cpuUsage()` (`RUSAGE_SELF`) updates the running thread's runtime first. Called just
      before, it makes the thread's reading exact: the same loop reads 0.372 ms. The pair costs
      about 1.5 µs.
  - _Still open._
    - _Quiet-machine runs._ The loads of 1–5 are near quiet but not quiet. The owner's runs stay
      pending (T14/T17).
    - _The prune's cost._ A prune walks every cell, about 131,000 of them. In a long flight that
      would cost a few milliseconds in the one call that prunes. The record never reaches the limit.
    - _The record's harness._ The module runner's getters still tax the rest of the record's path
      (the cache, the draw set) and any selection code not bound once. Bundling the record's
      runner, or saying so beside its timings, is for lane D or the orchestrator.

- **Deviations in T13.b, as built (the spike scene, 2026-10-03).**
  - _Files beyond the plan's two._ The plan names `spikeScene.ts` and `DescentSpike.tsx` and their
    tests. The build adds:
    - `view/spike/spikeRun.ts` and its test: `prepareDescent`, `SpikeRun` (the frame loop's
      drawing), `SpikeWorkers`, `DEFAULT_SPIKE_WORKERS`, `defaultSpikeWorkers` (Design note 11's
      clamp(⌊threads ÷ 4⌋, 1, 3)) and the metrics seam. The test is in `vitest.config.mts`'s
      `DOM_TESTS`, since it hands the engine canvases.
    - `view/spike/surfaceQuery.ts` and `surfaceQuery.worker.ts` with a test: a spike-only module
      worker. `surfaceQuery.ts` joins `tsconfig.worker.json`.
    - `smoke/spike.ts`: the captures, group "R05.T13.b the spike captures" in `page.ts` under
      `--smoke-captures`.
    - `test/countingRenderEngine.ts`: a target's `resize` now remakes its colour and depth
      handles, as the adapter's does.
  - _The terrain is measured before the run_ (the orchestrator's ruling, 2026-10-03). T13.a's
    altitudes were above the spheroid, and the test planet's relief is kilometres, so the hover and
    the contact could lie inside the ground. `prepareDescent` asks the surface query worker for
    three things:
    - the level table (`levelTable`), for the planet's `PlanetGeometry`; the render thread has no
      other source of it;
    - the landing site's height, from lane A's `surfaceHeightM` export: T4.c's collision
      interpolant, bit for bit with native;
    - a floor under each of T13.a's stretches (`trackStretches`, decision-r05-descent-clearance.md):
      the highest baked vertex plus ε_n over the stretch's patches at its bound level and their
      eight neighbours (T13.a's `stretchKeys`), in one batch, `SurfaceQuery.maxHeightsM(groups)`,
      each distinct patch baked once (about 600);
    - σ_n of every level (`SurfaceQuery.omittedSigmaM`), for T13.c's calibrated pass.

    It then builds T13.a's `DescentProfile` with `{ siteHeightM, stretchMaxHeightsM }`, so the
    script stays a function of the seed and the measured numbers. A profile the floors make
    unflyable throws, and `prepareDescent` raises `DescentRefused`: the spike refuses to fly and
    says `DESCENT REFUSED: terrain cannot be cleared on this seed, relaunch with another seed`.
    `trackPatchKeys` and `TRACK_BOUND_LEVEL` are gone.

  - _The contact._ The craft is a contact while it is grounded or descending (Design note 9),
    judged on `clearanceM`, the height above the site's terrain, not the datum:
    - descending by `isDescending`;
    - or grounded, within `GROUNDED_CLEARANCE_M` of the ground at any vertical speed: the held
      radius r_g, the hull's bounding radius (11.06 m) plus one finest patch, as T13.a's
      `craftContacts` has it, so that an exact hover is a contact (the orchestrator, 2026-10-03).
      T13.a's run takes a provisional 10 m radius where the scene takes the hull's 11.06 m.

    It is also held from its last rising edge to the end of the script (`contactRule`: the 64 Hz
    fixed step, each edge then found by bisection). The level low pass is not a contact.
    - The contact stands at `groundPointM`, the point at the site's height beneath the craft
      (T11.c's to-do), with the hull's bounding radius.
    - Far from the site, where the terrain under the craft is higher or lower, it holds no patch.
      The forced region works where the craft comes down.
    - Every height query and patch key is taken at the bake's direction d (Design note 5): the
      site's `DescentProfile.siteDir` and the track's `DescentPose.groundDir`, both T13.a's, never
      a point's geocentric direction. Lane D's collapse came from that mistake; a test pins it
      here.

  - _The light._ The star lies in the planet's equatorial plane over the site's meridian at
    touchdown: an equinox at local noon.
    - T13.a draws the parametric latitude within ±60°, whose geodetic latitude on WGS 84 is at
      most 60.083°, so the Sun stands 29.9° or more above the horizon (science check,
      2026-10-03).
    - The planet stands 1 au from the star and is held still in the system: its orbital motion
      over the 1,230 s script, about 0.014°, is neglected.
    - The rotation is T13.a's `testPlanetRotationAt`, and the figure is WGS 84's
      (`TEST_PLANET_FIGURE`).
  - _`SYNTHETIC_FIELD_BYTES`_ is 15,000,000 (decimal MB, as R09 Design note 17's figures are).
    `syntheticField()` writes every byte so that the pages are committed. Each pool posts it
    (`DEFAULT_SPIKE_WORKERS.pool`).
  - _The views._
    - The main view is `LitView` with an atmosphere. `LitView` gains a constructor flag,
      `atmosphere`: Hillaire's composite (T12.c) draws over the terrain target into a second
      scratch target, `<view>:spike-sky`, labelled `atmosphere composite`
      (`LIT_VIEW_ATMOSPHERE_LABEL`), and the display pass maps that target.
    - **Fixed in T11.c's `LitView`:** a resize remakes a target's textures, and `LitView` kept the
      old handles. The spike's first capture failed with "texture spike:spike-hdr:depth was not
      made by this engine". It now renews them on resize, and a regression test fails without
      the fix.
    - Two wireframe instruments sit side by side under the main view:
      - `ORBIT`: seven planetary radii over the craft (`ORBIT_INSTRUMENT_RADII`, so the disc fits a wide instrument), with the graticule, the craft and its path.
        The path is the craft's `predictedPath`, 256 body-fixed points from now to the end.
      - `CRAFT`: R02's chase preset.

      Each has R02's label block beside its canvas, not over it, at R02's default 60° field of
      view.

    - Each canvas is focusable, named (`VIEW, SPIKE LIT, SCRIPTED`; `VIEW, WIREFRAME, ORBIT,
SCRIPTED`; `VIEW, WIREFRAME, CRAFT, CHASE`), and points with `aria-details` to the one list of
      the scene's bodies and craft. All three lists would be identical. The list's selection is
      drawn as the bracket reticle on both instruments. The lit view draws no marks.
    - Each canvas's role is `img`, since it is a picture, not a control, and a lint suppression
      says why.
    - The main view's label block keeps R02's lines with `STYLE SPIKE LIT` and `CAMERA SCRIPTED`.
      It states `TEST PLANET: provisional, dry and hand-parameterised` and the debounced terrain
      line.
    - The page fills the window under a header strip with the `MEASUREMENT` banner. Only the list
      scrolls.
    - One console panel, `DESCENT`, shows at 4 Hz:
      - the spike seed in 16-digit hex, the `QUALITY` and the segment;
      - the script time in s;
      - `HEIGHT ABOVE SITE`: to 0.1 m below 10 m, whole metres to 10 km, then km to one decimal,
        with hysteresis at 9.5 and 10.5 km;
      - the ground speed, whole from 100 m/s; the vertical speed, signed, to 0.01 m/s;
      - `CAMERA ELV`, signed;
      - `SITE HEIGHT`, signed whole metres;
      - `FINEST TERRAIN HELD` `YES`/`NO`;
      - the patch counts, which break only at their `·`.

      Each reading's field holds its longest value, right-aligned. Every reading shows `—` while
      no run draws: a run that stops clears them rather than freezing them.

    - The statuses use fixed words with the remedy, and the cause goes to the log:
      - `MEASURING TERRAIN: landing site and ground track`;
      - `TERRAIN NOT MEASURED: surface query failed, relaunch to retry`;
      - `DESCENT REFUSED: terrain cannot be cleared on this seed, relaunch with another seed`;
      - `GRAPHICS NOT AVAILABLE: views could not be made, relaunch to retry` (`ViewDisplay`'s
        words);
      - `GRAPHICS NO ADAPTER: views not available, relaunch to retry`.

      A frame the engine refuses stops the run with the same fault.

    - _The labels, decided 2026-10-03_ (decision-r05-spike-ux.md, on the owner's delegation):
      - `MEASUREMENT` replaces `TRAINING`;
      - `SETTING`, `CLEARANCE` and `GROUND CONTACT` become `QUALITY`, `HEIGHT ABOVE SITE` and
        `FINEST TERRAIN HELD`;
      - the measuring status leads with its verb;
      - `CAMERA ELV` and `DESCENT REFUSED` are added.

      The guide gains the measurement banner in Layout, the `SCENE` row's `DESCENT SPIKE`, and 18
      drafted nomenclature rows. They were signed off on 2026-10-05 under the owner's delegation
      (decision-r07-owner-ux-signoff; R07.T19.d), with one amendment: `TEST STAR`, `TEST PLANET`
      and `TEST CRAFT` join one row with the kept scenes' test designations. `ELV`'s row is widened
      for `CAMERA ELV`.

    - The orbit instrument's `CAMERA` reads `SCRIPTED`, not `FREE`. It keeps only `FRAME`, `TIME`,
      `STYLE`, `CAMERA` and `FOV`, as the craft instrument does. It draws the craft as a target
      mark at its fixed size: as the own ship the craft carried none, and its hull is below a
      pixel at seven planetary radii.
    - The predicted path runs from the script's time to its end, dashed; the flown part is not
      drawn, and at the end there is none.
    - Until another mark is chosen, the orbit instrument marks the craft with the bracket reticle,
      so that it reads against the graticule.
    - The patch counts stand one to a line, four lines always, so the panel never changes height.
    - The side column is `clamp(27rem, 25vw, 30rem)`. Instrument label values never wrap, and an
      instrument's canvas may narrow to 5rem.
    - Checked on hidden, offscreen full-window screenshots at 1920 × 1080 and 1280 × 720 (an
      Electron `--require` capture hook on the built app; never shown, fresh profile, process
      group killed). Both fit with nothing cut off. At 1280 × 720 the orbit's canvas is narrow, and
      its disc small.
    - **Found for lane D:** at seed 7, the orbit coast shows a vertical speed of +18.37 m/s while
      the height above the site climbs from 400.0 to 401.0 km. The coast should be level.
      **Fixed (2026-10-04):** the coast is now a `level` segment (T13.a's "The orbit coast flies
      level", below).
  - _The metrics seam_ (the orchestrator, 2026-10-03, agreed with lane D).
    - `DescentSpike` takes `engineSource` (T13.c passes the source that wraps the GPU through
      `pipelineShim` or `capture`) and `listeners`: `onFrame(SpikeFrameSample)` and
      `onPatch("requested" | "baked" | "resident", key)`. "Requested" is a key's first request.
    - The listeners are read through a ref, so a new object does not restart the run. T13.c
      builds `SpikeMetrics` over them.
    - `engineFrame` and `patchesCalibrated` stay T13.c's and T14's: the engine has no synchronous
      frame counter.
    - T13.c adds the `--vertex-path` and `--normals` props.
    - Spike runs alone record `performance.measure` spans:
      - `terrain.select`, through TerrainPass's new `measureSelection` option;
      - `terrain.frame`;
      - `spike.segment:<name>`, at each segment's end, with the last closed when the script ends
        or the run is disposed.

      Each span is an entry the browser keeps: measurement overhead. T11.c's "no object after
      warm-up" holds for the ordinary pass, which measures nothing. TerrainPass also gains
      `onResident`.
  - _T11.c's reselection distance_ is now the box distance (`distanceToBoxM`), as selection
    measures it. A coarse patch's bounding sphere often held the camera, which clamped the rule
    to one finest patch and reselected on tiny moves (lane B's finding).
  - **Result, 2026-10-03** (SwiftShader, `just test-render --captures`, both variants, no
    uncaptured GPU error). Seed 5, ridges off. The site is 847.3 m and the track bound 957.9 m.
    The contact starts at 1,025.4 s and is held from 1,105.4 s of 1,230 s. Three shots:
    - the descent arc (high, 400 s): 46 patches;
    - the low pass (low, 1,095 s, clearance 414 m): 48 patches;
    - touchdown (low, end, clearance 1.0 m, contact held): 73 selected and 5 drawn, the rest the
      unseen forced region.

    Each streamed everything, with nothing standing in or missing and `limited` false, and both
    instruments drew. The lane viewed the PNGs (kept in the scratch directory):
    - the arc shows the curved limb with a blue rim under a black sky;
    - the low pass, pitched 30° down, and the nadir hover show ground filling the frame;
    - the orbit instrument shows the graticule and the craft's path.

    The on-screen look stays pending for the owner.
- **Deviations in T13.a, as built** (2026-10-03).
  - `view/spike/descentProfile.ts`: `landingSiteOf(seed)` (SplitMix64 from the seed, uniform in
    direction over ±60° parametric latitude, a geodetic ±60.083°, so the scene's Sun stands at least
    29.9° high, and in azimuth), `DescentProfile(figure, site, terrain?)` with `durationS`,
    `segmentAt`, `segmentSpans`, `positionAt` and `poseAt(tS): DescentPose`, and
    `DESCENT_SEGMENTS`, the table as data with each segment's vertical shape. The vertical speeds
    are re-fitted, not the durations: each boundary altitude holds exactly with the 5 s (1 s)
    velocity blends, so a level segment before a descending one climbs gently (the orbit coast by
    about 18 m/s, until it was flown level; the unlifted low pass by 0.076 m/s, 1.9 m at most, and
    not at all when the slowdown is split or the pass lifted). The horizontal speed is the ground
    speed of the track's point, body-fixed, on a great circle of radius (a + c) ÷ 2 = 6,367.4 km
    (not the IUGG mean radius (2a + c) ÷ 3, 6,371.0 km). The camera at 400 km moves 6.3% faster
    than that, and the coast's figure is kept as an upper bound (decision-r05-coast-speed.md); the
    camera looks along the track, pitched 30° down at 300 m/s and above, turning to the nadir at
    rest. **Corrected (2026-10-04):** the orbit coast is now flown level, and the arc blends in
    from it (below).
  - _Terrain (orchestrator's ruling, 2026-10-03):_ `terrain` takes `siteHeightM` (the terrain's
    height at the site) and `trackMaxHeightM` (an upper bound on the terrain under the low pass's
    track, baked ranges plus ε over the patches under it), both measured once by the caller (lane
    C's spike worker) so that the script stays a pure function of the seed and the two numbers.
    Every altitude and `groundPointM` sit on the site's height (`DescentPose.altitudeM` above the
    spheroid; the added `clearanceM` above the site, which the demand prediction reads); the low
    fast pass, and its neighbours' ends that meet it, fly 300 m above `trackMaxHeightM`. Other
    segments do not follow the relief along the track, which is fine above the low pass's
    altitude; the vertical descent and the hover are over the site itself.
    **Superseded (2026-10-03, decision-r05-descent-clearance.md):** the single track maximum gave
    way to per-stretch floors, below.
  - `view/spike/rotation.ts`: `testPlanetRotationAt(tS)`, a spin about the body-fixed z axis at
    the Earth Rotation Angle's rate, ω = 2π × 1.00273781191135448 ÷ 86,400 s (IERS Conventions 2010,
    eq. 5.15), with the angle reduced from whole seconds. **Corrected (T13.a's re-check):** the
    plan's 86,164.0905 s is the sidereal day, measured against the precessing equinox; a body frame
    that does not rotate turns with the stellar day, 86,164.0989 s (orchestrator's ruling).
  - `view/spike/demand.ts`: the closed form at k = 5 (`closedFormDemandPerS`, constants 8k² and
    3πk² ÷ ln 2), the per-level prediction (`perLevelDemand`, `levelRatio`, `capAltitudeM`) and
    `boundedPlanet(planet, rule, omittedSigmaM)`, the one place min(ε_n, 4σ_n) is built, keeping
    the hard bound where σ_n = 0. σ_n is lane A's `omittedSigmaM(level, ridges)` export, passed in
    by the caller (only workers and tests load the module); the interim table is gone.
  - `view/spike/fixedStep.ts` (`runFixedStep`, `segmentFigures`) and `view/spike/demandRecord.ts`
    (`runCell`, `measureTerrain`, `testWindows`, the fixture's format, `demandSummary`), with
    `view/spike/testPlanetFigure.ts` (`TEST_PLANET_FIGURE`, WGS 84). The run uses the terrain's
    own `PatchCache`, `DrawSetResolver` and `retain` (lane B's 5d90fab; an earlier cache of the run's
    own, which kept no ancestors, made the selection churn), in the terrain pass's order: select
    with the cache's baked ranges, resolve, retain, then bake. The pool is ideal: every request is
    baked and inserted the same frame, with its real baked range; the demand's breadth-first rule
    still costs a deep patch a frame per unbaked ancestor. So the measured demand is the patches
    baked a second. Each frame records the patches, the bakes, `limited`, the stand-ins and missing
    patches, the `selectPatches` time and D; bake time is kept out of the wall-time cap. Settings:
    high 1080p, τ 1 px, low 720p, τ 2 px, 60°, each with its own `terrainSlotLayout` and a budget of
    ⌊slots ÷ 2⌋. The record's terrain (`measureTerrain`): the site's height is the module's
    `surfaceHeightM` (T4.c's collision interpolant) at the site's direction; the track maximum is the
    highest baked height plus ε_14 over the level-14 patches under the low pass's and the
    slowdown's track, sampled every 250 m, a true bound. On seed 7 the site lies at −1,845.8 m.
    **Corrected (2026-10-03, lane B's diagnosis):** every height query and patch key takes the
    direction d of the spheroid point M·d (Design note 5), which the profile now exposes as
    `DescentPose.groundDir` and `DescentProfile.siteDir`, with `datumDirection(figure, p)` =
    M⁻¹p ÷ |M⁻¹p| for a point on the spheroid. The first record read the geocentric p ÷ |p|,
    0.036° (4 km) off at seed 7's site, and found −1,953.2 m there: the camera hovered 107 m
    underground looking down, so selection rightly culled everything. `siteHeight.wasm.test.ts`
    holds the measured height inside the finest bake's range under the site.
  - _The craft's contact_ (`craftContacts`): one contact at the ground point while
    `isDescending` holds or the craft is grounded, its clearance within the held radius r_g (its
    10 m plus one finest patch), so that an exact hover (vertical speed 0) keeps its forced
    region (Design note 9's grounded bodies).
  - _Scope (orchestrator's ruling, 2026-10-03):_ the plan's whole-descent fixed-step test is split.
    The unit test (`demandRecord.test.ts`) pins the selection hash of short windows, 1 s measured
    after 1 s of warm-up, ending 2 s before each segment's end, on both settings, under
    min(hard, 4σ_n), ridges off, reading baked ranges, the level table and σ_n from
    `view/spike/fixtures/descentRanges.txt`, which `just descent-demand --write-fixture` writes
    from the module (TEST_PLANET_VERSION 2; regenerate it with the module). The whole descent is
    the command `just descent-demand` (`apps/hyperion/scripts/descentDemand.mjs`): cells
    {hard, min(hard, 4σ_n)} × {ridges off, on} × {high, low}, a configurable rate (64 Hz by
    default) and a wall-time cap (2 h, shared by the cells), writing
    `docs/measurements/descent-spike/<date>-demand-<rules>.{json,md}` with any truncation, the rate
    and the frames covered stated. The ridged planet's 4σ cells are labelled "statistical, not a
    bound on the ridged planet (bound.rs finding)".
  - **Finding: D's factor of two** (windows re-measured 2026-10-03 after the direction fix and
    over the stretches' floors, D at the height above the floor). In the windows (min(hard, 4σ_n),
    ridges off), demand ÷ D is, high then low: orbit coast 0 / 0 (D 0.9 and 0.04 a second,
    nothing new selected), descent arc 0.57 / 0.47, approach and flare 1.22 / 1.23, low fast pass
    0.74 / 1.43, slowdown 0.30 / 1.28, vertical descent 0 / 0 (D 81 and 25), and the hover 0
    against 0 (re-measured after the clearance follow-up, 2026-10-03). The test asserts the six
    within a factor of two: high's arc, approach and low pass, low's approach, low pass and
    slowdown. **Re-measured at τ ÷ 1.1** (2026-10-05, decision-r05-record-tau.md, D at the same
    tolerance), high then low: orbit coast 4.41 / 0 (D 0.9 and 0.01 a second), descent arc
    0.49 / 0 (D 17.9 and 2.4), approach and flare 1.32 / 1.33, low fast pass 0.64 / 1.68,
    slowdown 0.61 / 1.09, vertical descent 0 / 0 and the hover 0 against 0. The test now asserts
    high's approach, low pass and slowdown, and low's approach, low pass and slowdown. High's arc
    misses at 0.494: its window's demand is unchanged at 8.9 a second while D rose 16%, and the
    window is far from the budget (about 210 patches), so the cause is the frustum's chord below.
    High's slowdown came within (0.30 → 0.61). Two causes are clear. D assumes a ring all
    round, 4k patches along the leading edge, where the 60° frustum along the track sees the edge's
    chord, about 4k tan(φ ÷ 2), 0.58 of it; and below the cap altitude (about 89 m on high) D's
    vertical term stays positive (h floored at the cap) while nothing new is selected, which is the
    vertical descent's window (about 54 m to 33 m). On the low setting D is also low, since below
    k ≈ 3 the quadtree's granularity floors the count (Design note 19's "about a quarter").
    **Proposed correction, not the gate:** D_frustum = Σ_L 4 k_L tan(φ_x ÷ 2) v ÷ S_L +
    (3π k² ÷ ln 2) |ḣ| ÷ h above the cap and the vertical term zero below it. The whole-descent
    record has the per-segment figures. For T18 and T19.
  - **Record: the whole descent under min(hard, 4σ_n)** (superseded by F4's record below;
    2026-10-03, `docs/measurements/descent-spike/2026-10-03-demand-calibrated.{json,md}`, seed 7,
    64 Hz,
    78,721 frames a cell, none truncated, load 13–43, timings provisional). The site is at
    −1,845.8 m (ridges off) and −1,644.6 m (on); the low pass is lifted 418.6 m (off) and 64.4 m
    (on) by its floors, and every stretch keeps its clearance (least margin 0.0 m). `limited` is
    0% everywhere, so the budget never binds under this rule. Demand ÷ D, whole segments, high
    then low, ridges off: orbit coast 2.02 / 6.63 (D near 0), descent arc 0.75 / 1.78, approach and
    flare 0.75 / 1.77, low fast pass 0.73 / 1.74, slowdown 1.19 / 7.07, vertical descent 0 / 0,
    hover 0 / 0. Ridges on: 2.14 / 0, 0.72 / 1.99, 2.50 / 19.07, 0.71 / 1.92, 0.60 / 5.33, 0 / 0,
    0 / 0. High's moving segments lie within a factor of two with ridges off, and all but the
    approach with ridges on; the low setting's prediction is too low wherever the quadtree's
    granularity floors its count (Design note 19's "about a quarter", worst in the ridged approach
    and both slowdowns), and below the cap altitude (the vertical descent) nothing new is selected
    while D stays positive, as the windows found. `selectPatches` p95 is 0.2–2.7 ms in every
    segment, against the 2 ms budget of decision-r05-patch-demand.md 4d on high's approach and
    slowdown at most (single calls reach 170 ms at the cold start and at segment changes,
    provisional under load).
  - **Record: the whole descent under the hard bound** (2026-10-04,
    `docs/measurements/descent-spike/2026-10-04-demand-hard.{json,md}`, seed 7, none truncated).
    The four cells ran as four parallel processes, each under its own selection-time and wall-time
    caps (`--wall-cap-hours`, added after the machine's freeze killed a sequential run that writes
    only at its end; the record is now also rewritten after every cell). Ridges on, high ran at
    16 Hz, a stated sampling reduction. The orbit coast still has seed 7's ~18.4 m/s climb (a
    T13.a fix is queued), so its figures may shift. Under the hard bound the budget binds only with
    ridges on: high is `limited` 100% of the orbit coast and the arc and 74% of the approach (980 of
    981 patches), low 48% of the arc and 25% of the approach; with ridges off it never binds. Demand
    ÷ D, high then low, ridges off: coast 0.65 / 0.67, arc 0.62 / 0.82, approach 0.59 / 0.80, low
    pass 1.15 / 1.31, slowdown 0.84 / 1.94; ridges on: 0.13 / 0.58, 0.11 / 0.50, 0.69 / 1.79,
    0.97 / 1.16, 0.50 / 1.19; the vertical descent and the hover select nothing new (0) below
    the cap altitude, as before. So under the hard bound D predicts every unbudgeted moving
    segment within a factor of two on both settings, which the 4σ rule did not on low (its k is
    small enough that the quadtree's granularity floors the count), and where the budget binds the
    demand falls well under D, as a cap must. The low fast pass's demand peaks at 357 a second
    (high, ridges off) and 374 (high, ridges on, 16 Hz) against D's 310 and 385. `selectPatches`
    p95 is 0.3–13.1 ms, over the 2 ms budget on high in every moving segment (provisional; four
    processes at once). Ruled on this record (2026-10-04, `decision-r05-high-bound.md`): both
    settings keep the hard ε_n plus the sagitta, and min(hard, 4σ_n) stays a recorded second pass
    only. A probe of the ridged high cell (16 Hz, ideal pool) puts the budget's effective tolerance
    τ′ (the largest ρ of a resident drawn leaf) at 2.1–3.0 px over the coast and the arc and
    1.2–2.1 px in the approach, with 981 patches against about 4,300–11,700 unbudgeted over the
    coast and arc. T6's ratios put that at 0.4–1.0 px of measured error, where min(hard, 4σ_n)
    reaches about 1.9 px at levels 4–8. On the low setting, the ridged arc reaches τ′ 2.4–2.6 px
    against τ = 2 px. The next record adds, per segment, τ′'s p50, p95 and maximum over limited
    frames (`Selection.limitExcess`) and the share of frames with a stand-in at level 12 or
    coarser. It re-runs the four hard cells after the coast fix. **Superseded** by F4's record
    (below).
  - **Record: F4's figures over the whole descent** (2026-10-05, F4 of
    `decision-r05-high-bound.md`; `docs/measurements/descent-spike/2026-10-05-demand-hard.{json,md}`
    and `2026-10-05-demand-calibrated.{json,md}`, record version 2, seed 7, no cell truncated).
    **Superseded** by the re-run at the pass's τ ÷ 1.1 (below), since it selected at τ.
    - _What was run._ All eight cells were re-run in full, with the level coast, F1–F3 and
      selection's perf (c). Ridges on, high ran at 16 Hz again. Each cell ran as its own process,
      nice, at most four at once, without the heavy-test lock (CPU only). The high hard cells were
      stopped twice, once to free the lock and once by a power-off, and rerun from the start
      under caps of 2 h (selection) and 4 h (wall). The two new records supersede `2026-10-04-demand-hard` and
      `2026-10-03-demand-calibrated`. Those are kept, with a banner that marks their coast and arc
      rows stale, since the high-bound ruling was made on them.
    - _The tooling_ (`fixedStep.ts`, `demandRecord.ts`, `scripts/descentDemand.mjs`).
      - Each frame records:
        - τ′ = τ × max(1, `limitExcess` ÷ w) (`effectiveTolerancePx`);
        - the drawn stand-ins of level 12 or coarser (`coarseStandIns`);
        - whether a stand-in covers a return, a patch the cache evicted earlier and selection
          wants again;
        - the largest ρ they draw: the stand-in's level's error at a covered patch's box
          distance, in pixels of bound;
        - the forced region's bakes.
      - Each segment adds:
        - τ′'s p50, p95 and maximum over the limited frames;
        - |Δτ′| between consecutive selections where either is limited, its ratio, and the share
          of steps beyond the morph bands' margin of 1.1 (`BAND_MARGIN`, held to the pass's
          1 + `RESELECT_FRACTION`);
        - the stand-in shares and their largest ρ;
        - the forced bakes a second.
      - Version 2 of the record carries each cell's caps, wall time and load, and notes.
        `--merge` writes one record from the per-process ones, and `--note` adds a note.
    - _Deviation: stand-ins counted from the draw set._ F3's acceptance read non-resident selected
      leaves of level 12 or coarser. F4 asks for "a stand-in at level 12 or coarser". A stand-in
      also draws the resident patches beneath it, since balance and forced splits are not gated,
      and it covers level-13 leaves. Counting the draw set's stand-ins is the wider reading.
    - _The record's tolerance_ (decision-r05-record-tau.md, 2026-10-05): the record selects at
      the pass's τ_sel = τ ÷ (1 + `RESELECT_FRACTION`), and D is computed at the same tolerance
      in the record and the spike's results. A probe found that at τ the record ran 12–22% low
      in patches and 0–19% in demand where unlimited. It also missed that the high setting's
      ridges-off arc hits the 981-patch budget (100% limited at t 703–957 s, τ′ 0.92–0.96 px).
      Frames the budget already limits select the same either way (the greedy orders by ρ), so
      their τ′ is unchanged. Superseded by the re-run of 2026-10-05 (record version 3, below).
    - _τ′, hard bound, ridges on._ The probe's 2.1–3.0 px holds. With ridges off neither setting
      is ever limited.

      | Segment            | High (τ 1 px), p50 / p95 / max | Low (τ 2 px), p50 / p95 / max |
      | ------------------ | ------------------------------ | ----------------------------- |
      | orbit coast        | 2.07 / 2.11 / 2.11             | unlimited                     |
      | descent arc        | 2.56 / 3.27 / 3.34             | 2.48 / 2.77 / 2.81            |
      | approach and flare | 1.94 / 2.65 / 2.66             | 2.23 / 2.25 / 2.26            |
      | later segments     | unlimited                      | unlimited                     |

    - _Δτ′._ Small everywhere: p95 at most 0.01 px and maximum 0.04 px. Δτ′ ÷ τ′ is at most
      0.019, so a vertex inside its band steps by about 0.1 in morph factor at most. No step
      crosses the margin of 1.1. High's steps are 16 Hz ones, each spanning four 64 Hz steps.
    - _Coarse stand-ins_ (share of frames, then in brackets the share for a return; then the
      largest ρ for a return):
      - High, ridges off: about 4–7% of frames from the coast to the low pass, mostly first bakes.
        Returns are 0.2% in the approach (up to 16.4 px of bound) and 1.1% in the low pass
        (29.5 px).
      - High, ridges on (16 Hz): 44.8% (8.5%) in the coast, 41.7% (0.7%) in the arc, 20.1% (1.1%)
        in the approach and 13.1% (5.2%) in the low pass. The returns reach 2.1, 3.1, 45.1 and
        67.6 px.
      - Low: at most 6.2% of frames, with returns at most 0.8%.
      - F3's 2.4 km window stays clean. Over the whole descent returns remain, as T8 (as built)
        found: the cache's capacity, not thrash.
      - The largest ρ is in the slowdown: 287 px (high, ridges off) to 1,015 px (high, on), on
        0.5–2.6% of frames, mostly first bakes. As the camera turns towards the nadir, the newly
        seen ground and the forced region under the craft are drawn by a coarse ancestor until
        the bakes descend, a level a frame. Those patches' bounds are inherited from that
        ancestor and reach near the camera. Being in bound terms, these ρ overstate the drawn
        error near the ground.
    - _The forced region's bakes_ (a second, averaged over each segment's whole span):

      | Segment            | High, off | Low, off | High, on | Low, on |
      | ------------------ | --------: | -------: | -------: | ------: |
      | approach and flare |       0.8 |      1.7 |     84.8 |    81.0 |
      | slowdown           |      10.7 |     11.5 |     15.1 |    11.5 |

      Every other segment bakes none. The ridged approach ends lower (369 m above its floor,
      against 508 m with ridges off), so its contact descends for longer. The probe's 100–260 a
      second was a 1 s window at 2.4 km. For T18's descending thresholds (Design note 9).

    - _Demand ÷ D, hard bound_ (high then low). The coast and arc rows are now current. From the
      approach on, F3 lowered the bakes by up to about 4% (high's low pass with ridges off: 357 to
      348 a second). The exceptions are the ridged approaches, both limited: high's fell from 278
      to 240 a second, low's from 139 to 125. D predicts every unbudgeted moving segment
      within a factor of two on both settings but one. High's ridged slowdown is at 0.497, just
      outside; it was 0.50 before as well.

      | Segment            | Ridges off  | Ridges on   |
      | ------------------ | ----------- | ----------- |
      | orbit coast        | 0.66 / 0.68 | 0.13 / 0.59 |
      | descent arc        | 0.61 / 0.81 | 0.11 / 0.49 |
      | approach and flare | 0.56 / 0.79 | 0.60 / 1.62 |
      | low fast pass      | 1.12 / 1.29 | 0.94 / 1.13 |
      | slowdown           | 0.83 / 1.93 | 0.50 / 1.18 |

      High's coast now selects 455 patches (maximum 482) with ridges off, at 8.7 a second
      against D's 13.2. Its arc selects 662 (915), at 16.1 against 26.4.

    - _min(hard, 4σ_n)._ `limited` is 0% everywhere, as before. Demand ÷ D, high then low: ridges
      off, coast 2.07 / 6.71, arc 0.75 / 1.79, approach 0.74 / 1.77, low pass 0.72 / 1.71 and
      slowdown 1.19 / 7.04; ridges on, 2.16 / 0 (D 0.0), 0.72 / 1.99, 2.41 / 18.67, 0.71 / 1.91
      and 0.59 / 5.31.
    - _`selectPatches` p95, hard bound_ (provisional, at loads of 4–34, four processes at once).
      The figures of record are perf (c)'s A/B and the owner's quiet-machine runs.
      - High, ridges off: coast 4.7 ms, arc 2.4, approach 8.3, low pass 8.1, slowdown 6.5.
      - High, ridges on: 3.9, 5.2, 29.2, 4.6 and 1.7.
      - Low: 1.7 ms at most in every segment.
      - Lane B reproduced the ridged high cell bit for bit (2026-10-05): its approach p95 is
        2.8–3.2 ms on the record's path at load 6–17 (ridges off 1.7–1.8 ms); the 29.2 ms was
        the cell's nice-10 process starved by the nice-0 integration `just ci` runs (one nice-0
        competitor on its core gives 28.96 / 55.72 ms). Record version 3 therefore times each
        selection on the thread's CPU clock too (below).
  - **Record: the whole descent at the pass's τ ÷ 1.1** (2026-10-05,
    `decision-r05-record-tau.md`;
    `docs/measurements/descent-spike/2026-10-05-demand-hard-2.{json,md}` and
    `2026-10-05-demand-calibrated-2.{json,md}`, record version 3, seed 7, no cell truncated). It
    supersedes F4's two records, which are kept with a banner because the high-bound ruling, F4's
    handoff and this plan cite them.
    - _The tooling_ (24cd2cf, 8c8910f).
      - `RESELECT_FRACTION` and `selectionTolerancePx(τ)` = τ ÷ (1 + `RESELECT_FRACTION`) moved to
        `view/terrain/selectionTolerance.ts`, which does not load the engine. The terrain pass,
        `fixedStep.ts` and `spikeHarness.ts` take τ_sel from it, the pass's by the same
        expression, bit for bit. `BAND_MARGIN` is now 1 + `RESELECT_FRACTION`, so its equality
        test went.
      - The record selects at τ_sel, its τ′ is τ_sel × max(1, `limitExcess` ÷ w), and D is
        computed at τ_sel. `SETTING_VIEWS` keeps the settings' τ of 1 and 2 px (Design note 7).
      - It still selects every frame, without the pass's cadence. The pass's selection at a frame
        is the record's at a pose at most 0.1 × d_min earlier, which moves the demand in time, not
        in size. The record's header says so.
      - Version 3 adds, per cell, τ and τ_sel (`tauPx`, `selectionTauPx`). Per segment it adds
        the share of the limited frames whose τ′ exceeds τ (`limitedOverTauFraction`, null where
        none is limited), and `selectPatches`' time on the thread's CPU clock beside its
        wall-clock time (`selectCpuMs`, with its p50, p95 and maximum). The CPU clock is
        `process.threadCpuUsage`, user and system, which the script passes in, since renderer code
        does not reach `process`. The summary calls the wall-clock times upper bounds under load.
      - Tests hold the pass's and the record's selection τ to
        `TERRAIN_SETTINGS[s].tauPx / (1 + RESELECT_FRACTION)` on both settings, the unlimited
        frames' τ′ to τ_sel, and the frames' and the spike's D to `perLevelDemand` at τ_sel.
    - _Deviation: the spike's k_n moves with D._ `SpikeRecorder` computes D under both bounds,
      and each level's k_n, at τ_sel. The ruling names D alone, but D's per-level term is
      4 k_L v ÷ S_L, and Design note 18 records the k_n that selection's τ rests on. The results'
      k_n are therefore 1.1 × T6's at the setting's τ, and the cap k S_finest is about 98 m on
      high, not 89 m. The committed `2026-10-05-effect-low*` results carry D and k_n at τ under the
      same results schema. They are provisional harness proofs and are not re-run.
    - _The pinned windows, re-blessed._ With the selection τ forced back to the setting's τ (a
      local edit, reverted), all 14 hashes pinned before reproduced, both against the module and
      against the fixture, and `--write-fixture` rewrote the fixture byte for byte. So the
      tolerance is the only input that moved. At τ_sel, 12 of the 14 hashes moved; high's vertical
      descent and hover did not, since their selections are the same at either tolerance. The
      fixture grew from 108,446 to 121,270 B (+11.8%, against the ruling's expected 20%), with
      lines only added. For `WITHIN_TWO`, see "Finding: D's factor of two" above.
    - _What was run._ All eight cells, as F4 ran them: seed 7, 64 Hz, ridges on and high at
      16 Hz, each cell its own process, nice, at most four at once, without the heavy-test lock.
      New this time:
      - each cell ran in its own scope capped at 5 GiB with no swap (`common.md`, CAP EVERY RUN),
        under caps of 2 h (selection) and 4 h (wall), and a kill at 4 h 5 min;
      - RSS stayed at 0.3–0.6 GiB a cell;
      - the cells ran twice, at 24cd2cf and then at 8c8910f with the CPU clock, and every figure but
        the times agreed, the eight hashes included, so the clock does not move selection. The
        record is the second run.
      - The files are named `-2`, since `recordStem` dates a record by its start.
    - _Where the budget binds, hard bound._ `limited` in F4's record, then in this one; after the
      semicolon, the share of the limited frames whose τ′ exceeds τ:

      | Cell             | Descent arc       | Approach and flare | Other segments          |
      | ---------------- | ----------------- | ------------------ | ----------------------- |
      | high, ridges off | 0% → 34%; 0%      | 0% → 30%; 0%       | 0%                      |
      | low, ridges off  | 0%                | 0%                 | 0%                      |
      | high, ridges on  | 100% → 100%; 100% | 74% → 84%; 88%     | coast 100% → 100%; 100% |
      | low, ridges on   | 48% → 58%; 82%    | 25% → 29%; 88%     | 0%                      |
      - _High, ridges off._ The budget binds on 34% of the arc's frames (about 310 s of its
        900 s, the probe's t 703–957 s among them), at τ′ 0.94 / 0.96 / 0.96 px (p50 / p95 /
        max), and on 30% of the approach's, at 0.92 px. τ′ never exceeds τ, so the drawn bound
        stays within 1 px at each selection, but `DETAIL LIMITED` shows there. With τ′ above
        τ_sel, the cadence no longer holds ρ ≤ τ between selections: τ′ ÷ 0.9 is about 1.07 px
        (decision-r05-record-tau.md). Since T11.c's re-selection move of m ÷ (1 + m) of d_min
        (2026-10-05, Risks, "The cadence's 1%"), the bound between selections is 1.1 τ′, at most
        about 1.06 px.
      - _Ridges on._ F4's τ′ stands where the budget already bound. High's coast is
        2.07 / 2.11 / 2.11 px and its arc 2.56 / 3.27 / 3.34 px, both identical. The approach's
        p50 fell from 1.94 to 1.62 px, as more of its frames are limited, by less, and its
        maximum stays 2.66 px. Low's arc is 2.39 / 2.77 / 2.81 px and its approach
        2.22 / 2.25 / 2.26 px.
      - _min(hard, 4σ_n)._ 0% everywhere, as before.

    - _Unlimited segments._ Whole segments' patch counts rose 0–23%, the ridged low coast's 30%
      (271 → 354). Demand rose −3% to +23%, and the ridged low coast's doubled (7.4 → 15.9 a
      second). On high with ridges off: the coast 455 → 554 patches (+22%) and 8.7 → 10.4 a
      second; the low pass 547 → 632 (+16%) and 347.5 → 376.6 a second; the slowdown 298 → 329
      (+10%) and 153.9 → 159.2. The low fast pass now peaks at 377 a second on high (ridges off)
      and 382 (ridges on, 16 Hz).
    - _Demand ÷ D, hard bound_ (high then low, both at τ_sel):

      | Segment            | Ridges off  | Ridges on   |
      | ------------------ | ----------- | ----------- |
      | orbit coast        | 0.72 / 1.26 | 0.12 / 1.15 |
      | descent arc        | 0.60 / 0.79 | 0.09 / 0.45 |
      | approach and flare | 0.56 / 0.76 | 0.54 / 1.49 |
      | low fast pass      | 1.10 / 1.35 | 0.90 / 0.67 |
      | slowdown           | 0.75 / 1.68 | 0.46 / 1.17 |

      D still predicts every unbudgeted moving segment within a factor of two on both settings,
      but for high's ridged slowdown, now 0.46 (0.50 before). Low's ridged low pass moved most,
      from 1.13 to 0.67. At τ_sel one more level, 17, meets D's k_L S_L ≥ h at the pass's 300 m
      above its floor, and its term alone is 55 of D's 141 a second (79 at τ). D's level rule
      steps with τ wherever a level sits near that threshold, so the ruling's "within a few
      percent" does not hold there. For T18 and T19, with the frustum correction above.

    - _min(hard, 4σ_n), demand ÷ D_ (high then low): ridges off, coast 1.95 / 0 (D 0.0), arc
      0.73 / 1.58, approach 0.73 / 1.57, low pass 0.73 / 1.78 and slowdown 0.90 / 5.91; ridges
      on, 2.35 / 0 (D 0.0), 0.69 / 1.80, 2.04 / 15.06, 0.68 / 1.81 and 0.59 / 4.86.
    - _`selectPatches`, hard bound, CPU then wall-clock p95_ (ms, loads of 2–19, four processes
      at once). On this machine's kernel (`CONFIG_HZ=1000`) the thread's CPU clock advances in
      steps of about 1 ms, and `/proc/thread-self/schedstat` steps with it; Node has no
      `CLOCK_THREAD_CPUTIME_ID`. So the CPU percentiles hold to about ±1 ms: enough to tell
      selection's work from a starved process's waits, not the last millisecond of 4d's 2 ms.
      Record version 4 reads the clock exactly (the follow-up below).
      - High, ridges off: coast 1.0 / 1.0, arc 2.0 / 2.1, approach 3.9 / 3.8, low pass
        3.0 / 3.6, slowdown 3.0 / 2.6.
      - High, ridges on (16 Hz): 2.0 / 2.2, 3.0 / 2.9, 5.0 / 5.3, 3.0 / 2.8 and 2.0 / 1.6. The
        approach's 5.0 ms is at τ_sel with 84% of its frames limited; lane B's 2.8–3.2 ms was at
        τ.
      - Low: CPU p95 at most 2.2 ms in every segment (wall at most 2.8 ms).
      - The largest single selections reach 528 ms on the wall clock but at most 26 ms of CPU
        on high and 51 ms on low. The rest of a wall-clock maximum is time the thread spent off
        its core.
  - **The follow-up: the CPU clock, the move's wording and the margin's name** (2026-10-06, lane
    D; record version 4).
    - _The CPU column is exact._ Read alone, `process.threadCpuUsage()` gives the thread's runtime
      as of the scheduler's last update, which this kernel makes once a tick (R05.T7 perf (d),
      lane B's finding). Version 3's CPU times therefore came in steps of about 1 ms. The script
      now reads the thread's clock just after `process.cpuUsage()`, which brings the calling
      thread's runtime up to date first.
      - The clock is `threadCpuClockMs` in `src/tools/threadCpuClock.ts` (new). The script loads
        it through Vite's runner, as it loads `machineLoad.ts`.
      - Its test stands in a kernel that counts CPU time in ticks, and fails without the call.
      - `FixedStepOptions.cpuNowMs`'s doc and the script's header say how the clock is read.
      - Off Linux the call is harmless. The resolution there is the system's and unmeasured:
        Windows's thread times are known to step at the clock interrupt, 15.6 ms by default.
    - _Shown on a short CPU-only replay_ (`target/laneD/rcpu/paired.mjs`, scratch). The 14 fixture
      windows (seed 7, ridges off, min(hard, 4σ_n), 64 Hz) ran through `runCell` with each
      reading in its own runs: three rounds, alternating, 5,418 selections each, at loads 20–22.
      Separate runs are needed, since the call updates the runtime that a plain reading would
      see.

      | Reading                     | CPU p50 / p95 / max (ms) | Readings of 0 | On a 0.997 ms step | Within 20 µs of wall |
      | --------------------------- | ------------------------ | ------------- | ------------------ | -------------------- |
      | `threadCpuUsage` alone (v3) | 0.000 / 0.996 / 7.557    | 78.1%         | 94.0%              | 0.7%                 |
      | after `cpuUsage` (v4)       | 0.130 / 0.754 / 5.703    | 0.0%          | 0.2%               | 98.8%                |

      The exact reading's distance from the wall clock is 0.4 µs at p50 and 6.9 µs at p95 (126 µs
      and 861 µs before). A reading costs 1.46 µs against 0.55 µs, so a selection's two cost about
      1.8 µs more. Both readings' 14 hashes are the pinned ones.

    - _New figures at seed 7_ (`target/laneD/rcpu/cells/`, scratch, not committed as a record).
      - Three hard-bound cells ran in full with the exact clock: high and low with ridges off at
        64 Hz, and high with ridges on at 16 Hz.
      - Each ran as F4's did: its own process, nice, in a capped scope, CPU only, no lock. Loads
        were 11–17 and wall times 372, 1,970 and 2,796 s.
      - The machine's crash at 18:35 cut the first ridged run short. It was re-run alone after
        the reboot.
      - Each hash, and every figure but the times, equals the `-2` record's.

      `selectPatches`' CPU time, p50 / p95 / max (ms), then the `-2` record's (version 3):

      | Segment             | High, off            | v3                  | Low, off           | v3                 | High, on (16 Hz)   | v3                  |
      | ------------------- | -------------------- | ------------------- | ------------------ | ------------------ | ------------------ | ------------------- |
      | orbit coast         | 0.37 / 0.62 / 1.48   | 1.00 / 1.00 / 2.79  | 0.06 / 0.12 / 0.67 | 0.00 / 0.99 / 1.05 | 1.25 / 2.13 / 2.93 | 1.99 / 2.00 / 4.31  |
      | descent arc         | 0.64 / 1.42 / 142.16 | 1.00 / 1.99 / 12.75 | 0.06 / 0.16 / 4.49 | 0.00 / 0.99 / 8.13 | 0.85 / 1.72 / 4.03 | 1.99 / 2.98 / 6.91  |
      | approach and flare  | 1.14 / 2.08 / 13.06  | 1.99 / 3.93 / 26.25 | 0.19 / 0.42 / 1.12 | 0.97 / 1.25 / 3.08 | 1.38 / 2.58 / 6.18 | 2.98 / 4.97 / 26.34 |
      | low fast pass       | 0.97 / 1.72 / 3.28   | 1.99 / 3.01 / 6.92  | 0.23 / 0.56 / 1.26 | 0.99 / 1.00 / 1.99 | 1.33 / 2.43 / 5.10 | 1.99 / 2.99 / 4.98  |
      | slowdown            | 0.56 / 1.15 / 13.37  | 1.00 / 2.95 / 6.76  | 0.23 / 0.50 / 1.57 | 0.00 / 1.00 / 1.98 | 0.63 / 1.09 / 2.16 | 1.00 / 2.00 / 2.40  |
      | vertical descent    | 0.12 / 0.26 / 0.61   | 0.00 / 1.00 / 1.00  | 0.16 / 0.20 / 0.48 | 0.00 / 1.00 / 1.00 | 0.12 / 0.18 / 0.53 | 0.00 / 1.00 / 1.00  |
      | hover and touchdown | 0.10 / 0.11 / 0.45   | 0.00 / 1.00 / 22.79 | 0.10 / 0.15 / 4.01 | 0.00 / 1.00 / 9.95 | 0.10 / 0.12 / 0.62 | 0.00 / 1.00 / 1.00  |

      Version 3's readings sat on whole ticks (0, 1.00, 1.99); version 4's do not. The new times
      also carry perf (d) (39e7534), which the `-2` records predate; the replay above isolates the
      clock.

      - Wall-clock p95 is within 0.21 ms of CPU p95 in every segment on high with ridges off,
        0.02 ms on low and 0.49 ms on the ridged cell.
      - High ridges-off's arc holds one selection of 142 ms of CPU, not examined: a garbage
        collection on the thread is one candidate.
      - _Finding, for T18 and lane B._ At τ_sel the ridged high approach's CPU p95 is 2.58 ms, and
        high ridges-off's is 2.08 ms: both above 4d's 2 ms. The ridged coast (2.13 ms) and low
        fast pass (2.43 ms) are above it too. Perf (d)'s 1.43 ms was measured at τ, where fewer
        of the approach's frames are limited (74% against 84%). These are provisional: loads of
        11–13, nice, under the module runner. The bundled game's figures are the owner's T17
        runs.

    - _The move's wording._ `fixedStep.ts`'s module doc and the record's header said the pass's
      selection at a frame is the record's "at a pose at most 0.1 × d_min earlier". That held
      until 13b011b (R05.T11.c, 2026-10-05, Risks, "The cadence's 1%"), after which the bound is
      m ÷ (1 + m), about 0.0909. Both now say so, the header from `RESELECT_MOVE_FRACTION` itself:
      "at most m ÷ (1 + m) = 0.0909 × d_min earlier, for the margin m = 0.1".
    - _Record version 4._ The version rule is the record's shape, which has not changed. But the
      CPU column's meaning has, and the header now says the reading is exact. So `mergeRecords`
      refuses version 3's files, which it would otherwise summarise as exact; a test holds that.
      The header also says the times are taken under Vite's module runner (Risks, "The record's
      module runner").
    - _The committed records are not refreshed._ The clock never moves selection: the hashes
      agree. So only the `-2` records' CPU column is coarse, and their notes already hold it to
      about ±1 ms. Selection times are provisional either way: the figures of record are lane B's
      A/Bs and the owner's quiet-machine runs, and a refresh earns its place only beside those.
      `2026-10-05-demand-{hard,calibrated}-2.md` gain a banner on the CPU column, the move's
      wording and the runner. Their JSON is unchanged, still version 3.
    - _The rename._ `RESELECT_FRACTION` is now `SELECTION_MARGIN`: since 13b011b it is the
      selection's margin m, and the move is `RESELECT_MOVE_FRACTION`. It runs across lanes B, C
      and D's files: `selectionTolerance.ts`, `terrainPass.ts`, `fixedStep.ts`, `demandRecord.ts`
      and their tests. The output is bit-identical: the 14 pinned hashes and the fixture are
      unchanged, and the header still prints τ ÷ 1.1. The plan's dated records keep the old name.
      Its text on the live code ("The cadence's 1%", T11.c's cadence) uses the new one.
  - **Resolved: the selection's "collapse" near the ground** (2026-10-03). It was the record's
    camera underground (the direction above), not selection: lane B's
    `belowDatum.wasm.test.ts` selects down to the finest level 1.6 m above the true ground. With
    the site at −1,845.8 m the windows select 66 patches (high) and 69 (low) at the hover, the
    forced region included, and the vertical descent's and the hover's hashes are pinned.
  - **Finding: budgeted selection under the hard bound churns** (probe, 2026-10-03, high, low fast
    pass): 952 patches, `limited` 72% of frames, about 830 new keys a frame (53,000 a second
    against D's 311), `selectPatches` p50 116 ms. Lane B fixed it (5d90fab, the streaming gate,
    resident ancestors pinned, deepest evicted first); the hard cells' full record follows the
    collapse's fix.
  - **Fixed: the orbit coast flies level** (2026-10-04, lane D, on lane C's finding in T13.b).
    - _The cause._ At seed 7 the coast climbed at +18.37 m/s, from 400.0 to 401.0 km above the
      site. The cause was the vertical re-fit, not the geometry. The solve put the 5 s blend from
      the coast's rate to the descent arc's −422.5 m/s inside the coast's last 5 s, where Design
      note 19 places blends. To keep the coast's end at 400 km, it then re-fitted the coast's rate
      to 422.5 × 2.5 ÷ 57.5 = 18.37 m/s. The figure is the same at every seed. The pose's
      `verticalSpeedMps` and the velocity's component along the spheroid's normal agreed to
      10⁻⁵ m/s, so no geocentric direction was mixed in.
    - _What "level" means here._ The height above the datum the path is flown over is constant:
      the spheroid, along its normal over the ground direction d, plus the site's height. The
      vertical speed is therefore 0. The geocentric radius is not constant. At a constant height
      over an oblate spheroid, |p| changes as the track crosses latitudes, at a rate of up to
      a f |sin 2φ dφ/dt| ≈ 26 m/s with the ground track at 7.67 km/s on the sphere of radius
      (a + c) ÷ 2 (the camera moves at about 8.15 km/s body-fixed at 400 km, a retrograde
      equatorial orbit's; see Design note 19 and decision-r05-coast-speed.md). At seed 7 the rate
      is 16.5 to 17.1 m/s, so |p| rises 1.01 km over the coast while its height stays 400 km: a
      geocentric check would still see a "climb". A Keplerian circular orbit would keep |p|
      instead. The scripted camera holds its height, as the table has it.
    - _As built._
      - `VerticalShape` gains `level`, and the coast is `level`. A level segment holds its rate
        (its drop over its duration, 0 here) and hosts no blend.
      - The segment after a level one must be `constant`; the constructor throws otherwise. It
        blends in from rest over its own first 5 s (`VerticalPiece.leadIn`). Design note 19's
        "over the last 5 s of the earlier segment" no longer holds after a level segment.
      - The arc's rate between blends becomes −423.7 m/s (was −422.5). The arc flies up to 1.05 km
        higher just after its start. The gap closes linearly to about 3 m at its end blend and to 0
        at its end.
      - A lifted low pass is now a `level` segment too, replacing `flySegments`' own flag. Its
        knots are the same, bit for bit, which the test pins.
    - _Tests_ (`descentProfile.test.ts`). At seeds 7 (over its site, −1,845.8 m), 0 and 5, every
      64 Hz pose of the coast has:
      - `clearanceM` exactly 400 km and `verticalSpeedMps` exactly 0;
      - the position 400 km above the site's height along the spheroid's normal, to 1 µm, and less
        than 1 µm off that normal;
      - the velocity's component along the normal below 1 mm/s.

      The path is closed-form, not integrated. The tolerances cover the rounding of
      |p| ≈ 6.8 × 10⁶ m (0.9 nm a ulp) and of `poseAt`'s ±1 ms central difference (about 5 µm/s;
      truncation 2 nm/s). The bug was 18.37 m/s. Also tested:
      - the arc starts from rest and reaches its rate after 5 s;
      - 003a6a3's fingerprints still hold for the table with a climbing coast;
      - from the approach on, today's poses lie within 1 µm of that table's.

    - _Windows._ `--write-fixture` rewrote the fixture byte for byte (1,979 keys). Only high's
      orbit-coast hash moved, from 95585482dc0e857e to c739aef5d81221ae. Unchanged: the other 13,
      the factor-of-two windows (threshold unchanged) and lane C's clearance tests, which pass.
    - **Stale for the coast.** The committed records
      `docs/measurements/descent-spike/2026-10-03-demand-calibrated.{json,md}` and
      `2026-10-04-demand-hard.{json,md}` were flown with the climbing coast. Not re-run (the
      orchestrator's instruction).
      - Their orbit-coast rows are stale in all four cells of each: patch counts, demand, D, the
        height above the floor and the selection times.
      - Their descent-arc rows were flown up to 1.05 km lower near the arc's start, so they may
        shift slightly.
      - From the approach on, they stand.
      - **Re-run (2026-10-05, F4).** All eight cells were flown with the level coast, as
        `2026-10-05-demand-hard.{json,md}` and `2026-10-05-demand-calibrated.{json,md}`, and the
        hard record's coast caveat is gone. The coast and arc rows moved little. On high with
        ridges off, the coast went from 454 to 455 patches and D from 13.3 to 13.2 a second, and
        the arc's demand from 16.4 to 16.1 a second. The two old records are kept with a banner
        that marks their coast and arc rows stale ("Record: F4's figures over the whole descent",
        above).
    - _Left as it was._ The unlifted low fast pass is still `constant` and climbs 0.076 m/s
      (1.9 m) into the slowdown's blend, by the same mechanism. In both of seed 7's records the low
      pass is lifted, so it already flies level there. Making it `level` would move the flare and
      the slowdown on every unlifted seed. For the orchestrator.

- **Deviations in T13.c, as built** (2026-10-03, lane D).
  - _The command line._ `cli.ts` takes the plan's options with commander (`--ridged`, the plan's
    name, not `--ridges`); `ClientArgs.spike` is `SpikeLaunch` (`preload/api.ts`): setting
    (default high), seed (a u64 in decimal, default 7, T13.a's record seed), smoke, out, workers
    (1–64), vertex path and normals (`null` for the setting's own), ridged (default off),
    Dawn safety (default on) and capture. Each spike option without `--descent-spike` is a usage
    error, as is `--setting low --vertex-path baked-offsets` (the low cache, Design note 4).
  - _The hand-off._ `preload/spikeLaunch.ts` carries the options as one
    `--hyperion-descent-spike=<JSON>` switch; `preload/spikeApi.ts`'s `spikeMember` gives
    `HyperionApi.spike` only when it is there (tested on an ordinary argv). `SpikeApi`:
    `launch`, `startTrace`, `stopTrace`, `sampleMemory` (the preload's
    `process.getProcessMemoryInfo().private`, handed to the main process's sampler), `writeResults`,
    `writeCapture`, `end`. One fixed channel each (`SPIKE_CHANNELS`, repeated in the preload and
    tested equal).
  - _The main process._ `main/ipcSender.ts`'s `isOwnPage(frame, pageUrl, mainFrame?)` is the smoke
    harness's inline check factored out (the smoke harness uses it), and the spike's handlers also
    require the window's own main frame. `main/spikeReport.ts` checks the report and the capture
    (every series the frames' length, every figure finite). `registerSpikeHandlers` in
    `main/spike.ts` rejects a foreign sender or unchecked arguments; `main/spikeSession.ts` holds
    the run: the trace and the 1 Hz `MemorySampler` between start and stop, the trace written into
    the run's profile and removed once reduced (it can reach 1.4 GB), a failed reduction a missing
    figure with its reason, the results through `buildResults`/`writeResults`, the capture's
    `capture.json` and `capture.bin`, and the exit status. `index.ts` applies `launchSwitches` in
    place of `graphicsSwitches` (the spike always asks for `gpuTiming`), reads `nvidia-smi`'s
    baseline before the window, sizes the window's content to 1920 × 1080 (high) or 1280 × 720
    (low), keeps it hidden and offscreen for `--smoke` or `HYPERION_SPIKE_HIDDEN=1` (the recipe's
    `--hidden`; headless Ozone's GPU process exits on a hardware adapter), and ends a run that
    never ends after 3 min (smoke) or 45 min with status 3.
  - _The renderer._ `view/spike/spikeHarness.ts`: `spikeGpu(gpu, capture, scriptTimeS)`, the
    spike's `ViewEngineSource` over a wrapped `GPU` (`LoadEngineOptions.gpu` the same), whose
    devices carry the pipeline tally, a `ResolveCounter` and the capture; `rowOf` and
    `SPIKE_PASS_ROWS` (`meta.passRows`); `SpikeRecorder`, which feeds `SpikeMetrics`. **The
    engine frame.** R01's timer numbers each resolve, one a `renderFrame`, so a spike frame of five
    views spans several `PassTimes.frame` numbers. The resolve counter counts
    `resolveQuerySet` on the device's encoders, which is that number as it is given out, so each
    spike frame records the last one it reached and `SpikeMetrics.passTimes` assigns an engine
    frame to the first spike frame whose last is at or after it (earlier ones, before the run,
    to none), summing a label timed twice (the two `view:wireframe` passes). `patchesCalibrated`
    re-runs `selectPatches` on each selection's own inputs under `boundedPlanet(…, "calibrated",
σ)`. The adapter's peak is the recorder's own running total of created less destroyed bytes
    (T11.a's tally keeps peaks per category). The predicted D of a segment is the mean of
    `perLevelDemand` over its 1 Hz poses at the height above the floor.
    `view/spike/spikeController.ts` runs the run: measuring from the measured descent, memory at
    1 Hz, the capture (120 frames from 5 s into the low fast pass, or from 5 s in a smoke run,
    started between frames in `onFrame`, ended early where the run ends first), and at the end
    the trace stopped, the results written and a pass sent; a smoke run passes at 10 s if a patch
    was baked in a worker. `--vertex-path` and `--normals` reach the terrain pass as
    `terrainSettingsFor(setting, variantOf(launch))`, through `SpikeRun`'s `variant` and
    `DescentSpike`'s `variant` prop (compared by its fields, so that a new object does not restart
    the run); a variant the setting cannot take ends the run as failed, and the report's optional
    `terrain` (the vertex path and normals drawn with) becomes the results'
    `run.options.terrainVertexPath` and `terrainNormals` (decision-r05-spike-ux.md, "Spike variant
    flags"). `view/spike/SpikeApp.tsx` is the root on a spike launch (`main.tsx`): the graphics
    status provider, then `DescentSpike` on the measured engine with the controller's listeners;
    the engine source's `load` hands the engine to the controller for its pass times and
    allocations. T13.a exports `DescentUnclearable` (a `RangeError`) for the fourth lift's
    refusal, for lane C's `DescentRefused`.
  - _The recipe._ `just descent-spike` runs `just build`, then `scripts/descentSpike.sh`: the
    server as `cargo run --release -p hyperion-server -- --num-workers 2 --port 7879`
    (`HYPERION_SPIKE_PORT`; release to reuse the build, and off the default port other lanes'
    servers use), `--companion-load <n>` busy threads, and the client on `out/` under
    `setsid timeout --kill-after=10` with a fresh `--user-data-dir`, every process group killed at
    the end. The plan's `--cold-cache` needs a cache kept across fresh profiles: the script keeps
    the profile's `*Cache` directories in `target/descent-spike/gpu-cache`, copies them into each
    new profile and back after, and `--cold-cache` empties them first.
  - _Review fixes (2026-10-03)._ Every way a run cannot go on ends it at once, not at the
    watchdog: `SpikeListeners.onFailed(status)` (the descent refused, the terrain not measured,
    the views not made, from `DescentSpike`'s own catch sites), no adapter or an engine that could
    not be made (the spike's engine source), and the safe mode or a renderer without WebGPU, which
    ask for no adapter at all (`SpikeApp`). A window closed before the run ended exits 1, not the
    ordinary quit's 0. `ResolveCounter` starts an epoch at each device it wraps, since a rebuild
    after a device loss makes a new timer that numbers its frames from 1 again
    (`runFrame(timerFrame)`). The report's canvas is the main view canvas's own drawing size, not
    the window's. The NVIDIA baseline is read by the recipe before Electron starts and handed over
    as `HYPERION_SPIKE_NVIDIA_BASELINE` (a file of `nvidia-smi -q -x`), since the GPU process can
    start while the main process would still be reading it. The recipe passes
    `--ozone-platform=x11` on a Wayland session, as `client` does, so that the client does not
    relaunch itself and leave the script a first process's 0. A refused `end` is logged.
  - **Acceptance, 2026-10-03:** `just descent-spike --smoke` (hidden, offscreen, RTX 3080, R01's
    Vulkan switches, a fresh profile) exited 0 after 26 s at load 12–17 with a patch baked in a
    worker, and again after the review fixes (26 s at load 25–29). The `vitest` acceptance
    (`src/main/cli src/main/spike src/preload`) passes.
  - **Pending (separate tasks):** T14.c's hidden `just descent-spike --setting low --hidden`
    results file (taken 2026-10-04: T14.c's as-built record), and the first real capture
    (`--capture <dir>`) with its offscreen
    `just replay <dir>`. By hand for the owner: the visible full descent
    (`just descent-spike --setting low`) and the presented replay (`just replay <dir> --present`).

- **The scripted descent's terrain clearance** (decision-r05-descent-clearance.md, 2026-10-03). The
  script is flown above per-stretch true bounds of the finest mesh. Each bound is the baked maximum
  plus ε_n over a level-14 corridor, with level 12 above 2 km, level 4 for the arc and level 16 for
  the last 250 m. The clearances are 300 m on the low pass, 200 m on the flare and slowdown, 100 m on
  the final approach, and the table's own over the site, whose height is exact.

  On rough seeds, boundaries are lifted and the vertical descent's top rises, with its 20 m/s kept.
  The per-level demand then reads the height above the floor, and the results record each segment's
  lift. So a lifted run measures different heights from the table, but its demand is predicted at the
  heights actually flown.

  Not covered:
  - The bound is of the finest mesh, not of what is drawn. A stand-in ancestor, drawn while
    streaming lags, which the low pass is designed to provoke, can stand up to its ε_L above the
    finest surface (877 m at level 8 with ridges). So the camera can pass through a coarse stand-in
    for a frame. That is a streaming failure the run already counts, not a script error.
  - The hover's 1 m is against the drawn ground only once the forced region is resident
    (`FORCED_REGION_RESIDENCY_S`).
  - Slopes beside the site make the 3D distance in the vertical descent and hover smaller than the
    vertical clearance (about 0.7 m at 1 m on a 45° slope).

  As built (lane D, `descentProfile.ts`):
  - `DescentSegment.stretches` holds each segment's rule as data (`SegmentStretches`: piece length,
    level, C, the flare's level 12 above 2 km, the slowdown's last piece "final approach");
    `trackStretches(profile)` returns the pieces (`TrackStretch`: piece, segment, from/to remaining
    distance, start/end on whole seconds, level, C), planned on the table's unlifted altitudes, so
    the same for every terrain: the orbit coast as one piece, the arc in nine of 100 s, the flare in
    twelve of 10 s (pieces 1–8 at level 12, 9–12 at 14 on seed 7), the low pass, slowdown 1–5 and
    the final approach.
  - `DescentTerrain.stretchMaxHeightsM` is additive. Without it, `trackMaxHeightM` (0 by default,
    as before) is the low pass's floor and every other floor is the site's height, which gives
    today's profile bit for bit (checked against the previous profile at every 64 Hz pose before
    the change; the test holds floors at the site equal to `trackMaxHeightM` at the site).
    `liftOverTrack` is gone: rule 3's boundary altitudes subsume it.
  - `DescentPose.floorM` and `heightAboveFloorM` (the clearance less the floor's height above the
    site, so exactly `clearanceM` where the floor is the site), `DescentProfile.minFloorMarginM`,
    `groundDirAt(tS)` (the track's direction alone, for the floors' keys) and the exported
    `FLOOR_TOLERANCE_M` (10⁻⁶ m): the 64 Hz check lets a deficit that small pass, since a boundary
    sitting exactly at its floor plus C (the low pass at the site's 300 m) is otherwise lifted for
    the integrals' 10⁻¹³ m rounding. The constructor throws a `RangeError` for floors that do not
    match the plan or are not finite, and after a fourth lift; raising a piece's two boundaries
    lifts its whole piece, so in practice the rounds converge in one, and the test of an
    unclearable floor uses an infinite one.
  - The record (`demandRecord.ts`): `stretchKeys(profile, stretch)` takes the level's patches
    under the track and their eight neighbours as lane C's `trackPatchKeys` does;
    `measureTerrain` returns the floors (baked maximum plus ε_n); each cell records
    `segmentLifts` and `minFloorMarginM`, each segment its least and greatest
    `heightAboveFloorM`; the fixed-step D and the craft's contact read `heightAboveFloorM`. On seed 7
    (ridges off) the low pass's own floor binds, and it flies level 300 m above it.
  - _A lifted low pass holds level_ (review, 2026-10-03): the back-to-front solve gave a level
    segment before a descending one a gentle climb to feed its end blend (1.9 m on the table's own
    low pass, but about 22 m at a 500 m lift and 80 m at 2 km, since the slowdown then falls
    faster). Where the low pass is lifted, it holds rate 0 and the piece after it starts from rest
    and ramps to its rate over its blend length (`VerticalPiece.leadInS`); the unlifted table keeps
    its climb, bit for bit (the test pins 64 Hz pose fingerprints taken from 003a6a3's profile).
    Old callers whose `trackMaxHeightM` lifted the low pass (including the default 0 m over a site
    below the datum) now fly it level, a change on purpose.
  - **Split segments re-anchored, valleys filled** (decision-r05-descent-clearance.md,
    follow-up, 2026-10-03, on lane D's question). Rule 3 compared a split segment's interior
    boundaries with the unlifted table, so a lifted low pass beside a split slowdown saw-toothed.
    As built: an interior boundary's shape is the table's shifted by its segment's two end lifts,
    blended in time (`interiorBoundaries`), it is lifted against the floors from there, and
    `fillValleys` raises it to the lower of the highest boundary before and after it in its
    segment. A segment is split only where an interior boundary differs from that shape. Within a
    split segment (and after a lifted low pass), a piece faster than the one before blends in from
    that one's rate (`VerticalPiece.leadIn`, a forward solve over each such run inside the
    back-to-front one) instead of the slower one blending into it: either way the blend bows its
    host by about the other's rate × b ÷ 2, and a level piece before a 200 m/s fall bulged 95 m
    (the flare under a 2 km ridge) and a gentle one before a 70 m/s climb dipped 30 m. The tests
    hold every 64 Hz pose of a segment within 0.5 m of min(the highest before, the highest after)
    in the ridges, lifted-low-pass and two-ridge-valley cases, and the 003a6a3 fingerprints.

- **The clearance follow-up, as built (lane C, 2026-10-03).**
  - `SurfaceQuery.maxHeightsM(groups)` (request `max-heights`) answers each group's floor in one
    batch, baking each distinct key once, and refuses an empty group.
  - T13.a's `stretchKeys` sampled the track half an edge apart only, so a pose whose patch the
    track crossed at a corner between samples could miss its far neighbours. It now samples at
    every 64 Hz pose too (a8a48eb, with a coverage test). The ranges fixture gains ten keys, and no
    pinned hash moved.
  - The contact reads `heightAboveFloorM`, as T13.a's demand record does.
  - `clearance.wasm.test.ts` runs the ruling's checks on the real module, for seed 7 (the rough site,
    1.85 km below the datum) with ridges on, in the suite:
    - the finest mesh at every 64 Hz pose's ground direction, and half an edge to either side, lies
      at or below the stretch's floor;
    - the altitude above the finest mesh under the camera is at least the piece's C;
    - touchdown is 1 m above the site, the table's value to 1e-6 m.

    Each run bakes about 600 patches (about 20 s at load 50 or more). All six runs the ruling names,
    seeds 0, 1 and 7 with ridges off and on, passed by hand on 2026-10-03. T13.a's lift solve meets
    its floors to rounding (−4 × 10⁻¹¹ m), so the checks take its `FLOOR_TOLERANCE_M`.

  - Only T13.a's `DescentUnclearable` (its lift limit) becomes `DescentRefused`. Floors of the
    wrong count or not finite are a measurement fault and show `TERRAIN NOT MEASURED`. No finite
    floor found by probing exhausts T13.a's four lifts, so the refused status is tested through
    `DescentSpike`'s `prepare` prop.
  - Scope (the orchestrator, 2026-10-03): `just ci` runs the roughest of the ruling's six wasm runs,
    seed 7 with ridges on. `just test-slow-client`, added to `just ci-slow`, runs all six under
    `HYPERION_SLOW_TESTS=1`.
  - Seams for T13.c (the orchestrator, 2026-10-03):
    - `PreparedDescent.omittedSigmaM`;
    - `SpikeListeners.onPrepared(prepared)`, called once the measurement is ready;
    - `SpikeListeners.onSelect(input, selection)`, through the new
      `TerrainPassOptions.onSelect`, called after each `selectPatches` and outside the
      `terrain.select` span; unset, it costs nothing.
- **The spike's terrain variants, as built (lane C, 2026-10-03; the spike variant flags ruling in
  decision-r05-spike-ux.md).**
  - `qualitySetting.ts` gains `TerrainVariant { vertexPath?, normals? }` and
    `terrainSettingsFor(setting, variant?)`: `SETTINGS[setting].terrain` with only the given fields
    replaced. It throws a `RangeError` for `low` with `baked-offsets`.
  - `TerrainPassOptions.terrain` defaults to `TERRAIN_SETTINGS[setting]` and is resolved once in
    the constructor. It is read for the resources, the pool's normals and both τ reads, after a
    device restore too. `setting` still decides the low setting's `DETAIL LIMITED`.
  - Tests:
    - no variant gives the setting's own object;
    - each field replaces only itself;
    - the low setting with `baked-offsets` throws;
    - a high variant of `face-differences` with mesh normals reaches the pool, the material and
      `terrainSlotLayout`'s slot count, and keeps them through a restore.

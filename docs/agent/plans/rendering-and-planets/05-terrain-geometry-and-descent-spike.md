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
    never set); main-thread time split into our code (`performance.measure` spans), the engine
    adapter (CPU-profiler self time in its lazily imported `engine-*.js` chunk) and idle; patches a
    second requested, baked and made resident, against the predicted demand (Design note 19); upload
    bytes from the engine's `writeBuffer` and `writeTexture` tally (R01's `uploaded` events);
    pipeline creations after warm-up, from a shim on `createRenderPipeline`, `createComputePipeline`
    and their asynchronous forms (installed on the device through a wrapped `GPU` handed to the
    spike's `ViewEngineSource`, since no device is reachable outside R01's adapter), each late one
    logged with its label, cold and warm caches run
    separately; garbage-collection pauses per thread from V8's GC trace slices; and memory at 1 Hz:
    `app.getAppMetrics()` and the renderer's `process.getProcessMemoryInfo()`, the GPU process's DRM
    fdinfo (`drm-total-*`, `drm-resident-*`, summed over client IDs, since ANGLE and Dawn open their
    own), on NVIDIA `nvidia-smi -q -x`'s process list with the device's `memory.used` less a
    baseline (not `--query-compute-apps`, which lists compute processes only and can miss the GPU
    process; researched for R12, its Design note 6), and the adapter's own tally of buffers and
    textures, against the 1 GB ceiling. The trace comes from Electron's `contentTracing` in the main
    process, over categories that include `devtools.timeline`, `disabled-by-default-v8.gc`,
    `disabled-by-default-v8.cpu_profiler`, `blink.user_timing` and `gpu`, and is reduced to the
    results file in the main process (T14.b). Chromium quantises WebGPU timestamps to 65.5 µs by
    default (Dawn's `timestamp_quantization` toggle, mask `0xFFFF0000` on the low word), which the
    brainstorm's "the forced switches make available, uncoarsened" gets wrong; measurement runs
    alone lift it through R01's `gpuTiming` option (`GPU_TIMING_SWITCH`), which the spike flag turns
    on before `ready` (R01 applies it, like `enable_subgroups_intel_gen9`, only in its Linux `vulkan`
    mode, so a run elsewhere records `PassTimes.timer` as `quantized`; accepted 2026-10-02,
    decisions-r06-r07.md item 8: frame intervals are the pass criterion; a quantized run is
    recorded as such). The safety toggles of Design note 22 are merged into both of R01's lists,
    `--enable-dawn-features` and `--disable-dawn-features`, with `mergeSwitchValue`, since appending
    a second switch would replace the first (R01 Design note 2). `gpuTiming` already puts
    `timestamp_quantization` in the disable list, so a run with timing on and safety off carries one
    `--disable-dawn-features=timestamp_quantization,lazy_clear_resource_on_first_use`.
19. **The scripted descent and its predicted demand** (researched 2026-09-29). The path is a pure
    function of script time in body-fixed coordinates over a landing site and approach azimuth
    drawn from the spike's seed, and the frames sample it at their own display times, so the
    trajectory is identical every run while the frame rate is free, which is what streaming must be
    measured against. A second, fixed-step mode samples it at 64 Hz for the CPU-only tests, whose
    selection sequence is then identical bit for bit. Its segments (provisional, T13.a):

    | Segment             | Altitude        | Horizontal speed  | Vertical speed    | Duration | Brainstorm's formula, high |
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
    segment shorter than 20 s), so that position and velocity are continuous at every boundary.
    T13.a re-fits the durations so that the boundary altitudes hold with the blends included. The
    path is a scripted camera, not a flight, so the blends' accelerations are not a craft's.

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
    per-level D from T6's bounds, which is the one the runs are measured against. The measured
    demand, first-time-selected keys a second with the cache's semantics stated, must lie within a
    factor of two of the per-level D (T13.a), and the recorded figure is patches a second sustained
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

    | Criterion             | 1080p, RTX 3080 (T = measured vsync period, 16.68 ms)                   | 720p30, UHD 620 low (T = 33.3 ms)                                                               |
    | --------------------- | ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
    | 50th percentile       | ≤ T + 0.5 ms                                                            | ≤ T + 0.5 ms                                                                                    |
    | 95th percentile       | ≤ T + 1 ms                                                              | ≤ 35 ms                                                                                         |
    | 99th percentile       | ≤ 2T                                                                    | ≤ 2T                                                                                            |
    | Missed frames         | ≤ 1% of intervals above 1.5 T                                           | ≤ 1% of intervals above 1.5 T                                                                   |
    | Hitches               | none above 3 T                                                          | none above 3 T                                                                                  |
    | Headroom              | main thread and GPU pass sum, each ≤ 0.8 T at the 95th percentile       | the same                                                                                        |
    | The rest of the frame | terrain and atmosphere GPU time within their rows' upper ends: 5 + 1 ms | 14 + 4 ms                                                                                       |
    | Memory                | GPU resident ≤ 3 GB; above 2 GB a finding                               | GPU resident ≤ 1 GB, with the 15 MB synthetic field (R09's Earth: about 12 MB) in three workers |

    The memory row reads the brainstorm's "2 to 3 GB" as the roadmap's correction does: a finding
    above 2 GB and a failure above 3 GB. Its "three workers" is the separate memory run of Design
    note 11 on the UHD 620, whose timed runs keep the default count; on the desktop the timed runs
    pin three workers and the row is read from them (T17). The last two rows keep the gate honest
    about what the spike does not draw (Design note 17):
    the budget's other rows must still fit beside terrain and atmosphere. Patches a second
    sustained are recorded against the predicted demand but are not a pass criterion: where demand
    outruns the workers, the view annunciates `TERRAIN: STREAMING`, as the budget intends, and the
    run records how long and where. Percentile and missed-frame conventions follow frame-time
    practice (PresentMon's `MsBetweenPresents`, Chromium's dropped-frame metric); the headroom row
    is the defined meaning of open question 2's "fit with headroom".

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

### R05.T13 The spike

**R05.T13.a The scripted descent.** `descentProfile.ts`, the path of Design note 19 as a pure
function of script time in body-fixed coordinates over a landing site and azimuth drawn from the
spike's seed, with its segments as data; the test planet's rotation of Design note 14 (the IERS
sidereal day re-checked); and `demand.ts`, the prediction of Design note 19 from T6's per-level
bounds.

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
including the per-level ε_n and k_n. It writes a Markdown summary beside the file.

- Files: `apps/hyperion/src/main/fdinfo.ts`, `apps/hyperion/src/main/results.ts`, their tests,
  `docs/measurements/descent-spike/README.md`.
- Tests: the fdinfo parser against recorded fixtures from i915 and amdgpu, summing distinct
  client IDs; the results writer's output parses against its schema.
- Acceptance: `pnpm --filter hyperion exec vitest run src/main/fdinfo src/main/results`;
  `just descent-spike --setting low` on the development machine writes a results file with every
  figure present or null with a stated reason (on the RTX 3080 the fdinfo readings are null:
  decided 2026-09-30 by a delegated decision, hardware item 2). The lane proves the file with a
  hidden run, whose presentation-time figures are null with the reason "no window shown"; the
  visible run, the one that counts, is pending by hand for the owner.

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

### R05.T16 The UHD 620 runs

By hand on the owner's UHD 620 laptop, by the owner, recorded, each run on a quiet machine (Design
note 27) with its results file and summary under `docs/measurements/descent-spike/`. The low
setting's harness and results file are first proved on the development machine (T14), so that the
owner's runs need no debugging. The runs are one baseline
and then one-factor changes from it, not a matrix.

**R05.T16.a The baseline.** The low setting at 720p paced to 30 fps with the two instruments and
the console open, the local server running with `--num-workers 2`, the default two height
workers, `face-differences`, mesh normals, ridged terms off, safety checks on and a warm pipeline
cache, over three seeds: three runs of about 21 minutes (the sum of Design note 19's durations).
One high-setting run at 1080p is recorded for comparison, not judged.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: the four results files exist, each with every figure of Design note 18 and the
  machine's load average and governor; the summary states pass or fail against every row of
  Design note 21's table. Each results file records `PassTimes.timer`, the platform and the
  launch mode; GPU-time rows on a `quantized` timer carry ±65.5 µs per pass and are marked
  marginal within that tolerance of their limit (decisions-r06-r07.md item 8).

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
separate memory run (decided 2026-09-30 by a delegated decision, hardware item 4).

The machine, driver and display are recorded with the results.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: as T16.a, on the discrete machine, with the replay's results file beside the
  browser's for the same span.

### R05.T18 Defaults from the measurements

The choices the plan left to measurement, made from T16's and T17's results and recorded with the
figures that decided them: the vertex path for the high setting (Design note 4; the low setting
keeps `FaceDifferences`), the high setting's normal scale (Design note 25), the descending
thresholds and `FORCED_REGION_RESIDENCY_S` (Design note 9), the cache budgets (Design note 10, until
R10), the worker counts (Design note 11) and, if T16 redesigned it, the low setting. The code's
defaults change in one commit; the plan's Design notes gain "as built" lines.

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
verdict names the selection bound as ours to fix; that is never a fired rule (decisions-r05.md
item 6).

- Files: this plan, `docs/measurements/descent-spike/README.md`.
- Acceptance: the verdict names each criterion of Design note 21 with its measured value on each
  machine; the owner has the drafted brainstorm edit.

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
  there, from T6's and its own recorded figures.
- **The descending thresholds** (Design note 9) are provisional: 1 km and 30 s, and
  `FORCED_REGION_RESIDENCY_S` 30 s, until T16 measures how long a forced region takes to become
  resident and T18 sets them.
- **The pass criterion's reserve** (Design note 21) counts only terrain and atmosphere against
  their rows' upper ends. If later plans' passes land above their own rows, the spike's pass does
  not carry over; R12's consolidated runs are where that shows.
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
    own utility process, so T14.c reports it apart from the app's memory.
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
  - **Pending:** the hidden `just descent-spike --setting low` proof waits on T13.c (which waits on
    T13.b) and is taken then; the visible run stays with the owner, its command in
    `docs/measurements/descent-spike/README.md` (decisions-r05.md item 7).
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
    - From above the atmosphere, a sky pixel whose ray meets the ground stores no transmittance
      to space, so the sun's disc is never drawn through the planet.
    - `geodeticOf` is the classical fixed-point iteration on φ and h. On WGS 84 it converges to
      under 10⁻⁸ m. Above a flattening of about 0.04 it fails to converge, so R08 replaces it (with
      Bowring 1976, for instance).
  - _The phase functions_ are evaluated per term in the shaders. `packMedium` encodes each term's
    phase in `Term.scattering.w` (0 none, 1 Rayleigh, 2 Cornette–Shanks) and its g in
    `Term.absorption.w`; a test holds the packing.
  - _The sizes_ follow Design note 16. Two steps a slice on both settings, and the ray-march step
    counts, are this task's choices; T18 revisits them.

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
    - a move of more than `RESELECT_FRACTION` (0.1) × the nearest selected non-finest patch's
      sphere distance, floored at one finest patch (spheres of ±24.5 km height ranges contain a
      low camera).

    It selects at τ ÷ 1.1 with `maxPatches` = ⌊slots ÷ 2⌋ (981 high `BakedOffsets`, 1,952
    fallback, 648 low) and the cache as `heightRanges`. The draw set, `retain`, the conditions and
    the contacts near drawn patches are recomputed only when selection runs, so a frame at rest
    reuses its `DrawSet`. The demand (T7.c's breadth-first list, less resident keys) goes to the
    pool when selection runs or a bake lands.

  - _The morph bands_ (`morphRangeM`, `MORPH_START_FRACTION` 0.7, a hand value).
    - Level n's band runs from 0.7 of the way from d_n to d₍ₙ₋₁₎, ending at d₍ₙ₋₁₎, where d_k is
      the distance at which `selectionErrorM(k)` subtends the setting's τ (not τ ÷ 1.1, so that a
      coarse–fine edge stays at morph 1 between selections).
    - Level 0 has none. The finest level's band is [0.7 d₍ₙ₋₁₎, d₍ₙ₋₁₎].
    - The bands are computed per level when selection runs. Stand-ins take their own level's.
    - Where the budget leaves a coarse patch beside a finer one inside the parent's band, CDLOD's
      crack-freedom does not hold, and the skirts cover the gap.
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
  - **Pending T13.b:** the spike's `ViewEngineSource` (the wrapped `GPU` handed to
    `requestAdapterOutcome` and as `LoadEngineOptions.gpu`), the `performance.measure` spans and the
    per-frame calls into `SpikeMetrics`, which need the spike's scene and loop.
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
    results file follows `main/results.ts`'s schema (version 1), whose types gained
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
  - **Pending:** a capture of the real descent (T13.c's `--capture`), its offscreen replay on the
    RTX 3080, and the presented replay, by hand for the owner:
    `just replay <capture-dir> --present` (a visible window on `:0`).
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
- **Deviations in T13.a, as built** (2026-10-03).
  - `view/spike/descentProfile.ts`: `landingSiteOf(seed)` (SplitMix64 from the seed, uniform in
    direction over ±60° parametric latitude, a geodetic ±60.083°, so the scene's Sun stands at least
    29.9° high, and in azimuth), `DescentProfile(figure, site, terrain?)` with `durationS`,
    `segmentAt`, `segmentSpans`, `positionAt` and `poseAt(tS): DescentPose`, and
    `DESCENT_SEGMENTS`, the table as data with each segment's vertical shape. The vertical speeds
    are re-fitted, not the durations: each boundary altitude holds exactly with the 5 s (1 s)
    velocity blends, so a level segment before a descending one climbs gently (the orbit coast by
    about 18 m/s, the low pass by about 0.07 m/s, more over raised terrain). The horizontal speed is
    the ground track's on a great circle of the mean radius; the camera looks along the track,
    pitched 30° down at 300 m/s and above, turning to the nadir at rest.
  - _Terrain (orchestrator's ruling, 2026-10-03):_ `terrain` takes `siteHeightM` (the terrain's
    height at the site) and `trackMaxHeightM` (an upper bound on the terrain under the low pass's
    track, baked ranges plus ε over the patches under it), both measured once by the caller (lane
    C's spike worker) so that the script stays a pure function of the seed and the two numbers.
    Every altitude and `groundPointM` sit on the site's height (`DescentPose.altitudeM` above the
    spheroid; the added `clearanceM` above the site, which the demand prediction reads); the low
    fast pass, and its neighbours' ends that meet it, fly 300 m above `trackMaxHeightM`. Other
    segments do not follow the relief along the track, which is fine above the low pass's
    altitude; the vertical descent and the hover are over the site itself.
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
    slowdown's track, sampled every 250 m, a true bound. On seed 7 the site lies at −1,953.2 m.
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
  - **Finding: D's factor of two.** In the windows (min(hard, 4σ_n), ridges off), the measured
    demand lies within a factor of two of the per-level D on high's descent arc (0.67) and approach
    and flare (1.39) and low's descent arc (0.81), which the test asserts. It misses on high's low
    fast pass (2.08), low's approach (3.66) and low pass (3.41), and both settings' slowdown (0–0.09);
    the vertical descent's and the hover's are void (the collapse below). Two causes are clear. D
    assumes a ring all round, 4k patches along the leading edge, where the 60° frustum along the
    track sees the edge's chord, about 4k tan(φ ÷ 2), 0.58 of it; and below the cap altitude D's
    vertical term stays positive (h floored at the cap) while nothing new is selected. On the low
    setting D is also low, since below k ≈ 3 the quadtree's granularity floors the count (Design
    note 19's "about a quarter"). **Proposed correction, not the gate:** D_frustum =
    Σ_L 4 k_L tan(φ_x ÷ 2) v ÷ S_L + (3π k² ÷ ln 2) |ḣ| ÷ h above the cap and the vertical term zero
    below it. The whole-descent record has the per-segment figures. For T18 and T19.
  - **Finding: the selection collapses near the ground** (2026-10-03, assigned to lane B): with
    baked ranges, at the hover 1.6 m above the site's collision height (−1,953 m, below the datum),
    the selection falls from 346 patches to none within 0.25 s, forced region included; without
    ranges the same pose selects 10,585. The vertical descent's and the hover's windows pin that
    sequence until the fix, and the full 4σ record waits on it.
  - **Finding: budgeted selection under the hard bound churns** (probe, 2026-10-03, high, low fast
    pass): 952 patches, `limited` 72% of frames, about 830 new keys a frame (53,000 a second
    against D's 311), `selectPatches` p50 116 ms. Lane B fixed it (5d90fab, the streaming gate,
    resident ancestors pinned, deepest evicted first); the hard cells' full record follows the
    collapse's fix.

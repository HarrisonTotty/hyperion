# Plan R05: Terrain Geometry and the Descent Spike (the Gate)

- **Milestone:** Rendering milestone RM2 (the gate).
- **Depends on:** [R01](01-graphics-platform-and-engine.md) (the platform, the engine adapter, the
  per-view canvas contexts, the SwiftShader smoke harness),
  [R02](02-real-scale-view-and-wireframe.md) (the view's camera, reversed-Z, camera-relative
  differencing, `BodyFixedPosition`, the photometric pipeline, the wireframe style and the label
  block) and [R04](04-cross-target-determinism.md) (`hyperion-base`, the `hyperion-surface`
  skeleton, both wasm targets in `just ci`, the terrain hazards, the client's first WebAssembly and
  the owner's ruling on the Content Security Policy, R04.T10.a). Nothing of galaxy plans 12 or 14
  and no session is needed: the spike runs on a scene built by hand.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)): step 3 of
  [Suggested order of attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack),
  the descent spike, in full;
  [The geometry: a quadtree on a cube sphere](../../brainstorming/rendering-and-planets.md#the-geometry-a-quadtree-on-a-cube-sphere)
  in full; the level-selection, collision-level and bound rules of
  [Level-of-detail consistency, and why collision agrees](../../brainstorming/rendering-and-planets.md#level-of-detail-consistency-and-why-collision-agrees)
  as the geometry needs them; the 2 m band limit and 0.5 m spacing of
  [The line between truth and decoration](../../brainstorming/rendering-and-planets.md#the-line-between-truth-and-decoration);
  of [Performance budget](../../brainstorming/rendering-and-planets.md#performance-budget), the
  terrain and atmosphere rows, the patch-demand paragraph, _τ_, the height-worker budget, the
  height-texture cache row and the third rule (the grounded-body rule and `TERRAIN: DETAIL
LIMITED`); Earth's reference atmosphere from
  [Atmosphere](../../brainstorming/rendering-and-planets.md#atmosphere) (Hillaire 2020, four
  tables, Earth's reference values, the physical Rayleigh coefficients); the "Workers hand heights
  to the render thread" bullet of
  [Runtime and code shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape);
  item 7 of
  [What the guide must gain](../../brainstorming/rendering-and-planets.md#what-the-guide-must-gain)
  as the view shows it; of [Testing](../../brainstorming/rendering-and-planets.md#testing), the
  level-of-detail selection, grounded-body, culling and precision tests for patches, and the
  descent by hand; [open question 2](../../brainstorming/rendering-and-planets.md#open-questions)
  in full; and the spike paragraph of the single-player brainstorm's
  [The rendering engine](../../brainstorming/single-player-experience.md#the-rendering-engine).

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
passes or fails at 1080p60 on a discrete GPU of the RTX 4060 class and at 30 fps at 720p on the UHD
620's low setting, and open question 2's rule, with a native wgpu replay and Dawn's safety toggles
priced, says whether a failure is the browser's. `TERRAIN: STREAMING` and `TERRAIN: DETAIL LIMITED`
annunciate on the view from here on.

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
  run, the discrete run and the verdict of open question 2.
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
    pub fn corner_neighbours(self) -> ArrayVec<Self, 4>;     // three at a cube corner
    pub fn vertex_dir(self, x: u8, y: u8) -> [f64; 3];      // vertex (x, y) of 65 × 65, unit
}
pub const MAX_LEVEL: u8 = 24;
pub const PATCH_QUADS: u32 = 64;                            // 65 × 65 vertices
pub const FINEST_SPACING_M: f64 = 0.5;                      // the brainstorm's 0.5 m
pub const BAND_LIMIT_M: f64 = 2.0;                          // the 2 m band limit
pub fn finest_level(radius_m: f64) -> u8;                   // DN 3: Earth → 19
pub fn vertex_spacing(radius_m: f64, level: u8) -> SpacingRange;   // min, mean, max, metres
```

### `hyperion_surface::test_planet` (provisional, replaced by R09)

```rust
pub struct TestPlanet { /* radius_m, seed, octave table (DN 12), rotation (DN 14) */ }
pub const TEST_PLANET: TestPlanet;                          // Earth-sized, hand-parameterised
pub const TEST_PLANET_VERSION: u32;                         // its golden header, DN 13
pub struct HeightSample { pub height_m: f64, pub gradient: [f64; 3] }  // above the reference radius
pub struct LatticeCache { /* per bake, keyed by u64 lattice keys, order-independent */ }
impl TestPlanet {
    pub fn height(&self, dir: [f64; 3], level: u8, cache: &mut LatticeCache) -> HeightSample;
    pub fn octaves_at(&self, level: u8) -> OctaveSet;        // fixed per level, faded, DN 12
    pub fn level_bound_m(&self, level: u8) -> f64;           // DN 15, derived and checked in T6
    pub fn height_range_m(&self, level: u8) -> (f64, f64);   // bounds for culling, DN 8
}
pub mod num { pub fn min(a: f64, b: f64) -> f64; pub fn max(a: f64, b: f64) -> f64; }  // DN 13
```

### `hyperion_surface::patch`

```rust
pub enum VertexPath { BakedOffsets, FaceDifferences }       // DN 4
pub enum NormalScale { Mesh, Double }                       // 65² or 129² normals, DN 5
pub struct BakeOptions { pub vertex_path: VertexPath, pub normals: NormalScale, pub skirt_m: f64 }
pub struct PatchBake {
    pub key: PatchKey,
    pub origin: [f64; 3],            // body-fixed metres; the client wraps it as BodyFixedPosition
    pub heights: Vec<f32>,           // 65 × 65 × 2: own-level height, morph target (DN 5)
    pub offsets: Option<Vec<f32>>,   // BakedOffsets only: q0, q1 per vertex, from the origin
    pub normals: Vec<f32>,           // octahedral pairs, body-fixed; `rg16float` on the GPU (DN 5)
    pub height_range_m: (f32, f32),
    pub bounding_radius_m: f64,      // about `origin`
}
pub fn bake_patch(planet: &TestPlanet, key: PatchKey, opts: &BakeOptions,
    cache: &mut LatticeCache) -> PatchBake;
pub fn finest_surface_height(planet: &TestPlanet, dir: [f64; 3],
    cache: &mut LatticeCache) -> f64;                        // the collision interpolant, DN 6
```

The wasm entry point (T5) exports `bake_patch` to the height workers as owned typed arrays whose
buffers the worker transfers (Design note 11), through the binding shape R04's loader established.

### Client: `view/terrain/`

```ts
// cube.ts, patchKey.ts: the mirror, bit-exact against the Rust goldens (DN 1)
function stToUv(s: number): number;
function uvToSt(u: number): number;
function faceUvToDir(face: Face, u: number, v: number): Vec3;
type PatchKey = { face: Face; level: number; i: number; j: number };
function patchKeyString(k: PatchKey): string; // map keys; the u64 word does not fit a number

// bounds.ts, cull.ts
type PatchBounds = { centre: BodyFixedVec3; radiusM: number; minH: number; maxH: number };
function patchBounds(planet: PlanetGeometry, key: PatchKey): PatchBounds;
function inFrustum(b: CameraRelativeBounds, f: Frustum): boolean;
function aboveHorizon(b: CameraRelativeBounds, h: HorizonCone): boolean;

// select.ts, grounded.ts
type SelectionInput = {
  planet: PlanetGeometry;
  views: readonly ViewSelectionInput[]; // pose, fov, viewport, priority
  setting: QualitySetting;
  grounded: readonly GroundContact[]; // body-fixed, radius, DN 9
};
type Selection = { patches: ReadonlyMap<string, SelectedPatch>; demand: readonly PatchRequest[] };
function selectPatches(input: SelectionInput): Selection; // pure, DN 7
function morphHold(v: BodyFixedVec3, grounded: readonly GroundContact[]): number; // DN 6
function screenSpaceErrorPx(boundM: number, distanceM: number, view: ViewSelectionInput): number;

// cache.ts
class PatchCache {
  constructor(layout: SlotLayout); /* fixed slots from R10's layout, LRU, pins, DN 10 */
}
function resolveDrawSet(sel: Selection, cache: PatchCache): DrawSet; // ancestor fallback

// workers/: pool.ts, heightWorker.ts
class HeightWorkerPool {
  constructor(opts: { workers: number; wasmUrl: URL });
  request(r: PatchRequest): void;
  cancelStale(keep: ReadonlySet<string>): void;
  onBaked(cb: (bake: BakedPatch) => void): () => void;
  postField(bytes: ArrayBuffer): void;
}

// terrainPass.ts, shaders/terrain.wgsl: the pass through R01's adapter
// annunciation.ts
function terrainAnnunciation(
  draw: DrawSet,
  sel: Selection,
  reference: Selection,
): "TERRAIN: STREAMING" | "TERRAIN: DETAIL LIMITED" | null; // DN 23
```

### Client: `view/atmosphere/`

```ts
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
  drawFrame(view: ViewFrame, sun: SunState): void; // sky-view, aerial perspective, ray march
}
const TABLE_SIZES: Record<QualitySetting, TableSizes>;
```

### Client: `view/spike/` and the main process

`descentProfile.ts` (the scripted path, pure), `demand.ts` (the predicted demand), `spikeScene.ts`,
`metrics.ts` (frame intervals, pass timestamps, upload and pipeline tallies, memory tallies),
`DescentSpike.tsx`; `apps/hyperion/src/main/spike.ts` (tracing, process memory, fdinfo, the
results file); the client flag `--descent-spike`; the recipes `just descent-spike` and
`just replay`; `tools/gpu-replay/`, a native wgpu replayer outside the workspace.

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
  `createView`, `createMesh`, `createMaterial` from a `WgslMaterialSpec`, `createPostProcess`,
  `createCompute` from a `KernelPair`, `onFault`; `RenderView`; `FrameSubmission` and `DrawItem`
  with its `offsetFromCameraM`), `loadRenderEngine` behind its dynamic import, `GpuCapabilities`
  (`shaderF16`, `subgroups`, `timestampQuery`), the WGSL-only guard, one canvas context per view on
  one device, the fault path (`GraphicsFault`, `GraphicsStatusStore`), the refusal of a fallback
  adapter; in the main process `graphicsSwitches` with its `gpuTiming` option and
  `GPU_TIMING_SWITCH`, which lift timestamp quantisation for measurement runs, and
  `mergeSwitchValue`, through which this plan adds Dawn's safety toggles to R01's
  `--enable-dawn-features` list rather than replacing it (Design note 18); `WGSL_CATALOGUE`, in
  which every pass here is registered so that the offline render test and the SwiftShader smoke
  harness (`just test-render`) render it; and `FakeGpu` for tests. Two asks, since the interface as
  sketched names meshes, materials and compute kernels but not their resources: per-draw sampled
  textures (the height and normal textures, the atmosphere's tables) and compute kernels that write
  2D and 3D storage textures (Hillaire's tables), with the byte sizes of both visible to the
  allocation tally (Design note 18).
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
  `compile_error!` and its crate-boundary entries; `wasm32-unknown-unknown` under
  `wasm-bindgen-test` through `tools/electron-node/node`, both wasm targets' fast goldens in
  `just ci` (`just test-wasm-fast`) and the slow wasip1 suite in `just ci-slow`; the testkit's
  embedded golden arm; the terrain hazards section of the sim-determinism skill, which binds every
  Rust task here; `just gen-surface`, which builds the surface crate's module and glue for the
  client; the module loader and probe worker in `apps/hyperion/src/renderer/src/wasm/`, which this
  plan's height workers extend; and the owner's ruling on the CSP question (R04.T10.a: add
  `'wasm-unsafe-eval'`, change nothing, or serve through a custom scheme), which gates R04's loader
  and so T10.b.
- **Galaxy plan 14:** nothing built. `body_fixed_at` (P14.T14.c) does not exist; the test planet
  carries its own rotation in the shape R02's rotation interface takes (Design note 14).
- **Tree facts relied on** (checked 2026-09-29): `crates/hyperion-server/src/config.rs`'s
  `--num-workers` / `HYPERION_WORKERS`, which the spike's single-player run caps; the client's
  commander CLI in `apps/hyperion/src/main/cli.ts`, which gains the spike flag; the justfile's `ci`
  recipe, which gains nothing of the spike's (Design note 20).

## Design notes

"Researched 2026-09-29" marks a note settled by a research agent for this plan; its sources are
given, and the question and answer are in the plan's notes. Figures a task turns into code are
re-checked against the cited source in that task, as the galaxy README's Figures rule requires.

1. **What is Rust and what is TypeScript.** The height function, the cube-sphere mapping, the
   patch bake and the collision interpolant are authoritative and live in `hyperion-surface`,
   because both sides must agree on them bit for bit. Patch selection, culling, balance, the
   cache, streaming priority and the draw submission are presentation and live in the client
   under R02's `view/terrain/`, engine-agnostic and tested without a GPU, as
   [The engine is kept at arm's length](../../brainstorming/rendering-and-planets.md#the-engine-is-kept-at-arms-length)
   lists them. Selection needs the bounds of patches not yet baked, so the client carries a
   TypeScript mirror of the warp and the patch geometry. The warp uses only `+ − × ÷` and `sqrt`,
   which IEEE 754 rounds exactly in both languages, so the mirror is pinned bit for bit to a golden
   file the Rust tests write (T2): a mismatch fails a vitest, not a picture. The mirror decides only
   what to draw; every height and vertex position reaching the GPU comes from the worker.
2. **Cube-sphere conventions.** S2's face order and axes (faces 0–5 = +x, +y, +z, −x, −y, −z,
   with S2's per-face (u, v) axes, so that S2's published cell statistics apply unchanged) and its
   quadratic warp, u = (4s² − 1)/3 for s ≥ ½ and (1 − 4(1 − s)²)/3 otherwise, whose inverse is
   s = ½√(1 + 3u) or 1 − ½√(1 − 3u). A point on a face edge or cube corner belongs to the face of
   largest |axis| with ties to the lowest face index, so every direction has one face. A
   `PatchKey` is (face, level, i, j) with `i, j < 2^level`, packed into a `u64` as face (3 bits),
   level (5 bits) and i, j (24 bits each) for cache keys in Rust; the client keys its maps by a
   string, since the word does not fit a JavaScript number. `MAX_LEVEL` is 24, 0.02 m spacing on
   an Earth, above any body's finest level. Edge neighbours across a face edge rotate (i, j) by
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
3. **The finest level per body** (researched 2026-09-29). The brainstorm samples the terrain "at
   0.5 m at its finest level" so that the piecewise-linear interpolant keeps the 2 m band limit
   "to within about a third of its amplitude". On the quadratic warp a level's vertex spacing runs
   from 0.65 to 1.17 times its mean (computed on a 2,048² face grid; the cell-area ratio came out
   2.095 against S2's 2.082), and the mesh is triangles, whose diagonal sees √2 times the axis
   spacing. The worst pointwise error of a plane wave on the triangle mesh is 0.56 of its
   amplitude at an axis spacing of λ/4 (0.5 m), and a third only at about 0.19 λ. So the finest
   level of a body of radius R is **the shallowest level whose largest vertex spacing is at most
   0.375 m**, which keeps the brainstorm's "within a third" true in two dimensions and satisfies
   its 0.5 m. For an Earth (6,371 km) that is level 19: spacing 0.18–0.32 m, mean 0.277 m, patches
   of 17.7 m mean edge. Level 18, the reading nearest the brainstorm's "32 m patches", has a
   largest spacing of 0.65 m and fails the claim everywhere. The brainstorm's 160 m demand cap
   becomes about 89 m at level 19. `finest_level(R)` is one function of the radius and
   `FINEST_SPACING_M`, stated in the crate's documentation, as open question 6 requires of the
   band limit; R09 inherits both. Reported for a brainstorm revision in Risks.
4. **Two vertex paths, measured.** The vertex shader never forms (R + h)·dir in `f32`. The
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
     numbers is formed. Cost: 8 bytes a vertex of height texture, about 34 KB a patch, and more
     vertex arithmetic.
     T4.b's precision test holds both to under 1 mm against the `f64` positions at level 19 on an
     Earth; the spike records upload bytes, memory and vertex time for each, and the verdict (T18)
     keeps the cheaper. `BakedOffsets` is the default on the high setting until then, since its
     precision holds by construction; the low setting uses `FaceDifferences` from the start,
     because `BakedOffsets` does not fit its 64 MiB cache (researched for R10, 2026-09-29: the
     all-round set at 100 m altitude is about 332 patches, 74 MB at 224 kB a patch with
     `BakedOffsets`, against about 545 slots at 123 kB without). T18 may choose `BakedOffsets` for
     the high setting only.
5. **What a patch carries.** Height is height above the reference radius, not radius, so its `f32`
   step at ±20 km is about 2 mm. The second channel is the morph target: at a vertex both of whose
   indices are even, the parent level's height there; at any other vertex, the parent mesh's own
   linear interpolation of its even neighbours, on the parent mesh's triangle diagonal (researched
   2026-09-29; holding the parent level's height evaluated at an odd vertex would not close the
   crack, because the parent draws a chord there). Normals come from the height function's analytic
   gradient, taken tangentially and corrected by R ÷ (R + h), stored in the body-fixed frame as
   octahedral pairs, returned by the bake as `f32` and packed by the worker into a `Float16Array`
   for an `rg16float` texture, which is core and filterable in WebGPU (angular error about 0.05°);
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
8. **Culling predicates.** Both run on the CPU in `f64` on camera-relative bounds. The frustum
   test uses the patch's bounding sphere and then its oriented box against the six planes (the far
   plane is infinite and omitted); a patch larger than the frustum and a camera inside a patch's
   volume both pass. The horizon test (Ring 2013, the Cesium horizon-culling method; Cozzi and
   Ring 2011) takes the occluder as the sphere of radius R_occ = R + h_min, the planet's lowest
   possible height, scales space by 1 ÷ R_occ, and with camera C and vt = P − C calls a point P
   occluded when −vt·C > |C|² − 1 and (vt·C)² ÷ |vt|² > |C|² − 1; a patch is culled only if all
   eight corners of its box, raised to its maximum height, are occluded, which is exact for a convex
   box. Exact tangency is visible, and a camera below R_occ disables the test. Researched
   2026-09-29.
9. **Grounded and descending bodies.** The brainstorm holds the finest level, morph at zero,
   "within a stated radius of every grounded or descending body in view". Until craft exist the
   input is a list, `GroundContact { position: BodyFixedPosition, radius_m }`, which the spike
   fills with its scripted craft and R10 and the sessions fill from the scene. A body is
   _descending_ while it is below 1 km above the reference surface under it and moving towards
   it, or while its time to contact at its present vertical speed is under 30 s (provisional
   figures; the spike records how long the forced patches take to become resident, and T18 sets
   them so that the region is resident before contact). The held radius r_g is the body's
   bounding radius plus one finest patch, and the forced region adds the ramp and its one-patch
   margin (Design note 6). On an Earth at level 19 a 20 m craft forces a few tens of 17.7 m
   patches, the brainstorm's "handful". The rule binds every setting and every style: the low
   setting's τ, the patch cache and every view's own tolerance are all overridden inside the
   forced region.
10. **The patch cache, and who owns its layout.** Patches are cached by patch, not by view, in
    one cache per body. R10 owns the cache's sizes, formats and slot layout per setting (its Design
    note 15 and T14); this plan owns the cache's behaviour, the eviction rule, the pins and the
    draw-set resolution, and builds the layout R10 specifies from the start, so that nothing is
    rebuilt when R10 lands (reconciled 2026-09-29 with R10's research). The layout: the cache is
    preallocated as fixed slots, the slot count derived from the setting's byte budget and the
    bytes a patch takes; heights and morph targets (and, from R10, the horizon map and the survey
    mask), all read with `textureLoad`, live in storage buffers addressed by slot index, tight and
    untiled, while normals, which want filtering, live in a 2D texture array or atlas with a
    one-texel gutter; and the terrain is one instanced draw over the shared 65 × 65 index buffer
    with a slot index per instance, which also keeps the CPU's per-draw cost down. This plan's
    provisional budgets are 64 MiB on the low setting and 256 MB on the discrete target (R10's
    figure: 128 MB does not hold three times the frustum peak); R10 replaces them with its sizing
    rule, max(1.3 × all-round peak, 3 × frustum peak) over the streaming views plus the forced
    region. Eviction is least recently used among unpinned slots. Two kinds of pin: a patch in the
    forced region is never evicted, whatever the budget, and a patch in the current draw set is
    evicted only after everything unpinned. If the pins alone exceed the slots, the cache reports
    it and the draw falls back to stand-ins outside the forced region, which annunciates
    `TERRAIN: STREAMING`. The cache counts the GPU bytes it holds, which the metrics read (Design
    note 18). Horizon maps are R10's: sun-independent (so they are baked once with the patch and
    never rebaked as the sun moves, correcting the brainstorm's "rebaking only as the sun moves"),
    in 16-bit floats, R10's Design note 10; this plan leaves a slot field for them and builds none.
11. **The height-worker pool** (researched 2026-09-29). Each worker is a module worker bundled by
    electron-vite as a same-origin file, started with `new Worker(new URL(…, import.meta.url))` and
    `type: "module"`, with `worker.format: "es"` in the renderer's Vite config so that the build
    matches the dev server; each loads its own instance of `hyperion-surface` built with
    wasm-bindgen's `--target web`, through `init` on a Vite `?url` import, so no `blob:`, no
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
    JavaScript, about 15 MB a worker at rest and twice that briefly; the spike posts a synthetic 15
    MB field to three workers so that the memory it costs is measured before R09 sends a real one.
    Default worker counts: single-player `clamp(floor(hardwareConcurrency ÷ 4), 1, 3)`, two on the
    i7-8665U's eight threads, matching the budget's two cores; a station
    `clamp(floor(hardwareConcurrency ÷ 2) − 1, 1, 4)`, three there; a setting overrides both. A page
    cannot set a worker's thread priority, so the count is the only control. Two findings for R04
    and the owner: a dedicated worker takes its policy from its own script's response, and a
    `file://` worker script has none, so compilation inside the worker probably does not need
    `'wasm-unsafe-eval'` while compilation on the render thread does; and no documented guarantee
    was found that module workers load from `file://` in Electron 44, so T10's first acceptance is a
    smoke test of the built app, with a privileged custom scheme served through `protocol.handle` as
    the fallback, which Electron's security checklist prefers anyway.
12. **The test planet** (researched 2026-09-29). An Earth-sized (6,371 km), dry world whose height
    is a sum of 3D improved Perlin gradient noise octaves (Perlin 2002: the reference
    implementation's 16-entry gradient table, the twelve cube-edge directions padded with (1, 1, 0),
    (0, −1, 1), (−1, 1, 0) and (0, −1, −1), indexed by four bits of one Threefry word, so that a
    uniform index gives an exactly zero-mean gradient, since both the twelve and the four padding
    vectors sum to zero; a multiply-high of a 64-bit word by 12 would not be exactly uniform, since
    2⁶⁴ is not a multiple of 12, and was dropped on R09's research of 2026-09-29; quintic fade)
    evaluated at the point on the sphere in metres. The octave of index k has lattice spacing λ_k =
    10,000 km ÷ 2^k and a target RMS σ_k following Earth's topographic spectrum: degree variances
    near ℓ⁻² (Balmino 1993; Rexer and Hirt 2015), which is Hurst exponent 0.5 and a per-octave gain
    of 2^−½, so σ_k = 1,732 m × 2^(−k÷2) for k ≤ 12, and above the ridge–valley scale, where
    landscape spectra steepen (Perron, Kirchner and Dietrich 2008), a gain of ½, σ_k = 27.1 m ×
    2^−(k−12) for k from 13 to 21. The total is σ_h ≈ 2.45 km, Earth's hypsometric standard
    deviation from its bimodal hypsometry (to be confirmed from Earth2014 in T3.b, as the
    brainstorm's own statistics were), and the spectrum puts 0.19% of the variance below 35 km, 106
    m RMS, inside the brainstorm's 0.1–0.2% and 35–125 m. The finest octave is k = 21 (4.9 m), so no
    lattice is finer than the 2 m band limit; Perlin noise is only roughly band-limited (Lagae et
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
    goldens will read it): a change to the test planet moves no universe.
14. **The test planet turns.** A grounded or hovering camera must see still ground, so the scripted
    path is defined in body-fixed coordinates and turned into the body frame each frame by the
    planet's rotation. Until P14.T14.c's `body_fixed_at` exists the test planet carries a fixed pole
    and a sidereal period of 86,164.0905 s (IERS; re-checked in T13.a) in the shape R02's rotation
    interface takes, reducing the angle from the integer span since its epoch, as `body_fixed_at`
    will. Patch origins are `BodyFixedPosition`s rotated into the body frame in `f64` once a frame
    and then differenced against the camera (R02).
15. **The level bound** (researched 2026-09-29). ε_n is a hard analytic bound on the distance
    between the level-n mesh and the finest one: the sum of the omitted octaves' certified maxima
    plus level n's linear-interpolation error, B ÷ σ_noise × Σσ_k over the omitted octaves plus
    h²÷8 × the included octaves' curvature bound. B, the certified maximum of the noise basis, is
    computed offline by grid search with a Lipschitz margin and pinned (expected 1.04–1.1; √(N÷4)
    is a heuristic, not a bound), and σ_noise, its RMS, is measured and pinned. The contract is the
    hard bound, as the brainstorm's test promises: T6's slow test asserts that no sampled
    difference ever exceeds it, and that the 99.9th percentile is at least a quarter of it, so that
    a loose bound cannot silently over-refine. If it is looser than that, selection gains an
    explicit calibration factor, named and recorded, never a sampled maximum in the contract. The
    brainstorm's patch-to-distance ratio of about five is provisional until this bound exists, and
    T6 records the ratio it implies per level: for fractal relief ε_n falls more slowly than the
    patch size, so the ratio grows towards the finest levels, and demand with it.
16. **Earth's atmosphere** (researched 2026-09-29). Hillaire 2020's four tables, from a list of
    medium terms (density profile, scattering and absorption per channel, phase function), the
    shape R08 generalises. The WGSL is ported from Bevy 0.19's atmosphere (MIT or Apache-2.0,
    already WGSL, already a list of terms, with a ray-marched mode for views from space), with
    sebh's UnrealEngineSkyAtmosphere (MIT) as the numerical reference, both licence notices kept in
    the ported files. Babylon's own Hillaire atmosphere in `@babylonjs/addons` is not used: it
    assumes forward depth with an infinite far plane, has exactly three media, rewrites the
    directional lights' colour and intensity every frame and is marked experimental. Earth's terms,
    each cited in T12.a:
    - **Rayleigh**, exponential with an 8 km scale height, 4.85, 11.5 and 28.7 × 10⁻⁶ m⁻¹ at 680,
      550 and 440 nm, from Peck and Reeder 1972's refractivity with Bates 1984's King factor at
      288.15 K and 101,325 Pa (the US Standard Atmosphere's sea level, N = 2.547 × 10²⁵ m⁻³);
      recomputed, they match Bucholtz 1995's 4.51 × 10⁻²⁷ cm² at 550 nm to 0.1%. The tutorials'
      5.8, 13.5, 33.1 × 10⁻⁶ are a pure λ⁻⁴ law with no King factor and are not used. The
      recomputation lives in a test; R08 builds the per-gas formula.
    - **Aerosol**, exponential with a 1.2 km scale height, Cornette–Shanks phase with g = 0.76
      (Bruneton 2008, the brainstorm's value), an optical depth of 0.1 at 550 nm with Ångström
      exponent 1.3 and single-scattering albedo 0.92: Earth's measured continental aerosol
      (AERONET and MODIS climatologies), not Hillaire's reference, whose 5.3 × 10⁻³ at every
      wavelength is 20–40 times cleaner than a typical sky and whose flat spectrum is unphysical.
      Hillaire's values stay available as a comparison mode against his published images.
    - **Ozone**, absorption only, a tent from 10 to 40 km peaking at 25 km, 300 Dobson units, with
      Serdyuchenko et al. 2014's 233 K cross-sections binned at the three wavelengths (0.650, 1.881
      and 0.085 × 10⁻⁶ m⁻¹ at the peak), as Bruneton 2017 does.
    - Top of the atmosphere 100 km above the reference radius; ground albedo 0.1.

    The transmittance and multiple-scattering tables depend on the atmosphere alone, per unit
    illuminance, so they are rebuilt when the atmosphere changes, not when the sun moves (a
    correction to the brainstorm's "whenever the atmosphere or the sun changes", harmless since they
    are cheap). The sky-view and aerial-perspective tables are rebuilt every frame. Views from above
    the atmosphere, and terrain beyond the aerial-perspective volume, take a per-pixel ray march.
    Table sizes, high (Hillaire's code): transmittance 256 × 64, multiple scattering 32 × 32,
    sky-view 192 × 108, aerial perspective 32³ reaching 32 km (Bevy's reach; the brainstorm's 32 km
    is Bevy's, where sebh's square-root slices reach 128 km). Low: the two per-planet tables at full
    size (128 KB, rarely built), sky-view 128 × 64 with at most 16 samples, aerial perspective 32 ×
    32 × 16, the ray march at half resolution with a depth-aware upsample and about 16 samples, and
    aerial perspective applied in one deferred pass to terrain alone, which is what the budget's
    "aerial perspective on terrain only" means. Absolute luminance is the tables' value times the
    Sun's top-of-atmosphere illuminance per channel, scaled so that its luminance is the Sun's (from
    R02's V-magnitude photometry, which gives 1.28 × 10⁵ lx for V = −26.76), and the three spectral
    samples are turned into Rec. 709 by Bruneton's spectral-radiance-to-luminance factors rather
    than read as RGB. The tables stay in their per-unit-illuminance form, never in absolute units,
    and the sun's disc is clamped after pre-exposure.

17. **The spike's lit view is not a render style.** R07 owns the photorealistic style, AgX, the
    histogram and bloom. The spike draws its terrain lit by one directional light from a Sun-like
    star 1 au away, shaded Lambertian with a single visual albedo of 0.15 (a hand value, labelled),
    in `rgba16float` with pre-exposure, and maps it to the display with R02's tone curve as one
    full-screen pass under a `MAN` exposure of EV100 15 by day, set per segment by the script. It
    is a spike scene inside R02's `VIEW`, and R07 replaces its shading, exposure and tone mapping.
    Because the spike's frame lacks the histogram, bloom, clouds, ocean, shadows and scatter, the
    pass criterion (Design note 21) reserves their budget rather than counting it as headroom.
18. **Metrics and the measurement switches** (researched 2026-09-29). A scripted run records, per
    frame and per altitude band: frame intervals from presentation times in the trace, falling back
    to `requestAnimationFrame` timestamps, at the 50th, 95th and 99th percentiles, with missed and
    hitching frames counted; GPU time per pass from each pass's `timestampWrites` at its start and
    end (timestamps inside passes need `--enable-unsafe-webgpu`, which is never set); main-thread
    time split into our code (`performance.measure` spans), the engine (CPU-profiler self time in
    Babylon's own chunk, which R01's manual chunk makes separable) and idle; patches a second
    requested, baked and made resident, against the predicted demand (Design note 19); upload bytes
    from the adapter's `writeBuffer` and `writeTexture` tally; pipeline creations after warm-up,
    from a shim on `createRenderPipeline`, `createComputePipeline` and their asynchronous forms,
    each late one logged with its label, cold and warm caches run separately; garbage-collection
    pauses per thread from V8's GC trace slices; and memory at 1 Hz: `app.getAppMetrics()` and the
    renderer's `process.getProcessMemoryInfo()`, the GPU process's DRM fdinfo (`drm-total-*`,
    `drm-resident-*`, summed over client IDs, since ANGLE and Dawn open their own), on NVIDIA
    `nvidia-smi -q -x`'s process list with the device's `memory.used` less a baseline (not
    `--query-compute-apps`, which lists compute processes only and can miss the GPU process;
    researched for R12, its Design note 6), and the adapter's own tally of buffers and textures, against
    the 1 GB ceiling. The trace comes from Electron's `contentTracing` in the main process, over
    categories that include `devtools.timeline`, `disabled-by-default-v8.gc`,
    `disabled-by-default-v8.cpu_profiler`, `blink.user_timing` and `gpu`, and is reduced to the
    results file in the main process (T14.b). Chromium quantises WebGPU timestamps to 65.5 µs by
    default (Dawn's `timestamp_quantization` toggle, mask `0xFFFF0000` on the low word), which the
    brainstorm's "the forced switches make available, uncoarsened" gets wrong; measurement runs
    alone lift it through R01's `gpuTiming` option (`GPU_TIMING_SWITCH`), which the spike flag turns
    on before `ready`. The safety toggles of Design note 22 are merged into R01's
    `--enable-dawn-features` list with `mergeSwitchValue`, since appending a second switch would
    replace the first.
19. **The scripted descent and its predicted demand** (researched 2026-09-29). The path is a pure
    function of script time in body-fixed coordinates over a landing site and approach azimuth
    drawn from the spike's seed, and the frames sample it at their own display times, so the
    trajectory is identical every run while the frame rate is free, which is what streaming must be
    measured against. A second, fixed-step mode samples it at 64 Hz for the CPU-only tests, whose
    selection sequence is then identical bit for bit. Its segments (provisional, T13.a):

    | Segment             | Altitude        | Horizontal speed  | Vertical speed    | Duration | Predicted demand, high |
    | ------------------- | --------------- | ----------------- | ----------------- | -------- | ---------------------- |
    | Orbit coast         | 400 km          | 7.67 km/s         | 0                 | 60 s     | 4 a second             |
    | Descent arc         | 400 km to 20 km | 7.67 to 1 km/s    | about −420 m/s    | 900 s    | 4 to 16 a second       |
    | Approach and flare  | 20 km to 300 m  | 1 km/s to 300 m/s | falling to 0      | 120 s    | 16 to 200 a second     |
    | Low fast pass       | 300 m           | 300 m/s           | 0                 | 30 s     | 200 a second           |
    | Slowdown            | 300 m to 200 m  | 300 m/s to 0      | about −2 m/s      | 60 s     | 200 to 3 a second      |
    | Vertical descent    | 200 m to 2 m    | 0                 | −20 m/s           | 10 s     | 29 to 65 a second      |
    | Hover and touchdown | 2 m to 1 m      | 0                 | −0.05 m/s, then 0 | 50 s     | near 0                 |

    The figures are the formula below at the segments' ends; the low setting's are about a ninth.
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
    run measures the ratio rather than assuming it. The measured demand, first-time-selected keys a
    second with the cache's semantics stated, must lie within a factor of two of the prediction
    (T13.a), and the recorded figure is patches a second sustained against it, as the brainstorm's
    descent test asks.

20. **Where the spike lives.** In the client, behind a command-line flag, `--descent-spike`, that
    opens a `DescentSpike` view in place of the consoles (the flag is parsed by the existing
    commander CLI and reaches the renderer through the preload's existing configuration path),
    with a `just descent-spike` recipe that builds and runs it with the chosen setting, toggles and
    output directory. Nothing of the spike runs in `just ci`, whose checks stay the CPU tests, the
    goldens on three targets and the SwiftShader smoke test, which gains the terrain and
    atmosphere passes (R01's harness: every shader compiles and a frame completes with finite
    texels, with and without `shader-f16` and `subgroups`). The native replayer is
    `tools/gpu-replay/`, a Rust binary with its own `[workspace]` table outside `crates/`, and the
    root manifest gains `exclude = ["tools/*"]`, so `just ci` never builds wgpu; `just replay` runs
    it. The spike's single-player runs start the local server with its pool capped by
    `--num-workers 2` and, in a second run, a companion CPU load on two threads standing in for the
    server's arrival work (the sky's census and the coarse pass, neither of which exists yet).
21. **The pass criterion** (researched 2026-09-29). The brainstorm's "passes at 1080p60 on the
    discrete target and at 30 fps at 720p on the UHD 620's low setting" is made checkable as frame
    intervals from presentation times over the whole scripted descent after a declared 10 s
    warm-up, and again per segment, so that a failure near the ground cannot be averaged away. The
    UHD 620 run is paced to every second vsync of a 60 Hz display, since uncapped intervals
    alternate between 16.7 and 33.3 ms and their percentiles mean nothing.

    | Criterion             | 1080p60, discrete (T = 16.7 ms)                                         | 720p30, UHD 620 low (T = 33.3 ms)                          |
    | --------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------- |
    | 50th percentile       | ≤ T + 0.5 ms                                                            | ≤ T + 0.5 ms                                               |
    | 95th percentile       | ≤ T + 1 ms                                                              | ≤ 35 ms                                                    |
    | 99th percentile       | ≤ 2T                                                                    | ≤ 2T                                                       |
    | Missed frames         | ≤ 1% of intervals above 1.5 T                                           | ≤ 1% of intervals above 1.5 T                              |
    | Hitches               | none above 3 T                                                          | none above 3 T                                             |
    | Headroom              | main thread and GPU pass sum, each ≤ 0.8 T at the 95th percentile       | the same                                                   |
    | The rest of the frame | terrain and atmosphere GPU time within their rows' upper ends: 5 + 1 ms | 14 + 4 ms                                                  |
    | Memory                | GPU resident ≤ 2–3 GB                                                   | GPU resident ≤ 1 GB, with the 15 MB field in three workers |

    The last two rows keep the gate honest about what the spike does not draw (Design note 17):
    the budget's other rows must still fit beside terrain and atmosphere. Patches a second
    sustained are recorded against the predicted demand but are not a pass criterion: where demand
    outruns the workers, the view annunciates `TERRAIN: STREAMING`, as the budget intends, and the
    run records how long and where. Percentile and missed-frame conventions follow frame-time
    practice (PresentMon's `MsBetweenPresents`, Chromium's dropped-frame metric); the headroom row
    is the defined meaning of open question 2's "fit with headroom".

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
    the drawn selection is coarser than the reference selection: the same pure selection at τ = 1
    px at the view's own resolution, with no depth cap. So the low setting annunciates the second
    whenever terrain is in view, which is honest: its 2 px tolerance draws the surface
    below what the camera's position warrants. Where both hold, `STREAMING` shows, because it clears
    by itself and the operator answers it differently. Each is steady text in `--text` on its
    plate, with no status colour and no flashing; a condition must hold for 250 ms before it shows
    and clear for 1 s before it goes, so that a patch arriving mid-frame cannot make the line
    flicker, which also keeps it within the guide's three-flashes-a-second limit. The wording is
    item 7 of the guide edits R02 drafts for the owner; this plan builds to the draft.
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
    patches a second for each, and the verdict keeps the doubled normals only if the descent still
    streams within the budget; otherwise the finest levels fall back to mesh-resolution normals, a
    labelled shading loss, never a geometric one. Reported as a brainstorm inconsistency.
26. **The low setting, in one place.** τ = 2 px at 720p presented upscaled; normals at the mesh's
    resolution; a 64 MiB patch cache of fixed slots, with the `FaceDifferences` vertex path; the
    atmosphere's low tables with aerial perspective on terrain only and the ray march at half
    resolution; one photorealistic view with the two wireframe instruments; `TERRAIN: DETAIL
LIMITED` whenever terrain is in view. Full depth under grounded bodies on every setting. Each is
    built in the task that builds its high form, as the budget's third rule requires.

27. **Timings need a quiet machine.** The development machine is shared with other agents' test
    runs, and every figure measured on it so far (the Threefry block at 22.8–41.7 ns against the
    repository's 10 ns, and any bench a task runs while other work is going) is provisional. Every
    benchmark and every recorded run in this plan (T3.c, T4, T10.b, T16, T17 and the replay) is
    taken on a quiet machine: no other test, build or agent running, the load average under 1
    before the run starts and recorded with it, and the CPU governor recorded. A figure taken
    otherwise is marked provisional in its results file and does not count towards the verdict.

## Tasks

T1 to T6 are Rust in `crates/hyperion-surface/` and a chain, T3 beside T1.b once T1.a exists. T7
to T9 are pure TypeScript and follow T2 and T6, whose bound table they read; they can run beside
T4 and T5. T10
needs T5 and R04's WebAssembly load. T11 needs T4, T7 and T8; T12 needs only R01 and R02 and can
run beside everything from T1. T13 and T14 need T9 to T12. T15 follows T14. T16 and T17, the runs,
need everything before them; T18 follows both, and T19 closes. Every Rust task runs under the
sim-determinism skill's terrain hazards section (R04) and is audited by the determinism auditor;
every renderer task that touches the label block follows the `console-ux` skill.

Rust files are under `crates/hyperion-surface/` and TypeScript files under
`apps/hyperion/src/renderer/src/` unless a path says otherwise.

### R05.T1 The cube sphere

**R05.T1.a The warp and the faces.** `cube::{Face, FaceUv, st_to_uv, uv_to_st, face_uv_to_xyz,
xyz_to_face_uv}` per Design note 2, with the warp and its inverse cited to S2's `s2coords.h`
(re-check the quadratic forms and the area ratio there).

- Files: `src/cube.rs`, `src/lib.rs`.
- Tests: `uv_to_st(st_to_uv(s))` returns s to one ulp over 10⁴ values and exactly at 0, ¼, ½, ¾ and
  1; `xyz_to_face_uv(face_uv_to_xyz(f))` round-trips to 10⁻¹⁵ in u and v; every direction has
  exactly one face, ties on edges and corners resolved by the lowest index; lines of constant u lie
  on planes through the centre (great circles); the ratio of largest to smallest cell area at level
  10 is 2.082 ± 0.01 (S2's figure).
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
  fail; a cube-corner patch has seven neighbours; `finest_level(6.371e6)` is 19, and
  `vertex_spacing` there gives a mean of 0.277 m ± 1% and a maximum of at most 0.375 m; the finest
  levels of the Moon (1,737.4 km) and Ceres (469.7 km) are pinned; for 100 radii from 100 km to
  70,000 km the finest level's largest spacing is at most 0.375 m and the level above's is not.
- Acceptance: `cargo test -p hyperion-surface cube:: geometry::`.

### R05.T2 The cube sphere's golden and the TypeScript mirror

A golden file of the warp at 1,000 values of s, the direction of every vertex of a fixed set of 50
patch keys at levels 0 to 24 on all six faces, the edge-neighbour table and `finest_level` for 20
radii, written as hexadecimal bits by the testkit's writer under `TEST_PLANET_VERSION`'s header
(Design note 13). R04's checks assert it on `wasm32-wasip1` and `wasm32-unknown-unknown`. The
client's `view/terrain/cube.ts` and `patchKey.ts` mirror the Rust functions line for line, and a
vitest reads the same golden file and asserts every value bit for bit (a `Float64Array` over the
parsed bits).

- Files: `tests/cube_golden.rs`, `tests/golden/cube_sphere.txt`, `view/terrain/cube.ts`,
  `view/terrain/patchKey.ts`, `view/terrain/cube.test.ts`.
- Tests: as stated; `patchKeyString` round-trips; the mirror's `finestLevel` agrees for the 20
  radii.
- Acceptance: `just ci` passes with the golden on all three Rust targets and the vitest green;
  `just bless` leaves the file byte for byte unchanged.

### R05.T3 The test planet

**R05.T3.a The noise basis.** Improved Perlin gradient noise in 3D with its analytic gradient
(Design note 12), lattice corners keyed per Design note 13 under the new `SelfTest` tag
`selftest.surface.test_planet` in the surface crate's part of R04's registry, with a table in the
module documentation of which integers go into the object word and the draw number;
`LatticeCache`, a dense array per octave over a bake's lattice box; `num::min`, `num::max` and an
`assert_finite` helper. The certified maximum B and the RMS σ_noise are computed by a slow test
(grid search at 1/256 of a cell with a Lipschitz margin; 10⁶ random points for σ_noise) and pinned
as constants that the test recomputes.

- Files: `src/noise.rs`, `src/num.rs`, the surface crate's tag registry file.
- Tests: the gradient agrees with a central difference to 10⁻⁶ relative at 10⁴ points; the
  ensemble mean over 10⁶ points is zero within three standard errors; no value exceeds B; a bake's
  heights are identical whether the cache is filled in raster, reverse or shuffled order
  (`hyperion_testkit::order::assert_order_independent`); `num::min(0.0, -0.0)` and
  `num::min(-0.0, 0.0)` are both −0.0 whether or not the inputs are known at compile time
  (`black_box`), and `num::max` of the pair is +0.0 both ways; the tag registry's collision check
  still passes.
- Acceptance: `cargo test -p hyperion-surface noise:: num::`; `just test-slow` runs
  `noise_bound_certified`.

**R05.T3.b The octave stack.** `TestPlanet`, `TEST_PLANET`, `height`, `octaves_at`,
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
  `just test-slow`.

**R05.T3.c Goldens and cost.** A golden of `height` and its gradient at 500 fixed directions and
levels, ridged on and off, asserted on native and both wasm targets through R04's checks, and a
Criterion bench of a 65 × 65 bake's worth of points with and without the lattice cache.

- Files: `tests/test_planet_golden.rs`, `tests/golden/test_planet.txt`,
  `benches/test_planet.rs`.
- Tests: the golden; the bench.
- Acceptance: `just ci` green on all three targets; `just bench -- test_planet` reports the
  microseconds a point with its gradient, recorded in the module documentation with the machine and
  its load average, measured on a quiet machine (Design note 27); a figure taken while other work
  shares the machine is marked provisional and re-measured. About 2 µs cached and 6 µs uncached are
  the research estimate and 10 µs the budget; more than 10 µs is a finding for T16.

### R05.T4 The patch bake

**R05.T4.a Heights, morph targets, normals, bounds and skirts.** `patch::{BakeOptions, PatchBake,
bake_patch}`: own-level heights; the morph-target channel of Design note 5, on the mesh's diagonal
convention, which the module documentation fixes (each quad split from its (0, 0) corner to its
(1, 1) corner in patch index space); normals from the analytic gradient as octahedral `f32` pairs at
`NormalScale::Mesh` or `Double`; the height range, the bounding radius and the skirt depths.

- Files: `src/patch.rs`, `src/patch/normals.rs`.
- Tests: two patches sharing an edge have bitwise-equal heights along it, within a face and across
  face edges; at morph 1 every vertex of a child lies on its parent's mesh to the `f64` rounding;
  decoded normals agree with the gradient's to 10⁻⁴ rad; the height range contains every baked
  height; the bake is independent of the lattice cache's fill order.
- Acceptance: `cargo test -p hyperion-surface patch::`.

**R05.T4.b The two vertex paths.** `VertexPath::BakedOffsets` (q₀ and q₁ per vertex, narrowed from
`f64`) and the `FaceDifferences` formula of Design note 4, implemented in `f64` and as an `f32`
reference performing exactly the operations the WGSL will, in the same order.

- Files: `src/patch/vertex.rs`.
- Tests: at level 19 on an Earth, over 100 patches spread over the six faces and their corners,
  both paths reproduce the `f64` vertex positions relative to the origin to under 1 mm, and the
  naive (R + h)·dir in `f32` does not, which is asserted too, so that the test would have caught
  the naive form; at level 0 both stay within 1 m, where such a patch is seen from 10⁷ m.
- Acceptance: `cargo test -p hyperion-surface patch::vertex`.

**R05.T4.c The collision interpolant.** `finest_surface_height`: the finest level's vertex heights
around the query direction, interpolated on the fixed diagonal.

- Files: `src/patch/collision.rs`.
- Tests: at every vertex of 20 finest-level patches it returns the baked height bitwise; inside a
  quad it is the barycentric interpolation on the stated diagonal; on a shared edge it is the same
  from either patch; it never reads a level other than the finest.
- Acceptance: `cargo test -p hyperion-surface patch::collision`.

### R05.T5 The workers' entry point

`bake_patch` exported to JavaScript through the binding shape R04 established, returning owned typed
arrays per Design note 11 (heights and morph targets as one `Float32Array`, offsets as another where
the path needs them, normals as a `Float32Array` that the worker packs into a `Float16Array`, and
the scalars), with a layout comment the TypeScript side mirrors; and a golden that the bytes of
three baked patches hash the same as the native bake's.

- Files: `src/wasm.rs` (under `cfg(target_arch = "wasm32")`), `tests/bake_golden.rs`,
  `tests/golden/bake.txt`.
- Tests: the golden on all three targets.
- Acceptance: `just ci` green, with the `wasm32-unknown-unknown` run executing the golden under
  Electron's V8 through R04's `node` shim.

### R05.T6 The level bound

`TestPlanet::level_bound_m(n)`, the analytic bound of Design note 15 per level from the octave
table, B, σ_noise and the interpolation term, with its derivation in the documentation, added with
the height ranges to T2's golden for the client's selection; and the slow test that measures the
mesh-to-mesh distance |S_n(p) − S_f(p)| between each level's mesh and the finest's at 10⁴ sample
points a level, the finest through `finest_surface_height` and level n through its own vertices on
the same diagonal rule.

- Files: `src/test_planet/bound.rs`, `tests/level_bound.rs`.
- Tests: no sample exceeds the bound at any level; the 99.9th percentile is at least a quarter of
  it at every level; the implied patch-to-distance ratio k_n = ε_n ÷ (S_n τ θ_px) at τ = 1 px,
  1080p and 60° is written out and recorded in the documentation for T13.a.
- Acceptance: `just test-slow` runs `level_bound_holds`; the ratios are in the module
  documentation.

### R05.T7 Selection

**R05.T7.a Bounds and culling.** `patchBounds`, from the mirror and the per-level height ranges in
T2's golden, and `inFrustum` and `aboveHorizon` (Design note 8), all in `f64` on camera-relative
vectors from R02's differencing.

- Files: `view/terrain/bounds.ts`, `view/terrain/cull.ts`, their tests.
- Tests: a patch larger than the frustum passes; a camera inside a patch's volume passes; from
  400 km, a patch just beyond the geometric horizon at its maximum height is visible and at its
  minimum height is culled; a 9 km peak beyond the horizon seen from 300 km is visible; a camera
  2 m above the ground, and one in a valley below the mean radius, cull nothing they can see; a
  camera below the occluder's radius disables the test; exact tangency is visible; root and
  face-sized patches are handled; and a brute-force oracle, rays from the camera to 10⁴ points on
  each patch's surface tested against the occluder, finds no false cull over 1,000 random cameras.
- Acceptance: `pnpm --filter hyperion test -- view/terrain/cull view/terrain/bounds`.

**R05.T7.b The selection.** `selectPatches` and `screenSpaceErrorPx` (Design note 7), with the
restricted quadtree enforced within and across faces.

- Files: `view/terrain/select.ts`, `view/terrain/select.test.ts`.
- Tests: every selected patch meets τ or is at the finest level; no two neighbours differ by more
  than one level, across face edges and at cube corners included; the result is identical for views
  and grounded bodies given in any order, called twice, or called after other calls (no module
  state); turning the camera about its own position changes the set only through culling; a
  smaller field of view refines; at the same pose τ = 2 px never selects a finer level than
  τ = 1 px; a camera inside a patch's bounds refines to the finest level.
- Acceptance: `pnpm --filter hyperion test -- view/terrain/select`.

**R05.T7.c The grounded-body rule.** `GroundContact`, the descending test of Design note 9, the
forced region and `morphHold`.

- Files: `view/terrain/grounded.ts`, `view/terrain/grounded.test.ts`, `view/terrain/select.ts`.
- Tests: on the high and the low setting, and at a view tolerance of 4 px (the wireframe's, which
  R10 uses), the selection contains the finest-level patches under every grounded body in view,
  with the ramp's margin; `morphHold` is 0 within r_g, 1 beyond the ramp, and continuous; two
  patches sharing an edge compute the same morph factor at every shared vertex near a grounded
  body; the morph is 1 wherever a finest patch meets a coarser one.
- Acceptance: `pnpm --filter hyperion test -- view/terrain/grounded view/terrain/select`.

**R05.T7.d Several views.** The union of the views' selections, per-request priorities (Design
note 24) and the `demand` list.

- Files: `view/terrain/select.ts`, `view/terrain/priority.ts`, their tests.
- Tests: two views at one pose request each patch once; a secondary view's requests rank below the
  primary's at equal error; forced-region patches outrank all; the demand list is ordered by
  priority with ties broken by `patchKeyString`.
- Acceptance: `pnpm --filter hyperion test -- view/terrain`.

### R05.T8 The patch cache

`PatchCache` over fixed slots (Design note 10), taking a `SlotLayout` (slot count, bytes per slot
and per-slot fields) that R10 later supplies and this task defines with the heights, morph targets
and normals of this plan and an unused horizon-map field; and `resolveDrawSet`: the nearest
resident ancestor stands in for a patch not yet baked, and the draw set marks the stand-ins and
lists each drawn patch's slot index for the instanced draw.

- Files: `view/terrain/cache.ts`, `view/terrain/slotLayout.ts`, their tests.
- Tests: the slot count follows from the budget and the layout (64 MiB and the low layout give the
  expected count); no more slots are ever used than exist; a forced-region patch survives any
  eviction pressure; drawn patches are evicted only after unpinned ones; least recently used goes
  first; pins that exceed the slots are reported and fall back to stand-ins outside the forced
  region; an ancestor stands in for a missing patch and is marked; with nothing resident but the
  six roots, the draw set is the roots; a freed slot is reused.
- Acceptance: `pnpm --filter hyperion test -- view/terrain/cache`.

### R05.T9 The annunciations

`terrainAnnunciation` and its debounce (Design note 23), shown in R02's label block.

- Files: `view/terrain/annunciation.ts`, its test, R02's label-block component and its test.
- Tests: a stand-in in the draw set gives `TERRAIN: STREAMING` after 250 ms, which clears 1 s after
  the last stand-in goes; the low setting with terrain in view gives `TERRAIN: DETAIL LIMITED`; the
  high setting with everything resident gives neither; both at once show `STREAMING`; the label
  block renders the text in `--text` with no status colour; a condition toggling every frame never
  makes the line flicker (fake timers).
- Acceptance: `pnpm --filter hyperion test -- view/terrain/annunciation`; the `console-ux` skill's
  lint and contrast scripts pass on the changed files.

### R05.T10 The height-worker pool

**R05.T10.a The pool.** `HeightWorkerPool` over a `WorkerLike` interface (`postMessage`,
`onmessage`, `terminate`) given by a factory, per Design note 11: one priority queue on the render
thread, at most two requests in flight a worker, re-scoring every frame, cancellation by removal,
generation tags, results handed to the cache, and the field posted to one worker at a time.

- Files: `view/terrain/workers/pool.ts`, `view/terrain/workers/messages.ts`, their tests with a
  scripted fake worker.
- Tests: requests reach idle workers in priority order; a cancelled request never reaches a worker;
  a result for a stale generation whose key is still wanted is cached, and one no longer wanted is
  dropped; no worker ever has more than two in flight; a field is posted to the second worker only
  after the first acknowledges; a worker that errors is replaced and its requests re-queued;
  `terminate` on unmount leaves no listener behind.
- Acceptance: `pnpm --filter hyperion test -- view/terrain/workers`.

**R05.T10.b The worker and its WebAssembly.** `heightWorker.ts`, extending R04's loader and probe
worker in `renderer/src/wasm/`: it loads the module `just gen-surface` produces (wasm-bindgen's web
target) through `init` on a `?url` import, bakes, and copies and transfers the typed arrays. The
renderer's Vite configuration gains `worker: { format: "es" }`. A Node-environment vitest loads the
same built `.wasm` from disk with `initSync` and checks three bakes against the native golden's
hashes (T5). In the built app, started through `loadFile`, a worker bakes one patch and logs the
module's content type and any policy violation, extending R04's own check of its probe worker;
T13.b's `--smoke` mode later makes this a command that exits with a status. If module workers do
not load from `file://`, this task serves the renderer from a privileged custom scheme through
`protocol.handle` instead, with R01's and the owner's agreement, as Design note 11's fallback.

- Files: `view/terrain/workers/heightWorker.ts`, `renderer/src/wasm/` (R04's loader, extended),
  `apps/hyperion/electron.vite.config.mts`, `view/terrain/workers/wasm.node.test.ts`.
- Tests: the Node-environment wasm test; the built-app check.
- Acceptance: `just gen-surface && pnpm --filter hyperion test -- view/terrain/workers`; by hand,
  recorded in the task's notes: the built app (`pnpm --filter hyperion build`, run from `out/`)
  bakes a patch in a worker with no policy violation, after the owner's ruling on the policy.

### R05.T11 The terrain pass

**R05.T11.a GPU resources.** Through R01's adapter: one shared index buffer for the 65 × 65 grid
on the fixed diagonal, with skirts; the slot layout's storage buffers (heights and morph targets,
and for `BakedOffsets` the offsets) and the normals' `rg16float` texture array, for either normal
scale, written slot by slot; a per-frame uniform per patch holding the `f32` narrowing of the `f64`
camera-relative patch origin (R02's differencing, after rotating the body-fixed origin into the
body frame), its morph range and the grounded contacts for `morphHold`; the upload tally (bytes a
frame through `writeBuffer` and `writeTexture`) and the allocation tally the metrics read.

- Files: `view/terrain/gpu/resources.ts`, `view/terrain/gpu/uniforms.ts`, their tests against R01's
  adapter fake.
- Tests: the index buffer's triangles use the (0, 0)–(1, 1) diagonal; the uniform's origin is the
  `f64` difference narrowed once; a freed slot is overwritten in place, with no new allocation after
  warm-up; the byte tally matches the formats' sizes.
- Acceptance: `pnpm --filter hyperion test -- view/terrain/gpu`.

**R05.T11.b The shaders.** `terrain.wgsl`: the vertex stage for both paths (Design note 4), the
morph and its hold (Design note 6), reversed-Z output (R02); the fragment stage's Lambertian shading
of Design note 17 from the decoded normal texture, writing pre-exposed `rgba16float`. A TypeScript
emulation of the `FaceDifferences` vertex arithmetic in `Math.fround`, operation for operation as
the WGSL has it, is checked against T4.b's Rust `f32` reference on shared inputs from T2's golden.

- Files: `view/terrain/shaders/terrain.wgsl`, `view/terrain/gpu/vertexEmulation.ts`, its test,
  R01's smoke-harness list of passes.
- Tests: the emulation agrees with the Rust `f32` reference bit for bit; R01's offline render test
  compiles the pass with the network disabled; R01's SwiftShader smoke test renders a frame of the
  test planet from 400 km and from 10 m with every texel finite, without `shader-f16` and without
  `subgroups`.
- Acceptance: `pnpm --filter hyperion test -- view/terrain`; `just test-render` passes with the
  terrain pass registered in `WGSL_CATALOGUE`.

**R05.T11.c The pass in a view.** The terrain pass driven each frame by selection, the cache, the
pool and the draw set, submitted inside R02's `VIEW` as the spike scene of Design note 17, with
R02's tone curve as one full-screen pass and a `MAN` exposure.

- Files: `view/terrain/terrainPass.ts`, `view/spike/litView.ts`, their tests.
- Tests: with a fake adapter, a frame submits one instanced draw whose instances are the drawn
  patches' slot indices, none for culled ones;
  stand-ins are drawn with their own morph range; the per-frame work allocates no new objects after
  warm-up (a counting fake); the exposure is shown as `MAN` with its EV100 in the label block.
- Acceptance: `pnpm --filter hyperion test -- view/terrain view/spike`; by hand, recorded in the
  task's notes: the test planet from 400 km and from 2 m on the development machine, the right way
  up, with no crack visible across a cube-face edge.

### R05.T12 Earth's atmosphere

**R05.T12.a The medium.** `MediumTerm`, `AtmosphereMedium` and `EARTH_REFERENCE` (Design note 16),
each constant with its citation re-checked: the three Rayleigh coefficients (Peck and Reeder 1972,
Bates 1984, the US Standard Atmosphere 1976 for N), the aerosol's optical depth, Ångström exponent,
single-scattering albedo, asymmetry and scale height (AERONET and MODIS climatologies; Bruneton
2008 for g = 0.76), the ozone column and cross-sections (Serdyuchenko et al. 2014, as Bruneton
2017 bins them), the top of the atmosphere and the ground albedo; Hillaire's reference medium as a
second constant for comparison; and the Sun's per-channel top-of-atmosphere illuminance and the
spectral-radiance-to-luminance factors (Bruneton 2017's method), computed by a script from the
ASTM E-490 solar spectrum and the CIE 1931 matching functions and committed as constants with a
header naming the script, its inputs and their sources.

- Files: `view/atmosphere/medium.ts`, `view/atmosphere/earth.ts`, `view/atmosphere/solar.ts`,
  their tests, and `apps/hyperion/scripts/solar-factors.ts`, run by Node's type stripping, with its
  input tables committed beside it under their sources and licences.
- Tests: the Rayleigh coefficients recomputed in the test from the refractivity and King-factor
  formulas at the three wavelengths agree with the constants to 0.5%, and the 550 nm cross-section
  with Bucholtz 1995's 4.51 × 10⁻²⁷ cm² to 1%; the ozone coefficients recompute from 300 DU and the
  cross-sections; the aerosol's optical depth integrates back to 0.1 at 550 nm; the Sun's
  per-channel illuminance has the luminance R02's photometry gives for V = −26.76.
- Acceptance: `pnpm --filter hyperion test -- view/atmosphere`.

**R05.T12.b The per-planet tables.** Transmittance and multiple scattering as compute passes
through R01's adapter, ported from Bevy 0.19's WGSL with its licence notice and checked against
sebh's reference code, rebuilt when the medium changes (not when the sun moves), at the sizes of
Design note 16. An `f64` TypeScript integrator of optical depth along a ray is the oracle.

- Files: `view/atmosphere/shaders/transmittance.wgsl`, `multiScattering.wgsl`,
  `view/atmosphere/tables.ts`, `view/atmosphere/opticalDepth.ts`, their tests.
- Tests: the oracle against closed forms for an exponential atmosphere at the zenith and the
  horizon; in R01's SwiftShader smoke harness, the transmittance table read back with
  `copyTextureToBuffer` agrees with the oracle to 1% at 20 fixed (altitude, angle) texels, and every
  texel of both tables is finite and within [0, 1] for transmittance.
- Acceptance: `pnpm --filter hyperion test -- view/atmosphere`; `just test-render` passes with the
  new property assertions.

**R05.T12.c The per-frame tables and the draw.** Sky-view and aerial perspective every frame, the
per-pixel ray march from above the atmosphere and for terrain beyond the aerial-perspective reach,
the deferred aerial-perspective pass over terrain that reads the reversed-Z depth, and the sky
drawn where depth is at the far plane; the high and low sizes of Design note 16; absolute luminance
from the per-channel solar illuminance, and the sun's disc clamped after pre-exposure.

- Files: `view/atmosphere/shaders/skyView.wgsl`, `aerialPerspective.wgsl`, `rayMarch.wgsl`,
  `composite.wgsl`, `view/atmosphere/hillaire.ts`, their tests.
- Tests: with a fake adapter, the per-planet tables are not rebuilt when only the sun moves and are
  when the medium changes; the low setting allocates the low sizes and applies aerial perspective to
  terrain alone; the smoke harness renders a frame from 400 km and from 2 m at noon and at the
  terminator with every texel finite and none above `rgba16float`'s maximum; the offline render
  test passes.
- Acceptance: `pnpm --filter hyperion test -- view/atmosphere`; `just test-render` passes; by hand,
  recorded: the comparison mode beside the published images of Hillaire 2020 from the ground at noon
  and sunset and from orbit, looked at by a person, as the brainstorm keeps image comparison.

### R05.T13 The spike

**R05.T13.a The scripted descent.** `descentProfile.ts`, the path of Design note 19 as a pure
function of script time in body-fixed coordinates over a landing site and azimuth drawn from the
spike's seed, with its segments as data; the test planet's rotation of Design note 14 (the IERS
sidereal day re-checked); and `demand.ts`, the prediction of Design note 19 from T6's per-level
bounds.

- Files: `view/spike/descentProfile.ts`, `view/spike/rotation.ts`, `view/spike/demand.ts`, their
  tests.
- Tests: position and velocity are continuous across every segment boundary; the path at a fixed
  seed is identical call to call, and two seeds give two sites; a hovering pose's body-fixed
  position is constant while its body-frame position moves at ω × r; the predicted demand
  reproduces the brainstorm's worked figures to 20% (about 4 a second in low orbit, 13 at 100 m/s
  and 1.5 km, 200 at 300 m/s and 300 m, 29 descending at 20 m/s through 200 m); the fixed-step
  run of the whole descent through `selectPatches` and a simulated cache gives a selection
  sequence whose hash is pinned, and its measured demand, first-time-selected keys a second, lies
  within a factor of two of the prediction in every segment on both settings.
- Acceptance: `pnpm --filter hyperion test -- view/spike`.

**R05.T13.b The spike scene and its runner.** `spikeScene.ts` (the test planet, its rotation, the
Sun-like light 1 au away with R02's photometry, the scripted craft as a `GroundContact` once it
is descending), `DescentSpike.tsx` (the full-window view, two small wireframe instrument canvases of
R02's style drawing the planet's graticule, the craft's hull and the orbit, and one console panel,
all on one device through R01's per-view contexts, with each canvas's DOM list), the client flag
`--descent-spike [--setting low|high] [--seed …] [--smoke]` in `cli.ts`, and a
`just descent-spike` recipe that builds the client, starts a local server with `--num-workers 2`,
and runs the flag. The preload gains narrow functions for the spike only, present when the flag is
given, each validated in its handler: start and stop the trace, sample memory and write the
results file (Design note 18).

- Files: `view/spike/spikeScene.ts`, `view/spike/DescentSpike.tsx`, its test,
  `apps/hyperion/src/main/cli.ts`, its test, `apps/hyperion/src/main/spike.ts`,
  `apps/hyperion/src/preload/api.ts`, `apps/hyperion/src/preload/index.ts`, `justfile`.
- Tests: the CLI parses the flag and its options and refuses a bad setting; the spike's IPC
  handlers refuse a sender that is not the window's own frame and arguments that do not validate;
  `DescentSpike` renders its three canvases, each focusable, named and paired with its list, and
  its label block states the test planet as provisional, dry and hand-parameterised.
- Acceptance: `pnpm --filter hyperion test`; `just descent-spike --smoke` exits 0; by hand,
  recorded: one full descent on the development machine at the low setting, orbit to 1 m in one
  motion.

### R05.T14 The metrics harness

**R05.T14.a In the renderer.** `metrics.ts`: frame intervals by `requestAnimationFrame` and
presentation time, per-pass GPU time from `timestampWrites` where `timestamp-query` is present,
`performance.measure` spans around our per-frame code, the upload and allocation tallies of T11.a,
a shim over `createRenderPipeline`, `createComputePipeline` and their asynchronous forms that
counts and labels creations after warm-up, and patches requested, baked and made resident a
second, each tagged with the script time and segment.

- Files: `view/spike/metrics.ts`, `view/spike/percentiles.ts`, `view/spike/pipelineShim.ts`,
  their tests.
- Tests: the percentiles of fixed interval lists against hand-computed values, including the
  missed-frame and hitch counts of Design note 21; the shim counts a creation after warm-up and
  passes the call through unchanged; per-segment figures do not mix segments.
- Acceptance: `pnpm --filter hyperion test -- view/spike`.

**R05.T14.b In the main process.** `spike.ts`: the measurement switches of Design note 18 set before
`ready` when the flag is given (R01's `gpuTiming`, and the safety-toggle set of Design note 22
merged with `mergeSwitchValue` when `--dawn-safety off` is given); `contentTracing` over the
descent; `app.getAppMetrics()` and the renderer's memory at 1 Hz; a reader of the GPU process's DRM
fdinfo; `nvidia-smi` where present; and the reducer, which parses the trace's JSON events in the
main process and writes the results file of Design note 18 with the machine, driver, Electron and
Chromium versions, setting, seed, switches and every figure, and a Markdown summary beside it.

- Files: `apps/hyperion/src/main/spike.ts`, `apps/hyperion/src/main/fdinfo.ts`,
  `apps/hyperion/src/main/reduceTrace.ts`, their tests, `docs/measurements/descent-spike/README.md`.
- Tests: the fdinfo parser against recorded fixtures from i915 and amdgpu, summing distinct
  client IDs; the reducer against a small recorded trace, finding its GC slices per thread and its
  user-timing spans; the switch set is exactly the documented one for each option and never contains
  `--enable-unsafe-webgpu`; the results writer's output parses against its schema.
- Acceptance: `pnpm --filter hyperion test -- main/`; `just descent-spike --setting low` on the
  development machine writes a results file with every figure present.

### R05.T15 The capture and the native replay

**R05.T15.a The capture.** A measurement-only shim on `GPUDevice`, `GPUQueue` and the encoders,
after webgpu_recorder's interception design, that writes WGSL sources, descriptors, uploads and
each frame's commands over a fixed span of the descent as JSON with binary blobs, when
`--capture <dir>` is given.

- Files: `view/spike/capture.ts`, its test against a fake device.
- Tests: a scripted sequence of device calls round-trips through the capture's reader to the same
  calls and bytes; the shim is absent unless the flag is given.
- Acceptance: `pnpm --filter hyperion test -- view/spike/capture`.

**R05.T15.b The replayer.** `tools/gpu-replay/`, a Rust binary with its own `[workspace]`, on
wgpu 30 and winit: loads a capture, validates every module with naga and reports rejections,
recreates the objects, replays the frames with FIFO presentation at the captured resolution, and
writes the same frame-interval and per-pass figures as the results file. The root `Cargo.toml`
gains `exclude = ["tools/*"]`, and the justfile a `replay` recipe that `ci` does not call.

- Files: `tools/gpu-replay/Cargo.toml`, `tools/gpu-replay/src/*.rs`, `Cargo.toml`, `justfile`.
- Tests: in the tool, the capture reader against a small checked-in capture; the naga validation
  report lists a deliberately invalid module.
- Acceptance: `just replay <capture>` replays the development machine's capture on its UHD 620
  and writes a results file; `just ci` does not build wgpu (its build log names no `wgpu` crate).

### R05.T16 The UHD 620 runs

By hand on the development machine, recorded: the low setting at 720p paced to 30 fps with the two
instruments and the console open, the local server running with `--num-workers 2`, over three
seeds; each run once more with a companion load on two threads (Design note 20); cold and warm
pipeline caches; both vertex paths; ridged terms on and off; and Dawn's safety checks on and off.
One high-setting run at 1080p is recorded for comparison, not judged. Each writes its results file
and summary under `docs/measurements/descent-spike/`, and a capture of one low run is replayed by
`just replay` for the native comparison. If a run misses Design note 21's rows, the task records
which pass or thread misses and by how much, and the low setting is redesigned within this plan
before T18, in the order that costs the picture least: normal and table sizes, the ray march's
resolution, τ within the brainstorm's lean, the render resolution under 720p presented upscaled,
and last the instruments' rate. Each change is one more recorded run.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: the results files exist for every run listed, each with every figure of Design note 18
  and the machine's load average and governor, taken on a quiet machine (Design note 27); the
  summary states pass or fail against every row of Design note 21's table.

### R05.T17 The discrete runs

By hand on a borrowed desktop with an RTX 4060-class GPU and a 60 Hz monitor (Design note 21; a
rented cloud GPU such as an L4 or an RTX 4000 SFF Ada gives advisory GPU-time figures only, since
its virtual display has no real vertical blank): the high setting at 1080p60 over the same three
seeds, with the same two instruments and console, the local server capped as in T16, safety
checks on and off, and one capture replayed natively on the same machine by `just replay`. The
machine, driver and display are recorded with the results.

- Files: `docs/measurements/descent-spike/*.json` and `*.md`.
- Acceptance: as T16, on the discrete machine, with the replay's results file beside the
  browser's for the same span.

### R05.T18 Defaults from the measurements

The choices the plan left to measurement, made from T16's and T17's results and recorded with the
figures that decided them: the vertex path for the high setting (Design note 4; the low setting
keeps `FaceDifferences`), the high setting's normal scale (Design note 25), the descending
thresholds (Design note 9), the cache budgets (Design note 10, until R10), the worker counts (Design
note 11) and, if T16 redesigned it, the low setting. The code's defaults change in one commit; the
plan's Design notes gain "as built" lines.

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
rule fires, the owner is told before any later plan depends on the browser.

- Files: this plan, `docs/measurements/descent-spike/README.md`.
- Acceptance: the verdict names each criterion of Design note 21 with its measured value on each
  machine; the owner has the drafted brainstorm edit.

## Verification

- **Determinism:** the cube-sphere, test-planet and bake goldens pass on native,
  `wasm32-wasip1` and `wasm32-unknown-unknown` in `just ci` (T2, T3.c, T5); the TypeScript mirror
  matches the Rust bits (T2); the bake is independent of its cache's fill order (T3.a, T4.a).
- **Geometry:** shared edges are bitwise equal across faces (T1.b, T4.a); both vertex paths are
  under 1 mm from `f64` at level 19 and the naive form is not (T4.b); the collision interpolant is
  the drawn finest mesh (T4.c).
- **Level of detail:** the hard bound holds and is not loose by more than four times (T6);
  selection meets τ, is balanced, is a pure function of its inputs and keeps the finest level under
  every grounded body on every setting and tolerance (T7); the morph is continuous at shared
  vertices and 1 at every level transition (T7.c).
- **Culling:** the predicates' named cases and the brute-force oracle's zero false culls (T7.a).
- **Streaming:** measured demand within a factor of two of the prediction on the fixed-step descent
  (T13.a); patches a second sustained against demand in every recorded run (T16, T17).
- **Atmosphere:** the Rayleigh, ozone and aerosol figures recompute (T12.a); the transmittance table
  matches an `f64` oracle to 1% on SwiftShader (T12.b); the comparison against Hillaire's published
  images, by eye (T12.c).
- **The gate:** the recorded runs against every row of Design note 21 on both machines (T16, T17),
  the replay and toggle comparisons, and the verdict (T19).
- **By eye, recorded:** the descent from orbit to a metre above the ground in one motion, watching
  for pops, cracks between levels and at cube-face edges, and the moment `TERRAIN: STREAMING`
  appears (T13.b, T16).

## Generator version

No change to generated output and no bump. The test planet belongs to no universe, and nothing a
universe generates reads `hyperion-surface` yet. Its goldens carry `TEST_PLANET_VERSION`, which
starts at 1 and moves with any change to the test planet's heights or the bake's bytes. The plan
reserves the `SelfTest` tag `selftest.surface.test_planet` in the surface crate's part of the
registry, and nothing under a `surface.*` or `body.surface.*` name, which R09 registers. The
constants `FINEST_SPACING_M`, `BAND_LIMIT_M` and the rule of `finest_level` become part of the
generator version when R09's real height function reads them; R09 must bump `GENERATOR_VERSION`,
which R04 places in `hyperion-base` where the surface crate can see it, for any later change to
them. No wire type changes: the spike's IPC is the preload's, not the protocol's.

## Risks and open points

- **The discrete machine does not exist yet.** The project has no discrete GPU; T17 needs one
  borrowed, or bought used, with a real monitor. A cloud GPU's figures are advisory and cannot sign
  off the gate (researched 2026-09-29: virtual displays have no real vertical blank, and the
  compositor path and driver branch may differ). Until T17 runs, the gate is half closed: the UHD
  620 half answers whether the low setting works, and nothing about the browser in general.
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
    no rebuild when the sun moves; the 32 km aerial-perspective reach is Bevy's figure, where sebh's
    reaches 128 km; Hillaire's reference aerosol is 20–40 times cleaner than Earth's typical sky, so
    "Earth's reference atmosphere" is taken as Earth's measured aerosol, with Hillaire's as a
    comparison mode.
  - Horizon maps are sun-independent (R10's research), so the budget's "it needs rebaking only as
    the sun moves" is wrong: they are baked once with each patch.
  - The low setting's patch counts are about a quarter of the high setting's, not a ninth, because
    of the quadtree's per-ring floor (R10's research); demand's "a ninth" holds only while k exceeds
    about 3.
  - Normals at twice the mesh's resolution quadruple the gradients the bake budget counts (Design
    note 25).
  - The worker's policy (Design note 11): a `file://` worker has no policy of its own, so
    `'wasm-unsafe-eval'` is probably needed only for compilation on the render thread; the change
    stays the owner's either way.
- **Timings measured so far are provisional.** The development machine is shared with other
  agents' tests; every figure in this plan measured today, including the research agents' Threefry
  timing and the estimates built on it, is re-measured on a quiet machine (Design note 27) before it
  decides anything.
- **The cache layout is R10's, built here.** R05 builds fixed slots, storage buffers and the
  instanced draw to R10's specification (Design note 10) so that nothing is rebuilt; if R10's
  plan changes the layout before T8 runs, T8 follows R10. `BakedOffsets` is barred from the low
  setting by its 64 MiB budget, whatever T18 finds.
- **Module workers from `file://`** are undocumented in Electron 44 (researched 2026-09-29, medium
  confidence). T10.b's smoke run settles it; the fallback, a privileged custom scheme, touches R01's
  main process and the policy's origin and needs the owner.
- **The test planet is not the real function.** Its cost per point, its spectrum and its bound
  stand in for R09's, whose Dendry channels and crater octaves the brainstorm estimates at 1 to 3 µs
  a point on their own. A spike that passes on the test planet passes for a function of the test
  planet's cost; T3.c's measured cost and the headroom row are what R09 must stay within, and R09's
  own benchmarks re-run the descent (its scripted path and harness are kept for that). The
  researched σ_h, the break at about 2.4 km and the noise basis's certified bound are estimates
  until T3 measures them.
- **The hard bound may be loose.** If T6 finds it looser than four times the 99.9th percentile,
  selection over-refines and demand rises; the calibration factor of Design note 15 is the answer,
  recorded, and the contract stays the bound.
- **The descending thresholds** (Design note 9) are provisional: 1 km and 30 s until T16 measures
  how long a forced region takes to become resident.
- **The pass criterion's reserve** (Design note 21) counts only terrain and atmosphere against
  their rows' upper ends. If later plans' passes land above their own rows, the spike's pass does
  not carry over; R12's consolidated runs are where that shows.
- **Names from R01, R02 and R04** were written in parallel with those plans and are re-validated
  before T1. The asks in particular: that R04's loader and probe worker, and its `just gen-surface`
  output, suit module workers under `file://` and a Node-environment test (`initSync` on the
  module's bytes); and that R01's engine interface carries per-draw textures and compute kernels
  writing storage textures, with their sizes visible to the allocation tally.
- **Measurement switches in a shipped binary.** The timestamp and Dawn-toggle switches are set only
  when the spike flag is given; T14.b's test holds that the ordinary launch sets exactly R01's
  switches.
- **Asked by later plans, not yet designed here** (the roadmap's asks table carries each). R07: a
  `PlanetGeometry` of a reference spheroid (equatorial and polar radii and a pole), drawn at zero
  height with no height worker, for its `mesh` regime (R07 Design notes 3 and 19). R10:
  `bake_patch`, `finest_surface_height` and the bound's use in selection taking their height source
  as a trait or an argument rather than `&TestPlanet`, so that R10 points them at R09's
  `Synthesiser` (R10 Design note 4). R08 changes this plan's atmosphere code in its own tasks:
  transmittance stored as optical depth, the ozone term through a curve of growth, and the channel
  wavelengths refitted (R08 Design notes 5 and 8).

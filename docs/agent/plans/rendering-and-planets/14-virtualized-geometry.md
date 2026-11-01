# Plan R14: Virtualized Geometry

- **Milestone:** Rendering milestone RM8 (new, 2026-11-01, the owner's priority: faster terrain
  viewing and streaming, by Nanite's techniques where they fit, as one system for every kind of 3D
  model, terrain first, Apple Silicon first). It runs beside RM6.
- **Depends on:** as built, [R01](01-graphics-platform-and-engine.md) (the engine's compute,
  indirect draws, kernel readback classes, memory categories, pass timer and the SwiftShader smoke
  harness), [R05](05-terrain-geometry-and-descent-spike.md) (the quadtree, pure selection, the patch
  cache and its resolver, the patch mesh, the culling predicates, the forced region) and
  [R10](10-terrain-on-generated-worlds.md) (the lit and wireframe terrain, the survey edge, the
  cascades and their casters, the overlay's terrain depth, the terrain run). It reads
  [R02](02-real-scale-view-and-wireframe.md) (the hulls' occluder faces, the wireframe),
  [R07](07-lit-bodies-styles-and-main-screen.md) (the smooth figure, the internal scale) and
  [R08](08-atmospheres.md) (the aerial-perspective composite). T5's consumers are
  [R11](11-surface-detail-clouds-oceans-rings-stills.md)'s rocks, scatter and ring particles and the
  craft art no plan builds yet. [R12](12-measurements-and-settings-ladder.md) holds this plan's
  ladder rows. Five lanes run beside it ([below](#running-beside-this-plan)).
- **Brainstorm sections covered** (by heading, in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)): "Virtualized geometry",
  in full (added with this plan); of "The geometry: a quadtree on a cube sphere", the mesh-shader
  row's verdict, which stands (no compute rasteriser); of "Level-of-detail consistency, and why
  collision agrees", the collision rule as the cluster path keeps it; of "Two styles of one
  renderer", both styles' terrain; of "Rings", the particles close to; of "Performance budget", the
  terrain geometry, shadows, scatter and rings rows, the memory table, the ladder's terrain detail
  and shadows rows and its three rules; of "Testing", "Level-of-detail selection", "The
  grounded-body rule", "Culling", "Collision agrees with what is drawn", "The descent" and "The
  performance runs".

## Goal

When this plan is done, every 3D model the view draws goes through one GPU-driven path in
`view/clusters/`. A consumer hands it pages of fixed-size clusters and a list of instances; a
compute pass culls each (instance, cluster) against every pass of the frame (the camera, four
cascades, the overlay's depth), picks its level of detail by a projected error against that pass's
own tolerance, and appends it to an indirect bucket that one `drawIndexedIndirect` draws. The lit
terrain is rasterised once into a visibility buffer and shaded once a pixel. Terrain drawn at a
stride its measured error allows, culled by a depth pyramid, submits a fraction of today's 21–32 M
triangles a frame on high. The smooth figure, the hull occluders, and then authored, procedural and
instanced meshes plug into the same interfaces.

What does not move: the heights are the workers' bakes, bit for bit; selection stays a pure function
on the CPU; the forced region under every grounded body is drawn at the finest level and stride 1
with the contacts' morph hold as today, so the drawn ground there is `finest_surface_height`'s
interpolant; and no generated value changes. Each lever lands behind a setting with today's path
kept as the way back, and is accepted on counts the pod can take and on the owner's Mac and RTX 3080
runs at 1920 × 1080.

## Scope and non-goals

In scope:

- **The shared path**: the cluster record, the templates, the cull passes, the cull kernel and its
  TypeScript twin, the indirect buckets, the per-pass counters; then a stable-order compaction, the
  visibility buffer and its resolve, measured-error strides, two-pass occlusion, the residency of
  mesh pages and the producers that fill them.
- **Its consumers**, in this order:

  | Consumer                                                   | Today                                                     | Task  | LOD mode                                    | Producer and residency                                        | Collision                                                         |
  | ---------------------------------------------------------- | --------------------------------------------------------- | ----- | ------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------------- |
  | Lit terrain: camera pass, cascades' casters, overlay depth | One instanced draw of the 10,752-triangle lit mesh a pass | T1–T3 | Strided template                            | The height workers' bakes in the patch cache's slots          | The forced region at stride 1, its hold as today (Design note 10) |
  | Wireframe terrain (depth and contours at 4 px)             | The same, depth-only with contours                        | T4.c  | Strided template, stride 1 (Design note 13) | The wireframe's own cache (520 slots)                         | As the lit terrain's                                              |
  | Mesh bodies' smooth figure and reference surface           | R05's patches at zero height (`smoothMesh.ts`)            | T4.a  | Strided template                            | None: constant zero-height pages                              | None                                                              |
  | Hull occluder faces (wireframe; silhouette over the image) | A few triangles, pulled by `occluder.wgsl`                | T4.b  | DAG cut (one level)                         | Built at load                                                 | Never the render mesh                                             |
  | Ships, stations, surface props (authored)                  | Not built: `TEST_HULL` outlines only                      | T5    | DAG cut                                     | Offline builder; pages fetched and decoded into the page pool | Authored hulls, never the render mesh                             |
  | Rocks and debris (procedural)                              | Not built (R11.T3–T4 plan the rocks)                      | T5    | DAG cut                                     | Built from a seed in a worker; page pool                      | R11's analytic rocks (R11 Design note 6)                          |
  | Scatter, ring particles (instanced)                        | Not built (R11.T6.a, R11.T9.d)                            | T5.d  | Instance cull, a few templates by size      | GPU-placed instances (R11's)                                  | None (presentation)                                               |

- **Shadows on the path**: the terrain's casters (T1), the cascades' reuse (T6), mesh casters.
- **The ladder's entries** in `ViewSettings.geometry` on both settings, for R12 to hold, and the
  memory categories R12 itemises.
- **Measurement**: per-pass clusters, triangles and GPU time in the terrain run's and the descent
  spike's results, and the counting harness on the pod.

Non-goals:

- **Selection, the quadtree and the morph**: R05's and R10's. Selection stays on the CPU, pure, with
  its budget (T3.e proposes the budget's change and builds it only once ruled). Seamless streaming's
  fixes (collapses, the time morph, the look-ahead) are the seamless lanes' (R10's records); this
  plan consumes them.
- **The bakes**: what a patch holds and how fast it bakes is R10.T6.a's and R05.T18's
  (`perf-terrain-bakes`). T3.b adds one presentation-only output beside the bake, after that lane.
- **A software rasteriser, mesh or task shaders, 64-bit atomics, bindless resources**: WebGPU has
  none of them (Design note 6), and the brainstorm's verdict on a compute rasteriser stands.
- **Geometry clipmaps, concurrent binary trees, RTIN** for terrain: rejected, clipmaps by the
  brainstorm's table, the other two by the research, since longest-edge bisection and adaptive
  triangulation break the fixed diagonal that `finest_surface_height` interpolates on and add a
  second source of what is drawn beside selection. CBT stays an option for R11's oceans.
- **Clouds and the ocean's surface**: R11's; neither is a consumer here (Risks).
- **R05's spike shading** (the test planet's `spike` shading): it keeps today's path, so the gate's
  recorded runs stay comparable.
- **Craft art, props, debris models**: no plan makes them yet. T5 builds the path they need and
  proves it on procedural test meshes.
- **The consolidated performance runs and the ladder's adjustment**: R12's. This plan records its
  own runs per task.

## Provides

Paths are under `apps/hyperion/src/renderer/src/view/` unless a path says otherwise. Signatures are
sketches, named to be grepped; each WGSL struct has its TypeScript packing and a layout test, and
each WGSL computation a TypeScript twin, as the repository's terrain shaders do.

### `clusters/record.ts`: the cluster record (T1.b; DAG fields from T4.b)

```ts
/** Bytes of one cluster's static record, written once when its page is filled. */
export const CLUSTER_RECORD_BYTES = 32;

/** How a record's error bytes are read (Design note 3). */
export type ClusterLodMode = "strided" | "dag";

export interface ClusterRecord {
  /**
   * Bounds' centre about the page's origin, m (`f32`), and radius, m: a sphere, which the GPU's
   * predicates test (Design note 4). If T1.b's lane measures the sphere's loss too large, the
   * record grows to 48 B with the box's two tangent half-extents and ν's extent (4.5 kB a slot).
   */
  readonly centreM: Vec3;
  readonly radiusM: number;
  /**
   * Eight bytes, read by the mode in `flags`. T1.b's terrain records hold (0, +∞) as two `f32`,
   * unused. A strided record marked measured (T3.b) holds four `f16` errors e₁, e₂, e₄, e₈, metres,
   * rounded up when packed. A DAG record holds (own, parent) as two `f32`, metres, drawn where
   * own ≤ tolerance < parent at their groups' LOD spheres (Design note 3).
   */
  readonly errors: readonly number[];
  /** Terrain: block origin x0, y0 in the 65-grid and its shape; a mesh: first triangle (24 bits) and count (8). */
  readonly template: number;
  /** Shape, mode, may-discard, material (bit layout in the doc comment). */
  readonly flags: number;
}
/** DAG mode only (T4.b): the parent group's LOD sphere, 16 B beside each record. */
export interface ClusterLod {
  readonly parentCentreM: Vec3;
  readonly parentRadiusM: number;
}
export function packClusterRecord(out: DataView, offsetBytes: number, r: ClusterRecord): void;
/**
 * A terrain patch's 96 records: 64 grid blocks, then 32 skirt-foot strips, edge by edge. Takes the
 * slot's upload, since `ResidentPatch` holds no `skirtDepthM`.
 */
export function terrainClusterRecords(
  planet: PlanetGeometry,
  patch: ResidentPatch,
  upload: SlotUpload,
  out: DataView,
): void;
```

```wgsl
struct ClusterRecord { centre : vec3f, radius : f32, errors : vec2u, template_ref : u32, flags : u32 }
```

### `clusters/templates.ts`: index data per cluster shape and stride (T1.b; generic from T4.b)

```ts
export type ClusterShape = "terrain-block" | "terrain-skirt" | "mesh";
export type Stride = 1 | 2 | 4 | 8;

export interface ClusterTemplate {
  readonly shape: ClusterShape;
  readonly stride: Stride;
  /** Triangles: block 128 / 32 / 8 / 2, skirt-foot strip 16 / 8 / 4 / 2 (Design note 2). */
  readonly triangles: number;
  /**
   * One `position` of three floats a vertex, (s·lx, s·ly, code), each triangle in the lit mesh's
   * winding and first-vertex order: the lit fragment reads `kind`, `lipM`, `slot` and `parent`
   * `flat` from a triangle's first vertex, so a grid triangle starts at a grid vertex and a foot
   * triangle at a foot vertex, as `litPatchMeshData`'s (p′, q′, p) and (q′, q, p) do.
   */
  readonly mesh: MeshSpec;
  /** The stride's error in the record's unit (T1: the level's largest spacing × s). */
  readonly errorScale: number;
}
export function terrainTemplates(): ReadonlyArray<ClusterTemplate>; // 8: 2 shapes × 4 strides
export function meshTemplate(): ClusterTemplate; // T4.b: 128 triangles by index pulling
```

### `clusters/cullPass.ts`: what a pass asks of the cull (T1.b, T1.c)

```ts
/** The most passes one dispatch culls for (a uniform array). */
export const MAX_CULL_PASSES = 8;

export type CullPass =
  | {
      readonly kind: "camera";
      readonly frustum: Frustum; // R05's planes, in the instance origins' frame (Design note 4)
      readonly horizon: HorizonCone;
      readonly tolerancePx: number; // τ′; the overlay's depth pass takes `strideCap` 1 (T1.e)
      readonly pxPerRad: number; // W_px ÷ (2 tan(fov_h ÷ 2)), as `screenSpaceErrorPx`
      readonly strideCap: Stride;
      readonly bucketBase: number;
    }
  | {
      readonly kind: "cascade";
      readonly cascade: CascadeIndex;
      readonly lightFromCamera: Float32Array; // the cascade's light-space box
      readonly receiverDepthM: readonly [near: number, far: number]; // T1.c
      readonly coveredBelow: Float32Array | null; // cascade k − 1's covered square (T1.c)
      readonly texelM: number; // 2 r ÷ 2,048 (T1.d)
      readonly strideCap: Stride;
      readonly bucketBase: number;
    };
export function packCullPasses(passes: ReadonlyArray<CullPass>, out: ArrayBuffer): void;
```

### `clusters/clusterCull.wgsl` and `clusters/cullTwin.ts` (T1.b; colour passes from T2.a)

- One dispatch a view-frame, a workgroup an instance, a thread a cluster (96 for terrain; at most
  `maxComputeInvocationsPerWorkgroup`, 256), testing every pass: the record's sphere against the
  frustum or the light box, the horizon cone, the receivers' depth interval (cascades), from T3.d
  the depth pyramid; then the coarsest template whose error is within the pass's tolerance at the
  cluster's distance, capped by the pass's `strideCap` and by Design note 10's forced-region rule;
  then an append of (instance, cluster) to the pass's bucket. Appends aggregate per workgroup: a
  workgroup counts its kept clusters in workgroup memory and makes one global `atomicAdd` per (pass,
  template), since one a cluster contends hardest on Apple's GPUs.
- `cullTwin.ts` computes the same decisions in `f64` TypeScript for the tests: its `cullCluster`
  takes a record, an instance and a pass and answers whether the cluster is kept, and at which
  template.
- A `KernelPair` (R01): the reference path only in T1 (`subgroup: null`), `presentation-only`, since
  its atomic appends' order is not reproducible (Design note 5).

### `clusters/buckets.ts`: the indirect buckets (T1.b)

```ts
export class ClusterBuckets {
  constructor(
    engine: RenderEngine,
    passes: number,
    templates: ReadonlyArray<ClusterTemplate>,
    capacity: number,
  );
  /** The lists, (instance, cluster) as two `u32` an entry, and the arguments, 20 B a bucket. */
  readonly lists: BufferHandle;
  readonly args: BufferHandle; // STORAGE | INDIRECT
  /** Zeroes every bucket's instance count; encoded before the cull. */
  clear(): void;
  /** One indirect draw a (pass, template), `firstInstance` 0, the bucket's base in `Draw` (Design note 6). */
  drawItems(pass: number, base: Omit<DrawItem, "mesh" | "indirect">): ReadonlyArray<DrawItem>;
  dispose(): void;
}
```

The vertex stage reads `(instance, cluster) = lists[bucketBase + instance_index]` and calls the
consumer's fetch: terrain's is `lit_place(vec3f(x0 + s·lx, y0 + s·ly, code), instance)`, unchanged,
with (x0, y0) derived from the cluster index (64 blocks, then 32 strips, edge by edge), so that the
vertex stage binds 7 storage buffers (the lit stage's 6 and the list) and reads no record. Its
`@builtin(position)` is `@invariant`, as every terrain vertex stage's is (Design note 5).

A bucket's capacity is `InstanceRecords`' capacity (the slot count) times the template's clusters an
instance, so no draw set, stand-ins included, can overflow it (Design notes 15, 16).

### `clusters/stats.ts`: the per-pass counters (T1.b)

```ts
export interface ClusterPassStats {
  readonly label: string; // the pass's timed label
  readonly clusters: number;
  readonly triangles: number;
  readonly gpuMs: number | null; // `timestamp-query`'s, else null
}
export class ClusterStats {
  /** Off in an ordinary frame: only the measurement switch arms it (Design note 18). */
  constructor(engine: RenderEngine, armed: boolean);
  /** Reduces the buckets' counts into a read-back ring by a `bit-exact` kernel. */
  sample(buckets: ClusterBuckets): void;
  latest(): ReadonlyArray<ClusterPassStats> | null;
}
```

Written into the terrain run's results (`src/renderer/src/displays/view/check/terrainCheckRun.ts`,
`src/main/terrainCheckResults.ts` and its markdown) and the descent spike's.

### `clusters/compact.wgsl`, `clusters/visibility.ts` and `clusters/resolve.ts` (T2)

```ts
/** A stable compaction: counts, an exclusive scan, a scatter, in the draw set's order (Design note 5). */
export const CLUSTER_COMPACT: KernelPair; // `bit-exact`: integer scans
/**
 * A view's visibility target: `r32uint`, drawn with the scene target's depth (T2.b's engine
 * target), at the scene's size.
 */
export class VisibilityBuffer {
  constructor(engine: RenderEngine, size: ViewSize);
  resize(size: ViewSize): void;
  /** (visible-cluster index << 7) | triangle; 0xffff_ffff none, reserved from the index space. */
  readonly ids: TextureHandle;
}
/** What a consumer gives the resolve: its WGSL decode and shading, and its bindings (≤ 8, Design note 6). */
export interface ResolveMaterial {
  readonly consumer: ClusterShape;
  readonly wgsl: string; // fn resolve_<consumer>(visible : u32, triangle : u32, pixel : vec2f) -> vec4f
  readonly storageBuffers: ReadonlyArray<StorageBufferSpec>;
}
```

### The engine's ID target (T2.b; an ask of R01)

The engine has no `r32uint` colour target, and a `RenderTarget` owns one colour attachment and its
own `depth32float` (`Drawing.encodeFrame` begins one pass with one colour attachment). The raster
writes an ID while it tests and writes the scene target's depth, so R01's contract grows:

```ts
// engine/types.ts
export type ColourTargetFormat = "rgba16float" | "rg11b10ufloat" | "rgba8unorm" | "r32uint";
export interface RenderTargetSpec {
  // … as built, and:
  /** Drawn with another target's depth, loaded and stored, in place of a depth of its own. */
  readonly depthOf?: RenderTarget;
}
```

- `r32uint` is a render attachment with no blend, no mips and no filtering; a material may bind it
  as a `texture_2d<u32>` read by `textureLoad`. `DepthSelfSample` covers the shared depth.
- The target's format is part of the pipeline key, so it reaches `prepareMaterial` and
  `terrain/materialSet.ts`.
- Files: `engine/types.ts`, `engine/memory.ts`,
  `engine/webgpu/{target,materials,drawing,resources}.ts`, `engine/catalogue.ts`;
  `materials.test.ts`, `drawing.test.ts` and `target.test.ts` test each form, and `WGSL_CATALOGUE`
  compiles them.

### `clusters/errors.ts`, `clusters/hzb.ts` (T3)

```ts
/**
 * e_s for s = 1, 2, 4, 8 of each of a bake's 96 clusters, metres, f64, monotone; the worker's
 * (T3.b). Formed from the drawn 3D offsets about the patch origin of both channels, own and morph
 * target (Design note 3).
 */
export function clusterErrors(
  bake: { heights: Float32Array; offsets: Float32Array | null; skirtDepthM: number },
  key: PatchKey,
  planet: PlanetGeometry,
): Float64Array;
/** This frame's depth pyramid of a view (reversed-Z: each texel the farthest it covers). */
export class DepthPyramid {
  constructor(engine: RenderEngine, size: ViewSize);
  build(depth: TextureHandle): void;
  readonly texture: TextureHandle; // r32float mips from half resolution
}
```

### `clusters/consumer.ts`, `clusters/pages.ts`, `clusters/residency.ts` (T4, T5)

```ts
/** What a consumer registers with the path. */
export interface ClusterConsumer {
  readonly shape: ClusterShape;
  readonly records: BufferHandle; // its pages' ClusterRecords
  readonly instances: BufferHandle; // its per-frame instances, origin less the camera first
  readonly instanceBytes: number; // never assumed: terrain's grows (seamless-2)
  readonly vertexWgsl: string; // fn cluster_place(instance : u32, cluster : u32, local : vec3f) -> ClusterPlace
  readonly templates: ReadonlyArray<ClusterTemplate>;
}

/** Fixed slots of mesh pages, LRU, with the cache's pins (Design note 14). */
export class GeometryPagePool {
  constructor(engine: RenderEngine, pageBytes: number, poolBytes: number);
  insert(page: MeshPage): { kind: "stored"; slot: number } | { kind: "refused" };
  retain(drawn: ReadonlySet<number>, forced: ReadonlySet<number>): void;
}

export interface ClusterResidency<K> {
  request(key: K, priority: number): void;
  answer(key: K): { kind: "resident"; slot: number } | { kind: "missing"; standIn: number | null };
}
```

- `MeshPage`: a versioned, little-endian page of at most 128 clusters' records, vertices (quantised
  positions about the page origin, octahedral normals, one UV set) and 8-bit local indices, with its
  group's LOD spheres; its format is in `clusters/pageFormat.ts` and, for the builder, in Rust.
- `tools/meshbuild/` (T5.a): a native Rust tool with its own workspace, as `tools/gpu-replay` has
  (the root workspace excludes `tools/*`), run as
  `cargo run --manifest-path tools/meshbuild/Cargo.toml -- <in> <out>`: clustering, grouping and
  simplifying with locked group borders into a DAG, writing pages. Pages are built from source
  meshes by a `just` recipe into the client's bundled assets, never committed (the 500 kB rule).

### Settings and memory (T1.b, extended by each task)

```ts
// quality/qualitySetting.ts, beside `terrain` (R12 Design note 3: a field of ViewSettings)
export interface GeometrySettings {
  /** Whether consumers draw through the cluster path; `false` is today's path, the way back. */
  readonly clusterPath: boolean;
  /** T1.c: casters culled by their receivers' view depths. */
  readonly casterDepthCull: boolean;
  /** T1.d: the coarsest stride a caster may take at its cascade's texel; 1 keeps stride 1. */
  readonly casterStrideCap: Stride;
  /** T1.e: the overlay's terrain depth. */
  readonly overlayDepth: "redraw" | "scene-depth" | "cluster";
  /** T2: a forward lit pass or the visibility buffer and its resolve. */
  readonly shading: "forward" | "visibility";
  /** T3: camera passes at stride 1, or at each cluster's measured error. */
  readonly strides: "off" | "measured";
  /** T3.d: two-pass occlusion against the depth pyramid. */
  readonly occlusion: "none" | "hzb";
  /** T5: the mesh page pool's bytes. */
  readonly pageBytes: number;
}
```

- `MemoryCategory` (`engine/memory.ts`) gains `geometry-clusters` (records, buckets, lists, the
  depth pyramid) and, in T5, `geometry-pages`; the visibility target is `render-targets`. R12's
  `MEMORY_ITEMS` (not built) groups them.

## Consumes

Re-validated on `hold/after-25` at `1fcd6c03` (2026-11-01): every name below was found at the file
and with the signature given. Paths are under `apps/hyperion/src/renderer/src/view/` unless given.

| Owner | Name                                                                                                                   | File                                                                                                                                   | As built                                                                                                                                                                                                                                                                                                                                          |
| ----- | ---------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R01   | `RenderEngine.dispatch`, `createComputeAsync`, `createMaterialAsync`, `readBuffer`, `onPassTimes`                      | `engine/types.ts`                                                                                                                      | `dispatch(kernel, bindings, workgroups: [x, y, z] \| IndirectArgs, pass?)`, each dispatch submitted before anything else is encoded (`webgpu/engine.ts`); `readBuffer` throws `PresentationOnlyReadback` for a buffer a `presentation-only` kernel wrote                                                                                          |
| R01   | `DrawItem.indirect`, `DrawItem.instanceCount`, `IndirectArgs`                                                          | `engine/types.ts`, `engine/webgpu/drawing.ts`                                                                                          | `indirect?: { buffer, offsetBytes }`, encoded as `drawIndexedIndirect` for an indexed mesh; a draw of `instanceCount` 0 without `indirect` is skipped                                                                                                                                                                                             |
| R01   | `KernelPair`, `selectKernel`                                                                                           | `engine/kernels.ts`                                                                                                                    | `{ name, reference, subgroup: string \| null, readback: "bit-exact" \| "presentation-only" }`; the subgroup variant only where the device has `subgroups`                                                                                                                                                                                         |
| R01   | `GpuCapabilities`, `WANTED_FEATURES`, `requiredLimits`, `MAX_REQUESTED_BUFFER_BYTES`                                   | `engine/platform.ts`                                                                                                                   | Features `subgroups`, `shader-f16`, `timestamp-query`, `float32-filterable`, `float32-blendable`, `rg11b10ufloat-renderable`, `depth-clip-control`; no `indirect-first-instance`, no `primitive-index`. Storage binding and buffer sizes raised to the adapter's, at most 1 GiB; `maxStorageBuffersPerShaderStage` never raised from 8            |
| R01   | `StorageBufferSpec.stages`                                                                                             | `engine/types.ts`                                                                                                                      | A binding counts against the per-stage limit in each stage it is visible to                                                                                                                                                                                                                                                                       |
| R01   | `MemoryCategory`, `BufferSpec`                                                                                         | `engine/memory.ts`                                                                                                                     | Nine categories, `height-cache` and `shadows` among them; `{ name, bytes, usage, category }`                                                                                                                                                                                                                                                      |
| R01   | `PassTimer`, `PASSES_PER_FRAME`                                                                                        | `engine/webgpu/timing.ts`                                                                                                              | 64 timed passes a frame                                                                                                                                                                                                                                                                                                                           |
| R01   | `ResilientEngine`, `watchDeviceLoss`, `WGSL_CATALOGUE`                                                                 | `engine/resilientEngine.ts`, `engine/webgpu/deviceLoss.ts`, `engine/catalogue.ts`                                                      | The rebuild after a loss; the catalogue test-render compiles and draws at every setting                                                                                                                                                                                                                                                           |
| R01   | Depth compare                                                                                                          | `engine/webgpu/materials.ts`                                                                                                           | `greater-equal` (reversed-Z): the later of two equal depths wins                                                                                                                                                                                                                                                                                  |
| R02   | `HullOutline`, `TEST_HULL`; `occluder.wgsl`                                                                            | `scene/hull.ts`, `shaders/occluder.wgsl`                                                                                               | `{ name, vertices, edges, faces, windows, eyePointM, lengthM }`; faces pulled from `corners` by `Draw.firstTriangle`, depth written from the fragment and pushed by `occluderSlopePx`                                                                                                                                                             |
| R05   | `selectPatches`, `screenSpaceErrorPx`, `selectionErrorM`, `MAX_SELECTION_VIEWS`                                        | `terrain/select.ts`                                                                                                                    | `selectPatches(input: SelectionInput): Selection`; `screenSpaceErrorPx(boundM, distanceM, view)` = ε W ÷ (2 d tan(fov_h ÷ 2)); 31 views                                                                                                                                                                                                           |
| R05   | `PatchCache`, `DrawSetResolver`, `DrawSet`, `ResidentPatch`, `CacheInsert`                                             | `terrain/cache.ts`                                                                                                                     | `insert(patch): { kind: "stored", slot, evicted } \| { kind: "refused" }`; `resolve(selection): DrawSet` (`count`, `patches`, `slots`, `views`); `{ key, generation, originM, heightRangeM, boundingRadiusM }`                                                                                                                                    |
| R05   | `patchBounds`, `PatchBounds`, `CameraRelativeBounds`                                                                   | `terrain/bounds.ts`                                                                                                                    | `patchBounds(planet, key, heightRangeM?)`, the oriented box and sphere R05 Design note 8 tests                                                                                                                                                                                                                                                    |
| R05   | `frustumOf`, `inFrustum`, `horizonCone`, `aboveHorizon`                                                                | `terrain/cull.ts`                                                                                                                      | `inFrustum(b: CameraRelativeBounds, f)`, `aboveHorizon(b, h)`: the CPU predicates the cull twins                                                                                                                                                                                                                                                  |
| R05   | `vertexSpacing`                                                                                                        | `terrain/cube.ts`                                                                                                                      | `vertexSpacing(radiusM, level): { minM, meanM, maxM }`                                                                                                                                                                                                                                                                                            |
| R05   | `INSTANCE_RECORD_BYTES`, `InstanceRecords`, `SLOT_RECORD_BYTES`                                                        | `terrain/gpu/uniforms.ts`                                                                                                              | 32 B (origin less camera, slot, morph band, parent slot, quadrant and holds); 112 B                                                                                                                                                                                                                                                               |
| R05   | `litPatchMeshData`, `LIT_PATCH_INDICES`, `INDIRECT_ARGS_BYTES`, `terrainMeshSpec`, `TerrainResources`                  | `terrain/gpu/resources.ts`                                                                                                             | Lit mesh 5,135 vertices, 32,256 indices: grid 8,192 triangles, skirt feet 512 (one winding), heads 512, fins 1,536 (both windings), each edge quad's foot then head; 20 B arguments; `field(name)`, `slotRecords`, `viewBuffers(id)`, `casterBuffers(id)`, `writeCasters`                                                                         |
| R05   | `QualitySetting`, `ViewSettings`, `TerrainSettings`, `SETTINGS`, `WIREFRAME_TERRAIN_TAU_PX`, `WIREFRAME_TERRAIN_SLOTS` | `quality/qualitySetting.ts`                                                                                                            | `"high" \| "low"`; high τ 1 px, 400,000,000 B cache (1,208 lit slots); low τ 2 px at 720 rows, 64 MiB (375); wireframe τ 4 px, 520 slots                                                                                                                                                                                                          |
| R05   | `GroundContact`, `FORCED_REGION_RESIDENCY_S`, `inForcedRegion`                                                         | `terrain/grounded.ts`                                                                                                                  | 30 s, provisional                                                                                                                                                                                                                                                                                                                                 |
| R05   | `finest_surface_height`; `PatchBake::height_range_m`                                                                   | `crates/hyperion-surface/src/patch/collision.rs`, `patch.rs`                                                                           | The collision interpolant on the mesh's own diagonal; the range covers both height channels, own and parent's                                                                                                                                                                                                                                     |
| R05   | `defaultHeightWorkers`                                                                                                 | `terrain/workers/pool.ts`                                                                                                              | clamp(⌊hardwareConcurrency ÷ 4⌋, 1, 3)                                                                                                                                                                                                                                                                                                            |
| R07   | `smoothMeshOf`, `SmoothMesh`, `SmoothPatch`, `MAX_SMOOTH_MESH_PATCHES`, `SMOOTH_MESH_TAU_PX`; `smoothMesh.wgsl`        | `bodies/smoothMesh.ts`                                                                                                                 | R05's patches at zero height, selected by `selectPatches` with no level table; 1,024 patches; 0.25 px                                                                                                                                                                                                                                             |
| R07   | `ViewSettings.internalScaleBounds`                                                                                     | `quality/qualitySetting.ts`                                                                                                            | [0.5, 1] on both settings: the scene target resizes with it                                                                                                                                                                                                                                                                                       |
| R08   | `AtmosphereScene`; the composite                                                                                       | `atmosphere/AtmosphereCache.ts`, `atmosphere/shaders/composite.wgsl`                                                                   | The composite reads the scene target's colour and reversed-Z depth and lays aerial perspective over every opaque pixel                                                                                                                                                                                                                            |
| R08   | `aerialPerspectiveScope`, `HillaireAtmosphere.aerialPerspective`                                                       | `atmosphere/hillaire.ts`, `atmosphere/settings.ts`                                                                                     | `scene` (high): every opaque pass drawn into the scene before the composite; `terrain` (low): the terrain alone                                                                                                                                                                                                                                   |
| R10   | `TerrainPass` (`#write`, `#drawCasters`, `#casterItems`, `#drawItem`, `maxPatches`, `symbologyDepthMaterial`)          | `terrain/terrainPass.ts`                                                                                                               | One instanced indirect lit draw a view; casters as four `instanceCount` draws of the lit mesh from per-cascade copies of the instance records; `maxPatches` half the slots                                                                                                                                                                        |
| R10   | `castsInto`, `morphRangeM`, `effectiveTauPx`, `MORPH_START_FRACTION`                                                   | `terrain/terrainPass.ts`                                                                                                               | `castsInto(set, index, fromCameraBodyFixedM, radiusM): boolean`; `effectiveTauPx(tauPx, limitExcess, weight)` = τ max(1, excess ÷ w); 0.7                                                                                                                                                                                                         |
| R10   | `CascadeSet`, `fitCascades`, `CASCADE_SIZE_PX`, `CASCADE_COUNT`                                                        | `terrain/shadows/cascades.ts`                                                                                                          | `{ rangeM, splitsM, weight, unclippedDepth, casterReachM, sunElevationRad, basis, cascades }`; 2,048; 4                                                                                                                                                                                                                                           |
| R10   | `cascadeAt`, `CASCADE_FOOTPRINT_TEXELS`, `cascadeReceiverLit`                                                          | `terrain/shadows/cascadeShadow.ts`                                                                                                     | `cascadeAt(distanceAlongViewM, farM): CascadeIndex \| null`; the receiver's twin                                                                                                                                                                                                                                                                  |
| R10   | `ShadowCasters`, `ShadowCasterProvider`, `TerrainShadows`                                                              | `terrain/shadows/terrainShadows.ts`                                                                                                    | `(set, index, view) => DrawItem[]`: the casters that are not terrain                                                                                                                                                                                                                                                                              |
| R10   | `lit_place`, `survey_kept_at`; `LitVertexOut`; the lit fragment                                                        | `terrain/shaders/terrainSurvey.wgsl`, `terrainLit.wgsl`, `terrainLitShade.wgsl`                                                        | `lit_place(grid : vec3f, instance : u32) -> LitPlace`; 12 inter-stage locations (92 B a vertex with the position); the geometric normal from `dpdxFine` before the survey `discard`; tiles read by `textureLoad`, cascades by `textureSampleCompareLevel`                                                                                         |
| R10   | `TerrainLayers.unsurveyedCells`                                                                                        | `terrain/workers/messages.ts`                                                                                                          | `Uint32Array`, empty on a wholly surveyed bake                                                                                                                                                                                                                                                                                                    |
| R10   | `LiveFrameTerrain.symbologyDepth`; `TERRAIN_DEPTH_FRACTION`                                                            | `terrain/frameTerrain.ts`; `depth/groundPull.ts`, `terrain/shaders/terrainDepth.wgsl`                                                  | `symbologyDepth(): ReadonlyArray<DrawItem>`, the terrain redrawn as depth before the marks, each depth pushed by 2⁻¹⁶ of itself with no slope term (R10.T16.c); the survey edge pulls its own depth (`shaders/lineStroke.wgsl`'s `line_depth`); only the hull occluders keep `occluderSlopePx` (`photoreal/overlay.ts`, `terrain/terrainPass.ts`) |
| R10   | `CoarseFineHolds`, `coarseFineMorph`                                                                                   | `terrain/coarseFineHolds.ts`                                                                                                           | k held at 0 or 1 on coarse–fine edges and corners, from the draw set                                                                                                                                                                                                                                                                              |
| R10   | The terrain run; the smoke harness's terrain frame                                                                     | `src/renderer/src/displays/view/check/terrainCheckRun.ts`, `src/main/terrainCheckResults.ts`, `src/renderer/src/smoke/terrainFrame.ts` | `just views-check --terrain [--whole] [--hidden] [--software] [--setting high\|low]`; test-render's `R10.T16 the frame` group (part 7)                                                                                                                                                                                                            |
| R10   | `levelBoundsCounts.ts`                                                                                                 | `terrain/levelBoundsCounts.ts`                                                                                                         | The count model the research's harness was built on                                                                                                                                                                                                                                                                                               |
| R11   | `RockBakeHook`, `rockMesh.ts`, `couldTouch`; `pebbles.wgsl`, `scatterCull.ts`; `ringParticles.wgsl`, `instancedShare`  | planned (R11.T4, T6, T9.d), not built                                                                                                  | T5's consumers; names as R11 has them                                                                                                                                                                                                                                                                                                             |
| R12   | `SettingSnapshot`, `MEMORY_ITEMS`, `MemoryItem`                                                                        | planned (R12.T1–T3), not built                                                                                                         | Read this plan's settings and categories (asks, Risks)                                                                                                                                                                                                                                                                                            |

The research's counting harness (`tools/triCount.ts`) and its JSON were not carried with its
records; T1.b rebuilds it on `levelBoundsCounts.ts` (Risks).

## Running beside this plan

| Lane                               | Task IDs             | What it changes                                                                                                                                                                        | What this plan takes, and the order                                                                                                                                                                                                             |
| ---------------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `geometry-clusters-1` (running)    | R14.T1.a–e           | This plan's T1, as written below                                                                                                                                                       | T1 is that brief, task for task; its records land here                                                                                                                                                                                          |
| `seamless-1-no-collapse` (running) | R10's records        | `PatchCache.insert` refuses rather than evict the selection's chain; perhaps a sub-patch stand-in (A's slot and a sub-rectangle); the seamless probe as a slow test; the 253 s flicker | The refusal binds the page pool too (Design note 14). A sub-patch at a level offset of 1–3 is a set of A's whole clusters (16, 4 or 1 block), which T2.a draws exactly (Design note 19). Its probe is T3's acceptance harness                   |
| `seamless-2-time-morph` (after 1)  | R10's records        | k = max(k_CDLOD, F(v, t)) from per-instance fades; the instance record may grow by a `vec4u`; the bands' τ′ slewed 2% a frame                                                          | The cluster path calls the same vertex function, so F comes with it; nothing reads 32 B for the record (`INSTANCE_RECORD_BYTES`). T3's stride changes take the same F (Design note 19). T2's resolve is the cheap home of a later dithered fade |
| `seamless-3-lookahead` (after 1)   | R10's records        | A look-ahead selection at weight 0.5; L, the demand-to-resident p50, as a getter                                                                                                       | T5's mesh requests read L for their prefetch                                                                                                                                                                                                    |
| `perf-terrain-bakes` (running)     | R10.T6.a, R05.T18    | The bake's cost and the bakes a second (workers, SIMD, shared normals)                                                                                                                 | T3.b's error output is computed beside the bake after this lane lands, in the worker, from the bake's heights                                                                                                                                   |
| `diag-frame-cost` (running)        | (the owner's report) | The Mac's frame breakdown, its profile command, everything but the terrain's triangles                                                                                                 | Its per-pass split decides T2 or T3 next (Tasks). Its profile command is a by-hand run of every task here                                                                                                                                       |

## Design notes

1. **One path, and what stays where.** The research (`research-terrain-gpu-driven`, 2026-11-01)
   found the terrain triangle-bound, not CPU- or draw-bound: on the hard bound every pose above 1 km
   on the three sample worlds draws the whole budget (603 patches, 6.48 M lit triangles on high,
   quads of 1.5–2.6 px), the four cascades 1.3–2.9 times that again (2.4–4.0 below 1 km), and the
   overlay's depth once more: 21–32 M triangles a frame on high above 1 km, 14–21 M below, 4.0 M on
   low. Selection costs 0.2–1.2 ms on its own cadence, and six draws a lit view (one lit, four
   casters, one overlay) are not the cost. So:
   - **On the CPU, unchanged**: selection (pure, R05 Design note 7), the draw set and its stand-ins,
     the coarse–fine holds, the cascades' fit, the forced region, the bakes' priority.
   - **On the GPU, new**: per (instance, cluster, pass), the cull, the level of detail and the
     append; the draws by indirect bucket; from T2 one shading a pixel.
   - **Consumers differ** only in their producer, their pages' records, their vertex fetch and their
     material; the record, the templates, the cull, the buckets, the counters, the visibility buffer
     and the pool are shared. That is the owner's "one system for every kind of 3D model".
   - Nanite's ideas that survive WebGPU are taken (clusters with an error pair, a parallel cut,
     compute culling, compacted lists drawn indirectly, a visibility buffer through the hardware
     rasteriser, two-pass occlusion, paged streaming); its software rasteriser, persistent threads
     and mesh shaders are not (Design note 6). Karis 2021, SIGGRAPH Advances, "A Deep Dive into
     Nanite Virtualized Geometry"; Haar and Aaltonen 2015, SIGGRAPH Advances, "GPU-Driven Rendering
     Pipelines"; Wihlidal 2016, GDC, "Optimizing the Graphics Pipeline with Compute" (_recalled_).

2. **The cluster format and its size.** At most 128 triangles a cluster, Nanite's size, so a
   cluster's triangle index fits 7 bits of the visibility ID.
   - **Terrain**: the 64 × 64-quad grid splits into 64 blocks of 8 × 8 quads (128 triangles, 81
     vertices), and each edge's skirt feet into 8 strips of 8 quads (16 triangles; the feet are one
     winding, so the brief's 32 / 16 / 8 / 4 counts them twice). At stride s a block is 128 ÷ s²
     triangles (128 / 32 / 8 / 2) and a strip 16 ÷ s (16 / 8 / 4 / 2). Skirt heads and fins stay in
     the edge patches' own lit mesh (T1.a).
   - **A block's footprint** is exactly that of the patch three levels finer, (f, L + 3, 8i + bx,
     8j + by), so its bounds are `patchBounds` of that key with the bake's two-channel height range,
     with no new bake output. They are conservative by convexity: the block's own vertices at both
     channels lie in the volume, and a morphed vertex lies on a chord between them, inside any
     convex bound (R05 Design note 8's proof covers the L + 3 patch's own mesh, not this one). The
     key needs L + 3 ≤ `MAX_LEVEL` (24), true while a body's finest level is at most 21 (an Earth's
     is 19); past it a block takes the patch's own slab, padded as `bounds.ts` pads. A strip's
     bounds are its edge block's with the range's floor lowered to min(h_min − the slot's skirt
     depth, 0), where its feet hang. (The brief's "the patch's oriented box split 8 × 8 over its
     tangent axes" is not conservative where the warp's spacing or the curvature varies across the
     patch, so that a block's vertices fall outside its slab; the descendant's bounds are, and cost
     the same.)
   - **The record** is 32 B (Provides): 96 a terrain slot, 3,072 B, under 1% of a high lit slot
     (330,996 B), 3.7 MB at 1,208 slots and 1.2 MB at 375. Written once, when the slot is filled; a
     rebake rewrites it.
   - **Per frame** only the consumers' instances are written, as today (terrain: the 32 B instance
     record a drawn patch, origin less the camera narrowed once from `f64`).
   - **Meshes** (T5): 128 triangles and at most 256 vertices a cluster, 8-bit local indices,
     positions quantised about the page origin, so a page is position-independent and a cluster
     record's bounds are page-local. WebGPU's index formats are `uint16` and `uint32` only, so 8-bit
     indices are always pulled from storage: three vertex invocations a triangle, 384 a cluster
     against about 70 indexed, the de-indexed ID path's factor (about 4.7 times a terrain block's
     81). T4.b and T5.e count it.

3. **The error metric and the two modes.** Object-space geometric error in metres, projected by the
   one function selection uses, `screenSpaceErrorPx` (ε W ÷ (2 d tan(fov_h ÷ 2))), with d the `f64`
   distance to the cluster's bounds as R05 Design note 7 takes it to a patch's, against a per-pass
   tolerance: for a camera pass τ′ px, the view's selection tolerance (`effectiveTauPx`); for a
   cascade its texel, 2 r ÷ 2,048 m, the finest detail the map resolves (T1.d). The overlay's depth
   has no slope term to spend: since R10.T16.c's grazing fix its terrain depth is pushed only by
   `TERRAIN_DEPTH_FRACTION`, 2⁻¹⁶ of the depth (about 0.15 m at 10 km, 4.4 m at 287 km), and the
   survey edge pulls its own depth, so the overlay's pass stays at stride 1 (T1.e). Two modes share
   one record:
   - **Strided template** (terrain, the smooth figure): the grid drawn at stride 1, 2, 4 or 8. Until
     T3 the error is a spacing proxy, s × the level's largest spacing (`vertexSpacing(a, L).maxM`),
     used only by the casters (T1.d), where the rule is the texel. From T3.b the worker measures
     e_s, the largest distance, over every stride-1 vertex, between the stride-s mesh and the
     stride-1 mesh, formed in `f64` from the drawn 3D offsets about the patch origin (on high the
     bake's `BakedOffsets`, on low the worker's own `f64` positions), for both channels, own and
     morph target (k = 0 and k = 1). Two terms make it more than a height difference:
     - **the morph**: `patchVertexOf` draws mix(own, morph, k), so the two meshes differ by (1 − k)
       Δ_own + k Δ_morph, linear in k, and a bound at k = 0 and k = 1 bounds every k; the own
       channel alone misses Δ_morph, which at s ≥ 4 is the parent channel's own stride error;
     - **the chord's sagitta**: a stride-s triangle is a flat chord s times longer, sagging below
       the datum's curve by about s² × `chordSagittaM(planet, L)`, 64 × 4.6 m, about 290 m, at s = 8
       on level 4 of an Earth; a 3D distance includes it, a height interpolant does not.

     e_s is made monotone (e₁ ≤ e₂ ≤ e₄ ≤ e₈) and rounded up into `f16`. The vertices hold the
     maximum: both meshes split their quads on the same diagonal, so each fine triangle lies in one
     coarse triangle, the difference is linear on it, and its length, being convex, is greatest at a
     vertex. A camera pass takes stride s only where the level's stated error plus e_s, projected at
     the cluster's own distance, is within τ′, so the drawn error stays within what selection
     guarantees; the gain comes from a patch's clusters beyond its nearest point and from smooth
     ground, and T3.a measures it.

   - **DAG cut** (meshes): a cluster is drawn where its own error ≤ tolerance < its parent group's,
     both projected at their groups' LOD spheres; monotone errors and nested spheres make the cut a
     consistent, crack-free one with no traversal (Karis 2021, _recalled_). In this mode the
     record's sphere is the cluster's group's LOD sphere, which also bounds the cluster for culling,
     and a 16 B `ClusterLod` record beside it holds the parent group's sphere. An instance's errors
     and LOD spheres are scaled by its largest scale factor before projection (R11's rocks are
     scaled instances). No morph: sub-pixel pops at τ ≤ 1 px, as Nanite accepts. The cut needs its
     children resident to be crack-free; Design note 14 says how residency keeps it so.

4. **GPU culling and its conservativeness.** Every test only refuses what cannot reach a pixel the
   pass draws, so the exact levers keep the image byte for byte:
   - Frustum and horizon: the record holds a sphere, not the 18 numbers of R05's oriented box
     (`PACKED_BOUNDS_LENGTH`), so the GPU tests the sphere: against the frustum's planes, against
     the horizon by the sphere form of Cozzi and Ring's test (_recalled_), and by `castsInto`. Each
     refuses only where the `f64` sphere predicate refuses, which implies R05's box predicate
     refuses, so the cull is conservative and keeps more than the CPU's box would. Tests run in the
     frame the instance origins are in (the body's rotated axes, as `#write` forms them), from the
     same `f64` camera-relative origin narrowed once. The `f32` tests widen by their own arithmetic
     and by ulp(the patch's half-size) of the narrowed offsets (about 3 cm at level 4 on an Earth),
     which the `f64` bounds' allowance, 64 ε (a + H), does not cover; the twins pin both. The
     horizon test takes its cone's terms formed in `f64` on the CPU (|C|² − 1 in R_occ's units,
     which `f32` cannot form near the ground), and is skipped below R_occ as R05's is. Exact. (If
     T1.b's counts show the sphere keeping too much, the 48 B record of Provides holds the box.)
   - Casters (T1.b): `castsInto` at the cluster's sphere, and refused wherever the patch's is. Exact
     under `castsInto`'s premise that a terrain caster lies within the caster reach ℓ of any
     receiver it shadows.
   - **The twins' claim**: at a boundary an `f32` test and its `f64` twin differ, so no twin can
     refuse "exactly" what the CPU refuses. Each GPU test never refuses what its `f64` predicate
     keeps, and keeps nothing it refuses beyond its widening; that holds for the frustum, the
     horizon, `castsInto` and the receiver-depth tests alike.
   - Casters by receiver depth (T1.c): receivers sample cascade k only at view depths in
     [fadeStart₍ₖ₋₁₎, C₍ₖ₊₁₎] (0 for k = 0), or by falling through from a slice its cascades do not
     cover. A caster of view depth d and radius ρ shadows receivers only within [d − ρ − ℓ max(0,
     s), d + ρ + ℓ max(0, −s)], s the sun's direction on the view axis; it is culled from cascade k
     where that interval misses k's and its light-space footprint (± ρ, the normal offset and
     `CASCADE_FOOTPRINT_TEXELS` included) lies inside cascade k − 1's covered square, so no
     fall-through receiver needs it. Exact under the same premise; non-terrain casters are
     untouched.
   - Occlusion (T3.d): two passes, as Karis 2021. The clusters visible last frame are drawn first, a
     depth pyramid is built from that depth, every cluster is tested against it, and the newly
     visible are drawn. Each pyramid texel holds the farthest depth (reversed-Z: the minimum) of
     every source texel it covers, the odd row or column included (540 → 270 → 135 → 68), and a
     cluster is refused only where its nearest depth lies behind the pyramid's farthest over its
     whole screen rectangle. Exact for the current frame, since the first pass's geometry is this
     frame's.
   - The LOD choice is not a cull: strides change pixels (T1.d, T3) and are bounded, not exact.

5. **Compaction, order and byte-identity.** Depth compare is `greater-equal`, so where two surfaces
   meet at exactly equal depth the later draw wins; a skirt's top row meets its neighbour's grid
   edge so.
   - **The same clip position in every pipeline.** T1.a's interior draw, the cluster path's bucket
     stage and T2.c's ID-only stage each compute a clip position in a pipeline of their own (and the
     compiler strips the ID-only stage's other outputs). WGSL holds a position to the same bits
     across programs only where `@builtin(position)` is `@invariant`, so every terrain vertex stage
     carries it, today's included: landed with T1.a (13100d5b, the six pinned lit, depth and caster
     sources re-pinned), and every byte-identity capture of this plan is taken against a base that
     has it. Each new vertex stage here carries it too.
   - Depth-only passes (casters, the overlay's depth) compact with `atomicAdd` appends. Their result
     is order-independent: the final depth is the nearest whatever the order, and their discards are
     per fragment. Such a cull is `presentation-only`, and its lists never reach the CPU.
   - A colour pass needs the draw set's order. From T2.a a stable compaction (per-instance counts,
     an exclusive scan, a scatter) writes each bucket in instance order. WebGPU gives no forward
     progress between workgroups, so no single-pass decoupled look-back: a reduce-then-scan in two
     or three dispatches, integer throughout, so its subgroup variant is `bit-exact` with its
     reference.
   - The counters are integer sums, read through a `bit-exact` reduction into a ring
     (`ClusterStats`), behind the measurement switch only. The reduction may read arguments that the
     `presentation-only` cull wrote because integer `atomicAdd` counts do not depend on the order of
     the adds; only the lists' order is irreproducible.
   - **Tie pixels.** T1.a draws the interior patches before the edge patches, and buckets draw by
     template, so the order at a skirt-grid tie can change. Where captures differ only at such
     pixels, each task lists them with their largest difference; if they show, the fallback is one
     list in the draw set's order drawn by the generic template (T4.b's index pulling).

6. **WebGPU's limits, and the fallbacks per adapter.** Core WebGPU has compute, storage buffers,
   `drawIndexedIndirect` and `dispatchWorkgroupsIndirect`, 32-bit atomics in storage and workgroup
   memory, and pass-level `timestampWrites` under the optional `timestamp-query`; it has no mesh or
   task shaders, no 64-bit atomics, no multi-draw-indirect, no bindless resources and no forward
   progress between workgroups (W3C WebGPU and WGSL, _recalled_). So:
   - **No 64-bit atomics** rules out Nanite's software rasteriser (an atomic max of depth and ID):
     the visibility buffer goes through the hardware rasteriser into an `r32uint` target.
   - **`firstInstance` must be 0** without `indirect-first-instance` (a non-zero one makes the draw
     a no-op), which R01 does not request: each bucket draw carries its list's base in its `Draw`
     uniforms, and `baseVertex` stays 0, since a template indexes its own small mesh.
   - **Storage bindings: 8 a stage** (`maxStorageBuffersPerShaderStage`, never raised). The lit
     vertex stage reads 6 today (heights, slots, instances, contacts, offsets, survey mask); the
     cluster list makes 7, with each cluster's (x0, y0) derived from its index (reading the records
     would make 8, the limit, before T3.c's strides). The resolve (T2.d) needs the vertex stage's 6,
     the list, and the fragment's class layers, lights and horizon map: 10. T2.d brings it to 8 or
     fewer by moving the contacts (32 KiB at most) and the lights into uniforms, or the list into
     the instances' buffer, and a test counts every material's bindings a stage.
   - **Sizes**: a storage binding is 128 MiB and a buffer 256 MiB by default; R01 raises both to the
     adapter's, at most 1 GiB. Every cluster buffer is sized from the slot count and stays under 128
     MiB, so the default-limits run (R01's `defaultLimits`, R05's `FaceDifferences` path) draws the
     same way. The mesh page pool's 128 MiB on high (T5) is exactly the default binding size, so its
     vertices and its indices are separate buffers, each one binding within it.
   - **Workgroups**: at most 256 invocations and 65,535 a dimension; a cull of more instances than
     that (T5's scatter) folds them into two dimensions.
   - **Triangle IDs**: `@builtin(primitive_index)` where the device offers the optional
     `primitive-index` feature (_recalled_: recent Chromium on Metal, D3D12 and Vulkan; T2.c adds it
     to `WANTED_FEATURES`, never required); otherwise a de-indexed draw whose triangle is
     `vertex_index ÷ 3`, costing about three vertex invocations a triangle, on a vertex stage that
     writes only the position and the flat ID. `instance_index` is a vertex-only builtin, so the
     `primitive_index` path still passes one flat varying, the visible-cluster index: 20 B a vertex
     either way.
   - **Subgroups** where offered (shipped in Chromium 134, _recalled_), never assumed 32 wide (Intel
     Gen9 runs 8, 16 or 32); the reference kernels without them.
   - **Indirect validation**: Dawn checks indexed indirect draws against the bound index buffer on
     the GPU, so a bad bucket draws nothing rather than reading out of range (_recalled_); it costs
     a small dispatch a render pass.
   - **No `timestamp-query`**: the counters without times.
   - **Per adapter**: every lever is a `GeometrySettings` field, and a capability the adapter lacks
     selects the fallback beside it (`primitive-index` → de-indexed; no subgroups → reference
     kernels); no adapter loses the path itself, since everything it needs is core.

7. **Apple's tile-based GPUs and discard (Apple Silicon first).** Apple's GPUs bin every
   post-transform triangle with its varyings into tiles before shading, so the cost grows with
   triangles × varyings, and a pass whose binned data overflows its buffer is split into partial
   renders that store and reload every attachment. Their hidden-surface removal rejects occluded
   opaque fragments before they shade, but not those of a shader that may discard, write depth or
   blend, which must shade in order; Apple's guidance is opaque first, then alpha-tested, then
   blended (Apple, "Tailor your apps for Apple GPUs and tile-based deferred rendering", _recalled_).
   An M1 has about a tenth of the RTX 3080's arithmetic (2.6 against 29.8 TFLOP/s; Pro 5.2, Max
   10.4, _recalled_), and its memory is the system's.
   - High's 21–32 M triangles and the lit pass's 92 B of varyings a vertex (about 285 MB a pass of
     post-transform data, an estimate) are the shape that hurts most there, and the survey `discard`
     on every patch keeps every terrain fragment out of hidden-surface removal.
   - So T1.a draws the interior patches, with no discard, before the edge patches; T1 halves the
     triangles; T2 leaves the raster pass only a position and a flat ID; every task's acceptance
     includes the Mac's fps.
   - The bodies pass is not yet opaque first: it draws the mesh bodies' figures, which may discard
     (`smoothMesh.wgsl`), before the terrain (`photoreal/renderer.ts`). T2.e orders it as interior
     terrain, then the mesh figures, then the edge patches (depth decides; ties listed).
   - **Pass splits cost a tiler its attachments**: each extra render pass stores and reloads them,
     at 1080p about 25 MB of `rgba16float` and `depth32float` each way. T2 splits the bodies pass
     into raster and resolve, and T3.d again around the pyramid's build, so each task that splits a
     pass counts its attachments' loads and stores in its Mac run.
   - The Mac's own split is unmeasured: 1 fps is 5–20 times what linear scaling of the RTX's frame
     predicts, so `diag-frame-cost`'s profile decides how much is terrain at all (Risks).
   - `navigator.hardwareConcurrency` counts efficiency cores, so the height workers' count on Apple
     Silicon is `perf-terrain-bakes`'s to set; and the unified memory makes every byte of Design
     note 15 system memory.

8. **Linux and Windows.** Dawn's backends are Vulkan (Linux: the RTX 3080, the recommended
   specification; the UHD 620 on Mesa's ANV, the minimum), D3D12 (Windows) and Metal (macOS). Every
   feature here is core on all three; the cross-platform rules are the repository's:
   - **D3D12**: Dawn supplies `instance_index`'s first instance itself (_recalled_); with
     `firstInstance` and `baseVertex` 0 everywhere, nothing depends on it. Large shaders compile
     slowly there, so every new pipeline is made by `createMaterialAsync` or `createComputeAsync`.
   - **Vulkan on ANV**: subgroup sizes vary (Design note 6); `primitive_index` needs the geometry
     shader feature, which Gen9 has (_recalled_).
   - **Builds and lints** on all three: the client is platform-free; T5.a's Rust tool runs
     `just cross-clippy`.
   - **Timers**: off Linux, R12.T0's audit finds Dawn's `timestamp_quantization` left on, so
     per-pass times on the Mac and Windows are quantised; fps comes from frame intervals.

9. **The visibility buffer (T2).** The lit pass splits in two:
   - **The raster**: the cluster path's buckets draw into the scene's depth and an `r32uint` ID,
     (visible-cluster index << 7) | triangle, 0xffff_ffff for none (reserved from the index space),
     cleared each frame. The engine draws neither today: T2.b adds the `r32uint` target and a target
     drawn with another's depth (Provides). The vertex stage writes only the clip position and one
     flat varying: the visible-cluster index on the `primitive_index` path, the ID on the de-indexed
     one. The interior patches draw first with no discard; the edge patches draw after with the
     survey `discard` kept in the raster: an unsurveyed fragment must write no depth, or it would
     hide R07's reference surface beneath, so the survey test cannot move to the resolve (a
     correction of the research's phase 2).
   - **The resolve**: one full-screen pass into the scene target. A pixel whose ID is 0xffff_ffff is
     discarded and writes no depth. For each pixel with an ID it reads the visible cluster's
     template from its list entry (the cluster word's top 3 bits carry it: the stride and the
     shape), refetches the triangle's three vertices through the consumer's vertex function
     (terrain: `lit_place`, so the morph, the holds, the contacts' hold and seamless-2's fade agree
     with the raster), forms perspective-correct barycentrics from the pixel, and calls the
     consumer's shading once. The terrain's shading needs no screen derivatives except the geometric
     normal, which the lit fragment takes from `dpdxFine` and is the triangle's face normal: the
     resolve takes it from the three vertices. Its tiles are read by `textureLoad` and its cascades
     by `textureSampleCompareLevel`, both allowed in non-uniform control flow.
   - **Order with other passes**: no opaque pass draws between the raster and the resolve; passes
     drawn before the raster write depth that the raster's test respects, and passes drawn after the
     resolve depth-test against the terrain's depth.
   - **What it removes**: the 2–4 shadings a pixel that 1–3 px² triangles cost in 2 × 2 quads, the
     92 B of varyings in the tiler, and the discard on interior patches.
   - **Sizes**: the ID target follows the scene target's size, R07's internal scale included. A
     still (R11.T10) larger than 4,096² takes the forward path or renders in tiles, so that the ID
     target stays within 64 MiB (an ask of R11.T10, Risks).
   - **Fallback**: the forward cluster pass (T2.a), the setting's `shading: "forward"`.

10. **Determinism and the collision ground.** No generated value moves: heights, selection, the draw
    set, the morph, the forced region and every golden are untouched.
    - **The forced region**: no camera, visibility or overlay pass takes a stride above 1 for a
      cluster of a patch in the forced region (`inForcedRegion`'s patches, flagged per instance),
      and the contacts' hold is unchanged (k = 0 within each held radius, rising across one finest
      patch beyond it), so the drawn ground under every grounded or descending body is
      `finest_surface_height`'s interpolant where it was before (R05 Design note 6, R10 Design note
      4), on every setting and in both styles. Casters may stride there: a shadow is not the ground.
    - **Twins**: the cull, the templates, the strides' errors, the compaction and the resolve's
      barycentrics each have a TypeScript twin tested in Vitest, as `vertexEmulation.ts` and
      `cascadeShadow.ts` are.
    - **Readback**: only `bit-exact` results reach the CPU (R01 Design note 16): the counters, never
      the atomic lists.
    - **Captures**: an exact lever's acceptance is byte-identical captures in test-render and the
      terrain run against the task's base build, compared in the lane and never stored (the
      brainstorm's "no golden images").
    - **Meshes** (T5): the offline builder is deterministic (the same input gives the same bytes, on
      every platform); the procedural builder draws from a seed in a worker; collision never reads a
      render mesh (authored hulls; R11's analytic rocks).
    - `golden_diff.py` says "No golden files changed" for every task.

11. **The atmosphere's aerial perspective.** R08's composite reads the scene's colour and reversed-Z
    depth and lays aerial perspective over every opaque pixel, so the visibility buffer writes the
    same depth into the same target and the composite is unchanged. On high (`scene` scope) a mesh
    consumer draws into the scene before the composite and takes aerial perspective with the
    terrain; on low (`terrain` scope) it draws after the composite without it, which is the low
    setting's "aerial perspective on terrain only". A translucent consumer, none yet, reads
    `HillaireAtmosphere.aerialPerspective`'s published volume.

12. **The shadows' casters.** The four cascades are the terrain's largest pass: 39–67% of high's
    triangles. T1 moves the terrain's casters onto the cull: refused per cluster where `castsInto`
    refuses (T1.b), refused by receiver depth (T1.c, 21–52% of caster triangles above 1 km on the
    counts), drawn at their cascade's texel (T1.d). Non-terrain casters stay `ShadowCasters`'
    providers until T6.c moves them onto the cull. The casters morph with the camera's distance, as
    the lit patches do, so T6.a skips a cascade only when all its inputs are unchanged. The light
    basis and every instance origin are in the body's rotated frame, so while simulation time runs
    the body's rotation and the sun's body-fixed direction change every frame: T6.a gains only with
    time paused and the camera still, and in flight T6.b's lower rate for far cascades is the lever.
    Mesh receivers sample the cascades as the terrain does; the horizon map is the terrain's own and
    shadows no mesh (Risks).

13. **The wireframe style.** The wireframe draws the terrain once, depth only at its own τ of 4 px,
    with contours evaluated from the interpolated height in the same pass. A stride would move the
    contours by the stride's height error over the local slope, which on flat ground is many pixels,
    so the wireframe's terrain stays at stride 1 and takes only the cull and the interior split
    (T4.c). Hull edges, graticules, orbits and contours stay R02's lines; the path supplies their
    occluders' depth (the hulls in T4.b, R11's rocks in T5), at a tolerance under the strokes'
    half-width so that hidden-line removal holds.

14. **Streaming and residency.** Terrain's residency is R05's patch cache, with the two pins it has
    (forced, drawn) and seamless-1's refusal to evict the selection's chain; T5's `GeometryPagePool`
    is the same contract for mesh pages: fixed slots, LRU, the same pins, a request queue scored by
    projected error, visibility and motion (with seamless-3's L for look-ahead), and a residency
    answer per page.
    - **Producers differ**: workers bake terrain pages; authored pages are fetched from the app's
      own assets and decoded in a worker; procedural pages are built from a seed in a worker.
    - **Never a hole**: a missing terrain patch is drawn by its nearest resident ancestor (R05). A
      DAG cut draws a cluster where own ≤ τ < parent, so where τ asks for children whose pages are
      not resident, the parent is not drawn (τ < own) and the children are absent; pinning the roots
      does not fill that gap. So on a group's pages arriving or leaving, the pool rewrites its
      parent clusters' records:
      - a parent whose children's group is not wholly resident has its own error set to 0, so it
        draws whenever τ < its parent's;
      - a group's children are drawn only once all of its pages are resident;
      - all of a group's pages are evicted together;
      - a mesh's DAG root pages are pinned, so some cut is always resident.
    - **The page table** earns its place only where a cluster reads pages other than its own
      (normals from a finer resident page, a mesh's shared vertex pages); until then a slot index is
      the address, as today.

15. **Memory budgets per setting.** Estimates from formats and sizes, to be measured by each task
    (R12 Design note 6's tally reads the categories):

    | Item                                   | High                                                        | Low                      | Category            |
    | -------------------------------------- | ----------------------------------------------------------- | ------------------------ | ------------------- |
    | Cluster records, terrain               | 3.7 MB (1,208 slots)                                        | 1.2 MB (375)             | `geometry-clusters` |
    | Cluster records, wireframe terrain     | 1.6 MB (520 slots)                                          | 1.6 MB                   | `geometry-clusters` |
    | Buckets and arguments, a lit view (T1) | ≤ 15.8 MB (4 cascades × 4 strides, the overlay at stride 1) | ≤ 0.3 MB (the overlay's) | `geometry-clusters` |
    | The same with T2.a's scan              | ≤ 5.6 MB (6 passes, one list each)                          | ≤ 0.6 MB (2 passes)      | `geometry-clusters` |
    | Visibility IDs, a view (T2)            | 8.3 MB at 1920 × 1080                                       | 3.7 MB at 1280 × 720     | `render-targets`    |
    | Depth pyramid, a view (T3.d)           | 2.8 MB (from half resolution)                               | 1.2 MB                   | `geometry-clusters` |
    | Mesh page pool (T5)                    | 128 MiB (lean)                                              | 32 MiB (lean)            | `geometry-pages`    |

    Buckets are sized by `InstanceRecords`' capacity, the slot count (1,208 high, 375 low), times 96
    clusters and 8 B an entry, so no draw set can overflow them (Design note 16). Before T5's pool,
    high adds about 5.3 MB shared and 16–17 MB a lit view (T1's buckets 15.8 MB; from T2.a the
    lists, the IDs and the pyramid, 16.7 MB), against a lit slot cache of 400 MB and the
    brainstorm's ceilings (a resident peak above 2 GB a finding and above 3 GB a failure on the
    discrete target, 1 GB on the low setting). Several views multiply the per-view rows; a wireframe
    instrument has no cascades, visibility IDs or pyramid.

16. **Failure and loss handling.**
    - **Device loss**: `ResilientEngine` rebuilds the engine and `TerrainPass`'s device record is
      made again (`#rebuild`); the cluster resources belong to that record, so the records are
      rewritten from the residents as their slots are re-uploaded, the buckets and targets made
      again and the kernels created again. The page pool re-uploads from its CPU-side decoded pages
      within its budget, else refetches.
    - **A refused kernel or material** (`GRAPHICS SHADER REFUSED: <displayName>`): the pass falls
      back to today's path for that consumer from that frame on, and states it in its fault, as a
      terrain fault does today.
    - **Overflow** is impossible by construction: a bucket's capacity is the consumer's whole
      cluster count (terrain: `InstanceRecords`' capacity, its slot count, × the template's clusters
      an instance, and from T2.a 16 interior-edge strips an instance drawn as a sub-patch mask,
      Design note 19). T5's pool sizes its lists from its slots.
    - **No timestamps, no subgroups, default limits**: Design note 6's fallbacks.

17. **The settings ladder R12 holds, and the way back.** `GeometrySettings` is a field of R05's
    `ViewSettings`, as R12 Design note 3 asks of every plan, with both settings built together (the
    budget's third rule). R12.T0 inventories it; R12.T11 may move its values within this policy,
    never the policy:

    | Field             | High                      | Low                                           | Policy                                               |
    | ----------------- | ------------------------- | --------------------------------------------- | ---------------------------------------------------- |
    | `clusterPath`     | on                        | on                                            | Every consumer on the path; off is today's path      |
    | `casterDepthCull` | on                        | on (no cascades on low)                       | Exact                                                |
    | `casterStrideCap` | 8                         | 8 (no cascades on low)                        | Casters at their texel                               |
    | `overlayDepth`    | T1.e's choice             | T1.e's choice                                 | No mark's visibility changes                         |
    | `shading`         | `visibility` (after T2.e) | `forward` until R12.T8.b's UHD 620 run (lean) | One shading a pixel where it pays                    |
    | `strides`         | `measured` (after T3.c)   | `measured`                                    | Drawn error within τ′; stride 1 in the forced region |
    | `occlusion`       | `hzb` (after T3.d)        | `none` until measured (lean)                  | Exact                                                |
    | `pageBytes`       | 128 MiB                   | 32 MiB                                        | Within the memory ceilings                           |
    - Nothing here moves a figure the brainstorm states (τ, 720p, 30 fps, the 4 px wireframe
      tolerance): strides stay within τ′, and `TERRAIN: DETAIL LIMITED` keeps its trigger, the
      selection's budget binding; if T3.e lets a triangle budget bind beyond τ′, that raises it too.
    - **The way back**: today's path stays in the tree, compiled and drawn by test-render at every
      setting, until the owner's Mac and RTX runs sign off T1–T3 (Verification); its removal is a
      later task (Risks).

18. **Measurement.** Every task is accepted on the same yardsticks:
    - **Counts** (the pod): triangles and draws a pass from `ClusterStats` in test-render's
      `R10.T16 the frame` group (a known scene, both variants) and the counting harness over R05's
      descent at the record seed (124 poses, 1920 × 1080 at 60°) on R10.T4.b's three sample worlds,
      both settings; before and after.
    - **GPU per pass on SwiftShader**: relative only.
    - **Timed passes**: each dispatch takes one of `PASSES_PER_FRAME` (64) timed passes. A view's
      cull, the scan's two or three dispatches, about ten pyramid mips and the second cull can pass
      64 with three views, so the pyramid's mips fold into one dispatch, or are timed under one
      label.
    - **The owner's runs** (pending each time): the Mac (M-class, Metal) and the RTX 3080 at 1920 ×
      1080, both settings, `just views-check --terrain --whole --hidden` and `diag-frame-cost`'s
      profile command once it lands: fps, the RTX's p50 and p95 against R08's 15.67 ms target and
      16.67 ms blocker, and GPU per pass.
    - **The seamless probe** (seamless-1's slow test) for anything that changes what a frame draws
      as the camera moves (T3).
    - The terrain views check does not finish on SwiftShader in a pod (high hit the software
      watchdog after 2,462 s; low could not open its instruments at 1280 × 720): try it once, then
      NOT RUN.

19. **Seams with the seamless lanes.**
    - **Stand-ins**: the draw set stays the CPU's, so "keep the parent until its children are
      resident" and seamless-1's refusal hold unchanged. seamless-1's sub-patch stand-in (its
      step 3) draws a missing patch as its ancestor A's slot and a sub-rectangle: the instance's
      65-grid mapped into A's grid at the sub-grid points, sampled on A's diagonal. A's 96 records,
      written per slot, do not bound what that instance draws, and at a level offset of 4 or more
      (the collapses were 3–10 levels) it is smaller than a block, so no block mask can express it:
      - until T2.a, an instance with seamless-1's sub-patch fields set is drawn by the edge patches'
        path (today's: `#drawCasters` and the lit mesh) in every pass, the casters (T1.b–d), the
        overlay's depth (T1.e) and the wireframe (T4.c) included;
      - from T2.a it is a cluster mask over A's blocks only where its level offset is 1–3 (a 4 × 4,
        2 × 2 or 1 × 1 set) and seamless-1 draws it as A's own blocks; otherwise it stays on the
        edge path, and its pixels are listed as T2.a's tie pixels are;
      - a mask's edges are interior lines of A's grid, where the lit mesh has no skirt, and the
        coarse–fine holds span one level, not offsets 2–3, so a mask instance draws foot strips
        along each of its interior edges: a stride-1 strip template placed at any multiple-of-8
        line, bounded by its block's bounds lowered as an edge strip's are (at most 16 strips an
        instance, counted in the buckets' capacity);
      - T2.a consumes sub-patches and never builds them (the draw set is R05 Design note 10's, the
        sub-patch seamless-1's).
    - **The time morph**: F enters through the vertex function both paths call. A stride change in
      T3.c is a transition of its own, a stateless distance morph (T3.c), so F stays the fade of a
      data change (a rebake, a level change), as seamless-2 builds it into `lit_place` (k =
      max(k_CDLOD, F)), and the stride's morph moves the place `lit_place` returns.
    - **Fades on the visibility path**: a dithered cross-fade costs least there, as complementary
      dither masks in the ID raster (on the fading patches alone) and one shading in the resolve,
      not as a second lit draw; the hand-over's cross-fade (R10.T10.g's remedy 2) and new survey
      rising out of the reference surface may move there once built.

## Tasks

Order: T1 first, the shared substrate and the largest exact cut (running as `geometry-clusters-1`).
Then the owner's Mac and RTX runs before and after T1, with `diag-frame-cost`'s split, decide T2 or
T3 next; the lean is T2 first on Apple, whose gains (overshading, varyings, hidden-surface removal)
are certain where T3's wait on a histogram. T3.a's histogram runs straight after T1, since it is
cheap and decides T3's gain. T3.b follows `perf-terrain-bakes`; T3.c follows seamless-2; T3.e
follows seamless-1–3 and a ruling. T4 follows T1 (T4.b needs T2.a's stable list only for its colour
silhouette). T6.a may run any time after T1.b; T6.c follows T4.b. T5 follows T4 and waits on content
(R11's rocks and particles, craft art), with T5.a buildable on procedural test meshes beforehand.

Client files are under `apps/hyperion/src/renderer/src/view/` unless a path says otherwise. Every
task runs:

- Vitest for its twins and packings, `pnpm --filter hyperion exec vitest run <path>`;
- `just test-render`, both variants, re-pinning the shader digests of every terrain and caster
  source it edits;
- the counts of Design note 18, before and after;
- `golden_diff.py`, which says "No golden files changed";
- and lists the owner's runs under NOT RUN with their commands.

Sizes are the lanes' (S, M, L, XL).

### R14.T1 The cluster path, terrain as its first consumer

Size L (`geometry-clusters-1`, client only). No generated value moves; the lit image is
byte-identical to the base's for every step but T1.d, whose change is bounded, and T1.e keeps every
mark's visibility. Expected triangles a frame, recomputed per band from the research's table (lit
from its interior patches, casters from its "both" column, the overlay either from the scene's depth
or at stride 1, which draws as many as the interior patches):

| Setting and band | Today   | Overlay at stride 1 | Overlay from the scene's depth |
| ---------------- | ------- | ------------------- | ------------------------------ |
| High, above 1 km | 21–32 M | 16.2–20.9 M         | 10.9–15.6 M                    |
| High, below 1 km | 14–21 M | 9.6–12.7 M          | 7.0–9.8 M                      |
| Low, above 1 km  | 4.0 M   | 3.2–3.3 M           | 1.6 M                          |
| Low, below 1 km  | 4.0 M   | 3.2 M               | 1.6 M                          |

The research did not model T1.c's fall-through, so the live caster counts will be higher: T1.c's and
T1.d's ratios are accepted against the rebuilt harness with the fall-through modelled, not against
the research's figures.

- _Provides:_ `view/clusters/`' record, templates, cull passes, cull kernel and twin, buckets and
  counters; `GeometrySettings` with `clusterPath`, `casterDepthCull`, `casterStrideCap` and
  `overlayDepth`; the `geometry-clusters` category; the interior/edge split and its classification;
  the per-pass counters in the terrain run's and the descent spike's results; the rebuilt counting
  harness.
- _Consumes:_ R01's `dispatch`, indirect draws, `KernelPair` and `MemoryCategory`; R05's
  `ResidentPatch`, `DrawSet`, `patchBounds`, the frustum and horizon predicates, `vertexSpacing` and
  the lit mesh; R10's `#drawCasters`, `castsInto`, `CascadeSet`, the cascades' receiver twin,
  `lit_place`, `survey_kept_at`, `TerrainLayers.unsurveyedCells` and `symbologyDepth`.

- **R14.T1.a Interior patches draw without fins, skirt heads or discard.** Exact; size M.
  - _Today:_ every lit patch draws the lit mesh's 10,752 triangles with the survey `discard`
    (`terrainLitShade.wgsl`). On a bake with no unsurveyed cell (`TerrainLayers.unsurveyedCells`
    empty) its 1,536 fin and 512 skirt-head triangles are collapsed or wholly discarded
    (`lit_place`, `survey_kept_at`), and the discard never fires, yet costs Apple's GPUs their
    hidden-surface removal.
  - _Lever:_ such patches draw only the grid and skirt-foot triangles (8,704), an index buffer of
    their own over the lit mesh's vertices in its numbering and winding, with a no-discard variant
    of the lit fragment (an override constant or a second entry point), in a draw of their own
    before the edge patches' draw, which stays as it is. Classified per resident bake; a rebake
    reclassifies.
  - _Files:_ `terrain/gpu/resources.ts`, `terrain/terrainPass.ts` (`#write`, `#drawItem`),
    `terrain/shaders/terrainLitShade.wgsl`, `terrain/terrainMaterial.ts`, tests.
  - _Tests:_ the interior index buffer is the lit mesh's grid and feet, in its winding; the
    classification and its rebake; the no-discard variant is catalogued.
  - _Acceptance:_ TERRAIN TEST's captures and the smoke's terrain frames byte-identical to the
    base's (tie pixels listed, Design note 5); lit triangles 0.81 of the base's on a wholly surveyed
    body.
  - _Built_ (389371b; `@invariant` on every terrain vertex stage, 13100d5b): records at the end of
    Risks.
- **R14.T1.b The cluster path's interfaces, casters at stride 1.** Exact; size L.
  - Builds `view/clusters/`' record, templates, cull passes, `clusterCull.wgsl` and its twin,
    buckets and counters (Provides), `GeometrySettings` with `clusterPath`, and the memory category
    `geometry-clusters`, and routes the interior patches' four caster passes through them at stride
    1: the cull reproduces `castsInto` at cluster bounds, and refuses every cluster of a patch
    `castsInto` refuses. Edge patches keep `#drawCasters`' path.
  - Rebuilds the research's counting harness on `levelBoundsCounts.ts` (Risks), with each lever's
    rule as built.
  - _Files:_ `clusters/{record,templates,cullPass,clusterCull.wgsl,cullTwin,buckets,stats}.ts`,
    `quality/qualitySetting.ts`, `engine/memory.ts`, `terrain/terrainPass.ts` (`#drawCasters`,
    `#casterItems`), `terrain/shaders/terrainShadow.wgsl`, `engine/catalogue.ts`, the terrain run's
    results, tests.
  - _Tests:_ the templates (index counts by stride, 128 / 32 / 8 / 2 and 16 / 8 / 4 / 2; every edge
    vertex of a block at stride s shared with its neighbour at s; the lit mesh's winding; strips
    closing to the grid; every template triangle's first vertex has its part's code, a grid triangle
    a grid vertex and a foot triangle a foot vertex); `ClusterRecord` and `CullPass` packing against
    their WGSL structs; over random sets, the cull's twin never refuses what `castsInto` keeps and
    keeps nothing it refuses beyond its widening (Design note 4), and the sphere tests keep every
    cluster R05's box predicates keep; the bucket stage's position is `@invariant`; a block's bounds
    hold all its vertices, morphed or not, on wasm bakes of R10.T4.b's sample worlds (a
    `.wasm.test.ts`) and on R05.T4.b's golden vertices (`vertex_f32.golden`); the buckets' arguments
    for a known set.
  - _Acceptance:_ captures byte-identical; caster triangles reported per cascade, in the terrain
    run's results and test-render's `R10.T16 the frame` log.
- **R14.T1.c Casters culled by the depths their receivers sample.** Exact under `castsInto`'s own
  relief premise; size M.
  - _Today:_ a near patch is drawn into all four cascades: 1.3–4.0 caster patches a drawn patch.
  - _Lever:_ Design note 4's receiver-depth rule, keeping the fall-through case; non-terrain casters
    untouched.
  - _Tests:_ a twin test over random sets and casters: no culled caster has a receiver that
    `cascadeShadow.ts`' receiver twin sends to cascade k.
  - _Acceptance:_ captures byte-identical (the layers may differ only where no receiver samples);
    caster triangles 0.48–0.79 of T1.b's above 1 km on the research's counts, which left out the
    fall-through, so accepted against the rebuilt harness's.
- **R14.T1.d Casters at their cascade's texel.** A lean; size S.
  - The coarsest stride s ≤ 8 with s × the level's largest spacing ≤ the cascade's texel, 2 r ÷
    2,048. All of a patch's clusters take one stride a cascade, so no crack opens inside a patch,
    and the skirts close the rest. `casterStrideCap` caps it.
  - _Acceptance:_ at ≥ 99.5% of the captures' terrain pixels the lit image within 1/3 stop of T1.c's
    (the hand-over check's unit), the rest listed with their largest difference; before and after
    captures in the outbox; caster triangles a further 0.65–1.00 above 1 km and 0.56–0.91 below on
    the research's counts, accepted against the rebuilt harness's.
- **R14.T1.e The overlay's terrain depth at what it resolves.** Choose by what is simpler to keep
  exact, and record the choice; size S–M.
  - _Today:_ `LiveFrameTerrain.symbologyDepth` redraws each of a view's terrain draws (since T1.a,
    the interior patches' among them) for the marks' depth test. Each depth is pushed by
    `TERRAIN_DEPTH_FRACTION`, 2⁻¹⁶ of itself, with no slope term since R10.T16.c, and the survey
    edge pulls its own depth (Design note 3).
  - _Either_ the overlay's depth test reads the photorealistic pass's scene depth where their sizes
    agree (no terrain redraw; on low the scene renders at 720 rows and the internal scale moves it,
    so only where they agree), _or_ the overlay's terrain depth goes on the cluster path at stride 1
    (`strideCap` 1: the cull and the interior split only). No stride above 1 unless its error,
    projected at each cluster, stays within the push's depth (about 0.15 m at 10 km), which the
    research's counts never show at 4 px or less (its overlay ratio at 4 px was 0.80–0.92 on high at
    1–10 km and 1.0 on low). `overlayDepth` records which.
  - _Acceptance:_ no mark's visibility changes in R10.T16's frame group (smoke part 7) or the views
    check's captures; overlay triangles counted, with no ratio target.

T1's NOT RUN: the owner's RTX 3080 and Mac runs of `just views-check --terrain --whole --hidden`,
both settings, and `diag-frame-cost`'s profile command, before and after: fps at 1920 × 1080 and GPU
per pass. They decide T2's and T3's order.

### R14.T2 The visibility buffer

Size L plus T2.b's engine change (an R01 contract, S–M), client only, after T1. The terrain's lit
pass on the cluster path, then rasterised once into IDs and shaded once a pixel (Design note 9).

- _Provides:_ `CLUSTER_COMPACT`, camera passes in the cull, the engine's ID target (an `r32uint`
  format and a target drawn with another's depth), `VisibilityBuffer`, `ResolveMaterial` and the
  terrain's, `shading`, `primitive-index` among `WANTED_FEATURES`.
- _Consumes:_ T1's interfaces; R10's lit shading (`shade_terrain`, the cascades' receiver, the
  horizon map); R08's `AtmosphereScene` and its composite; R07's internal scale.
- _Files (all of T2):_ `clusters/{compact.wgsl,visibility.ts,resolve.ts,resolveTerrain.wgsl}`,
  `terrain/terrainPass.ts`, `terrain/terrainMaterial.ts`, `terrain/materialSet.ts`,
  `terrain/shaders/terrainLit.wgsl` and `terrainLitShade.wgsl`, `engine/platform.ts`,
  `engine/types.ts`, `engine/memory.ts`, `engine/webgpu/{target,materials,drawing,resources}.ts`
  (T2.b), `photoreal/renderer.ts` (the passes' order), `engine/catalogue.ts`, tests.

- **R14.T2.a The lit pass on the cluster path, forward.** Exact; size M.
  - The stable compaction (`CLUSTER_COMPACT`, Design note 5) and a camera pass in the cull: the lit
    pass's interior patches draw through the buckets at stride 1, in the draw set's order, with the
    cluster frustum and horizon cull; edge patches keep their own draw after them. Sub-patch masks
    per instance (Design note 19). `shading: "forward"`.
  - _Tests:_ the scan's twin over random counts, both kernel paths byte for byte; the list's order
    is the draw set's; the frustum cull's twin against R05's brute-force oracle (zero false culls).
  - _Acceptance:_ captures byte-identical to T1's (tie pixels listed); lit triangles below T1's by
    the rebuilt harness's figure for spheres on 8 × 8-quad blocks (the research's 2–15% was boxes on
    4 × 4 clusters of 16 × 16 quads).
- **R14.T2.b The engine's ID target.** Size S–M, an ask of R01 (Provides, "The engine's ID target").
  - `ColourTargetFormat` gains `r32uint` (a render attachment, never blended, no mips, no
    filtering); `RenderTargetSpec.depthOf` draws a target with another's depth, loaded and stored; a
    material binds a `texture_2d<u32>` read by `textureLoad`; `DepthSelfSample` covers the shared
    depth; the target's format reaches the pipeline key (`prepareMaterial`, `materialSet.ts`).
  - _Tests:_ `materials.test.ts`, `drawing.test.ts` and `target.test.ts` for each form; each in
    `WGSL_CATALOGUE`, which test-render compiles and draws.
  - _Acceptance:_ test-render, both variants, with no capture changed (nothing draws the new forms
    yet but their own test frame).
- **R14.T2.c The ID raster.** Size M.
  - `VisibilityBuffer` on T2.b's target; the ID-only vertex stage, `@invariant`, writing the clip
    position and one flat varying (the visible-cluster index on the `primitive_index` path, since
    `instance_index` is vertex-only; the ID on the de-indexed one); `primitive-index` added to
    `WANTED_FEATURES` and used where the device has it; interior patches first with no discard, edge
    patches after with the survey discard.
  - _Tests:_ the ID's packing and its twin; the de-indexed and `primitive_index` paths give the same
    IDs on SwiftShader (test-render's both variants); every material's storage bindings a stage
    counted against 8.
  - _Acceptance:_ the ID target's depth byte-identical to T2.a's depth; the raster pass's
    inter-stage data at most 20 B a vertex.
- **R14.T2.d The resolve.** Size L.
  - The full-screen resolve with `ResolveMaterial`: a pixel whose ID is 0xffff_ffff discarded with
    no depth written; the visible cluster's template from its list entry (the cluster word's top 3
    bits); terrain's refetch through `lit_place`, barycentrics, the face normal for the cascades'
    receiver, then `shade_terrain` as the lit fragment calls it; bindings brought to 8 or fewer
    (Design note 6). No opaque pass between the raster and the resolve.
  - _Tests:_ the barycentric twin against the rasteriser's on test-render's terrain frame:
    barycentrics from the three clip positions' edge functions with perspective division, the
    reconstructed place within the hardware interpolant's own error (`f32`, a few ulps), not a
    fraction of the distance, which grazing triangles cannot meet; the resolve's output against
    T2.a's at the frame's pixels.
  - _Acceptance:_ at ≥ 99.5% of the judged terrain pixels of TERRAIN TEST's captures and
    test-render's frames the image within 1/3 stop of T2.a's, the rest listed; no change in the
    reference surface's pixels at the survey edge; the hand-over check (`--terrain --handover`)
    passes as before.
- **R14.T2.e The Mac's order, and the switch.** Size S.
  - `shading: "visibility"` on high once the owner's Mac and RTX runs are recorded; low stays
    `forward` until R12.T8.b's UHD 620 run (Design note 17). The bodies pass ordered as Apple asks
    (Design note 7): the interior terrain, then the mesh figures (which may discard), then the edge
    patches, accepted byte-identical (depth decides; ties listed). Each pass split by T2 counts its
    attachments' loads and stores in the Mac run.
  - _Acceptance (pending: the owner's runs):_ the Mac's fps on high at 1920 × 1080 at least T2.a's,
    the RTX's p95 not above T2.a's on both settings, GPU per pass recorded.

### R14.T3 Measured-error LOD and occlusion

Size L, client plus a presentation-only worker output, after T1 (and T2 where the order says).

- _Provides:_ `clusterErrors` and the measured records, strides in camera passes, `DepthPyramid`,
  `strides` and `occlusion`, the triangle budget's draft.
- _Consumes:_ T1 and T2; `BakedPatch` and the worker's bake (after `perf-terrain-bakes`);
  seamless-2's fade fields; `CoarseFineHolds`; the seamless probe; `selectPatches`' budget (T3.e).
- _Files (all of T3):_ `clusters/{errors.ts,hzb.ts,hzb.wgsl,clusterCull.wgsl}`,
  `terrain/workers/{heightBake,messages}.ts`, `terrain/coarseFineHolds.ts`,
  `terrain/terrainPass.ts`, `terrain/shaders/patchVertex.wgsl`, tests; T3.e's draft in this plan.

- **R14.T3.a The error histogram.** Size S. Over TERRAIN TEST and a served world (`owner-test`'s
  build: this tree serves no generated surface until 25), the per-cluster e₂, e₄, e₈ against each
  cluster's τ′ at R05's descent poses: the share of lit triangles that a stride within τ′ removes,
  by band and setting. It decides T3.c's gain before T3.b starts. _Acceptance:_ the table, both
  settings and both worlds, in the lane's outbox, with the harness that made it.
- **R14.T3.b The worker's error output.** Size M, after `perf-terrain-bakes`.
  - `clusterErrors` in the height worker after each bake, in `f64` from the drawn 3D offsets of both
    channels (Design note 3: on high the bake's `BakedOffsets`, on low its own `f64` positions): e_s
    per cluster, monotone, rounded up into the record's `f16`s, which the shaders read with
    `unpack2x16float` and no `enable f16`, so `assertNoF16Subgroups` still allows a subgroup
    variant; carried in `BakedPatch` beside the layers and written into the slot's records.
    Presentation only: never on the wire, in no golden.
  - _Tests:_ e_s ≥ the distance between the stride-s and the stride-1 meshes at every stride-1
    vertex, both channels at k ∈ {0, ½, 1} (brute force over wasm bakes of R10.T4.b's sample
    worlds); a flat patch on a curved datum has e_s > 0, growing as s²; monotone; the worker's
    output equals the main thread's on the same bake.
  - _Acceptance:_ the bake's time grows by at most 2% (pod, its bench).
- **R14.T3.c Strides in camera passes.** Size L, after seamless-2.
  - The cull picks the coarsest stride whose level's stated error plus e_s, projected at the
    cluster's distance, is within τ′; stride 1 in the forced region. `strides: "measured"`.
  - **The stride's transition** is a distance morph of its own, CDLOD-style and stateless: each
    vertex that the stride-2s mesh drops moves toward the stride-2s interpolant on the shared
    diagonal by k_s, from its own distance over a band before the threshold where its cluster takes
    2s. At the threshold the vertices already stand on the coarser grid, so a stride flipping there
    moves nothing, and no hysteresis is needed.
  - **Neighbours** differ by at most one stride: inside an instance through workgroup memory after a
    barrier (one workgroup an instance); across a patch edge by a second dispatch over the first's
    choices, since WebGPU gives no forward progress between workgroups. A shared vertex's k_s is
    computed alike on both sides (from both clusters' records, which the slot holds, or from the
    second dispatch's choices), and a finer cluster's edge row is held on the coarser side's line,
    as R10.T8.c's holds are between levels.
  - seamless-2's F stays the fade of a data change through `lit_place` (Design note 19). The
    seamless probe takes its strides from `cullTwin.ts`, since its counting engine sees no GPU.
  - **Fallback**, if the band costs too much of T3.a's gain: a time fade, with a per-(slot, cluster)
    state buffer (the previous stride and its change time; presentation only; one more vertex-stage
    binding, against the 8; rewritten with the slot's records and lost with the device), refining at
    τ′ and coarsening at 0.8 τ′.
  - _Tests:_ every shared vertex of two clusters at strides s and 2s computes the same position at
    every k, k_s and t, across patch edges included (the twin, over random sets); a cluster at its
    threshold places every vertex alike at s and 2s; the forced region's clusters at stride 1; the
    drawn error at every seen cluster ≤ its τ′ (item 30's oracle measure, decision-r10-item30, on
    R05's descent poses over the sample worlds' bakes).
  - _Acceptance:_ on the seamless probe at 3 × 41 ms, both settings, per-frame vertex moves from
    stride changes ≤ 1 px (high) and 2 px (low), normal changes ≤ 2°, no crack (no frame where the
    sky shows through the drawn surface in test-render's terrain frames); lit triangles as T3.a
    predicts, within 10%; the Mac's fps (pending).
- **R14.T3.d Two-pass occlusion.** Size M.
  - `DepthPyramid` and the two-pass cull of Design note 4 per view, in both the forward and the
    visibility paths. `occlusion: "hzb"`.
  - Last frame's visibility is a bit per (slot, cluster), cleared when a slot is refilled.
  - _Tests:_ the pyramid's twin (each texel the farthest of every source texel it covers, the odd
    row or column included, reversed-Z); a cluster wholly behind a nearer one is refused and one
    partly visible is kept, over random scenes.
  - _Acceptance:_ captures byte-identical with and without it, tie pixels listed (it reorders
    draws); clusters refused counted per pass; the frame's GPU time on the Mac and RTX recorded
    (pending).
- **R14.T3.e The triangle budget.** Size S to propose, M to build.
  - With strides, a frame's cost is its drawn triangles, not its patch count, so selection's budget
    (`maxPatches`, half the slots) may follow a triangle budget and the cache may grow without
    growing the frame. This changes R05 Design note 10's and R10 Design note 15's rule and
    `TERRAIN: DETAIL LIMITED`'s trigger: the task drafts the change with its counts for a ruling
    (decision-r10-t14b-constants is the rule's current ruling) and builds it only once ruled, after
    seamless-1–3.
  - _Acceptance:_ as ruled; `TERRAIN: DETAIL LIMITED` raised exactly when the drawn error exceeds τ′
    anywhere seen. With strides chosen on the GPU the CPU cannot know that, so the cull counts the
    clusters it draws over τ′ into a `bit-exact` reduction read each frame, which raises it (or the
    cull's twin decides it on the CPU).

### R14.T4 The first non-terrain consumers

Size M, client only, after T1.

- _Provides:_ `ClusterConsumer`, `meshTemplate`, the DAG-cut draw and `ClusterLod`.
- _Consumes:_ T1 (T2.a for a colour pass); R07's `smoothMeshOf` and `smoothMesh.wgsl`; R02's
  `HullOutline` and `occluder.wgsl`; R10's `terrainWire.wgsl`.
- _Files (all of T4):_ `clusters/consumer.ts`, `bodies/smoothMesh.ts`, `shaders/smoothMesh.wgsl`,
  `wireframe/{hulls,drawList,submit}.ts`, `shaders/occluder.wgsl`, `terrain/terrainPass.ts`
  (`#writeWire`), `terrain/shaders/terrainWire.wgsl`, tests.

- **R14.T4.a The smooth figure on the path.** Size M. `smoothMesh.ts`' patches through the cluster
  path's strided templates, selected as today (`selectPatches` with no level table,
  `SMOOTH_MESH_TAU_PX`). Its places are R05's `FaceDifferences` at h = 0 and read no heights, so it
  needs no page of its own. Its fragment may always discard (`disc_pixel`, wherever the body does
  not cover the pixel wholly, and beneath surveyed terrain), so its clusters are flagged may-discard
  and draw after the opaque ones (Design note 7). `ClusterConsumer` lands here. _Acceptance:_
  test-render's mesh-bodies frames (`src/renderer/src/smoke/meshBodies.ts`) byte-identical; draws
  counted.
- **R14.T4.b The generic mesh template, with the hull occluders.** Size S–M. `meshTemplate()`, a
  128-triangle cluster drawn by index pulling, and the DAG-cut mode's draw on a one-level DAG: the
  hull occluder faces (`occluder.wgsl`'s `corners`) as one page, depth-only in the wireframe and as
  the silhouette's fill over the image, depth pushed by `occluderSlopePx` as today. Index pulling
  costs three vertex invocations a triangle (Design note 2), counted. _Tests:_ the DAG cut's twin
  (own ≤ τ < parent) over random monotone DAGs selects a consistent cut. _Acceptance:_ test-render's
  hull and hidden-line frames byte-identical.
- **R14.T4.c The wireframe's terrain on the path.** Size S. The wireframe's depth-and-contour pass
  through the buckets at stride 1, with the interior split (no discard) and the cluster cull (Design
  note 13). _Acceptance:_ the wireframe's terrain frames byte-identical; triangles counted.

### R14.T5 Meshes: the builder, pages, producers and instances

Size L–XL, tool and client, after T4; each subtask waits on its content except T5.a.

- _Provides:_ the builder, `MeshPage`, `GeometryPagePool`, `ClusterResidency`, the procedural
  builder, the instance cull, the mesh `ResolveMaterial`, `geometry-pages` and `pageBytes`.
- _Consumes:_ T1–T4; R11's rocks (`RockBakeHook`, `rockMesh.ts`, `couldTouch`), pebbles and ring
  particles; R07's `body_brdf`; seamless-3's L.
- _Files (all of T5):_ `tools/meshbuild/` (its own workspace, outside `just ci`), the justfile's
  recipe for it (which makes smart-ci plan the full `just ci`),
  `clusters/{pageFormat,pages,residency}.ts`, `clusters/meshPage.worker.ts`,
  `clusters/instanceCull.wgsl`, `clusters/resolveMesh.wgsl`, tests.

- **R14.T5.a The offline DAG builder.** Size L. `tools/meshbuild/`, a native Rust tool: cluster to
  128 triangles, group, simplify with locked group borders, repeat to a root, monotone errors and
  nested LOD spheres; write `MeshPage`s. It binds meshoptimizer's clusteriser and simplifier
  (Kapoulkine, MIT, C++ through `cc`) only under these conditions, else it ports them to Rust, whose
  float results do not depend on the platform:
  - the C++ is compiled with `-ffp-contract=off` (MSVC `/fp:precise`, no `/fp:contract`) and never a
    fast-math flag, since Apple clang contracts multiply-adds into FMA on arm64 by default and GCC
    and MSVC differ again;
  - meshoptimizer's version is pinned, and its MIT notice sits beside the tool;
  - the byte test runs on all three platforms before any page is bundled: natively on Linux, on the
    owner's Mac and Windows by hand (pending; NOT RUN in a pod), since `just cross-clippy` builds
    against a stand-in compiler and runs no test.

  _Tests:_ on procedural test meshes (an icosphere with noise, a torus knot, a 10⁶-triangle terrain
  mesh): every group border watertight at every cut; errors monotone; the same input gives the same
  bytes. _Acceptance:_ `cargo test --manifest-path tools/meshbuild/Cargo.toml`, and
  `just cross-clippy` extended to the tool as it is to `gpu-replay`; at τ = 1 px a 10⁷-triangle mesh
  covering a quarter of 1080p (or 10⁶ triangles over a sixteenth, 129,600 px) draws at most 2
  triangles a covered pixel, and at most 1.2 times a hand-picked uniform cut's triangles. (A
  10⁶-triangle mesh over a quarter, 518,400 px, passes 2 a pixel whatever its cut.)

- **R14.T5.b Pages and residency.** Size M. `clusters/pageFormat.ts`, `GeometryPagePool`,
  `ClusterResidency`, the fetch-and-decode producer in a worker, `geometry-pages` and `pageBytes`.
  _Tests:_ LRU with the pins; the pool never evicts a drawn or forced page; a missing page draws its
  resident coarser group; a group's pages are evicted together; no hole and no overlap at every
  residency state of a three-level DAG (Design note 14's record rewrites). _Acceptance:_ a scripted
  fly-by of 100 instances of the test meshes shows no hole in any frame, and stays within
  `pageBytes`.
- **R14.T5.c The procedural builder.** Size M, with R11.T4. The same builder in a worker, from a
  seed (R11's rocks tessellated within R11 Design note 6's chordal bound, then clustered); the
  could-touch region's rocks at their finest cut whatever the setting (R11 Design note 7).
  _Acceptance:_ R11.T4.c's collision-agrees test holds with the rocks drawn through the path.
- **R14.T5.d Instances.** Size M, with R11.T6.a and R11.T9.d. Instance culling above the cluster
  cull (an instance's bounds first), folded dispatches beyond 65,535, a few size templates for
  particles; scatter and ring particles drawn by bucket. _Acceptance:_ 10⁶ test instances culled and
  drawn within R11's scatter row (1–2 ms on the discrete target, recorded).
- **R14.T5.e Mesh shading.** Size M. A mesh `ResolveMaterial` through R07's `body_brdf` with the
  cascades' receiver, in the visibility path, and its forward twin. _Acceptance:_ a lit test mesh's
  pixels within 1/3 stop of its forward rendering.

### R14.T6 Shadows on the cluster path

Size M, client only, after T1.

- _Provides:_ cascade reuse, far cascades at a lower rate, mesh casters on the cull.
- _Consumes:_ T1, T4.b; R10's `ShadowCasters`, `ShadowCasterProvider` and `TerrainShadows`.
- _Files (all of T6):_ `terrain/terrainPass.ts` (`#drawCasters`),
  `terrain/shadows/terrainShadows.ts`, `clusters/cullPass.ts`, tests.

- **R14.T6.a Cascades re-rendered only when they change.** Exact; size S. A cascade is not
  re-rendered while every input of its casters is unchanged since its last render, compared on the
  CPU: its snapped centre and light basis, the draw set's keys, generations and holds, the camera's
  position (which sets the casters' morph) and view axis (which sets T1.c's cull), bit for bit. The
  light basis and the instance origins are in the body's rotated frame, so with simulation time
  running they change every frame: T6.a skips only while the body's rotation and the sun's direction
  are unchanged (time paused, a still-camera check), and gains nothing in flight, where T6.b's rate
  is the lever. The interior casters take the no-discard variant (exact: the discard never fires
  there) if T1.b did not. _Acceptance:_ captures byte-identical; on a still camera with time paused,
  0 of 4 cascades re-rendered a frame after the first; during R05's descent, re-renders counted.
- **R14.T6.b Far cascades at a lower rate.** A lean; size S. Cascades 2 and 3 re-rendered every
  second or fourth frame while the sun's direction moves less than a stated share of its disc. Each
  `lightFromCamera` is camera-relative, so a cascade held from frame n − j is sampled with its own
  frame's matrix, translated by the camera's `f64` displacement since then and narrowed once;
  otherwise its shadows swim with the camera's motion. _Acceptance:_ shadow edges within one texel
  of every-frame rendering on the captures, and acne (the receivers' lit fraction) unchanged, at the
  descent's speed as well as under the sun's motion; the rate and the share recorded.
- **R14.T6.c Non-terrain casters on the cull.** Size M, after T4.b. `ShadowCasters`' providers
  become cluster consumers' cascade passes. _Acceptance:_ captures byte-identical; caster draws
  counted.
- **R14.T6.d Virtual shadow maps.** Recorded, not sized: a page-table shadow map over the same cull,
  once T6.a–c are measured.

## Verification

- **Exactness**: byte-identical captures for T1.a–c, T2.a, T3.d, T4, T6.a and T6.c against their
  bases (tie pixels listed); the bounded levers within 1/3 stop at ≥ 99.5% of terrain pixels (T1.d,
  T2.d); no mark's visibility changes (T1.e).
- **Conservativeness**: over random sets, each GPU test never refuses what its `f64` predicate keeps
  and keeps nothing it refuses beyond its widening (frustum, horizon, `castsInto`, receiver depth,
  occlusion; Design note 4).
- **The collision ground**: the forced region's clusters at stride 1, with the contacts' hold
  unchanged, in every camera, visibility and overlay pass, on both settings and in both styles
  (T3.c, T1.e); R10's collision-agrees test unchanged.
- **Determinism**: `golden_diff.py` "No golden files changed" for every task; only `bit-exact`
  kernels' results read back; the builder's bytes the same on three platforms (T5.a, under its
  build's conditions).
- **Counts** (the pod, Design note 18): triangles and draws a pass before and after each task, both
  settings, against the expectations stated per task.
- **Seamlessness**: the seamless probe's bounds for T3.c.
- **By hand, the owner's** (pending): the Mac and the RTX 3080 at 1920 × 1080, both settings,
  `just views-check --terrain --whole --hidden` and `diag-frame-cost`'s profile, after T1, T2, T3;
  the target is the owner's report's: 30 fps on high and 60 on low on the Mac, with the RTX's p95
  within R08's 15.67 ms; by eye, the descent from orbit to a metre with no new pop or crack.

## Generator version

No generated value moves, and no task bumps `GENERATOR_VERSION` (24) or `PROTOCOL_VERSION` (3).
Heights, selection, the draw set, the morph, the forced region and the materials are untouched;
strides, culls, the visibility buffer and the shading's order are presentation. T3.b's errors are
computed in the worker from a bake's heights after it, and never reach the wire or a golden. T5's
pages are assets and presentation; collision never reads them. No domain tag is added.

## Risks and open points

- **The Mac's split is unmeasured.** If `diag-frame-cost` finds the atmosphere or memory pressure
  dominant, T1 still cuts the terrain's share but the fps target moves to that lane.
- **The counts are a model's**: flat ±1 m boxes, one sun elevation, each patch's nearest point. Each
  task's acceptance is on live counts (`ClusterStats`).
- **The research's counting harness was not carried** (`tools/triCount.ts`, its JSON): T1.b rebuilds
  it on `levelBoundsCounts.ts`, and its first figures are checked against the research's table.
- **Tie pixels** (Design note 5): T1.a's split and the buckets' template order can flip pixels where
  a skirt's top row meets a neighbour's grid at equal depth. If they show, one list in the draw
  set's order by the generic template is the fallback.
- **The brief's skirt-strip triangle counts** (32 / 16 / 8 / 4) are twice the lit mesh's (16 / 8 / 4
  / 2: the feet are one winding); and its "oriented box split 8 × 8" is not conservative on the
  quadratic warp, where a block's bounds are its descendant patch's (Design note 2). Both are told
  to `geometry-clusters-1` with this plan.
- **The resolve's bindings** exceed 8 a stage as the lit material stands (10); T2.d packs them. If
  the packing costs a layout change in `TerrainResources`, it is client-only and moves nothing
  generated.
- **`primitive-index`'s availability** in Electron 45's Chromium on each backend is recalled, not
  checked; the de-indexed path stands without it.
- **No mesh content exists**: T4.b's hull occluders are the DAG-cut mode's only real test before T5,
  and the record reserves its fields from T1.b.
- **The builder's dependency** (T5.a): meshoptimizer is bound for the tool only, never shipped in
  the client, and only with contraction off, a pinned version and the three-platform byte test;
  otherwise the clusteriser and simplifier are ported to Rust (all three reviews' verdict).
- **The instance record grows** with seamless-2 (32 → 48 B): every reader goes by
  `INSTANCE_RECORD_BYTES`.
- **The triangle budget** (T3.e) changes R05's and R10's rule and the annunciation's trigger: a
  ruling first.
- **The low setting's visibility buffer** is unmeasured on the UHD 620 (R12.T8.b): low stays forward
  until then (a lean).
- **Mesh receivers and the horizon map**: a mesh in a valley at sunset takes the cascades near the
  camera but no terrain horizon beyond them (open: a horizon lookup at the mesh's ground point).
- **Not consumers**: clouds (volumetric or a 2D layer) and R11's ocean surface; the ocean's patch
  grid could take the strided templates later (open).
- **Today's path is kept** until the owner's runs sign off T1–T3; its removal is a later task.
- **Asks of other plans** (open):
  - R12: hold `GeometrySettings` in R12.T0's inventory and `SettingSnapshot`, and itemise
    `geometry-clusters` and `geometry-pages` in `MEMORY_ITEMS`.
  - R11: R11.T4's rock meshes, R11.T6.a's pebbles and R11.T9.d's particles draw through T5's
    consumers rather than paths of their own; R11.T10's stills above 4,096² tile or take the forward
    path (Design note 9).
  - The plan that builds craft (not written): hull art as meshes in T5.a's input format, beside the
    `HullOutline` the wireframe draws.
  - R05 and R10: T3.e's budget rule, by a ruling.
- **Choices this plan's review fold made**, one line each:
  - the record keeps a 32 B sphere, which the GPU tests conservatively (correctness S1); the 48 B
    box is T1.b's lane's fallback if the counts show the sphere keeping too much;
  - T3.c's stride transition is a stateless distance morph (correctness S7) rather than a time fade
    kept in a per-cluster GPU state (the draft, conformance S3): it needs no buffer, no binding past
    the 8 and nothing rebuilt after a loss, and the time fade is its fallback;
  - buckets are sized by `InstanceRecords`' capacity, the slot count (Design note 16), not
    `maxPatches` (Design note 15's draft table): overflow stays impossible with stand-ins, at twice
    the bytes;
  - the engine's ID target is a subtask of its own, T2.b (correctness B2's T2.0, completeness B1's
    T2.a′), so the visibility buffer is T2.b–e where the reviews say T2.b–d;
  - the overlay's depth stays at stride 1 unless a stride's error fits the 2⁻¹⁶ push, and T1.e has
    no ratio target (correctness S3 and conformance S1 agree; the draft's `occluderSlopePx` term was
    retired by R10.T16.c).
- **Records** go at the end of this section until the plan is split (`plan_split.py`);
  `geometry-clusters-1`'s brief asks for per-task record files, which the split makes.
- **R14.T1.a** (389371b, 13100d5b; recorded by the plan's fold from the commits, since the lane's
  tree had no plan): `TerrainViewFrame.draws` (interior first) replaces `draw`, read by
  `frameTerrain.ts` (the overlay's depth copies each draw), `spike/spikeRun.ts` and the smoke;
  `TerrainLevers` (`interior`), `ALL_LEVERS`, `NO_LEVERS` and `TerrainPass.setLevers` draw one build
  with and without a lever; `litInteriorMeshData`, `isInteriorBake`, `withoutSurveyDiscard` and the
  interior lit materials, catalogued and in `materialSet.ts`. Every terrain vertex stage's position
  is `@invariant`, its six sources re-pinned.

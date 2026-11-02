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
  row's verdict, amended (no mesh shaders and no compute rasteriser, but Nanite's cluster culling
  and level of detail taken; the roadmap's Brainstorm corrections); of "Level-of-detail consistency,
  and why collision agrees", the collision rule as the cluster path keeps it; of "Two styles of one
  renderer", both styles' terrain; of "Rings", the particles close to; of "Performance budget", the
  terrain geometry, shadows, scatter, rings and station wireframe rows, the memory table, the
  ladder's terrain detail and shadows rows and its three rules; of "Testing", "Level-of-detail
  selection", "The grounded-body rule", "Culling", "Collision agrees with what is drawn", "The
  descent" and "The performance runs".

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

The owner asked for faster streaming as well as faster viewing. The research found streaming
bake-bound (about 10 bakes a second on high against the 130–204 the descent asks), so streaming's
acceptance belongs to `perf-terrain-bakes` (patches a second on both settings against R05.T18's
asked rate) and to seamless-1–3 (the probe's bounds); this plan owns none of it, and lists what it
asks of them ([below](#running-beside-this-plan)).

## Scope and non-goals

In scope:

- **The shared path**: the cluster record, the templates, the cull passes, the cull kernel and its
  TypeScript twin, the indirect buckets, the per-pass counters; then a stable-order compaction, the
  visibility buffer and its resolve, measured-error strides, two-pass occlusion, the residency of
  mesh pages and the producers that fill them.
- **Its consumers**, in this order:

  | Consumer                                                   | Today                                                                                        | Task  | LOD mode                                    | Producer and residency                                        | Collision                                                         |
  | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ----- | ------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------------- |
  | Lit terrain: camera pass, cascades' casters, overlay depth | Instanced draws of the lit mesh a pass (10,752 triangles; interior patches 8,704 since T1.a) | T1–T3 | Strided template                            | The height workers' bakes in the patch cache's slots          | The forced region at stride 1, its hold as today (Design note 10) |
  | Wireframe terrain (depth and contours at 4 px)             | The same, depth-only with contours                                                           | T4.c  | Strided template, stride 1 (Design note 13) | The wireframe's own cache (520 slots)                         | As the lit terrain's                                              |
  | Mesh bodies' smooth figure and reference surface           | R05's patches at zero height (`smoothMesh.ts`)                                               | T4.a  | Strided template, stride 1 (T4.a)           | None: R05's `FaceDifferences` at h = 0, no page               | None                                                              |
  | Hull occluder faces (wireframe; silhouette over the image) | A few triangles, pulled by `occluder.wgsl`                                                   | T4.b  | DAG cut (one level)                         | Built at load                                                 | Never the render mesh                                             |
  | Ships, stations, surface props (authored)                  | Not built: `TEST_HULL` outlines only                                                         | T5    | DAG cut                                     | Offline builder; pages fetched and decoded into the page pool | Authored hulls, never the render mesh                             |
  | Rocks and debris (procedural)                              | Not built (R11.T3–T4 plan the rocks)                                                         | T5    | DAG cut                                     | Built from a seed in a worker; page pool                      | R11's analytic rocks (R11 Design note 6)                          |
  | Scatter, ring particles (instanced)                        | Not built (R11.T6.a, R11.T9.d)                                                               | T5.d  | Instance cull, a few templates by size      | GPU-placed instances (R11's)                                  | None (presentation)                                               |
  | Meshes as wireframe occluders                              | Not built                                                                                    | T5.f  | DAG cut at the occluder tolerance           | The page pool                                                 | None                                                              |

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
- **Clouds, the ocean's surface and the ring slab** (R11's; the slab analytic, R11.T9.c) and **the
  disc impostors** (R07's): none is a consumer here (Risks).
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
  /**
   * The lists, two `u32` an entry: the instance (with its consumer from T2.c, Design note 9) and the
   * cluster (its template in the top 3 bits); and the arguments, 20 B a bucket.
   */
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
  /** Reduces the buckets' counts into a read-back ring by a `bit-exact` kernel, never a copy. */
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
  resize(size: ViewSize): void;
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
  meshes, once and on Linux, by `just mesh-pages` into the client's bundled assets, never committed
  (the 500 kB rule; T5.a).

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
  /**
   * T1.e: the overlay's terrain depth; `scene-depth` reads the scene's where the sizes agree and
   * draws the cluster path at stride 1 elsewhere (the lean).
   */
  readonly overlayDepth: "redraw" | "scene-depth" | "cluster";
  /** T2: a forward lit pass or the visibility buffer and its resolve. */
  readonly shading: "forward" | "visibility";
  /** T3: camera passes at stride 1, or at each cluster's measured error. */
  readonly strides: "off" | "measured";
  /** T3.d: two-pass occlusion against the depth pyramid. */
  readonly occlusion: "none" | "hzb";
  /** T5: the mesh page pool's bytes. */
  readonly pageBytes: number;
  /** T5: a mesh's DAG-cut tolerance, px, in the camera pass and as a wireframe occluder (T5.f). */
  readonly meshTolerancePx: { readonly camera: number; readonly wireframeOccluder: number };
}
```

- `MemoryCategory` (`engine/memory.ts`) gains `geometry-clusters` (records, buckets, lists, the
  depth pyramid) and, in T5, `geometry-pages`; the visibility target is `render-targets`. R12's
  `MEMORY_ITEMS` (not built) groups them.

## Consumes

Re-validated on `hold/after-25` at `1fcd6c03` (2026-11-01), and again by the plan's correctness
review at `1ccd3b4f`, which corrected three paths: every name below was found at the file and with
the signature given. The `TerrainPass` row adds R14.T1.a's names as landed, and the rows from
`TIMING_FRAMES_IN_FLIGHT` on were added by the reviews' fold. Paths are under
`apps/hyperion/src/renderer/src/view/` unless given.

| Owner | Name                                                                                                                                                                                 | File                                                                                                                                   | As built                                                                                                                                                                                                                                                                                                                                          |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R01   | `RenderEngine.dispatch`, `createComputeAsync`, `createMaterialAsync`, `readBuffer`, `onPassTimes`                                                                                    | `engine/types.ts`                                                                                                                      | `dispatch(kernel, bindings, workgroups: [x, y, z] \| IndirectArgs, pass?)`, each dispatch submitted before anything else is encoded (`webgpu/engine.ts`); `readBuffer` throws `PresentationOnlyReadback` for a buffer a `presentation-only` kernel wrote                                                                                          |
| R01   | `DrawItem.indirect`, `DrawItem.instanceCount`, `IndirectArgs`                                                                                                                        | `engine/types.ts`, `engine/webgpu/drawing.ts`                                                                                          | `indirect?: { buffer, offsetBytes }`, encoded as `drawIndexedIndirect` for an indexed mesh; a draw of `instanceCount` 0 without `indirect` is skipped                                                                                                                                                                                             |
| R01   | `KernelPair`, `selectKernel`                                                                                                                                                         | `engine/kernels.ts`                                                                                                                    | `{ name, reference, subgroup: string \| null, readback: "bit-exact" \| "presentation-only" }`; the subgroup variant only where the device has `subgroups`                                                                                                                                                                                         |
| R01   | `GpuCapabilities`, `WANTED_FEATURES`, `requiredLimits`, `MAX_REQUESTED_BUFFER_BYTES`                                                                                                 | `engine/platform.ts`                                                                                                                   | Features `subgroups`, `shader-f16`, `timestamp-query`, `float32-filterable`, `float32-blendable`, `rg11b10ufloat-renderable`, `depth-clip-control`; no `indirect-first-instance`, no `primitive-index`. Storage binding and buffer sizes raised to the adapter's, at most 1 GiB; `maxStorageBuffersPerShaderStage` never raised from 8            |
| R01   | `StorageBufferSpec.stages`                                                                                                                                                           | `engine/types.ts`                                                                                                                      | A binding counts against the per-stage limit in each stage it is visible to                                                                                                                                                                                                                                                                       |
| R01   | `MemoryCategory`, `BufferSpec`                                                                                                                                                       | `engine/memory.ts`                                                                                                                     | Nine categories, `height-cache` and `shadows` among them; `{ name, bytes, usage, category }`                                                                                                                                                                                                                                                      |
| R01   | `PassTimer`, `PASSES_PER_FRAME`                                                                                                                                                      | `engine/webgpu/timing.ts`                                                                                                              | 64 timed passes a frame                                                                                                                                                                                                                                                                                                                           |
| R01   | `ResilientEngine`, `watchDeviceLoss`, `WGSL_CATALOGUE`                                                                                                                               | `engine/resilientEngine.ts`, `engine/webgpu/deviceLoss.ts`, `engine/catalogue.ts`                                                      | The rebuild after a loss; the catalogue test-render compiles and draws at every setting                                                                                                                                                                                                                                                           |
| R01   | Depth compare                                                                                                                                                                        | `engine/webgpu/materials.ts`                                                                                                           | `greater-equal` (reversed-Z): the later of two equal depths wins                                                                                                                                                                                                                                                                                  |
| R02   | `HullOutline`, `TEST_HULL`; `occluder.wgsl`                                                                                                                                          | `scene/hull.ts`, `shaders/occluder.wgsl`                                                                                               | `{ name, vertices, edges, faces, windows, eyePointM, lengthM }`; faces pulled from `corners` by `Draw.firstTriangle`, depth written from the fragment and pushed by `occluderSlopePx`                                                                                                                                                             |
| R05   | `selectPatches`, `screenSpaceErrorPx`, `selectionErrorM`, `MAX_SELECTION_VIEWS`                                                                                                      | `terrain/select.ts`                                                                                                                    | `selectPatches(input: SelectionInput): Selection`; `screenSpaceErrorPx(boundM, distanceM, view)` = ε W ÷ (2 d tan(fov_h ÷ 2)); 31 views                                                                                                                                                                                                           |
| R05   | `PatchCache`, `DrawSetResolver`, `DrawSet`, `ResidentPatch`, `CacheInsert`                                                                                                           | `terrain/cache.ts`                                                                                                                     | `insert(patch): { kind: "stored", slot, evicted } \| { kind: "refused" }`; `resolve(selection): DrawSet` (`count`, `patches`, `slots`, `views`); `{ key, generation, originM, heightRangeM, boundingRadiusM }`                                                                                                                                    |
| R05   | `patchBounds`, `PatchBounds`, `CameraRelativeBounds`                                                                                                                                 | `terrain/bounds.ts`                                                                                                                    | `patchBounds(planet, key, heightRangeM?)`, the oriented box and sphere R05 Design note 8 tests                                                                                                                                                                                                                                                    |
| R05   | `frustumOf`, `inFrustum`, `horizonCone`, `aboveHorizon`                                                                                                                              | `terrain/cull.ts`                                                                                                                      | `inFrustum(b: CameraRelativeBounds, f)`, `aboveHorizon(b, h)`: the CPU predicates the cull twins                                                                                                                                                                                                                                                  |
| R05   | `vertexSpacing`                                                                                                                                                                      | `terrain/cube.ts`                                                                                                                      | `vertexSpacing(radiusM, level): { minM, meanM, maxM }`                                                                                                                                                                                                                                                                                            |
| R05   | `INSTANCE_RECORD_BYTES`, `InstanceRecords`, `SLOT_RECORD_BYTES`                                                                                                                      | `terrain/gpu/uniforms.ts`                                                                                                              | 32 B (origin less camera, slot, morph band, parent slot, quadrant and holds); 112 B                                                                                                                                                                                                                                                               |
| R05   | `litPatchMeshData`, `LIT_PATCH_INDICES`, `INDIRECT_ARGS_BYTES`, `terrainMeshSpec`, `TerrainResources`                                                                                | `terrain/gpu/resources.ts`                                                                                                             | Lit mesh 5,135 vertices, 32,256 indices: grid 8,192 triangles, skirt feet 512 (one winding), heads 512, fins 1,536 (both windings), each edge quad's foot then head; 20 B arguments; `field(name)`, `slotRecords`, `viewBuffers(id)`, `casterBuffers(id)`, `writeCasters`                                                                         |
| R05   | `QualitySetting`, `ViewSettings`, `TerrainSettings`, `SETTINGS`, `WIREFRAME_TERRAIN_TAU_PX`, `WIREFRAME_TERRAIN_SLOTS`                                                               | `quality/qualitySetting.ts`                                                                                                            | `"high" \| "low"`; high τ 1 px, 400,000,000 B cache (1,208 lit slots); low τ 2 px at 720 rows, 64 MiB (375); wireframe τ 4 px, 520 slots                                                                                                                                                                                                          |
| R05   | `GroundContact`, `FORCED_REGION_RESIDENCY_S`, `inForcedRegion`                                                                                                                       | `terrain/grounded.ts`                                                                                                                  | 30 s, provisional                                                                                                                                                                                                                                                                                                                                 |
| R05   | `finest_surface_height`; `PatchBake::height_range_m`                                                                                                                                 | `crates/hyperion-surface/src/patch/collision.rs`, `patch.rs`                                                                           | The collision interpolant on the mesh's own diagonal; the range covers both height channels, own and parent's                                                                                                                                                                                                                                     |
| R05   | `defaultHeightWorkers`                                                                                                                                                               | `terrain/workers/pool.ts`                                                                                                              | clamp(⌊hardwareConcurrency ÷ 4⌋, 1, 3)                                                                                                                                                                                                                                                                                                            |
| R07   | `smoothMeshOf`, `SmoothMesh`, `SmoothPatch`, `MAX_SMOOTH_MESH_PATCHES`, `SMOOTH_MESH_TAU_PX`; `smoothMesh.wgsl`                                                                      | `bodies/smoothMesh.ts`                                                                                                                 | R05's patches at zero height, selected by `selectPatches` with no level table; 1,024 patches; 0.25 px                                                                                                                                                                                                                                             |
| R07   | `ViewSettings.internalScaleBounds`                                                                                                                                                   | `quality/qualitySetting.ts`                                                                                                            | [0.5, 1] on both settings: the scene target resizes with it                                                                                                                                                                                                                                                                                       |
| R08   | `AtmosphereScene`; the composite                                                                                                                                                     | `atmosphere/AtmosphereCache.ts`, `atmosphere/shaders/composite.wgsl`                                                                   | The composite reads the scene target's colour and reversed-Z depth and lays aerial perspective over every opaque pixel                                                                                                                                                                                                                            |
| R08   | `aerialPerspectiveScope`, `HillaireAtmosphere.aerialPerspective`                                                                                                                     | `atmosphere/hillaire.ts`, `atmosphere/settings.ts`                                                                                     | `scene` (high): every opaque pass drawn into the scene before the composite, and the volume published to later passes; `terrain` (low): the composite still covers every opaque pixel, and nothing is published (`aerialPerspective` returns `null`)                                                                                              |
| R10   | `TerrainPass` (`#write`, `#drawCasters`, `#casterItems`, `#drawItem`, `maxPatches`, `symbologyDepthMaterial`); since R14.T1.a `TerrainViewFrame.draws`, `TerrainLevers`, `setLevers` | `terrain/terrainPass.ts`                                                                                                               | Lit draws a view, interior patches first (R14.T1.a; read by `frameTerrain.ts`, `spike/spikeRun.ts` and the smoke); casters as four `instanceCount` draws of the lit mesh from per-cascade copies of the instance records; `maxPatches` half the slots; `InstanceRecords` sized by the slot count                                                  |
| R10   | `castsInto`, `morphRangeM`, `effectiveTauPx`, `MORPH_START_FRACTION`                                                                                                                 | `terrain/terrainPass.ts`                                                                                                               | `castsInto(set, index, fromCameraBodyFixedM, radiusM): boolean`; `effectiveTauPx(tauPx, limitExcess, weight)` = τ max(1, excess ÷ w); 0.7                                                                                                                                                                                                         |
| R10   | `CascadeSet`, `fitCascades`, `CASCADE_SIZE_PX`, `CASCADE_COUNT`                                                                                                                      | `terrain/shadows/cascades.ts`                                                                                                          | `{ rangeM, splitsM, weight, unclippedDepth, casterReachM, sunElevationRad, basis, cascades }`; 2,048; 4                                                                                                                                                                                                                                           |
| R10   | `cascadeAt`, `CASCADE_FOOTPRINT_TEXELS`, `cascadeReceiverLit`                                                                                                                        | `terrain/shadows/cascadeShadow.ts`                                                                                                     | `cascadeAt(distanceAlongViewM, farM): CascadeIndex \| null`; the receiver's twin                                                                                                                                                                                                                                                                  |
| R10   | `ShadowCasters`, `ShadowCasterProvider`, `TerrainShadows`                                                                                                                            | `terrain/shadows/terrainShadows.ts`                                                                                                    | `(set, index, view) => DrawItem[]`: the casters that are not terrain                                                                                                                                                                                                                                                                              |
| R10   | `lit_place`, `survey_kept_at`; `LitVertexOut`; the lit fragment                                                                                                                      | `terrain/shaders/terrainSurvey.wgsl`, `terrainLit.wgsl`, `terrainLitShade.wgsl`                                                        | `lit_place(grid : vec3f, instance : u32) -> LitPlace`; 12 inter-stage locations (92 B a vertex with the position); the geometric normal from `dpdxFine` before the survey `discard`; tiles read by `textureLoad`, cascades by `textureSampleCompareLevel`                                                                                         |
| R10   | `TerrainLayers.unsurveyedCells`                                                                                                                                                      | `terrain/workers/messages.ts`                                                                                                          | `Uint32Array`, empty on a wholly surveyed bake                                                                                                                                                                                                                                                                                                    |
| R10   | `LiveFrameTerrain.symbologyDepth`; `TERRAIN_DEPTH_FRACTION`                                                                                                                          | `terrain/frameTerrain.ts`; `depth/groundPull.ts`, `terrain/shaders/terrainDepth.wgsl`                                                  | `symbologyDepth(): ReadonlyArray<DrawItem>`, the terrain redrawn as depth before the marks, each depth pushed by 2⁻¹⁶ of itself with no slope term (R10.T16.c); the survey edge pulls its own depth (`shaders/lineStroke.wgsl`'s `line_depth`); only the hull occluders keep `occluderSlopePx` (`photoreal/overlay.ts`, `terrain/terrainPass.ts`) |
| R10   | `CoarseFineHolds`, `coarseFineMorph`                                                                                                                                                 | `terrain/coarseFineHolds.ts`                                                                                                           | k held at 0 or 1 on coarse–fine edges and corners, from the draw set                                                                                                                                                                                                                                                                              |
| R10   | The terrain run; the smoke harness's terrain frame                                                                                                                                   | `src/renderer/src/displays/view/check/terrainCheckRun.ts`, `src/main/terrainCheckResults.ts`, `src/renderer/src/smoke/terrainFrame.ts` | `just views-check --terrain [--whole] [--hidden] [--software] [--setting high\|low]`; test-render's `R10.T16 the frame` group (part 7)                                                                                                                                                                                                            |
| R10   | `levelBoundsCounts.ts`                                                                                                                                                               | `terrain/levelBoundsCounts.ts`                                                                                                         | The count model the research's harness was built on                                                                                                                                                                                                                                                                                               |
| R11   | `RockBakeHook`, `rockMesh.ts`, `couldTouch`; `pebbles.wgsl`, `scatterCull.ts`; `ringParticles.wgsl`, `instancedShare`                                                                | planned (R11.T4, T6, T9.d), not built                                                                                                  | T5's consumers; names as R11 has them                                                                                                                                                                                                                                                                                                             |
| R01   | `TIMING_FRAMES_IN_FLIGHT`; `WriterRecord.copied`; `ColourTargetFormat`, `RenderTargetSpec`, `DepthSelfSample`                                                                        | `engine/webgpu/timing.ts`; `engine/webgpu/readback.ts`; `engine/types.ts`                                                              | 135, sized on 15 resolves a photorealistic frame (decision-r07-t19); a copy carries its source's writer; `"rgba16float" \| "rg11b10ufloat" \| "rgba8unorm"`, one colour attachment and a `depth32float` of its own (T2.b grows them)                                                                                                              |
| R05   | `chordSagittaM`; `SELECTION_MARGIN`; `PACKED_BOUNDS_LENGTH`; `MAX_LEVEL`                                                                                                             | `terrain/select.ts`; `terrain/selectionTolerance.ts`; `terrain/bounds.ts`; `terrain/patchKey.ts`                                       | A level's chord sagitta, m; 0.1; 18 numbers a packed box; 24                                                                                                                                                                                                                                                                                      |
| R05   | `SlotUpload`                                                                                                                                                                         | `terrain/gpu/resources.ts`                                                                                                             | `{ slot, key, heights, offsets, normals, originHeightM, skirtDepthM, layers }`: heights own and morph target interleaved; offsets q₀ and q₁ on high's `baked-offsets` layout, else `null`                                                                                                                                                         |
| R11   | `STILL_TILE_MAX_PX`                                                                                                                                                                  | `stills/stillPlan.ts`                                                                                                                  | 4,096 px high, 2,048 low: stills render tile by tile                                                                                                                                                                                                                                                                                              |
| R14   | `bytesDigest` (R14.T1.a)                                                                                                                                                             | `src/renderer/src/smoke/harness.ts`                                                                                                    | The FNV-1a of a read-back's bytes: a frame's fingerprint in test-render's log                                                                                                                                                                                                                                                                     |
| R12   | `SettingSnapshot`, `MEMORY_ITEMS`, `MemoryItem`                                                                                                                                      | planned (R12.T1–T3), not built                                                                                                         | Read this plan's settings and categories (asks, Risks)                                                                                                                                                                                                                                                                                            |

The research's counting harness (`tools/triCount.ts`) and its JSON were not carried with its
records; T1.b commits its successor on `levelBoundsCounts.ts` (Design note 18).

## Running beside this plan

| Lane                                                            | Task IDs             | What it changes                                                                                                                                                                                                                                                                           | What this plan takes, and the order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| --------------------------------------------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `geometry-clusters-1` (running)                                 | R14.T1.a–e           | This plan's T1: T1.a landed (389371b, 13100d5b), T1.b–e in flight                                                                                                                                                                                                                         | T1 is that brief's five steps as this plan corrects them, and where the two differ this plan wins. Main told the lane on 2026-11-01: the skirt strips' counts and the L + 3 bounds (Design note 2); `@invariant` on every terrain vertex stage (landed as 13100d5b, Design note 5); the sphere-or-box record (Design note 4); T1.e's overlay acceptance, with no ratio target; `GeometrySettings` and the way back (Design note 17); seamless-1's stand-ins on the edge path (Design note 19). Its brief also differs on the counters (a `bit-exact` reduction, never a copy, Design note 5), on tie pixels (listed) and on records (inline in Risks), and its "the resolve takes the survey discard" is ruled out (Design note 9). Whatever the lane lands without is R14.T1.f |
| `seamless-1-no-collapse` (running)                              | R10's records        | `PatchCache.insert` refuses rather than evict the selection's chain; perhaps a sub-patch stand-in (A's slot and a sub-rectangle); the seamless probe as a slow test; the 253 s flicker. seamless-2 is chained after it in its lane                                                        | The refusal binds the page pool too (Design note 14). A sub-patch instance draws on the edge path until T2.a, and from T2.a as a mask of A's blocks only at a level offset of 1–3 that it draws as A's own blocks (Design note 19). Its probe takes T3.c's strides from `cullTwin.ts`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `seamless-2-time-morph` (after 1, in its lane)                  | R10's records        | k = max(k_CDLOD, F(v, t)) from per-instance fades; the instance record may grow by a `vec4u`; the bands' τ′ slewed 2% a frame                                                                                                                                                             | F comes through `lit_place`, which the cluster path calls; nothing reads 32 B for the record (`INSTANCE_RECORD_BYTES`); the forced flag's bit 18 is to be agreed with its spare bits (Design note 10). While a rebake fades, the cull keeps the union of the old and the new slots' bounds (Design note 4). T3.c's strides morph by distance, not by F, at the unslewed τ′ (Design note 3). T6.a follows it. T2's resolve is the cheap home of a later dithered fade                                                                                                                                                                                                                                                                                                            |
| `seamless-3-lookahead` (running beside 1; rebases when 1 lands) | R10's records        | A look-ahead selection at weight 0.5; L, the demand-to-resident p50, as a getter                                                                                                                                                                                                          | T5's mesh requests read L for their prefetch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `perf-terrain-bakes` (running)                                  | R10.T6.a, R05.T18    | The bake's cost and the bakes a second (workers, SIMD, shared normals); the progressive bake on high (research-terrain-gpu-driven §6's worker lane: everything but the double normals first, drawn at once, the slot byte-identical in the end), added to its brief by main on 2026-11-01 | Streaming's acceptance. T3.b's error output is computed beside the bake after this lane lands, in the worker, from the bake's heights. An ask: on Apple Silicon the height workers' count by performance cores, not `navigator.hardwareConcurrency`, which counts the efficiency cores                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `diag-frame-cost` (running)                                     | (the owner's report) | The Mac's frame breakdown, its profile command, everything but the terrain's triangles (main told it so on 2026-11-01); perhaps an automatic quality floor                                                                                                                                | Its per-pass split and the owner's Mac run decide T2.b–e against T3.c–d after T2.a (Tasks); its profile command is a by-hand run of every task here. If its floor lands, it steps `GeometrySettings` with the setting or leaves them at the setting's values; a change of the patch count at orbit is T3.e's ruling's                                                                                                                                                                                                                                                                                                                                                                                                                                                           |

**Concurrent edits.** `geometry-clusters-1` and seamless-1 and seamless-2 all edit `terrainPass.ts`'
`#write`, the instance record and the terrain shaders, and seamless-2 lands while T1 is in flight:
whichever lands second rebases. The cluster path's vertex stage reads the instance through the WGSL
`Instance` struct and `INSTANCE_RECORD_BYTES`, never a literal, and seamless-2's fade fields only
through `lit_place`.

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
   tolerance: for a camera pass τ′ px, the setting's τ × max(1, `limitExcess` ÷ w), unslewed
   (`effectiveTauPx(τ, limitExcess, w)`) and evaluated each frame at the cluster's own distance,
   which is neither selection's τ_sel = τ ÷ (1 + `SELECTION_MARGIN`) nor seamless-2's slewed band τ′
   (today `effectiveTauPx` is called with the bands' `bandTauPx`); for a cascade its texel, 2 r ÷
   2,048 m, the finest detail the map resolves (T1.d). The overlay's depth has no slope term to
   spend: since R10.T16.c's grazing fix its terrain depth is pushed only by
   `TERRAIN_DEPTH_FRACTION`, 2⁻¹⁶ of the depth (about 0.15 m at 10 km, 4.4 m at 287 km), and the
   survey edge pulls its own depth, so the overlay's pass stays at stride 1 (T1.e; from T3.c's
   strides, T3.c's "The overlay's depth"). Two modes share
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
     ground, and T3.a measures it. decision-r10-item30 measured the stated (hard) bound at 8–200
     times the drawn error, and selection draws each leaf where its stated error is near τ′ at its
     nearest point, so this rule leaves few strides outside a patch's far side. A rule on the
     measured per-patch deviation instead would change R10 Design note 5's stated-bound rule, so
     T3.a reports the gain under both, and T3.c builds the stated rule unless a ruling, with T3.e's,
     adopts the measured one.

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
   - **A rebake's fade** (seamless-2): the old bake is held in a spare slot for T while the new one
     draws with its heights mixed from the old by f(t), and the new slot's records bound only the
     new bake's two channels. So while an instance fades from an old bake, its clusters are culled
     at the union of the old and the new slots' records (or the cull keeps every cluster of a fading
     instance), and the twins test a rebake mid-fade; otherwise every exact lever would miss for up
     to 0.4 s a rebake.
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
     has it. Each new vertex stage here carries it too. The same holds for every non-terrain stage
     that a byte-identity acceptance compares. The vertex stages of `shaders/smoothMesh.wgsl` and
     `shaders/occluder.wgsl` are not `@invariant` today, so T4.a and T4.b each first mark their base
     stage's position `@invariant`, re-pinned, in a commit of its own, and take their base captures
     after it.
   - Depth-only passes (casters, the overlay's depth) compact with `atomicAdd` appends. Their result
     is order-independent: the final depth is the nearest whatever the order, and their discards are
     per fragment. Such a cull is `presentation-only`, and its lists never reach the CPU.
   - A colour pass needs the draw set's order. From T2.a a stable compaction (per-instance counts,
     an exclusive scan, a scatter) writes each bucket in instance order. WebGPU gives no forward
     progress between workgroups, so no single-pass decoupled look-back: a reduce-then-scan in two
     or three dispatches, integer throughout, so its subgroup variant is `bit-exact` with its
     reference.
   - The counters are integer sums, read through a `bit-exact` reduction into a ring
     (`ClusterStats`), behind the measurement switch only, and never copied: a buffer copy keeps the
     cull's `presentation-only` mark (R01's `WriterRecord.copied`, `engine/webgpu/readback.ts`),
     which `readBuffer` refuses, and `RenderEngine` has no buffer copy anyway. The reduction may
     read arguments that the `presentation-only` cull wrote because integer `atomicAdd` counts do
     not depend on the order of the adds; only the lists' order is irreproducible.
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
   - So T1.a draws the interior patches, with no discard, before the edge patches; T1 cuts a quarter
     to a half of the triangles (T1's table); T2 leaves the raster pass only a position and a flat
     ID; every task's acceptance includes the Mac's fps.
   - The bodies pass is not yet opaque first: it draws the mesh bodies' figures, which may discard
     (`smoothMesh.wgsl`), before the terrain (`photoreal/renderer.ts`). T2.e orders it as interior
     terrain, then the mesh figures, then the edge patches (depth decides; ties listed).
   - **Pass splits cost a tiler its attachments**: each extra render pass stores and reloads them,
     at 1080p about 25 MB of `rgba16float` and `depth32float` each way. T2 splits the bodies pass
     into raster and resolve, and T3.d again around the pyramid's build, so each task that splits a
     pass counts its attachments' loads and stores in its Mac run.
   - The Mac's own split is unmeasured: 1 fps is 5–20 times what linear scaling of the RTX's frame
     predicts, so `diag-frame-cost`'s profile decides how much is terrain at all (Risks).
   - **The Mac as a machine of record**: the roadmap's target hardware gains an entry for the
     owner's Mac, filled before T1's first owner run (its chip and GPU cores, unified memory, macOS,
     display and device-pixel ratio, and Electron), and each Mac record names it. It is a by-hand
     machine from the owner's report (2026-11-01: a planet with terrain and atmosphere at 1 fps or
     lower; the target 30 fps on high and 60 on low at 1920 × 1080), so a miss is a finding. Its
     memory ceiling is the discrete target's until measured: a resident peak above 2 GB a finding
     (lean).
   - `navigator.hardwareConcurrency` counts efficiency cores, so the height workers' count on Apple
     Silicon is asked of `perf-terrain-bakes`, by performance cores; and the unified memory makes
     every byte of Design note 15 system memory, the page pool's included.

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
   - **No machine runs D3D12**: the roadmap's target hardware is Linux only (the RTX 3080 on Vulkan,
     the UHD 620 on ANV) beside the owner's Mac, so every task lists Windows under NOT RUN (no
     hardware) and holds it to builds and lints (`just cross-clippy`, T5.a's tool included).
   - **Timers**: off Linux, R12.T0's audit finds Dawn's `timestamp_quantization` left on, so
     per-pass times on the Mac and Windows are quantised; fps comes from frame intervals.

9. **The visibility buffer (T2).** The lit pass splits in two:
   - **The raster**: the cluster path's buckets draw into the scene's depth and an `r32uint` ID,
     (visible-cluster index << 7) | triangle, 0xffff_ffff for none (reserved from the index space),
     cleared each frame. The engine draws neither today: T2.b adds the `r32uint` target and a target
     drawn with another's depth (Provides). The vertex stage writes only the clip position and one
     flat varying: the visible-cluster index on the `primitive_index` path, the ID on the de-indexed
     one. The interior patches draw with no discard. The edge patches draw the lit mesh, not
     clusters, so they write no ID. They draw forward in the resolve's render pass, after it, with
     the lit material and its survey `discard`, depth-tested against the raster's depth. An
     unsurveyed fragment so still writes no depth (it would hide R07's reference surface beneath),
     and the survey test still cannot move to the resolve (a correction of the research's phase 2).
     A `terrain-fin` shape (Design note 18) would bring them onto the path.
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
   - **Several consumers, one buffer**: the visible-cluster index addresses one list a view shares
     across every consumer, each entry (consumer, instance, cluster) in two `u32` words. No single
     shader holds every consumer's bindings beside the terrain's, so the resolve runs one
     full-screen pass for each consumer class present, each refusing the other classes' pixels (by a
     tile classification, or a depth-equal test on a class-depth target), each within 8 storage
     bindings a stage. T2.c fixes this layout, so that T5.e adds a pass, not a format. R11's planned
     additions to terrain shading (the cloud layer's shadow, the rings' shadow on the planet, the
     decoration's albedo) each land within the terrain resolve's 8 (lean: as textures or uniforms,
     not storage buffers).
   - **Order with other passes**: no opaque pass draws between the raster and the resolve; passes
     drawn before the raster write depth that the raster's test respects, and passes drawn after the
     resolve depth-test against the terrain's depth.
   - **What it removes**: the 2–4 shadings a pixel that 1–3 px² triangles cost in 2 × 2 quads, the
     92 B of varyings in the tiler, and the discard on interior patches.
   - **Sizes**: the ID target follows the scene target's size, R07's internal scale included. R11
     already renders stills tile by tile at `STILL_TILE_MAX_PX` (4,096 high, 2,048 low), so the ID
     target stays within 64 MiB (4,096² × 4 B), and the depth pyramid and each resolve pass follow
     the tile's size.
   - **Fallback**: the forward cluster pass (T2.a), the setting's `shading: "forward"`.

10. **Determinism and the collision ground.** No generated value moves: heights, selection, the draw
    set, the morph, the forced region and every golden are untouched.
    - **The forced region**: no camera, visibility or overlay pass takes a stride above 1 for a
      cluster of a patch in the forced region, and the contacts' hold is unchanged (k = 0 within
      each held radius, rising across one finest patch beyond it), so the drawn ground under every
      grounded or descending body is `finest_surface_height`'s interpolant where it was before (R05
      Design note 6, R10 Design note 4), on every setting and in both styles. Casters may stride
      there: a shadow is not the ground.
      - The instance record has no field for it: the flag is bit 18 of `parentQuadrant` (bits 0–17
        are the quadrant and the holds, 18–31 free; seamless-2's fade may take the record's spare
        bits, so the bit is agreed with that lane before either lands), written by `#write` from
        `inForcedRegion` and masked by every reader.
      - The hold ramps across one finest patch beyond r_g, so the cull also holds stride 1 for any
        cluster whose bounds meet a contact's r_g plus that ramp, flagged patch or not.
      - Every DAG-cut consumer draws its finest cut within any contact's could-touch region, on
        every setting and in both styles: R11's rocks (T5.c) and an authored mesh in contact (a ship
        on a pad, a docking port) alike.
      - T3.c adds a test-render frame with a grounded body whose pixels under the contact are
        byte-identical to the base's.
    - **Twins**: the cull, the templates, the strides' errors, the compaction and the resolve's
      barycentrics each have a TypeScript twin tested in Vitest, as `vertexEmulation.ts` and
      `cascadeShadow.ts` are.
    - **Readback**: only `bit-exact` results reach the CPU (R01 Design note 16): the counters, never
      the atomic lists.
    - **Captures**: an exact lever's acceptance is byte-identical captures in test-render and the
      terrain run against the task's base build, compared in the lane and never stored (the
      brainstorm's "no golden images").
      - Exact levers compare frame fingerprints: test-render logs each terrain frame's `bytesDigest`
        (T1.a built it in `smoke/harness.ts`), and one build draws a frame with and without its
        lever (`TerrainPass.setLevers`). In a pod the exact levers compare part 7's lit groups in
        one build by `setLevers`, as T1.a's do: R10.T9.b's cascades (`smoke/sharedPass.ts`) and
        R10.T16's frame (`smoke/terrainFrame.ts`). R05.T11.c's captures are R05's spike shading,
        which keeps today's path, and R10.T10.c's limb captures draw `TerrainResources` without
        `TerrainPass`, so neither shows a lever. Streamed frames vary from run to run (13100d5b's
        grazing frames), so no exact lever compares them across builds. TERRAIN TEST's captures are
        the owner's RTX run's, since the terrain run does not finish on SwiftShader (Design note
        18).
      - Bounded levers compare pixels: T1.d adds `scripts/compareCaptures.mjs` (by hand, as
        `levelCounts.mjs` runs), which takes two directories of `just test-render --captures=DIR`
        frames and reports each frame's differing pixels, their largest difference in stops, and the
        share within 1/3 stop over the frame's terrain pixels (a terrain mask captured beside each
        frame). Its comparisons hold the exposure fixed (`MAN`, or the base's metered value
        replayed), since auto exposure's histogram moves every pixel when the terrain's move.
    - **Meshes** (T5): the offline builder is deterministic (the same input gives the same bytes, on
      every platform); the procedural builder draws from a seed in a worker; collision never reads a
      render mesh (authored hulls; R11's analytic rocks).
    - `golden_diff.py` says "No golden files changed" for every task.

11. **The atmosphere's aerial perspective.** R08's composite reads the scene's colour and reversed-Z
    depth and lays aerial perspective over every opaque pixel, on both settings, with no terrain
    mask (`atmosphere/shaders/composite.wgsl`), so the visibility buffer writes the same depth into
    the same target and the composite is unchanged. A mesh consumer draws in R07's `bodies` slot,
    before the composite, on both settings (R07's pass list is one order for both,
    `photoreal/passes.ts`), and takes its aerial perspective as the mesh bodies do. The `terrain`
    scope's "terrain only" on low is what the composite publishes to later passes: nothing
    (`HillaireAtmosphere.aerialPerspective` returns `null`). A translucent consumer, none yet, reads
    that published volume on high.

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
    occluders' depth (the hulls in T4.b, the DAG consumers' in T5.f), at a tolerance under the
    strokes' half-width (`meshTolerancePx.wireframeOccluder`) so that hidden-line removal holds; at
    a coarse cut an authored mesh's silhouette would move against `HullOutline`'s edges, which T5.f
    accepts against the finest cut. Meshes write no overlay depth: the hulls' occluder faces (T4.b)
    hide a mark behind a craft or a station as today, and rocks and scatter hide none (lean).

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
    - **Where demand comes from**: the DAG cut is made on the GPU, but a `presentation-only`
      kernel's output never reaches the CPU (`readBuffer` throws `PresentationOnlyReadback`), so
      Nanite's GPU feedback is closed. The demand is the CPU's, for each instance: from its bounds
      and the error span of each of its DAG's levels, the coarsest level whose error projects within
      τ at the instance's nearest point, whose pages and their ancestors it requests, scored as
      above. If that proves too coarse, a `bit-exact` request kernel (a bitmask of pages a frame,
      OR-reduced) is the alternative, ruled first.
    - **Bounded pins**: R11's rocks are unique for each seed, so every distinct rock would pin a
      root of its own. Procedural decoration is instanced from a bounded set of prototypes a biome
      (lean: 64 DAGs, each instance with its own scale and rotation); unique meshes are only the
      could-touch rocks, which collision reads analytically. Pinned roots stay within 10% of
      `pageBytes` (lean); past it a request is refused and annunciated as the terrain cache's is.
    - **The page table** earns its place only where a cluster reads pages other than its own
      (normals from a finer resident page, a mesh's shared vertex pages); until then a slot index is
      the address, as today.

15. **Memory budgets per setting.** Estimates from formats and sizes, to be measured by each task
    (R12 Design note 6's tally reads the categories):

    | Item                                            | High                                                                                 | Low                                                                                               | Category            |
    | ----------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- | ------------------- |
    | Cluster records, terrain                        | 3.7 MB (1,208 slots)                                                                 | 1.2 MB (375)                                                                                      | `geometry-clusters` |
    | Cluster records, wireframe terrain              | 1.6 MB (520 slots)                                                                   | 1.6 MB                                                                                            | `geometry-clusters` |
    | Buckets and arguments, a lit view (T1)          | ≤ 15.8 MB (4 cascades × 4 strides, the overlay at stride 1)                          | ≤ 0.3 MB (the overlay's)                                                                          | `geometry-clusters` |
    | The same with T2.a's scan                       | ≤ 5.6 MB (6 passes, one list each)                                                   | ≤ 0.6 MB (2 passes)                                                                               | `geometry-clusters` |
    | Visibility IDs, a view (T2)                     | 8.3 MB at 1920 × 1080                                                                | 3.7 MB at 1280 × 720                                                                              | `render-targets`    |
    | Depth pyramid, a view (T3.d)                    | 2.8 MB (from half resolution)                                                        | 1.2 MB                                                                                            | `geometry-clusters` |
    | Mesh page pool (T5)                             | 128 MiB (lean)                                                                       | 32 MiB (lean)                                                                                     | `geometry-pages`    |
    | The pool's CPU-side copy                        | None (lean: refetched after a loss)                                                  | None                                                                                              | —                   |
    | Buckets, a wireframe view (T4.c)                | 0.4 MB (one pass at stride 1, 520 slots)                                             | 0.4 MB                                                                                            | `geometry-clusters` |
    | Today's path while kept (Design note 17)        | About 0.4 MB (four per-cascade instance copies, 4 × 1,208 × 32 B, and the lit mesh)  | About 0.2 MB                                                                                      | As today            |
    | **A lit primary and two wireframe instruments** | 21.9 MB with T1 (5.3 + 15.8 + 0.8); 22.7 MB from T2–T3 (5.3 + 5.6 + 8.3 + 2.8 + 0.8) | 3.8 MB with T1; 4.1 MB forward from T2.a, 5.4 MB with `hzb`, 9.0 MB were the visibility buffer on |                     |

    Buckets are sized by `InstanceRecords`' capacity, the slot count (1,208 high, 375 low), times 96
    clusters and 8 B an entry, so no draw set can overflow them (Design note 16). Before T5's pool,
    high adds about 5.3 MB shared and 16–17 MB a lit view (T1's buckets 15.8 MB; from T2.a the
    lists, the IDs and the pyramid, 16.7 MB), against a lit slot cache of 400 MB and the
    brainstorm's ceilings (a resident peak above 2 GB a finding and above 3 GB a failure on the
    discrete target, 1 GB on the low setting). Several views multiply the per-view rows; a wireframe
    instrument has no cascades, visibility IDs or pyramid. On unified memory (the Mac) every row is
    system memory. A second window's device duplicates all of it (Risks). Each task records its
    categories' bytes from the allocation events against its rows.

16. **Failure and loss handling.**
    - **Device loss**: `ResilientEngine` rebuilds the engine and `TerrainPass`'s device record is
      made again (`#rebuild`), which makes a new cache and pool with nothing resident: after a
      restore every patch is baked again, and each record is written as its slot fills. The cluster
      resources belong to that record, so the buckets and targets are made again and the kernels
      created again. The page pool keeps no CPU-side copy (lean) and refetches its pages. Each task
      that adds a cluster resource adds a restore test beside `terrainPassCascades.test.ts`'s: after
      a restore on the cluster path the buckets, records and kernels are made again, and the first
      drawn frame's items equal a fresh pass's.
    - **A refused kernel or material** (`GRAPHICS SHADER REFUSED: <displayName>`): the pass falls
      back to today's path for that consumer from that frame on, and states it in its fault, as a
      terrain fault does today. From T7, which retires that path, it faults its consumer instead
      (T7).
    - **Overflow** is impossible by construction: a bucket's capacity is the consumer's whole
      cluster count (terrain: `InstanceRecords`' capacity, its slot count, × the template's clusters
      an instance, and from T2.a 16 interior-edge strips an instance drawn as a sub-patch mask,
      Design note 19). T5's pool sizes its lists from its slots.
    - **No timestamps, no subgroups, default limits**: Design note 6's fallbacks.

17. **The settings ladder R12 holds, and the way back.** `GeometrySettings` is a field of R05's
    `ViewSettings`, as R12 Design note 3 asks of every plan, with both settings built together (the
    budget's third rule). R12.T0 inventories it; R12.T11 may move its values within this policy,
    never the policy:

    | Field             | High                                                                       | Low                                                                        | Policy                                               |
    | ----------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------- |
    | `clusterPath`     | on                                                                         | on                                                                         | Every consumer on the path; off is today's path      |
    | `casterDepthCull` | on                                                                         | on (no cascades on low)                                                    | Exact                                                |
    | `casterStrideCap` | 8                                                                          | 8 (no cascades on low)                                                     | Casters at their texel                               |
    | `overlayDepth`    | T1.e's choice (lean `scene-depth`); `cluster` once `strides` is `measured` | T1.e's choice (lean `scene-depth`); `cluster` once `strides` is `measured` | No mark's visibility changes                         |
    | `shading`         | `visibility` (after T2.e)                                                  | `forward` until the owner's UHD 620 run (lean, T2.e)                       | One shading a pixel where it pays                    |
    | `strides`         | `measured` (after T3.c)                                                    | `measured`                                                                 | Drawn error within τ′; stride 1 in the forced region |
    | `occlusion`       | `hzb` (after T3.d)                                                         | `hzb` (after T3.d; the UHD 620's run may turn it off)                      | Exact                                                |
    | `pageBytes`       | 128 MiB                                                                    | 32 MiB                                                                     | Within the memory ceilings                           |
    | `meshTolerancePx` | camera 1, wireframe occluder half the stroke width (lean)                  | camera 2, the same occluder rule (lean)                                    | A mesh's DAG cut within τ; hidden-line removal holds |
    - Nothing here moves a figure the brainstorm states (τ, 720p, 30 fps, the 4 px wireframe
      tolerance): strides stay within τ′, and `TERRAIN: DETAIL LIMITED` keeps its trigger, the
      selection's budget binding; if T3.e lets a triangle budget bind beyond τ′, that raises it too.
    - **The way back**: today's path stays in the tree, compiled and drawn by test-render at every
      setting, until the owner's Mac and RTX runs sign off T1–T3 (Verification); its removal is
      R14.T7. `GeometrySettings` is fixed per setting in `SETTINGS`, and a run chooses only
      `--setting high|low`, so the way back is made runnable:
      - the settings reach the passes as their levers: T1.a built `TerrainLevers` and
        `TerrainPass.setLevers` (`interior` only), T1.b makes `GeometrySettings` their source, and
        `clusterPath: false` draws as before this plan (`NO_LEVERS`);
      - T1.b adds a development override, `--geometry <field>=<value>[,…]`, to `just views-check`,
        `just descent-spike` and the smoke page, applied over `SETTINGS[s].geometry` and printed on
        each run's first line, so that the owner's before and after runs come from one build;
      - test-render's terrain part draws once more with `clusterPath=false` (a group in part 7);
      - the fallback on a refused kernel (Design note 16) is tested with test-render's
        `--fixture=broken-wgsl`: today's path drawn, and the fault stated (from T7, the fault
        alone);
      - each consumer that moves (T4.a, T4.b, T4.c, T6.c) keeps its old draw behind the same field
        until the owner's runs sign it off.
    - **At run time** `GeometrySettings` changes only with the setting, and a change rebuilds the
      path's pipelines, buckets and targets as a setting change rebuilds the terrain's today;
      `DepthPyramid` and `VisibilityBuffer` resize with the scene target. If `diag-frame-cost`'s
      automatic quality floor lands, it steps `GeometrySettings` with the rest of the setting, or
      leaves them at the setting's values; a change of the patch count at orbit is T3.e's ruling's.

18. **Measurement.** Every task is accepted on the same yardsticks:
    - **Counts** (the pod): triangles and draws a pass from `ClusterStats` in test-render's
      `R10.T16 the frame` group (a known scene, both variants) and the counting harness over R05's
      descent at the record seed (124 poses, 1920 × 1080 at 60°) on R10.T4.b's three sample worlds,
      both settings; before and after.
      - **The harness's home**: the research's was lost in scratch, so T1.b commits it as
        `clusters/clusterCounts.ts` (renderer code reads no files) run by
        `node --no-warnings scripts/clusterCounts.mjs [--setting high|low] [--worlds …] [--out <dir>]`
        through Vite's module runner, as `levelCounts.mjs` runs `levelBoundsCounts.ts`; by hand, not
        in `ci`, with no justfile recipe (which would plan the full `just ci`). It writes JSON and
        markdown to `target/cluster-counts/`, is checked against the research's table on its first
        run, and each task that builds a lever adds that lever's rule.
      - **Edge patches** are counted apart from interior ones, on the wholly surveyed sample worlds
        and on a served world mid-survey (`owner-test`'s build), since a body partly surveyed is the
        game's ordinary state and every expectation above is for a wholly surveyed body; each task
        records both. If edge patches pass 20% of the lit triangles, a later subtask puts their fins
        and skirt heads into a third cluster shape, `terrain-fin`, which keeps the survey discard.
    - **GPU per pass on SwiftShader**: relative only.
    - **Timed passes**: each dispatch takes one of `PASSES_PER_FRAME` (64) timed passes. A view's
      cull, the scan's two or three dispatches, about ten pyramid mips and the second cull can pass
      64 with three views, so the pyramid's mips fold into one dispatch, or are timed under one
      label. Each `target.render` resolves its own timestamps, and `TIMING_FRAMES_IN_FLIGHT` (135)
      is sized on 15 resolves a photorealistic frame (decision-r07-t19); a pass past 64 goes untimed
      with one warning, and a dropped resolve leaves a silent gap. So each task counts its timed
      passes and resolves for the heaviest frame (a photorealistic primary and two instruments)
      against both constants: new dispatches ride the next resolve, as dispatches do, or a constant
      is raised by decision-r07-t19's rule, and a Vitest pins the count.
    - **The owner's runs** (pending each time): the Mac (M-class, Metal) and the RTX 3080 at 1920 ×
      1080, both settings, `just views-check --terrain --whole --hidden` and `diag-frame-cost`'s
      profile command once it lands: fps, the RTX's p50 and p95 against R08's 15.67 ms target and
      16.67 ms blocker, and GPU per pass. Each records against the brainstorm's budget rows: the
      terrain's GPU time on the RTX (lit, casters, overlay depth, cull, resolve) against "Terrain
      geometry and patches" (3–5 ms discrete), the cascades' against "Shadows" (1.5–3 ms); T4.c the
      wireframe's against "Station wireframe view" (under 3 ms at 1080p), alone and during the
      descent; T5.d scatter's against "Scatter instances" (1–2 ms) and the ring particles' against
      "Rings, when in view" (0.5–1 ms). The UHD 620 columns go to R12.T8. The Mac's records name its
      target-hardware entry (Design note 7).
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
      max(k_CDLOD, F)), and the stride's morph moves the place `lit_place` returns. While a rebake
      fades, the cull keeps the union of the old and the new slots' bounds (Design note 4).
    - **Fades on the visibility path**: a dithered cross-fade costs least there, as complementary
      dither masks in the ID raster (on the fading patches alone) and one shading in the resolve,
      not as a second lit draw; the hand-over's cross-fade (R10.T10.g's remedy 2) and new survey
      rising out of the reference surface may move there once built.

## Tasks

Order: T1 first, the shared substrate and the largest exact cut (running as `geometry-clusters-1`).
Then T2.a, the lit pass on the cluster path, forward: it provides the camera passes and the stable
compaction that every colour pass needs (T3.c, T3.d, T4.a, T4.c), so it comes first whichever way
the next choice goes. After T2.a, the owner's Mac and RTX runs, with `diag-frame-cost`'s split,
decide between T2.b–e (the visibility buffer) and T3.c–d (strides and occlusion). That choice is the
owner's to rule on those runs; the lean is T2.b–e first on Apple, whose gains (overshading,
varyings, hidden-surface removal) are certain where T3's wait on a histogram. T3.a's histogram runs
straight after T1, since it is cheap and decides T3's gain. T3.b follows `perf-terrain-bakes`; T3.c
follows seamless-2; T3.e follows seamless-1–3 and a ruling, which takes T3.c's error rule with it.
T4 follows T2.a, but T4.b needs only T1 for its depth-only wireframe draw (its colour silhouette
waits on T2.a). T6.a follows T1.b and seamless-2 (or its test covers the fade fields); T6.c follows
T4.b. T5 follows T4 and waits on content (R11's rocks and particles, craft art), with T5.a buildable
on procedural test meshes beforehand. T7 follows the owner's sign-off.

Client files are under `apps/hyperion/src/renderer/src/view/` unless a path says otherwise. Every
task runs:

- Vitest for its twins and packings, `pnpm --filter hyperion exec vitest run <path>`;
- `just test-render`, both variants, re-pinning the shader digests of every terrain and caster
  source it edits;
- the counts of Design note 18, before and after;
- `golden_diff.py`, which says "No golden files changed";
- its categories' bytes from the allocation events against Design note 15's rows, and its timed
  passes and resolves counted for the heaviest frame (Design note 18);
- a restore test for each cluster resource it adds (Design note 16);
- and lists the owner's runs under NOT RUN with their commands, and Windows (no hardware).

New terrain groups join test-render's part 7, where R10's terrain groups moved after they took their
old part to 163 s of its 180 s watchdog (2026-10-21); a group that pushes part 7 past 70% of its
watchdog moves to a new part.

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
  the per-pass counters in the terrain run's and the descent spike's results; the counting harness
  and the capture comparison; the `--geometry` override and the way back (Design note 17).
- _Consumes:_ R01's `dispatch`, indirect draws, `KernelPair` and `MemoryCategory`; R05's
  `ResidentPatch`, `DrawSet`, `patchBounds`, the frustum and horizon predicates, `vertexSpacing` and
  the lit mesh, `SlotUpload`; T1.a's `TerrainLevers`; R10's `#drawCasters`, `castsInto`,
  `CascadeSet`, the cascades' receiver twin, `lit_place`, `survey_kept_at`,
  `TerrainLayers.unsurveyedCells` and `symbologyDepth`.

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
  - Makes the way back runnable (Design note 17): `GeometrySettings` becomes the source of T1.a's
    `TerrainLevers`; the `--geometry <field>=<value>[,…]` override on `just views-check`,
    `just descent-spike` and the smoke page; test-render's part 7 draws its terrain once more with
    `clusterPath=false`; and the refused-kernel fallback is tested with `--fixture=broken-wgsl`.
  - Commits the counting harness (Design note 18: `clusters/clusterCounts.ts`,
    `scripts/clusterCounts.mjs`) on `levelBoundsCounts.ts`, with each lever's rule as built and edge
    patches counted apart.
  - _Files:_
    `clusters/{record,templates,cullPass,clusterCull.wgsl,cullTwin,buckets,stats,clusterCounts}.ts`,
    `quality/qualitySetting.ts`, `engine/memory.ts`, `terrain/terrainPass.ts` (`#drawCasters`,
    `#casterItems`), `terrain/shaders/terrainShadow.wgsl`, `engine/catalogue.ts`, the terrain run's
    results, `src/main/results.ts` (the descent spike's counters: `RESULTS_VERSION` 6 → 7, with
    R05's recorded files still read by version) and its markdown, `src/main/cli.ts`,
    `scripts/{viewsCheck,descentSpike}.sh`, `scripts/clusterCounts.mjs`,
    `src/renderer/src/smoke/page.ts`, tests.
  - _Tests:_ the templates (index counts by stride, 128 / 32 / 8 / 2 and 16 / 8 / 4 / 2; every edge
    vertex of a block at stride s shared with its neighbour at s; the lit mesh's winding; strips
    closing to the grid; every template triangle's first vertex has its part's code, a grid triangle
    a grid vertex and a foot triangle a foot vertex); `ClusterRecord` and `CullPass` packing against
    their WGSL structs; over random sets, the cull's twin never refuses what `castsInto` keeps and
    keeps nothing it refuses beyond its widening (Design note 4), and the sphere tests keep every
    cluster R05's box predicates keep; the bucket stage's position is `@invariant`; a block's bounds
    hold all its vertices, morphed or not, on wasm bakes of R10.T4.b's sample worlds (a
    `.wasm.test.ts`) and on R05.T4.b's golden vertices (`vertex_f32.golden`); the buckets' arguments
    for a known set. Beside the random sets, the cull's twin tests the brainstorm's cases (Testing,
    "Culling"): a cluster larger than the frustum; the camera inside a cluster's bounds (the forced
    region's ordinary case); a cluster across the near plane; the horizon test at grazing altitude
    and below R_occ; the `f32` widening at 10⁷ m from the origin. A restore test (Design note 16)
    and the timed passes' count (Design note 18).
  - _Acceptance:_ captures byte-identical; caster triangles reported per cascade, in the terrain
    run's results and test-render's `R10.T16 the frame` log.
  - _Built_ (66d7c32): records at the end of Risks.
- **R14.T1.c Casters culled by the depths their receivers sample.** Exact under `castsInto`'s own
  relief premise; size M.
  - _Today:_ a near patch is drawn into all four cascades: 1.3–4.0 caster patches a drawn patch.
  - _Lever:_ Design note 4's receiver-depth rule, keeping the fall-through case; non-terrain casters
    untouched.
  - _Files:_ `clusters/{cullPass,clusterCull.wgsl,cullTwin}.ts`, `terrain/terrainPass.ts`, tests.
  - _Tests:_ a twin test over random sets and casters: no culled caster has a receiver that
    `cascadeShadow.ts`' receiver twin sends to cascade k, the receiver that falls through included.
  - _Acceptance:_ captures byte-identical (the layers may differ only where no receiver samples);
    caster triangles 0.48–0.79 of T1.b's above 1 km on the research's counts, which left out the
    fall-through, so accepted against the rebuilt harness's.
  - _Built_ (4190c9d): records at the end of Risks.
- **R14.T1.d Casters at their cascade's texel.** A lean; size S.
  - The coarsest stride s ≤ 8 with s × the level's largest spacing ≤ the cascade's texel, 2 r ÷
    2,048. All of a patch's clusters take one stride a cascade, so no crack opens inside a patch,
    and the skirts close the rest. `casterStrideCap` caps it.
  - Adds `scripts/compareCaptures.mjs`, and adds to `--captures` the lit frames of R10.T9.b's
    cascades and of R10.T16's frame, each with its terrain mask, drawn with and without the lever in
    one run at a fixed exposure (Design note 10).
  - _Files:_ `clusters/{cullPass,clusterCull.wgsl,cullTwin}.ts`, `terrain/terrainPass.ts`,
    `scripts/compareCaptures.mjs`, `smoke/sharedPass.ts`, `smoke/terrainFrame.ts`, tests.
  - _Tests:_ the stride twin: the coarsest s with s × the largest spacing ≤ the texel, one stride
    for every cluster of a patch in a cascade.
  - _Acceptance:_ at ≥ 99.5% of those lit captures' terrain pixels the lit image within 1/3 stop of
    T1.c's (the hand-over check's unit), the rest listed with their largest difference; before and
    after captures in the outbox; caster triangles a further 0.65–1.00 above 1 km and 0.56–0.91
    below on the research's counts, accepted against the rebuilt harness's.
  - _Built_ (b91da73): records at the end of Risks.
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
    projected at each cluster, stays within the push's depth (about 0.15 m at 10 km). `overlayDepth`
    records which.
  - The scene's depth carries neither the terrain's 2⁻¹⁶ push nor the survey edge's pull, so the
    first form keeps both in the marks' test. It also changes what the marks test against, every
    opaque pass's depth, and agrees in size only on high at internal scale 1, never on low's 720
    rows. **Lean** (the reviews' verdict): the scene's depth where the sizes agree, the cluster path
    at stride 1 elsewhere; from T3.c's strides, the cluster path (T3.c, "The overlay's depth").
  - _Files:_ `terrain/frameTerrain.ts`, `photoreal/renderer.ts`, `photoreal/overlay.ts`, tests.
  - _Tests:_ for each choice, every mark of R10.T16's frame group kept or hidden exactly as the base
    renders it, compared mark by mark from the overlay's draw list and the depth at the mark's
    pixel.
  - _Acceptance:_ no mark's visibility changes in R10.T16's frame group (smoke part 7) or the views
    check's captures, the hull, orbit and graticule marks at a limb among them; overlay triangles
    counted, with no ratio target (the research's overlay at 4 px was 0.80–0.92 of the base's on
    high at 1–10 km and 1.0 on low, so 0.7 was never in reach).
  - _Built_ (aac8ec0, c5171c5; `cluster`, not the lean): records at the end of Risks.
- **R14.T1.f What T1's lane lands without.** Conditional; size S–M, after T1.e. Whatever of this
  plan's T1 the lane's commits leave out, if any: `GeometrySettings` with each lever's field, the
  `geometry-clusters` category and the catalogue entries; the way back made runnable (Design note
  17); the counters through a `bit-exact` reduction; records inline. _Acceptance:_ T1.b's.
  - _Built_ (02fea32 and this lane's two commits): records at the end of Risks.

T1's NOT RUN: the owner's RTX 3080 and Mac runs of `just views-check --terrain --whole --hidden`,
both settings, and `diag-frame-cost`'s profile command, before and after: fps at 1920 × 1080 and GPU
per pass. With the runs after T2.a, they decide T2.b–e against T3.c–d (Tasks' order).

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
  (T2.b), `photoreal/renderer.ts` (the passes' order), `photoreal/passes.ts` (R07's pass list: the
  slots of the cull, the compaction, the ID raster and the resolve), `engine/catalogue.ts`, tests.

- **R14.T2.a The lit pass on the cluster path, forward.** Exact; size M.
  - The stable compaction (`CLUSTER_COMPACT`, Design note 5) and a camera pass in the cull: the lit
    pass's interior patches draw through the buckets at stride 1, in the draw set's order, with the
    cluster frustum and horizon cull; edge patches keep their own draw after them. A seamless-1
    sub-patch instance becomes a cluster mask over A's blocks, with its interior-edge strips, only
    where its level offset is 1–3 and seamless-1 draws it as A's own blocks; otherwise it stays on
    the edge path (Design note 19). T2.a consumes sub-patches and never builds them.
    `shading: "forward"`.
  - _Tests:_ the scan's twin over random counts, both kernel paths byte for byte; the list's order
    is the draw set's; the frustum cull's twin against R05's brute-force oracle (zero false culls),
    with T1.b's named cases; a restore test (Design note 16).
  - _Acceptance:_ captures byte-identical to T1's (tie pixels listed); lit triangles below T1's by
    the rebuilt harness's figure for spheres on 8 × 8-quad blocks (the research's 2–15% was boxes on
    4 × 4 clusters of 16 × 16 quads).
  - _Built_ (geometry-clusters-2): records at the end of Risks.
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
    `WANTED_FEATURES` and used where the device has it; interior patches, with no discard; the edge
    patches forward after T2.d's resolve (Design note 9).
  - The visible-cluster list is one a view shares across every consumer, each entry (consumer,
    instance, cluster), so this subtask fixes the layout T5.e adds a pass to (Design note 9).
  - _Tests:_ the ID's packing and its twin; the de-indexed and `primitive_index` paths give the same
    IDs on SwiftShader (test-render's both variants); every material's storage bindings a stage
    counted against 8.
  - _Acceptance:_ the scene's depth after the bodies pass byte-identical to T2.a's; the raster
    pass's inter-stage data at most 20 B a vertex.
- **R14.T2.d The resolve.** Size L.
  - The full-screen resolve with `ResolveMaterial`: a pixel whose ID is 0xffff_ffff discarded with
    no depth written; the visible cluster's template from its list entry (the cluster word's top 3
    bits); terrain's refetch through `lit_place`, barycentrics, the face normal for the cascades'
    receiver, then `shade_terrain` as the lit fragment calls it; bindings brought to 8 or fewer
    (Design note 6). No opaque pass between the raster and the resolve. One resolve pass a consumer
    class present, refusing the other classes' pixels (Design note 9); R11's planned terrain-shading
    additions listed by binding class against its 8. The edge patches draw forward in the resolve's
    render pass, after it, with the lit material and its survey `discard`, depth-tested against the
    raster's depth (Design note 9).
  - _Tests:_ the barycentric twin against the rasteriser's on test-render's terrain frame:
    barycentrics from the three clip positions' edge functions with perspective division, the
    reconstructed place within the hardware interpolant's own error (`f32`, a few ulps), not a
    fraction of the distance, which grazing triangles cannot meet; the resolve's output against
    T2.a's at the frame's pixels.
  - _Acceptance:_ at ≥ 99.5% of the judged terrain pixels of TERRAIN TEST's captures and the lit
    frames T1.d adds to `--captures` the image within 1/3 stop of T2.a's, the rest listed; no change
    in the reference surface's pixels at the survey edge; the hand-over check
    (`--terrain --handover`) passes as before.
- **R14.T2.e The Mac's order, and the switch.** Size S.
  - `shading: "visibility"` on high once the owner's Mac and RTX runs are recorded; low stays
    `forward` until the owner's UHD 620 run of `just views-check --terrain --hidden --setting low`,
    or R12.T8.b if that comes first (Design note 17; R12.T8.b needs R12.T1–T5, none built). The
    bodies pass ordered as Apple asks (Design note 7): the interior terrain, then the mesh figures
    (which may discard), then the edge patches, accepted byte-identical (depth decides; ties
    listed). Each pass split by T2 counts its attachments' loads and stores in the Mac run.
  - _Tests:_ on the pod, the frame's draw list orders the bodies pass opaque first, then
    may-discard.
  - _Acceptance (pending: the owner's runs):_ the Mac's fps on high at 1920 × 1080 at least T2.a's,
    the RTX's p95 not above T2.a's on both settings, GPU per pass recorded.

### R14.T3 Measured-error LOD and occlusion

Size L, client plus a presentation-only worker output: T3.a after T1, the rest after T2.a (and after
T2.b–e where the owner's ruling puts the visibility buffer first).

- _Provides:_ `clusterErrors` and the measured records, strides in camera passes, `DepthPyramid`,
  `strides` and `occlusion`, the triangle budget's draft.
- _Consumes:_ T1 and T2.a's camera passes and compaction (T2.c–d's visibility path where it is
  built); `BakedPatch` and the worker's bake (after `perf-terrain-bakes`); seamless-2's fade fields;
  `CoarseFineHolds`; the seamless probe; `selectPatches`' budget (T3.e).
- _Files (all of T3):_ `clusters/{errors.ts,hzb.ts,hzb.wgsl,clusterCull.wgsl}`,
  `terrain/workers/{heightBake,messages}.ts`, `terrain/coarseFineHolds.ts`,
  `terrain/terrainPass.ts`, `terrain/shaders/patchVertex.wgsl`, `terrain/frameTerrain.ts`,
  `photoreal/overlay.ts` (T3.c's overlay depth), `photoreal/renderer.ts` and `photoreal/passes.ts`
  (T3.d's split of the bodies pass, each part a
  labelled submission), tests; T3.e's draft in this plan.

- **R14.T3.a The error histogram.** Size S. Over TERRAIN TEST and a served world (`owner-test`'s
  build: this tree serves no generated surface until 25), the per-cluster e₂, e₄, e₈ against each
  cluster's τ′ at R05's descent poses: the share of lit triangles that a stride within τ′ removes,
  by band and setting, under both rules of Design note 3 (the level's stated bound plus e_s, and the
  measured per-patch deviation plus e_s). It decides T3.c's gain before T3.b starts. _Acceptance:_
  the table, both settings and both worlds, in the lane's outbox, with the harness that made it.
  - _Built_ (`r14-t3a-error-histogram`; `clusters/errors.ts` with T3.b's brute-force test): records
    at the end of Risks.
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
  - _Built_ (`r14-t3b-worker-errors`; the errors step 0.6–1.1% of a lit bake, the lane's outbox's
    figures): records at the end of Risks.
- **R14.T3.c Strides in camera passes.** Size L, after seamless-2.
  - The cull picks the coarsest stride whose level's stated error plus e_s, projected at the
    cluster's distance, is within τ′ (Design note 3's τ′, unslewed); stride 1 in the forced region.
    It builds the stated-bound rule unless the ruling T3.e drafts adopts the measured deviation's.
    `strides: "measured"`.
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
  - **Beside an edge patch**, which stays at stride 1, a cluster takes stride ≤ 2 on the shared
    edge.
  - **The overlay's depth.** Where `overlayDepth` is `scene-depth`, the scene's depth carries the
    camera pass's strides, whose error passes the 2⁻¹⁶ push. So with `strides: "measured"` the
    overlay draws its own depth on the cluster path at stride 1 (`cluster`), or reads the scene's
    only where every drawn cluster is at stride 1. T3.c's acceptance adds T1.e's: no mark's
    visibility changes in R10.T16's frame group.
  - **Fallback**, if the band costs too much of T3.a's gain: a time fade, with a per-(slot, cluster)
    state buffer (the previous stride and its change time; presentation only; one more vertex-stage
    binding, against the 8; rewritten with the slot's records and lost with the device), refining at
    τ′ and coarsening at 0.8 τ′.
  - _Tests:_ every shared vertex of two clusters at strides s and 2s computes the same position at
    every k, k_s and t, across patch edges included (the twin, over random sets); a cluster at its
    threshold places every vertex alike at s and 2s; the forced region's clusters at stride 1, and a
    test-render frame with a grounded body whose pixels under the contact are byte-identical to the
    base's (Design note 10); the drawn error at every seen cluster ≤ its τ′ (item 30's oracle
    measure, decision-r10-item30, on R05's descent poses over the sample worlds' bakes).
  - _Acceptance:_ on the seamless probe at 3 × 41 ms, both settings, per-frame vertex moves from
    stride changes ≤ 1 px (high) and 2 px (low), normal changes ≤ 2°, no crack (no frame where the
    sky shows through the drawn surface in test-render's terrain frames); lit triangles as T3.a
    predicts, within 10%; no mark's visibility changes in R10.T16's frame group (T1.e's); the Mac's
    fps (pending).
- **R14.T3.d Two-pass occlusion.** Size M.
  - `DepthPyramid` and the two-pass cull of Design note 4 per view, in both the forward and the
    visibility paths. `occlusion: "hzb"`.
  - Last frame's visibility is a bit per (slot, cluster), cleared when a slot is refilled, so keyed
    by the slot's (patch key, generation), never by instance index, which changes on every
    reselection. It is dropped (the first pass draws nothing, and the second tests everything
    against a cleared pyramid) on a restore, a resize, a change of internal scale, a camera cut
    (`present: false`, a view switch, the hand-over) and a view's first frame.
  - **The frame's order**: the bodies pass splits into labelled submissions, raster 1 (the mesh
    figures among it), the pyramid, cull 2, raster 2 loading depth (`colourLoad: "load"`); the
    cascades' submissions stay before both halves. Edge patches are tested against the pyramid by
    their patch bounds. The wireframe takes occlusion only after T4.c, measured.
  - _Tests:_ the pyramid's twin (each texel the farthest of every source texel it covers, the odd
    row or column included, reversed-Z); a cluster wholly behind a nearer one is refused and one
    partly visible is kept, over random scenes; a cluster across the near plane and the camera
    inside a cluster's bounds against the pyramid; each case that drops the history, and a restore
    test (Design note 16).
  - _Acceptance:_ captures byte-identical with and without it, tie pixels listed (it reorders
    draws); clusters refused counted per pass; the frame's GPU time on the Mac and RTX recorded
    (pending).
- **R14.T3.e The triangle budget.** Size S to propose, M to build.
  - With strides, a frame's cost is its drawn triangles, not its patch count, so selection's budget
    (`maxPatches`, half the slots) may follow a triangle budget and the cache may grow without
    growing the frame. This changes R05 Design note 10's and R10 Design note 15's rule and
    `TERRAIN: DETAIL LIMITED`'s trigger: the task drafts the change with its counts for a ruling
    (decision-r10-t14b-constants is the rule's current ruling) and builds it only once ruled, after
    seamless-1–3 and whatever `diag-frame-cost` does to the patch count at orbit; the same ruling
    takes T3.c's error rule (Design note 3). The draft counts the bake demand too: a larger cache
    drawn at the same cost asks for more bakes, which are the streaming wall (`perf-terrain-bakes`).
  - _Acceptance:_ as ruled; `TERRAIN: DETAIL LIMITED` raised exactly when the drawn error exceeds τ′
    anywhere seen. With strides chosen on the GPU the CPU cannot know that, so the cull counts the
    clusters it draws over τ′ into a `bit-exact` reduction read each frame, which raises it (or the
    cull's twin decides it on the CPU).

### R14.T4 The first non-terrain consumers

Size M, client only, after T2.a, whose stable compaction every colour pass needs; T4.b's depth-only
wireframe draw needs only T1.

- _Provides:_ `ClusterConsumer`, `meshTemplate`, the DAG-cut draw and `ClusterLod`.
- _Consumes:_ T1 and T2.a; R07's `smoothMeshOf` and `smoothMesh.wgsl`; R02's `HullOutline` and
  `occluder.wgsl`; R10's `terrainWire.wgsl`.
- _Files (all of T4):_ `clusters/consumer.ts`, `bodies/smoothMesh.ts`, `shaders/smoothMesh.wgsl`,
  `wireframe/{hulls,drawList,submit}.ts`, `shaders/occluder.wgsl`, `terrain/terrainPass.ts`
  (`#writeWire`), `terrain/shaders/terrainWire.wgsl`, tests.

- **R14.T4.a The smooth figure on the path.** Size M. `smoothMesh.ts`' patches through the cluster
  path's strided templates, selected as today (`selectPatches` with no level table,
  `SMOOTH_MESH_TAU_PX`), at stride 1: its level of detail is selection's, and a coarser stride pulls
  the outline inward by its chord's sagitta, so any stride above 1 must keep that sagitta within
  `smoothMeshTauPx`'s 0.25 px over sec²θ. Its places are R05's `FaceDifferences` at h = 0 and read
  no heights, so it needs no page of its own. Its fragment may always discard (`disc_pixel`,
  wherever the body does not cover the pixel wholly, and beneath surveyed terrain), so its clusters
  are flagged may-discard and draw after the opaque ones (Design note 7). `ClusterConsumer` lands
  here. _Tests:_ the figure's cluster places against `FaceDifferences`' at h = 0 (the twin); its
  triangles before and after. _Acceptance:_ test-render's mesh-bodies frames
  (`src/renderer/src/smoke/meshBodies.ts`) byte-identical, against a base whose `smoothMesh.wgsl`
  position is first made `@invariant` in a commit of its own (Design note 5); draws counted against
  today's (expected: one a stride-1 template bucket).
- **R14.T4.b The generic mesh template, with the hull occluders.** Size S–M. `meshTemplate()`, a
  128-triangle cluster drawn by index pulling, and the DAG-cut mode's draw on a one-level DAG: the
  hull occluder faces (`occluder.wgsl`'s `corners`) as one page, depth-only in the wireframe and as
  the silhouette's fill over the image, depth pushed by `occluderSlopePx` as today. Index pulling
  costs three vertex invocations a triangle (Design note 2), counted. _Tests:_ the DAG cut's twin
  (own ≤ τ < parent) over random monotone DAGs selects a consistent cut. _Acceptance:_ test-render's
  hull and hidden-line frames byte-identical, against a base whose `occluder.wgsl` position is first
  made `@invariant` in a commit of its own (Design note 5).
- **R14.T4.c The wireframe's terrain on the path.** Size S. The wireframe's depth-and-contour pass
  through the buckets at stride 1, with the interior split (no discard) and the cluster cull (Design
  note 13). _Tests:_ contours at stride 1 everywhere, a twin of `#writeWire`'s. _Acceptance:_ the
  wireframe's terrain frames byte-identical; triangles counted, and the wireframe's GPU time against
  the station-wireframe row (Design note 18).

### R14.T5 Meshes: the builder, pages, producers and instances

Size L–XL, tool and client, after T4; each subtask waits on its content except T5.a.

- _Provides:_ the builder, `MeshPage`, `GeometryPagePool`, `ClusterResidency`, the procedural
  builder, the instance cull, the mesh `ResolveMaterial`, the meshes' occluder depth in the
  wireframe, `geometry-pages`, `pageBytes` and `meshTolerancePx`.
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
    owner's Mac by hand (pending), and on Windows when a machine exists (NOT RUN: no hardware),
    since `just cross-clippy` builds against a stand-in compiler and runs no test;
  - pages are built once, on Linux, by `just mesh-pages`, which packaging and test-render's prepare
    step run, never for each platform; a committed SHA-256 of each test mesh's pages is checked by
    the tool's own tests natively and on the owner's Mac by hand.

  _Tests:_ on procedural test meshes (an icosphere with noise, a torus knot, a 10⁶-triangle terrain
  mesh): every group border watertight at every cut; errors monotone; the same input gives the same
  bytes. _Acceptance:_ `cargo test --manifest-path tools/meshbuild/Cargo.toml`, and
  `just cross-clippy` extended to the tool as it is to `gpu-replay`; at τ = 1 px a 10⁷-triangle mesh
  covering a quarter of 1080p (or 10⁶ triangles over a sixteenth, 129,600 px) draws at most 2
  triangles a covered pixel, and at most 1.2 times a hand-picked uniform cut's triangles. (A
  10⁶-triangle mesh over a quarter, 518,400 px, passes 2 a pixel whatever its cut.)
  - _Built_ (2026-11-02): records at the end of Risks.

- **R14.T5.b Pages and residency.** Size M. `clusters/pageFormat.ts`, `GeometryPagePool`,
  `ClusterResidency`, the fetch-and-decode producer in a worker, `geometry-pages` and `pageBytes`.
  _Tests:_ LRU with the pins; the pool never evicts a drawn or forced page; a missing page draws its
  resident coarser group; a group's pages are evicted together; no hole and no overlap at every
  residency state of a three-level DAG (Design note 14's record rewrites); the CPU's demand (Design
  note 14) requests the coarsest level within τ at an instance's nearest point and its ancestors;
  pinned roots past 10% of `pageBytes` refused and annunciated. _Acceptance:_ a scripted fly-by of
  100 instances of the test meshes shows no hole in any frame, and stays within `pageBytes`.
- **R14.T5.c The procedural builder.** Size M, with R11.T4. The same builder in a worker, from a
  seed (R11's rocks tessellated within R11 Design note 6's chordal bound, then clustered); the
  could-touch region's rocks at their finest cut whatever the setting (R11 Design note 7).
  Decoration is instanced from a bounded set of prototypes a biome (Design note 14). _Tests:_ the
  procedural builder's pages are the same bytes for the same seed, in a worker and on the main
  thread. _Acceptance:_ R11.T4.c's collision-agrees test holds with the rocks drawn through the
  path.
- **R14.T5.d Instances.** Size M, with R11.T6.a and R11.T9.d. Instance culling above the cluster
  cull (an instance's bounds first), folded dispatches beyond 65,535, a few size templates for
  particles; scatter and ring particles drawn by bucket. _Tests:_ the instance cull's twin against a
  brute-force oracle over 10⁶ instances; the folded dispatch's indices a bijection. _Acceptance:_
  10⁶ test instances culled and drawn within R11's scatter row (1–2 ms on the discrete target,
  recorded), and the ring particles within the rings row; R11.T9.d's own (the slab plus the
  instances conserve transmittance to 1% over 10⁴ rays; no instances on low); on low, scatter as R11
  has it (off, or a token density).
- **R14.T5.e Mesh shading.** Size M. A mesh `ResolveMaterial` through R07's `body_brdf` with the
  cascades' receiver, in the visibility path (a resolve pass of its own, Design note 9), and its
  forward twin. _Tests:_ the barycentric and BRDF twin. _Acceptance:_ a lit test mesh's pixels
  within 1/3 stop of its forward rendering.
- **R14.T5.f Meshes in the wireframe.** Size S–M, after T5.b. The DAG consumers' occluder depth for
  hidden-line removal at `meshTolerancePx.wireframeOccluder`, under the strokes' half-width (Design
  note 13). _Acceptance:_ in test-render's hidden-line frames no hull edge flips between hidden and
  shown against the finest cut.

### R14.T6 Shadows on the cluster path

Size M, client only, after T1.

- _Provides:_ cascade reuse, far cascades at a lower rate, mesh casters on the cull.
- _Consumes:_ T1, T4.b; R10's `ShadowCasters`, `ShadowCasterProvider` and `TerrainShadows`.
- _Files (all of T6):_ `terrain/terrainPass.ts` (`#drawCasters`),
  `terrain/shadows/terrainShadows.ts`, `clusters/cullPass.ts`, tests.

- **R14.T6.a Cascades re-rendered only when they change.** Exact; size S, after seamless-2 (or its
  test covers the fade fields). A cascade is re-rendered whenever any input of its casters differs
  from its last render, compared on the CPU bit for bit:
  - its snapped centre and light basis;
  - the draw set's keys, generations and holds, and every caster instance record as written
    (seamless-2's fades and slewed bands change an instance's morph fields frame by frame with no
    key change, so any 0 < F < 1 re-renders each cascade it casts into);
  - the contacts, whose hold sets the casters' k (`terrainPatch.wgsl`'s `Contact`) and moves
    whenever a craft moves;
  - the camera's position (which sets the casters' morph) and view axis (which sets T1.c's cull);
  - the caster settings (`casterStrideCap`, `casterDepthCull`);
  - each `ShadowCasterProvider`'s items for it: a provider reports a revision, and one that has none
    re-renders every frame, so a moving craft's shadow never freezes;
  - the layers' last owner: one `TerrainShadows` serves a session's views, whose caster passes clear
    and redraw the same layers, so a cascade is skipped only where this view rendered it last.

  The light basis and the instance origins are in the body's rotated frame, so with simulation time
  running they change every frame: T6.a skips only while the body's rotation and the sun's direction
  are unchanged (time paused, a still-camera check), and gains nothing in flight, where T6.b's rate
  is the lever. The interior casters take the no-discard variant (exact: the discard never fires
  there) if T1.b did not. _Files:_ `terrain/terrainPass.ts`, `terrain/shadows/terrainShadows.ts`,
  tests. _Tests:_ a Vitest toggles each input once and sees exactly one re-render. _Acceptance:_
  captures byte-identical; on a still camera with time paused, 0 of 4 cascades re-rendered a frame
  after the first; during R05's descent, re-renders counted.

- **R14.T6.b Far cascades at a lower rate.** A lean; size S. Cascades 2 and 3 re-rendered every
  second or fourth frame while the sun's direction moves less than a stated share of its disc. Each
  `lightFromCamera` is camera-relative, so a cascade held from frame n − j is sampled with its own
  frame's matrix, translated by the camera's `f64` displacement since then and narrowed once;
  otherwise its shadows swim with the camera's motion. _Acceptance:_ shadow edges within one texel
  of every-frame rendering on the captures, and acne (the receivers' lit fraction) unchanged, at the
  descent's speed as well as under the sun's motion; the rate and the share recorded. _Tests:_ the
  rate rule's twin.
- **R14.T6.c Non-terrain casters on the cull.** Size M, after T4.b. `ShadowCasters`' providers
  become cluster consumers' cascade passes, each keeping its old draw behind `clusterPath` until
  signed off (Design note 17). _Tests:_ each provider's items kept or refused by the cull as by
  `ShadowCasters` today. _Acceptance:_ captures byte-identical; caster draws counted.
- **R14.T6.d Virtual shadow maps.** Recorded, not sized: a page-table shadow map over the same cull,
  once T6.a–c are measured.

### R14.T7 Retire today's path

Size S, client only, after the owner's runs sign off T1–T3 (and each of T4–T6's consumers its own).

- Removes the interior patches' instanced lit, caster and overlay draws, `clusterPath`'s `false`
  and each moved consumer's old draw. It keeps the edge patches' lit-mesh draws, which edge patches
  and seamless-1's sub-patch stand-ins at a level offset of 4 or more still use (Design note 19),
  and R05's spike shading on today's path for good, so that the gate's recorded runs stay comparable
  (Scope).
- From T7 a refused cluster kernel or material faults its consumer and states it, as a refused
  terrain material does today, and the `--fixture=broken-wgsl` test expects that (Design note 16).
- _Acceptance:_ test-render, both variants, its captures unchanged against the build before; the
  catalogue no longer holds the retired forms.

## Verification

- **Exactness**: byte-identical captures for T1.a–c, T2.a, T2.b, T3.d, T4, T6.a, T6.c and T7 against
  their bases (tie pixels listed); the bounded levers within 1/3 stop at ≥ 99.5% of terrain pixels
  (T1.d, T2.d); no mark's visibility changes (T1.e).
- **Conservativeness**: over random sets, each GPU test never refuses what its `f64` predicate keeps
  and keeps nothing it refuses beyond its widening (frustum, horizon, `castsInto`, receiver depth,
  occlusion; Design note 4).
- **The collision ground**: the forced region's clusters at stride 1, with the contacts' hold
  unchanged, in every camera, visibility and overlay pass, on both settings and in both styles
  (T3.c, T1.e); stride 1 also for any cluster meeting a contact's r_g and its ramp, and every
  DAG-cut consumer at its finest cut within a contact's could-touch region (Design note 10); R10's
  collision-agrees test unchanged.
- **Determinism**: `golden_diff.py` "No golden files changed" for every task; only `bit-exact`
  kernels' results read back; the builder's bytes the same on three platforms (T5.a, under its
  build's conditions).
- **Counts** (the pod, Design note 18): triangles and draws a pass before and after each task, both
  settings, against the expectations stated per task, with edge patches counted apart on a served
  world mid-survey.
- **Memory** (Design note 15): each task's categories' bytes from the allocation events against its
  rows; a restore rebuilds every cluster resource (Design note 16).
- **Seamlessness**: the seamless probe's bounds for T3.c.
- **By hand, the owner's** (pending): the Mac and the RTX 3080 at 1920 × 1080, both settings,
  `just views-check --terrain --whole --hidden` and `diag-frame-cost`'s profile, after T1, T2, T3;
  the target is the owner's report's: 30 fps on high and 60 on low on the Mac, with the RTX's p95
  within R08's 15.67 ms; GPU times against the brainstorm's budget rows (Design note 18); by eye,
  the descent from orbit to a metre with no new pop or crack. Each Mac record names the roadmap's
  target-hardware entry for it; Windows is NOT RUN on every task (no hardware).

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
- **The research's counting harness was not carried** (`tools/triCount.ts`, its JSON): T1.b commits
  its successor (Design note 18) on `levelBoundsCounts.ts`, and its first figures are checked
  against the research's table.
- **Edge patches**: every expectation is for a wholly surveyed body, while a served world mid-survey
  is the game's ordinary state; the harness counts edge patches apart, and past 20% of the lit
  triangles a `terrain-fin` shape follows (Design note 18).
- **Tie pixels** (Design note 5): T1.a's split and the buckets' template order can flip pixels where
  a skirt's top row meets a neighbour's grid at equal depth. If they show, one list in the draw
  set's order by the generic template is the fallback.
- **The brief's skirt-strip triangle counts** (32 / 16 / 8 / 4) are twice the lit mesh's (16 / 8 / 4
  / 2: the feet are one winding); and its "oriented box split 8 × 8" is not conservative on the
  quadratic warp, where a block's bounds are its descendant patch's (Design note 2). Both were told
  to `geometry-clusters-1` on 2026-11-01.
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
- **The low setting's visibility buffer** is unmeasured on the UHD 620: low stays forward until the
  owner's UHD 620 run or R12.T8.b (a lean). R12.T8.b needs R12.T1–T5's perf mode and scenes, none
  built, so low may stay `forward` for long.
- **Windows (D3D12)**: no machine runs it. Every task lists it NOT RUN, and it is held to builds and
  lints (`just cross-clippy`, T5.a's tool included).
- **The Mac's** target-hardware entry is the roadmap's placeholder until the owner fills it, before
  T1's first owner run (Design note 7). Its memory ceiling is the discrete target's until measured
  (lean).
- **A second window's device** duplicates every row of Design note 15.
- **Mesh receivers and the horizon map**: a mesh in a valley at sunset takes the cascades near the
  camera but no terrain horizon beyond them (open: a horizon lookup at the mesh's ground point).
- **Not consumers**: clouds (volumetric or a 2D layer), R11's ocean surface, the ring slab
  (R11.T9.c, analytic) and the disc impostors; the ocean's patch grid could take the strided
  templates later (open).
- **Today's path is kept** until the owner's runs sign off T1–T3; R14.T7 removes it, but for the
  edge patches' lit-mesh draws and R05's spike shading, which keep it for good.
- **T5 closes after RM6's content**: R11's rocks, pebbles and particles are its consumers.
- **Asks of other plans** (open):
  - R01: T2.b's ID target (an `r32uint` format, a target drawn with another's depth, a `uint`
    texture binding), built by this plan as a change to R01's contract.
  - `perf-terrain-bakes` (R10.T6.a, R05.T18): streaming's acceptance; on Apple Silicon the height
    workers' count by performance cores.
  - R12: depend on R14 as built (R12.T0 reconciles R01–R11 today); hold `GeometrySettings` in
    R12.T0's inventory and `SettingSnapshot`; a `PASS_ROWS` row for each of R14's timed labels (the
    cull, the compaction, the ID raster, the resolve, the pyramid, the stats reduction), since R12
    Design note 4's test fails on a timed pass with no row; and `geometry-clusters` and
    `geometry-pages` in `MEMORY_ITEMS` (R12.T3).
  - R07: slots in `photorealisticPasses` (`photoreal/passes.ts`) for those passes, and T3.d's split
    of the bodies pass (R07 Design note 18 owns the order).
  - R11: R11.T4's rock meshes, R11.T6.a's pebbles and R11.T9.d's particles draw through T5's
    consumers rather than paths of their own; its planned additions to terrain shading land within
    the resolve's 8 storage bindings (Design note 9). (Its stills already tile at
    `STILL_TILE_MAX_PX`, so the draft's ask of R11.T10 is dropped.)
  - The plan that builds craft (not written): hull art as meshes in T5.a's input format, beside the
    `HullOutline` the wireframe draws.
  - R05 and R10: T3.e's budget rule and T3.c's error rule, by one ruling.
- **The draft's ten questions**, as the three reviews answered them (2026-11-01):
  1. T2 or T3: T2.a first either way; then the owner rules on the Mac and RTX runs between T2.b–e
     and T3.c–d (lean T2.b–e); T3.a runs straight after T1 (Tasks).
  2. Low's visibility buffer: `forward` until the owner's UHD 620 run or R12.T8.b, whichever is
     first; low still takes T3.c's strides and T3.d's occlusion (Design note 17).
  3. T3.e's budget: drafted for one ruling, which takes T3.c's error rule and counts the bake
     demand, after seamless-1–3 and `diag-frame-cost` (T3.e).
  4. The builder: meshoptimizer bound for the tool only, under T5.a's three conditions; else a Rust
     port.
  5. The pool's sizes: 128 / 32 MiB stay leans; vertices and indices in separate bindings, no CPU
     copy, pinned roots within 10% (Design notes 6, 14, 15).
  6. T1.e: the lane's choice, with the lean of the scene's depth where the sizes agree and the
     cluster path at stride 1 elsewhere, keeping the push and the pull (T1.e).
  7. The milestone: RM8, with a done-when; R12 depends on R14 as built (the roadmap).
  8. Records: inline in this section (below).
  9. The brainstorm's mesh-shader row: amended, with a Brainstorm corrections row.
  10. Stills above 4,096²: the ask dropped; R11 already tiles them (Design note 9).
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
    retired by R10.T16.c);
  - low's occlusion is `hzb` after T3.d on both settings (completeness Q2: low is not left on T1's
    path) over the draft's `none` until measured; the UHD 620's run may turn it off;
  - the counting harness and the capture comparison are node scripts run by hand, as
    `levelCounts.mjs` is, not the `just cluster-counts` and `just compare-captures` completeness S18
    and S9 name: a justfile recipe would plan the full `just ci` for T1.b and T1.d;
  - meshes write no overlay depth (completeness C4 asked either way): the hulls' occluder faces hide
    marks behind craft as today, so `meshTolerancePx` has no overlay field;
  - the visibility buffer's several consumers resolve in one pass each (completeness S14), fixed by
    T2.c before T5 needs it;
  - T3.c's neighbours across a patch edge come from a second dispatch (correctness S7) rather than a
    rule both sides compute alike (conformance S3): each cluster's e_s differs, and a neighbour
    patch's records sit in another slot, which the instance does not name;
  - the order is T1, then T2.a, then the owner's ruling on the Mac and RTX runs between T2.b–e and
    T3.c–d (conformance B1, all three reviews' verdicts on the draft's question 1);
  - the mesh-shader row of the brainstorm is amended and recorded in the roadmap's Brainstorm
    corrections (conformance C5) rather than left as written (correctness): the new section takes
    Nanite's cluster LOD and culling, which the row's words rejected;
  - the draft's ask of R11.T10 (stills above 4,096²) is dropped (conformance C4): R11 already tiles
    stills at `STILL_TILE_MAX_PX`.
- **Records** go inline at the end of this section until `plan_split.py` splits the plan, as the
  lane rules and the roadmap's Conventions put them for every plan but R09 and R10.
  `geometry-clusters-1`'s brief asks for per-task record files; a `14-virtualized-geometry/records/`
  directory would make `plan_task.py`'s `is_split()` read R14 as split, though no task has its
  `Records:` line, so the lane writes its records here (an ask of main to the lane).
- **R14.T1.a** (389371b, 13100d5b; recorded by the plan's fold from the commits, since the lane's
  tree had no plan): `TerrainViewFrame.draws` (interior first) replaces `draw`, read by
  `frameTerrain.ts` (the overlay's depth copies each draw), `spike/spikeRun.ts` and the smoke;
  `TerrainLevers` (`interior`), `ALL_LEVERS`, `NO_LEVERS` and `TerrainPass.setLevers` draw one build
  with and without a lever; `litInteriorMeshData`, `isInteriorBake`, `withoutSurveyDiscard` and the
  interior lit materials, catalogued and in `materialSet.ts`. Every terrain vertex stage's position
  is `@invariant`, its six sources re-pinned.
- **R14.T1.b** (66d7c32): `view/clusters/` as built: `record.ts` (32 B `ClusterRecord`, a sphere;
  flags shape, DAG and may-discard bits; strided errors (0, +∞)), `templates.ts` (one mesh of 8
  templates: blocks 128 / 32 / 8 / 2, strips 16 / 8 / 4 / 2), `terrain.ts` (96 clusters a page,
  records written at each slot's upload from the L + 3 descendant's `patchBounds`, the grid place
  decoded from the cluster index in WGSL and its twin `terrainClusterGrid`), `cullPass.ts`
  (`CullPass` 128 B, `CullParams` 144 B), `clusterCull.wgsl` and `cull.ts` (the twin `cullTwin`),
  `buckets.ts` (`ClusterBuckets`, 20 B arguments a bucket, the list base as the `clusterBase` `u32`
  uniform), `stats.ts`. `GeometrySettings` and `GEOMETRY_BEFORE_R14` in `qualitySetting.ts` are the
  levers' source (`TerrainLevers` and `setLevers` became `geometry` / `setGeometry`); the
  `geometry-clusters` category; catalogue entries. The sphere was kept (no 48 B box); the cull tests
  it against `castsInto`'s box conservatively, the near side left to the patch's own test.
  Stand-ins (seamless-1's sub-patches among them) take the edge path in every pass. Caster
  triangles 0.471 of the way back's on the smoke's cascades frame.
- **R14.T1.c** (4190c9d): the receivers' rule in `clusters/receivers.ts` (`cascadeCullReceivers`,
  `cascadeCullPasses`) and `cull.ts`' `receiversRefuse`, the kernel's `receivers_refuse`; also on the
  CPU per patch for the edge patches' lit-mesh casters (not for a stand-in). Margins: depth
  2 o_k + 3 t_k, across 3 t_k + o_k + o₍ₖ₋₁₎, the covered square r₍ₖ₋₁₎ − 2 t₍ₖ₋₁₎. `CullReceivers`
  is `depthM`, `reachM` (nearer, farther) and `coveredBelow` {centre, half-side, reach}, not the
  plan's `receiverDepthM` / `Float32Array`. 0.701 of T1.b's caster triangles on the smoke frame.
- **R14.T1.d** (b91da73): the stride per instance a pass, `casterStrideCap` 8 on both settings;
  `scripts/compareCaptures.mjs`; the smoke's in-run comparison and `--captures` frames with masks
  (`smoke/leverCaptures.ts`). No cluster strides in any pod scene: the smoke fixture's selected
  patches stay spaced above twice every cascade's texel.
- **R14.T1.e** (aac8ec0, c5171c5): `overlayDepth` `redraw` / `cluster` / `scene-depth`; both
  settings take `cluster`, not the lean. A camera pass (kind 2, up to 6 planes) in the cull, a fifth
  bucket pass a view, every lit pass clustered (low too), the overlay's pass culled alone without
  cascades; `terrainClusterDepth.wgsl`; `photoreal/overlayDepth.ts` for `scene-depth`, kept
  selectable. `TerrainViewFrame.overlayDepth`, `FrameTerrain.sceneDepth()`,
  `FrameTerrainPass.geometry`. `counts()` gives `overlayTriangles`. T3.c can keep `cluster` (main's
  note): it is already the setting.
- **R14.T1.f** (02fea32): the counters through a `bit-exact` reduction (6 words a pass: clusters,
  triangles, by stride); `ClusterPassStats.byStride` replaces `byTemplate`. Not built (left for
  main): the `--geometry` override on `views-check`, `descent-spike` and the smoke page; the
  `--fixture=broken-wgsl` fallback test; the counting harness (`clusterCounts.ts`); the descent
  spike's counters (`RESULTS_VERSION` 7); the brainstorm's twin cases, the wasm bounds test, a
  restore test and the timed passes' count pin.
- **From building T1** (`geometry-clusters-1`'s notes for the plan):
  - T1.e's lean fails its acceptance: the scene's depth carries the reference surface's tessellated
    depth, which hides a body's own datum lines beyond its survey (in-run, settled frame: redraw =
    cluster, scene depth differs). Make `cluster` the setting and drop the lean, or say how the
    reference surface stays out of the overlay's test.
  - T1.d's yield depends on the selection's error, not only on the cascade's texel: a cascade's texel
    is about a pixel's footprint at its slice's far end, so only casters well inside it stride, and
    the smoke fixture's smooth fine levels leave its patches spaced above every texel. Accept T1.d on
    real worlds (the owner's run or the harness), not on the pod's smoke.
  - Timed passes: the cull is one timed pass a lit view-frame (`terrain cluster cull`), the counters'
    reduction one more while measuring. The engine's timer makes its query set (1,024 B, `other`) at
    its first timed pass; R10.T16.b's release check now counts it apart.
  - Buckets hold 5 passes a view (4 cascades and the overlay), and every lit pass is clustered on
    both settings: about 1.25 × the plan's caster-only bucket bytes.
  - The receivers' rule (T1.c) needs only `CascadeSet` fields that exist (`fadeStartM`, `farM`,
    `normalOffsetM`, `texelM`, `centreFromCameraLightM`, `radiusM`, `casterReachM`, `basis`).
  - `createComputeAsync` can throw at the call on a lost device rather than reject; a
    `Promise.allSettled` over it needs the call wrapped.
  - The forced flag (bit 18 of `parentQuadrant`) is not written in T1: no T1 pass strides where it
    applies (casters may stride; the overlay stays at stride 1).
  - The research harness was not rebuilt: T1's ratios are the smoke scenes' alone, and the
    15.4–20.3 M a frame figure was not checked.
  - From the interim: strips 16 / 8 / 4 / 2; `TerrainViewFrame.draws`; interior by the survey mask;
    `@invariant` first; no tie pixel in any compared frame; in-run A/B over cross-build compares; the
    list base as a `u32` uniform below 2²⁴; 7 storage bindings in the cluster vertex stage.
- **R14.T1.f** (geometry-clusters-2): what T1's lane left, built.
  - `--geometry <field>=<value>[,…]` on `views-check` and `descent-spike` alone
    (`preload/geometryLaunch.ts`, `HyperionApi.geometry`, `qualitySetting.ts`' `geometryOf`,
    `TerrainSessionOptions.geometry`), stated on the run's first line. The smoke page's form is
    `just test-render --geometry=…` (`--smoke-geometry`, `smoke/geometry.ts`), over its lit passes'
    settings.
  - The refused-kernel fallback: `TerrainPassOptions.clusterCullKernel` (the fixture's seam) and
    `TerrainPass.clusterRefusal`, the fault stated on the pass and the console, with no operator
    annunciation (lean, 2026-11-01: no guide row). Part 7 checks it under `--fixture=broken-wgsl`
    (`BROKEN_CULL_KERNEL`).
  - The counting harness: `clusters/clusterCounts.ts` and `scripts/clusterCounts.mjs`, T1.a–e's
    rules. Its frame totals on high above 1 km are 20.9–30.7 M today and 11.2–15.6 M on the path.
  - The spike's counters: results version 7 adds `geometry` (`main/geometryResults.ts`,
    `DescentSpikeReport.geometry`), and version 6 is still read (`RESULTS_VERSIONS_READ`);
    `tools/gpu-replay` stays at 6. `descentSpike.sh` gives a hidden run with no display headless
    Ozone (main's message).
  - The bounds test uses the Earth-like and Moon-like worker fixtures; Mars-like has no field
    fixture.
  - Timed passes: 5 a lit view-frame on high (the cull and four casters), 6 measured, no resolve.
- **R14.T3.a** (`r14-t3a-error-histogram`; its table and harness in the lane's outbox):
  - `clusters/errors.ts`' `clusterErrors(bake: ClusterErrorBake, key, planet): Float64Array` lands
    ahead of T3.b, as Provides sketches it: e_s of all 96 clusters at `ERRORS_PER_CLUSTER` (4) ×
    cluster + log₂ s, metres, e₁ = 0, monotone; `clusterErrorM` reads one. T3.b calls it unchanged.
  - e_s holds the GPU's `f32` placement as the records' spheres pad for it (`CLUSTER_PAD_M` +
    `CLUSTER_PAD_FRACTION` × the cluster's reach) and, on a strip, twice the largest bend of a foot
    whose height crosses the skirt depth between k = 0 and 1, so T3.b's brute force
    (`errors.wasm.test.ts` over `test/strideOracle.ts`) holds e_s ≥ the drawn distance with no slack.
  - It bounds one k across a cluster (Design note 3); `morphFactor` gives each vertex its own, so
    inside a stride-s triangle the drawn error can pass e_s by about Δk |q₁ − q₀|: T3.c's drawn-error
    test is where it shows.
  - The verdict: within τ′ the stated rule removes 96.4–97.8% of the kept lit triangles on every
    world, setting and band, the measured 97.1–97.8% (stride 8's ceiling is 97.8%), so T3.c builds
    the stated rule; the budget binds at 113–117 of 124 poses, and the stated bound is 3–240 times
    the bakes' measured deviation. The rules part only with strides held to τ (T3.e's ruling).
  - T3.c's "lit triangles as T3.a predicts" reads the table: about 2.2% of the kept clusters' lit
    triangles drawn (0.1 M of 4.3 M a pose on high above 1 km).
  - The table ran on wasm whole-body fields of TERRAIN TEST's moon and the Earth-, Mars- and
    Moon-like sample worlds in place of a served world, since this tree serves none at 24.
- **R14.T5.a** (2026-11-02):
  - `tools/meshbuild/` is a Rust port, no meshoptimizer bound (lean, 2026-11-02): the C++ flags and
    the version pin fall away, and no MIT notice is needed (it takes meshoptimizer's collapse
    passes, 75° flip bound and border weight, cited, and copies no code). The byte test stands:
    run on Linux; the owner's Mac pending; Windows NOT RUN.
  - `just meshbuild-check` (rustfmt, Clippy, rustdoc, tests) runs in `ci`'s side phase beside
    `gpu-replay-check` (lean, 2026-11-02: the plan has the tool outside `just ci`; inside it, the
    owner's `just ci` on the Mac is the byte test). smart-ci has a rule for `tools/meshbuild/`;
    `cross-clippy` and `_cross-clippy-for` take a list of tools; the tool's own `clippy.toml` is
    held to the shared ban list by the testkit's `clippy_bans`.
  - The page format, version 1, is in `tools/meshbuild/src/page.rs`'s documentation, which T5.b's
    `clusters/pageFormat.ts` mirrors. The page table summarises each page before it is fetched:
    its levels, its own and parent error spans and a sphere holding its clusters.
  - One grid for the whole mesh (lean, 2026-11-02): 21 bits an axis about the least corner, the
    step the least power of two spanning the largest extent (1 mm at 2 km). So a vertex two pages
    share and a group's sphere are the same bits in each, and a leaf's error 0 excludes the
    rounding (half a step an axis).
  - For T5.b: a unit is the clusters one group made, or the leaves one group simplified, never
    split across pages while it fits. The units holding a root come first (the header's root
    pages, one page of about 128 kB a test mesh), then by level, coarsest first, so a level and
    every coarser one are a prefix of the pages. `cut.rs`'s `CutView::draws` is the cut's CPU twin.
- **R14.T2.a** (geometry-clusters-2): the lit pass on the cluster path, forward.
  - `CLUSTER_COMPACT` (`clusters/compact.ts`, `compact.wgsl`) is `bit-exact`: three dispatches of one
    kernel by phase (count, scan, scatter), timed as `terrain cluster compact`. Its subgroup variant
    takes the count's sums alone, since WGSL does not order a subgroup's invocations by workgroup
    index. Twins: `compactTwin`, `scanTwin`.
  - The cull has an ordered camera pass (kind 3) that marks one word a cluster (`marks`, binding 7).
    `CameraCullPass` gains `horizon` and `order`. `CullHorizon` and `cullHorizonOf` give R05's
    occluder as a sphere test: refused wholly beyond the tangent plane and inside the cone, with d²
    widened by `HORIZON_ROUNDING`. Twin: `cullMarksTwin`.
  - Buckets hold 6 passes a view (`CLUSTER_LIT_PASS`, the sixth), with marks and tallies a view in
    `TerrainClusterBuffers`.
  - `GeometrySettings.litClusters` is T2.a's lever, true on both settings and false on the way back
    (lean, 2026-11-02: the plan names no field for T2.a). `shading` waits for T2.b–e.
  - Materials `lit-cluster` and `lit-emissive-cluster` (`terrainLitClusterMaterialSpec`, the lit
    `Draw` taking `clusterBase` by `withClusterBase`), in the session set and the catalogue.
  - On the lit cluster path the overlay's redraw draws the lit pass's buckets with the cluster
    depth: the same triangles.
  - `TerrainGeometryCounts.litClusterTriangles`, also in the spike's geometry segments.
  - No seamless-1 sub-patch becomes a mask: seamless-1 draws a sub-patch's own grid on its
    ancestor's triangles, never as the ancestor's own blocks (Design note 19), so every one stays on
    the edge path.
  - Timed passes: 8 a lit view-frame on high (the cull, the compaction's 3 and the 4 casters), 9
    measured; no resolve of their own.
- **R14.T3.b** (`r14-t3b-worker-errors`):
  - Each lit bake (a body's with its weights, `terrainShadingOf`'s `lit`) comes with
    `BakedPatch.clusterErrors?: Float16Array`: e₁, e₂, e₄, e₈, metres, rounded up
    (`packClusterErrors`; +∞ past 65,504), four a cluster in `clusters/terrain.ts`' order. Wireframe,
    spike and floor bakes have none. It flows `SlotUpload.clusterErrors` →
    `TerrainClusterSource.clusterErrors` → the slot's records.
  - For T3.c's cull: a record with flags bit 6 (`CLUSTER_FLAG_MEASURED`) holds them in its error
    bytes, `unpack2x16float(errors.x)` = (e₁, e₂) and `.y` = (e₄, e₈), with no `enable f16`; an
    unmeasured one keeps (0, +∞) as `f32`, bit 6 clear. `ClusterRecord` is a union on `mode`: a
    strided record's `strideErrorBits` (or `null`), a DAG one's `errorsM`.
  - The worker forms e_s from the module's own offsets on both paths (asked of it for every lit
    bake, dropped on `face-differences`), not Design note 3's "own `f64` positions" on low, on the
    datum of the new `SurfaceFields.figureM` (`WorkerFields.figure`). Its allowance takes the
    patch's reach, vertices and feet, at least T3.a's per cluster.
  - `clusters/terrainGrid.ts` holds `Stride`, `STRIDES`, `CLUSTER_QUADS`, the page's cluster counts,
    `CLUSTER_PAD_*` and `edgeVertex` with no engine import (`templates.ts` and `terrain.ts`
    re-export them): what `errors.ts` imports stays in `tsconfig.worker.json`'s list.

/**
 * The smoke harness's twin check of the cluster path's cull (plan R14, R14.T1.b,
 * `R14.T1.b the cluster cull against its twin`): `clusterCull.wgsl` dispatched on the device over
 * random instances, records and cascade passes, its buckets read back and held between its twin's
 * (`clusters/cull.ts`) exact test and its twin at twice the kernel's rounding allowance.
 *
 * @remarks
 * Every entry the exact twin keeps must be in the device's bucket, and every entry of the device's
 * bucket must be the doubly widened twin's: so no cluster the `f64` test keeps is refused, and none
 * is kept that the test with the kernel's allowance would not keep. The strides take the pass's cap
 * of 8 at random tolerances, so that every template's bucket is filled; a bucket's count is its
 * entries'. The first pass culls by its box alone, the next three by their receivers' depths too
 * (R14.T1.c), the second with no cover below, and the fifth is a camera's frustum (R14.T1.e). The
 * buckets' order varies with the appends', so each is compared as a set.
 *
 * The sixth is the lit pass's ordered camera pass (plan R14, R14.T2.a, `R14.T2.a the stable
 * compaction against its twin`): the camera's frustum and an occluder's horizon, its marks read back
 * and held between the twin's exact and doubly widened marks, and the stable compaction
 * (`clusters/compact.ts`) dispatched over the device's marks, its pass's buckets read back and
 * held to its twin's lists byte for byte, entry by entry in the draw set's order; then once more
 * over random marks of more instances than its scan's workgroup has threads, so that the scan's runs
 * are several instances long. Each variant runs its own path, the default the subgroup variant
 * where the adapter offers subgroups, so that both paths are held to the twin.
 *
 * The seventh pass is a camera's of no planes at strides up to 8 (plan R14, R14.T3.c), which every
 * instance enters, the lit pass's cap 8 too: the records hold random measured errors (some none),
 * and each instance's record a random stride scale in its words 12 and 13 (none for some) and its
 * edges' shared bands in words 20 to 67 (half floats of kilometres, some none and some +∞), drawn
 * from generators of their own, so that each cluster's shared bands (`clusterStrideBandsM`) vary
 * with its neighbours' errors and its edges'; some instances its quadrant word's bit 18 (stride 1)
 * and some its edges' holds at k = 0 (at most 4 on those edges), the blocks' and strips' template
 * references their places, so that the seventh pass's buckets take every stride of both shapes.
 * Every seventh instance is a stand-in's mask of its page's own blocks (plan R14, Design note 19;
 * word 8's bit 31) one to three levels down, so that every pass keeps its blocks alone, by their
 * spheres, and its strips whatever theirs.
 *
 * The lit and the seventh passes take a τ′ ({@link PAST_TOLERANCE_RAD}) tighter than the instances'
 * own tolerances, and the dispatch counts their strides drawn past it (plan R14, R14.T3.e): each
 * pass's count, read back from its word past the buckets, held between its twin's
 * (`cullOverTwin`) over the exact and the doubly widened tests' clusters, the seventh's above 0.
 *
 * A second dispatch (plan R14, R14.T3.c's edge stand-ins) culls an edge bake's stand-ins over pages
 * of terrain's own layout, 116 clusters with their fin and head runs, each record's class random
 * (opaque, may-discard or none): the four quadrants' edge masks of one page, their side runs
 * shared as `shareMaskRuns` leaves them, one alone, which draws its six; masks one to three levels
 * down and an own grid on opaque blocks (`TERRAIN_INPUT_SURVEYED`), and an interior bake's mask and a
 * whole patch beside them. Its buckets are held to the twin's as a set, entry by entry; each edge
 * mask's to exactly its blocks of a cluster, its 16 strips in the strips' may-discard bucket and its
 * quadrant's runs of a cluster, each side run drawn once; and each surveyed one's to its blocks and
 * strips alone, opaque, none of a fin run.
 */

import type { Vec3 } from "../geometry/vec3";
import { ClusterBuckets } from "../view/clusters/buckets";
import {
  CLUSTER_COMPACT,
  COMPACT_PHASES,
  COMPACT_TEMPLATES,
  compactTwin,
  compactWorkgroups,
  packCompactParams,
} from "../view/clusters/compact";
import {
  CLUSTER_CULL_KERNEL,
  CULL_INPUT_WHOLE,
  cullMarksTwin,
  cullOverTwin,
  cullTwin,
  type CullTwinInput,
  NO_MARK,
} from "../view/clusters/cull";
import {
  type CameraCullPass,
  type CascadeCullPass,
  type CullPass,
  type CullReceivers,
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  cullHorizonOf,
  MAX_CULL_PASSES,
  packCullParams,
  packCullPasses,
} from "../view/clusters/cullPass";
import {
  CLUSTER_RECORD_BYTES,
  type ClusterRecord,
  packClusterRecord,
} from "../view/clusters/record";
import {
  EDGE_BANDS,
  edgeBandHalf,
  heldEdgeBandsM,
  INSTANCE_FINEST,
  INSTANCE_SUB_GRID_STRIDES,
  NO_STRIDE_BAND,
  NO_STRIDE_SCALE,
  type StrideScale,
} from "../view/clusters/strides";
import { TERRAIN_DISCARD_TEMPLATES, terrainClusterMesh } from "../view/clusters/templates";
import {
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_HEAD_CLUSTERS,
  TERRAIN_INPUT_COLUMN_RUN_SHARED,
  TERRAIN_INPUT_EDGE_MASK,
  TERRAIN_INPUT_ROW_RUN_SHARED,
  TERRAIN_INPUT_SURVEYED,
  TERRAIN_PAGE_CLUSTERS,
  TERRAIN_SUB_PATCH_MASK,
  terrainFinTemplateRef,
  terrainMaskCluster,
  terrainMaskRuns,
  terrainTemplateRef,
} from "../view/clusters/terrain";
import { subPatchWord } from "../view/terrain/gpu/uniforms";
import { quaternionFromRows } from "../view/camera/quaternion";
import { frustumOf } from "../view/terrain/cull";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../view/engine/types";
import type { Checks } from "./harness";

/** Whether the check's instance `n` is a sub-patch on its own grid (R14.T3.c's stand-ins). */
function smokeOwnGrid(n: number): boolean {
  return n % 7 !== 3 && n % 11 === 6;
}

/**
 * The check's instances, pages, clusters a page and passes: four cascades, a camera, and the lit
 * pass's ordered camera (R14.T2.a), the sixth.
 */
const INSTANCES = 48;
const PAGES = 12;
const CLUSTERS = 96;
const PASSES = 7;

/**
 * Words of the check's instance records: terrain's, the stride scale in words 12 and 13 and its
 * edges' shared bands in words 20 to 67 (T3.c), its class sets' words 16 to 19 0.
 */
const INSTANCE_WORDS = 68;

/** The cascades' passes, the first four, and the ordered pass's index, after the camera's. */
const CASCADES = 4;
const ORDERED = CASCADES + 1;

/** The striding camera pass's index, the seventh (R14.T3.c): every instance enters it. */
const STRIDING = ORDERED + 1;

/**
 * The lit and the striding passes' τ′ for the count of strides drawn past it (R14.T3.e), rad: a
 * sixth to a half of the instances' own 1 ÷ 1,000 to 1 ÷ 300 rad (`smokeScaleOf`), so that a cluster
 * within 2 to 6.7 times its own need of the camera is counted and one farther is not.
 */
const PAST_TOLERANCE_RAD = 0.0005;

/** The second compaction's instances: more than its scan's 128 threads, several a thread. */
const MANY_INSTANCES = 1_300;

/** A seeded uniform generator in [0, 1) (mulberry32). */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** A random unit vector of `next`. */
function unit(next: () => number): Vec3 {
  for (;;) {
    const v = { x: 2 * next() - 1, y: 2 * next() - 1, z: 2 * next() - 1 };
    const n = Math.hypot(v.x, v.y, v.z);
    if (n > 0.1 && n <= 1) {
      return { x: v.x / n, y: v.y / n, z: v.z / n };
    }
  }
}

/** An orthonormal frame about a random axis, the light's x, y and z. */
function frame(next: () => number): readonly [Vec3, Vec3, Vec3] {
  const z = unit(next);
  const a = Math.abs(z.x) < 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 1, z: 0 };
  const xRaw = { x: a.y * z.z - a.z * z.y, y: a.z * z.x - a.x * z.z, z: a.x * z.y - a.y * z.x };
  const n = Math.hypot(xRaw.x, xRaw.y, xRaw.z);
  const x = { x: xRaw.x / n, y: xRaw.y / n, z: xRaw.z / n };
  const y = { x: z.y * x.z - z.z * x.y, y: z.z * x.x - z.x * x.z, z: z.x * x.y - z.y * x.x };
  return [x, y, z];
}

/** Random receivers of pass `k`, about the instances' depths; the second pass's without a cover. */
function receiversOf(next: () => number, k: number): CullReceivers {
  const near = 8_000 * (next() - 0.5);
  return {
    depthM: [near, near + 8_000 * next()],
    reachM: [500 * next(), 500 * next()],
    coveredBelow:
      k === 1
        ? null
        : {
            centreM: [4_000 * (next() - 0.5), 4_000 * (next() - 0.5)],
            halfSideM: 2_000 * 2 ** k * next(),
            reachM: 50 * next(),
          },
  };
}

/** The entries of every bucket of `twin`. */
function entriesOf(twin: ReadonlyMap<number, ReadonlyArray<number>>): number {
  return [...twin.values()].reduce((sum, list) => sum + list.length, 0);
}

/** The device's buckets: each bucket's entries, as the twin keys them, from its lists and counts. */
function deviceBuckets(
  args: Uint32Array,
  lists: Uint32Array,
  buckets: ClusterBuckets,
  templates: number,
): Map<number, number[]> {
  const out = new Map<number, number[]>();
  for (let pass = 0; pass < PASSES; pass += 1) {
    if (pass === ORDERED) {
      continue;
    }
    const place = buckets.place(pass);
    for (let t = 0; t < templates; t += 1) {
      const bucket = place.bucketBase + t;
      const count = args[bucket * 5 + 1] ?? 0;
      const from = place.listBase + (buckets.templateOffsets[t] ?? 0);
      const entries = [...lists.subarray(from, from + count)].map(
        (e) => (e >>> 8) * 256 + (e & 0xff),
      );
      if (entries.length > 0) {
        out.set(bucket, entries);
      }
    }
  }
  return out;
}

/**
 * The shape of the check's cluster `c` of a page: 64 blocks, 24 strips, then 4 fin runs and 4 head
 * runs (plan R14, R14.T1.g), so that every template of terrain's mesh is drawn.
 */
function smokeShapeOf(c: number): ClusterRecord["shape"] {
  return c < 64
    ? "terrain-block"
    : c < 88
      ? "terrain-skirt"
      : c < 92
        ? "terrain-fin"
        : "terrain-head";
}

/**
 * The template reference of the check's cluster `c` of a page (`record.ts`): a block's grid place,
 * a strip's edge, so that blocks and strips lie on every edge (R14.T3.c).
 */
function smokeTemplateRefOf(c: number): number {
  if (c < 64) {
    return 8 * (c % 8) + ((8 * Math.floor(c / 8)) << 8);
  }
  return c < 88 ? (c % 4) << 16 : 0;
}

/** The `Float16Array` scratch the records' errors are narrowed through. */
const HALF = new Float16Array(1);
const HALF_BITS = new Uint16Array(HALF.buffer);

/**
 * A record's random measured errors (R14.T3.c), the bits of e₁ to e₈: e₂ from 0.1 to 1.6 m rising
 * by 1.3 to 2.3 times a stride, so that under {@link smokeScaleOf}'s scales the clusters at up to
 * some 20 km take every stride; none for a tenth.
 */
function smokeErrorBitsOf(next: () => number): readonly [number, number, number, number] | null {
  if (next() < 0.1) {
    return null;
  }
  const bitsOf = (e: number): number => {
    HALF[0] = e;
    return HALF_BITS[0] ?? 0;
  };
  const two = 0.1 + 1.5 * next();
  const four = two * (1.3 + next());
  return [bitsOf(0.5 * two), bitsOf(two), bitsOf(four), bitsOf(four * (1.3 + next()))];
}

/** An instance's random stride scale (R14.T3.c): E_L ÷ τ′ 0.1 to 1.6 km, 1 ÷ τ′ 300 to 1,000; none for a tenth. */
function smokeScaleOf(next: () => number): StrideScale {
  if (next() < 0.1) {
    return NO_STRIDE_SCALE;
  }
  return { statedM: Math.fround(100 + 1_500 * next()), perErrorM: Math.fround(300 + 700 * next()) };
}

/**
 * An instance's random edges' shared bands, m (R14.T3.c): 0.2 to 12 km, a tenth none (as beside a
 * patch two levels apart) and a fiftieth +∞.
 */
function smokeEdgeBandsOf(next: () => number): Float64Array {
  return Float64Array.from({ length: EDGE_BANDS }, () => {
    const roll = next();
    return roll < 0.1
      ? NO_STRIDE_BAND
      : roll < 0.12
        ? Number.POSITIVE_INFINITY
        : 200 + 11_800 * next();
  });
}

/** R14.T1.b's cull on the device against its twin: see the module's remarks. */
export async function checkClusterCull(engine: RenderEngine, checks: Checks): Promise<void> {
  const next = random(0x5eed_14);
  // The camera strides' inputs from a generator of their own (R14.T3.c), so that every other
  // input is as it was drawn before them.
  const nextStride = random(0x5eed_3c);
  const { templates } = terrainClusterMesh();
  // The records' measured errors from a generator of their own too.
  const nextError = random(0x5eed_3f);
  const records: ClusterRecord[] = [];
  for (let n = 0; n < PAGES * CLUSTERS; n += 1) {
    records.push({
      centreM: { x: 4_000 * (next() - 0.5), y: 4_000 * (next() - 0.5), z: 4_000 * (next() - 0.5) },
      radiusM: 1 + 300 * next(),
      mode: "strided",
      strideErrorBits: smokeErrorBitsOf(nextError),
      templateRef: smokeTemplateRefOf(n % CLUSTERS),
      shape: smokeShapeOf(n % CLUSTERS),
      // Every fin and head run, and every fifth block and strip, may discard (R14.T1.g).
      mayDiscard: n % CLUSTERS >= 88 || n % 5 === 0,
      material: 0,
    });
  }
  // Each origin narrowed as an instance record holds it, so that the twin takes the device's.
  // The edges' bands from a generator of their own, as the strides' are.
  const nextEdge = random(0x5eed_3e);
  // The holds' fades from one of their own too (R14.T3.c's release fade).
  const nextFade = random(0x5eed_3f);
  const instances = Array.from({ length: INSTANCES }, (_, n) => {
    const bandsM = smokeEdgeBandsOf(nextEdge);
    return {
      originM: {
        x: Math.fround(30_000 * (next() - 0.5)),
        y: Math.fround(30_000 * (next() - 0.5)),
        z: Math.fround(30_000 * (next() - 0.5)),
      },
      page: n % PAGES,
      passMask: Math.floor(next() * 64) | (1 << STRIDING),
      unitM: 0.01 + next(),
      strideScale: smokeScaleOf(nextStride),
      bandsM,
      // The edges' bands as the record holds them and the kernel reads them back.
      edgeBandsM: heldEdgeBandsM(bandsM),
      // Every fifth instance at stride 1 (the forced region's), every third's edges at most at 4.
      // Every seventh a mask one to three levels down at a random part; of the rest every eleventh
      // a sub-patch on its own grid four to six down, kept whole and at its camera passes' cap
      // (R14.T3.c's stand-ins).
      subWord:
        n % 7 === 3
          ? (TERRAIN_SUB_PATCH_MASK |
              subPatchWord(
                1 + (n % 3),
                Math.floor(nextStride() * 2 ** (1 + (n % 3))),
                Math.floor(nextStride() * 2 ** (1 + (n % 3))),
              )) >>>
            0
          : smokeOwnGrid(n)
            ? subPatchWord(
                4 + (n % 3),
                Math.floor(nextStride() * 2 ** (4 + (n % 3))),
                Math.floor(nextStride() * 2 ** (4 + (n % 3))),
              )
            : 0,
      whole: smokeOwnGrid(n),
      quadrantWord:
        (n % 5 === 0 ? INSTANCE_FINEST : 0) |
        (smokeOwnGrid(n) ? INSTANCE_SUB_GRID_STRIDES : 0) |
        (n % 3 === 0 ? Math.floor(nextStride() * 16) << 2 : 0),
      // Every fourth's edges fading in or out at random, each byte 0 or 1 to 255.
      holdFadesWord:
        n % 4 === 1
          ? [0, 1, 2, 3].reduce(
              (word, e) =>
                (word | (nextFade() < 0.5 ? 0 : (1 + Math.floor(nextFade() * 255)) << (8 * e))) >>>
                0,
              0,
            )
          : 0,
    };
  });
  const axes = frame(next);
  const cascades: CascadeCullPass[] = Array.from({ length: CASCADES }, (_, k) => ({
    kind: "cascade",
    axes,
    centreFromCameraLightM: {
      x: 2_000 * (next() - 0.5),
      y: 2_000 * (next() - 0.5),
      z: 2_000 * (next() - 0.5),
    },
    radiusM: 2_000 * 2 ** k,
    depthFarM: 2_000 * 2 ** k,
    toleranceM: 4 * next(),
    strideCap: 8,
    receivers: k === 0 ? null : receiversOf(next, k),
  }));
  // A camera at the instances' origin, 60° across at 16:9, looking along a random axis (R14.T1.e).
  const [right, up, back] = frame(next);
  const camera: CameraCullPass = {
    kind: "camera",
    planes: frustumOf({
      orientation: quaternionFromRows([
        { x: right.x, y: up.x, z: back.x },
        { x: right.y, y: up.y, z: back.y },
        { x: right.z, y: up.z, z: back.z },
      ]),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 1920, heightPx: 1080 },
    }).planes,
    toleranceRad: 0,
    strideCap: 1,
    horizon: null,
    order: "appended",
  };
  // The lit pass: the same camera, ordered, with the horizon of an occluder 20 km in radius whose
  // near side lies 3 km away, 50° off its view axis, so that its cone (60° about its axis) holds one
  // side of the frustum and the clusters the frustum keeps beyond the occluder there are refused by
  // it, the others kept (R14.T2.a).
  const off = (50 * Math.PI) / 180;
  const toCamera = {
    x: 23_000 * (Math.cos(off) * back.x + Math.sin(off) * right.x),
    y: 23_000 * (Math.cos(off) * back.y + Math.sin(off) * right.y),
    z: 23_000 * (Math.cos(off) * back.z + Math.sin(off) * right.z),
  };
  const lit: CameraCullPass = {
    ...camera,
    planes: camera.planes.slice(0, 5),
    horizon: cullHorizonOf(toCamera, 20_000),
    order: "ordered",
    strideCap: 8,
    toleranceRad: PAST_TOLERANCE_RAD,
  };
  // A camera pass of no planes, which keeps every cluster, at strides up to 8 by its instances'
  // bands (R14.T3.c).
  const striding: CameraCullPass = {
    kind: "camera",
    planes: [],
    toleranceRad: PAST_TOLERANCE_RAD,
    strideCap: 8,
    horizon: null,
    order: "appended",
  };
  const passes: CullPass[] = [...cascades, camera, lit, striding];
  const viewAxis = unit(next);
  const buckets = new ClusterBuckets(
    engine,
    "smoke clusters",
    PASSES,
    templates,
    templates.map(() => INSTANCES * CLUSTERS),
  );
  const made: BufferHandle[] = [];
  const buffer = (name: string, data: ArrayBufferView): BufferHandle => {
    const handle = engine.createBuffer({
      name,
      bytes: data.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "geometry-clusters",
    });
    made.push(handle);
    engine.writeBuffer(handle, 0, data);
    return handle;
  };
  const kernel = await engine.createComputeAsync(CLUSTER_CULL_KERNEL);
  const compact = await engine.createComputeAsync(CLUSTER_COMPACT);
  const marks = engine.createBuffer({
    name: "smoke clusters marks",
    bytes: INSTANCES * CLUSTERS * 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "geometry-clusters",
  });
  made.push(marks);
  const tallies = engine.createBuffer({
    name: "smoke clusters tallies",
    bytes: MANY_INSTANCES * COMPACT_TEMPLATES * 4,
    usage: BUFFER_USAGE.STORAGE,
    category: "geometry-clusters",
  });
  made.push(tallies);
  try {
    const recordBytes = new DataView(new ArrayBuffer(records.length * CLUSTER_RECORD_BYTES));
    records.forEach((record, n) => {
      packClusterRecord(recordBytes, n * CLUSTER_RECORD_BYTES, record);
    });
    const bytes = 4 * INSTANCE_WORDS;
    const words = new DataView(new ArrayBuffer(INSTANCES * bytes));
    const inputs = new Uint32Array(2 * INSTANCES);
    instances.forEach((instance, n) => {
      words.setFloat32(bytes * n, instance.originM.x, true);
      words.setFloat32(bytes * n + 4, instance.originM.y, true);
      words.setFloat32(bytes * n + 8, instance.originM.z, true);
      words.setUint32(bytes * n + 12, instance.page, true);
      words.setUint32(bytes * n + 28, instance.quadrantWord, true);
      words.setUint32(bytes * n + 32, instance.subWord, true);
      words.setUint32(bytes * n + 56, instance.holdFadesWord, true);
      words.setFloat32(bytes * n + 48, instance.strideScale.statedM, true);
      words.setFloat32(bytes * n + 52, instance.strideScale.perErrorM, true);
      for (let h = 0; h < EDGE_BANDS; h += 1) {
        words.setUint16(bytes * n + 80 + 2 * h, edgeBandHalf(instance.bandsM[h] ?? -1), true);
      }
      inputs[2 * n] = (instance.passMask | (instance.whole ? CULL_INPUT_WHOLE : 0)) >>> 0;
      inputs[2 * n + 1] = new Uint32Array(Float32Array.of(instance.unitM).buffer)[0] ?? 0;
    });
    const passBytes = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    const places = passes.map((_, k) => buckets.place(k));
    packCullPasses(passes, places, passBytes);
    const identity: readonly [Vec3, Vec3, Vec3] = [
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 1, z: 0 },
      { x: 0, y: 0, z: 1 },
    ];
    const paramBytes = new ArrayBuffer(CULL_PARAMS_BYTES);
    packCullParams(
      {
        rotationColumns: identity,
        instances: INSTANCES,
        instanceWords: INSTANCE_WORDS,
        clustersPerPage: CLUSTERS,
        passes: PASSES,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis,
        firstShape: 0,
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
        overWord: buckets.overWord,
      },
      paramBytes,
    );
    buckets.clear();
    engine.dispatch(
      kernel,
      {
        uniforms: { params: new Uint32Array(paramBytes), passes: new Uint32Array(passBytes) },
        buffers: {
          instance_words: buffer("smoke clusters instances", words),
          cull_inputs: buffer("smoke clusters inputs", inputs),
          records: buffer("smoke clusters records", recordBytes),
          lists: buckets.lists,
          args: buckets.args,
          marks,
          // No record here is a DAG's (R14.T4.b): one LOD sphere, unread.
          lods: buffer("smoke clusters lods", new Float32Array(4)),
        },
        sampled: {},
        storage: {},
      },
      [INSTANCES, 1, 1],
      "smoke cluster cull",
    );
    const deviceMarks = new Uint32Array(await engine.readBuffer(marks, "tolerance"));
    compactOnDevice(engine, compact, {
      marks,
      tallies,
      buckets,
      pass: ORDERED,
      instances: INSTANCES,
      clusters: CLUSTERS,
    });
    const args = new Uint32Array(await engine.readBuffer(buckets.args, "tolerance"));
    const lists = new Uint32Array(await engine.readBuffer(buckets.lists, "tolerance"));
    const device = deviceBuckets(args, lists, buckets, templates.length);
    const input: CullTwinInput = {
      instances,
      records: (page, cluster) => {
        const at = (page * CLUSTERS + cluster) * CLUSTER_RECORD_BYTES;
        const record = records[page * CLUSTERS + cluster] ?? records[0];
        if (record === undefined) {
          throw new Error("the check has no records");
        }
        return {
          ...record,
          centreM: {
            x: recordBytes.getFloat32(at, true),
            y: recordBytes.getFloat32(at + 4, true),
            z: recordBytes.getFloat32(at + 8, true),
          },
          radiusM: recordBytes.getFloat32(at + 12, true),
        };
      },
      clustersPerPage: CLUSTERS,
      rotationColumns: identity,
      passes,
      places,
      templates: templates.length,
      viewAxis,
      discardTemplates: TERRAIN_DISCARD_TEMPLATES,
    };
    const exact = cullTwin(input, 0);
    const widened = cullTwin(input, 2);
    const ordered = await orderedCheck(engine, buckets, input, deviceMarks);
    const boxes: CullPass[] = [];
    for (const pass of passes) {
      boxes.push(pass.kind === "cascade" ? { ...pass, receivers: null } : pass);
    }
    const byReceivers = entriesOf(cullTwin({ ...input, passes: boxes }, 0)) - entriesOf(exact);
    let missing = 0;
    let extra = 0;
    let kept = 0;
    const filled = new Set<number>();
    for (const [bucket, entries] of exact) {
      const held = new Set(device.get(bucket) ?? []);
      missing += entries.filter((entry) => !held.has(entry)).length;
    }
    for (const [bucket, entries] of device) {
      const allowed = new Set(widened.get(bucket) ?? []);
      extra += entries.filter((entry) => !allowed.has(entry)).length;
      kept += entries.length;
      filled.add(bucket % templates.length);
      if (new Set(entries).size !== entries.length) {
        extra += 1;
      }
    }
    checks.check(
      "R14.T1.b the cluster cull on the device keeps every cluster its twin's exact test keeps, and none its widened test refuses, its receivers' cull (R14.T1.c) on three of its passes and a camera's frustum (R14.T1.e) its fifth, each may-discard record in its shape's may-discard bucket (R14.T1.g)",
      missing === 0 &&
        extra === 0 &&
        kept > 0 &&
        filled.size === templates.length &&
        byReceivers > 0,
      `${String(kept)} entries kept of ${String(INSTANCES * CLUSTERS * (PASSES - 1))} tested, in ${String(filled.size)} of ${String(templates.length)} templates, ${String(byReceivers)} refused by the receivers' depths; ${String(missing)} the exact twin keeps missing, ${String(extra)} past the widened twin`,
    );
    const many = await manyCheck(engine, compact, tallies, next);
    // The camera's strides (R14.T3.c): the striding pass's buckets, held to its twin above, at
    // every strided template of both shapes, and none past what the instances hold them to.
    const stridingPlace = buckets.place(STRIDING);
    const strided = [...device.entries()].flatMap(([bucket, entries]) => {
      const t = bucket - stridingPlace.bucketBase;
      return t >= 0 && t < 8 ? entries.map((entry) => [t, entry] as const) : [];
    });
    const taken = new Set(strided.map(([t]) => t));
    const held = strided.filter(([t, entry]) => {
      const instance = instances[Math.floor(entry / 256)];
      const level = t % 4;
      return (
        instance !== undefined &&
        (((instance.quadrantWord & INSTANCE_FINEST) !== 0 && level > 0) ||
          (instance.strideScale.perErrorM <= 0 && !instance.whole && level > 0))
      );
    }).length;
    checks.check(
      "R14.T3.c a camera pass's strides on the device are its twin's: every stride of both shapes taken, none past an instance held at stride 1 or of no bands",
      missing === 0 && extra === 0 && taken.size === 8 && held === 0,
      `the striding pass's ${String(strided.length)} entries at templates ${[...taken].toSorted((a, b) => a - b).join(", ")} of terrain's 8 strided ones, ${String(held)} past their instances' hold; the lit pass's marks at ${[...ordered.strides].toSorted((a, b) => a - b).join(", ")}; ${String(missing)} the exact twin keeps missing, ${String(extra)} past the widened twin`,
    );
    // The sub-patches on their own grids (R14.T3.c's stand-ins): every block and strip of each in
    // the striding pass at its cap, stride 8, whatever its distance, but the forced region's at 1.
    const ownGrid = strided.filter(([, entry]) => smokeOwnGrid(Math.floor(entry / 256)));
    const offCap = ownGrid.filter(([t, entry]) => {
      const finest =
        ((instances[Math.floor(entry / 256)]?.quadrantWord ?? 0) & INSTANCE_FINEST) !== 0;
      return t % 4 !== (finest ? 0 : 3);
    }).length;
    checks.check(
      "R14.T3.c's stand-ins: a camera pass draws each block and strip of a sub-patch on its own grid at its cap on the device, as its twin does, and the forced region's at stride 1",
      missing === 0 && extra === 0 && ownGrid.some(([t]) => t % 4 === 3) && offCap === 0,
      `${String(ownGrid.length)} of the striding pass's entries of sub-patches on their own grids, ${String(offCap)} at another stride; ${String(missing)} the exact twin keeps missing, ${String(extra)} past the widened twin`,
    );
    // The strides drawn past τ′ (R14.T3.e), each pass's word past its buckets, as the compaction
    // left them, between its twin's over the exact and the doubly widened tests' clusters.
    const pastExact = cullOverTwin(input, 0);
    const pastWidened = cullOverTwin(input, 2);
    const past = passes.map((_, pass) => args[buckets.overWord + pass] ?? 0);
    const pastHeld = past.every(
      (count, pass) => count >= (pastExact.get(pass) ?? 0) && count <= (pastWidened.get(pass) ?? 0),
    );
    checks.check(
      "R14.T3.e the cull counts on the device the strides its camera passes draw past their τ′, as its twin does, and none in a cascade or a pass at stride 1",
      pastHeld && (past[STRIDING] ?? 0) > 0 && past.slice(0, ORDERED).every((count) => count === 0),
      `by pass ${past.join(", ")}; the exact twin's ${passes.map((_, pass) => String(pastExact.get(pass) ?? 0)).join(", ")}, the widened twin's ${passes.map((_, pass) => String(pastWidened.get(pass) ?? 0)).join(", ")}`,
    );
    await edgeStandInCheck(engine, kernel, checks);
    checks.check(
      "R14.T2.a the lit pass's marks on the device are its twin's, its horizon refusing some, and the stable compaction writes its twin's lists byte for byte, in the draw set's order",
      ordered.missing === 0 &&
        ordered.extra === 0 &&
        ordered.kept > 0 &&
        ordered.byHorizon > 0 &&
        ordered.differing === 0 &&
        many.differing === 0 &&
        many.entries > 0,
      `the lit pass: ${String(ordered.kept)} clusters marked of ${String(INSTANCES * CLUSTERS)}, ${String(ordered.byHorizon)} refused by the horizon alone; ${String(ordered.missing)} the exact twin marks missing, ${String(ordered.extra)} past the widened twin; its compaction ${String(ordered.entries)} entries, ${String(ordered.differing)} differing from the twin's; over ${String(MANY_INSTANCES)} instances of random marks ${String(many.entries)} entries, ${String(many.differing)} differing`,
    );
  } finally {
    for (const handle of made) {
      engine.releaseBuffer(handle);
    }
    buckets.dispose();
    engine.releaseCompute(kernel);
    engine.releaseCompute(compact);
  }
}

/** What the stable compaction is dispatched over: the marks, the tallies and a pass's buckets. */
interface CompactRun {
  readonly marks: BufferHandle;
  readonly tallies: BufferHandle;
  readonly buckets: ClusterBuckets;
  readonly pass: number;
  readonly instances: number;
  readonly clusters: number;
}

/** Dispatches the stable compaction's count, scan and scatter over `run` (R14.T2.a). */
function compactOnDevice(engine: RenderEngine, compact: ComputeHandle, run: CompactRun): void {
  const place = run.buckets.place(run.pass);
  for (const phase of COMPACT_PHASES) {
    engine.dispatch(
      compact,
      {
        uniforms: {
          params: packCompactParams({
            instances: run.instances,
            clustersPerPage: run.clusters,
            templates: run.buckets.templateOffsets.length,
            phase,
            bucketBase: place.bucketBase,
            listBase: place.listBase,
            templateOffsets: run.buckets.templateOffsets,
          }),
        },
        buffers: {
          marks: run.marks,
          tallies: run.tallies,
          lists: run.buckets.lists,
          args: run.buckets.args,
        },
        sampled: {},
        storage: {},
      },
      compactWorkgroups(phase, run.instances),
      "smoke cluster compact",
    );
  }
}

/** Pass `pass`'s buckets on the device, each template's entries in their list's order. */
async function deviceLists(
  engine: RenderEngine,
  buckets: ClusterBuckets,
  pass: number,
): Promise<number[][]> {
  const args = new Uint32Array(await engine.readBuffer(buckets.args, "tolerance"));
  const lists = new Uint32Array(await engine.readBuffer(buckets.lists, "tolerance"));
  const place = buckets.place(pass);
  return Array.from({ length: buckets.templateOffsets.length }, (_, t) => {
    const count = args[(place.bucketBase + t) * 5 + 1] ?? 0;
    const from = place.listBase + (buckets.templateOffsets[t] ?? 0);
    return [...lists.subarray(from, from + count)].map((e) => (e >>> 8) * 256 + (e & 0xff));
  });
}

/** The entries of `device` that differ from `twin`'s, place by place, and their total. */
function listsDiffer(
  device: ReadonlyArray<ReadonlyArray<number>>,
  twin: ReadonlyArray<ReadonlyArray<number>>,
): { readonly differing: number; readonly entries: number } {
  let differing = 0;
  let entries = 0;
  twin.forEach((list, t) => {
    const held = device[t] ?? [];
    entries += held.length;
    differing += Math.abs(held.length - list.length);
    list.forEach((entry, k) => {
      differing += k < held.length && held[k] !== entry ? 1 : 0;
    });
  });
  return { differing, entries };
}

/**
 * The ordered pass's marks on the device against its twin's exact and widened marks, those its
 * horizon alone refuses, and its compaction's lists against the twin's over the device's marks.
 */
async function orderedCheck(
  engine: RenderEngine,
  buckets: ClusterBuckets,
  input: CullTwinInput,
  deviceMarks: Uint32Array,
): Promise<{
  readonly kept: number;
  readonly missing: number;
  readonly extra: number;
  readonly byHorizon: number;
  readonly differing: number;
  readonly entries: number;
  readonly strides: ReadonlySet<number>;
}> {
  const exact = cullMarksTwin(input, 0);
  const widened = cullMarksTwin(input, 2);
  const noHorizon = cullMarksTwin(
    {
      ...input,
      passes: input.passes.map((pass) =>
        pass?.kind === "camera" && pass.order === "ordered" ? { ...pass, horizon: null } : pass,
      ),
    },
    0,
  );
  let kept = 0;
  let missing = 0;
  let extra = 0;
  let byHorizon = 0;
  const strides = new Set<number>();
  deviceMarks.forEach((mark, at) => {
    kept += mark === NO_MARK ? 0 : 1;
    if (mark < 8) {
      strides.add(mark);
    }
    missing += exact[at] !== NO_MARK && mark !== exact[at] ? 1 : 0;
    extra += mark !== NO_MARK && widened[at] !== mark ? 1 : 0;
    byHorizon += noHorizon[at] !== NO_MARK && exact[at] === NO_MARK ? 1 : 0;
  });
  const twin = compactTwin(deviceMarks, INSTANCES, CLUSTERS, buckets.templateOffsets.length);
  return {
    kept,
    missing,
    extra,
    byHorizon,
    strides,
    ...listsDiffer(await deviceLists(engine, buckets, ORDERED), twin.lists),
  };
}

/** The stable compaction over {@link MANY_INSTANCES} instances of random marks against its twin. */
async function manyCheck(
  engine: RenderEngine,
  compact: ComputeHandle,
  tallies: BufferHandle,
  next: () => number,
): Promise<{ readonly differing: number; readonly entries: number }> {
  const { templates } = terrainClusterMesh();
  const marksData = Uint32Array.from({ length: MANY_INSTANCES * CLUSTERS }, () =>
    next() < 0.5 ? NO_MARK : Math.floor(next() * templates.length),
  );
  const marks = engine.createBuffer({
    name: "smoke compaction marks",
    bytes: marksData.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "geometry-clusters",
  });
  // A bucket a template with room for every cluster of every instance, whatever its shape.
  const buckets = new ClusterBuckets(
    engine,
    "smoke compaction",
    1,
    templates,
    templates.map(() => MANY_INSTANCES * CLUSTERS),
  );
  try {
    engine.writeBuffer(marks, 0, marksData);
    buckets.clear();
    compactOnDevice(engine, compact, {
      marks,
      tallies,
      buckets,
      pass: 0,
      instances: MANY_INSTANCES,
      clusters: CLUSTERS,
    });
    const twin = compactTwin(marksData, MANY_INSTANCES, CLUSTERS, templates.length);
    return listsDiffer(await deviceLists(engine, buckets, 0), twin.lists);
  } finally {
    engine.releaseBuffer(marks);
    buckets.dispose();
  }
}

/** The edge stand-ins' check's pages, of terrain's own layout (R14.T3.c's edge stand-ins). */
const EDGE_PAGES = 2;

/** The shape of cluster `c` of a terrain page: its blocks, strips, head runs, then fin runs. */
function terrainShapeOf(c: number): ClusterRecord["shape"] {
  return c < TERRAIN_BLOCKS
    ? "terrain-block"
    : c < TERRAIN_CLUSTERS_PER_PAGE
      ? "terrain-skirt"
      : c < TERRAIN_CLUSTERS_PER_PAGE + TERRAIN_HEAD_CLUSTERS
        ? "terrain-head"
        : "terrain-fin";
}

/** One of the edge stand-ins' check's instances: its page, sub-patch word and cull input's bits. */
interface EdgeStandInInstance {
  readonly page: number;
  readonly subWord: number;
  readonly bits: number;
  readonly whole: boolean;
}

/** A stand-in's mask's sub-patch word, `levels` down at part (i, j). */
function mask(levels: number, i: number, j: number): number {
  return (TERRAIN_SUB_PATCH_MASK | subPatchWord(levels, i, j)) >>> 0;
}

/**
 * The edge stand-ins' check's instances: page 0's four edge masks, their side runs shared; page 1's
 * edge mask alone, its masks on opaque blocks one to three levels down, its own grid on opaque blocks,
 * kept whole, an interior bake's mask and its whole patch.
 */
function edgeStandIns(): EdgeStandInInstance[] {
  const out: EdgeStandInInstance[] = [];
  for (const [qx, qy] of [
    [0, 0],
    [1, 0],
    [0, 1],
    [1, 1],
  ] as const) {
    out.push({
      page: 0,
      subWord: mask(1, qx, qy),
      bits:
        TERRAIN_INPUT_EDGE_MASK |
        (qx === 1 ? TERRAIN_INPUT_COLUMN_RUN_SHARED : 0) |
        (qy === 1 ? TERRAIN_INPUT_ROW_RUN_SHARED : 0),
      whole: false,
    });
  }
  out.push(
    { page: 1, subWord: mask(1, 1, 1), bits: TERRAIN_INPUT_EDGE_MASK, whole: false },
    { page: 1, subWord: mask(1, 0, 1), bits: TERRAIN_INPUT_SURVEYED, whole: false },
    { page: 1, subWord: mask(2, 3, 0), bits: TERRAIN_INPUT_SURVEYED, whole: false },
    { page: 1, subWord: mask(3, 5, 6), bits: TERRAIN_INPUT_SURVEYED, whole: false },
    { page: 1, subWord: subPatchWord(4, 9, 2), bits: TERRAIN_INPUT_SURVEYED, whole: true },
    { page: 1, subWord: mask(2, 1, 2), bits: 0, whole: false },
    { page: 0, subWord: 0, bits: 0, whole: false },
  );
  return out;
}

/** The edge stand-ins on the device against their twin (R14.T3.c): see the module's remarks. */
async function edgeStandInCheck(
  engine: RenderEngine,
  kernel: ComputeHandle,
  checks: Checks,
): Promise<void> {
  const next = random(0x5eed_e5);
  const { templates } = terrainClusterMesh();
  const records: ClusterRecord[] = [];
  for (let n = 0; n < EDGE_PAGES * TERRAIN_PAGE_CLUSTERS; n += 1) {
    const c = n % TERRAIN_PAGE_CLUSTERS;
    const roll = next();
    const run = c >= TERRAIN_CLUSTERS_PER_PAGE;
    records.push({
      centreM: { x: 2_000 * (next() - 0.5), y: 2_000 * (next() - 0.5), z: 2_000 * (next() - 0.5) },
      // A sixth no cluster, a third of the blocks and strips may discard, every run may.
      radiusM: roll < 1 / 6 ? -1 : 1 + 300 * next(),
      mode: "strided",
      strideErrorBits: null,
      templateRef: run ? terrainFinTemplateRef(c) : terrainTemplateRef(c),
      shape: terrainShapeOf(c),
      mayDiscard: run || roll > 2 / 3,
      material: 0,
    });
  }
  const standIns = edgeStandIns();
  const instances = standIns.map((standIn, n) => ({
    originM: { x: Math.fround(100 * n), y: 0, z: Math.fround(-50 * n) },
    page: standIn.page,
    passMask: 1,
    unitM: 1,
    subWord: standIn.subWord,
    standInBits: standIn.bits,
    whole: standIn.whole,
  }));
  // One camera pass of no planes at stride 1, which keeps every cluster of every instance.
  const pass: CameraCullPass = {
    kind: "camera",
    planes: [],
    toleranceRad: 0,
    strideCap: 1,
    horizon: null,
    order: "appended",
  };
  const buckets = new ClusterBuckets(
    engine,
    "smoke edge stand-ins",
    1,
    templates,
    templates.map(() => instances.length * TERRAIN_PAGE_CLUSTERS),
  );
  const made: BufferHandle[] = [];
  const buffer = (name: string, data: ArrayBufferView): BufferHandle => {
    const handle = engine.createBuffer({
      name,
      bytes: data.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "geometry-clusters",
    });
    made.push(handle);
    engine.writeBuffer(handle, 0, data);
    return handle;
  };
  try {
    const recordBytes = new DataView(new ArrayBuffer(records.length * CLUSTER_RECORD_BYTES));
    records.forEach((record, n) => {
      packClusterRecord(recordBytes, n * CLUSTER_RECORD_BYTES, record);
    });
    const bytes = 4 * INSTANCE_WORDS;
    const words = new DataView(new ArrayBuffer(instances.length * bytes));
    const inputs = new Uint32Array(2 * instances.length);
    instances.forEach((instance, n) => {
      words.setFloat32(bytes * n, instance.originM.x, true);
      words.setFloat32(bytes * n + 4, instance.originM.y, true);
      words.setFloat32(bytes * n + 8, instance.originM.z, true);
      words.setUint32(bytes * n + 12, instance.page, true);
      words.setUint32(bytes * n + 32, instance.subWord, true);
      for (let h = 0; h < EDGE_BANDS; h += 1) {
        words.setUint16(bytes * n + 80 + 2 * h, edgeBandHalf(-1), true);
      }
      inputs[2 * n] =
        (instance.passMask | instance.standInBits | (instance.whole ? CULL_INPUT_WHOLE : 0)) >>> 0;
      inputs[2 * n + 1] = new Uint32Array(Float32Array.of(instance.unitM).buffer)[0] ?? 0;
    });
    const passBytes = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    const places = [buckets.place(0)];
    packCullPasses([pass], places, passBytes);
    const identity: readonly [Vec3, Vec3, Vec3] = [
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 1, z: 0 },
      { x: 0, y: 0, z: 1 },
    ];
    const paramBytes = new ArrayBuffer(CULL_PARAMS_BYTES);
    packCullParams(
      {
        rotationColumns: identity,
        instances: instances.length,
        instanceWords: INSTANCE_WORDS,
        clustersPerPage: TERRAIN_PAGE_CLUSTERS,
        passes: 1,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis: { x: 0, y: 0, z: 1 },
        firstShape: 0,
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
      },
      paramBytes,
    );
    const marks = engine.createBuffer({
      name: "smoke edge stand-ins marks",
      bytes: instances.length * TERRAIN_PAGE_CLUSTERS * 4,
      usage: BUFFER_USAGE.STORAGE,
      category: "geometry-clusters",
    });
    made.push(marks);
    buckets.clear();
    engine.dispatch(
      kernel,
      {
        uniforms: { params: new Uint32Array(paramBytes), passes: new Uint32Array(passBytes) },
        buffers: {
          instance_words: buffer("smoke edge stand-ins instances", words),
          cull_inputs: buffer("smoke edge stand-ins inputs", inputs),
          records: buffer("smoke edge stand-ins records", recordBytes),
          lists: buckets.lists,
          args: buckets.args,
          marks,
          lods: buffer("smoke edge stand-ins lods", new Float32Array(4)),
        },
        sampled: {},
        storage: {},
      },
      [instances.length, 1, 1],
      "smoke edge stand-ins cull",
    );
    const lists = await deviceLists(engine, buckets, 0);
    const twin = cullTwin(
      {
        instances,
        records: (page, cluster) => {
          const record = records[page * TERRAIN_PAGE_CLUSTERS + cluster];
          if (record === undefined) {
            throw new Error(`the check has no record ${String(cluster)} of page ${String(page)}`);
          }
          const at = (page * TERRAIN_PAGE_CLUSTERS + cluster) * CLUSTER_RECORD_BYTES;
          return {
            ...record,
            centreM: {
              x: recordBytes.getFloat32(at, true),
              y: recordBytes.getFloat32(at + 4, true),
              z: recordBytes.getFloat32(at + 8, true),
            },
            radiusM: recordBytes.getFloat32(at + 12, true),
          };
        },
        clustersPerPage: TERRAIN_PAGE_CLUSTERS,
        rotationColumns: identity,
        passes: [pass],
        places,
        templates: templates.length,
        viewAxis: { x: 0, y: 0, z: 1 },
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
      },
      0,
    );
    // The device's buckets against the twin's, as sets: the kernel appends in any order.
    let differing = 0;
    lists.forEach((list, t) => {
      const want = new Set(twin.get((places[0]?.bucketBase ?? 0) + t) ?? []);
      const got = new Set(list);
      differing += [...got].filter((e) => !want.has(e)).length;
      differing += [...want].filter((e) => !got.has(e)).length;
    });
    // What each instance drew on the device, cluster by cluster, and whether in a may-discard bucket.
    const drawn = standIns.map(() => new Map<number, boolean>());
    lists.forEach((list, t) => {
      for (const entry of list) {
        drawn[Math.floor(entry / 256)]?.set(entry % 256, templates[t]?.mayDiscard === true);
      }
    });
    const real = (page: number, c: number): boolean =>
      (records[page * TERRAIN_PAGE_CLUSTERS + c]?.radiusM ?? -1) >= 0;
    let wrong = 0;
    const sideRuns = new Map<number, number>();
    standIns.forEach((standIn, n) => {
      const got = drawn[n] ?? new Map<number, boolean>();
      const want = new Map<number, boolean>();
      const runs = terrainMaskRuns(standIn.subWord, standIn.bits);
      for (let c = 0; c < TERRAIN_PAGE_CLUSTERS; c += 1) {
        const record = records[standIn.page * TERRAIN_PAGE_CLUSTERS + c];
        const masked = terrainMaskCluster(standIn.subWord, c, standIn.bits);
        if (masked === "none") {
          continue;
        }
        if ((standIn.bits & TERRAIN_INPUT_SURVEYED) !== 0) {
          // Its blocks and strips, every one opaque; no run.
          if (c < TERRAIN_CLUSTERS_PER_PAGE) {
            want.set(c, false);
          }
        } else if ((standIn.bits & TERRAIN_INPUT_EDGE_MASK) !== 0) {
          // Its blocks of a cluster by their classes, its strips always may-discard, its runs.
          if (c >= TERRAIN_BLOCKS && c < TERRAIN_CLUSTERS_PER_PAGE) {
            want.set(c, true);
          } else if ((c < TERRAIN_BLOCKS || runs.includes(c)) && real(standIn.page, c)) {
            want.set(c, record?.mayDiscard === true);
          }
        } else if (real(standIn.page, c)) {
          want.set(c, record?.mayDiscard === true);
        }
      }
      wrong += [...want].filter(([c, discards]) => got.get(c) !== discards).length;
      wrong += [...got.keys()].filter((c) => !want.has(c)).length;
      if (standIn.page === 0 && (standIn.bits & TERRAIN_INPUT_EDGE_MASK) !== 0) {
        for (const c of [106, 107, 112, 113]) {
          sideRuns.set(c, (sideRuns.get(c) ?? 0) + (got.has(c) ? 1 : 0));
        }
      }
    });
    // Each of page 0's side runs of a cluster drawn by exactly one of its masks.
    const twice = [...sideRuns].filter(([c, times]) => times !== (real(0, c) ? 1 : 0)).length;
    const edgeRuns = standIns.reduce(
      (sum, standIn, n) =>
        sum +
        [...(drawn[n]?.keys() ?? [])].filter(
          (c) => c >= TERRAIN_CLUSTERS_PER_PAGE && (standIn.bits & TERRAIN_INPUT_EDGE_MASK) !== 0,
        ).length,
      0,
    );
    checks.check(
      "R14.T3.c's edge stand-ins: the cull on the device draws an edge mask's blocks of a cluster, its 16 strips by the may-discard template and its quadrant's runs, a shared side run once, and a stand-in on opaque blocks as an interior bake's, as its twin does",
      differing === 0 && wrong === 0 && twice === 0 && edgeRuns > 0,
      `${String(lists.reduce((sum, list) => sum + list.length, 0))} entries of ${String(standIns.length)} instances, ${String(edgeRuns)} of them edge masks' runs; ${String(differing)} differing from the twin's, ${String(wrong)} from each form's own, ${String(twice)} side runs not drawn once`,
    );
  } finally {
    for (const handle of made) {
      engine.releaseBuffer(handle);
    }
    buckets.dispose();
  }
}

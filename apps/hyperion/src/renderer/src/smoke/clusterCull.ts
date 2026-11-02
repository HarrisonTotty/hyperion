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
  cullMarksTwin,
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
import { TERRAIN_DISCARD_TEMPLATES, terrainClusterMesh } from "../view/clusters/templates";
import { quaternionFromRows } from "../view/camera/quaternion";
import { frustumOf } from "../view/terrain/cull";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../view/engine/types";
import type { Checks } from "./harness";

/**
 * The check's instances, pages, clusters a page and passes: four cascades, a camera, and the lit
 * pass's ordered camera (R14.T2.a), the sixth.
 */
const INSTANCES = 48;
const PAGES = 12;
const CLUSTERS = 96;
const PASSES = 6;

/** The cascades' passes, the first four, and the ordered pass's index, after the camera's. */
const CASCADES = 4;
const ORDERED = CASCADES + 1;

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

/** R14.T1.b's cull on the device against its twin: see the module's remarks. */
export async function checkClusterCull(engine: RenderEngine, checks: Checks): Promise<void> {
  const next = random(0x5eed_14);
  const { templates } = terrainClusterMesh();
  const records: ClusterRecord[] = [];
  for (let n = 0; n < PAGES * CLUSTERS; n += 1) {
    records.push({
      centreM: { x: 4_000 * (next() - 0.5), y: 4_000 * (next() - 0.5), z: 4_000 * (next() - 0.5) },
      radiusM: 1 + 300 * next(),
      mode: "strided",
      strideErrorBits: null,
      templateRef: 0,
      shape: smokeShapeOf(n % CLUSTERS),
      // Every fin and head run, and every fifth block and strip, may discard (R14.T1.g).
      mayDiscard: n % CLUSTERS >= 88 || n % 5 === 0,
      material: 0,
    });
  }
  // Each origin narrowed as an instance record holds it, so that the twin takes the device's.
  const instances = Array.from({ length: INSTANCES }, (_, n) => ({
    originM: {
      x: Math.fround(30_000 * (next() - 0.5)),
      y: Math.fround(30_000 * (next() - 0.5)),
      z: Math.fround(30_000 * (next() - 0.5)),
    },
    page: n % PAGES,
    passMask: Math.floor(next() * 64),
    unitM: 0.01 + next(),
  }));
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
  };
  const passes: CullPass[] = [...cascades, camera, lit];
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
    const words = new DataView(new ArrayBuffer(INSTANCES * 32));
    const inputs = new Uint32Array(2 * INSTANCES);
    instances.forEach((instance, n) => {
      words.setFloat32(32 * n, instance.originM.x, true);
      words.setFloat32(32 * n + 4, instance.originM.y, true);
      words.setFloat32(32 * n + 8, instance.originM.z, true);
      words.setUint32(32 * n + 12, instance.page, true);
      inputs[2 * n] = instance.passMask;
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
        instanceWords: 8,
        clustersPerPage: CLUSTERS,
        passes: PASSES,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis,
        firstShape: 0,
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
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
  deviceMarks.forEach((mark, at) => {
    kept += mark === NO_MARK ? 0 : 1;
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

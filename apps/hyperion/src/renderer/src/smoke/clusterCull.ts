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
 */

import type { Vec3 } from "../geometry/vec3";
import { ClusterBuckets } from "../view/clusters/buckets";
import { CLUSTER_CULL_KERNEL, cullTwin, type CullTwinInput } from "../view/clusters/cull";
import {
  type CameraCullPass,
  type CascadeCullPass,
  type CullPass,
  type CullReceivers,
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  MAX_CULL_PASSES,
  packCullParams,
  packCullPasses,
} from "../view/clusters/cullPass";
import {
  CLUSTER_RECORD_BYTES,
  type ClusterRecord,
  packClusterRecord,
} from "../view/clusters/record";
import { terrainClusterMesh } from "../view/clusters/templates";
import { quaternionFromRows } from "../view/camera/quaternion";
import { frustumOf } from "../view/terrain/cull";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, RenderEngine } from "../view/engine/types";
import type { Checks } from "./harness";

/** The check's instances, pages, clusters a page and passes: four cascades and a camera. */
const INSTANCES = 48;
const PAGES = 12;
const CLUSTERS = 96;
const PASSES = 5;

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
      errorsM: null,
      templateRef: 0,
      shape: n % CLUSTERS < 64 ? "terrain-block" : "terrain-skirt",
      mayDiscard: false,
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
    passMask: Math.floor(next() * 32),
    unitM: 0.01 + next(),
  }));
  const axes = frame(next);
  const cascades: CascadeCullPass[] = Array.from({ length: PASSES - 1 }, (_, k) => ({
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
    toleranceM: 0,
    strideCap: 1,
  };
  const passes: CullPass[] = [...cascades, camera];
  const viewAxis = unit(next);
  const buckets = new ClusterBuckets(
    engine,
    "smoke clusters",
    PASSES,
    templates,
    templates.map((t) => INSTANCES * (t.shape === "terrain-block" ? 64 : 32)),
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
        },
        sampled: {},
        storage: {},
      },
      [INSTANCES, 1, 1],
      "smoke cluster cull",
    );
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
    };
    const exact = cullTwin(input, 0);
    const widened = cullTwin(input, 2);
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
      "R14.T1.b the cluster cull on the device keeps every cluster its twin's exact test keeps, and none its widened test refuses, its receivers' cull (R14.T1.c) on three of its passes and a camera's frustum (R14.T1.e) its fifth",
      missing === 0 &&
        extra === 0 &&
        kept > 0 &&
        filled.size === templates.length &&
        byReceivers > 0,
      `${String(kept)} entries kept of ${String(INSTANCES * CLUSTERS * PASSES)} tested, in ${String(filled.size)} of ${String(templates.length)} templates, ${String(byReceivers)} refused by the receivers' depths; ${String(missing)} the exact twin keeps missing, ${String(extra)} past the widened twin`,
    );
  } finally {
    for (const handle of made) {
      engine.releaseBuffer(handle);
    }
    buckets.dispose();
    engine.releaseCompute(kernel);
  }
}

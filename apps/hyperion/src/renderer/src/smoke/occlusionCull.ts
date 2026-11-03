/**
 * The smoke harness's twin checks of the two-pass occlusion (plan R14, R14.T3.d,
 * `R14.T3.d the depth pyramid and the occlusion's passes against their twins`): the depth pyramid's
 * kernel over a depth the device drew, read back and held to its twin (`clusters/hzb.ts`'
 * `depthPyramidTwin`) texel for texel; and the cull's occlusion variant's early and late passes on
 * the device over random clusters behind and before that depth.
 *
 * @remarks
 * The depth is a frame of eight rectangles 20 to 60 m out. The clusters lie 10 to 150 m out, a
 * page an instance, with a random visibility of the last frame; three more instances are sub-patches
 * of the first three pages, which the early pass never marks and which write no visibility, and one
 * is kept whole, which the pyramid never refuses. The pass has no planes, so its camera's tests keep
 * every cluster: the early pass's marks are its twin's exactly. The late pass's marks and the
 * visibility it writes are held between its twin's with the pyramid's exact test, which refuses
 * the most, and with twice the kernel's allowance, which refuses the least: so the device refuses
 * no cluster the exact test keeps, and keeps none the widened test refuses.
 */

import { ClusterBuckets } from "../view/clusters/buckets";
import {
  CLUSTER_CULL_OCCLUSION_KERNEL,
  CULL_INPUT_WHOLE,
  type CullInstance,
  cullMarksTwin,
  type CullTwinInput,
  cullVisibilityTwin,
  NO_MARK,
} from "../view/clusters/cull";
import {
  type CameraCullPass,
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  type CullOcclusion,
  MAX_CULL_PASSES,
  packCullParams,
  packCullPasses,
} from "../view/clusters/cullPass";
import {
  DEPTH_PYRAMID_KERNEL,
  DepthPyramid,
  depthPyramidTwin,
  HZB_PARAMS_BYTES,
  type HzbAxes,
  hzbRefuses,
  type HzbTestInput,
  packHzbParams,
  pyramidLevels,
} from "../view/clusters/hzb";
import {
  CLUSTER_RECORD_BYTES,
  type ClusterRecord,
  packClusterRecord,
} from "../view/clusters/record";
import { TERRAIN_DISCARD_TEMPLATES, terrainClusterMesh } from "../view/clusters/templates";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, RenderEngine } from "../view/engine/types";
import { type Checks, drawOf, flatSpec, frameOf, rectangleMesh } from "./harness";

/** The depth's size: odd each way, so that every level's last row and column are clamped. */
const SIZE = { widthPx: 97, heightPx: 61 };
const ASPECT = SIZE.widthPx / SIZE.heightPx;

/** The harness's projection (`harness.ts`' `projection`): 90° vertical, so its y scale is 1. */
const PROJECTION = { scaleX: 1 / ASPECT, scaleY: 1 };

/** The harness's identity view: right x, up y, looking down −z. */
const AXES: HzbAxes = {
  right: { x: 1, y: 0, z: 0 },
  up: { x: 0, y: 1, z: 0 },
  forward: { x: 0, y: 0, z: -1 },
};

/** The pages, an instance each, and their clusters. */
const PAGES = 6;
const CLUSTERS = 96;

/** Words of an instance record: terrain's, its `sub.x` word 8 (R14.T3.d). */
const INSTANCE_WORDS = 16;

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

/** Page `page`'s cluster `cluster`'s bit of a visibility, `clusterCull.wgsl`'s `history`. */
function historyBit(words: Uint32Array, page: number, cluster: number): boolean {
  const word = words[page * Math.ceil(CLUSTERS / 32) + Math.floor(cluster / 32)] ?? 0;
  return ((word >>> (cluster % 32)) & 1) === 1;
}

/** How many of `a`'s words differ from `b`'s, bit for bit, and any length between them. */
function wordsDiffer(a: Float32Array, b: Float32Array): number {
  const x = new Uint32Array(a.buffer, a.byteOffset, a.length);
  const y = new Uint32Array(b.buffer, b.byteOffset, b.length);
  let differing = Math.abs(x.length - y.length);
  for (let n = 0; n < Math.min(x.length, y.length); n += 1) {
    differing += x[n] === y[n] ? 0 : 1;
  }
  return differing;
}

/** R14.T3.d's pyramid and passes on the device against their twins: see the module's remarks. */
export async function checkOcclusionCull(engine: RenderEngine, checks: Checks): Promise<void> {
  const next = random(0x3d_0cc);
  const made: BufferHandle[] = [];
  const buffer = (name: string, data: ArrayBufferView, readable = false): BufferHandle => {
    const handle = engine.createBuffer({
      name,
      bytes: data.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | (readable ? BUFFER_USAGE.COPY_SRC : 0),
      category: "geometry-clusters",
    });
    made.push(handle);
    engine.writeBuffer(handle, 0, data);
    return handle;
  };
  const target = engine.createRenderTarget({
    name: "smoke occlusion",
    size: SIZE,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const flat = engine.createMaterial(flatSpec("smoke occlusion flat"));
  const meshes = Array.from({ length: 8 }, (_, n) => {
    const z = 20 + 40 * next();
    const x = (2 * next() - 1) * ASPECT * z;
    const y = (2 * next() - 1) * z;
    const half = (0.1 + 0.3 * next()) * z;
    return rectangleMesh(
      engine,
      `smoke occluder ${String(n)}`,
      [x - half, y - half, x + half, y + half],
      -z,
    );
  });
  const pyramidKernel = await engine.createComputeAsync(DEPTH_PYRAMID_KERNEL);
  const cullKernel = await engine.createComputeAsync(CLUSTER_CULL_OCCLUSION_KERNEL);
  const pyramid = new DepthPyramid(engine, pyramidKernel, "smoke");
  const { templates } = terrainClusterMesh();
  const buckets = new ClusterBuckets(
    engine,
    "smoke occlusion",
    1,
    templates,
    templates.map(() => 0),
  );
  try {
    target.render(
      frameOf(
        "smoke occlusion depth",
        meshes.map((mesh) => drawOf(mesh, flat, [1, 1, 1, 1])),
        ASPECT,
      ),
    );
    const depthTexture = target.depth;
    if (depthTexture === null) {
      throw new Error("the occlusion's target has no depth");
    }
    const depth = new Float32Array(await engine.readTexture(depthTexture));
    pyramid.begin(SIZE, false);
    pyramid.build(depthTexture, PROJECTION);
    const frame = pyramid.frame;
    if (frame === null) {
      throw new Error("the pyramid was not built");
    }
    const device = new Float32Array(await engine.readBuffer(frame.buffer));
    const twin = depthPyramidTwin(depth, SIZE);
    const drawn = depth.filter((d) => d > 0).length;
    const levels = pyramidLevels(SIZE);
    const pyramidDiffering = wordsDiffer(device, twin);
    checks.check(
      "R14.T3.d the depth pyramid on the device is its twin's, texel for texel, each the farthest of every pixel it covers, the odd row and column included",
      pyramidDiffering === 0 && drawn > 0 && drawn < depth.length,
      `${String(device.length)} texels in ${String(levels.length)} levels over a depth of ${String(SIZE.widthPx)} × ${String(SIZE.heightPx)}, ${String(drawn)} of its pixels drawn; ${String(pyramidDiffering)} differing from the twin`,
    );

    // The clusters, a page an instance, 10 to 150 m out across the view; three sub-patches of the
    // first three pages, and the last page's instance kept whole.
    const records: ClusterRecord[] = Array.from({ length: PAGES * CLUSTERS }, () => {
      const z = 10 + 140 * next();
      return {
        centreM: {
          x: Math.fround((2 * next() - 1) * ASPECT * z),
          y: Math.fround((2 * next() - 1) * z),
          z: Math.fround(-z),
        },
        radiusM: Math.fround(0.3 + 5 * next()),
        mode: "strided",
        strideErrorBits: null,
        templateRef: 0,
        shape: "terrain-block",
        mayDiscard: false,
        material: 0,
      };
    });
    const instances: CullInstance[] = [
      ...Array.from({ length: PAGES }, (_, page) => ({
        originM: { x: 0, y: 0, z: 0 },
        page,
        passMask: 1,
        unitM: 0.1,
        ...(page === PAGES - 1 ? { whole: true } : {}),
      })),
      ...[0, 1, 2].map((page) => ({
        originM: { x: 0, y: 0, z: 0 },
        page,
        passMask: 1,
        unitM: 0.1,
        subPatch: true,
      })),
    ];
    const historyWords = PAGES * Math.ceil(CLUSTERS / 32);
    const before = Uint32Array.from({ length: historyWords }, () => Math.floor(next() * 2 ** 32));
    const recordBytes = new DataView(new ArrayBuffer(records.length * CLUSTER_RECORD_BYTES));
    records.forEach((record, n) => {
      packClusterRecord(recordBytes, n * CLUSTER_RECORD_BYTES, record);
    });
    const bytes = 4 * INSTANCE_WORDS;
    const words = new DataView(new ArrayBuffer(instances.length * bytes));
    const inputs = new Uint32Array(2 * instances.length);
    instances.forEach((instance, n) => {
      words.setUint32(bytes * n + 12, instance.page, true);
      words.setUint32(bytes * n + 32, instance.subPatch === true ? 1 : 0, true);
      inputs[2 * n] = (instance.passMask | (instance.whole === true ? CULL_INPUT_WHOLE : 0)) >>> 0;
      inputs[2 * n + 1] = new Uint32Array(Float32Array.of(instance.unitM).buffer)[0] ?? 0;
    });
    const marks = engine.createBuffer({
      name: "smoke occlusion marks",
      bytes: instances.length * CLUSTERS * 4,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
      category: "geometry-clusters",
    });
    made.push(marks);
    const history = buffer("smoke occlusion visibility", before, true);
    const bindings = {
      instance_words: buffer("smoke occlusion instances", words),
      cull_inputs: buffer("smoke occlusion inputs", inputs),
      records: buffer("smoke occlusion records", recordBytes),
      lists: buckets.lists,
      args: buckets.args,
      marks,
      history,
      pyramid: frame.buffer,
    };
    const camera: CameraCullPass = {
      kind: "camera",
      planes: [],
      toleranceRad: 0,
      strideCap: 1,
      horizon: null,
      order: "ordered",
    };
    const paramBytes = new ArrayBuffer(CULL_PARAMS_BYTES);
    packCullParams(
      {
        rotationColumns: [AXES.right, AXES.up, { x: 0, y: 0, z: 1 }],
        instances: instances.length,
        instanceWords: INSTANCE_WORDS,
        clustersPerPage: CLUSTERS,
        passes: 1,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis: AXES.forward,
        firstShape: 0,
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
      },
      paramBytes,
    );
    const hzbInput: HzbTestInput = {
      texels: device,
      levels: frame.levels,
      size: SIZE,
      projection: PROJECTION,
      axes: AXES,
    };
    const cull = async (occlusion: CullOcclusion): Promise<Uint32Array> => {
      const passBytes = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
      packCullPasses([{ ...camera, occlusion }], [buckets.place(0)], passBytes);
      const hzb = new ArrayBuffer(HZB_PARAMS_BYTES);
      packHzbParams(occlusion === "late" ? hzbInput : null, hzb);
      engine.dispatch(
        cullKernel,
        {
          uniforms: {
            params: new Uint32Array(paramBytes),
            passes: new Uint32Array(passBytes),
            hzb: new Uint32Array(hzb),
          },
          buffers: bindings,
          sampled: {},
          storage: {},
        },
        [instances.length, 1, 1],
        "smoke occlusion cull",
      );
      return new Uint32Array(await engine.readBuffer(marks, "tolerance"));
    };
    const earlyMarks = await cull("early");
    const lateMarks = await cull("late");
    const after = new Uint32Array(await engine.readBuffer(history, "tolerance"));
    const input = (occlusion: CullOcclusion): CullTwinInput => ({
      instances,
      records: (page, cluster) => records[page * CLUSTERS + cluster] ?? records[0] ?? fail(),
      clustersPerPage: CLUSTERS,
      rotationColumns: [AXES.right, AXES.up, { x: 0, y: 0, z: 1 }],
      passes: [{ ...camera, occlusion }],
      places: [buckets.place(0)],
      templates: templates.length,
      viewAxis: AXES.forward,
    });
    const visible = (page: number, cluster: number): boolean => historyBit(before, page, cluster);
    const earlyTwin = cullMarksTwin(input("early"), 0, {
      visible,
      earlyMarks: null,
      refuses: null,
    });
    const earlyDiffering = earlyMarks.filter((mark, at) => mark !== earlyTwin[at]).length;
    const seen = (slack: number) => ({
      visible,
      earlyMarks,
      refuses: (centreM: { x: number; y: number; z: number }, radiusM: number) =>
        hzbRefuses(hzbInput, centreM, radiusM, slack),
    });
    const most = cullMarksTwin(input("late"), 0, seen(0));
    const least = cullMarksTwin(input("late"), 2, seen(2));
    let missing = 0;
    let extra = 0;
    let refused = 0;
    lateMarks.forEach((mark, at) => {
      missing += (most[at] ?? NO_MARK) !== NO_MARK && mark === NO_MARK ? 1 : 0;
      extra += mark !== NO_MARK && (least[at] ?? NO_MARK) === NO_MARK ? 1 : 0;
      refused += (earlyMarks[at] ?? NO_MARK) === NO_MARK && mark === NO_MARK ? 1 : 0;
    });
    const fewest = cullVisibilityTwin(input("late"), seen(0), 0);
    const widest = cullVisibilityTwin(input("late"), seen(2), 2);
    let visibilityOff = 0;
    for (let page = 0; page < PAGES; page += 1) {
      for (let cluster = 0; cluster < CLUSTERS; cluster += 1) {
        const at = page * CLUSTERS + cluster;
        const held = historyBit(after, page, cluster);
        visibilityOff +=
          (fewest.get(at) === true && !held) || (held && widest.get(at) !== true) ? 1 : 0;
      }
    }
    checks.check(
      "R14.T3.d the occlusion's passes on the device are their twins': the early pass marks the clusters visible last frame, none of a sub-patch's, the late pass the rest the pyramid does not refuse, and the visibility it writes",
      earlyDiffering === 0 &&
        missing === 0 &&
        extra === 0 &&
        visibilityOff === 0 &&
        refused > 0 &&
        earlyMarks.some((mark) => mark !== NO_MARK) &&
        lateMarks.some((mark) => mark !== NO_MARK),
      `the early pass ${String(earlyMarks.filter((m) => m !== NO_MARK).length)} marked, ${String(earlyDiffering)} differing from its twin; the late pass ${String(lateMarks.filter((m) => m !== NO_MARK).length)} marked and ${String(refused)} refused by the pyramid, ${String(missing)} its exact twin marks missing, ${String(extra)} past its widened twin; ${String(visibilityOff)} visibility bits outside its twins'`,
    );
  } finally {
    for (const handle of made) {
      engine.releaseBuffer(handle);
    }
    buckets.dispose();
    pyramid.dispose();
    target.dispose();
    for (const mesh of meshes) {
      engine.releaseMesh(mesh);
    }
    engine.releaseMaterial(flat);
    engine.releaseCompute(pyramidKernel);
    engine.releaseCompute(cullKernel);
  }
}

/** Fails a check whose fixture lacks what it needs. */
function fail(): never {
  throw new Error("the occlusion check's records are empty");
}

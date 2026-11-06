/**
 * The cluster cull's records, passes and twin (plan R14, R14.T1.b): `ClusterRecord`, `CullPass` and
 * `CullParams` packed as `clusterCull.wgsl` lays them out, and the twin's choices of bucket and
 * stride against the box test of `castsInto`.
 */

import { describe, expect, it } from "vitest";

import { dot, type Vec3 } from "../../geometry/vec3";
import { declaredStructLayout } from "../engine/webgpu/uniforms";
import clusterCullWgsl from "./clusterCull.wgsl?raw";
import {
  cameraKeeps,
  cascadeKeeps,
  CLUSTER_CULL_KERNEL,
  CLUSTER_CULL_OCCLUSION_KERNEL,
  CULL_INPUT_WHOLE,
  CULL_ROUNDING_M,
  type CullInstance,
  cullMarksTwin,
  cullOverTwin,
  cullTwin,
  cullVisibilityTwin,
  horizonRefuses,
  NO_MARK,
  RECEIVER_ROUNDING_M,
  receiversRefuse,
  recordEdges,
  strideLevel,
} from "./cull";
import {
  type CameraCullPass,
  type CascadeCullPass,
  type CullReceivers,
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  cullHorizonOf,
  MAX_CULL_PASSES,
  packCullParams,
  packCullPasses,
} from "./cullPass";
import {
  CLUSTER_FLAG_MEASURED,
  CLUSTER_RECORD_BYTES,
  type ClusterRecord,
  clusterFlags,
  packClusterRecord,
} from "./record";
import { HZB_PARAMS_BYTES } from "./hzb";
import { TERRAIN_INPUT_EDGE_MASK, TERRAIN_SUB_PATCH_MASK } from "./terrain";
import { subPatchWord } from "../terrain/gpu/uniforms";
import {
  EDGE_BANDS,
  edgeBandIndex,
  INSTANCE_FINEST,
  INSTANCE_SUB_GRID_STRIDES,
  NO_STRIDE_BAND,
  NO_STRIDE_SCALE,
  type StrideScale,
} from "./strides";
import { TERRAIN_DISCARD_TEMPLATES } from "./templates";

/** Fails a test whose fixture lacks what it needs. */
function fail(): never {
  throw new Error("the test's fixture lacks what it needs");
}

/** A struct's members as (name, offset). */
function offsets(name: string): Array<readonly [string, number]> {
  return (declaredStructLayout(clusterCullWgsl, name)?.members ?? []).map((m) => [
    m.name,
    m.offsetBytes,
  ]);
}

const X: Vec3 = { x: 1, y: 0, z: 0 };
const Y: Vec3 = { x: 0, y: 1, z: 0 };
const Z: Vec3 = { x: 0, y: 0, z: 1 };

/** A cascade box of radius 100 m about the camera's place, its light straight up z. */
function cascade(extra: Partial<CascadeCullPass> = {}): CascadeCullPass {
  return {
    kind: "cascade",
    axes: [X, Y, Z],
    centreFromCameraLightM: { x: 0, y: 0, z: 0 },
    radiusM: 100,
    depthFarM: 100,
    toleranceM: 0.1,
    strideCap: 1,
    receivers: null,
    ...extra,
  };
}

/** A mask's strips, the first `side` of each of a page's four edges' eight. */
function maskStrips(side: number): number[] {
  return Array.from({ length: 4 * side }, (_, n) => 64 + 8 * Math.floor(n / side) + (n % side));
}

/** The bits of a measured record's half-float errors e₁, e₂, e₄ and e₈, m. */
function errorBits(
  errorsM: readonly [number, number, number, number],
): readonly [number, number, number, number] {
  const half = new Float16Array(errorsM);
  const bits = new Uint16Array(half.buffer);
  return [bits[0] ?? 0, bits[1] ?? 0, bits[2] ?? 0, bits[3] ?? 0];
}

/** A block record at `centreM` of radius `radiusM`. */
function record(
  centreM: Vec3,
  radiusM: number,
  shape: ClusterRecord["shape"] = "terrain-block",
): Extract<ClusterRecord, { readonly mode: "strided" }> {
  return {
    centreM,
    radiusM,
    mode: "strided",
    strideErrorBits: null,
    templateRef: 0,
    shape,
    mayDiscard: false,
    material: 0,
  };
}

describe("the cluster cull's structs (plan R14, R14.T1.b)", () => {
  it("lays a record out in 32 B as WGSL's ClusterRecord, and packs it there", () => {
    expect([
      offsets("ClusterRecord"),
      declaredStructLayout(clusterCullWgsl, "ClusterRecord")?.sizeBytes,
    ]).toEqual([
      [
        ["centre", 0],
        ["radius", 12],
        ["errors", 16],
        ["templateRef", 24],
        ["flags", 28],
      ],
      CLUSTER_RECORD_BYTES,
    ]);
    const out = new DataView(new ArrayBuffer(2 * CLUSTER_RECORD_BYTES));
    const made: ClusterRecord = {
      ...record({ x: 1.5, y: -2, z: 3 }, 4),
      templateRef: 0x2_1008,
      shape: "terrain-skirt",
      mayDiscard: true,
      material: 7,
    };
    packClusterRecord(out, CLUSTER_RECORD_BYTES, made);
    const at = CLUSTER_RECORD_BYTES;
    expect([
      [0, 4, 8, 12].map((o) => out.getFloat32(at + o, true)),
      [16, 20, 24, 28].map((o) => out.getUint32(at + o, true)),
    ]).toEqual([
      [1.5, -2, 3, 4],
      [0, 0x7f80_0000, 0x2_1008, clusterFlags(made)],
    ]);
    expect(clusterFlags(made)).toBe(1 | (1 << 5) | (7 << 8));
    expect(() => {
      packClusterRecord(out, CLUSTER_RECORD_BYTES + 1, made);
    }).toThrow(RangeError);
    expect(() => {
      packClusterRecord(out, 0, { ...made, material: 256 });
    }).toThrow(RangeError);
  });

  it("packs a DAG record's (own, parent) errors as two f32, the mode's bit set", () => {
    const out = new DataView(new ArrayBuffer(CLUSTER_RECORD_BYTES));
    const dag: ClusterRecord = { ...record(Z, 1, "mesh"), mode: "dag", errorsM: [0.5, 2] };
    packClusterRecord(out, 0, dag);
    expect([out.getFloat32(16, true), out.getFloat32(20, true), out.getUint32(28, true)]).toEqual([
      0.5,
      2,
      2 | (1 << 4),
    ]);
  });

  it("packs a measured strided record's e₁, e₂, e₄ and e₈ as four f16 in unpack2x16float's pairs, the measured bit set (R14.T3.b)", () => {
    const out = new DataView(new ArrayBuffer(CLUSTER_RECORD_BYTES));
    const bits = new Uint16Array(Float16Array.of(0, 1.5, 6, 24).buffer);
    packClusterRecord(out, 0, {
      ...record(Z, 1),
      strideErrorBits: [bits[0] ?? 0, bits[1] ?? 0, bits[2] ?? 0, bits[3] ?? 0],
    });
    // Each word's low half is the finer stride's, as `unpack2x16float` takes its `.x`.
    const pair = (word: number): readonly [number, number] => {
      const w = out.getUint32(16 + 4 * word, true);
      const h = new Float16Array(new Uint16Array([w & 0xffff, w >>> 16]).buffer);
      return [h[0] ?? Number.NaN, h[1] ?? Number.NaN];
    };
    expect([pair(0), pair(1), out.getUint32(28, true) & CLUSTER_FLAG_MEASURED]).toEqual([
      [0, 1.5],
      [6, 24],
      CLUSTER_FLAG_MEASURED,
    ]);
  });

  it("leaves an unmeasured strided record's measured bit clear (R14.T3.b)", () => {
    expect(clusterFlags(record(Z, 1)) & CLUSTER_FLAG_MEASURED).toBe(0);
  });

  it("refuses a measured error's bits that are not a u16 (R14.T3.b)", () => {
    const out = new DataView(new ArrayBuffer(CLUSTER_RECORD_BYTES));
    expect(() => {
      packClusterRecord(out, 0, { ...record(Z, 1), strideErrorBits: [0, 0, 0, 0x1_0000] });
    }).toThrow(RangeError);
  });

  it("lays a pass out in 128 B and the dispatch's parameters in 224 B, as WGSL's CullPass and CullParams", () => {
    expect([
      offsets("CullPass"),
      declaredStructLayout(clusterCullWgsl, "CullPass")?.sizeBytes,
    ]).toEqual([
      [
        ["header", 0],
        ["scalars", 16],
        ["axisX", 32],
        ["axisY", 48],
        ["axisZ", 64],
        ["centre", 80],
        ["receivers", 96],
        ["covered", 112],
      ],
      CULL_PASS_BYTES,
    ]);
    expect([
      offsets("CullParams"),
      declaredStructLayout(clusterCullWgsl, "CullParams")?.sizeBytes,
    ]).toEqual([
      [
        ["rotation0", 0],
        ["rotation1", 16],
        ["rotation2", 32],
        ["counts", 48],
        ["viewAxis", 64],
        ["templates", 76],
        ["listOffsets0", 80],
        ["listOffsets1", 96],
        ["listOffsets2", 112],
        ["listOffsets3", 128],
        ["capacities0", 144],
        ["capacities1", 160],
        ["capacities2", 176],
        ["capacities3", 192],
        ["shapes", 208],
      ],
      CULL_PARAMS_BYTES,
    ]);
    expect(clusterCullWgsl).toContain(`const MAX_CULL_PASSES : u32 = ${String(MAX_CULL_PASSES)}u;`);
  });

  it("packs each pass at its place, the rest zero", () => {
    const out = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    const pass = cascade({
      axes: [Y, Z, X],
      centreFromCameraLightM: { x: 5, y: 6, z: 7 },
      radiusM: 250,
      depthFarM: 260,
      toleranceM: 0.25,
      strideCap: 4,
    });
    packCullPasses(
      [cascade(), pass],
      [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 1000 },
      ],
      out,
    );
    const view = new DataView(out);
    const at = CULL_PASS_BYTES;
    const f = (o: number): number => view.getFloat32(at + o, true);
    expect([
      [0, 4, 8, 12].map((o) => view.getUint32(at + o, true)),
      [16, 20, 24, 28].map(f),
      [32, 36, 40, 48, 52, 56, 64, 68, 72].map(f),
      [80, 84, 88].map(f),
      new Uint8Array(out, 2 * CULL_PASS_BYTES).every((byte) => byte === 0),
    ]).toEqual([
      [1, 2, 8, 1000],
      [0.25, 250, 260, 0],
      [0, 1, 0, 0, 0, 1, 1, 0, 0],
      [5, 6, 7],
      true,
    ]);
    expect(() => {
      packCullPasses([cascade()], [], out);
    }).toThrow(RangeError);
  });

  it("packs a camera pass's planes after its scalars, kind 2, and a pass left out as none (R14.T1.e)", () => {
    const out = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    packCullPasses(
      [null, CAMERA],
      [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 64 },
      ],
      out,
    );
    const view = new DataView(out);
    const at = CULL_PASS_BYTES;
    const f = (o: number): number => view.getFloat32(at + o, true);
    expect([
      new Uint8Array(out, 0, CULL_PASS_BYTES).every((byte) => byte === 0),
      [0, 4, 8, 12].map((o) => view.getUint32(at + o, true)),
      f(20),
      [32, 36, 40, 44, 96, 100, 104, 108].map(f),
    ]).toEqual([
      true,
      [2, 0, 8, 64],
      5,
      [Math.fround(-Math.SQRT1_2), 0, Math.fround(-Math.SQRT1_2), 0, 0, 0, -1, -0.5],
    ]);
    expect(() => {
      packCullPasses(
        [{ ...CAMERA, planes: Array.from({ length: 7 }, () => ({ normal: X, offsetM: 0 })) }],
        [{ bucketBase: 0, listBase: 0 }],
        out,
      );
    }).toThrow(RangeError);
  });

  it("packs a pass's receivers after its centre, its switch in its scalars, and −1 for no cover", () => {
    const out = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    const covered = cascade({ receivers: RECEIVERS });
    const nearest = cascade({ receivers: { ...RECEIVERS, coveredBelow: null } });
    packCullPasses(
      [covered, nearest],
      [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 0 },
      ],
      out,
    );
    const view = new DataView(out);
    const f = (at: number, words: number[]): number[] =>
      words.map((o) => view.getFloat32(at + o, true));
    expect([
      f(0, [28, 96, 100, 104, 108, 112, 116, 120, 124]),
      f(CULL_PASS_BYTES, [28, 120, 124]),
      f(2 * CULL_PASS_BYTES, [28]),
    ]).toEqual([[1, 40, 200, 3, 5, 10, -20, 60, 2], [1, -1, 0], [0]]);
  });

  it("packs the dispatch's parameters where CullParams holds them", () => {
    const out = new ArrayBuffer(CULL_PARAMS_BYTES);
    packCullParams(
      {
        rotationColumns: [X, Y, Z],
        instances: 3,
        instanceWords: 8,
        clustersPerPage: 96,
        passes: 4,
        templateOffsets: [0, 10, 20, 30, 40, 45, 50, 55],
        templateCapacities: [10, 10, 10, 10, 5, 5, 5, 5],
        viewAxis: { x: 0, y: 0, z: -1 },
        firstShape: 2,
        discardTemplates: [8, 9, 0xff, 10, 11, 0xff, 0xff, 0xff],
      },
      out,
    );
    const view = new DataView(out);
    const u = (o: number): number => view.getUint32(o, true);
    expect([
      [48, 52, 56, 60, 76].map(u),
      view.getFloat32(72, true),
      [80, 84, 88, 92, 96, 100, 104, 108].map(u),
      [144, 148, 160, 172, 208, 212, 216].map(u),
    ]).toEqual([
      [3, 8, 96, 4, 8],
      -1,
      [0, 10, 20, 30, 40, 45, 50, 55],
      [10, 10, 5, 5, 2, 0x0aff_0908, 0xffff_ff0b],
    ]);
  });
});

/** A bucket's stride, of eight templates a pass, four strides a shape. */
function strideOf(bucket: number): number {
  return 2 ** (bucket % 4);
}

/** A camera at the origin looking down −z, its frustum's sides at 45°, its near plane 0.5 m out. */
const CAMERA: CameraCullPass = {
  kind: "camera",
  planes: [
    { normal: { x: -Math.SQRT1_2, y: 0, z: -Math.SQRT1_2 }, offsetM: 0 },
    { normal: { x: Math.SQRT1_2, y: 0, z: -Math.SQRT1_2 }, offsetM: 0 },
    { normal: { x: 0, y: -Math.SQRT1_2, z: -Math.SQRT1_2 }, offsetM: 0 },
    { normal: { x: 0, y: Math.SQRT1_2, z: -Math.SQRT1_2 }, offsetM: 0 },
    { normal: { x: 0, y: 0, z: -1 }, offsetM: -0.5 },
  ],
  toleranceRad: 0,
  strideCap: 1,
  horizon: null,
  order: "appended",
};

/** Receivers between 40 m and 200 m, reaching 3 m nearer and 5 m farther, a cover below. */
const RECEIVERS: CullReceivers = {
  depthM: [40, 200],
  reachM: [3, 5],
  coveredBelow: { centreM: [10, -20], halfSideM: 60, reachM: 2 },
};

/** The stride the twin takes under `strideCap` at a tolerance of 0.2 m for a unit of `unitM`. */
function strideAt(strideCap: CascadeCullPass["strideCap"], unitM: number): number {
  return 2 ** strideLevel({ strideCap, toleranceM: 0.2 }, unitM);
}

/** A quadrant word's coarse–fine hold at k = 0 on `edge`, bits 2 to 5: x = 0, x = 64, y = 0, y = 64. */
function heldEdge(edge: number): number {
  return 1 << (2 + edge);
}

/**
 * An instance kept whole that is a sub-patch on its own grid `levels` below its page's patch, at
 * its camera passes' cap (R14.T3.c's stand-ins), with `extra` over it.
 */
function ownGrid(levels: number, extra: Partial<CullInstance> = {}): Partial<CullInstance> {
  return {
    quadrantWord: INSTANCE_SUB_GRID_STRIDES,
    subWord: subPatchWord(levels, 1, 2),
    whole: true,
    ...extra,
  };
}

describe("the cluster cull's twin (plan R14, R14.T1.b)", () => {
  it("keeps a sphere inside a cascade's box, or touching it, and refuses one wholly outside it across or beyond", () => {
    const pass = cascade();
    const keeps = (c: Vec3, r = 1): boolean => cascadeKeeps(pass, c, r, 0);
    expect([
      keeps({ x: 0, y: 0, z: 0 }),
      keeps({ x: 100.9, y: 0, z: 0 }),
      keeps({ x: 101.1, y: 0, z: 0 }),
      keeps({ x: 0, y: -101.1, z: 0 }),
      keeps({ x: 0, y: 0, z: -101.1 }),
      // Towards the light the patch's own test decides: a sphere past the box there is kept.
      keeps({ x: 0, y: 0, z: 1e6 }),
    ]).toEqual([true, true, false, false, false, true]);
    // The kernel's allowance widens the box by a metre and some ulps of the lengths.
    expect(cascadeKeeps(pass, { x: 101.1 + CULL_ROUNDING_M - 0.2, y: 0, z: 0 }, 1)).toBe(true);
  });

  it("takes the coarsest stride within the tolerance under the pass's cap, 1 where none is", () => {
    const at = strideAt;
    expect([
      at(8, 0.02),
      at(8, 0.05),
      at(8, 0.06),
      at(8, 0.1),
      at(8, 0.3),
      at(2, 0.02),
      at(1, 0.02),
    ]).toEqual([8, 4, 2, 2, 1, 2, 1]);
  });

  it("keeps a sphere not wholly outside any of a camera's planes, or about the camera, and refuses one wholly outside one (R14.T1.e)", () => {
    const keeps = (c: Vec3, r = 1): boolean => cameraKeeps(CAMERA, c, r, 0);
    expect([
      keeps({ x: 0, y: 0, z: -100 }),
      // Across a side by under its radius, and by more: 1.4 ÷ √2 and 2 ÷ √2 beyond its plane.
      keeps({ x: 101.4, y: 0, z: -100 }),
      keeps({ x: 102, y: 0, z: -100 }),
      // Behind the camera, and about it.
      keeps({ x: 0, y: 0, z: 2 }),
      keeps({ x: 0, y: 0, z: 0.5 }),
      keeps({ x: 0, y: 0, z: 30 }, 40),
    ]).toEqual([true, true, false, false, true, true]);
    // The kernel's allowance widens each plane by a metre and some ulps of the distance.
    expect(cameraKeeps(CAMERA, { x: 102, y: 0, z: -100 }, 1)).toBe(true);
  });

  it("refuses a sphere whose receivers all lie beyond the far end, or below the near end inside the cover, and keeps the rest (R14.T1.c)", () => {
    // The view axis is the light's x, so a sphere's depth is its x; y is across the cover too.
    const pass = cascade({ receivers: RECEIVERS });
    const refuses = (x: number, y = -20, r = 1, p = pass): boolean =>
      receiversRefuse(p, X, { x, y, z: 0 }, r, 0);
    expect([
      // Beyond: x − r − 3 past 200.
      refuses(204.1),
      refuses(203.9),
      // Below 40 by x + r + 5, inside the cover by |x − 10| + r + 2 ≤ 60 across both axes.
      refuses(33.9),
      refuses(34.1),
      refuses(33.9, -20 + 56.9),
      refuses(33.9, -20 + 57.1),
      // In the band, or with no cover below, or off.
      refuses(100),
      refuses(33.9, -20, 1, cascade({ receivers: { ...RECEIVERS, coveredBelow: null } })),
      refuses(1e6, -20, 1, cascade()),
    ]).toEqual([true, false, true, false, true, false, false, false, false]);
    // The kernel's allowance narrows each test by a centimetre and some ulps of the lengths.
    expect(receiversRefuse(pass, X, { x: 204.1 + RECEIVER_ROUNDING_M, y: 0, z: 0 }, 1)).toBe(true);
    expect(receiversRefuse(pass, X, { x: 204.005, y: 0, z: 0 }, 1)).toBe(false);
  });

  it("gives every cluster of an instance one stride in a pass, the coarsest within its texel, blocks and strips alike (R14.T1.d)", () => {
    const records = Array.from({ length: 6 }, (_, n) =>
      record({ x: 10 * n, y: 0, z: 0 }, 1 + n, n < 4 ? "terrain-block" : "terrain-skirt"),
    );
    const buckets = cullTwin({
      instances: [
        { originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b11, unitM: 0.05 },
        { originM: { x: 0, y: 5, z: 0 }, page: 0, passMask: 0b11, unitM: 0.25 },
      ],
      records: (_page, cluster) => records[cluster] ?? record(Z, 0),
      clustersPerPage: records.length,
      rotationColumns: [X, Y, Z],
      // A texel of 0.2 m: stride 4 for a unit of 0.05 m, 1 for 0.25 m; the second pass capped at 2.
      passes: [
        cascade({ strideCap: 8, toleranceM: 0.2 }),
        cascade({ strideCap: 2, toleranceM: 0.2 }),
      ],
      places: [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 100 },
      ],
      templates: 8,
      viewAxis: Z,
    });
    const byInstance = new Map<string, Set<number>>();
    for (const [bucket, entries] of buckets) {
      for (const entry of entries) {
        const key = `${String(Math.floor(bucket / 8))}:${String(Math.floor(entry / 256))}`;
        byInstance.set(key, (byInstance.get(key) ?? new Set()).add(strideOf(bucket)));
      }
    }
    expect(
      Object.fromEntries([...byInstance].map(([key, strides]) => [key, [...strides]])),
    ).toEqual({ "0:0": [4], "0:1": [1], "1:0": [2], "1:1": [1] });
    expect([...buckets.keys()].toSorted((a, b) => a - b)).toEqual([0, 2, 4, 6, 8, 9, 12, 13]);
  });

  it("appends each kept cluster to its pass's bucket of its shape and stride, and nothing of a pass its instance may not enter", () => {
    const records = [
      record({ x: 0, y: 0, z: 0 }, 1),
      record({ x: 500, y: 0, z: 0 }, 1),
      record(Z, 1, "terrain-skirt"),
    ];
    const buckets = cullTwin({
      instances: [
        { originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b01, unitM: 0.02 },
        { originM: { x: 10, y: 0, z: 0 }, page: 0, passMask: 0b11, unitM: 0.3 },
      ],
      records: (_page, cluster) => records[cluster] ?? record(Z, 0),
      clustersPerPage: 3,
      rotationColumns: [X, Y, Z],
      passes: [cascade({ strideCap: 8, toleranceM: 0.2 }), cascade({ strideCap: 1 })],
      places: [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 100 },
      ],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    });
    expect(Object.fromEntries([...buckets].toSorted(([a], [b]) => a - b))).toEqual({
      // Pass 0: instance 0 at stride 8 (block bucket 3, strip bucket 7), instance 1 at 1.
      0: [256 * 1 + 0],
      3: [0],
      4: [256 * 1 + 2],
      7: [2],
      // Pass 1, stride 1 alone: instance 1's block and strip.
      8: [256 * 1 + 0],
      12: [256 * 1 + 2],
    });
  });

  /**
   * The clusters a camera pass keeps of a page whose blocks and strips stand before the camera and
   * whose strips' spheres behind it, for an instance of sub-patch word `subWord`.
   */
  function maskKept(subWord: number): number[] {
    const records = Array.from({ length: 116 }, (_, c) =>
      c < 64
        ? record({ x: 0, y: 0, z: -50 }, 1)
        : c < 96
          ? record({ x: 0, y: 0, z: 50 }, 1, "terrain-skirt")
          : record({ x: 0, y: 0, z: -50 }, 1, "terrain-fin"),
    );
    const buckets = cullTwin({
      instances: [{ originM: Z, page: 0, passMask: 0b1, unitM: 1, subWord }],
      records: (_page, cluster) => records[cluster] ?? record(Z, 0),
      clustersPerPage: records.length,
      rotationColumns: [X, Y, Z],
      passes: [CAMERA],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: 16,
      viewAxis: { x: 0, y: 0, z: -1 },
    });
    return [...buckets.values()].flat().toSorted((a, b) => a - b);
  }

  it("keeps a stand-in's mask's blocks over its area by their spheres, its strips along their sides whatever theirs, and no other cluster (Design note 19)", () => {
    // One level down at part (1, 0): blocks 4 to 7 of rows 0 to 3, and four strips a side; three
    // down at (5, 2): block 21 and one strip a side.
    const one = Array.from({ length: 16 }, (_, n) => 8 * Math.floor(n / 4) + 4 + (n % 4));
    expect([
      maskKept(TERRAIN_SUB_PATCH_MASK | subPatchWord(1, 1, 0)),
      maskKept(TERRAIN_SUB_PATCH_MASK | subPatchWord(3, 5, 2)),
    ]).toEqual([
      [...one, ...maskStrips(4)],
      [21, ...maskStrips(1)],
    ]);
  });

  it("keeps every block and fin of an instance that is not a mask by its sphere, a sub-patch's too", () => {
    const kept = Array.from({ length: 64 }, (_, c) => c).concat(
      Array.from({ length: 20 }, (_, n) => 96 + n),
    );
    expect([maskKept(0), maskKept(subPatchWord(2, 1, 1))]).toEqual([kept, kept]);
  });

  it("takes stride 1 in a camera pass for an instance without stride bands, whatever its cap and tolerance, so that the wireframe's contours stand on the stride-1 grid (R14.T4.c)", () => {
    const records = [
      record({ x: 0, y: 0, z: -50 }, 1),
      record({ x: 0, y: 0, z: -60 }, 1, "terrain-skirt"),
    ];
    const buckets = cullTwin({
      instances: [{ originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b1, unitM: 1e-6 }],
      records: (_page, cluster) => records[cluster] ?? record(Z, 0),
      clustersPerPage: records.length,
      rotationColumns: [X, Y, Z],
      passes: [{ ...CAMERA, strideCap: 8, toleranceRad: 1 }],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    });
    expect([...buckets.keys()].map(strideOf)).toEqual([1, 1]);
  });

  /** A page's block of errors e₂, e₄ and e₈ 1, 2 and 4 m, its bands 100, 200 and 400 m at SCALE. */
  const PAGE_BLOCK = { ...record(Z, 1), strideErrorBits: errorBits([0.5, 1, 2, 4]) };

  /** A scale of 1 ÷ τ′ 100: a band of 100 m a metre of error. */
  const SCALE: StrideScale = { statedM: 0, perErrorM: 100 };

  /** The page's cluster the strides test by default: block (3, 3), within the patch. */
  const INNER = 27;

  /**
   * One cluster's stride in a camera pass of cap `cap` for an instance of `extra`, cluster `at` of a
   * page of 116 whose blocks are {@link PAGE_BLOCK} but for `page`'s.
   */
  function cameraStride(
    cluster: ClusterRecord,
    distanceM: number,
    extra: Partial<CullInstance> = {},
    cap: CameraCullPass["strideCap"] = 8,
    at = INNER,
    page: ReadonlyMap<number, ClusterRecord> = new Map(),
  ): number {
    const buckets = cullTwin({
      instances: [{ originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b1, unitM: 1, ...extra }],
      records: (_page, c) =>
        c === at
          ? { ...cluster, centreM: { x: 0, y: 0, z: -distanceM } }
          : { ...(page.get(c) ?? PAGE_BLOCK), radiusM: -1 },
      clustersPerPage: 116,
      rotationColumns: [X, Y, Z],
      passes: [{ ...CAMERA, strideCap: cap }],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    });
    const [bucket] = buckets.keys();
    return bucket === undefined ? 0 : strideOf(bucket);
  }

  it("strides a camera pass's cluster coarser with its distance past its bands, its records' errors under its instance's scale (R14.T3.c)", () => {
    const at = (distanceM: number): number =>
      cameraStride(PAGE_BLOCK, distanceM, { strideScale: SCALE });
    expect([at(50), at(150), at(300), at(1_000)]).toEqual([1, 2, 4, 8]);
  });

  it("strides a block at the largest band of the vertices it drops: its neighbours' on its sides, none where one is not measured, and not a corner's (R14.T3.c's shared bands)", () => {
    const larger = { ...PAGE_BLOCK, strideErrorBits: errorBits([1, 3, 6, 12]) };
    const unmeasured = { ...PAGE_BLOCK, strideErrorBits: null };
    const at = (page: ReadonlyMap<number, ClusterRecord>): number =>
      cameraStride(PAGE_BLOCK, 350, { strideScale: SCALE }, 8, INNER, page);
    expect([
      at(new Map()),
      at(new Map([[INNER + 1, larger]])),
      at(new Map([[INNER - 8, larger]])),
      at(new Map([[INNER - 9, larger]])),
      at(new Map([[INNER + 8, unmeasured]])),
    ]).toEqual([4, 2, 2, 4, 1]);
  });

  it("strides a block or a strip on its patch's edge by that edge's shared bands there, none where its record holds none (R14.T3.c's shared bands)", () => {
    // Block (0, 3) on x = 0, and strip 7 of `gpu/resources.ts`' edge 3 (x = 0), along block (0, 0).
    const edgeBandsM = new Float64Array(EDGE_BANDS).fill(NO_STRIDE_BAND);
    edgeBandsM.set([300, 600, 1_200], edgeBandIndex(0, 3, 0));
    edgeBandsM.set([100, 200, 400], edgeBandIndex(0, 0, 0));
    const strip = { ...record(Z, 1, "terrain-skirt"), templateRef: 3 << 16 };
    expect([
      cameraStride(PAGE_BLOCK, 350, { strideScale: SCALE, edgeBandsM }, 8, 24),
      cameraStride(PAGE_BLOCK, 350, { strideScale: SCALE }, 8, 24),
      cameraStride(strip, 1_000, { strideScale: SCALE, edgeBandsM }, 8, 64 + 8 * 3 + 7),
      cameraStride(strip, 1_000, { strideScale: SCALE, edgeBandsM }, 8, 64 + 8 * 3 + 6),
      cameraStride(PAGE_BLOCK, 1_000, { strideScale: SCALE, edgeBandsM }, 8, 100),
    ]).toEqual([2, 1, 8, 1, 1]);
  });

  it("holds an instance with the quadrant word's bit 18, or kept whole as its heights fade, at stride 1, and every stride within the pass's cap (R14.T3.c)", () => {
    expect([
      cameraStride(PAGE_BLOCK, 1_000, { strideScale: SCALE, quadrantWord: INSTANCE_FINEST }),
      cameraStride(PAGE_BLOCK, 1_000, { strideScale: SCALE }, 2),
      cameraStride(PAGE_BLOCK, 1_000, { strideScale: NO_STRIDE_SCALE }),
      cameraStride(PAGE_BLOCK, 1_000, { strideScale: SCALE, whole: true }),
    ]).toEqual([1, 2, 1, 1]);
  });

  it("draws a sub-patch on its own grid with the quadrant word's bit 19 at the pass's cap up to its levels, kept whole or not, near or far, and with bit 18 at stride 1 (R14.T3.c's stand-ins)", () => {
    const strip = { ...record(Z, 1, "terrain-skirt"), templateRef: 3 << 16 };
    expect([
      cameraStride(PAGE_BLOCK, 1, ownGrid(5)),
      cameraStride(PAGE_BLOCK, 1_000, ownGrid(4)),
      cameraStride(PAGE_BLOCK, 1, ownGrid(4), 2),
      cameraStride(PAGE_BLOCK, 1, ownGrid(4, { whole: false })),
      cameraStride(PAGE_BLOCK, 1, ownGrid(2)),
      cameraStride(strip, 1, ownGrid(6), 8, 64 + 8 * 3 + 7),
      cameraStride(
        PAGE_BLOCK,
        1,
        ownGrid(5, { quadrantWord: INSTANCE_SUB_GRID_STRIDES | INSTANCE_FINEST }),
      ),
      cameraStride(PAGE_BLOCK, 1, ownGrid(5, { quadrantWord: 0 })),
    ]).toEqual([8, 8, 2, 8, 4, 8, 1, 1]);
  });

  it("strides a stand-in's mask's strip by its block's bands at its set's side, at that block's sphere and not its record's on the patch's edge (R14.T3.c's stand-ins)", () => {
    // A mask one level down at part (1, 0), its set blocks 4 to 7 of rows 0 to 3: strip 0 of side
    // 0 hangs from block 4.
    const mask = (TERRAIN_SUB_PATCH_MASK | subPatchWord(1, 1, 0)) >>> 0;
    const strip = { ...PAGE_BLOCK, shape: "terrain-skirt", templateRef: 0 } as const;
    const stride = (stripM: number, blockM: number): number => {
      const buckets = cullTwin({
        instances: [
          {
            originM: { x: 0, y: 0, z: 0 },
            page: 0,
            passMask: 0b1,
            unitM: 1,
            strideScale: SCALE,
            subWord: mask,
          },
        ],
        records: (_page, c) =>
          c === 64
            ? { ...strip, centreM: { x: 0, y: 0, z: -stripM } }
            : c === 4
              ? { ...PAGE_BLOCK, centreM: { x: 0, y: 0, z: -blockM } }
              : { ...PAGE_BLOCK, radiusM: -1 },
        clustersPerPage: 116,
        rotationColumns: [X, Y, Z],
        passes: [{ ...CAMERA, strideCap: 8 }],
        places: [{ bucketBase: 0, listBase: 0 }],
        templates: 8,
        viewAxis: { x: 0, y: 0, z: -1 },
      });
      const entry = [...buckets.entries()].find(([, entries]) => entries.includes(64));
      return entry === undefined ? 0 : strideOf(entry[0]);
    };
    expect([stride(1_000, 50), stride(50, 1_000), stride(1_000, 1_000)]).toEqual([1, 8, 8]);
  });

  /**
   * The twin's count of {@link PAGE_BLOCK}'s strides past τ′ `toleranceRad` in a camera pass,
   * `order`ed, of an instance at {@link SCALE}, its cluster `distanceM` ahead (plan R14, R14.T3.e).
   */
  function pastTolerance(
    distanceM: number,
    toleranceRad: number,
    order: CameraCullPass["order"] = "appended",
    extra: Partial<CullInstance> = {},
  ): number {
    const counts = cullOverTwin({
      instances: [
        {
          originM: { x: 0, y: 0, z: 0 },
          page: 0,
          passMask: 0b1,
          unitM: 1,
          strideScale: SCALE,
          ...extra,
        },
      ],
      records: (_page, c) =>
        c === INNER
          ? { ...PAGE_BLOCK, centreM: { x: 0, y: 0, z: -distanceM } }
          : { ...PAGE_BLOCK, radiusM: -1 },
      clustersPerPage: 116,
      rotationColumns: [X, Y, Z],
      passes: [{ ...CAMERA, strideCap: 8, toleranceRad, order }],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    });
    return counts.get(0) ?? 0;
  }

  it("counts a cluster a camera pass draws at a stride past its τ′: none at the tolerance its stride was chosen at, one where τ′ is tighter than its need there, none at stride 1, without a tolerance or with the instance kept whole (R14.T3.e)", () => {
    // At 300 m the block takes stride 4 at SCALE's 0.01 rad, its own need 200 m: past τ′ below
    // 200 ÷ (300 × 100) rad.
    expect([
      pastTolerance(300, 0.01),
      pastTolerance(300, 0.0067),
      pastTolerance(300, 0.0066),
      pastTolerance(300, 0.0066, "ordered"),
      pastTolerance(50, 1e-6),
      pastTolerance(300, 0),
      pastTolerance(300, 0.0066, "appended", { whole: true }),
    ]).toEqual([0, 0, 1, 1, 0, 0, 0]);
  });

  it("counts a cluster its may-discard template draws at stride 1, never past τ′, an edge mask's strips among them (R14.T3.e; R14.T3.c's edge stand-ins)", () => {
    // Every block as `pastTolerance(300, 0.0066)`'s, which counts each, but block `discarding` (if
    // any) may discard, and strip 64 (side 0's first, by block 0's bands for a mask at part (0, 0))
    // drawn where `strip`.
    const counted = (discarding: number, strip: boolean, extra: Partial<CullInstance>): number =>
      cullOverTwin({
        instances: [
          {
            originM: { x: 0, y: 0, z: 0 },
            page: 0,
            passMask: 0b1,
            unitM: 1,
            strideScale: SCALE,
            ...extra,
          },
        ],
        records: (_page, c) =>
          c < 64
            ? { ...PAGE_BLOCK, mayDiscard: c === discarding, centreM: { x: 0, y: 0, z: -300 } }
            : c === 64 && strip
              ? { ...PAGE_BLOCK, shape: "terrain-skirt", centreM: { x: 0, y: 0, z: -300 } }
              : { ...PAGE_BLOCK, radiusM: -1 },
        clustersPerPage: 116,
        rotationColumns: [X, Y, Z],
        passes: [{ ...CAMERA, strideCap: 8, toleranceRad: 0.0066, order: "appended" }],
        places: [{ bucketBase: 0, listBase: 0 }],
        templates: 16,
        viewAxis: { x: 0, y: 0, z: -1 },
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
      }).get(0) ?? 0;
    const mask = (TERRAIN_SUB_PATCH_MASK | subPatchWord(1, 0, 0)) >>> 0;
    const edgeMask = { subWord: mask, standInBits: TERRAIN_INPUT_EDGE_MASK };
    expect([
      // A whole patch's 36 blocks off its edges (its edges hold no bands here), and with one of
      // them may-discard.
      counted(-1, false, {}),
      counted(INNER, false, {}),
      // An interior bake's mask's strip, strided by its side block's bands, and an edge mask's,
      // drawn by the may-discard template.
      counted(-1, true, { subWord: mask }) - counted(-1, false, { subWord: mask }),
      counted(-1, true, edgeMask) - counted(-1, false, edgeMask),
    ]).toEqual([36, 35, 1, 0]);
  });

  it("holds a block or a strip on an edge a finer patch touches at stride 4, and none on another edge (R14.T3.c)", () => {
    const block = (x0: number, y0: number): ClusterRecord => ({
      ...PAGE_BLOCK,
      templateRef: x0 | (y0 << 8),
    });
    // At the page's block (3, 3) in this test, its bands its errors'.
    const strip = (edge: number): ClusterRecord => ({
      ...PAGE_BLOCK,
      shape: "terrain-skirt",
      templateRef: edge << 16,
    });
    const at = (cluster: ClusterRecord, quadrantWord: number): number =>
      cameraStride(cluster, 1_000, { strideScale: SCALE, quadrantWord });
    expect([
      at(block(0, 24), heldEdge(0)),
      at(block(56, 24), heldEdge(1)),
      at(block(24, 0), heldEdge(2)),
      at(block(24, 56), heldEdge(3)),
      at(block(24, 24), 0b1111 << 2),
      at(block(0, 24), heldEdge(1)),
    ]).toEqual([4, 4, 4, 4, 8, 8]);
    // A strip's edge 0 y = 0, 1 x = 64, 2 y = 64, 3 x = 0.
    expect([0, 1, 2, 3].map((edge) => at(strip(edge), heldEdge([2, 1, 3, 0][edge] ?? 0)))).toEqual([
      4, 4, 4, 4,
    ]);
    expect(recordEdges(record(Z, 1, "mesh"))).toBe(0);
  });

  it("holds a block on an edge whose hold fades in or out at stride 4 too, by its byte of the shares word (R14.T3.c)", () => {
    const at = (x0: number, holdFadesWord: number): number =>
      cameraStride({ ...PAGE_BLOCK, templateRef: x0 | (24 << 8) }, 1_000, {
        strideScale: SCALE,
        holdFadesWord,
      });
    // Edge x = 64's byte (edge 1): its blocks held, edge x = 0's free.
    expect([at(56, 64 << 8), at(0, 64 << 8), at(56, 0)]).toEqual([4, 8, 8]);
  });

  it("puts a may-discard record in its shape's may-discard bucket at stride 1, a fin run and a head run in theirs (R14.T1.g)", () => {
    const records = [
      { ...record({ x: 0, y: 0, z: 0 }, 1), mayDiscard: true },
      { ...record({ x: 0, y: 0, z: 0 }, 1, "terrain-skirt"), mayDiscard: true },
      { ...record({ x: 0, y: 0, z: 0 }, 1, "terrain-fin"), mayDiscard: true },
      record({ x: 0, y: 0, z: 0 }, 1),
      { ...record({ x: 0, y: 0, z: 0 }, 1, "terrain-head"), mayDiscard: true },
    ];
    const input = {
      instances: [{ originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b1, unitM: 0.02 }],
      records: (_page: number, cluster: number) => records[cluster] ?? record(Z, 0),
      clustersPerPage: records.length,
      rotationColumns: [X, Y, Z] as const,
      passes: [cascade({ strideCap: 8, toleranceM: 0.2 })],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: 12,
      viewAxis: { x: 0, y: 0, z: -1 },
    };
    const routed = cullTwin({ ...input, discardTemplates: TERRAIN_DISCARD_TEMPLATES });
    expect(Object.fromEntries([...routed].toSorted(([a], [b]) => a - b))).toEqual({
      // The opaque block at the cascade's stride 8; the others at their may-discard templates.
      3: [3],
      8: [0],
      9: [1],
      10: [2],
      11: [4],
    });
    // A consumer without may-discard templates draws them by their strided ones, its fin runs none.
    const plain = cullTwin(input);
    expect(Object.fromEntries([...plain].toSorted(([a], [b]) => a - b))).toEqual({
      3: [0, 3],
      7: [1],
    });
  });

  it("keeps every cluster of a whole instance in each pass it enters, its spheres untested, and none of a pass it may not (R10.T16.i's fades)", () => {
    const records = [record({ x: 0, y: 0, z: 5_000 }, 1), record({ x: 0, y: 0, z: 0 }, -1)];
    const input = {
      records: (_page: number, cluster: number) => records[cluster] ?? record(Z, 0),
      clustersPerPage: records.length,
      rotationColumns: [X, Y, Z] as const,
      passes: [{ ...CAMERA }, cascade({ strideCap: 1 })],
      places: [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 100 },
      ],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    };
    const instance = { originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b01, unitM: 1 };
    expect(cullTwin({ ...input, instances: [instance] }).size).toBe(0);
    // Whole: the sphere behind the camera is kept in the pass it enters; no cluster is still none.
    const whole = cullTwin({ ...input, instances: [{ ...instance, whole: true }] });
    expect(Object.fromEntries(whole)).toEqual({ 0: [0] });
    expect(CULL_INPUT_WHOLE).toBe(2 ** 31);
  });
});

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

/** `f32` arithmetic, one rounding an operation, as the kernel's. */
const f = Math.fround;

function dotF32(a: Vec3, b: Vec3): number {
  return f(f(f(a.x * b.x) + f(a.y * b.y)) + f(a.z * b.z));
}

function narrowed(v: Vec3): Vec3 {
  return { x: f(v.x), y: f(v.y), z: f(v.z) };
}

/**
 * `clusterCull.wgsl`'s `camera_keeps` in `f32`: the sphere at the instance's narrowed origin plus
 * its narrowed record offset, against the narrowed planes.
 */
function cameraKeepsF32(
  pass: CameraCullPass,
  originM: Vec3,
  offsetM: Vec3,
  radiusM: number,
): boolean {
  const o = narrowed(originM);
  const r = narrowed(offsetM);
  const centre = { x: f(o.x + r.x), y: f(o.y + r.y), z: f(o.z + r.z) };
  const radius = f(radiusM);
  const length = f(Math.sqrt(dotF32(centre, centre)));
  const slack = f(f(f(1e-6) * length) + 1);
  const reach = f(radius + slack);
  if (dotF32(centre, centre) <= f(reach * reach)) {
    return true;
  }
  return pass.planes.every(
    (plane) => f(dotF32(narrowed(plane.normal), centre) + f(plane.offsetM)) >= -reach,
  );
}

/** `clusterCull.wgsl`'s `cascade_keeps` in `f32`, the sphere formed as {@link cameraKeepsF32}'s. */
function cascadeKeepsF32(
  pass: CascadeCullPass,
  originM: Vec3,
  offsetM: Vec3,
  radiusM: number,
): boolean {
  const o = narrowed(originM);
  const r = narrowed(offsetM);
  const centre = { x: f(o.x + r.x), y: f(o.y + r.y), z: f(o.z + r.z) };
  const [xL, yL, zL] = pass.axes.map(narrowed);
  const c = narrowed(pass.centreFromCameraLightM);
  const v = {
    x: f(dotF32(centre, xL ?? X) - c.x),
    y: f(dotF32(centre, yL ?? Y) - c.y),
    z: f(dotF32(centre, zL ?? Z) - c.z),
  };
  const slack = f(
    f(
      f(1e-6) *
        f(f(f(Math.sqrt(dotF32(centre, centre))) + f(Math.sqrt(dotF32(c, c)))) + f(pass.radiusM)),
    ) + 1,
  );
  const reach = f(f(f(pass.radiusM) + f(radiusM)) + slack);
  return (
    Math.abs(v.x) <= reach &&
    Math.abs(v.y) <= reach &&
    v.z >= -f(f(f(pass.depthFarM) + f(radiusM)) + slack)
  );
}

/** A unit vector of `next`, uniform on the sphere. */
function unit(next: () => number): Vec3 {
  const z = 2 * next() - 1;
  const phi = 2 * Math.PI * next();
  const s = Math.sqrt(1 - z * z);
  return { x: s * Math.cos(phi), y: s * Math.sin(phi), z };
}

describe("the cluster cull's camera pass in the brainstorm's cases (plan R14, R14.T1.b's tests)", () => {
  it("keeps a cluster larger than the frustum that holds it, and one the camera is inside", () => {
    const exact = (c: Vec3, r: number): boolean => cameraKeeps(CAMERA, c, r, 0);
    expect([
      // Larger than the frustum's cross-section where it stands, wholly across all four sides.
      exact({ x: 0, y: 0, z: -300 }, 250),
      // Its centre behind the camera, the camera inside it.
      exact({ x: 0, y: 0, z: 5 }, 10),
      exact({ x: 40, y: -30, z: 20 }, 60),
    ]).toEqual([true, true, true]);
  });

  it("keeps a cluster across the near plane, and refuses one wholly behind it", () => {
    const exact = (c: Vec3, r: number): boolean => cameraKeeps(CAMERA, c, r, 0);
    expect([
      exact({ x: 0, y: 0, z: -0.5 }, 0.3),
      exact({ x: 0, y: 0, z: -0.3 }, 0.25),
      exact({ x: 0, y: 0, z: 0.4 }, 0.2),
    ]).toEqual([true, true, false]);
  });

  it("never refuses in f32 at 10⁷ m from the camera what the f64 sphere test keeps, nor keeps what the widened twin refuses", () => {
    const next = random(0x14_1f);
    let keptByExact = 0;
    let refusedByWidened = 0;
    const misses: string[] = [];
    for (let n = 0; n < 20_000; n += 1) {
      // An instance about 10⁷ m out near one of the frustum's planes, its cluster a patch's width off.
      const plane = CAMERA.planes[n % 4] ?? { normal: X, offsetM: 0 };
      const ahead = { x: 0, y: 0, z: -1e7 * (0.5 + next()) };
      const along = dotF32(plane.normal, ahead);
      const originM = {
        x: ahead.x - along * plane.normal.x + 3e4 * (next() - 0.5) * plane.normal.x,
        y: ahead.y - along * plane.normal.y + 3e4 * (next() - 0.5) * plane.normal.y,
        z: ahead.z - along * plane.normal.z + 3e4 * (next() - 0.5) * plane.normal.z,
      };
      const d = unit(next);
      const offsetM = { x: 5e3 * d.x, y: 5e3 * d.y, z: 5e3 * d.z };
      const radiusM = 1 + 2e3 * next();
      const centre = {
        x: originM.x + offsetM.x,
        y: originM.y + offsetM.y,
        z: originM.z + offsetM.z,
      };
      const kernel = cameraKeepsF32(CAMERA, originM, offsetM, radiusM);
      if (cameraKeeps(CAMERA, centre, radiusM, 0)) {
        keptByExact += 1;
        if (!kernel) {
          misses.push(`refused ${JSON.stringify(centre)} r ${String(radiusM)}`);
        }
      }
      if (!cameraKeeps(CAMERA, centre, radiusM, 2)) {
        refusedByWidened += 1;
        if (kernel) {
          misses.push(`kept ${JSON.stringify(centre)} r ${String(radiusM)}`);
        }
      }
    }
    expect(misses).toEqual([]);
    expect([keptByExact, refusedByWidened].every((count) => count > 2_000)).toBe(true);
  });

  it("never refuses in f32 at 10⁷ m from the camera what the f64 box test keeps, nor keeps what the widened twin refuses", () => {
    const next = random(0x14_2f);
    let keptByExact = 0;
    let refusedByWidened = 0;
    const misses: string[] = [];
    for (let n = 0; n < 20_000; n += 1) {
      const zL = unit(next);
      const xL = (() => {
        const a = Math.abs(zL.x) < 0.9 ? X : Y;
        const t = dotF32(a, zL);
        const v = { x: a.x - t * zL.x, y: a.y - t * zL.y, z: a.z - t * zL.z };
        const l = Math.hypot(v.x, v.y, v.z);
        return { x: v.x / l, y: v.y / l, z: v.z / l };
      })();
      const yL = {
        x: zL.y * xL.z - zL.z * xL.y,
        y: zL.z * xL.x - zL.x * xL.z,
        z: zL.x * xL.y - zL.y * xL.x,
      };
      const pass = cascade({
        axes: [xL, yL, zL],
        // A far cascade, its centre 10⁷ m out along its own x.
        centreFromCameraLightM: { x: 1e7, y: 0, z: 0 },
        radiusM: 2e5,
        depthFarM: 3e5,
      });
      // A cluster near the box's x or y face, or its far face, a patch's width off.
      const side = n % 3;
      const lx = side === 0 ? 1e7 + (2e5 + 3e4 * (next() - 0.5)) : 1e7 + 4e5 * (next() - 0.5);
      const ly = side === 1 ? 2e5 + 3e4 * (next() - 0.5) : 4e5 * (next() - 0.5);
      const lz = side === 2 ? -3e5 + 3e4 * (next() - 0.5) : 4e5 * (next() - 0.5);
      const centre = {
        x: lx * xL.x + ly * yL.x + lz * zL.x,
        y: lx * xL.y + ly * yL.y + lz * zL.y,
        z: lx * xL.z + ly * yL.z + lz * zL.z,
      };
      const d = unit(next);
      const offsetM = { x: 5e3 * d.x, y: 5e3 * d.y, z: 5e3 * d.z };
      const originM = { x: centre.x - offsetM.x, y: centre.y - offsetM.y, z: centre.z - offsetM.z };
      const radiusM = 1 + 2e3 * next();
      const kernel = cascadeKeepsF32(pass, originM, offsetM, radiusM);
      if (cascadeKeeps(pass, centre, radiusM, 0)) {
        keptByExact += 1;
        if (!kernel) {
          misses.push(`refused ${JSON.stringify(centre)} r ${String(radiusM)}`);
        }
      }
      if (!cascadeKeeps(pass, centre, radiusM, 2)) {
        refusedByWidened += 1;
        if (kernel) {
          misses.push(`kept ${JSON.stringify(centre)} r ${String(radiusM)}`);
        }
      }
    }
    expect(misses).toEqual([]);
    expect([keptByExact, refusedByWidened].every((count) => count > 2_000)).toBe(true);
  });
});

/** R05's point test (`terrain/cull.ts`' `aboveHorizon`, a corner's): whether `t` from the camera is occluded. */
function pointOccluded(cameraFromCentreM: Vec3, occluderRadiusM: number, t: Vec3): boolean {
  const r = occluderRadiusM;
  const c = { x: cameraFromCentreM.x / r, y: cameraFromCentreM.y / r, z: cameraFromCentreM.z / r };
  const horizonSq = c.x * c.x + c.y * c.y + c.z * c.z - 1;
  if (!(horizonSq > 0)) {
    return false;
  }
  const x = t.x / r;
  const y = t.y / r;
  const z = t.z / r;
  const vtDotVc = -(x * c.x + y * c.y + z * c.z);
  return vtDotVc > horizonSq && (vtDotVc * vtDotVc) / (x * x + y * y + z * z) > horizonSq;
}

/** `count` points on the sphere about `centre` of radius `radius`, a Fibonacci lattice, and its centre. */
function spherePoints(centre: Vec3, radius: number, count = 200): Vec3[] {
  const golden = Math.PI * (3 - Math.sqrt(5));
  const points: Vec3[] = [centre];
  for (let k = 0; k < count; k += 1) {
    const z = 1 - (2 * (k + 0.5)) / count;
    const s = Math.sqrt(1 - z * z);
    points.push({
      x: centre.x + radius * s * Math.cos(golden * k),
      y: centre.y + radius * s * Math.sin(golden * k),
      z: centre.z + radius * z,
    });
  }
  return points;
}

describe("the cluster cull's horizon and ordered passes (plan R14, R14.T2.a)", () => {
  /** An Earth-sized occluder, the camera `heightM` above it on z. */
  const R_OCC = 6_356_000;
  const above = (heightM: number): Vec3 => ({ x: 0, y: 0, z: R_OCC + heightM });

  it("packs a camera pass's horizon in its scalars and its sixth vec4f, and an ordered pass as kind 3", () => {
    const horizon = cullHorizonOf(above(1_000), R_OCC);
    if (horizon === null) {
      throw new Error("a camera above the occluder has a horizon");
    }
    const out = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    const lit: CameraCullPass = { ...CAMERA, horizon, order: "ordered" };
    packCullPasses(
      [CAMERA, lit],
      [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 64 },
      ],
      out,
    );
    const view = new DataView(out);
    const at = CULL_PASS_BYTES;
    expect([
      view.getUint32(0, true),
      view.getFloat32(28, true),
      view.getUint32(at, true),
      [24, 28, 112, 116, 120, 124].map((o) => view.getFloat32(at + o, true)),
    ]).toEqual([
      2,
      0,
      3,
      [
        Math.fround(horizon.planeM),
        Math.fround(horizon.sin),
        Math.fround(horizon.axis.x),
        Math.fround(horizon.axis.y),
        Math.fround(horizon.axis.z),
        Math.fround(horizon.cos),
      ],
    ]);
    // With a horizon a camera pass holds five planes at most.
    expect(() => {
      packCullPasses(
        [{ ...lit, planes: [...CAMERA.planes, CAMERA.planes[0] ?? fail()] }],
        [{ bucketBase: 0, listBase: 0 }],
        out,
      );
    }).toThrow(RangeError);
  });

  it("forms its horizon from the camera's place, and none at or below the occluder", () => {
    const horizon = cullHorizonOf(above(1_000), R_OCC);
    const c = R_OCC + 1_000;
    expect([
      horizon?.axis,
      horizon?.sin,
      horizon?.planeM,
      cullHorizonOf(above(0), R_OCC),
      cullHorizonOf(above(-50), R_OCC),
    ]).toEqual([{ x: -0, y: -0, z: -1 }, R_OCC / c, (c * c - R_OCC * R_OCC) / c, null, null]);
  });

  it("refuses at grazing altitude a cluster wholly beyond the horizon, keeps one straddling it, and tests nothing below the occluder", () => {
    // 2 m above the occluder: the horizon some 5 km out.
    const camera = above(2);
    const horizon = cullHorizonOf(camera, R_OCC) ?? fail();
    const pass: CameraCullPass = { ...CAMERA, planes: [], horizon, order: "ordered" };
    /** A sphere at angle `angleRad` round the occluder from the camera's foot, `heightM` up. */
    const at = (angleRad: number, heightM: number): Vec3 => ({
      x: (R_OCC + heightM) * Math.sin(angleRad),
      y: 0,
      z: (R_OCC + heightM) * Math.cos(angleRad) - camera.z,
    });
    const horizonRad = Math.acos(R_OCC / camera.z);
    expect([
      // 15 km out, some 8 m lies hidden: a sphere 50 m down is, one of 10 m on the occluder is not.
      horizonRefuses(horizon, at(3 * horizonRad, -50), 10, 0),
      horizonRefuses(horizon, at(3 * horizonRad, 0), 10, 0),
      horizonRefuses(horizon, at(1.01 * horizonRad, 0), 1, 0),
      horizonRefuses(horizon, at(3 * horizonRad, 0), 200, 0),
      cameraKeeps(pass, at(0.5, 0), 1_000, 0),
      cameraKeeps({ ...pass, horizon: cullHorizonOf(above(-1), R_OCC) }, at(0.5, 0), 1_000, 0),
    ]).toEqual([true, false, false, false, false, true]);
  });

  it("never refuses a sphere any of whose points R05's horizon test or the frustum leaves in sight (zero false culls)", () => {
    const next = random(0x14_2b);
    let refused = 0;
    const falseCulls: string[] = [];
    for (let n = 0; n < 4_000; n += 1) {
      const heightM = 10 ** (1 + 5 * next());
      const camera = above(heightM);
      const horizon = cullHorizonOf(camera, R_OCC) ?? fail();
      // A sphere about the horizon's distance away, in any direction, of a cluster's size.
      const reach = Math.sqrt(heightM * (2 * R_OCC + heightM)) * (0.5 + 2 * next());
      const d = unit(next);
      const centre = { x: reach * d.x, y: reach * d.y, z: reach * d.z };
      const radius = 10 ** (next() * 4);
      const pass: CameraCullPass = { ...CAMERA, horizon, order: "ordered" };
      if (!cameraKeeps(pass, centre, radius, 0)) {
        refused += 1;
        const seen = spherePoints(centre, radius).some(
          (p) =>
            !pointOccluded(camera, R_OCC, p) &&
            CAMERA.planes.every((plane) => dot(plane.normal, p) + plane.offsetM >= 0),
        );
        if (seen) {
          falseCulls.push(`${String(heightM)} m: ${JSON.stringify(centre)} r ${String(radius)}`);
        }
      }
    }
    expect(falseCulls).toEqual([]);
    expect(refused).toBeGreaterThan(400);
  });

  it("marks each cluster of the ordered pass with its template or none, and appends nothing for it", () => {
    const records = [
      record({ x: 0, y: 0, z: -100 }, 1),
      record({ x: 0, y: 0, z: 50 }, 1),
      record({ x: 0, y: 0, z: -80 }, 2, "terrain-skirt"),
    ];
    const input = {
      instances: [
        { originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b10, unitM: 0.1 },
        { originM: { x: 0, y: 0, z: 0 }, page: 0, passMask: 0b01, unitM: 0.1 },
      ],
      records: (_page: number, cluster: number) => records[cluster] ?? record(Z, 0),
      clustersPerPage: 3,
      rotationColumns: [X, Y, Z] as const,
      passes: [CAMERA, { ...CAMERA, order: "ordered" as const }],
      places: [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 100 },
      ],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    };
    expect([
      [...cullMarksTwin(input, 0)],
      Object.fromEntries([...cullTwin(input, 0)].toSorted(([a], [b]) => a - b)),
    ]).toEqual([
      // Instance 0 enters the ordered pass: its block ahead kept at stride 1 (template 0), the one
      // behind refused, its strip ahead kept (template 4); instance 1 does not enter it.
      [0, NO_MARK, 4, NO_MARK, NO_MARK, NO_MARK],
      // The appended pass, instance 1's alone.
      { 0: [256], 4: [256 + 2] },
    ]);
  });
});

/** Last frame's visibility in the occlusion's tests: each page's first cluster alone. */
function visible(_page: number, cluster: number): boolean {
  return cluster === 0;
}

/** A pyramid that refuses every sphere. */
function refusesAll(): boolean {
  return true;
}

/** A visibility twin's bits in their keys' order. */
function byPage(map: ReadonlyMap<number, boolean>): Array<readonly [number, boolean]> {
  return [...map].toSorted(([a], [b]) => a - b);
}

/** Marks three a page, instance by instance. */
function marksByInstance(marks: Uint32Array): number[][] {
  return Array.from({ length: marks.length / 3 }, (_, n) => [...marks.subarray(3 * n, 3 * n + 3)]);
}

describe("the two-pass occlusion's passes (plan R14, R14.T3.d)", () => {
  /** Three clusters: a block ahead, one behind the camera, and a strip ahead. */
  const records = [
    record({ x: 0, y: 0, z: -100 }, 1),
    record({ x: 0, y: 0, z: 50 }, 1),
    record({ x: 0, y: 0, z: -80 }, 2, "terrain-skirt"),
  ];
  const ORDERED: CameraCullPass = { ...CAMERA, order: "ordered" };

  /** One dispatch of `pass` over `instances`, the pass the second. */
  function inputOf(pass: CameraCullPass, instances: ReadonlyArray<CullInstance>) {
    return {
      instances,
      records: (_page: number, cluster: number) => records[cluster] ?? record(Z, 0),
      clustersPerPage: 3,
      rotationColumns: [X, Y, Z] as const,
      passes: [null, pass],
      places: [
        { bucketBase: 0, listBase: 0 },
        { bucketBase: 8, listBase: 100 },
      ],
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    };
  }

  const AT_CAMERA = { x: 0, y: 0, z: 0 };
  const INSTANCES: ReadonlyArray<CullInstance> = [
    { originM: AT_CAMERA, page: 0, passMask: 0b10, unitM: 0.1 },
    { originM: AT_CAMERA, page: 1, passMask: 0b10, unitM: 0.1, subWord: 1 },
    { originM: AT_CAMERA, page: 2, passMask: 0b10, unitM: 0.1, whole: true },
    { originM: AT_CAMERA, page: 3, passMask: 0b00, unitM: 0.1 },
  ];

  it("packs the early pass as kind 4 and the late as kind 5", () => {
    const out = new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES);
    const place = { bucketBase: 0, listBase: 0 };
    packCullPasses(
      [ORDERED, { ...ORDERED, occlusion: "early" }, { ...ORDERED, occlusion: "late" }],
      [place, place, place],
      out,
    );
    const view = new DataView(out);
    expect([0, 1, 2].map((n) => view.getUint32(n * CULL_PASS_BYTES, true))).toEqual([3, 4, 5]);
  });

  it("marks in the early pass only the kept clusters visible last frame, none of a sub-patch's", () => {
    const marks = cullMarksTwin(inputOf({ ...ORDERED, occlusion: "early" }, INSTANCES), 0, {
      visible,
      earlyMarks: null,
      refuses: null,
    });
    expect(marksByInstance(marks)).toEqual([
      [0, NO_MARK, NO_MARK],
      [NO_MARK, NO_MARK, NO_MARK],
      [0, NO_MARK, NO_MARK],
      [NO_MARK, NO_MARK, NO_MARK],
    ]);
  });

  it("marks in the late pass the kept clusters the early pass did not, but those the pyramid refuses, never one kept whole", () => {
    const input = inputOf({ ...ORDERED, occlusion: "late" }, INSTANCES);
    const earlyMarks = cullMarksTwin(inputOf({ ...ORDERED, occlusion: "early" }, INSTANCES), 0, {
      visible,
      earlyMarks: null,
      refuses: null,
    });
    const seen = { visible, earlyMarks, refuses: null };
    expect([
      marksByInstance(cullMarksTwin(input, 0, seen)),
      marksByInstance(cullMarksTwin(input, 0, { ...seen, refuses: refusesAll })),
    ]).toEqual([
      // The instance kept whole keeps every cluster, its sphere behind the camera too.
      [
        [NO_MARK, NO_MARK, 4],
        [0, NO_MARK, 4],
        [NO_MARK, 0, 4],
        [NO_MARK, NO_MARK, NO_MARK],
      ],
      [
        [NO_MARK, NO_MARK, NO_MARK],
        [NO_MARK, NO_MARK, NO_MARK],
        [NO_MARK, 0, 4],
        [NO_MARK, NO_MARK, NO_MARK],
      ],
    ]);
  });

  it("writes in the late pass each cluster's visibility, kept and not refused, cleared where its instance enters no pass, and none of a sub-patch's", () => {
    const input = inputOf({ ...ORDERED, occlusion: "late" }, INSTANCES);
    const seen = { visible, earlyMarks: new Uint32Array(12).fill(NO_MARK), refuses: null };
    expect([
      byPage(cullVisibilityTwin(input, seen, 0)),
      byPage(cullVisibilityTwin(input, { ...seen, refuses: refusesAll }, 0)),
      cullVisibilityTwin(inputOf({ ...ORDERED, occlusion: "early" }, INSTANCES), seen).size,
    ]).toEqual([
      [
        [0, true],
        [1, false],
        [2, true],
        [6, true],
        [7, true],
        [8, true],
        [9, false],
        [10, false],
        [11, false],
      ],
      [
        [0, false],
        [1, false],
        [2, false],
        [6, true],
        [7, true],
        [8, true],
        [9, false],
        [10, false],
        [11, false],
      ],
      0,
    ]);
  });

  it("needs the visibility for an occlusion's pass, and the early marks for the late one", () => {
    expect(() => cullMarksTwin(inputOf({ ...ORDERED, occlusion: "early" }, INSTANCES))).toThrow(
      Error,
    );
    expect(() =>
      cullMarksTwin(inputOf({ ...ORDERED, occlusion: "late" }, INSTANCES), 1, {
        visible,
        earlyMarks: null,
        refuses: null,
      }),
    ).toThrow(Error);
  });
});

describe("the cull's two kernels (plan R14, R14.T3.d)", () => {
  it("compose the occlusion's bindings and tests into its variant alone, the plain one's stand-ins in their place", () => {
    const occluding = CLUSTER_CULL_OCCLUSION_KERNEL.reference;
    const plain = CLUSTER_CULL_KERNEL.reference;
    expect([
      occluding.includes("@group(0) @binding(11) var<storage, read_write> history"),
      occluding.includes("fn hzb_refuses("),
      plain.includes("@binding(9)") || occluding.includes("@binding(8)"),
      plain.includes("fn occlusion_seen(word : u32, bit : u32) -> bool {\n  return true;"),
      [occluding, plain].every((source) => !source.includes("//OCCLUSION//")),
    ]).toEqual([true, true, false, true, true]);
  });

  it("lays HzbParams out as hzb.ts packs it", () => {
    const layout = declaredStructLayout(CLUSTER_CULL_OCCLUSION_KERNEL.reference, "HzbParams");
    expect([layout?.members.map((m) => [m.name, m.offsetBytes]), layout?.sizeBytes]).toEqual([
      [
        ["right", 0],
        ["up", 16],
        ["forward", 32],
        ["size", 48],
        ["offsets0", 64],
        ["offsets1", 80],
        ["offsets2", 96],
        ["offsets3", 112],
      ],
      HZB_PARAMS_BYTES,
    ]);
  });
});

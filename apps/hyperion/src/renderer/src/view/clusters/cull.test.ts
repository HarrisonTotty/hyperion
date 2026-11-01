/**
 * The cluster cull's records, passes and twin (plan R14, R14.T1.b): `ClusterRecord`, `CullPass` and
 * `CullParams` packed as `clusterCull.wgsl` lays them out, and the twin's choices of bucket and
 * stride against the box test of `castsInto`.
 */

import { describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { declaredStructLayout } from "../engine/webgpu/uniforms";
import clusterCullWgsl from "./clusterCull.wgsl?raw";
import {
  cameraKeeps,
  cascadeKeeps,
  CULL_ROUNDING_M,
  cullTwin,
  RECEIVER_ROUNDING_M,
  receiversRefuse,
  strideLevel,
} from "./cull";
import {
  type CameraCullPass,
  type CascadeCullPass,
  type CullReceivers,
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  MAX_CULL_PASSES,
  packCullParams,
  packCullPasses,
} from "./cullPass";
import {
  CLUSTER_RECORD_BYTES,
  type ClusterRecord,
  clusterFlags,
  packClusterRecord,
} from "./record";

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

/** A block record at `centreM` of radius `radiusM`. */
function record(
  centreM: Vec3,
  radiusM: number,
  shape: ClusterRecord["shape"] = "terrain-block",
): ClusterRecord {
  return {
    centreM,
    radiusM,
    mode: "strided",
    errorsM: null,
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

  it("lays a pass out in 128 B and the dispatch's parameters in 144 B, as WGSL's CullPass and CullParams", () => {
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
        ["listOffsetsLow", 80],
        ["listOffsetsHigh", 96],
        ["capacitiesLow", 112],
        ["capacitiesHigh", 128],
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
      },
      out,
    );
    const view = new DataView(out);
    const u = (o: number): number => view.getUint32(o, true);
    expect([
      [48, 52, 56, 60, 76].map(u),
      view.getFloat32(72, true),
      [80, 84, 88, 92, 96, 100, 104, 108].map(u),
      [112, 116, 128, 140].map(u),
    ]).toEqual([[3, 8, 96, 4, 8], -1, [0, 10, 20, 30, 40, 45, 50, 55], [10, 10, 5, 5]]);
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
  toleranceM: 0,
  strideCap: 1,
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

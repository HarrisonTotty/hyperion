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
  cascadeKeeps,
  CULL_ROUNDING_M,
  cullTwin,
  RECEIVER_ROUNDING_M,
  receiversRefuse,
  strideLevel,
} from "./cull";
import {
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

/**
 * An edge bake's stand-ins on the cluster path (plan R14, R14.T3.c's edge stand-ins;
 * `GeometrySettings.edgeStandIns`): the slot's opaque blocks written with its records, the routing of
 * each stand-in to its form, an edge mask's runs and the side runs its siblings draw, the records'
 * classes the cull takes, the strips' survey read inside their set, and the cull's twin in each form.
 */

import { describe, expect, it } from "vitest";

import { WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir } from "../terrain/cube";
import { packSurveyMask, surveyedAt } from "../terrain/gpu/resources";
import { subPatchWord } from "../terrain/gpu/uniforms";
import { type PatchKey, patchKeyString } from "../terrain/patchKey";
import { planetGeometry, surfacePoint } from "../terrain/planet";
import type { Vec3 } from "../../geometry/vec3";
import { CULL_INPUT_WHOLE, type CullInstance, cullTwin } from "./cull";
import type { CameraCullPass } from "./cullPass";
import { CLUSTER_FLAG_MAY_DISCARD, CLUSTER_RECORD_BYTES, type ClusterRecord } from "./record";
import { TERRAIN_DISCARD_TEMPLATES, terrainClusterMesh } from "./templates";
import {
  ALL_BLOCKS_OPAQUE,
  blockOpaque,
  type EdgeMaskRecord,
  edgeStandInForm,
  maskFootSurveyPlace,
  shareMaskRuns,
  standInOnOpaqueBlocks,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_INPUT_COLUMN_RUN_SHARED,
  TERRAIN_INPUT_EDGE_MASK,
  TERRAIN_INPUT_ROW_RUN_SHARED,
  TERRAIN_INPUT_SURVEYED,
  TERRAIN_PAGE_CLUSTERS,
  TERRAIN_SUB_PATCH_MASK,
  terrainClusterSurvey,
  terrainFinPlace,
  terrainFinTemplateRef,
  terrainMaskCluster,
  terrainMaskRuns,
  type TerrainOpaqueBlocks,
  terrainRecordClass,
  terrainTemplateRef,
  writeTerrainClusterRecords,
} from "./terrain";

const X: Vec3 = { x: 1, y: 0, z: 0 };
const Y: Vec3 = { x: 0, y: 1, z: 0 };
const Z: Vec3 = { x: 0, y: 0, z: 1 };

/** A mask's sub-patch word, `levels` down at part (i, j). */
function mask(levels: number, i: number, j: number): number {
  return (TERRAIN_SUB_PATCH_MASK | subPatchWord(levels, i, j)) >>> 0;
}

/** A survey mask whose vertices with x below `x0` are surveyed, the rest not, at cell code `code`. */
function surveyedLeftOf(x0: number, code: number): Uint8Array {
  const raw = new Uint8Array(65 * 65);
  for (let y = 0; y <= 64; y += 1) {
    raw.fill(255, 65 * y, 65 * y + x0);
  }
  return packSurveyMask(raw, code);
}

/** The opaque blocks of exactly `blocks`. */
function opaqueOf(blocks: ReadonlyArray<number>): TerrainOpaqueBlocks {
  const words = [0, 0];
  for (const b of blocks) {
    words[b >> 5] = ((words[b >> 5] ?? 0) | (1 << (b % 32))) >>> 0;
  }
  return [words[0] ?? 0, words[1] ?? 0];
}

/** The blocks of the columns `from` to `to` (inclusive), every row. */
function columns(from: number, to: number): number[] {
  const out: number[] = [];
  for (let by = 0; by < 8; by += 1) {
    for (let bx = from; bx <= to; bx += 1) {
      out.push(8 * by + bx);
    }
  }
  return out;
}

/** Run `c`'s line, place and half, `terrainFinPlace`'s, as text. */
function runOf(c: number): string {
  const place = terrainFinPlace(c);
  return place.line === "head"
    ? `head ${String(place.edge)}/${String(place.half)}`
    : `${place.line} ${String(place.at)}/${String(place.half)}`;
}

/** Each of `areas`' edge masks' records' side-run bits, as `shareMaskRuns` sets them. */
function shared(areas: ReadonlyArray<PatchKey>): number[] {
  const masks = new Map<string, EdgeMaskRecord>(
    areas.map((area, record) => [patchKeyString(area), { area, record }]),
  );
  const bits = areas.map(() => TERRAIN_INPUT_EDGE_MASK);
  shareMaskRuns(masks, bits);
  return bits.map((b) => b & (TERRAIN_INPUT_COLUMN_RUN_SHARED | TERRAIN_INPUT_ROW_RUN_SHARED));
}

/** Patch (i, j) of face 3's level 9. */
function areaAt(i: number, j: number): PatchKey {
  return { face: 3, level: 9, i, j };
}

/**
 * A page of terrain's layout: its clusters' records by the survey's class `classOf(c)`, at their
 * own template references.
 */
function page(classOf: (c: number) => "opaque" | "edge" | "none"): ClusterRecord[] {
  return Array.from({ length: TERRAIN_PAGE_CLUSTERS }, (_, c) => {
    const run = c >= TERRAIN_CLUSTERS_PER_PAGE;
    const cls = classOf(c);
    return {
      centreM: { x: 0, y: 0, z: -10 - c },
      radiusM: cls === "none" ? -1 : 5,
      mode: "strided",
      strideErrorBits: null,
      templateRef: run ? terrainFinTemplateRef(c) : terrainTemplateRef(c),
      shape:
        c < TERRAIN_BLOCKS
          ? "terrain-block"
          : run
            ? c < TERRAIN_CLUSTERS_PER_PAGE + 8
              ? "terrain-head"
              : "terrain-fin"
            : "terrain-skirt",
      mayDiscard: run || cls === "edge",
      material: 0,
    };
  });
}

/** An edge bake's records: a mix of the three classes, every run of a cluster but every fifth. */
const EDGE = page((c) =>
  c >= TERRAIN_CLUSTERS_PER_PAGE
    ? c % 5 === 0
      ? "none"
      : "edge"
    : ((["opaque", "edge", "none"] as const)[c % 3] ?? "opaque"),
);
/** An interior bake's: every block and strip opaque, no run. */
const INTERIOR = page((c) => (c >= TERRAIN_CLUSTERS_PER_PAGE ? "none" : "opaque"));

describe("an edge bake's opaque blocks (plan R14, R14.T3.c's edge stand-ins)", () => {
  it("are written with the slot's records: each block whose record is opaque, every one with no survey mask", () => {
    const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
    const key = { face: 2, level: 7, i: 40, j: 61 } as const;
    const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), 0);
    const source = {
      key,
      originM: { x, y, z },
      heightRangeM: [-50, 300],
      skirtDepthM: 10,
    } as const;
    const planet = planetGeometry(WGS84_FIGURE, null);
    const surveyMask = surveyedLeftOf(37, 3);
    const opaque = writeTerrainClusterRecords(planet, { ...source, surveyMask }, records, 0);
    const recordOpaque = (b: number): boolean =>
      records.getFloat32(b * CLUSTER_RECORD_BYTES + 12, true) >= 0 &&
      (records.getUint32(b * CLUSTER_RECORD_BYTES + 28, true) & CLUSTER_FLAG_MAY_DISCARD) === 0;
    const blocks = Array.from({ length: TERRAIN_BLOCKS }, (_, b) => b);
    expect(blocks.map((b) => blockOpaque(opaque, b))).toEqual(blocks.map(recordOpaque));
    expect(blocks.map((b) => blockOpaque(opaque, b))).toEqual(
      blocks.map((b) => terrainClusterSurvey(surveyMask, b) === "whole"),
    );
    // Cells of 8 quads, surveyed left of x = 37: the columns of blocks 0 to 3 alone.
    expect(opaque).toEqual(opaqueOf(columns(0, 3)));
    expect(writeTerrainClusterRecords(planet, source, records, 0)).toEqual(ALL_BLOCKS_OPAQUE);
  });

  it("hold a stand-in on blocks all opaque by its area's set one to three levels down, or by the one block that holds a deeper area", () => {
    const opaque = opaqueOf(columns(0, 3));
    expect([
      standInOnOpaqueBlocks(opaque, subPatchWord(1, 0, 1)),
      standInOnOpaqueBlocks(opaque, subPatchWord(1, 1, 0)),
      standInOnOpaqueBlocks(opaque, mask(2, 1, 3)),
      standInOnOpaqueBlocks(opaque, mask(2, 2, 3)),
      standInOnOpaqueBlocks(opaque, mask(3, 3, 7)),
      standInOnOpaqueBlocks(opaque, mask(3, 4, 7)),
      // Four levels down at (7, 9): block (3, 4); at (8, 9), block (4, 4).
      standInOnOpaqueBlocks(opaque, subPatchWord(4, 7, 9)),
      standInOnOpaqueBlocks(opaque, subPatchWord(4, 8, 9)),
      standInOnOpaqueBlocks(opaque, subPatchWord(6, 31, 0)),
      standInOnOpaqueBlocks(opaque, 0),
      standInOnOpaqueBlocks(ALL_BLOCKS_OPAQUE, 0),
    ]).toEqual([true, false, true, false, true, false, true, false, true, false, true]);
  });
});

describe("an edge bake's stand-in's form (plan R14, R14.T3.c's edge stand-ins)", () => {
  const opaque = opaqueOf(columns(0, 3));
  it("routes a stand-in by `edgeStandIns`: whole as its edge patch, on opaque blocks as an interior bake's, one level down across the survey's edge as an edge mask where masks are drawn, the rest by the lit mesh", () => {
    const cases: Array<readonly [Parameters<typeof edgeStandInForm>, string]> = [
      [["off", 0, opaque, true], "lit-mesh"],
      [["off", subPatchWord(1, 0, 0), opaque, true], "lit-mesh"],
      [["surveyed", 0, opaque, true], "edge-patch"],
      [["surveyed", subPatchWord(1, 0, 0), opaque, true], "surveyed"],
      [["surveyed", subPatchWord(5, 3, 30), opaque, false], "surveyed"],
      [["surveyed", subPatchWord(1, 1, 0), opaque, true], "lit-mesh"],
      [["masks", subPatchWord(1, 1, 0), opaque, true], "edge-mask"],
      [["masks", subPatchWord(1, 1, 0), opaque, false], "lit-mesh"],
      [["masks", subPatchWord(2, 2, 0), opaque, true], "lit-mesh"],
      [["masks", subPatchWord(2, 0, 0), opaque, true], "surveyed"],
      [["masks", subPatchWord(1, 0, 0), null, true], "lit-mesh"],
      [["masks", 0, null, true], "edge-patch"],
    ];
    expect(cases.map(([args]) => edgeStandInForm(...args))).toEqual(cases.map(([, form]) => form));
  });
});

describe("an edge mask's runs (plan R14, R14.T3.c's edge stand-ins)", () => {
  it("are its quadrant's two interior fins, its two side fins on column and row 32 and its two heads on its slot's patch's edges", () => {
    const quadrants = [
      [0, 0],
      [1, 0],
      [0, 1],
      [1, 1],
    ] as const;
    expect(
      quadrants.map(([i, j]) =>
        terrainMaskRuns(mask(1, i, j), TERRAIN_INPUT_EDGE_MASK)
          .map(runOf)
          .toSorted(),
      ),
    ).toEqual([
      ["column 16/0", "column 32/0", "head 0/0", "head 3/1", "row 16/0", "row 32/0"],
      ["column 32/0", "column 48/0", "head 0/1", "head 1/0", "row 16/1", "row 32/1"],
      ["column 16/1", "column 32/1", "head 2/1", "head 3/0", "row 32/0", "row 48/0"],
      ["column 32/1", "column 48/1", "head 1/1", "head 2/0", "row 32/1", "row 48/1"],
    ]);
  });

  it("leave a side run its sibling draws, and are none for a mask that is not an edge mask one level down", () => {
    expect([
      terrainMaskRuns(mask(1, 1, 1), TERRAIN_INPUT_EDGE_MASK | TERRAIN_INPUT_COLUMN_RUN_SHARED)
        .map(runOf)
        .toSorted(),
      terrainMaskRuns(
        mask(1, 1, 1),
        TERRAIN_INPUT_EDGE_MASK | TERRAIN_INPUT_COLUMN_RUN_SHARED | TERRAIN_INPUT_ROW_RUN_SHARED,
      )
        .map(runOf)
        .toSorted(),
      terrainMaskRuns(mask(1, 1, 1), 0),
      terrainMaskRuns(mask(2, 1, 1), TERRAIN_INPUT_EDGE_MASK),
      terrainMaskRuns(subPatchWord(1, 1, 1), TERRAIN_INPUT_EDGE_MASK),
    ]).toEqual([
      ["column 48/1", "head 1/1", "head 2/0", "row 32/1", "row 48/1"],
      ["column 48/1", "head 1/1", "head 2/0", "row 48/1"],
      [],
      [],
      [],
    ]);
  });

  it("are drawn by an edge mask by their spheres, its strips whatever theirs, and its other fin clusters not at all", () => {
    const word = mask(1, 0, 1);
    const runs = terrainMaskRuns(word, TERRAIN_INPUT_EDGE_MASK);
    const draws = Array.from({ length: TERRAIN_PAGE_CLUSTERS }, (_, c) =>
      terrainMaskCluster(word, c, TERRAIN_INPUT_EDGE_MASK),
    );
    expect([
      draws.filter((d) => d === "whole").length,
      draws.filter((d, c) => d === "sphere" && c >= TERRAIN_CLUSTERS_PER_PAGE),
      draws.filter((d, c) => d === "sphere" && c < TERRAIN_BLOCKS).length,
    ]).toEqual([16, runs.map(() => "sphere"), 16]);
    // Without its bit it is an interior bake's mask, as before: no run.
    expect(
      Array.from({ length: TERRAIN_PAGE_CLUSTERS }, (_, c) => terrainMaskCluster(word, c)),
    ).toEqual(draws.map((d, c) => (c >= TERRAIN_CLUSTERS_PER_PAGE ? "none" : d)));
  });

  it("each draw a side run once: the quadrant of x 0 a column's and of y 0 a row's, where its sibling of the same slot is an edge mask too", () => {
    const C = TERRAIN_INPUT_COLUMN_RUN_SHARED;
    const R = TERRAIN_INPUT_ROW_RUN_SHARED;
    expect([
      shared([areaAt(10, 20), areaAt(11, 20), areaAt(10, 21), areaAt(11, 21)]),
      shared([areaAt(10, 20), areaAt(11, 20)]),
      shared([areaAt(11, 21), areaAt(11, 20)]),
      shared([areaAt(11, 21)]),
      // Across a slot's patch's edge: siblings of other slots, which share no run.
      shared([areaAt(11, 20), areaAt(12, 20)]),
    ]).toEqual([[0, C, R, C | R], [0, C], [R, 0], [0], [0, 0]]);
  });
});

describe("the records' classes the cull takes of an edge bake's stand-in (plan R14, R14.T3.c's edge stand-ins)", () => {
  const classes = [
    { real: true, mayDiscard: false },
    { real: true, mayDiscard: true },
    { real: false, mayDiscard: false },
  ] as const;

  it("are an interior bake's on opaque blocks: blocks and strips real and opaque whatever their records, no fin cluster", () => {
    const clusters = [0, 63, 64, 95, 96, 115];
    expect(
      clusters.map((c) => classes.map((r) => terrainRecordClass(r, c, TERRAIN_INPUT_SURVEYED))),
    ).toEqual(
      clusters.map((c) =>
        classes.map(() => ({ real: c < TERRAIN_CLUSTERS_PER_PAGE, mayDiscard: false })),
      ),
    );
  });

  it("are an edge mask's strips real and may-discard whatever their records, its blocks and runs their own", () => {
    const clusters = [0, 63, 64, 95, 96, 115];
    expect(
      clusters.map((c) => classes.map((r) => terrainRecordClass(r, c, TERRAIN_INPUT_EDGE_MASK))),
    ).toEqual(
      clusters.map((c) =>
        classes.map((r) =>
          c >= TERRAIN_BLOCKS && c < TERRAIN_CLUSTERS_PER_PAGE
            ? { real: true, mayDiscard: true }
            : r,
        ),
      ),
    );
  });

  it("are the records' own without the bits, bit for bit", () => {
    for (const c of [0, 70, 100]) {
      for (const r of classes) {
        expect(terrainRecordClass(r, c, 0)).toBe(r);
      }
    }
  });
});

describe("a stand-in's mask's strips' survey read (plan R14, R14.T3.c's edge stand-ins)", () => {
  it("reads a foot half a quad inside its set, the set taken as a patch, and any other half a quad inside its patch", () => {
    expect([
      maskFootSurveyPlace(0, 64, 0),
      maskFootSurveyPlace(0, 12.25, 64),
      maskFootSurveyPlace(subPatchWord(1, 1, 0), 32, 10),
      maskFootSurveyPlace(mask(1, 0, 0), 32, 17),
      maskFootSurveyPlace(mask(1, 1, 0), 32, 32),
      maskFootSurveyPlace(mask(2, 1, 2), 32, 40.5),
      maskFootSurveyPlace(mask(3, 7, 7), 64, 56),
    ]).toEqual([
      [63.5, 0.5],
      [12.25, 63.5],
      [32, 10],
      [31.5, 17],
      [32.5, 31.5],
      [31.5, 40.5],
      [63.5, 56.5],
    ]);
  });

  it("keeps a far side's foot by its own cell, where its slot's patch's read takes the neighbour's", () => {
    // Cells of 32 quads: x below 32 surveyed, the rest not. Quadrant (0, 0)'s far side lies on x = 32.
    const packed = surveyedLeftOf(32, 5);
    const word = mask(1, 0, 0);
    const patchRead = (x: number, y: number): boolean =>
      surveyedAt(packed, ...maskFootSurveyPlace(0, x, y));
    const setRead = (x: number, y: number): boolean =>
      surveyedAt(packed, ...maskFootSurveyPlace(word, x, y));
    expect([patchRead(32, 9), setRead(32, 9), setRead(0, 9), setRead(17, 32)]).toEqual([
      false,
      true,
      true,
      true,
    ]);
  });
});

describe("the cull's twin of an edge bake's stand-ins (plan R14, R14.T3.c's edge stand-ins)", () => {
  /** A camera pass of no planes at stride 1, which keeps every cluster of a sphere. */
  const PASS: CameraCullPass = {
    kind: "camera",
    planes: [],
    toleranceRad: 0,
    strideCap: 1,
    horizon: null,
    order: "appended",
  };
  const { templates } = terrainClusterMesh();

  /** Each instance's drawn clusters, cluster by cluster, and whether by a may-discard template. */
  function drawn(
    records: ReadonlyArray<ClusterRecord>,
    instances: ReadonlyArray<Omit<CullInstance, "originM" | "page" | "passMask" | "unitM">>,
  ): Array<Array<readonly [number, boolean]>> {
    const buckets = cullTwin({
      instances: instances.map((instance) => ({
        originM: { x: 0, y: 0, z: 0 },
        page: 0,
        passMask: 1,
        unitM: 1,
        ...instance,
      })),
      records: (_page, c) => records[c] ?? records[0] ?? EDGE[0] ?? INTERIOR[0] ?? fail(),
      clustersPerPage: TERRAIN_PAGE_CLUSTERS,
      rotationColumns: [X, Y, Z],
      passes: [PASS],
      places: [{ bucketBase: 0, listBase: 0 }],
      templates: templates.length,
      viewAxis: { x: 0, y: 0, z: -1 },
      discardTemplates: TERRAIN_DISCARD_TEMPLATES,
    });
    const out = instances.map((): Array<readonly [number, boolean]> => []);
    for (const [t, entries] of buckets) {
      for (const entry of entries) {
        out[Math.floor(entry / 256)]?.push([entry % 256, templates[t]?.mayDiscard === true]);
      }
    }
    return out.map((list) => list.toSorted(([a], [b]) => a - b));
  }

  it("draws a wholly surveyed edge stand-in, a mask or an own grid, as an interior bake's draws it", () => {
    const forms = [
      { subWord: mask(1, 1, 0) },
      { subWord: mask(2, 3, 1) },
      { subWord: mask(3, 0, 7) },
      { subWord: subPatchWord(5, 12, 30), whole: true },
    ];
    const asInterior = drawn(INTERIOR, forms);
    const asSurveyed = drawn(
      EDGE,
      forms.map((form) => ({ ...form, standInBits: TERRAIN_INPUT_SURVEYED })),
    );
    expect(asSurveyed).toEqual(asInterior);
    // Without the bit the same edge records draw by their classes: the slot's, not its area's.
    expect(drawn(EDGE, forms)).not.toEqual(asInterior);
  });

  it("draws an edge mask's blocks of a cluster by their classes, its 16 strips by the may-discard template and its quadrant's runs of a cluster", () => {
    const word = mask(1, 1, 0);
    const [got = []] = drawn(EDGE, [{ subWord: word, standInBits: TERRAIN_INPUT_EDGE_MASK }]);
    const want: Array<readonly [number, boolean]> = [];
    const runs = terrainMaskRuns(word, TERRAIN_INPUT_EDGE_MASK);
    for (let c = 0; c < TERRAIN_PAGE_CLUSTERS; c += 1) {
      const record = EDGE[c] ?? fail();
      const inSet = terrainMaskCluster(word, c) !== "none";
      if (c >= TERRAIN_BLOCKS && c < TERRAIN_CLUSTERS_PER_PAGE) {
        if (inSet) {
          want.push([c, true]);
        }
      } else if ((c < TERRAIN_BLOCKS ? inSet : runs.includes(c)) && record.radiusM >= 0) {
        want.push([c, record.mayDiscard]);
      }
    }
    expect(got).toEqual(want);
    expect(got.filter(([c]) => c >= TERRAIN_BLOCKS && c < TERRAIN_CLUSTERS_PER_PAGE)).toEqual(
      Array.from({ length: 16 }, (_, n) => [64 + 8 * Math.floor(n / 4) + (n % 4), true]),
    );
  });

  it("draws each side run of one slot's four edge masks once, as `shareMaskRuns` leaves them", () => {
    const quadrants = [
      [0, 0],
      [1, 0],
      [0, 1],
      [1, 1],
    ] as const;
    const areas = quadrants.map(([i, j]): PatchKey => ({ face: 0, level: 4, i: 6 + i, j: 2 + j }));
    const bits = areas.map(() => TERRAIN_INPUT_EDGE_MASK);
    shareMaskRuns(
      new Map(areas.map((area, record) => [patchKeyString(area), { area, record }])),
      bits,
    );
    const runs = drawn(
      page((c) => (c >= TERRAIN_CLUSTERS_PER_PAGE ? "edge" : "opaque")),
      quadrants.map(([i, j], n) => ({ subWord: mask(1, i, j), standInBits: bits[n] ?? 0 })),
    ).map((list) => list.filter(([c]) => c >= TERRAIN_CLUSTERS_PER_PAGE).map(([c]) => c));
    const times = new Map<number, number>();
    for (const c of runs.flat()) {
      times.set(c, (times.get(c) ?? 0) + 1);
    }
    // Every fin and head run of the slot's patch drawn, each by one mask.
    expect([[...times.keys()].toSorted((a, b) => a - b), [...new Set(times.values())]]).toEqual([
      Array.from({ length: TERRAIN_PAGE_CLUSTERS - TERRAIN_CLUSTERS_PER_PAGE }, (_, n) => 96 + n),
      [1],
    ]);
  });

  it("draws an instance without the bits as before, bit for bit", () => {
    const forms = [
      { subWord: 0 },
      { subWord: mask(1, 0, 1) },
      { subWord: subPatchWord(4, 3, 3), whole: true },
    ];
    expect(
      drawn(
        EDGE,
        forms.map((form) => ({ ...form, standInBits: 0 })),
      ),
    ).toEqual(drawn(EDGE, forms));
  });

  it("keeps an own grid on opaque blocks whole with the bit beside its CULL_INPUT_WHOLE", () => {
    expect(TERRAIN_INPUT_SURVEYED & CULL_INPUT_WHOLE).toBe(0);
    const bits = [
      TERRAIN_INPUT_SURVEYED,
      TERRAIN_INPUT_EDGE_MASK,
      TERRAIN_INPUT_COLUMN_RUN_SHARED,
      TERRAIN_INPUT_ROW_RUN_SHARED,
    ];
    // Distinct bits, past the passes' eight and below CULL_INPUT_WHOLE.
    expect(bits.reduce((a, b) => a | b, 0)).toBe(0x7800_0000);
  });
});

/** Fails a test whose fixture lacks what it needs. */
function fail(): never {
  throw new Error("the test's fixture lacks what it needs");
}

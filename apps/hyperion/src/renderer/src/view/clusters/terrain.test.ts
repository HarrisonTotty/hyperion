/**
 * Terrain as the cluster path's first consumer (plan R14, R14.T1.b): its clusters cover the
 * interior patches' mesh exactly at stride 1, its templates stitch at every stride, and its records'
 * spheres hold every vertex the GPU can place, morphs and skirt feet included.
 */

import { describe, expect, it } from "vitest";

import { worstOutsideClusters } from "../../test/clusterBounds";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir } from "../terrain/cube";
import { litInteriorMeshData } from "../terrain/gpu/resources";
import { subPatchWord } from "../terrain/gpu/uniforms";
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, planetGeometry, surfacePoint } from "../terrain/planet";
import {
  CLUSTER_FLAG_MAY_DISCARD,
  CLUSTER_FLAG_MEASURED,
  CLUSTER_RECORD_BYTES,
  CLUSTER_SHAPE_CODE,
} from "./record";
import {
  edgeVertex,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_PAGE_CLUSTERS,
  terrainClusterGrid,
  terrainClusterPlace,
  terrainMaskCluster,
  TERRAIN_SUB_PATCH_MASK,
  type TerrainClusterSource,
  terrainTemplateRef,
  writeTerrainClusterRecords,
} from "./terrain";
import { STRIDES, terrainClusterMesh } from "./templates";

const MESH = terrainClusterMesh();

/** A template vertex's (lx, ly, code). */
function local(v: number): readonly [number, number, number] {
  const p = MESH.mesh.positions;
  return [p[3 * v] ?? Number.NaN, p[3 * v + 1] ?? Number.NaN, p[3 * v + 2] ?? Number.NaN];
}

/** Template `t`'s triangles of cluster `cluster`, each its three vertices' lit-mesh (x, y, code). */
function clusterTriangles(t: number, cluster: number): string[] {
  const template = MESH.templates[t];
  const indices = MESH.mesh.indices ?? new Uint32Array();
  const out: string[] = [];
  for (let n = 0; n < (template?.indexCount ?? 0); n += 3) {
    const at = (template?.firstIndex ?? 0) + n;
    out.push(
      [0, 1, 2]
        .map((k) => terrainClusterGrid(cluster, local(indices[at + k] ?? 0)).join(" "))
        .join(" | "),
    );
  }
  return out;
}

describe("terrain's clusters (plan R14, R14.T1.b)", () => {
  it("are 64 blocks of 8 × 8 quads and 32 skirt-foot strips of 8, eight an edge", () => {
    const places = Array.from({ length: TERRAIN_CLUSTERS_PER_PAGE }, (_, c) =>
      terrainClusterPlace(c),
    );
    expect(places.filter((p) => p.shape === "terrain-block")).toHaveLength(TERRAIN_BLOCKS);
    expect(
      [0, 1, 2, 3].map(
        (e) => places.filter((p) => p.shape === "terrain-skirt" && p.edge === e).length,
      ),
    ).toEqual([8, 8, 8, 8]);
    expect(() => terrainClusterPlace(96)).toThrow(RangeError);
  });

  it("draw at stride 1 exactly the interior mesh's triangles, each in its own vertex order", () => {
    const lit = litInteriorMeshData();
    const at = (v: number): string =>
      [0, 1, 2].map((k) => String(lit.positions[3 * v + k] ?? Number.NaN)).join(" ");
    const interior = new Set<string>();
    for (let n = 0; n < lit.indices.length; n += 3) {
      interior.add([0, 1, 2].map((k) => at(lit.indices[n + k] ?? 0)).join(" | "));
    }
    const drawn: string[] = [];
    for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
      const t = cluster < TERRAIN_BLOCKS ? 0 : STRIDES.length;
      drawn.push(...clusterTriangles(t, cluster));
    }
    expect([drawn.length, new Set(drawn).size]).toEqual([lit.indices.length / 3, interior.size]);
    expect(drawn.filter((triangle) => !interior.has(triangle))).toEqual([]);
  });

  it("give each template its triangles: 128, 32, 8 and 2 a block, 16, 8, 4 and 2 a strip", () => {
    expect(MESH.templates.map((t) => [t.shape, t.stride, t.triangles, t.indexCount / 3])).toEqual([
      ["terrain-block", 1, 128, 128],
      ["terrain-block", 2, 32, 32],
      ["terrain-block", 4, 8, 8],
      ["terrain-block", 8, 2, 2],
      ["terrain-skirt", 1, 16, 16],
      ["terrain-skirt", 2, 8, 8],
      ["terrain-skirt", 4, 4, 4],
      ["terrain-skirt", 8, 2, 2],
      ["terrain-block", 1, 128, 128],
      ["terrain-skirt", 1, 16, 16],
      ["terrain-fin", 1, 128, 128],
      ["terrain-head", 1, 64, 64],
    ]);
    expect(MESH.templates.map((t) => t.errorScale)).toEqual([1, 2, 4, 8, 1, 2, 4, 8, 1, 1, 1, 1]);
    expect(MESH.templates.map((t) => t.mayDiscard)).toEqual([
      ...Array.from({ length: 8 }, () => false),
      ...Array.from({ length: 4 }, () => true),
    ]);
  });

  it("wind every block triangle anticlockwise in the grid's (x, y), as the lit mesh does, at every stride", () => {
    for (let t = 0; t < STRIDES.length; t += 1) {
      for (const triangle of clusterTriangles(t, 27)) {
        const [a, b, c] = triangle.split(" | ").map((v) => v.split(" ").map(Number));
        const cross =
          ((b?.[0] ?? 0) - (a?.[0] ?? 0)) * ((c?.[1] ?? 0) - (a?.[1] ?? 0)) -
          ((b?.[1] ?? 0) - (a?.[1] ?? 0)) * ((c?.[0] ?? 0) - (a?.[0] ?? 0));
        expect(cross).toBeGreaterThan(0);
      }
    }
  });

  it("share every edge vertex of a block at stride s with its neighbour at s, and close each strip to its block", () => {
    /** The grid vertices a cluster's template at `t` draws. */
    const verticesOf = (t: number, cluster: number): Set<string> =>
      new Set(clusterTriangles(t, cluster).flatMap((triangle) => triangle.split(" | ")));
    STRIDES.forEach((stride, l) => {
      // Block (3, 4) and its right-hand neighbour (4, 4) along x = 32.
      const left = verticesOf(l, 4 * 8 + 3);
      const right = verticesOf(l, 4 * 8 + 4);
      const seam = [...left].filter((v) => v.startsWith("32 "));
      expect(seam).toHaveLength(8 / stride + 1);
      expect(seam.filter((v) => !right.has(v))).toEqual([]);
      // Edge 0's strip 3 hangs from block (3, 0): its grid vertices are the block's on y = 0.
      const strip = [...verticesOf(STRIDES.length + l, TERRAIN_BLOCKS + 3)].filter((v) =>
        v.endsWith(" 0"),
      );
      const blockEdge = [...verticesOf(l, 3)].filter((v) => v.endsWith(" 0 0"));
      expect(strip.toSorted()).toEqual(blockEdge.toSorted());
    });
  });

  it("name each cluster's first vertex and a strip's edge in its template reference", () => {
    for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
      const ref = terrainTemplateRef(cluster);
      const [x, y] = terrainClusterGrid(cluster, [0, 0, 0]);
      const place = terrainClusterPlace(cluster);
      expect([ref & 0xff, (ref >> 8) & 0xff, ref >> 16]).toEqual([x, y, place.edge]);
    }
    expect(edgeVertex(2, 8)).toEqual([56, 64]);
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

const MOON_LIKE_FIGURE: BodyFigure = {
  equatorialRadiusM: 1_737_400,
  polarRadiusM: 1_736_000,
  pole: null,
};

/** A level-9 patch's record source, and four measured errors a cluster. */
function measuredSource(): {
  readonly source: TerrainClusterSource;
  readonly clusterErrors: Float16Array;
} {
  const key = { face: 3, level: 9, i: 300, j: 41 } as const;
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), 0);
  return {
    source: { key, originM: { x, y, z }, heightRangeM: [0, 50], skirtDepthM: 4 },
    clusterErrors: Float16Array.from(
      { length: 4 * TERRAIN_CLUSTERS_PER_PAGE },
      (_, n) => (n % 4) * (1 + Math.floor(n / 4)),
    ),
  };
}

describe("a stand-in's mask of its slot's patch's own blocks (plan R14, Design note 19)", () => {
  it("draws the blocks over its area, a 4 × 4, 2 × 2 or 1 × 1 set, and the first of each side's strips, whatever their spheres", () => {
    const counts = [1, 2, 3].map((levels) => {
      const word = TERRAIN_SUB_PATCH_MASK | subPatchWord(levels, 0, 0);
      const of = Array.from({ length: TERRAIN_PAGE_CLUSTERS }, (_, c) =>
        terrainMaskCluster(word, c),
      );
      return [of.filter((m) => m === 1).length, of.filter((m) => m === 2).length];
    });
    expect([counts, terrainMaskCluster(subPatchWord(2, 1, 1), 100)]).toEqual([
      [
        [16, 16],
        [4, 8],
        [1, 4],
      ],
      1,
    ]);
  });

  it("lays its strips on its set's sides anticlockwise, as a patch's edges are, its own edge strips where a side lies on one", () => {
    // Two levels down at part (1, 2): the set x 16 to 32, y 32 to 48.
    const word = TERRAIN_SUB_PATCH_MASK | subPatchWord(2, 1, 2);
    const at = (cluster: number, lx: number): string =>
      terrainClusterGrid(cluster, [lx, 0, 1], word).slice(0, 2).join();
    const corner = TERRAIN_SUB_PATCH_MASK | subPatchWord(1, 0, 0);
    expect([
      [at(64, 0), at(64 + 1, 8)],
      [at(72, 0), at(72 + 1, 8)],
      [at(80, 0), at(80 + 1, 8)],
      [at(88, 0), at(88 + 1, 8)],
      terrainClusterGrid(64 + 3, [5, 0, 1], corner)
        .slice(0, 2)
        .join(),
      terrainClusterGrid(64 + 3, [5, 0, 1])
        .slice(0, 2)
        .join(),
    ]).toEqual([
      ["16,32", "32,32"],
      ["32,32", "32,48"],
      ["32,48", "16,48"],
      ["16,48", "16,32"],
      "29,0",
      "29,0",
    ]);
  });
});

describe("terrain's cluster records (plan R14, R14.T1.b)", () => {
  /** Every vertex the GPU places of a random patch, its morphs and its feet, against its clusters. */
  function worstOutside(figure: BodyFigure, key: PatchKey, seed: number): number {
    const next = random(seed);
    const span = 50 + 9_000 * next();
    const low = -span * next();
    const high = low + span;
    const skirtDepthM = 1 + 500 * next();
    const own = Float64Array.from({ length: 65 * 65 }, () => Math.fround(low + span * next()));
    const morph = Float64Array.from({ length: 65 * 65 }, () => Math.fround(low + span * next()));
    const h0M = own[32 * 65 + 32] ?? 0;
    const [x, y, z] = surfacePoint(figure, vertexDir(key, 32, 32), h0M);
    return worstOutsideClusters({
      figure,
      key,
      originM: { x, y, z },
      originHeightM: h0M,
      own,
      morph,
      heightRangeM: [low, high],
      skirtDepthM,
    });
  }

  it.each([
    ["an Earth's root", WGS84_FIGURE, { face: 1, level: 0, i: 0, j: 0 }],
    ["an Earth's level 3", WGS84_FIGURE, { face: 4, level: 3, i: 5, j: 2 }],
    ["an Earth's level 9 at a face's corner", WGS84_FIGURE, { face: 2, level: 9, i: 511, j: 0 }],
    ["an Earth's level 16", WGS84_FIGURE, { face: 0, level: 16, i: 40_000, j: 31_000 }],
    ["an Earth's finest level", WGS84_FIGURE, { face: 5, level: 19, i: 300_001, j: 7 }],
    ["a Moon's level 2", MOON_LIKE_FIGURE, { face: 3, level: 2, i: 1, j: 3 }],
    ["a Moon's level 12", MOON_LIKE_FIGURE, { face: 1, level: 12, i: 2_047, j: 2_048 }],
  ] as const)(
    "hold every vertex of %s, morphed and as skirt feet, inside their clusters' spheres",
    (_name, figure, key) => {
      for (const seed of [1, 2, 3]) {
        expect(worstOutside(figure, key, seed)).toBeLessThan(0);
      }
    },
  );

  it("write each record strided, with (0, +∞) for its error bytes, and with no survey mask an interior patch's, its blocks and strips opaque and no fin cluster drawn", () => {
    const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
    const key = { face: 0, level: 5, i: 3, j: 4 } as const;
    const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), 0);
    const started = performance.now();
    writeTerrainClusterRecords(
      planetGeometry(WGS84_FIGURE, null),
      { key, originM: { x, y, z }, heightRangeM: [-100, 400], skirtDepthM: 20 },
      records,
      0,
    );
    const tookMs = performance.now() - started;
    const at = (cluster: number, word: number): number =>
      records.getUint32(cluster * 32 + 4 * word, true);
    expect([at(0, 4), at(0, 5), at(0, 7), at(TERRAIN_BLOCKS, 7)]).toEqual([0, 0x7f80_0000, 0, 1]);
    // With no survey mask the patch is an interior one's: no fin cluster is drawn (R14.T1.g).
    const fins = Array.from(
      { length: TERRAIN_PAGE_CLUSTERS - TERRAIN_CLUSTERS_PER_PAGE },
      (_, n) => TERRAIN_CLUSTERS_PER_PAGE + n,
    );
    expect(fins.map((c) => [records.getFloat32(c * 32 + 12, true) < 0, at(c, 7)])).toEqual(
      fins.map((c) => [
        true,
        CLUSTER_SHAPE_CODE[c < TERRAIN_CLUSTERS_PER_PAGE + 8 ? "terrain-head" : "terrain-fin"] |
          CLUSTER_FLAG_MAY_DISCARD,
      ]),
    );
    // A slot's records are written at its upload, some ten a second: well under a frame's time.
    expect(tookMs).toBeLessThan(50);
  });

  it("write a bake's measured errors into each record, marked measured (R14.T3.b)", () => {
    const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
    const { source, clusterErrors } = measuredSource();
    writeTerrainClusterRecords(
      planetGeometry(WGS84_FIGURE, null),
      { ...source, clusterErrors },
      records,
      0,
    );
    const bits = new Uint16Array(clusterErrors.buffer);
    const recorded = (cluster: number): number[] =>
      [0, 1, 2, 3].map((n) => records.getUint16(cluster * 32 + 16 + 2 * n, true));
    const clusters = [0, 37, TERRAIN_BLOCKS, TERRAIN_CLUSTERS_PER_PAGE - 1];
    expect(
      clusters.map((c) => [
        recorded(c),
        records.getUint32(c * 32 + 28, true) & CLUSTER_FLAG_MEASURED,
      ]),
    ).toEqual(
      clusters.map((c) => [Array.from(bits.subarray(4 * c, 4 * c + 4)), CLUSTER_FLAG_MEASURED]),
    );
  });

  it("refuse a bake's measured errors short of four a cluster (R14.T3.b)", () => {
    const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
    const { source, clusterErrors } = measuredSource();
    expect(() => {
      writeTerrainClusterRecords(
        planetGeometry(WGS84_FIGURE, null),
        { ...source, clusterErrors: clusterErrors.subarray(4) },
        records,
        0,
      );
    }).toThrow(RangeError);
  });
});

/**
 * Terrain's fin clusters and its edge patches' records (plan R14, R14.T1.g): the fin run draws
 * exactly the lit mesh's skirt heads and survey fins, the may-discard templates the stride-1
 * blocks and strips, and each record's survey classification holds against the lit fragment's
 * survey test at every place its triangles can put a fragment.
 */

import { describe, expect, it } from "vitest";

import { worstOutsideClusters } from "../../test/clusterBounds";
import { seededRandom } from "../../test/seededRandom";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import { vertexDir } from "../terrain/cube";
import {
  GRID_INDICES,
  LIT_VERTEX_CODE,
  litPatchMeshData,
  packSurveyMask,
  SURVEY_FIN_LINES,
  surveyedAt,
} from "../terrain/gpu/resources";
import type { PatchKey } from "../terrain/patchKey";
import { planetGeometry, surfacePoint } from "../terrain/planet";
import { CLUSTER_FLAG_MAY_DISCARD, CLUSTER_RECORD_BYTES } from "./record";
import {
  type ClusterSurvey,
  FIN_LINES,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_PAGE_CLUSTERS,
  terrainClusterGrid,
  terrainClusterSurvey,
  terrainFinGrid,
  terrainFinPlace,
  terrainFinTemplateRef,
  writeTerrainClusterRecords,
} from "./terrain";
import {
  STRIDES,
  TERRAIN_DISCARD_TEMPLATES,
  terrainClusterMesh,
  terrainDiscardTemplateIndex,
} from "./templates";

const MESH = terrainClusterMesh();

/** The fin clusters, in order. */
const FINS = Array.from(
  { length: TERRAIN_PAGE_CLUSTERS - TERRAIN_CLUSTERS_PER_PAGE },
  (_, n) => TERRAIN_CLUSTERS_PER_PAGE + n,
);

/** A template vertex's (lx, ly, code). */
function local(v: number): readonly [number, number, number] {
  const p = MESH.mesh.positions;
  return [p[3 * v] ?? Number.NaN, p[3 * v + 1] ?? Number.NaN, p[3 * v + 2] ?? Number.NaN];
}

/** Template `t`'s triangles of `cluster`, each its three vertices' lit-mesh (x, y, code). */
function triangles(t: number, cluster: number): Array<ReadonlyArray<readonly number[]>> {
  const template = MESH.templates[t];
  const indices = MESH.mesh.indices ?? new Uint32Array();
  const out: Array<ReadonlyArray<readonly number[]>> = [];
  for (let n = 0; n < (template?.indexCount ?? 0); n += 3) {
    const at = (template?.firstIndex ?? 0) + n;
    out.push(
      [0, 1, 2].map((k) => {
        const vertex = local(indices[at + k] ?? 0);
        return cluster >= TERRAIN_CLUSTERS_PER_PAGE
          ? terrainFinGrid(cluster, vertex)
          : terrainClusterGrid(cluster, vertex);
      }),
    );
  }
  return out;
}

/** A triangle as text, its vertices in order. */
function text(triangle: ReadonlyArray<readonly number[]>): string {
  return triangle.map((v) => v.join(" ")).join(" | ");
}

/** Whether a triangle has two vertices at one place: no area wherever it is drawn. */
function degenerate(triangle: ReadonlyArray<readonly number[]>): boolean {
  const [a, b, c] = triangle.map((v) => v.join(" "));
  return a === b || b === c || a === c;
}

/** The template a cluster of the page draws by at stride 1 where it may discard. */
function discardTemplateOf(cluster: number): number {
  return terrainDiscardTemplateIndex(
    cluster < TERRAIN_BLOCKS
      ? "terrain-block"
      : cluster < TERRAIN_CLUSTERS_PER_PAGE
        ? "terrain-skirt"
        : terrainFinPlace(cluster).line === "head"
          ? "terrain-head"
          : "terrain-fin",
  );
}

/** `v` clamped half a quad inside the patch, as a skirt's survey reads it. */
function clamp(v: number): number {
  return Math.min(63.5, Math.max(0.5, v));
}

/**
 * The lit fragment's survey test at `f` (quads) in part `kind` (`terrainSurvey.wgsl`'s
 * `survey_kept_at`), by `surveyedAt`, its twin.
 */
function keptAt(packed: Uint8Array, f: readonly [number, number], kind: number): boolean {
  const [x, y] = f;
  switch (kind) {
    case LIT_VERTEX_CODE.skirtFoot:
      return surveyedAt(packed, clamp(x), clamp(y));
    case LIT_VERTEX_CODE.skirtHead:
      return !surveyedAt(packed, clamp(x), clamp(y));
    case LIT_VERTEX_CODE.columnFin: {
      const c = Math.round(x);
      return surveyedAt(packed, c - 0.5, y) !== surveyedAt(packed, c + 0.5, y);
    }
    case LIT_VERTEX_CODE.rowFin: {
      const c = Math.round(y);
      return surveyedAt(packed, x, c - 0.5) !== surveyedAt(packed, x, c + 0.5);
    }
    default:
      return surveyedAt(packed, x, y);
  }
}

/** Whether a fin's line `c` is an edge of the survey's cells, where its datum end stands at 0. */
function cellLine(packed: Uint8Array, c: number): boolean {
  const code = packed[65 * 65] ?? 0;
  return code >= 1 && code <= 5 && c % 2 ** code === 0;
}

/**
 * Whether any of `cluster`'s fragments, at stride 1, is kept and whether any is cut, over a lattice
 * of barycentric places in each of its triangles, each moved by `jitter` quads either way along x
 * and y (a fragment's interpolated place, rounded).
 */
function sampled(
  packed: Uint8Array,
  cluster: number,
  jitter: number,
): { readonly kept: boolean; readonly cut: boolean } {
  let kept = false;
  let cut = false;
  const t =
    cluster < TERRAIN_BLOCKS
      ? 0
      : cluster < TERRAIN_CLUSTERS_PER_PAGE
        ? STRIDES.length
        : discardTemplateOf(cluster);
  const steps = 4;
  for (const triangle of triangles(t, cluster)) {
    if (degenerate(triangle)) {
      continue;
    }
    const [a, b, c] = triangle;
    const kind = a?.[2] ?? 0;
    // A fin off the survey's cell edges stands on its grid vertices: it draws no fragment.
    const line = kind === LIT_VERTEX_CODE.columnFin ? (a?.[0] ?? 0) : (a?.[1] ?? 0);
    if (
      (kind === LIT_VERTEX_CODE.columnFin || kind === LIT_VERTEX_CODE.rowFin) &&
      !cellLine(packed, line)
    ) {
      continue;
    }
    for (let i = 0; i <= steps; i += 1) {
      for (let j = 0; j <= steps - i; j += 1) {
        const u = i / steps;
        const v = j / steps;
        const w = 1 - u - v;
        const at = (k: 0 | 1): number => u * (a?.[k] ?? 0) + v * (b?.[k] ?? 0) + w * (c?.[k] ?? 0);
        for (const [dx, dy] of [
          [0, 0],
          [jitter, 0],
          [-jitter, 0],
          [0, jitter],
          [0, -jitter],
        ] as const) {
          // A fin's fragments lie on its line: its interpolated place there rounds to it.
          const onLine = kind === LIT_VERTEX_CODE.columnFin || kind === LIT_VERTEX_CODE.rowFin;
          const f: [number, number] = [at(0) + (onLine ? 0 : dx), at(1) + (onLine ? 0 : dy)];
          if (keptAt(packed, f, kind)) {
            kept = true;
          } else {
            cut = true;
          }
        }
      }
    }
  }
  return { kept, cut };
}

/** A random survey mask at cell code `code`, the cells' states `share` surveyed. */
function randomMask(seed: number, code: number, share: number): Uint8Array {
  const next = seededRandom(seed);
  const quads = 2 ** Math.max(code, 0);
  const cells = Math.max(1, 64 / quads);
  const states = Array.from({ length: cells * cells }, () => next() < share);
  const mask = new Uint8Array(65 * 65);
  for (let y = 0; y <= 64; y += 1) {
    for (let x = 0; x <= 64; x += 1) {
      const cx = Math.min(cells - 1, Math.floor(x / quads));
      const cy = Math.min(cells - 1, Math.floor(y / quads));
      mask[65 * y + x] = (code === 0 ? next() < share : (states[cy * cells + cx] ?? false))
        ? 255
        : 0;
    }
  }
  return packSurveyMask(mask, code);
}

describe("terrain's fin clusters (plan R14, R14.T1.g)", () => {
  it("are the skirt heads in two runs an edge, then each fin line in two, columns then rows", () => {
    expect(SURVEY_FIN_LINES).toBe(FIN_LINES);
    expect(FINS.map((c) => terrainFinPlace(c))).toEqual([
      ...[0, 1, 2, 3].flatMap((edge) => [0, 1].map((half) => ({ line: "head", edge, half }))),
      ...(["column", "row"] as const).flatMap((line) =>
        FIN_LINES.flatMap((at) => [0, 1].map((half) => ({ line, at, half }))),
      ),
    ]);
    expect(() => terrainFinPlace(TERRAIN_PAGE_CLUSTERS)).toThrow(RangeError);
    expect(() => terrainFinPlace(TERRAIN_CLUSTERS_PER_PAGE - 1)).toThrow(RangeError);
  });

  it("draw by the head and fin runs exactly the lit mesh's skirt heads and fins, each in its own vertex order", () => {
    const lit = litPatchMeshData();
    const at = (v: number): string =>
      [0, 1, 2].map((k) => String(lit.positions[3 * v + k] ?? Number.NaN)).join(" ");
    const wanted = new Set<string>();
    for (let n = GRID_INDICES; n < lit.indices.length; n += 3) {
      const triangle = [0, 1, 2].map((k) => at(lit.indices[n + k] ?? 0)).join(" | ");
      const code = Number(triangle.split(" ")[2]);
      if (code !== LIT_VERTEX_CODE.skirtFoot) {
        wanted.add(triangle);
      }
    }
    const drawn = FINS.flatMap((c) => triangles(discardTemplateOf(c), c));
    const real = drawn.map(text);
    expect([real.length, new Set(real).size, drawn.filter(degenerate)]).toEqual([
      wanted.size,
      wanted.size,
      [],
    ]);
    expect(real.filter((triangle) => !wanted.has(triangle))).toEqual([]);
  });

  it("give the may-discard templates the stride-1 block's and strip's triangles, the fin run's 128 and the head run's 64", () => {
    expect(TERRAIN_DISCARD_TEMPLATES.map((t) => MESH.templates[t]?.shape)).toEqual([
      "terrain-block",
      "terrain-skirt",
      undefined,
      "terrain-fin",
      "terrain-head",
      undefined,
      undefined,
      undefined,
    ]);
    expect(
      (["terrain-fin", "terrain-head"] as const).map(
        (shape) => MESH.templates[terrainDiscardTemplateIndex(shape)]?.triangles,
      ),
    ).toEqual([128, 64]);
    const cluster = 27;
    expect(triangles(terrainDiscardTemplateIndex("terrain-block"), cluster).map(text)).toEqual(
      triangles(0, cluster).map(text),
    );
    const strip = TERRAIN_BLOCKS + 13;
    expect(triangles(terrainDiscardTemplateIndex("terrain-skirt"), strip).map(text)).toEqual(
      triangles(STRIDES.length, strip).map(text),
    );
  });

  it("name each fin cluster's first vertex, a head run's edge and a fin's axis in its template reference", () => {
    for (const cluster of FINS) {
      const ref = terrainFinTemplateRef(cluster);
      const [x, y] = terrainFinGrid(cluster, [0, 0, 0]);
      const place = terrainFinPlace(cluster);
      expect([ref & 0xff, (ref >> 8) & 0xff]).toEqual([x, y]);
      expect(ref >>> 16).toBe(
        place.line === "head" ? place.edge : place.line === "row" ? 0b1100 : 0b1000,
      );
    }
  });
});

describe("terrain's edge patches' records (plan R14, R14.T1.g)", () => {
  /** Every cluster's classification under `packed`, against its sampled fragments. */
  function misclassified(packed: Uint8Array): string[] {
    const wrong: string[] = [];
    for (let cluster = 0; cluster < TERRAIN_PAGE_CLUSTERS; cluster += 1) {
      const survey: ClusterSurvey = terrainClusterSurvey(packed, cluster);
      const { kept, cut } = sampled(packed, cluster, 1e-4);
      const fin = cluster >= TERRAIN_CLUSTERS_PER_PAGE;
      if ((survey === "whole" && (cut || fin)) || (survey === "none" && kept)) {
        wrong.push(`${String(cluster)} ${survey}: kept ${String(kept)}, cut ${String(cut)}`);
      }
    }
    return wrong;
  }

  it.each([0, 1, 2, 3, 4, 5, 6])(
    "keep no cluster whole that a fragment of is cut, and drop none that one is kept of, at cell code %i",
    (code) => {
      for (const [seed, share] of [
        [1, 0.5],
        [2, 0.2],
        [3, 0.8],
        [4, 0.97],
      ] as const) {
        expect(misclassified(randomMask(100 * code + seed, code, share))).toEqual([]);
      }
    },
  );

  it("classify an interior patch's mask as its blocks and strips whole and no fin cluster", () => {
    const packed = packSurveyMask(new Uint8Array(65 * 65).fill(255), 4);
    const surveys = Array.from({ length: TERRAIN_PAGE_CLUSTERS }, (_, c) =>
      terrainClusterSurvey(packed, c),
    );
    expect(surveys.slice(0, TERRAIN_CLUSTERS_PER_PAGE).every((s) => s === "whole")).toBe(true);
    expect(surveys.slice(TERRAIN_CLUSTERS_PER_PAGE).every((s) => s === "none")).toBe(true);
  });

  it("keep whole the blocks away from the survey's edge, cut those at it, and drop those beyond", () => {
    // Columns 0 to 31 surveyed, at cells of 16 quads: the edge on column 32.
    const mask = new Uint8Array(65 * 65);
    for (let y = 0; y <= 64; y += 1) {
      for (let x = 0; x <= 31; x += 1) {
        mask[65 * y + x] = 255;
      }
    }
    const packed = packSurveyMask(mask, 4);
    const row = [0, 1, 2, 3, 4, 5, 6, 7].map((bx) => terrainClusterSurvey(packed, 8 * 3 + bx));
    expect(row).toEqual(["whole", "whole", "whole", "edge", "edge", "none", "none", "none"]);
    // The fins on column 32 draw the edge; those on 16 and 48, and every row's, lie between cells
    // of one state.
    const fins = FINS.filter((c) => terrainClusterSurvey(packed, c) === "edge").map((c) =>
      terrainFinPlace(c),
    );
    expect(fins.filter((p) => p.line !== "head")).toEqual([
      { line: "column", at: 32, half: 0 },
      { line: "column", at: 32, half: 1 },
    ]);
  });

  it("mark the records it cuts may-discard, and those it drops no cluster", () => {
    const packed = randomMask(7, 3, 0.5);
    const key: PatchKey = { face: 2, level: 8, i: 100, j: 77 };
    const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), 0);
    const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
    writeTerrainClusterRecords(
      planetGeometry(WGS84_FIGURE, null),
      {
        key,
        originM: { x, y, z },
        heightRangeM: [-300, 900],
        skirtDepthM: 10,
        surveyMask: packed,
      },
      records,
      0,
    );
    for (let cluster = 0; cluster < TERRAIN_PAGE_CLUSTERS; cluster += 1) {
      const survey = terrainClusterSurvey(packed, cluster);
      const radius = records.getFloat32(cluster * CLUSTER_RECORD_BYTES + 12, true);
      const flags = records.getUint32(cluster * CLUSTER_RECORD_BYTES + 28, true);
      expect([radius < 0, (flags & CLUSTER_FLAG_MAY_DISCARD) !== 0]).toEqual([
        survey === "none",
        survey === "edge" || cluster >= TERRAIN_CLUSTERS_PER_PAGE,
      ]);
    }
  });

  it.each([
    ["an Earth's level 3", { face: 4, level: 3, i: 5, j: 2 }],
    ["an Earth's level 12", { face: 0, level: 12, i: 1_000, j: 3_001 }],
    ["an Earth's finest level", { face: 5, level: 19, i: 300_001, j: 7 }],
  ] as const)(
    "hold every vertex of %s's fin clusters, heads and fins at their places, inside their spheres",
    (_name, key) => {
      for (const seed of [1, 2, 3]) {
        const next = seededRandom(seed);
        const span = 50 + 9_000 * next();
        const low = -span * next();
        const own = Float64Array.from({ length: 65 * 65 }, () => Math.fround(low + span * next()));
        const morph = Float64Array.from({ length: 65 * 65 }, () =>
          Math.fround(low + span * next()),
        );
        const h0M = own[32 * 65 + 32] ?? 0;
        const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), h0M);
        for (const code of [1, 4]) {
          const surveyMask = randomMask(seed + 10 * code, code, 0.5);
          expect(
            worstOutsideClusters({
              figure: WGS84_FIGURE,
              key,
              originM: { x, y, z },
              originHeightM: h0M,
              own,
              morph,
              heightRangeM: [low, low + span],
              skirtDepthM: 1 + 500 * next(),
              surveyMask,
            }),
          ).toBeLessThan(0);
        }
      }
    },
  );
});

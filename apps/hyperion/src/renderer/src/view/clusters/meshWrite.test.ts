import { describe, expect, it } from "vitest";

import { type Vec3, vec3 } from "../../geometry/vec3";
import {
  encodeOctahedral,
  MAX_CLUSTER_TRIANGLES,
  meshGrid,
  type ProceduralCluster,
  writeProceduralMesh,
} from "./meshWrite";
import {
  MAX_PAGE_BYTES,
  MAX_PAGE_CLUSTERS,
  type MeshIndex,
  type MeshPage,
  NO_GROUP,
  pageClusterCorners,
  readMeshIndex,
  readMeshPage,
} from "./pageFormat";
import { GeometryPagePool, pagePoolSlots } from "./pages";
import { decodeOctahedral } from "../terrain/shadingNormal";
import { MirrorEngine } from "../../test/mirrorEngine";

/** A strip of `quads` unit quads along x at height `z`, two triangles a quad, its vertices' word `word`. */
function strip(quads: number, z: number, word: number): ProceduralCluster {
  const vertices = 2 * (quads + 1);
  const positionsM = new Float64Array(3 * vertices);
  const normals = new Float64Array(3 * vertices);
  for (let k = 0; k <= quads; k += 1) {
    positionsM.set([k * 0.37 + 100, 0.5, z, k * 0.37 + 100, 1.25, z + 0.01 * k], 6 * k);
    normals.set([0, 0.6, 0.8, 0.6, 0, -0.8], 6 * k);
  }
  const triangles = new Uint8Array(6 * quads);
  for (let k = 0; k < quads; k += 1) {
    triangles.set([2 * k, 2 * k + 2, 2 * k + 3, 2 * k, 2 * k + 3, 2 * k + 1], 6 * k);
  }
  return {
    positionsM,
    normals,
    words: new Uint32Array(vertices).fill(word),
    triangles,
    sphere: { centreM: vec3(100 + (0.37 * quads) / 2, 0.875, z), radiusM: 0.37 * quads + 1 },
    errorsM: [0, 2.5],
    material: 3,
  };
}

/** The index and pages of `file`, read as the producer reads them. */
function readAll(file: ArrayBuffer): { readonly index: MeshIndex; readonly pages: MeshPage[] } {
  const index = readMeshIndex(file);
  if (index.kind !== "read") {
    throw new Error(index.reason);
  }
  const pages = index.value.pages.map((entry, p) => {
    const page = readMeshPage(
      file.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes),
      entry,
      p,
    );
    if (page.kind !== "read") {
      throw new Error(page.reason);
    }
    return page.value;
  });
  return { index: index.value, pages };
}

/** `c` rounded as a snorm16 is. */
function snormRound(c: number): number {
  return Math.round(c * 32_767) / 32_767;
}

describe("the procedural builder's page writer (R14.T5.c)", () => {
  const clusters = Array.from({ length: 300 }, (_, c) => strip(1 + (c % 64), c * 0.01, c));

  it("writes a one-level DAG that the format reads back, every page a root page within its limits", () => {
    const { index, pages } = readAll(writeProceduralMesh(clusters));
    expect(index.clusters).toBe(300);
    expect(index.levels).toBe(1);
    expect(index.groups).toEqual([]);
    expect(index.rootPages).toBe(pages.length);
    expect(pages.length).toBeGreaterThan(1);
    expect(pages.reduce((sum, page) => sum + page.clusters, 0)).toBe(300);
    for (const page of pages) {
      expect(page.clusters).toBeLessThanOrEqual(MAX_PAGE_CLUSTERS);
      expect(page.body.byteLength).toBeLessThanOrEqual(MAX_PAGE_BYTES);
      expect([...page.generatingGroups, ...page.consumingGroups].every((g) => g === NO_GROUP)).toBe(
        true,
      );
    }
    expect(index.pages.every((entry) => entry.parentErrorSpanM[0] === 2.5)).toBe(true);
  });

  it("puts every vertex on the mesh's grid within half a step of where it was given", () => {
    const grid = meshGrid(clusters);
    const { pages } = readAll(writeProceduralMesh(clusters));
    let c = 0;
    for (const page of pages) {
      expect(page.stepM).toBe(grid.stepM);
      expect(page.originM).toEqual(grid.originM);
      for (let k = 0; k < page.clusters; k += 1) {
        const cluster = clusters[c];
        if (cluster === undefined) {
          throw new Error("every cluster is paged");
        }
        const corners = pageClusterCorners(page, k);
        for (let t = 0; t < cluster.triangles.length; t += 1) {
          const vertex = cluster.triangles[t] ?? 0;
          for (let axis = 0; axis < 3; axis += 1) {
            const given = cluster.positionsM[3 * vertex + axis] ?? 0;
            const origin = [grid.originM.x, grid.originM.y, grid.originM.z][axis] ?? 0;
            const at = (corners[3 * t + axis] ?? 0) * grid.stepM + origin;
            expect(Math.abs(at - given)).toBeLessThanOrEqual(grid.stepM / 2);
          }
        }
        c += 1;
      }
    }
  });

  it("packs each normal as WGSL's pack2x16snorm packs its octahedral pair", () => {
    for (const n of [vec3(0, 0.6, 0.8), vec3(0.6, 0, -0.8), vec3(-0.48, 0.6, -0.64)]) {
      const [x, y] = encodeOctahedral(n.x, n.y, n.z);
      const back: Vec3 = decodeOctahedral(snormRound(x), snormRound(y));
      expect(Math.hypot(back.x - n.x, back.y - n.y, back.z - n.z)).toBeLessThan(1e-4);
    }
  });

  it("refuses an empty mesh and a cluster past a page's limits", () => {
    expect(() => writeProceduralMesh([])).toThrow(RangeError);
    expect(() => writeProceduralMesh([strip(MAX_CLUSTER_TRIANGLES / 2 + 1, 0, 0)])).toThrow(
      RangeError,
    );
  });

  it("is stored whole by a pool of root slots alone, each page in its run of them", () => {
    const { index, pages } = readAll(writeProceduralMesh(clusters));
    const pool = new GeometryPagePool(new MirrorEngine(), pagePoolSlots(64, 0));
    const mesh = pool.addMesh(index);
    for (const page of pages) {
      expect(pool.insert(mesh, page).kind).toBe("stored");
    }
    expect(pool.residentPages(mesh).map(([page]) => page)).toEqual(pages.map((page) => page.index));
    expect(pages.every((page) => pool.canDraw(mesh, page.index))).toBe(true);
  });
});

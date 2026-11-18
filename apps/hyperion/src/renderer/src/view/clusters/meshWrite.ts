/**
 * The procedural builder's writer of the mesh page format, version 2 (plan R14, R14.T5.c): a mesh
 * made on the client, R11's rocks first, written as `tools/meshbuild/src/page.rs` writes an authored
 * one, so that `pageFormat.ts` reads it and the page pool and the cluster path draw it as they do
 * every mesh.
 *
 * @remarks
 * - **A one-level DAG**: every cluster is a root, of no group, its own error and its parent's its
 *   own (in the DAG-cut mode, own ≤ tolerance < parent at the group's sphere; Karis 2021), so that
 *   the cut draws a cluster wholly or not at all and no coarser cluster stands in. A consumer sets
 *   the parent error to the size below which its unit need not be drawn (R11's rocks: a rock's
 *   largest diameter, so that it is drawn out to where it subtends the camera's tolerance), and a
 *   cluster's sphere to its unit's (a rock's), so that every cluster of a unit is kept or refused
 *   together. Every page is a root page, so the mesh's pages are pinned whole while it is in the
 *   pool.
 * - **One grid**: 21 bits an axis about the mesh's least corner, the step the least power of two at
 *   which the largest extent spans the grid (`page.rs`' own rule), so that the vertex is the grid
 *   coordinates times the step about the page's origin, exactly in `f32`, every page with the same
 *   origin and step. Each vertex holds its unit normal as WGSL's `pack2x16snorm` packs its
 *   octahedral pair, and a fourth word: the UV's where the mesh has UVs, which a procedural mesh
 *   never has (flags bit 0 clear), so the word is its consumer's (the rocks': a vertex's rock).
 * - **Pages** take clusters in order while they hold at most {@link MAX_PAGE_CLUSTERS} and at most
 *   {@link MAX_PAGE_BYTES}, every section at a multiple of 16 B.
 * - Every byte is a function of the clusters given, in `f64` and their order, so that the same
 *   clusters give the same bytes in a worker and on the render thread. It imports nothing that
 *   needs the engine (`tsconfig.worker.json`).
 */

import type { Vec3 } from "../../geometry/vec3";
import {
  CLUSTER_TABLE_BYTES,
  MAX_PAGE_BYTES,
  MAX_PAGE_CLUSTERS,
  MESH_FORMAT_VERSION,
  MESH_HEADER_BYTES,
  MESH_MAGIC,
  NO_GROUP,
  PAGE_ENTRY_BYTES,
  PAGE_HEADER_BYTES,
  PAGE_LOD_BYTES,
  PAGE_MAGIC,
  PAGE_RECORD_BYTES,
  PAGE_TRIANGLE_BYTES,
  PAGE_VERTEX_BYTES,
  type PageSphere,
} from "./pageFormat";

/** The bits of one grid coordinate (`page.rs`). */
const GRID_BITS = 21;

/** The greatest grid coordinate. */
const GRID_MAX = 2 ** GRID_BITS - 1;

/** The most triangles a cluster holds. */
export const MAX_CLUSTER_TRIANGLES = 128;

/** The most vertices a cluster holds: its triangles' 8-bit indices. */
export const MAX_CLUSTER_VERTICES = 256;

/** The mesh shape's code with the DAG-cut flag (`record.ts`), material 0. */
const MESH_CLUSTER_FLAGS = 2 | (1 << 4);

/** One cluster of a procedural mesh (see the module's remarks). */
export interface ProceduralCluster {
  /** Its vertices about the mesh's frame's origin, m, three a vertex. */
  readonly positionsM: Float64Array;
  /** Each vertex's outward unit normal, three a vertex. */
  readonly normals: Float64Array;
  /** Each vertex's fourth word, its consumer's (see the module's remarks). */
  readonly words: Uint32Array;
  /** Three of its own vertices' indices a triangle, counter-clockwise from outside. */
  readonly triangles: Uint8Array;
  /** Its sphere, its unit's, about the mesh's frame's origin: it must hold every vertex. */
  readonly sphere: PageSphere;
  /** Its own error and its parent's, m: (0, the size below which its unit need not be drawn). */
  readonly errorsM: readonly [number, number];
  /** Its consumer's material, 0 to 255 (the record's flags, bits 8 to 15). */
  readonly material: number;
}

/** The grid a mesh's vertices are written on: its origin and its step, m. */
export interface MeshGrid {
  readonly originM: Vec3;
  readonly stepM: number;
}

/**
 * The grid of `clusters`' vertices (see the module's remarks): about their least corner, at the
 * least power of two at which their largest extent spans {@link GRID_MAX} steps.
 */
export function meshGrid(clusters: ReadonlyArray<ProceduralCluster>): MeshGrid {
  const low = [Infinity, Infinity, Infinity];
  const high = [-Infinity, -Infinity, -Infinity];
  for (const cluster of clusters) {
    const p = cluster.positionsM;
    for (let v = 0; v < p.length; v += 3) {
      for (let k = 0; k < 3; k += 1) {
        const x = p[v + k] ?? 0;
        low[k] = Math.min(low[k] ?? x, x);
        high[k] = Math.max(high[k] ?? x, x);
      }
    }
  }
  const [x = 0, y = 0, z = 0] = low.map((l) => (Number.isFinite(l) ? l : 0));
  const extent = Math.max(0, ...high.map((h, k) => (Number.isFinite(h) ? h - (low[k] ?? 0) : 0)));
  let stepM = 2 ** -30;
  while (extent / stepM > GRID_MAX) {
    stepM *= 2;
  }
  return { originM: { x, y, z }, stepM };
}

/** A grid coordinate of `x` along an axis whose origin is `origin`. */
function gridOf(x: number, origin: number, stepM: number): number {
  return Math.min(GRID_MAX, Math.max(0, Math.round((x - origin) / stepM)));
}

/** A snorm16 of `c`, clamped to [−1, 1], as WGSL's `pack2x16snorm` rounds it. */
function snorm16(c: number): number {
  return Math.round(Math.min(1, Math.max(-1, c)) * 32_767) & 0xffff;
}

/** WGSL's `pack2x16snorm` of (`a`, `b`). */
function pack2x16snorm(a: number, b: number): number {
  return (snorm16(a) | (snorm16(b) << 16)) >>> 0;
}

/** The sign of `x`, 1 at zero (the octahedral map's `sign_not_zero`). */
function signNotZero(x: number): number {
  return x < 0 ? -1 : 1;
}

/** The octahedral pair of the unit vector (`x`, `y`, `z`) (`patch::normals::encode_octahedral`). */
export function encodeOctahedral(x: number, y: number, z: number): readonly [number, number] {
  const l1 = Math.abs(x) + Math.abs(y) + Math.abs(z);
  const [u, v] = [x / l1, y / l1];
  return z >= 0 ? [u, v] : [(1 - Math.abs(v)) * signNotZero(u), (1 - Math.abs(u)) * signNotZero(v)];
}

/** `n` rounded up to a multiple of 16. */
function align16(n: number): number {
  return Math.ceil(n / 16) * 16;
}

/** The bytes a page of `clusters` takes. */
function pageBytesOf(clusters: ReadonlyArray<ProceduralCluster>): number {
  const n = clusters.length;
  const vertices = clusters.reduce((sum, c) => sum + c.positionsM.length / 3, 0);
  const triangles = clusters.reduce((sum, c) => sum + c.triangles.length / 3, 0);
  return align16(
    PAGE_HEADER_BYTES +
      n * (PAGE_RECORD_BYTES + PAGE_LOD_BYTES + CLUSTER_TABLE_BYTES) +
      vertices * PAGE_VERTEX_BYTES +
      triangles * PAGE_TRIANGLE_BYTES,
  );
}

/**
 * The pages `clusters` are laid in, in order (see the module's remarks): each a run of them.
 *
 * @throws RangeError for a cluster of more than {@link MAX_CLUSTER_TRIANGLES} triangles or
 *   {@link MAX_CLUSTER_VERTICES} vertices, or one that no page could hold.
 */
export function pageRuns(clusters: ReadonlyArray<ProceduralCluster>): Array<[number, number]> {
  const runs: Array<[number, number]> = [];
  const fixed = PAGE_RECORD_BYTES + PAGE_LOD_BYTES + CLUSTER_TABLE_BYTES;
  let first = 0;
  // The run's bytes past the header, before the page's last alignment.
  let body = 0;
  clusters.forEach((cluster, c) => {
    const triangles = cluster.triangles.length / 3;
    const vertices = cluster.positionsM.length / 3;
    const own = fixed + vertices * PAGE_VERTEX_BYTES + triangles * PAGE_TRIANGLE_BYTES;
    if (
      triangles < 1 ||
      triangles > MAX_CLUSTER_TRIANGLES ||
      !Number.isInteger(triangles) ||
      vertices > MAX_CLUSTER_VERTICES ||
      align16(PAGE_HEADER_BYTES + own) > MAX_PAGE_BYTES
    ) {
      throw new RangeError(
        `cluster ${String(c)} of ${String(triangles)} triangles over ${String(vertices)} vertices is no page's`,
      );
    }
    if (
      c - first + 1 > MAX_PAGE_CLUSTERS ||
      align16(PAGE_HEADER_BYTES + body + own) > MAX_PAGE_BYTES
    ) {
      runs.push([first, c]);
      first = c;
      body = 0;
    }
    body += own;
  });
  if (first < clusters.length) {
    runs.push([first, clusters.length]);
  }
  return runs;
}

/** One page's bytes of `clusters` on `grid`, as `page.rs`' `write_page` lays them out. */
function writePage(
  clusters: ReadonlyArray<ProceduralCluster>,
  index: number,
  grid: MeshGrid,
): ArrayBuffer {
  const n = clusters.length;
  const vertexCount = clusters.reduce((sum, c) => sum + c.positionsM.length / 3, 0);
  const triangleCount = clusters.reduce((sum, c) => sum + c.triangles.length / 3, 0);
  const records = PAGE_HEADER_BYTES;
  const lods = records + PAGE_RECORD_BYTES * n;
  const table = lods + PAGE_LOD_BYTES * n;
  const vertexAt = table + CLUSTER_TABLE_BYTES * n;
  const triangleAt = vertexAt + PAGE_VERTEX_BYTES * vertexCount;
  const total = pageBytesOf(clusters);
  const out = new DataView(new ArrayBuffer(total));
  [PAGE_MAGIC, MESH_FORMAT_VERSION, index, n, vertexCount, triangleCount, total, 0].forEach(
    (v, k) => {
      out.setUint32(4 * k, v, true);
    },
  );
  const { originM, stepM } = grid;
  out.setFloat64(32, originM.x, true);
  out.setFloat64(40, originM.y, true);
  out.setFloat64(48, originM.z, true);
  out.setFloat32(56, stepM, true);
  out.setFloat32(60, 1, true);
  [records, lods, table, vertexAt, triangleAt, 0].forEach((v, k) => {
    out.setUint32(72 + 4 * k, v, true);
  });
  // A sphere less the page's origin, widened past its vertices' rounding onto the grid and to f32.
  const slack = stepM;
  const sphere = (at: number, s: PageSphere): void => {
    out.setFloat32(at, s.centreM.x - originM.x, true);
    out.setFloat32(at + 4, s.centreM.y - originM.y, true);
    out.setFloat32(at + 8, s.centreM.z - originM.z, true);
    out.setFloat32(at + 12, s.radiusM * (1 + 1e-6) + slack, true);
  };
  let firstTriangle = 0;
  let firstVertex = 0;
  clusters.forEach((cluster, c) => {
    const vertices = cluster.positionsM.length / 3;
    const triangles = cluster.triangles.length / 3;
    const r = records + PAGE_RECORD_BYTES * c;
    sphere(r, cluster.sphere);
    out.setFloat32(r + 16, cluster.errorsM[0], true);
    out.setFloat32(r + 20, cluster.errorsM[1], true);
    out.setUint32(r + 24, (firstTriangle | (triangles << 24)) >>> 0, true);
    out.setUint32(r + 28, (MESH_CLUSTER_FLAGS | ((cluster.material & 0xff) << 8)) >>> 0, true);
    // A root's parent group's sphere is its own.
    sphere(lods + PAGE_LOD_BYTES * c, cluster.sphere);
    const e = table + CLUSTER_TABLE_BYTES * c;
    out.setUint32(e, firstVertex, true);
    out.setUint32(e + 4, vertices, true);
    out.setUint32(e + 8, NO_GROUP, true);
    out.setUint32(e + 12, NO_GROUP, true);
    for (let v = 0; v < vertices; v += 1) {
      const at = vertexAt + PAGE_VERTEX_BYTES * (firstVertex + v);
      const p = cluster.positionsM;
      const x = gridOf(p[3 * v] ?? 0, originM.x, stepM);
      const y = gridOf(p[3 * v + 1] ?? 0, originM.y, stepM);
      const z = gridOf(p[3 * v + 2] ?? 0, originM.z, stepM);
      out.setUint32(at, (x | (y << GRID_BITS)) >>> 0, true);
      out.setUint32(at + 4, ((y >>> (32 - GRID_BITS)) | (z << (2 * GRID_BITS - 32))) >>> 0, true);
      const nv = cluster.normals;
      const [ox, oy] = encodeOctahedral(nv[3 * v] ?? 0, nv[3 * v + 1] ?? 0, nv[3 * v + 2] ?? 1);
      out.setUint32(at + 8, pack2x16snorm(ox, oy), true);
      out.setUint32(at + 12, cluster.words[v] ?? 0, true);
    }
    for (let t = 0; t < triangles; t += 1) {
      const tri = cluster.triangles;
      out.setUint32(
        triangleAt + PAGE_TRIANGLE_BYTES * (firstTriangle + t),
        ((tri[3 * t] ?? 0) | ((tri[3 * t + 1] ?? 0) << 8) | ((tri[3 * t + 2] ?? 0) << 16)) >>> 0,
        true,
      );
    }
    firstTriangle += triangles;
    firstVertex += vertices;
  });
  return out.buffer;
}

/** A sphere holding `spheres`, about their centres' mean. */
function enclosing(spheres: ReadonlyArray<PageSphere>): PageSphere {
  const n = Math.max(1, spheres.length);
  const centreM = {
    x: spheres.reduce((s, b) => s + b.centreM.x, 0) / n,
    y: spheres.reduce((s, b) => s + b.centreM.y, 0) / n,
    z: spheres.reduce((s, b) => s + b.centreM.z, 0) / n,
  };
  const radiusM = Math.max(
    0,
    ...spheres.map(
      (b) =>
        Math.hypot(b.centreM.x - centreM.x, b.centreM.y - centreM.y, b.centreM.z - centreM.z) +
        b.radiusM,
    ),
  );
  return { centreM, radiusM: radiusM * (1 + 1e-6) };
}

/**
 * The mesh file of `clusters`, a one-level DAG (see the module's remarks): header, page table, no
 * group table, then the pages, every one a root page.
 *
 * @throws RangeError where `clusters` is empty, or for a cluster no page could hold
 *   ({@link pageRuns}).
 */
export function writeProceduralMesh(clusters: ReadonlyArray<ProceduralCluster>): ArrayBuffer {
  if (clusters.length === 0) {
    throw new RangeError("a mesh holds at least one cluster");
  }
  const grid = meshGrid(clusters);
  const runs = pageRuns(clusters);
  const bodies = runs.map(([from, to], p) => writePage(clusters.slice(from, to), p, grid));
  const tableEnd = align16(MESH_HEADER_BYTES + PAGE_ENTRY_BYTES * runs.length);
  const offsets: number[] = [];
  let end = tableEnd;
  for (const body of bodies) {
    end = align16(end);
    offsets.push(end);
    end += body.byteLength;
  }
  const bytes = new Uint8Array(end);
  const out = new DataView(bytes.buffer);
  [MESH_MAGIC, MESH_FORMAT_VERSION, runs.length, clusters.length, 0, 1, runs.length, 0].forEach(
    (v, k) => {
      out.setUint32(4 * k, v, true);
    },
  );
  runs.forEach(([from, to], p) => {
    const at = MESH_HEADER_BYTES + PAGE_ENTRY_BYTES * p;
    const members = clusters.slice(from, to);
    const own = members.map((c) => c.errorsM[0]);
    const parent = members.map((c) => c.errorsM[1]);
    out.setUint32(at, offsets[p] ?? 0, true);
    out.setUint32(at + 4, bodies[p]?.byteLength ?? 0, true);
    out.setUint32(at + 8, members.length, true);
    out.setUint32(at + 12, 0, true);
    [Math.min(...own), Math.max(...own), Math.min(...parent), Math.max(...parent)].forEach(
      (v, k) => {
        out.setFloat32(at + 16 + 4 * k, v, true);
      },
    );
    const bounds = enclosing(members.map((c) => c.sphere));
    out.setFloat32(at + 32, bounds.centreM.x - grid.originM.x, true);
    out.setFloat32(at + 36, bounds.centreM.y - grid.originM.y, true);
    out.setFloat32(at + 40, bounds.centreM.z - grid.originM.z, true);
    out.setFloat32(at + 44, bounds.radiusM + grid.stepM, true);
  });
  bodies.forEach((body, p) => {
    bytes.set(new Uint8Array(body), offsets[p] ?? 0);
  });
  return bytes.buffer;
}

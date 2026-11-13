/**
 * Mesh files for the page pool's and residency's tests (plan R14, R14.T5.b): a writer of the mesh
 * page format, version 2, from a described DAG (`tools/meshbuild/src/page.rs`' layout, which
 * `clusters/pageFormat.ts` reads), and a three-level DAG small enough to check at every residency
 * state.
 *
 * @remarks
 * The three-level DAG is a strip 16 m by 1 m, a quad a cluster piece, its grid's step 1 m about the
 * origin: eight leaves of 2 m (L0 to L7); groups G0 = {L0–L3} and G1 = {L4–L6} at level 0, which
 * make M0, M1 and M2, M3; and, at level 1, G2 = {M1, M2}, which makes the root R0, and G3 =
 * {M0, M3, L7}, which makes the root R1 over two pieces, so that L7 is a leaf left over into a
 * level-1 group and M1 and M2, made by different groups, are simplified together: a DAG, not a
 * tree. Its errors are monotone (G0 1 m, G1 1.5 m, G2 4 m, G3 6 m) and its spheres nested. Its
 * pages, coarsest first: the roots (page 0), G0's and G1's outputs (1, 2), and the leaves of G0, G1
 * and G3 (3, 4, 5).
 */

import type { Vec3 } from "../geometry/vec3";
import {
  CLUSTER_TABLE_BYTES,
  GROUP_ENTRY_BYTES,
  MESH_FORMAT_VERSION,
  MESH_HEADER_BYTES,
  MESH_MAGIC,
  NO_GROUP,
  PAGE_ENTRY_BYTES,
  PAGE_HEADER_BYTES,
  PAGE_LIST_ENTRY_BYTES,
  PAGE_LOD_BYTES,
  PAGE_MAGIC,
  PAGE_RECORD_BYTES,
  PAGE_TRIANGLE_BYTES,
  PAGE_VERTEX_BYTES,
  type PageSphere,
} from "../view/clusters/pageFormat";

/** One cluster of a described DAG. */
export interface SyntheticCluster {
  readonly level: number;
  readonly generatingGroup: number | null;
  readonly consumingGroup: number | null;
  readonly ownErrorM: number;
  /** +∞ at a root. */
  readonly parentErrorM: number;
  readonly ownSphere: PageSphere;
  /** Its consuming group's sphere, a root's own. */
  readonly parentSphere: PageSphere;
  /** Its triangles' corners as grid coordinates, nine a triangle. */
  readonly corners: ReadonlyArray<number>;
}

/** A described DAG and how its clusters are paged. */
export interface SyntheticMesh {
  readonly clusters: ReadonlyArray<SyntheticCluster>;
  readonly groups: number;
  /** Each page's clusters in order; the first {@link SyntheticMesh.rootPages} hold every root. */
  readonly pages: ReadonlyArray<ReadonlyArray<number>>;
  readonly rootPages: number;
  readonly originM: Vec3;
  readonly stepM: number;
}

/** The mesh shape's code with the DAG-cut flag (`record.ts`). */
const MESH_CLUSTER_FLAGS = 2 | (1 << 4);

function align16(n: number): number {
  return Math.ceil(n / 16) * 16;
}

/** One page's bytes, as `page.rs`' `write_page` lays them out. */
function pageBytes(mesh: SyntheticMesh, index: number): ArrayBuffer {
  const members = (mesh.pages[index] ?? []).map((c) => {
    const cluster = mesh.clusters[c];
    if (cluster === undefined) {
      throw new Error(`page ${String(index)} names no cluster ${String(c)}`);
    }
    const keys: string[] = [];
    const vertices: number[][] = [];
    const triangles: number[][] = [];
    for (let t = 0; t < cluster.corners.length; t += 9) {
      const triangle: number[] = [];
      for (let k = 0; k < 3; k += 1) {
        const corner = cluster.corners.slice(t + 3 * k, t + 3 * k + 3);
        const key = corner.join(",");
        let at = keys.indexOf(key);
        if (at < 0) {
          at = keys.length;
          keys.push(key);
          vertices.push(corner);
        }
        triangle.push(at);
      }
      triangles.push(triangle);
    }
    return { cluster, vertices, triangles };
  });
  const n = members.length;
  const vertexCount = members.reduce((sum, m) => sum + m.vertices.length, 0);
  const triangleCount = members.reduce((sum, m) => sum + m.triangles.length, 0);
  const records = PAGE_HEADER_BYTES;
  const lods = records + PAGE_RECORD_BYTES * n;
  const table = lods + PAGE_LOD_BYTES * n;
  const vertexAt = table + CLUSTER_TABLE_BYTES * n;
  const triangleAt = vertexAt + PAGE_VERTEX_BYTES * vertexCount;
  const total = align16(triangleAt + PAGE_TRIANGLE_BYTES * triangleCount);
  const out = new DataView(new ArrayBuffer(total));
  [PAGE_MAGIC, MESH_FORMAT_VERSION, index, n, vertexCount, triangleCount, total, 0].forEach(
    (v, k) => {
      out.setUint32(4 * k, v, true);
    },
  );
  out.setFloat64(32, mesh.originM.x, true);
  out.setFloat64(40, mesh.originM.y, true);
  out.setFloat64(48, mesh.originM.z, true);
  out.setFloat32(56, mesh.stepM, true);
  out.setFloat32(60, 1, true);
  [records, lods, table, vertexAt, triangleAt, 0].forEach((v, k) => {
    out.setUint32(72 + 4 * k, v, true);
  });
  const sphere = (at: number, s: PageSphere): void => {
    out.setFloat32(at, s.centreM.x, true);
    out.setFloat32(at + 4, s.centreM.y, true);
    out.setFloat32(at + 8, s.centreM.z, true);
    out.setFloat32(at + 12, s.radiusM, true);
  };
  let firstTriangle = 0;
  let firstVertex = 0;
  members.forEach(({ cluster, vertices, triangles }, c) => {
    const r = records + PAGE_RECORD_BYTES * c;
    sphere(r, cluster.ownSphere);
    out.setFloat32(r + 16, cluster.ownErrorM, true);
    out.setFloat32(r + 20, cluster.parentErrorM, true);
    out.setUint32(r + 24, (firstTriangle | (triangles.length << 24)) >>> 0, true);
    out.setUint32(r + 28, MESH_CLUSTER_FLAGS, true);
    sphere(lods + PAGE_LOD_BYTES * c, cluster.parentSphere);
    const e = table + CLUSTER_TABLE_BYTES * c;
    out.setUint32(e, firstVertex, true);
    out.setUint32(e + 4, vertices.length, true);
    out.setUint32(e + 8, cluster.generatingGroup ?? NO_GROUP, true);
    out.setUint32(e + 12, cluster.consumingGroup ?? NO_GROUP, true);
    vertices.forEach(([x = 0, y = 0, z = 0], v) => {
      const at = vertexAt + PAGE_VERTEX_BYTES * (firstVertex + v);
      out.setUint32(at, (x | (y << 21)) >>> 0, true);
      out.setUint32(at + 4, ((y >>> 11) | (z << 10)) >>> 0, true);
    });
    triangles.forEach(([a = 0, b = 0, d = 0], t) => {
      out.setUint32(
        triangleAt + PAGE_TRIANGLE_BYTES * (firstTriangle + t),
        a | (b << 8) | (d << 16),
        true,
      );
    });
    firstTriangle += triangles.length;
    firstVertex += vertices.length;
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

/** `mesh`'s file, as `page.rs`' `write_mesh` lays it out (see the module's remarks). */
export function writeMeshFile(mesh: SyntheticMesh): ArrayBuffer {
  const bodies = mesh.pages.map((_, p) => pageBytes(mesh, p));
  const pageOf = new Map<number, number>();
  mesh.pages.forEach((clusters, p) => {
    for (const c of clusters) {
      pageOf.set(c, p);
    }
  });
  const runs: number[][] = [];
  for (let g = 0; g < mesh.groups; g += 1) {
    for (const pick of ["consumingGroup", "generatingGroup"] as const) {
      const held = new Set<number>();
      mesh.clusters.forEach((cluster, c) => {
        if (cluster[pick] === g) {
          held.add(pageOf.get(c) ?? 0);
        }
      });
      runs.push([...held].toSorted((a, b) => a - b));
    }
  }
  const listEntries = runs.reduce((sum, run) => sum + run.length, 0);
  const tableEnd = align16(
    MESH_HEADER_BYTES +
      PAGE_ENTRY_BYTES * mesh.pages.length +
      GROUP_ENTRY_BYTES * mesh.groups +
      PAGE_LIST_ENTRY_BYTES * listEntries,
  );
  const offsets: number[] = [];
  let end = tableEnd;
  for (const body of bodies) {
    end = align16(end);
    offsets.push(end);
    end += body.byteLength;
  }
  const bytes = new Uint8Array(end);
  const out = new DataView(bytes.buffer);
  [
    MESH_MAGIC,
    MESH_FORMAT_VERSION,
    mesh.pages.length,
    mesh.clusters.length,
    mesh.groups,
    1 + Math.max(...mesh.clusters.map((c) => c.level)),
    mesh.rootPages,
    listEntries,
  ].forEach((v, k) => {
    out.setUint32(4 * k, v, true);
  });
  mesh.pages.forEach((clusters, p) => {
    const at = MESH_HEADER_BYTES + PAGE_ENTRY_BYTES * p;
    const members = clusters.flatMap((c) => mesh.clusters[c] ?? []);
    const levels = members.map((c) => c.level);
    const own = members.map((c) => c.ownErrorM);
    const parent = members.map((c) => c.parentErrorM);
    out.setUint32(at, offsets[p] ?? 0, true);
    out.setUint32(at + 4, bodies[p]?.byteLength ?? 0, true);
    out.setUint32(at + 8, members.length, true);
    out.setUint32(at + 12, Math.min(...levels) | (Math.max(...levels) << 16), true);
    [Math.min(...own), Math.max(...own), Math.min(...parent), Math.max(...parent)].forEach(
      (v, k) => {
        out.setFloat32(at + 16 + 4 * k, v, true);
      },
    );
    const bounds = enclosing(members.map((c) => c.ownSphere));
    out.setFloat32(at + 32, bounds.centreM.x, true);
    out.setFloat32(at + 36, bounds.centreM.y, true);
    out.setFloat32(at + 40, bounds.centreM.z, true);
    out.setFloat32(at + 44, bounds.radiusM, true);
  });
  const groupTable = MESH_HEADER_BYTES + PAGE_ENTRY_BYTES * mesh.pages.length;
  const list = groupTable + GROUP_ENTRY_BYTES * mesh.groups;
  let entry = 0;
  runs.forEach((run, k) => {
    out.setUint32(groupTable + 8 * k, entry, true);
    out.setUint32(groupTable + 8 * k + 4, run.length, true);
    for (const page of run) {
      out.setUint32(list + PAGE_LIST_ENTRY_BYTES * entry, page, true);
      entry += 1;
    }
  });
  bodies.forEach((body, p) => {
    bytes.set(new Uint8Array(body), offsets[p] ?? 0);
  });
  return bytes.buffer;
}

/** The corners of a quad from `x0` to `x1` m along the strip, two triangles. */
function quad(x0: number, x1: number): number[] {
  return [x0, 0, 0, x1, 0, 0, x1, 1, 0, x0, 0, 0, x1, 1, 0, x0, 1, 0];
}

/** A sphere about (`x`, 0.5, 0) of `radiusM`. */
function sphereAt(x: number, radiusM: number): PageSphere {
  return { centreM: { x, y: 0.5, z: 0 }, radiusM };
}

/** The groups' errors and LOD spheres (see the module's remarks). */
const GROUPS = [
  { errorM: 1, sphere: sphereAt(4, 5.5) },
  { errorM: 1.5, sphere: sphereAt(11, 4.5) },
  { errorM: 4, sphere: sphereAt(7, 9) },
  { errorM: 6, sphere: sphereAt(7.5, 9.5) },
] as const;

/** The three-level DAG (see the module's remarks), its clusters R0, R1, M0–M3, L0–L7 in order. */
export function threeLevelMesh(): SyntheticMesh {
  const group = (g: 0 | 1 | 2 | 3): (typeof GROUPS)[number] => GROUPS[g];
  const made = (
    gen: 0 | 1 | 2 | 3,
    con: 0 | 1 | 2 | 3 | null,
    level: number,
    corners: number[],
  ): SyntheticCluster => ({
    level,
    generatingGroup: gen,
    consumingGroup: con,
    ownErrorM: group(gen).errorM,
    parentErrorM: con === null ? Infinity : group(con).errorM,
    ownSphere: group(gen).sphere,
    parentSphere: con === null ? group(gen).sphere : group(con).sphere,
    corners,
  });
  const leaf = (i: number, con: 0 | 1 | 3): SyntheticCluster => ({
    level: 0,
    generatingGroup: null,
    consumingGroup: con,
    ownErrorM: 0,
    parentErrorM: group(con).errorM,
    ownSphere: sphereAt(2 * i + 1, 1.2),
    parentSphere: group(con).sphere,
    corners: quad(2 * i, 2 * i + 2),
  });
  return {
    clusters: [
      made(2, null, 2, quad(4, 12)),
      made(3, null, 2, [...quad(0, 4), ...quad(12, 16)]),
      made(0, 3, 1, quad(0, 4)),
      made(0, 2, 1, quad(4, 8)),
      made(1, 2, 1, quad(8, 12)),
      made(1, 3, 1, quad(12, 14)),
      leaf(0, 0),
      leaf(1, 0),
      leaf(2, 0),
      leaf(3, 0),
      leaf(4, 1),
      leaf(5, 1),
      leaf(6, 1),
      leaf(7, 3),
    ],
    groups: 4,
    pages: [[0, 1], [2, 3], [4, 5], [6, 7, 8, 9], [10, 11, 12], [13]],
    rootPages: 1,
    originM: { x: 0, y: 0, z: 0 },
    stepM: 1,
  };
}

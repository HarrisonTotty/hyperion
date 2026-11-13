/**
 * The mesh page format, version 2 (plan R14's `MeshPage`, R14.T5.b): the client's reader of what
 * `tools/meshbuild/src/page.rs` writes, whose documentation is the format's statement.
 *
 * @remarks
 * - **The mesh file**: its header ({@link MESH_HEADER_BYTES}: `HMSH`, the version, the pages,
 *   clusters, groups and levels, the root pages and the page list's entries), its page table
 *   ({@link PAGE_ENTRY_BYTES} a page: its offset and bytes, its clusters, its finest and coarsest
 *   levels, the spans of its own and parent errors and a sphere holding its clusters), its group
 *   table ({@link GROUP_ENTRY_BYTES} a group: the runs in the page list of the pages holding the
 *   clusters the group simplified and those holding the clusters it made) and the page list, then
 *   the pages. {@link readMeshIndex} reads all but the pages, which is what residency needs before
 *   any page is fetched.
 * - **A page**: its header ({@link PAGE_HEADER_BYTES}: `HMPG`, the version, its index, its
 *   clusters, vertices, triangles and bytes, its flags, its origin in the mesh's frame as three
 *   `f64`, its grid steps and the sections' offsets), then the records, 32 B a cluster as
 *   `record.ts`' `ClusterRecord` lays them out in the DAG-cut mode, their LOD spheres, 16 B, the
 *   cluster table, 16 B (first vertex, vertex count, the generating group and the consuming group,
 *   {@link NO_GROUP} for none), the vertices, 16 B, and the triangles, 4 B. {@link readMeshPage}
 *   checks every section against the page's bytes; its records and LOD spheres are then uploaded
 *   as they are, so the cull reads the builder's bits.
 * - Refusals are a result, never a throw: a fetched file is a trust boundary.
 *
 * It imports nothing that needs the engine, so that the producer's worker may read pages
 * (`tsconfig.worker.json`).
 */

import type { Vec3 } from "../../geometry/vec3";

/** `HMSH`, the mesh file's magic, little-endian. */
export const MESH_MAGIC = 0x4853_4d48;

/** `HMPG`, a page's magic, little-endian. */
export const PAGE_MAGIC = 0x4750_4d48;

/** The format's version, `page.rs`' `FORMAT_VERSION`. */
export const MESH_FORMAT_VERSION = 2;

/** Bytes of the mesh file's header. */
export const MESH_HEADER_BYTES = 32;

/** Bytes of a page table entry. */
export const PAGE_ENTRY_BYTES = 48;

/** Bytes of a group table entry. */
export const GROUP_ENTRY_BYTES = 16;

/** Bytes of a page list entry: a page's index. */
export const PAGE_LIST_ENTRY_BYTES = 4;

/** Bytes of a page's header. */
export const PAGE_HEADER_BYTES = 96;

/** Bytes of a cluster's record, as `record.ts`' `CLUSTER_RECORD_BYTES`. */
export const PAGE_RECORD_BYTES = 32;

/** Bytes of a cluster's LOD sphere, as `record.ts`' `CLUSTER_LOD_BYTES`. */
export const PAGE_LOD_BYTES = 16;

/** Bytes of a cluster's table entry. */
export const CLUSTER_TABLE_BYTES = 16;

/** Bytes of a vertex. */
export const PAGE_VERTEX_BYTES = 16;

/** Bytes of a triangle. */
export const PAGE_TRIANGLE_BYTES = 4;

/** The most clusters a page holds. */
export const MAX_PAGE_CLUSTERS = 128;

/** The most bytes a page holds: Nanite's streaming page (Karis 2021), `page.rs`' own. */
export const MAX_PAGE_BYTES = 128 * 1024;

/** The most bytes a root page holds where its units fit (`page.rs`' `ROOT_PAGE_BYTES`). */
export const ROOT_PAGE_BYTES = 16 * 1024;

/** The cluster table's group where there is none. */
export const NO_GROUP = 0xffff_ffff;

/** The bits of one grid coordinate. */
const GRID_BITS = 21;

/** A sphere about the page's origin, in the mesh's axes. */
export interface PageSphere {
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/** A page's entry in the page table: what it holds, read before it is fetched. */
export interface MeshPageEntry {
  /** Its offset and bytes in the mesh file. */
  readonly offsetBytes: number;
  readonly bytes: number;
  readonly clusters: number;
  /** Its clusters' finest and coarsest levels. */
  readonly levels: readonly [finest: number, coarsest: number];
  /** The least and greatest of its clusters' own errors, m. */
  readonly ownErrorSpanM: readonly [number, number];
  /** The least and greatest of its clusters' parent errors, m (+∞ at a root). */
  readonly parentErrorSpanM: readonly [number, number];
  /** A sphere holding its clusters' own spheres. */
  readonly bounds: PageSphere;
}

/** A group's entry in the group table: the pages holding its children and its parents. */
export interface MeshGroupPages {
  /** The pages holding the clusters it simplified, ascending. */
  readonly children: ReadonlyArray<number>;
  /** The pages holding the clusters it made, ascending. */
  readonly parents: ReadonlyArray<number>;
}

/** A mesh file's header, page table and group table. */
export interface MeshIndex {
  readonly clusters: number;
  readonly levels: number;
  /** How many pages, from the first, hold every root. */
  readonly rootPages: number;
  readonly pages: ReadonlyArray<MeshPageEntry>;
  readonly groups: ReadonlyArray<MeshGroupPages>;
}

/** A page as fetched and checked, ready for the pool. */
export interface MeshPage {
  readonly index: number;
  /** Its bytes as the file holds them: what the pool uploads. */
  readonly body: ArrayBuffer;
  readonly clusters: number;
  readonly vertices: number;
  readonly triangles: number;
  /** Its origin in the mesh's frame, m, as the `f64`s it holds. */
  readonly originM: Vec3;
  /** The position grid's step, m. */
  readonly stepM: number;
  /** The sections' offsets in {@link MeshPage.body}. */
  readonly sections: {
    readonly records: number;
    readonly lods: number;
    readonly table: number;
    readonly vertices: number;
    readonly triangles: number;
  };
  /** Each cluster's generating and consuming group, {@link NO_GROUP} for none. */
  readonly generatingGroups: Uint32Array;
  readonly consumingGroups: Uint32Array;
}

/** What a read made of its bytes: what they hold, or why they were refused. */
export type FormatRead<T> =
  | { readonly kind: "read"; readonly value: T }
  | { readonly kind: "refused"; readonly reason: string };

/** A refusal of `reason`. */
function refusal(reason: string): { readonly kind: "refused"; readonly reason: string } {
  return { kind: "refused", reason };
}

/** Whether `bytes` from `at` lie within `view`. */
function within(view: DataView, at: number, bytes: number): boolean {
  return at >= 0 && bytes >= 0 && at + bytes <= view.byteLength;
}

/**
 * The header, page table and group table of a mesh file (see the module's remarks).
 *
 * @param bytes - The file, or at least its first bytes through the page list.
 */
export function readMeshIndex(bytes: ArrayBuffer): FormatRead<MeshIndex> {
  const view = new DataView(bytes);
  if (!within(view, 0, MESH_HEADER_BYTES)) {
    return refusal(`a mesh file of ${String(bytes.byteLength)} B holds no header`);
  }
  const word = (at: number): number => view.getUint32(at, true);
  if (word(0) !== MESH_MAGIC) {
    return refusal("not a mesh file: its magic");
  }
  if (word(4) !== MESH_FORMAT_VERSION) {
    return refusal(`a version ${String(word(4))} mesh file, not ${String(MESH_FORMAT_VERSION)}`);
  }
  const pageCount = word(8);
  const clusters = word(12);
  const groupCount = word(16);
  const levels = word(20);
  const rootPages = word(24);
  const listEntries = word(28);
  const table = MESH_HEADER_BYTES;
  const groupTable = table + PAGE_ENTRY_BYTES * pageCount;
  const list = groupTable + GROUP_ENTRY_BYTES * groupCount;
  if (!within(view, 0, list + PAGE_LIST_ENTRY_BYTES * listEntries)) {
    return refusal(`a mesh file of ${String(bytes.byteLength)} B is truncated in its tables`);
  }
  if (rootPages < 1 || rootPages > pageCount) {
    return refusal(`${String(rootPages)} root pages of ${String(pageCount)}`);
  }
  const f32 = (at: number): number => view.getFloat32(at, true);
  const pages: MeshPageEntry[] = [];
  for (let p = 0; p < pageCount; p += 1) {
    const at = table + PAGE_ENTRY_BYTES * p;
    const levelWord = word(at + 12);
    pages.push({
      offsetBytes: word(at),
      bytes: word(at + 4),
      clusters: word(at + 8),
      levels: [levelWord & 0xffff, levelWord >>> 16],
      ownErrorSpanM: [f32(at + 16), f32(at + 20)],
      parentErrorSpanM: [f32(at + 24), f32(at + 28)],
      bounds: {
        centreM: { x: f32(at + 32), y: f32(at + 36), z: f32(at + 40) },
        radiusM: f32(at + 44),
      },
    });
  }
  const groups: MeshGroupPages[] = [];
  for (let g = 0; g < groupCount; g += 1) {
    const at = groupTable + GROUP_ENTRY_BYTES * g;
    const run = (first: number, count: number): number[] | null => {
      if (first + count > listEntries) {
        return null;
      }
      const held = Array.from({ length: count }, (_, k) =>
        word(list + PAGE_LIST_ENTRY_BYTES * (first + k)),
      );
      return held.every((p, k) => p < pageCount && (k === 0 || p > (held[k - 1] ?? 0)))
        ? held
        : null;
    };
    const children = run(word(at), word(at + 4));
    const parents = run(word(at + 8), word(at + 12));
    if (children === null || parents === null) {
      return refusal(`group ${String(g)}'s pages are not an ascending run of the mesh's pages`);
    }
    groups.push({ children, parents });
  }
  return { kind: "read", value: { clusters, levels, rootPages, pages, groups } };
}

/**
 * A page's bytes checked against its header and its entry in the page table (see the module's
 * remarks).
 *
 * @param body - The page's bytes, {@link MeshPageEntry.bytes} of them from its offset.
 */
export function readMeshPage(
  body: ArrayBuffer,
  entry: MeshPageEntry,
  index: number,
): FormatRead<MeshPage> {
  const view = new DataView(body);
  if (body.byteLength !== entry.bytes || !within(view, 0, PAGE_HEADER_BYTES)) {
    return refusal(
      `page ${String(index)} is ${String(body.byteLength)} B, not ${String(entry.bytes)}`,
    );
  }
  const word = (at: number): number => view.getUint32(at, true);
  if (word(0) !== PAGE_MAGIC || word(4) !== MESH_FORMAT_VERSION) {
    return refusal(`page ${String(index)} is not a version ${String(MESH_FORMAT_VERSION)} page`);
  }
  const pageIndex = word(8);
  const clusters = word(12);
  const vertices = word(16);
  const triangles = word(20);
  const bytes = word(24);
  if (pageIndex !== index || clusters !== entry.clusters || bytes !== body.byteLength) {
    return refusal(`page ${String(index)}'s header disagrees with the page table`);
  }
  if (clusters < 1 || clusters > MAX_PAGE_CLUSTERS || bytes > MAX_PAGE_BYTES) {
    return refusal(`page ${String(index)} of ${String(clusters)} clusters in ${String(bytes)} B`);
  }
  const sections = {
    records: word(72),
    lods: word(76),
    table: word(80),
    vertices: word(84),
    triangles: word(88),
  };
  const { records, table } = sections;
  if (
    !within(view, records, PAGE_RECORD_BYTES * clusters) ||
    !within(view, sections.lods, PAGE_LOD_BYTES * clusters) ||
    !within(view, table, CLUSTER_TABLE_BYTES * clusters) ||
    !within(view, sections.vertices, PAGE_VERTEX_BYTES * vertices) ||
    !within(view, sections.triangles, PAGE_TRIANGLE_BYTES * triangles) ||
    Object.values(sections).some((at) => at % 16 !== 0)
  ) {
    return refusal(`page ${String(index)}'s sections lie outside its bytes`);
  }
  const generatingGroups = new Uint32Array(clusters);
  const consumingGroups = new Uint32Array(clusters);
  for (let c = 0; c < clusters; c += 1) {
    const at = table + CLUSTER_TABLE_BYTES * c;
    const [firstVertex, vertexCount] = [word(at), word(at + 4)];
    const template = word(records + PAGE_RECORD_BYTES * c + 24);
    const [firstTriangle, triangleCount] = [template & 0xff_ffff, template >>> 24];
    if (
      firstVertex + vertexCount > vertices ||
      firstTriangle + triangleCount > triangles ||
      triangleCount < 1
    ) {
      return refusal(`page ${String(index)}'s cluster ${String(c)} reaches past its sections`);
    }
    for (let t = firstTriangle; t < firstTriangle + triangleCount; t += 1) {
      const indices = word(sections.triangles + PAGE_TRIANGLE_BYTES * t);
      if ([0, 8, 16].some((k) => ((indices >>> k) & 0xff) >= vertexCount)) {
        return refusal(`page ${String(index)}'s cluster ${String(c)} names a vertex it lacks`);
      }
    }
    generatingGroups[c] = word(at + 8);
    consumingGroups[c] = word(at + 12);
  }
  const f64 = (at: number): number => view.getFloat64(at, true);
  return {
    kind: "read",
    value: {
      index,
      body,
      clusters,
      vertices,
      triangles,
      originM: { x: f64(32), y: f64(40), z: f64(48) },
      stepM: view.getFloat32(56, true),
      sections,
      generatingGroups,
      consumingGroups,
    },
  };
}

/**
 * The grid coordinates of `cluster`'s triangles' corners in `page`, nine a triangle: a vertex is
 * these times the page's step about its origin, the same bits in every page of its mesh, so that
 * a shared edge is the same coordinates whichever page draws it.
 */
export function pageClusterCorners(page: MeshPage, cluster: number): Uint32Array {
  const view = new DataView(page.body);
  const word = (at: number): number => view.getUint32(at, true);
  const firstVertex = word(page.sections.table + CLUSTER_TABLE_BYTES * cluster);
  const template = word(page.sections.records + PAGE_RECORD_BYTES * cluster + 24);
  const [firstTriangle, count] = [template & 0xff_ffff, template >>> 24];
  const mask = (1 << GRID_BITS) - 1;
  const out = new Uint32Array(9 * count);
  for (let t = 0; t < count; t += 1) {
    const indices = word(page.sections.triangles + PAGE_TRIANGLE_BYTES * (firstTriangle + t));
    for (let k = 0; k < 3; k += 1) {
      const at =
        page.sections.vertices + PAGE_VERTEX_BYTES * (firstVertex + ((indices >>> (8 * k)) & 0xff));
      const [w0, w1] = [word(at), word(at + 4)];
      out[9 * t + 3 * k] = w0 & mask;
      out[9 * t + 3 * k + 1] = ((w0 >>> GRID_BITS) | (w1 << (32 - GRID_BITS))) & mask;
      out[9 * t + 3 * k + 2] = w1 >>> (2 * GRID_BITS - 32);
    }
  }
  return out;
}

/**
 * The mesh page format's reader (plan R14, R14.T5.b): a version 2 file's index, its group table
 * and its pages read as `tools/meshbuild` writes them, its sizes the cull's, and every malformed
 * file refused rather than thrown.
 */

import { describe, expect, it } from "vitest";

import { threeLevelMesh, writeMeshFile } from "../../test/meshPages";
import {
  MAX_PAGE_BYTES,
  type MeshIndex,
  NO_GROUP,
  PAGE_LOD_BYTES,
  PAGE_RECORD_BYTES,
  pageClusterCorners,
  readMeshIndex,
  readMeshPage,
} from "./pageFormat";
import { CLUSTER_LOD_BYTES, CLUSTER_RECORD_BYTES } from "./record";

/** The three-level DAG's file and its index. */
function file(): { readonly bytes: ArrayBuffer; readonly index: MeshIndex } {
  const bytes = writeMeshFile(threeLevelMesh());
  const read = readMeshIndex(bytes);
  if (read.kind === "refused") {
    throw new Error(read.reason);
  }
  return { bytes, index: read.value };
}

describe("the mesh page format's reader (plan R14, R14.T5.b)", () => {
  it("reads the index: the pages' summaries and each group's children's and parents' pages", () => {
    const { index } = file();
    expect([index.clusters, index.levels, index.rootPages, index.pages.length]).toEqual([
      14, 3, 1, 6,
    ]);
    expect(index.pages.map((p) => [p.clusters, p.levels, p.parentErrorSpanM])).toEqual([
      [2, [2, 2], [Infinity, Infinity]],
      [2, [1, 1], [4, 6]],
      [2, [1, 1], [4, 6]],
      [4, [0, 0], [1, 1]],
      [3, [0, 0], [1.5, 1.5]],
      [1, [0, 0], [6, 6]],
    ]);
    expect(index.groups).toEqual([
      { children: [3], parents: [1] },
      { children: [4], parents: [2] },
      { children: [1, 2], parents: [0] },
      { children: [1, 2, 5], parents: [0] },
    ]);
  });

  it("reads each page's clusters' groups and their corners on the mesh's one grid", () => {
    const { bytes, index } = file();
    const entry = index.pages[1];
    if (entry === undefined) {
      throw new Error("no page 1");
    }
    const read = readMeshPage(
      bytes.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes),
      entry,
      1,
    );
    if (read.kind === "refused") {
      throw new Error(read.reason);
    }
    const page = read.value;
    expect([
      [...page.generatingGroups],
      [...page.consumingGroups],
      [...pageClusterCorners(page, 0)],
      page.originM,
      page.stepM,
    ]).toEqual([
      [0, 0],
      [3, 2],
      [0, 0, 0, 4, 0, 0, 4, 1, 0, 0, 0, 0, 4, 1, 0, 0, 1, 0],
      { x: 0, y: 0, z: 0 },
      1,
    ]);
    const roots = index.pages[0];
    if (roots === undefined) {
      throw new Error("no root page");
    }
    const rootPage = readMeshPage(
      bytes.slice(roots.offsetBytes, roots.offsetBytes + roots.bytes),
      roots,
      0,
    );
    expect(rootPage.kind === "read" ? [...rootPage.value.consumingGroups] : null).toEqual([
      NO_GROUP,
      NO_GROUP,
    ]);
  });

  it("lays its records and LOD spheres out as the cull reads them, in pages of at most 128 KiB", () => {
    expect([PAGE_RECORD_BYTES, PAGE_LOD_BYTES, MAX_PAGE_BYTES]).toEqual([
      CLUSTER_RECORD_BYTES,
      CLUSTER_LOD_BYTES,
      131_072,
    ]);
  });

  it("refuses a truncated file, another format's or version's, and a group's pages out of order", () => {
    const { bytes } = file();
    const changed = (at: number, value: number): ArrayBuffer => {
      const copy = bytes.slice(0);
      new DataView(copy).setUint32(at, value, true);
      return copy;
    };
    // The group table follows the 6 pages' entries; group 2's children run is its first word.
    const groupTable = 32 + 48 * 6;
    expect(
      [
        readMeshIndex(bytes.slice(0, 20)),
        readMeshIndex(bytes.slice(0, groupTable)),
        readMeshIndex(changed(0, 0x5853_4d48)),
        readMeshIndex(changed(4, 1)),
        readMeshIndex(changed(24, 0)),
        readMeshIndex(changed(groupTable + 16 * 2 + 4, 9)),
      ].map((read) => read.kind),
    ).toEqual(["refused", "refused", "refused", "refused", "refused", "refused"]);
  });

  it("refuses a page whose header disagrees with its entry or whose sections pass its bytes", () => {
    const { bytes, index } = file();
    const entry = index.pages[3];
    if (entry === undefined) {
      throw new Error("no page 3");
    }
    const body = (): ArrayBuffer => bytes.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes);
    const changed = (at: number, value: number): ArrayBuffer => {
      const copy = body();
      new DataView(copy).setUint32(at, value, true);
      return copy;
    };
    expect(
      [
        readMeshPage(body(), entry, 3),
        readMeshPage(body(), entry, 4),
        readMeshPage(body().slice(0, entry.bytes - 16), entry, 3),
        readMeshPage(changed(0, 0), entry, 3),
        readMeshPage(changed(84, entry.bytes), entry, 3),
        readMeshPage(changed(80, 100), entry, 3),
      ].map((read) => read.kind),
    ).toEqual(["read", "refused", "refused", "refused", "refused", "refused"]);
  });

  it("refuses a page whose triangle names a vertex outside its cluster", () => {
    const { bytes, index } = file();
    const entry = index.pages[3];
    if (entry === undefined) {
      throw new Error("no page 3");
    }
    const body = bytes.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes);
    const view = new DataView(body);
    // Leaf L0's first triangle, its third corner the ninth vertex of a cluster of four.
    view.setUint32(view.getUint32(88, true), 0 | (1 << 8) | (9 << 16), true);
    expect(readMeshPage(body, entry, 3)).toEqual({
      kind: "refused",
      reason: "page 3's cluster 0 names a vertex it lacks",
    });
  });
});

/**
 * The mesh page pool (plan R14, R14.T5.b, Design note 14): its slots within its bytes, LRU with
 * the pins, a group's pages evicted together, roots past their share refused, and at every
 * residency state of a three-level DAG one cut with no hole and no overlap, a missing page drawn
 * by its resident coarser group.
 */

import { describe, expect, it } from "vitest";

import { threeLevelMesh, writeMeshFile } from "../../test/meshPages";
import { CutChecker, drawnClusters, type MeshFileRead, readMeshFile } from "../../test/meshFlyby";
import { MirrorEngine } from "../../test/mirrorEngine";
import { MAX_PAGE_CLUSTERS, type MeshPage, ROOT_PAGE_BYTES } from "./pageFormat";
import { GeometryPagePool, pagePoolLayout, pagePoolSlots, pageSlotOffsetBytes } from "./pages";

/** The three-level DAG read whole. */
function dag(): MeshFileRead {
  const read = readMeshFile(writeMeshFile(threeLevelMesh()));
  if (read.kind === "refused") {
    throw new Error(read.reason);
  }
  return read.value;
}

/** A pool of `pageSlots` page slots and `rootSlots` root slots, with its mirror. */
function poolOf(
  pageSlots: number,
  rootSlots = 2,
): { readonly pool: GeometryPagePool; readonly engine: MirrorEngine } {
  const engine = new MirrorEngine();
  return { pool: new GeometryPagePool(engine, pagePoolSlots(rootSlots, pageSlots)), engine };
}

/** Page `p` of `file`. */
function pageOf(file: MeshFileRead, p: number): MeshPage {
  const page = file.pages[p];
  if (page === undefined) {
    throw new Error(`no page ${String(p)}`);
  }
  return page;
}

/** Stores `pages` of `file` as `mesh` in `pool`, in order, each stored. */
function store(
  pool: GeometryPagePool,
  mesh: number,
  file: MeshFileRead,
  pages: ReadonlyArray<number>,
): void {
  for (const p of pages) {
    const page = file.pages[p];
    if (page === undefined || pool.insert(mesh, page).kind !== "stored") {
      throw new Error(`page ${String(p)} was not stored`);
    }
  }
}

/** The first four records' radii in `slot`, as the pool wrote them. */
function radiiOf(engine: MirrorEngine, pool: GeometryPagePool, slot: number): number[] {
  const view = new DataView(engine.bytes(pool.records).buffer);
  return [0, 1, 2, 3].map((c) => view.getFloat32((slot * MAX_PAGE_CLUSTERS + c) * 32 + 12, true));
}

/** The cut key of `cluster` on `page`. */
function key(page: number, cluster: number): number {
  return page * MAX_PAGE_CLUSTERS + cluster;
}

describe("the mesh page pool's slots (plan R14, R14.T5.b)", () => {
  it.each([
    ["low", 32 * 2 ** 20, 148, 220],
    ["high", 128 * 2 ** 20, 595, 880],
  ])(
    "holds %s's pages, records and LOD spheres within its bytes, its roots within 10%%",
    (_, bytes, roots, pages) => {
      const layout = pagePoolLayout(bytes);
      const engine = new MirrorEngine();
      const pool = new GeometryPagePool(engine, bytes);
      const record = MAX_PAGE_CLUSTERS * 48;
      expect([
        layout.rootSlots,
        layout.pageSlots,
        engine.heldBytes("geometry-pages") <= bytes,
        layout.rootSlots * (ROOT_PAGE_BYTES + record) <= 0.1 * bytes,
        pageSlotOffsetBytes(layout, layout.rootSlots),
      ]).toEqual([roots, pages, true, true, roots * ROOT_PAGE_BYTES]);
      pool.dispose();
      expect(engine.heldBytes("geometry-pages")).toBe(0);
    },
  );
});

describe("the mesh page pool's pins and eviction (plan R14, R14.T5.b)", () => {
  it("evicts the least recently used unpinned page, the finest first among those used as recently", () => {
    const file = dag();
    const { pool } = poolOf(3, 4);
    const [a = 0, b = 0, c = 0, d = 0] = [0, 1, 2, 3].map(() => pool.addMesh(file.index));
    for (const mesh of [a, b, c, d]) {
      store(pool, mesh, file, [0]);
    }
    store(pool, a, file, [1]);
    store(pool, b, file, [1]);
    store(pool, c, file, [3]);
    // a's page 1 and c's page 3 are used a frame before b's page 1.
    pool.retain(new Set([pool.slotOf(a, 1) ?? -1, pool.slotOf(c, 3) ?? -1]), new Set());
    pool.retain(new Set([pool.slotOf(b, 1) ?? -1]), new Set());
    const first = pool.insert(d, pageOf(file, 1));
    const second = pool.insert(a, pageOf(file, 2));
    expect(
      [first, second].map((arrival) => (arrival.kind === "stored" ? arrival.evicted : arrival)),
    ).toEqual([[{ mesh: c, page: 3 }], [{ mesh: a, page: 1 }]]);
  });

  it("never evicts a drawn or forced page, refusing the arrival instead", () => {
    const file = dag();
    const { pool } = poolOf(2);
    const mesh = pool.addMesh(file.index);
    store(pool, mesh, file, [0, 1, 2]);
    pool.retain(new Set([pool.slotOf(mesh, 1) ?? -1]), new Set([pool.slotOf(mesh, 2) ?? -1]));
    expect([
      pool.insert(mesh, pageOf(file, 3)),
      pool.pressure().pinned,
      pool.pressure().pagesRefused,
    ]).toEqual([{ kind: "refused", reason: "pinned" }, 2, 1]);
    expect([pool.slotOf(mesh, 1) !== undefined, pool.slotOf(mesh, 2) !== undefined]).toEqual([
      true,
      true,
    ]);
  });

  it("evicts a group's pages together: the pages a leaving page leaves with no reachable cluster", () => {
    const file = dag();
    const { pool } = poolOf(5);
    const mesh = pool.addMesh(file.index);
    store(pool, mesh, file, [0, 1, 2, 3, 4, 5]);
    // L7's page, of G3's children, is the oldest: without it G3 closes, so M0 and M3 are
    // unreachable, so G0 and G1, which made them, close, and the leaves of pages 3 and 4 with them.
    pool.retain(new Set([1, 2, 3, 4].map((p) => pool.slotOf(mesh, p) ?? -1)), new Set());
    pool.retain(new Set(), new Set());
    const other = pool.addMesh(file.index);
    store(pool, other, file, [0]);
    const arrival = pool.insert(other, pageOf(file, 1));
    expect(
      arrival.kind === "stored"
        ? arrival.evicted.map((k) => k.page).toSorted((p, q) => p - q)
        : arrival,
    ).toEqual([3, 4, 5]);
  });

  it("passes over a victim whose leaving would strand a pinned page", () => {
    const file = dag();
    const { pool } = poolOf(5);
    const mesh = pool.addMesh(file.index);
    store(pool, mesh, file, [0, 1, 2, 3, 4, 5]);
    // L7's page is the oldest unpinned, but its leaving strands page 3, which is drawn.
    pool.retain(new Set([1, 2, 4].map((p) => pool.slotOf(mesh, p) ?? -1)), new Set());
    pool.retain(new Set([pool.slotOf(mesh, 3) ?? -1]), new Set());
    const other = pool.addMesh(file.index);
    store(pool, other, file, [0]);
    const arrival = pool.insert(other, pageOf(file, 1));
    expect(arrival.kind === "stored" ? arrival.evicted : arrival).toEqual([{ mesh, page: 4 }]);
  });

  it("pins every root and refuses roots past its root slots, which its pressure states", () => {
    const file = dag();
    const { pool } = poolOf(1);
    const root = pageOf(file, 0);
    const answers = [0, 1, 2].map(() => pool.insert(pool.addMesh(file.index), root).kind);
    expect([answers, pool.pressure().rootsRefused, pool.pressure().rootSlotsUsed]).toEqual([
      ["stored", "stored", "refused"],
      1,
      2,
    ]);
  });

  it("holds no cluster past a page's own in a slot it reuses", () => {
    const file = dag();
    const { pool, engine } = poolOf(1);
    const a = pool.addMesh(file.index);
    const b = pool.addMesh(file.index);
    store(pool, a, file, [0, 3]);
    const slot = pool.slotOf(a, 3) ?? -1;
    pool.retain(new Set(), new Set());
    // b's page 4, of three clusters, takes the slot of a's page 3, of four.
    store(pool, b, file, [0, 4]);
    expect([pool.slotOf(b, 4), radiiOf(engine, pool, slot)[3]]).toEqual([slot, -1]);
  });

  it("writes the records of the slots its evictions free as no cluster", () => {
    const file = dag();
    const { pool, engine } = poolOf(5);
    const mesh = pool.addMesh(file.index);
    store(pool, mesh, file, [0, 1, 2, 3, 4, 5]);
    const freed = [3, 4, 5].map((p) => pool.slotOf(mesh, p) ?? -1);
    pool.retain(new Set([1, 2].map((p) => pool.slotOf(mesh, p) ?? -1)), new Set());
    const other = pool.addMesh(file.index);
    store(pool, other, file, [0, 1]);
    // Page 5's leaving strands pages 3 and 4: one of the three slots is the arrival's.
    const reused = pool.slotOf(other, 1);
    const empty = freed
      .filter((slot) => slot !== reused)
      .map((slot) => radiiOf(engine, pool, slot));
    expect([empty.length, empty.flat().every((r) => r === -1)]).toEqual([2, true]);
  });

  it("frees a removed mesh's slots, its roots' with them, and writes them as no cluster", () => {
    const file = dag();
    const { pool, engine } = poolOf(5);
    const mesh = pool.addMesh(file.index);
    store(pool, mesh, file, [0, 3]);
    const slots = [0, 3].map((p) => pool.slotOf(mesh, p) ?? -1);
    pool.removeMesh(mesh);
    expect([
      pool.slotOf(mesh, 3),
      pool.pressure().pageSlotsUsed,
      pool.pressure().rootSlotsUsed,
      slots.flatMap((slot) => radiiOf(engine, pool, slot)).every((r) => r === -1),
    ]).toEqual([undefined, 0, 0, true]);
  });
});

describe("the mesh page pool's record rewrites (plan R14, R14.T5.b, Design note 14)", () => {
  it("draws a missing page's resident coarser group in its place, and none of its resident children", () => {
    const file = dag();
    const { pool, engine } = poolOf(5);
    const mesh = pool.addMesh(file.index);
    store(pool, mesh, file, [0, 1, 2]);
    // Near, at a tolerance that asks for the leaves everywhere: G2 is open (its children on pages
    // 1 and 2), so M1 and M2 draw for their missing leaves; G3 is not (L7's page missing), so R1
    // draws for M0, M3 and L7, and M0 and M3 do not.
    const [near] = drawnClusters(pool, engine, [{ mesh, originM: { x: -8, y: 0, z: -3 } }], 1e-3);
    expect([...(near ?? [])].toSorted((p, q) => p - q)).toEqual([key(0, 1), key(1, 1), key(2, 0)]);
    const errors = pool.writtenErrors(mesh).get(pool.slotOf(mesh, 1) ?? -1);
    // M0: unreachable, its parent written 0; M1: its own written 0, standing in for its leaves.
    expect([...(errors ?? [])]).toEqual([1, 0, 0, 4]);
  });

  it("keeps one cut, no hole and no overlap, at every residency state of the three-level DAG", () => {
    const file = dag();
    const checker = new CutChecker(file);
    let states = 0;
    let mixed = 0;
    for (let state = 0; state < 32; state += 1) {
      const pages = [1, 2, 3, 4, 5].filter((_, k) => (state & (1 << k)) !== 0);
      // Half the states arrive finest first, so that pages land before their parents.
      const order = state % 2 === 0 ? pages : pages.toReversed();
      const { pool, engine } = poolOf(5);
      const mesh = pool.addMesh(file.index);
      store(pool, mesh, file, [0, ...order]);
      for (const distanceM of [0, 2, 8, 30, 120, 500, 2_000, 8_000]) {
        for (const toleranceRad of [1e-4, 1e-3, 1e-2]) {
          const [cut] = drawnClusters(
            pool,
            engine,
            [{ mesh, originM: { x: -8, y: -0.5, z: -distanceM } }],
            toleranceRad,
          );
          const drawn = cut ?? new Set<number>();
          expect(checker.defects(drawn)).toEqual({ holes: 0, overlaps: 0 });
          // A cut across levels: a root and a finer cluster both drawn.
          const keys = [...drawn];
          mixed +=
            keys.some((k) => k < MAX_PAGE_CLUSTERS) && keys.some((k) => k >= MAX_PAGE_CLUSTERS)
              ? 1
              : 0;
        }
      }
      states += 1;
    }
    expect([states, mixed > 50]).toEqual([32, true]);
  });
});

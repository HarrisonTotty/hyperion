/**
 * Mesh page residency (plan R14, R14.T5.b, Design note 14): the CPU's demand at an instance's
 * nearest point, the requests in score order up to the fetches in flight, the frame's pins, and
 * each page's answer, resident or stood in for by its nearest resident coarser page.
 */

import { describe, expect, it } from "vitest";

import { threeLevelMesh, writeMeshFile } from "../../test/meshPages";
import { MirrorEngine } from "../../test/mirrorEngine";
import { InThreadMeshPageProducer } from "./meshPageProducer";
import type { FormatRead, MeshIndex, MeshPage, MeshPageEntry } from "./pageFormat";
import { readMeshIndex } from "./pageFormat";
import { GeometryPagePool, pagePoolSlots } from "./pages";
import {
  MESH_FETCHES_IN_FLIGHT,
  meshPageDemand,
  type MeshPageProducer,
  MeshResidency,
} from "./residency";

const URL = "mesh:three-level";

/** The three-level DAG's index. */
function index(): MeshIndex {
  const read = readMeshIndex(writeMeshFile(threeLevelMesh()));
  if (read.kind === "refused") {
    throw new Error(read.reason);
  }
  return read.value;
}

/** A producer over the three-level DAG's file whose pages but the roots finish only when released. */
class HeldProducer implements MeshPageProducer {
  readonly asked: number[] = [];
  readonly forgotten: string[] = [];
  readonly #inner = new InThreadMeshPageProducer(() =>
    Promise.resolve(writeMeshFile(threeLevelMesh())),
  );
  readonly #held: Array<() => void> = [];
  disposed = false;

  index(url: string): Promise<FormatRead<MeshIndex>> {
    return this.#inner.index(url);
  }

  page(url: string, entry: MeshPageEntry, page: number): Promise<FormatRead<MeshPage>> {
    this.asked.push(page);
    if (page === 0) {
      return this.#inner.page(url, entry, page);
    }
    return new Promise((resolve, reject) => {
      this.#held.push(() => {
        void this.#inner.page(url, entry, page).then(resolve, reject);
      });
    });
  }

  /** Lets every page asked for so far finish, and waits for them. */
  async release(): Promise<void> {
    for (const run of this.#held.splice(0)) {
      run();
    }
    await new Promise((resolve) => {
      setTimeout(resolve, 0);
    });
  }

  forget(url: string): void {
    this.forgotten.push(url);
  }

  dispose(): void {
    this.disposed = true;
  }
}

/**
 * A producer over the three-level DAG's file that refuses every page but the roots, or the roots
 * too where `refuseRoots`.
 */
function refusing(refuseRoots: boolean): MeshPageProducer {
  const inner = new InThreadMeshPageProducer(() =>
    Promise.resolve(writeMeshFile(threeLevelMesh())),
  );
  return {
    index: (url) => inner.index(url),
    page: (url, entry, page) =>
      page === 0 && !refuseRoots
        ? inner.page(url, entry, page)
        : Promise.resolve({ kind: "refused", reason: "a test's refusal" }),
    forget: () => undefined,
    dispose: () => undefined,
  };
}

/** A residency over a pool of four root slots and `pageSlots`, the three-level DAG loaded. */
async function loaded(pageSlots = 5): Promise<{
  readonly residency: MeshResidency;
  readonly producer: HeldProducer;
  readonly mesh: number;
}> {
  const producer = new HeldProducer();
  const residency = new MeshResidency(
    new GeometryPagePool(new MirrorEngine(), pagePoolSlots(4, pageSlots)),
    producer,
  );
  const load = await residency.load(URL);
  if (load.kind === "refused") {
    throw new Error(load.reason);
  }
  return { residency, producer, mesh: load.mesh };
}

describe("the mesh pages' demand (plan R14, R14.T5.b, Design note 14)", () => {
  it("requests the coarsest level within τ at an instance's nearest point and its ancestors", () => {
    const pages = (nearestM: number): number[] =>
      meshPageDemand(index(), nearestM, 1e-3).map((want) => want.page);
    // At 2 km τ is 2 m there: M0–M3 (own 1 or 1.5 m, parents 4 or 6 m) and L7, a leaf left over
    // under G3's 6 m, are the cut, so their pages and the roots'; at 8 km the roots alone; inside
    // the bounds, every page.
    expect([pages(2_000), pages(8_000), pages(0), pages(1_200)]).toEqual([
      [0, 1, 2, 5],
      [0],
      [0, 1, 2, 3, 4, 5],
      [0, 1, 2, 4, 5],
    ]);
  });

  it("scores a page by its greatest parent error over the distance, the roots first, scaled with the instance", () => {
    expect(
      meshPageDemand(index(), 100, 1e-3, 2).map((want) => [want.page, want.priorityRad]),
    ).toEqual([
      [0, Infinity],
      [1, 0.12],
      [2, 0.12],
      [3, 0.02],
      [4, 0.03],
      [5, 0.12],
    ]);
  });
});

describe("the mesh pages' residency (plan R14, R14.T5.b)", () => {
  it("asks for the most wanted missing pages first, stores them when they arrive, and answers them resident", async () => {
    const { residency, producer, mesh } = await loaded();
    for (const want of meshPageDemand(index(), 0, 1e-3)) {
      residency.request({ mesh, page: want.page }, want.priorityRad);
    }
    residency.update();
    // The roots asked for with the mesh, then the rest by their scores.
    expect([producer.asked, residency.fetching]).toEqual([[0, 1, 2, 5, 4, 3], true]);
    await producer.release();
    residency.request({ mesh, page: 3 }, 1);
    residency.update();
    expect([
      residency.answer({ mesh, page: 0 }).kind,
      residency.answer({ mesh, page: 3 }).kind,
      residency.counts(),
      residency.pool.pressure().pinned,
    ]).toEqual([
      "resident",
      "resident",
      { fetched: 6, stored: 6, refused: 0, failed: 0, evicted: 0 },
      1,
    ]);
  });

  it("asks for no more pages than its unpinned page slots would take", async () => {
    const { residency, producer, mesh } = await loaded(2);
    const ask = (): void => {
      for (let page = 1; page < 6; page += 1) {
        residency.request({ mesh, page }, 6 - page);
      }
      residency.update();
    };
    ask();
    await producer.release();
    // Pages 1 and 2 arrive and are pinned by the next frame's requests: nothing more is asked.
    ask();
    ask();
    expect([producer.asked, residency.counts().refused]).toEqual([[0, 1, 2], 0]);
  });

  it("keeps no more fetches in flight than its bound", async () => {
    const { residency, producer } = await loaded(12);
    const meshes = await Promise.all([0, 1].map(() => residency.load(URL)));
    for (const load of meshes) {
      for (let page = 0; page < 6; page += 1) {
        residency.request({ mesh: load.kind === "loaded" ? load.mesh : -1, page }, 1);
      }
    }
    residency.update();
    // Each mesh's roots, then the bound's.
    expect(producer.asked).toHaveLength(3 + MESH_FETCHES_IN_FLIGHT);
  });

  it("answers a missing page by its nearest resident coarser page that the cut draws", async () => {
    const { residency, producer, mesh } = await loaded();
    const standIn = (page: number): unknown => residency.answer({ mesh, page });
    const slot = (page: number): number | undefined => residency.pool.slotOf(mesh, page);
    const arrive = async (pages: ReadonlyArray<number>): Promise<void> => {
      for (const page of pages) {
        residency.request({ mesh, page }, 1);
      }
      residency.update();
      await producer.release();
      residency.update();
    };
    // Page 1 alone: G2 and G3 lack pages 2 and 5, so M0 and M1 draw nowhere and the roots stand
    // in for page 3's leaves.
    await arrive([1]);
    const alone = standIn(3);
    // With pages 2 and 5, G2 and G3 open: M0 and M1, on page 1, stand in for G0's leaves.
    await arrive([2, 5]);
    expect([alone, standIn(3)]).toEqual([
      { kind: "missing", standIn: slot(0) },
      { kind: "missing", standIn: slot(1) },
    ]);
  });

  it("pins its forced pages as forced", async () => {
    const { residency, producer, mesh } = await loaded();
    residency.request({ mesh, page: 2 }, 1, "forced");
    residency.update();
    await producer.release();
    residency.request({ mesh, page: 2 }, 1, "forced");
    residency.update();
    expect(residency.pool.pressure().pinned).toBe(1);
  });

  it("frees an unloaded mesh's slots, and has the producer forget its file once no mesh is of it", async () => {
    const { residency, producer, mesh } = await loaded();
    const other = await residency.load(URL);
    residency.unload(mesh);
    const kept = [...producer.forgotten];
    residency.unload(other.kind === "loaded" ? other.mesh : -1);
    expect([kept, producer.forgotten, residency.pool.pressure().rootSlotsUsed]).toEqual([
      [],
      [URL],
      0,
    ]);
  });

  it("stops its producer when disposed", async () => {
    const { residency, producer } = await loaded();
    residency.dispose();
    expect(producer.disposed).toBe(true);
  });

  it("counts a page the producer refuses, and stores nothing of it", async () => {
    const residency = new MeshResidency(
      new GeometryPagePool(new MirrorEngine(), pagePoolSlots(2, 5)),
      refusing(false),
    );
    const load = await residency.load(URL);
    const mesh = load.kind === "loaded" ? load.mesh : -1;
    residency.request({ mesh, page: 3 }, 1);
    residency.update();
    await new Promise((resolve) => {
      setTimeout(resolve, 0);
    });
    residency.update();
    expect([residency.counts().failed, residency.answer({ mesh, page: 3 }).kind]).toEqual([
      1,
      "missing",
    ]);
  });

  it("refuses a mesh whose roots the producer refuses, and keeps none of it", async () => {
    const residency = new MeshResidency(
      new GeometryPagePool(new MirrorEngine(), pagePoolSlots(2, 5)),
      refusing(true),
    );
    expect([await residency.load(URL), residency.pool.pressure().rootSlotsUsed]).toEqual([
      { kind: "refused", reason: `${URL}'s roots: a test's refusal` },
      0,
    ]);
  });

  it("refuses a mesh whose roots find no root slot", async () => {
    const residency = new MeshResidency(
      new GeometryPagePool(new MirrorEngine(), pagePoolSlots(1, 5)),
      new InThreadMeshPageProducer(() => Promise.resolve(writeMeshFile(threeLevelMesh()))),
    );
    const loads = [await residency.load(URL), await residency.load(URL)];
    expect([loads.map((l) => l.kind), residency.pool.pressure().rootsRefused]).toEqual([
      ["loaded", "refused"],
      1,
    ]);
  });
});

/**
 * Mesh page residency (plan R14, R14.T5.b, Design note 14): the CPU's demand for each instance's
 * pages, the request queue scored by projected error, the producer's fetches, and the answer per
 * page, over {@link GeometryPagePool}.
 *
 * @remarks
 * - **Demand**: a `presentation-only` cut never reaches the CPU, so the demand is the CPU's: for an
 *   instance whose bounds are `nearestM` from the camera at their nearest point, a page is wanted
 *   where its greatest parent error projects past the tolerance there, since a cluster draws only
 *   where its parent's error does ({@link meshPageDemand}). That is the coarsest level within the
 *   tolerance at the nearest point and every coarser one: the errors are monotone up the DAG, so
 *   a wanted page's ancestors and its groups' other pages are wanted too, and the roots, of
 *   parent error +∞, always. Its score is that projection, rad; visibility and motion's
 *   look-ahead (seamless-3's L) wait for the instance cull (T5.d).
 * - **A mesh** is loaded with its root pages resident and pinned ({@link MeshResidency.load}),
 *   so that from its first frame some cut of it is drawn.
 * - **A frame**: each instance's wanted pages are {@link MeshResidency.request}ed; then
 *   {@link MeshResidency.update} pins the frame's resident wanted pages (`drawn`, or `forced` for
 *   the could-touch region's), stores the pages the producer has finished, the most wanted first,
 *   and asks for the most wanted missing pages up to {@link MESH_FETCHES_IN_FLIGHT}, and no more
 *   than the page slots not pinned, so that a pool whose wanted pages fill it asks for nothing it
 *   would refuse and the cut stays at its stand-ins there. A page arrives frames later; until then
 *   {@link MeshResidency.answer} names the nearest resident coarser page that stands in for it,
 *   whose records the pool has rewritten to draw in its place.
 */

import type { FormatRead, MeshIndex, MeshPage, MeshPageEntry } from "./pageFormat";
import type { GeometryPagePool, MeshPageKey } from "./pages";

/**
 * A residency's answer per key (plan R14's `ClusterResidency`): resident in its slot, or missing,
 * with the slot that stands in for it, `null` where none does yet.
 */
export interface ClusterResidency<K> {
  request(key: K, priority: number): void;
  answer(key: K): ResidencyAnswer;
}

/** A residency's answer for a key: resident in its slot, or missing with its stand-in's slot. */
export type ResidencyAnswer =
  | { readonly kind: "resident"; readonly slot: number }
  | { readonly kind: "missing"; readonly standIn: number | null };

/** The most page fetches a residency keeps in flight. */
export const MESH_FETCHES_IN_FLIGHT = 8;

/**
 * The least distance a page's score is taken at, m: inside an instance's bounds its pages are
 * still scored by their errors, the coarsest first.
 */
const DEMAND_FLOOR_M = 1e-3;

/** One page an instance wants, with its score. */
export interface PageDemand {
  readonly page: number;
  /**
   * Its greatest parent error over the instance's nearest distance, at least 1 mm, rad; +∞ for a
   * root page.
   */
  readonly priorityRad: number;
}

/**
 * The pages of `index` an instance wants (see the module's remarks): those whose greatest parent
 * error passes `toleranceRad` × `nearestM`, in page order.
 *
 * @param nearestM - The camera's distance to the instance's bounds at their nearest point, m, 0
 *   inside them.
 * @param scale - The instance's scale, which scales each page's errors.
 */
export function meshPageDemand(
  index: MeshIndex,
  nearestM: number,
  toleranceRad: number,
  scale = 1,
): PageDemand[] {
  const out: PageDemand[] = [];
  index.pages.forEach((entry, page) => {
    const parentM = scale * entry.parentErrorSpanM[1];
    if (parentM > toleranceRad * nearestM) {
      out.push({ page, priorityRad: parentM / Math.max(nearestM, DEMAND_FLOOR_M) });
    }
  });
  return out;
}

/** What makes a mesh's index and pages from its file: a worker, or this thread in tests. */
export interface MeshPageProducer {
  /** The mesh file's index, from its first bytes. */
  index(url: string): Promise<FormatRead<MeshIndex>>;
  /** Page `page` of the mesh file, at its entry's bytes, checked. */
  page(url: string, entry: MeshPageEntry, page: number): Promise<FormatRead<MeshPage>>;
  /** Drops what it holds of the mesh file; a later request fetches it again. */
  forget(url: string): void;
  /** Stops the producer; every fetch still in flight is dropped. */
  dispose(): void;
}

/** What loading a mesh did: added with its handle, or refused with the reason. */
export type MeshLoad =
  | { readonly kind: "loaded"; readonly mesh: number }
  | { readonly kind: "refused"; readonly reason: string };

/** A request of this frame: its best score and its pin. */
interface Wanted {
  priority: number;
  forced: boolean;
}

/** The key's string, for maps. */
function keyOf(key: MeshPageKey): string {
  return `${String(key.mesh)}:${String(key.page)}`;
}

/** Counters a residency keeps, for the fly-by and the metrics. */
export interface ResidencyCounts {
  /** Pages asked of the producer, stored, refused by the pool, and refused by the producer. */
  readonly fetched: number;
  readonly stored: number;
  readonly refused: number;
  readonly failed: number;
  /** Pages evicted to make room. */
  readonly evicted: number;
}

/**
 * The residency of meshes' pages in a pool (see the module's remarks): one a pool, its meshes
 * loaded by their files' URLs.
 */
export class MeshResidency implements ClusterResidency<MeshPageKey> {
  readonly pool: GeometryPagePool;
  readonly #producer: MeshPageProducer;
  readonly #urls = new Map<number, string>();
  /** For each mesh, each page's groups whose children it holds. */
  readonly #childOf = new Map<number, ReadonlyArray<ReadonlyArray<number>>>();
  #wanted = new Map<string, { key: MeshPageKey; want: Wanted }>();
  readonly #inFlight = new Set<string>();
  #arrived: Array<{ key: MeshPageKey; page: MeshPage }> = [];
  #disposed = false;
  #counts = { fetched: 0, stored: 0, refused: 0, failed: 0, evicted: 0 };

  constructor(pool: GeometryPagePool, producer: MeshPageProducer) {
    this.pool = pool;
    this.#producer = producer;
  }

  /**
   * Adds the mesh at `url` with its root pages resident and pinned, so that some cut of it is
   * always drawn; refused where its index or a root cannot be read, or its roots find no root
   * slots (the pool's `pressure().rootsRefused`, which the annunciation reads).
   */
  async load(url: string): Promise<MeshLoad> {
    const read = await this.#producer.index(url);
    if (read.kind === "refused") {
      return read;
    }
    const index = read.value;
    const roots = await Promise.all(
      index.pages
        .slice(0, index.rootPages)
        .map((entry, page) => this.#producer.page(url, entry, page)),
    );
    this.#counts.fetched += roots.length;
    const mesh = this.pool.addMesh(index);
    for (const root of roots) {
      const stored = root.kind === "read" ? this.pool.insert(mesh, root.value) : root;
      if (stored.kind === "refused") {
        this.pool.removeMesh(mesh);
        this.#counts.failed += root.kind === "refused" ? 1 : 0;
        return { kind: "refused", reason: `${url}'s roots: ${stored.reason}` };
      }
      this.#counts.stored += 1;
    }
    this.#urls.set(mesh, url);
    const childOf: number[][] = read.value.pages.map(() => []);
    read.value.groups.forEach((group, g) => {
      for (const page of group.children) {
        childOf[page]?.push(g);
      }
    });
    this.#childOf.set(mesh, childOf);
    return { kind: "loaded", mesh };
  }

  /**
   * Removes `mesh`: its slots freed, its roots' with them, and its file forgotten by the producer
   * once no loaded mesh is of it.
   */
  unload(mesh: number): void {
    const url = this.#urls.get(mesh);
    this.pool.removeMesh(mesh);
    this.#urls.delete(mesh);
    this.#childOf.delete(mesh);
    if (url !== undefined && ![...this.#urls.values()].includes(url)) {
      this.#producer.forget(url);
    }
  }

  /**
   * Asks for `key` this frame at `priority`, its score (rad: {@link PageDemand.priorityRad});
   * `forced` pins it as the forced region's.
   */
  request(key: MeshPageKey, priority: number, pin: "drawn" | "forced" = "drawn"): void {
    const at = keyOf(key);
    const known = this.#wanted.get(at);
    if (known === undefined) {
      this.#wanted.set(at, { key, want: { priority, forced: pin === "forced" } });
      return;
    }
    known.want.priority = Math.max(known.want.priority, priority);
    known.want.forced ||= pin === "forced";
  }

  /**
   * Ends the frame's requests (see the module's remarks): pins the wanted resident pages, stores
   * the arrivals, and asks for the most wanted missing pages; the requests are then cleared.
   */
  update(): void {
    this.#retain();
    const arrivals = this.#arrived.toSorted(
      (a, b) =>
        (this.#wanted.get(keyOf(b.key))?.want.priority ?? -1) -
        (this.#wanted.get(keyOf(a.key))?.want.priority ?? -1),
    );
    this.#arrived = [];
    for (const { key, page } of arrivals) {
      const stored = this.pool.insert(key.mesh, page);
      if (stored.kind === "stored") {
        this.#counts.stored += 1;
        this.#counts.evicted += stored.evicted.length;
        // A page wanted this frame is drawn this frame: pinned before the next arrival.
        this.#retain();
      } else {
        this.#counts.refused += 1;
      }
    }
    const missing = [...this.#wanted.values()]
      .filter(
        ({ key }) =>
          this.#urls.has(key.mesh) &&
          this.pool.slotOf(key.mesh, key.page) === undefined &&
          !this.#inFlight.has(keyOf(key)),
      )
      .toSorted((a, b) => b.want.priority - a.want.priority || a.key.page - b.key.page);
    // No more than the page slots an arrival could take: the free and the unpinned, so that a pool
    // whose every slot is pinned asks for nothing it would refuse.
    const room = this.pool.layout.pageSlots - this.pool.pressure().pinned - this.#inFlight.size;
    const fetches = Math.min(MESH_FETCHES_IN_FLIGHT - this.#inFlight.size, room);
    for (const { key } of missing.slice(0, Math.max(0, fetches))) {
      this.#fetch(key);
    }
    this.#wanted = new Map();
  }

  answer(key: MeshPageKey): ResidencyAnswer {
    const slot = this.pool.slotOf(key.mesh, key.page);
    return slot === undefined
      ? { kind: "missing", standIn: this.#standIn(key) }
      : { kind: "resident", slot };
  }

  /** The residency's counters since it was made. */
  counts(): ResidencyCounts {
    return { ...this.#counts };
  }

  /** Whether a fetch is still in flight. */
  get fetching(): boolean {
    return this.#inFlight.size > 0;
  }

  /** Stops the producer; the pool is its owner's to dispose. */
  dispose(): void {
    this.#disposed = true;
    this.#producer.dispose();
  }

  /** Pins the frame's resident wanted pages. */
  #retain(): void {
    const drawn = new Set<number>();
    const forced = new Set<number>();
    for (const { key, want } of this.#wanted.values()) {
      const slot = this.pool.slotOf(key.mesh, key.page);
      if (slot !== undefined) {
        (want.forced ? forced : drawn).add(slot);
      }
    }
    this.pool.retain(drawn, forced);
  }

  /** Asks the producer for `key`'s page; it arrives at a later {@link MeshResidency.update}. */
  #fetch(key: MeshPageKey): void {
    const url = this.#urls.get(key.mesh);
    const entry = this.pool.index(key.mesh)?.pages[key.page];
    if (url === undefined || entry === undefined) {
      return;
    }
    const at = keyOf(key);
    this.#inFlight.add(at);
    this.#counts.fetched += 1;
    void this.#arrive(key, url, entry).catch((error: unknown) => {
      this.#inFlight.delete(at);
      this.#counts.failed += 1;
      console.warn(`mesh page ${at} of ${url} failed`, error);
    });
  }

  /** Waits for `key`'s page and holds it for the next {@link MeshResidency.update}. */
  async #arrive(key: MeshPageKey, url: string, entry: MeshPageEntry): Promise<void> {
    const read = await this.#producer.page(url, entry, key.page);
    const at = keyOf(key);
    this.#inFlight.delete(at);
    if (this.#disposed) {
      return;
    }
    if (read.kind === "refused") {
      this.#counts.failed += 1;
      console.warn(`mesh page ${at} of ${url} refused: ${read.reason}`);
      return;
    }
    this.#arrived.push({ key, page: read.value });
  }

  /**
   * The nearest page coarser than `key`'s that stands in for it: up the groups whose children its
   * page holds to the pages holding what they made, breadth first, to the first resident one that
   * holds a cluster the cut can draw ({@link GeometryPagePool.canDraw}); `null` where none does.
   */
  #standIn(key: MeshPageKey): number | null {
    const index = this.pool.index(key.mesh);
    const childOf = this.#childOf.get(key.mesh);
    if (index === undefined || childOf === undefined) {
      return null;
    }
    const seen = new Set<number>([key.page]);
    let frontier = [key.page];
    while (frontier.length > 0) {
      const next: number[] = [];
      const found: number[] = [];
      for (const page of frontier) {
        for (const group of childOf[page] ?? []) {
          for (const parent of index.groups[group]?.parents ?? []) {
            if (seen.has(parent)) {
              continue;
            }
            seen.add(parent);
            const slot = this.pool.slotOf(key.mesh, parent);
            if (slot !== undefined && this.pool.canDraw(key.mesh, parent)) {
              found.push(slot);
            } else {
              next.push(parent);
            }
          }
        }
      }
      if (found.length > 0) {
        return Math.min(...found);
      }
      frontier = next;
    }
    return null;
  }
}

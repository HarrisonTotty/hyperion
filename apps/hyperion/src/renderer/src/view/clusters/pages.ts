/**
 * The mesh page pool (plan R14, R14.T5.b, Design note 14): fixed slots of mesh pages on the GPU,
 * least recently used among the unpinned, with the patch cache's pins, and the record rewrites
 * that keep every residency state's DAG cut whole.
 *
 * @remarks
 * - **Slots**: the pool's bytes (`GeometrySettings.pageBytes`) hold
 *   {@link ROOT_POOL_SHARE} of root slots of `ROOT_PAGE_BYTES` and the rest page slots of
 *   `MAX_PAGE_BYTES`, each slot with its {@link MAX_PAGE_CLUSTERS} records and LOD spheres in the
 *   pool's record buffers, so that all three buffers together stay within the pool's bytes
 *   ({@link pagePoolLayout}). A slot's index is its records' page, which the cull's instances
 *   name; its page's bytes start at {@link pageSlotOffsetBytes}. A root page larger than a root
 *   slot takes as many consecutive root slots as it needs, its records in the first.
 * - **Pins**: a mesh's root pages are pinned while it is in the pool, so that some cut is always
 *   resident; roots past the root slots are refused and {@link PagePoolPressure.rootsRefused}
 *   says so for the annunciation. A page in the last retained draw or forced region is never
 *   evicted; an arrival that finds only pinned pages is refused.
 * - **Never a hole**: a group is open once every page holding its children is resident and every
 *   cluster it made is reachable; a cluster is reachable if it is a root or its consuming group is
 *   open. A reachable cluster whose generating group is not open stands in for its children, its
 *   own error written 0 so that it draws wherever its parent's error passes the tolerance; a
 *   cluster that is not reachable has its parent error written 0, so that it never draws. So at
 *   every residency state the cut over the resident records is one cut of the DAG: where a group
 *   is missing its parents draw in its place, and its resident children do not.
 * - **Eviction**: least recently used among the unpinned page slots, the finest page first among
 *   those used as recently, together with every page its leaving leaves with no reachable
 *   cluster (a group's pages leave together); a victim whose leaving would take a pinned page is
 *   passed over. An evicted slot's records are written as no cluster (a negative radius).
 * - The pool keeps no copy of a page's bytes, only its records and cluster table (lean, Design
 *   note 15): after a device loss its owner makes a new pool, which fetches again.
 */

import type { BufferHandle, RenderEngine } from "../engine/types";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import {
  MAX_PAGE_BYTES,
  MAX_PAGE_CLUSTERS,
  type MeshIndex,
  type MeshPage,
  NO_GROUP,
  PAGE_LOD_BYTES,
  PAGE_RECORD_BYTES,
  ROOT_PAGE_BYTES,
} from "./pageFormat";

/** The share of the pool's bytes its root slots take: Design note 14's 10% (lean). */
export const ROOT_POOL_SHARE = 0.1;

/** Bytes of a slot's records and LOD spheres. */
const SLOT_RECORD_BYTES = MAX_PAGE_CLUSTERS * (PAGE_RECORD_BYTES + PAGE_LOD_BYTES);

/** What the pool asks of the engine. */
export type GeometryPagePoolEngine = Pick<
  RenderEngine,
  "createBuffer" | "writeBuffer" | "releaseBuffer"
>;

/** The pool's slots and bytes (see the module's remarks). */
export interface PagePoolLayout {
  /** Root slots, slots 0 to `rootSlots` − 1, of `ROOT_PAGE_BYTES`. */
  readonly rootSlots: number;
  /** Page slots, from `rootSlots`, of `MAX_PAGE_BYTES`. */
  readonly pageSlots: number;
  /** Bytes of the pages' buffer and of the records' and LOD spheres' buffers. */
  readonly pagesBytes: number;
  readonly recordsBytes: number;
  readonly lodsBytes: number;
}

/**
 * The slots a pool of `poolBytes` holds (see the module's remarks): its root slots within
 * {@link ROOT_POOL_SHARE} of it and its page slots within the rest, each with its records.
 */
export function pagePoolLayout(poolBytes: number): PagePoolLayout {
  const rootSlots = Math.floor(
    (ROOT_POOL_SHARE * poolBytes) / (ROOT_PAGE_BYTES + SLOT_RECORD_BYTES),
  );
  const pageSlots = Math.max(
    0,
    Math.floor(
      (poolBytes - rootSlots * (ROOT_PAGE_BYTES + SLOT_RECORD_BYTES)) /
        (MAX_PAGE_BYTES + SLOT_RECORD_BYTES),
    ),
  );
  return pagePoolSlots(rootSlots, pageSlots);
}

/** The layout of `rootSlots` root slots and `pageSlots` page slots, and their buffers' bytes. */
export function pagePoolSlots(rootSlots: number, pageSlots: number): PagePoolLayout {
  const slots = rootSlots + pageSlots;
  return {
    rootSlots,
    pageSlots,
    pagesBytes: rootSlots * ROOT_PAGE_BYTES + pageSlots * MAX_PAGE_BYTES,
    recordsBytes: slots * MAX_PAGE_CLUSTERS * PAGE_RECORD_BYTES,
    lodsBytes: slots * MAX_PAGE_CLUSTERS * PAGE_LOD_BYTES,
  };
}

/** Where slot `slot`'s page starts in the pages' buffer, bytes. */
export function pageSlotOffsetBytes(layout: PagePoolLayout, slot: number): number {
  return slot < layout.rootSlots
    ? slot * ROOT_PAGE_BYTES
    : layout.rootSlots * ROOT_PAGE_BYTES + (slot - layout.rootSlots) * MAX_PAGE_BYTES;
}

/** A mesh's page, as the pool and residency key it. */
export interface MeshPageKey {
  readonly mesh: number;
  readonly page: number;
}

/** What {@link GeometryPagePool.insert} did. */
export type PoolInsert =
  /** The page holds `slot`, and the pages `evicted` left to make room. */
  | {
      readonly kind: "stored";
      readonly slot: number;
      readonly evicted: ReadonlyArray<MeshPageKey>;
    }
  /**
   * The page was not stored: its mesh is not in the pool (`mesh`), a root found no root slots
   * (`roots`), or every page slot is pinned or holds a page a pinned one needs (`pinned`).
   */
  | { readonly kind: "refused"; readonly reason: "mesh" | "roots" | "pinned" };

/** The pins against the slots, which the annunciation and the metrics read. */
export interface PagePoolPressure {
  readonly rootSlotsUsed: number;
  readonly pageSlotsUsed: number;
  /** Page slots pinned by the last retain, drawn or forced. */
  readonly pinned: number;
  /** Root pages refused since the pool was made: roots past {@link ROOT_POOL_SHARE}. */
  readonly rootsRefused: number;
  /** Other pages refused since the pool was made, every slot pinned. */
  readonly pagesRefused: number;
}

/** A page in its slot. */
interface Resident {
  readonly mesh: number;
  readonly page: number;
  readonly root: boolean;
  /** The root slots it spans beyond its own, for a root page larger than one. */
  readonly span: number;
  /** Its record section, as fetched. */
  readonly records: Uint8Array;
  readonly generatingGroups: Uint32Array;
  readonly consumingGroups: Uint32Array;
  /** Each cluster's (own, parent) errors as last written, two a cluster. */
  readonly written: Float32Array;
  lastUsed: number;
  drawn: boolean;
  forced: boolean;
}

/** A mesh in the pool: its index and its resident pages' slots. */
interface PoolMesh {
  readonly index: MeshIndex;
  readonly slots: Map<number, number>;
}

/**
 * The mesh page pool (see the module's remarks).
 *
 * @remarks
 * It makes its three buffers, category `geometry-pages`, when constructed, and releases them in
 * {@link GeometryPagePool.dispose}.
 */
export class GeometryPagePool {
  readonly layout: PagePoolLayout;
  /** The pages' bytes, slot by slot ({@link pageSlotOffsetBytes}). */
  readonly pages: BufferHandle;
  /** Each slot's {@link MAX_PAGE_CLUSTERS} records, as the cull reads them. */
  readonly records: BufferHandle;
  /** Each record's LOD sphere, the cull's `lods`. */
  readonly lods: BufferHandle;
  readonly #engine: GeometryPagePoolEngine;
  readonly #slots: Array<Resident | "spanned" | null>;
  readonly #meshes = new Map<number, PoolMesh>();
  #nextMesh = 0;
  #frame = 0;
  #rootsRefused = 0;
  #pagesRefused = 0;

  /**
   * @param poolBytesOrSlots - The pool's bytes, all three buffers together
   *   (`GeometrySettings.pageBytes`), or its slots ({@link pagePoolSlots}).
   */
  constructor(engine: GeometryPagePoolEngine, poolBytesOrSlots: number | PagePoolLayout) {
    this.#engine = engine;
    this.layout =
      typeof poolBytesOrSlots === "number" ? pagePoolLayout(poolBytesOrSlots) : poolBytesOrSlots;
    const usage = BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST;
    const made = (name: string, bytes: number): BufferHandle =>
      engine.createBuffer({ name, bytes: Math.max(16, bytes), usage, category: "geometry-pages" });
    this.pages = made("geometry pages", this.layout.pagesBytes);
    this.records = made("geometry page records", this.layout.recordsBytes);
    this.lods = made("geometry page lods", this.layout.lodsBytes);
    this.#slots = Array.from({ length: this.layout.rootSlots + this.layout.pageSlots }, () => null);
  }

  /** Adds a mesh by its index, nothing of it resident; its handle names it to the pool. */
  addMesh(index: MeshIndex): number {
    const mesh = this.#nextMesh;
    this.#nextMesh += 1;
    this.#meshes.set(mesh, { index, slots: new Map() });
    return mesh;
  }

  /** Removes a mesh and frees every slot its pages hold, its roots' with them. */
  removeMesh(mesh: number): void {
    const held = this.#meshes.get(mesh);
    if (held === undefined) {
      return;
    }
    // A Map's iteration passes over the entries deleted from it as it goes.
    for (const slot of held.slots.values()) {
      this.#free(slot);
    }
    this.#meshes.delete(mesh);
  }

  /** The mesh's index, or `undefined` for one not in the pool. */
  index(mesh: number): MeshIndex | undefined {
    return this.#meshes.get(mesh)?.index;
  }

  /** The slot holding `page` of `mesh`, or `undefined` where it is not resident. */
  slotOf(mesh: number, page: number): number | undefined {
    return this.#meshes.get(mesh)?.slots.get(page);
  }

  /**
   * Whether `page` of `mesh` is resident and holds a cluster the cut can draw: a root, or one
   * whose consuming group is open (see the module's remarks).
   */
  canDraw(mesh: number, page: number): boolean {
    const held = this.#meshes.get(mesh);
    const slot = held?.slots.get(page);
    return held !== undefined && slot !== undefined && this.#reach(held, null).anyReachable(slot);
  }

  /** The resident pages of `mesh` by page, with their slots, in page order. */
  residentPages(mesh: number): ReadonlyArray<readonly [page: number, slot: number]> {
    const held = this.#meshes.get(mesh);
    return held === undefined ? [] : [...held.slots.entries()].toSorted((a, b) => a[0] - b[0]);
  }

  /**
   * Stores `page` of `mesh` (see the module's remarks), evicting where it must, and writes the
   * records its arrival changes; a page already resident is stored where it is.
   */
  insert(mesh: number, page: MeshPage): PoolInsert {
    const held = this.#meshes.get(mesh);
    if (held === undefined) {
      return { kind: "refused", reason: "mesh" };
    }
    const already = held.slots.get(page.index);
    if (already !== undefined) {
      return { kind: "stored", slot: already, evicted: [] };
    }
    const root = page.index < held.index.rootPages;
    const span = root ? Math.ceil(page.body.byteLength / ROOT_PAGE_BYTES) : 1;
    let slot: number | null;
    let evicted: MeshPageKey[] = [];
    if (root) {
      slot = this.#rootRun(span);
      if (slot === null) {
        this.#rootsRefused += 1;
        return { kind: "refused", reason: "roots" };
      }
    } else {
      slot = this.#freePageSlot();
      if (slot === null) {
        const victim = this.#victim();
        if (victim === null) {
          this.#pagesRefused += 1;
          return { kind: "refused", reason: "pinned" };
        }
        evicted = victim.map((s) => this.#evict(s));
        slot = this.#freePageSlot();
        if (slot === null) {
          throw new Error("an eviction freed no page slot");
        }
      }
    }
    const records = new Uint8Array(
      page.body,
      page.sections.records,
      page.clusters * PAGE_RECORD_BYTES,
    ).slice();
    const resident: Resident = {
      mesh,
      page: page.index,
      root,
      span,
      records,
      generatingGroups: page.generatingGroups,
      consumingGroups: page.consumingGroups,
      written: new Float32Array(2 * page.clusters).fill(Number.NaN),
      lastUsed: this.#frame,
      drawn: false,
      forced: false,
    };
    this.#slots[slot] = resident;
    for (let k = 1; k < span; k += 1) {
      this.#slots[slot + k] = "spanned";
    }
    held.slots.set(page.index, slot);
    this.#engine.writeBuffer(
      this.pages,
      pageSlotOffsetBytes(this.layout, slot),
      new Uint8Array(page.body),
    );
    this.#engine.writeBuffer(
      this.lods,
      slot * MAX_PAGE_CLUSTERS * PAGE_LOD_BYTES,
      new Uint8Array(page.body, page.sections.lods, page.clusters * PAGE_LOD_BYTES),
    );
    // The slot's records past its clusters hold none, so that a page of fewer clusters than the
    // last one here draws nothing of it.
    const empty = new DataView(
      new ArrayBuffer((MAX_PAGE_CLUSTERS - page.clusters) * PAGE_RECORD_BYTES),
    );
    for (let c = 0; c < MAX_PAGE_CLUSTERS - page.clusters; c += 1) {
      empty.setFloat32(c * PAGE_RECORD_BYTES + 12, -1, true);
    }
    if (empty.byteLength > 0) {
      this.#engine.writeBuffer(
        this.records,
        (slot * MAX_PAGE_CLUSTERS + page.clusters) * PAGE_RECORD_BYTES,
        new Uint8Array(empty.buffer),
      );
    }
    this.#rewrite(mesh);
    return { kind: "stored", slot, evicted };
  }

  /**
   * Pins the slots of this frame's draw and forced region against eviction until the next retain,
   * and marks them used now (see the module's remarks).
   */
  retain(drawn: ReadonlySet<number>, forced: ReadonlySet<number>): void {
    this.#frame += 1;
    this.#slots.forEach((resident, slot) => {
      if (resident === null || resident === "spanned") {
        return;
      }
      resident.drawn = drawn.has(slot);
      resident.forced = forced.has(slot);
      if (resident.drawn || resident.forced) {
        resident.lastUsed = this.#frame;
      }
    });
  }

  /** The pins against the slots (see {@link PagePoolPressure}). */
  pressure(): PagePoolPressure {
    let rootSlotsUsed = 0;
    let pageSlotsUsed = 0;
    let pinned = 0;
    this.#slots.forEach((resident, slot) => {
      if (resident === null) {
        return;
      }
      if (slot < this.layout.rootSlots) {
        rootSlotsUsed += 1;
        return;
      }
      pageSlotsUsed += 1;
      if (resident !== "spanned" && (resident.drawn || resident.forced)) {
        pinned += 1;
      }
    });
    return {
      rootSlotsUsed,
      pageSlotsUsed,
      pinned,
      rootsRefused: this.#rootsRefused,
      pagesRefused: this.#pagesRefused,
    };
  }

  /**
   * Each resident cluster's (own, parent) errors as the pool writes them for `mesh` (see the
   * module's remarks), by slot: two a cluster.
   */
  writtenErrors(mesh: number): ReadonlyMap<number, Float32Array> {
    const out = new Map<number, Float32Array>();
    for (const slot of this.#meshes.get(mesh)?.slots.values() ?? []) {
      const resident = this.#slots[slot];
      if (resident !== null && resident !== undefined && resident !== "spanned") {
        out.set(slot, resident.written.slice());
      }
    }
    return out;
  }

  /** Releases the pool's buffers; nothing is resident after. */
  dispose(): void {
    this.#engine.releaseBuffer(this.pages);
    this.#engine.releaseBuffer(this.records);
    this.#engine.releaseBuffer(this.lods);
    this.#slots.fill(null);
    this.#meshes.clear();
  }

  /** The first run of `span` free root slots, or `null`. */
  #rootRun(span: number): number | null {
    for (let first = 0; first + span <= this.layout.rootSlots; first += 1) {
      let free = true;
      for (let k = 0; k < span && free; k += 1) {
        free = this.#slots[first + k] === null;
      }
      if (free) {
        return first;
      }
    }
    return null;
  }

  /** The first free page slot, or `null`. */
  #freePageSlot(): number | null {
    for (let slot = this.layout.rootSlots; slot < this.#slots.length; slot += 1) {
      if (this.#slots[slot] === null) {
        return slot;
      }
    }
    return null;
  }

  /**
   * The slots to evict for one page slot: the least recently used unpinned page slot, the finest
   * page first among those used as recently, with the pages its leaving strands; `null` if every
   * candidate is pinned or strands a pinned page.
   */
  #victim(): number[] | null {
    const candidates: Array<{ slot: number; resident: Resident }> = [];
    this.#slots.forEach((resident, slot) => {
      if (
        slot >= this.layout.rootSlots &&
        resident !== null &&
        resident !== "spanned" &&
        !resident.drawn &&
        !resident.forced
      ) {
        candidates.push({ slot, resident });
      }
    });
    candidates.sort(
      (a, b) => a.resident.lastUsed - b.resident.lastUsed || b.resident.page - a.resident.page,
    );
    for (const { slot, resident } of candidates) {
      const stranded = this.#stranded(resident.mesh, slot);
      if (stranded.every((s) => !this.#pinned(s))) {
        return [slot, ...stranded];
      }
    }
    return null;
  }

  /** Whether `slot` holds a page pinned by the last retain. */
  #pinned(slot: number): boolean {
    const resident = this.#slots[slot];
    return resident !== null && resident !== undefined && resident !== "spanned"
      ? resident.root || resident.drawn || resident.forced
      : false;
  }

  /**
   * The slots of `mesh`'s pages, other than `leaving`'s, that hold a reachable cluster now and
   * would hold none once `leaving`'s page has left.
   */
  #stranded(mesh: number, leaving: number): number[] {
    const held = this.#meshes.get(mesh);
    if (held === undefined) {
      return [];
    }
    const now = this.#reach(held, null);
    const after = this.#reach(held, leaving);
    const out: number[] = [];
    for (const slot of held.slots.values()) {
      if (slot !== leaving && now.anyReachable(slot) && !after.anyReachable(slot)) {
        out.push(slot);
      }
    }
    return out;
  }

  /** Evicts `slot`'s page: its slot freed, its records written as no cluster. */
  #evict(slot: number): MeshPageKey {
    const resident = this.#slots[slot];
    if (resident === null || resident === undefined || resident === "spanned") {
      throw new Error(`slot ${String(slot)} holds no page to evict`);
    }
    this.#free(slot);
    this.#rewrite(resident.mesh);
    return { mesh: resident.mesh, page: resident.page };
  }

  /** Frees `slot` and any root slots its page spans, its records written as no cluster. */
  #free(slot: number): void {
    const resident = this.#slots[slot];
    if (resident === null || resident === undefined || resident === "spanned") {
      return;
    }
    for (let k = 0; k < resident.span; k += 1) {
      this.#slots[slot + k] = null;
    }
    this.#meshes.get(resident.mesh)?.slots.delete(resident.page);
    const none = new DataView(new ArrayBuffer(MAX_PAGE_CLUSTERS * PAGE_RECORD_BYTES));
    for (let c = 0; c < MAX_PAGE_CLUSTERS; c += 1) {
      none.setFloat32(c * PAGE_RECORD_BYTES + 12, -1, true);
    }
    this.#engine.writeBuffer(
      this.records,
      slot * MAX_PAGE_CLUSTERS * PAGE_RECORD_BYTES,
      new Uint8Array(none.buffer),
    );
  }

  /**
   * Which of `held`'s groups are open and its clusters reachable, `without` the page in that slot
   * (see the module's remarks).
   */
  #reach(
    held: PoolMesh,
    without: number | null,
  ): { open: (group: number) => boolean; anyReachable: (slot: number) => boolean } {
    const resident = (page: number): Resident | null => {
      const slot = held.slots.get(page);
      const r = slot === undefined || slot === without ? null : this.#slots[slot];
      return r === null || r === undefined || r === "spanned" ? null : r;
    };
    const memo = new Map<number, boolean>();
    const open = (group: number): boolean => {
      const known = memo.get(group);
      if (known !== undefined) {
        return known;
      }
      // A group in progress is closed: the builder's DAG has no cycle, so only a malformed file
      // reaches back.
      memo.set(group, false);
      const entry = held.index.groups[group];
      let result =
        entry !== undefined &&
        entry.children.every((page) => resident(page) !== null) &&
        entry.parents.every((page) => resident(page) !== null);
      for (const page of result && entry !== undefined ? entry.parents : []) {
        const r = resident(page);
        if (r === null) {
          result = false;
          break;
        }
        for (let c = 0; c < r.generatingGroups.length && result; c += 1) {
          const consumer = r.consumingGroups[c] ?? NO_GROUP;
          if (r.generatingGroups[c] === group && consumer !== NO_GROUP && !open(consumer)) {
            result = false;
          }
        }
      }
      memo.set(group, result);
      return result;
    };
    const anyReachable = (slot: number): boolean => {
      const r = slot === without ? null : this.#slots[slot];
      if (r === null || r === undefined || r === "spanned") {
        return false;
      }
      for (const consumer of r.consumingGroups) {
        if (consumer === NO_GROUP || open(consumer)) {
          return true;
        }
      }
      return false;
    };
    return { open, anyReachable };
  }

  /** Writes the records of `mesh`'s resident pages whose errors its residency now changes. */
  #rewrite(mesh: number): void {
    const held = this.#meshes.get(mesh);
    if (held === undefined) {
      return;
    }
    const { open } = this.#reach(held, null);
    for (const slot of held.slots.values()) {
      const r = this.#slots[slot];
      if (r === null || r === undefined || r === "spanned") {
        continue;
      }
      const source = new DataView(r.records.buffer, r.records.byteOffset, r.records.byteLength);
      const errors = new Float32Array(r.written.length);
      for (let c = 0; c < r.consumingGroups.length; c += 1) {
        const own = source.getFloat32(c * PAGE_RECORD_BYTES + 16, true);
        const parent = source.getFloat32(c * PAGE_RECORD_BYTES + 20, true);
        const consumer = r.consumingGroups[c] ?? NO_GROUP;
        const generator = r.generatingGroups[c] ?? NO_GROUP;
        const reachable = consumer === NO_GROUP || open(consumer);
        errors[2 * c] = reachable && generator !== NO_GROUP && !open(generator) ? 0 : own;
        errors[2 * c + 1] = reachable ? parent : 0;
      }
      if (errors.every((e, k) => Object.is(e, r.written[k]))) {
        continue;
      }
      r.written.set(errors);
      const block = r.records.slice();
      const out = new DataView(block.buffer);
      for (let c = 0; c < r.consumingGroups.length; c += 1) {
        out.setFloat32(c * PAGE_RECORD_BYTES + 16, errors[2 * c] ?? 0, true);
        out.setFloat32(c * PAGE_RECORD_BYTES + 20, errors[2 * c + 1] ?? 0, true);
      }
      this.#engine.writeBuffer(this.records, slot * MAX_PAGE_CLUSTERS * PAGE_RECORD_BYTES, block);
    }
  }
}

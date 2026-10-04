/**
 * The patch cache and the draw set (plan R05, T8, Design notes 7 and 10).
 *
 * @remarks
 * One cache per body, keyed by patch, not by view, so that views near each other share patches.
 * The cache is a fixed number of slots from its {@link SlotLayout}; it assigns slots and keeps the
 * eviction order, and the terrain pass (T11) writes each patch's bytes into the GPU buffers at the
 * slot it is given. Eviction is least recently used among unpinned patches. Two pins: a patch in a
 * grounded body's forced region is never evicted, whatever the budget; a patch in the current draw
 * set, the selection or the selection's resident ancestors is evicted only after everything
 * unpinned, and among patches used as recently the deepest goes first. The baked patches the
 * selection found hidden ({@link Selection.hiddenBaked}) are not pinned, but are used each frame
 * they are hidden, and among the unpinned patches used as recently they go last. The six roots are
 * never evicted either, so that once baked every selected patch has a resident ancestor to stand in
 * for it.
 *
 * {@link DrawSetResolver} (and {@link resolveDrawSet}) is what sees both the selection and the
 * cache: the selection stays a pure function of its inputs (Design note 7).
 */

import type { BodyFixedVec3 } from "./planet";
import type { SlotLayout } from "./slotLayout";
import type { HeightRangeLookup, Selection } from "./select";
import { ancestorIndex, MAX_LEVEL, type PatchKey, patchKeyIndex, patchKeyString } from "./patchKey";

/** What the cache keeps of a baked patch besides the bytes in its slot. */
export interface ResidentPatch {
  readonly key: PatchKey;
  /** The bake's generation tag (Design note 11). */
  readonly generation: number;
  /** The patch origin the offsets are relative to, body-fixed metres. */
  readonly originM: BodyFixedVec3;
  /** The baked heights' range above the datum, metres. */
  readonly heightRangeM: readonly [number, number];
  /** The bounding radius about {@link ResidentPatch.originM}, metres. */
  readonly boundingRadiusM: number;
}

/** A patch held in the cache, with its slot. */
export interface CachedPatch extends ResidentPatch {
  /** The slot index, from 0 to the layout's slot count less one. */
  readonly slot: number;
  /** Its `patchKeyString`, made once when it is stored. */
  readonly keyString: string;
}

/** What {@link PatchCache.insert} did. */
export type CacheInsert =
  /** The patch holds `slot`, having evicted the patch keyed `evicted` if it is not `null`. */
  | { readonly kind: "stored"; readonly slot: number; readonly evicted: string | null }
  /** Every slot is pinned by the forced region or a root, so nothing could be evicted. */
  | { readonly kind: "refused" };

/** The pins against the slots, which the metrics and the annunciation read. */
export interface CachePressure {
  /** Patches of the last retained selection in the forced region, resident or not. */
  readonly forced: number;
  /**
   * Resident patches pinned by the last retained selection, its ancestors or the draw set, outside
   * the forced region.
   */
  readonly drawn: number;
  /**
   * Whether the pins exceeded the slots at the last retain, or an insert has been refused since the
   * one before it.
   */
  readonly exceeded: boolean;
}

interface Entry {
  patch: CachedPatch;
  lastUsed: number;
  forced: boolean;
  drawn: boolean;
  /** One of the last retained selection's {@link Selection.hiddenBaked}, neither forced nor drawn. */
  hidden: boolean;
}

/** The leaves' maps per level, by {@link patchKeyIndex}. */
function levelMaps<T>(): Map<number, T>[] {
  return Array.from({ length: MAX_LEVEL + 1 }, () => new Map<number, T>());
}

/**
 * The fixed-slot patch cache of one body.
 *
 * @remarks
 * Each frame the terrain pass inserts the bakes that arrived, resolves the draw set with
 * {@link resolveDrawSet}, and then calls {@link PatchCache.retain} with the selection and the draw
 * set, which sets the pins and the recency for the next frame's evictions. A patch inserted between
 * two retains takes the pins the last retained selection gives its key, so a forced patch is pinned
 * from the moment it arrives, and a patch it demanded is draw-pinned (an ancestor asked for its
 * unbaked descendants, which the next retain pins as an ancestor).
 */
export class PatchCache implements HeightRangeLookup {
  readonly layout: SlotLayout;
  private readonly entries = new Map<string, Entry>();
  /** The same entries per level by {@link patchKeyIndex}, for lookups that build no string. */
  private readonly byIndex: Map<number, Entry>[] = levelMaps();
  private readonly freeSlots: number[] = [];
  /**
   * The last retained selection's forced, selected and demanded keys, cleared and refilled each
   * retain.
   */
  private readonly forcedKeys = new Set<string>();
  private readonly selectedKeys = new Set<string>();
  private readonly demandedKeys = new Set<string>();
  /** The resident patches pinned by the last retain outside the forced region. */
  private pinnedDrawn = 0;
  private tick = 0;
  /** An insert was refused since the last retain. */
  private refused = false;
  /** The last retain found the pins beyond the slots, or a refusal before it. */
  private exceededAtRetain = false;

  constructor(layout: SlotLayout) {
    this.layout = layout;
    // Popped from the end, so slot 0 is handed out first.
    for (let slot = layout.slotCount - 1; slot >= 0; slot -= 1) {
      this.freeSlots.push(slot);
    }
  }

  /** The number of slots, fixed for the cache's life. */
  get slotCount(): number {
    return this.layout.slotCount;
  }

  /** The number of slots holding a patch. */
  get usedSlots(): number {
    return this.entries.size;
  }

  /**
   * The GPU bytes the resident patches' data take, slots in use × bytes a slot; the slots
   * themselves are allocated whole, `slotCount × bytesPerSlot`, by the terrain pass.
   */
  get heldBytes(): number {
    return this.entries.size * this.layout.bytesPerSlot;
  }

  /** The cached patch keyed by `keyString` (a `patchKeyString`), if it is resident. */
  get(keyString: string): CachedPatch | undefined {
    return this.entries.get(keyString)?.patch;
  }

  /**
   * The cached patch at `level` with index `index` ({@link patchKeyIndex}), if it is resident: the
   * draw set's lookup, which builds no string.
   */
  residentAt(level: number, index: number): CachedPatch | undefined {
    return this.byIndex[level]?.get(index)?.patch;
  }

  /** Whether the patch keyed by `keyString` is resident. */
  has(keyString: string): boolean {
    return this.entries.has(keyString);
  }

  /**
   * The baked height range of a resident patch, metres, or `undefined` if it is not resident:
   * selection's {@link HeightRangeLookup}, through which baked ancestors tighten bounds.
   */
  heightRangeM(key: PatchKey): readonly [number, number] | undefined {
    return this.byIndex[key.level]?.get(patchKeyIndex(key))?.patch.heightRangeM;
  }

  /**
   * Stores a baked patch, in its own slot if it is already resident (a newer bake), else in a free
   * slot, else in the slot of the evicted patch: the least recently used unpinned one, the hidden
   * baked patches last among those used as recently, then the least recently used of those the
   * selection, its ancestors or the draw set pins; the deepest first among patches used as
   * recently. A forced or root patch is never evicted.
   */
  insert(patch: ResidentPatch): CacheInsert {
    const keyString = patchKeyString(patch.key);
    const present = this.entries.get(keyString);
    if (present !== undefined) {
      present.patch = { ...patch, slot: present.patch.slot, keyString };
      present.lastUsed = this.tick;
      return { kind: "stored", slot: present.patch.slot, evicted: null };
    }
    let slot = this.freeSlots.pop();
    let evicted: string | null = null;
    if (slot === undefined) {
      evicted = this.victim();
      if (evicted === null) {
        this.refused = true;
        return { kind: "refused" };
      }
      const victim = this.entries.get(evicted);
      if (victim === undefined) {
        throw new Error(`the eviction victim ${evicted} is not resident`);
      }
      slot = victim.patch.slot;
      this.forget(evicted, victim);
    }
    const forced = this.forcedKeys.has(keyString);
    const entry: Entry = {
      patch: { ...patch, slot, keyString },
      lastUsed: this.tick,
      forced,
      drawn: !forced && (this.selectedKeys.has(keyString) || this.demandedKeys.has(keyString)),
      hidden: false,
    };
    this.entries.set(keyString, entry);
    this.byIndex[patch.key.level]?.set(patchKeyIndex(patch.key), entry);
    return { kind: "stored", slot, evicted };
  }

  /** Drops a resident patch and frees its slot, returning whether it was resident. */
  remove(keyString: string): boolean {
    const entry = this.entries.get(keyString);
    if (entry === undefined) {
      return false;
    }
    this.forget(keyString, entry);
    this.freeSlots.push(entry.patch.slot);
    return true;
  }

  private forget(keyString: string, entry: Entry): void {
    this.entries.delete(keyString);
    this.byIndex[entry.patch.key.level]?.delete(patchKeyIndex(entry.patch.key));
  }

  /**
   * Sets the pins from this frame's selection and draw set and marks their patches as used.
   *
   * @remarks
   * Forced pins are the patches the selection marks `forced`. Draw pins are the patches drawn;
   * every other selected patch that is resident, so that the siblings an ancestor stands in for
   * are kept until all are resident; every resident ancestor of a selected patch, whose baked
   * range the selection's bounds read (the streaming gate) and which stands in for its
   * descendants. The patches the selection demanded are draw-pinned as they arrive. The baked
   * patches the selection found hidden ({@link Selection.hiddenBaked}), whose ranges its culling
   * read, are not pinned: they are marked as used, and go last among the unpinned patches used as
   * recently. All are replaced, not accumulated, so a patch leaving the selection and the draw set
   * becomes evictable at once. The pins are reported
   * as exceeding the slots when the forced patches, resident or not, and the other pinned resident
   * patches outnumber the slots, or when an insert was refused since the last retain.
   */
  retain(selection: Selection, draw: DrawSet): void {
    this.tick += 1;
    const forcedKeys = this.forcedKeys;
    const selectedKeys = this.selectedKeys;
    forcedKeys.clear();
    selectedKeys.clear();
    this.demandedKeys.clear();
    for (const request of selection.demand) {
      this.demandedKeys.add(patchKeyString(request.key));
    }
    for (const [keyString, selected] of selection.patches) {
      selectedKeys.add(keyString);
      if (selected.forced) {
        forcedKeys.add(keyString);
      }
    }
    for (const [keyString, entry] of this.entries) {
      entry.forced = forcedKeys.has(keyString);
      entry.drawn = !entry.forced && selectedKeys.has(keyString);
      entry.hidden = false;
      if (selectedKeys.has(keyString)) {
        entry.lastUsed = this.tick;
      }
    }
    // The resident ancestors of the selection hold the bounds selection refines on and stand in
    // for what is not baked: kept as the draw set is, or the selection collapses when they go.
    // Run before the draw set's touches, so that an ancestor found already touched was touched by
    // this walk, and everything above it already was too (the selection is a cut: no selected
    // patch is another's ancestor).
    for (const selected of selection.patches.values()) {
      const key = selected.key;
      for (let level = key.level - 1; level >= 0; level -= 1) {
        const entry = this.byIndex[level]?.get(ancestorIndex(key, level));
        if (entry === undefined) {
          continue;
        }
        if (entry.lastUsed === this.tick) {
          break;
        }
        entry.lastUsed = this.tick;
        entry.drawn = !entry.forced;
      }
    }
    // The baked patches the selection found hidden, whose own ranges may be what hides them: left
    // to age, they were evicted first, came back the next frame under their parents' looser
    // ranges and were drawn by those parents (R05.T8, the high-bound ruling's F3). Not pinned, so
    // that a cache whose pins fill it gives them up before the selection's own.
    for (const key of selection.hiddenBaked) {
      const entry = this.byIndex[key.level]?.get(patchKeyIndex(key));
      if (entry !== undefined) {
        entry.lastUsed = this.tick;
        entry.hidden = !entry.forced && !entry.drawn;
      }
    }
    for (let n = 0; n < draw.count; n += 1) {
      const drawn = draw.patches[n];
      const entry = drawn === undefined ? undefined : this.entries.get(drawn.keyString);
      if (entry !== undefined) {
        entry.lastUsed = this.tick;
        entry.drawn = !entry.forced;
      }
    }
    this.countPins();
    this.exceededAtRetain = this.refused || forcedKeys.size + this.pinnedDrawn > this.slotCount;
    this.refused = false;
  }

  /** The pins against the slots (Design note 10: pins beyond the slots are reported). */
  pressure(): CachePressure {
    this.countPins();
    return {
      forced: this.forcedKeys.size,
      drawn: this.pinnedDrawn,
      exceeded: this.exceededAtRetain || this.refused,
    };
  }

  private countPins(): void {
    let drawn = 0;
    for (const entry of this.entries.values()) {
      if (!entry.forced && entry.drawn) {
        drawn += 1;
      }
    }
    this.pinnedDrawn = drawn;
  }

  /** The patch to evict, or `null` if every resident patch is forced or a root. */
  private victim(): string | null {
    let best: string | null = null;
    let bestRank = 0;
    let bestUsed = 0;
    let bestLevel = 0;
    for (const [keyString, entry] of this.entries) {
      if (entry.forced || entry.patch.key.level === 0) {
        continue;
      }
      const rank = entry.drawn ? 2 : entry.hidden ? 1 : 0;
      const level = entry.patch.key.level;
      // Among patches used as recently, the deepest goes first: an ancestor holds the bounds and
      // stands in for everything beneath it, so losing one costs a whole subtree (R05.T13.a's
      // probe found the selection collapsing to the roots when ancestors went first).
      const better =
        best === null ||
        rank < bestRank ||
        (rank === bestRank &&
          (entry.lastUsed < bestUsed ||
            (entry.lastUsed === bestUsed &&
              (level > bestLevel || (level === bestLevel && keyString < best)))));
      if (better) {
        best = keyString;
        bestRank = rank;
        bestUsed = entry.lastUsed;
        bestLevel = level;
      }
    }
    return best;
  }
}

/** A patch the terrain pass draws this frame. */
export interface DrawnPatch {
  readonly keyString: string;
  readonly patch: CachedPatch;
  /** Whether it stands in for one or more selected patches not yet resident. */
  readonly standIn: boolean;
}

/**
 * What the terrain pass draws: resident patches covering the selection without overlap.
 *
 * @remarks
 * A {@link DrawSetResolver} returns the same object every call, its arrays rewritten in place, so
 * that resolving each frame allocates nothing after warm-up: read it before the next call.
 */
export interface DrawSet {
  /** How many patches are drawn: the length of {@link DrawSet.patches}, and of `slots`' prefix. */
  readonly count: number;
  /** The drawn patches, in the selection's order with each stand-in where it is first needed. */
  readonly patches: ReadonlyArray<DrawnPatch>;
  /**
   * Each drawn patch's slot index in its first {@link DrawSet.count} entries, in the same order: the
   * instanced draw's per-instance data. Its length is the cache's slot count, fixed.
   */
  readonly slots: Uint32Array;
  /**
   * How many selected patches are drawn by a coarser resident ancestor, those beneath an ancestor
   * standing in for a sibling included.
   */
  readonly standingIn: number;
  /** How many selected patches have no resident ancestor at all, and are not drawn. */
  readonly missing: number;
}

/** A drawn patch's record, reused from call to call. */
interface DrawnRecord {
  keyString: string;
  patch: CachedPatch;
  standIn: boolean;
}

/** The draw set the resolver rewrites in place. */
interface ResolvedSet {
  count: number;
  readonly patches: DrawnRecord[];
  readonly slots: Uint32Array;
  standingIn: number;
  missing: number;
}

/**
 * Resolves what to draw for one cache, each frame, with no allocation after warm-up: each selected
 * patch some view sees, if resident, else its nearest resident ancestor (Design note 7).
 *
 * @remarks
 * An ancestor standing in covers its whole area, so a resident selected patch beneath it is drawn
 * through the ancestor rather than on top of it; the drawn patches never overlap. It reads the
 * cache and changes nothing; {@link PatchCache.retain} records the frame's use. Lookups go by
 * level and {@link patchKeyIndex}, so no key or string is built; the records, the arrays and the
 * map of chosen patches are kept and reused, and no sort is run.
 */
export class DrawSetResolver {
  private readonly cache: PatchCache;
  private readonly set: ResolvedSet;
  /** The patches chosen this call, to their records. */
  private readonly chosen = new Map<CachedPatch, DrawnRecord>();
  /** Every record made so far, reused in order. */
  private readonly pool: DrawnRecord[] = [];
  private used = 0;

  constructor(cache: PatchCache) {
    this.cache = cache;
    this.set = {
      count: 0,
      patches: [],
      slots: new Uint32Array(cache.slotCount),
      standingIn: 0,
      missing: 0,
    };
  }

  /** The draw set of `selection` as the cache holds it now; the same object every call. */
  resolve(selection: Selection): DrawSet {
    const { cache, chosen, set } = this;
    chosen.clear();
    this.used = 0;
    let missing = 0;
    let unseen = 0;
    for (const selected of selection.patches.values()) {
      if (!selected.seen) {
        // A forced patch no view sees is kept resident by the pins, not drawn.
        unseen += 1;
        continue;
      }
      const key = selected.key;
      let found: CachedPatch | undefined;
      let standIn = false;
      for (let level = key.level; level >= 0; level -= 1) {
        found = cache.residentAt(level, ancestorIndex(key, level));
        if (found !== undefined) {
          standIn = level < key.level;
          break;
        }
      }
      if (found === undefined) {
        missing += 1;
        continue;
      }
      const known = chosen.get(found);
      if (known === undefined) {
        chosen.set(found, this.record(found, standIn));
      } else if (standIn) {
        known.standIn = true;
      }
    }
    let count = 0;
    let drawnAsThemselves = 0;
    for (const record of chosen.values()) {
      if (this.hasChosenAncestor(record.patch.key)) {
        continue;
      }
      set.patches[count] = record;
      set.slots[count] = record.patch.slot;
      count += 1;
      if (selection.patches.has(record.keyString)) {
        drawnAsThemselves += 1;
      }
    }
    set.patches.length = count;
    set.count = count;
    set.missing = missing;
    set.standingIn = selection.patches.size - unseen - missing - drawnAsThemselves;
    return set;
  }

  private record(patch: CachedPatch, standIn: boolean): DrawnRecord {
    let record = this.pool[this.used];
    if (record === undefined) {
      record = { keyString: patch.keyString, patch, standIn };
      this.pool.push(record);
    } else {
      record.keyString = patch.keyString;
      record.patch = patch;
      record.standIn = standIn;
    }
    this.used += 1;
    return record;
  }

  private hasChosenAncestor(key: PatchKey): boolean {
    for (let level = key.level - 1; level >= 0; level -= 1) {
      const patch = this.cache.residentAt(level, ancestorIndex(key, level));
      if (patch !== undefined && this.chosen.has(patch)) {
        return true;
      }
    }
    return false;
  }
}

/**
 * Resolves what to draw once (Design note 7), through a {@link DrawSetResolver} made for the call;
 * a terrain pass keeps one resolver per cache instead, which allocates nothing after warm-up.
 */
export function resolveDrawSet(selection: Selection, cache: PatchCache): DrawSet {
  return new DrawSetResolver(cache).resolve(selection);
}

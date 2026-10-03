/**
 * The patch cache and the draw set (plan R05, T8, Design notes 7 and 10).
 *
 * @remarks
 * One cache per body, keyed by patch, not by view, so that views near each other share patches.
 * The cache is a fixed number of slots from its {@link SlotLayout}; it assigns slots and keeps the
 * eviction order, and the terrain pass (T11) writes each patch's bytes into the GPU buffers at the
 * slot it is given. Eviction is least recently used among unpinned patches. Two pins: a patch in a
 * grounded body's forced region is never evicted, whatever the budget; a patch in the current draw
 * set is evicted only after everything unpinned. The six roots are never evicted either, so that
 * once baked every selected patch has a resident ancestor to stand in for it.
 *
 * {@link resolveDrawSet} is the one function that sees both the selection and the cache: the
 * selection stays a pure function of the views (Design note 7).
 */

import type { BodyFixedVec3 } from "./planet";
import type { SlotLayout } from "./slotLayout";
import type { HeightRangeLookup, Selection } from "./select";
import { type PatchKey, parentKey, patchKeyString } from "./patchKey";

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
  /** Resident patches pinned by the last retained selection or draw set, outside the forced region. */
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
}

/**
 * The fixed-slot patch cache of one body.
 *
 * @remarks
 * Each frame the terrain pass inserts the bakes that arrived, resolves the draw set with
 * {@link resolveDrawSet}, and then calls {@link PatchCache.retain} with the selection and the draw
 * set, which sets the pins and the recency for the next frame's evictions. A patch inserted between
 * two retains takes the pins the last retained selection gives its key, so a forced patch is pinned
 * from the moment it arrives.
 */
export class PatchCache implements HeightRangeLookup {
  readonly layout: SlotLayout;
  private readonly entries = new Map<string, Entry>();
  private readonly freeSlots: number[] = [];
  private forcedKeys: ReadonlySet<string> = new Set();
  private selectedKeys: ReadonlySet<string> = new Set();
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

  /** Whether the patch keyed by `keyString` is resident. */
  has(keyString: string): boolean {
    return this.entries.has(keyString);
  }

  /**
   * The baked height range of a resident patch, metres, or `undefined` if it is not resident:
   * selection's {@link HeightRangeLookup}, through which baked ancestors tighten bounds.
   */
  heightRangeM(key: PatchKey): readonly [number, number] | undefined {
    return this.entries.get(patchKeyString(key))?.patch.heightRangeM;
  }

  /**
   * Stores a baked patch, in its own slot if it is already resident (a newer bake), else in a free
   * slot, else in the slot of the evicted patch: the least recently used unpinned one, then the
   * least recently used of those the selection or the draw set pins. A forced or root patch is
   * never evicted.
   */
  insert(patch: ResidentPatch): CacheInsert {
    const keyString = patchKeyString(patch.key);
    const present = this.entries.get(keyString);
    if (present !== undefined) {
      present.patch = { ...patch, slot: present.patch.slot };
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
      this.entries.delete(evicted);
    }
    const forced = this.forcedKeys.has(keyString);
    this.entries.set(keyString, {
      patch: { ...patch, slot },
      lastUsed: this.tick,
      forced,
      drawn: !forced && this.selectedKeys.has(keyString),
    });
    return { kind: "stored", slot, evicted };
  }

  /** Drops a resident patch and frees its slot, returning whether it was resident. */
  remove(keyString: string): boolean {
    const entry = this.entries.get(keyString);
    if (entry === undefined) {
      return false;
    }
    this.entries.delete(keyString);
    this.freeSlots.push(entry.patch.slot);
    return true;
  }

  /**
   * Sets the pins from this frame's selection and draw set and marks their patches as used.
   *
   * @remarks
   * Forced pins are the patches the selection marks `forced`. Draw pins are the patches drawn and
   * every other selected patch that is resident, so that the siblings an ancestor stands in for
   * are kept until all are resident. Both are replaced, not accumulated, so a patch leaving the
   * selection and the draw set becomes evictable at once. The pins are reported as exceeding the
   * slots when the forced patches, resident or not, and the other pinned resident patches outnumber
   * the slots, or when an insert was refused since the last retain.
   */
  retain(selection: Selection, draw: DrawSet): void {
    this.tick += 1;
    const forcedKeys = new Set<string>();
    const selectedKeys = new Set<string>();
    for (const [keyString, selected] of selection.patches) {
      selectedKeys.add(keyString);
      if (selected.forced) {
        forcedKeys.add(keyString);
      }
    }
    this.forcedKeys = forcedKeys;
    this.selectedKeys = selectedKeys;
    for (const [keyString, entry] of this.entries) {
      entry.forced = forcedKeys.has(keyString);
      entry.drawn = !entry.forced && selectedKeys.has(keyString);
      if (selectedKeys.has(keyString)) {
        entry.lastUsed = this.tick;
      }
    }
    for (const drawn of draw.patches) {
      const entry = this.entries.get(drawn.keyString);
      if (entry !== undefined) {
        entry.lastUsed = this.tick;
        entry.drawn = !entry.forced;
      }
    }
    const pins = this.pins();
    this.exceededAtRetain = this.refused || pins.forced + pins.drawn > this.slotCount;
    this.refused = false;
  }

  /** The pins against the slots (Design note 10: pins beyond the slots are reported). */
  pressure(): CachePressure {
    return { ...this.pins(), exceeded: this.exceededAtRetain || this.refused };
  }

  private pins(): { forced: number; drawn: number } {
    let drawn = 0;
    for (const entry of this.entries.values()) {
      if (!entry.forced && entry.drawn) {
        drawn += 1;
      }
    }
    return { forced: this.forcedKeys.size, drawn };
  }

  /** The patch to evict, or `null` if every resident patch is forced or a root. */
  private victim(): string | null {
    let best: string | null = null;
    let bestRank = 0;
    let bestUsed = 0;
    for (const [keyString, entry] of this.entries) {
      if (entry.forced || entry.patch.key.level === 0) {
        continue;
      }
      const rank = entry.drawn ? 1 : 0;
      const better =
        best === null ||
        rank < bestRank ||
        (rank === bestRank &&
          (entry.lastUsed < bestUsed || (entry.lastUsed === bestUsed && keyString < best)));
      if (better) {
        best = keyString;
        bestRank = rank;
        bestUsed = entry.lastUsed;
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

/** What the terrain pass draws: resident patches covering the selection without overlap. */
export interface DrawSet {
  /** The drawn patches, ordered by `patchKeyString`. */
  readonly patches: ReadonlyArray<DrawnPatch>;
  /** Each drawn patch's slot index, in the same order: the instanced draw's per-instance data. */
  readonly slots: Uint32Array;
  /**
   * How many selected patches are drawn by a coarser resident ancestor, those beneath an ancestor
   * standing in for a sibling included.
   */
  readonly standingIn: number;
  /** How many selected patches have no resident ancestor at all, and are not drawn. */
  readonly missing: number;
}

/**
 * Resolves what to draw: each selected patch if resident, else its nearest resident ancestor
 * (Design note 7).
 *
 * @remarks
 * An ancestor standing in covers its whole area, so a resident selected patch beneath it is drawn
 * through the ancestor rather than on top of it; the drawn patches never overlap. It reads the
 * cache and changes nothing; {@link PatchCache.retain} records the frame's use.
 */
export function resolveDrawSet(selection: Selection, cache: PatchCache): DrawSet {
  const chosen = new Map<string, { patch: CachedPatch; standIn: boolean }>();
  let missing = 0;
  for (const [keyString, selected] of selection.patches) {
    const own = cache.get(keyString);
    if (own !== undefined) {
      if (!chosen.has(keyString)) {
        chosen.set(keyString, { patch: own, standIn: false });
      }
      continue;
    }
    let ancestor = parentKey(selected.key);
    let found: { keyString: string; patch: CachedPatch } | null = null;
    while (ancestor !== null) {
      const ancestorString = patchKeyString(ancestor);
      const patch = cache.get(ancestorString);
      if (patch !== undefined) {
        found = { keyString: ancestorString, patch };
        break;
      }
      ancestor = parentKey(ancestor);
    }
    if (found === null) {
      missing += 1;
      continue;
    }
    chosen.set(found.keyString, { patch: found.patch, standIn: true });
  }
  const patches: DrawnPatch[] = [];
  for (const [keyString, { patch, standIn }] of chosen) {
    if (!hasChosenAncestor(patch.key, chosen)) {
      patches.push({ keyString, patch, standIn });
    }
  }
  patches.sort((a, b) => (a.keyString < b.keyString ? -1 : a.keyString > b.keyString ? 1 : 0));
  const slots = Uint32Array.from(patches, (p) => p.patch.slot);
  let drawnAsThemselves = 0;
  for (const p of patches) {
    if (selection.patches.has(p.keyString)) {
      drawnAsThemselves += 1;
    }
  }
  const standingIn = selection.patches.size - missing - drawnAsThemselves;
  return { patches, slots, standingIn, missing };
}

function hasChosenAncestor(key: PatchKey, chosen: ReadonlyMap<string, unknown>): boolean {
  let ancestor = parentKey(key);
  while (ancestor !== null) {
    if (chosen.has(patchKeyString(ancestor))) {
      return true;
    }
    ancestor = parentKey(ancestor);
  }
  return false;
}

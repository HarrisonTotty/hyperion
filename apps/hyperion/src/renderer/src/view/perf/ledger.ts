/**
 * The itemised GPU memory of a perf run (plan R12, R12.T3.a; Design note 6): R05's allocation
 * tally grouped by the memory table's rows, `@hyperion/perf`'s `MemoryItem`s, through
 * `MEMORY_ITEMS`.
 *
 * @remarks
 * WebGPU has no memory query, so this is the itemised half of Design note 6: every buffer and
 * texture is made through the engine's one creation path with a `MemoryCategory`, sized by R01's
 * `textureBytes` from its format, size, layers and mips (`engine/memory.ts`), and counted by R05's
 * tally. It is a lower bound on resident bytes, since drivers pad and align beyond the texels' own
 * size; the kernel's figure is the main process's (R12.T3.b). Nothing here runs outside a perf run:
 * the renderer's half of a run (R12.T4.b) makes the ledger.
 */

import { MEMORY_ITEMS, MEMORY_ITEMS_ORDER, type MemoryItem } from "@hyperion/perf";

import type { MemoryCategory } from "../engine/memory";
import type { AllocationTally } from "../terrain/gpu/allocationTally";

/** Each category's row: `MEMORY_ITEMS`, which must name every category (a compile error else). */
const ITEMS: Readonly<Record<MemoryCategory, MemoryItem>> = MEMORY_ITEMS;

function isCategory(key: string): key is MemoryCategory {
  return Object.hasOwn(ITEMS, key);
}

/** Every `MemoryCategory`, in `MEMORY_ITEMS`' order. */
export const LEDGER_CATEGORIES: ReadonlyArray<MemoryCategory> =
  Object.keys(ITEMS).filter(isCategory);

/** Each memory item at zero: a literal, so that an item added to the union is a compile error here. */
const ZERO_ITEMS: Readonly<Record<MemoryItem, number>> = {
  "star-cubemap": 0,
  "height-cache": 0,
  "atmosphere-tables": 0,
  "atmosphere-view": 0,
  "coarse-field-gpu": 0,
  "render-targets": 0,
  shadows: 0,
  clouds: 0,
  ocean: 0,
  rings: 0,
  scatter: 0,
  stills: 0,
  other: 0,
};

/** The memory row `category` is reported in. */
export function itemOf(category: MemoryCategory): MemoryItem {
  return ITEMS[category];
}

/** The tally's live bytes now, by memory item: each item the sum of its categories. */
export function itemise(tally: AllocationTally): Readonly<Record<MemoryItem, number>> {
  const items = { ...ZERO_ITEMS };
  for (const category of LEDGER_CATEGORIES) {
    items[ITEMS[category]] += tally.liveBytes(category);
  }
  return items;
}

/** The ledger's figures: each item's live and peak bytes, and the whole tally's peak. */
export interface LedgerFigures {
  readonly liveBytes: Readonly<Record<MemoryItem, number>>;
  /** The most bytes each item held at once since the ledger began or {@link MemoryLedger.resetPeaks}. */
  readonly peakBytes: Readonly<Record<MemoryItem, number>>;
  /** The most bytes the whole tally held at once, over the same span. */
  readonly tallyPeakBytes: number;
}

/**
 * The itemised memory over a span, from the tally's events: each item's peak is the most its
 * categories held together at any event, not the sum of their own peaks, which may fall at
 * different times.
 */
export class MemoryLedger {
  readonly #tally: AllocationTally;
  readonly #unsubscribe: () => void;
  #peak: Record<MemoryItem, number>;
  #tallyPeakBytes: number;

  constructor(tally: AllocationTally) {
    this.#tally = tally;
    this.#peak = { ...itemise(tally) };
    this.#tallyPeakBytes = sumOf(this.#peak);
    this.#unsubscribe = tally.subscribe(() => {
      this.#observe();
    });
  }

  #observe(): void {
    const live = itemise(this.#tally);
    for (const item of MEMORY_ITEMS_ORDER) {
      this.#peak[item] = Math.max(this.#peak[item], live[item]);
    }
    this.#tallyPeakBytes = Math.max(this.#tallyPeakBytes, sumOf(live));
  }

  /** The figures now. */
  figures(): LedgerFigures {
    return {
      liveBytes: itemise(this.#tally),
      peakBytes: { ...this.#peak },
      tallyPeakBytes: this.#tallyPeakBytes,
    };
  }

  /** Lowers every peak to the live bytes: a repetition's start. */
  resetPeaks(): void {
    this.#peak = { ...itemise(this.#tally) };
    this.#tallyPeakBytes = sumOf(this.#peak);
  }

  /** Stops listening to the tally, which stays the caller's. */
  dispose(): void {
    this.#unsubscribe();
  }
}

function sumOf(items: Readonly<Record<MemoryItem, number>>): number {
  let bytes = 0;
  for (const item of MEMORY_ITEMS_ORDER) {
    bytes += items[item];
  }
  return bytes;
}

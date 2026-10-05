/**
 * A map from patch keys to values, held in typed arrays (plan R05, T7 perf (d)).
 *
 * @remarks
 * A key's fields fit two 32-bit words: its column i, and its face, level and row j. The table hashes
 * those words into an open-addressed `Int32Array` with linear probing, so a lookup builds no string
 * and boxes no number. A `Map` keyed by `patchKeyIndex` boxes its index, since an index above 2³⁰
 * is no small integer, and hashes the boxed double; that lookup was most of the patch cache's
 * `heightRangeM` cost to selection.
 */

import { MAX_LEVEL, type PatchKey } from "./patchKey";

/** The slots a new table starts with, a power of two. */
const INITIAL_SLOTS = 64;

/** The first word of an empty slot: no key has a negative column. */
const EMPTY = -1;

/** A key's second word: its face, level and row j, as a 32-bit integer. */
function secondWord(face: number, level: number, j: number): number {
  return (face << 29) | (level << 24) | j;
}

/** The home slot of words (`w0`, `w1`) in a table of `mask + 1` slots. */
function homeSlot(w0: number, w1: number, mask: number): number {
  let h = Math.imul(w0, 0x9e3779b1) ^ w1;
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b);
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
  return (h ^ (h >>> 16)) & mask;
}

/**
 * A map from patch keys to values, by each key's face, level, i and j.
 *
 * @remarks
 * Iteration order is not kept; the patch cache and selection only look up, add and remove. Keys
 * must be valid (`isValidPatchKey`): i and j below 2²⁴ and a level up to {@link MAX_LEVEL}.
 */
export class PatchKeyTable<T> {
  /** Two words a slot: i (or {@link EMPTY}), then face, level and j. */
  private words = new Int32Array(2 * INITIAL_SLOTS).fill(EMPTY);
  private values: (T | undefined)[] = Array.from({ length: INITIAL_SLOTS }, () => undefined);
  private mask = INITIAL_SLOTS - 1;
  private count = 0;

  /** The number of keys held. */
  get size(): number {
    return this.count;
  }

  /** The value at `key`, or `undefined`. */
  get(key: PatchKey): T | undefined {
    return this.getAt(key.face, key.level, key.i, key.j);
  }

  /** The value at the key (`face`, `level`, `i`, `j`), or `undefined`: a lookup with no key object. */
  getAt(face: number, level: number, i: number, j: number): T | undefined {
    const w1 = secondWord(face, level, j);
    const { words, mask } = this;
    for (let slot = homeSlot(i, w1, mask); ; slot = (slot + 1) & mask) {
      const w0 = words[2 * slot];
      if (w0 === EMPTY || w0 === undefined) {
        return undefined;
      }
      if (w0 === i && words[2 * slot + 1] === w1) {
        return this.values[slot];
      }
    }
  }

  /**
   * Sets the value at `key`, adding the key if it is new.
   *
   * @throws RangeError for a level deeper than {@link MAX_LEVEL}, whose key would not fit its words.
   */
  set(key: PatchKey, value: T): void {
    if (key.level > MAX_LEVEL) {
      throw new RangeError(`level ${key.level} is not a quadtree level`);
    }
    const w1 = secondWord(key.face, key.level, key.j);
    const { words, mask } = this;
    let slot = homeSlot(key.i, w1, mask);
    for (;;) {
      const w0 = words[2 * slot];
      if (w0 === EMPTY || w0 === undefined) {
        break;
      }
      if (w0 === key.i && words[2 * slot + 1] === w1) {
        this.values[slot] = value;
        return;
      }
      slot = (slot + 1) & mask;
    }
    words[2 * slot] = key.i;
    words[2 * slot + 1] = w1;
    this.values[slot] = value;
    this.count += 1;
    // At most half full, so that a probe stays short.
    if (2 * this.count > mask + 1) {
      this.grow();
    }
  }

  /** Removes `key`, returning whether it was held. */
  delete(key: PatchKey): boolean {
    const w1 = secondWord(key.face, key.level, key.j);
    const { words, values, mask } = this;
    let slot = homeSlot(key.i, w1, mask);
    for (;;) {
      const w0 = words[2 * slot];
      if (w0 === EMPTY || w0 === undefined) {
        return false;
      }
      if (w0 === key.i && words[2 * slot + 1] === w1) {
        break;
      }
      slot = (slot + 1) & mask;
    }
    // Backward-shift deletion: move up each later key of the run whose home slot does not lie
    // cyclically between the hole and it, so that every key stays reachable from its home slot.
    let hole = slot;
    for (let next = (hole + 1) & mask; ; next = (next + 1) & mask) {
      const w0 = words[2 * next] ?? EMPTY;
      if (w0 === EMPTY) {
        break;
      }
      const home = homeSlot(w0, words[2 * next + 1] ?? 0, mask);
      const stays = hole <= next ? hole < home && home <= next : hole < home || home <= next;
      if (!stays) {
        words[2 * hole] = w0;
        words[2 * hole + 1] = words[2 * next + 1] ?? 0;
        values[hole] = values[next];
        hole = next;
      }
    }
    words[2 * hole] = EMPTY;
    values[hole] = undefined;
    this.count -= 1;
    return true;
  }

  /** Doubles the slots and re-inserts every key. */
  private grow(): void {
    const oldWords = this.words;
    const oldValues = this.values;
    const slots = 2 * (this.mask + 1);
    this.words = new Int32Array(2 * slots).fill(EMPTY);
    this.values = Array.from({ length: slots }, () => undefined);
    this.mask = slots - 1;
    const { words, values, mask } = this;
    for (let old = 0; old < oldValues.length; old += 1) {
      const w0 = oldWords[2 * old] ?? EMPTY;
      if (w0 === EMPTY) {
        continue;
      }
      const w1 = oldWords[2 * old + 1] ?? 0;
      let slot = homeSlot(w0, w1, mask);
      while (words[2 * slot] !== EMPTY) {
        slot = (slot + 1) & mask;
      }
      words[2 * slot] = w0;
      words[2 * slot + 1] = w1;
      values[slot] = oldValues[old];
    }
  }
}

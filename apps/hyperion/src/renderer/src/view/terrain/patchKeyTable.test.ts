import { describe, expect, it } from "vitest";

import { seededRandom } from "../../test/seededRandom";
import { FACES, MAX_LEVEL, type PatchKey, patchKeyString } from "./patchKey";
import { PatchKeyTable } from "./patchKeyTable";

/** A random valid key, sometimes at a corner of its face's lattice. */
function randomKey(random: () => number): PatchKey {
  const level = Math.floor(random() * (MAX_LEVEL + 1));
  const side = 2 ** level;
  const face = FACES[Math.floor(random() * 6)] ?? 0;
  const corner = random() < 0.2;
  return {
    face,
    level,
    i: corner ? (random() < 0.5 ? 0 : side - 1) : Math.floor(random() * side),
    j: corner ? (random() < 0.5 ? 0 : side - 1) : Math.floor(random() * side),
  };
}

describe("a table of patch keys", () => {
  it("adds, finds, replaces and removes as a map by key string does", () => {
    const random = seededRandom(0x7461626c);
    const table = new PatchKeyTable<number>();
    const reference = new Map<string, number>();
    const keys: PatchKey[] = [];
    const mismatches: string[] = [];
    for (let step = 0; step < 40_000; step += 1) {
      const roll = random();
      if (roll < 0.55 || keys.length === 0) {
        // A new key, or one held already, so that values are replaced too.
        const key =
          roll < 0.45 || keys.length === 0
            ? randomKey(random)
            : (keys[Math.floor(random() * keys.length)] ?? randomKey(random));
        table.set(key, step);
        reference.set(patchKeyString(key), step);
        keys.push(key);
      } else {
        const key = keys[Math.floor(random() * keys.length)] ?? randomKey(random);
        const held = reference.delete(patchKeyString(key));
        if (table.delete(key) !== held) {
          mismatches.push(`delete ${patchKeyString(key)} at ${step}`);
        }
      }
      if (step % 997 === 0) {
        // Every key ever used, held or removed, and some never used.
        for (const key of [...keys, randomKey(random)]) {
          const want = reference.get(patchKeyString(key));
          if (table.get(key) !== want || table.getAt(key.face, key.level, key.i, key.j) !== want) {
            mismatches.push(`get ${patchKeyString(key)} at ${step}`);
          }
        }
        if (table.size !== reference.size) {
          mismatches.push(`size ${table.size} ≠ ${reference.size} at ${step}`);
        }
      }
    }
    expect(mismatches).toEqual([]);
    expect(reference.size).toBeGreaterThan(5_000);
  });

  it("tells apart keys that differ in one field alone", () => {
    const table = new PatchKeyTable<string>();
    const deepest = 2 ** MAX_LEVEL - 1;
    const keys: PatchKey[] = [
      { face: 0, level: MAX_LEVEL, i: deepest, j: deepest },
      { face: 5, level: MAX_LEVEL, i: deepest, j: deepest },
      { face: 5, level: MAX_LEVEL - 1, i: deepest >> 1, j: deepest >> 1 },
      { face: 5, level: MAX_LEVEL, i: deepest, j: deepest - 1 },
      { face: 5, level: MAX_LEVEL, i: deepest - 1, j: deepest },
      { face: 3, level: 0, i: 0, j: 0 },
      { face: 4, level: 0, i: 0, j: 0 },
    ];
    for (const key of keys) {
      table.set(key, patchKeyString(key));
    }
    expect(keys.map((key) => table.get(key))).toEqual(keys.map(patchKeyString));
    expect(table.get({ face: 2, level: 0, i: 0, j: 0 })).toBeUndefined();
    expect(table.size).toBe(keys.length);
  });

  it("refuses a level deeper than the quadtree's", () => {
    const table = new PatchKeyTable<number>();
    expect(() => table.set({ face: 0, level: MAX_LEVEL + 1, i: 0, j: 0 }, 1)).toThrow(RangeError);
  });
});

import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { selectionOf } from "../../test/terrainFixtures";
import { PatchCache, type ResidentPatch, resolveDrawSet } from "./cache";
import { childKeys, FACES, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import type { Selection } from "./select";
import { slotLayout } from "./slotLayout";

/** A cache of `slots` slots of 100 B each. */
function cacheOf(slots: number): PatchCache {
  return new PatchCache(
    slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], slots * 100),
  );
}

function resident(key: PatchKey, generation = 1): ResidentPatch {
  return {
    key,
    generation,
    originM: vec3(0, 0, 0),
    heightRangeM: [0, 0],
    boundingRadiusM: 1,
  };
}

/** Records a frame: the draw set of `selection` as the cache holds it now, then the pins. */
function frame(cache: PatchCache, selection: Selection): void {
  cache.retain(selection, resolveDrawSet(selection, cache));
}

const ROOT = rootKey(0);
const [A, B, C, D] = childKeys(ROOT);
const [AA, AB, AC, AD] = childKeys(A);

function insertAll(cache: PatchCache, keys: readonly PatchKey[]): void {
  for (const key of keys) {
    const result = cache.insert(resident(key));
    if (result.kind !== "stored") {
      throw new Error(`expected ${patchKeyString(key)} to be stored`);
    }
  }
}

describe("the patch cache", () => {
  it("never uses more slots than exist, nor a slot twice", () => {
    const cache = cacheOf(8);
    const keys = [...childKeys(A), ...childKeys(B), ...childKeys(C)];
    insertAll(cache, keys);
    expect(cache.usedSlots).toBe(8);
    const slots = keys.flatMap((k) => {
      const patch = cache.get(patchKeyString(k));
      return patch === undefined ? [] : [patch.slot];
    });
    expect(slots).toHaveLength(8);
    expect(new Set(slots).size).toBe(8);
    expect(slots.every((s) => s >= 0 && s < 8)).toBe(true);
  });

  it("evicts the least recently used patch first", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, B, C, D, AA, AB]);
    frame(cache, selectionOf([]));
    // Use everything but B in a frame where nothing is drawn or forced, through the selection.
    cache.retain(selectionOf([A, C, D, AA, AB]), resolveDrawSet(selectionOf([]), cache));
    const result = cache.insert(resident(AC));
    expect(result).toEqual({
      kind: "stored",
      slot: expect.any(Number),
      evicted: patchKeyString(B),
    });
    expect(cache.has(patchKeyString(B))).toBe(false);
  });

  it("evicts drawn patches only after unpinned ones", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, B, C, D, AA, AB]);
    // A is drawn this frame, and used before the others are touched again.
    frame(cache, selectionOf([A]));
    cache.retain(selectionOf([A]), resolveDrawSet(selectionOf([A]), cache));
    const evictions: (string | null)[] = [];
    for (const key of [AC, AD, ...childKeys(B)]) {
      const result = cache.insert(resident(key));
      evictions.push(result.kind === "stored" ? result.evicted : "refused");
    }
    // Every unpinned patch, the older and the newly stored, goes before A, the drawn one.
    expect(evictions).toHaveLength(6);
    expect(evictions).not.toContain(patchKeyString(A));
    expect(cache.has(patchKeyString(A))).toBe(true);
  });

  it("evicts a drawn patch once only forced patches remain beside it", () => {
    const cache = cacheOf(6);
    const forced = [B, C, D, AA, AB];
    insertAll(cache, [A, ...forced]);
    frame(cache, selectionOf([A, ...forced], forced));
    expect(cache.insert(resident(AC))).toEqual({
      kind: "stored",
      slot: expect.any(Number),
      evicted: patchKeyString(A),
    });
  });

  it("keeps a forced-region patch under any eviction pressure", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A]);
    frame(cache, selectionOf([A], [A]));
    for (let n = 0; n < 50; n += 1) {
      cache.insert(resident({ face: 1, level: 10, i: n, j: 0 }));
    }
    expect(cache.has(patchKeyString(A))).toBe(true);
  });

  it("keeps selected patches hidden under a stand-in pinned like the drawn ones", () => {
    const cache = cacheOf(6);
    const others = childKeys(B).slice(0, 3);
    insertAll(cache, [A, AA, AB]);
    frame(cache, selectionOf([AA, AB, AC, AD]));
    // Stored after the retain, so as recently used as AA and AB, and later in key order.
    insertAll(cache, others);
    for (let n = 0; n < 10; n += 1) {
      cache.insert(resident({ face: 1, level: 10, i: n, j: 0 }));
    }
    expect([A, AA, AB].map((k) => cache.has(patchKeyString(k)))).toEqual([true, true, true]);
  });

  it("pins a forced patch from the moment it arrives, before the next retain", () => {
    const cache = cacheOf(6);
    insertAll(cache, [B, C, D, ...childKeys(B).slice(0, 3)]);
    const forced = [AA, AB, AC, AD];
    frame(cache, selectionOf(forced, forced));
    insertAll(cache, forced);
    for (let n = 0; n < 20; n += 1) {
      cache.insert(resident({ face: 1, level: 10, i: n, j: 0 }));
    }
    expect(forced.every((k) => cache.has(patchKeyString(k)))).toBe(true);
  });

  it("refuses what cannot be stored and reports it after the frame's retain", () => {
    const cache = cacheOf(6);
    const forced = [A, B, C, D, AA, AB];
    insertAll(cache, forced);
    frame(cache, selectionOf(forced, forced));
    expect(cache.pressure()).toEqual({ forced: 6, drawn: 0, exceeded: false });
    expect(cache.insert(resident(AC))).toEqual({ kind: "refused" });
    expect(cache.pressure().exceeded).toBe(true);
    frame(cache, selectionOf(forced, forced));
    expect(cache.pressure().exceeded).toBe(true);
    frame(cache, selectionOf(forced, forced));
    expect(cache.pressure().exceeded).toBe(false);
  });

  it("reports forced patches beyond the slots, resident or not", () => {
    const cache = cacheOf(6);
    const forced = [...childKeys(A), ...childKeys(B)];
    insertAll(cache, forced.slice(0, 6));
    frame(cache, selectionOf(forced, forced));
    expect(cache.pressure()).toEqual({ forced: 8, drawn: 0, exceeded: true });
  });

  it("falls back to stand-ins outside the forced region when the pins fill the slots", () => {
    const cache = cacheOf(6);
    const other = rootKey(1);
    const [E, ...restOfOther] = childKeys(other);
    insertAll(cache, [A, AA, AB, AC, other, E]);
    const forced = [AA, AB, AC, AD];
    const selection = selectionOf([...forced, E, ...restOfOther], forced);
    frame(cache, selection);
    // The last forced patch arrives with every slot taken, and pushes out A, which stood in for it.
    const result = cache.insert(resident(AD));
    expect(result.kind === "stored" ? result.evicted : "refused").toBe(patchKeyString(A));
    const draw = resolveDrawSet(selection, cache);
    expect(draw.patches.map((p) => [p.keyString, p.standIn])).toEqual([
      ...forced
        .map((k) => patchKeyString(k))
        .toSorted()
        .map((s) => [s, false]),
      [patchKeyString(other), true],
    ]);
  });

  it("reuses a freed slot", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, B]);
    const slot = cache.get(patchKeyString(B))?.slot;
    expect(cache.remove(patchKeyString(B))).toBe(true);
    const result = cache.insert(resident(C));
    expect(result).toEqual({ kind: "stored", slot, evicted: null });
  });

  it("stores a newer bake of a resident patch in its own slot", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A]);
    const slot = cache.get(patchKeyString(A))?.slot;
    expect(cache.insert(resident(A, 2))).toEqual({ kind: "stored", slot, evicted: null });
    expect(cache.get(patchKeyString(A))?.generation).toBe(2);
    expect(cache.usedSlots).toBe(1);
  });

  it("counts the GPU bytes it holds", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, B, C]);
    expect(cache.heldBytes).toBe(300);
  });

  it("never evicts a root", () => {
    const cache = cacheOf(6);
    insertAll(cache, FACES.map(rootKey));
    expect(cache.insert(resident(A))).toEqual({ kind: "refused" });
  });
});

describe("the draw set", () => {
  it("draws a resident patch as itself and lists its slot", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A]);
    const draw = resolveDrawSet(selectionOf([A]), cache);
    expect(draw.patches.map((p) => [p.keyString, p.standIn])).toEqual([[patchKeyString(A), false]]);
    expect([...draw.slots]).toEqual([cache.get(patchKeyString(A))?.slot]);
    expect(draw.standingIn).toBe(0);
  });

  it("stands the nearest resident ancestor in for a missing patch and marks it", () => {
    const cache = cacheOf(6);
    insertAll(cache, [ROOT, A]);
    const draw = resolveDrawSet(selectionOf([AA, AB, AC, AD]), cache);
    expect(draw.patches.map((p) => [p.keyString, p.standIn])).toEqual([[patchKeyString(A), true]]);
    expect(draw.standingIn).toBe(4);
    expect(draw.missing).toBe(0);
  });

  it("draws an ancestor in place of the resident siblings it covers, never over them", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, AA, AB]);
    const draw = resolveDrawSet(selectionOf([AA, AB, AC, AD]), cache);
    expect(draw.patches.map((p) => p.keyString)).toEqual([patchKeyString(A)]);
    expect(draw.standingIn).toBe(4);
  });

  it("is the six roots when nothing else is resident", () => {
    const cache = cacheOf(10);
    insertAll(cache, FACES.map(rootKey));
    const selected = FACES.flatMap((f) => childKeys(rootKey(f)));
    const draw = resolveDrawSet(selectionOf(selected), cache);
    expect(draw.patches.map((p) => p.keyString)).toEqual(
      FACES.map((f) => patchKeyString(rootKey(f))),
    );
    expect(draw.patches.every((p) => p.standIn)).toBe(true);
  });

  it("counts selected patches with no resident ancestor as missing", () => {
    const cache = cacheOf(6);
    const draw = resolveDrawSet(selectionOf([A, B]), cache);
    expect(draw.patches).toHaveLength(0);
    expect(draw.missing).toBe(2);
  });
});

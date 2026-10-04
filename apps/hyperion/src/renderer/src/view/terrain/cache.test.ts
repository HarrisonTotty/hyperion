import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { goldenLevelTable, selectionOf, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lookAlong } from "../camera/quaternion";
import {
  type CacheInsert,
  DrawSetResolver,
  PatchCache,
  type ResidentPatch,
  resolveDrawSet,
} from "./cache";
import { vertexDir } from "./cube";
import { childKeys, FACES, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { type Selection, selectPatches } from "./select";
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

  it("pins every resident ancestor of the selection, above a stand-in too", () => {
    const cache = cacheOf(10);
    insertAll(cache, [ROOT, A, AA]);
    // AA's children are selected and unbaked: AA stands in for them, and A and the root above it
    // are pinned too.
    frame(cache, selectionOf(childKeys(AA)));
    expect(cache.pressure().drawn).toBe(3);
  });

  it("evicts the deeper of two patches used as recently first", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, B, AA, AB, ...childKeys(AA).slice(0, 2)]);
    frame(cache, selectionOf([]));
    const result = cache.insert(resident(C));
    expect(result.kind === "stored" ? result.evicted : "refused").toBe(
      patchKeyString(childKeys(AA)[0]),
    );
  });

  it("evicts a pinned ancestor only after every unpinned patch", () => {
    const cache = cacheOf(7);
    insertAll(cache, [A, AA, ...childKeys(B).slice(0, 4), C]);
    // AA's children are selected: AA and A are pinned as its ancestors; B's children and C are not.
    frame(cache, selectionOf(childKeys(AA)));
    const evicted = [D, ...childKeys(D)].map((k) => {
      const result = cache.insert(resident(k));
      return result.kind === "stored" ? result.evicted : "refused";
    });
    const unpinnedFirst = evicted.slice(0, 5);
    expect(unpinnedFirst).not.toContain(patchKeyString(A));
    expect(unpinnedFirst).not.toContain(patchKeyString(AA));
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

  it("pins a demanded ancestor from the moment it arrives, before the next retain", () => {
    const cache = cacheOf(6);
    const [BA, BB] = childKeys(B);
    insertAll(cache, [A, B, C, D, BA, BB]);
    // AA's children are selected and unbaked, so the selection demands AA, their parent.
    const selection: Selection = {
      ...selectionOf(childKeys(AA)),
      demand: [{ key: AA, priority: 1, forced: false }],
    };
    frame(cache, selection);
    // AA, then unpinned patches as recent as it and no deeper, which take the older ones' slots
    // first and then each other's.
    insertAll(cache, [AA, ...childKeys(C), childKeys(D)[0]]);
    expect(cache.has(patchKeyString(AA))).toBe(true);
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
    const drawn = new Map(draw.patches.map((p) => [p.keyString, p.standIn]));
    expect(drawn).toEqual(
      new Map([
        ...forced.map((k) => [patchKeyString(k), false] as const),
        [patchKeyString(other), true],
      ]),
    );
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

  it("gives a resident patch's baked height range, and none once it is gone", () => {
    const cache = cacheOf(6);
    cache.insert({ ...resident(A), heightRangeM: [-120, 340] });
    expect(cache.heightRangeM(A)).toEqual([-120, 340]);
    expect(cache.heightRangeM(B)).toBeUndefined();
    cache.remove(patchKeyString(A));
    expect(cache.heightRangeM(A)).toBeUndefined();
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

describe("the draw set's resolver", () => {
  it("returns the same set, arrays and records for an unchanged selection", () => {
    const cache = cacheOf(10);
    insertAll(cache, [ROOT, A, AA, AB]);
    const resolver = new DrawSetResolver(cache);
    const selection = selectionOf([AA, AB, AC, AD, B, C]);
    const first = resolver.resolve(selection);
    const records = [...first.patches];
    const slots = first.slots;
    const second = resolver.resolve(selection);
    expect(second).toBe(first);
    expect(second.slots).toBe(slots);
    expect(second.patches).toBe(first.patches);
    expect(second.patches.length).toBe(records.length);
    second.patches.forEach((p, n) => {
      expect(p).toBe(records[n]);
    });
  });

  it("keeps its storage when the selection changes", () => {
    const cache = cacheOf(10);
    insertAll(cache, [ROOT, A, B, C, D, AA, AB, AC, AD]);
    const resolver = new DrawSetResolver(cache);
    const coarse = resolver.resolve(selectionOf([A, B, C, D]));
    const slots = coarse.slots;
    const fine = resolver.resolve(selectionOf([AA, AB, AC, AD, B, C, D]));
    expect(fine).toBe(coarse);
    expect(fine.slots).toBe(slots);
    expect(fine.count).toBe(7);
    expect([...fine.slots.subarray(0, fine.count)]).toEqual(fine.patches.map((p) => p.patch.slot));
  });

  it("keeps a forced patch no view sees resident but leaves it out of the draw", () => {
    const cache = cacheOf(10);
    insertAll(cache, [ROOT, A, AA]);
    const base = selectionOf([AA, B], [AA]);
    const unseen = new Map(base.patches);
    const aa = unseen.get(patchKeyString(AA));
    if (aa === undefined) {
      throw new Error("no AA");
    }
    unseen.set(patchKeyString(AA), { ...aa, seen: false });
    const selection: Selection = { ...base, patches: unseen };
    const draw = resolveDrawSet(selection, cache);
    expect(draw.patches.map((p) => p.keyString)).toEqual([patchKeyString(ROOT)]);
    cache.retain(selection, draw);
    expect(cache.pressure().forced).toBe(1);
    for (let n = 0; n < 20; n += 1) {
      cache.insert(resident({ face: 1, level: 10, i: n, j: 0 }));
    }
    expect(cache.has(patchKeyString(AA))).toBe(true);
  });
});

describe("the draw set", () => {
  it("draws a resident patch as itself and lists its slot", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A]);
    const draw = resolveDrawSet(selectionOf([A]), cache);
    expect(draw.patches.map((p) => [p.keyString, p.standIn])).toEqual([[patchKeyString(A), false]]);
    expect(draw.count).toBe(1);
    expect([...draw.slots.subarray(0, draw.count)]).toEqual([cache.get(patchKeyString(A))?.slot]);
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

/** The `n`th patch `depth` levels under `key`, row by row. */
function descendant(key: PatchKey, depth: number, n: number): PatchKey {
  const side = 2 ** depth;
  return {
    face: key.face,
    level: key.level + depth,
    i: key.i * side + (n % side),
    j: key.j * side + (Math.floor(n / side) % side),
  };
}

function evictedBy(result: CacheInsert): string | null {
  if (result.kind !== "stored") {
    throw new Error("expected the patch to be stored");
  }
  return result.evicted;
}

describe("the patch cache under churn (the high-bound ruling's F3)", () => {
  it("keeps the coarse ancestors of the drawn patches, and a baked patch hidden beside them, while the drawn patches churn", () => {
    const cache = cacheOf(10);
    insertAll(cache, [A, AA, AB]);
    const evicted: string[] = [];
    for (let f = 0; f < 40; f += 1) {
      // Each frame draws four new patches six levels under AA, as a sliding forced region does,
      // while AB beside them is baked and hidden (culled by its own baked range).
      const fine = [0, 1, 2, 3].map((n) => descendant(AA, 6, 4 * f + n));
      frame(cache, selectionOf(fine, [], [AB]));
      for (const key of fine) {
        const victim = evictedBy(cache.insert(resident(key)));
        if (victim !== null) {
          evicted.push(victim);
        }
      }
    }
    const kept = [A, AA, AB].map(patchKeyString);
    expect(evicted.length).toBeGreaterThan(100);
    expect(evicted.filter((k) => kept.includes(k))).toEqual([]);
    expect(kept.every((k) => cache.has(k))).toBe(true);
  });

  it("evicts the hidden baked patches after every unpinned one and before the drawn ones", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, AA, AB, AC, B, C]);
    // AA is drawn and A is its ancestor; AB and AC are hidden; B and C are unpinned. D's children
    // are selected, so each is pinned as it arrives.
    frame(cache, selectionOf([AA, ...childKeys(D)], [], [AB, AC]));
    const evictions = childKeys(D).map((k) => evictedBy(cache.insert(resident(k))));
    expect(new Set(evictions.slice(0, 2))).toEqual(new Set([B, C].map(patchKeyString)));
    expect(new Set(evictions.slice(2))).toEqual(new Set([AB, AC].map(patchKeyString)));
    expect([A, AA].every((k) => cache.has(patchKeyString(k)))).toBe(true);
  });

  it("lets a patch age once the selection no longer finds it hidden", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, AA, AB]);
    frame(cache, selectionOf([AA], [], [AB]));
    frame(cache, selectionOf([AA]));
    insertAll(cache, [B, C, D]);
    expect(evictedBy(cache.insert(resident(AC)))).toBe(patchKeyString(AB));
  });

  it("does not count the hidden baked patches as pins", () => {
    const cache = cacheOf(6);
    insertAll(cache, [A, AA, AB, AC]);
    frame(cache, selectionOf([AA], [], [AB, AC]));
    // AA and its ancestor A.
    expect(cache.pressure()).toEqual({ forced: 0, drawn: 2, exceeded: false });
  });

  it("never holds more slots than exist, nor a slot twice, whatever the selection hides", () => {
    const cache = cacheOf(12);
    // Levels 1 to 3 of face 0.
    const pool: PatchKey[] = [];
    for (const k of [A, B, C, D]) {
      pool.push(k);
      for (const c of childKeys(k)) {
        pool.push(c, ...childKeys(c));
      }
    }
    let seed = 12_345;
    const next = (n: number): number => {
      seed = (Math.imul(seed, 1_103_515_245) + 12_345) >>> 0;
      return (seed >>> 8) % n;
    };
    const pick = (count: number): PatchKey[] =>
      Array.from({ length: count }, () => pool[next(pool.length)] ?? A);
    for (let f = 0; f < 300; f += 1) {
      const selected = pick(3 + next(6));
      const hidden = pick(next(16)).filter((k) => cache.has(patchKeyString(k)));
      frame(cache, selectionOf(selected, [], hidden));
      for (const key of selected) {
        if (!cache.has(patchKeyString(key))) {
          evictedBy(cache.insert(resident(key)));
        }
        expect(cache.usedSlots).toBeLessThanOrEqual(cache.slotCount);
        expect(cache.heldBytes).toBe(cache.usedSlots * cache.layout.bytesPerSlot);
      }
      const slots = pool.flatMap((k) => {
        const patch = cache.get(patchKeyString(k));
        return patch === undefined ? [] : [patch.slot];
      });
      expect(new Set(slots).size).toBe(slots.length);
      expect(slots.every((s) => s >= 0 && s < cache.slotCount)).toBe(true);
    }
  });

  it("evicts the same patches in the same order whatever order they were stored in", () => {
    const stored = [A, B, C, D, AA, AB, AC, AD];
    const evictions = (order: readonly PatchKey[]): (string | null)[] => {
      const cache = cacheOf(8);
      insertAll(cache, order);
      frame(cache, selectionOf([AA], [], [AB, C]));
      return [...childKeys(B), ...childKeys(D)].map((k) => evictedBy(cache.insert(resident(k))));
    };
    const forwards = evictions(stored);
    expect(forwards.every((k) => k !== null)).toBe(true);
    expect(evictions(stored.toReversed())).toEqual(forwards);
    expect(evictions([...stored.slice(4), ...stored.slice(0, 4)])).toEqual(forwards);
  });
});

/** The test planet's geometry with ridges off, for runs of selection and the cache together. */
const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** A point `heightM` above the datum, `step` × 12 finest patches east of a site on face 0. */
function alongTrack(step: number, heightM: number): Vec3 {
  const key: PatchKey = { face: 0, level: 19, i: 300_001 + 12 * step, j: 200_003 };
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), heightM);
  return vec3(x, y, z);
}

/** What a run of {@link flyLow} saw. */
interface LowRun {
  /** Baked patches of level 12 or coarser stored again after being evicted. */
  readonly coarseRebakes: number;
  /** Frames drawing by a stand-in a patch of level 12 or coarser that had been resident. */
  readonly coarseReturns: number;
  /** Every eviction, in order. */
  readonly evictions: ReadonlyArray<string>;
  readonly bakes: number;
}

/**
 * Flies a camera 1.5 km up, looking out to the horizon, 12 finest patches (about 210 m) a frame
 * along a track, with a grounded contact beneath it, as the descent's approach does: an ideal pool
 * bakes every request at once, on ground flat to ±50 m, so that a baked patch's range hides what
 * its ancestor's range would not.
 */
function flyLow(slots: number, frames: number, warmFrames: number): LowRun {
  const cache = cacheOf(slots);
  const resolver = new DrawSetResolver(cache);
  const everStored = new Set<string>();
  const evictions: string[] = [];
  let coarseRebakes = 0;
  let coarseReturns = 0;
  let bakes = 0;
  const tiltRad = 1.45;
  for (let f = 0; f < frames; f += 1) {
    const positionM = alongTrack(f, 1_500);
    const up = normalise(positionM);
    const east = normalise(vec3(-up.y, up.x, 0));
    const forward = vec3(
      -up.x * Math.cos(tiltRad) + east.x * Math.sin(tiltRad),
      -up.y * Math.cos(tiltRad) + east.y * Math.sin(tiltRad),
      -up.z * Math.cos(tiltRad) + east.z * Math.sin(tiltRad),
    );
    const selection = selectPatches({
      planet: PLANET,
      views: [
        {
          camera: { positionM, orientation: lookAlong(forward, up) },
          fovXRad: Math.PI / 3,
          viewport: { widthPx: 960, heightPx: 540 },
          weight: 1,
          tauPx: 2,
        },
      ],
      setting: "high",
      grounded: [{ positionM: alongTrack(f, 0), radiusM: 10 }],
      heightRanges: cache,
    });
    const measuring = f >= warmFrames;
    if (measuring) {
      const returns = [...selection.patches].some(
        ([k, p]) => p.seen && p.key.level <= 12 && !cache.has(k) && everStored.has(k),
      );
      coarseReturns += returns ? 1 : 0;
    }
    cache.retain(selection, resolver.resolve(selection));
    for (const request of selection.demand) {
      const keyString = patchKeyString(request.key);
      if (cache.has(keyString)) {
        continue;
      }
      const victim = evictedBy(cache.insert({ ...resident(request.key), heightRangeM: [-50, 50] }));
      bakes += 1;
      if (victim !== null) {
        evictions.push(victim);
      }
      if (measuring && request.key.level <= 12 && everStored.has(keyString)) {
        coarseRebakes += 1;
      }
      everStored.add(keyString);
    }
  }
  return { coarseRebakes, coarseReturns, evictions, bakes };
}

describe("selection and the cache over a low pass (the high-bound ruling's F3)", () => {
  it("bakes no coarse patch twice, and draws none by a stand-in once it was resident", () => {
    const run = flyLow(500, 48, 16);
    // The cache is under pressure: it evicts thousands of patches over the run.
    expect(run.evictions.length).toBeGreaterThan(1_000);
    expect([run.coarseRebakes, run.coarseReturns]).toEqual([0, 0]);
  });

  it("evicts the same patches in the same order on a second run", () => {
    const first = flyLow(500, 24, 0);
    const second = flyLow(500, 24, 0);
    expect(second.evictions.length).toBeGreaterThan(0);
    expect(second.evictions).toEqual(first.evictions);
    expect(second.bakes).toBe(first.bakes);
  });
});

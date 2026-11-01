import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { selectionOf } from "../../test/terrainFixtures";
import { type DrawSet, DrawSetResolver, PatchCache } from "./cache";
import { childKeys, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import { slotLayout } from "./slotLayout";
import { fadeAt, MORPH_IN_MS, TimeMorph } from "./timeMorph";

/** A cache of `slots` slots holding `keys`. */
function cacheWith(keys: ReadonlyArray<PatchKey>, slots = 32): PatchCache {
  const cache = new PatchCache(
    slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], slots * 100),
  );
  for (const key of keys) {
    cache.insert({
      key,
      generation: 1,
      originM: vec3(0, 0, 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
    });
  }
  return cache;
}

const ROOT = rootKey(2);
const CHILDREN = childKeys(ROOT);

/** The areas `drawn` draws, as `patchKeyString`s. */
function areas(drawn: DrawSet): string[] {
  return drawn.patches.map((p) => patchKeyString(p.within ?? p.patch.key));
}

/** The footprints at `nowMs` as `patchKeyString`s and fades. */
function footprints(morph: TimeMorph, nowMs: number): Array<[string, number]> {
  return morph.footprintsAt(nowMs).map(({ area, fade }) => [patchKeyString(area), fade]);
}

describe("the drawn ground's changes over time (R10.T16.i)", () => {
  it("fades by smoothstep from where it starts, and stays at its start until its first draw", () => {
    expect([
      fadeAt(1, 0, null, 500, 400),
      fadeAt(1, 0, 100, 100, 400),
      fadeAt(1, 0, 100, 300, 400),
      fadeAt(1, 0, 100, 500, 400),
      fadeAt(0.25, 1, 0, 1_000, 400),
    ]).toEqual([1, 1, 0.5, 0, 1]);
  });

  it("starts a refine at 1 over the finer patch's area at its first draw, and ends it at 0", () => {
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache);
    morph.footprintsAt(0);
    const [first] = CHILDREN;
    cache.insert({
      key: first,
      generation: 1,
      originM: vec3(0, 0, 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
    });
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache);
    expect([
      footprints(morph, 1_000),
      footprints(morph, 1_000 + MORPH_IN_MS / 2),
      footprints(morph, 1_000 + MORPH_IN_MS),
      morph.active,
    ]).toEqual([[[patchKeyString(first), 1]], [[patchKeyString(first), 0.5]], [], false]);
  });

  it("holds a coarsen's finer patches until its fade is 1, rising from 0, then presents the coarser", () => {
    const cache = cacheWith([ROOT, ...CHILDREN]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache);
    morph.footprintsAt(0);
    const coarse = selectionOf([ROOT]);
    const held = areas(morph.present(resolver.resolve(coarse), cache));
    const rising = footprints(morph, 1_000);
    const halfway = footprints(morph, 1_000 + MORPH_IN_MS / 2).map(([, fade]) => fade);
    footprints(morph, 1_000 + MORPH_IN_MS);
    const due = morph.swapDue;
    const after = areas(morph.present(resolver.resolve(coarse), cache));
    expect([held, rising, halfway, due, after]).toEqual([
      CHILDREN.map(patchKeyString),
      CHILDREN.map((k) => [patchKeyString(k), 0]),
      CHILDREN.map(() => 0.5),
      true,
      [patchKeyString(ROOT)],
    ]);
  });

  it("draws a coarser patch's parts over what was not drawn before, with no fade, beside the held ones", () => {
    const [first] = CHILDREN;
    const cache = cacheWith([ROOT, first]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf([first])), cache);
    morph.footprintsAt(0);
    const held = morph.present(resolver.resolve(selectionOf([ROOT])), cache);
    expect([areas(held).toSorted(), footprints(morph, 1_000)]).toEqual([
      CHILDREN.map(patchKeyString).toSorted(),
      [[patchKeyString(first), 0]],
    ]);
  });

  it("falls from the fade a coarsen reached where the selection refines back into it", () => {
    const cache = cacheWith([ROOT, ...CHILDREN]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    const fine = selectionOf(CHILDREN);
    morph.present(resolver.resolve(fine), cache);
    morph.footprintsAt(0);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache);
    footprints(morph, 1_000);
    footprints(morph, 1_000 + MORPH_IN_MS / 2);
    morph.present(resolver.resolve(fine), cache);
    expect(footprints(morph, 1_000 + MORPH_IN_MS / 2)).toEqual(
      CHILDREN.map((k) => [patchKeyString(k), 0.5]),
    );
  });

  it("swaps a coarsen at once, counted, where its finer patches have left the cache", () => {
    const cache = cacheWith([ROOT, ...CHILDREN]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache);
    for (const child of CHILDREN) {
      cache.remove(patchKeyString(child));
    }
    const presented = morph.present(resolver.resolve(selectionOf([ROOT])), cache);
    expect([areas(presented), morph.coarsensSwapped]).toEqual([[patchKeyString(ROOT)], 1]);
  });
});

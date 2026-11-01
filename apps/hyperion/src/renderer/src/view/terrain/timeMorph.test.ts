import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { selectionOf } from "../../test/terrainFixtures";
import { type DrawSet, DrawSetResolver, PatchCache } from "./cache";
import { childKeys, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import { slotLayout } from "./slotLayout";
import { DATUM_SLOT } from "./gpu/uniforms";
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

/** No area was seen before: every patch drawn where nothing was is ground newly in view. */
const unseen = (): boolean => false;
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
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.footprintsAt(0);
    const [first] = CHILDREN;
    cache.insert({
      key: first,
      generation: 1,
      originM: vec3(0, 0, 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
    });
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
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
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.footprintsAt(0);
    const coarse = selectionOf([ROOT]);
    const held = areas(morph.present(resolver.resolve(coarse), cache, unseen));
    const rising = footprints(morph, 1_000);
    const halfway = footprints(morph, 1_000 + MORPH_IN_MS / 2).map(([, fade]) => fade);
    footprints(morph, 1_000 + MORPH_IN_MS);
    const due = morph.swapDue;
    const after = areas(morph.present(resolver.resolve(coarse), cache, unseen));
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
    morph.present(resolver.resolve(selectionOf([first])), cache, unseen);
    morph.footprintsAt(0);
    const held = morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
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
    morph.present(resolver.resolve(fine), cache, unseen);
    morph.footprintsAt(0);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    footprints(morph, 1_000);
    footprints(morph, 1_000 + MORPH_IN_MS / 2);
    morph.present(resolver.resolve(fine), cache, unseen);
    expect(footprints(morph, 1_000 + MORPH_IN_MS / 2)).toEqual(
      CHILDREN.map((k) => [patchKeyString(k), 0.5]),
    );
  });

  it("rises a patch drawn where nothing was out of the datum over ground seen before, not over ground newly in view", () => {
    const [first, second] = CHILDREN;
    const cache = cacheWith([first, second]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf([first])), cache, unseen);
    const seen = (area: PatchKey): boolean => patchKeyString(area) === patchKeyString(second);
    morph.present(resolver.resolve(selectionOf([first, second])), cache, seen);
    morph.footprintsAt(1_000);
    const words = [...morph.heightWords.subarray(0, 2)];
    morph.footprintsAt(1_000 + MORPH_IN_MS / 2);
    const halfway = (morph.heightWords[1] ?? 0) >>> 8;
    morph.footprintsAt(1_000 + MORPH_IN_MS);
    expect([words, halfway, morph.heightWords[1], morph.active]).toEqual([
      [0, ((255 << 8) | (DATUM_SLOT << 16)) >>> 0],
      (DATUM_SLOT << 8) | 128,
      0,
      false,
    ]);
  });

  it("fades a rebake's heights from its older bake's slot, freeing it once the fade ends", () => {
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    morph.rebake(patchKeyString(ROOT), 7);
    morph.footprintsAt(1_000);
    const start = morph.heightWords[0];
    morph.footprintsAt(1_000 + MORPH_IN_MS);
    expect([start, morph.heightWords[0], morph.takeReleased()]).toEqual([
      ((255 << 8) | (7 << 16)) >>> 0,
      0,
      [7],
    ]);
  });

  it("swaps a coarsen at once, counted, where its finer patches have left the cache", () => {
    const cache = cacheWith([ROOT, ...CHILDREN]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    for (const child of CHILDREN) {
      cache.remove(patchKeyString(child));
    }
    const presented = morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    expect([areas(presented), morph.coarsensSwapped]).toEqual([[patchKeyString(ROOT)], 1]);
  });
});

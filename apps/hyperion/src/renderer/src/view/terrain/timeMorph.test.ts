import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { selectionOf } from "../../test/terrainFixtures";
import { type DrawSet, DrawSetResolver, PatchCache } from "./cache";
import { childKeys, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import { slotLayout } from "./slotLayout";
import { DATUM_SLOT } from "./gpu/uniforms";
import { MORPH_IN_MS, MORPH_MAX_MS, MORPH_MS_PER_PX, TimeMorph } from "./timeMorph";

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
  it("fades by smoothstep from its first draw, and stays at its start until then", () => {
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
    const fades = [1_000, 1_000, 1_000 + MORPH_IN_MS / 4, 1_000 + MORPH_IN_MS / 2].map(
      (ms) => footprints(morph, ms)[0]?.[1],
    );
    expect(fades).toEqual([1, 1, 1 - 0.25 * 0.25 * (3 - 0.5), 0.5]);
  });

  it("slows a change in progress as the pixels it moves the ground grow", () => {
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    let px = 0;
    const morph = new TimeMorph(32, MORPH_IN_MS, {
      morphPx: () => px,
      risePx: () => 0,
      changePx: () => 0,
    });
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
    footprints(morph, 1_000);
    // A quarter of the least span at it, then the camera closes in: the rest at twice the span.
    footprints(morph, 1_000 + MORPH_IN_MS / 4);
    px = (2 * MORPH_IN_MS) / MORPH_MS_PER_PX;
    const [[, fade] = ["", -1]] = footprints(morph, 1_000 + MORPH_IN_MS / 4 + MORPH_IN_MS / 2);
    expect(fade).toBeCloseTo(0.5, 12);
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

  it("draws a patch still falling over a finer one's area until its fade ends, then refines from 1", () => {
    const [first] = CHILDREN;
    const grandchildren = childKeys(first);
    const [finest] = grandchildren;
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.footprintsAt(0);
    for (const key of [first, finest]) {
      cache.insert({
        key,
        generation: 1,
        originM: vec3(0, 0, 0),
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
      });
    }
    // `first` refines; half way down, its children are selected and `finest` is resident.
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    footprints(morph, 1_000);
    const deeper = selectionOf([...grandchildren, ...CHILDREN.slice(1)]);
    const held = morph.present(resolver.resolve(deeper), cache, unseen);
    const drawer = held.patches.find(
      (p) => p.within !== null && patchKeyString(p.within) === patchKeyString(finest),
    );
    expect([drawer?.keyString, footprints(morph, 1_000 + MORPH_IN_MS / 2)]).toEqual([
      patchKeyString(first),
      grandchildren.map((k) => [patchKeyString(k), 0.5]),
    ]);
    footprints(morph, 1_000 + MORPH_IN_MS);
    expect(morph.swapDue).toBe(true);
    const swapped = morph.present(resolver.resolve(deeper), cache, unseen);
    expect([
      swapped.patches.find((p) => p.keyString === patchKeyString(finest))?.within ?? null,
      footprints(morph, 2_000),
    ]).toEqual([null, [[patchKeyString(finest), 1]]]);
  });

  it("refines two levels a level at a time, the one between drawn first over the finer area", () => {
    const [first] = CHILDREN;
    const grandchildren = childKeys(first);
    const [finest] = grandchildren;
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    const deeper = selectionOf([...grandchildren, ...CHILDREN.slice(1)]);
    morph.present(resolver.resolve(deeper), cache, unseen);
    morph.footprintsAt(0);
    for (const key of [first, finest]) {
      cache.insert({
        key,
        generation: 1,
        originM: vec3(0, 0, 0),
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
      });
    }
    const drawerOf = (drawn: DrawSet): string | undefined =>
      drawn.patches.find((p) => patchKeyString(p.within ?? p.patch.key) === patchKeyString(finest))
        ?.keyString;
    const between = drawerOf(morph.present(resolver.resolve(deeper), cache, unseen));
    footprints(morph, 1_000);
    const still = morph.swapDue;
    footprints(morph, 1_000 + MORPH_IN_MS);
    const due = morph.swapDue;
    const after = drawerOf(morph.present(resolver.resolve(deeper), cache, unseen));
    expect([between, still, due, after, footprints(morph, 2_000)]).toEqual([
      patchKeyString(first),
      false,
      true,
      patchKeyString(finest),
      [[patchKeyString(finest), 1]],
    ]);
  });

  it("lets a refine's hold no draw has shown give way at the next present", () => {
    const [first] = CHILDREN;
    const grandchildren = childKeys(first);
    const [finest] = grandchildren;
    const cache = cacheWith([ROOT, first, finest]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    morph.footprintsAt(0);
    const deeper = selectionOf([...grandchildren, ...CHILDREN.slice(1)]);
    morph.present(resolver.resolve(deeper), cache, unseen);
    const due = morph.swapDue;
    const drawn = morph.present(resolver.resolve(deeper), cache, unseen);
    expect([due, drawn.patches.some((p) => p.keyString === patchKeyString(finest))]).toEqual([
      true,
      true,
    ]);
  });

  it("joins a coarser patch's own parts with no hold and no rise", () => {
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.footprintsAt(0);
    const joined = morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    expect([areas(joined), footprints(morph, 1_000), morph.coarsensSwapped]).toEqual([
      [patchKeyString(ROOT)],
      [],
      0,
    ]);
  });

  it("raises a coarsen's finer patches alone, not the coarser patch's own parts, and releases them alone", () => {
    const [first] = CHILDREN;
    const cache = cacheWith([ROOT, first]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    const fine = selectionOf(CHILDREN);
    morph.present(resolver.resolve(fine), cache, unseen);
    morph.footprintsAt(0);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    const rising = footprints(morph, 1_000);
    footprints(morph, 1_000 + MORPH_IN_MS / 2);
    morph.present(resolver.resolve(fine), cache, unseen);
    expect([rising, footprints(morph, 1_000 + MORPH_IN_MS / 2)]).toEqual([
      [[patchKeyString(first), 0]],
      [[patchKeyString(first), 0.5]],
    ]);
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

  it("lengthens a change by the pixels it moves the ground, a pixel a 60 Hz frame, to the longest", () => {
    const [first, second] = CHILDREN;
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    // A refine moving the ground 40 px, and a rebake 1,000 px, past the longest span.
    const morph = new TimeMorph(32, MORPH_IN_MS, {
      morphPx: () => 40,
      risePx: () => 0,
      changePx: (_, changeM) => changeM,
    });
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.footprintsAt(0);
    cache.insert({
      key: first,
      generation: 1,
      originM: vec3(0, 0, 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
    });
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.rebake(patchKeyString(second), 7, 1_000);
    morph.footprintsAt(1_000);
    const spanMs = 40 * MORPH_MS_PER_PX;
    expect([
      MORPH_MS_PER_PX,
      footprints(morph, 1_000 + spanMs / 2),
      footprints(morph, 1_000 + MORPH_MAX_MS - 1).length,
      morph.takeReleased(),
      footprints(morph, 1_000 + MORPH_MAX_MS),
      morph.takeReleased(),
    ]).toEqual([37.5, [[patchKeyString(first), 0.5]], 0, [], [], [7]]);
  });

  it("swaps a rebake at once, counted, whose spare slot the record cannot name, and gives the slot back", () => {
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    morph.rebake(patchKeyString(ROOT), DATUM_SLOT);
    morph.footprintsAt(1_000);
    expect([morph.heightWords[0], morph.rebakesSwapped, morph.takeReleased()]).toEqual([
      0,
      1,
      [DATUM_SLOT],
    ]);
  });

  it("ends a rebake's fade with its patch, giving up its older slot", () => {
    const cache = cacheWith([ROOT]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    morph.rebake(patchKeyString(ROOT), 7);
    morph.forget(patchKeyString(ROOT));
    morph.footprintsAt(1_000);
    expect([morph.heightWords[0], morph.takeReleased(), morph.active]).toEqual([0, [7], false]);
  });

  it("cuts every change at its first draw under reduced motion", () => {
    const cache = cacheWith([ROOT, ...CHILDREN]);
    const resolver = new DrawSetResolver(cache);
    const morph = new TimeMorph(32);
    morph.present(resolver.resolve(selectionOf(CHILDREN)), cache, unseen);
    morph.footprintsAt(0);
    morph.present(resolver.resolve(selectionOf([ROOT])), cache, unseen);
    morph.rebake(patchKeyString(ROOT), 7);
    const cut = footprints(morph, 1_000);
    const due = morph.swapDue;
    morph.footprintsAt(1_000, true);
    expect([
      cut.length > 0,
      due,
      morph.swapDue,
      morph.heightWords[0],
      morph.takeReleased(),
    ]).toEqual([true, false, true, 0, [7]]);
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

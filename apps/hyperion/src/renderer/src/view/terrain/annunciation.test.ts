import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { selectionOf } from "../../test/terrainFixtures";
import {
  ANNUNCIATION_CLEAR_MS,
  ANNUNCIATION_ONSET_MS,
  coarserThan,
  TerrainAnnunciationDebounce,
  terrainAnnunciation,
} from "./annunciation";
import { PatchCache, type DrawSet, resolveDrawSet } from "./cache";
import { childKeys, type PatchKey, rootKey } from "./patchKey";
import type { Selection } from "./select";
import { slotLayout } from "./slotLayout";

/** The draw set of `selection` with `resident` in a cache. */
function drawOf(selection: Selection, resident: readonly PatchKey[]): DrawSet {
  const cache = new PatchCache(
    slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], 64 * 100),
  );
  for (const key of resident) {
    cache.insert({
      key,
      generation: 1,
      originM: vec3(0, 0, 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
    });
  }
  return resolveDrawSet(selection, cache);
}

const ROOT = rootKey(2);
const CHILDREN = childKeys(ROOT);
const GRANDCHILDREN = CHILDREN.flatMap((k) => childKeys(k));
const FRAME_MS = 1000 / 60;

/** Feeds `debounce` the same conditions every frame for `ms` from `startMs`, returning the end. */
function run(
  debounce: TerrainAnnunciationDebounce,
  conditions: (frame: number) => { streaming: boolean; detailLimited: boolean },
  startMs: number,
  ms: number,
): { endMs: number; lines: (string | null)[] } {
  const lines: (string | null)[] = [];
  let t = startMs;
  for (let frame = 0; t < startMs + ms; frame += 1) {
    lines.push(debounce.update(conditions(frame), t));
    t += FRAME_MS;
  }
  return { endMs: t, lines };
}

describe("the terrain annunciation's conditions", () => {
  it("is STREAMING while a stand-in is drawn", () => {
    const selection = selectionOf(CHILDREN);
    expect(terrainAnnunciation(drawOf(selection, [ROOT]), selection, selection)).toBe(
      "TERRAIN: STREAMING",
    );
  });

  it("is DETAIL LIMITED when the selection is coarser than the reference", () => {
    const low = selectionOf(CHILDREN);
    const reference = selectionOf(GRANDCHILDREN);
    expect(coarserThan(low, reference)).toBe(true);
    expect(terrainAnnunciation(drawOf(low, CHILDREN), low, reference)).toBe(
      "TERRAIN: DETAIL LIMITED",
    );
  });

  it("is neither with everything resident at the reference's own detail", () => {
    const selection = selectionOf(GRANDCHILDREN);
    expect(terrainAnnunciation(drawOf(selection, GRANDCHILDREN), selection, selection)).toBeNull();
  });

  it("is DETAIL LIMITED when the patch budget stopped the selection, its own reference", () => {
    const capped = { ...selectionOf(GRANDCHILDREN), limited: true };
    expect(terrainAnnunciation(drawOf(capped, GRANDCHILDREN), capped, capped)).toBe(
      "TERRAIN: DETAIL LIMITED",
    );
  });

  it("is STREAMING when both hold", () => {
    const low = selectionOf(CHILDREN);
    const reference = selectionOf(GRANDCHILDREN);
    expect(terrainAnnunciation(drawOf(low, [ROOT]), low, reference)).toBe("TERRAIN: STREAMING");
  });

  it("does not count a finer selection as limited", () => {
    expect(coarserThan(selectionOf(GRANDCHILDREN), selectionOf(CHILDREN))).toBe(false);
  });
});

describe("the terrain annunciation's debounce", () => {
  it("shows STREAMING after 250 ms and clears it 1 s after the last stand-in goes", () => {
    const debounce = new TerrainAnnunciationDebounce();
    const streaming = { streaming: true, detailLimited: false };
    const clear = { streaming: false, detailLimited: false };
    expect(debounce.update(streaming, 0)).toBeNull();
    expect(debounce.update(streaming, ANNUNCIATION_ONSET_MS - 1)).toBeNull();
    expect(debounce.update(streaming, ANNUNCIATION_ONSET_MS)).toBe("TERRAIN: STREAMING");
    expect(debounce.update(clear, 2_000)).toBe("TERRAIN: STREAMING");
    expect(debounce.update(clear, 2_000 + ANNUNCIATION_CLEAR_MS - 1)).toBe("TERRAIN: STREAMING");
    expect(debounce.update(clear, 2_000 + ANNUNCIATION_CLEAR_MS)).toBeNull();
  });

  it("shows DETAIL LIMITED steadily while the low setting has terrain in view", () => {
    const debounce = new TerrainAnnunciationDebounce();
    const { lines } = run(debounce, () => ({ streaming: false, detailLimited: true }), 0, 2_000);
    expect(lines.at(-1)).toBe("TERRAIN: DETAIL LIMITED");
  });

  it("shows STREAMING over DETAIL LIMITED when both are shown", () => {
    const debounce = new TerrainAnnunciationDebounce();
    const { lines } = run(debounce, () => ({ streaming: true, detailLimited: true }), 0, 1_000);
    expect(lines.at(-1)).toBe("TERRAIN: STREAMING");
  });

  it.each([
    ["hidden", false],
    ["shown", true],
  ])("never flickers under a condition toggling every frame from %s", (_, shownFirst) => {
    const debounce = new TerrainAnnunciationDebounce();
    let t = 0;
    if (shownFirst) {
      t = run(debounce, () => ({ streaming: true, detailLimited: false }), 0, 500).endMs;
    }
    const { lines } = run(
      debounce,
      (frame) => ({ streaming: frame % 2 === 0, detailLimited: false }),
      t,
      5_000,
    );
    expect(new Set(lines).size).toBe(1);
  });
});

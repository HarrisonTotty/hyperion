import { describe, expect, it } from "vitest";

import {
  EDGE_BANDS,
  edgeBandIndex,
  NO_STRIDE_BAND as NONE,
  STRIDE_HOLD_FADE_MS,
} from "../clusters/strides";
import { StrideHoldFades } from "./strideHoldFades";

/** A patch's edges' bands, edge x = 64's level-2 bands `level2` (none for `null`), the rest 50. */
function bandsWith(level2: number | null): Float64Array {
  const bands = new Float64Array(EDGE_BANDS).fill(50);
  for (let t = 0; t < 8; t += 1) {
    bands[edgeBandIndex(1, t, 2)] = level2 ?? NONE;
  }
  return bands;
}

/** Edge x = 64's level-2 bands in `bands`. */
function level2Of(bands: Float64Array): number[] {
  return Array.from({ length: 8 }, (_, t) => bands[edgeBandIndex(1, t, 2)] ?? Number.NaN);
}

/** Frames of patch "p" at `nowMs`, its holds `held`, edge x = 64's level-2 bands `level2`. */
function framesOf(
  frames: ReadonlyArray<readonly [nowMs: number, held: number, level2: number | null]>,
): Array<readonly [word: number, level2: number[]]> {
  const fades = new StrideHoldFades();
  return frames.map(([nowMs, held, level2]) => {
    const bands = bandsWith(level2);
    const word = fades.fade("p", held, bands, nowMs);
    fades.sweep();
    return [word, level2Of(bands)] as const;
  });
}

const HELD_X64 = 0b0010;

describe("the onsets of a view's coarse–fine holds (plan R14, R14.T3.c)", () => {
  it("fades an edge's hold in from its last level-2 bands, its share falling from 255 to 0 over its span", () => {
    const frames = framesOf([
      [0, 0, 120],
      [1_000, HELD_X64, null],
      [1_000 + STRIDE_HOLD_FADE_MS / 2, HELD_X64, null],
      [1_000 + STRIDE_HOLD_FADE_MS, HELD_X64, null],
    ]);
    const held = Array.from({ length: 8 }, () => 120);
    const none = Array.from({ length: 8 }, () => NONE);
    expect(frames).toEqual([
      [0, held],
      [255 << 8, held],
      [128 << 8, held],
      [0, none],
    ]);
  });

  it("takes no fade for a patch first seen held, nor for one whose hold has let go", () => {
    const none = Array.from({ length: 8 }, () => NONE);
    expect([
      framesOf([[0, HELD_X64, null]])[0],
      framesOf([
        [0, HELD_X64, null],
        [10, 0, 90],
      ])[1],
    ]).toEqual([
      [0, none],
      [0, Array.from({ length: 8 }, () => 90)],
    ]);
  });

  it("forgets a patch a frame does not fade, so that it comes back as first seen", () => {
    const fades = new StrideHoldFades();
    fades.fade("p", 0, bandsWith(120), 0);
    fades.sweep();
    fades.sweep();
    const bands = bandsWith(null);
    expect([fades.fade("p", HELD_X64, bands, 10), level2Of(bands)]).toEqual([
      0,
      Array.from({ length: 8 }, () => NONE),
    ]);
  });
});

import { describe, expect, it } from "vitest";

import { type ShotTerrainCounts, StreamSettle } from "./streamSettle";

function counts(
  selected: number,
  drawn: number,
  standingIn: number,
  missing: number,
): ShotTerrainCounts {
  return { selected, drawn, standingIn, missing };
}

describe("when the spike captures stop waiting for a shot's patches", () => {
  it("waits on through frames slower than the quiet window while patches still land", () => {
    // The spike-arc shot on SwiftShader at d072bcb7 (high): a frame each 5 to 6 s, the drawn less
    // the standing-in count -1 at the second and third frames, five patches made resident between.
    const settle = new StreamSettle(0);
    expect(settle.done(counts(4, 0, 0, 4), 0)).toBe(false);
    expect(settle.done(counts(7, 4, 5, 0), 11_312)).toBe(false);
    expect(settle.done(counts(9, 6, 7, 0), 16_934)).toBe(false);
    expect(settle.done(counts(14, 9, 8, 0), 22_143)).toBe(false);
  });

  it("does not end the wait at one slow frame whose counts are unchanged", () => {
    // The spike-low-pass shot's first frame took 3.2 s on low.
    const settle = new StreamSettle(0);
    expect(settle.done(counts(5, 3, 3, 0), 0)).toBe(false);
    expect(settle.done(counts(5, 3, 3, 0), 3_297)).toBe(false);
  });

  it("ends once every selected patch is drawn as itself", () => {
    const settle = new StreamSettle(0);
    expect(settle.done(counts(7, 4, 5, 0), 0)).toBe(false);
    expect(settle.done(counts(44, 44, 0, 0), 400)).toBe(true);
  });

  it("ends after 3 s and three frames in a row with no count changing", () => {
    const settle = new StreamSettle(0);
    expect(settle.done(counts(9, 6, 7, 0), 0)).toBe(false);
    expect(settle.done(counts(9, 6, 7, 0), 1_000)).toBe(false);
    expect(settle.done(counts(9, 6, 7, 0), 2_000)).toBe(false);
    expect(settle.done(counts(9, 6, 7, 0), 3_001)).toBe(true);
  });

  it("counts the quiet again from a frame whose counts changed", () => {
    const settle = new StreamSettle(0);
    expect(settle.done(counts(9, 6, 7, 0), 0)).toBe(false);
    expect(settle.done(counts(9, 6, 7, 0), 2_000)).toBe(false);
    expect(settle.done(counts(9, 6, 7, 0), 2_500)).toBe(false);
    expect(settle.done(counts(9, 7, 6, 0), 3_100)).toBe(false);
    expect(settle.done(counts(9, 7, 6, 0), 6_200)).toBe(false);
  });

  it("ends at the settle limit, whatever is still streaming", () => {
    const settle = new StreamSettle(0);
    expect(settle.done(counts(9, 6, 7, 0), 0)).toBe(false);
    expect(settle.done(counts(10, 6, 7, 0), 119_000)).toBe(false);
    expect(settle.done(counts(11, 6, 7, 0), 120_001)).toBe(true);
  });
});

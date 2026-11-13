import { describe, expect, it } from "vitest";

import { BLUE_NOISE_SIDE, blueNoiseTile } from "./blueNoise";

const tile = blueNoiseTile();
const n = BLUE_NOISE_SIDE * BLUE_NOISE_SIDE;

/** The variance of the tile's means over toroidal 4 × 4 blocks. */
function blockMeanVariance(values: ArrayLike<number>): number {
  const means: number[] = [];
  for (let by = 0; by < BLUE_NOISE_SIDE; by += 4) {
    for (let bx = 0; bx < BLUE_NOISE_SIDE; bx += 4) {
      let sum = 0;
      for (let y = 0; y < 4; y += 1) {
        for (let x = 0; x < 4; x += 1) {
          sum += values[(by + y) * BLUE_NOISE_SIDE + bx + x] ?? 0;
        }
      }
      means.push(sum / 16);
    }
  }
  const mean = means.reduce((a, b) => a + b, 0) / means.length;
  return means.reduce((a, b) => a + (b - mean) ** 2, 0) / means.length;
}

describe("the blue-noise tile", () => {
  it("holds every rank once, uniform over (0, 1)", () => {
    const ranks = Array.from(tile, (v) => Math.round(v * n - 0.5)).toSorted((a, b) => a - b);
    expect(ranks).toEqual(Array.from({ length: n }, (_, i) => i));
  });

  it("is the same on every call", () => {
    expect(blueNoiseTile()).toEqual(tile);
  });

  it("holds the bits the per-texel-modulo splat made (perf-long-tasks-2)", () => {
    // FNV-1a over the tile's f32 bytes, little-endian, as the tile was made before its splat
    // walked each row in two runs: every texel's sums are taken in the same order.
    let digest = 0x811c9dc5;
    for (const word of new Uint32Array(tile.buffer)) {
      for (let byte = 0; byte < 4; byte += 1) {
        digest = Math.imul(digest ^ ((word >>> (8 * byte)) & 0xff), 0x01000193) >>> 0;
      }
    }
    expect(digest.toString(16)).toBe("6e80e9a1");
  });

  it("has far less low-frequency energy than white noise", () => {
    // White noise's 4 × 4 block means vary by (1/12)/16; blue noise's by a small part of that.
    expect(blockMeanVariance(tile)).toBeLessThan(0.2 * (1 / 12 / 16));
  });
});

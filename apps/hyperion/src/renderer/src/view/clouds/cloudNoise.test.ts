import { describe, expect, it } from "vitest";

import {
  CLOUD_NOISE_SIZE,
  CLOUD_NOISE_STD,
  CloudNoiseBuilder,
  cloudNoiseTexels,
  decodeCloudNoise,
  inverseNormal,
  sampleCloudNoise,
} from "./cloudNoise";

const TILE = cloudNoiseTexels();

/** The fractional part of `x`. */
function frac(x: number): number {
  return x - Math.floor(x);
}

/** Point i of the additive R₃ sequence (Roberts 2018), in the unit cube: a fixed low-discrepancy set. */
function r3(i: number): { x: number; y: number; z: number } {
  // φ₃, the positive root of x⁴ = x + 1.
  const g = 1.220_744_084_605_759;
  return { x: frac(0.5 + i / g), y: frac(0.5 + i / (g * g)), z: frac(0.5 + i / (g * g * g)) };
}

describe("the clouds' noise tile", () => {
  it("holds one byte a texel of a 64³ tile", () => {
    expect(TILE.length).toBe(CLOUD_NOISE_SIZE ** 3);
  });

  it("is the same however its making is spread over steps", () => {
    const builder = new CloudNoiseBuilder();
    let steps = 0;
    let texels = builder.step();
    while (texels === null) {
      texels = builder.step();
      steps += 1;
    }
    expect(steps).toBe(2 * CLOUD_NOISE_SIZE - 1);
    expect(builder.texels).toBe(texels);
    expect(texels).toEqual(TILE);
  });

  it("stores a standard normal's quantiles", () => {
    let sum = 0;
    let squares = 0;
    let aboveOne = 0;
    for (const byte of TILE) {
      const n = decodeCloudNoise(byte);
      sum += n;
      squares += n * n;
      aboveOne += n > 1 ? 1 : 0;
    }
    const mean = sum / TILE.length;
    expect(Math.abs(mean)).toBeLessThan(0.01);
    expect(Math.abs(Math.sqrt(squares / TILE.length - mean * mean) - 1)).toBeLessThan(0.01);
    // Q(1) = 0.1587; the byte's step of 0.031 about 1 moves the share by about 0.008.
    expect(Math.abs(aboveOne / TILE.length - 0.1587)).toBeLessThan(0.01);
  });

  it("filters to its stated standard deviation over a fixed sequence of points", () => {
    let sum = 0;
    let squares = 0;
    const count = 1 << 16;
    for (let i = 0; i < count; i += 1) {
      const n = sampleCloudNoise(TILE, r3(i));
      sum += n;
      squares += n * n;
    }
    const mean = sum / count;
    expect(Math.sqrt(squares / count - mean * mean)).toBeCloseTo(CLOUD_NOISE_STD, 2);
  });

  it("repeats at the tile's edge and reads a texel's own value at its centre", () => {
    const u = { x: 0.137, y: 0.861, z: 0.402 };
    expect(sampleCloudNoise(TILE, { x: u.x + 1, y: u.y - 2, z: u.z + 3 })).toBeCloseTo(
      sampleCloudNoise(TILE, u),
      12,
    );
    const [i, j, k] = [5, 17, 40];
    const centre = {
      x: (i + 0.5) / CLOUD_NOISE_SIZE,
      y: (j + 0.5) / CLOUD_NOISE_SIZE,
      z: (k + 0.5) / CLOUD_NOISE_SIZE,
    };
    const stored = TILE[(k * CLOUD_NOISE_SIZE + j) * CLOUD_NOISE_SIZE + i] ?? -1;
    expect(sampleCloudNoise(TILE, centre)).toBeCloseTo(decodeCloudNoise(stored), 12);
  });
});

describe("inverseNormal", () => {
  it("meets the standard normal's quantiles in each of its three regions", () => {
    expect(inverseNormal(0.5)).toBe(0);
    expect(inverseNormal(0.975)).toBeCloseTo(1.959_963_985, 8);
    expect(inverseNormal(0.001)).toBeCloseTo(-3.090_232_306, 8);
    expect(inverseNormal(0.999_99)).toBeCloseTo(4.264_890_794, 7);
  });

  it("is odd about one half", () => {
    for (const p of [0.01, 0.1, 0.3]) {
      expect(inverseNormal(1 - p)).toBeCloseTo(-inverseNormal(p), 9);
    }
  });
});

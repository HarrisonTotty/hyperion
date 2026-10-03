import { describe, expect, it } from "vitest";

import {
  packRgb9e5,
  packRgb9e5Texels,
  RGB9E5_MAX,
  RGB9E5_MIN_POSITIVE,
  unpackRgb9e5,
} from "./pack";

describe("packRgb9e5", () => {
  it("holds the extension's largest value, 65,408, and clamps above it", () => {
    expect(RGB9E5_MAX).toBe(65_408);
    // Exponent 31, mantissa 511 on every channel.
    expect(packRgb9e5(RGB9E5_MAX, RGB9E5_MAX, RGB9E5_MAX)).toBe(0xffff_ffff);
    expect(packRgb9e5(1e9, 0, 0)).toBe(packRgb9e5(RGB9E5_MAX, 0, 0));
    expect(unpackRgb9e5(packRgb9e5(1e9, 0, 0))).toEqual([65_408, 0, 0]);
  });

  it("holds the smallest positive value, 2⁻²⁴, as exponent 0 and mantissa 1", () => {
    expect(RGB9E5_MIN_POSITIVE).toBe(2 ** -24);
    expect(packRgb9e5(2 ** -24, 0, 0)).toBe(1);
    expect(unpackRgb9e5(1)).toEqual([2 ** -24, 0, 0]);
    expect(packRgb9e5(2 ** -26, 0, 0)).toBe(0);
  });

  it("raises the exponent where the largest mantissa rounds up to 2⁹", () => {
    // 0.99999 takes exponent 15 at first; its mantissa rounds to 512, so it packs as 1.0.
    expect(packRgb9e5(0.999_99, 0, 0)).toBe(packRgb9e5(1, 0, 0));
    expect(packRgb9e5(1, 0, 0)).toBe(((16 << 27) | 256) >>> 0);
    expect(unpackRgb9e5(packRgb9e5(0.999_99, 0, 0))).toEqual([1, 0, 0]);
  });

  it("packs 1.0 as exponent 16 and mantissa 256, and three channels on one exponent", () => {
    expect(unpackRgb9e5(packRgb9e5(1, 0.5, 0.25))).toEqual([1, 0.5, 0.25]);
    // Green's 2⁻¹⁰ is under half a step at red's exponent (2⁻⁸) and packs to zero.
    expect(unpackRgb9e5(packRgb9e5(1, 2 ** -10, 0))).toEqual([1, 0, 0]);
  });

  it("rounds a mantissa just below a half down, as the WGSL packer does", () => {
    // At red's exponent (step 2⁻⁸), green is (½ − 2⁻²⁵) steps: an f32 sum q + ½ would round to 1.
    const green = Math.fround((0.5 - 2 ** -25) * 2 ** -8);
    expect(unpackRgb9e5(packRgb9e5(1, green, 0))).toEqual([1, 0, 0]);
  });

  it("takes NaN and negative channels as zero", () => {
    expect(packRgb9e5(Number.NaN, -1, 0)).toBe(0);
  });

  it("round-trips within half a step of the shared exponent", () => {
    for (const value of [3.3e-4, 0.017, 1.5, 123.456, 40_000]) {
      const [r, g] = unpackRgb9e5(packRgb9e5(value, value / 3, 0));
      expect(Math.abs(r - value) / value).toBeLessThanOrEqual(2 ** -9);
      expect(Math.abs(g - value / 3)).toBeLessThanOrEqual(value * 2 ** -9);
    }
  });

  it("rounds below 2⁻¹⁶ to the nearest 2⁻²⁴, where the exponent stops at 0", () => {
    const [r] = unpackRgb9e5(packRgb9e5(3.3e-7, 0, 0));
    expect(Math.abs(r - 3.3e-7)).toBeLessThanOrEqual(2 ** -25);
  });

  it("packs a level of rgba texels in order, ignoring alpha", () => {
    const packed = packRgb9e5Texels(new Float32Array([1, 0, 0, 7, 0, 0.5, 0, 9]));
    expect([...packed]).toEqual([packRgb9e5(1, 0, 0), packRgb9e5(0, 0.5, 0)]);
    expect(() => packRgb9e5Texels(new Float32Array(3))).toThrow(/whole rgba texels/);
  });
});

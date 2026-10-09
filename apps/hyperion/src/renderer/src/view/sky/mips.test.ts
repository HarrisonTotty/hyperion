import { describe, expect, it } from "vitest";

import { texelSolidAnglesSr } from "./cube";
import {
  cubeLevels,
  divideBySolidAngle,
  faceMipChain,
  mipSizes,
  mipStep,
  peakScaleExponent,
  scaleByPowerOfTwo,
} from "./mips";

/** Σ L Ω over a face's red channel, in `f64`. */
function redFlux(face: Float32Array, sizePx: number): number {
  const omegas = texelSolidAnglesSr(sizePx);
  let flux = 0;
  for (let texel = 0; texel < sizePx * sizePx; texel += 1) {
    flux += (face[texel * 4] ?? 0) * (omegas[texel] ?? 0);
  }
  return flux;
}

/** A face of seeded, uneven luminances spanning six decades. */
function unevenFace(sizePx: number): Float32Array {
  const face = new Float32Array(sizePx * sizePx * 4);
  let state = 12_345;
  for (let i = 0; i < face.length; i += 1) {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    face[i] = 10 ** ((state / 2_147_483_648) * 6 - 3);
  }
  return face;
}

describe("the cube's levels", () => {
  it("steps 3,072 down by halves to 3 and then 3 × 3 to 1", () => {
    const sizes = mipSizes(3_072);
    expect(sizes).toEqual([3_072, 1_536, 768, 384, 192, 96, 48, 24, 12, 6, 3, 1]);
    // The engine sizes level l as max(1, size >> l), and agrees at every level.
    expect(sizes.map((_, level) => Math.max(1, 3_072 >> level))).toEqual(sizes);
    expect(mipStep(3)).toBe(3);
  });

  it("steps 1,024 down by halves in 11 levels", () => {
    expect(mipSizes(1_024)).toHaveLength(11);
  });

  it("refuses a level side with no mip step", () => {
    expect(() => mipStep(5)).toThrow(/no mip step/);
  });

  it("refuses a face or solid angles of the wrong size", () => {
    expect(() => faceMipChain(new Float32Array(4 * 4 * 4), 3)).toThrow(/not a 3² face/);
    expect(() => divideBySolidAngle(new Float32Array(16), 2, texelSolidAnglesSr(3))).toThrow(
      /do not cover/,
    );
  });

  it("ignores a non-finite texel when it sets the scale", () => {
    expect(peakScaleExponent([new Float32Array([Number.NaN, 1, 0, 0])])).toBe(15);
  });

  it("conserves each level's flux, Σ L Ω, to 10⁻⁶", () => {
    for (const size of [48, 96]) {
      const chain = faceMipChain(unevenFace(size), size);
      const flux0 = redFlux(chain[0] ?? new Float32Array(), size);
      chain.forEach((level, index) => {
        const levelSize = Math.max(1, size >> index);
        expect(Math.abs(redFlux(level, levelSize) / flux0 - 1)).toBeLessThan(1e-6);
      });
    }
  });

  it("filters 3,072's last step over 3 × 3 texels into one", () => {
    // A 6-texel face exercises the same 6 → 3 → 1 tail as 3,072's.
    const face = unevenFace(6);
    const chain = faceMipChain(face, 6);
    expect(chain.map((level) => level.length / 4)).toEqual([36, 9, 1]);
    const omegas = texelSolidAnglesSr(3);
    let weighted = 0;
    let total = 0;
    for (let texel = 0; texel < 9; texel += 1) {
      weighted += (chain[1]?.[texel * 4] ?? 0) * (omegas[texel] ?? 0);
      total += omegas[texel] ?? 0;
    }
    expect(chain[2]?.[0]).toBeCloseTo(weighted / total, 4);
  });

  it("gives a face's texels the solid angles of a sixth of the sphere", () => {
    for (const size of [1, 3, 64]) {
      const sum = texelSolidAnglesSr(size).reduce((total, omega) => total + omega, 0);
      expect(sum).toBeCloseTo((4 * Math.PI) / 6, 12);
    }
    const omegas = texelSolidAnglesSr(64);
    // The corner texel is the smallest, the centre's the largest, by about 3√3.
    expect((omegas[32 * 64 + 32] ?? 0) / (omegas[0] ?? 1)).toBeGreaterThan(5);
  });

  it("turns splatted illuminance into luminance by each texel's solid angle", () => {
    const face = new Float32Array([2, 4, 6, 1]);
    divideBySolidAngle(face, 1, texelSolidAnglesSr(1));
    expect(face[0]).toBeCloseTo(2 / ((4 * Math.PI) / 6), 6);
    expect(face[3]).toBe(1);
  });

  it("scales the brightest channel to at most 2¹⁵ and above 2¹⁴ by an exact power of two", () => {
    for (const peak of [1e-9, 0.75, 1, 2 ** 15, 3e7]) {
      const faces = [new Float32Array([peak, peak / 2, 0, 1]), new Float32Array([0, 0, 0, 0])];
      const exponent = peakScaleExponent(faces);
      const before = faces[0]?.[1] ?? 0;
      faces.forEach((face) => scaleByPowerOfTwo(face, exponent));
      const scaled = faces[0]?.[0] ?? 0;
      expect(scaled).toBeLessThanOrEqual(2 ** 15);
      expect(scaled).toBeGreaterThan(2 ** 14);
      expect((faces[0]?.[1] ?? 0) * 2 ** -exponent).toBe(before);
    }
    expect(peakScaleExponent([new Float32Array(4)])).toBe(0);
  });

  it("joins six faces' chains into levels, face after face", () => {
    const chains = Array.from({ length: 6 }, (_, face) =>
      faceMipChain(new Float32Array(4 * 4 * 4).fill(face), 4),
    );
    const levels = cubeLevels(chains);
    expect(levels.map((level) => level.length)).toEqual([6 * 16 * 4, 6 * 4 * 4, 6 * 4]);
    expect(levels[2]?.[5 * 4]).toBeCloseTo(5, 6);
    expect(() => cubeLevels(chains.slice(1))).toThrow(/6 faces/);
  });
});

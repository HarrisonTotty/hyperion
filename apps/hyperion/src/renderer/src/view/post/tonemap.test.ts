import { describe, expect, it } from "vitest";

import {
  AGX_MAX_EV,
  AGX_MIN_EV,
  agxSigmoid,
  spriteToneCurve,
  toneCurve,
} from "../photometry/toneCurve";
import { srgbEncode, tonemapTexel, tpdf } from "./tonemap";

const grey = (v: number): readonly [number, number, number] => [v, v, v];

/**
 * Blender's AgX Base sRGB on the grey diagonal, 8-bit code × 255 at whole stops about 0.18 from −9
 * to +6: read once from Blender's `AgX_Base_sRGB.cube` (blender/blender, main,
 * release/datafiles/colormanagement/luts, fetched 2026-10-02; GPL, the LUT itself not committed),
 * input log₂(x ÷ 0.18) over −10 to +15 stops, linear between its 57 diagonal entries.
 */
const BLENDER_GREY_CODES: ReadonlyArray<readonly [number, number]> = [
  [-9, 2.72],
  [-8, 6.09],
  [-7, 10.34],
  [-6, 15.79],
  [-5, 23.02],
  [-4, 32.75],
  [-3, 46.1],
  [-2, 64.69],
  [-1, 90.74],
  [0, 124.8],
  [1, 159.64],
  [2, 188.35],
  [3, 210.57],
  [4, 227.52],
  [5, 240.59],
  [6, 250.75],
];

describe("AgX in the tone-mapping pass", () => {
  it("takes Filament's formula at pinned grey inputs", () => {
    // Computed in f64 from Filament's AgxToneMapper (the sigmoid of log₂ x between AgxMinEv and
    // AgxMaxEv, then pow 2.2), for grey, whose matrices cancel.
    for (const x of [0.01, 0.18, 1, 4]) {
      const e = (Math.log2(x) - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV);
      expect(toneCurve(grey(x))[1]).toBeCloseTo(agxSigmoid(e) ** 2.2, 3);
    }
    expect(Math.abs(toneCurve(grey(0.18))[1] - 0.2148)).toBeLessThan(1e-4);
  });

  it("is monotone in luminance above the toe's recovery", () => {
    let previous = -1;
    // The polynomial's toe dips below its black value up to about 3.4 × 10⁻⁴, 9 stops under 0.18.
    for (let stops = -9; stops <= 10; stops += 0.05) {
      const code = tonemapTexel(grey(0.18 * 2 ** stops), 1, null)[1];
      expect(code).toBeGreaterThanOrEqual(previous);
      previous = code;
    }
  });

  it("draws an isolated star identically in both styles before the dither", () => {
    // The wireframe's sprite writes spriteToneCurve through the sRGB view, which encodes.
    for (const light of [grey(0), grey(1e-3), [3, 1, 0.2] as const, grey(500)]) {
      const sprite = spriteToneCurve(light).map(srgbEncode);
      expect(tonemapTexel(light, 1, null)).toEqual(sprite);
    }
  });

  it("stays within one code of the undithered value after the dither", () => {
    for (const u of [0, 0.1, 0.5, 0.9, 1]) {
      const noise = grey(tpdf(u));
      const plain = tonemapTexel(grey(0.05), 1, null);
      const dithered = tonemapTexel(grey(0.05), 1, noise);
      expect(Math.abs(dithered[1] - plain[1]) * 255).toBeLessThanOrEqual(1);
    }
  });

  it("meets Blender's AgX Base sRGB within 6 codes from −3 stops up, and within 16 in the toe", () => {
    // The toe's gap is the seventh-order polynomial's (plan R07, Risks, T15 finding).
    for (const [stops, code] of BLENDER_GREY_CODES) {
      const ours = tonemapTexel(grey(0.18 * 2 ** stops), 1, null)[1] * 255;
      expect(Math.abs(Math.round(ours) - Math.round(code))).toBeLessThanOrEqual(
        stops >= -3 ? 6 : 16,
      );
    }
  });
});

describe("the dither's noise", () => {
  it("is triangular over [−1, 1], symmetric about 0", () => {
    expect(tpdf(0)).toBe(-1);
    expect(tpdf(1)).toBe(1);
    expect(tpdf(0.5)).toBeCloseTo(0, 12);
    expect(tpdf(0.25)).toBeCloseTo(-tpdf(0.75), 12);
    // The triangular distribution's CDF at −0.5 is 1/8.
    expect(tpdf(0.125)).toBeCloseTo(-0.5, 12);
  });

  it("encodes by the sRGB curve", () => {
    expect(srgbEncode(0)).toBe(0);
    expect(srgbEncode(1)).toBeCloseTo(1, 12);
    expect(srgbEncode(0.0031308)).toBeCloseTo(0.04045, 5);
  });
});

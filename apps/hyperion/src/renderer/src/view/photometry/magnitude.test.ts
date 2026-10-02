import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { pixelSolidAngle } from "../camera/projection";
import { exposureScale } from "./exposure";
import {
  apparentV,
  erf,
  illuminanceLx,
  PARSEC_M,
  pixelLuminance,
  PSF_QUAD_PX,
  psfPixelWeights,
  type SubpixelOffset,
} from "./magnitude";
import { spriteToneCurve } from "./toneCurve";

/** The centre pixel of a 1920 × 1080, 60° view, sr. */
const CENTRE_PIXEL_SR = pixelSolidAngle(
  vec3(0, 0, -1),
  { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 },
  { widthPx: 1920, heightPx: 1080 },
);

/** 100 sub-pixel positions on a 10 × 10 grid over the pixel. */
const GRID: readonly SubpixelOffset[] = Array.from({ length: 100 }, (_, i) => ({
  xPx: -0.45 + 0.1 * (i % 10),
  yPx: -0.45 + 0.1 * Math.floor(i / 10),
}));

/** A grey star's displayed total and peak, summed over its quad after the sprite's tone curve. */
function displayed(v: number, ev100: number, at: SubpixelOffset): { total: number; peak: number } {
  const e = illuminanceLx(v);
  const scale = exposureScale(ev100);
  let total = 0;
  let peak = 0;
  for (const weight of psfPixelWeights(at)) {
    const exposed = pixelLuminance(e, weight, CENTRE_PIXEL_SR) * scale;
    const value = spriteToneCurve([exposed, exposed, exposed])[1];
    total += value;
    peak = Math.max(peak, value);
  }
  return { total, peak };
}

describe("magnitude to illuminance", () => {
  it("gives 2.54 µlx for V = 0", () => {
    expect(illuminanceLx(0)).toBeCloseTo(2.54e-6, 12);
  });

  it("gives the Sun's 1.28 × 10⁵ lx for V = −26.76 within 1%", () => {
    expect(illuminanceLx(-26.76) / 1.28e5).toBeCloseTo(1, 2);
  });

  it("gives the absolute magnitude at 10 pc and five magnitudes more at 100 pc", () => {
    expect(apparentV(4.83, 10 * PARSEC_M)).toBeCloseTo(4.83, 12);
    expect(apparentV(4.83, 100 * PARSEC_M)).toBeCloseTo(9.83, 12);
  });
});

describe("pixel luminance", () => {
  it("makes a mag 6.5 star wholly in the centre pixel of a 1080p 60° view 1.76 × 10⁻² cd/m² within 1%", () => {
    expect(pixelLuminance(illuminanceLx(6.5), 1, CENTRE_PIXEL_SR) / 1.76e-2).toBeCloseTo(1, 2);
  });
});

describe("psfPixelWeights", () => {
  it("sums to 1 within 10⁻⁹ at 100 sub-pixel positions", () => {
    const sums = GRID.map((at) => psfPixelWeights(at).reduce((sum, w) => sum + w, 0));
    expect(Math.max(...sums.map((sum) => Math.abs(sum - 1)))).toBeLessThan(1e-9);
  });

  it("keeps the brightest pixel's weight within a factor of 2.5 across them", () => {
    const peaks = GRID.map((at) => Math.max(...psfPixelWeights(at)));
    expect(Math.max(...peaks) / Math.min(...peaks)).toBeLessThan(2.5);
  });

  it("puts 0.32 of a centred star's light in its own pixel", () => {
    const centre = psfPixelWeights({ xPx: 0, yPx: 0 })[(PSF_QUAD_PX * PSF_QUAD_PX - 1) / 2];
    expect(centre).toBeCloseTo(0.32, 2);
  });

  it("is built on an error function accurate to 10⁻¹⁴", () => {
    // erf(0.5), erf(1), erf(3) (Abramowitz and Stegun, table 7.1).
    expect(Math.abs(erf(0.5) - 0.520_499_877_813_046_5)).toBeLessThan(1e-14);
    expect(Math.abs(erf(1) - 0.842_700_792_949_714_9)).toBeLessThan(1e-14);
    expect(Math.abs(erf(3) - 0.999_977_909_503_001_4)).toBeLessThan(1e-14);
    expect(erf(-1)).toBe(-erf(1));
  });
});

describe("a star's displayed total as it moves across a pixel", () => {
  const MAGNITUDES = [6.5, 6, 5, 4, 3, 2, 1, 0, -1, -1.46];
  const EXPOSURES = [-1, 0, 1, 2];

  it("stays within 0.90–1.05 of the centred star's for every visible star, and within 0.8–1.25 for the rest", () => {
    const outside: string[] = [];
    for (const v of MAGNITUDES) {
      for (const ev100 of EXPOSURES) {
        const centred = displayed(v, ev100, { xPx: 0, yPx: 0 });
        const [low, high] = centred.peak >= 1e-3 ? [0.9, 1.05] : [0.8, 1.25];
        for (const at of GRID) {
          const ratio = displayed(v, ev100, at).total / centred.total;
          if (!(ratio >= low && ratio <= high)) {
            outside.push(`V ${v} EV100 ${ev100} at ${at.xPx},${at.yPx}: ${ratio}`);
          }
        }
      }
    }
    expect(outside).toEqual([]);
  });
});

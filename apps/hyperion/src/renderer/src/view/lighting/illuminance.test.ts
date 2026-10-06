import { describe, expect, it } from "vitest";

import { aHostDisc, SUN_ABSOLUTE_V } from "../../test/litFixtures";
import { apparentV, illuminanceLx } from "../photometry/magnitude";
import {
  photopicIlluminance,
  shiningStars,
  STAR_CUT_RELATIVE,
  starIlluminance,
} from "./illuminance";

/** One au, m (IAU 2012 Resolution B2). */
const AU_M = 1.495_978_707e11;

/** Mars's semi-major axis, au (NASA Mars fact sheet). */
const MARS_AU = 1.523_7;

/** The speed of light, m/s (exact). */
const C_M_S = 299_792_458;

describe("starIlluminance", () => {
  const sun = aHostDisc();

  it("gives 1.28 × 10⁵ lx from the Sun at 1 au", () => {
    // V = −26.76 at 1 au (Willmer 2018) and V = 0 at 2.54 µlx (Cox 2000, §15).
    // 1.287 × 10⁵ lx, the 1.28 of the brainstorm's rounding.
    expect(Math.abs(photopicIlluminance(starIlluminance(sun, AU_M)) / 1.28e5 - 1)).toBeLessThan(
      0.01,
    );
  });

  it("agrees with the magnitude form to 1%", () => {
    const disc = photopicIlluminance(starIlluminance(sun, AU_M));
    const magnitude = illuminanceLx(apparentV(SUN_ABSOLUTE_V, AU_M)) * sun.lux_per_v0;
    expect(Math.abs(disc / magnitude - 1)).toBeLessThan(0.01);
  });

  it("falls with the inverse square to Mars", () => {
    const earth = photopicIlluminance(starIlluminance(sun, AU_M));
    const mars = photopicIlluminance(starIlluminance(sun, MARS_AU * AU_M));
    expect(mars / earth).toBeCloseTo(1 / MARS_AU ** 2, 10);
  });

  it("orders the channels r, g, b from the disc's B, V, R", () => {
    const [r, g, b] = starIlluminance(sun, AU_M);
    const [lb, lv, lr] = sun.mean_luminance_cd_m2;
    expect(r / g).toBeCloseTo(lr / lv, 12);
    expect(b / g).toBeCloseTo(lb / lv, 12);
  });

  it("gives the V-band channel split's luminance as the photopic illuminance", () => {
    const channels = starIlluminance(sun, AU_M);
    const photopic = Math.PI * (sun.radius_m / AU_M) ** 2;
    const [lb, lv, lr] = sun.mean_luminance_cd_m2;
    expect(photopicIlluminance(channels)).toBeCloseTo(
      photopicIlluminance([photopic * lr, photopic * lv, photopic * lb]),
      6,
    );
    // The fixture's colour has unit luminance, so the sum is the photopic mean's illuminance.
    expect(
      photopicIlluminance(channels) / photopicIlluminance([lr, lv, lb]) / photopic,
    ).toBeCloseTo(1, 12);
  });

  it("turns the light by under 10⁻⁷ rad where the star's reflex motion is left out", () => {
    // The Sun's reflex speed about the Sun–Jupiter barycentre: Jupiter's orbital speed, 13.07 km/s,
    // times its mass over the total, 1.8982 × 10²⁷ ÷ (1.9885 × 10³⁰ + 1.8982 × 10²⁷) kg (NASA fact
    // sheets); the direction moves by v ÷ c.
    const reflexMS = 13_070 * (1.8982e27 / (1.9885e30 + 1.8982e27));
    expect(reflexMS / C_M_S).toBeLessThan(1e-7);
    expect(reflexMS / C_M_S).toBeCloseTo(4.16e-8, 9);
  });
});

describe("shiningStars", () => {
  const sun = aHostDisc();

  it("drops a companion under 10⁻⁴ of the brightest", () => {
    const bright = starIlluminance(sun, AU_M);
    const scale = STAR_CUT_RELATIVE * 0.5;
    const faint = [bright[0] * scale, bright[1] * scale, bright[2] * scale] as const;
    expect(shiningStars([bright, faint])).toEqual([0]);
  });

  it("keeps a companion at or above the cut", () => {
    const bright = starIlluminance(sun, AU_M);
    const companion = starIlluminance(sun, AU_M * 50);
    expect(shiningStars([companion, bright])).toEqual([0, 1]);
  });

  it("lights with nothing when no star shines", () => {
    expect(shiningStars([[0, 0, 0]])).toEqual([]);
  });
});

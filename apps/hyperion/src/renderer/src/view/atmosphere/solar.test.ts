import { describe, expect, it } from "vitest";

import { photopicIlluminance } from "../lighting/illuminance";
import { SUN_ABSOLUTE_V } from "../lighting/hostDisc";
import { illuminanceLx, PARSEC_M } from "../photometry/magnitude";
import { AU_M } from "../scenes/kept";
import { R06_SOLAR_LUX_PER_V0, referenceSunIlluminanceLx } from "./earth";
import { deltaUv } from "./spectralColour";
import {
  E490_ILLUMINANCE_LX,
  rec709Luminance,
  SKY_SPECTRAL_TO_LUMINANCE,
  skyLuminanceScale,
  SUN_APPARENT_V,
  SUN_SPECTRAL_TO_LUMINANCE,
  sunIlluminanceRgb,
} from "./solar";

describe("the Sun's light in three channels", () => {
  it("has the luminance R02's photometry gives for V = −26.76", () => {
    expect(rec709Luminance(sunIlluminanceRgb()) / illuminanceLx(SUN_APPARENT_V)).toBeCloseTo(1, 12);
  });

  it("is within 5% of E-490's own photopic illuminance before scaling", () => {
    expect(Math.abs(illuminanceLx(SUN_APPARENT_V) / E490_ILLUMINANCE_LX - 1)).toBeLessThan(0.05);
  });

  it("is near white: no channel more than 10% from the luminance", () => {
    const rgb = sunIlluminanceRgb();
    const y = rec709Luminance(rgb);
    for (const c of rgb) {
      expect(Math.abs(c / y - 1)).toBeLessThan(0.1);
    }
  });

  // R08.T6.d draws the Sun from R07's `starIlluminance` through R06's colour; this file stays as the
  // check of it (plan R08, R08.T6.d; the figures are recorded in R08's Risks).
  it("agrees with R08.T6.d's Sun in luminance but for R06's photopic factor of the Sun's spectrum", () => {
    const r06 = photopicIlluminance(referenceSunIlluminanceLx());
    const r05 = rec709Luminance(sunIlluminanceRgb());
    // R02's V photometry takes 2.54 µlx a V = 0 star whatever its spectrum, R06 1.0367 times that
    // for the Sun's, which is E-490's own 133,318 lx; M_V 4.81 at 1 au is V −26.762 (−26.76 here).
    const vOffset =
      10 ** (-0.4 * (SUN_ABSOLUTE_V + 5 * Math.log10(AU_M / (10 * PARSEC_M)) - SUN_APPARENT_V));
    expect(r06 / r05 / (R06_SOLAR_LUX_PER_V0 * vOffset) - 1).toBeCloseTo(0, 6);
    expect(Math.abs(r06 / E490_ILLUMINANCE_LX - 1)).toBeLessThan(0.01);
  });

  // R02's 2.54 µlx for V = 0 takes no account of the spectrum, a hot star's lux per unit of V flux
  // (the ruling's "Vega's"; R06's table gives an A0 spectrum 1.006 of it); R06's photometry of the
  // Sun's own spectrum is right (plan R08, R08.T6.j).
  it("takes R02's spectrum-blind zero point: 3.7% under R06's Sun and 3.6% under E-490's", () => {
    const r05 = rec709Luminance(sunIlluminanceRgb());
    expect(r05).toBeCloseTo(128_479, -1);
    expect(1 - r05 / photopicIlluminance(referenceSunIlluminanceLx())).toBeCloseTo(0.037, 3);
    expect(1 - r05 / E490_ILLUMINANCE_LX).toBeCloseTo(0.036, 3);
  });

  it("differs from R08.T6.d's Sun in chromaticity by 0.0012 in u′v′", () => {
    // Measured 2026-10-12: 0.00120, solar.ts's three spectral samples of E-490 against R06's colour
    // of the ATLAS9 Sun (plan R08's Risks, "Deviations in T6.d, as built").
    expect(deltaUv(sunIlluminanceRgb(), referenceSunIlluminanceLx())).toBeCloseTo(0.0012, 4);
  });

  it("weights the sky's red above the Sun's and its blue below, as the λ⁻³ shape does", () => {
    expect(SKY_SPECTRAL_TO_LUMINANCE[0]).toBeGreaterThan(SUN_SPECTRAL_TO_LUMINANCE[0]);
    expect(SKY_SPECTRAL_TO_LUMINANCE[2]).toBeLessThan(SUN_SPECTRAL_TO_LUMINANCE[2]);
    const sun = sunIlluminanceRgb();
    const sky = skyLuminanceScale();
    for (const c of [0, 1, 2] as const) {
      expect(sky[c] / sun[c]).toBeCloseTo(
        SKY_SPECTRAL_TO_LUMINANCE[c] / SUN_SPECTRAL_TO_LUMINANCE[c],
        12,
      );
    }
  });
});

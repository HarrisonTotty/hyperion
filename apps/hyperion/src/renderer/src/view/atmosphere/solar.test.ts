import { describe, expect, it } from "vitest";

import { illuminanceLx } from "../photometry/magnitude";
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

import { describe, expect, it } from "vitest";

import type { ExposureTriple } from "../photometry/exposure";
import { cameraLimitParts, cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";

/** A sky of surface brightness μ, mag arcsec⁻², as a luminance: B = 10^((12.58 − μ) ÷ 2.5). */
function skyOf(mu: number): number {
  return 10 ** ((12.58 - mu) / 2.5);
}

/** Design note 18's f/1.4 and 1/30 s at high gain, ISO 409,600, where read noise is σ_pre's alone. */
const HIGH_GAIN: ExposureTriple = { aperture: 1.4, shutterS: 1 / 30, iso: 409_600 };

describe("cameraLimitV", () => {
  it("reaches V 9.95, 11.65 and 13.5 at 60°, 30° and 13° in a dark sky at high gain", () => {
    const limits = [60, 30, 13].map((fov) =>
      cameraLimitV(DEFAULT_VIEW_CAMERA, HIGH_GAIN, fov, skyOf(24)),
    );
    [9.95, 11.65, 13.5].forEach((expected, i) => {
      expect(Math.abs((limits[i] ?? 0) - expected)).toBeLessThan(0.3);
    });
  });

  it("reaches V 9.4, 11.05 and 12.9 at base sensitivity", () => {
    const base = { ...HIGH_GAIN, iso: 100 };
    const limits = [60, 30, 13].map((fov) =>
      cameraLimitV(DEFAULT_VIEW_CAMERA, base, fov, skyOf(24)),
    );
    [9.4, 11.05, 12.9].forEach((expected, i) => {
      expect(Math.abs((limits[i] ?? 0) - expected)).toBeLessThan(0.3);
    });
  });

  it("gives Design note 18's 6 electrons of sky a pixel at 60° and μ 22.4", () => {
    const parts = cameraLimitParts(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 60, skyOf(22.4));
    expect(Math.abs(parts.skyElectrons - 5.8)).toBeLessThan(0.3);
  });

  it("reproduces Design note 18's first figures exactly at η 1.8", () => {
    const first = { ...DEFAULT_VIEW_CAMERA, etaSun: 1.8 };
    expect(cameraLimitV(first, HIGH_GAIN, 60, skyOf(22.4))).toBeCloseTo(9.4, 1);
    expect(cameraLimitV(first, HIGH_GAIN, 60, skyOf(24))).toBeCloseTo(9.55, 1);
  });

  it("loses at least 5 mag over 21 stops of exposure, 12 of them gain", () => {
    const dark = skyOf(24);
    const before = cameraLimitV(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 60, dark);
    // 9 stops from photons: 4 from the aperture (f/1.4 → f/5.6), 5 from the shutter (÷ 32).
    const after = cameraLimitV(
      DEFAULT_VIEW_CAMERA,
      { aperture: 5.6, shutterS: 1 / 960, iso: HIGH_GAIN.iso / 4_096 },
      60,
      dark,
    );
    expect(before - after).toBeGreaterThanOrEqual(5);
  });

  it("falls as the band behind the stars brightens", () => {
    const limits = [24, 22, 20, 18].map((mu) =>
      cameraLimitV(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 60, skyOf(mu)),
    );
    for (let i = 1; i < limits.length; i += 1) {
      expect(limits[i]).toBeLessThan(limits[i - 1] ?? Number.NEGATIVE_INFINITY);
    }
  });

  it("takes the read noise to 5 electrons at base sensitivity", () => {
    const base = cameraLimitParts(DEFAULT_VIEW_CAMERA, { ...HIGH_GAIN, iso: 100 }, 60, skyOf(24));
    expect(base.readNoiseE).toBeCloseTo(5, 6);
  });

  it("refuses a field of view or an exposure out of range", () => {
    expect(() => cameraLimitV(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 0, 1e-4)).toThrow(RangeError);
    expect(() =>
      cameraLimitV(DEFAULT_VIEW_CAMERA, { ...HIGH_GAIN, shutterS: 0 }, 60, 1e-4),
    ).toThrow(RangeError);
  });
});

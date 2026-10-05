import { describe, expect, it } from "vitest";

import { type ExposureTriple, programTriple, VIEW_CAMERA } from "../photometry/exposure";
import fixture from "../../../../../../../packages/protocol/fixtures/camera_eta_sun.json" with { type: "json" };
import { cameraLimitParts, cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";
import { DARK_SKY_CD_M2 } from "./viewSky";

/** A sky of surface brightness μ, mag arcsec⁻², as a luminance: B = 10^((12.58 − μ) ÷ 2.5). */
function skyOf(mu: number): number {
  return 10 ** ((12.58 - mu) / 2.5);
}

/** Design note 18's f/1.4 and 1/30 s at high gain, ISO 409,600, where read noise is σ_pre's alone. */
const HIGH_GAIN: ExposureTriple = { aperture: 1.4, shutterS: 1 / 30, iso: 409_600 };

describe("the default view camera", () => {
  it("takes the colour table's CAMERA_ETA_SUN, the fixture the sim's test also reads", () => {
    expect(DEFAULT_VIEW_CAMERA.etaSun).toBe(fixture.camera_eta_sun);
  });
});

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

  it("takes a black background as read noise alone, a little deeper than a dark sky", () => {
    const black = cameraLimitParts(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 60, 0);
    expect(black.skyElectrons).toBe(0);
    expect(black.limitV).toBeGreaterThan(
      cameraLimitV(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 60, skyOf(24)),
    );
  });

  it("refuses a field of view or an exposure out of range", () => {
    expect(() => cameraLimitV(DEFAULT_VIEW_CAMERA, HIGH_GAIN, 0, 1e-4)).toThrow(RangeError);
    expect(() =>
      cameraLimitV(DEFAULT_VIEW_CAMERA, { ...HIGH_GAIN, shutterS: 0 }, 60, 1e-4),
    ).toThrow(RangeError);
    expect(() => cameraLimitV(DEFAULT_VIEW_CAMERA, { ...HIGH_GAIN, ndEv: -1 }, 60, 1e-4)).toThrow(
      RangeError,
    );
  });

  it("takes an ND of x EV as the same shutter shortened by 2^−x, to 1E-9 mag", () => {
    for (const ndEv of [0.5, 6.1, 28]) {
      const filtered = cameraLimitV(DEFAULT_VIEW_CAMERA, { ...HIGH_GAIN, ndEv }, 60, skyOf(22.4));
      const shorter = cameraLimitV(
        DEFAULT_VIEW_CAMERA,
        { ...HIGH_GAIN, shutterS: HIGH_GAIN.shutterS * 2 ** -ndEv },
        60,
        skyOf(22.4),
      );
      expect(Math.abs(filtered - shorter)).toBeLessThan(1e-9);
    }
  });

  it("holds the gain at base below ISO 100 and at the top beyond ISO 409,600", () => {
    const at = (iso: number) => cameraLimitV(DEFAULT_VIEW_CAMERA, { ...HIGH_GAIN, iso }, 60, 0);
    expect(at(50)).toBe(at(100));
    expect(at(6e6)).toBe(at(409_600));
  });
});

/** The limit at 60° in a dark sky (μ 24) at the view camera's triple for an EV100. */
function limitAt(ev100: number): number {
  return cameraLimitV(DEFAULT_VIEW_CAMERA, programTriple(VIEW_CAMERA, ev100), 60, DARK_SKY_CD_M2);
}

describe("the view camera's limit through its program", () => {
  it("shares the sensor's base ISO and top gain", () => {
    expect([VIEW_CAMERA.baseIso, VIEW_CAMERA.maxIso]).toEqual([
      DEFAULT_VIEW_CAMERA.baseIso,
      DEFAULT_VIEW_CAMERA.maxIso,
    ]);
  });

  it("reaches V 10.06 at 60° at every EV100 up to 1, where high gain leaves only σ_pre", () => {
    for (const ev100 of [-14, -10, -6.12, -1, 0, 1]) {
      expect(Math.abs(limitAt(ev100) - 10.06)).toBeLessThan(0.01);
    }
  });

  it("falls to V 9.40 at EV100 5.88, 6.32 at 10 and 2.56 at 15, R06's V 2.5–3.5", () => {
    const cases: ReadonlyArray<readonly [number, number]> = [
      [5.88, 9.4],
      [10, 6.32],
      [15, 2.56],
    ];
    for (const [ev100, expected] of cases) {
      expect(Math.abs(limitAt(ev100) - expected)).toBeLessThan(0.01);
    }
  });
});

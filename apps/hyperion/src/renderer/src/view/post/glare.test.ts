import { describe, expect, it } from "vitest";

import {
  cieGlareSpreadRaw,
  glareSourceSolidAngleSr,
  glareSourceVeil,
  glareSpread,
  sphereIntegral,
  type GlareSource,
} from "./glare";

const EYE = { ageYears: 25, pigmentation: 0.5 };
const DEG = Math.PI / 180;

describe("the CIE glare spread function", () => {
  it("integrates to 1.047 at age 25 and pigmentation 0.5 before renormalisation", () => {
    // Plan R07, T14.a; Vos and van den Berg 1999 (CIE 135/1999), with the 0.0046° core constant.
    expect(sphereIntegral((t) => cieGlareSpreadRaw(t, EYE))).toBeCloseTo(1.047, 3);
  });

  it("integrates to 1.010 at pigmentation 0 before renormalisation", () => {
    const dark = { ageYears: 25, pigmentation: 0 };
    expect(sphereIntegral((t) => cieGlareSpreadRaw(t, dark))).toBeCloseTo(1.01, 3);
  });

  it("takes the complete equation's values at 1° and 10°", () => {
    // Hand values of CIE 135/1999's complete equation, A = 25, p = 0.5, computed in f64.
    expect(cieGlareSpreadRaw(1 * DEG, EYE)).toBeCloseTo(19.2665, 3);
    expect(cieGlareSpreadRaw(10 * DEG, EYE)).toBeCloseTo(0.061606, 5);
  });

  it("would integrate to about 37 with the 0.046° the open copies print", () => {
    const wrong = (theta: number): number => {
      const d = theta / DEG;
      const extra = 9.2e6 / (1 + (d / 0.046) ** 2) ** 1.5 - 9.2e6 / (1 + (d / 0.0046) ** 2) ** 1.5;
      return cieGlareSpreadRaw(theta, EYE) + (1 - 0.08 * (25 / 70) ** 4) * extra;
    };
    expect(sphereIntegral(wrong)).toBeGreaterThan(36);
  });
});

describe("glareSpread", () => {
  it("integrates to one over the sphere for an eye and a camera", () => {
    expect(sphereIntegral((t) => glareSpread("eye", t, EYE))).toBeCloseTo(1, 6);
    expect(sphereIntegral((t) => glareSpread("camera", t, EYE))).toBeCloseTo(1, 6);
  });

  it("falls monotonically away from the source", () => {
    for (const role of ["eye", "camera"] as const) {
      let previous = Infinity;
      for (let d = 1e-4; d < 90; d *= 1.5) {
        const value = glareSpread(role, d * DEG, EYE);
        expect(value).toBeLessThan(previous);
        previous = value;
      }
    }
  });

  it("puts a few per cent of a camera's energy beyond a few pixels", () => {
    const fivePx = 5 * ((60 * DEG) / 1920);
    const inside = sphereIntegral((t) => (t <= fivePx ? glareSpread("camera", t, EYE) : 0));
    expect(1 - inside).toBeGreaterThan(0.02);
    expect(1 - inside).toBeLessThan(0.04);
  });
});

describe("glareSourceVeil", () => {
  it("integrates over the sphere to the source's excess illuminance", () => {
    const source: GlareSource = {
      direction: { x: 0, y: 0, z: -1 },
      angularRadiusRad: 0.267 * DEG,
      excessLuminance: [3e9, 2e9, 1e9],
    };
    const omega = glareSourceSolidAngleSr(source);
    for (const role of ["eye", "camera"] as const) {
      for (const channel of [0, 1, 2] as const) {
        const total = sphereIntegral((t) => glareSourceVeil(source, t, role, EYE)[channel]);
        expect(total / (source.excessLuminance[channel] * omega)).toBeCloseTo(1, 6);
      }
    }
  });
});

import { describe, expect, it } from "vitest";

import {
  cieGlareSpreadRaw,
  glareSpreadFunction,
  glareSpreadTerms,
  poissonOverRectangle,
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
        // The veil steps at the limb, where the quadrature in ln θ loses a little.
        expect(Math.abs(total / (source.excessLuminance[channel] * omega) - 1)).toBeLessThan(2e-3);
      }
    }
  });
});

/** The veil of `source` at θ by brute-force quadrature over its disc, polar, for the tests. */
function bruteVeil(source: GlareSource, thetaRad: number, rings: number): number {
  const rho = source.angularRadiusRad;
  const spread = glareSpreadFunction("eye", EYE);
  let sum = 0;
  for (let i = 0; i < rings; i += 1) {
    const r = ((i + 0.5) / rings) * rho;
    const sectors = Math.max(16, Math.floor((6 * rings * r) / rho));
    for (let j = 0; j < sectors; j += 1) {
      const phi = (2 * Math.PI * (j + 0.5)) / sectors;
      const d = Math.hypot(thetaRad - r * Math.cos(phi), r * Math.sin(phi));
      sum += spread(d) * r * (rho / rings) * ((2 * Math.PI) / sectors);
    }
  }
  return sum * source.excessLuminance[1];
}

describe("a resolved source's veil", () => {
  // The Sun at 1 au seen at 1080p across 60°.
  const pxRad = (60 * DEG) / 1920;
  const sun: GlareSource = {
    direction: { x: 0, y: 0, z: -1 },
    angularRadiusRad: 0.267 * DEG,
    excessLuminance: [1, 1, 1],
  };

  it("is within +15% of a quadrature over the disc just outside the limb", () => {
    for (const beyondPx of [0.25, 0.5, 1, 2, 4, 8]) {
      const theta = sun.angularRadiusRad + beyondPx * pxRad;
      const ratio = glareSourceVeil(sun, theta, "eye", EYE)[1] / bruteVeil(sun, theta, 200);
      expect(ratio).toBeGreaterThan(1);
      expect(ratio).toBeLessThan(1.15);
    }
  });

  it("is within 1% of the quadrature by 64 px, where the point form alone is too", () => {
    const theta = sun.angularRadiusRad + 64 * pxRad;
    const truth = bruteVeil(sun, theta, 200);
    const veil = glareSourceVeil(sun, theta, "eye", EYE)[1];
    expect(Math.abs(veil / truth - 1)).toBeLessThan(0.01);
    const point = glareSourceSolidAngleSr(sun) * glareSpread("eye", theta, EYE);
    expect(Math.abs(point / truth - 1)).toBeLessThan(0.02);
  });

  it("tends to the point form far from the source", () => {
    // The rectangle's departure from a point falls as (ρ/θ)²: 0.2% at 5°, under 0.02% at 20°.
    let previous = Infinity;
    for (const thetaDeg of [5, 20, 60]) {
      const veil = glareSourceVeil(sun, thetaDeg * DEG, "eye", EYE)[1];
      const point = glareSourceSolidAngleSr(sun) * glareSpread("eye", thetaDeg * DEG, EYE);
      const departure = Math.abs(veil / point - 1);
      expect(departure).toBeLessThan(0.005);
      expect(departure).toBeLessThan(previous);
      previous = departure;
    }
  });

  it("is the point form for a source far below the narrowest term's scale", () => {
    const star: GlareSource = { ...sun, angularRadiusRad: 1e-9 };
    const theta = 0.01 * DEG;
    const point = glareSourceSolidAngleSr(star) * glareSpread("eye", theta, EYE);
    expect(glareSourceVeil(star, theta, "eye", EYE)[1] / point).toBeCloseTo(1, 9);
  });
});

/** `glare_poisson`'s beyond-the-limb arithmetic in `glare.wgsl`, each step rounded to f32. */
function poissonF32(amplitude: number, c: number, theta: number, rho: number): number {
  const f = Math.fround;
  const y = f(f(Math.PI * rho) / 4);
  const x1 = f(theta - rho);
  const x2 = f(theta + rho);
  const k = f(f(c * c) + f(y * y));
  const s1 = f(Math.sqrt(f(k + f(x1 * x1))));
  const s2 = f(Math.sqrt(f(k + f(x2 * x2))));
  const u1 = f(f(x1 * y) / f(c * s1));
  const u2 = f(f(x2 * y) / f(c * s2));
  const numerator = f(
    f(f(f(y / c) * f(4 * f(rho * theta))) * k) / f(f(f(x2 * s1) + f(x1 * s2)) * f(s1 * s2)),
  );
  return f(f(f(amplitude * f(c * c)) * 2) * f(Math.atan(f(numerator / f(1 + f(u1 * u2))))));
}

describe("the rectangle in f32", () => {
  it("keeps its digits far from small sources, against the f64 form", () => {
    const terms = glareSpreadTerms("eye", EYE);
    for (const rho of [1e-5, 1.55e-4, 0.267 * DEG]) {
      for (const thetaDeg of [1, 10, 30, 60]) {
        for (const term of terms.poisson) {
          if (rho < 0.01 * term.scaleRad) {
            continue;
          }
          const theta = thetaDeg * DEG;
          const exact = poissonOverRectangle(term, theta - rho, theta + rho, (Math.PI * rho) / 4);
          const single = poissonF32(term.amplitude, term.scaleRad, theta, rho);
          expect(Math.abs(single / exact - 1)).toBeLessThan(1e-4);
        }
      }
    }
  });
});

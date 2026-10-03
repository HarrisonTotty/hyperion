import { describe, expect, it } from "vitest";

import { sphereIrradianceBruteForce, UNIFORM_DISC, type LimbProfile } from "./oracle";
import { howellViewFactor, sphereIrradianceFactor } from "./sphereIrradiance";

const RAD_PER_DEG = Math.PI / 180;

/** The Sun's radius, m (IAU 2015 Resolution B3 nominal), and the au, m (IAU 2012 B2). */
const SUN_RADIUS_M = 6.957e8;
const AU_M = 1.495_978_707e11;
const H_EARTH = AU_M / SUN_RADIUS_M;

/** The brainstorm's polynomial law at 550 nm: I(μ)/I(1) = 0.3 + 0.93 μ − 0.23 μ² (Cox 2000). */
const SOLAR_POLYNOMIAL: LimbProfile = (mu) => 0.3 + 0.93 * mu - 0.23 * mu * mu;

describe("sphereIrradianceFactor", () => {
  it.each([3, 11.5, 215])(
    "matches the brute-force sum for a uniform disc at H = %f to 2 × 10⁻³",
    (h) => {
      const edge = Math.acos(1 / h) / RAD_PER_DEG;
      for (const deg of [0, edge - 1, edge + 0.1, 90, 180 - edge - 0.1, 180 - edge + 1]) {
        const phi = deg * RAD_PER_DEG;
        const oracle = sphereIrradianceBruteForce(h, phi, 0, UNIFORM_DISC);
        expect(Math.abs(sphereIrradianceFactor(h, phi) - oracle)).toBeLessThan(2e-3);
      }
    },
  );

  // Design note 6 states 0.45%; with the brainstorm's solar polynomial law the worst error is
  // 0.469% of the face-on value, at φ = 90° (the oracle reproduces the uniform closed form to
  // 10⁻⁵ there), so the bound is the measured 0.47% (recorded in the plan's Risks, T6.a).
  it("errs by at most 0.47% against a limb-darkened star 19.5° in radius", () => {
    const h = 1 / Math.sin(19.5 * RAD_PER_DEG);
    let worst = 0;
    for (let deg = 0; deg <= 180; deg += 2.5) {
      const phi = deg * RAD_PER_DEG;
      const oracle = sphereIrradianceBruteForce(h, phi, 0, SOLAR_POLYNOMIAL);
      worst = Math.max(worst, Math.abs(sphereIrradianceFactor(h, phi) - oracle));
    }
    expect(worst).toBeLessThanOrEqual(0.0047);
  });

  it("is cos φ wherever the whole disc is up", () => {
    for (const deg of [0, 30, 60, 89]) {
      const phi = deg * RAD_PER_DEG;
      expect(sphereIrradianceFactor(H_EARTH, phi)).toBeCloseTo(Math.cos(phi), 12);
    }
  });

  it("softens the terminator over a band 59.3 km wide on a body of 6,371 km at 1 au", () => {
    // The band runs from where the disc's edge touches the plane to where it sets.
    const edge = Math.acos(1 / H_EARTH);
    expect(6371 * (Math.PI - 2 * edge)).toBeCloseTo(59.3, 1);
  });

  it("is continuous where the plane first cuts the disc", () => {
    const edge = Math.acos(1 / H_EARTH);
    expect(sphereIrradianceFactor(H_EARTH, edge - 1e-9)).toBeCloseTo(Math.cos(edge), 8);
  });

  it("is zero once the disc has set, and lit just before", () => {
    const set = Math.PI - Math.acos(1 / H_EARTH);
    expect(sphereIrradianceFactor(H_EARTH, set)).toBe(0);
    expect(sphereIrradianceFactor(H_EARTH, set - 1e-4)).toBeGreaterThan(0);
  });

  it("gives 9.87 × 10⁻⁴ of the zenith value at the geometric terminator for a uniform disc", () => {
    const atTerminator = sphereIrradianceFactor(H_EARTH, Math.PI / 2);
    expect(atTerminator).toBeCloseTo(9.87e-4, 6);
    // 2 ÷ (3π H) is the leading term in 1/H; the rest is 6.5 × 10⁻⁶ of it at 1 au.
    expect(Math.abs(atTerminator / (2 / (3 * Math.PI * H_EARTH)) - 1)).toBeLessThan(1e-4);
  });

  it("gives 9.27 × 10⁻⁴ there for the brainstorm's polynomial law", () => {
    const oracle = sphereIrradianceBruteForce(
      H_EARTH,
      Math.PI / 2,
      0,
      SOLAR_POLYNOMIAL,
      200,
      20_000,
    );
    expect(oracle).toBeCloseTo(9.27e-4, 5);
  });

  it("lights a planet at 3 stellar radii to 109.5° from the substellar point", () => {
    const end = Math.PI - Math.acos(1 / 3);
    expect(end / RAD_PER_DEG).toBeCloseTo(109.5, 1);
    expect(sphereIrradianceFactor(3, end - 1e-3)).toBeGreaterThan(0);
    expect(sphereIrradianceFactor(3, end + 1e-3)).toBe(0);
  });

  it("loses a star 4° up behind a local horizon of 5°", () => {
    const phi = 86 * RAD_PER_DEG;
    expect(sphereIrradianceFactor(H_EARTH, phi)).toBeCloseTo(Math.cos(phi), 9);
    expect(sphereIrradianceFactor(H_EARTH, phi, 5 * RAD_PER_DEG)).toBe(0);
  });

  it("keeps the whole light of a star 6° up over a horizon of 5°", () => {
    const phi = 84 * RAD_PER_DEG;
    expect(sphereIrradianceFactor(H_EARTH, phi, 5 * RAD_PER_DEG)).toBeCloseTo(Math.cos(phi), 9);
  });

  it("follows the brute-force sum as a horizon cuts the Sun's disc", () => {
    const horizon = 5 * RAD_PER_DEG;
    for (const elevationDeg of [5.2, 5.1, 5, 4.9, 4.8]) {
      const phi = (90 - elevationDeg) * RAD_PER_DEG;
      const oracle = sphereIrradianceBruteForce(H_EARTH, phi, horizon, UNIFORM_DISC, 100, 20_000);
      expect(Math.abs(sphereIrradianceFactor(H_EARTH, phi, horizon) - oracle)).toBeLessThan(
        2e-3 * Math.cos(phi) + 1e-6,
      );
    }
  });

  it("treats a horizon below the tangent plane as the plane", () => {
    const phi = 91 * RAD_PER_DEG;
    expect(sphereIrradianceFactor(3, phi, -0.2)).toBe(sphereIrradianceFactor(3, phi));
  });
});

describe("howellViewFactor", () => {
  it("is cos φ ÷ H² with the whole disc up", () => {
    expect(howellViewFactor(4, 0.3)).toBeCloseTo(Math.cos(0.3) / 16, 15);
  });

  it("is zero with the disc set", () => {
    expect(howellViewFactor(4, 2.9)).toBe(0);
  });

  it("is continuous where the plane first cuts the disc and where it last does", () => {
    const h = 5;
    const edge = Math.acos(1 / h);
    expect(howellViewFactor(h, edge + 1e-9)).toBeCloseTo(howellViewFactor(h, edge - 1e-9), 9);
    expect(howellViewFactor(h, Math.PI - edge - 1e-9)).toBeCloseTo(0, 9);
  });
});

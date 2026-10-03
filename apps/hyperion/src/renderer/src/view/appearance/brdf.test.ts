import { describe, expect, it } from "vitest";

import { discIntegral } from "../lighting/oracle";
import { brdf } from "./brdf";
import type { PhotometricLaw } from "./law";
import { discIntegratedPhase, geometricAlbedo } from "./phase";
import { lambertPhase, lommelSeeligerPhase } from "./shapes";

const RAD_PER_DEG = Math.PI / 180;

const PHASES_DEG = [0, 30, 60, 90, 120, 150] as const;

describe("the shapes against the f64 disc integral", () => {
  it("gives a Lambert sphere p = 2A ÷ 3 and its closed-form Φ to 10⁻⁴", () => {
    const albedo = 0.3;
    const p = discIntegral((mu0) => albedo * mu0, 0);
    expect(Math.abs(p - (2 * albedo) / 3)).toBeLessThan(1e-4);
    for (const deg of PHASES_DEG) {
      const alpha = deg * RAD_PER_DEG;
      const phase = discIntegral((mu0) => albedo * mu0, alpha) / p;
      expect(Math.abs(phase - lambertPhase(alpha))).toBeLessThan(1e-4);
    }
  });

  it("gives a Lommel–Seeliger sphere p = ϖ ÷ 8 and its closed-form Φ to 10⁻⁴", () => {
    // I/F = (ϖ ÷ 4) μ₀ ÷ (μ₀ + μ), the law's L = 1 term with A = ϖ ÷ 8.
    const singleScatteringAlbedo = 0.6;
    const reflectance = (mu0: number, mu: number): number =>
      ((singleScatteringAlbedo / 4) * mu0) / (mu0 + mu);
    const p = discIntegral(reflectance, 0);
    expect(Math.abs(p - singleScatteringAlbedo / 8)).toBeLessThan(1e-4);
    for (const deg of PHASES_DEG) {
      const alpha = deg * RAD_PER_DEG;
      const phase = discIntegral(reflectance, alpha) / p;
      expect(Math.abs(phase - lommelSeeligerPhase(alpha))).toBeLessThan(1e-4);
    }
  });
});

describe("brdf", () => {
  const mars: PhotometricLaw = {
    a: [0.38, 0.23, 0.12],
    lommelSeeligerShare: 0.5,
    template: "mars",
    phaseExponent: [1, 1, 1],
  };

  it("is dark where the point is unlit or hidden", () => {
    expect(brdf(mars, 0, 0.5, 1)).toEqual([0, 0, 0]);
    expect(brdf(mars, -0.2, 0.5, 1)).toEqual([0, 0, 0]);
    expect(brdf(mars, 0.5, 0, 1)).toEqual([0, 0, 0]);
  });

  it("is the albedo scale at opposition on a Lommel–Seeliger surface", () => {
    const moon: PhotometricLaw = {
      a: [0.1, 0.12, 0.14],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    };
    const [r, g, b] = brdf(moon, 0.4, 0.4, 0);
    expect(r).toBeCloseTo(0.1, 6);
    expect(g).toBeCloseTo(0.12, 6);
    expect(b).toBeCloseTo(0.14, 6);
  });

  it.each(PHASES_DEG)("integrates over the disc to p Φ(α) of the law at %i° to 10⁻⁴", (deg) => {
    const alpha = deg * RAD_PER_DEG;
    const p = geometricAlbedo(mars);
    const phase = discIntegratedPhase(mars, alpha);
    for (let c = 0; c < 3; c += 1) {
      const integral = discIntegral((mu0, mu, a) => brdf(mars, mu0, mu, a)[c] ?? 0, alpha);
      expect(Math.abs(integral - (p[c] ?? 0) * (phase[c] ?? 0))).toBeLessThan(1e-4);
    }
  });
});

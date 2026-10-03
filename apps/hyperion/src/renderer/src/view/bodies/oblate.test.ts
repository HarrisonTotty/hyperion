import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { shapePhase } from "../appearance/shapes";
import { oblateAlbedoScale, projectedSecondMoment, spheroidGeometricIntegral } from "./oblate";

describe("the spheroid's disc integral", () => {
  it("gives a sphere m = ⅔ at every latitude, and its quadrature agrees", () => {
    expect(projectedSecondMoment(1, 0.4)).toBe(2 / 3);
    expect(projectedSecondMoment(1 - 1e-9, 0)).toBeCloseTo(2 / 3, 4);
    expect(projectedSecondMoment(1 - 1e-9, 1)).toBeCloseTo(2 / 3, 4);
  });

  it("leaves a sphere's albedo unscaled", () => {
    expect(oblateAlbedoScale(0, 1)).toBe(1);
    expect(oblateAlbedoScale(0.5, 1)).toBe(1);
  });

  it("makes an equator-on spheroid's zero-phase disc reach p against π a c", () => {
    const cOverA = 1 - 0.098;
    const pole = vec3(0, 0, 1);
    const view = vec3(1, 0, 0);
    for (const share of [0, 0.5, 1]) {
      // F = (E ÷ π)(a ÷ Δ)² A′ K = E p (a c ÷ Δ²), so p = A′ K ÷ (π c ÷ a), against A [L + ⅔(1 − L)].
      const k = spheroidGeometricIntegral(share, cOverA, pole, view, view);
      const albedo = (oblateAlbedoScale(share, cOverA) * k) / (Math.PI * cOverA);
      expect(albedo / (share + (2 / 3) * (1 - share))).toBeCloseTo(1, 3);
    }
  });

  it("reduces to the sphere's closed form K = π [L + ⅔(1 − L)] Φ_shape(α)", () => {
    const pole = vec3(0, 0, 1);
    const view = vec3(1, 0, 0);
    for (const alphaDeg of [0, 60, 120]) {
      const alpha = (alphaDeg * Math.PI) / 180;
      const star = vec3(Math.cos(alpha), Math.sin(alpha), 0);
      const k = spheroidGeometricIntegral(0, 1 - 1e-12, pole, star, view);
      expect(k / (Math.PI * (2 / 3) * shapePhase(0, alpha))).toBeCloseTo(1, 2);
    }
  });
});

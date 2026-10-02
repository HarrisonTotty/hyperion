import { describe, expect, it } from "vitest";

import { phaseFactor } from "./law";
import { phaseIntegral, shapePhase } from "./shapes";
import { PHASE_TEMPLATES } from "./templates";
import { SOLAR_SYSTEM_PHOTOMETRY, SUN_JOHNSON_MAG } from "../../test/litFixtures";

/** One astronomical unit, km, as Mallama et al. 2017 round it (their eq. 4). */
const AU_KM = 149.6e6;

describe("SOLAR_SYSTEM_PHOTOMETRY", () => {
  it.each(SOLAR_SYSTEM_PHOTOMETRY)(
    "gives $name's p_V from its V(1, 0) and radius to 0.5%",
    (planet) => {
      // Mallama et al. 2017, eqs. 3–4: p = 10^(−0.4 (V − V☉)) ÷ sin²(R ÷ 1 au).
      const ratio = 10 ** (-0.4 * (planet.v10Mag - SUN_JOHNSON_MAG.v));
      const p = ratio / Math.sin(planet.radiusKm / AU_KM) ** 2;
      expect(Math.abs(p / planet.geometricAlbedo.v - 1)).toBeLessThan(0.005);
    },
  );

  it.each(SOLAR_SYSTEM_PHOTOMETRY)("agrees $name's p_B ÷ p_V with its B − V to 3%", (planet) => {
    const sunBMinusV = SUN_JOHNSON_MAG.b - SUN_JOHNSON_MAG.v;
    const { b, v } = planet.geometricAlbedo;
    expect(Math.abs(b / v / 10 ** (-0.4 * (planet.bMinusVMag - sunBMinusV)) - 1)).toBeLessThan(
      0.03,
    );
  });

  it.each(SOLAR_SYSTEM_PHOTOMETRY)("agrees $name's p_R ÷ p_V with its V − R to 3%", (planet) => {
    const sunVMinusR = SUN_JOHNSON_MAG.v - SUN_JOHNSON_MAG.r;
    const { v, r } = planet.geometricAlbedo;
    expect(Math.abs(r / v / 10 ** (0.4 * (planet.vMinusRMag - sunVMinusR)) - 1)).toBeLessThan(0.03);
  });

  it.each(SOLAR_SYSTEM_PHOTOMETRY)("states $name's computed q_V to 0.5%", (planet) => {
    const law = {
      a: [1, 1, 1] as const,
      lommelSeeligerShare: PHASE_TEMPLATES[planet.template].lommelSeeligerShare,
      template: planet.template,
      phaseExponent: [1, 1, 1] as const,
    };
    const q = phaseIntegral(
      (alpha) =>
        phaseFactor(law, alpha)[1] *
        shapePhase(PHASE_TEMPLATES[planet.template].lommelSeeligerShare, alpha),
    );
    expect(Math.abs(q / planet.qV - 1)).toBeLessThan(0.005);
  });

  it("spans Design note 5's measured q_V, 0.48 for Mercury to 1.36 for Saturn", () => {
    const q = SOLAR_SYSTEM_PHOTOMETRY.map((planet) => planet.qV);
    expect(Math.min(...q)).toBeCloseTo(0.48, 2);
    expect(Math.max(...q)).toBeCloseTo(1.36, 2);
  });
});

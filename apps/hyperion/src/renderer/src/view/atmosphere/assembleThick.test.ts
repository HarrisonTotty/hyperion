/**
 * The provisional thick line (plan R08, R08.T10.a; `decision-r08-t10a-leans.md` item 1): a medium
 * whose vertical optical depth over every term but the absorbers reaches 1 on its worst channel
 * carries `ATMOSPHERE: APPROXIMATE` until R08.T13's `classifyRegime`.
 */
import { describe, expect, it } from "vitest";

import { titanClassMethaneMedium, venusClassMedium } from "../../test/atmosphereFixtures";
import { detailOf, recordInput, venus58Detail, venus92Detail } from "../../test/atmosphereRecords";
import { earthDetail } from "../../test/planetaryFixture";
import { assembleMedium, THICK_REASON } from "./assemble";
import { hydrostaticColumn } from "./column";
import { EARTH_DRY_AIR, EARTH_REFERENCE, EARTH_SLAB_REFERENCE } from "./earth";
import { tableRadiusM } from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import { opticalDepthToTop } from "./opticalDepth";
import { molecularTerm } from "./rayleigh";
import {
  EARTH_REFERENCE_RADIUS_M,
  MARS_REFERENCE,
  MARS_REFERENCE_RADIUS_M,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_SPHEROID,
} from "./reference/cases";
import {
  PROVISIONAL_THICK_OPTICAL_DEPTH,
  provisionalThick,
  verticalOpticalDepth,
} from "./thick/regime";

/**
 * An Earth-class column of dry air at 288.15 K and `surfacePa`, under its molecular term alone:
 * the U.S. Standard Atmosphere 1976's M₀, 28.9644 g mol⁻¹, and g₀, 9.80665 m s⁻².
 */
function earthClassAir(surfacePa: number): AtmosphereMedium {
  const column = hydrostaticColumn({
    surfacePa,
    temperature: { kind: "isothermal", temperatureK: 288.15 },
    meanMolarMassGPerMol: 28.9644,
    referenceGravityMS2: 9.806_65,
    referenceRadiusM: 6_371_000,
  });
  return {
    name: "Earth-class air",
    topHeightM: column.topHeightM,
    groundAlbedo: [0.1, 0.1, 0.1],
    terms: [molecularTerm(column, EARTH_DRY_AIR).term],
  };
}

describe("the thick line's vertical optical depth", () => {
  const cases: ReadonlyArray<readonly [string, AtmosphereMedium, number]> = [
    ["the measured Earth", EARTH_REFERENCE, EARTH_REFERENCE_RADIUS_M],
    ["the Mars fixture", MARS_REFERENCE, MARS_REFERENCE_RADIUS_M],
    ["the Saturn-class fixture", SATURN_OBLATE_REFERENCE, tableRadiusM(SATURN_OBLATE_SPHEROID)],
    ["the Venus-class medium", venusClassMedium().medium, venusClassMedium().radiusM],
  ];

  it.each(cases)(
    "is opticalDepthToTop's from the ground at μ = 1 for %s, to 10⁻⁶",
    (_, medium, radiusM) => {
      const closed = verticalOpticalDepth(medium);
      const simpson = opticalDepthToTop(medium, radiusM, radiusM, 1);
      for (const [c, depth] of closed.entries()) {
        expect(Math.abs(depth / (simpson[c] ?? Number.NaN) - 1)).toBeLessThan(1e-6);
      }
    },
  );
});

describe("the provisional thick line", () => {
  it("is 1 on the worst channel", () => {
    expect(PROVISIONAL_THICK_OPTICAL_DEPTH).toBe(1);
  });

  it("gives the measured Earth, with and without its ozone slab, no label", () => {
    expect([provisionalThick(EARTH_REFERENCE), provisionalThick(EARTH_SLAB_REFERENCE)]).toEqual([
      false,
      false,
    ]);
  });

  it("gives the Mars fixture, its dust included, no label", () => {
    expect(provisionalThick(MARS_REFERENCE)).toBe(false);
  });

  it("gives the Saturn-class fixture's gas no label", () => {
    expect(provisionalThick(SATURN_OBLATE_REFERENCE)).toBe(false);
  });

  it("labels a Titan-class column and the Venus-class medium", () => {
    expect([
      provisionalThick(titanClassMethaneMedium().medium),
      provisionalThick(venusClassMedium().medium),
    ]).toEqual([true, true]);
  });

  it("labels an Earth-class column raised to τ 1 on its worst channel, and not one just under", () => {
    const worst = Math.max(...verticalOpticalDepth(earthClassAir(101_325)));
    // The air's τ is proportional to its surface pressure on an isothermal column.
    const atOnePa = 101_325 / worst;
    expect([
      provisionalThick(earthClassAir(atOnePa * 1.001)),
      provisionalThick(earthClassAir(atOnePa * 0.999)),
    ]).toEqual([true, false]);
  });
});

describe("Venus-class records, assembled", () => {
  it.each([
    ["92", venus92Detail],
    ["58", venus58Detail],
  ] as const)("carry APPROXIMATE for their thickness at %s bar", (_, detail) => {
    const result = assembleMedium(recordInput(detail()));
    expect([
      result.labels.includes("atmosphereApproximate"),
      result.reasons.includes(THICK_REASON),
    ]).toEqual([true, true]);
  });

  it("leave the generated Earth's thinner air without the thick reason", () => {
    expect(assembleMedium(recordInput(detailOf(earthDetail()))).reasons).not.toContain(
      THICK_REASON,
    );
  });
});

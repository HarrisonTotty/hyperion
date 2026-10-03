import { describe, expect, it } from "vitest";

import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { levelBoundM, planetGeometry } from "../terrain/planet";
import {
  boundedPlanet,
  capAltitudeM,
  closedFormDemandPerS,
  horizontalConstant,
  perLevelDemand,
  verticalConstant,
} from "./demand";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const HIGH = { fovXRad: Math.PI / 3, widthPx: 1920, tauPx: 1 };
/** The brainstorm's cap, 89 m at level 19 (Design note 19), for the closed form's floor. */
const CAP_M = 89;

describe("the closed form at k = 5", () => {
  it("has the brainstorm's constants, the vertical one re-derived as about 340", () => {
    expect(horizontalConstant(5)).toBe(200);
    expect(verticalConstant(5)).toBeCloseTo(340, -1);
  });

  it("reproduces the brainstorm's worked figures to 20%", () => {
    const cases = [
      // Low orbit: about 4 a second.
      { altitudeM: 400_000, horizontalSpeedMps: 7670, verticalSpeedMps: 0, expected: 4 },
      // 100 m/s at 1.5 km: 13.
      { altitudeM: 1500, horizontalSpeedMps: 100, verticalSpeedMps: 0, expected: 13 },
      // 300 m/s at 300 m: 200.
      { altitudeM: 300, horizontalSpeedMps: 300, verticalSpeedMps: 0, expected: 200 },
      // Descending at 20 m/s through 200 m: 29 with 290, 34 with the re-derived 340.
      { altitudeM: 200, horizontalSpeedMps: 0, verticalSpeedMps: -20, expected: 34 },
    ];
    for (const { expected, ...state } of cases) {
      const demand = closedFormDemandPerS(state, CAP_M);
      expect(Math.abs(demand / expected - 1)).toBeLessThan(0.2);
    }
  });
});

describe("the per-level prediction", () => {
  it("floors the altitude at the cap below which the finest level is drawn under the camera", () => {
    const cap = capAltitudeM(PLANET, HIGH);
    const low = perLevelDemand(
      PLANET,
      { altitudeM: 1, horizontalSpeedMps: 0, verticalSpeedMps: -1 },
      HIGH,
    );
    expect(low.floorAltitudeM).toBe(cap);
  });

  it("is zero at rest and grows with speed", () => {
    const at = (v: number): number =>
      perLevelDemand(PLANET, { altitudeM: 300, horizontalSpeedMps: v, verticalSpeedMps: 0 }, HIGH)
        .perS;
    expect(at(0)).toBe(0);
    expect(at(300)).toBeCloseTo(2 * at(150), 9);
  });
});

describe("the bound rules", () => {
  const sigma = [645, 455, 320, 225, 156, 106, 73.3, 49.4, 31.3, 15.6, 7.82, 3.91, 1.96, 0.977];

  it("leaves the hard rule's planet as it is", () => {
    expect(boundedPlanet(PLANET, "hard", sigma)).toBe(PLANET);
  });

  it("takes min(ε_n, 4σ_n), keeping the hard bound where σ_n is zero or missing", () => {
    const calibrated = boundedPlanet(PLANET, "calibrated", sigma);
    expect(levelBoundM(calibrated, 4)).toBe(Math.min(levelBoundM(PLANET, 4), 4 * 156));
    expect(levelBoundM(calibrated, 4)).toBeLessThan(levelBoundM(PLANET, 4));
    expect(levelBoundM(calibrated, 18)).toBe(levelBoundM(PLANET, 18));
    expect(calibrated.levels?.[4 * 4 + 1]).toBe(PLANET.levels?.[4 * 4 + 1]);
  });
});

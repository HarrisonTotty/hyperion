import { describe, expect, it } from "vitest";

import { EARTH_REFERENCE } from "./earth";
import {
  columnLengthM,
  type DensityProfile,
  densityAt,
  extinction,
  type MediumTerm,
  termNamed,
} from "./medium";

const TENT: DensityProfile = { kind: "tent", bottomM: 10_000, peakM: 25_000, topM: 40_000 };
const EXP: DensityProfile = { kind: "exponential", scaleHeightM: 8_000 };

/** The midpoint rule over [0, top] in 1 m steps. */
function numericColumnM(profile: DensityProfile, topM: number): number {
  let sum = 0;
  for (let h = 0.5; h < topM; h += 1) {
    sum += densityAt(profile, h);
  }
  return sum;
}

describe("densityAt", () => {
  it("is 1 at the ground and 1/e at one scale height for an exponential", () => {
    expect(densityAt(EXP, 0)).toBe(1);
    expect(densityAt(EXP, 8_000)).toBeCloseTo(Math.exp(-1), 12);
  });

  it("is zero outside a tent, 1 at its peak and half way up each side", () => {
    expect(densityAt(TENT, 5_000)).toBe(0);
    expect(densityAt(TENT, 45_000)).toBe(0);
    expect(densityAt(TENT, 25_000)).toBe(1);
    expect(densityAt(TENT, 17_500)).toBeCloseTo(0.5, 12);
    expect(densityAt(TENT, 32_500)).toBeCloseTo(0.5, 12);
  });
});

describe("columnLengthM", () => {
  it("gives 15 km for the full tent, as Bruneton's ozone layer integrates", () => {
    expect(columnLengthM(TENT, 100_000)).toBeCloseTo(15_000, 9);
  });

  it("agrees with a numerical integral of the profile at partial heights", () => {
    for (const top of [12_000, 25_000, 31_000, 60_000]) {
      expect(columnLengthM(TENT, top)).toBeCloseTo(numericColumnM(TENT, top), 2);
      expect(columnLengthM(EXP, top)).toBeCloseTo(numericColumnM(EXP, top), 2);
    }
  });
});

describe("terms", () => {
  it("sums scattering and absorption into extinction", () => {
    const term: MediumTerm = {
      name: "t",
      density: EXP,
      scattering: [1, 2, 3],
      absorption: [0.5, 0.25, 0],
      phase: { kind: "none" },
    };
    expect(extinction(term)).toEqual([1.5, 2.25, 3]);
  });

  it("finds a term by name and refuses a missing one", () => {
    expect(termNamed(EARTH_REFERENCE, "ozone").name).toBe("ozone");
    expect(() => termNamed(EARTH_REFERENCE, "dust")).toThrow(/no term dust/);
  });
});

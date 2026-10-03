import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { rotateToBody } from "../coords/rotation";
import {
  TEST_PLANET_PERIOD_S,
  TEST_PLANET_RATE_RAD_PER_S,
  testPlanetAngleRad,
  testPlanetRotationAt,
} from "./rotation";

describe("the test planet's rotation", () => {
  it("turns once a stellar day, 86,164.0989 s (IERS 2010's ω)", () => {
    expect(TEST_PLANET_PERIOD_S).toBeCloseTo(86_164.0989, 3);
  });

  it("spins eastward about the pole, z fixed", () => {
    const x = rotateToBody(testPlanetRotationAt(TEST_PLANET_PERIOD_S / 4), vec3(1, 0, 0));
    expect(x.x).toBeCloseTo(0, 9);
    expect(x.y).toBeCloseTo(1, 9);
    const z = rotateToBody(testPlanetRotationAt(1234.5), vec3(0, 0, 1));
    expect(z).toEqual(vec3(0, 0, 1));
  });

  it("reduces the angle into [0, 2π), losing no precision far from the epoch", () => {
    const far = 1e9 + 0.25;
    const angle = testPlanetAngleRad(far);
    expect(angle).toBeGreaterThanOrEqual(0);
    expect(angle).toBeLessThan(2 * Math.PI);
    const step = testPlanetAngleRad(far + 1) - angle;
    expect(step).toBeCloseTo(TEST_PLANET_RATE_RAD_PER_S, 12);
  });
});

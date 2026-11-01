import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { scaledAmount } from "../atmosphere/absorbers";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES } from "../atmosphere/earth";
import {
  absorberAmountsToTop,
  absorberPressuresPa,
  curvesOpticalDepth,
  opticalDepthToTop,
} from "../atmosphere/opticalDepth";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { type GlareAir, glareAirTransmittance } from "./glareAir";

/** A spherical Earth for the rebuilt reference atmosphere, R05's mean radius. */
const EARTH_RADIUS_M = 6_371_008.8;
const CURVES = EARTH_REFERENCE_CURVES[0] ?? [];
const SUN_RADIUS_RAD = 0.00465;

/** The air of the reference Earth seen from `heightM` above its ground, on the body-fixed +x axis. */
function earthAir(heightM: number): GlareAir {
  return {
    medium: EARTH_REFERENCE,
    equatorialRadiusM: EARTH_RADIUS_M,
    polarRadiusM: EARTH_RADIUS_M,
    cameraM: vec3(EARTH_RADIUS_M + heightM, 0, 0),
    toBodyFixed: IDENTITY_QUATERNION,
  };
}

/** The unit direction `elevationDeg` above the horizon of a camera on +x, towards +y. */
function elevated(elevationDeg: number): ReturnType<typeof vec3> {
  const e = (elevationDeg * Math.PI) / 180;
  return vec3(Math.sin(e), Math.cos(e), 0);
}

/** The oracle the tables hold, e^−τ from radius `rM` at zenith cosine `mu` to the top. */
function oracle(rM: number, mu: number): readonly number[] {
  const depth = opticalDepthToTop(EARTH_REFERENCE, EARTH_RADIUS_M, rM, mu, 4000);
  const absorbed = curvesOpticalDepth(
    CURVES,
    absorberAmountsToTop(EARTH_REFERENCE, EARTH_RADIUS_M, rM, mu, 4000),
    absorberPressuresPa(EARTH_REFERENCE, EARTH_RADIUS_M, rM, mu, true, 4000),
  );
  return depth.map((tau, c) => Math.exp(-(tau + (absorbed[c] ?? 0))));
}

describe("glareAirTransmittance (fix-view-light)", () => {
  it.each([90, 30, 10, 5, 2])(
    "is the tables' oracle to 0.5% from the ground at %d° above the horizon",
    (elevationDeg) => {
      const air = earthAir(2);
      const mu = Math.sin((elevationDeg * Math.PI) / 180);
      const expected = oracle(EARTH_RADIUS_M + 2, mu);
      const got = glareAirTransmittance(air, CURVES, elevated(elevationDeg), SUN_RADIUS_RAD);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(got[c] / (expected[c] ?? 1) - 1)).toBeLessThan(0.005);
      }
    },
  );

  it("dims and reddens a setting Sun: at 5°, 1° and 0° each channel falls, blue fastest", () => {
    const air = earthAir(2);
    const [five, one, zero] = [5, 1, 0].map((deg) =>
      glareAirTransmittance(air, CURVES, elevated(deg), SUN_RADIUS_RAD),
    );
    for (const t of [five, one, zero]) {
      expect((t?.[0] ?? 0) > (t?.[1] ?? 0) && (t?.[1] ?? 0) > (t?.[2] ?? 0)).toBe(true);
    }
    for (const c of [0, 1, 2] as const) {
      expect((five?.[c] ?? 0) > (one?.[c] ?? 0) && (one?.[c] ?? 0) > (zero?.[c] ?? 0)).toBe(true);
    }
  });

  it("sees a Sun on the horizon along the line raised by its radius, not through the ground", () => {
    const t = glareAirTransmittance(earthAir(2), CURVES, elevated(-0.1), SUN_RADIUS_RAD);
    expect(t[0]).toBeGreaterThan(0);
  });

  it("gives 0 for a Sun wholly below the horizon", () => {
    expect(glareAirTransmittance(earthAir(2), CURVES, elevated(-2), SUN_RADIUS_RAD)).toEqual([
      0, 0, 0,
    ]);
  });

  /** The direction from a camera 1,000 km up whose line grazes the air `tangentM` above the ground. */
  function pastTheLimb(tangentM: number): ReturnType<typeof vec3> {
    const down = Math.acos((EARTH_RADIUS_M + tangentM) / (EARTH_RADIUS_M + 1e6));
    return vec3(-Math.sin(down), Math.cos(down), 0);
  }

  it("from orbit, a Sun through the ozone layer at the limb, 20 km up, is dimmed and bluer", () => {
    const [r, , b] = glareAirTransmittance(
      earthAir(1e6),
      CURVES,
      pastTheLimb(20_000),
      SUN_RADIUS_RAD,
    );
    // Ozone's Chappuis band takes red and green: the band above an orbital sunset is blue.
    expect(b > r && b < 0.5).toBe(true);
  });

  it("from orbit, a Sun through the lower air at the limb, 5 km up, is dimmed and redder", () => {
    const [r, g, b] = glareAirTransmittance(
      earthAir(1e6),
      CURVES,
      pastTheLimb(5_000),
      SUN_RADIUS_RAD,
    );
    expect(r > g && g > b && r < 0.1).toBe(true);
  });

  it("from orbit, leaves a Sun whole whose line passes above the air", () => {
    expect(
      glareAirTransmittance(earthAir(1e6), CURVES, pastTheLimb(200_000), SUN_RADIUS_RAD),
    ).toEqual([1, 1, 1]);
  });

  it("from orbit, is the oracle's along the chord: twice the half to the tangent, its absorbers' amounts doubled", () => {
    const tangent = EARTH_RADIUS_M + 20_000;
    const depth = opticalDepthToTop(EARTH_REFERENCE, EARTH_RADIUS_M, tangent, 0, 4000);
    const absorbed = curvesOpticalDepth(
      CURVES,
      absorberAmountsToTop(EARTH_REFERENCE, EARTH_RADIUS_M, tangent, 0, 4000).map((amount) =>
        scaledAmount(amount, 2),
      ),
      absorberPressuresPa(EARTH_REFERENCE, EARTH_RADIUS_M, tangent, 0, true, 4000),
    );
    const got = glareAirTransmittance(earthAir(1e6), CURVES, pastTheLimb(20_000), SUN_RADIUS_RAD);
    for (const c of [0, 1, 2] as const) {
      const expected = Math.exp(-(2 * depth[c] + absorbed[c]));
      expect(Math.abs(got[c] / expected - 1)).toBeLessThan(0.01);
    }
  });
});

import { describe, expect, it } from "vitest";

import { add, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { scaledAmount } from "../atmosphere/absorbers";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES } from "../atmosphere/earth";
import {
  absorberAmountsToTop,
  absorberPressuresPa,
  curvesOpticalDepth,
  opticalDepthToTop,
} from "../atmosphere/opticalDepth";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";
import { discLightThroughAir, discLuminanceRgb } from "../sky/discFlux";
import { type GlareAir, glareDiscAir, glareLineAir, type GroundBy } from "./glareAir";

/** A spherical Earth for the rebuilt reference atmosphere, R05's mean radius. */
const EARTH_RADIUS_M = 6_371_008.8;
const CURVES = EARTH_REFERENCE_CURVES[0] ?? [];
const SUN_RADIUS_RAD = 0.00465;
const AU_M = 149_597_870_700;

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
function elevated(elevationDeg: number): Vec3 {
  const e = (elevationDeg * Math.PI) / 180;
  return vec3(Math.sin(e), Math.cos(e), 0);
}

/** The direction from a camera 1,000 km up whose line grazes the air `tangentM` above the ground. */
function pastTheLimb(tangentM: number): Vec3 {
  const down = Math.acos((EARTH_RADIUS_M + tangentM) / (EARTH_RADIUS_M + 1e6));
  return vec3(-Math.sin(down), Math.cos(down), 0);
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

/** The line's transmittance alone. */
function along(air: GlareAir, direction: Vec3, distanceM = AU_M): Rgb {
  return glareLineAir(air, CURVES, direction, distanceM).transmittance;
}

/**
 * The Sun's dimmed light by brute force, a 48 × 48 grid over the disc, every point's own line, the
 * camera on +x looking towards +y at `elevationDeg`: over its light above the ground, the frame's
 * occluders counting the ground, or over its whole light, its lines counting it.
 */
function bruteForceLight(air: GlareAir, elevationDeg: number, groundBy: GroundBy): Rgb {
  const host = aHostDisc();
  const e = (elevationDeg * Math.PI) / 180;
  const axis = elevated(elevationDeg);
  const vertical = vec3(Math.cos(e), -Math.sin(e), 0);
  const horizontal = vec3(0, 0, 1);
  const clear: [number, number, number] = [0, 0, 0];
  const whole: [number, number, number] = [0, 0, 0];
  const dimmed: [number, number, number] = [0, 0, 0];
  const n = 48;
  for (let j = 0; j < n; j += 1) {
    for (let i = 0; i < n; i += 1) {
      const x = ((i + 0.5) / n) * 2 - 1;
      const y = ((j + 0.5) / n) * 2 - 1;
      if (x * x + y * y >= 1) {
        continue;
      }
      const luminance = discLuminanceRgb(host, Math.sqrt(1 - x * x - y * y));
      const d = normalise(
        add(
          axis,
          add(
            scale(vertical, Math.tan(SUN_RADIUS_RAD * y)),
            scale(horizontal, Math.tan(SUN_RADIUS_RAD * x)),
          ),
        ),
      );
      const line = glareLineAir(air, CURVES, d, AU_M);
      for (const c of [0, 1, 2] as const) {
        whole[c] += luminance[c];
      }
      if (line.clear) {
        for (const c of [0, 1, 2] as const) {
          clear[c] += luminance[c];
          dimmed[c] += luminance[c] * line.transmittance[c];
        }
      }
    }
  }
  const measure = groundBy === "lines" ? whole : clear;
  return [dimmed[0] / measure[0], dimmed[1] / measure[1], dimmed[2] / measure[2]];
}

describe("glareLineAir (fix-view-light)", () => {
  it.each([90, 30, 10, 5, 2])(
    "is the tables' oracle to 0.5% from the ground at %d° above the horizon",
    (elevationDeg) => {
      const mu = Math.sin((elevationDeg * Math.PI) / 180);
      const expected = oracle(EARTH_RADIUS_M + 2, mu);
      const got = along(earthAir(2), elevated(elevationDeg));
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(got[c] / (expected[c] ?? 1) - 1)).toBeLessThan(0.005);
      }
    },
  );

  it("dims and reddens a setting Sun: at 5°, 1° and 0° each channel falls, blue fastest", () => {
    const [five, one, zero] = [5, 1, 0].map((deg) => along(earthAir(2), elevated(deg)));
    for (const t of [five, one, zero]) {
      expect((t?.[0] ?? 0) > (t?.[1] ?? 0) && (t?.[1] ?? 0) > (t?.[2] ?? 0)).toBe(true);
    }
    for (const c of [0, 1, 2] as const) {
      expect((five?.[c] ?? 0) > (one?.[c] ?? 0) && (one?.[c] ?? 0) > (zero?.[c] ?? 0)).toBe(true);
    }
  });

  it("gives 0 along a line that meets the ground before the star", () => {
    expect(glareLineAir(earthAir(2), CURVES, elevated(-2), AU_M)).toEqual({
      transmittance: [0, 0, 0],
      clear: false,
    });
  });

  it("takes a camera below the datum, in a valley, as on it: the sky above clear, the ground below", () => {
    const valley = earthAir(-300);
    const up = glareLineAir(valley, CURVES, elevated(30), AU_M);
    expect(up.clear && up.transmittance[0] < 1).toBe(true);
    expect(glareLineAir(valley, CURVES, elevated(-1), AU_M).clear).toBe(false);
  });

  it("stops at the star: air and ground beyond it do not dim it", () => {
    // The star 1,000 km off, between the camera far out on +x and the body.
    const far = earthAir(1e8);
    const towards = vec3(-1, 0, 0);
    expect(glareLineAir(far, CURVES, towards, 1e6)).toEqual({
      transmittance: [1, 1, 1],
      clear: true,
    });
    expect(glareLineAir(far, CURVES, towards, AU_M).clear).toBe(false);
  });

  it("from orbit, a Sun through the ozone layer at the limb, 20 km up, is dimmed and bluer", () => {
    const [r, , b] = along(earthAir(1e6), pastTheLimb(20_000));
    // Ozone's Chappuis band takes red and green.
    expect(b > r && b < 0.5).toBe(true);
  });

  it("from orbit, a Sun through the lower air at the limb, 5 km up, is dimmed and redder", () => {
    const [r, g, b] = along(earthAir(1e6), pastTheLimb(5_000));
    expect(r > g && g > b && r < 0.1).toBe(true);
  });

  it("from orbit, leaves a Sun whole whose line passes above the air", () => {
    expect(along(earthAir(1e6), pastTheLimb(200_000))).toEqual([1, 1, 1]);
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
    const got = along(earthAir(1e6), pastTheLimb(20_000));
    for (const c of [0, 1, 2] as const) {
      const expected = Math.exp(-(2 * depth[c] + absorbed[c]));
      expect(Math.abs(got[c] / expected - 1)).toBeLessThan(0.01);
    }
  });
});

describe("glareDiscAir (fix-view-light)", () => {
  it("takes a high Sun's air along its centre's line", () => {
    expect(glareDiscAir(earthAir(2), CURVES, elevated(30), SUN_RADIUS_RAD, AU_M, "lines")).toEqual({
      kind: "uniform",
      transmittance: along(earthAir(2), elevated(30)),
    });
  });

  it.each(
    [5, 1, 0, -0.1].flatMap((elevationDeg) =>
      (["lines", "occluders"] as const).map((groundBy) => ({ elevationDeg, groundBy })),
    ),
  )(
    "holds the light of a Sun $elevationDeg° above the horizon, its ground by $groundBy, to a brute-force integral over its disc within 1%",
    ({ elevationDeg, groundBy }) => {
      const air = earthAir(2);
      const disc = glareDiscAir(
        air,
        CURVES,
        elevated(elevationDeg),
        SUN_RADIUS_RAD,
        AU_M,
        groundBy,
      );
      const { light } = discLightThroughAir(aHostDisc(), [disc], elevated(elevationDeg), 1e30);
      const expected = bruteForceLight(air, elevationDeg, groundBy);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(light[c] / expected[c] - 1)).toBeLessThan(0.01);
      }
    },
  );

  it("takes a Sun on the horizon across its disc, its light above its centre's line's", () => {
    const air = earthAir(2);
    const disc = glareDiscAir(air, CURVES, elevated(0), SUN_RADIUS_RAD, AU_M, "occluders");
    expect(disc.kind).toBe("across");
    const { light } = discLightThroughAir(aHostDisc(), [disc], elevated(0), 1e30);
    const centre = along(air, elevated(0));
    expect([0, 1, 2].every((c) => (light[c] ?? 0) > (centre[c] ?? 0))).toBe(true);
  });

  it("leaves no light once the whole disc is below the horizon", () => {
    const disc = glareDiscAir(earthAir(2), CURVES, elevated(-0.4), SUN_RADIUS_RAD, AU_M, "lines");
    expect(discLightThroughAir(aHostDisc(), [disc], elevated(-0.4), 0)).toEqual({
      light: [0, 0, 0],
      excess: [0, 0, 0],
    });
  });
});

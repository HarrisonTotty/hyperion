/**
 * The test planet's rotation (plan R05, T13.a, Design note 14): a fixed pole along the body frame's
 * z axis and the Earth's rate, in the shape R02's rotation interface takes (a `Rotation3` from the
 * body-fixed axes to the body frame, evaluated at each frame's time), until galaxy plan 14's
 * `body_fixed_at` exists.
 *
 * @remarks
 * The rate is the Earth Rotation Angle's, 1.00273781191135448 turns a UT1 day (IERS Conventions
 * 2010, eq. 5.15): ω = 2π × 1.00273781191135448 ÷ 86,400 s = 7.292115146706979 × 10⁻⁵ rad/s, which
 * IERS Table 1.1 and WGS 84 (NIMA TR8350.2 §3.2.4) round to 7.292115 × 10⁻⁵; a period of
 * 86,164.0989 s, the stellar day. Re-checked in T13.a: the plan's 86,164.0905 s is the sidereal day,
 * measured against the precessing equinox rather than inertial space, 8.4 ms shorter; a body frame
 * that does not rotate turns with the stellar day. The angle is reduced from the whole seconds since
 * the epoch, as `body_fixed_at` will, so that a long script loses no precision.
 */

import { vec3 } from "../../geometry/vec3";
import { type Rotation3, rotation3FromRows } from "../coords/rotation";

/** The test planet's rotation rate, rad/s: the Earth Rotation Angle's (IERS Conventions 2010, eq. 5.15). */
export const TEST_PLANET_RATE_RAD_PER_S = (2 * Math.PI * 1.002_737_811_911_354_6) / 86_400;

/** The test planet's rotation period, s: 2π ÷ ω, the stellar day. */
export const TEST_PLANET_PERIOD_S = (2 * Math.PI) / TEST_PLANET_RATE_RAD_PER_S;

/**
 * The rotation angle at `tS` seconds after the epoch, rad, in [0, 2π): the whole seconds reduced
 * modulo the period first, then the fraction added.
 */
export function testPlanetAngleRad(tS: number): number {
  const whole = Math.floor(tS);
  const reduced = (whole % TEST_PLANET_PERIOD_S) + (tS - whole);
  const angle = TEST_PLANET_RATE_RAD_PER_S * reduced;
  const turn = 2 * Math.PI;
  return ((angle % turn) + turn) % turn;
}

/** The test planet's rotation at `tS`: a spin about z, from its body-fixed axes to its body frame. */
export function testPlanetRotationAt(tS: number): Rotation3 {
  const theta = testPlanetAngleRad(tS);
  const c = Math.cos(theta);
  const s = Math.sin(theta);
  return rotation3FromRows([vec3(c, -s, 0), vec3(s, c, 0), vec3(0, 0, 1)]);
}

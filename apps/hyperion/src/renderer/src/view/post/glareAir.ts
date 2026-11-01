/**
 * The air between the camera and a glare source (fix-view-light, the setting Sun's halo): the
 * transmittance of a body's medium along the line of sight to a star, per channel, by which the
 * star's glare is dimmed and reddened as R08's composite dims its disc.
 *
 * @remarks
 * The composite multiplies the disc's pixels by the tables' transmittance along each pixel's ray,
 * on the GPU (`composite.wgsl`'s `transmittanceToSpace` and `transmittanceFromAbove`); the tables
 * hold the optical depth of `opticalDepth.ts`'s oracle on a grid, with the absorbers' columns read
 * through the light's curves of growth. This evaluates that oracle along the ray itself: every
 * term's extinction times its density integrated by Simpson's rule, and each absorber's column
 * and absorber-weighted pressure through the star's light slot's curves (`curvesOpticalDepth`),
 * each sample at its height above the body's datum, a spheroid, so that a camera on an oblate
 * ground stands at its own height. The ray runs from the camera, or from its entry into the top's
 * spheroid where the camera is above it, to its exit; 1 where it misses the air. Where the line to
 * the star's centre meets the ground, the star's visible part, which the occlusion share counts,
 * is seen along rays just clear of it, so the line is raised by the disc's angular radius towards
 * the camera's zenith; the whole disc behind the ground gives 0. A cloud deck's table and the
 * gravity bands' scale heights are not taken.
 */

import { dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { AbsorberCurve } from "../atmosphere/absorbers";
import {
  ABSORBER_PRESSURE_REFERENCE_PA,
  type AtmosphereMedium,
  densityAt,
  extinction,
} from "../atmosphere/medium";
import { amountAlong, curvesOpticalDepth } from "../atmosphere/opticalDepth";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import type { Rgb } from "../photometry/toneCurve";

/** Simpson intervals along a line of sight: 256, a few kilometres a step through the grazing air. */
export const GLARE_AIR_STEPS = 256;

/** One body's air as a glare source's line of sight crosses it. */
export interface GlareAir {
  readonly medium: AtmosphereMedium;
  /** The datum the medium is layered on, m: its equatorial radius, its pole along body-fixed +z. */
  readonly equatorialRadiusM: number;
  /** The datum's polar radius, m. */
  readonly polarRadiusM: number;
  /** The camera from the body's centre in its body-fixed axes, m. */
  readonly cameraM: Vec3;
  /** Rotates the scene's camera-relative axes, a glare source's direction's, into the body-fixed. */
  readonly toBodyFixed: Quaternion;
}

/**
 * The distances from `origin` along the unit `d` at which the ray meets the spheroid of semi-axes
 * (r, r, rz) about the centre, nearer first, or `null` where it misses.
 */
function spheroidHits(
  origin: Vec3,
  d: Vec3,
  r: number,
  rz: number,
): readonly [number, number] | null {
  const k = r / rz;
  const o = vec3(origin.x, origin.y, origin.z * k);
  const v = vec3(d.x, d.y, d.z * k);
  const a = dot(v, v);
  const b = dot(o, v);
  const c = dot(o, o) - r * r;
  const disc = b * b - a * c;
  if (!(disc >= 0) || !(a > 0)) {
    return null;
  }
  // The roots in the forms that do not cancel.
  const q = -(b + (b >= 0 ? Math.sqrt(disc) : -Math.sqrt(disc)));
  if (q === 0) {
    return [0, 0];
  }
  const first = q / a;
  const second = c / q;
  return first <= second ? [first, second] : [second, first];
}

/** A point's height above the datum, m: its distance less the datum's radius along its direction. */
function heightAboveM(p: Vec3, a: number, c: number): number {
  const r = norm(p);
  if (!(r > 0)) {
    return 0;
  }
  const radial = (p.x * p.x + p.y * p.y) / (r * r * a * a) + (p.z * p.z) / (r * r * c * c);
  return Math.max(0, r - 1 / Math.sqrt(radial));
}

/** Whether the ray from `origin` along `d` meets the datum ahead. */
function meetsGround(origin: Vec3, d: Vec3, a: number, c: number): boolean {
  const hits = spheroidHits(origin, d, a, c);
  return hits !== null && hits[1] > 0;
}

/**
 * The transmittance per channel of `air` along the line of sight to a star in `direction`, the
 * scene's camera-relative axes (the module's remarks).
 *
 * @param curves - The star's light slot's curves of growth, one an absorber term in the medium's
 *   order of them (the tables' `curves[slot]`); the absorbers are left out where they do not fit.
 * @param angularRadiusRad - The star's angular radius, by which a line meeting the ground is raised.
 */
export function glareAirTransmittance(
  air: GlareAir,
  curves: ReadonlyArray<AbsorberCurve>,
  direction: Vec3,
  angularRadiusRad: number,
  steps = GLARE_AIR_STEPS,
): Rgb {
  const a = air.equatorialRadiusM;
  const c = air.polarRadiusM;
  const top = air.medium.topHeightM;
  const origin = air.cameraM;
  let d = normalise(rotate(air.toBodyFixed, direction));
  if (meetsGround(origin, d, a, c)) {
    const up = normalise(origin);
    const across = sub(up, scale(d, dot(up, d)));
    const length = norm(across);
    if (!(length > 0)) {
      return [0, 0, 0];
    }
    d = normalise(
      vec3(
        d.x + (Math.tan(angularRadiusRad) * across.x) / length,
        d.y + (Math.tan(angularRadiusRad) * across.y) / length,
        d.z + (Math.tan(angularRadiusRad) * across.z) / length,
      ),
    );
    if (meetsGround(origin, d, a, c)) {
      return [0, 0, 0];
    }
  }
  const shell = spheroidHits(origin, d, a + top, c + top);
  if (shell === null || shell[1] <= 0) {
    return [1, 1, 1];
  }
  const t0 = Math.max(shell[0], 0);
  const n = steps + (steps % 2);
  const dt = (shell[1] - t0) / n;
  const terms = air.medium.terms.map((term) => ({ term, sigma: extinction(term) }));
  const absorbers = air.medium.terms.flatMap((term) =>
    term.absorber === undefined ? [] : [{ term, absorber: term.absorber }],
  );
  const pressure = air.medium.pressureBar;
  const sum: [number, number, number] = [0, 0, 0];
  const paths = new Float64Array(absorbers.length);
  const weighted = new Float64Array(absorbers.length);
  for (let i = 0; i <= n; i += 1) {
    const weight = i === 0 || i === n ? 1 : i % 2 === 1 ? 4 : 2;
    const t = t0 + i * dt;
    const h = heightAboveM(vec3(origin.x + t * d.x, origin.y + t * d.y, origin.z + t * d.z), a, c);
    for (const { term, sigma } of terms) {
      const density = densityAt(term.density, h) * weight;
      sum[0] += sigma[0] * density;
      sum[1] += sigma[1] * density;
      sum[2] += sigma[2] * density;
    }
    const ratio = pressure === undefined ? 0 : densityAt(pressure, h);
    absorbers.forEach(({ term }, k) => {
      const density = densityAt(term.density, h) * weight;
      paths[k] = (paths[k] ?? 0) + density;
      weighted[k] = (weighted[k] ?? 0) + density * ratio;
    });
  }
  const k3 = dt / 3;
  const depth: [number, number, number] = [sum[0] * k3, sum[1] * k3, sum[2] * k3];
  if (absorbers.length > 0 && curves.length === absorbers.length) {
    const absorbed = curvesOpticalDepth(
      curves,
      absorbers.map(({ absorber }, i) => amountAlong(absorber, (paths[i] ?? 0) * k3)),
      absorbers.map((_, i) =>
        pressure === undefined || !((paths[i] ?? 0) > 0)
          ? undefined
          : (ABSORBER_PRESSURE_REFERENCE_PA * (weighted[i] ?? 0)) / (paths[i] ?? Number.NaN),
      ),
    );
    depth[0] += absorbed[0];
    depth[1] += absorbed[1];
    depth[2] += absorbed[2];
  }
  const transmitted = depth.map((tau) => Math.exp(-tau));
  return [
    Number.isFinite(transmitted[0]) ? (transmitted[0] ?? 1) : 1,
    Number.isFinite(transmitted[1]) ? (transmitted[1] ?? 1) : 1,
    Number.isFinite(transmitted[2]) ? (transmitted[2] ?? 1) : 1,
  ];
}

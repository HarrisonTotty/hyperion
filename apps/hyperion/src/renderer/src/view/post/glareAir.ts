/**
 * The air between the camera and a glare source (fix-view-light, the setting Sun's halo): the
 * transmittance of a body's medium along lines of sight across a star's disc, per channel, by which
 * the star's glare is dimmed and reddened as R08's composite dims its disc.
 *
 * @remarks
 * The composite multiplies each of the disc's pixels by the tables' transmittance along its own
 * ray, on the GPU (`composite.wgsl`'s `transmittanceToSpace` and `transmittanceFromAbove`); the
 * tables hold the optical depth of `opticalDepth.ts`'s oracle on a grid, with the absorbers' columns
 * read through the light's curves of growth. This evaluates that oracle along a line itself
 * ({@link glareLineAir}): every term's extinction times its density integrated by Simpson's rule,
 * and each absorber's column and absorber-weighted pressure through the star's light slot's curves
 * (`curvesOpticalDepth`), each sample at its height above the body's datum, a spheroid, so that a
 * camera on an oblate ground stands at its own height. The line runs from the camera, or from its
 * entry into the top's spheroid where the camera is above it, to its exit or the star, whichever
 * is nearer; it is 1 where it misses the air, and 0 where it meets the ground before the star. A
 * camera below the datum, in a valley, is taken on it, as the composite takes its height: the
 * ground lies below its horizon.
 *
 * Near the horizon or a limb the transmittance changes by orders of magnitude across the disc,
 * the upper limb's light the most of what reaches the camera, so a disc's air is taken across it
 * ({@link glareDiscAir}): along lines through {@link DISC_AIR_NODES} Gauss–Legendre nodes of its
 * diameter along the body's vertical, the camera's zenith seen across the line of sight, wherever
 * its two limbs' optical depths differ by more than {@link DISC_AIR_UNIFORM_TAU} in a channel or
 * either limb's line meets the ground; and as its centre's line elsewhere. `discFlux.ts`'s
 * `discLightThroughAir` integrates the disc's light over them (science check, 2026-11-02). A cloud
 * deck's table, the gravity bands' scale heights and refraction are not taken; nor is the
 * composite's own refraction, which it does not draw either.
 */

import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { AbsorberCurve } from "../atmosphere/absorbers";
import {
  ABSORBER_PRESSURE_REFERENCE_PA,
  type AtmosphereMedium,
  densityAt,
  extinction,
} from "../atmosphere/medium";
import { amountAlong, curvesOpticalDepth } from "../atmosphere/opticalDepth";
import type { Quaternion } from "../camera/pose";
import { conjugate, rotate } from "../camera/quaternion";
import { gaussLegendre } from "../lighting/quadrature";
import type { Rgb } from "../photometry/toneCurve";

/** Simpson intervals along a line of sight: 256, a few kilometres a step through the grazing air. */
export const GLARE_AIR_STEPS = 256;

/** The Gauss–Legendre nodes across a disc whose air changes across it: 16. */
export const DISC_AIR_NODES = 16;

/**
 * The difference in optical depth between a disc's two limbs, in any channel, above which its air
 * is taken across it rather than along its centre's line: 0.05, a 5% change in its light.
 */
export const DISC_AIR_UNIFORM_TAU = 0.05;

/** {@link DISC_AIR_NODES}' Gauss–Legendre rule on [−1, 1]. */
export const DISC_AIR_RULE = gaussLegendre(DISC_AIR_NODES);

/** One body's air as a glare source's lines of sight cross it. */
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

/** The air along one line of sight to a star. */
export interface LineAir {
  /** Per channel; 0 where the line meets the ground. */
  readonly transmittance: Rgb;
  /** Whether the line clears the ground before the star. */
  readonly clear: boolean;
}

/** One body's air across a star's disc (the module's remarks). */
export type DiscAir =
  | {
      /** The same across the disc: its centre's line, which clears the ground. */
      readonly kind: "uniform";
      readonly transmittance: Rgb;
    }
  | {
      /** Changing across the disc, along `vertical`. */
      readonly kind: "across";
      /**
       * The unit vector across the line of sight, in the scene's camera-relative axes, along which
       * the lines are offset: the camera's zenith from the body seen across it.
       */
      readonly vertical: Vec3;
      /**
       * Each line's offset from the disc's centre along `vertical`, in units of its angular radius:
       * {@link DISC_AIR_RULE}'s nodes on the span of the disc whose lines clear the ground, from
       * the offset at which they first do to the upper limb, 1.
       */
      readonly offsets: ReadonlyArray<number>;
      /** The rule's weight at each offset, summing to the span's length. */
      readonly weights: ReadonlyArray<number>;
      /** The air along each line, in `offsets`' order. */
      readonly lines: ReadonlyArray<LineAir>;
    };

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

/**
 * Whether the ray from `origin` along `d` meets the datum before `distanceM`: from on or below it, a
 * camera in a valley, whether it runs below the datum's local horizon.
 */
function meetsGround(origin: Vec3, d: Vec3, a: number, c: number, distanceM: number): boolean {
  const inside =
    (origin.x * origin.x + origin.y * origin.y) / (a * a) + (origin.z * origin.z) / (c * c);
  if (inside <= 1) {
    return dot(d, vec3(origin.x / (a * a), origin.y / (a * a), origin.z / (c * c))) < 0;
  }
  const hits = spheroidHits(origin, d, a, c);
  return hits !== null && hits[0] > 0 && hits[0] < distanceM;
}

/** e^−τ, 1 for an optical depth that is not a number. */
function transmitted(tau: number): number {
  const t = Math.exp(-tau);
  return Number.isFinite(t) ? t : 1;
}

/** Whether the line of sight in `direction`, the scene's camera-relative axes, clears the ground. */
function clears(air: GlareAir, direction: Vec3, distanceM: number): boolean {
  const d = normalise(rotate(air.toBodyFixed, direction));
  return !meetsGround(air.cameraM, d, air.equatorialRadiusM, air.polarRadiusM, distanceM);
}

/**
 * The air along the line of sight in `direction`, the scene's camera-relative axes, to a star
 * `distanceM` away (the module's remarks).
 *
 * @param curves - The star's light slot's curves of growth, one an absorber term in the medium's
 *   order of them (the tables' `curves[slot]`); the absorbers are left out where they do not fit.
 */
export function glareLineAir(
  air: GlareAir,
  curves: ReadonlyArray<AbsorberCurve>,
  direction: Vec3,
  distanceM: number,
  steps = GLARE_AIR_STEPS,
): LineAir {
  const a = air.equatorialRadiusM;
  const c = air.polarRadiusM;
  const top = air.medium.topHeightM;
  const origin = air.cameraM;
  const d = normalise(rotate(air.toBodyFixed, direction));
  if (meetsGround(origin, d, a, c, distanceM)) {
    return { transmittance: [0, 0, 0], clear: false };
  }
  const shell = spheroidHits(origin, d, a + top, c + top);
  const t0 = Math.max(shell?.[0] ?? 0, 0);
  const t1 = Math.min(shell?.[1] ?? 0, distanceM);
  if (!(t1 > t0)) {
    return { transmittance: [1, 1, 1], clear: true };
  }
  const n = steps + (steps % 2);
  const dt = (t1 - t0) / n;
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
  return {
    transmittance: [transmitted(depth[0]), transmitted(depth[1]), transmitted(depth[2])],
    clear: true,
  };
}

/** Whether two lines' optical depths differ by more than {@link DISC_AIR_UNIFORM_TAU} in a channel. */
function differs(one: Rgb, other: Rgb): boolean {
  return [0, 1, 2].some((c) => {
    const difference = Math.abs(Math.log(one[c] ?? 0) - Math.log(other[c] ?? 0));
    return !(difference <= DISC_AIR_UNIFORM_TAU);
  });
}

/**
 * The air across a star's disc of angular radius `angularRadiusRad` whose centre lies in `axis`, the
 * scene's camera-relative axes, `distanceM` away (the module's remarks).
 *
 * @param curves - As {@link glareLineAir}'s.
 */
export function glareDiscAir(
  air: GlareAir,
  curves: ReadonlyArray<AbsorberCurve>,
  axis: Vec3,
  angularRadiusRad: number,
  distanceM: number,
): DiscAir {
  const centre = glareLineAir(air, curves, axis, distanceM);
  const d = normalise(rotate(air.toBodyFixed, axis));
  const up = normalise(air.cameraM);
  const across = sub(up, scale(d, dot(up, d)));
  const length = norm(across);
  // Straight up or down the air is the same across the disc.
  if (!(length > 1e-12) || !(angularRadiusRad > 0)) {
    return { kind: "uniform", transmittance: centre.transmittance };
  }
  const vertical = rotate(conjugate(air.toBodyFixed), scale(across, 1 / length));
  const offsetDirection = (offset: number): Vec3 =>
    normalise(add(axis, scale(vertical, Math.tan(angularRadiusRad * offset))));
  const through = (offset: number): LineAir =>
    glareLineAir(air, curves, offsetDirection(offset), distanceM);
  const upper = through(1);
  const lower = through(-1);
  if (!upper.clear) {
    // The whole disc behind the ground.
    return { kind: "uniform", transmittance: [0, 0, 0] };
  }
  if (centre.clear && lower.clear && !differs(upper.transmittance, lower.transmittance)) {
    return { kind: "uniform", transmittance: centre.transmittance };
  }
  // The span whose lines clear the ground, its lower end found by bisection where the lower limb's
  // does not, so that the rule integrates the light that reaches the camera without the edge.
  let from = -1;
  if (!lower.clear) {
    let hidden = -1;
    let seen = 1;
    for (let step = 0; step < 40; step += 1) {
      const middle = (hidden + seen) / 2;
      if (clears(air, offsetDirection(middle), distanceM)) {
        seen = middle;
      } else {
        hidden = middle;
      }
    }
    from = seen;
  }
  const half = (1 - from) / 2;
  const offsets = [...DISC_AIR_RULE.x].map((x) => from + half * (x + 1));
  const weights = [...DISC_AIR_RULE.w].map((w) => half * w);
  return { kind: "across", vertical, offsets, weights, lines: offsets.map(through) };
}

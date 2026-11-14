/**
 * The sun's glint on a sea seen from space (plan R11, R11.T8.b; R07 Design note 12): where it lies,
 * and its light above the display's range handed to R07's tone-mapping pass as a `GlareSource`.
 *
 * @remarks
 * - **The specular point** of a sphere of radius R for a camera at C (from its centre) and a sun
 *   along s is the point whose normal n bisects the directions to the camera and to the sun. It lies
 *   on the great circle through C's and s's directions, between them, where the angle from n to the
 *   camera equals the angle from n to the sun ({@link specularPoint}, by bisection on that arc).
 * - **The glare source.** The far ocean's texels hold the glint capped at the bloom threshold
 *   (`oceanFar.ts`); the rest, above it, is carried in closed form: the glint spot about the
 *   specular point is sampled on the sphere of the sea's equatorial radius (a lean: a spheroid's
 *   spot moves by under its flattening times its size), each sample's solid angle as the camera
 *   sees it, and the source is the region whose glint passes the cap. Its excess luminance times its
 *   solid angle is the energy the cap took from the texels, so the two together are the glint's
 *   whole light ({@link oceanGlintGlare}).
 */
import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../geometry/vec3";
import type { GlareSource } from "../post/glare";
import type { Rgb } from "../photometry/toneCurve";
import { filteredSlopeVariance, oceanFarLight, type OceanFarOptics } from "./oceanFar";
import { oceanTexelAt, type OceanTexels } from "./oceanMap";

/** The bisection's steps: the arc halves 64 times, well past an `f64`'s resolution of it. */
const BISECTION_STEPS = 64;

/** The samples along each axis of the glint spot's grid. */
export const GLINT_GRID = 64;

/** The spot's half-width the grid covers, in units of the largest filtered slope's σ: 4. */
export const GLINT_SIGMAS = 4;

/**
 * The spot's largest half-width, rad of the sphere's surface: 1.2, past 4σ at Cox and Munk's 20 m/s
 * clamp (1.01 rad), so that no sea's spot is cut short.
 */
const GLINT_MAX_EXTENT_RAD = 1.2;

/** The specular point of a sphere, and the cosine of its angle to the camera and to the sun. */
export interface SpecularPoint {
  /** The point, from the sphere's centre, m. */
  readonly pointM: Vec3;
  /** Its outward unit normal. */
  readonly normal: Vec3;
  /** μ_v = μ_s at it. */
  readonly cosine: number;
}

/** The angle between unit vectors `a` and `b`, rad, by atan2 for its accuracy at small angles. */
function angleBetween(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

/**
 * The specular point of a sphere of radius `radiusM` for a camera at `cameraM` from its centre and
 * a sun along the unit `sun` (see the module's remarks), or `null` where none is seen: the camera
 * inside the sphere, the sun opposite the camera, or the point at or past the limb or the
 * terminator.
 */
export function specularPoint(cameraM: Vec3, sun: Vec3, radiusM: number): SpecularPoint | null {
  const distance = norm(cameraM);
  if (!(distance > radiusM)) {
    return null;
  }
  const up = scale(cameraM, 1 / distance);
  const theta = angleBetween(up, sun);
  const toward = sub(sun, scale(up, dot(sun, up)));
  const sideways = norm(toward);
  if (Math.PI - theta < 1e-9) {
    return null;
  }
  if (theta < 1e-12 || sideways === 0) {
    return { pointM: scale(up, radiusM), normal: up, cosine: 1 };
  }
  const across = scale(toward, 1 / sideways);
  const normalAt = (phi: number): Vec3 =>
    add(scale(up, Math.cos(phi)), scale(across, Math.sin(phi)));
  // g(φ) = ∠(n, camera) − ∠(n, sun): negative under the camera, positive under the sun.
  const g = (phi: number): number => {
    const n = normalAt(phi);
    const view = normalise(sub(cameraM, scale(n, radiusM)));
    return angleBetween(n, view) - angleBetween(n, sun);
  };
  let low = 0;
  let high = theta;
  for (let step = 0; step < BISECTION_STEPS; step += 1) {
    const mid = (low + high) / 2;
    if (g(mid) < 0) {
      low = mid;
    } else {
      high = mid;
    }
  }
  const normal = normalAt((low + high) / 2);
  const cosine = dot(normal, sun);
  if (!(cosine > 0)) {
    return null;
  }
  return { pointM: scale(normal, radiusM), normal, cosine };
}

/**
 * The glint's light above the display's range as a glare source (see the module's remarks), or
 * `null` where no glint passes the cap: no specular point seen, no open water there, or a glint
 * spread below the cap.
 *
 * @param optics - The far ocean's, its cap the bloom threshold in cd/m² and its pixel angle the
 *   view's, which the texels' per-pixel filter takes.
 * @param texels - The body's ocean map.
 * @param cameraM - The camera from the body's centre, body-fixed, m.
 * @param toScene - Turns a body-fixed direction into the scene's camera-relative frame, the one
 *   `GlareSource.direction` is in.
 */
export function oceanGlintGlare(
  optics: OceanFarOptics,
  texels: OceanTexels,
  cameraM: Vec3,
  toScene: (direction: Vec3) => Vec3,
): GlareSource | null {
  const radius = optics.equatorialRadiusM;
  const specular = specularPoint(cameraM, optics.sunDirection, radius);
  if (specular === null) {
    return null;
  }
  const centre = specular.normal;
  const texel = oceanTexelAt(texels, [centre.x, centre.y, centre.z]);
  if (texel === null) {
    return null;
  }
  // No source where the glint at the specular point, about its brightest, stays under the cap: a
  // peak a little off it that passes the cap is left to the texels (a lean).
  const atCentre = sub(cameraM, specular.pointM);
  const centreDistance = norm(atCentre);
  const probe = oceanFarLight(
    optics,
    optics,
    scale(atCentre, 1 / centreDistance),
    centre,
    texel,
    centreDistance,
  );
  if (!(Math.max(...probe.glint) > optics.glintCapCdM2)) {
    return null;
  }
  // The spot's half-width: the widest filtered σ at the specular point, 4 times; the required facet
  // tilt grows as the surface angle from the specular point does.
  const footprint = (centreDistance * optics.pixelAngleRad) / Math.sqrt(specular.cosine);
  const variance = filteredSlopeVariance(
    Math.hypot(texel.windEastMps, texel.windNorthMps),
    optics.sea,
    footprint,
    radius,
  );
  // The grid is gnomonic about the specular point: a surface angle θ out is tan θ on it.
  const extent = Math.tan(
    Math.min(
      GLINT_SIGMAS * Math.sqrt(Math.max(variance.upwind, variance.crosswind)),
      GLINT_MAX_EXTENT_RAD,
    ),
  );
  const e1 = normalise(
    cross(centre, Math.abs(centre.z) < 0.9 ? { x: 0, y: 0, z: 1 } : { x: 1, y: 0, z: 0 }),
  );
  const e2 = cross(centre, e1);
  const step = (2 * extent) / GLINT_GRID;
  let hot = 0;
  const excess: [number, number, number] = [0, 0, 0];
  const whole: [number, number, number] = [0, 0, 0];
  for (let i = 0; i < GLINT_GRID; i += 1) {
    const alpha = -extent + (i + 0.5) * step;
    for (let j = 0; j < GLINT_GRID; j += 1) {
      const beta = -extent + (j + 0.5) * step;
      const m = normalise(add(centre, add(scale(e1, alpha), scale(e2, beta))));
      const point = scale(m, radius);
      const toCamera = sub(cameraM, point);
      const distance = norm(toCamera);
      const view = scale(toCamera, 1 / distance);
      const muV = dot(m, view);
      if (!(muV > 0)) {
        continue;
      }
      const there = oceanTexelAt(texels, [m.x, m.y, m.z]);
      if (there === null) {
        continue;
      }
      const light = oceanFarLight(optics, optics, view, m, there, distance);
      const kept = 1 - light.whitecapFraction;
      const over: Rgb = [
        kept * Math.max(light.glint[0] - optics.glintCapCdM2, 0),
        kept * Math.max(light.glint[1] - optics.glintCapCdM2, 0),
        kept * Math.max(light.glint[2] - optics.glintCapCdM2, 0),
      ];
      if (over[0] + over[1] + over[2] === 0) {
        continue;
      }
      // The gnomonic grid's area element, a² dα dβ ÷ (1 + α² + β²)^(3/2), as the camera sees it.
      const area = (radius * radius * step * step) / (1 + alpha * alpha + beta * beta) ** 1.5;
      const solid = (area * muV) / (distance * distance);
      hot += solid;
      for (let k = 0; k < 3; k += 1) {
        excess[k] = (excess[k] ?? 0) + (over[k] ?? 0) * solid;
        whole[k] = (whole[k] ?? 0) + kept * (light.glint[k] ?? 0) * solid;
      }
    }
  }
  if (!(hot > 0)) {
    return null;
  }
  return {
    direction: toScene(normalise(sub(specular.pointM, cameraM))),
    angularRadiusRad: 2 * Math.asin(Math.min(Math.sqrt(hot / (4 * Math.PI)), 1)),
    excessLuminance: [excess[0] / hot, excess[1] / hot, excess[2] / hot],
    luminance: [whole[0] / hot, whole[1] / hot, whole[2] / hot],
  };
}

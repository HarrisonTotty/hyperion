/**
 * A served body's cloud-free ground from its drawn class map (plan R08, fix-r08-decked-air-opacity;
 * research-decked-ground, main's ruling of 2026-11-05): the area-weighted mean of the named classes'
 * p q per channel over the map's surveyed cells, the ground a decked thin air's bake takes in place
 * of R07's whole-disc p q, which counts the body's clouds, and its air, again under a deck the
 * medium draws (`decision-r08-t10a-leans.md` §2's own trigger).
 *
 * @remarks
 * R10 draws a class's own cloud-free albedo, its palette's A_N, at a surface pressure of
 * {@link CLOUD_FREE_CLASSES_PA} and above, where the server sends the field no albedo scale
 * (`hyperion-server`'s `field_albedo_scale`; `hyperion-sim`'s `THIN_ATMOSPHERE_PRESSURE`, 30 kPa);
 * below it the classes are scaled to the body's p, its clouds among it, so the map gives no
 * cloud-free ground there. A class is named where it has a law of its own: an unnamed class shades
 * with the map's `elsewhere`, the body's uniform law (`terrain/classMap.ts`'s `classMapOf`), and is
 * left out with the unsurveyed cells. Each texel is weighted by its cell's solid angle on R05's cube
 * sphere, the face's (u, v) rectangle through S2's quadratic warp, and by each class's weight
 * (`bodies/discSurface.ts`'s `classWeightAt`), its share of the cell. A class's p q is its law's
 * geometric albedo (`appearance/phase.ts`'s `geometricAlbedo`) times its phase integral
 * (`lawPhaseIntegral`), the figure's own scale apart, as R07's p q is the body's. A maturity's
 * per-texel albedo is left out: a world of 30 kPa and more has none (space weathering).
 */

import { atan } from "../../lib/pinnedMath";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { geometricAlbedo, lawPhaseIntegral } from "../appearance/phase";
import type { Rgb } from "../photometry/toneCurve";
import { classWeightAt, type ClassMapTexels } from "../bodies/discSurface";
import { stToUv } from "../terrain/cube";
import type { Face } from "../terrain/patchKey";

/**
 * The surface pressure from which R10 draws each class's cloud-free albedo, Pa: 30 kPa,
 * `hyperion-sim`'s `planetary::derive::photometry::THIN_ATMOSPHERE_PRESSURE` (and
 * `hyperion-surface`'s `material::classify`), at and above which the server's `field_albedo_scale`
 * sends no scale.
 */
export const CLOUD_FREE_CLASSES_PA = 30_000;

/** The cube sphere's faces. */
const FACES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** The solid angle under the corner (u, v) of a face's tangent plane, sr: atan(u v ÷ √(1 + u² + v²)). */
function cornerSolidAngle(u: number, v: number): number {
  return atan((u * v) / Math.sqrt(1 + u * u + v * v));
}

/** The solid angle of each texel of a face of N × N, row by row (j, then i), sr. */
function texelSolidAngles(faceTexels: number): Float64Array {
  const edges = Float64Array.from({ length: faceTexels + 1 }, (_, k) => stToUv(k / faceTexels));
  const out = new Float64Array(faceTexels * faceTexels);
  for (let j = 0; j < faceTexels; j += 1) {
    for (let i = 0; i < faceTexels; i += 1) {
      const u0 = edges[i] ?? Number.NaN;
      const u1 = edges[i + 1] ?? Number.NaN;
      const v0 = edges[j] ?? Number.NaN;
      const v1 = edges[j + 1] ?? Number.NaN;
      out[j * faceTexels + i] =
        cornerSolidAngle(u1, v1) -
        cornerSolidAngle(u0, v1) -
        cornerSolidAngle(u1, v0) +
        cornerSolidAngle(u0, v0);
    }
  }
  return out;
}

/** Each map's ground once formed, by its texels, with the laws and `elsewhere` it was formed under. */
const GROUNDS = new WeakMap<
  ClassMapTexels,
  {
    readonly laws: ClassMapDiscSurface["laws"];
    readonly elsewhere: ClassMapDiscSurface["elsewhere"];
    readonly ground: Rgb | null;
  }
>();

/**
 * The area-weighted mean of a class map's named classes' p q per channel over its surveyed cells
 * (the module's remarks), or `null` where no named class covers any surveyed cell. Formed once a
 * map, its laws and its `elsewhere`.
 */
export function classMapGroundAlbedo(surface: ClassMapDiscSurface): Rgb | null {
  const held = GROUNDS.get(surface.texels);
  if (held?.laws === surface.laws && held.elsewhere === surface.elsewhere) {
    return held.ground;
  }
  const ground = groundOf(surface);
  GROUNDS.set(surface.texels, { laws: surface.laws, elsewhere: surface.elsewhere, ground });
  return ground;
}

function groundOf(surface: ClassMapDiscSurface): Rgb | null {
  const map = surface.texels;
  const n = map.faceTexels;
  const angles = texelSolidAngles(n);
  const sums: [number, number, number] = [0, 0, 0];
  let weight = 0;
  for (let k = 0; k < map.classes; k += 1) {
    const law = surface.laws[k];
    if (law === undefined || law === surface.elsewhere) {
      continue;
    }
    const p = geometricAlbedo(law);
    const q = lawPhaseIntegral(law);
    let classWeight = 0;
    for (const face of FACES) {
      for (let j = 0; j < n; j += 1) {
        for (let i = 0; i < n; i += 1) {
          classWeight += (angles[j * n + i] ?? Number.NaN) * classWeightAt(map, face, i, j, k);
        }
      }
    }
    for (let c = 0; c < 3; c += 1) {
      sums[c] = (sums[c] ?? 0) + classWeight * (p[c] ?? Number.NaN) * (q[c] ?? Number.NaN);
    }
    weight += classWeight;
  }
  if (!(weight > 0)) {
    return null;
  }
  return [sums[0] / weight, sums[1] / weight, sums[2] / weight];
}

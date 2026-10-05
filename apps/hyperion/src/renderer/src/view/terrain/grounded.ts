/**
 * The grounded-body rule: the finest level, with the morph held at zero, about every grounded or
 * descending body in view (plan R05, T7.c, Design notes 6 and 9).
 *
 * @remarks
 * Within the held radius r_g, the body's bounding radius plus one finest patch, the terrain is drawn
 * at the finest level with no morph, which is exactly the collision interpolant. The morph then
 * rises to 1 across one finest patch (the ramp), and the forced region, which is always selected at
 * the finest level whatever the view's tolerance or the setting, reaches one finest patch further,
 * so that wherever a finest patch meets a coarser neighbour the morph is already 1, as CDLOD's
 * crack-freedom requires. A contact is a sphere; a swept path is a chain of contacts spaced at
 * most one radius apart, which the forced region covers without a gap.
 */

import { sub, dot } from "../../geometry/vec3";
import type { PatchBounds } from "./bounds";
import { PATCH_QUADS, vertexSpacing } from "./cube";
import type { BodyFixedVec3, PlanetGeometry } from "./planet";
import { distanceToBoxFromM } from "./viewGeometry";

/**
 * A grounded or descending body: a sphere about its position (Design note 9).
 *
 * @remarks
 * The forced region and the morph hold are measured in three dimensions from `positionM`, as the
 * terrain shader measures the hold, so a body still above the ground is passed at the point of the
 * surface beneath it (or where it will touch down): a contact high above the terrain would hold
 * nothing.
 */
export interface GroundContact {
  /** Its position, body-fixed metres from the body's centre. */
  readonly positionM: BodyFixedVec3;
  /** Its bounding radius, metres. */
  readonly radiusM: number;
}

/**
 * The time from a forced region's first request to its last patch resident, seconds, the 99th
 * percentile over the recorded runs (Design note 9). Provisional: it starts at the 30 s
 * time-to-contact threshold, an upper bound, and R05.T18 replaces it with the measured figure.
 */
export const FORCED_REGION_RESIDENCY_S = 30;

/** Below this height above the spheroid, a body moving towards it is descending, metres (provisional). */
export const DESCENT_ALTITUDE_M = 1_000;

/** Under this time to contact at its present vertical speed, a body is descending, seconds (provisional). */
export const DESCENT_TIME_TO_CONTACT_S = 30;

/**
 * Whether a body is descending (Design note 9): below {@link DESCENT_ALTITUDE_M} above the spheroid
 * under it and moving towards it, or reaching it within {@link DESCENT_TIME_TO_CONTACT_S} at its
 * present vertical speed.
 *
 * @param altitudeM - Its height above the spheroid under it, metres.
 * @param verticalSpeedMps - Its speed along the spheroid's normal there, m/s, positive upward.
 */
export function isDescending(altitudeM: number, verticalSpeedMps: number): boolean {
  if (!(verticalSpeedMps < 0)) {
    return false;
  }
  return (
    altitudeM < DESCENT_ALTITUDE_M || altitudeM / -verticalSpeedMps < DESCENT_TIME_TO_CONTACT_S
  );
}

/** The finest level's patch edge on `planet`, metres: 64 of its largest vertex spacings. */
export function finestPatchSizeM(planet: PlanetGeometry): number {
  return PATCH_QUADS * vertexSpacing(planet.figure.equatorialRadiusM, planet.finestLevel).maxM;
}

/** The held radius r_g about a contact, metres: its bounding radius plus one finest patch. */
export function heldRadiusM(contact: GroundContact, patchSizeM: number): number {
  return contact.radiusM + patchSizeM;
}

/**
 * The forced region's radius about a contact, metres: the held radius, the ramp of one finest
 * patch and a margin of one more.
 */
export function forcedRadiusM(contact: GroundContact, patchSizeM: number): number {
  return heldRadiusM(contact, patchSizeM) + 2 * patchSizeM;
}

/**
 * The width beyond the held radius over which the morph hold rises from 0 to 1, metres: one finest
 * patch (Design note 6). The terrain pass writes it as each contact's `rampM`.
 */
export function morphRampM(patchSizeM: number): number {
  return patchSizeM;
}

/**
 * Whether a patch's bounding box comes within a contact's forced radius of any contact
 * (Design note 9).
 *
 * @remarks
 * The box, not the bounding sphere: the sphere of a patch whose height range is the level's ±24 km
 * reaches far beyond its footprint, where the box stays as wide as the patch.
 */
export function inForcedRegion(
  bounds: PatchBounds,
  grounded: ReadonlyArray<GroundContact>,
  patchSizeM: number,
): boolean {
  // A loop over `distanceToBoxFromM`, with nothing allocated: selection asks it of every patch.
  for (let n = 0; n < grounded.length; n += 1) {
    const g = grounded[n];
    if (
      g !== undefined &&
      distanceToBoxFromM(bounds, g.positionM) <= forcedRadiusM(g, patchSizeM)
    ) {
      return true;
    }
  }
  return false;
}

/**
 * One contact's hold at a distance `distanceM` from its centre: clamp((d − r_g) ÷ ramp, 0, 1), and
 * a step at r_g when the ramp is 0, term for term `terrain.wgsl`'s `morphFactor` (lane C's
 * R05.T11.b), which evaluates it in `f32` on camera-relative positions.
 */
export function contactHold(distanceM: number, heldM: number, rampM: number): number {
  const beyond = distanceM - heldM;
  if (!(rampM > 0)) {
    return beyond > 0 ? 1 : 0;
  }
  return Math.min(Math.max(beyond / rampM, 0), 1);
}

/**
 * The morph hold at a vertex's unmorphed position (Design note 6): the least over the contacts of
 * {@link contactHold} with the contact's held radius ({@link heldRadiusM}) and ramp
 * ({@link morphRampM}), 1 with no contact. The morph factor is min(k_CDLOD, morphHold), a function
 * of position alone, so the patches sharing a vertex agree.
 *
 * @param v - The vertex's unmorphed position, body-fixed metres.
 * @param patchSizeM - {@link finestPatchSizeM} of the planet.
 */
export function morphHold(
  v: BodyFixedVec3,
  grounded: ReadonlyArray<GroundContact>,
  patchSizeM: number,
): number {
  let hold = 1;
  for (const g of grounded) {
    const d = sub(v, g.positionM);
    hold = Math.min(
      hold,
      contactHold(Math.sqrt(dot(d, d)), heldRadiusM(g, patchSizeM), morphRampM(patchSizeM)),
    );
  }
  return hold;
}

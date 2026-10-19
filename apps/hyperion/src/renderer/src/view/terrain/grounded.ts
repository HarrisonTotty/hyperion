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
import { PACKED_BOUNDS_LENGTH, packBounds, type PatchBounds } from "./bounds";
import { PATCH_QUADS, vertexSpacing } from "./cube";
import type { BodyFigure, BodyFixedVec3, PlanetGeometry } from "./planet";
import { distanceToPackedBoxFromM } from "./viewGeometry";

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

/**
 * Where a craft stands over a body's ground: its height above the ground beneath it and that
 * ground's point and outward normal (plan R10, R10.T16.b), which decide whether it is a contact
 * and where the contact is passed.
 */
export interface GroundUnder {
  /** The craft's height above the ground beneath it, metres, along the ground's normal. */
  readonly heightM: number;
  /** The point of the ground beneath it, body-fixed metres. */
  readonly groundPointM: BodyFixedVec3;
  /** The ground's outward unit normal there, body-fixed. */
  readonly normal: BodyFixedVec3;
}

/**
 * The ground under a body-fixed point: R10.T16.b's one function for a contact's height and ground
 * point (decision-r10-t16-contracts §2), the datum's ({@link datumGroundUnder}) until R10.T14.c
 * gives it the workers' ground where it is held.
 */
export type GroundUnderFunction = (positionM: BodyFixedVec3) => GroundUnder;

/**
 * The datum's ground under a point: the spheroid point M·d of the direction d whose parametric
 * image holds the point (d = M⁻¹p ÷ |M⁻¹p|, the cube sphere's own mapping), the height the point's
 * offset from it along the spheroid's normal there.
 *
 * @remarks
 * Exact on a sphere. On a flattened body the parametric direction and the normal differ by up to
 * about f rad, so the height is short by up to about h f² ÷ 2 and the ground point stands up to
 * about h f off the normal's foot: 6 mm and 3.4 m at the 1 km a descent is judged within, for
 * WGS 84's f = 1 ÷ 298.257223563 (NIMA TR8350.2, 3rd ed., 2000, Table 3.1), well inside a finest
 * patch. Over surveyed relief the datum can lie kilometres from the ground: R10.T14.c replaces it
 * with the workers' ground, and amends Design note 9's "above the spheroid" (the ruling's §2).
 */
export function datumGroundUnder(
  figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">,
): GroundUnderFunction {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  return (p) => {
    const mx = p.x / a;
    const my = p.y / a;
    const mz = p.z / c;
    const m = Math.hypot(mx, my, mz);
    if (!(m > 0)) {
      // The body's centre: no direction, so the ground is taken straight up the pole.
      return { heightM: -c, groundPointM: { x: 0, y: 0, z: c }, normal: { x: 0, y: 0, z: 1 } };
    }
    const ground = { x: (a * mx) / m, y: (a * my) / m, z: (c * mz) / m };
    const nx = mx / m / a;
    const ny = my / m / a;
    const nz = mz / m / c;
    const n = Math.hypot(nx, ny, nz);
    const normal = { x: nx / n, y: ny / n, z: nz / n };
    const heightM =
      (p.x - ground.x) * normal.x + (p.y - ground.y) * normal.y + (p.z - ground.z) * normal.z;
    return { heightM, groundPointM: ground, normal };
  };
}

/**
 * A craft as a contact (Design note 9; R10.T16.b): passed at the ground beneath it, at its bounding
 * radius, while grounded (its height within its held radius, {@link heldRadiusM}, so that its held
 * sphere reaches the ground, as R05's spike rules it) or descending ({@link isDescending}); `null`
 * otherwise.
 *
 * @param positionM - The craft's position, body-fixed metres from the body's centre.
 * @param velocityMps - Its velocity against the body, body-fixed m/s, or `null` where it is not
 *   known, when it is a contact only while grounded.
 * @param radiusM - Its bounding radius, metres (the hull's farthest vertex from its reference).
 * @param ground - The ground under a point, the datum's until R10.T14.c ({@link datumGroundUnder}).
 * @param patchSizeM - The finest patch's edge, metres ({@link finestPatchSizeM}).
 */
export function craftContact(
  positionM: BodyFixedVec3,
  velocityMps: BodyFixedVec3 | null,
  radiusM: number,
  ground: GroundUnderFunction,
  patchSizeM: number,
): GroundContact | null {
  const under = ground(positionM);
  const contact: GroundContact = { positionM: under.groundPointM, radiusM };
  const grounded = under.heightM <= heldRadiusM(contact, patchSizeM);
  const descending =
    velocityMps !== null && isDescending(under.heightM, dot(velocityMps, under.normal));
  return grounded || descending ? contact : null;
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
 * {@link distanceToPackedBoxFromM}, bound once: selection asks the rule of every patch, and under a
 * module runner (the descent record's harness runs selection under Vite's) each read of an
 * imported binding is a getter call.
 */
const distanceOf = distanceToPackedBoxFromM;

/** Scratch for packing a {@link PatchBounds} for {@link inForcedRegionPacked}; used within one call. */
const PACKED = new Float64Array(PACKED_BOUNDS_LENGTH);

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
  packBounds(PACKED, bounds);
  return inForcedRegionPacked(PACKED, grounded, patchSizeM);
}

/**
 * {@link inForcedRegion} of packed bounds `p` ({@link PACKED_BOUNDS_LENGTH}'s layout): the rule's
 * one form, which selection asks of every patch on the bounds it keeps packed.
 */
export function inForcedRegionPacked(
  p: Float64Array,
  grounded: ReadonlyArray<GroundContact>,
  patchSizeM: number,
): boolean {
  // A loop over `distanceToPackedBoxFromM`, with nothing allocated.
  for (let n = 0; n < grounded.length; n += 1) {
    const g = grounded[n];
    if (g !== undefined && distanceOf(p, g.positionM) <= forcedRadiusM(g, patchSizeM)) {
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

/**
 * Which bodies can eclipse a star for another body: the shadow-cone pre-test, in `f64` on the CPU
 * (plan R07, Design note 6).
 *
 * @remarks
 * A body O of radius R_o at distance d from a star of radius R★ casts an umbra of radius
 * r_u = R_o − x (R★ − R_o) ÷ d and a penumbra of radius r_p = R_o + x (R★ + R_o) ÷ d at a
 * distance x behind it along the star–O axis. A body B can be eclipsed by O only where B's sphere
 * meets O's penumbral cone; the list is usually empty. A `contact` body of R03's frame, known only
 * by its apparent direction, is never an occluder (decision, 2026-10-02, item 4 of
 * `decisions-r06-r07.md`), and is never lit by this term either.
 */
import { dot, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import type { SceneBodyFrame } from "../../lib/scene/apparent";
import type { Occluder } from "./annuli";

/** A sphere the lighting terms take part in: a placed body, or a star. */
export interface LightingSphere {
  /** Its centre at the light's time, m. */
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/** A placed body as the lighting terms see it. */
export interface LightingBody extends LightingSphere {
  readonly id: string;
}

/**
 * A scene body as a sphere for the lighting terms, or `null` for a `contact` entry or a body of
 * unknown radius, which neither occludes nor is eclipsed.
 *
 * @param radiusM - The body's radius, m, from its summary; `null` where it is not granted.
 */
export function lightingBodyOf(frame: SceneBodyFrame, radiusM: number | null): LightingBody | null {
  let body: LightingBody | null;
  switch (frame.kind) {
    case "placed":
      body =
        radiusM === null || radiusM <= 0
          ? null
          : { id: frame.id, centreM: frame.geometricM, radiusM };
      break;
    case "contact":
      body = null;
      break;
  }
  return body;
}

/** The umbra's radius at x behind an occluder, m: R_o − x (R★ − R_o) ÷ d; negative past its apex. */
export function umbraRadius(
  occluderRadiusM: number,
  starRadiusM: number,
  starDistanceM: number,
  behindM: number,
): number {
  return occluderRadiusM - (behindM * (starRadiusM - occluderRadiusM)) / starDistanceM;
}

/** The penumbra's radius at x behind an occluder, m: R_o + x (R★ + R_o) ÷ d. */
export function penumbraRadius(
  occluderRadiusM: number,
  starRadiusM: number,
  starDistanceM: number,
  behindM: number,
): number {
  return occluderRadiusM + (behindM * (starRadiusM + occluderRadiusM)) / starDistanceM;
}

/** Whether `body` meets `occluder`'s penumbral cone from `star`. */
function inPenumbra(body: LightingSphere, occluder: LightingSphere, star: LightingSphere): boolean {
  const axis = sub(occluder.centreM, star.centreM);
  const distance = norm(axis);
  const along = scale(axis, 1 / distance);
  const offset = sub(body.centreM, occluder.centreM);
  const behind = dot(offset, along);
  if (behind + body.radiusM <= 0) {
    return false;
  }
  const across = norm(sub(offset, scale(along, behind)));
  const radius = penumbraRadius(occluder.radiusM, star.radiusM, distance, Math.max(behind, 0));
  return across < radius + body.radiusM;
}

/**
 * The bodies that may eclipse any of `stars` for `body`: each other body whose penumbral cone from
 * a star meets the body's sphere, nearer the star than the body's far side.
 */
export function occludersFor(
  body: LightingBody,
  stars: ReadonlyArray<LightingSphere>,
  bodies: ReadonlyArray<LightingBody>,
): LightingBody[] {
  return bodies.filter(
    (other) => other.id !== body.id && stars.some((star) => inPenumbra(body, other, star)),
  );
}

/** The occluders as the eclipse term takes them. */
export function asOccluders(bodies: ReadonlyArray<LightingBody>): Occluder[] {
  return bodies.map(({ centreM, radiusM }) => ({ centreM, radiusM }));
}

/**
 * The scripted occultation (plan R07, T9): a moon passing behind a planet's limb, both resolved,
 * over {@link OCCULTATION_STEPS} steps, the test, smoke check and captures of the mesh regime.
 *
 * @remarks
 * An Earth-sized planet 5 × 10⁷ m ahead, 45 px across in a 128 × 96, 40° view, and a
 * Ganymede-sized moon 7.6 × 10⁷ m from the camera, beyond it (2.7 to 3.2 × 10⁷ m from its centre),
 * about 12 px across, sliding along a line 25° above the view's x axis from 0.30 rad off the planet's centre, its footprint clear of the planet's, to
 * 0.08 rad, wholly behind it. A Sun-like star 1 au away lights both from above and in front, so
 * that the planet's shadow falls below and behind it, never on the moon. A depth writer over the
 * planet's footprint, standing for R10's terrain, makes the planet a mesh on every step
 * (`depthWriters`); the moon is a disc while its footprint is clear of the planet's and is
 * promoted once they overlap, so the script crosses the switch. Without it (`promoted` false) both
 * are discs in the painter's order, the same frames as T5 and T8.a draw them. Not in the `SCENE`
 * selector: nothing in a view writes depth yet, so a view would draw the discs only.
 */
import { add, scale, vec3, type Vec3 } from "../../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { LitBodyInput } from "../bodies/draw";
import { type ScreenCircle, sphereFootprint } from "../bodies/regime";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import type { PlacedLight } from "../lighting/hostLights";
import { AU_M, keptBody } from "./kept";

/** The script's steps. */
export const OCCULTATION_STEPS = 24;

/** The view the script is drawn in. */
export const OCCULTATION_VIEWPORT: Viewport = { widthPx: 128, heightPx: 96 };

/** The camera: at the origin, looking down −z, 40° across. */
export const OCCULTATION_CAMERA: ProjectionCamera = {
  orientation: IDENTITY_QUATERNION,
  fovXRad: (40 * Math.PI) / 180,
};

/** The planet, body index 1. */
export const OCCULTATION_PLANET = keptBody(1);

/** The moon, body index 2. */
export const OCCULTATION_MOON = keptBody(2);

/** The planet's radius, m: Earth's volumetric mean radius (NASA Earth Fact Sheet). */
const PLANET_RADIUS_M = 6.371e6;

/** The planet's centre from the camera, m. */
const PLANET_CENTRE_M: Vec3 = vec3(0, 0, -5e7);

/** The moon's radius, m: a test value, about Ganymede's. */
const MOON_RADIUS_M = 2.6e6;

/** The moon's distance from the camera, m: 2.6 × 10⁷ m beyond the planet's centre. */
const MOON_DISTANCE_M = 7.6e7;

/** The moon's angle off the planet's centre at the first and last steps, rad. */
const MOON_ANGLE_START_RAD = 0.3;
const MOON_ANGLE_END_RAD = 0.08;

/** The moon's path's angle above the view's x axis, rad. */
const MOON_PATH_RAD = (25 * Math.PI) / 180;

/** The unit direction from the bodies to the star: above, in front and a little to the left. */
const TOWARDS_STAR: Vec3 = (() => {
  const v = vec3(-0.3, 0.75, 0.59);
  const length = Math.hypot(v.x, v.y, v.z);
  return vec3(v.x / length, v.y / length, v.z / length);
})();

/** One step of the script: its bodies, its light and the view's depth writers. */
export interface OccultationFrame {
  readonly bodies: ReadonlyArray<LitBodyInput>;
  readonly lights: ReadonlyArray<PlacedLight>;
  readonly depthWriters: ReadonlyArray<ScreenCircle>;
}

/** A sphere of `radiusM`, of no known pole. */
function sphere(radiusM: number): LitBodyInput["figure"] {
  return { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: null };
}

/** The moon's angle off the planet's centre at `step`, rad. */
export function occultationMoonAngleRad(step: number): number {
  return (
    MOON_ANGLE_START_RAD +
    ((MOON_ANGLE_END_RAD - MOON_ANGLE_START_RAD) * step) / (OCCULTATION_STEPS - 1)
  );
}

/**
 * Step `step` of the script, 0 to {@link OCCULTATION_STEPS} − 1.
 *
 * @param promoted - Whether a depth writer over the planet makes it a mesh.
 * @param viewport - The view the depth writer's footprint is taken on, at
 *   {@link OCCULTATION_CAMERA}'s field: the script's own, or a larger one for the captures.
 */
export function occultationFrame(
  step: number,
  promoted = true,
  viewport: Viewport = OCCULTATION_VIEWPORT,
): OccultationFrame {
  const angle = occultationMoonAngleRad(step);
  const moonCentre = vec3(
    MOON_DISTANCE_M * Math.sin(angle) * Math.cos(MOON_PATH_RAD),
    MOON_DISTANCE_M * Math.sin(angle) * Math.sin(MOON_PATH_RAD),
    -MOON_DISTANCE_M * Math.cos(angle),
  );
  const bodies: LitBodyInput[] = [
    {
      id: OCCULTATION_PLANET,
      centreM: PLANET_CENTRE_M,
      figure: sphere(PLANET_RADIUS_M),
      photometry: PROVISIONAL_PHOTOMETRY,
    },
    {
      id: OCCULTATION_MOON,
      centreM: moonCentre,
      figure: sphere(MOON_RADIUS_M),
      photometry: PROVISIONAL_PHOTOMETRY,
    },
  ];
  const lights: PlacedLight[] = [
    { disc: sunLikeHostDisc(), centreM: add(PLANET_CENTRE_M, scale(TOWARDS_STAR, AU_M)) },
  ];
  const planet = sphereFootprint(PLANET_CENTRE_M, PLANET_RADIUS_M, OCCULTATION_CAMERA, viewport);
  return { bodies, lights, depthWriters: promoted && planet !== null ? [planet] : [] };
}

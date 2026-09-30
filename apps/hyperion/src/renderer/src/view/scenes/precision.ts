import { add, vec3, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { IDENTITY_QUATERNION, multiply, quaternionFromAxisAngle } from "../camera/quaternion";
import { TEST_HULL } from "../scene/hull";
import type { ViewScene } from "../scene/model";
import {
  AU_M,
  KEPT_BARYCENTRE,
  KEPT_SYSTEM,
  KEPT_TIDAL_RADIUS_M,
  type KeptScene,
  keptBody,
  keptTime,
} from "./kept";

/** The precision scene's name, as the `SCENE` selector shows it. */
export const PRECISION_SCENE_NAME = "PRECISION TEST";

/** The own ship's craft in the precision scene. */
export const PRECISION_SHIP = "test-ship";

/** The precision scene's moon, 10⁸ m ahead of the ship. */
export const PRECISION_MOON = keptBody(2);

/** The precision scene's planet, 1 au ahead of the ship. */
export const PRECISION_PLANET = keptBody(1);

/**
 * The ship's place, m from the barycentre in the system frame: some 2.4 au out, so that the
 * scene's positions are large numbers and a naive `f32` path would show it.
 */
export const PRECISION_SHIP_M: Vec3 = vec3(2.1e11, 2.9e11, 1.3e10);

/** The moon's offset from the ship, m: 10⁸ m ahead (−z) and a little to starboard and up. */
const MOON_OFFSET_M = vec3(4e6, 3e6, -1e8);

/** The planet's offset from the ship, m: 1 au ahead, a little to port. */
const PLANET_OFFSET_M = vec3(-2e9, -1e9, -AU_M);

/** How long the precision scene's camera script runs, s. */
export const PRECISION_DURATION_S = 20;

/**
 * The precision scene (plan R02, R02.T11.b): the test hull's plate 1 m from the seat's eye point, a
 * moon at 10⁸ m and a planet at 1 au, all in the system frame (no body is a frame candidate), and a
 * seat camera that sways and turns along a scripted path.
 *
 * @remarks
 * The camera is held in the ship's `craft` frame (Design note 22). Along the path it translates by
 * up to 0.3 m and turns by up to 6° in yaw and 3° in pitch, so that every mark moves across the
 * screen from the plate at 1 m to the planet at 1 au; a jitter or a step would show against that
 * smooth motion.
 */
export function precisionScene(): KeptScene {
  return {
    name: PRECISION_SCENE_NAME,
    durationS: PRECISION_DURATION_S,
    sceneAt: (tS) => precisionSceneAt(tS),
    cameraAt: (tS) => precisionCameraAt(tS),
  };
}

function precisionSceneAt(tS: number): ViewScene {
  const seat: CameraPose = {
    frame: { kind: "craft", craft: PRECISION_SHIP },
    positionM: TEST_HULL.eyePointM,
    orientation: IDENTITY_QUATERNION,
  };
  return {
    provenance: { kind: "kept", name: PRECISION_SCENE_NAME },
    time: keptTime(tS),
    timeRate: 1,
    system: KEPT_SYSTEM,
    barycentre: KEPT_BARYCENTRE,
    tidalRadiusM: KEPT_TIDAL_RADIUS_M,
    bodies: [
      {
        id: PRECISION_PLANET,
        parent: null,
        kind: "planet",
        designation: "TEST PLANET",
        // Earth's mean radius (IUGG; Moritz, Geodetic Reference System 1980).
        radiusM: 6.371e6,
        hillRadiusM: null,
        centreM: add(PRECISION_SHIP_M, PLANET_OFFSET_M),
        rotation: null,
      },
      {
        id: PRECISION_MOON,
        parent: null,
        kind: "moon",
        designation: "TEST MOON",
        // The Moon's mean radius (Archinal et al. 2018, IAU WGCCRE).
        radiusM: 1.7374e6,
        hillRadiusM: null,
        centreM: add(PRECISION_SHIP_M, MOON_OFFSET_M),
        rotation: null,
      },
    ],
    rings: [],
    orbits: [],
    craft: [
      {
        id: PRECISION_SHIP,
        designation: "TEST HULL",
        hull: TEST_HULL,
        pose: {
          position: { kind: "system", system: KEPT_SYSTEM, m: PRECISION_SHIP_M },
          attitude: IDENTITY_QUATERNION,
        },
        predictedPath: null,
        velocityMPerS: null,
      },
    ],
    stars: [],
    ownShip: PRECISION_SHIP,
    defaultPose: seat,
  };
}

/** The scripted seat camera: a sway of up to 0.3 m and a turn of up to 6° about the eye point. */
function precisionCameraAt(tS: number): CameraPose {
  const phase = (2 * Math.PI * tS) / PRECISION_DURATION_S;
  const yaw = quaternionFromAxisAngle(vec3(0, 1, 0), ((6 * Math.PI) / 180) * Math.sin(phase));
  const pitch = quaternionFromAxisAngle(vec3(1, 0, 0), ((3 * Math.PI) / 180) * Math.sin(2 * phase));
  const sway = vec3(0.3 * Math.sin(phase), 0.1 * Math.sin(3 * phase), 0.05 * Math.cos(phase));
  return {
    frame: { kind: "craft", craft: PRECISION_SHIP },
    positionM: add(TEST_HULL.eyePointM, sway),
    orientation: multiply(yaw, pitch),
  };
}

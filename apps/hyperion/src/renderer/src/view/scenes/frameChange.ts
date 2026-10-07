import { add, cross, normalise, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import type { CameraPose, Quaternion } from "../camera/pose";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import type { ViewPosition } from "../coords/position";
import { type Rotation3, rotateToBody, rotation3FromRows } from "../coords/rotation";
import { TEST_HULL } from "../scene/hull";
import { bodyKindSymbol, staticRetarded, type ViewScene } from "../scene/model";
import {
  AU_M,
  KEPT_BARYCENTRE,
  KEPT_SYSTEM,
  KEPT_TIDAL_RADIUS_M,
  type KeptScene,
  keptBody,
  keptTime,
} from "./kept";

/** The frame-change scene's name, as the `SCENE` selector shows it. */
export const FRAME_CHANGE_SCENE_NAME = "FRAME CHANGE TEST";

/** The scene's star, at the barycentre. */
export const FRAME_CHANGE_STAR = keptBody(0);

/** The scene's Earth-like planet, 1 au out. */
export const FRAME_CHANGE_PLANET = keptBody(3);

/** The planet's Moon-like moon. */
export const FRAME_CHANGE_MOON = keptBody(4);

/** The own ship, flying the scripted path through both Hill spheres. */
export const FRAME_CHANGE_SHIP = "test-ship";

/** A test craft grounded on the planet. */
export const FRAME_CHANGE_LANDER = "test-lander";

/** The planet's centre, m from the barycentre. */
export const FRAME_CHANGE_PLANET_M: Vec3 = vec3(AU_M, 0, 0);

/** The moon's centre, m from the barycentre: 3.844 × 10⁸ m from the planet. */
export const FRAME_CHANGE_MOON_M: Vec3 = add(FRAME_CHANGE_PLANET_M, vec3(0, 3.844e8, 0));

/** The planet's radius, m: Earth's mean radius, 6,371.0 km (IUGG; Moritz, Geodetic Reference System 1980). */
export const FRAME_CHANGE_PLANET_RADIUS_M = 6.371e6;

/** The planet's Hill radius, m: Earth's, 1.5 × 10⁹ m (R02.T4's figure). */
export const FRAME_CHANGE_PLANET_HILL_M = 1.5e9;

/** The moon's Hill radius, m: the Moon's, about 5.8 × 10⁷ m (R02.T4's figure). */
export const FRAME_CHANGE_MOON_HILL_M = 5.8e7;

/**
 * The planet's hand-set spin, rad/s: Earth's sidereal rate, 7.292115 × 10⁻⁵ (IERS Conventions
 * 2010, table 1.1).
 */
export const FRAME_CHANGE_SPIN_RAD_PER_S = 7.292_115e-5;

/** The planet's obliquity, rad: Earth's 23.44°, a hand-set tilt of its pole from the +z axis. */
const OBLIQUITY_RAD = (23.44 * Math.PI) / 180;

/** The prime meridian's angle at the script's start, rad. */
const MERIDIAN_AT_START_RAD = 0.4;

/** How long the frame-change scene's script runs, s. */
export const FRAME_CHANGE_DURATION_S = 600;

/** A landmark on the planet: its prime meridian on its equator, at its surface. */
export const FRAME_CHANGE_LANDMARK: ViewPosition = {
  kind: "body_fixed",
  body: FRAME_CHANGE_PLANET,
  m: vec3(FRAME_CHANGE_PLANET_RADIUS_M, 0, 0),
};

/** Where the lander stands: at 10° north and 20° east on the planet's surface, body-fixed. */
export const FRAME_CHANGE_LANDER_AT: Extract<ViewPosition, { kind: "body_fixed" }> = {
  kind: "body_fixed",
  body: FRAME_CHANGE_PLANET,
  m: vec3(
    FRAME_CHANGE_PLANET_RADIUS_M * Math.cos((10 * Math.PI) / 180) * Math.cos((20 * Math.PI) / 180),
    FRAME_CHANGE_PLANET_RADIUS_M * Math.cos((10 * Math.PI) / 180) * Math.sin((20 * Math.PI) / 180),
    FRAME_CHANGE_PLANET_RADIUS_M * Math.sin((10 * Math.PI) / 180),
  ),
};

/**
 * The planet's rotation from body-fixed to body axes at `tS`: a turn of the meridian by the spin
 * about the body-fixed pole, then the pole tilted about +x by the obliquity (R · p =
 * Tilt · Spin(θ) · p).
 */
export function frameChangeRotationAt(tS: number): Rotation3 {
  const theta = MERIDIAN_AT_START_RAD + FRAME_CHANGE_SPIN_RAD_PER_S * tS;
  const c = Math.cos(theta);
  const s = Math.sin(theta);
  const ce = Math.cos(OBLIQUITY_RAD);
  const se = Math.sin(OBLIQUITY_RAD);
  // Tilt = [[1, 0, 0], [0, ce, −se], [0, se, ce]]; Spin = [[c, −s, 0], [s, c, 0], [0, 0, 1]].
  return rotation3FromRows([vec3(c, -s, 0), vec3(ce * s, ce * c, -se), vec3(se * s, se * c, ce)]);
}

/** The own ship's path's corners, m from the barycentre, crossed in equal thirds of the script. */
const SHIP_PATH_M: readonly Vec3[] = [
  // Outside the planet's Hill sphere (1.8 × 10⁹ m from it).
  add(FRAME_CHANGE_PLANET_M, vec3(-1.8e9, 0, 2e7)),
  // Inside the moon's (2 × 10⁷ m from it).
  add(FRAME_CHANGE_MOON_M, vec3(-2e7, 0, 0)),
  // Back out of both, beyond the moon (2 × 10⁹ m from the planet).
  add(FRAME_CHANGE_PLANET_M, vec3(3e8, 2e9, -1e7)),
];

/** A point along a polyline, `fraction` 0 at its first corner and 1 at its last, equal legs. */
function alongPath(corners: readonly Vec3[], fraction: number): Vec3 {
  const legs = corners.length - 1;
  const at = Math.min(Math.max(fraction, 0), 1) * legs;
  const leg = Math.min(Math.floor(at), legs - 1);
  const from = corners[leg];
  const to = corners[leg + 1];
  if (from === undefined || to === undefined) {
    throw new Error(`a path of ${String(corners.length)} corners has no leg ${String(leg)}`);
  }
  return add(from, scale(sub(to, from), at - leg));
}

/** The own ship's position at `tS`, m from the barycentre: through both Hill spheres and out. */
export function frameChangeShipAt(tS: number): Vec3 {
  return alongPath(SHIP_PATH_M, tS / FRAME_CHANGE_DURATION_S);
}

/**
 * The scripted free camera's position at `tS`, m from the barycentre: the ship's path displaced by
 * 3 × 10⁷ m, so that it crosses each sphere's boundary at another moment from the ship.
 */
export function frameChangeCameraAt(tS: number): Vec3 {
  return add(frameChangeShipAt(tS), vec3(0, -3e7, 1.5e7));
}

/**
 * The frame-change scene (plan R02, R02.T11.c): a planet with a hand-set rotation, a moon inside its
 * Hill sphere, a landmark on the planet and a grounded test craft; the own ship flies into the
 * planet's and the moon's Hill spheres and out, and the scripted free camera does too, apart from
 * it. R02.T8.b's tests run on it.
 */
export function frameChangeScene(): KeptScene {
  return {
    name: FRAME_CHANGE_SCENE_NAME,
    durationS: FRAME_CHANGE_DURATION_S,
    sceneAt: (tS) => frameChangeSceneAt(tS),
    cameraAt: (tS) => {
      const positionM = frameChangeCameraAt(tS);
      return {
        frame: { kind: "system", system: KEPT_SYSTEM },
        positionM,
        orientation: lookAlong(sub(FRAME_CHANGE_PLANET_M, positionM), vec3(0, 0, 1)),
      };
    },
  };
}

/**
 * The lander's attitude: standing upright on the surface, its dorsal axis along the local vertical
 * and its nose to the east, both turned through the planet's rotation into the body frame.
 */
function landerAttitude(rotation: Rotation3): Quaternion {
  const up = normalise(rotateToBody(rotation, FRAME_CHANGE_LANDER_AT.m));
  const pole = rotateToBody(rotation, vec3(0, 0, 1));
  return lookAlong(cross(pole, up), up);
}

function frameChangeSceneAt(tS: number): ViewScene {
  const rotation = frameChangeRotationAt(tS);
  const defaultPose: CameraPose = {
    frame: { kind: "craft", craft: FRAME_CHANGE_SHIP },
    positionM: TEST_HULL.eyePointM,
    orientation: IDENTITY_QUATERNION,
  };
  return {
    provenance: { kind: "kept", name: FRAME_CHANGE_SCENE_NAME },
    time: keptTime(tS),
    timeRate: 1,
    system: KEPT_SYSTEM,
    barycentre: KEPT_BARYCENTRE,
    tidalRadiusM: KEPT_TIDAL_RADIUS_M,
    bodies: [
      {
        id: FRAME_CHANGE_STAR,
        parent: null,
        kind: "star",
        designation: "TEST STAR",
        // The nominal solar radius (IAU 2015 Resolution B3).
        radiusM: 6.957e8,
        hillRadiusM: null,
        centreM: vec3(0, 0, 0),
        retarded: staticRetarded(vec3(0, 0, 0)),
        rotation: null,
        appearance: null,
        symbol: bodyKindSymbol("star"),
        orbitNormal: null,
      },
      {
        id: FRAME_CHANGE_PLANET,
        parent: FRAME_CHANGE_STAR,
        kind: "planet",
        designation: "TEST PLANET",
        radiusM: FRAME_CHANGE_PLANET_RADIUS_M,
        hillRadiusM: FRAME_CHANGE_PLANET_HILL_M,
        centreM: FRAME_CHANGE_PLANET_M,
        retarded: staticRetarded(FRAME_CHANGE_PLANET_M),
        rotation,
        appearance: null,
        symbol: bodyKindSymbol("planet"),
        orbitNormal: null,
      },
      {
        id: FRAME_CHANGE_MOON,
        parent: FRAME_CHANGE_PLANET,
        kind: "moon",
        designation: "TEST MOON",
        // The Moon's mean radius, 1,737.4 km (Archinal et al. 2018, IAU WGCCRE).
        radiusM: 1.7374e6,
        hillRadiusM: FRAME_CHANGE_MOON_HILL_M,
        centreM: FRAME_CHANGE_MOON_M,
        retarded: staticRetarded(FRAME_CHANGE_MOON_M),
        rotation: null,
        appearance: null,
        symbol: bodyKindSymbol("moon"),
        orbitNormal: null,
      },
    ],
    rings: [],
    orbits: [],
    craft: [
      {
        id: FRAME_CHANGE_SHIP,
        designation: "TEST HULL",
        hull: TEST_HULL,
        pose: {
          position: { kind: "system", system: KEPT_SYSTEM, m: frameChangeShipAt(tS) },
          attitude: IDENTITY_QUATERNION,
        },
        predictedPath: null,
        velocityMPerS: null,
      },
      {
        id: FRAME_CHANGE_LANDER,
        designation: "TEST LANDER",
        hull: TEST_HULL,
        pose: { position: FRAME_CHANGE_LANDER_AT, attitude: landerAttitude(rotation) },
        predictedPath: null,
        velocityMPerS: null,
      },
    ],
    stars: [],
    ownShip: FRAME_CHANGE_SHIP,
    defaultPose,
  };
}

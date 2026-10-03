/**
 * The kept phase scene (plan R07, T8.a): three Earth-sized test planets about 106 px across at
 * 1080p and 60°, seen from one camera 1 au from a Sun-like star at phases 0°, 90° and 150°, and a
 * Jupiter-sized test giant 10¹⁰ m away at 60°, about 23 px across (T8.a's by-hand Jupiter).
 *
 * @remarks
 * The camera sits 1 au from the star along +x, in the system frame, with no own ship; each planet is
 * 2 × 10⁸ m from it, placed so that the star lights it at its phase: the full planet opposite the
 * star, the half planet across, the crescent towards the star at 30° from it. The star is lit by
 * `sunLikeHostDisc`, the scene's own host disc (decision-r07-t8a, item 1). The camera starts on the
 * half planet; the target keys step to the others. Static: nothing moves.
 */
import { add, scale, vec3, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { SUN_RADIUS_M, sunLikeHostDisc } from "../lighting/hostDisc";
import { bodyKindSymbol, type ViewBody, type ViewScene } from "../scene/model";
import {
  AU_M,
  KEPT_BARYCENTRE,
  KEPT_SYSTEM,
  KEPT_TIDAL_RADIUS_M,
  type KeptScene,
  keptBody,
  keptTime,
} from "./kept";

/** The phase scene's name, as the `SCENE` selector shows it. */
export const PHASE_SCENE_NAME = "PHASE TEST";

/** The scene's star, body index 0, the host disc's `star`. */
export const PHASE_STAR = keptBody(0);

/** The planets' phases, degrees, in body order. */
export const PHASE_SCENE_PHASES_DEG = [0, 90, 150] as const;

/** The planets, body indices 1 to 3, one per phase. */
export const PHASE_PLANETS = [keptBody(1), keptBody(2), keptBody(3)] as const;

/** The test giant, body index 4. */
export const PHASE_GIANT = keptBody(4);

/** The giant's radius, m: Jupiter's volumetric mean radius (NASA Jupiter Fact Sheet). */
export const PHASE_GIANT_RADIUS_M = 6.9911e7;

/** The giant's distance from the camera, m. */
export const PHASE_GIANT_DISTANCE_M = 1e10;

/** The giant's phase, degrees. */
export const PHASE_GIANT_PHASE_DEG = 60;

/** The camera's place, m from the barycentre: 1 au along +x. */
export const PHASE_CAMERA_M: Vec3 = vec3(AU_M, 0, 0);

/** Each planet's distance from the camera, m. */
export const PHASE_PLANET_DISTANCE_M = 2e8;

/** The planets' radius, m: Earth's volumetric mean radius (NASA Earth Fact Sheet). */
export const PHASE_PLANET_RADIUS_M = 6.371e6;

/** How long the static scene runs before it starts again, s. */
const PHASE_DURATION_S = 60;

const RAD_PER_DEG = Math.PI / 180;

/**
 * The direction from the camera to the planet at `phaseDeg`: −d̂ is at the phase from the star's
 * direction, the star being far enough (1 au against 2 × 10⁸ m) that its direction from the planet
 * is the camera's to 0.08°.
 */
export function phasePlanetDirection(phaseDeg: number): Vec3 {
  const alpha = phaseDeg * RAD_PER_DEG;
  // The star lies along −x from the camera; −d̂ = cos α (−x̂) + sin α ẑ.
  return vec3(Math.cos(alpha), 0, -Math.sin(alpha));
}

function planet(index: number, phaseDeg: number): ViewBody {
  const id = PHASE_PLANETS[index] ?? keptBody(index + 1);
  return {
    id,
    parent: null,
    kind: "planet",
    designation: `TEST PLANET ${String(phaseDeg)}°`,
    radiusM: PHASE_PLANET_RADIUS_M,
    hillRadiusM: null,
    centreM: add(PHASE_CAMERA_M, scale(phasePlanetDirection(phaseDeg), PHASE_PLANET_DISTANCE_M)),
    rotation: null,
    symbol: bodyKindSymbol("planet"),
    orbitNormal: null,
  };
}

/** The camera's pose: at the camera's place, looking at the planet at 90° with +y up. */
function cameraPose(): CameraPose {
  return {
    frame: { kind: "system", system: KEPT_SYSTEM },
    positionM: PHASE_CAMERA_M,
    orientation: lookAlong(phasePlanetDirection(90), vec3(0, 1, 0)),
  };
}

function phaseSceneAt(tS: number): ViewScene {
  return {
    provenance: { kind: "kept", name: PHASE_SCENE_NAME },
    time: keptTime(tS),
    timeRate: 1,
    system: KEPT_SYSTEM,
    barycentre: KEPT_BARYCENTRE,
    tidalRadiusM: KEPT_TIDAL_RADIUS_M,
    bodies: [
      {
        id: PHASE_STAR,
        parent: null,
        kind: "star",
        designation: "TEST STAR",
        radiusM: SUN_RADIUS_M,
        hillRadiusM: null,
        centreM: vec3(0, 0, 0),
        rotation: null,
        symbol: bodyKindSymbol("star"),
        orbitNormal: null,
      },
      ...PHASE_SCENE_PHASES_DEG.map((phaseDeg, index) => planet(index, phaseDeg)),
      {
        id: PHASE_GIANT,
        parent: null,
        kind: "planet",
        designation: "TEST GIANT",
        radiusM: PHASE_GIANT_RADIUS_M,
        hillRadiusM: null,
        centreM: add(
          PHASE_CAMERA_M,
          scale(phasePlanetDirection(PHASE_GIANT_PHASE_DEG), PHASE_GIANT_DISTANCE_M),
        ),
        rotation: null,
        symbol: bodyKindSymbol("planet"),
        orbitNormal: null,
      },
    ],
    rings: [],
    orbits: [],
    craft: [],
    stars: [],
    ownShip: null,
    defaultPose: cameraPose(),
    hostDiscs: [sunLikeHostDisc({ star: 0 })],
  };
}

/** The phase scene: a fixed camera on three planets at three phases. */
export function phaseScene(): KeptScene {
  return {
    name: PHASE_SCENE_NAME,
    durationS: PHASE_DURATION_S,
    sceneAt: phaseSceneAt,
    cameraAt: cameraPose,
  };
}

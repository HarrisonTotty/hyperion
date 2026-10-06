/**
 * The kept eclipse scene (plan R07, T10.c): a moon's shadow crossing an Earth-sized planet under
 * the camera, the moon crossing the star as the camera sees it from inside the moon's penumbra,
 * and a hot giant passing behind the star.
 *
 * @remarks
 * A Sun-like star at the barycentre, at rest, lights an Earth-sized planet on a circular orbit at
 * 1 au, a Moon-sized moon on a circular orbit 363,300 km about the planet (the Moon's perigee
 * distance, so that its umbra reaches the planet: a total eclipse), and a Jupiter-sized giant on
 * a circular orbit 0.05 au from the star, a hot Jupiter's (P 4.08 d). All three orbits lie in the
 * system's x–y plane, prograde, each at its two-body rate √(G(M + m) ÷ r³). The scene's clock runs
 * at {@link ECLIPSE_TIME_RATE} times the script's.
 *
 * The scene has no own ship: the camera is free, held in the planet's frame 20,000 km above the
 * point beneath the star at mid-eclipse ({@link ECLIPSE_CAMERA_OFFSET_M}), looking down at it. The
 * moon passes between the star and the planet at {@link ECLIPSE_MID_S}: its shadow crosses the
 * planet's disc from the view's left to its right at 0.985 km/s in the planet's frame, the umbra
 * 155 km and the penumbra 6,811 km across the shadow's axis, and the shadow's cone crosses the
 * camera, so that a camera turned to the star (the star as its target) sees the moon cross the
 * star's disc, totally for 345 s. At {@link ECLIPSE_CONJUNCTION_S} the giant passes behind the
 * star's centre, right to left, its disc wholly behind the star's for 10,000 s. The script starts
 * and ends with every body clear.
 *
 * Every body but the star moves, with its velocity on its retarded centre and no light time to the
 * camera (a kept scene's), so that lighting takes each source where the light reaching the lit
 * body left it (T10.a): the moon's shadow falls 34.8 km from where the moon's place at the scene's
 * time would cast it, the distance the moon moves in its 1.21 s light time to the planet. The
 * planet does not turn, and its reflex about the planet–moon barycentre (4,414 km) is left out.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { add, scale, vec3, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { SUN_RADIUS_M, sunLikeHostDisc } from "../lighting/hostDisc";
import {
  bodyKindSymbol,
  type RetardedCentre,
  staticRetarded,
  type ViewBody,
  type ViewBodyKind,
  type ViewScene,
} from "../scene/model";
import {
  AU_M,
  KEPT_BARYCENTRE,
  KEPT_SYSTEM,
  KEPT_TIDAL_RADIUS_M,
  type KeptScene,
  keptBody,
  keptTime,
} from "./kept";

/** The eclipse scene's name, as the `SCENE` selector shows it. */
export const ECLIPSE_SCENE_NAME = "ECLIPSE TEST";

/** The scene's star, body index 0, the host disc's `star`. */
export const ECLIPSE_STAR = keptBody(0);

/** The planet the moon's shadow crosses, body index 1. */
export const ECLIPSE_PLANET = keptBody(1);

/** The moon, body index 2. */
export const ECLIPSE_MOON = keptBody(2);

/** The giant that passes behind the star, body index 3. */
export const ECLIPSE_GIANT = keptBody(3);

/** The scene's seconds per second of the script: the clock runs a hundred times fast. */
export const ECLIPSE_TIME_RATE = 100;

/** How long the script runs before it starts again, s of the script (27,000 s of the scene's). */
export const ECLIPSE_DURATION_S = 270;

/** When the moon stands on the line from the star to the planet's centre, s of the scene's clock. */
export const ECLIPSE_MID_S = 10_100;

/**
 * When the giant stands on the line from the planet's centre through the star, beyond it, s of the
 * scene's clock.
 */
export const ECLIPSE_CONJUNCTION_S = 20_500;

/** The planet's radius, m: Earth's volumetric mean radius, 6,371.000 km (NASA Earth Fact Sheet). */
export const ECLIPSE_PLANET_RADIUS_M = 6.371e6;

/** The moon's radius, m: the Moon's mean radius, 1,737.4 km (Archinal et al. 2018, IAU WGCCRE). */
export const ECLIPSE_MOON_RADIUS_M = 1.7374e6;

/** The giant's radius, m: Jupiter's volumetric mean radius, 69,911 km (NASA Jupiter Fact Sheet). */
export const ECLIPSE_GIANT_RADIUS_M = 6.9911e7;

/** The moon's orbital radius about the planet, m: the Moon's perigee, 363,300 km (NASA Moon Fact Sheet). */
export const ECLIPSE_MOON_ORBIT_M = 3.633e8;

/** The giant's orbital radius about the star, m: 0.05 au. */
export const ECLIPSE_GIANT_ORBIT_M = 0.05 * AU_M;

/** The camera's height above the point beneath the star at mid-eclipse, m. */
const CAMERA_HEIGHT_M = 2e7;

/** The Sun's GM, m³/s²: IAU 2015 Resolution B3's nominal (GM)ᴺ☉ (Prša et al. 2016, AJ 152, 41). */
const GM_SUN_M3_S2 = 1.327_124_4e20;

/** Earth's GM, m³/s²: IAU 2015 Resolution B3's nominal (GM)ᴺ_E. */
const GM_EARTH_M3_S2 = 3.986_004e14;

/** The Moon's GM, m³/s²: 4,902.800066 km³/s² (DE430; Folkner et al. 2014, IPN PR 42-196). */
const GM_MOON_M3_S2 = 4.902_800_066e12;

/** Jupiter's GM, m³/s²: IAU 2015 Resolution B3's nominal (GM)ᴺ_J. */
const GM_JUPITER_M3_S2 = 1.266_865_3e17;

/** A circular orbit: its radius and its angular rate, rad/s. */
interface CircularOrbit {
  readonly radiusM: number;
  readonly rateRadPerS: number;
}

/** A circular orbit of radius `radiusM` at the two-body rate √(G(M + m) ÷ r³). */
function circular(radiusM: number, gmSumM3S2: number): CircularOrbit {
  return { radiusM, rateRadPerS: Math.sqrt(gmSumM3S2 / radiusM ** 3) };
}

const PLANET_ORBIT = circular(AU_M, GM_SUN_M3_S2 + GM_EARTH_M3_S2);
const MOON_ORBIT = circular(ECLIPSE_MOON_ORBIT_M, GM_EARTH_M3_S2 + GM_MOON_M3_S2);
const GIANT_ORBIT = circular(ECLIPSE_GIANT_ORBIT_M, GM_SUN_M3_S2 + GM_JUPITER_M3_S2);

/** The planet's angle about the star at scene time `sceneS`, rad: +x at 0. */
function planetAngleRad(sceneS: number): number {
  return PLANET_ORBIT.rateRadPerS * sceneS;
}

/** The moon's angle about the planet, rad: towards the star at {@link ECLIPSE_MID_S}. */
function moonAngleRad(sceneS: number): number {
  return (
    planetAngleRad(ECLIPSE_MID_S) + Math.PI + MOON_ORBIT.rateRadPerS * (sceneS - ECLIPSE_MID_S)
  );
}

/** The giant's angle about the star, rad: opposite the planet at {@link ECLIPSE_CONJUNCTION_S}. */
function giantAngleRad(sceneS: number): number {
  return (
    planetAngleRad(ECLIPSE_CONJUNCTION_S) +
    Math.PI +
    GIANT_ORBIT.rateRadPerS * (sceneS - ECLIPSE_CONJUNCTION_S)
  );
}

/** Where a body is and how fast it moves, against some centre along the galactic axes. */
export interface EclipsePlace {
  /** Its centre, m from the reference. */
  readonly centreM: Vec3;
  /** Its velocity against the reference, m/s. */
  readonly velocityMPerS: Vec3;
}

/** The place on a circular orbit in the x–y plane at `angleRad`, against the orbit's centre. */
function onOrbit(orbit: CircularOrbit, angleRad: number): EclipsePlace {
  const c = Math.cos(angleRad);
  const s = Math.sin(angleRad);
  const speedMPerS = orbit.radiusM * orbit.rateRadPerS;
  return {
    centreM: vec3(orbit.radiusM * c, orbit.radiusM * s, 0),
    velocityMPerS: vec3(-speedMPerS * s, speedMPerS * c, 0),
  };
}

/**
 * Where a body of the scene is at scene time `sceneS` and how fast it moves, against the
 * barycentre: the scene's exact tracks, which the tests' oracles evaluate at their own times.
 *
 * @throws Error for a body the scene does not have.
 */
export function eclipsePlaceAt(body: BodyIdHex, sceneS: number): EclipsePlace {
  const planet = onOrbit(PLANET_ORBIT, planetAngleRad(sceneS));
  let place: EclipsePlace;
  switch (body) {
    case ECLIPSE_STAR:
      place = { centreM: vec3(0, 0, 0), velocityMPerS: vec3(0, 0, 0) };
      break;
    case ECLIPSE_PLANET:
      place = planet;
      break;
    case ECLIPSE_MOON: {
      const about = onOrbit(MOON_ORBIT, moonAngleRad(sceneS));
      place = {
        centreM: add(planet.centreM, about.centreM),
        velocityMPerS: add(planet.velocityMPerS, about.velocityMPerS),
      };
      break;
    }
    case ECLIPSE_GIANT:
      place = onOrbit(GIANT_ORBIT, giantAngleRad(sceneS));
      break;
    default:
      throw new Error(`the eclipse scene has no body ${body}`);
  }
  return place;
}

/**
 * The camera's place in the planet's frame, m along the galactic axes: 20,000 km above the point
 * beneath the star at {@link ECLIPSE_MID_S}, on the line from the planet's centre to the star.
 */
export const ECLIPSE_CAMERA_OFFSET_M: Vec3 = (() => {
  const angleRad = planetAngleRad(ECLIPSE_MID_S);
  const distanceM = -(ECLIPSE_PLANET_RADIUS_M + CAMERA_HEIGHT_M);
  return vec3(distanceM * Math.cos(angleRad), distanceM * Math.sin(angleRad), 0);
})();

/** The camera's pose: held in the planet's frame, looking down at the planet with +z up. */
export const ECLIPSE_CAMERA_POSE: CameraPose = {
  frame: { kind: "body", body: ECLIPSE_PLANET },
  positionM: ECLIPSE_CAMERA_OFFSET_M,
  orientation: lookAlong(scale(ECLIPSE_CAMERA_OFFSET_M, -1), vec3(0, 0, 1)),
};

/**
 * The camera at its place turned to the star at scene time `sceneS`, with +z up: what the free
 * camera shows with the star as its target, for the tests and the captures.
 */
export function eclipsePoseAtStar(sceneS: number): CameraPose {
  const cameraM = add(eclipsePlaceAt(ECLIPSE_PLANET, sceneS).centreM, ECLIPSE_CAMERA_OFFSET_M);
  return { ...ECLIPSE_CAMERA_POSE, orientation: lookAlong(scale(cameraM, -1), vec3(0, 0, 1)) };
}

/**
 * The Hill radius a (m ÷ 3M)^⅓, m, of a body of GM `gmM3S2` on an orbit of radius `orbitM` about
 * one of GM `gmCentralM3S2`: the distance of the restricted three-body problem's L₁ and L₂ for
 * m ≪ M (Murray and Dermott 1999, _Solar System Dynamics_, ch. 3), whose μ = m ÷ (M + m) moves it
 * by 0.4% at most here.
 */
function hillRadiusM(orbitM: number, gmM3S2: number, gmCentralM3S2: number): number {
  return orbitM * Math.cbrt(gmM3S2 / (3 * gmCentralM3S2));
}

/** A body of the scene where it is at scene time `sceneS`. */
function bodyAt(
  id: BodyIdHex,
  sceneS: number,
  kind: ViewBodyKind,
  designation: string,
  radiusM: number,
  parent: BodyIdHex | null,
  hillM: number | null,
): ViewBody {
  const { centreM, velocityMPerS } = eclipsePlaceAt(id, sceneS);
  // A kept scene's body has no light time to the camera, so its retarded centre is its drawn one;
  // its velocity places it, as a source, where the light reaching another body left it (T10.a).
  const retarded: RetardedCentre =
    kind === "star" ? staticRetarded(centreM) : { centreM, velocityMPerS, lightTimeS: 0 };
  return {
    id,
    parent,
    kind,
    designation,
    radiusM,
    hillRadiusM: hillM,
    centreM,
    retarded,
    rotation: null,
    symbol: bodyKindSymbol(kind),
    orbitNormal: kind === "star" ? null : vec3(0, 0, 1),
  };
}

/** The scene at scene time `sceneS`, s since the script's start ({@link ECLIPSE_TIME_RATE} × its s). */
export function eclipseSceneAt(sceneS: number): ViewScene {
  return {
    provenance: { kind: "kept", name: ECLIPSE_SCENE_NAME },
    time: keptTime(sceneS),
    timeRate: ECLIPSE_TIME_RATE,
    system: KEPT_SYSTEM,
    barycentre: KEPT_BARYCENTRE,
    tidalRadiusM: KEPT_TIDAL_RADIUS_M,
    bodies: [
      bodyAt(ECLIPSE_STAR, sceneS, "star", "TEST STAR", SUN_RADIUS_M, null, null),
      bodyAt(
        ECLIPSE_PLANET,
        sceneS,
        "planet",
        "TEST PLANET",
        ECLIPSE_PLANET_RADIUS_M,
        ECLIPSE_STAR,
        hillRadiusM(AU_M, GM_EARTH_M3_S2, GM_SUN_M3_S2),
      ),
      bodyAt(
        ECLIPSE_MOON,
        sceneS,
        "moon",
        "TEST MOON",
        ECLIPSE_MOON_RADIUS_M,
        ECLIPSE_PLANET,
        hillRadiusM(ECLIPSE_MOON_ORBIT_M, GM_MOON_M3_S2, GM_EARTH_M3_S2),
      ),
      bodyAt(
        ECLIPSE_GIANT,
        sceneS,
        "planet",
        "TEST GIANT",
        ECLIPSE_GIANT_RADIUS_M,
        ECLIPSE_STAR,
        hillRadiusM(ECLIPSE_GIANT_ORBIT_M, GM_JUPITER_M3_S2, GM_SUN_M3_S2),
      ),
    ],
    rings: [],
    orbits: [],
    craft: [],
    stars: [],
    ownShip: null,
    defaultPose: ECLIPSE_CAMERA_POSE,
    hostDiscs: [sunLikeHostDisc({ star: 0 })],
  };
}

/**
 * The eclipse scene: a free camera above a planet the moon's shadow crosses, inside the moon's
 * penumbra, and a giant passing behind the star.
 */
export function eclipseScene(): KeptScene {
  return {
    name: ECLIPSE_SCENE_NAME,
    durationS: ECLIPSE_DURATION_S,
    sceneAt: (tS) => eclipseSceneAt(ECLIPSE_TIME_RATE * tS),
    cameraAt: () => ECLIPSE_CAMERA_POSE,
  };
}

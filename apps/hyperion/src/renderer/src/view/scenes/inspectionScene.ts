/**
 * `VIEW`'s body inspection (the owner's request, 2026-11-01): a kept scene of one served body,
 * opened from the `SYSTEM` display's orbit map once `FOCUS BODY` has focused it, drawn
 * photorealistically under the training banner, since it is no live operation.
 *
 * @remarks
 * - **Its bodies are the server's records**, the ones the `SYSTEM` display holds (`system_summary`
 *   and `system_bodies` at the display's time), drawn by the server scene's own path
 *   (`serverSceneAtPush` through `fromServer.ts`): every star and every present body on its orbit,
 *   each with its figure, photometry and rotation at its grant, and the planets' rings. The scene
 *   is seen from the inspected body's centre, at rest in its frame, so that the body is the scene's
 *   local body and is drawn where it is. Nothing is loaded here: the scene marks its bodies as
 *   served ({@link ViewScene.served}), so that `VIEW` asks their atmospheres' records and their
 *   detail seeds of the server as it does a server scene's, and lights the scene with its sky's host
 *   discs, asked from the camera's place in the system.
 * - **Its clock** runs at one second a second from the display's time when the inspection was
 *   opened, so that the body turns as it does; its script is a month long and no inspection plays
 *   it to its end.
 * - **Its camera** has no own ship to sit in: it starts free, in the body's frame, on the body's
 *   day side ({@link INSPECTION_ROUND_DEG} round from its star about its pole and
 *   {@link INSPECTION_ELEVATION_DEG} out of its equator on the star's side, so that a ring plane
 *   opens on its lit face), looking at its centre with its pole up, at the distance at which its
 *   disc spans {@link INSPECTION_DISC_SHARE} of the view's smaller side at the default field of
 *   view: framed for the view's own aspect at its first frame with a size
 *   (`KeptScene.framedPoseFor`), and for a 16:9 view until then.
 * - **Its exposure** opens at `AUTO` at its first metered value at once
 *   (`KeptScene.startExposure`), so that the body is exposed for from its first metered frame, on
 *   the `INCIDENT` meter (`KeptScene.startMeter`; ux-calls-view, item 3b), the light falling on
 *   the body, which draws its albedo as it is.
 * - **Its tidal radius**, which bounds only a camera flown out of the system, is the kept scenes'
 *   (`KEPT_TIDAL_RADIUS_M`): the server states a system's in its scene alone (a lean of this
 *   change, 2026-11-01).
 */

import type { BodyIdHex, DetailLevelDto, UniverseIdHex, UniverseTime } from "@hyperion/protocol";

import { add, cross, dot, normalise, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { timeAfter } from "../../lib/scene/lightTime";
import type { BodyGrant, SceneParts, SceneSystem, SystemPlace } from "../../lib/scene/model";
import { EARTH_MASS_KG } from "../../lib/system/bodiesWire";
import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../lib/system/constants";
import type { SystemBodies, SystemBody, SystemModel } from "../../lib/system/model";
import type { CameraPose } from "../camera/pose";
import { DEFAULT_FOV_DEG } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { serverSceneAtPush } from "../scene/fromServer";
import type { ServedSceneSource, ViewBody, ViewScene } from "../scene/model";
import { hostStar } from "../scene/servedBody";
import { KEPT_TIDAL_RADIUS_M, type KeptScene } from "./kept";

/** The inspection's name, as the `SCENE` selector and the label block's `SCENE` line show it. */
export const INSPECTION_SCENE_NAME = "BODY INSPECTION";

/** How long the inspection's script runs, s: 30 days at one second a second. */
export const INSPECTION_DURATION_S = 30 * 86_400;

/** The share of the view's smaller side the body's disc fills as the inspection opens. */
export const INSPECTION_DISC_SHARE = 2 / 3;

/** The aspect a view is framed for before its size is known, width over height: 16:9. */
export const INSPECTION_ASPECT = 16 / 9;

/**
 * How far round from its star the camera stands about the body's pole, degrees: with
 * {@link INSPECTION_ELEVATION_DEG}, the body is seen gibbous at a phase angle from 37° to 70°,
 * whatever its star's declination (44° with the star on its equator), its terminator in the view.
 */
export const INSPECTION_ROUND_DEG = 40;

/**
 * How far out of the body's equator the camera stands, on its star's side, degrees, so that a ring
 * plane opens on its lit face.
 */
export const INSPECTION_ELEVATION_DEG = 20;

/**
 * What an inspection is made of: the system's records as the `SYSTEM` display holds them, the body
 * and the display's time.
 */
export interface BodyInspection {
  /** The universe the system is in: the open universe. */
  readonly universe: UniverseIdHex;
  /** The system's designation of record and its barycentre, as the `SYSTEM` display opened it. */
  readonly place: SystemPlace;
  /** Its stars and their hierarchy (`system_summary`'s, or `system_bodies`' own). */
  readonly model: SystemModel;
  /** Its bodies, each at the detail level the server granted (`system_bodies`'). */
  readonly bodies: SystemBodies;
  /** The body inspected: a planet, a dwarf planet or a moon, present and on its orbit. */
  readonly body: BodyIdHex;
  /** The display's time when the inspection was opened, at which its script starts. */
  readonly time: UniverseTime;
}

/** The scene's origin, and a body at rest. */
const ZERO: Vec3 = vec3(0, 0, 0);

/** Galactic north, which stands for a pole where neither the body's nor its orbit's is given. */
const GALACTIC_NORTH: Vec3 = vec3(0, 0, 1);

/** Degrees to radians. */
const RAD_PER_DEG = Math.PI / 180;

/**
 * A body's Hill radius at pericentre, m, where its record holds its mass and a bound orbit: plan
 * 14's `hill_radius`, a (1 − e) (m ÷ 3M)^⅓, as the scene's wire adapter takes it (`sceneWire.ts`'
 * `hillRadii`), with the primary's mass M = μ ÷ G − m and μ = 4π² a³ ÷ P² (Kepler's third law),
 * since the record carries the orbit's period, and m its Earth masses times
 * {@link EARTH_MASS_KG}; `null` without them.
 */
export function hillRadiusMOf(body: SystemBody): number | null {
  if (body.massMearth.state !== "ok" || body.orbit.state !== "ok") {
    return null;
  }
  const massKg = body.massMearth.value * EARTH_MASS_KG;
  const { semiMajorAxisM, eccentricity, periodS } = body.orbit.value.orbit;
  const muM3PerS2 = (4 * Math.PI * Math.PI * semiMajorAxisM ** 3) / (periodS * periodS);
  const primaryKg = muM3PerS2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 - massKg;
  const e = eccentricity;
  return massKg > 0 && primaryKg > 0 && e >= 0 && e < 1
    ? semiMajorAxisM * (1 - e) * Math.cbrt(massKg / (3 * primaryKg))
    : null;
}

/**
 * The inspection's system as a scene holds one: its records, every body at the answer's grant,
 * placed by its elements (no body is a contact the server placed), and their Hill radii.
 */
function inspectionSystem(inspection: BodyInspection): SceneSystem {
  const { bodies } = inspection;
  const grant: BodyGrant = { level: bodies.granted, seen: null };
  const hillRadiiM = new Map<BodyIdHex, number>();
  for (const body of bodies.bodies) {
    const radiusM = hillRadiusMOf(body);
    if (radiusM !== null) {
      hillRadiiM.set(body.id, radiusM);
    }
  }
  return {
    model: inspection.model,
    bodies,
    grants: new Map(bodies.bodies.map((body) => [body.id, grant])),
    hillRadiiM,
    tidalRadiusM: KEPT_TIDAL_RADIUS_M,
    place: inspection.place,
  };
}

/** Each body's granted level, as the atmosphere cache asks its record by. */
function grantLevels(bodies: SystemBodies): ReadonlyMap<BodyIdHex, DetailLevelDto> {
  return new Map(bodies.bodies.map((body) => [body.id, bodies.granted]));
}

/**
 * The scene the server's path draws of `system` at `time`, seen from `body`'s centre at rest in its
 * frame, or `null` where it cannot be drawn: the body not placed.
 */
function drawnAt(
  system: SceneSystem,
  place: SystemPlace,
  body: BodyIdHex,
  opened: UniverseTime,
  time: UniverseTime,
): ViewScene | null {
  const parts: SceneParts = {
    clock: { time, rate: 1, state: "running" },
    ship: { position: { kind: "body", body, offsetM: ZERO }, velocityMPerS: ZERO, time: opened },
    system,
    craft: [],
  };
  return serverSceneAtPush(parts, place);
}

/**
 * The distance from a body's centre, m, at which its disc of `radiusM` spans
 * {@link INSPECTION_DISC_SHARE} s of a view's smaller side on screen at the default horizontal
 * field of view.
 *
 * @remarks
 * A sphere seen at its angular radius α = sin⁻¹(R ÷ d) projects, on the view's rectilinear
 * projection, to a disc of radius f tan α against the half-side f tan(½ FOV) across that side, so
 * its share is tan α ÷ tan(½ FOV). With x = s tan(½ FOV_x) ÷ max(aspect, 1), the smaller
 * half-side's tangent times s, tan α = x, and d = R ÷ sin(tan⁻¹ x) = R √(1 + x²) ÷ x.
 *
 * @param aspect - The view's width over its height.
 */
export function framedDistanceM(radiusM: number, aspect: number): number {
  const x =
    (INSPECTION_DISC_SHARE * Math.tan((DEFAULT_FOV_DEG * RAD_PER_DEG) / 2)) / Math.max(aspect, 1);
  return (radiusM * Math.hypot(1, x)) / x;
}

/** `v` turned by `angleRad` about the unit axis `axis` (Rodrigues' rotation formula). */
function turned(v: Vec3, axis: Vec3, angleRad: number): Vec3 {
  const cos = Math.cos(angleRad);
  return add(
    add(scale(v, cos), scale(cross(axis, v), Math.sin(angleRad))),
    scale(axis, dot(axis, v) * (1 - cos)),
  );
}

/** A unit vector across `pole`, for a star that stands on its line. */
function across(pole: Vec3): Vec3 {
  const trial = Math.abs(pole.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  return normalise(sub(trial, scale(pole, dot(trial, pole))));
}

/**
 * The camera's pose as the inspection opens on a view of `aspect`, width over height: in `body`'s
 * frame, on its day side, round from its star about its pole and out of its equator on the star's
 * side, at the framed distance, looking at its centre with its pole up. The pole is its figure's, else its orbit's
 * normal (Design note 14), else galactic north.
 */
export function framedPose(body: ViewBody, star: ViewBody, aspect: number): CameraPose {
  const pole = normalise(body.appearance?.figure?.pole ?? body.orbitNormal ?? GALACTIC_NORTH);
  const towardStar = sub(star.centreM, body.centreM);
  const level = sub(towardStar, scale(pole, dot(towardStar, pole)));
  const sunward = norm(level) > 0 ? normalise(level) : across(pole);
  const round = turned(sunward, pole, INSPECTION_ROUND_DEG * RAD_PER_DEG);
  // Out of the equator on the star's side, so that the phase stays on the day side however far
  // the star stands from the equator, and a ring is seen on its lit face.
  const side = dot(towardStar, pole) < 0 ? -1 : 1;
  const elevation = INSPECTION_ELEVATION_DEG * RAD_PER_DEG;
  const outward = normalise(
    add(scale(round, Math.cos(elevation)), scale(pole, side * Math.sin(elevation))),
  );
  return {
    frame: { kind: "body", body: body.id },
    positionM: scale(outward, framedDistanceM(body.radiusM, aspect)),
    orientation: lookAlong(scale(outward, -1), pole),
  };
}

/**
 * The inspection's kept scene ({@link BodyInspection}; see the module's remarks): its bodies the
 * server's records at its time, its camera framed on the body, photorealistic from its start.
 *
 * @throws Error when the inspected body is not drawn, which `SYSTEM` never opens: a body not in the
 *   records, not present, on no orbit or with no radius, or with no star to frame it against.
 */
export function inspectionScene(inspection: BodyInspection): KeptScene {
  const { body, place, time } = inspection;
  const system = inspectionSystem(inspection);
  const served: ServedSceneSource = {
    universe: inspection.universe,
    place,
    grants: grantLevels(inspection.bodies),
    seenFrom: body,
  };
  const record = inspection.bodies.bodies.find((each) => each.id === body);
  const first = drawnAt(system, place, body, time, time);
  const drawn = first?.bodies.find((each) => each.id === body);
  const star = first === null || record === undefined ? null : hostStar(first, system, record);
  if (drawn === undefined || !(drawn.radiusM > 0) || star === null) {
    throw new Error(`the inspected body ${body} is not drawn with a radius beside a star`);
  }
  const pose = framedPose(drawn, star, INSPECTION_ASPECT);
  const sceneAt = (tS: number): ViewScene => {
    const scene = drawnAt(system, place, body, time, timeAfter(time, tS));
    if (scene === null) {
      throw new Error(`the inspected body ${body} is not placed at ${String(tS)} s`);
    }
    // No own ship: the server path's stand-in stood at the body's centre, and the camera is free.
    return {
      ...scene,
      provenance: { kind: "kept", name: INSPECTION_SCENE_NAME },
      craft: [],
      ownShip: null,
      defaultPose: pose,
      served,
    };
  };
  return {
    name: INSPECTION_SCENE_NAME,
    durationS: INSPECTION_DURATION_S,
    sceneAt,
    cameraAt: () => pose,
    framedPoseFor: (aspect) => framedPose(drawn, star, aspect),
    startStyle: "photorealistic",
    // Exposed for the body from its first metered frame: the display's MAN default stands about
    // 14 stops over a sunlit disc (the owner's request, 2026-11-01).
    startExposure: "metered",
    // By the light falling on the body, which renders its albedo as it is, where AVG averages a
    // frame of mostly black sky (ux-calls-view, item 3b).
    startMeter: "incident",
  };
}

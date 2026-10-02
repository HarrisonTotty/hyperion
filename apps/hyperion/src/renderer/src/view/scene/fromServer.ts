/**
 * The server's scene as the view draws it (plan R02, R02.T17): R03's client scene at one frame
 * turned into an engine-agnostic {@link ViewScene}.
 *
 * @remarks
 * The positions are `sceneAt`'s (rendering plan R03, R03.T13): the ship's local body is drawn where
 * it is at the frame's time (`geometricM`), and every other body and star where the ship sees it
 * (`apparentM`), a free camera's own local body included (R03's design note 7); a contact stands at
 * the server's seen position. The scene's messages carry neither the system's designation nor its
 * galactic position, so both come from a {@link SystemPlace} the client already holds; the
 * designations are composed here, at every frame, so that one learnt after the arrival relabels the
 * bodies at once.
 */
import {
  type BodyIdHex,
  formatBodyId,
  type GalacticPosition,
  type SystemIdHex,
  type UniverseTime,
} from "@hyperion/protocol";

import { add, norm, scale, vec3, type Vec3 } from "../../geometry/vec3";
import { KM_PER_RSUN } from "../../lib/format";
import { type SceneFrame, sceneAt, shipObserver, systemPlacements } from "../../lib/scene/apparent";
import type { SceneCraft, SceneKinematics, SceneModel, ScenePosition } from "../../lib/scene/model";
import { bodySymbol } from "../../lib/system/bodySymbols";
import { orbitNormal } from "../../lib/system/hierarchy";
import type { SystemBody } from "../../lib/system/model";
import { bodyDesignation } from "../../lib/system/wire";
import type { CameraPose, CraftId } from "../camera/pose";
import { IDENTITY_QUATERNION, lookAlong } from "../camera/quaternion";
import { galacticTranslated, type ViewPosition } from "../coords/position";
import { frameOrigin } from "../coords/relative";
import { TEST_HULL } from "./hull";
import {
  bodyKindSymbol,
  type BodyMarkSymbol,
  type CraftPose,
  sceneOrigins,
  type ViewBody,
  type ViewBodyKind,
  type ViewCraft,
  type ViewOrbit,
  type ViewRing,
  type ViewScene,
} from "./model";

/** The own ship's craft in a server scene: R03's ship stand-in. */
export const SERVER_OWN_SHIP: CraftId = "ship";

/** The own ship's designation in the view while it is R03's stand-in, until sessions name it. */
export const SERVER_OWN_SHIP_DESIGNATION = "OWN SHIP";

/** Metres in the nominal solar radius (IAU 2015 Resolution B3), from `lib/format.ts`' kilometres. */
const SOLAR_RADIUS_M = KM_PER_RSUN * 1_000;

/** Galactic north, which stands for the own ship's dorsal side while its attitude is a guess. */
const GALACTIC_NORTH = vec3(0, 0, 1);

/**
 * What the client knows of a scene's system that the scene's messages do not carry: its
 * designation of record, which every body's extends, and its barycentre in the galactic frame.
 */
export interface SystemPlace {
  readonly system: SystemIdHex;
  /** The designation, or the system's ID where none is known yet. */
  readonly designation: string;
  /** The barycentre, or `null` where the client has not been told where the system is. */
  readonly barycentre: GalacticPosition | null;
}

/** Why the server's scene cannot be drawn now, or `null` when it can. */
export type ServerSceneGap = "no_system" | "no_tidal_radius";

/**
 * Whether a scene model can be drawn: it must hold a system, with its tidal radius, and the ship
 * stand-in in it.
 */
export function serverSceneGap(model: SceneModel): ServerSceneGap | null {
  if (model.system === null || shipObserver(model, model.clock.time) === null) {
    return "no_system";
  }
  return model.system.tidalRadiusM === null ? "no_tidal_radius" : null;
}

/**
 * The view's scene at the time the model's latest push states, with no frame before it: a pure
 * first frame for a view starting on the server's scene, before its drawing loop reads
 * `useScene`'s `frameAt`; `null` while the scene cannot be drawn.
 */
export function serverSceneAtPush(model: SceneModel, place: SystemPlace): ViewScene | null {
  const time = model.clock.time;
  const observer = shipObserver(model, time);
  const frame = observer === null ? null : sceneAt(model, observer, time, null);
  return frame === null ? null : viewSceneFromServer(model, frame, place);
}

/** What a view drawing the server's scene reads of `useScene` at each frame. */
export interface ServerSceneSource {
  /** The scene as the latest render holds it. */
  readonly model: SceneModel;
  readonly place: SystemPlace;
  /** `useScene`'s `frameAt`: the scene as the ship sees it at a frame's `performance.now()`. */
  readonly frameAt: (nowMs: number) => SceneFrame | null;
}

/**
 * The view's scene at a drawing frame's `performance.now()`: `useScene`'s `frameAt(nowMs)` turned
 * into a {@link ViewScene}, or `null` while it cannot be drawn.
 *
 * @remarks
 * The frame comes from the store's latest push, the model from the latest render. Between a push
 * into another system and the render that follows it the two disagree (the frame's stars are not
 * the model's hosts), and no scene is given, so that the run holds its last rather than mixing two
 * systems.
 */
export function serverSceneAtFrame(source: ServerSceneSource, nowMs: number): ViewScene | null {
  const frame = source.frameAt(nowMs);
  const system = source.model.system;
  if (frame === null || system === null) {
    return null;
  }
  const hosts = new Set<string>(system.model.hosts.map((host) => host.id));
  if (!frame.stars.every((star) => hosts.has(star.id))) {
    return null;
  }
  return viewSceneFromServer(source.model, frame, source.place);
}

/** The view's kind of a body of plan 14's, or `null` for a population, which is not a point. */
function viewKind(body: SystemBody): ViewBodyKind | null {
  let kind: ViewBodyKind | null;
  switch (body.kind.kind) {
    case "planet":
    case "dwarf_planet":
      kind = "planet";
      break;
    case "moon":
      kind = "moon";
      break;
    case "unresolved":
      kind = "unresolved";
      break;
    case "ring":
    case "belt":
    case "cometary_halo":
    case "protoplanetary_disc":
    case "debris_disc":
      kind = null;
      break;
  }
  return kind;
}

/** A body's own symbol from the ship-wide set: a giant's larger triangle, a dwarf's smaller one. */
function symbolOf(body: SystemBody, kind: ViewBodyKind): BodyMarkSymbol {
  return bodySymbol(body) ?? bodyKindSymbol(kind);
}

/** A body's parent as the view names it: its body, its star, or `null` for a pair or barycentre. */
function parentOf(
  body: SystemBody,
  system: SystemIdHex,
  records: ReadonlyMap<BodyIdHex, SystemBody>,
): BodyIdHex | null {
  let parent = body.parent;
  // A belt's member orbits what its belt orbits, as the `SYSTEM` display places it; the walk is
  // bounded by the bodies, since `toSystemBodiesModel` accepted a chain that ends at a host.
  for (let steps = 0; steps <= records.size; steps += 1) {
    if (parent === null) {
      return null;
    }
    switch (parent.kind) {
      case "body": {
        const record = records.get(parent.id);
        if (record === undefined || viewKind(record) !== null) {
          return parent.id;
        }
        parent = record.parent;
        break;
      }
      case "star":
        return formatBodyId({ system, bodyIndex: parent.bodyIndex });
      case "pair":
      case "barycentre":
        return null;
    }
  }
  return null;
}

/** A scene position as the view's {@link ViewPosition}, carried `seconds` ahead at `velocity`. */
function coasted(position: ScenePosition, velocity: Vec3, seconds: number): ViewPosition {
  const delta = scale(velocity, seconds);
  let result: ViewPosition;
  switch (position.kind) {
    case "galactic":
      // A craft in the galactic frame is not in the scene's system; it is not drawn (below).
      result = { kind: "galactic", position: position.position };
      break;
    case "system":
      result = { kind: "system", system: position.system, m: add(position.offsetM, delta) };
      break;
    case "body":
      result = { kind: "body", body: position.body, m: add(position.offsetM, delta) };
      break;
  }
  return result;
}

/** Scene seconds from `from` to `to`. */
function secondsFrom(from: UniverseTime, to: UniverseTime): number {
  return to.seconds - from.seconds + (to.nanos - from.nanos) / 1e9;
}

/** A pose of a craft's planned path as the view draws it, with the craft's attitude. */
function pathPose(pose: SceneKinematics, craft: SceneCraft): CraftPose {
  return { position: coasted(pose.position, pose.velocityMPerS, 0), attitude: craft.attitude };
}

/**
 * A contact craft at the frame's time, coasted from its pushed pose at its velocity (the same
 * straight line R03's `predictedPath` draws for a craft without a plan); its hull is `TEST_HULL`
 * while no outline is known (Design note 15). Only a planned path is drawn as its predicted path:
 * the straight line is not a flight computer's prediction.
 */
function viewCraft(craft: SceneCraft, time: UniverseTime): ViewCraft {
  const elapsedS = secondsFrom(craft.state.time, time);
  return {
    id: craft.craft,
    designation: craft.craft,
    hull: TEST_HULL,
    pose: {
      position: coasted(craft.state.position, craft.state.velocityMPerS, elapsedS),
      attitude: craft.attitude,
    },
    predictedPath:
      craft.plannedPath === null ? null : craft.plannedPath.map((pose) => pathPose(pose, craft)),
    velocityMPerS: craft.state.velocityMPerS,
  };
}

/** Whether a craft's position can be placed in the scene's system. */
function inScene(
  position: ViewPosition,
  system: SystemIdHex,
  bodies: ReadonlySet<string>,
): boolean {
  let placed: boolean;
  switch (position.kind) {
    case "galactic":
      placed = false;
      break;
    case "system":
      placed = position.system === system;
      break;
    case "body":
    case "body_fixed":
      placed = bodies.has(position.body);
      break;
  }
  return placed;
}

/**
 * The own ship's attitude while it is R03's stand-in, which has none: its nose along its velocity
 * in the system frame, its dorsal side towards galactic north, or the galactic axes at rest. A
 * choice of this task, until the flight model gives the ship an attitude.
 */
function standInAttitude(velocityMPerS: Vec3): CraftPose["attitude"] {
  return norm(velocityMPerS) > 0 ? lookAlong(velocityMPerS, GALACTIC_NORTH) : IDENTITY_QUATERNION;
}

/**
 * The view's scene from the server's scene at one frame (R02.T17), or `null` while it cannot be
 * drawn ({@link serverSceneGap}).
 *
 * @remarks
 * Bodies come from the frame, each from its record at its own grant: a radius only from a `bulk`
 * section (a body without one is drawn as its symbol at any range), a Hill radius only where
 * `sceneAt` gives one, so that a body below `mass_and_orbit` or placed by `seen` is never the
 * camera's frame. The ship's local body is drawn at its `geometricM`, every other body and star at
 * its `apparentM`. A body's rotation is not modelled yet (Design note 14): its pole is its orbit's
 * normal. Rings are drawn about their planet, in its orbital plane (plan 14's convention for this
 * generator version); orbits are every placed planet's and moon's about the body or star it
 * orbits, or about the barycentre for the root, and one about a pair below the root is not drawn.
 * The ship stand-in is the own ship, in the system frame at the observer's present position.
 */
export function viewSceneFromServer(
  model: SceneModel,
  frame: SceneFrame,
  place: SystemPlace,
): ViewScene | null {
  const system = model.system;
  if (system === null || system.tidalRadiusM === null) {
    return null;
  }
  const systemId = system.model.system;
  const records = new Map(system.bodies.bodies.map((body) => [body.id, body]));
  const placements = systemPlacements(system);

  const bodies: ViewBody[] = [];
  for (const host of system.model.hosts) {
    const seen = frame.stars.find((star) => star.id === host.id);
    if (seen === undefined) {
      continue;
    }
    bodies.push({
      id: host.id,
      parent: null,
      kind: "star",
      designation: bodyDesignation(place.designation, host.bodyIndex),
      radiusM: host.radiusRsun * SOLAR_RADIUS_M,
      hillRadiusM: null,
      centreM: seen.apparentM,
      rotation: null,
      orbitNormal: null,
      symbol: bodyKindSymbol("star"),
    });
  }
  for (const seen of frame.bodies) {
    const record = records.get(seen.id);
    const kind = record === undefined ? null : viewKind(record);
    if (record === undefined || kind === null) {
      continue;
    }
    const placed = seen.kind === "placed";
    bodies.push({
      id: seen.id,
      parent: parentOf(record, systemId, records),
      kind,
      designation: bodyDesignation(place.designation, record.bodyIndex),
      radiusM: record.bulk.state === "ok" ? record.bulk.value.radiusM : 0,
      hillRadiusM: placed ? seen.hillRadiusM : null,
      centreM: placed && seen.id === frame.localBody ? seen.geometricM : seen.apparentM,
      rotation: null,
      orbitNormal: record.orbit.state === "ok" ? orbitNormal(record.orbit.value.orbit) : null,
      symbol: symbolOf(record, kind),
    });
  }
  const drawn = new Set<string>(bodies.map((body) => body.id));
  const byId = new Map(bodies.map((body) => [body.id, body]));

  const rings: ViewRing[] = [];
  for (const record of system.bodies.bodies) {
    if (
      record.population.state !== "ok" ||
      record.population.value.kind !== "ring" ||
      record.state.kind !== "present" ||
      record.parent?.kind !== "body"
    ) {
      continue;
    }
    const planet = byId.get(record.parent.id);
    if (planet === undefined) {
      continue;
    }
    rings.push({
      body: planet.id,
      innerRadiusM: record.population.value.innerEdgeM,
      outerRadiusM: record.population.value.outerEdgeM,
      normal: planet.orbitNormal ?? GALACTIC_NORTH,
    });
  }

  const orbits: ViewOrbit[] = [];
  for (const body of bodies) {
    const placement = placements.placements.get(body.id);
    if (body.kind === "star" || placement?.kind !== "orbit") {
      continue;
    }
    const focus = placements.placements.get(placement.parentId);
    if (drawn.has(placement.parentId)) {
      orbits.push({ body: body.id, parent: placement.parentId, orbit: placement.orbit });
    } else if (focus?.kind === "origin") {
      orbits.push({ body: body.id, parent: null, orbit: placement.orbit });
    }
  }

  const ownShip: ViewCraft = {
    id: SERVER_OWN_SHIP,
    designation: SERVER_OWN_SHIP_DESIGNATION,
    hull: TEST_HULL,
    pose: {
      position: { kind: "system", system: systemId, m: frame.observer.positionM },
      attitude: standInAttitude(frame.observer.velocityMPerS),
    },
    predictedPath: null,
    velocityMPerS: frame.observer.velocityMPerS,
  };
  const craft = [
    ownShip,
    ...model.craft
      .filter((each) => each.craft !== SERVER_OWN_SHIP)
      .map((each) => viewCraft(each, frame.time))
      .filter((each) => inScene(each.pose.position, systemId, drawn)),
  ];

  const running = model.clock.state === "running";
  return {
    provenance: { kind: "server" },
    time: frame.time,
    timeRate: running ? model.clock.rate : 0,
    system: systemId,
    barycentre: place.barycentre,
    tidalRadiusM: system.tidalRadiusM,
    bodies,
    rings,
    orbits,
    craft,
    stars: [],
    ownShip: SERVER_OWN_SHIP,
    defaultPose: {
      frame: { kind: "system", system: systemId },
      positionM: frame.observer.positionM,
      orientation: IDENTITY_QUATERNION,
    },
  };
}

/** A view's position as the scene's camera report states it. */
function scenePositionOf(position: ViewPosition): ScenePosition {
  let result: ScenePosition;
  switch (position.kind) {
    case "galactic":
      result = { kind: "galactic", position: position.position };
      break;
    case "system":
      result = { kind: "system", system: position.system, offsetM: position.m };
      break;
    case "body":
    case "body_fixed":
      // No scene body has a rotation yet, so its fixed axes are its frame's (Design note 14).
      result = { kind: "body", body: position.body, offsetM: position.m };
      break;
  }
  return result;
}

/**
 * A view's camera pose as R03's camera report states it (`SceneView.reportCamera`): in the system
 * or body frame the camera is held in, a camera held about a craft in the frame of the craft's own
 * position, at the scene's time.
 *
 * @remarks
 * The view does not track its camera's velocity, so the report states it at rest in its frame. A
 * body frame's origin is the body as the view draws it, which for a body other than the ship's
 * local one is its apparent position, a light time's travel from where the server puts it; the
 * difference is far below the scene's reach, which is all the server checks.
 */
export function cameraKinematics(pose: CameraPose, scene: ViewScene): SceneKinematics {
  const origins = sceneOrigins(scene);
  const origin = frameOrigin(pose.frame, origins);
  const position: ViewPosition =
    origin.kind === "galactic"
      ? { kind: "galactic", position: galacticTranslated(origin.position, pose.positionM) }
      : { ...origin, m: add(origin.m, pose.positionM) };
  return { position: scenePositionOf(position), velocityMPerS: vec3(0, 0, 0), time: scene.time };
}

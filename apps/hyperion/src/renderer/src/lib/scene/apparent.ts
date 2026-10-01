/**
 * Where the ship sees each body and star of its system: apparent positions, light time and
 * aberration together, computed on the client every frame from the elements (rendering plan R03,
 * Design note 7).
 *
 * @remarks
 * {@link apparentPosition} mirrors the simulation's `observe::retarded_in_system` step for step,
 * so that the client agrees with its golden vectors: the light time by the same fixed point, each
 * value rounded to the nanosecond as the simulation rounds it (`lightTime.ts`), the emitted time a
 * `UniverseTime` less a span, the same stopping rules and cap, and the same Lorentz form for the
 * apparent point with its operations grouped as the simulation's `aberrated` documents. Norms are a
 * sum of squares and a square root, never `Math.hypot`, which is not correctly rounded. The
 * positions themselves come from `lib/orbit.ts`, which agrees with the simulation's to its own
 * bound, not bit for bit.
 *
 * {@link sceneAt} applies it to a whole scene: every present body with an orbit, every star, and
 * every contact the server placed by its seen position; and it names the ship's local body by
 * R02's `selectCameraFrame`, the one rule for which body is local, so that the scene and the view
 * never name different ones. That body is the one R02 draws geometrically at the present.
 */
import {
  type BodyIdHex,
  type DetailLevelDto,
  formatBodyId,
  type UniverseTime,
} from "@hyperion/protocol";

import { add, type Vec3 } from "../../geometry/vec3";
import {
  type CameraFrameCandidate,
  cameraFrameCandidate,
  selectCameraFrame,
} from "../../view/camera/frames";
import { layoutBodies } from "../../displays/system/bodyMap";
import { type BodyPlacement, composePosition, stateAt } from "../orbit";
import { layoutHierarchy } from "../system/hierarchy";
import type { OrbitHost, SystemBody } from "../system/model";
import {
  compareSpans,
  lightTime,
  secondsBetween,
  type Span,
  SPEED_OF_LIGHT_M_PER_S,
  spanDistance,
  spanSeconds,
  timeBefore,
  ZERO_SPAN,
} from "./lightTime";
import type { SceneModel, SceneSystem } from "./model";

/**
 * The change in the light time at or below which the iteration has converged: 1 ns, inclusive,
 * the simulation's `IN_SYSTEM_LIGHT_TIME_TOLERANCE`.
 */
export const LIGHT_TIME_TOLERANCE: Span = { seconds: 0, nanos: 1 };

/** The most corrections the iteration makes: the simulation's `IN_SYSTEM_MAX_CORRECTIONS`. */
export const MAX_LIGHT_TIME_CORRECTIONS = 10;

/** A position in a system's frame that is a function of time: the simulation's `SystemTrajectory`. */
export interface SystemTrack {
  /** Where the source is at `time`, m from the barycentre along the galactic axes; `null` if absent. */
  positionAt(time: UniverseTime): Vec3 | null;
}

/** The observer: the ship, at the time it observes. */
export interface SceneObserver {
  /** Its position, m from the system's barycentre along the galactic axes. */
  readonly positionM: Vec3;
  /** Its velocity relative to the barycentre, m/s, slower than light. */
  readonly velocityMPerS: Vec3;
}

/** What the observer sees of a source: the simulation's `InSystemRetardation`. */
export interface SeenSource {
  readonly kind: "seen";
  /** When the light seen left the source. */
  readonly emitted: UniverseTime;
  /** The light time, to the nanosecond. */
  readonly lightTime: Span;
  /** The corrections made, the confirming one counted. */
  readonly corrections: number;
  /** The last correction's change in the light time. */
  readonly residual: Span;
  /** Where the source was at the emitted time, m. */
  readonly geometricThenM: Vec3;
  /** Where the observer sees it, light time and aberration together, m. */
  readonly apparentM: Vec3;
}

/** What the observer sees of a source, or why it sees nothing. */
export type ApparentResult =
  | SeenSource
  /** The source is absent at a time the iteration needed. */
  | { readonly kind: "not_present_then"; readonly emitted: UniverseTime }
  /** The light time still shrank after the cap: a source near the speed of light. */
  | { readonly kind: "not_converged"; readonly corrections: number; readonly lastChange: Span };

function dot(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** |`to` − `from`|, as a sum of squares and a correctly rounded square root. */
function distanceM(from: Vec3, to: Vec3): number {
  const d = { x: to.x - from.x, y: to.y - from.y, z: to.z - from.z };
  return Math.sqrt(dot(d, d));
}

/**
 * The separation `r` of an emission event a light time `tau` before reception, boosted into the
 * rest frame of an observer moving at `velocity` (Jackson 1999, eq. 11.19): the simulation's
 * `aberrated`, grouped as it is, (r + v (γ τ)) + v (κ (r · v)) with κ = γ γ ÷ (c² (γ + 1)).
 */
function aberrated(r: Vec3, velocity: Vec3, tau: Span): Vec3 {
  const c2 = SPEED_OF_LIGHT_M_PER_S * SPEED_OF_LIGHT_M_PER_S;
  const beta2 = dot(velocity, velocity) / c2;
  const gamma = 1 / Math.sqrt(1 - beta2);
  const kappa = (gamma * gamma) / (c2 * (gamma + 1));
  const gammaTau = gamma * spanSeconds(tau);
  const along = kappa * dot(r, velocity);
  return {
    x: r.x + velocity.x * gammaTau + velocity.x * along,
    y: r.y + velocity.y * gammaTau + velocity.y * along,
    z: r.z + velocity.z * gammaTau + velocity.z * along,
  };
}

/**
 * What `observer` sees of `track` at `time`: the light time by a fixed point and the apparent point
 * by the exact special-relativistic aberration, the simulation's `retarded_in_system`.
 *
 * @remarks
 * The light time starts from the present distance, or from `previousTau` for a warm start (the
 * previous frame's, which brings it to about one correction), and each correction evaluates the
 * source again at the observer's time less the light time. It stops at the first change of at most
 * {@link LIGHT_TIME_TOLERANCE}, or from the second correction on at the first change no smaller
 * than the one before, which is rounding noise; past {@link MAX_LIGHT_TIME_CORRECTIONS} it gives
 * up. The apparent point is the emission event boosted into the observer's rest frame, which
 * points along the relativistically aberrated direction.
 *
 * @param time - The observer's present, at which `observer` is given.
 * @param previousTau - The light time found for the same source a frame before, or `null`.
 */
export function apparentPosition(
  track: SystemTrack,
  observer: SceneObserver,
  time: UniverseTime,
  previousTau: Span | null,
): ApparentResult {
  const from = observer.positionM;
  let tau: Span;
  if (previousTau === null) {
    const now = track.positionAt(time);
    if (now === null) {
      return { kind: "not_present_then", emitted: time };
    }
    tau = lightTime(distanceM(from, now));
  } else {
    tau = previousTau;
  }
  let previousChange: Span | null = null;
  for (let k = 1; k <= MAX_LIGHT_TIME_CORRECTIONS; k += 1) {
    const asked = timeBefore(time, tau);
    const then = track.positionAt(asked);
    if (then === null) {
      return { kind: "not_present_then", emitted: asked };
    }
    const next = lightTime(distanceM(from, then));
    const change = spanDistance(next, tau);
    tau = next;
    const converged = compareSpans(change, LIGHT_TIME_TOLERANCE) <= 0;
    const noise = previousChange !== null && compareSpans(change, previousChange) >= 0;
    if (converged || noise) {
      const emitted = timeBefore(time, tau);
      const geometricThenM = track.positionAt(emitted);
      if (geometricThenM === null) {
        return { kind: "not_present_then", emitted };
      }
      const r = {
        x: geometricThenM.x - from.x,
        y: geometricThenM.y - from.y,
        z: geometricThenM.z - from.z,
      };
      const seen = aberrated(r, observer.velocityMPerS, tau);
      return {
        kind: "seen",
        emitted,
        lightTime: tau,
        corrections: k,
        residual: change,
        geometricThenM,
        apparentM: { x: from.x + seen.x, y: from.y + seen.y, z: from.z + seen.z },
      };
    }
    previousChange = change;
  }
  return {
    kind: "not_converged",
    corrections: MAX_LIGHT_TIME_CORRECTIONS,
    lastChange: previousChange ?? ZERO_SPAN,
  };
}

/** Where every star and placed body of a system is, for `composePosition`. */
export interface SystemPlacements {
  /** The stars', the pairs' and every placed body's placements. */
  readonly placements: ReadonlyMap<string, BodyPlacement>;
  /** The IDs of the bodies placed on their orbits. */
  readonly placed: ReadonlySet<BodyIdHex>;
  /** The stars' body IDs, primary first. */
  readonly stars: ReadonlyArray<BodyIdHex>;
}

const placementsMemo = new WeakMap<SceneSystem, SystemPlacements>();

/**
 * The placements of a scene's system: the stars by plan 11's hierarchy, and every present body
 * with an orbit on its orbit about what it orbits (the `SYSTEM` display's `layoutBodies`).
 *
 * @remarks
 * Kept for each system model, which only a notification that changes the system replaces (the
 * wire adapter keeps it across a heartbeat or a craft push), so a frame does not lay it out anew.
 */
export function systemPlacements(system: SceneSystem): SystemPlacements {
  const known = placementsMemo.get(system);
  if (known !== undefined) {
    return known;
  }
  const hierarchy = layoutHierarchy(system.model.hierarchy, system.model.hosts);
  const bodies = layoutBodies(system.model.system, system.bodies.bodies, hierarchy);
  const result: SystemPlacements = {
    placements: bodies.placements,
    placed: new Set(bodies.parentKeys.keys()),
    stars: system.model.hosts.map((host) => host.id),
  };
  placementsMemo.set(system, result);
  return result;
}

/**
 * The track of a placed body or a star: its composed position at any time.
 *
 * @remarks
 * It is never absent: whether a body is in the scene is its record's state at the scene's time, and
 * the scene draws a body whose record says it is gone not at all, even within a light time of its
 * end (the plan's Risks, "Elements across an event within the light time").
 */
export function placedTrack(placements: SystemPlacements, id: string): SystemTrack {
  return { positionAt: (time) => composePosition(placements.placements, id, time) };
}

/** A placed body's or a star's velocity relative to the barycentre at `time`, m/s. */
function composedVelocity(placements: SystemPlacements, id: string, time: UniverseTime): Vec3 {
  let velocity: Vec3 = { x: 0, y: 0, z: 0 };
  let at = id;
  for (let steps = 0; steps <= placements.placements.size; steps += 1) {
    const placement = placements.placements.get(at);
    if (placement === undefined) {
      throw new Error(`no body ${at} is placed, on the chain of ${id}`);
    }
    if (placement.kind === "origin") {
      return velocity;
    }
    const own = stateAt(placement.orbit, time).velocityMPerS;
    const share = placement.kind === "member" ? placement.share : 1;
    velocity = add(velocity, { x: own.x * share, y: own.y * share, z: own.z * share });
    at = placement.parentId;
  }
  throw new Error(`the chain of parents of ${id} does not reach the origin`);
}

/**
 * The ship stand-in as the observer at `time`: its pose carried in a straight line at its velocity
 * in its frame, in the scene's system's frame.
 *
 * @remarks
 * `null` when the scene has no system, when the ship's frame is the galactic one or another
 * system's, or when it is a body's that is not placed, none of which a scene the server built has.
 */
export function shipObserver(model: SceneModel, time: UniverseTime): SceneObserver | null {
  const system = model.system;
  if (system === null) {
    return null;
  }
  const { position, velocityMPerS } = model.ship;
  const elapsedS = secondsBetween(time, model.ship.time);
  const moved = (offsetM: Vec3): Vec3 => ({
    x: offsetM.x + velocityMPerS.x * elapsedS,
    y: offsetM.y + velocityMPerS.y * elapsedS,
    z: offsetM.z + velocityMPerS.z * elapsedS,
  });
  let observer: SceneObserver | null = null;
  switch (position.kind) {
    case "galactic":
      break;
    case "system":
      if (position.system === system.model.system) {
        observer = { positionM: moved(position.offsetM), velocityMPerS };
      }
      break;
    case "body": {
      const placements = systemPlacements(system);
      if (placements.placed.has(position.body)) {
        observer = {
          positionM: add(
            composePosition(placements.placements, position.body, time),
            moved(position.offsetM),
          ),
          velocityMPerS: add(composedVelocity(placements, position.body, time), velocityMPerS),
        };
      }
      break;
    }
  }
  return observer;
}

/**
 * A body of the scene as the ship sees it at a frame's time: one the client places on its orbit,
 * or a contact the server placed by its seen position (Design note 13).
 */
export type SceneBodyFrame =
  | {
      readonly kind: "placed";
      readonly id: BodyIdHex;
      /** Where it is at the frame's time, m. */
      readonly geometricM: Vec3;
      /** Where the ship sees it, m. */
      readonly apparentM: Vec3;
      /** When the light seen left it. */
      readonly emitted: UniverseTime;
      readonly lightTime: Span;
      /** The detail level granted for it. */
      readonly level: DetailLevelDto;
      /** Its Hill radius at pericentre, m; `null` below `mass_and_orbit`. */
      readonly hillRadiusM: number | null;
    }
  | {
      readonly kind: "contact";
      readonly id: BodyIdHex;
      /** Where the ship sees it, m, as the server evaluated it; never extrapolated. */
      readonly apparentM: Vec3;
      /** When the light seen left it. */
      readonly emitted: UniverseTime;
      /** The detail level granted for it, `contact`. */
      readonly level: DetailLevelDto;
    };

/** A star of the scene as the ship sees it at a frame's time. */
export interface SceneStarFrame {
  readonly id: BodyIdHex;
  /** Where it is at the frame's time, m. */
  readonly geometricM: Vec3;
  /** Where the ship sees it, m. */
  readonly apparentM: Vec3;
  /** When the light seen left it. */
  readonly emitted: UniverseTime;
  readonly lightTime: Span;
}

/** The scene at one time, as the ship sees it. */
export interface SceneFrame {
  readonly time: UniverseTime;
  readonly observer: SceneObserver;
  /** Every body seen, in index order: present, with an orbit or a seen position. */
  readonly bodies: ReadonlyArray<SceneBodyFrame>;
  readonly stars: ReadonlyArray<SceneStarFrame>;
  /**
   * The ship's local body, drawn geometrically at the present, or `null` in the system frame: by
   * R02's `selectCameraFrame` over the ship's geometric position (Design note 7).
   */
  readonly localBody: BodyIdHex | null;
}

/** The candidates' parent: a moon's planet, a planet's star, or `null` for a pair or barycentre. */
function candidateParent(system: SceneSystem, host: OrbitHost): BodyIdHex | null {
  let parent: BodyIdHex | null;
  switch (host.kind) {
    case "body":
      parent = host.id;
      break;
    case "star":
      parent = formatBodyId({ system: system.model.system, bodyIndex: host.bodyIndex });
      break;
    case "pair":
    case "barycentre":
      parent = null;
      break;
  }
  return parent;
}

/** Whether a body may be a frame: planets, dwarf planets and moons, as R02's rule says. */
function framesBody(body: SystemBody): boolean {
  let frames: boolean;
  switch (body.kind.kind) {
    case "planet":
    case "dwarf_planet":
    case "moon":
      frames = true;
      break;
    case "ring":
    case "belt":
    case "cometary_halo":
    case "protoplanetary_disc":
    case "debris_disc":
    case "unresolved":
      frames = false;
      break;
  }
  return frames;
}

/**
 * The scene at `time` as `observer`, the ship, sees it: each body's and star's geometric and
 * apparent positions and emitted time, each body's level, whether it is a contact and its Hill
 * radius, and the ship's local body.
 *
 * @remarks
 * A body not present by its record is left out, as is a population and a body with neither an
 * orbit nor a seen position; one whose light time does not converge is left out of the drawing but
 * not of the frame rule. A contact is placed at the server's `apparent_m` with no geometric position
 * (Design note 13). The ship's local body is chosen among the planets, dwarf planets and moons with
 * a Hill radius, by their present geometric distance from the observer, with `previous`'s local
 * body as the current one. With `previous` the light times start warm from its own.
 *
 * @param previous - The frame before, or `null` for none: required, so that a caller that has one
 *   cannot forget it and lose the frame rule's hysteresis.
 * @returns The frame, or `null` when the scene has no system.
 */
export function sceneAt(
  model: SceneModel,
  observer: SceneObserver,
  time: UniverseTime,
  previous: SceneFrame | null,
): SceneFrame | null {
  const system = model.system;
  if (system === null) {
    return null;
  }
  const placements = systemPlacements(system);
  const previousTaus = new Map<string, Span>();
  for (const body of previous?.bodies ?? []) {
    if (body.kind === "placed") {
      previousTaus.set(body.id, body.lightTime);
    }
  }
  for (const star of previous?.stars ?? []) {
    previousTaus.set(star.id, star.lightTime);
  }

  const bodies: SceneBodyFrame[] = [];
  const candidates: CameraFrameCandidate[] = [];
  for (const body of system.bodies.bodies) {
    const grant = system.grants.get(body.id);
    if (body.state.kind !== "present" || grant === undefined) {
      continue;
    }
    if (grant.seen !== null) {
      bodies.push({
        kind: "contact",
        id: body.id,
        apparentM: grant.seen.apparentM,
        emitted: grant.seen.emitted,
        level: grant.level,
      });
      continue;
    }
    if (!placements.placed.has(body.id)) {
      continue;
    }
    const track = placedTrack(placements, body.id);
    const geometricM = track.positionAt(time);
    if (geometricM === null) {
      continue;
    }
    const hillRadiusM = system.hillRadiiM.get(body.id) ?? null;
    // The frame rule is geometric and present, whatever the light time does.
    if (hillRadiusM !== null && framesBody(body) && body.parent !== null) {
      candidates.push(
        cameraFrameCandidate(
          body.id,
          candidateParent(system, body.parent),
          distanceM(observer.positionM, geometricM),
          hillRadiusM,
        ),
      );
    }
    const seen = apparentPosition(track, observer, time, previousTaus.get(body.id) ?? null);
    if (seen.kind !== "seen") {
      continue;
    }
    bodies.push({
      kind: "placed",
      id: body.id,
      geometricM,
      apparentM: seen.apparentM,
      emitted: seen.emitted,
      lightTime: seen.lightTime,
      level: grant.level,
      hillRadiusM,
    });
  }

  const stars: SceneStarFrame[] = [];
  for (const id of placements.stars) {
    const track = placedTrack(placements, id);
    const geometricM = track.positionAt(time);
    const seen = apparentPosition(track, observer, time, previousTaus.get(id) ?? null);
    if (seen.kind !== "seen" || geometricM === null) {
      continue;
    }
    stars.push({
      id,
      geometricM,
      apparentM: seen.apparentM,
      emitted: seen.emitted,
      lightTime: seen.lightTime,
    });
  }

  return {
    time,
    observer,
    bodies,
    stars,
    localBody: selectCameraFrame(candidates, previous?.localBody ?? null),
  };
}

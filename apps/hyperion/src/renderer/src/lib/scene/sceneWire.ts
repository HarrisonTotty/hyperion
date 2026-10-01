/**
 * The adapter between the scene subscription's messages and the client's {@link SceneModel}
 * (rendering plan R03, R03.T12; Design notes 4 and 13).
 *
 * @remarks
 * The opening state is the whole scene and each notification only what changed, always with the
 * clock and a `sequence` one above the last. The scene is held as the wire states it, every
 * notification merged in (an arrival replaces the system and its bodies, a re-sent body replaces
 * its record and grant by ID, the clock, the ship and the craft are replaced), and the model is
 * built from that, so that applying every notification since a state equals the state built for
 * the end. The system's bodies are read by plan 14's `toSystemBodiesModel`, which checks them.
 */
import type {
  BodyGrantDto,
  BodyIdHex,
  BodySummaryDto,
  GalacticPosition,
  KinematicsDto,
  SceneBodyDto,
  SceneCraftDto,
  SceneNotificationDto,
  SceneStateDto,
  SceneSystemDto,
  SeenPositionDto,
  SystemIdHex,
  UniverseTime,
} from "@hyperion/protocol";
import { METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";

import { vec3, type Vec3 } from "../../geometry/vec3";
import { toSystemBodiesModel } from "../system/bodiesWire";
import type {
  BodyGrant,
  SceneCraft,
  SceneKinematics,
  SceneModel,
  ScenePosition,
  SceneSystem,
  SeenPosition,
} from "./model";

/** Gives a system's designation of record, which the scene's messages do not carry. */
export type Designate = (system: SystemIdHex) => string;

/** A scene as the display can use it, or why it cannot. */
export type SceneModelResult =
  | { readonly kind: "ok"; readonly model: SceneModel }
  | { readonly kind: "fault"; readonly fault: string };

/**
 * A notification applied, or why it could not be.
 *
 * @remarks
 * `sequence` is a gap or a step back in the notifications' numbering, which the socket never
 * causes: the server's bug, after which `useScene` resubscribes (Design note 4). `fault` is a
 * scene the display cannot use.
 */
export type SceneUpdate =
  | SceneModelResult
  | { readonly kind: "sequence"; readonly expected: number; readonly received: number };

function toVec3([x, y, z]: readonly [number, number, number]): Vec3 {
  return vec3(x, y, z);
}

function allFinite(values: readonly number[]): boolean {
  return values.every((value) => Number.isFinite(value));
}

/** Thrown inside this module for a value the scene cannot use, and caught into a fault. */
class Unusable extends Error {}

function check(condition: boolean, what: string): asserts condition {
  if (!condition) {
    throw new Unusable(what);
  }
}

/** The rates a scene clock runs at: paused, or a power of ten from 1× to 100,000×. */
const CLOCK_RATES: ReadonlySet<number> = new Set([0, 1, 10, 100, 1_000, 10_000, 100_000]);

/** Checks a time is whole seconds and nanoseconds in `[0, 10⁹)`, as the wire's form requires. */
function checkTime(time: UniverseTime, what: string): UniverseTime {
  check(
    Number.isSafeInteger(time.seconds) &&
      Number.isInteger(time.nanos) &&
      time.nanos >= 0 &&
      time.nanos < 1_000_000_000,
    `${what} time unusable`,
  );
  return time;
}

/** Checks a galactic position: whole cells, and offsets in `[0, 1 ly)`. */
function checkGalactic(position: GalacticPosition): GalacticPosition {
  check(
    position.cell_ly.every((cell) => Number.isSafeInteger(cell)) &&
      position.offset_m.every((offset) => offset >= 0 && offset < METRES_PER_LIGHT_YEAR),
    "galactic position unusable",
  );
  return position;
}

function toPosition(position: KinematicsDto["position"]): ScenePosition {
  let result: ScenePosition;
  switch (position.frame) {
    case "galactic":
      result = { kind: "galactic", position: checkGalactic(position.position) };
      break;
    case "system":
      check(allFinite(position.offset_m), "position unusable");
      result = { kind: "system", system: position.system, offsetM: toVec3(position.offset_m) };
      break;
    case "body":
      check(allFinite(position.offset_m), "position unusable");
      result = { kind: "body", body: position.body, offsetM: toVec3(position.offset_m) };
      break;
  }
  return result;
}

function toKinematics(kinematics: KinematicsDto): SceneKinematics {
  check(allFinite(kinematics.velocity_m_s), "velocity unusable");
  return {
    position: toPosition(kinematics.position),
    velocityMPerS: toVec3(kinematics.velocity_m_s),
    time: checkTime(kinematics.time, "pose"),
  };
}

function toCraft(craft: SceneCraftDto): SceneCraft {
  const [x, y, z, w] = craft.attitude;
  check(allFinite(craft.attitude) && allFinite(craft.angular_velocity_rad_s), "craft unusable");
  return {
    craft: craft.craft,
    hull: craft.hull,
    state: toKinematics(craft.state),
    attitude: { w, x, y, z },
    angularVelocityRadPerS: toVec3(craft.angular_velocity_rad_s),
    plannedPath: craft.planned_path === undefined ? null : craft.planned_path.map(toKinematics),
  };
}

function toSeen(seen: SeenPositionDto): SeenPosition {
  check(allFinite(seen.apparent_m), "seen position unusable");
  return { apparentM: toVec3(seen.apparent_m), emitted: checkTime(seen.emitted, "seen") };
}

function toGrant(grant: BodyGrantDto): BodyGrant {
  return { level: grant.level, seen: grant.seen === undefined ? null : toSeen(grant.seen) };
}

/**
 * The scene's system, read: its hosts and bodies by plan 14's adapter, and one grant per body in
 * the bodies' order.
 */
function toSystem(
  wire: SceneSystemDto,
  tidalRadiusM: number | null,
  designate: Designate,
): SceneSystem {
  const system = wire.system.hosts.system;
  const read = toSystemBodiesModel(wire.system, designate(system));
  if (read.kind === "fault") {
    throw new Unusable(read.fault);
  }
  check(
    wire.grants.length === read.bodies.bodies.length &&
      wire.grants.every((grant, index) => grant.body === read.bodies.bodies[index]?.id),
    "grants do not match the bodies",
  );
  check(
    tidalRadiusM === null || (Number.isFinite(tidalRadiusM) && tidalRadiusM > 0),
    "tidal radius unusable",
  );
  return {
    model: read.model,
    bodies: read.bodies,
    grants: new Map(wire.grants.map((grant) => [grant.body, toGrant(grant)])),
    hillRadiiM: hillRadii(wire.system.bodies),
    tidalRadiusM,
  };
}

/**
 * The simulation's constant of gravitation, m³ kg⁻¹ s⁻² (CODATA 2018,
 * `units::consts::GRAVITATIONAL_CONSTANT`), by which a primary's mass is read from an orbit's μ.
 */
const GRAVITATIONAL_CONSTANT = 6.674_3e-11;

/**
 * Every body's Hill radius at pericentre, m, where its record holds its mass and a bound orbit:
 * plan 14's `hill_radius`, a (1 − e) (m ÷ 3M)^⅓, operation for operation.
 *
 * @remarks
 * The primary's mass M is the orbit's μ ÷ G less the body's own, since μ = G (M + m); the kilograms
 * are the wire's, not the model's Earth masses, so that no conversion stands between the two sides.
 * A body below `mass_and_orbit`, whose mass and orbit are withheld, has none.
 */
function hillRadii(bodies: ReadonlyArray<BodySummaryDto>): ReadonlyMap<BodyIdHex, number> {
  const radii = new Map<BodyIdHex, number>();
  for (const body of bodies) {
    if (body.mass_kg.state !== "ok" || body.orbit.state !== "ok") {
      continue;
    }
    const massKg = body.mass_kg.value;
    const orbit = body.orbit.value.orbit;
    const primaryKg = orbit.mu_m3_s2 / GRAVITATIONAL_CONSTANT - massKg;
    const e = orbit.eccentricity;
    if (massKg > 0 && primaryKg > 0 && e >= 0 && e < 1) {
      radii.set(body.id, orbit.semi_major_axis_m * (1 - e) * Math.cbrt(massKg / (3 * primaryKg)));
    }
  }
  return radii;
}

/**
 * Builds the model of a scene the wire states whole, keeping `kept`'s system where the wire's is
 * the very one it was built from, so that a heartbeat or a craft push keeps the system model and
 * everything a frame keeps for it.
 */
function build(
  wire: SceneStateDto,
  tidalRadiusM: number | null,
  designate: Designate,
  kept: SceneModel | null,
): SceneModelResult {
  try {
    check(Number.isSafeInteger(wire.sequence) && wire.sequence >= 0, "sequence unusable");
    check(CLOCK_RATES.has(wire.clock.time_rate), "clock rate unusable");
    return {
      kind: "ok",
      model: {
        sequence: wire.sequence,
        clock: {
          time: checkTime(wire.clock.time, "clock"),
          rate: wire.clock.time_rate,
          state: wire.clock.state,
        },
        ship: toKinematics(wire.ship),
        system:
          wire.system === null
            ? null
            : kept !== null &&
                kept.system !== null &&
                kept.wire.system === wire.system &&
                kept.system.tidalRadiusM === tidalRadiusM
              ? kept.system
              : toSystem(wire.system, tidalRadiusM, designate),
        craft: wire.craft.map(toCraft),
        wire,
      },
    };
  } catch (error: unknown) {
    if (error instanceof Unusable) {
      return { kind: "fault", fault: error.message };
    }
    throw error;
  }
}

/**
 * Turns a scene subscription's opening state into the client's model of the scene, or says why it
 * cannot be shown.
 *
 * @remarks
 * The state carries no tidal radius for a system the scene is already in, so the model's
 * `tidalRadiusM` is `null` until an arrival states one.
 *
 * @param designate - Names a system, as the chart's answers carry its designation of record.
 */
export function toSceneModel(state: SceneStateDto, designate: Designate): SceneModelResult {
  return build(state, null, designate, null);
}

/** Puts `entry` in place of the one with the same ID in a list kept in ID order, or inserts it. */
function replaceById<T>(list: ReadonlyArray<T>, entry: T, idOf: (item: T) => BodyIdHex): T[] {
  const id = idOf(entry);
  const kept = list.filter((item) => idOf(item) !== id);
  const at = kept.findIndex((item) => idOf(item) > id);
  return at < 0 ? [...kept, entry] : [...kept.slice(0, at), entry, ...kept.slice(at)];
}

/** The system with one re-sent body merged in: its record and its grant replaced by ID. */
function withBody(system: SceneSystemDto, body: SceneBodyDto): SceneSystemDto {
  const grant: BodyGrantDto =
    body.seen === undefined
      ? { body: body.record.id, level: body.level }
      : { body: body.record.id, level: body.level, seen: body.seen };
  return {
    system: {
      ...system.system,
      bodies: replaceById(system.system.bodies, body.record, (record) => record.id),
    },
    grants: replaceById(system.grants, grant, (each) => each.body),
  };
}

/**
 * Applies one notification to the scene, giving the scene after it.
 *
 * @remarks
 * The notification's `sequence` must be the model's plus one; anything else is a `sequence` error
 * and the model is unchanged. An arrival replaces the system and every body before it, and keeps
 * its tidal radius; leaving for the galactic frame clears both. Re-sent bodies replace their
 * records and grants by ID, the latest winning. The clock always, and the ship and the craft when
 * sent, replace what was held. A body re-sent while the scene has no system is a fault.
 *
 * @param designate - Names a system, as for {@link toSceneModel}.
 */
export function applySceneNotification(
  model: SceneModel,
  notification: SceneNotificationDto,
  designate: Designate,
): SceneUpdate {
  const expected = model.sequence + 1;
  if (notification.sequence !== expected) {
    return { kind: "sequence", expected, received: notification.sequence };
  }
  let system = model.wire.system;
  let tidalRadiusM = model.system?.tidalRadiusM ?? null;
  if (notification.arrival !== undefined) {
    switch (notification.arrival.type) {
      case "system":
        system = notification.arrival.system;
        tidalRadiusM = notification.arrival.tidal_radius_m;
        break;
      case "no_system":
        system = null;
        tidalRadiusM = null;
        break;
    }
  }
  if (notification.bodies.length > 0 && system === null) {
    return { kind: "fault", fault: "bodies re-sent outside a system" };
  }
  for (const body of notification.bodies) {
    if (system !== null) {
      system = withBody(system, body);
    }
  }
  const wire: SceneStateDto = {
    sequence: notification.sequence,
    clock: notification.clock,
    ship: notification.ship ?? model.wire.ship,
    system,
    craft: notification.craft ?? model.wire.craft,
  };
  return build(wire, tidalRadiusM, designate, model);
}

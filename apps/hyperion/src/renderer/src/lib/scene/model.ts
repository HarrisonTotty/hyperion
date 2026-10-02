/**
 * The client's model of the scene a subscription delivers (rendering plan R03, Design notes 3, 4,
 * 9 and 13): the clock, the ship stand-in, the system the ship is in with every body at its own
 * granted level, and the craft.
 *
 * @remarks
 * Built from the wire by `sceneWire.ts`, the only module of the scene that reads the wire's
 * unit-suffixed field names. Positions are metres along the galactic axes, as on the wire.
 */
import type {
  BodyIdHex,
  DetailLevelDto,
  GalacticPosition,
  SceneClockStateDto,
  SceneStateDto,
  SystemIdHex,
  UniverseTime,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../../view/camera/pose";
import type { SystemBodies, SystemModel } from "../system/model";

/**
 * The scene clock as of the latest push: the time, the rate it runs at and whether it runs
 * (Design note 2).
 */
export interface SceneClock {
  /** The scene time when the push was made. */
  readonly time: UniverseTime;
  /** Scene seconds per real second: 0 (paused) or a power of ten from 1 to 100,000. */
  readonly rate: number;
  /** `running`, `paused`, or `window_limit` once the clock has reached the clock window's edge. */
  readonly state: SceneClockStateDto;
}

/**
 * A position in one of the scene's frames: the galactic frame, a system's (from its barycentre)
 * or a body's (from its centre, not rotating), each along the galactic axes.
 */
export type ScenePosition =
  | { readonly kind: "galactic"; readonly position: GalacticPosition }
  | { readonly kind: "system"; readonly system: SystemIdHex; readonly offsetM: Vec3 }
  | { readonly kind: "body"; readonly body: BodyIdHex; readonly offsetM: Vec3 };

/** A pose without attitude: a position in a frame, the velocity in that frame and their time. */
export interface SceneKinematics {
  readonly position: ScenePosition;
  /** The velocity relative to the frame's origin, m/s along the galactic axes. */
  readonly velocityMPerS: Vec3;
  readonly time: UniverseTime;
}

/** Where a body granted only `contact` was seen from the ship: the server's evaluation. */
export interface SeenPosition {
  /** Its apparent position, m from the system's barycentre along the galactic axes. */
  readonly apparentM: Vec3;
  /** When the light seen left it. */
  readonly emitted: UniverseTime;
}

/** What the ship knows of one body: its granted level, and where it was seen for a contact. */
export interface BodyGrant {
  readonly level: DetailLevelDto;
  /** Set for a body placed by the server's seen position (Design note 13), else `null`. */
  readonly seen: SeenPosition | null;
}

/**
 * Where a scene's system is: its designation of record, which every body's extends, and its
 * barycentre in the galactic frame, which the scene's bodies, all in the system's frame, do not
 * state (rendering plan R03, R03.T16).
 *
 * @remarks
 * The scene states it (`SceneSystemDto.place`); a place known only from the chart (the `SYSTEM`
 * display's opening) has no velocity or time, and a system known by neither has its ID for a
 * designation and no barycentre. `barycentreAt` (`place.ts`) carries it to another time.
 */
export interface SystemPlace {
  readonly system: SystemIdHex;
  /** The catalogue designation, or the system's ID where none is known. */
  readonly designation: string;
  /** The barycentre at {@link time}, or `null` where the client has not been told where it is. */
  readonly barycentre: GalacticPosition | null;
  /**
   * The barycentre's velocity, m/s along the galactic axes, constant (plan 08, P08.T7.a); `null`
   * for a place known only from the chart.
   */
  readonly velocityMPerS: Vec3 | null;
  /** When `barycentre` holds; `null` for a place known only from the chart. */
  readonly time: UniverseTime | null;
}

/**
 * The system the scene is in: its stars and hierarchy, its bodies each at its own grant, and the
 * grants.
 */
export interface SceneSystem {
  /** Its stars and how they pair, from the bodies answer's own hosts. */
  readonly model: SystemModel;
  /** Its bodies, each record degraded to the body's own grant by the server. */
  readonly bodies: SystemBodies;
  /** Every body's grant, by ID. */
  readonly grants: ReadonlyMap<BodyIdHex, BodyGrant>;
  /**
   * Every body's Hill radius at pericentre, m, by ID, where its record holds its mass and a bound
   * orbit; R02's frame selection reads it (rendering plan R03, R03.T13).
   */
  readonly hillRadiiM: ReadonlyMap<BodyIdHex, number>;
  /**
   * The system's tidal radius, m (R02's free-camera clamp): the latest arrival's, or the opening
   * state's when the scene was already in the system; `null` only from a state that omits it.
   */
  readonly tidalRadiusM: number | null;
  /**
   * Where the system is, as the scene states it (R03.T16); `null` from a server that does not
   * send it, where the system is named by the `designate` the adapter was given.
   */
  readonly place: SystemPlace | null;
}

/** A craft in the scene: a draft of the sessions plan's, the least a renderer needs (Design note 4). */
export interface SceneCraft {
  /** Its identity within the scene. */
  readonly craft: string;
  /** The key of its hull definition. */
  readonly hull: string;
  readonly state: SceneKinematics;
  /** Its attitude, body to frame axes. */
  readonly attitude: Quaternion;
  /** Its angular velocity, rad/s about the frame's axes. */
  readonly angularVelocityRadPerS: Vec3;
  /** The flight computer's predicted path, poses at stated times; `null` when it has none. */
  readonly plannedPath: ReadonlyArray<SceneKinematics> | null;
}

/** The whole scene as the client holds it, after the opening state and every notification since. */
export interface SceneModel {
  /** The sequence of the latest push applied; 0 for the opening state. */
  readonly sequence: number;
  readonly clock: SceneClock;
  /** The ship stand-in's pose; it moves in a straight line at its velocity in its frame. */
  readonly ship: SceneKinematics;
  /** The system the ship is in, or `null` in the galactic frame. */
  readonly system: SceneSystem | null;
  readonly craft: ReadonlyArray<SceneCraft>;
  /** The scene as the wire states it, which every notification is merged into. */
  readonly wire: SceneStateDto;
}

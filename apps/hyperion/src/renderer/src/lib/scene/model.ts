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
   * The system's tidal radius at the arrival, m (R02's free-camera clamp); `null` when the scene
   * was already in the system when the subscription opened, since the state does not carry it.
   */
  readonly tidalRadiusM: number | null;
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

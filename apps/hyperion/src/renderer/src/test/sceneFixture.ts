/**
 * Scene subscription messages built from plan 14's shared wire fixture (rendering plan R03,
 * R03.T12): a state, and the notifications of an arrival, a body re-sent at a new level, a
 * heartbeat and a leaving, by hand, since no server sends them to the client's tests.
 */
import type {
  BodyGrantDto,
  BodySummaryDto,
  KinematicsDto,
  SceneBodyDto,
  SceneClockDto,
  SceneCraftDto,
  SceneNotificationDto,
  SceneStateDto,
  SceneSystemDto,
  SystemBodiesDto,
  SystemIdHex,
} from "@hyperion/protocol";

import { earthContact, FIXTURE_EARTH, FIXTURE_SYSTEM, sliceBodies } from "./planetaryFixture";

/** The fixture system's designation, as a chart would carry it. */
export const SCENE_DESIGNATION = "H7K 4C0RFZ D-7";

/** Names every system by {@link SCENE_DESIGNATION}. */
export function designateFixture(_system: SystemIdHex): string {
  return SCENE_DESIGNATION;
}

/** The scene clock at `seconds`, running at `rate`. */
export function sceneClock(seconds: number, rate = 1_000): SceneClockDto {
  return { time: { seconds, nanos: 0 }, time_rate: rate, state: "running" };
}

/** The ship stand-in 1 AU from the fixture system's barycentre, at `seconds`. */
export function shipInSystem(seconds: number): KinematicsDto {
  return {
    position: { frame: "system", system: FIXTURE_SYSTEM, offset_m: [1.5e11, 2e9, 0] },
    velocity_m_s: [0, 29_780, 0],
    time: { seconds, nanos: 0 },
  };
}

/** The ship stand-in in the galactic frame, near the fixture system, at `seconds`. */
export function shipInSpace(seconds: number): KinematicsDto {
  return {
    position: {
      frame: "galactic",
      position: { cell_ly: [8_000, 10, 20], offset_m: [1e15, 2e15, 3e15] },
    },
    velocity_m_s: [1e4, 0, 0],
    time: { seconds, nanos: 0 },
  };
}

/** The slice's bodies answer as the scene carries it, without the response's `kind`. */
export function sliceSystemBodies(): SystemBodiesDto {
  const { kind: _kind, ...bodies } = sliceBodies();
  return bodies;
}

/** The slice's system as a scene holds it, every body granted the level asked. */
export function sliceSceneSystem(): SceneSystemDto {
  const system = sliceSystemBodies();
  return {
    system,
    grants: system.bodies.map((body): BodyGrantDto => ({ body: body.id, level: system.granted })),
  };
}

/** The slice's Earth at the `contact` level, as a list entry. */
export function earthContactSummary(): BodySummaryDto {
  const { surface: _surface, hooks: _hooks, ...summary } = earthContact().record;
  return summary;
}

/** The slice's Earth re-sent at `contact`, placed by the server's seen position. */
export function earthReSentAsContact(): SceneBodyDto {
  return {
    level: "contact",
    record: earthContactSummary(),
    seen: { apparent_m: [1.49e11, 1.2e10, -3e6], emitted: { seconds: 3_599, nanos: 500_000_000 } },
  };
}

/** The scene's opening state: the ship in the galactic frame, with no system. */
export function stateInSpace(): SceneStateDto {
  return {
    sequence: 0,
    clock: sceneClock(3_000),
    ship: shipInSpace(3_000),
    system: null,
    craft: [],
  };
}

/**
 * The notifications after {@link stateInSpace}: an arrival in the slice's system, the Earth
 * re-sent as a contact, a heartbeat, and a leaving.
 */
export function sceneSequence(): SceneNotificationDto[] {
  return [
    {
      sequence: 1,
      clock: sceneClock(3_400),
      ship: shipInSystem(3_400),
      arrival: { type: "system", system: sliceSceneSystem(), tidal_radius_m: 2.1e16 },
      bodies: [],
    },
    { sequence: 2, clock: sceneClock(3_600), bodies: [earthReSentAsContact()] },
    { sequence: 3, clock: sceneClock(4_600), bodies: [] },
    {
      sequence: 4,
      clock: sceneClock(5_600),
      ship: shipInSpace(5_600),
      arrival: { type: "no_system" },
      bodies: [],
    },
  ];
}

/** The state of the scene after the first three of {@link sceneSequence}, built directly. */
export function stateAfterHeartbeat(): SceneStateDto {
  const system = sliceSceneSystem();
  const earth = earthReSentAsContact();
  return {
    sequence: 3,
    clock: sceneClock(4_600),
    ship: shipInSystem(3_400),
    system: {
      system: {
        ...system.system,
        bodies: system.system.bodies.map((body) =>
          body.id === FIXTURE_EARTH ? earth.record : body,
        ),
      },
      grants: system.grants.map((grant) =>
        grant.body === FIXTURE_EARTH && earth.seen !== undefined
          ? { body: FIXTURE_EARTH, level: earth.level, seen: earth.seen }
          : grant,
      ),
    },
    craft: [],
  };
}

/** A craft in the galactic frame with no planned path. */
export function craftFixture(): SceneCraftDto {
  return {
    craft: "ISV-1",
    hull: "corvette",
    state: shipInSpace(3_000),
    attitude: [0.1, 0.2, 0.3, 0.5],
    angular_velocity_rad_s: [0, 0, 0.01],
  };
}

/** The state of the scene after the whole of {@link sceneSequence}, built directly. */
export function stateAfterLeaving(): SceneStateDto {
  return {
    sequence: 4,
    clock: sceneClock(5_600),
    ship: shipInSpace(5_600),
    system: null,
    craft: [],
  };
}

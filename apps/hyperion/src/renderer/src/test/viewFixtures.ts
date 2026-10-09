/**
 * Builders for the `VIEW` display's camera and scene tests (plan R02).
 *
 * @remarks
 * One system holds an Earth-like planet 1 au from the barycentre and a Moon-like moon, with their
 * Hill radii as `hill_radius` gives them (R02.T4's figures), and an own ship in the planet's
 * neighbourhood. Overrides make the variations a test needs.
 */
import { type BodyIdHex, galacticPositionFromLy, type SystemIdHex } from "@hyperion/protocol";

import { add, normalise, type Vec3, vec3 } from "../geometry/vec3";
import type { CameraPose, Quaternion } from "../view/camera/pose";
import type { CameraScene, OwnShip } from "../view/camera/state";
import type { ViewPosition } from "../view/coords/position";
import { TEST_HULL } from "../view/scene/hull";
import {
  bodyKindSymbol,
  type CraftPose,
  staticRetarded,
  type ViewBody,
  type ViewCraft,
  type ViewOrbit,
  type ViewScene,
  type ViewStar,
} from "../view/scene/model";
import type { DrawCamera } from "../view/wireframe/drawList";

/** The fixtures' system. */
export const FIXTURE_SYSTEM: SystemIdHex = "0200080020000000";

/** The fixtures' Earth-like planet. */
export const FIXTURE_PLANET: BodyIdHex = `${FIXTURE_SYSTEM}.0103`;

/** The fixtures' Moon-like moon of {@link FIXTURE_PLANET}. */
export const FIXTURE_MOON: BodyIdHex = `${FIXTURE_SYSTEM}.0104`;

/** One astronomical unit, m (IAU 2012 Resolution B2). */
export const AU_M = 1.495_978_707e11;

/** The planet's centre, m from the barycentre: 1 au along +x. */
export const FIXTURE_PLANET_CENTRE_M: Vec3 = vec3(AU_M, 0, 0);

/** The moon's centre, m from the barycentre: 3.844 × 10⁸ m beyond the planet along +y. */
export const FIXTURE_MOON_CENTRE_M: Vec3 = vec3(AU_M, 3.844e8, 0);

/** The planet's Hill radius, m: Earth's, 1.5 × 10⁹ m. */
export const FIXTURE_PLANET_HILL_M = 1.5e9;

/** The moon's Hill radius, m: the Moon's, about 5.8 × 10⁷ m. */
export const FIXTURE_MOON_HILL_M = 5.8e7;

/** The system's tidal radius, m: the Sun's, some 2.7 × 10⁵ au (R03's design note 7). */
export const FIXTURE_TIDAL_RADIUS_M = 2.7e5 * AU_M;

/** The own ship's craft ID. */
export const FIXTURE_SHIP = "ship";

/** The identity rotation. */
export const NO_TURN: Quaternion = { w: 1, x: 0, y: 0, z: 0 };

/** A camera pose, by default at rest 10⁸ m from the barycentre in the system frame. */
export function aCameraPose(overrides: Partial<CameraPose> = {}): CameraPose {
  return {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: vec3(0, 1e8, 0),
    orientation: NO_TURN,
    ...overrides,
  };
}

/** The own ship: a 20 m hull with its eye point 6 m forward of centre and 1.5 m up. */
export function anOwnShip(overrides: Partial<OwnShip> = {}): OwnShip {
  return {
    craft: FIXTURE_SHIP,
    attitude: NO_TURN,
    eyePointM: vec3(0, 1.5, -6),
    lengthM: 20,
    ...overrides,
  };
}

/** What {@link aCameraScene} can vary beyond the scene's own fields. */
export interface CameraSceneOptions {
  /** Where the own ship is. */
  readonly shipAt: ViewPosition;
}

/**
 * A camera scene over the fixtures' system: the planet and moon as frame bodies and targets, the
 * own ship 2 × 10⁷ m from the planet in its body frame, and a default pose 10⁸ m from the
 * barycentre.
 */
export function aCameraScene(
  overrides: Partial<CameraScene> = {},
  options: Partial<CameraSceneOptions> = {},
): CameraScene {
  const barycentre = galacticPositionFromLy([8_000, 26_000, 20]);
  const shipAt: ViewPosition = options.shipAt ?? {
    kind: "body",
    body: FIXTURE_PLANET,
    m: vec3(2e7, 0, 0),
  };
  const origins: CameraScene["origins"] = {
    systemBarycentre: () => barycentre,
    bodyCentreM: (body) =>
      body === FIXTURE_MOON ? FIXTURE_MOON_CENTRE_M : FIXTURE_PLANET_CENTRE_M,
    bodyFixedRotation: () => null,
    craftPosition: () => shipAt,
  };
  return {
    system: FIXTURE_SYSTEM,
    tidalRadiusM: FIXTURE_TIDAL_RADIUS_M,
    origins,
    frameBodies: [
      { id: FIXTURE_PLANET, parent: null, hillRadiusM: FIXTURE_PLANET_HILL_M },
      { id: FIXTURE_MOON, parent: FIXTURE_PLANET, hillRadiusM: FIXTURE_MOON_HILL_M },
    ],
    targets: [
      { kind: "body", body: FIXTURE_PLANET },
      { kind: "body", body: FIXTURE_MOON },
    ],
    ownShip: anOwnShip(),
    defaultPose: aCameraPose(),
    ...overrides,
  };
}

/**
 * A body of the fixtures' system, by default the Earth-like planet 1 au out, at rest where it is
 * drawn (`staticRetarded` at its centre) unless `overrides` gives its `retarded`.
 */
export function aBody(overrides: Partial<ViewBody> = {}): ViewBody {
  const centreM = overrides.centreM ?? FIXTURE_PLANET_CENTRE_M;
  return {
    id: FIXTURE_PLANET,
    parent: null,
    kind: "planet",
    designation: "TEST PLANET",
    radiusM: 6.371e6,
    hillRadiusM: FIXTURE_PLANET_HILL_M,
    centreM,
    retarded: staticRetarded(centreM),
    rotation: null,
    appearance: null,
    symbol: bodyKindSymbol("planet"),
    orbitNormal: null,
    ...overrides,
  };
}

/** A craft with the test hull, by default the own ship 2 × 10⁷ m from the planet. */
export function aViewCraft(overrides: Partial<ViewCraft> = {}): ViewCraft {
  return {
    id: FIXTURE_SHIP,
    designation: "TEST HULL",
    hull: TEST_HULL,
    pose: {
      position: { kind: "body", body: FIXTURE_PLANET, m: vec3(2e7, 0, 0) },
      attitude: NO_TURN,
    },
    predictedPath: null,
    velocityMPerS: null,
    ...overrides,
  };
}

/** A star of the sky, by default a Sun-like star 10 pc off along +y. */
export function aViewStar(overrides: Partial<ViewStar> = {}): ViewStar {
  return {
    id: "0200080020000001",
    direction: vec3(0, 1, 0),
    distanceM: 3.085_677_581_491_367e17,
    absoluteV: 4.83,
    tEffK: 5_772,
    ...overrides,
  };
}

/**
 * A view scene over the fixtures' system: the planet and its moon, the own ship near the planet,
 * one star, and a default pose 10⁸ m from the barycentre.
 */
export function aViewScene(overrides: Partial<ViewScene> = {}): ViewScene {
  return {
    provenance: { kind: "server" },
    time: { seconds: 0, nanos: 0 },
    timeRate: 1,
    system: FIXTURE_SYSTEM,
    barycentre: galacticPositionFromLy([8_000, 26_000, 20]),
    tidalRadiusM: FIXTURE_TIDAL_RADIUS_M,
    bodies: [
      aBody(),
      aBody({
        id: FIXTURE_MOON,
        parent: FIXTURE_PLANET,
        kind: "moon",
        designation: "TEST MOON",
        radiusM: 1.737e6,
        hillRadiusM: FIXTURE_MOON_HILL_M,
        centreM: FIXTURE_MOON_CENTRE_M,
      }),
    ],
    rings: [],
    orbits: [],
    craft: [aViewCraft()],
    stars: [aViewStar()],
    ownShip: FIXTURE_SHIP,
    defaultPose: aCameraPose(),
    ...overrides,
  };
}

/** A free camera 3 × 10⁷ m from the planet along +z, looking at it down −z. */
export const OFF_PLANET_CAMERA: DrawCamera = {
  pose: {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: add(FIXTURE_PLANET_CENTRE_M, vec3(0, 0, 3e7)),
    orientation: NO_TURN,
  },
  fovXRad: Math.PI / 3,
};

/** The moon's orbit about the planet, a Moon-like Kepler orbit, for a view that draws it. */
export const FIXTURE_MOON_ORBIT: ViewOrbit = {
  body: FIXTURE_MOON,
  parent: FIXTURE_PLANET,
  orbit: {
    semiMajorAxisM: 3.844e8,
    eccentricity: 0.0549,
    inclinationRad: 0.09,
    ascendingNodeRad: 1,
    argumentOfPeriapsisRad: 0.3,
    meanAnomalyAtEpochRad: 0,
    periodS: 2.36e6,
  },
};

/**
 * A craft's pose 10⁶ m off the planet's centre and `dzM` m towards {@link OFF_PLANET_CAMERA}, in
 * the system frame.
 */
export function poseOffPlanet(dzM: number): CraftPose {
  return {
    position: {
      kind: "system",
      system: FIXTURE_SYSTEM,
      m: add(FIXTURE_PLANET_CENTRE_M, vec3(1e6, 0, dzM)),
    },
    attitude: NO_TURN,
  };
}

/**
 * The fixture scene with marks of every kind a view strokes from {@link OFF_PLANET_CAMERA}: the
 * moon's orbit drawn, the own ship moving, another craft (`other`, its hull in view) on a
 * predicted path, and a sky of 40 stars ahead.
 */
export function aMarkedViewScene(overrides: Partial<ViewScene> = {}): ViewScene {
  return aViewScene({
    orbits: [FIXTURE_MOON_ORBIT],
    craft: [
      aViewCraft({ velocityMPerS: vec3(10, 0, -100) }),
      aViewCraft({
        id: "other",
        designation: "OTHER",
        pose: poseOffPlanet(1e7),
        predictedPath: [poseOffPlanet(1e7), poseOffPlanet(9e6), poseOffPlanet(8e6)],
      }),
    ],
    stars: Array.from({ length: 40 }, (_, i) =>
      aViewStar({
        id: `02000800200000${(i + 16).toString(16)}`,
        direction: normalise(vec3(Math.sin(i) * 0.3, Math.cos(i * 1.7) * 0.2, -1)),
        absoluteV: -2 + i * 0.2,
      }),
    ),
    ...overrides,
  });
}

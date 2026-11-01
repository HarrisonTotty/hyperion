/**
 * The kept atmosphere test scene, `ATMOSPHERE TEST` (plan R08, R08.T10.b): R08.T12.b's reference
 * cases as bodies of one system, to fly past at a few radii, and a moonlit night on the ground of
 * one of them.
 *
 * @remarks
 * - **Its bodies.** A Sun-like star, `TEST STAR`, at the barycentre, lit by the scene's own host disc
 *   (`sunLikeHostDisc`, as `PHASE TEST`'s); eight air fixtures about 1 au out, each carrying a
 *   reference case's medium: `reference/cases.ts`'s cloud-free four, `TEST EARTH` (`earth`, the
 *   measured Earth), `TEST EARTH O3 SLAB` (`earth-ozone-slab`, T6.d's 300 DU slab), `TEST MARS`
 *   (`mars`, with its hand dust) and `TEST SATURN` (`saturn-oblate`, the Saturn-class air on
 *   Saturn's level spheroid, its pole +z); `thickCases.ts`'s four (R08.T12.b.2), `TEST VENUS`
 *   (`venus-92bar`, Venus's air at its 9.2 MPa under P14.T24.g's four-mode sulphuric-acid deck),
 *   `TEST VENUS 5.80 MPa` (`venus-58bar`, the same air and deck at plan 14's 5.80 MPa),
 *   `TEST TITAN` (`titan-class`, a Titan-class air under its tholin haze) and
 *   `TEST SATURN NH3 ICE DECK` (`giant-deck`, the Saturn-class air over a thick NH3 ice deck, on a
 *   sphere of Saturn's mean radius); `TEST JUPITER` (R08.T16.a.2), the hand Jupiter
 *   (`test/giantFixtures.ts`), a gas envelope on Jupiter's figure and spin (f 0.065) given as a
 *   record and built as a generated giant's is; and `TEST MOON`, airless, of the Moon's radius,
 *   full in `TEST EARTH`'s night sky.
 * - **Its layout** ({@link ATMOSPHERE_FIXTURES}): `TEST EARTH` 1 au out along +x, and the other air
 *   fixtures a quarter of the way round the star, 1.41 au from it, so that its night is lit by its
 *   moon alone; there each after `TEST EARTH O3 SLAB` lies {@link ATMOSPHERE_LEG_M} from the one
 *   before, half of it farther from the star and the rest across its light, along +z and +x in
 *   turn, each leg so turned a quarter from the last about the star's direction, so that a camera
 *   flown from one to the next at a few radii sees each at about 60° of phase, passing none on its
 *   way, and no fixture stands in another's shadow. The legs between `TEST EARTH` and the others
 *   come on each at 45°.
 * - **Its media** are the cases' own, in three loads ({@link AtmosphereFixtureLoad}), all started
 *   when `VIEW` chooses the scene (its `SceneOption`'s `prepare`, from an effect;
 *   {@link prepareAtmosphereScene}), never as the scene is made: the cloud-free four from a lazy
 *   import of `reference/cases.ts`, which pulls the optics' data that no other kept scene needs;
 *   the thick four from `thickCases.ts`, whose Mie runs for about a minute, built by a worker
 *   of their own (`reference/thickCases.worker.ts`; {@link loadThickCases}), off the render thread;
 *   and `TEST JUPITER`'s record from a lazy import of `test/giantFixtures.ts`, which the
 *   atmosphere cache builds in its optics workers, as it builds a server body's reply (its setting
 *   `record`, `ATMOSPHERE: PENDING` until it lands, about 8 s of k-tables and optics).
 *   Each air body is stated `loading` (`ATMOSPHERE: PENDING`) until its load lands, then its case's
 *   medium, built once and held, since the atmosphere cache compares a kept medium by identity. A
 *   failed load is logged once and states each of its air bodies `failed`, item 6's failure, which
 *   a page cannot retry: `ATMOSPHERE NOT AVAILABLE: computation failed for <designations>, relaunch
 *   to retry` (`decision-r08-t10b-ux.md`, item 4), standing when `VIEW` is shown again; the other
 *   load's bodies are drawn as it lands. Each air body is drawn on its medium's own figure, the one
 *   its tables take (`hillaire.ts`'s `tablesFigureOf`): a sphere of the case's ground radius, or
 *   Saturn's spheroid; before its load lands, on the same figures restated here, which the test
 *   holds to the cases'. `TEST JUPITER` is drawn on its envelope's floor, the level surface its
 *   tables stand on (R08.T16.a.2), restated as its figure, black. Its disc is a grey Lambert sphere of its medium's ground albedo,
 *   provisional (`BODY PHOTOMETRY: NOT YET MODELLED`), so that the ground the composite's march
 *   draws at the limb is the disc's (`decision-r08-t10b-depth.md`, text B).
 * - **The moonlit night.** The own ship, `TEST HULL`, stands body-fixed {@link ATMOSPHERE_SHIP_HEIGHT_M}
 *   over `TEST EARTH`'s ground on its night side, pitched up at the moon and clear of the ground, the star {@link ATMOSPHERE_SUN_DEPRESSION_DEG}
 *   below the horizon there, beyond the measured Earth's twilight limit (about 20°, `sources.ts`'s
 *   `twilightLimitRad`), and its seat looks at `TEST MOON`, near the anti-solar point, out of
 *   `TEST EARTH`'s shadow and high in the sky. The stage starts at the seat.
 * - **Its stars** ({@link ATMOSPHERE_STARS}) are its own: a fixed field of a few hundred stars over
 *   the sphere, magnitudes 0 to 6, which a frame drawn with no server draws in the interim field's
 *   place, so that the sky's washing-out of faint stars shows.
 * - **Static**: nothing moves, and the script runs {@link ATMOSPHERE_DURATION_S} before it starts
 *   again.
 */

import type { BodyIdHex, HostDiscDto } from "@hyperion/protocol";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { BodyDetail } from "../../lib/system/model";
import {
  type BodyPhotometry,
  PROVISIONAL_LABELS,
  type WireAppearance,
} from "../appearance/fromWire";
import { lawFor } from "../appearance/phase";
import type { BuiltAtmosphere, KeptBodyAtmosphere } from "../atmosphere/AtmosphereCache";
import type { ReferenceCase } from "../atmosphere/reference/cases";
import type {
  FixtureCase,
  ThickCasesReply,
  ThickFixtureCases,
} from "../atmosphere/reference/thickCasesMessages";
import type { CameraPose, Quaternion } from "../camera/pose";
import { lookAlong, rotate } from "../camera/quaternion";
import { frozenHostDisc, SUN_RADIUS_M, sunLikeHostDisc } from "../lighting/hostDisc";
import { PARSEC_M } from "../photometry/magnitude";
import { TEST_HULL } from "../scene/hull";
import {
  bodyKindSymbol,
  staticRetarded,
  type ViewBody,
  type ViewScene,
  type ViewStar,
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

/** The atmosphere test scene's name, as the `SCENE` selector shows it. */
export const ATMOSPHERE_SCENE_NAME = "ATMOSPHERE TEST";

/** How long the static scene runs before it starts again, s. */
export const ATMOSPHERE_DURATION_S = 60;

/** The scene's star, body index 0, the host disc's `star`. */
export const ATMOSPHERE_STAR = keptBody(0);

/**
 * The scene's other bodies' indices start here, past every other kept scene's, so that no body of
 * another kept scene shares an ID, and with it an atmosphere cache's entry, with one of these.
 */
const BODY_INDEX_BASE = 0x40;

/** `TEST EARTH`. */
export const ATMOSPHERE_EARTH = keptBody(BODY_INDEX_BASE + 1);

/** `TEST EARTH O3 SLAB`. */
export const ATMOSPHERE_EARTH_SLAB = keptBody(BODY_INDEX_BASE + 2);

/** `TEST MARS`. */
export const ATMOSPHERE_MARS = keptBody(BODY_INDEX_BASE + 3);

/** `TEST SATURN`. */
export const ATMOSPHERE_SATURN = keptBody(BODY_INDEX_BASE + 4);

/** `TEST MOON`. */
export const ATMOSPHERE_MOON = keptBody(BODY_INDEX_BASE + 5);

/** `TEST VENUS`. */
export const ATMOSPHERE_VENUS = keptBody(BODY_INDEX_BASE + 6);

/** `TEST VENUS 5.80 MPa`. */
export const ATMOSPHERE_VENUS_58 = keptBody(BODY_INDEX_BASE + 7);

/** `TEST TITAN`. */
export const ATMOSPHERE_TITAN = keptBody(BODY_INDEX_BASE + 8);

/** `TEST SATURN NH3 ICE DECK`. */
export const ATMOSPHERE_GIANT_DECK = keptBody(BODY_INDEX_BASE + 9);

/** `TEST JUPITER` (R08.T16.a.2). */
export const ATMOSPHERE_JUPITER = keptBody(BODY_INDEX_BASE + 10);

/** A cloud-free reference case an air fixture carries, by its name in `reference/cases.ts`. */
export type AtmosphereCloudFreeCase = "earth" | "earth-ozone-slab" | "mars" | "saturn-oblate";

/** A cloudy or hazy reference case an air fixture carries, by its name in `thickCases.ts`. */
export type AtmosphereThickCase = keyof ThickFixtureCases;

/**
 * A record an air fixture carries, which the atmosphere cache builds as a server body's reply
 * (R08.T16.a.2): `hand-jupiter`, `test/giantFixtures.ts`'s hand Jupiter.
 */
export type AtmosphereRecordCase = "hand-jupiter";

/** The reference case, or the record, an air fixture carries. */
export type AtmosphereFixtureCase =
  AtmosphereCloudFreeCase | AtmosphereThickCase | AtmosphereRecordCase;

/**
 * Where an air fixture's case is loaded from: `cases`, the lazy import of `reference/cases.ts`;
 * `thick`, the thick cases' worker; `giant`, the lazy import of the hand Jupiter's record.
 */
export type AtmosphereFixtureLoad = "cases" | "thick" | "giant";

/** The load a case comes in. */
export function fixtureLoadOf(caseName: AtmosphereFixtureCase): AtmosphereFixtureLoad {
  let load: AtmosphereFixtureLoad;
  switch (caseName) {
    case "earth":
    case "earth-ozone-slab":
    case "mars":
    case "saturn-oblate":
      load = "cases";
      break;
    case "venus-92bar":
    case "venus-58bar":
    case "titan-class":
    case "giant-deck":
      load = "thick";
      break;
    case "hand-jupiter":
      load = "giant";
      break;
  }
  return load;
}

/** A figure as a body's tables take it: its equatorial and polar radii, m. */
export interface AtmosphereFixtureFigure {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
}

/** One of the scene's air fixtures. */
export interface AtmosphereFixture {
  readonly id: BodyIdHex;
  readonly designation: string;
  /** The reference case whose medium it carries, or the record it gives (`TEST JUPITER`'s). */
  readonly caseName: AtmosphereFixtureCase;
  /** Its centre, m from the barycentre along the galactic axes. */
  readonly centreM: Vec3;
  /**
   * The figure its medium is layered on, restated from its case, so that it is drawn on it before
   * the cases load; the case's own once they have (the test holds the two equal).
   */
  readonly figure: AtmosphereFixtureFigure;
  /** Whether it has no surface for a survey line: the giant. */
  readonly surfaceless: boolean;
  /**
   * Its medium's ground albedo, grey, restated from its case as its figure is (the test holds the
   * two equal): its disc's Lambert sphere is shaded with p = A ÷ 1.5, its Bond albedo A.
   */
  readonly groundAlbedo: number;
}

/** The distance from each air fixture to the next, m. */
export const ATMOSPHERE_LEG_M = 1e9;

/** `TEST EARTH`'s centre: 1 au out along +x, at rest. */
export const ATMOSPHERE_EARTH_CENTRE_M: Vec3 = vec3(AU_M, 0, 0);

/**
 * `TEST EARTH O3 SLAB`'s centre, where the other air fixtures' legs start: 1 au out along +y, a
 * quarter of the way round the star from `TEST EARTH` and 1.41 au from it, so that the moonlit
 * night is lit by `TEST MOON` alone. Seen from `TEST EARTH` the others stand 45° from the star,
 * below the night horizon, and the brightest, `TEST SATURN`, gives it about 1% of the Moon's light
 * (at 1.7 × 10⁹ m, as the legs first had it, 127 times: the flight of 2026-10-26).
 */
const FIXTURES_START_M: Vec3 = vec3(0, AU_M, 0);

/** √3 ÷ 2 of a leg: its part across the star's light. */
const ACROSS_M = (Math.sqrt(3) / 2) * ATMOSPHERE_LEG_M;

/**
 * The leg `index` from one air fixture to the next after `TEST EARTH O3 SLAB`: half away from the
 * star (+y) and the rest across its light, so that the next is seen at 60° of phase from the one
 * before, along +z and +x in turn, each so turned a quarter from the last about the star's
 * direction, so that no leg runs on through the fixture it leaves, and the fixtures climb away
 * across the light, none behind another, in its shadow, as legs turning one way would bring the
 * fifth behind the first.
 */
function legStep(index: number): Vec3 {
  return index % 2 === 0
    ? vec3(0, 0.5 * ATMOSPHERE_LEG_M, ACROSS_M)
    : vec3(ACROSS_M, 0.5 * ATMOSPHERE_LEG_M, 0);
}

/** The air fixture `legs` legs on from `TEST EARTH O3 SLAB`. */
function legsOn(legs: number): Vec3 {
  return Array.from({ length: legs }, (_, index) => legStep(index)).reduce(
    (atM, stepM) => add(atM, stepM),
    FIXTURES_START_M,
  );
}

/**
 * Earth's sphere: R05's `tableRadiusM` of WGS 84, (2a + c) ÷ 3 = 6,371,008.77 m, as
 * `reference/cases.ts`'s `EARTH_REFERENCE_RADIUS_M` gives it (WGS 84's a and 1 ÷ f; NIMA TR8350.2).
 */
const EARTH_SPHERE_M = (2 * 6_378_137 + 6_378_137 * (1 - 1 / 298.257_223_563)) / 3;

/** Mars's sphere: R08.T4.a's column's volumetric mean radius, 3,389.5 km (`MARS_REFERENCE_RADIUS_M`). */
const MARS_SPHERE_M = 3_389_500;

/** Saturn at 1 bar: `SATURN_OBLATE_SPHEROID`'s a and c (Archinal et al. 2018), m. */
const SATURN_FIGURE: AtmosphereFixtureFigure = {
  equatorialRadiusM: 60_268e3,
  polarRadiusM: 54_364e3,
};

/** Earth's ground albedo in `earth` and `earth-ozone-slab`: 0.1 (`earth.ts`'s, Bruneton 2017's). */
const EARTH_GROUND_ALBEDO = 0.1;

/** Mars's ground albedo in `mars`: 0.25 (`reference/cases.ts`' `MARS_GROUND_ALBEDO`). */
const MARS_GROUND_ALBEDO = 0.25;

/** The Saturn-class ground in `saturn-oblate`: 0.3 (`SATURN_OBLATE_GROUND_ALBEDO`). */
const SATURN_GROUND_ALBEDO = 0.3;

/** Venus's sphere in `venus-92bar` and `venus-58bar`: 6,051.8 km (`thickCases.ts`'s `VENUS_RADIUS_M`). */
const VENUS_SPHERE_M = 6_051_800;

/** Titan's sphere in `titan-class`: 2,574.73 km (`TITAN_RADIUS_M`). */
const TITAN_SPHERE_M = 2_574_730;

/** The giant's sphere in `giant-deck`: Saturn's figure's (2a + c) ÷ 3, 58,300 km (`GIANT_RADIUS_M`). */
const GIANT_SPHERE_M = (2 * SATURN_FIGURE.equatorialRadiusM + SATURN_FIGURE.polarRadiusM) / 3;

/** The Venus-class ground: 0.15 (`VENUS_GROUND_ALBEDO`). */
const VENUS_GROUND_ALBEDO = 0.15;

/** The Titan-class ground: 0.3 (`TITAN_GROUND_ALBEDO`). */
const TITAN_GROUND_ALBEDO = 0.3;

/** The giant's ground under its deck: black (`GIANT_GROUND_ALBEDO`). */
const GIANT_GROUND_ALBEDO = 0;

/**
 * The hand Jupiter's floor, the figure its tables take (R08.T16.a.2): the level surface where τ(550)
 * from its top reaches 10 (`thick/deckRead.ts`'s `deckDatumOf` over its medium's datum, Jupiter's
 * 1-bar figure lowered to its column's base, R08.T16.a.1), on which its medium above the floor
 * stands while its route is closed and its deck's once it opens, m, restated from its assembly
 * (the test holds the two equal): some 181 km under Jupiter's 1-bar 71,492 km (Archinal et al.
 * 2018).
 */
const JUPITER_FLOOR: AtmosphereFixtureFigure = {
  equatorialRadiusM: 71_310_949.177_708_8,
  polarRadiusM: 66_697_132.634_374_596,
};

/** An envelope's floor, the hand Jupiter's ground: black, as its tables take it. */
const ENVELOPE_GROUND_ALBEDO = 0;

/** A sphere of `radiusM`. */
function sphere(radiusM: number): AtmosphereFixtureFigure {
  return { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
}

/** The air fixtures, in the order the target keys step through them after `TEST STAR`. */
export const ATMOSPHERE_FIXTURES: ReadonlyArray<AtmosphereFixture> = [
  {
    id: ATMOSPHERE_EARTH,
    designation: "TEST EARTH",
    caseName: "earth",
    centreM: ATMOSPHERE_EARTH_CENTRE_M,
    figure: sphere(EARTH_SPHERE_M),
    surfaceless: false,
    groundAlbedo: EARTH_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_EARTH_SLAB,
    designation: "TEST EARTH O3 SLAB",
    caseName: "earth-ozone-slab",
    centreM: legsOn(0),
    figure: sphere(EARTH_SPHERE_M),
    surfaceless: false,
    groundAlbedo: EARTH_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_MARS,
    designation: "TEST MARS",
    caseName: "mars",
    centreM: legsOn(1),
    figure: sphere(MARS_SPHERE_M),
    surfaceless: false,
    groundAlbedo: MARS_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_SATURN,
    designation: "TEST SATURN",
    caseName: "saturn-oblate",
    centreM: legsOn(2),
    figure: SATURN_FIGURE,
    surfaceless: true,
    groundAlbedo: SATURN_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_VENUS,
    designation: "TEST VENUS",
    caseName: "venus-92bar",
    centreM: legsOn(3),
    figure: sphere(VENUS_SPHERE_M),
    surfaceless: false,
    groundAlbedo: VENUS_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_VENUS_58,
    designation: "TEST VENUS 5.80 MPa",
    caseName: "venus-58bar",
    centreM: legsOn(4),
    figure: sphere(VENUS_SPHERE_M),
    surfaceless: false,
    groundAlbedo: VENUS_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_TITAN,
    designation: "TEST TITAN",
    caseName: "titan-class",
    centreM: legsOn(5),
    figure: sphere(TITAN_SPHERE_M),
    surfaceless: false,
    groundAlbedo: TITAN_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_GIANT_DECK,
    designation: "TEST SATURN NH3 ICE DECK",
    caseName: "giant-deck",
    centreM: legsOn(6),
    figure: sphere(GIANT_SPHERE_M),
    surfaceless: true,
    groundAlbedo: GIANT_GROUND_ALBEDO,
  },
  {
    id: ATMOSPHERE_JUPITER,
    designation: "TEST JUPITER",
    caseName: "hand-jupiter",
    centreM: legsOn(7),
    figure: JUPITER_FLOOR,
    surfaceless: true,
    groundAlbedo: ENVELOPE_GROUND_ALBEDO,
  },
];

/** The Moon's volumetric mean radius, m: 1,737.4 km (NASA Moon Fact Sheet). */
export const ATMOSPHERE_MOON_RADIUS_M = 1.7374e6;

/** `TEST MOON`'s distance from `TEST EARTH`'s centre, m: the Moon's semi-major axis, 384,400 km (NASA Moon Fact Sheet). */
export const ATMOSPHERE_MOON_DISTANCE_M = 3.844e8;

/**
 * How far `TEST MOON` stands from `TEST EARTH`'s anti-solar point, towards +z, rad: 3°, which
 * keeps it out of the penumbra (some 8,200 km in radius at its distance, about 1.5° with its own
 * radius) and leaves it full, within a few degrees of opposition.
 */
const MOON_OFF_OPPOSITION_RAD = (3 * Math.PI) / 180;

/** `TEST MOON`'s centre, m from the barycentre. */
export const ATMOSPHERE_MOON_CENTRE_M: Vec3 = add(
  ATMOSPHERE_EARTH_CENTRE_M,
  scale(
    vec3(Math.cos(MOON_OFF_OPPOSITION_RAD), 0, Math.sin(MOON_OFF_OPPOSITION_RAD)),
    ATMOSPHERE_MOON_DISTANCE_M,
  ),
);

/** The own ship, the seat camera's. */
export const ATMOSPHERE_SHIP = "atmosphere-test-ship";

/**
 * The ship's height over `TEST EARTH`'s ground, m: its origin's, 10 m, so that the hull, 20 m long
 * and pitched up some 47° at the moon, stands clear of the ground, its stern's lowest corner about
 * 1 m up (`TEST_HULL`'s stern at z = +10 m, its keel at y = −2.5 m); the seat's eye stands about
 * 14.6 m up.
 */
export const ATMOSPHERE_SHIP_HEIGHT_M = 10;

/**
 * The star's depression below the horizon at the ship, degrees: 45°, well beyond the measured
 * Earth's twilight limit at the ground (about 20°), so that the night is lit by the moon alone.
 */
export const ATMOSPHERE_SUN_DEPRESSION_DEG = 45;

const SUN_DEPRESSION_RAD = (ATMOSPHERE_SUN_DEPRESSION_DEG * Math.PI) / 180;

/**
 * The ship's ground point's outward normal: in the x–z plane, the star's depression from the
 * anti-solar point's side, so that the star (along −x) stands that far below the horizon and the
 * moon (near +x, a little towards +z) high above it.
 */
export const ATMOSPHERE_SHIP_UP: Vec3 = vec3(
  Math.sin(SUN_DEPRESSION_RAD),
  0,
  Math.cos(SUN_DEPRESSION_RAD),
);

/** The ship's place, body-fixed, m from `TEST EARTH`'s centre: its body-fixed axes are the galactic ones. */
export const ATMOSPHERE_SHIP_BODY_FIXED_M: Vec3 = scale(
  ATMOSPHERE_SHIP_UP,
  EARTH_SPHERE_M + ATMOSPHERE_SHIP_HEIGHT_M,
);

/**
 * The ship's attitude, hull axes to the galactic: its nose at `TEST MOON`'s centre, its dorsal
 * side as near the local vertical as that allows, so that the seat looking forward looks at it.
 */
export const ATMOSPHERE_SHIP_ATTITUDE: Quaternion = lookAlong(
  sub(ATMOSPHERE_MOON_CENTRE_M, add(ATMOSPHERE_EARTH_CENTRE_M, ATMOSPHERE_SHIP_BODY_FIXED_M)),
  ATMOSPHERE_SHIP_UP,
);

/** The seat's eye, m from the barycentre along the galactic axes: where the stage starts. */
export function atmosphereSeatM(): Vec3 {
  return add(
    add(ATMOSPHERE_EARTH_CENTRE_M, ATMOSPHERE_SHIP_BODY_FIXED_M),
    rotate(ATMOSPHERE_SHIP_ATTITUDE, TEST_HULL.eyePointM),
  );
}

/** The seat's pose looking forward, at `TEST MOON`: the stage's start. */
function seatPose(): CameraPose {
  return {
    frame: { kind: "craft", craft: ATMOSPHERE_SHIP },
    positionM: rotate(ATMOSPHERE_SHIP_ATTITUDE, TEST_HULL.eyePointM),
    orientation: ATMOSPHERE_SHIP_ATTITUDE,
  };
}

/** The pole every body's figure is drawn about: +z, none having a rotation. */
const POLE: Vec3 = vec3(0, 0, 1);

/** A Lambert photometry of geometric albedo `p` in every channel, its phase integral 3 ÷ 2. */
function lambertPhotometry(p: number): BodyPhotometry {
  const albedo = [p, p, p] as const;
  const phaseIntegral = [1.5, 1.5, 1.5] as const;
  return {
    geometricAlbedo: albedo,
    phaseIntegral,
    law: lawFor(albedo, phaseIntegral, "lambert"),
    bondRatioCheck: null,
    provenance: "provisional",
  };
}

/**
 * An air fixture's photometry: a Lambert sphere of its medium's ground albedo A, p = A ÷ 1.5 in
 * every channel (q = 3 ÷ 2), the ground the scene's media light and veil.
 */
function airPhotometry(fixture: AtmosphereFixture): BodyPhotometry {
  return lambertPhotometry(fixture.groundAlbedo / 1.5);
}

/**
 * `TEST MOON`'s photometry: a Lambert sphere of the Moon's visual geometric albedo, 0.12 (NASA Moon
 * Fact Sheet), which sets its brightness when full, the only phase the scene shows it at.
 */
const MOON_PHOTOMETRY = lambertPhotometry(0.12);

/** The cloud-free cases the scene's media are built from, as `reference/cases.ts` exports them. */
export interface AtmosphereFixtureCases {
  readonly EARTH_CASE: ReferenceCase;
  readonly EARTH_OZONE_SLAB_CASE: ReferenceCase;
  readonly MARS_CASE: ReferenceCase;
  readonly SATURN_OBLATE_CASE: ReferenceCase;
}

/** The cloud-free case of `name` among `cases`. */
function caseOf(cases: AtmosphereFixtureCases, name: AtmosphereCloudFreeCase): ReferenceCase {
  let found: ReferenceCase;
  switch (name) {
    case "earth":
      found = cases.EARTH_CASE;
      break;
    case "earth-ozone-slab":
      found = cases.EARTH_OZONE_SLAB_CASE;
      break;
    case "mars":
      found = cases.MARS_CASE;
      break;
    case "saturn-oblate":
      found = cases.SATURN_OBLATE_CASE;
      break;
  }
  return found;
}

/**
 * What a case's medium's per-planet tables are made from (`BuiltAtmosphere`): its medium on its
 * figure, a sphere of the case's ground radius with one slice and one band (`body` `null`), or its
 * spheroid with its gravity, on R05's `tableRadiusM` of it, (2a + c) ÷ 3, as the assembly puts a
 * server body's (`assemble.ts`'s `mediumTableRadiusM`); its curves the case's, light slot 0's, where
 * the medium has an absorber, and none where it has none; and its medium's sources, for its thick
 * bake (R08.T14.c), where the case has them.
 *
 * @throws Error for a case named otherwise than `expected`, whose medium would be another's.
 */
export function builtOfCase(
  referenceCase: FixtureCase,
  expected: AtmosphereFixtureCase,
): BuiltAtmosphere {
  if (referenceCase.name !== expected) {
    throw new Error(`ATMOSPHERE TEST takes case ${expected}, not ${referenceCase.name}`);
  }
  const { medium, figure, curves, sources } = referenceCase;
  const slots = curves.length === 0 ? [] : [curves];
  const withSources = sources === undefined ? {} : { sources };
  let built: BuiltAtmosphere;
  switch (figure.kind) {
    case "sphere":
      built = { medium, bottomRadiusM: figure.radiusM, curves: slots, body: null, ...withSources };
      break;
    case "spheroid": {
      const { equatorialRadiusM, polarRadiusM } = figure.gravity.spheroid;
      built = {
        medium,
        bottomRadiusM: (2 * equatorialRadiusM + polarRadiusM) / 3,
        curves: slots,
        body: { figure: { equatorialRadiusM, polarRadiusM }, gravity: figure.gravity },
        ...withSources,
      };
      break;
    }
  }
  return built;
}

/**
 * The figure a body's tables draw it on (`hillaire.ts`'s `tablesFigureOf`): their body's datum
 * spheroid, or for tables of one slice and one band, the sphere of their ground's radius.
 */
export function builtFigureOf(built: BuiltAtmosphere): AtmosphereFixtureFigure {
  return built.body?.figure ?? sphere(built.bottomRadiusM);
}

/** One air fixture's medium as the scene sets it, made once and held. */
export interface HeldMedium {
  /** The figure its tables draw it on ({@link builtFigureOf}), which the body is drawn on. */
  readonly figure: AtmosphereFixtureFigure;
  /** Its setting, `medium` with the built medium, the same object every frame. */
  readonly setting: KeptBodyAtmosphere;
}

/** One load's media, or the scene's whole: loading, built and held by body, or failed to load. */
export type AtmosphereFixtureMedia =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly media: ReadonlyMap<BodyIdHex, HeldMedium> }
  | { readonly kind: "failed" };

/** Each load's media, by load. */
export type AtmosphereFixtureLoads = Readonly<
  Record<AtmosphereFixtureLoad, AtmosphereFixtureMedia>
>;

/** No medium in force on a kept body: the setting before an import lands, and after one fails. */
const LOADING: KeptBodyAtmosphere = { kind: "loading" };
const FAILED: KeptBodyAtmosphere = { kind: "failed" };
const UNSET: KeptBodyAtmosphere = { kind: "unset" };
const AIRLESS: KeptBodyAtmosphere = { kind: "airless" };

/** A load's media before it lands, and its failure. */
const MEDIA_LOADING: AtmosphereFixtureMedia = { kind: "loading" };
const MEDIA_FAILED: AtmosphereFixtureMedia = { kind: "failed" };

/** A kept medium's labels: none of the scene's own; the cache adds the router's where it applies. */
const NO_LABELS: ReadonlyArray<never> = Object.freeze([]);

/** The held media of the air fixtures of `load`, each built from its case by `caseNamed`. */
function heldMedia(
  load: AtmosphereFixtureLoad,
  caseNamed: (name: AtmosphereFixtureCase) => FixtureCase,
): ReadonlyMap<BodyIdHex, HeldMedium> {
  const media = new Map<BodyIdHex, HeldMedium>();
  for (const fixture of ATMOSPHERE_FIXTURES) {
    if (fixtureLoadOf(fixture.caseName) !== load) {
      continue;
    }
    const built = builtOfCase(caseNamed(fixture.caseName), fixture.caseName);
    media.set(fixture.id, {
      figure: builtFigureOf(built),
      setting: { kind: "medium", built, labels: NO_LABELS },
    });
  }
  return media;
}

/** The cloud-free case of a fixture of the `cases` load. */
function cloudFreeCaseOf(cases: AtmosphereFixtureCases, name: AtmosphereFixtureCase): FixtureCase {
  let found: FixtureCase;
  switch (name) {
    case "earth":
    case "earth-ozone-slab":
    case "mars":
    case "saturn-oblate":
      found = caseOf(cases, name);
      break;
    case "venus-92bar":
    case "venus-58bar":
    case "titan-class":
    case "giant-deck":
      throw new Error(`ATMOSPHERE TEST loads case ${name} from the thick cases' worker`);
    case "hand-jupiter":
      throw new Error(`ATMOSPHERE TEST loads ${name}'s record, not a case`);
  }
  return found;
}

/** The thick case of a fixture of the `thick` load. */
function thickCaseOf(cases: ThickFixtureCases, name: AtmosphereFixtureCase): FixtureCase {
  let found: FixtureCase;
  switch (name) {
    case "venus-92bar":
    case "venus-58bar":
    case "titan-class":
    case "giant-deck":
      found = cases[name];
      break;
    case "earth":
    case "earth-ozone-slab":
    case "mars":
    case "saturn-oblate":
      throw new Error(`ATMOSPHERE TEST loads case ${name} from reference/cases.ts`);
    case "hand-jupiter":
      throw new Error(`ATMOSPHERE TEST loads ${name}'s record, not a case`);
  }
  return found;
}

/**
 * The held settings of the air fixtures of the `giant` load: each the `record` setting of `detail`,
 * on its fixture's restated datum, which its assembly's tables take.
 */
function heldRecords(detail: BodyDetail): ReadonlyMap<BodyIdHex, HeldMedium> {
  const media = new Map<BodyIdHex, HeldMedium>();
  for (const fixture of ATMOSPHERE_FIXTURES) {
    if (fixtureLoadOf(fixture.caseName) === "giant") {
      media.set(fixture.id, { figure: fixture.figure, setting: { kind: "record", detail } });
    }
  }
  return media;
}

/**
 * The scene's whole media from each load's: `failed` once any has failed, `ready` with every
 * fixture's once all have landed, and `loading` until then.
 */
function wholeMediaOf(loads: AtmosphereFixtureLoads): AtmosphereFixtureMedia {
  const { cases, thick, giant } = loads;
  if (cases.kind === "failed" || thick.kind === "failed" || giant.kind === "failed") {
    return MEDIA_FAILED;
  }
  if (cases.kind === "ready" && thick.kind === "ready" && giant.kind === "ready") {
    return {
      kind: "ready",
      media: new Map([...cases.media, ...thick.media, ...giant.media]),
    };
  }
  return MEDIA_LOADING;
}

/**
 * The scene's media, loaded once a page from the reference cases and the hand Jupiter's record in
 * three loads (the module's remarks): all started at the first {@link AtmosphereFixtures.ready},
 * each fixture's medium built, or its record set, once when its load lands and held, or its load's
 * failure logged once.
 */
export class AtmosphereFixtures {
  readonly #load: () => Promise<AtmosphereFixtureCases>;
  readonly #loadThick: () => Promise<ThickFixtureCases>;
  readonly #loadGiant: () => Promise<BodyDetail>;
  #loads: AtmosphereFixtureLoads = {
    cases: MEDIA_LOADING,
    thick: MEDIA_LOADING,
    giant: MEDIA_LOADING,
  };
  #media: AtmosphereFixtureMedia = MEDIA_LOADING;
  #ready: Promise<void> | null = null;

  /**
   * @param load - Loads the cloud-free cases: the lazy import of `reference/cases.ts` in the app.
   * @param loadThick - Loads the thick cases: their worker's in the app ({@link loadThickCases}).
   * @param loadGiant - Loads `TEST JUPITER`'s record: the lazy import of `test/giantFixtures.ts`'s
   *   hand Jupiter in the app ({@link loadHandJupiter}).
   */
  constructor(
    load: () => Promise<AtmosphereFixtureCases>,
    loadThick: () => Promise<ThickFixtureCases>,
    loadGiant: () => Promise<BodyDetail>,
  ) {
    this.#load = load;
    this.#loadThick = loadThick;
    this.#loadGiant = loadGiant;
  }

  /** The scene's whole media as they stand, which a run that waits for every fixture reads. */
  get media(): AtmosphereFixtureMedia {
    return this.#media;
  }

  /** Each load's media as they stand, the same object until that load lands or fails. */
  get loads(): AtmosphereFixtureLoads {
    return this.#loads;
  }

  /**
   * Starts every load where it has not started, and returns what settles once each has landed
   * or failed; it never rejects, a failure being its load's `failed`, held for the page: a dynamic
   * import that failed stays failed in it, as a build that threw would again
   * (`decision-r08-t10b-ux.md`, item 4).
   */
  ready(): Promise<void> {
    this.#ready ??= Promise.all([
      this.#landed("cases", async () => {
        const cases = await this.#load();
        return heldMedia("cases", (name) => cloudFreeCaseOf(cases, name));
      }),
      this.#landed("thick", async () => {
        const cases = await this.#loadThick();
        return heldMedia("thick", (name) => thickCaseOf(cases, name));
      }),
      this.#landed("giant", async () => heldRecords(await this.#loadGiant())),
    ]).then(() => undefined);
    return this.#ready;
  }

  /** Runs `load`'s build and holds its media, or holds and logs its failure. */
  async #landed(
    load: AtmosphereFixtureLoad,
    build: () => Promise<ReadonlyMap<BodyIdHex, HeldMedium>>,
  ): Promise<void> {
    let media: AtmosphereFixtureMedia;
    try {
      media = { kind: "ready", media: await build() };
    } catch (error: unknown) {
      console.error(`ATMOSPHERE TEST: the fixtures' media did not load (${load}):`, error);
      media = MEDIA_FAILED;
    }
    this.#loads = { ...this.#loads, [load]: media };
    this.#media = wholeMediaOf(this.#loads);
  }
}

/** The part of a `Worker` {@link loadThickCases} uses, so that tests can stand in for one. */
export interface ThickCasesWorkerLike {
  addEventListener(type: "message", listener: (event: MessageEvent<ThickCasesReply>) => void): void;
  /** An `ErrorEvent` when the script threw, a plain `Event` when the module failed to load. */
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  terminate(): void;
}

/** Starts the thick cases' worker on the renderer's own module, as the page's load does. */
export function startThickCasesWorker(): ThickCasesWorkerLike {
  return new Worker(new URL("../atmosphere/reference/thickCases.worker.ts", import.meta.url), {
    type: "module",
    name: "thick-cases",
  });
}

/**
 * The thick cases from one worker's one reply, the worker ended once it has answered or failed.
 *
 * @throws Error, as the promise's rejection, where the worker's build failed, its script did not
 *   load or run, or its reply could not be read.
 */
export function loadThickCases(
  start: () => ThickCasesWorkerLike = startThickCasesWorker,
): Promise<ThickFixtureCases> {
  return new Promise((resolve, reject) => {
    const worker = start();
    worker.addEventListener("message", (event) => {
      worker.terminate();
      const reply = event.data;
      switch (reply.kind) {
        case "built":
          resolve(reply.cases);
          break;
        case "failed":
          reject(new Error(`the thick cases' worker could not build them: ${reply.message}`));
          break;
      }
    });
    const failed = (event: Event): void => {
      worker.terminate();
      // An `ErrorEvent` says what threw; a module that failed to load says nothing.
      const said =
        "message" in event && typeof event.message === "string" ? `: ${event.message}` : "";
      reject(new Error(`the thick cases' worker failed (${event.type})${said}`));
    };
    worker.addEventListener("error", failed);
    worker.addEventListener("messageerror", failed);
  });
}

/**
 * `TEST JUPITER`'s record, the hand Jupiter's (`test/giantFixtures.ts`), from a lazy import: a test
 * fixture, which the console's other scenes never load, built from the protocol fixture's Jupiter
 * and the sim's envelope-profile golden (lean, 2026-11-01: the views check's hand Jupiter drawn
 * through a generated giant's path).
 */
export async function loadHandJupiter(): Promise<BodyDetail> {
  const { handJupiterRecord } = await import("../../test/giantFixtures");
  return handJupiterRecord();
}

/**
 * The page's media, from the lazy imports of `reference/cases.ts` and the hand Jupiter and the
 * thick cases' worker, shared by every scene made.
 */
const PAGE_FIXTURES = new AtmosphereFixtures(
  () => import("../atmosphere/reference/cases"),
  () => loadThickCases(),
  loadHandJupiter,
);

/**
 * Starts the loading of `ATMOSPHERE TEST`'s media on the page where it has not started (its
 * `SceneOption`'s `prepare`): called from `VIEW`'s effect when the scene is chosen, never as the
 * scene is made.
 */
export function prepareAtmosphereScene(fixtures: AtmosphereFixtures = PAGE_FIXTURES): void {
  fixtures.ready().catch((error: unknown) => {
    // `ready` holds a failure as its load's `failed`; anything else reaching here is a bug.
    console.error("ATMOSPHERE TEST: the fixtures' media could not be started:", error);
  });
}

/**
 * The page's fixtures' media (`ATMOSPHERE TEST`'s as `SCENE_OPTIONS` makes it), for a run that
 * waits for them: the views check's atmosphere run.
 */
export function pageAtmosphereFixtures(): AtmosphereFixtures {
  return PAGE_FIXTURES;
}

/** The scene's star field's seed. */
const STAR_SEED = 0x0a7e_5ce1;

/** The scene's stars. */
export const ATMOSPHERE_STAR_COUNT = 500;

/** The faintest and the brightest of the scene's stars, V. */
export const ATMOSPHERE_STAR_MAGNITUDES = { brightest: 0, faintest: 6 } as const;

/**
 * Mulberry32: a small, fast 32-bit generator of uniform numbers in [0, 1), here for the scene's
 * fixed test field alone, which belongs to no universe and takes none of the sim's streams.
 */
function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let mixed = state;
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1);
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61);
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/**
 * The stars' temperatures, K, in turn, for their colours: about an M2.5, K4, G1, F1 and A0 dwarf's
 * (Pecaut & Mamajek 2013, ApJS 208, 9, Table 5, Mamajek's update of 2022.04.16: 3,470, 4,600,
 * 5,860, 7,020 and 9,700 K).
 */
const STAR_TEMPERATURES_K = [3_500, 4_500, 5_800, 7_000, 9_500] as const;

/**
 * The scene's own star field: {@link ATMOSPHERE_STAR_COUNT} stars spread uniformly over the sphere,
 * their apparent V uniform from {@link ATMOSPHERE_STAR_MAGNITUDES}' brightest to its faintest, each
 * at 10 pc so that its absolute V is its apparent, from a fixed seed: a test pattern for the sky's
 * washing-out of faint stars, as even a ladder of magnitudes as it can be, not the real sky's
 * counts.
 */
export const ATMOSPHERE_STARS: ReadonlyArray<ViewStar> = Object.freeze(
  (() => {
    const random = mulberry32(STAR_SEED);
    const { brightest, faintest } = ATMOSPHERE_STAR_MAGNITUDES;
    return Array.from({ length: ATMOSPHERE_STAR_COUNT }, (_, index): ViewStar => {
      const z = 2 * random() - 1;
      const azimuthRad = 2 * Math.PI * random();
      const across = Math.sqrt(Math.max(0, 1 - z * z));
      return Object.freeze({
        id: `02000800300${index.toString(16).padStart(5, "0")}`,
        direction: vec3(across * Math.cos(azimuthRad), across * Math.sin(azimuthRad), z),
        distanceM: 10 * PARSEC_M,
        absoluteV: brightest + (faintest - brightest) * random(),
        tEffK: STAR_TEMPERATURES_K[index % STAR_TEMPERATURES_K.length] ?? null,
      });
    });
  })(),
);

/**
 * The scene's host disc, a Sun at the star, made once and held by every frame; frozen, since every
 * frame shares it.
 */
const ATMOSPHERE_HOST_DISCS: ReadonlyArray<HostDiscDto> = Object.freeze([
  frozenHostDisc(sunLikeHostDisc({ star: 0 })),
]);

/** An air fixture's body under its load's media as they stand. */
function airBody(fixture: AtmosphereFixture, media: AtmosphereFixtureMedia): ViewBody {
  let held: HeldMedium | undefined;
  let setting: KeptBodyAtmosphere;
  switch (media.kind) {
    case "loading":
      setting = LOADING;
      break;
    case "failed":
      setting = FAILED;
      break;
    case "ready":
      held = media.media.get(fixture.id);
      setting = held?.setting ?? UNSET;
      break;
  }
  const figure = held?.figure ?? fixture.figure;
  const appearance: WireAppearance = {
    photometry: airPhotometry(fixture),
    figure: { ...figure, pole: POLE },
    // A hand grey Lambert sphere, provisional, as `PHASE TEST`'s bodies are.
    labels: PROVISIONAL_LABELS,
    bondRatioFinding: null,
  };
  return {
    id: fixture.id,
    parent: null,
    kind: "planet",
    designation: fixture.designation,
    radiusM: figure.equatorialRadiusM,
    hillRadiusM: null,
    centreM: fixture.centreM,
    retarded: staticRetarded(fixture.centreM),
    rotation: null,
    appearance,
    symbol: bodyKindSymbol("planet"),
    orbitNormal: null,
    ...(fixture.surfaceless ? { surfaceless: true as const } : {}),
    atmosphere: setting,
  };
}

/** The scene's bodies under the loads as they stand: the star, the air fixtures and the moon. */
function bodiesOf(loads: AtmosphereFixtureLoads): ReadonlyArray<ViewBody> {
  const starCentreM = vec3(0, 0, 0);
  return [
    {
      id: ATMOSPHERE_STAR,
      parent: null,
      kind: "star",
      designation: "TEST STAR",
      radiusM: SUN_RADIUS_M,
      hillRadiusM: null,
      centreM: starCentreM,
      retarded: staticRetarded(starCentreM),
      rotation: null,
      appearance: null,
      symbol: bodyKindSymbol("star"),
      orbitNormal: null,
    },
    ...ATMOSPHERE_FIXTURES.map((fixture) =>
      airBody(fixture, loads[fixtureLoadOf(fixture.caseName)]),
    ),
    {
      id: ATMOSPHERE_MOON,
      parent: ATMOSPHERE_EARTH,
      kind: "moon",
      designation: "TEST MOON",
      radiusM: ATMOSPHERE_MOON_RADIUS_M,
      hillRadiusM: null,
      centreM: ATMOSPHERE_MOON_CENTRE_M,
      retarded: staticRetarded(ATMOSPHERE_MOON_CENTRE_M),
      rotation: null,
      appearance: {
        photometry: MOON_PHOTOMETRY,
        figure: {
          equatorialRadiusM: ATMOSPHERE_MOON_RADIUS_M,
          polarRadiusM: ATMOSPHERE_MOON_RADIUS_M,
          pole: POLE,
        },
        labels: PROVISIONAL_LABELS,
        bondRatioFinding: null,
      },
      symbol: bodyKindSymbol("moon"),
      orbitNormal: null,
      atmosphere: AIRLESS,
    },
  ];
}

/**
 * The atmosphere test scene: the air fixtures and the moonlit night, the seat camera on the own
 * ship standing on `TEST EARTH`'s night side. Making it starts nothing: its media load from
 * {@link prepareAtmosphereScene}, which `VIEW` calls from an effect when it chooses the scene.
 *
 * @param fixtures - Where its media come from: the page's, from the lazy import of the reference
 *   cases, unless a test gives its own.
 */
export function atmosphereScene(fixtures: AtmosphereFixtures = PAGE_FIXTURES): KeptScene {
  // The bodies made again only when a load lands or fails, so that a frame's settings and figures
  // are the ones the frame before held.
  let shown: { readonly loads: AtmosphereFixtureLoads; readonly bodies: ReadonlyArray<ViewBody> } =
    { loads: fixtures.loads, bodies: bodiesOf(fixtures.loads) };
  const sceneAt = (tS: number): ViewScene => {
    if (shown.loads !== fixtures.loads) {
      shown = { loads: fixtures.loads, bodies: bodiesOf(fixtures.loads) };
    }
    return {
      provenance: { kind: "kept", name: ATMOSPHERE_SCENE_NAME },
      time: keptTime(tS),
      timeRate: 1,
      system: KEPT_SYSTEM,
      barycentre: KEPT_BARYCENTRE,
      tidalRadiusM: KEPT_TIDAL_RADIUS_M,
      bodies: shown.bodies,
      rings: [],
      orbits: [],
      craft: [
        {
          id: ATMOSPHERE_SHIP,
          designation: "TEST HULL",
          hull: TEST_HULL,
          pose: {
            position: {
              kind: "body_fixed",
              body: ATMOSPHERE_EARTH,
              m: ATMOSPHERE_SHIP_BODY_FIXED_M,
            },
            attitude: ATMOSPHERE_SHIP_ATTITUDE,
          },
          predictedPath: null,
          velocityMPerS: vec3(0, 0, 0),
        },
      ],
      stars: ATMOSPHERE_STARS,
      ownShip: ATMOSPHERE_SHIP,
      defaultPose: seatPose(),
      hostDiscs: ATMOSPHERE_HOST_DISCS,
    };
  };
  return {
    name: ATMOSPHERE_SCENE_NAME,
    durationS: ATMOSPHERE_DURATION_S,
    sceneAt,
    cameraAt: seatPose,
  };
}

/**
 * The phase angle at a body centred at `centreM` between the star (at the barycentre) and a camera
 * at `cameraM`, deg: 0 full, 180 new.
 */
export function phaseAngleDeg(centreM: Vec3, cameraM: Vec3): number {
  const toStar = normalise(scale(centreM, -1));
  const toCamera = normalise(sub(cameraM, centreM));
  return (Math.acos(Math.min(1, Math.max(-1, dot(toStar, toCamera)))) * 180) / Math.PI;
}

/**
 * The kept terrain test scene, `TERRAIN TEST` (plan R10, R10.T16.d; decision-r10-live-path D11;
 * decision-r10-t16-contracts §5): a descent from beyond the hand-over range to a few kilometres
 * over a surveyed Moon-like world, across its survey's edge into a sunset, its field served by a
 * fixture in the server's place.
 *
 * @remarks
 * - **Its bodies.** A Sun-like star, `TEST STAR`, at the barycentre, and `TEST MOON`, the airless
 *   Moon-like synthetic world of R09's `testing` worlds, at rest 1 au out along +x: its body ID,
 *   detail seed and figure are the terrain golden's (`terrain_test.golden`, {@link terrainTestGolden}),
 *   never restated here, so that a re-bless cannot pull them apart. Both sit in system
 *   `5e5e000000000000`, the synthetic worlds', which no galaxy places, `TEST MOON` its `.0003` (its
 *   world's own body); the star is `.0000`, an index no synthetic world uses.
 * - **Its rotation** is hand-set: one turn in {@link TERRAIN_ROTATION_PERIOD_S} about a pole tilted
 *   {@link TERRAIN_OBLIQUITY_RAD} from +z, short, so that the sun sets over the survey within the
 *   script at a clock of {@link TERRAIN_TIME_RATE} s/s.
 * - **Its photometry** is its field's palette's own: its primary crust's normal albedo under the
 *   Regolith law, R10's class law of that crust ({@link terrainMoonPhotometry}), so that its disc
 *   and figure over unsurveyed ground stand where its terrain's highlands do.
 * - **Its two fields** ({@link ViewsCheckTerrainFixture}): `TERRAIN TEST`'s own golden by default,
 *   and `TEST MOON` weathered's (`terrain_test_weathered.golden`; plan R10, R10.T1.c's client half;
 *   decision-r10-t1c-client-scope §3), the same moon, ID, seed and figure on a 1 Gyr surface whose
 *   young craters' haloes the maturity draws, which a `--terrain --weathered` check launch serves in
 *   its place. Every function here takes the fixture, `terrain-test` unless given, so that the
 *   scene, its spike and every launch but that one are as they were.
 * - **Its script.** The own ship, `TEST HULL`, whose seat the camera takes, flies a scripted path
 *   in `TEST MOON`'s body-fixed axes: from {@link TERRAIN_START_ALTITUDE_M} (beyond the hand-over
 *   range) down over the survey, on a smoothstep in the logarithm of its height over the datum, to
 *   {@link TERRAIN_LOWEST_ALTITUDE_M} at {@link TERRAIN_TEST_SCRIPT}'s `descentEndS`; then west
 *   along the parallel at that height, out of the survey across its edge, while the sun sets over
 *   the surveyed ground behind and beside it. Its seat looks down at the ground from high up and
 *   along its track, a little below the horizon, near the ground. The script loops, the scene's
 *   clock with it; the field's revision rises at `deltaAtS` and never falls
 *   ({@link terrainTestFixture}).
 * - **No contact.** The ship's lowest point is {@link TERRAIN_LOWEST_ALTITUDE_M} over the datum and
 *   its time to contact never below several hundred seconds, so it is never one of R05's contacts
 *   (Design note 9: under 1 km and closing, or under 30 s from contact): no patch is forced.
 */
import type { BodyIdHex, HostDiscDto, SurfaceHeaderDto, SystemIdHex } from "@hyperion/protocol";

import terrainGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test.golden?raw";
import weatheredGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test_weathered.golden?raw";
import type { ViewsCheckTerrainFixture } from "../../../../preload/api";
import { add, cross, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { BodyPhotometry, WireAppearance } from "../appearance/fromWire";
import type { PhotometricLaw } from "../appearance/law";
import { geometricAlbedo, lawPhaseIntegral } from "../appearance/phase";
import type { CameraPose, Quaternion } from "../camera/pose";
import { IDENTITY_QUATERNION, lookAlong, rotate } from "../camera/quaternion";
import { type Rotation3, rotateToBody, rotation3FromRows } from "../coords/rotation";
import { frozenHostDisc, SUN_RADIUS_M, sunLikeHostDisc } from "../lighting/hostDisc";
import { TEST_HULL } from "../scene/hull";
import { bodyKindSymbol, staticRetarded, type ViewBody, type ViewScene } from "../scene/model";
import { classRowLaw, PHASE_LAW } from "../terrain/terrainMaterial";
import { AU_M, KEPT_BARYCENTRE, KEPT_TIDAL_RADIUS_M, type KeptScene, keptTime } from "./kept";
import { terrainTestFixture, type TerrainTestFixture } from "./terrainFixture";
import {
  parseTerrainGolden,
  type TerrainGolden,
  type TerrainGoldenMaturity,
} from "./terrainGolden";

/** The terrain test scene's name, as the `SCENE` selector shows it. */
export const TERRAIN_SCENE_NAME = "TERRAIN TEST";

/** The scene's seconds per second of the script: the clock runs sixty times fast. */
export const TERRAIN_TIME_RATE = 60;

/**
 * The script's times, s of the script: its length; the end of the descent, where the ship reaches
 * its lowest point; the field's revision rise, the second survey's arrival, while the survey is in
 * view from about 50 km; and the sunset at the ship's ground point, over the surveyed ground, a
 * little before the ship crosses the survey's western edge (a test holds both).
 */
export const TERRAIN_TEST_SCRIPT = {
  durationS: 300,
  descentEndS: 180,
  deltaAtS: 120,
  sunsetS: 240,
} as const;

/** The ship's height over `TEST MOON`'s datum at the script's start, m: beyond the hand-over range. */
export const TERRAIN_START_ALTITUDE_M = 8e7;

/** The ship's height over the datum at its lowest, m: from the descent's end to the script's. */
export const TERRAIN_LOWEST_ALTITUDE_M = 4_000;

/** `TEST MOON`'s rotation period, s: a hand-set day, short, so that the sun moves within the script. */
export const TERRAIN_ROTATION_PERIOD_S = 86_400;

/** The tilt of `TEST MOON`'s pole from +z, rad: a hand-set 5°, about +x. */
export const TERRAIN_OBLIQUITY_RAD = (5 * Math.PI) / 180;

/**
 * The prime meridian's angle at the script's start, rad: hand-set, so that the sun sets at the
 * ship's ground point at {@link TERRAIN_TEST_SCRIPT}'s `sunsetS`.
 */
const MERIDIAN_AT_START_RAD = (166.5 * Math.PI) / 180;

/** The latitude of the ship's ground track, rad: 28° north, through the survey's middle rows. */
const TRACK_LATITUDE_RAD = (28 * Math.PI) / 180;

/**
 * The ship's ground longitude during the descent, rad: 47° east, inside the survey, which spans
 * about 42° to 48° on the track, astride the edge of the cube's faces +x and +y at 45°.
 */
const DESCENT_LONGITUDE_RAD = (47 * Math.PI) / 180;

/** The ship's ground longitude at the script's end, rad: 40° east, west of the survey. */
const TRACK_END_LONGITUDE_RAD = (40 * Math.PI) / 180;

/** Above this height the seat looks straight down, m. */
const NADIR_ABOVE_M = 2e6;

/** Below this height the seat looks along the track at {@link LOW_PITCH_RAD}, m. */
const LOW_LOOK_BELOW_M = 2e4;

/** How far below the local horizon the seat looks near the ground, rad: 12°. */
const LOW_PITCH_RAD = (12 * Math.PI) / 180;

/**
 * The Sun's GM, m³/s²: IAU 2015 Resolution B3's nominal (GM)ᴺ☉ (Prša et al. 2016, AJ 152, 41), as
 * `ECLIPSE TEST`'s.
 */
const GM_SUN_M3_S2 = 1.327_124_4e20;

/**
 * The Moon's GM, m³/s²: 4,902.800066 km³/s² (DE430; Folkner et al. 2014, IPN Progress Report
 * 42-196, Table 8).
 */
const GM_MOON_M3_S2 = 4.902_800_066e12;

/**
 * `TEST MOON`'s Hill radius about the star at 1 au, m: a (m ÷ 3M)^⅓ (Murray and Dermott 1999,
 * _Solar System Dynamics_, §3.5: the distance of L₁ and L₂ from the smaller mass, (μ₂ ÷ 3)^⅓ of
 * the separation for small μ₂), with the Moon's mass, 3.45 × 10⁸ m; the camera's frame is the
 * moon's throughout the script.
 */
export const TERRAIN_MOON_HILL_M = AU_M * Math.cbrt(GM_MOON_M3_S2 / (3 * GM_SUN_M3_S2));

/** `TEST MOON`'s centre, m from the barycentre: 1 au along +x, at rest. */
export const TERRAIN_MOON_CENTRE_M: Vec3 = vec3(AU_M, 0, 0);

/** The universe the fixture answers for, which no server holds. */
export const TERRAIN_TEST_UNIVERSE = "5e5e000000000000";

/** The own ship, the seat camera's. */
export const TERRAIN_TEST_SHIP = "test-ship";

/**
 * The particulate rule's onset, the normal albedo below which a particulate law's L(0) is 1:
 * `hyperion_surface`'s `LUNAR_LIMB_ONSET_NORMAL_ALBEDO` (R10.T10.a; decision-r10-limb), which a
 * test holds it to (decision-r10-t16d-part1, Q5).
 */
export const LIMB_ONSET_NORMAL_ALBEDO = 0.6;

/** Each fixture's golden: its text and its file's name, which the reader's refusals give. */
const FIXTURE_GOLDENS: Readonly<
  Record<ViewsCheckTerrainFixture, { readonly text: string; readonly file: string }>
> = {
  "terrain-test": { text: terrainGoldenText, file: "terrain_test.golden" },
  "terrain-test-weathered": { text: weatheredGoldenText, file: "terrain_test_weathered.golden" },
};

/** Each fixture's golden as read, once a page. */
const parsed = new Map<ViewsCheckTerrainFixture, TerrainGolden>();

/**
 * A fixture's terrain golden as read, once a page: `TEST MOON`'s ID, seed, figure, palette and
 * field; `TERRAIN TEST`'s own unless `fixture` names `TEST MOON` weathered's.
 */
export function terrainTestGolden(
  fixture: ViewsCheckTerrainFixture = "terrain-test",
): TerrainGolden {
  const known = parsed.get(fixture);
  if (known !== undefined) {
    return known;
  }
  const { text, file } = FIXTURE_GOLDENS[fixture];
  const golden = parseTerrainGolden(text, file);
  parsed.set(fixture, golden);
  return golden;
}

/** `TEST MOON`: the fixture's golden's body. */
export function terrainMoon(fixture: ViewsCheckTerrainFixture = "terrain-test"): BodyIdHex {
  return terrainTestGolden(fixture).body;
}

/** The scene's system: `TEST MOON`'s. */
export function terrainSystem(fixture: ViewsCheckTerrainFixture = "terrain-test"): SystemIdHex {
  const moon = terrainMoon(fixture);
  const [system] = moon.split(".");
  if (system === undefined) {
    throw new Error(`${moon} is not a body ID`);
  }
  return system;
}

/** `TEST STAR`, the system's body `.0000`, which no synthetic world is. */
export function terrainStar(fixture: ViewsCheckTerrainFixture = "terrain-test"): BodyIdHex {
  return `${terrainSystem(fixture)}.0000`;
}

/**
 * `TEST MOON`'s photometry from its field's palette (`header`): its primary crust's normal albedo
 * in B, V and R as the display's r, g and b take R, V and B, under R10's Regolith class law at
 * L(0) = 1 (`classRowLaw`: the Moon's template at s = 1), the law R10's workers give that crust's
 * regolith; p is then A, since L(0) = 1 (R10.T10.a's p = A (2 + L(0)) ÷ 3).
 *
 * @remarks
 * A field whose golden carries its maturity's lines (`maturity`, `TEST MOON` weathered's; R10.T1.c's
 * client half) takes A from their `crust_albedo` line instead, its B, V and R as r, g and b take
 * R, V and B, under the same law: the primary crust's regolith's drawn band-0 albedo at the body's
 * background maturity, T₀(m_body), its maturity nodes' last, as the worker draws it under the
 * header's albedo scale (the haloed scale, solved with the haloes' excess) and its clamps. The
 * palette's own albedo is not what that field draws: its header scales it, with clamps this does not
 * model, and its maturity moves the drawn albedo off it. So the disc and the figure over unsurveyed
 * ground, which the class map leaves to this law (`elsewhere`), stand where the terrain's mature
 * highlands do, as the scale's solve and the draw agree. `TERRAIN TEST`'s field, with neither
 * scale nor maturity, takes the palette's albedo as it always has, to the bit.
 *
 * @throws Error for a palette with no primary crust, a crust at or above the particulate rule's
 *   onset (where L(0) falls below 1), or a header with an albedo scale and no `maturity` to give
 *   the drawn albedo, whose clamps this does not model.
 */
export function terrainMoonPhotometry(
  header: SurfaceHeaderDto,
  maturity?: TerrainGoldenMaturity,
): BodyPhotometry {
  const crust = header.palette.find((entry) => entry.role === "primary_crust");
  if (crust === undefined) {
    throw new Error("the terrain test's palette names no primary crust");
  }
  if (header.albedo_scale !== null && maturity === undefined) {
    throw new Error(
      "the terrain test's photometry takes a field with no albedo scale, or its golden's crust_albedo",
    );
  }
  const [b, v, r] = maturity?.crustAlbedoBvr ?? crust.normal_albedo_bvr;
  if (!(v < LIMB_ONSET_NORMAL_ALBEDO)) {
    throw new Error(`the terrain test's crust, A_N ${String(v)}, is not below the onset`);
  }
  const law: PhotometricLaw = { ...classRowLaw(PHASE_LAW.regolith, 1), a: [r, v, b] };
  return {
    geometricAlbedo: geometricAlbedo(law),
    phaseIntegral: lawPhaseIntegral(law),
    law,
    bondRatioCheck: null,
    provenance: "modelled",
  };
}

/** `TEST MOON`'s photometry on each fixture's field, made once a page. */
const moonPhotometry = new Map<ViewsCheckTerrainFixture, BodyPhotometry>();

/** `TEST MOON`'s photometry on `fixture`'s field ({@link terrainMoonPhotometry}), made once a page. */
function moonPhotometryOf(fixture: ViewsCheckTerrainFixture): BodyPhotometry {
  const known = moonPhotometry.get(fixture);
  if (known !== undefined) {
    return known;
  }
  const { header, maturity } = terrainTestGolden(fixture);
  const photometry = terrainMoonPhotometry(header, maturity);
  moonPhotometry.set(fixture, photometry);
  return photometry;
}

/** `TEST MOON`'s rotation from body-fixed to body axes at scene time `sceneS`. */
export function terrainMoonRotationAt(sceneS: number): Rotation3 {
  const theta = MERIDIAN_AT_START_RAD + (2 * Math.PI * sceneS) / TERRAIN_ROTATION_PERIOD_S;
  const c = Math.cos(theta);
  const s = Math.sin(theta);
  const ce = Math.cos(TERRAIN_OBLIQUITY_RAD);
  const se = Math.sin(TERRAIN_OBLIQUITY_RAD);
  // Tilt = [[1, 0, 0], [0, ce, −se], [0, se, ce]]; Spin = [[c, −s, 0], [s, c, 0], [0, 0, 1]].
  return rotation3FromRows([vec3(c, -s, 0), vec3(ce * s, ce * c, -se), vec3(se * s, se * c, ce)]);
}

/** `TEST MOON`'s pole, the body-fixed +z, along the galactic axes. */
export const TERRAIN_MOON_POLE: Vec3 = vec3(
  0,
  -Math.sin(TERRAIN_OBLIQUITY_RAD),
  Math.cos(TERRAIN_OBLIQUITY_RAD),
);

/** The smoothstep 3u² − 2u³ of `u` clamped to [0, 1]. */
function smoothstep(u: number): number {
  const t = Math.min(Math.max(u, 0), 1);
  return t * t * (3 - 2 * t);
}

/** The ship's height over the datum at script time `tS`, m. */
export function terrainShipAltitudeM(tS: number): number {
  const s = smoothstep(tS / TERRAIN_TEST_SCRIPT.descentEndS);
  return TERRAIN_START_ALTITUDE_M * (TERRAIN_LOWEST_ALTITUDE_M / TERRAIN_START_ALTITUDE_M) ** s;
}

/** The ship's ground point at script time `tS`: its latitude and east longitude, rad. */
export function terrainShipGround(tS: number): {
  readonly latRad: number;
  readonly lonRad: number;
} {
  const traverse =
    (tS - TERRAIN_TEST_SCRIPT.descentEndS) /
    (TERRAIN_TEST_SCRIPT.durationS - TERRAIN_TEST_SCRIPT.descentEndS);
  const lonRad =
    DESCENT_LONGITUDE_RAD +
    (TRACK_END_LONGITUDE_RAD - DESCENT_LONGITUDE_RAD) * smoothstep(traverse);
  return { latRad: TRACK_LATITUDE_RAD, lonRad };
}

/** The unit direction of latitude `latRad` and east longitude `lonRad`, body-fixed. */
function directionOf(latRad: number, lonRad: number): Vec3 {
  return vec3(
    Math.cos(latRad) * Math.cos(lonRad),
    Math.cos(latRad) * Math.sin(lonRad),
    Math.sin(latRad),
  );
}

/** The datum's radius along the unit direction `n` of the spheroid `a`, `c`, m. */
function datumRadiusM(n: Vec3, aM: number, cM: number): number {
  return 1 / Math.sqrt((n.x * n.x + n.y * n.y) / (aM * aM) + (n.z * n.z) / (cM * cM));
}

/** The ship's place, body-fixed, m from `TEST MOON`'s centre, at script time `tS`. */
export function terrainShipBodyFixedM(
  tS: number,
  fixture: ViewsCheckTerrainFixture = "terrain-test",
): Vec3 {
  const { header } = terrainTestGolden(fixture);
  const { latRad, lonRad } = terrainShipGround(tS);
  const n = directionOf(latRad, lonRad);
  const radiusM = datumRadiusM(n, header.equatorial_radius_m, header.polar_radius_m);
  return scale(n, radiusM + terrainShipAltitudeM(tS));
}

/** The span of script time over which the ship's velocity is a central difference, s. */
const VELOCITY_STEP_S = 0.05;

/**
 * The ship's velocity against `TEST MOON`'s centre at script time `tS`, m per second of the scene,
 * along the galactic axes: its body-fixed motion's, by a central difference over
 * {@link VELOCITY_STEP_S} of the script, turned by the rotation, plus the rotation's own ω × r.
 */
function shipVelocityMPerS(
  tS: number,
  rotation: Rotation3,
  fixture: ViewsCheckTerrainFixture,
): Vec3 {
  const before = Math.max(tS - VELOCITY_STEP_S, 0);
  const after = Math.min(tS + VELOCITY_STEP_S, TERRAIN_TEST_SCRIPT.durationS);
  const sceneSpanS = (after - before) * TERRAIN_TIME_RATE;
  const fixed = scale(
    sub(terrainShipBodyFixedM(after, fixture), terrainShipBodyFixedM(before, fixture)),
    1 / sceneSpanS,
  );
  const omega = scale(TERRAIN_MOON_POLE, (2 * Math.PI) / TERRAIN_ROTATION_PERIOD_S);
  return add(
    rotateToBody(rotation, fixed),
    cross(omega, rotateToBody(rotation, terrainShipBodyFixedM(tS, fixture))),
  );
}

/**
 * The ship's attitude at script time `tS`, hull axes to the galactic: its seat looking down at the
 * ground from {@link NADIR_ABOVE_M} up, and west along the track {@link LOW_PITCH_RAD} below the
 * horizon from {@link LOW_LOOK_BELOW_M} down, its pitch between on the logarithm of its height.
 */
function shipAttitude(tS: number, rotation: Rotation3): Quaternion {
  const { latRad, lonRad } = terrainShipGround(tS);
  const radial = directionOf(latRad, lonRad);
  const west = vec3(Math.sin(lonRad), -Math.cos(lonRad), 0);
  const h = terrainShipAltitudeM(tS);
  const along =
    Math.log(Math.min(Math.max(h, LOW_LOOK_BELOW_M), NADIR_ABOVE_M) / LOW_LOOK_BELOW_M) /
    Math.log(NADIR_ABOVE_M / LOW_LOOK_BELOW_M);
  const pitch = LOW_PITCH_RAD + (Math.PI / 2 - LOW_PITCH_RAD) * along;
  const forward = sub(scale(west, Math.cos(pitch)), scale(radial, Math.sin(pitch)));
  const up = add(scale(west, Math.sin(pitch)), scale(radial, Math.cos(pitch)));
  return lookAlong(rotateToBody(rotation, forward), rotateToBody(rotation, normalise(up)));
}

/**
 * The scene's host disc, a Sun at the star, made once and held by every frame; frozen, since every
 * frame shares it.
 */
export const TERRAIN_HOST_DISCS: ReadonlyArray<HostDiscDto> = Object.freeze([
  frozenHostDisc(sunLikeHostDisc({ star: 0 })),
]);

/**
 * The scene at script time `tS`, its clock {@link TERRAIN_TIME_RATE} times the script's, on
 * `fixture`'s field.
 */
export function terrainSceneAt(
  tS: number,
  fixture: ViewsCheckTerrainFixture = "terrain-test",
): ViewScene {
  const golden = terrainTestGolden(fixture);
  const { header } = golden;
  const sceneS = TERRAIN_TIME_RATE * tS;
  const rotation = terrainMoonRotationAt(sceneS);
  const appearance: WireAppearance = {
    photometry: moonPhotometryOf(fixture),
    figure: {
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
      pole: TERRAIN_MOON_POLE,
    },
    labels: [],
    bondRatioFinding: null,
  };
  const star = terrainStar(fixture);
  const moon = golden.body;
  const bodies: ViewBody[] = [
    {
      id: star,
      parent: null,
      kind: "star",
      designation: "TEST STAR",
      radiusM: SUN_RADIUS_M,
      hillRadiusM: null,
      centreM: vec3(0, 0, 0),
      retarded: staticRetarded(vec3(0, 0, 0)),
      rotation: null,
      appearance: null,
      symbol: bodyKindSymbol("star"),
      orbitNormal: null,
    },
    {
      id: moon,
      parent: star,
      kind: "moon",
      designation: "TEST MOON",
      radiusM: header.equatorial_radius_m,
      hillRadiusM: TERRAIN_MOON_HILL_M,
      centreM: TERRAIN_MOON_CENTRE_M,
      retarded: staticRetarded(TERRAIN_MOON_CENTRE_M),
      rotation,
      appearance,
      symbol: bodyKindSymbol("moon"),
      orbitNormal: null,
      detailSeed: golden.seed,
    },
  ];
  return {
    provenance: { kind: "kept", name: TERRAIN_SCENE_NAME },
    time: keptTime(sceneS),
    timeRate: TERRAIN_TIME_RATE,
    system: terrainSystem(fixture),
    barycentre: KEPT_BARYCENTRE,
    tidalRadiusM: KEPT_TIDAL_RADIUS_M,
    bodies,
    rings: [],
    orbits: [],
    craft: [
      {
        id: TERRAIN_TEST_SHIP,
        designation: "TEST HULL",
        hull: TEST_HULL,
        pose: {
          position: { kind: "body_fixed", body: moon, m: terrainShipBodyFixedM(tS, fixture) },
          attitude: shipAttitude(tS, rotation),
        },
        predictedPath: null,
        velocityMPerS: shipVelocityMPerS(tS, rotation, fixture),
      },
    ],
    stars: [],
    ownShip: TERRAIN_TEST_SHIP,
    defaultPose: {
      frame: { kind: "craft", craft: TERRAIN_TEST_SHIP },
      positionM: TEST_HULL.eyePointM,
      orientation: IDENTITY_QUATERNION,
    },
    hostDiscs: TERRAIN_HOST_DISCS,
  };
}

/**
 * The terrain test scene: the scripted descent onto `TEST MOON`'s survey, the seat camera on the
 * own ship; on `fixture`'s field, under `TERRAIN TEST`'s name on either.
 */
export function terrainScene(fixture: ViewsCheckTerrainFixture = "terrain-test"): KeptScene {
  return {
    name: TERRAIN_SCENE_NAME,
    durationS: TERRAIN_TEST_SCRIPT.durationS,
    sceneAt: (tS) => terrainSceneAt(tS, fixture),
    cameraAt: (tS): CameraPose => {
      const attitude = shipAttitude(tS, terrainMoonRotationAt(TERRAIN_TIME_RATE * tS));
      return {
        frame: { kind: "craft", craft: TERRAIN_TEST_SHIP },
        positionM: rotate(attitude, TEST_HULL.eyePointM),
        orientation: attitude,
      };
    },
  };
}

/**
 * The terrain test's fixture (`SceneOption.terrain`): `TEST MOON`'s field served from `fixture`'s
 * terrain golden, its revision rising at {@link TERRAIN_TEST_SCRIPT}'s `deltaAtS`.
 *
 * @param chunkIntervalMs - How long its links wait before each chunk, ms: 0, each on the next task,
 *   by default (`TerrainTestFixtureOptions.chunkIntervalMs`).
 */
export function terrainSceneFixture(
  chunkIntervalMs = 0,
  fixture: ViewsCheckTerrainFixture = "terrain-test",
): TerrainTestFixture {
  return terrainTestFixture({
    universe: TERRAIN_TEST_UNIVERSE,
    golden: () => terrainTestGolden(fixture),
    deltaAtS: TERRAIN_TEST_SCRIPT.deltaAtS,
    chunkIntervalMs,
  });
}

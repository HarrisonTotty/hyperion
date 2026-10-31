/**
 * The hand-over check's check-only kept scenes (plan R10, R10.T10.g): `HAND-OVER 30°`,
 * `HAND-OVER 90°` and `HAND-OVER TERMINATOR`, one a look, which a `--views-check --terrain
 * --handover` launch alone offers in `VIEW`, and the world they are made on.
 *
 * @remarks
 * - **The world is a parameter** ({@link HandoverWorld}): the body the looks approach, its datum,
 *   its relief, the cells the frames are judged by, its survey's middle and its star, and how a
 *   look's scene and its session's fixture are made. {@link terrainTestHandoverWorld} is
 *   `TERRAIN TEST`'s: its star and moon, the moon's terrain served by the scene's fixture from the
 *   terrain golden, both survey passes at once; on a `--weathered` launch, `TEST MOON` weathered's
 *   field in its place, as `TERRAIN TEST WEATHERED` (plan R10, R10.T1.c's client half).
 *   {@link servedHandoverWorld} is a served world's (plan R10, R10.T15.b): the same members from the
 *   server (its field header's relief and level, its coverage's surveyed cells and their middle,
 *   its star's direction from the server's scene, its sky's host discs) and its own scene and field
 *   source, the server's link (`servedHandover.ts` reads it). The run (`handoverCheckRun.ts`) reads
 *   the world alone and is unchanged by either.
 * - **Each look's scene** is the world's star and body ({@link LookBodies}) with the body held still
 *   at the look's rotation ({@link lookGeometry}) and the clock at one second a second, and the own
 *   ship, `TEST HULL`, flown along the look's line ({@link limbRatioAt}) looking at the body's
 *   centre, its seat the camera's. Each look has its own fixture, so each choice of a look makes a
 *   fresh terrain session.
 * - **The range** the lines are drawn at is the run's, measured on the primary's canvas
 *   ({@link handoverRangeAtM}) and set ({@link HandoverScenes.setRangeM}) before the run chooses a
 *   look. A look chosen by hand before then, on a shown launch, is drawn at the provisional range
 *   `App` gives, the window's width's, and measures nothing; the run takes `VIEW` off a look
 *   already chosen before it chooses it, so that each look it measures starts afresh at its own
 *   range.
 * - **Never on an ordinary launch**: `App` makes these scenes on a `--handover` launch alone and
 *   hands them to the terrain probe's kept scenes (`TerrainProbeOptions.checkScenes`); they are no
 *   member of `SCENE_OPTIONS`.
 */

import type {
  BodyIdHex,
  DetailSeedHex,
  HostDiscDto,
  SurfaceHeaderDto,
  SystemIdHex,
  UniverseIdHex,
} from "@hyperion/protocol";

import type { QualitySettingName, ViewsCheckTerrainFixture } from "../../../../../preload/api";
import { add, norm, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { DEFAULT_FOV_DEG } from "../../../view/camera/projection";
import { handoverRangeM, handoverTauPx, pixelAngleRad } from "../../../view/terrain/handover";
import type { BodyPhotometry, WireAppearance } from "../../../view/appearance/fromWire";
import type { CameraPose } from "../../../view/camera/pose";
import { IDENTITY_QUATERNION, rotate } from "../../../view/camera/quaternion";
import { SUN_RADIUS_M } from "../../../view/lighting/hostDisc";
import { TEST_HULL } from "../../../view/scene/hull";
import type { ServedBody } from "../../../view/scene/servedBody";
import { type CoverageView, fieldCellAt, fieldCellCount } from "../../../view/terrain/coverage";
import { PATCH_QUADS, vertexDir } from "../../../view/terrain/cube";
import type { TerrainRequests } from "../../../view/terrain/session";
import {
  bodyKindSymbol,
  staticRetarded,
  type ViewBody,
  type ViewScene,
} from "../../../view/scene/model";
import {
  KEPT_BARYCENTRE,
  KEPT_TIDAL_RADIUS_M,
  type KeptScene,
  keptTime,
} from "../../../view/scenes/kept";
import { terrainTestFixture } from "../../../view/scenes/terrainFixture";
import {
  TERRAIN_HOST_DISCS,
  TERRAIN_MOON_CENTRE_M,
  TERRAIN_MOON_HILL_M,
  TERRAIN_TEST_SHIP,
  TERRAIN_TEST_UNIVERSE,
  terrainMoonPhotometry,
  terrainStar,
  terrainSystem,
  terrainTestGolden,
} from "../../../view/scenes/terrainScene";
import type { SurfaceEvents, TerrainFixture } from "../useTerrainSession";
import type { SceneOption } from "../viewRun";
import {
  approachDurationS,
  cameraOnLineM,
  type HandoverApproach,
  type HandoverFigure,
  type HandoverLook,
  HANDOVER_LOOKS,
  limbRatioAt,
  type LookGeometry,
  lookAttitude,
  lookGeometry,
} from "./handoverGeometry";

/** The world the hand-over check's looks are made on: see the module's remarks. */
export interface HandoverWorld {
  /** Its name in the record. */
  readonly name: string;
  /** The body the looks approach, whose survey the frames are judged over. */
  readonly body: BodyIdHex;
  readonly figure: HandoverFigure;
  /** The body's realised relief, m: its field header's, which its hand-over range is made of. */
  readonly reliefM: number;
  /** The level of the cells the frames' pixels are judged by: its field's. */
  readonly cellLevel: number;
  /** The cells its session's coverage holds surveyed once the survey has arrived whole. */
  readonly surveyedCells: number;
  /** The survey's middle, a body-fixed direction. */
  readonly surveyMiddle: Vec3;
  /** The body's star from the body's centre, m along the galactic axes. */
  readonly towardStarM: Vec3;
  /** The own ship's hull's eye point, m along its axes: the seat camera's place on it. */
  readonly eyePointM: Vec3;
  /** The look's scene on its `geometry`, the own ship flown along `approach`'s line. */
  scene(look: HandoverLook, geometry: LookGeometry, approach: HandoverApproach): KeptScene;
  /** A fixture for one look's terrain session, its whole survey served from its first ask. */
  fixture(): TerrainFixture;
}

/** The cells a coverage's runs hold surveyed (a resolution code above 0; margin cells are 0). */
function surveyedCellCount(
  ranges: ReadonlyArray<{
    readonly start: number;
    readonly end: number;
    readonly resolution: number;
  }>,
): number {
  return ranges.reduce((sum, run) => sum + (run.resolution > 0 ? run.end - run.start : 0), 0);
}

/**
 * The survey's middle on `TERRAIN TEST`'s moon, body-fixed: between the two survey passes' centres,
 * astride the edge of the cube's faces +x and +y (T10.e's scratch harness's).
 */
const TERRAIN_TEST_SURVEY_MIDDLE = vec3(1, 1, 0.71);

/** The span of the script over which the ship's velocity is a central difference, s. */
const VELOCITY_STEP_S = 0.05;

/**
 * What a look's scene draws of its world: the star at the origin and the body about it, as the
 * world gives them, `TERRAIN TEST`'s or a served one's.
 */
export interface LookBodies {
  readonly system: SystemIdHex;
  readonly star: {
    readonly id: BodyIdHex;
    readonly designation: string;
    readonly radiusM: number;
  };
  readonly body: {
    readonly id: BodyIdHex;
    readonly kind: "planet" | "dwarf_planet" | "moon";
    readonly designation: string;
    readonly figure: HandoverFigure;
    readonly hillRadiusM: number;
    readonly photometry: BodyPhotometry;
    /** Its detail seed, which the terrain session takes in place of asking `body_detail`. */
    readonly detailSeed?: DetailSeedHex;
  };
  /** The body's centre, m from the star along the galactic axes. */
  readonly bodyCentreM: Vec3;
  readonly hostDiscs: ReadonlyArray<HostDiscDto>;
}

/** The world's body at the look's rotation, the own ship on its line, at script time `tS`. */
function lookSceneAt(
  world: LookBodies,
  look: HandoverLook,
  geometry: LookGeometry,
  approach: HandoverApproach,
  tS: number,
): ViewScene {
  const appearance: WireAppearance = {
    photometry: world.body.photometry,
    figure: { ...world.body.figure, pole: geometry.pole },
    labels: [],
    bondRatioFinding: null,
  };
  const star = world.star.id;
  const moon = world.body.id;
  const centreM = world.bodyCentreM;
  const bodies: ViewBody[] = [
    {
      id: star,
      parent: null,
      kind: "star",
      designation: world.star.designation,
      radiusM: world.star.radiusM,
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
      kind: world.body.kind,
      designation: world.body.designation,
      radiusM: world.body.figure.equatorialRadiusM,
      hillRadiusM: world.body.hillRadiusM,
      centreM,
      retarded: staticRetarded(centreM),
      rotation: geometry.rotation,
      appearance,
      symbol: bodyKindSymbol(world.body.kind),
      orbitNormal: null,
      ...(world.body.detailSeed === undefined ? {} : { detailSeed: world.body.detailSeed }),
    },
  ];
  // The camera's line is the body's, about its centre: the scene's own place, the star's at the origin.
  const placeAt = (s: number): Vec3 => cameraOnLineM(geometry, limbRatioAt(approach, s));
  const place = placeAt(tS);
  const before = Math.max(tS - VELOCITY_STEP_S, 0);
  const after = tS + VELOCITY_STEP_S;
  // The clock runs at one second a second, so a second of the script is one of the scene.
  const velocity = scale(sub(placeAt(after), placeAt(before)), 1 / (after - before));
  // The moon is still, so its body-fixed axes are its galactic ones turned back by its rotation.
  const [r0, r1, r2] = geometry.rotation.rows;
  const bodyFixed = vec3(
    r0.x * place.x + r1.x * place.y + r2.x * place.z,
    r0.y * place.x + r1.y * place.y + r2.y * place.z,
    r0.z * place.x + r1.z * place.y + r2.z * place.z,
  );
  return {
    provenance: { kind: "kept", name: look.scene },
    time: keptTime(tS),
    timeRate: 1,
    system: world.system,
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
          position: { kind: "body_fixed", body: moon, m: bodyFixed },
          attitude: lookAttitude(geometry),
        },
        predictedPath: null,
        velocityMPerS: velocity,
      },
    ],
    stars: [],
    ownShip: TERRAIN_TEST_SHIP,
    defaultPose: {
      frame: { kind: "craft", craft: TERRAIN_TEST_SHIP },
      positionM: TEST_HULL.eyePointM,
      orientation: IDENTITY_QUATERNION,
    },
    hostDiscs: world.hostDiscs,
  };
}

/** `TERRAIN TEST`'s star and moon on `fixture`'s field, with `photometry`, for its looks' scenes. */
function terrainLookBodies(
  fixture: ViewsCheckTerrainFixture,
  photometry: BodyPhotometry,
): LookBodies {
  const golden = terrainTestGolden(fixture);
  const { header } = golden;
  return {
    system: terrainSystem(fixture),
    star: { id: terrainStar(fixture), designation: "TEST STAR", radiusM: SUN_RADIUS_M },
    body: {
      id: golden.body,
      kind: "moon",
      designation: "TEST MOON",
      figure: {
        equatorialRadiusM: header.equatorial_radius_m,
        polarRadiusM: header.polar_radius_m,
      },
      hillRadiusM: TERRAIN_MOON_HILL_M,
      photometry,
      detailSeed: golden.seed,
    },
    bodyCentreM: TERRAIN_MOON_CENTRE_M,
    hostDiscs: TERRAIN_HOST_DISCS,
  };
}

/**
 * The hand-over world's name in the record on each fixture's field: `TERRAIN TEST`'s own, or
 * `TEST MOON` weathered's (plan R10, R10.T1.c's client half) under the scene's name.
 */
const HANDOVER_WORLD_NAMES: Readonly<Record<ViewsCheckTerrainFixture, string>> = {
  "terrain-test": "TERRAIN TEST",
  "terrain-test-weathered": "TERRAIN TEST WEATHERED",
};

/**
 * `TERRAIN TEST`'s world on `fixture`'s field (`TERRAIN TEST`'s own unless given, or `TEST MOON`
 * weathered's on a `--weathered` launch): its moon, the golden's figure, relief, level and survey
 * (its last pass's surveyed cells), its survey's middle, and its star along −x at 1 au; each look's
 * fixture serves the golden's whole survey at its first ask (the revision's rise at 0 s of the
 * script).
 */
export function terrainTestHandoverWorld(
  fixture: ViewsCheckTerrainFixture = "terrain-test",
): HandoverWorld {
  const golden = terrainTestGolden(fixture);
  const { header } = golden;
  const survey = golden.surveys.at(-1);
  // Its crust's photometry, made once for every frame of every look: the weathered field's from its
  // golden's crust_albedo (`terrainMoonPhotometry`).
  const photometry = terrainMoonPhotometry(header, golden.maturity);
  const bodies = terrainLookBodies(fixture, photometry);
  return {
    name: HANDOVER_WORLD_NAMES[fixture],
    body: golden.body,
    figure: { equatorialRadiusM: header.equatorial_radius_m, polarRadiusM: header.polar_radius_m },
    reliefM: header.realised_relief_m,
    cellLevel: header.level,
    surveyedCells: survey === undefined ? 0 : surveyedCellCount(survey.ranges),
    surveyMiddle: TERRAIN_TEST_SURVEY_MIDDLE,
    towardStarM: sub(vec3(0, 0, 0), TERRAIN_MOON_CENTRE_M),
    eyePointM: TEST_HULL.eyePointM,
    scene: (look, geometry, approach): KeptScene => ({
      name: look.scene,
      durationS: approachDurationS(approach),
      sceneAt: (tS) => lookSceneAt(bodies, look, geometry, approach, tS),
      cameraAt: (): CameraPose => {
        const attitude = lookAttitude(geometry);
        return {
          frame: { kind: "craft", craft: TERRAIN_TEST_SHIP },
          positionM: rotate(attitude, TEST_HULL.eyePointM),
          orientation: attitude,
        };
      },
    }),
    fixture: () =>
      terrainTestFixture({
        universe: TERRAIN_TEST_UNIVERSE,
        golden: () => terrainTestGolden(fixture),
        deltaAtS: 0,
        chunkIntervalMs: 0,
      }),
  };
}

/**
 * A survey's middle, body-fixed: the mean direction of its surveyed cells' centres, or, for a
 * survey whose cells balance about the centre, as one of the whole body does (its mean shorter than
 * {@link WHOLE_SURVEY_MEAN} of a unit), `TERRAIN TEST`'s middle astride the edge of the cube's faces
 * +x and +y, which any whole survey holds (a lean of R10.T15.b.1's record).
 */
export function surveyMiddleOf(coverage: Pick<CoverageView, "level" | "resolutionAtM">): Vec3 {
  const { level } = coverage;
  let sum = vec3(0, 0, 0);
  let cells = 0;
  for (let index = 0; index < fieldCellCount(level); index += 1) {
    const cell = fieldCellAt(level, index);
    if (coverage.resolutionAtM(cell) === null) {
      continue;
    }
    // The cell's centre: the middle vertex of its 64 quads a side.
    const [x, y, z] = vertexDir(cell, PATCH_QUADS / 2, PATCH_QUADS / 2);
    sum = add(sum, vec3(x, y, z));
    cells += 1;
  }
  return cells > 0 && norm(sum) > WHOLE_SURVEY_MEAN * cells
    ? normalise(sum)
    : normalise(TERRAIN_TEST_SURVEY_MIDDLE);
}

/** The mean's length below which a survey is taken as one about the whole body, of a unit. */
export const WHOLE_SURVEY_MEAN = 0.1;

/** The cells a coverage holds surveyed: its mask's texels, a cell each, that are surveyed. */
export function coverageSurveyedCells(coverage: Pick<CoverageView, "maskTexels">): number {
  return coverage.maskTexels().reduce((count, texel) => count + (texel === 0 ? 0 : 1), 0);
}

/** The server's link a served world's looks ask through: its requests and its surface events. */
export interface ServedLink {
  readonly requests: TerrainRequests;
  readonly onSurface: SurfaceEvents;
}

/** What a served hand-over world is made of (plan R10, R10.T15.b): `servedHandover.ts` reads it. */
export interface ServedHandoverInput {
  /** The body as the server's scene draws it, with its star. */
  readonly served: ServedBody;
  /** Its system's discs, from the server's sky. */
  readonly hostDiscs: ReadonlyArray<HostDiscDto>;
  /** Its field's header and coverage, as the server answers them: the survey the looks judge. */
  readonly header: SurfaceHeaderDto;
  readonly coverage: CoverageView;
  /**
   * Its detail seed, from its record (`bodyDetailSeed`): each look's scene is a kept scene, whose
   * body draws its terrain and asks its readout only with a seed of its own (`surfaceUnsetOf`).
   */
  readonly detailSeed: DetailSeedHex;
  /** The link every look's terrain session asks through, for the world's life. */
  readonly link: ServedLink;
}

/**
 * A served world's hand-over world (plan R10, R10.T15.b): the body the server serves, named by its
 * designation, its figure, relief and level its field header's, its surveyed cells and their middle
 * ({@link surveyMiddleOf}) its coverage's, its star where the server's scene draws it, and each look's
 * scene its star and body ({@link LookBodies}), lit by its sky's discs, with the detail seed its
 * record gives, without which a kept scene's body draws no terrain (R10.T15.b.1); each look's
 * fixture is the server's link, which no look closes.
 */
export function servedHandoverWorld(input: ServedHandoverInput): HandoverWorld {
  const { served, header, coverage, link } = input;
  const figure: HandoverFigure = {
    equatorialRadiusM: header.equatorial_radius_m,
    polarRadiusM: header.polar_radius_m,
  };
  const bodies: LookBodies = {
    system: served.system,
    star: {
      id: served.star.id,
      designation: served.star.designation,
      radiusM: served.star.radiusM,
    },
    body: {
      id: served.body.id,
      kind: served.kind,
      designation: served.body.designation,
      figure,
      hillRadiusM: served.hillRadiusM,
      photometry: served.appearance.photometry,
      detailSeed: input.detailSeed,
    },
    bodyCentreM: scale(served.towardStarM, -1),
    hostDiscs: input.hostDiscs,
  };
  const universe: UniverseIdHex = served.universe;
  return {
    name: served.body.designation,
    body: served.body.id,
    figure,
    reliefM: header.realised_relief_m,
    cellLevel: header.level,
    surveyedCells: coverageSurveyedCells(coverage),
    surveyMiddle: surveyMiddleOf(coverage),
    towardStarM: served.towardStarM,
    eyePointM: TEST_HULL.eyePointM,
    scene: (look, geometry, approach): KeptScene => ({
      name: look.scene,
      durationS: approachDurationS(approach),
      sceneAt: (tS) => lookSceneAt(bodies, look, geometry, approach, tS),
      cameraAt: (): CameraPose => {
        const attitude = lookAttitude(geometry);
        return {
          frame: { kind: "craft", craft: TERRAIN_TEST_SHIP },
          positionM: rotate(attitude, TEST_HULL.eyePointM),
          orientation: attitude,
        };
      },
    }),
    fixture: (): TerrainFixture => ({
      universe,
      // The link is the launch's; a look's session neither opens nor closes it.
      open: () => ({ requests: link.requests, onSurface: link.onSurface, close: () => undefined }),
    }),
  };
}

/** A world's hand-over range on a canvas ({@link handoverRangeAtM}). */
export interface HandoverRange {
  /** The range, m, against the camera's distance to the limb. */
  readonly rangeM: number;
  /** The τ it hands over at, px, and the canvas's centre pixel, rad. */
  readonly tauPx: number;
  readonly radPerPx: number;
}

/**
 * `world`'s hand-over range on a canvas `widthPx` wide at the default field of view on `setting`:
 * its relief at the photorealistic style's τ over the canvas's centre pixel (`pixelAngleRad`),
 * as `handOverRegimes` on the presented pixel and the terrain run's `terrainHandover` take it.
 */
export function handoverRangeAtM(
  world: HandoverWorld,
  setting: QualitySettingName,
  widthPx: number,
): HandoverRange {
  const radPerPx = pixelAngleRad(
    { fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180, orientation: IDENTITY_QUATERNION },
    { widthPx, heightPx: widthPx },
  );
  const tauPx = handoverTauPx("photorealistic", setting);
  return { rangeM: handoverRangeM(world.reliefM, tauPx, radPerPx), tauPx, radPerPx };
}

/**
 * The check-only kept scenes of a `--handover` launch, one a look, and what the run sets and reads
 * of them: the range their lines are drawn at, and each look's script time as its stage plays it.
 */
export class HandoverScenes {
  readonly world: HandoverWorld;
  readonly approach: HandoverApproach;
  /** The scenes, in {@link HANDOVER_LOOKS}' order, each with its own fixture. */
  readonly sceneOptions: ReadonlyArray<SceneOption>;
  /** The range the looks are drawn at, m: the provisional one until the run sets its own. */
  #rangeM: number;
  #measured = false;
  readonly #geometries = new Map<string, LookGeometry>();
  readonly #scriptS = new Map<string, number>();

  /**
   * @param provisionalRangeM - The range a look chosen before the run measures its own is drawn
   *   at, m: `App`'s, the window's width's ({@link handoverRangeAtM}).
   * @throws Error for a provisional range that is not positive and finite.
   */
  constructor(world: HandoverWorld, approach: HandoverApproach, provisionalRangeM: number) {
    if (!(Number.isFinite(provisionalRangeM) && provisionalRangeM > 0)) {
      throw new Error(
        `the provisional hand-over range must be positive, not ${String(provisionalRangeM)}`,
      );
    }
    this.world = world;
    this.approach = approach;
    this.#rangeM = provisionalRangeM;
    this.sceneOptions = HANDOVER_LOOKS.map((look): SceneOption => {
      const inner = world.fixture();
      return {
        name: look.scene,
        make: () => {
          this.#scriptS.delete(look.name);
          return world.scene(look, this.geometry(look), approach);
        },
        terrain: {
          universe: inner.universe,
          open: () => inner.open(),
          notePlayed: (scriptS: number) => {
            this.#scriptS.set(look.name, scriptS);
            inner.notePlayed?.(scriptS);
          },
        },
      };
    });
  }

  /**
   * Sets the hand-over range the looks' lines are drawn at, m (the run's, from the primary's
   * canvas), before the run chooses a look.
   *
   * @throws Error for a range that is not positive and finite.
   */
  setRangeM(rangeM: number): void {
    if (!(Number.isFinite(rangeM) && rangeM > 0)) {
      throw new Error(`the hand-over range must be positive, not ${String(rangeM)}`);
    }
    this.#rangeM = rangeM;
    this.#measured = true;
    this.#geometries.clear();
  }

  /** The range the run set, m, or `null` before it did. */
  get rangeM(): number | null {
    return this.#measured ? this.#rangeM : null;
  }

  /**
   * The look's geometry at the range the run set, or at the provisional range before it did; made
   * once a range.
   */
  geometry(look: HandoverLook): LookGeometry {
    const rangeM = this.#rangeM;
    const known = this.#geometries.get(look.name);
    if (known !== undefined) {
      return known;
    }
    const geometry = lookGeometry(look, {
      figure: this.world.figure,
      surveyMiddle: this.world.surveyMiddle,
      towardStarM: this.world.towardStarM,
      rangeM,
    });
    this.#geometries.set(look.name, geometry);
    return geometry;
  }

  /** The script's time the look's stage last told its fixture, s; `null` since it was chosen. */
  scriptS(look: HandoverLook): number | null {
    return this.#scriptS.get(look.name) ?? null;
  }
}

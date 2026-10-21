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
 *   terrain golden, both survey passes at once. T15's served world gives the same members from the
 *   server (its body's header, its coverage's middle) and its own scene and field source; the run
 *   (`handoverCheckRun.ts`) reads the world alone and is unchanged by it. That half is T15's.
 * - **Each look's scene** is `TERRAIN TEST`'s bodies with the moon held still at the look's rotation
 *   ({@link lookGeometry}) and the clock at one second a second, and the own ship, `TEST HULL`,
 *   flown along the look's line ({@link limbRatioAt}) looking at the moon's centre, its seat the
 *   camera's. Each look has its own fixture, so each choice of a look makes a fresh terrain session.
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

import type { BodyIdHex } from "@hyperion/protocol";

import type { QualitySettingName } from "../../../../../preload/api";
import { scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { DEFAULT_FOV_DEG } from "../../../view/camera/projection";
import { handoverRangeM, handoverTauPx, pixelAngleRad } from "../../../view/terrain/handover";
import type { BodyPhotometry, WireAppearance } from "../../../view/appearance/fromWire";
import type { CameraPose } from "../../../view/camera/pose";
import { IDENTITY_QUATERNION, rotate } from "../../../view/camera/quaternion";
import { SUN_RADIUS_M } from "../../../view/lighting/hostDisc";
import { TEST_HULL } from "../../../view/scene/hull";
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
import type { TerrainFixture } from "../useTerrainSession";
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

/** `TERRAIN TEST`'s moon at the look's rotation, the own ship on its line, at script time `tS`. */
function terrainLookSceneAt(
  look: HandoverLook,
  geometry: LookGeometry,
  approach: HandoverApproach,
  photometry: BodyPhotometry,
  tS: number,
): ViewScene {
  const golden = terrainTestGolden();
  const { header } = golden;
  const appearance: WireAppearance = {
    photometry,
    figure: {
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
      pole: geometry.pole,
    },
    labels: [],
    bondRatioFinding: null,
  };
  const star = terrainStar();
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
      rotation: geometry.rotation,
      appearance,
      symbol: bodyKindSymbol("moon"),
      orbitNormal: null,
      detailSeed: golden.seed,
    },
  ];
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
    system: terrainSystem(),
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
    hostDiscs: TERRAIN_HOST_DISCS,
  };
}

/**
 * `TERRAIN TEST`'s world: its moon, the terrain golden's figure, relief, level and survey (its last
 * pass's surveyed cells), its survey's middle, and its star along −x at 1 au; each look's fixture
 * serves the golden's whole survey at its first ask (the revision's rise at 0 s of the script).
 */
export function terrainTestHandoverWorld(): HandoverWorld {
  const golden = terrainTestGolden();
  const { header } = golden;
  const survey = golden.surveys.at(-1);
  // Its crust's photometry, made once for every frame of every look.
  const photometry = terrainMoonPhotometry(header);
  return {
    name: "TERRAIN TEST",
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
      sceneAt: (tS) => terrainLookSceneAt(look, geometry, approach, photometry, tS),
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
        golden: terrainTestGolden,
        deltaAtS: 0,
        chunkIntervalMs: 0,
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

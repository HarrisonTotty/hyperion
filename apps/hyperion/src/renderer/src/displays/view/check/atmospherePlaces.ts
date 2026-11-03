/**
 * The atmosphere run's places in the check-only software mode (plan R08, R08.T17's by-eye list;
 * research-rm4-rm5-close-out §4, lane 4): each capture taken from a check-only kept scene that
 * stands its free camera at the capture's pose, in place of the tour's flight to it, which
 * SwiftShader's frames leave 10⁹ to 10¹⁰ m short.
 *
 * @remarks
 * - **Why.** The free camera steps once a frame, at most `MAX_FREE_STEP_S`, and SwiftShader's frames
 *   take 0.4 to 5 s, so a stage's hold of the tour's length flies a few hundredths of its distance
 *   in a pod and no capture was taken where it is labelled (R01.T16's software runs of
 *   2026-10-30). The GPU tour is flown as before.
 * - **The places** ({@link atmospherePlaces}): the tour's captures, each at its leg's planned end
 *   ({@link atmosphereLegs}), looking at its fixture's centre with galactic +z as near up as that
 *   allows; and on a full run the looks of plan R08's "By eye" list that the tour does not reach
 *   ({@link ATMOSPHERE_LOOKS}), each after its fixture's last tour capture: `TEST JUPITER` outside
 *   the 10⁹ m switch, `TEST SATURN` over its pole, and views from the ground (or 400 km up) of
 *   `TEST EARTH`, `TEST MARS` and `TEST VENUS` at a set height of the star over their horizon.
 * - **Each place's scene** ({@link placedAtmosphereScene}) is `ATMOSPHERE TEST`'s bodies on the
 *   page's media, with no own ship, so that its camera starts free (`newCameraState`) at the
 *   place, in its fixture's frame, where the label block's `FRAME` and `POSITION` lines read it, and
 *   no hull is drawn; it opens photorealistic, on the `LIT` meter for a look at a disc and `AVG`
 *   for a view of the sky, and its `SCENE` line names the place. The atmosphere cache and the
 *   exposure are held above the stage, so a new place keeps both.
 * - **Two scenes for them all** ({@link PlacedAtmosphere}): `PLACE A` and `PLACE B` each make the
 *   place the run last offered, and the run chooses them in turn, a new stage a place.
 * - **Never on an ordinary launch**: `App` makes these scenes on a software `--atmosphere` launch
 *   alone and hands them to `VIEW` after its own; they are no member of `SCENE_OPTIONS`.
 */

import {
  add,
  cross,
  dot,
  norm,
  normalise,
  scale,
  sub,
  type Vec3,
  vec3,
} from "../../../geometry/vec3";
import type { CameraPose, Quaternion } from "../../../view/camera/pose";
import { lookAlong } from "../../../view/camera/quaternion";
import type { MeterMode } from "../../../view/post/meter";
import {
  ATMOSPHERE_FIXTURES,
  type AtmosphereFixture,
  atmosphereScene,
  atmosphereSeatM,
  phaseAngleDeg,
  prepareAtmosphereScene,
} from "../../../view/scenes/atmosphereScene";
import type { KeptScene } from "../../../view/scenes/kept";
import { SCENE_OPTIONS, type SceneOption } from "../viewRun";
import {
  ATMOSPHERE_GOAL_RADII,
  ATMOSPHERE_SMOKE_TOUR,
  ATMOSPHERE_TOUR,
  type AtmosphereTourStep,
  atmosphereLegs,
  rangeCaptureLabel,
} from "./atmosphereCheckRun";

/** A place the software mode's atmosphere run captures from, by its kept scene. */
export interface AtmospherePlace {
  /** Its capture's label: a tour capture's (`fixture-test-mars`) or a look's. */
  readonly label: string;
  /** Its kept scene's name, as its label block's `SCENE` line reads it ({@link placeSceneName}). */
  readonly scene: string;
  /** The fixture it stands by, in whose frame its camera stands. */
  readonly designation: string;
  /** The camera, m from the fixture's centre along the galactic axes. */
  readonly fromCentreM: Vec3;
  /** The camera's orientation, camera axes to the galactic. */
  readonly orientation: Quaternion;
  /** Its range from the fixture's centre, m. */
  readonly rangeM: number;
  /** The phase angle at the fixture between the star and the camera, deg. */
  readonly phaseDeg: number;
  /** The meter its scene opens on: `lit` for a look at a disc, `average` for one at the sky. */
  readonly meter: Extract<MeterMode, "lit" | "average">;
}

/**
 * A place's kept scene's name, as its label block's `SCENE` line reads it: `PLACED` and its
 * capture's label, `PLACED FIXTURE-TEST-MARS`.
 */
export function placeSceneName(label: string): string {
  return `PLACED ${label.toUpperCase()}`;
}

/** The galactic up the disc looks are framed by, as near as their line of sight allows. */
const UP = vec3(0, 0, 1);

/** A fixture by its designation. */
function fixtureNamed(designation: string): AtmosphereFixture {
  const fixture = ATMOSPHERE_FIXTURES.find((each) => each.designation === designation);
  if (fixture === undefined) {
    throw new Error(`views check: ATMOSPHERE TEST has no fixture ${designation}`);
  }
  return fixture;
}

/** A look at `fixture`'s centre from `fromCentreM`, captured as `label`. */
function discPlace(fixture: AtmosphereFixture, label: string, fromCentreM: Vec3): AtmospherePlace {
  return {
    label,
    scene: placeSceneName(label),
    designation: fixture.designation,
    fromCentreM,
    orientation: lookAlong(scale(fromCentreM, -1), UP),
    rangeM: norm(fromCentreM),
    phaseDeg: phaseAngleDeg(fixture.centreM, add(fixture.centreM, fromCentreM)),
    meter: "lit",
  };
}

/**
 * A view from over a sphere fixture's ground: the camera `heightM` up where the star stands
 * `sunElevationDeg` over the horizon, looking `azimuthFromStarDeg` round from the star's azimuth
 * (0 towards it, 90 square to it, 180 away), `pitchDeg` over the apparent horizon (the horizontal
 * lowered by the horizon's dip at that height), its up the local vertical.
 */
export interface GroundLook {
  readonly kind: "ground";
  readonly label: string;
  readonly designation: string;
  readonly sunElevationDeg: number;
  readonly heightM: number;
  readonly azimuthFromStarDeg: number;
  readonly pitchDeg: number;
}

/** A look at a fixture's disc from `rangeM` off its centre along `direction`, not normalised. */
export interface DiscLook {
  readonly kind: "disc";
  readonly label: string;
  readonly designation: string;
  readonly rangeM: number;
  readonly direction: Vec3;
}

/** One of the software mode's looks beyond the tour. */
export type AtmosphereLook = GroundLook | DiscLook;

/** The height of the ground views over the ground, m: 10, the own ship's over `TEST EARTH`'s. */
const GROUND_HEIGHT_M = 10;

/** How high the ground views look over the horizon, deg: enough to show sky down to it. */
const GROUND_PITCH_DEG = 8;

/**
 * The tour's line to `TEST JUPITER`, from its centre towards the tour's approach: its first range
 * capture's direction.
 */
function jupiterApproach(): Vec3 {
  const leg = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_TOUR).find(
    (each) => each.target === "TEST JUPITER",
  );
  if (leg === undefined) {
    throw new Error("views check: the tour has no leg to TEST JUPITER");
  }
  return sub(leg.toM, leg.centreM);
}

/** `TEST JUPITER`'s range outside the 10⁹ m switch (R08.T16.a.2), beside the tour's 9.5 × 10⁸ m. */
export const JUPITER_OUTSIDE_RANGE_M = 1.05e9;

/**
 * The looks of plan R08's "By eye" list beyond the tour's captures, the software mode's alone:
 * - a Jupiter-class limb across 10⁹ m: `TEST JUPITER` at {@link JUPITER_OUTSIDE_RANGE_M}, on the
 *   tour's line, the other side of the switch from its 9.5 × 10⁸ m;
 * - a Saturn-class limb over the pole (the tour's comes on it over its equator): `TEST SATURN` at
 *   the tour's goal of {@link ATMOSPHERE_GOAL_RADII} radii over its +z pole;
 * - Earth's sky from the ground at noon and at sunset, the star 60° and 2° up, square to the star at
 *   noon and towards it at sunset; the limb from 400 km, the star 30° up, square to it; and stars
 *   near the horizon at night, the star 45° below it as at the seat, looking towards its azimuth,
 *   away from `TEST MOON`;
 * - Mars's day and its sunset, the star 45° and 2° up, square to it and towards it;
 * - the Venus-class surface by day, the star 45° up, square to it.
 */
export const ATMOSPHERE_LOOKS: ReadonlyArray<AtmosphereLook> = [
  {
    kind: "ground",
    label: "ground-test-mars-day",
    designation: "TEST MARS",
    sunElevationDeg: 45,
    heightM: GROUND_HEIGHT_M,
    azimuthFromStarDeg: 90,
    pitchDeg: GROUND_PITCH_DEG,
  },
  {
    kind: "ground",
    label: "ground-test-mars-sunset",
    designation: "TEST MARS",
    sunElevationDeg: 2,
    heightM: GROUND_HEIGHT_M,
    azimuthFromStarDeg: 0,
    pitchDeg: GROUND_PITCH_DEG,
  },
  {
    kind: "disc",
    label: "fixture-test-saturn-pole",
    designation: "TEST SATURN",
    rangeM: ATMOSPHERE_GOAL_RADII * fixtureNamed("TEST SATURN").figure.equatorialRadiusM,
    direction: UP,
  },
  {
    kind: "ground",
    label: "ground-test-venus-day",
    designation: "TEST VENUS",
    sunElevationDeg: 45,
    heightM: GROUND_HEIGHT_M,
    azimuthFromStarDeg: 90,
    pitchDeg: GROUND_PITCH_DEG,
  },
  {
    kind: "disc",
    label: rangeCaptureLabel("TEST JUPITER", JUPITER_OUTSIDE_RANGE_M),
    designation: "TEST JUPITER",
    rangeM: JUPITER_OUTSIDE_RANGE_M,
    direction: jupiterApproach(),
  },
  {
    kind: "ground",
    label: "ground-test-earth-noon",
    designation: "TEST EARTH",
    sunElevationDeg: 60,
    heightM: GROUND_HEIGHT_M,
    azimuthFromStarDeg: 90,
    pitchDeg: GROUND_PITCH_DEG,
  },
  {
    kind: "ground",
    label: "ground-test-earth-sunset",
    designation: "TEST EARTH",
    sunElevationDeg: 2,
    heightM: GROUND_HEIGHT_M,
    azimuthFromStarDeg: 0,
    pitchDeg: GROUND_PITCH_DEG,
  },
  {
    kind: "ground",
    label: "ground-test-earth-400-km",
    designation: "TEST EARTH",
    sunElevationDeg: 30,
    heightM: 400_000,
    azimuthFromStarDeg: 90,
    pitchDeg: 5,
  },
  {
    kind: "ground",
    label: "ground-test-earth-night-horizon",
    designation: "TEST EARTH",
    sunElevationDeg: -45,
    heightM: GROUND_HEIGHT_M,
    azimuthFromStarDeg: 0,
    pitchDeg: GROUND_PITCH_DEG,
  },
];

/** Degrees in radians. */
function rad(deg: number): number {
  return (deg * Math.PI) / 180;
}

/** A ground view's place on its fixture, which must be a sphere. */
function groundPlace(look: GroundLook): AtmospherePlace {
  const fixture = fixtureNamed(look.designation);
  const { equatorialRadiusM: radiusM, polarRadiusM } = fixture.figure;
  if (radiusM !== polarRadiusM) {
    throw new Error(`views check: ${look.designation} is no sphere to stand on`);
  }
  // The star's direction, and the one square to it towards galactic +z: the ground point's
  // vertical lies between them, the star `sunElevationDeg` over its horizon.
  const towardStar = normalise(scale(fixture.centreM, -1));
  const across = normalise(sub(UP, scale(towardStar, dot(UP, towardStar))));
  const e = rad(look.sunElevationDeg);
  const vertical = add(scale(towardStar, Math.sin(e)), scale(across, Math.cos(e)));
  // The horizontal towards the star's azimuth, and the one 90° round from it.
  const toStarAzimuth = sub(scale(towardStar, Math.cos(e)), scale(across, Math.sin(e)));
  const left = cross(vertical, toStarAzimuth);
  const a = rad(look.azimuthFromStarDeg);
  const horizontal = add(scale(toStarAzimuth, Math.cos(a)), scale(left, Math.sin(a)));
  const dipRad = Math.acos(radiusM / (radiusM + look.heightM));
  const p = rad(look.pitchDeg) - dipRad;
  const forward = add(scale(horizontal, Math.cos(p)), scale(vertical, Math.sin(p)));
  const fromCentreM = scale(vertical, radiusM + look.heightM);
  return {
    label: look.label,
    scene: placeSceneName(look.label),
    designation: look.designation,
    fromCentreM,
    orientation: lookAlong(forward, vertical),
    rangeM: norm(fromCentreM),
    phaseDeg: phaseAngleDeg(fixture.centreM, add(fixture.centreM, fromCentreM)),
    meter: "average",
  };
}

/** A look's place. */
function lookPlace(look: AtmosphereLook): AtmospherePlace {
  let place: AtmospherePlace;
  switch (look.kind) {
    case "ground":
      place = groundPlace(look);
      break;
    case "disc":
      place = discPlace(
        fixtureNamed(look.designation),
        look.label,
        scale(normalise(look.direction), look.rangeM),
      );
      break;
  }
  return place;
}

/**
 * The software mode's places, in the order the run takes them: `tour`'s captures, each at its leg's
 * planned end looking at its fixture's centre, then `looks`, each after its fixture's last tour
 * capture, or at the end where the tour captures it nowhere.
 *
 * @throws Error for a tour or a look naming a fixture the scene does not have, or a ground view on
 *   a fixture that is no sphere.
 */
export function atmospherePlaces(
  tour: ReadonlyArray<AtmosphereTourStep>,
  looks: ReadonlyArray<AtmosphereLook>,
): ReadonlyArray<AtmospherePlace> {
  const places: AtmospherePlace[] = atmosphereLegs(atmosphereSeatM(), tour).flatMap((leg) =>
    leg.capture === null
      ? []
      : [
          discPlace(
            fixtureNamed(leg.capture.designation),
            leg.capture.label,
            sub(leg.toM, leg.centreM),
          ),
        ],
  );
  for (const look of looks) {
    const after = places.findLastIndex((place) => place.designation === look.designation);
    places.splice(after === -1 ? places.length : after + 1, 0, lookPlace(look));
  }
  return places;
}

/**
 * `place`'s kept scene: `ATMOSPHERE TEST`'s bodies on the page's media (or `scene`'s, a test's),
 * with no own ship, its free camera at the place in its fixture's frame (see the module's remarks).
 */
export function placedAtmosphereScene(
  place: AtmospherePlace,
  scene: KeptScene = atmosphereScene(),
): KeptScene {
  const fixture = fixtureNamed(place.designation);
  const pose: CameraPose = {
    frame: { kind: "body", body: fixture.id },
    positionM: place.fromCentreM,
    orientation: place.orientation,
  };
  return {
    name: place.scene,
    durationS: scene.durationS,
    sceneAt: (tS) => ({
      ...scene.sceneAt(tS),
      provenance: { kind: "kept", name: place.scene },
      craft: [],
      ownShip: null,
      defaultPose: pose,
    }),
    cameraAt: () => pose,
    startStyle: "photorealistic",
    startMeter: place.meter,
  };
}

/** The two check-only kept scenes a software `--atmosphere` launch offers, taken in turn. */
export const PLACE_SCENE_NAMES = ["PLACE A", "PLACE B"] as const;

/**
 * A software `--atmosphere` launch's places, and the kept scenes `VIEW` offers for them: its own,
 * then two, `PLACE A` and `PLACE B`, each making the place last offered ({@link offer}), which the
 * run chooses in turn, so that each place mounts a stage of its own while the `SCENE` selector
 * holds two buttons more, not one a place (22 crowded the low setting's view to 816 × 265 px).
 */
export class PlacedAtmosphere {
  readonly places: ReadonlyArray<AtmospherePlace>;
  /** `VIEW`'s own kept scenes, then {@link PLACE_SCENE_NAMES}' two. */
  readonly sceneOptions: ReadonlyArray<SceneOption>;
  /** The place the two scenes make: the one last offered, the first before any is. */
  #current: AtmospherePlace;
  #offered = 0;

  /**
   * @param scene - The kept scene the places' scenes are made from: `ATMOSPHERE TEST` on the
   *   page's media, unless a test gives its own.
   * @throws Error for no places.
   */
  constructor(
    places: ReadonlyArray<AtmospherePlace>,
    scene: () => KeptScene = () => atmosphereScene(),
  ) {
    const [first] = places;
    if (first === undefined) {
      throw new Error("views check: an atmosphere run placed needs a place");
    }
    this.places = places;
    this.#current = first;
    this.sceneOptions = [
      ...SCENE_OPTIONS,
      ...PLACE_SCENE_NAMES.map((name): SceneOption => ({
        name,
        make: () => placedAtmosphereScene(this.#current, scene()),
        prepare: () => {
          prepareAtmosphereScene();
        },
      })),
    ];
  }

  /**
   * Offers `place` to the scenes and returns the name of the one to choose for it: the other of the
   * two from the one chosen last, so that choosing it mounts a new stage, which makes `place`.
   */
  offer(place: AtmospherePlace): string {
    this.#current = place;
    const name =
      PLACE_SCENE_NAMES[this.#offered % PLACE_SCENE_NAMES.length] ?? PLACE_SCENE_NAMES[0];
    this.#offered += 1;
    return name;
  }
}

/** A full run's places, or a smoke's (its one fixture, and no look), with their scenes. */
export function placedAtmosphere(smoke: boolean): PlacedAtmosphere {
  return new PlacedAtmosphere(
    smoke
      ? atmospherePlaces(ATMOSPHERE_SMOKE_TOUR, [])
      : atmospherePlaces(ATMOSPHERE_TOUR, ATMOSPHERE_LOOKS),
  );
}

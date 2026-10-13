/**
 * The sky's sources for a body (plan R08, Design note 7; R08.T7): the one list R07 lights the
 * body's surface with, its stars and its planetshine sources, as the atmosphere's per-view
 * kernels draw them, with Design note 7's two skips inside the air.
 *
 * @remarks
 * A body's air is lit by exactly the sources that light its surface in the same frame
 * (`decision-r08-design.md` item 3), so the arrays are R07's own and are passed in, never ranked
 * again: the stars of `lightsAt(centre, hosts, MAX_BODY_LIGHTS)` (`view/lighting/hostLights.ts`),
 * past `STAR_CUT_RELATIVE`, brightest first, and `planetshineSources(…)` at the setting's count
 * (`view/lighting/planetshine.ts`). Each element becomes one {@link SkySource} in order, stars
 * first, unless a skip leaves it out.
 *
 * R07's arrays are taken at the body's centre in the frame of the lights' centres. Each source is
 * carried to the camera: its direction from the camera, and its illuminance there by the inverse
 * square (a planetshine source's from its centre; Design note 7), its angular radius asin(R ÷ d).
 * The Moon's parallax from a camera on Earth's ground is about 1°, a Sun-like star's 4 × 10⁻⁵ rad.
 *
 * Inside the air (the camera below the medium's top), the sky loop skips two kinds of source whose
 * light on the sky cannot be seen; the surfaces keep both, and outside the air none is skipped
 * (R08.T8):
 * - below the twilight limit, a source whose upper limb lies below
 *   e = −[acos(R ÷ r_c) + 2 acos(R ÷ R_top)] at the camera, R the figure's smallest radius of
 *   curvature c² ÷ a, r_c = R + h the camera's geodetic height and R_top = R + the medium's top:
 *   it lights no air the camera can see, single scattering and no refraction (20.2° below the
 *   horizon on Earth's ground with a 100 km top, beside astronomical twilight's 18°);
 * - negligible, one whose photopic illuminance at the camera is under `STAR_CUT_RELATIVE` of the
 *   brightest source whose centre stands above the camera's horizon: the Moon by day, at
 *   2.5 × 10⁻⁶ of the Sun.
 */

import { dot, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { conjugate, rotate } from "../camera/quaternion";
import type { LightAtPoint } from "../lighting/hostLights";
import { photopicIlluminance, STAR_CUT_RELATIVE } from "../lighting/illuminance";
import type { SecondarySource } from "../lighting/planetshine";
import type { Rgb } from "../photometry/toneCurve";
import { type AtmosphereCamera, geodeticOf, type SpheroidFigure, type SunState } from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import { MAX_SKY_LIGHT_SLOTS, MAX_SKY_SOURCES } from "./settings";

/** Which of R07's arrays a sky source came from. */
export type SkySourceKind = "star" | "planetshine";

/** One of the sky's sources: a {@link SunState} with the element of R07's arrays it was made from. */
export interface SkySource extends SunState {
  /** Which of R07's arrays it came from. */
  readonly kind: SkySourceKind;
  /** Its index there. */
  readonly index: number;
  /** Its light slot: a star's index, the body's light list's rank; 0 for planetshine. */
  readonly curveSlot: number;
  /**
   * Never drawn by the composite: R06's host-disc layer draws a star's disc into the scene beneath,
   * which the composite dims by the star's own transmittance, and R07 draws a planetshine source.
   */
  readonly drawsDisc: false;
}

/** Why a source is left out of the sky, or `null` for one that is drawn. */
export type SkySkip = "below twilight limit" | "negligible" | null;

/** One source of R07's arrays as {@link skySourcesWithSkips} lists it: the source, and its skip. */
export interface ListedSkySource {
  readonly source: SkySource;
  readonly skip: SkySkip;
}

/** One of R07's sources carried to the camera, before the skips. */
interface Carried {
  readonly kind: SkySourceKind;
  readonly index: number;
  /** From the camera, body-fixed, unit. */
  readonly direction: Vec3;
  readonly illuminanceLx: Rgb;
  readonly angularRadiusRad: number;
}

/**
 * The twilight limit at a camera `heightM` above a figure under a medium `topHeightM` deep, rad
 * below the horizon (a positive angle): acos(R ÷ r_c) + 2 acos(R ÷ R_top), R = c² ÷ a, the
 * figure's smallest radius of curvature (Design note 7).
 */
export function twilightLimitRad(
  figure: SpheroidFigure,
  heightM: number,
  topHeightM: number,
): number {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const r = (c * c) / a;
  const camera = r + Math.max(heightM, 0);
  const top = r + topHeightM;
  return Math.acos(Math.min(r / camera, 1)) + 2 * Math.acos(Math.min(r / top, 1));
}

/**
 * The sky's sources for a body seen from `camera` (Design note 7): R07's `stars` and
 * `planetshine` for the body, each carried to the camera, less the two skips inside the air,
 * stars first, each in its array's order.
 *
 * @param stars - R07's `lightsAt(bodyCentre, hosts, MAX_BODY_LIGHTS)` for the body: each light's
 *   `toStarM` from the body's centre, in the frame of the lights' centres.
 * @param planetshine - R07's `planetshineSources(body, lit, SETTINGS[s].photoreal.planetshineSources)`,
 *   their directions along the same axes.
 * @param camera - The camera in the body-fixed axes.
 * @param toBodyFixed - Rotates the lights' axes into the body-fixed ones.
 * @throws RangeError for more stars than {@link MAX_SKY_LIGHT_SLOTS} or more sources than
 *   {@link MAX_SKY_SOURCES}, which the kernels do not hold.
 */
export function skySources(
  stars: ReadonlyArray<LightAtPoint>,
  planetshine: ReadonlyArray<SecondarySource>,
  camera: Pick<AtmosphereCamera, "positionM">,
  figure: SpheroidFigure,
  medium: Pick<AtmosphereMedium, "topHeightM">,
  toBodyFixed: Quaternion,
): SkySource[] {
  return skySourcesWithSkips(stars, planetshine, camera, figure, medium, toBodyFixed).flatMap(
    ({ source, skip }) => (skip === null ? [source] : []),
  );
}

/**
 * {@link skySources} with every source of R07's arrays and why the sky skips it, if it does: the
 * tests' and the label block's view of the list.
 *
 * @throws RangeError as {@link skySources}.
 */
export function skySourcesWithSkips(
  stars: ReadonlyArray<LightAtPoint>,
  planetshine: ReadonlyArray<SecondarySource>,
  camera: Pick<AtmosphereCamera, "positionM">,
  figure: SpheroidFigure,
  medium: Pick<AtmosphereMedium, "topHeightM">,
  toBodyFixed: Quaternion,
): ListedSkySource[] {
  if (stars.length > MAX_SKY_LIGHT_SLOTS) {
    throw new RangeError(
      `the sky's kernels hold ${MAX_SKY_LIGHT_SLOTS} stars' light slots, not ${stars.length}`,
    );
  }
  if (stars.length + planetshine.length > MAX_SKY_SOURCES) {
    throw new RangeError(
      `the sky's kernels hold ${MAX_SKY_SOURCES} sources, not ${stars.length + planetshine.length}`,
    );
  }
  // The camera from the body's centre in the lights' axes.
  const offsetM = rotate(conjugate(toBodyFixed), camera.positionM);
  const carry = (
    kind: SkySourceKind,
    index: number,
    fromCentreM: Vec3,
    radiusM: number,
    illuminanceAtCentre: Rgb,
  ): Carried => {
    const fromCameraM = sub(fromCentreM, offsetM);
    const distanceM = norm(fromCameraM);
    const square = (norm(fromCentreM) / distanceM) ** 2;
    return {
      kind,
      index,
      direction: scale(rotate(toBodyFixed, fromCameraM), 1 / distanceM),
      illuminanceLx: [
        illuminanceAtCentre[0] * square,
        illuminanceAtCentre[1] * square,
        illuminanceAtCentre[2] * square,
      ],
      angularRadiusRad: Math.asin(Math.min(radiusM / distanceM, 1)),
    };
  };
  const carried: Carried[] = [
    ...stars.map((light, i) =>
      carry("star", i, light.toStarM, light.host.disc.radius_m, light.illuminance),
    ),
    ...planetshine.map((source, i) =>
      carry(
        "planetshine",
        i,
        scale(source.direction, source.distanceM),
        source.radiusM,
        source.illuminance,
      ),
    ),
  ];
  const geodetic = geodeticOf(camera.positionM, figure);
  const inside = geodetic.heightM < medium.topHeightM;
  const limitRad = twilightLimitRad(figure, geodetic.heightM, medium.topHeightM);
  const elevationOf = (source: Carried): number =>
    Math.asin(Math.min(Math.max(dot(source.direction, geodetic.normal), -1), 1));
  const brightestUp = Math.max(
    0,
    ...carried
      .filter((source) => elevationOf(source) > 0)
      .map((source) => photopicIlluminance(source.illuminanceLx)),
  );
  return carried.map((source) => {
    let skip: SkySkip = null;
    if (inside && elevationOf(source) + source.angularRadiusRad < -limitRad) {
      skip = "below twilight limit";
    } else if (
      inside &&
      brightestUp > 0 &&
      photopicIlluminance(source.illuminanceLx) < STAR_CUT_RELATIVE * brightestUp
    ) {
      skip = "negligible";
    }
    return {
      source: {
        kind: source.kind,
        index: source.index,
        directionBodyFixed: source.direction,
        illuminanceLx: source.illuminanceLx,
        angularRadiusRad: source.angularRadiusRad,
        curveSlot: source.kind === "star" ? source.index : 0,
        drawsDisc: false,
      },
      skip,
    };
  });
}

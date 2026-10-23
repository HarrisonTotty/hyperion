/**
 * The geometry sets of the reference tracer's cases (plan R08, R08.T12.b.1; Design note 10's set
 * and `decision-r08-spheroid.md`'s): observers, views and suns in the tracer's own terms, each in
 * its image.
 *
 * @remarks
 * A geometry is written as `hyperion-fit`'s case format reads it (`atmosphere/case.rs`): an
 * observer on the meridian of longitude 0 at a geodetic height and latitude, a view and each sun
 * as a zenith angle from the local vertical and an azimuth from local north towards east, in the
 * observer's own frame, the body's centre the origin and its axis z, and the local frame up
 * (cos φ, 0, sin φ), north (−sin φ, 0, cos φ) and east (0, 1, 0) (`atmosphere/geometry.rs`'s
 * `LocalFrame`). A sun's direction is the same at every point, since it is at infinity.
 *
 * Each geometry names its **image** (Design note 10's I(g), the metric ruling): the geometries that
 * share its observer and its suns. On the ground, one sun zenith with every view; in orbit, the
 * limb heights at one sun, and each disc phase alone; aerial perspective, its paths at one sun.
 * `gate.ts` takes each image's brightest diffuse radiance as the floor's L_max.
 */

import type { Vec3 } from "../../../geometry/vec3";
import { geodeticOf, type SpheroidFigure } from "../hillaire";

/** A direction in an observer's local frame, degrees. */
export interface LocalDirection {
  /** From the local vertical, in [0, 180]. */
  readonly zenithDeg: number;
  /** From local north towards east. */
  readonly azimuthDeg: number;
}

/** Where an observer stands: on the meridian of longitude 0. */
export interface ReferenceObserver {
  /** The geodetic height above the datum, m: 0 on the ground. */
  readonly heightM: number;
  /** The geodetic latitude, degrees. */
  readonly latitudeDeg: number;
}

/** One radiance a case traces, in the tracer's terms, with its image. */
export interface ReferenceGeometry {
  /** Unique within its case: the reference's key for it. */
  readonly name: string;
  /** Its image, I(g) (the module's remarks). */
  readonly image: string;
  readonly observer: ReferenceObserver;
  readonly view: LocalDirection;
  /** Where the case's one sun is. */
  readonly sun: LocalDirection;
  /** The view ray ends here at a black target, m: aerial perspective only. */
  readonly maxDistanceM?: number;
  /** The detector's half-angle, degrees, where it is not the case's. */
  readonly detectorHalfAngleDeg?: number;
}

/** A pair of flux aggregates: the downwelling flux at a ground point and the plane albedo above. */
export interface ReferenceAggregate {
  readonly name: string;
  /** The ground point's geodetic latitude, degrees. */
  readonly latitudeDeg: number;
  readonly sun: LocalDirection;
}

/** Design note 10's view zeniths for the ground, degrees. */
export const GROUND_VIEW_ZENITHS_DEG: ReadonlyArray<number> = [0, 30, 60, 75, 85];

/** Design note 10's relative azimuths for the ground, degrees: the view's less the sun's. */
export const GROUND_RELATIVE_AZIMUTHS_DEG: ReadonlyArray<number> = [0, 45, 90, 135, 180];

/** Design note 10's sun zeniths for the ground, degrees. */
export const GROUND_SUN_ZENITHS_DEG: ReadonlyArray<number> = [0, 30, 60, 80, 90, 95];

/**
 * The least angle between a view's centre and a sun, degrees: 3° (Design note 10: no geometry
 * lies within 3° of a sun, so that the comparison is of diffuse radiance alone). R08.T13's Mars
 * points at exactly 3° stand, with a 0.5° cone.
 */
export const LEAST_SUN_DISTANCE_DEG = 3;

/**
 * A ground observer's height above the datum, m: 1, the least camera height the client's sky view
 * takes (`hillaire.ts`'s `skyViewHeight`), so that a sun on the horizon is not hidden or shown by
 * rounding at the datum. On Saturn's spheroid a ray that starts on the datum and runs along it at
 * 60° otherwise met the ground in the tracer and not in the client's march (R08.T12.b.1).
 */
export const GROUND_OBSERVER_HEIGHT_M = 1;

/** The cases' detector cone, a half-angle, degrees: Design note 10's 0.5°–1°, at its narrow end. */
export const REFERENCE_DETECTOR_HALF_ANGLE_DEG = 0.5;

const RAD = Math.PI / 180;

/** A vector in the body's frame: a position, m, or a unit direction. */
type Vec = Vec3;

/** The local frame at geodetic latitude φ on the meridian of longitude 0 (the module's remarks). */
interface Frame {
  readonly up: Vec;
  readonly north: Vec;
  readonly east: Vec;
}

function frameAt(latitudeRad: number): Frame {
  const s = Math.sin(latitudeRad);
  const c = Math.cos(latitudeRad);
  return { up: { x: c, y: 0, z: s }, north: { x: -s, y: 0, z: c }, east: { x: 0, y: 1, z: 0 } };
}

function dotOf(a: Vec, b: Vec): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** The unit vector at a local direction in `frame`. */
function directionIn(frame: Frame, direction: LocalDirection): Vec {
  const zenith = direction.zenithDeg * RAD;
  const azimuth = direction.azimuthDeg * RAD;
  const s = Math.sin(zenith);
  const c = Math.cos(zenith);
  const north = Math.cos(azimuth) * s;
  const east = Math.sin(azimuth) * s;
  return {
    x: frame.up.x * c + frame.north.x * north + frame.east.x * east,
    y: frame.up.y * c + frame.north.y * north + frame.east.y * east,
    z: frame.up.z * c + frame.north.z * north + frame.east.z * east,
  };
}

/** A unit vector as a local direction in `frame`, the azimuth in [0, 360). */
function localOf(frame: Frame, v: Vec): LocalDirection {
  const up = Math.min(Math.max(dotOf(v, frame.up), -1), 1);
  const azimuth = Math.atan2(dotOf(v, frame.east), dotOf(v, frame.north)) / RAD;
  return { zenithDeg: Math.acos(up) / RAD, azimuthDeg: azimuth < 0 ? azimuth + 360 : azimuth };
}

/**
 * The unit vector in the body's frame of a local direction at geodetic latitude `latitudeDeg` on
 * the meridian of longitude 0, as the tracer's `LocalFrame` turns it.
 */
export function localDirectionVector(latitudeDeg: number, direction: LocalDirection): Vec3 {
  return directionIn(frameAt(latitudeDeg * RAD), direction);
}

/** The angle between two local directions in one frame, degrees. */
export function angleBetweenDeg(a: LocalDirection, b: LocalDirection): number {
  const frame = frameAt(0);
  const cos = dotOf(directionIn(frame, a), directionIn(frame, b));
  return Math.acos(Math.min(Math.max(cos, -1), 1)) / RAD;
}

/**
 * The point at geodetic latitude φ and height h on the meridian of longitude 0 of the spheroid
 * (a, a, c), m: ((N + h) cos φ, 0, (N(1 − e²) + h) sin φ), N = a ÷ √(1 − e² sin²φ) (NIMA TR8350.2,
 * eqs. 4-14 and 4-15); on a sphere, (R + h)(cos φ, 0, sin φ).
 */
export function geodeticPoint(figure: SpheroidFigure, latitudeDeg: number, heightM: number): Vec3 {
  return pointAt(figure, latitudeDeg * RAD, heightM);
}

function pointAt(figure: SpheroidFigure, latitudeRad: number, heightM: number): Vec {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const e2 = ((a - c) * (a + c)) / (a * a);
  const s = Math.sin(latitudeRad);
  const n = a / Math.sqrt(1 - e2 * s * s);
  return { x: (n + heightM) * Math.cos(latitudeRad), y: 0, z: (n * (1 - e2) + heightM) * s };
}

/** A geometry in the tracer's terms before its names. */
type Placed = Pick<ReferenceGeometry, "observer" | "view" | "sun">;

/**
 * A view from orbit tangent to the limb (plan R08, R08.T12.b.1): the line of sight horizontal at
 * the tangent point T, at geodetic latitude `tangentLatitudeDeg` and height `tangentHeightM`, along
 * `lineOfSightAzimuthDeg` in T's frame, seen from `distanceM` back along it, and the sun at
 * `sunAtTangent` in T's frame; the observer, its view and the sun then in the observer's own frame,
 * the whole turned about the axis so that the observer stands on the meridian of longitude 0.
 *
 * @remarks
 * On a spheroid the height along a ray is stationary where the ray is horizontal, so T is the
 * line of sight's lowest point in geodetic height. A limb at n scale heights stands at the geodetic
 * height n H ÷ s(φ) there (R08.T12.d's record), the caller's to give.
 */
export function limbGeometry(
  figure: SpheroidFigure,
  tangentLatitudeDeg: number,
  tangentHeightM: number,
  lineOfSightAzimuthDeg: number,
  sunAtTangent: LocalDirection,
  distanceM: number,
): Placed {
  const tangentFrame = frameAt(tangentLatitudeDeg * RAD);
  const tangent = pointAt(figure, tangentLatitudeDeg * RAD, tangentHeightM);
  const look = directionIn(tangentFrame, { zenithDeg: 90, azimuthDeg: lineOfSightAzimuthDeg });
  const sun = directionIn(tangentFrame, sunAtTangent);
  const observer = {
    x: tangent.x - distanceM * look.x,
    y: tangent.y - distanceM * look.y,
    z: tangent.z - distanceM * look.z,
  };
  // Turn about z by the observer's longitude, so that it stands on the meridian of longitude 0.
  const longitude = Math.atan2(observer.y, observer.x);
  const cos = Math.cos(-longitude);
  const sin = Math.sin(-longitude);
  const turned = (v: Vec): Vec => ({ x: v.x * cos - v.y * sin, y: v.x * sin + v.y * cos, z: v.z });
  const position = turned(observer);
  const geodetic = geodeticOf(position, figure);
  const frame = frameAt(geodetic.latitudeRad);
  return {
    observer: { heightM: geodetic.heightM, latitudeDeg: geodetic.latitudeRad / RAD },
    view: localOf(frame, turned(look)),
    sun: localOf(frame, turned(sun)),
  };
}

/** A number as it reads in a geometry's name: at most three decimals, no trailing zeros. */
function label(value: number): string {
  return String(Math.round(value * 1000) / 1000);
}

/**
 * Design note 10's ground set at one observer: every view zenith of {@link GROUND_VIEW_ZENITHS_DEG}
 * at each of `viewAzimuthsDeg`, under each sun zenith of {@link GROUND_SUN_ZENITHS_DEG} at
 * `sunAzimuthDeg`, one image a sun zenith, less those within {@link LEAST_SUN_DISTANCE_DEG} of the
 * sun; the azimuths collapse to their first at a view zenith of 0°, and at a sun zenith of 0° where
 * `collapseUnderZenithSun` (on a sphere, or at a spheroid's pole), where the azimuth means nothing.
 *
 * @param prefix - The site's name, which starts each image's and geometry's name.
 */
export function groundSet(
  prefix: string,
  observer: ReferenceObserver,
  viewAzimuthsDeg: ReadonlyArray<number>,
  sunAzimuthDeg: number,
  collapseUnderZenithSun: boolean,
): ReferenceGeometry[] {
  const set: ReferenceGeometry[] = [];
  for (const sunZenith of GROUND_SUN_ZENITHS_DEG) {
    const sun = { zenithDeg: sunZenith, azimuthDeg: sunAzimuthDeg };
    for (const viewZenith of GROUND_VIEW_ZENITHS_DEG) {
      const collapsed = viewZenith === 0 || (sunZenith === 0 && collapseUnderZenithSun);
      const azimuths = collapsed ? viewAzimuthsDeg.slice(0, 1) : viewAzimuthsDeg;
      for (const azimuth of azimuths) {
        const view = { zenithDeg: viewZenith, azimuthDeg: azimuth };
        if (angleBetweenDeg(view, sun) < LEAST_SUN_DISTANCE_DEG - 1e-9) {
          continue;
        }
        set.push({
          name: `${prefix}-s${label(sunZenith)}-v${label(viewZenith)}-a${label(azimuth)}`,
          image: `${prefix}-s${label(sunZenith)}`,
          observer,
          view,
          sun,
        });
      }
    }
  }
  return set;
}

/** Design note 10's ground set on a sphere: one observer, the sun at azimuth 0, every azimuth. */
export function sphereGroundSet(): ReferenceGeometry[] {
  return groundSet(
    "ground",
    { heightM: GROUND_OBSERVER_HEIGHT_M, latitudeDeg: 0 },
    GROUND_RELATIVE_AZIMUTHS_DEG,
    0,
    true,
  );
}

/** Design note 10's disc phases chosen for the cases, degrees (the module's remarks). */
export const DISC_PHASES_DEG: ReadonlyArray<number> = [0, 30, 60, 75, 85];

/**
 * The disc at each of {@link DISC_PHASES_DEG}: the sub-observer point seen straight down from
 * `orbitHeightM`, its sun at the phase angle from its zenith, so that the phase is the angle at the
 * point between the sun and the observer. Each is an image alone.
 */
export function discSet(orbitHeightM: number): ReferenceGeometry[] {
  return DISC_PHASES_DEG.map((phase) => ({
    name: `disc-p${label(phase)}`,
    image: `disc-p${label(phase)}`,
    observer: { heightM: orbitHeightM, latitudeDeg: 0 },
    view: { zenithDeg: 180, azimuthDeg: 0 },
    sun: { zenithDeg: phase, azimuthDeg: 0 },
  }));
}

/** Design note 10's limb heights, in scale heights. */
export const SPHERE_LIMB_SCALE_HEIGHTS: ReadonlyArray<number> = [0.1, 0.3, 1, 3, 10];

/**
 * The sun at a sphere's limb's tangent point, chosen for the cases (the module's remarks): 60°
 * from its zenith and at right angles to the line of sight, which runs north there, so that the
 * tangent point is lit and the light singly scattered to the observer turns through 90°.
 */
export const SPHERE_LIMB_SUN: LocalDirection = { zenithDeg: 60, azimuthDeg: 90 };

/**
 * Design note 10's limb on a sphere of radius `radiusM`: tangent at
 * {@link SPHERE_LIMB_SCALE_HEIGHTS} times `scaleHeightM` over the equator, seen looking north from
 * `orbitHeightM`, under {@link SPHERE_LIMB_SUN}: one image, the limb heights at one sun.
 */
export function sphereLimbSet(
  radiusM: number,
  scaleHeightM: number,
  orbitHeightM: number,
): ReferenceGeometry[] {
  const figure = { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
  return SPHERE_LIMB_SCALE_HEIGHTS.map((n) => {
    const tangentHeightM = n * scaleHeightM;
    const distanceM = Math.sqrt((radiusM + orbitHeightM) ** 2 - (radiusM + tangentHeightM) ** 2);
    const placed = limbGeometry(figure, 0, tangentHeightM, 0, SPHERE_LIMB_SUN, distanceM);
    return {
      name: `limb-${label(n)}H`,
      image: "limb",
      observer: placed.observer,
      view: placed.view,
      sun: placed.sun,
    };
  });
}

/** Design note 10's aerial perspective paths, m. */
export const AERIAL_DISTANCES_M: ReadonlyArray<number> = [1_000, 10_000, 32_000];

/** The aerial perspective's observer height above the ground, m: 2, about the eye's. */
export const AERIAL_OBSERVER_HEIGHT_M = 2;

/**
 * The aerial perspective's view and sun, chosen for the cases (the module's remarks): looking east
 * along the horizon, the sun 60° from the zenith in the north, at right angles to the view.
 */
export const AERIAL_VIEW: LocalDirection = { zenithDeg: 90, azimuthDeg: 90 };

/** {@link AERIAL_VIEW}'s sun. */
export const AERIAL_SUN: LocalDirection = { zenithDeg: 60, azimuthDeg: 0 };

/** Design note 10's aerial perspective: {@link AERIAL_DISTANCES_M} to a black target, one image. */
export function aerialSet(): ReferenceGeometry[] {
  return AERIAL_DISTANCES_M.map((distance) => ({
    name: `aerial-${label(distance / 1000)}km`,
    image: "aerial",
    observer: { heightM: AERIAL_OBSERVER_HEIGHT_M, latitudeDeg: 0 },
    view: AERIAL_VIEW,
    sun: AERIAL_SUN,
    maxDistanceM: distance,
  }));
}

/**
 * Design note 10's aggregates: one for each ground image's sun zenith below 90° (0°, 30°, 60° and
 * 80°), at the ground observer's latitude, 0°.
 */
export function sphereAggregates(): ReferenceAggregate[] {
  return GROUND_SUN_ZENITHS_DEG.filter((zenith) => zenith < 90).map((zenith) => ({
    name: `flux-s${label(zenith)}`,
    latitudeDeg: 0,
    sun: { zenithDeg: zenith, azimuthDeg: 0 },
  }));
}

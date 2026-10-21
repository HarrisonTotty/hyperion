/**
 * The hand-over check's geometry (plan R10, R10.T10.g; decision-r10-t16-leans §1): its three looks,
 * the camera's line through the hand-over and back, the body's fixed rotation that sets its
 * survey's middle at each look's incidence and emission, and the cells the frames' pixels are
 * judged by.
 *
 * @remarks
 * - **The looks** ({@link HANDOVER_LOOKS}): the survey's middle at incidence i from its star and
 *   emission e from the camera, measured from its datum normal, the sun, the normal and the camera
 *   in one plane, so that the phase is α = i + e: (15°, 15°) and (45°, 45°), judged (T10.e's 30°
 *   and 90°, the normal bisecting), and (78°, 12°), the terminator's, recorded.
 * - **The rotation** ({@link lookGeometry}): the body is held still (its rotation fixed for the
 *   look) so that its survey's middle's datum normal points at i from the star in the plane of the
 *   galactic x and y axes, and the body's north at that point along galactic +z. The star lies
 *   along −x from the body (the kept scenes' star at the barycentre, the body 1 au along +x).
 * - **The camera's line** ({@link limbRatioAt}): on the line to the body's centre, looking at the
 *   centre, from {@link HandoverApproach.startRatio} times the hand-over range (the camera's
 *   distance to the limb, as `handsOver` measures it) closing at {@link HandoverApproach.ratePerS} of
 *   that distance a second to {@link HandoverApproach.closestRatio} of the range, held there
 *   {@link HandoverApproach.holdS}, then backing out at the same rate to
 *   {@link HandoverApproach.endRatio}: T10.e's scratch harness's line, whose 1% a second is 0.91% of
 *   the range a second at the hand-over (range ÷ 1.1) and 1.11% at the hand-back (range ÷ 0.9).
 * - **The camera's direction** is solved, not taken far off: a camera at a finite distance sees
 *   the survey's middle from a direction tilted away from its centre's by about R sin e ÷ D (4.5°
 *   at 90° on the high setting's range, 12° on the low's), so the line's direction is the one
 *   from which the camera at the hand-over range sees the survey's middle at exactly e. At the
 *   switches, at range ÷ 1.1 and ÷ 0.9, the angles drift from the nominal by about a tenth of that
 *   tilt; each frame's record states its own ({@link lookAngles}).
 * - **The cells** ({@link texelOfDirection}): a direction's cell at a level, as R07's class map
 *   (`classMapTexelOf`, the disc shader's twin) finds its texel, numbered as the coverage mask's
 *   texels (face, then row j, then column i), the probe's `coverageCells`; the mapping's
 *   self-check ({@link texelMappingMismatches}) takes each cell's centre back to its cell.
 */

import type { HandoverAngles, HandoverLookName } from "../../../../../preload/api";
import { add, cross, dot, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { classMapTexelOf } from "../../../view/bodies/discSurface";
import type { Quaternion } from "../../../view/camera/pose";
import { lookAlong, rotate } from "../../../view/camera/quaternion";
import { type Rotation3, rotateToBody, rotation3FromRows } from "../../../view/coords/rotation";
import { PATCH_QUADS, vertexDir } from "../../../view/terrain/cube";
import { FACES } from "../../../view/terrain/patchKey";

/** Degrees in a radian. */
const DEG_PER_RAD = 180 / Math.PI;

/** One of the hand-over check's looks. */
export interface HandoverLook {
  readonly name: HandoverLookName;
  /** Its check-only kept scene's name, as `SCENE` shows it. */
  readonly scene: string;
  /** The survey's middle's incidence from its star, rad. */
  readonly incidenceRad: number;
  /** Its emission toward the camera at the hand-over range, rad. */
  readonly emissionRad: number;
  /** Whether the verdict judges its figures (30° and 90°), or records them (the terminator's). */
  readonly judged: boolean;
}

/** `deg` in radians. */
function rad(deg: number): number {
  return deg / DEG_PER_RAD;
}

/**
 * The three looks, in the order the run takes them (decision-r10-t16-leans §1; plan 10's T10.g:
 * "the harness at 30°, 90° and a view near the terminator (incidence 75° or more)").
 */
export const HANDOVER_LOOKS: ReadonlyArray<HandoverLook> = [
  {
    name: "handover-30",
    scene: "HAND-OVER 30°",
    incidenceRad: rad(15),
    emissionRad: rad(15),
    judged: true,
  },
  {
    name: "handover-90",
    scene: "HAND-OVER 90°",
    incidenceRad: rad(45),
    emissionRad: rad(45),
    judged: true,
  },
  {
    name: "handover-terminator",
    scene: "HAND-OVER TERMINATOR",
    incidenceRad: rad(78),
    emissionRad: rad(12),
    judged: false,
  },
];

/** The camera's line through a look: see the module's remarks. */
export interface HandoverApproach {
  /** The closing and opening rate, 1/s: the fraction of the camera's distance to the limb a second. */
  readonly ratePerS: number;
  /** The camera's distance to the limb at the script's start, over the hand-over range. */
  readonly startRatio: number;
  /** At its closest, over the range: inside the hand-over at range ÷ 1.1. */
  readonly closestRatio: number;
  /** How long it holds at its closest, s. */
  readonly holdS: number;
  /** At the script's end, over the range: outside the hand-back at range ÷ 0.9. */
  readonly endRatio: number;
}

/**
 * A full run's line, T10.e's scratch harness's (1% a second, 3 to 0.85 of the range, a 12 s hold,
 * out to 1.25: about 3 minutes a look), and a smoke's, five times as fast with a 3 s hold (about
 * 40 s a look), which proves the harness and not the photometry.
 */
export const HANDOVER_APPROACH: {
  readonly full: HandoverApproach;
  readonly smoke: HandoverApproach;
} = {
  full: { ratePerS: 0.01, startRatio: 3, closestRatio: 0.85, holdS: 12, endRatio: 1.25 },
  smoke: { ratePerS: 0.05, startRatio: 3, closestRatio: 0.85, holdS: 3, endRatio: 1.25 },
};

/** How long the approach closes, s. */
function closingS(approach: HandoverApproach): number {
  return Math.log(approach.startRatio / approach.closestRatio) / approach.ratePerS;
}

/** How long it backs out, s. */
function openingS(approach: HandoverApproach): number {
  return Math.log(approach.endRatio / approach.closestRatio) / approach.ratePerS;
}

/** The script's length, s: the closing, the hold and the backing out. */
export function approachDurationS(approach: HandoverApproach): number {
  return closingS(approach) + approach.holdS + openingS(approach);
}

/** The part of the line at the script's time `tS`: closing, holding or backing out. */
export function approachPart(approach: HandoverApproach, tS: number): "in" | "hold" | "out" {
  const inS = closingS(approach);
  return tS <= inS ? "in" : tS <= inS + approach.holdS ? "hold" : "out";
}

/** The camera's distance to the limb over the hand-over range at the script's time `tS`. */
export function limbRatioAt(approach: HandoverApproach, tS: number): number {
  const inS = closingS(approach);
  let ratio: number;
  switch (approachPart(approach, tS)) {
    case "in":
      ratio = approach.startRatio * Math.exp(-approach.ratePerS * Math.max(tS, 0));
      break;
    case "hold":
      ratio = approach.closestRatio;
      break;
    case "out":
      ratio = approach.closestRatio * Math.exp(approach.ratePerS * (tS - inS - approach.holdS));
      break;
  }
  return ratio;
}

/** The script's time at which the closing camera reaches `ratio` of the range, s. */
export function closingTimeS(approach: HandoverApproach, ratio: number): number {
  return Math.log(approach.startRatio / ratio) / approach.ratePerS;
}

/** A body's datum: the spheroid of its equatorial and polar radii, m. */
export interface HandoverFigure {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
}

/** The point of the datum along the body-fixed direction `direction`, m from the centre. */
export function datumPointM(direction: Vec3, figure: HandoverFigure): Vec3 {
  const d = normalise(direction);
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const radiusM = 1 / Math.sqrt((d.x * d.x + d.y * d.y) / (a * a) + (d.z * d.z) / (c * c));
  return scale(d, radiusM);
}

/** The datum's outward unit normal at its point `pointM`, body-fixed. */
export function datumNormal(pointM: Vec3, figure: HandoverFigure): Vec3 {
  const a2 = figure.equatorialRadiusM * figure.equatorialRadiusM;
  const c2 = figure.polarRadiusM * figure.polarRadiusM;
  return normalise(vec3(pointM.x / a2, pointM.y / a2, pointM.z / c2));
}

/** The camera's distance from the body's centre at a distance `limbM` to its limb, m. */
export function centreDistanceM(limbM: number, figure: HandoverFigure): number {
  return Math.sqrt(limbM * limbM + figure.equatorialRadiusM * figure.equatorialRadiusM);
}

/** The galactic +z, the look's north at the survey's middle. */
const GALACTIC_Z = vec3(0, 0, 1);

/** What fixes a look's geometry besides the look: the body, its survey and the range. */
export interface LookGeometryInput {
  readonly figure: HandoverFigure;
  /** The survey's middle, a body-fixed direction (any length). */
  readonly surveyMiddle: Vec3;
  /** The body's star from the body's centre, m along the galactic axes; its unit is the sun's −x. */
  readonly towardStarM: Vec3;
  /** The hand-over range, m, against the camera's distance to the limb. */
  readonly rangeM: number;
}

/** A look's geometry: see the module's remarks. */
export interface LookGeometry {
  /** The body's fixed rotation, body-fixed to galactic axes. */
  readonly rotation: Rotation3;
  /** The body's pole along the galactic axes. */
  readonly pole: Vec3;
  /** The unit direction from the body's centre to the camera's line, galactic. */
  readonly towardCamera: Vec3;
  /** The survey's middle on the datum, body-fixed, m from the centre, and its datum normal. */
  readonly surveyPointM: Vec3;
  readonly surveyNormal: Vec3;
  /** The body's star from its centre, galactic, m. */
  readonly towardStarM: Vec3;
  readonly rangeM: number;
  readonly figure: HandoverFigure;
}

/** The angle between two vectors of any length, rad, by atan2 (exact near 0 and π). */
function angleBetween(a: Vec3, b: Vec3): number {
  const c = cross(a, b);
  return Math.atan2(Math.sqrt(dot(c, c)), dot(a, b));
}

/**
 * The angles at the survey's middle seen from a camera at `cameraFromCentreM` (galactic, m from the
 * body's centre): the star's incidence and the camera's emission from the datum normal, and the
 * phase between them, rad.
 */
export function lookAnglesRad(
  geometry: LookGeometry,
  cameraFromCentreM: Vec3,
): { readonly incidenceRad: number; readonly emissionRad: number; readonly phaseRad: number } {
  const point = rotateToBody(geometry.rotation, geometry.surveyPointM);
  const normal = rotateToBody(geometry.rotation, geometry.surveyNormal);
  const toStar = sub(geometry.towardStarM, point);
  const toCamera = sub(cameraFromCentreM, point);
  return {
    incidenceRad: angleBetween(normal, toStar),
    emissionRad: angleBetween(normal, toCamera),
    phaseRad: angleBetween(toStar, toCamera),
  };
}

/** {@link lookAnglesRad} in degrees, as the record holds them. */
export function lookAngles(geometry: LookGeometry, cameraFromCentreM: Vec3): HandoverAngles {
  const angles = lookAnglesRad(geometry, cameraFromCentreM);
  return {
    incidenceDeg: angles.incidenceRad * DEG_PER_RAD,
    emissionDeg: angles.emissionRad * DEG_PER_RAD,
    phaseDeg: angles.phaseRad * DEG_PER_RAD,
  };
}

/** The camera's place on the look's line at `limbRatio` of the range, galactic, m from the centre. */
export function cameraOnLineM(geometry: LookGeometry, limbRatio: number): Vec3 {
  return scale(
    geometry.towardCamera,
    centreDistanceM(limbRatio * geometry.rangeM, geometry.figure),
  );
}

/** The own ship's attitude on the look's line: looking at the body's centre, galactic +z up. */
export function lookAttitude(geometry: LookGeometry): Quaternion {
  return lookAlong(scale(geometry.towardCamera, -1), GALACTIC_Z);
}

/**
 * The seat camera's eye at `limbRatio` of the range, galactic, m from the body's centre: the ship's
 * place on the line and its hull's eye point `eyePointM` turned by its attitude.
 */
export function eyeOnLineM(geometry: LookGeometry, limbRatio: number, eyePointM: Vec3): Vec3 {
  return add(cameraOnLineM(geometry, limbRatio), rotate(lookAttitude(geometry), eyePointM));
}

/** Bisection steps for the camera's direction: far below a nanoradian over a bracket under π. */
const BISECTION_STEPS = 64;

/** `v`'s direction in the plane of the galactic x and y axes. */
function inPlane(v: Vec3): Vec3 {
  return normalise(vec3(v.x, v.y, 0));
}

/**
 * The look's geometry: the rotation that puts the survey's middle's datum normal at the look's
 * incidence in the galactic x–y plane and its north along +z, and the direction of the camera's
 * line from which, at the hand-over range, the camera sees the survey's middle at the look's
 * emission, beyond the normal from the star (see the module's remarks).
 *
 * @throws Error for a range or a figure that is not positive, or a survey's middle with no length.
 */
export function lookGeometry(look: HandoverLook, input: LookGeometryInput): LookGeometry {
  const { figure, rangeM } = input;
  if (
    !(rangeM > 0) ||
    !(figure.equatorialRadiusM > 0) ||
    !(figure.polarRadiusM > 0) ||
    !(dot(input.surveyMiddle, input.surveyMiddle) > 0)
  ) {
    throw new Error("a hand-over look needs a positive range and figure and a survey's middle");
  }
  const surveyPointM = datumPointM(input.surveyMiddle, figure);
  const surveyNormal = datumNormal(surveyPointM, figure);
  // The body-fixed frame at the survey's middle (its normal, north and east) taken to the look's
  // (the normal at i from the sun in the x–y plane, galactic +z, and their cross product).
  const e1 = surveyNormal;
  const e2 = normalise(sub(GALACTIC_Z, scale(e1, dot(GALACTIC_Z, e1))));
  const e3 = cross(e1, e2);
  const turnFor = (sun: Vec3): Rotation3 => {
    // In the plane of x and y, a right angle from the sun: +y for a star along −x.
    const side = normalise(cross(sun, GALACTIC_Z));
    const f1 = add(
      scale(sun, Math.cos(look.incidenceRad)),
      scale(side, Math.sin(look.incidenceRad)),
    );
    const f2 = GALACTIC_Z;
    const f3 = cross(f1, f2);
    const row = (k: "x" | "y" | "z"): Vec3 =>
      vec3(
        f1[k] * e1.x + f2[k] * e2.x + f3[k] * e3.x,
        f1[k] * e1.y + f2[k] * e2.y + f3[k] * e3.y,
        f1[k] * e1.z + f2[k] * e2.z + f3[k] * e3.z,
      );
    return rotation3FromRows([row("x"), row("y"), row("z")]);
  };
  // The sun as the survey's middle sees it, in the x–y plane: from the centre first, then from the
  // middle the turn puts out there, R ÷ 1 au off it (1 × 10⁻⁵ rad); twice is exact to 10⁻¹⁰.
  let sun = inPlane(input.towardStarM);
  for (let pass = 0; pass < 2; pass += 1) {
    sun = inPlane(sub(input.towardStarM, rotateToBody(turnFor(sun), surveyPointM)));
  }
  const rotation = turnFor(sun);
  const side = normalise(cross(sun, GALACTIC_Z));
  const at = (thetaRad: number): LookGeometry => ({
    rotation,
    pole: rotateToBody(rotation, GALACTIC_Z),
    towardCamera: add(scale(sun, Math.cos(thetaRad)), scale(side, Math.sin(thetaRad))),
    surveyPointM,
    surveyNormal,
    towardStarM: input.towardStarM,
    rangeM,
    figure,
  });
  // The emission at the range grows with the line's angle from the sun, θ, from about 0 at θ = i
  // (the line along the normal) past e at θ = i + e (the parallax adds to it).
  const emissionAt = (thetaRad: number): number => {
    const geometry = at(thetaRad);
    return lookAnglesRad(geometry, cameraOnLineM(geometry, 1)).emissionRad;
  };
  let low = look.incidenceRad;
  let high = look.incidenceRad + look.emissionRad;
  for (let step = 0; step < BISECTION_STEPS; step += 1) {
    const middle = (low + high) / 2;
    if (emissionAt(middle) < look.emissionRad) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return at((low + high) / 2);
}

/**
 * The cell holding the body-fixed `direction` at `level`, as its coverage-mask texel: face × N² +
 * row j × N + column i, N = 2^level (the disc shader's `class_map_texel`, `classMapTexelOf`).
 */
export function texelOfDirection(direction: Vec3, level: number): number {
  const n = 2 ** level;
  const { face, i, j } = classMapTexelOf([direction.x, direction.y, direction.z], n);
  return (face * n + j) * n + i;
}

/**
 * The centre of the cell `texel` at `level`, a body-fixed unit direction (its patch's middle
 * vertex, `vertexDir`'s); `null` for a texel outside the level's six faces.
 */
export function cellCentreDirection(texel: number, level: number): Vec3 | null {
  const n = 2 ** level;
  const perFace = n * n;
  const face = FACES[Math.floor(texel / perFace)];
  if (face === undefined || !Number.isInteger(texel) || texel < 0) {
    return null;
  }
  const j = Math.floor((texel - face * perFace) / n);
  const i = texel - face * perFace - j * n;
  const [x, y, z] = vertexDir({ face, level, i, j }, PATCH_QUADS / 2, PATCH_QUADS / 2);
  return vec3(x, y, z);
}

/**
 * The mapping's self-check (T10.e's scratch harness's): how many of `cells` (coverage-mask texels at
 * `level`) do not hold their own centre's direction; 0 where the pixels' cells are the coverage's.
 */
export function texelMappingMismatches(cells: Iterable<number>, level: number): number {
  let mismatches = 0;
  for (const texel of cells) {
    const centre = cellCentreDirection(texel, level);
    if (centre === null || texelOfDirection(centre, level) !== texel) {
      mismatches += 1;
    }
  }
  return mismatches;
}

/**
 * Where the ray from `originM` along `direction` (body-fixed, m from the centre; any length) first
 * meets the datum from outside it, m from the centre; `null` where it misses or the datum is behind.
 */
export function rayDatumHitM(originM: Vec3, direction: Vec3, figure: HandoverFigure): Vec3 | null {
  // In axes stretched along z by a ÷ c the datum is the sphere of radius a.
  const k = figure.equatorialRadiusM / figure.polarRadiusM;
  const o = vec3(originM.x, originM.y, originM.z * k);
  const d = normalise(vec3(direction.x, direction.y, direction.z * k));
  const b = dot(o, d);
  const c = dot(o, o) - figure.equatorialRadiusM * figure.equatorialRadiusM;
  const discriminant = b * b - c;
  if (discriminant < 0) {
    return null;
  }
  const t = -b - Math.sqrt(discriminant);
  if (!(t > 0)) {
    return null;
  }
  const hit = add(o, scale(d, t));
  return vec3(hit.x, hit.y, hit.z / k);
}

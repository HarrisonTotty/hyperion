/**
 * The hand-over check's reading of a frame and its comparison of two (plan R10, R10.T10.g;
 * decision-r10-t16-leans §1).
 *
 * @remarks
 * - **A frame's reading** ({@link readFrame}): of the scene target's HDR, read back over a region
 *   about the body, each pixel's meter class (its alpha, an exact small integer) says whether the
 *   body's lit or unlit side is drawn there; its luminance is Rec. 709's of the linear red, green
 *   and blue (the meter's own weights, `METER_LUMA`); and its cell is the one the ray through the
 *   pixel's centre meets on the body's datum, from that frame's camera, kept where the cell is
 *   surveyed. A body's pixel whose ray misses the datum is counted (`bodyMisses`): the camera
 *   model's self-check, which the limb's relief alone may trip.
 * - **The comparison** ({@link compareReadings}): the body's and the survey's mean luminance over
 *   each frame's own pixels and over the pixels both share (the same pixel of the body, and for the
 *   survey the same pixel assigned to the same cell), means, not sums, so that the camera's closing
 *   between the frames moves no figure; and each surveyed cell's mean over the other frame's pixels
 *   on it, each against the disc's frame resampled at that pixel's own ground point
 *   ({@link resampledAt}), every cell with one such pixel or more listed. The judging is the main
 *   process's.
 * - **Why resampled** (plan R10, R10.T15.b.2): the camera closes between the two frames, by 0.25%
 *   of its distance in the software mode, whose frames carry the script 0.25 s each, and so each
 *   pixel's ground point moves within its cell, about 0.7 px at the served airless world's 90° look
 *   on the high setting. Where a cell's light changes steeply across it, within half a degree of
 *   the terminator, that alone moved the mean of the pixels both frames give a cell by over a third
 *   of a stop: about 0.5% of the cells of two identical smooth frames (a simulation of the look,
 *   R10.T15.b's record), all of them there. Comparing each pixel with the disc's light at its own
 *   ground point removes the closing to the interpolation's error, the disc's frame being the
 *   smooth one.
 * - **The μ floor** (plan R10, R10.T15.b; decision-r10-t16-leans, "R10.T10.g terminator
 *   cross-fade"): each surveyed cell a frame's pixels meet is flagged where the disc's floor on the
 *   law's μ binds at the cell's centre, seen from that frame's eye ({@link muFloorBinds}), and a cell
 *   flagged in either frame is marked in the comparison (`HandoverTexelRecord.muFloored`), for the
 *   main process to leave out of its per-cell bound and count. The means take every pixel as before.
 */

import type {
  HandoverOwnMeans,
  HandoverResampleGaps,
  HandoverSharedMeans,
  HandoverTexelRecord,
} from "../../../../../preload/api";
import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { type NormalMapTexels, normalMapNormalAt } from "../../../view/bodies/discNormals";
import type { Quaternion } from "../../../view/camera/pose";
import { rotate } from "../../../view/camera/quaternion";
import { rotateToBodyFixed } from "../../../view/coords/rotation";
import { METER_LUMA } from "../../../view/post/histogram";
import { METER_CLASS } from "../../../view/post/meter";
import {
  cellCentreDirection,
  datumNormal,
  datumPointM,
  type HandoverFigure,
  type LookGeometry,
  rayDatumHitM,
  texelOfDirection,
} from "./handoverGeometry";

/** A pixel's cell where the body is not drawn there. */
export const OFF_BODY = -2;

/** A pixel's cell where the body is drawn and its ray meets no surveyed cell. */
export const UNSURVEYED = -1;

/**
 * The share of the datum's μ_ν below which the disc floors the law's μ on its shading normal n′:
 * μ = max(n′ · v, ½ μ_ν) (plan R10, R10.T10.g; `bodyDisc.wgsl`'s `shade` and its twin
 * `discShading.ts`), so that a cell whose slope faces away near the limb never draws black.
 */
export const MU_FLOOR_SHARE = 0.5;

/**
 * Whether the disc's μ floor binds at the centre of the cell `texel` at `level`, seen from `eyeM`
 * (plan R10, R10.T15.b): n′ · v < ½ μ_ν, with ν the datum's normal at the cell's centre on the
 * datum, v the unit vector from there to the eye, μ_ν = ν · v, and n′ the normal map's shading normal
 * there by the disc's own reconstruction (`normalMapNormalAt`), ν where `normals` is `null` or holds
 * none there. With n′ = ν it binds only where μ_ν < 0: a cell whose centre lies past the limb.
 *
 * @param eyeM - The eye, body-fixed, m from the body's centre.
 * @returns `false` for a texel outside the level's six faces.
 */
export function muFloorBinds(
  texel: number,
  level: number,
  eyeM: Vec3,
  figure: HandoverFigure,
  normals: NormalMapTexels | null,
): boolean {
  const centre = cellCentreDirection(texel, level);
  if (centre === null) {
    return false;
  }
  const pointM = datumPointM(centre, figure);
  const datum = datumNormal(pointM, figure);
  const towardEye = normalise(sub(eyeM, pointM));
  const shading =
    normals === null ? null : normalMapNormalAt(normals, [centre.x, centre.y, centre.z]);
  return dot(shading ?? datum, towardEye) < MU_FLOOR_SHARE * dot(datum, towardEye);
}

/** A region of the scene target, px from its top left. */
export interface PixelRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** A frame's camera, along the body-fixed axes. */
export interface FrameCamera {
  /** The eye, m from the body's centre. */
  readonly eyeM: Vec3;
  /** The view's right, up and backward unit axes: it looks down −back (R02's Design note 4). */
  readonly right: Vec3;
  readonly up: Vec3;
  readonly back: Vec3;
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
}

/**
 * The camera of a frame drawn from `eyeFromCentreM` (galactic, m from the body's centre) at
 * `attitude` (the view's axes to the galactic), along the body's fixed axes.
 */
export function frameCamera(
  geometry: LookGeometry,
  eyeFromCentreM: Vec3,
  attitude: Quaternion,
  fovXRad: number,
): FrameCamera {
  const toFixed = (v: Vec3): Vec3 => rotateToBodyFixed(geometry.rotation, v);
  return {
    eyeM: toFixed(eyeFromCentreM),
    right: toFixed(rotate(attitude, vec3(1, 0, 0))),
    up: toFixed(rotate(attitude, vec3(0, 1, 0))),
    back: toFixed(rotate(attitude, vec3(0, 0, 1))),
    fovXRad,
  };
}

/** The whole pixels from `low` to `high` within [0, `size`), at least one. */
function clip(low: number, high: number, size: number): readonly [number, number] {
  const from = Math.max(0, Math.floor(low));
  const to = Math.min(size, Math.ceil(high));
  return [from, Math.max(to, from + 1)];
}

/**
 * The region of a `widthPx` × `heightPx` target that holds the body's disc, seen from `distanceM`
 * off its centre along the view's axis, with `marginPx` about it, clipped to the target; the whole
 * target where the disc fills it.
 *
 * @param radiusM - The body's largest radius, m: its equatorial.
 */
export function bodyRect(
  widthPx: number,
  heightPx: number,
  fovXRad: number,
  distanceM: number,
  radiusM: number,
  marginPx: number,
): PixelRect {
  const focalPx = widthPx / 2 / Math.tan(fovXRad / 2);
  const angular = distanceM > radiusM ? Math.asin(radiusM / distanceM) : Math.PI / 2;
  const radiusPx = angular < Math.PI / 2 ? focalPx * Math.tan(angular) + marginPx : Infinity;
  const [x0, x1] = clip(widthPx / 2 - radiusPx, widthPx / 2 + radiusPx, widthPx);
  const [y0, y1] = clip(heightPx / 2 - radiusPx, heightPx / 2 + radiusPx, heightPx);
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

/** Each half's value, by its 16 bits, made once a page. */
let halfValues: Float32Array | null = null;

/** The value of the IEEE 754 half `bits` (IEEE 754-2008, binary16: 1 sign, 5 exponent, 10 bits). */
function halfOf(bits: number): number {
  const sign = (bits & 0x8000) === 0 ? 1 : -1;
  const exponent = (bits >> 10) & 0x1f;
  const mantissa = bits & 0x3ff;
  if (exponent === 0) {
    return sign * 2 ** -14 * (mantissa / 1024);
  }
  if (exponent === 0x1f) {
    return mantissa === 0 ? sign * Infinity : Number.NaN;
  }
  return sign * 2 ** (exponent - 15) * (1 + mantissa / 1024);
}

/** The table of every half's value. */
function halves(): Float32Array {
  if (halfValues === null) {
    const table = new Float32Array(65_536);
    for (let bits = 0; bits < table.length; bits += 1) {
      table[bits] = halfOf(bits);
    }
    halfValues = table;
  }
  return halfValues;
}

/** One frame read over a region of its scene target. */
export interface FrameReading {
  /** The target's whole size, px. */
  readonly widthPx: number;
  readonly heightPx: number;
  /** The region read; the arrays below are its pixels, row after row. */
  readonly rect: PixelRect;
  /** Each pixel's Rec. 709 luminance, the HDR's units; 0 off the body. */
  readonly luminance: Float32Array;
  /** Each pixel's surveyed cell (a coverage-mask texel), {@link UNSURVEYED} or {@link OFF_BODY}. */
  readonly cells: Int32Array;
  /** The body's pixels; of those, the rays that miss the datum; and those on a surveyed cell. */
  readonly bodyPixels: number;
  readonly bodyMisses: number;
  readonly surveyPixels: number;
  /**
   * The surveyed cells its pixels meet where the disc's μ floor binds at the cell's centre, seen
   * from this frame's eye ({@link muFloorBinds}).
   */
  readonly floored: ReadonlySet<number>;
  /** Each pixel's meter class: 1 where the body's lit side is drawn, else 0. */
  readonly lit: Uint8Array;
  /** The frame's camera and the body's datum, by which its pixels' ground points are found. */
  readonly camera: FrameCamera;
  readonly figure: HandoverFigure;
}

/** What {@link readFrame} reads. */
export interface FrameReadInput {
  /** The region's `rgba16float` texels, rows tightly packed, as half bits. */
  readonly halfBits: Uint16Array;
  readonly widthPx: number;
  readonly heightPx: number;
  readonly rect: PixelRect;
  readonly camera: FrameCamera;
  readonly figure: HandoverFigure;
  /** The level of the cells. */
  readonly level: number;
  /** The surveyed cells, as coverage-mask texels at `level`. */
  readonly surveyed: ReadonlySet<number>;
  /**
   * The disc's normal map as the frame was drawn with it (the class map's, R10.T10.g), at `level`,
   * whose shading normals flag the μ floor ({@link FrameReading.floored}); `null` for none, where
   * the disc shades on the datum's normal.
   */
  readonly normals: NormalMapTexels | null;
}

/**
 * Reads a frame: see the module's remarks.
 *
 * @throws Error where the texels do not fill the region, or the region leaves the target.
 */
export function readFrame(input: FrameReadInput): FrameReading {
  const { rect, widthPx, heightPx, camera, figure, level, surveyed } = input;
  const count = rect.width * rect.height;
  if (
    rect.x < 0 ||
    rect.y < 0 ||
    rect.x + rect.width > widthPx ||
    rect.y + rect.height > heightPx ||
    input.halfBits.length < count * 4
  ) {
    throw new Error(
      `a read-back of ${String(input.halfBits.length)} halves does not fill its region of the target`,
    );
  }
  const value = halves();
  const luminance = new Float32Array(count);
  const cells = new Int32Array(count).fill(OFF_BODY);
  const lit = new Uint8Array(count);
  const [lumaR, lumaG, lumaB] = METER_LUMA;
  let bodyPixels = 0;
  let bodyMisses = 0;
  let surveyPixels = 0;
  // Each surveyed cell met, flagged once: whether its μ floor binds from this frame's eye.
  const flags = new Map<number, boolean>();
  for (let row = 0; row < rect.height; row += 1) {
    const y = rect.y + row;
    for (let column = 0; column < rect.width; column += 1) {
      const p = row * rect.width + column;
      const at = p * 4;
      const meter = Math.round(value[input.halfBits[at + 3] ?? 0] ?? 0);
      if (meter !== METER_CLASS.litBody && meter !== METER_CLASS.unlitBody) {
        continue;
      }
      bodyPixels += 1;
      lit[p] = meter === METER_CLASS.litBody ? 1 : 0;
      luminance[p] =
        lumaR * (value[input.halfBits[at] ?? 0] ?? 0) +
        lumaG * (value[input.halfBits[at + 1] ?? 0] ?? 0) +
        lumaB * (value[input.halfBits[at + 2] ?? 0] ?? 0);
      const hit = pixelGroundM(camera, rect.x + column, y, widthPx, heightPx, figure);
      cells[p] = UNSURVEYED;
      if (hit === null) {
        bodyMisses += 1;
        continue;
      }
      const texel = texelOfDirection(hit, level);
      if (surveyed.has(texel)) {
        cells[p] = texel;
        surveyPixels += 1;
        if (!flags.has(texel)) {
          flags.set(texel, muFloorBinds(texel, level, camera.eyeM, figure, input.normals));
        }
      }
    }
  }
  return {
    widthPx,
    heightPx,
    rect,
    luminance,
    cells,
    bodyPixels,
    bodyMisses,
    surveyPixels,
    floored: new Set([...flags].flatMap(([texel, binds]) => (binds ? [texel] : []))),
    lit,
    camera,
    figure,
  };
}

/**
 * Where the ray through the centre of pixel (`x`, `y`) of a `widthPx` × `heightPx` target from
 * `camera` first meets the datum, body-fixed, m from the centre; `null` where it misses.
 */
export function pixelGroundM(
  camera: FrameCamera,
  x: number,
  y: number,
  widthPx: number,
  heightPx: number,
  figure: HandoverFigure,
): Vec3 | null {
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const viewX = (((x + 0.5) / widthPx) * 2 - 1) / s;
  const viewY = (1 - ((y + 0.5) / heightPx) * 2) / (s * (widthPx / heightPx));
  const ray = sub(add(scale(camera.right, viewX), scale(camera.up, viewY)), camera.back);
  return rayDatumHitM(camera.eyeM, ray, figure);
}

/**
 * The place a body-fixed point `pointM` projects to in a `widthPx` × `heightPx` target from
 * `camera`, px, pixel (x, y)'s centre at (x, y): the inverse of {@link pixelGroundM}'s ray; `null`
 * for a point not in front of the camera.
 */
export function projectToPixel(
  camera: FrameCamera,
  pointM: Vec3,
  widthPx: number,
  heightPx: number,
): { readonly x: number; readonly y: number } | null {
  const v = sub(pointM, camera.eyeM);
  const ahead = -dot(v, camera.back);
  if (!(ahead > 0)) {
    return null;
  }
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const viewX = dot(v, camera.right) / ahead;
  const viewY = dot(v, camera.up) / ahead;
  return {
    x: ((viewX * s + 1) / 2) * widthPx - 0.5,
    y: ((1 - viewY * s * (widthPx / heightPx)) / 2) * heightPx - 0.5,
  };
}

/**
 * The luminance of `reading` at the body-fixed ground point `pointM`: bilinear between the centres
 * of the four pixels about the point's projection into the reading's camera, or `null` where any
 * of the four lies outside the region or off the body, where they are not all of one meter class
 * (lit or unlit: the terminator's kink, across which the light is not linear), or where the point
 * faces away from the reading's eye.
 */
export function resampledAt(reading: FrameReading, pointM: Vec3): number | null {
  const toEye = sub(reading.camera.eyeM, pointM);
  if (!(dot(datumNormal(pointM, reading.figure), toEye) > 0)) {
    return null;
  }
  const at = projectToPixel(reading.camera, pointM, reading.widthPx, reading.heightPx);
  if (at === null) {
    return null;
  }
  const { rect } = reading;
  const x0 = Math.floor(at.x);
  const y0 = Math.floor(at.y);
  if (
    x0 < rect.x ||
    y0 < rect.y ||
    x0 + 1 >= rect.x + rect.width ||
    y0 + 1 >= rect.y + rect.height
  ) {
    return null;
  }
  const fx = at.x - x0;
  const fy = at.y - y0;
  let sum = 0;
  let litCorners = 0;
  for (let corner = 0; corner < 4; corner += 1) {
    const dx = corner & 1;
    const dy = corner >> 1;
    const p = (y0 + dy - rect.y) * rect.width + (x0 + dx - rect.x);
    if ((reading.cells[p] ?? OFF_BODY) === OFF_BODY) {
      return null;
    }
    litCorners += reading.lit[p] ?? 0;
    sum += (dx === 1 ? fx : 1 - fx) * (dy === 1 ? fy : 1 - fy) * (reading.luminance[p] ?? 0);
  }
  return litCorners === 0 || litCorners === 4 ? sum : null;
}

/** Two frames' comparison: see the module's remarks. */
export interface ReadingComparison {
  readonly body: { readonly own: HandoverOwnMeans; readonly shared: HandoverSharedMeans };
  readonly survey: { readonly own: HandoverOwnMeans; readonly shared: HandoverSharedMeans };
  /**
   * Every surveyed cell with a pixel of the frame not resampled whose ground point the resampled
   * frame shows ({@link resampledAt}), in texel order, each marked where its μ floor binds in either
   * frame.
   */
  readonly texels: ReadonlyArray<HandoverTexelRecord>;
  /** The kept frame's surveyed pixels the cells' means leave out, and the cells left with none. */
  readonly resampleGaps: HandoverResampleGaps;
}

/**
 * Which of a comparison's two frames is resampled at the other's pixels' ground points: the disc's,
 * the smooth one (`before` at the switch from the disc and at the terrain's first draw, `after` at
 * the hand-back).
 */
export type ResampledFrame = "before" | "after";

/** A sum and its count. */
interface Tally {
  sum: number;
  n: number;
}

/** A tally's mean, 0 over nothing. */
function meanOf(tally: Tally): number {
  return tally.n > 0 ? tally.sum / tally.n : 0;
}

/** The mean over `reading`'s pixels whose cell passes `counted`. */
function ownTally(reading: FrameReading, counted: (cell: number) => boolean): Tally {
  const tally: Tally = { sum: 0, n: 0 };
  for (let p = 0; p < reading.cells.length; p += 1) {
    if (counted(reading.cells[p] ?? OFF_BODY)) {
      tally.sum += reading.luminance[p] ?? 0;
      tally.n += 1;
    }
  }
  return tally;
}

/** The own means of `before` and `after` over the pixels whose cell passes `counted`. */
function ownMeans(
  before: FrameReading,
  after: FrameReading,
  counted: (cell: number) => boolean,
): HandoverOwnMeans {
  const b = ownTally(before, counted);
  const a = ownTally(after, counted);
  return { pixelsBefore: b.n, pixelsAfter: a.n, meanBefore: meanOf(b), meanAfter: meanOf(a) };
}

/** Whether a pixel's cell is the body's. */
function onBody(cell: number): boolean {
  return cell !== OFF_BODY;
}

/** Whether a pixel's cell is a surveyed one. */
function onSurvey(cell: number): boolean {
  return cell >= 0;
}

/** The sums of the pixels two frames share, before and after, and their count. */
interface PairTally {
  b: number;
  a: number;
  n: number;
}

/** A pair tally's means, 0 over nothing. */
function sharedMeans(tally: PairTally): HandoverSharedMeans {
  return {
    pixels: tally.n,
    meanBefore: tally.n > 0 ? tally.b / tally.n : 0,
    meanAfter: tally.n > 0 ? tally.a / tally.n : 0,
  };
}

/**
 * Compares the frames either side of a switch: see the module's remarks.
 *
 * @param resampled - The frame resampled at the other's pixels' ground points for each cell's means.
 * @throws Error where the two were read from targets of different sizes, whose pixels are not the
 *   same pixels.
 */
export function compareReadings(
  before: FrameReading,
  after: FrameReading,
  resampled: ResampledFrame,
): ReadingComparison {
  if (before.widthPx !== after.widthPx || before.heightPx !== after.heightPx) {
    throw new Error(
      `the scene target was ${String(before.widthPx)} × ${String(before.heightPx)} px before and ${String(after.widthPx)} × ${String(after.heightPx)} px after`,
    );
  }
  const body: PairTally = { b: 0, a: 0, n: 0 };
  const survey: PairTally = { b: 0, a: 0, n: 0 };
  const texels = new Map<number, PairTally>();
  // The pixels of the two regions' overlap, by their place in the target.
  const x0 = Math.max(before.rect.x, after.rect.x);
  const x1 = Math.min(before.rect.x + before.rect.width, after.rect.x + after.rect.width);
  const y0 = Math.max(before.rect.y, after.rect.y);
  const y1 = Math.min(before.rect.y + before.rect.height, after.rect.y + after.rect.height);
  for (let y = y0; y < y1; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      const pb = (y - before.rect.y) * before.rect.width + (x - before.rect.x);
      const pa = (y - after.rect.y) * after.rect.width + (x - after.rect.x);
      const cellB = before.cells[pb] ?? OFF_BODY;
      const cellA = after.cells[pa] ?? OFF_BODY;
      if (cellB === OFF_BODY || cellA === OFF_BODY) {
        continue;
      }
      const lumB = before.luminance[pb] ?? 0;
      const lumA = after.luminance[pa] ?? 0;
      body.b += lumB;
      body.a += lumA;
      body.n += 1;
      if (cellB >= 0 && cellB === cellA) {
        survey.b += lumB;
        survey.a += lumA;
        survey.n += 1;
      }
    }
  }
  // Each cell's means: every surveyed pixel of the frame kept against the resampled frame at its
  // ground point.
  const kept = resampled === "before" ? after : before;
  const other = resampled === "before" ? before : after;
  let gapPixels = 0;
  const gapCells = new Set<number>();
  for (let row = 0; row < kept.rect.height; row += 1) {
    for (let column = 0; column < kept.rect.width; column += 1) {
      const p = row * kept.rect.width + column;
      const cell = kept.cells[p] ?? OFF_BODY;
      if (cell < 0) {
        continue;
      }
      const ground = pixelGroundM(
        kept.camera,
        kept.rect.x + column,
        kept.rect.y + row,
        kept.widthPx,
        kept.heightPx,
        kept.figure,
      );
      const there = ground === null ? null : resampledAt(other, ground);
      if (there === null) {
        gapPixels += 1;
        gapCells.add(cell);
        continue;
      }
      const own = kept.luminance[p] ?? 0;
      const entry = texels.get(cell) ?? { b: 0, a: 0, n: 0 };
      entry.b += resampled === "before" ? there : own;
      entry.a += resampled === "before" ? own : there;
      entry.n += 1;
      texels.set(cell, entry);
    }
  }
  return {
    body: { own: ownMeans(before, after, onBody), shared: sharedMeans(body) },
    survey: { own: ownMeans(before, after, onSurvey), shared: sharedMeans(survey) },
    texels: [...texels]
      .toSorted(([x], [y]) => x - y)
      .map(([texel, tally]) => ({
        texel,
        sharedPixels: tally.n,
        meanBefore: tally.b / tally.n,
        meanAfter: tally.a / tally.n,
        muFloored: before.floored.has(texel) || after.floored.has(texel),
      })),
    resampleGaps: {
      pixels: gapPixels,
      cells: [...gapCells].filter((cell) => !texels.has(cell)).length,
    },
  };
}

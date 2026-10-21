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
 *   survey the same pixel assigned to the same cell), and each surveyed cell's mean over the pixels
 *   both frames assign to it, every cell with one such pixel or more listed. Means, not sums, so
 *   that the camera's closing between the frames moves no figure; a cell's shared pixels remove the
 *   closing altogether. The judging is the main process's.
 */

import type {
  HandoverOwnMeans,
  HandoverSharedMeans,
  HandoverTexelRecord,
} from "../../../../../preload/api";
import { add, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import type { Quaternion } from "../../../view/camera/pose";
import { rotate } from "../../../view/camera/quaternion";
import { rotateToBodyFixed } from "../../../view/coords/rotation";
import { METER_LUMA } from "../../../view/post/histogram";
import { METER_CLASS } from "../../../view/post/meter";
import {
  type HandoverFigure,
  type LookGeometry,
  rayDatumHitM,
  texelOfDirection,
} from "./handoverGeometry";

/** A pixel's cell where the body is not drawn there. */
export const OFF_BODY = -2;

/** A pixel's cell where the body is drawn and its ray meets no surveyed cell. */
export const UNSURVEYED = -1;

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
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = widthPx / heightPx;
  const [lumaR, lumaG, lumaB] = METER_LUMA;
  let bodyPixels = 0;
  let bodyMisses = 0;
  let surveyPixels = 0;
  for (let row = 0; row < rect.height; row += 1) {
    const y = rect.y + row;
    const viewY = (1 - ((y + 0.5) / heightPx) * 2) / (s * aspect);
    for (let column = 0; column < rect.width; column += 1) {
      const p = row * rect.width + column;
      const at = p * 4;
      const meter = Math.round(value[input.halfBits[at + 3] ?? 0] ?? 0);
      if (meter !== METER_CLASS.litBody && meter !== METER_CLASS.unlitBody) {
        continue;
      }
      bodyPixels += 1;
      luminance[p] =
        lumaR * (value[input.halfBits[at] ?? 0] ?? 0) +
        lumaG * (value[input.halfBits[at + 1] ?? 0] ?? 0) +
        lumaB * (value[input.halfBits[at + 2] ?? 0] ?? 0);
      const x = rect.x + column;
      const viewX = (((x + 0.5) / widthPx) * 2 - 1) / s;
      const ray = sub(add(scale(camera.right, viewX), scale(camera.up, viewY)), camera.back);
      const hit = rayDatumHitM(camera.eyeM, ray, figure);
      cells[p] = UNSURVEYED;
      if (hit === null) {
        bodyMisses += 1;
        continue;
      }
      const texel = texelOfDirection(hit, level);
      if (surveyed.has(texel)) {
        cells[p] = texel;
        surveyPixels += 1;
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
  };
}

/** Two frames' comparison: see the module's remarks. */
export interface ReadingComparison {
  readonly body: { readonly own: HandoverOwnMeans; readonly shared: HandoverSharedMeans };
  readonly survey: { readonly own: HandoverOwnMeans; readonly shared: HandoverSharedMeans };
  /** Every surveyed cell with a pixel both frames assign to it, in texel order. */
  readonly texels: ReadonlyArray<HandoverTexelRecord>;
}

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
 * @throws Error where the two were read from targets of different sizes, whose pixels are not the
 *   same pixels.
 */
export function compareReadings(before: FrameReading, after: FrameReading): ReadingComparison {
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
        const entry = texels.get(cellB) ?? { b: 0, a: 0, n: 0 };
        entry.b += lumB;
        entry.a += lumA;
        entry.n += 1;
        texels.set(cellB, entry);
      }
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
      })),
  };
}

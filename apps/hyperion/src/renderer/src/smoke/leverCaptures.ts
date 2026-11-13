/**
 * The smoke harness's comparisons and captures of plan R14's levers (Design note 10): a frame drawn
 * with and without a lever in one run, from the same selection and residents at the same exposure,
 * compared over its terrain by luminance in stops, and saved with its terrain's mask under
 * `--captures` for `scripts/compareCaptures.mjs`. Streamed frames vary from run to run, so the two
 * are never compared across builds.
 */

import { METER_CLASS } from "../view/post/meter";
import { base64Of, type CapturedImage, srgb8 } from "./atmosphere";

/** A third of a stop, the bounded levers' unit. */
export const THIRD_STOP = 1 / 3;

/** A capture's size, px. */
export interface CaptureSize {
  readonly widthPx: number;
  readonly heightPx: number;
}

/** Rec. 709's luminance of texel `p` of a read-back. */
export function luminanceAt(texels: Float32Array, p: number): number {
  return (
    0.2126 * (texels[4 * p] ?? 0) +
    0.7152 * (texels[4 * p + 1] ?? 0) +
    0.0722 * (texels[4 * p + 2] ?? 0)
  );
}

/**
 * Whether texel `p` of a read-back was drawn by a body, its meter class lit or unlit: the terrain's,
 * where the terrain alone was drawn.
 */
export function isTerrain(texels: Float32Array, p: number): boolean {
  const meter = texels[4 * p + 3];
  return meter === METER_CLASS.litBody || meter === METER_CLASS.unlitBody;
}

/** How far apart two frames are over their terrain ({@link stopsApart}). */
export interface StopsApart {
  /** The terrain's texels. */
  readonly terrain: number;
  /** Those that differ at all. */
  readonly differing: number;
  /** Those within {@link THIRD_STOP} by luminance. */
  readonly within: number;
  /** The largest difference, stops. */
  readonly largestStops: number;
}

/** `before`'s brightest luminance. */
function brightestOf(before: Float32Array): number {
  let brightest = 0;
  for (let p = 0; p < Math.floor(before.length / 4); p += 1) {
    brightest = Math.max(brightest, luminanceAt(before, p));
  }
  return brightest;
}

/**
 * Texel `p`'s luminances in `before` and `after`, each taken at least at `floor`, and the stops
 * between them: {@link stopsApart}'s and {@link beyondThirdStop}'s one judgement.
 */
function stopsAt(
  before: Float32Array,
  after: Float32Array,
  p: number,
  floor: number,
): { readonly a: number; readonly b: number; readonly stops: number } {
  const a = Math.max(luminanceAt(before, p), floor);
  const b = Math.max(luminanceAt(after, p), floor);
  return { a, b, stops: Math.abs(Math.log2(b / a)) };
}

/** The luminance a darker texel is taken at: 10⁻⁶ of the brightest. */
function luminanceFloor(brightest: number): number {
  return Math.max(1e-6 * brightest, Number.MIN_VALUE);
}

/**
 * `after`'s terrain texels against `before`'s by luminance, the exposure the same, the terrain
 * `mask`'s bodies' texels (`before`'s by default). A luminance under 10⁻⁶ of `before`'s brightest is
 * taken at that floor.
 */
export function stopsApart(
  before: Float32Array,
  after: Float32Array,
  mask: Float32Array = before,
): StopsApart {
  const texels = Math.floor(before.length / 4);
  const floor = luminanceFloor(brightestOf(before));
  let terrain = 0;
  let differing = 0;
  let within = 0;
  let largestStops = 0;
  for (let p = 0; p < texels; p += 1) {
    if (!isTerrain(mask, p)) {
      continue;
    }
    terrain += 1;
    const { stops } = stopsAt(before, after, p, floor);
    differing += stops > 0 ? 1 : 0;
    within += stops <= THIRD_STOP ? 1 : 0;
    largestStops = Math.max(largestStops, stops);
  }
  return { terrain, differing, within, largestStops };
}

/**
 * `after`'s terrain texels beyond {@link THIRD_STOP} of `before`'s, as {@link stopsApart} judges
 * them, listed for a check's detail: the first `most` in reading order, each at its place in a frame
 * `widthPx` wide, rows from the top, with both luminances as shares of `before`'s brightest, then
 * how many more; "none" where there are none.
 */
export function beyondThirdStop(
  before: Float32Array,
  after: Float32Array,
  widthPx: number,
  mask: Float32Array = before,
  most = 12,
): string {
  const brightest = brightestOf(before);
  const floor = luminanceFloor(brightest);
  const share = Math.max(brightest, Number.MIN_VALUE);
  const listed: string[] = [];
  let beyond = 0;
  for (let p = 0; p < Math.floor(before.length / 4); p += 1) {
    if (!isTerrain(mask, p)) {
      continue;
    }
    const { a, b, stops } = stopsAt(before, after, p, floor);
    if (stops <= THIRD_STOP) {
      continue;
    }
    beyond += 1;
    if (listed.length < most) {
      listed.push(
        `(${String(p % widthPx)}, ${String(Math.floor(p / widthPx))}) ${(a / share).toExponential(1)} → ${(b / share).toExponential(1)}`,
      );
    }
  }
  if (beyond === 0) {
    return "none";
  }
  return beyond > listed.length
    ? `${listed.join(", ")} and ${String(beyond - listed.length)} more`
    : listed.join(", ");
}

/**
 * The exposure that maps `texels`' brightest terrain texel (by `mask`'s bodies) to 1, so that a
 * lever's two frames are captured at one.
 */
export function captureScale(texels: Float32Array, mask: Float32Array = texels): number {
  let brightest = 0;
  for (let p = 0; p < Math.floor(texels.length / 4); p += 1) {
    if (isTerrain(mask, p)) {
      brightest = Math.max(brightest, luminanceAt(texels, p));
    }
  }
  return 1 / Math.max(brightest, Number.MIN_VALUE);
}

/** `texels` as an 8-bit sRGB capture named `name`, scaled by `scale` and clipped, rows from the top. */
export function captureOf(
  name: string,
  texels: Float32Array,
  size: CaptureSize,
  scale: number,
): CapturedImage {
  const rgba = new Uint8Array(4 * size.widthPx * size.heightPx);
  for (let p = 0; p < size.widthPx * size.heightPx; p += 1) {
    for (let c = 0; c < 3; c += 1) {
      rgba[4 * p + c] = srgb8(Math.min(1, scale * (texels[4 * p + c] ?? 0)));
    }
    rgba[4 * p + 3] = 255;
  }
  return { name, width: size.widthPx, height: size.heightPx, rgba: base64Of(rgba) };
}

/** The terrain's mask of `texels` as a capture named `name`: white on its bodies, black elsewhere. */
export function maskOf(name: string, texels: Float32Array, size: CaptureSize): CapturedImage {
  const rgba = new Uint8Array(4 * size.widthPx * size.heightPx);
  for (let p = 0; p < size.widthPx * size.heightPx; p += 1) {
    const value = isTerrain(texels, p) ? 255 : 0;
    rgba.fill(value, 4 * p, 4 * p + 3);
    rgba[4 * p + 3] = 255;
  }
  return { name, width: size.widthPx, height: size.heightPx, rgba: base64Of(rgba) };
}

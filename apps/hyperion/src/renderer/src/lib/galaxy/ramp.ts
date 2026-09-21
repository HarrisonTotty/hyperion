/**
 * The single-hue brightness ramp for density maps, and the rasteriser that paints a map with it.
 *
 * @remarks
 * The ramp is linear in sRGB from `--surface-0` to `--text`, read from the stylesheet and never
 * written here (plan 05, design note D6). The picture is logarithmic in density because the map's
 * codes are already linear in log₁₀ of systems per square light-year (plan 04, design note 12), so
 * a linear ramp over the codes is a logarithmic ramp over density. Code 0, at or below the floor,
 * is exactly `--surface-0`, and every code above it is at least one level brighter, as the guide
 * requires of a raster field.
 */

/** An sRGB colour with channels 0 to 255. */
export interface Rgb {
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

/** A colour token that is not a `#rrggbb` colour, which means the stylesheet is broken. */
export class ColourTokenError extends Error {
  override readonly name = "ColourTokenError";
}

/** Levels in a ramp: level 0 is the background, levels 1 to 255 run from floor to ceiling. */
export const RAMP_LEVELS = 256;

const HEX_COLOUR = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i;

/**
 * Parses a colour token's value, as `getComputedStyle` returns it for a custom property.
 *
 * @remarks
 * The value is trimmed first, since a computed custom property keeps the stylesheet's leading
 * space.
 *
 * @throws ColourTokenError when the value is not a six-digit hex colour.
 */
export function parseHexColour(text: string): Rgb {
  const match = HEX_COLOUR.exec(text.trim());
  const [, r, g, b] = match ?? [];
  if (r === undefined || g === undefined || b === undefined) {
    throw new ColourTokenError(`colour token is not #rrggbb: "${text}"`);
  }
  return { r: Number.parseInt(r, 16), g: Number.parseInt(g, 16), b: Number.parseInt(b, 16) };
}

/**
 * Builds the 256-level ramp as RGBA bytes, linear in sRGB from `surface0` (level 0, exactly) to
 * `text` (level 255, exactly), fully opaque.
 */
export function buildRamp(surface0: Rgb, text: Rgb): Uint8ClampedArray {
  const ramp = new Uint8ClampedArray(RAMP_LEVELS * 4);
  const last = RAMP_LEVELS - 1;
  for (let level = 0; level < RAMP_LEVELS; level += 1) {
    const t = level / last;
    ramp[level * 4] = Math.round(surface0.r + (text.r - surface0.r) * t);
    ramp[level * 4 + 1] = Math.round(surface0.g + (text.g - surface0.g) * t);
    ramp[level * 4 + 2] = Math.round(surface0.b + (text.b - surface0.b) * t);
    ramp[level * 4 + 3] = 255;
  }
  return ramp;
}

/**
 * The ramp level for a map code: 0 for code 0, otherwise 1 + round((code − 1) ÷ (maxCode − 1) ×
 * 254).
 *
 * @remarks
 * Code 1 (the floor) is level 1 and `maxCode` (the ceiling) level 255 at 8 and at 16 bits, so a
 * value above the floor is never painted as the background.
 *
 * @throws RangeError when `maxCode` is below 2, which no map has.
 */
export function codeToLevel(code: number, maxCode: number): number {
  if (!(maxCode >= 2)) {
    throw new RangeError(`a map's largest code is at least 2, got ${String(maxCode)}`);
  }
  if (code <= 0) {
    return 0;
  }
  const clamped = Math.min(code, maxCode);
  return 1 + Math.round(((clamped - 1) / (maxCode - 1)) * (RAMP_LEVELS - 2));
}

/**
 * The CSS colour of one ramp level, for legends drawn in SVG from the same ramp as the picture.
 *
 * @throws RangeError when the level is outside the ramp.
 */
export function rampColour(ramp: Uint8ClampedArray, level: number): string {
  const r = ramp[level * 4];
  const g = ramp[level * 4 + 1];
  const b = ramp[level * 4 + 2];
  if (!Number.isInteger(level) || r === undefined || g === undefined || b === undefined) {
    throw new RangeError(`no ramp level ${String(level)}`);
  }
  return `rgb(${r} ${g} ${b})`;
}

/**
 * The decoded codes of a density map.
 *
 * @remarks
 * A structural subset of `DecodedDensityMap` from `@hyperion/protocol` (plan 04). Codes run row by
 * row from the top, left to right, as `ImageData` does.
 */
export interface DecodedCodes {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly codes: Uint8Array | Uint16Array;
  readonly maxCode: number;
}

/**
 * Paints a decoded map with the ramp, as RGBA bytes ready for an `ImageData` of the map's size.
 *
 * @throws RangeError when the codes do not fill the map or the ramp is not 256 levels.
 */
export function rasterise(decoded: DecodedCodes, ramp: Uint8ClampedArray): Uint8ClampedArray {
  const pixels = decoded.widthPx * decoded.heightPx;
  if (decoded.codes.length !== pixels) {
    throw new RangeError(`${decoded.codes.length} codes cannot fill a map of ${pixels} pixels`);
  }
  if (ramp.length !== RAMP_LEVELS * 4) {
    throw new RangeError(`a ramp has ${RAMP_LEVELS * 4} bytes, got ${ramp.length}`);
  }
  const levels = new Uint8Array(decoded.maxCode + 1);
  for (let code = 0; code <= decoded.maxCode; code += 1) {
    levels[code] = codeToLevel(code, decoded.maxCode);
  }
  const rgba = new Uint8ClampedArray(pixels * 4);
  for (let pixel = 0; pixel < pixels; pixel += 1) {
    const code = decoded.codes[pixel] ?? 0;
    const level = levels[Math.min(code, decoded.maxCode)] ?? 0;
    rgba.set(ramp.subarray(level * 4, level * 4 + 4), pixel * 4);
  }
  return rgba;
}

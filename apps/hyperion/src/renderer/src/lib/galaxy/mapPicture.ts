/**
 * Fitting a density map's pixels to the device pixels of its picture without losing any.
 *
 * @remarks
 * A map is requested at the smallest resolution plan 04 offers that is at least as wide as the
 * picture's backing store, so it never has fewer pixels than the screen shows. What excess remains
 * is removed by averaging, never by dropping: each device pixel takes the area-weighted mean of the
 * linear column density of the map pixels it covers. That mean is the column density of the device
 * pixel's own area, a value of the data, not one invented between samples, so a feature thinner
 * than a device pixel, such as the edge-on young disc, is dimmed by its share of the area and never
 * lost. Log codes are not averaged: the mean of logarithms is not the logarithm of the mean.
 */
import { type DecodedCodes, RAMP_LEVELS } from "./ramp";

/** The map widths the protocol offers, in pixels, narrowest first (plan 04, design note 12). */
export const MAP_RESOLUTIONS_PX: ReadonlyArray<number> = [128, 256, 512, 1_024];

/**
 * The map width to request for a picture: the narrowest the protocol offers that is at least the
 * picture's width in device pixels, or the widest when none is.
 *
 * @param backingWidthPx - The picture's width in device pixels.
 */
export function mapResolutionFor(backingWidthPx: number): number {
  const widest = MAP_RESOLUTIONS_PX.at(-1) ?? 1_024;
  return MAP_RESOLUTIONS_PX.find((resolution) => resolution >= backingWidthPx) ?? widest;
}

/**
 * The codes turned a quarter-turn clockwise, as a picture is when it is drawn turned: the raster's
 * top row becomes the picture's right-hand column, and its left-hand column the top row.
 */
export function turnClockwise(decoded: DecodedCodes): DecodedCodes {
  const { widthPx, heightPx, codes } = decoded;
  const turned = new (codes instanceof Uint16Array ? Uint16Array : Uint8Array)(codes.length);
  // The turned picture is `heightPx` wide; its pixel (column c, row r) is the raster's pixel in
  // column r and row heightPx − 1 − c.
  for (let row = 0; row < widthPx; row += 1) {
    for (let column = 0; column < heightPx; column += 1) {
      turned[row * heightPx + column] = codes[(heightPx - 1 - column) * widthPx + row] ?? 0;
    }
  }
  return { widthPx: heightPx, heightPx: widthPx, codes: turned, maxCode: decoded.maxCode };
}

/**
 * For each pixel of a reduced axis, the first source pixel it covers and the share of its width
 * that each covered source pixel fills.
 */
interface AxisCover {
  readonly first: Int32Array;
  readonly shares: ReadonlyArray<Float64Array>;
}

function axisCover(sourcePx: number, reducedPx: number): AxisCover {
  const ratio = sourcePx / reducedPx;
  const first = new Int32Array(reducedPx);
  const shares: Float64Array[] = [];
  for (let pixel = 0; pixel < reducedPx; pixel += 1) {
    const start = pixel * ratio;
    const end = (pixel + 1) * ratio;
    const firstSource = Math.floor(start);
    const lastSource = Math.min(sourcePx - 1, Math.ceil(end) - 1);
    const share = new Float64Array(lastSource - firstSource + 1);
    for (let source = firstSource; source <= lastSource; source += 1) {
      share[source - firstSource] = (Math.min(source + 1, end) - Math.max(source, start)) / ratio;
    }
    first[pixel] = firstSource;
    shares.push(share);
  }
  return { first, shares };
}

/** How far below 1 a mean of floor-level pixels may fall by rounding and still be the floor. */
const FLOOR_TOLERANCE = 1e-9;

/**
 * The ramp levels of a map reduced to `widthPx` by `heightPx`, each the area-weighted mean of the
 * linear column density of the map pixels under it.
 *
 * @remarks
 * Densities are taken relative to the floor, code 1: code 0 (at or below the floor, or empty)
 * counts as none. A mean below the floor is level 0, the background, as the guide has values at or
 * below the floor shown; otherwise its level is placed on the ramp as a code's is, linear in log₁₀,
 * 1 at the floor and 255 at the ceiling. A dimension no larger than the map's is required; the map
 * is never enlarged here, since drawing it larger repeats pixels and loses none.
 *
 * @param spanLog10 - The map's ceiling less its floor, in dex.
 * @throws RangeError when the reduced size is larger than the map or empty, or the codes do not
 *   fill the map.
 */
export function reducedLevels(
  decoded: DecodedCodes,
  spanLog10: number,
  widthPx: number,
  heightPx: number,
): Uint8Array {
  const { widthPx: sourceWidthPx, heightPx: sourceHeightPx, codes, maxCode } = decoded;
  if (codes.length !== sourceWidthPx * sourceHeightPx) {
    throw new RangeError(
      `${codes.length} codes cannot fill a map of ${sourceWidthPx} × ${sourceHeightPx}`,
    );
  }
  const fits =
    Number.isInteger(widthPx) &&
    Number.isInteger(heightPx) &&
    widthPx >= 1 &&
    heightPx >= 1 &&
    widthPx <= sourceWidthPx &&
    heightPx <= sourceHeightPx;
  if (!fits) {
    throw new RangeError(
      `a ${sourceWidthPx} × ${sourceHeightPx} map cannot be reduced to ${widthPx} × ${heightPx}`,
    );
  }
  // Each code's density as a multiple of the floor's.
  const density = new Float64Array(maxCode + 1);
  for (let code = 1; code <= maxCode; code += 1) {
    density[code] = spanLog10 > 0 ? 10 ** (((code - 1) * spanLog10) / (maxCode - 1)) : 1;
  }
  const across = axisCover(sourceWidthPx, widthPx);
  const down = axisCover(sourceHeightPx, heightPx);

  // Across first, one source row at a time, then down.
  const rowMeans = new Float64Array(sourceHeightPx * widthPx);
  for (let sourceRow = 0; sourceRow < sourceHeightPx; sourceRow += 1) {
    const rowStart = sourceRow * sourceWidthPx;
    for (let column = 0; column < widthPx; column += 1) {
      const first = across.first[column] ?? 0;
      const shares = across.shares[column] ?? new Float64Array(0);
      let sum = 0;
      for (let index = 0; index < shares.length; index += 1) {
        sum += (shares[index] ?? 0) * (density[codes[rowStart + first + index] ?? 0] ?? 0);
      }
      rowMeans[sourceRow * widthPx + column] = sum;
    }
  }
  const levels = new Uint8Array(widthPx * heightPx);
  const top = RAMP_LEVELS - 1;
  for (let row = 0; row < heightPx; row += 1) {
    const first = down.first[row] ?? 0;
    const shares = down.shares[row] ?? new Float64Array(0);
    for (let column = 0; column < widthPx; column += 1) {
      let mean = 0;
      for (let index = 0; index < shares.length; index += 1) {
        mean += (shares[index] ?? 0) * (rowMeans[(first + index) * widthPx + column] ?? 0);
      }
      let level = 0;
      if (mean >= 1 - FLOOR_TOLERANCE) {
        const aboveFloor = spanLog10 > 0 ? Math.log10(Math.max(1, mean)) / spanLog10 : 0;
        level = 1 + Math.round(Math.min(1, aboveFloor) * (top - 1));
      }
      levels[row * widthPx + column] = level;
    }
  }
  return levels;
}

/**
 * Paints ramp levels as RGBA bytes, ready for an `ImageData` of the same size.
 *
 * @throws RangeError when the ramp is not 256 levels.
 */
export function paintLevels(levels: Uint8Array, ramp: Uint8ClampedArray): Uint8ClampedArray {
  if (ramp.length !== RAMP_LEVELS * 4) {
    throw new RangeError(`a ramp has ${RAMP_LEVELS * 4} bytes, got ${ramp.length}`);
  }
  const rgba = new Uint8ClampedArray(levels.length * 4);
  for (let pixel = 0; pixel < levels.length; pixel += 1) {
    const level = levels[pixel] ?? 0;
    rgba.set(ramp.subarray(level * 4, level * 4 + 4), pixel * 4);
  }
  return rgba;
}

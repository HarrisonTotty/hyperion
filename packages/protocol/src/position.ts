import type { GalacticPosition } from "./generated/GalacticPosition";

/**
 * Metres in one light-year, exact by definition.
 *
 * @remarks
 * The IAU light-year is the distance light travels in vacuum in one Julian year:
 * 299,792,458 m/s × 31,557,600 s. It is the value of `units::consts::METRES_PER_LIGHT_YEAR` in
 * `hyperion-sim`, and is exact as a JavaScript number because it is even and below 2⁵⁴.
 */
export const METRES_PER_LIGHT_YEAR = 9_460_730_472_580_800;

/** Bounds of the `i32` that carries each cell coordinate on the wire. */
const CELL_MIN_LY = -(2 ** 31);
const CELL_MAX_LY = 2 ** 31 - 1;

/** Axis names, for error messages. */
const AXES = ["x", "y", "z"] as const;

type Axis = 0 | 1 | 2;

/** Splits one coordinate into its whole light-year cell and its offset in metres. */
function splitAxis(ly: number, axis: Axis): readonly [cellLy: number, offsetM: number] {
  if (!Number.isFinite(ly)) {
    throw new RangeError(`${AXES[axis]} coordinate ${ly} ly is not finite`);
  }
  let cellLy = Math.floor(ly);
  let offsetM = (ly - cellLy) * METRES_PER_LIGHT_YEAR;
  // Just below a whole light-year the fraction can round up to one; carry it into the cell so that
  // the offset stays in [0, 1 ly), as the server requires.
  if (offsetM >= METRES_PER_LIGHT_YEAR) {
    cellLy += 1;
    offsetM = 0;
  }
  if (cellLy < CELL_MIN_LY || cellLy > CELL_MAX_LY) {
    throw new RangeError(`${AXES[axis]} coordinate ${ly} ly is outside the galactic frame`);
  }
  // `Math.floor(-0)` is `-0`, which must not reach the wire as a distinct cell.
  return [cellLy === 0 ? 0 : cellLy, offsetM];
}

/**
 * Builds the exact wire form of a point given in light-years in the `GALACTIC` frame.
 *
 * @remarks
 * Each coordinate is floored to its 1 ly cell and the remainder kept as an offset in metres in
 * `[0, 1 ly)`, so a negative coordinate such as −0.25 ly lands in cell −1 with an offset of
 * 0.75 ly. The input's own precision is the limit: an `f64` of light-years resolves about 65 km at
 * 50,000 ly.
 *
 * @param xyzLy - The point's x, y and z in light-years.
 * @throws RangeError if a coordinate is not finite or does not fit the frame's `i32` cells.
 */
export function galacticPositionFromLy(xyzLy: readonly [number, number, number]): GalacticPosition {
  const [xCell, xOffset] = splitAxis(xyzLy[0], 0);
  const [yCell, yOffset] = splitAxis(xyzLy[1], 1);
  const [zCell, zOffset] = splitAxis(xyzLy[2], 2);
  return { cell_ly: [xCell, yCell, zCell], offset_m: [xOffset, yOffset, zOffset] };
}

/** One component of `to − from` in light-years, cells subtracted before offsets. */
function axisDeltaLy(from: GalacticPosition, to: GalacticPosition, axis: Axis): number {
  const cellsLy = to.cell_ly[axis] - from.cell_ly[axis];
  const offsetsM = to.offset_m[axis] - from.offset_m[axis];
  return cellsLy + offsetsM / METRES_PER_LIGHT_YEAR;
}

/**
 * The vector from `from` to `to` in light-years, along the `GALACTIC` axes.
 *
 * @remarks
 * Subtracting whole cells before offsets keeps the result exact to well under a metre anywhere in
 * the galaxy, which is how a chart centred far from the galactic centre gets its coordinates.
 */
export function galacticDeltaLy(
  from: GalacticPosition,
  to: GalacticPosition,
): [number, number, number] {
  return [axisDeltaLy(from, to, 0), axisDeltaLy(from, to, 1), axisDeltaLy(from, to, 2)];
}

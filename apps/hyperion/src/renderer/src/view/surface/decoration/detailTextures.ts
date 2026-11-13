/**
 * The detail textures' projection's reference (plan R11, R11.T2.a): the tile coordinates, the turn
 * and the weights `detailTextures.wgsl` forms, and the projection itself over a sample function in
 * place of the texture, so that the projection's arithmetic and its zero mean are tested on the
 * CPU and its coordinates and weights against the shader through the decoration twin.
 *
 * @remarks
 * Read `detailTextures.wgsl`'s header for the design: every coordinate the decoration lattice's,
 * two scales, the second turned by atan(4 ÷ 3) and three tiles wide, both offset by whole cells,
 * the weights |n · axis|⁴ or the biplanar's.
 */

import type { LatticePlace } from "./decorationLattice";

/** The weights' sharpness (`DETAIL_SHARPNESS`). */
export const DETAIL_SHARPNESS = 4;

/** The second scale's tiles a first tile's (`DETAIL_SECOND_SCALE`). */
export const DETAIL_SECOND_SCALE = 3;

/** The turn's denominator: (3, 4) ÷ 5 (`DETAIL_TURN_DENOMINATOR`). */
const DETAIL_TURN_DENOMINATOR = 5;

/** A detail set's projection (`DetailProjection`). */
export interface DetailProjection {
  /** The tile's period in the set's lattice cells, a positive integer. */
  readonly period: number;
  /** The first scale's offsets along x and y, then the second's, whole cells, `u32`. */
  readonly offsets: readonly [number, number, number, number];
  /** The texel's height above the datum in the set's cells. */
  readonly heightCells: number;
}

/** A tile coordinate in [0, 1) along one axis; `detail_coordinate`. */
export function detailCoordinate(
  cell: number,
  frac: number,
  offset: number,
  period: number,
): number {
  return ((((cell % period) + (offset % period)) % period) + frac) / period;
}

/** The first scale's tile coordinates; `detail_first`. */
export function detailFirst(
  place: LatticePlace,
  projection: DetailProjection,
): readonly [number, number] {
  const { period, offsets } = projection;
  return [
    detailCoordinate(place.x, place.fx, offsets[0], period),
    detailCoordinate(place.y, place.fy, offsets[1], period),
  ];
}

/** x − ⌊x⌋. */
function fract(x: number): number {
  return x - Math.floor(x);
}

/**
 * The second scale's tile coordinates, the place turned by (3, 4) ÷ 5 and offset, three tiles wide;
 * `detail_second`.
 */
export function detailSecond(
  place: LatticePlace,
  projection: DetailProjection,
): readonly [number, number] {
  const wrap = DETAIL_TURN_DENOMINATOR * DETAIL_SECOND_SCALE * projection.period;
  const x = ((place.x % wrap) + (projection.offsets[2] % wrap)) % wrap;
  const y = ((place.y % wrap) + (projection.offsets[3] % wrap)) % wrap;
  const turnedX = (3 * x + 4 * (wrap - y)) % wrap;
  const turnedY = (4 * x + 3 * y) % wrap;
  const fx = 3 * place.fx - 4 * place.fy;
  const fy = 4 * place.fx + 3 * place.fy;
  return [fract((turnedX + fx) / wrap), fract((turnedY + fy) / wrap)];
}

/** The turn back to the face's axes of a second-scale slope; `detail_unturn`. */
export function detailUnturn(slope: readonly [number, number]): readonly [number, number] {
  return [0.6 * slope[0] + 0.8 * slope[1], -0.8 * slope[0] + 0.6 * slope[1]];
}

/** A side plane's place, a face axis's cell and fraction beside the height; `detail_side_place`. */
export function detailSidePlace(
  cell: number,
  frac: number,
  heightCells: number,
  period: number,
): LatticePlace {
  const wrapped = heightCells - period * Math.floor(heightCells / period);
  const whole = Math.floor(wrapped);
  return { x: cell, y: Math.min(whole, period - 1), fx: frac, fy: wrapped - whole };
}

/** A normal's parts along the face's x, y and the ground's up. */
export type Along = readonly [number, number, number];

/** The triplanar weights of planes facing x, y and up; `detail_triplanar_weights`. */
export function triplanarWeights(along: Along): readonly [number, number, number] {
  const w = along.map((a) => Math.abs(a) ** DETAIL_SHARPNESS);
  const sum = Math.max((w[0] ?? 0) + (w[1] ?? 0) + (w[2] ?? 0), 1e-6);
  return [(w[0] ?? 0) / sum, (w[1] ?? 0) / sum, (w[2] ?? 0) / sum];
}

/**
 * The biplanar weights: each part less the least, to the power, normalised, so that the plane left
 * out weighs nothing where it changes (Quilez's biplanar mapping); `detail_biplanar_weights`.
 */
export function biplanarWeights(along: Along): readonly [number, number, number] {
  const a = along.map((x) => Math.abs(x));
  const least = Math.min(...a);
  const w = a.map((x) => Math.max(x - least, 0) ** DETAIL_SHARPNESS);
  const sum = Math.max((w[0] ?? 0) + (w[1] ?? 0) + (w[2] ?? 0), 1e-12);
  return [(w[0] ?? 0) / sum, (w[1] ?? 0) / sum, (w[2] ?? 0) / sum];
}

/** A set's texel at tile coordinates in [0, 1)²: slopes, albedo and hue deviations. */
export type DetailSample = (u: number, v: number) => readonly [number, number, number, number];

/** One plane's two scales, the second's slopes turned back, summed ÷ √2; `detail_plane`. */
function plane(
  sample: DetailSample,
  place: LatticePlace,
  projection: DetailProjection,
): readonly [number, number, number, number] {
  const [au, av] = detailFirst(place, projection);
  const [bu, bv] = detailSecond(place, projection);
  const a = sample(au, av);
  const b = sample(bu, bv);
  const [bx, by] = detailUnturn([b[0], b[1]]);
  return [
    Math.SQRT1_2 * (a[0] + bx),
    Math.SQRT1_2 * (a[1] + by),
    Math.SQRT1_2 * (a[2] + b[2]),
    Math.SQRT1_2 * (a[3] + b[3]),
  ];
}

/**
 * The set's deviation at the place for planes of weights `w`, as `detail_projected` forms it from
 * the texture, here from `sample`.
 */
export function detailProjected(
  sample: DetailSample,
  place: LatticePlace,
  projection: DetailProjection,
  w: readonly [number, number, number],
): readonly [number, number, number, number] {
  const out = [0, 0, 0, 0];
  if (w[2] > 0) {
    const top = plane(sample, place, projection);
    for (let c = 0; c < 4; c += 1) {
      out[c] = (out[c] ?? 0) + w[2] * (top[c] ?? 0);
    }
  }
  if (w[1] > 0) {
    const side = plane(
      sample,
      detailSidePlace(place.x, place.fx, projection.heightCells, projection.period),
      projection,
    );
    out[0] = (out[0] ?? 0) + w[1] * side[0];
    out[2] = (out[2] ?? 0) + w[1] * side[2];
    out[3] = (out[3] ?? 0) + w[1] * side[3];
  }
  if (w[0] > 0) {
    const side = plane(
      sample,
      detailSidePlace(place.y, place.fy, projection.heightCells, projection.period),
      projection,
    );
    out[1] = (out[1] ?? 0) + w[0] * side[0];
    out[2] = (out[2] ?? 0) + w[0] * side[2];
    out[3] = (out[3] ?? 0) + w[0] * side[3];
  }
  return [out[0] ?? 0, out[1] ?? 0, out[2] ?? 0, out[3] ?? 0];
}

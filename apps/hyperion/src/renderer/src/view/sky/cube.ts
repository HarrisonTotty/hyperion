/**
 * The star cube's geometry: which face and texel a direction falls in, and each texel's solid
 * angle (plan R06, Design note 21).
 *
 * @remarks
 * Faces are in WebGPU's layer order, +X, −X, +Y, −Y, +Z, −Z, on the galactic axes, and a direction
 * maps to a face and its (u, v) as WebGPU's cube sampling does (WebGPU §"Cube map faces", after
 * the Vulkan and D3D tables): row 0 is the face's top. The bake's CPU splat and its WGSL splat
 * both follow this one mapping, and compute it alike to the bit (see {@link cubeTexelOf}).
 */

/** The number of faces of a cube. */
export const CUBE_FACE_COUNT = 6;

/** A cube face's layer: 0–5 for +X, −X, +Y, −Y, +Z, −Z. */
export type CubeFace = 0 | 1 | 2 | 3 | 4 | 5;

/** Where a direction lands on a cube of a given face size. */
export interface CubeTexel {
  readonly face: CubeFace;
  readonly column: number;
  readonly row: number;
}

/**
 * The texel along one face axis: the largest c in [0, size) with c × 2m ≤ (a + m) × size, each
 * product and sum rounded to `f32`.
 *
 * @remarks
 * WGSL's division may be off by 2.5 ulp but its addition and multiplication are correctly rounded,
 * so the splat shader and this function both take an estimate and correct it by these comparisons
 * alone; they then agree on every texel, edges included, which the GPU-against-CPU splat test of
 * T13.g needs.
 */
function texelAlong(a: number, major: number, sizePx: number): number {
  const f = Math.fround;
  const span = f(2 * major);
  const scaled = f(f(a + major) * sizePx);
  let texel = Math.min(sizePx - 1, Math.max(0, Math.floor(scaled / span)));
  while (texel > 0 && f(texel * span) > scaled) {
    texel -= 1;
  }
  while (texel + 1 < sizePx && f((texel + 1) * span) <= scaled) {
    texel += 1;
  }
  return texel;
}

/**
 * The face and texel a direction falls in.
 *
 * @param x - The direction's components, `f32` values, any length but not zero.
 * @param sizePx - The face's side, texels.
 * @remarks
 * Ties between axes go to x, then y, then z. A direction exactly on a face's far edge is kept on
 * the face's last texel.
 * @throws Error for a zero or non-finite direction.
 */
export function cubeTexelOf(x: number, y: number, z: number, sizePx: number): CubeTexel {
  if (!(Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(z))) {
    throw new Error(`(${x}, ${y}, ${z}) is not a direction`);
  }
  const ax = Math.abs(x);
  const ay = Math.abs(y);
  const az = Math.abs(z);
  let face: CubeFace;
  let u: number;
  let v: number;
  let major: number;
  if (ax >= ay && ax >= az) {
    major = ax;
    face = x >= 0 ? 0 : 1;
    u = x >= 0 ? -z : z;
    v = -y;
  } else if (ay >= az) {
    major = ay;
    face = y >= 0 ? 2 : 3;
    u = x;
    v = y >= 0 ? z : -z;
  } else {
    major = az;
    face = z >= 0 ? 4 : 5;
    u = z >= 0 ? x : -x;
    v = -y;
  }
  if (!(major > 0)) {
    throw new Error(`(${x}, ${y}, ${z}) is not a direction`);
  }
  return {
    face,
    column: texelAlong(Math.fround(u), Math.fround(major), sizePx),
    row: texelAlong(Math.fround(v), Math.fround(major), sizePx),
  };
}

/** The solid angle of the face region from (0, 0) to (a, b), face coordinates in [−1, 1], sr. */
function cornerArea(a: number, b: number): number {
  return Math.atan2(a * b, Math.sqrt(a * a + b * b + 1));
}

/**
 * Every texel's solid angle on one face, sr, rows top to bottom; the same on all six faces.
 *
 * @remarks
 * The exact area of a texel's projection on the unit sphere: with A(a, b) = atan2(ab, √(a² + b² +
 * 1)), the solid angle subtended by the face region from the centre to (a, b), a texel is
 * A(x₀, y₀) − A(x₀, y₁) − A(x₁, y₀) + A(x₁, y₁). The six faces' sum is 4π.
 */
export function texelSolidAnglesSr(sizePx: number): Float64Array {
  const edges = Array.from({ length: sizePx + 1 }, (_, i) => (2 * i) / sizePx - 1);
  const angles = new Float64Array(sizePx * sizePx);
  // Each corner's A once, a row of them at a time, where each texel took its four afresh: the
  // same values summed in the same order, a quarter of the arctangents (the owner's report,
  // 2026-11-01: the high cube's 2,048² faces held the main thread some 2 s on the pod at its
  // first bake).
  let above = edges.map((x) => cornerArea(x, edges[0] ?? 0));
  for (let row = 0; row < sizePx; row += 1) {
    const y1 = edges[row + 1] ?? 0;
    const below = edges.map((x) => cornerArea(x, y1));
    for (let column = 0; column < sizePx; column += 1) {
      angles[row * sizePx + column] =
        (above[column] ?? 0) -
        (below[column] ?? 0) -
        (above[column + 1] ?? 0) +
        (below[column + 1] ?? 0);
    }
    above = below;
  }
  return angles;
}

/**
 * Where texel (column, row) of a face is in {@link texelSolidAnglesLowerF32}'s triangle: its row
 * r(r + 1)/2 + c with r the larger of the two and c the smaller, so that a texel above the diagonal
 * reads its mirror's.
 */
export function lowerTriangleIndex(column: number, row: number): number {
  const r = Math.max(column, row);
  return (r * (r + 1)) / 2 + Math.min(column, row);
}

/**
 * One face's texel solid angles as the GPU bake reads them, `f32`, sr: the lower triangle of rows
 * (column ≤ row), row after row, half the face's table (plan R06, T13.g; the owner's priority,
 * 2026-11-01).
 *
 * @remarks
 * Each is {@link texelSolidAnglesSr}'s value narrowed to `f32`, the same corners summed in the
 * same order, with no table of `f64` between (a 3,072² face's is 75 MB). A face's solid angles are
 * symmetric across its diagonal: A(a, b) = A(b, a) exactly, since ab and a² + b² commute, and a
 * texel's mirror sums the same four areas with its middle two swapped. That order can move an
 * `f64` sum by an ulp, but at the settings' face sizes no texel's `f32` value moves (the tests
 * compare every texel), so the triangle read at {@link lowerTriangleIndex} gives every texel the
 * full table's `f32` value. A face is not symmetric left to right at 3,072² (12,116 texels differ
 * from their mirrors), so no smaller part of the face serves.
 */
export function texelSolidAnglesLowerF32(sizePx: number): Float32Array {
  const edges = Array.from({ length: sizePx + 1 }, (_, i) => (2 * i) / sizePx - 1);
  const angles = new Float32Array((sizePx * (sizePx + 1)) / 2);
  // A row's texels need the corners to the column after its diagonal, and the next row's one
  // further, so each row's corners stop there: half the arctangents of the whole face's.
  const cornersOf = (y: number, count: number): number[] =>
    edges.slice(0, Math.min(count, sizePx + 1)).map((x) => cornerArea(x, y));
  let above = cornersOf(edges[0] ?? 0, 2);
  let at = 0;
  for (let row = 0; row < sizePx; row += 1) {
    const below = cornersOf(edges[row + 1] ?? 0, row + 3);
    for (let column = 0; column <= row; column += 1) {
      angles[at] =
        (above[column] ?? 0) -
        (below[column] ?? 0) -
        (above[column + 1] ?? 0) +
        (below[column + 1] ?? 0);
      at += 1;
    }
    above = below;
  }
  return angles;
}

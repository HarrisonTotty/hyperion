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
  for (let row = 0; row < sizePx; row += 1) {
    const y0 = edges[row] ?? 0;
    const y1 = edges[row + 1] ?? 0;
    for (let column = 0; column < sizePx; column += 1) {
      const x0 = edges[column] ?? 0;
      const x1 = edges[column + 1] ?? 0;
      angles[row * sizePx + column] =
        cornerArea(x0, y0) - cornerArea(x0, y1) - cornerArea(x1, y0) + cornerArea(x1, y1);
    }
  }
  return angles;
}

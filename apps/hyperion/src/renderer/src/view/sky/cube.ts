/**
 * The star cube's geometry: which face and texel a direction falls in, and each texel's solid
 * angle (plan R06, Design note 21).
 *
 * @remarks
 * Faces are in WebGPU's layer order, +X, −X, +Y, −Y, +Z, −Z, on the galactic axes, and a direction
 * maps to a face and its (u, v) as WebGPU's cube sampling does (WebGPU §"Cube map faces", after
 * the Vulkan and D3D tables): row 0 is the face's top. The bake's CPU splat and its WGSL splat
 * both follow this one mapping.
 */

/** The faces of a cube, in WebGPU's layer order. */
export const CUBE_FACE_COUNT = 6;

/** Where a direction lands on a cube of a given face size. */
export interface CubeTexel {
  /** 0–5: +X, −X, +Y, −Y, +Z, −Z. */
  readonly face: number;
  readonly column: number;
  readonly row: number;
}

/**
 * The face and texel a direction falls in.
 *
 * @param x - The direction's components, any length but not zero.
 * @param sizePx - The face's side, texels.
 * @remarks
 * Ties between axes go to x, then y, then z. A direction exactly on a face's far edge is kept on
 * the face's last texel.
 * @throws Error for a zero or non-finite direction.
 */
export function cubeTexelOf(x: number, y: number, z: number, sizePx: number): CubeTexel {
  const ax = Math.abs(x);
  const ay = Math.abs(y);
  const az = Math.abs(z);
  let face: number;
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
  if (!(major > 0) || !Number.isFinite(major)) {
    throw new Error(`(${x}, ${y}, ${z}) is not a direction`);
  }
  const s = (u / major + 1) / 2;
  const t = (v / major + 1) / 2;
  return {
    face,
    column: Math.min(sizePx - 1, Math.max(0, Math.floor(s * sizePx))),
    row: Math.min(sizePx - 1, Math.max(0, Math.floor(t * sizePx))),
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

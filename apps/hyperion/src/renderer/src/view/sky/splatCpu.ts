/**
 * The bake's CPU splat: the baked stars summed into the texels of six `rgba32float` faces, the
 * fallback where the device lacks `float32-blendable` (plan R06, Design note 21 and T13.b).
 *
 * @remarks
 * The GPU splat draws the same points as a `point-list`, one texel each, with additive blending,
 * reading {@link SPLAT_POINT_FLOATS} floats a point from a storage buffer; this function reads the
 * same buffer and adds in `f32`, as the GPU does, so the two differ only in the order of the sums
 * (to 10⁻⁶ relative, T13.g's test). A face's texel then holds the summed illuminance of its stars,
 * lx, in red, green and blue, and their count in alpha.
 */

import { CUBE_FACE_COUNT, cubeTexelOf } from "./cube";

/**
 * Floats a splat point takes: its direction (x, y, z, 0) and its illuminance per channel, lx
 * (r, g, b, 1), two `vec4f` in the WGSL splat's storage buffer.
 */
export const SPLAT_POINT_FLOATS = 8;

/**
 * Packs points into the splat's layout.
 *
 * @param directions - Three floats a point, any length but not zero, galactic axes.
 * @param illuminanceLx - Three floats a point, red, green and blue, lx.
 * @throws Error when the two arrays hold different numbers of points.
 */
export function splatPoints(directions: Float32Array, illuminanceLx: Float32Array): Float32Array {
  if (directions.length % 3 !== 0 || directions.length !== illuminanceLx.length) {
    throw new Error(
      `${directions.length} direction floats and ${illuminanceLx.length} illuminance floats are not the same whole points`,
    );
  }
  const count = directions.length / 3;
  const points = new Float32Array(count * SPLAT_POINT_FLOATS);
  for (let point = 0; point < count; point += 1) {
    const at = point * SPLAT_POINT_FLOATS;
    points.set(directions.subarray(point * 3, point * 3 + 3), at);
    points.set(illuminanceLx.subarray(point * 3, point * 3 + 3), at + 4);
    points[at + 7] = 1;
  }
  return points;
}

/**
 * Sums the points into six faces of `sizePx`² `rgba` texels.
 *
 * @param points - {@link SPLAT_POINT_FLOATS} floats a point, from {@link splatPoints}.
 * @returns Six faces in WebGPU's layer order, rows top to bottom.
 * @throws Error when `points` is not whole points.
 */
export function splatCpu(points: Float32Array, sizePx: number): Float32Array[] {
  if (points.length % SPLAT_POINT_FLOATS !== 0) {
    throw new Error(`${points.length} floats are not whole splat points`);
  }
  const faces = Array.from(
    { length: CUBE_FACE_COUNT },
    () => new Float32Array(sizePx * sizePx * 4),
  );
  for (let at = 0; at < points.length; at += SPLAT_POINT_FLOATS) {
    const { face, column, row } = cubeTexelOf(
      points[at] ?? 0,
      points[at + 1] ?? 0,
      points[at + 2] ?? 0,
      sizePx,
    );
    const texels = faces[face];
    if (texels === undefined) {
      throw new Error(`cube face ${face} is out of range`);
    }
    const texel = (row * sizePx + column) * 4;
    for (let channel = 0; channel < 4; channel += 1) {
      // A Float32Array store rounds each sum to f32, as the GPU's blend does.
      texels[texel + channel] = (texels[texel + channel] ?? 0) + (points[at + 4 + channel] ?? 0);
    }
  }
  return faces;
}

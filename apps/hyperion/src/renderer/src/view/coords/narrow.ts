import type { Vec3 } from "../../geometry/vec3";

/**
 * Narrows a camera-relative `f64` vector to the `f32` the GPU takes: the only place in the view
 * where a position becomes an `f32` (plan R02, Design note 1).
 *
 * @remarks
 * The vector must already be relative to the camera (from `relativeToCamera` or
 * `originMinusCamera`), so that the `f32`'s relative error, 6 × 10⁻⁸, stays below the
 * 5 × 10⁻⁴ rad of a pixel at every distance. A grep of `view/` for the ways of making an `f32`
 * shows that nothing else narrows a position (R02.T6.b's acceptance).
 *
 * @returns Three `f32` components, x, y, z.
 */
export function narrow(v: Vec3): Float32Array {
  return new Float32Array([v.x, v.y, v.z]);
}

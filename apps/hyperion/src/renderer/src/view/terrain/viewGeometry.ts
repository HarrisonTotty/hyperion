/**
 * A view's culling and error terms prepared once for selection's inner loop (plan R05, T7's cost
 * work, the patch-demand ruling's item 4d).
 *
 * @remarks
 * The same predicates as `cull.ts`'s {@link inFrustum} and {@link aboveHorizon} and `bounds.ts`'s
 * {@link distanceToBoxM}, over the same `f64` camera-relative differences, but on plain numbers
 * held in a typed array, so that testing a patch against a view allocates nothing. A test holds
 * the two forms to the same answers.
 */

import { NEAR_PLANE_M } from "../camera/projection";
import type { PatchBounds } from "./bounds";
import type { Frustum } from "./cull";
import type { BodyFixedVec3 } from "./planet";

/** One view, prepared: its frustum's planes, its horizon and its error scale. */
export interface ViewGeometry {
  /** The camera, body-fixed metres from the body's centre. */
  readonly cameraX: number;
  readonly cameraY: number;
  readonly cameraZ: number;
  /** Each plane's inward normal and offset, four numbers a plane, metres. */
  readonly planes: Float64Array;
  /** The camera scaled by 1 ÷ R_occ (Design note 8). */
  readonly scaledX: number;
  readonly scaledY: number;
  readonly scaledZ: number;
  /** |C|² − 1 in the scaled space; not positive at or below the occluder, which disables the test. */
  readonly horizonSq: number;
  /** R_occ, metres. */
  readonly occluderRadiusM: number;
  /** W_px ÷ (2 tan(fov_h ÷ 2) τ): ρ ÷ τ is this times the error over the distance. */
  readonly excessPerMetre: number;
}

/** Prepares a view whose frustum is `frustum`, camera `cameraM` and occluder radius `occluderRadiusM`. */
export function viewGeometry(
  frustum: Frustum,
  cameraM: BodyFixedVec3,
  occluderRadiusM: number,
  excessPerMetre: number,
): ViewGeometry {
  const planes = new Float64Array(4 * frustum.planes.length);
  frustum.planes.forEach((p, n) => {
    planes[4 * n] = p.normal.x;
    planes[4 * n + 1] = p.normal.y;
    planes[4 * n + 2] = p.normal.z;
    planes[4 * n + 3] = p.offsetM;
  });
  const sx = cameraM.x / occluderRadiusM;
  const sy = cameraM.y / occluderRadiusM;
  const sz = cameraM.z / occluderRadiusM;
  return {
    cameraX: cameraM.x,
    cameraY: cameraM.y,
    cameraZ: cameraM.z,
    planes,
    scaledX: sx,
    scaledY: sy,
    scaledZ: sz,
    horizonSq: sx * sx + sy * sy + sz * sz - 1,
    occluderRadiusM,
    excessPerMetre,
  };
}

/**
 * A view's ρ ÷ τ for a patch of bounds `b` at error `errorM` metres, or −1 where the view cannot
 * see it (outside its frustum or below its horizon). The distance is floored at the near plane,
 * so that a camera inside a volume gives a finite excess.
 */
export function viewExcess(v: ViewGeometry, b: PatchBounds, errorM: number): number {
  const box = b.box;
  const [a0, a1, a2] = box.axes;
  const [e0, e1, e2] = box.halfExtentsM;
  // The box's centre (also the sphere's) from the camera, formed once in f64.
  const cx = box.centre.x - v.cameraX;
  const cy = box.centre.y - v.cameraY;
  const cz = box.centre.z - v.cameraZ;
  const r = b.radiusM;
  const planes = v.planes;
  if (cx * cx + cy * cy + cz * cz > r * r) {
    for (let p = 0; p < planes.length; p += 4) {
      const nx = planes[p] ?? 0;
      const ny = planes[p + 1] ?? 0;
      const nz = planes[p + 2] ?? 0;
      const centre = nx * cx + ny * cy + nz * cz + (planes[p + 3] ?? 0);
      if (centre < -r) {
        return -1;
      }
      const reach =
        centre +
        Math.abs(nx * a0.x + ny * a0.y + nz * a0.z) * e0 +
        Math.abs(nx * a1.x + ny * a1.y + nz * a1.z) * e1 +
        Math.abs(nx * a2.x + ny * a2.y + nz * a2.z) * e2;
      if (reach < 0) {
        return -1;
      }
    }
  }
  if (v.horizonSq > 0) {
    const ro = v.occluderRadiusM;
    let visible = false;
    for (let corner = 0; corner < 8 && !visible; corner += 1) {
      const s0 = (corner & 1) === 0 ? -e0 : e0;
      const s1 = (corner & 2) === 0 ? -e1 : e1;
      const s2 = (corner & 4) === 0 ? -e2 : e2;
      const x = (cx + a0.x * s0 + a1.x * s1 + a2.x * s2) / ro;
      const y = (cy + a0.y * s0 + a1.y * s1 + a2.y * s2) / ro;
      const z = (cz + a0.z * s0 + a1.z * s1 + a2.z * s2) / ro;
      const vtDotVc = -(x * v.scaledX + y * v.scaledY + z * v.scaledZ);
      visible = !(
        vtDotVc > v.horizonSq && (vtDotVc * vtDotVc) / (x * x + y * y + z * z) > v.horizonSq
      );
    }
    if (!visible) {
      return -1;
    }
  }
  const o0 = Math.max(0, Math.abs(cx * a0.x + cy * a0.y + cz * a0.z) - e0);
  const o1 = Math.max(0, Math.abs(cx * a1.x + cy * a1.y + cz * a1.z) - e1);
  const o2 = Math.max(0, Math.abs(cx * a2.x + cy * a2.y + cz * a2.z) - e2);
  // No nearer than the near plane: a camera inside a volume has the error of one 0.1 m away, so
  // the excess stays finite and a secondary view's weight still ranks it (Design note 24).
  const d = Math.max(Math.sqrt(o0 * o0 + o1 * o1 + o2 * o2), NEAR_PLANE_M);
  return (errorM * v.excessPerMetre) / d;
}

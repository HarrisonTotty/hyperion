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
import {
  PACKED_AXES,
  PACKED_BOUNDS_LENGTH,
  PACKED_CENTRE,
  PACKED_HALF_EXTENTS,
  PACKED_RADIUS,
  packBounds,
  type PatchBounds,
} from "./bounds";
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

/** Scratch for packing a {@link PatchBounds} for the packed forms; used within one call. */
const PACKED = new Float64Array(PACKED_BOUNDS_LENGTH);

/**
 * The packed layout's offsets and the near plane, bound once here. The packed forms read them on
 * every call, and under a module runner (the descent record runs under Vite's) each read of an
 * imported binding is a getter call.
 */
const CENTRE = PACKED_CENTRE;
const RADIUS = PACKED_RADIUS;
const AXES = PACKED_AXES;
const HALF = PACKED_HALF_EXTENTS;
const NEAR_M = NEAR_PLANE_M;

/**
 * A view's ρ ÷ τ for a patch of bounds `b` at error `errorM` metres, or −1 where the view cannot
 * see it (outside its frustum or below its horizon). The distance is floored at the near plane,
 * so that a camera inside a volume gives a finite excess.
 */
export function viewExcess(v: ViewGeometry, b: PatchBounds, errorM: number): number {
  packBounds(PACKED, b);
  return viewExcessPacked(v, PACKED, errorM);
}

/**
 * {@link viewExcess} of packed bounds `p` ({@link PACKED_BOUNDS_LENGTH}'s layout): the one form of
 * the test, which selection calls on the bounds it keeps packed (R05.T7 perf (d)).
 */
export function viewExcessPacked(v: ViewGeometry, p: Float64Array, errorM: number): number {
  const a0x = p[AXES] ?? 0;
  const a0y = p[AXES + 1] ?? 0;
  const a0z = p[AXES + 2] ?? 0;
  const a1x = p[AXES + 3] ?? 0;
  const a1y = p[AXES + 4] ?? 0;
  const a1z = p[AXES + 5] ?? 0;
  const a2x = p[AXES + 6] ?? 0;
  const a2y = p[AXES + 7] ?? 0;
  const a2z = p[AXES + 8] ?? 0;
  const e0 = p[HALF] ?? 0;
  const e1 = p[HALF + 1] ?? 0;
  const e2 = p[HALF + 2] ?? 0;
  // The box's centre (also the sphere's) from the camera, formed once in f64.
  const cx = (p[CENTRE] ?? 0) - v.cameraX;
  const cy = (p[CENTRE + 1] ?? 0) - v.cameraY;
  const cz = (p[CENTRE + 2] ?? 0) - v.cameraZ;
  const r = p[RADIUS] ?? 0;
  const planes = v.planes;
  if (cx * cx + cy * cy + cz * cz > r * r) {
    for (let q = 0; q < planes.length; q += 4) {
      const nx = planes[q] ?? 0;
      const ny = planes[q + 1] ?? 0;
      const nz = planes[q + 2] ?? 0;
      const centre = nx * cx + ny * cy + nz * cz + (planes[q + 3] ?? 0);
      if (centre < -r) {
        return -1;
      }
      // The box's reach past the plane is the centre's plus terms none of which is negative (the
      // half-extents never are), so it can fall below 0 only where the centre's does.
      if (centre < 0) {
        const reach =
          centre +
          Math.abs(nx * a0x + ny * a0y + nz * a0z) * e0 +
          Math.abs(nx * a1x + ny * a1y + nz * a1z) * e1 +
          Math.abs(nx * a2x + ny * a2y + nz * a2z) * e2;
        if (reach < 0) {
          return -1;
        }
      }
    }
  }
  // The centre's offsets along the axes, which the horizon's corner order and the distance share.
  const along0 = cx * a0x + cy * a0y + cz * a0z;
  const along1 = cx * a1x + cy * a1y + cz * a1z;
  const along2 = cx * a2x + cy * a2y + cz * a2z;
  if (v.horizonSq > 0) {
    const ro = v.occluderRadiusM;
    // Visible where any corner is: tried from the top corner nearest the camera, which a patch
    // above the horizon most often shows, so that the scan stops sooner. The answer is the same
    // whatever the order.
    const near1 = along1 > 0 ? -e1 : e1;
    const near2 = along2 > 0 ? -e2 : e2;
    let visible = false;
    for (let corner = 0; corner < 8 && !visible; corner += 1) {
      const s0 = (corner & 1) === 0 ? e0 : -e0;
      const s1 = (corner & 2) === 0 ? near1 : -near1;
      const s2 = (corner & 4) === 0 ? near2 : -near2;
      const x = (cx + a0x * s0 + a1x * s1 + a2x * s2) / ro;
      const y = (cy + a0y * s0 + a1y * s1 + a2y * s2) / ro;
      const z = (cz + a0z * s0 + a1z * s1 + a2z * s2) / ro;
      const vtDotVc = -(x * v.scaledX + y * v.scaledY + z * v.scaledZ);
      visible = !(
        vtDotVc > v.horizonSq && (vtDotVc * vtDotVc) / (x * x + y * y + z * z) > v.horizonSq
      );
    }
    if (!visible) {
      return -1;
    }
  }
  const o0 = Math.max(0, Math.abs(along0) - e0);
  const o1 = Math.max(0, Math.abs(along1) - e1);
  const o2 = Math.max(0, Math.abs(along2) - e2);
  // No nearer than the near plane: a camera inside a volume has the error of one 0.1 m away, so
  // the excess stays finite and a secondary view's weight still ranks it (Design note 24).
  const d = Math.max(Math.sqrt(o0 * o0 + o1 * o1 + o2 * o2), NEAR_M);
  return (errorM * v.excessPerMetre) / d;
}

/**
 * The distance from `pointM`, body-fixed metres from the body's centre, to the nearest point of a
 * patch's box, metres; 0 inside it.
 *
 * @remarks
 * The same `f64` operations in the same order as {@link distanceToBoxM} of
 * `relativeBounds(b, pointM)`, with nothing allocated: the forced-region test's distance (Design
 * note 9).
 */
export function distanceToBoxFromM(b: PatchBounds, pointM: BodyFixedVec3): number {
  packBounds(PACKED, b);
  return distanceToPackedBoxFromM(PACKED, pointM);
}

/** {@link distanceToBoxFromM} of packed bounds `p` ({@link PACKED_BOUNDS_LENGTH}'s layout). */
export function distanceToPackedBoxFromM(p: Float64Array, pointM: BodyFixedVec3): number {
  const cx = (p[CENTRE] ?? 0) - pointM.x;
  const cy = (p[CENTRE + 1] ?? 0) - pointM.y;
  const cz = (p[CENTRE + 2] ?? 0) - pointM.z;
  const o0 = Math.max(
    0,
    Math.abs(cx * (p[AXES] ?? 0) + cy * (p[AXES + 1] ?? 0) + cz * (p[AXES + 2] ?? 0)) -
      (p[HALF] ?? 0),
  );
  const o1 = Math.max(
    0,
    Math.abs(cx * (p[AXES + 3] ?? 0) + cy * (p[AXES + 4] ?? 0) + cz * (p[AXES + 5] ?? 0)) -
      (p[HALF + 1] ?? 0),
  );
  const o2 = Math.max(
    0,
    Math.abs(cx * (p[AXES + 6] ?? 0) + cy * (p[AXES + 7] ?? 0) + cz * (p[AXES + 8] ?? 0)) -
      (p[HALF + 2] ?? 0),
  );
  let sum = 0;
  sum += o0 * o0;
  sum += o1 * o1;
  sum += o2 * o2;
  return Math.sqrt(sum);
}

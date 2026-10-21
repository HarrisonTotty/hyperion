/**
 * Where the light shows a system on a spatial view, against where it is now (plan 12, P12.T11).
 *
 * @remarks
 * In observed mode a chart draws every system at its present position; the apparent position, where
 * the light the observer receives shows it, is a tick joined to the present position by a dashed
 * line (the guide's spatial displays). These helpers say how far apart the two are on the screen.
 */
import type { LocalFrame } from "../geometry/frame";
import { dot, sub, type Vec3 } from "../geometry/vec3";
import { type Camera, viewBasis } from "./camera";

/** An offset on the screen, CSS px: right and down, as screen coordinates run. */
export interface ScreenOffset {
  readonly dxPx: number;
  readonly dyPx: number;
}

/**
 * The offset on the screen beyond which a system that is not selected is drawn with its apparent
 * position's tick and joiner, CSS px: four pixels (plan 12, P12.T11). The selected system has its
 * own whatever its offset, and both are drawn only outside the mark's outline.
 */
export const APPARENT_OFFSET_MIN_PX = 4;

/**
 * The screen offset, CSS px, from a system's present position to its apparent position under the
 * view's projection.
 *
 * @remarks
 * Pure. The view is orthographic (`project`), so the offset depends on the camera's angles and
 * zoom alone, not on the viewport or where on it the system lies: dx = k·(d·right) and
 * dy = −k·(d·up), with d the apparent position less the present one and k the camera's pixels per
 * scene unit. Seen along d, the two positions coincide on the screen and the offset is zero.
 *
 * @param present - The present position, relative to the view centre in scene units.
 * @param apparent - The apparent position, in the same units and axes.
 */
export function apparentOffset(
  present: Vec3,
  apparent: Vec3,
  frame: LocalFrame,
  camera: Camera,
): ScreenOffset {
  const basis = viewBasis(frame, camera);
  const delta = sub(apparent, present);
  return {
    dxPx: camera.pxPerUnit * dot(delta, basis.right),
    dyPx: -camera.pxPerUnit * dot(delta, basis.up),
  };
}

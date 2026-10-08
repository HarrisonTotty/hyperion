/**
 * A drag's turn of a view's camera (plan R07, T19.f): the pointer's travel across the canvas as a
 * turn of the line of sight, sized by the field of view so that the scene follows the pointer.
 */

import type { ViewTurn } from "./look";

/** A point on a view's canvas, CSS px from its top left. */
export interface CanvasPointPx {
  readonly xPx: number;
  readonly yPx: number;
}

/** A canvas's laid-out size, CSS px. */
export interface CanvasSizePx {
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * The turn that carries the direction under `fromPx` to `toPx`, on a canvas of `sizePx` whose
 * horizontal field of view is `fovXDeg`.
 *
 * @remarks
 * With x and y from the canvas's centre and the focal length f = (W ÷ 2) ÷ tan(FOV ÷ 2) in CSS
 * px, the yaw is atan(x₂ ÷ f) − atan(x₁ ÷ f) and the pitch atan(y₂ ÷ f) − atan(y₁ ÷ f): exact for
 * a direction on the centre lines, so a point dragged along either stays under the pointer, and
 * right to first order elsewhere. A drag to the right turns the line of sight left (a positive
 * yaw), and a drag down pitches it up, since screen y runs down: the far scene moves with the
 * pointer. A canvas not yet laid out turns nothing.
 *
 * @param fovXDeg - The view's horizontal field of view, degrees, in (0, 180).
 */
export function dragTurn(
  fromPx: CanvasPointPx,
  toPx: CanvasPointPx,
  sizePx: CanvasSizePx,
  fovXDeg: number,
): ViewTurn {
  if (!(sizePx.widthPx > 0 && sizePx.heightPx > 0)) {
    return { yawRad: 0, pitchRad: 0 };
  }
  const focalPx = sizePx.widthPx / 2 / Math.tan((fovXDeg * Math.PI) / 360);
  const centreX = sizePx.widthPx / 2;
  const centreY = sizePx.heightPx / 2;
  const angle = (offsetPx: number): number => Math.atan(offsetPx / focalPx);
  return {
    yawRad: angle(toPx.xPx - centreX) - angle(fromPx.xPx - centreX),
    pitchRad: angle(toPx.yPx - centreY) - angle(fromPx.yPx - centreY),
  };
}

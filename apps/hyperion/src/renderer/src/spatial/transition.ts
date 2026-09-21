import { type Camera, clampElevationDeg, wrapAzimuthDeg } from "./camera";

/**
 * How long a preset change takes, inside the guide's 80 to 150 ms for state transitions.
 *
 * @remarks
 * Skipped entirely under `prefers-reduced-motion: reduce`.
 */
export const TRANSITION_MS = 120;

/**
 * The guide's simple ease-out, 1 − (1 − t)³, for `t` from 0 to 1.
 *
 * @remarks
 * `t` outside [0, 1] is clamped, so a late frame lands exactly on the end.
 */
export function easeOut(t: number): number {
  const clamped = Math.min(1, Math.max(0, t));
  return 1 - (1 - clamped) ** 3;
}

/**
 * The camera part of the way from `from` to `to`.
 *
 * @remarks
 * The azimuth turns along the shorter arc, and the elevation and the logarithm of the zoom move
 * linearly, so that zooming feels even. At `progress` 1 or more the result is `to` itself.
 *
 * @param progress - Eased progress, 0 at `from` and 1 at `to`.
 */
export function tweenCamera(from: Camera, to: Camera, progress: number): Camera {
  if (progress >= 1) {
    return to;
  }
  if (progress <= 0) {
    return from;
  }
  const turnDeg = ((((to.azimuthDeg - from.azimuthDeg) % 360) + 540) % 360) - 180;
  const zoomRatio = Math.log(to.pxPerUnit / from.pxPerUnit);
  return {
    azimuthDeg: wrapAzimuthDeg(from.azimuthDeg + turnDeg * progress),
    elevationDeg: clampElevationDeg(
      from.elevationDeg + (to.elevationDeg - from.elevationDeg) * progress,
    ),
    pxPerUnit: from.pxPerUnit * Math.exp(zoomRatio * progress),
  };
}

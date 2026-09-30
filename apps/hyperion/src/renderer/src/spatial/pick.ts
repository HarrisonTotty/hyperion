import type { Anchor, ScreenPoint } from "./drawList";

/** Screen distances closer than this are a tie, settled by depth and then by ID. */
const TIE_PX = 0.5;

/**
 * The mark a pointer at `pointPx` picks, or `null` when none is near enough.
 *
 * @remarks
 * A mark is a candidate when its centre is within the larger of `tolerancePx` and its own radius,
 * so a large symbol can be picked at its rim. The nearest candidate on screen wins. Candidates
 * less than half a pixel further than the nearest tie with it, and of those the one nearest the
 * viewer (smallest depth) wins, then the lowest ID. The result does not depend on the order of
 * `anchors`.
 *
 * @param tolerancePx - How far a pointer may miss; the chart passes 1 rem, half the guide's
 *   2 rem target.
 */
export function pick(
  anchors: ReadonlyArray<Anchor>,
  pointPx: ScreenPoint,
  tolerancePx: number,
): string | null {
  const candidates: Array<{ anchor: Anchor; distancePx: number }> = [];
  let nearestPx = Number.POSITIVE_INFINITY;
  for (const anchor of anchors) {
    const distancePx = Math.hypot(anchor.xPx - pointPx.xPx, anchor.yPx - pointPx.yPx);
    if (distancePx <= Math.max(tolerancePx, anchor.radiusPx)) {
      candidates.push({ anchor, distancePx });
      nearestPx = Math.min(nearestPx, distancePx);
    }
  }
  let best: Anchor | null = null;
  for (const { anchor, distancePx } of candidates) {
    if (distancePx - nearestPx >= TIE_PX) {
      continue;
    }
    const better =
      best === null ||
      anchor.depth < best.depth ||
      (anchor.depth === best.depth && anchor.id < best.id);
    if (better) {
      best = anchor;
    }
  }
  return best?.id ?? null;
}

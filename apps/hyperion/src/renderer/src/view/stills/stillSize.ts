/**
 * A still's size (plan R11, R11.T10.e.1; ruling 6 of decision-leans-54): the console's choice, by
 * default the view's output size at scale 1, never below it, up to `STILL_MAX_SIDE_PX` on a side.
 *
 * @remarks
 * Pure. The sizes offered are whole multiples of the view's output size, its canvas's device
 * pixels whatever its internal scale or the setting's render rows, so that a still keeps the view's
 * aspect exactly and every point lands at the view's position times the multiple: R06's discs and
 * the sprites, prepared at the still's size, land where T10.a's projection puts them.
 */

import type { ViewSize } from "../engine/types";
import { STILL_MAX_SIDE_PX } from "./stillLabel";

/**
 * The multiples of the view's output size a still may be taken at: 1 to 4, so that a 1080p view
 * gives up to an 8K UHD still of 7,680 × 4,320, within {@link STILL_MAX_SIDE_PX} (a lean,
 * 2026-11-18).
 */
export const STILL_SCALES: ReadonlyArray<number> = [1, 2, 3, 4];

/** One size a still may be taken at: its multiple of the view's output size, and its pixels. */
export interface StillSizeOption {
  readonly scale: number;
  readonly size: ViewSize;
}

/** The still's size at `scale` times the view's output size `output`, px. */
export function stillSizeAt(output: ViewSize, scale: number): ViewSize {
  return { widthPx: output.widthPx * scale, heightPx: output.heightPx * scale };
}

/**
 * The sizes a view of output size `output` offers: every {@link STILL_SCALES} multiple whose sides
 * are within {@link STILL_MAX_SIDE_PX}, the output size itself always, the smallest first.
 */
export function stillSizeOptions(output: ViewSize): ReadonlyArray<StillSizeOption> {
  return STILL_SCALES.filter(
    (scale) =>
      scale === 1 || Math.max(output.widthPx, output.heightPx) * scale <= STILL_MAX_SIDE_PX,
  ).map((scale) => ({ scale, size: stillSizeAt(output, scale) }));
}

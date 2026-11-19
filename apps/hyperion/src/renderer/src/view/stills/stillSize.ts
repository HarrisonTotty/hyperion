/**
 * A still's size (plan R11, R11.T10.e.1; ruling 6 of decision-leans-54): the console's choice, by
 * default the view's output size at scale 1, never below it but where it draws air (below), up to
 * `STILL_MAX_SIDE_PX` on a side.
 *
 * @remarks
 * Pure. The sizes offered are whole multiples of the view's output size, its canvas's device
 * pixels whatever its internal scale or the setting's render rows, so that a still keeps the view's
 * aspect exactly and every point lands at the view's position times the multiple: R06's discs and
 * the sprites, prepared at the still's size, land where T10.a's projection puts them.
 *
 * A still that draws air is drawn untiled, so its size is held within the tile limit, the view's
 * setting's `STILL_TILE_MAX_PX` (plan R11, R11.T10.e; main's ruling, 2026-11-19, its lean taken):
 * {@link stillSizeWithin}, its aspect kept, the size the `SIZE` choice offers and the label states.
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
 * `size` held within `limitPx` on its longer side (main's ruling on stills with air, 2026-11-19): the
 * longer side made `limitPx`, the shorter its share of it rounded to the pixel, at least 1, so that
 * the aspect is kept to within half a pixel; `size` itself where it is within the limit already.
 *
 * @remarks
 * The sprites are carried to a still by the width's ratio and the projection takes the still's own
 * aspect (`spritesAtScale`), so a scale that is not whole lands every point where the projection
 * puts it whatever the rounding.
 *
 * @param limitPx - The longest side, px: a positive integer.
 * @throws RangeError for a limit that is not a positive integer.
 */
export function stillSizeWithin(size: ViewSize, limitPx: number): ViewSize {
  if (!(Number.isSafeInteger(limitPx) && limitPx > 0)) {
    throw new RangeError("a still's size limit must be a positive integer");
  }
  const { widthPx, heightPx } = size;
  if (Math.max(widthPx, heightPx) <= limitPx) {
    return size;
  }
  return widthPx >= heightPx
    ? { widthPx: limitPx, heightPx: Math.max(1, Math.round((heightPx * limitPx) / widthPx)) }
    : { widthPx: Math.max(1, Math.round((widthPx * limitPx) / heightPx)), heightPx: limitPx };
}

/**
 * The sizes a view of output size `output` offers: every {@link STILL_SCALES} multiple whose sides
 * are within {@link STILL_MAX_SIDE_PX}, the output size itself always, the smallest first; with
 * `limitPx`, a still that draws air, each held within it ({@link stillSizeWithin}), a multiple
 * that would draw the size of the one before it left out, so that every size offered is one drawn.
 *
 * @param limitPx - The longest side a still may be drawn at now, px; absent or `null`, none.
 */
export function stillSizeOptions(
  output: ViewSize,
  limitPx: number | null = null,
): ReadonlyArray<StillSizeOption> {
  const options: StillSizeOption[] = [];
  for (const scale of STILL_SCALES) {
    if (scale !== 1 && Math.max(output.widthPx, output.heightPx) * scale > STILL_MAX_SIDE_PX) {
      continue;
    }
    const whole = stillSizeAt(output, scale);
    const size = limitPx === null ? whole : stillSizeWithin(whole, limitPx);
    const before = options.at(-1);
    if (
      before === undefined ||
      before.size.widthPx !== size.widthPx ||
      before.size.heightPx !== size.heightPx
    ) {
      options.push({ scale, size });
    }
  }
  return options;
}

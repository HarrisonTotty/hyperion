/** The rows of a scrolling list that are in view, and the rows to render around them. */
export interface WindowRange {
  /** 0-based index of the first row to render, overscan included. */
  readonly first: number;
  /** 0-based index of the last row to render, inclusive, overscan included. */
  readonly last: number;
  /** 0-based index of the first row at least partly in view. */
  readonly firstVisible: number;
  /** 0-based index of the last row at least partly in view, inclusive. */
  readonly lastVisible: number;
}

// Keeps a row boundary that lands a hair off an exact pixel, at fractional interface scales, from
// counting the neighbouring row as visible.
const EDGE_TOLERANCE_ROWS = 1e-6;

/**
 * Works out which rows of a fixed-row-height list are in view and which to render.
 *
 * @remarks
 * Shared by every scrolling list, windowed or not: the visible pair feeds `formatListPosition`,
 * and `first` and `last` bound what a windowed list renders. A scroll offset outside the list
 * (elastic overscroll, or a list that shrank) is clamped to it. A viewport of no height still
 * reports the row at the top as visible, so that the position readout always names a row.
 *
 * @param scrollTopPx - The list's scroll offset.
 * @param rowHeightPx - Height of every row; must be positive.
 * @param viewportHeightPx - Height of the list's visible area.
 * @param total - Number of rows in the list.
 * @param overscan - Rows to render beyond each end of the visible ones.
 * @returns The range, or `null` when the list has no rows.
 * @throws RangeError when the row height is not positive, an offset or height is not finite, or a
 *   count is not a whole number.
 */
export function windowRange(
  scrollTopPx: number,
  rowHeightPx: number,
  viewportHeightPx: number,
  total: number,
  overscan: number,
): WindowRange | null {
  if (!(rowHeightPx > 0) || !Number.isFinite(rowHeightPx)) {
    throw new RangeError(`row height must be positive, got ${String(rowHeightPx)}`);
  }
  if (!Number.isFinite(scrollTopPx) || !Number.isFinite(viewportHeightPx)) {
    throw new RangeError(
      `scroll offset and viewport height must be finite, got ${scrollTopPx} and ${viewportHeightPx}`,
    );
  }
  if (!Number.isInteger(total) || total < 0 || !Number.isInteger(overscan) || overscan < 0) {
    throw new RangeError(`row counts must be whole, got ${total} rows and ${overscan} overscan`);
  }
  if (total === 0) {
    return null;
  }
  const viewportPx = Math.max(0, viewportHeightPx);
  const maxScrollPx = Math.max(0, total * rowHeightPx - viewportPx);
  const scrollPx = Math.min(Math.max(0, scrollTopPx), maxScrollPx);
  const lastRow = total - 1;

  const firstVisible = Math.min(lastRow, Math.floor(scrollPx / rowHeightPx + EDGE_TOLERANCE_ROWS));
  const endRows = (scrollPx + viewportPx) / rowHeightPx;
  const lastVisible = Math.min(
    lastRow,
    Math.max(firstVisible, Math.ceil(endRows - EDGE_TOLERANCE_ROWS) - 1),
  );
  return {
    first: Math.max(0, firstVisible - overscan),
    last: Math.min(lastRow, lastVisible + overscan),
    firstVisible,
    lastVisible,
  };
}

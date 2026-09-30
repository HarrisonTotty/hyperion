import type { ElementSize } from "../../lib/useElementSize";
import { COMPACT_BELOW_REM } from "./chartLayout";

/**
 * How the `GALAXY MAP` page is laid out (plan 07, P07.T11.b): `stacked`, the page's controls and
 * each view's words in a column beside the pictures, which take the page's height; or `compact`,
 * where the page is too short for that, with the controls in a row across the page above the
 * pictures and a wider column of words beside them.
 */
export type MapLayout = "stacked" | "compact";

/**
 * The layout of a `GALAXY MAP` page of `size`: `compact` when it is shorter than the local chart's
 * {@link COMPACT_BELOW_REM}, and `stacked` otherwise and before it is measured.
 *
 * @remarks
 * The rule is the `LOCAL CHART` page's (the orchestrator's rulings 134.9 and 149.2), so that the
 * two pages of the panel change layout together: 1280 × 720 (a page of about 32 rem) is compact,
 * 1920 × 1080 at 100% (about 55 rem) stacked. It is made on the page's height in `rem`, so it
 * follows the interface scale, and needs no hysteresis, since the page's height does not depend on
 * its layout.
 */
export function mapLayout(size: ElementSize | null): MapLayout {
  if (size === null || !(size.remPx > 0)) {
    return "stacked";
  }
  return size.heightPx < COMPACT_BELOW_REM * size.remPx ? "compact" : "stacked";
}

/*
 * The page's layout, as `styles.css` lays out `.galaxy-map__grid`: a column for the words, at
 * least SIDE_REM wide (COMPACT_SIDE_REM compact), the gutter for the vertical axes' labels, the
 * pictures, and the gutter for the horizontal axes' labels; face-on above edge-on, VIEW_GAP_REM
 * apart. Stacked, the pictures start TABS_CLEAR_REM down, clear of the panel's page tabs, which
 * stand at its top left; compact, they start under the controls' row, COMPACT_CONTROLS_REM tall,
 * which clears the tabs itself.
 */
const SIDE_REM = 24;
/**
 * The compact words column: the face-on view's title and key hint side by side (about 26.7 rem in
 * B612 at 1280 × 720) and each legend's floor on one line (about 25 rem).
 */
const COMPACT_SIDE_REM = 27;
const AXIS_BEFORE_REM = 3.75;
const AXIS_AFTER_REM = 1.75;
const VIEW_GAP_REM = 0.5;
const TABS_CLEAR_REM = 2.5;
/**
 * The compact controls' row: the tabs' clearance, a line of the population and quantity choices
 * (2 rem), a line of the overlay's toggle and name and the frame and scale (2.25 rem, the name
 * taking two lines of text), and the gaps.
 */
const COMPACT_CONTROLS_REM = 7.5;

/**
 * The pictures' width in CSS pixels for a page of `size` laid out `layout`: as wide as the page
 * leaves beside the words and axes, and no taller together than the page leaves them, the edge-on
 * picture being half the face-on picture's height; even, so that the edge-on picture is a whole
 * number of pixels tall.
 */
export function mapPictureWidth(size: ElementSize, layout: MapLayout): number {
  const compact = layout === "compact";
  const sideRem = compact ? COMPACT_SIDE_REM : SIDE_REM;
  const aboveRem = compact ? COMPACT_CONTROLS_REM : TABS_CLEAR_REM;
  const acrossPx = size.widthPx - (sideRem + AXIS_BEFORE_REM + AXIS_AFTER_REM) * size.remPx;
  const downPx = ((size.heightPx - (aboveRem + VIEW_GAP_REM) * size.remPx) * 2) / 3;
  return Math.max(0, 2 * Math.floor(Math.min(acrossPx, downPx) / 2));
}

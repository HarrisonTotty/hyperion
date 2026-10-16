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
 * (2 rem), a line of the overlay's toggle and its name or why it is held back, and the frame and
 * scale (2.25 rem, the name or the reason taking two lines of text), and the gaps.
 */
const COMPACT_CONTROLS_REM = 7.5;

/** The tallest words of a view's two kinds of state, face-on and edge-on, in `rem`. */
interface ViewWordsRem {
  readonly faceOn: number;
  readonly edgeOn: number;
}

/** The tallest words of each layout, plain and under the overlay. */
interface LayoutWordsRem {
  readonly plain: ViewWordsRem;
  readonly overlay: ViewWordsRem;
}

/**
 * The tallest words a view shows beside its picture, in `rem`, as measured in hidden runs of the
 * console (decision-gg-port-d0 §3.4), each plus the 0.5 rem of headroom the ruling asks for B612 as
 * macOS and Windows render it.
 *
 * @remarks
 * `plain` is the tallest of `COLUMN DENSITY` and `EXTINCTION` in every data state the ruling names
 * (nominal, pending, refused, and with the link down). A stale view adds no line of its own in any
 * state: the page states the link's reason, or a stale view's `PENDING`, once for both views in one
 * line where the link's reason stands, and a new map's failure stands in the picture's place
 * (§3.3; decision-gg-port-a2 §2); `overlay` is the `DUST OVERLAY`'s,
 * its two readings and its two legends side by side. Stacked, the face-on words stand under the
 * page's controls, so theirs include the controls, less the tabs' clearance, which the picture's
 * own top keeps too. Each is the tallest across the sizes and shell states measured, so at the
 * narrowest words column its layout gives (24 rem stacked, 27 rem compact), where the words wrap
 * the most. The figures and the runs are plan 07's P07.T11.b record, and its P07.T11.d record for
 * the stacked overlay's face-on words, which since then count the overlay's brightness line and
 * the page's line for the views' wait.
 */
const WORDS_REM: Readonly<Record<MapLayout, LayoutWordsRem>> = {
  // 150 px face-on and 128 px edge-on plain, 222 px and 200 px under the overlay, at 1280 × 720 and
  // 1536 × 864 (a 27–28 rem column).
  compact: {
    plain: { faceOn: 9.875, edgeOn: 8.5 },
    overlay: { faceOn: 14.375, edgeOn: 13 },
  },
  // Face-on with the controls: 414 px plain at 1280 × 1024 (the 24 rem column, the link's reason
  // and the room's above the words); 490 px under the overlay, its name and brightness line among
  // the controls and the page's line for the views' wait above them, the link's reason or a stale
  // view's PENDING (P07.T11.d; 464 px without that line), at 1280 × 1024 and 1920 × 1080 alike;
  // edge-on 146 px plain and 218 px under the overlay, at both sizes too.
  stacked: {
    plain: { faceOn: 26.375, edgeOn: 9.625 },
    overlay: { faceOn: 31.125, edgeOn: 14.125 },
  },
};

/**
 * The pictures' width in CSS pixels for a page of `size` laid out `layout`: as wide as the page
 * leaves beside the words and axes, and no taller together than the page leaves them, the edge-on
 * picture being half the face-on picture's height; even, so that the edge-on picture is a whole
 * number of pixels tall.
 *
 * @remarks
 * The pictures are never given a width at which a plain state's words would take the page past its
 * foot (decision-gg-port-d0 §3.2.2). Compact, each view's row is as tall as its picture or its
 * words, whichever is taller, so where the edge-on row's tallest plain words are taller than half
 * the picture the height allows, the pictures take what those words leave, and the face-on row
 * likewise. Stacked, the rows are {@link mapRows}', which a page of 44 rem or more always holds:
 * the pictures are as wide as it allows. The rule reads the page's size alone, never what it shows,
 * so nothing moves as the quantity, the overlay or a data state changes.
 */
export function mapPictureWidth(size: ElementSize, layout: MapLayout): number {
  const remPx = size.remPx;
  if (layout === "stacked") {
    const acrossPx = size.widthPx - (SIDE_REM + AXIS_BEFORE_REM + AXIS_AFTER_REM) * remPx;
    const downPx = ((size.heightPx - (TABS_CLEAR_REM + VIEW_GAP_REM) * remPx) * 2) / 3;
    return evenWidth(Math.min(acrossPx, downPx));
  }
  const acrossPx = size.widthPx - (COMPACT_SIDE_REM + AXIS_BEFORE_REM + AXIS_AFTER_REM) * remPx;
  // The height both rows share, under the controls and less the gap between the views.
  const rowsPx = size.heightPx - (COMPACT_CONTROLS_REM + VIEW_GAP_REM) * remPx;
  const { faceOn, edgeOn } = WORDS_REM.compact.plain;
  return evenWidth(
    Math.min(
      acrossPx,
      (rowsPx * 2) / 3,
      // The edge-on row as tall as its words, the face-on picture taking the rest.
      rowsPx - edgeOn * remPx,
      // The face-on row as tall as its words, the edge-on picture half the rest.
      2 * (rowsPx - faceOn * remPx),
    ),
  );
}

/** The largest even whole number of pixels not above `widthPx`, and 0 for none. */
function evenWidth(widthPx: number): number {
  return Math.max(0, 2 * Math.floor(widthPx / 2));
}

/** The height each view takes on the page, in CSS pixels. */
export interface MapRows {
  /** The face-on view's: its picture or its words, and stacked the tabs' clearance above them. */
  readonly faceOnPx: number;
  /** The edge-on view's: its picture or its words. */
  readonly edgeOnPx: number;
}

/**
 * The height each view takes on a page of `size` laid out `layout`: its picture's, or its tallest
 * plain words' where they are taller, so that no plain state's words outrun their row
 * (decision-gg-port-d0 §3.2.2).
 *
 * @remarks
 * Stacked, the face-on view's words stand under the page's controls, beside a picture that starts
 * the tabs' clearance down; on a page too narrow for a picture as tall as those words, as at
 * 1280 × 1024, the view keeps their height and its picture stands at its top. `styles.css` lays the
 * stacked rows out by these, so the rows come from the page's size alone; compact rows are as tall
 * as their picture or their words, which `mapPictureWidth` keeps on the page.
 */
export function mapRows(size: ElementSize, layout: MapLayout): MapRows {
  const pictureWidthPx = mapPictureWidth(size, layout);
  const { faceOn, edgeOn } = WORDS_REM[layout].plain;
  const clearPx = layout === "stacked" ? TABS_CLEAR_REM * size.remPx : 0;
  return {
    faceOnPx: Math.max(pictureWidthPx, faceOn * size.remPx) + clearPx,
    edgeOnPx: Math.max(pictureWidthPx / 2, edgeOn * size.remPx),
  };
}

/**
 * Whether a page of `size` laid out `layout` has room for the `DUST OVERLAY`: whether the rows it
 * gives anyway hold the overlay's words beside the pictures (decision-gg-port-d0 §3.2.3).
 *
 * @remarks
 * The face-on view must be as tall as its words under the overlay (stacked, with the page's
 * controls above them), and the edge-on view as tall as its own; where the pictures are taller than
 * the plain words, as they are at every size measured but 1280 × 1024, that is the ruling's test of
 * the face-on picture's height and of half of it. The rule reads the page's size alone, so the
 * overlay never changes the layout, and nothing moves when it is put on. At 1280 × 720 the
 * overlay's words alone are taller than the page, and it has no room; at 1920 × 1080 it has. Where
 * it has none, the page holds the toggle back with its reason, as the `VIEW` display holds back a
 * slot it has no room for.
 */
export function overlayHasRoom(size: ElementSize, layout: MapLayout): boolean {
  const rows = mapRows(size, layout);
  const words = WORDS_REM[layout].overlay;
  const clearPx = layout === "stacked" ? TABS_CLEAR_REM * size.remPx : 0;
  return (
    mapPictureWidth(size, layout) > 0 &&
    words.faceOn * size.remPx <= rows.faceOnPx - clearPx &&
    words.edgeOn * size.remPx <= rows.edgeOnPx
  );
}

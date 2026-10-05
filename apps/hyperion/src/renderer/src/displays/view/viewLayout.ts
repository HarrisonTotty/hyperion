/**
 * How the `VIEW` display is laid out (plan R07, R07.T19.b; decision-r07-t19-layout, item 1): two
 * layouts, chosen from the size of the `.view` box alone and never from what it shows, so that
 * nothing moves when the style, the instruments or a reason changes.
 */
import type { ReactNode } from "react";

import type { ElementSize } from "../../lib/useElementSize";

/**
 * `full`, the stage beside two side columns (`Instruments`, `Targets`, `Camera` and `Style`; then
 * `Exposure` and `Exposure meter`); or `compact`, one column whose `Camera`, `Style`, `Exposure`
 * and `Exposure meter` panels fold behind a row of disclosure buttons, one open at a time.
 */
export type ViewLayout = "full" | "compact";

/**
 * The least width of the `.view` box for the full layout, rem: a 50.5 rem stage (a 35 rem
 * instrument slot, three 0.5 rem insets and the primary's least 14 rem label block), then the
 * 26 rem and 20 rem columns, each after a 1 rem gap.
 */
export const FULL_MIN_WIDTH_REM = 98.5;

/**
 * The least height of the `.view` box for the full layout, rem: the first column's tallest state
 * with its list at two rows, as measured at 1920 × 1080 (R07.T19.b's hidden captures, 16 px to the
 * rem): `Instruments` 204 px with both closed, `Targets` 174 px at two rows under its two-line
 * `RANGE FROM CAMERA` head, `Camera` 339 px with `NO OWN SHIP`, both limit reasons and reduced
 * motion, `Style` 141 px with a two-line refusal (99 px without), and three 0.5 rem gaps: 882 px.
 * The ruling estimated about 52 rem (decision-r07-t19-layout, item 1a).
 */
export const FULL_MIN_HEIGHT_REM = 55.25;

/**
 * The layout of a `.view` box of `size`: `full` where it is at least {@link FULL_MIN_WIDTH_REM}
 * wide and {@link FULL_MIN_HEIGHT_REM} tall, as 1920 × 1080 at 100% is, and before it is measured;
 * `compact` otherwise, as 1280 × 720 and 1920 × 1080 at 125% and 150% are.
 *
 * @remarks
 * Reckoned in `rem`, so it follows the interface scale; it needs no hysteresis, since the box's
 * size does not depend on its layout.
 */
export function viewLayout(size: ElementSize | null): ViewLayout {
  if (size === null || !(size.remPx > 0)) {
    return "full";
  }
  return size.widthPx >= FULL_MIN_WIDTH_REM * size.remPx &&
    size.heightPx >= FULL_MIN_HEIGHT_REM * size.remPx
    ? "full"
    : "compact";
}

/** A side panel that folds in the compact layout. */
export type FoldPanel = "instruments" | "camera" | "style" | "exposure" | "meter";

/** The panel open when the display mounts, and again when the one open folds. */
export const DEFAULT_FOLD: FoldPanel = "camera";

/**
 * The panel open once `panel`'s disclosure button is pressed while `open` is: opening another folds
 * the one open, and folding it opens `CAMERA` again, so that exactly one is open.
 */
export function toggledFold(open: FoldPanel, panel: FoldPanel): FoldPanel {
  return panel === open ? DEFAULT_FOLD : panel;
}

/**
 * The row of disclosure buttons under the list, by the panels' own names, in the panels' order;
 * `EXPOSURE METER` stands only while its panel would, last, so that nothing moves.
 */
export const FOLD_BUTTONS: ReadonlyArray<{ readonly panel: FoldPanel; readonly label: string }> = [
  { panel: "camera", label: "CAMERA" },
  { panel: "style", label: "STYLE" },
  { panel: "exposure", label: "EXPOSURE" },
  { panel: "meter", label: "EXPOSURE METER" },
];

/**
 * How the side column's camera and style panels stand: their IDs, by which the disclosure buttons
 * control them, whether each is folded, and what stands between the list and them, the compact
 * layout's row of disclosure buttons and its standing lines.
 */
export interface SideFolds {
  readonly cameraId: string;
  readonly cameraHidden: boolean;
  readonly styleId: string;
  readonly styleHidden: boolean;
  /** The row of disclosure buttons and its standing lines, or `null` in the full layout. */
  readonly row: ReactNode;
}

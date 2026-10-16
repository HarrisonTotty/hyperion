/**
 * How the `VIEW` display is laid out (plan R07, R07.T19.b; decision-r07-t19-layout, item 1): two
 * layouts, chosen from the size of the `.view` box alone and never from what it shows, so that
 * nothing moves when the style, the instruments or a reason changes.
 */
import type { ReactNode } from "react";

import type { ElementSize } from "../../lib/useElementSize";

/**
 * `full`, the stage beside two side columns (`Instruments`, `Targets` and `Camera`; then the
 * `CONTROLS` view's `Style`, `Exposure` and `Exposure meter`, decision-r07-t19b-exposure-fit item
 * 2); or `compact`, one column whose `Camera`, `Style`, `Exposure` and `Exposure meter` panels fold
 * behind a row of disclosure buttons, one open at a time.
 */
export type ViewLayout = "full" | "compact";

/**
 * The least width of the `.view` box for the full layout, rem: a 51 rem stage (a 35 rem instrument
 * slot, three 0.5 rem insets and the 14.5 rem label block the primary has at 1280 × 720 with the
 * slots open), then the 26 rem and 20 rem columns, each after a 1 rem gap.
 *
 * @remarks
 * 99 rem, where it was 98.5 (R06.T13.j, as the desktop ruled): at 98.5 the primary's block was
 * 14 rem beside the slots, and the `STARS` limit's run with its field, `V 10.0 mag S CAM`, passed
 * its plate by 2.5 px. The boxes from 98.5 to 99 rem, about 8 px of window width, are compact, as
 * every narrower box already is. A stated limit: the room rule still opens a slot beside a 14 rem
 * primary block, so below the guide's least size, 1280 × 720, the limit's run may pass its plate.
 */
export const FULL_MIN_WIDTH_REM = 99;

/**
 * The least height of the `.view` box for the full layout, rem: the taller column's tallest state
 * with the list at two rows, rounded up to the next 0.25 rem, as measured at 1920 × 1080
 * (P05.T13's hidden runs, after R07.T19.d's, 16 px to the rem, the side column's line box 1.25).
 *
 * @remarks
 * The first column's tallest, 730.5 px: `Instruments` 202.5 px with both closed, `Targets` 173 px
 * at two rows under its two-line `RANGE FROM CAMERA` head, `Camera` 339 px with `NO OWN SHIP`, both
 * limit reasons and reduced motion, and two 0.5 rem gaps. The second's, 752 px: `Style` 99.5 px
 * with a two-line refusal added at 17.5 px a line and its 0.5 rem margin, 43 px (`QUALITY LOW`'s on
 * an instrument, or the software adapter's, which this machine cannot raise); the exposure
 * `INHIBITED · OPERATOR` with nothing metered and a refused entry, 425.5 px, its
 * `AUTO NOT AVAILABLE: NO LIT SIDE` (or `NO DARK SIDE`, or `STAR DISC ONLY`) on two lines and
 * `INHIBIT` held back beside its three-line reason, since P05.T13 tracks their states; the meter
 * with its status in the reading's place, 168 px; and two gaps. 752 px is 47 rem; it was 731.5 px
 * (45.75 rem) before P05.T13, the exposure then 365 px beside a two-line reason with the meter's
 * `METERED`, which that state now reaches at 749 px. It must never pass 52.5 rem, so that a
 * maximised 1920 × 1080 window (a box of about 53.5 rem) is full (decision-r07-t19b-exposure-fit,
 * item 2): a task that adds to a side panel re-measures, and asks for a ruling rather than pass it.
 */
export const FULL_MIN_HEIGHT_REM = 47;

/**
 * The bound on {@link FULL_MIN_HEIGHT_REM}, rem: a maximised 1920 × 1080 window's box, about
 * 53.5 rem under a title bar and a desktop panel, less 1 rem (decision-r07-t19b-exposure-fit,
 * item 2).
 */
export const FULL_MIN_HEIGHT_BOUND_REM = 52.5;

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
 * How the first column's camera panel stands: its ID, by which its disclosure button controls it,
 * whether it is folded, and what stands between the list and it, the compact layout's row of
 * disclosure buttons and its standing lines.
 */
export interface SideFolds {
  readonly cameraId: string;
  readonly cameraHidden: boolean;
  /** The row of disclosure buttons and its standing lines, or `null` in the full layout. */
  readonly row: ReactNode;
}

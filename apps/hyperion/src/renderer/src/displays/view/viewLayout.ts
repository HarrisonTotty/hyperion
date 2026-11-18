/**
 * How the `VIEW` display is laid out (plan R07, R07.T19.b; decision-r07-t19-layout, item 1): two
 * layouts, chosen from the size of the `.view` box alone and never from what it shows, so that
 * nothing moves when the style, the instruments or a reason changes.
 */
import type { ReactNode } from "react";

import type { ElementSize } from "../../lib/useElementSize";

/**
 * `full`, the stage beside two side columns (`Instruments`, `Targets` and `Camera`, with `Stills`
 * folding in `Camera`'s place behind a row of two disclosure buttons where it stands; then the
 * `CONTROLS` view's `Style`, `Exposure` and `Exposure meter`, decision-r07-t19b-exposure-fit item
 * 2, the last two folding behind a row of their own below {@link FULL_WHOLE_MIN_HEIGHT_REM}); or
 * `compact`, one column whose `Camera`, `Stills`, `Style`, `Exposure`, `White balance` and
 * `Exposure meter` panels fold behind a row of disclosure buttons, one open at a time.
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
 * with the list at two rows, the second column folded as it folds below
 * {@link FULL_WHOLE_MIN_HEIGHT_REM}, rounded up to the next 0.25 rem, as measured hidden and
 * offscreen (plan R11, R11.T10.b's follow-up, 2026-11-18: B612 as shipped, 16 px to the rem, the
 * side column's line box 1.25): the first column's, 813.5 px.
 *
 * @remarks
 * The first column's tallest, 730.5 px: `Instruments` 202.5 px with both closed, `Targets` 173 px
 * at two rows under its two-line `RANGE FROM CAMERA` head, `Camera` 339 px with `NO OWN SHIP`, both
 * limit reasons and reduced motion, and two 0.5 rem gaps. The second's, 792 px: `Style` 99.5 px
 * with a two-line refusal added at 17.5 px a line and its 0.5 rem margin, 43 px (the software
 * adapter's, which this machine cannot raise), and the `BASELINE` row, 32 px under its 0.5 rem
 * margin, 8 px; the exposure `INHIBITED · OPERATOR` with nothing metered and a refused entry,
 * 425.5 px, its `AUTO NOT AVAILABLE: NO LIT SIDE` (or `NO DARK SIDE`, or `STAR DISC ONLY`) on two
 * lines and `INHIBIT` held back beside its three-line reason, since P05.T13 tracks their states;
 * the meter with its status in the reading's place, 168 px; and two gaps. 792 px is 49.5 rem.
 * - **The `BASELINE` row** (R10.T12.a) stands for the `CONTROLS` view, the `PRIMARY` view or,
 *   since R10.T12.d, an instrument on its `GROUND` page or not, so that with an instrument there,
 *   whose `QUALITY LOW` refusal is the other two-line one, the column is 792 px too, 49.5 rem; it
 *   was 752 px there before R10.T12.d. The row is 17.42 rem wide against the panel's 18.375 rem of
 *   content (B612's shipped metrics, no kerning, so it errs wide); were its buttons ever to wrap
 *   under its legend, the column would be 824 px, 51.5 rem, still within the bound.
 * - **The `GRID` row** (the owner's request, 2026-10-31), 32 px under its 0.5 rem margin, stands
 *   only while the photorealistic style is asked for and offered, so never beside the style's
 *   reason (R10.T12.d's lean): the panel's tallest stays the reason with `BASELINE`, 182.5 px, the
 *   `GRID` row with `BASELINE` 179.5 px. Beside the reason it would have made the column 832 px.
 * - **The exposure's keys** (`7 ENABLE`, `8 INHIBIT`, the owner's request, 2026-11-01), each shown
 *   before its button's label, were re-measured hidden and offscreen at 1920 × 1080 (B612's shipped
 *   fonts, in a pod): no state of the second column grows, its tallest without the refused entry
 *   770.5 px before and after, so 792 px with it; in the compact layout's column a state grows by
 *   at most 3 px, its tallest unchanged at 615 px.
 * - **The `GROUND` page** (R10.T12.d) stands in the camera panel's place in the first column, which
 *   it never makes taller: its parts give way, with the list at its floor, before it would.
 * - **Before.** 752 px, 47 rem, before R10.T12.a; 731.5 px (45.75 rem) before P05.T13, the
 *   exposure then 365 px beside a two-line reason with the meter's `METERED`, which that state now
 *   reaches at 749 px.
 * - **The bound.** It must never pass 52.5 rem ({@link FULL_MIN_HEIGHT_BOUND_REM}), so that a
 *   maximised 1920 × 1080 window (a box of about 53.5 rem) is full (decision-r07-t19b-exposure-fit,
 *   item 2): a task that adds to a side panel re-measures, and asks for a ruling rather than pass
 *   it. The row held back with a reason, its draft form, would have passed it at 852.5 px with a
 *   two-line reason, and a panel of its own would have stood at it, 839.5 px.
 * - **`STILLS`** (plan R11, R11.T10.b; the UX ruling, 2026-11-18) shares the first column's last
 *   place with `Camera` behind a row of two disclosure buttons, so the place is the taller of the
 *   two. Laid out last in the second column, the panel would have taken it to 905 to 1,049 px.
 * - **Measured** (R11.T10.b's follow-up, 2026-11-18; the parts above are from earlier runs). The
 *   first column's tallest is 813.5 px: `Instruments` 202.5 px, `Targets` at two rows under its
 *   two-line head 173 px, the `CAMERA` `STILLS` row 32 px, the stills' two-line line 35 px and
 *   `Camera` at its worst 339 px, with four 0.5 rem gaps; at its 10 rem floor the list held less
 *   than two rows at 800.5 px. The second column whole is 899 px, so it folds below
 *   {@link FULL_WHOLE_MIN_HEIGHT_REM}, where its tallest is 763 px: `Style` 182.5 px, its row of
 *   `EXPOSURE` and `EXPOSURE METER` 32 px and the exposure at its worst 532.5 px, two gaps. The
 *   51 rem estimate holds.
 */
export const FULL_MIN_HEIGHT_REM = 51;

/**
 * The bound on {@link FULL_MIN_HEIGHT_REM}, rem: a maximised 1920 × 1080 window's box, about
 * 53.5 rem under a title bar and a desktop panel, less 1 rem (decision-r07-t19b-exposure-fit,
 * item 2).
 */
export const FULL_MIN_HEIGHT_BOUND_REM = 52.5;

/**
 * The least height of the `.view` box at which the full layout's second column stands whole, rem;
 * below it, from {@link FULL_MIN_HEIGHT_REM}, its `Exposure` and `Exposure meter` panels share the
 * column's last place behind a row of their two disclosure buttons, one open (plan R11,
 * R11.T10.b's follow-up; the UX ruling, 2026-11-18, after R15.T6's fold).
 *
 * @remarks
 * The second column's tallest state whole, rounded up to the next 0.25 rem, as measured hidden and
 * offscreen (2026-11-18; B612 as shipped, 16 px to the rem): 899 px, `Style` 182.5 px with an
 * instrument's two-line `QUALITY LOW` refusal and the `BASELINE` row, the exposure 532.5 px at
 * `INHIBITED · OPERATOR` with `LIT` weighing nothing and a refused entry, its `WHITE BALANCE`
 * 95 px under its 0.75 rem margin, the meter with its status in the reading's place 168 px, and two
 * 0.5 rem gaps. A 1920 × 1080 window's box, 919 px in the client (57.4 rem), stands whole,
 * unchanged; a maximised one's, 853 px, folds. Chosen from the box's size alone, as
 * {@link viewLayout} is, so that nothing moves when the style or a reason changes; a task that
 * would take the whole column past 57.25 rem asks for a ruling first.
 */
export const FULL_WHOLE_MIN_HEIGHT_REM = 56.25;

/**
 * Whether the full layout's second column folds its `Exposure` and `Exposure meter`, one open: in
 * the full layout of a `.view` box under {@link FULL_WHOLE_MIN_HEIGHT_REM} tall, never before the
 * box is measured.
 */
export function secondColumnFolds(size: ElementSize | null): boolean {
  if (size === null || !(size.remPx > 0) || viewLayout(size) !== "full") {
    return false;
  }
  return size.heightPx < FULL_WHOLE_MIN_HEIGHT_REM * size.remPx;
}

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

/**
 * A side panel that folds in the compact layout: `balance` is the exposure's `WHITE BALANCE`, a
 * part of its own there alone, which the full layout keeps in the exposure panel (the UX ruling,
 * 2026-11-18).
 */
export type FoldPanel =
  "instruments" | "camera" | "style" | "exposure" | "balance" | "meter" | "stills";

/** The panel open when the display mounts, and again when the one open folds. */
export const DEFAULT_FOLD: FoldPanel = "camera";

/** A panel of the full layout's second column that folds below {@link FULL_WHOLE_MIN_HEIGHT_REM}. */
export type SecondColumnFold = "exposure" | "meter";

/**
 * The full layout's second column's panel open each time it starts to fold, but the one holding the
 * focus, and again when the one open folds ({@link secondColumnFolds}).
 */
export const SECOND_COLUMN_FOLD: SecondColumnFold = "exposure";

/** Whether `panel` folds in the full layout's second column. */
export function inSecondColumnFold(panel: FoldPanel | null): panel is SecondColumnFold {
  return panel === "exposure" || panel === "meter";
}

/**
 * The panel open once `panel`'s disclosure button is pressed while `open` is: opening another folds
 * the one open, and folding it opens `CAMERA` again, so that exactly one is open.
 */
export function toggledFold(open: FoldPanel, panel: FoldPanel): FoldPanel {
  return panel === open ? DEFAULT_FOLD : panel;
}

/**
 * The second column's panel open once `panel`'s disclosure button is pressed while `open` is, as
 * {@link toggledFold} has it, `EXPOSURE` opening again when the open one folds.
 */
export function toggledSecondFold(
  open: SecondColumnFold,
  panel: SecondColumnFold,
): SecondColumnFold {
  return panel === open ? SECOND_COLUMN_FOLD : panel;
}

/**
 * The row of disclosure buttons under the list, by the panels' own names, in the panels' order:
 * `STILLS` (plan R11, R11.T10.c) after `CAMERA`, whose place it shares; `WHITE BALANCE` after
 * `EXPOSURE` and `EXPOSURE METER` last, which stand only beside a photorealistic image, the white
 * balance whenever the meter does, so that nothing moves (the UX ruling, 2026-11-18). In the full
 * layout the first column's row holds `CAMERA` and `STILLS` alone, where `STILLS` stands (plan R11,
 * R11.T10.b; the UX ruling, 2026-11-18), and the second column's, below
 * {@link FULL_WHOLE_MIN_HEIGHT_REM}, `EXPOSURE` and `EXPOSURE METER`.
 */
export const FOLD_BUTTONS: ReadonlyArray<{ readonly panel: FoldPanel; readonly label: string }> = [
  { panel: "camera", label: "CAMERA" },
  { panel: "stills", label: "STILLS" },
  { panel: "style", label: "STYLE" },
  { panel: "exposure", label: "EXPOSURE" },
  { panel: "balance", label: "WHITE BALANCE" },
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

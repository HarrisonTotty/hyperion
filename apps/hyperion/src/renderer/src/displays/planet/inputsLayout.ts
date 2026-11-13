import type { ElementSize } from "../../lib/useElementSize";

/**
 * How the `PLANET VIEWER`'s `INPUTS` column is laid out: `full`, its groups `STAR`, `ORBIT` and
 * `PLANET` standing open and `VOLATILES · RANK` and `SHOWN` folding on their own; or `compact`,
 * where the column is too short for that, with exactly one of the five open (R15.T6's review).
 */
export type InputsLayout = "full" | "compact";

/**
 * The column's height, in `rem`, below which it is laid out `compact`.
 *
 * @remarks
 * Above the full layout's tallest state, measured on the client, hidden, at 100% (R15.T6's fold,
 * 2026-11-05): `SHOWN` open with its field empty, the volatile ranks open, `SUN` `HELD`, an
 * entry's refusal, a server's refusal of three lines under its field and `GENERATE`'s outcome
 * naming that field, 934 px (58.4 rem). The column of a 1920 × 1080 window is 959 px (59.9 rem) and
 * stays `full`, unchanged; 1280 × 720's is 599 px (37.4 rem), where the compact layout's tallest
 * state measured 582 px. Reckoned in `rem`, so it follows the interface scale; it needs no
 * hysteresis, since the column's height does not depend on its layout (the work area holds it to
 * the window).
 */
export const FULL_MIN_HEIGHT_REM = 59;

/**
 * The layout of an `INPUTS` column of `size`: `compact` when it is shorter than
 * {@link FULL_MIN_HEIGHT_REM}, and `full` otherwise and before it is measured.
 */
export function inputsLayout(size: ElementSize | null): InputsLayout {
  if (size === null || !(size.remPx > 0)) {
    return "full";
  }
  return size.heightPx < FULL_MIN_HEIGHT_REM * size.remPx ? "compact" : "full";
}

/** A part of the `INPUTS` panel that folds in the compact layout, in the panel's order. */
export type InputsFold = "shown" | "star" | "orbit" | "planet" | "volatiles";

/** The part open as the display mounts, and again when the one open folds: `STAR`. */
export const DEFAULT_INPUTS_FOLD: InputsFold = "star";

/**
 * The part open once `part`'s disclosure is pressed while `open` is: opening another folds the
 * one open, and folding it opens `STAR` again, so that exactly one is open, as `VIEW`'s compact
 * column has it (`viewLayout.ts`'s `toggledFold`).
 */
export function toggledInputsFold(open: InputsFold, part: InputsFold): InputsFold {
  return part === open ? DEFAULT_INPUTS_FOLD : part;
}

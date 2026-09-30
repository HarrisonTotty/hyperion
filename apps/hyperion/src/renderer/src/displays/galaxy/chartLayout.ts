import type { ElementSize } from "../../lib/useElementSize";

/**
 * How the `LOCAL CHART` page is laid out: `stacked`, the chart over what it is asked for, the census
 * and the legend; or `compact`, where the page is too short for that, with what the chart is asked
 * for in a column beside it and the census line beside its frame and centre (the orchestrator's
 * ruling 134.9).
 */
export type ChartLayout = "stacked" | "compact";

/**
 * The page height, in `rem`, below which the chart is laid out `compact`.
 *
 * @remarks
 * Below about 44 rem the compact view is taller than the stacked one would be: stacked, a page of
 * 1600 × 900 (about 41 rem) or of 1920 × 1080 at 125% (about 39 rem) would leave the view some
 * 13–17 rem with the census table shown, where compact gives it 20 rem or more, and the fitted
 * sphere is limited by the view's shorter side (the orchestrator's ruling 149.2). The switch is made
 * on the page's height in `rem`, so it follows the interface scale; it needs no hysteresis, since
 * the page's height does not depend on its layout. 1920 × 1080 at 100% is 52.5 rem and stays
 * stacked, unchanged, as ruling 134.9 has it.
 */
export const COMPACT_BELOW_REM = 44;

/**
 * The layout of a `LOCAL CHART` page of `size`: `compact` when it is shorter than
 * {@link COMPACT_BELOW_REM}, and `stacked` otherwise and before it is measured.
 */
export function chartLayout(size: ElementSize | null): ChartLayout {
  if (size === null || !(size.remPx > 0)) {
    return "stacked";
  }
  return size.heightPx < COMPACT_BELOW_REM * size.remPx ? "compact" : "stacked";
}

/**
 * Whether a chart page's query controls give way to its census table: on a compact page, while the
 * table is shown, which it can only be beside an answer (the orchestrator's ruling 149.1).
 *
 * @remarks
 * Without an answer, as after another universe is opened, there is no table and no toggle to fold
 * it, so the controls come back whatever the table's state was.
 */
export function controlsGiveWay(
  layout: ChartLayout,
  tableShown: boolean,
  hasAnswer: boolean,
): boolean {
  return layout === "compact" && tableShown && hasAnswer;
}

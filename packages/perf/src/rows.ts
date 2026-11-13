/**
 * The budget's rows: the frame-time table's, the memory table's, and the map from each timed pass
 * to its row (plan R12, Design notes 4 and 6).
 */

/** A view's role in a run (R07 Design note 14), the first half of a {@link PassKey}. */
export type ViewRole = "primary" | "instrument" | "station";

/** Every {@link ViewRole}, for validators. */
export const VIEW_ROLES: ReadonlyArray<ViewRole> = ["primary", "instrument", "station"];

/**
 * Every row of the frame-time table, in the brainstorm's order (Performance budget, the frame-time
 * table), then the rows it has no estimate for.
 *
 * @remarks
 * `cockpit-instruments` is the wireframe instruments' share of the single-player frame, measured
 * only; `compositing` is the GPU process's busy time less the timed passes (Design note 6), the
 * consoles' cost, which the table leaves out; `other` is a timed pass of no row of the table, such
 * as a still.
 */
export const BUDGET_ROWS = [
  "terrain",
  "atmosphere-frame",
  "atmosphere-tables",
  "clouds",
  "ocean",
  "shadows",
  "stars",
  "exposure-histogram",
  "bloom-tonemap",
  "scatter",
  "rings",
  "station-wireframe",
  "cockpit-instruments",
  "compositing",
  "other",
] as const;

/** A row of the frame-time table. */
export type BudgetRow = (typeof BUDGET_ROWS)[number];

/** Whether a string names a {@link BudgetRow}. */
export function isBudgetRow(value: string): value is BudgetRow {
  return BUDGET_ROWS.some((row) => row === value);
}

/**
 * A timed pass as the table keys it: the view's role and the pass's `FrameSubmission.label`, so
 * that the same wireframe passes land in `cockpit-instruments` on an instrument and in
 * `station-wireframe` on a station (Design note 4).
 */
export type PassKey = `${ViewRole}:${string}`;

/** Whether a string is a {@link PassKey}: a role, a colon and a label that is not empty. */
export function isPassKey(value: string): value is PassKey {
  const colon = value.indexOf(":");
  if (colon < 0 || colon === value.length - 1) return false;
  const role = value.slice(0, colon);
  return VIEW_ROLES.some((known) => known === role);
}

/**
 * Every pass the harness times, by role and label, to its row (Design note 4).
 *
 * @remarks
 * Empty until R12.T0's list of timed pass labels (the plan's records, R12.T0) is entered here by
 * R12.T4.c, whose Vitest fails on a pass that the renderer times without a row. A record stores
 * each pass's row beside its key, so that a record stays readable when this map changes.
 */
export const PASS_ROWS: Readonly<Record<PassKey, BudgetRow>> = {};

/**
 * Every row of the memory table, in the brainstorm's order (Performance budget, the memory table),
 * then the lines it has no estimate for.
 *
 * @remarks
 * `atmosphere-view` is R08's per-view tables, about 0.43 MB a view on high (R08 Design note 11),
 * which the brainstorm counts within its atmosphere row. `ocean`, `rings`, `scatter`, `stills` and
 * `other` are measured only.
 */
export const MEMORY_ITEMS_ORDER = [
  "star-cubemap",
  "height-cache",
  "atmosphere-tables",
  "atmosphere-view",
  "coarse-field-gpu",
  "render-targets",
  "shadows",
  "clouds",
  "ocean",
  "rings",
  "scatter",
  "stills",
  "other",
] as const;

/** A row of the memory table: the reporting line a `MemoryCategory` is counted in. */
export type MemoryItem = (typeof MEMORY_ITEMS_ORDER)[number];

/** Whether a string names a {@link MemoryItem}. */
export function isMemoryItem(value: string): value is MemoryItem {
  return MEMORY_ITEMS_ORDER.some((item) => item === value);
}

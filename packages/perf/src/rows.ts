/**
 * The budget's rows: the frame-time table's, the memory table's, and the map from each timed pass
 * to its row (plan R12, Design notes 4 and 6).
 */

/**
 * Every role a view has in a run (R07 Design note 14): the client's `primary` and `instrument`
 * slots, and `station`, a station machine's wireframe primary.
 */
export const VIEW_ROLES = ["primary", "instrument", "station"] as const;

/** A view's role in a run, the first half of a {@link PassKey}. */
export type ViewRole = (typeof VIEW_ROLES)[number];

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
 * The passes both styles submit (R12.T0's record): the terrain's and the bodies' cluster culls and
 * the terrain's compaction (plan R14), and R06's star bake with its splat, at a bake.
 */
const SHARED_PASSES = [
  "terrain cluster cull",
  "terrain cluster compact",
  "body cluster cull",
  "sky bake",
  "sky:bakeSplat",
] as const;

/** The passes only a wireframe view submits: R02's draw list and plan R14's two culls. */
const WIREFRAME_PASSES = ["view:wireframe", "hull cluster cull", "mesh occluder cull"] as const;

/** A wireframe view's passes, the shared ones among them, all to `row`. */
function wireframeRows(
  role: "instrument" | "station",
  row: "cockpit-instruments" | "station-wireframe",
): Readonly<Record<PassKey, BudgetRow>> {
  const rows: Record<PassKey, BudgetRow> = {};
  for (const label of [...WIREFRAME_PASSES, ...SHARED_PASSES]) {
    rows[`${role}:${label}`] = row;
  }
  return rows;
}

/**
 * Every pass the harness times, by role and label, to its row (Design note 4): R12.T0's list of
 * timed pass labels (the plan's records, R12.T0) and R11's that have landed, R11.T2's `decoration`,
 * R11.T7's `clouds` (R12.T4.c), with the volume's march and resolve, `clouds volume` and
 * `clouds resolve` (R11.T7.c), R11.T8.b's `ocean` and R11.T8.c's `ocean copies`, R11.T9.c's
 * `rings` and R11.T9.d's ring particles (their placement, cull and draws).
 *
 * @remarks
 * A photorealistic view is a `primary`: R06's sky and its bake are the star field's row; R07's disc
 * cells, discs and symbology `other`, with the culls of the hulls and mesh occluders its symbology
 * draws through R02's wireframe list (`VIEW`'s own ship, R12.T4.c); R10's cascades `shadows`; the bodies' passes, the terrain's
 * and plan R14's visibility buffer, pyramid and culls, and R11's detail sets, `terrain`; R08's
 * tables, its per-frame passes, R07's histogram and its bloom and tone mapping their own rows;
 * R05's own `spike display`, the test planet's, `other`. A wireframe view's passes, the shared ones
 * among them, are `cockpit-instruments` on an `instrument` and `station-wireframe` on a `station`.
 * The renderer's `passRowOf` (`view/perf/passRows.ts`) reads this map, and keys the terrain's
 * cluster counters, `terrain clusters (<view ID>) …`, which carry the view's ID, by their prefix;
 * its Vitest fails on a pass that the renderer times without a row. A record stores each pass's row
 * beside its key, so that a record stays readable when this map changes.
 */
export const PASS_ROWS: Readonly<Record<PassKey, BudgetRow>> = {
  "primary:sky": "stars",
  "primary:sky bake": "stars",
  "primary:sky:bakeSplat": "stars",
  "primary:disc cells": "other",
  "primary:discs": "other",
  "primary:symbology": "other",
  "primary:hull cluster cull": "other",
  "primary:mesh occluder cull": "other",
  "primary:spike display": "other",
  "primary:terrain shadows": "shadows",
  "primary:bodies": "terrain",
  "primary:bodies late": "terrain",
  "primary:terrain": "terrain",
  "primary:figure id raster": "terrain",
  "primary:figure resolve": "terrain",
  "primary:terrain id raster": "terrain",
  "primary:terrain id raster late": "terrain",
  "primary:terrain resolve": "terrain",
  "primary:depth pyramid": "terrain",
  "primary:terrain cluster cull": "terrain",
  "primary:terrain cluster compact": "terrain",
  "primary:body cluster cull": "terrain",
  "primary:decoration": "terrain",
  "primary:atmosphere tables": "atmosphere-tables",
  "primary:atmosphere view": "atmosphere-frame",
  "primary:atmosphere behind": "atmosphere-frame",
  "primary:atmosphere composite": "atmosphere-frame",
  "primary:histogram": "exposure-histogram",
  "primary:bloom": "bloom-tonemap",
  "primary:tonemap": "bloom-tonemap",
  "primary:clouds": "clouds",
  "primary:clouds volume": "clouds",
  "primary:clouds resolve": "clouds",
  "primary:ocean": "ocean",
  "primary:ocean copies": "ocean",
  "primary:rings": "rings",
  "primary:ring particles": "rings",
  "primary:ring particle place": "rings",
  "primary:ring particle cull": "rings",
  ...wireframeRows("instrument", "cockpit-instruments"),
  ...wireframeRows("station", "station-wireframe"),
};

/**
 * The rows of the passes whose labels carry a view's ID, by the label's prefix and the view's role
 * (R12.T0's record): the terrain's cluster counters, `terrain clusters (<view ID>) stats` and
 * `… over counts` (plan R14), each in the terrain's row on a photorealistic view and in the
 * wireframe's on the others.
 */
export const PASS_ROW_PREFIXES: Readonly<Record<string, Readonly<Record<ViewRole, BudgetRow>>>> = {
  "terrain clusters (": {
    primary: "terrain",
    instrument: "cockpit-instruments",
    station: "station-wireframe",
  },
};

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

/**
 * Every R01 `MemoryCategory` (`apps/hyperion`'s `view/engine/memory.ts`), by its owning plan's
 * name, to the memory row it is reported in (Design note 6; R12.T3.a).
 *
 * @remarks
 * Its keys are the renderer's union exactly: `view/perf/ledger.test.ts`'s type-level test fails on
 * a category without an entry, or an entry of no category. The five names this plan asked for map
 * to their own rows (`height-cache`, `atmosphere-tables`, `atmosphere-view`, `coarse-field-gpu`
 * and `shadows`), as do R01's, R06's cube and R11's that have rows. R06's bake transients
 * (`sky-scratch`), R11's detail sets (`decoration`) and R14's cluster records and mesh pages
 * (`geometry-clusters`, `geometry-pages`) have no row in the brainstorm's table, and are counted
 * in `other` (R12.T0's lean, kept by T3.a, 2026-11-17).
 */
export const MEMORY_ITEMS = {
  "render-targets": "render-targets",
  other: "other",
  "atmosphere-tables": "atmosphere-tables",
  "atmosphere-view": "atmosphere-view",
  clouds: "clouds",
  "coarse-field-gpu": "coarse-field-gpu",
  decoration: "other",
  "geometry-clusters": "other",
  "geometry-pages": "other",
  "height-cache": "height-cache",
  ocean: "ocean",
  rings: "rings",
  scatter: "scatter",
  shadows: "shadows",
  "sky-cube": "star-cubemap",
  "sky-scratch": "other",
} as const satisfies Readonly<Record<string, MemoryItem>>;

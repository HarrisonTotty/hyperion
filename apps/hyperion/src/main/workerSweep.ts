/**
 * The height-worker sweep's table (plan R10, R10.T14.b's "Bakes under load"): one descent spike's
 * `--sizes on` results file a worker count, each flown with one configuration and only its
 * `--workers` changed, read for the bake clock's whole-flight figures (decision-r10-t14b-measure
 * 6(c)): bakes a second, post to reply at p50 and p95, and the share of the bakes never made
 * resident.
 *
 * @remarks
 * `scripts/workerSweep.mjs` flies the runs and writes the table. A sweep whose runs differ in any
 * other option, setting, seed, world or style is refused, so that the worker count is the one
 * thing it varies. `defaultHeightWorkers` (`workers/pool.ts`) is R10.T6.a's lean, eight workers at
 * the target's 16 threads, until such a table there confirms or lowers it.
 */

import type { SpikeSizesBakeWindow } from "../preload/api";
import type { DescentResults } from "./results";

/** What the sweep reads of a run's results file. */
export interface SweptRun {
  readonly run: Pick<DescentResults["run"], "setting" | "seed" | "world" | "style" | "options">;
  /** The `--sizes on` block, whose bake clock the sweep reads; absent from an unmeasured run. */
  readonly sizes?: { readonly bakes: { readonly total: SpikeSizesBakeWindow } };
}

/** One worker count's whole-flight bake figures. */
export interface SweepRow {
  readonly workers: number;
  readonly bakesPerSecond: number | null;
  readonly postToReplyP50Ms: number | null;
  readonly postToReplyP95Ms: number | null;
  readonly baked: number;
  /** Baked less made resident (6(c)). */
  readonly neverResident: number;
  /** {@link SweepRow.neverResident} ÷ {@link SweepRow.baked}, or `null` with nothing baked. */
  readonly neverResidentShare: number | null;
}

/** The options a sweep's runs may differ in: the swept count, and where each run writes. */
const SWEPT_OPTIONS: ReadonlySet<string> = new Set(["workers", "out"]);

/** A run's configuration: everything but {@link SWEPT_OPTIONS}, as one comparable string. */
function configurationOf(run: SweptRun["run"]): string {
  const options = Object.entries(run.options)
    .filter(([name]) => !SWEPT_OPTIONS.has(name))
    .toSorted(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  return JSON.stringify({
    setting: run.setting,
    seed: run.seed,
    world: run.world,
    style: run.style,
    options,
  });
}

/** The sweep's rows, or why its runs are not one sweep. */
export type Sweep =
  | { readonly ok: true; readonly rows: ReadonlyArray<SweepRow> }
  | { readonly ok: false; readonly problem: string };

/**
 * The sweep's rows, in rising worker count.
 *
 * @returns The rows; or a problem where no run is given, a run holds no worker count or no
 *   `sizes` block, two runs have one worker count, or two runs' configurations differ in anything
 *   but `--workers` and `--out`.
 */
export function sweepOf(runs: ReadonlyArray<SweptRun>): Sweep {
  const [first] = runs;
  if (first === undefined) {
    return { ok: false, problem: "a worker sweep needs one run at least" };
  }
  const configuration = configurationOf(first.run);
  const rows: SweepRow[] = [];
  for (const swept of runs) {
    const workers = swept.run.options["workers"];
    if (typeof workers !== "number") {
      return { ok: false, problem: "a swept run records its --workers" };
    }
    if (configurationOf(swept.run) !== configuration) {
      return {
        ok: false,
        problem:
          `the run at ${String(workers)} workers was flown with another configuration than the ` +
          "first: a sweep varies --workers alone",
      };
    }
    if (swept.sizes === undefined) {
      return {
        ok: false,
        problem: `the run at ${String(workers)} workers holds no sizes block: fly --sizes on`,
      };
    }
    if (rows.some((row) => row.workers === workers)) {
      return { ok: false, problem: `two runs of the sweep have ${String(workers)} workers` };
    }
    const { total } = swept.sizes.bakes;
    rows.push({
      workers,
      bakesPerSecond: total.bakesPerSecond,
      postToReplyP50Ms: total.postToReplyMs.p50,
      postToReplyP95Ms: total.postToReplyMs.p95,
      baked: total.baked,
      neverResident: total.bakedNeverResident,
      neverResidentShare: total.baked > 0 ? total.bakedNeverResident / total.baked : null,
    });
  }
  return { ok: true, rows: rows.toSorted((a, b) => a.workers - b.workers) };
}

/** A figure to `digits` decimals, or `—` where none was measured. */
function figure(value: number | null, digits: number): string {
  return value === null ? "—" : value.toFixed(digits);
}

/**
 * The sweep's table in Markdown: a heading line naming `machine` (as "pod, N vCPU" or the
 * target's name), then one row a worker count.
 */
export function sweepTableLines(rows: ReadonlyArray<SweepRow>, machine: string): string[] {
  return [
    `Height-worker sweep, ${machine}: the bake clock over the whole flight.`,
    "",
    "| Workers | Bakes a second | Post to reply p50 | p95 | Baked | Never resident |",
    "| ------- | -------------- | ----------------- | --- | ----- | -------------- |",
    ...rows.map(
      (row) =>
        `| ${String(row.workers)} | ${figure(row.bakesPerSecond, 1)} | ` +
        `${figure(row.postToReplyP50Ms, 0)} ms | ${figure(row.postToReplyP95Ms, 0)} ms | ` +
        `${String(row.baked)} | ${String(row.neverResident)} ` +
        `(${row.neverResidentShare === null ? "—" : `${(100 * row.neverResidentShare).toFixed(1)}%`}) |`,
    ),
  ];
}

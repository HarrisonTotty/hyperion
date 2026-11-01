import { describe, expect, it } from "vitest";

import type { SpikeSizesBakeWindow } from "../preload/api";
import { type SweptRun, sweepOf, sweepTableLines } from "./workerSweep";

/** A run's whole-flight bake clock: `baked` bakes, `never` of them never resident. */
function total(baked: number, never: number, p50: number): SpikeSizesBakeWindow {
  return {
    posted: baked + 2,
    baked,
    failed: 0,
    unsurveyed: 0,
    postToReplyMs: { p50, p95: 3 * p50, max: 5 * p50 },
    computeMs: { p50: 80, p95: 160, max: 400 },
    residents: baked - never,
    bakedNeverResident: never,
    spanMs: 1_000_000,
    bakesPerSecond: baked / 1_000,
  };
}

/** A served high run at `workers` workers, its options as a `--sizes on` launch records them. */
function runAt(workers: number, options: Readonly<Record<string, string | number>> = {}): SweptRun {
  return {
    run: {
      setting: "high",
      seed: "7",
      world: "served",
      style: "photorealistic",
      options: {
        setting: "high",
        seed: "7",
        world: "served",
        sizes: "on",
        universe: "d4805486584ef791",
        body: "41ffecb200000002.0400",
        workers,
        out: `target/sweep/workers-${String(workers)}`,
        ...options,
      },
    },
    sizes: { bakes: { total: total(1_000 * workers, 50 * workers, 900 / workers) } },
  };
}

describe("the height-worker sweep", () => {
  it("reads one row a worker count from runs flown with one configuration", () => {
    const sweep = sweepOf([runAt(6), runAt(2), runAt(4), runAt(3)]);
    if (!sweep.ok) {
      throw new Error(sweep.problem);
    }
    expect(sweep.rows.map((row) => row.workers)).toEqual([2, 3, 4, 6]);
    expect(sweep.rows[0]).toEqual({
      workers: 2,
      bakesPerSecond: 2,
      postToReplyP50Ms: 450,
      postToReplyP95Ms: 1_350,
      baked: 2_000,
      neverResident: 100,
      neverResidentShare: 0.05,
    });
  });

  it("refuses runs whose configurations differ in more than the worker count", () => {
    const sweep = sweepOf([runAt(2), runAt(3, { setting: "low" })]);
    expect(sweep).toEqual({
      ok: false,
      problem:
        "the run at 3 workers was flown with another configuration than the first: a sweep " +
        "varies --workers alone",
    });
  });

  it("refuses a run with no sizes block, two runs at one count, and an empty sweep", () => {
    const { sizes: _, ...unmeasured } = runAt(3);
    expect(sweepOf([runAt(2), unmeasured])).toEqual({
      ok: false,
      problem: "the run at 3 workers holds no sizes block: fly --sizes on",
    });
    expect(sweepOf([runAt(2), runAt(2)])).toEqual({
      ok: false,
      problem: "two runs of the sweep have 2 workers",
    });
    expect(sweepOf([])).toEqual({ ok: false, problem: "a worker sweep needs one run at least" });
  });

  it("writes a row a worker count, a missing figure as a dash", () => {
    const sweep = sweepOf([runAt(2)]);
    if (!sweep.ok) {
      throw new Error(sweep.problem);
    }
    const [row] = sweep.rows;
    if (row === undefined) {
      throw new Error("the sweep has a row");
    }
    const lines = sweepTableLines(
      [row, { ...row, workers: 3, bakesPerSecond: null }],
      "pod, 16 vCPU",
    );
    expect(lines[0]).toBe(
      "Height-worker sweep, pod, 16 vCPU: the bake clock over the whole flight.",
    );
    expect(lines.slice(4)).toEqual([
      "| 2 | 2.0 | 450 ms | 1350 ms | 2000 | 100 (5.0%) |",
      "| 3 | — | 450 ms | 1350 ms | 2000 | 100 (5.0%) |",
    ]);
  });
});

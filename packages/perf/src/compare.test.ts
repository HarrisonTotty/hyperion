import { describe, expect, it } from "vitest";

import {
  compareMetric,
  compareRecords,
  incomparability,
  perfCompare,
  previousComparable,
} from "./compare.ts";
import type { PerfRunRecord } from "./record.ts";
import { serialisePerfRecord } from "./record.ts";
import { metricsOf, sampleRecord } from "./test/records.ts";

const MAIN = [8, 8.1, 7.9, 8, 8.05];
const MISSED = [0.01, 0.012, 0.011, 0.01, 0.009];

/** A record whose five GPU busy times are given, the other metrics steady. */
function withGpu(
  gpu: ReadonlyArray<number>,
  n = 1,
  replace: Partial<PerfRunRecord> = {},
): PerfRunRecord {
  return sampleRecord(metricsOf(gpu, MAIN, MISSED), {
    id: `2026-11-13-uhd620-cockpit-descent-${String(n)}`,
    ...replace,
  });
}

function withMissed(missed: ReadonlyArray<number>, n = 1): PerfRunRecord {
  return sampleRecord(metricsOf([20, 20, 20, 20, 20], MAIN, missed), {
    id: `2026-11-13-uhd620-cockpit-descent-${String(n)}`,
  });
}

const OLD = [20, 20.2, 19.9, 20.1, 20];

describe("the comparison's verdict", () => {
  it("is REGRESSION on complete separation with the ratio beyond 1 + ε", () => {
    const c = compareMetric(
      withGpu(OLD),
      withGpu([21, 21.2, 20.9, 21.1, 21], 2),
      "gpuBusyPerFrameMs",
    );
    expect(c).toMatchObject({ separation: "worse", verdict: "REGRESSION" });
    expect(c.ratio).toBeCloseTo(1.05, 10);
    expect(c.p).toBeCloseTo(2 / 252, 12);
  });

  it("is BETTER on the mirror", () => {
    const c = compareMetric(
      withGpu(OLD),
      withGpu([19, 19.2, 18.9, 19.1, 19], 2),
      "gpuBusyPerFrameMs",
    );
    expect(c).toMatchObject({ separation: "better", verdict: "BETTER" });
    expect(c.ratio).toBeCloseTo(0.95, 10);
  });

  it("is SAME on complete separation within ε", () => {
    const c = compareMetric(
      withGpu(OLD),
      withGpu([20.3, 20.4, 20.35, 20.5, 20.45], 2),
      "gpuBusyPerFrameMs",
    );
    expect(c.separation).toBe("worse");
    expect(c.ratio).toBeLessThan(1.03);
    expect(c.verdict).toBe("SAME");
  });

  it("is SAME on overlapping values, however far apart the medians", () => {
    const c = compareMetric(withGpu(OLD), withGpu([25, 25, 25, 25, 20.1], 2), "gpuBusyPerFrameMs");
    expect(c).toMatchObject({ separation: "none", verdict: "SAME" });
    expect(c.ratio).toBeCloseTo(1.25, 10);
  });

  it("judges the missed-frame fraction by its absolute ε of one point", () => {
    const old = withMissed([0.01, 0.012, 0.011, 0.01, 0.009]);
    const within = compareMetric(
      old,
      withMissed([0.018, 0.017, 0.0175, 0.018, 0.019], 2),
      "missedFraction",
    );
    expect(within).toMatchObject({ separation: "worse", ratio: null, verdict: "SAME" });
    expect(within.difference).toBeCloseTo(0.008, 10);
    const beyond = compareMetric(
      old,
      withMissed([0.03, 0.031, 0.029, 0.03, 0.032], 2),
      "missedFraction",
    );
    expect(beyond.verdict).toBe("REGRESSION");
    const better = compareMetric(
      withMissed([0.05, 0.051, 0.049, 0.05, 0.052]),
      withMissed(MISSED, 2),
      "missedFraction",
    );
    expect(better.verdict).toBe("BETTER");
  });

  it("states nothing from fewer repetitions than complete separation can stand on", () => {
    const one = sampleRecord(metricsOf([20], [8], [0.01]));
    const later = sampleRecord(metricsOf([30], [8], [0.01]), {
      id: "2026-11-13-uhd620-cockpit-descent-2",
    });
    const c = compareMetric(one, later, "gpuBusyPerFrameMs");
    expect(c).toMatchObject({ separation: "worse", p: 1, verdict: "SAME" });
  });

  it("states nothing where a side lacks the metric", () => {
    const missing = sampleRecord(
      OLD.map((_, i) => ({
        gpuBusyPerFrameMs: null,
        mainThreadPerFrameMs: MAIN[i] ?? null,
        missedFraction: MISSED[i] ?? null,
      })),
    );
    expect(compareMetric(missing, withGpu(OLD, 2), "gpuBusyPerFrameMs")).toMatchObject({
      old: null,
      verdict: null,
    });
  });
});

describe("what compares", () => {
  const base = withGpu(OLD);

  it("refuses a different machine, naming the field", () => {
    const other = withGpu(OLD, 2, {
      machine: { ...base.machine, cpu: { ...base.machine.cpu, logicalCores: 4 } },
    });
    expect(incomparability(base, other)?.field).toBe("machine.cpu.logicalCores");
    const gpu = withGpu(OLD, 2, {
      machine: {
        ...base.machine,
        gpu: { vendorId: 0x10de, deviceId: 0x2206, driverVersion: null, description: null },
      },
    });
    expect(incomparability(base, gpu)?.field).toBe("machine.gpu.vendorId");
    expect(() => compareRecords(base, gpu)).toThrow(/machine\.gpu\.vendorId/);
  });

  it("refuses a different power profile, naming the field", () => {
    const other = withGpu(OLD, 2, {
      state: { ...base.state, profile: { ...base.state.profile, governor: "performance" } },
    });
    expect(incomparability(base, other)).toEqual({
      field: "state.profile.governor",
      old: '"powersave"',
      new: '"performance"',
    });
  });

  it("refuses another scene version or setting", () => {
    expect(
      incomparability(base, withGpu(OLD, 2, { scene: { ...base.scene, version: 2 } }))?.field,
    ).toBe("scene.version");
    expect(
      incomparability(base, withGpu(OLD, 2, { setting: { ...base.setting, quality: "high" } }))
        ?.field,
    ).toBe("setting.quality");
  });

  it("compares a change of the setting's values and prints it", () => {
    const after = withGpu(OLD, 2, {
      setting: {
        quality: "low",
        values: { ...base.setting.values, "quality.scatterDensity": 0.5 },
      },
      commit: "89abcdef0123456789abcdef0123456789abcdef",
    });
    const comparison = compareRecords(base, after);
    expect(comparison.changes.map((c) => c.field)).toEqual([
      "commit",
      "setting.values.quality.scatterDensity",
    ]);
  });

  it("finds the last comparable record before the one named, else refuses naming the field", () => {
    const first = withGpu(OLD, 1);
    const otherMachine = withGpu(OLD, 2, { state: { ...base.state, supply: "battery" } });
    const third = withGpu([21, 21.2, 20.9, 21.1, 21], 3);
    const records = [first, otherMachine, third];
    expect(previousComparable(records, third)).toMatchObject({
      kind: "comparable",
      old: { id: first.id },
    });
    expect(previousComparable(records, otherMachine)).toMatchObject({
      kind: "refused",
      nearest: { id: first.id },
      differs: { field: "state.supply" },
    });
    expect(previousComparable(records, first)).toEqual({ kind: "none" });
  });
});

describe("perf-compare", () => {
  const first = withGpu(OLD, 1);
  const regressed = withGpu([21, 21.2, 20.9, 21.1, 21], 2, { notes: "after" });
  const store = `${serialisePerfRecord(first)}\n${serialisePerfRecord(regressed)}\n`;

  it("exits 0 on a regression and prints the medians, ranges, effect and verdict", () => {
    const outcome = perfCompare(store, regressed.id, null);
    expect(outcome.exitCode).toBe(0);
    expect(outcome.text).toContain(`${regressed.id} against ${first.id}`);
    expect(outcome.text).toMatch(
      /GPU busy a frame, ms +20\.000 \[19\.900, 20\.200\] +21\.000 \[20\.900, 21\.200\] +ratio 1\.050 +0\.0079 +REGRESSION/,
    );
    expect(outcome.text).toContain("For reading, without a verdict:");
  });

  it("exits 0 for a scene's first record", () => {
    expect(perfCompare(store, first.id, null)).toMatchObject({ exitCode: 0 });
  });

  it("exits 1 for a refusal, naming the field", () => {
    const battery = withGpu(OLD, 3, { state: { ...first.state, supply: "battery" } });
    const refused = perfCompare(`${store}${serialisePerfRecord(battery)}\n`, battery.id, null);
    expect(refused.exitCode).toBe(1);
    expect(refused.text).toContain("differs in state.supply");
  });

  it("exits 1 for a record not in the store", () => {
    expect(perfCompare(store, "2026-11-13-uhd620-cockpit-descent-9", null).exitCode).toBe(1);
  });

  it("compares against the record named", () => {
    expect(perfCompare(store, regressed.id, first.id).text).toContain(
      `${regressed.id} against ${first.id}`,
    );
  });
});

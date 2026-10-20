import { describe, expect, it } from "vitest";

import { between, seededRandom } from "../../test/seededRandom";
import { QUALITY_SETTINGS, type QualitySetting } from "../quality/qualitySetting";
import {
  type Batch,
  BatchController,
  batchCapNs,
  type BatchMeasurement,
  EMPTY_SUBMIT_SAMPLES,
  type MeasuredPass,
  medianNs,
  passTimeBoundNs,
  TIMESTAMP_QUANTUM_NS,
  timestampBudgetNs,
} from "./batchController";

/** The quantum q, ns, as R01 Design note 4 found it. */
const Q = 65_536;

/** Frame intervals, ns: 59.94 Hz (the projector), 144 Hz and 29.97 Hz. */
const FRAME_INTERVALS_NS = [16_683_000, 6_944_000, 33_367_000] as const;

describe("the batch cap", () => {
  it.each(FRAME_INTERVALS_NS)("is 6 ms on low, at a frame interval of %i ns", (frameNs) => {
    expect(batchCapNs("low", frameNs)).toBe(6_000_000);
  });

  it.each(FRAME_INTERVALS_NS)("is a quarter of %i ns on high", (frameNs) => {
    expect(batchCapNs("high", frameNs)).toBe(frameNs / 4);
  });

  it("is 4.171 ms on high at 59.94 Hz", () => {
    expect(batchCapNs("high", 16_683_000)).toBe(4_170_750);
  });

  it("leaves two quanta with timestamps: 5.869 ms on low, 4.040 ms on high at 59.94 Hz", () => {
    expect([timestampBudgetNs("low", 16_683_000), timestampBudgetNs("high", 16_683_000)]).toEqual([
      5_868_928, 4_039_678,
    ]);
  });

  it.each(
    QUALITY_SETTINGS.flatMap((setting) =>
      [0, -16_683_000, Number.NaN, Number.POSITIVE_INFINITY].map((frameNs) => ({
        setting,
        frameNs,
      })),
    ),
  )("refuses on $setting a frame interval of $frameNs ns", ({ setting, frameNs }) => {
    expect(() => batchCapNs(setting, frameNs)).toThrow(RangeError);
  });
});

describe("a pass time's bound", () => {
  it("is the quantum R01 found", () => {
    expect(TIMESTAMP_QUANTUM_NS).toBe(Q);
  });

  it("holds every quantised pass's true time", () => {
    const random = seededRandom(17);
    const under: string[] = [];
    for (let k = 0; k < 10_000; k += 1) {
      const trueNs = Math.floor(between(random, 0, 3_000_000));
      const startNs = Math.floor(between(random, 0, 1e12));
      const measuredNs = Q * (Math.floor((startNs + trueNs) / Q) - Math.floor(startNs / Q));
      if (!(passTimeBoundNs(measuredNs, "quantized") > trueNs)) {
        under.push(`${trueNs} ns measured ${measuredNs} ns`);
      }
    }
    expect(under).toEqual([]);
  });

  it("is the measured time under a full timer", () => {
    expect(passTimeBoundNs(123_456, "full")).toBe(123_456);
  });

  it.each([-1, Number.NaN])("refuses a pass time of %f ns", (ns) => {
    expect(() => passTimeBoundNs(ns, "quantized")).toThrow(RangeError);
  });
});

describe("the median", () => {
  it("of five round trips is the middle one, in any order", () => {
    expect(medianNs([3_400_000, 2_900_000, 3_000_000, 9_000_000, 2_800_000])).toBe(3_000_000);
  });

  it("of an even count is the mean of the middle two", () => {
    expect(medianNs([4, 1, 3, 2])).toBe(2.5);
  });

  it.each([[[]], [[1, Number.NaN, 3]]])("refuses %j", (samples) => {
    expect(() => medianNs(samples)).toThrow(RangeError);
  });
});

/** A job's units: `tiles` × `samples` × `passes` of them, each labelled by its tile and pass. */
function jobUnits(tiles: number, samples: number, passes: number): string[] {
  const units: string[] = [];
  for (let t = 0; t < tiles; t += 1) {
    for (let s = 0; s < samples; s += 1) {
      for (let p = 0; p < passes; p += 1) {
        units.push(`t${t}/p${p}`);
      }
    }
  }
  return units;
}

/** Each label's true GPU time on the fake, ns: from 20 µs to 2.5 ms. */
function passCosts(units: ReadonlyArray<string>, seed: number): ReadonlyMap<string, number> {
  const random = seededRandom(seed);
  const costs = new Map<string, number>();
  for (const label of units) {
    if (!costs.has(label)) {
      costs.set(label, Math.floor(between(random, 20_000, 2_500_000)));
    }
  }
  return costs;
}

function costOf(costs: ReadonlyMap<string, number>, label: string): number {
  const ns = costs.get(label);
  if (ns === undefined) {
    throw new Error(`the fake has no cost for ${label}`);
  }
  return ns;
}

interface TimedRun {
  readonly batches: ReadonlyArray<Batch>;
  /** Each batch's true GPU time on the fake, ns. */
  readonly trueNs: ReadonlyArray<number>;
}

/**
 * Runs a job on a fake GPU with timestamps: each batch's pass times resolve two calls after it is
 * proposed, quantised under `quantized` from a random start.
 */
function runTimed(
  setting: QualitySetting,
  frameIntervalNs: number,
  timer: "quantized" | "full",
  maxUnits?: number,
): TimedRun {
  const units = jobUnits(4, 6, 8);
  const costs = passCosts(units, 5);
  const random = seededRandom(11);
  const controller = new BatchController({
    setting,
    frameIntervalNs,
    timing: { timer },
    units,
    ...(maxUnits === undefined ? {} : { maxUnits }),
  });
  const batches: Batch[] = [];
  const trueNs: number[] = [];
  const resolving: MeasuredPass[][] = [];
  for (;;) {
    const arrived = resolving.length >= 2 ? resolving.shift() : undefined;
    const measured: BatchMeasurement =
      arrived === undefined ? { kind: "none" } : { kind: "pass-times", passes: arrived };
    const batch = controller.nextBatch(measured);
    if (batch === null) {
      return { batches, trueNs };
    }
    batches.push(batch);
    const passes = units.slice(batch.first, batch.first + batch.count).map((label) => {
      const ns = costOf(costs, label);
      const startNs = Math.floor(between(random, 0, 1e12));
      const measuredNs =
        timer === "quantized" ? Q * (Math.floor((startNs + ns) / Q) - Math.floor(startNs / Q)) : ns;
      return { label, ns: measuredNs };
    });
    trueNs.push(
      units
        .slice(batch.first, batch.first + batch.count)
        .reduce((sum, l) => sum + costOf(costs, l), 0),
    );
    resolving.push(passes);
  }
}

const TIMED_CASES = QUALITY_SETTINGS.flatMap((setting) =>
  FRAME_INTERVALS_NS.flatMap((frameNs) =>
    (["quantized", "full"] as const).map((timer) => ({ setting, frameNs, timer })),
  ),
);

describe("a batch sized from timestamps", () => {
  it.each(TIMED_CASES)(
    "is never predicted over the cap less two quanta ($setting, $frameNs ns, $timer)",
    ({ setting, frameNs, timer }) => {
      const budgetNs = batchCapNs(setting, frameNs) - 2 * Q;
      const over = runTimed(setting, frameNs, timer).batches.filter(
        (b) => b.predictedNs !== null && !b.overBudget && b.predictedNs > budgetNs,
      );
      expect(over).toEqual([]);
    },
  );

  it.each(TIMED_CASES)(
    "never runs past the cap less two quanta when predicted in it ($setting, $frameNs ns, $timer)",
    ({ setting, frameNs, timer }) => {
      const budgetNs = batchCapNs(setting, frameNs) - 2 * Q;
      const run = runTimed(setting, frameNs, timer);
      const over = run.batches.filter(
        (b, k) => b.predictedNs !== null && !b.overBudget && (run.trueNs[k] ?? 0) > budgetNs,
      );
      expect(over).toEqual([]);
    },
  );

  it("packs several measured passes into one batch", () => {
    const largest = Math.max(
      ...runTimed("low", 16_683_000, "quantized").batches.map((b) => b.count),
    );
    expect(largest).toBeGreaterThan(1);
  });

  it("proposes a unit whose pass has not been measured alone, unpredicted", () => {
    const [first] = runTimed("low", 16_683_000, "quantized").batches;
    expect(first).toEqual({
      first: 0,
      count: 1,
      predictedNs: null,
      budgetNs: 5_868_928,
      overBudget: false,
    });
  });

  it("proposes a unit predicted over the budget alone, flagged", () => {
    const controller = new BatchController({
      setting: "high",
      frameIntervalNs: 6_944_000,
      timing: { timer: "full" },
      units: ["clouds", "clouds"],
    });
    controller.nextBatch({ kind: "none" });
    const batch = controller.nextBatch({
      kind: "pass-times",
      passes: [{ label: "clouds", ns: 3_000_000 }],
    });
    expect(batch).toMatchObject({ count: 1, predictedNs: 3_000_000, overBudget: true });
  });

  it("proposes every unit once, in order", () => {
    let next = 0;
    const gaps = runTimed("high", 16_683_000, "full").batches.filter((b) => {
      const gap = b.first !== next;
      next = b.first + b.count;
      return gap;
    });
    expect({ gaps: gaps.length, end: next }).toEqual({ gaps: 0, end: 4 * 6 * 8 });
  });

  it("never proposes more than maxUnits", () => {
    const largest = Math.max(...runTimed("low", 16_683_000, "full", 3).batches.map((b) => b.count));
    expect(largest).toBe(3);
  });

  it("ignores passes whose labels are not the job's", () => {
    const controller = new BatchController({
      setting: "low",
      frameIntervalNs: 16_683_000,
      timing: { timer: "full" },
      units: ["sky", "sky", "sky"],
    });
    controller.nextBatch({ kind: "none" });
    const batch = controller.nextBatch({
      kind: "pass-times",
      passes: [{ label: "live/terrain", ns: 100 }],
    });
    expect(batch).toMatchObject({ count: 1, predictedNs: null });
  });

  it("refuses a round trip", () => {
    const controller = new BatchController({
      setting: "low",
      frameIntervalNs: 16_683_000,
      timing: { timer: "quantized" },
      units: ["sky", "sky"],
    });
    controller.nextBatch({ kind: "none" });
    expect(() => controller.nextBatch({ kind: "round-trip", roundTripNs: 4_000_000 })).toThrow(
      Error,
    );
  });
});

/** Five empty submits' round trips, ns: their median is 3 ms, their mean is not. */
const EMPTY_ROUND_TRIPS_NS = [3_100_000, 2_900_000, 3_000_000, 3_800_000, 2_950_000] as const;

/** The fake's submit overhead, ns: a round trip is the batch's GPU time plus this. */
const OVERHEAD_NS = 3_000_000;

interface RoundTripRun {
  readonly batches: ReadonlyArray<Batch>;
  /** The fake's cost per unit while each batch ran, ns. */
  readonly costNs: ReadonlyArray<number>;
}

/**
 * Runs a job of `units` units on a fake GPU without timestamps, whose cost per unit is
 * `costAt(k)` ns while batch k runs, feeding the controller each batch's round trip.
 */
function runRoundTrips(
  setting: QualitySetting,
  units: number,
  costAt: (k: number) => number,
  maxUnits?: number,
): RoundTripRun {
  const controller = new BatchController({
    setting,
    frameIntervalNs: 16_683_000,
    timing: { timer: "absent", emptyRoundTripsNs: EMPTY_ROUND_TRIPS_NS },
    units: Array.from({ length: units }, () => "unit"),
    ...(maxUnits === undefined ? {} : { maxUnits }),
  });
  const batches: Batch[] = [];
  const costNs: number[] = [];
  let measured: BatchMeasurement = { kind: "none" };
  for (;;) {
    const batch = controller.nextBatch(measured);
    if (batch === null) {
      return { batches, costNs };
    }
    const cost = costAt(batches.length);
    batches.push(batch);
    costNs.push(cost);
    measured = { kind: "round-trip", roundTripNs: batch.count * cost + OVERHEAD_NS };
  }
}

describe("a batch sized from round trips", () => {
  it("starts at one unit", () => {
    expect(runRoundTrips("low", 100, () => 50_000).batches[0]).toMatchObject({
      first: 0,
      count: 1,
      predictedNs: null,
    });
  });

  it.each(QUALITY_SETTINGS)(
    "is never over the cap at the cost per unit it last measured (%s)",
    (setting) => {
      const capNs = batchCapNs(setting, 16_683_000);
      // The cost per unit triples at batch 30 and falls to a fifth at batch 40.
      const run = runRoundTrips(setting, 20_000, (k) =>
        k < 30 ? 37_000 : k < 40 ? 111_000 : 7_400,
      );
      const over = run.batches.filter(
        (b, k) => k > 0 && b.count * (run.costNs[k - 1] ?? 0) > capNs,
      );
      expect(over).toEqual([]);
    },
  );

  it("grows by at most 1.5 times a step, and from one unit to two", () => {
    const run = runRoundTrips("low", 20_000, (k) => (k < 30 ? 37_000 : 7_400));
    const fast = run.batches.filter((b, k) => {
      const before = run.batches[k - 1];
      return before !== undefined && b.count > Math.max(2, 1.5 * before.count);
    });
    expect(fast).toEqual([]);
  });

  it("reaches the cap's size at a steady cost", () => {
    // 6 ms holds 162 units of 37,000 ns.
    const run = runRoundTrips("low", 20_000, () => 37_000);
    expect(run.batches.at(-2)?.count).toBe(162);
  });

  it("halves after an overrun", () => {
    // Steady at 162 units of 37,000 ns; at batch 30 a unit costs 44,400 ns, so 162 take 7.19 ms.
    const run = runRoundTrips("low", 20_000, (k) => (k < 30 ? 37_000 : 44_400));
    expect([run.batches[30]?.count, run.batches[31]?.count]).toEqual([162, 81]);
  });

  it("shrinks below half after an overrun when half would still exceed the cap", () => {
    // At batch 30 the cost per unit triples: 81 units would take 9 ms, and 6 ms holds 54.
    const run = runRoundTrips("low", 20_000, (k) => (k < 30 ? 37_000 : 111_000));
    expect(run.batches[31]?.count).toBe(54);
  });

  it("takes the GPU time as the round trip less the median of five empty submits", () => {
    const run = runRoundTrips("low", 100, () => 50_000);
    expect(run.batches[1]).toMatchObject({ count: 2, predictedNs: 100_000 });
  });

  it("repeats its last size while no round trip has arrived", () => {
    const controller = new BatchController({
      setting: "low",
      frameIntervalNs: 16_683_000,
      timing: { timer: "absent", emptyRoundTripsNs: EMPTY_ROUND_TRIPS_NS },
      units: Array.from({ length: 100 }, () => "unit"),
    });
    controller.nextBatch({ kind: "none" });
    controller.nextBatch({ kind: "round-trip", roundTripNs: 3_050_000 });
    const counts = [controller.nextBatch({ kind: "round-trip", roundTripNs: 3_100_000 })?.count];
    counts.push(controller.nextBatch({ kind: "none" })?.count);
    expect(counts).toEqual([3, 3]);
  });

  it("never proposes more than maxUnits", () => {
    const largest = Math.max(
      ...runRoundTrips("low", 2_000, () => 37_000, 40).batches.map((b) => b.count),
    );
    expect(largest).toBe(40);
  });

  it("proposes a unit over the cap alone, flagged", () => {
    const run = runRoundTrips("low", 10, () => 7_000_000);
    expect(run.batches[1]).toMatchObject({ count: 1, predictedNs: 7_000_000, overBudget: true });
  });

  it.each([
    ["four", EMPTY_ROUND_TRIPS_NS.slice(0, 4)],
    ["six", [...EMPTY_ROUND_TRIPS_NS, 3_000_000]],
    ["a negative one", [3_000_000, 3_000_000, -1, 3_000_000, 3_000_000]],
  ])("refuses a baseline of %s round trips", (_, emptyRoundTripsNs) => {
    expect(
      () =>
        new BatchController({
          setting: "low",
          frameIntervalNs: 16_683_000,
          timing: { timer: "absent", emptyRoundTripsNs },
          units: ["unit"],
        }),
    ).toThrow(RangeError);
  });

  it("takes five empty submits for its baseline", () => {
    expect(EMPTY_SUBMIT_SAMPLES).toBe(5);
  });

  it("refuses pass times", () => {
    const controller = new BatchController({
      setting: "low",
      frameIntervalNs: 16_683_000,
      timing: { timer: "absent", emptyRoundTripsNs: EMPTY_ROUND_TRIPS_NS },
      units: ["unit", "unit"],
    });
    controller.nextBatch({ kind: "none" });
    expect(() => controller.nextBatch({ kind: "pass-times", passes: [] })).toThrow(Error);
  });

  it("refuses a round trip before any batch", () => {
    const controller = new BatchController({
      setting: "low",
      frameIntervalNs: 16_683_000,
      timing: { timer: "absent", emptyRoundTripsNs: EMPTY_ROUND_TRIPS_NS },
      units: ["unit"],
    });
    expect(() => controller.nextBatch({ kind: "round-trip", roundTripNs: 4_000_000 })).toThrow(
      Error,
    );
  });
});

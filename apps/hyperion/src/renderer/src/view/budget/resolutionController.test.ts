import { describe, expect, it } from "vitest";

import type { PassTimes } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import { overlappedPasses, serialPasses, timedPass } from "../../test/passTimes";
import {
  CONTROLLER_TUNING,
  gpuTimeMs,
  ResolutionController,
  type ResolutionTarget,
  type ScaleBounds,
} from "./resolutionController";
import { frameGpuBudgetMs } from "./viewBudget";

/** A 60 Hz primary with two instruments open: 0.8 × 16.67 ms less two canvases' 0.3 ms. */
const TARGET: ResolutionTarget = { periodMs: 1000 / 60, gpuBudgetMs: frameGpuBudgetMs(60, 2) };
const BUDGET_MS = TARGET.gpuBudgetMs;
const BAND_FLOOR_MS = CONTROLLER_TUNING.raiseBelow * BUDGET_MS;

/** Timer quantum under Dawn's timestamp quantization, ms (R01 Design note 4). */
const QUANTUM_MS = 65_536 / 1e6;

const BOUNDS_CASES = [
  ["the setting's", SETTINGS.high.internalScaleBounds],
  ["a narrower", [0.6, 0.8]],
] as const;

/** A frame's GPU time at a scale: a fixed part and a part in proportion to the pixels. */
type Load = (scale: number, frame: number) => number;

function load(fixedMs: number, pixelMs: number): Load {
  return (scale) => fixedMs + pixelMs * scale * scale;
}

function exact(ms: number): number {
  return ms;
}

function everyResolved(): number {
  return Infinity;
}

interface RunOptions {
  /** Updates between drawing a frame and its time having resolved. */
  readonly latency?: number;
  /** Turns a frame's exact GPU time into what the controller is given. */
  readonly measure?: (ms: number) => number;
  /** The most resolved frames handed over at an update (the rest wait for later ones). */
  readonly deliver?: (frame: number) => number;
}

/**
 * Drives a controller for `frames` frames at 60 Hz, handing each update the GPU times of the
 * frames resolved since the last, each once; returns the scale every frame was drawn at.
 */
function run(
  controller: ResolutionController,
  frames: number,
  cost: Load,
  { latency = 2, measure = exact, deliver = everyResolved }: RunOptions = {},
  firstFrame = 0,
): number[] {
  const scales: number[] = [];
  const drawn: Array<{ readonly ms: number; readonly at: number }> = [];
  let next = 0;
  for (let i = 0; i < frames; i += 1) {
    const frame = firstFrame + i;
    scales.push(controller.scale);
    drawn.push({ ms: cost(controller.scale, frame), at: i });
    const batch: number[] = [];
    const most = deliver(frame);
    while (next < drawn.length && batch.length < most && i - (drawn[next]?.at ?? i) >= latency) {
      batch.push(measure(drawn[next]?.ms ?? 0));
      next += 1;
    }
    controller.update(batch, TARGET.periodMs);
  }
  return scales;
}

/** The frames at which the scale changed, and the sign of each change. */
function changes(scales: ReadonlyArray<number>): Array<{ frame: number; sign: number }> {
  const out: Array<{ frame: number; sign: number }> = [];
  for (let i = 1; i < scales.length; i += 1) {
    const before = scales[i - 1] ?? 0;
    const after = scales[i] ?? 0;
    if (after !== before) {
      out.push({ frame: i, sign: Math.sign(after - before) });
    }
  }
  return out;
}

/** A light load that fits at full scale, a heavier one, and the light one again. */
function stepped(): { before: number[]; after: number[]; back: number[] } {
  const controller = new ResolutionController(SETTINGS.high.internalScaleBounds, TARGET);
  const before = run(controller, 1_000, load(2, 9));
  const after = run(controller, 2_000, load(2, 18), {}, 1_000);
  const back = run(controller, 2_000, load(2, 9), {}, 3_000);
  return { before, after, back };
}

/** A small deterministic generator in [0, 1), for the timestamps' phases. */
function uniform(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    return state / 2 ** 32;
  };
}

/**
 * A frame's GPU time as a quantized timer reads it: `passes` passes sharing the time, each
 * timestamp floored to the 65,536 ns grid from a random phase.
 */
function quantized(passes: number, seed: number): (ms: number) => number {
  const next = uniform(seed);
  return (ms) => {
    let total = 0;
    for (let p = 0; p < passes; p += 1) {
      const begin = next() * QUANTUM_MS;
      const end = begin + ms / passes;
      total +=
        Math.floor(end / QUANTUM_MS) * QUANTUM_MS - Math.floor(begin / QUANTUM_MS) * QUANTUM_MS;
    }
    return total;
  };
}

/**
 * A controller on bounds [0.5, 1] dropped once by two frames at twice the budget, its settle past:
 * at 0.67.
 */
function droppedOnce(): ResolutionController {
  const controller = new ResolutionController([0.5, 1], TARGET);
  controller.update([2 * BUDGET_MS], TARGET.periodMs);
  controller.update([2 * BUDGET_MS], TARGET.periodMs);
  for (let update = 0; update < CONTROLLER_TUNING.settleUpdates; update += 1) {
    controller.update([], TARGET.periodMs);
  }
  return controller;
}

/** Every 100 frames, two frames do twice the pixel work. */
const PIXEL_SPIKES: Load = (scale, frame) => load(2, frame % 100 < 2 ? 12 : 6)(scale, frame);

describe("ResolutionController", () => {
  it("starts at the bounds' max", () => {
    expect(new ResolutionController([0.5, 1], TARGET).scale).toBe(1);
    expect(new ResolutionController([0.6, 0.8], TARGET).scale).toBe(0.8);
  });

  it("converges on a synthetic load and holds there", () => {
    const controller = new ResolutionController(SETTINGS.high.internalScaleBounds, TARGET);
    const cost = load(2, 16);
    const scales = run(controller, 1_200, cost);
    const settledMs = cost(scales.at(-1) ?? 0, 0);
    expect(settledMs).toBeLessThanOrEqual(BUDGET_MS);
    expect(settledMs).toBeGreaterThanOrEqual(BAND_FLOOR_MS);
    expect(changes(scales).at(-1)?.frame ?? 0).toBeLessThan(120);
  });

  it.each(BOUNDS_CASES)(
    "holds at the least of %s bounds under a load too heavy for it",
    (_, bounds: ScaleBounds) => {
      const scales = run(new ResolutionController(bounds, TARGET), 1_200, load(4, 80));
      expect(Math.min(...scales)).toBe(bounds[0]);
      expect(scales.at(-1)).toBe(bounds[0]);
    },
  );

  it.each(BOUNDS_CASES)("stays at the greatest of %s bounds under a light load", (_, bounds) => {
    const scales = run(new ResolutionController(bounds, TARGET), 1_200, load(1, 2));
    expect(new Set(scales)).toEqual(new Set([bounds[1]]));
  });

  it("drops under a step in load and does not rise again under it", () => {
    const { before, after } = stepped();
    expect(changes(before)).toEqual([]);
    const down = changes(after);
    expect(down.length).toBeGreaterThan(0);
    expect(down.every((change) => change.sign < 0)).toBe(true);
  });

  it("rises when the load falls back, does not drop again, and holds inside the band", () => {
    const { back } = stepped();
    const up = changes(back);
    expect(up.length).toBeGreaterThan(0);
    expect(up.every((change) => change.sign > 0)).toBe(true);
    // A rise scales the run's worst frame as pixels alone, so with a fixed part it may land short
    // of the top, inside the band.
    const settledMs = load(2, 9)(back.at(-1) ?? 0, 0);
    expect(settledMs).toBeGreaterThanOrEqual(BAND_FLOOR_MS);
    expect(settledMs).toBeLessThanOrEqual(BUDGET_MS);
  });

  it("settles rather than cycles when each rise brings back frames over the budget", () => {
    // The spikes are over the budget at full scale and within it below, while the frames between
    // would allow full scale.
    const controller = new ResolutionController(SETTINGS.high.internalScaleBounds, TARGET);
    const scales = run(controller, 6_000, PIXEL_SPIKES);
    const all = changes(scales);
    expect(all.length).toBeLessThanOrEqual(5);
    expect(all.at(-1)?.frame ?? 0).toBeLessThan(1_000);
    expect(PIXEL_SPIKES(scales.at(-1) ?? 0, 0)).toBeLessThanOrEqual(BUDGET_MS);
  });

  it("steps down until a recurring fixed cost fits, then holds", () => {
    // Two frames every 100 carry 9 ms more at any scale (a periodic bake, say), which the square
    // law under-reads, so each recurrence takes one more step.
    const base = load(2, 6);
    const spiky: Load = (scale, frame) => base(scale, frame) + (frame % 100 < 2 ? 9 : 0);
    const controller = new ResolutionController(SETTINGS.high.internalScaleBounds, TARGET);
    const scales = run(controller, 6_000, spiky);
    const all = changes(scales);
    expect(all.at(-1)?.frame ?? 0).toBeLessThan(1_000);
    expect(all.filter((change) => change.sign > 0).length).toBeLessThanOrEqual(2);
    expect(spiky(scales.at(-1) ?? 0, 0)).toBeLessThanOrEqual(BUDGET_MS);
  });

  it("is stable with pass times quantized to 65,536 ns", () => {
    const cost = load(2, 16);
    const exactScale = run(new ResolutionController([0.5, 1], TARGET), 1_000, cost).at(-1) ?? 0;
    const controller = new ResolutionController([0.5, 1], TARGET);
    const scales = run(controller, 6_000, cost, { measure: quantized(12, 7) });
    expect(Math.abs((scales.at(-1) ?? 0) - exactScale)).toBeLessThan(0.02);
    expect(changes(scales.slice(300))).toEqual([]);
  });

  it("drops and rises with times handed over unevenly, none, one or two an update", () => {
    const pattern = [0, 2, 1, 0, 2, 1, 1];
    const deliver = (frame: number) => pattern[frame % pattern.length] ?? 1;
    const controller = new ResolutionController([0.5, 1], TARGET);
    const heavy = load(2, 16);
    const down = run(controller, 1_000, heavy, { deliver }).at(-1) ?? 0;
    expect(down).toBeLessThan(1);
    expect(heavy(down, 0)).toBeLessThanOrEqual(BUDGET_MS);
    const up = run(controller, 1_000, load(1, 4), { deliver }, 1_000).at(-1) ?? 0;
    expect(up).toBe(1);
  });

  it("uses the frame interval when timestamps are unavailable", () => {
    // Each frame costs 4 ms and 20 ms of pixels at full scale, and waits for the next vsync.
    const periodMs = TARGET.periodMs;
    const cost = load(4, 20);
    const controller = new ResolutionController([0.5, 1], TARGET);
    const intervals: number[] = [];
    for (let frame = 0; frame < 20_000; frame += 1) {
      const intervalMs =
        periodMs * Math.max(1, Math.ceil(cost(controller.scale, frame) / periodMs));
      intervals.push(intervalMs);
      controller.update(undefined, intervalMs);
    }
    expect(intervals[0]).toBeGreaterThan(1.5 * periodMs);
    expect(controller.scale).toBeLessThan(1);
    expect(cost(controller.scale, 0)).toBeLessThanOrEqual(periodMs);
    // Probes that fail cost a few frames each, ever more rarely.
    const missed = intervals.slice(1_000).filter((ms) => ms > 1.5 * periodMs).length;
    expect(missed / (intervals.length - 1_000)).toBeLessThan(0.005);
  });

  it("reads GPU time, not the interval, where both are given", () => {
    // Every interval is a missed frame (the main thread's), and the GPU is well within budget.
    const controller = new ResolutionController([0.5, 1], TARGET);
    for (let frame = 0; frame < 600; frame += 1) {
      controller.update([6], 3 * TARGET.periodMs);
    }
    expect(controller.scale).toBe(1);
  });

  it("turns to the frame interval when the timer goes absent", () => {
    const controller = new ResolutionController([0.5, 1], TARGET);
    run(controller, 300, load(2, 6));
    expect(controller.scale).toBe(1);
    for (let frame = 0; frame < 10; frame += 1) {
      controller.update(undefined, 2 * TARGET.periodMs);
    }
    expect(controller.scale).toBeLessThan(1);
  });

  it("ignores GPU times that are not finite", () => {
    // Read as frames, an infinite time would be over the budget.
    const controller = new ResolutionController([0.5, 1], TARGET);
    for (let frame = 0; frame < 600; frame += 1) {
      controller.update([Number.NaN, Infinity], TARGET.periodMs);
    }
    expect(controller.scale).toBe(1);
  });

  it("ignores negative GPU times", () => {
    // Read as frames, a run of them would cost nothing and raise the scale to the top.
    const controller = new ResolutionController([0.5, 1], TARGET);
    run(controller, 300, load(4, 80));
    expect(controller.scale).toBe(0.5);
    for (let frame = 0; frame < 600; frame += 1) {
      controller.update([-1], TARGET.periodMs);
    }
    expect(controller.scale).toBe(0.5);
  });

  it("ignores intervals that are not finite", () => {
    // Read as frames, an infinite interval would be a missed one.
    const controller = new ResolutionController([0.5, 1], TARGET);
    for (const intervalMs of [Infinity, Number.NaN, Infinity, Infinity]) {
      controller.update(undefined, intervalMs);
    }
    expect(controller.scale).toBe(1);
  });

  it("ignores intervals that are not positive", () => {
    // Read as frames, a run of them would meet every vsync and probe upward.
    const controller = new ResolutionController([0.5, 1], TARGET);
    for (let frame = 0; frame < 40; frame += 1) {
      controller.update(undefined, 3 * TARGET.periodMs);
    }
    expect(controller.scale).toBe(0.5);
    for (let frame = 0; frame < 1_000; frame += 1) {
      controller.update(undefined, frame % 2 === 0 ? 0 : -5);
    }
    expect(controller.scale).toBe(0.5);
  });

  it("rises to the top from a run of frames that cost nothing", () => {
    const controller = new ResolutionController([0.5, 1], TARGET);
    run(controller, 300, load(4, 80));
    expect(controller.scale).toBe(0.5);
    run(controller, 100, () => 0, {}, 300);
    expect(controller.scale).toBe(1);
  });

  it("rises past a lone frame at or over raiseBelow, leaving it out of the run's worst, as a drop leaves a lone frame over the budget (lane fix-floor-calm)", () => {
    const controller = droppedOnce();
    const from = controller.scale;
    // Every tenth frame at 0.8 of the budget, over the band's floor; the rest at half of it.
    let frames = 0;
    while (controller.scale === from && frames < 1_000) {
      controller.update([frames % 10 === 5 ? 0.8 * BUDGET_MS : 0.5 * BUDGET_MS], TARGET.periodMs);
      frames += 1;
    }
    expect(frames).toBe(CONTROLLER_TUNING.raiseHoldFrames);
    expect(controller.scale).toBeCloseTo(from * Math.sqrt(CONTROLLER_TUNING.aim / 0.5), 12);
  });

  it("does not rise while two frames in four stand at or over raiseBelow, as before", () => {
    const controller = droppedOnce();
    const from = controller.scale;
    for (let frame = 0; frame < 1_000; frame += 1) {
      const pair = frame % 10 === 5 || frame % 10 === 6;
      controller.update([pair ? 0.8 * BUDGET_MS : 0.5 * BUDGET_MS], TARGET.periodMs);
    }
    expect(controller.scale).toBe(from);
  });

  it("says when its drop aims below its bounds' min, and only then (lane fix-floor-calm)", () => {
    const wanted = (ms: number, scale: number): boolean => {
      const controller = new ResolutionController([scale, 1], TARGET);
      controller.update([ms], TARGET.periodMs);
      controller.update([ms], TARGET.periodMs);
      return controller.wantedBelowMin;
    };
    // 5% over aims at 0.93 of the scale; five times over at 0.42.
    expect([
      wanted(1.05 * BUDGET_MS, 0.5),
      wanted(5 * BUDGET_MS, 0.5),
      wanted(1.05 * BUDGET_MS, 0.95),
    ]).toEqual([false, true, true]);
  });

  it("keeps its scale and starts its run again when restarted (lane fix-floor-calm)", () => {
    const controller = droppedOnce();
    const from = controller.scale;
    const light = (): void => {
      controller.update([0.5 * BUDGET_MS], TARGET.periodMs);
    };
    for (let frame = 0; frame < CONTROLLER_TUNING.raiseHoldFrames - 1; frame += 1) {
      light();
    }
    controller.restart();
    for (let frame = 0; frame < CONTROLLER_TUNING.raiseHoldFrames - 1; frame += 1) {
      light();
    }
    const held = controller.scale;
    light();
    expect([held, controller.scale > from]).toEqual([from, true]);
  });

  it("drops further when its budget shrinks", () => {
    const controller = new ResolutionController([0.5, 1], TARGET);
    const cost = load(2, 12);
    const wide = run(controller, 600, cost).at(-1) ?? 0;
    controller.retarget({ ...TARGET, gpuBudgetMs: 0.7 * BUDGET_MS });
    const narrow = run(controller, 600, cost, {}, 600).at(-1) ?? 0;
    expect(narrow).toBeLessThan(wide);
    expect(cost(narrow, 0)).toBeLessThanOrEqual(0.7 * BUDGET_MS);
  });

  it("waits only the first wait to rise after a new budget, whatever rises failed before", () => {
    const controller = new ResolutionController([0.5, 1], TARGET);
    run(controller, 1_000, PIXEL_SPIKES);
    const held = controller.scale;
    expect(held).toBeLessThan(1);
    controller.retarget({ ...TARGET, gpuBudgetMs: 1.5 * BUDGET_MS });
    const scales = run(controller, 200, load(2, 6), {}, 1_000);
    const first = changes([held, ...scales])[0];
    expect(first?.sign).toBe(1);
    expect(first?.frame ?? Infinity).toBeLessThanOrEqual(CONTROLLER_TUNING.raiseHoldFrames + 4);
  });

  it("waits only the first wait again once a rise has held through its watch", () => {
    const controller = new ResolutionController([0.5, 1], TARGET);
    // A rise that the next spike undoes: the wait before the next rise doubles.
    const failedDrop = changes(run(controller, 150, PIXEL_SPIKES)).at(-1);
    expect(failedDrop?.sign).toBe(-1);
    const failed = controller.scale;
    const light = load(2, 6);
    const recovery = changes([failed, ...run(controller, 400, light, {}, 150)]);
    const waited = 150 - (failedDrop?.frame ?? 150) + (recovery[0]?.frame ?? 0);
    expect(waited).toBeGreaterThan(2 * CONTROLLER_TUNING.raiseHoldFrames);
    // That rise holds through its watch; a later drop and lightening rise after the first wait.
    run(controller, 100, load(2, 18), {}, 550);
    const dropped = controller.scale;
    expect(dropped).toBeLessThan(1);
    const next = changes([dropped, ...run(controller, 200, light, {}, 650)]);
    expect(next[0]?.sign).toBe(1);
    expect(next[0]?.frame ?? Infinity).toBeLessThanOrEqual(CONTROLLER_TUNING.raiseHoldFrames + 4);
  });

  it("keeps the frames it holds when given the budget it already has", () => {
    const over = 1.2 * BUDGET_MS;
    const controller = new ResolutionController([0.5, 1], TARGET);
    controller.update([over], TARGET.periodMs);
    controller.retarget({ ...TARGET });
    controller.update([over], TARGET.periodMs);
    expect(controller.scale).toBeLessThan(1);
  });

  it("refuses bounds and targets that are not positive and ordered", () => {
    expect(() => new ResolutionController([0.8, 0.6], TARGET)).toThrow(RangeError);
    expect(() => new ResolutionController([0, 1], TARGET)).toThrow(RangeError);
    expect(() => new ResolutionController([0.5, Number.NaN], TARGET)).toThrow(RangeError);
    expect(() => new ResolutionController([0.5, 1], { periodMs: 16, gpuBudgetMs: 0 })).toThrow(
      RangeError,
    );
  });
});

/** The reading before lane fix-mac-gpu-time: every pass's `ns` summed, ms. */
function summedMs(times: ReadonlyArray<PassTimes>): number {
  let ns = 0;
  for (const resolve of times) {
    for (const pass of resolve.passes) {
      ns += pass.ns;
    }
  }
  return ns / 1e6;
}

/** A frame's resolves, each its passes' times, ns, stamped end to end as a serial timer does. */
function serialFrame(...resolves: ReadonlyArray<ReadonlyArray<number>>): PassTimes[] {
  return resolves.map((passesNs, i) => ({
    frame: i + 1,
    timer: "quantized",
    passes: serialPasses(passesNs.map((ns, j) => [`pass ${String(j)}`, ns] as const)),
  }));
}

/**
 * The frames today's tests time, each its resolves' passes, ns: this file's, the views probe's,
 * the frame pacing's, the spike's metrics' and the pass timer's own.
 */
const FIXTURE_FRAMES: ReadonlyArray<ReadonlyArray<ReadonlyArray<number>>> = [
  [[65_536, 131_072], [1_000_000]],
  [[2e6, 5e6, 1e6]],
  [[2e5]],
  [[4e6], [3e6], [1.5e6]],
  [[5e6], [2e6], [2e6]],
  [[100e6], [1e6]],
  [[4_000_000], [1], [9_000_000, 1_000_000], [250_000, 500_000]],
  [[250, 3_000]],
];

/**
 * Frame `n` of a sweep: 1 to 5 resolves of 1 to 8 passes each, every pass a whole number of
 * Dawn's 65,536 ns quanta, as its timer gives them.
 */
function quantisedFrame(n: number): number[][] {
  const resolves: number[][] = [];
  for (let r = 0; r < 1 + (n % 5); r += 1) {
    const passes: number[] = [];
    for (let k = 0; k < 1 + ((n * 7 + r * 3) % 8); k += 1) {
      passes.push(65_536 * (1 + ((n * 31 + r * 17 + k * 13) % 300)));
    }
    resolves.push(passes);
  }
  return resolves;
}

describe("gpuTimeMs", () => {
  it("sums every pass of every resolve the frame made, in ms", () => {
    expect(gpuTimeMs(serialFrame([65_536, 131_072], [1_000_000]))).toBeCloseTo(1.196_608, 9);
  });

  it("reads serial stamps as the sum it read before, exactly, on today's fixtures and quantised frames", () => {
    const quantised = Array.from({ length: 200 }, (_, n) => quantisedFrame(n));
    const frames = [...FIXTURE_FRAMES, ...quantised].map((resolves) => serialFrame(...resolves));
    expect(frames.filter((frame) => gpuTimeMs(frame) !== summedMs(frame))).toEqual([]);
  });

  it("counts the composite's wait on the march once, as the M5 stamps a frame", () => {
    const frame: PassTimes[] = [
      { frame: 1, timer: "full", passes: overlappedPasses(3_500_000, 12_000_000, 500_000) },
    ];
    expect([summedMs(frame), gpuTimeMs(frame)]).toEqual([28, 16]);
  });

  it("adds the unions of a frame's resolves, whose stamps share no origin", () => {
    const frame: PassTimes[] = [
      { frame: 1, timer: "full", passes: overlappedPasses(1_000_000, 4_000_000, 500_000) },
      { frame: 2, timer: "full", passes: [timedPass("view:wireframe", 2_000_000)] },
    ];
    expect(gpuTimeMs(frame)).toBe(7.5);
  });

  it("skips a pass whose end precedes its begin, and an empty one, which the sum counted", () => {
    const frame: PassTimes[] = [
      {
        frame: 1,
        timer: "full",
        passes: [
          timedPass("reset", -3_000_000, 5_000_000),
          timedPass("unwritten", 0, 0),
          timedPass("tonemap", 1_000_000, 6_000_000),
        ],
      },
    ];
    expect([summedMs(frame), gpuTimeMs(frame)]).toEqual([-2, 1]);
  });
});

describe("ResolutionController on the M5's stamps", () => {
  it("holds scale 1 at 60 fps where the passes' union fits the floor's budget and their sum does not", () => {
    // The frame-rate floor's target (viewBudget.ts: 0.8 of 30 fps's period, 26.7 ms).
    const floor: ResolutionTarget = { periodMs: 1000 / 30, gpuBudgetMs: frameGpuBudgetMs(30, 0) };
    // 16 ms of GPU work a frame, drawn every 16.7 ms; the composite's span holds the 12 ms march.
    const frame: PassTimes[] = [
      { frame: 1, timer: "full", passes: overlappedPasses(3_500_000, 12_000_000, 500_000) },
    ];
    const read = (reading: (times: ReadonlyArray<PassTimes>) => number): number => {
      const control = new ResolutionController(SETTINGS.high.internalScaleBounds, floor);
      let scale = control.scale;
      for (let n = 0; n < 600; n += 1) {
        scale = control.update([reading(frame)], 1000 / 60);
      }
      return scale;
    };
    const [min, max] = SETTINGS.high.internalScaleBounds;
    expect([summedMs(frame) > floor.gpuBudgetMs, read(summedMs) < max, read(gpuTimeMs)]).toEqual([
      true,
      true,
      max,
    ]);
    expect(read(summedMs)).toBeGreaterThanOrEqual(min);
  });
});

import { describe, expect, it } from "vitest";

import { type RenderStyle, viewId } from "../camera/state";
import type { PassTimes } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import {
  BudgetedScale,
  CALM_FRAMES,
  CALM_INTERVAL_SHARE,
  CALM_SHARE,
  drawsInFrame,
  FramePacer,
  PENDING_PRIMARY_FRAMES,
  PrimaryFrameTimes,
} from "./framePacing";
import { type ViewBudget, viewBudgets, type ViewSpec } from "./viewBudget";

const PRIMARY = viewId("view");

/** One resolve's times: its number and its passes' GPU time, ms each. */
function resolve(frame: number, ...passesMs: ReadonlyArray<number>): PassTimes {
  return {
    frame,
    timer: "quantized",
    passes: passesMs.map((ms, i) => ({
      label: `pass ${String(i)}`,
      ns: ms * 1e6,
      bracketed: false,
    })),
  };
}

function primaryBudget(style: RenderStyle, instruments: number): ViewBudget {
  const views: ViewSpec[] = [
    { id: PRIMARY, slot: "primary", style },
    ...Array.from({ length: instruments }, (_, i) => ({
      id: viewId(`instrument-${String(i + 1)}`),
      slot: "instrument" as const,
      style: "wireframe" as const,
    })),
  ];
  const budget = viewBudgets(views, "high").get(PRIMARY);
  if (budget === undefined) {
    throw new Error("no primary budget");
  }
  return budget;
}

describe("drawsInFrame", () => {
  it("draws a 60 Hz view in every frame and a 30 Hz view in every second one", () => {
    const frames = Array.from({ length: 6 }, (_, frame) => frame);
    expect([
      frames.map((frame) => drawsInFrame(frame, 60)),
      frames.map((frame) => drawsInFrame(frame, 30)),
    ]).toEqual([
      [true, true, true, true, true, true],
      [true, false, true, false, true, false],
    ]);
  });

  it("draws a 30 Hz instrument only in frames the primary draws in, at either rate", () => {
    const frames = Array.from({ length: 12 }, (_, frame) => frame);
    const instrument = frames.filter((frame) => drawsInFrame(frame, 30));
    expect([
      instrument.every((frame) => drawsInFrame(frame, 60)),
      instrument.every((frame) => drawsInFrame(frame, 30)),
    ]).toEqual([true, true]);
  });
});

/** What a loop drew on a clock: the callbacks the primary and a 30 Hz instrument drew in. */
interface Drawn {
  readonly primary: ReadonlyArray<number>;
  readonly instrument: ReadonlyArray<number>;
}

/**
 * Runs a loop's pacing over callbacks at `stampsMs`, as `VIEW`'s does: the primary at `primaryHz`,
 * then a 30 Hz instrument asked only in the primary's frames; the callbacks' indices each drew in.
 */
function paced(pacer: FramePacer, stampsMs: ReadonlyArray<number>, primaryHz: 60 | 30): Drawn {
  const primary: number[] = [];
  const instrument: number[] = [];
  stampsMs.forEach((nowMs, i) => {
    pacer.frame(nowMs);
    if (!pacer.draws(primaryHz)) {
      return;
    }
    primary.push(i);
    if (pacer.draws(30, primaryHz)) {
      instrument.push(i);
    }
  });
  return { primary, instrument };
}

/** The callbacks of a display at `hz` for `seconds` from `startMs`, each on its vsync. */
function vsyncs(hz: number, seconds: number, startMs = 0): number[] {
  return Array.from({ length: Math.round(hz * seconds) }, (_, i) => startMs + (i * 1000) / hz);
}

/** The draws a second among `drawn` (callback indices) of the callbacks from `from` to `to`. */
function perSecond(
  drawn: ReadonlyArray<number>,
  stampsMs: ReadonlyArray<number>,
  from: number,
  to: number,
): number {
  const count = drawn.filter((i) => i >= from && i < to).length;
  return (1000 * count) / ((stampsMs[to] ?? 0) - (stampsMs[from] ?? 0));
}

/** The intervals between the draws `drawn` made among the callbacks at `stampsMs`, ms. */
function drawIntervalsMs(
  drawn: ReadonlyArray<number>,
  stampsMs: ReadonlyArray<number>,
): ReadonlyArray<number> {
  return drawn.slice(1).map((i, n) => (stampsMs[i] ?? 0) - (stampsMs[drawn[n] ?? 0] ?? 0));
}

describe("FramePacer (R07.T19; the owner's frame rate)", () => {
  it("draws high, low and an instrument at 60, 30 and 30 a second on a 120 Hz clock, paced by rate", () => {
    const stamps = vsyncs(120, 4);
    const high = paced(new FramePacer("rate"), stamps, 60);
    const low = paced(new FramePacer("rate"), stamps, 30);
    // After the first second: the clock has measured its period.
    expect([
      perSecond(high.primary, stamps, 120, 360),
      perSecond(low.primary, stamps, 120, 360),
      perSecond(high.instrument, stamps, 120, 360),
    ]).toEqual([60, 30, 30]);
  });

  it("draws at up to the display's rate on a 120 Hz clock, paced by counting frames as before", () => {
    const stamps = vsyncs(120, 4);
    const high = paced(new FramePacer("display"), stamps, 60);
    const low = paced(new FramePacer("display"), stamps, 30);
    expect([
      perSecond(high.primary, stamps, 120, 360),
      perSecond(low.primary, stamps, 120, 360),
      perSecond(high.instrument, stamps, 120, 360),
    ]).toEqual([120, 60, 60]);
  });

  it.each([
    ["60 Hz", vsyncs(60, 4)],
    ["59.94 Hz", vsyncs(59.94, 4)],
    // A millisecond's jitter either way about each vsync.
    ["60 Hz with jitter", vsyncs(60, 4).map((ms, i) => ms + (i % 2 === 0 ? 0.9 : -0.9))],
  ])("draws the frames counting frames draws on a %s clock, on time", (_, stamps) => {
    expect([
      paced(new FramePacer("rate"), stamps, 60),
      paced(new FramePacer("rate"), stamps, 30),
    ]).toEqual([
      paced(new FramePacer("display"), stamps, 60),
      paced(new FramePacer("display"), stamps, 30),
    ]);
  });

  it("draws high at 72, low at 28.8 and an instrument beside high at 36 a second on a 144 Hz clock", () => {
    const stamps = vsyncs(144, 4);
    const high = paced(new FramePacer("rate"), stamps, 60);
    const low = paced(new FramePacer("rate"), stamps, 30);
    // The instrument keeps to the primary's every second vsync: every fourth, not every sixth.
    expect([
      perSecond(high.primary, stamps, 144, 504),
      perSecond(low.primary, stamps, 144, 504),
      perSecond(high.instrument, stamps, 144, 504),
      perSecond(low.instrument, stamps, 144, 504),
    ]).toEqual([72, 28.8, 36, 28.8]);
  });

  it("holds 60, 30 and 30 a second when the display goes from 120 to 60 Hz", () => {
    const stamps = [...vsyncs(120, 2), ...vsyncs(60, 4, 2000)];
    const high = paced(new FramePacer("rate"), stamps, 60);
    const low = paced(new FramePacer("rate"), stamps, 30);
    // From a quarter of a second after the switch, through the clock's own change of period.
    const from = 240 + 15;
    const to = from + 180;
    expect([
      perSecond(high.primary, stamps, from, to),
      perSecond(low.primary, stamps, from, to),
      perSecond(high.instrument, stamps, from, to),
    ]).toEqual([60, 30, 30]);
  });

  it("draws in the callback after a late frame, where counting frames waits a vsync more", () => {
    // A 30 Hz view on a 60 Hz clock; the frame drawn at callback 40 is late, its vsync 41 missed.
    const stamps = vsyncs(60, 2).filter((_, i) => i !== 41);
    const late = 41;
    const rate = paced(new FramePacer("rate"), stamps, 30).primary;
    const display = paced(new FramePacer("display"), stamps, 30).primary;
    expect([rate.find((i) => i > 40), display.find((i) => i > 40)]).toEqual([late, late + 1]);
  });

  it("brings light frames after a GPU-bound stretch back to their rate within a quarter of a second", () => {
    // 120 Hz: four seconds of frames three vsyncs apart, then light frames on every vsync.
    const boundMs = 4000;
    const stamps = [...vsyncs(40, 4), ...vsyncs(120, 2, boundMs)];
    const light = stamps.findIndex((ms) => ms >= boundMs);
    const quarter = stamps.findIndex((ms) => ms >= boundMs + 250);
    const vsyncMs = 1000 / 120;
    const after = (drawn: ReadonlyArray<number>): ReadonlyArray<number> =>
      drawIntervalsMs(
        drawn.filter((i) => i >= quarter),
        stamps,
      ).map((ms) => Math.round(ms / vsyncMs));
    const high = paced(new FramePacer("rate"), stamps, 60);
    const low = paced(new FramePacer("rate"), stamps, 30);
    expect([
      light > 0,
      new Set(after(high.primary)),
      new Set(after(low.primary)),
      new Set(after(high.instrument)),
    ]).toEqual([true, new Set([2]), new Set([4]), new Set([4])]);
  });

  it("draws every view at one rate in the same frames, whenever it is first asked", () => {
    const pacer = new FramePacer("rate");
    const first: number[] = [];
    const second: number[] = [];
    vsyncs(120, 2).forEach((nowMs, i) => {
      pacer.frame(nowMs);
      if (pacer.draws(30)) {
        first.push(i);
      }
      if (i >= 7 && pacer.draws(30)) {
        second.push(i);
      }
    });
    expect(second).toEqual(first.filter((i) => i >= 7));
  });

  it("measures the clock's vsync period under either pacing", () => {
    const rate = new FramePacer("rate");
    const display = new FramePacer("display");
    for (const nowMs of vsyncs(120, 1)) {
      rate.frame(nowMs);
      display.frame(nowMs);
    }
    expect([rate.periodMs, display.periodMs].map((ms) => ms.toFixed(3))).toEqual([
      "8.333",
      "8.333",
    ]);
  });
});

/** Times grouped from a first primary frame started at resolve 0. */
function started(): PrimaryFrameTimes {
  const times = new PrimaryFrameTimes();
  times.startFrame(0);
  return times;
}

describe("PrimaryFrameTimes", () => {
  it("counts an instrument's passes in the primary frame that submitted them", () => {
    const times = started();
    // The primary's frame resolves 1 and 2, an instrument's in the same frame 3; the next starts.
    times.startFrame(3);
    times.passTimes(resolve(1, 4));
    times.passTimes(resolve(2, 3));
    expect(times.take()).toEqual([]);
    times.passTimes(resolve(3, 1.5));
    expect(times.take()).toEqual([8.5]);
  });

  it("reports each frame once its resolves are all in, oldest first, none on some takes", () => {
    const times = started();
    times.startFrame(1);
    times.passTimes(resolve(1, 5));
    times.startFrame(3);
    expect(times.take()).toEqual([5]);
    expect(times.take()).toEqual([]);
    times.passTimes(resolve(2, 2));
    times.passTimes(resolve(3, 2));
    expect(times.take()).toEqual([4]);
  });

  it("takes reports that arrive before their frame's group ends", () => {
    const times = started();
    // The frame's resolves report while it is still being drawn, before the next starts.
    times.passTimes(resolve(1, 1));
    times.passTimes(resolve(2, 6));
    times.startFrame(2);
    expect(times.take()).toEqual([7]);
  });

  it("gives no entry for a frame that resolved nothing", () => {
    const times = started();
    times.startFrame(0);
    times.startFrame(1);
    times.startFrame(1);
    times.passTimes(resolve(1, 2));
    expect(times.take()).toEqual([2]);
  });

  it("discards a frame with a hole once a later frame completes, and ignores its late report", () => {
    const times = started();
    // Resolve 2 is dropped (a number with no report): the first frame never completes.
    times.startFrame(2);
    times.startFrame(3);
    times.passTimes(resolve(1, 1));
    times.passTimes(resolve(3, 4));
    times.passTimes(resolve(2, 9));
    expect(times.take()).toEqual([4]);
  });

  it("holds at most a bounded number of groups while no report arrives", () => {
    const times = started();
    for (let mark = 1; mark <= PENDING_PRIMARY_FRAMES + 3; mark += 1) {
      times.startFrame(mark);
    }
    // The oldest groups were let go; the newest still completes.
    times.passTimes(resolve(1, 5));
    times.passTimes(resolve(PENDING_PRIMARY_FRAMES + 3, 1));
    expect(times.take()).toEqual([1]);
  });

  it("discards every group at a restore, the new timer counting from 0", () => {
    const times = started();
    times.startFrame(40);
    times.reset(0);
    // The first frame after the restore sets the mark; its group ends at the next.
    times.startFrame(0);
    times.passTimes(resolve(39, 100));
    times.startFrame(2);
    times.passTimes(resolve(1, 1));
    times.passTimes(resolve(2, 1));
    expect(times.take()).toEqual([2]);
  });

  it("starts from its mark, with no group before the first frame", () => {
    const times = new PrimaryFrameTimes(10);
    times.passTimes(resolve(9, 100));
    times.startFrame(10);
    times.startFrame(11);
    times.passTimes(resolve(11, 1));
    expect(times.take()).toEqual([1]);
  });
});

describe("BudgetedScale", () => {
  const [min, max] = SETTINGS.high.internalScaleBounds;

  it("renders a photorealistic primary alone at the bounds' max, with no controller", () => {
    const scale = new BudgetedScale();
    expect([
      scale.update(primaryBudget("photorealistic", 0), [30, 30], 16),
      scale.controller,
    ]).toEqual([max, null]);
  });

  it("makes a controller when an instrument opens beside a photorealistic primary", () => {
    const scale = new BudgetedScale();
    const budget = primaryBudget("photorealistic", 1);
    let s = max;
    // Every frame twice the budget: the controller drops the scale.
    for (let frame = 0; frame < 20; frame += 1) {
      s = scale.update(budget, [2 * (budget.control?.target.gpuBudgetMs ?? 0)], 16.7);
    }
    expect([scale.controller === null, s < max, s >= min]).toEqual([false, true, true]);
  });

  it("keeps its controller's scale when a second instrument opens, and retargets it", () => {
    const scale = new BudgetedScale();
    const one = primaryBudget("photorealistic", 1);
    for (let frame = 0; frame < 20; frame += 1) {
      scale.update(one, [2 * (one.control?.target.gpuBudgetMs ?? 0)], 16.7);
    }
    const controller = scale.controller;
    const before = controller?.scale;
    const after = scale.update(primaryBudget("photorealistic", 2), [], 16.7);
    expect([scale.controller === controller, after]).toEqual([true, before]);
  });

  it("returns to the bounds' max once the instruments close, dropping the controller", () => {
    const scale = new BudgetedScale();
    const budget = primaryBudget("photorealistic", 1);
    for (let frame = 0; frame < 20; frame += 1) {
      scale.update(budget, [2 * (budget.control?.target.gpuBudgetMs ?? 0)], 16.7);
    }
    expect([scale.update(primaryBudget("photorealistic", 0), [], 16.7), scale.controller]).toEqual([
      max,
      null,
    ]);
  });

  it("steps a floor's air to the low setting's sizes as its scale drops, beside instruments never (the owner's report)", () => {
    const floor = viewBudgets(
      [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
      "high",
      1080,
    ).get(PRIMARY);
    const gpuBudgetMs = floor?.control?.target.gpuBudgetMs ?? 0;
    if (floor === undefined || floor.control?.kind !== "floor") {
      throw new Error("the primary alone on high should have its floor");
    }
    const scale = new BudgetedScale();
    scale.update(floor, [0.2 * gpuBudgetMs], 16.7);
    expect(scale.airStepped).toBe(false);
    for (let frame = 0; frame < 20; frame += 1) {
      scale.update(floor, [3 * gpuBudgetMs], 16.7);
    }
    expect(scale.airStepped).toBe(true);
    const beside = new BudgetedScale();
    const one = primaryBudget("photorealistic", 1);
    for (let frame = 0; frame < 20; frame += 1) {
      beside.update(one, [3 * (one.control?.target.gpuBudgetMs ?? 0)], 16.7);
    }
    expect(beside.airStepped).toBe(false);
  });

  it("stands at the floor's lowest step only while its scale is held at its bounds' min, beside instruments never (R07.T8.i)", () => {
    const floor = viewBudgets(
      [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
      "high",
      1080,
    ).get(PRIMARY);
    const gpuBudgetMs = floor?.control?.target.gpuBudgetMs ?? 0;
    if (floor === undefined || floor.control?.kind !== "floor") {
      throw new Error("the primary alone on high should have its floor");
    }
    const lowest = floor.control.bounds[0];
    const scale = new BudgetedScale();
    scale.update(floor, [0.2 * gpuBudgetMs], 16.7);
    const atFirst = scale.atLowestStep;
    let stepped = false;
    let frames = 0;
    while (scale.controller?.scale !== lowest && frames < 2_000) {
      scale.update(floor, [3 * gpuBudgetMs], 16.7);
      stepped ||= scale.atLowestStep && scale.controller?.scale !== lowest;
      frames += 1;
    }
    const atLowest = scale.atLowestStep;
    while (scale.controller?.scale === lowest && frames < 4_000) {
      scale.update(floor, [0.3 * gpuBudgetMs], 16.7);
      frames += 1;
    }
    const beside = new BudgetedScale();
    const one = primaryBudget("photorealistic", 1);
    for (let frame = 0; frame < 200; frame += 1) {
      beside.update(one, [3 * (one.control?.target.gpuBudgetMs ?? 0)], 16.7);
    }
    expect([atFirst, stepped, atLowest, scale.atLowestStep, beside.atLowestStep]).toEqual([
      false,
      false,
      true,
      false,
      false,
    ]);
  });

  it("brings a floor's air back once its scale is at its max and its frames have stayed calm", () => {
    const floor = viewBudgets(
      [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
      "high",
      1080,
    ).get(PRIMARY);
    const gpuBudgetMs = floor?.control?.target.gpuBudgetMs ?? 0;
    if (floor === undefined) {
      throw new Error("no primary budget");
    }
    const scale = new BudgetedScale();
    for (let frame = 0; frame < 20; frame += 1) {
      scale.update(floor, [3 * gpuBudgetMs], 16.7);
    }
    // Frames well under the budget raise the scale back to its max; the air waits for its calm run.
    let frames = 0;
    while (scale.controller?.scale !== max && frames < 2_000) {
      scale.update(floor, [0.3 * gpuBudgetMs], 16.7);
      frames += 1;
    }
    expect([scale.controller?.scale, scale.airStepped]).toEqual([max, true]);
    // The frame that brought the scale back was calm, its run's first; frames at the calm share's
    // edge go on with it, and one over it, a frame short of the run, starts it again.
    for (let frame = 0; frame < CALM_FRAMES - 2; frame += 1) {
      scale.update(floor, [CALM_SHARE * gpuBudgetMs], 16.7);
    }
    scale.update(floor, [0.6 * gpuBudgetMs], 16.7);
    expect(scale.airStepped).toBe(true);
    for (let frame = 0; frame < CALM_FRAMES; frame += 1) {
      scale.update(floor, [0.3 * gpuBudgetMs], 16.7);
    }
    expect(scale.airStepped).toBe(false);
  });

  it("brings a floor's air back by its frames' intervals while the timer is absent", () => {
    const floor = viewBudgets(
      [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
      "high",
      1080,
    ).get(PRIMARY);
    if (floor === undefined || floor.control === null) {
      throw new Error("the primary alone on high should have its floor");
    }
    const periodMs = floor.control.target.periodMs;
    const scale = new BudgetedScale();
    // Frames that miss the floor's period by half again step it down.
    for (let frame = 0; frame < 20; frame += 1) {
      scale.update(floor, undefined, 2 * periodMs);
    }
    expect(scale.airStepped).toBe(true);
    // At a 60 Hz display's every vsync, under 0.6 of the floor's period, the scale climbs back and
    // the air returns after its calm run; at the floor's own 30 Hz it would not.
    let frames = 0;
    while (scale.airStepped && frames < 5_000) {
      scale.update(floor, undefined, 16.7);
      frames += 1;
    }
    expect([scale.airStepped, scale.controller?.scale]).toEqual([false, max]);
    expect(16.7).toBeLessThanOrEqual(CALM_INTERVAL_SHARE * periodMs);
    expect(periodMs).toBeGreaterThan(CALM_INTERVAL_SHARE * periodMs);
  });

  it("gives a wireframe primary its budget's scale, 1", () => {
    const scale = new BudgetedScale();
    expect(scale.update(primaryBudget("wireframe", 2), undefined, 16.7)).toBe(1);
  });

  it("feeds the interval to the controller while the timer is absent", () => {
    const scale = new BudgetedScale();
    const budget = primaryBudget("photorealistic", 1);
    let s = max;
    // Every frame misses its vsync (over 1.5 periods): the interval drops the scale.
    for (let frame = 0; frame < 10; frame += 1) {
      s = scale.update(budget, undefined, 40);
    }
    expect(s < max).toBe(true);
  });
});

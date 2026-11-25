import { describe, expect, it } from "vitest";

import { type RenderStyle, viewId } from "../camera/state";
import type { PassTimes } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import { overlappedPasses, serialPasses } from "../../test/passTimes";
import {
  AIR_TUNING,
  BudgetedScale,
  CALM_INTERVAL_SHARE,
  drawsInFrame,
  FramePacer,
  PENDING_PRIMARY_FRAMES,
  PrimaryFrameTimes,
} from "./framePacing";
import { CONTROLLER_TUNING, gpuTimeMs } from "./resolutionController";
import { SELECTION_SCALE_HOLD_FRAMES, selectionScaleStep } from "./selectionScale";
import { type ViewBudget, viewBudgets, type ViewSpec } from "./viewBudget";

const PRIMARY = viewId("view");

/** One resolve's times: its number and its passes' GPU time, ms each. */
function resolve(frame: number, ...passesMs: ReadonlyArray<number>): PassTimes {
  return {
    frame,
    timer: "quantized",
    passes: serialPasses(passesMs.map((ms, i) => [`pass ${String(i)}`, ms * 1e6] as const)),
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

  it("drops a floor's scale alone on a mild overload, its air kept (lane fix-floor-calm)", () => {
    const floor = floorBudget(1080);
    const [lowest, highest] = floor.control.bounds;
    const budgetMs = floor.control.target.gpuBudgetMs;
    // Frames 5% over the budget at full scale: the controller drops to 0.93 of it.
    const mild = driveFloor(floor, 600, (_, scale) => 1.05 * budgetMs * scale * scale);
    expect([mild.changes, mild.scale < highest && mild.scale > lowest]).toEqual([[], true]);
  });

  it("steps a floor's air in the frame its drop would pass its lowest step (lane fix-floor-calm)", () => {
    const floor = floorBudget(1080);
    const budgetMs = floor.control.target.gpuBudgetMs;
    // Frames three times the budget: the drop aims at 0.55, below the floor's 0.67.
    const heavy = driveFloor(floor, 600, (_, scale) => 3 * budgetMs * scale * scale);
    expect([heavy.changes, heavy.lowestScale]).toEqual([[1], floor.control.bounds[0]]);
  });

  /** A heavy load, its own air the stepped frame again, that lightens after 600 frames. */
  const lightening = (n: number, scale: number, stepped: boolean): number =>
    (n < 600 ? heavyMs(scale) : lightMs(scale)) * (stepped ? 1 : 2);

  it("measures what its step saved, and holds its air stepped while its frames with that cost would not fit at its lowest step (lane fix-floor-calm)", () => {
    const floor = floorBudget(1080);
    const run = driveFloor(floor, 600, lightening);
    // Stepped at the lowest step the heavy frames, 13.3 ms, given the cost, 26.7, stay over 24.
    const heldMs = 2 * heavyMs(floor.control.bounds[0]);
    expect([
      run.changes,
      run.costRatio,
      heldMs > CONTROLLER_TUNING.aim * floor.control.target.gpuBudgetMs,
    ]).toEqual([[1], 1, true]);
  });

  it("brings its air back once its frames with the measured cost would fit at its lowest step (lane fix-floor-calm)", () => {
    const run = driveFloor(floorBudget(1080), 2_400, lightening);
    // Back 67 frames after the load lightens: its run of 60, and the settle of the scale's rise.
    expect([run.changes, run.airStepped, run.scale]).toEqual([
      [1, 600 + AIR_TUNING.returnFrames + 7],
      false,
      1,
    ]);
  });

  it("keeps its air stepped when its floor's bounds change, reading the new controller's frames once they settle (lane fix-floor-calm)", () => {
    const before = floorBudget(1080);
    const after = floorBudget(886);
    const scale = new BudgetedScale();
    let s = scale.update(before, [], 1000 / 60);
    for (let n = 0; n < 600; n += 1) {
      s = scale.update(before, [heavyMs(s) * (scale.airStepped ? 1 : 2)], 1000 / 60);
    }
    const stepped = scale.airStepped;
    // The window's rows change: a new controller at its max, the air as it stood.
    s = scale.update(after, [heavyMs(s)], 1000 / 60);
    const kept = scale.airStepped;
    let frames = 0;
    while (scale.airStepped && frames < 1_000) {
      s = scale.update(after, [lightMs(s)], 1000 / 60);
      frames += 1;
    }
    // The update the bounds changed in was the settle's first.
    expect([stepped, kept, frames]).toEqual([
      true,
      true,
      CONTROLLER_TUNING.settleUpdates - 1 + AIR_TUNING.returnFrames,
    ]);
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

  it("holds the lit selection's scale at the floor's step its scale stands on, taking a new one only after its hold (perf-terrain-selection-scale)", () => {
    const floor = viewBudgets(
      [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
      "high",
      886,
    ).get(PRIMARY);
    if (floor === undefined || floor.control?.kind !== "floor") {
      throw new Error("the primary alone on high should have its floor");
    }
    const { bounds } = floor.control;
    const gpuBudgetMs = floor.control.target.gpuBudgetMs;
    const scale = new BudgetedScale();
    const before = scale.selectionScale;
    scale.update(floor, [0.2 * gpuBudgetMs], 16.7);
    const atMax = scale.selectionScale;
    // Frames far over the budget drop the scale to the floor's lowest step, the M5's 720 of 886.
    const scales: number[] = [];
    const selections: number[] = [];
    let frames = 0;
    while (scale.controller?.scale !== bounds[0] && frames < 2_000) {
      scales.push(scale.update(floor, [3 * gpuBudgetMs], 16.7));
      selections.push(scale.selectionScale);
      frames += 1;
    }
    for (let frame = 0; frame < SELECTION_SCALE_HOLD_FRAMES; frame += 1) {
      scale.update(floor, [0.9 * gpuBudgetMs], 16.7);
    }
    const settled = scale.selectionScale;
    // Never finer than a step its scale stood on, never coarser than its scale's own step.
    const snapped = scales.map((each) => selectionScaleStep(each, bounds));
    expect([before, atMax, settled]).toEqual([1, 1, bounds[0]]);
    expect(selections.every((each, n) => each >= (snapped[n] ?? 0))).toBe(true);
    expect(bounds[0]).toBeCloseTo(720 / 886, 12);
  });

  it("takes the budget's scale as the lit selection's where no control is set", () => {
    const scale = new BudgetedScale();
    scale.update(primaryBudget("wireframe", 0), undefined, 16.7);
    const wireframe = scale.selectionScale;
    const one = primaryBudget("photorealistic", 1);
    for (let frame = 0; frame < 200; frame += 1) {
      scale.update(one, [3 * (one.control?.target.gpuBudgetMs ?? 0)], 16.7);
    }
    const dropped = scale.selectionScale;
    // The instruments closed, with no rows known: no floor, the budget's scale.
    scale.update(primaryBudget("photorealistic", 0), [], 16.7);
    expect([wireframe, dropped < max, scale.selectionScale]).toEqual([1, true, max]);
  });

  it("takes a new control's max as the lit selection's at once, whatever step the old one held", () => {
    const floor = viewBudgets(
      [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
      "high",
      886,
    ).get(PRIMARY);
    if (floor === undefined || floor.control?.kind !== "floor") {
      throw new Error("the primary alone on high should have its floor");
    }
    const gpuBudgetMs = floor.control.target.gpuBudgetMs;
    const scale = new BudgetedScale();
    let frames = 0;
    while (scale.selectionScale !== floor.control.bounds[0] && frames < 2_000) {
      scale.update(floor, [3 * gpuBudgetMs], 16.7);
      frames += 1;
    }
    const held = scale.selectionScale;
    // An instrument opens: the controller's bounds change, and its new controller starts at max.
    const one = primaryBudget("photorealistic", 1);
    scale.update(one, [3 * (one.control?.target.gpuBudgetMs ?? 0)], 16.7);
    expect([held, scale.selectionScale]).toEqual([floor.control.bounds[0], max]);
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

/** A stepped frame of 30 ms at full scale, ms, all pixels; its own air costs as much again. */
function heavyMs(scale: number): number {
  return 30 * scale * scale;
}

/** A stepped frame of 10 ms at full scale: its own air at 1080 rows' lowest step 8.9 ms. */
function lightMs(scale: number): number {
  return 10 * scale * scale;
}

/** The primary alone on high, with its frame-rate floor, at `rows` render rows. */
function floorBudget(
  rows: number,
): ViewBudget & { readonly control: NonNullable<ViewBudget["control"]> } {
  const budget = viewBudgets(
    [{ id: PRIMARY, slot: "primary", style: "photorealistic" }],
    "high",
    rows,
  ).get(PRIMARY);
  const control = budget?.control;
  if (budget === undefined || control?.kind !== "floor") {
    throw new Error("the primary alone on high should have its floor");
  }
  return { ...budget, control };
}

/** The reading before lane fix-mac-gpu-time: every pass's `ns` summed, ms. */
function summedMs(times: ReadonlyArray<PassTimes>): number {
  return times.flatMap((each) => each.passes).reduce((sum, pass) => sum + pass.ns, 0) / 1e6;
}

describe("BudgetedScale on the M5's stamps (lane fix-mac-gpu-time)", () => {
  it("holds a floor's scale at its max and its air unstepped at 60 fps where the passes' union fits and their sum does not", () => {
    const floor = floorBudget(1080);
    const times = new PrimaryFrameTimes(0);
    const scale = new BudgetedScale();
    let stepped = false;
    let lowest = floor.control.bounds[1];
    // 16 ms of GPU work a frame, drawn every 16.7 ms: 28 ms of spans, the composite's holding the
    // 12 ms march, against the floor's 26.7 ms.
    for (let frame = 1; frame <= 600; frame += 1) {
      times.startFrame(frame - 1);
      times.passTimes({
        frame,
        timer: "full",
        passes: overlappedPasses(3_500_000, 12_000_000, 500_000),
      });
      lowest = Math.min(lowest, scale.update(floor, times.take(), 1000 / 60));
      stepped ||= scale.airStepped;
    }
    const passes = overlappedPasses(3_500_000, 12_000_000, 500_000);
    const summed = summedMs([{ frame: 1, timer: "full", passes }]);
    expect([summed > floor.control.target.gpuBudgetMs, lowest, stepped]).toEqual([
      true,
      floor.control.bounds[1],
      false,
    ]);
  });
});

/**
 * The owner's Mac batch of 2026-11-08 (Apple M5, run `high-dpr1`: the high setting at DPR 1, 886
 * render rows, display pacing at 60 Hz), each window's frames as its record gives them: the frame's
 * GPU time as the pass timer's spans summed (its median, p95 and max, ms); of it, the passes
 * encoded before the atmosphere's composite, whose time the composite's span also holds (the
 * `atmosphere view` dispatches and `atmosphere behind`, their medians summed, ms; the composite is
 * them plus about 0.5 ms in all 70 runs); and the composite's median. All were drawn at the
 * floor's lowest step, its air stepped, and every frame was drawn 16.7 ms apart from orbit.
 */
const MAC_BATCH_HIGH_DPR1 = {
  orbit: { medianMs: 24.31, p95Ms: 29.08, maxMs: 29.48, heldMs: 7.42, compositeMs: 7.95 },
  close: { medianMs: 41.96, p95Ms: 44.21, maxMs: 45.69, heldMs: 12.95, compositeMs: 13.49 },
} as const;

type MacWindow = keyof typeof MAC_BATCH_HIGH_DPR1;

/**
 * Frame `n` of a window of the batch, at `scale` of the floor's lowest step `lowest`, as the M5
 * stamps it: its summed time the window's median, but its p95 at every 20th frame from the 5th and
 * its max at every 20th from the 15th; each part grown as the square of the scale, the controller's
 * own model, from the step it was measured at.
 */
function macFrame(window: MacWindow, n: number, scale: number, lowest: number): PassTimes[] {
  const { medianMs, p95Ms, maxMs, heldMs, compositeMs } = MAC_BATCH_HIGH_DPR1[window];
  const summed = n % 20 === 5 ? p95Ms : n % 20 === 15 ? maxMs : medianMs;
  const grown = (scale / lowest) ** 2;
  const ownMs = compositeMs - heldMs;
  const ns = (ms: number): number => Math.round(ms * grown * 1e6);
  return [
    {
      frame: n + 1,
      timer: "full",
      passes: overlappedPasses(ns(summed - 2 * heldMs - ownMs), ns(heldMs), ns(ownMs)),
    },
  ];
}

/** What a replay of the batch's windows ended at, and the least it went to. */
interface Replayed {
  readonly scale: number;
  readonly airStepped: boolean;
  readonly lowestScale: number;
  readonly everStepped: boolean;
}

/** Replays `frames` frames of each of `windows` in turn through one floor, read by `reading`. */
function replayMac(
  windows: ReadonlyArray<MacWindow>,
  reading: (times: ReadonlyArray<PassTimes>) => number,
  frames = 600,
): Replayed[] {
  const floor = floorBudget(886);
  const lowest = floor.control.bounds[0];
  const scale = new BudgetedScale();
  let s = scale.update(floor, [], 1000 / 60);
  return windows.map((window) => {
    let lowestScale = s;
    let everStepped = scale.airStepped;
    for (let n = 0; n < frames; n += 1) {
      s = scale.update(floor, [reading(macFrame(window, n, s, lowest))], 1000 / 60);
      lowestScale = Math.min(lowestScale, s);
      everStepped ||= scale.airStepped;
    }
    return { scale: s, airStepped: scale.airStepped, lowestScale, everStepped };
  });
}

/** What a run of a floor's frames came to: when its air changed, and how it ended. */
interface DrivenFloor {
  /** The frames after which its air changed, counted from 0. */
  readonly changes: ReadonlyArray<number>;
  readonly scale: number;
  readonly lowestScale: number;
  readonly airStepped: boolean;
  readonly costRatio: number;
}

/**
 * Drives `floor` for `frames` frames at 60 Hz, frame `n` costing `costMs(n, scale, stepped)` at the
 * scale and the air it was drawn at, its time handed over at the next update.
 */
function driveFloor(
  floor: ViewBudget,
  frames: number,
  costMs: (n: number, scale: number, stepped: boolean) => number,
): DrivenFloor {
  const scale = new BudgetedScale();
  let s = scale.update(floor, [], 1000 / 60);
  let lowestScale = s;
  let stepped = scale.airStepped;
  const changes: number[] = [];
  for (let n = 0; n < frames; n += 1) {
    s = scale.update(floor, [costMs(n, s, scale.airStepped)], 1000 / 60);
    lowestScale = Math.min(lowestScale, s);
    if (scale.airStepped !== stepped) {
      stepped = scale.airStepped;
      changes.push(n);
    }
  }
  return { changes, scale: s, lowestScale, airStepped: stepped, costRatio: scale.airCostRatio };
}

/** What a replay of the batch's windows, the air given a cost, came to in each window. */
interface ReplayedAir {
  /** The window's frames after which the air changed, counted from its first. */
  readonly airChanges: ReadonlyArray<number>;
  /** The window's frames after which the scale changed. */
  readonly scaleChanges: ReadonlyArray<number>;
  readonly scale: number;
  readonly airStepped: boolean;
  readonly costRatio: number;
}

/**
 * Replays `frames` frames of each of `windows` in turn through one floor, on the union, each frame
 * of the floor's own air `airFactor` times its stepped one (the batch timed only the stepped air).
 */
function replayMacAir(
  windows: ReadonlyArray<MacWindow>,
  airFactor: number,
  frames: number,
): ReplayedAir[] {
  const floor = floorBudget(886);
  const lowest = floor.control.bounds[0];
  const scale = new BudgetedScale();
  let s = scale.update(floor, [], 1000 / 60);
  return windows.map((window) => {
    const airChanges: number[] = [];
    const scaleChanges: number[] = [];
    for (let n = 0; n < frames; n += 1) {
      const stepped = scale.airStepped;
      const before = s;
      const ms = gpuTimeMs(macFrame(window, n, s, lowest)) * (stepped ? 1 : airFactor);
      s = scale.update(floor, [ms], 1000 / 60);
      if (scale.airStepped !== stepped) {
        airChanges.push(n);
      }
      if (s !== before) {
        scaleChanges.push(n);
      }
    }
    return {
      airChanges,
      scaleChanges,
      scale: s,
      airStepped: scale.airStepped,
      costRatio: scale.airCostRatio,
    };
  });
}

/** A small deterministic generator in [0, 1), for a load's jitter. */
function jitter(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    return state / 2 ** 32;
  };
}

/** The most of `changes` within any `spanFrames` frames. */
function mostWithin(changes: ReadonlyArray<number>, spanFrames: number): number {
  return Math.max(
    0,
    ...changes.map((at) => changes.filter((each) => each >= at && each < at + spanFrames).length),
  );
}

describe("the 2026-11-08 Mac batch's high-dpr1 frames, replayed through the floor (lane fix-mac-gpu-time)", () => {
  const floor = floorBudget(886);
  const [lowest, highest] = floor.control.bounds;

  it("step high to the floor's lowest scale and the low setting's air at orbit and closed in on the sum, as the batch found", () => {
    expect([...replayMac(["orbit"], summedMs), ...replayMac(["close"], summedMs)]).toEqual([
      { scale: lowest, airStepped: true, lowestScale: lowest, everStepped: true },
      { scale: lowest, airStepped: true, lowestScale: lowest, everStepped: true },
    ]);
  });

  it("hold high at its full scale and its own air from orbit on the union", () => {
    expect(replayMac(["orbit"], gpuTimeMs)).toEqual([
      { scale: highest, airStepped: false, lowestScale: highest, everStepped: false },
    ]);
  });

  it("still step high closed in on the union, whose frames are over the budget at the lowest step", () => {
    const atLowest = gpuTimeMs(macFrame("close", 0, lowest, lowest));
    expect([atLowest > floor.control.target.gpuBudgetMs, replayMac(["close"], gpuTimeMs)]).toEqual([
      true,
      [{ scale: lowest, airStepped: true, lowestScale: lowest, everStepped: true }],
    ]);
  });

  it("bring high's own air back at orbit after close within 67 frames, the step having measured no cost of the air, and change nothing after (lane fix-floor-calm)", () => {
    const [close, orbit] = replayMacAir(["close", "orbit"], 1, 3_600);
    expect([close, orbit]).toEqual([
      { airChanges: [1], scaleChanges: [1], scale: lowest, airStepped: true, costRatio: 0 },
      // The controller's rise past orbit's lone dear frames to its band at frame 30, the air at 67.
      {
        airChanges: [67],
        scaleChanges: [30],
        scale: orbit?.scale,
        airStepped: false,
        costRatio: 0,
      },
    ]);
    expect(orbit?.scale).toBeCloseTo(0.9687, 4);
  });

  it("hold close stepped at the lowest step while its frames are over the budget there, whatever the air costs (lane fix-floor-calm)", () => {
    const pinned = (airFactor: number): ReadonlyArray<Pick<ReplayedAir, "airChanges" | "scale">> =>
      replayMacAir(["close"], airFactor, 3_600).map(({ airChanges, scale }) => ({
        airChanges,
        scale,
      }));
    expect([pinned(1), pinned(2)]).toEqual([
      [{ airChanges: [1], scale: lowest }],
      [{ airChanges: [1], scale: lowest }],
    ]);
  });

  it("hold orbit stepped from launch and after close where the air costs a stepped frame again, as R08's W5 puts it, its scale in the controller's band (lane fix-floor-calm)", () => {
    const [fresh] = replayMacAir(["orbit"], 2, 3_600);
    const [, after] = replayMacAir(["close", "orbit"], 2, 3_600);
    expect([fresh?.airChanges, fresh?.airStepped, after?.airChanges, after?.airStepped]).toEqual([
      [1],
      true,
      [],
      true,
    ]);
    expect([fresh?.scale, after?.scale].map((s) => Number(s?.toFixed(4)))).toEqual([
      0.9687, 0.9687,
    ]);
  });

  it("wait twice as long to bring orbit's air back when close stepped it within 30 s of its last return (lane fix-floor-calm)", () => {
    const replayed = replayMacAir(["close", "orbit", "close", "orbit"], 1, 1_200);
    expect(replayed.map(({ airChanges }) => airChanges)).toEqual([
      [1],
      [AIR_TUNING.returnFrames + 7],
      [1],
      [2 * AIR_TUNING.returnFrames + 7],
    ]);
  });
});

describe("the floor's air at its boundary (lane fix-floor-calm)", () => {
  const floor = floorBudget(886);
  const lowest = floor.control.bounds[0];
  const budgetMs = floor.control.target.gpuBudgetMs;
  /** 20 minutes at 60 Hz, and 30 s, the guard's watch. */
  const TWENTY_MINUTES_FRAMES = 72_000;
  const WATCH = AIR_TUNING.returnWatchUpdates;
  /**
   * A load whose stepped frame is `fixedMs` and pixels, half the budget at the lowest step, and
   * whose own air as much again in pixels, so that its own frame there is the budget, each ±5%;
   * every tenth frame `dearFactor` dearer.
   */
  const boundary = (
    fixedMs: number,
    dearFactor = 1,
  ): ((n: number, scale: number, stepped: boolean) => number) => {
    const next = jitter(7);
    const pixelMs = (budgetMs / 2 - fixedMs) / lowest ** 2;
    const airMs = budgetMs / 2 / lowest ** 2;
    return (n, scale, stepped) =>
      (fixedMs + (pixelMs + (stepped ? 0 : airMs)) * scale * scale) *
      (0.95 + 0.1 * next()) *
      (n % 10 === 5 ? dearFactor : 1);
  };

  it("changes the air of a load at the boundary at most once in any 30 s", () => {
    const run = driveFloor(floor, TWENTY_MINUTES_FRAMES, boundary(0));
    expect([run.changes, mostWithin(run.changes, WATCH), run.airStepped]).toEqual([[1], 1, true]);
  });

  it("tries a return a fixed cost misjudged once at its scale, and changes nothing after it", () => {
    // 10 ms of it fixed: carried to the lowest step by the square of the scale, it reads 3.4 ms
    // under, and the first return fails on the frames it was judged on.
    const run = driveFloor(floor, TWENTY_MINUTES_FRAMES, boundary(10));
    const [step, back, again] = run.changes;
    expect([
      run.changes.length,
      step,
      (again ?? 0) - (back ?? 0) <= AIR_TUNING.quickFailUpdates,
      run.airStepped,
    ]).toEqual([3, 1, true, true]);
  });

  it("holds back the return a fixed cost misjudged through the load's lone dear frames", () => {
    // Every tenth frame 15% dearer, over the hold's band but alone in four.
    const run = driveFloor(floor, TWENTY_MINUTES_FRAMES, boundary(10, 1.15));
    expect([run.changes.length, mostWithin(run.changes.slice(2), WATCH), run.airStepped]).toEqual([
      3,
      1,
      true,
    ]);
  });
});

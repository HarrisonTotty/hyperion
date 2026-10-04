import { describe, expect, it } from "vitest";

import { type RenderStyle, viewId } from "../camera/state";
import type { PassTimes } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import {
  BudgetedScale,
  drawsInFrame,
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

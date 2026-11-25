import { describe, expect, it } from "vitest";

import { AIR_TUNING, FloorAir } from "./floorAir";
import { CONTROLLER_TUNING, type ResolutionTarget, type ScaleBounds } from "./resolutionController";

/** A floor from half scale to full, held to 30 Hz's budget of 26.7 ms. */
const BOUNDS: ScaleBounds = [0.5, 1];
const TARGET: ResolutionTarget = { periodMs: 1000 / 30, gpuBudgetMs: 0.8 * (1000 / 30) };

/** One update of `air` with one frame of `ms`, at `scale`; whether the air changed. */
function feed(air: FloorAir, ms: number, scale: number, wantedBelowMin = false): boolean {
  return air.update({
    framesMs: [ms],
    intervalMs: 1000 / 60,
    scale,
    wantedBelowMin,
    bounds: BOUNDS,
    target: TARGET,
  });
}

/** Feeds `count` updates of `ms` at `scale`. */
function feedMany(air: FloorAir, count: number, ms: number, scale: number): void {
  for (let n = 0; n < count; n += 1) {
    feed(air, ms, scale);
  }
}

/**
 * Steps `air` at the lowest step on two own frames of `ownMs` over the budget, then feeds `after`
 * stepped frames there: the step's measure, their medians' ratio less 1, once 30 are read.
 */
function stepAt(air: FloorAir, ownMs: number, after: ReadonlyArray<number>): void {
  feed(air, ownMs, BOUNDS[0]);
  feed(air, ownMs, BOUNDS[0], true);
  for (const ms of after) {
    feed(air, ms, BOUNDS[0]);
  }
}

/** The frames of stepped `ms` at `scale` until the air returns, at most `most`. */
function framesToReturn(air: FloorAir, ms: number, scale: number, most = 10_000): number {
  let frames = 0;
  while (air.stepped && frames < most) {
    feed(air, ms, scale);
    frames += 1;
  }
  return frames;
}

/** The stepped frames a measure reads: the settle, then its window's most. */
const AFTER = CONTROLLER_TUNING.settleUpdates + AIR_TUNING.afterFramesMax;

/**
 * The frames over the line that the drop rule's window of four still holds as lighter ones come,
 * before a run can start: two.
 */
const WINDOW_LAG = CONTROLLER_TUNING.dropWindow - CONTROLLER_TUNING.dropCount;

describe("a floor's air under the pass timer (lane fix-floor-calm)", () => {
  it("takes the air's cost from the step's own and stepped frames at the lowest step", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    expect([air.stepped, air.costRatio]).toEqual([true, 1]);
  });

  it("caps the air's measured cost at its most", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 4),
    );
    expect(air.costRatio).toBe(AIR_TUNING.maxCostRatio);
  });

  it("keeps the air's cost where the step's stepped frames spread over their bound", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, (_, n) => (n % 2 === 0 ? 10 : 16)),
    );
    expect(air.costRatio).toBe(AIR_TUNING.unmeasuredCostRatio);
  });

  it("keeps the air's cost where the frames that drove the step spread over their bound", () => {
    const air = new FloorAir();
    feed(air, 28, BOUNDS[0]);
    feed(air, 50, BOUNDS[0], true);
    feedMany(air, AFTER, 7, BOUNDS[0]);
    expect(air.costRatio).toBe(AIR_TUNING.unmeasuredCostRatio);
  });

  it("brings the air back after its run of frames that would fit, given its cost", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    // 14 ms given a share of 1 is 28, over 0.9 of the budget; 8 ms is 16, under it.
    const held = framesToReturn(air, 14, BOUNDS[0], 1_000);
    expect([held, air.stepped, framesToReturn(air, 8, BOUNDS[0])]).toEqual([
      1_000,
      true,
      AIR_TUNING.returnFrames + WINDOW_LAG,
    ]);
  });

  it("doubles the wait after a return that fails within its watch, and resets it once one holds", () => {
    const air = new FloorAir();
    const measured = Array.from({ length: AFTER }, () => 14);
    stepAt(air, 28, measured);
    const first = framesToReturn(air, 8, BOUNDS[0]);
    // Its own frames fit for a while, then go over: a failure, 100 updates after the return.
    feedMany(air, 100, 16, BOUNDS[0]);
    stepAt(air, 28, measured);
    const second = framesToReturn(air, 8, BOUNDS[0]);
    // This one holds through its watch: the next step is no failure, and the wait is the first.
    feedMany(air, AIR_TUNING.returnWatchUpdates + 1, 16, BOUNDS[0]);
    stepAt(air, 28, measured);
    const third = framesToReturn(air, 8, BOUNDS[0]);
    expect([first, second, third]).toEqual([
      AIR_TUNING.returnFrames + WINDOW_LAG,
      2 * AIR_TUNING.returnFrames + WINDOW_LAG,
      AIR_TUNING.returnFrames + WINDOW_LAG,
    ]);
  });

  describe("after a return that failed on its own frames", () => {
    /** A floor whose return at full scale, on stepped frames of 20 ms, failed 2 updates after it. */
    function failedQuickly(): FloorAir {
      const air = new FloorAir();
      feed(air, 28, 1);
      feed(air, 28, 1, true);
      // Read at full scale, 20 ms carried to the lowest step and given a share of 1 is 10 ms.
      framesToReturn(air, 20, 1);
      feed(air, 28, 1);
      feed(air, 28, 1, true);
      return air;
    }

    it("holds the air stepped while the stepped frames at that scale stay near those it was judged on", () => {
      const air = failedQuickly();
      expect([air.stepped, framesToReturn(air, 20, 1, 2_000)]).toEqual([true, 2_000]);
    });

    it("holds it through lone heavier frames", () => {
      const air = failedQuickly();
      let frames = 0;
      while (air.stepped && frames < 2_000) {
        feed(air, frames % 10 === 5 ? 23 : 20, 1);
        frames += 1;
      }
      expect(frames).toBe(2_000);
    });

    it("judges afresh once the frames there are heavier, two in four, and brings the air back after the doubled wait", () => {
      const air = failedQuickly();
      // The settle, then two heavier frames to lift the hold, then the doubled run.
      expect(framesToReturn(air, 23, 1)).toBe(
        CONTROLLER_TUNING.settleUpdates + 2 * AIR_TUNING.returnFrames,
      );
    });

    it("judges frames at another scale as before", () => {
      const air = failedQuickly();
      // The update the scale changes in, its settle, then the doubled run.
      expect(framesToReturn(air, 20, 0.9)).toBe(
        1 + CONTROLLER_TUNING.settleUpdates + 2 * AIR_TUNING.returnFrames,
      );
    });
  });

  it("returns in no update whose scale changes, but once it has read frames at the new scale", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    feedMany(air, AIR_TUNING.returnFrames - 1 + WINDOW_LAG, 8, BOUNDS[0]);
    // The run's last frame arrives as the scale rises: the frames after it were drawn at 0.6.
    const atChange = feed(air, 8, 0.6);
    let frames = 0;
    while (air.stepped && frames < 100) {
      feed(air, 8, 0.6);
      frames += 1;
    }
    expect([atChange, frames]).toEqual([false, CONTROLLER_TUNING.settleUpdates + 1]);
  });

  it("starts its windows again when the timer comes back, its frames before the gap counted for nothing", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    feedMany(air, 35, 8, 0.6);
    for (let n = 0; n < 100; n += 1) {
      air.update({
        framesMs: undefined,
        intervalMs: 1000 / 30,
        scale: 0.6,
        wantedBelowMin: false,
        bounds: BOUNDS,
        target: TARGET,
      });
    }
    expect([air.stepped, framesToReturn(air, 8, 0.6)]).toEqual([
      true,
      CONTROLLER_TUNING.settleUpdates + AIR_TUNING.returnFrames,
    ]);
  });
});

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

describe("a floor's air by the middle step (lane perf-air-mac)", () => {
  /** One update of `air` by the middle step with one frame of `ms`, at `scale`. */
  function feedMiddle(air: FloorAir, ms: number, scale: number, wantedBelowMin = false): boolean {
    return air.update(
      {
        framesMs: [ms],
        intervalMs: 1000 / 60,
        scale,
        wantedBelowMin,
        bounds: BOUNDS,
        target: TARGET,
      },
      "middle",
    );
  }

  /**
   * Steps `air` at the lowest scale by the middle step on two frames of `aboveMs` over the budget,
   * then feeds `after` frames at the step below.
   */
  function stepMiddleAt(air: FloorAir, aboveMs: number, after: ReadonlyArray<number>): void {
    feedMiddle(air, aboveMs, BOUNDS[0]);
    feedMiddle(air, aboveMs, BOUNDS[0], true);
    for (const ms of after) {
      feedMiddle(air, ms, BOUNDS[0]);
    }
  }

  /** The frames of `ms` at `scale` until the air's step changes from `from`, at most `most`. */
  function framesToChange(air: FloorAir, ms: number, scale: number, most = 10_000): number {
    const from = air.step;
    let frames = 0;
    while (air.step === from && frames < most) {
      feedMiddle(air, ms, scale);
      frames += 1;
    }
    return frames;
  }

  it("steps high's own air to the middle step first, and to low's only on a drop below the lowest scale from there", () => {
    const air = new FloorAir();
    stepMiddleAt(
      air,
      40,
      Array.from({ length: AFTER }, () => 20),
    );
    const atMiddle = air.step;
    stepMiddleAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    expect([atMiddle, air.step, air.stepped]).toEqual(["middle", "low", true]);
  });

  it("measures each step's share from its own pair", () => {
    const air = new FloorAir();
    stepMiddleAt(
      air,
      40,
      Array.from({ length: AFTER }, () => 20),
    );
    stepMiddleAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    expect(air.costRatios).toEqual([1, 1]);
  });

  it("takes its priors until a pair has measured a step, their product about the floor's own prior", () => {
    const air = new FloorAir();
    feedMiddle(air, 10, 1);
    const [own, middle] = AIR_TUNING.unmeasuredMiddleCostRatios;
    expect(air.costRatios).toEqual(AIR_TUNING.unmeasuredMiddleCostRatios);
    expect((1 + own) * (1 + middle) - 1).toBeCloseTo(AIR_TUNING.unmeasuredCostRatio, 1);
  });

  it("takes no second step on the frames drawn before the first took effect", () => {
    const air = new FloorAir();
    feedMiddle(air, 40, BOUNDS[0]);
    feedMiddle(air, 40, BOUNDS[0], true);
    // The controller's drop still aims below the lowest scale on the own air's late frames.
    for (let n = 0; n < CONTROLLER_TUNING.settleUpdates; n += 1) {
      feedMiddle(air, 40, BOUNDS[0], true);
    }
    expect(air.step).toBe("middle");
  });

  it("brings the air back a step at a time, each by its own step's share: to the middle step where it would fit, no further while high's own would not", () => {
    const air = new FloorAir();
    stepMiddleAt(
      air,
      40,
      Array.from({ length: AFTER }, () => 20),
    );
    stepMiddleAt(
      air,
      30,
      Array.from({ length: AFTER }, () => 20),
    );
    // Low's frames of 20 ms given the middle step's share of 0.5 are 30, over 0.9 of the budget;
    // of 12 ms, 18, under it.
    const heldAtLow = framesToChange(air, 20, BOUNDS[0], 2_000);
    const toMiddle = framesToChange(air, 12, BOUNDS[0]);
    const atMiddle = air.step;
    // The middle step's frames of 20 ms given high's own share of 1 are 40, over it.
    const held = framesToChange(air, 20, BOUNDS[0], 2_000);
    expect([air.costRatios, heldAtLow, toMiddle, atMiddle, held, air.step]).toEqual([
      [1, 0.5],
      2_000,
      AIR_TUNING.returnFrames + WINDOW_LAG,
      "middle",
      2_000,
      "middle",
    ]);
  });

  it("brings high's own air back from the middle step once its frames with that step's share would fit", () => {
    const air = new FloorAir();
    stepMiddleAt(
      air,
      40,
      Array.from({ length: AFTER }, () => 20),
    );
    // 10 ms given high's own share of 1 is 20, under 0.9 of the budget.
    expect([framesToChange(air, 10, BOUNDS[0]), air.step, air.stepped]).toEqual([
      AIR_TUNING.returnFrames + WINDOW_LAG,
      "own",
      false,
    ]);
  });

  it("holds a return that failed on its own frames at the step it was tried from alone", () => {
    const air = new FloorAir();
    stepMiddleAt(
      air,
      40,
      Array.from({ length: AFTER }, () => 20),
    );
    // Back to high's own on 10 ms frames, which fail at once: the hold is the middle step's.
    framesToChange(air, 10, BOUNDS[0]);
    feedMiddle(air, 40, BOUNDS[0]);
    feedMiddle(air, 40, BOUNDS[0], true);
    const heldAtMiddle = framesToChange(air, 10, BOUNDS[0], 2_000);
    expect([air.step, heldAtMiddle]).toEqual(["middle", 2_000]);
  });

  it("steps straight to low's sizes without a timer, as the floor's own does", () => {
    const air = new FloorAir();
    air.update(
      {
        framesMs: undefined,
        intervalMs: 1000 / 30,
        scale: 0.6,
        wantedBelowMin: false,
        bounds: BOUNDS,
        target: TARGET,
      },
      "middle",
    );
    expect(air.step).toBe("low");
  });

  it("starts again at high's own air with its priors when its kind changes", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 4),
    );
    feedMiddle(air, 10, 1);
    expect([air.step, air.costRatios]).toEqual(["own", AIR_TUNING.unmeasuredMiddleCostRatios]);
  });

  it("keeps the floor's own two steps under `low`, its default", () => {
    const air = new FloorAir();
    stepAt(
      air,
      28,
      Array.from({ length: AFTER }, () => 14),
    );
    feed(air, 28, BOUNDS[0], true);
    expect([air.step, air.costRatios]).toEqual(["low", [1]]);
  });
});

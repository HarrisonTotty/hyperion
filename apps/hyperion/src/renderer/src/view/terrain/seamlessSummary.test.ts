import { describe, expect, it } from "vitest";

import {
  isCollapse,
  isDrawnHalving,
  isFlicker,
  isPop,
  percentile,
  type SeamEvent,
  type SeamFrame,
  type SeamRun,
  summariseSeamRun,
} from "./seamlessSummary";

function frameAt(tS: number, drawn: number, more: Partial<SeamFrame> = {}): SeamFrame {
  return {
    tS,
    drawn,
    standingIn: 0,
    missing: 0,
    maxGap: 0,
    limitExcess: 0,
    selected: drawn,
    ...more,
  };
}

function eventAt(tS: number, more: Partial<SeamEvent> = {}): SeamEvent {
  return {
    tS,
    kind: "refine",
    fineLevel: 10,
    levelJump: 1,
    maxPx: 0.5,
    levelPx: 0.5,
    maxNormalDeg: 1,
    minFineK: 1,
    standIn: false,
    ...more,
  };
}

function runOf(frames: SeamFrame[], events: SeamEvent[], more: Partial<SeamRun> = {}): SeamRun {
  return {
    costMs: 41,
    workers: 3,
    setting: "high",
    fromS: 0,
    toS: 10,
    handOverS: 0,
    firstDrawS: 0,
    bakesAnswered: 0,
    refusedBakes: 0,
    selectedEvicted: 0,
    latenciesS: [],
    frames,
    events,
    collapses: [],
    frameChanges: [],
    pops: [],
    ...more,
  };
}

describe("the seamless probe's summary", () => {
  it("takes a percentile by nearest rank, and none of nothing", () => {
    expect(percentile([5, 1, 4, 2, 3], 50)).toBe(3);
    expect(percentile([5, 1, 4, 2, 3], 95)).toBe(5);
    expect(percentile([], 50)).toBeNull();
  });

  it("calls a change a pop over 1 px or 2°, and not at them", () => {
    expect(isPop(eventAt(0, { maxPx: 1, maxNormalDeg: 2 }))).toBe(false);
    expect(isPop(eventAt(0, { maxPx: 1.01 }))).toBe(true);
    expect(isPop(eventAt(0, { maxNormalDeg: 2.01 }))).toBe(true);
  });

  it("calls a frame a collapse below half of more than 20 drawn patches", () => {
    expect(isCollapse(frameAt(0, 122), frameAt(0.1, 2, { selected: 122 }))).toBe(true);
    expect(isCollapse(frameAt(0, 122), frameAt(0.1, 61))).toBe(false);
    expect(isCollapse(frameAt(0, 20), frameAt(0.1, 2, { selected: 20 }))).toBe(false);
  });

  it("calls a drawn set halved with its selection no collapse, but a halving", () => {
    // The survey leaving the view: 38 patches selected and drawn, then 15.
    const [before, after] = [frameAt(0, 38), frameAt(0.1, 15)];
    expect([isDrawnHalving(before, after), isCollapse(before, after)]).toEqual([true, false]);
  });

  it("calls a selection that halves or doubles and comes back within two frames a flicker", () => {
    const sizes = (...selected: number[]): SeamFrame[] => selected.map((n, at) => frameAt(at, n));
    expect([
      isFlicker(sizes(76, 187, 79), 1),
      isFlicker(sizes(300, 0, 0, 310), 1),
      isFlicker(sizes(38, 15, 14, 12), 1),
      isFlicker(sizes(10, 0, 10), 1),
    ]).toEqual([true, true, false, false]);
  });

  it("counts the collapses, the jumps, the coarsens' largest move and the pops", () => {
    const frames = [
      frameAt(0, 100),
      frameAt(1, 40, { standingIn: 3, selected: 100 }),
      frameAt(2, 100),
    ];
    const events = [
      eventAt(0.5, { kind: "coarsen", levelJump: 3, maxPx: 12, levelPx: 4 }),
      eventAt(1, { kind: "refine", levelJump: 1, maxPx: 3, minFineK: 0.2 }),
      eventAt(1.5, { kind: "appear", levelJump: 0, maxPx: -1, levelPx: -1, maxNormalDeg: -1 }),
      eventAt(2, { kind: "rebake", levelJump: 0, maxNormalDeg: 30 }),
    ];
    const summary = summariseSeamRun(runOf(frames, events, { latenciesS: [1, 2, 3] }));
    expect(summary).toMatchObject({
      spanS: 2,
      frames: 3,
      collapses: 1,
      drawnHalvings: 1,
      flickers: 0,
      jumps: 1,
      jumpMaxPx: 12,
      largestCoarsenPx: 12,
      largestCoarsenLevelPx: 4,
      popsPerS: 1.5,
      refinesBelowHalfK: 1,
      demandToResidentS: { p50: 2, p95: 3, max: 3, count: 3 },
      normalJumpDeg: { max: 30 },
    });
    expect(summary.standInFrames).toBeCloseTo(1 / 3, 12);
  });

  it("takes the frames' largest move and turn, their pops a second and the appearances", () => {
    const frameChanges = [
      { tS: 1, movePx: 0.5, normalDeg: 1, pops: 0, appears: 0, appearMovePx: 0 },
      { tS: 2, movePx: 3, normalDeg: 0.5, pops: 2, appears: 1, appearMovePx: 7 },
    ];
    const pop = {
      tS: 2,
      areaLevel: 9,
      level: 9,
      levelBefore: 8,
      movePx: 3,
      normalDeg: 0.5,
      ownFade: 0.5,
      rebakeFade: 0,
      k: 0.5,
      kBefore: 0,
    };
    const pops = [
      { ...pop, cause: "refine" as const },
      { ...pop, cause: "refine" as const },
    ];
    const summary = summariseSeamRun(
      runOf([frameAt(0, 10), frameAt(4, 10)], [], { frameChanges, pops }),
    );
    expect(summary).toMatchObject({
      frameMovePxMax: 3,
      frameNormalDeg: { max: 1 },
      framePopsPerS: 0.5,
      framePopsByCause: { fade: 0, rebake: 0, refine: 2, coarsen: 0, appear: 0, survey: 0 },
      appearsWithoutTransition: 1,
      appearMovePxMax: 7,
    });
  });

  it("measures the bands' steps of τ′ with the excess floored at 1", () => {
    const frames = [
      frameAt(0, 10, { limitExcess: 0 }),
      frameAt(1, 10, { limitExcess: 0.5 }),
      frameAt(2, 10, { limitExcess: 1.02 }),
      frameAt(3, 10, { limitExcess: 2.04 }),
    ];
    const { bandSteps } = summariseSeamRun(runOf(frames, []));
    expect(bandSteps).toMatchObject({ count: 2, over5Percent: 1 });
    expect(bandSteps.max).toBeCloseTo(1, 12);
  });
});

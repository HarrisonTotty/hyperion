import { describe, expect, it } from "vitest";

import { goldenLevelTable } from "../../test/terrainFixtures";
import { levelHeightRangeM, planetGeometry } from "../terrain/planet";
import {
  fixtureRangeOf,
  formatRanges,
  memoised,
  parseRanges,
  recordProfile,
  SETTING_VIEWS,
} from "./demandRecord";
import { vec3 } from "../../geometry/vec3";
import { selectionOf, UNIT_BOUNDS } from "../../test/terrainFixtures";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { PatchBounds } from "../terrain/bounds";
import { DrawSetResolver, PatchCache } from "../terrain/cache";
import { finestPatchSizeM } from "../terrain/grounded";
import { childKeys, type PatchKey, patchKeyString, rootKey } from "../terrain/patchKey";
import {
  type Selection,
  screenSpaceErrorPx,
  selectionErrorM,
  type ViewSelectionInput,
} from "../terrain/select";
import { type SlotLayout, slotLayout } from "../terrain/slotLayout";
import { RESELECT_FRACTION, selectionTolerancePx } from "../terrain/selectionTolerance";
import { perLevelDemand } from "./demand";
import { SPIKE_CRAFT_RADIUS_M } from "./spikeScene";
import {
  coarseStandIns,
  craftContacts,
  effectiveTolerancePx,
  type FixedStepFrame,
  type FixedStepOptions,
  runFixedStep,
  segmentFigures,
} from "./fixedStep";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

const PLANET = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
const HIGH = SETTING_VIEWS[0];
/** The high setting's τ_sel, as the terrain pass selects: τ ÷ (1 + RESELECT_FRACTION). */
const HIGH_SELECTION_TAU_PX = (HIGH?.view.tauPx ?? NaN) / (1 + RESELECT_FRACTION);

/** A cache layout of `slots` slots of 100 B each. */
function tinyLayout(slots: number): SlotLayout {
  return slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], slots * 100);
}

/** A short orbit-coast run, cheap to select, with the level's own range for every bake. */
function orbitRun(overrides: Partial<FixedStepOptions> = {}): ReturnType<typeof runFixedStep> {
  if (HIGH === undefined) {
    throw new Error("no high setting");
  }
  return runFixedStep({
    profile: recordProfile(),
    planet: PLANET,
    setting: "high",
    view: HIGH.view,
    rateHz: 8,
    fromS: 10,
    toS: 11,
    measureFromS: 10.5,
    maxPatches: HIGH.maxPatches,
    layout: HIGH.layout,
    rangeOf: (key) => levelHeightRangeM(PLANET, key.level),
    nowMs: () => 0,
    ...overrides,
  });
}

describe("the fixed-step run", () => {
  it("samples the span at its rate, marking the warm-up", () => {
    const run = orbitRun();
    expect(run.frames.map(({ tS }) => tS)).toEqual([
      10, 10.125, 10.25, 10.375, 10.5, 10.625, 10.75, 10.875, 11,
    ]);
    expect(run.frames.filter(({ warmup }) => warmup)).toHaveLength(4);
    expect(run.truncated).toBe(false);
  });

  it("bakes the selection's requests into the cache, the roots first, until none is missing", () => {
    const run = orbitRun({ fromS: 10, toS: 12, rateHz: 16 });
    const [first] = run.frames;
    // With nothing baked the selection is the visible roots, all requested and baked at once.
    expect(first?.demanded).toBe(first?.patches);
    expect(first?.patches).toBeLessThanOrEqual(6);
    const last = run.frames.at(-1);
    expect(last?.missing).toBe(0);
    const total = run.frames.reduce((sum, f) => sum + f.demanded, 0);
    expect(total).toBeLessThanOrEqual(HIGH?.layout.slotCount ?? 0);
  });

  it("is identical run to run, its hash included", () => {
    expect(orbitRun().hash).toBe(orbitRun().hash);
    expect(orbitRun().hash).toMatch(/^[0-9a-f]{16}$/);
  });

  it("stops at the wall-time deadline and says so", () => {
    let clock = 0;
    const run = orbitRun({
      nowMs: () => {
        clock += 1;
        return clock;
      },
      deadlineMs: 6,
    });
    expect(run.truncated).toBe(true);
    expect(run.frames.length).toBeLessThan(9);
  });

  it("stops at the wall deadline however long the bakes took", () => {
    let clock = 0;
    const run = orbitRun({
      nowMs: () => clock,
      rangeOf: (key) => {
        clock += 10;
        return levelHeightRangeM(PLANET, key.level);
      },
      deadlineMs: Infinity,
      wallDeadlineMs: 15,
    });
    expect(run.truncated).toBe(true);
    expect(run.frames.length).toBeLessThan(9);
  });

  it("times each selection on the thread's CPU clock too, where one is given", () => {
    let cpuMs = 0;
    const run = orbitRun({
      cpuNowMs: () => {
        cpuMs += 3;
        return cpuMs;
      },
    });
    expect(run.frames.map(({ selectCpuMs }) => selectCpuMs)).toEqual(run.frames.map(() => 3));
  });

  it("leaves the selection's CPU time null without a CPU clock", () => {
    expect(orbitRun().frames.every(({ selectCpuMs }) => selectCpuMs === null)).toBe(true);
  });

  it("figures a segment over its measured frames alone", () => {
    const run = orbitRun();
    const [coast] = segmentFigures(run, 8);
    expect(coast?.segment).toBe("orbit coast");
    expect(coast?.frames).toBe(5);
    const measured = run.frames.filter(({ warmup }) => !warmup);
    expect(coast?.demandPerS).toBeCloseTo(
      measured.reduce((sum, f) => sum + f.demanded, 0) / (5 / 8),
      9,
    );
  });
});

/** A measured frame of the orbit coast with the given budget figures. */
function frameOf(overrides: Partial<FixedStepFrame>): FixedStepFrame {
  return {
    tS: 0,
    warmup: false,
    segment: "orbit coast",
    patches: 10,
    demanded: 0,
    limited: false,
    tauPrimePx: 1,
    standingIn: 0,
    missing: 0,
    coarseStandIns: 0,
    coarseReturns: 0,
    coarseStandInRhoPx: 0,
    coarseReturnRhoPx: 0,
    forcedDemanded: 0,
    selectMs: 0,
    selectCpuMs: null,
    predictedPerS: 0,
    heightAboveFloorM: 400_000,
    ...overrides,
  };
}

describe("the budget's figures (decision-r05-high-bound.md, F4)", () => {
  it("takes τ′ as τ unlimited, and τ × max(1, limitExcess ÷ w) under the budget", () => {
    expect(effectiveTolerancePx({ limitExcess: 0 }, { tauPx: 1, weight: 1 })).toBe(1);
    expect(effectiveTolerancePx({ limitExcess: 2.5 }, { tauPx: 2, weight: 1 })).toBe(5);
    // A secondary view's own excess is limitExcess ÷ w (R05.T7 as built).
    expect(effectiveTolerancePx({ limitExcess: 0.9 }, { tauPx: 4, weight: 0.25 })).toBeCloseTo(
      14.4,
      12,
    );
    expect(effectiveTolerancePx({ limitExcess: 0.2 }, { tauPx: 4, weight: 0.25 })).toBe(4);
  });

  it("selects at the terrain pass's τ_sel, and says so beside the setting's τ", () => {
    const run = orbitRun();
    expect([run.tauPx, run.selectionTauPx]).toEqual([HIGH?.view.tauPx, HIGH_SELECTION_TAU_PX]);
  });

  it("records τ′ above τ_sel on the frames a binding budget limits, and τ_sel on the others", () => {
    const run = orbitRun({ maxPatches: 40, toS: 12, rateHz: 16 });
    const limited = run.frames.filter((f) => f.limited);
    expect(limited.length).toBeGreaterThan(0);
    expect(limited.every((f) => f.tauPrimePx > HIGH_SELECTION_TAU_PX)).toBe(true);
    expect(
      run.frames.filter((f) => !f.limited).every((f) => f.tauPrimePx === HIGH_SELECTION_TAU_PX),
    ).toBe(true);
    const unbudgeted = orbitRun({ maxPatches: undefined, toS: 12, rateHz: 16 });
    expect(
      unbudgeted.frames.every((f) => !f.limited && f.tauPrimePx === HIGH_SELECTION_TAU_PX),
    ).toBe(true);
  });

  it("selects the low setting at its τ_sel too, which its unlimited frames take as τ′", () => {
    const low = SETTING_VIEWS[1];
    if (low === undefined) {
      throw new Error("no low setting");
    }
    const run = orbitRun({
      setting: "low",
      view: low.view,
      maxPatches: low.maxPatches,
      layout: low.layout,
    });
    expect(run.frames.map(({ limited, tauPrimePx }) => [limited, tauPrimePx])).toEqual(
      run.frames.map(() => [false, low.view.tauPx / (1 + RESELECT_FRACTION)]),
    );
  });

  it("predicts D at τ_sel, the tolerance the frame selected at", () => {
    if (HIGH === undefined) {
      throw new Error("no high setting");
    }
    const profile = recordProfile();
    const view = { ...HIGH.view, tauPx: selectionTolerancePx(HIGH.view.tauPx) };
    const run = orbitRun();
    expect(run.frames.map(({ predictedPerS }) => predictedPerS)).toEqual(
      run.frames.map(({ tS }) => {
        const pose = profile.poseAt(tS);
        return perLevelDemand(PLANET, { ...pose, altitudeM: pose.heightAboveFloorM }, view).perS;
      }),
    );
  });

  it("counts no stand-in on the first frame, whose unbaked roots have nothing to stand in", () => {
    const [first] = orbitRun({ toS: 12, rateHz: 16 }).frames;
    expect([first?.coarseStandIns, (first?.missing ?? 0) > 0]).toEqual([0, true]);
  });

  it("counts the roots standing in for their unbaked children on the second frame", () => {
    const second = orbitRun({ toS: 12, rateHz: 16 }).frames[1];
    expect(second?.coarseStandIns).toBeGreaterThan(0);
    expect(second?.coarseStandInRhoPx).toBeGreaterThan(1);
  });

  it("counts no return while the cache holds everything it baked", () => {
    const run = orbitRun({ toS: 12, rateHz: 16 });
    expect(run.frames.every((f) => f.coarseReturns === 0 && f.coarseReturnRhoPx === 0)).toBe(true);
  });

  it("counts returns where a cache too small for the selection evicts coarse patches it needs", () => {
    const run = orbitRun({ toS: 12, rateHz: 16, maxPatches: undefined, layout: tinyLayout(100) });
    const returns = run.frames.reduce((sum, f) => sum + f.coarseReturns, 0);
    const standIns = run.frames.reduce((sum, f) => sum + f.coarseStandIns, 0);
    expect(returns).toBeGreaterThan(0);
    expect(returns).toBeLessThanOrEqual(standIns);
  });

  it("bakes no forced patch in orbit, where the craft has no contact", () => {
    expect(orbitRun({ toS: 12, rateHz: 16 }).frames.every((f) => f.forcedDemanded === 0)).toBe(
      true,
    );
  });
});

/**
 * A draw set over four faces: on face 0, a stands in for its four unbaked children, ab among them
 * once evicted; on face 2, the root stands in for three unbaked children and covers the fourth,
 * resident and evicted earlier; on face 3, a level-12 patch stands in for its child; on face 4, a
 * level-13 patch for its child; face 1's root has no resident ancestor.
 */
function standInScene(): Parameters<typeof coarseStandIns> {
  if (HIGH === undefined) {
    throw new Error("no high setting");
  }
  const cache = new PatchCache(HIGH.layout);
  const a = childKeys(rootKey(0))[0];
  const faceTwo = childKeys(rootKey(2));
  const twelve: PatchKey = { face: 3, level: 12, i: 0, j: 0 };
  const thirteen: PatchKey = { face: 4, level: 13, i: 0, j: 0 };
  for (const key of [rootKey(0), a, rootKey(2), faceTwo[0], twelve, thirteen]) {
    cache.insert({
      key,
      generation: 0,
      originM: vec3(0, 0, 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
    });
  }
  const unitBoxes = selectionOf([
    ...childKeys(a),
    ...faceTwo,
    rootKey(1),
    { face: 3, level: 13, i: 0, j: 0 },
    { face: 4, level: 14, i: 0, j: 0 },
  ]);
  // The resident patch the face-2 root covers lies nearest the camera.
  const resident = patchKeyString(faceTwo[0]);
  const selection: Selection = {
    ...unitBoxes,
    patches: new Map(
      [...unitBoxes.patches].map(([keyString, patch]) => [
        keyString,
        keyString === resident ? { ...patch, bounds: NEAR_BOUNDS } : patch,
      ]),
    ),
  };
  const draw = new DrawSetResolver(cache).resolve(selection);
  const evicted = new Set([childKeys(a)[1], faceTwo[0]].map(patchKeyString));
  return [selection, draw, cache, evicted, PLANET, STAND_IN_VIEW];
}

/** A view 1 km above the fixture's unit box about the origin, so 999 m from such a patch's box. */
const STAND_IN_VIEW: ViewSelectionInput = {
  camera: { positionM: vec3(0, 0, 1_000), orientation: IDENTITY_QUATERNION },
  fovXRad: Math.PI / 3,
  viewport: { widthPx: 1_920, heightPx: 1_080 },
  weight: 1,
  tauPx: 1,
};

/** The fixture's unit box moved 900 m up, 99 m from {@link STAND_IN_VIEW}'s camera. */
const NEAR_BOUNDS: PatchBounds = {
  ...UNIT_BOUNDS,
  centre: vec3(0, 0, 900),
  box: { ...UNIT_BOUNDS.box, centre: vec3(0, 0, 900) },
};

/** The ρ a stand-in of `level` draws `distanceM` away, 999 m by default. */
function standInRhoPx(level: number, distanceM = 999): number {
  return screenSpaceErrorPx(selectionErrorM(PLANET, level), distanceM, STAND_IN_VIEW);
}

describe("the coarse stand-ins (decision-r05-high-bound.md, F4)", () => {
  it("counts each drawn stand-in of level 12 or coarser, and none finer", () => {
    expect(coarseStandIns(...standInScene()).count).toBe(3);
  });

  it("counts a stand-in for a return only where the covered patch is not resident", () => {
    expect(coarseStandIns(...standInScene()).returns).toBe(1);
  });

  it("takes the largest ρ a stand-in draws over every patch it covers, a resident one included", () => {
    // The root's error at the resident patch, 99 m away, the nearest it covers.
    expect(coarseStandIns(...standInScene()).rhoPx).toBeCloseTo(standInRhoPx(0, 99), 9);
  });

  it("takes the returns' largest ρ from the stand-ins over them alone", () => {
    expect(coarseStandIns(...standInScene()).returnRhoPx).toBeCloseTo(standInRhoPx(1), 9);
  });
});

/** A run of `frames` with the high setting's τ of 1 px. */
function runOf(frames: FixedStepFrame[]): ReturnType<typeof runFixedStep> {
  return { tauPx: 1, selectionTauPx: selectionTolerancePx(1), frames, hash: "", truncated: false };
}

/** One segment's figures over `frames` at 8 Hz. */
function figuresOf(frames: FixedStepFrame[]): ReturnType<typeof segmentFigures>[number] {
  const [figures] = segmentFigures(runOf(frames), 8);
  if (figures === undefined) {
    throw new Error("no measured frame");
  }
  return figures;
}

/**
 * A warm-up frame limited at τ′ 5, then measured frames limited at 2.4 and 2, two unlimited (τ′ 1)
 * and one limited at 1.05: steps 5 → 2.4 (from the warm-up, the largest), 2.4 → 2, 2 → 1 and
 * 1 → 1.05, and none between the two unlimited frames.
 */
function limitedFrames(): FixedStepFrame[] {
  return [
    frameOf({ warmup: true, limited: true, tauPrimePx: 5 }),
    frameOf({ limited: true, tauPrimePx: 2.4 }),
    frameOf({ limited: true, tauPrimePx: 2 }),
    frameOf({}),
    frameOf({}),
    frameOf({ limited: true, tauPrimePx: 1.05 }),
  ];
}

/** Five frames, two drawing coarse stand-ins (one for a return) and baking forced patches. */
function standInFrames(): FixedStepFrame[] {
  return [
    frameOf({}),
    frameOf({ coarseStandIns: 1, coarseStandInRhoPx: 3.5, forcedDemanded: 2 }),
    frameOf({}),
    frameOf({
      coarseStandIns: 2,
      coarseReturns: 1,
      coarseStandInRhoPx: 7,
      coarseReturnRhoPx: 6,
      forcedDemanded: 4,
    }),
    frameOf({}),
  ];
}

describe("the budget's figures over a segment (decision-r05-high-bound.md, F4)", () => {
  it("takes τ′'s percentiles over the limited frames alone", () => {
    const figures = figuresOf(limitedFrames());
    expect([figures.tauPrimePxP50, figures.tauPrimePxP95, figures.tauPrimePxMax]).toEqual([
      2, 2.4, 2.4,
    ]);
  });

  it("takes τ′'s steps where either side is limited, the warm-up's last frame included", () => {
    const figures = figuresOf(limitedFrames());
    // Steps of 2.6, 0.4, 1 and 0.05 px.
    expect(figures.tauPrimeStepPxP95).toBeCloseTo(2.6, 12);
    expect(figures.tauPrimeStepPxMax).toBeCloseTo(2.6, 12);
  });

  it("takes each step's ratio as the larger τ′ over the smaller, less 1", () => {
    expect(figuresOf(limitedFrames()).tauPrimeStepRatioMax).toBeCloseTo(5 / 2.4 - 1, 12);
  });

  it("counts the steps beyond the bands' margin", () => {
    // Factors 2.08, 1.2, 2 and 1.05 against 1.1.
    expect(figuresOf(limitedFrames()).tauPrimeStepsOverMargin).toBeCloseTo(3 / 4, 12);
  });

  it("puts a step across a segment boundary in the later segment", () => {
    const frames = [
      frameOf({ limited: true, tauPrimePx: 3 }),
      frameOf({ segment: "descent arc", limited: true, tauPrimePx: 2 }),
    ];
    const [coast, arc] = segmentFigures(runOf(frames), 8);
    expect([coast?.tauPrimeStepPxMax, arc?.tauPrimeStepPxMax]).toEqual([null, 1]);
  });

  it("leaves τ′ and its steps null where no frame is limited", () => {
    const figures = figuresOf([frameOf({}), frameOf({})]);
    expect([
      figures.tauPrimePxP50,
      figures.tauPrimePxMax,
      figures.tauPrimeStepPxMax,
      figures.tauPrimeStepsOverMargin,
      figures.limitedOverTauFraction,
    ]).toEqual([null, null, null, null, null]);
  });

  it("takes the share of the limited frames whose τ′ exceeds the setting's τ", () => {
    // Of three limited frames, one draws within τ = 1 px and two beyond it; the unlimited frame
    // does not count.
    const figures = figuresOf([
      frameOf({ limited: true, tauPrimePx: 0.95 }),
      frameOf({ limited: true, tauPrimePx: 1.2 }),
      frameOf({}),
      frameOf({ limited: true, tauPrimePx: 1.5 }),
    ]);
    expect(figures.limitedOverTauFraction).toBeCloseTo(2 / 3, 12);
  });

  it("leaves the stand-ins' largest ρ null where none is drawn", () => {
    const figures = figuresOf([frameOf({}), frameOf({})]);
    expect([figures.coarseStandInMaxRhoPx, figures.coarseReturnMaxRhoPx]).toEqual([null, null]);
  });

  it("takes the shares of frames drawing coarse stand-ins, and stand-ins for returns", () => {
    const figures = figuresOf(standInFrames());
    expect(figures.coarseStandInFraction).toBeCloseTo(2 / 5, 12);
    expect(figures.coarseReturnFraction).toBeCloseTo(1 / 5, 12);
  });

  it("takes the largest ρ the coarse stand-ins draw, and those for returns", () => {
    const figures = figuresOf(standInFrames());
    expect([figures.coarseStandInMaxRhoPx, figures.coarseReturnMaxRhoPx]).toEqual([7, 6]);
  });

  it("takes the selection's CPU-time percentiles beside its wall-clock ones", () => {
    const figures = figuresOf(
      [4, 1, 3, 2].map((cpu) => frameOf({ selectMs: 10 * cpu, selectCpuMs: cpu })),
    );
    expect([figures.selectCpuMsP50, figures.selectCpuMsP95, figures.selectCpuMsMax]).toEqual([
      2, 4, 4,
    ]);
  });

  it("leaves the selection's CPU-time percentiles null where no frame has one", () => {
    const figures = figuresOf([frameOf({ selectMs: 5 }), frameOf({ selectMs: 7 })]);
    expect([figures.selectCpuMsP50, figures.selectCpuMsP95, figures.selectCpuMsMax]).toEqual([
      null,
      null,
      null,
    ]);
  });

  it("takes the forced bakes a second over the segment's span", () => {
    // Six forced bakes over five frames at 8 Hz.
    expect(figuresOf(standInFrames()).forcedDemandPerS).toBeCloseTo(6 / (5 / 8), 12);
  });
});

describe("the craft's contact", () => {
  const profile = recordProfile();
  const patchSizeM = finestPatchSizeM(PLANET);
  const at = (segment: string, fromEnd: number): ReturnType<typeof profile.poseAt> => {
    const span = profile.segmentSpans().find(({ name }) => name === segment);
    return profile.poseAt((span?.endS ?? NaN) - fromEnd);
  };

  it("holds at an exact hover, still and a metre up, where isDescending does not", () => {
    const hover = profile.poseAt(profile.durationS);
    expect(hover.verticalSpeedMps).toBe(0);
    expect(craftContacts(hover, patchSizeM)).toEqual([
      { positionM: hover.groundPointM, radiusM: SPIKE_CRAFT_RADIUS_M },
    ]);
  });

  it("holds while descending, and not on the low fast pass or in orbit", () => {
    expect(craftContacts(at("vertical descent", 5), patchSizeM)).toHaveLength(1);
    expect(craftContacts(at("low fast pass", 10), patchSizeM)).toEqual([]);
    expect(craftContacts(at("orbit coast", 10), patchSizeM)).toEqual([]);
  });
});

describe("the ranges fixture", () => {
  it("round-trips its text", () => {
    const fixture = {
      levelTable: Float64Array.from([0.1, 1 / 3]),
      omittedSigmaM: [645, 0.0529, 0],
      siteHeightM: -1952.5,
      ranges: new Map<string, readonly [number, number]>([
        ["0/19/300001/200003", [-12.5, 40.25]],
        ["3/2/1/0", [-4000.125, 6000]],
      ]),
    };
    expect(parseRanges(formatRanges(fixture))).toEqual(fixture);
    expect(formatRanges(fixture).split("\n")[3]).toBe("0 19 300001 200003 -12.5 40.25");
  });

  it("refuses a key it lacks, naming the command that regenerates it", () => {
    const rangeOf = fixtureRangeOf(new Map());
    expect(() => rangeOf({ face: 1, level: 3, i: 2, j: 1 })).toThrow(
      /descent-demand --write-fixture/,
    );
  });

  it("bakes each key once through the memo", () => {
    let bakes = 0;
    const memo = memoised(() => {
      bakes += 1;
      return [0, 1];
    });
    memo.rangeOf({ face: 0, level: 1, i: 0, j: 0 });
    memo.rangeOf({ face: 0, level: 1, i: 0, j: 0 });
    expect(bakes).toBe(1);
    expect(memo.ranges.size).toBe(1);
  });
});

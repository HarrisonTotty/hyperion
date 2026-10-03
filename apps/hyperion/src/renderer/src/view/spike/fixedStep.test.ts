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
import { type FixedStepOptions, runFixedStep, segmentFigures } from "./fixedStep";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

const PLANET = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
const HIGH = SETTING_VIEWS[0];

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

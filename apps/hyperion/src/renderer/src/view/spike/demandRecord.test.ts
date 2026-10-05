import { describe, expect, it } from "vitest";

import { goldenLevelTable } from "../../test/terrainFixtures";
import { levelHeightRangeM, planetGeometry } from "../terrain/planet";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";
import ridgesOffRanges from "./fixtures/descentRanges.txt?raw";
import {
  demandSummary,
  fixtureRangeOf,
  parseRanges,
  patchKeyAt,
  RIDGED_CALIBRATED_NOTE,
  recordProfile,
  runCell,
  SETTING_VIEWS,
  stretchKeys,
  testWindows,
} from "./demandRecord";
import { DescentProfile, landingSiteOf, trackStretches } from "./descentProfile";
import { cornerNeighbours, EDGES, edgeNeighbour, patchKeyString } from "../terrain/patchKey";

const FIXTURE = parseRanges(ridgesOffRanges);

/** The module's ridges-off level table, σ_n and baked ranges, as the fixture holds them. */
const SOURCE = {
  levelTable: FIXTURE.levelTable,
  omittedSigmaM: FIXTURE.omittedSigmaM,
  rangeOf: fixtureRangeOf(FIXTURE.ranges),
  // The record asks the collision interpolant at the landing site alone; the stretches' floors
  // come from the fixture's ranges.
  surfaceHeightM: () => FIXTURE.siteHeightM,
};

/**
 * Each window's selection-sequence hash under min(hard, 4σ_n), ridges off, as
 * `just descent-demand --write-fixture` printed them on 2026-10-03 (TEST_PLANET_VERSION 2, the
 * terrain cache of 5d90fab, the site's height read along the spheroid point's direction d, the
 * profile flown over the stretches' floors of decision-r05-descent-clearance.md), and on 2026-10-04
 * with the orbit coast flown level, which moved high's coast alone.
 */
const PINNED: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  high: {
    "orbit coast": "c739aef5d81221ae",
    "descent arc": "97e2ca6b8aee73b2",
    "approach and flare": "dfd9239538b60509",
    "low fast pass": "5af4f15ee2ca16cc",
    slowdown: "349b772887d62f19",
    "vertical descent": "7f86f5753748ba08",
    "hover and touchdown": "3815401074eed29e",
  },
  low: {
    "orbit coast": "c9a36f3ba4350b99",
    "descent arc": "5706a45e7208ead2",
    "approach and flare": "b3481b07adaad58f",
    "low fast pass": "0e3991c21a0c6b9f",
    slowdown: "279852ec8d9689c5",
    "vertical descent": "32c0390b8dfa7eae",
    "hover and touchdown": "1e59c48692289e86",
  },
};

/**
 * The windows where the measured demand lies within a factor of two of the per-level D. The
 * others miss, recorded as findings in the plan's Risks (T13.a, as built), never by a looser test.
 */
const WITHIN_TWO: ReadonlyArray<string> = [
  "high/descent arc",
  "high/approach and flare",
  "high/low fast pass",
  "low/approach and flare",
  "low/low fast pass",
  "low/slowdown",
];

const PROFILE = recordProfile();

describe("the ranges fixture", () => {
  it("holds the module's ridges-off level table, as the golden pins it", () => {
    expect(FIXTURE.levelTable).toEqual(goldenLevelTable("off"));
    expect(FIXTURE.omittedSigmaM).toHaveLength(25);
  });
});

describe("the descent's fixed-step windows", () => {
  for (const settingView of SETTING_VIEWS) {
    for (const window of testWindows(PROFILE)) {
      it(`pins ${settingView.setting}'s ${window.segment} selection sequence`, () => {
        const cell = runCell({
          rule: "calibrated",
          ridges: "off",
          settingView,
          source: SOURCE,
          rateHz: 64,
          fromS: window.fromS,
          toS: window.toS,
          measureFromS: window.measureFromS,
          nowMs: () => 0,
          deadlineMs: Infinity,
        });
        expect(cell.hash).toBe(PINNED[settingView.setting]?.[window.segment]);
        expect(cell.segments[0]?.segment).toBe(window.segment);
      });
    }
  }
});

describe("the measured demand", () => {
  for (const entry of WITHIN_TWO) {
    const [setting, segment] = entry.split("/");
    it(`lies within a factor of two of D in ${setting}'s ${segment}`, () => {
      const settingView = SETTING_VIEWS.find((s) => s.setting === setting);
      const window = testWindows(PROFILE).find((w) => w.segment === segment);
      if (settingView === undefined || window === undefined) {
        throw new Error(`no window ${entry}`);
      }
      const cell = runCell({
        rule: "calibrated",
        ridges: "off",
        settingView,
        source: SOURCE,
        rateHz: 64,
        fromS: window.fromS,
        toS: window.toS,
        measureFromS: window.measureFromS,
        nowMs: () => 0,
        deadlineMs: Infinity,
      });
      const [figures] = cell.segments;
      const ratio = (figures?.demandPerS ?? NaN) / (figures?.predictedPerS ?? NaN);
      expect(ratio).toBeGreaterThanOrEqual(0.5);
      expect(ratio).toBeLessThanOrEqual(2);
    });
  }
});

describe("the demand record", () => {
  it("labels the ridged planet's 4σ cells statistical, and no other", () => {
    const [high] = SETTING_VIEWS;
    if (high === undefined) {
      throw new Error("no high setting");
    }
    const planet = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("on"));
    const source = {
      levelTable: goldenLevelTable("on"),
      omittedSigmaM: FIXTURE.omittedSigmaM,
      rangeOf: (key: { readonly level: number }) => levelHeightRangeM(planet, key.level),
      surfaceHeightM: () => 0,
    };
    const cellOf = (rule: "hard" | "calibrated", ridges: "off" | "on") =>
      runCell({
        rule,
        ridges,
        settingView: high,
        source,
        rateHz: 4,
        fromS: 10,
        toS: 10.5,
        measureFromS: 10,
        nowMs: () => 0,
        deadlineMs: Infinity,
      });
    expect(cellOf("calibrated", "on").note).toBe(RIDGED_CALIBRATED_NOTE);
    expect(cellOf("calibrated", "off").note).toBeNull();
    expect(cellOf("hard", "on").note).toBeNull();
    expect(demandSummary([cellOf("calibrated", "on")], "2026-10-03T00:00:00Z")).toContain(
      RIDGED_CALIBRATED_NOTE,
    );
  });
});

describe("stretchKeys", () => {
  it("hold every 64 Hz pose's ground patch and its neighbours, stretch by stretch", () => {
    const datum = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n));
    const missed: string[] = [];
    for (const stretch of trackStretches(datum)) {
      const keys = new Set(stretchKeys(datum, stretch).map((key) => patchKeyString(key)));
      for (let n = Math.ceil(stretch.startS * 64); n <= stretch.endS * 64; n += 1) {
        const g = datum.groundDirAt(n / 64);
        const key = patchKeyAt([g.x, g.y, g.z], stretch.level);
        const around = [
          key,
          ...EDGES.map((edge) => edgeNeighbour(key, edge)),
          ...cornerNeighbours(key),
        ];
        for (const each of around) {
          if (each !== null && !keys.has(patchKeyString(each))) {
            missed.push(`${stretch.piece} at ${n / 64} s: ${patchKeyString(each)}`);
          }
        }
      }
    }
    expect(missed).toEqual([]);
  });
});

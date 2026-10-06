import { describe, expect, it } from "vitest";

import { goldenLevelTable } from "../../test/terrainFixtures";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";
import { levelHeightRangeM, planetGeometry } from "../terrain/planet";
import { SELECTION_MARGIN } from "../terrain/selectionTolerance";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";
import ridgesOffRanges from "./fixtures/descentRanges.txt?raw";
import {
  DEMAND_RECORD_SCHEMA,
  DEMAND_RECORD_VERSION,
  type DemandRecordFile,
  demandSummary,
  fixtureRangeOf,
  mergeRecords,
  parseRanges,
  patchKeyAt,
  RECORD_SEED,
  type RecordedCell,
  recordRules,
  recordStem,
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
 * with the orbit coast flown level, which moved high's coast alone. 2026-10-05: selected at
 * τ ÷ 1.1 (decision-r05-record-tau.md); every window moved but high's vertical descent and hover,
 * whose selections are the same at either tolerance. Before re-pinning, the 14 windows run at the
 * setting's τ reproduced the hashes pinned before, and the fixture, byte for byte.
 */
const PINNED: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  high: {
    "orbit coast": "df104b5501ce5e0b",
    "descent arc": "a4180564c215ffe6",
    "approach and flare": "f77246a34f435036",
    "low fast pass": "fcc42d115d7da663",
    slowdown: "64e24061df5b2400",
    "vertical descent": "7f86f5753748ba08",
    "hover and touchdown": "3815401074eed29e",
  },
  low: {
    "orbit coast": "3674ed69e62032f9",
    "descent arc": "2c266fb1ea0c57d8",
    "approach and flare": "7e583e2ae02512d0",
    "low fast pass": "1f9f60c3def409b7",
    slowdown: "03e68d9fca900571",
    "vertical descent": "e2a6afd03bee7c06",
    "hover and touchdown": "927a1d8e235bfa8e",
  },
};

/**
 * The windows where the measured demand lies within a factor of two of the per-level D. The
 * others miss, recorded as findings in the plan's Risks (T13.a, as built), never by a looser test.
 * At τ ÷ 1.1 (2026-10-05) high's descent arc missed (0.49) and high's slowdown came within (0.61).
 */
const WITHIN_TWO: ReadonlyArray<string> = [
  "high/approach and flare",
  "high/low fast pass",
  "high/slowdown",
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
  it.each(SETTING_VIEWS.map((settingView) => [settingView.setting, settingView] as const))(
    "selects the %s setting at the terrain pass's τ ÷ (1 + SELECTION_MARGIN)",
    (setting, settingView) => {
      const planet = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
      const cell = runCell({
        rule: "hard",
        ridges: "off",
        settingView,
        source: { ...SOURCE, rangeOf: (key) => levelHeightRangeM(planet, key.level) },
        rateHz: 4,
        fromS: 10,
        toS: 10.5,
        measureFromS: 10,
        nowMs: () => 0,
        deadlineMs: Infinity,
      });
      expect([cell.tauPx, cell.selectionTauPx]).toEqual([
        TERRAIN_SETTINGS[setting].tauPx,
        TERRAIN_SETTINGS[setting].tauPx / (1 + SELECTION_MARGIN),
      ]);
    },
  );

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

/** The orbit coast's row of a summary's second table. */
function budgetRowOf(summary: string): string | undefined {
  return summary.split("\n").filter((line) => line.startsWith("| orbit coast"))[1];
}

/** A record file of this version holding `cells`. */
function fileOf(startedAt: string, cells: RecordedCell[]): DemandRecordFile {
  return {
    schema: DEMAND_RECORD_SCHEMA,
    version: DEMAND_RECORD_VERSION,
    startedAt,
    seed: String(RECORD_SEED),
    testPlanetVersion: 2,
    notes: [],
    cells,
  };
}

describe("the record's file (version 3)", () => {
  const [high] = SETTING_VIEWS;
  /** A short orbit-coast cell of `rule`, under a budget of `maxPatches`, as the script records it. */
  const cellOf = (rule: "hard" | "calibrated", maxPatches: number): RecordedCell => {
    if (high === undefined) {
      throw new Error("no high setting");
    }
    const planet = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
    const cell = runCell({
      rule,
      ridges: "off",
      settingView: { ...high, maxPatches },
      source: {
        ...SOURCE,
        rangeOf: (key) => levelHeightRangeM(planet, key.level),
      },
      rateHz: 16,
      fromS: 10,
      toS: 12,
      measureFromS: 11,
      nowMs: () => 0,
      deadlineMs: Infinity,
    });
    return { ...cell, capHours: 0.5, wallCapHours: null, wallS: 1, loadAverage: [1, 2, 3] };
  };

  /** Two records, the hard cell's started later, merged in that order. */
  const merged = (): DemandRecordFile =>
    mergeRecords(
      [
        fileOf("2026-10-04T15:00:00.000Z", [cellOf("hard", 40)]),
        fileOf("2026-10-04T14:00:00.000Z", [cellOf("calibrated", 981)]),
      ],
      ["four processes at once"],
    );

  it("merges the processes' cells in the order given", () => {
    expect(merged().cells.map(({ rule }) => rule)).toEqual(["hard", "calibrated"]);
  });

  it("dates a merged record from its earliest start", () => {
    expect(merged().startedAt).toBe("2026-10-04T14:00:00.000Z");
  });

  it("gives a merged record the notes passed, not its parts'", () => {
    expect(merged().notes).toEqual(["four processes at once"]);
  });

  it("names a merged record from its cells' rules in order", () => {
    const file = merged();
    expect(recordStem(file.startedAt, recordRules(file))).toBe("2026-10-04-demand-hard+calibrated");
  });

  it("names a run's record from the rules it runs, whatever cells it has written", () => {
    expect(recordStem("2026-10-04T14:00:00.000Z", ["hard", "calibrated"])).toBe(
      "2026-10-04-demand-hard+calibrated",
    );
  });

  it("refuses to merge a record of another version", () => {
    const cell = cellOf("hard", 981);
    const old = { ...fileOf("2026-10-03T00:00:00.000Z", [cell]), version: 1 };
    expect(() => mergeRecords([fileOf("2026-10-04T00:00:00.000Z", [cell]), old], [])).toThrow(
      /version 1/,
    );
  });

  it("refuses to merge records of two seeds", () => {
    const cell = cellOf("hard", 981);
    const other = { ...fileOf("2026-10-04T00:00:00.000Z", [cell]), seed: "5" };
    expect(() => mergeRecords([fileOf("2026-10-04T00:00:00.000Z", [cell]), other], [])).toThrow(
      /seed 5/,
    );
  });

  it("summarises τ′'s percentiles where the budget binds", () => {
    const limited = cellOf("hard", 40);
    const [coast] = limited.segments;
    expect(coast?.tauPrimePxMax ?? 0).toBeGreaterThan(1);
    expect(budgetRowOf(demandSummary([limited], "2026-10-04T00:00:00Z"))).toContain(
      `| ${(coast?.tauPrimePxP50 ?? NaN).toFixed(2)} / ${(coast?.tauPrimePxP95 ?? NaN).toFixed(2)} / `,
    );
  });

  it("summarises no τ′ where the budget never binds", () => {
    expect(budgetRowOf(demandSummary([cellOf("hard", 981)], "2026-10-04T00:00:00Z"))).toMatch(
      /^\| orbit coast \| — \| — \| — \| — \|/,
    );
  });

  it("summarises the coarse stand-ins' share of frames and their returns'", () => {
    const cell = cellOf("hard", 981);
    const [coast] = cell.segments;
    expect(budgetRowOf(demandSummary([cell], "2026-10-04T00:00:00Z"))).toContain(
      `| ${(100 * (coast?.coarseStandInFraction ?? NaN)).toFixed(1)}% (${(100 * (coast?.coarseReturnFraction ?? NaN)).toFixed(1)}%) |`,
    );
  });

  it("lists the record's notes", () => {
    expect(demandSummary([], "2026-10-04T00:00:00Z", ["a note"])).toContain("- a note");
  });

  it("says how many cells recorded no load average, which Windows keeps none of", () => {
    const windows = { ...cellOf("hard", 40), loadAverage: [] };
    const summary = demandSummary([windows, cellOf("calibrated", 981)], "2026-10-04T00:00:00Z");
    expect(summary).toContain("1 of the 2 cells recorded no load average: Windows keeps none");
  });

  it("says nothing of the load average where every cell recorded one", () => {
    const summary = demandSummary([cellOf("hard", 40)], "2026-10-04T00:00:00Z");
    expect(summary).not.toContain("recorded no load average");
  });

  it("states the tolerance it selects at, the terrain pass's", () => {
    expect(demandSummary([], "2026-10-05T00:00:00Z")).toContain(
      "Selected at τ ÷ 1.1, the terrain pass's τ_sel; D at the same tolerance;",
    );
  });

  it("summarises the selection's CPU time beside its wall-clock time", () => {
    if (high === undefined) {
      throw new Error("no high setting");
    }
    const planet = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
    let cpuMs = 0;
    const cell = runCell({
      rule: "hard",
      ridges: "off",
      settingView: high,
      source: { ...SOURCE, rangeOf: (key) => levelHeightRangeM(planet, key.level) },
      rateHz: 16,
      fromS: 10,
      toS: 11,
      measureFromS: 10.5,
      nowMs: () => 0,
      cpuNowMs: () => {
        cpuMs += 1.25;
        return cpuMs;
      },
      deadlineMs: Infinity,
    });
    const firstRow = demandSummary([cell], "2026-10-05T00:00:00Z")
      .split("\n")
      .find((line) => line.startsWith("| orbit coast"));
    expect(firstRow).toMatch(/\| 0\.0 \/ 0\.0 \/ 0\.0 \| 1\.3 \/ 1\.3 \/ 1\.3 \|$/);
  });

  it("summarises no CPU time for a run without a CPU clock", () => {
    const row = demandSummary([cellOf("hard", 981)], "2026-10-05T00:00:00Z")
      .split("\n")
      .find((line) => line.startsWith("| orbit coast"));
    expect(row).toMatch(/\| — \|$/);
  });

  it("summarises the share of limited frames whose τ′ exceeds τ", () => {
    const limited = cellOf("hard", 40);
    const [coast] = limited.segments;
    const firstRow = demandSummary([limited], "2026-10-04T00:00:00Z")
      .split("\n")
      .find((line) => line.startsWith("| orbit coast"));
    expect(firstRow).toContain(
      `| ${(100 * (coast?.limitedFraction ?? NaN)).toFixed(0)}% | ${(100 * (coast?.limitedOverTauFraction ?? NaN)).toFixed(1)}% |`,
    );
  });
});

describe("the forced region's bakes", () => {
  it("are part of the demand in high's approach, where the craft descends", () => {
    const [high] = SETTING_VIEWS;
    const window = testWindows(PROFILE).find((w) => w.segment === "approach and flare");
    if (high === undefined || window === undefined) {
      throw new Error("no approach window");
    }
    const [figures] = runCell({
      rule: "calibrated",
      ridges: "off",
      settingView: high,
      source: SOURCE,
      rateHz: 64,
      fromS: window.fromS,
      toS: window.toS,
      measureFromS: window.measureFromS,
      nowMs: () => 0,
      deadlineMs: Infinity,
    }).segments;
    expect(figures?.forcedDemandPerS).toBeGreaterThan(0);
    expect(figures?.forcedDemandPerS).toBeLessThanOrEqual(figures?.demandPerS ?? 0);
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

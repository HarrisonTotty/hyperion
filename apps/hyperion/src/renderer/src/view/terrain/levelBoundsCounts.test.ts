/**
 * R10.T4.b's counts, rebuilt (plan R10, R10.T15.b): the harness's cells, figures and record on a
 * cut of its poses; the whole run is `scripts/levelCounts.mjs`'s, by hand.
 */

import { describe, expect, it } from "vitest";

import fixture from "./fixtures/level-bounds.json" with { type: "json" };
import {
  type CellFigures,
  cellFigures,
  COUNT_FOV_X_RAD,
  COUNT_STYLES,
  COUNT_VIEWPORT,
  countCell,
  countProfile,
  countTimesS,
  GUARD_PATCHES,
  levelCountsMarkdown,
  levelCountsRecord,
  type SampleWorld,
  sampleWorld,
  savingOf,
} from "./levelBoundsCounts";
import { planetGeometry } from "./planet";
import { selectPatches } from "./select";

const WORLDS: ReadonlyArray<SampleWorld> = fixture.worlds.map(sampleWorld);

function worldNamed(name: string): SampleWorld {
  const world = WORLDS.find((each) => each.name === name);
  if (world === undefined) {
    throw new Error(`the fixture has no ${name}`);
  }
  return world;
}

function styleNamed(name: "high" | "low" | "wireframe"): (typeof COUNT_STYLES)[number] {
  const style = COUNT_STYLES.find((each) => each.name === name);
  if (style === undefined) {
    throw new Error(`no style ${name}`);
  }
  return style;
}

/** The figures of a cell with these sums, capped poses and τ′ alone. */
function figures(sum: number, capped: number): CellFigures {
  return { sum, capped, limited: 0, tauPrimeMeanPx: 1, tauPrimeMaxPx: 1, poses: 124 };
}

describe("the counts' set-up (R10.T4.b's record)", () => {
  it("counts the descent every 10 s from 0 s to its end, 124 poses, on every world", () => {
    for (const world of WORLDS) {
      const times = countTimesS(countProfile(world));
      expect([times.length, times[0], times[1], times.at(-1)]).toEqual([124, 0, 10, 1230]);
    }
  });

  it("selects each style at its τ and its pass's budget, half its slots", () => {
    expect(COUNT_STYLES.map(({ name, tauPx, maxPatches }) => [name, tauPx, maxPatches])).toEqual([
      ["high", 1, 604],
      ["low", 2, 187],
      ["wireframe", 4, 260],
    ]);
    expect([COUNT_VIEWPORT, COUNT_FOV_X_RAD, GUARD_PATCHES]).toEqual([
      { widthPx: 1920, heightPx: 1080 },
      Math.PI / 3,
      16_384,
    ]);
  });

  it("states T4.a's patches past 4σ of the body and of their own cells", () => {
    expect(WORLDS.map((world) => [world.name, world.pastBodySigma, world.patches])).toEqual([
      ["earth_like", 0, 76],
      ["mars_like", 41, 72],
      ["moon_like", 49, 68],
      ["ceres_like", 46, 60],
      ["europa_like", 45, 68],
      ["locked_earth_like", 0, 76],
    ]);
    expect(WORLDS.every((world) => world.pastOwnSigma <= world.patches)).toBe(true);
  });
});

describe("a cell of the counts", () => {
  const world = worldNamed("mars_like");
  const timesS = [300, 600, 1200];

  it("counts the guard's patches and takes τ′ from the budget's excess, pose by pose", () => {
    const style = styleNamed("high");
    const cell = countCell({ world, rule: "hard", style, boxes: "flat", timesS });
    const planet = planetGeometry(world.figure, world.table, world.sigmaM, "hard");
    const profile = countProfile(world);
    expect(cell.poses.map((pose) => pose.tS)).toEqual(timesS);
    for (const pose of cell.poses) {
      const at = profile.poseAt(pose.tS);
      const select = (maxPatches: number): ReturnType<typeof selectPatches> =>
        selectPatches({
          planet,
          views: [
            {
              camera: { positionM: at.positionM, orientation: at.orientation },
              fovXRad: COUNT_FOV_X_RAD,
              viewport: COUNT_VIEWPORT,
              weight: 1,
              tauPx: style.tauPx,
            },
          ],
          setting: "high",
          grounded: [],
          heightRanges: { heightRangeM: () => [-1, 1] },
          maxPatches,
        });
      const guard = select(GUARD_PATCHES);
      const budget = select(style.maxPatches);
      expect(pose).toEqual({
        tS: pose.tS,
        patches: guard.patches.size,
        capped: guard.limited,
        limited: budget.limited,
        tauPrimePx: style.tauPx * Math.max(1, budget.limitExcess),
      });
    }
  });

  it("selects no more under min(hard, 4σ) than under the hard bound", () => {
    const style = styleNamed("low");
    const [hard, statistical] = (["hard", "statistical"] as const).map((rule) =>
      countCell({ world, rule, style, boxes: "unbaked", timesS }),
    );
    if (hard === undefined || statistical === undefined) {
      throw new Error("two rules, two cells");
    }
    expect(
      hard.poses.map((pose, i) => pose.patches >= (statistical.poses[i]?.patches ?? Infinity)),
    ).toEqual([true, true, true]);
    expect(cellFigures(statistical).sum).toBeGreaterThan(0);
  });

  it("sums a cell's poses, its capped and limited ones, and τ′'s mean and largest", () => {
    const summed = cellFigures({
      world: "w",
      rule: "hard",
      style: "high",
      tauPx: 1,
      maxPatches: 604,
      boxes: "unbaked",
      poses: [
        { tS: 0, patches: 100, capped: false, limited: false, tauPrimePx: 1 },
        { tS: 10, patches: GUARD_PATCHES, capped: true, limited: true, tauPrimePx: 9 },
      ],
    });
    expect(summed).toEqual({
      sum: 100 + GUARD_PATCHES,
      capped: 1,
      limited: 1,
      tauPrimeMeanPx: 5,
      tauPrimeMaxPx: 9,
      poses: 2,
    });
  });
});

describe("the saving of min(hard, 4σ)", () => {
  it("is exact where the guard capped neither sum", () => {
    expect(savingOf(figures(1_000, 0), figures(100, 0))).toEqual({ ratio: 10, bounds: "exact" });
  });

  it("is a lower bound where the guard capped the hard sum alone", () => {
    expect(savingOf(figures(1_000, 3), figures(100, 0)).bounds).toBe("at least");
  });

  it("bounds nothing where the guard capped the min(hard, 4σ) sum", () => {
    expect(savingOf(figures(1_000, 124), figures(500, 46)).bounds).toBe("none");
  });
});

describe("the counts' record", () => {
  it("states each table's figures with their marks, and every world's T4.a counts", () => {
    const world = worldNamed("ceres_like");
    const cells = (["hard", "statistical"] as const).flatMap((rule) =>
      COUNT_STYLES.flatMap((style) =>
        (["unbaked", "flat"] as const).map((boxes) =>
          countCell({ world, rule, style, boxes, timesS: [600] }),
        ),
      ),
    );
    const record = levelCountsRecord({
      startedAt: "2026-10-29T00:00:00Z",
      generatorVersion: fixture.generator_version,
      worlds: [world],
      cells,
      notes: ["a cut of one pose"],
    });
    expect(record).toMatchObject({
      schema: "hyperion.terrain.level-counts",
      version: 1,
      seed: "7",
      poseStepS: 10,
      guardPatches: GUARD_PATCHES,
      fovXDeg: 60,
      worlds: [{ name: "ceres_like", patches: 60, pastBodySigma: 46 }],
    });
    expect(record.cells).toHaveLength(12);
    const markdown = levelCountsMarkdown(record);
    expect(markdown).toContain("## The sums and the savings");
    expect(markdown).toContain("| ceres_like |");
    expect(markdown).toContain("46 of 60 (0.767)");
    expect(markdown).toContain("- a cut of one pose");
    expect(markdown).toContain("wireframe (4 px, 260)");
  });
});

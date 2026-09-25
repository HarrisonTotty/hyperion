import type { MassLayer, ObjectKindDto, StellarBriefDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { aStellarBrief, aSystemsInRange } from "../../test/galaxyFixtures";
import {
  HR_SPECTRAL_BANDS,
  hrDrawList,
  hrSpectralLetters,
  hrXPx,
  hrYPx,
  pickHr,
  type PlotAreaPx,
  projectHr,
} from "./hrProjection";
import type { ChartSystem } from "./model";
import { toChartResult } from "./wire";

/** A plot 460 px wide and 250 px high, 50 px in from the canvas's left and 20 px down. */
const AREA: PlotAreaPx = { leftPx: 50, topPx: 20, widthPx: 460, heightPx: 250 };

/** CSS pixels in one `rem` at the default interface scale. */
const REM_PX = 16;

interface SystemSpec {
  readonly relLy?: readonly [number, number, number];
  readonly layer?: MassLayer;
  readonly stellar: StellarBriefDto | null;
}

/** The chart systems of an answer holding these, in the order given (1 ly apart by default). */
function systemsOf(specs: ReadonlyArray<SystemSpec>): ReadonlyArray<ChartSystem> {
  return toChartResult(
    aSystemsInRange({
      systems: specs.map(({ relLy, layer = "c", stellar }, index) => ({
        relLy: relLy ?? [index + 1, 0, 0],
        layer,
        stellar,
      })),
    }),
  ).systems;
}

/** A brief of `kind` at the temperature and luminosity given. */
function brief(kind: ObjectKindDto, teffK: number | null, logL: number | null): StellarBriefDto {
  return { ...aStellarBrief("c", kind), teff_k: teffK, log_luminosity_lsun: logL };
}

describe("hrXPx and hrYPx", () => {
  it("place the Sun where log T_eff and log L put it", () => {
    // (log 200,000 − log 5772) ÷ (log 200,000 − log 1000) of the width, and 6.5 ÷ 12.5 of the height.
    expect(hrXPx(5_772, AREA)).toBeCloseTo(357.803, 3);
    expect(hrYPx(0, AREA)).toBeCloseTo(150, 9);
  });

  it("run the temperature axis hot to cool from left to right", () => {
    expect(hrXPx(200_000, AREA)).toBeCloseTo(50, 9);
    expect(hrXPx(1_000, AREA)).toBeCloseTo(510, 9);
  });

  it("run the luminosity axis bright to faint from top to bottom", () => {
    expect(hrYPx(6.5, AREA)).toBeCloseTo(20, 9);
    expect(hrYPx(-6, AREA)).toBeCloseTo(270, 9);
  });
});

describe("projectHr", () => {
  it("plots the Sun at its pixel with the chart's symbol and size class", () => {
    const projection = projectHr(systemsOf([{ stellar: brief("dwarf", 5_772, 0) }]), AREA, 50);

    const [sun] = projection.points;
    expect(sun?.xPx).toBeCloseTo(357.803, 3);
    expect(sun?.yPx).toBeCloseTo(150, 9);
    expect(sun?.shape).toBe("circle");
    expect(sun?.sizeClass).toBe(2);
    expect(sun?.status).toBe("available");
  });

  it("marks a system beyond the drive range plain, as the chart does", () => {
    const projection = projectHr(
      systemsOf([{ relLy: [60, 0, 0], stellar: brief("dwarf", 5_772, 0) }]),
      AREA,
      50,
    );

    expect(projection.points[0]?.status).toBe("plain");
  });

  it("pegs a point beyond an axis at its edge and counts it off scale", () => {
    const projection = projectHr(
      systemsOf([
        { stellar: brief("white_dwarf", 250_000, -1) },
        { stellar: brief("substellar", 600, -7) },
        { stellar: brief("supergiant", 3_600, 6.8) },
      ]),
      AREA,
      50,
    );

    const [hot, cool, bright] = projection.points;
    expect([hot?.xPx, hot?.teffPeg, hot?.luminosityPeg]).toEqual([50, "hot", null]);
    expect(cool?.xPx).toBeCloseTo(510, 9);
    expect(cool?.yPx).toBeCloseTo(270, 9);
    expect([cool?.teffPeg, cool?.luminosityPeg]).toEqual(["cool", "faint"]);
    expect([bright?.yPx, bright?.luminosityPeg]).toEqual([20, "bright"]);
    expect(projection.counts.offScale).toBe(3);
    expect(projection.counts.plotted).toBe(3);
  });

  it("counts neutron stars and black holes without plotting them", () => {
    const projection = projectHr(
      systemsOf([
        { stellar: aStellarBrief("e", "neutron_star") },
        { stellar: aStellarBrief("e", "black_hole") },
        { stellar: aStellarBrief("c", "white_dwarf") },
      ]),
      AREA,
      50,
    );

    expect(projection.points.map((point) => point.shape)).toEqual(["diamond"]);
    expect(projection.counts.noPhotosphere).toBe(2);
  });

  it("counts a star that left no remnant and a system not yet formed apart", () => {
    const projection = projectHr(
      systemsOf([{ stellar: aStellarBrief("e", "no_remnant") }, { stellar: null }]),
      AREA,
      50,
    );

    expect(projection.points).toHaveLength(0);
    expect(projection.counts).toEqual({
      plotted: 0,
      offScale: 0,
      noPhotosphere: 0,
      noRemnant: 1,
      notYetFormed: 1,
      noData: 0,
    });
  });

  it("counts a star whose temperature or luminosity is missing rather than hide it", () => {
    const projection = projectHr(
      systemsOf([{ stellar: brief("dwarf", null, 0) }, { stellar: brief("giant", 0, 1) }]),
      AREA,
      50,
    );

    expect(projection.points).toHaveLength(0);
    expect(projection.counts.noData).toBe(2);
  });

  it("plots nothing and counts nothing for no systems", () => {
    const projection = projectHr([], AREA, 50);

    expect(projection.points).toHaveLength(0);
    expect(Object.values(projection.counts).every((count) => count === 0)).toBe(true);
  });
});

describe("hrDrawList", () => {
  const SYSTEMS = systemsOf([
    { stellar: brief("dwarf", 5_772, 0) },
    { relLy: [60, 0, 0], stellar: brief("giant", 4_800, 1.8) },
    { stellar: brief("white_dwarf", 250_000, -1) },
  ]);

  it("draws every point filled, in --accent within the drive range and --text beyond it", () => {
    const list = hrDrawList(projectHr(SYSTEMS, AREA, 50), AREA, null, REM_PX);

    const symbols = list.ops.filter((op) => op.kind === "symbol");
    expect(symbols.map((op) => [op.shape, op.stroke, op.fill])).toEqual([
      ["ringed-circle", "text", "text"],
      ["diamond", "accent", "accent"],
      ["circle", "accent", "accent"],
    ]);
  });

  it("draws the off-scale mark beside a pegged point, pointing off the scale", () => {
    const list = hrDrawList(projectHr(SYSTEMS, AREA, 50), AREA, null, REM_PX);

    // The white dwarf is hotter than the left edge: its arrowhead's tip is left of it.
    const marks = list.ops.filter(
      (op) => op.kind === "ticks" && op.segments.every((segment) => segment.from.xPx < 50),
    );
    expect(marks).toHaveLength(1);
  });

  it("brackets the selected point with the chart's reticle, drawn last", () => {
    const id = SYSTEMS[0]?.id ?? "";

    const list = hrDrawList(projectHr(SYSTEMS, AREA, 50), AREA, id, REM_PX);

    const last = list.ops.at(-1);
    expect(last?.kind).toBe("reticle");
    expect(last?.kind === "reticle" ? [last.id, last.stroke] : null).toEqual([id, "accent"]);
  });

  it("draws its grid in --line and its frame in --text-muted, with no points", () => {
    const list = hrDrawList(projectHr([], AREA, 50), AREA, null, REM_PX);

    const lines = list.ops.filter((op) => op.kind === "line");
    expect(lines.every((op) => op.stroke === "line")).toBe(true);
    expect(lines).toHaveLength(5 + 7);
    expect(list.ops.some((op) => op.kind === "polyline" && op.stroke === "textMuted")).toBe(true);
    expect(list.anchors).toHaveLength(0);
  });
});

describe("pickHr", () => {
  const SYSTEMS = systemsOf([{ stellar: brief("dwarf", 5_772, 0) }]);
  const list = hrDrawList(projectHr(SYSTEMS, AREA, 50), AREA, null, REM_PX);
  const sunXPx = hrXPx(5_772, AREA);

  it("picks a point a full rem from it", () => {
    expect(pickHr(list.anchors, { xPx: sunXPx + REM_PX, yPx: 150 }, REM_PX)).toBe(SYSTEMS[0]?.id);
  });

  it("picks nothing beyond a rem of every point", () => {
    expect(pickHr(list.anchors, { xPx: sunXPx + REM_PX + 1, yPx: 150 }, REM_PX)).toBeNull();
  });
});

describe("hrSpectralLetters", () => {
  it("runs O to T from left to right, each inside its band", () => {
    const letters = hrSpectralLetters(AREA);

    expect(letters.map((place) => place.letter).join("")).toBe("OBAFGKMLT");
    for (const [index, place] of letters.entries()) {
      const band = HR_SPECTRAL_BANDS[index];
      expect(place.xPx).toBeGreaterThan(hrXPx(band?.hotK ?? Number.NaN, AREA));
      expect(place.xPx).toBeLessThan(hrXPx(band?.coolK ?? Number.NaN, AREA));
    }
  });

  it("puts G between the Sun's neighbours on the dwarf scale", () => {
    const g = hrSpectralLetters(AREA).find((place) => place.letter === "G");

    expect(g?.xPx).toBeGreaterThan(hrXPx(5_930, AREA));
    expect(g?.xPx).toBeLessThan(hrXPx(5_270, AREA));
  });
});

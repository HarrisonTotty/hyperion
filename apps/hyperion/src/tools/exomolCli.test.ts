import { describe, expect, it } from "vitest";

import {
  BAND_STRENGTH_FACTOR,
  binTemperatures,
  type CacheRecord,
  EXOMOL_TABLES,
  type HitranBand,
  hitranBands,
  matchBands,
  mergeFailures,
  N2O_SATURATION,
  NH3_SATURATION,
  type SurveyWindow,
  tableBroadening,
  tableSourceOf,
} from "./exomolCli";
import { bandKey, packVibration } from "./exomolLines";
import {
  ammoniaRayleighCm2,
  columnCapCm2,
  nitrousOxideRayleighCm2,
  saturationPressurePa,
} from "./kDistributions";
import type { PartialCell } from "./kDistributionsCli";

/** A quanta string, `v1 v2 l2 v3`, packed. */
function pack(q: string): number {
  const [a = 0, b = 0, c = 0, d = 0] = q.split(" ").map(Number);
  return packVibration(a, b, c, d);
}

/** A band key from two quanta strings, `v1 v2 l2 v3`. */
function key(upper: string, lower: string): number {
  return bandKey(pack(upper), pack(lower));
}

/** A global-quanta field of a `.par` record: 15 characters, right-aligned. */
function quanta(q: string): string {
  return q.padStart(15);
}

/** A HITRAN `.par` record of ¹⁴N₂¹⁶O (molecule 4, local 1) in its fixed columns. */
function parRecord(nu: number, s: number, upper: string, lower: string): string {
  return (
    " 41" +
    nu.toFixed(6).padStart(12) +
    s.toExponential(3).toUpperCase().padStart(10) +
    " 6.933E-07.08800.110   12.57020.71-.009082" +
    quanta(upper) +
    quanta(lower) +
    " ".repeat(63)
  );
}

describe("HITRAN2024's N₂O bands from a .par file", () => {
  it("sum each band's lines by its global quanta, keeping its weakest line and its extent", () => {
    const text = [
      parRecord(13_933, 2e-27, "1 0 0 6", "0 0 0 0"),
      parRecord(13_990, 4e-27, "1 0 0 6", "0 0 0 0"),
      parRecord(13_300, 1e-27, "6 0 0 3", "0 0 0 0"),
    ].join("\n");
    const bands = hitranBands(text);
    expect(
      bands.map((b) => [b.key, b.lines, b.leastIntensityCm, b.fromPerCm, b.toPerCm, b.intensityCm]),
    ).toEqual([
      [key("1 0 0 6", "0 0 0 0"), 2, 2e-27, 13_933, 13_990, expect.closeTo(6e-27, 40)],
      [key("6 0 0 3", "0 0 0 0"), 1, 1e-27, 13_300, 13_300, 1e-27],
    ]);
  });
});

/** A HITRAN band of a strength and centre. */
function band(upper: string, centre: number, strength: number): HitranBand {
  return {
    key: key(upper, "0 0 0 0"),
    intensityCm: strength,
    intensityWavenumber: strength * centre,
    lines: 40,
    leastIntensityCm: 1e-30,
    fromPerCm: centre - 20,
    toPerCm: centre + 20,
  };
}

/** A survey window of one HITRAN band holding TYM bands [upper, centre, strength]. */
function window(
  h: HitranBand,
  tym: ReadonlyArray<readonly [string, number, number]>,
): SurveyWindow {
  return {
    band: h.key,
    fromPerCm: h.fromPerCm,
    toPerCm: h.toPerCm,
    leastIntensityCm: h.leastIntensityCm,
    tym: tym.map(([upper, centre, s]) => [key(upper, "0 0 0 0"), s, s * centre, 30] as const),
  };
}

describe("matching HITRAN2024's bands to TYM's", () => {
  it("matches a TYM band of the same assignment that agrees in centre and strength", () => {
    const h = band("6 0 0 3", 13_290, 1e-24);
    const { matches, apart } = matchBands([h], [window(h, [["6 0 0 3", 13_290.5, 1.4e-24]])]);
    expect([matches[0]?.verdict, matches[0]?.kept, apart]).toEqual([
      "matched",
      "HITRAN2024",
      [h.key],
    ]);
  });

  it("takes TYM's dominant band, doubtful, where TYM assigns none so", () => {
    const h = band("1 0 0 6", 13_990, 1e-24);
    const { matches, apart } = matchBands(
      [h],
      [
        window(h, [
          ["0 20 0 1", 13_990.8, 0.8e-24],
          ["7 0 0 3", 13_985, 1e-26],
        ]),
      ],
    );
    expect([matches[0]?.tymBand, matches[0]?.verdict, apart]).toEqual([
      "0 20 0 1 - 0 0 0 0",
      "doubtful",
      [key("0 20 0 1", "0 0 0 0")],
    ]);
  });

  it("is doubtful past a factor of 3 in strength, and leaves out the same-assignment band too", () => {
    const h = band("2 4 0 4", 13_360, 1e-24);
    const weak = 0.9 / BAND_STRENGTH_FACTOR;
    const { matches, apart } = matchBands([h], [window(h, [["2 4 0 4", 13_360, weak * 1e-24]])]);
    expect([matches[0]?.verdict, apart]).toEqual(["doubtful", [h.key]]);
  });
});

/** A partial cell with merge rows [T, p, mean, HITRAN's, TYM's]. */
function cell(rows: ReadonlyArray<readonly [number, number, number, number, number]>): PartialCell {
  return {
    nm: 716,
    w: [1],
    o: 0,
    s: 1,
    k: "",
    worstNodeError: 0,
    worstChain: { error: 0, temperatureK: 0, pressurePa: 0, columnCm2: 0 },
    worstBeyondCap: { error: 0, temperatureK: 0, pressurePa: 0, columnCm2: 0 },
    failures: [],
    measuredShapeTo: [],
    beyondCutoffShare: [],
    nearWingShare: [],
    quantisation: 0,
    belowGrid: 0,
    merge: rows,
  };
}

describe("the merged cells' means", () => {
  it("pass where each equals its two lists' sum to 10⁻⁹, and fail past it", () => {
    expect(mergeFailures([cell([[100, 1e3, 3e-30, 1e-30, 2e-30]])])).toEqual([]);
    expect(
      mergeFailures([cell([[100, 1e3, 3.000_01e-30, 1e-30, 2e-30]])]).map((f) => f.nm),
    ).toEqual([716]);
  });
});

describe("the grids and the records", () => {
  it("bin at the nodes and at the midpoints between them", () => {
    expect(binTemperatures([100, 150, 200])).toEqual([100, 125, 150, 175, 200]);
  });

  it("make 300 K a node of NH₃'s grid and run N₂O's over 100–500 K", () => {
    expect([
      EXOMOL_TABLES["NH3"]?.temperaturesK.at(-1),
      EXOMOL_TABLES["N2O"]?.temperaturesK,
    ]).toEqual([300, [100, 150, 200, 250, 300, 350, 400, 450, 500]]);
  });

  it("take a list's .broad ratios to its default air width, and their exponents' offsets", () => {
    const table = EXOMOL_TABLES["NH3"];
    if (table === undefined) {
      throw new Error("NH3 has a table");
    }
    const record: CacheRecord = {
      species: "NH3",
      definition: {
        molecularMassDa: 17,
        airHalfWidthPerCmBar: 0.05,
        airExponent: 0.5,
        broadeners: {},
      },
      temperatures: {},
      partitionSum296: 1,
      cellMeans296M2: [],
      intensity296Cm: 2,
      broadening: [
        { gas: "H2", intensityHalfWidthPerBar: 0.16, intensityHalfWidthExponentPerBar: 0.096 },
      ],
      bands: [],
      bandExtents: [],
      apart: [],
      passes: [],
    };
    // Σ S γ ÷ (γ_air Σ S) = 0.16 ÷ (0.05 × 2); n̄ = 0.096 ÷ 0.16 = 0.6, less n_air's 0.5.
    const { ratios, exponents } = tableBroadening(table, record, undefined);
    expect([ratios["H2"], exponents["H2"]]).toEqual([
      expect.closeTo(1.6, 12),
      expect.closeTo(0.1, 12),
    ]);
  });

  it("name each table's inputs and its licence in its sidecar's records", () => {
    for (const table of Object.values(EXOMOL_TABLES)) {
      const source = tableSourceOf(table);
      expect(source.terms).toMatch(/CC BY-SA 4\.0/u);
      expect(source.inputs.split(", ").length).toBeGreaterThanOrEqual(9);
      expect(source.description).toMatch(/CC BY-SA 4\.0/u);
    }
  });
});

/** A tenth of a value's leading place: the unit of its second significant figure. */
function secondPlace(x: number): number {
  return 10 ** (Math.floor(Math.log10(x)) - 1);
}

describe("the tables' Rayleigh cross-sections", () => {
  it("are N₂O's and NH₃'s own, whose columns the between-node test also stops at", () => {
    expect([EXOMOL_TABLES["N2O"]?.rayleighCm2, EXOMOL_TABLES["NH3"]?.rayleighCm2]).toEqual([
      nitrousOxideRayleighCm2,
      ammoniaRayleighCm2,
    ]);
  });
});

describe("N₂O's saturation (from its triple point, as O₂'s)", () => {
  // The ruling's p_sat and caps, 400 p_sat ÷ (m g) at 0.5 m s⁻², each to half its last place.
  it.each([
    [100, 0.31, 3.4e23],
    [150, 3_300, 3.6e27],
    [175, 46_000, 5.1e28],
    [225, 700_000, 7.6e29],
  ])("is %d K's %f Pa, capping the columns at %f cm⁻²", (t, pa, capCm2) => {
    expect([
      Math.abs(saturationPressurePa(N2O_SATURATION, t) - pa) <= secondPlace(pa) / 2,
      Math.abs(columnCapCm2(N2O_SATURATION, t, 1e7, 44.0128) - capCm2) <= secondPlace(capCm2) / 2,
    ]).toEqual([true, true]);
  });

  it("has none at or above its critical point, 309.52 K", () => {
    expect([
      saturationPressurePa(N2O_SATURATION, 309.52),
      Number.isFinite(saturationPressurePa(N2O_SATURATION, 309.5)),
    ]).toEqual([Infinity, true]);
  });
});

describe("NH₃'s saturation (Fray and Schmitt 2009)", () => {
  it.each([
    [100, 5.745e-5],
    [150, 17.79],
    [195, 5_874],
  ])("is %d K's %f Pa", (t, pa) => {
    expect(saturationPressurePa(NH3_SATURATION, t) / pa).toBeCloseTo(1, 3);
  });

  it("joins the solid's fit at the triple point, and rises above it", () => {
    const at = saturationPressurePa(NH3_SATURATION, 195.41);
    expect([
      saturationPressurePa(NH3_SATURATION, 195.410_001) / at,
      saturationPressurePa(NH3_SATURATION, 300) > 1e6,
    ]).toEqual([expect.closeTo(1, 6), true]);
  });
});

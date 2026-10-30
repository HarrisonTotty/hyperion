import { describe, expect, it } from "vitest";

import { ladderFinestStepK } from "./atomicKDistributions";
import { THINNED_PRESSURES_PA } from "./atomicKDistributionsCli";
import {
  BAND_STRENGTH_FACTOR,
  binTemperatures,
  type CacheRecord,
  cellsWithLines,
  EXOMOL_TABLES,
  gridOfTable,
  type HitranBand,
  hitranBands,
  HOT_TABLES,
  type HotTable,
  matchBands,
  mergeFailures,
  N2O_SATURATION,
  NH3_SATURATION,
  reductionSourceOf,
  rulesOf,
  sampleCellsOf,
  type SurveyWindow,
  tableBroadening,
  tableLists,
  tableSourceOf,
  withReferences,
} from "./exomolCli";
import { bandKey, type ExoMolFile, packVibration, partitionFileOf } from "./exomolLines";
import {
  ammoniaRayleighCm2,
  chainRowsPa,
  columnCapCm2,
  nitrousOxideRayleighCm2,
  raisedFirstNodeChecksPa,
  saturationPressurePa,
  STORED_PRESSURES_PA,
} from "./kDistributions";
import { FINEST_STEP_K, type PartialCell } from "./kDistributionsCli";

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
      // Every file of every list the table takes, its isotopologues' among them, with its SHA-256.
      const files: ExoMolFile[] = [];
      for (const list of tableLists(table)) {
        files.push(
          list.def,
          ...(list.kind === "exomol"
            ? [list.states, ...list.transitions, ...list.broadeners]
            : [list.lines]),
        );
        const pf = partitionFileOf(list);
        if (pf !== undefined) {
          files.push(pf);
        }
      }
      expect(source.terms).toMatch(/CC BY-SA 4\.0/u);
      expect(files.filter((f) => !source.inputs.includes(`${f.name} ${f.sha256}`))).toEqual([]);
      // Each list names three files at least: its definition and its states and a transition
      // file, or its definition, its REPACK file and its .pf.
      expect(files.length).toBeGreaterThanOrEqual(3 * tableLists(table).length);
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

/** A hot table by its key, or a test failure. */
function hot(species: string): HotTable {
  const table = HOT_TABLES[species];
  if (table === undefined) {
    throw new Error(`no hot table ${species}`);
  }
  return table;
}

/** A definition of a toy list, for the reduction's source. */
const TOY_DEFINITION = {
  molecularMassDa: 64,
  airHalfWidthPerCmBar: 0.07,
  airExponent: 0.5,
  broadeners: {},
};

describe("R08.T4.f's hot tables (decision-r08-t4ef-storage §1.4; decision-r08-t4e-budget §1.4)", () => {
  it.each([
    ["TiO", 1_000, 4_000],
    ["FeH", 1_000, 4_000],
    ["SiO", 1_000, 4_000],
    ["N2", 1_800, 4_000],
    ["H2S", 296, 1_500],
    ["CO", 300, 4_000],
    ["HCN", 300, 4_000],
    ["H2O", 300, 4_000],
    ["VO", 1_000, 4_000],
  ] as const)("run %s from its floor or hand-over, %i K, to %i K (E2)", (species, first, last) => {
    const nodes = hot(species).temperaturesK;
    expect([nodes[0], nodes.at(-1)]).toEqual([first, last]);
  });

  it("measure VO to 3,000 K, its last comparison, its nodes running on to 4,000 K", () => {
    expect([hot("VO").measuredRangeK, hot("VO").temperaturesK.at(-1)]).toEqual([
      [1_000, 3_000],
      4_000,
    ]);
  });

  it("take E1′'s 18 nodes over 1,000–4,000 K", () => {
    expect(hot("TiO").temperaturesK).toEqual([
      1_000, 1_100, 1_200, 1_300, 1_400, 1_500, 1_650, 1_800, 1_950, 2_100, 2_300, 2_500, 2_750,
      3_000, 3_300, 3_600, 3_950, 4_000,
    ]);
  });

  it("bin TiO's five Ti isotopologues together, each at its own abundance", () => {
    const lists = tableLists(hot("TiO"));
    const share = lists.reduce((sum, list) => sum + list.abundance, 0);
    // The Ti fractions sum to 1, so the five take ¹⁶O's share of TiO.
    expect([lists.length, share]).toEqual([5, expect.closeTo(0.997_57, 9)]);
  });

  it("bin isotopologues together only beside a list's own files, never a REPACK file", () => {
    const shared = Object.values(HOT_TABLES).filter((t) => t.isotopologues.length > 0);
    expect(shared.filter((t) => t.exomol.kind !== "exomol").map((t) => t.exomol.species)).toEqual(
      [],
    );
  });

  it("take each list's Q(T) from its .pf", () => {
    const lists = Object.values(HOT_TABLES).flatMap(tableLists);
    expect(lists.filter((list) => list.partition.from !== "pf").map((l) => l.species)).toEqual([]);
  });

  it("scale CO's Li2015 lines by the stated hand-over ratio, and only CO's", () => {
    const scaled = Object.values(HOT_TABLES).filter((t) => t.exomol.intensityScale !== undefined);
    expect([scaled.map((t) => t.exomol.species), hot("CO").exomol.intensityScale?.factor]).toEqual([
      ["CO"],
      2.05,
    ]);
  });

  it("subtract the pedestal from H₂O's lines alone, R08.T4.d's H₂O table's convention", () => {
    expect(
      Object.entries(HOT_TABLES)
        .filter(([, table]) => table.policy.pedestal)
        .map(([species]) => species),
    ).toEqual(["H2O"]);
  });

  it("name POKAZATEL's 133 transition files of 100 cm⁻¹, end to end over the binned range", () => {
    const source = hot("H2O").exomol;
    const files = source.kind === "exomol" ? source.transitions : [];
    const gaps = files.filter((f, i) => i > 0 && f.fromPerCm !== files[i - 1]?.toPerCm);
    expect([
      files.length,
      files[0]?.fromPerCm,
      files.at(-1)?.toPerCm,
      gaps.length,
      source.url,
    ]).toEqual([133, 13_100, 26_400, 0, "https://www.exomol.com/db/H2O/1H2-16O/POKAZATEL/"]);
  });

  it("refine to a quarter of E1's step, R08.T4.g's tables to 12.5 K", () => {
    const nh3 = EXOMOL_TABLES["NH3"];
    if (nh3 === undefined) {
      throw new Error("NH3 has a table");
    }
    expect([rulesOf(hot("TiO")).finestStepK(1_500), rulesOf(nh3).finestStepK(150)]).toEqual([
      ladderFinestStepK(1_500),
      FINEST_STEP_K,
    ]);
  });
});

describe("a hot table's grid (E3)", () => {
  it("takes E3's thinned pressures, read at their rows", () => {
    const grid = gridOfTable(hot("TiO"), undefined);
    expect([grid.pressuresPa, grid.rowsPa]).toEqual([
      THINNED_PRESSURES_PA,
      chainRowsPa(THINNED_PRESSURES_PA),
    ]);
  });

  it("takes the pressures a run gives, the alternates the merge kept among them", () => {
    const pressures = [...THINNED_PRESSURES_PA, 177_827.941_003_892_28].toSorted((a, b) => a - b);
    expect(gridOfTable(hot("TiO"), undefined, pressures).pressuresPa).toEqual(pressures);
  });

  it("keeps R08.T4.g's tables on the 30 standard pressures", () => {
    const nh3 = EXOMOL_TABLES["NH3"];
    if (nh3 === undefined) {
      throw new Error("NH3 has a table");
    }
    expect(gridOfTable(nh3, undefined).pressuresPa).toEqual(STORED_PRESSURES_PA);
  });

  it("refuses pressures given to an R08.T4.g table", () => {
    const nh3 = EXOMOL_TABLES["NH3"];
    if (nh3 === undefined) {
      throw new Error("NH3 has a table");
    }
    expect(() => gridOfTable(nh3, undefined, THINNED_PRESSURES_PA)).toThrow("30 standard");
  });

  it("checks a raised first node's hold below it (E4)", () => {
    const raised = gridOfTable(hot("TiO"), undefined, THINNED_PRESSURES_PA.slice(2));
    expect(reductionSourceOf(hot("TiO"), TOY_DEFINITION, raised).belowGridChecksPa).toEqual(
      raisedFirstNodeChecksPa(100),
    );
  });

  it("checks no raised hold on a table from 10 Pa", () => {
    const grid = gridOfTable(hot("TiO"), undefined);
    expect(reductionSourceOf(hot("TiO"), TOY_DEFINITION, grid).belowGridChecksPa).toBeUndefined();
  });
});

describe("the projection's samples", () => {
  /** A cache record whose direct sums light 600 and 650 nm at 1,000 K. */
  const record = (means: ReadonlyArray<number>): CacheRecord => ({
    species: "TiO",
    definition: TOY_DEFINITION,
    temperatures: { "1000": { partitionSum: 1, cellMeansM2: means } },
    partitionSum296: 1,
    cellMeans296M2: [],
    intensity296Cm: 1,
    broadening: [],
    bands: [],
    bandExtents: [],
    apart: [],
    passes: [],
  });
  const means = Array.from({ length: 381 }, (_, c) => (c === 220 ? 2e-20 : c === 270 ? 1e-20 : 0));

  it("take every cell a line's wing reaches: each lit cell and its neighbours", () => {
    expect(cellsWithLines(hot("TiO"), record(means))).toEqual([599, 600, 601, 649, 650, 651]);
  });

  it("take the strongest cell, a median one and the strongest's weaker neighbour", () => {
    const lit = [599, 600, 601, 649, 650, 651];
    // Ranked 600, 650, 601, 649, 651, 599: the median the fourth, 649; 600's weaker neighbour 599.
    const shaded = means.map((m, c) =>
      c === 221 ? 5e-21 : c === 219 ? 1e-21 : c === 269 ? 3e-21 : c === 271 ? 2e-21 : m,
    );
    expect(sampleCellsOf(lit, shaded)).toEqual({ strongest: 600, median: 649, edge: 599 });
  });
});

describe("a file of several species' references (exomol-cell-references --into)", () => {
  const file = withReferences(
    withReferences(undefined, "FeH", '{"species":"FeH","cells":[1]}'),
    "TiO",
    '{"species":"TiO","cells":[2]}',
  );

  it("adds a species' entry after those it holds", () => {
    expect(file).toBe(
      '{"references":[{"species":"FeH","cells":[1]},{"species":"TiO","cells":[2]}]}\n',
    );
  });

  it("replaces a species' entry, keeping the others in their order", () => {
    expect(withReferences(file, "FeH", '{"species":"FeH","cells":[3]}')).toBe(
      '{"references":[{"species":"TiO","cells":[2]},{"species":"FeH","cells":[3]}]}\n',
    );
  });

  it("refuses a file that is not one of several species' references", () => {
    expect(() => withReferences('{"species":"FeH"}', "TiO", "{}")).toThrow("several species");
  });
});

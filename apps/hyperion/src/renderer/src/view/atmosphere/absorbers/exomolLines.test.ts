import { beforeAll, describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import { earthLikeColumn, planckBakeSpectrum } from "../../../test/atmosphereFixtures";
import { inlinePart } from "../../../test/kDistributionFixtures";
import {
  ABSORBER_TRANSMITTANCE_FLOOR,
  absorberCurve,
  absorberSpecs,
  type ColumnAbsorberSpec,
} from "../absorbers";
import { BAKE_RANGE_NM } from "../medium";
import {
  type BakeSpectrum,
  bakeSpectrumAt,
  bakeSpectrumOf,
  matchingFunctionsAt,
} from "../spectralColour";
import merge from "./exomol/n2oMerge.json" with { type: "json" };
import {
  cellMeanSigma,
  committedLineSpecies,
  type KDistributionTable,
  kDistributionAt,
  loadKDistributions,
} from "./kDistribution";

/** The committed sidecars' paths, R08.T4.d's directory and ExoMol's, as the loader globs them. */
const HITRAN_SIDECARS = import.meta.glob("./kDistributions/*.json");
const EXOMOL_SIDECARS = import.meta.glob("./exomol/kDistributions/*.json");

/** The Sun: R06's `solar_colour()`, the 5,772 K main-sequence row of `bake_spectra.json`. */
const THE_SUN: BakeSpectrum = (() => {
  const row = bakeSpectra.suns.find((s) => s.teff_k === 5_772 && s.grid === "MainSequence");
  if (row === undefined) {
    throw new Error("bake_spectra.json holds no 5,772 K main-sequence sun");
  }
  return bakeSpectrumOf(row.bake_spectrum);
})();

/** T4.b's test suns: the Sun and the 2,500 K and 30,000 K Planck suns. */
const SUNS: ReadonlyArray<readonly [string, BakeSpectrum]> = [
  ["the Sun", THE_SUN],
  ["a 2,500 K Planck sun", planckBakeSpectrum(2_500)],
  ["a 30,000 K Planck sun", planckBakeSpectrum(30_000)],
];

/**
 * NH₃'s saturation pressure, Pa, below its triple point: N. Fray and B. Schmitt, Planet. Space
 * Sci. 57 (2009) 2053, Eq. 4 and Table 5 (row NH3-1), ln(p ÷ 1 bar) = Σ Aᵢ ÷ Tⁱ, fitted over
 * 15–195.41 K; the reduction's cap takes the same fit (`src/tools/exomolCli.ts`).
 */
function ammoniaSaturationPa(temperatureK: number): number {
  const a = [15.96, -3_537, -33_100, 1.742e6, -2.995e7];
  return 1e5 * Math.exp(a.reduce((sum, ai, i) => sum + ai / temperatureK ** i, 0));
}

/** NH₃'s molecular mass, kg: 17.026 549 Da (its `def.json`, HITRAN's) × CODATA 2018's m_u. */
const AMMONIA_MASS_KG = 17.026_549 * 1.660_539_066_6e-27;

/**
 * The saturated column of a pure vapour over a grazing path, m⁻²: 40 p_sat ÷ (m g) at
 * g = 0.5 m s⁻² (`decision-r08-t4d-kdist.md` §3.1).
 */
function saturatedColumnPerM2(temperatureK: number): number {
  return (40 * ammoniaSaturationPa(temperatureK)) / (AMMONIA_MASS_KG * 0.5);
}

/** Each 1 nm cell's channel weights c̄_c S over 380–760 nm for a sun, by T4.b's weighting rule. */
function cellWeights(sun: BakeSpectrum): ReadonlyArray<Float64Array> {
  const [low, high] = BAKE_RANGE_NM;
  const cells = high - low + 1;
  const weights = [new Float64Array(cells), new Float64Array(cells), new Float64Array(cells)];
  for (let c = 0; c < cells; c += 1) {
    for (let j = 0; j < 20; j += 1) {
      const nm = low + c - 0.5 + (j + 0.5) / 20;
      const s = bakeSpectrumAt(sun, Math.min(Math.max(nm, low), high)) / 20;
      const m = matchingFunctionsAt(Math.min(Math.max(nm, low), high));
      for (const [k, row] of weights.entries()) {
        row[c] = (row[c] ?? 0) + (m[k] ?? 0) * s;
      }
    }
  }
  return weights;
}

/**
 * R08.T10.a's Δ_c, the most a pending species' cells could move a channel's light, at k = 100
 * (follow-up 4, ruling 3): Σ |c̄_c| S min(1, k ⟨σ⟩ u) ÷ Σ c̄_c S, the largest over the channels.
 */
function largestChange(
  weights: ReadonlyArray<Float64Array>,
  cellMeansM2: Float64Array,
  columnPerM2: number,
): number {
  return Math.max(
    ...weights.map((row) => {
      let changed = 0;
      let total = 0;
      for (let c = 0; c < row.length; c += 1) {
        const w = row[c] ?? 0;
        changed += Math.abs(w) * Math.min(1, 100 * (cellMeansM2[c] ?? 0) * columnPerM2);
        total += w;
      }
      return changed / total;
    }),
  );
}

/** A table's cell means at a temperature, m², on 380–760 nm, from its 10 Pa row. */
function cellMeansAt(table: KDistributionTable, temperatureK: number): Float64Array {
  const kd = kDistributionAt(table, temperatureK);
  const out = new Float64Array(BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1);
  for (const cell of kd.cells) {
    const g = cell.weights.length;
    out[cell.cellNm - BAKE_RANGE_NM[0]] = cellMeanSigma(cell.weights, cell.sigmas.slice(0, g));
  }
  return out;
}

let ammonia: KDistributionTable | undefined;
let nitrousOxide: KDistributionTable | undefined;

beforeAll(async () => {
  [ammonia, nitrousOxide] = await loadKDistributions(["NH3", "N2O"], inlinePart);
}, 120_000);

/** A loaded table, or a test failure. */
function loaded(table: KDistributionTable | undefined): KDistributionTable {
  if (table === undefined) {
    throw new Error("the committed table did not load");
  }
  return table;
}

describe("NH₃'s table from C2018 (R08.T4.g)", () => {
  it("starts where a pure vapour's saturated column is negligible, Δ_c under 10⁻³ at k = 100", () => {
    const table = loaded(ammonia);
    const first = table.temperaturesK[0] ?? Number.NaN;
    for (const [, sun] of SUNS) {
      expect(
        largestChange(cellWeights(sun), cellMeansAt(table, first), saturatedColumnPerM2(first)),
      ).toBeLessThan(1e-3);
    }
  });

  it("finds T_min, where Δ_c at k = 100 reaches 10⁻³, within its first interval", () => {
    // The nodes run down from 300 K in 50 K steps to the first at or below T_min, so T_min lies
    // within the first 50 K: the plan records it (103.9 K, the least over the three suns).
    const table = loaded(ammonia);
    const first = table.temperaturesK[0] ?? Number.NaN;
    const crossings = SUNS.map(([, sun]) => {
      const weights = cellWeights(sun);
      let lo = first;
      let hi = table.temperaturesK.at(-1) ?? Number.NaN;
      // 20 halvings of the 200 K bracket: 2 × 10⁻⁴ K.
      for (let k = 0; k < 20; k += 1) {
        const mid = 0.5 * (lo + hi);
        if (largestChange(weights, cellMeansAt(table, mid), saturatedColumnPerM2(mid)) < 1e-3) {
          lo = mid;
        } else {
          hi = mid;
        }
      }
      return lo;
    });
    const least = Math.min(...crossings);
    expect([least > first, least < first + 50]).toEqual([true, true]);
  }, 60_000);

  it("runs to C2018's 300 K, a node, measured there and held beyond", () => {
    const table = loaded(ammonia);
    expect([table.temperaturesK.at(-1), table.measuredRangeK[1]]).toEqual([300, 300]);
    expect(kDistributionAt(table, 350).provenance).toBe("estimated");
  });

  it("estimates every node's line shape, its air widths C2018's default", () => {
    expect(loaded(ammonia).measuredShapeTo.every((p) => p === -1)).toBe(true);
  });

  it("gives H₂ and He their widths from C2018's .broad files, and none for self", () => {
    const table = loaded(ammonia);
    expect([
      table.broadening.has("H2"),
      table.broadening.has("He"),
      table.broadening.has("self"),
    ]).toEqual([true, true, false]);
  });
});

/** The record's rows, typed as the tool writes them. */
interface MergeRecord {
  readonly bands: ReadonlyArray<{
    readonly band: string;
    readonly tymBand: string;
    readonly kept: string;
    readonly verdict: string;
    readonly strengthRatio: number;
    readonly centreOffsetPerCm: number;
  }>;
  readonly cells: ReadonlyArray<{
    readonly nm: number;
    readonly hitranBands: ReadonlyArray<string>;
    readonly tymBands: ReadonlyArray<string>;
    readonly tymWeakerBands: number;
    readonly means: ReadonlyArray<ReadonlyArray<number>>;
  }>;
}

const MERGE: MergeRecord = merge;

describe("N₂O's merged table (R08.T4.g, decision-r08-t4d-kdist-chain.md §11)", () => {
  it("takes each 714–760 nm cell's mean as HITRAN2024's plus its added TYM bands', to 10⁻⁹", () => {
    const cells = MERGE.cells.filter((c) => c.nm >= 714 && c.nm <= 760);
    let worst = 0;
    for (const cell of cells) {
      for (const [, , mean = 0, hitran = 0, tym = 0] of cell.means) {
        worst = Math.max(worst, Math.abs(mean - (hitran + tym)) / mean);
      }
    }
    expect([cells.length, worst <= 1e-9]).toEqual([47, true]);
  });

  it("takes no band from both lists in any cell", () => {
    expect(
      MERGE.cells.filter((c) => c.hitranBands.some((b) => c.tymBands.includes(b))).map((c) => c.nm),
    ).toEqual([]);
  });

  it("names each cell's added TYM bands at HITRAN2024's cut-off, and counts the weaker", () => {
    expect(
      MERGE.cells.filter((c) => c.tymBands.length === 0 || c.tymWeakerBands <= 0).map((c) => c.nm),
    ).toEqual([]);
  });

  it("leaves out from 714 nm every TYM band matched to one of HITRAN2024's", () => {
    const matched = new Set(MERGE.bands.map((b) => b.tymBand));
    expect(
      MERGE.cells.filter((c) => c.tymBands.some((b) => matched.has(b))).map((c) => c.nm),
    ).toEqual([]);
  });

  it("keeps HITRAN2024's four bands, strongest first, each matched to a TYM band", () => {
    expect(MERGE.bands.map((b) => [b.band, b.kept, b.verdict])).toEqual([
      ["1 0 0 6 - 0 0 0 0", "HITRAN2024", "doubtful"],
      ["6 0 0 3 - 0 0 0 0", "HITRAN2024", "matched"],
      ["2 0 0 5 - 0 0 0 0", "HITRAN2024", "doubtful"],
      ["2 4 0 4 - 0 0 0 0", "HITRAN2024", "doubtful"],
    ]);
  });

  it("matches each band within the recorded tolerances: a factor of 3 in strength, 2 cm⁻¹ in centre", () => {
    expect(
      MERGE.bands.filter(
        (b) =>
          !(b.strengthRatio >= 1 / 3 && b.strengthRatio <= 3 && Math.abs(b.centreOffsetPerCm) <= 2),
      ),
    ).toEqual([]);
  });

  it("runs 100–500 K, measured to 296 K", () => {
    const table = loaded(nitrousOxide);
    expect([table.temperaturesK[0], table.temperaturesK.at(-1), table.measuredRangeK]).toEqual([
      100,
      500,
      [100, 296],
    ]);
  });

  it("is one table, under exomol/, R08.T4.d's HITRAN-only table gone", () => {
    expect([
      committedLineSpecies().filter((k) => k === "N2O").length,
      Object.keys(HITRAN_SIDECARS).filter((p) => p.endsWith("/N2O.json")),
      Object.keys(EXOMOL_SIDECARS).filter((p) => p.endsWith("/N2O.json")),
    ]).toEqual([1, [], ["./exomol/kDistributions/N2O.json"]]);
  });
});

/** A species' layer over the whole of an Earth-like column, with a column, m⁻². */
function wellMixed(species: string, columnPerM2: number) {
  return { species, columnPerM2, basePa: 101_325, topPa: 0 };
}

describe.each([
  ["NH₃", "NH3", () => loaded(ammonia)],
  ["N₂O", "N2O", () => loaded(nitrousOxide)],
])("%s's curves", (_, species, table) => {
  it.each([
    ["a 2,500 K", planckBakeSpectrum(2_500)],
    ["a 30,000 K", planckBakeSpectrum(30_000)],
  ])("lie in [10⁻⁶, 1] and are non-increasing along each row under %s Planck sun", (__, sun) => {
    const specs = absorberSpecs([wellMixed(species, 1e26)], earthLikeColumn(), [], [], {
      kDistributions: [table()],
    });
    const spec = specs.specs.find((s): s is ColumnAbsorberSpec => s.kind === "column");
    if (spec === undefined) {
      throw new Error("no column spec was formed");
    }
    const curve = absorberCurve(spec, sun);
    const nodes = curve.logColumns.length;
    let least = Infinity;
    let most = -Infinity;
    let rise = 0;
    for (const channel of curve.transmittance) {
      for (let r = 0; r < curve.logPressures.length; r += 1) {
        for (let j = 0; j < nodes; j += 1) {
          const t = channel[r * nodes + j] ?? Number.NaN;
          least = Math.min(least, t);
          most = Math.max(most, t);
          rise = Math.max(rise, j > 0 ? t - (channel[r * nodes + j - 1] ?? Number.NaN) : 0);
        }
      }
    }
    expect([least >= ABSORBER_TRANSMITTANCE_FLOOR, most <= 1, rise]).toEqual([true, true, 0]);
  });
});

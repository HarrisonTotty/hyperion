import { describe, expect, it } from "vitest";

import {
  ABO_MATCH_NM,
  type AboLine,
  aboWidth,
  allardH2WidthM3PerS,
  atomicBroadeningRatios,
  type AtomicLineSet,
  atomicLineSet,
  atomicLineStates,
  atomicSpectra,
  BALMER_SELF_BROADENING,
  FIXED_GRID_PA,
  gammaFunction,
  ladderFinestStepK,
  ladderTemperatures,
  NA_D_H2_WIDTHS,
  parseAboTable,
  perturberScale,
  selfBroadeningSource,
  vanDerWaalsWidthM3PerS,
} from "./atomicKDistributions";
import { type KuruczLine } from "./atomicLines";
import { lineAt } from "./atomicLinesToys";

/** A toy stage's line set: Fe I's mass and limit unless given, Q 25 at every temperature. */
function toySet(
  lines: ReadonlyArray<KuruczLine>,
  key = "Fe",
  limitEv = 7.902,
  abo: ReadonlyArray<AboLine> = [],
): AtomicLineSet {
  const [symbol = "", ion] = key.split("+");
  return atomicLineSet(
    { key, symbol, stage: ion === undefined ? 0 : 1 },
    26,
    55.845,
    limitEv,
    lines,
    abo,
    () => 25,
  );
}

/** A perturber's reduced mass with Fe, Da. */
function withIron(massDa: number): number {
  return (55.845 * massDa) / (55.845 + massDa);
}

/** Na D2's width per H₂ at a temperature from Allard et al.'s tables. */
function d2At(temperatureK: number): number {
  return allardH2WidthM3PerS(NA_D_H2_WIDTHS.d2HwhmPerCm, temperatureK);
}

/**
 * A record of Barklem et al.'s ABO table in its ReadMe's columns (spec 1–5, λ 7–16 Å, J 18–21 and
 * 23–26, logGam 105–110, Texp 112–116), its values made up but for Fe I 2300.142 Å's log Γ.
 */
function aboRecord(logGamma = "-7.812"): string {
  return (
    "26.00".padEnd(5) +
    " " +
    "2300.142".padStart(10) +
    " " +
    "4.0".padStart(4) +
    " " +
    "5.0".padStart(4) +
    " ".repeat(104 - 26) +
    logGamma.padStart(6) +
    " " +
    "0.374".padStart(5)
  );
}

/** An ABO line of Fe I at an air wavelength, nm, and its log Γ. */
function aboAt(airNm: number, logGammaPerHydrogen: number, lowerJ = 0.5): AboLine {
  return {
    spectrumCode: 26,
    airNm,
    lowerJ,
    upperJ: 1.5,
    logGammaPerHydrogen,
    temperatureExponent: 0.38,
  };
}

describe("R08.T4.e's temperature ladders", () => {
  it("run E1's 50 K to 1,000 K, 100 K to 2,000 K and 200 K beyond: 26 nodes from 1,000 K, 36 from 500 K", () => {
    expect([ladderTemperatures(1_000).length, ladderTemperatures(500).length]).toEqual([26, 36]);
  });

  it("run E1′'s steps, the largest multiple of 50 K at most 10% of the lower node: 20 nodes from 1,000 K", () => {
    // decision-r08-t4e-budget §3's list.
    expect(ladderTemperatures(1_000, 5_000, "E1prime")).toEqual([
      1_000, 1_100, 1_200, 1_300, 1_400, 1_500, 1_650, 1_800, 1_950, 2_100, 2_300, 2_500, 2_750,
      3_000, 3_300, 3_600, 3_950, 4_300, 4_700, 5_000,
    ]);
  });

  it("run E1′ to 30 nodes from 500 K", () => {
    expect(ladderTemperatures(500, 5_000, "E1prime").length).toBe(30);
  });

  it("agree to 1,500 K", () => {
    const e1 = ladderTemperatures(500).filter((t) => t <= 1_500);
    expect(ladderTemperatures(500, 5_000, "E1prime").filter((t) => t <= 1_500)).toEqual(e1);
  });

  it("refine to a quarter of E1's step: 12.5 K below 1,000 K, 25 K to 2,000 K, 50 K beyond", () => {
    expect([500, 1_000, 1_950, 2_000, 4_700].map(ladderFinestStepK)).toEqual([
      12.5, 25, 25, 50, 50,
    ]);
  });
});

describe("Barklem et al.'s ABO table", () => {
  it("reads a record's spectrum, air wavelength, J's, log Γ and exponent", () => {
    expect(parseAboTable(`${aboRecord()}\n`)).toEqual([
      {
        spectrumCode: 26,
        airNm: 230.0142,
        lowerJ: 4,
        upperJ: 5,
        logGammaPerHydrogen: -7.812,
        temperatureExponent: 0.374,
      },
    ]);
  });

  it("refuses a record whose field is not a number", () => {
    expect(() => parseAboTable(aboRecord("  abc"))).toThrow("log Gamma");
  });
});

describe("a line's van der Waals source", () => {
  const abo = [aboAt(500.0015, -7.6), aboAt(500.0005, -7.4), aboAt(510, -7.2, 1.5)];
  const kurucz = { ...lineAt(510, -1, 0), logGammaVanDerWaalsPerHydrogen: -7.3 };
  const set = toySet([lineAt(500, -1, 0), kurucz, lineAt(520, -1, 0)], "Fe", 7.902, abo);
  const sourceAt = (nm: number): AtomicLineSet["vanDerWaals"][number] | undefined =>
    set.vanDerWaals[set.lines.findIndex((l) => l.listedNm === nm)];

  it("matches an ABO line within 0.02 Å", () => {
    expect(ABO_MATCH_NM).toBe(0.002);
  });

  it("is the nearest ABO line of the same J's within the match", () => {
    expect(sourceAt(500)).toEqual({ kind: "abo", logGammaPerHydrogen: -7.4, exponent: 0.38 });
  });

  it("is Kurucz's field where no ABO line of the same J's matches", () => {
    expect(sourceAt(510)).toEqual({ kind: "kurucz", logGammaPerHydrogen: -7.3 });
  });

  it("is Unsöld's where neither gives one", () => {
    expect(sourceAt(520)).toEqual({ kind: "unsold" });
  });
});

describe("the atoms' spectra", () => {
  const set = toySet([lineAt(500.0, 0, 0), lineAt(500.3, -1, 8_000), lineAt(499.7, -2, 0)]);

  it("take every pressure's points on the grid their lines take at 10 Pa, so that the cut-off keeps its place", () => {
    const spectra = atomicSpectra(set);
    const points = [FIXED_GRID_PA, 1e3, 1e5, 1e7].map(
      (p) => spectra(500, 2_000, p, [1])?.node.sorted.length,
    );
    expect(new Set(points).size).toBe(1);
  });

  it("widen with pressure, the Lorentz half-width radiative plus H₂'s van der Waals", () => {
    const low = atomicLineStates(set, [0], 2_000, 10)[0]?.lorentzPerCm ?? Number.NaN;
    const high = atomicLineStates(set, [0], 2_000, 1e6)[0]?.lorentzPerCm ?? Number.NaN;
    expect(high / low).toBeGreaterThan(100);
  });
});

describe("the broadening widths", () => {
  it("convert an ABO cross-section to Barklem et al.'s width per H atom at 10,000 K", () => {
    // Barklem, Piskunov and O'Mara 2000's Table 1 (CDS J/A+AS/142/467): Fe I 2300.142 Å, σ 211,
    // α 0.251, log Γ −7.812; Na I 5889.950 Å, σ 407, α 0.273, log Γ −7.526.
    const muFeH = (55.845 * 1.007_94) / (55.845 + 1.007_94);
    const muNaH = (22.989_77 * 1.007_94) / (22.989_77 + 1.007_94);
    expect([
      aboWidth(211, 0.251, muFeH).logGammaPerHydrogen,
      aboWidth(407, 0.273, muNaH).logGammaPerHydrogen,
      aboWidth(407, 0.273, muNaH).exponent,
    ]).toEqual([expect.closeTo(-7.812, 2), expect.closeTo(-7.526, 2), expect.closeTo(0.3635, 9)]);
  });

  it("take Γ by Lanczos's series", () => {
    expect([gammaFunction(1), gammaFunction(1.5), gammaFunction(0.5), gammaFunction(5)]).toEqual([
      expect.closeTo(1, 12),
      expect.closeTo(Math.sqrt(Math.PI) / 2, 12),
      expect.closeTo(Math.sqrt(Math.PI), 12),
      expect.closeTo(24, 10),
    ]);
  });

  it("give H-alpha, H-beta and H-gamma Barklem et al. 2000b's self-broadening", () => {
    const rows = BALMER_SELF_BROADENING.map((r) => [r.upperN, r.sigmaA02, r.alpha]);
    expect(rows).toEqual([
      [3, 1_180, 0.677],
      [4, 2_320, 0.455],
      [5, 4_208, 0.38],
    ]);
  });

  it("give the Balmer members above H-gamma H-gamma's self-broadening", () => {
    expect(selfBroadeningSource(9)).toEqual(selfBroadeningSource(5));
  });

  it("scale a width per H atom to another perturber by (α_p ÷ α_H)^(2/5) (μ_H ÷ μ_p)^n", () => {
    const toH2 = perturberScale("H2", 55.845, 0.3);
    expect([perturberScale("H", 55.845, 0.3), toH2]).toEqual([
      1,
      expect.closeTo((5.429 / 4.5) ** 0.4 * (withIron(1.008) / withIron(2.016)) ** 0.3, 12),
    ]);
  });

  it("take Na D's H₂ widths from Allard et al. 2019's half widths, as an angular full width", () => {
    // A half width of 21.982 cm⁻¹ at 10²¹ cm⁻³ and 1,000 K, as an angular full width per m⁻³.
    expect(d2At(1_000)).toBeCloseTo(((21.982 / 1e21) * 4 * Math.PI * 2.997_924_58e10) / 1e6, 20);
  });

  it("take Na D's H₂ widths beyond Allard et al.'s tables as a power law from their end interval", () => {
    const slope = Math.log(d2At(3_000) / d2At(2_500)) / Math.log(3_000 / 2_500);
    expect(Math.log(d2At(5_000) / d2At(3_000)) / Math.log(5_000 / 3_000)).toBeCloseTo(slope, 9);
  });

  it("refuse half widths that are not one a temperature of Allard et al.'s tables", () => {
    expect(() => allardH2WidthM3PerS(NA_D_H2_WIDTHS.d2HwhmPerCm.slice(1), 1_000)).toThrow(
      "one a temperature",
    );
  });

  it("state ratios to H₂ of 1 for H₂, and air's as its gases' by volume", () => {
    const ratios = atomicBroadeningRatios(toySet([lineAt(500, 0, 0)]), 2_000);
    const air = 0.780_84 * ratios.N2 + 0.209_476 * ratios.O2 + 0.009_34 * ratios.Ar;
    expect([ratios.H2, ratios.air]).toEqual([1, expect.closeTo(air / 0.999_656, 12)]);
  });

  it("scale N₂'s width from the line's H width by Unsöld's rule", () => {
    const ratios = atomicBroadeningRatios(toySet([lineAt(500, 0, 0)]), 2_000);
    expect(ratios.N2 / ratios.H).toBeCloseTo(perturberScale("N2", 55.845, 0.3), 12);
  });
});

describe("Na D's widths", () => {
  const d2 = { ...lineAt(589.158, 0.108, 0), upperJ: 1.5 };
  const d1 = { ...lineAt(589.756, -0.194, 0), upperJ: 0.5 };
  const base = toySet([d2, d1], "Na");
  const abo = { kind: "abo" as const, logGammaPerHydrogen: -7.526, exponent: 0.3635 };
  const set: AtomicLineSet = { ...base, vanDerWaals: base.vanDerWaals.map(() => abo) };
  const sodium: AtomicLineSet = {
    ...set,
    vanDerWaals: set.lines.map((line) => ({
      kind: "allardH2" as const,
      hwhmPerCm: line === d2 ? NA_D_H2_WIDTHS.d2HwhmPerCm : NA_D_H2_WIDTHS.d1HwhmPerCm,
      otherwise: abo,
    })),
  };
  const i = set.lines.indexOf(d2);

  it("keep their ABO width for H", () => {
    expect(vanDerWaalsWidthM3PerS(sodium, i, "H", 1_000)).toBe(
      vanDerWaalsWidthM3PerS(set, i, "H", 1_000),
    );
  });

  it("take He's from Allard et al.'s H₂ width by Unsöld's rule", () => {
    expect(
      vanDerWaalsWidthM3PerS(sodium, i, "He", 1_000) /
        vanDerWaalsWidthM3PerS(sodium, i, "H2", 1_000),
    ).toBeCloseTo(perturberScale("He", 55.845, 0.3635) / perturberScale("H2", 55.845, 0.3635), 12);
  });

  it("lie 1.25–1.37 times under the ABO width scaled to H₂ for D2", () => {
    const aboOverAllard =
      vanDerWaalsWidthM3PerS(set, i, "H2", 1_000) / vanDerWaalsWidthM3PerS(sodium, i, "H2", 1_000);
    expect([aboOverAllard > 1.25, aboOverAllard < 1.37]).toEqual([true, true]);
  });
});

import { describe, expect, it } from "vitest";

import bakeWavelengths from "../../../../packages/protocol/fixtures/bake_wavelengths_nm.json" with { type: "json" };
import {
  BAKE_BIN_EDGES_NM,
  type BarklemTable,
  BOLTZMANN_J_PER_K,
  cellOf,
  cellWidthPerCm,
  classicalDampingPerS,
  dopplerHalfWidthWithTurbulencePerCm,
  ELECTRON_RADIUS_CM,
  hydrogenEquilibriumPa,
  type KuruczLine,
  kuruczVacuumNm,
  linesInWindow,
  lineBinWidthsNm,
  lineCellMeansM2,
  lineStrengthCm,
  mortonAirIndex,
  parseBarklemTable,
  parseIonisationEnergies,
  parseKuruczLines,
  partitionFunctionAt,
  PERTURBER_POLARISABILITIES_A03,
  PER_CM_PER_EV,
  REFERENCE_GRAVITY_M_S2,
  REFERENCE_PRESSURE_PA,
  referenceColumnPerM2,
  roundUpToThreeFigures,
  rydbergWidthM3PerS,
  sahaBalance,
  SLANT_FACTOR,
  SOLAR_ABUNDANCES,
  stageBinShares,
  ATOMIC_MASS_KG,
  vacuumFromAirNm,
  vanDerWaalsWidthPerS,
} from "./atomicLines";
import { lineAt, toySahaData, toyTable } from "./atomicLinesToys";
import { voigtK } from "./molecularLines";

/** A field right-aligned in its columns. */
function pad(text: string, columns: number): string {
  return text.padStart(columns);
}

/**
 * A record in Kurucz's 160-column format (`0linelists.readme`), its values made up: a line listed
 * at `nm` with log gf, its levels' energies (the first may be the upper one) and a reference code.
 */
function record(nm: number, logGf: number, first: number, second: number, ref = "XX"): string {
  return (
    pad(nm.toFixed(4), 11) +
    pad(logGf.toFixed(3), 7) +
    pad("28.01", 6) +
    pad(first.toFixed(3), 12) +
    pad("2.5", 5) +
    " " +
    "lower lab ".slice(0, 10) +
    pad(second.toFixed(3), 12) +
    pad("3.5", 5) +
    " " +
    "upper lab ".slice(0, 10) +
    pad("8.19", 6) +
    pad("-5.38", 6) +
    pad("-7.59", 6) +
    ref.padEnd(4) +
    " 0 0  0 0.000  0 0.000    0    0           1200 1283"
  );
}

describe("Kurucz's records", () => {
  it("read the wavelength, log gf, both levels and the reference code", () => {
    const [line] = parseKuruczLines(record(406.8136, -1.29, 32_499.53, 57_080.38, "HEIS"));
    expect([
      line?.listedNm,
      line?.logGf,
      line?.lowerPerCm,
      line?.upperPerCm,
      line?.reference,
    ]).toEqual([406.8136, -1.29, 32_499.53, 57_080.38, "HEIS"]);
  });

  it("read Kurucz's three damping constants: log Γ_rad, log Γ_Stark ÷ n_e and log Γ_vdW ÷ n_H", () => {
    const [line] = parseKuruczLines(record(406.8136, -1.29, 32_499.53, 57_080.38));
    expect([
      line?.logGammaRadPerS,
      line?.logGammaStarkPerElectron,
      line?.logGammaVanDerWaalsPerHydrogen,
    ]).toEqual([8.19, -5.38, -7.59]);
  });

  it("read a blank damping constant as 0, Kurucz's none", () => {
    const base = record(406.8136, -1.29, 32_499.53, 57_080.38);
    const [line] = parseKuruczLines(`${base.slice(0, 80)}${" ".repeat(18)}${base.slice(98)}`);
    expect([
      line?.logGammaRadPerS,
      line?.logGammaStarkPerElectron,
      line?.logGammaVanDerWaalsPerHydrogen,
    ]).toEqual([0, 0, 0]);
  });

  it("take the lesser energy as the lower level, a negative (predicted) one by its magnitude", () => {
    const [line] = parseKuruczLines(record(500, 0, 40_000, -20_000));
    expect([line?.lowerPerCm, line?.upperPerCm, line?.lowerJ, line?.upperJ]).toEqual([
      20_000, 40_000, 3.5, 2.5,
    ]);
  });

  it("add the hyperfine and isotopic log fractions to log gf", () => {
    const base = record(500, -1, 0, 20_000);
    const withFractions = `${base.slice(0, 109)}-0.300  0-0.200${base.slice(124)}`;
    expect(parseKuruczLines(withFractions)[0]?.logGf).toBeCloseTo(-1.5, 12);
  });

  it("refuse a record under the format's 124 columns", () => {
    expect(() => parseKuruczLines(record(500, 0, 0, 1).slice(0, 100))).toThrow("124");
  });
});

describe("Morton's air-to-vacuum conversion", () => {
  it("gives standard air's index at 500 nm in vacuum: 1.000 278 973", () => {
    // 1 + 8.342 54e-5 + 2.406 147e-2 ÷ 126 + 1.5998e-4 ÷ 34.9, s = 2 µm⁻¹, summed by hand.
    expect(mortonAirIndex(500)).toBeCloseTo(1.000_278_973, 9);
  });

  it("inverts λ_air = λ_vac ÷ n(λ_vac) to 10⁻¹² nm", () => {
    for (const air of [200.5, 406.8136, 589.436, 777.0]) {
      const vacuum = vacuumFromAirNm(air);
      expect(Math.abs(vacuum / mortonAirIndex(vacuum) - air)).toBeLessThan(1e-12);
    }
  });

  it("takes Kurucz's wavelengths as vacuum below 200 nm and as air above", () => {
    expect([kuruczVacuumNm(199.9), kuruczVacuumNm(500) > 500.13]).toEqual([199.9, true]);
  });
});

describe("Barklem and Collet's tables", () => {
  it("read each row's values at the header's temperatures", () => {
    const table = parseBarklemTable(toyTable({ Ni_II: [1, 2, 3] }, [100, 1_000, 10_000]), "toy");
    expect([table.temperaturesK, table.rows.get("Ni_II")]).toEqual([
      [100, 1_000, 10_000],
      [1, 2, 3],
    ]);
  });

  it("refuse a row of another length than the header's", () => {
    expect(() =>
      parseBarklemTable(toyTable({ Ni_II: [1, 2] }, [100, 1_000, 10_000]), "toy"),
    ).toThrow("row Ni_II");
  });

  it("read table 4's ionisation energies by element", () => {
    const text =
      "# Atom.Num.  Elem.    IE1          IE2        IE3\n      28       Ni       7.6399      18.169      35.190\n";
    expect(parseIonisationEnergies(text).get("Ni")).toEqual([7.6399, 18.169, 35.19]);
  });

  it("interpolate Q by a cubic spline in log Q over log T, exact for a power law and at the nodes", () => {
    // The toy table holds six figures, as Barklem and Collet's do.
    const temperaturesK = [100, 300, 1_000, 3_000, 10_000];
    const values = temperaturesK.map((t) => Number((2 * t ** 0.3).toExponential(5)));
    const table = parseBarklemTable(toyTable({ X_I: values }, temperaturesK), "toy");
    expect(partitionFunctionAt(table, "X_I", 3_000)).toBeCloseTo(values[3] ?? Number.NaN, 9);
    expect(partitionFunctionAt(table, "X_I", 4_600) / (2 * 4_600 ** 0.3)).toBeCloseTo(1, 4);
  });

  it("interpolate H₂'s K_p in log K_p + 5,040 D₀ ÷ T, exact for its Boltzmann factor alone", () => {
    const temperaturesK = [100, 300, 1_000, 3_000, 10_000];
    const logK = temperaturesK.map((t) => 7 - (5_040 * 4.478_007) / t);
    const table: BarklemTable = parseBarklemTable(toyTable({ H2: logK }, temperaturesK), "toy");
    expect(Math.log10(hydrogenEquilibriumPa(table, 4_600))).toBeCloseTo(
      7 - (5_040 * 4.478_007) / 4_600,
      4,
    );
  });
});

describe("the Saha balance", () => {
  const data = toySahaData();
  const balance = sahaBalance(data, 4_600, REFERENCE_PRESSURE_PA);

  it("fills the pressure: P = n k T over every particle", () => {
    const metals = SOLAR_ABUNDANCES.filter((e) => e.symbol !== "H").reduce(
      (s, e) => s + 10 ** (e.logEpsilon - 12),
      0,
    );
    const particles =
      balance.atomicHydrogenPerM3 +
      balance.protonsPerM3 +
      balance.molecularHydrogenPerM3 +
      metals * balance.hydrogenNucleiPerM3 +
      balance.electronsPerM3;
    expect(particles * BOLTZMANN_J_PER_K * 4_600).toBeCloseTo(REFERENCE_PRESSURE_PA, 6);
  });

  it("holds the charge: the electrons are the ions' charges", () => {
    let charge = balance.protonsPerM3;
    for (const e of SOLAR_ABUNDANCES.filter((x) => x.symbol !== "H")) {
      const [, single = 0, double = 0] = balance.stageFractions.get(e.symbol) ?? [];
      charge += 10 ** (e.logEpsilon - 12) * balance.hydrogenNucleiPerM3 * (single + 2 * double);
    }
    expect(charge / balance.electronsPerM3).toBeCloseTo(1, 9);
  });

  it("keeps each element's stages summing to its atoms", () => {
    for (const [, fractions] of balance.stageFractions) {
      expect(fractions.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
    }
  });

  it("gives Na's stages Saha's ratio n⁺ n_e ÷ n⁰ = 2 (U⁺ ÷ U⁰)(2π m_e k T ÷ h²)^(3/2) e^(−χ ÷ kT)", () => {
    const [neutral = 0, single = 0] = balance.stageFractions.get("Na") ?? [];
    const electronTerm =
      ((2 * Math.PI * 9.109_383_701_5e-31 * BOLTZMANN_J_PER_K * 4_600) / 6.626_070_15e-34 ** 2) **
      1.5;
    const saha =
      2 *
      (1 / 2) *
      electronTerm *
      Math.exp((-5.14 * 1.602_176_634e-19) / (BOLTZMANN_J_PER_K * 4_600));
    expect(((single / neutral) * balance.electronsPerM3) / saha).toBeCloseTo(1, 9);
  });

  it("gives a stage the element's column above 0.1 bar at 10 m s⁻², times its fraction, doubled", () => {
    const perH =
      SOLAR_ABUNDANCES.reduce((s, e) => s + 10 ** (e.logEpsilon - 12) * e.atomicWeightDa, 0) *
      ATOMIC_MASS_KG;
    const [, single = 0] = balance.stageFractions.get("Na") ?? [];
    const expected =
      (SLANT_FACTOR * REFERENCE_PRESSURE_PA * 10 ** (6.24 - 12) * single) /
      (REFERENCE_GRAVITY_M_S2 * perH);
    expect(referenceColumnPerM2(balance, "Na", 1) / expected).toBeCloseTo(1, 12);
  });

  it("gives H I's stage its free atoms alone, the nuclei bound in H₂ left out (R08.T4.e)", () => {
    // H₂ that forms: K_p = 100 Pa at every node, so that at 0.1 bar most H is molecular.
    const toy = toySahaData();
    const temperaturesK = [100, 1_000, 3_000, 10_000];
    const molecular = sahaBalance(
      {
        ...toy,
        equilibriumConstants: parseBarklemTable(
          toyTable({ H2: temperaturesK.map(() => 2) }, temperaturesK),
          "toy 7",
        ),
      },
      2_000,
      REFERENCE_PRESSURE_PA,
    );
    const [neutral = 0] = molecular.stageFractions.get("H") ?? [];
    const free = molecular.atomicHydrogenPerM3 / molecular.hydrogenNucleiPerM3;
    const column =
      (SLANT_FACTOR * REFERENCE_PRESSURE_PA * free) /
      (REFERENCE_GRAVITY_M_S2 *
        SOLAR_ABUNDANCES.reduce((s, e) => s + 10 ** (e.logEpsilon - 12) * e.atomicWeightDa, 0) *
        ATOMIC_MASS_KG);
    expect([free < 0.5 * neutral, referenceColumnPerM2(molecular, "H", 0) / column]).toEqual([
      true,
      expect.closeTo(1, 12),
    ]);
  });
});

/** A line's equivalent width over [a, b] nm by a fine uniform trapezoid in wavelength, an f64 reference. */
function referenceWidthNm(
  wavenumber: number,
  column: number,
  strength: number,
  doppler: number,
  lorentz: number,
  [a, b]: readonly [number, number],
): number {
  const n = 400_000;
  const h = (b - a) / n;
  let sum = 0;
  for (let i = 0; i <= n; i += 1) {
    const nm = a + i * h;
    const nu = 1e7 / nm;
    const tau =
      ((column * strength * Math.sqrt(Math.LN2 / Math.PI)) / doppler) *
      voigtK(
        (Math.sqrt(Math.LN2) * (nu - wavenumber)) / doppler,
        (Math.sqrt(Math.LN2) * lorentz) / doppler,
      );
    sum += (i === 0 || i === n ? 0.5 : 1) * (1 - Math.exp(-tau));
  }
  return sum * h;
}

describe("a line's equivalent widths by bake bin", () => {
  it("cut the bins at the bake bins' edges, 25.33 nm either side of the protocol's centres", () => {
    const half = 380 / 15 / 2;
    expect(BAKE_BIN_EDGES_NM.map((e, k) => (k < 15 ? e + half : e))).toEqual([
      ...bakeWavelengths.wavelengths_nm.map((c) => expect.closeTo(c, 9)),
      760,
    ]);
  });

  it("are N S λ² ÷ 10⁷ nm in its own bin for a weak line", () => {
    const nu = 1e7 / 500;
    const widths = lineBinWidthsNm(nu, 1e10, 1e-15, 0.05, 0.005);
    const expected = (1e10 * 1e-15 * 500 ** 2) / 1e7;
    expect([widths[4] ?? 0, widths.reduce((a, b) => a + b, 0) - (widths[4] ?? 0)]).toEqual([
      expect.closeTo(expected, expected * 1e-3),
      expect.closeTo(0, expected * 1e-2),
    ]);
  });

  it("match an f64 quadrature of 1 − e^(−τ) for a saturated, damped line, within 10⁻³", () => {
    const nu = 1e7 / 406.82;
    const [column, strength, doppler, lorentz] = [1e16, 1e-14, 0.12, 0.03];
    const bin = [BAKE_BIN_EDGES_NM[1] ?? 0, BAKE_BIN_EDGES_NM[2] ?? 0] as const;
    const widths = lineBinWidthsNm(nu, column, strength, doppler, lorentz);
    const reference = referenceWidthNm(nu, column, strength, doppler, lorentz, bin);
    expect((widths[1] ?? 0) / reference).toBeCloseTo(1, 3);
  });

  it("split a line on a bin's edge between the two bins", () => {
    const edgeNm = BAKE_BIN_EDGES_NM[5] ?? 0;
    const widths = lineBinWidthsNm(1e7 / edgeNm, 1e14, 1e-14, 0.05, 0.005);
    expect((widths[4] ?? 0) / (widths[5] ?? 1)).toBeCloseTo(1, 2);
  });

  it("count only the part inside 380–760 nm of a line outside it", () => {
    const outside = lineBinWidthsNm(1e7 / 370, 1e18, 1e-14, 0.05, 0.05);
    const all = (1e18 * 1e-14 * 370 ** 2) / 1e7;
    expect([(outside[0] ?? 0) > 0, outside.reduce((a, b) => a + b, 0) < 1e-3 * all]).toEqual([
      true,
      true,
    ]);
  });
});

/**
 * A gas's static polarisability, a₀³, by Lorentz–Lorenz from its refractivity at a reference state
 * of 0 °C and 1 atm with its compressibility.
 */
function alphaA03(nMinusOne: number, compressibility: number): number {
  const n = 1 + nMinusOne;
  const density = 101_325 / (compressibility * BOLTZMANN_J_PER_K * 273.15);
  return ((3 / (4 * Math.PI * density)) * (n * n - 1)) / (n * n + 2) / 5.291_772_109_03e-11 ** 3;
}

describe("a line's strength and widths", () => {
  it("is π r_e gf e^(−c₂E ÷ T) ÷ Q", () => {
    const line = lineAt(500, -1, 10_000);
    expect(lineStrengthCm(line, 4_000, 30)).toBeCloseTo(
      (Math.PI * ELECTRON_RADIUS_CM * 0.1 * Math.exp((-1.438_776_877 * 10_000) / 4_000)) / 30,
      30,
    );
  });

  it("takes classical damping, 0.2223 ÷ λ² s⁻¹ with λ in cm", () => {
    expect(classicalDampingPerS(500) / (0.2223 / 500e-7 ** 2)).toBeCloseTo(1, 3);
  });

  it("takes Doppler broadening with 1 km s⁻¹ of microturbulence", () => {
    const nu = 20_000;
    const b = Math.sqrt((2 * BOLTZMANN_J_PER_K * 5_000) / (58.693 * ATOMIC_MASS_KG) + 1e6);
    expect(dopplerHalfWidthWithTurbulencePerCm(nu, 5_000, 58.693)).toBeCloseTo(
      (nu * Math.sqrt(Math.LN2) * b) / 299_792_458,
      12,
    );
  });

  it("takes H₂'s and He's polarisabilities from their dispersions at λ → ∞ by Lorentz–Lorenz", () => {
    // Peck and Huang 1977 and Mansfield and Peck 1969 at 0 °C and 1 atm, with their Z.
    expect([PERTURBER_POLARISABILITIES_A03.H2, PERTURBER_POLARISABILITIES_A03.He]).toEqual([
      expect.closeTo(alphaA03((14_895.6 / 180.7 + 4_903.7 / 92) * 1e-6, 1.000_624), 3),
      expect.closeTo(alphaA03((1_470.091 / 423.98) * 1e-5, 1.000_532), 3),
    ]);
  });

  it("takes Unsöld's van der Waals width in proportion to the perturbers, as T^0.3 and as ΔC₆^0.4", () => {
    const line = lineAt(500, 0, 2 * PER_CM_PER_EV, 4.48 * PER_CM_PER_EV);
    const at = (t: number, n: number): number =>
      vanDerWaalsWidthPerS(line, 7.6, 1, 58.693, t, { H: n, H2: 0, He: 0 });
    expect([at(4_000, 2e23) / at(4_000, 1e23), at(8_000, 1e23) / at(4_000, 1e23)]).toEqual([
      expect.closeTo(2, 12),
      expect.closeTo(2 ** 0.3, 12),
    ]);
  });
});

/** K I's limit, eV, as decision-r08-t4e-unsold §6's test values take it, and K's mass, Da. */
const POTASSIUM_LIMIT_EV = 4.340_664;
const POTASSIUM_DA = 39.0983;

/** A line between two levels, cm⁻¹, its vacuum wavelength theirs. */
function between(lowerPerCm: number, upperPerCm: number): KuruczLine {
  return lineAt(1e7 / (upperPerCm - lowerPerCm), 0, lowerPerCm, upperPerCm);
}

/** The same line with a Kurucz damping field, which keeps it on Unsöld's width alone. */
function onUnsold(line: KuruczLine): KuruczLine {
  return { ...line, logGammaVanDerWaalsPerHydrogen: -7.5 };
}

/** A line's full van der Waals width per perturber at factor 1, cm³ s⁻¹. */
function widthPer(
  line: KuruczLine,
  perturber: "H" | "H2" | "He",
  temperatureK: number,
  { limitEv = POTASSIUM_LIMIT_EV, massDa = POTASSIUM_DA, coreCharge = 1 } = {},
): number {
  const perturbers = { H: 0, H2: 0, He: 0, [perturber]: 1 };
  return vanDerWaalsWidthPerS(line, limitEv, coreCharge, massDa, temperatureK, perturbers) * 1e6;
}

/** A level of n* below a limit, cm⁻¹. */
function levelAt(limitEv: number, nStar: number): number {
  return (limitEv - 13.605_693_122_994 / (nStar * nStar)) * PER_CM_PER_EV;
}

/** The mean relative speed of K and a perturber at a temperature, m s⁻¹. */
function potassiumSpeed(perturberDa: number, temperatureK: number): number {
  const reducedKg = ((POTASSIUM_DA * perturberDa) / (POTASSIUM_DA + perturberDa)) * ATOMIC_MASS_KG;
  return Math.sqrt((8 * BOLTZMANN_J_PER_K * temperatureK) / (Math.PI * reducedKg));
}

/** An H I line's width per H₂ at 1,000 K, m³ s⁻¹, its levels taken as hydrogenic or not. */
function hydrogen(line: KuruczLine, hydrogenic: boolean): number {
  return vanDerWaalsWidthPerS(line, 13.5984, 1, 1.008, 1_000, { H: 0, H2: 1, He: 0 }, hydrogenic);
}

describe("the Rydberg bound on Unsöld's widths (decision-r08-t4e-unsold)", () => {
  // K's 4p₁/₂ level, cm⁻¹ (the ruling's figure).
  const p12 = 12_985.185;

  it("keeps Unsöld's width below n* 6, by a near-hydrogenic level, and for an ion", () => {
    // K 4s–5p 404.41 nm and 4p₁/₂–8s 532.33 nm (n* 5.816); Na 3p₁/₂–11d 423.90 nm (n* 10.985).
    expect([
      widthPer(between(0, 24_720.139), "H2", 1_000) / 1.8335e-8,
      widthPer(between(p12, 31_765.37), "H2", 5_000) / 7.689e-8,
      widthPer(between(16_956.172, 40_540.07), "H2", 1_000, {
        limitEv: 5.139_076,
        massDa: 22.989_77,
      }) / 1.3368e-7,
    ]).toEqual([expect.closeTo(1, 3), expect.closeTo(1, 3), expect.closeTo(1, 3)]);
    // An ion's line from n* 10.8 keeps Unsöld's T^0.3 law, where the bound's is about T^(1/6).
    const ion = between(p12, 34_072.2);
    const ionAt = (t: number): number => widthPer(ion, "H2", t, { coreCharge: 2 });
    expect(ionAt(2_000) / ionAt(1_000)).toBeCloseTo(2 ** 0.3, 12);
  });

  it("bounds K 4p₁/₂–13s 474.09 nm and 4p₁/₂–9s 508.42 nm by the ruling's figures, within 1%", () => {
    const thirteen = between(p12, 34_072.2);
    const nine = between(p12, 32_648.35);
    expect([
      widthPer(thirteen, "H2", 1_000) / 1.8137e-8,
      widthPer(thirteen, "He", 1_000) / 7.5856e-9,
      widthPer(thirteen, "H", 1_000) / 2.3311e-8,
      widthPer(nine, "H2", 1_000) / 2.9896e-8,
      widthPer(onUnsold(thirteen), "H2", 1_000) / 1.2911e-7,
      widthPer(onUnsold(thirteen), "He", 1_000) / 6.1758e-8,
      widthPer(onUnsold(thirteen), "H", 1_000) / 1.4636e-7,
      widthPer(onUnsold(nine), "H2", 1_000) / 6.1423e-8,
    ]).toEqual(Array.from({ length: 8 }, () => expect.closeTo(1, 2)));
  });

  it("reaches the polarisation width's asymptote at high n* and above the limit", () => {
    // K 5s–20p 732.11 nm (n* 18.29), within 2.1% of γ_p alone, 1.5619 × 10⁻⁸ cm³ s⁻¹.
    const asymptote = rydbergWidthM3PerS(Infinity, "H2", potassiumSpeed(2.016, 1_000)) * 1e6;
    const twenty = widthPer(between(21_026.551, 34_681.84), "H2", 1_000);
    expect([twenty / 1.5941e-8, asymptote / 1.5619e-8, twenty / asymptote < 1.021]).toEqual([
      expect.closeTo(1, 2),
      expect.closeTo(1, 2),
      true,
    ]);
    expect(widthPer(between(21_026.551, 35_100), "H2", 1_000)).toBeCloseTo(asymptote, 20);
  });

  it("keeps H I's levels on Unsöld's width, all hydrogenic, but an entry at or above its limit", () => {
    // Balmer n = 42 from Kurucz's levels, whose n* from Barklem and Collet's 13.5984 eV is 42.10,
    // and the series-limit entry at 109,678.76 cm⁻¹, above it.
    const member = between(82_259.1, 109_616.59);
    const limit = between(82_259.1, 109_678.76);
    expect([
      hydrogen(member, true) / hydrogen(onUnsold(member), true),
      hydrogen(member, false) < hydrogen(onUnsold(member), false),
      hydrogen(limit, true) < hydrogen(onUnsold(limit), true),
    ]).toEqual([1, true, true]);
  });

  /** K I's 4p₁/₂ to levels from n* 6.5 to 30.5 at a fixed temperature. */
  const rydbergSeries = Array.from({ length: 25 }, (_, k) =>
    between(p12, levelAt(POTASSIUM_LIMIT_EV, 6.5 + k)),
  );

  it.each(["H", "H2", "He"] as const)("is non-increasing in n* from 6 by %s", (perturber) => {
    const widths = rydbergSeries.map((l) => widthPer(l, perturber, 1_000));
    expect(widths.every((w, k) => k === 0 || w <= (widths[k - 1] ?? 0))).toBe(true);
  });

  it.each(["H", "H2", "He"] as const)("is never above Unsöld's width by %s", (perturber) => {
    const widths = rydbergSeries.map((l) => widthPer(l, perturber, 1_000));
    const unsold = rydbergSeries.map((l) => widthPer(onUnsold(l), perturber, 1_000));
    expect(widths.every((w, k) => w <= (unsold[k] ?? 0))).toBe(true);
  });

  it("is continuous in T", () => {
    // Steps of 0.1% in T from 300 K to 10,000 K move the width by under 0.1% at every n*.
    for (const n of [6.3, 7.5, 10.8, 18.3]) {
      const line = between(p12, levelAt(POTASSIUM_LIMIT_EV, n));
      let worst = 0;
      for (let t = 300; t < 10_000; t *= 1.001) {
        worst = Math.max(
          worst,
          Math.abs(widthPer(line, "H2", t * 1.001) / widthPer(line, "H2", t) - 1),
        );
      }
      expect(worst).toBeLessThan(1e-3);
    }
  });

  /** Rb nS + He at 304 K, n* 29.87 (33s, nS's quantum defect 3.13): a toy line at Rb's mass and limit. */
  const rubidiumWidth = (): number =>
    widthPer(between(0, levelAt(4.177_128, 29.87)), "He", 304, {
      limitEv: 4.177_128,
      massDa: 85.4678,
    });

  /** K 4s–np + He at 700 K, n* 12.3. */
  const potassiumWidth = (): number =>
    widthPer(between(0, levelAt(POTASSIUM_LIMIT_EV, 12.3)), "He", 700);

  it("lies within 1.5 times Lei et al.'s Rydberg width of Rb by He", () => {
    // Lei et al. 2025 (Phys. Rev. Applied 23, 034028, Table III) observe γ/n 5.8(6) × 10⁻¹⁰ Hz cm³,
    // 3.64 × 10⁻⁹ cm³ s⁻¹ as an angular width.
    const rb = rubidiumWidth() / 3.64e-9;
    expect([rb > 1 / 1.5, rb < 1.5]).toEqual([true, true]);
  });

  it("lies within twice Füchtbauer and von Heesen's Rydberg width of K by He", () => {
    // Füchtbauer and von Heesen 1939 (Z. Phys. 113, 323, Fig. 3, self-broadening removed; 1 cm⁻¹
    // an amagat is 7.011 × 10⁻⁹ cm³ s⁻¹) measure 0.88–1.06 × 10⁻⁸ at 5–22 amagat, as main read it.
    const k = potassiumWidth();
    expect([k > 1.06e-8 / 2, k < 0.88e-8 * 2]).toEqual([true, true]);
  });

  it("gives the ruling's bound for those two, 4.66 and 6.72 × 10⁻⁹ cm³ s⁻¹", () => {
    expect([rubidiumWidth(), potassiumWidth()]).toEqual([
      expect.closeTo(4.66e-9, 11),
      expect.closeTo(6.72e-9, 11),
    ]);
  });
});

describe("a stage's cell means", () => {
  it("put each line's π r_e gf e^(−c₂E ÷ T) ÷ Q over its cell's width, m²", () => {
    const line = lineAt(600.2, -1, 5_000);
    const { cellMeansM2, linesInRange } = lineCellMeansM2([line], 3_000, 20, 10);
    const expected = (lineStrengthCm(line, 3_000, 20) / cellWidthPerCm(220)) * 1e-4;
    expect([cellOf(600.2), cellMeansM2[220], linesInRange]).toEqual([
      220,
      expect.closeTo(expected, 40),
      1,
    ]);
  });

  it("leave out a line from a level above the ionisation limit, and lines outside the cells", () => {
    const lines = [lineAt(600, -1, 11 * PER_CM_PER_EV), lineAt(379.4, -1, 0), lineAt(760.6, -1, 0)];
    expect(lineCellMeansM2(lines, 3_000, 20, 10).linesInRange).toBe(0);
  });
});

describe("rounding up to three figures", () => {
  it.each([
    [1.2345e-30, 1.24e-30],
    [1.23e-30, 1.23e-30],
    [9.991e-12, 1e-11],
    [0, 0],
  ])("takes %s to %s", (value, expected) => {
    expect(roundUpToThreeFigures(value)).toBe(expected);
  });

  it("never rounds a value down", () => {
    for (let i = 0; i < 2_000; i += 1) {
      const value = 10 ** (-40 + (i * 37) / 2_000) * (1 + i / 7_919);
      const rounded = roundUpToThreeFigures(value);
      expect([rounded >= value, Number(rounded.toPrecision(3)) === rounded]).toEqual([true, true]);
    }
  });
});

describe("a stage's lines' shares at its reference column", () => {
  const data = toySahaData();
  /** Na⁺, whose toy limit is table 4's toy 20 eV. */
  const sodiumIon = { key: "Na+", symbol: "Na", stage: 1 } as const;

  it("sum the lines in 360–780 nm (vacuum) from a level below the limit, and no others", () => {
    const lines = [
      lineAt(359, -1, 1_000),
      lineAt(361, -1, 1_000),
      lineAt(779, -1, 1_000),
      lineAt(781, -1, 1_000),
      lineAt(500, -1, 21 * PER_CM_PER_EV),
    ];
    expect(linesInWindow(lines, 20).map((l) => l.vacuumNm)).toEqual([361, 779]);
  });

  it("give a weak line's share as N S λ² ÷ 10⁷ over the bin's 25.33 nm, at the stage's own column", () => {
    // At log gf −12 the line's centre depth at Na⁺'s column is about 2 × 10⁻³: the linear limit.
    const line = lineAt(500, -12, 2_000);
    const shares = stageBinShares(data, sodiumIon, [line], 4_600);
    const balance = sahaBalance(data, 4_600, REFERENCE_PRESSURE_PA);
    const columnCm2 = referenceColumnPerM2(balance, "Na", 1) * 1e-4;
    const q = partitionFunctionAt(data.partitionFunctions, "Na_II", 4_600);
    const expected = (columnCm2 * lineStrengthCm(line, 4_600, q) * 500 ** 2) / 1e7 / (380 / 15);
    expect([shares.columnPerM2, (shares.binShares[4] ?? 0) / expected, shares.linesSummed]).toEqual(
      [referenceColumnPerM2(balance, "Na", 1), expect.closeTo(1, 2), 1],
    );
  });

  it("name each bin's largest lines, the largest first, at most five", () => {
    const lines = Array.from({ length: 7 }, (_, i) => lineAt(500 + i * 0.5, -6 + i * 0.1, 2_000));
    const leading = stageBinShares(data, sodiumIon, lines, 4_600).leading[4] ?? [];
    expect([leading.length, leading[0]?.line.vacuumNm, leading.at(-1)?.line.vacuumNm]).toEqual([
      5, 503, 501,
    ]);
  });
});

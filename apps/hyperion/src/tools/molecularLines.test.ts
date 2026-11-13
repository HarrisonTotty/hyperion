import { describe, expect, it } from "vitest";

import {
  cellPointCount,
  cellSpectrum,
  cellWavenumbersPerCm,
  dopplerHalfWidthPerCm,
  HITRAN_REFERENCE_K,
  intensityAt,
  LINE_CUTOFF_PER_CM,
  type LineList,
  type LineSpecies,
  lineStates,
  linesNear,
  NEAR_ZONE_HALF_WIDTHS,
  parseHitranLines,
  parsePartitionSums,
  partitionSumAt,
  SECOND_RADIATION_CONSTANT_CM_K,
  voigtHalfWidthPerCm,
  voigtK,
  voigtProfile,
} from "./molecularLines";

/**
 * K(x, y) = (y ÷ π) ∫ e^(−t²) ÷ ((x − t)² + y²) dt by the trapezoid rule over |t| ≤ 9, at a step of
 * y ÷ 20 or finer. For an integrand analytic in a strip of half-width y about the real axis the
 * rule's error falls as e^(−2πy ÷ h) (L. N. Trefethen and J. A. C. Weideman, SIAM Rev. 56 (2014)
 * 385, Theorem 5.1), here e^(−40π), and e^(−81) is the tail beyond |t| = 9: an f64 reference far
 * below Humlíček's 10⁻⁴.
 */
function referenceK(x: number, y: number): number {
  const h = Math.min(y / 20, 0.01);
  const n = Math.ceil(18 / h);
  let sum = 0;
  for (let i = 0; i <= n; i += 1) {
    const t = -9 + i * h;
    const w = i === 0 || i === n ? 0.5 : 1;
    sum += (w * Math.exp(-t * t)) / ((x - t) ** 2 + y * y);
  }
  return ((y / Math.PI) * sum * 18) / n;
}

describe("Humlíček's Voigt function", () => {
  // Humlíček 1982 (J. Quant. Spectrosc. Radiat. Transfer 27, 437) states a relative error under
  // 10⁻⁴ for his W4; the points span its four regions.
  it.each(
    [0.02, 0.1, 0.3, 1, 3, 10].flatMap((y) =>
      [0, 0.5, 1, 2, 3, 5, 8, 12, 20, 40].map((x) => [x, y] as const),
    ),
  )("gives K(%s, %s) within 10⁻⁴ of an f64 quadrature of the convolution", (x, y) => {
    const reference = referenceK(x, y);
    expect(Math.abs(voigtK(x, y) - reference) / reference).toBeLessThan(1e-4);
  });

  it.each([0, 0.5, 1, 2, 2.5])("tends to the Gaussian e^(−x²) as y → 0, at x = %s", (x) => {
    expect(Math.abs(voigtK(x, 1e-9) - Math.exp(-x * x)) / Math.exp(-x * x)).toBeLessThan(1e-4);
  });

  it("gives a profile whose integral over wavenumber is 1", () => {
    const doppler = 0.015;
    const lorentz = 0.07;
    // Trapezoid over ±2,000 cm⁻¹, with the Lorentzian tails beyond, 2γ ÷ (π × 2,000), added.
    const h = 0.0005;
    let sum = 0;
    for (let nu = -2_000; nu <= 2_000; nu += h) {
      sum += voigtProfile(nu, doppler, lorentz) * h;
    }
    const tails = (2 * lorentz) / (Math.PI * 2_000);
    expect(Math.abs(sum + tails - 1)).toBeLessThan(1e-4);
  });
});

describe("a line's widths and strength", () => {
  it("gives O₂'s Doppler half-width at 296 K and 14,500 cm⁻¹: 0.015 795 450 cm⁻¹", () => {
    // (ν ÷ c) √(2 ln 2 R T ÷ M) with CODATA 2018's R and HITRAN's 31.989 83 g mol⁻¹, computed apart.
    expect(dopplerHalfWidthPerCm(14_500, 296, 31.989_83)).toBeCloseTo(0.015_795_450_2, 10);
  });

  it("gives the Voigt half-width as the Gaussian's alone and the Lorentzian's alone at the limits", () => {
    expect(voigtHalfWidthPerCm(0, 0.02)).toBeCloseTo(0.02, 12);
    // 0.5346 + √0.2166 = 1.000 01 (Olivero and Longbothum's Eq. 5 at f_G = 0).
    expect(voigtHalfWidthPerCm(0.1, 0)).toBeCloseTo(0.1 * (0.5346 + Math.sqrt(0.2166)), 12);
  });

  it("keeps the intensity at 296 K, and S(T) Q(T) ÷ [e^(−c₂E″÷T)(1 − e^(−c₂ν÷T))] at every T", () => {
    const [s0, e, nu, q296] = [1e-24, 800, 14_000, 215.7];
    const qAt = (t: number): number => q296 * (t / HITRAN_REFERENCE_K) ** 1.5;
    const invariant = (t: number): number => {
      const c2 = SECOND_RADIATION_CONSTANT_CM_K;
      return (
        (intensityAt(s0, e, nu, q296, qAt(t), t) * qAt(t)) /
        (Math.exp((-c2 * e) / t) * -Math.expm1((-c2 * nu) / t))
      );
    };
    expect(intensityAt(s0, e, nu, q296, q296, HITRAN_REFERENCE_K)).toBe(s0);
    for (const t of [75, 150, 400, 500]) {
      expect(invariant(t) / invariant(HITRAN_REFERENCE_K)).toBeCloseTo(1, 12);
    }
  });
});

/** A HITRAN CSV record of CO₂ at a wavenumber. */
function record(nu: number, iso = 1, extra = "0.0616,0.72,0.0558,0.81"): string {
  return `7,${iso},${nu},3.284e-27,79.9201,0.05221,0.7631,-0.006269,0.054,0.6773,#,#,#,#,${extra}`;
}

describe("HITRAN's files", () => {
  it("reads a request's records, sorted by wavenumber, with a broadener HITRAN does not give as NaN", () => {
    const lines = parseHitranLines(`${record(13_200.5, 2)}\n${record(13_100.25)}\n`);
    expect([
      Array.from(lines.wavenumbersPerCm),
      Array.from(lines.isotopologues),
      Number.isNaN(lines.foreignHalfWidthsPerCmAtm.H2[0] ?? 0),
      lines.foreignHalfWidthsPerCmAtm.CO2[0],
    ]).toEqual([[13_100.25, 13_200.5], [36, 37], true, 0.0616]);
  });

  it("refuses a record of another field count or an isotopologue it does not reduce", () => {
    expect(() => parseHitranLines("7,1,13100.5\n")).toThrow("has 3 fields");
    expect(() => parseHitranLines(`${record(13_100, 9)}\n`)).toThrow("7/9");
  });

  it("reads TIPS partition sums and takes Q linearly between whole kelvins", () => {
    const sums = parsePartitionSums("   1   1.0\n   2   3.0\n   3   7.0\n");
    expect([partitionSumAt(sums, 2), partitionSumAt(sums, 2.25)]).toEqual([3, 4]);
    expect(() => parsePartitionSums("1 1\n3 2\n")).toThrow("steps of 1 K");
  });
});

/**
 * A line list of a few toy lines, sorted by wavenumber as a parsed list is, with air broadening
 * only, every one of isotopologue 36 (¹⁶O₂).
 */
function toyLines(centres: ReadonlyArray<number>, intensity = 1e-24): LineList {
  const n = centres.length;
  const fill = (v: number): Float64Array => new Float64Array(n).fill(v);
  return {
    isotopologues: new Uint16Array(n).fill(36),
    wavenumbersPerCm: Float64Array.from(centres).toSorted(),
    intensitiesCm: fill(intensity),
    lowerEnergiesPerCm: fill(100),
    airHalfWidthsPerCmAtm: fill(0.05),
    airExponents: fill(0.7),
    selfHalfWidthsPerCmAtm: fill(0.05),
    foreignHalfWidthsPerCmAtm: {
      H2: fill(Number.NaN),
      He: fill(Number.NaN),
      CO2: fill(Number.NaN),
      H2O: fill(Number.NaN),
    },
    selfExponents: fill(Number.NaN),
    foreignExponents: {
      H2: fill(Number.NaN),
      He: fill(Number.NaN),
      CO2: fill(Number.NaN),
      H2O: fill(Number.NaN),
    },
  };
}

/** A far-wing factor of CO₂–N₂'s form at 296 K (σ 3, 10 and 70 cm⁻¹). */
function toyChi(a: number): number {
  if (a <= 3) {
    return 1;
  }
  if (a <= 10) {
    return Math.exp(-0.303 * (a - 3));
  }
  return a <= 70 ? Math.exp(-2.121 - 0.0219 * (a - 10)) : Math.exp(-3.435 - 0.02 * (a - 70));
}

/** A toy species whose partition sum is 1 at every kelvin, so that S(T) follows E″ alone. */
function toySpecies(lines: LineList): LineSpecies {
  return {
    lines,
    partitionSums: new Map([[36, { firstK: 1, values: new Float64Array(1_000).fill(1) }]]),
  };
}

describe("a cell's line-by-line spectrum", () => {
  const cellNm = 690;
  const [low, high] = cellWavenumbersPerCm(cellNm);
  // Lines inside the cell, at its edges, and outside it within and beyond the cut-off.
  const centres = [low + 3.3, low + 10.01, high - 0.02, low - 7, high + 24, low - 26];
  const species = toySpecies(toyLines(centres));

  it("takes every line within the cut-off of the cell, and none beyond", () => {
    expect(linesNear(species.lines, low, high)).toEqual([1, 2, 3, 4, 5]);
  });

  // The coarse grid's wing errs by at most ¾ (step ÷ Δν)², 2.3 × 10⁻³ where it is first used
  // (molecularLines.ts's cellSpectrum); within each line's near zone the sum is exact.
  it.each([
    [100, 10],
    [296, 1e3],
    [250, 3e4],
    [296, 1e5],
    [500, 1e7],
  ])("matches a brute-force sum of the cut-off profiles at %s K and %s Pa", (t, p) => {
    const indices = linesNear(species.lines, low, high);
    const states = lineStates(species, indices, t, p);
    const points = cellPointCount(cellNm, states);
    const sigma = cellSpectrum(cellNm, species, indices, states, points).crossSectionsCm2;
    let worst = 0;
    for (let j = 0; j < points; j += 1) {
      const nu = 1e7 / (cellNm - 0.5 + (j + 0.5) / points);
      let brute = 0;
      for (const [k, i] of indices.entries()) {
        const s = states[k];
        const offset = nu - (species.lines.wavenumbersPerCm[i] ?? Number.NaN);
        if (s !== undefined && Math.abs(offset) <= LINE_CUTOFF_PER_CM) {
          brute += s.intensityCm * voigtProfile(offset, s.dopplerPerCm, s.lorentzPerCm);
        }
      }
      worst = Math.max(worst, brute > 0 ? Math.abs((sigma[j] ?? 0) - brute) / brute : 0);
    }
    expect(worst).toBeLessThan(2.3e-3);
  });

  // A far-wing factor of CO₂–N₂'s form at 296 K, out to 250 cm⁻¹ ({@link toyChi}), and the
  // pedestal's subtraction: the same bound, χ's breaks being summed exactly.
  const wide = toySpecies(toyLines([low + 3.3, high - 0.02, low - 60, high + 140, low - 249]));
  it.each([
    [296, 1e4, true],
    [296, 3e6, true],
    [200, 1e5, false],
    [400, 9e6, false],
  ])(
    "matches a brute-force sum of χ-factor or pedestal profiles at %s K and %s Pa (χ: %s)",
    (t, p, withChi) => {
      const cutoff = withChi ? 250 : LINE_CUTOFF_PER_CM;
      const shape = withChi
        ? {
            cutoffPerCm: cutoff,
            pedestal: false,
            farWing: {
              factor: (offset: number) => toyChi(Math.abs(offset)),
              breaksPerCm: [3, 10, 70],
            },
          }
        : { cutoffPerCm: cutoff, pedestal: true };
      const indices = linesNear(wide.lines, low, high, cutoff);
      const states = lineStates(wide, indices, t, p);
      const points = cellPointCount(cellNm, states);
      const sigma = cellSpectrum(cellNm, wide, indices, states, points, shape).crossSectionsCm2;
      let worst = 0;
      for (let j = 0; j < points; j += 1) {
        const nu = 1e7 / (cellNm - 0.5 + (j + 0.5) / points);
        let brute = 0;
        for (const [k, i] of indices.entries()) {
          const s = states[k];
          const a = Math.abs(nu - (wide.lines.wavenumbersPerCm[i] ?? Number.NaN));
          if (s !== undefined && a <= cutoff) {
            const pedestal = withChi ? 0 : voigtProfile(cutoff, s.dopplerPerCm, s.lorentzPerCm);
            brute +=
              s.intensityCm *
              (voigtProfile(a, s.dopplerPerCm, s.lorentzPerCm) - pedestal) *
              (withChi ? toyChi(a) : 1);
          }
        }
        worst = Math.max(worst, brute > 0 ? Math.abs((sigma[j] ?? 0) - brute) / brute : 0);
      }
      expect(worst).toBeLessThan(2.3e-3);
    },
  );

  it("matches a brute-force sum under an asymmetric far-wing factor, its near zones meeting it", () => {
    // SpeCT's quantum asymmetry at 296 K beyond 3 cm⁻¹, times the toy χ: the blue wing raised and
    // the red lowered, so that the far part's two edges differ.
    const asymmetric = (offset: number): number => {
      const x = (1.438_777 * offset) / 296;
      return Math.abs(offset) <= 3 ? 1 : (toyChi(Math.abs(offset)) * x) / -Math.expm1(-x);
    };
    const shape = {
      cutoffPerCm: 250,
      pedestal: false,
      farWing: { factor: asymmetric, breaksPerCm: [3, 10, 70] },
    };
    const indices = linesNear(wide.lines, low, high, 250);
    const states = lineStates(wide, indices, 296, 3e6);
    const points = cellPointCount(cellNm, states);
    const sigma = cellSpectrum(cellNm, wide, indices, states, points, shape).crossSectionsCm2;
    let worst = 0;
    for (let j = 0; j < points; j += 1) {
      const nu = 1e7 / (cellNm - 0.5 + (j + 0.5) / points);
      let brute = 0;
      for (const [k, i] of indices.entries()) {
        const s = states[k];
        const offset = nu - (wide.lines.wavenumbersPerCm[i] ?? Number.NaN);
        if (s !== undefined && Math.abs(offset) <= 250) {
          brute +=
            s.intensityCm *
            voigtProfile(offset, s.dopplerPerCm, s.lorentzPerCm) *
            asymmetric(offset);
        }
      }
      worst = Math.max(worst, brute > 0 ? Math.abs((sigma[j] ?? 0) - brute) / brute : 0);
    }
    expect(worst).toBeLessThan(2.3e-3);
  });

  it("is exact within a lone line's near zone", () => {
    const lone = toySpecies(toyLines([low + 10]));
    const indices = linesNear(lone.lines, low, high);
    const states = lineStates(lone, indices, 296, 1e3);
    const points = cellPointCount(cellNm, states);
    const sigma = cellSpectrum(cellNm, lone, indices, states, points).crossSectionsCm2;
    const s = states[0];
    if (s === undefined) {
      throw new Error("the lone line has no state");
    }
    const zone = NEAR_ZONE_HALF_WIDTHS * voigtHalfWidthPerCm(s.lorentzPerCm, s.dopplerPerCm);
    let worst = 0;
    for (let j = 0; j < points; j += 1) {
      const offset = 1e7 / (cellNm - 0.5 + (j + 0.5) / points) - (low + 10);
      if (Math.abs(offset) < zone) {
        const exact = s.intensityCm * voigtProfile(offset, s.dopplerPerCm, s.lorentzPerCm);
        worst = Math.max(worst, Math.abs((sigma[j] ?? 0) - exact) / exact);
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });
});

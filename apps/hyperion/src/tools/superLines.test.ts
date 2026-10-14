import { describe, expect, it } from "vitest";

import { intervalTransmittance, lineByLine, type ShapePolicy } from "./kDistributions";
import {
  cellWavenumbersPerCm,
  intensityAt,
  type LineList,
  type LineSpecies,
  voigtProfile,
} from "./molecularLines";
import {
  binStepPerCm,
  circularKernel,
  coarsenBins,
  depositLine,
  emptyBins,
  fft,
  mergedCell,
  mergedSpectra,
  powerOfTwoAtLeast,
  SUPER_LINE_STEPS_PER_CUTOFF,
  superLineCell,
  superLinePointCount,
  superLineSpectra,
  type SuperLineBins,
  type SuperLineSpecies,
  type TransformedWindow,
} from "./superLines";

/** The plain line shape: Voigt within 25 cm⁻¹, no pedestal. */
const PLAIN_SHAPE = { cutoffPerCm: 25, pedestal: false } as const;

/** The plain shape: Voigt within 25 cm⁻¹, estimated past γ_L = 0.25 cm⁻¹. */
const PLAIN: ShapePolicy = {
  variants: [{ broadener: "air" }],
  cutoffPerCm: 25,
  pedestal: false,
  estimatedAboveHalfWidthPerCm: 0.25,
};

/** A deterministic sequence in [0, 1): a linear congruential generator, for the toy lines. */
function sequence(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    return state / 2 ** 32;
  };
}

/** A run of bins' total and first moment, its node i at (from + i) × step. */
function moment(values: ArrayLike<number>, step: number, from: number): number[] {
  let sum = 0;
  let first = 0;
  for (let i = 0; i < values.length; i += 1) {
    sum += values[i] ?? 0;
    first += (values[i] ?? 0) * (from + i) * step;
  }
  return [sum, first];
}

/** A sorted node's mean σ. */
function nodeMean(node: { readonly prefix: Float64Array; readonly sorted: Float64Array }): number {
  return (node.prefix.at(-1) ?? 0) / node.sorted.length;
}

/** A random sequence of 64 complex points and its transform by the direct sum. */
function directTransform(): {
  readonly re: Float64Array;
  readonly im: Float64Array;
  readonly sumRe: Float64Array;
  readonly sumIm: Float64Array;
} {
  const n = 64;
  const next = sequence(7);
  const re = Float64Array.from({ length: n }, () => next() - 0.5);
  const im = Float64Array.from({ length: n }, () => next() - 0.5);
  const sumRe = new Float64Array(n);
  const sumIm = new Float64Array(n);
  for (let k = 0; k < n; k += 1) {
    for (let j = 0; j < n; j += 1) {
      const angle = (-2 * Math.PI * j * k) / n;
      sumRe[k] = (sumRe[k] ?? 0) + (re[j] ?? 0) * Math.cos(angle) - (im[j] ?? 0) * Math.sin(angle);
      sumIm[k] = (sumIm[k] ?? 0) + (re[j] ?? 0) * Math.sin(angle) + (im[j] ?? 0) * Math.cos(angle);
    }
  }
  return { re, im, sumRe, sumIm };
}

/** The largest difference between two pairs of arrays. */
function largestDifference(
  a: readonly [Float64Array, Float64Array],
  b: readonly [Float64Array, Float64Array],
): number {
  let worst = 0;
  for (let k = 0; k < a[0].length; k += 1) {
    worst = Math.max(
      worst,
      Math.abs((a[0][k] ?? 0) - (b[0][k] ?? 0)),
      Math.abs((a[1][k] ?? 0) - (b[1][k] ?? 0)),
    );
  }
  return worst;
}

describe("the FFT", () => {
  it("equals the direct transform of a random sequence", () => {
    const { re, im, sumRe, sumIm } = directTransform();
    fft(re, im);
    expect(largestDifference([re, im], [sumRe, sumIm])).toBeLessThan(1e-12);
  });

  it("inverts its own transform", () => {
    const { re, im } = directTransform();
    const original = [Float64Array.from(re), Float64Array.from(im)] as const;
    fft(re, im);
    fft(re, im, true);
    expect(largestDifference([re, im], original)).toBeLessThan(1e-15);
  });

  it("refuses a length that is not a power of two", () => {
    expect(() => {
      fft(new Float64Array(6), new Float64Array(6));
    }).toThrow("power-of-two");
  });

  it("finds the least power of two at or above a length", () => {
    expect(powerOfTwoAtLeast(1_025)).toBe(2_048);
  });
});

describe("the bins", () => {
  it("share a line between the two nodes about it, keeping its total and centre", () => {
    const bins = emptyBins(200, 13_000, 13_001, 0.1);
    depositLine(bins, 13_000.37, 2);
    const total = bins.values.reduce((sum, v) => sum + v, 0);
    const centre =
      bins.values.reduce((sum, v, k) => sum + v * (bins.originPerCm + k * bins.stepPerCm), 0) /
      total;
    expect([total, centre]).toEqual([2, expect.closeTo(13_000.37, 9)]);
  });

  it("refuse a line outside them", () => {
    expect(() => {
      depositLine(emptyBins(200, 13_000, 13_001, 0.1), 12_999, 1);
    }).toThrow("outside the bins");
  });

  it("coarsen by an integer factor, keeping the total and the first moment", () => {
    const next = sequence(3);
    const fine = Float64Array.from({ length: 400 }, () => next());
    const read = (i: number): number => fine[i] ?? 0;
    const coarse = coarsenBins(read, 3, -1, 135);
    const [fs = 0, ff = 0] = moment(fine, 1, 0);
    const [cs = 0, cf = 0] = moment(coarse, 3, -1);
    expect([cs / fs, cf / ff]).toEqual([expect.closeTo(1, 12), expect.closeTo(1, 12)]);
  });

  it("take a twelfth of the Doppler half-width at the grid's lowest wavenumber", () => {
    // N₂O at 100 K and 13,100 cm⁻¹: α_D = 7.07 × 10⁻³ cm⁻¹.
    expect(binStepPerCm(100, 13_100, 44.001_062) * 12).toBeCloseTo(7.07e-3, 5);
  });
});

describe("the kernel", () => {
  it("is laid circularly, symmetric about 0", () => {
    const kernel = circularKernel(64, 1, 5, 0.5, 0.1);
    expect([kernel[3], kernel[61]]).toEqual([voigtProfile(3, 0.5, 0.1), voigtProfile(3, 0.5, 0.1)]);
  });

  it("takes the share of each step within the cut-off, so the cut-off falls where it is given", () => {
    // A 5 cm⁻¹ cut-off on whole steps: the sample at 5 stands for [4.5, 5.5], half inside; at 2.2
    // steps, the cut-off 5.5 in steps of 2.5 leaves the sample at 5 a share 0.7.
    expect([
      circularKernel(64, 1, 5, 0.5, 0.1)[5],
      circularKernel(64, 1, 5, 0.5, 0.1)[6],
      circularKernel(64, 2.5, 5.5, 0.5, 0.1)[2],
    ]).toEqual([
      0.5 * voigtProfile(5, 0.5, 0.1),
      0,
      expect.closeTo(0.7 * voigtProfile(5, 0.5, 0.1), 15),
    ]);
  });
});

/** Toy N₂O lines of one width (0.08 cm⁻¹ atm⁻¹, n 0.5) and E″ 0, about a cell. */
function toyLines(cellNm: number, count: number, seed: number): LineSpecies {
  const [low, high] = cellWavenumbersPerCm(cellNm);
  return linesBetween(low - 30, high + 30, count, seed);
}

/** Toy N₂O lines as {@link toyLines}' over a range of wavenumbers, cm⁻¹. */
function linesBetween(
  fromPerCm: number,
  toPerCm: number,
  count: number,
  seed: number,
): LineSpecies {
  const next = sequence(seed);
  const centres = Float64Array.from(
    { length: count },
    () => fromPerCm + (toPerCm - fromPerCm) * next(),
  ).toSorted();
  const fill = (v: number): Float64Array => new Float64Array(count).fill(v);
  const lines: LineList = {
    isotopologues: new Uint16Array(count).fill(21),
    wavenumbersPerCm: centres,
    intensitiesCm: Float64Array.from({ length: count }, () => 10 ** (-28 + 4 * next())),
    lowerEnergiesPerCm: fill(0),
    airHalfWidthsPerCmAtm: fill(0.08),
    airExponents: fill(0.5),
    selfHalfWidthsPerCmAtm: fill(0.08),
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
  return {
    lines,
    partitionSums: new Map([[21, { firstK: 1, values: new Float64Array(1_000).fill(1) }]]),
  };
}

/** The same lines binned at each temperature, as a super-line species. */
function binnedSpecies(lines: LineSpecies, temperaturesK: ReadonlyArray<number>): SuperLineSpecies {
  const low = 13_000;
  const high = 14_500;
  const bins = new Map<number, SuperLineBins>();
  for (const t of temperaturesK) {
    const b = emptyBins(t, low, high, binStepPerCm(t, low, 44.001_062));
    const { wavenumbersPerCm, intensitiesCm } = lines.lines;
    for (let i = 0; i < wavenumbersPerCm.length; i += 1) {
      const nu = wavenumbersPerCm[i] ?? 0;
      depositLine(b, nu, intensityAt(intensitiesCm[i] ?? 0, 0, nu, 1, 1, t));
    }
    bins.set(t, b);
  }
  const at = (t: number): SuperLineBins => {
    const b = bins.get(t);
    if (b === undefined) {
      throw new Error(`no toy bins at ${t} K`);
    }
    return b;
  };
  return {
    gridAt: (t) => {
      const b = at(t);
      return { originPerCm: b.originPerCm, stepPerCm: b.stepPerCm, count: b.values.length };
    },
    binsAt: (t, first, last) => {
      const b = at(t);
      return {
        originPerCm: b.originPerCm,
        stepPerCm: b.stepPerCm,
        first,
        values: Float64Array.from({ length: last - first + 1 }, (_, i) => b.values[first + i] ?? 0),
      };
    },
    molarMassGPerMol: 44.001_062,
    airHalfWidthPerCmAtm: 0.08,
    airExponent: 0.5,
    widthsEstimated: true,
  };
}

describe("a cell lit by a band's cut-off wings", () => {
  // 3,000 lines 5–30 cm⁻¹ above the cell, whose wings stop inside it, as N₂O's 668 nm cell's do:
  // there the spectrum's steps at the cut-off are what the step must resolve alike at every
  // pressure (R08.T4.g's second ruling). Within 4.4 × 10⁻⁵ at 0.1 MPa, where the half-width
  // bounds the step, and 2.9 and 3.0 × 10⁻⁵ at 8.66 and 13.3 MPa; 1.3 and 1.4 × 10⁻⁴ there with the
  // step bounded by the half-width alone.
  const cellNm = 740;
  const [, high] = cellWavenumbersPerCm(cellNm);
  const lines = linesBetween(high + 5, high + 30, 3_000, 5);
  const species = binnedSpecies(lines, [150, 296]);

  it.each([
    [150, 1e5],
    [150, 8.6596e6],
    [296, 1.3335e7],
  ])("draws a curve of growth within 5 × 10⁻⁵ of line by line at %d K and %d Pa", (t, p) => {
    const superLine = superLineSpectra(species, PLAIN)(cellNm, t, p, [1]);
    const exact = lineByLine(lines, PLAIN)(cellNm, t, p, [1]);
    if (superLine === undefined || exact === undefined) {
      throw new Error("the toy cell has lines");
    }
    let worst = 0;
    for (let k = 0; k <= 72; k += 1) {
      const u = 1e20 * 10 ** (k / 6);
      worst = Math.max(
        worst,
        Math.abs(
          intervalTransmittance(superLine.node, 0, 1, u) -
            intervalTransmittance(exact.node, 0, 1, u),
        ),
      );
    }
    expect(worst).toBeLessThan(5e-5);
  });
});

describe("a cell's super-line spectrum", () => {
  const cellNm = 740;
  const lines = toyLines(cellNm, 3_000, 11);
  const temperatures = [150, 296];
  const species = binnedSpecies(lines, temperatures);

  it.each([
    [150, 10],
    [150, 1e4],
    [296, 1e5],
    [296, 3e6],
    // Where each line's wing stops at the cut-off at 13–15% of its peak, the step bounded in cm⁻¹.
    [150, 8.6596e6],
    [296, 1.3335e7],
  ])(
    "draws a curve of growth within 3 × 10⁻⁵ of the same lines line by line at %d K and %d Pa",
    (t, p) => {
      const superLine = superLineSpectra(species, PLAIN)(cellNm, t, p, [1]);
      const exact = lineByLine(lines, PLAIN)(cellNm, t, p, [1]);
      if (superLine === undefined || exact === undefined) {
        throw new Error("the toy cell has lines");
      }
      let worst = 0;
      for (let k = 0; k <= 60; k += 1) {
        const u = 1e22 * 10 ** (k / 6);
        worst = Math.max(
          worst,
          Math.abs(
            intervalTransmittance(superLine.node, 0, 1, u) -
              intervalTransmittance(exact.node, 0, 1, u),
          ),
        );
      }
      expect(worst).toBeLessThan(3e-5);
    },
  );

  it("keeps the cell's mean cross-section to the line-by-line one's within 10⁻⁴", () => {
    const superLine = superLineSpectra(species, PLAIN)(cellNm, 296, 1e5, [1]);
    const exact = lineByLine(lines, PLAIN)(cellNm, 296, 1e5, [1]);
    if (superLine === undefined || exact === undefined) {
      throw new Error("the toy cell has lines");
    }
    expect(Math.abs(nodeMean(superLine.node) / nodeMean(exact.node) - 1)).toBeLessThan(1e-4);
  });

  it("marks every node estimated where the widths are a stated estimate", () => {
    expect(superLineSpectra(species, PLAIN)(cellNm, 150, 10, [1])?.estimatedShape).toBe(true);
  });

  it("takes as many points as the line-by-line rule for its narrowest half-width", () => {
    expect(superLinePointCount(cellNm, 150, 1e-6, 44.001_062, 25)).toBeGreaterThan(512);
  });

  it.each([
    [10, false],
    [8.6596e6, true],
  ])(
    "coarsens its bins at %d Pa under a tenth of the half-width and a 600th of the cut-off, the cut-off's binding: %s",
    (p, byCutoff) => {
      const cache = new Map<string, TransformedWindow>();
      superLineCell(cellNm, species, 150, p, 25, 512, cache);
      const steps = [...cache.values()].map((w) => w.stepPerCm);
      const bound = 25 / SUPER_LINE_STEPS_PER_CUTOFF;
      expect([
        SUPER_LINE_STEPS_PER_CUTOFF,
        steps.length,
        steps.every((h) => h < bound),
        steps.every((h) => h > bound / 2) === byCutoff,
      ]).toEqual([600, 1, true, true]);
    },
  );

  it("finds no lines in a cell whose window holds no bin", () => {
    const empty = binnedSpecies(toyLines(cellNm, 0, 1), [296]);
    expect(superLineCell(cellNm, empty, 296, 1e5, 25, 512, new Map())).toBeUndefined();
  });

  it("merges with lines drawn line by line on the same points, the means adding", () => {
    const hitran = toyLines(cellNm, 40, 5);
    const cell = mergedCell(cellNm, hitran, species, 296, 1e5, PLAIN_SHAPE, new Map());
    if (cell === undefined) {
      throw new Error("the toy cell has lines");
    }
    let sum = 0;
    for (const v of cell.crossSectionsCm2) {
      sum += v;
    }
    const mean = sum / cell.crossSectionsCm2.length;
    expect(Math.abs(mean - (cell.linesMeanCm2 + cell.superLinesMeanCm2)) / mean).toBeLessThan(
      1e-12,
    );
  });

  it("takes every super-line band and no line below the split", () => {
    const merged = mergedSpectra(toyLines(cellNm, 40, 5), species, species, PLAIN, 714);
    const below = superLineSpectra(species, PLAIN)(713, 296, 1e5, [1]);
    expect(merged(713, 296, 1e5, [1])?.node.sorted).toEqual(below?.node.sorted);
  });

  it("estimates a merged node's shape where its super-lines' widths are a stated estimate", () => {
    const merged = mergedSpectra(toyLines(cellNm, 40, 5), species, species, PLAIN, 714);
    expect(merged(cellNm, 296, 1e5, [1])?.estimatedShape).toBe(true);
  });

  it("refuses a policy of two variants, which super-lines do not take", () => {
    expect(() =>
      superLineSpectra(species, {
        ...PLAIN,
        variants: [{ broadener: "self" }, { broadener: "air" }],
      }),
    ).toThrow("one variant");
  });
});

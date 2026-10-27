import { describe, expect, it } from "vitest";

import {
  type CurveRows,
  EXP_SUM_TOLERANCE,
  expSumCoefficientsAt,
  expSumRowsAt,
  expSumTransmittance,
  fitExpSum,
  nonNegativeLeastSquares,
} from "./expSum";

/** A curve's vertical amount, molecules m⁻²: any scale serves. */
const VERTICAL = 1e25;

/** The curves' columns, `absorbers.ts`'s: 40 a decade from 10⁻³ to 10³ of the vertical amount. */
const LOG_COLUMNS = Float64Array.from(
  { length: 241 },
  (_, j) => Math.log(VERTICAL) + (-3 + j / 40) * Math.LN10,
);

/** A curve of rows from a sum known exactly: weights and, at each row, k U. */
function knownCurve(
  weights: ReadonlyArray<number>,
  ratesByRow: ReadonlyArray<ReadonlyArray<number>>,
  pressuresPa: ReadonlyArray<number>,
): CurveRows {
  const nodes = LOG_COLUMNS.length;
  const transmittance = new Float64Array(nodes * ratesByRow.length);
  for (const [r, rates] of ratesByRow.entries()) {
    for (const [j, lnU] of LOG_COLUMNS.entries()) {
      const x = Math.exp(lnU) / VERTICAL;
      transmittance[r * nodes + j] = weights.reduce(
        (sum, w, i) => sum + w * Math.exp(-(rates[i] ?? Number.NaN) * x),
        0,
      );
    }
  }
  const logPressures = Float64Array.from(pressuresPa, Math.log);
  return {
    logColumns: LOG_COLUMNS,
    logPressures,
    readTopLogPressure: logPressures.at(-1) ?? Number.NaN,
    transmittance,
  };
}

describe("non-negative least squares", () => {
  it("holds a component at 0 where the free solution would take it negative", () => {
    // Free: x = (7/6, −5/6); with x₂ ≥ 0 the best is x = (3/4, 0).
    const a = Float64Array.of(1, 0, 0, 1, 1, 1);
    const x = nonNegativeLeastSquares(a, 3, 2, Float64Array.of(1, -1, 0.5));
    expect(Math.abs((x[0] ?? Number.NaN) - 0.75)).toBeLessThan(1e-14);
    expect(x[1]).toBe(0);
  });

  it("is the free least-squares solution where that is not negative", () => {
    const a = Float64Array.of(2, 1, 1, 3, 0, 1);
    const truth = [0.4, 1.3];
    const b = Float64Array.of(2 * 0.4 + 1.3, 0.4 + 3 * 1.3, 1.3);
    const x = nonNegativeLeastSquares(a, 3, 2, b);
    for (const [i, value] of truth.entries()) {
      expect(Math.abs((x[i] ?? Number.NaN) - value)).toBeLessThan(1e-13);
    }
  });
});

describe("an exponential sum fitted to a curve of growth", () => {
  it("is one term of k = 0 for a curve within the tolerance of 1", () => {
    const fit = fitExpSum(knownCurve([1], [[1e-7]], [1e5]), Math.log(1e5));
    expect([...fit.weights]).toEqual([1]);
    expect([...fit.coefficients]).toEqual([0]);
    expect(fit.largestError).toBeLessThanOrEqual(EXP_SUM_TOLERANCE);
  });

  it("recovers a sum of three terms within the tolerance in three terms, and to 10⁻⁶ when asked", () => {
    const curve = knownCurve([0.5, 0.3, 0.2], [[0.02, 1.5, 40]], [1e5]);
    const fit = fitExpSum(curve, Math.log(1e5));
    expect(fit.weights.length).toBeLessThanOrEqual(3);
    expect(fit.largestError).toBeLessThanOrEqual(EXP_SUM_TOLERANCE);
    const tight = fitExpSum(curve, Math.log(1e5), { tolerance: 1e-6, maxTerms: 8 });
    expect(tight.largestError).toBeLessThanOrEqual(1e-6);
    expect(Math.abs(tight.weights.reduce((sum, w) => sum + w, 0) - 1)).toBeLessThan(1e-12);
  });

  it("shares its weights over the rows and fits each row's k, a sum whose k go as √p to 10⁻⁵", () => {
    const pressures = [1e3, 1e4, 1e5, 1e6, 1e7];
    const base = [0.05, 2, 60];
    const rows = pressures.map((p) => base.map((k) => k * Math.sqrt(p / 1e5)));
    const fit = fitExpSum(knownCurve([0.6, 0.3, 0.1], rows, pressures), Math.log(1e5), {
      tolerance: 1e-6,
      maxTerms: 8,
    });
    expect(fit.referenceRow).toBe(2);
    expect(fit.largestError).toBeLessThanOrEqual(1e-5);
    expect(fit.rowErrors.length).toBe(pressures.length);
  });

  it("reads k at a pressure between its rows, ln k linear in ln p, and holds it beyond them", () => {
    const pressures = [1e4, 1e5, 1e6];
    const rows = pressures.map((p) => [3 * (p / 1e5)]);
    const fit = fitExpSum(knownCurve([1], rows, pressures), Math.log(1e5), {
      tolerance: 1e-9,
      maxTerms: 8,
    });
    const n = fit.weights.length;
    expect(n).toBe(1);
    const k = (p: number): number => expSumCoefficientsAt(fit, p)[0] ?? Number.NaN;
    const kAt = (row: number): number => fit.coefficients[row * n] ?? Number.NaN;
    expect(Math.abs(k(Math.sqrt(1e4 * 1e5)) - Math.sqrt(kAt(0) * kAt(1)))).toBeLessThan(
      1e-12 * kAt(1),
    );
    expect(Math.abs(k(10) - kAt(0))).toBeLessThan(1e-14 * kAt(0));
    expect(Math.abs(k(1e9) - kAt(2))).toBeLessThan(1e-14 * kAt(2));
    expect(expSumRowsAt(fit, 1e5)).toEqual({ row: 0, next: 1, share: 1 });
    expect(Math.abs(kAt(1) * VERTICAL - 3)).toBeLessThan(1e-6);
    expect(Math.abs(expSumTransmittance(fit, 1, VERTICAL) - Math.exp(-3))).toBeLessThan(1e-8);
  });

  it("refuses a curve of several rows read with no pressure, and a curve whose sizes disagree", () => {
    const fit = fitExpSum(knownCurve([1], [[1], [2]], [1e4, 1e5]), Math.log(1e5));
    expect(() => expSumCoefficientsAt(fit, undefined)).toThrow(RangeError);
    expect(() =>
      fitExpSum({ ...knownCurve([1], [[1]], [1e5]), transmittance: new Float64Array(3) }, 0),
    ).toThrow(RangeError);
  });
});

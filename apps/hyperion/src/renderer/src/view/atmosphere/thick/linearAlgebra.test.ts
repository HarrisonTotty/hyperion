import { describe, expect, it } from "vitest";

import {
  type BandMatrix,
  bandIndex,
  bandMatrix,
  choleskyLower,
  factorBand,
  solveBand,
  solveLower,
  solveLowerTransposed,
  symmetricEigen,
} from "./linearAlgebra";

function entry(values: Float64Array, i: number): number {
  const value = values[i];
  if (value === undefined) {
    throw new Error(`no entry ${i}`);
  }
  return value;
}

/** A fixed symmetric positive-definite matrix: B Bᵀ + n 1, B's entries from a fixed formula. */
function positiveDefinite(n: number): Float64Array {
  const b = Float64Array.from({ length: n * n }, (_, k) => Math.sin(1.3 * k + 0.7));
  const a = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j < n; j += 1) {
      let sum = i === j ? n : 0;
      for (let k = 0; k < n; k += 1) {
        sum += entry(b, i * n + k) * entry(b, j * n + k);
      }
      a[i * n + j] = sum;
    }
  }
  return a;
}

describe("the Cholesky factorisation", () => {
  const n = 9;
  const a = positiveDefinite(n);

  it("factors a positive-definite matrix as L Lᵀ", () => {
    const l = choleskyLower(a, n);
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        let sum = 0;
        for (let k = 0; k < n; k += 1) {
          sum += entry(l, i * n + k) * entry(l, j * n + k);
        }
        expect(sum).toBeCloseTo(entry(a, i * n + j), 12);
      }
    }
  });

  it("leaves L zero above its diagonal", () => {
    const l = choleskyLower(a, n);
    for (let i = 0; i < n; i += 1) {
      for (let j = i + 1; j < n; j += 1) {
        expect(entry(l, i * n + j)).toBe(0);
      }
    }
  });

  it("inverts L and Lᵀ by its triangular solves", () => {
    const l = choleskyLower(a, n);
    const x = Float64Array.from({ length: n }, (_, i) => i - 3.5);
    const lx = new Float64Array(n);
    const ltx = new Float64Array(n);
    for (let i = 0; i < n; i += 1) {
      for (let k = 0; k < n; k += 1) {
        lx[i] = entry(lx, i) + entry(l, i * n + k) * entry(x, k);
        ltx[i] = entry(ltx, i) + entry(l, k * n + i) * entry(x, k);
      }
    }
    solveLower(l, n, lx);
    solveLowerTransposed(l, n, ltx);
    x.forEach((value, i) => {
      expect(entry(lx, i)).toBeCloseTo(value, 13);
      expect(entry(ltx, i)).toBeCloseTo(value, 13);
    });
  });

  it("refuses a matrix that is not positive definite", () => {
    for (const matrix of [
      [1, 2, 2, 1],
      [1, 0, 0, 0],
      [-1, 0, 0, 1],
    ]) {
      expect(() => choleskyLower(Float64Array.from(matrix), 2)).toThrow(RangeError);
    }
  });

  it("refuses a matrix that is not n × n", () => {
    expect(() => choleskyLower(new Float64Array(5), 2)).toThrow(RangeError);
  });

  it("refuses a triangular solve of the wrong size", () => {
    const l = choleskyLower(a, n);
    for (const solve of [solveLower, solveLowerTransposed]) {
      expect(() => solve(l, n, new Float64Array(n - 1))).toThrow(RangeError);
      expect(() => solve(new Float64Array(n), n, new Float64Array(n))).toThrow(RangeError);
    }
  });
});

describe("the cyclic Jacobi method", () => {
  it("finds the second-difference matrix's eigenvalues 2 − 2 cos(kπ ÷ (n + 1)), ascending", () => {
    const n = 12;
    const a = new Float64Array(n * n);
    for (let i = 0; i < n; i += 1) {
      a[i * n + i] = 2;
      if (i + 1 < n) {
        a[i * n + i + 1] = -1;
        a[(i + 1) * n + i] = -1;
      }
    }
    const { values, vectors } = symmetricEigen(a, n);
    values.forEach((value, k) => {
      expect(value).toBeCloseTo(2 - 2 * Math.cos(((k + 1) * Math.PI) / (n + 1)), 14);
    });
    // A V = V Λ and Vᵀ V = 1.
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        let av = 0;
        let vtv = 0;
        for (let k = 0; k < n; k += 1) {
          av += entry(a, i * n + k) * entry(vectors, k * n + j);
          vtv += entry(vectors, k * n + i) * entry(vectors, k * n + j);
        }
        expect(av).toBeCloseTo(entry(vectors, i * n + j) * entry(values, j), 13);
        expect(vtv).toBeCloseTo(i === j ? 1 : 0, 13);
      }
    }
  });

  it("keeps a graded positive-definite matrix's small eigenvalue to its relative accuracy", () => {
    // D Â D with Â = [[1, ½], [½, 1]] (condition 3) and D = diag(1, 10⁻⁸): its eigenvalues are about
    // 1 and det ÷ 1 = 0.75 × 10⁻¹⁶, which Jacobi keeps to its relative accuracy (Demmel and Veselić
    // 1992), where a method accurate only to ε ‖A‖ would lose it entirely.
    const a = Float64Array.from([1, 0.5e-8, 0.5e-8, 1e-16]);
    const { values } = symmetricEigen(a, 2);
    const determinant = 1e-16 - 0.25e-16;
    const product = entry(values, 0) * entry(values, 1);
    expect(Math.abs(product - determinant) / determinant).toBeLessThan(1e-14);
    expect(Math.abs(entry(values, 0) - 0.75e-16) / 0.75e-16).toBeLessThan(1e-14);
  });
});

/** A band matrix with zero diagonal entries, which needs the pivoting. */
function needsPivots(n: number, lower: number, upper: number): BandMatrix {
  const m = bandMatrix(n, lower, upper);
  for (let r = 0; r < n; r += 1) {
    for (let c = Math.max(0, r - lower); c <= Math.min(n - 1, r + upper); c += 1) {
      const value = r === c && r % 3 === 0 ? 0 : Math.cos(0.9 * r + 1.7 * c) + (r === c ? 0.2 : 0);
      m.entries[bandIndex(m, r, c)] = value;
    }
  }
  return m;
}

function multiplyBand(m: BandMatrix, x: Float64Array): Float64Array {
  const out = new Float64Array(m.order);
  for (let r = 0; r < m.order; r += 1) {
    let sum = 0;
    for (let c = Math.max(0, r - m.lower); c <= Math.min(m.order - 1, r + m.upper); c += 1) {
      sum += entry(m.entries, bandIndex(m, r, c)) * entry(x, c);
    }
    out[r] = sum;
  }
  return out;
}

describe("banded elimination with partial pivoting", () => {
  it("solves several right-hand sides on one factorisation", () => {
    const n = 40;
    const original = needsPivots(n, 3, 5);
    const copy = { ...original, entries: Float64Array.from(original.entries) };
    const lu = factorBand(copy);
    for (const shift of [0, 1.5, -2]) {
      const x = Float64Array.from({ length: n }, (_, i) => Math.sin(0.3 * i + shift) + shift);
      const b = multiplyBand(original, x);
      solveBand(lu, b);
      b.forEach((value, i) => {
        expect(value).toBeCloseTo(entry(x, i), 11);
      });
    }
  });

  it("refuses a singular matrix", () => {
    const m = bandMatrix(4, 1, 1);
    for (let r = 0; r < 4; r += 1) {
      m.entries[bandIndex(m, r, r)] = r === 2 ? 0 : 1;
    }
    expect(() => factorBand(m)).toThrow(Error);
  });

  it("refuses an entry off the band or off the matrix", () => {
    const m = bandMatrix(4, 1, 1);
    for (const [row, column] of [
      [0, 2],
      [3, 1],
      [-1, 0],
      [4, 4],
    ] as const) {
      expect(() => bandIndex(m, row, column)).toThrow(RangeError);
    }
  });

  it("refuses an order below 1 or a bandwidth that is negative or not whole", () => {
    for (const [order, lower, upper] of [
      [0, 1, 1],
      [3, -1, 1],
      [3, 1, 1.5],
    ] as const) {
      expect(() => bandMatrix(order, lower, upper)).toThrow(RangeError);
    }
  });

  it("refuses a right-hand side of the wrong size", () => {
    const fine = bandMatrix(3, 1, 1);
    for (let r = 0; r < 3; r += 1) {
      fine.entries[bandIndex(fine, r, r)] = 2;
    }
    expect(() => solveBand(factorBand(fine), new Float64Array(2))).toThrow(RangeError);
  });
});

import { describe, expect, it } from "vitest";

import {
  type BandMatrix,
  bandIndex,
  bandMatrix,
  choleskyLower,
  factorBand,
  factorDense,
  realEigen,
  type RealEigensystem,
  realSchurForm,
  refinedShiftedSolve,
  schurEigensystem,
  shiftedSolveScratch,
  solveBand,
  solveBandColumns,
  solveDense,
  solveLower,
  solveShiftedSchur,
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
    // D Â D with Â = [[1, ½], [½, 1]] (condition 3) and D = diag(1, 10⁻⁸): its eigenvalues are
    // about 1 and det ÷ 1 = 0.75 × 10⁻¹⁶, which Jacobi keeps to its relative accuracy (Demmel and
    // Veselić 1992), where a method accurate only to ε ‖A‖ would lose it entirely.
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

/** `m` with about half its band's entries set to 0 by a fixed pattern, its diagonal kept. */
function withZeros(m: BandMatrix): BandMatrix {
  for (let r = 0; r < m.order; r += 1) {
    for (let c = Math.max(0, r - m.lower); c <= Math.min(m.order - 1, r + m.upper); c += 1) {
      if (r !== c && (7 * r + 3 * c) % 5 < 3) {
        m.entries[bandIndex(m, r, c)] = 0;
      }
    }
  }
  return m;
}

/**
 * The banded elimination with partial pivoting as R08.T14.a built it, every product taken, kept as
 * the oracle of `factorBand` and `solveBand`, which skip the factors' zeros (R08.T14.b).
 */
function denseBandElimination(matrix: BandMatrix): {
  readonly entries: Float64Array;
  readonly pivots: Int32Array;
  readonly solve: (b: Float64Array) => Float64Array;
} {
  const { order: n, lower: ml, upper: mu, width: w, entries: a } = matrix;
  const pivots = new Int32Array(n);
  for (let k = 0; k < n; k += 1) {
    const last = Math.min(n - 1, k + ml);
    let pivotRow = k;
    let largest = Math.abs(entry(a, k * w + ml));
    for (let r = k + 1; r <= last; r += 1) {
      const candidate = Math.abs(entry(a, r * w + (k - r + ml)));
      if (candidate > largest) {
        largest = candidate;
        pivotRow = r;
      }
    }
    pivots[k] = pivotRow;
    const columnEnd = Math.min(n - 1, k + ml + mu);
    if (pivotRow !== k) {
      for (let c = k; c <= columnEnd; c += 1) {
        const ik = k * w + (c - k + ml);
        const ip = pivotRow * w + (c - pivotRow + ml);
        const held = entry(a, ik);
        a[ik] = entry(a, ip);
        a[ip] = held;
      }
    }
    const pivot = entry(a, k * w + ml);
    for (let r = k + 1; r <= last; r += 1) {
      const ir = r * w + (k - r + ml);
      const multiplier = entry(a, ir) / pivot;
      a[ir] = multiplier;
      if (multiplier !== 0) {
        for (let c = k + 1; c <= columnEnd; c += 1) {
          const target = r * w + (c - r + ml);
          a[target] = entry(a, target) - multiplier * entry(a, k * w + (c - k + ml));
        }
      }
    }
  }
  const solve = (b: Float64Array): Float64Array => {
    for (let k = 0; k < n; k += 1) {
      const p = pivots[k] ?? k;
      if (p !== k) {
        const held = entry(b, k);
        b[k] = entry(b, p);
        b[p] = held;
      }
      const bk = entry(b, k);
      if (bk !== 0) {
        for (let r = k + 1; r <= Math.min(n - 1, k + ml); r += 1) {
          b[r] = entry(b, r) - entry(a, r * w + (k - r + ml)) * bk;
        }
      }
    }
    for (let k = n - 1; k >= 0; k -= 1) {
      let sum = entry(b, k);
      for (let c = k + 1; c <= Math.min(n - 1, k + ml + mu); c += 1) {
        sum -= entry(a, k * w + (c - k + ml)) * entry(b, c);
      }
      b[k] = sum / entry(a, k * w + ml);
    }
    return b;
  };
  return { entries: a, pivots, solve };
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

  it("solves right-hand sides together, each column to the bit of its solve alone (R08.T14.b)", () => {
    const n = 40;
    const columns = 5;
    const lu = factorBand(needsPivots(n, 3, 5));
    const together = Float64Array.from({ length: n * columns }, (_, k) =>
      k % 7 === 3 ? 0 : Math.sin(0.37 * k) - 0.1,
    );
    const alone = Array.from({ length: columns }, (_column, j) =>
      Float64Array.from({ length: n }, (_row, r) => entry(together, r * columns + j)),
    );
    solveBandColumns(lu, together, columns);
    alone.forEach((b, j) => {
      solveBand(lu, b);
      b.forEach((value, r) => {
        expect(Object.is(entry(together, r * columns + j), value)).toBe(true);
      });
    });
    expect(() => {
      solveBandColumns(lu, new Float64Array(n * columns), columns + 1);
    }).toThrow(RangeError);
  });

  it("eliminates over its entries not 0 alone, as the dense elimination does to the bit but for a zero's sign (R08.T14.b)", () => {
    const n = 48;
    const sparse = withZeros(needsPivots(n, 7, 9));
    const dense = denseBandElimination({ ...sparse, entries: Float64Array.from(sparse.entries) });
    const lu = factorBand(sparse);
    expect(Array.from(lu.pivots)).toEqual(Array.from(dense.pivots));
    lu.factors.entries.forEach((value, i) => {
      // `===` holds 0 and −0 equal and every other value to its bits.
      expect(value === entry(dense.entries, i)).toBe(true);
    });
    // The zeros were skipped: U and L hold fewer entries than their bands.
    expect(lu.upperStart[n]).toBeLessThan(n * (7 + 9));
    expect(lu.lowerStart[n]).toBeLessThan(n * 7);
    const b = Float64Array.from({ length: n }, (_, i) => Math.cos(0.41 * i) + 0.3);
    const expected = dense.solve(Float64Array.from(b));
    solveBand(lu, b);
    b.forEach((value, i) => {
      expect(value === entry(expected, i)).toBe(true);
    });
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

/** A fixed nonsymmetric matrix, its entries from a fixed formula, with n on its diagonal. */
function nonsymmetric(n: number): Float64Array {
  return Float64Array.from({ length: n * n }, (_, k) =>
    Math.floor(k / n) === k % n ? n : Math.sin(0.9 * k + 0.4) + 0.3 * Math.cos(2.1 * k),
  );
}

/** A × B, both n × n. */
function matrixProduct(a: Float64Array, b: Float64Array, n: number): Float64Array {
  const out = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j < n; j += 1) {
      let sum = 0;
      for (let k = 0; k < n; k += 1) {
        sum += entry(a, i * n + k) * entry(b, k * n + j);
      }
      out[i * n + j] = sum;
    }
  }
  return out;
}

/** A⁻¹ by elimination, column by column. */
function inverse(a: Float64Array, n: number): Float64Array {
  const lu = factorDense(a, n);
  const out = new Float64Array(n * n);
  for (let j = 0; j < n; j += 1) {
    const column = Float64Array.from({ length: n }, (_, i) => (i === j ? 1 : 0));
    solveDense(lu, column);
    column.forEach((value, i) => {
      out[i * n + j] = value;
    });
  }
  return out;
}

/** The largest |A v − λ v| over the eigensystem's pairs, real and complex, and the largest |‖v‖ − 1|. */
function residuals(a: Float64Array, n: number, eigen: RealEigensystem): readonly [number, number] {
  let worst = 0;
  let norm = 0;
  for (let j = 0; j < n; j += 1) {
    const li = entry(eigen.valuesIm, j);
    if (li < 0) {
      continue;
    }
    const lr = entry(eigen.valuesRe, j);
    const x = Float64Array.from({ length: n }, (_, i) => entry(eigen.vectors, i * n + j));
    const y = Float64Array.from({ length: n }, (_, i) =>
      li > 0 ? entry(eigen.vectors, i * n + j + 1) : 0,
    );
    let length = 0;
    for (let i = 0; i < n; i += 1) {
      let ax = 0;
      let ay = 0;
      for (let k = 0; k < n; k += 1) {
        ax += entry(a, i * n + k) * entry(x, k);
        ay += entry(a, i * n + k) * entry(y, k);
      }
      // A (x + iy) = (lr + i li)(x + iy).
      worst = Math.max(
        worst,
        Math.abs(ax - (lr * entry(x, i) - li * entry(y, i))),
        Math.abs(ay - (li * entry(x, i) + lr * entry(y, i))),
      );
      length += entry(x, i) ** 2 + entry(y, i) ** 2;
    }
    norm = Math.max(norm, Math.abs(Math.sqrt(length) - 1));
  }
  return [worst, norm];
}

describe("dense elimination with partial pivoting", () => {
  it("solves a nonsymmetric system, pivoting past a zero leading entry", () => {
    const n = 7;
    const a = nonsymmetric(n);
    a[0] = 0;
    const x = Float64Array.from({ length: n }, (_, i) => 1.5 - 0.7 * i);
    const b = new Float64Array(n);
    for (let i = 0; i < n; i += 1) {
      for (let k = 0; k < n; k += 1) {
        b[i] = entry(b, i) + entry(a, i * n + k) * entry(x, k);
      }
    }
    const lu = factorDense(a, n);
    expect(lu.pivots[0]).not.toBe(0);
    solveDense(lu, b);
    // Measured 8.3 × 10⁻¹³ (2026-10-27), the system's conditioning with its first pivot moved.
    x.forEach((value, i) => {
      expect(Math.abs(entry(b, i) - value)).toBeLessThan(1e-11);
    });
  });

  it("refuses a singular matrix and a right-hand side or matrix of the wrong size", () => {
    expect(() => factorDense(new Float64Array([1, 2, 2, 4]), 2)).toThrow(Error);
    expect(() => factorDense(new Float64Array(3), 2)).toThrow(RangeError);
    const lu = factorDense(new Float64Array([2, 1, 1, 3]), 2);
    expect(() => {
      solveDense(lu, new Float64Array(3));
    }).toThrow(RangeError);
  });
});

describe("the real nonsymmetric eigenproblem", () => {
  it("solves (A − s 1) x = b on A's Schur form as elimination does, to 10⁻¹³, two complex pairs among the blocks (R08.T14.e)", () => {
    const n = 7;
    const lambda = new Float64Array(n * n);
    lambda[0] = 3;
    lambda[1 * n + 1] = -1.5;
    lambda[2 * n + 2] = 0.25;
    for (const [k, re, im] of [
      [3, 1.2, 0.8],
      [5, -0.4, 2.5],
    ] as const) {
      lambda[k * n + k] = re;
      lambda[(k + 1) * n + k + 1] = re;
      lambda[k * n + k + 1] = im;
      lambda[(k + 1) * n + k] = -im;
    }
    const s = nonsymmetric(n);
    const a = matrixProduct(matrixProduct(s, lambda, n), inverse(s, n), n);
    const schur = realSchurForm(a, n);
    // The eigensystem from the form is realEigen's to the bit, the form left as it was.
    expect(schurEigensystem(schur, n)).toEqual(realEigen(a, n));
    for (const shift of [0.7, -2.2, 9]) {
      const b = Float64Array.from({ length: n }, (_, i) => Math.cos(1.3 * i) - 0.2);
      const shifted = Float64Array.from(a, (value, k) =>
        Math.floor(k / n) === k % n ? value - shift : value,
      );
      const expected = Float64Array.from(b);
      solveDense(factorDense(shifted, n), expected);
      const refined = Float64Array.from(b);
      solveShiftedSchur(schur, n, shift, b, new Float64Array(n));
      refinedShiftedSolve(a, schur, n, shift, refined, shiftedSolveScratch(n));
      const largest = Math.max(...Array.from(expected, Math.abs));
      b.forEach((value, i) => {
        expect(Math.abs(value - entry(expected, i))).toBeLessThan(1e-13 * largest);
        expect(Math.abs(entry(refined, i) - entry(expected, i))).toBeLessThan(1e-14 * largest);
      });
    }
    expect(() => {
      solveShiftedSchur(schur, n, 3, new Float64Array(n), new Float64Array(n));
    }).not.toThrow();
    expect(() => {
      solveShiftedSchur(schur, n, 0, new Float64Array(n - 1), new Float64Array(n));
    }).toThrow(RangeError);
  });

  it("finds the eigenvalues S Λ S⁻¹ was built with, two complex pairs among them", () => {
    // Λ: 3, −1.5, 0.25 and the blocks [[a, b], [−b, a]], eigenvalues a ± bi.
    const n = 7;
    const lambda = new Float64Array(n * n);
    lambda[0] = 3;
    lambda[1 * n + 1] = -1.5;
    lambda[2 * n + 2] = 0.25;
    for (const [k, re, im] of [
      [3, 1.2, 0.8],
      [5, -0.4, 2.5],
    ] as const) {
      lambda[k * n + k] = re;
      lambda[(k + 1) * n + k + 1] = re;
      lambda[k * n + k + 1] = im;
      lambda[(k + 1) * n + k] = -im;
    }
    const s = nonsymmetric(n);
    const a = matrixProduct(matrixProduct(s, lambda, n), inverse(s, n), n);
    const eigen = realEigen(a, n);
    const found = Array.from(eigen.valuesRe, (re, j) => [re, entry(eigen.valuesIm, j)] as const)
      .toSorted((x, y) => x[0] - y[0] || x[1] - y[1])
      .map(([re, im]) => [Number(re.toFixed(10)), Number(im.toFixed(10))]);
    expect(found).toEqual([
      [-1.5, 0],
      [-0.4, -2.5],
      [-0.4, 2.5],
      [0.25, 0],
      [1.2, -0.8],
      [1.2, 0.8],
      [3, 0],
    ]);
    const [worst, norm] = residuals(a, n, eigen);
    expect(worst).toBeLessThan(1e-12);
    expect(norm).toBeLessThan(1e-14);
  });

  it("holds every eigenpair of a general matrix, and a pair's conjugates in order, + then −", () => {
    const n = 12;
    const a = nonsymmetric(n);
    // A rotation-like part makes complex pairs likely: a skew-symmetric addition.
    for (let i = 0; i < n; i += 1) {
      for (let j = i + 1; j < n; j += 1) {
        const skew = 2 * Math.cos(0.37 * (i + 2 * j));
        a[i * n + j] = entry(a, i * n + j) + skew;
        a[j * n + i] = entry(a, j * n + i) - skew;
      }
    }
    const eigen = realEigen(a, n);
    const pairs = Array.from(eigen.valuesIm.keys()).filter((j) => entry(eigen.valuesIm, j) > 0);
    expect(pairs.length).toBeGreaterThan(0);
    expect(
      pairs.map((j) => [
        entry(eigen.valuesRe, j + 1) - entry(eigen.valuesRe, j),
        entry(eigen.valuesIm, j + 1) + entry(eigen.valuesIm, j),
      ]),
    ).toEqual(pairs.map(() => [0, 0]));
    const [worst, norm] = residuals(a, n, eigen);
    expect(worst).toBeLessThan(1e-11);
    expect(norm).toBeLessThan(1e-14);
  });

  it("agrees with Jacobi on a symmetric matrix, its eigenvalues real", () => {
    const n = 9;
    const a = positiveDefinite(n);
    const eigen = realEigen(a, n);
    expect(eigen.valuesIm.every((im) => im === 0)).toBe(true);
    const jacobi = symmetricEigen(a, n).values;
    const sorted = Array.from(eigen.valuesRe).toSorted((x, y) => x - y);
    sorted.forEach((value, j) => {
      expect(Math.abs(value - entry(jacobi, j))).toBeLessThan(1e-12 * entry(jacobi, n - 1));
    });
    expect(residuals(a, n, eigen)[0]).toBeLessThan(1e-11);
  });

  it("finds the cube roots of 1 from their companion matrix", () => {
    const eigen = realEigen(new Float64Array([0, 0, 1, 1, 0, 0, 0, 1, 0]), 3);
    const roots = Array.from(eigen.valuesRe, (re, j) => [re, entry(eigen.valuesIm, j)] as const)
      .toSorted((x, y) => x[0] - y[0] || x[1] - y[1])
      .flatMap(([re, im]) => [re, im]);
    const expected = [-0.5, -Math.sqrt(3) / 2, -0.5, Math.sqrt(3) / 2, 1, 0];
    roots.forEach((value, i) => {
      expect(Math.abs(value - (expected[i] ?? Number.NaN))).toBeLessThan(1e-14);
    });
  });

  it("converges where eigenvalues repeat to rounding, as the vector mode's high modes repeat 1 ÷ μ²", () => {
    // Each of four values four times, and a perturbation of rank one and size 10⁻⁸: three of each
    // four stay exactly repeated, which stalled the iteration at rounding before its relaxed test.
    const n = 16;
    const values = [207.6, 9.2, 2.2, 1.155];
    for (let trial = 0; trial < 40; trial += 1) {
      const a = new Float64Array(n * n);
      const u = Float64Array.from({ length: n }, (_, i) => Math.sin(1.7 * i + trial));
      const v = Float64Array.from({ length: n }, (_, i) => Math.cos(2.3 * i - 0.5 * trial));
      for (let i = 0; i < n; i += 1) {
        for (let j = 0; j < n; j += 1) {
          a[i * n + j] =
            (i === j ? (values[Math.floor(i / 4)] ?? 1) : 0) + 1e-8 * entry(u, i) * entry(v, j);
        }
      }
      const eigen = realEigen(a, n);
      const [worst] = residuals(a, n, eigen);
      expect(worst).toBeLessThan(1e-10);
    }
  });

  it("converges on a nearly defective cluster, which no shift separates, by splitting it", () => {
    // S J S⁻¹ with J's 2 three times in a Jordan block coupled by 10⁻¹⁰, and 6, 7 and 8: without
    // the cluster's split the iteration stalls in its 120 sweeps (checked 2026-10-27).
    const n = 6;
    const jordan = new Float64Array(n * n);
    for (let i = 0; i < n; i += 1) {
      jordan[i * n + i] = i < 3 ? 2 : 3 + i;
    }
    jordan[1] = 1e-10;
    jordan[n + 2] = 1e-10;
    const s = Float64Array.from(
      { length: n * n },
      (_, k) => (Math.floor(k / n) === k % n ? 2 : 0) + 0.3 * Math.sin(1.1 * k + 0.2),
    );
    const a = matrixProduct(matrixProduct(s, jordan, n), inverse(s, n), n);
    const eigen = realEigen(a, n);
    const values = Array.from(eigen.valuesRe).toSorted((x, y) => x - y);
    // The triple 2 is known only to about ε^⅓ of its conditioning; the others to rounding.
    expect(values.slice(3).map((v) => Number(v.toFixed(9)))).toEqual([6, 7, 8]);
    values.slice(0, 3).forEach((v) => {
      expect(Math.abs(v - 2)).toBeLessThan(1e-4);
    });
    expect(residuals(a, n, eigen)[0]).toBeLessThan(1e-8);
  });

  it("gives finite unit vectors for a repeated eigenvalue", () => {
    // Upper triangular, its eigenvalue 2 twice and a Jordan block's coupling.
    const a = new Float64Array([2, 1, 0, 0, 2, 3, 0, 0, 5]);
    const eigen = realEigen(a, 3);
    expect(eigen.vectors.every(Number.isFinite)).toBe(true);
    expect(residuals(a, 3, eigen)[1]).toBeLessThan(1e-14);
    expect(Array.from(eigen.valuesRe).toSorted((x, y) => x - y)).toEqual([2, 2, 5]);
  });

  it("refuses an entry that is not finite and a matrix of the wrong size", () => {
    expect(() => realEigen(new Float64Array([1, Number.NaN, 0, 1]), 2)).toThrow(Error);
    expect(() => realEigen(new Float64Array(3), 2)).toThrow(RangeError);
  });
});

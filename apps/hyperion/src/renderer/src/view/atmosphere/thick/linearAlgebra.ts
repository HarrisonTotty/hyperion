/**
 * Dense and banded linear algebra in `f64` for the discrete-ordinates solver (plan R08, R08.T14.a):
 * the Cholesky factorisation and its triangular solves, the cyclic Jacobi method for the symmetric
 * eigenproblem, and banded Gaussian elimination with partial pivoting.
 *
 * @remarks
 * Each is the standard method, cited to G. H. Golub and C. F. Van Loan, _Matrix Computations_,
 * 4th ed. (Johns Hopkins, 2013): triangular systems §3.1, the Cholesky factorisation §4.2, banded
 * elimination with partial pivoting §4.3 and the cyclic Jacobi method §8.5. The sections are
 * placed by the book's contents; its text was not re-read for this module (plan R08's record of
 * R08.T14.a), and each method is held by its own test. Pure: no state, no platform API. Dense
 * matrices are row-major `Float64Array`s, entry (i, j) of an n × n matrix at `i * n + j`.
 */

/**
 * The lower Cholesky factor L of a symmetric positive-definite matrix, A = L Lᵀ, row by row
 * (Golub and Van Loan 2013, §4.2). Only A's lower triangle is read.
 *
 * @param a - The n × n matrix, row-major; not modified.
 * @returns L, n × n row-major, zero above its diagonal.
 * @throws RangeError if A is not positive definite to working precision (a pivot that is not
 *   positive and finite), or if `a` does not hold n × n entries.
 */
export function choleskyLower(a: Float64Array, n: number): Float64Array {
  requireSquare(a, n);
  const l = new Float64Array(n * n);
  for (let j = 0; j < n; j += 1) {
    let diagonal = at(a, j * n + j);
    for (let k = 0; k < j; k += 1) {
      const ljk = at(l, j * n + k);
      diagonal -= ljk * ljk;
    }
    if (!(diagonal > 0 && Number.isFinite(diagonal))) {
      throw new RangeError(`the matrix is not positive definite: pivot ${j} is ${diagonal}`);
    }
    const ljj = Math.sqrt(diagonal);
    l[j * n + j] = ljj;
    for (let i = j + 1; i < n; i += 1) {
      let sum = at(a, i * n + j);
      for (let k = 0; k < j; k += 1) {
        sum -= at(l, i * n + k) * at(l, j * n + k);
      }
      l[i * n + j] = sum / ljj;
    }
  }
  return l;
}

/**
 * Solves L x = b in place, L lower triangular (forward substitution, Golub and Van Loan §3.1).
 *
 * @throws RangeError if `l` is not n × n or `b` does not hold n entries.
 */
export function solveLower(l: Float64Array, n: number, b: Float64Array): void {
  requireSquare(l, n);
  requireLength(b, n);
  for (let i = 0; i < n; i += 1) {
    let sum = at(b, i);
    for (let k = 0; k < i; k += 1) {
      sum -= at(l, i * n + k) * at(b, k);
    }
    b[i] = sum / at(l, i * n + i);
  }
}

/**
 * Solves Lᵀ x = b in place, L lower triangular (back substitution on Lᵀ, Golub and Van Loan §3.1).
 *
 * @throws RangeError if `l` is not n × n or `b` does not hold n entries.
 */
export function solveLowerTransposed(l: Float64Array, n: number, b: Float64Array): void {
  requireSquare(l, n);
  requireLength(b, n);
  for (let i = n - 1; i >= 0; i -= 1) {
    let sum = at(b, i);
    for (let k = i + 1; k < n; k += 1) {
      sum -= at(l, k * n + i) * at(b, k);
    }
    b[i] = sum / at(l, i * n + i);
  }
}

/** A symmetric matrix's eigenvalues, ascending, and its orthonormal eigenvectors. */
export interface SymmetricEigensystem {
  /** λ_j, ascending. */
  readonly values: Float64Array;
  /** n × n row-major: column j is the unit eigenvector of `values[j]`. */
  readonly vectors: Float64Array;
}

/**
 * The sweeps after which {@link symmetricEigen} gives up. Jacobi's convergence is quadratic once
 * the off-diagonal part is small, and an order-16 solve here takes 6 to 9 sweeps.
 */
const JACOBI_MAX_SWEEPS = 64;

/**
 * The eigenvalues and eigenvectors of a real symmetric matrix by the cyclic Jacobi method (Golub
 * and Van Loan 2013, §8.5: the 2 × 2 symmetric Schur decomposition and the row-by-row cyclic
 * sweep).
 *
 * @remarks
 * Each rotation annihilates one off-diagonal pair, with t = tan θ the smaller root of
 * t² + 2τt − 1 = 0, τ = (a_qq − a_pp) ÷ 2a_pq, so that |θ| ≤ π ÷ 4. A pair is taken as converged,
 * and set to 0, when |a_pq| ≤ ε √(|a_pp a_qq|), ε the `f64` epsilon: the relative criterion under
 * which Jacobi's eigenvalues of a positive-definite matrix keep their relative accuracy (J. Demmel
 * and K. Veselić, "Jacobi's method is more accurate than QR", SIAM J. Matrix Anal. Appl. 13(4)
 * (1992) 1204–1245). The sweeps stop when a whole sweep rotates nothing.
 *
 * @param a - The n × n symmetric matrix, row-major; not modified. Only its lower triangle is read.
 * @throws RangeError if `a` does not hold n × n entries.
 * @throws Error if the sweeps do not converge within {@link JACOBI_MAX_SWEEPS}, which a finite
 *   symmetric matrix does not reach.
 */
export function symmetricEigen(a: Float64Array, n: number): SymmetricEigensystem {
  requireSquare(a, n);
  const m = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j <= i; j += 1) {
      const v = at(a, i * n + j);
      m[i * n + j] = v;
      m[j * n + i] = v;
    }
  }
  const v = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    v[i * n + i] = 1;
  }
  let converged = false;
  for (let sweep = 0; sweep < JACOBI_MAX_SWEEPS && !converged; sweep += 1) {
    converged = true;
    for (let p = 0; p < n - 1; p += 1) {
      for (let q = p + 1; q < n; q += 1) {
        const apq = at(m, p * n + q);
        if (apq === 0) {
          continue;
        }
        const app = at(m, p * n + p);
        const aqq = at(m, q * n + q);
        if (Math.abs(apq) <= Number.EPSILON * Math.sqrt(Math.abs(app * aqq))) {
          m[p * n + q] = 0;
          m[q * n + p] = 0;
          continue;
        }
        converged = false;
        const tau = (aqq - app) / (2 * apq);
        // Past 1e150, 1 + τ² would overflow; t ≈ 1 ÷ 2τ there to working precision.
        const t =
          Math.abs(tau) > 1e150
            ? 1 / (2 * tau)
            : (tau >= 0 ? 1 : -1) / (Math.abs(tau) + Math.sqrt(1 + tau * tau));
        const c = 1 / Math.sqrt(1 + t * t);
        const s = t * c;
        m[p * n + p] = app - t * apq;
        m[q * n + q] = aqq + t * apq;
        m[p * n + q] = 0;
        m[q * n + p] = 0;
        for (let r = 0; r < n; r += 1) {
          if (r !== p && r !== q) {
            const arp = at(m, r * n + p);
            const arq = at(m, r * n + q);
            const newRp = c * arp - s * arq;
            const newRq = s * arp + c * arq;
            m[r * n + p] = newRp;
            m[p * n + r] = newRp;
            m[r * n + q] = newRq;
            m[q * n + r] = newRq;
          }
          const vrp = at(v, r * n + p);
          const vrq = at(v, r * n + q);
          v[r * n + p] = c * vrp - s * vrq;
          v[r * n + q] = s * vrp + c * vrq;
        }
      }
    }
  }
  if (!converged) {
    throw new Error(`Jacobi's sweeps did not converge in ${JACOBI_MAX_SWEEPS}`);
  }
  const order = Array.from({ length: n }, (_, i) => i).toSorted(
    (i, j) => at(m, i * n + i) - at(m, j * n + j),
  );
  const values = new Float64Array(n);
  const vectors = new Float64Array(n * n);
  order.forEach((source, j) => {
    values[j] = at(m, source * n + source);
    for (let i = 0; i < n; i += 1) {
      vectors[i * n + j] = at(v, i * n + source);
    }
  });
  return { values, vectors };
}

/**
 * A banded matrix stored with room for the fill-in of partial pivoting: row r holds columns
 * r − `lower` … r + `lower` + `upper`, entry (r, c) at `r * width + (c − r + lower)`.
 */
export interface BandMatrix {
  /** n, its order. */
  readonly order: number;
  /** Its lower bandwidth: a_rc = 0 for r − c > `lower`. */
  readonly lower: number;
  /** Its upper bandwidth: a_rc = 0 for c − r > `upper`. */
  readonly upper: number;
  /** 2 `lower` + `upper` + 1, the stored entries a row. */
  readonly width: number;
  readonly entries: Float64Array;
}

/**
 * A zero n × n band matrix of the given bandwidths, ready for {@link bandIndex} and
 * {@link factorBand}.
 *
 * @throws RangeError for an order below 1 or a bandwidth that is negative or not an integer.
 */
export function bandMatrix(order: number, lower: number, upper: number): BandMatrix {
  if (!Number.isInteger(order) || order < 1) {
    throw new RangeError(`a band matrix needs an order of at least 1, not ${order}`);
  }
  if (!Number.isInteger(lower) || lower < 0 || !Number.isInteger(upper) || upper < 0) {
    throw new RangeError(`a band matrix needs whole bandwidths ≥ 0, not ${lower} and ${upper}`);
  }
  const width = 2 * lower + upper + 1;
  return { order, lower, upper, width, entries: new Float64Array(order * width) };
}

/**
 * The index of entry (row, column) in a band matrix's `entries`.
 *
 * @throws RangeError for an entry outside the matrix or its band.
 */
export function bandIndex(matrix: BandMatrix, row: number, column: number): number {
  const offset = column - row + matrix.lower;
  if (
    row < 0 ||
    row >= matrix.order ||
    column < 0 ||
    column >= matrix.order ||
    column - row > matrix.upper ||
    offset < 0
  ) {
    throw new RangeError(`(${row}, ${column}) is outside the band`);
  }
  return row * matrix.width + offset;
}

/** A band matrix factorised in place by {@link factorBand}: P A = L U, with its row interchanges. */
export interface BandLu {
  /** The factors: U on and above the diagonal, the multipliers of L below it. */
  readonly factors: BandMatrix;
  /** The row each step k swapped with row k. */
  readonly pivots: Int32Array;
}

/**
 * Factorises a band matrix in place by Gaussian elimination with partial pivoting (Golub and Van
 * Loan 2013, §4.3: U's upper bandwidth grows to `lower` + `upper`, for which the storage has room).
 *
 * @remarks
 * Each step's interchange swaps only the columns from the step on, and its multipliers stay in the
 * rows where they were made, so that {@link solveBand} applies the interchanges and the eliminations
 * in turn, as they were made.
 *
 * @returns The factors, sharing `matrix`'s storage, and the pivots.
 * @throws Error if a pivot column is all zero: the matrix is singular.
 */
export function factorBand(matrix: BandMatrix): BandLu {
  const { order: n, lower: ml, upper: mu, width: w, entries: a } = matrix;
  const pivots = new Int32Array(n);
  for (let k = 0; k < n; k += 1) {
    const last = Math.min(n - 1, k + ml);
    let pivotRow = k;
    let largest = Math.abs(at(a, k * w + ml));
    for (let r = k + 1; r <= last; r += 1) {
      const candidate = Math.abs(at(a, r * w + (k - r + ml)));
      if (candidate > largest) {
        largest = candidate;
        pivotRow = r;
      }
    }
    if (!(largest > 0)) {
      throw new Error(`the band matrix is singular at column ${k}`);
    }
    pivots[k] = pivotRow;
    const columnEnd = Math.min(n - 1, k + ml + mu);
    if (pivotRow !== k) {
      for (let c = k; c <= columnEnd; c += 1) {
        const ik = k * w + (c - k + ml);
        const ip = pivotRow * w + (c - pivotRow + ml);
        const held = at(a, ik);
        a[ik] = at(a, ip);
        a[ip] = held;
      }
    }
    const pivot = at(a, k * w + ml);
    for (let r = k + 1; r <= last; r += 1) {
      const ir = r * w + (k - r + ml);
      const multiplier = at(a, ir) / pivot;
      a[ir] = multiplier;
      if (multiplier !== 0) {
        for (let c = k + 1; c <= columnEnd; c += 1) {
          const target = r * w + (c - r + ml);
          a[target] = at(a, target) - multiplier * at(a, k * w + (c - k + ml));
        }
      }
    }
  }
  return { factors: matrix, pivots };
}

/**
 * Solves A x = b in place from {@link factorBand}'s factors: the interchanges and eliminations in
 * the order they were made, then back substitution with U.
 *
 * @throws RangeError if `b`'s length is not the matrix's order.
 */
export function solveBand(lu: BandLu, b: Float64Array): void {
  const { order: n, lower: ml, upper: mu, width: w, entries: a } = lu.factors;
  if (b.length !== n) {
    throw new RangeError(`a right-hand side of ${b.length} entries for an order-${n} matrix`);
  }
  for (let k = 0; k < n; k += 1) {
    const p = at(lu.pivots, k);
    if (p !== k) {
      const held = at(b, k);
      b[k] = at(b, p);
      b[p] = held;
    }
    const bk = at(b, k);
    if (bk !== 0) {
      const last = Math.min(n - 1, k + ml);
      for (let r = k + 1; r <= last; r += 1) {
        b[r] = at(b, r) - at(a, r * w + (k - r + ml)) * bk;
      }
    }
  }
  for (let k = n - 1; k >= 0; k -= 1) {
    const columnEnd = Math.min(n - 1, k + ml + mu);
    let sum = at(b, k);
    for (let c = k + 1; c <= columnEnd; c += 1) {
      sum -= at(a, k * w + (c - k + ml)) * at(b, c);
    }
    b[k] = sum / at(a, k * w + ml);
  }
}

function requireSquare(a: Float64Array, n: number): void {
  if (!Number.isInteger(n) || n < 1 || a.length !== n * n) {
    throw new RangeError(`expected an n × n matrix with n ≥ 1, not ${a.length} entries for n ${n}`);
  }
}

function requireLength(b: Float64Array, n: number): void {
  if (b.length !== n) {
    throw new RangeError(`expected a vector of ${n} entries, not ${b.length}`);
  }
}

/** An entry that the loops' bounds keep in range; NaN, never `undefined`, should they not. */
function at(a: Float64Array | Int32Array, i: number): number {
  return a[i] ?? Number.NaN;
}

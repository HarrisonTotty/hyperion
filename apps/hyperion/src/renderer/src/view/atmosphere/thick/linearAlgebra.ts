/**
 * Dense and banded linear algebra in `f64` for the discrete-ordinates solver (plan R08, R08.T14.a
 * and T14.e): the Cholesky factorisation and its triangular solves, the cyclic Jacobi method for
 * the symmetric eigenproblem, banded Gaussian elimination with partial pivoting, and, for the
 * vector mode, dense Gaussian elimination with partial pivoting and the real nonsymmetric
 * eigenproblem.
 *
 * @remarks
 * Each is the standard method, cited to G. H. Golub and C. F. Van Loan, _Matrix Computations_,
 * 4th ed. (Johns Hopkins, 2013): triangular systems §3.1, Gaussian elimination with partial
 * pivoting §3.4, the Cholesky factorisation §4.2, banded elimination with partial pivoting §4.3,
 * the Hessenberg reduction §7.4, the Francis double-shift QR iteration §7.5 and the eigenvectors of
 * a real Schur form §7.6, and the cyclic Jacobi method §8.5. The sections are placed by the book's
 * contents; its text was not re-read for this module (plan R08's records of R08.T14.a and T14.e),
 * and each method is held by its own test. Pure: no state, no platform API. Dense matrices are
 * row-major `Float64Array`s, entry (i, j) of an n × n matrix at `i * n + j`.
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
    const p = lu.pivots[k] ?? k;
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

/** A dense matrix factorised by {@link factorDense}: P A = L U, with its row interchanges. */
export interface DenseLu {
  /** n, its order. */
  readonly order: number;
  /** U on and above the diagonal and L's multipliers below it, n × n row-major. */
  readonly factors: Float64Array;
  /** The row each step k swapped with row k. */
  readonly pivots: Int32Array;
}

/**
 * Factorises a dense matrix by Gaussian elimination with partial pivoting (Golub and Van Loan 2013,
 * §3.4), each interchange swapping whole rows, multipliers included, as LAPACK's `dgetrf` does.
 *
 * @param a - The n × n matrix, row-major; not modified.
 * @throws RangeError if `a` does not hold n × n entries.
 * @throws Error if a pivot column is all zero: the matrix is singular.
 */
export function factorDense(a: Float64Array, n: number): DenseLu {
  requireSquare(a, n);
  const f = Float64Array.from(a);
  const pivots = new Int32Array(n);
  for (let k = 0; k < n; k += 1) {
    let pivotRow = k;
    let largest = Math.abs(at(f, k * n + k));
    for (let r = k + 1; r < n; r += 1) {
      const candidate = Math.abs(at(f, r * n + k));
      if (candidate > largest) {
        largest = candidate;
        pivotRow = r;
      }
    }
    if (!(largest > 0)) {
      throw new Error(`the matrix is singular at column ${k}`);
    }
    pivots[k] = pivotRow;
    if (pivotRow !== k) {
      for (let c = 0; c < n; c += 1) {
        const held = at(f, k * n + c);
        f[k * n + c] = at(f, pivotRow * n + c);
        f[pivotRow * n + c] = held;
      }
    }
    const pivot = at(f, k * n + k);
    for (let r = k + 1; r < n; r += 1) {
      const multiplier = at(f, r * n + k) / pivot;
      f[r * n + k] = multiplier;
      if (multiplier !== 0) {
        for (let c = k + 1; c < n; c += 1) {
          f[r * n + c] = at(f, r * n + c) - multiplier * at(f, k * n + c);
        }
      }
    }
  }
  return { order: n, factors: f, pivots };
}

/**
 * Solves A x = b in place from {@link factorDense}'s factors: the interchanges, then forward
 * substitution with L's unit diagonal and back substitution with U.
 *
 * @throws RangeError if `b`'s length is not the matrix's order.
 */
export function solveDense(lu: DenseLu, b: Float64Array): void {
  const { order: n, factors: f, pivots } = lu;
  requireLength(b, n);
  for (let k = 0; k < n; k += 1) {
    const p = pivots[k] ?? k;
    if (p !== k) {
      const held = at(b, k);
      b[k] = at(b, p);
      b[p] = held;
    }
  }
  for (let i = 1; i < n; i += 1) {
    let sum = at(b, i);
    for (let k = 0; k < i; k += 1) {
      sum -= at(f, i * n + k) * at(b, k);
    }
    b[i] = sum;
  }
  for (let i = n - 1; i >= 0; i -= 1) {
    let sum = at(b, i);
    for (let k = i + 1; k < n; k += 1) {
      sum -= at(f, i * n + k) * at(b, k);
    }
    b[i] = sum / at(f, i * n + i);
  }
}

/**
 * A real matrix's eigenvalues and eigenvectors, in LAPACK's `dgeev` layout.
 *
 * @remarks
 * A complex pair λ, λ̄ stands at j and j + 1, Im λ_j > 0. Column j of `vectors` is then the real
 * part and column j + 1 the imaginary part of λ_j's eigenvector, whose complex 2-norm is 1; λ̄'s is
 * its conjugate. A real eigenvalue's column is its unit eigenvector.
 */
export interface RealEigensystem {
  /** Re λ_j, in the order the real Schur form holds them. */
  readonly valuesRe: Float64Array;
  /** Im λ_j: 0 for a real eigenvalue, + then − for a complex pair. */
  readonly valuesIm: Float64Array;
  /** n × n row-major (the remarks). */
  readonly vectors: Float64Array;
}

/**
 * The QR sweeps without a deflation after which {@link realEigen} gives up. Francis's iteration
 * converges quadratically near an eigenvalue, about two sweeps an eigenvalue; an exceptional shift
 * every tenth sweep breaks the cycles it can fall into (Golub and Van Loan 2013, §7.5.6).
 */
const QR_MAX_SWEEPS = 120;

/**
 * The sweeps of one window after which a subdiagonal entry is also taken as 0 at n ε times the
 * matrix's largest entry, not only at ε times its neighbours (`realEigen`'s remarks): a window of
 * an eigenvalue repeated to rounding stalls at the noise the matrix's larger entries left there. n
 * ε ‖H‖ is the iteration's own backward error, so the relaxed test costs no accuracy it has.
 */
const STAGNANT_SWEEPS = 10;

/**
 * The sweeps of one window after which a cluster that no shift separates is split by force
 * (`splitCluster`): a nearly defective cluster, whose eigenvalues are known only to about the cube
 * root of the rounding, converges no further.
 */
const CLUSTER_SWEEPS = 30;

/**
 * The eigenvalues and eigenvectors of a real, generally nonsymmetric matrix (Golub and Van Loan
 * 2013): Householder reduction to upper Hessenberg form (§7.4), the Francis double-shift QR
 * iteration to the real Schur form T = Zᵀ A Z with its transformations accumulated (§7.5), each
 * 2 × 2 diagonal block with real eigenvalues split by a rotation, and the eigenvectors of T by back
 * substitution, mapped back by Z (§7.6).
 *
 * @remarks
 * An entry below the diagonal is taken as 0 where it is at most ε (|t_ii| + |t_jj|) of its
 * neighbours, ε the `f64` epsilon, or, once a window has gone {@link STAGNANT_SWEEPS} sweeps
 * without one, at most n ε times the matrix's largest entry: an eigenvalue repeated to rounding (as
 * the vector mode's high modes repeat 1 ÷ μ_i² over the Stokes components) leaves an unreduced
 * window of noise at that level, which no shift reduces further. A window still stalled after
 * {@link CLUSTER_SWEEPS} sweeps is a nearly defective cluster (as the four Stokes components of one
 * node can be at a high mode, their 1 ÷ μ² shared and their coupling small and not normal): its
 * smallest entry below the diagonal is set to 0 where it is at most √ε times the window's largest,
 * a perturbation within the cluster's own conditioning, its eigenvalues then known to about that.
 * In the back substitution a divisor
 * T_ii − λ smaller than ε ‖T‖ is
 * taken as ε ‖T‖ (‖T‖ the largest |t_ij|), the perturbation LAPACK's `dtrevc` makes, so that a
 * repeated eigenvalue gives a finite vector. Balancing is not done; the vector mode's matrices
 * (`polarisation.ts`) are scaled symmetrically beforehand.
 *
 * @param a - The n × n matrix, row-major; not modified.
 * @throws RangeError if `a` does not hold n × n entries.
 * @throws Error if the iteration does not converge within {@link QR_MAX_SWEEPS} sweeps of one
 *   window, or if an entry is not finite.
 */
export function realEigen(a: Float64Array, n: number): RealEigensystem {
  requireSquare(a, n);
  if (!a.every(Number.isFinite)) {
    throw new Error("the matrix has an entry that is not finite");
  }
  const h = Float64Array.from(a);
  const z = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    z[i * n + i] = 1;
  }
  hessenbergReduce(h, z, n);
  realSchur(h, z, n);
  return schurEigenvectors(h, z, n);
}

/** Reduces h to upper Hessenberg form in place by Householder reflections, accumulated into z. */
function hessenbergReduce(h: Float64Array, z: Float64Array, n: number): void {
  const v = new Float64Array(n);
  for (let k = 0; k + 2 < n; k += 1) {
    let squares = 0;
    for (let i = k + 1; i < n; i += 1) {
      squares += at(h, i * n + k) ** 2;
    }
    if (squares === 0) {
      continue;
    }
    const alpha = Math.sqrt(squares);
    const x0 = at(h, (k + 1) * n + k);
    const sign = x0 >= 0 ? 1 : -1;
    let norm2 = 0;
    for (let i = k + 1; i < n; i += 1) {
      v[i] = i === k + 1 ? x0 + sign * alpha : at(h, i * n + k);
      norm2 += at(v, i) ** 2;
    }
    const beta = 2 / norm2;
    for (let j = k; j < n; j += 1) {
      let s = 0;
      for (let i = k + 1; i < n; i += 1) {
        s += at(v, i) * at(h, i * n + j);
      }
      s *= beta;
      for (let i = k + 1; i < n; i += 1) {
        h[i * n + j] = at(h, i * n + j) - s * at(v, i);
      }
    }
    for (const m of [h, z]) {
      for (let i = 0; i < n; i += 1) {
        let s = 0;
        for (let j = k + 1; j < n; j += 1) {
          s += at(m, i * n + j) * at(v, j);
        }
        s *= beta;
        for (let j = k + 1; j < n; j += 1) {
          m[i * n + j] = at(m, i * n + j) - s * at(v, j);
        }
      }
    }
    h[(k + 1) * n + k] = -sign * alpha;
    for (let i = k + 2; i < n; i += 1) {
      h[i * n + k] = 0;
    }
  }
}

/**
 * Reduces an upper Hessenberg h to real Schur form in place by Francis double-shift QR sweeps on
 * its unreduced windows (Golub and Van Loan 2013, Algorithms 7.5.1 and 7.5.2), every transformation
 * applied to the whole matrix and accumulated into z, so that h becomes T and z the Schur vectors.
 */
function realSchur(h: Float64Array, z: Float64Array, n: number): void {
  let scale = 0;
  for (const value of h) {
    scale = Math.max(scale, Math.abs(value));
  }
  let hi = n - 1;
  let sweeps = 0;
  while (hi >= 0) {
    let low = hi;
    while (low > 0) {
      let neighbours = Math.abs(at(h, (low - 1) * n + low - 1)) + Math.abs(at(h, low * n + low));
      if (neighbours === 0) {
        neighbours = scale;
      }
      if (sweeps >= STAGNANT_SWEEPS) {
        neighbours = Math.max(neighbours, n * scale);
      }
      if (Math.abs(at(h, low * n + low - 1)) <= Number.EPSILON * neighbours) {
        h[low * n + low - 1] = 0;
        break;
      }
      low -= 1;
    }
    if (low === hi) {
      hi -= 1;
      sweeps = 0;
      continue;
    }
    if (low === hi - 1) {
      splitRealBlock(h, z, n, low);
      hi -= 2;
      sweeps = 0;
      continue;
    }
    sweeps += 1;
    if (sweeps > QR_MAX_SWEEPS) {
      throw new Error(`the QR iteration did not converge in ${QR_MAX_SWEEPS} sweeps`);
    }
    if (sweeps > CLUSTER_SWEEPS && splitCluster(h, n, low, hi)) {
      sweeps = 0;
      continue;
    }
    const p = hi - 1;
    let sum: number;
    let product: number;
    if (sweeps % 10 === 0) {
      // An exceptional shift: complex roots about the corner, off any cycle of the standard ones.
      const w = Math.abs(at(h, hi * n + p)) + Math.abs(at(h, p * n + p - 1));
      const d = at(h, hi * n + hi);
      sum = 2 * d + 1.5 * w;
      product = d * d + 1.5 * w * d + w * w;
    } else {
      sum = at(h, p * n + p) + at(h, hi * n + hi);
      product = at(h, p * n + p) * at(h, hi * n + hi) - at(h, p * n + hi) * at(h, hi * n + p);
    }
    francisSweep(h, z, n, low, hi, sum, product);
  }
}

/**
 * Splits a window that has stalled on a nearly defective cluster (`realEigen`'s remarks): its
 * smallest entry below the diagonal is set to 0 where it is at most √ε times the window's largest
 * entry.
 *
 * @returns Whether it split the window.
 */
function splitCluster(h: Float64Array, n: number, low: number, hi: number): boolean {
  let largest = 0;
  for (let i = low; i <= hi; i += 1) {
    for (let j = Math.max(low, i - 1); j <= hi; j += 1) {
      largest = Math.max(largest, Math.abs(at(h, i * n + j)));
    }
  }
  let smallest = Number.POSITIVE_INFINITY;
  let where = -1;
  for (let k = low + 1; k <= hi; k += 1) {
    const value = Math.abs(at(h, k * n + k - 1));
    if (value < smallest) {
      smallest = value;
      where = k;
    }
  }
  if (where < 0 || !(smallest <= Math.sqrt(Number.EPSILON) * largest)) {
    return false;
  }
  h[where * n + where - 1] = 0;
  return true;
}

/**
 * One implicit double-shift sweep on window [low, hi] for the shifts that are the roots of
 * x² − `sum` x + `product` (Golub and Van Loan 2013, Algorithm 7.5.1): the bulge of the first
 * column of (H − σ₁)(H − σ₂) chased down by 3-element reflections and a closing rotation.
 */
function francisSweep(
  h: Float64Array,
  z: Float64Array,
  n: number,
  low: number,
  hi: number,
  sum: number,
  product: number,
): void {
  const h00 = at(h, low * n + low);
  const h10 = at(h, (low + 1) * n + low);
  let x = h00 * h00 + at(h, low * n + low + 1) * h10 - sum * h00 + product;
  let y = h10 * (h00 + at(h, (low + 1) * n + low + 1) - sum);
  let w = h10 * at(h, (low + 2) * n + low + 1);
  for (let k = low; k + 2 <= hi; k += 1) {
    const r = Math.hypot(x, y, w);
    if (r > 0) {
      const sign = x >= 0 ? 1 : -1;
      const v0 = x + sign * r;
      const beta = 2 / (v0 * v0 + y * y + w * w);
      const first = Math.max(low, k - 1);
      for (let j = first; j < n; j += 1) {
        const s =
          beta * (v0 * at(h, k * n + j) + y * at(h, (k + 1) * n + j) + w * at(h, (k + 2) * n + j));
        h[k * n + j] = at(h, k * n + j) - s * v0;
        h[(k + 1) * n + j] = at(h, (k + 1) * n + j) - s * y;
        h[(k + 2) * n + j] = at(h, (k + 2) * n + j) - s * w;
      }
      const last = Math.min(k + 3, hi);
      for (const [m, rows] of [
        [h, last + 1],
        [z, n],
      ] as const) {
        for (let i = 0; i < rows; i += 1) {
          const s =
            beta * (at(m, i * n + k) * v0 + at(m, i * n + k + 1) * y + at(m, i * n + k + 2) * w);
          m[i * n + k] = at(m, i * n + k) - s * v0;
          m[i * n + k + 1] = at(m, i * n + k + 1) - s * y;
          m[i * n + k + 2] = at(m, i * n + k + 2) - s * w;
        }
      }
      if (k > low) {
        h[k * n + k - 1] = -sign * r;
        h[(k + 1) * n + k - 1] = 0;
        h[(k + 2) * n + k - 1] = 0;
      }
    }
    x = at(h, (k + 1) * n + k);
    y = at(h, (k + 2) * n + k);
    if (k + 3 <= hi) {
      w = at(h, (k + 3) * n + k);
    }
  }
  const r = Math.hypot(x, y);
  if (r > 0) {
    rotate(h, z, n, hi - 1, x / r, y / r, hi - 2, hi + 1);
    h[hi * n + hi - 2] = 0;
  }
}

/**
 * Applies the rotation G = [[c, s], [−s, c]] to rows p, p + 1 of h from column `fromColumn` on, Gᵀ
 * to its columns p, p + 1 in rows 0 … `toRow` − 1, and Gᵀ to z's columns p, p + 1.
 */
function rotate(
  h: Float64Array,
  z: Float64Array,
  n: number,
  p: number,
  c: number,
  s: number,
  fromColumn: number,
  toRow: number,
): void {
  for (let j = Math.max(fromColumn, 0); j < n; j += 1) {
    const a1 = at(h, p * n + j);
    const a2 = at(h, (p + 1) * n + j);
    h[p * n + j] = c * a1 + s * a2;
    h[(p + 1) * n + j] = -s * a1 + c * a2;
  }
  for (const [m, rows] of [
    [h, toRow],
    [z, n],
  ] as const) {
    for (let i = 0; i < rows; i += 1) {
      const a1 = at(m, i * n + p);
      const a2 = at(m, i * n + p + 1);
      m[i * n + p] = c * a1 + s * a2;
      m[i * n + p + 1] = -s * a1 + c * a2;
    }
  }
}

/**
 * Splits the 2 × 2 diagonal block at p, p + 1 into a triangular one where its eigenvalues are real,
 * by the rotation whose first column is the eigenvector (λ − d, c) of its eigenvalue λ farther from
 * d; a block of a complex pair stays as it is.
 */
function splitRealBlock(h: Float64Array, z: Float64Array, n: number, p: number): void {
  const a = at(h, p * n + p);
  const b = at(h, p * n + p + 1);
  const c = at(h, (p + 1) * n + p);
  const d = at(h, (p + 1) * n + p + 1);
  if (c === 0) {
    return;
  }
  const half = (a - d) / 2;
  const discriminant = half * half + b * c;
  if (discriminant < 0) {
    return;
  }
  const root = Math.sqrt(discriminant);
  const shifted = half + (half >= 0 ? root : -root);
  const length = Math.hypot(shifted, c);
  rotate(h, z, n, p, shifted / length, c / length, p, p + 2);
  h[(p + 1) * n + p] = 0;
}

/** The eigenvalues and eigenvectors of a real Schur form t = zᵀ A z (Golub and Van Loan §7.6). */
function schurEigenvectors(t: Float64Array, z: Float64Array, n: number): RealEigensystem {
  const valuesRe = new Float64Array(n);
  const valuesIm = new Float64Array(n);
  for (let k = 0; k < n;) {
    if (k + 1 < n && at(t, (k + 1) * n + k) !== 0) {
      const a = at(t, k * n + k);
      const b = at(t, k * n + k + 1);
      const c = at(t, (k + 1) * n + k);
      const d = at(t, (k + 1) * n + k + 1);
      const half = (a - d) / 2;
      const imaginary = Math.sqrt(Math.max(-(half * half + b * c), 0));
      valuesRe[k] = (a + d) / 2;
      valuesRe[k + 1] = (a + d) / 2;
      valuesIm[k] = imaginary;
      valuesIm[k + 1] = -imaginary;
      k += 2;
    } else {
      valuesRe[k] = at(t, k * n + k);
      k += 1;
    }
  }
  let largest = 0;
  for (const value of t) {
    largest = Math.max(largest, Math.abs(value));
  }
  const small = Math.max(largest, Number.MIN_VALUE) * Number.EPSILON;
  const vectors = new Float64Array(n * n);
  const re = new Float64Array(n);
  const im = new Float64Array(n);
  for (let k = n - 1; k >= 0; k -= 1) {
    const li = at(valuesIm, k);
    if (li < 0) {
      continue;
    }
    const lr = at(valuesRe, k);
    re.fill(0);
    im.fill(0);
    const top = li > 0 ? k + 1 : k;
    if (li > 0) {
      const a = at(t, k * n + k);
      const b = at(t, k * n + k + 1);
      const c = at(t, (k + 1) * n + k);
      const d = at(t, (k + 1) * n + k + 1);
      // An eigenvector of the block: (b, λ − a) solves its first row, (λ − d, c) its second.
      if (Math.abs(b) >= Math.abs(c)) {
        re[k] = b;
        re[k + 1] = lr - a;
        im[k + 1] = li;
      } else {
        re[k] = lr - d;
        im[k] = li;
        re[k + 1] = c;
      }
    } else {
      re[k] = 1;
    }
    let i = k - 1;
    while (i >= 0) {
      const pair = i > 0 && at(t, i * n + i - 1) !== 0;
      const rows = pair ? [i - 1, i] : [i];
      const rhs = rows.map((row) => {
        let sr = 0;
        let si = 0;
        for (let j = i + 1; j <= top; j += 1) {
          const tij = at(t, row * n + j);
          sr -= tij * at(re, j);
          si -= tij * at(im, j);
        }
        return [sr, si] as const;
      });
      if (pair) {
        const [r1, r2] = rhs;
        if (r1 === undefined || r2 === undefined) {
          throw new Error("a 2 × 2 block has two rows");
        }
        // (T_block − λ) v = r, complex, by Cramer's rule.
        const a11: Complex = [at(t, (i - 1) * n + i - 1) - lr, -li];
        const a12 = at(t, (i - 1) * n + i);
        const a21 = at(t, i * n + i - 1);
        const a22: Complex = [at(t, i * n + i) - lr, -li];
        let det = sub(mul(a11, a22), [a12 * a21, 0]);
        if (Math.hypot(det[0], det[1]) < small) {
          det = [small, 0];
        }
        const first = div(sub(mul(r1, a22), scaled(r2, a12)), det);
        const second = div(sub(mul(a11, r2), scaled(r1, a21)), det);
        [re[i - 1], im[i - 1]] = first;
        [re[i], im[i]] = second;
        i -= 2;
      } else {
        const [r] = rhs;
        if (r === undefined) {
          throw new Error("a 1 × 1 block has a row");
        }
        let divisor: Complex = [at(t, i * n + i) - lr, -li];
        if (Math.hypot(divisor[0], divisor[1]) < small) {
          divisor = [small, 0];
        }
        [re[i], im[i]] = div(r, divisor);
        i -= 1;
      }
    }
    let norm2 = 0;
    const xr = new Float64Array(n);
    const xi = new Float64Array(n);
    for (let r = 0; r < n; r += 1) {
      let sr = 0;
      let si = 0;
      for (let j = 0; j <= top; j += 1) {
        const zrj = at(z, r * n + j);
        sr += zrj * at(re, j);
        si += zrj * at(im, j);
      }
      xr[r] = sr;
      xi[r] = si;
      norm2 += sr * sr + si * si;
    }
    const inverse = 1 / Math.sqrt(norm2);
    for (let r = 0; r < n; r += 1) {
      vectors[r * n + k] = at(xr, r) * inverse;
      if (li > 0) {
        vectors[r * n + k + 1] = at(xi, r) * inverse;
      }
    }
  }
  return { valuesRe, valuesIm, vectors };
}

/** A complex number as its real and imaginary parts. */
type Complex = readonly [number, number];

function mul(x: Complex, y: Complex): Complex {
  return [x[0] * y[0] - x[1] * y[1], x[0] * y[1] + x[1] * y[0]];
}

function sub(x: Complex, y: Complex): Complex {
  return [x[0] - y[0], x[1] - y[1]];
}

function scaled(x: Complex, s: number): Complex {
  return [x[0] * s, x[1] * s];
}

/** x ÷ y by Smith's algorithm, which keeps the intermediate products in range. */
function div(x: Complex, y: Complex): Complex {
  const [a, b] = x;
  const [c, d] = y;
  if (Math.abs(c) >= Math.abs(d)) {
    const ratio = d / c;
    const denominator = c + d * ratio;
    return [(a + b * ratio) / denominator, (b - a * ratio) / denominator];
  }
  const ratio = c / d;
  const denominator = c * ratio + d;
  return [(a * ratio + b) / denominator, (b * ratio - a) / denominator];
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

/**
 * An entry that the loops' bounds keep in range; NaN, never `undefined`, should they not.
 * Float64Array alone, so that the hot loops' calls stay monomorphic and inlined (a pivot is read in
 * place).
 */
function at(a: Float64Array, i: number): number {
  return a[i] ?? Number.NaN;
}

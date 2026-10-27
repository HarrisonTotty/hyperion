/**
 * Exponential sums fitted to a bake bin's curve of growth (plan R08, R08.T14.b; Design note 5,
 * "Spectral bakes"): T_b(u, p̄) ≈ Σᵢ wᵢ e^(−kᵢ(p̄) u), so that a bake that solves by layers runs one
 * solve a term (correlated-k; A. A. Lacis and V. Oinas, "A description of the correlated k
 * distribution method for modeling nongray gaseous absorption, thermal emission, and multiple
 * scattering in vertically inhomogeneous atmospheres", JGR 96 (1991) 9027–9063,
 * doi:10.1029/90JD01945).
 *
 * @remarks
 * - **The fit at one row** (exponential-sum fitting of transmissions: W. J. Wiscombe and J. W.
 *   Evans, "Exponential-sum fitting of radiative transmission functions", J. Comput. Phys. 24
 *   (1977) 416–444, doi:10.1016/0021-9991(77)90031-6, cited for the method, not read): a
 *   non-negative least-squares fit of the weights at fixed k on a grid of
 *   {@link EXP_SUM_GRID_PER_DECADE} a decade over {@link EXP_SUM_GRID_DECADES} of kU about the
 *   curve's vertical amount U, with k = 0 among them (a window), by Lawson and Hanson's active-set
 *   method ({@link nonNegativeLeastSquares}), the weights held to sum to 1 (T(0) = 1); then the
 *   terms dropped one at a time, the one whose loss raises the largest error least, each drop
 *   refitted, while the largest error |T − Σ w e^(−ku)| over the curve's nodes stays within the
 *   tolerance or the terms exceed their cap; then the k freed from the grid and fitted with the
 *   weights by turns (Levenberg–Marquardt in ln k, then the weights again), kept where it lowers
 *   the largest error. A curve within the tolerance of 1 at every node is one term of k = 0.
 * - **Pressure rows** (the brief's lean): the weights are fitted at the reference row, the row
 *   nearest the column's own absorber-weighted pressure, and shared by every row, whose k are
 *   fitted alone (Levenberg–Marquardt in ln k, each row started from its neighbour's): the g
 *   intervals are the same at every pressure, as correlated-k assumes. A layer reads k at its own
 *   Curtis–Godson pressure ({@link expSumCoefficientsAt}).
 * - The error recorded is the largest |T − Σ w e^(−ku)| over every node and row: the curve's own
 *   columns, 10⁻³ U to 10³ U (`absorbers.ts`'s `ABSORBER_CURVE_DECADES`).
 *
 * Pure: no state, no clock, no platform API.
 */

/** The grid's k a decade of kU, for the non-negative fit. */
export const EXP_SUM_GRID_PER_DECADE = 8;

/**
 * The span of the fit's grid in kU, decades: 10^−4.5 to 10^5, past which a term is transparent or
 * opaque at every node of the curve (its nodes span 10⁻³ U to 10³ U), with k = 0 beside them.
 */
export const EXP_SUM_GRID_DECADES: readonly [number, number] = [-4.5, 5];

/** The largest |ΔT| over a curve's nodes a fit is cut back to (the module's remarks). */
export const EXP_SUM_TOLERANCE = 2e-3;

/** The most terms an absorber's fit keeps in one bin, whatever its error. */
export const EXP_SUM_MAX_TERMS = 8;

/** A curve's exponential sum: its weights, and its k at each pressure row. */
export interface ExpSumFit {
  /** wᵢ, above 0, summing to 1, ascending in k at the reference row. */
  readonly weights: Float64Array;
  /** ln(p ÷ 1 Pa) of each row, ascending: the curve's. */
  readonly logPressures: Float64Array;
  /** ln(p ÷ 1 Pa) above which the read holds p̄: the curve's. */
  readonly readTopLogPressure: number;
  /**
   * kᵢ at each row, per unit of the curve's amount (m² a molecule, or m⁵ a pair), row-major
   * `[r × terms + i]`, at least 0.
   */
  readonly coefficients: Float64Array;
  /** The row the weights were fitted at. */
  readonly referenceRow: number;
  /** The largest |T − Σ w e^(−ku)| over the curve's nodes and rows. */
  readonly largestError: number;
  /** The largest |T − Σ w e^(−ku)| over each row's nodes. */
  readonly rowErrors: Float64Array;
  /**
   * The rows whose k are not ascending in the terms' order: correlated-k takes each term as the
   * same g interval at every pressure, and a crossing marks two whose order a row's fit swapped.
   */
  readonly crossedRows: number;
}

/** What a curve of growth gives its fit: `absorbers.ts`'s `AbsorberBinCurves` for one bin. */
export interface CurveRows {
  /** ln(u ÷ unit) at each node, ascending. */
  readonly logColumns: Float64Array;
  /** ln(p ÷ 1 Pa) at each row, ascending. */
  readonly logPressures: Float64Array;
  readonly readTopLogPressure: number;
  /** T(u, p̄) at each row and node, row-major `[r × nodes + j]`. */
  readonly transmittance: Float64Array;
}

/** How a fit is cut back. */
export interface ExpSumOptions {
  /** The largest |ΔT| allowed before the terms' cap: {@link EXP_SUM_TOLERANCE}. */
  readonly tolerance: number;
  /** The cap on the terms: {@link EXP_SUM_MAX_TERMS}. */
  readonly maxTerms: number;
}

/** The default cut. */
export const EXP_SUM_OPTIONS: ExpSumOptions = {
  tolerance: EXP_SUM_TOLERANCE,
  maxTerms: EXP_SUM_MAX_TERMS,
};

// ---------------------------------------------------------------------------------------------
// Non-negative least squares.

/**
 * The least-squares solution of A x ≈ b over the columns `columns` alone, by Householder QR (G. H.
 * Golub and C. F. Van Loan, _Matrix Computations_, 4th ed., §5.3, cited, not re-read): x at each
 * listed column, in their order.
 */
function leastSquaresOn(
  a: Float64Array,
  rows: number,
  stride: number,
  columns: ReadonlyArray<number>,
  b: Float64Array,
): Float64Array {
  const n = columns.length;
  const q = new Float64Array(rows * n);
  for (let i = 0; i < rows; i += 1) {
    for (const [c, column] of columns.entries()) {
      q[i * n + c] = at(a, i * stride + column);
    }
  }
  const y = Float64Array.from(b);
  for (let c = 0; c < n; c += 1) {
    let norm = 0;
    for (let i = c; i < rows; i += 1) {
      norm = Math.hypot(norm, at(q, i * n + c));
    }
    if (norm === 0) {
      continue;
    }
    const head = at(q, c * n + c);
    const alpha = head > 0 ? -norm : norm;
    // v = x − α e₁, stored in place below the diagonal; v₀ = head − α.
    const v0 = head - alpha;
    q[c * n + c] = v0;
    let vv = 0;
    for (let i = c; i < rows; i += 1) {
      vv += at(q, i * n + c) ** 2;
    }
    for (let k = c + 1; k < n; k += 1) {
      let dot = 0;
      for (let i = c; i < rows; i += 1) {
        dot += at(q, i * n + c) * at(q, i * n + k);
      }
      const factor = (2 * dot) / vv;
      for (let i = c; i < rows; i += 1) {
        q[i * n + k] = at(q, i * n + k) - factor * at(q, i * n + c);
      }
    }
    let dot = 0;
    for (let i = c; i < rows; i += 1) {
      dot += at(q, i * n + c) * at(y, i);
    }
    const factor = (2 * dot) / vv;
    for (let i = c; i < rows; i += 1) {
      y[i] = at(y, i) - factor * at(q, i * n + c);
    }
    q[c * n + c] = alpha;
  }
  const x = new Float64Array(n);
  for (let c = n - 1; c >= 0; c -= 1) {
    let sum = at(y, c);
    for (let k = c + 1; k < n; k += 1) {
      sum -= at(q, c * n + k) * at(x, k);
    }
    const diagonal = at(q, c * n + c);
    x[c] = diagonal === 0 ? 0 : sum / diagonal;
  }
  return x;
}

/**
 * The non-negative least-squares solution of A x ≈ b, x ≥ 0: C. L. Lawson and R. J. Hanson,
 * _Solving Least Squares Problems_ (Prentice-Hall, 1974; SIAM, 1995), ch. 23, algorithm NNLS, its
 * least-squares steps by Householder QR on the passive set.
 *
 * @param a - A, row-major, `rows` × `columns`.
 * @returns x, `columns` entries, each at least 0.
 */
export function nonNegativeLeastSquares(
  a: Float64Array,
  rows: number,
  columns: number,
  b: Float64Array,
): Float64Array {
  const x = new Float64Array(columns);
  const passive = new Uint8Array(columns);
  const residual = Float64Array.from(b);
  let scale = 0;
  for (const value of a) {
    scale = Math.max(scale, Math.abs(value));
  }
  let bNorm = 0;
  for (const value of b) {
    bNorm = Math.hypot(bNorm, value);
  }
  const tolerance = 1e-12 * scale * Math.max(bNorm, 1) * rows;
  const excluded = new Uint8Array(columns);
  for (let iteration = 0; iteration < 3 * columns + 10; iteration += 1) {
    let best = -1;
    let bestGradient = tolerance;
    for (let j = 0; j < columns; j += 1) {
      if (passive[j] === 1 || excluded[j] === 1) {
        continue;
      }
      let gradient = 0;
      for (let i = 0; i < rows; i += 1) {
        gradient += at(a, i * columns + j) * at(residual, i);
      }
      if (gradient > bestGradient) {
        bestGradient = gradient;
        best = j;
      }
    }
    if (best < 0) {
      break;
    }
    passive[best] = 1;
    for (let inner = 0; inner < 3 * columns + 10; inner += 1) {
      const set: number[] = [];
      for (let j = 0; j < columns; j += 1) {
        if (passive[j] === 1) {
          set.push(j);
        }
      }
      const z = leastSquaresOn(a, rows, columns, set, b);
      if (set.every((_, c) => at(z, c) > 0)) {
        for (const [c, j] of set.entries()) {
          x[j] = at(z, c);
        }
        break;
      }
      // The column just freed came back non-positive: rounding on a near-dependent set. It is
      // left out for the rest of the fit rather than cycled.
      if (inner === 0 && at(z, set.indexOf(best)) <= 0) {
        passive[best] = 0;
        excluded[best] = 1;
        break;
      }
      let alpha = Infinity;
      for (const [c, j] of set.entries()) {
        const zj = at(z, c);
        if (zj <= 0) {
          const xj = at(x, j);
          alpha = Math.min(alpha, xj / (xj - zj));
        }
      }
      for (const [c, j] of set.entries()) {
        x[j] = at(x, j) + alpha * (at(z, c) - at(x, j));
        if (at(x, j) <= 1e-15) {
          x[j] = 0;
          passive[j] = 0;
        }
      }
    }
    for (let i = 0; i < rows; i += 1) {
      let sum = at(b, i);
      for (let j = 0; j < columns; j += 1) {
        sum -= at(a, i * columns + j) * at(x, j);
      }
      residual[i] = sum;
    }
  }
  return x;
}

// ---------------------------------------------------------------------------------------------
// One row.

/** A sum's terms on the normalised amount x = u ÷ U: weights and kU. */
interface Terms {
  readonly weights: Float64Array;
  /** κᵢ = kᵢ U. */
  readonly rates: Float64Array;
}

/** The weight that holds the weights' sum to 1 in the non-negative fit, against unit residuals. */
const SUM_ROW_WEIGHT = 1e3;

/** The least and largest κ = kU a freed term takes: transparent and opaque at every node. */
const RATE_BOUNDS: readonly [number, number] = [1e-9, 1e6];

/** The largest |T − Σ w e^(−κx)| over the nodes. */
function largestError(x: Float64Array, t: Float64Array, terms: Terms): number {
  let worst = 0;
  for (const [j, xj] of x.entries()) {
    worst = Math.max(worst, Math.abs(at(t, j) - sumAt(terms, xj)));
  }
  return worst;
}

function sumAt(terms: Terms, x: number): number {
  let sum = 0;
  for (const [i, w] of terms.weights.entries()) {
    sum += w * Math.exp(-at(terms.rates, i) * x);
  }
  return sum;
}

/** The non-negative weights on fixed rates, their sum held to 1, renormalised, zero ones dropped. */
function weightsOn(x: Float64Array, t: Float64Array, rates: ReadonlyArray<number>): Terms {
  const rows = x.length + 1;
  const columns = rates.length;
  const a = new Float64Array(rows * columns);
  const b = new Float64Array(rows);
  for (const [j, xj] of x.entries()) {
    for (const [c, rate] of rates.entries()) {
      a[j * columns + c] = Math.exp(-rate * xj);
    }
    b[j] = at(t, j);
  }
  for (let c = 0; c < columns; c += 1) {
    a[x.length * columns + c] = SUM_ROW_WEIGHT;
  }
  b[x.length] = SUM_ROW_WEIGHT;
  const w = nonNegativeLeastSquares(a, rows, columns, b);
  const kept: number[] = [];
  const keptRates: number[] = [];
  let total = 0;
  for (const [c, wc] of w.entries()) {
    if (wc > 0) {
      kept.push(wc);
      keptRates.push(rates[c] ?? Number.NaN);
      total += wc;
    }
  }
  return {
    weights: Float64Array.from(kept, (wc) => wc / total),
    rates: Float64Array.from(keptRates),
  };
}

/** The fit's grid of κ: 0, then {@link EXP_SUM_GRID_PER_DECADE} a decade over the span. */
function rateGrid(): ReadonlyArray<number> {
  const [first, last] = EXP_SUM_GRID_DECADES;
  const count = Math.round((last - first) * EXP_SUM_GRID_PER_DECADE) + 1;
  return [
    0,
    ...Array.from({ length: count }, (_, m) => 10 ** (first + m / EXP_SUM_GRID_PER_DECADE)),
  ];
}

/**
 * The solution of A x = b for a symmetric A, by Cholesky's factorisation (Golub and Van Loan §4.2,
 * as `linearAlgebra.ts`'s `choleskyLower`), or `undefined` where A is not positive definite: a
 * damped step that is not yet a descent, which the fit meets as an outcome and answers with more
 * damping, not as an error.
 *
 * @param a - A, n × n row-major, its lower triangle read.
 */
function definiteSolve(a: Float64Array, n: number, b: Float64Array): Float64Array | undefined {
  const l = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j <= i; j += 1) {
      let sum = at(a, i * n + j);
      for (let k = 0; k < j; k += 1) {
        sum -= at(l, i * n + k) * at(l, j * n + k);
      }
      if (i === j) {
        if (!(sum > 0)) {
          return undefined;
        }
        l[i * n + i] = Math.sqrt(sum);
      } else {
        l[i * n + j] = sum / at(l, j * n + j);
      }
    }
  }
  const x = Float64Array.from(b);
  for (let i = 0; i < n; i += 1) {
    let sum = at(x, i);
    for (let k = 0; k < i; k += 1) {
      sum -= at(l, i * n + k) * at(x, k);
    }
    x[i] = sum / at(l, i * n + i);
  }
  for (let i = n - 1; i >= 0; i -= 1) {
    let sum = at(x, i);
    for (let k = i + 1; k < n; k += 1) {
      sum -= at(l, k * n + i) * at(x, k);
    }
    x[i] = sum / at(l, i * n + i);
  }
  return x;
}

/** The weight the joint fit holds the weights' sum to 1 by, against its nodes' residuals. */
const JOINT_SUM_WEIGHT = 30;

/**
 * The terms that best fit a row in least squares, by Levenberg–Marquardt (K. Levenberg, Q. Appl.
 * Math. 2 (1944) 164; D. W. Marquardt, J. SIAM 11 (1963) 431; cited, not re-read) in ln κ, and in
 * the weights too where `freeWeights`, their sum held to 1 by a residual of its own, each weight
 * held at or above 0 and each κ in {@link RATE_BOUNDS}.
 */
function leastSquaresTerms(
  x: Float64Array,
  t: Float64Array,
  start: Terms,
  freeWeights: boolean,
): Terms {
  const n = start.weights.length;
  const size = freeWeights ? 2 * n : n;
  const bounded = (r: number): number => Math.min(Math.max(r, RATE_BOUNDS[0]), RATE_BOUNDS[1]);
  // The parameters: the weights where free, then ln κ.
  let theta = new Float64Array(size);
  for (let i = 0; i < n; i += 1) {
    if (freeWeights) {
      theta[i] = at(start.weights, i);
    }
    theta[size - n + i] = Math.log(bounded(at(start.rates, i)));
  }
  const weightOf = (values: Float64Array, i: number): number =>
    freeWeights ? at(values, i) : at(start.weights, i);
  const squares = (values: Float64Array): number => {
    let sum = 0;
    for (const [j, xj] of x.entries()) {
      let model = 0;
      for (let i = 0; i < n; i += 1) {
        model += weightOf(values, i) * Math.exp(-Math.exp(at(values, size - n + i)) * xj);
      }
      sum += (model - at(t, j)) ** 2;
    }
    if (freeWeights) {
      let total = -1;
      for (let i = 0; i < n; i += 1) {
        total += at(values, i);
      }
      sum += (JOINT_SUM_WEIGHT * total) ** 2;
    }
    return sum;
  };
  let current = squares(theta);
  let damping = 1e-3;
  const h = new Float64Array(size * size);
  const g = new Float64Array(size);
  const row = new Float64Array(size);
  const accumulate = (r: number): void => {
    for (let a = 0; a < size; a += 1) {
      g[a] = at(g, a) + at(row, a) * r;
      for (let b = 0; b <= a; b += 1) {
        h[a * size + b] = at(h, a * size + b) + at(row, a) * at(row, b);
      }
    }
  };
  for (let iteration = 0; iteration < 100 && current > 0; iteration += 1) {
    h.fill(0);
    g.fill(0);
    for (const [j, xj] of x.entries()) {
      let model = 0;
      for (let i = 0; i < n; i += 1) {
        const rate = Math.exp(at(theta, size - n + i));
        const e = Math.exp(-rate * xj);
        const w = weightOf(theta, i);
        model += w * e;
        if (freeWeights) {
          row[i] = e;
        }
        row[size - n + i] = -w * rate * xj * e;
      }
      accumulate(model - at(t, j));
    }
    if (freeWeights) {
      let total = -1;
      row.fill(0);
      for (let i = 0; i < n; i += 1) {
        total += at(theta, i);
        row[i] = JOINT_SUM_WEIGHT;
      }
      accumulate(JOINT_SUM_WEIGHT * total);
    }
    let improved = false;
    while (damping < 1e12) {
      const m = new Float64Array(size * size);
      for (let a = 0; a < size; a += 1) {
        for (let b = 0; b <= a; b += 1) {
          const value = at(h, a * size + b);
          m[a * size + b] = value;
          m[b * size + a] = value;
        }
        m[a * size + a] = at(h, a * size + a) * (1 + damping) + 1e-30;
      }
      const step = definiteSolve(
        m,
        size,
        Float64Array.from(g, (ga) => -ga),
      );
      if (step === undefined) {
        damping *= 10;
        continue;
      }
      const trial = Float64Array.from(theta, (value, a) =>
        a < size - n
          ? Math.max(value + at(step, a), 0)
          : Math.log(bounded(Math.exp(value + at(step, a)))),
      );
      const value = squares(trial);
      if (value < current) {
        const relative = (current - value) / current;
        theta = trial;
        current = value;
        damping = Math.max(damping / 3, 1e-12);
        improved = relative > 1e-12;
        break;
      }
      damping *= 4;
    }
    if (!improved) {
      break;
    }
  }
  const rates = Float64Array.from({ length: n }, (_, i) => Math.exp(at(theta, size - n + i)));
  if (!freeWeights) {
    return { weights: start.weights, rates };
  }
  let total = 0;
  for (let i = 0; i < n; i += 1) {
    total += at(theta, i);
  }
  const kept = Array.from({ length: n }, (_, i) => i).filter((i) => at(theta, i) > 0);
  return {
    weights: Float64Array.from(kept, (i) => at(theta, i) / total),
    rates: Float64Array.from(kept, (i) => at(rates, i)),
  };
}

/** The rates that best fit a row for fixed weights ({@link leastSquaresTerms}). */
function ratesFor(
  x: Float64Array,
  t: Float64Array,
  weights: Float64Array,
  start: Float64Array,
): Float64Array {
  return leastSquaresTerms(x, t, { weights, rates: start }, false).rates;
}

/**
 * The weights and rates freed from the grid together ({@link leastSquaresTerms}), kept where they
 * lower the largest error.
 */
function polished(
  x: Float64Array,
  t: Float64Array,
  start: Terms,
): { readonly terms: Terms; readonly error: number } {
  const startError = largestError(x, t, start);
  const terms = leastSquaresTerms(x, t, start, true);
  const error = largestError(x, t, terms);
  return error < startError ? { terms, error } : { terms: start, error: startError };
}

/**
 * One row's fit, cut back (the module's remarks): each step drops the term whose loss, the weights
 * refitted on the grid's rates, raises the largest error least, and polishes that set only where
 * the grid's rates alone miss the tolerance.
 */
function fitRow(x: Float64Array, t: Float64Array, options: ExpSumOptions): Terms {
  let opaque = 0;
  for (const value of t) {
    opaque = Math.max(opaque, 1 - value);
  }
  if (opaque <= options.tolerance) {
    return { weights: Float64Array.of(1), rates: Float64Array.of(0) };
  }
  let terms = weightsOn(x, t, rateGrid());
  let error = largestError(x, t, terms);
  while (terms.weights.length > 1) {
    let bestTerms: Terms | undefined;
    let bestError = Infinity;
    for (let drop = 0; drop < terms.rates.length; drop += 1) {
      const rates = Array.from(terms.rates).filter((_, i) => i !== drop);
      const trial = weightsOn(x, t, rates);
      const trialError = largestError(x, t, trial);
      if (trialError < bestError) {
        bestError = trialError;
        bestTerms = trial;
      }
    }
    if (bestTerms === undefined) {
      break;
    }
    if (bestError > options.tolerance) {
      const refined = polished(x, t, bestTerms);
      bestTerms = refined.terms;
      bestError = refined.error;
    }
    if (bestError > options.tolerance && terms.weights.length <= options.maxTerms) {
      break;
    }
    terms = bestTerms;
    error = bestError;
  }
  const refined = polished(x, t, terms);
  if (refined.error < error) {
    terms = refined.terms;
  }
  const order = Array.from(terms.rates.keys()).toSorted(
    (i, k) => at(terms.rates, i) - at(terms.rates, k),
  );
  return {
    weights: Float64Array.from(order, (i) => at(terms.weights, i)),
    rates: Float64Array.from(order, (i) => at(terms.rates, i)),
  };
}

// ---------------------------------------------------------------------------------------------
// The curve.

/**
 * A curve's exponential sum over its rows (the module's remarks): the weights at the reference
 * row, the k of every row for those weights.
 *
 * @param referenceLogPressure - ln(p ÷ 1 Pa) the weights are fitted nearest: the column's own
 *   absorber-weighted pressure. Ignored for a curve of one row.
 * @throws RangeError for a curve with no nodes or rows, or whose arrays disagree in size.
 */
export function fitExpSum(
  curve: CurveRows,
  referenceLogPressure: number,
  options: ExpSumOptions = EXP_SUM_OPTIONS,
): ExpSumFit {
  const nodes = curve.logColumns.length;
  const rows = curve.logPressures.length;
  if (nodes < 2 || rows < 1 || curve.transmittance.length !== nodes * rows) {
    throw new RangeError(
      `a curve needs nodes and rows, ${nodes} × ${rows} values, not ${curve.transmittance.length}`,
    );
  }
  const middle = at(curve.logColumns, (nodes - 1) >> 1);
  const x = Float64Array.from(curve.logColumns, (lnU) => Math.exp(lnU - middle));
  const unit = Math.exp(middle);
  const rowOf = (r: number): Float64Array =>
    curve.transmittance.subarray(r * nodes, (r + 1) * nodes);
  let reference = 0;
  for (let r = 1; r < rows; r += 1) {
    if (
      Math.abs(at(curve.logPressures, r) - referenceLogPressure) <
      Math.abs(at(curve.logPressures, reference) - referenceLogPressure)
    ) {
      reference = r;
    }
  }
  const fitted = fitRow(x, rowOf(reference), options);
  const n = fitted.weights.length;
  const rates = new Float64Array(rows * n);
  rates.set(fitted.rates, reference * n);
  const windowRow = fitted.rates.every((rate) => rate === 0);
  // Each row from its neighbour's, outward from the reference row.
  for (const step of [-1, 1]) {
    for (let r = reference + step; r >= 0 && r < rows; r += step) {
      const start = rates.slice((r - step) * n, (r - step + 1) * n);
      const row = rowOf(r);
      const solved =
        windowRow && row.every((v) => 1 - v <= options.tolerance)
          ? start
          : ratesFor(
              x,
              row,
              fitted.weights,
              start.map((rate) => Math.max(rate, RATE_BOUNDS[0])),
            );
      rates.set(solved, r * n);
    }
  }
  const rowErrors = Float64Array.from({ length: rows }, (_, r) =>
    largestError(x, rowOf(r), {
      weights: fitted.weights,
      rates: rates.subarray(r * n, (r + 1) * n),
    }),
  );
  return {
    weights: fitted.weights,
    logPressures: curve.logPressures,
    readTopLogPressure: curve.readTopLogPressure,
    coefficients: rates.map((rate) => rate / unit),
    referenceRow: reference,
    largestError: Math.max(...rowErrors),
    rowErrors,
    crossedRows: Array.from({ length: rows }, (_, r) => r).filter((r) => {
      for (let i = 1; i < n; i += 1) {
        if (at(rates, r * n + i) < at(rates, r * n + i - 1)) {
          return true;
        }
      }
      return false;
    }).length,
  };
}

/** Where a pressure is read: between `row` and `next`, a `share` of the way from the first. */
export interface ExpSumRows {
  readonly row: number;
  readonly next: number;
  /** In [0, 1]. */
  readonly share: number;
}

/**
 * The two rows a path's absorber-weighted pressure is read between, and its share of the way from
 * the first to the second: p held at the first row below and at the read's top above, as the
 * curves' read holds it (`absorbers.ts`'s `absorberBinTransmittance`); row 0 alone for a fit of one
 * row.
 *
 * @param pressurePa - The Curtis–Godson pressure, Pa; ignored for a fit of one row.
 * @throws RangeError for a fit of several rows read with no pressure above 0.
 */
export function expSumRowsAt(fit: ExpSumFit, pressurePa: number | undefined): ExpSumRows {
  const rows = fit.logPressures.length;
  if (rows === 1) {
    return { row: 0, next: 0, share: 0 };
  }
  if (pressurePa === undefined || !(Number.isFinite(pressurePa) && pressurePa > 0)) {
    throw new RangeError(
      `an exponential sum with pressure rows is read at a pressure above 0, not ${String(pressurePa)}`,
    );
  }
  const first = at(fit.logPressures, 0);
  const top = Math.min(at(fit.logPressures, rows - 1), fit.readTopLogPressure);
  const y = Math.min(Math.max(Math.log(pressurePa), first), top);
  let r = 0;
  while (r < rows - 2 && at(fit.logPressures, r + 1) < y) {
    r += 1;
  }
  const lo = at(fit.logPressures, r);
  const hi = at(fit.logPressures, r + 1);
  return { row: r, next: r + 1, share: Math.min(Math.max((y - lo) / (hi - lo), 0), 1) };
}

/**
 * Each term's k at a path's absorber-weighted pressure: ln k linear in ln p between the two rows
 * about it ({@link expSumRowsAt}), k linear where either is 0.
 *
 * @param pressurePa - The Curtis–Godson pressure, Pa; ignored for a fit of one row.
 * @throws RangeError as {@link expSumRowsAt}.
 */
export function expSumCoefficientsAt(fit: ExpSumFit, pressurePa: number | undefined): Float64Array {
  const n = fit.weights.length;
  const { row, next, share } = expSumRowsAt(fit, pressurePa);
  return Float64Array.from({ length: n }, (_, i) => {
    const a = at(fit.coefficients, row * n + i);
    const b = at(fit.coefficients, next * n + i);
    return a > 0 && b > 0
      ? Math.exp(Math.log(a) + share * (Math.log(b) - Math.log(a)))
      : a + share * (b - a);
  });
}

/** Σᵢ wᵢ e^(−kᵢ u) at one row: the fit's transmittance through an amount u. */
export function expSumTransmittance(fit: ExpSumFit, row: number, amount: number): number {
  const n = fit.weights.length;
  let sum = 0;
  for (let i = 0; i < n; i += 1) {
    sum += at(fit.weights, i) * Math.exp(-at(fit.coefficients, row * n + i) * amount);
  }
  return sum;
}

/** An entry that the loops' bounds keep in range; NaN, never `undefined`, should they not. */
function at(a: Float64Array, i: number): number {
  return a[i] ?? Number.NaN;
}

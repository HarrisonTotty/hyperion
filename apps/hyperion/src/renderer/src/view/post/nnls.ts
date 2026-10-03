/**
 * Non-negative least squares, Lawson and Hanson's active-set algorithm (Solving Least Squares
 * Problems, 1974, ch. 23), for the bloom kernel's level weights (plan R07, Design note 12).
 *
 * @remarks
 * Sized for a handful of unknowns: each step solves the passive set's normal equations by Gaussian
 * elimination with partial pivoting.
 */

/** A dense matrix as its rows. */
export type Rows = ReadonlyArray<ReadonlyArray<number>>;

function column(a: Rows, j: number): number[] {
  return a.map((row) => row[j] ?? 0);
}

function residual(a: Rows, b: ReadonlyArray<number>, x: ReadonlyArray<number>): number[] {
  return a.map((row, i) => (b[i] ?? 0) - row.reduce((sum, v, j) => sum + v * (x[j] ?? 0), 0));
}

function gradient(a: Rows, r: ReadonlyArray<number>, n: number): number[] {
  const out: number[] = [];
  for (let j = 0; j < n; j += 1) {
    out.push(column(a, j).reduce((sum, v, i) => sum + v * (r[i] ?? 0), 0));
  }
  return out;
}

/** Solves the square system `m x = v` in place by Gaussian elimination with partial pivoting. */
function solveSquare(m: number[][], v: number[]): number[] {
  const n = v.length;
  for (let k = 0; k < n; k += 1) {
    let pivot = k;
    for (let i = k + 1; i < n; i += 1) {
      if (Math.abs(m[i]?.[k] ?? 0) > Math.abs(m[pivot]?.[k] ?? 0)) {
        pivot = i;
      }
    }
    const rowK = m[pivot];
    const rowSwap = m[k];
    if (rowK === undefined || rowSwap === undefined) {
      throw new Error("nnls: row index out of range");
    }
    m[k] = rowK;
    m[pivot] = rowSwap;
    const vk = v[pivot] ?? 0;
    v[pivot] = v[k] ?? 0;
    v[k] = vk;
    const diagonal = rowK[k] ?? 0;
    if (diagonal === 0) {
      throw new Error("nnls: singular passive set");
    }
    for (let i = k + 1; i < n; i += 1) {
      const row = m[i];
      if (row === undefined) {
        continue;
      }
      const f = (row[k] ?? 0) / diagonal;
      for (let j = k; j < n; j += 1) {
        row[j] = (row[j] ?? 0) - f * (rowK[j] ?? 0);
      }
      v[i] = (v[i] ?? 0) - f * (v[k] ?? 0);
    }
  }
  const x = Array.from({ length: n }, () => 0);
  for (let i = n - 1; i >= 0; i -= 1) {
    const row = m[i] ?? [];
    let sum = v[i] ?? 0;
    for (let j = i + 1; j < n; j += 1) {
      sum -= (row[j] ?? 0) * (x[j] ?? 0);
    }
    x[i] = sum / (row[i] ?? 1);
  }
  return x;
}

/** The unconstrained least-squares solution over the columns in `passive`, zero elsewhere. */
function passiveSolve(a: Rows, b: ReadonlyArray<number>, passive: ReadonlyArray<number>): number[] {
  const n = a[0]?.length ?? 0;
  const normal = passive.map((p) =>
    passive.map((q) => a.reduce((sum, row) => sum + (row[p] ?? 0) * (row[q] ?? 0), 0)),
  );
  const rhs = passive.map((p) => a.reduce((sum, row, i) => sum + (row[p] ?? 0) * (b[i] ?? 0), 0));
  const solved = solveSquare(normal, rhs);
  const out = Array.from({ length: n }, () => 0);
  passive.forEach((p, k) => {
    out[p] = solved[k] ?? 0;
  });
  return out;
}

/**
 * The `x ≥ 0` that minimises ‖A x − b‖₂.
 *
 * @param a - The matrix, as rows of equal length.
 * @param b - The target, one value per row of `a`.
 */
export function nnls(a: Rows, b: ReadonlyArray<number>): number[] {
  const n = a[0]?.length ?? 0;
  const tolerance = 1e-12;
  let x = Array.from({ length: n }, () => 0);
  const passive: number[] = [];
  for (let outer = 0; outer < 3 * n + 10; outer += 1) {
    const w = gradient(a, residual(a, b, x), n);
    let best = -1;
    let bestValue = tolerance;
    for (let j = 0; j < n; j += 1) {
      if (!passive.includes(j) && (w[j] ?? 0) > bestValue) {
        best = j;
        bestValue = w[j] ?? 0;
      }
    }
    if (best < 0) {
      return x;
    }
    passive.push(best);
    for (let inner = 0; inner < 3 * n + 10; inner += 1) {
      const s = passiveSolve(a, b, passive);
      if (passive.every((p) => (s[p] ?? 0) > tolerance)) {
        x = s;
        break;
      }
      let alpha = Infinity;
      for (const p of passive) {
        const sp = s[p] ?? 0;
        const xp = x[p] ?? 0;
        if (sp <= tolerance) {
          alpha = Math.min(alpha, xp / (xp - sp));
        }
      }
      x = x.map((xi, i) => xi + alpha * ((s[i] ?? 0) - xi));
      for (let k = passive.length - 1; k >= 0; k -= 1) {
        const p = passive[k] ?? 0;
        if ((x[p] ?? 0) <= tolerance) {
          x[p] = 0;
          passive.splice(k, 1);
        }
      }
    }
  }
  return x;
}

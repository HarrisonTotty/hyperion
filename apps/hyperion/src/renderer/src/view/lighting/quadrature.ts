/**
 * Gauss–Legendre rules, shared by the lighting's quadratures and its `f64` oracles (plan R07,
 * Design note 6; T10.b).
 */

/** A Gauss–Legendre rule on [−1, 1]. */
export interface GaussLegendreRule {
  /** The nodes. */
  readonly x: Float64Array;
  /** The weights, summing to 2. */
  readonly w: Float64Array;
}

/** Gauss–Legendre nodes and weights on [−1, 1], by Newton's method on Pₙ. */
export function gaussLegendre(n: number): GaussLegendreRule {
  const x = new Float64Array(n);
  const w = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let z = Math.cos((Math.PI * (i + 0.75)) / (n + 0.5));
    let derivative = 0;
    for (let iteration = 0; iteration < 100; iteration += 1) {
      let p0 = 1;
      let p1 = 0;
      for (let k = 1; k <= n; k += 1) {
        const p2 = p1;
        p1 = p0;
        p0 = ((2 * k - 1) * z * p1 - (k - 1) * p2) / k;
      }
      derivative = (n * (z * p0 - p1)) / (z * z - 1);
      const step = p0 / derivative;
      z -= step;
      if (Math.abs(step) < 1e-15) {
        break;
      }
    }
    x[i] = z;
    w[i] = 2 / ((1 - z * z) * derivative * derivative);
  }
  return { x, w };
}

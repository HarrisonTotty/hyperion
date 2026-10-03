/**
 * The `f64` oracles that the lighting and shading closed forms are tested against (plan R07,
 * Design notes 5 and 6): slow, dense numerical integrals, never called by the renderer.
 */

/** A reflectance I/F as a function of μ₀, μ and the phase angle, rad. */
export type Reflectance = (mu0: number, mu: number, phaseRad: number) => number;

/**
 * The disc integral of a reflectance at phase α: (1 ÷ π) ∫ (I/F) μ dS over the lit, visible
 * hemisphere of a unit sphere, which is p Φ(α) for the law, p at α = 0.
 *
 * @remarks
 * In photometric coordinates, luminance longitude Λ from the sub-observer point towards the Sun
 * and latitude B: μ = cos B cos Λ, μ₀ = cos B cos(α − Λ), dS = cos B dB dΛ, over Λ from α − π/2
 * to π/2 and B from −π/2 to π/2. Gauss–Legendre in both, `nodes` per axis; the integrand is smooth
 * inside the domain, so a few hundred nodes reach 10⁻⁸.
 *
 * @param phaseRad - Phase angle, rad, 0 to π.
 * @param nodes - Gauss–Legendre nodes per axis.
 */
export function discIntegral(reflectance: Reflectance, phaseRad: number, nodes = 200): number {
  const { x, w } = gaussLegendre(nodes);
  const lowLon = phaseRad - Math.PI / 2;
  const highLon = Math.PI / 2;
  const halfLon = (highLon - lowLon) / 2;
  const midLon = (highLon + lowLon) / 2;
  let sum = 0;
  for (let i = 0; i < nodes; i += 1) {
    const lon = midLon + halfLon * (x[i] ?? 0);
    let inner = 0;
    for (let j = 0; j < nodes; j += 1) {
      const lat = (Math.PI / 2) * (x[j] ?? 0);
      const cosLat = Math.cos(lat);
      const mu = cosLat * Math.cos(lon);
      const mu0 = cosLat * Math.cos(phaseRad - lon);
      if (mu > 0 && mu0 > 0) {
        inner += (w[j] ?? 0) * reflectance(mu0, mu, phaseRad) * mu * cosLat;
      }
    }
    sum += (w[i] ?? 0) * inner * (Math.PI / 2);
  }
  return (sum * halfLon) / Math.PI;
}

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

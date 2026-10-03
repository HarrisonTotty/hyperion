/**
 * The disc-integrated phase functions of the law's two shapes, and the phase integral (plan R07,
 * Design note 5).
 *
 * @remarks
 * The law's disc term is L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀, a Lommel–Seeliger share L beside a
 * Lambert share. Each shape has its disc-integrated phase function in closed form, normalised to
 * one at opposition: Lambert's Φ_L(α) = [sin α + (π − α) cos α] ÷ π, and Lommel–Seeliger's
 * Φ_LS(α) = 1 − sin(α/2) tan(α/2) ln cot(α/4), checked numerically: Φ_LS(0) = 1 and its phase
 * integral equals Fairbairn's q = 16/3 (1 − ln 2) (Fairbairn, "Planetary photometry: the
 * Lommel–Seeliger law", JRASC 99 (2005), Table I, with p = ϖ/8). Under I/F the
 * Lommel–Seeliger term's geometric albedo is 1 and Lambert's ⅔, so the mixture's is
 * L + ⅔(1 − L) and its phase function the albedo-weighted mean of the two.
 */

/** Lambert sphere's disc-integrated phase function Φ_L(α), Φ_L(0) = 1, Φ_L(π) = 0. */
export function lambertPhase(alphaRad: number): number {
  const alpha = Math.min(Math.max(alphaRad, 0), Math.PI);
  // Rounding leaves some −10⁻¹⁷ at π; a phase function is never negative.
  return Math.max(0, (Math.sin(alpha) + (Math.PI - alpha) * Math.cos(alpha)) / Math.PI);
}

/** Lommel–Seeliger sphere's disc-integrated phase function Φ_LS(α), Φ_LS(0) = 1, Φ_LS(π) = 0. */
export function lommelSeeligerPhase(alphaRad: number): number {
  if (alphaRad <= 0) {
    return 1;
  }
  if (alphaRad >= Math.PI) {
    return 0;
  }
  const half = alphaRad / 2;
  return 1 - Math.sin(half) * Math.tan(half) * Math.log(1 / Math.tan(alphaRad / 4));
}

/**
 * The geometric albedo of the law's disc term per unit albedo scale A: L + ⅔(1 − L).
 *
 * @param lommelSeeligerShare - L, 0 (Lambert) to 1 (Lommel–Seeliger).
 */
export function shapeGeometricAlbedo(lommelSeeligerShare: number): number {
  return lommelSeeligerShare + (2 / 3) * (1 - lommelSeeligerShare);
}

/**
 * Φ_shape(α; L), the disc term's disc-integrated phase function, one at opposition.
 *
 * @param lommelSeeligerShare - L, 0 (Lambert) to 1 (Lommel–Seeliger).
 */
export function shapePhase(lommelSeeligerShare: number, alphaRad: number): number {
  const l = lommelSeeligerShare;
  const lambertWeight = (2 / 3) * (1 - l);
  return (
    (l * lommelSeeligerPhase(alphaRad) + lambertWeight * lambertPhase(alphaRad)) /
    (l + lambertWeight)
  );
}

/** Intervals of the phase integral's Simpson rule over [0, π]; even. */
const PHASE_INTEGRAL_INTERVALS = 7200;

/**
 * The phase integral q = 2 ∫₀^π Φ(α) sin α dα of a phase function normalised to one at opposition.
 *
 * @remarks
 * Simpson's rule over 7,200 intervals (0.025°), finer than the 0.5° table so that the table's
 * linear interpolation, not the quadrature, sets the error.
 */
export function phaseIntegral(phase: (alphaRad: number) => number): number {
  const n = PHASE_INTEGRAL_INTERVALS;
  const h = Math.PI / n;
  let sum = 0;
  for (let i = 0; i <= n; i += 1) {
    const alpha = i * h;
    const weight = i === 0 || i === n ? 1 : i % 2 === 1 ? 4 : 2;
    sum += weight * phase(alpha) * Math.sin(alpha);
  }
  return (2 * sum * h) / 3;
}

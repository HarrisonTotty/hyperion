/**
 * A Monte Carlo reference for a cloud layer's reflectance (plan R11, R11.T7.a): photons through a
 * plane-parallel, conservatively scattering slab with a Henyey–Greenstein phase function.
 *
 * @remarks
 * A test reference only, not used at run time: it re-runs plan R11's twelve-case Monte Carlo
 * column, against which `deltaEddington`'s accuracy is stated (`cloudOptics.ts`), at a size a unit
 * test can afford. Pure: the uniform source is injected, so that the caller picks the seed and
 * production code imports nothing from the tests.
 */

/** A homogeneous conservative slab, the beam on it and the run's size. */
export interface SlabMonteCarloInput {
  /** τ, the slab's vertical optical depth, dimensionless: finite and at least 0. */
  readonly tau: number;
  /** g, the Henyey–Greenstein phase function's asymmetry parameter, in [0, 1). */
  readonly asymmetry: number;
  /** μ₀, the cosine of the beam's zenith angle, in (0, 1]. */
  readonly mu0: number;
  /** N, the photons to trace: a positive safe integer. */
  readonly photons: number;
}

/** Where a run's photons left the slab. */
export interface SlabMonteCarloResult {
  /** N, the photons traced. */
  readonly photons: number;
  /** The photons that left through the top. */
  readonly reflected: number;
  /** The photons that left through the base, scattered or not. */
  readonly transmitted: number;
  /** R_N, the reflected fraction; its standard error is √(R_N(1 − R_N) ÷ N). */
  readonly reflectance: number;
  /** T_N, the transmitted fraction, 1 − R_N since the slab absorbs nothing. */
  readonly transmittance: number;
}

/**
 * The |g| below which a scattering is drawn isotropic: the inverse below divides by 2g, and loses
 * about ε ÷ |g| in cos θ; the Henyey–Greenstein distribution differs from isotropic by O(g), so at
 * |g| < 10⁻⁸ either error is negligible.
 */
const ISOTROPIC_BELOW = 1e-8;

/**
 * cos θ of a Henyey–Greenstein scattering (Henyey and Greenstein 1941, eq. 2, whose eq. 3 makes g
 * the mean cosine) from a uniform ξ in [0, 1): the inverse of its cumulative distribution,
 * cos θ = [1 + g² − ((1 − g²) ÷ (1 − g + 2gξ))²] ÷ (2g), and 2ξ − 1 at g = 0 (as in Wang and
 * Jacques's MCML manual, "Monte Carlo Modeling of Light Transport in Multi-layered Tissues in
 * Standard C", eq. 3.19), clamped to [−1, 1] against round-off.
 */
function scatteringCosine(g: number, xi: number): number {
  if (Math.abs(g) < ISOTROPIC_BELOW) {
    return 2 * xi - 1;
  }
  const q = (1 - g * g) / (1 - g + 2 * g * xi);
  return Math.min(1, Math.max(-1, (1 + g * g - q * q) / (2 * g)));
}

/**
 * Traces photons through a plane-parallel slab that scatters conservatively with a
 * Henyey–Greenstein phase function of asymmetry g (Henyey and Greenstein 1941), lit by a collimated
 * beam at μ₀ over a black surface, and counts where they leave it.
 *
 * @remarks
 * Each photon starts at the top going down at μ₀. Its free path is −ln(1 − ξ) in optical depth,
 * which never takes the logarithm of 0 since ξ < 1; it moves μ times that in depth. Leaving the top
 * counts as reflected and leaving the base as transmitted; otherwise it scatters: θ from the
 * Henyey–Greenstein inverse, the azimuth φ = 2πξ, and μ′ = μ cos θ + √(1 − μ²) sin θ cos φ,
 * clamped to [−1, 1]. The slab is horizontally uniform, so μ is the only direction kept. The run is
 * analog and absorbs nothing, so R_N + T_N = 1. Its cost grows with τ: at τ = 50 a photon scatters
 * about 120 times at μ₀ = 1 and 90 at μ₀ = 0.5, and at τ = 10 about 20 (counted, 2026-10-20).
 *
 * A test reference for `deltaEddington`, not used at run time.
 *
 * @param uniform - The uniform source in [0, 1), such as the tests' seeded one; called three times
 *   a scattering and once a final flight.
 * @throws RangeError if τ is negative, g is outside [0, 1), μ₀ outside (0, 1], any of them is not
 *   finite, or the photon count is not a positive safe integer.
 */
export function slabMonteCarlo(
  input: SlabMonteCarloInput,
  uniform: () => number,
): SlabMonteCarloResult {
  const { tau, asymmetry: g, mu0, photons } = input;
  if (!Number.isFinite(tau) || tau < 0) {
    throw new RangeError(`tau must be finite and at least 0, not ${tau}`);
  }
  if (!Number.isFinite(g) || g < 0 || g >= 1) {
    throw new RangeError(`asymmetry must be in [0, 1), not ${g}`);
  }
  if (!Number.isFinite(mu0) || mu0 <= 0 || mu0 > 1) {
    throw new RangeError(`mu0 must be in (0, 1], not ${mu0}`);
  }
  if (!Number.isSafeInteger(photons) || photons <= 0) {
    throw new RangeError(`photons must be a positive safe integer, not ${photons}`);
  }

  let reflected = 0;
  let transmitted = 0;
  for (let n = 0; n < photons; n += 1) {
    let depth = 0;
    let mu = mu0;
    for (;;) {
      depth -= mu * Math.log(1 - uniform());
      if (depth < 0) {
        reflected += 1;
        break;
      }
      if (depth > tau) {
        transmitted += 1;
        break;
      }
      const cosTheta = scatteringCosine(g, uniform());
      const sinTheta = Math.sqrt(Math.max(0, 1 - cosTheta * cosTheta));
      const cosPhi = Math.cos(2 * Math.PI * uniform());
      const sinMu = Math.sqrt(Math.max(0, 1 - mu * mu));
      mu = Math.min(1, Math.max(-1, mu * cosTheta + sinMu * sinTheta * cosPhi));
    }
  }
  return {
    photons,
    reflected,
    transmitted,
    reflectance: reflected / photons,
    transmittance: transmitted / photons,
  };
}

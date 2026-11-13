/**
 * Plan R11's twelve slab cases (R11.T7.a), the test fixture of `cloudOptics.test.ts` and
 * `slabMonteCarlo.test.ts`.
 *
 * @remarks
 * A test fixture, not used at run time. It sits here rather than in `test/`, the shared fixtures'
 * home, because R11.T7.a.1's lane may not edit `test/`; it moves there when a later task can.
 */

/** One case of the table: a conservative slab and its reflectance by two methods. */
export interface SlabCase {
  /** τ, the slab's vertical optical depth, unscaled. */
  readonly tau: number;
  /** g, the Henyey–Greenstein asymmetry parameter. */
  readonly g: number;
  /** μ₀, the cosine of the sun's zenith angle. */
  readonly mu0: number;
  /** R by a plane-parallel Henyey–Greenstein Monte Carlo of statistical error about 0.002. */
  readonly monteCarlo: number;
  /** R by the conservative δ-Eddington solution, to four decimals. */
  readonly deltaEddington: number;
}

/**
 * The table of plan R11's R11.T7.a, computed by the plan's research on 2026-09-29: ϖ₀ = 1, the
 * δ-Eddington column from the conservative solution (Joseph, Wiscombe and Weinman 1976; Meador and
 * Weaver 1980) and the Monte Carlo column from a plane-parallel Henyey–Greenstein run of
 * statistical error about 0.002, read as one standard deviation.
 */
export const SLAB_CASES: ReadonlyArray<SlabCase> = [
  { tau: 1, g: 0.85, mu0: 1.0, monteCarlo: 0.0429, deltaEddington: 0.0467 },
  { tau: 1, g: 0.85, mu0: 0.5, monteCarlo: 0.1649, deltaEddington: 0.149 },
  { tau: 10, g: 0.85, mu0: 1.0, monteCarlo: 0.4216, deltaEddington: 0.4191 },
  { tau: 10, g: 0.85, mu0: 0.5, monteCarlo: 0.6033, deltaEddington: 0.588 },
  { tau: 50, g: 0.85, mu0: 1.0, monteCarlo: 0.8078, deltaEddington: 0.8113 },
  { tau: 50, g: 0.85, mu0: 0.5, monteCarlo: 0.8702, deltaEddington: 0.8679 },
  { tau: 1, g: 0.75, mu0: 1.0, monteCarlo: 0.0787, deltaEddington: 0.0833 },
  { tau: 1, g: 0.75, mu0: 0.5, monteCarlo: 0.241, deltaEddington: 0.2193 },
  { tau: 10, g: 0.75, mu0: 1.0, monteCarlo: 0.5676, deltaEddington: 0.5663 },
  { tau: 10, g: 0.75, mu0: 0.5, monteCarlo: 0.7059, deltaEddington: 0.6956 },
  { tau: 50, g: 0.75, mu0: 1.0, monteCarlo: 0.8774, deltaEddington: 0.8795 },
  { tau: 50, g: 0.75, mu0: 0.5, monteCarlo: 0.9161, deltaEddington: 0.9157 },
];

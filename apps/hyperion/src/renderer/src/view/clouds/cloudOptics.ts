/**
 * A cloud layer's optics (plan R11, R11.T7.a, Design note 10): the δ-Eddington reflectance and the
 * direct and diffuse transmittance of a plane-parallel layer lit by a direct solar beam.
 *
 * @remarks
 * Pure: no state, no clock, no platform API, `f64` throughout, so that it is the CPU reference for
 * the cloud layer's and the volume's WGSL (R11.T7.b, T7.c). Both settings' clouds share these
 * optics; the layer's per-texel τ is the column integral of the density the volume marches.
 */

/** A homogeneous plane-parallel layer and the sun on it, each quantity dimensionless. */
export interface DeltaEddingtonInput {
  /** τ, the layer's vertical optical depth at the wavelength, unscaled: finite and at least 0. */
  readonly tau: number;
  /**
   * g, the asymmetry parameter (the mean cosine of the scattering angle) of the full phase
   * function, in [0, 1).
   */
  readonly asymmetry: number;
  /** ϖ₀, the single-scattering albedo, in [0, 1]. */
  readonly singleScatterAlbedo: number;
  /** μ₀, the cosine of the sun's zenith angle at the layer, in (0, 1]. */
  readonly mu0: number;
}

/**
 * Where a direct solar beam on a layer goes, each part a fraction of the flux the beam brings to a
 * horizontal surface, μ₀πF₀, with a black surface below the layer.
 *
 * @remarks
 * The four parts sum to 1. Round-off can leave a part that is 0 in exact arithmetic a few times
 * 10⁻¹⁶ below 0.
 */
export interface SlabFluxFractions {
  /** R, the fraction scattered back up out of the layer's top. */
  readonly reflectance: number;
  /**
   * T_dir, the fraction that crosses the layer unscattered, e^(−τ ÷ μ₀) with the unscaled τ: the
   * only part a cloud shadow lets through as sunlight.
   */
  readonly directTransmittance: number;
  /**
   * T_diff, the fraction that leaves the layer's base scattered: the total transmittance less
   * T_dir.
   */
  readonly diffuseTransmittance: number;
  /** A, the fraction absorbed in the layer: 1 − R − T_dir − T_diff, 0 when ϖ₀ = 1. */
  readonly absorptance: number;
}

/**
 * The co-albedo 1 − ϖ₀ at and below which {@link deltaEddington} takes the conservative solution.
 *
 * @remarks
 * The general solution divides two quantities that vanish with k, the two-stream eigenvalue, which
 * goes as √(1 − ϖ₀); its relative error grows as ε ÷ k, ε the `f64` epsilon, and below a co-albedo
 * of about 10⁻¹³ it loses accuracy. At 10⁻¹² it is within 2 × 10⁻¹¹ of a two-stream ODE solution
 * at τ 1 and 10 (main's check, 2026-10-20), and within about 7 × 10⁻¹¹ down to τ = 10⁻³ (an
 * 80-digit solution, R11.T7.a.1's review); across the switch R moves by at most 8 × 10⁻¹¹ at
 * τ ≤ 50 and μ₀ down to 0.05, and the absorptance the conservative side drops is at most
 * 1.2 × 10⁻¹⁰ there.
 */
export const CONSERVATIVE_CO_ALBEDO = 1e-12;

/**
 * Half the width, in kμ₀, of the window about the general solution's removable singularity at
 * kμ₀ = 1 inside which {@link deltaEddington} interpolates.
 *
 * @remarks
 * At kμ₀ = 1 the closed form's denominator and its brackets all vanish, and near it each is a
 * small difference of order-one terms, so the closed form's relative error grows as
 * ε ÷ |1 − kμ₀|, ε the `f64` epsilon. Inside the window the scattered part of R and T is
 * interpolated linearly in μ₀ between the closed form at kμ₀ = 1 ∓ h, whose error is then about
 * ε ÷ h, and the interpolation's about h² times its second derivative in kμ₀; h = 10⁻⁵, near
 * ε^(1/3), balances the two. Inside the window the result is within 8.3 × 10⁻¹² of an 80-digit
 * solution, and the step at the window's edges is up to 1.7 × 10⁻¹¹ (R11.T7.a.1's checks).
 */
const SINGULAR_HALF_WIDTH = 1e-5;

/** R and T of the δ-scaled problem, T its total transmittance (direct and diffuse). */
interface ScaledSolution {
  readonly reflectance: number;
  readonly transmittance: number;
}

/** The δ-scaled layer: τ′, ϖ′, g′ and the co-albedo 1 − ϖ′, which is kept apart for accuracy. */
interface ScaledLayer {
  readonly tauScaled: number;
  readonly albedoScaled: number;
  readonly coAlbedoScaled: number;
  readonly asymmetryScaled: number;
}

function requireIn(
  name: string,
  value: number,
  valid: (v: number) => boolean,
  domain: string,
): void {
  if (!Number.isFinite(value) || !valid(value)) {
    throw new RangeError(`${name} must be ${domain}, not ${value}`);
  }
}

/**
 * The δ-scaling of Joseph, Wiscombe and Weinman 1976, with the Henyey–Greenstein forward fraction
 * f = g² (their eq. 5a): τ′ = (1 − ϖ₀f) τ (eq. 13), ϖ′ = (1 − f) ϖ₀ ÷ (1 − ϖ₀f) (eq. 14) and
 * g′ = (g − f) ÷ (1 − f) = g ÷ (1 + g) (eqs. 2b and 5b). The co-albedo is
 * 1 − ϖ′ = (1 − ϖ₀) ÷ (1 − ϖ₀f), the same quantity without the cancellation.
 */
function deltaScale(tau: number, g: number, w0: number): ScaledLayer {
  const f = g * g;
  const remaining = 1 - w0 * f;
  return {
    tauScaled: remaining * tau,
    albedoScaled: ((1 - f) * w0) / remaining,
    coAlbedoScaled: (1 - w0) / remaining,
    asymmetryScaled: g / (1 + g),
  };
}

/**
 * The conservative (ϖ′ = 1) Eddington solution for a collimated beam over a black surface
 * (Meador and Weaver 1980, eq. 24): R = [γ₁τ′ + (γ₃ − γ₁μ₀)(1 − e^(−τ′ ÷ μ₀))] ÷ (1 + γ₁τ′) and
 * T = 1 − R,
 * with γ₁ = 3(1 − g′) ÷ 4 and γ₃ = (2 − 3g′μ₀) ÷ 4.
 */
function conservative(layer: ScaledLayer, mu0: number): ScaledSolution {
  const { tauScaled: t, asymmetryScaled: gs } = layer;
  const g1 = (3 * (1 - gs)) / 4;
  const g3 = (2 - 3 * gs * mu0) / 4;
  const reflectance = (g1 * t + (g3 - g1 * mu0) * -Math.expm1(-t / mu0)) / (1 + g1 * t);
  return { reflectance, transmittance: 1 - reflectance };
}

/**
 * The scattered part of the general solution at one μ₀: R, and the scaled problem's diffuse
 * transmittance, its total less its own direct beam e^(−τ′ ÷ μ₀). Both carry the factor ϖ′ and the
 * removable singularity at kμ₀ = 1; the direct beam has neither.
 */
interface ScatteredPart {
  readonly reflectance: number;
  readonly scatteredTransmittance: number;
}

/**
 * The general (ϖ′ < 1) Eddington solution for a collimated beam over a black surface (Meador and
 * Weaver 1980), divided through by e^(kτ′) so that only decaying exponentials appear.
 *
 * @remarks
 * With the Eddington coefficients γ₁ = [7 − ϖ′(4 + 3g′)] ÷ 4, γ₂ = −[1 − ϖ′(4 − 3g′)] ÷ 4 and
 * γ₃ = (2 − 3g′μ₀) ÷ 4 (their Table 1, the Eddington row) and γ₄ = 1 − γ₃ (eq. 21);
 * k = √(γ₁² − γ₂²) (eq. 18), computed as √(3(1 − ϖ′)(1 − ϖ′g′)), which is the same and has no
 * cancellation;
 * α₁ = γ₁γ₄ + γ₂γ₃ and α₂ = γ₁γ₃ + γ₂γ₄ (eqs. 16 and 17); and t = τ′, R and T being their eqs. 14
 * and 15 so divided:
 *
 * - D = (1 − k²μ₀²)((k + γ₁) + (k − γ₁)e^(−2kt));
 * - R = (ϖ′ ÷ D)[(1 − kμ₀)(α₂ + kγ₃) − (1 + kμ₀)(α₂ − kγ₃)e^(−2kt)
 *   − 2k(γ₃ − α₂μ₀)e^(−t÷μ₀)e^(−kt)];
 * - T = e^(−t÷μ₀) − (ϖ′ ÷ D)[(1 + kμ₀)(α₁ + kγ₄)e^(−t÷μ₀) − (1 − kμ₀)(α₁ − kγ₄)e^(−2kt)e^(−t÷μ₀)
 *   − 2k(γ₄ + α₁μ₀)e^(−kt)].
 *
 * This returns R and T's second term, T − e^(−t÷μ₀). D and both brackets vanish at kμ₀ = 1, a
 * removable singularity that {@link general} steps around; this function divides by zero there.
 */
function closedForm(layer: ScaledLayer, mu0: number, k: number): ScatteredPart {
  const { tauScaled: t, albedoScaled: w, asymmetryScaled: gs } = layer;
  const g1 = (7 - w * (4 + 3 * gs)) / 4;
  const g2 = -(1 - w * (4 - 3 * gs)) / 4;
  const g3 = (2 - 3 * gs * mu0) / 4;
  const g4 = 1 - g3;
  const a1 = g1 * g4 + g2 * g3;
  const a2 = g1 * g3 + g2 * g4;
  const km = k * mu0;
  const eK = Math.exp(-k * t);
  const e2K = eK * eK;
  const eSun = Math.exp(-t / mu0);
  const scale = w / ((1 - km * km) * (k + g1 + (k - g1) * e2K));
  const reflectance =
    scale *
    ((1 - km) * (a2 + k * g3) -
      (1 + km) * (a2 - k * g3) * e2K -
      2 * k * (g3 - a2 * mu0) * eSun * eK);
  const scatteredTransmittance =
    -scale *
    ((1 + km) * (a1 + k * g4) * eSun -
      (1 - km) * (a1 - k * g4) * e2K * eSun -
      2 * k * (g4 + a1 * mu0) * eK);
  return { reflectance, scatteredTransmittance };
}

/**
 * The general solution, stepping around its removable singularity at kμ₀ = 1.
 *
 * @remarks
 * Within {@link SINGULAR_HALF_WIDTH} of kμ₀ = 1, the scattered part, R and T − e^(−τ′ ÷ μ₀), is
 * interpolated linearly in μ₀ between the closed form at μ₀ = (1 ∓ h) ÷ k, where it is accurate,
 * and the scaled direct beam e^(−τ′ ÷ μ₀) is added at μ₀ itself, so that ϖ₀ = 0 still gives R = 0
 * and T = e^(−τ ÷ μ₀) exactly. The point is reached where k ≥ 1: at a low ϖ₀, or at any ϖ₀ < 1
 * with g near enough to 1, since ϖ′ falls to 0 as g rises to 1 (from g 0.930 at ϖ₀ 0.9, where
 * k = 1.11 at g 0.95; from g 0.994 at ϖ₀ 0.99). The closed form is analytic in μ₀, so a node just
 * above μ₀ = 1 is a valid evaluation.
 */
function general(layer: ScaledLayer, mu0: number): ScaledSolution {
  const { tauScaled: t, coAlbedoScaled: c, albedoScaled: w, asymmetryScaled: gs } = layer;
  const k = Math.sqrt(3 * c * (1 - w * gs));
  let part: ScatteredPart;
  if (Math.abs(1 - k * mu0) >= SINGULAR_HALF_WIDTH) {
    part = closedForm(layer, mu0, k);
  } else {
    const muBelow = (1 - SINGULAR_HALF_WIDTH) / k;
    const muAbove = (1 + SINGULAR_HALF_WIDTH) / k;
    const below = closedForm(layer, muBelow, k);
    const above = closedForm(layer, muAbove, k);
    const s = (mu0 - muBelow) / (muAbove - muBelow);
    part = {
      reflectance: below.reflectance + s * (above.reflectance - below.reflectance),
      scatteredTransmittance:
        below.scatteredTransmittance +
        s * (above.scatteredTransmittance - below.scatteredTransmittance),
    };
  }
  return {
    reflectance: part.reflectance,
    transmittance: Math.exp(-t / mu0) + part.scatteredTransmittance,
  };
}

/**
 * The reflectance, transmittance and absorptance of a homogeneous plane-parallel layer lit by a
 * direct solar beam, by the δ-Eddington approximation, for a cloud layer's shading and its shadow.
 *
 * @remarks
 * The model is the Eddington two-stream approximation for a collimated beam over a black surface,
 * solved in closed form (Meador and Weaver 1980, for the coefficients γ₁–γ₄, Table 1 and eq. 21,
 * and both solutions, eqs. 14–18 and 24), on the layer δ-scaled with the Henyey–Greenstein forward
 * fraction f = g² (Joseph, Wiscombe and Weinman 1976, eqs. 5, 13 and 14): τ′ = (1 − ϖ₀f) τ,
 * ϖ′ = (1 − f) ϖ₀ ÷ (1 − ϖ₀f), g′ = g ÷ (1 + g). Plain Eddington,
 * without the scaling, is not used: it gives R < 0 for thin, forward-scattering layers (−0.041 at
 * τ = 1, g = 0.85, μ₀ = 1). Nor is the diffuse-only two-stream, which ignores μ₀ and falls about
 * 20% short at τ = 10 (Design note 10). At 1 − ϖ₀ ≤ {@link CONSERVATIVE_CO_ALBEDO} the
 * conservative solution is taken, with A = 0; the general solution's removable singularity at
 * kμ₀ = 1 is stepped around by interpolation (see `general`'s remarks), so every ϖ₀ in the domain
 * works.
 *
 * The direct part is e^(−τ ÷ μ₀) with the **unscaled** τ. The scaled problem's own direct beam,
 * e^(−τ′ ÷ μ₀), includes the light scattered into the phase function's forward peak, which a
 * shadow on the terrain must not count as sun; that light is in the diffuse part, which lights the
 * ground through the sky term (Design note 10).
 *
 * Accuracy, conservative, against a plane-parallel Henyey–Greenstein Monte Carlo: R is within 3% of
 * plan R11's twelve-case Monte Carlo column at τ ≥ 10 (largest 2.53%, at τ = 10, g = 0.85,
 * μ₀ = 0.5) and within 10% at τ = 1 at either μ₀ (largest 9.65%, at g = 0.85, μ₀ = 0.5); against a
 * 2 × 10⁶-photon run its largest biases are 2.62% at τ ≥ 10 and 9.85% at τ = 1 (main's check,
 * 2026-10-20), and two 10⁷-photon runs put the τ = 1, g = 0.85, μ₀ = 1 bias at 10.0 ± 0.2% and
 * 10.2 ± 0.15%: about 10%, so the 10% bound holds against the table's column, not against a
 * converged run. At ϖ₀ 0.99–0.999, A is 2–10% below an absorbing Monte Carlo's (plan R11's Risks,
 * deferred findings of R11.T7.a.1).
 *
 * @param input - The layer and the sun; every quantity dimensionless.
 * @returns R, T_dir, T_diff and A, each a fraction of μ₀πF₀.
 * @throws RangeError if τ is negative, g is outside [0, 1), ϖ₀ outside [0, 1], μ₀ outside (0, 1],
 *   or any input is not finite. At g = −0.5 the scaled g′ would reach −1, and g = 1 is degenerate.
 */
export function deltaEddington(input: DeltaEddingtonInput): SlabFluxFractions {
  const { tau, asymmetry: g, singleScatterAlbedo: w0, mu0 } = input;
  requireIn("tau", tau, (v) => v >= 0, "finite and at least 0");
  requireIn("asymmetry", g, (v) => v >= 0 && v < 1, "in [0, 1)");
  requireIn("singleScatterAlbedo", w0, (v) => v >= 0 && v <= 1, "in [0, 1]");
  requireIn("mu0", mu0, (v) => v > 0 && v <= 1, "in (0, 1]");

  const layer = deltaScale(tau, g, w0);
  const isConservative = 1 - w0 <= CONSERVATIVE_CO_ALBEDO;
  const scaled = isConservative ? conservative(layer, mu0) : general(layer, mu0);
  const directTransmittance = Math.exp(-tau / mu0);
  const reflectance = scaled.reflectance;
  const diffuseTransmittance = scaled.transmittance - directTransmittance;
  const absorptance = isConservative ? 0 : 1 - reflectance - scaled.transmittance;
  return { reflectance, directTransmittance, diffuseTransmittance, absorptance };
}

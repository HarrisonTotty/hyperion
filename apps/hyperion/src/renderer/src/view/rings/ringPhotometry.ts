/**
 * A ring's photometry in every regime (plan R11, R11.T9.a; Design note 14): the classical
 * single-scattering layer's lit and unlit faces, the particles' phase function and opposition surge,
 * and Hapke's shadow-hiding term, the measured fallback of the shadowing table (Design note 15).
 *
 * @remarks
 * With μ = |sin B| the viewer's and μ₀ = |sin B′| the star's elevation sines against the ring plane
 * and P normalised over 4π, the lit face is I/F = ϖ₀ P(α) μ₀ ÷ (4(μ + μ₀)) × [1 − exp(−τ(1/μ +
 * 1/μ₀))], times the shadowing factor of Design note 15, and the unlit face, the transmitted beam
 * single-scattered, I/F = ϖ₀ P(α) μ₀ ÷ (4(μ − μ₀)) × [exp(−τ/μ) − exp(−τ/μ₀)], whose limit at
 * μ → μ₀ is ϖ₀ P τ exp(−τ/μ) ÷ 4μ (Chandrasekhar 1960, §I.11; Cuzzi et al. 1984, in "Planetary
 * Rings", eqs. 1–2). So a dense B ring is dark from below while the Cassini Division and the C ring
 * are bright. The main ring's particles scatter by Salo and French 2010's power law (Icarus 210,
 * 785, eq. 11), P(α) = c_n (π − α)^n with n = 3.09, Callisto's (Dones et al. 1993, Icarus 105, 184),
 * times an opposition surge of the half-widths Déau et al. 2009 fit (Icarus 204, 250); dusty rings
 * take a forward lobe. Pure, `f64`, no state: the CPU reference for `ringSlab.wgsl` (R11.T9.c).
 */
import type { RingRegion } from "./ringProfile";

/** The power law's exponent n: 3.09, Callisto's (Dones et al. 1993; Salo and French 2010, §3.2). */
export const POWER_LAW_PHASE_EXPONENT = 3.09;

/**
 * ∫₀^π u^n sin u du, by its series Σ_k (−1)^k π^(n+2k+2) ÷ ((2k + 1)! (n + 2k + 2)), which
 * converges as π^(2k) ÷ (2k + 1)! does: exact to rounding for any n ≥ 0.
 */
function powerLawMoment(n: number): number {
  let sum = 0;
  let term = Math.PI ** (n + 2); // π^(n+2k+2) ÷ (2k+1)! at k = 0
  for (let k = 0; k < 40; k += 1) {
    const value = term / (n + 2 * k + 2);
    sum += k % 2 === 0 ? value : -value;
    if (value < 1e-18 * Math.abs(sum)) {
      break;
    }
    term *= (Math.PI * Math.PI) / ((2 * k + 2) * (2 * k + 3));
  }
  return sum;
}

/**
 * The power law's c_n, which normalises P(α) = c_n (π − α)^n over 4π: (1 ÷ 4π) ∫ P dΩ = 1, so
 * c_n = 2 ÷ ∫₀^π u^n sin u du.
 */
export function powerLawNormalisation(n: number): number {
  return 2 / powerLawMoment(n);
}

/** c_n at {@link POWER_LAW_PHASE_EXPONENT}. */
export const POWER_LAW_NORMALISATION = powerLawNormalisation(POWER_LAW_PHASE_EXPONENT);

/** The main rings' particle phase function at phase angle α, rad in [0, π]: c_n (π − α)^n. */
export function powerLawPhase(alphaRad: number): number {
  const u = Math.PI - Math.min(Math.max(alphaRad, 0), Math.PI);
  return POWER_LAW_NORMALISATION * u ** POWER_LAW_PHASE_EXPONENT;
}

/**
 * A dusty ring's forward lobe's asymmetry g₁: 0.7. From memory and of low confidence (Design note
 * 14): dust of a few microns scatters strongly forward, as Jupiter's main ring and Saturn's G ring
 * do (Throop et al. 2004, Icarus 172, 59; Hedman and Stark 2015, ApJ 811, 67); not re-checked, no
 * source being reachable from the lanes that wrote it (2026-11-02, 2026-11-13), and tested only for
 * a dusty ring's being brighter back-lit.
 */
export const DUSTY_FORWARD_ASYMMETRY = 0.7;

/** The dusty phase's backward lobe's asymmetry, −g₂ with g₂ = 0.3: this module's lean (2026-11-02). */
export const DUSTY_BACKWARD_ASYMMETRY = 0.3;

/** The forward lobe's weight in the dusty phase, 0.9: this module's lean (2026-11-02). */
export const DUSTY_FORWARD_WEIGHT = 0.9;

/** The Henyey–Greenstein phase at scattering angle cosine `cosTheta`, normalised over 4π. */
function henyeyGreenstein(g: number, cosTheta: number): number {
  const g2 = g * g;
  return (1 - g2) / (1 + g2 - 2 * g * cosTheta) ** 1.5;
}

/**
 * A dusty ring's two-term Henyey–Greenstein phase at phase angle α, rad, normalised over 4π: a
 * forward lobe of {@link DUSTY_FORWARD_ASYMMETRY} and a weaker backward one, the scattering angle
 * θ = π − α.
 */
export function dustyPhase(alphaRad: number): number {
  const cosTheta = -Math.cos(alphaRad);
  return (
    DUSTY_FORWARD_WEIGHT * henyeyGreenstein(DUSTY_FORWARD_ASYMMETRY, cosTheta) +
    (1 - DUSTY_FORWARD_WEIGHT) * henyeyGreenstein(-DUSTY_BACKWARD_ASYMMETRY, cosTheta)
  );
}

/** A region's particle phase function at phase angle α, rad, normalised over 4π. */
export function ringPhase(alphaRad: number, region: RingRegion): number {
  let phase: number;
  switch (region) {
    case "dense":
    case "sparse":
    case "gap":
      phase = powerLawPhase(alphaRad);
      break;
    case "dusty":
      phase = dustyPhase(alphaRad);
      break;
  }
  return phase;
}

/** Degrees to radians. */
const DEG = Math.PI / 180;

/**
 * The opposition surge's half-width at half maximum, rad (Déau et al. 2009, Icarus 204, 250, their
 * fits of the linear-by-exponential model to Cassini ISS): 0.20° for the A and B rings, 0.26° for
 * the C ring and 0.28° for the Cassini Division (Design note 14). A dusty ring takes none.
 */
export const SURGE_HALF_WIDTH_RAD: Readonly<Record<Exclude<RingRegion, "dusty">, number>> = {
  dense: 0.2 * DEG,
  sparse: 0.26 * DEG,
  gap: 0.28 * DEG,
};

/**
 * The surge's amplitude in a sparse ring, the peak's ratio to the background: about 1.45 (Déau et
 * al. 2009; Design note 14).
 */
export const SPARSE_SURGE_AMPLITUDE = 1.45;

/**
 * The surge's amplitude at normal optical depth τ, falling with τ as Déau et al. 2009 found:
 * 1 + 0.45 ÷ (1 + τ), 1.45 as τ → 0, 1.30 at the A ring's τ of 0.5, 1.18 at the B ring's 1.5. The
 * form is this module's lean (2026-11-02), of low confidence: the paper's amplitudes are read only
 * for their trend.
 */
export function surgeAmplitude(tau: number): number {
  return 1 + (SPARSE_SURGE_AMPLITUDE - 1) / (1 + Math.max(tau, 0));
}

/**
 * The opposition surge's factor at phase angle α, rad: 1 + (A(τ) − 1) exp(−α ln 2 ÷ w), the
 * exponential part of Déau et al.'s linear-by-exponential model, A its amplitude and w its
 * half-width at half maximum ({@link SURGE_HALF_WIDTH_RAD}); 1 for a dusty ring.
 */
export function oppositionSurge(alphaRad: number, region: RingRegion, tau: number): number {
  if (region === "dusty") {
    return 1;
  }
  const w = SURGE_HALF_WIDTH_RAD[region];
  return 1 + (surgeAmplitude(tau) - 1) * Math.exp((-Math.max(alphaRad, 0) * Math.LN2) / w);
}

/** A ring's layer and the light on it, each quantity dimensionless. */
export interface RingScatter {
  /** The layer's normal optical depth, at least 0. */
  readonly tau: number;
  /** μ = |sin B|, the viewer's elevation's sine against the ring plane, in [0, 1]. */
  readonly mu: number;
  /** μ₀ = |sin B′|, the star's, in [0, 1]. */
  readonly mu0: number;
  /** ϖ₀, the particles' single-scattering albedo in the channel. */
  readonly albedo: number;
  /** P(α), the particle phase function at the phase angle, normalised over 4π, its surge included. */
  readonly phase: number;
}

/**
 * The lit face's reflectance I/F (Design note 14): ϖ₀ P μ₀ ÷ (4(μ + μ₀)) × [1 − exp(−τ(1/μ +
 * 1/μ₀))] × the shadowing factor; ϖ₀ P ÷ 4 edge-on (μ = 0), 0 with no star (μ₀ = 0).
 *
 * @param shadowing - The shadowing factor of Design note 15 (the table's, R11.T9.b), 1 for the
 *   classical layer.
 */
export function ringReflectance(scatter: RingScatter, shadowing = 1): number {
  const { tau, mu, mu0, albedo, phase } = scatter;
  if (!(mu0 > 0) || !(tau > 0)) {
    return 0;
  }
  // −expm1 keeps the thin layer's 1 − e^(−x) ≈ x, where 1 − exp loses its digits.
  const slant = mu > 0 ? tau * (1 / mu + 1 / mu0) : Number.POSITIVE_INFINITY;
  return ((albedo * phase * mu0) / (4 * (mu + mu0))) * -Math.expm1(-slant) * shadowing;
}

/** Below this |μ − μ₀| ÷ max(μ, μ₀) the unlit face takes its limit's expansion. */
const UNLIT_NEAR = 1e-6;

/**
 * The unlit face's reflectance I/F (Design note 14), the transmitted beam single-scattered:
 * ϖ₀ P μ₀ ÷ (4(μ − μ₀)) × [exp(−τ/μ) − exp(−τ/μ₀)], and at μ → μ₀ its limit ϖ₀ P τ exp(−τ/μ) ÷ 4μ;
 * 0 with no star (μ₀ = 0).
 */
export function ringTransmitted(scatter: RingScatter): number {
  const { tau, mu, mu0, albedo, phase } = scatter;
  if (!(mu0 > 0) || !(tau > 0)) {
    return 0;
  }
  if (Math.abs(mu - mu0) > UNLIT_NEAR * Math.max(mu, mu0)) {
    // e^(−τ/μ) is 0 edge-on (μ = 0), where τ ÷ μ is infinite.
    return (
      ((albedo * phase * mu0) / (4 * (mu - mu0))) * (Math.exp(-tau / mu) - Math.exp(-tau / mu0))
    );
  }
  // ϖ₀ P τ e^(−τ/μ₀) E(x) ÷ 4μ with E(x) = expm1(x) ÷ x and x = τ (μ − μ₀) ÷ (μ μ₀), small here.
  const x = (tau * (mu - mu0)) / (mu * mu0);
  return ((albedo * phase * tau * Math.exp(-tau / mu0)) / (4 * mu)) * (1 + x / 2 + (x * x) / 6);
}

/**
 * Hapke's shadow-hiding opposition term as a ring's shadowing factor, the fallback the shadowing
 * table is measured against (Design note 15): 1 + B₀ ÷ (1 + tan(α/2) ÷ h) with B₀ = 1, h = −(3/8)
 * Y ln(1 − D_eff), Y = √3 ÷ ln W for an R⁻³ layer of radius ratio W, and D_eff = D₀ sin|B| (Salo
 * and French 2010, eq. 18, after Hapke 1986, Icarus 67, 264; the form from memory). It depends on
 * the phase alone through h, and so on the elevation only through D_eff.
 *
 * @param fillingFactor - D₀, the layer's central filling factor, in [0, 1).
 * @param elevationRad - B_eff, the effective elevation, rad.
 * @param radiusRatio - W = R_max ÷ R_min, above 1.
 */
export function hapkeShadowHiding(
  alphaRad: number,
  fillingFactor: number,
  elevationRad: number,
  radiusRatio: number,
): number {
  const effective = fillingFactor * Math.abs(Math.sin(elevationRad));
  const y = Math.sqrt(3) / Math.log(radiusRatio);
  const h = -(3 / 8) * y * Math.log1p(-effective);
  if (!(h > 0)) {
    return 1;
  }
  return 1 + 1 / (1 + Math.tan(Math.max(alphaRad, 0) / 2) / h);
}

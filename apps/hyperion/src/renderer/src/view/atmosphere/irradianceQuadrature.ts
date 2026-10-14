/**
 * The quadrature over the sky's hemisphere that the sky-irradiance table integrates (plan R08,
 * R08.T9.b; Design note 7, _Surface lighting_): `irradiance.wgsl`'s directions and weights, which
 * its twin in `tablesCpu.ts` takes from here.
 *
 * @remarks
 * A texel's irradiance on a horizontal surface is E = ∫ L(ω) μ_v dω over the directions above the
 * horizon, μ_v ≥ 0, L the sky's in-scattered radiance along ω per unit of the sun's illuminance.
 * Bruneton 2017 integrates it from his precomputed scattering texture, 16 × 64 directions a texel,
 * θ and φ every π ÷ 32 over the upper hemisphere (`ComputeIndirectIrradiance` in
 * precomputed_atmospheric_scattering's `functions.glsl`, `SAMPLE_COUNT` 32); here each
 * direction is a march, as the multiple-scattering table's 64 are, so the directions are placed
 * where L varies:
 *
 * - **About the sun.** ω is taken in the sun's frame, its angle γ from the sun and its azimuth ψ
 *   about it, so that a tabulated phase's forward peak lies along γ = 0. γ is integrated in
 *   u = √(γ ÷ π), the phase tables' own coordinate (`medium.ts`'s `PhaseTable`), dense near the
 *   forward direction, where dω = sin γ dγ dψ = 2π u sin(π u²) du dψ.
 * - **At the horizon.** A ring of directions γ from the sun is wholly above the horizon for
 *   γ < γ_a = |π ÷ 2 − θ_s| on the sun's side (for a sun above it), cut by it between γ_a and
 *   π − γ_a, and wholly below beyond (the reverse about the anti-sun for a sun below it). So u is
 *   integrated on two pieces, the whole rings and the cut ones, Gauss–Legendre on each, and each cut
 *   ring only over its arc above the horizon, cos ψ ≥ −μ_s cos γ ÷ (√(1 − μ_s²) sin γ), so that no
 *   node straddles the horizon, where μ_v's kink would cost a rule its order.
 * - **By symmetry.** L is even in ψ about the sun's vertical plane, so ψ runs over [0, ψ_max] and
 *   counts twice.
 *
 * Every angle is taken by direction cosines and `preciseTrig.wgsl`'s forms in the kernel (plan R08,
 * Design note 18), the same forms in `f64` here. A test holds the kernel's constants to
 * {@link gaussLegendre}'s.
 */

import type { Vec3 } from "../../geometry/vec3";

/** A Gauss–Legendre rule on [0, 1]: its nodes t_k, ascending, and weights, which sum to 1. */
export interface UnitGaussRule {
  readonly nodes: Float64Array;
  readonly weights: Float64Array;
}

/**
 * The n-point Gauss–Legendre rule on [0, 1], exact for polynomials of degree 2n − 1: the roots of
 * the Legendre polynomial P_n by Newton's iteration from Tricomi's start cos(π (k − ¼) ÷ (n + ½)),
 * weights 2 ÷ ((1 − x²) P_n′(x)²), mapped from [−1, 1] (Abramowitz and Stegun 1964, §25.4.29;
 * Press et al., _Numerical Recipes_, 3rd ed., §4.6.1).
 *
 * @throws RangeError for n not an integer of at least 1.
 */
export function gaussLegendre(n: number): UnitGaussRule {
  if (!Number.isInteger(n) || n < 1) {
    throw new RangeError(`a Gauss–Legendre rule takes a whole number of nodes, not ${n}`);
  }
  const nodes = new Float64Array(n);
  const weights = new Float64Array(n);
  for (let k = 1; k <= n; k += 1) {
    let x = Math.cos((Math.PI * (k - 0.25)) / (n + 0.5));
    let derivative = 1;
    for (let iteration = 0; iteration < 100; iteration += 1) {
      // P_n(x) in p1 and P_{n−1}(x) in p0, by Bonnet's recurrence.
      let p0 = 1;
      let p1 = x;
      for (let j = 2; j <= n; j += 1) {
        const p2 = ((2 * j - 1) * x * p1 - (j - 1) * p0) / j;
        p0 = p1;
        p1 = p2;
      }
      derivative = (n * (x * p1 - p0)) / (x * x - 1);
      const step = p1 / derivative;
      x -= step;
      if (Math.abs(step) < 1e-16) {
        break;
      }
    }
    // The k-th root descends from near 1; the unit rule's nodes ascend.
    nodes[n - k] = (1 + x) / 2;
    weights[n - k] = 1 / ((1 - x * x) * derivative * derivative);
  }
  return { nodes, weights };
}

/**
 * Gauss–Legendre nodes in u on each of the irradiance texel's two pieces (the whole rings and the
 * cut ones): 8, measured with {@link IRRADIANCE_ARC_NODES} and `tables.ts`'s `IRRADIANCE_SAMPLES`
 * against 24 × 24 nodes and 128 steps (plan R08's Risks, "Deviations in T9.b, as built"): within
 * 0.95% of it on Earth's medium, on Earth's with an aerosol of Henyey–Greenstein g = 0.85 and on
 * the Saturn-class air, from the ground to 2/3 of the top and for suns from the zenith to 6° down.
 */
export const IRRADIANCE_RING_NODES = 8;

/**
 * Gauss–Legendre nodes in ψ on each ring's arc above the horizon: 6, which the same measurement
 * found as close as 8 (4 left 1.0%).
 */
export const IRRADIANCE_ARC_NODES = 6;

/** One direction of a texel's hemisphere and its weight in E = Σ w L. */
export interface IrradianceDirection {
  /**
   * The unit direction in the texel's frame: z its vertical, the sun at (0, √(1 − μ_s²), μ_s), as
   * the multiple-scattering kernel places it.
   */
  readonly direction: Vec3;
  /** Its quadrature weight times its μ_v, sr: Σ w is π for a sun whose half-sky is all lit. */
  readonly weight: number;
}

/** A piece of u: its ends, and whether its rings lie wholly above the horizon. */
interface Piece {
  readonly from: number;
  readonly to: number;
  readonly whole: boolean;
}

/**
 * The two pieces of u = √(γ ÷ π) for a sun at zenith cosine `muSun` (the module's remarks): the
 * whole rings, [0, u_a] for a sun above the horizon and [u_b, 1] below it, then the cut rings
 * [u_a, u_b], u_a = √(γ_a ÷ π) and u_b = √(1 − γ_a ÷ π), γ_a = atan2(|μ_s|, √(1 − μ_s²)).
 */
function piecesOf(muSun: number): readonly [Piece, Piece] {
  const across = Math.sqrt(Math.max((1 - muSun) * (1 + muSun), 0));
  const gammaA = Math.atan2(Math.abs(muSun), across);
  const uA = Math.sqrt(gammaA / Math.PI);
  const uB = Math.sqrt(1 - gammaA / Math.PI);
  const whole: Piece =
    muSun >= 0 ? { from: 0, to: uA, whole: true } : { from: uB, to: 1, whole: true };
  return [whole, { from: uA, to: uB, whole: false }];
}

/**
 * The directions of a texel's hemisphere for a sun at zenith cosine `muSun`, and their weights
 * (the module's remarks), piece by piece, ring by ring, arc node by arc node: 2 × `ringNodes` ×
 * `arcNodes` of them, those of a ring or piece that holds no sky with weight 0 and the sun's
 * direction, as `irradiance.wgsl` takes them.
 *
 * @param muSun - The sun's zenith cosine at the texel, in [−1, 1].
 */
export function irradianceDirections(
  muSun: number,
  ringNodes: number = IRRADIANCE_RING_NODES,
  arcNodes: number = IRRADIANCE_ARC_NODES,
): IrradianceDirection[] {
  const mu = Math.min(Math.max(muSun, -1), 1);
  const across = Math.sqrt(Math.max((1 - mu) * (1 + mu), 0));
  const sun: Vec3 = { x: 0, y: across, z: mu };
  const ring = gaussLegendre(ringNodes);
  const arc = gaussLegendre(arcNodes);
  const out: IrradianceDirection[] = [];
  for (const piece of piecesOf(mu)) {
    const span = piece.to - piece.from;
    for (let i = 0; i < ringNodes; i += 1) {
      const u = piece.from + span * (ring.nodes[i] ?? Number.NaN);
      const gamma = Math.PI * u * u;
      const sinGamma = Math.sin(gamma);
      const cosGamma = Math.cos(gamma);
      // μ_v = A + B cos ψ along the ring.
      const a = mu * cosGamma;
      const b = across * sinGamma;
      let psiMax = Math.PI;
      if (!piece.whole) {
        if (a <= -b) {
          psiMax = 0;
        } else if (a < b) {
          psiMax = Math.atan2(Math.sqrt(Math.max((b - a) * (b + a), 0)), -a);
        }
      }
      // du dγ-measure 2π u sin(π u²), the piece's and the ring's Gauss weights, and the mirror's 2.
      const ringWeight = span * (ring.weights[i] ?? Number.NaN) * 2 * Math.PI * u * sinGamma * 2;
      for (let j = 0; j < arcNodes; j += 1) {
        const psi = psiMax * (arc.nodes[j] ?? Number.NaN);
        const cosPsi = Math.cos(psi);
        const sinPsi = Math.sin(psi);
        // cos γ s + sin γ (cos ψ e₁ + sin ψ e₂), e₁ = (0, −μ_s, √(1 − μ_s²)), e₂ = (1, 0, 0).
        const direction: Vec3 = {
          x: sinGamma * sinPsi,
          y: cosGamma * across - sinGamma * cosPsi * mu,
          z: cosGamma * mu + sinGamma * cosPsi * across,
        };
        const muView = Math.max(direction.z, 0);
        const weight =
          psiMax > 0 ? ringWeight * psiMax * (arc.weights[j] ?? Number.NaN) * muView : 0;
        out.push({ direction: psiMax > 0 ? direction : sun, weight });
      }
    }
  }
  return out;
}

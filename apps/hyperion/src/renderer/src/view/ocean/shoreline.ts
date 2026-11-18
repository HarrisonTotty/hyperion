/**
 * The sea at its shore (plan R11, R11.T8.d; Design note 12: "The shoreline is foam driven by depth
 * and the terrain's slope, amplitude attenuated by depth, and a depth fade, never a depth bias"):
 * the CPU twin, in `f64`, of `shoreline.wgsl`, which the near sea (`oceanNear.wgsl`) composes.
 *
 * @remarks
 * - **Amplitude by depth.** In the surf zone a random sea's height is limited by its depth: H_rms ≤
 *   γ_s d with γ_s = 0.42 (Thornton and Guza 1982, J. Geophys. Res. 87, 9499, their inner surf
 *   zone's measurements, the ratio Thornton and Guza 1983, J. Geophys. Res. 88, 5925, takes), so
 *   every component's amplitude is scaled by min(1, γ_s d ÷ H_rms), H_rms = 2 √(Σ Aᵢ²) the drawn
 *   set's own ({@link shoreAmplitudeScale}), the depth the water's thickness where the near sea
 *   reads it. One scale for every component is a bulk limit (a lean, 2026-11-18): the depth limits
 *   the long components, which carry H_rms, more than the short (TMA, Bouws et al. 1985), and the
 *   wavenumbers stay deep water's, no shoaling or refraction.
 * - **Foam from depth and slope.** The share of the offshore waves broken by a depth d is
 *   exp(−(γ d ÷ H_rms)²), the Rayleigh share of the drawn set's heights above the breaking height
 *   γ d, H_rms the offshore set's, not the depth-limited one: Baldock et al. 1998's Q_b (Coastal
 *   Eng. 34, 173, its explicit form) taken with the offshore height, which rises to 1 at the
 *   waterline where Baldock's local Q_b stays near exp(−(γ ÷ 0.42)²) ≈ 0.05 through the inner surf
 *   zone (a lean, 2026-11-18: the broken share is the foam's, shoaling aside); γ = 0.73, Battjes and
 *   Stive 1985's mean breaker index (J. Geophys. Res. 90, 9159). The breakers' kind is the surf
 *   similarity parameter ξ = tan β ÷ √(H ÷ L₀) (Battjes 1974, Proc. 14th Coastal Eng. Conf., 466),
 *   tan β the bed's slope, L₀ the peak's deep-water wavelength and H the offshore H_rms, the
 *   energy-equivalent regular wave (H_m0 would make ξ 2^¼ smaller): spilling below 0.5, plunging to
 *   3.3, surging above, Battjes' bounds for the offshore height; the foam's share whole for spilling
 *   and falling linearly to none at surging (a lean, 2026-11-18: a surging breaker hardly breaks).
 *   The foam's light is the whitecaps' ({@link shoreFoamShare}), on the setting that draws them.
 * - **The depth fade.** The sea fades to the bed it covers as its water thins: α = smoothstep from
 *   0 to the depth a pixel spans on the beach, max(5 cm, ℓ tan β), ℓ the pixel's reach on the sea
 *   along the bed's slope ({@link shorePixelReachM}; {@link shoreFade}), so that the waterline is
 *   antialiased at the pixel and where the bed's depth and the sea's cannot be told apart the bed
 *   shows, not a band of shimmer: shading, never a depth bias (the brainstorm's depth rules).
 */
/** The inner surf zone's limit of a random sea's height: H_rms ≤ 0.42 d (Thornton and Guza 1983). */
export const SURF_HEIGHT_RATIO = 0.42;

/** The breaker index γ = H_max ÷ d: 0.73, Battjes and Stive 1985's mean (0.6 to 0.83). */
export const BREAKER_INDEX = 0.73;

/** The surf similarity's bounds (Battjes 1974): spilling below 0.5, surging above 3.3. */
export const SURF_SIMILARITY_BOUNDS: readonly [number, number] = [0.5, 3.3];

/** The depth fade's least span, m: 5 cm, a film the bed shows through (a lean, 2026-11-18). */
export const SHORE_FADE_MIN_M = 0.05;

/** A set's root-mean-square wave height, m: 2 √(Σ Aᵢ²), the Rayleigh sea's √8 σ_η. */
export function setRmsHeightM(set: {
  readonly components: ReadonlyArray<{ readonly amplitudeM: number }>;
}): number {
  let sum = 0;
  for (const c of set.components) {
    sum += c.amplitudeM * c.amplitudeM;
  }
  return 2 * Math.sqrt(sum);
}

/**
 * The share of every component's amplitude kept at a water depth of `depthM` (the module's
 * remarks): min(1, γ_s d ÷ H_rms), 1 for a calm sea.
 */
export function shoreAmplitudeScale(depthM: number, rmsHeightM: number): number {
  if (!(rmsHeightM > 0)) {
    return 1;
  }
  return Math.min(Math.max((SURF_HEIGHT_RATIO * Math.max(depthM, 0)) / rmsHeightM, 0), 1);
}

/** The surf similarity parameter ξ = tan β ÷ √(H ÷ L₀) (Battjes 1974); ∞ for a calm sea. */
export function surfSimilarity(
  slopeTan: number,
  rmsHeightM: number,
  deepWavelengthM: number,
): number {
  if (!(rmsHeightM > 0 && deepWavelengthM > 0)) {
    return Number.POSITIVE_INFINITY;
  }
  return Math.abs(slopeTan) / Math.sqrt(rmsHeightM / deepWavelengthM);
}

/**
 * The shore's foam share at a water depth of `depthM` over a bed of slope `slopeTan` (the module's
 * remarks): the breaking share Q_b = exp(−(γ d ÷ H_rms)²) times the breakers' foam by ξ, whole
 * below 0.5 and none above 3.3.
 */
export function shoreFoamShare(
  depthM: number,
  slopeTan: number,
  rmsHeightM: number,
  deepWavelengthM: number,
): number {
  if (!(rmsHeightM > 0)) {
    return 0;
  }
  const ratio = (BREAKER_INDEX * Math.max(depthM, 0)) / rmsHeightM;
  const breaking = Math.exp(-ratio * ratio);
  const xi = surfSimilarity(slopeTan, rmsHeightM, deepWavelengthM);
  const [spilling, surging] = SURF_SIMILARITY_BOUNDS;
  const kind = Math.min(Math.max((surging - xi) / (surging - spilling), 0), 1);
  return breaking * kind;
}

/**
 * A pixel's reach on the sea along the bed's slope, m (the module's remarks): its footprint, an
 * ellipse t δ across the view and t δ ÷ μ_v along it, μ_v the view's cosine to the sea's up, at
 * least `minCosine`, measured along the slope's horizontal direction, whose cosine to the view's is
 * `slopeAlongView`.
 *
 * @param distanceM - The sea's distance along the pixel's ray, t.
 * @param pixelAngleRad - The pixel's angle, δ.
 */
export function shorePixelReachM(
  distanceM: number,
  pixelAngleRad: number,
  viewCosine: number,
  slopeAlongView: number,
  minCosine: number,
): number {
  const along = slopeAlongView / Math.max(Math.abs(viewCosine), minCosine);
  const across = Math.sqrt(Math.max(1 - slopeAlongView * slopeAlongView, 0));
  return distanceM * pixelAngleRad * Math.hypot(along, across);
}

/**
 * The near sea's opacity at its shore (the module's remarks): smoothstep from 0 to the depth a pixel
 * spans on a bed of slope `slopeTan`, max(5 cm, ℓ tan β), at the water's depth `depthM`, ℓ the
 * pixel's reach along the slope ({@link shorePixelReachM}).
 */
export function shoreFade(depthM: number, footprintM: number, slopeTan: number): number {
  const span = Math.max(SHORE_FADE_MIN_M, footprintM * Math.abs(slopeTan));
  const x = Math.min(Math.max(depthM / span, 0), 1);
  return x * x * (3 - 2 * x);
}

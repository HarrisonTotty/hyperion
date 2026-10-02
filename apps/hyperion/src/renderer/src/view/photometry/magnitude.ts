/**
 * From a star's magnitude to the luminance of the pixels its light falls in (plan R02, Design
 * note 10; brainstorm, "Luminance in physical units, and an exposure model").
 *
 * @remarks
 * Apparent V from absolute V and distance, with no extinction until R06; illuminance from apparent
 * V; the light spread over a 7 × 7 quad of pixels by a pixel-integrated Gaussian point-spread
 * function; and each pixel's luminance its share of the illuminance over its own solid angle, so
 * that point sources brighten with resolution while discs do not.
 */

/**
 * The illuminance of a star of apparent visual magnitude 0 outside the atmosphere, lx: 2.54 µlx.
 *
 * @remarks
 * Allen's value, Cox (ed.), _Allen's Astrophysical Quantities_, 4th ed. (2000), §15 (Crumey cites
 * the edition as Cox 1999); Crumey, "Human contrast threshold and astronomical visibility", MNRAS
 * 442 (2014) 2600, §2, which takes the same zero point. Cross-checked by the Sun: V = −26.76 (Willmer, ApJS 236,
 * 47, 2018) gives 1.28 × 10⁵ lx, the solar illuminance at 1 au.
 */
export const V0_ILLUMINANCE_LX = 2.54e-6;

/** One parsec, m: 648,000 ÷ π au, the au being 149,597,870,700 m (IAU 2012 Resolution B2; IAU 2015 B2). */
export const PARSEC_M = (648_000 / Math.PI) * 149_597_870_700;

/**
 * The standard deviation of the point-spread function, px: 0.64, a full width at half maximum of
 * 1.5 px (2√(2 ln 2) × 0.64 = 1.507).
 *
 * @remarks
 * Plan R02, Design note 10 (researched 2026-09-29): wide enough that a star's displayed total stays
 * within about ±0.08 mag as it moves across a pixel, so that a turning camera cannot make it flash.
 */
export const PSF_SIGMA_PX = 0.64;

/**
 * The side of the square of pixels a star's light is spread over, px: 7, ±3 about its pixel.
 *
 * @remarks
 * Plan R02, Design note 10's point-spread function: at σ = {@link PSF_SIGMA_PX}, ±3.5 px about the
 * star's pixel centre holds all but 2.8 × 10⁻⁶ of the light, at worst, with the star at its pixel's
 * edge, 3 px (4.7 σ) from the quad's near side in each axis.
 */
export const PSF_QUAD_PX = 7;

/**
 * A star's apparent V magnitude at a distance: V = M_V + 5 log₁₀(d ÷ 10 pc), with no extinction.
 *
 * @param absoluteV - The absolute V magnitude, M_V.
 * @param distanceM - The distance, m, positive.
 */
export function apparentV(absoluteV: number, distanceM: number): number {
  return absoluteV + 5 * Math.log10(distanceM / (10 * PARSEC_M));
}

/**
 * The illuminance of a star of apparent V magnitude, lx: 2.54 µlx × 10^(−0.4 V).
 *
 * @remarks
 * V tracks photopic illuminance to within 0.08 mag from O5 to M6 (the brainstorm, after Pickles
 * 1998 against the CIE 1924 curve), so the V magnitude stands for the star's visible light.
 */
export function illuminanceLx(apparentVMag: number): number {
  return V0_ILLUMINANCE_LX * 10 ** (-0.4 * apparentVMag);
}

/**
 * A pixel's luminance from a star, cd/m²: its share of the star's illuminance over the pixel's
 * solid angle, E × weight ÷ Ω.
 *
 * @param illuminanceLxValue - The star's illuminance, lx.
 * @param psfWeight - The pixel's share of the light, from {@link psfPixelWeights}.
 * @param pixelSolidAngleSr - The pixel's solid angle, sr (the camera's `pixelSolidAngle`).
 */
export function pixelLuminance(
  illuminanceLxValue: number,
  psfWeight: number,
  pixelSolidAngleSr: number,
): number {
  return (illuminanceLxValue * psfWeight) / pixelSolidAngleSr;
}

/** Continued-fraction erfc for x ≥ 2.5 (modified Lentz), to about 5 × 10⁻¹⁵ relative. */
function erfcLarge(x: number): number {
  // erfc x = e^(−x²)/√π · 1/(x + (1/2)/(x + 1/(x + (3/2)/(x + 2/(x + …))))).
  const tiny = 1e-300;
  let f = x;
  let c = x;
  let d = 0;
  for (let n = 1; n < 200; n += 1) {
    const a = n / 2;
    d = x + a * d;
    d = Math.abs(d) < tiny ? tiny : d;
    c = x + a / c;
    c = Math.abs(c) < tiny ? tiny : c;
    d = 1 / d;
    const delta = c * d;
    f *= delta;
    if (Math.abs(delta - 1) < 1e-16) {
      break;
    }
  }
  return Math.exp(-x * x) / (Math.sqrt(Math.PI) * f);
}

/**
 * The error function, to about 10⁻¹⁴: its Maclaurin series below 2.5, the continued fraction of
 * its complement above.
 */
export function erf(x: number): number {
  const ax = Math.abs(x);
  if (ax >= 2.5) {
    return Math.sign(x) * (1 - erfcLarge(ax));
  }
  // erf x = 2/√π Σ (−1)ⁿ x^(2n+1) ÷ (n! (2n + 1)).
  let term = ax;
  let sum = ax;
  const x2 = ax * ax;
  for (let n = 1; n < 100; n += 1) {
    term *= -x2 / n;
    const next = term / (2 * n + 1);
    sum += next;
    if (Math.abs(next) < 1e-17 * Math.abs(sum)) {
      break;
    }
  }
  return (Math.sign(x) * 2 * sum) / Math.sqrt(Math.PI);
}

/** The fraction of a unit Gaussian of σ `sigmaPx`, centred at `centrePx`, that falls in [a, b]. */
function share(aPx: number, bPx: number, centrePx: number, sigmaPx: number): number {
  const s = Math.SQRT2 * sigmaPx;
  return (erf((bPx - centrePx) / s) - erf((aPx - centrePx) / s)) / 2;
}

/** Where a star falls within its pixel: offsets from the pixel's centre, px, each in [−0.5, 0.5]. */
export interface SubpixelOffset {
  /** Rightward offset, px. */
  readonly xPx: number;
  /** Downward offset, px. */
  readonly yPx: number;
}

/**
 * Each pixel's share of a star's light over the {@link PSF_QUAD_PX} square about its pixel, from a
 * pixel-integrated Gaussian point-spread function (plan R02, Design note 10).
 *
 * @remarks
 * Each weight is the product of differences of the error function across the pixel's edges in x
 * and in y, so that the weights are exact integrals at any sub-pixel position, and they are
 * normalised over the quad (whose tails beyond ±3.5 px hold at most 2.8 × 10⁻⁶ of the light at σ =
 * 0.64 px) so that they sum to 1: a star's total never changes as it moves across pixels.
 *
 * @returns `PSF_QUAD_PX²` weights, row by row from the top left; the star's own pixel is the
 * centre.
 */
export function psfPixelWeights(subpixel: SubpixelOffset, sigmaPx = PSF_SIGMA_PX): Float64Array {
  const half = (PSF_QUAD_PX - 1) / 2;
  const across: number[] = [];
  const down: number[] = [];
  for (let i = -half; i <= half; i += 1) {
    across.push(share(i - 0.5, i + 0.5, subpixel.xPx, sigmaPx));
    down.push(share(i - 0.5, i + 0.5, subpixel.yPx, sigmaPx));
  }
  const total = across.reduce((sum, w) => sum + w, 0) * down.reduce((sum, w) => sum + w, 0);
  const weights = new Float64Array(PSF_QUAD_PX * PSF_QUAD_PX);
  for (const [row, wy] of down.entries()) {
    for (const [column, wx] of across.entries()) {
      weights[row * PSF_QUAD_PX + column] = (wx * wy) / total;
    }
  }
  return weights;
}

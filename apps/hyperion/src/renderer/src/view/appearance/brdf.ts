/**
 * The body BRDF as reflectance I/F, the TypeScript reference of `shaders/litBody.wgsl`'s
 * `body_brdf` (plan R07, Design note 5).
 *
 * @remarks
 * I/F = A · f(α) · [L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀] per display channel, with f read from the
 * law's 0.5° table by linear interpolation, exactly as the shader reads it. A point's radiance is
 * (I/F) × E ÷ π for an illuminance E face-on to the light.
 */
import type { Rgb } from "../photometry/toneCurve";
import { phaseFactorFromTable, phaseFactorTableOf, type PhotometricLaw } from "./law";

/**
 * The reflectance I/F of a point under the law.
 *
 * @param mu0 - Cosine of the incidence angle; the point is unlit at 0 or below.
 * @param mu - Cosine of the emission angle; the point is hidden at 0 or below.
 * @param phaseRad - Phase angle, rad, 0 to π.
 */
export function brdf(law: PhotometricLaw, mu0: number, mu: number, phaseRad: number): Rgb {
  if (mu0 <= 0 || mu <= 0) {
    return [0, 0, 0];
  }
  const share = law.lommelSeeligerShare;
  const disc = (share * 2 * mu0) / (mu0 + mu) + (1 - share) * mu0;
  const [r, g, b] = phaseFactorFromTable(phaseFactorTableOf(law), phaseRad);
  return [law.a[0] * r * disc, law.a[1] * g * disc, law.a[2] * b * disc];
}

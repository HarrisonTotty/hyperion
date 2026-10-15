/**
 * The body BRDF as reflectance I/F, the TypeScript reference of `shaders/litBody.wgsl`'s
 * `body_brdf` (plan R07, Design note 5).
 *
 * @remarks
 * I/F = A · f(α) · [L(α) · 2μ₀ ÷ (μ₀ + μ) + (1 − L(α)) μ₀] per display channel, with f and L(α)
 * read from the law's 0.5° table by linear interpolation, exactly as the shader reads its row (f in
 * r, g and b, L(α) in a; decision-r07-t8b). A point's radiance is (I/F) × E ÷ π for an illuminance E
 * face-on to the light.
 */
import type { Rgb } from "../photometry/toneCurve";
import {
  limbShareFromTable,
  phaseFactorFromTable,
  type PhaseFactorTable,
  phaseFactorTableOf,
  type PhotometricLaw,
} from "./law";

/**
 * The reflectance I/F of a point under the law, f from the law's own table.
 *
 * @param mu0 - Cosine of the incidence angle; the point is unlit at 0 or below.
 * @param mu - Cosine of the emission angle; the point is hidden at 0 or below.
 * @param phaseRad - Phase angle, rad, 0 to π.
 */
export function brdf(law: PhotometricLaw, mu0: number, mu: number, phaseRad: number): Rgb {
  return brdfFromTable(law, phaseFactorTableOf(law), mu0, mu, phaseRad);
}

/**
 * The reflectance I/F with f and L(α) read from a given table, as the shader reads the row a law
 * names.
 *
 * @remarks
 * The row is the law's own (one row per class law, R10.T10.b; decision-r07-t8b): its f is
 * tabulated against its L(α), which the row's alpha carries, so only `law`'s A is read here; a
 * texel of several classes sums each class's I/F through its own row, never one law of weighted
 * parameters.
 */
export function brdfFromTable(
  law: Pick<PhotometricLaw, "a">,
  table: PhaseFactorTable,
  mu0: number,
  mu: number,
  phaseRad: number,
): Rgb {
  if (mu0 <= 0 || mu <= 0) {
    return [0, 0, 0];
  }
  const share = limbShareFromTable(table, phaseRad);
  const disc = (share * 2 * mu0) / (mu0 + mu) + (1 - share) * mu0;
  const [r, g, b] = phaseFactorFromTable(table, phaseRad);
  return [law.a[0] * r * disc, law.a[1] * g * disc, law.a[2] * b * disc];
}

/**
 * A Sun-like host disc built as R06's `host_discs` builds one, for kept scenes and tests (plan R07,
 * T8.a; decision-r07-t8a, item 1).
 *
 * @remarks
 * The photopic mean luminance follows from the absolute V and the radius, L̄ = 2.54 µlx ×
 * `lux_per_v0` × 10^(−0.4 M_V) × (10 pc)² ÷ (π R²), split by an illustrative warm white of unit
 * Rec. 709 luminance, not R06's colour table's (a stated departure until a fixture from R06's
 * `host_discs` exists). The defaults are the Sun: M_V 4.81 (Willmer 2018, ApJS 236, 47, Table 3),
 * R 6.957 × 10⁸ m (IAU 2015 Resolution B3), T_eff 5,772 K, log g 4.438, `lux_per_v0` 1 (R06's
 * table gives the Sun 1 within 0.1), and V-band limb darkening c 0.7837, α 0.6893 in all three
 * channels: Claret and Southworth 2022 (A&A 664, A128, Table 3, Z = 0, ξ = 2 km/s) at the grid's
 * log g 4.5, as R06's test pins it (at the Sun's own 4.438 α is 0.6884, R06's built table's).
 */
import type { HostDiscDto } from "@hyperion/protocol";

import { PARSEC_M, V0_ILLUMINANCE_LX } from "../photometry/magnitude";

/** The Sun's absolute V magnitude, Willmer 2018, ApJS 236, 47, Table 3. */
export const SUN_ABSOLUTE_V = 4.81;

/** The Sun's nominal radius, m (IAU 2015 Resolution B3). */
export const SUN_RADIUS_M = 6.957e8;

/** An illustrative warm white of unit Rec. 709 luminance (r, g, b), not R06's table's. */
const SUN_COLOUR_RGB = [1.08, 0.99, 0.863_4] as const;

/** The Sun's V-band power-2 limb darkening at log g 4.5 (Claret and Southworth 2022, Table 3). */
const SUN_LIMB = { c: 0.7837, alpha: 0.6893 } as const;

/**
 * A host star's disc as R06 sends it, a Sun unless overridden; `absoluteV` sets the luminance.
 */
export function sunLikeHostDisc(
  overrides: Partial<HostDiscDto> & { readonly absoluteV?: number } = {},
): HostDiscDto {
  const { absoluteV = SUN_ABSOLUTE_V, ...dto } = overrides;
  const radius = dto.radius_m ?? SUN_RADIUS_M;
  const luxPerV0 = dto.lux_per_v0 ?? 1;
  const tenParsecs = 10 * PARSEC_M;
  const mean =
    (V0_ILLUMINANCE_LX * luxPerV0 * 10 ** (-0.4 * absoluteV) * tenParsecs * tenParsecs) /
    (Math.PI * radius * radius);
  const [r, g, b] = SUN_COLOUR_RGB;
  const disc = 1 - (SUN_LIMB.c * SUN_LIMB.alpha) / (SUN_LIMB.alpha + 2);
  return {
    star: 0,
    radius_m: radius,
    teff_k: 5_772,
    log_g: 4.438,
    mean_luminance_cd_m2: [mean * b, mean * g, mean * r],
    central_luminance_cd_m2: [(mean * b) / disc, (mean * g) / disc, (mean * r) / disc],
    limb: [{ ...SUN_LIMB }, { ...SUN_LIMB }, { ...SUN_LIMB }],
    chroma: [r, g],
    lux_per_v0: luxPerV0,
    bake_spectrum: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    ...dto,
  };
}

/**
 * A host disc frozen through, its arrays and limb laws with it, for a kept scene that holds one
 * disc in every frame: an edit in place, which would reach every frame after it, throws instead.
 *
 * @returns `disc` itself, frozen.
 */
export function frozenHostDisc(disc: HostDiscDto): HostDiscDto {
  for (const law of disc.limb) {
    Object.freeze(law);
  }
  for (const value of Object.values(disc)) {
    if (Array.isArray(value)) {
      Object.freeze(value);
    }
  }
  return Object.freeze(disc);
}

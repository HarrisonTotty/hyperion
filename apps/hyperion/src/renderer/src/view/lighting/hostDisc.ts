/**
 * A Sun-like host disc built as R06's `host_discs` builds one, for kept scenes and tests (plan R07,
 * T8.a; decision-r07-t8a, item 1), with R06's solar photometry (plan R08, R08.T6.j).
 *
 * @remarks
 * The photopic mean luminance follows from the absolute V and the radius, L̄ = 2.54 µlx ×
 * `lux_per_v0` × 10^(−0.4 M_V) × (10 pc)² ÷ (π R²), split by the star's linear Rec. 709 colour of
 * unit luminance. The defaults are the Sun: M_V 4.81 (Willmer 2018, ApJS 236, 47, Table 3),
 * R 6.957 × 10⁸ m (IAU 2015 Resolution B3), T_eff 5,772 K, log g 4.438, R06's colour,
 * `lux_per_v0` and bake spectrum of the Sun ({@link SUN_COLOUR_RGB}, {@link SUN_LUX_PER_V0},
 * {@link SUN_BAKE_SPECTRUM}), and V-band limb darkening c 0.7837, α 0.6893 in all three channels:
 * Claret and Southworth 2022 (A&A 664, A128, Table 3, Z = 0, ξ = 2 km/s) at the grid's log g 4.5,
 * as R06's test pins it (at the Sun's own 4.438 α is 0.6884, R06's built table's).
 *
 * The Sun at 1 au is then 133,462 lx, ASTM E-490's own photopic 133,318 lx to 0.11%
 * (`solar.ts`'s `E490_ILLUMINANCE_LX`). Until R08.T6.j it took an illustrative warm white
 * (1.08, 0.99, 0.8634) and `lux_per_v0` 1, R02's 2.54 µlx for a V = 0 star whatever its spectrum,
 * a hot star's figure (the ruling's "Vega's"): 128,731 lx, 3.4% under E-490's, R06's 133,462 lx
 * being 3.67% above it (`decision-r08-t6d-findings.md` §5).
 */
import type { HostDiscDto } from "@hyperion/protocol";

import { PARSEC_M, V0_ILLUMINANCE_LX } from "../photometry/magnitude";
import type { Rgb } from "../photometry/toneCurve";
import { CHANNEL_LUMINANCE } from "./illuminance";

/** The Sun's absolute V magnitude, Willmer 2018, ApJS 236, 47, Table 3. */
export const SUN_ABSOLUTE_V = 4.81;

/** The Sun's nominal radius, m (IAU 2015 Resolution B3). */
export const SUN_RADIUS_M = 6.957e8;

/**
 * R06's colour of the Sun's light, linear Rec. 709 (r, g, b) at unit luminance:
 * `hyperion_sim::sky::colour::solar_colour()`, the colour table (`tables/star_colour.rs`, ATLAS9,
 * Castelli and Kurucz 2003) at 5,772 K and log g 4.438 by `star_colour`'s bilinear rule.
 *
 * @remarks
 * Its red and green are the chroma as `HostDiscDto.chroma` carries it (`f32`), 1.0844254 and
 * 0.9815597: the Sun's row of `packages/protocol/fixtures/bake_spectra.json`, which `hyperion-fit`'s
 * `star_colour` test holds to the table and `hostDisc.test.ts` holds this to. Its blue follows from
 * R06's luminance row ({@link CHANNEL_LUMINANCE}) as R06's `blue()` gives it: 0.9340067, which is
 * `blue()`'s 0.9340063 from the unrounded red and green to 5 × 10⁻⁷, the wire's rounding.
 */
export const SUN_COLOUR_RGB: Rgb = ((): Rgb => {
  const red = 1.084_425_4;
  const green = 0.981_559_7;
  const [yr, yg, yb] = CHANNEL_LUMINANCE;
  return [red, green, (1 - yr * red - yg * green) / yb];
})();

/**
 * R06's photopic illuminance of a V = 0 star of the Sun's spectrum over R02's 2.54 µlx:
 * `solar_colour().lux_per_v0()`, as the Sun's row of `bake_spectra.json` holds it.
 *
 * @remarks
 * R02's 2.54 µlx (Allen's, Cox 2000; Crumey 2014, §2) takes no account of the spectrum, and is a
 * hot star's lux per unit of V flux: R06's table gives an A0 spectrum 1.006 of it. The Johnson V
 * passband and the photopic V(λ) differ in width and centre, so the lux per unit of V flux depends
 * on the spectrum; this is the Sun's correction, from ATLAS9 (Castelli and Kurucz 2003), about
 * 3.0% for the Sun's spectrum against an A0's and 0.6% for the zero point below an A0's. It puts
 * the Sun (M_V 4.81 at 1 au, V −26.762) at 133,462 lx, E-490's own 133,318 lx to 0.11%, and about
 * 0.5% above E-490 scaled to the IAU 2015 nominal total solar irradiance, 1,361 W m⁻² (E-490 is
 * scaled to 1,366.1 W m⁻²); nothing is rescaled for it (`decision-r08-t6d-findings.md` §5).
 */
export const SUN_LUX_PER_V0 = 1.036_746_866_278_959_8;

/**
 * R06's bake spectrum of the Sun: `solar_colour().bake_spectrum()`, its mean spectral irradiance
 * over the 15 bake bins per lux of photopic illuminance, W m⁻² nm⁻¹ lx⁻¹, as the Sun's row of
 * `bake_spectra.json` holds it.
 */
export const SUN_BAKE_SPECTRUM: Readonly<HostDiscDto["bake_spectrum"]> = Object.freeze<
  HostDiscDto["bake_spectrum"]
>([
  8.960915207343157e-6, 1.2362490278026274e-5, 1.3612987280831015e-5, 1.5485020042778694e-5,
  1.4690473858970552e-5, 1.3958072723148161e-5, 1.4076616230965863e-5, 1.381707381874496e-5,
  1.3363324889932911e-5, 1.2474684459518065e-5, 1.1775456367565396e-5, 1.1371777803239652e-5,
  1.0711433533074792e-5, 1.001879082420593e-5, 9.483862860417408e-6,
]);

/** The Sun's V-band power-2 limb darkening at log g 4.5 (Claret and Southworth 2022, Table 3). */
const SUN_LIMB = { c: 0.7837, alpha: 0.6893 } as const;

/**
 * A host star's disc as R06 sends it, the Sun unless overridden; `absoluteV` sets the luminance, and
 * `colourRgb`, a linear Rec. 709 colour of unit luminance, the channels' split of it and the chroma
 * in place of R06's solar colour.
 */
export function sunLikeHostDisc(
  overrides: Partial<HostDiscDto> & {
    readonly absoluteV?: number;
    readonly colourRgb?: Rgb;
  } = {},
): HostDiscDto {
  const { absoluteV = SUN_ABSOLUTE_V, colourRgb = SUN_COLOUR_RGB, ...dto } = overrides;
  const radius = dto.radius_m ?? SUN_RADIUS_M;
  const luxPerV0 = dto.lux_per_v0 ?? SUN_LUX_PER_V0;
  const tenParsecs = 10 * PARSEC_M;
  const mean =
    (V0_ILLUMINANCE_LX * luxPerV0 * 10 ** (-0.4 * absoluteV) * tenParsecs * tenParsecs) /
    (Math.PI * radius * radius);
  const [r, g, b] = colourRgb;
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
    bake_spectrum: [...SUN_BAKE_SPECTRUM],
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

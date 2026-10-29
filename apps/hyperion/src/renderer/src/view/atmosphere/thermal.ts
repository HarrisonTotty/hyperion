/**
 * The ground's thermal emission (plan R08, R08.T18.a; decision-composition §1.9): Planck's law at
 * the 15 bake bins, the ground's emissivity by Kirchhoff's law, and the reduction to the render
 * channels, which R07's disc (`discReflectance.ts`) and R10's terrain (`terrainLit.wgsl`'s emissive
 * form) both call; the temperature field a body's record gives the ground; and the visibility cut
 * below which nothing is computed.
 *
 * @remarks
 * **Units.** The renderer's HDR target holds pre-exposed luminance: each channel is a linear
 * Rec. 709 luminance in cd m⁻² times the frame's exposure scale (R02's `exposureScale`). A star's
 * light reaches a disc as an illuminance per channel, lx, R06's photopic illuminance times the
 * star's linear Rec. 709 colour at unit luminance (`lighting/illuminance.ts`), and a surface's I/F
 * turns it into luminance as I/F × E ÷ π: R07's records carry the exposure ÷ π for it
 * (`DiscRecord.exposureOverPi`). The emission is absolute, so it takes no star: a black body's
 * spectral radiance B_λ(T), W m⁻² sr⁻¹ nm⁻¹, becomes a luminance per channel as
 * K_cd Σ c̄(λ) ε(λ) B_λ(T) Δλ, c̄ the CIE 1931 2° matching functions taken to linear Rec. 709
 * (`spectralColour.ts`' {@link MATCHING_FUNCTIONS}, whose Rec. 709 luminance row is ȳ) and
 * K_cd = 683 lm W⁻¹ ({@link LUMINOUS_EFFICACY_LM_PER_W}), so that its Rec. 709 luminance is the
 * photopic luminance 683 ∫ ȳ ε B dλ, the scale R06 builds the stars' channels in. The frame's
 * exposure scale then multiplies it, as π × `exposureOverPi` multiplies the reflected light.
 *
 * **Spectrally**, B_λ is taken at the 15 bake bins' centres ({@link BAKE_WAVELENGTHS_NM}, the
 * wavelengths every off-frame bake solves at), each bin's emissivity is constant, and each bin's
 * value reaches the channels through the colour pipeline's rows in it: the 1 nm rows of 380–760 nm
 * with their trapezoid weights, each in its bin as `binConversion.ts`' `binOfRow` puts it, so that
 * the weight of bin b in channel c is V_cb = K_cd Σ c̄_c(λ) Δλ over its rows
 * ({@link EMISSION_CHANNEL_WEIGHTS}), the conversion R08.T14.b's bakes take with a flat spectrum in
 * place of the star's. Against Planck's law times the matching functions integrated over their
 * whole table at 0.1 nm, 360–830 nm, the bins read 0.14%, 0.31% and 1.51% high in r, g and b at
 * 2,500 K, and 2.2%, 18.8% and 2.7% at 1,000 K, where green and blue are a few per cent of red and
 * the colour is out of the Rec. 709 gamut (`thermal.test.ts`): the bins' 25 nm steps across B's
 * Wien slope, and the 760–830 nm the bake range leaves out, 0.13% of red at 1,000 K.
 *
 * **Kirchhoff's law** gives the ground's emissivity in each bin as 1 − A, A its albedo there by the
 * bake's ground rule (`bake.ts`' `binGroundAlbedo`: linear in wavelength between the render
 * channels' wavelengths and held at the nearer channel outside them), each held to [0, 1]. A is the
 * disc's or the terrain's law's A per channel, c A_N scaled for the figure, which stands for the
 * hemispherical albedo that Kirchhoff's law takes: the two differ little for the dark surfaces hot
 * enough to glow (R08's Risks, "Deviations in T18.a, as built").
 *
 * **The temperature field** ({@link SurfaceTemperatureField}) is built from the record's surface
 * section as the wire carries it at generator version 23 (`BodySurface`, P14.T24.a, P14.T51.c):
 * - **A world locked 1:1** to its seasonal host (`climateRegime.forcing` `locked`) takes its
 *   eyeball about the substellar point, σT⁴ = σT_n⁴ + (σT_ss⁴ − σT_n⁴) max(μ, 0), μ the cosine
 *   from the substellar point: the sim's own dayside flux E_n + (3/2)(E_d − E_n) μ and night E_n
 *   (Pierrehumbert 2011, ApJ 726, L8; Cowan and Agol 2011, ApJ 729, 54, whose day and night means
 *   the profile keeps), so T_ss⁴ = T_n⁴ + (3/2)(T_d⁴ − T_n⁴), which an airless world's night floor
 *   makes T_ss cos^¼θ above it. T_d and T_n are Cowan and Agol's eqs. 4–5 as the sim scales them to
 *   the surface, T_s (8/3 − 5ε/3)^¼ and T_s ε^¼, the redistribution ε solved from the record's
 *   mean T_s and its day–night contrast T_d − T_n, which a locked world's are about the substellar
 *   axis, undamped; the profile's emission mean is then T_s, as the record's is. Its equator–pole
 *   contrast, the sim's zonal model's, is not added: the eyeball already holds its poles on the
 *   terminator.
 * - **Any other world** takes a zonal profile about its pole with a day–night term about its
 *   brightest star: T = T_s + T₂ P₂(sin φ) + ΔT_dn μ, T₂ = −(2/3)(T(0°) − T(90°)), the signed
 *   equator–pole contrast shaped as P₂, as rendering plan R09 imposes it on its annual field
 *   (R09 Design note 8; North 1975, J. Atmos. Sci. 32, 2033, the two-mode solution), and ΔT_dn
 *   shaped as P₁, whose hemispheres differ by it. Both have no area mean, so T_s is the profile's
 *   area mean, 1.3 K below the record's emission mean for Earth-like contrasts.
 * - **The melt** (`crust.melt_area_fraction` f, P14.T51.c): the sim melts the area above its lid's
 *   transition of that profile (`hooks/crust.rs`' `melt_area`), so the profile already holds the
 *   melt there at its own temperature, above the transition; and the transition is the profile's
 *   value at the melt's edge: T_sol⁴ = T_n⁴ + (1 − 2f)(T_ss⁴ − T_n⁴) on a locked world (the sim's
 *   cap of μ above 1 − 2f), and the zonal profile at sin φ = f, or at 1 − f where the poles are the
 *   warmer. The terrain's `Melt` texels, and a disc's `Melt` classes, emit at no less than it
 *   ({@link SurfaceTemperatureField.meltK}). A `magma_ocean` surface is molten everywhere (its melt
 *   area is 1), so the whole surface is held at no less than the dry peridotite solidus the sim
 *   holds it at, 1,394 K (Hirschmann 2000; {@link SurfaceTemperatureField.floorK}).
 * - A body with a medium (any volatile air, or a `tenuous` haze) is R08.T18.b's, whose ground is
 *   seen through it ({@link groundTemperatureFieldOf}).
 *
 * **The cut** (decision-composition §1.9): a body whose emitted luminance is nowhere above
 * {@link THERMAL_CUT} of its reflected luminance in any channel computes nothing
 * ({@link aboveThermalCut}): its draws are the kernels' plain forms, byte for byte. The emitted is
 * the field's hottest point's at the least A of the body's laws, the reflected the brightest law's
 * A face-on to its stars, E A ÷ π, never below {@link REFLECTED_FLOOR_CD_M2}.
 */

import type { BodySurface } from "../../lib/system/model";
import { SPEED_OF_LIGHT_M_PER_S } from "../../lib/scene/lightTime";
import type { Rgb } from "../photometry/toneCurve";
import {
  BAKE_BIN_COUNT,
  BAKE_RANGE_NM,
  BAKE_WAVELENGTHS_NM,
  CHANNEL_WAVELENGTHS_NM,
} from "./medium";
import { BOLTZMANN_J_PER_K } from "./rayleigh";
import { MATCHING_FUNCTIONS, PIPELINE_WAVELENGTHS_NM } from "./spectralColour";
import thermalWgsl from "./shaders/thermal.wgsl?raw";

/**
 * The Planck constant h, J s: exact in the 2019 SI, as are c and k_B (BIPM, _The International
 * System of Units_, 9th edition, 2019, Table 1; CODATA 2018, Tiesinga et al., Rev. Mod. Phys. 93
 * (2021) 025010). c is `lib/scene/lightTime.ts`' {@link SPEED_OF_LIGHT_M_PER_S} and k_B
 * `rayleigh.ts`' {@link BOLTZMANN_J_PER_K}.
 */
export const PLANCK_J_S = 6.626_070_15e-34;

/**
 * The luminous efficacy K_cd of monochromatic radiation of 540 × 10¹² Hz, lm W⁻¹: 683, exact in the
 * 2019 SI's definition of the candela (BIPM 2019, §2.3.1), the factor of the photopic luminance
 * 683 ∫ ȳ L_e,λ dλ (CIE 015:2018 gives K_m = 683.002 lm W⁻¹ at ȳ's peak, 3 × 10⁻⁶ above it), as
 * R05's `solar.ts` and the sim's `sky::eye` take it.
 */
export const LUMINOUS_EFFICACY_LM_PER_W = 683;

/** Nanometres per metre. */
const NM_PER_M = 1e9;

/**
 * Planck's second radiation constant c₂ = h c ÷ k_B, nm K: 1.438 776 877 × 10⁷, the exponent
 * h c ÷ (λ k_B T) = c₂ ÷ (λ T) with λ in nm.
 */
export const SECOND_RADIATION_NM_K =
  ((PLANCK_J_S * SPEED_OF_LIGHT_M_PER_S) / BOLTZMANN_J_PER_K) * NM_PER_M;

/**
 * 2 h c² in the units that give B_λ in W m⁻² sr⁻¹ nm⁻¹ for λ in nm, W m⁻² sr⁻¹ nm⁴:
 * 2 h c² × 10⁴⁵ (m⁵ to nm⁵) × 10⁻⁹ (per m to per nm) = 1.191 042 972 × 10²⁰.
 */
const FIRST_RADIATION_W_NM4_PER_M2_SR =
  2 * PLANCK_J_S * SPEED_OF_LIGHT_M_PER_S * SPEED_OF_LIGHT_M_PER_S * NM_PER_M ** 4;

/**
 * A black body's spectral radiance B_λ(T) at a vacuum wavelength, W m⁻² sr⁻¹ nm⁻¹: Planck's law,
 * 2 h c² λ⁻⁵ ÷ (e^(h c ÷ λ k_B T) − 1).
 *
 * @param temperatureK - Positive; 0 gives 0.
 */
export function planckRadiance(wavelengthNm: number, temperatureK: number): number {
  if (!(temperatureK > 0)) {
    return 0;
  }
  const x = SECOND_RADIATION_NM_K / (wavelengthNm * temperatureK);
  return FIRST_RADIATION_W_NM4_PER_M2_SR / wavelengthNm ** 5 / Math.expm1(x);
}

/** B_λ(T) at each bake bin's centre ({@link BAKE_WAVELENGTHS_NM}), W m⁻² sr⁻¹ nm⁻¹. */
export function planckBins(temperatureK: number): Float64Array {
  return Float64Array.from(BAKE_WAVELENGTHS_NM, (nm) => planckRadiance(nm, temperatureK));
}

/**
 * The ground rule's weights in each bake bin (the module's remarks): A_b = Σ_c g_bc A_c, by
 * channel r, g, b, linear in wavelength between the two render channels about the bin's centre and
 * the nearer channel's alone outside them, as `bake.ts`' `binGroundAlbedo` blends them.
 */
export const GROUND_RULE_WEIGHTS: ReadonlyArray<Rgb> = BAKE_WAVELENGTHS_NM.map((nm): Rgb => {
  const [red, green, blue] = CHANNEL_WAVELENGTHS_NM;
  if (nm >= red) {
    return [1, 0, 0];
  }
  if (nm <= blue) {
    return [0, 0, 1];
  }
  if (nm >= green) {
    const w = (nm - green) / (red - green);
    return [w, 1 - w, 0];
  }
  const w = (nm - blue) / (green - blue);
  return [0, w, 1 - w];
});

/** A ground rule's weights dotted with a per-channel quantity. */
function ruled(weights: Rgb, values: Rgb): number {
  return weights[0] * values[0] + weights[1] * values[1] + weights[2] * values[2];
}

/**
 * The ground's emissivity in each bake bin by Kirchhoff's law, 1 − A, A the albedo there by the
 * ground rule ({@link GROUND_RULE_WEIGHTS}), held to [0, 1].
 *
 * @param albedo - The ground's A per render channel (r, g, b).
 */
export function kirchhoffEmissivity(albedo: Rgb): Float64Array {
  return Float64Array.from(
    GROUND_RULE_WEIGHTS,
    (weights) => 1 - Math.min(Math.max(ruled(weights, albedo), 0), 1),
  );
}

/**
 * The bin of a pipeline row, nm: ⌊15 (λ − 380) ÷ 380⌋, the last bin for 760 nm, as
 * `binConversion.ts`' `binOfRow` takes it; written here so that the render thread, which draws the
 * emission, loads none of the thick bake's modules (`AtmosphereCache.test.ts`).
 */
function binOfRow(wavelengthNm: number): number {
  const [low, high] = BAKE_RANGE_NM;
  const bin = Math.floor((BAKE_BIN_COUNT * (wavelengthNm - low)) / (high - low));
  return Math.min(Math.max(bin, 0), BAKE_BIN_COUNT - 1);
}

/**
 * Each bake bin's weight in each render channel (the module's remarks), lm W⁻¹ nm:
 * V_cb = K_cd Σ c̄_c(λ) Δλ over the colour pipeline's 1 nm rows in bin b, with their trapezoid
 * weights, so that Σ_b V_cb X_b is the luminance in cd m⁻² of a radiance X_b W m⁻² sr⁻¹ nm⁻¹
 * constant in each bin.
 */
export const EMISSION_CHANNEL_WEIGHTS: ReadonlyArray<Rgb> = (() => {
  const sums = Array.from({ length: BAKE_BIN_COUNT }, () => [0, 0, 0]);
  const offset = (PIPELINE_WAVELENGTHS_NM[0] ?? Number.NaN) - MATCHING_FUNCTIONS.firstNm;
  const last = PIPELINE_WAVELENGTHS_NM.length - 1;
  for (const [i, nm] of PIPELINE_WAVELENGTHS_NM.entries()) {
    const trapezoid = i === 0 || i === last ? 0.5 : 1;
    const bin = sums[binOfRow(nm)];
    if (bin === undefined) {
      throw new Error(`the pipeline's row at ${nm} nm has no bin`);
    }
    bin[0] = (bin[0] ?? 0) + (MATCHING_FUNCTIONS.red[offset + i] ?? Number.NaN) * trapezoid;
    bin[1] = (bin[1] ?? 0) + (MATCHING_FUNCTIONS.green[offset + i] ?? Number.NaN) * trapezoid;
    bin[2] = (bin[2] ?? 0) + (MATCHING_FUNCTIONS.blue[offset + i] ?? Number.NaN) * trapezoid;
  }
  return sums.map(([r = 0, g = 0, b = 0]): Rgb => [
    LUMINOUS_EFFICACY_LM_PER_W * r,
    LUMINOUS_EFFICACY_LM_PER_W * g,
    LUMINOUS_EFFICACY_LM_PER_W * b,
  ]);
})();

/**
 * The luminance per render channel of a ground of the given emissivity per bake bin at
 * `temperatureK`, cd m⁻² (linear Rec. 709, before the exposure): Σ_b V_cb ε_b B_λ(λ_b, T).
 *
 * @param emissivity - ε per bake bin, 15 values ({@link kirchhoffEmissivity}).
 */
export function emittedLuminance(emissivity: ArrayLike<number>, temperatureK: number): Rgb {
  const planck = planckBins(temperatureK);
  const out = [0, 0, 0];
  for (const [b, weights] of EMISSION_CHANNEL_WEIGHTS.entries()) {
    const radiance = (emissivity[b] ?? Number.NaN) * (planck[b] ?? Number.NaN);
    for (const c of [0, 1, 2] as const) {
      out[c] = (out[c] ?? 0) + weights[c] * radiance;
    }
  }
  return [out[0] ?? 0, out[1] ?? 0, out[2] ?? 0];
}

/**
 * The exponent c₂ ÷ (λ T) past which a bin's term is dropped by {@link thermalRadiance} and its
 * kernel: there e^(−80) = 1.8 × 10⁻³⁵ of 2 h c² λ⁻⁵, at most 10⁻²³ cd m⁻² in any channel, and still
 * a normal `f32`, so that the kernel never takes `exp` past `f32`'s range.
 */
export const THERMAL_EXPONENT_MAX = 80;

/** Each bin's c₂ ÷ λ_b, K: its exponent at 1 K. */
const BIN_EXPONENT_K: ReadonlyArray<number> = BAKE_WAVELENGTHS_NM.map(
  (nm) => SECOND_RADIATION_NM_K / nm,
);

/** Each bin's V_cb × 2 h c² λ_b⁻⁵, cd m⁻², the luminance per channel of 1 ÷ (e^x − 1) in it. */
const BIN_LUMINANCE_CD_M2: ReadonlyArray<Rgb> = EMISSION_CHANNEL_WEIGHTS.map((weights, b): Rgb => {
  const scale = FIRST_RADIATION_W_NM4_PER_M2_SR / (BAKE_WAVELENGTHS_NM[b] ?? Number.NaN) ** 5;
  return [weights[0] * scale, weights[1] * scale, weights[2] * scale];
});

/**
 * The shared function (decision-composition §1.9): the luminance per render channel, cd m⁻² before
 * the exposure, that a share of a surface emits at `temperatureK`, its laws' A summed by their
 * shares: Σ_b V_cb ε_b B_λ(λ_b, T) × share, with ε_b share = share − A_b held to [0, share], A_b the
 * ground rule of `albedoSum` ({@link GROUND_RULE_WEIGHTS}), so that its shares' laws need not be
 * divided out. `thermal.wgsl`'s `thermal_radiance` is it in `f32`, operation for operation: each
 * bin's 1 ÷ (e^x − 1) as e^(−x) ÷ (1 − e^(−x)), and none past {@link THERMAL_EXPONENT_MAX}.
 *
 * @param albedoSum - Σ share_m A_m per render channel over the laws it emits for.
 * @param share - Σ share_m, in [0, 1].
 * @param temperatureK - Positive.
 */
export function thermalRadiance(albedoSum: Rgb, share: number, temperatureK: number): Rgb {
  const out = [0, 0, 0];
  for (const [b, weights] of GROUND_RULE_WEIGHTS.entries()) {
    const x = (BIN_EXPONENT_K[b] ?? Number.NaN) / temperatureK;
    if (!(x <= THERMAL_EXPONENT_MAX)) {
      continue;
    }
    const e = Math.exp(-x);
    const planck = e / (1 - e);
    const emissivity = Math.min(Math.max(share - ruled(weights, albedoSum), 0), share);
    const luminance = BIN_LUMINANCE_CD_M2[b];
    if (luminance === undefined) {
      throw new Error(`bin ${b} has no luminance`);
    }
    for (const c of [0, 1, 2] as const) {
      out[c] = (out[c] ?? 0) + luminance[c] * (emissivity * planck);
    }
  }
  return [out[0] ?? 0, out[1] ?? 0, out[2] ?? 0];
}

/** An `f32` constant of WGSL, written in full so that the compiler rounds it once. */
function wgslNumber(value: number): string {
  if (!Number.isFinite(value)) {
    throw new Error(`a thermal constant is not finite: ${value}`);
  }
  return value.toExponential();
}

/**
 * The tables `thermal.wgsl` reads, as WGSL constants: each bin's c₂ ÷ λ_b, its luminance per
 * channel of 1 ÷ (e^x − 1), its ground rule's weights, and {@link THERMAL_EXPONENT_MAX}.
 */
function thermalConstantsWgsl(): string {
  const vectors = (rows: ReadonlyArray<Rgb>): string =>
    rows.map((row) => `vec3f(${row.map(wgslNumber).join(", ")})`).join(",\n  ");
  return [
    `// The thermal emission's tables (\`thermal.ts\`, R08.T18.a), written from its constants.`,
    `const THERMAL_BINS : u32 = ${BAKE_BIN_COUNT}u;`,
    `const THERMAL_EXPONENT_MAX : f32 = ${wgslNumber(THERMAL_EXPONENT_MAX)};`,
    `const THERMAL_BIN_EXPONENT_K = array<f32, ${BAKE_BIN_COUNT}>(\n  ${BIN_EXPONENT_K.map(wgslNumber).join(",\n  ")},\n);`,
    `const THERMAL_BIN_LUMINANCE = array<vec3f, ${BAKE_BIN_COUNT}>(\n  ${vectors(BIN_LUMINANCE_CD_M2)},\n);`,
    `const THERMAL_GROUND_RULE = array<vec3f, ${BAKE_BIN_COUNT}>(\n  ${vectors(GROUND_RULE_WEIGHTS)},\n);`,
    "",
  ].join("\n");
}

/**
 * The shared function's WGSL (`shaders/thermal.wgsl`, after its tables): `thermal_radiance` and the
 * temperature field's `thermal_field_temperature`, with no binding, composed into the disc's and
 * the terrain's emissive forms alone.
 */
export const THERMAL_WGSL: string = thermalConstantsWgsl() + thermalWgsl;

/**
 * The least temperature a field gives, K: the cosmic microwave background's 2.72548 K (Fixsen,
 * ApJ 707 (2009) 916), the sim's night floor's own least, so that a zonal profile's cold end stays
 * positive.
 */
export const FIELD_LEAST_K = 2.725_48;

/**
 * The dry peridotite solidus at the surface, K: 1,394 (Hirschmann 2000, G³ 1, 1042), at or above
 * which the sim holds a molten surface (`derive/atmosphere.rs`' `SILICATE_SOLIDUS`).
 */
export const SILICATE_SOLIDUS_K = 1_394;

/** A locked world's eyeball about its substellar point (the module's remarks). */
export interface SubstellarProfile {
  readonly kind: "substellar";
  /** T_ss, K: the substellar point's. */
  readonly substellarK: number;
  /** T_n, K: the night's, and the terminator's. */
  readonly nightK: number;
}

/** A rotating world's zonal profile with its day–night term (the module's remarks). */
export interface ZonalProfile {
  readonly kind: "zonal";
  /** T_s, K: the profile's area mean. */
  readonly meanK: number;
  /** T(0°) − T(90°), K, signed. */
  readonly equatorPoleK: number;
  /** The day's less the night's, K, not negative. */
  readonly dayNightK: number;
}

/** The temperature of a body's ground (the module's remarks). */
export interface SurfaceTemperatureField {
  readonly profile: SubstellarProfile | ZonalProfile;
  /** The least temperature anywhere, K: a molten surface's solidus, else {@link FIELD_LEAST_K}. */
  readonly floorK: number;
  /** The least temperature of the melt, K, its lid's transition; `null` with no melt. */
  readonly meltK: number | null;
}

/** Cowan and Agol's (2011) day and night, as the sim scales them, ÷ T_s at redistribution ε. */
function dayNightOf(epsilon: number): readonly [number, number] {
  return [(8 / 3 - (5 * epsilon) / 3) ** 0.25, epsilon ** 0.25];
}

/**
 * The redistribution ε whose day and night (`dayNightOf`) differ by `contrast` × T_s: by
 * bisection, the difference falling from (8/3)^¼ at ε = 0 to 0 at 1; 0 for a contrast past the
 * first, 1 for none.
 */
function redistributionOf(contrast: number): number {
  let low = 0;
  let high = 1;
  for (let i = 0; i < 64; i += 1) {
    const mid = (low + high) / 2;
    const [day, night] = dayNightOf(mid);
    if (day - night > contrast) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return (low + high) / 2;
}

/**
 * A surface's temperature field (the module's remarks): a locked world's eyeball, or any other's
 * zonal profile, with its melt's least temperature and a molten surface's floor.
 */
export function surfaceTemperatureFieldOf(surface: BodySurface): SurfaceTemperatureField {
  const meanK = surface.meanSurfaceTemperatureK;
  const melt = surface.crust.kind === "value" ? surface.crust.value.meltAreaFraction : 0;
  const floorK = surface.state === "magma_ocean" ? SILICATE_SOLIDUS_K : FIELD_LEAST_K;
  if (surface.climateRegime.forcing === "locked") {
    const [day, night] = dayNightOf(redistributionOf(surface.dayNightContrastK / meanK));
    const dayK = day * meanK;
    const nightK = night * meanK;
    const night4 = nightK ** 4;
    const span4 = 1.5 * (dayK ** 4 - night4);
    const substellarK = (night4 + span4) ** 0.25;
    // The cap of μ above 1 − 2f is the melt, its edge the transition (the sim's eyeball).
    const meltK = melt > 0 ? (night4 + Math.max(1 - 2 * melt, 0) * span4) ** 0.25 : null;
    return { profile: { kind: "substellar", substellarK, nightK }, floorK, meltK };
  }
  const equatorPoleK = surface.equatorPoleContrastK;
  // The melt's edge where the area above it is f: a band about the equator where it is the
  // warmer, caps about the poles where they are.
  const edge = equatorPoleK >= 0 ? melt : 1 - melt;
  const meltK = melt > 0 ? meanK - (equatorPoleK / 3) * (3 * edge * edge - 1) : null;
  return {
    profile: { kind: "zonal", meanK, equatorPoleK, dayNightK: surface.dayNightContrastK },
    floorK,
    meltK,
  };
}

/**
 * The field of a body whose ground this task draws, or `null` for one with a medium, R08.T18.b's
 * (the module's remarks): an `airless` surface, or one with no volatile air that is not `tenuous`,
 * as a melt surface whose volatiles are gone.
 */
export function groundTemperatureFieldOf(surface: BodySurface): SurfaceTemperatureField | null {
  const noMedium =
    surface.state === "airless" || (surface.gases.length === 0 && surface.state !== "tenuous");
  return noMedium ? surfaceTemperatureFieldOf(surface) : null;
}

/**
 * A field's profile as `thermal.wgsl`'s `thermal_field_temperature` reads it: 1, T_n⁴ and
 * T_ss⁴ − T_n⁴ (K⁴) for a locked world's eyeball, and 0; or 2, T_s, (T(0°) − T(90°)) ÷ 3 and the
 * day–night contrast (K) for a zonal profile.
 */
export function packTemperatureField(
  field: SurfaceTemperatureField,
): readonly [number, number, number, number] {
  const { profile } = field;
  let packed: readonly [number, number, number, number];
  switch (profile.kind) {
    case "substellar": {
      const night4 = profile.nightK ** 4;
      packed = [1, night4, profile.substellarK ** 4 - night4, 0];
      break;
    }
    case "zonal":
      packed = [2, profile.meanK, profile.equatorPoleK / 3, profile.dayNightK];
      break;
  }
  return packed;
}

/**
 * A field's temperature at a point, K, before the melt's: its profile's, held to its floor, by
 * `thermal_field_temperature`'s arithmetic on {@link packTemperatureField}'s numbers.
 *
 * @param cosSubstellar - The cosine of the point's angle from the brightest star's direction, μ.
 * @param sinLatitude - The sine of its latitude about the body's pole.
 */
export function fieldTemperatureK(
  field: SurfaceTemperatureField,
  cosSubstellar: number,
  sinLatitude: number,
): number {
  const [kind, y, z, w] = packTemperatureField(field);
  const t =
    kind < 1.5
      ? Math.sqrt(Math.sqrt(y + z * Math.max(cosSubstellar, 0)))
      : y - z * (3 * sinLatitude * sinLatitude - 1) + w * cosSubstellar;
  return Math.max(t, field.floorK);
}

/** A field's hottest temperature, K: its profile's peak, held to its floor and its melt's. */
export function fieldPeakK(field: SurfaceTemperatureField): number {
  const { profile } = field;
  let peak: number;
  switch (profile.kind) {
    case "substellar":
      peak = profile.substellarK;
      break;
    case "zonal":
      // P₂'s range is [−½, 1]: the equator is the hotter end where the contrast is positive.
      peak =
        profile.meanK +
        Math.max(profile.equatorPoleK / 3, (-2 * profile.equatorPoleK) / 3) +
        Math.abs(profile.dayNightK);
      break;
  }
  return Math.max(peak, field.floorK, field.meltK ?? 0);
}

/**
 * The share of a body's reflected luminance its emitted luminance must exceed in some channel for
 * it to be drawn: 10⁻⁶ (decision-composition §1.9).
 */
export const THERMAL_CUT = 1e-6;

/**
 * The least reflected luminance the cut compares with, cd m⁻²: 10⁻⁵, the foot of the brainstorm's
 * scene span, the galactic band's faintest background (`exposure.ts`' `MAN_EV100_MIN`), so that a
 * body no star lights is drawn only where its own light would show in a scene (lean, 2026-10-29).
 */
export const REFLECTED_FLOOR_CD_M2 = 1e-5;

/**
 * Whether a body's ground emits more than {@link THERMAL_CUT} of what it reflects in some channel
 * (the module's remarks): its field's peak ({@link fieldPeakK}) emitted at each law's A against the
 * brightest law's A face-on to its stars, E_c A_c ÷ π summed over the stars, held to
 * {@link REFLECTED_FLOOR_CD_M2}.
 *
 * @param albedos - The A per render channel of each of the laws its surface shades with.
 * @param illuminancesLx - Each star's illuminance face-on at the body per channel, lx.
 */
export function aboveThermalCut(
  field: SurfaceTemperatureField,
  albedos: ReadonlyArray<Rgb>,
  illuminancesLx: ReadonlyArray<Rgb>,
): boolean {
  const peakK = fieldPeakK(field);
  return ([0, 1, 2] as const).some((c) => {
    const brightest = Math.max(0, ...albedos.map((a) => a[c]));
    const lit = illuminancesLx.reduce((sum, e) => sum + e[c], 0);
    const reflected = Math.max((brightest * lit) / Math.PI, REFLECTED_FLOOR_CD_M2);
    const emitted = Math.max(...albedos.map((a) => thermalRadiance(a, 1, peakK)[c]));
    return emitted > THERMAL_CUT * reflected;
  });
}

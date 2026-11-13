/**
 * A sea's liquid as the ocean draws it (plan R11, R11.T8.a; Design note 12;
 * decision-r11-palette-liquids §1.9): the palette entry's liquid record, read at the three render
 * channels, with the optics every ocean pass shares.
 *
 * @remarks
 * Pure: no state, no clock, `f64` throughout, the CPU reference for `oceanFar.wgsl` and
 * `oceanNear.wgsl`.
 *
 * - **The record is the one source.** The index, absorption, scattering, density, surface tension
 *   and viscosity are the server's record (`PaletteEntryDto.liquid`, resolved for the body at its
 *   temperature; decision-r11-palette-liquids §1.7), never constants of this module. Design note
 *   12's four indices (water 1.333, seawater 1.339, liquid methane about 1.29, ethane about 1.37)
 *   retire as the registry's test values ({@link LIQUID_INDEX_CHECKS}), with ethane corrected to
 *   about 1.40 at Titan's 94 K (decision-r11-palette-liquids §1.9, finding 2).
 * - **Bins to channels.** Each by-bin member is read at {@link CHANNEL_WAVELENGTHS_NM}, linear in
 *   wavelength between the bake bins' centres and held at the end bins' values outside them, as the
 *   records were reduced from their files (linear in n and in a; `testing/liquids.rs`).
 * - **Not modelled is absent.** A member the record lacks is `null` here and named in
 *   {@link OceanLiquid.missing}, so that a console labels it Missing; a record whose basis is not
 *   `measured` is {@link OceanLiquid.approximate}. Nothing stands in for an absent member: without
 *   absorption the column is drawn opaque, its surface's reflection alone, and without scattering
 *   the deep term is 0, Beer–Lambert alone (main's ruling for R09.T2.e, 2026-10-15).
 * - **No salt, no particles.** The record is the pure liquid's: Earth's sea is `H2O`, so its index
 *   is about 0.006 below seawater's and its scattering about 30% below (decision-r11-palette-liquids
 *   §5), and suspended and dissolved matter and life are not modelled.
 */
import type { LiquidBasisDto, LiquidDto } from "@hyperion/protocol";

import { BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM } from "../atmosphere/medium";
import type { Rgb } from "../photometry/toneCurve";

/** A member of a liquid record that may be absent, by the name a console labels. */
export type LiquidMember = "absorption" | "scattering" | "surface-tension" | "viscosity";

/** A sea's liquid at the render channels (red, green, blue), from its palette record. */
export interface OceanLiquid {
  /** The temperature every member is resolved at, K. */
  readonly temperatureK: number;
  readonly densityKgM3: number;
  /** n relative to vacuum, per channel. */
  readonly refractiveIndex: Rgb;
  /** a, Napierian, m⁻¹, per channel; `null` where not modelled. */
  readonly absorptionPerM: Rgb | null;
  /** b over 4π, m⁻¹, per channel, its backscattering b ÷ 2; `null` where not modelled. */
  readonly scatteringPerM: Rgb | null;
  /** γ, N/m; `null` where not modelled. */
  readonly surfaceTensionNPerM: number | null;
  /** μ, dynamic, Pa s; `null` where not modelled. */
  readonly viscosityPaS: number | null;
  readonly basis: LiquidBasisDto;
  /** The members the record lacks, in {@link LiquidMember}'s order. */
  readonly missing: ReadonlyArray<LiquidMember>;
  /** Whether the record's basis is other than `measured`: derived or a stand-in. */
  readonly approximate: boolean;
}

/**
 * Design note 12's four indices, kept as the registry's test values (decision-r11-palette-liquids
 * §4, P14.T49.h's tests): n near 589 nm, each with the temperature it is quoted at, K.
 *
 * @remarks
 * Water 1.333 at 20 °C (IAPWS R9-97, 1.33299 at 589.3 nm and 293.15 K); seawater 1.339, S = 35 at
 * 20 °C (Quan and Fry 1995, Appl. Opt. 34, 3477); liquid methane about 1.29 near 111 K
 * (Martonchik and Orton 1994, Appl. Opt. 33, 8306, about 1.294 at 92 K by Lorentz–Lorenz); ethane
 * about 1.40 at 94 K, Lorentz–Lorenz from a molar refraction of about 11.3 cm³ mol⁻¹ at 651.5 kg m⁻³
 * (decision-r11-palette-liquids §1.8), not Design note 12's 1.37, which is near 130 K. The sources
 * could not be re-read; the figures are the ruling's, as it states them.
 */
export const LIQUID_INDEX_CHECKS: ReadonlyArray<{
  readonly liquid: "water" | "seawater" | "methane" | "ethane";
  readonly refractiveIndex: number;
  readonly temperatureK: number;
}> = [
  { liquid: "water", refractiveIndex: 1.333, temperatureK: 293.15 },
  { liquid: "seawater", refractiveIndex: 1.339, temperatureK: 293.15 },
  { liquid: "methane", refractiveIndex: 1.29, temperatureK: 111 },
  { liquid: "ethane", refractiveIndex: 1.4, temperatureK: 94 },
];

/**
 * A by-bin member at `wavelengthNm`: linear in wavelength between the two bake bins' centres about
 * it ({@link BAKE_WAVELENGTHS_NM}), the end bin's value outside them.
 *
 * @param bins - One value a bake bin, shortest first.
 */
export function binValueAt(bins: ReadonlyArray<number>, wavelengthNm: number): number {
  const first = BAKE_WAVELENGTHS_NM[0];
  const last = BAKE_WAVELENGTHS_NM[BAKE_WAVELENGTHS_NM.length - 1];
  if (first === undefined || last === undefined || bins.length !== BAKE_WAVELENGTHS_NM.length) {
    throw new RangeError(
      `a liquid member has ${BAKE_WAVELENGTHS_NM.length} bins, not ${bins.length}`,
    );
  }
  if (!(wavelengthNm > first)) {
    return bins[0] ?? Number.NaN;
  }
  if (!(wavelengthNm < last)) {
    return bins[bins.length - 1] ?? Number.NaN;
  }
  const step = (last - first) / (bins.length - 1);
  const at = (wavelengthNm - first) / step;
  const below = Math.min(Math.floor(at), bins.length - 2);
  const t = at - below;
  const lower = bins[below] ?? Number.NaN;
  const upper = bins[below + 1] ?? Number.NaN;
  return lower + (upper - lower) * t;
}

/** A by-bin member at each render channel ({@link CHANNEL_WAVELENGTHS_NM}). */
export function binsAtChannels(bins: ReadonlyArray<number>): Rgb {
  return [
    binValueAt(bins, CHANNEL_WAVELENGTHS_NM[0]),
    binValueAt(bins, CHANNEL_WAVELENGTHS_NM[1]),
    binValueAt(bins, CHANNEL_WAVELENGTHS_NM[2]),
  ];
}

/** A member that may be absent, `null` for absent or `null`. */
function present<T>(value: T | null | undefined): T | null {
  return value ?? null;
}

/**
 * A palette entry's liquid record at the render channels.
 *
 * @throws RangeError where a by-bin member does not hold one value a bake bin.
 * @param record - A record the coarse-field store took (`coarseFieldStore.ts` checks every member).
 */
export function oceanLiquidOf(record: LiquidDto): OceanLiquid {
  const absorption = present(record.absorption_per_m);
  const scattering = present(record.scattering_per_m);
  const tension = present(record.surface_tension_n_per_m);
  const viscosity = present(record.viscosity_pa_s);
  const missing: LiquidMember[] = [];
  if (absorption === null) {
    missing.push("absorption");
  }
  if (scattering === null) {
    missing.push("scattering");
  }
  if (tension === null) {
    missing.push("surface-tension");
  }
  if (viscosity === null) {
    missing.push("viscosity");
  }
  return {
    temperatureK: record.temperature_k,
    densityKgM3: record.density_kg_m3,
    refractiveIndex: binsAtChannels(record.refractive_index),
    absorptionPerM: absorption === null ? null : binsAtChannels(absorption),
    scatteringPerM: scattering === null ? null : binsAtChannels(scattering),
    surfaceTensionNPerM: tension,
    viscosityPaS: viscosity,
    basis: record.basis,
    missing,
    approximate: record.basis !== "measured",
  };
}

/**
 * The Fresnel reflectance of unpolarised light on a smooth surface of index `n` (relative to the
 * medium above it, taken as vacuum: air's n − 1 is under 10⁻³), at incidence cosine
 * `cosIncidence` in [0, 1]: the exact dielectric form, the mean of the s and p reflectances
 * (Cook and Torrance 1982, ACM TOG 1, 7, eq. for F with g = √(n² + c² − 1)).
 */
export function fresnelReflectance(n: number, cosIncidence: number): number {
  const c = Math.min(Math.max(cosIncidence, 0), 1);
  const g = Math.sqrt(n * n - 1 + c * c);
  const a = (g - c) / (g + c);
  const b = (c * (g + c) - 1) / (c * (g - c) + 1);
  return 0.5 * a * a * (1 + b * b);
}

/** The Fresnel reflectance at normal incidence, ((n − 1) ÷ (n + 1))². */
export function normalReflectance(n: number): number {
  const r = (n - 1) / (n + 1);
  return r * r;
}

/**
 * The coefficient of Morel and Prieur's deep irradiance reflectance, R = 0.33 b_b ÷ (a + b_b)
 * (Morel and Prieur 1977, Limnol. Oceanogr. 22, 709; Gordon, Brown and Jacobs 1975's first-order
 * term, 0.3244), as decision-r11-palette-liquids §1.9 rules it.
 */
export const DEEP_REFLECTANCE_COEFFICIENT = 0.33;

/**
 * The deep column's irradiance reflectance just beneath the surface, R(0−) =
 * 0.33 b_b ÷ (a + b_b), b_b = b ÷ 2 (the pure liquid's symmetric phase function), per channel; 0
 * where scattering is not modelled (Beer–Lambert alone), `null` where absorption is not, whose
 * column is drawn opaque.
 */
export function deepReflectance(liquid: OceanLiquid): Rgb | null {
  const a = liquid.absorptionPerM;
  if (a === null) {
    return null;
  }
  const b = liquid.scatteringPerM;
  if (b === null) {
    return [0, 0, 0];
  }
  const one = (ac: number, bc: number): number => {
    const backward = bc / 2;
    return (DEEP_REFLECTANCE_COEFFICIENT * backward) / (ac + backward);
  };
  return [one(a[0], b[0]), one(a[1], b[1]), one(a[2], b[2])];
}

/**
 * The upwelling radiance's factor Q, sr: E_u ÷ L_u beneath the surface, π for an isotropic
 * upwelling field (a lean, 2026-11-02; measured Q is 3–5 sr in the open sea, Morel and Gentili
 * 1993, Appl. Opt. 32, 6864), so that the deep term is not brightened by a sun-angle model the
 * record cannot support.
 */
export const UPWELLING_Q_SR = Math.PI;

/**
 * The column's remote-sensing reflectance, sr⁻¹: the water-leaving radiance per unit of the
 * downwelling irradiance above the surface, t_down t_up R ÷ (n² Q), each transmittance 1 − F at
 * its own cosine (the radiance's n² law across the surface; Mobley 1994, "Light and Water", §10.4's
 * form), per channel.
 *
 * @param reflectance - R(0−) per channel: {@link deepReflectance}, or a shallow column's
 *   ({@link shallowReflectance}).
 * @param cosSun - The sun's incidence cosine on the mean surface.
 * @param cosView - The view's cosine on the mean surface.
 */
export function remoteSensingReflectance(
  liquid: OceanLiquid,
  reflectance: Rgb,
  cosSun: number,
  cosView: number,
): Rgb {
  const one = (k: 0 | 1 | 2): number => {
    const n = liquid.refractiveIndex[k];
    const down = 1 - fresnelReflectance(n, cosSun);
    const up = 1 - fresnelReflectance(n, cosView);
    return (down * up * reflectance[k]) / (n * n * UPWELLING_Q_SR);
  };
  return [one(0), one(1), one(2)];
}

/**
 * The cosine of a beam's direction beneath the surface, Snell's law from incidence cosine
 * `cosIncidence` into index `n`.
 */
export function refractedCosine(n: number, cosIncidence: number): number {
  const c = Math.min(Math.max(cosIncidence, 0), 1);
  return Math.sqrt(1 - (1 - c * c) / (n * n));
}

/**
 * The downwelling irradiance's diffuse attenuation, m⁻¹, K_d ≈ (a + b_b) ÷ μ_d, μ_d the refracted
 * sun's cosine (Gordon 1989, Limnol. Oceanogr. 34, 1389's first-order form), per channel; `null`
 * where absorption is not modelled.
 */
export function diffuseAttenuationPerM(liquid: OceanLiquid, cosSun: number): Rgb | null {
  const a = liquid.absorptionPerM;
  if (a === null) {
    return null;
  }
  const b = liquid.scatteringPerM ?? [0, 0, 0];
  const one = (k: 0 | 1 | 2): number =>
    (a[k] + b[k] / 2) / refractedCosine(liquid.refractiveIndex[k], cosSun);
  return [one(0), one(1), one(2)];
}

/**
 * A shallow column's irradiance reflectance over a bottom of albedo A at depth H, Maritorena, Morel
 * and Gentili 1994 (Limnol. Oceanogr. 39, 1689): R(H) = R∞ + (A − R∞) e^(−2 K_d H), per
 * channel.
 *
 * @param deep - R∞, {@link deepReflectance}.
 * @param attenuationPerM - K_d, {@link diffuseAttenuationPerM}.
 */
export function shallowReflectance(
  deep: Rgb,
  bottomAlbedo: Rgb,
  depthM: number,
  attenuationPerM: Rgb,
): Rgb {
  const one = (k: 0 | 1 | 2): number =>
    deep[k] + (bottomAlbedo[k] - deep[k]) * Math.exp(-2 * attenuationPerM[k] * Math.max(depthM, 0));
  return [one(0), one(1), one(2)];
}

/**
 * The column's two-way Beer–Lambert transmittance from the surface to the bed `thicknessM` along
 * the view and back to the sun, e^(−a (t ÷ μ_v′ + t ÷ μ_s′)), each cosine the refracted one, per
 * channel; `null` where absorption is not modelled, an opaque column.
 */
export function columnTransmittance(
  liquid: OceanLiquid,
  thicknessM: number,
  cosView: number,
  cosSun: number,
): Rgb | null {
  const a = liquid.absorptionPerM;
  if (a === null) {
    return null;
  }
  const t = Math.max(thicknessM, 0);
  const one = (k: 0 | 1 | 2): number => {
    const n = liquid.refractiveIndex[k];
    const path = t / refractedCosine(n, cosView) + t / refractedCosine(n, cosSun);
    return Math.exp(-a[k] * path);
  };
  return [one(0), one(1), one(2)];
}

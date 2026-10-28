/**
 * The spectral column a thick bake solves (plan R08, R08.T14.b; Design notes 5 and 9): a drawn
 * medium's terms each with the source behind it, so that a bake bin's optics come from those
 * sources at the bin's own wavelength, never from the medium's three channels.
 *
 * @remarks
 * `assemble.ts`'s `assembleMedium` gives a medium in the three render channels only, and builds the
 * physical sources behind its terms privately. The bake takes them as plain data that a structured
 * clone carries: each term's name and density profile as drawn, and the source its kind needs to
 * give coefficients and a phase function at any wavelength.
 *
 * - **molecular:** the well-mixed gas's fractions and its number density at relative density 1,
 *   n_s, through `rayleigh.ts`'s {@link molecularMixture}: σ_s = n_s σ_mix(λ), the Rayleigh phase
 *   at ρ_mix(λ) (`molecularTerm`'s rule at any λ);
 * - **thomson:** the electrons' number density at relative density 1: σ_s = n_e σ_T, Rayleigh's
 *   phase at ρ = 0 (`absorbers.ts`'s `thomsonTerm`);
 * - **aerosol:** the mode's particles and its extinction at 550 nm at relative density 1, through
 *   `aerosol.ts`'s {@link aerosolModeOptics} and {@link aerosolPhaseTable}: σ_ext(λ) =
 *   σ_ext,550 σ_ext(λ) ÷ σ_ext(550) per particle, ω(λ), and the mode's own √θ phase table at λ
 *   (`aerosolTerm`'s scale and table, at any λ);
 * - **absorber:** the term's amount at relative density 1 (molecules m⁻³, or molecule pairs m⁻⁶);
 *   its absorption is each light's curves of growth in the bins (`absorbers.ts`'s
 *   `absorberBinCurves`), which the bake takes beside the column, matched to the term by order.
 * - **channels** (R08.T14.c; `decision-r08-t13c2-classes.md` §6): a kept scene's term that has no
 *   physical source, such as `TEST EARTH`'s Cornette–Shanks aerosol: its own coefficients and phase
 *   in the three render channels, carried to any wavelength by the ground rule
 *   ({@link channelBlendAt}, `bake.ts`'s `binGroundAlbedo`'s): linear in wavelength between the
 *   channels' wavelengths and held at the nearer channel outside them. Where one channel holds
 *   alone its phase is that channel's own; where two blend, their Legendre moments blend
 *   ({@link SpectralPhase}'s `moments`).
 *
 * {@link spectralColumnOf} lays the sources beside a drawn medium's terms, from
 * `columnLayout.ts`, which imports none of the optics, so that the render thread forms a bake's
 * column (R08.T14.c). Mie, the MMF and the phase files run where the column's optics are taken,
 * in the optics worker, never on the render thread: this module imports the optics, and the
 * render thread only types its data.
 */

import type { Rgb } from "../../photometry/toneCurve";
import { THOMSON_CROSS_SECTION_M2 } from "../absorbers";
import {
  type AerosolParticles,
  aerosolModeOptics,
  aerosolPhaseTable,
  deltaMTruncation,
} from "../aerosol";
import {
  CHANNEL_WAVELENGTHS_NM,
  type DensityProfile,
  type MediumTerm,
  type PhaseFunction,
  type PhaseMatrixElement,
  type PhaseTable,
  type TabulatedDensity,
  type TermAbsorber,
} from "../medium";
import { type GasFractions, molecularMixture, type MolecularMixture } from "../rayleigh";

/** The wavelength an aerosol mode's column scale is set at, nm: `aerosolTerm`'s 550. */
export const AEROSOL_SCALE_WAVELENGTH_NM = 550;

/**
 * An absorber term's amount at relative density 1, in its kind's unit (`medium.ts`'s
 * `TermAbsorber` without its spec, which the bake's caller holds).
 */
export type SpectralAbsorberAmount =
  | { readonly kind: "column"; readonly numberDensityPerM3: number }
  | { readonly kind: "pair"; readonly pairDensityPerM6: number };

/**
 * What a term's optics come from at any wavelength (the module's remarks), plain data.
 *
 * - `molecular`: the well-mixed gas's fractions and n_s, its number density at relative density 1,
 *   molecules m⁻³.
 * - `thomson`: n_e at relative density 1, electrons m⁻³.
 * - `aerosol`: the mode's particles and its extinction coefficient at 550 nm at relative density
 *   1, m⁻¹.
 * - `absorber`: its amount at relative density 1; it absorbs by each light's bin curves.
 * - `channels`: a drawn term's own scattering and absorption coefficients per render channel at
 *   relative density 1, m⁻¹, and its phase, carried to any wavelength by the ground rule (the
 *   module's remarks); for a term that is not an absorber.
 */
export type SpectralSource =
  | {
      readonly kind: "molecular";
      readonly fractions: GasFractions;
      readonly numberDensityPerM3: number;
    }
  | { readonly kind: "thomson"; readonly electronDensityPerM3: number }
  | {
      readonly kind: "aerosol";
      readonly particles: AerosolParticles;
      readonly extinction550PerM: number;
    }
  | { readonly kind: "absorber"; readonly amount: SpectralAbsorberAmount }
  | {
      readonly kind: "channels";
      readonly scattering: Rgb;
      readonly absorption: Rgb;
      readonly phase: PhaseFunction;
    };

/**
 * A drawn term's channel-optics source (the module's remarks): its coefficients and phase as drawn,
 * for a kept scene's term that has no physical source to bake from.
 *
 * @remarks
 * The term's arrays are kept, not copied. An absorber term's source is its amount
 * ({@link absorberSourceOf}): {@link spectralColumnOf} refuses a channel source on it.
 */
export function channelSource(term: MediumTerm): SpectralSource {
  return {
    kind: "channels",
    scattering: term.scattering,
    absorption: term.absorption,
    phase: term.phase,
  };
}

/** An absorber's source: its amount at relative density 1, as its drawn term carries it. */
export function absorberSource(absorber: TermAbsorber): SpectralSource {
  let amount: SpectralAbsorberAmount;
  switch (absorber.kind) {
    case "column":
      amount = { kind: "column", numberDensityPerM3: absorber.numberDensityPerM3 };
      break;
    case "pair":
      amount = { kind: "pair", pairDensityPerM6: absorber.pairDensityPerM6 };
      break;
  }
  return { kind: "absorber", amount };
}

/**
 * An absorber term's source ({@link absorberSource} of its absorber).
 *
 * @throws RangeError for a term that absorbs by no curve.
 */
export function absorberSourceOf(term: MediumTerm): SpectralSource {
  if (term.absorber === undefined) {
    throw new RangeError(`term ${term.name} absorbs by no curve, so it has no absorber source`);
  }
  return absorberSource(term.absorber);
}

/** One term of a spectral column: its name and density as drawn, and its source. */
export interface SpectralTerm {
  /** The drawn term's name, unique within the column (`medium.ts`'s `MediumTerm.name`). */
  readonly name: string;
  /** The drawn term's density profile over the gravity-scaled height h\*. */
  readonly density: DensityProfile;
  readonly source: SpectralSource;
}

/**
 * A medium as a thick bake solves it: the drawn medium's top, ground and pressure profile, and its
 * terms by their sources (the module's remarks). Plain data, as a request to the optics worker
 * carries it.
 */
export interface SpectralColumn {
  /** The drawn medium's name, for logs. */
  readonly name: string;
  /** The top of the atmosphere above the ground in h\*, m. */
  readonly topHeightM: number;
  /** The ground's Lambertian albedo per render channel, red, green, blue, in [0, 1]. */
  readonly groundAlbedo: Rgb;
  readonly terms: ReadonlyArray<SpectralTerm>;
  /**
   * The column's pressure over 1 bar as a profile over the heights (`medium.ts`'s
   * `AtmosphereMedium.pressureBar`), which an absorber whose curves have pressure rows needs.
   */
  readonly pressureBar?: TabulatedDensity;
}

/**
 * A drawn medium's spectral column (`columnLayout.ts`, a light module the render thread loads to
 * form a bake's request, R08.T14.c).
 */
export { spectralColumnOf } from "./columnLayout";

/**
 * A term's phase function at one wavelength.
 *
 * - `rayleigh`: the dipole phase with the depolarisation ratio ρ (`medium.ts`'s `rayleigh` phase).
 * - `tabulated`: an aerosol mode's √θ table, its channel 0 the one read: {@link aerosolPhaseTable}
 *   of the mode's optics at the wavelength in all three channels, each normalised alone, so that
 *   at a render channel's wavelength it is that channel of the drawn term's table; for a channel
 *   source where one channel holds alone, that channel of its drawn table in all three (its values,
 *   its forward delta's share and its matrix's elements).
 * - `none`: a term that scatters nothing.
 * - `moments` (R08.T14.c): a phase known by its Legendre moments χ₀ … χ_N alone, χ₀ = 1 and
 *   N = {@link CHANNEL_SOURCE_MOMENT_ORDER}, with its forward delta's share of the scattering: a
 *   channel source's Cornette–Shanks phase ({@link cornetteShanksMoments}), and its blend of two
 *   channels' phases between their wavelengths. It carries no scattering matrix.
 */
export type SpectralPhase =
  | { readonly kind: "rayleigh"; readonly depolarisation: number }
  | { readonly kind: "tabulated"; readonly table: PhaseTable }
  | { readonly kind: "none" }
  | {
      readonly kind: "moments";
      /** χ₀ … χ_N, χ₀ = 1; not to be changed. */
      readonly moments: Float64Array;
      /** f_c, its forward delta's share of the scattering, in [0, 1). */
      readonly forwardDelta: number;
    };

/**
 * The highest moment a {@link SpectralPhase}'s `moments` carries, l = 0 … 64: as many as
 * `deltaM.test.ts`'s full 64-stream solves read, the most any {@link phaseMoments} caller takes
 * (the thick bake and the correction take 16).
 */
export const CHANNEL_SOURCE_MOMENT_ORDER = 64;

/**
 * Which render channels the ground rule takes at a wavelength (the module's remarks).
 *
 * - `one`: one channel alone, at its own wavelength or beyond the channels' range on its side.
 * - `two`: the two channels neighbouring in wavelength, `low` below the wavelength and `high`
 *   above it, the wavelength strictly between theirs.
 */
export type ChannelBlend =
  | { readonly kind: "one"; readonly channel: 0 | 1 | 2 }
  | {
      readonly kind: "two";
      readonly low: 0 | 1 | 2;
      readonly high: 0 | 1 | 2;
      /** λ − λ_low, nm, above 0. */
      readonly offsetNm: number;
      /** λ_high − λ_low, nm, above the offset. */
      readonly spanNm: number;
    };

/** The render channels in ascending order of their wavelengths. */
const CHANNELS_BY_WAVELENGTH: readonly [0 | 1 | 2, 0 | 1 | 2, 0 | 1 | 2] = (() => {
  const [first, second, third] = ([0, 1, 2] as const).toSorted(
    (x, y) => CHANNEL_WAVELENGTHS_NM[x] - CHANNEL_WAVELENGTHS_NM[y],
  );
  if (first === undefined || second === undefined || third === undefined) {
    throw new Error("the render channels are three");
  }
  return [first, second, third];
})();

/** A render channel's wavelength, nm ({@link CHANNEL_WAVELENGTHS_NM}). */
function channelWavelengthNm(channel: 0 | 1 | 2): number {
  return CHANNEL_WAVELENGTHS_NM[channel];
}

/**
 * The ground rule's channels at a wavelength (the module's remarks): the render channels sorted by
 * their wavelengths ({@link CHANNEL_WAVELENGTHS_NM}), the nearer end's alone outside them, a
 * channel alone at its own wavelength, and its two neighbours between.
 */
export function channelBlendAt(wavelengthNm: number): ChannelBlend {
  const [first, second, third] = CHANNELS_BY_WAVELENGTH;
  const at = channelWavelengthNm;
  if (wavelengthNm <= at(first)) {
    return { kind: "one", channel: first };
  }
  if (wavelengthNm >= at(third)) {
    return { kind: "one", channel: third };
  }
  if (wavelengthNm === at(second)) {
    return { kind: "one", channel: second };
  }
  const [low, high] = wavelengthNm < at(second) ? [first, second] : [second, third];
  return { kind: "two", low, high, offsetNm: wavelengthNm - at(low), spanNm: at(high) - at(low) };
}

/**
 * A quantity under a two-channel blend from its values in the blend's channels:
 * x_low + (x_high − x_low)(λ − λ_low) ÷ (λ_high − λ_low), the weights 1 − w and w,
 * w = (λ − λ_low) ÷ (λ_high − λ_low), in `bake.ts`'s `binGroundAlbedo`'s order of operations.
 */
function blended(
  blend: Extract<ChannelBlend, { kind: "two" }>,
  lowValue: number,
  highValue: number,
): number {
  return lowValue + ((highValue - lowValue) * blend.offsetNm) / blend.spanNm;
}

/**
 * A per-channel quantity at a wavelength by the ground rule ({@link channelBlendAt}): the channel's
 * own value where one channel holds alone, else its two neighbours' blend, linear in wavelength.
 * `bake.ts`'s `binGroundAlbedo` is this rule on the ground's albedo.
 */
export function channelRuleAt(values: Rgb, wavelengthNm: number): number {
  const blend = channelBlendAt(wavelengthNm);
  let value: number;
  switch (blend.kind) {
    case "one":
      value = values[blend.channel];
      break;
    case "two":
      value = blended(blend, values[blend.low], values[blend.high]);
      break;
  }
  return value;
}

/** A term's optics at one wavelength, at relative density 1. */
export interface SpectralTermOptics {
  /** σ_s, m⁻¹. */
  readonly scatteringPerM: number;
  /** Absorption other than a gas's curves, m⁻¹: an aerosol's σ_ext − σ_s, else 0. */
  readonly absorptionPerM: number;
  readonly phase: SpectralPhase;
}

/** A column's sources with what does not depend on the wavelength formed once. */
type PreparedSource =
  | {
      readonly kind: "molecular";
      readonly mixture: MolecularMixture;
      readonly numberDensityPerM3: number;
    }
  | { readonly kind: "thomson"; readonly scatteringPerM: number }
  | {
      readonly kind: "aerosol";
      readonly particles: AerosolParticles;
      readonly extinction550PerM: number;
      readonly crossSection550M2: () => number;
    }
  | { readonly kind: "absorber" }
  | {
      readonly kind: "channels";
      readonly scattering: Rgb;
      readonly absorption: Rgb;
      readonly phase: PhaseFunction;
      /** A channel's full moments χ₀ … χ_N ({@link channelMoments}), formed on first use. */
      readonly moments: (channel: 0 | 1 | 2) => Float64Array;
    };

/**
 * A channel's full Legendre moments χ₀ … χ_order of a drawn phase: Rayleigh's closed form at its ρ
 * ({@link phaseMoments}), a table's `deltaMTruncation(table, c, order ÷ 2).fullMoments`, and
 * {@link cornetteShanksMoments}; χ₀ = 1 alone for a term that scatters nothing.
 */
function channelMoments(phase: PhaseFunction, channel: 0 | 1 | 2, order: number): Float64Array {
  let moments: Float64Array;
  switch (phase.kind) {
    case "rayleigh":
      moments = phaseMoments(
        { kind: "rayleigh", depolarisation: phase.depolarisation[channel] },
        order,
      );
      break;
    case "tabulated":
      moments = deltaMTruncation(phase.table, channel, order / 2).fullMoments;
      break;
    case "cornette-shanks":
      moments = cornetteShanksMoments(phase.asymmetry, order);
      break;
    case "none":
      moments = new Float64Array(order + 1);
      moments[0] = 1;
      break;
  }
  return moments;
}

/** A drawn phase's forward delta's share in one channel: a table's `forwardPeak`, else 0. */
function channelForwardDelta(phase: PhaseFunction, channel: 0 | 1 | 2): number {
  return phase.kind === "tabulated" ? phase.table.forwardPeak[channel] : 0;
}

/**
 * A phase table's one channel as a table of its own, every channel of the copy that channel: its
 * values, its forward delta's share and its matrix's elements, the arrays kept, so that it reads
 * as {@link aerosolPhaseTable}'s table of one mode's optics in all three channels.
 */
function channelTable(table: PhaseTable, channel: 0 | 1 | 2): PhaseTable {
  const one = (element: PhaseMatrixElement): PhaseMatrixElement => [
    element[channel],
    element[channel],
    element[channel],
  ];
  const { matrix } = table;
  const share = table.forwardPeak[channel];
  return {
    u: table.u,
    values: one(table.values),
    matrix:
      matrix === undefined
        ? undefined
        : {
            a2: one(matrix.a2),
            a3: one(matrix.a3),
            a4: one(matrix.a4),
            b1: one(matrix.b1),
            b2: one(matrix.b2),
          },
    forwardPeak: [share, share, share],
  };
}

/**
 * A channel source's phase where one channel holds alone: Rayleigh at its ρ, its table's channel,
 * Cornette–Shanks by its moments, or none.
 */
function oneChannelPhase(
  source: Extract<PreparedSource, { kind: "channels" }>,
  channel: 0 | 1 | 2,
): SpectralPhase {
  const { phase } = source;
  let one: SpectralPhase;
  switch (phase.kind) {
    case "rayleigh":
      one = { kind: "rayleigh", depolarisation: phase.depolarisation[channel] };
      break;
    case "tabulated":
      one = { kind: "tabulated", table: channelTable(phase.table, channel) };
      break;
    case "cornette-shanks":
      one = { kind: "moments", moments: source.moments(channel).slice(), forwardDelta: 0 };
      break;
    case "none":
      one = { kind: "none" };
      break;
  }
  return one;
}

/** A channel source's optics at a wavelength by the ground rule (the module's remarks). */
function channelOptics(
  source: Extract<PreparedSource, { kind: "channels" }>,
  wavelengthNm: number,
): SpectralTermOptics {
  const blend = channelBlendAt(wavelengthNm);
  let optics: SpectralTermOptics;
  switch (blend.kind) {
    case "one":
      optics = {
        scatteringPerM: source.scattering[blend.channel],
        absorptionPerM: source.absorption[blend.channel],
        phase: oneChannelPhase(source, blend.channel),
      };
      break;
    case "two": {
      const { low, high } = blend;
      let phase: SpectralPhase;
      if (source.phase.kind === "none") {
        phase = { kind: "none" };
      } else {
        const lowMoments = source.moments(low);
        const highMoments = source.moments(high);
        const moments = new Float64Array(CHANNEL_SOURCE_MOMENT_ORDER + 1);
        moments[0] = 1;
        for (let l = 1; l <= CHANNEL_SOURCE_MOMENT_ORDER; l += 1) {
          moments[l] = blended(blend, lowMoments[l] ?? Number.NaN, highMoments[l] ?? Number.NaN);
        }
        phase = {
          kind: "moments",
          moments,
          forwardDelta: blended(
            blend,
            channelForwardDelta(source.phase, low),
            channelForwardDelta(source.phase, high),
          ),
        };
      }
      optics = {
        scatteringPerM: blended(blend, source.scattering[low], source.scattering[high]),
        absorptionPerM: blended(blend, source.absorption[low], source.absorption[high]),
        phase,
      };
      break;
    }
  }
  return optics;
}

function prepareSource(source: SpectralSource): PreparedSource {
  let prepared: PreparedSource;
  switch (source.kind) {
    case "molecular":
      prepared = {
        kind: "molecular",
        mixture: molecularMixture(source.fractions),
        numberDensityPerM3: source.numberDensityPerM3,
      };
      break;
    case "thomson":
      prepared = {
        kind: "thomson",
        scatteringPerM: source.electronDensityPerM3 * THOMSON_CROSS_SECTION_M2,
      };
      break;
    case "aerosol": {
      let at550: number | undefined;
      prepared = {
        kind: "aerosol",
        particles: source.particles,
        extinction550PerM: source.extinction550PerM,
        crossSection550M2: () => {
          at550 ??= aerosolModeOptics(
            source.particles,
            AEROSOL_SCALE_WAVELENGTH_NM,
          ).extinctionCrossSectionM2;
          return at550;
        },
      };
      break;
    }
    case "absorber":
      prepared = { kind: "absorber" };
      break;
    case "channels": {
      const { phase } = source;
      const formed: Array<Float64Array | undefined> = [undefined, undefined, undefined];
      prepared = {
        kind: "channels",
        scattering: source.scattering,
        absorption: source.absorption,
        phase,
        moments: (channel) => {
          const known = formed[channel];
          if (known !== undefined) {
            return known;
          }
          const moments = channelMoments(phase, channel, CHANNEL_SOURCE_MOMENT_ORDER);
          formed[channel] = moments;
          return moments;
        },
      };
      break;
    }
  }
  return prepared;
}

function opticsOf(source: PreparedSource, wavelengthNm: number): SpectralTermOptics {
  let optics: SpectralTermOptics;
  switch (source.kind) {
    case "molecular":
      optics = {
        scatteringPerM: source.numberDensityPerM3 * source.mixture.crossSectionM2(wavelengthNm),
        absorptionPerM: 0,
        phase: { kind: "rayleigh", depolarisation: source.mixture.depolarisation(wavelengthNm) },
      };
      break;
    case "thomson":
      optics = {
        scatteringPerM: source.scatteringPerM,
        absorptionPerM: 0,
        phase: { kind: "rayleigh", depolarisation: 0 },
      };
      break;
    case "aerosol": {
      const mode = aerosolModeOptics(source.particles, wavelengthNm);
      // `aerosolTerm`'s scale, in its order of operations, so that at a render channel's
      // wavelength the coefficients are the drawn term's to the bit.
      const extinctionPerM =
        (source.extinction550PerM * mode.extinctionCrossSectionM2) / source.crossSection550M2();
      const scatteringPerM = extinctionPerM * mode.singleScatteringAlbedo;
      optics = {
        scatteringPerM,
        absorptionPerM: extinctionPerM - scatteringPerM,
        phase: { kind: "tabulated", table: aerosolPhaseTable([mode, mode, mode]) },
      };
      break;
    }
    case "absorber":
      optics = { scatteringPerM: 0, absorptionPerM: 0, phase: { kind: "none" } };
      break;
    case "channels":
      optics = channelOptics(source, wavelengthNm);
      break;
  }
  return optics;
}

/**
 * Each term's optics at each wavelength, `[wavelength][term]` (the module's remarks): an aerosol
 * mode's Mie, MMF or phase-file optics computed once a wavelength, and once more at 550 nm for its
 * scale.
 *
 * @remarks
 * A channel source takes its channels by the ground rule ({@link channelBlendAt}): σ_s = Σ w_c σ_s,c
 * and σ_a = Σ w_c σ_a,c, and its phase that channel's own where one holds alone, else the blend of
 * the two channels' full moments, χ_l = Σ w_c χ_l,c, l ≤ {@link CHANNEL_SOURCE_MOMENT_ORDER}, with
 * f = Σ w_c f_c.
 *
 * @param wavelengthsNm - Vacuum wavelengths, nm, within every source's range (380–780 nm for the
 *   aerosols, 300–1,000 nm for Rayleigh; any for a channel source).
 * @throws RangeError as `molecularMixture` and `aerosolModeOptics` refuse their inputs.
 */
export function spectralOptics(
  column: SpectralColumn,
  wavelengthsNm: ReadonlyArray<number>,
): ReadonlyArray<ReadonlyArray<SpectralTermOptics>> {
  const sources = column.terms.map((term) => prepareSource(term.source));
  return wavelengthsNm.map((nm) => sources.map((source) => opticsOf(source, nm)));
}

/**
 * A phase function's Legendre moments χ_l = ∫ p P_l(cos Θ) dΩ, l = 0 … `order`, χ₀ = 1, as the
 * solver takes them (`discreteOrdinates.ts`'s `OrdinateLayer.moments`).
 *
 * @remarks
 * Rayleigh's are closed: χ₂ = (1 − ρ) ÷ (5(2 + ρ)), the rest past χ₀ zero (the dipole phase
 * 3 ÷ (16π(1 + 2γ)) × ((1 + 3γ) + (1 − γ) cos²Θ), γ = ρ ÷ (2 − ρ), is (1 ÷ 4π)(1 + c P₂) with
 * c = (1 − γ) ÷ (2(1 + 2γ)), whose χ₂ is c ÷ 5; derived here, 0.1 at ρ = 0 as Chandrasekhar's).
 * A table's are `aerosol.ts`'s {@link deltaMTruncation}'s full moments of its channel 0, taken on
 * the table as read; its forward delta is among them. A `moments` phase gives its first ones.
 *
 * @param order - The highest l, an even whole number ≥ 2.
 * @throws RangeError for an order that is not an even whole number ≥ 2, or past a `moments`
 *   phase's own ({@link CHANNEL_SOURCE_MOMENT_ORDER}).
 */
export function phaseMoments(phase: SpectralPhase, order: number): Float64Array {
  if (!(Number.isInteger(order) && order >= 2 && order % 2 === 0)) {
    throw new RangeError(`a phase's moments are taken to an even order ≥ 2, not ${order}`);
  }
  const moments = new Float64Array(order + 1);
  moments[0] = 1;
  switch (phase.kind) {
    case "rayleigh": {
      const rho = phase.depolarisation;
      moments[2] = (1 - rho) / (5 * (2 + rho));
      break;
    }
    case "tabulated":
      moments.set(deltaMTruncation(phase.table, 0, order / 2).fullMoments);
      break;
    case "none":
      break;
    case "moments": {
      const known = phase.moments.length - 1;
      if (order > known) {
        throw new RangeError(`a phase of ${known + 1} moments has no χ_${order}`);
      }
      moments.set(phase.moments.subarray(0, order + 1));
      break;
    }
  }
  return moments;
}

/**
 * Cornette and Shanks's phase function's Legendre moments χ_l = ∫ p P_l(cos Θ) dΩ, l = 0 …
 * `order`, in closed form (derived here), for its shape parameter g (W. M. Cornette and J. G.
 * Shanks, Appl. Opt. 31(16) (1992) 3152, doi:10.1364/AO.31.003152, the form `medium.ts`'s
 * `phaseAt` draws, its denominator unfloored).
 *
 * @remarks
 * The phase is p_CS(μ) = (3 ÷ 8π)(1 − g²)(1 + μ²) ÷ ((2 + g²)(1 + g² − 2gμ)^{3/2}) =
 * A (1 + μ²) p_HG(μ), A = 3 ÷ (2(2 + g²)), with p_HG Henyey and Greenstein's, whose moments are
 * g^l. Applying μ P_l = ((l + 1) P_{l+1} + l P_{l−1}) ÷ (2l + 1) twice gives
 * μ² P_l = α_l P_{l+2} + β_l P_l + γ_l P_{l−2}, with α_l = (l + 1)(l + 2) ÷ ((2l + 1)(2l + 3)),
 * β_l = (2l² + 2l − 1) ÷ ((2l − 1)(2l + 3)) and γ_l = l(l − 1) ÷ ((2l − 1)(2l + 1)), so that
 *
 * χ_l = A [g^l (1 + β_l) + α_l g^{l+2} + γ_l g^{l−2}],
 *
 * with no γ term for l < 2 (γ₀ = γ₁ = 0). χ₀ = A (4 + 2g²) ÷ 3 = 1, set exactly; χ₁ =
 * 3g(4 + g²) ÷ (5(2 + g²)), `thick/regime.ts`'s `cornetteShanksMeanCosine`; at g = 0 the phase is
 * Rayleigh's at ρ = 0, χ₂ = 1 ÷ 10. `spectralColumn.test.ts` holds it to a Gauss–Legendre
 * quadrature of `phaseAt` to 10⁻¹⁰ for l ≤ 32. `phaseAt` floors its denominator at 10⁻⁶, which
 * differs from this only where (1 − |g|)² < 10⁻⁶, that is 1 − |g| < 10⁻³, g near 1 in the forward
 * direction or near −1 in the backward.
 *
 * @param asymmetry - g, finite and in (−1, 1).
 * @param order - The highest l, a whole number.
 * @throws RangeError for a g outside (−1, 1) or an order that is not a whole number.
 */
export function cornetteShanksMoments(asymmetry: number, order: number): Float64Array {
  const g = asymmetry;
  if (!(g > -1 && g < 1)) {
    throw new RangeError(`Cornette and Shanks's g lies in (−1, 1), not ${g}`);
  }
  if (!(Number.isInteger(order) && order >= 0)) {
    throw new RangeError(`a phase's moments are taken to a whole order, not ${order}`);
  }
  const a = 3 / (2 * (2 + g * g));
  const moments = new Float64Array(order + 1);
  moments[0] = 1;
  for (let l = 1; l <= order; l += 1) {
    const alpha = ((l + 1) * (l + 2)) / ((2 * l + 1) * (2 * l + 3));
    const beta = (2 * l * l + 2 * l - 1) / ((2 * l - 1) * (2 * l + 3));
    const gamma = l < 2 ? 0 : (l * (l - 1)) / ((2 * l - 1) * (2 * l + 1));
    const below = l < 2 ? 0 : gamma * g ** (l - 2);
    moments[l] = a * (g ** l * (1 + beta) + alpha * g ** (l + 2) + below);
  }
  return moments;
}

/**
 * A phase's forward delta's share of the scattering: a table's channel 0's
 * (`medium.ts`'s `PhaseTable.forwardPeak`), a `moments` phase's own, and 0 for a closed form.
 */
export function forwardDeltaShare(phase: SpectralPhase): number {
  let share: number;
  switch (phase.kind) {
    case "tabulated":
      share = phase.table.forwardPeak[0];
      break;
    case "moments":
      share = phase.forwardDelta;
      break;
    case "rayleigh":
    case "none":
      share = 0;
      break;
  }
  return share;
}

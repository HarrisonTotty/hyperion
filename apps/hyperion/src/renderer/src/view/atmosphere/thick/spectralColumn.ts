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
 *
 * {@link spectralColumnOf} lays the sources beside a drawn medium's terms. Mie, the MMF and the
 * phase files run where the column's optics are taken, in the optics worker, never on the render
 * thread: this module imports the optics, and the render thread only types its data.
 */

import type { Rgb } from "../../photometry/toneCurve";
import { THOMSON_CROSS_SECTION_M2 } from "../absorbers";
import {
  type AerosolParticles,
  aerosolModeOptics,
  aerosolPhaseTable,
  deltaMTruncation,
} from "../aerosol";
import type { AtmosphereMedium, DensityProfile, PhaseTable, TabulatedDensity } from "../medium";
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
  | { readonly kind: "absorber"; readonly amount: SpectralAbsorberAmount };

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
 * A drawn medium's spectral column: its name, top, ground, pressure profile and each term's name
 * and density, with `sources[i]` the source of `medium.terms[i]` (R08.T14.c's hand-off: the
 * assembly gives the sources beside its terms).
 *
 * @throws RangeError for a source count other than the terms', an absorber term without an
 *   absorber source of its kind, or an absorber source on a term that absorbs by none.
 */
export function spectralColumnOf(
  medium: AtmosphereMedium,
  sources: ReadonlyArray<SpectralSource>,
): SpectralColumn {
  if (sources.length !== medium.terms.length) {
    throw new RangeError(
      `medium ${medium.name} has ${medium.terms.length} terms, not ${sources.length} sources`,
    );
  }
  const terms = medium.terms.map((term, i): SpectralTerm => {
    const source = sources[i];
    if (source === undefined) {
      throw new RangeError(`medium ${medium.name}'s term ${term.name} has no source`);
    }
    const absorbs = term.absorber?.kind;
    const given = source.kind === "absorber" ? source.amount.kind : undefined;
    if (absorbs !== given) {
      throw new RangeError(
        `term ${term.name} absorbs by ${absorbs ?? "no curve"}, its source by ${given ?? "none"}`,
      );
    }
    return { name: term.name, density: term.density, source };
  });
  return {
    name: medium.name,
    topHeightM: medium.topHeightM,
    groundAlbedo: medium.groundAlbedo,
    terms,
    ...(medium.pressureBar === undefined ? {} : { pressureBar: medium.pressureBar }),
  };
}

/**
 * A term's phase function at one wavelength.
 *
 * - `rayleigh`: the dipole phase with the depolarisation ratio ρ (`medium.ts`'s `rayleigh` phase).
 * - `tabulated`: an aerosol mode's √θ table, its channel 0 the one read: {@link aerosolPhaseTable}
 *   of the mode's optics at the wavelength in all three channels, each normalised alone, so that
 *   at a render channel's wavelength it is that channel of the drawn term's table.
 * - `none`: a term that scatters nothing.
 */
export type SpectralPhase =
  | { readonly kind: "rayleigh"; readonly depolarisation: number }
  | { readonly kind: "tabulated"; readonly table: PhaseTable }
  | { readonly kind: "none" };

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
  | { readonly kind: "absorber" };

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
  }
  return optics;
}

/**
 * Each term's optics at each wavelength, `[wavelength][term]` (the module's remarks): an aerosol
 * mode's Mie, MMF or phase-file optics computed once a wavelength, and once more at 550 nm for its
 * scale.
 *
 * @param wavelengthsNm - Vacuum wavelengths, nm, within every source's range (380–780 nm for the
 *   aerosols, 300–1,000 nm for Rayleigh).
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
 * the table as read; its forward delta is among them.
 *
 * @param order - The highest l, an even whole number ≥ 2.
 * @throws RangeError for an order that is not an even whole number ≥ 2.
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
  }
  return moments;
}

/** A phase's forward delta's share of the scattering (`medium.ts`'s `PhaseTable.forwardPeak`), 0 for a closed form. */
export function forwardDeltaShare(phase: SpectralPhase): number {
  return phase.kind === "tabulated" ? phase.table.forwardPeak[0] : 0;
}

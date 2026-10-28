/**
 * Absorbing gases (plan R08, Design notes 5 and 8; R08.T4.b and R08.T4.c): their cross-sections,
 * the absorber terms laid on a column, each sun's curves of growth, the pairs' collision-induced
 * and free–free absorption, and Thomson scattering by free electrons.
 *
 * @remarks
 * An absorber is a gas whose absorption bands are narrow beside a channel, ozone's Chappuis band
 * and methane's bands, so a band-averaged cross-section, biased dark by Jensen's inequality and
 * wrong for a saturated band, is not used. Each absorber, channel and sun takes a curve of growth
 * instead (ruled 2026-10-10, `decision-r08-t4b-t12c.md`),
 *
 *   T_c(u) = Σ c̄_c S e^(−σ(λ)u) Δλ ÷ Σ c̄_c S Δλ,
 *
 * on σ's own grid over the bake range, with u the column crossed, molecules m⁻², S the sun's
 * spectrum interpolated from its 15 bin means (`spectralColour.ts`'s `bakeSpectrumAt`) and c̄_c
 * the signed matching functions in linear Rec. 709, the weights that give the star its colour. So
 * the star's colour times T_c is the colour of its light through the absorber wherever that colour
 * lies in Rec. 709. T_c is clamped to [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1] and made
 * non-increasing in u over the curve's grid ({@link channelTransmittance}, {@link absorberCurve}).
 * The tables accumulate u, and each sun's −ln T_c(u) is added to the optical depth where they are
 * read (Design note 8; R08.T6.c). The spectral bakes take one curve per bake bin instead
 * ({@link absorberBinCurves}).
 *
 * Species are keyed by the substance registry's strings (decision-composition §1.9): a species
 * the client holds no cross-sections for draws nothing, and is reported, never dropped silently.
 * Species whose mixing ratios hold constant with height share one curve, since
 * σ_mix(λ) = Σ xᵢσᵢ(λ) is then exact, so the {@link ABSORBER_COLUMNS} stored columns count
 * vertical profiles, not species.
 *
 * A pair absorber (R08.T4.c) absorbs as σ n_A n_B, its cross-section σ in m⁵ a molecule pair:
 * the collision-induced absorption of two gases, or H⁻'s free–free absorption by an H atom and a
 * free electron. Its column is the pair column u = ∫ n_A n_B ds, molecule pairs m⁻⁵, and its
 * vertical profile is the product of its members' densities, so it takes a stored column of its
 * own; otherwise its curves are a column absorber's, over σu. `absorberSpecs` forms each pair from
 * its members' layers ({@link absorberSpecs}).
 *
 * Stated approximations: one temperature a profile, its column-weighted mean (the pair-weighted
 * mean for a pair), for the cross-sections ({@link absorberSpecs}); cross-sections held, and
 * labelled, beyond their law's measured temperatures; Karkoschka and Tomasko's methane at infinite
 * pressure, without their finite-pressure correction; and S from 15 bin means, adequate for FGK, A
 * and B stars but not for M dwarfs' TiO bands (R08's Risks).
 */

import type { Rgb } from "../photometry/toneCurve";
import alkaliPairsFile from "./absorbers/alkaliPairs.json" with { type: "json" };
import crossSectionsFile from "./absorbers/crossSections.json" with { type: "json" };
import oxygenPairsFile from "./absorbers/oxygenPairs.json" with { type: "json" };
import { HE_MINUS_FREE_FREE_RANGE_K, heMinusFreeFreeM5 } from "./absorbers/heliumMinus";
import { H_MINUS_FREE_FREE_RANGE_K, hMinusBoundFreeM2, hMinusFreeFreeM5 } from "./absorbers/hMinus";
import {
  broadeningFactor,
  cellSigmasAt,
  curveRowsPa,
  isLatticeMidpoint,
  type KDistributionTable,
  kDistributionAt,
  type SpeciesKDistribution,
} from "./absorbers/kDistribution";
import { ABSORBER_K_DISTRIBUTIONS } from "./absorbers/lineAbsorbers";
import { isTransparent } from "./absorbers/transparency";
import type { AtmosphereColumn } from "./column";
import {
  BAKE_BIN_COUNT,
  BAKE_BIN_WIDTH_NM,
  BAKE_RANGE_NM,
  columnLengthM,
  densityAt,
  type MediumTerm,
  type TabulatedDensity,
  tabulatedDensity,
} from "./medium";
import type { OpticsProvenance } from "./rayleigh";
import {
  type BakeSpectrum,
  bakeSpectrumAt,
  bakeSpectrumOf,
  matchingFunctionsAt,
} from "./spectralColour";

/**
 * The absorber columns the per-planet tables store: 5, the transmittance table's alpha and a
 * second `rgba16float` table's four channels (Design notes 5 and 8). Each holds one vertical
 * profile's column, whatever its number of species.
 */
export const ABSORBER_COLUMNS = 5;

/**
 * How a table's σ is taken between its temperatures (the table's own field, written by the tool
 * with its data).
 *
 * - `linear`: linear in T between the two measured temperatures around it. Ozone's, whose 10 K
 *   steps are the data's own resolution.
 * - `lnQuadratic`: Karkoschka and Tomasko 2010's Eq. 8, ln σ the quadratic through the table's
 *   three temperatures in Lagrange's form, for their 100, 198 and 296 K
 *   ln σ(T) = ½z(z − 1) ln σ₁₀₀ + (1 − z²) ln σ₁₉₈ + ½z(z + 1) ln σ₂₉₆, z = (T − 198 K) ÷ 98 K
 *   (as Collins et al. 2018, Sci. Adv. 4, eaas9593, Methods Eqs. 4–5, restate it, and NEMESIS's
 *   `tkark.f` and PICASO apply it; `decision-r08-t4b-t12c.md` §1.6). A point with a 0 at any of the
 *   three temperatures, below the table's three figures, is linear in σ instead, extrapolated from
 *   the nearest pair beyond them and floored at 0.
 * - `lnLinear` (R08.T4.c): ln σ linear in T between the two temperatures around it, where σ is
 *   above 0 at both, and linear in σ where it is not. H₂–He's, whose collision-induced absorption
 *   grows by orders of magnitude over 200–9,900 K (`decision-r08-t4b-t12c.md`, follow-up 3), and
 *   H₂–H₂'s, over 60–7,000 K.
 * - `lnLnLinear` (R08.T4.d): a power law in T through a table's two temperatures, ln σ linear in
 *   ln T, where σ is above 0 at both, and linear in σ where it is not. H₂O's self continuum through
 *   CKD's 260 and 296 K (`decision-r08-t4d-kdist.md` §6.3).
 * - `constant` (R08.T4.d): a table's one temperature's σ at every temperature. H₂O's foreign
 *   continuum, held at 296 K as CKD prescribes.
 */
export type TemperatureLaw = "linear" | "lnQuadratic" | "lnLinear" | "lnLnLinear" | "constant";

/** Whether a file's string is a {@link TemperatureLaw}. */
function isTemperatureLaw(value: string): value is TemperatureLaw {
  return (
    value === "linear" ||
    value === "lnQuadratic" ||
    value === "lnLinear" ||
    value === "lnLnLinear" ||
    value === "constant"
  );
}

/** How many temperatures a law takes, if it takes a fixed number. */
const LAW_TEMPERATURES: Readonly<Partial<Record<TemperatureLaw, number>>> = {
  lnQuadratic: 3,
  lnLnLinear: 2,
  constant: 1,
};

/** A species' measured cross-sections at several temperatures on a uniform grid of vacuum wavelengths. */
export interface CrossSectionTable {
  /** The registry key. */
  readonly species: string;
  /** The data's citation and how it was reduced. */
  readonly source: string;
  /** The first grid point, nm. */
  readonly firstNm: number;
  /** The grid's spacing, nm. */
  readonly stepNm: number;
  /** The tabulated temperatures, K, ascending. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** How σ is taken between and beyond them. */
  readonly temperatureLaw: TemperatureLaw;
  /**
   * The temperatures over which the law gives a `measured` σ, K: the table's own for `linear`,
   * its law's stated range for `lnQuadratic` (Karkoschka and Tomasko's 50–300 K).
   */
  readonly measuredRangeK: readonly [number, number];
  /** σ at each temperature, m² a molecule: `crossSectionsM2[t][i]` at `firstNm + i stepNm`. */
  readonly crossSectionsM2: ReadonlyArray<Float64Array>;
}

/**
 * A molecule pair's measured cross-sections at several temperatures on a uniform grid of vacuum
 * wavelengths (R08.T4.c): its absorption coefficient is σ n_A n_B.
 */
export interface PairCrossSectionTable {
  /** The pair's key, its members' registry keys joined by `+` (`O2+O2`). */
  readonly pair: string;
  /** The members' registry keys, as the pair's key names them. */
  readonly members: readonly [string, string];
  /** The data's citation and how it was reduced. */
  readonly source: string;
  /** The first grid point, nm. */
  readonly firstNm: number;
  /** The grid's spacing, nm. */
  readonly stepNm: number;
  /** The tabulated temperatures, K, ascending. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** How σ is taken between and beyond them. */
  readonly temperatureLaw: TemperatureLaw;
  /** The temperatures over which the law gives a `measured` σ, K. */
  readonly measuredRangeK: readonly [number, number];
  /** The temperatures over which the law is applied, K, the nearest end held beyond. */
  readonly lawRangeK: readonly [number, number];
  /** The wavelength below which the table's σ is `estimated` at every temperature, nm. */
  readonly estimatedBelowNm?: number;
  /**
   * For a pair whose second member is {@link AIR_PARTNER}, every other listed gas: each gas's weight
   * in the partner's pair column (1 for a gas not listed), and the gases with which σ is `measured`.
   */
  readonly air?: AirPartners;
  /** σ at each temperature, m⁵ a molecule pair: `crossSectionsM5[t][i]` at `firstNm + i stepNm`. */
  readonly crossSectionsM5: ReadonlyArray<Float64Array>;
}

/** The weights and measured gases of a pair's {@link AIR_PARTNER}. */
export interface AirPartners {
  readonly weights: ReadonlyMap<string, number>;
  readonly measured: ReadonlySet<string>;
}

/** One table of a cross-sections file as its JSON import reads it, a species' or a pair's. */
interface FileTable {
  readonly source: string;
  readonly reduction: string;
  readonly firstNm: number;
  readonly stepNm: number;
  readonly temperaturesK: ReadonlyArray<number>;
  readonly temperatureLaw: string;
  readonly measuredRangeK: ReadonlyArray<number>;
  readonly lawRangeK?: ReadonlyArray<number>;
  readonly estimatedBelowNm?: number;
  readonly partners?: Readonly<Record<string, number>>;
  readonly measuredPartners?: ReadonlyArray<string>;
  readonly rows: ReadonlyArray<ReadonlyArray<number>>;
}

/** `crossSections.json`'s species tables, as its JSON import reads them. */
export interface CrossSectionsFile {
  readonly tables: ReadonlyArray<FileTable & { readonly species: string }>;
}

/** `crossSections.json`'s pair tables, as its JSON import reads them. */
export interface PairCrossSectionsFile {
  readonly pairs: ReadonlyArray<
    FileTable & { readonly pair: string; readonly members: ReadonlyArray<string> }
  >;
}

/** How far a row's wavelength may lie from its grid point, nm. */
const GRID_TOLERANCE_NM = 1e-9;

/** How far a wavelength may lie from a grid point and still be read as on it, in grid steps. */
const GRID_TOLERANCE_STEPS = 1e-9;

/** A file table's checked grid: its law, its ranges and its values at each temperature. */
interface CheckedGrid {
  readonly temperatureLaw: TemperatureLaw;
  readonly measuredRangeK: readonly [number, number];
  readonly lawRangeK: readonly [number, number];
  readonly columns: ReadonlyArray<Float64Array>;
}

/**
 * A file table's temperatures, law, measured range and rows, checked: ascending temperatures, a law
 * and a measured range that fit them, rows on the grid with a finite σ ≥ 0 at each temperature,
 * and a grid that covers {@link BAKE_RANGE_NM} with a point on each of its ends.
 *
 * @throws Error naming the table and the check it fails.
 */
function readGrid(table: FileTable, where: string): CheckedGrid {
  const { firstNm, stepNm, temperaturesK, rows } = table;
  if (
    temperaturesK.length === 0 ||
    temperaturesK.some((t, i) => !(t > (temperaturesK[i - 1] ?? 0)))
  ) {
    throw new Error(`${where}'s temperatures do not ascend from above 0 K`);
  }
  const law = table.temperatureLaw;
  const fixed = isTemperatureLaw(law) ? LAW_TEMPERATURES[law] : undefined;
  if (!isTemperatureLaw(law) || (fixed !== undefined && temperaturesK.length !== fixed)) {
    throw new Error(`${where}'s temperature law ${law} does not fit its temperatures`);
  }
  const [lowK, highK] = table.measuredRangeK;
  const coldest = temperaturesK[0] ?? Number.NaN;
  const hottest = temperaturesK[temperaturesK.length - 1] ?? Number.NaN;
  if (
    lowK === undefined ||
    highK === undefined ||
    table.measuredRangeK.length !== 2 ||
    !(lowK > 0 && highK >= lowK) ||
    ((law === "linear" || law === "lnLinear") && !(lowK >= coldest && highK <= hottest))
  ) {
    throw new Error(`${where}'s measured range does not fit its law and temperatures`);
  }
  const [lawLowK, lawHighK] = table.lawRangeK ?? [lowK, highK];
  if (
    lawLowK === undefined ||
    lawHighK === undefined ||
    (table.lawRangeK !== undefined && table.lawRangeK.length !== 2) ||
    !(lawLowK > 0 && lawLowK <= lowK && lawHighK >= highK)
  ) {
    throw new Error(`${where}'s law range does not hold its measured range`);
  }
  const columns = temperaturesK.map(() => new Float64Array(rows.length));
  for (const [i, row] of rows.entries()) {
    const nm = row[0] ?? Number.NaN;
    if (
      !(Math.abs(nm - (firstNm + i * stepNm)) <= GRID_TOLERANCE_NM) ||
      row.length !== columns.length + 1
    ) {
      throw new Error(
        `${where}'s row ${i} is not [${firstNm + i * stepNm}, σ at each temperature]`,
      );
    }
    for (const [t, column] of columns.entries()) {
      const sigma = row[t + 1] ?? Number.NaN;
      if (!(Number.isFinite(sigma) && sigma >= 0)) {
        throw new Error(`${where}'s σ at ${nm} nm and ${temperaturesK[t]} K is ${sigma}`);
      }
      column[i] = sigma;
    }
  }
  const [low, high] = BAKE_RANGE_NM;
  const lowIndex = (low - firstNm) / stepNm;
  const highIndex = (high - firstNm) / stepNm;
  if (
    !(lowIndex >= 0 && highIndex <= rows.length - 1) ||
    !Number.isInteger(lowIndex) ||
    !Number.isInteger(highIndex)
  ) {
    throw new Error(`${where}'s grid does not cover ${low}–${high} nm with a point on each end`);
  }
  return {
    temperatureLaw: law,
    measuredRangeK: [lowK, highK],
    lawRangeK: [lawLowK, lawHighK],
    columns,
  };
}

/**
 * A cross-sections file's species tables, checked: unique species, ascending temperatures and a
 * law and a measured range that fit them, rows on the grid with a finite σ ≥ 0 at each
 * temperature, and a grid that covers {@link BAKE_RANGE_NM} with a point on each of its ends.
 *
 * @throws Error naming the table and the check it fails.
 */
export function readCrossSections(file: CrossSectionsFile): ReadonlyArray<CrossSectionTable> {
  const seen = new Set<string>();
  return file.tables.map((table) => {
    const { species, firstNm, stepNm, temperaturesK } = table;
    const where = `crossSections.json's ${species}`;
    if (seen.has(species)) {
      throw new Error(`${where} is listed twice`);
    }
    seen.add(species);
    const grid = readGrid(table, where);
    return {
      species,
      source: `${table.source}. ${table.reduction}`,
      firstNm,
      stepNm,
      temperaturesK,
      temperatureLaw: grid.temperatureLaw,
      measuredRangeK: grid.measuredRangeK,
      crossSectionsM2: grid.columns,
    };
  });
}

/**
 * A pair member that stands for every other listed gas, each weighted by the table's partners
 * (H₂O's foreign continuum, `H2O+air`; R08.T4.d).
 */
export const AIR_PARTNER = "air";

/**
 * A cross-sections file's pair tables, checked as {@link readCrossSections} checks a species', and
 * each pair's key its two members' joined by `+`, listed once.
 *
 * @param fileName - The file's name, for messages.
 * @throws Error naming the pair and the check it fails.
 */
export function readPairCrossSections(
  file: PairCrossSectionsFile,
  fileName = "crossSections.json",
): ReadonlyArray<PairCrossSectionTable> {
  const seen = new Set<string>();
  return file.pairs.map((table) => {
    const { pair, members, firstNm, stepNm, temperaturesK } = table;
    const where = `${fileName}'s pair ${pair}`;
    const [a, b] = members;
    if (members.length !== 2 || a === undefined || b === undefined || pair !== `${a}+${b}`) {
      throw new Error(`${where} is not its two members' keys joined by +`);
    }
    if (seen.has(pair)) {
      throw new Error(`${where} is listed twice`);
    }
    seen.add(pair);
    const grid = readGrid(table, where);
    const { partners, measuredPartners } = table;
    if (
      (partners !== undefined) !== (b === AIR_PARTNER) ||
      (measuredPartners !== undefined) !== (b === AIR_PARTNER)
    ) {
      throw new Error(
        `${where} gives partners unless and only if its second member is ${AIR_PARTNER}`,
      );
    }
    if (Object.values(partners ?? {}).some((w) => !(Number.isFinite(w) && w > 0))) {
      throw new Error(`${where}'s partner weights are not finite and above 0`);
    }
    return {
      pair,
      members: [a, b],
      source: `${table.source}. ${table.reduction}`,
      firstNm,
      stepNm,
      temperaturesK,
      temperatureLaw: grid.temperatureLaw,
      measuredRangeK: grid.measuredRangeK,
      lawRangeK: grid.lawRangeK,
      ...(table.estimatedBelowNm === undefined ? {} : { estimatedBelowNm: table.estimatedBelowNm }),
      ...(partners === undefined || measuredPartners === undefined
        ? {}
        : {
            air: {
              weights: new Map(Object.entries(partners)),
              measured: new Set(measuredPartners),
            },
          }),
      crossSectionsM5: grid.columns,
    };
  });
}

/**
 * The cross-sections the client holds (`absorbers/crossSections.json`, written by
 * `scripts/atmosphereData.mjs cross-sections`), on vacuum wavelengths over 380–800 nm:
 *
 * - `O3`: Serdyuchenko et al. 2014 (Atmos. Meas. Tech. 7, 625; Gorshelev et al. 2014, 609) at
 *   193–293 K every 10 K, binned to 1 nm, each bin the mean over [λ − 0.5, λ + 0.5) nm
 *   (`decisions-r05.md` item 2), linear in T and measured over 193–293 K;
 * - `CH4`: Karkoschka and Tomasko 2010 (Icarus 205, 674) at 100, 198 and 296 K, as NASA's
 *   Planetary Spectrum Generator converts them, interpolated onto a 0.25 nm grid, finer than their
 *   10 cm⁻¹ spectral width (0.27 nm at 518 nm), so that no band-model value is averaged
 *   (`decision-r08-licences.md` row 1), by their Eq. 8 (`lnQuadratic`) and measured over 50–300 K,
 *   the span of the k-tables their Table 4's caption names (`decision-r08-t4b-t12c.md` §1.6).
 *   These are their coefficients "for the limit of infinite pressure", e^(−ku) without their
 *   finite-pressure correction (their Eqs. 2–4, a Goody random band of Voigt lines with Table 4's
 *   line-spacing parameter, which PSG's file omits): a stated approximation. Measured with Irwin's
 *   line spacings at 100 K under the Sun, the correction would raise the red channel's
 *   transmittance through 30 km-amagat by at most 1.4% at 1 atm, 4.4% at 0.3 atm and 11.5% at
 *   0.1 atm (through 3 km-amagat, 0.1%, 0.4% and 1.0%); a body's methane lies chiefly at 0.5 bar
 *   and deeper. Above 300 K its values are held, `estimated`: no open line list reaches its
 *   visible bands hotter (R08.T4.f confirms at fetch; `decision-r08-t4b-t12c.md`, follow-up 2);
 * - `SO2` (R08.T4.c): its near-ultraviolet tail, Hermans, Vandaele and Fally 2009 (J. Quant.
 *   Spectrosc. Radiat. Transfer 110, 756) at 298, 318, 338 and 358 K, as the MPI-Mainz atlas's
 *   CC BY 4.0 snapshot holds them (Keller-Rudek et al. 2013), binned to 1 nm, linear in T and
 *   measured over 298–358 K. The source ends at 416.66 nm, so σ is 0 from 417 nm up, where the
 *   ã–X̃ band's tail has fallen below 10⁻²⁵ cm²: 2.4 × 10⁻²² cm² at 383 nm, 1.2 × 10⁻²⁴ at 400 nm.
 */
export const ABSORBER_CROSS_SECTIONS: ReadonlyArray<CrossSectionTable> =
  readCrossSections(crossSectionsFile);

/**
 * The pairs' cross-sections the client holds (`absorbers/oxygenPairs.json`, R08.T4.d, and
 * `absorbers/crossSections.json`'s `pairs`, R08.T4.c), m⁵ a molecule pair on vacuum wavelengths
 * over 380–800 nm, binned to 1 nm, linear in T unless stated:
 *
 * - `O2+O2`, O₂–O₂ collision-induced absorption from HITRAN2024's file (Terragni et al. 2025;
 *   Karman et al. 2019, Icarus 328, 160): its double transitions near 380, 446, 477, 532, 577
 *   and 630 nm (the sets' own peaks: 380.3, 446.5, 477.0, 532.8, 577.7 and 630.1 nm),
 *   Finkenzeller and Volkamer 2022's sets over 380–499 nm (with Thalman and Volkamer's 203 K set
 *   below their 223 K) and Thalman and Volkamer 2013's over 500–653 nm; the B band (676–703 nm)
 *   and the A band (747–760 nm in the range) from Karman et al. 2018's computed sets at 206–346 K
 *   (Nature Chem. 10, 549; HITRAN's alternate file `O2-O2_theory_temp_2018.cia`), in place of
 *   R08.T4.c's single temperatures (`decision-r08-t4b-t12c.md`, follow-up 5, ruling 7); and the
 *   bins those do not cover whole at their one temperature, held at every other (Spiering et al.
 *   2011's 672–675 nm at 293 K; Tran et al. 2006's 733–746 nm at 296 K, up to 15% of the 760 nm
 *   bin). Measured over 206–287 K, where each double-transition bin lies between two of the
 *   measurements' sets (follow-up 3, ruling 7) and each B- and A-band bin within Karman's, and
 *   held, `estimated`, beyond. Earth's vertical pair column, 1.28 × 10⁴³ cm⁻⁵ on Design note 3's
 *   column (US Standard Atmosphere 1976's is 1.32 × 10⁴³), gives τ ≈ 0.009 at 477 nm;
 * - `O2+N2`, O₂–N₂'s collision-induced absorption in the A band: Karman et al. 2018's computed sets
 *   at 206–346 K over 747–760 nm (HITRAN's alternate file `O2-N2_theory_temp_2018.cia`), and
 *   HITRAN2024's O₂–N₂ file below (Tran et al. 2006, nonzero from 733 nm, at 296 K, held), measured
 *   over 206–346 K. The two meet at 746/747 nm with a step: at 296 K Tran et al.'s 746 nm bin is
 *   10.0 times Karman et al.'s 747 nm, at 1.7% and 0.17% of the 760 nm bin (O₂–O₂'s meet within
 *   3%, at 746/747 nm and 675/676 nm alike);
 * - `O2+CO2`, O₂–CO₂'s in the A band, from HITRAN2024's O₂–CO₂ file (Vangvichith, Tran and
 *   Hartmann 2009, at 296 K, measured there alone; Karman et al. 2018 computed no O₂–CO₂ set),
 *   nonzero over 733–760 nm in the range and 9.5 times O₂–N₂'s Tran et al. set at 760 nm;
 * - `H2+He`, Abel et al. 2012's ab initio coefficients (J. Chem. Phys. 136, 044319) from
 *   HITRAN's file at 26 of its temperatures, 200–9,900 K, ln σ linear in T (`lnLinear`), the sets
 *   of 950–1,175 K skipped for breaking the trend (the tool's `HYDROGEN_HELIUM_TEMPERATURES_K`):
 *   the file ends at 20,000 cm⁻¹, so it covers 501–760 nm and is 0 below;
 * - `H2+H2`, H₂–H₂ collision-induced absorption from Borysow 2002 (A&A 390, 779; DOI
 *   10.1051/0004-6361:20020555), equilibrium hydrogen, reduced to 1 nm cell means where its tables
 *   reach, 607–760 nm at 60–350 K and 589–760 nm at 400–1,000 K (597 nm at 600 K), and from
 *   Borysow, Jørgensen and Fu 2001 (J. Quant. Spectrosc. Radiat. Transfer 68, 235) over 501–760 nm
 *   at 2,000–7,000 K; ln σ linear in T (`lnLinear`), measured over 60–7,000 K. It is 0 below, where
 *   the tables end (below 1,000 K the 5–0 overtone is not tabulated), a stated approximation
 *   (`decision-r08-cmf-licence.md` §8). HITRAN's H₂–H₂ file ends at 1 µm.
 *
 * - the alkali resonance doublets' far wings beyond the atoms' 25 cm⁻¹ shape (R08.T4.e,
 *   `absorbers/alkaliPairs.json`; follow-up 5, ruling 2), linear in the perturber's density:
 *   `Na+H2` from N. F. Allard et al. 2019's open tables (Astron. Astrophys. 628, A120), their
 *   one-perturber wing, measured over 500–3,000 K; and `Na+He`, `K+H2` and `K+He`, the ruled
 *   estimate (decision-r08-t4ef-storage §6), R08.T4.c's impact (Lorentz) wing at the atoms' widths,
 *   `estimated` at every temperature and wavelength. The atoms' own tables hold each line within
 *   25 cm⁻¹ of its centre, so nothing is counted twice.
 *
 * In closed form ({@link pairCrossSectionAt}): H⁻'s free–free, the `H+e-` pair, and He⁻'s, the
 * `He+e-` pair. Pending, with its estimated cell means (`absorbers/transparency.ts`'s
 * `PENDING_ABSORBERS`): H₂⁻'s free–free (`H2+e-`, Bell 1980's table not obtained).
 */
export const ABSORBER_PAIR_CROSS_SECTIONS: ReadonlyArray<PairCrossSectionTable> =
  readPairCrossSections(
    { pairs: [...oxygenPairsFile.pairs, ...crossSectionsFile.pairs, ...alkaliPairsFile.pairs] },
    "oxygenPairs.json, crossSections.json or alkaliPairs.json",
  );

/** A species' cross-sections at one temperature, on its table's grid. */
export interface SpeciesCrossSection {
  /** The registry key. */
  readonly species: string;
  /**
   * `measured` within the table's measured range, by its {@link TemperatureLaw}; `estimated`
   * beyond it, the nearest end's values held.
   */
  readonly provenance: Exclude<OpticsProvenance, "none">;
  /** The temperature asked for, K. */
  readonly temperatureK: number;
  /** The first grid point, nm. */
  readonly firstNm: number;
  /** The grid's spacing, nm. */
  readonly stepNm: number;
  /** σ at each grid point, m² a molecule. */
  readonly crossSectionsM2: Float64Array;
  /** The data's citation. */
  readonly source: string;
}

/**
 * How certain an absorber's optics are: `measured`, or `estimated`, either beyond its table's
 * temperatures, the nearest end held (`temperature`), or as a stated estimate in part at every
 * temperature (`optics`; R08.T4.d).
 */
export type OpticsCertainty =
  | { readonly provenance: "measured" }
  | { readonly provenance: "estimated"; readonly estimatedBy: "temperature" | "optics" };

/** A molecule pair's cross-sections at one temperature, on its table's grid (R08.T4.c). */
export type PairCrossSection = PairCrossSectionData & OpticsCertainty;

/**
 * A {@link PairCrossSection}'s values: `estimated` by `optics` at every temperature where the table
 * holds estimated wavelengths with σ above 0 (H₂O's continuum below 567 nm), by `temperature`
 * beyond its measured range.
 */
export interface PairCrossSectionData {
  /** The pair's key, its members' registry keys joined by `+`. */
  readonly pair: string;
  /** The members' registry keys, as the pair's key names them. */
  readonly members: readonly [string, string];
  /** The temperature asked for, K. */
  readonly temperatureK: number;
  /** The first grid point, nm. */
  readonly firstNm: number;
  /** The grid's spacing, nm. */
  readonly stepNm: number;
  /** σ at each grid point, m⁵ a molecule pair. */
  readonly crossSectionsM5: Float64Array;
  /** The data's citation. */
  readonly source: string;
}

/** The certainty a temperature law's provenance gives, `optics` overriding it where set. */
function certaintyOf(byTemperature: "measured" | "estimated", optics: boolean): OpticsCertainty {
  if (optics) {
    return { provenance: "estimated", estimatedBy: "optics" };
  }
  return byTemperature === "estimated"
    ? { provenance: "estimated", estimatedBy: "temperature" }
    : { provenance: "measured" };
}

/** The less certain of two certainties, either possibly absent: `optics` before `temperature`. */
function lessCertain(
  a: OpticsCertainty | undefined,
  b: OpticsCertainty | undefined,
): OpticsCertainty | undefined {
  const estimated = [a, b].flatMap((c) => (c?.provenance === "estimated" ? [c] : []));
  return estimated.find((c) => c.estimatedBy === "optics") ?? estimated[0] ?? a ?? b;
}

/** A table's temperatures, law and values, whatever their unit: what the laws read. */
interface TemperatureGrid {
  /** For messages: the species or the pair. */
  readonly label: string;
  readonly temperaturesK: ReadonlyArray<number>;
  readonly temperatureLaw: TemperatureLaw;
  readonly measuredRangeK: readonly [number, number];
  /** Where the law is applied, the measured range if absent. */
  readonly lawRangeK?: readonly [number, number];
  readonly values: ReadonlyArray<Float64Array>;
}

/** Refuses a temperature that is not finite and above 0 K. */
function checkTemperature(temperatureK: number): void {
  if (!(Number.isFinite(temperatureK) && temperatureK > 0)) {
    throw new RangeError(
      `a cross-section's temperature is finite and above 0 K, not ${temperatureK}`,
    );
  }
}

/**
 * A grid's values at a temperature by its law within its law's range (its measured range unless
 * given), the nearest end's held beyond it, and the provenance that follows: `measured` within the
 * measured range only.
 */
function gridAt(
  grid: TemperatureGrid,
  temperatureK: number,
): { readonly values: Float64Array; readonly provenance: "measured" | "estimated" } {
  const [lowK, highK] = grid.measuredRangeK;
  const [lawLowK, lawHighK] = grid.lawRangeK ?? grid.measuredRangeK;
  const atK = Math.min(Math.max(temperatureK, lawLowK), lawHighK);
  let values: Float64Array;
  switch (grid.temperatureLaw) {
    case "linear":
      values = linearInTemperature(grid, atK);
      break;
    case "lnQuadratic":
      values = lnQuadraticInTemperature(grid, atK);
      break;
    case "lnLinear":
      values = lnLinearInTemperature(grid, atK);
      break;
    case "lnLnLinear":
      values = lnLnLinearInTemperature(grid, atK);
      break;
    case "constant":
      values = Float64Array.from(grid.values[0] ?? []);
      break;
  }
  return {
    values,
    provenance: temperatureK < lowK || temperatureK > highK ? "estimated" : "measured",
  };
}

/**
 * The grid the closed forms are evaluated on: 380–800 nm at 1 nm, the committed tables' span and
 * finest smooth step.
 */
const CLOSED_FORM_GRID = { firstNm: 380, stepNm: 1, points: 421 } as const;

/** A closed form evaluated on {@link CLOSED_FORM_GRID}. */
function onClosedFormGrid(value: (wavelengthNm: number) => number): Float64Array {
  return Float64Array.from({ length: CLOSED_FORM_GRID.points }, (_, i) =>
    value(CLOSED_FORM_GRID.firstNm + i * CLOSED_FORM_GRID.stepNm),
  );
}

/** John 1988's citation, for the closed forms' sources. */
const JOHN_1988 =
  "T. L. John, Continuous absorption by the negative hydrogen ion reconsidered, Astron. " +
  "Astrophys. 193 (1988) 189-192 (ADS 1988A&A...193..189J)";

/**
 * A species' cross-sections at a temperature, or `undefined` if the client holds none for it.
 *
 * @remarks
 * By the table's {@link TemperatureLaw} within its measured range. Beyond it the nearest end's
 * values are held and the result is `estimated`: methane above 300 K is R08.T4.f's. A key no table
 * holds may have a closed form (R08.T4.c): `H-`, H⁻'s bound–free cross-section by John 1988's
 * Eqs. 4–5 with the stimulated-emission factor at the temperature (`absorbers/hMinus.ts`),
 * `measured` at every temperature, since the fit holds over all of 125–1,641.9 nm and the factor
 * is exact.
 *
 * @param tables - The tables to look in; the committed ones unless a test gives its own.
 * @throws RangeError for a temperature that is not finite and above 0 K.
 */
export function crossSectionAt(
  species: string,
  temperatureK: number,
  tables: ReadonlyArray<CrossSectionTable> = ABSORBER_CROSS_SECTIONS,
): SpeciesCrossSection | undefined {
  checkTemperature(temperatureK);
  const table = tables.find((t) => t.species === species);
  if (table === undefined) {
    return species === "H-"
      ? {
          species,
          provenance: "measured",
          temperatureK,
          firstNm: CLOSED_FORM_GRID.firstNm,
          stepNm: CLOSED_FORM_GRID.stepNm,
          crossSectionsM2: onClosedFormGrid((nm) => hMinusBoundFreeM2(nm, temperatureK)),
          source: `${JOHN_1988}, Eqs. 3-5 and Table 2: bound-free, per H- ion, with the stimulated-emission factor`,
        }
      : undefined;
  }
  const { values, provenance } = gridAt(
    {
      label: table.species,
      temperaturesK: table.temperaturesK,
      temperatureLaw: table.temperatureLaw,
      measuredRangeK: table.measuredRangeK,
      values: table.crossSectionsM2,
    },
    temperatureK,
  );
  return {
    species,
    provenance,
    temperatureK,
    firstNm: table.firstNm,
    stepNm: table.stepNm,
    crossSectionsM2: values,
    source: table.source,
  };
}

/** A pair held in closed form: its members, the temperatures its source spans, and its σ. */
interface ClosedFormPair {
  readonly pair: string;
  readonly members: readonly [string, string];
  /** The source's temperatures, K: `measured` within, the nearer end held beyond. */
  readonly measuredRangeK: readonly [number, number];
  /** σ, m⁵ a pair, at a vacuum wavelength in nm and a temperature within the range. */
  readonly crossSectionM5: (wavelengthNm: number, temperatureK: number) => number;
  readonly source: string;
}

/**
 * The pairs held in closed form (R08.T4.c; `decision-r08-t4b-t12c.md`, follow-up 3): H⁻'s
 * free–free absorption, the `H+e-` pair, by John 1988's Eq. 6, and He⁻'s, the `He+e-` pair, from
 * John 1994's Table 2 ({@link pairCrossSectionAt}).
 */
const CLOSED_FORM_PAIR_SOURCES: ReadonlyArray<ClosedFormPair> = [
  {
    pair: "H+e-",
    members: ["H", "e-"],
    measuredRangeK: H_MINUS_FREE_FREE_RANGE_K,
    crossSectionM5: hMinusFreeFreeM5,
    source: `${JOHN_1988}, Eq. 6 and Tables 3a-3b: free-free, per H atom and electron, k_ff k T`,
  },
  {
    pair: "He+e-",
    members: ["He", "e-"],
    measuredRangeK: HE_MINUS_FREE_FREE_RANGE_K,
    crossSectionM5: heMinusFreeFreeM5,
    source:
      "T. L. John, Absorption coefficients of the negative helium ion, Mon. Not. R. Astron. Soc. " +
      "269 (1994) 871-878 (ADS 1994MNRAS.269..871J), Table 2: free-free, per He atom and " +
      "electron, K k T, A = K/lambda^2 interpolated linearly in 1/lambda and in 5040/T, held " +
      "below 506.3 nm",
  },
];

/** The pairs held in closed form, by key and members (R08.T4.c). */
export const CLOSED_FORM_PAIRS: ReadonlyArray<{
  readonly pair: string;
  readonly members: readonly [string, string];
}> = CLOSED_FORM_PAIR_SOURCES.map(({ pair, members }) => ({ pair, members }));

/**
 * A molecule pair's cross-sections at a temperature, or `undefined` if the client holds none for
 * it (R08.T4.c).
 *
 * @remarks
 * As {@link crossSectionAt}, from the pair tables. The closed forms: `H+e-`, H⁻'s free–free
 * absorption, John 1988's Eq. 6 as k_ff k_B T (`absorbers/hMinus.ts`), and `He+e-`, He⁻'s, John
 * 1994's Table 2 as K k_B T (`absorbers/heliumMinus.ts`), each `measured` over its source's
 * 1,400–10,080 K and held at the nearer end, `estimated`, beyond.
 *
 * @param tables - The pair tables to look in; the committed ones unless a test gives its own.
 * @throws RangeError for a temperature that is not finite and above 0 K.
 */
export function pairCrossSectionAt(
  pair: string,
  temperatureK: number,
  tables: ReadonlyArray<PairCrossSectionTable> = ABSORBER_PAIR_CROSS_SECTIONS,
): PairCrossSection | undefined {
  checkTemperature(temperatureK);
  const table = tables.find((t) => t.pair === pair);
  if (table === undefined) {
    const closed = CLOSED_FORM_PAIR_SOURCES.find((c) => c.pair === pair);
    if (closed === undefined) {
      return undefined;
    }
    const [lowK, highK] = closed.measuredRangeK;
    const atK = Math.min(Math.max(temperatureK, lowK), highK);
    return {
      pair,
      members: closed.members,
      ...certaintyOf(temperatureK < lowK || temperatureK > highK ? "estimated" : "measured", false),
      temperatureK,
      firstNm: CLOSED_FORM_GRID.firstNm,
      stepNm: CLOSED_FORM_GRID.stepNm,
      crossSectionsM5: onClosedFormGrid((nm) => closed.crossSectionM5(nm, atK)),
      source: closed.source,
    };
  }
  const { values, provenance } = gridAt(
    {
      label: table.pair,
      temperaturesK: table.temperaturesK,
      temperatureLaw: table.temperatureLaw,
      measuredRangeK: table.measuredRangeK,
      lawRangeK: table.lawRangeK,
      values: table.crossSectionsM5,
    },
    temperatureK,
  );
  const estimatedBelow = table.estimatedBelowNm;
  const estimatedBands =
    estimatedBelow !== undefined &&
    values.some((v, i) => v > 0 && table.firstNm + i * table.stepNm < estimatedBelow);
  return {
    pair,
    members: table.members,
    ...certaintyOf(provenance, estimatedBands),
    temperatureK,
    firstNm: table.firstNm,
    stepNm: table.stepNm,
    crossSectionsM5: values,
    source: table.source,
  };
}

/**
 * Values linear in T between the two tabulated temperatures around it, or from the nearest pair
 * beyond them, floored at 0; a tabulated temperature's own values there.
 */
function linearInTemperature(table: TemperatureGrid, temperatureK: number): Float64Array {
  const { temperaturesK, values: columns } = table;
  const last = temperaturesK.length - 1;
  const exact = temperaturesK.indexOf(temperatureK);
  if (exact >= 0 || last === 0) {
    return Float64Array.from(columns[Math.max(exact, 0)] ?? []);
  }
  let k = 0;
  while (k < last - 1 && (temperaturesK[k + 1] ?? Number.POSITIVE_INFINITY) < temperatureK) {
    k += 1;
  }
  const below = columns[k] ?? new Float64Array(0);
  const above = columns[k + 1] ?? new Float64Array(0);
  const lowK = temperaturesK[k] ?? Number.NaN;
  const t = (temperatureK - lowK) / ((temperaturesK[k + 1] ?? Number.NaN) - lowK);
  return below.map((v, i) => Math.max(v + ((above[i] ?? Number.NaN) - v) * t, 0));
}

/**
 * ln σ linear in T between the two tabulated temperatures around it; at a point with a 0 at either,
 * {@link linearInTemperature}'s value there. The law's range is the table's, so it never
 * extrapolates ({@link gridAt} holds the ends).
 */
function lnLinearInTemperature(table: TemperatureGrid, temperatureK: number): Float64Array {
  const { temperaturesK, values: columns } = table;
  const last = temperaturesK.length - 1;
  const exact = temperaturesK.indexOf(temperatureK);
  if (exact >= 0 || last === 0) {
    return Float64Array.from(columns[Math.max(exact, 0)] ?? []);
  }
  const linear = linearInTemperature(table, temperatureK);
  let k = 0;
  while (k < last - 1 && (temperaturesK[k + 1] ?? Number.POSITIVE_INFINITY) < temperatureK) {
    k += 1;
  }
  const below = columns[k] ?? new Float64Array(0);
  const above = columns[k + 1] ?? new Float64Array(0);
  const lowK = temperaturesK[k] ?? Number.NaN;
  const t = (temperatureK - lowK) / ((temperaturesK[k + 1] ?? Number.NaN) - lowK);
  return below.map((a, i) => {
    const b = above[i] ?? Number.NaN;
    return a > 0 && b > 0
      ? Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * t)
      : (linear[i] ?? Number.NaN);
  });
}

/**
 * A power law in T through a table's two temperatures, ln σ linear in ln T, extrapolated as the
 * law's range allows; at a point with a 0 at either, {@link linearInTemperature}'s value there.
 */
function lnLnLinearInTemperature(table: TemperatureGrid, temperatureK: number): Float64Array {
  const [t0, t1] = table.temperaturesK;
  const [s0, s1] = table.values;
  if (t0 === undefined || t1 === undefined || s0 === undefined || s1 === undefined) {
    throw new Error(`${table.label}'s power law needs two temperatures`);
  }
  const linear = linearInTemperature(table, temperatureK);
  const f = Math.log(temperatureK / t0) / Math.log(t1 / t0);
  return s0.map((a, i) => {
    const b = s1[i] ?? Number.NaN;
    return a > 0 && b > 0
      ? Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * f)
      : (linear[i] ?? Number.NaN);
  });
}

/**
 * ln σ the Lagrange quadratic through a table's three temperatures (Eq. 8); at a point with a 0 at
 * any of the three, {@link linearInTemperature}'s value there.
 */
function lnQuadraticInTemperature(table: TemperatureGrid, temperatureK: number): Float64Array {
  const [t0, t1, t2] = table.temperaturesK;
  const [s0, s1, s2] = table.values;
  if (
    t0 === undefined ||
    t1 === undefined ||
    t2 === undefined ||
    s0 === undefined ||
    s1 === undefined ||
    s2 === undefined
  ) {
    throw new Error(`${table.label}'s quadratic law needs three temperatures`);
  }
  const exact = table.temperaturesK.indexOf(temperatureK);
  if (exact >= 0) {
    return Float64Array.from(table.values[exact] ?? []);
  }
  const linear = linearInTemperature(table, temperatureK);
  const t = temperatureK;
  const l0 = ((t - t1) * (t - t2)) / ((t0 - t1) * (t0 - t2));
  const l1 = ((t - t0) * (t - t2)) / ((t1 - t0) * (t1 - t2));
  const l2 = ((t - t0) * (t - t1)) / ((t2 - t0) * (t2 - t1));
  return s1.map((b, i) => {
    const a = s0[i] ?? Number.NaN;
    const c = s2[i] ?? Number.NaN;
    return a > 0 && b > 0 && c > 0
      ? Math.exp(l0 * Math.log(a) + l1 * Math.log(b) + l2 * Math.log(c))
      : (linear[i] ?? Number.NaN);
  });
}

/**
 * One node of an absorber's vertical profile (plan R08, R08.T6.h; P14.T24.c as amended,
 * `decision-r08-t6d-findings.md` §2): a pressure and the species' mixing ratio there, relative, in
 * any unit, since the layer's column scales it.
 */
export interface AbsorberProfileNode {
  /** p, Pa, finite and above 0. */
  readonly pressurePa: number;
  /** The relative mixing ratio, finite and at least 0. */
  readonly mixingRatio: number;
}

/**
 * One absorbing species on its layer, as a body's record gives it (decision-composition §1.9 and
 * §2: P14.T24.c's `absorbers[]`, `{ species, column_m2, base_pa, top_pa }`, with ozone's profile
 * since R08.T6.h, or a gas of T24.a's fractions that absorbs, such as methane, from the datum to the
 * top).
 */
export interface AbsorberLayer {
  /** The registry key. */
  readonly species: string;
  /** The species' vertical column, molecules m⁻², at least 0. */
  readonly columnPerM2: number;
  /**
   * The layer's base, Pa: without a profile, the mixing ratio is constant from here up to the top,
   * and 0 outside.
   */
  readonly basePa: number;
  /** The layer's top, Pa, below the base; 0 or less than the column's top for no top. */
  readonly topPa: number;
  /**
   * The mixing ratio's shape through the layer (R08.T6.h): at least two nodes, their pressures
   * descending from {@link AbsorberLayer.basePa} at the first to {@link AbsorberLayer.topPa} at the
   * last, read by {@link profileMixingRatio}. Absent, the mixing ratio is constant.
   */
  readonly profile?: ReadonlyArray<AbsorberProfileNode>;
}

/**
 * An absorber profile's mixing ratio at a pressure (plan R08, R08.T6.h; P14.T24.c as amended):
 * linear in ln x against ln p between the nodes about it, linear in x where either node's is 0, and
 * 0 outside the first and last nodes.
 *
 * @remarks
 * Between two nodes the mixing ratio goes as a power of pressure, so exponentially in height on an
 * isothermal stretch of column, which suits a profile spanning decades (Earth's ozone spans four,
 * `earth.ts`'s `EARTH_OZONE_PROFILE`); a node of 0, where the log fails, takes the linear rule. The
 * profile is assumed checked ({@link absorberSpecs}).
 *
 * @param profile - Nodes whose pressures descend.
 */
export function profileMixingRatio(
  profile: ReadonlyArray<AbsorberProfileNode>,
  pressurePa: number,
): number {
  const first = profile[0];
  const last = profile.at(-1);
  if (
    first === undefined ||
    last === undefined ||
    !(pressurePa <= first.pressurePa && pressurePa >= last.pressurePa)
  ) {
    return 0;
  }
  let i = 0;
  while (i < profile.length - 2 && (profile[i + 1]?.pressurePa ?? Number.NaN) > pressurePa) {
    i += 1;
  }
  const a = profile[i] ?? first;
  const b = profile[i + 1] ?? last;
  if (pressurePa === a.pressurePa) {
    return a.mixingRatio;
  }
  if (pressurePa === b.pressurePa) {
    return b.mixingRatio;
  }
  const t = Math.log(a.pressurePa / pressurePa) / Math.log(a.pressurePa / b.pressurePa);
  return a.mixingRatio > 0 && b.mixingRatio > 0
    ? Math.exp(Math.log(a.mixingRatio) + t * Math.log(b.mixingRatio / a.mixingRatio))
    : a.mixingRatio + t * (b.mixingRatio - a.mixingRatio);
}

/**
 * A layer's profile checked against the layer: at least two nodes, finite, their pressures
 * descending from its base to its top, their mixing ratios at least 0 and some above 0.
 *
 * @throws RangeError for a profile that breaks any of these.
 */
function checkProfile(layer: AbsorberLayer): void {
  const { profile, species } = layer;
  if (profile === undefined) {
    return;
  }
  if (profile.length < 2) {
    throw new RangeError(
      `absorber ${species}'s profile has at least two nodes, not ${profile.length}`,
    );
  }
  for (const [i, { pressurePa, mixingRatio }] of profile.entries()) {
    const above = i === 0 ? Number.POSITIVE_INFINITY : (profile[i - 1]?.pressurePa ?? Number.NaN);
    if (!(Number.isFinite(pressurePa) && pressurePa > 0 && pressurePa < above)) {
      throw new RangeError(
        `absorber ${species}'s profile descends in pressure, finite and above 0: ${pressurePa} Pa at node ${i}`,
      );
    }
    if (!(Number.isFinite(mixingRatio) && mixingRatio >= 0)) {
      throw new RangeError(
        `absorber ${species}'s profile's mixing ratio is finite and at least 0: ${mixingRatio} at node ${i}`,
      );
    }
  }
  if (profile[0]?.pressurePa !== layer.basePa || profile.at(-1)?.pressurePa !== layer.topPa) {
    throw new RangeError(
      `absorber ${species}'s profile runs from its base, ${layer.basePa} Pa, to its top, ${layer.topPa} Pa`,
    );
  }
  if (!profile.some(({ mixingRatio }) => mixingRatio > 0)) {
    throw new RangeError(`absorber ${species}'s profile has a mixing ratio above 0`);
  }
}

/**
 * A key equal for two profiles exactly when they are equal node for node, or both absent: each
 * number's shortest round-trip form, which tells every two doubles apart.
 */
function profileKey(profile: ReadonlyArray<AbsorberProfileNode> | undefined): string {
  return profile === undefined
    ? "constant"
    : profile.map(({ pressurePa, mixingRatio }) => `${pressurePa}/${mixingRatio}`).join(",");
}

/** One species of an absorber spec. */
export interface AbsorberSpecies {
  /** The registry key. */
  readonly species: string;
  /** Its vertical column, molecules m⁻². */
  readonly columnPerM2: number;
  /** Its cross-sections at the spec's temperature; `undefined` where the client holds none. */
  readonly crossSection: SpeciesCrossSection | undefined;
  /**
   * Its lines (R08.T4.d), absent for a species the client draws by cross-sections alone: their
   * k-distributions at the spec's temperature and their broadening factor in the layer's mixture
   * ({@link SpeciesLines}).
   */
  readonly lines?: SpeciesLines;
}

/**
 * A species' molecular lines in a spec (R08.T4.d): each 1 nm cell's k-distribution at the spec's
 * temperature, read at a path's absorber-weighted (Curtis–Godson) pressure times the broadening
 * factor.
 */
export type SpeciesLines = SpeciesLinesData & OpticsCertainty;

/**
 * A {@link SpeciesLines}' tables: `measured` where the k-distribution is at the layer's temperature
 * and every broadening pressure up to the layer's base lies within the measured line shape and the
 * stored grid, with a width for every broadening gas but those the effect bound weighs
 * ({@link SpeciesLinesData.widthlessShare}); otherwise `estimated`, by `temperature` where it is the
 * temperature alone (`decision-r08-t4d-kdist.md` §3 and §5).
 */
export interface SpeciesLinesData {
  readonly kDistribution: SpeciesKDistribution;
  /**
   * The air-equivalent broadening pressure over the pressure, from the layer's mixture
   * (`absorbers/kDistribution.ts`'s `broadeningFactor`): 1 for air.
   */
  readonly broadeningFactor: number;
  /**
   * ε, the share of the broadening pressure that rests on gases the table holds no width for,
   * taken as air (`broadeningFactor`'s `widthlessShare`), 0 where there are none: these no longer
   * make the lines `estimated` by themselves, and R08.T10.c's effect bound weighs them instead
   * (`negligible.ts`'s `BroadenerVerdict`; `decision-r08-approximate-plate.md`, item 1a).
   */
  readonly widthlessShare: number;
}

/** The highest stored broadening pressure, Pa: above it the 10 MPa table is held, `estimated`. */
export const LINE_TABLE_TOP_PA = 1e7;

/**
 * The absorbing species on one vertical profile, which share one curve of growth a sun (Design
 * note 5; decision-composition §1.9): a layer between two pressures of a column, in which each
 * species' mixing ratio is constant.
 */
export interface ColumnAbsorberSpec {
  readonly kind: "column";
  /** The term's name, `absorber:` and the species' keys joined by `+`, unique within a medium. */
  readonly name: string;
  /** The species, in the order first listed, each once. */
  readonly species: ReadonlyArray<AbsorberSpecies>;
  /** Their summed vertical column U, molecules m⁻², above 0: the curve's u is a column of the mixture. */
  readonly columnPerM2: number;
  /** The layer's base on its column, Pa. */
  readonly basePa: number;
  /** The layer's top on its column, Pa. */
  readonly topPa: number;
  /**
   * Its species' shared mixing-ratio profile (R08.T6.h), as their layers give it, read only between
   * {@link ColumnAbsorberSpec.basePa} and {@link ColumnAbsorberSpec.topPa}; absent, constant.
   */
  readonly profile?: ReadonlyArray<AbsorberProfileNode>;
  /** The temperature the cross-sections are taken at, K: the layer's column-weighted mean. */
  readonly temperatureK: number;
}

/** One molecule pair of a pair spec (R08.T4.c). */
export interface AbsorberPair {
  /** The pair's key, its members' registry keys joined by `+` (`O2+O2`, `H+e-`). */
  readonly pair: string;
  /** Its vertical pair column ∫ n_A n_B dz, molecule pairs m⁻⁵. */
  readonly pairColumnPerM5: number;
  /** Its cross-sections at the spec's temperature. */
  readonly crossSection: PairCrossSection;
}

/**
 * The molecule pairs on one vertical profile, which share one curve of growth a sun (R08.T4.c;
 * `decision-r08-t4b-t12c.md`, follow-up 2): the overlap of their members' layers, where each
 * member's mixing ratio is constant, so that every pair's density is a constant multiple of the
 * column's n² there, and σ_mix = Σ (Uᵢ ÷ U) σᵢ is exact for every path.
 */
export interface PairAbsorberSpec {
  readonly kind: "pair";
  /** The term's name, `pair:` and the pairs' keys joined by `,` (`pair:O2+O2,H2+He`). */
  readonly name: string;
  /** The pairs, in the order formed, each once. */
  readonly pairs: ReadonlyArray<AbsorberPair>;
  /** Their summed vertical pair column U, molecule pairs m⁻⁵, above 0: the curve's u is of the mixture. */
  readonly pairColumnPerM5: number;
  /** The overlap's base on its column, Pa. */
  readonly basePa: number;
  /** The overlap's top on its column, Pa. */
  readonly topPa: number;
  /** The temperature the cross-sections are taken at, K: the overlap's pair-weighted mean. */
  readonly temperatureK: number;
}

/**
 * The absorbers on one vertical profile: molecules over their column (T4.b's), or molecule pairs
 * over their pair column (R08.T4.c).
 */
export type AbsorberSpec = ColumnAbsorberSpec | PairAbsorberSpec;

/**
 * An amount of an absorber crossed, in its kind's unit (R08.T4.c): a column absorber's column,
 * molecules m⁻², or a pair absorber's pair column, molecule pairs m⁻⁵. The curves and optics of
 * one kind read only an amount of that kind.
 */
export type AbsorberAmount =
  | { readonly kind: "column"; readonly columnPerM2: number }
  | { readonly kind: "pair"; readonly pairColumnPerM5: number };

/** An amount's value in its kind's unit, molecules m⁻² or molecule pairs m⁻⁵. */
function amountValue(amount: AbsorberAmount): number {
  let value: number;
  switch (amount.kind) {
    case "column":
      value = amount.columnPerM2;
      break;
    case "pair":
      value = amount.pairColumnPerM5;
      break;
  }
  return value;
}

/**
 * A spec's vertical amount U, the abscissa its curves are spaced about: a column spec's column, or a
 * pair spec's pair column.
 */
export function absorberVerticalAmount(spec: AbsorberSpec): AbsorberAmount {
  let amount: AbsorberAmount;
  switch (spec.kind) {
    case "column":
      amount = { kind: "column", columnPerM2: spec.columnPerM2 };
      break;
    case "pair":
      amount = { kind: "pair", pairColumnPerM5: spec.pairColumnPerM5 };
      break;
  }
  return amount;
}

/** An amount times a pure number, of the same kind. */
export function scaledAmount(amount: AbsorberAmount, factor: number): AbsorberAmount {
  let scaled: AbsorberAmount;
  switch (amount.kind) {
    case "column":
      scaled = { kind: "column", columnPerM2: amount.columnPerM2 * factor };
      break;
    case "pair":
      scaled = { kind: "pair", pairColumnPerM5: amount.pairColumnPerM5 * factor };
      break;
  }
  return scaled;
}

/**
 * An amount's value, checked against the kind of the optics or curve that reads it.
 *
 * @throws Error for an amount of the other kind, a molecule column read as a pair column or the
 *   reverse: a caller's bug, many orders of magnitude off.
 * @throws RangeError for a value that is not finite and at least 0.
 */
function amountFor(amount: AbsorberAmount, kind: AbsorberSpec["kind"]): number {
  if (amount.kind !== kind) {
    throw new Error(`a ${amount.kind} amount is read on a ${kind} absorber's optics or curve`);
  }
  const value = amountValue(amount);
  if (!(Number.isFinite(value) && value >= 0)) {
    throw new RangeError(`an absorber's amount is finite and at least 0, not ${value}`);
  }
  return value;
}

/** Why a listed absorber is drawn approximately or not at all, for `atmosphereApproximate`. */
export type AbsorberApproximationReason =
  /** The client holds no cross-sections for the species: it draws nothing. */
  | "noCrossSections"
  /** Its layer's temperature lies beyond its table's measured range: the nearest end is held. */
  | "temperatureOutsideTable"
  /**
   * Its optics are a stated estimate at every temperature (R08.T4.d: an overtone-ladder
   * species' k-distributions).
   */
  | "estimatedOptics"
  /**
   * More vertical profiles than {@link ABSORBER_COLUMNS}, its own of the least optical depth: it
   * draws nothing.
   */
  | "noColumnFree";

/**
 * A listed absorber drawn approximately or not at all: drawn as nothing (`none`) for want of
 * cross-sections or of a column, or drawn `estimated`, with held cross-sections beyond their
 * table's measured temperatures or with estimated optics. `species` is a pair's key (`O2+O2`) for a
 * pair.
 */
export type AbsorberApproximation =
  | {
      readonly species: string;
      readonly provenance: "none";
      readonly reason: Exclude<
        AbsorberApproximationReason,
        "temperatureOutsideTable" | "estimatedOptics"
      >;
    }
  | {
      readonly species: string;
      readonly provenance: "estimated";
      readonly reason: "temperatureOutsideTable" | "estimatedOptics";
    };

/** A body's absorbers merged by profile, and what is drawn approximately or not at all. */
export interface AbsorberSpecs {
  /**
   * At most {@link ABSORBER_COLUMNS}, in the order their first species was listed: past five
   * profiles, the five of greatest {@link absorberRankingDepth}.
   */
  readonly specs: ReadonlyArray<AbsorberSpec>;
  /** R08.T10.a turns these into `atmosphereApproximate`. */
  readonly approximations: ReadonlyArray<AbsorberApproximation>;
}

/** The levels of a layer laid on a column: inside it, with its edges, and the column's levels outside. */
interface LayerOnColumn {
  readonly baseM: number;
  readonly topM: number;
  /** Heights, m, relative densities and temperatures, K, from the base's edge to the top's. */
  readonly altitudesM: ReadonlyArray<number>;
  readonly relative: ReadonlyArray<number>;
  readonly temperaturesK: ReadonlyArray<number>;
  /** The column level just below the base and just above the top, m, if there is one. */
  readonly belowM: number | undefined;
  readonly aboveM: number | undefined;
}

/** The column's pressures clamped to its datum and its top, and the layer's base above its top. */
function clampedLayer(
  column: AtmosphereColumn,
  basePa: number,
  topPa: number,
): { readonly basePa: number; readonly topPa: number } {
  if (!(Number.isFinite(basePa) && Number.isFinite(topPa) && topPa >= 0 && basePa > topPa)) {
    throw new RangeError(
      `an absorber layer's base lies below its top, both finite and the top at least 0: base ${basePa} Pa, top ${topPa} Pa`,
    );
  }
  const datumPa = column.pressuresPa[0] ?? Number.NaN;
  const columnTopPa = column.pressuresPa[column.pressuresPa.length - 1] ?? Number.NaN;
  const base = Math.min(basePa, datumPa);
  const top = Math.max(topPa, columnTopPa);
  if (!(base > top)) {
    throw new RangeError(
      `an absorber layer from ${basePa} Pa to ${topPa} Pa lies outside its column, ${datumPa}–${columnTopPa} Pa`,
    );
  }
  return { basePa: base, topPa: top };
}

/**
 * The height and temperature at a pressure within the column: between the two levels around it,
 * linear in ln p, as the column's levels are spaced.
 */
function atPressure(
  column: AtmosphereColumn,
  pressurePa: number,
): { readonly heightM: number; readonly temperatureK: number } {
  const { pressuresPa, altitudesM, temperaturesK } = column;
  let i = 0;
  while (i < pressuresPa.length - 2 && (pressuresPa[i + 1] ?? Number.NaN) > pressurePa) {
    i += 1;
  }
  const p0 = pressuresPa[i] ?? Number.NaN;
  const p1 = pressuresPa[i + 1] ?? Number.NaN;
  const t = Math.min(Math.max(Math.log(p0 / pressurePa) / Math.log(p0 / p1), 0), 1);
  const z0 = altitudesM[i] ?? Number.NaN;
  const z1 = altitudesM[i + 1] ?? Number.NaN;
  const k0 = temperaturesK[i] ?? Number.NaN;
  const k1 = temperaturesK[i + 1] ?? Number.NaN;
  return { heightM: z0 + (z1 - z0) * t, temperatureK: k0 + (k1 - k0) * t };
}

/**
 * A layer between two pressures, clamped to the column, laid on the column's levels: its relative
 * density the column's, times the profile's mixing ratio at each level's pressure where it has one
 * (R08.T6.h).
 *
 * @remarks
 * A profile is read at the column's levels alone, 0.0157 apart in ln p (`column.ts`'s
 * `COLUMN_INTERVALS`), a tenth of the AFGL template's 1 km spacing in the stratosphere, so that its
 * nodes' kinks fall between levels and are taken linearly there.
 */
function layerOnColumn(
  column: AtmosphereColumn,
  basePa: number,
  topPa: number,
  profile?: ReadonlyArray<AbsorberProfileNode>,
): LayerOnColumn {
  const mixing = (pressurePa: number): number =>
    profile === undefined ? 1 : profileMixingRatio(profile, pressurePa);
  const base = atPressure(column, basePa);
  const top = atPressure(column, topPa);
  const altitudesM = [base.heightM];
  const relative = [densityAt(column.density, base.heightM) * mixing(basePa)];
  const temperaturesK = [base.temperatureK];
  let belowM: number | undefined;
  let aboveM: number | undefined;
  for (const [i, h] of column.altitudesM.entries()) {
    if (h < base.heightM) {
      belowM = h;
    } else if (h > base.heightM && h < top.heightM) {
      altitudesM.push(h);
      relative.push(
        (column.density.relative[i] ?? Number.NaN) * mixing(column.pressuresPa[i] ?? Number.NaN),
      );
      temperaturesK.push(column.temperaturesK[i] ?? Number.NaN);
    } else if (h > top.heightM && aboveM === undefined) {
      aboveM = h;
    }
  }
  altitudesM.push(top.heightM);
  relative.push(densityAt(column.density, top.heightM) * mixing(topPa));
  temperaturesK.push(top.temperatureK);
  return {
    baseM: base.heightM,
    topM: top.heightM,
    altitudesM,
    relative,
    temperaturesK,
    belowM,
    aboveM,
  };
}

/**
 * ∫ ρᵖ dz over a layer, by the trapezoid rule on its levels: p = 1 for a species' column, 2 for a
 * pair's.
 */
function layerIntegralM(layer: LayerOnColumn, power: 1 | 2): number {
  let sum = 0;
  for (let i = 1; i < layer.altitudesM.length; i += 1) {
    const dz = (layer.altitudesM[i] ?? Number.NaN) - (layer.altitudesM[i - 1] ?? Number.NaN);
    const n0 = (layer.relative[i - 1] ?? Number.NaN) ** power;
    const n1 = (layer.relative[i] ?? Number.NaN) ** power;
    sum += 0.5 * (n0 + n1) * dz;
  }
  return sum;
}

/**
 * The mean temperature over a layer weighted by ρᵖ, K: ∫ ρᵖ T dz ÷ ∫ ρᵖ dz by the trapezoid rule,
 * the column-weighted mean for p = 1 and the pair-weighted mean for p = 2.
 */
function layerTemperatureK(layer: LayerOnColumn, power: 1 | 2): number {
  let weight = 0;
  let sum = 0;
  for (let i = 1; i < layer.altitudesM.length; i += 1) {
    const dz = (layer.altitudesM[i] ?? Number.NaN) - (layer.altitudesM[i - 1] ?? Number.NaN);
    const n0 = (layer.relative[i - 1] ?? Number.NaN) ** power;
    const n1 = (layer.relative[i] ?? Number.NaN) ** power;
    weight += 0.5 * (n0 + n1) * dz;
    sum +=
      0.5 *
      (n0 * (layer.temperaturesK[i - 1] ?? Number.NaN) +
        n1 * (layer.temperaturesK[i] ?? Number.NaN)) *
      dz;
  }
  return sum / weight;
}

/**
 * A layer as {@link absorberSpecs} lays it on its column, for R08.T10.a's bound (`negligible.ts`):
 * its base and top clamped to the column, and ∫ρᵖ dz and the ρᵖ-weighted mean temperature over it.
 */
export interface LayerMoments {
  /** The base, Pa, clamped to the column's datum. */
  readonly basePa: number;
  /** The top, Pa, clamped to the column's top. */
  readonly topPa: number;
  /** ∫ρᵖ dz by the trapezoid rule on its levels, m, ρ the column's relative density. */
  readonly integralM: number;
  /** ∫ρᵖ T dz ÷ ∫ρᵖ dz, K. */
  readonly temperatureK: number;
}

/**
 * {@link LayerMoments} of a layer between two pressures on a column, its profile's mixing ratio
 * folded in where it has one, by the rules {@link absorberSpecs} takes a profile's temperature and a
 * pair member's density by: p = 1 for a species' column (its column-weighted temperature), 2 for a
 * pair's (its pair-weighted temperature and ∫ρ² dz).
 *
 * @throws RangeError for a base that does not lie below its top, or a layer outside its column.
 */
export function layerMoments(
  column: AtmosphereColumn,
  basePa: number,
  topPa: number,
  power: 1 | 2,
  profile?: ReadonlyArray<AbsorberProfileNode>,
): LayerMoments {
  const clamped = clampedLayer(column, basePa, topPa);
  const layer = layerOnColumn(column, clamped.basePa, clamped.topPa, profile);
  return {
    ...clamped,
    integralM: layerIntegralM(layer, power),
    temperatureK: layerTemperatureK(layer, power),
  };
}

/** A listed layer with a column, clamped to the column, at its place in the list. */
interface ListedLayer {
  readonly index: number;
  readonly layer: AbsorberLayer;
  readonly basePa: number;
  readonly topPa: number;
}

/**
 * A profile on its way to a spec: its place among the profiles, the species or pairs it reports,
 * and its spec if it has anything to draw.
 */
interface BuiltProfile {
  /** A column profile's first species' place in the list, a pair profile's later member's + ½. */
  readonly order: number;
  /** The keys it reports, with the certainty of each one's optics, or none for want of optics. */
  readonly members: ReadonlyArray<{
    readonly key: string;
    readonly certainty: OpticsCertainty | undefined;
  }>;
  readonly spec: AbsorberSpec | undefined;
}

/** The pairs the client can form: the committed pair tables', then the closed forms'. */
function formablePairs(
  pairTables: ReadonlyArray<PairCrossSectionTable>,
): ReadonlyArray<{ readonly pair: string; readonly members: readonly [string, string] }> {
  const held = pairTables.map(({ pair, members }) => ({ pair, members }));
  return [...held, ...CLOSED_FORM_PAIRS.filter((p) => !held.some((h) => h.pair === p.pair))];
}

/**
 * A body's absorbers merged into one spec per vertical profile on its column, each species' cross-
 * sections taken at its profile's temperature (decision-composition §1.9), and the pairs its
 * layers form (R08.T4.c).
 *
 * @remarks
 * Layers merge when their base and top, clamped to the column's datum and top, are equal, and
 * their profiles too (R08.T6.h), node for node or both constant: their mixing ratios are then in a
 * constant ratio in the same air, so σ_mix = Σ xᵢσᵢ, xᵢ = Uᵢ ÷ U, is exact for every path. A layer
 * with a profile is laid on the column as x(p) n(p) between its base and top, clamped; its nodes
 * outside the column are not read. A layer of column 0 has nothing to draw and is left out. A spec's temperature is the
 * layer's column-weighted mean, ∫ n T dz ÷ ∫ n dz, which gives the vertical column's mean σ exactly
 * where σ is linear in T (ozone) and nearly so for methane's Eq. 8; a slant path through part of
 * the layer reads the same σ, a stated approximation. A species `absorbers/transparency.ts` states
 * has no visible absorption, and for which no table is held, takes no column and no label.
 *
 * **Pairs** (`decision-r08-t4b-t12c.md`, follow-up 2). Each pair the client holds a table or a
 * closed form for (`O2+O2`, `O2+N2`, `O2+CO2`, `H2+He`, `H2+H2`, `H+e-`, `He+e-`) is formed when
 * both its members are listed with a column, and not otherwise. Each member's density is c ρ
 * inside its layer, c = U ÷ ∫ ρ dz over it, so the pair's vertical column is c_A c_B ∫ ρ² dz over
 * the overlap of the two layers (for a member paired with itself, c²). Pairs merge when their
 * overlaps are equal, and their spec takes the overlap's pair-weighted mean temperature,
 * ∫ ρ² T dz ÷ ∫ ρ² dz. A pair profile takes a stored
 * column as a column profile does, and is ranked with them; it comes after the later listed of
 * its members. A member whose layer carries a profile forms no pair, nor is it an `air` pair's
 * partner (R08.T6.h): no pair the client forms has such a member today, ozone's being the one
 * profile the record carries, its mixing ratio under 10⁻⁵ (AFGL's peak is 7.6 ppmv).
 *
 * Past {@link ABSORBER_COLUMNS} profiles, the five of greatest optical depth are kept
 * ({@link absorberRankingDepth}; `decision-r08-cmf-licence.md` §2). Equal depths keep the earlier
 * listed, and the kept profiles take their columns in the order listed.
 *
 * Reported, never dropped silently: a species with no cross-sections (`noCrossSections`, drawn as
 * nothing), one whose temperature lies beyond its table's measured range
 * (`temperatureOutsideTable`), and every species of a dropped profile (`noColumnFree`, drawn as
 * nothing), grouped by profile, each profile in the order its first species was listed; a pair
 * is reported by its key. A profile none of whose species has cross-sections takes no column.
 *
 * **Lines** (R08.T4.d). A species with a k-distribution table draws its lines through it, at the
 * profile's temperature, with its broadening factor in the layer's mixture: the bulk gas's mole
 * fractions (`options.bulkMoleFractions`, air if absent, a stated approximation) with the species'
 * own, its column over the layer's number column (the air's, without a profile: its mean mole
 * fraction), taking `self`. A species may hold both lines and cross-sections; it is reported by the
 * less certain of the two.
 *
 * @param tables - The cross-sections to take; the committed ones unless a test gives its own.
 * @param pairTables - The pairs' cross-sections to take, likewise.
 * @param options - The k-distribution tables to take (the committed ones unless a test gives its
 *   own) and the bulk gas's mole fractions by registry key, which broaden the lines.
 * @throws RangeError for a species listed twice, a column that is not finite and at least 0, a
 *   layer whose base does not lie below its top, a layer outside its column, or a profile that
 *   does not descend from the layer's base to its top in finite nodes of mixing ratio at least 0,
 *   or whose mixing ratio is 0 throughout the column.
 */
export function absorberSpecs(
  layers: ReadonlyArray<AbsorberLayer>,
  column: AtmosphereColumn,
  tables: ReadonlyArray<CrossSectionTable> = ABSORBER_CROSS_SECTIONS,
  pairTables: ReadonlyArray<PairCrossSectionTable> = ABSORBER_PAIR_CROSS_SECTIONS,
  options: AbsorberSpecsOptions = {},
): AbsorberSpecs {
  const kTables = options.kDistributions ?? ABSORBER_K_DISTRIBUTIONS;
  const seen = new Set<string>();
  const listed = new Map<string, ListedLayer>();
  const profiles = new Map<
    string,
    {
      readonly basePa: number;
      readonly topPa: number;
      readonly profile: ReadonlyArray<AbsorberProfileNode> | undefined;
      readonly order: number;
      readonly layers: AbsorberLayer[];
    }
  >();
  for (const [index, layer] of layers.entries()) {
    if (seen.has(layer.species)) {
      throw new RangeError(`absorber ${layer.species} is listed twice`);
    }
    seen.add(layer.species);
    if (!(Number.isFinite(layer.columnPerM2) && layer.columnPerM2 >= 0)) {
      throw new RangeError(
        `absorber ${layer.species}'s column is finite and at least 0, not ${layer.columnPerM2} m⁻²`,
      );
    }
    checkProfile(layer);
    const clamped = clampedLayer(column, layer.basePa, layer.topPa);
    if (layer.columnPerM2 === 0) {
      continue;
    }
    const onColumn = layerOnColumn(column, clamped.basePa, clamped.topPa, layer.profile);
    if (!(layerIntegralM(onColumn, 1) > 0)) {
      throw new RangeError(
        `absorber ${layer.species}'s profile has a mixing ratio above 0 within its column`,
      );
    }
    listed.set(layer.species, { index, layer, ...clamped });
    // Below a stated none's limit the species is transparent whatever the client holds for it, a
    // table from a visibility floor among them (the belowK contract, ruled 2026-10-19; R08.T4.e):
    // consulted before any table, so that a floor's table is not held at its first node, labelled,
    // on a colder body. Its pairs stay listed.
    if (isTransparent(layer.species, layerTemperatureK(onColumn, 1))) {
      continue;
    }
    const key = `${clamped.basePa}:${clamped.topPa}:${profileKey(layer.profile)}`;
    const profile = profiles.get(key) ?? {
      ...clamped,
      profile: layer.profile,
      order: index,
      layers: [],
    };
    profile.layers.push(layer);
    profiles.set(key, profile);
  }
  const built: BuiltProfile[] = [...profiles.values()].map((profile) =>
    columnProfile(profile, column, tables, kTables, options.bulkMoleFractions),
  );
  built.push(...pairProfiles(listed, column, pairTables));
  built.sort((a, b) => a.order - b.order);
  const kept = keptProfiles(built.flatMap(({ spec }) => (spec === undefined ? [] : [spec])));
  const specs: AbsorberSpec[] = [];
  const approximations: AbsorberApproximation[] = [];
  for (const { members, spec } of built) {
    const drawn = spec !== undefined && kept.has(spec);
    const dropped = spec !== undefined && !drawn;
    for (const { key, certainty } of members) {
      if (dropped) {
        approximations.push({ species: key, provenance: "none", reason: "noColumnFree" });
      } else if (certainty === undefined) {
        approximations.push({ species: key, provenance: "none", reason: "noCrossSections" });
      } else if (certainty.provenance === "estimated") {
        approximations.push({
          species: key,
          provenance: "estimated",
          reason:
            certainty.estimatedBy === "optics" ? "estimatedOptics" : "temperatureOutsideTable",
        });
      }
    }
    if (drawn) {
      specs.push(spec);
    }
  }
  return { specs, approximations };
}

/** What {@link absorberSpecs} takes beyond its layers, column and cross-section tables (R08.T4.d). */
export interface AbsorberSpecsOptions {
  /** The k-distribution tables to take; the committed ones unless a test gives its own. */
  readonly kDistributions?: ReadonlyArray<KDistributionTable>;
  /**
   * The bulk gas's mole fractions by registry key, which broaden the lines; air (every ratio 1) if
   * absent.
   */
  readonly bulkMoleFractions?: ReadonlyMap<string, number>;
}

/**
 * The mixture a species' lines are broadened in on a layer (R08.T4.d), as {@link absorberSpecs}
 * takes it and R08.T10.a's bound reads it (`negligible.ts`): the species' own mole fraction xₛ, its
 * column over the layer's number column clamped to [0, 1], and the bulk's other gases the rest,
 * 1 − xₛ, in their proportions, or air if no bulk is given.
 */
export function lineBroadeningMixture(
  species: string,
  ownFraction: number,
  bulk: ReadonlyMap<string, number> | undefined,
): ReadonlyMap<string, number> {
  const x = Math.min(Math.max(ownFraction, 0), 1);
  const others = [...(bulk ?? new Map([["air", 1]]))].filter(([gas]) => gas !== species);
  const total = others.reduce((sum, [, f]) => sum + f, 0);
  const mixture = new Map<string, number>(
    total > 0 ? others.map(([gas, f]) => [gas, ((1 - x) * f) / total]) : [["air", 1 - x]],
  );
  mixture.set(species, x);
  return mixture;
}

/**
 * A species' lines in a layer: its k-distributions at the layer's temperature and its broadening
 * factor, its own mole fraction xₛ (its column over the layer's number column) taking `self` and
 * the bulk's gases the rest, 1 − xₛ, in their proportions, or air's ratio if no bulk is given
 * ({@link lineBroadeningMixture}).
 */
function speciesLines(
  table: KDistributionTable,
  temperatureK: number,
  ownFraction: number,
  bulk: ReadonlyMap<string, number> | undefined,
  basePa: number,
): SpeciesLines {
  const mixture = lineBroadeningMixture(table.species, ownFraction, bulk);
  const broadening = broadeningFactor(table, mixture, temperatureK);
  const kDistribution = kDistributionAt(table, temperatureK, broadening.selfShare);
  const deepestPa = basePa * broadening.factor;
  const optics =
    table.basis === "estimated" ||
    kDistribution.inEstimatedInterval ||
    broadening.estimated ||
    !kDistribution.measuredBelowGrid ||
    deepestPa > kDistribution.measuredShapeToPa ||
    deepestPa > LINE_TABLE_TOP_PA;
  const temperature = kDistribution.provenance === "estimated" && table.basis === "measured";
  return {
    kDistribution,
    broadeningFactor: broadening.factor,
    widthlessShare: broadening.widthlessShare,
    ...certaintyOf(temperature ? "estimated" : "measured", optics),
  };
}

/** A column profile's species at its temperature, and its spec if any of them has optics. */
function columnProfile(
  profile: {
    readonly basePa: number;
    readonly topPa: number;
    readonly profile: ReadonlyArray<AbsorberProfileNode> | undefined;
    readonly order: number;
    readonly layers: ReadonlyArray<AbsorberLayer>;
  },
  column: AtmosphereColumn,
  tables: ReadonlyArray<CrossSectionTable>,
  kTables: ReadonlyArray<KDistributionTable>,
  bulk: ReadonlyMap<string, number> | undefined,
): BuiltProfile {
  const temperatureK = layerTemperatureK(
    layerOnColumn(column, profile.basePa, profile.topPa, profile.profile),
    1,
  );
  // The air's number column over the layer, without the profile: a species' column over it is its
  // mean mole fraction there, which its lines' self-broadening takes (R08.T4.d).
  const numberColumnPerM2 =
    column.surfaceNumberDensityPerM3 *
    layerIntegralM(layerOnColumn(column, profile.basePa, profile.topPa), 1);
  const species = profile.layers.map((layer): AbsorberSpecies => {
    const table = kTables.find((t) => t.species === layer.species);
    const base = {
      species: layer.species,
      columnPerM2: layer.columnPerM2,
      crossSection: crossSectionAt(layer.species, temperatureK, tables),
    };
    return table === undefined
      ? base
      : {
          ...base,
          lines: speciesLines(
            table,
            temperatureK,
            layer.columnPerM2 / numberColumnPerM2,
            bulk,
            profile.basePa,
          ),
        };
  });
  const spec: ColumnAbsorberSpec | undefined = species.some(
    (s) => s.crossSection !== undefined || s.lines !== undefined,
  )
    ? {
        kind: "column",
        name: `absorber:${species.map((s) => s.species).join("+")}`,
        species,
        columnPerM2: species.reduce((sum, s) => sum + s.columnPerM2, 0),
        basePa: profile.basePa,
        topPa: profile.topPa,
        ...(profile.profile === undefined ? {} : { profile: profile.profile }),
        temperatureK,
      }
    : undefined;
  return {
    order: profile.order,
    members: species.map((s) => ({
      key: s.species,
      certainty: lessCertain(
        s.crossSection === undefined ? undefined : certaintyOf(s.crossSection.provenance, false),
        s.lines,
      ),
    })),
    spec,
  };
}

/**
 * The share of an {@link AIR_PARTNER} pair's partner molecules that may be gases other than those
 * its σ is measured with, which the pair takes as air: 2% (`decision-r08-t4d-kdist-chain.md` §15,
 * ruling 1). CKD's foreign continuum is defined against the foreign density, every gas but H₂O at
 * Earth air's mix, whose closures were made in real air: its Ar, CO₂ and Ne are 0.97% of the dry
 * gas. Were a trace partner's efficiency 0 or twice air's, it would move the foreign part by at
 * most its share: on Earth 2% of the drawn foreign part's 9.1 × 10⁻³ in red through 38 air masses,
 * 1.8 × 10⁻⁴, and about 4 × 10⁻⁴ along a horizontal ray (`decision-r08-t4d-kdist.md` §6.5; the
 * ruling's 5.7 × 10⁻³ is MT_CKD's). The limit is relative, a stated one: the error scales with the
 * foreign τ, so a humid world whose foreign part passes τ 0.05 in a channel passes 10⁻³ at 2%.
 * Past the share, the pair is `estimated`.
 */
export const AIR_TRACE_SHARE = 0.02;

/**
 * The pair profiles the listed layers form: each formable pair whose members are both listed with
 * a column and whose layers overlap, merged by overlap; an {@link AIR_PARTNER} pair `estimated`
 * where its partners other than its measured gases pass {@link AIR_TRACE_SHARE} of its partner
 * molecules.
 */
function pairProfiles(
  listed: ReadonlyMap<string, ListedLayer>,
  column: AtmosphereColumn,
  pairTables: ReadonlyArray<PairCrossSectionTable>,
): BuiltProfile[] {
  const densityAtUnit = (member: ListedLayer): number =>
    member.layer.columnPerM2 /
    layerIntegralM(layerOnColumn(column, member.basePa, member.topPa), 1);
  const overlaps = new Map<
    string,
    {
      readonly basePa: number;
      readonly topPa: number;
      order: number;
      readonly pairs: { pair: string; pairColumnPerM5: number }[];
    }
  >();
  const partnersEstimated = new Set<string>();
  for (const { pair, members } of formablePairs(pairTables)) {
    const a = listed.get(members[0]);
    const air = members[1] === AIR_PARTNER;
    const table = pairTables.find((t) => t.pair === pair);
    // `air`: every other listed gas, each by its weight, over the union of their overlaps with a. A
    // member with a profile forms no pair, as a nor as a partner (R08.T6.h): its density is no
    // constant multiple of the column's, so the pair's would be no constant multiple of n² over the
    // overlap.
    const partners = (
      air
        ? [...listed.values()].filter((l) => l.layer.species !== members[0])
        : [listed.get(members[1])].filter((l) => l !== undefined)
    ).filter((l) => l.layer.profile === undefined);
    if (a === undefined || a.layer.profile !== undefined || partners.length === 0) {
      continue;
    }
    const basePa = Math.min(a.basePa, Math.max(...partners.map((b) => b.basePa)));
    const topPa = Math.max(a.topPa, Math.min(...partners.map((b) => b.topPa)));
    if (!(basePa > topPa)) {
      continue;
    }
    let pairColumnPerM5 = 0;
    // `air`'s partners' pair column, unweighted, and that of the gases its σ is not measured with.
    let partnersPerM5 = 0;
    let tracePerM5 = 0;
    for (const b of partners) {
      const from = Math.min(a.basePa, b.basePa);
      const to = Math.max(a.topPa, b.topPa);
      if (!(from > to)) {
        continue;
      }
      const weight = air ? (table?.air?.weights.get(b.layer.species) ?? 1) : 1;
      const bareColumnPerM5 =
        densityAtUnit(a) * densityAtUnit(b) * layerIntegralM(layerOnColumn(column, from, to), 2);
      pairColumnPerM5 += weight * bareColumnPerM5;
      partnersPerM5 += bareColumnPerM5;
      if (table?.air?.measured.has(b.layer.species) !== true) {
        tracePerM5 += bareColumnPerM5;
      }
    }
    if (air && tracePerM5 > AIR_TRACE_SHARE * partnersPerM5) {
      partnersEstimated.add(pair);
    }
    if (!(pairColumnPerM5 > 0)) {
      continue;
    }
    const key = `${basePa}:${topPa}`;
    const order = Math.max(a.index, ...partners.map((b) => b.index)) + 0.5;
    const entry = overlaps.get(key) ?? { basePa, topPa, order, pairs: [] };
    entry.order = Math.min(entry.order, order);
    entry.pairs.push({ pair, pairColumnPerM5 });
    overlaps.set(key, entry);
  }
  return [...overlaps.values()].map((entry) => {
    const temperatureK = layerTemperatureK(layerOnColumn(column, entry.basePa, entry.topPa), 2);
    const pairs = entry.pairs.flatMap(({ pair, pairColumnPerM5 }): AbsorberPair[] => {
      const found = pairCrossSectionAt(pair, temperatureK, pairTables);
      const crossSection: PairCrossSection | undefined =
        found !== undefined && partnersEstimated.has(pair)
          ? { ...found, provenance: "estimated", estimatedBy: "optics" }
          : found;
      return crossSection === undefined ? [] : [{ pair, pairColumnPerM5, crossSection }];
    });
    const spec: PairAbsorberSpec = {
      kind: "pair",
      name: `pair:${pairs.map((p) => p.pair).join(",")}`,
      pairs,
      pairColumnPerM5: pairs.reduce((sum, p) => sum + p.pairColumnPerM5, 0),
      basePa: entry.basePa,
      topPa: entry.topPa,
      temperatureK,
    };
    return {
      order: entry.order,
      members: pairs.map((p) => ({ key: p.pair, certainty: p.crossSection })),
      spec,
    };
  });
}

/**
 * The specs that take the {@link ABSORBER_COLUMNS} columns: all of them while they fit, otherwise
 * the five of greatest {@link absorberRankingDepth}, equal depths keeping the earlier listed.
 */
function keptProfiles(specs: ReadonlyArray<AbsorberSpec>): ReadonlySet<AbsorberSpec> {
  if (specs.length <= ABSORBER_COLUMNS) {
    return new Set(specs);
  }
  const ranked = specs
    .map((spec, index) => ({ spec, index, depth: absorberRankingDepth(spec) }))
    .toSorted((a, b) => b.depth - a.depth || a.index - b.index);
  return new Set(ranked.slice(0, ABSORBER_COLUMNS).map(({ spec }) => spec));
}

/** An equal-energy spectrum in the bake bins, which the ranking weighs by. */
const EQUAL_ENERGY: BakeSpectrum = bakeSpectrumOf(Array.from({ length: BAKE_BIN_COUNT }, () => 1));

/**
 * The optical depth by which a profile is ranked for the stored columns
 * (`decision-r08-cmf-licence.md` §2.1): the largest over the channels of
 * −ln[Σ w_c e^(−Σᵢ Uᵢσᵢ) ÷ Σ w_c] through its vertical column (its vertical pair column for a pair
 * spec), w_c = max(c̄_c, 0) on σ's grid over the bake range under an equal-energy spectrum, with
 * the species that have cross-sections at the profile's temperature.
 *
 * @remarks
 * The same for every sun, since the shared per-planet table is built once per body. The weights
 * are clipped so that each term is a true weighted mean, positive and increasing in U, which is all
 * a ranking needs; the drawn curves keep the signed weights. Each sum is taken relative to its least
 * weighed σ, so the depth stays finite however deep.
 */
export function absorberRankingDepth(spec: AbsorberSpec): number {
  const optics = absorberOptics(spec, EQUAL_ENERGY);
  const sigma = mixedCrossSection(optics);
  const u = amountValue(absorberVerticalAmount(spec));
  return Math.max(
    ...optics.weights.map((channel) => {
      let least = Infinity;
      for (const [i, w] of channel.entries()) {
        least = w > 0 ? Math.min(least, sigma[i] ?? Number.NaN) : least;
      }
      let passed = 0;
      let total = 0;
      for (const [i, w] of channel.entries()) {
        if (w > 0) {
          passed += w * Math.exp(-((sigma[i] ?? Number.NaN) - least) * u);
          total += w;
        }
      }
      return least * u - Math.log(passed / total);
    }),
  );
}

/**
 * A layer's density on its column, ρᵖ of the column's relative density (p = 1 for a species, 2
 * for a pair) from its base to its top, with a level at each edge, and 0 at the column's next level
 * outside each edge, which the `tabulated` rule ramps to over that one interval; a species' times
 * its profile's mixing ratio where it has one (R08.T6.h).
 */
function layerDensity(
  column: AtmosphereColumn,
  basePa: number,
  topPa: number,
  power: 1 | 2,
  profile?: ReadonlyArray<AbsorberProfileNode>,
): TabulatedDensity {
  const clamped = clampedLayer(column, basePa, topPa);
  const layer = layerOnColumn(column, clamped.basePa, clamped.topPa, profile);
  const altitudes = [...layer.altitudesM];
  const relative = layer.relative.map((rho) => rho ** power);
  if (layer.belowM !== undefined) {
    altitudes.unshift(layer.belowM);
    relative.unshift(0);
  }
  if (layer.aboveM !== undefined) {
    altitudes.push(layer.aboveM);
    relative.push(0);
  }
  return tabulatedDensity(Float64Array.from(altitudes), Float64Array.from(relative));
}

/**
 * An absorber's term on its column (Design note 2): its layer's share of the column's density,
 * absorbing by its curves of growth alone.
 *
 * @remarks
 * A column spec's density is the column's relative density from the layer's base to its top, with a
 * level at each edge, and 0 at the column's next level outside each edge, which the `tabulated`
 * rule ramps to over that one interval; with a profile (R08.T6.h), x(p) times it, x the profile's
 * mixing ratio at each level's pressure ({@link profileMixingRatio}), so that a profile that is
 * above 0 at the datum and the column's top has no ramp and no layer edge. Its number density at
 * relative density 1 is U ÷ ∫ ρ dz, so that the vertical column is the spec's whatever the ramps
 * and the profile hold. A pair spec's (R08.T4.c) is the
 * square of the column's relative density over its overlap, ramped alike, and its pair density at
 * relative density 1, n_A n_B there, is U ÷ ∫ ρ² dz, so that u = n ∫ ρ² ds along any path is its
 * pair column. Scattering and `absorption` are 0 and the phase `none`: the term's absorption is
 * each sun's −ln T_c(u), which the tables add on read from the amount they store in its slot
 * (R08.T6.c; `tables.ts`'s `absorberSlots`).
 *
 * @throws RangeError for a spec whose layer lies outside the column, or whose vertical amount is
 *   not above 0.
 */
export function absorberTerm(absorber: AbsorberSpec, column: AtmosphereColumn): MediumTerm {
  const amount = amountValue(absorberVerticalAmount(absorber));
  if (!(Number.isFinite(amount) && amount > 0)) {
    throw new RangeError(`absorber ${absorber.name}'s vertical amount is above 0, not ${amount}`);
  }
  const common = {
    name: absorber.name,
    scattering: [0, 0, 0],
    absorption: [0, 0, 0],
    phase: { kind: "none" },
  } as const;
  let term: MediumTerm;
  switch (absorber.kind) {
    case "column": {
      const density = layerDensity(column, absorber.basePa, absorber.topPa, 1, absorber.profile);
      const lengthM = columnLengthM(density, column.topHeightM);
      term = {
        ...common,
        density,
        absorber: { kind: "column", spec: absorber, numberDensityPerM3: amount / lengthM },
      };
      break;
    }
    case "pair": {
      const density = layerDensity(column, absorber.basePa, absorber.topPa, 2);
      const lengthM = columnLengthM(density, column.topHeightM);
      term = {
        ...common,
        density,
        absorber: { kind: "pair", spec: absorber, pairDensityPerM6: amount / lengthM },
      };
      break;
    }
  }
  return term;
}

/**
 * The Thomson cross-section, m²: 6.652 458 7321 × 10⁻²⁹ (CODATA 2018), the same at every
 * wavelength.
 */
export const THOMSON_CROSS_SECTION_M2 = 6.652_458_7321e-29;

/**
 * Thomson scattering by free electrons as a medium term (R08.T4.c): a scattering term, not an
 * absorber, on e⁻'s layer, with {@link THOMSON_CROSS_SECTION_M2} in every channel, no absorption,
 * and Rayleigh's phase at ρ = 0, the dipole phase (3 ÷ 16π)(1 + cos²θ) that Thomson's
 * cross-section has.
 *
 * @remarks
 * The density is the layer's, its profile's too where it has one, as {@link absorberTerm} lays a
 * column spec's, and the electrons'
 * number density at relative density 1 is U ÷ ∫ ρ dz. The term is named `thomson`. R08.T10.a builds
 * it from the record's e⁻ (`decision-r08-t4b-t12c.md`, follow-up 2), whose `rayleigh` flag stays
 * false: this term is its scattering.
 *
 * @throws RangeError for a layer that is not e⁻'s, whose column is not finite and above 0, that
 *   lies outside the column, or whose profile is not one {@link absorberSpecs} accepts.
 */
export function thomsonTerm(electrons: AbsorberLayer, column: AtmosphereColumn): MediumTerm {
  return thomsonTermOf(electrons, column).term;
}

/** A Thomson term and the electrons' number density it is formed from. */
export interface ThomsonTerm {
  /** {@link thomsonTerm}'s term. */
  readonly term: MediumTerm;
  /**
   * n_e at relative density 1, electrons m⁻³: U ÷ ∫ρ dz, whose product with
   * {@link THOMSON_CROSS_SECTION_M2} is the term's coefficient in every channel.
   */
  readonly electronDensityPerM3: number;
}

/**
 * {@link thomsonTerm}'s term with the electrons' number density it is formed from, for the thick
 * bake's spectral column (`thick/spectralColumn.ts`'s `thomson` source).
 *
 * @throws RangeError as {@link thomsonTerm}.
 */
export function thomsonTermOf(electrons: AbsorberLayer, column: AtmosphereColumn): ThomsonTerm {
  if (electrons.species !== "e-") {
    throw new RangeError(`Thomson scattering is by e-, not ${electrons.species}`);
  }
  if (!(Number.isFinite(electrons.columnPerM2) && electrons.columnPerM2 > 0)) {
    throw new RangeError(
      `the electrons' column is finite and above 0, not ${electrons.columnPerM2} m⁻²`,
    );
  }
  checkProfile(electrons);
  const density = layerDensity(column, electrons.basePa, electrons.topPa, 1, electrons.profile);
  const lengthM = columnLengthM(density, column.topHeightM);
  if (!(lengthM > 0)) {
    throw new RangeError(`e-'s profile has a mixing ratio above 0 within its column`);
  }
  const numberDensityPerM3 = electrons.columnPerM2 / lengthM;
  const scattering = numberDensityPerM3 * THOMSON_CROSS_SECTION_M2;
  return {
    term: {
      name: "thomson",
      density,
      scattering: [scattering, scattering, scattering],
      absorption: [0, 0, 0],
      phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
    },
    electronDensityPerM3: numberDensityPerM3,
  };
}

/**
 * The least transmittance a channel's curve holds: 10⁻⁶, an optical depth of 13.8
 * (`decision-r08-t4b-t12c.md` §1). Below it the light is black on any display, and the signed sum
 * may leave Rec. 709 (methane's red goes negative) or underflow.
 */
export const ABSORBER_TRANSMITTANCE_FLOOR = 1e-6;

/** What a column and a pair absorber's optics share under one sun on σ's grid. */
export interface AbsorberOpticsBase {
  /** The grid, nm: the bake range at the finest step of the absorber's species. */
  readonly wavelengthsNm: Float64Array;
  /** S Δλ at each point, the trapezoid rule's weights of the sun's light, in R06's units. */
  readonly light: Float64Array;
  /**
   * c̄ S Δλ per channel at each point, signed: the sun's colour through a column u is
   * Σ weights e^(−σu). {@link channelTransmittance} applies the curves' rule to them.
   */
  readonly weights: readonly [Float64Array, Float64Array, Float64Array];
  /** The bake bin each point falls in, 0 to {@link BAKE_BIN_COUNT} − 1 (760 nm in the last). */
  readonly bins: Uint8Array;
}

/**
 * One line species of a column absorber's optics (R08.T4.d): its share of the mixture's column,
 * its lines, and the place of each of its cells among the optics' cells.
 */
export interface LineSpeciesOptics {
  /** Uₛ ÷ U: its column over the mixture's, so that its column on a path is xₛ u. */
  readonly fraction: number;
  readonly lines: SpeciesLines;
  /** Each of its k-distribution's cells' index into {@link LineOptics.cellsNm}. */
  readonly cellIndex: Int32Array;
}

/**
 * A column absorber's lines on σ's grid (R08.T4.d): each grid point's 1 nm cell, whose
 * transmittance multiplies the point's, the species' lines overlapping at random within a cell
 * (each species' k-distribution independent of the others', a stated approximation; Lacis and
 * Oinas 1991, §4).
 */
export interface LineOptics {
  /** The cells any of its species has lines in, nm, ascending. */
  readonly cellsNm: Int32Array;
  /** Each grid point's index into {@link cellsNm}, or −1 where no species has lines. */
  readonly pointCells: Int32Array;
  readonly species: ReadonlyArray<LineSpeciesOptics>;
}

/** A column absorber under one sun on σ's grid: what its curves of growth are summed from. */
export interface ColumnAbsorberOptics extends AbsorberOpticsBase {
  readonly kind: "column";
  /** σ_mix = Σ xᵢσᵢ at each point of the species with cross-sections, m² per molecule of the mixture. */
  readonly crossSectionM2: Float64Array;
  /** Its species' lines (R08.T4.d), or `undefined` for a mixture of cross-sections alone. */
  readonly lines: LineOptics | undefined;
}

/** A pair absorber under one sun on σ's grid (R08.T4.c). */
export interface PairAbsorberOptics extends AbsorberOpticsBase {
  readonly kind: "pair";
  /** σ_mix = Σ (Uᵢ ÷ U) σᵢ at each point, m⁵ per molecule pair of the absorber's mixture. */
  readonly crossSectionM5: Float64Array;
}

/** One absorber under one sun on σ's grid: a column absorber's or a pair absorber's. */
export type AbsorberOptics = ColumnAbsorberOptics | PairAbsorberOptics;

/**
 * An optics' mixed cross-section, in its kind's unit (m² a molecule, m⁵ a molecule pair), which an
 * amount of the same kind turns into an optical depth.
 */
function mixedCrossSection(optics: AbsorberOptics): Float64Array {
  let sigma: Float64Array;
  switch (optics.kind) {
    case "column":
      sigma = optics.crossSectionM2;
      break;
    case "pair":
      sigma = optics.crossSectionM5;
      break;
  }
  return sigma;
}

/** An optics' lines, if any: a pair absorber has none. */
function opticsLines(optics: AbsorberOptics): LineOptics | undefined {
  let lines: LineOptics | undefined;
  switch (optics.kind) {
    case "column":
      lines = optics.lines;
      break;
    case "pair":
      lines = undefined;
      break;
  }
  return lines;
}

/** The grid step when no species of an absorber holds cross-sections, nm. */
const DEFAULT_STEP_NM = 1;

/** One part of an absorber's mixture: its fraction of the vertical amount and its σ on its grid. */
interface MixturePart {
  /** For messages: the species or the pair. */
  readonly label: string;
  /** Uᵢ ÷ U. */
  readonly fraction: number;
  readonly firstNm: number;
  readonly stepNm: number;
  /** σ at each grid point, in the spec's unit. */
  readonly values: Float64Array;
}

/** The parts of a spec's mixture that hold cross-sections. */
function mixtureParts(absorber: AbsorberSpec): ReadonlyArray<MixturePart> {
  let parts: ReadonlyArray<MixturePart>;
  switch (absorber.kind) {
    case "column":
      parts = absorber.species.flatMap((s): MixturePart[] =>
        s.crossSection === undefined
          ? []
          : [
              {
                label: s.species,
                fraction: s.columnPerM2 / absorber.columnPerM2,
                firstNm: s.crossSection.firstNm,
                stepNm: s.crossSection.stepNm,
                values: s.crossSection.crossSectionsM2,
              },
            ],
      );
      break;
    case "pair":
      parts = absorber.pairs.map((p) => ({
        label: p.pair,
        fraction: p.pairColumnPerM5 / absorber.pairColumnPerM5,
        firstNm: p.crossSection.firstNm,
        stepNm: p.crossSection.stepNm,
        values: p.crossSection.crossSectionsM5,
      }));
      break;
  }
  return parts;
}

/** σ at a wavelength on a part's grid, linear between its points. */
function sigmaAt(part: MixturePart, wavelengthNm: number): number {
  const x = (wavelengthNm - part.firstNm) / part.stepNm;
  const k = Math.floor(x + GRID_TOLERANCE_STEPS);
  const t = Math.max(x - k, 0);
  const a = part.values[k] ?? Number.NaN;
  return t <= GRID_TOLERANCE_STEPS ? a : a + ((part.values[k + 1] ?? Number.NaN) - a) * t;
}

/**
 * Refuses a part's cross-sections that do not cover {@link BAKE_RANGE_NM} or hold a σ that is not
 * finite and at least 0, which would otherwise turn into a curve black at the floor.
 */
function checkPart(part: MixturePart): void {
  const [low, high] = BAKE_RANGE_NM;
  const last = part.firstNm + (part.values.length - 1) * part.stepNm;
  if (!(part.stepNm > 0 && part.firstNm <= low && last >= high)) {
    throw new RangeError(
      `${part.label}'s cross-sections span ${part.firstNm}–${last} nm, not the bake range ${low}–${high} nm`,
    );
  }
  if (!part.values.every((sigma) => Number.isFinite(sigma) && sigma >= 0)) {
    throw new RangeError(
      `${part.label}'s cross-sections hold a σ that is not finite and at least 0`,
    );
  }
}

/** The 1 nm cell [λ − 0.5, λ + 0.5) a grid point falls in, nm. */
function cellOf(wavelengthNm: number): number {
  return Math.floor(wavelengthNm + 0.5);
}

/** A column spec's lines on a grid's points, or `undefined` if none of its species has lines. */
function lineOptics(absorber: AbsorberSpec, wavelengthsNm: Float64Array): LineOptics | undefined {
  if (absorber.kind !== "column") {
    return undefined;
  }
  const withLines = absorber.species.flatMap((s) =>
    s.lines === undefined
      ? []
      : [{ fraction: s.columnPerM2 / absorber.columnPerM2, lines: s.lines }],
  );
  if (withLines.length === 0) {
    return undefined;
  }
  const cells = [
    ...new Set(withLines.flatMap(({ lines }) => lines.kDistribution.cells.map((c) => c.cellNm))),
  ].toSorted((a, b) => a - b);
  const indexOf = new Map(cells.map((nm, i) => [nm, i]));
  return {
    cellsNm: Int32Array.from(cells),
    pointCells: Int32Array.from(wavelengthsNm, (nm) => indexOf.get(cellOf(nm)) ?? -1),
    species: withLines.map(({ fraction, lines }) => ({
      fraction,
      lines,
      cellIndex: Int32Array.from(lines.kDistribution.cells, (c) => indexOf.get(c.cellNm) ?? -1),
    })),
  };
}

/**
 * An absorber's mixed cross-section and a sun's light and channel weights on σ's grid (Design
 * note 5): the bake range at the finest step among its species, so that a single species is summed
 * on its own grid, point for point, and 1 nm where none holds cross-sections. A pair spec's
 * (R08.T4.c) mixes its pairs' σ in m⁵. A column spec's line species (R08.T4.d) give each point its
 * cell's lines.
 *
 * @throws RangeError for a species or pair whose cross-sections do not cover the bake range or are
 *   not finite and at least 0.
 */
export function absorberOptics(absorber: AbsorberSpec, spectrum: BakeSpectrum): AbsorberOptics {
  const parts = mixtureParts(absorber);
  for (const part of parts) {
    checkPart(part);
  }
  const stepNm = Math.min(...parts.map((p) => p.stepNm), DEFAULT_STEP_NM);
  const [low, high] = BAKE_RANGE_NM;
  const count = Math.round((high - low) / stepNm) + 1;
  const wavelengthsNm = Float64Array.from({ length: count }, (_, i) => low + i * stepNm);
  const mixed = new Float64Array(count);
  const light = new Float64Array(count);
  const bins = new Uint8Array(count);
  const weights: [Float64Array, Float64Array, Float64Array] = [
    new Float64Array(count),
    new Float64Array(count),
    new Float64Array(count),
  ];
  for (const [i, nm] of wavelengthsNm.entries()) {
    let sigma = 0;
    for (const part of parts) {
      sigma += part.fraction * sigmaAt(part, nm);
    }
    mixed[i] = sigma;
    const s = bakeSpectrumAt(spectrum, nm) * stepNm * (i === 0 || i === count - 1 ? 0.5 : 1);
    light[i] = s;
    bins[i] = Math.min(Math.floor((nm - low) / BAKE_BIN_WIDTH_NM), BAKE_BIN_COUNT - 1);
    const matching = matchingFunctionsAt(nm);
    for (const [c, channel] of weights.entries()) {
      channel[i] = (matching[c] ?? Number.NaN) * s;
    }
  }
  const base = { wavelengthsNm, light, weights, bins };
  let optics: AbsorberOptics;
  switch (absorber.kind) {
    case "column":
      optics = {
        kind: "column",
        crossSectionM2: mixed,
        lines: lineOptics(absorber, wavelengthsNm),
        ...base,
      };
      break;
    case "pair":
      optics = { kind: "pair", crossSectionM5: mixed, ...base };
      break;
  }
  return optics;
}

/** An absorber's optics with the curves' rule applied, ready to be summed at any column. */
interface PreparedOptics {
  /** σ_mix at each point, in the spec's unit ({@link mixedCrossSection}). */
  readonly sigma: Float64Array;
  /** The least σ on the grid, in the spec's unit: every sum is taken relative to it. */
  readonly leastSigma: number;
  /** Each channel's weights by the rule, and their sums. */
  readonly channelWeights: readonly [Float64Array, Float64Array, Float64Array];
  readonly channelTotals: readonly [number, number, number];
  readonly light: Float64Array;
  readonly bins: Uint8Array;
  /** Each bin's light, Σ S Δλ over its points. */
  readonly binTotals: Float64Array;
  /** The lines, if any (R08.T4.d). */
  readonly lines: LineOptics | undefined;
}

/**
 * The curves' weighting rule (`decision-r08-t4b-t12c.md` §1): each channel's signed weights c̄ S Δλ
 * while the sun's colour in it, Σ c̄ S Δλ, is positive, and max(c̄, 0) S Δλ where it is not (a star
 * outside Rec. 709, below about 1,900 K for a Planck spectrum).
 *
 * @throws RangeError for a sun with no light in a channel's weights.
 */
function prepare(optics: AbsorberOptics): PreparedOptics {
  const ruled = optics.weights.map((weights, c) => {
    const signed = weights.reduce((sum, w) => sum + w, 0);
    if (signed > 0) {
      return { weights, total: signed };
    }
    const clipped = weights.map((w) => Math.max(w, 0));
    const total = clipped.reduce((sum, w) => sum + w, 0);
    if (!(total > 0)) {
      throw new RangeError(`the sun has no light in channel ${String(c)}'s weights`);
    }
    return { weights: clipped, total };
  });
  const [red, green, blue] = ruled;
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("an absorber's optics have three channels");
  }
  const sigma = mixedCrossSection(optics);
  const binTotals = new Float64Array(BAKE_BIN_COUNT);
  for (const [i, s] of optics.light.entries()) {
    const b = optics.bins[i] ?? 0;
    binTotals[b] = (binTotals[b] ?? 0) + s;
  }
  return {
    sigma,
    leastSigma: sigma.reduce((least, value) => Math.min(least, value), Infinity),
    channelWeights: [red.weights, green.weights, blue.weights],
    channelTotals: [red.total, green.total, blue.total],
    light: optics.light,
    bins: optics.bins,
    binTotals,
    lines: opticsLines(optics),
  };
}

/**
 * The lines' g-point cross-sections at a pressure, per species and cell: each species'
 * k-distribution read at the pressure times its broadening factor ({@link cellSigmasAt}), times its
 * column fraction, so that a cell's species transmittance through the mixture's column u is
 * Σ_g w_g e^(−σ_g u).
 */
function lineSigmasAt(
  lines: LineOptics,
  pressurePa: number,
): ReadonlyArray<ReadonlyArray<Float64Array>> {
  return lines.species.map((s) =>
    cellSigmasAt(s.lines.kDistribution, pressurePa * s.lines.broadeningFactor).map((sigmas) =>
      sigmas.map((v) => v * s.fraction),
    ),
  );
}

/**
 * A part of an absorber's curves of growth, for one of the optics workers (R08.T4.d;
 * `decision-r08-t4d-kdist-chain.md` §15, ruling 3): the line cells whose index among the optics'
 * cells is `index` modulo `count`, and, in share 0, the grid's points without lines.
 */
export interface CurveShare {
  /** The share's place, 0 to `count` − 1. */
  readonly index: number;
  /** How many shares the curves are split into, at least 1. */
  readonly count: number;
}

/** The whole of the curves, one share. */
const WHOLE: CurveShare = { index: 0, count: 1 };

/**
 * A line cell's largest σ times the curve's top column, 10³ U, at or under which a row of the
 * curves takes the cell's mean σ̄, one exponential a column, e^(−σ̄u) for Σ_g w_g e^(−σ_g u)
 * (`decision-r08-t4d-kdist-chain.md` §15, ruling 3): 0.02. The two differ by between
 * ½ e^(−σ̄u) Var(σ) u² and ½ Var(σ) u², at most (σ_max u)² ÷ 8 by Popoviciu's inequality: under
 * 4.95 × 10⁻⁵ at 0.02, per species in a cell, inside the ruling's 2 × 10⁻⁴ for a cell of up to
 * four species; the bound of a reduced cell stored as its mean alone (G = 1).
 */
export const LINEAR_CELL_DEPTH = 0.02;

/**
 * One pressure row of the lines: each species' cells' g-point σ ({@link lineSigmasAt}), and each
 * cell's mean σ̄ where the row takes it ({@link LINEAR_CELL_DEPTH}), NaN where it does not.
 */
interface LineRow {
  readonly sigmas: ReadonlyArray<ReadonlyArray<Float64Array>>;
  readonly means: ReadonlyArray<Float64Array>;
}

/** The lines' row at a pressure for curves whose largest column of the mixture is `topColumnPerM2`. */
function lineRowAt(lines: LineOptics, pressurePa: number, topColumnPerM2: number): LineRow {
  const sigmas = lineSigmasAt(lines, pressurePa);
  const means = lines.species.map((s, k) =>
    Float64Array.from(s.lines.kDistribution.cells, (cell, c) => {
      const values = sigmas[k]?.[c] ?? new Float64Array(0);
      let largest = 0;
      let mean = 0;
      for (const [g, w] of cell.weights.entries()) {
        const v = values[g] ?? 0;
        largest = Math.max(largest, v);
        mean += w * v;
      }
      return largest * topColumnPerM2 <= LINEAR_CELL_DEPTH ? mean : Number.NaN;
    }),
  );
  return { sigmas, means };
}

/**
 * Each of the lines' cells' transmittance through the mixture's column u: the product over the
 * species of Σ_g w_g e^(−σ_g u), random overlap within the cell, or e^(−σ̄u) where `means` gives
 * a cell's mean ({@link lineRowAt}); for the cells of `share` alone, the others left at 1.
 */
function lineCellTransmittances(
  lines: LineOptics,
  sigmas: ReadonlyArray<ReadonlyArray<Float64Array>>,
  u: number,
  means: ReadonlyArray<Float64Array> | undefined,
  share: CurveShare,
): Float64Array {
  const cells = new Float64Array(lines.cellsNm.length).fill(1);
  for (const [k, s] of lines.species.entries()) {
    const perCell = sigmas[k] ?? [];
    const perMean = means?.[k];
    for (const [c, cell] of s.lines.kDistribution.cells.entries()) {
      const at = s.cellIndex[c] ?? -1;
      if (at % share.count !== share.index) {
        continue;
      }
      const mean = perMean?.[c] ?? Number.NaN;
      let t = 0;
      if (Number.isNaN(mean)) {
        const values = perCell[c] ?? new Float64Array(0);
        for (const [g, w] of cell.weights.entries()) {
          t += w * Math.exp(-(values[g] ?? 0) * u);
        }
      } else {
        t = Math.exp(-mean * u);
      }
      cells[at] = (cells[at] ?? 1) * t;
    }
  }
  return cells;
}

/**
 * A transmittance clamped to [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1].
 *
 * @throws Error for NaN, which checked optics never give: a broken invariant, not a black curve.
 */
function clampedTransmittance(value: number): number {
  if (Number.isNaN(value)) {
    throw new Error("an absorber's transmittance is NaN: its optics were not checked");
  }
  return Math.min(Math.max(value, ABSORBER_TRANSMITTANCE_FLOOR), 1);
}

/**
 * The channels' clamped transmittances through an amount u, from one exponential a point:
 * e^(−(σ − σ_min)u), so that no sum underflows before its own least term, times each point's
 * cell's lines (R08.T4.d), `lineCells`, where it has any. u is in the unit of the optics' kind
 * (molecules m⁻² or molecule pairs m⁻⁵), checked by its callers ({@link amountFor}). The curves'
 * own sums, with their bins, are {@link sumsOf}'s.
 */
function sumsAt(p: PreparedOptics, amountInKind: number, lineCells: Float64Array | undefined): Rgb {
  const u = amountInKind;
  const [red, green, blue] = p.channelWeights;
  const pointCells = p.lines?.pointCells;
  let passedRed = 0;
  let passedGreen = 0;
  let passedBlue = 0;
  for (let i = 0; i < p.sigma.length; i += 1) {
    const cell = pointCells?.[i] ?? -1;
    const line = cell >= 0 ? (lineCells?.[cell] ?? 1) : 1;
    const e = Math.exp(-((p.sigma[i] ?? Number.NaN) - p.leastSigma) * u) * line;
    passedRed += (red[i] ?? Number.NaN) * e;
    passedGreen += (green[i] ?? Number.NaN) * e;
    passedBlue += (blue[i] ?? Number.NaN) * e;
  }
  const scale = Math.exp(-p.leastSigma * u);
  const [totalRed, totalGreen, totalBlue] = p.channelTotals;
  return [
    clampedTransmittance((scale * passedRed) / totalRed),
    clampedTransmittance((scale * passedGreen) / totalGreen),
    clampedTransmittance((scale * passedBlue) / totalBlue),
  ];
}

/**
 * ln T_cell of the lines' cell `at` through the mixture's column u at a row, its species' sums
 * Σ_g w_g e^(−σ_g u) taken as logarithms relative to their largest terms, so that a cell whose sum
 * underflows to 0 still has its logarithm; −σ̄u where the row takes a cell's mean.
 */
function lineCellLog(lines: LineOptics, row: LineRow, at: number, u: number): number {
  let log = 0;
  for (const [k, s] of lines.species.entries()) {
    const c = s.cellIndex.indexOf(at);
    const cell = s.lines.kDistribution.cells[c];
    if (cell === undefined) {
      continue;
    }
    const mean = row.means[k]?.[c] ?? Number.NaN;
    if (!Number.isNaN(mean)) {
      log -= mean * u;
      continue;
    }
    const values = row.sigmas[k]?.[c] ?? new Float64Array(0);
    const exponents = cell.weights.map((w, g) => Math.log(w) - (values[g] ?? 0) * u);
    const top = Math.max(...exponents);
    let sum = 0;
    for (const x of exponents) {
      sum += Math.exp(x - top);
    }
    log += top + Math.log(sum);
  }
  return log;
}

/**
 * ln Σ S e^(−σu) T_cell over a bin's points in a share, summed relative to its largest term, with
 * the points' lines as logarithms ({@link lineCellLog} where a cell's sum underflowed), so that
 * they underflow no sooner: −∞ only for a share with no point in the bin.
 */
function binLogPassed(
  p: PreparedOptics,
  bin: number,
  u: number,
  lineCells: Float64Array | undefined,
  share: CurveShare,
  logOfCell: (cell: number) => number,
): number {
  const exponents: number[] = [];
  for (const [i, b] of p.bins.entries()) {
    const cell = p.lines?.pointCells[i] ?? -1;
    if (b === bin && inShare(cell, share)) {
      const line = cell >= 0 ? (lineCells?.[cell] ?? 1) : 1;
      const logLine = line > 0 ? Math.log(line) : logOfCell(cell);
      exponents.push(-(p.sigma[i] ?? Number.NaN) * u + logLine + Math.log(p.light[i] ?? 0));
    }
  }
  const top = Math.max(...exponents);
  if (!Number.isFinite(top)) {
    return -Infinity;
  }
  let passed = 0;
  for (const x of exponents) {
    passed += Math.exp(x - top);
  }
  return top + Math.log(passed);
}

/**
 * The transmittance per channel through a column of an absorber's mixture, by the curves' rule
 * (`decision-r08-t4b-t12c.md` §1): T_c = Σ c̄_c S e^(−σu) T_cell(u, p̄) Δλ ÷ Σ c̄_c S Δλ with the
 * signed matching functions (clipped at zero only in a channel where the sun's own colour is not
 * positive), clamped to [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1], T_cell the point's cell's lines
 * (R08.T4.d), 1 for an absorber without them.
 *
 * @remarks
 * The rule lives in this module's weighting alone, so that a ruling on the weights changes one
 * place. Unlike a weighted mean of e^(−σu) the signed sum can pass 1 (blue, whose weight is
 * negative over 500–620 nm, under cool stars) and fall below 0 (methane's red, once the light
 * leaves Rec. 709): those are where the clamp acts. A curve on a grid of columns also takes the
 * running minimum ({@link absorberCurve}); this single column does not.
 *
 * @param amount - u, of the optics' kind.
 * @param pressurePa - The path's absorber-weighted (Curtis–Godson) pressure, Pa, at which the lines
 *   are read: required for an absorber with lines, ignored without them.
 * @throws Error for an amount of the other kind.
 * @throws RangeError for an amount that is not finite and at least 0, a missing or non-positive
 *   pressure for an absorber with lines, or a sun with no light in a channel's weights.
 */
export function channelTransmittance(
  optics: AbsorberOptics,
  amount: AbsorberAmount,
  pressurePa: number | undefined,
): Rgb {
  const prepared = prepare(optics);
  const u = amountFor(amount, optics.kind);
  const lines = prepared.lines;
  if (lines === undefined) {
    return sumsAt(prepared, u, undefined);
  }
  if (pressurePa === undefined) {
    throw new RangeError("an absorber with lines is read at a path's pressure");
  }
  return sumsAt(
    prepared,
    u,
    lineCellTransmittances(lines, lineSigmasAt(lines, pressurePa), u, undefined, WHOLE),
  );
}

/** The decades of the curve's column grid about the absorber's vertical column U: 10⁻³ U to 10³ U. */
export const ABSORBER_CURVE_DECADES: readonly [number, number] = [-3, 3];

/** The curve's nodes per decade of column. */
export const ABSORBER_CURVE_NODES_PER_DECADE = 40;

/**
 * The line species whose broadening factor sets a column spec's pressure rows: its species with
 * lines of the largest column, or `undefined` for a spec without lines.
 */
function dominantLines(absorber: AbsorberSpec): SpeciesLines | undefined {
  if (absorber.kind !== "column") {
    return undefined;
  }
  let best: AbsorberSpecies | undefined;
  for (const s of absorber.species) {
    if (s.lines !== undefined && (best === undefined || s.columnPerM2 > best.columnPerM2)) {
      best = s;
    }
  }
  return best?.lines;
}

/**
 * A curve's pressure rows, ln(p ÷ 1 Pa), ascending (`decision-r08-t4d-kdist.md` §2.4): for an
 * absorber with lines, the stored broadening pressures of its dominant line species
 * ({@link dominantLines}) as pressures, p = p_eff ÷ f, from the first, 10 Pa ÷ f, through every one
 * up to the layer's base and one more beyond, so that the cubic read has its stencil there; for one
 * without lines, its base alone, since its curves do not depend on pressure.
 *
 * @remarks
 * Where the dominant table is thinned (R08.T4.e's economy E3: its nodes a strict subset of the
 * standard pressures), its rows are the standard pressures over its span (`curveRowsPa`), each
 * table read between its own nodes: a client change, no stored layout (ruled 2026-10-16). To them
 * are added the 8-a-decade midpoints any of the spec's line tables holds
 * (decision-r08-t4e-unsold's follow-up, the 581 nm residual), each as its physical pressure
 * p_mid ÷ f_s on its own species' factor, within the rows' span, one within
 * {@link MIDPOINT_MERGE_DECADES} of a row already there left out.
 */
export function absorberCurvePressures(absorber: AbsorberSpec): Float64Array {
  const lines = dominantLines(absorber);
  if (lines === undefined) {
    return Float64Array.of(Math.log(absorber.basePa));
  }
  const nodes = curveRowsPa(lines.kDistribution.pressuresPa);
  const f = lines.broadeningFactor;
  const deepest = nodes.findIndex((p) => p / f >= absorber.basePa);
  const last = deepest < 0 ? nodes.length - 1 : Math.min(deepest + 1, nodes.length - 1);
  const rows = nodes.slice(0, last + 1).map((p) => Math.log(p / f));
  const low = rows[0] ?? Number.NaN;
  const high = rows.at(-1) ?? Number.NaN;
  const merge = MIDPOINT_MERGE_DECADES * Math.LN10;
  for (const s of absorber.kind === "column" ? absorber.species : []) {
    for (const p of s.lines?.kDistribution.pressuresPa ?? []) {
      const row = Math.log(p / (s.lines?.broadeningFactor ?? 1));
      if (
        isLatticeMidpoint(p) &&
        row > low &&
        row < high &&
        rows.every((r) => Math.abs(r - row) >= merge)
      ) {
        rows.push(row);
      }
    }
  }
  return Float64Array.from(rows.toSorted((a, b) => a - b));
}

/** How near a midpoint row lies to a row already there and is left out, decades: 1/32. */
export const MIDPOINT_MERGE_DECADES = 1 / 32;

/**
 * The read's top for a curve's rows, ln(p ÷ 1 Pa): {@link LINE_TABLE_TOP_PA} ÷ f for an absorber
 * with lines, f its dominant line species' broadening factor; its base for one without.
 */
export function absorberReadTopLogPressure(absorber: AbsorberSpec): number {
  const lines = dominantLines(absorber);
  return Math.log(
    lines === undefined ? absorber.basePa : LINE_TABLE_TOP_PA / lines.broadeningFactor,
  );
}

/** One absorber's curves of growth under one sun, computed at arrival (Design note 5). */
export interface AbsorberCurve {
  /** The absorber's kind, which sets the unit of its columns and the amount it reads. */
  readonly kind: AbsorberSpec["kind"];
  /**
   * ln(u ÷ 1 m⁻²) at each node, ascending: {@link ABSORBER_CURVE_DECADES} about U, evenly; for a
   * pair absorber, ln(u ÷ 1 m⁻⁵) of its pair column.
   */
  readonly logColumns: Float64Array;
  /**
   * ln(p ÷ 1 Pa) at each pressure row, ascending ({@link absorberCurvePressures}; R08.T4.d): one
   * row for an absorber without lines.
   */
  readonly logPressures: Float64Array;
  /**
   * ln(p ÷ 1 Pa) above which the read holds p̄: the 10 MPa node's row, p = 10 MPa ÷ f
   * (`decision-r08-t4d-kdist-chain.md` §8), the 13.3 MPa row beyond it only centring the stencil.
   */
  readonly readTopLogPressure: number;
  /**
   * T_c(u, p̄) per channel at each row and node, row-major, `[r × nodes + j]`, in
   * [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1] and non-increasing along each row's nodes.
   */
  readonly transmittance: readonly [Float64Array, Float64Array, Float64Array];
  /** −ln T_c(u, p̄) per channel at each row and node, row-major, in [0, 13.8]. */
  readonly opticalDepth: readonly [Float64Array, Float64Array, Float64Array];
}

/** One absorber's curves of growth in each bake bin under one sun (Design note 5). */
export interface AbsorberBinCurves {
  /** The absorber's kind, as {@link AbsorberCurve.kind}. */
  readonly kind: AbsorberSpec["kind"];
  /** ln(u ÷ 1 m⁻²) at each node, the channel curve's (ln(u ÷ 1 m⁻⁵) for a pair absorber). */
  readonly logColumns: Float64Array;
  /** ln(p ÷ 1 Pa) at each pressure row, the channel curve's. */
  readonly logPressures: Float64Array;
  /** As {@link AbsorberCurve.readTopLogPressure}. */
  readonly readTopLogPressure: number;
  /** T_b(u, p̄) at each row and node, row-major, one array a bake bin: the bin's mean of e^(−σu)T_cell weighted by S. */
  readonly transmittance: ReadonlyArray<Float64Array>;
  /** −ln T_b(u, p̄) at each row and node, row-major, one array a bake bin, finite however deep. */
  readonly opticalDepth: ReadonlyArray<Float64Array>;
}

/**
 * The curve's columns about a spec's vertical amount U ({@link absorberVerticalAmount}):
 * molecules m⁻², or molecule pairs m⁻⁵ for a pair spec.
 */
function curveColumns(absorber: AbsorberSpec): Float64Array {
  const amount = amountValue(absorberVerticalAmount(absorber));
  if (!(Number.isFinite(amount) && amount > 0)) {
    throw new RangeError(`absorber ${absorber.name}'s vertical amount is above 0, not ${amount}`);
  }
  const [first, last] = ABSORBER_CURVE_DECADES;
  const nodes = (last - first) * ABSORBER_CURVE_NODES_PER_DECADE + 1;
  return Float64Array.from(
    { length: nodes },
    (_, k) => amount * 10 ** (first + k / ABSORBER_CURVE_NODES_PER_DECADE),
  );
}

/** One sun's tabulated curves, as {@link tabulate} gives them. */
interface Tabulated {
  readonly channels: AbsorberCurve;
  readonly bins: AbsorberBinCurves | undefined;
}

/**
 * One share's sums of an absorber's curves under each sun ({@link absorberCurveSums}), before
 * they are scaled, normalised and clamped: what the optics workers' shares add.
 */
export interface AbsorberCurveSums {
  readonly share: CurveShare;
  /**
   * Per sun, per channel, the share's passed light Σ c̄ S e^(−(σ − σ_min)u) T_cell over its grid
   * points, at each row's column nodes (row by row), σ_min the grid's least σ.
   */
  readonly channels: ReadonlyArray<readonly [Float64Array, Float64Array, Float64Array]>;
  /** Per sun, per bake bin, Σ S e^(−(σ − σ_min)u) T_cell over the share's points in the bin. */
  readonly bins: ReadonlyArray<ReadonlyArray<Float64Array>> | undefined;
  /**
   * Per sun, per bake bin, ln Σ S e^(−σu) T_cell over the same points where that sum underflows to
   * 0, summed relative to its largest term; NaN elsewhere.
   */
  readonly binLogs: ReadonlyArray<ReadonlyArray<Float64Array>> | undefined;
}

/** What an absorber's curves are tabulated on: its columns, rows and each sun's optics. */
interface CurveContext {
  readonly absorber: AbsorberSpec;
  readonly columns: Float64Array;
  readonly logPressures: Float64Array;
  readonly readTopLogPressure: number;
  readonly prepared: ReadonlyArray<PreparedOptics>;
  readonly withBins: boolean;
}

/**
 * An absorber's curves' grid and each sun's prepared optics.
 *
 * @throws RangeError as {@link channelTransmittance}, for a spec whose column is not above 0, or,
 *   for the bins, a sun with no light in a bake bin.
 */
function curveContext(
  absorber: AbsorberSpec,
  spectra: ReadonlyArray<BakeSpectrum>,
  withBins: boolean,
): CurveContext {
  const prepared = spectra.map((spectrum) => prepare(absorberOptics(absorber, spectrum)));
  if (withBins && !prepared.every((p) => p.binTotals.every((total) => total > 0))) {
    throw new RangeError("a sun has no light in a bake bin, whose curve would be undefined");
  }
  return {
    absorber,
    columns: curveColumns(absorber),
    logPressures: absorberCurvePressures(absorber),
    readTopLogPressure: absorberReadTopLogPressure(absorber),
    prepared,
    withBins,
  };
}

/**
 * The lines' cell transmittances at a row's pressure and a column, for a share, the row taking a
 * linear cell's mean ({@link lineRowAt}); undefined for an absorber without lines.
 */
function shareLineCells(
  lines: LineOptics | undefined,
  row: LineRow | undefined,
  u: number,
  share: CurveShare,
): Float64Array | undefined {
  return lines === undefined || row === undefined
    ? undefined
    : lineCellTransmittances(lines, row.sigmas, u, row.means, share);
}

/** Whether a grid point of line cell `cell` (−1 for none) falls in `share`. */
function inShare(cell: number, share: CurveShare): boolean {
  return cell >= 0 ? cell % share.count === share.index : share.index === 0;
}

/**
 * A share's sums under each sun ({@link AbsorberCurveSums}): one exponential a point, a column and
 * a pressure row, over the share's points alone, the lines' cell transmittances taken once for
 * every sun (R08.T4.d), since they are the absorber's alone.
 */
function sumsOf(context: CurveContext, share: CurveShare): AbsorberCurveSums {
  const { columns, logPressures, prepared, withBins } = context;
  const nodes = columns.length;
  const size = nodes * logPressures.length;
  const channels = prepared.map((): readonly [Float64Array, Float64Array, Float64Array] => [
    new Float64Array(size),
    new Float64Array(size),
    new Float64Array(size),
  ]);
  const bins = withBins
    ? prepared.map(() => Array.from({ length: BAKE_BIN_COUNT }, () => new Float64Array(size)))
    : undefined;
  const binLogs = withBins
    ? prepared.map(() =>
        Array.from({ length: BAKE_BIN_COUNT }, () => new Float64Array(size).fill(Number.NaN)),
      )
    : undefined;
  // The lines and their cells, and σ on the grid, are the absorber's (`lineOptics`,
  // `mixedCrossSection`), the same under every sun; only the weights are each sun's.
  const first = prepared[0];
  const lines = first?.lines;
  const sigma = first?.sigma ?? new Float64Array(0);
  const leastSigma = first?.leastSigma ?? Number.NaN;
  if (!prepared.every((p) => p.sigma.length === sigma.length && p.leastSigma === leastSigma)) {
    throw new Error("an absorber's σ is its own, the same grid under every sun");
  }
  const topColumnPerM2 = columns[nodes - 1] ?? Number.NaN;
  // The share's points, and whether it holds any point of a bin: one that holds none passes
  // nothing there, −∞.
  const points = Int32Array.from({ length: sigma.length }, (_, i) => i).filter((i) =>
    inShare(lines?.pointCells[i] ?? -1, share),
  );
  const pointsIn = new Uint8Array(BAKE_BIN_COUNT);
  for (const i of points) {
    pointsIn[first?.bins[i] ?? 0] = 1;
  }
  const passedAt = new Float64Array(points.length);
  const binPassed = new Float64Array(BAKE_BIN_COUNT);
  for (const [r, logPressure] of logPressures.entries()) {
    const row =
      lines === undefined ? undefined : lineRowAt(lines, Math.exp(logPressure), topColumnPerM2);
    for (const [k, u] of columns.entries()) {
      const lineCells = shareLineCells(lines, row, u, share);
      const at = r * nodes + k;
      for (const [j, i] of points.entries()) {
        const cell = lines?.pointCells[i] ?? -1;
        const line = cell >= 0 ? (lineCells?.[cell] ?? 1) : 1;
        passedAt[j] = Math.exp(-((sigma[i] ?? Number.NaN) - leastSigma) * u) * line;
      }
      for (const [s, p] of prepared.entries()) {
        const [red, green, blue] = p.channelWeights;
        let passedRed = 0;
        let passedGreen = 0;
        let passedBlue = 0;
        binPassed.fill(0);
        for (const [j, i] of points.entries()) {
          const e = passedAt[j] ?? Number.NaN;
          passedRed += (red[i] ?? Number.NaN) * e;
          passedGreen += (green[i] ?? Number.NaN) * e;
          passedBlue += (blue[i] ?? Number.NaN) * e;
          if (withBins) {
            const b = p.bins[i] ?? 0;
            binPassed[b] = (binPassed[b] ?? 0) + (p.light[i] ?? Number.NaN) * e;
          }
        }
        const out = channels[s];
        if (out !== undefined) {
          out[0][at] = passedRed;
          out[1][at] = passedGreen;
          out[2][at] = passedBlue;
        }
        for (const [b, bin] of (bins?.[s] ?? []).entries()) {
          const passed = binPassed[b] ?? Number.NaN;
          bin[at] = passed;
          const logs = binLogs?.[s]?.[b];
          if (!(passed > 0) && logs !== undefined) {
            logs[at] =
              pointsIn[b] === 1
                ? binLogPassed(p, b, u, lineCells, share, (cell) =>
                    lines === undefined || row === undefined
                      ? Number.NaN
                      : lineCellLog(lines, row, cell, u),
                  )
                : -Infinity;
          }
        }
      }
    }
  }
  return { share, channels, bins, binLogs };
}

/**
 * The curves from every share's sums ({@link sumsOf}): the shares' passed light added, then each
 * channel's clamped transmittance and the running minimum along each row, and each bin's optical
 * depth, as `sumsAt` takes them from one column.
 *
 * @throws RangeError for shares that are not each of `0` to `count` − 1 once, of one count, or that
 *   hold another number of suns or of nodes.
 */
function curvesFromSums(
  context: CurveContext,
  sums: ReadonlyArray<AbsorberCurveSums>,
): ReadonlyArray<Tabulated> {
  const { absorber, columns, logPressures, readTopLogPressure, prepared, withBins } = context;
  const nodes = columns.length;
  const size = nodes * logPressures.length;
  const count = sums[0]?.share.count ?? 0;
  const whole =
    count >= 1 &&
    sums.length === count &&
    sums.every(
      (part, i) =>
        part.share.count === count &&
        sums.findIndex((other) => other.share.index === part.share.index) === i &&
        part.share.index >= 0 &&
        part.share.index < count &&
        part.channels.length === prepared.length &&
        part.channels.every((perSun) => perSun.every((c) => c.length === size)) &&
        (part.bins !== undefined) === withBins &&
        (part.binLogs !== undefined) === withBins,
    );
  if (!whole) {
    throw new RangeError(
      `absorber ${absorber.name}'s curve sums are not every share of one split once, on its grid`,
    );
  }
  const logColumns = columns.map(Math.log);
  return prepared.map((p, s) => {
    const transmittance: [Float64Array, Float64Array, Float64Array] = [
      new Float64Array(size),
      new Float64Array(size),
      new Float64Array(size),
    ];
    const binDepths = withBins
      ? Array.from({ length: BAKE_BIN_COUNT }, () => new Float64Array(size))
      : undefined;
    for (const [r] of logPressures.entries()) {
      for (const [k, u] of columns.entries()) {
        const at = r * nodes + k;
        const scale = Math.exp(-p.leastSigma * u);
        for (const [c, channel] of transmittance.entries()) {
          let passed = 0;
          for (const part of sums) {
            passed += part.channels[s]?.[c]?.[at] ?? Number.NaN;
          }
          const t = clampedTransmittance((scale * passed) / (p.channelTotals[c] ?? Number.NaN));
          channel[at] = k === 0 ? t : Math.min(t, channel[at - 1] ?? Number.NaN);
        }
        for (const [b, bin] of (binDepths ?? []).entries()) {
          let passed = 0;
          for (const part of sums) {
            passed += part.bins?.[s]?.[b]?.[at] ?? Number.NaN;
          }
          const total = p.binTotals[b] ?? Number.NaN;
          bin[at] =
            passed > 0
              ? p.leastSigma * u - Math.log(passed / total)
              : -logSumOf(sums.map((part) => part.binLogs?.[s]?.[b]?.[at] ?? Number.NaN)) +
                Math.log(total);
        }
      }
    }
    return {
      channels: {
        kind: absorber.kind,
        logColumns,
        logPressures,
        readTopLogPressure,
        transmittance,
        opticalDepth: [
          transmittance[0].map((t) => -Math.log(t)),
          transmittance[1].map((t) => -Math.log(t)),
          transmittance[2].map((t) => -Math.log(t)),
        ],
      },
      bins:
        binDepths === undefined
          ? undefined
          : {
              kind: absorber.kind,
              logColumns,
              logPressures,
              readTopLogPressure,
              transmittance: binDepths.map((bin) => bin.map((t) => Math.exp(-t))),
              opticalDepth: binDepths,
            },
    };
  });
}

/**
 * ln Σ e^(xᵢ) of the shares' logarithms, relative to the largest: −∞ where every one is.
 */
function logSumOf(logs: ReadonlyArray<number>): number {
  const top = Math.max(...logs);
  if (!Number.isFinite(top)) {
    return top;
  }
  if (logs.length === 1) {
    return top;
  }
  let sum = 0;
  for (const x of logs) {
    sum += Math.exp(x - top);
  }
  return top + Math.log(sum);
}

/**
 * An absorber's channel curve and, if asked, its bin curves under each sun, in one share.
 *
 * @throws RangeError as {@link curveContext}.
 */
function tabulate(
  absorber: AbsorberSpec,
  spectra: ReadonlyArray<BakeSpectrum>,
  withBins: boolean,
): ReadonlyArray<Tabulated> {
  const context = curveContext(absorber, spectra, withBins);
  return curvesFromSums(context, [sumsOf(context, WHOLE)]);
}

/**
 * {@link tabulate} under one sun.
 *
 * @throws RangeError as {@link tabulate}.
 */
function tabulateOne(absorber: AbsorberSpec, spectrum: BakeSpectrum, withBins: boolean): Tabulated {
  const [one] = tabulate(absorber, [spectrum], withBins);
  if (one === undefined) {
    throw new Error("one sun gives one set of curves");
  }
  return one;
}

/**
 * {@link tabulate}'s bin curves, which it makes when asked.
 *
 * @throws Error when it made none.
 */
function withBinCurves({ channels, bins }: Tabulated): {
  readonly channels: AbsorberCurve;
  readonly bins: AbsorberBinCurves;
} {
  if (bins === undefined) {
    throw new Error("the bins' curves were asked for and not made");
  }
  return { channels, bins };
}

/**
 * An absorber's curves of growth under one sun (Design note 5): T_c(u, p̄) per channel on a log grid
 * of column about its vertical column U, from 10⁻³ U, the linear regime, to 10³ U, past any slant
 * path through it, at each of its pressure rows (one for an absorber without lines). The optics
 * worker computes each sun's at arrival, all of them at once by {@link absorberCurvesBySun}.
 *
 * @remarks
 * Each node is {@link channelTransmittance}'s at the row's pressure, then the running minimum along
 * each row's nodes makes T_c non-increasing in u (`decision-r08-t4b-t12c.md` §1). It acts near and
 * past the column at which the light leaves Rec. 709, from up to five nodes before it (ozone's red
 * under the 6,500 K and 30,000 K suns), and can go on holding after the light has come back into
 * the gamut (R08's Risks).
 *
 * @throws RangeError as {@link channelTransmittance}, or for a spec whose column is not above 0.
 */
export function absorberCurve(absorber: AbsorberSpec, spectrum: BakeSpectrum): AbsorberCurve {
  return tabulateOne(absorber, spectrum, false).channels;
}

/**
 * An absorber's curves of growth in each of the 15 bake bins under one sun (Design note 5;
 * `decision-r08-t4b-t12c.md` §1.4), on the channel curve's columns and pressure rows:
 * T_b(u, p̄) = Σ S e^(−σu) T_cell Δλ ÷ Σ S Δλ over the bin's points alone, for the spectral bakes
 * and the reference's 15-wavelength runs.
 *
 * @remarks
 * The weights are S alone, so each bin's curve is a true weighted mean of e^(−σu)T_cell,
 * non-increasing and in (0, 1] with no clamp; a bake reduces its bins to the channels on storage
 * with the signed matching functions. A bake that follows paths applies a bin's curve to each
 * path's column, and one that solves by layers (R08.T14.b) fits it with an exponential sum
 * (correlated-k; Lacis and Oinas 1991, JGR 96, 9027). Each grid point counts in its 25.33 nm bin
 * and 760 nm in the last; the bins' edges are not grid points, so a box's edge moves by up to half
 * a step. The sums are taken relative to the grid's least σ, and again relative to the bin's own
 * where that underflows. {@link absorberCurves} gives both kinds of curve from one set of
 * exponentials.
 *
 * @throws RangeError as {@link absorberCurve}, or for a sun with no light in a bake bin.
 */
export function absorberBinCurves(
  absorber: AbsorberSpec,
  spectrum: BakeSpectrum,
): AbsorberBinCurves {
  return withBinCurves(tabulateOne(absorber, spectrum, true)).bins;
}

/**
 * An absorber's channel curve and its bake bins' curves under one sun, from one set of
 * exponentials, as {@link absorberCurve} and {@link absorberBinCurves} give them: for an optics
 * worker that needs both.
 *
 * @throws RangeError as {@link absorberBinCurves}.
 */
export function absorberCurves(
  absorber: AbsorberSpec,
  spectrum: BakeSpectrum,
): { readonly channels: AbsorberCurve; readonly bins: AbsorberBinCurves } {
  return withBinCurves(tabulateOne(absorber, spectrum, true));
}

/**
 * Each sun's channel curve and bake bins' curves of an absorber, as {@link absorberCurves} gives
 * them sun by sun, for the optics worker at arrival (R08.T4.d): the lines' cell transmittances at
 * each pressure row and column, the costly exponentials, are taken once and shared by every sun,
 * channel and bin.
 *
 * @returns One entry a sun, in `spectra`'s order.
 * @throws RangeError as {@link absorberBinCurves}, for any of the suns.
 */
export function absorberCurvesBySun(
  absorber: AbsorberSpec,
  spectra: ReadonlyArray<BakeSpectrum>,
): ReadonlyArray<{ readonly channels: AbsorberCurve; readonly bins: AbsorberBinCurves }> {
  return tabulate(absorber, spectra, true).map(withBinCurves);
}

/**
 * One optics worker's share of an absorber's curves under each sun, channels and bins
 * (`decision-r08-t4d-kdist-chain.md` §15, ruling 3): the sums over the share's line cells
 * ({@link CurveShare}), which carry the exponentials, for {@link absorberCurvesFromSums} to join.
 * The workers take the brightest sun's curves first, alone, and the other suns' after.
 *
 * @throws RangeError as {@link absorberCurvesBySun}, or for a share that is not one of `count`.
 */
export function absorberCurveSums(
  absorber: AbsorberSpec,
  spectra: ReadonlyArray<BakeSpectrum>,
  share: CurveShare,
): AbsorberCurveSums {
  if (
    !(Number.isInteger(share.count) && share.count >= 1) ||
    !(Number.isInteger(share.index) && share.index >= 0 && share.index < share.count)
  ) {
    throw new RangeError(`a curve share is one of its count, not ${share.index} of ${share.count}`);
  }
  return sumsOf(curveContext(absorber, spectra, true), share);
}

/**
 * Each sun's channel curve and bake bins' curves from every share's sums
 * ({@link absorberCurveSums}), as {@link absorberCurvesBySun} gives them but for the order of the
 * shares' sums.
 *
 * @throws RangeError as {@link absorberCurvesBySun}, or for sums that are not every share of one
 *   split once, under these suns.
 */
export function absorberCurvesFromSums(
  absorber: AbsorberSpec,
  spectra: ReadonlyArray<BakeSpectrum>,
  sums: ReadonlyArray<AbsorberCurveSums>,
): ReadonlyArray<{ readonly channels: AbsorberCurve; readonly bins: AbsorberBinCurves }> {
  return curvesFromSums(curveContext(absorber, spectra, true), sums).map(withBinCurves);
}

/**
 * The optical depth at ln u read from a row of a curve's nodes: linear in ln u on a log–log scale
 * between nodes, proportional to u below the first node (the linear regime), and the last
 * interval's power law beyond the last.
 */
function depthAt(logColumns: Float64Array, depths: Float64Array, row: number, x: number): number {
  const nodes = logColumns.length;
  const last = nodes - 1;
  const offset = row * nodes;
  const x0 = logColumns[0] ?? Number.NaN;
  if (x <= x0) {
    return (depths[offset] ?? Number.NaN) * Math.exp(x - x0);
  }
  let k = 0;
  while (k < last - 1 && (logColumns[k + 1] ?? Number.NaN) < x) {
    k += 1;
  }
  const xa = logColumns[k] ?? Number.NaN;
  const xb = logColumns[k + 1] ?? Number.NaN;
  const a = depths[offset + k] ?? Number.NaN;
  const b = depths[offset + k + 1] ?? Number.NaN;
  const t = (x - xa) / (xb - xa);
  return a > 0 && b > 0 ? Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * t) : a + (b - a) * t;
}

/** The largest optical depth a read takes, −ln {@link ABSORBER_TRANSMITTANCE_FLOOR}. */
const CURVE_DEPTH_MAX = -Math.log(ABSORBER_TRANSMITTANCE_FLOOR);

/**
 * The optical depth at ln u and a pressure from a curve's rows, as the kernels and the twin read it
 * (`decision-r08-t4d-kdist-chain.md`, ruling 3 and §8): each row's {@link depthAt}, then 4-point
 * Lagrange in ln p̄ on the depth τ over the four rows nearest, its weights from the rows' own ln p̄
 * (their spacing halves at 1 MPa ÷ f), unclamped between the rows (at high pressure T at a fixed u
 * is not monotone in p), and τ then clamped to [0, ceiling]; linear in τ between the two rows about
 * the pressure on a curve of under four rows; p̄ held at the first row below and at the read's top
 * above. The one place the read is set (the tool's `cubicInLogPressure` is its twin).
 *
 * @param ceiling - The largest τ the read returns: −ln 10⁻⁶ for a channel curve, ∞ for a bin's.
 * @throws RangeError for a curve of several rows read with no pressure, or one not finite and above
 *   0.
 */
function depthAtPressure(
  logColumns: Float64Array,
  logPressures: Float64Array,
  readTopLogPressure: number,
  depths: Float64Array,
  x: number,
  pressurePa: number | undefined,
  ceiling: number,
): number {
  const rows = logPressures.length;
  if (rows === 1) {
    return depthAt(logColumns, depths, 0, x);
  }
  if (pressurePa === undefined || !(Number.isFinite(pressurePa) && pressurePa > 0)) {
    throw new RangeError(
      `a curve with lines is read at a path's pressure above 0, not ${String(pressurePa)}`,
    );
  }
  const first = logPressures[0] ?? Number.NaN;
  const top = Math.min(logPressures[rows - 1] ?? Number.NaN, readTopLogPressure);
  const y = Math.min(Math.max(Math.log(pressurePa), first), top);
  let r = 0;
  while (r < rows - 2 && (logPressures[r + 1] ?? Number.NaN) < y) {
    r += 1;
  }
  let depth: number;
  if (rows < 4) {
    const lo = logPressures[r] ?? Number.NaN;
    const hi = logPressures[r + 1] ?? Number.NaN;
    const a = depthAt(logColumns, depths, r, x);
    const b = depthAt(logColumns, depths, r + 1, x);
    depth = a + ((b - a) * (y - lo)) / (hi - lo);
  } else {
    const s0 = Math.min(Math.max(r - 1, 0), rows - 4);
    depth = 0;
    for (let k = s0; k < s0 + 4; k += 1) {
      const xk = logPressures[k] ?? Number.NaN;
      let w = 1;
      for (let m = s0; m < s0 + 4; m += 1) {
        if (m !== k) {
          w *= (y - (logPressures[m] ?? Number.NaN)) / (xk - (logPressures[m] ?? Number.NaN));
        }
      }
      depth += w * depthAt(logColumns, depths, k, x);
    }
  }
  return Math.min(Math.max(depth, 0), ceiling);
}

/**
 * The transmittance per channel through a column, read from a curve by its nodes' log–log rule and,
 * for a curve with lines, cubic in ln p between its rows ({@link depthAtPressure}), clamped to
 * [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1] as the curve is.
 *
 * @param amount - u, of the curve's kind.
 * @param pressurePa - The path's absorber-weighted (Curtis–Godson) pressure, Pa: required for a
 *   curve of several rows, ignored for one of a single row.
 * @throws Error for an amount of the other kind.
 * @throws RangeError for an amount that is not finite and at least 0, or a missing pressure.
 */
export function absorberTransmittance(
  curve: AbsorberCurve,
  amount: AbsorberAmount,
  pressurePa: number | undefined,
): Rgb {
  const column = amountFor(amount, curve.kind);
  if (column === 0) {
    return [1, 1, 1];
  }
  const x = Math.log(column);
  const read = (depths: Float64Array): number =>
    clampedTransmittance(
      Math.exp(
        -depthAtPressure(
          curve.logColumns,
          curve.logPressures,
          curve.readTopLogPressure,
          depths,
          x,
          pressurePa,
          CURVE_DEPTH_MAX,
        ),
      ),
    );
  return [read(curve.opticalDepth[0]), read(curve.opticalDepth[1]), read(curve.opticalDepth[2])];
}

/**
 * The transmittance in each bake bin through a column, read from the bins' curves by the same
 * rule as {@link absorberTransmittance}, unclamped.
 *
 * @param amount - u, of the curves' kind.
 * @param pressurePa - The path's absorber-weighted pressure, Pa, as {@link absorberTransmittance}'s.
 * @throws Error for an amount of the other kind.
 * @throws RangeError for an amount that is not finite and at least 0, or a missing pressure.
 */
export function absorberBinTransmittance(
  curves: AbsorberBinCurves,
  amount: AbsorberAmount,
  pressurePa: number | undefined,
): Float64Array {
  const column = amountFor(amount, curves.kind);
  const x = Math.log(column);
  return Float64Array.from(curves.opticalDepth, (depths) =>
    column === 0
      ? 1
      : Math.exp(
          -depthAtPressure(
            curves.logColumns,
            curves.logPressures,
            curves.readTopLogPressure,
            depths,
            x,
            pressurePa,
            Infinity,
          ),
        ),
  );
}

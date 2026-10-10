/**
 * Absorbing gases (plan R08, Design notes 5 and 8; R08.T4.b): their cross-sections, the absorber
 * terms laid on a column, and each sun's curves of growth.
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
 * Stated approximations: one temperature a profile, its column-weighted mean, for the
 * cross-sections ({@link absorberSpecs}); cross-sections held, and labelled, beyond their law's
 * measured temperatures; Karkoschka and Tomasko's methane at infinite pressure, without their
 * finite-pressure correction; and S from 15 bin means, adequate for FGK, A and B stars but not for
 * M dwarfs' TiO bands (R08's Risks).
 */

import type { Rgb } from "../photometry/toneCurve";
import crossSectionsFile from "./absorbers/crossSections.json" with { type: "json" };
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
 */
export type TemperatureLaw = "linear" | "lnQuadratic";

/** Whether a file's string is a {@link TemperatureLaw}. */
function isTemperatureLaw(value: string): value is TemperatureLaw {
  return value === "linear" || value === "lnQuadratic";
}

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

/** `crossSections.json`'s shape, as its JSON import reads it. */
export interface CrossSectionsFile {
  readonly tables: ReadonlyArray<{
    readonly species: string;
    readonly source: string;
    readonly reduction: string;
    readonly firstNm: number;
    readonly stepNm: number;
    readonly temperaturesK: ReadonlyArray<number>;
    readonly temperatureLaw: string;
    readonly measuredRangeK: ReadonlyArray<number>;
    readonly rows: ReadonlyArray<ReadonlyArray<number>>;
  }>;
}

/** How far a row's wavelength may lie from its grid point, nm. */
const GRID_TOLERANCE_NM = 1e-9;

/** How far a wavelength may lie from a grid point and still be read as on it, in grid steps. */
const GRID_TOLERANCE_STEPS = 1e-9;

/**
 * A cross-sections file's tables, checked: unique species, ascending temperatures and a law and a
 * measured range that fit them, rows on the grid with a finite σ ≥ 0 at each temperature, and a
 * grid that covers {@link BAKE_RANGE_NM} with a point on each of its ends.
 *
 * @throws Error naming the table and the check it fails.
 */
export function readCrossSections(file: CrossSectionsFile): ReadonlyArray<CrossSectionTable> {
  const seen = new Set<string>();
  return file.tables.map((table) => {
    const { species, firstNm, stepNm, temperaturesK, rows } = table;
    const where = `crossSections.json's ${species}`;
    if (seen.has(species)) {
      throw new Error(`${where} is listed twice`);
    }
    seen.add(species);
    if (
      temperaturesK.length === 0 ||
      temperaturesK.some((t, i) => !(t > (temperaturesK[i - 1] ?? 0)))
    ) {
      throw new Error(`${where}'s temperatures do not ascend from above 0 K`);
    }
    const law = table.temperatureLaw;
    if (!isTemperatureLaw(law) || (law === "lnQuadratic" && temperaturesK.length !== 3)) {
      throw new Error(`${where}'s temperature law ${law} does not fit its temperatures`);
    }
    const [lowK, highK] = table.measuredRangeK;
    const coldest = temperaturesK[0] ?? Number.NaN;
    const hottest = temperaturesK[temperaturesK.length - 1] ?? Number.NaN;
    if (
      lowK === undefined ||
      highK === undefined ||
      table.measuredRangeK.length !== 2 ||
      !(lowK > 0 && highK > lowK) ||
      (law === "linear" && !(lowK >= coldest && highK <= hottest))
    ) {
      throw new Error(`${where}'s measured range does not fit its law and temperatures`);
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
      species,
      source: `${table.source}. ${table.reduction}`,
      firstNm,
      stepNm,
      temperaturesK,
      temperatureLaw: law,
      measuredRangeK: [lowK, highK],
      crossSectionsM2: columns,
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
 *   and deeper.
 */
export const ABSORBER_CROSS_SECTIONS: ReadonlyArray<CrossSectionTable> =
  readCrossSections(crossSectionsFile);

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
 * A species' cross-sections at a temperature, or `undefined` if the client holds none for it.
 *
 * @remarks
 * By the table's {@link TemperatureLaw} within its measured range. Beyond it the nearest end's
 * values are held and the result is `estimated`: methane above 300 K is R08.T4.c's.
 *
 * @param tables - The tables to look in; the committed ones unless a test gives its own.
 * @throws RangeError for a temperature that is not finite and above 0 K.
 */
export function crossSectionAt(
  species: string,
  temperatureK: number,
  tables: ReadonlyArray<CrossSectionTable> = ABSORBER_CROSS_SECTIONS,
): SpeciesCrossSection | undefined {
  if (!(Number.isFinite(temperatureK) && temperatureK > 0)) {
    throw new RangeError(
      `a cross-section's temperature is finite and above 0 K, not ${temperatureK}`,
    );
  }
  const table = tables.find((t) => t.species === species);
  if (table === undefined) {
    return undefined;
  }
  const [lowK, highK] = table.measuredRangeK;
  const atK = Math.min(Math.max(temperatureK, lowK), highK);
  let values: Float64Array;
  switch (table.temperatureLaw) {
    case "linear":
      values = linearInTemperature(table, atK);
      break;
    case "lnQuadratic":
      values = lnQuadraticInTemperature(table, atK);
      break;
  }
  return {
    species,
    provenance: temperatureK < lowK || temperatureK > highK ? "estimated" : "measured",
    temperatureK,
    firstNm: table.firstNm,
    stepNm: table.stepNm,
    crossSectionsM2: values,
    source: table.source,
  };
}

/**
 * σ linear in T between the two tabulated temperatures around it, or from the nearest pair beyond
 * them, floored at 0; a tabulated temperature's own values there.
 */
function linearInTemperature(table: CrossSectionTable, temperatureK: number): Float64Array {
  const { temperaturesK, crossSectionsM2 } = table;
  const last = temperaturesK.length - 1;
  const exact = temperaturesK.indexOf(temperatureK);
  if (exact >= 0 || last === 0) {
    return Float64Array.from(crossSectionsM2[Math.max(exact, 0)] ?? []);
  }
  let k = 0;
  while (k < last - 1 && (temperaturesK[k + 1] ?? Number.POSITIVE_INFINITY) < temperatureK) {
    k += 1;
  }
  const below = crossSectionsM2[k] ?? new Float64Array(0);
  const above = crossSectionsM2[k + 1] ?? new Float64Array(0);
  const lowK = temperaturesK[k] ?? Number.NaN;
  const t = (temperatureK - lowK) / ((temperaturesK[k + 1] ?? Number.NaN) - lowK);
  return below.map((v, i) => Math.max(v + ((above[i] ?? Number.NaN) - v) * t, 0));
}

/**
 * ln σ the Lagrange quadratic through a table's three temperatures (Eq. 8); at a point with a 0 at
 * any of the three, {@link linearInTemperature}'s value there.
 */
function lnQuadraticInTemperature(table: CrossSectionTable, temperatureK: number): Float64Array {
  const [t0, t1, t2] = table.temperaturesK;
  const [s0, s1, s2] = table.crossSectionsM2;
  if (
    t0 === undefined ||
    t1 === undefined ||
    t2 === undefined ||
    s0 === undefined ||
    s1 === undefined ||
    s2 === undefined
  ) {
    throw new Error(`${table.species}'s quadratic law needs three temperatures`);
  }
  const exact = table.temperaturesK.indexOf(temperatureK);
  if (exact >= 0) {
    return Float64Array.from(table.crossSectionsM2[exact] ?? []);
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
 * One absorbing species on its layer, as a body's record gives it (decision-composition §1.9 and
 * §2: P14.T24.c's `absorbers[]`, `{ species, column_m2, base_pa, top_pa }`, or a gas of T24.a's
 * fractions that absorbs, such as methane, from the datum to the top).
 */
export interface AbsorberLayer {
  /** The registry key. */
  readonly species: string;
  /** The species' vertical column, molecules m⁻², at least 0. */
  readonly columnPerM2: number;
  /** The layer's base, Pa: the mixing ratio is constant from here up to the top, 0 outside. */
  readonly basePa: number;
  /** The layer's top, Pa, below the base; 0 or less than the column's top for no top. */
  readonly topPa: number;
}

/** One species of an absorber spec. */
export interface AbsorberSpecies {
  /** The registry key. */
  readonly species: string;
  /** Its vertical column, molecules m⁻². */
  readonly columnPerM2: number;
  /** Its cross-sections at the spec's temperature; `undefined` where the client holds none. */
  readonly crossSection: SpeciesCrossSection | undefined;
}

/**
 * The absorbing species on one vertical profile, which share one curve of growth a sun (Design
 * note 5; decision-composition §1.9): a layer between two pressures of a column, in which each
 * species' mixing ratio is constant.
 */
export interface AbsorberSpec {
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
  /** The temperature the cross-sections are taken at, K: the layer's column-weighted mean. */
  readonly temperatureK: number;
}

/** Why a listed absorber is drawn approximately or not at all, for `atmosphereApproximate`. */
export type AbsorberApproximationReason =
  /** The client holds no cross-sections for the species: it draws nothing. */
  | "noCrossSections"
  /** Its layer's temperature lies beyond its table's measured range: the nearest end is held. */
  | "temperatureOutsideTable"
  /**
   * More vertical profiles than {@link ABSORBER_COLUMNS}, its own of the least optical depth: it
   * draws nothing.
   */
  | "noColumnFree";

/**
 * A listed absorber drawn approximately or not at all: drawn as nothing (`none`) for want of
 * cross-sections or of a column, or with held cross-sections (`estimated`) beyond their table's
 * measured temperatures.
 */
export type AbsorberApproximation =
  | {
      readonly species: string;
      readonly provenance: "none";
      readonly reason: Exclude<AbsorberApproximationReason, "temperatureOutsideTable">;
    }
  | {
      readonly species: string;
      readonly provenance: "estimated";
      readonly reason: "temperatureOutsideTable";
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

/** A layer between two pressures, clamped to the column, laid on the column's levels. */
function layerOnColumn(column: AtmosphereColumn, basePa: number, topPa: number): LayerOnColumn {
  const base = atPressure(column, basePa);
  const top = atPressure(column, topPa);
  const altitudesM = [base.heightM];
  const relative = [densityAt(column.density, base.heightM)];
  const temperaturesK = [base.temperatureK];
  let belowM: number | undefined;
  let aboveM: number | undefined;
  for (const [i, h] of column.altitudesM.entries()) {
    if (h < base.heightM) {
      belowM = h;
    } else if (h > base.heightM && h < top.heightM) {
      altitudesM.push(h);
      relative.push(column.density.relative[i] ?? Number.NaN);
      temperaturesK.push(column.temperaturesK[i] ?? Number.NaN);
    } else if (h > top.heightM && aboveM === undefined) {
      aboveM = h;
    }
  }
  altitudesM.push(top.heightM);
  relative.push(densityAt(column.density, top.heightM));
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

/** The column-weighted mean temperature over a layer, K: ∫ n T dz ÷ ∫ n dz, by the trapezoid rule. */
function layerTemperatureK(layer: LayerOnColumn): number {
  let weight = 0;
  let sum = 0;
  for (let i = 1; i < layer.altitudesM.length; i += 1) {
    const dz = (layer.altitudesM[i] ?? Number.NaN) - (layer.altitudesM[i - 1] ?? Number.NaN);
    const n0 = layer.relative[i - 1] ?? Number.NaN;
    const n1 = layer.relative[i] ?? Number.NaN;
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
 * A body's absorbers merged into one spec per vertical profile on its column, each species' cross-
 * sections taken at its profile's temperature (decision-composition §1.9).
 *
 * @remarks
 * Layers merge when their base and top, clamped to the column's datum and top, are equal: their
 * mixing ratios are then constant in the same air, so σ_mix = Σ xᵢσᵢ, xᵢ = Uᵢ ÷ U, is exact for
 * every path. A layer of column 0 has nothing to draw and is left out. A spec's temperature is the
 * layer's column-weighted mean, ∫ n T dz ÷ ∫ n dz, which gives the vertical column's mean σ exactly
 * where σ is linear in T (ozone) and nearly so for methane's Eq. 8; a slant path through part of
 * the layer reads the same σ, a stated approximation.
 *
 * Past {@link ABSORBER_COLUMNS} profiles, the five of greatest optical depth are kept
 * ({@link absorberRankingDepth}; `decision-r08-cmf-licence.md` §2). Equal depths keep the earlier
 * listed, and the kept profiles take their columns in the order listed.
 *
 * Reported, never dropped silently: a species with no cross-sections (`noCrossSections`, drawn as
 * nothing), one whose temperature lies beyond its table's measured range
 * (`temperatureOutsideTable`), and every species of a dropped profile (`noColumnFree`, drawn as
 * nothing), grouped by profile, each profile in the order its first species was listed. A
 * profile none of whose species has cross-sections takes no column.
 *
 * @param tables - The cross-sections to take; the committed ones unless a test gives its own.
 * @throws RangeError for a species listed twice, a column that is not finite and at least 0, a
 *   layer whose base does not lie below its top, or a layer outside its column.
 */
export function absorberSpecs(
  layers: ReadonlyArray<AbsorberLayer>,
  column: AtmosphereColumn,
  tables: ReadonlyArray<CrossSectionTable> = ABSORBER_CROSS_SECTIONS,
): AbsorberSpecs {
  const listed = new Set<string>();
  const profiles = new Map<string, { basePa: number; topPa: number; layers: AbsorberLayer[] }>();
  for (const layer of layers) {
    if (listed.has(layer.species)) {
      throw new RangeError(`absorber ${layer.species} is listed twice`);
    }
    listed.add(layer.species);
    if (!(Number.isFinite(layer.columnPerM2) && layer.columnPerM2 >= 0)) {
      throw new RangeError(
        `absorber ${layer.species}'s column is finite and at least 0, not ${layer.columnPerM2} m⁻²`,
      );
    }
    const clamped = clampedLayer(column, layer.basePa, layer.topPa);
    if (layer.columnPerM2 === 0) {
      continue;
    }
    const key = `${clamped.basePa}:${clamped.topPa}`;
    const profile = profiles.get(key) ?? { ...clamped, layers: [] };
    profile.layers.push(layer);
    profiles.set(key, profile);
  }
  const built = [...profiles.values()].map((profile) => {
    const temperatureK = layerTemperatureK(layerOnColumn(column, profile.basePa, profile.topPa));
    const species = profile.layers.map((layer): AbsorberSpecies => ({
      species: layer.species,
      columnPerM2: layer.columnPerM2,
      crossSection: crossSectionAt(layer.species, temperatureK, tables),
    }));
    const spec: AbsorberSpec | undefined = species.some((s) => s.crossSection !== undefined)
      ? {
          name: `absorber:${species.map((s) => s.species).join("+")}`,
          species,
          columnPerM2: species.reduce((sum, s) => sum + s.columnPerM2, 0),
          basePa: profile.basePa,
          topPa: profile.topPa,
          temperatureK,
        }
      : undefined;
    return { species, spec };
  });
  const kept = keptProfiles(built.flatMap(({ spec }) => (spec === undefined ? [] : [spec])));
  const specs: AbsorberSpec[] = [];
  const approximations: AbsorberApproximation[] = [];
  for (const { species, spec } of built) {
    const drawn = spec !== undefined && kept.has(spec);
    const dropped = spec !== undefined && !drawn;
    for (const s of species) {
      if (dropped) {
        approximations.push({ species: s.species, provenance: "none", reason: "noColumnFree" });
      } else if (s.crossSection === undefined) {
        approximations.push({ species: s.species, provenance: "none", reason: "noCrossSections" });
      } else if (s.crossSection.provenance === "estimated") {
        approximations.push({
          species: s.species,
          provenance: "estimated",
          reason: "temperatureOutsideTable",
        });
      }
    }
    if (drawn) {
      specs.push(spec);
    }
  }
  return { specs, approximations };
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
 * −ln[Σ w_c e^(−Σᵢ Uᵢσᵢ) ÷ Σ w_c] through its vertical column, w_c = max(c̄_c, 0) on σ's grid over
 * the bake range under an equal-energy spectrum, with the species that have cross-sections at the
 * profile's temperature.
 *
 * @remarks
 * The same for every sun, since the shared per-planet table is built once per body. The weights
 * are clipped so that each term is a true weighted mean, positive and increasing in U, which is all
 * a ranking needs; the drawn curves keep the signed weights. Each sum is taken relative to its least
 * weighed σ, so the depth stays finite however deep.
 */
export function absorberRankingDepth(spec: AbsorberSpec): number {
  const { crossSectionM2, weights } = absorberOptics(spec, EQUAL_ENERGY);
  const u = spec.columnPerM2;
  return Math.max(
    ...weights.map((channel) => {
      let least = Infinity;
      for (const [i, w] of channel.entries()) {
        least = w > 0 ? Math.min(least, crossSectionM2[i] ?? Number.NaN) : least;
      }
      let passed = 0;
      let total = 0;
      for (const [i, w] of channel.entries()) {
        if (w > 0) {
          passed += w * Math.exp(-((crossSectionM2[i] ?? Number.NaN) - least) * u);
          total += w;
        }
      }
      return least * u - Math.log(passed / total);
    }),
  );
}

/**
 * An absorber's term on its column (Design note 2): its layer's share of the column's density,
 * absorbing by its curves of growth alone.
 *
 * @remarks
 * The density is the column's relative density from the layer's base to its top, with a level at
 * each edge, and 0 at the column's next level outside each edge, which the `tabulated` rule
 * ramps to over that one interval. Its number density at relative density 1 is U ÷ ∫ ρ dz, so
 * that the vertical column is the spec's whatever the ramps hold. Scattering and `absorption` are
 * 0 and the phase `none`: the term's absorption is each sun's −ln T_c(u), which the tables add from
 * R08.T6.c (`medium.ts`'s `checkNoAbsorbers` refuses it until then).
 *
 * @throws RangeError for a spec whose layer lies outside the column, or whose column is not above 0.
 */
export function absorberTerm(absorber: AbsorberSpec, column: AtmosphereColumn): MediumTerm {
  if (!(Number.isFinite(absorber.columnPerM2) && absorber.columnPerM2 > 0)) {
    throw new RangeError(
      `absorber ${absorber.name}'s column is above 0, not ${absorber.columnPerM2} m⁻²`,
    );
  }
  const clamped = clampedLayer(column, absorber.basePa, absorber.topPa);
  const layer = layerOnColumn(column, clamped.basePa, clamped.topPa);
  const altitudes = [...layer.altitudesM];
  const relative = [...layer.relative];
  if (layer.belowM !== undefined) {
    altitudes.unshift(layer.belowM);
    relative.unshift(0);
  }
  if (layer.aboveM !== undefined) {
    altitudes.push(layer.aboveM);
    relative.push(0);
  }
  const density: TabulatedDensity = tabulatedDensity(
    Float64Array.from(altitudes),
    Float64Array.from(relative),
  );
  const lengthM = columnLengthM(density, column.topHeightM);
  return {
    name: absorber.name,
    density,
    scattering: [0, 0, 0],
    absorption: [0, 0, 0],
    phase: { kind: "none" },
    absorber: { spec: absorber, numberDensityPerM3: absorber.columnPerM2 / lengthM },
  };
}

/**
 * The least transmittance a channel's curve holds: 10⁻⁶, an optical depth of 13.8
 * (`decision-r08-t4b-t12c.md` §1). Below it the light is black on any display, and the signed sum
 * may leave Rec. 709 (methane's red goes negative) or underflow.
 */
export const ABSORBER_TRANSMITTANCE_FLOOR = 1e-6;

/** One absorber under one sun on σ's grid: what its curves of growth are summed from. */
export interface AbsorberOptics {
  /** The grid, nm: the bake range at the finest step of the absorber's species. */
  readonly wavelengthsNm: Float64Array;
  /** σ_mix = Σ xᵢσᵢ at each point, m² per molecule of the absorber's mixture. */
  readonly crossSectionM2: Float64Array;
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

/** The grid step when no species of an absorber holds cross-sections, nm. */
const DEFAULT_STEP_NM = 1;

/** σ at a wavelength on a species' grid, linear between its points. */
function sigmaAt(cross: SpeciesCrossSection, wavelengthNm: number): number {
  const x = (wavelengthNm - cross.firstNm) / cross.stepNm;
  const k = Math.floor(x + GRID_TOLERANCE_STEPS);
  const t = Math.max(x - k, 0);
  const a = cross.crossSectionsM2[k] ?? Number.NaN;
  return t <= GRID_TOLERANCE_STEPS ? a : a + ((cross.crossSectionsM2[k + 1] ?? Number.NaN) - a) * t;
}

/**
 * Refuses a species' cross-sections that do not cover {@link BAKE_RANGE_NM} or hold a σ that is not
 * finite and at least 0, which would otherwise turn into a curve black at the floor.
 */
function checkCrossSection(cross: SpeciesCrossSection): void {
  const [low, high] = BAKE_RANGE_NM;
  const last = cross.firstNm + (cross.crossSectionsM2.length - 1) * cross.stepNm;
  if (!(cross.stepNm > 0 && cross.firstNm <= low && last >= high)) {
    throw new RangeError(
      `${cross.species}'s cross-sections span ${cross.firstNm}–${last} nm, not the bake range ${low}–${high} nm`,
    );
  }
  if (!cross.crossSectionsM2.every((sigma) => Number.isFinite(sigma) && sigma >= 0)) {
    throw new RangeError(
      `${cross.species}'s cross-sections hold a σ that is not finite and at least 0`,
    );
  }
}

/**
 * An absorber's mixed cross-section and a sun's light and channel weights on σ's grid (Design
 * note 5): the bake range at the finest step among its species, so that a single species is summed
 * on its own grid, point for point.
 *
 * @throws RangeError for a species whose cross-sections do not cover the bake range or are not
 *   finite and at least 0.
 */
export function absorberOptics(absorber: AbsorberSpec, spectrum: BakeSpectrum): AbsorberOptics {
  const measured = absorber.species.flatMap((s) => (s.crossSection === undefined ? [] : [s]));
  for (const s of measured) {
    if (s.crossSection !== undefined) {
      checkCrossSection(s.crossSection);
    }
  }
  const stepNm = Math.min(
    ...measured.map((s) => s.crossSection?.stepNm ?? DEFAULT_STEP_NM),
    DEFAULT_STEP_NM,
  );
  const [low, high] = BAKE_RANGE_NM;
  const count = Math.round((high - low) / stepNm) + 1;
  const wavelengthsNm = Float64Array.from({ length: count }, (_, i) => low + i * stepNm);
  const crossSectionM2 = new Float64Array(count);
  const light = new Float64Array(count);
  const bins = new Uint8Array(count);
  const weights: [Float64Array, Float64Array, Float64Array] = [
    new Float64Array(count),
    new Float64Array(count),
    new Float64Array(count),
  ];
  for (const [i, nm] of wavelengthsNm.entries()) {
    let sigma = 0;
    for (const s of measured) {
      if (s.crossSection !== undefined) {
        sigma += (s.columnPerM2 / absorber.columnPerM2) * sigmaAt(s.crossSection, nm);
      }
    }
    crossSectionM2[i] = sigma;
    const s = bakeSpectrumAt(spectrum, nm) * stepNm * (i === 0 || i === count - 1 ? 0.5 : 1);
    light[i] = s;
    bins[i] = Math.min(Math.floor((nm - low) / BAKE_BIN_WIDTH_NM), BAKE_BIN_COUNT - 1);
    const matching = matchingFunctionsAt(nm);
    for (const [c, channel] of weights.entries()) {
      channel[i] = (matching[c] ?? Number.NaN) * s;
    }
  }
  return { wavelengthsNm, crossSectionM2, light, weights, bins };
}

/** An absorber's optics with the curves' rule applied, ready to be summed at any column. */
interface PreparedOptics {
  readonly crossSectionM2: Float64Array;
  /** The least σ on the grid, m²: every sum is taken relative to it. */
  readonly leastM2: number;
  /** Each channel's weights by the rule, and their sums. */
  readonly channelWeights: readonly [Float64Array, Float64Array, Float64Array];
  readonly channelTotals: readonly [number, number, number];
  readonly light: Float64Array;
  readonly bins: Uint8Array;
  /** Each bin's light, Σ S Δλ over its points. */
  readonly binTotals: Float64Array;
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
  const binTotals = new Float64Array(BAKE_BIN_COUNT);
  for (const [i, s] of optics.light.entries()) {
    const b = optics.bins[i] ?? 0;
    binTotals[b] = (binTotals[b] ?? 0) + s;
  }
  return {
    crossSectionM2: optics.crossSectionM2,
    leastM2: optics.crossSectionM2.reduce((least, sigma) => Math.min(least, sigma), Infinity),
    channelWeights: [red.weights, green.weights, blue.weights],
    channelTotals: [red.total, green.total, blue.total],
    light: optics.light,
    bins: optics.bins,
    binTotals,
  };
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
 * The channels' clamped transmittances and, if asked, each bake bin's optical depth through a
 * column, from one exponential a point: e^(−(σ − σ_min)u), so that no sum underflows before
 * its own least term.
 */
function sumsAt(
  p: PreparedOptics,
  columnPerM2: number,
  withBins: boolean,
): { readonly channels: Rgb; readonly binDepths: Float64Array | undefined } {
  const u = columnPerM2;
  const [red, green, blue] = p.channelWeights;
  let passedRed = 0;
  let passedGreen = 0;
  let passedBlue = 0;
  const binPassed = withBins ? new Float64Array(BAKE_BIN_COUNT) : undefined;
  for (let i = 0; i < p.crossSectionM2.length; i += 1) {
    const e = Math.exp(-((p.crossSectionM2[i] ?? Number.NaN) - p.leastM2) * u);
    passedRed += (red[i] ?? Number.NaN) * e;
    passedGreen += (green[i] ?? Number.NaN) * e;
    passedBlue += (blue[i] ?? Number.NaN) * e;
    if (binPassed !== undefined) {
      const b = p.bins[i] ?? 0;
      binPassed[b] = (binPassed[b] ?? 0) + (p.light[i] ?? Number.NaN) * e;
    }
  }
  const scale = Math.exp(-p.leastM2 * u);
  const [totalRed, totalGreen, totalBlue] = p.channelTotals;
  const channels: Rgb = [
    clampedTransmittance((scale * passedRed) / totalRed),
    clampedTransmittance((scale * passedGreen) / totalGreen),
    clampedTransmittance((scale * passedBlue) / totalBlue),
  ];
  if (binPassed === undefined) {
    return { channels, binDepths: undefined };
  }
  const binDepths = binPassed.map((passed, b) => {
    const total = p.binTotals[b] ?? Number.NaN;
    return passed > 0 ? p.leastM2 * u - Math.log(passed / total) : binDepthAlone(p, b, u, total);
  });
  return { channels, binDepths };
}

/** A bin's optical depth summed relative to its own least σ, where the shared sum underflows. */
function binDepthAlone(p: PreparedOptics, bin: number, u: number, total: number): number {
  let least = Infinity;
  for (const [i, b] of p.bins.entries()) {
    if (b === bin) {
      least = Math.min(least, p.crossSectionM2[i] ?? Number.NaN);
    }
  }
  let passed = 0;
  for (const [i, b] of p.bins.entries()) {
    if (b === bin) {
      passed +=
        (p.light[i] ?? Number.NaN) * Math.exp(-((p.crossSectionM2[i] ?? Number.NaN) - least) * u);
    }
  }
  return least * u - Math.log(passed / total);
}

/**
 * The transmittance per channel through a column of an absorber's mixture, by the curves' rule
 * (`decision-r08-t4b-t12c.md` §1): T_c = Σ c̄_c S e^(−σu) Δλ ÷ Σ c̄_c S Δλ with the signed matching
 * functions (clipped at zero only in a channel where the sun's own colour is not positive),
 * clamped to [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1].
 *
 * @remarks
 * The rule lives in this module's weighting alone, so that a ruling on the weights changes one
 * place. Unlike a weighted mean of e^(−σu) the signed sum can pass 1 (blue, whose weight is
 * negative over 500–620 nm, under cool stars) and fall below 0 (methane's red, once the light
 * leaves Rec. 709): those are where the clamp acts. A curve on a grid of columns also takes the
 * running minimum ({@link absorberCurve}); this single column does not.
 *
 * @param columnPerM2 - u, molecules m⁻², at least 0.
 * @throws RangeError for a column that is not finite and at least 0, or a sun with no light in a
 *   channel's weights.
 */
export function channelTransmittance(optics: AbsorberOptics, columnPerM2: number): Rgb {
  checkColumn(columnPerM2);
  return sumsAt(prepare(optics), columnPerM2, false).channels;
}

/** Refuses a column that is not finite and at least 0. */
function checkColumn(columnPerM2: number): void {
  if (!(Number.isFinite(columnPerM2) && columnPerM2 >= 0)) {
    throw new RangeError(`a column is finite and at least 0, not ${columnPerM2} m⁻²`);
  }
}

/** The decades of the curve's column grid about the absorber's vertical column U: 10⁻³ U to 10³ U. */
export const ABSORBER_CURVE_DECADES: readonly [number, number] = [-3, 3];

/** The curve's nodes per decade of column. */
export const ABSORBER_CURVE_NODES_PER_DECADE = 40;

/** One absorber's curves of growth under one sun, computed at arrival (Design note 5). */
export interface AbsorberCurve {
  /** ln(u ÷ 1 m⁻²) at each node, ascending: {@link ABSORBER_CURVE_DECADES} about U, evenly. */
  readonly logColumns: Float64Array;
  /**
   * T_c(u) per channel at each node, in [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1] and
   * non-increasing along the nodes.
   */
  readonly transmittance: readonly [Float64Array, Float64Array, Float64Array];
  /** −ln T_c(u) per channel at each node, in [0, 13.8]. */
  readonly opticalDepth: readonly [Float64Array, Float64Array, Float64Array];
}

/** One absorber's curves of growth in each bake bin under one sun (Design note 5). */
export interface AbsorberBinCurves {
  /** ln(u ÷ 1 m⁻²) at each node, the channel curve's. */
  readonly logColumns: Float64Array;
  /** T_b(u) at each node, one array a bake bin: the bin's mean of e^(−σu) weighted by S. */
  readonly transmittance: ReadonlyArray<Float64Array>;
  /** −ln T_b(u) at each node, one array a bake bin, finite however deep. */
  readonly opticalDepth: ReadonlyArray<Float64Array>;
}

/** The curve's columns, molecules m⁻², about a vertical column U. */
function curveColumns(absorber: AbsorberSpec): Float64Array {
  if (!(Number.isFinite(absorber.columnPerM2) && absorber.columnPerM2 > 0)) {
    throw new RangeError(
      `absorber ${absorber.name}'s column is above 0, not ${absorber.columnPerM2} m⁻²`,
    );
  }
  const [first, last] = ABSORBER_CURVE_DECADES;
  const nodes = (last - first) * ABSORBER_CURVE_NODES_PER_DECADE + 1;
  return Float64Array.from(
    { length: nodes },
    (_, k) => absorber.columnPerM2 * 10 ** (first + k / ABSORBER_CURVE_NODES_PER_DECADE),
  );
}

/**
 * An absorber's channel curve and, if asked, its bin curves under one sun, from one exponential a
 * point and a column.
 *
 * @throws RangeError as {@link channelTransmittance}, for a spec whose column is not above 0, or,
 *   for the bins, a sun with no light in a bake bin.
 */
function tabulate(
  absorber: AbsorberSpec,
  spectrum: BakeSpectrum,
  withBins: boolean,
): { readonly channels: AbsorberCurve; readonly bins: AbsorberBinCurves | undefined } {
  const columns = curveColumns(absorber);
  const prepared = prepare(absorberOptics(absorber, spectrum));
  if (withBins && !prepared.binTotals.every((total) => total > 0)) {
    throw new RangeError("the sun has no light in a bake bin, whose curve would be undefined");
  }
  const transmittance: [Float64Array, Float64Array, Float64Array] = [
    new Float64Array(columns.length),
    new Float64Array(columns.length),
    new Float64Array(columns.length),
  ];
  const binDepths = withBins
    ? Array.from({ length: BAKE_BIN_COUNT }, () => new Float64Array(columns.length))
    : undefined;
  for (const [k, u] of columns.entries()) {
    const sums = sumsAt(prepared, u, withBins);
    for (const [c, channel] of transmittance.entries()) {
      const t = sums.channels[c] ?? Number.NaN;
      channel[k] = k === 0 ? t : Math.min(t, channel[k - 1] ?? Number.NaN);
    }
    for (const [b, bin] of (binDepths ?? []).entries()) {
      bin[k] = sums.binDepths?.[b] ?? Number.NaN;
    }
  }
  const logColumns = columns.map(Math.log);
  return {
    channels: {
      logColumns,
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
            logColumns,
            transmittance: binDepths.map((bin) => bin.map((t) => Math.exp(-t))),
            opticalDepth: binDepths,
          },
  };
}

/**
 * An absorber's curves of growth under one sun (Design note 5): T_c(u) per channel on a log grid
 * of column about its vertical column U, from 10⁻³ U, the linear regime, to 10³ U, past any slant
 * path through it. The optics worker computes each sun's at arrival.
 *
 * @remarks
 * Each node is {@link channelTransmittance}'s, then the running minimum along the nodes makes T_c
 * non-increasing in u (`decision-r08-t4b-t12c.md` §1). It acts near and past the column at which
 * the light leaves Rec. 709, from up to five nodes before it (ozone's red under the 6,500 K and
 * 30,000 K suns), and can go on holding after the light has come back into the gamut (R08's
 * Risks).
 *
 * @throws RangeError as {@link channelTransmittance}, or for a spec whose column is not above 0.
 */
export function absorberCurve(absorber: AbsorberSpec, spectrum: BakeSpectrum): AbsorberCurve {
  return tabulate(absorber, spectrum, false).channels;
}

/**
 * An absorber's curves of growth in each of the 15 bake bins under one sun (Design note 5;
 * `decision-r08-t4b-t12c.md` §1.4), on the channel curve's columns: T_b(u) = Σ S e^(−σu) Δλ ÷
 * Σ S Δλ over the bin's points alone, for the spectral bakes and the reference's 15-wavelength
 * runs.
 *
 * @remarks
 * The weights are S alone, so each bin's curve is a true weighted mean of e^(−σu), non-increasing
 * and in (0, 1] with no clamp; a bake reduces its bins to the channels on storage with the signed
 * matching functions. A bake that follows paths applies a bin's curve to each path's column, and
 * one that solves by layers (R08.T14.b) fits it with an exponential sum (correlated-k; Lacis and
 * Oinas 1991, JGR 96, 9027). Each grid point counts in its 25.33 nm bin and 760 nm in the last; the
 * bins' edges are not grid points, so a box's edge moves by up to half a step. The sums are taken
 * relative to the grid's least σ, and again relative to the bin's own where that underflows.
 * {@link absorberCurves} gives both kinds of curve from one set of exponentials.
 *
 * @throws RangeError as {@link absorberCurve}, or for a sun with no light in a bake bin.
 */
export function absorberBinCurves(
  absorber: AbsorberSpec,
  spectrum: BakeSpectrum,
): AbsorberBinCurves {
  const { bins } = tabulate(absorber, spectrum, true);
  if (bins === undefined) {
    throw new Error("the bins' curves were asked for and not made");
  }
  return bins;
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
  const { channels, bins } = tabulate(absorber, spectrum, true);
  if (bins === undefined) {
    throw new Error("the bins' curves were asked for and not made");
  }
  return { channels, bins };
}

/**
 * The optical depth at ln u read from a curve's nodes: linear in ln u on a log–log scale between
 * nodes, proportional to u below the first node (the linear regime), and the last interval's power
 * law beyond the last.
 */
function depthAt(logColumns: Float64Array, depths: Float64Array, x: number): number {
  const last = logColumns.length - 1;
  const x0 = logColumns[0] ?? Number.NaN;
  if (x <= x0) {
    return (depths[0] ?? Number.NaN) * Math.exp(x - x0);
  }
  let k = 0;
  while (k < last - 1 && (logColumns[k + 1] ?? Number.NaN) < x) {
    k += 1;
  }
  const xa = logColumns[k] ?? Number.NaN;
  const xb = logColumns[k + 1] ?? Number.NaN;
  const a = depths[k] ?? Number.NaN;
  const b = depths[k + 1] ?? Number.NaN;
  const t = (x - xa) / (xb - xa);
  return a > 0 && b > 0 ? Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * t) : a + (b - a) * t;
}

/**
 * The transmittance per channel through a column, read from a curve by its nodes' log–log rule,
 * clamped to [{@link ABSORBER_TRANSMITTANCE_FLOOR}, 1] as the curve is.
 *
 * @param columnPerM2 - u, molecules m⁻², at least 0.
 * @throws RangeError for a column that is not finite and at least 0.
 */
export function absorberTransmittance(curve: AbsorberCurve, columnPerM2: number): Rgb {
  checkColumn(columnPerM2);
  if (columnPerM2 === 0) {
    return [1, 1, 1];
  }
  const x = Math.log(columnPerM2);
  const read = (depths: Float64Array): number =>
    clampedTransmittance(Math.exp(-depthAt(curve.logColumns, depths, x)));
  return [read(curve.opticalDepth[0]), read(curve.opticalDepth[1]), read(curve.opticalDepth[2])];
}

/**
 * The transmittance in each bake bin through a column, read from the bins' curves by the same
 * rule as {@link absorberTransmittance}, unclamped.
 *
 * @param columnPerM2 - u, molecules m⁻², at least 0.
 * @throws RangeError for a column that is not finite and at least 0.
 */
export function absorberBinTransmittance(
  curves: AbsorberBinCurves,
  columnPerM2: number,
): Float64Array {
  checkColumn(columnPerM2);
  const x = Math.log(columnPerM2);
  return Float64Array.from(curves.opticalDepth, (depths) =>
    columnPerM2 === 0 ? 1 : Math.exp(-depthAt(curves.logColumns, depths, x)),
  );
}

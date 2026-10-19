/**
 * `node scripts/atmosphereData.mjs`: reduces the atmosphere's fetched data to the committed values
 * the renderer reads (plan R08, R08.T4.a), since a renderer test fetches and reads no raw file.
 *
 * @remarks
 * Each input is checked against its SHA-256 and never committed by this tool; only the reduced
 * values are, with their citations in `NOTICE`'s Data section (R05.T12.d's ruling,
 * `decisions-r05.md` item 4, and `decision-r08-licences.md`). Later R08 tasks add their own
 * subcommands (R08.T4.b's cross-sections, R08.T5.b's refractive indices).
 *
 * `matching --cie <CIE_xyz_1931_2deg.csv> [--out <json>]` reduces the CIE 1931 2° standard
 * observer's colour-matching functions, 360–830 nm at 1 nm (CIE 018:2019 / ISO/CIE 11664-1:2019,
 * DOI 10.25039/CIE.DS.xvudnb9b, from https://files.cie.co.at/Publications-datasets/, CC BY-SA 4.0;
 * SHA-256 {@link CIE_CSV_SHA256}) to linear Rec. 709 r̄, ḡ and b̄ by {@link XYZ_TO_REC709}, the
 * matrix `view/photometry/toneCurve.ts` carries as `XYZ_TO_SRGB`. The CSV is the one R06 commits
 * unmodified in `crates/hyperion-fit/data/cie_cmf/` as a tool input (its `PROVENANCE.toml`), so no
 * fetch is needed; any copy with the same checksum serves. The output is
 * `src/renderer/src/view/atmosphere/colourMatching.json` unless `--out` names another file: the
 * file is adapted material of the CIE's table, offered under CC BY-SA 4.0 (`NOTICE`).
 *
 * `cross-sections --ozone <serdyuchenkogorshelev5digits.dat> --methane <ch4.txt> --so2 <file>
 * (four times) --o2o2 <O2-O2_2024.cia> --h2he <H2-He_2011.cia> --o2n2 <O2-N2_2024.cia> --o2co2
 * <O2-CO2_2024.cia> --h2h2 <file> (three times) [--out <json>]` reduces the absorbers'
 * cross-sections (plan R08, R08.T4.b and R08.T4.c) to
 * `src/renderer/src/view/atmosphere/absorbers/crossSections.json`, over 380–800 nm on vacuum
 * wavelengths, in m² a molecule, and the pairs' in m⁵ a molecule pair:
 *
 * - ozone from Serdyuchenko et al. 2014's table at 11 temperatures ({@link OZONE_SOURCE}),
 *   binned to 1 nm, each bin the mean over [λ − 0.5, λ + 0.5) nm (`decisions-r05.md` item 2);
 * - methane from NASA PSG's conversion of Karkoschka and Tomasko 2010 at 100, 198 and 296 K
 *   ({@link METHANE_SOURCE}), interpolated linearly onto a 0.25 nm grid, so that no band-model
 *   value is averaged (`decision-r08-licences.md` row 1);
 * - sulphur dioxide's near-ultraviolet tail from Hermans, Vandaele and Fally 2009 at 298–358 K,
 *   as the MPI-Mainz atlas's CC BY 4.0 snapshot holds it ({@link SULPHUR_DIOXIDE_SOURCE}), binned
 *   to 1 nm;
 * - the collision-induced absorption of O₂–O₂, O₂–N₂, O₂–CO₂ and H₂–He from HITRAN's files
 *   ({@link OXYGEN_PAIR_SOURCE}, {@link OXYGEN_NITROGEN_PAIR_SOURCE},
 *   {@link OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE}, {@link HYDROGEN_HELIUM_PAIR_SOURCE}), binned to
 *   1 nm;
 * - H₂–H₂'s from Borysow's tables ({@link HYDROGEN_PAIR_SOURCE}), binned to 1 nm.
 *
 * No raw file is committed: the ozone page states no terms (`decisions-r05.md` item 4), PSG's
 * file states none (`decision-r08-licences.md`), HITRAN states none beyond its citation policy,
 * Borysow's page reserves its rights but offers the tables for use with citation
 * (`decision-r08-cmf-licence.md` §8), and the atlas's snapshot is CC BY 4.0, whose attribution
 * `NOTICE` gives.
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

import { runAtomicBounds, runAtomicShares } from "./atomicLinesCli";
import {
  runExomolBands,
  runExomolBins,
  runExomolCellReferences,
  runExomolKDistributions,
  runExomolKDistributionsMerge,
} from "./exomolCli";
import { runHclOvertones } from "./hclOvertones";
import { broadeningRatios } from "./kDistributions";
import { LINE_SOURCES, runKDistributions, runKDistributionsMerge } from "./kDistributionsCli";
import { runBackgroundError, runCellReferences, runEarthReference } from "./lineReferences";
import { parseHitranLines } from "./molecularLines";
import { LOWTRAN_SOURCE, parseLowtranContinuum, reduceWaterContinuum } from "./waterContinuum";
import { CIE_CSV_SHA256, type MatchingSample, parseCie, XYZ_TO_REC709 } from "./solarFactors";

/** The matching functions' file, relative to `apps/hyperion`. */
export const MATCHING_OUTPUT = "src/renderer/src/view/atmosphere/colourMatching.json";

/** The CIE's required citation for the 1931 2° functions (CIE's dataset page; `NOTICE`). */
export const CIE_CITATION =
  "CIE 2019, Colour-matching functions of CIE 1931 standard colorimetric observer, International " +
  "Commission on Illumination (CIE), Vienna, AT, DOI: 10.25039/CIE.DS.xvudnb9b";

/** What `colourMatching.json` holds. */
export interface ReducedMatching {
  readonly description: string;
  readonly source: string;
  readonly sha256: string;
  readonly licence: string;
  /** The matrix that took x̄, ȳ, z̄ to r̄, ḡ, b̄, as rows. */
  readonly xyzToRgb: readonly (readonly [number, number, number])[];
  /** The first row's wavelength, nm. */
  readonly firstNm: number;
  /** The rows' spacing, nm. */
  readonly stepNm: number;
  /** Each row: the wavelength, nm, and r̄, ḡ, b̄. */
  readonly rows: readonly (readonly [number, number, number, number])[];
}

/**
 * The matching functions as linear Rec. 709, row by row.
 *
 * @throws Error if the rows are not at a constant step of 1 nm.
 */
export function reduceMatching(cmf: readonly MatchingSample[]): ReducedMatching {
  const first = cmf[0];
  if (first === undefined) {
    throw new Error("the matching functions have no rows");
  }
  const rows = cmf.map((row, i) => {
    if (row.wavelengthNm !== first.wavelengthNm + i) {
      throw new Error(`row ${i + 1} is at ${row.wavelengthNm} nm, not ${first.wavelengthNm + i}`);
    }
    const [r, g, b] = XYZ_TO_REC709.map((m) => m[0] * row.x + m[1] * row.y + m[2] * row.z);
    return [row.wavelengthNm, r ?? Number.NaN, g ?? Number.NaN, b ?? Number.NaN] as const;
  });
  return {
    description:
      "The CIE 1931 2-degree colour-matching functions as linear Rec. 709 r, g and b at 1 nm, " +
      "each row [nm, r, g, b], written by apps/hyperion/scripts/atmosphereData.mjs matching " +
      "(rendering plan R08, R08.T4.a). Adapted from the CIE's table: x_bar, y_bar and z_bar " +
      "taken to r, g and b by xyzToRgb; its inverse recovers the table.",
    source: `${CIE_CITATION}; CIE_xyz_1931_2deg.csv from https://files.cie.co.at/Publications-datasets/`,
    sha256: CIE_CSV_SHA256,
    licence:
      "CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/). Adapted material of the " +
      "CIE's data set " +
      "(https://cie.co.at/datatable/cie-1931-colour-matching-functions-2-degree-observer), " +
      "offered under CC BY-SA 4.0, and provided as is, without warranties (CC BY-SA 4.0, " +
      "Section 5).",
    xyzToRgb: XYZ_TO_REC709,
    firstNm: first.wavelengthNm,
    stepNm: 1,
    rows,
  };
}

/** A row of numbers as JSON on one line, each in its shortest round-trip form. */
function line(values: readonly number[]): string {
  return `[${values.join(", ")}]`;
}

/** The file's text: JSON, one row a line, every number in its shortest round-trip form. */
export function matchingText(reduced: ReducedMatching): string {
  const fields = [
    `  "description": ${JSON.stringify(reduced.description)}`,
    `  "source": ${JSON.stringify(reduced.source)}`,
    `  "sha256": ${JSON.stringify(reduced.sha256)}`,
    `  "licence": ${JSON.stringify(reduced.licence)}`,
    `  "xyzToRgb": [\n${reduced.xyzToRgb.map((r) => `    ${line(r)}`).join(",\n")}\n  ]`,
    `  "firstNm": ${reduced.firstNm}`,
    `  "stepNm": ${reduced.stepNm}`,
    `  "rows": [\n${reduced.rows.map((r) => `    ${line(r)}`).join(",\n")}\n  ]`,
  ];
  return `{\n${fields.join(",\n")}\n}\n`;
}

/**
 * A file's text, if its SHA-256 is the one given.
 *
 * @throws Error naming both digests otherwise.
 */
export function readChecked(path: string, sha256: string): string {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${sha256}`);
  }
  return bytes.toString("utf8");
}

/** The absorbers' cross-sections' file, relative to `apps/hyperion`. */
export const CROSS_SECTIONS_OUTPUT =
  "src/renderer/src/view/atmosphere/absorbers/crossSections.json";

/** O₂'s collision-induced pairs' file (R08.T4.d), relative to `apps/hyperion`. */
export const OXYGEN_PAIRS_OUTPUT = "src/renderer/src/view/atmosphere/absorbers/oxygenPairs.json";

/** Where a fetched data file came from, and how it is checked. */
export interface FetchedSource {
  /** The data's citation. */
  readonly citation: string;
  /** The file's URL. */
  readonly url: string;
  /** The file's SHA-256, or each file's after its name for a source of several. */
  readonly sha256: string;
  /** The date the file with this checksum was fetched. */
  readonly fetched: string;
  /** What the source says about its terms, and so what may be committed. */
  readonly terms: string;
}

/**
 * Serdyuchenko et al. 2014's ozone cross-sections, the IUP Bremen file of 5 significant figures,
 * on vacuum wavelengths every 0.01 nm over 213.33–1,100 nm at 193–293 K in steps of 10 K
 * (`DATE OF FILE CREATION: 05.03.2012`; served with `Last-Modified` 2023-02-19).
 */
export const OZONE_SOURCE: FetchedSource = {
  citation:
    "A. Serdyuchenko, V. Gorshelev, M. Weber, W. Chehade and J. P. Burrows, High spectral " +
    "resolution ozone absorption cross-sections - Part 2: Temperature dependence, Atmos. Meas. " +
    "Tech. 7 (2014) 625-636, DOI 10.5194/amt-7-625-2014; V. Gorshelev, A. Serdyuchenko, " +
    "M. Weber, W. Chehade and J. P. Burrows, High spectral resolution ozone absorption " +
    "cross-sections - Part 1: Measurements, data analysis and comparison with previous " +
    "measurements around 293 K, Atmos. Meas. Tech. 7 (2014) 609-624, DOI 10.5194/amt-7-609-2014 " +
    "(IUP Bremen, MolSpec Lab)",
  url: "https://www.iup.uni-bremen.de/gruppen/molspec/downloads/serdyuchenkogorshelev5digits.dat",
  sha256: "4dfbf021b746512c192df5f0d43c54cee6ea3b4365bb490bcf6ed347f0ce7092",
  fetched: "2026-10-10",
  terms:
    "The data page states no terms: reduced values with their citation, the raw table not " +
    "committed (decisions-r05.md item 4)",
};

/**
 * NASA's Planetary Spectrum Generator's conversion of Karkoschka and Tomasko 2010's methane
 * coefficients to cross-sections, cm² a molecule at 100, 198 and 296 K, 4,949 rows to 0.836 µm.
 *
 * @remarks
 * PSG's rows are σ = k ÷ 2.686 78 × 10²⁴ cm⁻² (Loschmidt's number times 1 km) of Karkoschka and
 * Tomasko's k in km⁻¹ amagat⁻¹, which their Table 4 gives to three significant figures, the least
 * step 10⁻⁴ km⁻¹ amagat⁻¹ (3.721 93 × 10⁻²⁹ cm²). Its wavelengths are their table's air wavelengths to 0.01 nm, at 5 cm⁻¹
 * below 19,300 cm⁻¹ and 25 cm⁻¹ above, half their stated spectral widths of 10 and 50 cm⁻¹
 * ({@link methaneWavenumberPerCm} recovers each wavenumber). Its ultraviolet completion from the
 * MPI-Mainz atlas (Keller-Rudek et al. 2013) ends at 152 nm, so no value of it is used here.
 */
export const METHANE_SOURCE: FetchedSource = {
  citation:
    "E. Karkoschka and M. G. Tomasko, Methane absorption coefficients for the jovian planets " +
    "from laboratory, Huygens, and HST data, Icarus 205 (2010) 674-694, DOI " +
    "10.1016/j.icarus.2009.07.044, as converted to cross-sections by NASA's Planetary Spectrum " +
    "Generator (G. L. Villanueva et al., J. Quant. Spectrosc. Radiat. Transfer 217 (2018) 86-104; " +
    "https://psg.gsfc.nasa.gov)",
  url: "https://psg.gsfc.nasa.gov/data/linelists/xuv/data/ch4.txt",
  sha256: "cf7f7195a9ceadad1480b436d1657b722d1bb6264a603618faead3dd8aa4a3ef",
  fetched: "2026-10-09 and 2026-10-10",
  terms:
    "The file states no terms: reduced values with their citation, the raw file not committed " +
    "(decision-r08-licences.md row 1)",
};

/** The span the cross-sections are reduced over, nm (vacuum): 380 to 800. */
export const CROSS_SECTION_RANGE_NM: readonly [number, number] = [380, 800];

/** Ozone's bins' spacing, nm: 1, each bin centred ({@link binOzone}). */
export const OZONE_STEP_NM = 1;

/** Methane's grid spacing, nm: 0.25 (`decision-r08-licences.md` row 1). */
export const METHANE_STEP_NM = 0.25;

/** The significant figures the reduced cross-sections are written to: 6. */
export const CROSS_SECTION_FIGURES = 6;

/** One cm² in m². */
const M2_PER_CM2 = 1e-4;

/** A table of cross-sections at several temperatures on a uniform grid, as written. */
export interface ReducedCrossSections {
  /** The registry key (decision-composition §1.9). */
  readonly species: string;
  readonly source: FetchedSource;
  /** How the source's values were taken onto the grid. */
  readonly reduction: string;
  /** The first row's vacuum wavelength, nm. */
  readonly firstNm: number;
  /** The rows' spacing, nm. */
  readonly stepNm: number;
  /** The temperatures of the columns after the wavelength, K, ascending. */
  readonly temperaturesK: readonly number[];
  /**
   * How σ is taken between the temperatures: `linear` in T, or `lnQuadratic`, ln σ the quadratic
   * through three temperatures (the renderer's `absorbers.ts` reads it).
   */
  readonly temperatureLaw: "linear" | "lnQuadratic";
  /** The temperatures over which the law gives a measured σ, K. */
  readonly measuredRangeK: readonly [number, number];
  /** Each row: the vacuum wavelength, nm, then σ at each temperature, m² a molecule. */
  readonly rows: readonly (readonly number[])[];
}

/** The source's rows: a wavelength in nm and a value per temperature column. */
export interface SourceTable {
  /** The columns' temperatures, K, in the file's order. */
  readonly temperaturesK: readonly number[];
  /** Each row: the wavelength (in the file's own unit and convention), then the columns. */
  readonly rows: readonly (readonly number[])[];
}

/** A number written to {@link CROSS_SECTION_FIGURES} significant figures, as a number. */
function rounded(value: number): number {
  return value === 0 ? 0 : Number(value.toPrecision(CROSS_SECTION_FIGURES));
}

/**
 * The ozone file's rows and the temperature of each column, from its `COLUMN n: … @293K` lines.
 *
 * @throws Error if the header names no temperatures, or a data row has the wrong column count.
 */
export function parseOzone(text: string): SourceTable {
  const temperaturesK: number[] = [];
  const rows: number[][] = [];
  for (const entry of text.split(/\r?\n/u)) {
    const column = /^\s*COLUMN\s+(\d+):.*@\s*(\d+)\s*K/u.exec(entry);
    if (column !== null) {
      temperaturesK[Number(column[1]) - 2] = Number(column[2]);
      continue;
    }
    const fields = entry.trim().split(/\s+/u);
    if (fields.length < 2 || !fields.every((f) => /^[-+0-9.eE]+$/u.test(f))) {
      continue;
    }
    rows.push(fields.map(Number));
  }
  const width = temperaturesK.length + 1;
  if (temperaturesK.length === 0 || temperaturesK.some((t) => !Number.isFinite(t))) {
    throw new Error("the ozone file's header names no temperature columns");
  }
  for (const [i, row] of rows.entries()) {
    if (row.length !== width || !row.every(Number.isFinite)) {
      throw new Error(`ozone data row ${i + 1} has ${row.length} values, not ${width}`);
    }
  }
  return { temperaturesK, rows };
}

/**
 * Ozone's 1 nm bins: at each whole nm λ of {@link CROSS_SECTION_RANGE_NM}, the mean of the
 * source's rows on [λ − 0.5, λ + 0.5) nm at each temperature, the columns put in ascending order
 * of temperature, in m².
 *
 * @remarks
 * The rows are matched on whole hundredths of a nm, the source's grid, so that a bin holds exactly
 * its 100 rows. Near 380 nm, where the source's optical density is at its limit (the IUP page's
 * 340–450 nm limits), its noise leaves a bin's mean below zero at one temperature (382 nm, 193 K);
 * such a mean is written as 0, since a negative σ would make a transmittance above 1. The
 * returned `negatives` counts them.
 *
 * @throws Error if a bin's rows are missing from the source.
 */
export function binOzone(table: SourceTable): {
  readonly reduced: ReducedCrossSections;
  readonly negatives: number;
} {
  const byHundredth = new Map<number, readonly number[]>();
  for (const row of table.rows) {
    byHundredth.set(Math.round((row[0] ?? Number.NaN) * 100), row);
  }
  const order = table.temperaturesK
    .map((temperatureK, column) => ({ temperatureK, column }))
    .toSorted((a, b) => a.temperatureK - b.temperatureK);
  const [low, high] = CROSS_SECTION_RANGE_NM;
  let negatives = 0;
  const rows: number[][] = [];
  for (let nm = low; nm <= high; nm += OZONE_STEP_NM) {
    const sums = order.map(() => 0);
    for (let h = nm * 100 - 50; h < nm * 100 + 50; h += 1) {
      const row = byHundredth.get(h);
      if (row === undefined) {
        throw new Error(`the ozone file has no row at ${h / 100} nm`);
      }
      for (const [k, { column }] of order.entries()) {
        sums[k] = (sums[k] ?? 0) + (row[column + 1] ?? Number.NaN);
      }
    }
    const means = sums.map((s) => {
      const mean = (s / 100) * M2_PER_CM2;
      if (mean < 0) {
        negatives += 1;
        return 0;
      }
      return rounded(mean);
    });
    rows.push([nm, ...means]);
  }
  return {
    reduced: {
      species: "O3",
      source: OZONE_SOURCE,
      reduction:
        "Each row is the mean of the source's 0.01 nm rows (vacuum wavelengths) over " +
        "[nm - 0.5, nm + 0.5), converted from cm2 to m2 and written to 6 significant figures. A " +
        "mean below zero, the source's noise near 380 nm (one, at 382 nm and 193 K), is written as 0.",
      firstNm: low,
      stepNm: OZONE_STEP_NM,
      temperaturesK: order.map((o) => o.temperatureK),
      temperatureLaw: "linear",
      measuredRangeK: [
        order[0]?.temperatureK ?? Number.NaN,
        order.at(-1)?.temperatureK ?? Number.NaN,
      ],
      rows,
    },
    negatives,
  };
}

/**
 * PSG's methane file's rows (air wavelength in µm, σ in cm² at each temperature) and its
 * temperatures, from its `#TEMP:` line.
 *
 * @throws Error unless the file declares cross-sections (`#TYPE:3`) and its temperatures, and
 *   every data row has a value for each.
 */
export function parseMethane(text: string): SourceTable {
  let temperaturesK: number[] | undefined;
  let crossSections = false;
  const rows: number[][] = [];
  for (const entry of text.split(/\r?\n/u)) {
    if (entry.startsWith("#")) {
      const temp = /^#TEMP:\s*([^!]*)/u.exec(entry);
      if (temp !== null) {
        temperaturesK = (temp[1] ?? "").trim().split(/\s+/u).map(Number);
      }
      if (/^#TYPE:\s*3\b/u.test(entry)) {
        crossSections = true;
      }
      continue;
    }
    if (entry.trim().length > 0) {
      rows.push(entry.trim().split(/\s+/u).map(Number));
    }
  }
  if (!crossSections || temperaturesK === undefined || !temperaturesK.every(Number.isFinite)) {
    throw new Error("the methane file does not declare cross-sections (#TYPE:3) and temperatures");
  }
  const width = temperaturesK.length + 1;
  for (const [i, row] of rows.entries()) {
    if (row.length !== width || !row.every(Number.isFinite)) {
      throw new Error(`methane data row ${i + 1} has ${row.length} values, not ${width}`);
    }
  }
  return { temperaturesK, rows };
}

/**
 * The refractive index of standard air at a vacuum-or-air wavelength in µm: Edlén 1966's
 * dispersion formula, (n − 1) × 10⁸ = 8,342.13 + 2,406,030 ÷ (130 − σ²) + 15,997 ÷ (38.9 − σ²),
 * σ in µm⁻¹ (B. Edlén, "The refractive index of air", Metrologia 2 (1966) 71–80;
 * σ the vacuum wavenumber, which an air wavelength stands in for to about 4 × 10⁻⁹ in n).
 *
 * @remarks
 * It only identifies each row's wavenumber ({@link methaneWavenumberPerCm}), which a tolerance of
 * 1 cm⁻¹ checks, so the choice among standard-air formulas, which differ by about 10⁻⁸ in n, does
 * not matter.
 */
export function standardAirIndex(wavelengthUm: number): number {
  const s2 = 1 / (wavelengthUm * wavelengthUm);
  return 1 + 1e-8 * (8_342.13 + 2_406_030 / (130 - s2) + 15_997 / (38.9 - s2));
}

/** Karkoschka and Tomasko's sampling below and above 19,300 cm⁻¹: 5 and 25 cm⁻¹. */
const METHANE_SAMPLING: { readonly belowPerCm: number; readonly abovePerCm: number } = {
  belowPerCm: 5,
  abovePerCm: 25,
};

/** The wavenumber at which the sampling changes, cm⁻¹ (their Table 4's caption). */
const METHANE_SAMPLING_BREAK_PER_CM = 19_300;

/** How far a row's computed wavenumber may lie from the sampling's, cm⁻¹. */
const WAVENUMBER_TOLERANCE_PER_CM = 1;

/**
 * The wavenumber of a PSG methane row, cm⁻¹: its air wavelength taken to vacuum by
 * {@link standardAirIndex} and rounded to Karkoschka and Tomasko's sampling, 5 cm⁻¹ below
 * 19,300 cm⁻¹ and 25 cm⁻¹ above.
 *
 * @throws Error if the computed wavenumber lies more than 1 cm⁻¹ from the sampling, that is if the
 *   row is not on their grid in air.
 */
export function methaneWavenumberPerCm(airWavelengthUm: number): number {
  const computed = 1e4 / (airWavelengthUm * standardAirIndex(airWavelengthUm));
  const step =
    computed < METHANE_SAMPLING_BREAK_PER_CM
      ? METHANE_SAMPLING.belowPerCm
      : METHANE_SAMPLING.abovePerCm;
  const onGrid = Math.round(computed / step) * step;
  if (Math.abs(computed - onGrid) > WAVENUMBER_TOLERANCE_PER_CM) {
    throw new Error(
      `methane row at ${airWavelengthUm} µm (air) is ${computed} cm⁻¹, not on the ${step} cm⁻¹ grid`,
    );
  }
  return onGrid;
}

/**
 * The temperatures over which Karkoschka and Tomasko's Eq. 8 gives a measured σ, K: 50–300, the
 * span of the k-tables (Irwin's) their Table 4's caption names (`decision-r08-t4b-t12c.md` §1.6).
 */
export const METHANE_MEASURED_RANGE_K: readonly [number, number] = [50, 300];

/** The air wavelength below which PSG's file holds the MPI-Mainz ultraviolet completion, µm. */
const METHANE_ULTRAVIOLET_BELOW_UM = 0.3;

/**
 * Methane's σ on the 0.25 nm grid of {@link CROSS_SECTION_RANGE_NM}, vacuum wavelengths, in m², at
 * each of the file's temperatures.
 *
 * @remarks
 * Karkoschka and Tomasko's rows (PSG's rows from 0.3 µm up; below lies only the MPI-Mainz
 * completion, which ends at 152 nm) are put on vacuum wavelengths 10⁷ ÷ ν nm, ν each row's
 * wavenumber ({@link methaneWavenumberPerCm}), and interpolated linearly in wavelength. The grid
 * is finer than their stated spectral width everywhere (10 cm⁻¹ is 0.27 nm at 518 nm and 0.64 nm
 * at 800 nm; 50 cm⁻¹ is 0.80–1.34 nm over their 400–518 nm), so no value is averaged with another. Below
 * their first row, 25,000 cm⁻¹ (400 nm), σ is 0: their table starts there, its first value above
 * 0 is at 405.27 nm, and Karkoschka 1998's coefficients (PDS GBAT_0001) are 0 to their 10⁻⁴
 * km⁻¹ amagat⁻¹ from 300 to 400.8 nm. Between the temperatures, ln σ is the quadratic through
 * the three (`lnQuadratic`), their Eq. 8 as Collins et al. 2018 (Sci. Adv. 4, eaas9593, Methods
 * Eqs. 4–5) restate it, measured over 50–300 K, the span of the k-tables their Table 4's caption
 * names (`decision-r08-t4b-t12c.md` §1.6).
 *
 * @throws Error if the temperatures or the rows' wavelengths do not ascend, or the rows do not
 *   reach the range's top.
 */
export function resampleMethane(table: SourceTable): ReducedCrossSections {
  if (table.temperaturesK.some((t, i) => !(t > (table.temperaturesK[i - 1] ?? 0)))) {
    throw new Error(
      `the methane file's temperatures do not ascend: ${table.temperaturesK.join(", ")} K`,
    );
  }
  const points = table.rows
    .filter((row) => (row[0] ?? 0) >= METHANE_ULTRAVIOLET_BELOW_UM)
    .map((row) => ({
      nm: 1e7 / methaneWavenumberPerCm(row[0] ?? Number.NaN),
      values: row.slice(1).map((cm2) => cm2 * M2_PER_CM2),
    }));
  for (let i = 1; i < points.length; i += 1) {
    if (!((points[i]?.nm ?? Number.NaN) > (points[i - 1]?.nm ?? Number.NaN))) {
      throw new Error(`methane rows ${i} and ${i + 1} do not ascend in wavelength`);
    }
  }
  const [low, high] = CROSS_SECTION_RANGE_NM;
  const last = points.at(-1);
  if (last === undefined || last.nm < high) {
    throw new Error(`the methane rows end at ${last?.nm ?? Number.NaN} nm, below ${high} nm`);
  }
  const zeros = table.temperaturesK.map(() => 0);
  const rows: number[][] = [];
  const count = Math.round((high - low) / METHANE_STEP_NM);
  let j = 0;
  for (let k = 0; k <= count; k += 1) {
    const nm = low + k * METHANE_STEP_NM;
    while ((points[j + 1]?.nm ?? Number.POSITIVE_INFINITY) <= nm) {
      j += 1;
    }
    const a = points[j];
    const b = points[j + 1];
    if (a === undefined || nm < a.nm) {
      rows.push([nm, ...zeros]);
      continue;
    }
    const t = b === undefined ? 0 : (nm - a.nm) / (b.nm - a.nm);
    rows.push([nm, ...a.values.map((v, c) => rounded(v + ((b?.values[c] ?? v) - v) * t))]);
  }
  return {
    species: "CH4",
    source: METHANE_SOURCE,
    reduction:
      "Karkoschka and Tomasko's rows, on air wavelengths in PSG's file, are put on vacuum " +
      "wavelengths 1e7/nu nm, nu each row's wavenumber on their 5 and 25 cm-1 sampling (Edlen " +
      "1966's standard air), and interpolated linearly in wavelength onto the 0.25 nm grid, " +
      "finer than their 10 and 50 cm-1 spectral widths, so that no value is averaged; converted " +
      "from cm2 to m2 and written to 6 significant figures. Below their first row, 25000 cm-1 " +
      "(400 nm), sigma is 0: their table starts there, and Karkoschka 1998's coefficients (PDS " +
      "GBAT_0001, DOI 10.17189/2bp8-k793) are 0 from 300 to 400.8 nm. The infinite-pressure " +
      "coefficients, without their finite-pressure correction (their Eqs. 2-4). Between the " +
      "temperatures ln sigma is the quadratic through the three, their Eq. 8 (as Collins et al. " +
      "2018, Sci. Adv. 4, eaas9593, restate it), measured over 50-300 K, the span of the " +
      "k-tables their Table 4's caption names.",
    firstNm: low,
    stepNm: METHANE_STEP_NM,
    temperaturesK: table.temperaturesK,
    temperatureLaw: "lnQuadratic",
    measuredRangeK: METHANE_MEASURED_RANGE_K,
    rows,
  };
}

/**
 * The MPI-Mainz UV/VIS Spectral Atlas's frozen snapshot of 2013-06-24 (H. Keller-Rudek,
 * G. K. Moortgat, R. Sander and R. Sörensen, Earth Syst. Sci. Data 5 (2013) 365–373), the Zenodo
 * archive whose four `SO2_VandaeleHermansFally(2009)_*` files ({@link SULPHUR_DIOXIDE_MEMBERS})
 * hold sulphur dioxide's cross-sections at 298, 318, 338 and 358 K, 227.275–416.658 nm.
 *
 * @remarks
 * The atlas's notes: Fourier-transform measurements at 2 cm⁻¹ resolution, 23,500–44,500 cm⁻¹,
 * BIRA-IASB's files at 0.5 cm⁻¹ intervals. Its wavelengths are vacuum wavelengths 10⁷ ÷ ν: the
 * first row, 227.275 nm, is 43,999.5 cm⁻¹, and the last, 416.658 nm, 24,000.5 cm⁻¹. The
 * archive's SHA-256 is this record's; Zenodo gives its MD5 as
 * `617a822cef2117f4390cce8b407180bf`, which it matched.
 */
export const SULPHUR_DIOXIDE_SOURCE: FetchedSource = {
  citation:
    "C. Hermans, A. C. Vandaele and S. Fally, Fourier transform measurements of SO2 absorption " +
    "cross sections: I. Temperature dependence in the 24000-29000 cm-1 (345-420 nm) region, " +
    "J. Quant. Spectrosc. Radiat. Transfer 110 (2009) 756-765, DOI 10.1016/j.jqsrt.2009.01.031; " +
    "A. C. Vandaele, C. Hermans and S. Fally, Fourier transform measurements of SO2 absorption " +
    "cross sections: II. Temperature dependence in the 29000-44000 cm-1 (227-345 nm) region, " +
    "J. Quant. Spectrosc. Radiat. Transfer 110 (2009) 2115-2126, DOI " +
    "10.1016/j.jqsrt.2009.05.006; as held in the MPI-Mainz UV/VIS Spectral Atlas's snapshot of " +
    "2013-06-24 (H. Keller-Rudek, G. K. Moortgat, R. Sander and R. Sorensen, Earth Syst. Sci. " +
    "Data 5 (2013) 365-373, DOI 10.5194/essd-5-365-2013; Zenodo, DOI 10.5281/zenodo.6951)",
  url: "https://zenodo.org/api/records/6951/files/uv-vis-spectral-atlas-2013-06-24.zip/content",
  sha256: "bea06f6f390b774b5f0aeffda1b912a6446ba287578d66486414c728c148e6c6",
  fetched: "2026-10-10",
  terms:
    "The snapshot is licensed CC BY 4.0 (Zenodo record 6951, cc-by-4.0): reduced values with " +
    "attribution in NOTICE, the files not committed (decision-composition section 5)",
};

/** One file of a fetched archive, by its name, its own SHA-256 and the temperature it holds. */
export interface ArchiveMember {
  readonly name: string;
  readonly sha256: string;
  readonly temperatureK: number;
}

/** The snapshot's four sulphur dioxide files ({@link SULPHUR_DIOXIDE_SOURCE}), coldest first. */
export const SULPHUR_DIOXIDE_MEMBERS: ReadonlyArray<ArchiveMember> = [
  {
    name: "SO2_VandaeleHermansFally(2009)_298K_227.275-416.658nm.txt",
    sha256: "71b35094443e65ff8814d36e9b37418ee5df1941bca6697cb31fb65625478272",
    temperatureK: 298,
  },
  {
    name: "SO2_VandaeleHermansFally(2009)_318K_227.275-416.658nm.txt",
    sha256: "e1f68d3362034d1bb6dcbd12a6c06738e2e538c914ec29fcfe8cadb8e7c1b7e0",
    temperatureK: 318,
  },
  {
    name: "SO2_VandaeleHermansFally(2009)_338K_227.275-416.658nm.txt",
    sha256: "13c88cacfa6565de1af000e6c3fa1d51ef5d0180dcc1f9da86700fdae4d94c68",
    temperatureK: 338,
  },
  {
    name: "SO2_VandaeleHermansFally(2009)_358K_227.275-416.658nm.txt",
    sha256: "8a12209e848d525eca7c22ce7eeacabe915e6ecd5b6ec01408a4ea66d98ac59d",
    temperatureK: 358,
  },
];

/** Every bin of the reduced tables is 1 nm wide, centred on its whole nm. */
export const CONTINUUM_STEP_NM = 1;

/** A two-column source file's rows: a vacuum wavelength, nm, and σ, cm² a molecule. */
export type SpectrumRows = ReadonlyArray<readonly [number, number]>;

/**
 * A two-column source's rows (wavelength and σ), its comment and blank lines skipped.
 *
 * @throws Error naming the file for a row that is not two finite numbers, or rows that do not
 *   ascend in wavelength.
 */
export function parseTwoColumns(text: string, what: string): SpectrumRows {
  const rows: (readonly [number, number])[] = [];
  for (const [i, entry] of text.split(/\r?\n/u).entries()) {
    const trimmed = entry.trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }
    const fields = trimmed.split(/\s+/u).map(Number);
    const [nm, sigma] = fields;
    if (fields.length !== 2 || nm === undefined || sigma === undefined) {
      throw new Error(`${what} line ${i + 1} is not a wavelength and a cross-section`);
    }
    if (!(Number.isFinite(nm) && Number.isFinite(sigma))) {
      throw new Error(`${what} line ${i + 1} holds a value that is not finite`);
    }
    const previous = rows.at(-1);
    if (previous !== undefined && !(nm > previous[0])) {
      throw new Error(`${what} line ${i + 1} does not ascend in wavelength`);
    }
    rows.push([nm, sigma]);
  }
  return rows;
}

/**
 * Sulphur dioxide's 1 nm bins over {@link CROSS_SECTION_RANGE_NM}, in m², at its four
 * temperatures: each whole nm λ the mean of the source's rows on [λ − 0.5, λ + 0.5), where the
 * source covers the whole bin, and 0 above it.
 *
 * @remarks
 * The rows are uniform in wavenumber (0.5 cm⁻¹, 0.006–0.009 nm over 380–417 nm), so a bin's plain
 * mean is its mean over wavenumber, within 0.3% of its mean over wavelength. The source ends at
 * 416.658 nm (24,000.5 cm⁻¹), so the bins from 417 nm up are 0: the band's tail falls below
 * 10⁻²⁵ cm² from about 410 nm, and its last means are at the measurement's noise. A mean below
 * zero, that noise, is written as 0, since a negative σ would make a transmittance above 1; the
 * returned `negatives` counts them.
 *
 * @throws Error unless one table is given for each of {@link SULPHUR_DIOXIDE_MEMBERS}'
 *   temperatures, each starting below the range's first bin.
 */
export function binSulphurDioxide(
  tables: ReadonlyArray<{ readonly temperatureK: number; readonly rows: SpectrumRows }>,
): { readonly reduced: ReducedCrossSections; readonly negatives: number } {
  const order = SULPHUR_DIOXIDE_MEMBERS.map((member) => {
    const table = tables.find((t) => t.temperatureK === member.temperatureK);
    if (table === undefined) {
      throw new Error(`no sulphur dioxide table at ${member.temperatureK} K`);
    }
    return table;
  });
  const [low, high] = CROSS_SECTION_RANGE_NM;
  for (const table of order) {
    if (!((table.rows[0]?.[0] ?? Number.POSITIVE_INFINITY) <= low - CONTINUUM_STEP_NM / 2)) {
      throw new Error(`the ${table.temperatureK} K sulphur dioxide rows start above ${low} nm`);
    }
  }
  let negatives = 0;
  const rows: number[][] = [];
  for (let nm = low; nm <= high; nm += CONTINUUM_STEP_NM) {
    const means = order.map((table) => {
      const mean = binMean(table.rows, nm);
      if (mean === undefined) {
        return 0;
      }
      if (mean < 0) {
        negatives += 1;
        return 0;
      }
      return rounded(mean * M2_PER_CM2);
    });
    rows.push([nm, ...means]);
  }
  return {
    reduced: {
      species: "SO2",
      source: SULPHUR_DIOXIDE_SOURCE,
      reduction:
        "The snapshot's four files (" +
        SULPHUR_DIOXIDE_MEMBERS.map((m) => `${m.name}, SHA-256 ${m.sha256}`).join("; ") +
        "), vacuum wavelengths 1e7/nu at 0.5 cm-1, each row the mean of the source's rows over " +
        "[nm - 0.5, nm + 0.5), converted from cm2 to m2 and written to 6 significant figures. " +
        "The source ends at 416.658 nm, so sigma is 0 from 417 nm up; a mean below zero, the " +
        "measurement's noise in the tail, is written as 0. Linear in T between 298 and 358 K and " +
        "measured there; held beyond.",
      firstNm: low,
      stepNm: CONTINUUM_STEP_NM,
      temperaturesK: order.map((t) => t.temperatureK),
      temperatureLaw: "linear",
      measuredRangeK: [
        order[0]?.temperatureK ?? Number.NaN,
        order.at(-1)?.temperatureK ?? Number.NaN,
      ],
      rows,
    },
    negatives,
  };
}

/**
 * The mean of a spectrum's values on [λ − 0.5, λ + 0.5) nm, or `undefined` unless its rows span
 * the whole bin.
 */
function binMean(rows: SpectrumRows, centreNm: number): number | undefined {
  const from = centreNm - CONTINUUM_STEP_NM / 2;
  const to = centreNm + CONTINUUM_STEP_NM / 2;
  const first = rows[0]?.[0] ?? Number.POSITIVE_INFINITY;
  const last = rows.at(-1)?.[0] ?? Number.NEGATIVE_INFINITY;
  if (!(first <= from && last >= to)) {
    return undefined;
  }
  let sum = 0;
  let count = 0;
  for (const [nm, value] of rows) {
    if (nm >= from && nm < to) {
      sum += value;
      count += 1;
    }
  }
  return count > 0 ? sum / count : undefined;
}

/**
 * What HITRAN states about its terms, read 2026-10-10: its FAQ (https://hitran.org/faq/, SHA-256
 * `4e43df9b…1611`), "The HITRAN database and its associated compilations are free", and its
 * citation policy (https://hitran.org/citepolicy/, `21a33a9d…a2f`), which requires the edition's
 * article and encourages the original sources'. Neither states a licence.
 */
const HITRAN_TERMS =
  "HITRAN states that its database and compilations are free (hitran.org/faq) and asks for its " +
  "edition's article and the original sources to be cited (hitran.org/citepolicy); it states no " +
  "licence: reduced values with their citations, the raw file not committed " +
  "(decision-composition section 5)";

/**
 * HITRAN2024's O₂–O₂ collision-induced absorption file, cm⁵ a molecule pair on vacuum wavenumbers
 * (Terragni et al. 2025; the section as Karman et al. 2019 and Richard et al. 2012 describe it).
 *
 * @remarks
 * Its sets in 380–800 nm, by the reference number each header ends with (the file's
 * `CIA_References_2024.pdf`):
 * - 54, Finkenzeller and Volkamer 2022: 20,000–33,670 cm⁻¹ (297–500 nm) at 223, 263 and 293 K;
 * - 32, Thalman and Volkamer 2013: 16,645–29,837 cm⁻¹ (335–601 nm) at 203, 233, 253, 273 and
 *   293 K, and 15,290–16,664 cm⁻¹ (600–654 nm, the 630 nm band) at 203, 233, 253 and 287 K;
 * - 30, Spiering et al. 2011: 14,206–14,898 cm⁻¹ (671–704 nm, the B band's) at 293 K;
 * - 4, Tran, Boulet and Hartmann 2006: 12,600–13,840 cm⁻¹ (723–794 nm, the A band's) at 296 K.
 */
export const OXYGEN_PAIR_SOURCE: FetchedSource = {
  citation:
    "HITRAN2024 collision-induced absorption, file O2-O2_2024.cia: J. Terragni et al., Collision " +
    "Induced Absorption in HITRAN2024: Enhanced and Improved Data for Atmospheric and Planetary " +
    "Studies, J. Quant. Spectrosc. Radiat. Transfer 347 (2025) 109631, DOI " +
    "10.1016/j.jqsrt.2025.109631; T. Karman et al., Update of the HITRAN collision-induced " +
    "absorption section, Icarus 328 (2019) 160-175, DOI 10.1016/j.icarus.2019.02.034; I. E. " +
    "Gordon et al., The HITRAN2024 molecular spectroscopic database, J. Quant. Spectrosc. Radiat. " +
    "Transfer 353 (2026) 109807, DOI 10.1016/j.jqsrt.2026.109807. Its visible sets: H. " +
    "Finkenzeller and R. Volkamer, J. Quant. Spectrosc. Radiat. Transfer 279 (2022) 108063, DOI " +
    "10.1016/j.jqsrt.2021.108063; R. Thalman and R. Volkamer, Phys. Chem. Chem. Phys. 15 (2013) " +
    "15371, DOI 10.1039/c3cp50968k; F. R. Spiering et al., Phys. Chem. Chem. Phys. 13 (2011) " +
    "9616-9621, DOI 10.1039/C1CP20403C; H. Tran, C. Boulet and J.-M. Hartmann, J. Geophys. Res. " +
    "111 (2006) D15210, DOI 10.1029/2005JD006869",
  url: "https://hitran.org/data/CIA/main/O2-O2_2024.cia",
  sha256: "0146acfe5c1414b8deaa56be9a63159154479d5804d1615855888a2e1efd4961",
  fetched: "2026-10-10",
  terms: HITRAN_TERMS,
};

/**
 * HITRAN's H₂–He collision-induced absorption file, Abel et al. 2012's ab initio coefficients,
 * cm⁵ a molecule pair, 20–20,000 cm⁻¹ at 200–9,900 K in 334 sets (reference 7).
 *
 * @remarks
 * It ends at 20,000 cm⁻¹, 500 nm, so it reaches the range over 500–760 nm only, the third and
 * fourth overtones' tail. HITRAN's H₂–H₂ file (Abel et al. 2011, 20–10,000 cm⁻¹ at 200–3,000 K)
 * and its alternates end at 10,000 cm⁻¹, 1 µm, so H₂–H₂ is taken from Borysow's tables instead
 * ({@link HYDROGEN_PAIR_SOURCE}).
 */
export const HYDROGEN_HELIUM_PAIR_SOURCE: FetchedSource = {
  citation:
    "M. Abel, L. Frommhold, X. Li and K. L. C. Hunt, Infrared absorption by collisional H2-He " +
    "complexes at temperatures up to 9000 K and frequencies from 0 to 20000 cm-1, J. Chem. Phys. " +
    "136 (2012) 044319, DOI 10.1063/1.3676405, as HITRAN's collision-induced absorption section " +
    "gives it (file H2-He_2011.cia; C. Richard et al., J. Quant. Spectrosc. Radiat. Transfer 113 " +
    "(2012) 1276, DOI 10.1016/j.jqsrt.2011.11.004; I. E. Gordon et al., The HITRAN2024 molecular " +
    "spectroscopic database, J. Quant. Spectrosc. Radiat. Transfer 353 (2026) 109807, DOI " +
    "10.1016/j.jqsrt.2026.109807)",
  url: "https://hitran.org/data/CIA/main/H2-He_2011.cia",
  sha256: "4f0eb9cd69a1c383f53a1431495bae0c01a30a41cc1b8433c2d726763ff45431",
  fetched: "2026-10-10",
  terms: HITRAN_TERMS,
};

/** One temperature's set of a HITRAN collision-induced absorption file. */
export interface CiaSet {
  /** The header's pair, as `O2-O2`. */
  readonly pair: string;
  /** The set's lower wavenumber limit, cm⁻¹, as its header gives it. */
  readonly minPerCm: number;
  /** The set's upper wavenumber limit, cm⁻¹, as its header gives it. */
  readonly maxPerCm: number;
  /** The set's temperature, K. */
  readonly temperatureK: number;
  /** The header's reference number, which the file's references table resolves. */
  readonly reference: string;
  /** Each point: the vacuum wavenumber, cm⁻¹, and the coefficient, cm⁵ a molecule pair. */
  readonly points: ReadonlyArray<readonly [number, number]>;
}

/**
 * A HITRAN collision-induced absorption file's sets (Richard et al. 2012, Fig. 1's header: the
 * pair in 20 characters, then the wavenumber limits in 10 each, the point count in 7, the
 * temperature in 7, the largest coefficient in 10, the resolution in 6, a comment in 21 and the
 * reference in 3), each followed by its points.
 *
 * @throws Error for a header whose count or temperature does not read, a point that is not two
 *   finite numbers, or a file that ends inside a set.
 */
export function parseCia(text: string): ReadonlyArray<CiaSet> {
  const lines = text.split(/\r?\n/u);
  const sets: CiaSet[] = [];
  let i = 0;
  while (i < lines.length) {
    const header = lines[i] ?? "";
    if (header.trim().length === 0) {
      i += 1;
      continue;
    }
    const count = Number(header.slice(40, 47));
    const temperatureK = Number(header.slice(47, 54));
    if (!(Number.isInteger(count) && count > 0 && temperatureK > 0)) {
      throw new Error(`CIA line ${i + 1} is not a set's header`);
    }
    const points: (readonly [number, number])[] = [];
    for (let k = 1; k <= count; k += 1) {
      const fields = (lines[i + k] ?? "").trim().split(/\s+/u).map(Number);
      const [nu, value] = fields;
      if (nu === undefined || value === undefined || !Number.isFinite(nu + value)) {
        throw new Error(`CIA line ${i + k + 1} is not a wavenumber and a coefficient`);
      }
      points.push([nu, value]);
    }
    sets.push({
      pair: header.slice(0, 20).trim(),
      minPerCm: Number(header.slice(20, 30)),
      maxPerCm: Number(header.slice(30, 40)),
      temperatureK,
      reference: header.trim().split(/\s+/u).at(-1) ?? "",
      points,
    });
    i += count + 1;
  }
  return sets;
}

/** A pair's cross-sections at several temperatures on a uniform grid, as written. */
export interface ReducedPairCrossSections {
  /** The pair's key, its members' registry keys joined by `+`. */
  readonly pair: string;
  /** The members' registry keys (decision-composition §1.9). */
  readonly members: readonly [string, string];
  readonly source: FetchedSource;
  /** How the source's values were taken onto the grid. */
  readonly reduction: string;
  /** The first row's vacuum wavelength, nm. */
  readonly firstNm: number;
  /** The rows' spacing, nm. */
  readonly stepNm: number;
  /** The temperatures of the columns after the wavelength, K, ascending. */
  readonly temperaturesK: readonly number[];
  /**
   * `linear` in T between the temperatures, `lnLinear` (ln σ linear in T), `lnLnLinear` (a power
   * law in T) or `constant` (one temperature's σ at every T).
   */
  readonly temperatureLaw: "linear" | "lnLinear" | "lnLnLinear" | "constant";
  /** The temperatures over which the table gives a measured σ, K. */
  readonly measuredRangeK: readonly [number, number];
  /** The temperatures over which the law is applied, held beyond, K: the measured range if absent. */
  readonly lawRangeK?: readonly [number, number];
  /** The wavelength below which every bin is `estimated`, nm (H₂O's continuum, R08.T4.d). */
  readonly estimatedBelowNm?: number;
  /**
   * For a pair whose second member is `air`, every other gas: each gas's weight in the partner's
   * column, 1 for a gas not listed, and the gases with which σ is `measured`.
   */
  readonly partners?: Readonly<Record<string, number>>;
  readonly measuredPartners?: readonly string[];
  /** Each row: the vacuum wavelength, nm, then σ at each temperature, m⁵ a molecule pair. */
  readonly rows: readonly (readonly number[])[];
}

/** One cm⁵ in m⁵. */
const M5_PER_CM5 = 1e-10;

/** One part of a pair's reduction: the sets of one reference, in its priority. */
export interface PairRegion {
  /** The reference number of the sets this region takes. */
  readonly reference: string;
  /**
   * A second reference whose sets colder than this one's coldest extend a bin below it, or
   * `undefined` (Thalman and Volkamer's 203 K under Finkenzeller and Volkamer's 223 K).
   */
  readonly colderFrom: string | undefined;
}

/** How a pair's table is reduced from a CIA file's sets. */
export interface PairReduction {
  readonly pair: string;
  readonly members: readonly [string, string];
  readonly source: FetchedSource;
  /** The regions in priority: each bin takes the first whose sets cover it whole at some temperature. */
  readonly regions: readonly PairRegion[];
  /** The sets' temperatures used, K, or every set's if absent. */
  readonly setTemperaturesK: readonly number[] | undefined;
  /** The table's temperatures, K, ascending. */
  readonly temperaturesK: readonly number[];
  /** How the renderer takes σ between them. */
  readonly temperatureLaw: "linear" | "lnLinear";
  readonly measuredRangeK: readonly [number, number];
  /** The reduction's statement, for the file. */
  readonly reduction: string;
}

/**
 * A pair's 1 nm bins over {@link CROSS_SECTION_RANGE_NM}, in m⁵, at the reduction's temperatures.
 *
 * @remarks
 * Each bin takes the first of the reduction's regions with a set that covers the bin whole,
 * [λ − 0.5, λ + 0.5) nm on vacuum wavelengths 10⁷ ÷ ν. At each temperature of that region's sets
 * that cover it (and of its `colderFrom` reference's, below its own coldest), the bin is the mean
 * of their points in it; at the table's temperatures it is linear in T between those, and held
 * beyond them. A bin no set covers is 0. A mean below zero, a measurement's noise, is written as
 * 0; `negatives` counts them.
 *
 * @throws Error if the sets hold no point of a region's reference, or a chosen temperature has no
 *   set.
 */
export function reducePair(
  sets: ReadonlyArray<CiaSet>,
  plan: PairReduction,
): { readonly reduced: ReducedPairCrossSections; readonly negatives: number } {
  const references = plan.regions.flatMap((r) =>
    r.colderFrom === undefined ? [r.reference] : [r.reference, r.colderFrom],
  );
  const chosen = sets.filter(
    (s) =>
      references.includes(s.reference) && (plan.setTemperaturesK?.includes(s.temperatureK) ?? true),
  );
  for (const reference of references) {
    if (!chosen.some((s) => s.reference === reference)) {
      throw new Error(`${plan.pair}'s file holds no set of reference ${reference}`);
    }
  }
  for (const t of plan.setTemperaturesK ?? []) {
    if (!chosen.some((s) => s.temperatureK === t)) {
      throw new Error(`${plan.pair}'s file holds no set at ${t} K`);
    }
  }
  const spectra = chosen.map((s) => ({
    reference: s.reference,
    temperatureK: s.temperatureK,
    rows: s.points
      .filter(([nu]) => nu > 0)
      .map(([nu, value]): readonly [number, number] => [1e7 / nu, value])
      .toReversed(),
  }));
  const covering = (reference: string, nm: number) =>
    spectra
      .filter((s) => s.reference === reference)
      .flatMap((s) => {
        const mean = binMean(s.rows, nm);
        return mean === undefined ? [] : [{ temperatureK: s.temperatureK, mean }];
      });
  const [low, high] = CROSS_SECTION_RANGE_NM;
  let negatives = 0;
  const rows: number[][] = [];
  for (let nm = low; nm <= high; nm += CONTINUUM_STEP_NM) {
    let values = plan.temperaturesK.map(() => 0);
    for (const region of plan.regions) {
      const own = covering(region.reference, nm);
      if (own.length === 0) {
        continue;
      }
      const coldest = Math.min(...own.map((p) => p.temperatureK));
      const colder =
        region.colderFrom === undefined
          ? []
          : covering(region.colderFrom, nm).filter((p) => p.temperatureK < coldest);
      const available = [...colder, ...own].toSorted((a, b) => a.temperatureK - b.temperatureK);
      values = plan.temperaturesK.map((t) => {
        const mean = linearOrHeld(available, t);
        if (mean < 0) {
          negatives += 1;
          return 0;
        }
        return rounded(mean * M5_PER_CM5);
      });
      break;
    }
    rows.push([nm, ...values]);
  }
  return {
    reduced: {
      pair: plan.pair,
      members: plan.members,
      source: plan.source,
      reduction: plan.reduction,
      firstNm: low,
      stepNm: CONTINUUM_STEP_NM,
      temperaturesK: plan.temperaturesK,
      temperatureLaw: plan.temperatureLaw,
      measuredRangeK: plan.measuredRangeK,
      rows,
    },
    negatives,
  };
}

/** A value at a temperature, linear between the two points around it and held beyond them. */
function linearOrHeld(
  points: ReadonlyArray<{ readonly temperatureK: number; readonly mean: number }>,
  temperatureK: number,
): number {
  const first = points[0];
  const last = points.at(-1);
  if (first === undefined || last === undefined) {
    return Number.NaN;
  }
  if (temperatureK <= first.temperatureK) {
    return first.mean;
  }
  if (temperatureK >= last.temperatureK) {
    return last.mean;
  }
  const k = points.findIndex((p) => p.temperatureK >= temperatureK);
  const above = points[k];
  const below = points[k - 1];
  if (above === undefined || below === undefined) {
    return Number.NaN;
  }
  const t = (temperatureK - below.temperatureK) / (above.temperatureK - below.temperatureK);
  return below.mean + (above.mean - below.mean) * t;
}

/**
 * Karman et al. 2018's citation, the computed A- and B-band collision-induced absorption of HITRAN's
 * `theory_temp_2018` alternate files (reference 27 of HITRAN's CIA references, CIA_References_2024.pdf,
 * SHA-256 `081b6452…0053`).
 */
const KARMAN_2018 =
  "T. Karman, M. A. J. Koenis, A. Banerjee, D. H. Parker, I. E. Gordon, A. van der Avoird, W. J. " +
  "van der Zande and G. C. Groenenboom, O2-O2 and O2-N2 collision-induced absorption mechanisms " +
  "unravelled, Nature Chem. 10 (2018) 549-554, DOI 10.1038/s41557-018-0015-x";

/**
 * The label {@link labelledSets} gives the sets of HITRAN's `theory_temp_2018` alternate files, so
 * that their reference 27 is not the main O₂–O₂ file's own reference 27 (an infrared set at 293 K,
 * outside the range).
 */
export const THEORY_2018_LABEL = "2018:";

/** A file's sets with a label before each set's reference, to keep two files' references apart. */
export function labelledSets(sets: ReadonlyArray<CiaSet>, label: string): CiaSet[] {
  return sets.map((set) => ({ ...set, reference: `${label}${set.reference}` }));
}

/**
 * HITRAN's O₂–O₂ alternate file `O2-O2_theory_temp_2018.cia`: Karman et al. 2018's computed sets
 * (headed "Theory fit to expt.", reference 27) at 15 temperatures, 206–346 K every 10 K, for the
 * A band's b¹Σg⁺ ← X³Σg⁻ (0, 0) over 12,802–13,402 cm⁻¹ (746.2–781.1 nm) and the B band's (1, 0)
 * over 14,206–14,806 cm⁻¹ (675.4–703.9 nm), beside the a¹Δg bands in the infrared.
 *
 * @remarks
 * HITRAN's CIA readme (CIA_Readme_2024.pdf, SHA-256 `694bd796…9830`, its Table 1) lists the set
 * among its Alternate files, which "could have a clear advantage over the recommended set for
 * specific applications, e.g. extended temperature ranges"; they replace the B and A bands' single
 * temperatures here (`decision-r08-t4b-t12c.md`, follow-up 5, ruling 7, `measured` over 206–346 K).
 */
export const OXYGEN_PAIR_THEORY_SOURCE: FetchedSource = {
  citation: `${KARMAN_2018}, as HITRAN's collision-induced absorption section gives it (alternate file O2-O2_theory_temp_2018.cia; J. Terragni et al., J. Quant. Spectrosc. Radiat. Transfer 347 (2025) 109631, DOI 10.1016/j.jqsrt.2025.109631)`,
  url: "https://hitran.org/data/CIA/alternate/O2-O2_theory_temp_2018.cia",
  sha256: "0330b4ae088acf7a392be34b0206ddcd4fe40700f40eebc99312c060b9f559d4",
  fetched: "2026-10-12",
  terms: HITRAN_TERMS,
};

/**
 * HITRAN's O₂–N₂ alternate file `O2-N2_theory_temp_2018.cia`: Karman et al. 2018's computed sets
 * (reference 27) at 206–346 K every 10 K for the A band over 12,802–13,402 cm⁻¹ (746.2–781.1 nm)
 * and the a¹Δg band in the infrared; it holds no B-band set.
 */
export const OXYGEN_NITROGEN_PAIR_THEORY_SOURCE: FetchedSource = {
  citation: `${KARMAN_2018}, as HITRAN's collision-induced absorption section gives it (alternate file O2-N2_theory_temp_2018.cia; J. Terragni et al., J. Quant. Spectrosc. Radiat. Transfer 347 (2025) 109631, DOI 10.1016/j.jqsrt.2025.109631)`,
  url: "https://hitran.org/data/CIA/alternate/O2-N2_theory_temp_2018.cia",
  sha256: "ddc92262115bd348d7199b8cf6f0074f38c07c4873329263b657120db7bab329",
  fetched: "2026-10-12",
  terms: HITRAN_TERMS,
};

/** A fetched file's name, the last part of its URL. */
function fileName(source: FetchedSource): string {
  return source.url.split("/").at(-1) ?? source.url;
}

/** Two fetched files as one source: both citations, URLs and checksums, each after its file's name. */
function bothSources(primary: FetchedSource, theory: FetchedSource): FetchedSource {
  return {
    citation: `${primary.citation}; with ${theory.citation}`,
    url: `${primary.url} and ${theory.url}`,
    sha256: `${fileName(primary)} ${primary.sha256}; ${fileName(theory)} ${theory.sha256}`,
    fetched: `${primary.fetched} and ${theory.fetched}`,
    terms: primary.terms,
  };
}

/** Karman et al. 2018's temperatures, K: 206–346 K every 10 K. */
export const KARMAN_TEMPERATURES_K: readonly number[] = Array.from(
  { length: 15 },
  (_, i) => 206 + 10 * i,
);

/** A sorted union of temperature lists, each once. */
function unionK(...lists: ReadonlyArray<ReadonlyArray<number>>): number[] {
  return [...new Set(lists.flat())].toSorted((a, b) => a - b);
}

/**
 * O₂–O₂'s reduction ({@link OXYGEN_PAIR_SOURCE} and {@link OXYGEN_PAIR_THEORY_SOURCE}):
 * Finkenzeller and Volkamer's 297–500 nm sets first, extended below their 223 K by Thalman and
 * Volkamer's 203 K set, then Thalman and Volkamer's, then Karman et al. 2018's B and A bands, then
 * the B band's and the A band's single temperatures where Karman's sets do not cover a bin whole.
 *
 * @remarks
 * The table's temperatures are the union of the visible double transitions' sets' (203–293 K) and
 * Karman's (206–346 K), so that each region's own piecewise-linear law in T is kept exactly. It is
 * measured over 206–287 K, where every double-transition bin lies between two of the measurements'
 * own sets (`decision-r08-t4b-t12c.md`, follow-up 3, ruling 7) and every B- and A-band bin lies
 * within Karman's computed range (follow-up 5, ruling 7); outside it a bin's nearest set is held.
 * Karman's B band covers 676–703 nm whole and their A band 747–760 nm, so Spiering et al.'s B band
 * keeps 672–675 nm and Tran et al.'s A band 733–746 nm (up to 15% of the 760 nm bin), each at its
 * one temperature, held at every other: a stated approximation, recorded.
 */
export const OXYGEN_PAIR_REDUCTION: PairReduction = {
  pair: "O2+O2",
  members: ["O2", "O2"],
  source: bothSources(OXYGEN_PAIR_SOURCE, OXYGEN_PAIR_THEORY_SOURCE),
  regions: [
    { reference: "54", colderFrom: "32" },
    { reference: "32", colderFrom: undefined },
    { reference: `${THEORY_2018_LABEL}27`, colderFrom: undefined },
    { reference: "30", colderFrom: undefined },
    { reference: "4", colderFrom: undefined },
  ],
  setTemperaturesK: undefined,
  temperaturesK: unionK([203, 223, 233, 253, 263, 273, 287, 293], KARMAN_TEMPERATURES_K),
  temperatureLaw: "linear",
  measuredRangeK: [206, 287],
  reduction:
    "The main file's sets and the theory file's (labelled 2018:), on vacuum wavelengths 1e7/nu. " +
    "Each 1 nm bin, [nm - 0.5, nm + 0.5), takes the first of references 54 (Finkenzeller and " +
    "Volkamer 2022, 297-500 nm at 223, 263 and 293 K, with reference 32's 203 K set below them), " +
    "32 (Thalman and Volkamer 2013, 335-654 nm at 203-293 K), 2018:27 (Karman et al. 2018, the B " +
    "band 675.4-703.9 nm and the A band 746.2-781.1 nm at 206-346 K), 30 (Spiering et al. 2011, the " +
    "B band's, 671-704 nm at 293 K) and 4 (Tran et al. 2006, the A band's, 723-794 nm at 296 K) " +
    "with a set covering it whole: at each of those sets' temperatures the mean of its points in " +
    "the bin, linear in T between them and held beyond, converted from cm5 to m5 and written to 6 " +
    "significant figures. A bin no set covers (654-671, 704-723 and 794-800 nm) is 0, and a mean " +
    "below zero, the measurements' noise, is written as 0. Measured over 206-287 K, where every " +
    "double-transition bin lies between two of the measurements' sets and every B- and A-band bin " +
    "within Karman et al.'s; Spiering et al.'s 672-675 nm and Tran et al.'s 733-746 nm, which " +
    "Karman et al.'s sets do not cover whole, are held at their one temperature.",
};

/**
 * HITRAN2024's O₂–N₂ collision-induced absorption file (Terragni et al. 2025), whose A-band set
 * (reference 4, Tran, Boulet and Hartmann 2006, 12,600–13,840 cm⁻¹ at 296 K) is the O₂–N₂ pair's
 * in the range.
 *
 * @remarks
 * O₂–N₂ alone, not the file of O₂–air, which adds O₂–O₂'s own A band (`O2+O2`'s table holds it)
 * and so would count it twice (HITRAN's CIA readme, "Users should not double count").
 */
export const OXYGEN_NITROGEN_PAIR_SOURCE: FetchedSource = {
  citation:
    "HITRAN2024 collision-induced absorption, file O2-N2_2024.cia: J. Terragni et al., J. Quant. " +
    "Spectrosc. Radiat. Transfer 347 (2025) 109631, DOI 10.1016/j.jqsrt.2025.109631; I. E. " +
    "Gordon et al., The HITRAN2024 molecular spectroscopic database, J. Quant. Spectrosc. Radiat. " +
    "Transfer 353 (2026) 109807, DOI 10.1016/j.jqsrt.2026.109807. Its A-band set: H. Tran, C. " +
    "Boulet and J.-M. Hartmann, J. Geophys. Res. 111 (2006) D15210, DOI 10.1029/2005JD006869",
  url: "https://hitran.org/data/CIA/main/O2-N2_2024.cia",
  sha256: "90851fbfae226d7ecda93efc0de16105f72966a532adc7f1bc234cb13833799f",
  fetched: "2026-10-11",
  terms: HITRAN_TERMS,
};

/**
 * HITRAN2024's O₂–CO₂ collision-induced absorption file, whose A-band set (reference 5,
 * Vangvichith, Tran and Hartmann 2009, 12,600–13,840 cm⁻¹ at 296 K) is the O₂–CO₂ pair's in the
 * range.
 *
 * @remarks
 * Found at T4.c's fetch (`decision-r08-t4b-t12c.md`, follow-up 3, addition 5, had found none and
 * set it pending in T4.d): its data are nonzero over 733–760 nm in the range.
 */
export const OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE: FetchedSource = {
  citation:
    "HITRAN2024 collision-induced absorption, file O2-CO2_2024.cia: J. Terragni et al., J. Quant. " +
    "Spectrosc. Radiat. Transfer 347 (2025) 109631, DOI 10.1016/j.jqsrt.2025.109631; I. E. " +
    "Gordon et al., The HITRAN2024 molecular spectroscopic database, J. Quant. Spectrosc. Radiat. " +
    "Transfer 353 (2026) 109807, DOI 10.1016/j.jqsrt.2026.109807. Its A-band set: M. " +
    "Vangvichith, H. Tran and J.-M. Hartmann, J. Quant. Spectrosc. Radiat. Transfer 110 (2009) " +
    "2212-2216, DOI 10.1016/j.jqsrt.2009.06.002",
  url: "https://hitran.org/data/CIA/main/O2-CO2_2024.cia",
  sha256: "1866debcc07900ed7647809e4263229f663ce0d367ef9ed07749117ae843e7df",
  fetched: "2026-10-11",
  terms: HITRAN_TERMS,
};

/**
 * O₂–CO₂'s reduction ({@link OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE}): the A band's set at its one
 * temperature, 296 K, and measured there alone.
 */
export const OXYGEN_CARBON_DIOXIDE_PAIR_REDUCTION: PairReduction = {
  pair: "O2+CO2",
  members: ["O2", "CO2"],
  source: OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE,
  regions: [{ reference: "5", colderFrom: undefined }],
  setTemperaturesK: undefined,
  temperaturesK: [296],
  temperatureLaw: "linear",
  measuredRangeK: [296, 296],
  reduction:
    "The file's A-band set (reference 5, Vangvichith et al. 2009, 12600-13840 cm-1 at 296 K) on " +
    "vacuum wavelengths 1e7/nu: each 1 nm bin, [nm - 0.5, nm + 0.5), that the set covers whole, " +
    "the mean of its points in the bin, converted from cm5 to m5 and written to 6 significant " +
    "figures; every other bin is 0. Measured at 296 K alone; held, estimated, at every other " +
    "temperature.",
};

/**
 * O₂–N₂'s reduction ({@link OXYGEN_NITROGEN_PAIR_SOURCE} and
 * {@link OXYGEN_NITROGEN_PAIR_THEORY_SOURCE}): Karman et al. 2018's A band over 747–760 nm, then
 * Tran et al.'s A-band set, nonzero from 733 nm, where Karman's do not cover a bin whole.
 *
 * @remarks
 * Measured over Karman's 206–346 K. Tran et al.'s 733–746 nm bins, at 296 K alone, are held at
 * every other temperature: a stated approximation, as O₂–O₂'s are.
 */
export const OXYGEN_NITROGEN_PAIR_REDUCTION: PairReduction = {
  pair: "O2+N2",
  members: ["O2", "N2"],
  source: bothSources(OXYGEN_NITROGEN_PAIR_SOURCE, OXYGEN_NITROGEN_PAIR_THEORY_SOURCE),
  regions: [
    { reference: `${THEORY_2018_LABEL}27`, colderFrom: undefined },
    { reference: "4", colderFrom: undefined },
  ],
  setTemperaturesK: undefined,
  temperaturesK: unionK([296], KARMAN_TEMPERATURES_K),
  temperatureLaw: "linear",
  measuredRangeK: [206, 346],
  reduction:
    "The theory file's A-band sets (labelled 2018:27, Karman et al. 2018, 12802-13402 cm-1, " +
    "746.2-781.1 nm, at 206-346 K) and then the main file's (reference 4, Tran et al. 2006, " +
    "12600-13840 cm-1 at 296 K), on vacuum wavelengths 1e7/nu: each 1 nm bin, [nm - 0.5, nm + " +
    "0.5), takes the first with a set that covers it whole, at each of its sets' temperatures the " +
    "mean of its points in the bin, linear in T between them and held beyond, converted from cm5 " +
    "to m5 and written to 6 significant figures; every other bin is 0. Measured over 206-346 K; " +
    "Tran et al.'s 733-746 nm, which Karman et al.'s sets do not cover whole, are held at 296 K.",
};

/**
 * The temperatures of H₂–He's table, K: the file's own sets at these. The sets of 950–1,175 K are
 * skipped: over 501–760 nm each lies a median 14–29% (at most 59%) below the ln-linear bridge
 * between the 900 and 1,200 K sets, and falls with temperature in 18–100% of the bins, where every
 * other set departs from its neighbours' trend by at most 0.3% (median), a few at steps of up to
 * 10% (1,800, 2,250 and 3,475–3,500 K), and falls in at most 5% of them. The 925 K set lies on the
 * bridge (median +1.3%, at most +7.9%) and is kept. Between 925 and 1,200 K σ is ln-linear in T.
 */
export const HYDROGEN_HELIUM_TEMPERATURES_K: readonly number[] = [
  200, 225, 250, 300, 350, 400, 500, 600, 700, 800, 900, 925, 1_200, 1_500, 1_800, 2_000, 2_250,
  2_500, 3_000, 3_475, 3_500, 4_000, 5_000, 6_000, 7_500, 9_900,
];

/**
 * H₂–He's reduction ({@link HYDROGEN_HELIUM_PAIR_SOURCE}): its sets at
 * {@link HYDROGEN_HELIUM_TEMPERATURES_K}, over 501–760 nm, where its 20,000 cm⁻¹ end allows, ln σ
 * linear in T between them.
 */
export const HYDROGEN_HELIUM_PAIR_REDUCTION: PairReduction = {
  pair: "H2+He",
  members: ["H2", "He"],
  source: HYDROGEN_HELIUM_PAIR_SOURCE,
  regions: [{ reference: "7", colderFrom: undefined }],
  setTemperaturesK: HYDROGEN_HELIUM_TEMPERATURES_K,
  temperaturesK: HYDROGEN_HELIUM_TEMPERATURES_K,
  temperatureLaw: "lnLinear",
  measuredRangeK: [200, 9_900],
  reduction:
    "The file's sets at the table's 26 temperatures, on vacuum wavelengths 1e7/nu: each 1 nm " +
    "bin, [nm - 0.5, nm + 0.5), the mean of a set's points in it, converted from cm5 to m5 and " +
    "written to 6 significant figures. The file ends at 20000 cm-1, so the bins up to 500 nm are " +
    "0: it holds nothing shorter. ln sigma is linear in T between the temperatures, within 5% of " +
    "every other set but at the file's own steps (1800-1825, 600-625 and 6000-6025 K, to 11%); " +
    "measured over 200-9900 K. The sets of 950-1175 K are skipped: each lies a median 14-29% " +
    "(at most 59%) below the ln-linear bridge between the 900 and 1200 K sets and falls with T " +
    "in 18-100% of the bins, where every other set departs from its neighbours' trend by at most " +
    "0.3% (median) and falls in at most 5%; the 925 K set lies on the bridge and is kept.",
};

/** A fetched file, by its name and SHA-256. */
export interface NamedFile {
  readonly name: string;
  readonly sha256: string;
}

/**
 * Borysow's three H₂–H₂ tables, from her page, fetched 2026-10-11 (`decision-r08-cmf-licence.md`
 * §8), coldest first.
 * - `final_CIA_LT.dat`: 60–350 K, every 10 cm⁻¹ to 16,490 cm⁻¹ (Borysow 2002).
 * - `final_CIA_HT.dat`: 400–1,000 K, every 10 cm⁻¹ to 17,000 cm⁻¹, its 600 K column ending at
 *   16,790 cm⁻¹ (Borysow 2002).
 * - `CIA.H2H2.Yi`: 1,000–7,000 K, every 20 cm⁻¹ to 20,000 cm⁻¹ (Borysow, Jørgensen and Fu 2001).
 */
export const HYDROGEN_PAIR_FILES: readonly [NamedFile, NamedFile, NamedFile] = [
  {
    name: "final_CIA_LT.dat",
    sha256: "fcd884332d31b948f8bba13a9135cb18f89ca44147e87bf5d04e385297c844dc",
  },
  {
    name: "final_CIA_HT.dat",
    sha256: "8616d396f9656c0b36540411a24283b07f009ae718df3ef9681dad7aaed66aec",
  },
  {
    name: "CIA.H2H2.Yi",
    sha256: "c189a3e08316657dfa58d99b4f5026126fff5dafa2f79c126f637a0db86585d8",
  },
];

/**
 * Borysow's H₂–H₂ collision-induced absorption, k in cm⁻¹ amagat⁻² for hydrogen in thermal
 * equilibrium (`decision-r08-cmf-licence.md` §8). Her page reads "All rights reserved" but offers
 * the tables for use and asks that the papers be cited.
 *
 * @remarks
 * HITRAN's H₂–H₂ file and its alternates end at 10,000 cm⁻¹ (1 µm), so these tables are the only
 * open source that reaches the range. They are computed, not measured, and the sources state their
 * seams:
 * - Borysow 2002 §7: the second overtone takes a different model at 400–500 K than at 600–1,000
 *   K, so σ at 760 nm nearly doubles from 500 to 600 K.
 * - Her 60–350 K and 400–1,000 K tables part in the overtones' far wing. At 650 nm, 400 K's value
 *   is a thirtieth of 350 K's. σ there is below 10⁻⁶⁰ m⁵, beside 10⁻⁵⁸ at 760 nm.
 * - At 1,000 K the 2001 table's values are 0.82–2.3 times the 2002 table's over 589–760 nm, an
 *   agreement the 2002 paper calls "remarkable" (its §8 and Fig. 2).
 */
export const HYDROGEN_PAIR_SOURCE: FetchedSource = {
  citation:
    'A. Borysow, "Collision-induced absorption coefficients of H2 pairs at temperatures from 60 K ' +
    'to 1000 K", Astron. Astrophys. 390 (2002) 779-782, DOI 10.1051/0004-6361:20020555; ' +
    "final_CIA_LT.dat and final_CIA_HT.dat from http://www.astro.ku.dk/~aborysow/programs/ " +
    `(SHA-256 ${HYDROGEN_PAIR_FILES[0].sha256} and ${HYDROGEN_PAIR_FILES[1].sha256}, fetched ` +
    "2026-10-11); above 1000 K, A. Borysow, U. G. Jorgensen and Y. Fu, J. Quant. Spectrosc. " +
    "Radiat. Transfer 68 (2001) 235-255, DOI 10.1016/S0022-4073(00)00023-6, CIA.H2H2.Yi from the " +
    `same page (SHA-256 ${HYDROGEN_PAIR_FILES[2].sha256})`,
  url: "http://www.astro.ku.dk/~aborysow/programs/",
  sha256: HYDROGEN_PAIR_FILES.map((f) => `${f.name} ${f.sha256}`).join("; "),
  fetched: "2026-10-11",
  terms:
    'The page states "All rights reserved" and asks that the papers be cited; no open source ' +
    "holds H2-H2 above 10,000 cm-1: reduced values with their citation, the raw files not " +
    "committed (decision-r08-cmf-licence.md §8)",
};

/** One of Borysow's tables: its temperatures and each one's points. */
export interface BorysowTable {
  /** The columns' temperatures, K, in the file's order. */
  readonly temperaturesK: readonly number[];
  /**
   * Each column's points, ascending in wavenumber: the vacuum wavenumber, cm⁻¹, and k, cm⁻¹
   * amagat⁻². A column the file leaves blank on a row has no point there.
   */
  readonly sets: ReadonlyArray<ReadonlyArray<readonly [number, number]>>;
}

/** A line with its tabs expanded to every eighth character, as a terminal shows it. */
function expandTabs(text: string): string {
  let out = "";
  for (const ch of text) {
    out += ch === "\t" ? " ".repeat(8 - (out.length % 8)) : ch;
  }
  return out;
}

/** How far a value's right edge may lie from its column's, characters. */
const COLUMN_EDGE_TOLERANCE = 4;

/**
 * One of Borysow's tables. It has header lines, the one naming the most temperatures (`60K`)
 * giving the columns, then rows of a wavenumber and a right-aligned value per column.
 *
 * @remarks
 * A row may leave a column blank (`final_CIA_HT.dat`'s 600 K from 16,800 cm⁻¹). A shorter row's
 * values are placed in the columns whose right edges, in the last full row, are nearest their own,
 * with tabs expanded.
 *
 * @throws Error naming the file for a header that names no temperatures, a short row before any
 *   full one, a value no column's edge lies near, a negative coefficient, or a column whose
 *   wavenumbers do not ascend.
 */
export function parseBorysow(text: string, what: string): BorysowTable {
  const lines = text.split(/\r?\n/u).map(expandTabs);
  let temperaturesK: number[] = [];
  for (const entry of lines) {
    const named = [...entry.matchAll(/(\d+)\s*K\b/gu)].map((m) => Number(m[1]));
    if (named.length > temperaturesK.length) {
      temperaturesK = named;
    }
  }
  if (temperaturesK.length === 0) {
    throw new Error(`${what} names no temperatures`);
  }
  const sets: (readonly [number, number])[][] = temperaturesK.map(() => []);
  let edges: number[] | undefined;
  for (const [i, entry] of lines.entries()) {
    const tokens = [...entry.matchAll(/\S+/gu)];
    const values = tokens.map((t) => Number(t[0]));
    const [nu, ...coefficients] = values;
    if (nu === undefined || !values.every((v) => Number.isFinite(v))) {
      continue;
    }
    const ends = tokens.map((t) => t.index + t[0].length);
    if (coefficients.length === temperaturesK.length) {
      edges = ends;
    } else if (edges === undefined || coefficients.length > temperaturesK.length) {
      throw new Error(`${what} line ${i + 1} does not fit the table's columns`);
    }
    const columnEdges = edges;
    let previous = 0;
    for (const [k, value] of coefficients.entries()) {
      const end = ends[k + 1] ?? Number.NaN;
      const column = columnEdges.findIndex(
        (edge, c) => c > previous && Math.abs(edge - end) <= COLUMN_EDGE_TOLERANCE,
      );
      const set = sets[column - 1];
      if (column < 0 || set === undefined) {
        throw new Error(`${what} line ${i + 1}'s value ${k + 1} lies under no column`);
      }
      if (value < 0) {
        throw new Error(`${what} line ${i + 1} holds a coefficient below 0`);
      }
      const last = set.at(-1);
      if (last !== undefined && !(nu > last[0])) {
        throw new Error(`${what} line ${i + 1} does not ascend in wavenumber`);
      }
      set.push([nu, value]);
      previous = column;
    }
  }
  return { temperaturesK, sets };
}

/**
 * Borysow's amagat for hydrogen, cm⁻³: 2.686 76 × 10¹⁹, the ideal gas at standard temperature and
 * pressure (Borysow 2002, footnote 1). σ = k ÷ n₀².
 */
export const BORYSOW_AMAGAT_PER_CM3 = 2.686_76e19;

/**
 * The mean over [low, high] cm⁻¹ of points' interpolant, linear in wavenumber, or `undefined`
 * unless the points span the interval.
 */
function intervalMean(
  points: ReadonlyArray<readonly [number, number]>,
  lowPerCm: number,
  highPerCm: number,
): number | undefined {
  const first = points[0];
  const last = points.at(-1);
  if (first === undefined || last === undefined || first[0] > lowPerCm || last[0] < highPerCm) {
    return undefined;
  }
  let integral = 0;
  for (const [k, [x0, y0]] of points.entries()) {
    const [x1, y1] = points[k + 1] ?? [x0, y0];
    const from = Math.max(x0, lowPerCm);
    const to = Math.min(x1, highPerCm);
    if (to > from) {
      const at = (x: number) => y0 + ((y1 - y0) * (x - x0)) / (x1 - x0);
      integral += 0.5 * (at(from) + at(to)) * (to - from);
    }
  }
  return integral / (highPerCm - lowPerCm);
}

/**
 * H₂–H₂'s 1 nm bins over {@link CROSS_SECTION_RANGE_NM}, in m⁵, from Borysow's three tables
 * ({@link HYDROGEN_PAIR_FILES}, in that order).
 *
 * @remarks
 * The temperatures are the 2002 tables' 60–1,000 K, then the 2001 table's above 1,000 K; its own
 * 1,000 K set is not used. At each temperature a bin, [λ − 0.5, λ + 0.5) nm on vacuum wavelengths
 * 10⁷ ÷ ν, is the mean of k over its wavenumbers, k interpolated linearly between the table's
 * points, divided by n₀² ({@link BORYSOW_AMAGAT_PER_CM3}). A bin the table does not cover whole is
 * 0. The 60–350 K table ends at 16,490 cm⁻¹ and the 400–1,000 K one at 17,000 cm⁻¹ (16,790 cm⁻¹ at
 * 600 K), so their bins below 607 and 589 nm (597 nm at 600 K) are 0. The 5–0 overtone near
 * 541 nm is untabulated below 1,000 K, a stated approximation (`decision-r08-cmf-licence.md` §8).
 *
 * @throws Error unless the tables' temperatures ascend from one to the next.
 */
export function reduceHydrogenPair(
  tables: readonly [BorysowTable, BorysowTable, BorysowTable],
): ReducedPairCrossSections {
  const [low, high, hot] = tables;
  const highFrom = high.temperaturesK[0] ?? Number.NaN;
  const highTo = high.temperaturesK.at(-1) ?? Number.NaN;
  if (!((low.temperaturesK.at(-1) ?? Number.NaN) < highFrom)) {
    throw new Error("H2-H2's 2002 tables' temperatures do not ascend from one to the other");
  }
  const columns = [
    ...low.temperaturesK.map((t, k) => ({ temperatureK: t, points: low.sets[k] ?? [] })),
    ...high.temperaturesK.map((t, k) => ({ temperatureK: t, points: high.sets[k] ?? [] })),
    ...hot.temperaturesK.flatMap((t, k) =>
      t > highTo ? [{ temperatureK: t, points: hot.sets[k] ?? [] }] : [],
    ),
  ];
  if (columns.some((c, k) => k > 0 && !(c.temperatureK > (columns[k - 1]?.temperatureK ?? 0)))) {
    throw new Error("H2-H2's tables' temperatures do not ascend");
  }
  const [first, last] = CROSS_SECTION_RANGE_NM;
  const rows: number[][] = [];
  for (let nm = first; nm <= last; nm += CONTINUUM_STEP_NM) {
    const lowPerCm = 1e7 / (nm + CONTINUUM_STEP_NM / 2);
    const highPerCm = 1e7 / (nm - CONTINUUM_STEP_NM / 2);
    rows.push([
      nm,
      ...columns.map(({ points }) => {
        const mean = intervalMean(points, lowPerCm, highPerCm);
        return mean === undefined ? 0 : rounded((mean / BORYSOW_AMAGAT_PER_CM3 ** 2) * M5_PER_CM5);
      }),
    ]);
  }
  const temperaturesK = columns.map((c) => c.temperatureK);
  return {
    pair: "H2+H2",
    members: ["H2", "H2"],
    source: HYDROGEN_PAIR_SOURCE,
    reduction:
      "Borysow's coefficients k, cm-1 amagat-2, for hydrogen in thermal equilibrium, as sigma = " +
      "k / n0^2 with her amagat n0 = 2.68676e19 cm-3 (her 2002 paper's footnote 1), converted " +
      "from cm5 to m5: each 1 nm bin, [nm - 0.5, nm + 0.5) on vacuum wavelengths 1e7/nu, the " +
      "mean over its wavenumbers of the table's values interpolated linearly in wavenumber, where " +
      "the table covers the whole bin, written to 6 significant figures; every other bin is 0. " +
      "The 2002 tables give 60-1000 K and the 2001 table the temperatures above (its 1000 K set " +
      "is not used). They end at 16490 cm-1 (60-350 K), 17000 cm-1 (400-1000 K; 16790 cm-1 at " +
      "600 K) and 20000 cm-1 (2000-7000 K), so sigma is 0 below 607, 589 (597 at 600 K) and 501 " +
      "nm: the 5-0 overtone near 541 nm is untabulated below 1000 K, a stated approximation. ln " +
      `sigma is linear in T between the temperatures; measured over ${temperaturesK[0]}-` +
      `${temperaturesK.at(-1)} K.`,
    firstNm: first,
    stepNm: CONTINUUM_STEP_NM,
    temperaturesK,
    temperatureLaw: "lnLinear",
    measuredRangeK: [temperaturesK[0] ?? Number.NaN, temperaturesK.at(-1) ?? Number.NaN],
    rows,
  };
}

/** A reduced table's fields after its key, as the cross-sections files write them. */
function tableFields(table: ReducedCrossSections | ReducedPairCrossSections): string[] {
  const s = table.source;
  const pairFields =
    "pair" in table
      ? [
          ...(table.lawRangeK === undefined ? [] : [`      "lawRangeK": ${line(table.lawRangeK)}`]),
          ...(table.estimatedBelowNm === undefined
            ? []
            : [`      "estimatedBelowNm": ${table.estimatedBelowNm}`]),
          ...(table.partners === undefined
            ? []
            : [
                `      "partners": {${Object.entries(table.partners)
                  .map(([gas, w]) => `${JSON.stringify(gas)}: ${Number(w.toPrecision(4))}`)
                  .join(", ")}}`,
              ]),
          ...(table.measuredPartners === undefined
            ? []
            : [
                `      "measuredPartners": [${table.measuredPartners.map((g) => JSON.stringify(g)).join(", ")}]`,
              ]),
        ]
      : [];
  return [
    `      "source": ${JSON.stringify(s.citation)}`,
    `      "url": ${JSON.stringify(s.url)}`,
    `      "sha256": ${JSON.stringify(s.sha256)}`,
    `      "fetched": ${JSON.stringify(s.fetched)}`,
    `      "terms": ${JSON.stringify(s.terms)}`,
    `      "reduction": ${JSON.stringify(table.reduction)}`,
    `      "firstNm": ${table.firstNm}`,
    `      "stepNm": ${table.stepNm}`,
    `      "temperaturesK": ${line(table.temperaturesK)}`,
    `      "temperatureLaw": ${JSON.stringify(table.temperatureLaw)}`,
    `      "measuredRangeK": ${line(table.measuredRangeK)}`,
    ...pairFields,
    `      "rows": [\n${table.rows.map((r) => `        ${line(r)}`).join(",\n")}\n      ]`,
  ];
}

/** A pair table as the cross-sections files write it. */
function pairEntry(table: ReducedPairCrossSections): string {
  const fields = [
    `      "pair": ${JSON.stringify(table.pair)}`,
    `      "members": [${table.members.map((m) => JSON.stringify(m)).join(", ")}]`,
    ...tableFields(table),
  ];
  return `    {\n${fields.join(",\n")}\n    }`;
}

/** The text of `crossSections.json`: its species tables and its pairs. */
export function crossSectionsText(
  tables: readonly ReducedCrossSections[],
  pairs: readonly ReducedPairCrossSections[],
): string {
  const description =
    "Absorption cross-sections of the absorbers the client draws, on vacuum wavelengths " +
    "(rendering plan R08, R08.T4.b and R08.T4.c), written by apps/hyperion/scripts/" +
    "atmosphereData.mjs cross-sections from the fetched sources named here, which are not " +
    "committed. Each table's rows are [nm, sigma at each of temperaturesK]: m2 a molecule in " +
    "tables, and m5 a molecule pair in pairs, whose absorption coefficient is sigma n_A n_B. " +
    "Reduced values; the citations are in NOTICE's Data section.";
  const entries = tables.map((table) => {
    const fields = [`      "species": ${JSON.stringify(table.species)}`, ...tableFields(table)];
    return `    {\n${fields.join(",\n")}\n    }`;
  });
  return (
    `{\n  "description": ${JSON.stringify(description)},\n` +
    `  "tables": [\n${entries.join(",\n")}\n  ],\n` +
    `  "pairs": [\n${pairs.map(pairEntry).join(",\n")}\n  ]\n}\n`
  );
}

/**
 * The text of `oxygenPairs.json` (R08.T4.d): O₂'s collision-induced pairs, kept apart from
 * `crossSections.json`, which the 500 kB file hook no longer leaves room in for their widened
 * temperatures.
 */
export function oxygenPairsText(pairs: readonly ReducedPairCrossSections[]): string {
  const description =
    "Collision-induced absorption cross-sections of O2's pairs, on vacuum wavelengths (rendering " +
    "plan R08, R08.T4.c and R08.T4.d, Karman et al. 2018's A and B bands), written by " +
    "apps/hyperion/scripts/atmosphereData.mjs cross-sections from the fetched sources named " +
    "here, which are not committed. Each table's rows are [nm, sigma at each of temperaturesK], " +
    "m5 a molecule pair, whose absorption coefficient is sigma n_A n_B. Reduced values; the " +
    "citations are in NOTICE's Data section.";
  return (
    `{\n  "description": ${JSON.stringify(description)},\n` +
    `  "pairs": [\n${pairs.map(pairEntry).join(",\n")}\n  ]\n}\n`
  );
}

const USAGE =
  "usage: atmosphereData matching --cie <CIE_xyz_1931_2deg.csv> [--out <json>]\n" +
  "       atmosphereData cross-sections --ozone <serdyuchenkogorshelev5digits.dat> " +
  "--methane <ch4.txt> --so2 <file> (each of the snapshot's four) --o2o2 <O2-O2_2024.cia> " +
  "--h2he <H2-He_2011.cia> --o2n2 <O2-N2_2024.cia> --o2co2 <O2-CO2_2024.cia> --h2h2 <file> " +
  "(each of Borysow's three) --o2o2-theory <O2-O2_theory_temp_2018.cia> " +
  "--o2n2-theory <O2-N2_theory_temp_2018.cia> --lowtran <lowtran7.f> --h2o-lines " +
  "<h2o-13100-26400-broad.csv> [--out <json>] [--pairs-out <json>]\n" +
  "       atmosphereData k-distributions --species <key> --lines <file> --tips <dir> " +
  "[--cells a-b] [--stride n:k] [--temperatures t,...] --out <partial.json>\n" +
  "       atmosphereData k-distributions-merge --species <key> --out-dir <dir> [--lines <file>] " +
  "<partial.json>...\n" +
  "       atmosphereData earth-reference --paths <earthLinePaths.json> --o2-lines <csv> " +
  "--h2o-lines <csv> --tips <dir> --out <json>\n" +
  "       atmosphereData cell-references --species <key> --lines <file> --tips <dir> " +
  "--table <sidecar.json> --cells <nm,...> --out <json>\n" +
  "       atmosphereData background-error --species <key> --lines <file> --tips <dir> " +
  "--cells <nm,...> --gas CO2|H2|He|H2O [--pressures <Pa,...>]\n" +
  "       atmosphereData exomol-bands --species N2O --data <dir> --hitran-par <file> " +
  "--cache <dir> [--jobs n]\n" +
  "       atmosphereData exomol-bins --species NH3|N2O --data <dir> [--broad <dir>] " +
  "--cache <dir> [--temperatures t,...] [--jobs n]\n" +
  "       atmosphereData exomol-k-distributions --species NH3|N2O --cache <dir> " +
  "[--hitran-lines <csv> --tips <dir>] [--cells a-b] [--stride n:k] [--temperatures t,...] " +
  "--out <partial.json>\n" +
  "       atmosphereData exomol-k-distributions-merge --species NH3|N2O --cache <dir> " +
  "--out-dir <dir> [--hitran-par <file> --record <json>] <partial.json>...\n" +
  "       atmosphereData exomol-cell-references --species NH3|N2O --cache <dir> " +
  "[--hitran-lines <csv> --tips <dir>] --table <sidecar.json> --cells <nm,...> --out <json>\n" +
  "       atmosphereData atomic-shares --kurucz <dir> --barklem <dir> [--asd-zn2 <csv>] " +
  "[--stages <key,...>] [--out <json>]\n" +
  "       atmosphereData atomic-bounds --kurucz <dir> --barklem <dir> --asd-zn2 <csv> [--out <json>]\n" +
  "       atmosphereData hcl-overtones --lines <hcl-13100-26400-broad.csv> --tips <dir> [--bounds <json>]";

/** `matching`: writes `colourMatching.json`. */
function runMatching(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: { cie: { type: "string" }, out: { type: "string" } },
  });
  if (values.cie === undefined) {
    throw new Error(USAGE);
  }
  const reduced = reduceMatching(parseCie(readChecked(values.cie, CIE_CSV_SHA256)));
  const out = values.out ?? MATCHING_OUTPUT;
  writeFileSync(out, matchingText(reduced));
  process.stdout.write(`atmosphere-data: wrote ${reduced.rows.length} rows to ${out}\n`);
}

/** `cross-sections`: writes `absorbers/crossSections.json`. */
function runCrossSections(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      ozone: { type: "string" },
      methane: { type: "string" },
      so2: { type: "string", multiple: true },
      o2o2: { type: "string" },
      h2he: { type: "string" },
      o2n2: { type: "string" },
      o2co2: { type: "string" },
      h2h2: { type: "string", multiple: true },
      "o2o2-theory": { type: "string" },
      "o2n2-theory": { type: "string" },
      lowtran: { type: "string" },
      "h2o-lines": { type: "string" },
      out: { type: "string" },
      "pairs-out": { type: "string" },
    },
  });
  const { ozone: ozonePath, methane: methanePath, so2, o2o2, h2he, o2n2, o2co2, h2h2 } = values;
  const o2o2Theory = values["o2o2-theory"];
  const o2n2Theory = values["o2n2-theory"];
  const lowtran = values.lowtran;
  const waterLines = values["h2o-lines"];
  if (
    ozonePath === undefined ||
    methanePath === undefined ||
    so2 === undefined ||
    o2o2 === undefined ||
    h2he === undefined ||
    o2n2 === undefined ||
    o2co2 === undefined ||
    h2h2 === undefined ||
    o2o2Theory === undefined ||
    o2n2Theory === undefined ||
    lowtran === undefined ||
    waterLines === undefined
  ) {
    throw new Error(USAGE);
  }
  const ozone = binOzone(parseOzone(readChecked(ozonePath, OZONE_SOURCE.sha256)));
  const methane = resampleMethane(parseMethane(readChecked(methanePath, METHANE_SOURCE.sha256)));
  const sulphurDioxide = binSulphurDioxide(readSulphurDioxide(so2));
  const theorySets = (path: string, sha256: string): CiaSet[] =>
    labelledSets(parseCia(readChecked(path, sha256)), THEORY_2018_LABEL);
  const oxygen = reducePair(
    [
      ...parseCia(readChecked(o2o2, OXYGEN_PAIR_SOURCE.sha256)),
      ...theorySets(o2o2Theory, OXYGEN_PAIR_THEORY_SOURCE.sha256),
    ],
    OXYGEN_PAIR_REDUCTION,
  );
  const oxygenNitrogen = reducePair(
    [
      ...parseCia(readChecked(o2n2, OXYGEN_NITROGEN_PAIR_SOURCE.sha256)),
      ...theorySets(o2n2Theory, OXYGEN_NITROGEN_PAIR_THEORY_SOURCE.sha256),
    ],
    OXYGEN_NITROGEN_PAIR_REDUCTION,
  );
  const oxygenCarbonDioxide = reducePair(
    parseCia(readChecked(o2co2, OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE.sha256)),
    OXYGEN_CARBON_DIOXIDE_PAIR_REDUCTION,
  );
  const hydrogenHelium = reducePair(
    parseCia(readChecked(h2he, HYDROGEN_HELIUM_PAIR_SOURCE.sha256)),
    HYDROGEN_HELIUM_PAIR_REDUCTION,
  );
  const hydrogen = reduceHydrogenPair(readHydrogenPair(h2h2));
  const waterSource = LINE_SOURCES["H2O"];
  if (waterSource === undefined) {
    throw new Error("the k-distribution tool lists no H2O lines");
  }
  const h2oLines = parseHitranLines(readChecked(waterLines, waterSource.lines.sha256));
  const h2oPartners = Object.fromEntries(
    Object.entries(broadeningRatios(h2oLines)).filter(([gas]) => gas !== "self" && gas !== "H2O"),
  );
  const water = reduceWaterContinuum(
    parseLowtranContinuum(readChecked(lowtran, LOWTRAN_SOURCE.sha256)),
    h2oLines,
    h2oPartners,
  );
  const out = values.out ?? CROSS_SECTIONS_OUTPUT;
  const pairsOut = values["pairs-out"] ?? OXYGEN_PAIRS_OUTPUT;
  writeFileSync(
    out,
    crossSectionsText(
      [ozone.reduced, methane, sulphurDioxide.reduced],
      [hydrogenHelium.reduced, hydrogen, water.self, water.foreign],
    ),
  );
  writeFileSync(
    pairsOut,
    oxygenPairsText([oxygen.reduced, oxygenNitrogen.reduced, oxygenCarbonDioxide.reduced]),
  );
  process.stdout.write(
    `atmosphere-data: wrote ${ozone.reduced.rows.length} ozone rows (${ozone.negatives} negative ` +
      `means as 0), ${methane.rows.length} methane rows, ${sulphurDioxide.reduced.rows.length} ` +
      `sulphur dioxide rows (${sulphurDioxide.negatives} as 0), ${oxygen.reduced.rows.length} ` +
      `O2-O2 rows (${oxygen.negatives} as 0), ${oxygenNitrogen.reduced.rows.length} O2-N2 rows, ` +
      `${oxygenCarbonDioxide.reduced.rows.length} O2-CO2 rows, ` +
      `${hydrogenHelium.reduced.rows.length} H2-He rows (${hydrogenHelium.negatives} as 0), ` +
      `${hydrogen.rows.length} H2-H2 rows and H2O's continuum (CKD over the lines below 567 nm: ` +
      `foreign ${water.foreignRatio.toPrecision(4)}, self ${water.selfRatio.toPrecision(4)}, ` +
      `self 260 K / 296 K ${water.selfColdRatio.toPrecision(4)}) to ${out}, and O2's pairs to ${pairsOut}\n`,
  );
}

/**
 * The four sulphur dioxide files, each identified by its SHA-256 among
 * {@link SULPHUR_DIOXIDE_MEMBERS}.
 *
 * @throws Error for a file that is none of them, or unless each is given once.
 */
function readSulphurDioxide(
  paths: readonly string[],
): ReadonlyArray<{ readonly temperatureK: number; readonly rows: SpectrumRows }> {
  const read = paths.map((path) => {
    const bytes = readFileSync(path);
    const digest = createHash("sha256").update(bytes).digest("hex");
    const member = SULPHUR_DIOXIDE_MEMBERS.find((m) => m.sha256 === digest);
    if (member === undefined) {
      throw new Error(`${path}: SHA-256 ${digest} is none of the snapshot's SO2 files`);
    }
    return {
      temperatureK: member.temperatureK,
      rows: parseTwoColumns(bytes.toString("utf8"), member.name),
    };
  });
  const temperatures = new Set(read.map((r) => r.temperatureK));
  if (read.length !== SULPHUR_DIOXIDE_MEMBERS.length || temperatures.size !== read.length) {
    throw new Error(`give each of the snapshot's ${SULPHUR_DIOXIDE_MEMBERS.length} SO2 files once`);
  }
  return read;
}

/**
 * Borysow's three H₂–H₂ tables, each identified by its SHA-256 among
 * {@link HYDROGEN_PAIR_FILES}, in that list's order.
 *
 * @throws Error for a file that is none of them, or unless each is given once.
 */
function readHydrogenPair(
  paths: readonly string[],
): readonly [BorysowTable, BorysowTable, BorysowTable] {
  const read = paths.map((path) => {
    const bytes = readFileSync(path);
    const digest = createHash("sha256").update(bytes).digest("hex");
    const file = HYDROGEN_PAIR_FILES.find((f) => f.sha256 === digest);
    if (file === undefined) {
      throw new Error(`${path}: SHA-256 ${digest} is none of Borysow's H2-H2 tables`);
    }
    return { file, table: parseBorysow(bytes.toString("latin1"), file.name) };
  });
  const [low, high, hot] = HYDROGEN_PAIR_FILES.map((f) => read.find((r) => r.file === f)?.table);
  if (
    read.length !== HYDROGEN_PAIR_FILES.length ||
    low === undefined ||
    high === undefined ||
    hot === undefined
  ) {
    throw new Error(`give each of Borysow's ${HYDROGEN_PAIR_FILES.length} H2-H2 tables once`);
  }
  return [low, high, hot];
}

/**
 * The command line; writes the reduced file. Returns the exit code, or a promise of it for the
 * subcommands that stream their inputs (`exomol-bands`, `exomol-bins`).
 */
export function main(args: readonly string[]): number | Promise<number> {
  try {
    const [command, ...rest] = args;
    if (command === "exomol-bins" || command === "exomol-bands") {
      // The subcommands that stream their inputs through bzip2, and so run asynchronously.
      return (command === "exomol-bins" ? runExomolBins(rest) : runExomolBands(rest)).then(
        () => 0,
        (error: unknown) => {
          console.error(
            `atmosphere-data: ${error instanceof Error ? error.message : String(error)}`,
          );
          return 1;
        },
      );
    } else if (command === "exomol-k-distributions") {
      runExomolKDistributions(rest);
    } else if (command === "exomol-k-distributions-merge") {
      return runExomolKDistributionsMerge(rest);
    } else if (command === "exomol-cell-references") {
      runExomolCellReferences(rest);
    } else if (command === "matching") {
      runMatching(rest);
    } else if (command === "cross-sections") {
      runCrossSections(rest);
    } else if (command === "k-distributions") {
      runKDistributions(rest);
    } else if (command === "k-distributions-merge") {
      return runKDistributionsMerge(rest);
    } else if (command === "earth-reference") {
      runEarthReference(rest);
    } else if (command === "cell-references") {
      runCellReferences(rest);
    } else if (command === "background-error") {
      runBackgroundError(rest);
    } else if (command === "atomic-shares") {
      runAtomicShares(rest);
    } else if (command === "atomic-bounds") {
      runAtomicBounds(rest);
    } else if (command === "hcl-overtones") {
      runHclOvertones(rest);
    } else {
      throw new Error(USAGE);
    }
    return 0;
  } catch (error: unknown) {
    console.error(`atmosphere-data: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}

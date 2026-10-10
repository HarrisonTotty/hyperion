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
 * `cross-sections --ozone <serdyuchenkogorshelev5digits.dat> --methane <ch4.txt> [--out <json>]`
 * reduces the absorbers' cross-sections (plan R08, R08.T4.b) to
 * `src/renderer/src/view/atmosphere/absorbers/crossSections.json`, over 380–800 nm on vacuum
 * wavelengths, in m² a molecule:
 *
 * - ozone from Serdyuchenko et al. 2014's table at 11 temperatures ({@link OZONE_SOURCE}),
 *   binned to 1 nm, each bin the mean over [λ − 0.5, λ + 0.5) nm (`decisions-r05.md` item 2);
 * - methane from NASA PSG's conversion of Karkoschka and Tomasko 2010 at 100, 198 and 296 K
 *   ({@link METHANE_SOURCE}), interpolated linearly onto a 0.25 nm grid, so that no band-model
 *   value is averaged (`decision-r08-licences.md` row 1).
 *
 * Neither raw file is committed: the ozone page states no terms (`decisions-r05.md` item 4), and
 * PSG's file states none (`decision-r08-licences.md`).
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";

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

/** Where a fetched data file came from, and how it is checked. */
export interface FetchedSource {
  /** The data's citation. */
  readonly citation: string;
  /** The file's URL. */
  readonly url: string;
  /** The file's SHA-256. */
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

/** The file's text: JSON, one wavelength's row a line, every number in its shortest round-trip form. */
export function crossSectionsText(tables: readonly ReducedCrossSections[]): string {
  const description =
    "Absorption cross-sections of the absorbers the client draws, m2 a molecule, on vacuum " +
    "wavelengths (rendering plan R08, R08.T4.b), written by apps/hyperion/scripts/" +
    "atmosphereData.mjs cross-sections from the fetched sources named here, which are not " +
    "committed. Each table's rows are [nm, sigma at each of temperaturesK]. Reduced values; " +
    "the citations are in NOTICE's Data section.";
  const entries = tables.map((table) => {
    const s = table.source;
    const fields = [
      `      "species": ${JSON.stringify(table.species)}`,
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
      `      "rows": [\n${table.rows.map((r) => `        ${line(r)}`).join(",\n")}\n      ]`,
    ];
    return `    {\n${fields.join(",\n")}\n    }`;
  });
  return `{\n  "description": ${JSON.stringify(description)},\n  "tables": [\n${entries.join(",\n")}\n  ]\n}\n`;
}

const USAGE =
  "usage: atmosphereData matching --cie <CIE_xyz_1931_2deg.csv> [--out <json>]\n" +
  "       atmosphereData cross-sections --ozone <serdyuchenkogorshelev5digits.dat> " +
  "--methane <ch4.txt> [--out <json>]";

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
      out: { type: "string" },
    },
  });
  if (values.ozone === undefined || values.methane === undefined) {
    throw new Error(USAGE);
  }
  const ozone = binOzone(parseOzone(readChecked(values.ozone, OZONE_SOURCE.sha256)));
  const methane = resampleMethane(parseMethane(readChecked(values.methane, METHANE_SOURCE.sha256)));
  const out = values.out ?? CROSS_SECTIONS_OUTPUT;
  writeFileSync(out, crossSectionsText([ozone.reduced, methane]));
  process.stdout.write(
    `atmosphere-data: wrote ${ozone.reduced.rows.length} ozone rows (${ozone.negatives} negative ` +
      `means as 0) and ${methane.rows.length} methane rows to ${out}\n`,
  );
}

/** The command line; writes the reduced file. Returns the exit code. */
export function main(args: readonly string[]): number {
  try {
    const [command, ...rest] = args;
    if (command === "matching") {
      runMatching(rest);
    } else if (command === "cross-sections") {
      runCrossSections(rest);
    } else {
      throw new Error(USAGE);
    }
    return 0;
  } catch (error: unknown) {
    console.error(`atmosphere-data: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}

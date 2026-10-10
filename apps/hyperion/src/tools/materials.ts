/**
 * `node scripts/materials.mjs`: reduces the aerosol materials' refractive indices to the files of
 * `view/atmosphere/materials/` (plan R08, R08.T5.b; decision-r08-licences.md rows 2–5,
 * decision-composition §1.9 and §5).
 *
 * @remarks
 * Each fetched source is read from a path given on the command line, checked against the SHA-256
 * recorded here, and never committed. Only its values reduced onto {@link MATERIAL_GRID_NM} are
 * written: the real index n interpolated linearly in wavelength between the source's points, and
 * the imaginary index k linearly in log k (linearly where either end is 0), with each file's own
 * exceptions stated in its `reduction`. Two files are made from no fetched table: CO₂ ice, a
 * derived fit (`science-r08-sulphur-co2ice.md`), and NH₄SH, a stated stand-in (row 5) whose index
 * is a Lorentz–Lorenz estimate (`science-r08-nonspherical.md` §3.1, {@link nh4shIndex}). The files
 * are JSON so that the client imports them and `hyperion-fit` reads them as one source of truth
 * (P14.T49.e). Prettier formats them after they are written (`apps/hyperion/scripts/materials.mjs`).
 *
 * Inputs, each downloaded on 2026-10-09 into its own directory and read as data only:
 *
 * - refractiveindex.info's database (M. N. Polyanskiy, Sci. Data 11, 94 (2024); CC0 1.0), commit
 *   {@link RI_COMMIT}, files under `database/data/`;
 * - optool's `lnk_data/` (C. Dominik, M. Min and R. Tazaki 2021, ascl:2104.010), commit
 *   {@link OPTOOL_COMMIT}; its MIT licence covers its software, and its data files state no terms;
 * - ARIA's Palmer and Williams files (https://eodg.atm.ox.ac.uk/ARIA/; no terms stated);
 * - NASA Ames's Mars dust file as kept in S. Ranjan's repository at commit 1e67a481 (no terms
 *   stated for the data);
 * - HITRAN2024's aerosol archive, `hitran_ri.tar` (https://hitran.org/aerosols/; "free", no
 *   licence stated), SHA-256 {@link HITRAN_TAR_SHA256}, from which only the named ASCII files were
 *   extracted (none of its code was run).
 */

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

/** The wavelengths every material file is given at, nm: 380 to 780 in steps of 5. */
export const MATERIAL_GRID_NM: ReadonlyArray<number> = Array.from(
  { length: 81 },
  (_, i) => 380 + 5 * i,
);

/** The refractiveindex.info database commit the CC0 sources were fetched at. */
export const RI_COMMIT = "c5c2f188e848453def5970e347399d653df2ffc2";

/** The optool commit whose `lnk_data/` the ammonia-ice and enstatite-glass sources come from. */
export const OPTOOL_COMMIT = "0b1fa6df95e3a694dc64abdbf8e36dad127165ab";

/** The SHA-256 of HITRAN2024's `hitran_ri.tar` (207,422,976 bytes), recorded at the first fetch. */
export const HITRAN_TAR_SHA256 = "c22643a856ec98b3371c70e64351b19a9f3ea51b38ce369087e17e7e4ecc231e";

/** The day every source was fetched. */
export const FETCHED = "2026-10-09";

/** One row of a source table: the wavelength, µm, and the complex index n + ik (k may be NaN). */
export interface IndexSample {
  readonly wavelengthUm: number;
  readonly n: number;
  readonly k: number;
}

/** The phase a material file describes. */
export type FilePhase = "liquid" | "solid";

/** The shape class of a material's particles (Design note 6; P14.T24.c's closed physics enum). */
export type FileShape = "sphere" | "nonSphericalMineral" | "crystal" | "aggregate";

/** How a file's values were obtained (decision-composition §1.9). */
export type FileProvenance = "measured" | "derived" | "standIn";

/** The header of a material file: every field but the three arrays. */
export interface MaterialHeader {
  /** The sim's substance key (P14.T49.b). */
  readonly key: string;
  readonly phase: FilePhase;
  /** A composition variant (an acid's concentration), or null for the key's only one. */
  readonly variant: string | null;
  /** What the sample is, in words. */
  readonly name: string;
  /** The sample's temperature, K, or null where the source does not state one. */
  readonly temperatureK: number | null;
  readonly shape: FileShape;
  readonly provenance: FileProvenance;
  /** The paper the values come from. */
  readonly paper: string;
  /** Where the values were read, with the URL, the checksum and the date fetched. */
  readonly source: string;
  /** The licence basis (decision-composition §5). */
  readonly licence: string;
  /** How the values were brought onto the grid, and every exception. */
  readonly reduction: string;
  /** For a stand-in, what stands in and why; otherwise null. */
  readonly standIn: string | null;
  /** For a derived file, the parameterisation its values are computed from; otherwise null. */
  readonly fit: CauchyFit | null;
}

/**
 * A derived index: n = a + b ÷ λ² (λ in µm) and k = αλ ÷ 4π, over the wavelengths it was fitted
 * on.
 */
export interface CauchyFit {
  readonly form: "cauchy";
  readonly a: number;
  readonly bUm2: number;
  /** α, m⁻¹. */
  readonly absorptionPerM: number;
  /** The range of the data the fit was made on, µm. */
  readonly fittedFromUm: number;
  readonly fittedToUm: number;
}

/** A material file as written: its header and the index on {@link MATERIAL_GRID_NM}. */
export interface MaterialFile extends MaterialHeader {
  readonly wavelengthsNm: ReadonlyArray<number>;
  readonly n: ReadonlyArray<number>;
  readonly k: ReadonlyArray<number>;
}

const NUMBER = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/u;

function numeric(field: string): number {
  if (field === "NaN") {
    return Number.NaN;
  }
  if (!NUMBER.test(field)) {
    throw new Error(`"${field}" is not a number`);
  }
  return Number(field);
}

/**
 * The rows a reduction reads, by wavelength: from the last at or below the grid's first wavelength
 * to the first at or above its last, so that a source's rows elsewhere (an infrared band printed
 * with rounded wavelengths) cannot stop it. Exact duplicate rows are dropped, and two values at one
 * wavelength inside the window are refused.
 */
function usedRows(samples: ReadonlyArray<IndexSample>, ignoreBelowUm: number): IndexSample[] {
  const sorted = samples
    .filter((s) => s.wavelengthUm >= ignoreBelowUm)
    .toSorted((a, b) => a.wavelengthUm - b.wavelengthUm);
  const lowUm = (MATERIAL_GRID_NM[0] ?? Number.NaN) / 1000;
  const highUm = (MATERIAL_GRID_NM.at(-1) ?? Number.NaN) / 1000;
  const start = Math.max(
    0,
    sorted.findLastIndex((s) => s.wavelengthUm <= lowUm),
  );
  const after = sorted.findIndex((s) => s.wavelengthUm >= highUm);
  const window = sorted.slice(start, after < 0 ? sorted.length : after + 1);
  const out: IndexSample[] = [];
  for (const row of window) {
    const last = out.at(-1);
    if (last !== undefined && last.wavelengthUm === row.wavelengthUm) {
      if (last.n === row.n && Object.is(last.k, row.k)) {
        continue;
      }
      throw new Error(`resample: two rows at ${row.wavelengthUm} µm`);
    }
    out.push(row);
  }
  if (out.length < 2) {
    throw new Error("resample: fewer than two rows near the grid");
  }
  return out;
}

/**
 * The rows of a refractiveindex.info database file with one `tabulated nk` block: λ (µm), n, k.
 *
 * @throws Error if the file has no such block, or more than one data block.
 */
export function parseRefractiveIndexInfo(text: string): IndexSample[] {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim() === "- type: tabulated nk");
  if (start < 0 || lines.filter((line) => line.includes("type:")).length !== 1) {
    throw new Error("refractiveindex.info: expected exactly one `tabulated nk` block");
  }
  const rows: IndexSample[] = [];
  for (const line of lines.slice(start + 2)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3 || !NUMBER.test(fields[0] ?? "")) {
      break;
    }
    rows.push({
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/**
 * The rows of an ARIA `.ri` file in `#FORMAT=WAVN N K`: wavenumber (cm⁻¹), n, k (k may be NaN).
 *
 * @throws Error if the format line is missing, or a data line is not three numbers.
 */
export function parseAria(text: string): IndexSample[] {
  const lines = text.split(/\r?\n/u);
  if (!lines.some((line) => line.trim() === "#FORMAT=WAVN N K")) {
    throw new Error("ARIA: expected #FORMAT=WAVN N K");
  }
  const rows: IndexSample[] = [];
  for (const [i, line] of lines.entries()) {
    if (line.startsWith("#") || line.trim().length === 0) {
      continue;
    }
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3) {
      throw new Error(`ARIA: line ${i + 1} has ${fields.length} fields`);
    }
    rows.push({
      wavelengthUm: 1e4 / numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/**
 * The rows of an optool `.lnk` file: `#` comments, a count and density line, then λ (µm), n, k.
 *
 * @throws Error if the count disagrees with the rows, or a row is not three numbers.
 */
export function parseLnk(text: string): IndexSample[] {
  const data = text
    .split(/\r?\n/u)
    .filter((line) => !line.startsWith("#") && line.trim().length > 0)
    .map((line) => line.trim().split(/\s+/u));
  const [head, ...body] = data;
  const count = head === undefined ? Number.NaN : numeric(head[0] ?? "");
  if (head?.length !== 2 || count !== body.length) {
    throw new Error(`lnk: the count line says ${count} rows, found ${body.length}`);
  }
  const rows = body.map((fields, i) => {
    if (fields.length !== 3) {
      throw new Error(`lnk: row ${i + 1} has ${fields.length} fields`);
    }
    return {
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    };
  });
  return rows;
}

/**
 * The rows of a HITRAN-RI ASCII table in four columns: wavenumber (cm⁻¹), λ (µm), n, k. Lines that
 * are not four numbers (the header) are skipped.
 */
export function parseHitranTable(text: string): IndexSample[] {
  const rows: IndexSample[] = [];
  for (const line of text.split(/\r?\n/u)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 4 || !fields.every((f) => NUMBER.test(f))) {
      continue;
    }
    rows.push({
      wavelengthUm: numeric(fields[1] ?? ""),
      n: numeric(fields[2] ?? ""),
      k: numeric(fields[3] ?? ""),
    });
  }
  return rows;
}

/** The rows of a table in three columns, λ (µm), n, k, after a header line that is not numbers. */
export function parseWavelengthTable(text: string): IndexSample[] {
  const rows: IndexSample[] = [];
  for (const line of text.split(/\r?\n/u)) {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 3 || !fields.every((f) => NUMBER.test(f))) {
      continue;
    }
    rows.push({
      wavelengthUm: numeric(fields[0] ?? ""),
      n: numeric(fields[1] ?? ""),
      k: numeric(fields[2] ?? ""),
    });
  }
  return rows;
}

/** How one source is brought onto the grid. */
export interface ResampleOptions {
  /**
   * Source rows below this wavelength, µm, are not used: past a gap across an absorption edge, an
   * interpolation would carry ultraviolet values into the visible.
   */
  readonly ignoreBelowUm?: number;
  /**
   * How far beyond the source's first or last used row, nm, its end value may be held; a grid
   * point farther out is an error.
   */
  readonly holdNm: number;
  /**
   * Where the source gives k as NaN (no measurement), it takes the k of the shortest wavelength
   * that has one. Every NaN must lie at shorter wavelengths than that row.
   */
  readonly unmeasuredK?: "shortestMeasured";
}

function interpolateK(k0: number, k1: number, t: number): number {
  return k0 > 0 && k1 > 0 ? k0 * (k1 / k0) ** t : k0 + t * (k1 - k0);
}

/** Six significant figures, past what any source measures. */
function rounded(value: number): number {
  return Number(value.toPrecision(6));
}

/**
 * A source reduced onto {@link MATERIAL_GRID_NM}: n linear and k log-linear in wavelength.
 *
 * @throws Error if two rows near the grid disagree at one wavelength, a grid point lies farther
 *   beyond the source than `holdNm`, a row has no finite n > 0 and k ≥ 0, or an unmeasured k
 *   cannot be set.
 */
export function resample(
  samples: ReadonlyArray<IndexSample>,
  options: ResampleOptions,
): { readonly n: number[]; readonly k: number[] } {
  let rows = usedRows(samples, options.ignoreBelowUm ?? 0);
  if (options.unmeasuredK === "shortestMeasured") {
    const first = rows.find((s) => Number.isFinite(s.k));
    if (first === undefined) {
      throw new Error("resample: no measured k");
    }
    rows = rows.map((s) => {
      if (Number.isFinite(s.k)) {
        return s;
      }
      if (s.wavelengthUm > first.wavelengthUm) {
        throw new Error(`resample: k unmeasured at ${s.wavelengthUm} µm, past the first measured`);
      }
      return { ...s, k: first.k };
    });
  }
  if (rows.some((s) => !Number.isFinite(s.n) || !Number.isFinite(s.k) || s.n <= 0 || s.k < 0)) {
    throw new Error("resample: a row has no finite n > 0 and k ≥ 0");
  }
  const first = rows[0];
  const last = rows.at(-1);
  if (first === undefined || last === undefined) {
    throw new Error("resample: no rows");
  }
  const n: number[] = [];
  const k: number[] = [];
  for (const nm of MATERIAL_GRID_NM) {
    const um = nm / 1000;
    if (um <= first.wavelengthUm || um >= last.wavelengthUm) {
      const end = um <= first.wavelengthUm ? first : last;
      const gapNm = Math.abs(end.wavelengthUm - um) * 1000;
      if (gapNm > options.holdNm + 1e-9) {
        throw new Error(`resample: ${nm} nm lies ${gapNm.toFixed(1)} nm beyond the source`);
      }
      n.push(rounded(end.n));
      k.push(rounded(end.k));
      continue;
    }
    const upper = rows.findIndex((s) => s.wavelengthUm >= um);
    const hi = rows[upper];
    const lo = rows[upper - 1];
    if (hi === undefined || lo === undefined) {
      throw new Error(`resample: no bracket at ${nm} nm`);
    }
    const t = (um - lo.wavelengthUm) / (hi.wavelengthUm - lo.wavelengthUm);
    n.push(rounded(lo.n + t * (hi.n - lo.n)));
    k.push(rounded(interpolateK(lo.k, hi.k, t)));
  }
  return { n, k };
}

const RI_URL = `https://raw.githubusercontent.com/polyanskiy/refractiveindex.info-database/${RI_COMMIT}/database/data`;
const OPTOOL_URL = `https://raw.githubusercontent.com/cdominik/optool/${OPTOOL_COMMIT}/lnk_data`;
const RI_LICENCE =
  "CC0 1.0 (the refractiveindex.info database, M. N. Polyanskiy, Sci. Data 11, 94 (2024)); reduced values";
const NO_TERMS = "reduced values; the source states no terms (decision-r08-licences.md)";
const OPTOOL_TERMS =
  "reduced values; the source states no terms for its data, and optool's MIT licence covers its software (decision-r08-licences.md)";
const STANDARD_REDUCTION =
  "n linear and k linear in log k in wavelength between the source's rows, onto 380–780 nm every 5 nm, to six significant figures";

/** A material file reduced from one fetched source. */
interface FetchedSpec {
  readonly file: string;
  /** The command-line option that names the source's path. */
  readonly option: string;
  readonly sha256: string;
  readonly parse: (text: string) => IndexSample[];
  readonly resample: ResampleOptions;
  readonly header: MaterialHeader;
}

function fetchedSource(what: string, url: string, sha256: string): string {
  return `${what}, ${url}, SHA-256 ${sha256}, fetched ${FETCHED}`;
}

/** Every material file reduced from a fetched source, in the registry's order (a key's default first). */
export const FETCHED_MATERIALS: ReadonlyArray<FetchedSpec> = [
  {
    file: "water.json",
    option: "water",
    sha256: "df1af6b4352c3378cf81b149ac2280de30d80e2941d147786d5e7cd5044a8847",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "H2O",
      phase: "liquid",
      variant: null,
      name: "liquid water at 25 °C",
      temperatureK: 298,
      shape: "sphere",
      provenance: "measured",
      paper:
        'G. M. Hale and M. R. Querry, "Optical constants of water in the 200-nm to 200-µm wavelength region", Appl. Opt. 12 (1973) 555–563, DOI 10.1364/AO.12.000555',
      source: fetchedSource(
        "refractiveindex.info, main/H2O/nk/Hale.yml",
        `${RI_URL}/main/H2O/nk/Hale.yml`,
        "df1af6b4352c3378cf81b149ac2280de30d80e2941d147786d5e7cd5044a8847",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; the source's rows are every 25 nm`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "water-ice.json",
    option: "ice",
    sha256: "a8577eaa1fabb9245957397344ce23efb20dd4c57cd7f442710d2d2373e0bbd6",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "H2O",
      phase: "solid",
      variant: null,
      name: "water ice Ih at −7 °C",
      temperatureK: 266,
      shape: "crystal",
      provenance: "measured",
      paper:
        'S. G. Warren and R. E. Brandt, "Optical constants of ice from the ultraviolet to the microwave: A revised compilation", J. Geophys. Res. 113 (2008) D14220, DOI 10.1029/2007JD009744',
      source: fetchedSource(
        "refractiveindex.info, main/H2O/nk/Warren-2008.yml",
        `${RI_URL}/main/H2O/nk/Warren-2008.yml`,
        "a8577eaa1fabb9245957397344ce23efb20dd4c57cd7f442710d2d2373e0bbd6",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; the source's rows are every 10 nm from 390 nm`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "ammonia-ice.json",
    option: "ammonia",
    sha256: "14a5b66bb35101a89cacb07cd80585413ee4aa8986d966138d866ffd4fb9babd",
    parse: parseLnk,
    resample: { holdNm: 0 },
    header: {
      key: "NH3",
      phase: "solid",
      variant: null,
      name: "crystalline ammonia ice",
      temperatureK: null,
      shape: "crystal",
      provenance: "measured",
      paper:
        'J. V. Martonchik, G. S. Orton and J. F. Appleby, "Optical properties of NH3 ice from the far infrared to the near ultraviolet", Appl. Opt. 23 (1984) 541–547, DOI 10.1364/AO.23.000541',
      source: fetchedSource(
        "optool, lnk_data/nh3-m-Martonchik1983.lnk",
        `${OPTOOL_URL}/nh3-m-Martonchik1983.lnk`,
        "14a5b66bb35101a89cacb07cd80585413ee4aa8986d966138d866ffd4fb9babd",
      ),
      licence: OPTOOL_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source has rows at 320, 433, 668 and 1,462 nm here, so the visible is interpolated between them; the copy states no temperature. The paper's visible k comes from earlier published spectra (its abstract), not read here`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "methane-liquid.json",
    option: "methane-liquid",
    sha256: "75b1cb8ae81a6afed2c597b524bcc16f89492eb9fba4b744813ce5cde9600d67",
    parse: parseRefractiveIndexInfo,
    resample: { ignoreBelowUm: 0.4, holdNm: 20 },
    header: {
      key: "CH4",
      phase: "liquid",
      variant: null,
      name: "liquid methane at 90 K",
      temperatureK: 90,
      shape: "sphere",
      provenance: "measured",
      paper:
        'J. V. Martonchik and G. S. Orton, "Optical constants of liquid and solid methane", Appl. Opt. 33 (1994) 8306–8317, DOI 10.1364/AO.33.008306',
      source: fetchedSource(
        "refractiveindex.info, organic/CH4 - methane/nk/Martonchik-liquid-90K.yml",
        `${RI_URL}/organic/CH4%20-%20methane/nk/Martonchik-liquid-90K.yml`,
        "75b1cb8ae81a6afed2c597b524bcc16f89492eb9fba4b744813ce5cde9600d67",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}. The source has no row between 133.7 and 400 nm, across methane's ultraviolet absorption edge, so its rows below 400 nm are not used and its 400 nm values (n 1.300, k 0) are held over 380–400 nm. Its visible k rows sit at band maxima and minima (the database's caution), so k between them is indicative`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "methane-ice.json",
    option: "methane-ice",
    sha256: "6fb859140db36f1039d5c64aafb7cb4d16de7db18b37a9f414f9c061193acfcd",
    parse: parseRefractiveIndexInfo,
    resample: { ignoreBelowUm: 0.4, holdNm: 20 },
    header: {
      key: "CH4",
      phase: "solid",
      variant: null,
      name: "phase I solid methane at 90 K",
      temperatureK: 90,
      shape: "crystal",
      provenance: "measured",
      paper:
        'J. V. Martonchik and G. S. Orton, "Optical constants of liquid and solid methane", Appl. Opt. 33 (1994) 8306–8317, DOI 10.1364/AO.33.008306',
      source: fetchedSource(
        "refractiveindex.info, organic/CH4 - methane/nk/Martonchik-solid-90K.yml",
        `${RI_URL}/organic/CH4%20-%20methane/nk/Martonchik-solid-90K.yml`,
        "6fb859140db36f1039d5c64aafb7cb4d16de7db18b37a9f414f9c061193acfcd",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}. The source's visible rows are 400 nm and 1,000 nm, both with k 0, and none between 133.7 and 400 nm, so its rows below 400 nm are not used and its 400 nm values (n 1.326, k 0) are held over 380–400 nm. Its k of 0 is a gap in the source, not a measurement: the liquid shows bands of k about 10⁻⁷ at 620–730 nm, which for ice grains of 10 µm would move 1 − ω by about 10⁻⁵`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "sulphuric-acid-75.json",
    option: "h2so4-75",
    sha256: "ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe",
    parse: parseAria,
    resample: { holdNm: 0, unmeasuredK: "shortestMeasured" },
    header: {
      key: "H2SO4",
      phase: "liquid",
      variant: "75wt%",
      name: "75 wt% aqueous sulphuric acid at 300 K",
      temperatureK: 300,
      shape: "sphere",
      provenance: "measured",
      paper:
        'K. F. Palmer and D. Williams, "Optical constants of sulfuric acid; application to the clouds of Venus?", Appl. Opt. 14 (1975) 208–219, DOI 10.1364/AO.14.000208',
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, H2SO4_75%_300K_R_Palmer_1975.ri",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Acids/Sulphuric/70%25_to_79%25/Sulphuric_acid_75%25_300K_(Palmer_and_Williams_1975)/original/H2SO4_75%25_300K_R_Palmer_1975.ri",
        "ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe",
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}. In the visible the source gives n only, at 359.7, 408.2, 449.4, 555.6 and 701.8 nm, with k NaN short of 701.8 nm; there k is held at its shortest measured value, 2.07 × 10⁻⁸ at 701.8 nm. The acid's visible absorption lies below the source's measurement, and at this k a Venus mode-2 droplet's 1 − ω is about 10⁻⁶. n at 555.6 nm, 1.431, lies inside Hansen and Hovenier 1974's 1.44 ± 0.015 at 550 nm (J. Atmos. Sci. 31, 1137)`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "sulphuric-acid-84.json",
    option: "h2so4-84",
    sha256: "6ae3eceb2c3895ec367f9b86e8e082445f1473413d224c2b625d85baaf452dc3",
    parse: parseAria,
    resample: { holdNm: 0, unmeasuredK: "shortestMeasured" },
    header: {
      key: "H2SO4",
      phase: "liquid",
      variant: "84.5wt%",
      name: "84.5 wt% aqueous sulphuric acid at 300 K",
      temperatureK: 300,
      shape: "sphere",
      provenance: "measured",
      paper:
        'K. F. Palmer and D. Williams, "Optical constants of sulfuric acid; application to the clouds of Venus?", Appl. Opt. 14 (1975) 208–219, DOI 10.1364/AO.14.000208',
      source: fetchedSource(
        "the Aerosol Refractive Index Archive (ARIA), University of Oxford, H2SO4_84.5%_300K_R_Palmer_1975.ri",
        "https://eodg.atm.ox.ac.uk/ARIA/data_files/Acids/Sulphuric/80%25_to_100%25/Sulphuric_acid_84.5%25_300K_(Palmer_and_Williams_1975)/original/H2SO4_84.5%25_300K_R_Palmer_1975.ri",
        "6ae3eceb2c3895ec367f9b86e8e082445f1473413d224c2b625d85baaf452dc3",
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}. In the visible the source gives n only, at 359.7, 408.2, 449.4, 555.6 and 701.8 nm, with k NaN short of 714.3 nm; there k is held at its shortest measured value, 1.14 × 10⁻⁸ at 714.3 nm, the acid's visible absorption lying below the source's measurement. n at 555.6 nm, 1.438, lies inside Hansen and Hovenier 1974's 1.44 ± 0.015 at 550 nm`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "iron.json",
    option: "iron",
    sha256: "8d774beaac808662370242790c18b9998a9856ba850d9f35c45f6db3da31a7cd",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 0 },
    header: {
      key: "Fe",
      phase: "solid",
      variant: null,
      name: "metallic iron films at room temperature",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'P. B. Johnson and R. W. Christy, "Optical constants of transition metals: Ti, V, Cr, Mn, Fe, Co, Ni, and Pd", Phys. Rev. B 9 (1974) 5056–5070, DOI 10.1103/PhysRevB.9.5056',
      source: fetchedSource(
        "refractiveindex.info, main/Fe/nk/Johnson.yml",
        `${RI_URL}/main/Fe/nk/Johnson.yml`,
        "8d774beaac808662370242790c18b9998a9856ba850d9f35c45f6db3da31a7cd",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}; room-temperature solid iron, so a liquid iron cloud takes it as a named analogue (a stand-in), drawn as spheres, while solid grains are non-spherical minerals (science-r08-nonspherical.md §2.2); 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "soot.json",
    option: "soot",
    sha256: "3fb9dd0dfdb19eea4b0fa52c1076df5e9fa0be9473ad2f456c5114c572b7a256",
    parse: parseRefractiveIndexInfo,
    resample: { holdNm: 56 },
    header: {
      key: "soot",
      phase: "solid",
      variant: null,
      name: "propane soot at room temperature",
      temperatureK: 295,
      shape: "sphere",
      provenance: "measured",
      paper:
        'W. H. Dalzell and A. F. Sarofim, "Optical constants of soot and their application to heat-flux calculations", J. Heat Transfer 91 (1969) 100–104, DOI 10.1115/1.3580063',
      source: fetchedSource(
        "refractiveindex.info, other/soots/propane soot/nk/Dalzell.yml",
        `${RI_URL}/other/soots/propane%20soot/nk/Dalzell.yml`,
        "3fb9dd0dfdb19eea4b0fa52c1076df5e9fa0be9473ad2f456c5114c572b7a256",
      ),
      licence: RI_LICENCE,
      reduction: `${STANDARD_REDUCTION}. The source's visible rows are 435.8, 450, 550, 650 and 806.5 nm, so its 435.8 nm values (n 1.57, k 0.46) are held over 380–435.8 nm; soot is nearly grey across the visible (n 1.56–1.57, k 0.46–0.53 in the source); 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "tholin.json",
    option: "tholin",
    sha256: "8cef815bfcf19123b27dc061bc3a92377a5ad2440d9ba3a0823d19427e0198b6",
    parse: parseHitranTable,
    resample: { holdNm: 0 },
    header: {
      key: "tholin",
      phase: "solid",
      variant: null,
      name: "Titan tholin, a laboratory haze analogue from a simulated Titan atmosphere",
      temperatureK: 295,
      shape: "sphere",
      provenance: "measured",
      paper:
        'B. N. Khare, C. Sagan, E. T. Arakawa, F. Suits, T. A. Callcott and M. W. Williams, "Optical constants of organic tholins produced in a simulated Titanian atmosphere: From soft x-ray to microwave frequencies", Icarus 60 (1984) 127–137, DOI 10.1016/0019-1035(84)90142-8',
      source: `HITRAN2024's aerosol compilation (I. E. Gordon et al., J. Quant. Spectrosc. Radiat. Transfer 353 (2026) 109807), https://hitran.org/data/Aerosols/Aerosols-2024/hitran_ri.tar, SHA-256 ${HITRAN_TAR_SHA256}, its file hitran_ri/ascii/exoplanets/khare_tholins.dat, SHA-256 8cef815bfcf19123b27dc061bc3a92377a5ad2440d9ba3a0823d19427e0198b6, fetched ${FETCHED}; only this ASCII file was read, and none of the archive's code was run`,
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows here are 354.2, 387.4, 413.3, 442.8, 563.5, 688.8 and 873.1 nm. Khare et al. 1984 is taken over He et al. 2022 (Planet. Sci. J. 3, 25; CC BY 4.0), whose data begin at 400 nm and so do not cover 380–400 nm; 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "mars-dust.json",
    option: "mars-dust",
    sha256: "bf6069ad22fbfbbef7bf957d386a8177647c87fd98ed6574b5c5f8d6c8016769",
    parse: parseWavelengthTable,
    resample: { holdNm: 0 },
    header: {
      key: "mars_dust",
      phase: "solid",
      variant: null,
      name: "Martian atmospheric dust, indices retrieved from CRISM and MER observations",
      temperatureK: null,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'M. J. Wolff et al., "Wavelength dependence of dust aerosol single scattering albedo as observed by the Compact Reconnaissance Imaging Spectrometer", J. Geophys. Res. 114 (2009) E00D04, DOI 10.1029/2009JE003350',
      source: fetchedSource(
        "NASA Ames Mars Climate Modeling Group's Dust_Refractive_Indicies.txt, as kept in S. Ranjan's ranjanwordsworthsasselov2017b repository (commit 1e67a481; its MIT licence covers its code, not these data)",
        "https://raw.githubusercontent.com/sukritranjan/ranjanwordsworthsasselov2017b/1e67a4819255ff907df2a97eb8cecd9eee32c928/Raw_Data/ComplexRefractionIndices/Dust_Refractive_Indicies.txt",
        "bf6069ad22fbfbbef7bf957d386a8177647c87fd98ed6574b5c5f8d6c8016769",
      ),
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows used are 321, 440, 460, 500, 600, 630, 700 and 800 nm, so 380–440 nm is interpolated between its 321 and 440 nm rows, the 321 nm row being the anchor there (science-r08-nonspherical.md §3.2, ruled 2026-10-10). The indices are retrieved, not laboratory: Wolff et al. fit them to observed single-scattering albedos with non-spherical (T-matrix cylinder) particles, so Mie gives this dust's cross-sections only (Design note 6). The source's two ultraviolet rows, 263 and 321 nm, lie outside Wolff et al. 2009's CRISM range (440–2,920 nm): they are part of Wolff's distributed compilation but separately retrieved (k printed to three figures, where the CRISM rows have six), presumably Wolff et al. 2010's MARCI values (Icarus 208, 143), unconfirmed, since that paper is closed. The 263 nm row lies outside the grid and is not used. The interpolated k at 380 nm, 0.00989, lies between holding the 440 nm row (0.00767), which is wrong because Mars dust's k rises towards the ultraviolet, and extrapolating the 440–500 nm rows' log-slope (0.0107)`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "enstatite-glass.json",
    option: "enstatite",
    sha256: "9f07f23cb0b233fd48210824c38afe031fd5ad84bd0b182b7fe290caa1fe0de8",
    parse: parseLnk,
    resample: { holdNm: 0 },
    header: {
      key: "MgSiO3",
      phase: "solid",
      variant: null,
      name: "amorphous MgSiO3 (pyroxene glass, Mg fraction 1.0)",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'J. Dorschner, B. Begemann, Th. Henning, C. Jäger and H. Mutschke, "Steps toward interstellar silicate mineralogy. II. Study of Mg-Fe-silicate glasses of variable composition", Astron. Astrophys. 300 (1995) 503–520',
      source: fetchedSource(
        "optool, lnk_data/pyr-mg100-Dorschner1995.lnk",
        `${OPTOOL_URL}/pyr-mg100-Dorschner1995.lnk`,
        "9f07f23cb0b233fd48210824c38afe031fd5ad84bd0b182b7fe290caa1fe0de8",
      ),
      licence: OPTOOL_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are every 20–50 nm here, and its placeholder k of 10⁻⁶ below 320 nm lies outside the grid; 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
  {
    file: "forsterite-amorphous.json",
    option: "forsterite",
    sha256: "2582d4ab3cd9f7c9f8e53ae9988bd71a13c7080ba68f858bdf60133536c2fa32",
    parse: parseHitranTable,
    resample: { holdNm: 0 },
    header: {
      key: "Mg2SiO4",
      phase: "solid",
      variant: null,
      name: "amorphous Mg2SiO4 made by the sol-gel method",
      temperatureK: 295,
      shape: "nonSphericalMineral",
      provenance: "measured",
      paper:
        'C. Jäger, J. Dorschner, H. Mutschke, Th. Posch and Th. Henning, "Steps toward interstellar silicate mineralogy. VII. Spectral properties and crystallization behaviour of magnesium silicates produced by the sol-gel method", Astron. Astrophys. 408 (2003) 193–204, DOI 10.1051/0004-6361:20030916',
      source: `HITRAN2024's aerosol compilation (I. E. Gordon et al., J. Quant. Spectrosc. Radiat. Transfer 353 (2026) 109807), https://hitran.org/data/Aerosols/Aerosols-2024/hitran_ri.tar, SHA-256 ${HITRAN_TAR_SHA256}, its file hitran_ri/ascii/exoplanets/jager_mg2sio4.dat, SHA-256 2582d4ab3cd9f7c9f8e53ae9988bd71a13c7080ba68f858bdf60133536c2fa32, fetched ${FETCHED}; only this ASCII file was read, and none of the archive's code was run`,
      licence: NO_TERMS,
      reduction: `${STANDARD_REDUCTION}; the source's rows are about 0.1 nm apart, and its k is printed to four decimal places (10⁻⁴), so k here is that coarse. Dorschner et al. 1995 has no Mg-pure olivine glass, so this Mg2SiO4 is Jäger et al.'s, as Kitzmann and Heng 2018's compilation also takes it (MNRAS 475, 94); 295 K stands for the source's room temperature`,
      standIn: null,
      fit: null,
    },
  },
];

/**
 * CO₂ ice's derived index (`science-r08-sulphur-co2ice.md`): the real part a two-term Cauchy fit to
 * Warren 1986's Table I over 0.30–1.10 µm, and α = 10⁻² m⁻¹, Hansen 2005's estimated visible
 * absorption, below that paper's detection limit over 0.25–1.0 µm (JGR 110, E11003; its abstract).
 */
export const CO2_ICE_FIT: CauchyFit = {
  form: "cauchy",
  a: 1.3994,
  bUm2: 0.004312,
  absorptionPerM: 1e-2,
  fittedFromUm: 0.3,
  fittedToUm: 1.1,
};

/** A derived index at a wavelength, nm: n = a + b ÷ λ², k = αλ ÷ 4π. */
export function cauchyIndex(
  fit: CauchyFit,
  wavelengthNm: number,
): { readonly n: number; readonly k: number } {
  const um = wavelengthNm / 1000;
  return {
    n: fit.a + fit.bUm2 / (um * um),
    k: (fit.absorptionPerM * wavelengthNm * 1e-9) / (4 * Math.PI),
  };
}

/**
 * A gas's measured refractivity in the one-term form n − 1 = A ÷ (B − λ⁻²), λ the vacuum wavelength
 * in µm, stated at 0 °C and 760 mm.
 */
export interface OneTermRefractivity {
  /** A, µm⁻². */
  readonly aPerUm2: number;
  /** B, µm⁻². */
  readonly bPerUm2: number;
  /**
   * The compressibility factor Z of the gas the formula describes, at 0 °C and 760 mm: its molar
   * volume is Z times the ideal gas's.
   */
  readonly compressibility: number;
  readonly source: string;
}

/**
 * NH₃'s refractivity: C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1, p. 22,
 * n − 1 = 0.032 953 ÷ (90.392 − λ⁻²), measured over 480–670.8 nm, with Z = 0.984 798 × 0.7708 ÷
 * 0.7605 = 0.998 14. The same row as `view/atmosphere/rayleigh.ts`'s `GAS_DISPERSION.NH3`, which
 * `view/atmosphere/materials/materials.test.ts` checks.
 *
 * @remarks
 * The paper reduces every refractivity to its "theoretic density" (p. 5): for ammonia (p. 21) it
 * multiplies the real gas's refractivity at 0 °C and 760 mm by 0.7605 ÷ 0.7708, the theoretic over
 * the measured weight of a litre, g. So the formula's gas is all but ideal, and Z is the real gas's,
 * NIST's 0.984 798, over that factor (`rayleigh.ts` holds the reading).
 */
export const NH3_REFRACTIVITY: OneTermRefractivity = {
  aPerUm2: 0.032_953,
  bPerUm2: 90.392,
  compressibility: (0.984_798 * 0.770_8) / 0.760_5,
  source: "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1",
};

/**
 * H₂S's refractivity: C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171, p. 174,
 * n − 1 = 0.053 711 ÷ (86.756 − λ⁻²), measured over 486.1–656.3 nm, at the number density of
 * hydrogen at 0 °C and 760 mm, with Z = 0.991 571 ÷ 0.990 92 = 1.000 66. The same row as
 * `view/atmosphere/rayleigh.ts`'s `GAS_DISPERSION.H2S`, which
 * `view/atmosphere/materials/materials.test.ts` checks.
 */
export const H2S_REFRACTIVITY: OneTermRefractivity = {
  aPerUm2: 0.053_711,
  bPerUm2: 86.756,
  compressibility: 0.991_571 / 0.990_92,
  source: "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171",
};

/**
 * HCl's refractivity, for NH₄Cl's increment ({@link NH4CL_IONIC_INCREMENT}): C. and M. Cuthbertson,
 * Phil. Trans. R. Soc. Lond. A 213 (1914) 1, p. 12, (μ − 1) D ÷ (d₀76) = 4.6425 × 10²⁷ ÷ (10,664 ×
 * 10²⁷ − n²), n = 3 × 10¹⁰ ÷ λ cm s⁻¹ as the paper's frequencies take it: n − 1 = 0.051 583 ÷
 * (118.49 − λ⁻²), measured over 480–670.8 nm.
 *
 * @remarks
 * The same paper as NH₃'s row, so the two gases' reductions match. The paper reduces HCl to the
 * number density of hydrogen at 0 °C and 76 cm through Gray and Burt's volume ratio, 1.0079, and
 * Leduc's expansion coefficient (p. 11), so Z is NIST's for H₂ there, 1.000 624. Reading Gray and
 * Burt's ratio against NIST's H₂ at 16 °C instead gives 0.999 46, and the increment +3.58%, 6.5 ×
 * 10⁻⁴ more.
 */
export const HCL_REFRACTIVITY: OneTermRefractivity = {
  aPerUm2: 0.051_583,
  bPerUm2: 118.49,
  compressibility: 1.000_624,
  source: "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1",
};

/** The Avogadro constant, mol⁻¹, exact in the 2019 SI. */
const AVOGADRO_PER_MOL = 6.022_140_76e23;

/** The Boltzmann constant, J K⁻¹, exact in the 2019 SI. */
const BOLTZMANN_J_PER_K = 1.380_649e-23;

/**
 * The ideal gas's molar volume at 0 °C and 101,325 Pa, cm³ mol⁻¹: N_A k_B × 273.15 K ÷ 101,325 Pa,
 * exact in the 2019 SI (CODATA's 22.413 969 54 × 10⁻³ m³ mol⁻¹).
 */
export const IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL =
  ((AVOGADRO_PER_MOL * BOLTZMANN_J_PER_K * 273.15) / 101_325) * 1e6;

/**
 * The molar volume of a crystal, cm³ mol⁻¹, from its unit cell's volume in Å³ and the formula units
 * the cell holds.
 */
export function cellMolarVolumeCm3PerMol(cellVolumeA3: number, formulaUnits: number): number {
  return (AVOGADRO_PER_MOL * cellVolumeA3 * 1e-24) / formulaUnits;
}

/**
 * NH₄SH's molar volume, cm³ mol⁻¹, from its cell: C. D. West, "The crystal structures of some
 * alkali hydrosulfides and monosulfides", Z. Kristallogr. 88 (1934) 97–115, as COD entry 1010249
 * gives it (CC0): P4/nmm, a = 6.011 Å, c = 4.009 Å, two formula units.
 *
 * @remarks
 * That is 43.62 cm³ mol⁻¹, a density of 1.1717 g cm⁻³ at M = 51.107 g mol⁻¹ (CIAAW's conventional
 * atomic weights). The ruling's 1.1715 matches the entry's rounded cell volume, 144.9 Å³, where a²c
 * is 144.85 Å³; n differs by about 3 × 10⁻⁴. The cell is as published: a room-temperature
 * measurement, and before 1947 possibly in kX units (1 kX = 1.002 02 Å), which would lower n by
 * about 0.005. A cloud near 200 K is presumably denser, raising n by a comparable amount
 * (unmeasured). Both lie inside the stated band.
 */
export const NH4SH_MOLAR_VOLUME_CM3_PER_MOL = cellMolarVolumeCm3PerMol(6.011 ** 2 * 4.009, 2);

/**
 * NH₄SH's molar mass, g mol⁻¹, from CIAAW's conventional atomic weights (H 1.008, N 14.007,
 * S 32.06). Only the file's stated density uses it: n depends on the molar volume alone.
 */
export const NH4SH_MOLAR_MASS_G_PER_MOL = 14.007 + 5 * 1.008 + 32.06;

/** The Lorentz–Lorenz factor (n² − 1) ÷ (n² + 2) of an index. */
function lorentzLorenzFactor(index: number): number {
  return (index * index - 1) / (index * index + 2);
}

/**
 * The additive molar refraction of gases, cm³ mol⁻¹: R = Σ (n² − 1) ÷ (n² + 2) × Z V₀ over their
 * refractivities at 0 °C and 760 mm.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, where every formula's pole lies outside it.
 * @throws RangeError if the wavelength lies at or past a formula's pole.
 */
export function gasMolarRefractionCm3PerMol(
  gases: ReadonlyArray<OneTermRefractivity>,
  wavelengthNm: number,
): number {
  const inverseUm2 = (1000 / wavelengthNm) ** 2;
  let molarRefraction = 0;
  for (const gas of gases) {
    if (!(gas.bPerUm2 > inverseUm2)) {
      throw new RangeError(`${wavelengthNm} nm lies at or past a pole of ${gas.source}`);
    }
    const n = 1 + gas.aPerUm2 / (gas.bPerUm2 - inverseUm2);
    molarRefraction +=
      lorentzLorenzFactor(n) * gas.compressibility * IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL;
  }
  return molarRefraction;
}

/**
 * A solid's ionic increment δ, its excess molar refraction over its constituent gases':
 * (n² − 1) ÷ (n² + 2) × V_m ÷ R − 1, with R their {@link gasMolarRefractionCm3PerMol}.
 *
 * @param wavelengthNm - The vacuum wavelength the solid's index is measured at, nm.
 * @param molarVolumeCm3PerMol - The solid's molar volume V_m.
 * @param index - The solid's real index at the wavelength.
 * @throws RangeError as {@link gasMolarRefractionCm3PerMol} does.
 */
export function ionicIncrement(
  gases: ReadonlyArray<OneTermRefractivity>,
  wavelengthNm: number,
  molarVolumeCm3PerMol: number,
  index: number,
): number {
  return (
    (lorentzLorenzFactor(index) * molarVolumeCm3PerMol) /
      gasMolarRefractionCm3PerMol(gases, wavelengthNm) -
    1
  );
}

/** NH₄Cl's measured index and density, the ionic increment's calibration. */
export interface IonicCalibration {
  /** n_D, the real index at the sodium D lines. */
  readonly indexD: number;
  /** ρ, g cm⁻³. */
  readonly densityGPerCm3: number;
  /** M, g mol⁻¹. */
  readonly molarMassGPerMol: number;
  /** V_m = M ÷ ρ, cm³ mol⁻¹. */
  readonly molarVolumeCm3PerMol: number;
}

/** NH₄Cl's density, g cm⁻³ (the CRC Handbook, 92nd ed., through Wikipedia, as the ruling takes it). */
const NH4CL_DENSITY_G_PER_CM3 = 1.519;

/** NH₄Cl's molar mass, g mol⁻¹, from CIAAW's conventional atomic weights (Cl 35.45). */
const NH4CL_MOLAR_MASS_G_PER_MOL = 14.007 + 4 * 1.008 + 35.45;

/**
 * NH₄Cl's measured index and density, the ionic increment's calibration
 * (science-r08-nonspherical.md §3.1): n_D = 1.642 and 1.519 g cm⁻³, at M = 53.489 g mol⁻¹.
 *
 * @remarks
 * Both are the ruling's, read through Wikipedia: the index from Chemister.ru, the density from the
 * CRC Handbook. The Handbook of Mineralogy's sal ammoniac gives n = 1.639(1), a measured 1.532 g
 * cm⁻³ and a = 3.8756 Å with one formula unit (an X-ray 1.5258 g cm⁻³), which would lower the
 * increment to +2.7–3.1% and NH₄SH's n(550) by up to 0.007, inside its band (R08's Risks, for
 * "main").
 */
export const NH4CL_CALIBRATION: IonicCalibration = {
  indexD: 1.642,
  densityGPerCm3: NH4CL_DENSITY_G_PER_CM3,
  molarMassGPerMol: NH4CL_MOLAR_MASS_G_PER_MOL,
  molarVolumeCm3PerMol: NH4CL_MOLAR_MASS_G_PER_MOL / NH4CL_DENSITY_G_PER_CM3,
};

/** The sodium D lines' mean, 589.3 nm, as the Cuthbertsons' air wavelengths are read. */
const SODIUM_D_NM = 589.3;

/**
 * NH₄Cl's ionic increment over NH₃ plus HCl, re-derived from {@link NH3_REFRACTIVITY} and
 * {@link HCL_REFRACTIVITY} (the same paper) at the D line: +3.51%.
 */
export const NH4CL_IONIC_INCREMENT = ionicIncrement(
  [NH3_REFRACTIVITY, HCL_REFRACTIVITY],
  SODIUM_D_NM,
  NH4CL_CALIBRATION.molarVolumeCm3PerMol,
  NH4CL_CALIBRATION.indexD,
);

/**
 * NH₄SH's ionic increment of molar refraction over NH₃'s plus H₂S's, +3.5%: ammonium chloride's,
 * {@link NH4CL_IONIC_INCREMENT} (+3.51%), as science-r08-nonspherical.md §3.1 rules.
 *
 * @remarks
 * The ruling's +4.5% (+4.3% to +4.7%) took NH₃ at the real gas's Z, 0.984 798, and HCl's index
 * from a secondary source with Z from 0.9924 to 1; with NH₃'s formula read at its theoretic density
 * and HCl from the same paper, the increment is +3.51% (R08's Risks, "Deviations in T3.b's
 * follow-up, as built"). SH⁻ is more polarisable than Cl⁻, and NH₄Br's increment is about +11% (the ruling's
 * estimate, from memory), so the stated band runs from 0 to +12%.
 */
export const NH4SH_IONIC_INCREMENT = 0.035;

/** The stated uncertainty band of {@link NH4SH_IONIC_INCREMENT}, 0 to +12%, as fractions. */
export const NH4SH_IONIC_INCREMENT_BAND: readonly [number, number] = [0, 0.12];

/**
 * A solid's real index by Lorentz–Lorenz from its constituents' gas refractivities: the additive
 * molar refraction R ({@link gasMolarRefractionCm3PerMol}) raised by an ionic increment δ, then
 * x = (1 + δ) R ÷ V_m and n = √((1 + 2x) ÷ (1 − x)).
 *
 * @param wavelengthNm - The vacuum wavelength, nm, where every formula's pole lies outside it.
 * @param molarVolumeCm3PerMol - The solid's molar volume V_m.
 * @param increment - δ, the solid's excess molar refraction over the gases' sum.
 * @throws RangeError if the wavelength lies at or past a formula's pole, or x is not in (0, 1).
 */
export function lorentzLorenzIndex(
  gases: ReadonlyArray<OneTermRefractivity>,
  wavelengthNm: number,
  molarVolumeCm3PerMol: number,
  increment: number,
): number {
  const x =
    ((1 + increment) * gasMolarRefractionCm3PerMol(gases, wavelengthNm)) / molarVolumeCm3PerMol;
  if (!(x > 0 && x < 1)) {
    throw new RangeError(`Lorentz–Lorenz: x = ${x} is not in (0, 1)`);
  }
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/**
 * NH₄SH's estimated real index at a vacuum wavelength, nm (science-r08-nonspherical.md §3.1):
 * {@link lorentzLorenzIndex} over NH₃ and H₂S at NH₄SH's cell, with NH₄Cl's ionic increment unless
 * another is given.
 *
 * @throws RangeError as {@link lorentzLorenzIndex} does, for a wavelength at or past NH₃'s pole
 *   (about 105 nm).
 */
export function nh4shIndex(wavelengthNm: number, increment = NH4SH_IONIC_INCREMENT): number {
  return lorentzLorenzIndex(
    [NH3_REFRACTIVITY, H2S_REFRACTIVITY],
    wavelengthNm,
    NH4SH_MOLAR_VOLUME_CM3_PER_MOL,
    increment,
  );
}

/**
 * NH₃ ice's molar volume, cm³ mol⁻¹, the method's check on a measured molecular solid: the cubic
 * cell of I. Olovsson and D. H. Templeton, Acta Cryst. 12 (1959) 832 (COD 2310927), a = 5.138 Å with
 * four molecules.
 */
export const NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL = cellMolarVolumeCm3PerMol(5.138 ** 3, 4);

/**
 * How far NH₄SH's estimate moves over {@link NH4SH_IONIC_INCREMENT_BAND} on the file's grid: the
 * largest rise to the band's top and the largest fall to its foot.
 */
export function nh4shBandSpan(): { readonly above: number; readonly below: number } {
  const [low, high] = NH4SH_IONIC_INCREMENT_BAND;
  let above = 0;
  let below = 0;
  for (const nm of MATERIAL_GRID_NM) {
    const n = nh4shIndex(nm);
    above = Math.max(above, nh4shIndex(nm, high) - n);
    below = Math.max(below, n - nh4shIndex(nm, low));
  }
  return { above, below };
}

/** The files made from no fetched table. */
export const DERIVED_MATERIALS: ReadonlyArray<MaterialFile> = [
  {
    key: "CO2",
    phase: "solid",
    variant: null,
    name: "CO2 ice deposited near 80 K",
    temperatureK: 80,
    shape: "crystal",
    provenance: "derived",
    paper:
      'S. G. Warren, "Optical constants of carbon dioxide ice", Appl. Opt. 25 (1986) 2650–2674, DOI 10.1364/AO.25.002650, Table I (pp. 2663–2667), for n; G. B. Hansen, "Ultraviolet to near-infrared absorption spectrum of carbon dioxide ice from 0.174 to 1.8 µm", J. Geophys. Res. 110 (2005) E11003, DOI 10.1029/2005JE002531, for k',
    source:
      "n(λ) = 1.3994 + 0.004312 µm² ÷ λ², a two-term Cauchy fit to Warren 1986's Table I over 0.30–1.10 µm (19 rows; worst residual 0.0009, inside Warren's stated ±0.05), made by the science agent from the author's copy, https://atmos.uw.edu/~sgw/PAPERS/1986_CO2ice_mcx.pdf, SHA-256 55873a3bcb4820867112c1243b4f583fa311f995d9a56f3db26f635664a7b485, fetched 2026-10-09 (science-r08-sulphur-co2ice.md); k = αλ ÷ 4π with α = 10⁻² m⁻¹, Hansen 2005's estimated visible absorption, below its detection limit over 0.25–1.0 µm. Warren's Egan–Spagnolo k, about 10³ larger, is the stated upper bound",
    licence:
      "derived values: Optica holds Warren's article under its pre-2017 agreement (reuse by permission only, text and data mining reserved; its copyright page, 2026-10-09), and AGU's terms for Hansen 2005 could not be read; the data are the authors' (decision-r08-licences.md). No table of Warren's is committed",
    reduction:
      "the fit and k evaluated on 380–780 nm every 5 nm, to six significant figures. Warren's index is for CO2 deposited near 80 K; Egan and Spagnolo's 1.35 ± 0.05 at 195 K suggests a lower n when warmer, recorded and not applied. Hansen 2005's k is for ice grown at 150 K (its abstract), so the file's temperature, 80 K, is n's. Its shape class is crystal, so Mie gives its cross-sections only",
    standIn: null,
    fit: CO2_ICE_FIT,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(CO2_ICE_FIT, nm).n)),
    k: MATERIAL_GRID_NM.map((nm) => rounded(cauchyIndex(CO2_ICE_FIT, nm).k)),
  },
  {
    key: "NH4SH",
    phase: "solid",
    variant: null,
    name: "ammonium hydrosulphide ice (no visible optical constants are published)",
    temperatureK: null,
    shape: "crystal",
    provenance: "standIn",
    paper:
      "the estimate's inputs: C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1–26, DOI 10.1098/rsta.1914.0001, for NH3's and HCl's dispersions; C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171–176, DOI 10.1098/rspa.1910.0003, for H2S's; C. D. West, \"The crystal structures of some alkali hydrosulfides and monosulfides\", Z. Kristallogr. 88 (1934) 97–115, DOI 10.1524/zkri.1934.88.1.97, for NH4SH's cell",
    source: `a Lorentz–Lorenz estimate, not a measurement (science-r08-nonspherical.md §3.1, ruled 2026-10-10): the molar refraction of NH3 plus H2S from their gas dispersions as stated at 0 °C and 760 mm (view/atmosphere/rayleigh.ts's Cuthbertson rows, each at the density its paper reduces to: Z = ${NH3_REFRACTIVITY.compressibility.toFixed(6)} for NH3 and ${H2S_REFRACTIVITY.compressibility.toFixed(6)} for H2S), raised by NH4Cl's ionic increment of ${(NH4SH_IONIC_INCREMENT * 100).toFixed(1)}% (its n_D ${NH4CL_CALIBRATION.indexD} and ${NH4CL_CALIBRATION.densityGPerCm3} g cm⁻³ against NH3 and HCl from Cuthbertson 1914 at ${SODIUM_D_NM} nm, ${(NH4CL_IONIC_INCREMENT * 100).toFixed(2)}%), over the molar volume of West 1934's cell as the Crystallography Open Database gives it, entry 1010249, https://www.crystallography.net/cod/1010249.cif, SHA-256 e721ed9fed41cab1c267ecadd4de31692340d3bfcefd6d9183312ae3a746bb71, fetched 2026-10-10 (P4/nmm, a = 6.011 Å, c = 4.009 Å, two formula units: ${NH4SH_MOLAR_VOLUME_CM3_PER_MOL.toFixed(2)} cm³ mol⁻¹, ${(NH4SH_MOLAR_MASS_G_PER_MOL / NH4SH_MOLAR_VOLUME_CM3_PER_MOL).toFixed(4)} g cm⁻³). The cell is as published, at room temperature and possibly in kX units, about ±0.005 in n. The gas dispersions are measured over 480–670.8 nm (NH3) and 486.1–656.3 nm (H2S) and extrapolated beyond. No visible optical constants of NH4SH are published: Howett et al. 2007 (JOSA B 24, 126) measured it only over 1,300–12,000 cm⁻¹ (0.83–7.7 µm)`,
    licence:
      "estimated values from cited constants and CC0 crystal data (the Crystallography Open Database's entry 1010249); no data set is copied (decision-r08-licences.md row 5)",
    reduction:
      "n = √((1 + 2x) ÷ (1 − x)), with x the raised molar refraction over the molar volume, evaluated on 380–780 nm every 5 nm to six significant figures; k = 0",
    standIn: `a non-absorbing particle whose real index is a Lorentz–Lorenz estimate: ${nh4shIndex(380).toFixed(3)} at 380 nm, ${nh4shIndex(550).toFixed(3)} at 550 nm and ${nh4shIndex(780).toFixed(3)} at 780 nm, within +${nh4shBandSpan().above.toFixed(3)} and −${nh4shBandSpan().below.toFixed(3)} for an ionic increment from ${(NH4SH_IONIC_INCREMENT_BAND[0] * 100).toFixed(0)} to +${(NH4SH_IONIC_INCREMENT_BAND[1] * 100).toFixed(0)}%. The same estimate for NH3 ice, at the cell of I. Olovsson and D. H. Templeton (Acta Cryst. 12 (1959) 832; COD 2310927; a = 5.138 Å, four molecules), gives ${lorentzLorenzIndex([NH3_REFRACTIVITY], 550, NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL, 0).toFixed(3)} against the 1.436 at 550 nm that Martonchik et al. 1984 measure (Appl. Opt. 23, 541). k = 0, since pure NH4SH is a white solid (PubChem CID 25515, from ICSC); the colours it takes on Jupiter come from radiolysis products (Loeffler and Hudson 2018, Icarus 302, 418). It replaces the earlier stand-in of 1.80, which Sromovsky et al. 2017 (Icarus 291, 232, §4.3) state without a source, after the best-fit effective index of 1.85 that Sato et al. 2013 (Icarus 222, 100) find for Jupiter's clouds, a fit that particle shape and size trade against. 1.80 would need a molar refraction ${(ionicIncrement([NH3_REFRACTIVITY, H2S_REFRACTIVITY], 550, NH4SH_MOLAR_VOLUME_CM3_PER_MOL, 1.8) * 100).toFixed(0)}% above the additive one. A body whose medium shows this mode with less than CLOUD_DECK_SPLIT_OPTICAL_DEPTH above it at 550 nm reads ATMOSPHERE: APPROXIMATE, until measured visible constants replace it`,
    fit: null,
    wavelengthsNm: MATERIAL_GRID_NM,
    n: MATERIAL_GRID_NM.map((nm) => rounded(nh4shIndex(nm))),
    k: MATERIAL_GRID_NM.map(() => 0),
  },
];

/**
 * One fetched source reduced to its material file.
 *
 * @throws Error as its parser and {@link resample} do.
 */
export function reduceSource(
  spec: Pick<FetchedSpec, "parse" | "resample" | "header">,
  text: string,
): MaterialFile {
  const { n, k } = resample(spec.parse(text), spec.resample);
  return { ...spec.header, wavelengthsNm: MATERIAL_GRID_NM, n, k };
}

function readChecked(path: string, sha256: string): string {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${sha256}`);
  }
  return bytes.toString("utf8");
}

const DERIVED_FILES: Readonly<Record<string, string>> = {
  CO2: "carbon-dioxide-ice.json",
  NH4SH: "ammonium-hydrosulphide.json",
};

/**
 * The command line: `--out <dir>` and one `--<option> <path>` per fetched source (the options of
 * {@link FETCHED_MATERIALS}). Writes every material file into the directory. Returns the exit code.
 */
export function main(args: readonly string[]): number {
  try {
    const options: Record<string, { readonly type: "string" }> = { out: { type: "string" } };
    for (const spec of FETCHED_MATERIALS) {
      options[spec.option] = { type: "string" };
    }
    const values: Readonly<Record<string, unknown>> = parseArgs({
      args: [...args],
      options,
      strict: true,
    }).values;
    const out = values["out"];
    if (typeof out !== "string") {
      throw new Error(
        `usage: materials --out <dir> ${FETCHED_MATERIALS.map((s) => `--${s.option} <path>`).join(" ")}`,
      );
    }
    mkdirSync(out, { recursive: true });
    for (const spec of FETCHED_MATERIALS) {
      const path = values[spec.option];
      if (typeof path !== "string") {
        throw new Error(`missing --${spec.option}`);
      }
      const file = reduceSource(spec, readChecked(path, spec.sha256));
      writeFileSync(join(out, spec.file), `${JSON.stringify(file, null, 2)}\n`);
    }
    for (const file of DERIVED_MATERIALS) {
      const name = DERIVED_FILES[file.key];
      if (name === undefined) {
        throw new Error(`no file name for the derived material ${file.key}`);
      }
      writeFileSync(join(out, name), `${JSON.stringify(file, null, 2)}\n`);
    }
    return 0;
  } catch (error: unknown) {
    console.error(`materials: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
  }
}

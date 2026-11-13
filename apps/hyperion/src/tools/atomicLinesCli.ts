/**
 * `node scripts/atmosphereData.mjs atomic-shares …` and `atomic-bounds …` (plan R08, R08.T4.h):
 * the fifteen registry rows' atomic and ionic answers, as R08.T4.e leaves them with O's, from Kurucz's line lists and Barklem and
 * Collet 2016's tables.
 *
 * @remarks
 * `atomic-shares --kurucz <dir> --barklem <dir> [--asd-zn2 <csv>] [--stages <key,…>] [--out <json>]`
 * prints each stage's largest bake-bin share at its reference column at each of
 * {@link SHARE_TEMPERATURES_K} with the lines it comes from ({@link stageBinShares}), and writes the
 * record as JSON where `--out` names a file.
 *
 * `atomic-bounds --kurucz <dir> --barklem <dir> --asd-zn2 <csv> [--out <json>]` writes
 * `absorbers/atomicBounds.json`: each of {@link ATOMIC_ANSWERS}' stages' 1 nm cell means by
 * R08.T4.c's method ({@link lineCellMeansM2}), at T4.c's nodes for an interim pending entry and
 * from its `belowK` for a stated none, decided by the shares ({@link belowKOf}).
 *
 * Each input is checked against its SHA-256 and none is committed (`decision-r08-licences.md`):
 * Kurucz states no terms, and Barklem and Collet's tables are CDS's, free for a scientific use.
 */

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import {
  type AtomicStage,
  barklemName,
  BIN_SHARE_BOUND,
  type CellMeans,
  ELECTRON_RADIUS_CM,
  ionisationLimitEv,
  type KuruczLine,
  lineCellMeansM2,
  LINE_WINDOW_NM,
  parseBarklemTable,
  parseIonisationEnergies,
  parseKuruczLines,
  partitionFunctionAt,
  PER_CM_PER_EV,
  roundUpToThreeFigures,
  type SahaData,
  type StageShares,
  stageBinShares,
} from "./atomicLines";
import { readCheckedText, SPEED_OF_LIGHT_M_PER_S } from "./molecularLines";

/** A fetched file and its SHA-256. */
export interface CheckedInput {
  readonly name: string;
  readonly sha256: string;
}

/** Kurucz's lists' directory. */
export const KURUCZ_URL = "http://kurucz.harvard.edu/linelists/gfall/";

/** Kurucz's citation (confirmed at the fetch of 2026-10-19). */
export const KURUCZ_CITATION =
  'R. L. Kurucz, "Including all the lines", Can. J. Phys. 89 (2011) 417, DOI 10.1139/p10-104';

/** Kurucz's lists, by the stage each holds, as main fetched them 2026-10-16 and again 2026-10-19. */
export const KURUCZ_LISTS: ReadonlyArray<
  AtomicStage & CheckedInput & { readonly spectrum: string }
> = [
  {
    key: "Na+",
    symbol: "Na",
    stage: 1,
    spectrum: "Na II",
    name: "gf1101.all",
    sha256: "e95dee2bd77ef96922826918936933722cc195c7ed9629fa7859697055cfd5f4",
  },
  {
    key: "Mg+",
    symbol: "Mg",
    stage: 1,
    spectrum: "Mg II",
    name: "gf1201.all",
    sha256: "aec1d84e33d971399d5fd8069720fc1c71e4c03fd1d1839c0224164b6047c9d6",
  },
  {
    key: "Al",
    symbol: "Al",
    stage: 0,
    spectrum: "Al I",
    name: "gf1300.all",
    sha256: "c9d02e6de21ecf52e9ef0ddbaa9ecfb1c06a1ece80828695d67bf78509107086",
  },
  {
    key: "Al+",
    symbol: "Al",
    stage: 1,
    spectrum: "Al II",
    name: "gf1301.all",
    sha256: "5245199232ad5a159841ca5d5f686d221355455b257e8ad8a1503f5648a7510b",
  },
  {
    key: "Si+",
    symbol: "Si",
    stage: 1,
    spectrum: "Si II",
    name: "gf1401.all",
    sha256: "f29a6989f3171aa32da8728057288216e7ce5750b2e8a7ce33455a40d72ddd16",
  },
  {
    key: "K+",
    symbol: "K",
    stage: 1,
    spectrum: "K II",
    name: "gf1901.all",
    sha256: "ce3a13690a68e117c125480f2bcebf2430b6db22fd469c0688c60af53e9aebe8",
  },
  {
    key: "V",
    symbol: "V",
    stage: 0,
    spectrum: "V I",
    name: "gf2300.all",
    sha256: "300da04a03b4bca4800252e83590aba67447b2164bec48c17db17d0a27f593c8",
  },
  {
    key: "V+",
    symbol: "V",
    stage: 1,
    spectrum: "V II",
    name: "gf2301.all",
    sha256: "e722d0b468e3c30db66d98321d5af534a7782b2a0faf2e75eed608f160802175",
  },
  {
    key: "Cr+",
    symbol: "Cr",
    stage: 1,
    spectrum: "Cr II",
    name: "gf2401.all",
    sha256: "166b4839fac2a14b6485d16a969aaee2a839027a74a7ce8a6ba77bb089ab31b3",
  },
  {
    key: "Mn",
    symbol: "Mn",
    stage: 0,
    spectrum: "Mn I",
    name: "gf2500.all",
    sha256: "850c89ac85f4491074ee0b86f0d6227f7e3275b82c195b80d5991425fcfa1bf0",
  },
  {
    key: "Mn+",
    symbol: "Mn",
    stage: 1,
    spectrum: "Mn II",
    name: "gf2501.all",
    sha256: "04a5e5b0b3d96b62c2b1a65b3309a8892d87a67e08e7031529d9dd9ac933928f",
  },
  {
    key: "Ni",
    symbol: "Ni",
    stage: 0,
    spectrum: "Ni I",
    name: "gf2800.all",
    sha256: "52d88aebef5098c599574848a39003a06ee918446825c8d5b658ac3c4e1fbf83",
  },
  {
    key: "Ni+",
    symbol: "Ni",
    stage: 1,
    spectrum: "Ni II",
    name: "gf2801.all",
    sha256: "60993b3cad578d31b2f454899675bbcd87ccb7421b6fca8786bb3550a401b963",
  },
  // R08.T4.e's: O I, stated none to 5,000 K by its lines' share (decision-r08-t4e-budget §7).
  {
    key: "O",
    symbol: "O",
    stage: 0,
    spectrum: "O I",
    name: "gf0800.all",
    sha256: "5f6fce8aa4226b6c827b841d9657ffe07c26c24c7eb49667433ece8461809ba1",
  },
  {
    key: "Zn",
    symbol: "Zn",
    stage: 0,
    spectrum: "Zn I",
    name: "gf3000.all",
    sha256: "6fbf50f9bae5ef291e1243ed2c5aa06cbc7576430fec987e8a61ca5b190cfedd",
  },
  {
    key: "Zn+",
    symbol: "Zn",
    stage: 1,
    spectrum: "Zn II",
    name: "gf3001.all",
    sha256: "d220385ed0218c15f1e3d0206edc90ca050381acfa6419f3f73def52d6334c6c",
  },
];

/** Barklem and Collet's catalogue. */
export const BARKLEM_URL = "https://cdsarc.cds.unistra.fr/ftp/J/A+A/588/A96/";

/** Barklem and Collet's citation. */
export const BARKLEM_CITATION =
  "P. S. Barklem and R. Collet, Astron. Astrophys. 588 (2016) A96, via CDS VizieR J/A+A/588/A96";

/**
 * Barklem and Collet's tables the sums read, as main fetched them 2026-10-16 and again 2026-10-19
 * (table 7 first then).
 */
export const BARKLEM_TABLES = {
  ionisationEnergies: {
    name: "table4.dat",
    sha256: "722c36a3294fc0d3d599145b2b19b502f9fa85362541a6acd789c098668632cf",
  },
  equilibriumConstants: {
    name: "table7.dat",
    sha256: "5282b947328269d1936844ed5b088c3f7f9d56a1c45af8ce242992f86f2b9d5c",
  },
  partitionFunctions: {
    name: "table8.dat",
    sha256: "c08800be067ccbbcc5486c61085569da153fbebcaccfd4bef3d3e203240846be",
  },
} as const satisfies Readonly<Record<string, CheckedInput>>;

/** Barklem and Collet's tables from a directory holding them, each checked. */
export function readSahaData(directory: string): SahaData {
  const read = (file: CheckedInput): string =>
    readCheckedText(join(directory, file.name), file.sha256);
  return {
    ionisationEnergiesEv: parseIonisationEnergies(read(BARKLEM_TABLES.ionisationEnergies)),
    equilibriumConstants: parseBarklemTable(read(BARKLEM_TABLES.equilibriumConstants), "table 7"),
    partitionFunctions: parseBarklemTable(read(BARKLEM_TABLES.partitionFunctions), "table 8"),
  };
}

/**
 * The temperatures the shares are taken at, K: the science ruling's six (2,000–4,600 K), whose
 * largest is a stage's most favourable, and 5,000 K for a stated none's `belowK` (ruled
 * 2026-10-19).
 */
export const SHARE_TEMPERATURES_K = [2_000, 2_500, 3_000, 3_500, 4_000, 4_600, 5_000] as const;

/** R08.T4.c's nodes for an atom's or ion's cell means, K. */
export const ATOMIC_NODES_K = [300, 1_000, 2_000, 3_000, 4_000, 5_000] as const;

/** He's pending nodes above 5,000 K, K: a stated none's entry takes them above its `belowK`. */
export const HOT_NODES_K = [5_000, 6_000, 8_000, 10_000] as const;

/** A stage's largest bake-bin share at one temperature, with the bin and its leading lines. */
export interface LargestShare {
  readonly temperatureK: number;
  readonly share: number;
  /** The bin, 0–14. */
  readonly bin: number;
  readonly columnPerM2: number;
  readonly leading: StageShares["leading"][number];
  readonly linesSummed: number;
}

/** A stage's shares at each of {@link SHARE_TEMPERATURES_K}: each one's largest. */
export function largestShares(
  data: SahaData,
  stage: AtomicStage,
  lines: ReadonlyArray<KuruczLine>,
): ReadonlyArray<LargestShare> {
  return SHARE_TEMPERATURES_K.map((temperatureK) => {
    const shares = stageBinShares(data, stage, lines, temperatureK);
    let bin = 0;
    for (const [k, s] of shares.binShares.entries()) {
      if (s > (shares.binShares[bin] ?? 0)) {
        bin = k;
      }
    }
    return {
      temperatureK,
      share: shares.binShares[bin] ?? 0,
      bin,
      columnPerM2: shares.columnPerM2,
      leading: shares.leading[bin] ?? [],
      linesSummed: shares.linesSummed,
    };
  });
}

/**
 * How a stage's shares answer the third reason (ruled 2026-10-19): `undefined` (not a stated none:
 * a pending entry) if its share reaches {@link BIN_SHARE_BOUND} at any of 2,000–4,600 K; 5,000 K
 * if it stays under at all seven temperatures; 4,600 K if under through 4,600 K but not at 5,000 K.
 */
export function belowKOf(shares: ReadonlyArray<LargestShare>): 4_600 | 5_000 | undefined {
  const through4600 = shares.filter((s) => s.temperatureK <= 4_600);
  if (through4600.some((s) => s.share >= BIN_SHARE_BOUND)) {
    return undefined;
  }
  return shares.every((s) => s.share < BIN_SHARE_BOUND) ? 5_000 : 4_600;
}

/** The nodes of a stated none's pending entry from its `belowK`, K: it and He's nodes above it. */
export function nodesFrom(belowK: 4_600 | 5_000): ReadonlyArray<number> {
  return belowK === 5_000 ? HOT_NODES_K : [4_600, ...HOT_NODES_K];
}

/** A Kurucz list's lines, checked. */
export function readKuruczList(directory: string, list: CheckedInput): ReadonlyArray<KuruczLine> {
  return parseKuruczLines(readCheckedText(join(directory, list.name), list.sha256));
}

/** A line's description for a record: spectrum, vacuum wavelength, levels, log gf and reference. */
export function describeLine(spectrum: string, line: KuruczLine): string {
  const ev = (line.lowerPerCm / PER_CM_PER_EV).toFixed(2);
  return (
    `${spectrum} ${line.vacuumNm.toFixed(2)} nm (${line.lowerLabel} - ${line.upperLabel}, ` +
    `log gf ${line.logGf.toFixed(3)}, from ${ev} eV, ${line.reference.length > 0 ? line.reference : "no code"})`
  );
}

/** The bin's span, for a record: `405-431 nm`. */
export function binSpan(bin: number): string {
  const low = 380 + (bin * 380) / 15;
  return `${Math.round(low)}-${Math.round(low + 380 / 15)} nm`;
}

/** `atomic-shares`: prints and records each stage's largest share at each temperature. */
export function runAtomicShares(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      kurucz: { type: "string" },
      barklem: { type: "string" },
      "asd-zn2": { type: "string" },
      stages: { type: "string" },
      out: { type: "string" },
    },
  });
  if (values.kurucz === undefined || values.barklem === undefined) {
    throw new Error(
      "usage: atomic-shares --kurucz <dir> --barklem <dir> [--asd-zn2 <csv>] [--stages a,b] [--out <json>]",
    );
  }
  const data = readSahaData(values.barklem);
  const wanted = values.stages?.split(",");
  const record: Record<string, unknown> = {};
  for (const list of KURUCZ_LISTS.filter((l) => wanted === undefined || wanted.includes(l.key))) {
    const lines = stageLines(list, values.kurucz, values["asd-zn2"]);
    const shares = largestShares(data, list, lines);
    const belowK = belowKOf(shares);
    process.stdout.write(
      `${list.key} (${list.name}, ${shares[0]?.linesSummed ?? 0} lines in ${LINE_WINDOW_NM.join("-")} nm): ${belowK === undefined ? "pending" : `stated none below ${belowK} K`}\n`,
    );
    for (const s of shares) {
      const top = s.leading
        .slice(0, 3)
        .map((l) => `${describeLine(list.spectrum, l.line)} ${l.share.toExponential(2)}`)
        .join("; ");
      process.stdout.write(
        `  ${s.temperatureK} K: column ${s.columnPerM2.toExponential(3)} m-2, largest share ${s.share.toExponential(3)} in ${binSpan(s.bin)}${top.length > 0 ? `: ${top}` : ""}\n`,
      );
    }
    record[list.key] = {
      belowK: belowK ?? null,
      shares: shares.map((s) => ({
        temperatureK: s.temperatureK,
        columnPerM2: s.columnPerM2,
        share: s.share,
        bin: binSpan(s.bin),
        leading: s.leading.map((l) => ({
          line: describeLine(list.spectrum, l.line),
          share: l.share,
        })),
      })),
    };
  }
  if (values.out !== undefined) {
    writeFileSync(values.out, `${JSON.stringify(record, null, 2)}\n`);
  }
}

// --- Zn⁺'s lines (§4.1, ruled 2026-10-19): Kurucz's gf3001.all holds none in the range ---

/** NIST ASD's request for Zn II's lines over 350–800 nm (air), as main made it 2026-10-19. */
export const ASD_ZN2_URL =
  "https://physics.nist.gov/cgi-bin/ASD/lines1.pl?spectra=Zn+II&limits_type=0&low_w=350&upp_w=800" +
  "&unit=1&de=0&format=2&line_out=0&remove_js=on&en_unit=1&output=0&page_size=15&show_obs_wl=1" +
  "&show_calc_wl=1&order_out=0&show_av=2&tsb_value=0&A_out=0&intens_out=on&allowed_out=1" +
  "&forbid_out=1&conf_out=on&term_out=on&enrg_out=on&J_out=on&g_out=on&submit=Retrieve+Data";

/** ASD's Zn II lines as fetched 2026-10-19 (main's file, byte for byte): 17 lines. */
export const ASD_ZN2: CheckedInput = {
  name: "zn2.csv",
  sha256: "6303dcd2a2645e9853b3f8a80ee3d7504b7d22fb71c7254789218ae65f987e75",
};

/** ASD's citation. */
export const ASD_CITATION =
  "A. Kramida, Yu. Ralchenko, J. Reader and NIST ASD Team, NIST Atomic Spectra Database " +
  "(ver. 5.12), 2024, DOI 10.18434/T4W30F";

/** Martin and Kaufman 1970, whose levels Zn⁺'s lines take. */
const MARTIN_KAUFMAN_1970 =
  "W. C. Martin and V. Kaufman, J. Res. Natl. Bur. Stand. 74A (1970) 11, Table 2 (read 2026-10-19 in NIST's open copy)";

/** Mayo, Ortiz and Campos 2006, ASD's source for seven lines' A; closed, no open copy found. */
const MAYO_2006 =
  "R. Mayo, M. Ortiz and J. Campos, Eur. Phys. J. D 37 (2006) 181, DOI 10.1140/epjd/e2005-00283-5";

/** Nandy and Sahoo 2015, ASD's source for four lines' A, computed (relativistic coupled-cluster). */
const NANDY_SAHOO_2015 =
  "D. K. Nandy and B. K. Sahoo, Mon. Not. R. Astron. Soc. 447 (2015) 3812, DOI 10.1093/mnras/stu2707, Table 10";

/** Kisielius et al. 2015, whose configuration-interaction line strength one 6.0 eV line takes. */
const KISIELIUS_2015 =
  "R. Kisielius et al., Astrophys. J. 804 (2015) 76, DOI 10.1088/0004-637X/804/1/76, Table 2 " +
  "(read in arXiv:1504.01667v1)";

/**
 * Zn II's levels, cm⁻¹, by configuration, term and J as ASD writes them: Martin and Kaufman 1970,
 * Table 2 (ASD's levels, after Sugar and Musgrove 1995, are the same to their last figure).
 */
export const ZN2_LEVELS_PER_CM: Readonly<Record<string, number>> = {
  "3d10.4p 2P* 1/2": 48_481.0,
  "3d10.4p 2P* 3/2": 49_355.04,
  "3d9.4s2 2D 5/2": 62_722.45,
  "3d9.4s2 2D 3/2": 65_441.64,
  "3d10.5s 2S 1/2": 88_437.15,
  "3d10.4d 2D 3/2": 96_909.74,
  "3d10.4d 2D 5/2": 96_960.4,
  "3d10.5p 2P* 1/2": 101_365.9,
  "3d10.5p 2P* 3/2": 101_611.4,
  "3d10.6s 2S 1/2": 114_498.02,
  "3d10.4f 2F* 7/2": 117_263.4,
  "3d10.4f 2F* 5/2": 117_264.0,
  "3d10.5d 2D 3/2": 117_969.32,
  "3d10.5d 2D 5/2": 117_993.61,
  "3d10.7s 2S 1/2": 125_880.0,
  "3d10.6d 2D 3/2": 127_630.6,
  "3d10.6d 2D 5/2": 127_643.7,
};

/**
 * The transition probability a Zn II line takes, A s⁻¹, where neither a measured lifetime of its
 * upper level nor a computed A from an open paper is found: above every A ASD lists for a Zn II
 * line in the range (2.72 × 10⁸ s⁻¹), the ruling's bound.
 *
 * @remarks
 * For 621.64 and 748.09 nm (4p ²P₃/₂–3d⁹4s² ²D) computed values exist but are not inputs here:
 * Kisielius et al. 2015's line strengths are only in their CDS catalogue (J/ApJ/804/76, AAS-origin,
 * CC BY-NC by CDS), and R. L. Kurucz's `atoms/3001/gf3001.pos` (2012) gives log gf −3.12 and −2.24,
 * 4.0 and 3.4 dex below the bound's (R08.T4.h's science check), so the bound errs high by that much.
 */
export const ZN2_BOUND_A_PER_S = 3e8;

/** Where a Zn II line's transition probability comes from (§4.1). */
export type ZincIonTransitionProbability =
  /** ASD's value, its primary paper closed: the licence ruling's stated exception. */
  | { readonly kind: "asd"; readonly paper: string }
  /** A computed A, s⁻¹, from an open paper. */
  | { readonly kind: "computed"; readonly perS: number; readonly paper: string }
  /** A computed E1 line strength, e² a₀², from an open paper. */
  | { readonly kind: "lineStrength"; readonly atomicUnits: number; readonly paper: string }
  /** {@link ZN2_BOUND_A_PER_S}: no measured lifetime of the upper level, no computed A taken. */
  | { readonly kind: "bound" };

/**
 * Each of ASD's 17 Zn II lines' transition probability, by its lower and upper level (ASD's
 * references, read 2026-10-19: T7870, Mayo et al. 2006; T9418, Nandy and Sahoo 2015; none for the
 * five without an A).
 */
export const ZN2_TRANSITION_PROBABILITIES: Readonly<Record<string, ZincIonTransitionProbability>> =
  {
    "3d10.5p 2P* 1/2 - 3d10.6d 2D 3/2": { kind: "bound" },
    "3d10.5p 2P* 3/2 - 3d10.6d 2D 5/2": { kind: "bound" },
    "3d10.5p 2P* 1/2 - 3d10.7s 2S 1/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.5p 2P* 3/2 - 3d10.7s 2S 1/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.4d 2D 3/2 - 3d10.4f 2F* 5/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.4d 2D 5/2 - 3d10.4f 2F* 5/2": { kind: "computed", perS: 1.46e7, paper: NANDY_SAHOO_2015 },
    "3d10.4d 2D 5/2 - 3d10.4f 2F* 7/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.4p 2P* 1/2 - 3d9.4s2 2D 3/2": {
      kind: "lineStrength",
      atomicUnits: 1.196e-2,
      paper: KISIELIUS_2015,
    },
    "3d10.5p 2P* 1/2 - 3d10.5d 2D 3/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.5p 2P* 3/2 - 3d10.5d 2D 5/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.5p 2P* 3/2 - 3d10.5d 2D 3/2": { kind: "asd", paper: MAYO_2006 },
    "3d10.4p 2P* 3/2 - 3d9.4s2 2D 3/2": { kind: "bound" },
    "3d10.4p 2P* 3/2 - 3d9.4s2 2D 5/2": { kind: "bound" },
    "3d10.5s 2S 1/2 - 3d10.5p 2P* 3/2": { kind: "computed", perS: 5.36e7, paper: NANDY_SAHOO_2015 },
    "3d10.5p 2P* 1/2 - 3d10.6s 2S 1/2": { kind: "computed", perS: 3.22e7, paper: NANDY_SAHOO_2015 },
    "3d10.5s 2S 1/2 - 3d10.5p 2P* 1/2": { kind: "computed", perS: 5.1e7, paper: NANDY_SAHOO_2015 },
    "3d10.5p 2P* 3/2 - 3d10.6s 2S 1/2": { kind: "computed", perS: 6.36e7, paper: NANDY_SAHOO_2015 },
  };

/** The hartree as a wavenumber, cm⁻¹ (CODATA 2018: 219,474.631 363 2). */
const HARTREE_PER_CM = 219_474.631_363_2;

/** A field of ASD's CSV, its `="…"` quoting removed. */
function asdField(value: string | undefined): string {
  return (value ?? "")
    .replace(/^"?=?"*/u, "")
    .replace(/"*$/u, "")
    .trim();
}

/** One of ASD's CSV rows split on its commas (no field holds one). */
function asdRow(line: string): string[] {
  return line.split(",").map(asdField);
}

/**
 * Zn⁺'s lines (§4.1, ruled 2026-10-19): ASD's Zn II lines over 350–800 nm, a finding aid for the
 * transitions, each at Martin and Kaufman 1970's levels ({@link ZN2_LEVELS_PER_CM}; the vacuum
 * wavelength from their difference) and its transition probability from
 * {@link ZN2_TRANSITION_PROBABILITIES}: gf = g_u A λ² ÷ 8π² r_e c, or (2 ÷ 3)(ΔE ÷ E_h) S for a
 * line strength.
 *
 * @throws Error for a row whose levels or transition the tables do not hold, or an ASD value
 *   missing where the table takes it.
 */
export function zincIonLines(csv: string): ReadonlyArray<KuruczLine> {
  const [header, ...rows] = csv.split(/\r?\n/u).filter((l) => l.trim().length > 0);
  const columns = asdRow(header ?? "");
  const at = (row: ReadonlyArray<string>, name: string): string => row[columns.indexOf(name)] ?? "";
  return rows.map((line) => {
    const row = asdRow(line);
    const lower = `${at(row, "conf_i")} ${at(row, "term_i")} ${at(row, "J_i")}`;
    const upper = `${at(row, "conf_k")} ${at(row, "term_k")} ${at(row, "J_k")}`;
    const lowerPerCm = ZN2_LEVELS_PER_CM[lower];
    const upperPerCm = ZN2_LEVELS_PER_CM[upper];
    const source = ZN2_TRANSITION_PROBABILITIES[`${lower} - ${upper}`];
    const upperG = Number(at(row, "g_k"));
    if (
      lowerPerCm === undefined ||
      upperPerCm === undefined ||
      source === undefined ||
      !(upperG > 0)
    ) {
      throw new Error(`ASD's Zn II line ${lower} - ${upper} is not in the tool's tables`);
    }
    const wavenumber = upperPerCm - lowerPerCm;
    const vacuumM = 1e-2 / wavenumber;
    const fromA = (a: number): number =>
      (upperG * a * vacuumM * vacuumM) /
      (8 * Math.PI ** 2 * ELECTRON_RADIUS_CM * 1e-2 * SPEED_OF_LIGHT_M_PER_S);
    let gf: number;
    let reference: string;
    switch (source.kind) {
      case "asd": {
        const a = Number(at(row, "Aki(s^-1)"));
        if (!(a > 0)) {
          throw new Error(`ASD gives Zn II ${lower} - ${upper} no A, which the tool's table takes`);
        }
        gf = fromA(a);
        reference = "ASD";
        break;
      }
      case "computed":
        gf = fromA(source.perS);
        reference = "NS15";
        break;
      case "lineStrength":
        gf = (2 / 3) * (wavenumber / HARTREE_PER_CM) * source.atomicUnits;
        reference = "K15";
        break;
      case "bound":
        gf = fromA(ZN2_BOUND_A_PER_S);
        reference = "bound";
        break;
    }
    const vacuumNm = 1e7 / wavenumber;
    return {
      listedNm: vacuumNm,
      vacuumNm,
      logGf: Math.log10(gf),
      lowerPerCm,
      upperPerCm,
      lowerJ: angularMomentum(at(row, "J_i")),
      upperJ: angularMomentum(at(row, "J_k")),
      lowerLabel: `${at(row, "conf_i")} ${at(row, "term_i")}`,
      upperLabel: `${at(row, "conf_k")} ${at(row, "term_k")}`,
      reference,
      logGammaRadPerS: 0,
      logGammaStarkPerElectron: 0,
      logGammaVanDerWaalsPerHydrogen: 0,
    };
  });
}

/** A J written as a fraction (`3/2`) or a whole number, as a number. */
function angularMomentum(text: string): number {
  const [num, den] = text.split("/").map(Number);
  return den === undefined ? (num ?? Number.NaN) : (num ?? Number.NaN) / den;
}

// --- The answers' cell means: atomicBounds.json ---

/**
 * The atomic and ionic answers `atomicBounds.json` holds (R08.T4.h's fifteen rows, as R08.T4.e
 * leaves them): R08.T4.h's interim entries of the stages R08.T4.e defers past its byte budget
 * (decision-r08-t4e-budget §5), Al, V, Mn, Ni, V⁺, Cr⁺ and Mn⁺, Zn's having left for its table;
 * the stated nones decided by their shares, with an entry from their `belowK`; Mg⁺ and Si⁺,
 * decided so and drawn from their `belowK` by R08.T4.e's tables; and O, R08.T4.e's stated none to
 * 5,000 K (decision-r08-t4e-budget §7).
 */
export const ATOMIC_ANSWERS: ReadonlyArray<{
  readonly key: string;
  /**
   * `interim`: a pending entry at T4.c's nodes (R08.T4.h's, kept for a stage R08.T4.e defers past
   * its byte budget, decision-r08-t4e-budget §5); `byShares`: decided by its lines' shares, a
   * stated none with a pending entry from its `belowK`; `drawnFromFloor`: decided so, but drawn
   * from its `belowK` by R08.T4.e's table, so that no pending entry stays (Mg⁺'s and Si⁺'s; the
   * `belowK` contract's table branch).
   */
  readonly answer: "interim" | "byShares" | "drawnFromFloor";
}> = [
  { key: "Al", answer: "interim" },
  { key: "V", answer: "interim" },
  { key: "Mn", answer: "interim" },
  { key: "Ni", answer: "interim" },
  { key: "V+", answer: "interim" },
  { key: "Cr+", answer: "interim" },
  { key: "Mn+", answer: "interim" },
  { key: "Na+", answer: "byShares" },
  { key: "Al+", answer: "byShares" },
  { key: "Si+", answer: "drawnFromFloor" },
  { key: "K+", answer: "byShares" },
  { key: "Ni+", answer: "byShares" },
  { key: "Zn+", answer: "byShares" },
  { key: "Mg+", answer: "drawnFromFloor" },
  { key: "O", answer: "byShares" },
];

/** A stage's lines: Kurucz's list, or for Zn⁺ ASD's transitions ({@link zincIonLines}). */
export function stageLines(
  list: (typeof KURUCZ_LISTS)[number],
  kuruczDirectory: string,
  asdZn2Path: string | undefined,
): ReadonlyArray<KuruczLine> {
  if (list.key !== "Zn+") {
    return readKuruczList(kuruczDirectory, list);
  }
  if (asdZn2Path === undefined) {
    throw new Error("Zn+ takes ASD's Zn II lines: give --asd-zn2");
  }
  return zincIonLines(readCheckedText(asdZn2Path, ASD_ZN2.sha256));
}

/** A species' entry of a bounds file as the tools write it, `transparency.ts`'s form. */
export interface BoundEntry {
  readonly key: string;
  readonly kind: "species";
  readonly task: "R08.T4.e" | "R08.T4.f";
  /** The part of the key's absorption, where one task holds two entries for it. */
  readonly part?: string;
  readonly basis: "measured" | "estimated";
  readonly temperaturesK: ReadonlyArray<number>;
  readonly source: string;
  readonly method: string;
  readonly cellMeansM2: ReadonlyArray<ReadonlyArray<number>>;
}

/** An entry of `atomicBounds.json`: R08.T4.e's. */
export type AtomicBoundEntry = BoundEntry & { readonly task: "R08.T4.e" };

/**
 * An entry's text in `bounds.json`'s layout: two-space JSON at the entries' depth, its part after
 * its task where it has one, each node's cells on one line.
 */
export function boundEntryText(e: BoundEntry): string {
  return [
    "    {",
    `      "key": ${JSON.stringify(e.key)},`,
    `      "kind": ${JSON.stringify(e.kind)},`,
    `      "task": ${JSON.stringify(e.task)},`,
    ...(e.part === undefined ? [] : [`      "part": ${JSON.stringify(e.part)},`]),
    `      "basis": ${JSON.stringify(e.basis)},`,
    `      "temperaturesK": [${e.temperaturesK.join(", ")}],`,
    `      "source": ${JSON.stringify(e.source)},`,
    `      "method": ${JSON.stringify(e.method)},`,
    `      "cellMeansM2": [`,
    e.cellMeansM2.map((row) => `        [${row.join(", ")}]`).join(",\n"),
    "      ]",
    "    }",
  ].join("\n");
}

/** What `atomic-bounds` found for one stage, beside its entry. */
export interface AtomicAnswer {
  readonly key: string;
  readonly shares: ReadonlyArray<LargestShare> | undefined;
  readonly belowK: 4_600 | 5_000 | undefined;
  readonly entry: AtomicBoundEntry | undefined;
}

/** A stage's entry's `source`: its list's file, URL, SHA-256, fetch dates and citations. */
function sourceOf(list: (typeof KURUCZ_LISTS)[number]): string {
  const partitions =
    "an LTE line sum with Barklem and Collet 2016's partition functions (Astron. Astrophys. 588 (2016) A96, via CDS VizieR J/A+A/588/A96)";
  if (list.key === "Zn+") {
    return (
      `Zn II's 17 lines over 350-800 nm (air) that ${ASD_CITATION} lists (${ASD_ZN2.name}, ${ASD_ZN2_URL}, SHA-256 ${ASD_ZN2.sha256}, fetched 2026-10-19), a finding aid for the transitions, Kurucz's ${list.name} holding none in the range: ` +
      `their levels from ${MARTIN_KAUFMAN_1970}, the vacuum wavelengths their differences; ` +
      `A from ASD for 7 lines (5p-7s, 5p-5d, 4d 2D3/2-4f and 4d 2D5/2-4f 2F7/2), whose primary paper ASD cites, ${MAYO_2006}, is closed with no open copy found, so that ASD is their source (a stated exception, decision-r08-cmf-licence.md §7.3.3); ` +
      `computed A from ${NANDY_SAHOO_2015} for 5 (5s-5p, 5p-6s, and 4d 2D5/2-4f 2F5/2, which ASD gives no A); ` +
      `a computed line strength from ${KISIELIUS_2015} for 589.60 nm (4p 2P1/2-3d9 4s2 2D3/2); ` +
      `and the bound A = 3e8 s-1, above every A ASD lists for a Zn II line in the range, for the 4 that take no value from an open paper: 621.64 and 748.09 nm (4p 2P3/2-3d9 4s2 2D), no lifetime of 3d9 4s2 2D having been measured and their computed line strengths (Kisielius et al. 2015) being only in their CDS catalogue, CC BY-NC and not an input, R. L. Kurucz's computed atoms/3001/gf3001.pos (2012) putting them 4.0 and 3.4 dex below the bound; and 380.74 and 384.14 nm (5p-6d), none found; so that the entry is estimated and its largest cells are the bound's; ${partitions}`
    );
  }
  return `R. L. Kurucz's line list ${list.name} (${KURUCZ_URL}${list.name}, SHA-256 ${list.sha256}, fetched 2026-10-16 and 2026-10-19; ${KURUCZ_CITATION}), ${partitions}`;
}

/** The ionisation limit as the method states it, cm⁻¹. */
function limitText(data: SahaData, list: AtomicStage): string {
  return `${(ionisationLimitEv(data, list) * PER_CM_PER_EV).toFixed(1)} cm-1`;
}

/** A stage's largest cell over its nodes, as its entry's method names it. */
export interface LargestCell {
  readonly temperatureK: number;
  /** The cell, 0–380, centred on 380 + cell nm. */
  readonly cell: number;
  /** Its largest line, or `undefined` for a stage with no line in range. */
  readonly line: KuruczLine | undefined;
  /** The lines in range from a level below the limit, at that node. */
  readonly linesInRange: number;
}

/** A stage's cell means at its nodes and its largest cell. */
export interface StageCellMeans {
  /** At each node, the 381 cell means rounded up to three figures, m² an absorber. */
  readonly rows: ReadonlyArray<ReadonlyArray<number>>;
  readonly largest: LargestCell;
}

/** The cell means of a stage at nodes, each rounded up to three figures, and its largest cell. */
export function stageCellMeans(
  data: SahaData,
  list: AtomicStage,
  lines: ReadonlyArray<KuruczLine>,
  nodesK: ReadonlyArray<number>,
): StageCellMeans {
  const limitEv = ionisationLimitEv(data, list);
  let largest: LargestCell & { readonly valueM2: number } = {
    temperatureK: 0,
    cell: 0,
    valueM2: -1,
    line: undefined,
    linesInRange: 0,
  };
  const rows = nodesK.map((temperatureK) => {
    const q = partitionFunctionAt(data.partitionFunctions, barklemName(list), temperatureK);
    const means: CellMeans = lineCellMeansM2(lines, temperatureK, q, limitEv);
    for (const [c, v] of means.cellMeansM2.entries()) {
      if (v > largest.valueM2) {
        largest = {
          temperatureK,
          cell: c,
          valueM2: v,
          line: means.leading[c],
          linesInRange: means.linesInRange,
        };
      }
    }
    return Array.from(means.cellMeansM2, roundUpToThreeFigures);
  });
  const { temperatureK, cell, line, linesInRange } = largest;
  return { rows, largest: { temperatureK, cell, line, linesInRange } };
}

/**
 * Each answer of {@link ATOMIC_ANSWERS} and its entry: the shares decide a `byShares` stage
 * ({@link belowKOf}); an interim or pending stage takes T4.c's nodes, a stated none its `belowK`
 * and He's nodes above ({@link nodesFrom}).
 */
export function atomicAnswers(
  data: SahaData,
  linesOf: (list: (typeof KURUCZ_LISTS)[number]) => ReadonlyArray<KuruczLine>,
): ReadonlyArray<AtomicAnswer> {
  return ATOMIC_ANSWERS.map(({ key, answer }) => {
    const list = KURUCZ_LISTS.find((l) => l.key === key);
    if (list === undefined) {
      throw new Error(`no line list for ${key}`);
    }
    const lines = linesOf(list);
    let shares: ReturnType<typeof largestShares> | undefined;
    switch (answer) {
      case "interim":
        shares = undefined;
        break;
      case "byShares":
        shares = largestShares(data, list, lines);
        break;
      case "drawnFromFloor": {
        // Its table answers from its belowK; below it, its stated none.
        const drawn = largestShares(data, list, lines);
        return { key, shares: drawn, belowK: belowKOf(drawn), entry: undefined };
      }
    }
    const belowK = shares === undefined ? undefined : belowKOf(shares);
    const nodesK = belowK === undefined ? ATOMIC_NODES_K : nodesFrom(belowK);
    const { rows, largest } = stageCellMeans(data, list, lines, nodesK);
    const leading =
      largest.line === undefined ? "no line" : describeLine(list.spectrum, largest.line);
    const role =
      belowK === undefined
        ? `An interim entry (R08.T4.h), kept: R08.T4.e defers its table past its byte budget (main, 2026-10-19; decision-r08-t4e-budget section 5).`
        : `Above ${belowK} K, where ${key} is no longer stated none (${key === "O" ? "R08.T4.e" : "R08.T4.h"}: its largest bake-bin share at its reference column ${belowK === 5_000 ? "stays under 1e-3 to 5000 K" : "passes 1e-3 between 4600 and 5000 K"}).`;
    const entry: AtomicBoundEntry = {
      key,
      kind: "species",
      task: "R08.T4.e",
      basis: key === "Zn+" ? "estimated" : "measured",
      temperaturesK: nodesK,
      source: sourceOf(list),
      method:
        `LTE line sum over ${key === "Zn+" ? "the lines" : "Kurucz's lines"} with a lower level below the ionisation limit (${limitText(data, list)}, Barklem and Collet 2016's table 4): each line's pi r_e gf exp(-c2 E_low / T) / Q(T), ` +
        `${key === "Zn+" ? "" : "gf with Kurucz's hyperfine and isotope fractions, "}in the 1 nm cell of its vacuum wavelength (${key === "Zn+" ? "from its levels" : "Morton 2000's air-to-vacuum conversion above 200 nm"}), divided by the cell's width in cm-1: each 1 nm vacuum cell's mean over 380-760 nm at each node, Q(T) from Barklem and Collet 2016's table 8 (a cubic spline in log Q over log T between its nodes). ` +
        `Stimulated emission is neglected, which only raises the values. ${role} The largest cell, ${380 + largest.cell} nm at ${largest.temperatureK} K, holds ${leading}, among ${largest.linesInRange} in-range lines.`,
      cellMeansM2: rows,
    };
    return { key, shares, belowK, entry };
  });
}

/** `atomicBounds.json`'s description, in `bounds.json`'s form. */
export const ATOMIC_BOUNDS_DESCRIPTION =
  "Cross-sections of the client's pending atoms and ions added by rendering plan R08's R08.T4.h " +
  "(decision-r08-t4ef-storage and its addendum of 2026-10-19), beside bounds.json: per species " +
  "and task, its 1 nm cell-mean cross-sections over 380-760 nm at its source's temperature nodes, " +
  "cellMeansM2, m2 an absorber, one row of 381 cells a node, the cells centred on 380, 381, ..., " +
  "760 nm, each [nm - 0.5, nm + 0.5) on vacuum wavelengths, rounded up to 3 significant figures so " +
  "that each stays a bound: the interim entries of the atoms and ions R08.T4.e defers past its " +
  "byte budget (decision-r08-t4e-budget), and the entries above their limits of the stages stated " +
  "none below them (O's from R08.T4.e). Reduced values from R. L. " +
  "Kurucz's atomic line lists and, for Zn+, NIST ASD's Zn II lines with their levels from Martin " +
  "and Kaufman 1970 and their transition probabilities as its entry's source states, each with its " +
  "citation, written by apps/hyperion/scripts/atmosphereData.mjs atomic-bounds. R08.T10.a bounds " +
  "each channel's change by them at each body's own column.";

/** `atomicBounds.json`'s text in `bounds.json`'s layout: two-space JSON, each node's cells on one line. */
export function atomicBoundsText(entries: ReadonlyArray<AtomicBoundEntry>): string {
  return `{\n  "description": ${JSON.stringify(ATOMIC_BOUNDS_DESCRIPTION)},\n  "bounds": [\n${entries.map(boundEntryText).join(",\n")}\n  ]\n}\n`;
}

/** `atomic-bounds`: writes `absorbers/atomicBounds.json` and prints each answer. */
export function runAtomicBounds(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      kurucz: { type: "string" },
      barklem: { type: "string" },
      "asd-zn2": { type: "string" },
      out: { type: "string" },
    },
  });
  if (
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values["asd-zn2"] === undefined
  ) {
    throw new Error(
      "usage: atomic-bounds --kurucz <dir> --barklem <dir> --asd-zn2 <csv> [--out <json>]",
    );
  }
  const { kurucz } = values;
  const asd = values["asd-zn2"];
  const data = readSahaData(values.barklem);
  const answers = atomicAnswers(data, (list) => stageLines(list, kurucz, asd));
  for (const a of answers) {
    const top = a.shares
      ?.filter((s) => s.temperatureK <= 4_600)
      .reduce((best, s) => (s.share > best.share ? s : best));
    const largest =
      a.shares === undefined || top === undefined
        ? ""
        : `, largest share ${top.share.toExponential(2)} to 4600 K (at ${top.temperatureK} K, bin ${top.bin}), ${a.shares.at(-1)?.share.toExponential(2)} at 5000 K (bin ${a.shares.at(-1)?.bin}; ${a.shares.at(-1)?.linesSummed} lines)`;
    process.stdout.write(
      `${a.key}: ${a.entry === undefined ? `stated none below ${a.belowK ?? "-"} K, drawn above it by R08.T4.e's table` : a.belowK === undefined ? `pending at ${a.entry.temperaturesK.join(", ")} K` : `stated none below ${a.belowK} K, pending at ${a.entry.temperaturesK.join(", ")} K`}${largest}\n`,
    );
  }
  const entries = answers.flatMap((a) => (a.entry === undefined ? [] : [a.entry]));
  const out = values.out ?? "src/renderer/src/view/atmosphere/absorbers/atomicBounds.json";
  writeFileSync(out, atomicBoundsText(entries));
  process.stdout.write(`atmosphere-data: wrote ${entries.length} entries to ${out}\n`);
}

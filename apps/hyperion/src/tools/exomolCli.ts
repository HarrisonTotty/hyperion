/**
 * `node scripts/atmosphereData.mjs exomol-bands …`, `exomol-bins …`, `exomol-k-distributions …`,
 * `exomol-k-distributions-merge …` and `exomol-cell-references …` (plan R08, R08.T4.g): ExoMol's
 * temperate lines, NH₃ from C2018 and N₂O from TYM merged with HITRAN2024's bands, reduced by
 * super-lines (`superLines.ts`) on R08.T4.d's machinery, with T4.d's stride, merge and refine
 * protocol.
 *
 * @remarks
 * - `exomol-bands --species N2O --data <dir> --hitran-par <file> --cache <dir> [--jobs <n>]`, N₂O's
 *   first step: surveys TYM's lines over HITRAN2024's four bands' windows at 296 K by TYM's own
 *   bands, matches them ({@link matchBands}) and writes the matches and the TYM bands to bin apart
 *   to the cache.
 * - `exomol-bins --species <key> --data <dir> [--broad <dir>] --cache <dir> --temperatures <t,…>
 *   [--jobs <n>]` checks every input's SHA-256, sums Q(T) from the states, and streams the
 *   transition files once (`bzip2 -dc`, `jobs` at a time), binning every line at each temperature
 *   not yet in the cache, the reduction's nodes and the midpoints between them alike
 *   ({@link binTemperatures}); it writes each temperature's bins and the pass's records to the
 *   cache, outside the repository, so that a refinement pass reads only its new temperatures. For
 *   N₂O the survey's matched TYM bands are binned apart.
 * - `exomol-k-distributions --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>]
 *   [--cells <a>-<b>] [--stride <n>:<k>] [--temperatures <t,…>] --out <partial>` reduces the cells
 *   as `k-distributions` does, each node's spectrum from the cached bins (with HITRAN2024's lines
 *   line by line for N₂O from 714 nm), to a partial file.
 * - `exomol-k-distributions-merge --species <key> --cache <dir> --out-dir <dir> [--hitran-par
 *   <file> --record <json>] <partial>…` merges the partials as `k-distributions-merge` does (exit 2
 *   prints `refine: <temperatures>`; exit 0 writes `<key>-<n>.bin` and `<key>.json`), and for N₂O
 *   checks each merged cell's mean against its two lists' to 10⁻⁹ and writes the band-match record
 *   `--record` names.
 * - `exomol-cell-references --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>]
 *   --table <sidecar> --cells <nm,…> --out <json>` writes named cells' super-line curves of growth
 *   beside the reduction's `f64` curves, as `cell-references` does.
 *
 * Every input is fetched by URL and checked against its SHA-256 here; none is committed, nor any
 * decompressed, filtered or binned copy (`decision-r08-licences.md`). The bins, the records and the
 * partials stay in the cache and work directories outside the repository.
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import {
  bandKey,
  bandLabel,
  binsFileName,
  type BroadeningFile,
  cachedSpecies,
  checkFile,
  type ExoMolDefinition,
  type ExoMolSource,
  packVibration,
  parseBroadening,
  parseDefinition,
  parsePartitionFunction,
  readStates,
  streamPass,
  type SurveyedBand,
  writeBins,
} from "./exomolLines";
import {
  ammoniaRayleighCm2,
  broadeningExponentOffsets,
  broadeningRatios,
  chainColumnCm2,
  EXPONENT_OFFSET_FLOOR,
  gatedColumnCm2,
  nitrousOxideRayleighCm2,
  type NodeSpectra,
  PART_LAYOUT_TEXT,
  type RayleighCrossSection,
  type ReductionGrid,
  type SaturationCurve,
  type ShapePolicy,
  STORED_PRESSURES_PA,
} from "./kDistributions";
import {
  type CheckedFile,
  FINEST_STEP_K,
  LINE_SOURCES,
  type LineSource,
  mergePartials,
  type PartialCell,
  type PartialFile,
  readLineSpecies,
  reducePartialCell,
  type TableSource,
} from "./kDistributionsCli";
import { cellReference } from "./lineReferences";
import { LINE_CUTOFF_PER_CM, type LineSpecies, readCheckedText } from "./molecularLines";
import { mergedCell, mergedSpectra, superLineSpectra, type TransformedWindow } from "./superLines";

/** ExoMol's licence, as the database states it (read 2026-10-10, 2026-10-11 and 2026-10-13). */
export const EXOMOL_TERMS =
  "CC BY-SA 4.0: 'All data in ExoMol is released under the Creative Commons " +
  "Attribution-ShareAlike 4.0 International (CC BY-SA 4.0) licence' (https://www.exomol.com/data/licence/); " +
  "the values here are reduced from it, adapted material offered under CC BY-SA 4.0 (README.md " +
  "beside this directory, and NOTICE); the line list's files are not committed " +
  "(decision-r08-licences.md)";

/** ExoMol's database paper, cited beside each list's own (good practice; ExoMol asks for the lists'). */
const EXOMOL_2024 =
  "J. Tennyson et al., The 2024 release of the ExoMol database: molecular line lists for exoplanet " +
  "and other hot atmospheres, J. Quant. Spectrosc. Radiat. Transf. 326 (2024) 109083, DOI " +
  "10.1016/j.jqsrt.2024.109083";

/** The plain shape, T4.d's: Voigt within 25 cm⁻¹ at every pressure, `estimated` past γ_L = 0.25 cm⁻¹. */
const CUTOFF_25: ShapePolicy = {
  variants: [{ broadener: "air" }],
  cutoffPerCm: LINE_CUTOFF_PER_CM,
  pedestal: false,
  estimatedAboveHalfWidthPerCm: 0.25,
};

/**
 * NH₃'s saturation: Fray and Schmitt 2009's sublimation fit (Planet. Space Sci. 57, 2053, Eq. 4 and
 * Table 5, row NH3-1: ln(p ÷ 1 bar) = 15.96 − 3,537 ÷ T − 33,100 ÷ T² + 1.742 × 10⁶ ÷ T³ − 2.995 ×
 * 10⁷ ÷ T⁴, fitted over 15–195.41 K, its measurements at 162–195 K and the colder values a
 * thermodynamic extension; 5.7 × 10⁻⁵ Pa at 100 K and 17.8 Pa at 150 K), to their triple point,
 * 195.41 K (their Table 2); above it, Clausius–Clapeyron from the fit's own triple-point pressure,
 * 6,116 Pa, with ΔH_vap = ΔH_sub − ΔH_fus = 25.57 kJ mol⁻¹ there (ΔH_sub 31.22 kJ mol⁻¹ from the
 * fit's slope at 195.41 K, NIST's WebBook giving 31.2 after Overstreet and Giauque 1937, J. Am.
 * Chem. Soc. 59, 254; ΔH_fus 5.653 kJ mol⁻¹, a secondary compilation's value of the same
 * measurement, which NIST's WebBook does not list). A constant ΔH_vap overstates p_sat upward
 * (1.47 MPa at 300 K against NIST's 1.06 MPa), which only widens the cap. NH₃'s critical point is
 * 405.4 K (NIST's WebBook).
 */
export const NH3_SATURATION: SaturationCurve = {
  kind: "sublimationFit",
  tripleK: 195.41,
  coefficients: [15.96, -3_537, -33_100, 1.742e6, -2.995e7],
  vaporisationJPerMol: 25_570,
  criticalK: 405.4,
  source:
    "Fray and Schmitt 2009, Planet. Space Sci. 57, 2053, Eq. 4 and Table 5 (NH3-1, 15-195.41 K); " +
    "above the triple point (195.41 K) Clausius-Clapeyron at dH_vap 25.57 kJ/mol (dH_sub 31.22 from " +
    "the fit's slope less dH_fus 5.653, after Overstreet and Giauque 1937)",
};

/**
 * N₂O's saturation (R08.T4.g's ruling: N₂O condenses below its critical point, so it takes the
 * condensing cap, `decision-r08-t4d-kdist-chain.md` §§4 and 8; a derivation from the triple point,
 * as O₂'s): the triple point 182.33 K and 87.84 kPa and the critical point 309.52 K, Lemmon and
 * Span's equation of state (E. W. Lemmon and R. Span, J. Chem. Eng. Data 51 (2006) 785, as REFPROP
 * 10 and NIST's WebBook give it); ΔH_fus 1,563.0 and
 * ΔH_vap 3,958 ± 3 cal mol⁻¹ at 1 atm (R. W. Blue and W. F. Giauque, J. Am. Chem. Soc. 57 (1935)
 * 991, as H. J. Hoge, J. Res. Natl. Bur. Stand. 34 (1945) 281, quotes them on pp. 285 and 291), at
 * 4.1833 international joules a calorie, 4.1840 J: 6.540 and 16.560 kJ mol⁻¹, so that ΔH_sub =
 * 23.100 kJ mol⁻¹. p_sat is 0.31 Pa at 100 K, 3.3 kPa at 150 K, 46 kPa at 175 K and 0.70 MPa at
 * 225 K. Below the triple point a constant ΔH_sub overstates p_sat as the solid cools: +2% at
 * 175 K, +12% at 150 K and 2.4 times at 100 K against REFPROP 10's sublimation fit (Bell 2018, from
 * Blue and Giauque, Atake and Chihara 1974 and Terlain 1983; its coefficients as a public copy of
 * REFPROP's N2O.FLD gives them, itself extrapolated below 125 K), and 1.3–2.0 times at 100 K
 * against a heat-capacity estimate. Fray and Schmitt 2009 list N₂O, but their fit could not be
 * read here.
 * Above it the ideal vapour understates p_sat by up to 1.3% to 222 K and overstates it beyond
 * (+3.4% at 250 K, +8.3% at 300 K, against Lemmon and Span's through NIST's WebBook). Every error
 * but the 1.3% widens the cap.
 */
export const N2O_SATURATION: SaturationCurve = {
  kind: "clausiusClapeyron",
  tripleK: 182.33,
  triplePa: 87_840,
  sublimationJPerMol: 23_100,
  vaporisationJPerMol: 16_560,
  criticalK: 309.52,
  source:
    "triple point 182.33 K and 87.84 kPa, Tc 309.52 K (Lemmon and Span 2006, J. Chem. Eng. Data 51, " +
    "785, via REFPROP 10 and NIST); dH_fus 1563.0 and dH_vap 3958 cal/mol (Blue and Giauque 1935, " +
    "J. Am. Chem. Soc. 57, 991, as Hoge 1945 quotes them), dH_sub 23.10 and dH_vap 16.56 kJ/mol",
};

/** HITRAN2024's citation (Gordon et al. 2026), for N₂O's merged table. */
const HITRAN2024 =
  "I. E. Gordon et al., The HITRAN2024 molecular spectroscopic database, J. Quant. Spectrosc. " +
  "Radiat. Transfer 353 (2026) 109807, DOI 10.1016/j.jqsrt.2026.109807";

/** The C2018 list's directory. */
const C2018_URL = "https://www.exomol.com/db/NH3/14N-1H3/C2018/";

/** The TYM list's directory. */
const TYM_URL = "https://www.exomol.com/db/N2O/14N2-16O/TYM/";

/** C2018's transition files over 13,000–20,000 cm⁻¹, as fetched 2026-10-11 and 2026-10-12. */
const C2018_TRANSITIONS = [
  [13_000, 233_001_268, "2875f4ce391bf248384491c50921878508fe6d1a92fc5f71ca2280c6cfda7d1e"],
  [14_000, 292_806_789, "f85e9ebfa4f5ebfb96b702111f58447f802741a3ec47227fb53f1bc5598be05e"],
  [15_000, 351_312_959, "3059fa98fc92a07f9a02e1f683a2ead36e5d14d83d898f2f9eee96a81c5b363f"],
  [16_000, 417_475_282, "7cc457e226d4076de377a3b28ba2b6f40e50f1217e7e0a62fbb698b41835b3ba"],
  [17_000, 496_726_222, "90f3bb697c566d0c53ed2688c5e45150f19ca278280c3bd449c907abfd3f1129"],
  [18_000, 564_342_608, "54621bd4da2a2681ed738d824dff6e9e89cd1491c09a8c598f09341ee3a6084c"],
  [19_000, 412_824_802, "87e0fa2eeb21e2decbb88be61f85a56412dfa87d72c452839128edf530cf2431"],
] as const;

/** TYM's transition files over 13,000–20,000 cm⁻¹, as fetched 2026-10-13. */
const TYM_TRANSITIONS = [
  [13_000, 1_659_547_010, "49ffea78fb068b47d3aca799c9f944edaaa9699600f8a28531b2cbd6886b4aa9"],
  [14_000, 1_472_806_233, "1bd5da1a095df06dcee8fa0f529cd9f6f909f4f6fef8f2459db4c7b5d8de12d1"],
  [15_000, 979_440_659, "9268e88142c663e818e47c3d46273a7d080bd0911cf6190baf92fe4852080f57"],
  [16_000, 626_985_254, "9e0956b52331f7bc1fcef410cbdede2436d79ec03747cb30f9cfe1844bc84913"],
  [17_000, 378_508_706, "7edc746cae59207fe4b202bda3dcab37d256cc0cc58643aa6f83b6a2b3da2012"],
  [18_000, 212_629_537, "0efa3cd729b52a8d61a698cff658159f6fa877e07d4daeb41aa80651fcb6596b"],
  [19_000, 109_224_392, "4fec3c3e43d87a6882e23752bd2a72d2b9af25ddb69a9fb7c04fcea44cfc5ab6"],
] as const;

/** The H₂ and He `.broad` files, one level above C2018's directory. */
const NH3_BROADENERS: ReadonlyArray<BroadeningFile> = [
  {
    gas: "H2",
    name: "14N-1H3__H2.broad",
    url: "https://www.exomol.com/db/NH3/14N-1H3/14N-1H3__H2.broad",
    bytes: 5_175,
    sha256: "7124fd73993c736542ded26a1b0eeb7ae07792c453947686e6125c073675e8bb",
  },
  {
    gas: "He",
    name: "14N-1H3__He.broad",
    url: "https://www.exomol.com/db/NH3/14N-1H3/14N-1H3__He.broad",
    bytes: 3_165,
    sha256: "f2d86928b26821bf341da02b18fb6cf6ab704d370678a4e31eeb9e5037d59150",
  },
];

/** C2018 (¹⁴NH₃), ExoMol's list valid to 300 K. */
export const C2018: ExoMolSource = {
  species: "NH3",
  citation:
    "ExoMol line list C2018 for 14NH3, version 20170131 (P. A. Coles, R. I. Ovsyannikov, O. L. " +
    "Polyansky, S. N. Yurchenko and J. Tennyson, Improved potential energy surface and spectral " +
    "assignments for ammonia in the near-infrared region, J. Quant. Spectrosc. Radiat. Transf. 219 " +
    "(2018) 199-212, DOI 10.1016/j.jqsrt.2018.07.022), its 7 transition files of 13000-20000 cm-1; " +
    "the H2 and He widths from ExoMol's .broad files (H2 in Nemtchinov, Sung and Varanasi 2004's " +
    "polynomial form, JQSRT 83, 243, fitted to HITRAN's data; He from HITRAN's data, Wilzewski et " +
    "al. 2016, JQSRT 168, 193; as S. N. Yurchenko et al., " +
    `MNRAS 533 (2024) 3442, section 5.4, compile them); ${EXOMOL_2024}`,
  url: C2018_URL,
  fetched: "2026-10-13 (the checksums those of 2026-10-11 and 2026-10-12)",
  terms: EXOMOL_TERMS,
  def: {
    name: "14N-1H3__C2018.def.json",
    bytes: 4_572,
    sha256: "f8ce0d6176baf7f4d585b30a19c4a4ec6515def26d902c1da8d2f73a2ac7bba0",
  },
  states: {
    name: "14N-1H3__C2018.states.bz2",
    bytes: 34_438_850,
    sha256: "58af64e31a8de20d7b99fe685d7bbb12f06d51a34580e5ba4dc9e8030934147a",
    layout: { energy: 2, degeneracy: 3, j: 4, k: 16 },
  },
  transitions: C2018_TRANSITIONS.map(([from, bytes, sha256]) => ({
    name: `14N-1H3__C2018__${from}-${from + 1_000}.trans.bz2`,
    bytes,
    sha256,
    fromPerCm: from,
    toPerCm: from + 1_000,
  })),
  broadeners: NH3_BROADENERS,
  abundance: 0.995_872,
  binRangePerCm: [13_100, 20_000],
};

/** TYM (¹⁴N₂¹⁶O), ExoMol's list valid to 2,000 K. */
export const TYM: ExoMolSource = {
  species: "N2O",
  citation:
    "ExoMol line list TYM for 14N2-16O, version 20240527 (S. N. Yurchenko, T. M. Mellor and J. " +
    "Tennyson, ExoMol line lists - LIX. High-temperature line list for N2O, Mon. Not. R. Astron. " +
    "Soc. 534 (2024) 1364-1375, DOI 10.1093/mnras/stae2201), its 7 transition files of 13000-20000 " +
    `cm-1; ${EXOMOL_2024}`,
  url: TYM_URL,
  fetched: "2026-10-13",
  terms: EXOMOL_TERMS,
  def: {
    name: "14N2-16O__TYM.def.json",
    bytes: 5_974,
    sha256: "04819317792776e40b841a6f51b1538159cb4d4b29081a60ff9814bca1c4ac1a",
  },
  states: {
    name: "14N2-16O__TYM.states.bz2",
    bytes: 50_810_304,
    sha256: "98223720a125d31e993c30cb66551c033264be5d69a63415d309280bb8712003",
    layout: { energy: 2, degeneracy: 3, j: 4, vibration: [8, 9, 10, 11] },
  },
  transitions: TYM_TRANSITIONS.map(([from, bytes, sha256]) => ({
    name: `14N2-16O__TYM__${from}-${from + 1_000}.trans.bz2`,
    bytes,
    sha256,
    fromPerCm: from,
    toPerCm: from + 1_000,
  })),
  partitionFunction: {
    name: "14N2-16O__TYM.pf",
    bytes: 125_000,
    sha256: "ea4313508abb065bf887588755afb6399d0b68d46c447de5193900dd73bc4290",
  },
  broadeners: [],
  abundance: 0.990_333,
  binRangePerCm: [13_100, 20_000],
};

/** HITRAN2024's N₂O band assignments over 13,100–26,400 cm⁻¹, fetched 2026-10-13. */
const N2O_PAR: CheckedFile & { readonly url: string } = {
  name: "n2o-13100-26400-par.txt",
  sha256: "8556a5662a2f8924878ad25ee1c21dc55ed2c6a6c310657e8c1fa85a877aea9e",
  url: "https://hitran.org/lbl/api?iso_ids_list=21,22,23,24,25&numin=13100&numax=26400&fixwidth=0&request_params=par_line",
};

/** What every ExoMol table names: its list, grid, shape, condensation and statements. */
interface ExoMolTableBase {
  readonly exomol: ExoMolSource;
  readonly basis: "measured" | "estimated";
  readonly measuredRangeK: readonly [number, number];
  /** The first pass's temperature nodes, K. */
  readonly temperaturesK: ReadonlyArray<number>;
  readonly policy: ShapePolicy;
  /** Where the species condenses within its temperatures: the chain test's column cap. */
  readonly saturation: SaturationCurve | undefined;
  /**
   * The species' Rayleigh cross-section, as the client's `rayleigh.ts` takes it for a `measured`
   * gas, or `heliumRayleighCm2` for a species with none measured: the between-node test also
   * stops at its column (R08.T4.g's ruling, `kDistributions.ts`'s `chainColumnCm2`).
   */
  readonly rayleighCm2: RayleighCrossSection;
  readonly reduction: string;
  /** The cells reduced: every cell a line's wing reaches within the list's coverage. */
  readonly cellsNm: readonly [number, number];
}

/**
 * An ExoMol table: one list's super-lines alone (NH₃'s), or a list's super-lines merged with
 * HITRAN2024's bands (N₂O's; `decision-r08-t4d-kdist-chain.md` §11).
 */
export type ExoMolTable =
  | (ExoMolTableBase & { readonly kind: "superLines" })
  | (ExoMolTableBase & {
      readonly kind: "merged";
      /** The HITRAN2024 part: its lines' source and band assignments, and where the merge splits. */
      readonly hitran: {
        readonly source: LineSource;
        readonly par: CheckedFile & { readonly url: string };
        /** Below it, every TYM band; from it, HITRAN2024's bands and TYM's others, nm. */
        readonly splitNm: number;
        /** The wavenumbers over which TYM's bands are matched and binned apart, cm⁻¹. */
        readonly apartRangePerCm: readonly [number, number];
      };
    });

/** What every ExoMol reduction's statement shares (T4.d's statement's, on super-lines). */
const SUPER_LINE_REDUCTION =
  "Each 1 nm vacuum cell's absorption at each node from super-lines: every line's S(T) (ExoMol's " +
  "intensity formula, Q(T) summed from the states file, times HITRAN's natural abundance of the " +
  "isotopologue) binned linearly on a wavenumber grid of a twelfth of the Doppler half-width at " +
  "13100 cm-1, finer than a tenth of the narrowest Voigt half-width; at each node the bins over the " +
  "cell and 25 cm-1 each side, coarsened by a whole factor while the step stays under a tenth of " +
  "the node's narrowest Voigt half-width and a 600th of the cut-off (0.042 cm-1), their binning's " +
  "response divided out, and convolved by " +
  "FFT with Voigt profiles (Humlicek 1982) cut off 25 cm-1 from each bin at every pressure, at the " +
  "Doppler widths of the cell's two edges blended linearly in wavenumber; sampled at the cell's " +
  "points as R08.T4.d's lines are (a quarter of the narrowest Voigt half-width or finer). Every " +
  "line takes the list's default air width (cm-1 per bar at 296 K, ExoMol's unit), estimated, so " +
  "that every node's line shape is estimated; sorted into its k-distribution and reduced to " +
  "two-point Gauss-Legendre g-intervals shared by every node of the cell, by bisection to 8e-4 in T " +
  "over every column; temperature nodes 50 K apart, refined to 25 and 12.5 K where the chain test " +
  "(cubic in T on ln sigma, the rows, the read cubic in ln p on tau) fails 2e-3 within its columns " +
  "(the gated column, and the species' own Rayleigh column, R08.T4.g's ruling); ln sigma stored in " +
  "16 bits a value (decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md).";

/** The ExoMol tables of R08.T4.g. */
export const EXOMOL_TABLES: Readonly<Record<string, ExoMolTable>> = {
  NH3: {
    kind: "superLines",
    exomol: C2018,
    basis: "measured",
    measuredRangeK: [100, 300],
    temperaturesK: [100, 150, 200, 250, 300],
    policy: CUTOFF_25,
    saturation: NH3_SATURATION,
    rayleighCm2: ammoniaRayleighCm2,
    reduction:
      "NH3's 552 and 648 nm bands and every other C2018 band of 13100-20000 cm-1 (R08.T4.g). Air " +
      "widths 0.053 cm-1/bar (0.0537 cm-1/atm), n 0.5, C2018's default; H2's and He's ratios to it " +
      "and their exponents from the .broad files by |m| and K'' (m1), |m| (m0) or J'' (a0), m = -J'' " +
      "(P), J'' + 1 (R), J'' (Q), each line's S at 296 K the weight. " +
      SUPER_LINE_REDUCTION,
    cellsNm: [500, 760],
  },
  N2O: {
    kind: "merged",
    exomol: TYM,
    saturation: N2O_SATURATION,
    rayleighCm2: nitrousOxideRayleighCm2,
    basis: "measured",
    measuredRangeK: [100, 296],
    temperaturesK: [100, 150, 200, 250, 300, 350, 400, 450, 500],
    policy: CUTOFF_25,
    reduction:
      "N2O merged (decision-r08-t4d-kdist-chain.md section 11, R08.T4.g): below 714 nm every TYM " +
      "band; from 714 nm HITRAN2024's four bands (1 0 0 6, 6 0 0 3, 2 4 0 4 and 2 0 0 5 from 0 0 0 " +
      "0, line by line as R08.T4.d) and TYM's bands HITRAN2024 lacks, each band from one list " +
      "only, TYM's lines of the matched bands binned apart by their states' Herzberg quanta (v1 " +
      "v2lin l2 v3, upper and lower) and so left out there, those under HITRAN's intensity cut-off " +
      "with them. TYM's air width 0.07 cm-1/bar (0.0709 cm-1/atm), n 0.5, its default, estimated; " +
      "the broadening ratios and exponents HITRAN2024's, from its lines alone. " +
      SUPER_LINE_REDUCTION,
    cellsNm: [500, 760],
    hitran: {
      source: LINE_SOURCES["N2O"] ?? failSource("N2O"),
      par: N2O_PAR,
      splitNm: 714,
      apartRangePerCm: [13_100, 14_200],
    },
  },
};

/** A missing HITRAN source: a broken invariant of the module's own records. */
function failSource(key: string): never {
  throw new Error(`no HITRAN line source ${key}`);
}

/** An ExoMol table by its registry key. */
export function exomolTableOf(species: string | undefined): ExoMolTable | undefined {
  return EXOMOL_TABLES[species ?? ""];
}

/**
 * The temperatures a grid's reduction reads bins at: its nodes, and the midpoints between them
 * that the chain test takes, ascending.
 */
export function binTemperatures(temperaturesK: ReadonlyArray<number>): number[] {
  const out = new Set<number>(temperaturesK);
  for (let i = 0; i + 1 < temperaturesK.length; i += 1) {
    out.add(0.5 * ((temperaturesK[i] ?? 0) + (temperaturesK[i + 1] ?? 0)));
  }
  return [...out].toSorted((a, b) => a - b);
}

/** One of HITRAN's bands, from a `.par` file's records. */
export interface HitranBand {
  /** The band's key ({@link bandKey}). */
  readonly key: number;
  /** Σ S over its lines at 296 K, cm a molecule. */
  readonly intensityCm: number;
  /** Σ S ν, cm × cm⁻¹: over {@link intensityCm}, the band's intensity-weighted centre. */
  readonly intensityWavenumber: number;
  readonly lines: number;
  /** Its weakest line's S, cm a molecule: HITRAN's intensity cut-off there. */
  readonly leastIntensityCm: number;
  /** Its lowest and highest line's ν, cm⁻¹. */
  readonly fromPerCm: number;
  readonly toPerCm: number;
}

/**
 * A HITRAN global-quanta field of a linear triatomic, `v1 v2 l2 v3`, packed ({@link packVibration}).
 *
 * @throws Error for quanta that are not four integers.
 */
function parQuanta(text: string): number {
  const q = text.trim().split(/\s+/u).map(Number);
  const [v1, v2, l2, v3] = q;
  if (
    q.length !== 4 ||
    v1 === undefined ||
    v2 === undefined ||
    l2 === undefined ||
    v3 === undefined ||
    !q.every(Number.isInteger)
  ) {
    throw new Error(`HITRAN record's quanta ${JSON.stringify(text)} are not v1 v2 l2 v3`);
  }
  return packVibration(v1, v2, l2, v3);
}

/**
 * The bands of a HITRAN `.par` file's ¹⁴N₂¹⁶O records (molecule 4, local 1; 160-character records,
 * Rothman et al. 1998, Appendix A: ν in columns 4–15, S in 16–25, the upper and lower global quanta
 * `v1 v2 l2 v3` in 68–82 and 83–97).
 *
 * @throws Error for a record whose quanta are not four integers.
 */
export function hitranBands(parText: string): ReadonlyArray<HitranBand> {
  const bands = new Map<number, HitranBand>();
  for (const record of parText.split(/\r?\n/u)) {
    if (
      record.length < 97 ||
      Number(record.slice(0, 2)) !== 4 ||
      Number(record.slice(2, 3)) !== 1
    ) {
      continue;
    }
    const key = bandKey(parQuanta(record.slice(67, 82)), parQuanta(record.slice(82, 97)));
    const nu = Number(record.slice(3, 15));
    const s = Number(record.slice(15, 25));
    const b = bands.get(key);
    bands.set(key, {
      key,
      intensityCm: (b?.intensityCm ?? 0) + s,
      intensityWavenumber: (b?.intensityWavenumber ?? 0) + s * nu,
      lines: (b?.lines ?? 0) + 1,
      leastIntensityCm: Math.min(b?.leastIntensityCm ?? Infinity, s),
      fromPerCm: Math.min(b?.fromPerCm ?? Infinity, nu),
      toPerCm: Math.max(b?.toPerCm ?? -Infinity, nu),
    });
  }
  return [...bands.values()].toSorted((a, b) => b.intensityCm - a.intensityCm);
}

/** A cache's record of the passes over a list, as `exomol-bins` writes it. */
export interface CacheRecord {
  readonly species: string;
  /** The list's `def.json` as the reduction reads it (its mass and default widths). */
  readonly definition: ExoMolDefinition;
  /** Q(T) from the states, and each 1 nm cell's direct-sum mean σ, m², by temperature. */
  readonly temperatures: Readonly<
    Record<string, { readonly partitionSum: number; readonly cellMeansM2: ReadonlyArray<number> }>
  >;
  readonly partitionSum296: number;
  /** The `.pf`'s Q at 296 K, where the list serves one. */
  readonly listPartitionSum296?: number;
  readonly cellMeans296M2: ReadonlyArray<number>;
  /** The share of the 296 K direct sums binned apart (N₂O's matched bands), m². */
  readonly cellMeansApart296M2?: ReadonlyArray<number>;
  /** Σ S(296) over every binned line, cm a molecule. */
  readonly intensity296Cm: number;
  readonly broadening: ReadonlyArray<{
    readonly gas: string;
    readonly intensityHalfWidthPerBar: number;
    readonly intensityHalfWidthExponentPerBar: number;
  }>;
  /** Over the range binned apart: [key, Σ S, Σ S ν, lines] at 296 K, by band. */
  readonly bands: ReadonlyArray<readonly [number, number, number, number]>;
  /** Over the same range: [key, the lowest and the highest line's ν, cm⁻¹], by band. */
  readonly bandExtents: ReadonlyArray<readonly [number, number, number]>;
  /** The band keys binned apart. */
  readonly apart: ReadonlyArray<number>;
  readonly passes: ReadonlyArray<{
    readonly temperaturesK: ReadonlyArray<number>;
    readonly linesRead: number;
    readonly linesBinned: number;
    readonly seconds: number;
  }>;
}

/** A cache's record file. */
function recordPath(cacheDir: string, species: string): string {
  return join(cacheDir, `${species}-record.json`);
}

/** Whether a value read from a cache's record is one: a trust boundary of our own writing. */
function isRecord(value: unknown): value is CacheRecord {
  return (
    typeof value === "object" &&
    value !== null &&
    "temperatures" in value &&
    "passes" in value &&
    "broadening" in value
  );
}

/**
 * A cache's record.
 *
 * @throws Error where the cache holds none, or another's.
 */
export function readCacheRecord(cacheDir: string, species: string): CacheRecord {
  const parsed: unknown = JSON.parse(readFileSync(recordPath(cacheDir, species), "utf8"));
  if (!isRecord(parsed) || parsed.species !== species) {
    throw new Error(`${cacheDir} holds no record of ${species}'s bins`);
  }
  return parsed;
}

/** `exomol-bins`: one streaming pass, binning the temperatures not yet cached. */
export async function runExomolBins(rest: readonly string[]): Promise<void> {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      data: { type: "string" },
      broad: { type: "string" },
      cache: { type: "string" },
      temperatures: { type: "string" },
      jobs: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  const { data, cache } = values;
  if (table === undefined || data === undefined || cache === undefined) {
    throw new Error(
      "usage: exomol-bins --species <key> --data <dir> [--broad <dir>] --cache <dir> [--temperatures t,...] [--jobs n]",
    );
  }
  const source = table.exomol;
  const grid = values.temperatures?.split(",").map(Number) ?? table.temperaturesK;
  const wanted = binTemperatures(grid);
  const cached = (t: number): boolean =>
    existsSync(join(cache, binsFileName(source.species, "main", t)));
  const temperaturesK = wanted.filter((t) => !cached(t));
  if (temperaturesK.length === 0) {
    process.stdout.write(`exomol-bins: ${source.species}'s bins are cached at every temperature\n`);
    return;
  }
  process.stdout.write(`exomol-bins: checking ${source.species}'s inputs\n`);
  checkFile(join(data, source.def.name), source.def);
  checkFile(join(data, source.states.name), source.states);
  for (const file of source.transitions) {
    checkFile(join(data, file.name), file);
  }
  const definition = parseDefinition(
    readCheckedText(join(data, source.def.name), source.def.sha256),
    source.broadeners.map((file) => file.gas),
    source.def.name,
  );
  const broadening = source.broadeners.map((file) => {
    const dir = values.broad ?? data;
    return parseBroadening(readCheckedText(join(dir, file.name), file.sha256), file, definition);
  });
  let apart:
    | { readonly bands: ReadonlySet<number>; readonly rangePerCm: readonly [number, number] }
    | undefined;
  if (table.kind === "merged") {
    // TYM's bands matched to HITRAN2024's, by the survey run before binning.
    const survey = readBandSurvey(cache, source.species);
    apart = { bands: new Set(survey.apart), rangePerCm: table.hitran.apartRangePerCm };
  }
  const states = await readStates(join(data, source.states.name), source);
  const listPf =
    source.partitionFunction === undefined
      ? undefined
      : parsePartitionFunction(
          readCheckedText(
            join(data, source.partitionFunction.name),
            source.partitionFunction.sha256,
          ),
        ).get(296);
  const jobs = Number(values.jobs ?? "3");
  const { bins, record } = await streamPass(source, definition, states, data, {
    temperaturesK,
    ...(apart === undefined ? {} : { apart }),
    broadening,
    jobs,
  });
  for (const b of bins.main) {
    writeBins(join(cache, binsFileName(source.species, "main", b.temperatureK)), b);
  }
  for (const b of bins.apart ?? []) {
    writeBins(join(cache, binsFileName(source.species, "apart", b.temperatureK)), b);
  }
  const previous = existsSync(recordPath(cache, source.species))
    ? readCacheRecord(cache, source.species)
    : undefined;
  const temperatures: Record<string, { partitionSum: number; cellMeansM2: ReadonlyArray<number> }> =
    {
      ...previous?.temperatures,
    };
  for (const [t, [temperatureK, q]] of record.partitionSums.entries()) {
    temperatures[String(temperatureK)] = {
      partitionSum: q,
      cellMeansM2: Array.from(record.cellMeansM2[t] ?? []),
    };
  }
  const written: CacheRecord = {
    species: source.species,
    definition,
    temperatures,
    partitionSum296: record.partitionSum296,
    ...(listPf === undefined ? {} : { listPartitionSum296: listPf }),
    cellMeans296M2: Array.from(record.cellMeansM2[record.partitionSums.length] ?? []),
    cellMeansApart296M2: Array.from(record.cellMeansApart296M2),
    intensity296Cm: record.intensity296Cm,
    broadening: record.broadening,
    bands: [...record.bands.entries()].map(
      ([key, b]) => [key, b.intensityCm, b.intensityWavenumber, b.lines] as const,
    ),
    bandExtents: [...record.bands.entries()].map(
      ([key, b]) => [key, b.fromPerCm, b.toPerCm] as const,
    ),
    apart: [...(apart?.bands ?? [])],
    passes: [
      ...(previous?.passes ?? []),
      {
        temperaturesK,
        linesRead: record.linesRead,
        linesBinned: record.linesBinned,
        seconds: record.seconds,
      },
    ],
  };
  writeFileSync(recordPath(cache, source.species), `${JSON.stringify(written)}\n`);
  process.stdout.write(
    `exomol-bins: ${source.species} ${temperaturesK.join(",")} K: ${record.linesBinned} of ${record.linesRead} lines binned, Q(296) ${record.partitionSum296.toPrecision(7)}${listPf === undefined ? "" : ` (the list's .pf ${listPf})`}, ${record.seconds.toFixed(0)} s\n`,
  );
}

/**
 * A table's broadening records: N₂O's HITRAN2024's, from its lines alone (T4.d's, the TYM-only
 * bands entering them not at all); NH₃'s from its `.broad` sums, each gas's ratio Σ S γ_j ÷ (γ_air
 * Σ S) to the default air width and its exponent's offset n̄_j − n_air, n̄_j the S γ-weighted mean.
 */
export function tableBroadening(
  table: ExoMolTable,
  record: CacheRecord,
  hitranLines: LineSpecies | undefined,
): {
  readonly ratios: Readonly<Record<string, number>>;
  readonly exponents: Readonly<Record<string, number>>;
} {
  if (table.kind === "merged") {
    if (hitranLines === undefined) {
      throw new Error(
        `${table.exomol.species}'s table takes HITRAN2024's lines: give --hitran-lines`,
      );
    }
    return {
      ratios: broadeningRatios(hitranLines.lines),
      exponents: broadeningExponentOffsets(hitranLines.lines),
    };
  }
  const ratios: Record<string, number> = {};
  const exponents: Record<string, number> = {};
  for (const b of record.broadening) {
    ratios[b.gas] =
      b.intensityHalfWidthPerBar / (record.definition.airHalfWidthPerCmBar * record.intensity296Cm);
    const offset =
      b.intensityHalfWidthExponentPerBar / b.intensityHalfWidthPerBar -
      record.definition.airExponent;
    exponents[b.gas] = Math.abs(offset) < EXPONENT_OFFSET_FLOOR ? 0 : offset;
  }
  return { ratios, exponents };
}

/** The pressures at which a merged cell's means are recorded: 1 kPa, 0.1 and 1 MPa. */
export const MERGE_RECORD_PRESSURES_PA = [1e3, 1e5, 1e6] as const;

/** A table's node spectra from its cache, and for N₂O the merged cells' part means. */
function tableSpectra(
  table: ExoMolTable,
  cacheDir: string,
  hitranLines: LineSpecies | undefined,
  definition: ExoMolDefinition,
): {
  readonly spectra: NodeSpectra;
  readonly parts?: (
    cellNm: number,
    temperatureK: number,
    pressurePa: number,
  ) => readonly [number, number];
  /** Closes the cache's bins files; no spectrum follows. */
  readonly close: () => void;
} {
  const source = table.exomol;
  if (table.kind === "superLines") {
    const species = cachedSpecies(source, definition, cacheDir, "all");
    return { spectra: superLineSpectra(species, table.policy), close: species.close };
  }
  if (hitranLines === undefined) {
    throw new Error(`${source.species}'s table takes HITRAN2024's lines: give --hitran-lines`);
  }
  const all = cachedSpecies(source, definition, cacheDir, "all");
  const lacking = cachedSpecies(source, definition, cacheDir, "main");
  const shape = { cutoffPerCm: table.policy.cutoffPerCm, pedestal: false };
  const cache = new Map<string, TransformedWindow>();
  return {
    spectra: mergedSpectra(hitranLines, all, lacking, table.policy, table.hitran.splitNm),
    parts: (cellNm, temperatureK, pressurePa) => {
      cache.clear();
      const cell = mergedCell(cellNm, hitranLines, lacking, temperatureK, pressurePa, shape, cache);
      return [cell?.linesMeanCm2 ?? 0, cell?.superLinesMeanCm2 ?? 0];
    },
    close: () => {
      all.close();
      lacking.close();
    },
  };
}

/** The reduction's grid: its temperature nodes and the stored pressures. */
function gridOfTable(
  table: ExoMolTable,
  temperaturesK: ReadonlyArray<number> | undefined,
): ReductionGrid {
  return { temperaturesK: temperaturesK ?? table.temperaturesK, pressuresPa: STORED_PRESSURES_PA };
}

/** HITRAN2024's N₂O lines, for N₂O's merged table. */
function hitranSpecies(
  table: ExoMolTable,
  lines: string | undefined,
  tips: string | undefined,
): LineSpecies | undefined {
  if (table.kind === "superLines") {
    return undefined;
  }
  if (lines === undefined || tips === undefined) {
    throw new Error(
      `${table.exomol.species}'s table takes HITRAN2024's lines: give --hitran-lines and --tips`,
    );
  }
  return readLineSpecies(table.hitran.source, lines, tips);
}

/** `exomol-k-distributions`: reduces a range of a table's cells from its cached bins to a partial. */
export function runExomolKDistributions(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      cache: { type: "string" },
      "hitran-lines": { type: "string" },
      tips: { type: "string" },
      cells: { type: "string" },
      stride: { type: "string" },
      temperatures: { type: "string" },
      out: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  if (table === undefined || values.cache === undefined || values.out === undefined) {
    throw new Error(
      "usage: exomol-k-distributions --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>] [--cells a-b] [--stride n:k] [--temperatures t,...] --out <json>",
    );
  }
  const grid = gridOfTable(table, values.temperatures?.split(",").map(Number));
  const record = readCacheRecord(values.cache, table.exomol.species);
  const missing = binTemperatures(grid.temperaturesK).filter(
    (t) => !existsSync(join(values.cache ?? "", binsFileName(table.exomol.species, "main", t))),
  );
  if (missing.length > 0) {
    throw new Error(`the cache holds no bins at ${missing.join(", ")} K: run exomol-bins first`);
  }
  const lines = hitranSpecies(table, values["hitran-lines"], values.tips);
  const { spectra, parts, close } = tableSpectra(table, values.cache, lines, record.definition);
  const reductionSource = {
    species: table.exomol.species,
    policy: table.policy,
    molecularMassDa: record.definition.molecularMassDa,
    ...(table.saturation === undefined ? {} : { saturation: table.saturation }),
    rayleighCm2: table.rayleighCm2,
  };
  const range = values.cells?.split("-").map(Number);
  const from = range?.[0] ?? table.cellsNm[0];
  const to = range?.[1] ?? table.cellsNm[1];
  const [stride, offset] = (values.stride ?? "1:0").split(":").map(Number);
  const cells: PartialCell[] = [];
  const nP = grid.pressuresPa.length;
  try {
    for (let cellNm = from + (offset ?? 0); cellNm <= to; cellNm += stride ?? 1) {
      const reduced = reducePartialCell(cellNm, spectra, grid, reductionSource);
      if (reduced === undefined) {
        continue;
      }
      const split = table.kind === "merged" ? table.hitran.splitNm : undefined;
      if (parts === undefined || split === undefined || cellNm < split) {
        cells.push(reduced.partial);
        continue;
      }
      // The merged cell's mean at each recorded node, the table's own (its f64 g-points') beside its
      // two lists' on the same points (decision-r08-t4d-kdist-chain.md section 11, ruling 3).
      const { weights, values: sigmas } = reduced.reduced;
      const g = weights.length;
      const merge = grid.temperaturesK.flatMap((t, ti) =>
        MERGE_RECORD_PRESSURES_PA.map((p) => {
          const pi = grid.pressuresPa.findIndex((x) => Math.abs(x / p - 1) < 1e-9);
          let mean = 0;
          for (let k = 0; k < g; k += 1) {
            mean += (weights[k] ?? 0) * (sigmas[(ti * nP + pi) * g + k] ?? 0);
          }
          const [hitranMean, tymMean] = parts(cellNm, t, p);
          // m², as the table's values are.
          return [t, p, mean, hitranMean * 1e-4, tymMean * 1e-4] as const;
        }),
      );
      cells.push({ ...reduced.partial, merge });
    }
  } finally {
    close();
  }
  const { ratios, exponents } = tableBroadening(table, record, lines);
  const partial: PartialFile = {
    species: table.exomol.species,
    temperaturesK: grid.temperaturesK,
    pressuresPa: grid.pressuresPa,
    broadening: ratios,
    broadeningExponents: exponents,
    cells,
  };
  writeFileSync(values.out, `${JSON.stringify(partial)}\n`);
}

/** The largest relative difference a merged cell's mean may take from its two lists' sum: 10⁻⁹. */
export const MERGE_MEAN_TOLERANCE = 1e-9;

/** A merged cell's record failing the 10⁻⁹ test, or none. */
export function mergeFailures(
  cells: ReadonlyArray<PartialCell>,
): ReadonlyArray<{ readonly nm: number; readonly error: number }> {
  return cells.flatMap((c) =>
    (c.merge ?? []).flatMap(([, , mean, h, t]) => {
      const error = Math.abs(mean - (h + t)) / Math.max(Math.abs(mean), Number.MIN_VALUE);
      return error > MERGE_MEAN_TOLERANCE ? [{ nm: c.nm, error }] : [];
    }),
  );
}

/** A table's inputs as its sidecar lists them: every file and its SHA-256. */
function tableInputs(table: ExoMolTable): string {
  const s = table.exomol;
  const files = [
    s.def,
    s.states,
    ...s.transitions,
    ...(s.partitionFunction === undefined ? [] : [s.partitionFunction]),
    ...s.broadeners,
  ];
  const own = files.map((f) => `${f.name} ${f.sha256}`).join(", ");
  const hitran = table.kind === "merged" ? table.hitran : undefined;
  return hitran === undefined
    ? own
    : `${own}; ${hitran.source.lines.name} ${hitran.source.lines.sha256}, ${hitran.par.name} ${hitran.par.sha256}, ${hitran.source.tips.map((t) => `${t.name} ${t.sha256}`).join(", ")}`;
}

/** A table as its sidecar names it ({@link TableSource}). */
export function tableSourceOf(table: ExoMolTable): TableSource {
  const s = table.exomol;
  const hitran = table.kind === "merged" ? table.hitran : undefined;
  return {
    species: s.species,
    citation:
      hitran === undefined
        ? s.citation
        : `${s.citation}; ${HITRAN2024} (${hitran.source.citation})`,
    url: hitran === undefined ? s.url : `${s.url}; ${hitran.source.url}; ${hitran.par.url}`,
    inputs: tableInputs(table),
    fetched: s.fetched,
    terms: s.terms,
    basis: table.basis,
    measuredRangeK: table.measuredRangeK,
    policy: table.policy,
    reduction: table.reduction,
    description:
      `Line absorbers' k-distributions per 1 nm cell reduced from the ${s.citation.split(" (")[0] ?? "line list"} ` +
      "(rendering plan R08, R08.T4.g; decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md " +
      "sections 9-11), written by apps/hyperion/scripts/atmosphereData.mjs " +
      "exomol-k-distributions-merge from the fetched files named here, which are not committed. " +
      "Adapted material of the ExoMol database, offered under CC BY-SA 4.0 (README.md beside this " +
      "directory, and NOTICE). " +
      PART_LAYOUT_TEXT,
  };
}

/** Whether a value read from a partial is a {@link PartialFile}: a trust boundary of our own writing. */
function isPartial(value: unknown): value is PartialFile {
  return typeof value === "object" && value !== null && "cells" in value && "species" in value;
}

/** One of HITRAN2024's bands against TYM's, as the band survey matched them. */
export interface BandMatch {
  /** HITRAN2024's assignment, upper - lower. */
  readonly band: string;
  readonly hitranLines: number;
  /** HITRAN2024's band's 296 K intensity-weighted centre and its strength, Σ S, cm⁻¹ and cm. */
  readonly hitranCentrePerCm: number;
  readonly hitranStrengthCm: number;
  /** The TYM band taken for it, by TYM's own assignment of its states. */
  readonly tymBand: string;
  /** Its lines within the HITRAN band's window and at or above its weakest line, and theirs. */
  readonly tymLines: number;
  readonly tymCentrePerCm: number;
  readonly tymStrengthCm: number;
  readonly centreOffsetPerCm: number;
  readonly strengthRatio: number;
  /** The list each cell from 714 nm takes the band from: HITRAN2024's, always. */
  readonly kept: "HITRAN2024";
  /** `matched` where the assignments agree and so do the checks; `doubtful` otherwise. */
  readonly verdict: "matched" | "doubtful";
  readonly reason: string;
}

/**
 * The band match's centre tolerance, cm⁻¹: the two lists' 296 K intensity-weighted centres within
 * it, each over the HITRAN band's window ({@link BAND_WINDOW_MARGIN_PER_CM}) and at or above the
 * band's weakest HITRAN line, HITRAN's intensity cut-off there.
 */
export const BAND_CENTRE_TOLERANCE_PER_CM = 2;

/** The band match's strength tolerance: the two lists' 296 K strengths within this factor (the plan's). */
export const BAND_STRENGTH_FACTOR = 3;

/** How far each side of a HITRAN band's lines its window reaches, cm⁻¹, for TYM's computed positions. */
export const BAND_WINDOW_MARGIN_PER_CM = 2;

/** A survey's window: one HITRAN band's, and every TYM band's lines in it ([key, Σ S, Σ S ν, lines]). */
export interface SurveyWindow {
  readonly band: number;
  readonly fromPerCm: number;
  readonly toPerCm: number;
  readonly leastIntensityCm: number;
  readonly tym: ReadonlyArray<readonly [number, number, number, number]>;
}

/** Whether a TYM band's lines in a window agree with a HITRAN band within the tolerances. */
function agrees(h: HitranBand, intensityCm: number, intensityWavenumber: number): boolean {
  const ratio = intensityCm / h.intensityCm;
  return (
    intensityCm > 0 &&
    Math.abs(intensityWavenumber / intensityCm - h.intensityWavenumber / h.intensityCm) <=
      BAND_CENTRE_TOLERANCE_PER_CM &&
    ratio >= 1 / BAND_STRENGTH_FACTOR &&
    ratio <= BAND_STRENGTH_FACTOR
  );
}

/**
 * HITRAN2024's bands matched to TYM's (`decision-r08-t4d-kdist-chain.md` §11): by assignment
 * (upper and lower v₁ v₂ l₂ v₃, both ¹⁴N₂¹⁶O), checked by centre and strength; where TYM holds no
 * band of HITRAN's assignment in the window, or that band fails the checks, TYM's dominant band in
 * the window from the same lower level, where it agrees, `doubtful`; where none agrees, the
 * same-assignment band or else the dominant one, `doubtful`. Every TYM band taken is binned apart
 * and so left out from 714 nm, HITRAN2024's kept.
 *
 * @returns The matches, in HITRAN's bands' order, and the TYM band keys to bin apart.
 */
export function matchBands(
  hitran: ReadonlyArray<HitranBand>,
  windows: ReadonlyArray<SurveyWindow>,
): { readonly matches: ReadonlyArray<BandMatch>; readonly apart: ReadonlyArray<number> } {
  const apart = new Set<number>();
  const matches = hitran.map((h): BandMatch => {
    const window = windows.find((w) => w.band === h.key);
    const lower = h.key % 16_777_216;
    const fromLower = (window?.tym ?? []).filter(([key]) => key % 16_777_216 === lower);
    const same = fromLower.find(([key]) => key === h.key);
    const dominant = fromLower.reduce<readonly [number, number, number, number] | undefined>(
      (best, b) => (best === undefined || b[1] > best[1] ? b : best),
      undefined,
    );
    let taken: readonly [number, number, number, number] | undefined;
    let verdict: "matched" | "doubtful";
    let reason: string;
    if (same !== undefined && agrees(h, same[1], same[2])) {
      taken = same;
      verdict = "matched";
      reason = "the same assignment, within both tolerances";
    } else if (dominant !== undefined && agrees(h, dominant[1], dominant[2])) {
      taken = dominant;
      verdict = "doubtful";
      reason =
        same === undefined
          ? "TYM assigns no band so here; its dominant band in the window agrees in centre and strength"
          : "TYM's band of the same assignment fails a check; its dominant band in the window agrees";
    } else {
      taken = same ?? dominant;
      verdict = "doubtful";
      reason = "no TYM band in the window agrees in centre and strength; the nearest taken";
    }
    if (taken !== undefined) {
      apart.add(taken[0]);
    }
    if (verdict === "doubtful" && same !== undefined) {
      apart.add(same[0]);
    }
    const [key = -1, intensityCm = 0, intensityWavenumber = 0, lines = 0] = taken ?? [];
    const tymCentre = intensityCm > 0 ? intensityWavenumber / intensityCm : Number.NaN;
    return {
      band: bandLabel(h.key),
      hitranLines: h.lines,
      hitranCentrePerCm: h.intensityWavenumber / h.intensityCm,
      hitranStrengthCm: h.intensityCm,
      tymBand: key >= 0 ? bandLabel(key) : "none",
      tymLines: lines,
      tymCentrePerCm: tymCentre,
      tymStrengthCm: intensityCm,
      centreOffsetPerCm: tymCentre - h.intensityWavenumber / h.intensityCm,
      strengthRatio: intensityCm / h.intensityCm,
      kept: "HITRAN2024",
      verdict,
      reason,
    };
  });
  return { matches, apart: [...apart].toSorted((a, b) => a - b) };
}

/** A cache's band survey (`exomol-bands`), as it writes it. */
export interface BandSurvey {
  readonly species: string;
  readonly windows: ReadonlyArray<SurveyWindow>;
  readonly matches: ReadonlyArray<BandMatch>;
  readonly apart: ReadonlyArray<number>;
  readonly seconds: number;
}

/** A cache's band-survey file. */
function surveyPath(cacheDir: string, species: string): string {
  return join(cacheDir, `${species}-bands.json`);
}

/**
 * A cache's band survey.
 *
 * @throws Error where the cache holds none, or another's.
 */
export function readBandSurvey(cacheDir: string, species: string): BandSurvey {
  const parsed: unknown = JSON.parse(readFileSync(surveyPath(cacheDir, species), "utf8"));
  if (!isBandSurvey(parsed) || parsed.species !== species) {
    throw new Error(`${cacheDir} holds no band survey of ${species}: run exomol-bands first`);
  }
  return parsed;
}

/** Whether a value read from a cache is a {@link BandSurvey}: a trust boundary of our own writing. */
function isBandSurvey(value: unknown): value is BandSurvey {
  return (
    typeof value === "object" &&
    value !== null &&
    "species" in value &&
    "apart" in value &&
    "matches" in value &&
    "windows" in value
  );
}

/**
 * `exomol-bands`: N₂O's band survey, before binning: TYM's lines over HITRAN2024's bands' windows
 * at 296 K, by TYM's own band, and the matches ({@link matchBands}).
 */
export async function runExomolBands(rest: readonly string[]): Promise<void> {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      data: { type: "string" },
      "hitran-par": { type: "string" },
      cache: { type: "string" },
      jobs: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  const { data, cache } = values;
  const par = values["hitran-par"];
  if (table?.kind !== "merged" || data === undefined || cache === undefined || par === undefined) {
    throw new Error(
      "usage: exomol-bands --species N2O --data <dir> --hitran-par <file> --cache <dir> [--jobs n]",
    );
  }
  const source = table.exomol;
  const range = table.hitran.apartRangePerCm;
  checkFile(join(data, source.states.name), source.states);
  for (const file of source.transitions.filter(
    (f) => f.toPerCm > range[0] && f.fromPerCm < range[1],
  )) {
    checkFile(join(data, file.name), file);
  }
  const hitran = hitranBands(readCheckedText(par, table.hitran.par.sha256));
  const windows = hitran.map((h) => ({
    band: h.key,
    fromPerCm: h.fromPerCm - BAND_WINDOW_MARGIN_PER_CM,
    toPerCm: h.toPerCm + BAND_WINDOW_MARGIN_PER_CM,
    leastIntensityCm: h.leastIntensityCm,
  }));
  const definition = parseDefinition(
    readCheckedText(join(data, source.def.name), source.def.sha256),
    [],
    source.def.name,
  );
  const states = await readStates(join(data, source.states.name), source);
  const { record } = await streamPass(source, definition, states, data, {
    temperaturesK: [],
    apart: { bands: new Set(), rangePerCm: range },
    broadening: [],
    jobs: Number(values.jobs ?? "2"),
    survey: windows,
    filesPerCm: range,
  });
  const surveyed = windows.map((w, i): SurveyWindow => ({
    band: w.band,
    fromPerCm: w.fromPerCm,
    toPerCm: w.toPerCm,
    leastIntensityCm: w.leastIntensityCm,
    tym: [...(record.surveyed[i] ?? new Map<number, SurveyedBand>()).entries()]
      .map(([key, b]) => [key, b.intensityCm, b.intensityWavenumber, b.lines] as const)
      .toSorted((a, b) => b[1] - a[1]),
  }));
  const { matches, apart } = matchBands(hitran, surveyed);
  const survey: BandSurvey = {
    species: source.species,
    windows: surveyed,
    matches,
    apart,
    seconds: record.seconds,
  };
  writeFileSync(surveyPath(cache, source.species), `${JSON.stringify(survey)}\n`);
  for (const m of matches) {
    process.stdout.write(
      `exomol-bands: ${m.band} (${m.hitranLines} lines, ${m.hitranCentrePerCm.toFixed(2)} cm-1, ${m.hitranStrengthCm.toExponential(3)} cm): TYM ${m.tymBand} (${m.tymLines} lines, ${m.tymCentrePerCm.toFixed(2)}, ${m.tymStrengthCm.toExponential(3)}), offset ${m.centreOffsetPerCm.toFixed(2)}, ratio ${m.strengthRatio.toFixed(3)}, ${m.verdict}\n`,
    );
  }
  process.stdout.write(
    `exomol-bands: ${apart.length} TYM bands binned apart (${apart.map(bandLabel).join("; ")}), ${record.seconds.toFixed(0)} s\n`,
  );
}

/** `exomol-k-distributions-merge`: merges a table's partials as `k-distributions-merge` does. */
export function runExomolKDistributionsMerge(rest: readonly string[]): number {
  const { values, positionals } = parseArgs({
    args: [...rest],
    allowPositionals: true,
    options: {
      species: { type: "string" },
      cache: { type: "string" },
      "out-dir": { type: "string" },
      "hitran-par": { type: "string" },
      record: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  const outDir = values["out-dir"];
  if (
    table === undefined ||
    outDir === undefined ||
    values.cache === undefined ||
    positionals.length === 0
  ) {
    throw new Error(
      "usage: exomol-k-distributions-merge --species <key> --cache <dir> --out-dir <dir> [--hitran-par <file> --record <json>] <partial>...",
    );
  }
  const partials = positionals.map((path): PartialFile => {
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    if (!isPartial(parsed) || parsed.species !== table.exomol.species) {
      throw new Error(`${path} is not a partial of ${table.exomol.species}`);
    }
    return parsed;
  });
  const failures = mergeFailures(partials.flatMap((p) => p.cells));
  if (failures.length > 0) {
    for (const f of failures) {
      process.stdout.write(
        `merge fails: ${f.nm} nm, the table's mean ${f.error.toExponential(2)} from its lists'\n`,
      );
    }
    process.stdout.write(`refused: ${failures.length} merged means past ${MERGE_MEAN_TOLERANCE}\n`);
    return 3;
  }
  const outcome = mergePartials(tableSourceOf(table), partials, (line) =>
    process.stdout.write(line),
  );
  let code: number;
  switch (outcome.kind) {
    case "refine":
      process.stdout.write(`refine: ${outcome.temperaturesK.join(",")}\n`);
      process.stdout.write(
        `exomol-bins then takes these nodes, and their midpoints, for the cache (the finest step ${FINEST_STEP_K} K)\n`,
      );
      code = 2;
      break;
    case "refused":
      process.stdout.write(`refused: ${outcome.reason}\n`);
      code = 3;
      break;
    case "written":
      for (const part of outcome.parts) {
        writeFileSync(join(outDir, part.file), part.bytes);
      }
      writeFileSync(join(outDir, `${table.exomol.species}.json`), outcome.sidecar);
      process.stdout.write(outcome.summary);
      code = 0;
      break;
  }
  if (code === 0 && table.kind === "merged" && values.record !== undefined) {
    const par = values["hitran-par"];
    if (par === undefined) {
      throw new Error("the band-match record needs --hitran-par");
    }
    const record = readCacheRecord(values.cache, table.exomol.species);
    const hitran = hitranBands(readCheckedText(par, table.hitran.par.sha256));
    writeFileSync(values.record, mergeRecordText(table, hitran, record, partials, values.cache));
  }
  return code;
}

/** A number to some significant figures in its shortest decimal form. */
function figures(value: number, digits: number): string {
  if (!Number.isFinite(value)) {
    // JSON has no NaN: a band TYM holds none of has no centre.
    return "null";
  }
  return value === 0 ? "0" : String(Number(value.toPrecision(digits)));
}

/** The wavenumbers whose lines reach a cell: the cell and the cut-off each side, cm⁻¹. */
function windowOf(nm: number): readonly [number, number] {
  return [1e7 / (nm + 0.5) - LINE_CUTOFF_PER_CM, 1e7 / (nm - 0.5) + LINE_CUTOFF_PER_CM];
}

/**
 * N₂O's merge record (`exomol/n2oMerge.json`): the band-match table, and each merged cell's bands
 * by list and its means at the recorded nodes (the table's f64, HITRAN2024's and TYM's), full
 * precision, for the 10⁻⁹ test.
 */
function mergeRecordText(
  table: ExoMolTable,
  hitran: ReadonlyArray<HitranBand>,
  record: CacheRecord,
  partials: ReadonlyArray<PartialFile>,
  cacheDir: string,
): string {
  const { matches } = readBandSurvey(cacheDir, table.exomol.species);
  const apart = new Set(record.apart);
  const split = table.kind === "merged" ? table.hitran.splitNm : 714;
  const cells = partials
    .flatMap((p) => p.cells)
    .filter((c) => c.nm >= split)
    .toSorted((a, b) => a.nm - b.nm);
  const description =
    "N2O's merged k-distribution (rendering plan R08, R08.T4.g; decision-r08-t4d-kdist-chain.md " +
    "section 11): HITRAN2024's bands against TYM's of the same assignment (v1 v2 l2 v3, upper - " +
    `lower, 14N2-16O), each match checked by its 296 K intensity-weighted centre (within ${BAND_CENTRE_TOLERANCE_PER_CM} cm-1) ` +
    `and strength (within a factor of ${BAND_STRENGTH_FACTOR}), HITRAN2024's kept and TYM's left out; and, from ${split} nm, ` +
    "each cell's bands by list within 25 cm-1 of it (TYM's by name where a band's 296 K strength " +
    "reaches HITRAN2024's cut-off, its weakest line's, the weaker counted), and its mean " +
    "cross-section (m2 a molecule) at " +
    "each temperature node and 1 kPa, 0.1 and 1 MPa: the table's f64 g-points' Sum w sigma, " +
    "HITRAN2024's lines' and TYM's added bands' on the same points. Written by " +
    "apps/hyperion/scripts/atmosphereData.mjs exomol-k-distributions-merge; adapted material of the " +
    "ExoMol database, offered under CC BY-SA 4.0 (README.md beside this file, and NOTICE).";
  const bandRows = matches.map(
    (m) =>
      `    {"band": ${JSON.stringify(m.band)}, "hitranLines": ${m.hitranLines}, "hitranCentrePerCm": ${figures(m.hitranCentrePerCm, 9)}, "hitranStrengthCm": ${figures(m.hitranStrengthCm, 6)}, "tymBand": ${JSON.stringify(m.tymBand)}, "tymLines": ${m.tymLines}, "tymCentrePerCm": ${figures(m.tymCentrePerCm, 9)}, "tymStrengthCm": ${figures(m.tymStrengthCm, 6)}, "centreOffsetPerCm": ${figures(m.centreOffsetPerCm, 4)}, "strengthRatio": ${figures(m.strengthRatio, 4)}, "kept": ${JSON.stringify(m.kept)}, "verdict": ${JSON.stringify(m.verdict)}, "reason": ${JSON.stringify(m.reason)}}`,
  );
  // TYM's bands over the range binned apart (612,365 for N₂O), their extents by key. Each cell
  // lists by name those whose 296 K strength reaches HITRAN2024's own cut-off, its weakest line's
  // S in any of its bands, the bands HITRAN would hold were they in it, and counts the rest.
  const extents = new Map(record.bandExtents.map(([key, from, to]) => [key, [from, to] as const]));
  const cutoffCm = Math.min(...hitran.map((b) => b.leastIntensityCm));
  const added = record.bands.filter(([key, intensityCm]) => !apart.has(key) && intensityCm > 0);
  const cellRows = cells.map((c) => {
    const [lo, hi] = windowOf(c.nm);
    const hitranIn = hitran
      .filter((b) => b.toPerCm >= lo && b.fromPerCm <= hi)
      .map((b) => bandLabel(b.key));
    const reaching = added.filter(([key]) => {
      const extent = extents.get(key);
      return extent !== undefined && extent[1] >= lo && extent[0] <= hi;
    });
    const tymIn = reaching
      .filter(([, intensityCm]) => intensityCm >= cutoffCm)
      .map(([key]) => bandLabel(key));
    const means = (c.merge ?? [])
      .map(([t, p, mean, h, y]) => `[${t}, ${p}, ${mean}, ${h}, ${y}]`)
      .join(", ");
    return `    {"nm": ${c.nm}, "hitranBands": ${JSON.stringify(hitranIn)}, "tymBands": ${JSON.stringify(tymIn)}, "tymWeakerBands": ${reaching.length - tymIn.length}, "means": [${means}]}`;
  });
  return `{\n  "description": ${JSON.stringify(description)},\n  "bands": [\n${bandRows.join(",\n")}\n  ],\n  "cells": [\n${cellRows.join(",\n")}\n  ]\n}\n`;
}

/** `exomol-cell-references`: named cells' super-line curves and the reduction's f64 curves. */
export function runExomolCellReferences(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      cache: { type: "string" },
      "hitran-lines": { type: "string" },
      tips: { type: "string" },
      table: { type: "string" },
      cells: { type: "string" },
      out: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  if (
    table === undefined ||
    values.cache === undefined ||
    values.table === undefined ||
    values.cells === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: exomol-cell-references --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>] --table <sidecar.json> --cells <nm,...> --out <json>",
    );
  }
  const sidecar: unknown = JSON.parse(readFileSync(values.table, "utf8"));
  if (
    typeof sidecar !== "object" ||
    sidecar === null ||
    !("temperaturesK" in sidecar) ||
    !Array.isArray(sidecar.temperaturesK)
  ) {
    throw new Error(`${values.table} is not a k-distribution sidecar`);
  }
  const grid = gridOfTable(table, sidecar.temperaturesK.map(Number));
  const lines = hitranSpecies(table, values["hitran-lines"], values.tips);
  const { definition } = readCacheRecord(values.cache, table.exomol.species);
  const { spectra, close } = tableSpectra(table, values.cache, lines, definition);
  const source = table.exomol;
  const cellsNm = values.cells.split(",").map(Number);
  let entries: ReturnType<typeof cellReference>[];
  try {
    entries = cellsNm.map((cellNm) => {
      const cap = (t: number, p: number): number =>
        gatedColumnCm2(table.saturation, cellNm, t, p, definition.molecularMassDa);
      const chainCap = (t: number, p: number): number =>
        chainColumnCm2(
          table.saturation,
          cellNm,
          t,
          p,
          definition.molecularMassDa,
          table.rayleighCm2,
        );
      const entry = cellReference(cellNm, spectra, grid, table.policy, cap, chainCap);
      process.stdout.write(`exomol-cell-references: ${source.species} ${cellNm} nm\n`);
      return entry;
    });
  } finally {
    close();
  }
  writeFileSync(values.out, `${JSON.stringify({ species: source.species, cells: entries })}\n`);
}

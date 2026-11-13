/**
 * `node scripts/atmosphereData.mjs exomol-bands …`, `exomol-bins …`, `exomol-projection …`,
 * `exomol-k-distributions …`, `exomol-k-distributions-merge …` and `exomol-cell-references …` (plan
 * R08, R08.T4.g and R08.T4.f): ExoMol's temperate lines, NH₃ from C2018 and N₂O from TYM merged
 * with HITRAN2024's bands (R08.T4.g), and its hot molecules (R08.T4.f), reduced by super-lines
 * (`superLines.ts`) on R08.T4.d's machinery, with T4.d's stride, merge and refine protocol.
 *
 * @remarks
 * R08.T4.f's tables take decision-r08-t4ef-storage's economies on that protocol, as R08.T4.e's
 * atoms do: E1′'s ladder from each species' floor (decision-r08-t4e-budget §1.4), refined to a
 * quarter of E1's step; E3's thinned pressures with the alternates and midpoints the merge asks for
 * (`refine-pressures:`); and E4's raised first node where its hold passes. Toto's TiO bins its five
 * Ti isotopologues into one set of bins.
 *
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
 * - `exomol-projection --species <key> --cache <dir> --out <json>` (R08.T4.f) reduces three sample
 *   cells (the strongest, a median one and a band edge, by the cache's direct sums at the middle
 *   node) over the table's whole ladder on E3's pressures, keeping the alternates and midpoints they
 *   ask for and counting the intervals they would refine, and projects the table's bytes from their
 *   mean G before any full reduction (decision-r08-t4ef-storage §1.4, "each task projects its bytes
 *   first").
 * - `exomol-k-distributions --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>]
 *   [--cells <a>-<b>] [--stride <n>:<k>] [--temperatures <t,…>] [--pressures <p,…>] --out
 *   <partial>` reduces the cells as `k-distributions` does, each node's spectrum from the cached
 *   bins (with HITRAN2024's lines line by line for N₂O from 714 nm), to a partial file; an R08.T4.f
 *   table on E3's thinned pressures unless given.
 * - `exomol-k-distributions-merge --species <key> --cache <dir> --out-dir <dir> [--hitran-par
 *   <file> --record <json>] <partial>…` merges the partials as `k-distributions-merge` does (exit 2
 *   prints `refine: <temperatures>`, and on a thinned table `refine-pressures: <pressures>`; exit 0
 *   writes `<key>-<n>.bin` and `<key>.json`), and for N₂O
 *   checks each merged cell's mean against its two lists' to 10⁻⁹ and writes the band-match record
 *   `--record` names.
 * - `exomol-cell-references --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>]
 *   --table <sidecar> --cells <nm,…> --out <json> [--into]` writes named cells' super-line curves of
 *   growth beside the reduction's `f64` curves, as `cell-references` does; with `--into`, as the
 *   species' entry of a file of several (`{"references": […]}`, R08.T4.f's), in place of any it held.
 *
 * Every input is fetched by URL and checked against its SHA-256 here; none is committed, nor any
 * decompressed, filtered or binned copy (`decision-r08-licences.md`). The bins, the records and the
 * partials stay in the cache and work directories outside the repository.
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import { ladderFinestStepK, ladderTemperatures } from "./atomicKDistributions";
import { tableBytes, THINNED_PRESSURES_PA } from "./atomicKDistributionsCli";
import {
  type ApartBands,
  type AssumedWidth,
  bandKey,
  bandLabel,
  binsFileName,
  type BroadeningFile,
  cachedSpecies,
  checkFile,
  type ExoMolDefinition,
  type ExoMolFile,
  type ExoMolFilesSource,
  type ExoMolSource,
  packVibration,
  partitionFileOf,
  type PassBins,
  type PassRecord,
  parseBroadening,
  parsePartitionFunction,
  readDefinition,
  readStates,
  type RepackSource,
  type StatesLayout,
  streamPass,
  type SurveyedBand,
  type TransitionFile,
  writeBins,
} from "./exomolLines";
import {
  ammoniaRayleighCm2,
  belowGridError,
  broadeningExponentOffsets,
  broadeningRatios,
  CHAIN_BOUND,
  chainColumnCm2,
  chainRowsPa,
  EXPONENT_OFFSET_FLOOR,
  gatedColumnCm2,
  carbonMonoxideRayleighCm2,
  heliumRayleighCm2,
  hydrogenSulphideRayleighCm2,
  latticePressurePa,
  nitrogenRayleighCm2,
  nitrousOxideRayleighCm2,
  type NodeSpectra,
  PART_LAYOUT_TEXT,
  raisedFirstNodeChecksPa,
  type RayleighCrossSection,
  type ReductionGrid,
  type SaturationCurve,
  type ShapePolicy,
  STORED_PRESSURES_PA,
  waterRayleighCm2,
} from "./kDistributions";
import {
  type CellReductionSource,
  type CheckedFile,
  CKD_CONVENTION,
  FINEST_STEP_K,
  LINE_SOURCES,
  type LineSource,
  mergePartials,
  type PartialCell,
  type PartialFile,
  readLineSpecies,
  reducePartialCell,
  sortFailures,
  type TableSource,
  withKeptPressures,
} from "./kDistributionsCli";
import { cellReference } from "./lineReferences";
import { LINE_CUTOFF_PER_CM, type LineSpecies, readCheckedText } from "./molecularLines";
import {
  applyStimulatedEmission,
  binStepPerCm,
  emptyBins,
  mergedCell,
  mergedSpectra,
  type SuperLineBins,
  superLineSpectra,
  type TransformedWindow,
} from "./superLines";

/**
 * ExoMol's licence, as the database states it (read 2026-10-10, 2026-10-11, 2026-10-13 and
 * 2026-10-29).
 */
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
export const C2018: ExoMolFilesSource = {
  kind: "exomol",
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
    format: "json",
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
  partition: { from: "states", check: undefined },
  broadeners: NH3_BROADENERS,
  abundance: 0.995_872,
  binRangePerCm: [13_100, 20_000],
};

/** TYM (¹⁴N₂¹⁶O), ExoMol's list valid to 2,000 K. */
export const TYM: ExoMolFilesSource = {
  kind: "exomol",
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
    format: "json",
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
  partition: {
    from: "states",
    check: {
      name: "14N2-16O__TYM.pf",
      bytes: 125_000,
      sha256: "ea4313508abb065bf887588755afb6399d0b68d46c447de5193900dd73bc4290",
    },
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
 * An ExoMol table: R08.T4.g's, one list's super-lines alone (NH₃'s) or a list's super-lines merged
 * with HITRAN2024's bands (N₂O's; `decision-r08-t4d-kdist-chain.md` §11), on the 30 standard
 * pressures with intervals refined to 12.5 K; or R08.T4.f's hot ones ({@link HotTable}).
 */
export type ExoMolTable =
  | (ExoMolTableBase & { readonly kind: "superLines" })
  | HotTable
  | (ExoMolTableBase & {
      readonly kind: "merged";
      /** TYM's own files: its states' quanta match its bands to HITRAN2024's. */
      readonly exomol: ExoMolFilesSource;
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

/**
 * An R08.T4.f table: one list's super-lines, with its other isotopologues' in the same bins, on
 * R08.T4.f's grid rules: E1′'s ladder, E3's thinned pressures with the alternates and midpoints the
 * merge asks for, E4's raised first node, intervals refined to a quarter of E1's step
 * (decision-r08-t4ef-storage §1.4, decision-r08-t4e-budget §1.4).
 */
export interface HotTable extends ExoMolTableBase {
  readonly kind: "hot";
  /**
   * The list's other isotopologues, binned with it into one set of bins, each at its own Q and
   * abundance (Toto's four minor Ti isotopologues), or none.
   */
  readonly isotopologues: ReadonlyArray<ExoMolFilesSource>;
  /** What the sidecar records beyond the reduction: the range, holds and choices, each with why. */
  readonly notes: string;
}

/** The task whose grid rules a table takes, and what follows from them. */
export interface TaskRules {
  readonly task: "R08.T4.g" | "R08.T4.f";
  /** Whether a run may give the grid's pressures: R08.T4.f's alternates and midpoints. */
  readonly ownPressures: boolean;
  /** The finest step a failing interval refines to from its lower node, K. */
  readonly finestStepK: (lowerK: number) => number;
  /** How the merge names that step where it asks for a refinement. */
  readonly finestStepText: string;
  /** The rulings the sidecar's description names. */
  readonly rulings: string;
}

/** R08.T4.g's rules: the 30 standard pressures, intervals refined to 12.5 K. */
const T4G_RULES: TaskRules = {
  task: "R08.T4.g",
  ownPressures: false,
  finestStepK: () => FINEST_STEP_K,
  finestStepText: `the finest step ${FINEST_STEP_K} K`,
  rulings:
    "rendering plan R08, R08.T4.g; decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md " +
    "sections 9-11",
};

/** R08.T4.f's rules ({@link HotTable}). */
const T4F_RULES: TaskRules = {
  task: "R08.T4.f",
  ownPressures: true,
  finestStepK: ladderFinestStepK,
  finestStepText: "the finest step a quarter of E1's",
  rulings:
    "rendering plan R08, R08.T4.f; decision-r08-t4ef-storage; decision-r08-t4e-budget; " +
    "decision-r08-t4d-kdist-chain.md sections 9-11",
};

/** A table's task rules, by its kind. */
export function rulesOf(table: ExoMolTable): TaskRules {
  let rules: TaskRules;
  switch (table.kind) {
    case "superLines":
    case "merged":
      rules = T4G_RULES;
      break;
    case "hot":
      rules = T4F_RULES;
      break;
  }
  return rules;
}

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

// --- R08.T4.f's hot lists, fetched from the database's archive on Zenodo ---

/**
 * The terms of R08.T4.f's lists: the database's licence, and its archive's records', each read at
 * its fetch.
 */
const EXOMOL_ARCHIVE_TERMS =
  "CC BY-SA 4.0, the ExoMol database's licence ('All data in ExoMol is released under the Creative " +
  "Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0) licence', " +
  "https://www.exomol.com/data/licence/, as read 2026-10-10 to 2026-10-13); the files fetched from " +
  "the database's archive of each dataset on Zenodo, whose records state CC BY 4.0 (read " +
  "2026-10-29); the values here are reduced from them, adapted material offered under CC BY-SA 4.0 " +
  "(README.md beside this directory, and NOTICE); the line lists' files are not committed " +
  "(decision-r08-licences.md)";

/** When and where R08.T4.f's lists were fetched. */
const ARCHIVE_FETCHED =
  "2026-10-29, from the database's archive on Zenodo (ExoMol's own site was not reachable from the " +
  "reducing machine)";

/** The wavenumbers R08.T4.f bins, cm⁻¹: every line whose 25 cm⁻¹ wing reaches 380–760 nm. */
const HOT_BIN_RANGE_PER_CM: readonly [number, number] = [13_100, 26_400];

/** A list's archive record: its Zenodo record's files' page and its DOI. */
function archiveUrl(record: number): string {
  return `https://zenodo.org/records/${record} (DOI 10.5281/zenodo.${record})`;
}

/** A fetched file: its name, size and SHA-256. */
function fetched(name: string, bytes: number, sha256: string): ExoMolFile {
  return { name, bytes, sha256 };
}

/**
 * An R08.T4.f list as the database's archive holds it: its `.def` text, states, `.pf` (whose Q it
 * takes) and transition files, over {@link HOT_BIN_RANGE_PER_CM}.
 */
function archivedList(list: {
  readonly species: string;
  readonly citation: string;
  readonly record: number;
  readonly def: ExoMolFile;
  readonly states: ExoMolFile;
  readonly layout: StatesLayout;
  readonly pf: ExoMolFile;
  readonly transitions: ReadonlyArray<TransitionFile>;
  readonly abundance: number;
  readonly assumedWidth?: AssumedWidth;
  readonly maxDeltaV?: number;
  readonly intensityScale?: ExoMolSource["intensityScale"];
}): ExoMolFilesSource {
  return {
    kind: "exomol",
    species: list.species,
    citation: `${list.citation}; ${EXOMOL_2024}`,
    url: archiveUrl(list.record),
    fetched: ARCHIVE_FETCHED,
    terms: EXOMOL_ARCHIVE_TERMS,
    def: { ...list.def, format: "text" },
    states: { ...list.states, layout: list.layout },
    transitions: list.transitions,
    partition: { from: "pf", file: list.pf },
    broadeners: [],
    abundance: list.abundance,
    binRangePerCm: HOT_BIN_RANGE_PER_CM,
    ...(list.assumedWidth === undefined ? {} : { assumedWidth: list.assumedWidth }),
    ...(list.maxDeltaV === undefined ? {} : { maxDeltaV: list.maxDeltaV }),
    ...(list.intensityScale === undefined ? {} : { intensityScale: list.intensityScale }),
  };
}

/** A list held in one transition file over every wavenumber. */
function oneFile(name: string, bytes: number, sha256: string): ReadonlyArray<TransitionFile> {
  return [{ ...fetched(name, bytes, sha256), fromPerCm: 0, toPerCm: Infinity }];
}

/**
 * The representative isotopic compositions of the elements R08.T4.f's lists name that HITRAN holds
 * no abundance for, IUPAC's (J. Meija et al., Isotopic compositions of the elements 2013, Pure
 * Appl. Chem. 88 (2016) 293, Table 1): ¹H 0.999 885, ¹⁶O 0.997 57, ⁴⁶Ti 0.0825, ⁴⁷Ti 0.0744, ⁴⁸Ti
 * 0.7372, ⁴⁹Ti 0.0541, ⁵⁰Ti 0.0518, ⁵¹V 0.997 50, ⁵⁶Fe 0.917 54, ²⁸Si 0.922 23. Ti's, V's and Fe's
 * were read through `periodictable` 2.1.0's copy of CIAAW's table; H's, O's and Si's, which it
 * holds only as CIAAW 2021's intervals, as NIST's Atomic Weights and Isotopic Compositions
 * tabulates them (through `molmass`'s copy, the science check's); Table 1 itself was not reachable
 * to re-read.
 */
const ISOTOPE_FRACTIONS = {
  H1: 0.999_885,
  O16: 0.997_57,
  Ti46: 0.082_5,
  Ti47: 0.074_4,
  Ti48: 0.737_2,
  Ti49: 0.054_1,
  Ti50: 0.051_8,
  V51: 0.997_5,
  Fe56: 0.917_54,
  Si28: 0.922_23,
} as const;

/** Toto's states: ID, energy, degeneracy and J in fields 1–4 (McKemmish et al. 2019's layout). */
const TOTO_LAYOUT: StatesLayout = { energy: 2, degeneracy: 3, j: 4 };

/** Toto's paper. */
const TOTO_CITATION =
  "L. K. McKemmish, T. Masseron, H. J. Hoeijmakers, V. Perez-Mesa, S. L. Grimm, S. N. Yurchenko " +
  "and J. Tennyson, ExoMol Molecular linelists - XXXIII. The spectrum of Titanium Oxide, Mon. Not. " +
  "R. Astron. Soc. 488 (2019) 2836-2854, DOI 10.1093/mnras/stz1818";

/** One of Toto's five Ti isotopologues, by its mass number, from its archive record. */
function totoList(
  mass: 46 | 47 | 48 | 49 | 50,
  record: number,
  files: {
    readonly def: readonly [number, string];
    readonly states: readonly [number, string];
    readonly pf: readonly [number, string];
    readonly trans: readonly [number, string];
  },
): ExoMolFilesSource {
  const iso = `${mass}Ti-16O__Toto`;
  return archivedList({
    species: "TiO",
    citation: `ExoMol line list Toto for ${mass}Ti-16O, version ${mass === 48 ? "20210825, as its archive holds it (the states' MARVEL update of 2021-08-25)" : "20190514, as its archive holds it"} (${TOTO_CITATION})`,
    record,
    def: fetched(`${iso}.def`, ...files.def),
    states: fetched(`${iso}.states.bz2`, ...files.states),
    layout: TOTO_LAYOUT,
    pf: fetched(`${iso}.pf`, ...files.pf),
    transitions: oneFile(`${iso}.trans.bz2`, ...files.trans),
    abundance: ISOTOPE_FRACTIONS[`Ti${mass}`] * ISOTOPE_FRACTIONS.O16,
  });
}

/** Toto's ⁴⁸Ti¹⁶O, the main isotopologue (archive record 5716758). */
export const TOTO_48: ExoMolFilesSource = totoList(48, 5_716_758, {
  def: [6_844, "5fdf86107b5afa9f40e5a92fc8ce1d3cab547856a9e0e8545527ee38481aec60"],
  states: [5_434_787, "6bd2cb3df67dc10c2c17303098c14572b4ab5b727f61b0515b2c6c66d9439b75"],
  pf: [200_000, "38c80b7adaed26bce40f6c31f52280305626c1ea021028474f48b23d08ab9c30"],
  trans: [722_985_132, "ddd537c5be17c0ebcc06295f46ba48586a4781e4dc34d734f70a45f3cd7e053b"],
});

/** Toto's four minor Ti isotopologues (archive records 5716754, 5716756, 5716760 and 5716762). */
export const TOTO_MINOR: ReadonlyArray<ExoMolFilesSource> = [
  totoList(46, 5_716_754, {
    def: [6_757, "b685a16559f068da2eb7e5646305bf8463fe400f53737160a13bc8479dde53dd"],
    states: [5_401_846, "1d99b13b14daa17cf9f8afbd9dc039c5f13fdf0a97a040c7c3f6c6abb06c08c8"],
    pf: [200_000, "e2cf8a02b5ef66e880e8c9518dd9826b8cea90599c75f36b87f660544cd31ec2"],
    trans: [715_709_028, "84643c7fd39640d6aff10143c89a7788b58037ec14a6bcc28d6861aeb56c3c38"],
  }),
  totoList(47, 5_716_756, {
    def: [6_757, "e1bbe90a65959dd3a06a900d283076614ec0b262ac04839761f773b22e02ac99"],
    states: [5_405_680, "424ed78dd3971fd6140052272647369e228f64def2d6d2a3c4d1fa4c6b8dd844"],
    pf: [200_000, "b85a0d740b01c138200693e215fcfe7c7210edb862479cc325d963043b88086f"],
    trans: [720_485_973, "95904c33a03816e4a2f57e5c06c784a3df077f354cf5e6c89b6b73d395f921b2"],
  }),
  totoList(49, 5_716_760, {
    def: [6_757, "b100d011a514c2af9150dc577205d9c7ae01fd991bcf29cdfaa668780beb684f"],
    states: [5_409_974, "e8c6bb5bdcd45a4ef0ab10f5be8a05f6433339d9ba1c3b5dda99ae4339f59f76"],
    pf: [200_000, "216e5fab2d10d72ef3838f1c248fe46d3b768b0d61fe5314adeb685a3e4af664"],
    trans: [727_036_426, "7d4b4175e320e07174088617ab81dea09e8eef936c7c4c7f9c856089f8346428"],
  }),
  totoList(50, 5_716_762, {
    def: [6_757, "1d2be786d617298d4c35935e4aae72cc4615314b23d8e8e44fe3cf8df73d9c9c"],
    states: [5_411_508, "10308306284a06609e8ce410df1cc5e02e0c7cf3258ccf1c71fb8f2979cfa5f1"],
    pf: [200_000, "e9da891f7695a666a8477501cca2d0ab34cfeb6a538cb1eb2d8277fabae5833d"],
    trans: [731_527_555, "394e6503d0276435345b8d64b698e935545b5d602fdc342265dd342614045fc9"],
  }),
];

/** MoLLIST's FeH, as ExoMol carries it (archive record 5362929). */
export const MOLLIST_FEH: ExoMolSource = archivedList({
  species: "FeH",
  citation:
    "ExoMol's copy of MoLLIST's line list for 56Fe-1H, version 20160726 (M. Dulick et al., " +
    "Astrophys. J. 594 (2003) 651-663, DOI 10.1086/376791; S. Wende, A. Reiners, A. Seifahrt and " +
    "P. F. Bernath, CRIRES spectroscopy and empirical line-by-line identification of FeH molecular " +
    "absorption in an M dwarf, Astron. Astrophys. 523 (2010) A58, DOI " +
    "10.1051/0004-6361/201015220, which its README names; P. F. Bernath, J. Quant. Spectrosc. " +
    "Radiat. Transf. 240 (2020) 106687)",
  record: 5_362_929,
  def: fetched(
    "56Fe-1H__MoLLIST.def",
    5_700,
    "e88184c3eed856c27c46d7396d62ed218dc1d8da91965aab6c0bf38b4b959636",
  ),
  states: fetched(
    "56Fe-1H__MoLLIST.states.bz2",
    25_982,
    "6696d4fa43bf51086b09cc75e34b53558e61c19c6ab6058842dc2ef219b2f179",
  ),
  layout: { energy: 2, degeneracy: 3, j: 4 },
  pf: fetched(
    "56Fe-1H__MoLLIST.pf",
    260_000,
    "46169ea08b0bf07af19a0167df107349ed7a632cabd4d989ec4973e7691118be",
  ),
  transitions: oneFile(
    "56Fe-1H__MoLLIST.trans.bz2",
    918_356,
    "47306bfa141be5ff4383dc6169c1a40e74ca72ff88ec495a6fac40f364a6e77e",
  ),
  abundance: ISOTOPE_FRACTIONS.Fe56 * ISOTOPE_FRACTIONS.H1,
});

/** AYT2's transition files over 13,000–27,000 cm⁻¹ (archive record 5716826). */
const AYT2_TRANSITIONS = [
  [13_000, 22_488_837, "a47f3f26ae1e43964d9045f146d18a7b7e49a137fff31a31a060716a405f0f97"],
  [14_000, 13_754_244, "7809ea0fb858a5a83a8590feeb3d0b308079172d42ecfc11bb3cd8a344c69111"],
  [15_000, 7_787_859, "66570c8e8d1ab11ad9ef8aa68e762e32653374097fff4d80818e838c3666ab3f"],
  [16_000, 3_999_537, "ee86d38e89df00f4b767d53c841ec97725ec25be848f5ea7d912470ad735b593"],
  [17_000, 1_923_094, "4999f3c4b3f566bb73c7808cc71cf05a46abd5ad8562ed8c11dccc383bbe9936"],
  [18_000, 814_589, "8986c08f6e579ab8615f62b226a7d66f12800a2dacb7fe1c639f9807a55c5812"],
  [19_000, 300_003, "2b6255f0be3938ac78addc58a54dfa8d0bcec63a59a1c59fe58c5363a823741f"],
  [20_000, 134_050, "35133c722f03a73d91bbfc642c3b27d762e392e4291cbf5df3c79cd2e7f4363d"],
  [21_000, 84_082, "9711a3649f2769fb04dac964c8a946e49f547a1d3fde99e00c0e0f03b2e0924b"],
  [22_000, 63_234, "100874bac4a67ea2378c54a668e5defc5572186c762889466d40586eb5161568"],
  [23_000, 50_171, "2b602f46b46518d29d68b99f5874512becb36e03b1be919753d22b0622526c67"],
  [24_000, 40_164, "d64fc883c0d4be5884d95b81ed565df40204695d199b7ab007a7f474e53b9214"],
  [25_000, 31_503, "7b6d03bdf6153692e56d53cfa0d72e5cd6c3f96b115ad79c4dfb012da0a62f76"],
  [26_000, 24_470, "c58fd954a5445d01511a7808b201dcbcb3e5ea2135cbb3bc45a5bfaaf360c150"],
] as const;

/** AYT2's ¹H₂³²S (archive record 5716826), valid to 1,500 K by its definition. */
export const AYT2: ExoMolSource = archivedList({
  species: "H2S",
  citation:
    "ExoMol line list AYT2 for 1H2-32S, as its archive holds it (A. A. A. Azzam, J. Tennyson, S. N. " +
    "Yurchenko and O. V. Naumenko, ExoMol molecular line lists - XVI: The rotation-vibration " +
    "spectrum of hot H2S, Mon. Not. R. Astron. Soc. 460 (2016) 4063-4074, DOI " +
    "10.1093/mnras/stw1133), its 14 transition files of 13000-27000 cm-1",
  record: 5_716_826,
  def: fetched(
    "1H2-32S__AYT2.def",
    8_282,
    "af3c48c92f3d3d0b13912de3c228ea9f6f2c9a571f4a907f9f4d5ed9dfb43a43",
  ),
  states: fetched(
    "1H2-32S__AYT2.states.bz2",
    2_277_131,
    "ca78ab7547fca4882e6eee40aa279cec5037d877ea98c303526e59efed89e9b8",
  ),
  layout: { energy: 2, degeneracy: 3, j: 4 },
  pf: fetched(
    "1H2-32S__AYT2.pf",
    78_000,
    "786beaca5627d4f318324983de8bc8104a2f11d33890657260adc610374910a3",
  ),
  transitions: AYT2_TRANSITIONS.map(([from, bytes, sha256]) => ({
    name: `1H2-32S__AYT2__${from}-${from + 1_000}.trans.bz2`,
    bytes,
    sha256,
    fromPerCm: from,
    toPerCm: from + 1_000,
  })),
  // HITRAN's natural abundance of H₂³²S (hitran.org/docs/iso-meta/, molecule 31, local 1).
  abundance: 0.949_884,
});

/** SiOUVenIR's ²⁸Si¹⁶O (archive record 5910818). */
export const SIOUVENIR: ExoMolSource = archivedList({
  species: "SiO",
  citation:
    "ExoMol line list SiOUVenIR for 28Si-16O, as its archive holds it (S. N. Yurchenko, J. " +
    "Tennyson, A.-M. Syme et al., ExoMol line lists - XLIV. Infrared and ultraviolet line list for " +
    "silicon monoxide (28Si16O), Mon. Not. R. Astron. Soc. 510 (2022) 903-919, DOI " +
    "10.1093/mnras/stab3267)",
  record: 5_910_818,
  def: fetched(
    "28Si-16O__SiOUVenIR.def",
    6_639,
    "68b4dd01d6777ec75d36980e1ef0e9eaab8a4de4c4d89c2be971aa66bbca15d1",
  ),
  states: fetched(
    "28Si-16O__SiOUVenIR.states.bz2",
    3_239_299,
    "50dc8c419338f55ec2daa0ff41b0435417e96f7d55866407467333bc46349156",
  ),
  layout: { energy: 2, degeneracy: 3, j: 4 },
  pf: fetched(
    "28Si-16O__SiOUVenIR.pf",
    250_000,
    "bdd468fabb897535d91c05609d3264bd90455b63c17a32bcb11c7323b5238cc9",
  ),
  transitions: oneFile(
    "28Si-16O__SiOUVenIR.trans.bz2",
    1_127_696_581,
    "fff9d3d02b1870b975f11da627ed0c5acc3208f99ae9623f4e4a9616fdf444df",
  ),
  abundance: ISOTOPE_FRACTIONS.Si28 * ISOTOPE_FRACTIONS.O16,
});

/**
 * Li2015's ¹²C¹⁶O (archive record 5716770), its bands from Δv = 8 dropped (decision-r08-t4b-t12c.md
 * follow-up 3: its 9–0 band is stronger than its 8–0 and 7–0, breaking the ladder's decline; those
 * bands are the `overtones` part's, pending).
 */
export const LI2015: ExoMolSource = archivedList({
  species: "CO",
  citation:
    "ExoMol line list Li2015 for 12C-16O, as its archive holds it (G. Li, I. E. Gordon, L. S. " +
    "Rothman, Y. Tan, S.-M. Hu, S. Kassi, A. Campargue and E. S. Medvedev, Rovibrational line " +
    "lists for nine isotopologues of the CO molecule in the X 1Sigma+ ground electronic state, " +
    "Astrophys. J. Suppl. Ser. 216 (2015) 15, DOI 10.1088/0067-0049/216/1/15), its transitions of " +
    "v' - v'' 8 and up dropped",
  record: 5_716_770,
  def: fetched(
    "12C-16O__Li2015.def",
    5_074,
    "1cf80b1a54f43ecff6ecfad99f090bc9d14dcd66cf522e5bf49bfa68c0c61739",
  ),
  states: fetched(
    "12C-16O__Li2015.states.bz2",
    54_318,
    "b7823c655b9554c2c0693520622e290d80e5fd151ee4ebb3083575decd8657c1",
  ),
  // ID, energy, degeneracy, J and v (Li2015's states: i E g J v kp).
  layout: { energy: 2, degeneracy: 3, j: 4, v: 5 },
  pf: fetched(
    "12C-16O__Li2015.pf",
    234_000,
    "0a6c56318875a4cf6bfc7f8aa9b5609865025c922bf0ccf42f6b017c1ed29b43",
  ),
  transitions: oneFile(
    "12C-16O__Li2015.trans.bz2",
    1_403_037,
    "20a9aa869fdede362300a26f818bad21130f1f2b73f477d38b2d39d8bb361410",
  ),
  // HITRAN's natural abundance of ¹²C¹⁶O (hitran.org/docs/iso-meta/, molecule 5, local 1).
  abundance: 0.986_544,
  maxDeltaV: 7,
  intensityScale: {
    factor: 2.05,
    why:
      "R08.T4.d's HITRAN2024 table's 7-0 band (Medvedev and Ushakov 2022, x1.074 after Balashov et " +
      "al. 2023, measurement-anchored) over Li2015's at the 300 K hand-over: the table's 13 cells' " +
      "means at 10 Pa summed, 1.2044e-33 m2, over Li2015's direct sums in the same cells, 5.8622e-34 " +
      "m2, 2.054, to three figures (R08.T4.c's bound took the peak cell's 2.13, rounded up); every " +
      "line binned, the Delta v = 7 bands alone in the range, takes it, a stated estimate",
  },
});

/** Harris's H¹²C¹⁴N (archive record 5716828). */
export const HARRIS: ExoMolSource = archivedList({
  species: "HCN",
  citation:
    "ExoMol line list Harris for 1H-12C-14N, as its archive holds it (G. J. Harris et al., Mon. " +
    "Not. R. Astron. Soc. 367 (2006) 400-406, DOI 10.1111/j.1365-2966.2005.09960.x; R. J. " +
    "Barber et al., ExoMol line lists III: An improved hot rotation-vibration line list for HCN " +
    "and HNC, Mon. Not. R. Astron. Soc. 437 (2014) 1828-1835, DOI 10.1093/mnras/stt2011)",
  record: 5_716_828,
  def: fetched(
    "1H-12C-14N__Harris.def",
    10_695,
    "8c3d48b1e698915b13779662b95503cde478fa6f46d7abd8dbeca07d7cd0541d",
  ),
  states: fetched(
    "1H-12C-14N__Harris.states.bz2",
    1_364_477,
    "295058a3d9eba6cc167b8fe185cdf74bf7caa90ae6114b745cd159354134d166",
  ),
  layout: { energy: 2, degeneracy: 3, j: 4 },
  pf: fetched(
    "1H-12C-14N__Harris.pf",
    104_000,
    "028f4b42b3ef9bf9527b172b9b89a7dd47259d8b1bf556de0fd201684968b4f4",
  ),
  transitions: oneFile(
    "1H-12C-14N__Harris.trans.bz2",
    392_693_188,
    "df87d95d9bf81af24a4a6b3626a8227ff4bbb31602760a7e7b0d80da3bf10ca1",
  ),
  // HITRAN's natural abundance of H¹²C¹⁴N (hitran.org/docs/iso-meta/, molecule 23, local 1).
  abundance: 0.985_114,
});

/**
 * The width WCCRMT's lines take, its `.def` giving none: 0.07 cm⁻¹ bar⁻¹ at 296 K with n = 0.5, the
 * default every other R08.T4.f list's `.def` gives but Harris's (0.084), a stated estimate.
 */
const WCCRMT_WIDTH: AssumedWidth = {
  halfWidthPerCmBar: 0.07,
  exponent: 0.5,
  why: "WCCRMT's .def gives no default width; the 0.07 cm-1/bar, n 0.5 every other R08.T4.f list's .def gives but Harris's (0.084), estimated",
};

/** WCCRMT's ¹⁴N₂ triplet system (archive record 5717009). */
export const WCCRMT: ExoMolSource = archivedList({
  species: "N2",
  citation:
    "ExoMol line list WCCRMT for 14N2, the triplet system, as its archive holds it (C. M. Western, " +
    "L. Carter-Blatchford, P. Crozet, A. J. Ross, J. Morville and D. W. Tokaryk, The spectrum of N2 " +
    "from 4,500 to 15,700 cm-1 revisited with PGOPHER, J. Quant. Spectrosc. Radiat. Transf. 219 " +
    "(2018) 127-141, DOI 10.1016/j.jqsrt.2018.07.017; its partition function P. S. Barklem and R. " +
    "Collet, Astron. Astrophys. 588 (2016) A96)",
  record: 5_717_009,
  def: fetched(
    "14N2__WCCRMT.def",
    7_236,
    "2148ea263843c15eb47669bbb20e5179b401606d2389fabba2f8832e2c5deae7",
  ),
  states: fetched(
    "14N2__WCCRMT.states.bz2",
    504_460,
    "de6a6aa09cf4ae58743532373453685386d068eedec49ffe1d9d0ff650245b03",
  ),
  layout: { energy: 2, degeneracy: 3, j: 4 },
  pf: fetched(
    "14N2__WCCRMT.pf",
    225_000,
    "4de565a667b4f294eec15c4366931b277c9ad1bb780e9705492c0db50514b91f",
  ),
  transitions: oneFile(
    "14N2__WCCRMT.trans.bz2",
    85_375_712,
    "6c1984b7a43c3ebd85e19c0a8dbca12c697d1bba5c05d41f2800147d86b02797",
  ),
  // HITRAN's natural abundance of ¹⁴N₂ (hitran.org/docs/iso-meta/, molecule 22, local 1).
  abundance: 0.992_687,
  assumedWidth: WCCRMT_WIDTH,
});

// --- R08.T4.f's part 2: hot H₂O, fetched from ExoMol's own site ---

/** The POKAZATEL list's directory. */
const POKAZATEL_URL = "https://www.exomol.com/db/H2O/1H2-16O/POKAZATEL/";

/**
 * POKAZATEL's transition files over {@link HOT_BIN_RANGE_PER_CM}, 100 cm⁻¹ each, as fetched
 * 2026-10-29: each one's size its HTTP Content-Length and its SHA-256 the fetch's (ExoMol
 * publishes no checksums), each passing `bzip2 -t`.
 */
const POKAZATEL_TRANSITIONS = [
  [13_100, 143_135_111, "9b104cfc178cebf1c6b32157568473abba4ac1b858589471bf325d67096b7222"],
  [13_200, 141_329_944, "15cd4f37355eab39c5b077edb276654f4a40a8948e85aa595d51b0047c0f4019"],
  [13_300, 139_492_529, "7c8349ea5820aa9da0c79ea04b8b2dc141ba8dfea05db924736f31edd640482e"],
  [13_400, 137_810_225, "e7d048e7a3809a0edfc3e96659c28f97d3efb9aeb9765d22e571f71128ee8940"],
  [13_500, 136_044_408, "67470d9056b31b7ec9f83271a14f2c17198de0b648bc771e6321e83371aa4d46"],
  [13_600, 134_266_120, "1f3b9eb9f045c40992ee702c18797363494b9f4057ae267ad6b1b65ab009481e"],
  [13_700, 132_596_640, "0f92ef8b05391aa00913c15c53f30f33461cd47dab5131e4e8ed3bb1113cba23"],
  [13_800, 130_835_345, "9aefec555af3e298daa571ba83eedd27fbf1e7dbe3b75e00108b673ba5eff708"],
  [13_900, 129_066_290, "16274a6f6b03bab0f637759ee6485aaa4dde665d3e361768e129900d0d32aeeb"],
  [14_000, 127_429_874, "4254d3bcb0c643ca1a98f4042ba9dba15c9dc7657887cb82efae61fea6c1b09f"],
  [14_100, 125_779_855, "5199d1e4c3e49b050f68dcd37a9d79a9e0967114230548210fb6844492290618"],
  [14_200, 124_111_249, "6177919f3b888807950c53bc3514da697249c6494511e5d1f609935d6f1fb9f9"],
  [14_300, 122_485_697, "1987bfec54b6e48255c526d6b9998afe765f19ca90ddea3d9018af46061dcaed"],
  [14_400, 120_871_118, "45cf439093acea7eb8fedf9e72ba61bb51b17456fdc7bc78987c41a660f581af"],
  [14_500, 119_317_695, "0679e1618310670e55cb44a1200184666bbbdc826e1ceba8d06aeae6021b8161"],
  [14_600, 117_707_935, "e3ce68b8af934f7ae9bba993183b631ee04c0112217ae2beee75c3575b256b00"],
  [14_700, 116_130_253, "8d668530992a05b6982f6f3bc886d916237d254c7f810a8a4ea9f3b257ebbcfc"],
  [14_800, 114_546_109, "bf7a48a12a4dc95c1e777c183590349323c8100cc5925a81f1a26531d6b7d7fa"],
  [14_900, 113_037_292, "49a0dda32a657b38fe19c71aa40ba61b11ca5701d6e0e39c9acc5d74a25074bb"],
  [15_000, 111_583_590, "6d0169a24ebaf6940c4f58392a21703928887b3e5b0dc58c2e402e0e1d96b5e8"],
  [15_100, 110_054_894, "efca4c3ed0c013013fc083a4c21035d6144b5a27c6fa94bef4baae0feaea2bf5"],
  [15_200, 108_516_002, "bb02644463d8e14937380d0112b034a34f681f73b0589989768e2a1729c2e682"],
  [15_300, 107_037_686, "3715711aa97c083981d6c849e88f7ac34cdc31bd80fd43bbc2cfd3a3bbf934cc"],
  [15_400, 105_536_960, "d74d2b84f8cd360fe4c356012eef94d2ca9e9812d30d46b06f7b811cfb50c0da"],
  [15_500, 104_065_167, "b8a5afb4e66e043e3e8499016579fd7286d60abd439a3622638c1d28a84abaf1"],
  [15_600, 102_607_266, "0f9c5ea8b09e6ad3bcdc634ad5d5f6c75174484221c4483b92dce4a15c8bb55a"],
  [15_700, 101_246_142, "c3c2acf9bb70fd0763b6b558643994aad4ea4f14a94a1bc3f8dfa6ceed5443f8"],
  [15_800, 99_847_128, "d569ebb377211c6aa1002ba0685cb1826a5566f1f33d0ef0d6a81e5bc9f0178a"],
  [15_900, 98_463_244, "5c4c9333e46909a995969f9d565b1683a3793a825b3de46418f4bcbb29a9df66"],
  [16_000, 97_033_524, "e64aec1c577cef87270bc6b87aa163a280b48aaea0aa69f0fba24a2fdd31cc59"],
  [16_100, 95_661_486, "d6aedd481c14712452fd0f2fb17d2775199098eee88d6b4bb29390cb185b33cc"],
  [16_200, 94_366_072, "b70abf8533adef94ed5c4ce48cc95e1afa11c85a9b651f85e27bfba742e7a027"],
  [16_300, 93_090_491, "3a4fc6056c394e8b9a4fad18b847ca2c60b14a18d8f7aa09463f7586cb4dba7e"],
  [16_400, 91_687_298, "4bf18ec52a3ce95d3735dec362d3b733332e8129f8cd92eac51ab6a549cfc91c"],
  [16_500, 90_418_578, "e074e7c5fec1035fb0b49307dc422d2b92aec5695e80a7cda02b482efb838add"],
  [16_600, 89_145_245, "1b3be279f2a5ccedd9a9415f7c35887148b3ffa3b0e2c62e8963eccba91d5bbe"],
  [16_700, 87_820_083, "d2a76b06ada3b9e43b8be8f9656e0777aa858ce1500cd38c9e657b27d6f91ddf"],
  [16_800, 86_600_576, "24cc844a8efdb79bad03c12e3a494988af46b9762700911c73dc14e291a9b212"],
  [16_900, 85_251_800, "ec58c27709df3587f45ae79bb8ca21643d05dec7c651d97ba34d97e4f6f7c0d6"],
  [17_000, 84_010_262, "d885dfaab227a03780777890819d9b273aebf217d6286987d9b4c8cd4dfff156"],
  [17_100, 82_817_876, "a5f9089e5ed9443791df68dd0cf10bc1de5fef6e9356e66ad66a64be7105a0cd"],
  [17_200, 81_656_190, "ea16b49378e615c8b87eefd73be49e706dfc6a6210ec888b83c72b56decc1232"],
  [17_300, 80_461_924, "911d136906af64a7f9661a0a4ce8ed0e5c72a1f11f0679123e8363d3ac2ce071"],
  [17_400, 79_288_454, "be456c34486b9bb2ebd7dda02e38caefc7d844287fbb075e96a3c5aaca41a1d1"],
  [17_500, 78_122_151, "7f6bf298edd3220f0e007607b32797b233b63a2a58f3c9dd108d95328a745408"],
  [17_600, 76_979_391, "fbbfbeebcb3864340b246c61b87fb5aa00aa97db3cd0eef89f56f9aa554921bd"],
  [17_700, 75_912_980, "011b3fb96c530ab6590e5a281036806736364e6bdd9d81ef6b1d6918f35bf3ea"],
  [17_800, 74_739_599, "57d77b57d905521db4e2e5e0a6db81791a095aebf7fe7cfd406988bb5c12e16f"],
  [17_900, 73_575_653, "4a3b9f385c8e66506386ed8e5f22cf911de8b97451355a315277b5c35ed31d1f"],
  [18_000, 72_536_361, "9eba4ce4a04a098bba18423cb31be656a1f989e33d563d5cae6d92e1e179af07"],
  [18_100, 71_430_077, "d2fb4b3d69bdbcd7f8a5bebec3927f54f8af92f42824dbf85e25e1819b867ef0"],
  [18_200, 70_344_074, "88307ee37ba6747226cf9ced2133d190db6bbf83390eb5dfa1cb74ba82f6b4e7"],
  [18_300, 69_175_220, "0f5de1395a7bf0da40cc569539ec5ac58bdceb5037e4106ae699ab87cdc8c8fb"],
  [18_400, 68_146_537, "da73ad800a5b7c6ce160a2a9f15cbfdb2599fe27a7bc82be563f1fa09a507b21"],
  [18_500, 67_111_803, "38ea234cde46702ad06f2eceb995036046b7bd68b8ab83ae824c9f675921f5cd"],
  [18_600, 66_060_239, "783f5aaf6891655ae66ad4ea2f45f9d339783d4e287ab0cde9d2d850bcdf0157"],
  [18_700, 65_125_975, "bd1acc300140d41a5ad8c8cda703693cb5444a7a9048178d73ad85553be7c30c"],
  [18_800, 64_006_138, "75c80bd0b852e29c73d608c25d06df4b7b127aa3d7a54b02066c7fbb5ccc0448"],
  [18_900, 63_043_045, "84a4ec9b357f80615d4166c85019071ba313eac2b1629fa736b88cbe48f62781"],
  [19_000, 62_081_473, "a8dd9ee2cb9e9af84e3313ebcc09c76f6c26cab84a73e2b56803c721443296e0"],
  [19_100, 61_104_177, "5545ed5c70a4ad14b19191579f858095a924e9246843ceebf8ebf464c0f4ff4d"],
  [19_200, 60_159_558, "95a089ebf8298639ac4957782ed9e8fac77ad908dc656c4b54aecbb686449274"],
  [19_300, 59_183_347, "0484ac6cbe080b1ff570db38396f0d45f0ad11d3870d92f42dd87cf3d6c5a451"],
  [19_400, 58_228_567, "04b1fdac970ffcbd33abfbbb794f4d847353ff86202413def94cead98ce73706"],
  [19_500, 57_297_797, "494e7eea648f596beaddd30d6fa951220cd5b376e13590373fc407e0b3d41747"],
  [19_600, 56_366_714, "05682772c81513acfdbc7dd4013196c0326c987ccd79ae5b257edfafdd8ac065"],
  [19_700, 55_474_838, "6a04ca2103cb9e01cc40f1b007549c740a34fca35f955ac5baeae81756385eb8"],
  [19_800, 54_521_105, "2dd4e042f867b7389fa5b5719c94fa1d4f8e5c20cea5f4ceb9980adfdd7d3a91"],
  [19_900, 53_678_184, "77f5519e0647ce081f9fe41555b626984c08e9173cd07c124e3bf1b162981fad"],
  [20_000, 52_775_332, "2478d45d0f38828edb3e63df648df5fc6c6cf06901d8493152e3a8bb1bddb0b3"],
  [20_100, 51_921_045, "83fb3423742540ebd2ac70394675b4bcb4e3a74c7437aec691d62df0298d83b1"],
  [20_200, 51_057_662, "950f2eebee6ea8cd3fb1c633f93aadeb88e4ce417c4dea493e9ad457785b2d43"],
  [20_300, 50_220_180, "bcf1f65ccb98b2c89c47e15ec52f4a1a4c484043ae4eed176f6b5fda42ba3ae0"],
  [20_400, 49_397_513, "874ded84a4755bc0c0f0084a95fc0fd05978bb8f4b31d93ab4a729230bd8cb5b"],
  [20_500, 48_567_309, "aa9d4f4a7c0657cc86eb1e81095f2df00e5cad1f5f3bfac343b42f31e92d8223"],
  [20_600, 47_799_462, "835ecb2131b2b0303a03ca6393e3bcd480b6690143e300b504d142f6db759345"],
  [20_700, 46_955_844, "8f85cce35de1aed54f36fba0b4dc7857b97602a9a6e386e3ba23dcf67c448d86"],
  [20_800, 46_175_844, "9b01da31f5eb92886ee8716f92fc9931835e94fca803efde633b450d396df616"],
  [20_900, 45_397_644, "26474799e4fbc4ee4998cac8f837d9b58ab098e974ef6b7d224bf29d82145554"],
  [21_000, 44_611_741, "e0a373854ec4e5fbcc4d101dc40e5c01031439675611375daf92b1e21d1ec6e7"],
  [21_100, 43_882_621, "69a6b73b68887cb705b6a31b802821f151f8ba6d4f4cd46c955abb64eddfec0f"],
  [21_200, 43_109_429, "940ab5cdc5150c82b189eb76c042f0ffcc87b4f71b0cf703ceb771fe0a9346cb"],
  [21_300, 42_329_330, "b62bc9890d26153bd6c1821c0842d7c4e62a6f97a10f91898d351342d2b66154"],
  [21_400, 41_601_472, "ae384e786f1be7842eae177d02bcbfbeb9e53d68df7d99a303c84bf847481b7f"],
  [21_500, 40_904_681, "22bc5cbc09236ec6ee99f943e61ea245e5f6848c71bd108fc7ef6202c1c4be45"],
  [21_600, 40_159_660, "dbe71ba3f2e10a4dc322eb00f744d0c6bcf4a0eda8a17862c97761e72b2bfdd7"],
  [21_700, 39_502_271, "53c9b66dae4a44c163ecb5c1f303e2b75faadb23e346bfa97b34bad75b703673"],
  [21_800, 38_757_538, "4574693fa302de0cb9c5f288154973da48adfae3410bd14ea3e9d843272a66c7"],
  [21_900, 38_067_324, "dea6a30f561cbb1b497b14dc308df3d9434a4b8b89ccd1273b286a39c07994c3"],
  [22_000, 37_398_357, "895a2ad22606eb762788672adc96b9c221eab3246930f81b95140f6b7bb5e5e5"],
  [22_100, 36_708_598, "638732b4b776d87dfd16821d1bfc20563d11160503622872243e83cfdd91d075"],
  [22_200, 36_062_233, "a7bc12d51995cbc9057152f9b0105bdb4244b7750fea923ba7fdaae16dbec3df"],
  [22_300, 35_392_047, "c017c896ff4a338094c260ec8cbd3e217db9ed34a77e096bfcb1ee9cc34f3845"],
  [22_400, 34_765_838, "d20d51f17f114e08e46aa1208c562065ff42aa06da3a387f76ef9a831e6b40e7"],
  [22_500, 34_113_421, "cd41674f05262c1fbf19451f09bb72875e912d691b35f2d69cbde232d8888c60"],
  [22_600, 33_479_775, "e321655e0e4ae42ebdc29829dd363f7b68308ccdf97de7c7ae3d10f130158cb0"],
  [22_700, 32_865_609, "887cba541e2992b865146d09bb11a3c341fa6c083d078cc5d6dc0c230d910698"],
  [22_800, 32_258_716, "9389a645d3a9d21cd72d5306d17dd3b591bb783b7dab3b4ed969998c735252b5"],
  [22_900, 31_621_364, "e1b3c334942b009e601776b073faf0296b6722756ba66b4ba69569b17de5388e"],
  [23_000, 31_018_474, "db8b358f8359199cf8c876210b40b7841a4647cddc00ca4f1a84bff9d9bf8e92"],
  [23_100, 30_471_898, "be9394ddcdf1fe462a5958d78657f8b1bb4c8f46b72af0e3da04cd62793a1c24"],
  [23_200, 29_907_120, "c79c3af41e3b39c62bfbed7fc7da3155a8bebe4b15c9de57ab4324599d30aa33"],
  [23_300, 29_269_441, "441b8d8de2baaeb938e1e0de678cd37ae489b85a181c21c24929909cdcac52df"],
  [23_400, 28_746_671, "b010585ab72bfa26833b6d64761c6f9def6537fba7e7b57141b35d49fbe368dc"],
  [23_500, 28_190_755, "f7804954e6ad8ec78f1efb74dc101e77a8747651d21dd4badfe6dcf9d1d7bdfa"],
  [23_600, 27_617_102, "b3c1ec7dc22b67544567b1dc879a8c64444f10bc14b93d6532e93a280b08a47a"],
  [23_700, 27_074_814, "edd7a03ceafa4cbba497195ae1101d6ae7cdf2c277fab0b123a2cadaf5e70510"],
  [23_800, 26_568_536, "978d1e59cad3eb586c5d725e6ac079c8fde4140a27ae7a2ff7baa72500d832af"],
  [23_900, 26_048_357, "4c8d60997d4330a931bcc6a1ca26c75008308dad341bd125f6b5e2d882bb1068"],
  [24_000, 25_504_819, "5c737d96ee5c9f5cd22e41e1d5a3729f7e6f63b837ae64dffe5a6e4d37168020"],
  [24_100, 25_030_793, "7433baab06658e5e0e849075438d58505e5eafb91540a689a847f89cb8da837f"],
  [24_200, 24_550_027, "bb5bd0c15a11deb9d8e359b8162a22562dda1d51acdd40aa627768a02f6ac349"],
  [24_300, 24_043_086, "afcef8edeca772af443fa3cefcfaad8a27792c6aa1e6c1456ccbc3b9f562be10"],
  [24_400, 23_579_703, "07feb8df4b5ed053047d3a625e8ae47cfd8b91cf750c3bd1a5f67b65c92c05bb"],
  [24_500, 23_093_791, "3d41cdf2ae92a02ee7e671e5f4b13feff63bbe7a54fcb2f7ebb240c0840defa2"],
  [24_600, 22_654_720, "034b1b1d356ee37d9d82a2262ee917eeb5a174aad63eaccb03d4f41877765b1f"],
  [24_700, 22_178_552, "40d83b236f905219aa1c364d76b5aa15440f7aff10961082362263336a1ca070"],
  [24_800, 21_718_327, "f143e76036a3bf5c2c85fdfa316341e3484852225cdd7ef14db91d36b7a8ab32"],
  [24_900, 21_286_651, "775ebc0832231d8be72f5e5b0f020fcf0a2a845ad03824c5c32dbc6ee79ad232"],
  [25_000, 20_828_892, "f22f5bf04adc8aa9009cd0d4cf2216bdcc3c846ea96bc753376f76ce56b465da"],
  [25_100, 20_391_306, "93b15926d6415dd6e2126bc5cbf72e2f13d6e82728b21aa67eeae1b95a1f0a5c"],
  [25_200, 19_961_827, "5a5ec4fa2eaa40aba1bf1e622694551fdf46a734fe7a5c9250d7bd7e98c1e2ef"],
  [25_300, 19_496_346, "aa294a833d9b0a287cf434de4fa1173cae2d43543db1cd0415b0450f64612813"],
  [25_400, 19_107_932, "1801098b29efaeb70d438ad07df5f865c819b259444c30a11573efa4d3ec4716"],
  [25_500, 18_697_772, "07dff0564775cb5af16ae19469f42f47b1b4f1f410039c4af8a7f344ce8727bd"],
  [25_600, 18_270_040, "6b617c38c7fc544702b9badb50de0cc6728c848247a96ecf6e8becc7c82778bc"],
  [25_700, 17_901_972, "ca70703a99d1d70373471d871dc261a42be0a2ac0fd443b43d7979af4b4ea073"],
  [25_800, 17_513_213, "48a284590f41f844d7869ec8f265b5ed0d6f195b9247a9848e7e429fb7f58ac2"],
  [25_900, 17_110_789, "1e429dce7a8bd14a8286986328023a9b2ee2f1997bbe84c1d6d7966d576c665d"],
  [26_000, 16_742_139, "c062ccc7f46c34c7c632e95bbce3ed1165b27d25a1efb349a4cf7505db1cf8de"],
  [26_100, 16_391_239, "03d6bb0338a249bbb6dd93dc67c728a162dcaa849f99c4d80e9540478b90b9a9"],
  [26_200, 16_001_239, "e7a128df908657e7d20702bd6443b98ccf6f0c6de64539ccafc142f66eeb4b6b"],
  [26_300, 15_647_617, "78c45451388f93cf51ba655d88831ce122a3670435672061f13548443b6edd6b"],
] as const;

/**
 * POKAZATEL (¹H₂¹⁶O), ExoMol's recommended H₂O list, valid to 6,000 K by its definition; its
 * states the MARVELised file served on 2026-10-29 (12,667,256 bytes, last modified 2026-04-09:
 * 14,664 of its 810,269 states MARVEL's), its transitions three columns, so that ν is E′ − E″.
 */
export const POKAZATEL: ExoMolFilesSource = {
  kind: "exomol",
  species: "H2O",
  citation:
    "ExoMol line list POKAZATEL for 1H2-16O, version 20230621 (O. L. Polyansky, A. A. Kyuberis, " +
    "N. F. Zobov, J. Tennyson, S. N. Yurchenko and L. Lodi, ExoMol molecular line lists XXX: a " +
    "complete high-accuracy line list for water, Mon. Not. R. Astron. Soc. 480 (2018) 2597-2608, " +
    "DOI 10.1093/mnras/sty1877), its 133 transition files of 13100-26400 cm-1 and its MARVELised " +
    `states; ${EXOMOL_2024}`,
  url: POKAZATEL_URL,
  fetched: "2026-10-29, from ExoMol's site",
  terms: EXOMOL_TERMS,
  def: {
    name: "1H2-16O__POKAZATEL.def",
    bytes: 13_776,
    sha256: "50d2248b1fba7ac095b7306864c7a2556d24f989c58494c87ce16f83e7c50c87",
    format: "text",
  },
  states: {
    name: "1H2-16O__POKAZATEL.states.bz2",
    bytes: 12_667_256,
    sha256: "72e9067baa46884ec7811be329d5e7710adc17f37ad60f8235df3b5ccfdac092",
    layout: { energy: 2, degeneracy: 3, j: 4 },
  },
  transitions: POKAZATEL_TRANSITIONS.map(([from, bytes, sha256]) => ({
    name: `1H2-16O__POKAZATEL__${from}-${from + 100}.trans.bz2`,
    bytes,
    sha256,
    fromPerCm: from,
    toPerCm: from + 100,
  })),
  partition: {
    from: "pf",
    file: {
      name: "1H2-16O__POKAZATEL.pf",
      bytes: 260_000,
      sha256: "39441c486f09e50fcc450e85e321dff968a2db4e8a606af50ae346cb293dae39",
    },
  },
  broadeners: [],
  // HITRAN's natural abundance of H₂¹⁶O (hitran.org/docs/iso-meta/, molecule 1, local 1).
  abundance: 0.997_317,
  binRangePerCm: HOT_BIN_RANGE_PER_CM,
};

/** HyVO's directory on ExoMol's site, whose `.def` and `.pf` VO's table takes. */
const HYVO_URL = "https://www.exomol.com/db/VO/51V-16O/HyVO/";

/** Cubillos's REPACK record on Zenodo (version 3, 2025-09-20), which holds HyVO's strong lines. */
const REPACK_RECORD = 14_266_247;

/**
 * The terms of VO's lines: HyVO's, the database's licence, which binds REPACK's repacking of it
 * (whose record states CC BY 4.0), each read at its fetch.
 */
const REPACK_TERMS =
  "CC BY-SA 4.0, the ExoMol database's licence ('All data in ExoMol is released under the Creative " +
  "Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0) licence', " +
  "https://www.exomol.com/data/licence/, as read 2026-10-29), HyVO being ExoMol's; REPACK's record " +
  "on Zenodo, its strong lines repacked from HyVO, states CC BY 4.0 (read 2026-10-30); the values " +
  "here are reduced from them, adapted material offered under CC BY-SA 4.0 (README.md beside this " +
  "directory, and NOTICE); the line list's files are not committed (decision-r08-licences.md)";

/**
 * HyVO's ⁵¹V¹⁶O (ExoMol's recommended VO list, version 20231218, valid to 5,400 K by its
 * definition) through its strong lines, as Cubillos's REPACK record holds them (version 3: a line is
 * dropped where, at both 100 and 3,500 K, its strength is at most 0.01 times a stronger kept
 * line's at its centre, by that line's Doppler profile, within six Doppler widths; `repack`'s
 * `pack.py` and `cutils.c`), with HyVO's `.def` and `.pf` from ExoMol's site. Main's ruling of
 * 2026-10-30: HyVO's 27 transition files over the range are 92.7 GB, and VOMYT's lines carry half
 * HyVO's strength.
 */
export const HYVO_REPACK: RepackSource = {
  kind: "repack",
  species: "VO",
  citation:
    "ExoMol line list HyVO for 51V-16O, version 20231218 (C. A. Bowesman, Q. Qu, L. K. McKemmish, " +
    "S. N. Yurchenko and J. Tennyson, ExoMol line lists - LV: Hyperfine-resolved molecular line " +
    "list for vanadium monoxide (51V16O), Mon. Not. R. Astron. Soc. 529 (2024) 1321-1332, DOI " +
    "10.1093/mnras/stae542), its strong lines as P. E. Cubillos's REPACK holds them (Repacked " +
    "ExoMol Opacity Line Lists, Zenodo record 14266247, version 3, 2025-09-20, DOI " +
    "10.5281/zenodo.14266247, threshold 0.01 at 100 and 3500 K; the method P. E. Cubillos, An " +
    "Algorithm to Compress Line-transition Data for Radiative-transfer Calculations, Astrophys. J. " +
    `850 (2017) 32, DOI 10.3847/1538-4357/aa9228); ${EXOMOL_2024}`,
  url: `${archiveUrl(REPACK_RECORD)}; HyVO's .def and .pf ${HYVO_URL}`,
  fetched:
    "2026-10-30, REPACK's file from Zenodo (its MD5 the record's); HyVO's .def and .pf 2026-10-29 " +
    "from ExoMol's site",
  terms: REPACK_TERMS,
  def: {
    ...fetched(
      "51V-16O__HyVO.def",
      7_777,
      "3f45e96cd915c74906a43d9018c77fbe40f110e140da181224b713c0bc9a290d",
    ),
    format: "text",
  },
  lines: {
    ...fetched(
      "VO_exomol_hyvo_0.22-50um_100-3500K_threshold_0.01_lbl.dat",
      1_593_290_104,
      "949a06a2f430d387c52a595e87319b79f38ca071e9f3bca2fcdf614eb54f4dac",
    ),
    md5: "cf548637f7977b9754a73ab3ad048d0f",
    // REPACK's isotope name for 51V16O (its isotopes.dat), on every record.
    isotope: 16,
  },
  partition: {
    from: "pf",
    file: fetched(
      "51V-16O__HyVO.pf",
      134_999,
      "33b052747d800f4a6d8dcba4ae651513167bf895f397fcfb8782cacec4d1d63f",
    ),
  },
  abundance: ISOTOPE_FRACTIONS.V51 * ISOTOPE_FRACTIONS.O16,
  binRangePerCm: HOT_BIN_RANGE_PER_CM,
};

/** What every R08.T4.f reduction's statement shares (R08.T4.g's, on decision-r08-t4ef-storage's economies). */
const HOT_SUPER_LINE_REDUCTION =
  "Each 1 nm vacuum cell's absorption at each node from super-lines: every line's S(T) (ExoMol's " +
  "intensity formula, Q(T) from the list's .pf, times the isotopologue's natural abundance) binned " +
  "linearly on a wavenumber grid of a twelfth of the Doppler half-width at 13100 cm-1 (the " +
  "heaviest isotopologue's where a table takes several), finer than a tenth of the narrowest Voigt " +
  "half-width; at each node the bins over the cell and 25 cm-1 each side, coarsened by a whole " +
  "factor while the step stays under a tenth of the node's narrowest Voigt half-width and a 600th " +
  "of the cut-off (0.042 cm-1), their binning's response divided out, and convolved by FFT with " +
  "Voigt profiles (Humlicek 1982) cut off 25 cm-1 from each bin at every pressure, at the Doppler " +
  "widths of the cell's two edges blended linearly in wavenumber; sampled at the cell's points as " +
  "R08.T4.d's lines are. Every line takes the list's default air width (cm-1 per bar at 296 K, " +
  "ExoMol's unit), estimated, so that every node's line shape is estimated; sorted into its " +
  "k-distribution and reduced to two-point Gauss-Legendre g-intervals shared by every node of the " +
  "cell, by bisection to 8e-4 in T over every column; temperature nodes on E1' (decision-r08-t4e-" +
  "budget section 1.4: 50 K steps to 1000 K, then each step the largest multiple of 50 K within " +
  "10% of its lower node) from the species' floor (decision-r08-t4ef-storage section 1.4, E2), " +
  "refined by bisection to a quarter of E1's step where the chain test (cubic in T on ln sigma, the " +
  "client's rows at the standard pressures each read between the table's nodes, the read cubic in " +
  "ln p on tau) fails 2e-3 within its columns (the gated column, and the species' own Rayleigh " +
  "column, R08.T4.g's ruling); alternate pressure nodes below 1 MPa dropped where it passes (E3), " +
  "and an 8-a-decade midpoint added between two held rows below 1 MPa where it fails by the read " +
  "itself (decision-r08-t4e-unsold's follow-up); ln sigma stored in 16 bits a value " +
  "(decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md).";

/** The 10% ladder's nodes from a floor to 4,000 K, R08.T4.f's top (decision-r08-t4e-budget §1.4). */
const HOT_TOP_K = 4_000;

/**
 * An R08.T4.f table: one list's super-lines (with its other isotopologues'), on E1′'s ladder from
 * its floor, its measured range its nodes', CUTOFF_25's shape unless given, the cells of 380–760 nm
 * its lines reach.
 */
function hotTable(spec: {
  readonly exomol: ExoMolSource;
  readonly isotopologues?: ReadonlyArray<ExoMolFilesSource>;
  readonly temperaturesK: ReadonlyArray<number>;
  /**
   * Where its measured range ends below its top node, K (VO's 3,000 K, its last comparison), or
   * none.
   */
  readonly measuredToK?: number;
  readonly rayleighCm2: RayleighCrossSection;
  /** The line shape, where not CUTOFF_25: H₂O's continuum's convention ({@link CKD_CONVENTION}). */
  readonly policy?: ShapePolicy;
  readonly what: string;
  readonly notes: string;
  /**
   * `estimated` where the list's intensities take a stated scale (CO's), its hot bands are known
   * incomplete in the range (HCN's, as R08.T4.c judged them) or its model is extrapolated past its
   * fitted range (N₂'s); `measured` otherwise.
   */
  readonly basis: "measured" | "estimated";
}): HotTable {
  const first = spec.temperaturesK[0] ?? 0;
  const last = spec.temperaturesK.at(-1) ?? 0;
  return {
    kind: "hot",
    exomol: spec.exomol,
    isotopologues: spec.isotopologues ?? [],
    basis: spec.basis,
    measuredRangeK: [first, spec.measuredToK ?? last],
    temperaturesK: spec.temperaturesK,
    policy: spec.policy ?? CUTOFF_25,
    saturation: undefined,
    rayleighCm2: spec.rayleighCm2,
    reduction: `${spec.what} ${HOT_SUPER_LINE_REDUCTION}`,
    cellsNm: [380, 760],
    notes: spec.notes,
  };
}

/** R08.T4.f's tables, by registry key (decision-r08-t4ef-storage §5.4's order). */
export const HOT_TABLES: Readonly<Record<string, HotTable>> = {
  FeH: hotTable({
    exomol: MOLLIST_FEH,
    temperaturesK: ladderTemperatures(1_000, HOT_TOP_K, "E1prime"),
    rayleighCm2: heliumRayleighCm2,
    basis: "measured",
    what: "FeH's F4Delta-X4Delta bands below 760 nm (MoLLIST, R08.T4.f), from 1000 K (E2).",
    notes:
      "Below 1000 K the 1000 K node is held, estimated: Fe condenses at about 1100-2000 K at " +
      "photospheric pressures (decision-r08-t4ef-storage section 1.4, E2); above 4000 K the 4000 K " +
      "node is held, estimated. Q(T) is the list's .pf, valid over 1000-8000 K by its README (its " +
      "296 K row, read for the cache's 296 K diagnostics alone, lies under the states' own sum): its " +
      "states file holds only the states its transitions reach, whose sum falls 3.1 times under the " +
      ".pf at 3000 K. Fetched from the database's archive, whose states file differs from the one " +
      "R08.T4.c summed (SHA-256 e978d8ec...900a), its transitions and .pf the same.",
  }),
  TiO: hotTable({
    exomol: TOTO_48,
    isotopologues: TOTO_MINOR,
    temperaturesK: ladderTemperatures(1_000, HOT_TOP_K, "E1prime"),
    rayleighCm2: heliumRayleighCm2,
    basis: "measured",
    what: "TiO's bands (Toto, its five Ti isotopologues, R08.T4.f), from 1000 K (E2).",
    notes:
      "Below 1000 K the 1000 K node is held, estimated: Ti condenses as the calcium titanates and " +
      "perovskite (Ca3Ti2O7, Ca4Ti3O10, CaTiO3) at about 1700-2050 K over 1 mbar to 10 bar (H. R. " +
      "Wakeford et al., Mon. Not. R. Astron. Soc. 464 (2017) 4247, DOI 10.1093/mnras/stw2639, " +
      "Eqs. 5-7, after Lodders 2002; decision-r08-t4ef-storage section 1.4, E2); above " +
      "4000 K the 4000 K node is held, estimated, Toto being valid to 5000 K. The five Ti " +
      "isotopologues share one set of bins, each line at its own Q (its .pf) and abundance (CIAAW's " +
      "representative Ti and O fractions), and take 48TiO's Doppler width in the convolution (the " +
      "others' differ by 0.8-1.6%). Fetched from the database's archive, which holds 48TiO with the " +
      "states' MARVEL update of 2021-08-25 and the others as of 2019-05-14; its 48TiO transitions " +
      "differ from the 2024-05-09 version " +
      "R08.T4.c summed (SHA-256 50bca5ad...bec7), its .pf the same.",
  }),
  H2S: hotTable({
    exomol: AYT2,
    temperaturesK: [296, ...ladderTemperatures(350, 1_500, "E1prime")],
    rayleighCm2: hydrogenSulphideRayleighCm2,
    basis: "measured",
    what: "H2S's visible overtones (AYT2, R08.T4.f), from 296 K.",
    notes:
      "From 296 K, where its overtones already pass the pending bound, to AYT2's limit, 1500 K by " +
      "its definition: below 296 K the 296 K node is held and above 1500 K the 1500 K node, each " +
      "estimated (its paper gives 2000 K). AYT2 is complete only to 11000 cm-1 above room " +
      "temperature and to about 20000 cm-1 (500 nm) at 296 K, its visible intensities less " +
      "accurate (Azzam et al. 2016, abstract and section 2): its hot visible values are lower " +
      "limits, and those below 500 nm lie past its completeness at every temperature. The table " +
      "is kept measured, as R08.T4.c judged the list. " +
      "The nodes 296 K, then T4.d's 50 K grid from 350 K and E1' from 1000 K. Fetched " +
      "from the database's archive, whose states file differs from the one R08.T4.c summed " +
      "(SHA-256 52cf071a...da13), its .pf the same.",
  }),
  SiO: hotTable({
    exomol: SIOUVENIR,
    temperaturesK: ladderTemperatures(1_000, HOT_TOP_K, "E1prime"),
    rayleighCm2: heliumRayleighCm2,
    basis: "measured",
    what: "SiO's A-X hot bands and its other systems in the range (SiOUVenIR, R08.T4.f), from 1000 K (E2).",
    notes:
      "Below 1000 K the 1000 K node is held, estimated: SiO's visible absorption begins near " +
      "1000 K, in its spin-forbidden d3Delta-X band (ab initio) at 1000-1500 K, its A-X hot bands " +
      "dominant from about 2500 K (R08.T4.c's bound; decision-r08-t4ef-storage section 1.4, E2), " +
      "where its X 12-0 band's 296 K cells are 1e-39 m2; above 4000 K the 4000 K node is held, " +
      "estimated, SiOUVenIR " +
      "being valid to 10000 K. Fetched from the database's archive, whose states file differs " +
      "from the one R08.T4.c summed (SHA-256 acf9fc10...0756), its transitions and .pf the same.",
  }),
  CO: hotTable({
    exomol: LI2015,
    temperaturesK: ladderTemperatures(300, HOT_TOP_K, "E1prime"),
    rayleighCm2: carbonMonoxideRayleighCm2,
    what: "Hot CO (Li2015, its bands from v' - v'' = 8 dropped, R08.T4.f), from its hand-over.",
    notes:
      "Hands over from R08.T4.d's HITRAN2024 table at 300 K, its first node at or above HITRAN's " +
      "296 K. Li2015's Delta v = 7 bands lie 2.05 times under HITRAN2024's measurement-anchored " +
      "7-0 band there, summed over that table's 13 cells at 10 Pa (1.40-2.22 times cell by cell over " +
      "691-700 nm, falling along the P branch; 1.79-2.22 over 691-696 nm, 96% of the band), so " +
      "every line takes that summed ratio, 2.05 (the source's intensityScale; R08.T4.c's bound " +
      "took the peak cell's 2.13, rounded up), and the table is estimated: " +
      "at and below 300 K the client reads R08.T4.d's table, above it this one (kDistribution.ts's " +
      "tableAt). There, over the columns at which R08.T4.d's table transmits 0.2-0.8, a cell's " +
      "transmittance steps by up to 0.03 at 1 kPa and 0.13 at 0.1 MPa (hotLines.test.ts): its " +
      "lines saturated, each step compounds the cell's own ratio's departure from the summed scale " +
      "with the list's default width (0.07 cm-1 per bar, n = 0.5, its .def) over HITRAN's air " +
      "widths (about 0.06 cm-1 per atm at 296 K, intensity-weighted, from an earlier HITRAN " +
      "edition's 1-0 band, the science check's figure), the width the larger part at 1 kPa. With " +
      "n = 0.5 against HITRAN's 0.67-0.76 the assumed width grows apart with temperature, about " +
      "2.2 times a HITRAN-like width at 4000 K. The cells from 701 nm, past HITRAN2024's lines, " +
      "hold Li2015's alone, at most 1.6e-36 m2 a cell at 300 K. The bands from Delta v = 8 are " +
      "left out, bounded by CO's pending overtones part. " +
      "Above 4000 K the 4000 K node is held, estimated, Li2015 being valid to 9000 K. Fetched from " +
      "the database's archive, whose states file differs from the one R08.T4.c summed (SHA-256 " +
      "83ca3036...04e1), its transitions and .pf the same.",
    basis: "estimated",
  }),
  HCN: hotTable({
    exomol: HARRIS,
    temperaturesK: ladderTemperatures(300, HOT_TOP_K, "E1prime"),
    rayleighCm2: heliumRayleighCm2,
    basis: "estimated",
    what: "Hot HCN (Harris, R08.T4.f), from its hand-over.",
    notes:
      "Hands over from R08.T4.d's HITRAN2024 table at 300 K, its first node at or above HITRAN's " +
      "296 K. There Harris's cell means at 10 Pa are 1.003-1.005 times that table's in its 41 cells " +
      "above a hundredth of its peak (HITRAN2024's sum over its 84 cells 0.996 of Harris's direct " +
      "sums). Its 43 weaker cells, at the bands' edges, run from 0.84-0.90 times that table's (five " +
      "cells of line wings alone, Harris's lines narrower) through 12 within 1% to 3.4 times where " +
      "both hold lines; in six (658, 659, 696, 741, 748 and 750 nm), where R08.T4.d's table holds " +
      "only wings of at most 4e-38 m2, Harris's lines give 4.5e-35 to 6.8e-34 m2, and its 15 cells " +
      "past that table's lines (660-662, 697-707 and 749 nm) hold up to 2.0e-34 m2. Over the " +
      "columns at which R08.T4.d's table transmits 0.2-0.8 (from about 1.3e28 m-2 at 0.1 MPa), a " +
      "cell's transmittance steps by up to 0.09 in the stronger cells at 1 kPa and 0.1 MPa, the " +
      "hot table transmitting more, as narrower lines would (Harris's default width, 0.084 cm-1 " +
      "per bar with n = 0.5, its .def, against HITRAN's air widths of about 0.11 cm-1 per atm at " +
      "300 K, intensity-weighted, the science check's figure from HITRAN2020's), and by up to 0.48 " +
      "at 1 kPa and 0.77 at 0.1 MPa in the weaker cells (hotLines.test.ts). With n = 0.5 against " +
      "HITRAN's 0.75-0.84 the assumed width is about a HITRAN-like one's near 600 K and 2.5 times " +
      "it at 4000 K. At and below 300 K the client reads R08.T4.d's table, above it this one " +
      "(kDistribution.ts's tableAt). Above 4000 K, Harris's limit, the 4000 K node " +
      "is held, estimated. The table is estimated, as R08.T4.c judged Harris's values: the list " +
      "holds few hot bands in the range, so its hot values are lower limits. The list ends at " +
      "17585.79 cm-1 (568.6 nm), so its cells run 568-760 nm, the 568 nm cell holding wings alone. " +
      "Fetched from the database's archive, whose states file differs from the one R08.T4.c summed " +
      "(SHA-256 631f775e...516a), its transitions and .pf the same.",
  }),
  N2: hotTable({
    exomol: WCCRMT,
    temperaturesK: ladderTemperatures(1_800, HOT_TOP_K, "E1prime"),
    rayleighCm2: nitrogenRayleighCm2,
    basis: "estimated",
    what:
      "N2's triplet system in the range, the first positive (B3Pig-A3Sigmau+) bands from the " +
      "metastable A state among them (WCCRMT, R08.T4.f), from 1800 K.",
    notes:
      "Stated none below 1800 K (transparency.ts's N2 entry, whose limit the table's first node " +
      "meets: the belowK contract's table branch); above 4000 K the 4000 K node is held, " +
      "estimated. In LTE: a non-LTE A-state population (an aurora's) is not modelled. The table " +
      "is estimated, as the plan's band model was: WCCRMT's PGOPHER model is fitted to spectra over " +
      "4500-15700 cm-1 and carried past it, its widths assumed. Its .def gives no default width, so " +
      "every line takes 0.07 cm-1 per bar with n = 0.5, the default all the other R08.T4.f lists' " +
      ".def give but Harris's (0.084), estimated; its Q(T) is its .pf, interpolated from Barklem " +
      "and Collet 2016's, its states file holding the triplet states A, B, W and B' alone. At the " +
      "nodes they share (1800, 2500, 3000 and 4000 K) its 760 nm cell lies 20-34 times under the " +
      "band model's bound, which put each band's strength in its origin's cell from short " +
      "lifetimes, and its other 460-760 nm cells above a hundredth of the bound's peak 14 times " +
      "under it at the median (0.5-44). Its states do not reach the C state, so the second " +
      "positive (C3Piu-B3Pig) bands are not in it: the band model of both systems stays pending " +
      "as N2's secondPositive part (bounds.json), its cells over 380-459 nm reaching 10^3-10^7 " +
      "times the table's.",
  }),
  H2O: hotTable({
    exomol: POKAZATEL,
    temperaturesK: ladderTemperatures(300, HOT_TOP_K, "E1prime"),
    rayleighCm2: waterRayleighCm2,
    policy: CKD_CONVENTION,
    basis: "measured",
    what:
      "Hot H2O (POKAZATEL, R08.T4.f), from its hand-over; every profile less its value at the " +
      "25 cm-1 cut-off, the continuum's convention (Clough, Kneizys and Davies 1989), as " +
      "R08.T4.d's H2O table's lines (decision-r08-t4d-kdist.md section 6.4), nodes whose " +
      "half-width passes 2.5 cm-1 estimated.",
    notes:
      "Hands over from R08.T4.d's HITRAN2024 table at 300 K, its first node at or above HITRAN's " +
      "296 K: at and below 300 K the client reads R08.T4.d's table, above it this one " +
      "(kDistribution.ts's tableAt). There, at 10 Pa, the 82 cells above a hundredth of that " +
      "table's peak agree in mean to a median 1.012 (0.68-1.17; over 680-760 nm 1.027 summed), " +
      "and over the columns at which R08.T4.d's table transmits 0.2-0.8 a cell's transmittance " +
      "steps by up to 0.096 at 1 kPa and 0.104 at 0.1 MPa in them (hotLines.test.ts). Below 500 " +
      "nm, where both tables' cells are at most 3.3e-3 of the peak, POKAZATEL's lines sum to " +
      "0.48 (380-420 nm), 0.80 (420-460 nm) and 0.68 (460-500 nm) times HITRAN2024's, 0.05-4.1 " +
      "cell by cell, and a transmittance steps by up to 0.55 at 1 kPa and 0.60 at 0.1 MPa: the " +
      "lists disagree on water's weak blue lines, and which is nearer the measurements is not " +
      "settled here. Both tables subtract each line's value at the 25 cm-1 cut-off, the " +
      "continuum's convention, so that CKD's continuum, drawn at every temperature, is not " +
      "counted twice (decision-r08-t4d-kdist.md section 6.4). POKAZATEL holds 1H2-16O alone, so " +
      "HITRAN2024's H2-18O, H2-17O and HDO (0.27% of its abundances) are left out above 300 K. " +
      "Every line takes the list's default width, 0.07 cm-1 per bar with n = 0.5 (its .def), " +
      "estimated, and the table holds no self width (R08.T4.d's gives self 4.87 times air): H2O " +
      "being dipolar (kDistribution.ts's DIPOLAR_BROADENERS), any share of it leaves this " +
      "table's lines estimated, a steam atmosphere's taking air's width. Against the pending " +
      "bound it replaces (ExoMol's cross-section service over the same list, per molecule of " +
      "1H2-16O, rounded up), the direct sums' cell means are 0.995-0.996 times it at the median " +
      "at 1000, 3000 and 4000 K (the abundance, 0.9973, and the round-up), 0.69-1.43 cell by " +
      "cell at 1000 K and 0.93-1.05 at 4000 K. Q(T) is the list's .pf (174.5813 at 296 K). Above " +
      "4000 K the 4000 K node is held, estimated, POKAZATEL being valid to 6000 K by its .def. " +
      "Fetched from ExoMol's site, the database's archive holding POKAZATEL's opacities but not " +
      "its transitions; its states the MARVELised file served on 2026-10-29.",
  }),
  VO: hotTable({
    exomol: HYVO_REPACK,
    temperaturesK: ladderTemperatures(1_000, HOT_TOP_K, "E1prime"),
    measuredToK: 3_000,
    rayleighCm2: heliumRayleighCm2,
    basis: "measured",
    what: "VO's bands (HyVO's strong lines, as Cubillos's REPACK holds them, R08.T4.f), from 1000 K (E2).",
    notes:
      "Below 1000 K the 1000 K node is held, estimated (decision-r08-t4ef-storage section 1.4, " +
      "E2: V dissolves in the Ti-bearing condensates, which cold-trap it below about 1500-2000 K " +
      "at photospheric pressures, K. Lodders, Astrophys. J. 577 (2002) 974, DOI 10.1086/342241, " +
      "its abstract). REPACK keeps HyVO's strong lines alone: a line is dropped where, at both " +
      "100 and 3500 K, its strength is at most 0.01 times a stronger kept line's at its centre, " +
      "by that line's Doppler profile, within six Doppler widths (repack's pack.py and cutils.c; " +
      "Cubillos 2017). 7594976 of its 56903218 records lie in 13100-26400 cm-1, and the weak " +
      "lines' quasi-continuum is left out (main's ruling of 2026-10-30 waives " +
      "decision-r08-t4ef-storage section 5.4's no-cut-off rule for VO). Against the pending " +
      "bound's HyVO cell means (ExoMol's cross-section service, per molecule of 51V16O) the " +
      "direct sums over the cells above a hundredth of its peak are 0.991, 0.985 and 0.974 " +
      "summed at 1000, 2000 and 3000 K; cell by cell 0.947-1.008 between the 10th and 90th " +
      "percentiles and 0.904-1.053 at the extremes (623 and 607 nm; 653 nm), and to 0.83 in a " +
      "cell at a thousandth of the peak (602 nm at 1000 K), the deficit growing with temperature " +
      "as the hot weak lines' share does. Cell means test the thin limit alone: the weak lines' " +
      "share of the window opacity at large columns is not measured. The table is measured to " +
      "3000 K, the last temperature compared, and estimated above it: the strong lines' census " +
      "by lower-state energy, fitted to the three comparisons, puts the summed deficit at about " +
      "3.0% at 3300 K, 3.3-3.4% at 3500 K (past the 3% gate) and 4.0-4.3% at 4000 K; above 4000 " +
      "K the 4000 K node is held, estimated, HyVO being valid to 5400 K. Q(T) is HyVO's .pf, and " +
      "the mass and default width (0.07 cm-1 per bar with n = 0.5, estimated) its .def's. VOMYT " +
      "(McKemmish et al. 2016) was not taken: its lines carry about half HyVO's strength over " +
      "the range (0.50 summed; 0.38-0.68 over 450-760 nm and 1.2-1.4 times below 450 nm, main's " +
      "research advice).",
  }),
};

/** The ExoMol tables of R08.T4.g, then R08.T4.f's ({@link HOT_TABLES}). */
export const EXOMOL_TABLES: Readonly<Record<string, ExoMolTable>> = {
  ...HOT_TABLES,
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
  /**
   * Each list binned, where a table takes several isotopologues: its states file, abundance, Q at
   * 296 K and the lines it binned, at its last pass.
   */
  readonly isotopologues?: ReadonlyArray<{
    readonly states: string;
    readonly abundance: number;
    readonly partitionSum296: number;
    readonly linesBinned: number;
  }>;
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

/** A table's lists: its own, then a hot table's other isotopologues'. */
export function tableLists(table: ExoMolTable): ReadonlyArray<ExoMolSource> {
  let lists: ReadonlyArray<ExoMolSource>;
  switch (table.kind) {
    case "superLines":
    case "merged":
      lists = [table.exomol];
      break;
    case "hot":
      lists = [table.exomol, ...table.isotopologues];
      break;
  }
  return lists;
}

/** The files that hold a list's lines: its states and transition files, or its REPACK file. */
function linesFilesOf(source: ExoMolSource): ReadonlyArray<ExoMolFile> {
  let files: ReadonlyArray<ExoMolFile>;
  switch (source.kind) {
    case "exomol":
      files = [source.states, ...source.transitions];
      break;
    case "repack":
      files = [source.lines];
      break;
  }
  return files;
}

/**
 * Checks a list's fetched files against its record: definition, states, transitions (or its
 * REPACK file) and `.pf`.
 */
function checkInputs(data: string, source: ExoMolSource): void {
  checkFile(join(data, source.def.name), source.def);
  for (const file of linesFilesOf(source)) {
    checkFile(join(data, file.name), file);
  }
  const pf = partitionFileOf(source);
  if (pf !== undefined) {
    checkFile(join(data, pf.name), pf);
  }
}

/** A list's `.pf` table, from its checked file, or `undefined` where it serves none. */
function pfOf(data: string, source: ExoMolSource): ReadonlyMap<number, number> | undefined {
  const file = partitionFileOf(source);
  return file === undefined
    ? undefined
    : parsePartitionFunction(readCheckedText(join(data, file.name), file.sha256));
}

/** What a pass over a table's lists gives: its bins, its record and each list's own figures. */
interface TablePass {
  readonly bins: PassBins;
  readonly record: PassRecord;
  readonly isotopologues: NonNullable<CacheRecord["isotopologues"]> | undefined;
}

/**
 * One streaming pass over a table's isotopologues into one set of bins (R08.T4.f's TiO), each list
 * at its own Q and abundance, in turn, on the heaviest's grid, whose Doppler step is the finest;
 * stimulated emission applied once every list is in. The record sums the lists' lines, intensities
 * and direct sums; its Q(T) is the first list's, the others' in {@link TablePass.isotopologues}.
 */
async function sharedPass(
  table: HotTable,
  data: string,
  temperaturesK: ReadonlyArray<number>,
  jobs: number,
): Promise<TablePass> {
  const sources = [table.exomol, ...table.isotopologues].map((source) => {
    if (source.kind !== "exomol") {
      throw new Error(`${source.species}'s isotopologues share bins from ExoMol's own files only`);
    }
    return source;
  });
  const definitions = sources.map((source) => readDefinition(data, source));
  const heaviestDa = Math.max(...definitions.map((d) => d.molecularMassDa));
  const [low, high] = table.exomol.binRangePerCm;
  if (sources.some((s) => s.binRangePerCm.join() !== table.exomol.binRangePerCm.join())) {
    throw new Error(`${table.exomol.species}'s isotopologues bin over different ranges`);
  }
  const main: SuperLineBins[] = temperaturesK.map((t) =>
    emptyBins(t, low, high, binStepPerCm(t, low, heaviestDa)),
  );
  const records: PassRecord[] = [];
  for (const [i, source] of sources.entries()) {
    // One list at a time, so that a single list's states are held at once.
    // oxlint-disable-next-line eslint/no-await-in-loop
    const states = await readStates(join(data, source.states.name), source);
    const definition = definitions[i] ?? readDefinition(data, source);
    // The lists fill the shared bins one after another, so each pass waits for the last.
    // oxlint-disable-next-line eslint/no-await-in-loop
    const { record } = await streamPass(source, definition, states, data, {
      temperaturesK,
      target: { kind: "shared", into: main },
      broadening: [],
      jobs,
    });
    records.push(record);
  }
  for (const bins of main) {
    applyStimulatedEmission(bins);
  }
  const first = records[0];
  if (first === undefined) {
    throw new Error(`${table.exomol.species}'s table names no list`);
  }
  const sum = (of: (r: PassRecord) => number): number =>
    records.reduce((total, r) => total + of(r), 0);
  const cellMeansM2 = first.cellMeansM2.map((row, t) =>
    Float64Array.from(row, (_, c) => sum((r) => r.cellMeansM2[t]?.[c] ?? 0)),
  );
  return {
    bins: { main, apart: undefined },
    record: {
      ...first,
      linesRead: sum((r) => r.linesRead),
      linesBinned: sum((r) => r.linesBinned),
      intensity296Cm: sum((r) => r.intensity296Cm),
      cellMeansM2,
      seconds: sum((r) => r.seconds),
    },
    isotopologues: sources.map((source, i) => ({
      states: source.states.name,
      abundance: source.abundance,
      partitionSum296: records[i]?.partitionSum296 ?? Number.NaN,
      linesBinned: records[i]?.linesBinned ?? 0,
    })),
  };
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
  for (const list of tableLists(table)) {
    checkInputs(data, list);
  }
  const definition = readDefinition(data, source);
  const broadening = (source.kind === "exomol" ? source.broadeners : []).map((file) => {
    const dir = values.broad ?? data;
    return parseBroadening(readCheckedText(join(dir, file.name), file.sha256), file, definition);
  });
  let apart: ApartBands | undefined;
  if (table.kind === "merged") {
    // TYM's bands matched to HITRAN2024's, by the survey run before binning.
    const survey = readBandSurvey(cache, source.species);
    apart = { bands: new Set(survey.apart), rangePerCm: table.hitran.apartRangePerCm };
  }
  const listPf = pfOf(data, source)?.get(296);
  const jobs = Number(values.jobs ?? "3");
  let pass: TablePass;
  if (table.kind === "hot" && table.isotopologues.length > 0) {
    pass = await sharedPass(table, data, temperaturesK, jobs);
  } else {
    const states =
      source.kind === "exomol"
        ? await readStates(join(data, source.states.name), source)
        : undefined;
    const own = await streamPass(source, definition, states, data, {
      temperaturesK,
      target: { kind: "own", apart },
      broadening,
      jobs,
    });
    pass = { ...own, isotopologues: undefined };
  }
  const { bins, record } = pass;
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
    ...(pass.isotopologues === undefined ? {} : { isotopologues: pass.isotopologues }),
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
  if (table.kind !== "merged") {
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

/**
 * The reduction's grid: its temperature nodes and pressures, R08.T4.g's the 30 standard pressures,
 * R08.T4.f's E3's thinned pressures unless given, with their rows ({@link chainRowsPa}).
 *
 * @throws Error for pressures given to a table on the 30 standard pressures.
 */
export function gridOfTable(
  table: ExoMolTable,
  temperaturesK: ReadonlyArray<number> | undefined,
  pressuresPa?: ReadonlyArray<number>,
): ReductionGrid {
  const nodes = temperaturesK ?? table.temperaturesK;
  if (!rulesOf(table).ownPressures) {
    if (pressuresPa !== undefined) {
      throw new Error(`${table.exomol.species}'s table takes the 30 standard pressures`);
    }
    return { temperaturesK: nodes, pressuresPa: STORED_PRESSURES_PA };
  }
  const pressures = pressuresPa ?? THINNED_PRESSURES_PA;
  return { temperaturesK: nodes, pressuresPa: pressures, rowsPa: chainRowsPa(pressures) };
}

/** HITRAN2024's N₂O lines, for N₂O's merged table. */
function hitranSpecies(
  table: ExoMolTable,
  lines: string | undefined,
  tips: string | undefined,
): LineSpecies | undefined {
  if (table.kind !== "merged") {
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
      pressures: { type: "string" },
      out: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  if (table === undefined || values.cache === undefined || values.out === undefined) {
    throw new Error(
      "usage: exomol-k-distributions --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>] [--cells a-b] [--stride n:k] [--temperatures t,...] [--pressures p,...] --out <json>",
    );
  }
  const grid = gridOfTable(
    table,
    values.temperatures?.split(",").map(Number),
    values.pressures?.split(",").map(Number),
  );
  const record = readCacheRecord(values.cache, table.exomol.species);
  const missing = binTemperatures(grid.temperaturesK).filter(
    (t) => !existsSync(join(values.cache ?? "", binsFileName(table.exomol.species, "main", t))),
  );
  if (missing.length > 0) {
    throw new Error(`the cache holds no bins at ${missing.join(", ")} K: run exomol-bins first`);
  }
  const lines = hitranSpecies(table, values["hitran-lines"], values.tips);
  const { spectra, parts, close } = tableSpectra(table, values.cache, lines, record.definition);
  const reductionSource = reductionSourceOf(table, record.definition, grid);
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
  // The run's costs, for the record: its CPU time and its peak resident memory.
  const usage = process.resourceUsage();
  process.stdout.write(
    `${table.exomol.species}: ${cells.length} cells, CPU ${((usage.userCPUTime + usage.systemCPUTime) / 1e6).toFixed(0)} s, peak RSS ${(usage.maxRSS / 1024).toFixed(0)} MB\n`,
  );
}

/** The passes a projection's sample takes: E3's alternates and midpoints kept as the merge asks. */
const PROJECTION_PASSES = 4;

/** A sample cell reduced over its table's whole ladder for the projection (`exomol-projection`). */
export interface ExoMolSample {
  readonly role: "strongest" | "median" | "edge";
  readonly cellNm: number;
  readonly g: number;
  /** The nodes reduced, K, and the pressures with the alternates and midpoints kept, Pa. */
  readonly temperaturesK: ReadonlyArray<number>;
  readonly pressuresPa: ReadonlyArray<number>;
  /** The temperature intervals the merge would bisect, each adding a node to the table. */
  readonly refine: number;
  /** The intervals failing at their finest step, which would be `estimated`. */
  readonly estimated: number;
  /** Failures at a node's temperature that no alternate or midpoint reaches. */
  readonly atNodes: number;
  readonly worstChain: number;
  readonly worstNode: number;
  /** E4's raised first pressure node, Pa: the highest whose hold passes its checks. */
  readonly firstPa: number;
  /** The 10 Pa hold's 1 Pa check. */
  readonly belowGrid1Pa: number;
  readonly passes: number;
  /** The CPU time of its last pass, s. */
  readonly seconds: number;
}

/**
 * The cells a table's lines reach, nm: those within its range whose direct-sum mean, or a
 * neighbour's, is above 0 at some cached temperature (a line's 25 cm⁻¹ wing reaches the next cell).
 */
export function cellsWithLines(table: ExoMolTable, record: CacheRecord): ReadonlyArray<number> {
  const [from, to] = table.cellsNm;
  const lit = new Set<number>();
  for (const { cellMeansM2 } of Object.values(record.temperatures)) {
    for (const [c, mean] of cellMeansM2.entries()) {
      if (mean > 0) {
        for (const nm of [379 + c, 380 + c, 381 + c]) {
          lit.add(nm);
        }
      }
    }
  }
  return [...lit].filter((nm) => nm >= from && nm <= to).toSorted((a, b) => a - b);
}

/**
 * A table's three sample cells, nm, by the cache's direct sums at a temperature: its strongest
 * cell, a median one, and its strongest cell's weaker neighbour, the band's edge.
 */
export function sampleCellsOf(
  cells: ReadonlyArray<number>,
  meansM2: ReadonlyArray<number>,
): Readonly<Record<ExoMolSample["role"], number>> {
  const mean = (nm: number): number => meansM2[nm - 380] ?? 0;
  const ranked = cells.toSorted((a, b) => mean(b) - mean(a));
  const strongest = ranked[0] ?? 380;
  const median = ranked[Math.floor(ranked.length / 2)] ?? strongest;
  const neighbours = [strongest - 1, strongest + 1].filter((nm) => cells.includes(nm));
  const edge = neighbours.toSorted((a, b) => mean(a) - mean(b))[0] ?? median;
  return { strongest, median, edge };
}

/**
 * `exomol-projection`: R08.T4.f's samples over each table's whole ladder on E3's pressures, the
 * alternates and midpoints they ask for kept, and the table's projected bytes from their mean G
 * (decision-r08-t4ef-storage §1.4), as a JSON record. A temperature interval a sample would bisect
 * is counted, not reduced: its bins wait for the full reduction's refinement pass.
 */
export function runExomolProjection(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      cache: { type: "string" },
      out: { type: "string" },
    },
  });
  const table = exomolTableOf(values.species);
  const { cache, out } = values;
  if (table?.kind !== "hot" || cache === undefined || out === undefined) {
    throw new Error("usage: exomol-projection --species <R08.T4.f key> --cache <dir> --out <json>");
  }
  const species = table.exomol.species;
  const record = readCacheRecord(cache, species);
  const nodesK = table.temperaturesK;
  const middleK = nodesK[Math.floor(nodesK.length / 2)] ?? nodesK[0] ?? 1_000;
  const cells = cellsWithLines(table, record);
  const means = record.temperatures[String(middleK)]?.cellMeansM2 ?? [];
  const samples = sampleCellsOf(cells, means);
  const { spectra, close } = tableSpectra(table, cache, undefined, record.definition);
  const runs: ExoMolSample[] = [];
  try {
    for (const [role, cellNm] of Object.entries(samples)) {
      let grid = gridOfTable(table, nodesK);
      for (let pass = 1; pass <= PROJECTION_PASSES; pass += 1) {
        const started = process.cpuUsage();
        const source = reductionSourceOf(table, record.definition, grid);
        const reduced = reducePartialCell(cellNm, spectra, grid, source);
        if (reduced === undefined) {
          break;
        }
        const { reduced: cell, partial } = reduced;
        const sorting = sortFailures(
          grid.temperaturesK,
          partial.failures,
          ladderFinestStepK,
          grid.pressuresPa,
        );
        const added = [...sorting.keepPressuresPa, ...sorting.addMidpointsPa];
        if (added.length > 0 && pass < PROJECTION_PASSES) {
          grid = gridOfTable(table, nodesK, withKeptPressures(grid.pressuresPa, added));
          continue;
        }
        let firstPa = grid.pressuresPa[0] ?? 10;
        for (const [k, p] of grid.pressuresPa.entries()) {
          if (p > 1e6) {
            break;
          }
          const error = belowGridError(
            cellNm,
            spectra,
            grid,
            table.policy,
            cell,
            record.definition.molecularMassDa,
            raisedFirstNodeChecksPa(p),
            k,
          );
          // Each node's check takes every lower one's, so the first to fail ends the search.
          if (!(error <= CHAIN_BOUND)) {
            break;
          }
          firstPa = p;
        }
        const used = process.cpuUsage(started);
        runs.push({
          role: role === "strongest" || role === "median" ? role : "edge",
          cellNm,
          g: cell.weights.length,
          temperaturesK: grid.temperaturesK,
          pressuresPa: grid.pressuresPa,
          refine: sorting.refine.length,
          estimated: sorting.estimated.length,
          atNodes: sorting.atNodes.length,
          worstChain: partial.worstChain.error,
          worstNode: partial.worstNodeError + partial.quantisation,
          firstPa,
          belowGrid1Pa: partial.belowGrid,
          passes: pass,
          seconds: (used.user + used.system) / 1e6,
        });
        break;
      }
    }
  } finally {
    close();
  }
  const meanG = runs.reduce((sum, r) => sum + r.g, 0) / Math.max(runs.length, 1);
  const pressures = Math.max(...runs.map((r) => r.pressuresPa.length), THINNED_PRESSURES_PA.length);
  const refined = nodesK.length + Math.max(0, ...runs.map((r) => r.refine));
  const projected = {
    species,
    cells: cells.length,
    cellsNm: [cells[0], cells.at(-1)],
    nodesK,
    meanG,
    pressures,
    temperaturesRefined: refined,
    bytes: tableBytes(cells.length, meanG, refined, pressures),
    bytesUnrefined: tableBytes(cells.length, meanG, nodesK.length, THINNED_PRESSURES_PA.length),
    bytesAsSpecified: tableBytes(
      cells.length,
      meanG,
      (nodesK.at(-1) ?? 0) / 50 - (nodesK[0] ?? 0) / 50 + 1,
      STORED_PRESSURES_PA.length,
    ),
    cpuSecondsPerCellNode:
      runs.reduce((sum, r) => sum + r.seconds / r.temperaturesK.length, 0) /
      Math.max(runs.length, 1),
    samples: runs,
  };
  writeFileSync(out, `${JSON.stringify(projected, null, 1)}\n`);
  process.stdout.write(
    `${species}: ${cells.length} cells, G ${runs.map((r) => r.g).join("/")} (mean ${meanG.toFixed(1)}), T ${refined} P ${pressures}: ${(projected.bytes / 1e6).toFixed(2)} MB (as specified ${(projected.bytesAsSpecified / 1e6).toFixed(2)} MB); ${runs.map((r) => `${r.role} ${r.cellNm} nm chain ${r.worstChain.toExponential(2)} refine ${r.refine} estimated ${r.estimated} at nodesK ${r.atNodes} first ${r.firstPa.toPrecision(3)} Pa 1 Pa ${r.belowGrid1Pa.toExponential(2)} ${r.seconds.toFixed(0)} s`).join("; ")}\n`,
  );
}

/**
 * What reducing a table's cell needs besides its spectra ({@link CellReductionSource}): its shape,
 * mass, condensation and Rayleigh cross-section, and where E4 raises its first pressure node above
 * 10 Pa, the hold's checks at 1 Pa and at every standard node below it.
 */
export function reductionSourceOf(
  table: ExoMolTable,
  definition: ExoMolDefinition,
  grid: ReductionGrid,
): CellReductionSource {
  const firstPa = grid.pressuresPa[0] ?? STORED_PRESSURES_PA[0] ?? 10;
  return {
    species: table.exomol.species,
    policy: table.policy,
    molecularMassDa: definition.molecularMassDa,
    ...(table.saturation === undefined ? {} : { saturation: table.saturation }),
    rayleighCm2: table.rayleighCm2,
    ...(firstPa > (STORED_PRESSURES_PA[0] ?? 10) * (1 + 1e-9)
      ? { belowGridChecksPa: raisedFirstNodeChecksPa(firstPa) }
      : {}),
  };
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

/** A table's inputs as its sidecar lists them: every file of every list and its SHA-256. */
function tableInputs(table: ExoMolTable): string {
  const files: ExoMolFile[] = [];
  for (const s of tableLists(table)) {
    files.push(s.def, ...linesFilesOf(s));
    const pf = partitionFileOf(s);
    if (pf !== undefined) {
      files.push(pf);
    }
    if (s.kind === "exomol") {
      files.push(...s.broadeners);
    }
  }
  const own = files.map((f) => `${f.name} ${f.sha256}`).join(", ");
  const hitran = table.kind === "merged" ? table.hitran : undefined;
  return hitran === undefined
    ? own
    : `${own}; ${hitran.source.lines.name} ${hitran.source.lines.sha256}, ${hitran.par.name} ${hitran.par.sha256}, ${hitran.source.tips.map((t) => `${t.name} ${t.sha256}`).join(", ")}`;
}

/** A table as its sidecar names it ({@link TableSource}). */
export function tableSourceOf(table: ExoMolTable): TableSource {
  const s = table.exomol;
  const rules = rulesOf(table);
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
    reduction: table.kind === "hot" ? `${table.reduction} ${table.notes}` : table.reduction,
    description:
      `Line absorbers' k-distributions per 1 nm cell reduced from the ${s.citation.split(" (")[0] ?? "line list"} ` +
      `(${rules.rulings}), written by apps/hyperion/scripts/atmosphereData.mjs ` +
      "exomol-k-distributions-merge from the fetched files named here, which are not committed. " +
      "Adapted material of the ExoMol database, offered under CC BY-SA 4.0 (README.md beside this " +
      "directory, and NOTICE). " +
      PART_LAYOUT_TEXT,
    finestStepK: rules.finestStepK,
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
  const definition = readDefinition(data, source);
  const states = await readStates(join(data, source.states.name), source);
  const { record } = await streamPass(source, definition, states, data, {
    temperaturesK: [],
    target: { kind: "own", apart: { bands: new Set(), rangePerCm: range } },
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
      if (outcome.pressuresPa !== undefined) {
        process.stdout.write(`refine-pressures: ${outcome.pressuresPa.join(",")}\n`);
      }
      process.stdout.write(
        `exomol-bins then takes these nodes, and their midpoints, for the cache (${rulesOf(table).finestStepText})\n`,
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
      into: { type: "boolean" },
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
      "usage: exomol-cell-references --species <key> --cache <dir> [--hitran-lines <csv> --tips <dir>] --table <sidecar.json> --cells <nm,...> --out <json> [--into]",
    );
  }
  const sidecar: unknown = JSON.parse(readFileSync(values.table, "utf8"));
  if (
    typeof sidecar !== "object" ||
    sidecar === null ||
    !("temperaturesK" in sidecar) ||
    !Array.isArray(sidecar.temperaturesK) ||
    !("pressuresPa" in sidecar) ||
    !Array.isArray(sidecar.pressuresPa)
  ) {
    throw new Error(`${values.table} is not a k-distribution sidecar`);
  }
  const nodesK = sidecar.temperaturesK.map(Number);
  // A sidecar writes its pressures to six figures; the lattice's are the reduction's own.
  const ownPressures = rulesOf(table).ownPressures;
  const grid = gridOfTable(
    table,
    nodesK,
    ownPressures ? sidecar.pressuresPa.map(Number).map(latticePressurePa) : undefined,
  );
  // R08.T4.g's references sit about 250 K; a hot table's about its middle node.
  const nearK = ownPressures ? (nodesK[Math.floor(nodesK.length / 2)] ?? 250) : 250;
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
      const entry = cellReference(cellNm, spectra, grid, table.policy, cap, chainCap, nearK);
      process.stdout.write(`exomol-cell-references: ${source.species} ${cellNm} nm\n`);
      return entry;
    });
  } finally {
    close();
  }
  const entry = JSON.stringify({ species: source.species, cells: entries });
  if (values.into !== true) {
    writeFileSync(values.out, `${entry}\n`);
    return;
  }
  const held = existsSync(values.out) ? readFileSync(values.out, "utf8") : undefined;
  writeFileSync(values.out, withReferences(held, source.species, entry));
}

/**
 * A file of several species' references (`{"references": […]}`) with one species' entry put in, in
 * place of any it held: the other species' entries kept as they were, in their order, this one's
 * last.
 *
 * @param heldText - The file's text as it was, or `undefined` for a file not yet written.
 * @throws Error for held text that is not a file of several species' references.
 */
export function withReferences(
  heldText: string | undefined,
  species: string,
  entry: string,
): string {
  const others: string[] = [];
  if (heldText !== undefined) {
    const held: unknown = JSON.parse(heldText);
    const references =
      typeof held === "object" && held !== null && "references" in held
        ? held.references
        : undefined;
    if (!Array.isArray(references)) {
      throw new Error("the file is not one of several species' references");
    }
    for (const r of references) {
      if (typeof r === "object" && r !== null && "species" in r && r.species !== species) {
        others.push(JSON.stringify(r));
      }
    }
  }
  return `{"references":[${[...others, entry].join(",")}]}\n`;
}

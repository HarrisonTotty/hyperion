/**
 * `node scripts/atmosphereData.mjs k-distributions …` and `k-distributions-merge …` (plan R08,
 * R08.T4.d): reduces a species' fetched lines to its k-distribution files.
 *
 * @remarks
 * `k-distributions --species <key> --lines <csv> --tips <dir> [--cells <a>-<b>] [--stride <n>:<k>]
 * [--temperatures <t,…>] --out <partial>` reduces the cells a–b (380–760 unless given), or every
 * nth from a + k, on the species' grid
 * ({@link LINE_SOURCES}; its nodes 50 K apart unless given) and writes a partial file of their
 * 16-bit cells and test records, so that several processes may share a species' cells.
 * `k-distributions-merge --species <key> --out-dir <dir> [--lines <file>] <partial>…` gathers the
 * partials and checks the rulings' bounds on every cell, the broadening records taken from the
 * species' lines where `--lines` gives them ({@link withLineBroadening}). Where the chain test fails between temperature nodes
 * more than 12.5 K apart it writes nothing and prints the refined nodes to run again with
 * (`decision-r08-t4d-kdist-chain.md`, ruling 4); otherwise it writes the species' binary parts
 * `<dir>/<key>-<n>.bin`, each under {@link PART_BYTES}, and their sidecar `<dir>/<key>.json`
 * (§9), any interval still failing at 12.5 K recorded and `estimated`. Each input is checked against its SHA-256 and none is committed
 * (`decision-r08-licences.md`).
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import {
  belowGridError,
  broadeningExponentOffsets,
  chiHeldFlags,
  broadeningRatios,
  CHAIN_BOUND,
  chainColumnCm2,
  type ChainError,
  CHI_CO2_CO2,
  CHI_CO2_N2,
  chiReachPerCm,
  cellBytes,
  encodeCell,
  type EncodedCell,
  type KDistributionMeta,
  type KDistributionPart,
  type NodeSpectra,
  kDistributionMetaText,
  kDistributionPart,
  lineByLine,
  MIX_SHARES,
  NODE_BOUND,
  QUANTISATION_BOUND,
  quantisationError,
  type RayleighCrossSection,
  type ReducedCell,
  reduceGridCell,
  type ReductionGrid,
  type SaturationCurve,
  gatedColumnCm2,
  type ShapePolicy,
  STORED_PRESSURES_PA,
} from "./kDistributions";
import {
  HITRAN_PARAMETERS,
  type LadderBand,
  ladderLines,
  LINE_CUTOFF_PER_CM,
  type LineList,
  type LineSpecies,
  parseHitranLines,
  parsePartitionSums,
  readCheckedText,
} from "./molecularLines";

/** A fetched file's name and checksum. */
export interface CheckedFile {
  readonly name: string;
  readonly sha256: string;
}

/** A line species' source, grid and policy. */
export interface LineSource {
  readonly species: string;
  readonly citation: string;
  readonly url: string;
  readonly lines: CheckedFile;
  /** TIPS by global isotopologue number. */
  readonly tips: ReadonlyArray<CheckedFile & { readonly isotopologue: number }>;
  readonly fetched: string;
  readonly terms: string;
  readonly basis: "measured" | "estimated";
  readonly measuredRangeK: readonly [number, number];
  /** The first and last temperature nodes, K: nodes run 50 K apart between them unless refined. */
  readonly rangeK: readonly [number, number];
  readonly policy: ShapePolicy;
  /** The main isotopologue's molecular mass, Da, for the 1 Pa check's and the cap's columns. */
  readonly molecularMassDa: number;
  /** Where the species condenses within its temperatures: the chain test's column cap. */
  readonly saturation?: SaturationCurve;
  readonly reduction: string;
  /**
   * For an overtone-ladder species, its anchor band and target: its lines file is then the anchor's
   * HITRAN `.par` file, whose band is moved and scaled ({@link ladderLines}).
   */
  readonly ladder?: LadderBand;
}

/** HITRAN2024's citation (Gordon et al. 2026). */
const HITRAN2024 =
  "I. E. Gordon et al., The HITRAN2024 molecular spectroscopic database, J. Quant. Spectrosc. " +
  "Radiat. Transfer 353 (2026) 109807, DOI 10.1016/j.jqsrt.2026.109807";

/** HITRAN's terms, as T4.c read them (`atmosphereData.ts`'s `HITRAN_TERMS`). */
const HITRAN_TERMS =
  "HITRAN states that its database and compilations are free (hitran.org/faq) and asks for its " +
  "edition's article and the original sources to be cited (hitran.org/citepolicy); it states no " +
  "licence: reduced values with their citations, the lines not committed (decision-composition " +
  "section 5)";

/** The HITRAN API request of every line species, by its isotopologues. */
function hitranUrl(isotopologues: ReadonlyArray<number>): string {
  return (
    `https://hitran.org/lbl/api?iso_ids_list=${isotopologues.join(",")}&numin=13100&numax=26400` +
    `&fixwidth=0&sep=[comma]&request_params=${HITRAN_PARAMETERS.join(",")}`
  );
}

/** TIPS files by isotopologue, as fetched 2026-10-12 (each matching T4.c's of 2026-10-10/11). */
const TIPS: Readonly<Record<number, string>> = {
  1: "3d6812b28d411597564006e8c51dd3e3b0bf4f5a47c8c055d1ed25cbff9375a2",
  2: "08efeff76e4b1d8adab23f1c6a38331aeb3462d6cac806383dbd42979a7383d5",
  3: "99c9f4d901ca95a5510fd5f21e3b4e294d14aeabe51dcb6c4bd9f4991234c9ad",
  4: "1de5c3a00b51323f6d101dc70c92543be5e205a2a08991b051c7e8e4225d7a04",
  7: "ad1c625eea7377331b829fcf51e1aa68dfa4a84f0c7c8f7b324b516535c0d480",
  8: "eb3ea36542f22e24ce2acc84e7d16f5efb02104d12046f82e49651d529ffed4d",
  9: "5df8544e44060891173a2869da1ebdd2486201e21491c2a8b67a7f75aae3080b",
  21: "9afea5328b9639ebd9b7876953cb14c5bef3022de0d623a14470416faf78b1c8",
  26: "2cd7ded504d40fdd3d37de3bda0266d09be71552011995b09158a081f199c9a2",
  36: "cfee961a1ed969799d451c609993d8266f3362ee45210a1c2627cfc1c6018166",
  37: "06e902498613e3e01d3c42f4603d03a43b837b4333dd13e00fca036d5d25455c",
  38: "e4b62e3b8dec5095f226562721c302a18ee297fd180c3b81bfa65c7be15b4476",
  70: "57537cda4e86f0fdfe2ad3b03490c18303a314963f9c78c821a109eed4e4139b",
  103: "d475e2099cdde6c2ae93c491e14c4be01a16f0989169e17d7bf48e3a66a1a9a7",
  115: "e5b0e11b1cc53796e2b29a7a55cb9d8f8a65843c1a9ce2bdd1e2f43a3ee07980",
  76: "4a3c3ef6ce7bf921e51c0af703042d6df0e58d1cef7c27d3d8632ebb013a477a",
  78: "6e128ee585006db0ea4a4da8544dded9268dff2f0907923f904e1f2aaaf050f8",
  79: "55b99f4efda6901f89c6b470fb8f717829b36086ce9cffbeec44ed8027f887d4",
  90: "e160468f182642f500f0596749eaa56ac4b9d3c2277637351e2ba88d73d7b178",
  92: "af7eb473f9c8425e18e775b16e06991dc2de9c7e5a329845182fc538ee9cf1b6",
};

/** TIPS entries for isotopologues. */
function tipsOf(
  ...ids: ReadonlyArray<number>
): ReadonlyArray<CheckedFile & { readonly isotopologue: number }> {
  return ids.map((id) => ({ isotopologue: id, name: `q${id}.txt`, sha256: TIPS[id] ?? "" }));
}

/** The temperature step the refinement stops at, K (`decision-r08-t4d-kdist-chain.md`, ruling 4). */
export const FINEST_STEP_K = 12.5;

/** A species' first temperature nodes: every 50 K over its range (`decision-r08-t4d-kdist.md`, ruling 2). */
export function every50K([from, to]: readonly [number, number]): number[] {
  const nodes: number[] = [];
  for (let t = from; t <= to; t += 50) {
    nodes.push(t);
  }
  return nodes;
}

/**
 * The plain shape (`decision-r08-t4d-kdist-chain.md`, ruling 1): Voigt within 25 cm⁻¹ of each line's
 * centre at every pressure, no pedestal, `estimated` past γ_L = 0.25 cm⁻¹.
 */
const CUTOFF_25: ShapePolicy = {
  variants: [{ broadener: "air" }],
  cutoffPerCm: LINE_CUTOFF_PER_CM,
  pedestal: false,
  estimatedAboveHalfWidthPerCm: 0.25,
};

/**
 * H₂O's shape: its continuum's convention, Voigt within 25 cm⁻¹ less each line's pedestal at every
 * pressure, `estimated` past γ_L = 2.5 cm⁻¹ (`decision-r08-t4d-kdist.md` §5.2, §6.4).
 */
const CKD_CONVENTION: ShapePolicy = {
  variants: [{ broadener: "air" }],
  cutoffPerCm: LINE_CUTOFF_PER_CM,
  pedestal: true,
  estimatedAboveHalfWidthPerCm: 2.5,
};

/** CO₂'s temperature range, K. */
const CO2_RANGE_K: readonly [number, number] = [100, 500];

/**
 * CO₂'s shape (`decision-r08-t4d-kdist-chain.md`, rulings 2 and §7): Voigt × χ out to the fixed
 * distance where both sets' χ < 10⁻³ at every temperature of its range, two variants on shared
 * g-points: CO₂–CO₂'s χ for self-broadening and CO₂–N₂'s for every other gas.
 */
const CO2_CHI: ShapePolicy = {
  variants: [
    { broadener: "self", chi: CHI_CO2_CO2 },
    { broadener: "air", chi: CHI_CO2_N2 },
  ],
  cutoffPerCm: Math.max(
    chiReachPerCm(CHI_CO2_CO2, CO2_RANGE_K[0], CO2_RANGE_K[1]),
    chiReachPerCm(CHI_CO2_N2, CO2_RANGE_K[0], CO2_RANGE_K[1]),
  ),
  pedestal: false,
  estimatedAboveHalfWidthPerCm: 0.25,
};

/** What every HITRAN reduction's statement shares. */
const REDUCTION_COMMON =
  "on a quarter of the cell's narrowest Voigt half-width or finer; sorted into its k-distribution " +
  "and reduced to two-point Gauss-Legendre g-intervals shared by every node of the cell, by " +
  "bisection to 8e-4 in T over every column; temperature nodes 50 K apart, refined to 25 and 12.5 K " +
  "where the chain test (cubic in T on ln sigma, the rows, the read cubic in ln p on tau) fails 2e-3; " +
  "ln sigma stored in 16 bits a value (decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md).";

/** The plain shape's statement. */
const HITRAN_REDUCTION =
  "Each 1 nm vacuum cell's absorption line by line at each node (Voigt profiles, Humlicek 1982; " +
  "HITRAN's air widths and n_air at the node's air-equivalent broadening pressure, S(T) by TIPS; " +
  "cut off 25 cm-1 from each line's centre at every pressure, nodes whose cell's intensity-weighted " +
  "Lorentz half-width passes 0.25 cm-1 estimated, no far-wing shape being published for the " +
  "species), " +
  REDUCTION_COMMON;

/** H₂O's statement. */
const H2O_REDUCTION =
  "Each 1 nm vacuum cell's absorption line by line at each node (Voigt profiles, Humlicek 1982; " +
  "HITRAN's air widths and n_air at the node's air-equivalent broadening pressure, S(T) by TIPS; " +
  "cut off 25 cm-1 from each line's centre less its value there, the continuum's convention " +
  "(Clough, Kneizys and Davies 1989), at every pressure, nodes whose cell's intensity-weighted " +
  "Lorentz half-width passes 2.5 cm-1 estimated), " +
  REDUCTION_COMMON;

/** CO₂'s statement. */
const CO2_REDUCTION =
  "Each 1 nm vacuum cell's absorption line by line at each node (Voigt profiles, Humlicek 1982; " +
  "HITRAN's air widths and n_air at the node's air-equivalent broadening pressure, S(T) by TIPS), " +
  "each line's profile beyond 3 cm-1 times a far-wing factor chi and SpeCT's Eq. 11 shape (the " +
  "quantum asymmetry x/(1 - exp(-x)), x = c2 (nu - nu0)/T, and its detailed-balance term) out to " +
  `${CO2_CHI.cutoffPerCm} cm-1, where chi < 1e-3 at every temperature of 100-500 K: two variants on ` +
  "shared g-points, SpeCT's nu3-band sets (Chaverot et al. 2025, A&A 702, A137, Tables 2 and 3, " +
  "CC BY 4.0), CO2-CO2's (sigma 3, 50, 220 cm-1; B1 = 0.055 - 0.11 exp(-0.008 T), B2 = 0.021 + " +
  "0.08 exp(-0.0095 T), B3 = 0.01 cm; fitted at 218-751 K, Tran et al. 2011 and 2024, Burch et " +
  "al. 1969) for self-broadening and CO2-N2's (sigma 3, 10, 70 cm-1; B1 = 0.416 - 0.354 " +
  "exp(-0.00386 T), B2 = 0.00167 + 0.0421 exp(-0.00248 T), B3 = 0.019 cm; Perrin and Hartmann " +
  "1989's set marginally adapted, on their 296-773 K and Burch et al.'s 295 K measurements) for " +
  "every other gas, mixed at fixed g by the self share of the Lorentz width. Table 2 prints " +
  "CO2-N2's B2 alpha as 0.0167, a misprint for 0.00167: SpeCT's own code reads 0.00167 " +
  "(gitlab.com/ChaverotG/spect-public commit e7c4579c08, fcts.f90, chi_CO2N2, a finding aid, no " +
  "licence stated: one number read, no code used), Perrin and Hartmann's value, as 'marginally " +
  "adapted' implies; against SpeCT's lab compilation (Zenodo 15564294) with HITRAN2024's lines the " +
  "printed value errs -0.70 in mean ln(model/lab) at 296 K (Perrin and Hartmann) and -0.43 " +
  "(Burch et al.), the coded +0.04 and +0.33. The check: CO2-CO2 +0.04 (rms 0.11) against Tran " +
  "et al. 2024's 295 K data, Perrin and Hartmann's self set -0.71 (rms 1.10); CO2-N2 tied with " +
  "Perrin and Hartmann's within the lab sets' own disagreement (decision-r08-t4d-kdist-chain.md " +
  "§§12-14). Each set's coefficients held outside its fit's temperatures, those nodes flagged " +
  "(chiHeld) for R08.T10.a's bound on the body's own column, and nodes whose cell's " +
  "intensity-weighted Lorentz half-width passes 0.25 cm-1 estimated (chi fitted in the " +
  "infrared). " +
  REDUCTION_COMMON;

/**
 * O₂'s saturation (`decision-r08-t4d-kdist-chain.md` §4.1, a derivation from the triple point): the
 * triple point 54.36 K (O₂'s ITS-90 fixed point, 54.3584 K) and 146.25 Pa (as the ruling takes it;
 * R. Schmidt and W. Wagner, Fluid Phase Equilib. 19 (1985) 175, give 146.28 Pa, 0.02% apart);
 * ΔH_vap 7.63 kJ mol⁻¹ there, from the
 * slope of Brower and Thodos 1968's liquid Antoine fit (as NIST's WebBook gives it); ΔH_sub = ΔH_vap
 * + ΔH_fus, 0.445 kJ mol⁻¹ (Giauque and Johnston 1929, 106.3 cal mol⁻¹), 8.07 kJ mol⁻¹, so that
 * p_sat(50 K) = 30.8 Pa; the critical point 154.58 K (NIST). A constant ΔH_vap overstates p_sat
 * towards the critical point, which only widens the cap.
 */
const O2_SATURATION: SaturationCurve = {
  kind: "clausiusClapeyron",
  tripleK: 54.36,
  triplePa: 146.25,
  sublimationJPerMol: 8_070,
  vaporisationJPerMol: 7_630,
  criticalK: 154.58,
  source:
    "triple point 54.36 K (ITS-90) and 146.25 Pa (Schmidt and Wagner 1985: 146.28 Pa); dH_vap 7.63 kJ/mol from Brower and Thodos 1968 (NIST); " +
    "dH_fus 0.445 kJ/mol (Giauque and Johnston 1929); Tc 154.58 K (NIST)",
};

/**
 * CO₂'s saturation: Span and Wagner's ancillary equations (R. Span and W. Wagner, J. Phys. Chem.
 * Ref. Data 25 (1996) 1509, Eqs. 3.12 and 3.13, with their triple point 216.592 K, 0.51795 MPa, and
 * critical point 304.1282 K, 7.3773 MPa), which give 0.10145 MPa at 194.7 K. Eq. 3.12 is fitted
 * above 154 K and extrapolated here to CO₂'s 100 K (within 0.01 Pa above 85 K, their p. 1521).
 */
const CO2_SATURATION: SaturationCurve = {
  kind: "ancillary",
  tripleK: 216.592,
  triplePa: 517_950,
  criticalK: 304.1282,
  criticalPa: 7_377_300,
  sublimation: [
    [-14.740846, 1],
    [2.4327015, 1.9],
    [-5.3061778, 2.9],
  ],
  vapour: [
    [-7.0602087, 1],
    [1.9391218, 1.5],
    [-1.6463597, 2],
    [-3.2995634, 4],
  ],
  source: "Span and Wagner 1996, J. Phys. Chem. Ref. Data 25, 1509, Eqs. 3.12-3.13",
};

/**
 * The line species reduced from HITRAN2024 (R08.T4.d): each species' lines over 13,100–26,400 cm⁻¹,
 * fetched 2026-10-12 with every broadener HITRAN gives, and its grid. Temperatures run from where
 * the species' saturated column is negligible (`decision-r08-t4d-kdist.md` §3.1: H₂O from 150 K;
 * CO₂, N₂O and HCN from 100 K, below which a pure vapour at saturation over 40 p_sat ÷ (m g) at
 * 0.5 m s⁻² stays under 10⁻³ at k = 100; the others from 50 K) to HITRAN's 500 K; they are
 * `measured` to 296 K, HITRAN's 500 K values lacking hot lines (`decision-r08-t4b-t12c.md`,
 * follow-up 5). CO₂'s χ sets are held beyond their own fits' temperatures, those nodes flagged
 * for R08.T10.a's bound on the body's own column (`decision-r08-t4d-kdist-chain.md` §14).
 */
export const LINE_SOURCES: Readonly<Record<string, LineSource>> = {
  O2: {
    species: "O2",
    citation: `${HITRAN2024}; O2 lines 13100-26400 cm-1 (849 lines: 16O2, 16O18O, 16O17O)`,
    url: hitranUrl([36, 37, 38]),
    lines: {
      name: "o2-13100-26400-broad.csv",
      sha256: "e2b53f8b13602f541f35fe8ac4a0e9d5e33a5fa9e6f856e5a5bfe141bb2af3a8",
    },
    tips: tipsOf(36, 37, 38),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [50, 296],
    rangeK: [50, 500],
    policy: CUTOFF_25,
    molecularMassDa: 31.98983,
    saturation: O2_SATURATION,
    reduction: HITRAN_REDUCTION,
  },
  H2O: {
    species: "H2O",
    citation: `${HITRAN2024}; H2O lines 13100-26400 cm-1 (133187 lines: H2(16O), H2(18O), H2(17O), HD(16O))`,
    url: hitranUrl([1, 2, 3, 4]),
    lines: {
      name: "h2o-13100-26400-broad.csv",
      sha256: "18b9d052654c6dd28974d9ff29e13f2f51eff2a13e27d028429c06c92018aae4",
    },
    tips: tipsOf(1, 2, 3, 4),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [150, 296],
    rangeK: [150, 500],
    policy: CKD_CONVENTION,
    molecularMassDa: 18.010565,
    reduction: H2O_REDUCTION,
  },
  CO2: {
    species: "CO2",
    citation: `${HITRAN2024}; CO2 lines 13100-26400 cm-1 (1204 lines: 12C16O2, 13C16O2, 16O12C18O)`,
    url: hitranUrl([7, 8, 9, 10, 11, 12, 13, 14, 121, 15, 120, 122]),
    lines: {
      name: "co2-13100-26400-broad.csv",
      sha256: "00e953dfd16bca0768d46ae4264f569b1e758a22941399ae1aac8e96ecd0efd5",
    },
    tips: tipsOf(7, 8, 9),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [100, 296],
    rangeK: CO2_RANGE_K,
    policy: CO2_CHI,
    molecularMassDa: 43.98983,
    saturation: CO2_SATURATION,
    reduction: CO2_REDUCTION,
  },
  CO: {
    species: "CO",
    citation: `${HITRAN2024}; CO lines 13100-26400 cm-1 (49 lines of 12C16O's 7-0 band; Medvedev and Ushakov 2022, DOI 10.1016/j.jqsrt.2022.108255)`,
    url: hitranUrl([26, 27, 28, 29, 30, 31]),
    lines: {
      name: "co-13100-26400-broad.csv",
      sha256: "a3bebd841b73c01611c02ef2b8fffef19a862bbb366fc49ffc31a6ab7894e48f",
    },
    tips: tipsOf(26),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [50, 296],
    rangeK: [50, 500],
    policy: CUTOFF_25,
    molecularMassDa: 27.994915,
    reduction: HITRAN_REDUCTION,
  },
  N2O: {
    species: "N2O",
    citation: `${HITRAN2024}; N2O lines 13100-26400 cm-1 (184 lines of 14N2-16O to 14017.06 cm-1; ABG-IMRHT, Huang, Tashkun and Schwenke 2025, DOI 10.1016/j.jqsrt.2025.109502)`,
    url: hitranUrl([21, 22, 23, 24, 25]),
    lines: {
      name: "n2o-13100-26400-broad.csv",
      sha256: "3bdc91e1a512e4f91258dd6e9488a667a8d638819c26db518ad168ae43d12910",
    },
    tips: tipsOf(21),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [100, 296],
    rangeK: [100, 500],
    policy: CUTOFF_25,
    molecularMassDa: 44.001062,
    reduction: HITRAN_REDUCTION,
  },
  HCN: {
    species: "HCN",
    citation: `${HITRAN2024}; HCN lines 13100-26400 cm-1 (1918 lines of H12C14N to 17585.79 cm-1)`,
    url: hitranUrl([70, 71, 72]),
    lines: {
      name: "hcn-13100-26400-broad.csv",
      sha256: "5916d38ea32789b91e0d9595ef121c934eb31d8c25c63ce58ad1bd93482acc33",
    },
    tips: tipsOf(70),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [100, 296],
    rangeK: [100, 500],
    policy: CUTOFF_25,
    molecularMassDa: 27.010899,
    reduction: HITRAN_REDUCTION,
  },
  H2: {
    species: "H2",
    citation: `${HITRAN2024}; H2 lines 13100-26400 cm-1 (5558 electric-quadrupole and HD lines: 1H2, HD)`,
    url: hitranUrl([103, 115]),
    lines: {
      name: "h2-13100-26400-broad.csv",
      sha256: "b098d083b0af37a89a36070e6eecbcc0d7cd6903c0345a0548bc086105163dbd",
    },
    tips: tipsOf(103, 115),
    fetched: "2026-10-12",
    terms: HITRAN_TERMS,
    basis: "measured",
    measuredRangeK: [50, 296],
    rangeK: [50, 500],
    policy: CUTOFF_25,
    molecularMassDa: 2.01565,
    reduction: HITRAN_REDUCTION,
  },
};

/**
 * What every overtone-ladder species' statement shares: R08.T4.c's stated estimate, now on the
 * k-distribution machinery (`decision-r08-t4b-t12c.md`, follow-up 3, ruling 3), `estimated`.
 */
const LADDER_REDUCTION =
  "A stated overtone-ladder estimate (decision-r08-t4b-t12c.md follow-up 3, ruling 3; no line list " +
  "reaches 380-760 nm and no measured visible intensity was read, the papers found being closed): " +
  "the anchor band's HITRAN2024 lines, each moved by its offset from the band's intensity-weighted " +
  "centroid to the target polyad and divided by 10 for each quantum, keeping its lower-state energy " +
  "and widths (R08.T4.c's construction, whose 1 nm cell means it reproduces). " +
  HITRAN_REDUCTION;

/** An overtone-ladder species' source: its anchor's `.par` file, band and target. */
function ladderSource(
  species: string,
  anchor: string,
  file: CheckedFile,
  url: string,
  band: LadderBand,
  molecularMassDa: number,
): LineSource {
  return {
    species,
    citation: `${HITRAN2024}; ${anchor}, moved to ${band.targetPerCm} cm-1 and scaled by 10^-${band.quanta}`,
    url,
    lines: file,
    tips: tipsOf(band.isotopologue),
    fetched: "2026-10-12 (matching R08.T4.c's fetch of 2026-10-10 and 2026-10-11)",
    terms: HITRAN_TERMS,
    basis: "estimated",
    measuredRangeK: [50, 296],
    rangeK: [50, 500],
    policy: CUTOFF_25,
    molecularMassDa,
    reduction: LADDER_REDUCTION,
    ladder: band,
  };
}

/** HITRAN's `.par` request for a molecule's isotopologues over a range, as R08.T4.c made it. */
function parUrl(isotopologues: ReadonlyArray<number>, low: number, high: number): string {
  return `https://hitran.org/lbl/api?iso_ids_list=${isotopologues.join(",")}&numin=${low}&numax=${high}&fixwidth=0&request_params=par_line`;
}

/**
 * The overtone-ladder species (R08.T4.d's "C₂H₂, C₂H₄, C₂H₆, PH₃ and CH₃OH"), each R08.T4.c's
 * anchor and target, so that their cells reproduce its bounds: C₂H₂'s 3ν₃ to its five-quantum
 * polyad near 15,600 cm⁻¹ (10⁻²); C₂H₄'s ν₉ and C₂H₆'s ν₇ to five CH quanta near 14,300 and
 * 13,800 cm⁻¹ (10⁻⁴); CH₃OH's ν₈ to the OH stretch's Δv = 4 near 13,600 cm⁻¹ (10⁻³, likely over ten
 * times high); PH₃'s ν₁ + ν₃ to seven PH quanta, placed at 13,700 cm⁻¹ (10⁻⁵).
 */
export const LADDER_SOURCES: Readonly<Record<string, LineSource>> = {
  C2H2: ladderSource(
    "C2H2",
    "12C2H2's 3nu3 band (9639.08 cm-1, S 5.4896e-21 cm)",
    {
      name: "c2h2-7000-27000.par",
      sha256: "daed4246bd6181390fdbae6bdd45c961ed23d5544849dff8f7d09897ffdc0739",
    },
    parUrl([76, 77, 105], 7_000, 27_000),
    {
      isotopologue: 76,
      upper: "003 0 0 0 0+ u",
      lower: "000 0 0 0 0+ g",
      targetPerCm: 15_600,
      quanta: 2,
    },
    26.01565,
  ),
  C2H4: ladderSource(
    "C2H4",
    "12C2H4's nu9 band (3114.52 cm-1, S 3.1154e-18 cm)",
    {
      name: "c2h4-0-27000.par",
      sha256: "a1e8ce7716aa9ea93621a39d55a07e1274725c001270b30aab99d31c130129ea",
    },
    parUrl([90, 91], 0, 27_000),
    { isotopologue: 90, upper: "V9", lower: "GROUND", targetPerCm: 14_300, quanta: 4 },
    28.0313,
  ),
  C2H6: ladderSource(
    "C2H6",
    "12C2H6's nu7 band (2988.44 cm-1, S 1.2010e-17 cm)",
    {
      name: "c2h6-2000-27000.par",
      sha256: "7c1108e618510fbf7dbcc6858cd270af5475122714e57322e95ecb95b8786bd1",
    },
    parUrl([78, 106], 2_000, 27_000),
    { isotopologue: 78, upper: "V7", lower: "GROUND", targetPerCm: 13_800, quanta: 4 },
    30.04695,
  ),
  CH3OH: ladderSource(
    "CH3OH",
    "CH3OH's nu8 C-O stretch (1033.41 cm-1, S 1.1681e-17 cm)",
    {
      name: "ch3oh-0-27000.par",
      sha256: "2aa4bef0039d2fa101f2c736af07b475de1bd6c57f31afb0bebfa561a64a6fdb",
    },
    parUrl([92], 0, 27_000),
    { isotopologue: 92, upper: "V8", lower: "GROUND", targetPerCm: 13_600, quanta: 3 },
    32.026215,
  ),
  PH3: ladderSource(
    "PH3",
    "PH3's nu1+nu3 band (4577.88 cm-1, S 5.8545e-20 cm)",
    {
      name: "ph3-1500-27000.par",
      sha256: "c7f62f8c073db4429b9ffeb8e0fce6d0beed2a016fb30679778a8c3fcf4851dd",
    },
    parUrl([79], 1_500, 27_000),
    { isotopologue: 79, upper: "1 0 1 0", lower: "0 0 0 0", targetPerCm: 13_700, quanta: 5 },
    33.997241,
  ),
};

/** The largest part a species' table is split into, bytes: under the 500 kB hook. */
export const PART_BYTES = 480_000;

/** A cell as a partial file holds it: its 16-bit values and its test records. */
export interface PartialCell {
  readonly nm: number;
  readonly w: ReadonlyArray<number>;
  readonly o: number;
  readonly s: number;
  readonly k: string;
  readonly worstNodeError: number;
  readonly worstChain: ChainError;
  readonly worstBeyondCap: ChainError;
  readonly failures: ReadonlyArray<ChainError>;
  /** A two-variant cell's fixed-g mix (`CellMixing`), its columns null where ∞, as JSON writes it. */
  readonly mixing?: {
    readonly worst: ReadonlyArray<ChainError>;
    readonly limitColumnsCm2: ReadonlyArray<number | null>;
    /** The tested temperatures, K, and `[share][t]` u_mix at each alone (null for none). */
    readonly temperaturesK?: ReadonlyArray<number>;
    readonly byTemperature?: ReadonlyArray<ReadonlyArray<number | null>>;
  };
  readonly measuredShapeTo: ReadonlyArray<number>;
  readonly beyondCutoffShare: ReadonlyArray<number>;
  readonly nearWingShare: ReadonlyArray<number>;
  /** The 16-bit test's worst within the cell's column caps ({@link quantisationError}). */
  readonly quantisation: number;
  /** Its worst beyond them, recorded; absent from partials written before it was split. */
  readonly quantisationBeyondCap?: number;
  readonly belowGrid: number;
  /**
   * A merged cell's means (R08.T4.g, N₂O from 714 nm): [T, p, the table's f64 Σ w σ, HITRAN2024's
   * lines', TYM's added bands'], K, Pa and m² a molecule, at the recorded nodes.
   */
  readonly merge?: ReadonlyArray<readonly [number, number, number, number, number]>;
}

/** A partial file: a range of a species' cells. */
export interface PartialFile {
  readonly species: string;
  readonly temperaturesK: ReadonlyArray<number>;
  readonly pressuresPa: ReadonlyArray<number>;
  readonly broadening: Readonly<Record<string, number>>;
  /**
   * Each gas's temperature exponent over air's ({@link broadeningExponentOffsets}); absent from
   * the partials written before `decision-r08-t4d-kdist-chain.md` §13, which the merge's `--lines`
   * makes good.
   */
  readonly broadeningExponents?: Readonly<Record<string, number>>;
  readonly cells: ReadonlyArray<PartialCell>;
}

/** A line species' source by its registry key: HITRAN's lists, then the ladder's estimates. */
export function sourceOf(species: string | undefined): LineSource | undefined {
  return LINE_SOURCES[species ?? ""] ?? LADDER_SOURCES[species ?? ""];
}

/** A species' lines and partition sums from its checked files. */
export function readLineSpecies(
  source: LineSource,
  linesPath: string,
  tipsDir: string,
): LineSpecies {
  return {
    lines: readLineList(source, linesPath),
    partitionSums: new Map(
      source.tips.map((t) => [
        t.isotopologue,
        parsePartitionSums(readCheckedText(join(tipsDir, t.name), t.sha256)),
      ]),
    ),
  };
}

/** A species' lines from its checked file: HITRAN's records, or its ladder's moved band. */
export function readLineList(source: LineSource, linesPath: string): LineList {
  const text = readCheckedText(linesPath, source.lines.sha256);
  return source.ladder === undefined ? parseHitranLines(text) : ladderLines(text, source.ladder);
}

/** The species' grid: its temperature nodes (50 K apart unless given) and the stored pressures. */
export function gridOf(source: LineSource, temperaturesK?: ReadonlyArray<number>): ReductionGrid {
  return {
    temperaturesK: temperaturesK ?? every50K(source.rangeK),
    pressuresPa: STORED_PRESSURES_PA,
  };
}

/**
 * What reducing a cell needs of its species besides its spectra: its shape, mass, condensation and
 * Rayleigh scattering.
 */
export interface CellReductionSource {
  readonly species: string;
  readonly policy: ShapePolicy;
  readonly molecularMassDa: number;
  readonly saturation?: SaturationCurve;
  /**
   * The species' measured Rayleigh cross-section, whose column the between-node test also stops at
   * (R08.T4.g's ruling, {@link chainColumnCm2}); helium's where absent, as T4.d's tables were gated.
   */
  readonly rayleighCm2?: RayleighCrossSection;
}

/**
 * One cell reduced on its grid and tested ({@link reduceGridCell}, {@link quantisationError},
 * {@link belowGridError}) as a partial file holds it, with its line logged, beside its `f64`
 * reduction; or `undefined` where the cell has no line within the cut-off. T4.d's line-by-line
 * cells and T4.g's super-line cells alike.
 */
export function reducePartialCell(
  cellNm: number,
  spectra: NodeSpectra,
  grid: ReductionGrid,
  source: CellReductionSource,
): { readonly partial: PartialCell; readonly reduced: ReducedCell } | undefined {
  const started = performance.now();
  const nodes = source.policy.variants.length * grid.temperaturesK.length * grid.pressuresPa.length;
  const cap = (t: number, p: number): number =>
    gatedColumnCm2(source.saturation, cellNm, t, p, source.molecularMassDa);
  const chainCap = (t: number, p: number): number =>
    chainColumnCm2(source.saturation, cellNm, t, p, source.molecularMassDa, source.rayleighCm2);
  const cell = reduceGridCell(cellNm, spectra, grid, source.policy, cap, chainCap);
  if (cell === undefined) {
    return undefined;
  }
  const encoded = encodeCell(cell);
  const nodesPerVariant = grid.temperaturesK.length * grid.pressuresPa.length;
  const quantisation = quantisationError(cell, encoded, nodes, (n) => {
    const i = n % nodesPerVariant;
    const nP = grid.pressuresPa.length;
    return cap(grid.temperaturesK[Math.floor(i / nP)] ?? 0, grid.pressuresPa[i % nP] ?? 0);
  });
  const partial: PartialCell = {
    nm: cellNm,
    w: Array.from(cell.weights),
    o: encoded.lnOffset,
    s: encoded.lnStep,
    k: Buffer.from(
      encoded.codes.buffer,
      encoded.codes.byteOffset,
      encoded.codes.byteLength,
    ).toString("base64"),
    worstNodeError: cell.worstNodeError,
    worstChain: cell.worstChain,
    worstBeyondCap: cell.worstBeyondCap,
    failures: cell.failures,
    ...(cell.mixing === undefined
      ? {}
      : {
          mixing: {
            worst: cell.mixing.worst,
            limitColumnsCm2: cell.mixing.limitColumnsCm2.map((u) =>
              Number.isFinite(u) ? u : null,
            ),
            temperaturesK: cell.mixing.temperaturesK,
            byTemperature: cell.mixing.limitColumnsByTemperatureCm2.map((row) =>
              row.map((u) => (Number.isFinite(u) ? u : null)),
            ),
          },
        }),
    measuredShapeTo: Array.from(cell.measuredShapeTo),
    beyondCutoffShare: Array.from(cell.beyondCutoffShare),
    nearWingShare: Array.from(cell.nearWingShare),
    quantisation: quantisation.within,
    quantisationBeyondCap: quantisation.beyond,
    belowGrid: belowGridError(cellNm, spectra, grid, source.policy, cell, source.molecularMassDa),
  };
  const c = cell.worstChain;
  const mixing =
    cell.mixing === undefined
      ? ""
      : `, mixing ${cell.mixing.worst.map((e) => e.error.toExponential(2)).join("/")} at w ${MIX_SHARES.join("/")}`;
  process.stdout.write(
    `${source.species} ${cellNm} nm: G ${cell.weights.length}, node ${cell.worstNodeError.toExponential(2)}, chain ${c.error.toExponential(2)} at ${c.temperatureK} K ${c.pressurePa.toPrecision(3)} Pa ${c.columnCm2.toExponential(2)} cm-2, beyond cap ${cell.worstBeyondCap.error.toExponential(2)}, ${cell.failures.length} failing${mixing}, ${((performance.now() - started) / 1000).toFixed(1)} s\n`,
  );
  return { partial, reduced: cell };
}

/** `k-distributions`: reduces a range of a species' cells to a partial file. */
export function runKDistributions(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: {
      species: { type: "string" },
      lines: { type: "string" },
      tips: { type: "string" },
      cells: { type: "string" },
      stride: { type: "string" },
      temperatures: { type: "string" },
      out: { type: "string" },
    },
  });
  const source = sourceOf(values.species);
  if (
    source === undefined ||
    values.lines === undefined ||
    values.tips === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: k-distributions --species <key> --lines <csv> --tips <dir> [--cells a-b] [--stride n:k] [--temperatures t,...] --out <json>",
    );
  }
  const [from, to] = (values.cells ?? "380-760").split("-").map(Number);
  const species = readLineSpecies(source, values.lines, values.tips);
  const spectra = lineByLine(species, source.policy);
  const grid = gridOf(source, values.temperatures?.split(",").map(Number));
  const cells: PartialCell[] = [];
  // Every stride-th cell from the first's offset, so that processes sharing a species balance.
  const [stride, offset] = (values.stride ?? "1:0").split(":").map(Number);
  for (let cellNm = (from ?? 380) + (offset ?? 0); cellNm <= (to ?? 760); cellNm += stride ?? 1) {
    const cell = reducePartialCell(cellNm, spectra, grid, source);
    if (cell !== undefined) {
      cells.push(cell.partial);
    }
  }
  const partial: PartialFile = {
    species: source.species,
    temperaturesK: grid.temperaturesK,
    pressuresPa: grid.pressuresPa,
    broadening: broadeningRatios(species.lines),
    broadeningExponents: broadeningExponentOffsets(species.lines),
    cells,
  };
  writeFileSync(values.out, `${JSON.stringify(partial)}\n`);
}

/** Whether a value read from a partial is a {@link PartialFile}: a trust boundary of our own writing. */
function isPartial(value: unknown): value is PartialFile {
  return typeof value === "object" && value !== null && "cells" in value && "species" in value;
}

/** What the merge found of the chain test's failures, by where they lie on the temperature grid. */
export interface FailureSorting {
  /** The intervals [t, t + 1] whose failures can still be refined: wider than 12.5 K. */
  readonly refine: ReadonlyArray<number>;
  /** The intervals failing at 12.5 K, recorded and `estimated`. */
  readonly estimated: ReadonlyArray<readonly [number, number]>;
  /** Failures at a node's temperature, between pressure rows, which refinement cannot reach. */
  readonly atNodes: ReadonlyArray<ChainError>;
}

/**
 * The chain test's failures sorted onto the temperature grid: a failure between nodes t and t + 1
 * refines that interval while it is wider than {@link FINEST_STEP_K}, and is recorded and
 * `estimated` once it is not (`decision-r08-t4d-kdist-chain.md`, ruling 4).
 */
export function sortFailures(
  temperaturesK: ReadonlyArray<number>,
  failures: ReadonlyArray<ChainError>,
): FailureSorting {
  const refine = new Set<number>();
  const estimated = new Map<number, readonly [number, number]>();
  const atNodes: ChainError[] = [];
  for (const f of failures) {
    const t = temperaturesK.findIndex(
      (lo, i) => lo < f.temperatureK && f.temperatureK < (temperaturesK[i + 1] ?? -Infinity),
    );
    if (t < 0) {
      atNodes.push(f);
      continue;
    }
    const lo = temperaturesK[t] ?? 0;
    const hi = temperaturesK[t + 1] ?? 0;
    if (hi - lo > FINEST_STEP_K + 1e-9) {
      refine.add(t);
    } else {
      estimated.set(t, [lo, hi]);
    }
  }
  return {
    refine: [...refine].toSorted((a, b) => a - b),
    estimated: [...estimated.entries()].toSorted((a, b) => a[0] - b[0]).map(([, v]) => v),
    atNodes,
  };
}

/** The grid with each interval to refine halved. */
export function refinedTemperatures(
  temperaturesK: ReadonlyArray<number>,
  refine: ReadonlyArray<number>,
): number[] {
  const halves = new Set(refine);
  return temperaturesK.flatMap((t, i) =>
    halves.has(i) ? [t, 0.5 * (t + (temperaturesK[i + 1] ?? t))] : [t],
  );
}

/** A chain test's failure, for the merge's log. */
function describe(e: ChainError): string {
  return `${e.error.toExponential(2)} at ${e.temperatureK} K ${e.pressurePa.toPrecision(3)} Pa ${e.columnCm2.toExponential(2)} cm-2`;
}

/**
 * Partials with their species' broadening records taken from its lines ({@link broadeningRatios}
 * and {@link broadeningExponentOffsets}), their own set aside: the merge's `--lines`, which makes
 * good the partials written before `decision-r08-t4d-kdist-chain.md` §13.
 */
export function withLineBroadening(
  partials: ReadonlyArray<PartialFile>,
  lines: LineList,
): PartialFile[] {
  const broadening = broadeningRatios(lines);
  const broadeningExponents = broadeningExponentOffsets(lines);
  return partials.map((p) => Object.assign({}, p, { broadening, broadeningExponents }));
}

/** A partial's broadening records as one comparable string. */
function broadeningRecords(partial: PartialFile): string {
  return JSON.stringify([partial.broadening, partial.broadeningExponents ?? null]);
}

/** What merging a species' partials comes to. */
export type MergeOutcome =
  /** The chain test fails between nodes wider than 12.5 K apart: run again on these nodes. */
  | { readonly kind: "refine"; readonly temperaturesK: ReadonlyArray<number> }
  /** A failure refinement cannot reach, at a node or of the mix or the node tier. */
  | { readonly kind: "refused"; readonly reason: string }
  /** The species' sidecar and its binary parts, each part's file named. */
  | {
      readonly kind: "written";
      readonly sidecar: string;
      readonly parts: ReadonlyArray<{ readonly file: string; readonly bytes: Uint8Array }>;
      readonly summary: string;
    };

/**
 * What a table's sidecar names of its source besides the cells: a HITRAN {@link LineSource}'s
 * ({@link lineTableSource}) or one of R08.T4.g's ExoMol lists' (`exomolCli.ts`'s
 * `tableSourceOf`).
 */
export interface TableSource {
  readonly species: string;
  readonly citation: string;
  readonly url: string;
  /** Every file the table was reduced from with its SHA-256, as the sidecar's `sha256` lists them. */
  readonly inputs: string;
  readonly fetched: string;
  readonly terms: string;
  readonly basis: "measured" | "estimated";
  readonly measuredRangeK: readonly [number, number];
  readonly policy: ShapePolicy;
  readonly reduction: string;
  /** The sidecar's description where it is not R08.T4.d's: an ExoMol list's, naming its licence. */
  readonly description: string | undefined;
}

/** A HITRAN source as its sidecar names it: its lines file and its TIPS files the inputs. */
export function lineTableSource(source: LineSource): TableSource {
  return {
    species: source.species,
    citation: source.citation,
    url: source.url,
    inputs: `${source.lines.name} ${source.lines.sha256}; ${source.tips.map((t) => `${t.name} ${t.sha256}`).join(", ")}`,
    fetched: source.fetched,
    terms: source.terms,
    basis: source.basis,
    measuredRangeK: source.measuredRangeK,
    policy: source.policy,
    reduction: source.reduction,
    description: undefined,
  };
}

/**
 * A species' partials merged and checked against the rulings' bounds: refine where the chain test
 * fails between nodes it can still split, refuse where it fails otherwise, and the sidecar and
 * binary parts, each under {@link PART_BYTES}, where every cell passes.
 *
 * @param log - Takes each line of the merge's record: every failure and the species' worst figures.
 * @throws Error for no partial, or partials on different temperature grids.
 */
export function mergePartials(
  source: TableSource,
  partials: ReadonlyArray<PartialFile>,
  log: (line: string) => void,
): MergeOutcome {
  const first = partials[0];
  if (first === undefined) {
    throw new Error("no partial");
  }
  const temperaturesK = first.temperaturesK;
  if (partials.some((p) => p.temperaturesK.join() !== temperaturesK.join())) {
    throw new Error(`${source.species}'s partials are on different temperature grids`);
  }
  const exponents = first.broadeningExponents;
  if (exponents === undefined) {
    return {
      kind: "refused",
      reason: "the partials hold no broadening exponents (written before §13): merge with --lines",
    };
  }
  if (partials.some((q) => broadeningRecords(q) !== broadeningRecords(first))) {
    return {
      kind: "refused",
      reason: "the partials' broadening records differ: merge with --lines",
    };
  }
  const cells = partials.flatMap((p) => p.cells).toSorted((a, b) => a.nm - b.nm);
  const worst = (of: (c: PartialCell) => number): number => Math.max(0, ...cells.map(of));
  const nodeWorst = worst((c) => c.worstNodeError + c.quantisation);
  const chainWorst = worst((c) => c.worstChain.error);
  const beyondWorst = worst((c) => c.worstBeyondCap.error);
  const quantisationWorst = worst((c) => c.quantisation);
  const belowWorst = worst((c) => c.belowGrid);
  const mixed = cells.some((c) => c.mixing !== undefined);
  const mixingWorst = mixed
    ? worst((c) => Math.max(0, ...(c.mixing?.worst.map((e) => e.error) ?? [])))
    : undefined;
  const sorting = sortFailures(
    temperaturesK,
    cells.flatMap((c) => c.failures),
  );
  const nodeFailures = cells.filter(
    (c) => c.worstNodeError + c.quantisation > NODE_BOUND || c.quantisation > QUANTISATION_BOUND,
  );
  // The mix is recorded, not gated (§16), by tested temperature, a sample inside an estimated
  // interval setting none (§17): a cell whose mix errs past the tier within the cap logs the least
  // column at which it does, at each share.
  const mixTemperaturesK = cells.find((c) => c.mixing !== undefined)?.mixing?.temperaturesK;
  if (
    cells.some(
      (c) =>
        c.mixing !== undefined &&
        (c.mixing.byTemperature === undefined ||
          c.mixing.temperaturesK?.join() !== mixTemperaturesK?.join()),
    )
  ) {
    return {
      kind: "refused",
      reason:
        "the partials' mix records hold no temperatures, or differ in them (written before §17)",
    };
  }
  const inEstimated = (t: number): boolean => sorting.estimated.some(([a, b]) => t > a && t < b);
  const limitsOf = (c: PartialCell): number[][] =>
    MIX_SHARES.map((_, i) =>
      (mixTemperaturesK ?? []).map((t, k) => {
        const u = c.mixing?.byTemperature?.[i]?.[k] ?? null;
        return u === null || inEstimated(t) ? Infinity : u;
      }),
    );
  const mixingRecords = cells.filter((c) =>
    limitsOf(c).some((row) => row.some((u) => Number.isFinite(u))),
  );
  for (const c of nodeFailures) {
    log(
      `node fails: ${c.nm} nm ${c.worstNodeError.toExponential(2)} + ${c.quantisation.toExponential(2)}\n`,
    );
  }
  for (const c of mixingRecords) {
    const least = limitsOf(c).map((row) => {
      const u = Math.min(...row);
      return Number.isFinite(u) ? u.toExponential(2) : "none";
    });
    log(
      `mixing records: ${c.nm} nm least u_mix ${least.join("/")} cm-2 at w ${MIX_SHARES.join("/")}\n`,
    );
  }
  for (const c of cells) {
    for (const f of c.failures) {
      log(`chain fails: ${c.nm} nm ${describe(f)}\n`);
    }
  }
  log(
    `${source.species}: ${cells.length} cells; worst node ${nodeWorst.toExponential(2)} (quantisation ${quantisationWorst.toExponential(2)}), worst chain ${chainWorst.toExponential(2)}, beyond cap ${beyondWorst.toExponential(2)}, 1 Pa ${belowWorst.toExponential(2)}${mixingWorst === undefined ? "" : `, mixing ${mixingWorst.toExponential(2)}`}\n`,
  );
  if (sorting.refine.length > 0) {
    return { kind: "refine", temperaturesK: refinedTemperatures(temperaturesK, sorting.refine) };
  }
  if (sorting.atNodes.length > 0 || nodeFailures.length > 0) {
    return {
      kind: "refused",
      reason: `${sorting.atNodes.length} failures at node temperatures, ${nodeFailures.length} node failures`,
    };
  }
  const nT = temperaturesK.length;
  const nP = first.pressuresPa.length;
  const measuredShapeTo = temperaturesK.map((_, t) =>
    Math.min(...cells.map((c) => c.measuredShapeTo[t] ?? -1)),
  );
  const shareOf = (of: (c: PartialCell) => ReadonlyArray<number>): number[] =>
    Array.from({ length: nT * nP }, (_, n) => Math.max(0, ...cells.map((c) => of(c)[n] ?? 0)));
  const encoded: EncodedCell[] = cells.map((c) => {
    const bytes = Buffer.from(c.k, "base64");
    const codes = new Uint16Array(bytes.length / 2);
    for (let i = 0; i < codes.length; i += 1) {
      codes[i] = bytes.readUInt16LE(2 * i);
    }
    return { cellNm: c.nm, weights: Float64Array.from(c.w), lnOffset: c.o, lnStep: c.s, codes };
  });
  const meta: KDistributionMeta = {
    species: source.species,
    source: source.citation,
    url: source.url,
    sha256: source.inputs,
    fetched: source.fetched,
    terms: source.terms,
    reduction: source.reduction,
    basis: source.basis,
    measuredRangeK: source.measuredRangeK,
    measuredBelowGrid: belowWorst <= CHAIN_BOUND,
    measuredShapeTo,
    estimatedTemperaturesK: sorting.estimated,
    broadening: first.broadening,
    broadeningExponents: exponents,
    variants: source.policy.variants.map((v) => v.broadener),
    chiHeld: chiHeldFlags(source.policy, temperaturesK),
    beyondCutoffShare: shareOf((c) => c.beyondCutoffShare),
    nearWingShare: shareOf((c) => c.nearWingShare),
    worstNodeError: nodeWorst,
    worstChainError: chainWorst,
    worstBeyondCapError: beyondWorst,
    ...(mixed
      ? {
          mixing: {
            shares: [...MIX_SHARES],
            temperaturesK: mixTemperaturesK ?? [],
            worstErrors: MIX_SHARES.map((_, i) =>
              Math.max(0, ...cells.map((c) => c.mixing?.worst[i]?.error ?? 0)),
            ),
            limitColumnsCm2: cells.map(limitsOf),
          },
        }
      : {}),
    worstQuantisationError: quantisationWorst,
    ...(cells.some((c) => c.quantisationBeyondCap !== undefined)
      ? { worstQuantisationBeyondCapError: worst((c) => c.quantisationBeyondCap ?? 0) }
      : {}),
    linearCells: encoded.filter((c) => c.weights.length === 1).length,
    ...(source.description === undefined ? {} : { description: source.description }),
  };
  const grid = { temperaturesK, pressuresPa: first.pressuresPa };
  const variants = source.policy.variants.length;
  // Each cell whole in one part, so that a part is read without its neighbours.
  const groups: EncodedCell[][] = [[]];
  let size = 16;
  for (const cell of encoded) {
    const current = groups.at(-1) ?? [];
    if (current.length > 0 && size + cellBytes(cell) > PART_BYTES) {
      groups.push([cell]);
      size = 16 + cellBytes(cell);
    } else {
      current.push(cell);
      size += cellBytes(cell);
    }
  }
  const parts = groups.map((group, i) => ({
    file: `${source.species}-${i + 1}.bin`,
    cells: group.length,
    bytes: kDistributionPart(grid, variants, group),
  }));
  const listing: KDistributionPart[] = parts.map((part) => ({
    file: part.file,
    cells: part.cells,
    bytes: part.bytes.length,
    sha256: createHash("sha256").update(part.bytes).digest("hex"),
  }));
  const total = listing.reduce((sum, p) => sum + p.bytes, 0);
  return {
    kind: "written",
    sidecar: kDistributionMetaText(meta, grid, listing),
    parts: parts.map(({ file, bytes }) => ({ file, bytes })),
    summary: `${source.species}: written in ${parts.length} parts, ${total} bytes, ${meta.linearCells} cells of G = 1; ${sorting.estimated.length} intervals estimated at 12.5 K (${sorting.estimated.map(([a, b]) => `${a}-${b} K`).join(", ")})\n`,
  };
}

/**
 * `k-distributions-merge`: checks a species' partials and writes its parts ({@link mergePartials}),
 * or, where the chain test fails between nodes it can still refine, prints the refined nodes.
 *
 * @returns The exit code: 0 when written, 2 to refine, 3 when refused.
 */
export function runKDistributionsMerge(rest: readonly string[]): number {
  const { values, positionals } = parseArgs({
    args: [...rest],
    allowPositionals: true,
    options: {
      species: { type: "string" },
      "out-dir": { type: "string" },
      lines: { type: "string" },
    },
  });
  const source = sourceOf(values.species);
  const outDir = values["out-dir"];
  if (source === undefined || outDir === undefined || positionals.length === 0) {
    throw new Error(
      "usage: k-distributions-merge --species <key> --out-dir <dir> [--lines <file>] <partial>…",
    );
  }
  // With the species' lines, its broadening records are taken from them, the partials' set aside
  // (those written before decision-r08-t4d-kdist-chain.md §13 hold no exponents).
  const lines = values.lines === undefined ? undefined : readLineList(source, values.lines);
  const partials = positionals.map((path): PartialFile => {
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    if (!isPartial(parsed) || parsed.species !== source.species) {
      throw new Error(`${path} is not a partial of ${source.species}`);
    }
    return parsed;
  });
  const outcome = mergePartials(
    lineTableSource(source),
    lines === undefined ? partials : withLineBroadening(partials, lines),
    (line) => process.stdout.write(line),
  );
  let code: number;
  switch (outcome.kind) {
    case "refine":
      process.stdout.write(`refine: ${outcome.temperaturesK.join(",")}\n`);
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
      writeFileSync(join(outDir, `${source.species}.json`), outcome.sidecar);
      process.stdout.write(outcome.summary);
      code = 0;
      break;
  }
  return code;
}

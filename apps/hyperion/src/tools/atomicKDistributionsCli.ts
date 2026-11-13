/**
 * `node scripts/atmosphereData.mjs atomic-…` and `alkali-pairs` (plan R08, R08.T4.e;
 * decision-r08-t4ef-storage and decision-r08-t4e-budget): the drawn atoms' and ions' visibility
 * floors, the projection of their tables' bytes from sample cells before any full reduction
 * ("each task projects its bytes first"), their tables on R08.T4.d's stride, merge and refine
 * protocol, their reduced line lists, their line-by-line references, and the alkali far wings'
 * pairs.
 *
 * @remarks
 * - `atomic-floors --kurucz <dir> --barklem <dir> [--ladder E1|E1prime] [--species <key,…>]
 *   --out <json>` takes each stage's largest bake-bin share at its reference column
 *   (`atomicLines.ts`'s `stageBinShares`) at every node of its ladder, and at a hundred times the
 *   column, and records its visibility floor T_v: the highest node at and below which the share
 *   stays under 10⁻³.
 * - `atomic-projection …` reduces each stage's three sample cells (its strongest, a median one and
 *   a band edge) at its lowest, middle and highest E1 nodes on all 30 pressures, for their
 *   g-points, and tries each economy's between-node test on four-node windows (the first round's
 *   projection); `atomic-reprojection … --samples <json,…> --ladder E1prime` reduces the same
 *   cells over the whole ladder on E3's thinned pressures, refined as the merge refines
 *   (decision-r08-t4e-budget §3's re-projection).
 * - `atomic-k-distributions --species <key> … --temperatures t,… [--pressures p,…] [--stride n:k]
 *   --out <partial>` and `atomic-k-distributions-merge --species <key> --out-dir <dir>
 *   --measured-from <K> <partial>…` are `k-distributions` and its merge for a Kurucz source
 *   (`reducePartialCell`, `mergePartials`), the merge printing `refine:` and, on a thinned table,
 *   `refine-pressures:` to run again with.
 * - `atomic-lines … --species <key,…> --out-dir <dir>` writes the line files
 *   (`absorbers/lines/`), `atomic-cell-references` the atoms' line-by-line references, and
 *   `alkali-pairs` `absorbers/alkaliPairs.json` (`alkaliPairs.ts`).
 *
 * Each input is checked against its SHA-256 and none is committed (`decision-r08-licences.md`):
 * Kurucz states no terms; Barklem and Collet's, Barklem et al.'s, Deb and Hibbert's, Stehlé and
 * Hutcheon's and Allard et al.'s tables are CDS's, free for a scientific use.
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

import {
  type AboLine,
  aboText,
  ATOMIC_SHAPE,
  type AtomicLineSet,
  atomicBroadeningRatios,
  atomicLineSet,
  atomicSpectra,
  cellsWithLines,
  ladderFinestStepK,
  ladderTemperatures,
  LADDER_TOP_K,
  parseAboTable,
  NA_D_H2_WIDTHS,
  selfBroadeningSource,
  type VanDerWaalsSource,
  type TemperatureLadder,
  vanDerWaalsCounts,
} from "./atomicKDistributions";
import {
  type AtomicStage,
  barklemName,
  BIN_SHARE_BOUND,
  ELECTRON_RADIUS_CM,
  ionisationLimitEv,
  type KuruczLine,
  lineCellMeansM2,
  parseKuruczLines,
  partitionFunctionAt,
  sahaBalance,
  type SahaData,
  SOLAR_ABUNDANCES,
  stageBinShares,
} from "./atomicLines";
import {
  angstromHalfWidthPerCm,
  type BalmerStarkLine,
  readBalmerStark,
  starkHalfWidthA,
  STEHLE_URL,
} from "./balmerStark";
import {
  ALLARD_TEMPERATURES_K,
  allardWing,
  impactWing,
  PAIR_GRID,
  pairCellMeansM5,
  readAllardTables,
} from "./alkaliPairs";
import {
  BARKLEM_TABLES,
  type CheckedInput,
  KURUCZ_CITATION,
  KURUCZ_URL,
  readSahaData,
} from "./atomicLinesCli";
import {
  mergePartials,
  type PartialCell,
  type PartialFile,
  reducePartialCell,
  refinedTemperatures,
  sortFailures,
  type TableSource,
  withKeptPressures,
} from "./kDistributionsCli";
import {
  BELOW_GRID_CHECK_PA,
  belowGridError,
  CHAIN_BOUND,
  type ChainError,
  chainRowsPa,
  curveRowsPa,
  latticePressurePa,
  PART_LAYOUT_TEXT,
  raisedFirstNodeChecksPa,
  reduceGridCell,
  type ReductionGrid,
  type SaturationCurve,
  STORED_PRESSURES_PA,
  gatedColumnCm2,
  chainColumnCm2,
} from "./kDistributions";
import { cellReference } from "./lineReferences";
import { readCheckedText, SPEED_OF_LIGHT_M_PER_S } from "./molecularLines";

/** A drawn stage's source: its registry key, list and atomic number. */
export interface AtomicSource extends AtomicStage {
  readonly spectrum: string;
  readonly atomicNumber: number;
  readonly list: CheckedInput;
  /** The range's first node before any floor, K: 500 for Na and K, 1,000 for the rest. */
  readonly rangeFromK: number;
  /**
   * A floor ruled before the build (R08.T4.h's stated nones below 4,600 K, drawn from it), K, in
   * place of the ladder's: the stage's `belowK`.
   */
  readonly ruledFloorK?: number;
  /**
   * The table's temperature nodes where they are ruled rather than a ladder's, K: Mg⁺'s and Si⁺'s
   * 4,400, 4,600, 4,800 and 5,000 K (decision-r08-t4e-budget §6), four nodes for the cubic read, the
   * 4,400 K node below their `belowK` shaping the cubic alone.
   */
  readonly tableNodesK?: ReadonlyArray<number>;
  /**
   * The pure condensed element's saturation, at which the between-node test alone also stops
   * (R08.T4.e's residual protocol, item (ii), decision-r08-t4e-unsold §5): K's.
   */
  readonly chainSaturation?: SaturationCurve;
}

/**
 * Liquid potassium's vapour pressure (R08.T4.e's residual protocol, item (ii)): NIST Chemistry
 * WebBook's Antoine fit, log₁₀(p ÷ 1 bar) = 4.45718 − 4,691.58 ÷ (T + 24.195), over 679.4–1,033.0 K,
 * NIST's coefficients from E. F. Fiock and W. H. Rodebush, J. Am. Chem. Soc. 48 (1926) 2522 (read
 * 2026-10-20), used from K's 650 K node, 29 K below the fit (main's ruling: it can't matter at that
 * margin), to its top, with no condensing cap outside them. Alcock, Itkin and Horrigan 1984's fit,
 * the ruling's example, was not open at write.
 */
export const POTASSIUM_SATURATION: SaturationCurve = {
  kind: "antoine",
  a: 4.457_18,
  bK: 4_691.58,
  cK: 24.195,
  fromK: 650,
  criticalK: 1_033,
  source:
    "NIST Chemistry WebBook's Antoine fit for liquid K after E. F. Fiock and W. H. Rodebush, J. Am. Chem. Soc. 48 (1926) 2522, 679.4-1033 K, used from 650 K",
};

/** A stage's source by its key, list file and checksum (the brief's §6.2). */
function stageSource(
  key: string,
  spectrum: string,
  atomicNumber: number,
  name: string,
  sha256: string,
  extra: Partial<
    Pick<AtomicSource, "rangeFromK" | "ruledFloorK" | "tableNodesK" | "chainSaturation">
  > = {},
): AtomicSource {
  const [symbol = "", ion] = key.split("+");
  return {
    key,
    symbol,
    stage: ion === undefined ? 0 : 1,
    spectrum,
    atomicNumber,
    list: { name, sha256 },
    rangeFromK: extra.rangeFromK ?? 1_000,
    ...(extra.ruledFloorK === undefined ? {} : { ruledFloorK: extra.ruledFloorK }),
    ...(extra.tableNodesK === undefined ? {} : { tableNodesK: extra.tableNodesK }),
    ...(extra.chainSaturation === undefined ? {} : { chainSaturation: extra.chainSaturation }),
  };
}

/** Mg⁺'s and Si⁺'s ruled nodes, K (decision-r08-t4e-budget §6): E1's from 4,400 K. */
const FROM_4400_K: ReadonlyArray<number> = [4_400, 4_600, 4_800, 5_000];

/**
 * The stages R08.T4.e draws (the brief's §3.3) and their Kurucz lists, as main fetched them
 * 2026-10-16 and this task again 2026-10-19 (each matching): the atoms and the ions with a line in
 * the range; Mg⁺, Si⁺ and Ni⁺ from their ruled 4,600 K.
 */
export const ATOMIC_SOURCES: ReadonlyArray<AtomicSource> = [
  stageSource(
    "H",
    "H I",
    1,
    "gf0100.all",
    "a29b79785726f7043fce2693b62ef84f9fc52bc371ea0fb999ca12df56cebe02",
  ),
  stageSource(
    "O",
    "O I",
    8,
    "gf0800.all",
    "5f6fce8aa4226b6c827b841d9657ffe07c26c24c7eb49667433ece8461809ba1",
  ),
  stageSource(
    "Na",
    "Na I",
    11,
    "gf1100.all",
    "8216f39c6b3a26655651e00ae2856888d7f5b6287dc5086ab0d43139c13ded9b",
    { rangeFromK: 500 },
  ),
  stageSource(
    "Mg",
    "Mg I",
    12,
    "gf1200.all",
    "861c0ddd69e32525ca83327d7879a355eedf56872b98657cc00c112c8fb4a650",
  ),
  stageSource(
    "Mg+",
    "Mg II",
    12,
    "gf1201.all",
    "aec1d84e33d971399d5fd8069720fc1c71e4c03fd1d1839c0224164b6047c9d6",
    { ruledFloorK: 4_600, tableNodesK: FROM_4400_K },
  ),
  stageSource(
    "Al",
    "Al I",
    13,
    "gf1300.all",
    "c9d02e6de21ecf52e9ef0ddbaa9ecfb1c06a1ece80828695d67bf78509107086",
  ),
  stageSource(
    "Si",
    "Si I",
    14,
    "gf1400.all",
    "d371f7b10005245bc22f84407f2b25624c2cd7a50c61243f3ea9ed5a1e9cd2a2",
  ),
  stageSource(
    "Si+",
    "Si II",
    14,
    "gf1401.all",
    "f29a6989f3171aa32da8728057288216e7ce5750b2e8a7ce33455a40d72ddd16",
    { ruledFloorK: 4_600, tableNodesK: FROM_4400_K },
  ),
  stageSource(
    "K",
    "K I",
    19,
    "gf1900.all",
    "329f431f447ec70e75127e43361acb0f827cf918448faf50f33604945a60f674",
    { rangeFromK: 500, chainSaturation: POTASSIUM_SATURATION },
  ),
  stageSource(
    "Ca",
    "Ca I",
    20,
    "gf2000.all",
    "85410bce338bba778f63c046c2adaa89f709e95be0fc435d5038c454b53dd3c2",
  ),
  stageSource(
    "Ca+",
    "Ca II",
    20,
    "gf2001.all",
    "336bbb97776496c08fbecb97985ec3c251f94d944df42c4bbaf45e1c87438a0d",
  ),
  stageSource(
    "Ti",
    "Ti I",
    22,
    "gf2200.all",
    "6e0794987a826a9949ed933a89ae1ddebc3e4ffe167c49d25939a311c8c7b2ef",
  ),
  stageSource(
    "Ti+",
    "Ti II",
    22,
    "gf2201.all",
    "ce91d4c5423b019a94ef215bcf91a51307f4a0ea0d6696153c9b4e627ab1fa42",
  ),
  stageSource(
    "V",
    "V I",
    23,
    "gf2300.all",
    "300da04a03b4bca4800252e83590aba67447b2164bec48c17db17d0a27f593c8",
  ),
  stageSource(
    "V+",
    "V II",
    23,
    "gf2301.all",
    "e722d0b468e3c30db66d98321d5af534a7782b2a0faf2e75eed608f160802175",
  ),
  stageSource(
    "Cr",
    "Cr I",
    24,
    "gf2400.all",
    "b471d0629a336b24e1eeb5460f46f919f0e9408c5c01518f8c0839eda7cc2784",
  ),
  stageSource(
    "Cr+",
    "Cr II",
    24,
    "gf2401.all",
    "166b4839fac2a14b6485d16a969aaee2a839027a74a7ce8a6ba77bb089ab31b3",
  ),
  stageSource(
    "Mn",
    "Mn I",
    25,
    "gf2500.all",
    "850c89ac85f4491074ee0b86f0d6227f7e3275b82c195b80d5991425fcfa1bf0",
  ),
  stageSource(
    "Mn+",
    "Mn II",
    25,
    "gf2501.all",
    "04a5e5b0b3d96b62c2b1a65b3309a8892d87a67e08e7031529d9dd9ac933928f",
  ),
  stageSource(
    "Fe",
    "Fe I",
    26,
    "gf2600.all",
    "eba61c86de416a9622b697ad6fc2b418178dd387fa3e6711656e2d4c57f7ae76",
  ),
  stageSource(
    "Fe+",
    "Fe II",
    26,
    "gf2601.all",
    "2f1a8b1f27692a13e487a089084160c2ee99d5d44adb4686adbe33b418795ac4",
  ),
  stageSource(
    "Ni",
    "Ni I",
    28,
    "gf2800.all",
    "52d88aebef5098c599574848a39003a06ee918446825c8d5b658ac3c4e1fbf83",
  ),
  stageSource(
    "Ni+",
    "Ni II",
    28,
    "gf2801.all",
    "60993b3cad578d31b2f454899675bbcd87ccb7421b6fca8786bb3550a401b963",
    { ruledFloorK: 4_600 },
  ),
  stageSource(
    "Zn",
    "Zn I",
    30,
    "gf3000.all",
    "6fbf50f9bae5ef291e1243ed2c5aa06cbc7576430fec987e8a61ca5b190cfedd",
  ),
];

/** Barklem, Piskunov and O'Mara 2000's ABO table, fetched 2026-10-19 (its SHA-256 recorded then). */
export const ABO_TABLE: CheckedInput = {
  name: "table1.dat.gz",
  sha256: "5934b7306dc13d136b054e8c06d705ecba34d5da6ec71e0f065bdd70e3cdcfbf",
};

/**
 * The ABO table's lines ({@link parseAboTable}) from its gzipped file, its SHA-256 checked on the
 * bytes parsed.
 *
 * @throws Error where the file's SHA-256 is not {@link ABO_TABLE}'s.
 */
export function readAboTable(path: string): ReadonlyArray<AboLine> {
  const bytes = readFileSync(path);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== ABO_TABLE.sha256) {
    throw new Error(`${path}: SHA-256 ${digest}, expected ${ABO_TABLE.sha256}`);
  }
  return parseAboTable(aboText(bytes));
}

/** Deb and Hibbert 2011's tables 5, 6 and 9 (CDS J/A+A/536/A74), as main fetched them 2026-10-16. */
export const DEB_HIBBERT_TABLES = {
  levels: {
    name: "table5.dat",
    sha256: "f095e43272369f2e58e2107c3ed6dfa349d3014605a40856f260cfd35041d4ed",
  },
  nist: {
    name: "table6.dat",
    sha256: "5eae72c8704a34b8df3fc4d7713b578b6f9ae97dd6d0a83ed1f554193d912dc3",
  },
  others: {
    name: "table9.dat",
    sha256: "8e821169dff709d29ac0093c11b8ca5f889a23b52d28e802ba20c2723c381f7c",
  },
} as const satisfies Readonly<Record<string, CheckedInput>>;

/**
 * [Fe II]'s forbidden lines (R08.T4.c's follow-up 5): Deb and Hibbert 2011's E2 + M1 A-values
 * (A&A 536, A74, tables 6 and 9, the sums), between table 5's levels, gf = g_u A λ² ÷ (8π² r_e c),
 * on the vacuum wavelength of the levels' difference, as Kurucz lines with no damping constants.
 */
export function forbiddenIronIonLines(
  levelsText: string,
  ...aValueTexts: ReadonlyArray<string>
): ReadonlyArray<KuruczLine> {
  const levels = new Map<
    number,
    { readonly perCm: number; readonly j: number; readonly label: string }
  >();
  for (const row of levelsText.split(/\r?\n/u)) {
    if (row.trim().length === 0) {
      continue;
    }
    levels.set(Number(row.slice(0, 3)), {
      j: Number(row.slice(28, 31)),
      perCm: Number(row.slice(32, 42)),
      label: `${row.slice(4, 21).trim()} ${row.slice(22, 27).trim()}`,
    });
  }
  const lines: KuruczLine[] = [];
  for (const text of aValueTexts) {
    for (const row of text.split(/\r?\n/u)) {
      if (row.trim().length === 0) {
        continue;
      }
      const lower = levels.get(Number(row.slice(0, 3)));
      const upper = levels.get(Number(row.slice(4, 7)));
      const aPerS = Number(row.slice(39, 48));
      if (lower === undefined || upper === undefined || !(aPerS > 0)) {
        continue;
      }
      const wavenumber = upper.perCm - lower.perCm;
      const vacuumM = 1e-2 / wavenumber;
      const gf =
        ((2 * upper.j + 1) * aPerS * vacuumM * vacuumM) /
        (8 * Math.PI ** 2 * ELECTRON_RADIUS_CM * 1e-2 * SPEED_OF_LIGHT_M_PER_S);
      lines.push({
        listedNm: 1e7 / wavenumber,
        vacuumNm: 1e7 / wavenumber,
        logGf: Math.log10(gf),
        lowerPerCm: lower.perCm,
        upperPerCm: upper.perCm,
        lowerJ: lower.j,
        upperJ: upper.j,
        lowerLabel: lower.label,
        upperLabel: upper.label,
        reference: "DH11",
        logGammaRadPerS: 0,
        logGammaStarkPerElectron: 0,
        logGammaVanDerWaalsPerHydrogen: 0,
      });
    }
  }
  return lines;
}

/** The inputs a stage's line set reads. */
export interface AtomicInputs {
  readonly kuruczDir: string;
  readonly data: SahaData;
  readonly abo: ReadonlyArray<AboLine>;
  /** Deb and Hibbert's tables' directory, for Fe⁺. */
  readonly debHibbertDir: string | undefined;
  /** Stehlé and Hutcheon's Balmer profiles' directory (`balmerStark.ts`), for H. */
  readonly stehleDir?: string | undefined;
}

/**
 * Na I's line set with Na D's impact widths by H₂ from Allard et al. 2019's tables
 * (`atomicKDistributions.ts`'s {@link NA_D_H2_WIDTHS}): D2, the ground state to 3p ²P₃/₂ (589.16 nm
 * vacuum), and D1, to ²P₁/₂ (589.76 nm), each keeping its ABO width for H, its widths by He and
 * the air gases scaled from Allard's by Unsöld's rule.
 *
 * @throws Error where either line is missing or holds no ABO width.
 */
export function withSodiumH2Widths(set: AtomicLineSet): AtomicLineSet {
  let found = 0;
  const vanDerWaals = set.vanDerWaals.map((source, i): VanDerWaalsSource => {
    const line = set.lines[i];
    const d = line?.lowerPerCm === 0 && line.lowerJ === 0.5 ? line.upperJ : undefined;
    if (d !== 1.5 && d !== 0.5) {
      return source;
    }
    if (source.kind !== "abo") {
      throw new Error("Na D takes its ABO width for H, and the table holds none");
    }
    found += 1;
    return {
      kind: "allardH2",
      hwhmPerCm: d === 1.5 ? NA_D_H2_WIDTHS.d2HwhmPerCm : NA_D_H2_WIDTHS.d1HwhmPerCm,
      otherwise: source,
    };
  });
  if (found !== 2) {
    throw new Error(`Na I's list holds ${found} D lines from the ground state, not 2`);
  }
  return { ...set, vanDerWaals };
}

/**
 * The Rydberg constant for hydrogen, cm⁻¹: R_∞ ÷ (1 + m_e ÷ m_p), CODATA 2018's R_∞
 * 109,737.315 681 60 cm⁻¹ and m_p ÷ m_e 1,836.152 673 43.
 */
const RYDBERG_HYDROGEN_PER_CM = 109_737.315_681_6 / (1 + 1 / 1_836.152_673_43);

/** A hydrogen level's principal quantum number from its energy above the ground state, cm⁻¹. */
export function hydrogenLevelN(energyPerCm: number): number {
  return Math.round(1 / Math.sqrt(1 - energyPerCm / RYDBERG_HYDROGEN_PER_CM));
}

/**
 * H I's line set: each Balmer line (from n = 2) broadened by H atoms by Barklem, Piskunov and
 * O'Mara 2000b's self-broadening ({@link selfBroadeningSource}), by H₂ and He by Unsöld's as
 * before (decision-r08-t4e-budget §8), and by Stark broadening at Stehlé and Hutcheon's half width
 * for its upper level (H10's above it, whose lines' 25 cm⁻¹ shapes reach no cell of the range) at
 * the node's electron density, taken from the solar-abundance Saha balance at the node's
 * temperature and pressure (`atomicLines.ts`'s {@link sahaBalance}): an `estimated` n_e, since it
 * is not an axis of the table.
 */
export function hydrogenLineSet(
  base: AtomicLineSet,
  data: SahaData,
  stark: ReadonlyArray<BalmerStarkLine>,
): AtomicLineSet {
  const balmer = base.lines.map((line) =>
    hydrogenLevelN(line.lowerPerCm) === 2 ? hydrogenLevelN(line.upperPerCm) : undefined,
  );
  const vanDerWaals = base.vanDerWaals.map((source, i) => {
    const upper = balmer[i];
    return upper === undefined ? source : selfBroadeningSource(upper);
  });
  const starkOf = balmer.map((upper) =>
    upper === undefined ? undefined : (stark.find((s) => s.upperN === upper) ?? stark.at(-1)),
  );
  const electrons = new Map<string, number>();
  const electronsPerCm3 = (temperatureK: number, pressurePa: number): number => {
    const key = `${temperatureK}:${pressurePa}`;
    let n = electrons.get(key);
    if (n === undefined) {
      n = sahaBalance(data, temperatureK, pressurePa).electronsPerM3 / 1e6;
      electrons.set(key, n);
    }
    return n;
  };
  return {
    ...base,
    vanDerWaals,
    starkHalfWidthPerCm: (index, temperatureK, pressurePa) => {
      const line = starkOf[index];
      return line === undefined
        ? 0
        : angstromHalfWidthPerCm(
            starkHalfWidthA(line, electronsPerCm3(temperatureK, pressurePa), temperatureK),
            line.wavelengthA,
          );
    },
  };
}

/** A stage's mass, Da: its element's standard atomic weight. */
function massOf(stage: AtomicStage): number {
  const element = SOLAR_ABUNDANCES.find((e) => e.symbol === stage.symbol);
  if (element === undefined) {
    throw new Error(`no atomic weight for ${stage.key}`);
  }
  return element.atomicWeightDa;
}

/** A drawn stage's line set from its checked list (and Fe⁺'s forbidden lines). */
export function readAtomicLineSet(source: AtomicSource, inputs: AtomicInputs): AtomicLineSet {
  const listed = parseKuruczLines(
    readCheckedText(join(inputs.kuruczDir, source.list.name), source.list.sha256),
  );
  let extra: ReadonlyArray<KuruczLine> = [];
  if (source.key === "Fe+") {
    const dir = inputs.debHibbertDir;
    if (dir === undefined) {
      throw new Error("Fe+ takes Deb and Hibbert's [Fe II] lines: give --deb-hibbert");
    }
    const read = (f: CheckedInput): string => readCheckedText(join(dir, f.name), f.sha256);
    extra = forbiddenIronIonLines(
      read(DEB_HIBBERT_TABLES.levels),
      read(DEB_HIBBERT_TABLES.nist),
      read(DEB_HIBBERT_TABLES.others),
    );
  }
  const set = atomicLineSet(
    source,
    source.atomicNumber,
    massOf(source),
    ionisationLimitEv(inputs.data, source),
    [...listed, ...extra],
    inputs.abo,
    (t) => partitionFunctionAt(inputs.data.partitionFunctions, barklemName(source), t),
  );
  if (source.key === "Na") {
    return withSodiumH2Widths(set);
  }
  if (source.key !== "H") {
    return set;
  }
  if (inputs.stehleDir === undefined) {
    throw new Error("H takes Stehle and Hutcheon's Balmer Stark widths: give --stehle");
  }
  return hydrogenLineSet(set, inputs.data, readBalmerStark(inputs.stehleDir));
}

/** A stage's shares on the ladder: at each node, its largest at the reference column and at 100×. */
export interface LadderShares {
  readonly key: string;
  readonly temperaturesK: ReadonlyArray<number>;
  readonly shares: ReadonlyArray<number>;
  readonly sharesAt100: ReadonlyArray<number>;
  /** The visibility floor T_v, K, or `undefined` for none (the share reaches 10⁻³ at the range's first node). */
  readonly floorK: number | undefined;
  /** Whether the share stays under 10⁻³ at every node: a stated none to 5,000 K. */
  readonly statedNone: boolean;
  /** The first node at which the share at 100 times the column reaches 10⁻³, K, or `undefined`. */
  readonly reachesAt100K: number | undefined;
}

/**
 * A stage's visibility floor on its ladder shares (decision-r08-t4ef-storage §3): the highest node
 * at and below which every share stays under {@link BIN_SHARE_BOUND}, or `undefined` where the
 * first node's share reaches it.
 */
export function visibilityFloorK(
  temperaturesK: ReadonlyArray<number>,
  shares: ReadonlyArray<number>,
): number | undefined {
  let floor: number | undefined;
  for (const [i, t] of temperaturesK.entries()) {
    if (!((shares[i] ?? Infinity) < BIN_SHARE_BOUND)) {
      break;
    }
    floor = t;
  }
  return floor;
}

/** A stage's ladder shares ({@link LadderShares}) from its whole list's lines, as T4.h's sums take them. */
export function ladderShares(
  data: SahaData,
  source: AtomicSource,
  lines: ReadonlyArray<KuruczLine>,
  ladder: TemperatureLadder = "E1",
): LadderShares {
  const temperaturesK = ladderTemperatures(source.rangeFromK, LADDER_TOP_K, ladder);
  const largest = (t: number, factor: number): number =>
    Math.max(...stageBinShares(data, source, lines, t, factor).binShares);
  const shares = temperaturesK.map((t) => largest(t, 1));
  const sharesAt100 = temperaturesK.map((t) => largest(t, 100));
  const floorK = visibilityFloorK(temperaturesK, shares);
  const reaches = temperaturesK.findIndex((_, i) => (sharesAt100[i] ?? 0) >= BIN_SHARE_BOUND);
  return {
    key: source.key,
    temperaturesK,
    shares,
    sharesAt100,
    floorK,
    statedNone: shares.every((s) => s < BIN_SHARE_BOUND),
    reachesAt100K: reaches < 0 ? undefined : temperaturesK[reaches],
  };
}

/** Command-line options shared by the subcommands. */
const COMMON_OPTIONS = {
  kurucz: { type: "string" },
  barklem: { type: "string" },
  abo: { type: "string" },
  "deb-hibbert": { type: "string" },
  stehle: { type: "string" },
  floors: { type: "string" },
  ladder: { type: "string" },
  samples: { type: "string" },
  species: { type: "string" },
  stride: { type: "string" },
  out: { type: "string" },
} as const;

/** The sources a run takes: those named, every stride-th from the offset. */
function sourcesOf(species: string | undefined, stride: string | undefined): AtomicSource[] {
  const wanted = species?.split(",");
  const [n, k] = (stride ?? "1:0").split(":").map(Number);
  return ATOMIC_SOURCES.filter((s) => wanted === undefined || wanted.includes(s.key)).filter(
    (_, i) => i % (n ?? 1) === (k ?? 0),
  );
}

/** Whether a value read from a floors record is a {@link LadderShares}: our own writing. */
function isLadderShares(value: unknown): value is LadderShares {
  return (
    typeof value === "object" &&
    value !== null &&
    "temperaturesK" in value &&
    "shares" in value &&
    "statedNone" in value
  );
}

/** An `atomic-floors` record, by stage key, checked. */
export function readFloors(path: string): Readonly<Record<string, LadderShares>> {
  const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error(`${path} is not a floors record`);
  }
  const floors: Record<string, LadderShares> = {};
  for (const [key, value] of Object.entries(parsed)) {
    if (!isLadderShares(value)) {
      throw new Error(`${path}'s ${key} is not a stage's ladder shares`);
    }
    floors[key] = value;
  }
  return floors;
}

/** A ladder by its command-line name: E1 unless E1prime is given. */
export function ladderOf(name: string | undefined): TemperatureLadder {
  if (name === undefined || name === "E1") {
    return "E1";
  }
  if (name === "E1prime") {
    return "E1prime";
  }
  throw new Error(`--ladder is E1 or E1prime, not ${name}`);
}

/** `atomic-floors`: records each drawn stage's ladder shares and visibility floor. */
export function runAtomicFloors(rest: readonly string[]): void {
  const { values } = parseArgs({ args: [...rest], options: COMMON_OPTIONS });
  if (values.kurucz === undefined || values.barklem === undefined || values.out === undefined) {
    throw new Error(
      "usage: atomic-floors --kurucz <dir> --barklem <dir> [--species a,b] --out <json>",
    );
  }
  const data = readSahaData(values.barklem);
  const ladder = ladderOf(values.ladder);
  const record: Record<string, LadderShares> = {};
  for (const s of sourcesOf(values.species, values.stride)) {
    const lines = parseKuruczLines(
      readCheckedText(join(values.kurucz, s.list.name), s.list.sha256),
    );
    const shares = ladderShares(data, s, lines, ladder);
    record[s.key] = shares;
    process.stdout.write(
      `${s.key}: floor ${shares.floorK ?? "none"}${shares.statedNone ? " (stated none to 5000 K)" : ""}, 1e-3 at 100x from ${shares.reachesAt100K ?? "none"} K; ${shares.temperaturesK.map((t, i) => `${t}:${(shares.shares[i] ?? 0).toExponential(1)}`).join(" ")}\n`,
    );
  }
  writeFileSync(values.out, `${JSON.stringify(record, null, 1)}\n`);
}

/** The first node of a stage's table: its ruled floor, its visibility floor, or its range's start. */
export function firstNodeK(source: AtomicSource, floors: LadderShares | undefined): number {
  return source.ruledFloorK ?? floors?.floorK ?? source.rangeFromK;
}

/**
 * A stage's table nodes before refinement, K: its ruled nodes (Mg⁺'s and Si⁺'s), or its ladder's
 * from its first node ({@link firstNodeK}) to 5,000 K.
 */
export function tableNodesOf(
  source: AtomicSource,
  floors: LadderShares | undefined,
  ladder: TemperatureLadder,
): ReadonlyArray<number> {
  return source.tableNodesK ?? ladderTemperatures(firstNodeK(source, floors), LADDER_TOP_K, ladder);
}

/** E3's thinned pressures: alternate nodes below 1 MPa dropped, 20 of the 30. */
export const THINNED_PRESSURES_PA: ReadonlyArray<number> = STORED_PRESSURES_PA.filter(
  (p, i) => p >= 1e6 * (1 - 1e-9) || i % 2 === 0,
);

/**
 * A further thinning the projection tries for the record only, not a ruled economy: one node a
 * decade below 1 MPa, 15 of the 30, the rows still the standard 30.
 */
export const DECADE_PRESSURES_PA: ReadonlyArray<number> = STORED_PRESSURES_PA.filter(
  (p, i) => p >= 1e6 * (1 - 1e-9) || i % 4 === 0,
);

/** A stage's three sample cells: its strongest, a median one and a band edge, nm. */
export function sampleCells(
  set: AtomicLineSet,
  cells: ReadonlyArray<number>,
  temperatureK: number,
): {
  readonly strongest: number;
  readonly median: number;
  readonly edge: number;
} {
  const q = set.partitionFunction(temperatureK);
  const means = lineCellMeansM2(set.lines, temperatureK, q, set.limitEv).cellMeansM2;
  const mean = (c: number): number => means[c - 380] ?? 0;
  const ranked = cells.toSorted((a, b) => mean(b) - mean(a));
  const strongest = ranked[0] ?? 380;
  const median = ranked[Math.floor(ranked.length / 2)] ?? strongest;
  // The edge of the strongest cell's feature: its neighbour with the lesser mean, holding its wing.
  const neighbours = [strongest - 1, strongest + 1].filter((c) => cells.includes(c));
  const edge = neighbours.toSorted((a, b) => mean(a) - mean(b))[0] ?? median;
  return { strongest, median, edge };
}

/** One sample reduction's figures. */
interface SampleRun {
  readonly temperaturesK: ReadonlyArray<number>;
  readonly pressures: number;
  readonly g: number;
  readonly seconds: number;
  readonly worstNode: number;
  readonly worstChain: ChainError;
  readonly failures: number;
  readonly failingTemperaturesK: ReadonlyArray<number>;
  readonly failingPressuresPa: ReadonlyArray<number>;
}

/** One window of the E1 projection's samples: each economy's run and E4's raised node. */
interface ProjectionWindow {
  readonly e1: SampleRun | undefined;
  readonly e3: SampleRun | undefined;
  readonly e3Passes: number | undefined;
  readonly e4: { readonly firstPa: number } | undefined;
  readonly decade: SampleRun | undefined;
  readonly belowGrid1Pa: number | undefined;
}

/** A reduction of a sample cell on a grid, timed, with its chain test's figures. */
function sampleRun(
  set: AtomicLineSet,
  cellNm: number,
  grid: ReductionGrid,
):
  | { readonly run: SampleRun; readonly cell: NonNullable<ReturnType<typeof reduceGridCell>> }
  | undefined {
  const spectra = atomicSpectra(set);
  const started = performance.now();
  const saturation = atomicSourceOf(set.stage.key)?.chainSaturation;
  const cap = (t: number, p: number): number => gatedColumnCm2(undefined, cellNm, t, p, set.massDa);
  const chainCap = (t: number, p: number): number =>
    chainColumnCm2(saturation, cellNm, t, p, set.massDa, undefined);
  const cell = reduceGridCell(cellNm, spectra, grid, ATOMIC_SHAPE, cap, chainCap);
  if (cell === undefined) {
    return undefined;
  }
  return {
    cell,
    run: {
      temperaturesK: grid.temperaturesK,
      pressures: grid.pressuresPa.length,
      g: cell.weights.length,
      seconds: (performance.now() - started) / 1000,
      worstNode: cell.worstNodeError,
      worstChain: cell.worstChain,
      failures: cell.failures.length,
      failingTemperaturesK: [...new Set(cell.failures.map((f) => f.temperatureK))].toSorted(
        (a, b) => a - b,
      ),
      failingPressuresPa: [...new Set(cell.failures.map((f) => f.pressurePa))].toSorted(
        (a, b) => a - b,
      ),
    },
  };
}

/**
 * A sample cell's window reduced with E1's refinement, as the merge would ask for it: each interval
 * whose midpoint fails the chain test halved, at most twice (to a quarter of its ladder step), the
 * last run's figures and nodes returned; failures left are the intervals that would be `estimated`.
 */
function refinedRun(
  set: AtomicLineSet,
  cellNm: number,
  grid: ReductionGrid,
):
  | {
      readonly run: SampleRun;
      readonly cell: NonNullable<ReturnType<typeof reduceGridCell>>;
      readonly grid: ReductionGrid;
      readonly passes: number;
    }
  | undefined {
  let current = grid;
  let result = sampleRun(set, cellNm, current);
  let passes = 1;
  while (result !== undefined && passes < 3) {
    const nodes = current.temperaturesK;
    const failing = result.cell.failures
      .map((f) =>
        nodes.findIndex(
          (lo, i) => lo < f.temperatureK && f.temperatureK < (nodes[i + 1] ?? -Infinity),
        ),
      )
      .filter(
        (i) =>
          i >= 0 && (nodes[i + 1] ?? 0) - (nodes[i] ?? 0) > ladderFinestStepK(nodes[i] ?? 0) + 1e-9,
      );
    if (failing.length === 0) {
      break;
    }
    const halves = new Set(failing);
    current = atomicGrid(
      nodes.flatMap((t, i) => (halves.has(i) ? [t, 0.5 * (t + (nodes[i + 1] ?? t))] : [t])),
      current.pressuresPa,
    );
    result = sampleRun(set, cellNm, current);
    passes += 1;
  }
  return result === undefined ? undefined : { ...result, grid: current, passes };
}

/** The highest thinned first node whose hold passes the generalised 1 Pa check, and each tried. */
function raisedFirstNode(
  set: AtomicLineSet,
  cellNm: number,
  grid: ReductionGrid,
  cell: NonNullable<ReturnType<typeof reduceGridCell>>,
): { readonly firstPa: number; readonly errors: ReadonlyArray<readonly [number, number]> } {
  const spectra = atomicSpectra(set);
  const errors: (readonly [number, number])[] = [];
  let firstPa = grid.pressuresPa[0] ?? 10;
  for (const [k, p] of grid.pressuresPa.entries()) {
    if (p > 1e6) {
      break;
    }
    const error = belowGridError(
      cellNm,
      spectra,
      grid,
      ATOMIC_SHAPE,
      cell,
      set.massDa,
      raisedFirstNodeChecksPa(p),
      k,
    );
    errors.push([p, error]);
    // Each node's check takes every lower one's, so the first to fail ends the search.
    if (!(error <= CHAIN_BOUND)) {
      break;
    }
    firstPa = p;
  }
  return { firstPa, errors };
}

/** Four consecutive nodes of a ladder about a place in it: its first four, middle four or last four. */
function windowOf(nodes: ReadonlyArray<number>, where: "low" | "middle" | "high"): number[] {
  const n = nodes.length;
  if (n <= 4) {
    return [...nodes];
  }
  const start = where === "low" ? 0 : where === "high" ? n - 4 : Math.floor(n / 2) - 2;
  return nodes.slice(start, start + 4);
}

/** A number written to a given count of decimals, its trailing zeros dropped. */
function decimals(value: number, places: number): string {
  return String(Number(value.toFixed(places)));
}

/** A line file's rows, each a line's reduced values ({@link lineFileText}). */
function lineRows(set: AtomicLineSet): string[] {
  return set.lines.map((line, i) => {
    const vdw = set.vanDerWaals[i];
    const own = vdw?.kind === "allardH2" ? vdw.otherwise : vdw;
    const abo =
      own?.kind === "abo"
        ? `[${decimals(own.logGammaPerHydrogen, 3)}, ${decimals(own.exponent, 3)}]`
        : "null";
    return (
      `    [${decimals(line.vacuumNm, 5)}, ${decimals(line.logGf, 3)}, ${decimals(line.lowerPerCm, 3)}, ` +
      `${line.lowerJ}, ${decimals(line.logGammaRadPerS, 2)}, ${decimals(line.logGammaStarkPerElectron, 2)}, ` +
      `${decimals(line.logGammaVanDerWaalsPerHydrogen, 2)}, ${abo}, ${JSON.stringify(line.reference)}]`
    );
  });
}

/** The columns of a line file's rows, in order. */
export const LINE_FILE_COLUMNS = [
  "vacuumNm",
  "logGf",
  "lowerPerCm",
  "lowerJ",
  "logGammaRadPerS",
  "logGammaStarkPerElectron",
  "logGammaVanDerWaalsPerHydrogen",
  "abo",
  "reference",
] as const;

/** The largest a line file may be before it is split into parts, bytes: under the 500 kB hook. */
export const LINE_FILE_BYTES = 480_000;

/**
 * A stage's reduced line list as `absorbers/lines/<key>.json` holds it (the brief's §3.1): its
 * source's file, SHA-256, URL, fetch date, citation and selection rule, and each line's vacuum
 * wavelength, log gf, lower level's energy and J, Kurucz's three damping constants, its ABO width
 * where Barklem et al.'s table holds it, and Kurucz's reference code, one line a row. Where the
 * file would pass {@link LINE_FILE_BYTES} it is split into parts `<key>-<n>.json`, each with the
 * header and its share of the lines, in wavenumber order (a recorded deviation from "one file a
 * spectrum").
 *
 * @returns Each file's name and text.
 */
export function lineFiles(
  source: AtomicSource,
  set: AtomicLineSet,
): ReadonlyArray<{ readonly file: string; readonly text: string }> {
  const rows = lineRows(set);
  const header = (part: string): string =>
    `{\n  "species": ${JSON.stringify(source.key)},\n  "spectrum": ${JSON.stringify(source.spectrum)},\n` +
    `  "source": ${JSON.stringify(`R. L. Kurucz's line list ${source.list.name}, SHA-256 ${source.list.sha256}`)},\n` +
    `  "url": ${JSON.stringify(`${KURUCZ_URL}${source.list.name}`)},\n` +
    `  "fetched": "2026-10-16 by main and 2026-10-19 by R08.T4.e (each matching)",\n` +
    `  "citation": ${JSON.stringify(`${KURUCZ_CITATION}; the ABO widths from P. S. Barklem, N. Piskunov and B. J. O'Mara, Astron. Astrophys. Suppl. Ser. 142 (2000) 467 (CDS J/A+AS/142/467, table1.dat.gz, SHA-256 ${ABO_TABLE.sha256})`)},\n` +
    `  "selection": ${JSON.stringify(`Every line of the list over 360-780 nm by vacuum wavelength (Morton 2000's air-to-vacuum conversion above 200 nm) from a level below the stage's ionisation limit, ${set.limitEv.toFixed(4)} eV (Barklem and Collet 2016's table 4): the lines the k-distribution table was reduced from (rendering plan R08, R08.T4.e), in ascending wavenumber.`)},\n` +
    `  "units": "vacuumNm nm; lowerPerCm cm-1; Kurucz's damping constants, each 0 where he gives none: logGammaRadPerS log10 s-1, logGammaStarkPerElectron log10 rad s-1 cm3, logGammaVanDerWaalsPerHydrogen log10 rad s-1 cm3 at 10000 K; abo [log10 FWHM per H atom at 10000 K, rad s-1 cm3, temperature exponent] or null",\n` +
    part +
    `  "columns": ${JSON.stringify(LINE_FILE_COLUMNS)},\n`;
  const text = (part: string, slice: ReadonlyArray<string>): string =>
    `${header(part)}  "lines": [\n${slice.join(",\n")}\n  ]\n}\n`;
  const whole = text("", rows);
  if (Buffer.byteLength(whole) <= LINE_FILE_BYTES) {
    return [{ file: `${source.key}.json`, text: whole }];
  }
  // Each part as full as it may be, the part's header sized for up to 99 parts.
  const room = LINE_FILE_BYTES - Buffer.byteLength(text(`  "part": [99, 99],\n`, []));
  const slices: string[][] = [[]];
  let size = 0;
  for (const row of rows) {
    const bytes = Buffer.byteLength(row) + 2;
    const current = slices.at(-1) ?? [];
    if (current.length > 0 && size + bytes > room) {
      slices.push([row]);
      size = bytes;
    } else {
      current.push(row);
      size += bytes;
    }
  }
  return slices.map((slice, k) => ({
    file: `${source.key}-${k + 1}.json`,
    text: text(`  "part": [${k + 1}, ${slices.length}],\n`, slice),
  }));
}

/** A stage's whole line list as one text, for the projection's byte count ({@link lineFiles}). */
export function lineFileText(source: AtomicSource, set: AtomicLineSet): string {
  return lineFiles(source, set)
    .map((f) => f.text)
    .join("");
}

/** `atomic-lines`: writes the drawn stages' line files into a directory ({@link lineFiles}). */
export function runAtomicLines(rest: readonly string[]): void {
  const { values } = parseArgs({ args: [...rest], options: REDUCTION_OPTIONS });
  const outDir = values["out-dir"];
  if (
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values.abo === undefined ||
    values.species === undefined ||
    outDir === undefined
  ) {
    throw new Error(
      "usage: atomic-lines --kurucz <dir> --barklem <dir> --abo <table1.dat.gz> [--stehle <dir>] --species <key,...> --out-dir <dir>",
    );
  }
  const inputs: AtomicInputs = {
    kuruczDir: values.kurucz,
    data: readSahaData(values.barklem),
    abo: readAboTable(values.abo),
    debHibbertDir: values["deb-hibbert"],
    stehleDir: values.stehle,
  };
  for (const key of values.species.split(",")) {
    const source = atomicSourceOf(key);
    if (source === undefined) {
      throw new Error(`no drawn stage ${key}`);
    }
    for (const f of lineFiles(source, readAtomicLineSet(source, inputs))) {
      writeFileSync(join(outDir, f.file), f.text);
      process.stdout.write(`${f.file}: ${Buffer.byteLength(f.text)} bytes\n`);
    }
  }
}

/** A table's bytes on T4.d's format: per cell 20 + 8G + 2 V T P G, a part's 16-byte header aside. */
export function tableBytes(
  cells: number,
  g: number,
  temperatures: number,
  pressures: number,
): number {
  return cells * (20 + 8 * g + 2 * temperatures * pressures * g);
}

/** `atomic-projection`: the samples' g-points, costs and economies' tests, as a JSON record. */
export function runAtomicProjection(rest: readonly string[]): void {
  const { values } = parseArgs({ args: [...rest], options: COMMON_OPTIONS });
  if (
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values.abo === undefined ||
    values.floors === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: atomic-projection --kurucz <dir> --barklem <dir> --abo <table1.dat.gz> --deb-hibbert <dir> --floors <json> [--species a,b] [--stride n:k] --out <json>",
    );
  }
  const data = readSahaData(values.barklem);
  const inputs: AtomicInputs = {
    kuruczDir: values.kurucz,
    data,
    abo: readAboTable(values.abo),
    debHibbertDir: values["deb-hibbert"],
    stehleDir: values.stehle,
  };
  const floors = readFloors(values.floors);
  const record: Record<string, unknown> = {};
  for (const s of sourcesOf(values.species, values.stride)) {
    const ladder = floors[s.key];
    if (ladder?.statedNone === true && s.ruledFloorK === undefined) {
      record[s.key] = { statedNone: true };
      process.stdout.write(`${s.key}: stated none to ${LADDER_TOP_K} K, no table\n`);
      continue;
    }
    const set = readAtomicLineSet(s, inputs);
    const cells = cellsWithLines(set);
    const nodes = ladderTemperatures(firstNodeK(s, ladder));
    const middleK = nodes[Math.floor(nodes.length / 2)] ?? LADDER_TOP_K;
    const samples = sampleCells(set, cells, middleK);
    const runs: Record<string, unknown> = {};
    for (const [role, cellNm] of Object.entries(samples)) {
      const gRun = sampleRun(set, cellNm, {
        temperaturesK: [...new Set([nodes[0] ?? 0, middleK, nodes.at(-1) ?? 0])],
        pressuresPa: STORED_PRESSURES_PA,
      });
      const windows: Record<string, ProjectionWindow> = {};
      // A ruled floor's table may start a node lower (the brief's §3.5): its window from there too.
      const lowerStart =
        s.ruledFloorK === undefined ? [] : ladderTemperatures(s.ruledFloorK - 200).slice(0, 4);
      const places: ReadonlyArray<readonly [string, ReadonlyArray<number>]> = [
        ["low", windowOf(nodes, "low")],
        ["middle", windowOf(nodes, "middle")],
        ["high", windowOf(nodes, "high")],
        ...(lowerStart.length > 0 ? [["fromLowerNode", lowerStart] as const] : []),
      ];
      for (const [where, temperaturesK] of places) {
        const e1 = sampleRun(set, cellNm, { temperaturesK, pressuresPa: STORED_PRESSURES_PA });
        const thinned: ReductionGrid = {
          temperaturesK,
          pressuresPa: THINNED_PRESSURES_PA,
          rowsPa: curveRowsPa(THINNED_PRESSURES_PA),
        };
        const e3 = refinedRun(set, cellNm, thinned);
        const e4 = e3 === undefined ? undefined : raisedFirstNode(set, cellNm, e3.grid, e3.cell);
        const decade = sampleRun(set, cellNm, {
          temperaturesK,
          pressuresPa: DECADE_PRESSURES_PA,
          rowsPa: curveRowsPa(DECADE_PRESSURES_PA),
        });
        const onePa =
          e1 === undefined
            ? undefined
            : belowGridError(
                cellNm,
                atomicSpectra(set),
                { temperaturesK, pressuresPa: STORED_PRESSURES_PA },
                ATOMIC_SHAPE,
                e1.cell,
                set.massDa,
                [BELOW_GRID_CHECK_PA],
              );
        windows[where] = {
          e1: e1?.run,
          e3: e3?.run,
          e3Passes: e3?.passes,
          e4,
          decade: decade?.run,
          belowGrid1Pa: onePa,
        };
      }
      runs[role] = { cellNm, g: gRun?.run, windows };
      process.stdout.write(
        `${s.key} ${role} ${cellNm} nm: G ${gRun?.run.g ?? "-"} in ${gRun?.run.seconds.toFixed(1) ?? "-"} s; ` +
          `${Object.entries(windows)
            .map(([w, x]) => {
              return `${w} E1 ${x.e1?.worstChain.error.toExponential(2) ?? "-"} (${x.e1?.failures ?? "-"}) E3 ${x.e3?.worstChain.error.toExponential(2) ?? "-"} (${x.e3?.failures ?? "-"}) E4 ${x.e4?.firstPa.toPrecision(3) ?? "-"} Pa decade ${x.decade?.worstChain.error.toExponential(2) ?? "-"} (${x.decade?.failures ?? "-"})`;
            })
            .join("; ")}\n`,
      );
    }
    record[s.key] = {
      lines: set.lines.length,
      lineFileBytes: Buffer.byteLength(lineFileText(s, set)),
      vanDerWaals: vanDerWaalsCounts(set),
      ratios: atomicBroadeningRatios(set, middleK),
      cells: cells.length,
      nodes,
      samples: runs,
    };
    writeFileSync(values.out, `${JSON.stringify(record, null, 1)}\n`);
  }
}

/** One pass of a sample's whole-ladder reduction: its grid, g-points, cost and chain test. */
interface ReprojectionPass {
  readonly temperatures: number;
  readonly pressures: number;
  readonly g: number;
  readonly seconds: number;
  readonly failures: number;
  readonly worstChain: ChainError;
}

/** A sample cell reduced over its stage's whole ladder, refined as the merge refines a table. */
export interface WholeLadderSample {
  readonly cellNm: number;
  readonly passes: ReadonlyArray<ReprojectionPass>;
  /** The refined nodes, K, and the stored pressures with the alternates kept, Pa. */
  readonly temperaturesK: ReadonlyArray<number>;
  readonly pressuresPa: ReadonlyArray<number>;
  readonly g: number;
  /** The intervals still failing at their finest step, K, `estimated`. */
  readonly estimated: ReadonlyArray<readonly [number, number]>;
  /** Failures at a node's temperature that no alternate can reach. */
  readonly atNodes: number;
  /** Whether the refinement settled within the passes allowed. */
  readonly settled: boolean;
  /** E4's raised first node, Pa, and its tries. */
  readonly e4: ReturnType<typeof raisedFirstNode>;
  /** The 10 Pa hold's 1 Pa check. */
  readonly belowGrid1Pa: number;
}

/** The passes a whole-ladder sample may take: E1′'s three bisections and the alternates kept. */
const REPROJECTION_PASSES = 6;

/**
 * A sample cell reduced on E3's thinned pressures (rows at the standard 30) over a stage's whole
 * ladder, each failing interval bisected and each failing alternate kept as the merge's
 * `sortFailures` sorts them, until the chain test passes or only `estimated` intervals remain.
 */
export function wholeLadderSample(
  set: AtomicLineSet,
  cellNm: number,
  temperaturesK: ReadonlyArray<number>,
): WholeLadderSample | undefined {
  let grid: ReductionGrid = {
    temperaturesK,
    pressuresPa: THINNED_PRESSURES_PA,
    rowsPa: curveRowsPa(THINNED_PRESSURES_PA),
  };
  const passes: ReprojectionPass[] = [];
  for (let pass = 0; pass < REPROJECTION_PASSES; pass += 1) {
    const result = sampleRun(set, cellNm, grid);
    if (result === undefined) {
      return undefined;
    }
    const { run, cell } = result;
    passes.push({
      temperatures: grid.temperaturesK.length,
      pressures: grid.pressuresPa.length,
      g: run.g,
      seconds: run.seconds,
      failures: run.failures,
      worstChain: run.worstChain,
    });
    const sorting = sortFailures(
      grid.temperaturesK,
      cell.failures,
      ladderFinestStepK,
      grid.pressuresPa,
    );
    const settled =
      sorting.refine.length === 0 &&
      sorting.keepPressuresPa.length === 0 &&
      sorting.addMidpointsPa.length === 0;
    if (settled || pass + 1 === REPROJECTION_PASSES) {
      return {
        cellNm,
        passes,
        temperaturesK: grid.temperaturesK,
        pressuresPa: grid.pressuresPa,
        g: run.g,
        estimated: sorting.estimated,
        atNodes: sorting.atNodes.length,
        settled,
        e4: raisedFirstNode(set, cellNm, grid, cell),
        belowGrid1Pa: belowGridError(
          cellNm,
          atomicSpectra(set),
          grid,
          ATOMIC_SHAPE,
          cell,
          set.massDa,
        ),
      };
    }
    const pressuresPa = withKeptPressures(grid.pressuresPa, [
      ...sorting.keepPressuresPa,
      ...sorting.addMidpointsPa,
    ]);
    grid = {
      temperaturesK: refinedTemperatures(grid.temperaturesK, sorting.refine),
      pressuresPa,
      rowsPa: chainRowsPa(pressuresPa),
    };
  }
  return undefined;
}

/** The earlier projection's sample cells by stage key and role, from its records. */
function earlierSamples(paths: string): Map<string, Readonly<Record<string, number>>> {
  const samples = new Map<string, Readonly<Record<string, number>>>();
  for (const path of paths.split(",")) {
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    if (typeof parsed !== "object" || parsed === null) {
      throw new Error(`${path} is not a projection record`);
    }
    for (const [key, value] of Object.entries(parsed)) {
      const runs: unknown =
        typeof value === "object" && value !== null && "samples" in value
          ? value.samples
          : undefined;
      if (typeof runs !== "object" || runs === null) {
        continue;
      }
      const cells: Record<string, number> = {};
      for (const [role, run] of Object.entries(runs)) {
        const nm: unknown =
          typeof run === "object" && run !== null && "cellNm" in run ? run.cellNm : undefined;
        if (typeof nm === "number") {
          cells[role] = nm;
        }
      }
      samples.set(key, cells);
    }
  }
  return samples;
}

/**
 * `atomic-reprojection`: each stage's earlier sample cells reduced over its whole ladder on E3's
 * thinned pressures and refined as the merge refines (decision-r08-t4e-budget §3: "re-run your
 * samples' between-node test on E1′'s nodes … and re-project"), as a JSON record.
 */
export function runAtomicReprojection(rest: readonly string[]): void {
  const { values } = parseArgs({ args: [...rest], options: COMMON_OPTIONS });
  if (
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values.abo === undefined ||
    values.floors === undefined ||
    values.samples === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: atomic-reprojection --kurucz <dir> --barklem <dir> --abo <table1.dat.gz> --deb-hibbert <dir> --floors <json> --samples <json,...> [--ladder E1|E1prime] [--species a,b] [--stride n:k] --out <json>",
    );
  }
  const ladder = ladderOf(values.ladder);
  const data = readSahaData(values.barklem);
  const inputs: AtomicInputs = {
    kuruczDir: values.kurucz,
    data,
    abo: readAboTable(values.abo),
    debHibbertDir: values["deb-hibbert"],
    stehleDir: values.stehle,
  };
  const floors = readFloors(values.floors);
  const samples = earlierSamples(values.samples);
  const record: Record<string, unknown> = {};
  for (const s of sourcesOf(values.species, values.stride)) {
    const cells = samples.get(s.key);
    if (cells === undefined) {
      throw new Error(`no earlier samples for ${s.key}`);
    }
    const set = readAtomicLineSet(s, inputs);
    const nodes = tableNodesOf(s, floors[s.key], ladder);
    const runs: Record<string, WholeLadderSample | undefined> = {};
    for (const [role, cellNm] of Object.entries(cells)) {
      const run = wholeLadderSample(set, cellNm, nodes);
      runs[role] = run;
      const seconds = run?.passes.reduce((t, p) => t + p.seconds, 0) ?? 0;
      process.stdout.write(
        run === undefined
          ? `${s.key} ${ladder} ${role} ${cellNm} nm: no lines\n`
          : `${s.key} ${ladder} ${role} ${cellNm} nm: ${run.passes.length} passes, T ${run.temperaturesK.length} P ${run.pressuresPa.length} G ${run.g}, ${run.estimated.length} estimated, ${run.atNodes} at nodes${run.settled ? "" : ", not settled"}, E4 ${run.e4.firstPa.toPrecision(3)} Pa, 1 Pa ${run.belowGrid1Pa.toExponential(2)}, ${seconds.toFixed(0)} s\n`,
      );
    }
    record[s.key] = {
      ladder,
      cells: cellsWithLines(set).length,
      nodes,
      lineFileBytes: Buffer.byteLength(lineFileText(s, set)),
      samples: runs,
    };
    writeFileSync(values.out, `${JSON.stringify(record, null, 1)}\n`);
  }
}

// --- The tables' reduction: R08.T4.d's stride, merge and refine protocol on a Kurucz source ---

/** A drawn stage's source by its registry key. */
export function atomicSourceOf(key: string | undefined): AtomicSource | undefined {
  return ATOMIC_SOURCES.find((s) => s.key === key);
}

/** The terms of the atomic tables' sources, as read at their fetch (the sidecar's `terms`). */
const ATOMIC_TERMS =
  "R. L. Kurucz states no terms for his line lists: reduced values with their citation, the lists " +
  "not committed (decision-r08-licences.md; decision-r08-cmf-licence.md section 7.3). Barklem and " +
  "Collet 2016's and Barklem, Piskunov and O'Mara 2000's tables are CDS's (VizieR: free of usage in " +
  "a scientific context; A&A-origin tables free for a scientific usage), read 2026-10-19, and only " +
  "derived values are committed";

/** What every atomic table's reduction statement says (its sidecar's `reduction`). */
export const ATOMIC_REDUCTION =
  "Each 1 nm vacuum cell's absorption line by line at each node (Voigt profiles, Humlicek 1982): " +
  "every line of the stage's list over 360-780 nm by vacuum wavelength from a level below its " +
  "ionisation limit, its strength in LTE per absorber of the stage (Boltzmann populations with " +
  "Barklem and Collet 2016's partition functions, stimulated emission counted, no Saha factor: the " +
  "column is the stage's own), its Doppler width thermal, its Lorentz width radiative (Kurucz's " +
  "Gamma_rad, else the classical) plus van der Waals at the node's pressure of pure H2 (Barklem, " +
  "Piskunov and O'Mara 2000's ABO width per H atom where their table holds the line, else Kurucz's " +
  "own, each taken to H2 by Unsold's scaling, C6 proportional to the perturber's polarisability, " +
  "else Unsold's per perturber, a neutral atom's from an upper level of n* 6 or more and at least " +
  "0.1 from an integer bounded by the high-Rydberg width 2 v N (sigma_pol + sigma_F), Alekseev and " +
  "Sobel'man 1966 and Omont 1977, decision-r08-t4e-unsold); cut off 25 cm-1 from each line's " +
  "centre at every pressure (Na D's and K's far " +
  "wings beyond it are their pairs' cross-sections), nodes whose cell's intensity-weighted Lorentz " +
  "half-width passes 0.25 cm-1 estimated; sorted into its k-distribution and reduced to two-point " +
  "Gauss-Legendre g-intervals shared by every node of the cell, by bisection to 8e-4 in T over every " +
  "column to 400 p_eff / (m g) at g = 0.5 m s-2 and helium's Rayleigh column; temperature nodes on " +
  "the stage's ladder (E1, decision-r08-t4ef-storage section 5.3, or E1', decision-r08-t4e-budget " +
  "section 3) from its visibility floor, refined by bisection to a quarter of E1's step where the " +
  "chain test (cubic in T on ln sigma, the client's rows at the standard pressures each read between " +
  "the table's nodes, the read cubic in ln p on tau) fails 2e-3; alternate pressure nodes below " +
  "1 MPa dropped where it passes (E3), and an 8-a-decade midpoint added between two held rows " +
  "below 1 MPa where it fails by the read itself (decision-r08-t4e-unsold's follow-up); ln sigma " +
  "stored in 16 bits a value " +
  "(decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md).";

/** The sidecar's description of an atomic table (R08.T4.e). */
export const ATOMIC_DESCRIPTION =
  "Atoms' and ions' k-distributions per 1 nm cell (rendering plan R08, R08.T4.e; " +
  "decision-r08-t4ef-storage; decision-r08-t4e-budget), written by " +
  "apps/hyperion/scripts/atmosphereData.mjs atomic-k-distributions-merge from the fetched lists " +
  "named here, which are not committed. " +
  PART_LAYOUT_TEXT +
  " For these tables pressuresPa are H2-equivalent, the pressure of pure H2 that gives the lines " +
  "their van der Waals widths at the node's temperature, and the broadening ratios are each gas's " +
  "width over H2's, a gas not listed taken as H2, estimated. Reduced values; the citations are in " +
  "NOTICE's Data section.";

/** A stage's inputs as its sidecar lists them: its list, Barklem and Collet's tables, ABO's. */
function atomicInputs(source: AtomicSource): string {
  const own = `${source.list.name} ${source.list.sha256}`;
  const saha = Object.values(BARKLEM_TABLES)
    .map((t) => `${t.name} ${t.sha256}`)
    .join(", ");
  const forbidden =
    source.key === "Fe+"
      ? `; ${Object.values(DEB_HIBBERT_TABLES)
          .map((t) => `${t.name} ${t.sha256}`)
          .join(", ")}`
      : "";
  return `${own}; ${saha}; ${ABO_TABLE.name} ${ABO_TABLE.sha256}${forbidden}`;
}

/**
 * What H I's table adds to {@link ATOMIC_REDUCTION}: its Balmer lines' Stark broadening and
 * self-broadening, and why it is `estimated`.
 */
export const HYDROGEN_REDUCTION =
  " H I's Balmer lines in LTE (a non-LTE thermosphere is not modelled) also take Stark broadening, " +
  "a Lorentzian of Stehle and Hutcheon 1999's half width for the line's upper level (H10's above " +
  "it) at the node's electron density from the solar-abundance Saha balance at its temperature and " +
  "pressure, and in their broadening ratios Barklem, Piskunov and O'Mara 2000b's self-broadening by " +
  "H atoms (H-alpha, H-beta and H-gamma, H-gamma's above), H2 and He keeping Unsold's. The table " +
  "is estimated: the electron density is not an axis, and a Lorentzian overstates the quasi-static " +
  "ion wing.";

/**
 * What Na I's table adds to {@link ATOMIC_REDUCTION}: Na D's widths by H₂ from Allard et al. 2019
 * (`atomicKDistributions.ts`'s `NA_D_H2_WIDTHS`).
 */
export const SODIUM_REDUCTION =
  " Na D's van der Waals widths by H2 are Allard et al. 2019's ab initio impact half widths (their " +
  "tables' headers at 1e21 cm-3 over 500-3000 K, ln w linear in ln T, and a power law beyond to " +
  "5000 K), by He and the air gases scaled from them by Unsold's rule, by H the lines' ABO widths; " +
  "their impact shifts are left out (under 0.13 cm-1 at 1e19 cm-3).";

/** Allard et al. 2019's deposit as a sidecar's inputs name it. */
const ALLARD_INPUTS =
  "opacity.tar.gz b0c73fe0c6cedb00f60565bc3ac891209e3bdf36d3f160fc0851c0e0b5fdf791 (its tables' " +
  "own SHA-256 in src/tools/alkaliPairs.ts's ALLARD_TABLES)";

/** Allard et al. 2019's paper and deposit, for Na D's widths by H₂. */
const ALLARD_WIDTHS_CITATION =
  "Na D's impact widths by H2 from N. F. Allard, F. Spiegelman, T. Leininger and P. Molliere, " +
  "Astron. Astrophys. 628 (2019) A120 (CDS J/A+A/628/A120)";

/** Allard et al. 2019's deposit's directory. */
const ALLARD_URL = "https://cdsarc.cds.unistra.fr/ftp/J/A+A/628/A120/";

/** A drawn stage's table as its sidecar names it ({@link TableSource}). */
export function atomicTableSource(
  source: AtomicSource,
  measuredRangeK: readonly [number, number],
): TableSource {
  const hydrogen = source.key === "H";
  const sodium = source.key === "Na";
  return {
    species: source.key,
    citation:
      `R. L. Kurucz's line list ${source.list.name} (${source.spectrum}; ${KURUCZ_CITATION}); ` +
      "P. S. Barklem and R. Collet, Astron. Astrophys. 588 (2016) A96 (CDS J/A+A/588/A96, tables 4, 7 " +
      "and 8); P. S. Barklem, N. Piskunov and B. J. O'Mara, Astron. Astrophys. Suppl. Ser. 142 " +
      "(2000) 467 (CDS J/A+AS/142/467)" +
      (source.key === "Fe+"
        ? "; [Fe II]'s forbidden lines from S. N. Deb and A. Hibbert, Astron. Astrophys. 536 (2011) " +
          "A74 (CDS J/A+A/536/A74, tables 5, 6 and 9)"
        : "") +
      (hydrogen
        ? "; the Balmer lines' Stark half widths from C. Stehle and R. Hutcheon, Astron. Astrophys. " +
          "Suppl. Ser. 140 (1999) 93 (CDS VI/98A), and their self-broadening from P. S. Barklem, N. " +
          "Piskunov and B. J. O'Mara, Astron. Astrophys. 363 (2000) 1091, Table 3"
        : "") +
      (sodium ? `; ${ALLARD_WIDTHS_CITATION}` : ""),
    url:
      `${KURUCZ_URL}${source.list.name}; https://cdsarc.cds.unistra.fr/ftp/J/A+A/588/A96/; ` +
      "https://cdsarc.cds.unistra.fr/ftp/J/A+AS/142/467/" +
      (source.key === "Fe+" ? "; https://cdsarc.cds.unistra.fr/ftp/J/A+A/536/A74/" : "") +
      (hydrogen ? `; ${STEHLE_URL}` : "") +
      (sodium ? `; ${ALLARD_URL}` : ""),
    inputs:
      atomicInputs(source) +
      (hydrogen
        ? "; Stehle and Hutcheon's ba03-ba10 profil1-profil11.dat, each SHA-256 in " +
          "src/tools/balmerStark.ts's STEHLE_PROFILES"
        : "") +
      (sodium ? `; ${ALLARD_INPUTS}` : ""),
    fetched: "2026-10-16 by main and 2026-10-19 by R08.T4.e (each matching)",
    terms: sodium
      ? `${ATOMIC_TERMS}; Allard et al.'s tables are CDS's too (J/A+A/628/A120, read 2026-10-19; ` +
        "the deposit's README states none), read as data, the deposit's readers not run"
      : ATOMIC_TERMS,
    basis: hydrogen ? "estimated" : "measured",
    measuredRangeK,
    policy: ATOMIC_SHAPE,
    reduction:
      ATOMIC_REDUCTION +
      (hydrogen ? HYDROGEN_REDUCTION : "") +
      (sodium ? SODIUM_REDUCTION : "") +
      (source.chainSaturation === undefined
        ? ""
        : ` The between-node test also stops at 400 p_sat / (m g) at g = 0.5 m s-2, p_sat ` +
          `${source.chainSaturation.source} (R08.T4.e's residual protocol, item (ii), ` +
          "decision-r08-t4e-unsold), errors beyond it recorded, not gated."),
    description: ATOMIC_DESCRIPTION,
    finestStepK: ladderFinestStepK,
  };
}

/** A list of numbers from the command line, or `undefined` where none is given. */
function numbersOf(text: string | undefined): number[] | undefined {
  return text?.split(",").map(Number);
}

/** The atoms' grid: the nodes given, E3's thinned pressures unless given, and their rows. */
export function atomicGrid(
  temperaturesK: ReadonlyArray<number>,
  pressuresPa: ReadonlyArray<number> = THINNED_PRESSURES_PA,
): ReductionGrid {
  return { temperaturesK, pressuresPa, rowsPa: chainRowsPa(pressuresPa) };
}

/**
 * The broadening records a stage's partials carry: its gases' ratios to H₂ at the middle node
 * ({@link atomicBroadeningRatios}) and their exponent offsets, all 0, since every gas's width shares
 * each line's exponent.
 */
export function atomicBroadening(
  set: AtomicLineSet,
  temperaturesK: ReadonlyArray<number>,
): Pick<PartialFile, "broadening" | "broadeningExponents"> {
  const middleK = temperaturesK[Math.floor(temperaturesK.length / 2)] ?? LADDER_TOP_K;
  const broadening = atomicBroadeningRatios(set, middleK);
  return {
    broadening,
    broadeningExponents: Object.fromEntries(Object.keys(broadening).map((gas) => [gas, 0])),
  };
}

/** Options of the atomic reduction's subcommands. */
const REDUCTION_OPTIONS = {
  ...COMMON_OPTIONS,
  cells: { type: "string" },
  temperatures: { type: "string" },
  pressures: { type: "string" },
  "out-dir": { type: "string" },
  "measured-from": { type: "string" },
} as const;

/**
 * `atomic-k-distributions`: reduces a range of a drawn stage's cells to a partial file, as
 * `k-distributions` does a HITRAN species' ({@link reducePartialCell}), on the nodes given and E3's
 * thinned pressures unless given; a first pressure node above 10 Pa (E4) has its hold checked at
 * 1 Pa and every standard node below it.
 */
export function runAtomicKDistributions(rest: readonly string[]): void {
  const { values } = parseArgs({ args: [...rest], options: REDUCTION_OPTIONS });
  const source = atomicSourceOf(values.species);
  const temperaturesK = numbersOf(values.temperatures);
  if (
    source === undefined ||
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values.abo === undefined ||
    temperaturesK === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: atomic-k-distributions --species <key> --kurucz <dir> --barklem <dir> --abo <table1.dat.gz> [--deb-hibbert <dir>] --temperatures t,... [--pressures p,...] [--cells a-b] [--stride n:k] --out <json>",
    );
  }
  const set = readAtomicLineSet(source, {
    kuruczDir: values.kurucz,
    data: readSahaData(values.barklem),
    abo: readAboTable(values.abo),
    debHibbertDir: values["deb-hibbert"],
    stehleDir: values.stehle,
  });
  const grid = atomicGrid(temperaturesK, numbersOf(values.pressures));
  const firstPa = grid.pressuresPa[0] ?? STORED_PRESSURES_PA[0] ?? 10;
  const reductionSource = {
    species: source.key,
    policy: ATOMIC_SHAPE,
    molecularMassDa: set.massDa,
    ...(source.chainSaturation === undefined ? {} : { chainSaturation: source.chainSaturation }),
    ...(firstPa > (STORED_PRESSURES_PA[0] ?? 10) * (1 + 1e-9)
      ? { belowGridChecksPa: raisedFirstNodeChecksPa(firstPa) }
      : {}),
  };
  const spectra = atomicSpectra(set);
  const [from, to] = (values.cells ?? "380-760").split("-").map(Number);
  const [stride, offset] = (values.stride ?? "1:0").split(":").map(Number);
  const cells: PartialCell[] = [];
  for (let cellNm = (from ?? 380) + (offset ?? 0); cellNm <= (to ?? 760); cellNm += stride ?? 1) {
    const cell = reducePartialCell(cellNm, spectra, grid, reductionSource);
    if (cell !== undefined) {
      cells.push(cell.partial);
    }
  }
  const partial: PartialFile = {
    species: source.key,
    temperaturesK: grid.temperaturesK,
    pressuresPa: grid.pressuresPa,
    ...atomicBroadening(set, grid.temperaturesK),
    cells,
  };
  writeFileSync(values.out, `${JSON.stringify(partial)}\n`);
  // The run's costs, for the record: its CPU time and its peak resident memory.
  const usage = process.resourceUsage();
  process.stdout.write(
    `${source.key}: ${cells.length} cells, CPU ${((usage.userCPUTime + usage.systemCPUTime) / 1e6).toFixed(0)} s, peak RSS ${(usage.maxRSS / 1024).toFixed(0)} MB\n`,
  );
}

/** Whether a value read from a partial is a {@link PartialFile}: a trust boundary of our own writing. */
function isPartial(value: unknown): value is PartialFile {
  return typeof value === "object" && value !== null && "cells" in value && "species" in value;
}

/**
 * `atomic-k-distributions-merge`: checks a drawn stage's partials and writes its parts and sidecar
 * (`mergePartials`), or prints the refined nodes (`refine:`) and, on a thinned table, the pressures
 * with the alternates to keep (`refine-pressures:`) to run again with.
 *
 * @returns The exit code: 0 when written, 2 to refine, 3 when refused.
 */
export function runAtomicKDistributionsMerge(rest: readonly string[]): number {
  const { values, positionals } = parseArgs({
    args: [...rest],
    allowPositionals: true,
    options: REDUCTION_OPTIONS,
  });
  const source = atomicSourceOf(values.species);
  const outDir = values["out-dir"];
  const measuredFromK = Number(values["measured-from"]);
  if (
    source === undefined ||
    outDir === undefined ||
    !Number.isFinite(measuredFromK) ||
    positionals.length === 0
  ) {
    throw new Error(
      "usage: atomic-k-distributions-merge --species <key> --out-dir <dir> --measured-from <K> <partial>...",
    );
  }
  const read = positionals.map((path): PartialFile => {
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    if (!isPartial(parsed) || parsed.species !== source.key) {
      throw new Error(`${path} is not a partial of ${source.key}`);
    }
    return parsed;
  });
  // With the stage's inputs given, its broadening records are taken from its lines afresh, the
  // partials' set aside (as `k-distributions-merge --lines` does).
  const partials =
    values.kurucz === undefined || values.barklem === undefined || values.abo === undefined
      ? read
      : (() => {
          const set = readAtomicLineSet(source, {
            kuruczDir: values.kurucz,
            data: readSahaData(values.barklem),
            abo: readAboTable(values.abo),
            debHibbertDir: values["deb-hibbert"],
            stehleDir: values.stehle,
          });
          return read.map((p) => Object.assign({}, p, atomicBroadening(set, p.temperaturesK)));
        })();
  const topK = Math.max(...(partials[0]?.temperaturesK ?? [LADDER_TOP_K]));
  const outcome = mergePartials(
    atomicTableSource(source, [measuredFromK, topK]),
    partials,
    (line) => process.stdout.write(line),
  );
  let code: number;
  switch (outcome.kind) {
    case "refine":
      process.stdout.write(`refine: ${outcome.temperaturesK.join(",")}\n`);
      if (outcome.pressuresPa !== undefined) {
        process.stdout.write(`refine-pressures: ${outcome.pressuresPa.join(",")}\n`);
      }
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
      writeFileSync(join(outDir, `${source.key}.json`), outcome.sidecar);
      process.stdout.write(outcome.summary);
      code = 0;
      break;
  }
  return code;
}

// --- The alkali far wings' pairs (alkaliPairs.ts) ---

/** The estimated pairs' temperatures, K: the atoms' range, 500–5,000 K. */
export const ESTIMATED_PAIR_TEMPERATURES_K = [
  500, 1_000, 1_500, 2_000, 2_500, 3_000, 3_500, 4_000, 4_500, 5_000,
] as const;

/** A resonance doublet's indices in a stage's set: D2 (to J = 3/2) and D1 (to J = 1/2). */
export function resonanceDoublet(set: AtomicLineSet): { readonly d2: number; readonly d1: number } {
  const of = (upperJ: number): number => {
    const i = set.lines.findIndex(
      (l) => l.lowerPerCm === 0 && l.lowerJ === 0.5 && l.upperJ === upperJ,
    );
    if (i < 0) {
      throw new Error(`${set.stage.key}'s list holds no resonance line to J = ${upperJ}`);
    }
    return i;
  };
  return { d2: of(1.5), d1: of(0.5) };
}

/** A pair's rows on {@link PAIR_GRID}: each cell's nm and its σ at each temperature, m⁵. */
function pairRows(columns: ReadonlyArray<Float64Array>): string {
  return Array.from({ length: PAIR_GRID.points }, (_, c) => {
    const nm = PAIR_GRID.firstNm + c * PAIR_GRID.stepNm;
    const values = columns.map((col) => {
      const v = col[c] ?? 0;
      return v === 0 ? "0" : String(Number(v.toPrecision(6)));
    });
    return `      [${nm}, ${values.join(", ")}]`;
  }).join(",\n");
}

/** One pair of the file, as `absorbers.ts`'s `readPairCrossSections` reads it. */
interface PairEntry {
  readonly pair: string;
  readonly source: string;
  readonly url: string;
  readonly sha256: string;
  /** Its sources' terms as read at fetch: Allard et al.'s for Na's pairs, ABO's for K's. */
  readonly terms: string;
  readonly reduction: string;
  readonly temperaturesK: ReadonlyArray<number>;
  readonly measuredRangeK: readonly [number, number];
  /** Every wavelength `estimated` (the grid's end, 800 nm, and beyond), for a stated estimate. */
  readonly estimatedEverywhere: boolean;
  readonly columns: ReadonlyArray<Float64Array>;
}

/** A pair entry's JSON text. */
function pairText(entry: PairEntry): string {
  const members = entry.pair.split("+");
  const fields = [
    `      "pair": ${JSON.stringify(entry.pair)}`,
    `      "members": ${JSON.stringify(members)}`,
    `      "source": ${JSON.stringify(entry.source)}`,
    `      "url": ${JSON.stringify(entry.url)}`,
    `      "sha256": ${JSON.stringify(entry.sha256)}`,
    `      "fetched": "2026-10-19"`,
    `      "terms": ${JSON.stringify(entry.terms)}`,
    `      "reduction": ${JSON.stringify(entry.reduction)}`,
    `      "firstNm": ${PAIR_GRID.firstNm}`,
    `      "stepNm": ${PAIR_GRID.stepNm}`,
    `      "temperaturesK": [${entry.temperaturesK.join(", ")}]`,
    `      "temperatureLaw": "linear"`,
    `      "measuredRangeK": [${entry.measuredRangeK.join(", ")}]`,
    ...(entry.estimatedEverywhere ? [`      "estimatedBelowNm": 801`] : []),
    `      "rows": [\n${pairRows(entry.columns)}\n      ]`,
  ];
  return `    {\n${fields.join(",\n")}\n    }`;
}

/** The pairs' sources' terms, as read at their fetch. */
const ALLARD_PAIR_TERMS =
  "Allard et al.'s tables are CDS's (J/A+A/628/A120; VizieR's data free of usage in a scientific " +
  "context, A&A-origin tables free for a scientific usage, read 2026-10-19; the deposit's README " +
  "states none); Kurucz states no terms: reduced values with their citations, the raw files not " +
  "committed and the deposit's readers not run (decision-r08-licences.md)";

/** The terms of K's pairs, which take Barklem et al.'s ABO widths, not Allard et al.'s tables. */
const ABO_PAIR_TERMS =
  "Barklem et al.'s ABO table is CDS's (J/A+AS/142/467; VizieR's data free of usage in a " +
  "scientific context, A&A-origin tables free for a scientific usage, read 2026-10-19; its ReadMe " +
  "states none); Kurucz states no terms: reduced values with their citations, the raw files not " +
  "committed (decision-r08-licences.md)";

/** The lines' and partition functions' citation every pair shares. */
function linesCitation(list: string): string {
  return (
    `the doublet's gf and vacuum wavenumbers from R. L. Kurucz's ${list} (${KURUCZ_CITATION}); ` +
    "its lower level's share by Barklem and Collet 2016's partition functions (Astron. Astrophys. " +
    "588 (2016) A96, CDS J/A+A/588/A96 table 8)"
  );
}

/** What every pair's reduction shares: the cells and the split from the atom's own table. */
const PAIR_CELLS =
  "Each 1 nm vacuum cell, [nm - 0.5, nm + 0.5), of 380-800 nm takes the mean over 2000 equal steps " +
  "of wavelength of D2's and D1's wings at points more than 25 cm-1 from each line's centre, where " +
  "the atom's own k-distribution table holds the line (R08.T4.e), converted from cm5 to m5, linear " +
  "in T between the temperatures and held beyond them (estimated).";

/** A line's vacuum wavenumber in a set, cm⁻¹. */
function lineCentrePerCm(set: AtomicLineSet, index: number): number {
  return set.wavenumbersPerCm[index] ?? Number.NaN;
}

/** A checked input as a sidecar names it: its file and SHA-256. */
function checkedName(input: CheckedInput): string {
  return `${input.name} ${input.sha256}`;
}

/** A pair's largest cell at a temperature, for the run's log. */
function peakText(column: Float64Array): string {
  const i = column.indexOf(Math.max(...column));
  return `${(column[i] ?? 0).toExponential(2)} m5 at ${PAIR_GRID.firstNm + i} nm`;
}

/** `alkali-pairs`: writes the four alkali wing pairs' file ({@link PairEntry}). */
export function runAlkaliPairs(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: { ...COMMON_OPTIONS, allard: { type: "string" } },
  });
  if (
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values.abo === undefined ||
    values.allard === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: alkali-pairs --kurucz <dir> --barklem <dir> --abo <table1.dat.gz> --allard <tables dir> --out <json>",
    );
  }
  const inputs: AtomicInputs = {
    kuruczDir: values.kurucz,
    data: readSahaData(values.barklem),
    abo: readAboTable(values.abo),
    debHibbertDir: undefined,
  };
  const sodium = atomicSourceOf("Na");
  const potassium = atomicSourceOf("K");
  if (sodium === undefined || potassium === undefined) {
    throw new Error("the atomic sources hold Na and K");
  }
  const na = readAtomicLineSet(sodium, inputs);
  const k = readAtomicLineSet(potassium, inputs);
  const naD = resonanceDoublet(na);
  const kD = resonanceDoublet(k);
  const allard = values.allard;
  const naH2 = ALLARD_TEMPERATURES_K.map((t) => {
    const tables = readAllardTables(allard, t);
    return pairCellMeansM5([
      {
        centrePerCm: lineCentrePerCm(na, naD.d2),
        wing: allardWing(na, naD.d2, tables.d2, undefined, t),
      },
      {
        centrePerCm: lineCentrePerCm(na, naD.d1),
        wing: allardWing(na, naD.d1, tables.d1, tables.d1Wing, t),
      },
    ]);
  });
  const estimate = (
    set: AtomicLineSet,
    doublet: { readonly d2: number; readonly d1: number },
    perturber: "H2" | "He",
  ): Float64Array[] =>
    ESTIMATED_PAIR_TEMPERATURES_K.map((t) =>
      pairCellMeansM5([
        {
          centrePerCm: lineCentrePerCm(set, doublet.d2),
          wing: impactWing(set, doublet.d2, perturber, t),
        },
        {
          centrePerCm: lineCentrePerCm(set, doublet.d1),
          wing: impactWing(set, doublet.d1, perturber, t),
        },
      ]),
    );
  const estimateReduction = (atom: string, perturber: string, widths: string): string =>
    `A stated estimate (decision-r08-t4ef-storage section 6, ruling 6; no open ${atom}-${perturber} ` +
    "set found at fetch): R08.T4.c's impact (Lorentz) far wing, linear in the perturber's density, " +
    `per pair S gamma_p / (pi Delta^2), with S the line's strength per ${atom} atom and gamma_p its ` +
    `half width per ${perturber} as the atom's table takes it (${widths} scaled by Unsold's ` +
    "polarisability rule), so that the wing meets the core at " +
    "25 cm-1; estimated at every temperature and wavelength (estimatedBelowNm past the grid), since " +
    "past about 50 cm-1 the wing is quasi-static, not impact, and may lie a few times above or below " +
    "it (Na-H2's open tables run 2-12 times their impact estimate in D2's blue wing at " +
    "200-2000 cm-1). " +
    PAIR_CELLS;
  const aboCitation =
    "P. S. Barklem, N. Piskunov and B. J. O'Mara, Astron. Astrophys. Suppl. Ser. 142 (2000) 467 " +
    "(CDS J/A+AS/142/467)";
  const aboWidths = "Barklem, Piskunov and O'Mara 2000's ABO width per H atom";
  const saha = `${BARKLEM_TABLES.partitionFunctions.name} ${BARKLEM_TABLES.partitionFunctions.sha256}`;
  const entries: PairEntry[] = [
    {
      pair: "Na+H2",
      source:
        "N. F. Allard, F. Spiegelman, T. Leininger and P. Molliere, New study of the line profiles of " +
        "sodium perturbed by H2, Astron. Astrophys. 628 (2019) A120, DOI 10.1051/0004-6361/201935593 " +
        "(CDS J/A+A/628/A120, opacity.tar.gz, tables 2017); " +
        linesCitation(sodium.list.name),
      url: `${ALLARD_URL}; ${KURUCZ_URL}${sodium.list.name}`,
      sha256: `${ALLARD_INPUTS}; ${checkedName(sodium.list)}; ${saha}`,
      terms: ALLARD_PAIR_TERMS,
      reduction:
        "Allard et al.'s one-perturber wing: each table's first-order expansion coefficient c1 " +
        "(their section 3.3, Eq. 15) at n_ref = 1e21 cm-3, the normalised profile per H2 c1 / n_ref, " +
        "times the line's strength per Na atom S, linear in c1 between the tables' detunings and 0 " +
        "where c1 falls below 0 (D2's blue edge); D1's red wing beyond its near-wing table from its " +
        "redwing file, sigma at 1e21 cm-3 linear in density by the deposit's README, ln sigma linear " +
        "between its points; D1's blue wing beyond its near-wing table, not deposited, left out (2-5% " +
        "of D2's blue wing at 200-2000 cm-1 by its impact estimate); 0 beyond the tables' detunings. " +
        "Measured over the tables' 500-3000 K. " +
        PAIR_CELLS,
      temperaturesK: ALLARD_TEMPERATURES_K,
      measuredRangeK: [500, 3_000],
      estimatedEverywhere: false,
      columns: naH2,
    },
    {
      pair: "Na+He",
      source: `Stated estimate; ${ALLARD_WIDTHS_CITATION}; ${linesCitation(sodium.list.name)}`,
      url: `${KURUCZ_URL}${sodium.list.name}; ${ALLARD_URL}`,
      sha256: `${checkedName(sodium.list)}; ${saha}; ${ALLARD_INPUTS}`,
      terms: ALLARD_PAIR_TERMS,
      reduction: estimateReduction("Na", "He", "Allard et al. 2019's ab initio impact width by H2"),
      temperaturesK: ESTIMATED_PAIR_TEMPERATURES_K,
      measuredRangeK: [500, 5_000],
      estimatedEverywhere: true,
      columns: estimate(na, naD, "He"),
    },
    {
      pair: "K+H2",
      source: `Stated estimate; ${aboCitation}; ${linesCitation(potassium.list.name)}`,
      url: `${KURUCZ_URL}${potassium.list.name}; https://cdsarc.cds.unistra.fr/ftp/J/A+AS/142/467/`,
      sha256: `${checkedName(potassium.list)}; ${saha}; ${ABO_TABLE.name} ${ABO_TABLE.sha256}`,
      terms: ABO_PAIR_TERMS,
      reduction:
        estimateReduction("K", "H2", aboWidths) +
        " Allard et al. 2016's open K-H2 set (Astron. Astrophys. 589 (2016) A21, CDS " +
        "J/A+A/589/A21) holds only the red wing beyond 776 nm, outside the range.",
      temperaturesK: ESTIMATED_PAIR_TEMPERATURES_K,
      measuredRangeK: [500, 5_000],
      estimatedEverywhere: true,
      columns: estimate(k, kD, "H2"),
    },
    {
      pair: "K+He",
      source: `Stated estimate; ${aboCitation}; ${linesCitation(potassium.list.name)}`,
      url: `${KURUCZ_URL}${potassium.list.name}; https://cdsarc.cds.unistra.fr/ftp/J/A+AS/142/467/`,
      sha256: `${checkedName(potassium.list)}; ${saha}; ${ABO_TABLE.name} ${ABO_TABLE.sha256}`,
      terms: ABO_PAIR_TERMS,
      reduction: estimateReduction("K", "He", aboWidths),
      temperaturesK: ESTIMATED_PAIR_TEMPERATURES_K,
      measuredRangeK: [500, 5_000],
      estimatedEverywhere: true,
      columns: estimate(k, kD, "He"),
    },
  ];
  const description =
    "The alkali resonance doublets' far wings beyond the atoms' 25 cm-1 shape as pair " +
    "cross-sections, m5 a pair, on vacuum wavelengths (rendering plan R08, R08.T4.e; follow-up 5, " +
    "ruling 2), written by apps/hyperion/scripts/atmosphereData.mjs alkali-pairs from the fetched " +
    "files named here, which are not committed: Na D's with H2 from Allard et al. 2019's open " +
    "tables, and Na D's with He and K's with H2 and He as the ruled estimate. Each row is [nm, sigma " +
    "at each temperature]. Reduced values; the citations are in NOTICE's Data section.";
  writeFileSync(
    values.out,
    `{\n  "description": ${JSON.stringify(description)},\n  "pairs": [\n${entries.map(pairText).join(",\n")}\n  ]\n}\n`,
  );
  for (const e of entries) {
    process.stdout.write(
      `${e.pair}: ${e.temperaturesK.map((t, i) => `${t} K ${peakText(e.columns[i] ?? new Float64Array(0))}`).join("; ")}\n`,
    );
  }
}

// --- The atoms' line-by-line references (lineReferences.ts's cellReference) ---

/** Whether a value is a sidecar's grid: a trust boundary of our own writing. */
function isSidecarGrid(value: unknown): value is {
  readonly temperaturesK: ReadonlyArray<number>;
  readonly pressuresPa: ReadonlyArray<number>;
} {
  return (
    typeof value === "object" &&
    value !== null &&
    "temperaturesK" in value &&
    "pressuresPa" in value
  );
}

/**
 * `atomic-cell-references --references <key=nm,…;…> --tables <dir> …`: each named stage's named
 * cells' line-by-line curves of growth beside the reduction's `f64` ones through the client's
 * chain, at three node pressures and three midpoints about its table's middle node, on its
 * committed table's grid (R08.T4.d's `cell-references`, for R08.T4.e's tables), as one JSON file.
 */
export function runAtomicCellReferences(rest: readonly string[]): void {
  const { values } = parseArgs({
    args: [...rest],
    options: { ...REDUCTION_OPTIONS, references: { type: "string" }, tables: { type: "string" } },
  });
  if (
    values.kurucz === undefined ||
    values.barklem === undefined ||
    values.abo === undefined ||
    values.references === undefined ||
    values.tables === undefined ||
    values.out === undefined
  ) {
    throw new Error(
      "usage: atomic-cell-references --kurucz <dir> --barklem <dir> --abo <table1.dat.gz> [--stehle <dir>] --references <key=nm,...;...> --tables <dir> --out <json>",
    );
  }
  const inputs: AtomicInputs = {
    kuruczDir: values.kurucz,
    data: readSahaData(values.barklem),
    abo: readAboTable(values.abo),
    debHibbertDir: values["deb-hibbert"],
    stehleDir: values.stehle,
  };
  const tables = values.tables;
  const references = values.references.split(";").map((spec) => {
    const [key = "", cells = ""] = spec.split("=");
    const source = atomicSourceOf(key);
    if (source === undefined) {
      throw new Error(`no drawn stage ${key}`);
    }
    const sidecar: unknown = JSON.parse(readFileSync(join(tables, `${key}.json`), "utf8"));
    if (!isSidecarGrid(sidecar)) {
      throw new Error(`${key}.json is not a k-distribution sidecar`);
    }
    const grid = atomicGrid(sidecar.temperaturesK, sidecar.pressuresPa.map(latticePressurePa));
    const set = readAtomicLineSet(source, inputs);
    const spectra = atomicSpectra(set);
    const middleK = grid.temperaturesK[Math.floor(grid.temperaturesK.length / 2)] ?? LADDER_TOP_K;
    return {
      species: key,
      cells: cells
        .split(",")
        .map(Number)
        .map((cellNm) => {
          const cap = (t: number, p: number): number =>
            gatedColumnCm2(undefined, cellNm, t, p, set.massDa);
          const chainCap = (t: number, p: number): number =>
            chainColumnCm2(undefined, cellNm, t, p, set.massDa, undefined);
          process.stdout.write(`atomic-cell-references: ${key} ${cellNm} nm\n`);
          return cellReference(cellNm, spectra, grid, ATOMIC_SHAPE, cap, chainCap, middleK);
        }),
    };
  });
  writeFileSync(values.out, `${JSON.stringify({ references })}\n`);
}

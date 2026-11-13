/**
 * The alkali resonance lines' far wings as pair cross-sections (plan R08, R08.T4.e; follow-up 5,
 * ruling 2; decision-r08-t4ef-storage §6 and its ruling 6): Na D's and K's doublets' absorption
 * beyond the atoms' 25 cm⁻¹ shape, per (atom, perturber) pair, m⁵, linear in the perturber's
 * density, for `absorbers/alkaliPairs.json`.
 *
 * @remarks
 * - **`Na+H2`** is N. F. Allard et al.'s (Astron. Astrophys. 628 (2019) A120; CDS
 *   J/A+A/628/A120, its `opacity.tar.gz`'s tables 2017, read as data, the deposit's readers not
 *   run). Each table holds, at each detuning Δω from the line, the coefficients of the profile's
 *   expansion in the H₂ density (their §3.3, Eq. 15: I(Δω) = I_c ∗ [δ − n_p I_w + (n_p²/2!) I_w^∗2
 *   − …], σ = π r₀ f I, their Eq. 16) at n_ref = 10²¹ cm⁻³. The first-order term, the one-perturber
 *   wing linear in n_p, is c₁(Δω) ÷ n_ref of the normalised profile a perturber, so S c₁ ÷ n_ref a
 *   pair (cm⁵), S the line's strength per atom. At ±20–30 cm⁻¹ of either line at 500–3,000 K,
 *   about the core's 25 cm⁻¹ edge, π Δ² c₁ ÷ w is 0.6–1.3, w the table's impact half width: the
 *   one-perturber wing meets the impact (Lorentz) core there. At −100 cm⁻¹ from D1 at 1,000 K the
 *   README's linear D1 red wing is 0.91 times π r₀ f c₁ (the higher orders at 10²¹). Where c₁
 *   falls below 0 at D2's blue edge (beyond +2,900 cm⁻¹ at 1,000 K) the pair is 0. D1's table
 *   reaches −100 to +86 cm⁻¹ at 1,000 K; its red wing beyond is its `redwing` file, σ at 10²¹ cm⁻³
 *   linear in density by the README; its blue wing beyond the near-wing table is not deposited and
 *   is left out (by its impact estimate 2–5% of D2's blue wing at 200–2,000 cm⁻¹). The tables span
 *   500–3,000 K; beyond, the nearer end is held, `estimated`.
 * - **`Na+He`, `K+H2` and `K+He`** take the ruled estimate: T4.c's impact (Lorentz) far wing,
 *   S γ_p ÷ (π Δ²) a pair, γ_p the line's half width per perturber as the atoms' tables take it
 *   (`atomicKDistributions.ts`'s `vanDerWaalsWidthM3PerS`: for K Barklem, Piskunov and O'Mara
 *   2000's ABO width, for Na D Allard et al.'s H₂ width, scaled by Unsöld's rule), so that the wing
 *   meets the core at 25 cm⁻¹; `estimated` at every temperature: past about 50 cm⁻¹ the wing is
 *   quasi-static, not impact. No open Na–He or K–He set was found at fetch, and K–H₂'s deposit (Allard et al. 2016, CDS J/A+A/589/A21) holds
 *   only the red wing beyond 776 nm.
 * - Each line's strength per atom of the stage is S = π r_e gf e^(−c₂E″/T)(1 − e^(−c₂ν/T)) ÷ Q(T),
 *   Barklem and Collet 2016's Q; Allard's σ, per atom in the line's lower level, takes the same
 *   factor g″ e^(−c₂E″/T)(1 − e^(−c₂ν/T)) ÷ Q(T).
 * - Each 1 nm vacuum cell [λ − 0.5, λ + 0.5) of 380–800 nm takes the mean over the cell of every
 *   line's pair σ at points more than 25 cm⁻¹ from its centre: within it, the atom's own table holds
 *   the line, so nothing is counted twice.
 */

import { join } from "node:path";

import {
  atomicLineStrengthCm,
  type AtomicLineSet,
  CM3_PER_M3,
  NA_D_H2_WIDTHS,
  vanDerWaalsWidthM3PerS,
} from "./atomicKDistributions";
import { LINE_CUTOFF_PER_CM, readCheckedText, SPEED_OF_LIGHT_M_PER_S } from "./molecularLines";

/** One of Allard et al.'s expansion tables, as its README's lines 10–16 give it. */
export interface AllardTable {
  readonly temperatureK: number;
  /** The line's centre, Å (approximate, the README says), and the table's reference density, cm⁻³. */
  readonly wavelengthA: number;
  readonly referencePerCm3: number;
  /** π r₀ f, cm: the profile's area normalisation. */
  readonly normalisationCm: number;
  /**
   * The impact half width at half maximum and shift at the reference density, cm⁻¹ (line 12; the
   * deposit's `lect_sig.f` builds its impact core as a Lorentzian of this half width).
   */
  readonly impactHwhmPerCm: number;
  readonly impactShiftPerCm: number;
  /** The detunings, cm⁻¹, ascending, and each one's first-order coefficient c₁. */
  readonly detuningsPerCm: ReadonlyArray<number>;
  readonly firstOrder: ReadonlyArray<number>;
}

/**
 * An expansion table (the README's "Contents of the table files"): a text header to its `end` line,
 * then the line centre, reference density, vn and π r₀ f; the entries' count and the expansion's
 * order; the impact width and shift; then for each entry its Δω and its coefficients, first order
 * first, running over as many lines as they take.
 *
 * @throws Error for a table that is not so.
 */
export function parseAllardTable(text: string): AllardTable {
  const lines = text.split(/\r?\n/u);
  const temperature = /^TK:\s*(\S+)/u.exec(lines[1] ?? "");
  const end = lines.findIndex((l) => l.trim() === "end");
  const numbers = (i: number): number[] => (lines[end + i] ?? "").trim().split(/\s+/u).map(Number);
  const [wavelengthA, referencePerCm3, , normalisationCm] = numbers(1);
  const [count, order] = numbers(2);
  const [impactHwhmPerCm, impactShiftPerCm] = numbers(3);
  const values = lines
    .slice(end + 4)
    .join(" ")
    .trim()
    .split(/\s+/u)
    .map(Number);
  const header = [
    Number(temperature?.[1]),
    wavelengthA,
    referencePerCm3,
    normalisationCm,
    impactHwhmPerCm,
    impactShiftPerCm,
  ];
  if (
    temperature === null ||
    end < 0 ||
    wavelengthA === undefined ||
    referencePerCm3 === undefined ||
    normalisationCm === undefined ||
    count === undefined ||
    order === undefined ||
    impactHwhmPerCm === undefined ||
    impactShiftPerCm === undefined ||
    !header.every(Number.isFinite) ||
    !Number.isInteger(count) ||
    count <= 0 ||
    !Number.isInteger(order) ||
    order <= 0 ||
    values.length < count * (order + 1) ||
    !values.slice(0, count * (order + 1)).every(Number.isFinite)
  ) {
    throw new Error("an Allard et al. table is not as its README gives it");
  }
  const detuningsPerCm: number[] = [];
  const firstOrder: number[] = [];
  for (let k = 0; k < count; k += 1) {
    detuningsPerCm.push(values[k * (order + 1)] ?? Number.NaN);
    firstOrder.push(values[k * (order + 1) + 1] ?? Number.NaN);
  }
  if (detuningsPerCm.some((d, i) => !(d > (detuningsPerCm[i - 1] ?? -Infinity)))) {
    throw new Error("an Allard et al. table's detunings do not ascend");
  }
  return {
    temperatureK: Number(temperature[1]),
    wavelengthA,
    referencePerCm3,
    normalisationCm,
    impactHwhmPerCm,
    impactShiftPerCm,
    detuningsPerCm,
    firstOrder,
  };
}

/** A far-wing file: Δω, cm⁻¹, ascending, and σ at its density (10²¹ cm⁻³), cm². */
export interface AllardWing {
  readonly detuningsPerCm: ReadonlyArray<number>;
  readonly crossSectionsCm2: ReadonlyArray<number>;
}

/** A red-wing file (two columns, Δω and σ; the D1 README's item 1). */
export function parseAllardWing(text: string): AllardWing {
  const rows = text
    .split(/\r?\n/u)
    .map((l) => l.trim().split(/\s+/u).map(Number))
    .filter((r) => r.length === 2 && r.every(Number.isFinite));
  const sorted = rows.toSorted((a, b) => (a[0] ?? 0) - (b[0] ?? 0));
  return {
    detuningsPerCm: sorted.map((r) => r[0] ?? Number.NaN),
    crossSectionsCm2: sorted.map((r) => r[1] ?? Number.NaN),
  };
}

/** Linear interpolation on an ascending grid, or `undefined` outside it. */
function within(
  grid: ReadonlyArray<number>,
  values: ReadonlyArray<number>,
  x: number,
): number | undefined {
  const n = grid.length;
  if (n === 0 || x < (grid[0] ?? Infinity) || x > (grid[n - 1] ?? -Infinity)) {
    return undefined;
  }
  let i = 0;
  while (i < n - 2 && (grid[i + 1] ?? Infinity) < x) {
    i += 1;
  }
  const x0 = grid[i] ?? 0;
  const x1 = grid[i + 1] ?? x0;
  const y0 = values[i] ?? 0;
  const y1 = values[i + 1] ?? y0;
  return x1 > x0 ? y0 + ((y1 - y0) * (x - x0)) / (x1 - x0) : y0;
}

/**
 * Allard et al.'s one-perturber wing at a detuning, as the normalised profile per perturber,
 * (cm⁻¹)⁻¹ per cm⁻³: c₁(Δω) ÷ n_ref within the table, c₁ linear between its detunings and 0 where
 * below 0; on the red side beyond it the wing file's σ ÷ (π r₀ f n_ref), ln σ linear between its
 * points; 0 elsewhere. Times a line's strength per atom, cm, it is the pair's σ, cm⁵.
 */
export function allardProfilePerPerturber(
  table: AllardTable,
  wing: AllardWing | undefined,
  detuningPerCm: number,
): number {
  const c1 = within(table.detuningsPerCm, table.firstOrder, detuningPerCm);
  if (c1 !== undefined) {
    return Math.max(c1, 0) / table.referencePerCm3;
  }
  if (wing === undefined || detuningPerCm > (table.detuningsPerCm[0] ?? 0)) {
    return 0;
  }
  const lnSigma = within(
    wing.detuningsPerCm,
    wing.crossSectionsCm2.map((s) => Math.log(Math.max(s, 1e-300))),
    detuningPerCm,
  );
  return lnSigma === undefined
    ? 0
    : Math.exp(lnSigma) / (table.normalisationCm * table.referencePerCm3);
}

/** A line's far-wing pair cross-section, cm⁵ per atom of the stage, at a detuning, cm⁻¹. */
export type PairWing = (detuningPerCm: number) => number;

/** One line's wing for a pair's cell means: its centre and its σ. */
export interface WingLine {
  readonly centrePerCm: number;
  readonly wing: PairWing;
}

/** The cells' grid: 380–800 nm at 1 nm, the committed pair tables'. */
export const PAIR_GRID = { firstNm: 380, stepNm: 1, points: 421 } as const;

/** Sample points a 1 nm cell's mean takes. */
const POINTS_PER_CELL = 2_000;

/**
 * Each cell's mean pair σ over [λ − 0.5, λ + 0.5) nm, vacuum, m⁵: the lines' wings summed at
 * {@link POINTS_PER_CELL} equal steps of wavelength, each line's taken only more than
 * {@link LINE_CUTOFF_PER_CM} from its centre, where the atom's own table ends.
 */
export function pairCellMeansM5(lines: ReadonlyArray<WingLine>): Float64Array {
  return Float64Array.from({ length: PAIR_GRID.points }, (_, c) => {
    const lowNm = PAIR_GRID.firstNm + c * PAIR_GRID.stepNm - 0.5;
    let sum = 0;
    for (let j = 0; j < POINTS_PER_CELL; j += 1) {
      const nu = 1e7 / (lowNm + (j + 0.5) / POINTS_PER_CELL);
      for (const line of lines) {
        const d = nu - line.centrePerCm;
        if (Math.abs(d) > LINE_CUTOFF_PER_CM) {
          sum += line.wing(d);
        }
      }
    }
    // cm⁵ to m⁵.
    return (sum / POINTS_PER_CELL) * 1e-10;
  });
}

/** A line of a set by its index, or an error naming the broken invariant. */
function lineOf(set: AtomicLineSet, index: number): AtomicLineSet["lines"][number] {
  const line = set.lines[index];
  if (line === undefined) {
    throw new Error(`no line ${index} in ${set.stage.key}'s set`);
  }
  return line;
}

/**
 * A line's impact (Lorentz) far wing per pair at a temperature, cm⁵ per atom of the stage:
 * S γ_p ÷ (π Δ²), γ_p its half width per perturber, cm⁻¹ cm³, from the atoms' widths.
 */
export function impactWing(
  set: AtomicLineSet,
  index: number,
  perturber: "H2" | "He",
  temperatureK: number,
): PairWing {
  const line = lineOf(set, index);
  const strengthCm = atomicLineStrengthCm(line, temperatureK, set.partitionFunction(temperatureK));
  // The FWHM in angular frequency per m⁻³ as a half width in wavenumber per cm⁻³.
  const halfWidthPerCm =
    (vanDerWaalsWidthM3PerS(set, index, perturber, temperatureK) * CM3_PER_M3) /
    (4 * Math.PI * SPEED_OF_LIGHT_M_PER_S * 100);
  return (d) => (strengthCm * halfWidthPerCm) / (Math.PI * d * d);
}

/**
 * Allard et al.'s wing for a line of the set, per atom of the stage at a temperature, cm⁵: the
 * line's strength S times their normalised one-perturber profile ({@link allardProfilePerPerturber}),
 * their σ = π r₀ f I being per atom in the lower level, whose share S ÷ (π r_e f) takes.
 */
export function allardWing(
  set: AtomicLineSet,
  index: number,
  table: AllardTable,
  wing: AllardWing | undefined,
  temperatureK: number,
): PairWing {
  const line = lineOf(set, index);
  const strengthCm = atomicLineStrengthCm(line, temperatureK, set.partitionFunction(temperatureK));
  return (d) => strengthCm * allardProfilePerPerturber(table, wing, d);
}

/** The Allard et al. files R08.T4.e reads, by path in the deposit's tables, with their SHA-256. */
export const ALLARD_TABLES: Readonly<Record<string, string>> = {
  "TABLES_D1_NaH2_2017/README_D1_T1000/redwing_NaH2_D1_21_1000.omg":
    "5367233e1eb0a671ac735892dc11ed2c414efdb2f1a7949aa7b053fc785fb1cd",
  "TABLES_D1_NaH2_2017/README_D1_T1000/tableD1_NaH2_1000_1e21_FS17_nearwing.omg":
    "b1786f1e62f2b9a2a4348355b49d170ec7b52234b8a291bfa5440a29c3739f57",
  "TABLES_D1_NaH2_2017/T1500/redwing_NaH2_D1_21_1500.omg":
    "b9ddb68f05093814c987a7859f6227f4d62168175276d978f15d01fa17af6e77",
  "TABLES_D1_NaH2_2017/T1500/table_nearwing_1500.omg":
    "741effee253cd998a1bc1d44afe261e2367ea0295c0b076dd0064a4cb441198a",
  "TABLES_D1_NaH2_2017/T2000/redwing_NaH2_D1_21_2000.omg":
    "0ea592def54565859408b52d8e3c0faf8a6eed75926a2bea4ecbff2ce36352c7",
  "TABLES_D1_NaH2_2017/T2000/table_nearwing_2000.omg":
    "594ab01b75b1a286c724f2cd6c2813e4a184b8fb4a676d2228da7fde67f64812",
  "TABLES_D1_NaH2_2017/T2500/redwing_NaH2_D1_21_2500.omg":
    "f7279222aa1d320c450f46f5cd35094ef035364f3e82bb6293dc602c84d8b33e",
  "TABLES_D1_NaH2_2017/T2500/table_nearwing_2500.omg":
    "c9430610ed8171769cff39b81082bdf11e09bec2e6c77e370f7c2e4c9515d70b",
  "TABLES_D1_NaH2_2017/T3000/redwing_NaH2_D1_21_3000.omg":
    "6aae30739a86cfad7872099f633d807bbb80afd1224539c19d6220b6abecb991",
  "TABLES_D1_NaH2_2017/T3000/table_nearwing_3000.omg":
    "351795c8c3e04f56d424b23e1f047d0c1d7a2307afa21159e1baac912bce31e5",
  "TABLES_D1_NaH2_2017/T500/redwing_NaH2_D1_21_500.omg":
    "2fd2f3c96ae14d75817c7a353cc034e7446b3878347eca00bdce63ac67960e98",
  "TABLES_D1_NaH2_2017/T500/table_nearwing_500.omg":
    "a5f6057031f083806e77729ee13108f3463cf24f2bf372fb794811672f227fb6",
  "TABLES_D1_NaH2_2017/T600/redwing_NaH2_D1_21_600.omg":
    "0f656ec1febdf0a7ef0170c15590f3cb1f412a088114711e7fd697a7793782ac",
  "TABLES_D1_NaH2_2017/T600/table_nearwing_600.omg":
    "93fd09da93ca47b551fcd750af05fd888ffc0b9b5642cbf6392844a85a47502a",
  "TABLES_D1_NaH2_2017/T725/redwing_NaH2_D1_21_725.omg":
    "05e0c7a35afaa49e96a3409515587265027cf53eb57c31faa068ed706084502f",
  "TABLES_D1_NaH2_2017/T725/table_nearwing_725.omg":
    "9a88635b546242c834ec95d87dd1fa70fc0d67c595b216bf0b24ba7d3b62dfbb",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_1000_1e21_FS17.omg":
    "02dd81ed422104362c87617ca48aebf20f6ddaae9a5318d6c2dcb05f03ae404c",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_1500_1e21_FS17.omg":
    "bf2589adeca645aeff863a63f20502f8a7a9bb4e213fb45665fb820b90bfbd33",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_2000_1e21_FS17.omg":
    "dace79e18f164c01ea177dc02fbe663bb3db72256a491c4c7ea4f27b903225e1",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_2500_1e21_FS17.omg":
    "821a52a9c1f3c8e37e4f5f9f46f4cb00421a7dffe30f1af62d345b0c2f8fea21",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_3000_1e21_FS17.omg":
    "4d8b0f7dbcb8748babc593c6cbcc9cde7a9d7f247b0150f1ff4cb3eb190a07d4",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_500_1e21_FS17.omg":
    "a32a299d0658f188efd6c3005f7ec0d1df8976ab55b7746d2c96fa1855895d45",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_600_1e21_FS17.omg":
    "3cd38bdee8a5d6d8359d1fc2295e25fe97e806d11ede3e217e6757dcf82ce49a",
  "TABLES_D2_NaH2_2017/tableD2_NaH2_725_1e21_FS17.omg":
    "8f246f12cd5a2a2acc25d2b0c8f87449b5b4be395e94ddc71d22e44104774a9e",
};

/** Allard et al.'s tables' temperatures, K: those of their widths ({@link NA_D_H2_WIDTHS}). */
export const ALLARD_TEMPERATURES_K = NA_D_H2_WIDTHS.temperaturesK;

/** D2's table, D1's near-wing table and D1's red wing at one of their temperatures, checked. */
export function readAllardTables(
  dir: string,
  temperatureK: (typeof ALLARD_TEMPERATURES_K)[number],
): { readonly d2: AllardTable; readonly d1: AllardTable; readonly d1Wing: AllardWing } {
  const read = (path: string): string => {
    const sha256 = ALLARD_TABLES[path];
    if (sha256 === undefined) {
      throw new Error(`no recorded Allard et al. table ${path}`);
    }
    return readCheckedText(join(dir, path), sha256);
  };
  const d1Dir =
    temperatureK === 1_000
      ? "TABLES_D1_NaH2_2017/README_D1_T1000"
      : `TABLES_D1_NaH2_2017/T${temperatureK}`;
  const d1Name =
    temperatureK === 1_000
      ? "tableD1_NaH2_1000_1e21_FS17_nearwing.omg"
      : `table_nearwing_${temperatureK}.omg`;
  return {
    d2: parseAllardTable(read(`TABLES_D2_NaH2_2017/tableD2_NaH2_${temperatureK}_1e21_FS17.omg`)),
    d1: parseAllardTable(read(`${d1Dir}/${d1Name}`)),
    d1Wing: parseAllardWing(read(`${d1Dir}/redwing_NaH2_D1_21_${temperatureK}.omg`)),
  };
}

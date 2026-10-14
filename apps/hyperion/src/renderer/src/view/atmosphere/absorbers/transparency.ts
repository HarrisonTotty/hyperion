/**
 * What the client states about absorbing species it draws nothing for over the bake range
 * (plan R08, R08.T4.c; `decision-r08-t4b-t12c.md`, follow-ups 2 to 4): the species no column can
 * make visible, each with its reason and evidence, and those whose absorption a later task draws,
 * each with its 1 nm cell-mean cross-sections.
 *
 * @remarks
 * Every row the substance registry flags `absorber` has cross-sections, a committed line table
 * (R08.T4.d's and R08.T4.g's k-distributions, `kDistribution.ts`), an entry of
 * {@link PENDING_ABSORBERS} with its cell means while R08.T4.e and T4.f are unbuilt, or an entry of
 * {@link NO_VISIBLE_ABSORPTION} (R08.T19 holds the client to it). A species leaves R08.T4.d's and
 * R08.T4.g's pending lists when its table is committed.
 * - A transparent species takes no column and no label.
 * - A pending one draws nothing and is labelled `ATMOSPHERE: APPROXIMATE` meanwhile, under Design
 *   note 12's τ rule, as any species without cross-sections is, unless R08.T10.a finds it
 *   negligible on the body (follow-up 4, ruling 3). For each sun and channel it bounds the change
 *   of the channel's light, Δ_c = Σ_cells |c̄_c| S min(1, k⟨σ⟩_cell u_max) ÷ Σ_cells c̄_c S, with
 *   k = 1 for measured values and 100 for estimated ones, and the species is negligible where
 *   Δ_c ≤ 10⁻³ in every channel. T4.c records the cell means; T10.a applies them.
 *
 * The values are reduced, with their citations: HITRAN's, Kurucz's and the overtone-ladder
 * estimates in `bounds.json`, and ExoMol's, adapted material under CC BY-SA 4.0, in
 * `exomol/bounds.json` with its licence note (decision-composition §5).
 */

import { BAKE_RANGE_NM } from "../medium";
import bounds from "./bounds.json" with { type: "json" };
import exomolBounds from "./exomol/bounds.json" with { type: "json" };

/**
 * Why a species has nothing to draw over 380–760 nm (`decision-r08-t4b-t12c.md`, follow-ups 2 and
 * 3).
 *
 * - `bandsOutsideRange`: its catalogued bands and lines lie outside the range, or arise from
 *   levels no profile below its stated temperature populates.
 * - `underOvertoneBound`: its strongest catalogued band in or nearest the range, extrapolated up the
 *   overtone ladder at a factor of at least 10 a quantum, stays under τ 10⁻³ in every 1 nm cell at
 *   10³ km-amagat. No species holds it as built (follow-up 3); it stays for later rows.
 * - `noBoundStates`: a free particle, with no lines; its optics are other terms' (e⁻'s Thomson
 *   scattering and its free–free pairs).
 */
export type TransparencyReason = "bandsOutsideRange" | "underOvertoneBound" | "noBoundStates";

/** A species stated to have nothing to draw over the bake range, with its evidence. */
export interface TransparentSpecies {
  /** The registry key. */
  readonly species: string;
  /** Why it has nothing to draw. */
  readonly reason: TransparencyReason;
  /**
   * The profile temperature, K, below which the statement holds, or `undefined` at every
   * temperature. Above it the species is pending ({@link PENDING_ABSORBERS}).
   */
  readonly belowK: number | undefined;
  /** The bands, their catalogued coverage and the check's figures, each with its source. */
  readonly evidence: string;
}

/** The plan task that draws a pending species' absorption. */
export type PendingTask = "R08.T4.d" | "R08.T4.e" | "R08.T4.f" | "R08.T4.g";

/** Whether values rest on measured data or on a stated estimate (follow-up 3, ruling 4). */
export type BoundBasis = "measured" | "estimated";

/**
 * The cells a pending absorber's cross-sections are given in, nm: 1 nm each, [λ − 0.5, λ + 0.5) on
 * vacuum wavelengths, centred on every whole nm of the bake range, {@link BAKE_RANGE_NM}.
 */
export const PENDING_CELLS_NM: readonly [number, number] = BAKE_RANGE_NM;

/** The number of {@link PENDING_CELLS_NM}'s cells: 381. */
export const PENDING_CELL_COUNT = PENDING_CELLS_NM[1] - PENDING_CELLS_NM[0] + 1;

/** What a species' and a pair's pending cross-sections share. */
export interface AbsorberBoundBase {
  /** The registry key, or a pair's key (its members' joined by `+`). */
  readonly key: string;
  /** The task that draws its absorption. */
  readonly task: PendingTask;
  /**
   * The part of the key's absorption it bounds, where one task holds two entries for a key (CO's
   * overtones from 8–0, beside its hot list in R08.T4.f), or `undefined`.
   */
  readonly part: string | undefined;
  /** Whether the values are measured or a stated estimate, which takes the hundredfold margin. */
  readonly basis: BoundBasis;
  /** The source's temperature nodes, K, ascending. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** The data's citation: its paper, file, checksum and the date fetched. */
  readonly source: string;
  /** How the values were reduced. */
  readonly method: string;
}

/** A species' cell means, m² a molecule. */
export interface SpeciesBound extends AbsorberBoundBase {
  readonly kind: "species";
  /** `cellMeansM2[t][c]`: at node t, the mean over the cell centred on 380 + c nm. */
  readonly cellMeansM2: ReadonlyArray<Float64Array>;
}

/** A pair's cell means, m⁵ a molecule pair. */
export interface PairBound extends AbsorberBoundBase {
  readonly kind: "pair";
  /** `cellMeansM5[t][c]`: at node t, the mean over the cell centred on 380 + c nm. */
  readonly cellMeansM5: ReadonlyArray<Float64Array>;
}

/**
 * A pending absorber's 1 nm cell-mean cross-sections over {@link PENDING_CELLS_NM} at the
 * temperature nodes of its source (`decision-r08-t4b-t12c.md`, follow-up 4, ruling 3), rounded up
 * to 3 significant figures so that each stays a bound. R08.T10.a bounds each channel's change by
 * them ({@link PENDING_ABSORBERS}); between the nodes it takes ln σ linear in 1/T, as a lower
 * level's Boltzmann factor goes (linear in T it would fall below a line's value: 5.6 times for Mg⁺
 * at 2,500 K between its 2,000 and 3,000 K nodes), and past them as `estimated`.
 */
export type AbsorberBound = SpeciesBound | PairBound;

/** A bounds file's entries, as its JSON import reads them. */
export interface AbsorberBoundsFile {
  readonly bounds: ReadonlyArray<{
    readonly key: string;
    readonly kind: string;
    readonly task: string;
    readonly part?: string;
    readonly basis: string;
    readonly temperaturesK: ReadonlyArray<number>;
    readonly cellMeansM2?: ReadonlyArray<ReadonlyArray<number>>;
    readonly cellMeansM5?: ReadonlyArray<ReadonlyArray<number>>;
    readonly source: string;
    readonly method: string;
  }>;
}

/** Whether a file's string is a {@link PendingTask}. */
function isPendingTask(value: string): value is PendingTask {
  return (
    value === "R08.T4.d" || value === "R08.T4.e" || value === "R08.T4.f" || value === "R08.T4.g"
  );
}

/** Whether a file's string is a {@link BoundBasis}. */
function isBoundBasis(value: string): value is BoundBasis {
  return value === "measured" || value === "estimated";
}

/**
 * A bounds file's entries, checked: a known kind, task and basis; temperatures finite, above 0 K
 * and ascending; at each node {@link PENDING_CELL_COUNT} cell means, finite, at least 0 and one
 * above 0; a key, task and part listed once.
 *
 * @throws Error naming the entry and the check it fails.
 */
export function readAbsorberBounds(
  file: AbsorberBoundsFile,
  where: string,
): ReadonlyArray<AbsorberBound> {
  const seen = new Set<string>();
  return file.bounds.map((entry) => {
    const identity = `${entry.key} (${entry.task}${entry.part === undefined ? "" : `, ${entry.part}`})`;
    const label = `${where}'s ${identity}`;
    if (seen.has(identity)) {
      throw new Error(`${label} is listed twice`);
    }
    seen.add(identity);
    const { task, basis, temperaturesK } = entry;
    if (!isPendingTask(task) || !isBoundBasis(basis)) {
      throw new Error(`${label}'s task or basis is not one of the plan's`);
    }
    if (
      temperaturesK.length === 0 ||
      temperaturesK.some((t, i) => !(Number.isFinite(t) && t > (temperaturesK[i - 1] ?? 0)))
    ) {
      throw new Error(`${label}'s temperatures do not ascend from above 0 K`);
    }
    const rows = entry.kind === "pair" ? entry.cellMeansM5 : entry.cellMeansM2;
    if (
      rows === undefined ||
      rows.length !== temperaturesK.length ||
      !rows.every(
        (row) =>
          row.length === PENDING_CELL_COUNT &&
          row.every((s) => Number.isFinite(s) && s >= 0) &&
          row.some((s) => s > 0),
      )
    ) {
      throw new Error(
        `${label} needs ${PENDING_CELL_COUNT} cell means at each node, finite, at least 0 and one above 0`,
      );
    }
    const base = {
      key: entry.key,
      task,
      part: entry.part,
      basis,
      temperaturesK,
      source: entry.source,
      method: entry.method,
    };
    const cells = rows.map((row) => Float64Array.from(row));
    switch (entry.kind) {
      case "species":
        return { ...base, kind: "species", cellMeansM2: cells };
      case "pair":
        return { ...base, kind: "pair", cellMeansM5: cells };
      default:
        throw new Error(`${label}'s kind ${entry.kind} is neither species nor pair`);
    }
  });
}

/**
 * Every pending absorber's cell means: `bounds.json`'s (HITRAN2024, Kurucz's line lists and the
 * stated estimates) and `exomol/bounds.json`'s (ExoMol, CC BY-SA 4.0).
 */
export const ABSORBER_BOUNDS: ReadonlyArray<AbsorberBound> = [
  ...readAbsorberBounds(bounds, "bounds.json"),
  ...readAbsorberBounds(exomolBounds, "exomol/bounds.json"),
];

/**
 * The species with no visible absorption to draw (`decision-r08-t4b-t12c.md`, follow-up 3,
 * ruling 1, and follow-up 4, ruling 2). Follow-up 2's other expected entries (CO, N₂O, HCN, H₂S,
 * SiO, O, C₂H₂, C₂H₄, C₂H₆, PH₃, CH₃OH) all have catalogued bands or lines in the range above the
 * bound, or no catalogued data and an overtone ladder above it, and have R08.T4.d's line tables or
 * are {@link PENDING_ABSORBERS}. So does CO₂, whose HITRAN2024 lines reach 565–737 nm (follow-up
 * 4's check).
 */
export const NO_VISIBLE_ABSORPTION: ReadonlyArray<TransparentSpecies> = [
  {
    species: "He",
    reason: "bandsOutsideRange",
    belowK: 5_000,
    evidence:
      "Every line from He I's ground state lies in the extreme ultraviolet (1s2 1S - 1s2p 1P at " +
      "58.43 nm, the forbidden 1s2s 3S at 62.6 nm); its lines in the range (587.7, 668.0, 389.0 " +
      "and 447.3 nm, vacuum) arise from n = 2 levels 19.8-21.2 eV up (NIST ASD v5.12, Kramida " +
      "et al. 2024, DOI 10.18434/T4W30F, fetched 2026-10-10). In LTE, from Kurucz's He I list " +
      "(gf0200.all), their largest 1 nm cell reaches tau 1.7e-12 at 4000 K and 3.3e-7 at 5000 K " +
      "at 1e3 km-amagat, and 1e-3 only near 5980 K (He I 587.7 nm), so the statement holds below " +
      "5000 K; a non-LTE metastable 2 3S population (389.0 nm) is not modelled.",
  },
  {
    species: "N2",
    reason: "bandsOutsideRange",
    belowK: 1_800,
    evidence:
      "N2 has no electric-dipole vibration-rotation spectrum. HITRAN2024's electric-quadrupole " +
      "lines (14N2 and 14N15N) end with the 4-0 band at 9354 cm-1 (1069 nm; 2.3e-31 cm/molecule " +
      "at 296 K, the bands falling 13-53 times a quantum), and nothing is catalogued over " +
      "13100-26400 cm-1 (https://hitran.org/lbl/api, n2-0-13100.par SHA-256 " +
      "34963c3f98ee0b553a3d8b9efcbc8a44cc1950a0d1d66ee7277690cebd30c79f, fetched 2026-10-11). " +
      "The first band in range, 6-0 near 13550 cm-1 (738 nm), at a tenth a quantum and wholly in " +
      "one 1 nm cell, is at most 1.3e-38 m2, tau 3.4e-7 at 1e3 km-amagat. Its electronic systems " +
      "from the ground state lie below 202 nm: the spin-forbidden Vegard-Kaplan A-X from 201 nm, " +
      "Lyman-Birge-Hopfield a-X from 145 nm, and the dipole-allowed b-X and higher from about " +
      "100 nm (A. Lofthus and P. H. Krupenie, J. Phys. Chem. Ref. Data 6 (1977) 113, DOI " +
      "10.1063/1.555546). The first positive (B-A) bands absorb in the range from the metastable " +
      "A state, 6.17 eV up: in LTE a band-model estimate reaches tau 2.5e-6 at 1800 K and 1.4e-4 " +
      "at 2000 K in the 760 nm cell at 1e3 km-amagat, so the statement holds below 1800 K; a " +
      "non-LTE A-state population (an aurora's) is not modelled.",
  },
  {
    species: "e-",
    reason: "noBoundStates",
    belowK: undefined,
    evidence:
      "A free electron has no bound states and no lines. Its optics are other terms': Thomson " +
      "scattering (absorbers.ts's thomsonTerm) and free-free absorption in its pairs, H- (H+e-) " +
      "and He- (He+e-) in closed form and H2- (H2+e-) pending. Its free-free absorption with " +
      "ions (H+, Na+, Ca+ and others), tens of times H-'s free-free a pair at 600 nm and 5000 K " +
      "by Kramers' hydrogenic estimate, matters only in ultra-hot atmospheres and is not " +
      "modelled: the registry pairs no ion with e-.",
  },
];

/** A pending absorber before its cell means are attached. */
interface PendingListing {
  /** The registry key, or a pair's key. */
  readonly key: string;
  /** The task that draws its absorption. */
  readonly task: PendingTask;
  /** The part of the key's absorption, as its bound's ({@link AbsorberBoundBase.part}). */
  readonly part?: string;
  /** What the task draws. */
  readonly what: string;
}

/**
 * The species and pairs whose absorption over the bake range R08.T4.d–g draw
 * (`decision-r08-t4b-t12c.md`, follow-ups 2 to 5). The ions are listed for when P14.T49.b adds
 * their rows. A resonance line's far wing, which grows with its perturbers' density, is a pair of
 * the atom and each perturber (`K+H2`, `K+He`; follow-up 5, ruling 2), beside the atom's own
 * entry per atom.
 */
const PENDING: ReadonlyArray<PendingListing> = [
  { key: "Na", task: "R08.T4.e", what: "Na D (589.0 and 589.6 nm), pressure-broadened" },
  { key: "K", task: "R08.T4.e", what: "K I's lines in the range, per atom (404.5 and 404.8 nm)" },
  {
    key: "K+H2",
    task: "R08.T4.e",
    what: "the far blue wing of K I 766.7 and 770.1 nm per (K, H2) pair, from Allard et al.",
  },
  {
    key: "K+He",
    task: "R08.T4.e",
    what: "the far blue wing of K I 766.7 and 770.1 nm per (K, He) pair, from Allard et al.",
  },
  { key: "Ca", task: "R08.T4.e", what: "Ca I 422.7 nm" },
  { key: "Cr", task: "R08.T4.e", what: "Cr I 425.4, 427.5 and 429.0 nm" },
  { key: "Si", task: "R08.T4.e", what: "Si I 390.6 nm" },
  { key: "Mg", task: "R08.T4.e", what: "Mg I's b triplet (516.7, 517.3 and 518.4 nm)" },
  { key: "Ti", task: "R08.T4.e", what: "Ti I's lines in the range" },
  { key: "Fe", task: "R08.T4.e", what: "Fe I's lines in the range" },
  { key: "H", task: "R08.T4.e", what: "H I's Balmer lines in LTE, H-alpha 656.3 nm and up" },
  { key: "O", task: "R08.T4.e", what: "the forbidden [O I] 630.0 and 636.4 nm lines" },
  { key: "He", task: "R08.T4.e", what: "He I's lines in the range, for profiles above 5000 K" },
  { key: "Ca+", task: "R08.T4.e", what: "Ca II's H and K lines (393.4 and 396.8 nm)" },
  { key: "Fe+", task: "R08.T4.e", what: "Fe II's lines in the range" },
  { key: "Ti+", task: "R08.T4.e", what: "Ti II's lines in the range" },
  {
    key: "Mg+",
    task: "R08.T4.e",
    what: "Mg II 448.1 nm, from 8.86 eV, which reaches tau 1e-3 at 1e3 km-amagat near 2570 K",
  },
  {
    key: "H2+e-",
    task: "R08.T4.e",
    what: "H2- free-free, from Bell 1980's table once a lawful copy is fetched",
  },
  { key: "TiO", task: "R08.T4.f", what: "its bands (ExoMol's Toto)" },
  { key: "VO", task: "R08.T4.f", what: "its bands (ExoMol's HyVO, which its service serves)" },
  { key: "FeH", task: "R08.T4.f", what: "its F-X bands below 760 nm (MoLLIST)" },
  { key: "H2S", task: "R08.T4.f", what: "its visible overtones (AYT2), from 296 K" },
  {
    key: "SiO",
    task: "R08.T4.f",
    what: "its A-X hot bands (SiOUVenIR), which reach the range above about 1000 K",
  },
  { key: "CO", task: "R08.T4.f", what: "hot CO (Li2015, its 8-0 bands and up dropped)" },
  {
    key: "CO",
    task: "R08.T4.f",
    part: "overtones",
    what: "the 8-0 to 13-0 bands, which HITRAN2024 lacks and Li2015 holds only to 11-0, dropped, by the overtone ladder below 7-0 (estimated)",
  },
  { key: "N2O", task: "R08.T4.f", what: "hot N2O (TYM)" },
  {
    key: "NH3",
    task: "R08.T4.f",
    what: "hot NH3 above C2018's 300 K, to which R08.T4.g's table is held (ExoMol's CoYuTe)",
  },
  {
    key: "CO2",
    task: "R08.T4.f",
    what: "hot CO2 above HITRAN's 500 K, to which R08.T4.d's table is held (ExoMol's UCL-4000 considered)",
  },
  { key: "HCN", task: "R08.T4.f", what: "hot HCN (Harris)" },
  { key: "H2O", task: "R08.T4.f", what: "hot H2O (POKAZATEL)" },
  {
    key: "N2",
    task: "R08.T4.f",
    what: "the first positive (B-A) bands from the metastable A state, above 1800 K (estimated)",
  },
];

/** A pending absorber: its key, the task that draws it, what it draws, and its cell means. */
export interface PendingAbsorber extends PendingListing {
  /** Its 1 nm cell-mean cross-sections at its source's nodes. */
  readonly bound: AbsorberBound;
}

/**
 * The species and pairs whose absorption over the bake range R08.T4.d–g draw, each with its 1 nm
 * cell-mean cross-sections (`decision-r08-t4b-t12c.md`, follow-up 3, ruling 2, and follow-up 4,
 * ruling 3). A species may be listed under two tasks (a temperate list in T4.d and a hot one in
 * T4.f), each with its own values, or twice under one task by part (CO's `overtones`, from 8–0,
 * beside its hot list in T4.f).
 *
 * @throws Error at load for a listing whose values are in neither bounds file, a broken invariant
 *   of the committed data.
 */
export const PENDING_ABSORBERS: ReadonlyArray<PendingAbsorber> = PENDING.map((listing) => {
  const bound = ABSORBER_BOUNDS.find(
    (b) => b.key === listing.key && b.task === listing.task && b.part === listing.part,
  );
  if (bound === undefined) {
    const which = listing.part === undefined ? listing.task : `${listing.task}, ${listing.part}`;
    throw new Error(`pending absorber ${listing.key} (${which}) has no cell means`);
  }
  const { key, task, part, what } = listing;
  return part === undefined ? { key, task, what, bound } : { key, task, part, what, bound };
});

/**
 * Whether the client states a species has no visible absorption to draw in a profile at a
 * temperature, K.
 */
export function isTransparent(species: string, temperatureK: number): boolean {
  const entry = NO_VISIBLE_ABSORPTION.find((e) => e.species === species);
  return entry !== undefined && (entry.belowK === undefined || temperatureK < entry.belowK);
}

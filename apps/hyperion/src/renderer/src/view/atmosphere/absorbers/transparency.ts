/**
 * What the client states about absorbing species it draws nothing for over the bake range
 * (plan R08, R08.T4.c; `decision-r08-t4b-t12c.md`, follow-ups 2 to 4): the species no column can
 * make visible, each with its reason and evidence, and those whose absorption a later task draws,
 * each with its 1 nm cell-mean cross-sections.
 *
 * @remarks
 * Every row the substance registry flags `absorber` has cross-sections, a committed line table
 * (R08.T4.d's, R08.T4.e's and R08.T4.g's k-distributions, `kDistribution.ts`), an entry of
 * {@link PENDING_ABSORBERS} with its cell means for what a later task, or a deferral, leaves, or an
 * entry of {@link NO_VISIBLE_ABSORPTION} (R08.T19 holds the client to it). A species leaves
 * R08.T4.d's, R08.T4.e's and R08.T4.g's pending lists when its table is committed.
 * - A transparent species takes no column and no label.
 * - A pending one draws nothing and is labelled `ATMOSPHERE: APPROXIMATE` meanwhile, under Design
 *   note 12's τ rule, as any species without cross-sections is, unless R08.T10.a finds it
 *   negligible on the body (follow-up 4, ruling 3). For each sun and channel it bounds the change
 *   of the channel's light, Δ_c = Σ_cells |c̄_c| S min(1, k⟨σ⟩_cell u_max) ÷ Σ_cells c̄_c S, with
 *   k = 1 for measured values and 100 for estimated ones, and the species is negligible where
 *   Δ_c ≤ 10⁻³ in every channel. T4.c records the cell means; T10.a applies them.
 *
 * The values are reduced, with their citations: HITRAN's, Kurucz's and the overtone-ladder
 * estimates in `bounds.json`; the atoms and ions R08.T4.h adds, from Kurucz's lists and, for Zn⁺,
 * NIST ASD's Zn II lines, in `atomicBounds.json` (written by `src/tools/atomicLinesCli.ts`'s
 * `atomic-bounds`), so that neither file nears the 500 kB hook; and ExoMol's, adapted material
 * under CC BY-SA 4.0, in `exomol/bounds.json` with its licence note (decision-composition §5).
 */

import { BAKE_RANGE_NM } from "../medium";
import atomicBounds from "./atomicBounds.json" with { type: "json" };
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
 * - `underEquivalentWidthBound`: an atom or ion whose catalogued lines in the range take under
 *   10⁻³ of every bake bin's light, a summed equivalent width under 0.0253 nm in each 25.33 nm bin,
 *   at its reference column (R08.T4.h, the third reason; science-r08-unowned-absorbers §2.2): the
 *   stage's vertical column above 0.1 bar at g = 10 m s⁻², doubled for the mean slant path, at
 *   solar abundances with every atom of the element free, its stages in Saha equilibrium, taken at
 *   2,000–5,000 K (`src/tools/atomicLines.ts`'s `referenceColumnPerM2` and `stageBinShares`).
 */
export type TransparencyReason =
  "bandsOutsideRange" | "underOvertoneBound" | "noBoundStates" | "underEquivalentWidthBound";

/** A species stated to have nothing to draw over the bake range, with its evidence. */
export interface TransparentSpecies {
  /** The registry key. */
  readonly species: string;
  /** Why it has nothing to draw. */
  readonly reason: TransparencyReason;
  /**
   * The profile temperature, K, below which the statement holds, or `undefined` at every
   * temperature (the `belowK` contract, ruled 2026-10-19). Below it the species is transparent,
   * with no column and no label, whatever data the client holds for it (`absorbers.ts`'s
   * `absorberSpecs` consults it before any table). At and above it the key has exactly one answer:
   * a committed table or cross-sections whose first temperature node is at or below it (a
   * visibility floor's, R08.T4.e), or a pending entry whose first node is at or above it
   * ({@link PENDING_ABSORBERS}; He's form).
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
 * stated estimates), `atomicBounds.json`'s (R08.T4.h's atoms and ions: Kurucz's line lists and,
 * for Zn⁺, NIST ASD's Zn II lines) and `exomol/bounds.json`'s (ExoMol, CC BY-SA 4.0).
 */
export const ABSORBER_BOUNDS: ReadonlyArray<AbsorberBound> = [
  ...readAbsorberBounds(bounds, "bounds.json"),
  ...readAbsorberBounds(atomicBounds, "atomicBounds.json"),
  ...readAbsorberBounds(exomolBounds, "exomol/bounds.json"),
];

/**
 * How the third reason's shares are taken (R08.T4.h; science-r08-unowned-absorbers §2.2, ruled
 * 2026-10-19), which each such statement's evidence cites.
 */
const EQUIVALENT_WIDTH_METHOD =
  "the third reason's sums (src/tools/atomicLinesCli.ts's atomic-shares: each bake bin's share " +
  "of the light the lines' Voigt equivalent widths take, Humlicek 1982, with Doppler broadening " +
  "and 1 km/s of microturbulence, classical damping and twice Unsold's van der Waals width, over " +
  "every line in 360-780 nm, vacuum, from a level below the ionisation limit, summed by bin over " +
  "380-760 nm, at the stage's reference column: its vertical column above 0.1 bar at g = 10 m/s2, " +
  "doubled for the mean slant path, at Asplund et al. 2009's solar abundances with every atom of " +
  "the element free and its stages split by a Saha balance at 0.1 bar with Barklem and Collet " +
  "2016's partition functions, ionisation energies and H2 dissociation; at 2000, 2500, 3000, " +
  "3500, 4000, 4600 and 5000 K)";

/**
 * The species with no visible absorption to draw (`decision-r08-t4b-t12c.md`, follow-up 3,
 * ruling 1, and follow-up 4, ruling 2). Follow-up 2's other expected entries (CO, N₂O, HCN, H₂S,
 * SiO, O, C₂H₂, C₂H₄, C₂H₆, PH₃, CH₃OH) all have catalogued bands or lines in the range above the
 * bound, or no catalogued data and an overtone ladder above it, and have R08.T4.d's line tables or
 * are {@link PENDING_ABSORBERS}. So does CO₂, whose HITRAN2024 lines reach 565–737 nm (follow-up
 * 4's check).
 *
 * @remarks
 * R08.T4.h states seven ions none by the third reason (`underEquivalentWidthBound`), each under
 * 10⁻³ of every bin's light through 4,600 K, and below 5,000 K where it stays under at 5,000 K
 * too; above its limit each is pending in R08.T4.e (`atomicBounds.json`), as He is. Mg⁺ is among
 * them: on Kurucz's full list its 448.1 nm lines take 7.2 × 10⁻⁴ at 4,600 K. R08.T4.e draws Mg⁺
 * and Si⁺ from their limit by its tables (the `belowK` contract's table branch), defers Ni⁺, and
 * adds O, stated none to 5,000 K in Na⁺'s form (decision-r08-t4e-budget §7), and H and Zn, each
 * stated none below its visibility floor, 3,300 K on the 10% ladder, from which its table draws
 * it (the floors of the deferred Ca⁺, Ti⁺ and Fe⁺ are recorded in the plan, their entries kept).
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
  {
    species: "Na+",
    reason: "underEquivalentWidthBound",
    belowK: 5_000,
    evidence:
      "Every Na II line in the range arises 32.9 eV up or more. On R. L. Kurucz's gf1101.all (8 lines " +
      "in 360-780 nm; Kurucz 2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share of 9.4e-35 at 4600 K and 7.8e-32 at 5000 K, in the 405-431 nm bin " +
      "(Na II 408.87 nm, from 33.32 eV), against the science ruling's 0 from NIST ASD's lines. " +
      "Above 5000 K it is pending (R08.T4.e).",
  },
  {
    species: "Mg+",
    reason: "underEquivalentWidthBound",
    belowK: 4_600,
    evidence:
      "Mg II's lines in the range arise 8.65 eV up or more. On R. L. Kurucz's gf1201.all (103 lines " +
      "in 360-780 nm; Kurucz 2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share of 7.2e-4 at 4600 K, in the 431-456 nm bin (Mg II 448.24 and 448.26 nm, " +
      "3d 2D - 4f 2F, from 8.86 eV), against the science ruling's 7.1e-4 from NIST ASD's lines; " +
      "1.7e-3 at 5000 K, so the statement holds below 4600 K and Mg+ is drawn above it by " +
      "R08.T4.e's table (its nodes 4400, 4600, 4800 and 5000 K, the 4400 K node shaping its cubic).",
  },
  {
    species: "Al+",
    reason: "underEquivalentWidthBound",
    belowK: 5_000,
    evidence:
      "Al II's lines in the range arise 7.42 eV up or more. On R. L. Kurucz's gf1301.all (684 lines in " +
      "360-780 nm; Kurucz 2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share of 4.4e-5 at 4600 K and 1.6e-4 at 5000 K, in the 380-405 nm bin (Al II " +
      "390.18 nm, from 7.42 eV), against the science ruling's 4.8e-6 from NIST ASD's lines. Above " +
      "5000 K it is pending (R08.T4.e).",
  },
  {
    species: "Si+",
    reason: "underEquivalentWidthBound",
    belowK: 4_600,
    evidence:
      "Si II's lines in the range arise 6.86 eV up or more. On R. L. Kurucz's gf1401.all (225 lines in " +
      "360-780 nm, 50 of them from autoionising levels above the 16.35 eV limit, left out; Kurucz " +
      "2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share to 4600 K of 3.3e-4 at 4600 K, in the 380-405 nm bin (Si II 385.71 and " +
      "386.37 nm, from 6.86 eV), against the science ruling's 4.8e-4 from NIST ASD's lines; 1.3e-3 " +
      "at 5000 K (634.89 and 637.31 nm, from 8.12 eV), so the statement holds below 4600 K and Si+ " +
      "is drawn above it by R08.T4.e's table (its nodes 4400, 4600, 4800 and 5000 K, the 4400 K " +
      "node shaping its cubic).",
  },
  {
    species: "K+",
    reason: "underEquivalentWidthBound",
    belowK: 5_000,
    evidence:
      "Every K II line in the range arises 20.1 eV up or more. On R. L. Kurucz's gf1901.all (54 lines " +
      "in 360-780 nm; Kurucz 2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share of 2.3e-18 at 4600 K and 1.4e-16 at 5000 K, in the 405-431 nm bin (K II " +
      "418.73 nm, from 20.15 eV), against the science ruling's 0 from NIST ASD's lines. Above " +
      "5000 K it is pending (R08.T4.e).",
  },
  {
    species: "Ni+",
    reason: "underEquivalentWidthBound",
    belowK: 4_600,
    evidence:
      "Ni II's lines in the range arise 3.10 eV up or more. On R. L. Kurucz's gf2801.all (6560 lines in " +
      "360-780 nm, where NIST ASD's 370-800 nm held forbidden lines only; Kurucz 2011, Can. J. Phys. " +
      "89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share to 4600 K of 6.7e-4 at 4600 K, in the 405-431 nm bin, 58% of it Ni II " +
      "406.82 nm ((1G)4s 2G - (3F)4p 2F, log gf -1.29, from 4.03 eV, saturated), against the " +
      "science ruling's 1.5e-4; 1.5e-3 at 5000 K, so the statement holds below 4600 K and Ni+ is " +
      "pending above it (R08.T4.e, deferred past its byte budget: decision-r08-t4e-budget).",
  },
  {
    species: "Zn+",
    reason: "underEquivalentWidthBound",
    belowK: 5_000,
    evidence:
      "Kurucz's gf3001.all holds no Zn II line in the range. Its lines are the 17 NIST ASD v5.12 " +
      "lists over 350-800 nm (air), at Martin and Kaufman 1970's levels, with ASD's A for 7 (their " +
      "primary paper, Mayo et al. 2006, closed: a stated exception), Nandy and Sahoo 2015's computed " +
      "A for 5, Kisielius et al. 2015's computed line strength for 589.60 nm, and A = 3e8 s-1, above " +
      "every A ASD lists in the range, for the 4 that take no value from an open paper (the 6.12 eV " +
      "621.64 and 748.09 nm lines, whose upper level's lifetime has not been measured and whose " +
      "computed strengths, Kisielius et al.'s in a CC BY-NC catalogue and Kurucz's gf3001.pos, lie " +
      "3.4-4.0 dex below the bound; and 380.74 and 384.14 nm). On these, " +
      EQUIVALENT_WIDTH_METHOD +
      " give the bound's largest share, 1.5e-4 at 4600 K and 6.1e-4 at 5000 K, in the 735-760 nm " +
      "bin (Zn II 748.09 nm), against the science ruling's 0 from ASD's lines with a transition " +
      "probability; at Kurucz's computed gf it would be near 1e-6 at 5000 K. Above 5000 K it is " +
      "pending (R08.T4.e, estimated).",
  },
  {
    species: "O",
    reason: "underEquivalentWidthBound",
    belowK: 5_000,
    evidence:
      "O I's lines in the range are the forbidden [O I] 630.03 and 636.38 nm (air; 2p4 3P2,1 - " +
      "2p4 1D2, from 0 and 0.02 eV) and permitted lines from 9.1 eV up. On R. L. Kurucz's " +
      "gf0800.all (160 lines in 360-780 nm; Kurucz 2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      " give a largest share to 4600 K of 4.9e-4 at 4600 K and 5.8e-4 at 5000 K, in the 608-633 nm " +
      "bin ([O I] 630.03 nm, log gf -9.82, tau0 near 2.8 at the reference column, so that the Doppler " +
      "curve of growth keeps about half its linear width, main's check giving 4.8e-4), so the " +
      "statement holds below 5000 K (R08.T4.e; decision-r08-t4e-budget section 7). P. J. Storey " +
      "and C. J. Zeippen's A = 6.446e-3 s-1 for the line (Mon. Not. R. Astron. Soc. 312 (2000) 813, " +
      "Table 4, as main's ruling gives it, not re-read; log gf near -9.72, above Kurucz's) would " +
      "raise the share by about 10%, still under 1e-3: the statement stays on Kurucz's list. On " +
      "the flat part of its curve of growth its " +
      "share reaches about 1e-3 at 100 times the reference column (1.6e-3 from 1000 K). Above " +
      "5000 K it is pending (R08.T4.e).",
  },
  {
    species: "H",
    reason: "underEquivalentWidthBound",
    belowK: 3_300,
    evidence:
      "H I's lines in the range are the Balmer series, from n = 2, 10.2 eV up. On R. L. Kurucz's " +
      "gf0100.all (Kurucz 2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      ", taken instead at the nodes of R08.T4.e's 10% ladder (decision-r08-t4e-budget section 3) with " +
      "the free H atoms as the column (the H2-bound nuclei left out), give a largest share of 4.9e-5 " +
      "at 3300 K and 1.0e-3 at 3600 K, in the 633-659 nm bin (H-alpha, 656.28 nm air): H's " +
      "visibility floor is 3300 K, below which the statement holds, and from which R08.T4.e's " +
      "table draws it. At 100 times the reference column the share reaches 1e-3 from 3300 K (1.9e-3 " +
      "there). A non-LTE thermosphere is not modelled.",
  },
  {
    species: "Zn",
    reason: "underEquivalentWidthBound",
    belowK: 3_300,
    evidence:
      "Zn I's lines in the range arise 4.0 eV up or more. On R. L. Kurucz's gf3000.all (Kurucz " +
      "2011, Can. J. Phys. 89, 417), " +
      EQUIVALENT_WIDTH_METHOD +
      ", taken instead at the nodes of R08.T4.e's 10% ladder (decision-r08-t4e-budget section 3), " +
      "give a largest share of 8.2e-4 at 3300 K and 1.3e-3 at 3600 K, in the 456-481 nm bin (Zn I " +
      "481.05 nm air, 4s4p 3P2 - 4s5s 3S1, from 4.08 eV), against the science ruling's 3.1e-3 at " +
      "4600 K: Zn's visibility floor is 3300 K, below which the statement holds, and from which " +
      "R08.T4.e's table draws it. At 100 times the reference column the share reaches 1e-3 from " +
      "2750 K.",
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
 * (`decision-r08-t4b-t12c.md`, follow-ups 2 to 5). R08.T4.e's tables and the alkali pairs
 * (`absorbers/alkaliPairs.json`) replace the atoms' entries they draw. What stays for R08.T4.e: He
 * above 5,000 K; the stages it defers past its byte budget (decision-r08-t4e-budget §§4–5), each
 * with the entry it had, T4.c's in `bounds.json` or R08.T4.h's interim one in `atomicBounds.json`;
 * the stated nones above their limits (`atomicBounds.json`), O's among them; and H₂⁻'s free–free.
 */
const PENDING: ReadonlyArray<PendingListing> = [
  { key: "He", task: "R08.T4.e", what: "He I's lines in the range, for profiles above 5000 K" },
  // The stages R08.T4.e defers past its byte budget (decision-r08-t4e-budget §§4-5), each keeping
  // its entry: T4.c's in bounds.json, T4.h's interim ones in atomicBounds.json.
  {
    key: "Si",
    task: "R08.T4.e",
    what: "Si I 390.6 nm, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Ti",
    task: "R08.T4.e",
    what: "Ti I's lines in the range, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Ca",
    task: "R08.T4.e",
    what: "Ca I 422.7 nm, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Ca+",
    task: "R08.T4.e",
    what: "Ca II's H and K lines (393.4 and 396.8 nm), deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Fe+",
    task: "R08.T4.e",
    what: "Fe II's lines in the range, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Ti+",
    task: "R08.T4.e",
    what: "Ti II's lines in the range, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Al",
    task: "R08.T4.e",
    what: "Al I 394.4 and 396.2 nm, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "V",
    task: "R08.T4.e",
    what: "V I's lines in the range, strongest at 437.9, 438.5, 439.0, 411.2 and 384.1 nm, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Mn",
    task: "R08.T4.e",
    what: "Mn I 403.1, 403.3 and 403.4 nm and its 2.1-2.3 eV lines, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Ni",
    task: "R08.T4.e",
    what: "Ni I's lines in the range, strongest at 385.8, 380.7 and 547.7 nm, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "V+",
    task: "R08.T4.e",
    what: "V II's lines (387-404 nm, from 1.4-1.8 eV), deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Cr+",
    task: "R08.T4.e",
    what: "Cr II's lines (455.9 and 458.8 nm and others, from 3.9-4.1 eV), deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "Mn+",
    task: "R08.T4.e",
    what: "Mn II's lines (412.8 and 420.5 nm, and 432-440 nm), deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  // The stated nones above their limits, as He is: R08.T4.e's range, to 5,000 K, does not draw
  // them above 5,000 K. Mg⁺ and Si⁺ are drawn from their limit as a visibility floor by its tables.
  {
    key: "Na+",
    task: "R08.T4.e",
    what: "Na II's lines in the range above 5000 K, which R08.T4.e's range (to 5000 K) does not draw",
  },
  {
    key: "Al+",
    task: "R08.T4.e",
    what: "Al II's lines in the range above 5000 K, which R08.T4.e's range (to 5000 K) does not draw",
  },
  {
    key: "K+",
    task: "R08.T4.e",
    what: "K II's lines in the range above 5000 K, which R08.T4.e's range (to 5000 K) does not draw",
  },
  {
    key: "Ni+",
    task: "R08.T4.e",
    what: "Ni II's lines in the range above 4600 K, deferred past T4.e's byte budget (main, 2026-10-19)",
  },
  {
    key: "O",
    task: "R08.T4.e",
    what: "O I's lines in the range ([O I] 630.0 and 636.4 nm) above 5000 K, which R08.T4.e's range (to 5000 K) does not draw",
  },
  {
    key: "Zn+",
    task: "R08.T4.e",
    what: "Zn II's lines in the range above 5000 K, which R08.T4.e's range (to 5000 K) does not draw",
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
  {
    key: "HCl",
    task: "R08.T4.f",
    part: "overtones",
    what: "the 8-0 band (HITRAN2024's, an extrapolation) and the 9-0 to 11-0 bands, which HITRAN2024 lacks, by the overtone ladder below 8-0 (estimated)",
  },
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

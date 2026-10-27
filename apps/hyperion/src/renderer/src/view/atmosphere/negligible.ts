/**
 * The negligible-absorbers bound (plan R08, R08.T10.a; `decision-r08-t4b-t12c.md`, follow-ups 3–5;
 * `decision-r08-t4d-kdist-chain.md` §§14, 16 and 17): for one body, whether each absorber the
 * client does not draw, or draws approximately, could change any channel's light by more than
 * {@link NEGLIGIBLE_CHANGE}, so that the medium can give a negligible pending species no column, no
 * curve and no label, and name it `negligible`.
 *
 * @remarks
 * **What is tested** ({@link negligibleAbsorbers}): each listed species or pair with an entry still
 * in `PENDING_ABSORBERS` (`absorbers/transparency.ts`), and each species or pair drawn with
 * estimated optics or past its data's temperatures (`absorbers.ts`'s `absorberSpecs`, its
 * `estimated` members). A species stated transparent at its profile's temperature is not tested.
 *
 * **The bound.** For each of the body's suns and each channel c,
 *
 *   Δ_c = Σ_cells |c̄_c| S Δλ min(1, Σ_shares max_entries k⟨σ⟩_cell u_max) ÷ Σ_cells c̄_c S Δλ,
 *
 * on the 1 nm cells centred on every whole nm of 380–760 nm, c̄_c S Δλ the colour pipeline's own
 * row weights (`spectralColour.ts`'s `sunWeights`, the curves' weights for a 1 nm cell) under the
 * curves' rule ({@link boundCellWeights}). Since 1 − e^(−x) ≤ min(1, x) and, cell by cell,
 * 1 − ⟨e^(−σu)⟩ ≤ min(1, ⟨σ⟩u) (Jensen's inequality), Δ_c bounds the change of the channel's light
 * from above. u_max is per entry, in its kind's unit (a column, molecules m⁻², or a pair column,
 * ∫ n_A n_B ds, molecule pairs m⁻⁵), so it sits inside the sum, not outside it as the plan's formula
 * prints it. An entry's cell means are taken at its profile's column-weighted temperature (the
 * pair-weighted for a pair), a pending entry's interpolated with ln σ linear in 1/T between its
 * nodes and held beyond them ({@link boundCellMeansAt}). Entries of one share are alternatives,
 * a temperate table and its hot list, of which the larger k⟨σ⟩u is taken cell by cell (follow-up 5,
 * ruling 3); shares add (a part such as CO's `overtones`, a drawn species' lines and its
 * cross-sections).
 *
 * **k** is 1 for measured data and {@link ESTIMATE_MARGIN} for estimated data, a temperature past
 * the data's, or a line table's nodes past its measured range (HITRAN's 500 K nodes above 296 K,
 * which lack hot lines; follow-up 5, ruling 4).
 *
 * **u_max** is the largest column along any grazing chord through the profile, both halves
 * ({@link largestChordAmount}): the largest column any table holds, the transmittance table's
 * deepest texel holding the whole chord tangent at the datum (`opticalDepth.ts`'s
 * `opticalDepthToTop`), not the one-sided horizontal ray from the ground, which is half of it.
 *
 * **The held χ** (§14), on a drawn line table whose χ is held at the temperature, and **CO₂'s
 * fixed-g mix** (§§16–17), on one with a mix record, are separate tests ({@link HeldChiVerdict},
 * {@link FixedGMixVerdict}), as is **the trace broadeners' effect bound** (R08.T10.c;
 * `decision-r08-approximate-plate.md`, item 1a), on measured lines broadened in part by a gas their
 * table holds no width for ({@link BroadenerVerdict}).
 *
 * Pure and synchronous: the optics worker calls it once a body.
 */

import type { Rgb } from "../photometry/toneCurve";
import {
  ABSORBER_CROSS_SECTIONS,
  ABSORBER_PAIR_CROSS_SECTIONS,
  AIR_PARTNER,
  AIR_TRACE_SHARE,
  type AbsorberAmount,
  type AbsorberLayer,
  type AbsorberSpecs,
  absorberSpecs,
  absorberTerm,
  type ColumnAbsorberSpec,
  type CrossSectionTable,
  layerMoments,
  lineBroadeningMixture,
  type PairAbsorberSpec,
  type PairCrossSection,
  type PairCrossSectionTable,
  type SpeciesCrossSection,
  type SpeciesLines,
} from "./absorbers";
import {
  broadeningFactor,
  type KDistributionTable,
  mixLimitColumnM2,
  type SpeciesKDistribution,
} from "./absorbers/kDistribution";
import {
  type AbsorberBound,
  isTransparent,
  PENDING_ABSORBERS,
  PENDING_CELL_COUNT,
  PENDING_CELLS_NM,
  type PendingAbsorber,
  type PendingTask,
} from "./absorbers/transparency";
import type { AtmosphereColumn } from "./column";
import { densityAt, type DensityProfile, type MediumTerm } from "./medium";
import { type BakeSpectrum, sunWeights } from "./spectralColour";

/**
 * The most an absorber may change any channel's light and be negligible: 10⁻³
 * (`decision-r08-t4b-t12c.md`, follow-up 3, ruling 4, and follow-up 4, ruling 3), far under the 5%
 * metric and any just-noticeable difference.
 */
export const NEGLIGIBLE_CHANGE = 1e-3;

/**
 * k for estimated data, a temperature past the data's, or a table's unmeasured nodes: 100
 * (follow-up 3, ruling 4: the overtone ladder's factor of 10 a quantum against the 5–20 observed in
 * X–H stretches errs at most 8 times over two or three quanta, kept two decades clear, so that no
 * estimate exempts a species within its own uncertainty).
 */
export const ESTIMATE_MARGIN = 100;

/**
 * σ₁, cm⁻¹: where the held χ sets first depart from 1, and so the inner edge of the Lorentz share
 * f₃ beyond it: 3, both of SpeCT's CO₂ sets' first break (Chaverot et al. 2025, A&A 702, A137,
 * Table 3: CO₂–CO₂'s σ 3, 50 and 220 cm⁻¹, CO₂–N₂'s 3, 10 and 70 cm⁻¹, as CO₂'s sidecar's
 * reduction states them), `src/tools/kDistributions.ts`'s `NEAR_WING_FROM_PER_CM`.
 */
export const CHI_FROM_PER_CM = 3;

/**
 * The near-wing share's outer edge, cm⁻¹: 25, the lines' cut-off to which the reduction takes it
 * (`src/tools/molecularLines.ts`'s `LINE_CUTOFF_PER_CM`; `src/tools/kDistributions.ts`'s
 * `cellNodeOf`).
 */
const NEAR_WING_TO_PER_CM = 25;

/**
 * χ's reach, cm⁻¹: 293, beyond which χ < 10⁻³ at every temperature over 100–500 K, so that no held
 * wing lands farther from its line (`decision-r08-t4d-kdist-chain.md` §13; CO₂'s sidecar's
 * reduction, "out to 293 cm-1").
 */
export const CHI_REACH_PER_CM = 293;

/**
 * The largest near-wing share from which γ_L is recovered: 0.1, where γ_L is about 0.5 cm⁻¹, far on
 * the share's rising branch (its peak, 0.575, is at γ_L = √75 cm⁻¹), so that the cell with the
 * largest share is the one with the widest lines ({@link lorentzHalfWidthPerCm}).
 */
const NEAR_WING_RECOVERY_LIMIT = 0.1;

/** nm per cm, for a cell's wavenumbers: ν = 10⁷ ÷ λ cm⁻¹ for λ in nm. */
const NM_PER_CM = 1e7;

/** Why an entry takes its k. */
export type MarginRule =
  /** Measured data within its temperatures: k = 1. */
  | "measured"
  /** A stated estimate (a basis, or optics `estimated` at the body): k = {@link ESTIMATE_MARGIN}. */
  | "estimatedData"
  /** A temperature past the data's, its nearest values held: k = {@link ESTIMATE_MARGIN}. */
  | "pastTemperatures"
  /**
   * A line table read between nodes past its measured range, HITRAN's 500 K nodes above 296 K,
   * which lack hot lines: k = {@link ESTIMATE_MARGIN}.
   */
  | "unmeasuredNodes";

/** What every entry a bound weighs carries, drawn or pending. */
export interface BoundEntryBase {
  /**
   * The share the entry falls in: `lines`, `cross-sections` or `pair` for drawn data and the hot
   * list that alternates with it, `whole` for a pending entry of a key the client draws nothing for,
   * or `part:` and the entry's part. Entries of one share alternate, shares add.
   */
  readonly share: string;
  readonly k: number;
  readonly rule: MarginRule;
  /** The temperature its cell means are taken at, K. */
  readonly temperatureK: number;
  /** u_max, the largest grazing chord's amount, in its kind's unit ({@link largestChordAmount}). */
  readonly amount: AbsorberAmount;
}

/** Data the medium draws, taken as estimated at the body. */
export interface DrawnBoundEntry extends BoundEntryBase {
  readonly origin: "drawn";
}

/** A `PENDING_ABSORBERS` entry, its absorption not drawn. */
export interface PendingBoundEntry extends BoundEntryBase {
  readonly origin: "pending";
  /** The task that draws it. */
  readonly task: PendingTask;
  /** Its part, as its bound's; `undefined` for a whole entry. */
  readonly part: string | undefined;
}

/** One entry a bound weighs: drawn data taken as estimated, or a pending entry. */
export type BoundEntry = DrawnBoundEntry | PendingBoundEntry;

/** The bound on one key's absorption (`absorption`), the plan's Δ_c. */
export interface AbsorptionVerdict {
  readonly test: "absorption";
  /** The species' registry key, or a pair's key. */
  readonly key: string;
  /** The entries weighed, drawn data first, then pending entries in `PENDING_ABSORBERS`' order. */
  readonly entries: ReadonlyArray<BoundEntry>;
  /** Δ_c for each sun, in light-slot order, per channel. */
  readonly changes: ReadonlyArray<Rgb>;
  /** The largest Δ_c over the suns and channels. */
  readonly largestChange: number;
  /** Whether Δ_c ≤ {@link NEGLIGIBLE_CHANGE} in every channel under every sun. */
  readonly negligible: boolean;
}

/**
 * The bound on a held χ's effect (`decision-r08-t4d-kdist-chain.md` §14), over a drawn two-variant
 * table's cells at a temperature where a variant the mix takes holds its χ: Δ_c,χ =
 * Σ_cells w_c,cell min(1, f₃ ⟨σ⟩_cell u_max) ÷ Σ_cells c̄_c S Δλ with k = 1, w_c,cell the largest
 * |c̄_c| S Δλ within {@link CHI_REACH_PER_CM} of the cell. The true χ lies in [0, 1], by which §14
 * bounds any held χ's effect from above. The form is §14's as ruled, min(1, ·) taken at the source
 * cell on its already reduced ⟨σ⟩, which may run low: by 1.7% on Earth and 5.6% on a snowball at a
 * few bar, and up to 5 times at Venus's pressure, as recorded for the owner's ruling.
 */
export interface HeldChiVerdict {
  readonly test: "heldChi";
  readonly key: string;
  /** f₃ = 1 − (2 ÷ π) arctan(σ₁ ÷ γ_L), the Lorentz share beyond σ₁ at the body's largest p_eff. */
  readonly lorentzShare: number;
  /** γ_L there, cm⁻¹, or `undefined` where the table records no near-wing share (f₃ taken as 1). */
  readonly lorentzHalfWidthPerCm: number | undefined;
  readonly amount: AbsorberAmount;
  readonly temperatureK: number;
  /** Δ_c,χ for each sun, per channel. */
  readonly changes: ReadonlyArray<Rgb>;
  readonly largestChange: number;
  /** Whether Δ_c,χ ≤ {@link NEGLIGIBLE_CHANGE} everywhere: the hold then takes no label. */
  readonly negligible: boolean;
}

/**
 * CO₂'s fixed-g mix (`decision-r08-t4d-kdist-chain.md` §§16–17): whether the body's u_max reaches a
 * cell's u_mix(w, T) at its self share w and temperature (`absorbers/kDistribution.ts`'s
 * `mixLimitColumnM2`), where the species is `estimated`.
 */
export interface FixedGMixVerdict {
  readonly test: "fixedGMix";
  readonly key: string;
  /** w = x r_self ÷ Σ_j x_j r_j at the profile's temperature. */
  readonly selfShare: number;
  readonly temperatureK: number;
  readonly amount: AbsorberAmount;
  /** The largest u_max ÷ u_mix over the cells: 0 where no cell records a u_mix. */
  readonly largestColumnRatio: number;
  /** Whether u_max stays under every cell's u_mix: the mix then takes no label. */
  readonly negligible: boolean;
}

/**
 * The trace broadeners' effect bound (R08.T10.c; `decision-r08-approximate-plate.md`, item 1a,
 * 2026-10-27), on a drawn line table whose lines are otherwise measured at the body and whose
 * broadening pressure rests in part on gases the table holds no width for, each taken as air
 * (`absorbers.ts`'s `SpeciesLines.widthlessShare`, ε). The lines' strengths and air widths are
 * measured: only the broadening pressure is uncertain, by those gases' terms.
 *
 * @remarks
 * Each such gas's ratio to air at the profile's temperature is taken to lie in [0, 2]: item 1a's
 * assumption, which borrows the bracket `AIR_TRACE_SHARE`'s doc puts on a continuum partner's
 * efficiency, not a measured range of line widths. The committed molecular tables'
 * intensity-weighted ratios to air for the foreign gases without a dipole they list lie in
 * 0.34–1.58 (HCl's He, CO₂'s H₂); no single line is checked. So the broadening pressure lies within a factor s ∈ [1 − ε, 1 + ε] of the one
 * drawn. A Lorentz or Voigt profile has |∂ln f ÷ ∂ln γ| ≤ 1, γ the Lorentz half-width and the
 * Doppler width held (H₂O's, its value at 25 cm⁻¹ subtracted, reaches 1.02 at its shape's 2.5 cm⁻¹
 * limit), so a cell's transmittance moves by at most ⟨τe^(−τ)⟩ ≤ 1 − T ≤ min(1, ⟨σ⟩u) per unit of
 * ln s, and by at most min(1, ⟨σ⟩u) whatever s is, since it lies in [1 − that, 1] at every width.
 *
 * Hence Δ_c,γ = min(1, −ln(1 − ε)) Δ_c(k = 1), the exact form of the ruling's ε Δ_c(k = 1), which
 * is its first-order term (accepted on the issue, 2026-10-27), Δ_c(k = 1) the lines' own bound at
 * k = 1 on the chord, cells and suns their `absorption` test takes. Both steps take a cell's ⟨σ⟩ as
 * the same at every width to 1 + ε times the broadening pressure; the cell means are read over the
 * stored rows to the body's largest, with the first past it (R08.T10.c's deferred findings). A gas
 * with a permanent dipole that the ruling names takes no such range (`kDistribution.ts`'s
 * `DIPOLAR_BROADENERS`), and keeps the species `estimated`.
 */
export interface BroadenerVerdict {
  readonly test: "broadener";
  readonly key: string;
  /** ε, the share of the broadening pressure on gases the table holds no width for, above 0. */
  readonly widthlessShare: number;
  /** min(1, −ln(1 − ε)), 1 for ε ≥ 1: the most the lines' widths move, as |ln s|. */
  readonly widthFactor: number;
  readonly amount: AbsorberAmount;
  readonly temperatureK: number;
  /** Δ_c(k = 1) for each sun, per channel: the lines' own bound, their data taken as measured. */
  readonly lineChanges: ReadonlyArray<Rgb>;
  /** Δ_c,γ = {@link BroadenerVerdict.widthFactor} × Δ_c(k = 1) for each sun, per channel. */
  readonly changes: ReadonlyArray<Rgb>;
  readonly largestChange: number;
  /** Whether Δ_c,γ ≤ {@link NEGLIGIBLE_CHANGE} everywhere: the gases are then taken as air, unlabelled. */
  readonly negligible: boolean;
}

/** One test's outcome for one key. */
export type NegligibleVerdict =
  AbsorptionVerdict | HeldChiVerdict | FixedGMixVerdict | BroadenerVerdict;

/** What the bound reads of a body: what its optics worker passes to `absorberSpecs`, and its suns. */
export interface NegligibleBody {
  /** The body's column (`column.ts`'s `hydrostaticColumn`): its datum is the medium's lowest level. */
  readonly column: AtmosphereColumn;
  /** The absorber layers, as `absorberSpecs` takes them, the bulk gases among them. */
  readonly layers: ReadonlyArray<AbsorberLayer>;
  /** The bulk gas's mole fractions by registry key, which broaden the lines. */
  readonly bulkMoleFractions: ReadonlyMap<string, number>;
  /** The body's suns' bake spectra, in light-slot order: at least one. */
  readonly suns: ReadonlyArray<BakeSpectrum>;
  /** The loaded k-distribution tables (`absorbers/kDistribution.ts`'s `loadKDistributions`). */
  readonly kDistributions: ReadonlyArray<KDistributionTable>;
}

/** The tables the bound takes, the committed ones unless a caller gives its own. */
export interface NegligibleOptions {
  /**
   * `absorberSpecs`' result for the body's layers, column, tables and options, where the caller has
   * it; computed if absent.
   */
  readonly specs?: AbsorberSpecs;
  readonly crossSections?: ReadonlyArray<CrossSectionTable>;
  readonly pairCrossSections?: ReadonlyArray<PairCrossSectionTable>;
  readonly pending?: ReadonlyArray<PendingAbsorber>;
}

/** A channel's per-cell weights and their sum. */
export interface BoundCellWeights {
  /**
   * Per channel, each cell's weight c̄_c S Δλ, in R06's units: the colour pipeline's 1 nm rows,
   * the trapezoid rule's half at 380 and 760 nm, signed while the sun's colour in the channel is
   * positive, max(c̄_c, 0) S Δλ where it is not (the curves' rule, `absorbers.ts`'s
   * `channelTransmittance`).
   */
  readonly cells: readonly [Float64Array, Float64Array, Float64Array];
  /** Each channel's Σ_cells of its weights, above 0. */
  readonly totals: Rgb;
}

/**
 * A sun's {@link BoundCellWeights}, on the cells of `PENDING_CELLS_NM`.
 *
 * @throws RangeError for a sun with no light in a channel's weights.
 */
export function boundCellWeights(sun: BakeSpectrum): BoundCellWeights {
  const { red, green, blue } = sunWeights(sun);
  const ruled = [red, green, blue].map((row, c) => {
    const signed = row.reduce((sum, w) => sum + w, 0);
    if (signed > 0) {
      return { row, total: signed };
    }
    const clipped = row.map((w) => Math.max(w, 0));
    const total = clipped.reduce((sum, w) => sum + w, 0);
    if (!(total > 0)) {
      throw new RangeError(`the sun has no light in channel ${String(c)}'s weights`);
    }
    return { row: clipped, total };
  });
  const [r, g, b] = ruled;
  if (r === undefined || g === undefined || b === undefined) {
    throw new Error("a sun's weights have three channels");
  }
  return { cells: [r.row, g.row, b.row], totals: [r.total, g.total, b.total] };
}

/**
 * Δ_c for each channel: Σ_cells |w_c,cell| min(1, x_cell) ÷ Σ_cells w_c,cell, x_cell a cell's bound
 * on its optical depth (Σ k⟨σ⟩u), at least 0.
 *
 * @param cellDepths - x at each of the {@link PENDING_CELL_COUNT} cells.
 * @param cellWeights - Each channel's weights to take in the sum; the weights' own by default (the
 *   held χ's take the largest within reach).
 */
export function channelChanges(
  weights: BoundCellWeights,
  cellDepths: Float64Array,
  cellWeights: readonly [Float64Array, Float64Array, Float64Array] = weights.cells,
): Rgb {
  const change = cellWeights.map((row, c) => {
    let sum = 0;
    for (const [i, w] of row.entries()) {
      sum += Math.abs(w) * Math.min(1, cellDepths[i] ?? 0);
    }
    return sum / (weights.totals[c] ?? Number.NaN);
  });
  return [change[0] ?? Number.NaN, change[1] ?? Number.NaN, change[2] ?? Number.NaN];
}

/** A cell's wavenumbers, cm⁻¹: [10⁷ ÷ (λ + 0.5), 10⁷ ÷ (λ − 0.5)] for the cell centred on λ nm. */
function cellWavenumbersPerCm(cell: number): readonly [number, number] {
  const nm = PENDING_CELLS_NM[0] + cell;
  return [NM_PER_CM / (nm + 0.5), NM_PER_CM / (nm - 0.5)];
}

/**
 * Each channel's largest |w| over the cells within a reach of each cell, cm⁻¹: those whose
 * wavenumbers come within the reach of the cell's anywhere, so that a wing landing on any part of
 * them is weighed at their weight.
 */
function reachWeights(
  weights: BoundCellWeights,
  reachPerCm: number,
): readonly [Float64Array, Float64Array, Float64Array] {
  const spans = Array.from({ length: PENDING_CELL_COUNT }, (_, c) => cellWavenumbersPerCm(c));
  const within = spans.map(([low, high]) =>
    spans.flatMap(([a, b], j) => (a <= high + reachPerCm && b >= low - reachPerCm ? [j] : [])),
  );
  const [red, green, blue] = weights.cells.map((row) =>
    Float64Array.from(within, (cells) =>
      cells.reduce((largest, j) => Math.max(largest, Math.abs(row[j] ?? 0)), 0),
    ),
  );
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("a sun's weights have three channels");
  }
  return [red, green, blue];
}

/** A pending entry's cell means at a temperature, and whether the temperature lies past its nodes. */
export interface BoundCellMeans {
  /** At each of the {@link PENDING_CELL_COUNT} cells, m² a molecule or m⁵ a molecule pair. */
  readonly cellMeans: Float64Array;
  /** Whether the temperature lies below the entry's first node or above its last: its ends held. */
  readonly pastNodes: boolean;
}

/**
 * A pending entry's 1 nm cell means at a temperature (follow-up 5, ruling 4): ln σ linear in 1/T
 * between the two nodes about it, the Boltzmann law's own form, exact for a line from a single lower
 * level and above the true value for a sum of such lines over a near-constant partition function
 * (ln σ is then convex in 1/T); σ linear in 1/T where either node's is 0, above the convex curve too;
 * the nearest node's values held beyond the nodes.
 *
 * @throws RangeError for a temperature that is not finite and above 0 K.
 */
export function boundCellMeansAt(bound: AbsorberBound, temperatureK: number): BoundCellMeans {
  if (!(Number.isFinite(temperatureK) && temperatureK > 0)) {
    throw new RangeError(`a bound's temperature is finite and above 0 K, not ${temperatureK}`);
  }
  let rows: ReadonlyArray<Float64Array>;
  switch (bound.kind) {
    case "species":
      rows = bound.cellMeansM2;
      break;
    case "pair":
      rows = bound.cellMeansM5;
      break;
  }
  const nodes = bound.temperaturesK;
  const first = nodes[0] ?? Number.NaN;
  const last = nodes.at(-1) ?? Number.NaN;
  const pastNodes = temperatureK < first || temperatureK > last;
  const exact = nodes.indexOf(temperatureK);
  if (exact >= 0 || nodes.length === 1 || pastNodes) {
    const at = exact >= 0 ? exact : temperatureK <= first ? 0 : nodes.length - 1;
    return { cellMeans: Float64Array.from(rows[at] ?? []), pastNodes };
  }
  let k = 0;
  while (k < nodes.length - 2 && (nodes[k + 1] ?? Infinity) < temperatureK) {
    k += 1;
  }
  const lowK = nodes[k] ?? Number.NaN;
  const highK = nodes[k + 1] ?? Number.NaN;
  const f = (1 / temperatureK - 1 / lowK) / (1 / highK - 1 / lowK);
  const below = rows[k] ?? new Float64Array(0);
  const above = rows[k + 1] ?? new Float64Array(0);
  const cellMeans = below.map((a, i) => {
    const b = above[i] ?? Number.NaN;
    return a > 0 && b > 0
      ? Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * f)
      : a + (b - a) * f;
  });
  return { cellMeans, pastNodes };
}

/**
 * ∫ n ds over [x₀, x₁] of a chord tangent at radius r_t, x the height above the tangent, for n linear
 * in height between n₀ and n₁: s(x) = √(x(2r_t + x)) the distance along the chord from the tangent,
 * so that n₀(s₁ − s₀) + β[(x₁ − x₀)s₁ − ∫s dx], β the slope, with ∫s dx from F(x) =
 * ½[(r_t + x)s − r_t² ln((r_t + x + s) ÷ r_t)], whose derivative is s.
 */
function chordSegment(
  tangentRadiusM: number,
  x0M: number,
  x1M: number,
  n0: number,
  n1: number,
): number {
  const s = (xM: number): number => Math.sqrt(xM * (2 * tangentRadiusM + xM));
  const primitive = (xM: number, sM: number): number =>
    0.5 *
    ((tangentRadiusM + xM) * sM - tangentRadiusM ** 2 * Math.log1p((xM + sM) / tangentRadiusM));
  const s0M = s(x0M);
  const s1M = s(x1M);
  const slope = (n1 - n0) / (x1M - x0M);
  const integralS = primitive(x1M, s1M) - primitive(x0M, s0M);
  return n0 * (s1M - s0M) + slope * ((x1M - x0M) * s1M - integralS);
}

/** The heights a profile's density is linear between, m, with the column's levels, from 0 to its top. */
function profileBreaksM(profile: DensityProfile, column: AtmosphereColumn): Float64Array {
  let own: ReadonlyArray<number>;
  switch (profile.kind) {
    case "tabulated":
      own = Array.from(profile.altitudesM);
      break;
    case "tent":
      own = [profile.bottomM, profile.peakM, profile.topM];
      break;
    case "exponential":
      own = [];
      break;
  }
  const topM = column.topHeightM;
  const heights = [0, ...column.altitudesM, ...own, topM].filter((h) => h >= 0 && h <= topM);
  return Float64Array.from(new Set(heights)).toSorted();
}

/**
 * ∫ρ ds along the largest grazing chord through a profile, both halves, m: the largest over the
 * chords tangent at each of its breaks from the datum to the column's top, ρ linear in height
 * between its breaks and the column's levels (exact for a tabulated or tent profile; above an
 * exponential one, which is convex), on a sphere of the column's reference radius.
 */
function largestChordM(profile: DensityProfile, column: AtmosphereColumn): number {
  const heights = profileBreaksM(profile, column);
  const densities = heights.map((h) => densityAt(profile, h));
  let largest = 0;
  for (let t = 0; t < heights.length - 1; t += 1) {
    const tangentM = heights[t] ?? Number.NaN;
    const radiusM = column.referenceRadiusM + tangentM;
    let sum = 0;
    for (let j = t; j < heights.length - 1; j += 1) {
      sum += chordSegment(
        radiusM,
        (heights[j] ?? Number.NaN) - tangentM,
        (heights[j + 1] ?? Number.NaN) - tangentM,
        densities[j] ?? Number.NaN,
        densities[j + 1] ?? Number.NaN,
      );
    }
    largest = Math.max(largest, sum);
  }
  return 2 * largest;
}

/**
 * u_max, an absorber term's largest amount along any grazing chord through its medium, both halves
 * (R08.T10.a): its density at relative density 1 times {@link largestChordM} of its profile, a
 * column for a column absorber and a pair column for a pair absorber.
 *
 * @remarks
 * The plan's "horizontal ray at the medium's lowest level" is, by its sources' "the largest column
 * any table holds" (follow-up 3, ruling 4), the chord tangent at the datum from the top down, which
 * the transmittance table's deepest texel holds whole (`opticalDepth.ts`'s `opticalDepthToTop`):
 * √(2πR ÷ H) vertical columns for an isothermal layer, twice the one-sided ray from the ground. For
 * a layer from the datum whose density falls with height the datum's chord is the largest. The
 * chords tangent above the datum are weighed too, for a layer whose density peaks aloft, but
 * sampled at tangents on the profile's breaks and the column's levels, not bounded from above
 * between them: a symmetric tent sampled only at its ends and peak would run 13% low, a 1 km tent
 * at Earth's level spacing 1.2%, one at 20 km 10⁻⁴. On an oblate body the chord is taken on the
 * R_ref sphere, about 9% low on a Saturn-class figure, until R08.T16.a.
 *
 * The bound is per key and per straight path: keys negligible on one body add, each up to
 * {@link NEGLIGIBLE_CHANGE}.
 *
 * @throws Error for a term that is not an absorber's.
 */
export function largestChordAmount(term: MediumTerm, column: AtmosphereColumn): AbsorberAmount {
  const absorber = term.absorber;
  if (absorber === undefined) {
    throw new Error(`term ${term.name} is not an absorber's`);
  }
  const lengthM = largestChordM(term.density, column);
  let amount: AbsorberAmount;
  switch (absorber.kind) {
    case "column":
      amount = { kind: "column", columnPerM2: absorber.numberDensityPerM3 * lengthM };
      break;
    case "pair":
      amount = { kind: "pair", pairColumnPerM5: absorber.pairDensityPerM6 * lengthM };
      break;
  }
  return amount;
}

/** An amount's value in its kind's unit. */
function amountValue(amount: AbsorberAmount): number {
  let value: number;
  switch (amount.kind) {
    case "column":
      value = amount.columnPerM2;
      break;
    case "pair":
      value = amount.pairColumnPerM5;
      break;
  }
  return value;
}

/**
 * The Lorentz half-width γ_L, cm⁻¹, of a line table's widest cell at a temperature and broadening
 * pressure, from its sidecar's near-wing shares ({@link KDistributionTable.nearWingShare}), or
 * `undefined` where it records none.
 *
 * @remarks
 * The share is s = (2 ÷ π)[arctan(25 ÷ γ) − arctan(3 ÷ γ)] = (2 ÷ π) arctan(22γ ÷ (γ² + 75)), so on
 * its rising branch γ = 150q ÷ (22 + √(484 − 300q²)), q = tan(πs ÷ 2). Each node's share is taken
 * at the top of its two-figure rounding, so that γ is not low. A Lorentz width is linear in the
 * broadening pressure, so each temperature node's γ ÷ p is the largest over its pressure nodes whose
 * share is at most 0.1, well on the rising branch, and γ_L is that, the larger of the two nodes'
 * about the temperature (the end node's beyond them), times the pressure.
 *
 * @param broadeningPa - The air-equivalent broadening pressure, Pa.
 */
export function lorentzHalfWidthPerCm(
  table: KDistributionTable,
  temperatureK: number,
  broadeningPa: number,
): number | undefined {
  const shares = table.nearWingShare;
  if (shares === undefined) {
    return undefined;
  }
  const pressures = table.pressuresPa.length;
  const span = NEAR_WING_TO_PER_CM - CHI_FROM_PER_CM;
  const product = NEAR_WING_TO_PER_CM * CHI_FROM_PER_CM;
  const perPa = (t: number): number | undefined => {
    let largest: number | undefined;
    for (let p = 0; p < pressures; p += 1) {
      const s = shares[t * pressures + p] ?? 0;
      if (!(s > 0 && s <= NEAR_WING_RECOVERY_LIMIT)) {
        continue;
      }
      const unit = 10 ** (Math.floor(Math.log10(s)) - 1);
      const q = Math.tan((Math.PI * (s + 0.5 * unit)) / 2);
      const gamma = (2 * product * q) / (span + Math.sqrt(span ** 2 - 4 * product * q ** 2));
      const ratio = gamma / (table.pressuresPa[p] ?? Number.NaN);
      largest = largest === undefined ? ratio : Math.max(largest, ratio);
    }
    return largest;
  };
  const nodes = table.temperaturesK;
  let k = 0;
  while (k < nodes.length - 2 && (nodes[k + 1] ?? Infinity) < temperatureK) {
    k += 1;
  }
  const about =
    temperatureK <= (nodes[0] ?? Infinity)
      ? [0]
      : temperatureK >= (nodes.at(-1) ?? -Infinity)
        ? [nodes.length - 1]
        : [k, k + 1];
  const ratios = about.flatMap((t) => {
    const r = perPa(t);
    return r === undefined ? [] : [r];
  });
  return ratios.length === 0 ? undefined : Math.max(...ratios) * broadeningPa;
}

/**
 * f₃ = 1 − (2 ÷ π) arctan(σ₁ ÷ γ_L) = (2 ÷ π) arctan(γ_L ÷ σ₁), the share of a Lorentz line's
 * strength beyond σ₁ = {@link CHI_FROM_PER_CM} (`decision-r08-t4d-kdist-chain.md` §14); 1, all of
 * it, where γ_L is unknown.
 */
function lorentzShareBeyond(gammaPerCm: number | undefined): number {
  return gammaPerCm === undefined ? 1 : (2 / Math.PI) * Math.atan(gammaPerCm / CHI_FROM_PER_CM);
}

/** The cell a wavelength on a table's grid falls in, [λ − 0.5, λ + 0.5): its index from 380 nm. */
function cellIndexOf(wavelengthNm: number): number {
  return Math.floor(wavelengthNm + 0.5 + 1e-9) - PENDING_CELLS_NM[0];
}

/**
 * A grid's mean over each cell's points, on the bound's cells, for the cells below `belowNm` alone
 * where given (0 elsewhere).
 */
function gridCellMeans(
  firstNm: number,
  stepNm: number,
  values: Float64Array,
  belowNm = Infinity,
): Float64Array {
  const sums = new Float64Array(PENDING_CELL_COUNT);
  const counts = new Float64Array(PENDING_CELL_COUNT);
  for (const [i, v] of values.entries()) {
    const nm = firstNm + i * stepNm;
    const cell = cellIndexOf(nm);
    if (cell >= 0 && cell < PENDING_CELL_COUNT && nm < belowNm) {
      sums[cell] = (sums[cell] ?? 0) + v;
      counts[cell] = (counts[cell] ?? 0) + 1;
    }
  }
  return sums.map((sum, c) => ((counts[c] ?? 0) > 0 ? sum / (counts[c] ?? 1) : 0));
}

/**
 * A line species' cell means at its temperature, m²: each cell's Σ_g w_g σ_g, the largest over the
 * table's pressure rows up to the first at or past the body's largest broadening pressure, so that
 * no path's read exceeds it.
 */
function lineCellMeans(kd: SpeciesKDistribution, largestBroadeningPa: number): Float64Array {
  const out = new Float64Array(PENDING_CELL_COUNT);
  const pressures = kd.pressuresPa.length;
  const reach = kd.pressuresPa.findIndex((p) => p >= largestBroadeningPa);
  const lastRow = reach < 0 ? pressures - 1 : reach;
  for (const cell of kd.cells) {
    const g = cell.weights.length;
    let largest = 0;
    for (let p = 0; p <= lastRow; p += 1) {
      let mean = 0;
      for (const [k, w] of cell.weights.entries()) {
        mean += w * (cell.sigmas[p * g + k] ?? 0);
      }
      largest = Math.max(largest, mean);
    }
    const at = cell.cellNm - PENDING_CELLS_NM[0];
    if (at >= 0 && at < PENDING_CELL_COUNT) {
      out[at] = largest;
    }
  }
  return out;
}

/** An entry with its cell means, before the bound sums it. */
interface WeighedEntry {
  readonly entry: BoundEntry;
  readonly cellMeans: Float64Array;
}

/** A key's entries and its place among the verdicts. */
interface Subject {
  readonly key: string;
  readonly order: number;
  readonly entries: WeighedEntry[];
}

/** A listed layer with a column, at its place in the list. */
interface Listed {
  readonly index: number;
  readonly layer: AbsorberLayer;
}

/** The k and rule a drawn line table's estimate takes. */
function linesRule(lines: SpeciesLines, table: KDistributionTable | undefined): MarginRule {
  if (lines.provenance === "measured") {
    return "measured";
  }
  if (lines.estimatedBy === "optics") {
    return "estimatedData";
  }
  const nodes = table?.temperaturesK ?? [];
  const t = lines.kDistribution.temperatureK;
  return t >= (nodes[0] ?? Infinity) && t <= (nodes.at(-1) ?? -Infinity)
    ? "unmeasuredNodes"
    : "pastTemperatures";
}

/** The k a rule gives. */
function marginOf(rule: MarginRule): number {
  let k: number;
  switch (rule) {
    case "measured":
      k = 1;
      break;
    case "estimatedData":
    case "pastTemperatures":
    case "unmeasuredNodes":
      k = ESTIMATE_MARGIN;
      break;
  }
  return k;
}

/**
 * The verdicts of the negligible-absorbers bound on one body (R08.T10.a), one a tested key and test,
 * in the order of the body's layers (a pair after the later listed of its members), each key's tests
 * in the order `absorption`, `heldChi`, `fixedGMix`, `broadener`.
 *
 * @remarks
 * - `absorption` ({@link AbsorptionVerdict}): every key with drawn data `estimated` at the body (the
 *   estimated part alone: H₂O's continuum below 567 nm where only those wavelengths are estimated)
 *   or a pending entry in force. A pending entry with a part is in force wherever its species is
 *   listed and not stated transparent; a whole entry likewise where the client draws nothing for the
 *   key; a hot list beside a drawn table (CO, N₂O, HCN, H₂O, NH₃ and CO₂ in R08.T4.f) only above the
 *   drawn table's measured range, below which the table holds its lines. A pending pair is formed
 *   from its members' layers as `absorberSpecs` forms a pair.
 * - `heldChi` ({@link HeldChiVerdict}): a drawn line table whose χ is held at the temperature.
 * - `fixedGMix` ({@link FixedGMixVerdict}): a drawn line table with a fixed-g mix record.
 * - `broadener` ({@link BroadenerVerdict}, R08.T10.c): a drawn line table measured at the body
 *   (k = 1) whose broadening pressure rests in part on gases it holds no width for. Lines estimated
 *   on other grounds take their `absorption` test at k = 100, which covers the broadener too.
 *
 * A drawn absorber whose profile took no column (`noColumnFree`) is not tested: it is drawn as
 * nothing and labelled for that. Lines estimated over part of their pressures alone (an atom's
 * table below 10 Pa, `decision-r08-t4e-budget.md`'s deferred findings; a line shape past its
 * measured width at depth) take k = 100 on their whole chord's absorption, from above.
 *
 * @throws RangeError for no sun, a sun with no light in a channel's weights, or the layers
 *   `absorberSpecs` refuses.
 */
export function negligibleAbsorbers(
  body: NegligibleBody,
  options: NegligibleOptions = {},
): ReadonlyArray<NegligibleVerdict> {
  const { column, layers, bulkMoleFractions, suns, kDistributions } = body;
  if (suns.length === 0) {
    throw new RangeError("the negligible-absorbers bound needs at least one sun");
  }
  const crossSections = options.crossSections ?? ABSORBER_CROSS_SECTIONS;
  const pairTables = options.pairCrossSections ?? ABSORBER_PAIR_CROSS_SECTIONS;
  const pending = options.pending ?? PENDING_ABSORBERS;
  const specs =
    options.specs ??
    absorberSpecs(layers, column, crossSections, pairTables, {
      kDistributions,
      bulkMoleFractions,
    });
  const weights = suns.map(boundCellWeights);
  const listed = new Map<string, Listed>();
  for (const [index, layer] of layers.entries()) {
    if (layer.columnPerM2 > 0) {
      listed.set(layer.species, { index, layer });
    }
  }
  const pairOrder = (members: ReadonlyArray<string>): number =>
    Math.max(...members.map((m) => listed.get(m)?.index ?? -1)) + 0.5;
  const subjects = new Map<string, Subject>();
  const add = (key: string, order: number, weighed: WeighedEntry): void => {
    const subject = subjects.get(key) ?? { key, order, entries: [] };
    subject.entries.push(weighed);
    subjects.set(key, subject);
  };
  const extra: { readonly order: number; readonly verdict: NegligibleVerdict }[] = [];
  const pairPartnersEstimated = airPairsPartnerEstimated(listed, column, pairTables);

  for (const spec of specs.specs) {
    const u = amountValue(largestChordAmount(absorberTerm(spec, column), column));
    switch (spec.kind) {
      case "column":
        for (const s of spec.species) {
          const order = listed.get(s.species)?.index ?? -1;
          const amount: AbsorberAmount = {
            kind: "column",
            columnPerM2: (s.columnPerM2 / spec.columnPerM2) * u,
          };
          const temperatureK = spec.temperatureK;
          const crossSection = s.crossSection;
          if (crossSection?.provenance === "estimated") {
            add(s.species, order, drawnCrossSection(crossSection, amount, temperatureK));
          }
          const lines = s.lines;
          if (lines === undefined) {
            continue;
          }
          const table = kDistributions.find((t) => t.species === s.species);
          const largestBroadeningPa = spec.basePa * lines.broadeningFactor;
          const cellMeans = lineCellMeans(lines.kDistribution, largestBroadeningPa);
          const rule = linesRule(lines, table);
          if (rule !== "measured") {
            add(s.species, order, {
              entry: {
                origin: "drawn",
                share: "lines",
                k: marginOf(rule),
                rule,
                temperatureK,
                amount,
              },
              cellMeans,
            });
          }
          if (lines.kDistribution.chiHeld) {
            extra.push({
              order: order + 0.1,
              verdict: heldChiVerdict(s.species, table, lines, cellMeans, amount, weights, {
                temperatureK,
                largestBroadeningPa,
              }),
            });
          }
          if (table?.mixLimits !== undefined) {
            const numberColumnPerM2 =
              column.surfaceNumberDensityPerM3 *
              layerMoments(column, spec.basePa, spec.topPa, 1).integralM;
            const mixture = lineBroadeningMixture(
              s.species,
              s.columnPerM2 / numberColumnPerM2,
              bulkMoleFractions,
            );
            const { selfShare } = broadeningFactor(table, mixture, temperatureK);
            extra.push({
              order: order + 0.2,
              verdict: fixedGMixVerdict(s.species, table, selfShare, temperatureK, amount),
            });
          }
          if (rule === "measured" && lines.widthlessShare > 0) {
            extra.push({
              order: order + 0.3,
              verdict: broadenerVerdict(
                s.species,
                lines.widthlessShare,
                cellMeans,
                amount,
                weights,
                {
                  temperatureK,
                },
              ),
            });
          }
        }
        break;
      case "pair":
        for (const p of spec.pairs) {
          if (p.crossSection.provenance !== "estimated") {
            continue;
          }
          const amount: AbsorberAmount = {
            kind: "pair",
            pairColumnPerM5: (p.pairColumnPerM5 / spec.pairColumnPerM5) * u,
          };
          const table = pairTables.find((t) => t.pair === p.pair);
          add(
            p.pair,
            pairOrder(p.crossSection.members),
            drawnPair(p.crossSection, table, pairPartnersEstimated.has(p.pair), amount, {
              temperatureK: spec.temperatureK,
            }),
          );
        }
        break;
    }
  }

  const chordCache = new Map<string, number>();
  for (const listing of pending) {
    const weighed = pendingEntry(listing, {
      column,
      listed,
      kDistributions,
      crossSections,
      chordCache,
    });
    if (weighed !== undefined) {
      add(
        listing.key,
        weighed.entry.amount.kind === "pair"
          ? pairOrder(weighed.members)
          : (listed.get(listing.key)?.index ?? -1),
        weighed,
      );
    }
  }

  const verdicts: { readonly order: number; readonly verdict: NegligibleVerdict }[] = [
    ...[...subjects.values()].map((subject) => ({
      order: subject.order,
      verdict: absorptionVerdict(subject, weights),
    })),
    ...extra,
  ];
  return verdicts
    .toSorted((a, b) =>
      a.order !== b.order
        ? a.order - b.order
        : a.verdict.key < b.verdict.key
          ? -1
          : a.verdict.key > b.verdict.key
            ? 1
            : 0,
    )
    .map(({ verdict }) => verdict);
}

/** A drawn cross-section estimated at its temperature: past its law's measured range, k = 100. */
function drawnCrossSection(
  crossSection: SpeciesCrossSection,
  amount: AbsorberAmount,
  temperatureK: number,
): WeighedEntry {
  const rule: MarginRule = "pastTemperatures";
  return {
    entry: {
      origin: "drawn",
      share: "cross-sections",
      k: marginOf(rule),
      rule,
      temperatureK,
      amount,
    },
    cellMeans: gridCellMeans(
      crossSection.firstNm,
      crossSection.stepNm,
      crossSection.crossSectionsM2,
    ),
  };
}

/**
 * A drawn pair estimated at the body: every cell where its temperature lies past its table's
 * measured range, its partners pass {@link AIR_TRACE_SHARE} of an `air` pair's, or the table holds no
 * wavelength limit; otherwise the cells below its `estimatedBelowNm` alone (H₂O's continuum below
 * 567 nm), the rest measured.
 */
function drawnPair(
  crossSection: PairCrossSection,
  table: PairCrossSectionTable | undefined,
  byPartners: boolean,
  amount: AbsorberAmount,
  at: { readonly temperatureK: number },
): WeighedEntry {
  const rule: MarginRule =
    crossSection.provenance === "estimated" && crossSection.estimatedBy === "temperature"
      ? "pastTemperatures"
      : "estimatedData";
  // Only H2O's continuum's cells below its limit are estimated where nothing else is.
  const belowNm =
    table === undefined ||
    byPartners ||
    at.temperatureK < table.measuredRangeK[0] ||
    at.temperatureK > table.measuredRangeK[1]
      ? Infinity
      : (table.estimatedBelowNm ?? Infinity);
  return {
    entry: {
      origin: "drawn",
      share: "pair",
      k: marginOf(rule),
      rule,
      temperatureK: at.temperatureK,
      amount,
    },
    cellMeans: gridCellMeans(
      crossSection.firstNm,
      crossSection.stepNm,
      crossSection.crossSectionsM5,
      belowNm,
    ),
  };
}

/**
 * The `air` pairs whose partners other than the gases their σ is measured with pass
 * {@link AIR_TRACE_SHARE} of the partner molecules, by `absorberSpecs`' own rule: each listed gas but
 * the pair's first member, neither with a profile, weighed by its bare pair column over the overlap
 * of the two layers.
 */
function airPairsPartnerEstimated(
  listed: ReadonlyMap<string, Listed>,
  column: AtmosphereColumn,
  pairTables: ReadonlyArray<PairCrossSectionTable>,
): ReadonlySet<string> {
  const unit = (
    layer: AbsorberLayer,
  ): { readonly perM: number; readonly base: number; readonly top: number } => {
    const m = layerMoments(column, layer.basePa, layer.topPa, 1);
    return { perM: layer.columnPerM2 / m.integralM, base: m.basePa, top: m.topPa };
  };
  const out = new Set<string>();
  for (const table of pairTables) {
    const [first, second] = table.members;
    const a = listed.get(first)?.layer;
    if (second !== AIR_PARTNER || a === undefined || a.profile !== undefined) {
      continue;
    }
    const ua = unit(a);
    let all = 0;
    let trace = 0;
    for (const { layer: b } of listed.values()) {
      if (b.species === first || b.profile !== undefined) {
        continue;
      }
      const ub = unit(b);
      const from = Math.min(ua.base, ub.base);
      const to = Math.max(ua.top, ub.top);
      if (!(from > to)) {
        continue;
      }
      const bare = ua.perM * ub.perM * layerMoments(column, from, to, 2).integralM;
      all += bare;
      trace += table.air?.measured.has(b.species) === true ? 0 : bare;
    }
    if (trace > AIR_TRACE_SHARE * all) {
      out.add(table.pair);
    }
  }
  return out;
}

/** What a pending entry is weighed against: the body's column and layers, and the drawn tables. */
interface PendingContext {
  readonly column: AtmosphereColumn;
  readonly listed: ReadonlyMap<string, Listed>;
  readonly kDistributions: ReadonlyArray<KDistributionTable>;
  readonly crossSections: ReadonlyArray<CrossSectionTable>;
  /** Each key's largest chord already found, in its kind's unit. */
  readonly chordCache: Map<string, number>;
}

/**
 * A pending entry weighed on the body, with the members whose layers formed it, or `undefined`
 * where it is not in force: its species not listed with a column, stated transparent at its
 * temperature, or, for a hot list beside a drawn table, at or under the table's measured range; a
 * pair whose members are not both listed, or carry a profile, or do not overlap.
 */
function pendingEntry(
  listing: PendingAbsorber,
  context: PendingContext,
): (WeighedEntry & { readonly members: ReadonlyArray<string> }) | undefined {
  const { bound } = listing;
  const { column, listed } = context;
  let members: ReadonlyArray<string>;
  let temperatureK: number;
  let amount: AbsorberAmount;
  let share: string;
  switch (bound.kind) {
    case "species": {
      const layer = listed.get(listing.key)?.layer;
      if (layer === undefined) {
        return undefined;
      }
      const moments = layerMoments(column, layer.basePa, layer.topPa, 1, layer.profile);
      temperatureK = moments.temperatureK;
      if (isTransparent(listing.key, temperatureK)) {
        return undefined;
      }
      const lineTable = context.kDistributions.find((t) => t.species === listing.key);
      const crossTable = context.crossSections.find((t) => t.species === listing.key);
      const drawnTopK = (lineTable ?? crossTable)?.measuredRangeK[1];
      if (listing.part === undefined && drawnTopK !== undefined && !(temperatureK > drawnTopK)) {
        return undefined;
      }
      share =
        listing.part !== undefined
          ? `part:${listing.part}`
          : lineTable !== undefined
            ? "lines"
            : crossTable !== undefined
              ? "cross-sections"
              : "whole";
      const cached = context.chordCache.get(listing.key);
      const stub: ColumnAbsorberSpec = {
        kind: "column",
        name: `absorber:${listing.key}`,
        species: [
          { species: listing.key, columnPerM2: layer.columnPerM2, crossSection: undefined },
        ],
        columnPerM2: layer.columnPerM2,
        basePa: moments.basePa,
        topPa: moments.topPa,
        ...(layer.profile === undefined ? {} : { profile: layer.profile }),
        temperatureK,
      };
      const u = cached ?? amountValue(largestChordAmount(absorberTerm(stub, column), column));
      context.chordCache.set(listing.key, u);
      amount = { kind: "column", columnPerM2: u };
      members = [listing.key];
      break;
    }
    case "pair": {
      const split = pairMembers(listing.key, listed);
      if (split === undefined) {
        return undefined;
      }
      const [a, b] = split;
      if (a.profile !== undefined || b.profile !== undefined) {
        return undefined;
      }
      const ma = layerMoments(column, a.basePa, a.topPa, 1);
      const mb = layerMoments(column, b.basePa, b.topPa, 1);
      const basePa = Math.min(ma.basePa, mb.basePa);
      const topPa = Math.max(ma.topPa, mb.topPa);
      if (!(basePa > topPa)) {
        return undefined;
      }
      const overlap = layerMoments(column, basePa, topPa, 2);
      temperatureK = overlap.temperatureK;
      const pairColumnPerM5 =
        (a.columnPerM2 / ma.integralM) * (b.columnPerM2 / mb.integralM) * overlap.integralM;
      const cached = context.chordCache.get(listing.key);
      const stub: PairAbsorberSpec = {
        kind: "pair",
        name: `pair:${listing.key}`,
        pairs: [],
        pairColumnPerM5,
        basePa,
        topPa,
        temperatureK,
      };
      const u = cached ?? amountValue(largestChordAmount(absorberTerm(stub, column), column));
      context.chordCache.set(listing.key, u);
      amount = { kind: "pair", pairColumnPerM5: u };
      share = listing.part === undefined ? "whole" : `part:${listing.part}`;
      members = [a.species, b.species];
      break;
    }
  }
  const { cellMeans, pastNodes } = boundCellMeansAt(bound, temperatureK);
  const rule: MarginRule =
    bound.basis === "estimated" ? "estimatedData" : pastNodes ? "pastTemperatures" : "measured";
  return {
    entry: {
      origin: "pending",
      task: listing.task,
      part: listing.part,
      share,
      k: marginOf(rule),
      rule,
      temperatureK,
      amount,
    },
    cellMeans,
    members,
  };
}

/**
 * A pair key's two members among the listed layers: the split at a `+` whose two sides are both
 * listed, since an ion's key holds a `+` of its own (`Ca+`).
 */
function pairMembers(
  key: string,
  listed: ReadonlyMap<string, Listed>,
): readonly [AbsorberLayer, AbsorberLayer] | undefined {
  for (let i = key.indexOf("+"); i > 0; i = key.indexOf("+", i + 1)) {
    const a = listed.get(key.slice(0, i))?.layer;
    const b = listed.get(key.slice(i + 1))?.layer;
    if (a !== undefined && b !== undefined) {
      return [a, b];
    }
  }
  return undefined;
}

/** A key's {@link AbsorptionVerdict}: its entries' shares summed cell by cell, alternatives' largest. */
function absorptionVerdict(
  subject: Subject,
  weights: ReadonlyArray<BoundCellWeights>,
): AbsorptionVerdict {
  const shares = new Map<string, Float64Array>();
  for (const { entry, cellMeans } of subject.entries) {
    const depths = shares.get(entry.share) ?? new Float64Array(PENDING_CELL_COUNT);
    const scale = entry.k * amountValue(entry.amount);
    for (const [c, sigma] of cellMeans.entries()) {
      depths[c] = Math.max(depths[c] ?? 0, scale * sigma);
    }
    shares.set(entry.share, depths);
  }
  const total = new Float64Array(PENDING_CELL_COUNT);
  for (const depths of shares.values()) {
    for (const [c, x] of depths.entries()) {
      total[c] = (total[c] ?? 0) + x;
    }
  }
  const changes = weights.map((w) => channelChanges(w, total));
  const largestChange = Math.max(...changes.flat());
  return {
    test: "absorption",
    key: subject.key,
    entries: subject.entries.map(({ entry }) => entry),
    changes,
    largestChange,
    negligible: largestChange <= NEGLIGIBLE_CHANGE,
  };
}

/** A drawn line table's {@link HeldChiVerdict}. */
function heldChiVerdict(
  key: string,
  table: KDistributionTable | undefined,
  lines: SpeciesLines,
  cellMeans: Float64Array,
  amount: AbsorberAmount,
  weights: ReadonlyArray<BoundCellWeights>,
  at: { readonly temperatureK: number; readonly largestBroadeningPa: number },
): HeldChiVerdict {
  const gamma =
    table === undefined
      ? undefined
      : lorentzHalfWidthPerCm(table, at.temperatureK, at.largestBroadeningPa);
  const lorentzShare = lorentzShareBeyond(gamma);
  const u = amountValue(amount);
  const depths = new Float64Array(PENDING_CELL_COUNT);
  for (const cell of lines.kDistribution.cells) {
    const c = cell.cellNm - PENDING_CELLS_NM[0];
    depths[c] = lorentzShare * (cellMeans[c] ?? 0) * u;
  }
  const changes = weights.map((w) => channelChanges(w, depths, reachWeights(w, CHI_REACH_PER_CM)));
  const largestChange = Math.max(...changes.flat());
  return {
    test: "heldChi",
    key,
    lorentzShare,
    lorentzHalfWidthPerCm: gamma,
    amount,
    temperatureK: at.temperatureK,
    changes,
    largestChange,
    negligible: largestChange <= NEGLIGIBLE_CHANGE,
  };
}

/**
 * The most the lines' widths move for a share ε of the broadening pressure resting on gases taken
 * as air whose ratios lie in [0, 2], as |ln s|: −ln(1 − ε), capped at 1, where the bound on any
 * width takes over ({@link BroadenerVerdict}).
 */
function widthFactorOf(widthlessShare: number): number {
  return widthlessShare < 1 ? Math.min(1, -Math.log1p(-widthlessShare)) : 1;
}

/** A drawn line table's {@link BroadenerVerdict}, on its lines' cell means at k = 1. */
function broadenerVerdict(
  key: string,
  widthlessShare: number,
  cellMeans: Float64Array,
  amount: AbsorberAmount,
  weights: ReadonlyArray<BoundCellWeights>,
  at: { readonly temperatureK: number },
): BroadenerVerdict {
  const u = amountValue(amount);
  const depths = cellMeans.map((sigma) => sigma * u);
  const widthFactor = widthFactorOf(widthlessShare);
  const lineChanges = weights.map((w) => channelChanges(w, depths));
  const changes = lineChanges.map(([r, g, b]): Rgb => [
    widthFactor * r,
    widthFactor * g,
    widthFactor * b,
  ]);
  const largestChange = Math.max(...changes.flat());
  return {
    test: "broadener",
    key,
    widthlessShare,
    widthFactor,
    amount,
    temperatureK: at.temperatureK,
    lineChanges,
    changes,
    largestChange,
    negligible: largestChange <= NEGLIGIBLE_CHANGE,
  };
}

/** A drawn two-variant table's {@link FixedGMixVerdict}. */
function fixedGMixVerdict(
  key: string,
  table: KDistributionTable,
  selfShare: number,
  temperatureK: number,
  amount: AbsorberAmount,
): FixedGMixVerdict {
  const u = amountValue(amount);
  let largestColumnRatio = 0;
  for (let c = 0; c < table.cells.length; c += 1) {
    const limit = mixLimitColumnM2(table, c, selfShare, temperatureK);
    largestColumnRatio = Math.max(largestColumnRatio, u / limit);
  }
  return {
    test: "fixedGMix",
    key,
    selfShare,
    temperatureK,
    amount,
    largestColumnRatio,
    negligible: largestColumnRatio < 1,
  };
}

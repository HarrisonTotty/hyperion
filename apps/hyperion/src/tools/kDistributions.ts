/**
 * The k-distributions of R08.T4.d (plan R08, R08.T4.d; Design note 5): each 1 nm cell's
 * absorption cross-section, reduced from its lines on a grid of temperature and broadening
 * pressure to a few g-points whose cross-sections reproduce the cell's curve of growth.
 *
 * @remarks
 * A cell's cross-section σ(λ), sorted, is its k-distribution σ(g), g ∈ [0, 1] the fraction of the
 * cell below σ: the cell's mean transmittance through a column u is T(u) = ∫₀¹ e^(−σ(g)u) dg
 * exactly, for any homogeneous path (A. A. Lacis and V. Oinas, J. Geophys. Res. 96 (1991) 9027,
 * §2). The reduction keeps intervals of g, the same at every node of the grid (correlated-k, their
 * §3), and in each interval two Gauss–Legendre points of weight Δg ÷ 2 whose σ is read from the
 * sorted spectrum at the rule's abscissae and scaled so that the pair keeps the interval's mean σ
 * (so the curve of growth's linear regime is exact):
 *
 *   T_k(u) = Σ_I ½Δg_I (e^(−σ_I1 u) + e^(−σ_I2 u)).
 *
 * The intervals are found by bisection: from [0, 1], the interval that contributes most to the
 * largest bound on |T(u) − T_k(u)|, the sum of the intervals' own errors' magnitudes, over every
 * node and column is split where ln σ is halfway across it, until the bound is at most the target
 * at every node, first on {@link SEARCH_COLUMNS_PER_DECADE} columns a decade and then on
 * {@link VERIFY_COLUMNS_PER_DECADE}.
 */

import {
  cellPointCount,
  cellSpectrum,
  cellWavenumbersPerCm,
  FOREIGN_BROADENERS,
  LINE_CUTOFF_PER_CM,
  type LineList,
  type LineShape,
  type LineSpecies,
  lineStates,
  linesNear,
  MOLAR_GAS_CONSTANT_J_PER_MOL_K,
  SECOND_RADIATION_CONSTANT_CM_K,
} from "./molecularLines";

/** The columns per decade at which the bisection searches for each node's worst error. */
export const SEARCH_COLUMNS_PER_DECADE = 6;

/** The columns per decade at which the reduction verifies the target before it stops. */
export const VERIFY_COLUMNS_PER_DECADE = 24;

/** One node's sorted cross-sections and their running sums. */
export interface SortedNode {
  /** σ ascending, cm² a molecule, one value a point of equal weight. */
  readonly sorted: Float64Array;
  /** prefix[i] = Σ of the first i values. */
  readonly prefix: Float64Array;
}

/** A node's sorted cross-sections, with their prefix sums. */
export function sortedNode(crossSections: Float64Array): SortedNode {
  const sorted = Float64Array.from(crossSections).toSorted();
  const prefix = new Float64Array(sorted.length + 1);
  for (let i = 0; i < sorted.length; i += 1) {
    prefix[i + 1] = (prefix[i] ?? 0) + (sorted[i] ?? 0);
  }
  return { sorted, prefix };
}

/**
 * The mean σ over g ∈ [a, b) of a node, each point covering [i ÷ M, (i + 1) ÷ M), the ends
 * weighted by their overlap.
 */
export function intervalMean(node: SortedNode, a: number, b: number): number {
  const m = node.sorted.length;
  const lo = a * m;
  const hi = b * m;
  const i0 = Math.floor(lo);
  const i1 = Math.min(Math.ceil(hi), m);
  if (i1 - i0 === 1) {
    return node.sorted[i0] ?? Number.NaN;
  }
  const inner = (node.prefix[Math.floor(hi)] ?? 0) - (node.prefix[Math.ceil(lo)] ?? 0);
  const head = (Math.ceil(lo) - lo) * (node.sorted[i0] ?? 0);
  const tail = Math.floor(hi) < m ? (hi - Math.floor(hi)) * (node.sorted[Math.floor(hi)] ?? 0) : 0;
  return (inner + head + tail) / (hi - lo);
}

/** ∫ over g ∈ [a, b) of e^(−σ(g)u) for a node, the ends weighted by their overlap. */
export function intervalTransmittance(node: SortedNode, a: number, b: number, u: number): number {
  const m = node.sorted.length;
  const lo = a * m;
  const hi = b * m;
  const i0 = Math.floor(lo);
  const i1 = Math.min(Math.ceil(hi), m);
  let sum = 0;
  for (let i = i0; i < i1; i += 1) {
    const w = Math.min(i + 1, hi) - Math.max(i, lo);
    sum += w * Math.exp(-(node.sorted[i] ?? 0) * u);
  }
  return sum / m;
}

/**
 * The columns at which a node's error is measured, cm⁻²: from where ½ u² ⟨σ²⟩, the error's bound in
 * the linear regime, is a tenth of the target, to where the cell's own transmittance falls below
 * half the target (beyond, the error is under it, since both curves lie in [0, T(u)] there to
 * within the target), `perDecade` a decade, and at most `maxColumnCm2`.
 */
export function errorColumns(
  node: SortedNode,
  target: number,
  perDecade: number,
  maxColumnCm2: number,
): Float64Array {
  const m = node.sorted.length;
  let squares = 0;
  for (const s of node.sorted) {
    squares += s * s;
  }
  const meanSquare = squares / m;
  if (!(meanSquare > 0)) {
    return new Float64Array(0);
  }
  const first = Math.sqrt((0.2 * target) / meanSquare);
  const columns: number[] = [];
  for (let k = 0; k < 40 * perDecade; k += 1) {
    const u = first * 10 ** (k / perDecade);
    if (u > maxColumnCm2) {
      break;
    }
    columns.push(u);
    if (intervalTransmittance(node, 0, 1, u) < 0.5 * target) {
      break;
    }
  }
  return Float64Array.from(columns);
}

/** σ(g) of a node's staircase at a g in [0, 1): the value of the point that covers it. */
function sigmaAtG(node: SortedNode, g: number): number {
  const m = node.sorted.length;
  return node.sorted[Math.min(Math.floor(g * m), m - 1)] ?? Number.NaN;
}

/** The two Gauss–Legendre abscissae of an interval [a, b), as fractions of it: ½ ∓ 1 ÷ (2√3). */
const GAUSS_FRACTIONS = [0.5 - 0.5 / Math.sqrt(3), 0.5 + 0.5 / Math.sqrt(3)] as const;

/**
 * An interval's two g-points at a node: σ at its Gauss–Legendre abscissae, each of weight Δg ÷ 2,
 * both scaled by the interval's mean over their mean, so that the pair keeps the interval's mean
 * σ exactly; a pair whose mean is 0 takes the interval's mean.
 */
export function intervalPoints(node: SortedNode, a: number, b: number): readonly [number, number] {
  const width = b - a;
  const s1 = sigmaAtG(node, a + GAUSS_FRACTIONS[0] * width);
  const s2 = sigmaAtG(node, a + GAUSS_FRACTIONS[1] * width);
  const mean = intervalMean(node, a, b);
  const pairMean = 0.5 * (s1 + s2);
  if (!(pairMean > 0)) {
    return [mean, mean];
  }
  const scale = mean / pairMean;
  return [s1 * scale, s2 * scale];
}

/**
 * Where an interval [a, b) is split: at the g where ln σ of the node that errs most is halfway
 * between its values at the interval's ends, since the two-point rule's error grows with the range
 * of ln σ an interval spans (the integrand e^(−σu), as a function of ln σ, has derivatives bounded
 * independently of u). Where σ is 0 at the lower end, or the halfway g falls within a point of
 * either end, the middle of [a, b) instead.
 */
function splitPoint(node: SortedNode | undefined, a: number, b: number): number {
  const middle = 0.5 * (a + b);
  if (node === undefined) {
    return middle;
  }
  const m = node.sorted.length;
  const i0 = Math.floor(a * m);
  const i1 = Math.min(Math.ceil(b * m), m) - 1;
  const lo = node.sorted[i0] ?? 0;
  const hi = node.sorted[i1] ?? 0;
  if (!(lo > 0 && hi > lo)) {
    return middle;
  }
  const half = Math.sqrt(lo * hi);
  let low = i0;
  let high = i1;
  while (low < high) {
    const mid = (low + high) >> 1;
    if ((node.sorted[mid] ?? 0) < half) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }
  const g = low / m;
  const minimum = 1 / m;
  return g - a < minimum || b - g < minimum ? middle : g;
}

/** A cell's reduced k-distribution: its g-points and each node's σ at them. */
export interface CellReduction {
  /** The g-points' weights, summing to 1, ascending in g: two a g-interval, each half its width. */
  readonly weights: Float64Array;
  /** σ per node and g-point, cm² a molecule: `values[node][point]`. */
  readonly values: ReadonlyArray<Float64Array>;
  /** The largest bound on the error left at any node, on the verification's columns. */
  readonly worstError: number;
}

/**
 * The intervals' edges after bisection from `edges` until the error bound on `columns` is at most
 * the target at every node, with that bound.
 *
 * @throws Error if bisection reaches an interval narrower than a point of the finest node.
 */
function bisect(
  nodes: ReadonlyArray<SortedNode>,
  columns: ReadonlyArray<Float64Array>,
  target: number,
  initialEdges: ReadonlyArray<number>,
): { readonly edges: number[]; readonly worst: number } {
  // Each interval's error magnitude at each node and column, kept so that a split recomputes only
  // its own two halves.
  const intervalError = (a: number, b: number): Float64Array[] =>
    nodes.map((node, n) => {
      const [s1, s2] = intervalPoints(node, a, b);
      const half = 0.5 * (b - a);
      return (columns[n] ?? new Float64Array(0)).map((u) =>
        Math.abs(
          intervalTransmittance(node, a, b, u) - half * (Math.exp(-s1 * u) + Math.exp(-s2 * u)),
        ),
      );
    });
  let edges = [...initialEdges];
  let errors = edges.slice(1).map((b, i) => intervalError(edges[i] ?? 0, b));
  const finest = Math.max(...nodes.map((node) => node.sorted.length));
  for (;;) {
    let worst = 0;
    let worstNode = 0;
    let worstColumn = 0;
    for (const [n, cols] of columns.entries()) {
      for (let c = 0; c < cols.length; c += 1) {
        let total = 0;
        for (const e of errors) {
          total += e[n]?.[c] ?? 0;
        }
        if (total > worst) {
          worst = total;
          worstNode = n;
          worstColumn = c;
        }
      }
    }
    if (worst <= target) {
      return { edges, worst };
    }
    let split = 0;
    let largest = -Infinity;
    for (const [i, e] of errors.entries()) {
      const value = e[worstNode]?.[worstColumn] ?? 0;
      if (value > largest) {
        largest = value;
        split = i;
      }
    }
    const a = edges[split] ?? 0;
    const b = edges[split + 1] ?? 1;
    if ((b - a) * finest < 1) {
      throw new Error(
        `a cell's interval [${a}, ${b}) holds under a point and still errs by ${worst}`,
      );
    }
    const middle = splitPoint(nodes[worstNode], a, b);
    edges = [...edges.slice(0, split + 1), middle, ...edges.slice(split + 1)];
    errors = [
      ...errors.slice(0, split),
      intervalError(a, middle),
      intervalError(middle, b),
      ...errors.slice(split + 1),
    ];
  }
}

/**
 * The g-intervals shared by every node of a cell, by bisection to the target on
 * {@link SEARCH_COLUMNS_PER_DECADE} columns a decade and then on
 * {@link VERIFY_COLUMNS_PER_DECADE}, and each node's two Gauss–Legendre points in each
 * ({@link intervalPoints}).
 *
 * @param capsCm2 - Each node's largest column the error is measured to, cm⁻² (its gated column,
 *   {@link columnCapCm2} and {@link rayleighCapCm2}): every column where absent.
 * @throws Error if bisection reaches an interval narrower than a point of the finest node without
 *   meeting the target: a broken invariant, since an interval of one point has no error.
 */
export function reduceCell(
  nodes: ReadonlyArray<SortedNode>,
  target: number,
  capsCm2: ReadonlyArray<number> = [],
): CellReduction {
  const columnsAt = (perDecade: number): Float64Array[] =>
    nodes.map((node, n) => errorColumns(node, target, perDecade, capsCm2[n] ?? Infinity));
  const searched = bisect(nodes, columnsAt(SEARCH_COLUMNS_PER_DECADE), target, [0, 1]);
  const { edges, worst } = bisect(
    nodes,
    columnsAt(VERIFY_COLUMNS_PER_DECADE),
    target,
    searched.edges,
  );
  const widths = edges.slice(1).map((b, i) => b - (edges[i] ?? 0));
  return {
    weights: Float64Array.from(widths.flatMap((w) => [0.5 * w, 0.5 * w])),
    values: nodes.map((node) =>
      Float64Array.from(edges.slice(1).flatMap((b, i) => intervalPoints(node, edges[i] ?? 0, b))),
    ),
    worstError: worst,
  };
}

/**
 * A cell stored as its mean σ alone (G = 1, weight 1) at each node, with the largest error that
 * leaves on each node's columns to its cap.
 */
export function linearCell(
  nodes: ReadonlyArray<SortedNode>,
  capsCm2: ReadonlyArray<number>,
): CellReduction {
  let worst = 0;
  const values = nodes.map((node, n) => {
    const mean = intervalMean(node, 0, 1);
    for (const u of errorColumns(
      node,
      REDUCTION_TARGET,
      VERIFY_COLUMNS_PER_DECADE,
      capsCm2[n] ?? Infinity,
    )) {
      worst = Math.max(worst, Math.abs(intervalTransmittance(node, 0, 1, u) - Math.exp(-mean * u)));
    }
    return Float64Array.of(mean);
  });
  return { weights: Float64Array.of(1), values, worstError: worst };
}

/** The temperature and broadening-pressure grid a species is reduced on. */
export interface ReductionGrid {
  /** K, ascending. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** Air-equivalent broadening pressures, Pa, ascending. */
  readonly pressuresPa: ReadonlyArray<number>;
}

/**
 * The stored pressures on the air-equivalent broadening pressure (`decision-r08-t4d-kdist.md`,
 * ruling 2; `decision-r08-t4d-kdist-chain.md` §8): 4 a decade from 10 Pa to 1 MPa, 8 a decade from
 * 1 to 10 MPa, and one node at 13.3 MPa only to centre the read's stencil, `estimated`: 30 nodes.
 * Below the first the 10 Pa table is held; above 10 MPa the read holds p̄ there.
 */
export const STORED_PRESSURES_PA: ReadonlyArray<number> = [
  ...Array.from({ length: 21 }, (_, k) => 10 * 10 ** (k / 4)),
  ...Array.from({ length: 9 }, (_, k) => 1e6 * 10 ** ((k + 1) / 8)),
];

/** The highest broadening pressure the read takes, Pa: above it p̄ is held (§8). */
export const READ_TOP_PA = 1e7;

/**
 * The plan's per-cell bound at a stored node, 10⁻³ in T over every column (R08.T4.d's tests;
 * `decision-r08-t4d-kdist.md`, ruling 1).
 */
export const NODE_BOUND = 1e-3;

/**
 * The bound between nodes, through the client's whole chain, 2 × 10⁻³ in T over every column: the
 * channel bound (`decision-r08-t4d-kdist.md`, ruling 1).
 */
export const CHAIN_BOUND = 2e-3;

/**
 * The bound on what storing ln σ in 16 bits moves a curve, 2 × 10⁻⁴ (`decision-r08-t4d-kdist.md`,
 * its tests).
 */
export const QUANTISATION_BOUND = 2e-4;

/**
 * The reduction's own target at a node: {@link NODE_BOUND} less {@link QUANTISATION_BOUND}, so that
 * the stored table meets the node tier.
 */
export const REDUCTION_TARGET = NODE_BOUND - QUANTISATION_BOUND;

/** The columns per decade at which the chain test measures each midpoint's error. */
export const CHAIN_COLUMNS_PER_DECADE = 12;

/**
 * A far-wing factor for one pair: χ = 1 within σ₁ of a line's centre, then exponential with slope
 * B₁ to σ₂, B₂ to σ₃ and B₃ beyond (H. Tran et al., Icarus 306 (2018) 116, arXiv:1802.01352,
 * Eq. 5, the piecewise form's open statement), each B = α + β e^(−εT), cm (M. Chaverot et al.,
 * Astron. Astrophys. 702 (2025) A137, DOI 10.1051/0004-6361/202555762, Eq. 12, after Perrin and
 * Hartmann 1989). The sets are fitted in CO₂'s ν₃ band near 4.3 µm and taken here for its visible
 * bands, as SpeCT takes its ν₃ sets for every band beyond 4,300 cm⁻¹, for want of visible data: an
 * extrapolation, recorded.
 *
 * @remarks
 * CO₂'s two sets are SpeCT's ν₃-band sets (Chaverot et al. 2025, Tables 2 and 3; CC BY 4.0),
 * reduced values with citation (`decision-r08-t4d-kdist-chain.md` §§12–13), each defined against
 * SpeCT's Eq. 11 line shape ({@link ChiWingShape}). The version of record was read from HAL's
 * deposit, hal-05308891.
 *
 * - CO₂–CO₂ rests on Tran et al. 2011's and Tran et al. 2024's measurements and Burch et al. 1969's,
 *   fitted at 218, 295, 473 and 751 K (their Fig. 3); it is held outside 218–751 K.
 * - CO₂–N₂ is Perrin and Hartmann 1989's (J. Quant. Spectrosc. Radiat. Transfer 42, 311, DOI
 *   10.1016/0022-4073(89)90077-0) "marginally adapted" (their §5.1.2), on Perrin and Hartmann's
 *   296–773 K and Burch et al.'s 295 K measurements; it is held outside 295–773 K. C. Cousin et
 *   al. 1985's 193–238 K measurements (Appl. Opt. 24, 3899) are not in SpeCT's compilation, so the
 *   low hold stays at 295 K (§13, ruling 3).
 * - **A misprint corrected.** Table 2 prints CO₂–N₂'s ν₃ B₂ α as 0.0167. SpeCT's own code reads
 *   0.00167 (gitlab.com/ChaverotG/spect-public, commit e7c4579c08, `fcts.f90`, function
 *   `chi_CO2N2`, its ν₃ branch: `B2 = 0.00167d0 + 0.0421d0 * dexp(-0.00248d0 * T)`), Perrin and
 *   Hartmann's own value, as "marginally adapted" implies: only B₃ moves, 0.0200 to 0.019. The
 *   repository states no licence, so it is a finding aid only, one number read and no code copied
 *   or run. Against SpeCT's lab compilation (Zenodo 10.5281/zenodo.15564294) with HITRAN2024's
 *   lines, the printed value errs −0.70 in mean ln(model ÷ lab) at Perrin and Hartmann's 296 K and
 *   −0.43 against Burch et al., the coded one +0.04 and +0.33.
 * - **The science check** (§12, ruling 4), both sets with HITRAN2024's lines against that
 *   compilation: CO₂–CO₂ errs +0.04 (rms 0.11) against Tran et al. 2024's 295 K data, where Perrin
 *   and Hartmann's self set errs −0.71 (rms 1.10), and SpeCT's on a plain Voigt −0.42 (rms 0.50);
 *   CO₂–N₂ ties with Perrin and Hartmann's (aggregate rms 0.417 against 0.375) within the lab sets'
 *   own 0.15–0.45 disagreement. The compilation's CO₂–CO₂ ν₃ data are at 295 K only, so SpeCT's
 *   fits at 218, 473 and 751 K are untested here.
 */
export interface ChiFactorSet {
  /** The absorber and its broadener, as `CO2-CO2`. */
  readonly pair: string;
  /** The line shape the set was fitted under, which its χ multiplies ({@link chiWingFactor}). */
  readonly wingShape: ChiWingShape;
  /**
   * The temperatures of the measurements the set is fitted to, K: its coefficients are held
   * beyond them, and a node there is `estimated` where χ is visible ({@link reduceGridCell}).
   */
  readonly measuredRangeK: readonly [number, number];
  /** σ₁, σ₂ and σ₃, cm⁻¹. */
  readonly breaksPerCm: readonly [number, number, number];
  /** α, β and ε of B₁–B₃: cm, cm and K⁻¹. */
  readonly alphaCm: readonly [number, number, number];
  readonly betaCm: readonly [number, number, number];
  readonly epsilonPerK: readonly [number, number, number];
}

/**
 * How a χ set's factor meets a line's profile: `lorentz`, χ times the profile, as Perrin and
 * Hartmann 1989 fitted theirs; `quantumAsymmetric`, χ times the profile times SpeCT's quantum
 * asymmetry and detailed-balance factors wherever χ ≠ 1 (Chaverot et al. 2025, Eq. 11), as SpeCT
 * fitted its sets (their §5.1: "re-adjust them to guarantee a correct match with the change of
 * asymmetry factor"). The asymmetry is the fluctuation–dissipation theorem's (Tran et al. 2018,
 * Eq. 3).
 */
export type ChiWingShape = "lorentz" | "quantumAsymmetric";

/** CO₂ broadened by CO₂: SpeCT's ν₃ set ({@link ChiFactorSet}), fitted at 218–751 K. */
export const CHI_CO2_CO2: ChiFactorSet = {
  pair: "CO2-CO2",
  wingShape: "quantumAsymmetric",
  measuredRangeK: [218, 751],
  breaksPerCm: [3, 50, 220],
  alphaCm: [0.055, 0.021, 0.01],
  betaCm: [-0.11, 0.08, 0],
  epsilonPerK: [0.008, 0.0095, 0],
};

/**
 * CO₂ broadened by N₂: SpeCT's ν₃ set as its code reads it, B₂'s α 0.00167 for Table 2's misprinted
 * 0.0167 ({@link ChiFactorSet}), fitted at 295–773 K.
 */
export const CHI_CO2_N2: ChiFactorSet = {
  pair: "CO2-N2",
  wingShape: "quantumAsymmetric",
  measuredRangeK: [295, 773],
  breaksPerCm: [3, 10, 70],
  alphaCm: [0.416, 0.00167, 0.019],
  betaCm: [-0.354, 0.0421, 0],
  epsilonPerK: [0.00386, 0.00248, 0],
};

/** B₁–B₃ at a temperature, cm, held beyond the set's measured temperatures and not clamped. */
export function chiSlopes(
  set: ChiFactorSet,
  temperatureK: number,
): readonly [number, number, number] {
  const [lowK, highK] = set.measuredRangeK;
  const t = Math.min(Math.max(temperatureK, lowK), highK);
  const b = (k: 0 | 1 | 2): number =>
    set.alphaCm[k] + set.betaCm[k] * Math.exp(-set.epsilonPerK[k] * t);
  return [b(0), b(1), b(2)];
}

/** χ at an offset from a line's centre, cm⁻¹, and a temperature, K ({@link ChiFactorSet}). */
export function chiFactor(set: ChiFactorSet, temperatureK: number, offsetPerCm: number): number {
  const [s1, s2, s3] = set.breaksPerCm;
  const [b1, b2, b3] = chiSlopes(set, temperatureK);
  const a = Math.abs(offsetPerCm);
  if (a <= s1) {
    return 1;
  }
  if (a <= s2) {
    return Math.exp(-b1 * (a - s1));
  }
  if (a <= s3) {
    return Math.exp(-b1 * (s2 - s1) - b2 * (a - s2));
  }
  return Math.exp(-b1 * (s2 - s1) - b2 * (s3 - s2) - b3 * (a - s3));
}

/**
 * The far-wing factor a χ set multiplies a line's profile by, at a signed offset ν − ν₀ from its
 * centre ν₀ (cm⁻¹) and a temperature: χ alone for a `lorentz` set; for a `quantumAsymmetric` one,
 * beyond σ₁, where χ ≠ 1, χ times the quantum asymmetry factor x ÷ (1 − e^(−x)), x = c₂ (ν − ν₀) ÷
 * T, and the detailed-balance factor ν (1 − e^(−c₂ν/T)) ÷ (ν₀ (1 − e^(−c₂ν₀/T))) (Chaverot et al.
 * 2025, Eq. 11; their Eq. 10, the core within σ₁, takes neither). The coefficients are held as
 * {@link chiSlopes} holds them; the asymmetry is taken at the temperature itself.
 */
export function chiWingFactor(
  set: ChiFactorSet,
  temperatureK: number,
  offsetPerCm: number,
  centrePerCm: number,
): number {
  const chi = chiFactor(set, temperatureK, offsetPerCm);
  let asymmetric: boolean;
  switch (set.wingShape) {
    case "lorentz":
      asymmetric = false;
      break;
    case "quantumAsymmetric":
      asymmetric = Math.abs(offsetPerCm) > set.breaksPerCm[0];
      break;
  }
  if (!asymmetric) {
    return chi;
  }
  const c2 = SECOND_RADIATION_CONSTANT_CM_K;
  const x = (c2 * offsetPerCm) / temperatureK;
  const asymmetry = x / -Math.expm1(-x);
  const nu = centrePerCm + offsetPerCm;
  const balance =
    (nu * -Math.expm1((-c2 * nu) / temperatureK)) /
    (centrePerCm * -Math.expm1((-c2 * centrePerCm) / temperatureK));
  return chi * asymmetry * balance;
}

/**
 * Which of a policy's variants hold their χ at each temperature node: those whose set's fit
 * temperatures leave the node outside ({@link ChiFactorSet.measuredRangeK}), `[t][v]`; a variant
 * without χ holds none. The sidecar carries them for R08.T10.a, which bounds a held χ's effect on
 * the body's own column (`decision-r08-t4d-kdist-chain.md` §14).
 */
export function chiHeldFlags(
  policy: ShapePolicy,
  temperaturesK: ReadonlyArray<number>,
): boolean[][] {
  return temperaturesK.map((t) =>
    policy.variants.map(
      (v) => v.chi !== undefined && (t < v.chi.measuredRangeK[0] || t > v.chi.measuredRangeK[1]),
    ),
  );
}

/** The χ below which a far wing is cut (`decision-r08-t4d-kdist-chain.md`, ruling 2). */
export const CHI_FLOOR = 1e-3;

/**
 * The fixed distance at which a χ-factor shape is cut, cm⁻¹: the least whole offset beyond which
 * χ < {@link CHI_FLOOR} at every temperature of [fromK, toK], by 1 K steps, so that the shape's
 * reach does not move with p or T.
 */
export function chiReachPerCm(set: ChiFactorSet, fromK: number, toK: number): number {
  const [s1, s2, s3] = set.breaksPerCm;
  const depth = -Math.log(CHI_FLOOR);
  let reach = 0;
  for (let t = fromK; t <= toK; t += 1) {
    const [b1, b2, b3] = chiSlopes(set, t);
    const atS2 = b1 * (s2 - s1);
    const atS3 = atS2 + b2 * (s3 - s2);
    const a =
      atS2 >= depth
        ? s1 + depth / b1
        : atS3 >= depth
          ? s2 + (depth - atS2) / b2
          : s3 + (depth - atS3) / b3;
    reach = Math.max(reach, a);
  }
  return Math.ceil(reach);
}

/**
 * One far-wing shape a species' table is reduced for: the broadener it stands for in the client's
 * mix (`self`, or `air` for every other gas), and its χ, or none.
 */
export interface ShapeVariant {
  readonly broadener: "self" | "air";
  readonly chi?: ChiFactorSet;
}

/**
 * A species' line shape, the same at every pressure (`decision-r08-t4d-kdist-chain.md`, ruling 1):
 * Voigt within `cutoffPerCm` of each line's centre, times each variant's χ where it has one; whether
 * each line's pedestal (its profile at the cut-off) is subtracted, CKD's convention; and the
 * Lorentzian half-width above which a node is `estimated`.
 */
export interface ShapePolicy {
  /** One shape for a species without a published far wing; CO₂'s two, CO₂–CO₂'s and CO₂–N₂'s χ. */
  readonly variants: ReadonlyArray<ShapeVariant>;
  /** 25 cm⁻¹ ({@link LINE_CUTOFF_PER_CM}), or a χ shape's reach ({@link chiReachPerCm}). */
  readonly cutoffPerCm: number;
  readonly pedestal: boolean;
  /** cm⁻¹; nodes whose cell's S-weighted γ_L passes it are `estimated`. */
  readonly estimatedAboveHalfWidthPerCm: number;
}

/**
 * The line shape at a temperature for a mix of a policy's variants, `weights[v]` of each (one 1 for
 * a variant's own shape): the one place the shape is set. χ is Σ_v w_v χ_v, each variant without a
 * χ counting 1.
 */
export function lineShape(
  policy: ShapePolicy,
  temperatureK: number,
  weights: ReadonlyArray<number>,
): LineShape {
  const parts = policy.variants
    .map((v, i) => ({ chi: v.chi, w: weights[i] ?? 0 }))
    .filter((p) => p.w > 0);
  if (parts.every((p) => p.chi === undefined)) {
    return { cutoffPerCm: policy.cutoffPerCm, pedestal: policy.pedestal };
  }
  const breaks = [...new Set(parts.flatMap((p) => p.chi?.breaksPerCm ?? []))].toSorted(
    (a, b) => a - b,
  );
  return {
    cutoffPerCm: policy.cutoffPerCm,
    pedestal: policy.pedestal,
    farWing: {
      factor: (offset, centre) =>
        parts.reduce(
          (sum, p) =>
            sum +
            p.w * (p.chi === undefined ? 1 : chiWingFactor(p.chi, temperatureK, offset, centre)),
          0,
        ),
      breaksPerCm: breaks,
    },
  };
}

/** One variant's own weights: 1 for it, 0 for the others. */
function only(policy: ShapePolicy, variant: number): number[] {
  return policy.variants.map((_, i) => (i === variant ? 1 : 0));
}

/** A cell's node: its sorted spectrum, its S-weighted γ_L, and what its shape leaves out. */
export interface CellNode {
  readonly node: SortedNode;
  /** Whether the line shape there is `estimated`: γ_L past the policy's half-width. */
  readonly estimatedShape: boolean;
  /** The cell's intensity-weighted Lorentzian half-width, cm⁻¹. */
  readonly lorentzPerCm: number;
  /**
   * The share of a Lorentz line's strength beyond the cut-off c, 2 γ_L ÷ (π c) times χ there: the
   * most a far wing would add (`decision-r08-t4d-kdist-chain.md` §7(d)).
   */
  readonly beyondCutoffShare: number;
  /**
   * The share 3–25 cm⁻¹ from the centre, (2 ÷ π)[arctan(25 ÷ γ_L) − arctan(3 ÷ γ_L)], which a
   * sub-Lorentzian wing would reduce (§7(d)).
   */
  readonly nearWingShare: number;
}

/** Where a published χ first departs from 1, cm⁻¹: the inner edge of the near-wing share. */
export const NEAR_WING_FROM_PER_CM = 3;

/** The intensity-weighted Lorentzian half-width of a set of line states, cm⁻¹. */
function meanLorentzPerCm(
  states: ReadonlyArray<{ intensityCm: number; lorentzPerCm: number }>,
): number {
  let weighted = 0;
  let total = 0;
  for (const s of states) {
    weighted += s.intensityCm * s.lorentzPerCm;
    total += s.intensityCm;
  }
  return total > 0 ? weighted / total : 0;
}

/**
 * Where a cell's spectrum at a node comes from (R08.T4.g): one node's sorted spectrum, its
 * S-weighted γ_L and its shares, for a mix of the policy's variants (one 1 for a variant's own
 * shape), or `undefined` where no line lies within the cut-off of the cell. T4.d's is line by line
 * ({@link lineByLine}); ExoMol's lists take super-lines (`superLines.ts`). The reduction and its
 * tests read every source alike ({@link reduceGridCell}, {@link belowGridError}).
 */
export type NodeSpectra = (
  cellNm: number,
  temperatureK: number,
  pressurePa: number,
  weights: ReadonlyArray<number>,
) => CellNode | undefined;

/** A species' lines as a {@link NodeSpectra}, line by line under the policy's shape ({@link cellNode}). */
export function lineByLine(species: LineSpecies, policy: ShapePolicy): NodeSpectra {
  return (cellNm, temperatureK, pressurePa, weights) =>
    cellNode(cellNm, species, temperatureK, pressurePa, policy, weights);
}

/**
 * A cell's sorted line-by-line spectrum at a temperature and broadening pressure, with the policy's
 * shape for a mix of its variants ({@link lineShape}), or `undefined` where no line lies within the
 * cut-off of the cell.
 */
export function cellNode(
  cellNm: number,
  species: LineSpecies,
  temperatureK: number,
  pressurePa: number,
  policy: ShapePolicy,
  weights: ReadonlyArray<number>,
): CellNode | undefined {
  const shape = lineShape(policy, temperatureK, weights);
  const [low, high] = cellWavenumbersPerCm(cellNm);
  const indices = linesNear(species.lines, low, high, shape.cutoffPerCm);
  if (indices.length === 0) {
    return undefined;
  }
  const states = lineStates(species, indices, temperatureK, pressurePa);
  const points = cellPointCount(cellNm, states);
  return cellNodeOf(
    cellNm,
    cellSpectrum(cellNm, species, indices, states, points, shape).crossSectionsCm2,
    meanLorentzPerCm(states),
    shape,
    policy,
  );
}

/**
 * A cell's node from its spectrum and its intensity-weighted γ_L (cm⁻¹), under a shape and policy:
 * the sorted spectrum, whether the shape is `estimated` (γ_L past the policy's half-width, or
 * `widthsEstimated`, lines whose widths are a stated estimate, R08.T4.g), and its two Lorentz
 * shares, the far wing's factor at the cut-off taken as the mean of its two sides at the cell's
 * centre.
 */
export function cellNodeOf(
  cellNm: number,
  crossSectionsCm2: Float64Array,
  lorentzPerCm: number,
  shape: LineShape,
  policy: ShapePolicy,
  widthsEstimated = false,
): CellNode {
  const gamma = lorentzPerCm;
  return {
    node: sortedNode(crossSectionsCm2),
    estimatedShape: widthsEstimated || gamma > policy.estimatedAboveHalfWidthPerCm,
    lorentzPerCm: gamma,
    beyondCutoffShare:
      ((2 * gamma) / (Math.PI * shape.cutoffPerCm)) *
      (shape.farWing === undefined
        ? 1
        : 0.5 *
          (shape.farWing.factor(shape.cutoffPerCm, 1e7 / cellNm) +
            shape.farWing.factor(-shape.cutoffPerCm, 1e7 / cellNm))),
    nearWingShare:
      gamma > 0
        ? (2 / Math.PI) *
          (Math.atan(LINE_CUTOFF_PER_CM / gamma) - Math.atan(NEAR_WING_FROM_PER_CM / gamma))
        : 0,
  };
}

/**
 * Each broadening gas's ratio to air for a species' lines, with `self` for the molecule's own
 * (`decision-r08-t4d-kdist.md` §5.3): r_j = Σ Sᵢ γ_j,i ÷ Σ Sᵢ γ_air,i over the lines HITRAN2024
 * gives γ_j for, at 296 K. A gas HITRAN gives for no line is left out, so that the client takes it
 * as air and `estimated`. Each gas's temperature exponent enters beside it
 * ({@link broadeningExponentOffsets}).
 */
export function broadeningRatios(lines: LineList): Readonly<Record<string, number>> {
  const ratioOf = (widths: Float64Array): number | undefined => {
    let given = 0;
    let air = 0;
    for (let i = 0; i < widths.length; i += 1) {
      const gamma = widths[i] ?? Number.NaN;
      const s = lines.intensitiesCm[i] ?? 0;
      if (Number.isFinite(gamma)) {
        given += s * gamma;
        air += s * (lines.airHalfWidthsPerCmAtm[i] ?? Number.NaN);
      }
    }
    return air > 0 ? given / air : undefined;
  };
  const ratios: Record<string, number> = {};
  const self = ratioOf(lines.selfHalfWidthsPerCmAtm);
  if (self !== undefined) {
    ratios["self"] = self;
  }
  for (const gas of FOREIGN_BROADENERS) {
    const r = ratioOf(lines.foreignHalfWidthsPerCmAtm[gas]);
    if (r !== undefined) {
      ratios[gas] = r;
    }
  }
  return ratios;
}

/**
 * The least exponent offset {@link broadeningExponentOffsets} keeps: 10⁻⁶. Where a gas's widths and
 * exponents are air's to their printed digits the two means differ by those digits alone (H₂'s
 * γ_H2 and n_H2, HITRAN2024's γ_air and n_air so, leave −7.5 × 10⁻¹²). An offset this small moves
 * (296 ÷ T)^Δn from 1 by under 2 × 10⁻⁶ over 50–500 K, a five-hundredth of the node tier.
 */
export const EXPONENT_OFFSET_FLOOR = 1e-6;

/**
 * Each broadening gas's temperature exponent over air's for a species' lines, with `self` for the
 * molecule's own (`decision-r08-t4d-kdist-chain.md` §13, ruling 4): n̄_j − n̄_air, each the
 * Sᵢ γᵢ-weighted mean exponent over the lines HITRAN2024 gives γ_j for, n_air standing for a line's
 * n_j where HITRAN gives none. The client's broadening pressure is then
 * p_eff = p Σ_j x_j r_j (296 ÷ T)^(n̄_j − n̄_air) at the profile's temperature, the tables staying
 * built on γ_air (296 ÷ T)^(n_air): r_j (296 ÷ T)^(n̄_j − n̄_air) is Σ Sγ_j (296 ÷ T)^(n_j) ÷
 * Σ Sγ_air (296 ÷ T)^(n_air) to first order in ln(296 ÷ T). An offset under
 * {@link EXPONENT_OFFSET_FLOOR} is 0.
 */
export function broadeningExponentOffsets(lines: LineList): Readonly<Record<string, number>> {
  const offsetOf = (widths: Float64Array, exponents: Float64Array): number | undefined => {
    let given = 0;
    let givenN = 0;
    let air = 0;
    let airN = 0;
    for (let i = 0; i < widths.length; i += 1) {
      const gamma = widths[i] ?? Number.NaN;
      if (!Number.isFinite(gamma)) {
        continue;
      }
      const s = lines.intensitiesCm[i] ?? 0;
      const nAir = lines.airExponents[i] ?? Number.NaN;
      const own = exponents[i] ?? Number.NaN;
      const gammaAir = lines.airHalfWidthsPerCmAtm[i] ?? Number.NaN;
      given += s * gamma;
      givenN += s * gamma * (Number.isFinite(own) ? own : nAir);
      air += s * gammaAir;
      airN += s * gammaAir * nAir;
    }
    if (!(given > 0 && air > 0)) {
      return undefined;
    }
    const offset = givenN / given - airN / air;
    return Math.abs(offset) < EXPONENT_OFFSET_FLOOR ? 0 : offset;
  };
  const offsets: Record<string, number> = {};
  const self = offsetOf(lines.selfHalfWidthsPerCmAtm, lines.selfExponents);
  if (self !== undefined) {
    offsets["self"] = self;
  }
  for (const gas of FOREIGN_BROADENERS) {
    const d = offsetOf(lines.foreignHalfWidthsPerCmAtm[gas], lines.foreignExponents[gas]);
    if (d !== undefined) {
      offsets[gas] = d;
    }
  }
  return offsets;
}

/** The weights of 4-point Lagrange interpolation at x from four abscissae. */
export function lagrangeWeights(
  xs: readonly [number, number, number, number],
  x: number,
): number[] {
  return xs.map((xi, i) => {
    let w = 1;
    for (const [j, xj] of xs.entries()) {
      if (j !== i) {
        w *= (x - xj) / (xi - xj);
      }
    }
    return w;
  });
}

/** The four nodes of a grid nearest an interval [i, i + 1]: i − 1 to i + 2, held inside the grid. */
export function stencil(i: number, count: number): readonly [number, number, number, number] {
  const first = Math.min(Math.max(i - 1, 0), Math.max(count - 4, 0));
  return [first, first + 1, first + 2, first + 3];
}

/**
 * A g-point's σ at a temperature from its values at the grid's temperatures (the client's chain,
 * `absorbers/kDistribution.ts`): 4-point Lagrange in T on ln σ over the four nodes nearest, and
 * linear in σ between the two about it where any of the four is 0
 * (`decision-r08-t4d-kdist.md`, ruling 2).
 */
export function cubicInTemperature(
  temperaturesK: ReadonlyArray<number>,
  values: ReadonlyArray<number>,
  temperatureK: number,
): number {
  const n = temperaturesK.length;
  let i = 0;
  while (i < n - 2 && (temperaturesK[i + 1] ?? Infinity) < temperatureK) {
    i += 1;
  }
  const lo = temperaturesK[i] ?? Number.NaN;
  const hi = temperaturesK[i + 1] ?? lo;
  if (n < 4) {
    const f = hi > lo ? (temperatureK - lo) / (hi - lo) : 0;
    return (values[i] ?? 0) + ((values[i + 1] ?? values[i] ?? 0) - (values[i] ?? 0)) * f;
  }
  const s = stencil(i, n);
  const v = s.map((k) => values[k] ?? 0);
  if (v.every((x) => x > 0)) {
    const w = lagrangeWeights(
      [
        temperaturesK[s[0]] ?? 0,
        temperaturesK[s[1]] ?? 0,
        temperaturesK[s[2]] ?? 0,
        temperaturesK[s[3]] ?? 0,
      ],
      temperatureK,
    );
    return Math.exp(w.reduce((sum, wk, k) => sum + wk * Math.log(v[k] ?? 1), 0));
  }
  const f = (temperatureK - lo) / (hi - lo);
  return (values[i] ?? 0) + ((values[i + 1] ?? 0) - (values[i] ?? 0)) * f;
}

/** The largest optical depth a curve holds, −ln 10⁻⁶: T_c in [10⁻⁶, 1] (Design note 8). */
export const CURVE_DEPTH_MAX = -Math.log(1e-6);

/**
 * A curve's optical depth at a pressure from its rows (the client's read, the kernels' and the
 * twin's; `decision-r08-t4d-kdist-chain.md`, ruling 3): 4-point Lagrange in ln p on the stored depth
 * τ = −ln T_c over the four rows nearest, unclamped between rows (at high pressure T at fixed u is
 * not monotone in p), then τ clamped to [0, {@link CURVE_DEPTH_MAX}]. Below the first row and above
 * the last the end row is held; with under four rows the read is linear.
 */
export function cubicInLogPressure(
  logPressures: ReadonlyArray<number>,
  depths: ReadonlyArray<number>,
  logPressure: number,
): number {
  const n = logPressures.length;
  const first = logPressures[0] ?? 0;
  const last = logPressures[n - 1] ?? first;
  const x = Math.min(Math.max(logPressure, first), last);
  let i = 0;
  while (i < n - 2 && (logPressures[i + 1] ?? Infinity) < x) {
    i += 1;
  }
  let depth: number;
  if (n < 4) {
    const lo = logPressures[i] ?? 0;
    const hi = logPressures[i + 1] ?? lo;
    const a = depths[i] ?? 0;
    const b = depths[i + 1] ?? a;
    depth = a + (b - a) * (hi > lo ? (x - lo) / (hi - lo) : 0);
  } else {
    const s = stencil(i, n);
    const w = lagrangeWeights(
      [
        logPressures[s[0]] ?? 0,
        logPressures[s[1]] ?? 0,
        logPressures[s[2]] ?? 0,
        logPressures[s[3]] ?? 0,
      ],
      x,
    );
    depth = w.reduce((sum, wk, k) => sum + wk * (depths[s[k] ?? 0] ?? 0), 0);
  }
  return Math.min(Math.max(depth, 0), CURVE_DEPTH_MAX);
}

/** A cell's mean transmittance through u from g-points: Σ w e^(−σu). */
function exponentialSum(weights: Float64Array, sigmas: ArrayLike<number>, u: number): number {
  let t = 0;
  for (let g = 0; g < weights.length; g += 1) {
    t += (weights[g] ?? 0) * Math.exp(-(sigmas[g] ?? 0) * u);
  }
  return t;
}

/** Where a chain test found its largest error: the error, the state and the column. */
export interface ChainError {
  readonly error: number;
  readonly temperatureK: number;
  readonly pressurePa: number;
  readonly columnCm2: number;
}

/** The chain test's worst errors within the column cap and beyond it. */
export interface ChainErrors {
  readonly within: ChainError;
  readonly beyond: ChainError;
  /** The least column within the cap at which the chain errs past {@link CHAIN_BOUND}, cm⁻²; ∞ for none. */
  readonly firstFailingColumnCm2: number;
}

/**
 * The client's chain at a state, as a cell's curve of growth: σ at fixed g cubic in T from the
 * cell's node values ({@link cubicInTemperature}), each of the four rows about the pressure summed
 * into a curve, and the curve's depth read cubic in ln p ({@link cubicInLogPressure}).
 *
 * @param values - One variant's σ, `values[t × P + p][g]`, cm² a molecule.
 * @returns T at a column, cm⁻².
 */
export function chainCurve(
  grid: ReductionGrid,
  weights: Float64Array,
  values: ReadonlyArray<Float64Array>,
  temperatureK: number,
  pressurePa: number,
): (columnCm2: number) => number {
  const nT = grid.temperaturesK.length;
  const nP = grid.pressuresPa.length;
  const logP = grid.pressuresPa.map(Math.log);
  let j = 0;
  while (j < nP - 2 && (grid.pressuresPa[j + 1] ?? Infinity) < pressurePa) {
    j += 1;
  }
  const rows = nP < 4 ? [j, Math.min(j + 1, nP - 1)] : [...stencil(j, nP)];
  const g = weights.length;
  const rowSigmas = rows.map((p) =>
    Float64Array.from({ length: g }, (_, k) =>
      cubicInTemperature(
        grid.temperaturesK,
        Array.from({ length: nT }, (_unused, t) => values[t * nP + p]?.[k] ?? 0),
        temperatureK,
      ),
    ),
  );
  const rowLogP = rows.map((p) => logP[p] ?? 0);
  return (u) => {
    const depths = rowSigmas.map(
      (sigmas) => -Math.log(Math.max(exponentialSum(weights, sigmas, u), 1e-300)),
    );
    return Math.exp(-cubicInLogPressure(rowLogP, depths, Math.log(pressurePa)));
  };
}

/**
 * The client's chain ({@link chainCurve}) against line by line at a state between nodes, over the
 * line-by-line curve's columns at {@link CHAIN_COLUMNS_PER_DECADE} a decade, split at the column
 * cap.
 *
 * @param values - One variant's σ, `values[t × P + p][g]`, cm² a molecule.
 * @param capCm2 - The column the gated test stops at, cm⁻² ({@link columnCapCm2}).
 */
export function chainError(
  grid: ReductionGrid,
  weights: Float64Array,
  values: ReadonlyArray<Float64Array>,
  exact: SortedNode,
  temperatureK: number,
  pressurePa: number,
  capCm2 = Infinity,
): ChainErrors {
  const curve = chainCurve(grid, weights, values, temperatureK, pressurePa);
  const none = { error: 0, temperatureK, pressurePa, columnCm2: 0 };
  let within: ChainError = none;
  let beyond: ChainError = none;
  let firstFailingColumnCm2 = Infinity;
  for (const u of errorColumns(exact, REDUCTION_TARGET, CHAIN_COLUMNS_PER_DECADE, Infinity)) {
    const truth = intervalTransmittance(exact, 0, 1, u);
    const error = Math.abs(curve(u) - truth);
    if (u <= capCm2 && error > CHAIN_BOUND) {
      firstFailingColumnCm2 = Math.min(firstFailingColumnCm2, u);
    }
    if (u <= capCm2 && error > within.error) {
      within = { error, temperatureK, pressurePa, columnCm2: u };
    } else if (u > capCm2 && error > beyond.error) {
      beyond = { error, temperatureK, pressurePa, columnCm2: u };
    }
  }
  return { within, beyond, firstFailingColumnCm2 };
}

/**
 * A condensing species' saturation pressure, none at or above its critical temperature: by
 * Clausius–Clapeyron from its triple point, with the enthalpy of sublimation below it and of
 * vaporisation above (a derivation from the triple point, `decision-r08-t4d-kdist-chain.md` §4.1);
 * or by a reference equation of state's ancillary equations, ln(p ÷ p_t) = (T_t ÷ T) Σ aᵢ (1 −
 * T ÷ T_t)^tᵢ below the triple point and ln(p ÷ p_c) = (T_c ÷ T) Σ bᵢ (1 − T ÷ T_c)^sᵢ above it.
 */
export type SaturationCurve =
  | {
      readonly kind: "clausiusClapeyron";
      readonly tripleK: number;
      readonly triplePa: number;
      readonly sublimationJPerMol: number;
      readonly vaporisationJPerMol: number;
      readonly criticalK: number;
      /** The figures' sources. */
      readonly source: string;
    }
  | {
      readonly kind: "ancillary";
      readonly tripleK: number;
      readonly triplePa: number;
      readonly criticalK: number;
      readonly criticalPa: number;
      /** The sublimation equation's terms, [aᵢ, tᵢ]. */
      readonly sublimation: ReadonlyArray<readonly [number, number]>;
      /** The vapour-pressure equation's terms, [bᵢ, sᵢ]. */
      readonly vapour: ReadonlyArray<readonly [number, number]>;
      readonly source: string;
    }
  /**
   * A sublimation-pressure fit ln(p ÷ 1 bar) = Σ Aᵢ ÷ Tⁱ below the triple point (N. Fray and B.
   * Schmitt, Planet. Space Sci. 57 (2009) 2053, Eq. 4), and above it Clausius–Clapeyron from the
   * fit's own pressure at the triple point with a constant enthalpy of vaporisation (R08.T4.g).
   */
  | {
      readonly kind: "sublimationFit";
      readonly tripleK: number;
      /** A₀, A₁, … of the fit, p in bar. */
      readonly coefficients: ReadonlyArray<number>;
      readonly vaporisationJPerMol: number;
      readonly criticalK: number;
      readonly source: string;
    };

/** Σ Aᵢ ÷ Tⁱ, a sublimation fit's ln(p ÷ 1 bar). */
function sublimationFitLog(coefficients: ReadonlyArray<number>, temperatureK: number): number {
  return coefficients.reduce((sum, a, i) => sum + a / temperatureK ** i, 0);
}

/** Σ aᵢ xᵗⁱ over an ancillary equation's terms. */
function ancillarySeries(terms: ReadonlyArray<readonly [number, number]>, x: number): number {
  return terms.reduce((sum, [a, t]) => sum + a * x ** t, 0);
}

/** A species' saturation pressure at a temperature, Pa, or ∞ at or above its critical temperature. */
export function saturationPressurePa(curve: SaturationCurve, temperatureK: number): number {
  if (temperatureK >= curve.criticalK) {
    return Infinity;
  }
  let pressurePa: number;
  switch (curve.kind) {
    case "clausiusClapeyron": {
      const enthalpy =
        temperatureK < curve.tripleK ? curve.sublimationJPerMol : curve.vaporisationJPerMol;
      pressurePa =
        curve.triplePa *
        Math.exp(
          (-enthalpy / MOLAR_GAS_CONSTANT_J_PER_MOL_K) * (1 / temperatureK - 1 / curve.tripleK),
        );
      break;
    }
    case "ancillary":
      pressurePa =
        temperatureK < curve.tripleK
          ? curve.triplePa *
            Math.exp(
              (curve.tripleK / temperatureK) *
                ancillarySeries(curve.sublimation, 1 - temperatureK / curve.tripleK),
            )
          : curve.criticalPa *
            Math.exp(
              (curve.criticalK / temperatureK) *
                ancillarySeries(curve.vapour, 1 - temperatureK / curve.criticalK),
            );
      break;
    case "sublimationFit": {
      const below = Math.min(temperatureK, curve.tripleK);
      const solidPa = 1e5 * Math.exp(sublimationFitLog(curve.coefficients, below));
      pressurePa =
        temperatureK <= curve.tripleK
          ? solidPa
          : solidPa *
            Math.exp(
              (-curve.vaporisationJPerMol / MOLAR_GAS_CONSTANT_J_PER_MOL_K) *
                (1 / temperatureK - 1 / curve.tripleK),
            );
      break;
    }
  }
  return pressurePa;
}

/** Boltzmann's constant, J K⁻¹ (exact since the SI's 2019 redefinition). */
const BOLTZMANN_J_PER_K = 1.380_649e-23;

/**
 * Helium's Rayleigh cross-section, cm², at a vacuum wavelength, nm, as the client's `rayleigh.ts`
 * takes it: Mansfield and Peck 1969's dispersion (J. Opt. Soc. Am. 59, 199), (n − 1) × 10⁵ =
 * 1,470.091 ÷ (423.98 − σ²), σ in µm⁻¹, at 0 °C and 1 atm with compressibility 1.000 532, and a
 * King factor of 1, an atom's: σ_R = 24π³ ÷ (λ⁴ N²) ((n² − 1) ÷ (n² + 2))². The fit is stated
 * over 0.48–2.06 µm, so below 480 nm it is extrapolated (5.06 × 10²⁸ cm⁻² of cap at 380 nm),
 * checked there against R. L. Kurucz's helium cross-section after A. Dalgarno 1962 (Smithsonian
 * Astrophys. Obs. Spec. Rep. 309, 1970), 2.721 against 2.730 × 10⁻²⁸ cm² at 380 nm, 0.33%
 * (`decision-r08-t4d-kdist-chain.md` §12, ruling 7).
 */
export function heliumRayleighCm2(wavelengthNm: number): number {
  const perUm = 1_000 / wavelengthNm;
  const delta = (1_470.091 / (423.98 - perUm * perUm)) * 1e-5;
  const nSquaredMinusOne = delta * (2 + delta);
  const lorentzLorenz = nSquaredMinusOne / (nSquaredMinusOne + 3);
  const density = 101_325 / (1.000_532 * BOLTZMANN_J_PER_K * 273.15);
  const lambdaM = wavelengthNm * 1e-9;
  return (
    ((24 * Math.PI ** 3) / (lambdaM ** 4 * density * density)) * lorentzLorenz * lorentzLorenz * 1e4
  );
}

/**
 * The third column cap, cm⁻² (`decision-r08-t4d-kdist-chain.md` §9): −ln 10⁻⁶ ÷ σ_R,He at a cell's
 * centre. Helium scatters least of any bulk gas, and a species at mole fraction x lies on a bulk
 * column u ÷ x ≥ u, so any straight path holding more is under the curves' 10⁻⁶ floor by Rayleigh
 * scattering alone.
 */
export function rayleighCapCm2(cellNm: number): number {
  return CURVE_DEPTH_MAX / heliumRayleighCm2(cellNm);
}

/** A species' Rayleigh cross-section at a vacuum wavelength, nm, in cm² a molecule. */
export type RayleighCrossSection = (wavelengthNm: number) => number;

/**
 * A Rayleigh cross-section from a refractivity n − 1 at a reference number density and a King
 * factor, cm²: 24π³ ÷ (λ⁴ N²) ((n² − 1) ÷ (n² + 2))² F_K, as the client's
 * `crossSectionFromDispersionM2` takes it (n² − 1 as δ(2 + δ), so that no difference of two
 * numbers near 1 is taken).
 */
function dispersionRayleighCm2(
  nMinusOne: number,
  densityPerM3: number,
  kingFactor: number,
  wavelengthNm: number,
): number {
  const nSquaredMinusOne = nMinusOne * (2 + nMinusOne);
  const lorentzLorenz = nSquaredMinusOne / (nSquaredMinusOne + 3);
  const lambdaSquaredM2 = (wavelengthNm * 1e-9) ** 2;
  return (
    (((24 * Math.PI ** 3) / (lambdaSquaredM2 * lambdaSquaredM2 * densityPerM3 * densityPerM3)) *
      lorentzLorenz *
      lorentzLorenz *
      kingFactor) /
    M2_PER_CM2
  );
}

/** A King factor from a linear depolarisation ratio ρₚ: (3 + 6ρₚ) ÷ (3 − 4ρₚ). */
function kingFactorOfLinearDepolarisation(rhoP: number): number {
  return (3 + 6 * rhoP) / (3 - 4 * rhoP);
}

/**
 * N₂O's Rayleigh cross-section, cm², at a vacuum wavelength, nm, as the client's `rayleigh.ts`
 * takes it (its `rayleighCrossSectionM2`, `kDistributions.test.ts` holding the two equal): He,
 * Fang, Shoshanim, Brown and Rudich 2021's dispersion (Atmos. Chem. Phys. 21, 14927, eq. 9),
 * (n − 1) × 10⁸ = 22,095 + 1.662 91 × 10¹⁴ ÷ (6.752 26 × 10⁹ − ν²), ν in cm⁻¹, at 15 °C and 1 atm
 * with Z = 1, and Sneep and Ubachs 2005's King factor (J. Quant. Spectrosc. Radiat. Transfer 92,
 * 293, eq. 19, after Alms et al. 1975), ρₚ = 0.0577 + 11.8 × 10⁻¹² ν².
 */
export function nitrousOxideRayleighCm2(wavelengthNm: number): number {
  const perCm = 1e7 / wavelengthNm;
  const nuSquared = perCm * perCm;
  const nMinusOne = (22_095 + 1.662_91e14 / (6.752_26e9 - nuSquared)) * 1e-8;
  const density = 101_325 / (BOLTZMANN_J_PER_K * 288.15);
  const king = kingFactorOfLinearDepolarisation(0.057_7 + 11.8e-12 * nuSquared);
  return dispersionRayleighCm2(nMinusOne, density, king, wavelengthNm);
}

/**
 * NH₃'s Rayleigh cross-section, cm², at a vacuum wavelength, nm, as the client's `rayleigh.ts`
 * takes it (held equal by `kDistributions.test.ts`): C. and M. Cuthbertson 1914's dispersion
 * (Phil. Trans. R. Soc. Lond. A 213, 1), n − 1 = 0.032 953 ÷ (90.392 − σ²), σ in µm⁻¹, at 0 °C and
 * 1 atm with the client's Z = 0.984 798 × 0.7708 ÷ 0.7605, and a King factor from ρₚ = 0.000 91 at
 * 632.8 nm, vibrational Raman excluded (R. I. Keir, PhD thesis, University of New England 1995).
 */
export function ammoniaRayleighCm2(wavelengthNm: number): number {
  const perUm = 1_000 / wavelengthNm;
  const nMinusOne = 0.032_953 / (90.392 - perUm * perUm);
  const density = 101_325 / (((0.984_798 * 0.770_8) / 0.760_5) * BOLTZMANN_J_PER_K * 273.15);
  return dispersionRayleighCm2(
    nMinusOne,
    density,
    kingFactorOfLinearDepolarisation(0.000_91),
    wavelengthNm,
  );
}

/**
 * The species' own Rayleigh column at a cell, cm⁻² (R08.T4.g's ruling on the between-node test):
 * −ln 10⁻⁶ ÷ σ_R,s at the cell's centre for a species whose σ_R is measured, helium's
 * ({@link rayleighCapCm2}) otherwise. A path holding a column u of the species scatters at least
 * u σ_R,s from the species alone, whatever else it holds, so any straight path holding more is under
 * the curves' 10⁻⁶ floor by Rayleigh scattering alone; helium's cap is the same bound with the
 * species' own scattering left out.
 *
 * @param rayleighCm2 - The species' measured cross-section, or `undefined` for helium's.
 */
export function speciesRayleighCapCm2(
  rayleighCm2: RayleighCrossSection | undefined,
  cellNm: number,
): number {
  return rayleighCm2 === undefined ? rayleighCapCm2(cellNm) : CURVE_DEPTH_MAX / rayleighCm2(cellNm);
}

/**
 * The largest depth a linear cell's peak takes at its gated column: 0.02, where the cell mean is
 * exact to (0.02)² ÷ 8 = 5 × 10⁻⁵ in T, ½ Var(σ) u² bounded by Popoviciu's inequality (½ (0.02)²
 * = 2 × 10⁻⁴ the cruder bound; `decision-r08-t4d-kdist-chain.md` §9).
 */
export const LINEAR_CELL_DEPTH = 0.02;

/** The supersaturation the cold-end cap allows: 10, as observed in planetary atmospheres (§4.2). */
export const SUPERSATURATION = 10;

/**
 * The factor of the pressure cap, 10 (`decision-r08-t4d-kdist-chain.md` §8): 2.5 for the
 * broadening axis (p = p_eff ÷ Σ x_j r_j, HITRAN2024's least visible ratio He on HCN's 0.40) and a
 * fourfold spare over the isothermal tangent estimate.
 */
export const PRESSURE_CAP_FACTOR = 10;

/** The species' column 40 p ÷ (m g) at {@link BELOW_GRID_GRAVITY_M_S2}, cm⁻², for a pressure, Pa. */
function grazingColumnCm2(pressurePa: number, molecularMassDa: number): number {
  return (
    ((40 * pressurePa) / (molecularMassDa * ATOMIC_MASS_KG * BELOW_GRID_GRAVITY_M_S2)) * M2_PER_CM2
  );
}

/**
 * The chain test's column cap at a temperature and broadening pressure, cm⁻²
 * (`decision-r08-t4d-kdist-chain.md`, ruling 4 and §8): 400 p_eff ÷ (m g) at
 * {@link BELOW_GRID_GRAVITY_M_S2}, the most a tangent path on any generated body holds
 * (√2 × sqrt(2πR ÷ H) ÷ g, at worst 57 s² m⁻¹ against the 40's 80) times
 * {@link PRESSURE_CAP_FACTOR}; or, where lower, for a condensing species, {@link SUPERSATURATION}
 * times its saturated column 40 p_sat ÷ (m g).
 */
export function columnCapCm2(
  curve: SaturationCurve | undefined,
  temperatureK: number,
  pressurePa: number,
  molecularMassDa: number,
): number {
  const byPressure = PRESSURE_CAP_FACTOR * grazingColumnCm2(pressurePa, molecularMassDa);
  if (curve === undefined) {
    return byPressure;
  }
  return Math.min(
    byPressure,
    SUPERSATURATION * grazingColumnCm2(saturationPressurePa(curve, temperatureK), molecularMassDa),
  );
}

/**
 * A cell's gated column at a temperature and broadening pressure, cm⁻²: the least of the pressure
 * cap and the condensing cap ({@link columnCapCm2}) and the Rayleigh cap at the cell
 * ({@link rayleighCapCm2}), the columns every node and chain test of the cell stops at.
 */
export function gatedColumnCm2(
  curve: SaturationCurve | undefined,
  cellNm: number,
  temperatureK: number,
  pressurePa: number,
  molecularMassDa: number,
): number {
  return Math.min(
    columnCapCm2(curve, temperatureK, pressurePa, molecularMassDa),
    rayleighCapCm2(cellNm),
  );
}

/**
 * A cell's between-node test column at a temperature and broadening pressure, cm⁻²: its gated
 * column ({@link gatedColumnCm2}) or, where lower, the species' own Rayleigh column
 * ({@link speciesRayleighCapCm2}; R08.T4.g's ruling). The node tier, the bisection, G = 1 and the
 * 16-bit test keep the gated column, since the stored nodes also feed R08.T14's layer solves, whose
 * diffuse paths can pass a straight path's Rayleigh column.
 */
export function chainColumnCm2(
  curve: SaturationCurve | undefined,
  cellNm: number,
  temperatureK: number,
  pressurePa: number,
  molecularMassDa: number,
  rayleighCm2: RayleighCrossSection | undefined,
): number {
  return Math.min(
    gatedColumnCm2(curve, cellNm, temperatureK, pressurePa, molecularMassDa),
    speciesRayleighCapCm2(rayleighCm2, cellNm),
  );
}

/** One cell of a species' reduction. */
export interface ReducedCell {
  readonly cellNm: number;
  /** The g-points' weights, summing to 1, shared by every variant and node. */
  readonly weights: Float64Array;
  /** `values[((v × T + t) × P + p) × G + g]`, m² a molecule, v the policy's variant. */
  readonly values: Float64Array;
  /** The error bound the reduction left at its nodes, before storage. */
  readonly worstNodeError: number;
  /** The chain test's worst midpoint within its cap ({@link chainColumnCm2}), over every variant. */
  readonly worstChain: ChainError;
  /** The chain test's worst midpoint beyond its cap, recorded. */
  readonly worstBeyondCap: ChainError;
  /** Every midpoint whose error within the chain test's cap passes {@link CHAIN_BOUND}. */
  readonly failures: ReadonlyArray<ChainError>;
  /**
   * At each temperature node, the highest pressure node whose line shape is measured (no variant's
   * γ_L past the policy's), or −1.
   */
  readonly measuredShapeTo: Int32Array;
  /** Each node's {@link CellNode.beyondCutoffShare} and {@link CellNode.nearWingShare}, `[t × P + p]`. */
  readonly beyondCutoffShare: Float64Array;
  readonly nearWingShare: Float64Array;
  /** For a two-variant table, its fixed-g mix's record ({@link CellMixing}). */
  readonly mixing?: CellMixing;
}

/**
 * A two-variant cell's fixed-g mix against line by line (`decision-r08-t4d-kdist-chain.md` §§7(a)
 * and 16): at each self share w of {@link MIX_SHARES}, the client's chain on w σ_self(g) +
 * (1 − w) σ_air(g) against line by line at that width share of the variants' χ, at
 * {@link MIXING_PRESSURES_PA} and every temperature node and midpoint, within the column cap.
 * Recorded, not gated: R08.T10.a labels a body by its own column.
 */
export interface CellMixing {
  /** At each share, the mix's worst error within the cap. */
  readonly worst: ReadonlyArray<ChainError>;
  /**
   * At each share, u_mix(w): the least column at which the mix errs past {@link CHAIN_BOUND} at
   * any of the states, cm⁻²; ∞ where it never does within the cap.
   */
  readonly limitColumnsCm2: ReadonlyArray<number>;
  /** The temperatures the mix is tested at, K: the grid's nodes and the midpoints between them. */
  readonly temperaturesK: ReadonlyArray<number>;
  /** `[share][t]`: u_mix at each tested temperature alone, over the pressures, cm⁻²; ∞ for none. */
  readonly limitColumnsByTemperatureCm2: ReadonlyArray<ReadonlyArray<number>>;
}

/** The self shares the fixed-g mix is recorded at: 0.05, 0.25, 0.5, 0.75 and 0.95 (§16). */
export const MIX_SHARES = [0.05, 0.25, 0.5, 0.75, 0.95] as const;

/** cm² in m². */
const M2_PER_CM2 = 1e-4;

/** The pressures the mixing test takes, Pa: 0.1, 1 and 9 MPa (§§7(a) and 16). */
export const MIXING_PRESSURES_PA = [1e5, 1e6, 9e6] as const;

/**
 * A cell's reduction on a grid ({@link reduceCell}), every variant's nodes sharing its g-points, and
 * its chain test at every midpoint between rows, between temperature nodes, and at both at once, for
 * every variant, gated within the chain test's cap at each state; or `undefined` where the cell has
 * no line within the cut-off.
 *
 * @param spectra - The cell's spectrum at each node and test state: line by line
 *   ({@link lineByLine}) or from super-lines (R08.T4.g), under `policy`'s shape.
 * @param capCm2 - The column cap at a temperature and broadening pressure ({@link columnCapCm2}):
 *   the nodes', the bisection's, G = 1's and the mixing record's.
 * @param chainCapCm2 - The between-node test's, at most `capCm2` ({@link chainColumnCm2}).
 */
export function reduceGridCell(
  cellNm: number,
  spectra: NodeSpectra,
  grid: ReductionGrid,
  policy: ShapePolicy,
  capCm2: (temperatureK: number, pressurePa: number) => number,
  chainCapCm2: (temperatureK: number, pressurePa: number) => number,
): ReducedCell | undefined {
  const nT = grid.temperaturesK.length;
  const nP = grid.pressuresPa.length;
  const nV = policy.variants.length;
  const at: CellNode[] = [];
  for (let v = 0; v < nV; v += 1) {
    for (const t of grid.temperaturesK) {
      for (const p of grid.pressuresPa) {
        const n = spectra(cellNm, t, p, only(policy, v));
        if (n === undefined) {
          return undefined;
        }
        at.push(n);
      }
    }
  }
  const nodesPerVariant = nT * nP;
  const caps = at.map((_unused, n) => {
    const i = n % nodesPerVariant;
    return capCm2(grid.temperaturesK[Math.floor(i / nP)] ?? 0, grid.pressuresPa[i % nP] ?? 0);
  });
  // A linear cell (§9): its largest k times its gated column at most 0.02 at every node, stored as
  // its mean alone, G = 1, its error measured on the same columns.
  const linear = at.every(
    (n, i) => (n.node.sorted.at(-1) ?? 0) * (caps[i] ?? Infinity) <= LINEAR_CELL_DEPTH,
  );
  const r = linear
    ? linearCell(
        at.map((n) => n.node),
        caps,
      )
    : reduceCell(
        at.map((n) => n.node),
        REDUCTION_TARGET,
        caps,
      );
  // A node's shape is measured where no variant's γ_L passes the policy's. A held χ marks nothing
  // here: R08.T10.a bounds its effect on the body's own column (decision-r08-t4d-kdist-chain.md
  // §14), from the sidecar's flags ({@link chiHeldFlags}).
  const measuredAt = (t: number, p: number): boolean => {
    for (let v = 0; v < nV; v += 1) {
      if (at[v * nodesPerVariant + t * nP + p]?.estimatedShape !== false) {
        return false;
      }
    }
    return true;
  };
  const measuredShapeTo = Int32Array.from({ length: nT }, (_, t) => {
    let last = -1;
    for (let p = 0; p < nP; p += 1) {
      if (measuredAt(t, p)) {
        last = p;
      } else {
        break;
      }
    }
    return last;
  });
  const share = (of: (n: CellNode) => number): Float64Array =>
    Float64Array.from({ length: nodesPerVariant }, (_, n) =>
      Math.max(
        ...Array.from({ length: nV }, (_unused, v) => {
          const node = at[v * nodesPerVariant + n];
          return node === undefined ? 0 : of(node);
        }),
      ),
    );
  const beyondCutoffShare = share((n) => n.beyondCutoffShare);
  const nearWingShare = share((n) => n.nearWingShare);
  const midT = (t: number): number =>
    0.5 * ((grid.temperaturesK[t] ?? 0) + (grid.temperaturesK[t + 1] ?? 0));
  const midP = (p: number): number =>
    Math.sqrt((grid.pressuresPa[p] ?? 0) * (grid.pressuresPa[p + 1] ?? 0));
  // Every midpoint the read takes: none above READ_TOP_PA, where p̄ is held.
  const read = (p: number): boolean => p <= READ_TOP_PA * (1 + 1e-12);
  const states: (readonly [number, number])[] = [];
  for (let t = 0; t < nT; t += 1) {
    for (let p = 0; p < nP; p += 1) {
      const node = grid.pressuresPa[p] ?? 0;
      if (t + 1 < nT && read(node)) {
        states.push([midT(t), node]);
      }
      if (p + 1 < nP && read(midP(p))) {
        states.push([grid.temperaturesK[t] ?? 0, midP(p)]);
      }
      if (t + 1 < nT && p + 1 < nP && read(midP(p))) {
        states.push([midT(t), midP(p)]);
      }
    }
  }
  const none: ChainError = { error: 0, temperatureK: 0, pressurePa: 0, columnCm2: 0 };
  let worstChain = none;
  let worstBeyondCap = none;
  const failures: ChainError[] = [];
  for (let v = 0; v < nV; v += 1) {
    const values = r.values.slice(v * nodesPerVariant, (v + 1) * nodesPerVariant);
    for (const [t, p] of states) {
      const exact = spectra(cellNm, t, p, only(policy, v));
      if (exact === undefined) {
        continue;
      }
      const e = chainError(grid, r.weights, values, exact.node, t, p, chainCapCm2(t, p));
      if (e.within.error > worstChain.error) {
        worstChain = e.within;
      }
      if (e.beyond.error > worstBeyondCap.error) {
        worstBeyondCap = e.beyond;
      }

      if (e.within.error > CHAIN_BOUND) {
        failures.push(e.within);
      }
    }
  }
  let mixing: CellMixing | undefined;
  if (nV === 2) {
    const temperatures = grid.temperaturesK.flatMap((t, i) => (i + 1 < nT ? [t, midT(i)] : [t]));
    const worst: ChainError[] = [];
    const limitColumnsCm2: number[] = [];
    const limitColumnsByTemperatureCm2: number[][] = [];
    for (const w of MIX_SHARES) {
      const mixed = r.values
        .slice(0, nodesPerVariant)
        .map((a, n) =>
          a.map((x, k) => w * x + (1 - w) * (r.values[nodesPerVariant + n]?.[k] ?? 0)),
        );
      let worstAt: ChainError = none;
      let limit = Infinity;
      const byTemperature: number[] = [];
      for (const t of temperatures) {
        let atT = Infinity;
        for (const p of MIXING_PRESSURES_PA) {
          const exact = spectra(cellNm, t, p, [w, 1 - w]);
          if (exact === undefined) {
            continue;
          }
          const e = chainError(grid, r.weights, mixed, exact.node, t, p, capCm2(t, p));
          if (e.within.error > worstAt.error) {
            worstAt = e.within;
          }
          atT = Math.min(atT, e.firstFailingColumnCm2);
        }
        byTemperature.push(atT);
        limit = Math.min(limit, atT);
      }
      worst.push(worstAt);
      limitColumnsCm2.push(limit);
      limitColumnsByTemperatureCm2.push(byTemperature);
    }
    mixing = { worst, limitColumnsCm2, temperaturesK: temperatures, limitColumnsByTemperatureCm2 };
  }
  const g = r.weights.length;
  const values = new Float64Array(at.length * g);
  for (const [n, v] of r.values.entries()) {
    for (let k = 0; k < g; k += 1) {
      values[n * g + k] = (v[k] ?? Number.NaN) * M2_PER_CM2;
    }
  }
  return {
    cellNm,
    weights: r.weights,
    values,
    worstNodeError: r.worstError,
    worstChain,
    worstBeyondCap,
    failures,
    measuredShapeTo,
    beyondCutoffShare,
    nearWingShare,
    ...(mixing === undefined ? {} : { mixing }),
  };
}

/**
 * The gravity of the 1 Pa check's column, m s⁻²: below Pluto's 0.62 (`decision-r08-t4d-kdist.md`
 * §3.2).
 */
export const BELOW_GRID_GRAVITY_M_S2 = 0.5;

/** The 1 Pa check's pressure, Pa. */
export const BELOW_GRID_CHECK_PA = 1;

/** The atomic mass constant, kg (CODATA 2018). */
const ATOMIC_MASS_KG = 1.660_539_066_6e-27;

/**
 * The held 10 Pa table against line by line at 1 Pa (`decision-r08-t4d-kdist.md` §3.2): at each
 * temperature node and for each variant, the largest |T| difference over columns to 40 p ÷ (m g)
 * at {@link BELOW_GRID_GRAVITY_M_S2}, the most a grazing path above p can hold for a pure gas of
 * molecular mass m (Da).
 *
 * @param spectra - The cell's spectrum at each state, as the reduction took it ({@link NodeSpectra}).
 */
export function belowGridError(
  cellNm: number,
  spectra: NodeSpectra,
  grid: ReductionGrid,
  policy: ShapePolicy,
  cell: ReducedCell,
  molecularMassDa: number,
): number {
  const nT = grid.temperaturesK.length;
  const nP = grid.pressuresPa.length;
  const g = cell.weights.length;
  const capCm2 = grazingColumnCm2(BELOW_GRID_CHECK_PA, molecularMassDa);
  let worst = 0;
  for (let v = 0; v < policy.variants.length; v += 1) {
    for (const [t, temperatureK] of grid.temperaturesK.entries()) {
      const exact = spectra(cellNm, temperatureK, BELOW_GRID_CHECK_PA, only(policy, v));
      if (exact === undefined) {
        continue;
      }
      const first = (v * nT + t) * nP * g;
      const held = cell.values.subarray(first, first + g).map((x) => x / M2_PER_CM2);
      for (const u of errorColumns(
        exact.node,
        REDUCTION_TARGET,
        CHAIN_COLUMNS_PER_DECADE,
        capCm2,
      )) {
        worst = Math.max(
          worst,
          Math.abs(
            exponentialSum(cell.weights, held, u) - intervalTransmittance(exact.node, 0, 1, u),
          ),
        );
      }
    }
  }
  return worst;
}

/**
 * The largest change storing a cell in 16 bits makes to its curve at any node, against its `f64`
 * values, over columns from where its largest σ takes τ = 10⁻³ to where it takes 10⁶: within each
 * node's column cap, the gate, and beyond it, recorded (`decision-r08-t4d-kdist-chain.md` §8, the
 * caps; §9, a linear cell's columns stopping at its cap).
 *
 * @param capCm2 - Node n's gated column, cm⁻², n = (v × T + t) × P + p.
 */
export function quantisationError(
  cell: ReducedCell,
  encoded: EncodedCell,
  nodes: number,
  capCm2: (node: number) => number,
): { readonly within: number; readonly beyond: number } {
  const decoded = decodeCell(encoded);
  const g = cell.weights.length;
  let within = 0;
  let beyond = 0;
  for (let n = 0; n < nodes; n += 1) {
    const capM2 = capCm2(n) / M2_PER_CM2;
    const exact = cell.values.subarray(n * g, (n + 1) * g);
    const stored = decoded.subarray(n * g, (n + 1) * g);
    let largest = 0;
    for (const v of exact) {
      largest = Math.max(largest, v);
    }
    if (!(largest > 0)) {
      continue;
    }
    for (let k = 0; k <= 9 * 6; k += 1) {
      const u = (1e-3 / largest) * 10 ** (k / 6);
      const change = Math.abs(
        exponentialSum(cell.weights, exact, u) - exponentialSum(cell.weights, stored, u),
      );
      if (u <= capM2) {
        within = Math.max(within, change);
      } else {
        beyond = Math.max(beyond, change);
      }
    }
  }
  return { within, beyond };
}

/** The codes of a 16-bit ln σ: 0 for σ = 0, and 1 to 65,535 for ln σ = offset + step (code − 1). */
const CODE_MAX = 65_535;

/** A cell's values stored as 16-bit ln σ, with its own offset and step. */
export interface EncodedCell {
  readonly cellNm: number;
  readonly weights: Float64Array;
  /** ln(σ ÷ 1 m²) of code 1. */
  readonly lnOffset: number;
  /** The step in ln σ between codes. */
  readonly lnStep: number;
  /** The codes, `[((v × T + t) × P + p) × G + g]`. */
  readonly codes: Uint16Array;
}

/**
 * A cell's values as 16-bit ln σ over its own range of ln σ, code 0 for σ = 0
 * (`decision-r08-t4d-kdist.md` §4). The cell's own offset and step, finer than one per file, keep
 * the step under 9.2 × 10⁻⁴ in ln σ for any cell spanning 60 e-folds.
 */
export function encodeCell(cell: ReducedCell): EncodedCell {
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of cell.values) {
    if (v > 0) {
      lo = Math.min(lo, Math.log(v));
      hi = Math.max(hi, Math.log(v));
    }
  }
  const lnOffset = Number.isFinite(lo) ? lo : 0;
  const lnStep = hi > lo ? (hi - lo) / (CODE_MAX - 1) : 1;
  const codes = Uint16Array.from(cell.values, (v) =>
    v > 0 ? 1 + Math.round((Math.log(v) - lnOffset) / lnStep) : 0,
  );
  return { cellNm: cell.cellNm, weights: cell.weights, lnOffset, lnStep, codes };
}

/** An encoded cell's values, m², as the client decodes them. */
export function decodeCell(cell: EncodedCell): Float64Array {
  return Float64Array.from(cell.codes, (code) =>
    code === 0 ? 0 : Math.exp(cell.lnOffset + cell.lnStep * (code - 1)),
  );
}

/** What a k-distribution table names besides its cells, in its sidecar. */
export interface KDistributionMeta {
  readonly species: string;
  readonly source: string;
  readonly url: string;
  readonly sha256: string;
  readonly fetched: string;
  readonly terms: string;
  readonly reduction: string;
  readonly basis: "measured" | "estimated";
  readonly measuredRangeK: readonly [number, number];
  /** Whether the held 10 Pa table stays `measured` below it, by its 1 Pa check. */
  readonly measuredBelowGrid: boolean;
  /** At each temperature node, the highest pressure node whose line shape is measured, or −1. */
  readonly measuredShapeTo: ReadonlyArray<number>;
  /**
   * The temperature intervals still failing the chain test at 12.5 K steps, over which the species
   * is `estimated` (`decision-r08-t4d-kdist-chain.md`, ruling 4).
   */
  readonly estimatedTemperaturesK: ReadonlyArray<readonly [number, number]>;
  readonly broadening: Readonly<Record<string, number>>;
  /** Each broadening gas's {@link broadeningExponentOffsets}, n̄_j − n̄_air. */
  readonly broadeningExponents: Readonly<Record<string, number>>;
  /** The variants' broadeners, in the cells' value order: `["air"]`, or CO₂'s `["self", "air"]`. */
  readonly variants: ReadonlyArray<"self" | "air">;
  /** {@link chiHeldFlags}, `[t][v]`. */
  readonly chiHeld: ReadonlyArray<ReadonlyArray<boolean>>;
  /**
   * Each node's largest {@link CellNode.beyondCutoffShare} and {@link CellNode.nearWingShare} over
   * the cells, `[t × P + p]`.
   */
  readonly beyondCutoffShare: ReadonlyArray<number>;
  readonly nearWingShare: ReadonlyArray<number>;
  /** The reduction's worst node error, and its worst chain midpoint within the cap and beyond it. */
  readonly worstNodeError: number;
  readonly worstChainError: number;
  readonly worstBeyondCapError: number;
  /**
   * A two-variant table's fixed-g mix (§§16–17): its shares and tested temperatures, its worst
   * error at each share over every tested state, and `[cell][share][t]` each cell's u_mix(w, T),
   * cm⁻² (∞ for none, and for a sample inside an `estimated` interval), in the cells' order.
   */
  readonly mixing?: {
    readonly shares: ReadonlyArray<number>;
    readonly temperaturesK: ReadonlyArray<number>;
    readonly worstErrors: ReadonlyArray<number>;
    readonly limitColumnsCm2: ReadonlyArray<ReadonlyArray<ReadonlyArray<number>>>;
  };
  readonly worstQuantisationError: number;
  /** The 16-bit test's worst beyond the column caps, recorded, where the partials give it. */
  readonly worstQuantisationBeyondCapError?: number;
  /** The cells stored as their mean alone (G = 1, §9). */
  readonly linearCells: number;
  /**
   * The sidecar's description, where it is not R08.T4.d's: an ExoMol table's names its list and
   * licence (R08.T4.g).
   */
  readonly description?: string;
}

/** A binary part's file and what it holds, as the sidecar lists it. */
export interface KDistributionPart {
  readonly file: string;
  readonly cells: number;
  readonly bytes: number;
  readonly sha256: string;
}

/** The magic of a k-distribution part, `HYKD`, and its format's version. */
export const PART_MAGIC = "HYKD";

/** The binary part format's version, 1: the layout {@link kDistributionPart} writes. */
export const PART_VERSION = 1;

/** A binary part's header bytes: the magic, then six u16. */
const PART_HEADER_BYTES = 16;

/** A cell's bytes in a part: nm and G (u16), offset and step (f64), G weights (f64), its codes. */
export function cellBytes(cell: EncodedCell): number {
  return 2 + 2 + 8 + 8 + 8 * cell.weights.length + 2 * cell.codes.length;
}

/**
 * A k-distribution part as `absorbers/kDistribution.ts` reads it, little-endian
 * (`decision-r08-t4d-kdist-chain.md` §9): `HYKD`, u16 version, cell count, variants, temperatures,
 * pressures and 0; then per cell u16 nm, u16 G, f64 ln σ of code 1, f64 its step, G f64 weights,
 * and V × T × P × G u16 codes, `[((v × T + t) × P + p) × G + g]`.
 */
export function kDistributionPart(
  grid: ReductionGrid,
  variants: number,
  cells: ReadonlyArray<EncodedCell>,
): Uint8Array {
  const total = PART_HEADER_BYTES + cells.reduce((sum, c) => sum + cellBytes(c), 0);
  const bytes = new Uint8Array(total);
  const view = new DataView(bytes.buffer);
  for (let i = 0; i < PART_MAGIC.length; i += 1) {
    view.setUint8(i, PART_MAGIC.charCodeAt(i));
  }
  const header = [
    PART_VERSION,
    cells.length,
    variants,
    grid.temperaturesK.length,
    grid.pressuresPa.length,
    0,
  ];
  for (const [i, v] of header.entries()) {
    view.setUint16(4 + 2 * i, v, true);
  }
  let at = PART_HEADER_BYTES;
  for (const cell of cells) {
    const expected =
      variants * grid.temperaturesK.length * grid.pressuresPa.length * cell.weights.length;
    if (cell.codes.length !== expected) {
      throw new Error(`cell ${cell.cellNm} nm holds ${cell.codes.length} codes, not ${expected}`);
    }
    view.setUint16(at, cell.cellNm, true);
    view.setUint16(at + 2, cell.weights.length, true);
    view.setFloat64(at + 4, cell.lnOffset, true);
    view.setFloat64(at + 12, cell.lnStep, true);
    at += 20;
    for (const w of cell.weights) {
      view.setFloat64(at, w, true);
      at += 8;
    }
    for (const code of cell.codes) {
      view.setUint16(at, code, true);
      at += 2;
    }
  }
  return bytes;
}

/** A number to some significant figures in its shortest decimal form. */
function figures(value: number, digits: number): string {
  return value === 0 ? "0" : String(Number(value.toPrecision(digits)));
}

/** How a sidecar's description states its parts' layout and its values' axes. */
export const PART_LAYOUT_TEXT =
  "The cells are in the binary parts listed, little-endian: " +
  "HYKD, u16 version, cells, variants, temperatures, pressures and 0; then per cell u16 nm, u16 G, " +
  "f64 ln sigma (m2 a molecule) of code 1, f64 its step, G f64 weights, and u16 codes over " +
  "variants x temperaturesK x pressuresPa x g, g fastest, code 0 for sigma 0, else ln sigma = " +
  "offset + step (code - 1). pressuresPa are air-equivalent broadening pressures; a two-variant " +
  "table's values are mixed at fixed g by the self share of the Lorentz width. A cell of G = 1 " +
  "is stored as its mean (decision-r08-t4d-kdist-chain.md section 9).";

/**
 * A k-distribution table's sidecar, as `absorbers/kDistribution.ts` reads it: the grid, the
 * records, and its binary parts in order.
 */
export function kDistributionMetaText(
  meta: KDistributionMeta,
  grid: ReductionGrid,
  parts: ReadonlyArray<KDistributionPart>,
): string {
  const description =
    meta.description ??
    "Line absorbers' k-distributions per 1 nm cell (rendering plan R08, R08.T4.d; " +
      "decision-r08-t4d-kdist.md; decision-r08-t4d-kdist-chain.md), written by " +
      "apps/hyperion/scripts/atmosphereData.mjs k-distributions-merge from the fetched lines named " +
      "here, which are not committed. " +
      PART_LAYOUT_TEXT +
      " Reduced values; the citations are in NOTICE's Data section.";
  const fields = [
    `  "description": ${JSON.stringify(description)}`,
    `  "species": ${JSON.stringify(meta.species)}`,
    `  "source": ${JSON.stringify(meta.source)}`,
    `  "url": ${JSON.stringify(meta.url)}`,
    `  "sha256": ${JSON.stringify(meta.sha256)}`,
    `  "fetched": ${JSON.stringify(meta.fetched)}`,
    `  "terms": ${JSON.stringify(meta.terms)}`,
    `  "reduction": ${JSON.stringify(meta.reduction)}`,
    `  "basis": ${JSON.stringify(meta.basis)}`,
    `  "temperaturesK": [${grid.temperaturesK.join(", ")}]`,
    `  "measuredRangeK": [${meta.measuredRangeK.join(", ")}]`,
    `  "pressuresPa": [${grid.pressuresPa.map((p) => figures(p, 6)).join(", ")}]`,
    `  "measuredBelowGrid": ${String(meta.measuredBelowGrid)}`,
    `  "measuredShapeTo": [${meta.measuredShapeTo.join(", ")}]`,
    `  "estimatedTemperaturesK": [${meta.estimatedTemperaturesK.map(([a, b]) => `[${a}, ${b}]`).join(", ")}]`,
    `  "broadening": {${Object.entries(meta.broadening)
      .map(([gas, r]) => `${JSON.stringify(gas)}: ${figures(r, 4)}`)
      .join(", ")}}`,
    `  "broadeningExponents": {${Object.entries(meta.broadeningExponents)
      .map(([gas, d]) => `${JSON.stringify(gas)}: ${figures(d, 3)}`)
      .join(", ")}}`,
    `  "variants": [${meta.variants.map((v) => JSON.stringify(v)).join(", ")}]`,
    `  "chiHeld": [${meta.chiHeld.map((row) => `[${row.join(", ")}]`).join(", ")}]`,
    `  "beyondCutoffShare": [${meta.beyondCutoffShare.map((x) => figures(x, 2)).join(", ")}]`,
    `  "nearWingShare": [${meta.nearWingShare.map((x) => figures(x, 2)).join(", ")}]`,
    `  "worstNodeError": ${figures(meta.worstNodeError, 3)}`,
    `  "worstChainError": ${figures(meta.worstChainError, 3)}`,
    `  "worstBeyondCapError": ${figures(meta.worstBeyondCapError, 3)}`,
    ...(meta.mixing === undefined
      ? []
      : [
          `  "mixing": {"shares": [${meta.mixing.shares.join(", ")}], "temperaturesK": [${meta.mixing.temperaturesK.join(
            ", ",
          )}], "worstErrors": [${meta.mixing.worstErrors
            .map((x) => figures(x, 3))
            .join(", ")}], "limitColumnsCm2": [${meta.mixing.limitColumnsCm2
            .map(
              (cell) =>
                `[${cell
                  .map(
                    (row) =>
                      `[${row.map((u) => (Number.isFinite(u) ? figures(u, 3) : "null")).join(", ")}]`,
                  )
                  .join(", ")}]`,
            )
            .join(", ")}]}`,
        ]),
    `  "worstQuantisationError": ${figures(meta.worstQuantisationError, 3)}`,
    ...(meta.worstQuantisationBeyondCapError === undefined
      ? []
      : [
          `  "worstQuantisationBeyondCapError": ${figures(meta.worstQuantisationBeyondCapError, 3)}`,
        ]),
    `  "linearCells": ${meta.linearCells}`,
  ];
  const rows = parts.map(
    (p) =>
      `    {"file": ${JSON.stringify(p.file)}, "cells": ${p.cells}, "bytes": ${p.bytes}, "sha256": ${JSON.stringify(p.sha256)}}`,
  );
  return `{\n${fields.join(",\n")},\n  "parts": [\n${rows.join(",\n")}\n  ]\n}\n`;
}

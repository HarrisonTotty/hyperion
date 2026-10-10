/**
 * The optics of an aerosol mode of spheres over its size distribution (plan R08, R08.T5.b; Design
 * note 6): T5.a's Mie, summed by Gauss–Legendre quadrature in ln r, with the mode's scattering
 * matrix on the phase tables' angles.
 *
 * @remarks
 * A mode's distribution is Hansen and Travis 1974's standard gamma distribution or a log-normal,
 * each given by its effective radius and effective variance (J. E. Hansen and L. D. Travis, "Light
 * scattering in planetary atmospheres", Space Sci. Rev. 16, 527–610, eqs. (2.53)–(2.61)). The
 * quadrature runs in y = ln r over the interval where every weight the optics need lies above
 * {@link SIZE_WEIGHT_FLOOR} of its own peak: the geometric cross-section's weight r²n(r) at the
 * small end, which large particles' extinction follows, and the Rayleigh scattering weight r⁶n(r)
 * at the large end, which small particles' scattering follows. For a log-normal that is ±5.3σ
 * about each weight's centre. The rule is composite: Gauss–Legendre of order
 * {@link SIZE_PANEL_ORDER} on panels no wider than {@link SIZE_PANEL_SIZE_PARAMETER} in size
 * parameter and {@link SIZE_PANEL_LN_RADIUS} in ln r, every panel halved, so the node count doubles,
 * until the mean scattering and extinction efficiencies change by under {@link SIZE_CONVERGENCE} of
 * themselves and the asymmetry by under {@link SIZE_CONVERGENCE}.
 *
 * The matrix is block-diagonal (a₁, a₂, a₃, a₄, b₁, b₂; J. W. Hovenier, C. van der Mee and H.
 * Domke 2004, "Transfer of polarized light in planetary atmospheres", Kluwer), normalised so that
 * ∫ a₁ dΩ ÷ 4π = 1. From T5.a's amplitudes (S₁ perpendicular, S₂ parallel to the scattering plane,
 * time factor e^(+iωt) and index n − ik, Wiscombe's and Hansen and Travis's convention), with
 * Stokes Q = I_∥ − I_⊥ and V as Hansen and Travis's (1.4) defines it:
 *
 * - a₁ = a₂ ∝ ½(|S₁|² + |S₂|²);
 * - b₁ ∝ ½(|S₂|² − |S₁|²), so that −b₁ ÷ a₁ is the degree of linear polarisation, positive when the
 *   light is polarised perpendicular to the scattering plane;
 * - a₃ = a₄ ∝ Re(S₁S₂*);
 * - b₂ ∝ −Im(S₁S₂*) = Im(S₂S₁*).
 *
 * These are W. A. de Rooij and C. C. A. H. van der Stap 1984's (13)–(16) (Astron. Astrophys. 131,
 * 237–248), whose S₁ acts on the perpendicular field with m = n − ik, as T5.a's does; Hansen and
 * Travis's (2.33) and their Fig. 16 agree (their printed (2.36) and (2.41) swap S₁ and S₂ in the
 * 1–2 block). A sphere of x = 10⁻³ gives their Rayleigh matrix (2.15) with δ = 0. R08.T12.c's
 * sphere cases (Kokhanovsky et al. 2010, IPRT) check b₂ against published matrices. The medium
 * around the particle is taken as index 1, a relative error of 3 × 10⁻⁴ in air at the surface and
 * less above.
 */

import { gaussLegendre } from "../lighting/quadrature";
import {
  type MaterialForm,
  type MaterialProvenance,
  type ResolvedMaterial,
  type ShapeClass,
  resolveMaterial,
  indexAt,
} from "./materials/materials";
import {
  type ComplexIndex,
  MIE_MAX_SIZE_PARAMETER,
  MIE_MIN_SIZE_PARAMETER,
  mieSphere,
  type MieResult,
} from "./mie";

/**
 * A size distribution of spheres, by effective radius r_eff = ⟨r³⟩ ÷ ⟨r²⟩ and effective variance
 * v_eff = ⟨(r − r_eff)² r²⟩ ÷ (r_eff² ⟨r²⟩) (Hansen and Travis 1974, (2.53) and (2.54)).
 *
 * - `gamma`: the standard gamma distribution n(r) ∝ r^((1 − 3b) ÷ b) e^(−r ÷ ab), a = r_eff and
 *   b = v_eff (their (2.56)); 0 < v_eff < 0.5, the range in which it can be normalised.
 * - `logNormal`: n(r) ∝ r⁻¹ exp(−(ln r − ln r_g)² ÷ 2σ²), with σ² = ln(1 + v_eff) and
 *   r_g = r_eff (1 + v_eff)^(−5/2) (their (2.60) on p. 557, with the relations of their Fig. 14's footnote, p. 558); v_eff > 0.
 */
export type SizeDistribution =
  | {
      readonly kind: "gamma";
      readonly effectiveRadiusUm: number;
      readonly effectiveVariance: number;
    }
  | {
      readonly kind: "logNormal";
      readonly effectiveRadiusUm: number;
      readonly effectiveVariance: number;
    };

/**
 * The fraction of its own peak at which a quadrature weight is cut, the ends of the ln r interval
 * (±5.26σ for a Gaussian weight).
 */
export const SIZE_WEIGHT_FLOOR = 1e-6;

/** The Gauss–Legendre order of each panel of the composite rule. */
export const SIZE_PANEL_ORDER = 8;

/**
 * The widest a panel may be in size parameter x = 2πr ÷ λ at the coarsest level.
 *
 * @remarks
 * Mie's ripple repeats every Δx ≈ arctan(√(m² − 1)) ÷ √(m² − 1) (P. Chýlek, "Partial-wave
 * resonances and the ripple structure in the Mie normalized extinction cross section", J. Opt.
 * Soc. Am. 66, 285–287, 1976), 0.82 at m = 1.33 and 0.66 at m = 1.8, at every x. A global rule in
 * ln r spaces its nodes as x and so starves the large particles' ripple; panels of fixed width in x
 * give the ripple the same number of nodes everywhere.
 */
export const SIZE_PANEL_SIZE_PARAMETER = 2;

/** The widest a panel may be in ln r at the coarsest level, so the distribution's own shape is resolved. */
export const SIZE_PANEL_LN_RADIUS = 0.5;

/** The node count past which the quadrature gives up as unconverged. */
export const SIZE_NODES_MAX = 65_536;

/** The finest refinement {@link sizeQuadrature} gives: its panels 2⁻¹⁶ of the coarsest's. */
export const SIZE_LEVEL_MAX = 16;

/** The convergence tolerance on the mean efficiencies (relative) and the asymmetry (absolute). */
export const SIZE_CONVERGENCE = 1e-4;

/** The number of phase-table angles, on u = √(θ ÷ π) (Design note 6). */
export const PHASE_TABLE_SIZE = 256;

/** The phase tables' abscissae u = √(θ ÷ π), from 0 (forward) to 1 (backward), evenly spaced. */
export const PHASE_TABLE_U: Float64Array = Float64Array.from(
  { length: PHASE_TABLE_SIZE },
  (_, i) => i / (PHASE_TABLE_SIZE - 1),
);

/** The cosines of the phase tables' scattering angles, cos(π u²), from 1 to −1; shared, so never written (each matrix holds its own copy). */
export const PHASE_TABLE_MU: Float64Array = PHASE_TABLE_U.map((u) => Math.cos(Math.PI * u * u));

/**
 * A scattering matrix's six elements at a set of scattering-angle cosines, block-diagonal and
 * normalised so that ∫ a₁ dΩ ÷ 4π = 1 − {@link ScatteringMatrix.forwardPeak} (the module's remarks
 * give each element's form).
 */
export interface ScatteringMatrix {
  /** The cosines the elements are given at. */
  readonly mu: Float64Array;
  /**
   * The share of the scattering in a forward peak narrower than the source's angles resolve, held
   * as a delta at θ = 0 with the identity's matrix (a₂ = a₃ = a₄ = a₁, b₁ = b₂ = 0, as diffraction
   * scatters): 0 for Mie and the MMF, which are sampled at the cosines asked; a phase file's
   * unresolved diffraction peak for the tabulated models (`nonSpherical.ts`).
   */
  readonly forwardPeak: number;
  readonly a1: Float64Array;
  readonly a2: Float64Array;
  readonly a3: Float64Array;
  readonly a4: Float64Array;
  readonly b1: Float64Array;
  readonly b2: Float64Array;
}

/** The ends of a size distribution's quadrature interval, µm. */
export interface SizeRange {
  readonly fromUm: number;
  readonly toUm: number;
}

/** A Gauss–Legendre rule in ln r over a distribution's range. */
export interface SizeQuadrature {
  /** The nodes' radii, µm. */
  readonly radiiUm: Float64Array;
  /** Each node's share of the distribution's geometric cross-section, r² n(r) dr, summing to 1. */
  readonly areaWeights: Float64Array;
}

/** The bulk optics of a mode at one wavelength, per particle and per unit geometric cross-section. */
export interface ModeBulkOptics {
  readonly wavelengthNm: number;
  /** The area-weighted mean extinction efficiency, ⟨σ_ext⟩ ÷ ⟨πr²⟩. */
  readonly qExt: number;
  /** The area-weighted mean scattering efficiency, ⟨σ_sca⟩ ÷ ⟨πr²⟩. */
  readonly qSca: number;
  /** The single-scattering albedo, ⟨σ_sca⟩ ÷ ⟨σ_ext⟩. */
  readonly singleScatteringAlbedo: number;
  /** The asymmetry parameter, the mean cosine of the scattering angle. */
  readonly asymmetry: number;
  /** The mean geometric cross-section per particle, ⟨πr²⟩, m². */
  readonly geometricCrossSectionM2: number;
  /** The mean extinction cross-section per particle, m². */
  readonly extinctionCrossSectionM2: number;
  /** The mean scattering cross-section per particle, m². */
  readonly scatteringCrossSectionM2: number;
  /** The mean volume per particle, (4/3)π⟨r³⟩, m³. */
  readonly volumeM3: number;
}

/** The optics of a mode of spheres at one wavelength, as {@link sphereModeOptics} gives them. */
export interface SphereModeOptics extends ModeBulkOptics {
  /** The mode's matrix, summed once at the converged level (not itself a convergence test). */
  readonly matrix: ScatteringMatrix;
  /** The quadrature's node count at the converged level. */
  readonly nodes: number;
}

/** A mode of aerosol particles by its registry material (decision-composition §1.9). */
export interface AerosolMode {
  /** The sim's material key (P14.T49.b), never narrowed: an unknown key takes the generic stand-in. */
  readonly material: string;
  /** The phase or variant the mode is in, where its material has several files. */
  readonly form?: MaterialForm;
  /** The particles' shape class, where the mode states it; otherwise its material file's. */
  readonly shape?: ShapeClass;
  readonly sizes: SizeDistribution;
}

/**
 * What a mode's phase function rests on (R08.T5.c; science-r08-nonspherical.md):
 *
 * - `model`: a published model inside its range: Mie for spheres, the MMF for an aggregate inside
 *   its phase-shift gate, a TAMUdust2020 kernel for a non-spherical mineral, Yang et al. 2013 for
 *   water ice;
 * - `analogue`: a table stands in past its validated range: the k-floor rule exceeded, another ice
 *   on water ice's table, water ice's crystals below the table's smallest size, or an aggregate
 *   past Tazaki and Tanaka's (9) whose own term 2kR_c|m_MG − 1| is below 1 (the MMF table kept,
 *   science-r08-mmf.md);
 * - `fallback`: the Henyey–Greenstein phase from Mie's or the MMF's g, or, for a small grain of a
 *   mineral no kernel covers, the sphere's own Mie table.
 *
 * An `analogue` or `fallback` mode with less than `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` above it at
 * 550 nm gives `atmosphereApproximate` (Design note 12; `aerosol.ts`).
 */
export type PhaseBasis = "model" | "analogue" | "fallback";

/**
 * The model a mode's phase function and matrix come from: Mie's for spheres, the MMF's for
 * aggregates (`aggregate.ts`), TAMUdust2020's irregular hexahedra and Yang et al. 2013's roughened
 * column aggregate (`nonSpherical.ts`), or Henyey–Greenstein's from g, which has no matrix.
 */
export type PhaseModel = "mie" | "mmf" | "tamudust2020" | "yang2013" | "henyeyGreenstein";

/**
 * A mode's optics at one wavelength, the shape class they hold for, where its index came from, and
 * what its phase function rests on.
 *
 * @remarks
 * From {@link modeOptics}, every field is Mie's. For a `sphere` that is the model; for any other
 * shape only the cross-sections are (Design note 6), so ω and g are the spheres' values, the phase
 * is Henyey–Greenstein's from that g (`phaseBasis` `fallback`), and `matrix` is undefined, a total
 * depolariser, since a sphere's matrix is not the particle's (decision-r08-vector.md). R08.T5.c's
 * `nonSphericalModeOptics` and `aggregateOptics` give the published models' optics in the same type.
 */
export interface ModeOptics extends ModeBulkOptics {
  readonly matrix: ScatteringMatrix | undefined;
  readonly shape: ShapeClass;
  readonly provenance: MaterialProvenance;
  readonly phaseBasis: PhaseBasis;
  readonly phaseModel: PhaseModel;
  /** Why an `analogue` or `fallback` is one, for the label's reasons; undefined for a `model`. */
  readonly phaseNote: string | undefined;
}

/** A distribution's weights in y = ln(r ÷ 1 µm), for the quadrature and its interval. */
interface LogWeight {
  /** ln(r n(r)) up to a constant, at y = ln(r ÷ 1 µm): the number weight per unit y. */
  readonly logNumberPerY: (y: number) => number;
  /**
   * One end, in y, of where the weight rᵖ n(r) dr lies above {@link SIZE_WEIGHT_FLOOR} of its peak:
   * below the peak for side −1, above it for +1.
   */
  readonly cut: (p: number, side: -1 | 1) => number;
  /** ⟨r²⟩ over the whole distribution, µm². */
  readonly meanSquareRadiusUm2: number;
}

const LOG_FLOOR = -Math.log(SIZE_WEIGHT_FLOOR);

/** No scattering angles: Mie then sums the efficiencies alone. */
const NO_ANGLES = new Float64Array(0);

function checkSizes(sizes: SizeDistribution): void {
  const { effectiveRadiusUm: a, effectiveVariance: b } = sizes;
  if (!(a > 0 && Number.isFinite(a))) {
    throw new RangeError(`effective radius ${a} µm must be finite and > 0`);
  }
  switch (sizes.kind) {
    case "gamma":
      if (!(b > 0 && b < 0.5)) {
        throw new RangeError(`a gamma distribution's effective variance ${b} must lie in (0, 0.5)`);
      }
      return;
    case "logNormal":
      if (!(b > 0 && Number.isFinite(b))) {
        throw new RangeError(`a log-normal's effective variance ${b} must be finite and > 0`);
      }
      return;
  }
}

/**
 * The standard gamma distribution's weights: n(r) ∝ r^(α − 1) e^(−r/θ) with α = (1 − 2b) ÷ b and
 * θ = ab, so rᵖ r n(r) in y is e^((α + p)y − e^y/θ), whose peak is at e^y = (α + p)θ; and
 * ⟨r²⟩ = a²(1 − b)(1 − 2b).
 */
function gammaWeight(a: number, b: number): LogWeight {
  const alpha = (1 - 2 * b) / b;
  const theta = a * b;
  return {
    logNumberPerY: (y) => alpha * y - Math.exp(y) / theta,
    cut: (p, side) => gammaCut(alpha + p, theta, side),
    meanSquareRadiusUm2: a * a * (1 - b) * (1 - 2 * b),
  };
}

/**
 * The log-normal's weights: r n(r) in y is a Gaussian of mean ln r_g and variance σ², so rᵖ r n(r)
 * is one of mean ln r_g + pσ²; and ⟨r²⟩ = r_g² e^(2σ²) = r_eff² (1 + v_eff)⁻³.
 */
function logNormalWeight(a: number, b: number): LogWeight {
  const sigma2 = Math.log1p(b);
  const sigma = Math.sqrt(sigma2);
  const mu = Math.log(a) - 2.5 * sigma2;
  return {
    logNumberPerY: (y) => -((y - mu) ** 2) / (2 * sigma2),
    cut: (p, side) => mu + p * sigma2 + side * sigma * Math.sqrt(2 * LOG_FLOOR),
    meanSquareRadiusUm2: (a * a) / (1 + b) ** 3,
  };
}

function logWeight(sizes: SizeDistribution): LogWeight {
  checkSizes(sizes);
  let weight: LogWeight;
  switch (sizes.kind) {
    case "gamma":
      weight = gammaWeight(sizes.effectiveRadiusUm, sizes.effectiveVariance);
      break;
    case "logNormal":
      weight = logNormalWeight(sizes.effectiveRadiusUm, sizes.effectiveVariance);
      break;
  }
  return weight;
}

/**
 * The y = ln r where e^(cy − e^y/θ) falls to {@link SIZE_WEIGHT_FLOOR} of its peak on one side: with
 * z = e^y ÷ θ, z − c − c ln(z ÷ c) = ln(1 ÷ floor), solved by bisection in ln z.
 */
function gammaCut(c: number, theta: number, side: -1 | 1): number {
  const peak = Math.log(c);
  const excess = (lnZ: number): number => Math.exp(lnZ) - c - c * (lnZ - peak) - LOG_FLOOR;
  let inner = peak;
  let outer = peak + side;
  while (excess(outer) < 0) {
    outer += side * (Math.abs(outer - peak) + 1);
  }
  for (let i = 0; i < 200; i += 1) {
    const middle = 0.5 * (inner + outer);
    if (excess(middle) < 0) {
      inner = middle;
    } else {
      outer = middle;
    }
  }
  return 0.5 * (inner + outer) + Math.log(theta);
}

/**
 * The interval a distribution's quadrature covers: from where its geometric-cross-section weight
 * r² n(r) falls to {@link SIZE_WEIGHT_FLOOR} of its peak below, to where its Rayleigh scattering
 * weight r⁶ n(r) does above.
 *
 * @throws RangeError for an effective radius or variance outside the distribution's range.
 */
export function sizeRange(sizes: SizeDistribution): SizeRange {
  const weight = logWeight(sizes);
  return { fromUm: Math.exp(weight.cut(2, -1)), toUm: Math.exp(weight.cut(6, 1)) };
}

/**
 * A distribution's number density per unit ln r, ln(r n(r)) up to a constant, as a function of
 * ln(r ÷ 1 µm): R08.T5.c's tables integrate a mode over their own size nodes with it.
 *
 * @throws RangeError for an effective radius or variance outside the distribution's range.
 */
export function logNumberPerLnRadius(sizes: SizeDistribution): (lnRadiusUm: number) => number {
  return logWeight(sizes).logNumberPerY;
}

const PANEL_RULE = gaussLegendre(SIZE_PANEL_ORDER);

/**
 * The panels' ends in y = ln r at one level: each panel spans at most
 * {@link SIZE_PANEL_SIZE_PARAMETER} ÷ 2^level in size parameter and
 * {@link SIZE_PANEL_LN_RADIUS} ÷ 2^level in ln r.
 */
function panelEnds(low: number, high: number, wavenumberPerUm: number, level: number): number[] {
  const dx = SIZE_PANEL_SIZE_PARAMETER / 2 ** level;
  const dy = SIZE_PANEL_LN_RADIUS / 2 ** level;
  const ends = [low];
  let y = low;
  while (y < high) {
    const x = wavenumberPerUm * Math.exp(y);
    y = Math.min(high, y + Math.min(dy, Math.log1p(dx / x)));
    ends.push(y);
  }
  return ends;
}

function quadratureOn(
  sizes: SizeDistribution,
  range: SizeRange,
  wavenumberPerUm: number,
  level: number,
): SizeQuadrature {
  const weight = logWeight(sizes);
  const ends = panelEnds(Math.log(range.fromUm), Math.log(range.toUm), wavenumberPerUm, level);
  const nodes = (ends.length - 1) * SIZE_PANEL_ORDER;
  const radiiUm = new Float64Array(nodes);
  const logArea = new Float64Array(nodes);
  let peak = -Infinity;
  for (let p = 0; p + 1 < ends.length; p += 1) {
    const from = ends[p] ?? Number.NaN;
    const half = 0.5 * ((ends[p + 1] ?? Number.NaN) - from);
    for (let i = 0; i < SIZE_PANEL_ORDER; i += 1) {
      const y = from + half * (1 + (PANEL_RULE.x[i] ?? 0));
      const node = p * SIZE_PANEL_ORDER + i;
      radiiUm[node] = Math.exp(y);
      logArea[node] = Math.log(half * (PANEL_RULE.w[i] ?? 0)) + weight.logNumberPerY(y) + 2 * y;
      peak = Math.max(peak, logArea[node] ?? -Infinity);
    }
  }
  const areaWeights = logArea.map((l) => Math.exp(l - peak));
  const total = areaWeights.reduce((sum, w) => sum + w, 0);
  return { radiiUm, areaWeights: areaWeights.map((w) => w / total) };
}

/**
 * The composite Gauss–Legendre rule in ln r over {@link sizeRange}, at one wavelength and level of
 * refinement: {@link sphereModeOptics}'s, which also leaves out the radii below Mie's smallest size
 * parameter.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, finite and > 0, which sets the panels' widths
 *   in size parameter.
 * @param level - The refinement, an integer in 0–{@link SIZE_LEVEL_MAX}, 0 the coarsest; each level
 *   halves every panel.
 * @throws RangeError for an effective radius or variance outside the distribution's range, a bad
 *   wavelength or a bad level.
 */
export function sizeQuadrature(
  sizes: SizeDistribution,
  wavelengthNm: number,
  level = 0,
): SizeQuadrature {
  if (!(wavelengthNm > 0 && Number.isFinite(wavelengthNm))) {
    throw new RangeError(`wavelength ${wavelengthNm} nm must be finite and > 0`);
  }
  if (!(Number.isInteger(level) && level >= 0 && level <= SIZE_LEVEL_MAX)) {
    throw new RangeError(`level ${level} must be an integer in 0–${SIZE_LEVEL_MAX}`);
  }
  return quadratureOn(sizes, sizeRange(sizes), (2 * Math.PI * 1000) / wavelengthNm, level);
}

/**
 * One sphere's scattering matrix at the cosines its amplitudes were computed at, normalised as
 * {@link ScatteringMatrix} is.
 *
 * @param result - {@link mieSphere}'s result.
 * @param sizeParameter - The size parameter it was computed at.
 * @param mu - The cosines it was computed at.
 */
export function sphereScatteringMatrix(
  result: MieResult,
  sizeParameter: number,
  mu: Float64Array,
): ScatteringMatrix {
  const matrix = emptyMatrix(mu);
  accumulate(matrix, result, 1);
  return finishMatrix(matrix, sizeParameter * sizeParameter * result.qSca);
}

interface MutableMatrix {
  readonly mu: Float64Array;
  readonly a1: Float64Array;
  readonly a3: Float64Array;
  readonly b1: Float64Array;
  readonly b2: Float64Array;
}

function emptyMatrix(mu: Float64Array): MutableMatrix {
  const length = mu.length;
  return {
    mu,
    a1: new Float64Array(length),
    a3: new Float64Array(length),
    b1: new Float64Array(length),
    b2: new Float64Array(length),
  };
}

/** Adds one sphere's |S|² terms, weighted, to the sums of a mode's matrix. */
function accumulate(matrix: MutableMatrix, result: MieResult, weight: number): void {
  const { s1, s2 } = result;
  for (let j = 0; j < matrix.mu.length; j += 1) {
    const s1Re = s1[2 * j] ?? 0;
    const s1Im = s1[2 * j + 1] ?? 0;
    const s2Re = s2[2 * j] ?? 0;
    const s2Im = s2[2 * j + 1] ?? 0;
    const s1Sq = s1Re * s1Re + s1Im * s1Im;
    const s2Sq = s2Re * s2Re + s2Im * s2Im;
    // S₁S₂* = (s1Re s2Re + s1Im s2Im) + i (s1Im s2Re − s1Re s2Im), and b₂ takes −Im(S₁S₂*).
    matrix.a1[j] = (matrix.a1[j] ?? 0) + weight * 0.5 * (s1Sq + s2Sq);
    matrix.b1[j] = (matrix.b1[j] ?? 0) + weight * 0.5 * (s2Sq - s1Sq);
    matrix.a3[j] = (matrix.a3[j] ?? 0) + weight * (s1Re * s2Re + s1Im * s2Im);
    matrix.b2[j] = (matrix.b2[j] ?? 0) + weight * (s1Re * s2Im - s1Im * s2Re);
  }
}

/**
 * Normalises the sums: with F = Σ wᵢ ½(|S₁|² + |S₂|²) and the scattering sum Σ wᵢ xᵢ² Q_sca,ᵢ,
 * a₁ = 4 F ÷ Σ wᵢ xᵢ² Q_sca,ᵢ, so that ∫ a₁ dΩ ÷ 4π = 1 (since ∫₋₁¹ (|S₁|² + |S₂|²) dμ = x² Q_sca).
 */
function finishMatrix(matrix: MutableMatrix, scatteringSum: number): ScatteringMatrix {
  const scale = 4 / scatteringSum;
  const a1 = matrix.a1.map((v) => v * scale);
  const a3 = matrix.a3.map((v) => v * scale);
  return {
    mu: matrix.mu.slice(),
    forwardPeak: 0,
    a1,
    a2: a1.slice(),
    a3,
    a4: a3.slice(),
    b1: matrix.b1.map((v) => v * scale),
    b2: matrix.b2.map((v) => v * scale),
  };
}

interface Sums {
  readonly area: number;
  readonly ext: number;
  readonly sca: number;
  readonly scaG: number;
}

/** The area-weighted efficiency sums at one node count; with `matrix`, the angular sums too. */
function sumNodes(
  quadrature: SizeQuadrature,
  index: ComplexIndex,
  wavelengthUm: number,
  mu: Float64Array,
  matrix: MutableMatrix | undefined,
): Sums {
  let ext = 0;
  let sca = 0;
  let scaG = 0;
  let area = 0;
  const wavenumber = (2 * Math.PI) / wavelengthUm;
  for (let i = 0; i < quadrature.radiiUm.length; i += 1) {
    const x = wavenumber * (quadrature.radiiUm[i] ?? 0);
    const w = quadrature.areaWeights[i] ?? 0;
    const result = mieSphere(x, index, matrix === undefined ? NO_ANGLES : mu);
    area += w;
    ext += w * result.qExt;
    sca += w * result.qSca;
    scaG += w * result.qSca * result.asymmetry;
    if (matrix !== undefined) {
      // The area weight is x² n(r) dr up to a constant, and the matrix sums take n(r) dr.
      accumulate(matrix, result, w / (x * x));
    }
  }
  return { area, ext, sca, scaG };
}

function relative(a: number, b: number): number {
  return Math.abs(a - b) / Math.abs(b);
}

function converged(coarse: Sums, fine: Sums): boolean {
  return (
    relative(coarse.ext / coarse.area, fine.ext / fine.area) < SIZE_CONVERGENCE &&
    relative(coarse.sca / coarse.area, fine.sca / fine.area) < SIZE_CONVERGENCE &&
    Math.abs(coarse.scaG / coarse.sca - fine.scaG / fine.sca) < SIZE_CONVERGENCE
  );
}

/**
 * The optics of a mode of homogeneous spheres of one index at one wavelength, over its size
 * distribution.
 *
 * @remarks
 * The radii below the one at {@link MIE_MIN_SIZE_PARAMETER} are left out of the interval: their
 * weight in scattering, r⁶, is about 5 × 10⁻²⁶ of a 1 nm particle's at 380 nm.
 *
 * @param wavelengthNm - The vacuum wavelength, nm.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError if the distribution reaches past {@link MIE_MAX_SIZE_PARAMETER} at this
 *   wavelength (a geometric-optics particle, not modelled here), or for a bad distribution or index.
 * @throws Error if the quadrature has not converged by {@link SIZE_NODES_MAX} nodes.
 */
export function sphereModeOptics(
  sizes: SizeDistribution,
  index: ComplexIndex,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): SphereModeOptics {
  if (!(wavelengthNm > 0 && Number.isFinite(wavelengthNm))) {
    throw new RangeError(`wavelength ${wavelengthNm} nm must be finite and > 0`);
  }
  const wavelengthUm = wavelengthNm / 1000;
  const full = sizeRange(sizes);
  const smallestUm = (MIE_MIN_SIZE_PARAMETER * wavelengthUm) / (2 * Math.PI);
  const largestUm = (MIE_MAX_SIZE_PARAMETER * wavelengthUm) / (2 * Math.PI);
  if (full.toUm > largestUm) {
    throw new RangeError(
      `the distribution reaches ${full.toUm} µm, past Mie's x = ${MIE_MAX_SIZE_PARAMETER} at ${wavelengthNm} nm`,
    );
  }
  if (full.toUm <= smallestUm) {
    throw new RangeError(
      `the distribution lies below Mie's x = ${MIE_MIN_SIZE_PARAMETER} at ${wavelengthNm} nm`,
    );
  }
  const range = { fromUm: Math.max(full.fromUm, smallestUm), toUm: full.toUm };
  const wavenumberPerUm = (2 * Math.PI) / wavelengthUm;
  let coarse = sumNodes(
    quadratureOn(sizes, range, wavenumberPerUm, 0),
    index,
    wavelengthUm,
    mu,
    undefined,
  );
  for (let level = 1; ; level += 1) {
    const quadrature = quadratureOn(sizes, range, wavenumberPerUm, level);
    const nodes = quadrature.radiiUm.length;
    if (nodes > SIZE_NODES_MAX) {
      throw new Error(`the size quadrature did not converge by ${SIZE_NODES_MAX} nodes`);
    }
    const fine = sumNodes(quadrature, index, wavelengthUm, mu, undefined);
    if (converged(coarse, fine)) {
      const sums = emptyMatrix(mu);
      const final = sumNodes(quadrature, index, wavelengthUm, mu, sums);
      return finish(sizes, wavelengthNm, final, sums, nodes);
    }
    coarse = fine;
  }
}

/**
 * The mode's optics from its converged sums. The matrix sums took each area weight w ∝ x² n(r) dr
 * divided by x², so the scattering sum that normalises them is Σ w Q_sca.
 */
function finish(
  sizes: SizeDistribution,
  wavelengthNm: number,
  sums: Sums,
  matrix: MutableMatrix,
  nodes: number,
): SphereModeOptics {
  const weight = logWeight(sizes);
  const qExt = sums.ext / sums.area;
  const qSca = sums.sca / sums.area;
  const geometricUm2 = Math.PI * weight.meanSquareRadiusUm2;
  return {
    wavelengthNm,
    qExt,
    qSca,
    singleScatteringAlbedo: sums.sca / sums.ext,
    asymmetry: sums.scaG / sums.sca,
    geometricCrossSectionM2: geometricUm2 * 1e-12,
    extinctionCrossSectionM2: qExt * geometricUm2 * 1e-12,
    scatteringCrossSectionM2: qSca * geometricUm2 * 1e-12,
    volumeM3: (4 / 3) * Math.PI * sizes.effectiveRadiusUm * weight.meanSquareRadiusUm2 * 1e-18,
    matrix: finishMatrix(matrix, sums.sca),
    nodes,
  };
}

/**
 * A mode's material resolved for its shape: a mode declared a non-spherical mineral or a crystal,
 * with no phase given, is a solid (water ice, not water; solid methane, not liquid), since
 * `resolveMaterial`'s default file is a key's first, its liquid where it has one.
 */
export function resolveModeMaterial(mode: AerosolMode): ResolvedMaterial {
  const solidShape = mode.shape === "nonSphericalMineral" || mode.shape === "crystal";
  const form: MaterialForm | undefined =
    solidShape && mode.form?.phase === undefined ? { ...mode.form, phase: "solid" } : mode.form;
  return resolveMaterial(mode.material, form);
}

/**
 * A mode's shape class: its own where it states one; a `sphere` if it is a liquid, whatever its
 * file says (a droplet: liquid iron is Mie, solid iron a `nonSphericalMineral`, Visscher et al.
 * 2010; science-r08-nonspherical.md §2.2); else its material file's; else a `sphere`, the generic
 * stand-in's convention (decision-composition §1.9).
 */
export function modeShape(mode: AerosolMode, material: ResolvedMaterial): ShapeClass {
  if (mode.shape !== undefined) {
    return mode.shape;
  }
  if ((mode.form?.phase ?? material.file?.phase) === "liquid") {
    return "sphere";
  }
  return material.file?.shape ?? "sphere";
}

/**
 * A mode's optics at one wavelength, over its size distribution, with its material's index from
 * the registry.
 *
 * @remarks
 * Every mode goes through sphere Mie here. For a non-spherical mineral or a crystal, only the
 * cross-sections are to be used, since a sphere's rainbow and glory are spurious for them (Design
 * note 6): the result says so by its `shape`, carries no matrix, and is a `fallback` whose phase is
 * Henyey–Greenstein's from this g. The published models for those classes are R08.T5.c's
 * `nonSphericalModeOptics`, which `aerosol.ts`'s `aerosolModeOptics` routes them to.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError as {@link sphereModeOptics} does, for a wavelength outside the files' range,
 *   or for an aggregate, whose optics are R08.T5.c's `aggregateOptics`.
 * @throws Error if the quadrature has not converged by {@link SIZE_NODES_MAX} nodes.
 */
export function modeOptics(
  mode: AerosolMode,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): ModeOptics {
  const material = resolveModeMaterial(mode);
  const shape = modeShape(mode, material);
  let keepsMatrix: boolean;
  switch (shape) {
    case "sphere":
      keepsMatrix = true;
      break;
    case "nonSphericalMineral":
    case "crystal":
      // A sphere's matrix is not the particle's, so its amplitudes are not summed.
      keepsMatrix = false;
      break;
    case "aggregate":
      throw new RangeError(`a mode of ${mode.material} aggregates takes aggregateOptics, not Mie`);
  }
  const index = indexAt(material, wavelengthNm);
  const optics = sphereModeOptics(mode.sizes, index, wavelengthNm, keepsMatrix ? mu : NO_ANGLES);
  return {
    wavelengthNm: optics.wavelengthNm,
    qExt: optics.qExt,
    qSca: optics.qSca,
    singleScatteringAlbedo: optics.singleScatteringAlbedo,
    asymmetry: optics.asymmetry,
    geometricCrossSectionM2: optics.geometricCrossSectionM2,
    extinctionCrossSectionM2: optics.extinctionCrossSectionM2,
    scatteringCrossSectionM2: optics.scatteringCrossSectionM2,
    volumeM3: optics.volumeM3,
    matrix: keepsMatrix ? optics.matrix : undefined,
    shape,
    provenance: material.provenance,
    phaseBasis: keepsMatrix ? "model" : "fallback",
    phaseModel: keepsMatrix ? "mie" : "henyeyGreenstein",
    phaseNote: keepsMatrix
      ? undefined
      : `a ${shape} of ${mode.material} through sphere Mie: Henyey–Greenstein from Mie's g, a fallback`,
  };
}

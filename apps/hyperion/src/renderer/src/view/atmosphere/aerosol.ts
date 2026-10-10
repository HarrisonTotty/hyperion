/**
 * An aerosol mode as a medium term (plan R08, R08.T5.c; Design notes 2, 6 and 12): its optics by
 * shape, its √θ phase tables per channel with the matrix where the model gives one, the delta-M
 * series the bakes truncate, its density on the column, and the label its phase basis gives.
 *
 * @remarks
 * - **Optics by shape.** A liquid is a sphere whatever its file says (a droplet; liquid iron is a
 *   `sphere` by the ruling, solid iron a `nonSphericalMineral`); spheres take Mie
 *   (`sizeDistribution.ts`), minerals and crystals the published models (`nonSpherical.ts`), and
 *   aggregates the MMF (`aggregate.ts`).
 * - **The tables** stand on {@link PHASE_TABLE_U}, u = √(θ ÷ π), 256 entries. Each channel's a₁
 *   is divided by 4π (the matrices are normalised to ∫ a₁ dΩ ÷ 4π = 1, R08.T6.a's `PhaseTable` is
 *   in sr⁻¹) and then by the exact integral of its own linear-in-u interpolant, the reading
 *   `phaseAt` gives it, so that the table integrates to 1 as read; the other elements take the same
 *   factors. A phase file's unresolved forward peak is laid on the first entries, sized so that
 *   the table's mean cosine as read is the mode's g ({@link aerosolPhaseTable}). A
 *   Henyey–Greenstein fallback is tabulated from its g the same way, with no matrix.
 * - **The bakes' series.** Single scattering reads the full table and extinction; only the
 *   multiple-scattering bakes truncate the forward peak, by delta-M (W. J. Wiscombe, "The delta-M
 *   method: Rapid yet accurate radiative flux calculations for strongly asymmetric phase
 *   functions", J. Atmos. Sci. 34 (1977) 1408–1422): with 2M streams the fraction f = χ_2M and the
 *   moments (χ_l − f) ÷ (1 − f), l < 2M ({@link deltaMTruncation}).
 * - **The density.** Between its base and top pressures the mode's mixing ratio goes as
 *   (p ÷ p_base)^f, f its deck's f_sed or 0 for a well-mixed layer (P14.T24.c's profile), so its
 *   number density is that ratio times the column's; it is laid on the column's levels with a level
 *   at each end, and scaled so that its vertical optical depth at 550 nm is the mode's.
 */

import { gaussLegendre } from "../lighting/quadrature";
import { type AggregateMode, aggregateOptics } from "./aggregate";
import type { AtmosphereColumn } from "./column";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "./thick/regime";
import type { AtmosphereLabel } from "./labels";
import {
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  type MediumTerm,
  type PhaseMatrixTable,
  type PhaseTable,
  phaseTable,
  tabulatedDensity,
} from "./medium";
import {
  ELEMENT_INDICES,
  forwardPeakSplit,
  ICE_ANALOGUE_ASYMMETRY_BIAS,
  nonSphericalModeOptics,
} from "./nonSpherical";
import {
  type AerosolMode,
  modeOptics,
  type ModeOptics,
  modeShape,
  type PhaseBasis,
  type PhaseModel,
  PHASE_TABLE_MU,
  PHASE_TABLE_U,
  resolveModeMaterial,
} from "./sizeDistribution";
import type { Rgb } from "../photometry/toneCurve";

/** An aerosol mode's particles: a size distribution of one material, or identical aggregates. */
export type AerosolParticles =
  | { readonly kind: "distribution"; readonly mode: AerosolMode }
  | { readonly kind: "aggregate"; readonly mode: AggregateMode };

/**
 * A mode's optics at one wavelength by its shape (the module's remarks).
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError for a distribution whose shape is `aggregate` (an aggregate takes its
 *   monomers), or as the shape's own optics do.
 */
export function aerosolModeOptics(
  particles: AerosolParticles,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): ModeOptics {
  let optics: ModeOptics;
  switch (particles.kind) {
    case "aggregate":
      optics = aggregateOptics(particles.mode, wavelengthNm, mu);
      break;
    case "distribution":
      optics = distributionOptics(particles.mode, wavelengthNm, mu);
      break;
  }
  return optics;
}

/** A size distribution's optics by its shape: a liquid is a sphere whatever its file says. */
function distributionOptics(mode: AerosolMode, wavelengthNm: number, mu: Float64Array): ModeOptics {
  const shape = modeShape(mode, resolveModeMaterial(mode));
  let optics: ModeOptics;
  switch (shape) {
    case "sphere":
      optics = modeOptics({ ...mode, shape }, wavelengthNm, mu);
      break;
    case "nonSphericalMineral":
    case "crystal":
      optics = nonSphericalModeOptics({ ...mode, shape }, wavelengthNm, mu);
      break;
    case "aggregate":
      throw new RangeError(
        `a distribution of ${mode.material} cannot be an aggregate: an aggregate mode names its monomers`,
      );
  }
  return optics;
}

// ---------------------------------------------------------------------------------------------
// The phase tables.

/** The Gauss–Legendre rule on each interval of a table's integrals (order 8), as (node, weight). */
const TABLE_GL: ReadonlyArray<readonly [number, number]> = (() => {
  const rule = gaussLegendre(8);
  return Array.from(rule.x, (x, i) => [x, rule.w[i] ?? Number.NaN] as const);
})();

/**
 * ∫ p P(μ) dΩ of a function linear in u between a table's entries, with P a weight in μ = cos πu²:
 * 2π ∫₀¹ p(u) P(μ(u)) sin(πu²) 2πu du, by Gauss–Legendre on each interval (exact to rounding for
 * the interpolant the table is read with).
 */
function tableIntegral(
  u: Float64Array,
  values: Float64Array,
  weight: (mu: number) => number,
): number {
  let sum = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const v0 = values[i] ?? Number.NaN;
    const v1 = values[i + 1] ?? Number.NaN;
    const half = 0.5 * (u1 - u0);
    for (const [x, w] of TABLE_GL) {
      const t = 0.5 * (1 + x);
      const uu = u0 + t * (u1 - u0);
      const angle = Math.PI * uu * uu;
      sum +=
        half *
        w *
        (v0 + t * (v1 - v0)) *
        weight(Math.cos(angle)) *
        Math.sin(angle) *
        2 *
        Math.PI *
        uu;
    }
  }
  return 2 * Math.PI * sum;
}

/** The integral of a table channel over the sphere, as `phaseAt` reads it, sr⁻¹ × sr. */
export function phaseTableIntegral(table: PhaseTable, channel: 0 | 1 | 2): number {
  return tableIntegral(table.u, table.values[channel], () => 1);
}

/** The asymmetry of a table channel as read: ∫ p cos θ dΩ. */
export function phaseTableAsymmetry(table: PhaseTable, channel: 0 | 1 | 2): number {
  return tableIntegral(table.u, table.values[channel], (mu) => mu);
}

/**
 * Henyey–Greenstein's phase of asymmetry g at cosine μ, sr⁻¹: (1 − g²) ÷ (4π (1 + g² − 2gμ)^1.5).
 */
export function henyeyGreenstein(asymmetry: number, mu: number): number {
  const g = asymmetry;
  return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * mu) ** 1.5);
}

/**
 * The table entries an unresolved forward peak is laid on: a triangle in u over the first 4
 * intervals, θ < 0.044°, inside both phase files' first angle beyond 0° (0.05° for Yang et al.,
 * 0.08° for TAMUdust2020), so it adds nothing where the files have values.
 */
export const FORWARD_PEAK_ENTRIES = 4;

/** The forward peak's shape on {@link PHASE_TABLE_U}: 1 − k ÷ {@link FORWARD_PEAK_ENTRIES}, then 0. */
const FORWARD_PEAK_SHAPE: Float64Array = PHASE_TABLE_U.map((_, k) =>
  Math.max(0, 1 - k / FORWARD_PEAK_ENTRIES),
);

/** One channel's entries on {@link PHASE_TABLE_U}, before the integral's factor. */
interface ChannelEntries {
  /** a₁ ÷ 4π, or Henyey–Greenstein's phase. */
  readonly a1: Float64Array;
  /** a₂, a₃, a₄, b₁ and b₂ ÷ 4π, or none. */
  readonly matrix:
    readonly [Float64Array, Float64Array, Float64Array, Float64Array, Float64Array] | undefined;
  /** The matrix's unresolved forward peak's share of the scattering. */
  readonly forwardPeak: number;
}

/** One channel's {@link ChannelEntries}: the mode's matrix, or Henyey–Greenstein's from its g. */
function channelEntries(optics: ModeOptics): ChannelEntries {
  const { matrix } = optics;
  if (matrix === undefined) {
    return {
      a1: PHASE_TABLE_MU.map((mu) => henyeyGreenstein(optics.asymmetry, mu)),
      matrix: undefined,
      forwardPeak: 0,
    };
  }
  if (
    matrix.mu.length !== PHASE_TABLE_MU.length ||
    matrix.mu.some((mu, i) => mu !== PHASE_TABLE_MU[i])
  ) {
    throw new RangeError("a mode's matrix for a phase table must be given at PHASE_TABLE_MU");
  }
  const scale = 1 / (4 * Math.PI);
  const scaled = (a: Float64Array): Float64Array => a.map((v) => v * scale);
  return {
    a1: scaled(matrix.a1),
    matrix: [
      scaled(matrix.a2),
      scaled(matrix.a3),
      scaled(matrix.a4),
      scaled(matrix.b1),
      scaled(matrix.b2),
    ],
    forwardPeak: matrix.forwardPeak,
  };
}

/**
 * A mode's phase table from its optics in the three channels (red, green, blue, as
 * {@link CHANNEL_WAVELENGTHS_NM}), each normalised to integrate to 1 as read (the module's
 * remarks). The matrix is kept when every channel has one, as it does from one model; otherwise
 * the term is a total depolariser.
 *
 * @remarks
 * A channel whose matrix has a forward peak (a phase file's unresolved diffraction peak) has it
 * laid on the first {@link FORWARD_PEAK_ENTRIES} entries, with the identity's matrix, and sized
 * again on the table as read so that the table integrates to 1 and its mean cosine is the mode's
 * g ({@link forwardPeakSplit}); the other channels are only normalised.
 *
 * @throws RangeError for optics whose matrix is not at {@link PHASE_TABLE_MU}.
 */
export function aerosolPhaseTable(
  optics: readonly [ModeOptics, ModeOptics, ModeOptics],
): PhaseTable {
  const channels = optics.map(channelEntries);
  const peakIntegral = tableIntegral(PHASE_TABLE_U, FORWARD_PEAK_SHAPE, () => 1);
  const peakCosine = tableIntegral(PHASE_TABLE_U, FORWARD_PEAK_SHAPE, (mu) => mu) / peakIntegral;
  const values: Float64Array[] = [];
  const elements: Float64Array[][] = [[], [], [], [], []];
  for (const [c, entries] of channels.entries()) {
    const integral = tableIntegral(PHASE_TABLE_U, entries.a1, () => 1);
    const split =
      entries.forwardPeak > 0
        ? forwardPeakSplit(
            integral,
            tableIntegral(PHASE_TABLE_U, entries.a1, (mu) => mu),
            optics[c]?.asymmetry ?? Number.NaN,
            peakCosine,
          )
        : { scale: 1 / integral, forwardPeak: 0 };
    const factor = split.scale;
    const peak = split.forwardPeak / peakIntegral;
    const withPeak = (v: number, k: number): number =>
      v * factor + peak * (FORWARD_PEAK_SHAPE[k] ?? 0);
    values.push(entries.a1.map(withPeak));
    for (const e of ELEMENT_INDICES) {
      const element = entries.matrix?.[e];
      // The delta's matrix is the identity's: a₂, a₃ and a₄ take it, b₁ and b₂ do not.
      const diagonal = e === 0 || e === 1 || e === 2;
      elements[e]?.push(
        element === undefined
          ? new Float64Array(0)
          : element.map((v, k) => (diagonal ? withPeak(v, k) : v * factor)),
      );
    }
  }
  const [red, green, blue] = values;
  if (red === undefined || green === undefined || blue === undefined) {
    throw new Error("a phase table needs three channels");
  }
  const kept = channels.every((c) => c.matrix !== undefined);
  const element = (e: number): readonly [Float64Array, Float64Array, Float64Array] => {
    const [r, g, b] = elements[e] ?? [];
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table's matrix needs three channels");
    }
    return [r, g, b];
  };
  const matrix: PhaseMatrixTable | undefined = kept
    ? { a2: element(0), a3: element(1), a4: element(2), b1: element(3), b2: element(4) }
    : undefined;
  return phaseTable(PHASE_TABLE_U, [red, green, blue], matrix);
}

/** The streams the bakes' delta-M truncation assumes by default: 2M = 32 moments kept (R08.T14.b sets its own). */
export const DELTA_M_STREAMS = 16;

/** A channel's delta-M series: the truncated fraction and the moments kept. */
export interface DeltaMSeries {
  /** f = χ_2M, the forward peak's fraction moved into the direct beam. */
  readonly fraction: number;
  /** χ′_l = (χ_l − f) ÷ (1 − f) for l = 0 … 2M − 1; χ′₀ = 1 when the table integrates to 1. */
  readonly moments: Float64Array;
  /** The table's own χ_l, l = 0 … 2M. */
  readonly fullMoments: Float64Array;
}

/**
 * A table channel's Legendre moments χ_l = ∫ p Pₗ(cos θ) dΩ and their delta-M truncation for
 * 2M streams (Wiscombe 1977), for the multiple-scattering bakes only (Design note 6).
 *
 * @param streams - M, the streams in a hemisphere, an integer ≥ 1.
 * @throws RangeError for a bad stream count.
 */
export function deltaMTruncation(
  table: PhaseTable,
  channel: 0 | 1 | 2,
  streams: number = DELTA_M_STREAMS,
): DeltaMSeries {
  if (!(Number.isInteger(streams) && streams >= 1)) {
    throw new RangeError(`delta-M needs an integer stream count ≥ 1, not ${streams}`);
  }
  const order = 2 * streams;
  const full = new Float64Array(order + 1);
  for (let l = 0; l <= order; l += 1) {
    full[l] = tableIntegral(table.u, table.values[channel], (mu) => legendre(l, mu));
  }
  const fraction = full[order] ?? Number.NaN;
  const moments = Float64Array.from(
    { length: order },
    (_, l) => ((full[l] ?? Number.NaN) - fraction) / (1 - fraction),
  );
  return { fraction, moments, fullMoments: full };
}

/** Pₗ(μ) by the three-term recurrence. */
function legendre(l: number, mu: number): number {
  if (l === 0) {
    return 1;
  }
  let p0 = 1;
  let p1 = mu;
  for (let n = 1; n < l; n += 1) {
    const p2 = ((2 * n + 1) * mu * p1 - n * p0) / (n + 1);
    p0 = p1;
    p1 = p2;
  }
  return p1;
}

// ---------------------------------------------------------------------------------------------
// The term.

/** An aerosol mode as the inventory gives it (P14.T24.c): its particles, column and profile. */
export interface AerosolModeSpec {
  /** The term's name, unique within its medium. */
  readonly name: string;
  readonly particles: AerosolParticles;
  /** The column's vertical optical depth at 550 nm, > 0. */
  readonly opticalDepth550: number;
  /** The layer's base and top pressures, Pa, base > top > 0. */
  readonly basePa: number;
  readonly topPa: number;
  /** The mixing ratio's exponent f in (p ÷ p_base)^f: the deck's f_sed, or 0 for a well-mixed layer. */
  readonly mixingExponent: number;
}

/** An aerosol term and the optics it was built from. */
export interface AerosolTerm {
  readonly term: MediumTerm;
  /** The mode's optics in the three channels, red, green, blue. */
  readonly channels: readonly [ModeOptics, ModeOptics, ModeOptics];
  /** At 550 nm, which sets the column's scale. */
  readonly at550: ModeOptics;
}

/** ln p at a level of the column. */
function lnPressure(column: AtmosphereColumn, i: number): number {
  return Math.log(column.pressuresPa[i] ?? Number.NaN);
}

/** A column quantity at pressure p, linear in ln p between the levels that bracket it. */
function atPressure(column: AtmosphereColumn, values: Float64Array, pressurePa: number): number {
  const target = Math.log(pressurePa);
  const levels = column.pressuresPa.length;
  for (let i = 0; i + 1 < levels; i += 1) {
    const a = lnPressure(column, i);
    const b = lnPressure(column, i + 1);
    if (target <= a && target >= b) {
      const t = (target - a) / (b - a);
      return (
        (values[i] ?? Number.NaN) + t * ((values[i + 1] ?? Number.NaN) - (values[i] ?? Number.NaN))
      );
    }
  }
  throw new RangeError(`pressure ${pressurePa} Pa lies outside the column`);
}

/**
 * A mode as a medium term on its column (the module's remarks): its density between its base and
 * top, its coefficients per channel at relative density 1 so that the column's τ(550) is the
 * mode's, and its tabulated phase.
 *
 * @throws RangeError for a non-positive or non-finite optical depth, a profile whose base is not
 *   above its top or that lies outside the column, or as {@link aerosolModeOptics}.
 */
export function aerosolTerm(spec: AerosolModeSpec, column: AtmosphereColumn): AerosolTerm {
  const { opticalDepth550: tau, mixingExponent: f } = spec;
  if (!(tau > 0 && Number.isFinite(tau))) {
    throw new RangeError(`a mode's optical depth ${tau} must be finite and > 0`);
  }
  if (!(spec.topPa > 0 && spec.basePa > spec.topPa && Number.isFinite(spec.basePa))) {
    throw new RangeError(`a mode's base ${spec.basePa} Pa must lie below its top ${spec.topPa} Pa`);
  }
  if (!Number.isFinite(f)) {
    throw new RangeError(`a mode's mixing exponent ${f} must be finite`);
  }
  const surfacePa = column.pressuresPa[0] ?? Number.NaN;
  const columnTopPa = column.pressuresPa.at(-1) ?? Number.NaN;
  const basePa = Math.min(spec.basePa, surfacePa);
  const topPa = Math.max(spec.topPa, columnTopPa);
  if (!(basePa > topPa)) {
    throw new RangeError(`the mode ${spec.name} lies outside its column`);
  }
  const lnDensity = column.density.relative.map(Math.log);
  const relativeAt = (p: number): number =>
    (p / spec.basePa) ** f * Math.exp(atPressure(column, lnDensity, p));
  const heights: number[] = [];
  const values: number[] = [];
  const levels = column.altitudesM.length;
  const hBase = atPressure(column, column.altitudesM, basePa);
  const hTop = atPressure(column, column.altitudesM, topPa);
  // The column level below the base and above the top hold 0, so the layer ends within a level.
  for (let i = 0; i < levels; i += 1) {
    const h = column.altitudesM[i] ?? Number.NaN;
    if (h < hBase && (column.altitudesM[i + 1] ?? Infinity) >= hBase) {
      heights.push(h);
      values.push(0);
    }
  }
  heights.push(hBase);
  values.push(relativeAt(basePa));
  for (let i = 0; i < levels; i += 1) {
    const h = column.altitudesM[i] ?? Number.NaN;
    if (h > hBase && h < hTop) {
      heights.push(h);
      values.push(relativeAt(column.pressuresPa[i] ?? Number.NaN));
    }
  }
  heights.push(hTop);
  values.push(relativeAt(topPa));
  const above = column.altitudesM.find((h) => h > hTop);
  if (above !== undefined) {
    heights.push(above);
    values.push(0);
  }
  const peak = Math.max(...values);
  const density = tabulatedDensity(
    Float64Array.from(heights),
    Float64Array.from(values, (v) => v / peak),
  );
  const lengthM = columnLengthM(density, column.topHeightM);
  const [red, green, blue] = CHANNEL_WAVELENGTHS_NM;
  const channels: readonly [ModeOptics, ModeOptics, ModeOptics] = [
    aerosolModeOptics(spec.particles, red),
    aerosolModeOptics(spec.particles, green),
    aerosolModeOptics(spec.particles, blue),
  ];
  const at550 =
    channels.find((c) => c.wavelengthNm === 550) ?? aerosolModeOptics(spec.particles, 550);
  const extinction550 = tau / lengthM;
  const scale = (c: ModeOptics): number =>
    (extinction550 * c.extinctionCrossSectionM2) / at550.extinctionCrossSectionM2;
  const scattering: Rgb = [
    scale(channels[0]) * channels[0].singleScatteringAlbedo,
    scale(channels[1]) * channels[1].singleScatteringAlbedo,
    scale(channels[2]) * channels[2].singleScatteringAlbedo,
  ];
  const absorption: Rgb = [
    scale(channels[0]) - scattering[0],
    scale(channels[1]) - scattering[1],
    scale(channels[2]) - scattering[2],
  ];
  return {
    term: {
      name: spec.name,
      density,
      scattering,
      absorption,
      phase: { kind: "tabulated", table: aerosolPhaseTable(channels) },
    },
    channels,
    at550,
  };
}

// ---------------------------------------------------------------------------------------------
// The label.

/**
 * Whether a validated model with known residuals is labelled `ATMOSPHERE: APPROXIMATE`:
 * `unlabelled`, the ruling's lean (science-r08-nonspherical.md §5 item 1), as Mie is not for
 * spheres, with the hexahedra's 20–37% at side and back angles recorded in R08's Risks; or
 * `labelled`. Pending the sign-off agent.
 */
export const MODEL_RESIDUALS_LABEL: "unlabelled" | "labelled" = "unlabelled";

/** The models with known residuals that {@link MODEL_RESIDUALS_LABEL} would label: TAMUdust2020's hexahedra. */
export const MODELS_WITH_KNOWN_RESIDUALS: ReadonlySet<PhaseModel> = new Set(["tamudust2020"]);

/**
 * Whether an ice analogue whose estimated |Δg| is at most {@link SMALL_BIAS_MAX_ABS_DG} is
 * labelled: `labelled`, the ruling's lean, every analogue labelled, CH₄ ice's −0.007 included
 * (science-r08-nonspherical.md §5 item 2); or `exempt`, the alternative. Pending the sign-off agent.
 */
export const SMALL_BIAS_ANALOGUES: "labelled" | "exempt" = "labelled";

/** The ruling's alternative's threshold for {@link SMALL_BIAS_ANALOGUES}: |Δg| ≤ 0.01. */
export const SMALL_BIAS_MAX_ABS_DG = 0.01;

/**
 * A mode as the phase label sees it: its material, its optics in every channel it is drawn in (an
 * {@link AerosolTerm}'s `channels`), and the optical depth above it.
 *
 * @remarks
 * Every drawn channel counts, since a mode's basis may change with wavelength: the Titan-like haze
 * is an MMF `model` at 550 nm and an `analogue` below 453 nm, its blue channel included
 * (science-r08-mmf.md §1.4).
 */
export interface AerosolLayer {
  readonly material: string;
  readonly optics: ReadonlyArray<ModeOptics>;
  /** The vertical optical depth at 550 nm of everything above the mode's top, ≥ 0. */
  readonly opticalDepthAbove550: number;
}

/** The order of the bases from the most validated to the least. */
const BASIS_RANK: Readonly<Record<PhaseBasis, number>> = { model: 0, analogue: 1, fallback: 2 };

/**
 * The least validated of a mode's bases over its channels: `fallback` if any channel is one,
 * else `analogue` if any is, else `model`.
 */
export function leastValidatedBasis(optics: ReadonlyArray<ModeOptics>): PhaseBasis {
  let basis: PhaseBasis = "model";
  for (const channel of optics) {
    if (BASIS_RANK[channel.phaseBasis] > BASIS_RANK[basis]) {
      basis = channel.phaseBasis;
    }
  }
  return basis;
}

/** A mode drawn on an analogue's or a fallback's phase where it can be seen, and why. */
export interface PhaseApproximation {
  readonly material: string;
  readonly reason: string;
}

/** Whether a model with known residuals is labelled, by {@link MODEL_RESIDUALS_LABEL}. */
function modelLabelled(model: PhaseModel): boolean {
  let label: boolean;
  switch (MODEL_RESIDUALS_LABEL) {
    case "unlabelled":
      label = false;
      break;
    case "labelled":
      label = MODELS_WITH_KNOWN_RESIDUALS.has(model);
      break;
  }
  return label;
}

/** Whether an analogue of a material is labelled, by {@link SMALL_BIAS_ANALOGUES}. */
function analogueLabelled(material: string): boolean {
  const bias = ICE_ANALOGUE_ASYMMETRY_BIAS[material];
  let label: boolean;
  switch (SMALL_BIAS_ANALOGUES) {
    case "labelled":
      label = true;
      break;
    case "exempt":
      label = bias === undefined || Math.abs(bias) > SMALL_BIAS_MAX_ABS_DG;
      break;
  }
  return label;
}

/** Whether one channel's phase basis is labelled, by the two pending constants. */
function labelled(material: string, optics: ModeOptics): boolean {
  let label: boolean;
  switch (optics.phaseBasis) {
    case "model":
      label = modelLabelled(optics.phaseModel);
      break;
    case "analogue":
      label = analogueLabelled(material);
      break;
    case "fallback":
      label = true;
      break;
  }
  return label;
}

/**
 * The modes drawn with an analogue's or a fallback's phase under less than
 * {@link CLOUD_DECK_SPLIT_OPTICAL_DEPTH} at 550 nm: the reasons R08.T10.a's `atmosphereApproximate`
 * collects (Design note 12). A deeper mode does not reach the picture.
 *
 * @throws RangeError for an optical depth that is not finite and ≥ 0.
 */
export function phaseApproximations(
  layers: Iterable<AerosolLayer>,
): ReadonlyArray<PhaseApproximation> {
  const out: PhaseApproximation[] = [];
  for (const layer of layers) {
    const depth = layer.opticalDepthAbove550;
    if (!(depth >= 0 && Number.isFinite(depth))) {
      throw new RangeError(`optical depth ${depth} above a mode must be finite and ≥ 0`);
    }
    const channel = layer.optics.find((optics) => labelled(layer.material, optics));
    if (depth < CLOUD_DECK_SPLIT_OPTICAL_DEPTH && channel !== undefined) {
      out.push({
        material: layer.material,
        reason: `${channel.phaseNote ?? `${layer.material}: a ${channel.phaseBasis} phase`} (at ${channel.wavelengthNm} nm)`,
      });
    }
  }
  return out;
}

/**
 * The labels a body's modes give by their phase bases: `atmosphereApproximate` for a visible
 * analogue or fallback.
 *
 * @throws RangeError for an optical depth that is not finite and ≥ 0.
 */
export function phaseLabels(layers: Iterable<AerosolLayer>): ReadonlyArray<AtmosphereLabel> {
  return phaseApproximations(layers).length > 0 ? ["atmosphereApproximate"] : [];
}

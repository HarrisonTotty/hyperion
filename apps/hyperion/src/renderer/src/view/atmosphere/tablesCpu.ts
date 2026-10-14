/**
 * The CPU twin of the atmosphere's kernels in `f64` (plan R08, R08.T6.a): Hillaire's per-planet
 * transmittance and multiple-scattering tables, the per-frame sky view and aerial perspective, and
 * the per-pixel ray march, over a medium of any number of terms, as R05's kernels compute them.
 *
 * @remarks
 * Each twin follows its kernel's WGSL step for step, at the kernel's sizes and step counts unless
 * a caller asks for others: the same parameterisations, the same midpoints and placed steps
 * (`marchSteps.ts`, R05.T12.e, cut at the medium's layer edges since R08.T6.i), the same bilinear reads of the tables with their texel centres at
 * (i + ½) ÷ size, through the same texel mapping (Bruneton 2017's exact pair, plan R08, Design
 * note 19, since R08.T6.g), and the same in-scattering per step, throughput · S · Δt · g(σ_t Δt) with
 * g(x) = (1 − e^(−x)) ÷ x. It differs only in arithmetic: `f64` throughout, g taken exactly where
 * `common.wgsl`'s `stepFactor` switches to a series for `f32`, and the tables kept in `f64` where
 * the GPU stores `rgba16float`. So it is the GPU's oracle (R08.T6.b's readbacks agree with it to
 * 10⁻³) and the quadrature gate's subject (R05.T12.e's rays, with the multiple-scattering term).
 *
 * The medium is data: a list of terms, each a density profile (`exponential`, `tent` or
 * `tabulated`), scattering and absorption per channel at unit density, and a phase function
 * (`rayleigh`, `cornette-shanks`, `none` or `tabulated`, per channel). The twin names no species
 * and holds no limit on their number; the kernels' packer refuses more than `MAX_TERMS`
 * (`tables.ts`, R08.T6.b). It refuses one medium that no builder should make, as the packer does
 * (`medium.ts`'s `checkTabulatedTops`): a tabulated term whose last level lies below the medium's
 * top at a density other than 0, which the `tabulated` rule would hold up to the top, as R08.T3.a's
 * column holds its last level's 10⁻⁷ of the ground's; a medium built on a column takes a top no
 * higher than the column's.
 *
 * An absorber term (R08.T4.b) absorbs by its curves of growth, one a light (plan R08, Design notes
 * 5 and 8, R08.T6.c). The transmittance twin stores, as the kernel does, the optical depth of every
 * other term in rgb and the absorbers' columns u ÷ U, slot 0 in its alpha and slots 1 to 4 in the
 * absorber-column table; every read adds light slot 0's curves to the optical depth and
 * exponentiates, and every march's own path takes its absorbers by the path's column under the same
 * curves, the step's share being how much it deepens the whole path's.
 *
 * The kernels read a tabulated density and phase from tables resampled onto their own grids
 * (`tables.ts`'s `kernelMedium`), so the twin is their oracle when it is given the medium as they
 * read it: `planetTwin(kernelMedium(medium), …)` and the twins built on it (R08.T6.b's smoke).
 *
 * The multiple-scattering kernel takes R05.T12.e's step factor from `common.wgsl` since R08.T6.a,
 * in place of (S − S e^(−x)) ÷ max(σ_t, 10⁻¹²), whose `f32` cancellation the per-frame kernels
 * shed in R05.T12.e. In `f64` the two are the same sum, so this twin is unchanged by it; the
 * change's size on the GPU's table, measured by emulating the two factors in `f32`
 * ({@link MultiScatteringOptions.stepFactor}), is in plan R08's Risks.
 *
 * Oblate bodies (plan R08, Design note 17; R08.T6.e) have a twin of their own, ahead of the kernels
 * (R08.T6.f): {@link oblatePlanetTwin} builds a body's transmittance slices over κ = s R_α ÷ R_ref,
 * each the reference medium on the sphere κ R_ref, and its multiple-scattering bands over
 * s = g(φ) ÷ g_ref, each an ordinary build at its latitude's g(φ) and √(MN) ({@link twinBandOf}),
 * all with Design note 19's texel mapping. The oblate marches ({@link oblateSkyViewTwin},
 * {@link oblateAerialPerspectiveTwin}, {@link oblateRayMarchTwin}) read the medium at each
 * sample's h\* = s h, s at its own latitude; the sky view and aerial perspective march each ray on
 * its own osculating sphere R_α at the camera, their layer edges at h\* over the camera's s, and the
 * ray march keeps R05's spheroid shells through h\* at the equator and the poles. Each source's
 * transmittance is read through the slices on the sphere its own ray fits (`OblateReader`, Design
 * note 17's _The ray's own gradient_), its optical depth divided by s, and the multiple scattering
 * is the bands' about the sample's s. {@link spheroidOpticalDepth} is the true spheroid's brute
 * force that the slicing is held against. The spherical twin above stays the kernels' oracle until
 * R08.T6.f, and a sphere with ω = 0 gives its tables to 10⁻¹².
 *
 * Not twinned: the composite, a full-screen material whose only read of the medium is the
 * transmittance table along the view (`composite.wgsl`'s `transmittanceToSpace`), and the GPU's
 * `rgba16float` rounding.
 *
 * Built on `opticalDepth.ts` (the shell's geometry and the transmittance table's mapping: Bruneton
 * 2017's `GetTransmittanceTextureUvFromRMu` in precomputed_atmospheric_scattering's
 * `functions.glsl`, whose x_r = ρ ÷ H is Bruneton and Neyret 2008's, §4, without its sub-texel
 * remap, as sebh and Bevy take it) and `marchSteps.ts` (the steps' placement), which it extends
 * rather than replaces. The sources are the kernels' own: Bevy 0.19.1's atmosphere (MIT or Apache-2.0),
 * Bruneton's precomputed_atmospheric_scattering (BSD-3-Clause) and sebh's UnrealEngineSkyAtmosphere
 * (MIT), whose notices are in `shaders/common.wgsl` and `shaders/multiScattering.wgsl`.
 */

import { scale, type Vec3 } from "../../geometry/vec3";
import { rotate } from "../camera/quaternion";
import type { ViewSize } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import { HALF_MAX } from "../sky/half";
import {
  type AtmosphereCamera,
  atmosphereInputs,
  curveSlotOf,
  FIT_SCALE_FLOOR_M,
  frameSourcesOf,
  type Geodetic,
  geodeticOf,
  skyAxisOf,
  skyViewHeight,
  type SpheroidFigure,
  type SunState,
  tableRadiusM,
  type TableSizes,
  type VolumeSize,
} from "./hillaire";
import {
  ABSORBER_CURVE_DECADES,
  ABSORBER_CURVE_NODES_PER_DECADE,
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberCurve,
} from "./absorbers";
import { type ForwardLobe, forwardLobeAt, forwardLobeOfTable, lobeIsDrawn } from "./forwardLobe";
import {
  marchEdgesM,
  type MarchPieces,
  marchPieces,
  marchPiecesSteps,
  marchSplit,
  marchStep,
  type MarchStep,
} from "./marchSteps";
import {
  ABSORBER_PRESSURE_FLOOR_PA,
  ABSORBER_PRESSURE_REFERENCE_PA,
  type AtmosphereMedium,
  checkTabulatedTops,
  densityLookup,
  extinction,
  layerEdgesM,
  type PhaseFunction,
  phaseAt,
} from "./medium";
import {
  bandLatitudeRad,
  type BodyGravity,
  gravityRatio,
  gravityRatioSlope,
  principalRadiiM,
} from "./oblate";
import {
  distanceToTopM,
  intersectsGround,
  localRadiusM,
  maxDistanceM,
  type Shell,
  texelUvFromUnit,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
  unitFromTexelUv,
} from "./opticalDepth";
import {
  IRRADIANCE_ARC_NODES,
  IRRADIANCE_RING_NODES,
  irradianceDirections,
} from "./irradianceQuadrature";
import type { SurfaceLightingReader } from "./surfaceLighting";
import {
  ABSORBER_SLOTS,
  type AbsorberCurveSlots,
  absorberSlots,
  checkAbsorberCurves,
  EDGES_PER_TERM,
  IRRADIANCE_SAMPLES,
  IRRADIANCE_SIZE,
  lineSlotMask,
  OBLATE_SCALE_HEIGHTS,
  packedScaleHeightsM,
  MULTI_SCATTERING_SAMPLES,
  MULTI_SCATTERING_SIZE,
  multiScatteringLayersOf,
  NO_ABSORBER_CURVES,
  PRESSURE_CHANNELS_A_TABLE,
  pressureChannelCount,
  pressureChannelOf,
  resampledPhase,
  SHARED_TABLE_CURVE_SLOT,
  type TableSize,
  TRANSMITTANCE_SAMPLES,
  TRANSMITTANCE_SIZE,
  TRANSMITTANCE_STEPS_A_PIECE,
} from "./tables";

/** A radiance or transmittance per channel that a twin writes into. */
export type TwinRgb = [number, number, number];

const CHANNELS = [0, 1, 2] as const;

/**
 * A kernel's output in `f64`: RGBA per texel, row by row from texel (0, 0), the layout of a
 * readback, texel (x, y) at (y × width + x) × 4.
 */
export interface TwinTable {
  readonly widthTexels: number;
  readonly heightTexels: number;
  readonly texels: Float64Array;
}

/** A volume's output in `f64`: RGBA per texel, texel (x, y, z) at ((z × height + y) × width + x) × 4. */
export interface TwinVolume extends TwinTable {
  readonly slices: number;
}

/**
 * One term as the twin reads it: its profile, its coefficients at unit density, its phase, and, for
 * an absorber, its slot and n ÷ U (`tables.ts`'s `absorberSlots`, R08.T6.c).
 */
interface TwinTerm {
  /** `medium.ts`'s `densityLookup` of its density: its value at a height, found from the last. */
  readonly densityOf: (heightM: number) => number;
  readonly scattering: Rgb;
  readonly absorption: Rgb;
  readonly extinction: Rgb;
  /** Its layer edges, m (`medium.ts`'s `layerEdgesM`), where the transmittance march is split. */
  readonly edgesM: ReadonlyArray<number>;
  readonly phase: PhaseFunction;
  /** Its absorber slot, 0 to `ABSORBER_SLOTS` − 1, or −1 for a term that is not an absorber. */
  readonly slot: number;
  /** n ÷ U, m⁻¹, for an absorber; 0 otherwise. */
  readonly columnPerM: number;
  /**
   * Its slot's pressure channel (`tables.ts`'s `pressureChannelOf`; R08.T4.d), or −1 for a term
   * that is not a line slot.
   */
  readonly pressureChannel: number;
  /**
   * A tabulated term's forward lobe (`forwardLobe.ts`, R08.T7), of its table on the kernels' grid;
   * null for every other term.
   */
  readonly lobe: ForwardLobe | null;
}

/**
 * The medium's terms with their extinctions and absorber slots, checked.
 *
 * @throws RangeError for a tabulated density whose last level lies below the medium's top with a
 *   density other than 0: it would hold that density up to the top (the `tabulated` rule), as a
 *   column does with its last level's 10⁻⁷ of the ground's (R08.T3.a), so a medium built on it
 *   takes a top no higher than its last level; and, as `absorberSlots`, for more absorber terms
 *   than the tables hold or one with no column.
 */
function termsOf(medium: AtmosphereMedium): readonly TwinTerm[] {
  checkTabulatedTops(medium);
  const slots = new Map(absorberSlots(medium).map((slot) => [slot.termIndex, slot]));
  const lines = lineSlotMask(medium);
  return medium.terms.map((term, i) => {
    const slot = slots.get(i)?.slot ?? -1;
    return {
      densityOf: densityLookup(term.density),
      scattering: term.scattering,
      absorption: term.absorption,
      extinction: extinction(term),
      edgesM: layerEdgesM(term.density),
      phase: term.phase,
      slot,
      columnPerM: slots.get(i)?.columnPerM ?? 0,
      pressureChannel: slot < 0 ? -1 : pressureChannelOf(lines, slot),
      lobe: lobeOfPhase(term.phase),
    };
  });
}

/**
 * Terms with every coefficient per metre, and every absorber column per metre, times `factor`: a
 * latitude band's medium in its own gravity-scaled frame (plan R08, Design note 17; R08.T6.e). A
 * band at s is the reference medium read at h\* = s h on lengths l, which is the same air on lengths
 * l\* = s l with every coefficient divided by s; so its tables are an ordinary build on the sphere
 * of radius s √(MN), its rows at h\*, with `factor` 1 ÷ s. The lobes' shares are ratios and stay.
 */
function scaledTerms(terms: readonly TwinTerm[], factor: number): readonly TwinTerm[] {
  if (factor === 1) {
    return terms;
  }
  const times = (rgb: Rgb): Rgb => [rgb[0] * factor, rgb[1] * factor, rgb[2] * factor];
  return terms.map((term) => ({
    ...term,
    scattering: times(term.scattering),
    absorption: times(term.absorption),
    extinction: times(term.extinction),
    columnPerM: term.columnPerM * factor,
  }));
}

/**
 * A phase's forward lobe as the kernels draw it (`forwardLobe.ts`, R08.T7): a tabulated phase's, of
 * its table on the kernels' grid, where it is drawn (`lobeIsDrawn`); null for every other phase.
 */
function lobeOfPhase(phase: PhaseFunction): ForwardLobe | null {
  if (phase.kind !== "tabulated") {
    return null;
  }
  const lobe = forwardLobeOfTable(resampledPhase(phase.table));
  return lobeIsDrawn(lobe) ? lobe : null;
}

/**
 * The medium's pressure over `tables.ts`'s `ABSORBER_PRESSURE_REFERENCE_PA` at a height above the
 * ground, as `medium.wgsl`'s `pressureBarAt` reads it (R08.T4.d), or null for a medium without a
 * pressure profile, whose absorbers' curves have no pressure rows.
 */
function pressureBarLookup(medium: AtmosphereMedium): ((heightM: number) => number) | null {
  return medium.pressureBar === undefined ? null : densityLookup(medium.pressureBar);
}

/** Where a sample's absorber columns per metre stand in {@link sampleInto}'s output: 9 on. */
const COLUMNS_AT = 9;

/** Where a sample's absorption stands in {@link sampleInto}'s output, past the columns: 14 on. */
const ABSORPTION_AT = COLUMNS_AT + ABSORBER_SLOTS;

/**
 * Where a sample's pressure-weighted columns per metre stand in {@link sampleInto}'s output, past
 * the absorption, by pressure channel: 17 on (R08.T4.d).
 */
const PRESSURE_COLUMNS_AT = ABSORPTION_AT + 3;

/**
 * The floats of one sample: three triples, the absorber slots' columns, the absorption and the
 * slots' pressure-weighted columns.
 */
/**
 * Where a sample's in-lobe scattering stands in {@link sampleInto}'s output, past the pressure
 * columns: Σ P σ_s of its tabulated terms (R08.T7; `medium.wgsl`'s `lobeScattering`).
 */
const LOBE_AT = PRESSURE_COLUMNS_AT + ABSORBER_SLOTS;

const SAMPLE_FLOATS = LOBE_AT + 3;

/**
 * The medium at a height into `out`, {@link SAMPLE_FLOATS} long: the scattering in 0–2 and the
 * extinction in 3–5, m⁻¹; given each term's phase per channel for the ray, the scattering towards
 * the eye in 6–8, m⁻¹ sr⁻¹; each absorber slot's column per metre, its n ÷ U times its density,
 * in 9–13; and the absorption in 14–16, m⁻¹, the terms' own coefficients summed, never the
 * extinction less the scattering (`medium.wgsl`'s `mediumAtHeight` and `source.wgsl`'s
 * `sampleMediumAt`; R08.T6.d); and each line slot's column per metre times p ÷ 1 bar there, from
 * `pressureBarOf`, in its pressure channel at 17–21, 0 elsewhere and for a medium without a pressure
 * profile (R08.T4.d; `common.wgsl`'s `pressureColumnsOf`). Given `densities`, each term's density
 * there, term k's at k, for each source's own phases ({@link phasedInto}, R08.T7).
 *
 * @remarks
 * The twins' innermost call, a few million times a table: written with indexed loops and its sums
 * in locals, each summed term by term in the terms' order as before, so that its values are the
 * same to the bit (R08.T6.d, for the smoke's watchdog).
 */
function sampleInto(
  terms: readonly TwinTerm[],
  pressureBarOf: ((heightM: number) => number) | null,
  phases: Float64Array | null,
  heightM: number,
  out: Float64Array,
  densities?: Float64Array,
): void {
  out.fill(0, COLUMNS_AT, ABSORPTION_AT);
  let s0 = 0;
  let s1 = 0;
  let s2 = 0;
  let e0 = 0;
  let e1 = 0;
  let e2 = 0;
  let p0 = 0;
  let p1 = 0;
  let p2 = 0;
  let a0 = 0;
  let a1 = 0;
  let a2 = 0;
  let l0 = 0;
  let l1 = 0;
  let l2 = 0;
  for (let k = 0; k < terms.length; k += 1) {
    const term = terms[k];
    if (term === undefined) {
      continue;
    }
    const d = term.densityOf(heightM);
    if (densities !== undefined) {
      densities[k] = d;
    }
    const { scattering, extinction: sigmaT, absorption } = term;
    const r = scattering[0] * d;
    const g = scattering[1] * d;
    const b = scattering[2] * d;
    s0 += r;
    s1 += g;
    s2 += b;
    e0 += sigmaT[0] * d;
    e1 += sigmaT[1] * d;
    e2 += sigmaT[2] * d;
    a0 += absorption[0] * d;
    a1 += absorption[1] * d;
    a2 += absorption[2] * d;
    const lobe = term.lobe;
    if (lobe !== null) {
      l0 += lobe.share[0] * r;
      l1 += lobe.share[1] * g;
      l2 += lobe.share[2] * b;
    }
    if (phases !== null) {
      p0 += r * (phases[k * 3] ?? Number.NaN);
      p1 += g * (phases[k * 3 + 1] ?? Number.NaN);
      p2 += b * (phases[k * 3 + 2] ?? Number.NaN);
    }
    if (term.slot >= 0) {
      out[COLUMNS_AT + term.slot] = (out[COLUMNS_AT + term.slot] ?? 0) + term.columnPerM * d;
    }
  }
  out[0] = s0;
  out[1] = s1;
  out[2] = s2;
  out[3] = e0;
  out[4] = e1;
  out[5] = e2;
  out[6] = p0;
  out[7] = p1;
  out[8] = p2;
  out[ABSORPTION_AT] = a0;
  out[ABSORPTION_AT + 1] = a1;
  out[ABSORPTION_AT + 2] = a2;
  const ratio = pressureBarOf === null ? 0 : pressureBarOf(heightM);
  out.fill(0, PRESSURE_COLUMNS_AT, SAMPLE_FLOATS);
  out[LOBE_AT] = l0;
  out[LOBE_AT + 1] = l1;
  out[LOBE_AT + 2] = l2;
  for (let k = 0; k < terms.length; k += 1) {
    const term = terms[k];
    if (term !== undefined && term.pressureChannel >= 0) {
      out[PRESSURE_COLUMNS_AT + term.pressureChannel] = (out[COLUMNS_AT + term.slot] ?? 0) * ratio;
    }
  }
}

/** Whether any of a medium's terms is an absorber. */
function hasAbsorbers(terms: readonly TwinTerm[]): boolean {
  return terms.some((term) => term.slot >= 0);
}

/** Each term's phase per channel for one cosine, term k's at 3k–3k + 2. */
function phasesAt(terms: readonly TwinTerm[], cosTheta: number): Float64Array {
  const phases = new Float64Array(terms.length * 3);
  for (const [k, term] of terms.entries()) {
    phases.set(phaseAt(term.phase, cosTheta), k * 3);
  }
  return phases;
}

/** A value clamped to [0, 1]. */
function unitClamp(x: number): number {
  return Math.min(Math.max(x, 0), 1);
}

/** Texel i's unit range on an axis of n texels, Bruneton's mapping clamped, as the kernels write. */
function unitOfTexel(i: number, n: number): number {
  return unitClamp(unitFromTexelUv((i + 0.5) / n, n));
}

/**
 * A table's bilinear read at a unit range (u, v), each clamped to [0, 1] and taken through
 * Bruneton's texel mapping: `common.wgsl`'s `tableAtUnit`.
 */
function tableAtUnitInto(table: TwinTable, u: number, v: number, out: TwinRgb): void {
  bilinearInto(
    table,
    texelUvFromUnit(unitClamp(u), table.widthTexels),
    texelUvFromUnit(unitClamp(v), table.heightTexels),
    out,
  );
}

/** Bilinear read of a table's RGB, texel centres at (i + ½) ÷ size, clamped: `common.wgsl`'s. */
function bilinearInto(table: TwinTable, u: number, v: number, out: TwinRgb): void {
  bilinearChannelsInto(table, u, v, CHANNELS, out);
}

/** The four channels of a texel, RGBA. */
const RGBA = [0, 1, 2, 3] as const;

/** Bilinear read of the given channels of a table into `out` at their indices, as `bilinear`. */
function bilinearChannelsInto(
  table: TwinTable,
  u: number,
  v: number,
  channels: ReadonlyArray<number>,
  out: number[] | Float64Array,
): void {
  const w = table.widthTexels;
  const h = table.heightTexels;
  const px = Math.min(Math.max(u * w - 0.5, 0), w - 1);
  const py = Math.min(Math.max(v * h - 0.5, 0), h - 1);
  const x0 = Math.floor(px);
  const y0 = Math.floor(py);
  const x1 = Math.min(x0 + 1, w - 1);
  const y1 = Math.min(y0 + 1, h - 1);
  const fx = px - x0;
  const fy = py - y0;
  const texels = table.texels;
  const i00 = (y0 * w + x0) * 4;
  const i10 = (y0 * w + x1) * 4;
  const i01 = (y1 * w + x0) * 4;
  const i11 = (y1 * w + x1) * 4;
  for (let j = 0; j < channels.length; j += 1) {
    const c = channels[j] ?? 0;
    const a = (texels[i00 + c] ?? Number.NaN) * (1 - fx) + (texels[i10 + c] ?? Number.NaN) * fx;
    const b = (texels[i01 + c] ?? Number.NaN) * (1 - fx) + (texels[i11 + c] ?? Number.NaN) * fx;
    out[c] = a * (1 - fy) + b * fy;
  }
}

/**
 * The step factor g(x) = (1 − e^(−x)) ÷ x, x a step's optical depth, exact in `f64`, and 1 at 0:
 * `common.wgsl`'s `stepFactor` without the series it takes below 0.01 for `f32`.
 */
export function exactStepFactor(x: number): number {
  return x > 0 ? -Math.expm1(-x) / x : 1;
}

// --- Transmittance -------------------------------------------------------------------------------

/**
 * Where the line of a ray from `heightM` above a sphere of radius `bottomRadiusM`, at zenith cosine
 * `mu`, crosses the sphere `edgeM` above it: its two signed distances, m, nearer first (a caller
 * keeps those ahead of the ray), or `null` where the line misses that sphere or starts on it tangent
 * to it. `common.wgsl`'s `shellCrossings` in `f64`: the roots
 * of t² + 2rμt + C = 0, C = (h − e)(2R + h + e), as q = −(rμ + sign(rμ)√((rμ)² − C)) and C ÷ q
 * (Press et al., _Numerical Recipes_, 3rd ed., §5.6), neither of which cancels (plan R08,
 * R08.T6.d).
 */
export function shellCrossingsM(
  bottomRadiusM: number,
  heightM: number,
  mu: number,
  edgeM: number,
): readonly [number, number] | null {
  const rMu = (bottomRadiusM + heightM) * mu;
  const c = (heightM - edgeM) * (2 * bottomRadiusM + heightM + edgeM);
  const disc = rMu * rMu - c;
  if (disc < 0) {
    return null;
  }
  const q = -(rMu + (rMu >= 0 ? 1 : -1) * Math.sqrt(disc));
  if (q === 0) {
    return null;
  }
  const other = c / q;
  return [Math.min(q, other), Math.max(q, other)];
}

/** A transmittance ray's pieces between the places it crosses layer edges, and their steps. */
export interface RayPieces {
  /** Each piece's end along the ray, m, ascending, the ray's own end last. */
  readonly endsM: ReadonlyArray<number>;
  /** The steps each piece takes. */
  readonly steps: ReadonlyArray<number>;
}

/**
 * How `transmittance.wgsl`'s `rayToTop` splits a ray of length `tMaxM` from `heightM` at zenith
 * cosine `mu` (plan R08, R08.T6.d; `decision-r08-t6b-floor.md` §9.2): at every distance within
 * (0, t_max) where it crosses one of `edgesM` (each once; edges at or below the ground or at or
 * above the top are crossed by no ray of the table's), into pieces. The `samples` steps are spread
 * over them in proportion to their length, at least {@link TRANSMITTANCE_STEPS_A_PIECE} a piece (or
 * the steps over the pieces, if fewer), piece p taking the least plus round(rest × end_p ÷ t_max)
 * less its predecessors', the last the rest, so that they sum to `samples`.
 *
 * @param edgesM - The terms' layer edges, m, in any order.
 */
export function transmittancePieces(
  edgesM: ReadonlyArray<number>,
  shell: Shell,
  heightM: number,
  mu: number,
  tMaxM: number,
  samples: number,
): RayPieces {
  const atmosphereM = shell.topRadiusM - shell.bottomRadiusM;
  const crossings: number[] = [];
  for (const edgeM of edgesM) {
    if (!(edgeM > 0 && edgeM < atmosphereM)) {
      continue;
    }
    const roots = shellCrossingsM(shell.bottomRadiusM, heightM, mu, edgeM);
    for (const t of roots ?? []) {
      if (t > 0 && t < tMaxM) {
        crossings.push(t);
      }
    }
  }
  crossings.sort((a, b) => a - b);
  const endsM: number[] = [];
  for (const t of crossings) {
    const previous = endsM.at(-1);
    if (previous === undefined || t > previous) {
      endsM.push(t);
    }
  }
  endsM.push(tMaxM);
  const n = Math.max(Math.floor(samples), 1);
  const pieces = endsM.length;
  const least = Math.min(TRANSMITTANCE_STEPS_A_PIECE, Math.floor(n / pieces));
  const rest = n - least * pieces;
  let spent = 0;
  const steps = endsM.map((end, p) => {
    const share = p + 1 < pieces ? Math.floor(rest * (end / tMaxM) + 0.5) : rest;
    const taken = least + share - spent;
    spent = share;
    return taken;
  });
  return { endsM, steps };
}

/**
 * The optical depth along a ray to the top by the transmittance kernel's midpoint rule, of every
 * term but the absorbers, into `out`, each absorber slot's column u ÷ U into `columns` and each
 * line slot's pressure-weighted column into `pressureColumns` by pressure channel (`tables.ts`'s
 * `pressureChannelOf`; R08.T4.d), the ray split at the terms' layer edges as
 * {@link transmittancePieces} splits it (R08.T6.d).
 */
function kernelOpticalDepthInto(
  terms: readonly TwinTerm[],
  pressureBarOf: ((heightM: number) => number) | null,
  shell: Shell,
  rM: number,
  mu: number,
  samples: number,
  out: TwinRgb,
  columns: Float64Array,
  pressureColumns: Float64Array,
  scratch: Float64Array,
): void {
  const tMaxM = distanceToTopM(shell, rM, mu);
  const edgesM = terms.flatMap((term) => term.edgesM);
  const { endsM, steps } = transmittancePieces(
    edgesM,
    shell,
    rM - shell.bottomRadiusM,
    mu,
    tMaxM,
    samples,
  );
  out.fill(0);
  columns.fill(0);
  pressureColumns.fill(0);
  let start = 0;
  for (const [p, end] of endsM.entries()) {
    const n = steps[p] ?? 0;
    const dt = n > 0 ? (end - start) / n : 0;
    for (let i = 0; i < n; i += 1) {
      const t = start + (i + 0.5) * dt;
      sampleInto(
        terms,
        pressureBarOf,
        null,
        localRadiusM(rM, mu, t) - shell.bottomRadiusM,
        scratch,
      );
      for (const c of CHANNELS) {
        out[c] += (scratch[3 + c] ?? Number.NaN) * dt;
      }
      for (let k = 0; k < ABSORBER_SLOTS; k += 1) {
        columns[k] = (columns[k] ?? 0) + (scratch[COLUMNS_AT + k] ?? Number.NaN) * dt;
        pressureColumns[k] =
          (pressureColumns[k] ?? 0) + (scratch[PRESSURE_COLUMNS_AT + k] ?? Number.NaN) * dt;
      }
    }
    start = end;
  }
}

/**
 * The optical depth per channel from radius `rM` along zenith cosine `mu` to the top, by
 * `transmittance.wgsl`'s rule: `samples` steps, equal on each piece of the ray between the places it
 * crosses a term's layer edges ({@link transmittancePieces}, R08.T6.d), each sampled at its
 * midpoint.
 *
 * @remarks
 * The table's rays never pass below the ground's tangent (x_μ ≤ 1), and the tangent ray itself,
 * its last column since R08.T6.g's texel mapping, is integrated to the top as Bruneton 2017's
 * `ComputeTransmittanceToTopAtmosphereBoundary` integrates it, never to its tangent point. A ray
 * that does meet the ground is marched through to the top, its heights below the ground taken as
 * the ground's; no texel asks for one. Every term but the absorbers, whose columns the table keeps
 * beside it ({@link transmittanceTwin}).
 *
 * @param bottomRadiusM - The ground's radius, m; the top is `medium.topHeightM` above it.
 * @param samples - The steps, at least 1: the kernel's {@link TRANSMITTANCE_SAMPLES} by default.
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function kernelOpticalDepth(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  samples = TRANSMITTANCE_SAMPLES,
): Rgb {
  return kernelRayToTop(medium, bottomRadiusM, rM, mu, samples).opticalDepth;
}

/** One transmittance ray to the top as the kernel integrates it (`transmittance.wgsl`'s `RayToTop`). */
export interface TwinRayToTop {
  /** The optical depth of every term but the absorbers. */
  readonly opticalDepth: Rgb;
  /** Each absorber slot's column u ÷ U, slot by slot, 0 past the medium's absorbers. */
  readonly columns: Float64Array;
  /**
   * Each line slot's pressure-weighted column, (p ÷ 10⁵ Pa)(u ÷ U) summed, by pressure channel
   * (`tables.ts`'s `pressureChannelOf`; R08.T4.d).
   */
  readonly pressureColumns: Float64Array;
}

/**
 * {@link kernelOpticalDepth} with the absorbers' columns beside it: one texel's ray, unheld to the
 * half float's largest, which the table holds it to.
 *
 * @throws RangeError as {@link kernelOpticalDepth}.
 */
export function kernelRayToTop(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  rM: number,
  mu: number,
  samples = TRANSMITTANCE_SAMPLES,
): TwinRayToTop {
  const out: TwinRgb = [0, 0, 0];
  const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  const columns = new Float64Array(ABSORBER_SLOTS);
  const pressureColumns = new Float64Array(ABSORBER_SLOTS);
  const scratch = new Float64Array(SAMPLE_FLOATS);
  kernelOpticalDepthInto(
    termsOf(medium),
    pressureBarLookup(medium),
    shell,
    rM,
    mu,
    samples,
    out,
    columns,
    pressureColumns,
    scratch,
  );
  return { opticalDepth: out, columns, pressureColumns };
}

/** The tables the transmittance kernel writes, in `f64`, as {@link transmittanceTables} gives them. */
interface TransmittancePair {
  readonly transmittance: TwinTable;
  readonly absorberColumns: TwinTable;
  readonly absorberPressureColumns: TwinTable;
  readonly lastAbsorberPressureColumn: TwinTable;
}

/**
 * The transmittance kernel's four tables at each texel's (r, μ), as `transmittance.wgsl` stores
 * them: the optical depth with slot 0's column, slots 1 to 4's columns, and the pressure-weighted
 * columns of pressure channels 0 to 3 and of channel 4 (R08.T4.d), the last one texel of 0 unless
 * the medium has five line slots (`tables.ts`'s `lastPressureColumnSize`).
 */
function transmittanceTables(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  size: TableSize,
  samples: number,
  coefficientFactor = 1,
): TransmittancePair {
  const terms = scaledTerms(termsOf(medium), coefficientFactor);
  const pressureBarOf = pressureBarLookup(medium);
  const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  const { widthTexels: w, heightTexels: h } = size;
  const first = new Float64Array(w * h * 4);
  const more = new Float64Array(w * h * 4);
  const pressureColumnFour = new Float64Array(w * h * 4);
  const fifthChannel = pressureChannelCount(lineSlotMask(medium)) > PRESSURE_CHANNELS_A_TABLE;
  const pressureColumnLast = new Float64Array(fifthChannel ? w * h * 4 : 4);
  const depth: TwinRgb = [0, 0, 0];
  const columns = new Float64Array(ABSORBER_SLOTS);
  const pressureColumns = new Float64Array(ABSORBER_SLOTS);
  const scratch = new Float64Array(SAMPLE_FLOATS);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const { rM, mu } = transmittanceUvToRMu(shell, unitOfTexel(x, w), unitOfTexel(y, h));
      kernelOpticalDepthInto(
        terms,
        pressureBarOf,
        shell,
        rM,
        mu,
        samples,
        depth,
        columns,
        pressureColumns,
        scratch,
      );
      // Held to the half float's largest, as the kernel stores them.
      first.set(
        [depth[0], depth[1], depth[2], columns[0] ?? Number.NaN].map((v) => Math.min(v, HALF_MAX)),
        (y * w + x) * 4,
      );
      more.set(
        columns.subarray(1, 5).map((v) => Math.min(v, HALF_MAX)),
        (y * w + x) * 4,
      );
      pressureColumnFour.set(
        pressureColumns.subarray(0, 4).map((v) => Math.min(v, HALF_MAX)),
        (y * w + x) * 4,
      );
      if (fifthChannel) {
        pressureColumnLast[(y * w + x) * 4] = Math.min(pressureColumns[4] ?? Number.NaN, HALF_MAX);
      }
    }
  }
  return {
    transmittance: { widthTexels: w, heightTexels: h, texels: first },
    absorberColumns: { widthTexels: w, heightTexels: h, texels: more },
    absorberPressureColumns: { widthTexels: w, heightTexels: h, texels: pressureColumnFour },
    lastAbsorberPressureColumn: fifthChannel
      ? { widthTexels: w, heightTexels: h, texels: pressureColumnLast }
      : { widthTexels: 1, heightTexels: 1, texels: pressureColumnLast },
  };
}

/**
 * The transmittance table's twin at each texel's (r, μ) of Bruneton 2017's parameterisation
 * (`transmittanceUvToRMu`), as `transmittance.wgsl` stores it since R08.T6.c (Design note 8): the
 * optical depth τ per channel of every term but the absorbers in rgb, and absorber slot 0's column
 * u ÷ U in alpha, 0 for a medium with none; each held to the half float's largest, 65,504, as the
 * kernel stores it.
 *
 * @param bottomRadiusM - The tables' ground radius, m (`tableRadiusM(figure)` for a body).
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks), or, as `tables.ts`'s `absorberSlots`, more absorbers than
 *   the tables hold.
 */
export function transmittanceTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  size: TableSize = TRANSMITTANCE_SIZE,
  samples = TRANSMITTANCE_SAMPLES,
): TwinTable {
  return transmittanceTables(medium, bottomRadiusM, size, samples).transmittance;
}

/**
 * The transmittance tables a reader takes, with the lights' curves of growth that turn their
 * absorber columns into optical depth (plan R08, Design notes 5 and 8, R08.T6.c).
 */
export interface TwinTransmittance {
  /** The optical depths in rgb and absorber slot 0's column in alpha ({@link transmittanceTwin}). */
  readonly transmittance: TwinTable;
  /**
   * The absorber-column table: absorber slots 1 to 4's columns u ÷ U in rgba at each of the
   * transmittance table's texels, 0 where the medium has no such slot.
   */
  readonly absorberColumns: TwinTable;
  /**
   * The pressure-weighted columns (R08.T4.d), (p ÷ 10⁵ Pa)(u ÷ U) summed, by pressure channel
   * (`tables.ts`'s `pressureChannelOf`): channels 0 to 3 in rgba here.
   */
  readonly absorberPressureColumns: TwinTable;
  /**
   * Pressure channel 4's in r, at the transmittance's texels for a medium of five line slots and
   * one texel of 0 otherwise (`tables.ts`'s `lastPressureColumnSize`).
   */
  readonly lastAbsorberPressureColumn: TwinTable;
  /** Each light's curves, one an absorber slot (`tables.ts`'s `AbsorberCurveSlots`). */
  readonly curves: AbsorberCurveSlots;
}

/**
 * Both tables the transmittance kernel writes for a medium, with the lights' curves a reader takes
 * them with: {@link transmittanceTwin}'s table and the absorber-column table, from one march a
 * texel.
 *
 * @param bottomRadiusM - The tables' ground radius, m.
 * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium with
 *   no absorber.
 * @param coefficientFactor - The factor every coefficient takes: 1 ÷ s_b for a latitude band's own
 *   tables in its frame ({@link twinBandOf}, R08.T6.e), 1 otherwise.
 * @throws RangeError as {@link transmittanceTwin}, or as `tables.ts`'s `checkAbsorberCurves`.
 */
export function transmittanceTablesTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
  size: TableSize = TRANSMITTANCE_SIZE,
  samples = TRANSMITTANCE_SAMPLES,
  coefficientFactor = 1,
): TwinTransmittance {
  checkAbsorberCurves(medium, curves);
  return {
    ...transmittanceTables(medium, bottomRadiusM, size, samples, coefficientFactor),
    curves,
  };
}

/** A medium's absorbers as a reader of the tables turns their columns into optical depth. */
interface TwinAbsorbers {
  /** One light slot's curve for each absorber slot; none for a medium with no absorber. */
  readonly curves: ReadonlyArray<AbsorberCurve>;
  /** Each curve as the march reads it, slot by slot ({@link readerOf}). */
  readonly readers: ReadonlyArray<CurveReader>;
  /** Each slot's pressure channel, −1 for a slot without lines (`tables.ts`'s `pressureChannelOf`). */
  readonly channels: Int8Array;
  /** Whether the medium's line slots take a fifth channel, `lastAbsorberPressureColumn`'s. */
  readonly fifthChannel: boolean;
}

/**
 * The medium's absorbers under light slot `curveSlot`'s curves, checked against the curves given.
 *
 * @throws RangeError as `tables.ts`'s `checkAbsorberCurves`, or for a medium with absorbers and
 *   no curves at `curveSlot`.
 */
function twinAbsorbers(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  curveSlot: number,
): TwinAbsorbers {
  checkAbsorberCurves(medium, curves);
  const slots = absorberSlots(medium);
  const light = curves[curveSlot];
  if (slots.length > 0 && light === undefined) {
    throw new RangeError(
      `medium ${medium.name}'s absorbers have no curves at light slot ${curveSlot} of ${curves.length}`,
    );
  }
  const taken = light ?? [];
  const lines = lineSlotMask(medium);
  return {
    curves: taken,
    readers: taken.map(readerOf),
    channels: Int8Array.from(slots, ({ slot }) => pressureChannelOf(lines, slot)),
    fifthChannel: pressureChannelCount(lines) > PRESSURE_CHANNELS_A_TABLE,
  };
}

/** ln(u ÷ U) at a curve's node 0: 10⁻³ (`absorbers.ts`'s `ABSORBER_CURVE_DECADES`). */
const CURVE_FIRST_LN = ABSORBER_CURVE_DECADES[0] * Math.LN10;

/** A curve's nodes per unit of ln u. */
const CURVE_NODES_PER_LN = ABSORBER_CURVE_NODES_PER_DECADE / Math.LN10;

/** The deepest optical depth a curve holds: −ln of `absorbers.ts`'s floor. */
const CURVE_DEPTH_MAX = -Math.log(ABSORBER_TRANSMITTANCE_FLOOR);

/**
 * −ln T_c per channel through a path's column `relativeColumn`, u ÷ U, from one curve:
 * `common.wgsl`'s `curveDepth` in `f64`. It is `absorbers.ts`'s `absorberTransmittance` read rule,
 * clamp included (log–log between the nodes about the column, linear where either is 0,
 * proportional to u below node 0, the last interval's power law beyond the last node), with the
 * node found from ln(u ÷ U) on the curve's even grid, as the kernels find it, rather than by a
 * search; on a curve with pressure rows (R08.T4.d), 4-point Lagrange in ln p̄ between its rows, as
 * `absorbers.ts`'s `depthAtPressure` and `common.wgsl`'s `curveDepth` take it. A test holds the
 * two equal.
 *
 * @param relativeColumn - u ÷ U, the column over the curve's absorber's vertical column; 0 or less
 *   for none.
 * @param pressurePa - The path's absorber-weighted pressure, Pa: read on a curve of several rows,
 *   ignored on one of a single row.
 * @throws RangeError for a curve of several rows read with no pressure, or one not finite and above
 *   0, as `absorberTransmittance` refuses it.
 */
export function curveOpticalDepth(
  curve: AbsorberCurve,
  relativeColumn: number,
  pressurePa: number | undefined,
): Rgb {
  if (
    curve.logPressures.length > 1 &&
    (pressurePa === undefined || !(Number.isFinite(pressurePa) && pressurePa > 0))
  ) {
    throw new RangeError(
      `a curve with lines is read at a path's pressure above 0, not ${String(pressurePa)}`,
    );
  }
  const out: TwinRgb = [0, 0, 0];
  addCurveDepth(readerOf(curve), relativeColumn, Math.log(pressurePa ?? 1), out);
  return out;
}

/**
 * A curve as the march reads it: its nodes, and its node depths with their logarithms per
 * channel, taken once a curve, since the march reads a curve at every step and its log–log rule
 * takes the same two logarithms each time (R08.T6.d).
 */
interface CurveReader {
  readonly nodes: number;
  /** Its pressure rows, 1 for an absorber without lines (R08.T4.d). */
  readonly rows: number;
  /** Each row's ln(p ÷ 1 Pa), ascending. */
  readonly logPressures: Float64Array;
  /** The read's top, ln(p ÷ 1 Pa): above it p̄ is held. */
  readonly readTopLogPressure: number;
  /** Row-major, `[r × nodes + j]`. */
  readonly depths: readonly [Float64Array, Float64Array, Float64Array];
  readonly logs: readonly [Float64Array, Float64Array, Float64Array];
  /** A row's depth per channel, the read's scratch. */
  readonly row: TwinRgb;
}

/** Each curve's {@link CurveReader}, made once. */
const CURVE_READERS = new WeakMap<AbsorberCurve, CurveReader>();

/** A curve's {@link CurveReader}, cached. */
function readerOf(curve: AbsorberCurve): CurveReader {
  let reader = CURVE_READERS.get(curve);
  if (reader === undefined) {
    const [r, g, b] = curve.opticalDepth;
    reader = {
      nodes: curve.logColumns.length,
      rows: curve.logPressures.length,
      logPressures: curve.logPressures,
      readTopLogPressure: curve.readTopLogPressure,
      depths: curve.opticalDepth,
      logs: [r.map(Math.log), g.map(Math.log), b.map(Math.log)],
      row: [0, 0, 0],
    };
    CURVE_READERS.set(curve, reader);
  }
  return reader;
}

/** u ÷ U at a curve's node 0, 10⁻³: exp of {@link CURVE_FIRST_LN}. */
const CURVE_FIRST_COLUMN = Math.exp(CURVE_FIRST_LN);

/**
 * Row `r`'s depth per channel at `node`, the column's place on the grid, unclamped
 * (`common.wgsl`'s `curveRowDepth`), into the reader's `row`.
 */
function rowDepthInto(reader: CurveReader, r: number, node: number, proportion: number): void {
  const offset = r * reader.nodes;
  const out = reader.row;
  if (node <= 0) {
    for (const c of CHANNELS) {
      out[c] = (reader.depths[c][offset] ?? Number.NaN) * proportion;
    }
    return;
  }
  const k = Math.min(Math.floor(node), reader.nodes - 2);
  const t = node - k;
  for (const c of CHANNELS) {
    const depths = reader.depths[c];
    const logs = reader.logs[c];
    const a = depths[offset + k] ?? Number.NaN;
    const b = depths[offset + k + 1] ?? Number.NaN;
    const la = logs[offset + k] ?? Number.NaN;
    const lb = logs[offset + k + 1] ?? Number.NaN;
    out[c] = a > 0 && b > 0 ? Math.exp(la + (lb - la) * t) : a + (b - a) * t;
  }
}

/**
 * {@link curveOpticalDepth} added into `out`, at ln(p̄ ÷ 1 Pa) `lnP`: each row's depth, then
 * 4-point Lagrange in ln p̄ on the depth over the four rows nearest, weighted by the rows' own ln p̄;
 * linear on a curve of two or three rows; p̄ held at the first row and at the read's top; the sum
 * clamped to [0, −ln 10⁻⁶] (`common.wgsl`'s `curveDepth`).
 */
function addCurveDepth(reader: CurveReader, x: number, lnP: number, out: TwinRgb): void {
  if (!(x > 0)) {
    return;
  }
  const node = (Math.log(x) - CURVE_FIRST_LN) * CURVE_NODES_PER_LN;
  // exp(ln u − ln u₀) = (u ÷ U) ÷ 10⁻³, below node 0.
  const proportion = x / CURVE_FIRST_COLUMN;
  const rows = reader.rows;
  const row = reader.row;
  if (rows <= 1) {
    rowDepthInto(reader, 0, node, proportion);
    for (const c of CHANNELS) {
      out[c] += Math.min(Math.max(row[c], 0), CURVE_DEPTH_MAX);
    }
    return;
  }
  const lnPs = reader.logPressures;
  const lowest = lnPs[0] ?? Number.NaN;
  const top = Math.min(lnPs[rows - 1] ?? Number.NaN, reader.readTopLogPressure);
  const y = Math.min(Math.max(lnP, lowest), top);
  let r = 0;
  while (r < rows - 2 && (lnPs[r + 1] ?? Number.NaN) < y) {
    r += 1;
  }
  let d0 = 0;
  let d1 = 0;
  let d2 = 0;
  if (rows < 4) {
    const lo = lnPs[r] ?? Number.NaN;
    const hi = lnPs[r + 1] ?? Number.NaN;
    const f = (y - lo) / (hi - lo);
    rowDepthInto(reader, r, node, proportion);
    const [a0, a1, a2] = row;
    rowDepthInto(reader, r + 1, node, proportion);
    d0 = a0 + (row[0] - a0) * f;
    d1 = a1 + (row[1] - a1) * f;
    d2 = a2 + (row[2] - a2) * f;
  } else {
    const s0 = Math.min(Math.max(r - 1, 0), rows - 4);
    for (let m = s0; m < s0 + 4; m += 1) {
      const xm = lnPs[m] ?? Number.NaN;
      let w = 1;
      for (let n = s0; n < s0 + 4; n += 1) {
        if (n !== m) {
          w *= (y - (lnPs[n] ?? Number.NaN)) / (xm - (lnPs[n] ?? Number.NaN));
        }
      }
      rowDepthInto(reader, m, node, proportion);
      d0 += w * row[0];
      d1 += w * row[1];
      d2 += w * row[2];
    }
  }
  out[0] += Math.min(Math.max(d0, 0), CURVE_DEPTH_MAX);
  out[1] += Math.min(Math.max(d1, 0), CURVE_DEPTH_MAX);
  out[2] += Math.min(Math.max(d2, 0), CURVE_DEPTH_MAX);
}

/**
 * ln(p̄ ÷ 1 Pa) of a path's slot from its column and pressure-weighted column (R08.T4.d), p̄ at
 * least {@link ABSORBER_PRESSURE_FLOOR_PA}, as `common.wgsl`'s `pathLogPressure` takes it.
 */
function pathLogPressure(x: number, w: number): number {
  return Math.log(Math.max((ABSORBER_PRESSURE_REFERENCE_PA * w) / x, ABSORBER_PRESSURE_FLOOR_PA));
}

/**
 * The absorbers' optical depth per channel through a path's columns u ÷ U, slot by slot, and its
 * pressure-weighted columns, by pressure channel, under one light slot's curves, into `out`:
 * `common.wgsl`'s `absorberDepth`, each curve read by {@link curveOpticalDepth} at its slot's p̄, a
 * slot without lines at none.
 */
function absorberDepthInto(
  absorbers: TwinAbsorbers,
  relative: Float64Array,
  pressureColumns: Float64Array,
  out: TwinRgb,
): void {
  out.fill(0);
  const { readers, channels } = absorbers;
  for (let k = 0; k < readers.length; k += 1) {
    const reader = readers[k];
    if (reader !== undefined) {
      const x = relative[k] ?? Number.NaN;
      const c = channels[k] ?? -1;
      const lnP = c < 0 ? 0 : pathLogPressure(x, pressureColumns[c] ?? Number.NaN);
      addCurveDepth(reader, x, lnP, out);
    }
  }
}

/**
 * The sun's transmittance at the transmittance table's unit range (u, v) into `out`:
 * `common.wgsl`'s `transmittanceThroughLayer` for the light slot `absorbers` holds, the optical
 * depth, the absorber columns and their pressure-weighted columns read bilinearly through
 * Bruneton's texel mapping, the curves added at each slot's p̄ and the sum exponentiated.
 */
function transmittanceThroughTablesInto(
  tables: TwinTransmittance,
  absorbers: TwinAbsorbers,
  u: number,
  v: number,
  out: TwinRgb,
  scratch: ReadScratch,
): void {
  storedReadInto(tables, absorbers, u, v, scratch);
  const { stored, columns, pressureColumns } = scratch;
  absorberDepthInto(absorbers, columns, pressureColumns, out);
  for (const c of CHANNELS) {
    out[c] = Math.exp(-((stored[c] ?? Number.NaN) + out[c]));
  }
}

/**
 * The transmittance tables' stored values at the unit range (u, v), read bilinearly through
 * Bruneton's texel mapping into `scratch`: the optical depth in `stored`'s rgb, the absorber slots'
 * columns in `columns` (slot 0's from `stored`'s alpha) and their pressure-weighted columns in
 * `pressureColumns`, the tables a medium's absorbers need only.
 */
function storedReadInto(
  tables: TwinTransmittance,
  absorbers: TwinAbsorbers,
  u: number,
  v: number,
  scratch: ReadScratch,
): void {
  const ut = texelUvFromUnit(unitClamp(u), tables.transmittance.widthTexels);
  const vt = texelUvFromUnit(unitClamp(v), tables.transmittance.heightTexels);
  const { stored, columns, more, pressureColumns, pressureColumnFour, pressureColumnLast } =
    scratch;
  bilinearChannelsInto(tables.transmittance, ut, vt, RGBA, stored);
  columns.fill(0);
  pressureColumns.fill(0);
  columns[0] = stored[3] ?? Number.NaN;
  if (absorbers.curves.length > 0) {
    bilinearChannelsInto(tables.absorberPressureColumns, ut, vt, RGBA, pressureColumnFour);
    pressureColumns.set(pressureColumnFour, 0);
    if (absorbers.fifthChannel) {
      bilinearChannelsInto(tables.lastAbsorberPressureColumn, ut, vt, RGBA, pressureColumnLast);
      pressureColumns[4] = pressureColumnLast[0] ?? Number.NaN;
    }
  }
  if (absorbers.curves.length > 1) {
    bilinearChannelsInto(tables.absorberColumns, ut, vt, RGBA, more);
    columns.set(more, 1);
  }
}

/**
 * The arrays {@link transmittanceThroughTablesInto} works in, made once a march rather than as
 * views at every read: the stored texel, the absorber slots' columns and slots 1 to 4's texel, and
 * the pressure-weighted columns with their two texels (R08.T4.d).
 */
interface ReadScratch {
  readonly stored: Float64Array;
  readonly columns: Float64Array;
  readonly more: Float64Array;
  readonly pressureColumns: Float64Array;
  readonly pressureColumnFour: Float64Array;
  readonly pressureColumnLast: Float64Array;
}

/** A fresh {@link ReadScratch}. */
function readScratch(): ReadScratch {
  return {
    stored: new Float64Array(4),
    columns: new Float64Array(ABSORBER_SLOTS),
    more: new Float64Array(4),
    pressureColumns: new Float64Array(ABSORBER_SLOTS),
    pressureColumnFour: new Float64Array(4),
    pressureColumnLast: new Float64Array(4),
  };
}

// --- Multiple scattering -------------------------------------------------------------------------

/** The directions each multiple-scattering texel integrates over: `multiScattering.wgsl`'s 64. */
export const MULTI_SCATTERING_DIRECTIONS = 64;

/** The directions' stratification, 8 × 8 (`SQRT_DIRECTIONS`). */
const SQRT_DIRECTIONS = 8;

/**
 * How far above the ground the multiple-scattering table's lowest radius sits, m: 10,
 * `common.wgsl`'s `GROUND_OFFSET_M`, taken off the height range on the write and the read alike: a
 * deliberate change from sebh's `PLANET_RADIUS_OFFSET`, which his code takes off the range too
 * but also adds to the unitless row coordinate, on the write alone (R08.T6.b).
 */
export const MULTI_SCATTERING_GROUND_OFFSET_M = 10;

/** How the multiple-scattering twin steps along each of its rays. */
export interface MultiScatteringOptions {
  readonly size: TableSize;
  /** The steps along each ray, at least 1 for even steps and at least 2 for placed. */
  readonly samples: number;
  /**
   * `pieces`: `placed`, each side cut into pieces where the ray crosses a layer edge
   * ({@link rayPieces}), the kernel's since R08.T6.i, which on a medium with no edge is `placed` to
   * the bit. `placed`: R05.T12.e's steps toward each ray's lowest point, or for the side before it
   * toward the texel's point (`marchSplit` with `fromCamera`), the kernel's from R08.T6.d. `even`:
   * R05's equal steps sampled at their midpoints, the kernel's until then, against which the placed
   * ones were measured (R08.T6.a).
   */
  readonly placement: "even" | "placed" | "pieces";
  /**
   * The step factor g(x) = (1 − e^(−x)) ÷ x: {@link exactStepFactor} for the twin, or an `f32`
   * emulation of a kernel's form, through which a test measures what `f32` does to the table.
   */
  readonly stepFactor: (x: number) => number;
}

/**
 * The multiple-scattering kernel's own: Hillaire 2020's 32² (his Table 2, at 20 steps a ray) with
 * R05.T12.b's 32 steps a ray, placed by R05.T12.e's rule since R08.T6.d and cut at the layer edges
 * since R08.T6.i (`multiScattering.wgsl`).
 */
export const MULTI_SCATTERING_KERNEL_OPTIONS: MultiScatteringOptions = {
  size: MULTI_SCATTERING_SIZE,
  samples: MULTI_SCATTERING_SAMPLES,
  placement: "pieces",
  stepFactor: exactStepFactor,
};

/**
 * What a multiple-scattering texel reads: the medium, its shell, the transmittance tables and the
 * curves of the light slot it is built with (R08.T6.c).
 */
interface MultiScatteringInputs {
  readonly terms: readonly TwinTerm[];
  /** The marches' layer edges of the terms ({@link marchEdgesOf}), m. */
  readonly edgesM: ReadonlyArray<number>;
  /** The medium's pressure ratio at a height ({@link pressureBarLookup}), or null. */
  readonly pressureBarOf: ((heightM: number) => number) | null;
  readonly shell: Shell;
  readonly groundAlbedo: Rgb;
  readonly transmittance: TwinTransmittance;
  readonly absorbers: TwinAbsorbers;
  readonly options: MultiScatteringOptions;
}

/**
 * The steps of one multiple-scattering ray of length `tMaxM` from radius `rM` along `dir`, whose
 * zenith cosine is `mu`.
 */
function multiScatteringSteps(
  inputs: MultiScatteringInputs,
  rM: number,
  dir: Vec3,
  mu: number,
  tMaxM: number,
): MarchStep[] {
  const options = inputs.options;
  let steps: MarchStep[];
  switch (options.placement) {
    case "even": {
      const n = Math.max(Math.floor(options.samples), 1);
      const dtM = tMaxM / n;
      steps = Array.from({ length: n }, (_, i) => ({ tM: (i + 0.5) * dtM, dtM }));
      break;
    }
    case "placed": {
      const split = marchSplit(0, tMaxM, -rM * mu, options.samples, true);
      steps = Array.from({ length: options.samples }, (_, i) => marchStep(split, i));
      break;
    }
    case "pieces": {
      // From the texel's point, which `skyView.wgsl` marches the same way: t* at −r μ, as above.
      const ray: TwinSegment = {
        originM: { x: 0, y: 0, z: rM },
        direction: dir,
        tStartM: 0,
        tEndM: tMaxM,
        fromCamera: true,
      };
      const sphere: MarchGeometry = { kind: "sphere", bottomRadiusM: inputs.shell.bottomRadiusM };
      steps = piecedSteps(ray, options.samples, inputs.edgesM, sphere);
      break;
    }
  }
  return steps;
}

/**
 * A march's absorber path in `f64` (`common.wgsl`'s `AbsorberPath`): the columns u ÷ U crossed
 * from the ray's start, slot by slot, their pressure-weighted columns (R08.T4.d), and the optical
 * depth one light slot's curves give them.
 */
class TwinAbsorberPath {
  readonly columns = new Float64Array(ABSORBER_SLOTS);
  readonly pressureColumns = new Float64Array(ABSORBER_SLOTS);
  readonly depth: TwinRgb = [0, 0, 0];
  readonly #next: TwinRgb = [0, 0, 0];
  readonly #active: boolean;
  readonly #depthOf: (columns: Float64Array, pressureColumns: Float64Array, out: TwinRgb) => void;

  /**
   * @param active - Whether the medium has an absorber; a path through none stays empty.
   * @param depthOf - The optical depth the light's curves give a path's columns and
   *   pressure-weighted columns, into `out`.
   */
  constructor(
    active: boolean,
    depthOf: (columns: Float64Array, pressureColumns: Float64Array, out: TwinRgb) => void,
  ) {
    this.#active = active;
    this.#depthOf = depthOf;
  }

  /**
   * The path one step `dtM` longer through a sample's columns and pressure-weighted columns per
   * metre (`local`, from `COLUMNS_AT` and `PRESSURE_COLUMNS_AT`), into `stepDepth`: how much the
   * step deepens the whole path's optical depth (`absorberStep`), which may fall a little below 0
   * where the path's p̄ moves on a curve with pressure rows. Nothing for a medium with no absorber.
   */
  stepInto(local: Float64Array, dtM: number, stepDepth: TwinRgb): void {
    stepDepth.fill(0);
    if (!this.#active) {
      return;
    }
    for (let k = 0; k < ABSORBER_SLOTS; k += 1) {
      this.columns[k] = (this.columns[k] ?? 0) + (local[COLUMNS_AT + k] ?? Number.NaN) * dtM;
      this.pressureColumns[k] =
        (this.pressureColumns[k] ?? 0) + (local[PRESSURE_COLUMNS_AT + k] ?? Number.NaN) * dtM;
    }
    this.#depthOf(this.columns, this.pressureColumns, this.#next);
    for (const c of CHANNELS) {
      stepDepth[c] = this.#next[c] - this.depth[c];
      this.depth[c] = this.#next[c];
    }
  }
}

/** The arrays one multiple-scattering direction's integral works in. */
interface DirectionScratch {
  readonly sample: Float64Array;
  readonly read: ReadScratch;
  readonly toSun: TwinRgb;
  readonly absorbed: TwinRgb;
}

/** The floats {@link integrateDirectionInto} writes: three triples. */
const DIRECTION_FLOATS = 9;

/**
 * One direction's integral (`multiScattering.wgsl`'s `integrate`): the luminance it brings per
 * unit illuminance into 0–2 (single scattering under the isotropic phase, and the ground's
 * Lambertian bounce); f_ms's integrand into 3–5, Σ throughput σ_s Δt g(d), which the kernel no
 * longer forms and the twin keeps for its tests; and its escape, 1 − f_ms accumulated, into 6–8
 * (R08.T6.d; `decision-r08-t6b-floor.md` §9.3): the throughput at the ray's end plus
 * Σ throughput (σ_a Δt + the step's absorber depth) g(d), equal to 1 − f_ms since
 * Σ throughput (1 − e^(−d)) telescopes to one less the end's throughput. Its own path takes its
 * absorbers by the path's column under the curves the table is built with, as the sun's
 * transmittance does (R08.T6.c).
 */
function integrateDirectionInto(
  inputs: MultiScatteringInputs,
  rM: number,
  dir: Vec3,
  sun: Vec3,
  out: Float64Array,
  scratch: DirectionScratch,
): void {
  const { terms, pressureBarOf, shell, transmittance, absorbers, options } = inputs;
  const { sample, read, toSun, absorbed } = scratch;
  const mu = dir.z;
  const tMax = maxDistanceM(shell, rM, mu);
  out.fill(0);
  const throughput: TwinRgb = [1, 1, 1];
  const path = new TwinAbsorberPath(hasAbsorbers(terms), (columns, pressureColumns, depth) => {
    absorberDepthInto(absorbers, columns, pressureColumns, depth);
  });
  for (const { tM, dtM } of multiScatteringSteps(inputs, rM, dir, mu, tMax)) {
    const px = tM * dir.x;
    const py = tM * dir.y;
    const pz = rM + tM * dir.z;
    const rP = Math.sqrt(px * px + py * py + pz * pz);
    const muSun = (sun.x * px + sun.y * py + sun.z * pz) / rP;
    sampleInto(terms, pressureBarOf, null, rP - shell.bottomRadiusM, sample);
    const lit = intersectsGround(shell, rP, muSun) ? 0 : 1;
    const [u, v] = transmittanceRMuToUv(shell, rP, muSun);
    transmittanceThroughTablesInto(transmittance, absorbers, u, v, toSun, read);
    path.stepInto(sample, dtM, absorbed);
    for (const c of CHANNELS) {
      // The tabulated terms' in-lobe scattering truncated, the beam raised by e^(x P) (R08.T7).
      const lobe = sample[LOBE_AT + c] ?? 0;
      const fullExtinction = sample[3 + c] ?? Number.NaN;
      const scattering = Math.max((sample[c] ?? Number.NaN) - lobe, 0);
      const stepDepth = Math.max(fullExtinction - lobe, 0) * dtM + absorbed[c];
      const g = options.stepFactor(stepDepth);
      const boost = lobeBoost(read.stored[c] ?? Number.NaN, lobe, fullExtinction);
      const source = (lit * toSun[c] * boost * scattering) / (4 * Math.PI);
      const absorbedHere = (sample[ABSORPTION_AT + c] ?? Number.NaN) * dtM + absorbed[c];
      out[c] = (out[c] ?? 0) + throughput[c] * source * dtM * g;
      out[3 + c] = (out[3 + c] ?? 0) + throughput[c] * scattering * dtM * g;
      out[6 + c] = (out[6 + c] ?? 0) + throughput[c] * absorbedHere * g;
      throughput[c] *= Math.exp(-stepDepth);
    }
  }
  for (const c of CHANNELS) {
    out[6 + c] = (out[6 + c] ?? 0) + throughput[c];
  }
  if (intersectsGround(shell, rM, mu)) {
    const gx = tMax * dir.x;
    const gy = tMax * dir.y;
    const gz = rM + tMax * dir.z;
    const muSun = (sun.x * gx + sun.y * gy + sun.z * gz) / Math.sqrt(gx * gx + gy * gy + gz * gz);
    const [u, v] = transmittanceRMuToUv(shell, shell.bottomRadiusM, muSun);
    transmittanceThroughTablesInto(transmittance, absorbers, u, v, toSun, read);
    sampleInto(terms, pressureBarOf, null, 0, sample);
    for (const c of CHANNELS) {
      const boost = lobeBoost(
        read.stored[c] ?? Number.NaN,
        sample[LOBE_AT + c] ?? 0,
        sample[3 + c] ?? Number.NaN,
      );
      out[c] =
        (out[c] ?? 0) +
        (toSun[c] * boost * throughput[c] * Math.max(muSun, 0) * inputs.groundAlbedo[c]) / Math.PI;
    }
  }
}

/**
 * The forward-diffuse light a lobe's truncation keeps in the beam, e^(x P), x P the stored optical
 * depth `depth` times the lobes' share `lobe` of the extinction (`multiScattering.wgsl`'s
 * `transmittanceToSun`, R08.T7), the exponent held under 80; 1 where there is no lobe.
 */
function lobeBoost(depth: number, lobe: number, extinctionPerM: number): number {
  return lobe > 0 ? Math.exp(Math.min((depth * lobe) / Math.max(extinctionPerM, 1e-30), 80)) : 1;
}

/**
 * One multiple-scattering texel's sums over its 64 directions, each their mean: the second order
 * L₂, f_ms as Hillaire forms it, and the escape, 1 − f_ms accumulated (R08.T6.d).
 */
export interface MultiScatteringSeries {
  readonly secondOrder: Rgb;
  readonly fMs: Rgb;
  readonly escape: Rgb;
}

/** What a multiple-scattering texel stores from its series: its colour and its alpha. */
export interface MultiScatteringStored {
  /** L₂ ÷ mean(escape) per channel, or L₂ alone where the escape is 0. */
  readonly value: Rgb;
  /** How many channels keep L₂ alone, 0 to 3: the texel's alpha. */
  readonly alone: number;
}

/**
 * The value a multiple-scattering texel stores from its series, as `multiScattering.wgsl` stores it
 * (R08.T6.d; `decision-r08-t6b-floor.md` §9.3): Hillaire's L₂ ÷ (1 − f_ms) with 1 − f_ms the
 * accumulated escape, or the second order L₂ alone, a lower bound of the term, in a channel whose
 * escape is 0. No ε and no cap.
 */
export function multiScatteringStored(series: MultiScatteringSeries): MultiScatteringStored {
  let alone = 0;
  const value = CHANNELS.map((c) => {
    const escape = series.escape[c];
    if (escape > 0) {
      return series.secondOrder[c] / escape;
    }
    alone += 1;
    return series.secondOrder[c];
  });
  return { value: [value[0] ?? Number.NaN, value[1] ?? Number.NaN, value[2] ?? Number.NaN], alone };
}

/** One multiple-scattering texel's {@link MultiScatteringSeries}. */
function multiScatteringSeriesAt(
  inputs: MultiScatteringInputs,
  x: number,
  y: number,
): MultiScatteringSeries {
  const { widthTexels: w, heightTexels: h } = inputs.options.size;
  const { bottomRadiusM: bottom, topRadiusM: top } = inputs.shell;
  const unitX = unitOfTexel(x, w);
  const unitY = unitOfTexel(y, h);
  const rM =
    bottom +
    MULTI_SCATTERING_GROUND_OFFSET_M +
    unitY * (top - bottom - MULTI_SCATTERING_GROUND_OFFSET_M);
  const muSun = unitX * 2 - 1;
  const sun = { x: 0, y: Math.sqrt(Math.max(1 - muSun * muSun, 0)), z: muSun };
  const sum = new Float64Array(DIRECTION_FLOATS);
  const one = new Float64Array(DIRECTION_FLOATS);
  const scratch: DirectionScratch = {
    sample: new Float64Array(SAMPLE_FLOATS),
    read: readScratch(),
    toSun: [0, 0, 0],
    absorbed: [0, 0, 0],
  };
  for (let z = 0; z < MULTI_SCATTERING_DIRECTIONS; z += 1) {
    const a = (0.5 + Math.floor(z / SQRT_DIRECTIONS)) / SQRT_DIRECTIONS;
    const b = (0.5 + (z % SQRT_DIRECTIONS)) / SQRT_DIRECTIONS;
    const theta = 2 * Math.PI * a;
    const cosPhi = 1 - 2 * b;
    const sinPhi = Math.sqrt(Math.max(1 - cosPhi * cosPhi, 0));
    const dir = { x: Math.cos(theta) * sinPhi, y: Math.sin(theta) * sinPhi, z: cosPhi };
    integrateDirectionInto(inputs, rM, dir, sun, one, scratch);
    for (let i = 0; i < DIRECTION_FLOATS; i += 1) {
      sum[i] = (sum[i] ?? 0) + (one[i] ?? Number.NaN);
    }
  }
  const mean = (at: number): Rgb => [
    (sum[at] ?? Number.NaN) / MULTI_SCATTERING_DIRECTIONS,
    (sum[at + 1] ?? Number.NaN) / MULTI_SCATTERING_DIRECTIONS,
    (sum[at + 2] ?? Number.NaN) / MULTI_SCATTERING_DIRECTIONS,
  ];
  return { secondOrder: mean(0), fMs: mean(3), escape: mean(6) };
}

/**
 * The multiple-scattering table's twin: at each texel's height and sun zenith cosine (sebh's
 * mapping, its ground {@link MULTI_SCATTERING_GROUND_OFFSET_M} up), the luminance that every order
 * past the first adds per unit illuminance under an isotropic phase, as `multiScattering.wgsl`
 * stores it: L₂ ÷ (1 − f_ms) with 1 − f_ms the accumulated escape (Hillaire 2020, eqs. 9 and 10;
 * R08.T6.d), or L₂ alone in a channel whose escape is 0 in `f64`, with alpha the count of such
 * channels, 0 to 3. It reads `transmittance` as the kernel reads its tables: the optical depth and
 * the absorber columns with light slot `curveSlot`'s curves (R08.T6.c).
 *
 * @param bottomRadiusM - The tables' ground radius, m, the transmittance table's own.
 * @param texels - The texels to compute, each [x, y]; every texel by default. Texels not listed
 *   are left 0.
 * @param curveSlot - The light slot whose curves the table is built with, the kernel's
 *   `curveSlot()`: `tables.ts`'s `SHARED_TABLE_CURVE_SLOT` until R08.T7.
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks), as `tables.ts`'s `checkAbsorberCurves` for curves that do
 *   not fit the medium's absorbers or none at `curveSlot`, or from `marchSplit` for placed steps of
 *   fewer than 2.
 */
export function multiScatteringTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  transmittance: TwinTransmittance,
  options: MultiScatteringOptions = MULTI_SCATTERING_KERNEL_OPTIONS,
  texels?: ReadonlyArray<readonly [number, number]>,
  curveSlot = SHARED_TABLE_CURVE_SLOT,
): TwinTable {
  return multiScatteringTableOf(
    multiScatteringInputsOf(medium, bottomRadiusM, transmittance, options, curveSlot, 1),
    texels,
  );
}

/**
 * What {@link multiScatteringTwin} reads, its terms' coefficients times `coefficientFactor`
 * ({@link scaledTerms}: 1 but in a latitude band's own frame, R08.T6.e).
 */
function multiScatteringInputsOf(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  transmittance: TwinTransmittance,
  options: MultiScatteringOptions,
  curveSlot: number,
  coefficientFactor: number,
): MultiScatteringInputs {
  const terms = scaledTerms(termsOf(medium), coefficientFactor);
  return {
    terms,
    edgesM: marchEdgesOf(terms, medium.topHeightM),
    pressureBarOf: pressureBarLookup(medium),
    shell: { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM },
    groundAlbedo: medium.groundAlbedo,
    transmittance,
    absorbers: twinAbsorbers(medium, transmittance.curves, curveSlot),
    options,
  };
}

/** The multiple-scattering table of `inputs` at the listed texels, or every texel. */
function multiScatteringTableOf(
  inputs: MultiScatteringInputs,
  texels?: ReadonlyArray<readonly [number, number]>,
): TwinTable {
  const { widthTexels: w, heightTexels: h } = inputs.options.size;
  const out = new Float64Array(w * h * 4);
  const which =
    texels ?? Array.from({ length: w * h }, (_, i) => [i % w, Math.floor(i / w)] as const);
  for (const [x, y] of which) {
    const { value, alone } = multiScatteringStored(multiScatteringSeriesAt(inputs, x, y));
    out.set([value[0], value[1], value[2], alone], (y * w + x) * 4);
  }
  return { widthTexels: w, heightTexels: h, texels: out };
}

/**
 * Each listed texel's {@link MultiScatteringSeries}, as {@link multiScatteringTwin} sums them: the
 * tests' view of the escape beside the f_ms it replaces (R08.T6.d).
 *
 * @throws RangeError as {@link multiScatteringTwin}.
 */
export function multiScatteringSeriesTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  transmittance: TwinTransmittance,
  texels: ReadonlyArray<readonly [number, number]>,
  options: MultiScatteringOptions = MULTI_SCATTERING_KERNEL_OPTIONS,
  curveSlot = SHARED_TABLE_CURVE_SLOT,
): MultiScatteringSeries[] {
  const terms = termsOf(medium);
  const inputs: MultiScatteringInputs = {
    terms,
    edgesM: marchEdgesOf(terms, medium.topHeightM),
    pressureBarOf: pressureBarLookup(medium),
    shell: { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM },
    groundAlbedo: medium.groundAlbedo,
    transmittance,
    absorbers: twinAbsorbers(medium, transmittance.curves, curveSlot),
    options,
  };
  return texels.map(([x, y]) => multiScatteringSeriesAt(inputs, x, y));
}

// --- The per-planet tables and their reads -------------------------------------------------------

/**
 * A planet's per-planet tables in `f64`, at the kernels' sizes and steps, with the lights' curves
 * they were built with: the transmittance and absorber-column tables (R08.T6.c) and the multiple
 * scattering.
 */
export interface PlanetTwin extends TwinTransmittance {
  readonly medium: AtmosphereMedium;
  /** The tables' ground radius, m. */
  readonly bottomRadiusM: number;
  /** The multiple-scattering table built with light slot 0's curves: its layer 0. */
  readonly multiScattering: TwinTable;
  /**
   * Each layer of the multiple-scattering table, layer k built with light slot k's curves
   * (`tables.ts`'s `multiScatteringLayersOf`, R08.T7), layer 0 `multiScattering` itself; absent
   * for a table of one layer.
   */
  readonly multiScatteringBySlot?: ReadonlyArray<TwinTable>;
}

/**
 * The per-planet tables for a medium, as `AtmosphereTables` builds them: transmittance and the
 * absorber columns at 256 × 64 with 256 steps, split at the terms' layer edges, then multiple
 * scattering at 32² with 32 placed steps (R08.T6.d) reading them, one layer a light slot under an
 * absorber, each with its slot's curves, and one layer otherwise (R08.T7).
 *
 * @param bottomRadiusM - The tables' ground radius, m (`tableRadiusM(figure)` for a body).
 * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium with
 *   no absorber.
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks), or as `tables.ts`'s `checkAbsorberCurves`.
 */
export function planetTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
): PlanetTwin {
  const tables = transmittanceTablesTwin(medium, bottomRadiusM, curves);
  const layers = multiScatteringLayersOf(medium, curves);
  const bySlot = Array.from({ length: layers }, (_, slot) =>
    multiScatteringTwin(
      medium,
      bottomRadiusM,
      tables,
      MULTI_SCATTERING_KERNEL_OPTIONS,
      undefined,
      slot,
    ),
  );
  const first = bySlot[0];
  if (first === undefined) {
    throw new Error("the multiple-scattering table has at least one layer");
  }
  return {
    medium,
    bottomRadiusM,
    ...tables,
    multiScattering: first,
    ...(layers > 1 ? { multiScatteringBySlot: bySlot } : {}),
  };
}

/**
 * What a march reads the per-planet tables through, at a height above the datum, for a light slot
 * (R08.T7): the twin's own tables ({@link tableSources}), or a test's stand-in, which may take no
 * slot.
 */
export interface TwinSources {
  /** The tables' ground radius, m: the sphere the sun's ground shadow is tested on. */
  readonly bottomRadiusM: number;
  /**
   * The transmittance from a height to the top along a source's zenith cosine, into `out`, under
   * light slot `slot`'s curves.
   */
  readonly sunTransmittance: (heightM: number, muSun: number, out: TwinRgb, slot: number) => void;
  /** The multiple-scattering source per unit illuminance at a height, into `out`, slot `slot`'s. */
  readonly multiScattering: (heightM: number, muSun: number, out: TwinRgb, slot: number) => void;
  /**
   * The optical depth per channel light slot `slot`'s curves of growth give a march's own path
   * through the absorber columns u ÷ U, slot by slot, and the pressure-weighted columns, by pressure
   * channel, into `out` (R08.T6.c, R08.T4.d, R08.T7). Asked only of a medium with an absorber.
   */
  readonly absorberPathDepth: (
    columns: Float64Array,
    pressureColumns: Float64Array,
    out: TwinRgb,
    slot: number,
  ) => void;
  /**
   * The stored optical depth per channel of every term but the absorbers from a height to the top
   * along a source's zenith cosine, into `out`: the forward lobes' depth (R08.T7). A stand-in with
   * no tabulated term may leave it out, and its marches then add no lobe.
   */
  readonly sunOpticalDepth?: (heightM: number, muSun: number, out: TwinRgb) => void;
}

/**
 * A planet's tables read as the per-frame kernels read them: `source.wgsl`'s `tableTransmittance`
 * (the height clamped into the shell; the optical depth and absorber columns with the light slot's
 * curves, exponentiated, R08.T6.c and R08.T7) and `tableMultiScattering` (sebh's unit range, its
 * 10 m offset, the slot's layer, or the one layer), each bilinear through Bruneton's texel mapping
 * (plan R08, Design note 19).
 *
 * @throws RangeError as `tables.ts`'s `checkAbsorberCurves`, for curves that do not fit the
 *   planet's absorbers, or, at a read, for a slot the curves do not hold.
 */
export function tableSources(planet: PlanetTwin): TwinSources {
  const bottom = planet.bottomRadiusM;
  const atmosphereM = planet.medium.topHeightM;
  const shell = { bottomRadiusM: bottom, topRadiusM: bottom + atmosphereM };
  const layers = planet.multiScatteringBySlot ?? [planet.multiScattering];
  checkAbsorberCurves(planet.medium, planet.curves);
  const bySlot = new Map<number, TwinAbsorbers>();
  const absorbersAt = (slot: number): TwinAbsorbers => {
    let absorbers = bySlot.get(slot);
    if (absorbers === undefined) {
      absorbers = twinAbsorbers(planet.medium, planet.curves, slot);
      bySlot.set(slot, absorbers);
    }
    return absorbers;
  };
  absorbersAt(0);
  const scratch = readScratch();
  return {
    bottomRadiusM: bottom,
    sunTransmittance: (heightM, muSun, out, slot) => {
      const rM = bottom + Math.min(Math.max(heightM, 0), atmosphereM);
      const [u, v] = transmittanceRMuToUv(shell, rM, muSun);
      transmittanceThroughTablesInto(planet, absorbersAt(slot), u, v, out, scratch);
    },
    multiScattering: (heightM, muSun, out, slot) => {
      const v =
        (Math.max(heightM, 0) - MULTI_SCATTERING_GROUND_OFFSET_M) /
        (atmosphereM - MULTI_SCATTERING_GROUND_OFFSET_M);
      const table = layers[Math.min(slot, layers.length - 1)] ?? planet.multiScattering;
      tableAtUnitInto(table, muSun * 0.5 + 0.5, v, out);
    },
    absorberPathDepth: (columns, pressureColumns, out, slot) => {
      absorberDepthInto(absorbersAt(slot), columns, pressureColumns, out);
    },
    sunOpticalDepth: (heightM, muSun, out) => {
      const rM = bottom + Math.min(Math.max(heightM, 0), atmosphereM);
      const [u, v] = transmittanceRMuToUv(shell, rM, muSun);
      tableAtUnitInto(planet.transmittance, u, v, out);
    },
  };
}

// --- The per-frame marches -----------------------------------------------------------------------

/** One ray of a per-frame march: its origin from the centre of what it is marched about. */
export interface TwinRay {
  readonly originM: Vec3;
  /** The unit direction of the ray. */
  readonly direction: Vec3;
  /**
   * The unit direction to the sun: the one source of a march per unit of its illuminance, at light
   * slot 0, where {@link TwinRay.sources} is absent; the first source's otherwise, or the zenith.
   */
  readonly sun: Vec3;
  /** The segment marched, m along the ray. */
  readonly tStartM: number;
  readonly tEndM: number;
  /** Whether the segment starts at a camera inside the atmosphere (`marchSplit`'s `fromCamera`). */
  readonly fromCamera: boolean;
  /**
   * The march's sources, summed in one march (Design note 7, R08.T7), in the frame of the ray;
   * absent for the one source `sun` per unit of its illuminance.
   */
  readonly sources?: ReadonlyArray<TwinRaySource>;
}

/** One source of a per-frame march (`view.wgsl`'s `SkySource` as a march takes it, R08.T7). */
export interface TwinRaySource {
  /** Its unit direction, in the frame of the ray. */
  readonly direction: Vec3;
  /** Its light's weight per channel in what the march gathers: its illuminance times the scale. */
  readonly weight: Rgb;
  /**
   * Its light slot: its absorber curves, its multiple-scattering layer and, under an absorber, its
   * own path along the view; a medium with none takes every source on one path.
   */
  readonly slot: number;
}

/** A per-frame march's segment, its ray without the sun: what its steps are placed on. */
export type TwinSegment = Omit<TwinRay, "sun" | "sources">;

/**
 * What a march takes a sample's height and sun cosine against: a sphere about the centre (the
 * sky view's and aerial perspective's, of the camera's Gaussian radius √(MN)), or the datum
 * spheroid (the ray march's, the height along the radius and the normal of the similar spheroid
 * through the sample, as `rayMarch.wgsl`'s `heightAbove` and `normalAt`).
 */
export type MarchGeometry =
  | { readonly kind: "sphere"; readonly bottomRadiusM: number }
  | { readonly kind: "spheroid"; readonly figure: SpheroidFigure }
  | {
      readonly kind: "gravitySpheroid";
      readonly figure: SpheroidFigure;
      /**
       * 1 ÷ s at the equator and at the pole: an oblate body's shell through h\* = e there is the
       * quadric (a + e ÷ s_e, c + e ÷ s_p), as R08.T12.d's tracer takes its shells (plan R08, Design
       * note 17; R08.T6.e). Its samples are taken as the datum spheroid's.
       */
      readonly equatorialShellScale: number;
      readonly polarShellScale: number;
    };

/**
 * A march's result: the in-scattered radiance in its sources' weights (per unit of the one sun's
 * illuminance, or pre-exposed luminance, R08.T7) and the transmittance.
 */
export interface TwinMarch {
  readonly luminance: Rgb;
  readonly transmittance: Rgb;
}

/**
 * A ray's steps as R05.T12.e places them: split at its lowest point t* = clamp(−o·d, t_start,
 * t_end) and placed quadratically toward it, or the camera side of a two-sided segment from a
 * camera inside the atmosphere toward the camera (`marchSplit`, `marchStep`).
 *
 * @param samples - The steps, an integer of at least 2.
 * @throws RangeError from `marchSplit` for fewer than 2 steps.
 */
export function placedSteps(ray: TwinRay, samples: number): MarchStep[] {
  const o = ray.originM;
  const d = ray.direction;
  const nearestM = -(o.x * d.x + o.y * d.y + o.z * d.z);
  const split = marchSplit(ray.tStartM, ray.tEndM, nearestM, samples, ray.fromCamera);
  return Array.from({ length: samples }, (_, i) => marchStep(split, i));
}

/**
 * The per-frame marches' layer edges of a medium's terms (plan R08, R08.T6.i): each ramp of a
 * term's density as one edge at its middle height (`marchSteps.ts`'s `marchEdgesM`), the terms'
 * in order, as `tables.ts`'s `packLayerEdges` packs them for the kernels.
 */
function marchEdgesOf(terms: readonly TwinTerm[], topHeightM: number): number[] {
  return terms.flatMap((term) => marchEdgesM(term.edgesM.slice(0, EDGES_PER_TERM), topHeightM));
}

/**
 * The per-frame marches' layer edges of a medium, m (plan R08, R08.T6.i): `tables.ts`'s
 * `packLayerEdges`' middles, in `f64`, of the medium as given. A twin of the kernels takes
 * `kernelMedium(medium)`, whose edges are widened to the kernels' levels.
 *
 * @throws RangeError as the twins, for a medium with a tabulated term that ends below its top at a
 *   density other than 0 (the module's remarks).
 */
export function mediumMarchEdgesM(medium: AtmosphereMedium): number[] {
  return marchEdgesOf(termsOf(medium), medium.topHeightM);
}

/**
 * The two roots of a ray from `o` along unit `d` on the spheroid of radii (a, a, c), nearer first,
 * or `null` where it misses it: as `rayMarch.wgsl`'s `quadricHits`, as
 * s ÷ q_a and q_c ÷ s with s = −(q_b + sign(q_b)√disc), neither of which cancels.
 */
function quadricRootsM(o: Vec3, d: Vec3, a: number, c: number): readonly [number, number] | null {
  const k = a / c;
  const qa = d.x * d.x + d.y * d.y + d.z * k * (d.z * k);
  const qb = o.x * d.x + o.y * d.y + o.z * k * (d.z * k);
  const qc = o.x * o.x + o.y * o.y + o.z * k * (o.z * k) - a * a;
  const disc = qb * qb - qa * qc;
  if (disc < 0) {
    return null;
  }
  const root = Math.sqrt(disc);
  const s = -(qb + (qb >= 0 ? root : -root));
  if (s === 0) {
    return null;
  }
  const first = s / qa;
  const second = qc / s;
  return [Math.min(first, second), Math.max(first, second)];
}

/**
 * Where a per-frame march's ray crosses the marches' layer edges `edgesM`, as signed distances from
 * its lowest point at `lowestM` along it, m (plan R08, R08.T6.i): on a sphere about the centre,
 * each edge's sphere (`shellCrossingsM`, as `skyView.wgsl` and `multiScattering.wgsl` find them,
 * from the ray's height at its origin and zenith cosine there); on the datum spheroid, each edge
 * e's shell quadric, the spheroid (a + e, c + e), as `rayMarch.wgsl` finds them, and as it clips the
 * ray at the top's (a + H, c + H).
 */
function crossingsFromLowestM(
  ray: TwinSegment,
  lowestM: number,
  edgesM: ReadonlyArray<number>,
  geometry: MarchGeometry,
): number[] {
  const o = ray.originM;
  const d = ray.direction;
  const rM = Math.sqrt(o.x * o.x + o.y * o.y + o.z * o.z);
  const mu = (o.x * d.x + o.y * d.y + o.z * d.z) / rM;
  const out: number[] = [];
  for (const edgeM of edgesM) {
    let roots: readonly [number, number] | null;
    switch (geometry.kind) {
      case "sphere": {
        roots = shellCrossingsM(geometry.bottomRadiusM, rM - geometry.bottomRadiusM, mu, edgeM);
        break;
      }
      case "spheroid": {
        const { equatorialRadiusM: a, polarRadiusM: c } = geometry.figure;
        roots = quadricRootsM(o, d, a + edgeM, c + edgeM);
        break;
      }
      case "gravitySpheroid": {
        const { equatorialRadiusM: a, polarRadiusM: c } = geometry.figure;
        roots = quadricRootsM(
          o,
          d,
          a + edgeM * geometry.equatorialShellScale,
          c + edgeM * geometry.polarShellScale,
        );
        break;
      }
    }
    for (const t of roots ?? []) {
      out.push(t - lowestM);
    }
  }
  return out;
}

/**
 * A per-frame march's segment as the kernels cut it since R08.T6.i (plan R08;
 * `decision-r08-t6d-findings.md` §1): R05.T12.e's split ({@link placedSteps}'), each side cut into
 * pieces where the ray crosses one of `edgesM` (`marchSteps.ts`'s `marchPieces`), on the sphere or
 * spheroid it is marched about. With no edge, or none crossed, its steps are
 * {@link placedSteps}' to the bit.
 *
 * @param samples - The steps, an integer of at least 2.
 * @param edgesM - The marches' layer edges, m ({@link mediumMarchEdgesM}).
 * @throws RangeError from `marchSplit` for fewer than 2 steps.
 */
export function rayPieces(
  ray: TwinSegment,
  samples: number,
  edgesM: ReadonlyArray<number>,
  geometry: MarchGeometry,
): MarchPieces {
  const o = ray.originM;
  const d = ray.direction;
  const nearestM = -(o.x * d.x + o.y * d.y + o.z * d.z);
  const split = marchSplit(ray.tStartM, ray.tEndM, nearestM, samples, ray.fromCamera);
  return marchPieces(split, crossingsFromLowestM(ray, split.lowestM, edgesM, geometry));
}

/**
 * A per-frame march's steps as the kernels place them since R08.T6.i: {@link rayPieces}' segment,
 * stepped piece by piece (`marchSteps.ts`'s `marchPiecesSteps`).
 *
 * @throws RangeError as {@link rayPieces}.
 */
export function piecedSteps(
  ray: TwinSegment,
  samples: number,
  edgesM: ReadonlyArray<number>,
  geometry: MarchGeometry,
): MarchStep[] {
  return marchPiecesSteps(rayPieces(ray, samples, edgesM, geometry));
}

/**
 * Whether the ground of a sphere of radius `bottomRadiusM` hides the sun from a height above it at
 * the sun's zenith cosine: `common.wgsl`'s `shellIntersectsGround` at r = R + max(h, 0), as
 * `source.wgsl`'s `sourceAt` tests it.
 */
function groundShadows(bottomRadiusM: number, heightM: number, muSun: number): boolean {
  const rM = bottomRadiusM + Math.max(heightM, 0);
  return muSun < 0 && rM * rM * (muSun * muSun - 1) + bottomRadiusM * bottomRadiusM >= 0;
}

/**
 * Each term's scattering towards the eye for one source, summed per channel into `out`, from the
 * terms' densities at a sample and their phases for the source (`source.wgsl`'s
 * `phasedScatteringOf`): each term's scattering times its density, times its phase, in the terms'
 * order, as {@link sampleInto} sums it for one sun, to the bit.
 */
function phasedInto(
  terms: readonly TwinTerm[],
  densities: Float64Array,
  phases: Float64Array,
  out: TwinRgb,
): void {
  let p0 = 0;
  let p1 = 0;
  let p2 = 0;
  for (let k = 0; k < terms.length; k += 1) {
    const term = terms[k];
    if (term === undefined) {
      continue;
    }
    const d = densities[k] ?? Number.NaN;
    const scattering = term.scattering;
    p0 += scattering[0] * d * (phases[k * 3] ?? Number.NaN);
    p1 += scattering[1] * d * (phases[k * 3 + 1] ?? Number.NaN);
    p2 += scattering[2] * d * (phases[k * 3 + 2] ?? Number.NaN);
  }
  out[0] = p0;
  out[1] = p1;
  out[2] = p2;
}

/** One source as a march takes it: its direction, weight and slot, and its terms' phases. */
interface MarchSource extends TwinRaySource {
  /** Each term's phase per channel for the angle between the ray and the source. */
  readonly phases: Float64Array;
  /** u = √(θ ÷ π) times 255, the place in the phase tables' entries of the angle (R08.T7). */
  readonly entry: number;
  /** Its direction in the body-fixed axes: an oblate march's lookups' (R08.T6.e). */
  readonly bodyFixed: Vec3;
}

/** A light slot's state along a march: its absorber path, its throughput and its share of a step. */
interface SlotPath {
  readonly path: TwinAbsorberPath;
  readonly throughput: TwinRgb;
  readonly share: TwinRgb;
  readonly absorbed: TwinRgb;
}

/**
 * How a march's ray lies in the body-fixed axes: a point p of the ray's frame is
 * `originM` + p.x `x` + p.y `y` + p.z `z` (R08.T6.e).
 */
interface MarchFrame {
  readonly originM: Vec3;
  readonly x: Vec3;
  readonly y: Vec3;
  readonly z: Vec3;
}

/** The origin, for a direction taken into the body-fixed axes. */
const NO_OFFSET: Vec3 = { x: 0, y: 0, z: 0 };

/** The body-fixed axes themselves, about the body's centre: the ray march's frame. */
const BODY_FRAME: MarchFrame = {
  originM: NO_OFFSET,
  x: { x: 1, y: 0, z: 0 },
  y: { x: 0, y: 1, z: 0 },
  z: { x: 0, y: 0, z: 1 },
};

/** A point or direction of a march's frame in the body-fixed axes. */
function inBodyAxes(frame: MarchFrame, p: Vec3, withOrigin: boolean): Vec3 {
  const o = withOrigin ? frame.originM : NO_OFFSET;
  const { x, y, z } = frame;
  return {
    x: o.x + p.x * x.x + p.y * y.x + p.z * z.x,
    y: o.y + p.x * x.y + p.y * y.y + p.z * z.y,
    z: o.z + p.x * x.z + p.y * y.z + p.z * z.z,
  };
}

/**
 * What a march reads the per-planet tables through: a sphere's tables at a sample's height
 * ({@link TwinSources}, R05's and R08.T6's), or an oblate body's slices and bands at its
 * gravity-scaled height, each sample's latitude its own ({@link OblateMarch}, R08.T6.e).
 */
type MarchReads = { readonly kind: "sphere"; readonly sources: TwinSources } | OblateMarch;

/**
 * An oblate body's march (plan R08, Design note 17; R08.T6.e): its reads, the frame of its ray,
 * and which height the medium is read at, h\* = s h: the march's own geometry's (the sky view's and
 * the aerial perspective's osculating sphere R_α), or each sample's geodetic height above the datum
 * (the ray march's).
 */
interface OblateMarch {
  readonly kind: "oblate";
  readonly reader: OblateReader;
  readonly frame: MarchFrame;
  readonly heights: "geometry" | "geodetic";
}

/**
 * A per-frame march in progress: the accumulated radiance along one ray, summed over its sources,
 * and each light slot's throughput, slot 0's the march's transmittance (R08.T7).
 */
class Marcher {
  readonly luminance: TwinRgb = [0, 0, 0];
  readonly #terms: readonly TwinTerm[];
  readonly #pressureBarOf: ((heightM: number) => number) | null;
  readonly #reads: MarchReads;
  readonly #geometry: MarchGeometry;
  readonly #ray: TwinRay;
  readonly #marchSources: readonly MarchSource[];
  /** The light slots' paths in ascending order of slot, slot 0 first. */
  readonly #paths: ReadonlyMap<number, SlotPath>;
  readonly #absorbing: boolean;
  readonly #local = new Float64Array(SAMPLE_FLOATS);
  readonly #densities: Float64Array;
  readonly #toSun: TwinRgb = [0, 0, 0];
  readonly #multiple: TwinRgb = [0, 0, 0];
  readonly #phased: TwinRgb = [0, 0, 0];
  readonly #depth: TwinRgb = [0, 0, 0];
  /** Whether the march adds the forward lobes' higher orders: a tabulated term, and its depth read. */
  readonly #lobes: boolean;

  constructor(
    terms: readonly TwinTerm[],
    pressureBarOf: ((heightM: number) => number) | null,
    reads: MarchReads,
    geometry: MarchGeometry,
    ray: TwinRay,
  ) {
    this.#terms = terms;
    this.#pressureBarOf = pressureBarOf;
    this.#reads = reads;
    this.#geometry = geometry;
    this.#ray = ray;
    this.#densities = new Float64Array(terms.length);
    const d = ray.direction;
    const given: ReadonlyArray<TwinRaySource> = ray.sources ?? [
      { direction: ray.sun, weight: [1, 1, 1], slot: 0 },
    ];
    this.#absorbing = hasAbsorbers(terms);
    this.#marchSources = given.map((source) => {
      const s = source.direction;
      const cosTheta = d.x * s.x + d.y * s.y + d.z * s.z;
      const theta = Math.acos(Math.min(Math.max(cosTheta, -1), 1));
      return {
        direction: source.direction,
        weight: source.weight,
        slot: source.slot,
        phases: phasesAt(terms, cosTheta),
        entry: Math.sqrt(theta / Math.PI) * 255,
        bodyFixed: reads.kind === "oblate" ? inBodyAxes(reads.frame, s, false) : s,
      };
    });
    this.#lobes =
      terms.some((term) => term.lobe !== null) &&
      (reads.kind === "oblate" || reads.sources.sunOpticalDepth !== undefined);
    // A path a light slot under an absorber, slot 0's always (the views' mean transmittance), and
    // one path for every source of a medium with none, as `hillaire.ts`'s `frameSourcesOf` sends
    // them (`source.wgsl`'s `ViewMarch`).
    const last = Math.max(0, ...this.#marchSources.map((source) => this.#pathOf(source)));
    const paths = new Map<number, SlotPath>();
    for (let slot = 0; slot <= last; slot += 1) {
      paths.set(slot, {
        path: new TwinAbsorberPath(this.#absorbing, (columns, pressureColumns, out) => {
          if (reads.kind === "oblate") {
            reads.reader.absorberPathDepthInto(columns, pressureColumns, out, slot);
          } else {
            reads.sources.absorberPathDepth(columns, pressureColumns, out, slot);
          }
        }),
        throughput: [1, 1, 1],
        share: [0, 0, 0],
        absorbed: [0, 0, 0],
      });
    }
    this.#paths = paths;
  }

  /** The march's transmittance: light slot 0's throughput. */
  get throughput(): TwinRgb {
    return this.throughputOf(0);
  }

  /** A light slot's throughput, slot 0's for a slot with no path of its own. */
  throughputOf(slot: number): TwinRgb {
    const path = this.#paths.get(this.#absorbing ? slot : 0) ?? this.#paths.get(0);
    if (path === undefined) {
      throw new Error("a march has light slot 0's path");
    }
    return path.throughput;
  }

  /**
   * One step at distance `tM` along the ray, `dtM` long: the medium there, each source's
   * scattering source (`source.wgsl`'s `sourceAt`: single scattering of its light shadowed by the
   * ground on the tables' sphere, plus multiple scattering, both at its slot) weighted into its
   * slot's share, and each slot's share's in-scattering through its own path, which takes its
   * absorbers by its column under its slot's curves (`viewMarchStep`; R08.T6.c, R08.T7). On an
   * oblate body the medium and the tables are read at the sample's gravity-scaled height, its s at
   * its own latitude, and each source's transmittance on the sphere its own ray fits
   * ({@link OblateReader}, R08.T6.e).
   */
  step(tM: number, dtM: number): void {
    const { originM: o, direction: d } = this.#ray;
    const px = o.x + tM * d.x;
    const py = o.y + tM * d.y;
    const pz = o.z + tM * d.z;
    let heightM: number;
    // The sample's normal, unnormalised, and its length, which the sun cosines are taken against
    // as (s · g) ÷ |g|, the form R05's single sun took.
    let gx: number;
    let gy: number;
    let gz: number;
    let gLength: number;
    const geometry = this.#geometry;
    switch (geometry.kind) {
      case "sphere": {
        const rP = Math.sqrt(px * px + py * py + pz * pz);
        heightM = rP - geometry.bottomRadiusM;
        gx = px;
        gy = py;
        gz = pz;
        gLength = rP;
        break;
      }
      case "spheroid":
      case "gravitySpheroid": {
        const a2 = geometry.figure.equatorialRadiusM ** 2;
        const c2 = geometry.figure.polarRadiusM ** 2;
        const rP = Math.sqrt(px * px + py * py + pz * pz);
        const surfaceM =
          1 / Math.sqrt((px * px + py * py) / (rP * rP * a2) + (pz * pz) / (rP * rP * c2));
        heightM = rP - surfaceM;
        gx = px / a2;
        gy = py / a2;
        gz = pz / c2;
        gLength = Math.sqrt(gx * gx + gy * gy + gz * gz);
        break;
      }
    }
    const reads = this.#reads;
    if (reads.kind === "oblate") {
      this.#oblateStep(reads, { x: px, y: py, z: pz }, heightM, { x: gx, y: gy, z: gz }, dtM);
      return;
    }
    const local = this.#local;
    sampleInto(this.#terms, this.#pressureBarOf, null, heightM, local, this.#densities);
    const sources = reads.sources;
    for (const path of this.#paths.values()) {
      path.share.fill(0);
    }
    for (const source of this.#marchSources) {
      const s = source.direction;
      const muSun = (s.x * gx + s.y * gy + s.z * gz) / gLength;
      const slot = source.slot;
      const lit = groundShadows(sources.bottomRadiusM, heightM, muSun) ? 0 : 1;
      sources.sunTransmittance(heightM, muSun, this.#toSun, slot);
      sources.multiScattering(heightM, muSun, this.#multiple, slot);
      phasedInto(this.#terms, this.#densities, source.phases, this.#phased);
      if (this.#lobes) {
        sources.sunOpticalDepth?.(heightM, muSun, this.#depth);
        this.#addLobeOrders(source.entry, local, source.phases);
      }
      this.#addShare(source, lit);
    }
    this.#advance(dtM);
  }

  /**
   * {@link Marcher.step} on an oblate body (R08.T6.e): the sample's latitude, geodetic height and
   * datum normal from its body-fixed place, s there, the medium and the multiple scattering at
   * h\* = s h of the march's height (the geometry's, or the geodetic one), the latter at the
   * sample's own sun cosine against the geometry's normal `g`, and each source's transmittance and
   * shadow from the sliced lookup at the sample's body-fixed place, towards the source.
   */
  #oblateStep(reads: OblateMarch, p: Vec3, heightM: number, g: Vec3, dtM: number): void {
    const reader = reads.reader;
    const placeM = inBodyAxes(reads.frame, p, true);
    const geodetic = geodeticOf(placeM, reader.figure);
    const s = gravityRatio(reader.gravity, geodetic.latitudeRad);
    const geodeticHeights = reads.heights === "geodetic";
    const marchHeightM = geodeticHeights ? geodetic.heightM : heightM;
    const normal = geodeticHeights ? geodetic.normal : g;
    const normalLength = Math.sqrt(normal.x ** 2 + normal.y ** 2 + normal.z ** 2);
    const scaledHeightM = s * marchHeightM;
    const local = this.#local;
    sampleInto(this.#terms, this.#pressureBarOf, null, scaledHeightM, local, this.#densities);
    for (const path of this.#paths.values()) {
      path.share.fill(0);
    }
    // Each source's lookup starts where the march stands: its height and normal, the latter in the
    // body-fixed axes, at the sample's own geodetic latitude.
    const start: Geodetic = geodeticHeights
      ? geodetic
      : {
          ...geodetic,
          heightM: marchHeightM,
          normal: scale(inBodyAxes(reads.frame, normal, false), 1 / normalLength),
        };
    for (const source of this.#marchSources) {
      const v = geodeticHeights ? source.bodyFixed : source.direction;
      const muSun = (v.x * normal.x + v.y * normal.y + v.z * normal.z) / normalLength;
      const slot = source.slot;
      const lit = reader.sunInto(placeM, start, source.bodyFixed, slot, this.#toSun, this.#depth)
        ? 1
        : 0;
      reader.multiScatteringInto(scaledHeightM, s, muSun, slot, this.#multiple);
      phasedInto(this.#terms, this.#densities, source.phases, this.#phased);
      if (this.#lobes) {
        this.#addLobeOrders(source.entry, local, source.phases);
      }
      this.#addShare(source, lit);
    }
    this.#advance(dtM);
  }

  /** A source's single and multiple scattering at the step, weighted into its path's share. */
  #addShare(source: MarchSource, lit: number): void {
    const share = this.#paths.get(this.#pathOf(source))?.share;
    if (share === undefined) {
      throw new Error(`a march has no path for light slot ${source.slot}`);
    }
    const local = this.#local;
    for (const c of CHANNELS) {
      const one =
        lit * this.#toSun[c] * this.#phased[c] + this.#multiple[c] * (local[c] ?? Number.NaN);
      share[c] += source.weight[c] * one;
    }
  }

  /** Each slot's share's in-scattering over the step `dtM` through its own path. */
  #advance(dtM: number): void {
    const local = this.#local;
    for (const { path, throughput, share, absorbed } of this.#paths.values()) {
      path.stepInto(local, dtM, absorbed);
      for (const c of CHANNELS) {
        const stepDepth = (local[3 + c] ?? Number.NaN) * dtM + absorbed[c];
        this.luminance[c] += throughput[c] * share[c] * dtM * exactStepFactor(stepDepth);
        throughput[c] *= Math.exp(-stepDepth);
      }
    }
  }

  /**
   * Adds the tabulated terms' forward-lobe higher orders to the source's phased scattering
   * (`source.wgsl`'s `lobeOrdersOf`, R08.T7): each term's scattering times e^(x P) Ẽ(x, θ), x the
   * lobe terms' share of the sample's extinction times the stored optical depth along the source,
   * and beyond the table (e^(x P) − 1) times the term's phase at the angle (`phases`).
   */
  #addLobeOrders(entry: number, local: Float64Array, phases: Float64Array): void {
    const terms = this.#terms;
    const densities = this.#densities;
    for (const c of CHANNELS) {
      let lobeScattering = 0;
      for (let k = 0; k < terms.length; k += 1) {
        const term = terms[k];
        if (term !== undefined && term.lobe !== null) {
          lobeScattering += term.scattering[c] * (densities[k] ?? Number.NaN);
        }
      }
      const x = (this.#depth[c] * lobeScattering) / Math.max(local[3 + c] ?? Number.NaN, 1e-30);
      for (let k = 0; k < terms.length; k += 1) {
        const term = terms[k];
        const lobe = term?.lobe;
        if (term === undefined || lobe === null || lobe === undefined) {
          continue;
        }
        const scattered = term.scattering[c] * (densities[k] ?? Number.NaN);
        const boost = Math.exp(Math.min(x * lobe.share[c], 80));
        const phase = phases[k * 3 + c] ?? Number.NaN;
        this.#phased[c] += scattered * boost * forwardLobeAt(lobe, c, x, entry, phase);
      }
    }
  }

  /** The path a source's light takes: its slot under an absorber, slot 0 otherwise. */
  #pathOf(source: TwinRaySource): number {
    return this.#absorbing ? source.slot : 0;
  }
}

/**
 * A per-frame march of one ray over the given steps, as the sky-view, aerial-perspective and
 * ray-march kernels march each of theirs.
 *
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function marchTwin(
  medium: AtmosphereMedium,
  sources: TwinSources,
  geometry: MarchGeometry,
  ray: TwinRay,
  steps: readonly MarchStep[],
): TwinMarch {
  const marcher = new Marcher(
    termsOf(medium),
    pressureBarLookup(medium),
    { kind: "sphere", sources },
    geometry,
    ray,
  );
  for (const { tM, dtM } of steps) {
    marcher.step(tM, dtM);
  }
  return { luminance: [...marcher.luminance], transmittance: [...marcher.throughput] };
}

// --- The sky's irradiance (R08.T9.b) -------------------------------------------------------------

/** How the sky-irradiance twin integrates each texel's hemisphere (`irradianceQuadrature.ts`). */
export interface IrradianceOptions {
  readonly size: TableSize;
  /**
   * The steps along each direction's ray, at least 2, placed toward the texel's point and cut at
   * the medium's layer edges, as the multiple-scattering table's are.
   */
  readonly samples: number;
  /** Gauss–Legendre nodes in u = √(γ ÷ π) on each of the two pieces. */
  readonly ringNodes: number;
  /** Gauss–Legendre nodes in ψ on each ring's arc above the horizon. */
  readonly arcNodes: number;
}

/** The irradiance kernel's own: Bruneton's 64 × 16, its steps and its directions (`irradiance.wgsl`). */
export const IRRADIANCE_KERNEL_OPTIONS: IrradianceOptions = {
  size: IRRADIANCE_SIZE,
  samples: IRRADIANCE_SAMPLES,
  ringNodes: IRRADIANCE_RING_NODES,
  arcNodes: IRRADIANCE_ARC_NODES,
};

/** What a sky-irradiance texel reads: the medium in the build's frame and its tables' reads. */
interface IrradianceInputs {
  readonly terms: readonly TwinTerm[];
  readonly edgesM: ReadonlyArray<number>;
  readonly pressureBarOf: ((heightM: number) => number) | null;
  readonly shell: Shell;
  readonly sources: TwinSources;
  readonly curveSlot: number;
  readonly options: IrradianceOptions;
}

/**
 * One sky-irradiance texel (`irradiance.wgsl`'s): at its height h = x_r² H and sun zenith cosine
 * μ_s = 2 x_μ − 1 (Bruneton 2017's `GetRMuSFromIrradianceTextureUv`, in heights, but for the
 * height's root, which puts the rows near the ground), the sky's
 * radiance per unit illuminance along each of the hemisphere's directions, a per-frame march's
 * ({@link Marcher}: single scattering of each term's phase, shadowed by the ground, the forward
 * lobes' higher orders and the multiple scattering, through the ray's own throughput under the light
 * slot's curves), weighted by its quadrature weight and μ_v and summed.
 */
function irradianceTexelInto(inputs: IrradianceInputs, x: number, y: number, out: TwinRgb): void {
  const { widthTexels: w, heightTexels: h } = inputs.options.size;
  const { bottomRadiusM: bottom, topRadiusM: top } = inputs.shell;
  const root = unitOfTexel(y, h);
  const heightM = root * root * (top - bottom);
  const muSun = unitOfTexel(x, w) * 2 - 1;
  const across = Math.sqrt(Math.max((1 - muSun) * (1 + muSun), 0));
  const sun = { x: 0, y: across, z: muSun };
  const originM = { x: 0, y: 0, z: bottom + heightM };
  const geometry: MarchGeometry = { kind: "sphere", bottomRadiusM: bottom };
  out.fill(0);
  const directions = irradianceDirections(muSun, inputs.options.ringNodes, inputs.options.arcNodes);
  for (const { direction, weight } of directions) {
    if (!(weight > 0)) {
      continue;
    }
    const segment: TwinSegment = {
      originM,
      direction,
      tStartM: 0,
      tEndM: maxDistanceM(inputs.shell, bottom + heightM, direction.z),
      fromCamera: true,
    };
    const marcher = new Marcher(
      inputs.terms,
      inputs.pressureBarOf,
      { kind: "sphere", sources: inputs.sources },
      geometry,
      { ...segment, sun, sources: [{ direction: sun, weight: [1, 1, 1], slot: inputs.curveSlot }] },
    );
    for (const { tM, dtM } of piecedSteps(
      segment,
      inputs.options.samples,
      inputs.edgesM,
      geometry,
    )) {
      marcher.step(tM, dtM);
    }
    for (const c of CHANNELS) {
      out[c] += weight * marcher.luminance[c];
    }
  }
}

/**
 * The sky-irradiance table's twin (plan R08, R08.T9.b; Design notes 7 and 19): at each texel's
 * height and sun zenith cosine, Bruneton 2017's layout (64 in μ_s by 16 in height, his texel
 * mapping; the height's root over the atmosphere's in place of his linear axis), the sky's diffuse irradiance on a horizontal surface per unit of the sun's illuminance,
 * E = ∫ L μ_v dω over the hemisphere above the horizon (`irradianceQuadrature.ts`), as
 * `irradiance.wgsl` stores it in rgb, alpha 0. It reads `transmittance` and the multiple-scattering
 * layers as the per-frame marches read a sphere's tables (`tableSources`), with light slot
 * `curveSlot`'s curves and multiple-scattering layer.
 *
 * @param bottomRadiusM - The tables' ground radius, m: R_ref, or a latitude band's sphere.
 * @param multiScatteringBySlot - The multiple-scattering table's layers, one a light slot.
 * @param texels - The texels to compute, each [x, y]; every texel by default. Texels not listed
 *   are left 0.
 * @param coefficientFactor - The factor every coefficient takes: 1 ÷ s_b in a latitude band's own
 *   frame ({@link twinBandOf}), 1 otherwise.
 * @throws RangeError as {@link multiScatteringTwin}.
 */
export function irradianceTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  transmittance: TwinTransmittance,
  multiScatteringBySlot: ReadonlyArray<TwinTable>,
  options: IrradianceOptions = IRRADIANCE_KERNEL_OPTIONS,
  texels?: ReadonlyArray<readonly [number, number]>,
  curveSlot = SHARED_TABLE_CURVE_SLOT,
  coefficientFactor = 1,
): TwinTable {
  const first = multiScatteringBySlot[0];
  if (first === undefined) {
    throw new RangeError("the sky's irradiance reads at least one multiple-scattering layer");
  }
  const terms = scaledTerms(termsOf(medium), coefficientFactor);
  const inputs: IrradianceInputs = {
    terms,
    edgesM: marchEdgesOf(terms, medium.topHeightM),
    pressureBarOf: pressureBarLookup(medium),
    shell: { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM },
    sources: tableSources({
      medium,
      bottomRadiusM,
      ...transmittance,
      multiScattering: first,
      multiScatteringBySlot,
    }),
    curveSlot,
    options,
  };
  const { widthTexels: w, heightTexels: h } = options.size;
  const out = new Float64Array(w * h * 4);
  const which =
    texels ?? Array.from({ length: w * h }, (_, i) => [i % w, Math.floor(i / w)] as const);
  const value: TwinRgb = [0, 0, 0];
  for (const [x, y] of which) {
    irradianceTexelInto(inputs, x, y, value);
    out.set([value[0], value[1], value[2], 0], (y * w + x) * 4);
  }
  return { widthTexels: w, heightTexels: h, texels: out };
}

/**
 * A frame's view as the per-frame kernels take it (`view.wgsl`'s `AtmosphereView`, filled as
 * `HillaireAtmosphere` fills it), in the body-fixed axes, m.
 */
export interface TwinView {
  /** The camera's position from the body's centre. */
  readonly cameraM: Vec3;
  /** The camera's geodetic height above the datum. */
  readonly heightM: number;
  /** The datum's outward normal under the camera. */
  readonly normal: Vec3;
  /** The camera's Gaussian radius √(MN): the ground of the sphere the sky view is built on. */
  readonly cameraRadiusM: number;
  /**
   * The unit direction to the first source, or the zenith where none is drawn: what a twilight
   * ray's class is judged by (`marchSteps.ts`'s `grazingTwilight`).
   */
  readonly sun: Vec3;
  /**
   * The drawn sources, body-fixed, each light weighted as the per-view tables hold it (R08.T7):
   * by its illuminance times the frame's scale ({@link frameTwinView}), or by 1 for one sun's
   * light per unit of its illuminance ({@link twinView}).
   */
  readonly sources: ReadonlyArray<TwinRaySource>;
  /** The sky view's x axis, the first source's horizontal direction (`hillaire.ts`'s `skyAxisOf`). */
  readonly skyAxis: Vec3;
  /** The ray basis: right × tan(fov_x ÷ 2), up × tan(fov_y ÷ 2), and forward. */
  readonly right: Vec3;
  readonly upRay: Vec3;
  readonly forward: Vec3;
  readonly figure: SpheroidFigure;
  /** The projection's near plane, for distances from depth. */
  readonly nearM: number;
  readonly viewport: ViewSize;
}

/** The view's camera and ray basis, which every {@link TwinView} shares. */
function twinCamera(
  camera: AtmosphereCamera,
  figure: SpheroidFigure,
  nearM: number,
): Omit<TwinView, "sun" | "sources" | "skyAxis"> {
  const inputs = atmosphereInputs(camera, figure);
  const tanX = Math.tan(camera.fovXRad / 2);
  const tanY = tanX / (camera.viewport.widthPx / camera.viewport.heightPx);
  return {
    cameraM: camera.positionM,
    heightM: inputs.heightM,
    normal: inputs.normal,
    cameraRadiusM: inputs.radiusM - inputs.heightM,
    right: scale(rotate(camera.orientation, { x: 1, y: 0, z: 0 }), tanX),
    upRay: scale(rotate(camera.orientation, { x: 0, y: 1, z: 0 }), tanY),
    forward: rotate(camera.orientation, { x: 0, y: 0, z: -1 }),
    figure,
    nearM,
    viewport: camera.viewport,
  };
}

/**
 * A frame's view from its camera and one sun, its light per unit of its illuminance in each
 * channel (weight 1) at its light slot: the tables R05 and R08.T6 stored, against which a test
 * compares one source's sky.
 */
export function twinView(
  camera: AtmosphereCamera,
  sun: SunState,
  figure: SpheroidFigure,
  nearM: number,
): TwinView {
  const base = twinCamera(camera, figure, nearM);
  return {
    ...base,
    sun: sun.directionBodyFixed,
    sources: [{ direction: sun.directionBodyFixed, weight: [1, 1, 1], slot: curveSlotOf(sun) }],
    skyAxis: skyAxisOf(base.normal, [sun]),
  };
}

/**
 * A frame's view as `HillaireAtmosphere.drawFrame` fills its uniform for `suns` (R08.T7): each
 * source's light weighted by its illuminance times `exposureScale`, the frame's pre-exposure, as
 * `hillaire.ts`'s `frameSourcesOf` forms it, so that the twins hold what the per-view tables hold.
 *
 * @param absorbers - Whether the medium has an absorber, and `lightSlots` its curves' lights.
 * @throws RangeError as `frameSourcesOf`.
 */
export function frameTwinView(
  camera: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
  figure: SpheroidFigure,
  nearM: number,
  exposureScale: number,
  absorbers: boolean,
  lightSlots: number,
): TwinView {
  const base = twinCamera(camera, figure, nearM);
  const weighted = frameSourcesOf(suns, exposureScale, absorbers, lightSlots);
  return {
    ...base,
    sun: suns[0]?.directionBodyFixed ?? base.normal,
    sources: suns.map((sun, i) => {
      const w = weighted[i]?.weight ?? [0, 0, 0, 0];
      return {
        direction: sun.directionBodyFixed,
        weight: [w[0], w[1], w[2]],
        slot: curveSlotOf(sun),
      };
    }),
    skyAxis: skyAxisOf(base.normal, suns),
  };
}

/** The unnormalised ray through a point of the output, (0, 0) at its top left (`rayThrough`). */
function unnormalisedRay(view: TwinView, u: number, v: number): Vec3 {
  const nx = u * 2 - 1;
  const ny = 1 - v * 2;
  const { forward: f, right: r, upRay: up } = view;
  return {
    x: f.x + nx * r.x + ny * up.x,
    y: f.y + nx * r.y + ny * up.y,
    z: f.z + nx * r.z + ny * up.z,
  };
}

/** A vector's unit direction. */
function unit(v: Vec3): Vec3 {
  const length = Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
  return { x: v.x / length, y: v.y / length, z: v.z / length };
}

/**
 * The ray of the sky-view texel (x, y), as `skyView.wgsl` marches it: sebh's zenith
 * parameterisation (`view.wgsl`'s `skyViewUvToParams`) on the camera's own sphere, the camera kept
 * 1 m inside the shell, and the local azimuth φ = 2π u from the view's sky axis over the whole
 * circle (R08.T7), in the frame whose z is the datum's normal, x the sky axis and y = z × x, the
 * view's sources taken into the same frame.
 */
export function skyViewRayTwin(
  medium: AtmosphereMedium,
  view: TwinView,
  x: number,
  y: number,
  size: TableSize,
): TwinRay {
  const bottom = view.cameraRadiusM;
  const atmosphereM = medium.topHeightM;
  const heightM = skyViewHeight(view.heightM, atmosphereM);
  const rM = bottom + heightM;
  const { widthTexels: w, heightTexels: h } = size;
  const unitX = unitOfTexel(x, w);
  const unitY = unitOfTexel(y, h);
  const vHorizon = Math.sqrt(Math.max(heightM * (2 * bottom + heightM), 0));
  const beta = Math.acos(Math.min(Math.max(vHorizon / rM, -1), 1));
  const zenithHorizon = Math.PI - beta;
  let viewZenithCos: number;
  if (unitY < 0.5) {
    const c = 1 - 2 * unitY;
    viewZenithCos = Math.cos(zenithHorizon * (1 - c * c));
  } else {
    const c = unitY * 2 - 1;
    viewZenithCos = Math.cos(zenithHorizon + beta * c * c);
  }
  const phi = 2 * Math.PI * unitX;
  const viewZenithSin = Math.sqrt(Math.max(1 - viewZenithCos * viewZenithCos, 0));
  const n = view.normal;
  const ax = view.skyAxis;
  const ay = {
    x: n.y * ax.z - n.z * ax.y,
    y: n.z * ax.x - n.x * ax.z,
    z: n.x * ax.y - n.y * ax.x,
  };
  const inFrame = (s: Vec3): Vec3 => ({
    x: s.x * ax.x + s.y * ax.y + s.z * ax.z,
    y: s.x * ay.x + s.y * ay.y + s.z * ay.z,
    z: s.x * n.x + s.y * n.y + s.z * n.z,
  });
  const sources = view.sources.map((source) => ({
    ...source,
    direction: inFrame(source.direction),
  }));
  const shell = { bottomRadiusM: bottom, topRadiusM: bottom + atmosphereM };
  return {
    originM: { x: 0, y: 0, z: rM },
    direction: {
      x: viewZenithSin * Math.cos(phi),
      y: viewZenithSin * Math.sin(phi),
      z: viewZenithCos,
    },
    sun: sources[0]?.direction ?? { x: 0, y: 0, z: 1 },
    sources,
    tStartM: 0,
    tEndM: maxDistanceM(shell, rM, viewZenithCos),
    fromCamera: true,
  };
}

/**
 * The sky-view table's twin: each texel's ray marched at `samples` placed steps (at least 2) on
 * the camera's own sphere, cut at the medium's layer edges ({@link piecedSteps}, R08.T6.i), reading
 * the planet's tables at each sample's height, alpha 1.
 *
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function skyViewTwin(
  planet: PlanetTwin,
  view: TwinView,
  size: TableSize,
  samples: number,
): TwinTable {
  const texelAt = skyViewTexelsTwin(planet, view, size, samples);
  const { widthTexels: w, heightTexels: h } = size;
  const texels = new Float64Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      texels.set([...texelAt(x, y), 1], (y * w + x) * 4);
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

/**
 * The sky-view table's twin texel by texel, each marched when first asked for and kept
 * ({@link skyViewTwin}'s march): what a read of a few directions takes without the whole table.
 *
 * @throws RangeError as {@link skyViewTwin}.
 */
export function skyViewTexelsTwin(
  planet: PlanetTwin,
  view: TwinView,
  size: TableSize,
  samples: number,
): (x: number, y: number) => Rgb {
  const terms = termsOf(planet.medium);
  const edgesM = marchEdgesOf(terms, planet.medium.topHeightM);
  const pressureBarOf = pressureBarLookup(planet.medium);
  const sources = tableSources(planet);
  const geometry: MarchGeometry = { kind: "sphere", bottomRadiusM: view.cameraRadiusM };
  const n = Math.max(Math.floor(samples), 2);
  const kept = new Map<number, Rgb>();
  return (x, y) => {
    const key = y * size.widthTexels + x;
    let texel = kept.get(key);
    if (texel === undefined) {
      const ray = skyViewRayTwin(planet.medium, view, x, y, size);
      const marcher = new Marcher(terms, pressureBarOf, { kind: "sphere", sources }, geometry, ray);
      for (const { tM, dtM } of piecedSteps(ray, n, edgesM, geometry)) {
        marcher.step(tM, dtM);
      }
      const [r, g, b] = marcher.luminance;
      texel = [r, g, b];
      kept.set(key, texel);
    }
    return texel;
  };
}

/**
 * The sky view read in a direction as the composite reads it (`composite.wgsl`'s sky branch, R08.T7):
 * the ray's zenith angle and local azimuth about the view's sky axis to the table's unit range
 * (`view.wgsl`'s `skyViewParamsToUv`, its horizon split at the camera's sphere), through Bruneton's
 * texel mapping, and the four texels about it filtered bilinearly as the composite's linear,
 * clamped sampler does, texel centres at (i + ½) ÷ size.
 *
 * @param texelAt - The table's texel (x, y): a table's ({@link twinTableTexels}) or one computed on
 *   demand ({@link skyViewTexelsTwin}).
 * @param direction - A unit direction, body-fixed.
 */
export function skyViewReadTwin(
  texelAt: (x: number, y: number) => Rgb,
  size: TableSize,
  medium: AtmosphereMedium,
  view: TwinView,
  direction: Vec3,
): Rgb {
  return skyViewReadOn(texelAt, size, view.cameraRadiusM, medium.topHeightM, view, direction);
}

/**
 * {@link skyViewReadTwin}'s read on the sphere of ground radius `bottomRadiusM`, m, under an
 * atmosphere `atmosphereM` high: the camera's own sphere, or an oblate body's R_α (R08.T6.e).
 */
function skyViewReadOn(
  texelAt: (x: number, y: number) => Rgb,
  size: TableSize,
  bottomRadiusM: number,
  atmosphereM: number,
  view: TwinView,
  direction: Vec3,
): Rgb {
  const heightM = skyViewHeight(view.heightM, atmosphereM);
  const rM = bottomRadiusM + heightM;
  const n = view.normal;
  const ax = view.skyAxis;
  const ay = {
    x: n.y * ax.z - n.z * ax.y,
    y: n.z * ax.x - n.x * ax.z,
    z: n.x * ax.y - n.y * ax.x,
  };
  const d = direction;
  const mu = d.x * n.x + d.y * n.y + d.z * n.z;
  const across = {
    x: d.y * n.z - d.z * n.y,
    y: d.z * n.x - d.x * n.z,
    z: d.x * n.y - d.y * n.x,
  };
  const zenith = Math.atan2(Math.hypot(across.x, across.y, across.z), mu);
  const vHorizon = Math.sqrt(Math.max(heightM * (2 * bottomRadiusM + heightM), 0));
  const beta = Math.acos(Math.min(Math.max(vHorizon / rM, -1), 1));
  const zenithHorizon = Math.PI - beta;
  const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + atmosphereM };
  let v: number;
  if (!intersectsGround(shell, rM, mu)) {
    v = (1 - Math.sqrt(Math.max(1 - zenith / zenithHorizon, 0))) * 0.5;
  } else {
    v = Math.sqrt(Math.max((zenith - zenithHorizon) / Math.max(beta, 1e-6), 0)) * 0.5 + 0.5;
  }
  const x = d.x * ax.x + d.y * ax.y + d.z * ax.z;
  const y = d.x * ay.x + d.y * ay.y + d.z * ay.z;
  let phi = x === 0 && y === 0 ? 0 : Math.atan2(y, x);
  if (phi < 0) {
    phi += 2 * Math.PI;
  }
  const { widthTexels: w, heightTexels: h } = size;
  const px = Math.min(
    Math.max(texelUvFromUnit(unitClamp(phi / (2 * Math.PI)), w) * w - 0.5, 0),
    w - 1,
  );
  const py = Math.min(Math.max(texelUvFromUnit(unitClamp(v), h) * h - 0.5, 0), h - 1);
  const x0 = Math.floor(px);
  const y0 = Math.floor(py);
  const x1 = Math.min(x0 + 1, w - 1);
  const y1 = Math.min(y0 + 1, h - 1);
  const fx = px - x0;
  const fy = py - y0;
  const a = texelAt(x0, y0);
  const b = texelAt(x1, y0);
  const c = texelAt(x0, y1);
  const e = texelAt(x1, y1);
  const out: TwinRgb = [0, 0, 0];
  for (const k of CHANNELS) {
    const top = a[k] * (1 - fx) + b[k] * fx;
    const below = c[k] * (1 - fx) + e[k] * fx;
    out[k] = top * (1 - fy) + below * fy;
  }
  return out;
}

/** A table's texel (x, y)'s RGB, for {@link skyViewReadTwin}. */
export function twinTableTexels(table: TwinTable): (x: number, y: number) => Rgb {
  return (x, y) => {
    const at = (y * table.widthTexels + x) * 4;
    const t = table.texels;
    return [t[at] ?? Number.NaN, t[at + 1] ?? Number.NaN, t[at + 2] ?? Number.NaN];
  };
}

/**
 * The aerial-perspective volume's twin: for each froxel column, the running in-scattered radiance
 * in the view's sources' weights (rgb; R08.T7) and the mean transmittance (a) at each slice's far face, out to
 * `reachM` in slices of equal depth with `samplesPerSlice` even steps each (at least 1), on the
 * camera's own sphere, as `aerialPerspective.wgsl` stores them, froxel (x, y) along the ray through
 * the screen at (x, y) ÷ (size − 1), Bruneton's texel mapping (plan R08, Design note 19).
 *
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function aerialPerspectiveTwin(
  planet: PlanetTwin,
  view: TwinView,
  size: VolumeSize,
  samplesPerSlice: number,
  reachM: number,
): TwinVolume {
  const terms = termsOf(planet.medium);
  const pressureBarOf = pressureBarLookup(planet.medium);
  const sources = tableSources(planet);
  const bottom = view.cameraRadiusM;
  const geometry: MarchGeometry = { kind: "sphere", bottomRadiusM: bottom };
  const { widthTexels: w, heightTexels: h, slices } = size;
  const perSlice = Math.max(Math.floor(samplesPerSlice), 1);
  const dtM = reachM / (slices * perSlice);
  const lift = bottom + Math.max(view.heightM, 0);
  const n = view.normal;
  const originM = { x: n.x * lift, y: n.y * lift, z: n.z * lift };
  const texels = new Float64Array(w * h * slices * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      // Bruneton's mapping on the screen axes (Design note 19): froxel (x, y) at (x, y) ÷ (size − 1).
      const direction = unit(unnormalisedRay(view, unitOfTexel(x, w), unitOfTexel(y, h)));
      const ray: TwinRay = {
        originM,
        direction,
        sun: view.sun,
        sources: view.sources,
        tStartM: 0,
        tEndM: reachM,
        fromCamera: true,
      };
      const marcher = new Marcher(terms, pressureBarOf, { kind: "sphere", sources }, geometry, ray);
      for (let slice = 0; slice < slices; slice += 1) {
        for (let step = 0; step < perSlice; step += 1) {
          marcher.step((slice * perSlice + step + 0.5) * dtM, dtM);
        }
        const t = marcher.throughput;
        texels.set([...marcher.luminance, (t[0] + t[1] + t[2]) / 3], ((slice * h + y) * w + x) * 4);
      }
    }
  }
  return { widthTexels: w, heightTexels: h, slices, texels };
}

/** The near and far distances along a ray to a spheroid of radii (a, a, c), or (−1, −1) if missed. */
function spheroidHits(o: Vec3, d: Vec3, a: number, c: number): readonly [number, number] {
  const k = a / c;
  const qa = d.x * d.x + d.y * d.y + d.z * k * (d.z * k);
  const qb = o.x * d.x + o.y * d.y + o.z * k * (d.z * k);
  const qc = o.x * o.x + o.y * o.y + o.z * k * (o.z * k) - a * a;
  const disc = qb * qb - qa * qc;
  if (disc < 0) {
    return [-1, -1];
  }
  const root = Math.sqrt(disc);
  return [(-qb - root) / qa, (-qb + root) / qa];
}

/**
 * A ray-march texel's ray, as `rayMarch.wgsl` clips it: against the datum spheroid's shells, a +
 * top and c + top, ending at the ground or at the surface the depth puts it on. `null` where the
 * kernel marches nothing: the camera inside the atmosphere and the surface within the
 * aerial-perspective reach or none, or a ray that misses the atmosphere.
 */
export interface RayMarchRay {
  readonly ray: TwinRay;
  /** The distance to the ground the ray meets with nothing drawn in front of it, m, or `null`. */
  readonly bareGroundM: number | null;
}

/**
 * The ray of the ray-march texel at `u`, `v` of the target, where the scene's reversed-Z depth is
 * `depth` (0 where nothing is drawn).
 *
 * @param reachM - The aerial-perspective volume's reach, m, beyond which the march serves surfaces.
 */
export function rayMarchRayTwin(
  medium: AtmosphereMedium,
  view: TwinView,
  u: number,
  v: number,
  depth: number,
  reachM: number,
): RayMarchRay | null {
  const topM = medium.topHeightM;
  const aboveTop = view.heightM >= topM;
  const unnormalised = unnormalisedRay(view, u, v);
  const lengthM = Math.sqrt(unnormalised.x ** 2 + unnormalised.y ** 2 + unnormalised.z ** 2);
  const tSurfaceM = depth > 0 ? (view.nearM / depth) * lengthM : Number.POSITIVE_INFINITY;
  if (!(aboveTop || (depth > 0 && tSurfaceM > reachM))) {
    return null;
  }
  return rayMarchRayAlongTwin(medium, view, view.cameraM, unit(unnormalised), tSurfaceM);
}

/**
 * A ray from `originM` along the unit `direction`, clipped as `rayMarch.wgsl` clips a texel's ray:
 * against the datum spheroid's shells, ending at the ground or at a surface `tSurfaceM` along it
 * (infinite where nothing is drawn). `null` where it misses the atmosphere. {@link rayMarchRayTwin}
 * takes the camera's ray through a texel; the smoke takes it for a texel's ray turned or moved by
 * the kernel's own resolution (R08.T6.g, `decision-r08-f32.md` §10).
 */
export function rayMarchRayAlongTwin(
  medium: AtmosphereMedium,
  view: TwinView,
  originM: Vec3,
  direction: Vec3,
  tSurfaceM: number,
): RayMarchRay | null {
  const topM = medium.topHeightM;
  const o = originM;
  const a = view.figure.equatorialRadiusM;
  const c = view.figure.polarRadiusM;
  const [shellNear, shellFar] = spheroidHits(o, direction, a + topM, c + topM);
  const [groundNear] = spheroidHits(o, direction, a, c);
  const tStartM = Math.max(shellNear, 0);
  const hitsGround = groundNear > 0;
  const tEndM = Math.min(hitsGround ? Math.min(shellFar, groundNear) : shellFar, tSurfaceM);
  if (shellFar <= 0 || tEndM <= tStartM) {
    return null;
  }
  return {
    ray: {
      originM: o,
      direction,
      sun: view.sun,
      sources: view.sources,
      tStartM,
      tEndM,
      fromCamera: shellNear <= 0,
    },
    bareGroundM:
      !Number.isFinite(tSurfaceM) && hitsGround && groundNear <= tEndM ? groundNear : null,
  };
}

/** The ray march's settings: `TableSizes`' scale, steps and reach. */
export type RayMarchSettings = Pick<
  TableSizes,
  "rayMarchScale" | "rayMarchSamples" | "aerialPerspectiveReachM"
>;

/**
 * One ray-march texel's twin along a clipped ray (`rayMarchRayTwin`, `rayMarchRayAlongTwin`): its
 * in-scattered radiance in the view's sources' weights (R08.T7) and mean transmittance, with the bare ground
 * seen from above lit by the sun, and (0, 0, 0, 1) where the kernel marches nothing (`null`), its
 * placed steps cut at the medium's layer edges on their shell quadrics ({@link piecedSteps},
 * R08.T6.i). The smoke marches a texel's ray again, turned or moved, through it (R08.T6.g).
 *
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function rayMarchTexelTwin(
  planet: PlanetTwin,
  view: TwinView,
  settings: RayMarchSettings,
  marched: RayMarchRay | null,
): readonly [number, number, number, number] {
  const out = new Float64Array(4);
  rayMarchTexelInto(rayMarchContext(planet, view, settings), marched, out);
  return [out[0] ?? Number.NaN, out[1] ?? Number.NaN, out[2] ?? Number.NaN, out[3] ?? Number.NaN];
}

/** What every ray-march texel of a view reads. */
interface RayMarchContext {
  readonly medium: AtmosphereMedium;
  readonly terms: readonly TwinTerm[];
  /** The marches' layer edges of the terms ({@link marchEdgesOf}), m. */
  readonly edgesM: ReadonlyArray<number>;
  readonly pressureBarOf: ((heightM: number) => number) | null;
  /** A sphere's table reads, or an oblate body's slices and bands (R08.T6.e). */
  readonly reads: MarchReads;
  readonly geometry: MarchGeometry;
  readonly view: TwinView;
  readonly samples: number;
}

function rayMarchContext(
  planet: PlanetTwin,
  view: TwinView,
  settings: RayMarchSettings,
): RayMarchContext {
  const terms = termsOf(planet.medium);
  return {
    medium: planet.medium,
    terms,
    edgesM: marchEdgesOf(terms, planet.medium.topHeightM),
    pressureBarOf: pressureBarLookup(planet.medium),
    reads: { kind: "sphere", sources: tableSources(planet) },
    geometry: { kind: "spheroid", figure: view.figure },
    view,
    samples: Math.max(Math.floor(settings.rayMarchSamples), 2),
  };
}

/** One ray-march texel into `out`, RGBA: `rayMarch.wgsl`'s `main` along its clipped ray. */
function rayMarchTexelInto(
  context: RayMarchContext,
  marched: RayMarchRay | null,
  out: Float64Array,
): void {
  const { medium, terms, edgesM, pressureBarOf, reads, geometry, view, samples } = context;
  if (marched === null) {
    out.set([0, 0, 0, 1]);
    return;
  }
  const { ray, bareGroundM } = marched;
  const marcher = new Marcher(terms, pressureBarOf, reads, geometry, ray);
  for (const { tM, dtM } of piecedSteps(ray, samples, edgesM, geometry)) {
    marcher.step(tM, dtM);
  }
  const luminance = marcher.luminance;
  const t = marcher.throughput;
  let meanTransmittance = (t[0] + t[1] + t[2]) / 3;
  if (bareGroundM !== null) {
    const o = ray.originM;
    const d = ray.direction;
    const groundM = {
      x: o.x + bareGroundM * d.x,
      y: o.y + bareGroundM * d.y,
      z: o.z + bareGroundM * d.z,
    };
    const toSun: TwinRgb = [0, 0, 0];
    // Each source's light on it, weighted as the sky is, through its own slot's throughput (R08.T7).
    const lights = ray.sources ?? [{ direction: ray.sun, weight: [1, 1, 1], slot: 0 }];
    for (const light of lights) {
      const muSun = groundLightInto(reads, view.figure, groundM, light, toSun);
      const through = marcher.throughputOf(light.slot);
      for (const c of CHANNELS) {
        luminance[c] +=
          (through[c] * light.weight[c] * toSun[c] * Math.max(muSun, 0) * medium.groundAlbedo[c]) /
          Math.PI;
      }
    }
    meanTransmittance = 0;
  }
  out.set([...luminance, meanTransmittance]);
}

/**
 * The transmittance into `out` of a source's light to the bare ground at the body-fixed `groundM`,
 * and the source's cosine there against the datum's normal, which the ground's Lambertian
 * reflection takes: the table at the ground, or on an oblate body the sliced lookup from the
 * ground's own latitude (R08.T6.e).
 */
function groundLightInto(
  reads: MarchReads,
  figure: SpheroidFigure,
  groundM: Vec3,
  light: TwinRaySource,
  out: TwinRgb,
): number {
  const s = light.direction;
  if (reads.kind === "oblate") {
    const geodetic = geodeticOf(groundM, figure);
    const n = geodetic.normal;
    reads.reader.sunInto(groundM, geodetic, s, light.slot, out, [0, 0, 0]);
    return s.x * n.x + s.y * n.y + s.z * n.z;
  }
  const a2 = figure.equatorialRadiusM ** 2;
  const c2 = figure.polarRadiusM ** 2;
  const nx = groundM.x / a2;
  const ny = groundM.y / a2;
  const nz = groundM.z / c2;
  const length = Math.sqrt(nx * nx + ny * ny + nz * nz);
  const muSun = (s.x * nx + s.y * ny + s.z * nz) / length;
  reads.sources.sunTransmittance(0, muSun, out, light.slot);
  return muSun;
}

/**
 * The ray-march target's twin at the setting's scale of the viewport: each texel's in-scattered
 * radiance in the view's sources' weights (rgb; R08.T7) and mean transmittance (a), with the bare ground seen
 * from above lit by the sun, and (0, 0, 0, 1) where the kernel marches nothing.
 *
 * @param depthAt - The scene's reversed-Z depth at a full-resolution pixel, 0 where nothing is
 *   drawn: the texel the kernel loads.
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function rayMarchTwin(
  planet: PlanetTwin,
  view: TwinView,
  settings: RayMarchSettings,
  depthAt: (xPx: number, yPx: number) => number,
): TwinTable {
  const context = rayMarchContext(planet, view, settings);
  const fullW = view.viewport.widthPx;
  const fullH = view.viewport.heightPx;
  const w = Math.ceil(fullW * settings.rayMarchScale);
  const h = Math.ceil(fullH * settings.rayMarchScale);
  const texels = new Float64Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const u = (x + 0.5) / w;
      const v = (y + 0.5) / h;
      const depth = depthAt(
        Math.min(Math.floor(u * fullW), fullW - 1),
        Math.min(Math.floor(v * fullH), fullH - 1),
      );
      const marched = rayMarchRayTwin(
        planet.medium,
        view,
        u,
        v,
        depth,
        settings.aerialPerspectiveReachM,
      );
      rayMarchTexelInto(context, marched, texels.subarray((y * w + x) * 4, (y * w + x) * 4 + 4));
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

// --- Oblate bodies: the slices and the bands (R08.T6.e) ------------------------------------------

/** A body's datum and gravity as the oblate twin reads them (plan R08, Design note 17). */
export interface TwinBody {
  /** The datum spheroid: R07's `BodyFigure`. */
  readonly figure: SpheroidFigure;
  /** R08.T3.d's `bodyGravity` of the body, its slicing included. */
  readonly gravity: BodyGravity;
}

/** The sizes and steps the oblate twin builds its slices and bands at. */
export interface OblateTwinSizes {
  readonly transmittance: TableSize;
  /** The steps along each transmittance texel's ray: the kernel's {@link TRANSMITTANCE_SAMPLES}. */
  readonly transmittanceSamples: number;
  readonly multiScattering: MultiScatteringOptions;
}

/** The kernels' own sizes and steps, as {@link planetTwin} builds its one slice and band. */
export const OBLATE_KERNEL_SIZES: OblateTwinSizes = {
  transmittance: TRANSMITTANCE_SIZE,
  transmittanceSamples: TRANSMITTANCE_SAMPLES,
  multiScattering: MULTI_SCATTERING_KERNEL_OPTIONS,
};

/**
 * One latitude band's per-planet tables (plan R08, Design note 17, _Latitude bands_): an ordinary
 * build at the band's g(φ) and √(MN), in its own gravity-scaled frame, so that its rows stand at
 * h\* = s h with the reference medium's top.
 *
 * @remarks
 * A band at s is the reference medium read at h\* = s h on lengths l; on lengths l\* = s l the same
 * air is the reference medium with every coefficient divided by s, on a sphere of radius s √(MN)
 * ({@link scaledTerms}). Its multiple-scattering table holds Hillaire's Ψ, which that change of
 * length does not move, against which a march multiplies its own scattering per metre. A body of
 * one band takes R05's per-planet build as it is, at s = 1 on R_ref (Design note 17).
 */
export interface TwinBand {
  /** s_b. */
  readonly gravityRatio: number;
  /** The geodetic latitude whose s is the band's, rad (`oblate.ts`'s `bandLatitudeRad`). */
  readonly latitudeRad: number;
  /**
   * The ground of the band's sphere in its gravity-scaled frame, m: s_b √(MN) at its latitude, or
   * R_ref for a body of one band.
   */
  readonly bottomRadiusM: number;
  /** Its own transmittance tables, the reference medium on its sphere with optical depth ÷ s_b. */
  readonly transmittance: TwinTransmittance;
  /** Its multiple-scattering table, one layer a light slot as {@link planetTwin}'s. */
  readonly multiScatteringBySlot: ReadonlyArray<TwinTable>;
}

/**
 * An oblate body's per-planet tables in `f64` (plan R08, Design note 17; R08.T6.e): the
 * transmittance slices over κ = s R_α ÷ R_ref and the multiple-scattering bands over s of its
 * slicing, each with Design note 19's texel mapping, as one-slice tables take it.
 */
export interface OblatePlanetTwin {
  readonly medium: AtmosphereMedium;
  readonly body: TwinBody;
  /** R_ref, m: R05's `tableRadiusM(figure)`, the column's and the slices' scale. */
  readonly referenceRadiusM: number;
  /** Each light's curves of growth for the medium's absorbers. */
  readonly curves: AbsorberCurveSlots;
  /** κ_k of the slices, ascending: the slicing's. */
  readonly kappa: Float64Array;
  /** Slice k: the reference medium on the sphere of radius κ_k R_ref, its rows at h\*. */
  readonly slices: ReadonlyArray<TwinTransmittance>;
  /** The latitude bands, ascending in s: the slicing's. */
  readonly bands: ReadonlyArray<TwinBand>;
}

/**
 * One latitude band at s = `gravityRatioValue` (Design note 17): its latitude, its sphere s √(MN)
 * there (R_ref where `oneBand`), and its own transmittance and multiple-scattering tables.
 *
 * @param oneBand - Whether it is a body's only band, built at R_ref as R05's per-planet tables are.
 * @param reuse - A transmittance table already built on the band's own sphere at s = 1, which the
 *   band then takes as its own: a body of one slice and one band builds the table once.
 * @throws RangeError as {@link planetTwin}.
 */
export function twinBandOf(
  medium: AtmosphereMedium,
  body: TwinBody,
  gravityRatioValue: number,
  curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
  sizes: OblateTwinSizes = OBLATE_KERNEL_SIZES,
  oneBand = false,
  reuse: TwinTransmittance | null = null,
): TwinBand {
  checkAbsorberCurves(medium, curves);
  const latitudeRad = bandLatitudeRad(body.gravity, gravityRatioValue);
  let bottomRadiusM: number;
  if (oneBand) {
    bottomRadiusM = tableRadiusM(body.figure);
  } else {
    const { meridionalM, primeVerticalM } = principalRadiiM(body.figure, latitudeRad);
    bottomRadiusM = gravityRatioValue * Math.sqrt(meridionalM * primeVerticalM);
  }
  const factor = 1 / gravityRatioValue;
  const transmittance: TwinTransmittance = reuse ?? {
    ...transmittanceTables(
      medium,
      bottomRadiusM,
      sizes.transmittance,
      sizes.transmittanceSamples,
      factor,
    ),
    curves,
  };
  const layers = multiScatteringLayersOf(medium, curves);
  const multiScatteringBySlot = Array.from({ length: layers }, (_, slot) =>
    multiScatteringTableOf(
      multiScatteringInputsOf(
        medium,
        bottomRadiusM,
        transmittance,
        sizes.multiScattering,
        slot,
        factor,
      ),
    ),
  );
  return {
    gravityRatio: gravityRatioValue,
    latitudeRad,
    bottomRadiusM,
    transmittance,
    multiScatteringBySlot,
  };
}

/**
 * An oblate body's per-planet tables (plan R08, Design note 17; R08.T6.e): a transmittance slice
 * at each κ_k of the body's slicing, the reference medium on the sphere κ_k R_ref, and a
 * multiple-scattering band at each s_b ({@link twinBandOf}). Earth's slicing is one slice at κ = 1
 * and one band at s = 1, whose tables are {@link planetTwin}'s.
 *
 * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium with
 *   no absorber.
 * @throws RangeError as {@link planetTwin}.
 */
export function oblatePlanetTwin(
  medium: AtmosphereMedium,
  body: TwinBody,
  curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
  sizes: OblateTwinSizes = OBLATE_KERNEL_SIZES,
): OblatePlanetTwin {
  checkAbsorberCurves(medium, curves);
  const referenceRadiusM = tableRadiusM(body.figure);
  const { kappa, bandGravityRatio } = body.gravity.slicing;
  const slices = Array.from(kappa, (k) => ({
    ...transmittanceTables(
      medium,
      k * referenceRadiusM,
      sizes.transmittance,
      sizes.transmittanceSamples,
    ),
    curves,
  }));
  const oneBand = bandGravityRatio.length === 1;
  const first = slices[0];
  const shared = oneBand && kappa.length === 1 && kappa[0] === 1 && first !== undefined;
  const bands = Array.from(bandGravityRatio, (s) =>
    twinBandOf(medium, body, s, curves, sizes, oneBand, shared && s === 1 ? first : null),
  );
  return { medium, body, referenceRadiusM, curves, kappa, slices, bands };
}

/**
 * How the sliced lookup found a ray's sphere (plan R08, Design note 17, _The ray's own gradient_):
 *
 * - `single`: one slice and one band, the slice read at the sample's h\* and μ, as R05's table at h;
 * - `sample`: at the sample's own s and R_α, κ = s R_α ÷ R_ref and μ (a body of one band, whose s
 *   varies by under 2%, and the record's comparison);
 * - `rising`: a ray that rises or runs level, or dips by under half a local scale height: the sphere
 *   through the start, with the ray's slope of h\* there and through h\* where the ray has risen one
 *   local scale height;
 * - `deep`: a ray that dips by more than half a local scale height: the sphere through the ray's
 *   lowest h\* with its curvature there, found from three points about the tangent point.
 */
export type SlicedFit = "single" | "sample" | "rising" | "deep";

/** One slice a sliced lookup reads, and how. */
export interface SliceRead {
  readonly slice: number;
  /** Its weight in the interpolation, linear in √κ. */
  readonly weight: number;
  /** The zenith cosine the slice is read at. */
  readonly cosine: number;
}

/** A sliced lookup of one ray towards a source (plan R08, Design note 17; R08.T6.e). */
export interface SlicedRay {
  /** s at the ray's start. */
  readonly gravityRatio: number;
  /** h\* = s h at the ray's start, m. */
  readonly scaledHeightM: number;
  /** The ray's zenith cosine against the datum's normal. */
  readonly cosine: number;
  readonly fit: SlicedFit;
  /** κ\* of the sphere it reads, before it is held to the slices' range; +∞ for a flat fit. */
  readonly kappa: number;
  /** μ\* on that sphere. */
  readonly fittedCosine: number;
  /** ln(κ\* ÷ κ at the nearer end of the slices' range) where κ\* lies beyond it; 0 within. */
  readonly beyondRange: number;
  /**
   * The lowest h\* of the ray on its sphere, m, for a ray that dips below its start's horizon,
   * which the slices are then read at (Design note 17); `null` for a ray that does not.
   */
  readonly lowestScaledHeightM: number | null;
  /** Whether the ground hides the source along the ray. */
  readonly shadowed: boolean;
  readonly reads: ReadonlyArray<SliceRead>;
}

/** How a sliced lookup takes its sphere: Design note 17's fitted rule, or the sample's alone. */
export type SliceRule = "fitted" | "sample";

/**
 * Below this squared horizontal part a ray is taken as vertical: its vertical plane is any, and its
 * radius of curvature the Gaussian √(MN).
 */
const VERTICAL_BELOW = 1e-24;

/** The datum's north and east at a point of geodetic normal `n`, both unit and horizontal. */
function horizontalAxes(n: Vec3): { readonly north: Vec3; readonly east: Vec3 } {
  const cosLatitude = Math.hypot(n.x, n.y);
  if (cosLatitude < 1e-12) {
    // At a pole every azimuth is the same: north along −x (+x below), east along y.
    const sign = n.z >= 0 ? 1 : -1;
    return { north: { x: -sign, y: 0, z: 0 }, east: { x: 0, y: 1, z: 0 } };
  }
  const cosL = n.x / cosLatitude;
  const sinL = n.y / cosLatitude;
  return {
    north: { x: -n.z * cosL, y: -n.z * sinL, z: cosLatitude },
    east: { x: -sinL, y: cosL, z: 0 },
  };
}

/** A direction's vertical plane at a point of the datum (Design note 17). */
interface VerticalPlane {
  /** R_α, m: the datum's radius of curvature in the plane. */
  readonly curvatureRadiusM: number;
  /** The direction's part along the point's north, √(1 − μ²) cos α. */
  readonly towardsNorth: number;
  /** M, m, at the point's latitude. */
  readonly meridionalM: number;
}

/**
 * The vertical plane of `direction` at the point of geodetic latitude `latitudeRad` and normal `n`:
 * its R_α (`oblate.ts`'s `directionalCurvatureRadiusM`, by Euler's theorem from the direction's
 * north and east parts; the Gaussian √(MN) for a vertical direction), its northward part and M.
 *
 * @remarks
 * 1 ÷ R_α = cos²α ÷ M + sin²α ÷ N is taken as R_α = M ÷ (1 − (1 − M ÷ N) sin²α), which is M = N
 * itself on a sphere, so that a sphere's per-view marches stand on R05's sphere to the bit.
 */
function verticalPlaneOf(
  figure: SpheroidFigure,
  latitudeRad: number,
  n: Vec3,
  direction: Vec3,
): VerticalPlane {
  const { north, east } = horizontalAxes(n);
  const along = direction.x * north.x + direction.y * north.y + direction.z * north.z;
  const across = direction.x * east.x + direction.y * east.y + direction.z * east.z;
  const { meridionalM: m, primeVerticalM: nRadius } = principalRadiiM(figure, latitudeRad);
  const horizontal = along * along + across * across;
  return {
    curvatureRadiusM:
      horizontal > VERTICAL_BELOW
        ? m / (1 - (1 - m / nRadius) * ((across * across) / horizontal))
        : Math.sqrt(m * nRadius),
    towardsNorth: along,
    meridionalM: m,
  };
}

/** R_α of {@link verticalPlaneOf}, m. */
function curvatureAlongM(
  figure: SpheroidFigure,
  latitudeRad: number,
  n: Vec3,
  direction: Vec3,
): number {
  return verticalPlaneOf(figure, latitudeRad, n, direction).curvatureRadiusM;
}

/** The geodetic latitude of a geodetic normal, rad. */
function latitudeOfNormal(n: Vec3): number {
  return Math.atan2(n.z, Math.hypot(n.x, n.y));
}

/**
 * −√(1 − ρ²) for ρ = (r + lowest) ÷ (r + h), the zenith cosine from height `heightM` above a sphere
 * of radius `radiusM` of the ray whose lowest point is `lowestM` above it, taken as
 * −√((1 − ρ)(1 + ρ)) with 1 − ρ = (h − lowest) ÷ (r + h), which does not cancel.
 */
function cosineToLowest(radiusM: number, heightM: number, lowestM: number): number {
  const reach = radiusM + heightM;
  const short = (heightM - lowestM) / reach;
  return -Math.sqrt(Math.max(short * (2 - short), 0));
}

/** A ray's lowest height above a sphere of radius `radiusM`, from `heightM` at cosine `mu` < 0, m. */
function lowestOnSphereM(radiusM: number, heightM: number, mu: number): number {
  const reach = radiusM + heightM;
  // (r + h)√(1 − μ²) − r = h − (r + h)(1 − √(1 − μ²)), the latter as μ² ÷ (1 + √(1 − μ²)).
  const sine = Math.sqrt(Math.max((1 - mu) * (1 + mu), 0));
  return heightM - (reach * mu * mu) / (1 + sine);
}

/**
 * A planet's slices and bands as the per-frame marches read them on an oblate body (plan R08,
 * Design note 17; R08.T6.e): each source's transmittance on the sphere its own ray fits, read
 * through the slices at h\*, with its optical depth divided by s, and the multiple scattering of
 * the bands about the sample's s at h\*.
 */
class OblateReader {
  readonly figure: SpheroidFigure;
  readonly gravity: BodyGravity;
  readonly #planet: OblatePlanetTwin;
  readonly #terms: readonly TwinTerm[];
  readonly #shells: readonly Shell[];
  readonly #lnKappa: Float64Array;
  readonly #rootKappa: Float64Array;
  readonly #lnBands: Float64Array;
  readonly #topM: number;
  readonly #absorbers = new Map<number, TwinAbsorbers>();
  readonly #scratch = readScratch();
  readonly #sample = new Float64Array(SAMPLE_FLOATS);
  readonly #vertical: TwinRgb = [0, 0, 0];
  readonly #depth: TwinRgb = [0, 0, 0];
  readonly #columns = new Float64Array(ABSORBER_SLOTS);
  readonly #pressureColumns = new Float64Array(ABSORBER_SLOTS);
  readonly #absorbed: TwinRgb = [0, 0, 0];
  readonly #lower: TwinRgb = [0, 0, 0];
  readonly #upper: TwinRgb = [0, 0, 0];

  constructor(planet: OblatePlanetTwin) {
    this.#planet = planet;
    this.figure = planet.body.figure;
    this.gravity = planet.body.gravity;
    this.#terms = termsOf(planet.medium);
    this.#topM = planet.medium.topHeightM;
    this.#shells = Array.from(planet.kappa, (k) => {
      const bottomRadiusM = k * planet.referenceRadiusM;
      return { bottomRadiusM, topRadiusM: bottomRadiusM + this.#topM };
    });
    this.#lnKappa = planet.kappa.map(Math.log);
    this.#rootKappa = planet.kappa.map(Math.sqrt);
    this.#lnBands = Float64Array.from(planet.bands, (band) => Math.log(band.gravityRatio));
    checkAbsorberCurves(planet.medium, planet.curves);
    this.absorbersAt(0);
  }

  /** The medium's absorbers under light slot `slot`'s curves, checked once. */
  absorbersAt(slot: number): TwinAbsorbers {
    let absorbers = this.#absorbers.get(slot);
    if (absorbers === undefined) {
      absorbers = twinAbsorbers(this.#planet.medium, this.#planet.curves, slot);
      this.#absorbers.set(slot, absorbers);
    }
    return absorbers;
  }

  /** h\* = s h of a body-fixed point, m. */
  scaledHeightAtM(pointM: Vec3): number {
    const geodetic = geodeticOf(pointM, this.figure);
    return gravityRatio(this.gravity, geodetic.latitudeRad) * geodetic.heightM;
  }

  /**
   * The local scale height in h\* at `scaledHeightM`, m: the stored vertical optical depth above it
   * in green, slice 0's, over the extinction there, held to the room left to the top (all of it
   * where nothing extinguishes) and to at least `hillaire.ts`'s {@link FIT_SCALE_FLOOR_M}
   * (R08.T6.f): the height over which the column above lies, so one scale height for an
   * exponential, by which the fitted lookup places its second point.
   */
  #localScaleHeightM(scaledHeightM: number): number {
    const room = Math.max(this.#topM - scaledHeightM, 1);
    const shell = this.#shells[0];
    const slice = this.#planet.slices[0];
    if (shell === undefined || slice === undefined) {
      return Math.max(room, FIT_SCALE_FLOOR_M);
    }
    const height = Math.min(Math.max(scaledHeightM, 0), this.#topM);
    const [u, v] = transmittanceRMuToUv(shell, shell.bottomRadiusM + height, 1);
    tableAtUnitInto(slice.transmittance, u, v, this.#vertical);
    sampleInto(this.#terms, null, null, height, this.#sample);
    const extinctionPerM = this.#sample[4] ?? 0;
    const held = extinctionPerM > 0 ? Math.min(this.#vertical[1] / extinctionPerM, room) : room;
    return Math.max(held, FIT_SCALE_FLOOR_M);
  }

  /**
   * The sliced lookup of the ray from the body-fixed `positionM`, whose geodetic coordinates are
   * `geodetic`, along the unit body-fixed `direction` (plan R08, Design note 17; R08.T6.e).
   *
   * @remarks
   * s, h\* and μ are the start's. With one slice and one band the slice is read at h\* and μ. With
   * one band, or under `rule` `sample`, the sphere is the start's own, κ = s R_α ÷ R_ref, R_α of the
   * ray's azimuth (Euler's theorem). Otherwise it is fitted to the ray's own h\*(x), as
   * `decision-r08-spheroid.md` §2 measured it, x the distance along the ray:
   *
   * - through the start, with the ray's slope of h\* there, s μ + h (∂s ÷ ∂φ) √(1 − μ²) cos α ÷
   *   (M + h), analytic (`oblate.ts`'s `gravityRatioSlope`; the gradients of h and φ are the normal
   *   and north ÷ (M + h)), its cosine μ\* that slope over s, the frame's lengths being s x;
   * - and through h\* at x₁, where the ray on the start's sphere R_α has risen one local scale height
   *   H ÷ s (past its return to the start's height for a ray that dips), one more geodetic
   *   conversion: the sphere's radius is (x² + 2 h\*₀ μ\* x + h\*₀² − h\*₁²) ÷ 2(h\*₁ − h\*₀ − μ\* x) at
   *   x = s x₁, the ray from h\*₀ at μ\* on it passing h\*₁ there.
   *
   * A ray that dips below its start's horizon by more than half a local scale height in h\* on the
   * start's sphere is fitted at its lowest point instead: h\* at its tangent point on the start's
   * sphere and a distance √(2 R_α H ÷ s) either side, three conversions, give the parabola whose
   * vertex is the lowest h\* and whose curvature c gives the radius s² ÷ 2c less that height. Every
   * ray that dips is read on each slice at the cosine that keeps its lowest h\* (Design note 17),
   * and the ground hides the source where that height is below the datum. The slices are
   * interpolated linearly in √κ\*, a κ\* beyond their range taking the nearer slice, and how far
   * it lies beyond is kept. H is {@link OblateReader}'s local scale height, or `scaleHeightM` where
   * it is given: the lit passes' reading of the body's table (R08.T9.b).
   */
  rayOf(
    positionM: Vec3,
    geodetic: Geodetic,
    direction: Vec3,
    rule: SliceRule,
    scaleHeightM: number | null = null,
  ): SlicedRay {
    const planet = this.#planet;
    const referenceM = planet.referenceRadiusM;
    const kappa = planet.kappa;
    const h0 = geodetic.heightM;
    const latitude = geodetic.latitudeRad;
    const s0 = gravityRatio(this.gravity, latitude);
    const z0 = s0 * h0;
    const n = geodetic.normal;
    const mu = Math.min(Math.max(direction.x * n.x + direction.y * n.y + direction.z * n.z, -1), 1);
    const oneBand = planet.bands.length === 1;
    if (kappa.length === 1 && oneBand) {
      const bottom = (kappa[0] ?? Number.NaN) * referenceM;
      return {
        gravityRatio: s0,
        scaledHeightM: z0,
        cosine: mu,
        fit: "single",
        kappa: kappa[0] ?? Number.NaN,
        fittedCosine: mu,
        beyondRange: 0,
        lowestScaledHeightM: mu < 0 ? lowestOnSphereM(bottom, z0, mu) : null,
        shadowed: groundShadows(bottom, z0, mu),
        reads: [{ slice: 0, weight: 1, cosine: mu }],
      };
    }
    const plane = verticalPlaneOf(this.figure, latitude, n, direction);
    const rAlpha = plane.curvatureRadiusM;
    if (rule === "sample" || oneBand) {
      const kappaStar = (s0 * rAlpha) / referenceM;
      const sphereM = kappaStar * referenceM;
      const lowest = mu < 0 ? lowestOnSphereM(sphereM, z0, mu) : null;
      return this.#read(
        s0,
        z0,
        mu,
        "sample",
        kappaStar,
        mu,
        rule === "sample" ? null : lowest,
        lowest,
      );
    }
    const scaleM = scaleHeightM ?? this.#localScaleHeightM(z0);
    const riseM = scaleM / s0;
    const reach = rAlpha + h0;
    const dip = mu < 0 ? h0 - lowestOnSphereM(rAlpha, h0, mu) : 0;
    const at = (x: number): number =>
      this.scaledHeightAtM({
        x: positionM.x + x * direction.x,
        y: positionM.y + x * direction.y,
        z: positionM.z + x * direction.z,
      });
    if (mu < 0 && s0 * dip > scaleM / 2) {
      const xt = -reach * mu;
      const delta = Math.sqrt(2 * riseM * rAlpha);
      const zt = at(xt);
      const zp = at(xt + delta);
      const zm = at(xt - delta);
      const curvature = (zp + zm - 2 * zt) / (2 * delta * delta);
      if (curvature > 0) {
        const lowest = zt - (zp - zm) ** 2 / (16 * curvature * delta * delta);
        const radiusM = (s0 * s0) / (2 * curvature) - lowest;
        const fitted = cosineToLowest(radiusM, z0, lowest);
        return this.#read(s0, z0, mu, "deep", radiusM / referenceM, fitted, lowest, lowest);
      }
    }
    let x1 = -reach * mu + Math.sqrt(reach * reach * mu * mu + riseM * (2 * reach + riseM));
    if (mu < 0) {
      x1 = Math.max(x1, -2 * reach * mu);
    }
    const z1 = at(x1);
    const slope =
      s0 * mu +
      (h0 * gravityRatioSlope(this.gravity, latitude) * plane.towardsNorth) /
        (plane.meridionalM + h0);
    const fitted = Math.min(Math.max(slope / s0, -1), 1);
    const xs = s0 * x1;
    const denominator = 2 * (z1 - z0 - fitted * xs);
    const radiusM =
      denominator > 0
        ? (xs * xs + 2 * z0 * fitted * xs + z0 * z0 - z1 * z1) / denominator
        : Number.POSITIVE_INFINITY;
    const kappaStar = radiusM / referenceM;
    let lowest: number | null = null;
    if (fitted < 0) {
      const sphereM = Number.isFinite(radiusM)
        ? radiusM
        : (kappa[kappa.length - 1] ?? Number.NaN) * referenceM;
      lowest = lowestOnSphereM(sphereM, z0, fitted);
    }
    return this.#read(s0, z0, mu, "rising", kappaStar, fitted, lowest, lowest);
  }

  /**
   * A {@link SlicedRay} on the sphere κ\*, μ\*: its slices interpolated linearly in √κ\*, each read at
   * `fittedCosine`, or at the cosine that keeps `readLowestM` where that is given; the ground
   * hiding the source where `lowestM` is below the datum.
   *
   * @param gravityRatioValue - s at the ray's start.
   * @param scaledHeightM - h\* at the ray's start, m.
   * @param mu - The ray's zenith cosine against the datum's normal there.
   */
  #read(
    gravityRatioValue: number,
    scaledHeightM: number,
    mu: number,
    fit: SlicedFit,
    kappaStar: number,
    fittedCosine: number,
    readLowestM: number | null,
    lowestM: number | null,
  ): SlicedRay {
    const lnKappa = this.#lnKappa;
    const rootKappa = this.#rootKappa;
    const last = lnKappa.length - 1;
    const first = lnKappa[0] ?? Number.NaN;
    const top = lnKappa[last] ?? Number.NaN;
    const lk = kappaStar > 0 ? Math.log(kappaStar) : Number.NEGATIVE_INFINITY;
    let lower = 0;
    let weight = 0;
    let beyondRange = 0;
    if (!(lk > first)) {
      beyondRange = lk - first;
    } else if (!(lk < top)) {
      lower = Math.max(last - 1, 0);
      weight = last > 0 ? 1 : 0;
      beyondRange = lk - top;
    } else {
      while (lower < last - 1 && (lnKappa[lower + 1] ?? Number.NaN) <= lk) {
        lower += 1;
      }
      // Linear in √κ (ruled 2026-10-13 for R08.T6.f, after R08.T6.e's record): exact, to first
      // order in H ÷ R, for Chapman's grazing column, which goes as √R, and for a vertical ray,
      // where Design note 17's "in ln κ" left 6.8 × 10⁻⁴ of the grazing τ halfway between slices.
      const r0 = rootKappa[lower] ?? Number.NaN;
      weight = (Math.sqrt(kappaStar) - r0) / ((rootKappa[lower + 1] ?? Number.NaN) - r0);
    }
    const cosineOn = (slice: number): number => {
      if (readLowestM === null) {
        return fittedCosine;
      }
      const radiusM = (this.#planet.kappa[slice] ?? Number.NaN) * this.#planet.referenceRadiusM;
      return cosineToLowest(radiusM, scaledHeightM, readLowestM);
    };
    const reads: SliceRead[] = [];
    if (weight < 1) {
      reads.push({ slice: lower, weight: 1 - weight, cosine: cosineOn(lower) });
    }
    if (weight > 0) {
      reads.push({ slice: lower + 1, weight, cosine: cosineOn(lower + 1) });
    }
    return {
      gravityRatio: gravityRatioValue,
      scaledHeightM,
      cosine: mu,
      fit,
      kappa: kappaStar,
      fittedCosine,
      beyondRange: Number.isFinite(beyondRange) ? beyondRange : Number.POSITIVE_INFINITY,
      lowestScaledHeightM: lowestM,
      shadowed: lowestM !== null && lowestM < 0,
      reads,
    };
  }

  /**
   * The slices read at κ and a fixed μ from h\* = `scaledHeightM`, with s taken as 1: the lookup's
   * interpolation alone, which {@link slicedStoredOpticalDepth} reads.
   */
  fixedCosineRay(kappa: number, scaledHeightM: number, mu: number): SlicedRay {
    return this.#read(1, scaledHeightM, mu, "sample", kappa, mu, null, null);
  }

  /**
   * The stored optical depth and absorber columns along a sliced ray into the reader's own arrays:
   * each slice read at the ray's h\*, held in the shell, at its own cosine, weighted, and the sum
   * divided by s (Design note 17).
   */
  #storedAlong(ray: SlicedRay, absorbers: TwinAbsorbers): void {
    const depth = this.#depth;
    const columns = this.#columns;
    const pressureColumns = this.#pressureColumns;
    depth.fill(0);
    columns.fill(0);
    pressureColumns.fill(0);
    const scratch = this.#scratch;
    const height = Math.min(Math.max(ray.scaledHeightM, 0), this.#topM);
    for (const { slice, weight, cosine } of ray.reads) {
      const shell = this.#shells[slice];
      const tables = this.#planet.slices[slice];
      if (shell === undefined || tables === undefined) {
        throw new Error(`a sliced read names slice ${slice} of ${this.#shells.length}`);
      }
      const [u, v] = transmittanceRMuToUv(shell, shell.bottomRadiusM + height, cosine);
      storedReadInto(tables, absorbers, u, v, scratch);
      for (const c of CHANNELS) {
        depth[c] += weight * (scratch.stored[c] ?? Number.NaN);
      }
      for (let k = 0; k < ABSORBER_SLOTS; k += 1) {
        columns[k] = (columns[k] ?? 0) + weight * (scratch.columns[k] ?? Number.NaN);
        pressureColumns[k] =
          (pressureColumns[k] ?? 0) + weight * (scratch.pressureColumns[k] ?? Number.NaN);
      }
    }
    const divisor = ray.gravityRatio;
    for (const c of CHANNELS) {
      depth[c] /= divisor;
    }
    for (let k = 0; k < ABSORBER_SLOTS; k += 1) {
      columns[k] = (columns[k] ?? 0) / divisor;
      pressureColumns[k] = (pressureColumns[k] ?? 0) / divisor;
    }
  }

  /**
   * A source's transmittance into `transmittance` from the body-fixed `positionM` along `direction`
   * under light slot `slot`'s curves, and the stored optical depth into `depth`, the forward lobes'
   * read: the sliced lookup's stored values, divided by s, with the slot's curves added; whether the
   * ground leaves the source lit.
   */
  sunInto(
    positionM: Vec3,
    geodetic: Geodetic,
    direction: Vec3,
    slot: number,
    transmittance: TwinRgb,
    depth: TwinRgb,
  ): boolean {
    const ray = this.rayOf(positionM, geodetic, direction, "fitted");
    this.#depthsInto(ray, slot, depth);
    for (const c of CHANNELS) {
      transmittance[c] = Math.exp(-(depth[c] + this.#absorbed[c]));
    }
    return !ray.shadowed;
  }

  /**
   * A sliced ray's stored optical depth, divided by s, into `depth`, and its absorbers' under light
   * slot `slot`'s curves into the reader's own array, which {@link OblateReader.sunInto} adds.
   */
  opticalDepthInto(ray: SlicedRay, slot: number, depth: TwinRgb): Rgb {
    this.#depthsInto(ray, slot, depth);
    return [this.#absorbed[0], this.#absorbed[1], this.#absorbed[2]];
  }

  /** {@link OblateReader.opticalDepthInto} without the copy of the absorbers' depth. */
  #depthsInto(ray: SlicedRay, slot: number, depth: TwinRgb): void {
    const absorbers = this.absorbersAt(slot);
    this.#storedAlong(ray, absorbers);
    absorberDepthInto(absorbers, this.#columns, this.#pressureColumns, this.#absorbed);
    for (const c of CHANNELS) {
      depth[c] = this.#depth[c];
    }
  }

  /**
   * The multiple scattering at `scaledHeightM` and sun cosine `muSun` into `out`, light slot
   * `slot`'s layer: each band's table read there (sebh's unit range, its 10 m offset, Bruneton's
   * texel mapping), linear in ln s between the bands about `s`, the nearer band beyond them.
   */
  multiScatteringInto(
    scaledHeightM: number,
    s: number,
    muSun: number,
    slot: number,
    out: TwinRgb,
  ): void {
    const bands = this.#planet.bands;
    const v =
      (Math.max(scaledHeightM, 0) - MULTI_SCATTERING_GROUND_OFFSET_M) /
      (this.#topM - MULTI_SCATTERING_GROUND_OFFSET_M);
    const u = muSun * 0.5 + 0.5;
    const layerOf = (band: TwinBand | undefined): TwinTable => {
      const layers = band?.multiScatteringBySlot ?? [];
      const layer = layers[Math.min(slot, layers.length - 1)];
      if (layer === undefined) {
        throw new Error("an oblate twin's band has at least one multiple-scattering layer");
      }
      return layer;
    };
    const lnBands = this.#lnBands;
    const last = lnBands.length - 1;
    const ls = Math.log(s);
    if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
      tableAtUnitInto(layerOf(bands[0]), u, v, out);
      return;
    }
    if (!(ls < (lnBands[last] ?? Number.NaN))) {
      tableAtUnitInto(layerOf(bands[last]), u, v, out);
      return;
    }
    let lower = 0;
    while (lower < last - 1 && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
      lower += 1;
    }
    const l0 = lnBands[lower] ?? Number.NaN;
    const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
    tableAtUnitInto(layerOf(bands[lower]), u, v, this.#lower);
    tableAtUnitInto(layerOf(bands[lower + 1]), u, v, this.#upper);
    for (const c of CHANNELS) {
      out[c] = this.#lower[c] * (1 - w) + this.#upper[c] * w;
    }
  }

  /** A march's own path's absorber depth under light slot `slot`'s curves (`TwinSources`'). */
  absorberPathDepthInto(
    columns: Float64Array,
    pressureColumns: Float64Array,
    out: TwinRgb,
    slot: number,
  ): void {
    absorberDepthInto(this.absorbersAt(slot), columns, pressureColumns, out);
  }
}

/** Each planet's {@link OblateReader}, made once: a lookup's tests call it ray by ray. */
const OBLATE_READERS = new WeakMap<OblatePlanetTwin, OblateReader>();

/** A planet's {@link OblateReader}, cached. */
function oblateReaderOf(planet: OblatePlanetTwin): OblateReader {
  let reader = OBLATE_READERS.get(planet);
  if (reader === undefined) {
    reader = new OblateReader(planet);
    OBLATE_READERS.set(planet, reader);
  }
  return reader;
}

/** A sliced lookup's ray and what it reads (R08.T6.e). */
export interface SlicedSunTransmittance {
  readonly ray: SlicedRay;
  /** The optical depth of every term but the absorbers, the slices' stored depth divided by s. */
  readonly storedOpticalDepth: Rgb;
  /** The absorbers' optical depth under the light slot's curves, through the slices' columns ÷ s. */
  readonly absorberOpticalDepth: Rgb;
  /** e^(−τ) of the two, unshadowed. */
  readonly transmittance: Rgb;
}

/**
 * The transmittance of the ray from the body-fixed `positionM` along the unit `direction` to the
 * top, by the sliced lookup the oblate marches read each source through (plan R08, Design note 17,
 * _The ray's own gradient_; R08.T6.e), with its sphere and slices for the record.
 *
 * @param rule - `fitted`, the marches' rule, or `sample`, the sample's own s and R_α at a fixed μ,
 *   which `decision-r08-spheroid.md` measured it against.
 * @param slot - The light slot whose curves the absorbers take.
 * @throws RangeError as {@link oblatePlanetTwin}'s curves, for a slot they do not hold.
 */
export function slicedSunTransmittance(
  planet: OblatePlanetTwin,
  positionM: Vec3,
  direction: Vec3,
  rule: SliceRule = "fitted",
  slot = 0,
): SlicedSunTransmittance {
  const reader = oblateReaderOf(planet);
  const ray = reader.rayOf(positionM, geodeticOf(positionM, planet.body.figure), direction, rule);
  const depth: TwinRgb = [0, 0, 0];
  const absorbed = reader.opticalDepthInto(ray, slot, depth);
  return {
    ray,
    storedOpticalDepth: [depth[0], depth[1], depth[2]],
    absorberOpticalDepth: absorbed,
    transmittance: [
      Math.exp(-(depth[0] + absorbed[0])),
      Math.exp(-(depth[1] + absorbed[1])),
      Math.exp(-(depth[2] + absorbed[2])),
    ],
  };
}

/**
 * The slices' stored optical depth at κ, interpolated linearly in √κ between the slices about
 * it, each read at the same h\* and μ, the nearer slice beyond them: what {@link slicedSunTransmittance}
 * reads at a fixed μ, before the division by s (R08.T6.e's interpolation test).
 *
 * @param scaledHeightM - h\*, m, held in the shell.
 */
export function slicedStoredOpticalDepth(
  planet: OblatePlanetTwin,
  kappa: number,
  scaledHeightM: number,
  mu: number,
): Rgb {
  const reader = oblateReaderOf(planet);
  const depth: TwinRgb = [0, 0, 0];
  reader.opticalDepthInto(reader.fixedCosineRay(kappa, scaledHeightM, mu), 0, depth);
  return [depth[0], depth[1], depth[2]];
}

/**
 * The multiple scattering an oblate march reads at h\* = `scaledHeightM`, s and the sun's cosine
 * `muSun`, light slot `slot`'s layer: the bands about s, linear in ln s (Design note 17; R08.T6.e).
 */
export function bandMultiScattering(
  planet: OblatePlanetTwin,
  scaledHeightM: number,
  s: number,
  muSun: number,
  slot = 0,
): Rgb {
  const out: TwinRgb = [0, 0, 0];
  oblateReaderOf(planet).multiScatteringInto(scaledHeightM, s, muSun, slot, out);
  return [out[0], out[1], out[2]];
}

// --- Surface lighting (R08.T9.b) -----------------------------------------------------------------

/**
 * An oblate body's sky-irradiance tables (plan R08, R08.T9.b; Design note 17, _Latitude bands_):
 * band by band, a layer a light slot, each {@link irradianceTwin} in the band's own frame over its
 * own transmittance tables and multiple-scattering layer, as `AtmosphereTables` builds them. Earth's
 * one band at s = 1 reads the one slice's tables.
 *
 * @param texels - The texels to compute, each [x, y]; every texel by default.
 * @throws RangeError as {@link irradianceTwin}.
 */
export function oblateIrradianceTwin(
  planet: OblatePlanetTwin,
  options: IrradianceOptions = IRRADIANCE_KERNEL_OPTIONS,
  texels?: ReadonlyArray<readonly [number, number]>,
): ReadonlyArray<ReadonlyArray<TwinTable>> {
  return planet.bands.map((band) =>
    band.multiScatteringBySlot.map((_, slot) =>
      irradianceTwin(
        planet.medium,
        band.bottomRadiusM,
        band.transmittance,
        band.multiScatteringBySlot,
        options,
        texels,
        slot,
        1 / band.gravityRatio,
      ),
    ),
  );
}

/** The three-node rule's nodes about a wide source's centre, in units of its angular radius. */
const SOURCE_NODE_OFFSET = Math.SQRT1_2;

/** Their weights, the upper node first (Design note 7). */
const SOURCE_NODE_WEIGHTS = [0.25, 0.5, 0.25] as const;

/** A lit point at longitude 0 as `surfaceLighting.wgsl`'s `atmosphereSurfacePlace` places it. */
interface SurfacePlace {
  readonly positionM: Vec3;
  readonly geodetic: Geodetic;
  readonly north: Vec3;
  readonly east: Vec3;
}

/** A geodetic latitude held to [−π ÷ 2, π ÷ 2], as `surfaceLighting.wgsl` holds it. */
function clampLatitude(latitudeRad: number): number {
  return Math.min(Math.max(latitudeRad, -Math.PI / 2), Math.PI / 2);
}

/**
 * The unit direction of zenith cosine `mu` and azimuth `azimuthRad` from north towards east at a
 * lit point (`surfaceLighting.wgsl`'s `atmosphereDirection`).
 */
function directionOf(place: SurfacePlace, mu: number, azimuthRad: number): Vec3 {
  const m = Math.min(Math.max(mu, -1), 1);
  const horizontal = Math.sqrt(Math.max((1 - m) * (1 + m), 0));
  const n = place.geodetic.normal;
  const along = horizontal * Math.cos(azimuthRad);
  const across = horizontal * Math.sin(azimuthRad);
  return {
    x: m * n.x + along * place.north.x + across * place.east.x,
    y: m * n.y + along * place.north.y + across * place.east.y,
    z: m * n.z + along * place.north.z + across * place.east.z,
  };
}

/**
 * The twin of `surfaceLighting.wgsl`'s three lookups for one body (plan R08, R08.T9.b; Design notes
 * 7 and 17), in `f64` over `planet`'s slices and `irradiance`'s bands ({@link oblateIrradianceTwin}),
 * operation for operation where `f64` allows:
 *
 * - the lit point at longitude 0, its geodetic height above the datum along its normal at its
 *   geodetic latitude; a source's direction from its zenith cosine against the datum's normal and
 *   its azimuth from north towards east;
 * - a source's transmittance by the sliced lookup from there (`OblateReader`'s), the fit's local
 *   scale height read, as the lit passes read it, from the body's table's tabulation
 *   (`scaleHeightsM`, `tables.ts`'s `packedScaleHeightsM`), linear between its heights; 0 where the
 *   ground hides the source;
 * - the sky's irradiance from the irradiance table at h\* = s h (its root over the top's) and μ_s,
 *   linear in ln s between the bands about the point's s;
 * - a wide source's transmittance by the three-node rule (Design note 7).
 *
 * The light slot of each lookup is held to the medium's light slots.
 *
 * @param irradiance - The irradiance tables, band by band, a layer a light slot.
 * @param scaleHeightsM - The local scale heights the body's table holds; the medium's own by
 *   default.
 */
export function surfaceLightingTwin(
  planet: OblatePlanetTwin,
  irradiance: ReadonlyArray<ReadonlyArray<TwinTable>>,
  scaleHeightsM: ArrayLike<number> = packedScaleHeightsM(planet.medium),
): SurfaceLightingReader {
  const reader = oblateReaderOf(planet);
  const { figure, gravity } = planet.body;
  const topM = planet.medium.topHeightM;
  const slots = multiScatteringLayersOf(planet.medium, planet.curves);
  const slotOf = (curve: number): number => Math.min(Math.max(curve, 0), slots - 1);
  const severalBands = planet.bands.length > 1;
  const lnBands = planet.bands.map((band) => Math.log(band.gravityRatio));
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const e2 = ((a - c) * (a + c)) / (a * a);
  const scaleHeightAt = (scaledHeightM: number): number => {
    const last = OBLATE_SCALE_HEIGHTS - 1;
    const x = Math.min(Math.max(scaledHeightM / topM, 0), 1) * last;
    const i = Math.min(Math.floor(x), last - 1);
    const below = scaleHeightsM[i] ?? Number.NaN;
    const above = scaleHeightsM[i + 1] ?? Number.NaN;
    return Math.max(below + (x - i) * (above - below), FIT_SCALE_FLOOR_M);
  };
  const placeOf = (altitudeM: number, latitudeRad: number): SurfacePlace => {
    const latitude = clampLatitude(latitudeRad);
    const sinL = Math.sin(latitude);
    const cosL = Math.cos(latitude);
    const primeVertical = a / Math.sqrt(1 - e2 * sinL * sinL);
    const { meridionalM, primeVerticalM } = principalRadiiM(figure, latitude);
    return {
      positionM: {
        x: (primeVertical + altitudeM) * cosL,
        y: 0,
        z: (primeVertical * (1 - e2) + altitudeM) * sinL,
      },
      geodetic: {
        latitudeRad: latitude,
        longitudeRad: 0,
        heightM: altitudeM,
        normal: { x: cosL, y: 0, z: sinL },
        gaussianRadiusM: Math.sqrt(meridionalM * primeVerticalM),
      },
      north: { x: -sinL, y: 0, z: cosL },
      east: { x: 0, y: 1, z: 0 },
    };
  };
  const transmittanceAlong = (place: SurfacePlace, d: Vec3, curve: number): Rgb => {
    const s = gravityRatio(gravity, place.geodetic.latitudeRad);
    const scaleHeightM = severalBands ? scaleHeightAt(s * place.geodetic.heightM) : null;
    const ray = reader.rayOf(place.positionM, place.geodetic, d, "fitted", scaleHeightM);
    if (ray.shadowed) {
      return [0, 0, 0];
    }
    const depth: TwinRgb = [0, 0, 0];
    const absorbed = reader.opticalDepthInto(ray, slotOf(curve), depth);
    return [
      Math.exp(-(depth[0] + absorbed[0])),
      Math.exp(-(depth[1] + absorbed[1])),
      Math.exp(-(depth[2] + absorbed[2])),
    ];
  };
  const layerOf = (band: number, curve: number): TwinTable => {
    const layers = irradiance[band] ?? [];
    const layer = layers[Math.min(slotOf(curve), layers.length - 1)];
    if (layer === undefined) {
      throw new Error(`the sky's irradiance has no layer for band ${band}`);
    }
    return layer;
  };
  const lower: TwinRgb = [0, 0, 0];
  const upper: TwinRgb = [0, 0, 0];
  return {
    sunTransmittance: (altitudeM, muSun, latitudeRad, sunAzimuthRad, curve) => {
      const place = placeOf(altitudeM, latitudeRad);
      return transmittanceAlong(place, directionOf(place, muSun, sunAzimuthRad), curve);
    },
    skyIrradiance: (altitudeM, muSun, latitudeRad, curve) => {
      const s = gravityRatio(gravity, clampLatitude(latitudeRad));
      const u = muSun * 0.5 + 0.5;
      const v = Math.sqrt(Math.max(s * altitudeM, 0) / topM);
      const last = lnBands.length - 1;
      const ls = Math.log(s);
      if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
        tableAtUnitInto(layerOf(0, curve), u, v, lower);
        return [lower[0], lower[1], lower[2]];
      }
      if (!(ls < (lnBands[last] ?? Number.NaN))) {
        tableAtUnitInto(layerOf(last, curve), u, v, lower);
        return [lower[0], lower[1], lower[2]];
      }
      let band = 0;
      while (band < last - 1 && (lnBands[band + 1] ?? Number.NaN) <= ls) {
        band += 1;
      }
      const l0 = lnBands[band] ?? Number.NaN;
      const w = (ls - l0) / ((lnBands[band + 1] ?? Number.NaN) - l0);
      tableAtUnitInto(layerOf(band, curve), u, v, lower);
      tableAtUnitInto(layerOf(band + 1, curve), u, v, upper);
      return [
        lower[0] * (1 - w) + upper[0] * w,
        lower[1] * (1 - w) + upper[1] * w,
        lower[2] * (1 - w) + upper[2] * w,
      ];
    },
    sourceTransmittance: (
      altitudeM,
      muCentre,
      latitudeRad,
      azimuthRad,
      angularRadiusRad,
      curve,
    ) => {
      const place = placeOf(altitudeM, latitudeRad);
      const centre = directionOf(place, muCentre, azimuthRad);
      const n = place.geodetic.normal;
      const cosZenith = centre.x * n.x + centre.y * n.y + centre.z * n.z;
      const across = {
        x: n.x - cosZenith * centre.x,
        y: n.y - cosZenith * centre.y,
        z: n.z - cosZenith * centre.z,
      };
      const sinZenith = Math.hypot(across.x, across.y, across.z);
      const up = sinZenith > 1e-6 ? scale(across, 1 / sinZenith) : place.north;
      const delta = Math.min(Math.abs(angularRadiusRad) * SOURCE_NODE_OFFSET, Math.PI);
      const cosDelta = Math.cos(delta);
      const sinDelta = Math.sin(delta);
      const nodes: Vec3[] = [1, 0, -1].map((sign) => ({
        x: (sign === 0 ? 1 : cosDelta) * centre.x + sign * sinDelta * up.x,
        y: (sign === 0 ? 1 : cosDelta) * centre.y + sign * sinDelta * up.y,
        z: (sign === 0 ? 1 : cosDelta) * centre.z + sign * sinDelta * up.z,
      }));
      const sum: TwinRgb = [0, 0, 0];
      let total = 0;
      nodes.forEach((node, k) => {
        const mu = node.x * n.x + node.y * n.y + node.z * n.z;
        if (!(mu > 0)) {
          return;
        }
        const w = (SOURCE_NODE_WEIGHTS[k] ?? Number.NaN) * mu;
        const t = transmittanceAlong(place, node, curve);
        for (const ch of CHANNELS) {
          sum[ch] += w * t[ch];
        }
        total += w;
      });
      return total > 0 ? [sum[0] / total, sum[1] / total, sum[2] / total] : [0, 0, 0];
    },
  };
}

// --- The true spheroid's beam, by brute force (R08.T6.e) -----------------------------------------

/** Gauss–Legendre's 8 nodes on [−1, 1] (Abramowitz and Stegun 1964, Table 25.4), positive half. */
const GAUSS_8_NODES = [
  0.183_434_642_495_649_8, 0.525_532_409_916_329, 0.796_666_477_413_626_7, 0.960_289_856_497_536_3,
] as const;

/** Their weights. */
const GAUSS_8_WEIGHTS = [
  0.362_683_783_378_362, 0.313_706_645_877_887_3, 0.222_381_034_453_374_5, 0.101_228_536_290_376_3,
] as const;

/** A ray's optical depth on the true spheroid ({@link spheroidOpticalDepth}). */
export interface SpheroidOpticalDepth {
  /** Every term's extinction integrated, the transmittance tables' stored terms', per channel. */
  readonly opticalDepth: Rgb;
  /** Each absorber slot's column u ÷ U, slot by slot. */
  readonly columns: Float64Array;
  /** The ray's length to its end, m. */
  readonly lengthM: number;
  /** Whether it ends on the ground. */
  readonly hitsGround: boolean;
}

/**
 * The optical depth of a ray on the true spheroid, density at h\* = s h at every point, in `f64`
 * (plan R08, R08.T6.e; `decision-r08-spheroid.md`): the sliced lookup's reference, and Design note
 * 10's reference-beam quadrature on a spheroid, against which R08.T13 holds the tracer's
 * `sunOpticalDepth`.
 *
 * @remarks
 * Each point's geodetic latitude and height come from {@link geodeticOf}'s closed form, its s from
 * `oblate.ts`'s `gravityRatio`. The ray ends on the datum, or where it leaves h\* = `topHeightM`
 * past its lowest point: found by a scan at half a piece and 60 bisections. It is integrated by
 * Gauss–Legendre's 8 points on pieces of at most `pieceM`, which converges spectrally on a smooth
 * profile (exponential); a tent's or a table's kinks converge only as fast as they allow, which a
 * caller checks by halving `pieceM`. Every term's extinction is summed, as the transmittance tables
 * store them, and the absorbers' columns beside it.
 *
 * @param pieceM - The pieces' largest length, m.
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0, or for a ray that neither meets the ground nor leaves the top within a few body
 *   radii.
 */
export function spheroidOpticalDepth(
  medium: AtmosphereMedium,
  body: TwinBody,
  positionM: Vec3,
  direction: Vec3,
  pieceM = 5_000,
): SpheroidOpticalDepth {
  const terms = termsOf(medium);
  const topM = medium.topHeightM;
  const figure = body.figure;
  const sample = new Float64Array(SAMPLE_FLOATS);
  const point = (t: number): Vec3 => ({
    x: positionM.x + t * direction.x,
    y: positionM.y + t * direction.y,
    z: positionM.z + t * direction.z,
  });
  const heightsAt = (t: number): readonly [number, number] => {
    const geodetic = geodeticOf(point(t), figure);
    return [gravityRatio(body.gravity, geodetic.latitudeRad) * geodetic.heightM, geodetic.heightM];
  };
  const stepM = pieceM / 2;
  const limitM = 8 * (figure.equatorialRadiusM + topM);
  let t = 0;
  let previous = heightsAt(0)[0];
  let endM = Number.NaN;
  let hitsGround = false;
  while (t < limitM) {
    const next = t + stepM;
    const [scaled, geometric] = heightsAt(next);
    if (geometric < 0) {
      let low = t;
      let high = next;
      for (let i = 0; i < 60; i += 1) {
        const middle = 0.5 * (low + high);
        if (heightsAt(middle)[1] < 0) {
          high = middle;
        } else {
          low = middle;
        }
      }
      endM = low;
      hitsGround = true;
      break;
    }
    if (scaled > topM && scaled > previous) {
      let low = t;
      let high = next;
      for (let i = 0; i < 60; i += 1) {
        const middle = 0.5 * (low + high);
        if (heightsAt(middle)[0] > topM) {
          high = middle;
        } else {
          low = middle;
        }
      }
      endM = high;
      break;
    }
    previous = scaled;
    t = next;
  }
  if (!Number.isFinite(endM)) {
    throw new RangeError("a spheroid ray neither met the ground nor left the top");
  }
  const depth: TwinRgb = [0, 0, 0];
  const columns = new Float64Array(ABSORBER_SLOTS);
  const pieces = Math.max(Math.ceil(endM / pieceM), 1);
  const lengthM = endM / pieces;
  const add = (at: number, weight: number): void => {
    sampleInto(terms, null, null, heightsAt(at)[0], sample);
    for (const c of CHANNELS) {
      depth[c] += weight * (sample[3 + c] ?? Number.NaN);
    }
    for (let k = 0; k < ABSORBER_SLOTS; k += 1) {
      columns[k] = (columns[k] ?? 0) + weight * (sample[COLUMNS_AT + k] ?? Number.NaN);
    }
  };
  for (let p = 0; p < pieces; p += 1) {
    const middle = (p + 0.5) * lengthM;
    for (const [i, node] of GAUSS_8_NODES.entries()) {
      const offset = 0.5 * lengthM * node;
      const weight = 0.5 * lengthM * (GAUSS_8_WEIGHTS[i] ?? Number.NaN);
      add(middle - offset, weight);
      add(middle + offset, weight);
    }
  }
  return { opticalDepth: [depth[0], depth[1], depth[2]], columns, lengthM: endM, hitsGround };
}

// --- The oblate per-frame marches (R08.T6.e) -----------------------------------------------------

/** An oblate body's camera: its latitude, s there, and the atmosphere's geometric height above it. */
interface OblateCamera {
  readonly latitudeRad: number;
  readonly gravityRatio: number;
  /** The medium's top over s at the camera, m: the per-frame spheres' atmosphere. */
  readonly atmosphereM: number;
}

/** The view's {@link OblateCamera}. */
function oblateCameraOf(planet: OblatePlanetTwin, view: TwinView): OblateCamera {
  const latitudeRad = latitudeOfNormal(view.normal);
  const s = gravityRatio(planet.body.gravity, latitudeRad);
  return { latitudeRad, gravityRatio: s, atmosphereM: planet.medium.topHeightM / s };
}

/**
 * An oblate sky-view texel's ray ({@link oblateSkyViewRayTwin}): the ray in the sky view's frame,
 * z the datum's normal at the camera and x the sky axis, and the osculating sphere R_α it is
 * marched on.
 */
export interface OblateSkyViewRay {
  readonly ray: TwinRay;
  /** R_α of the texel's column at the camera, m: the sphere's ground. */
  readonly bottomRadiusM: number;
  /** The atmosphere's height on it, the medium's top over the camera's s, m. */
  readonly atmosphereM: number;
}

/** The sky view's frame at a view: z the datum's normal, x the sky axis, y = z × x. */
function skyFrameAxes(view: TwinView): { readonly x: Vec3; readonly y: Vec3; readonly z: Vec3 } {
  const n = view.normal;
  const ax = view.skyAxis;
  return {
    x: ax,
    y: { x: n.y * ax.z - n.z * ax.y, y: n.z * ax.x - n.x * ax.z, z: n.x * ax.y - n.y * ax.x },
    z: n,
  };
}

/**
 * The ray of the sky-view texel (x, y) on an oblate body (plan R08, Design note 17, _Per-view
 * marches_; R08.T6.e): {@link skyViewRayTwin}'s parameterisation on the osculating sphere R_α of the
 * texel's column, α its local azimuth from north at the camera, and the atmosphere's height there
 * the medium's top over the camera's s, the camera kept 1 m inside.
 */
export function oblateSkyViewRayTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  x: number,
  y: number,
  size: TableSize,
): OblateSkyViewRay {
  const camera = oblateCameraOf(planet, view);
  const axes = skyFrameAxes(view);
  const { widthTexels: w, heightTexels: h } = size;
  const phi = 2 * Math.PI * unitOfTexel(x, w);
  const horizontal = {
    x: Math.cos(phi) * axes.x.x + Math.sin(phi) * axes.y.x,
    y: Math.cos(phi) * axes.x.y + Math.sin(phi) * axes.y.y,
    z: Math.cos(phi) * axes.x.z + Math.sin(phi) * axes.y.z,
  };
  const bottom = curvatureAlongM(planet.body.figure, camera.latitudeRad, view.normal, horizontal);
  const heightM = skyViewHeight(view.heightM, camera.atmosphereM);
  const rM = bottom + heightM;
  const unitY = unitOfTexel(y, h);
  const vHorizon = Math.sqrt(Math.max(heightM * (2 * bottom + heightM), 0));
  const beta = Math.acos(Math.min(Math.max(vHorizon / rM, -1), 1));
  const zenithHorizon = Math.PI - beta;
  let viewZenithCos: number;
  if (unitY < 0.5) {
    const c = 1 - 2 * unitY;
    viewZenithCos = Math.cos(zenithHorizon * (1 - c * c));
  } else {
    const c = unitY * 2 - 1;
    viewZenithCos = Math.cos(zenithHorizon + beta * c * c);
  }
  const viewZenithSin = Math.sqrt(Math.max(1 - viewZenithCos * viewZenithCos, 0));
  const inFrame = (s: Vec3): Vec3 => ({
    x: s.x * axes.x.x + s.y * axes.x.y + s.z * axes.x.z,
    y: s.x * axes.y.x + s.y * axes.y.y + s.z * axes.y.z,
    z: s.x * axes.z.x + s.y * axes.z.y + s.z * axes.z.z,
  });
  const sources = view.sources.map((source) => ({
    ...source,
    direction: inFrame(source.direction),
  }));
  const shell = { bottomRadiusM: bottom, topRadiusM: bottom + camera.atmosphereM };
  return {
    ray: {
      originM: { x: 0, y: 0, z: rM },
      direction: {
        x: viewZenithSin * Math.cos(phi),
        y: viewZenithSin * Math.sin(phi),
        z: viewZenithCos,
      },
      sun: sources[0]?.direction ?? { x: 0, y: 0, z: 1 },
      sources,
      tStartM: 0,
      tEndM: maxDistanceM(shell, rM, viewZenithCos),
      fromCamera: true,
    },
    bottomRadiusM: bottom,
    atmosphereM: camera.atmosphereM,
  };
}

/**
 * The sky-view table's twin on an oblate body, texel by texel, each marched when first asked for
 * and kept (R08.T6.e): {@link oblateSkyViewRayTwin}'s ray at `samples` placed steps, cut at the
 * medium's layer edges, each at h\* ÷ the camera's s, on its sphere R_α; the medium at each
 * sample's h\*, its s at its own latitude, and the tables through the slices and bands.
 *
 * @throws RangeError as {@link skyViewTwin}.
 */
export function oblateSkyViewTexelsTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  size: TableSize,
  samples: number,
): (x: number, y: number) => Rgb {
  const march = oblateSkyViewMarcher(planet, view, samples);
  const kept = new Map<number, Rgb>();
  return (x, y) => {
    const key = y * size.widthTexels + x;
    let texel = kept.get(key);
    if (texel === undefined) {
      texel = march(oblateSkyViewRayTwin(planet, view, x, y, size));
      kept.set(key, texel);
    }
    return texel;
  };
}

/**
 * The march of an oblate sky-view ray on its sphere (R08.T6.e): {@link oblateSkyViewTexelsTwin}'s,
 * for a texel's own ray or one the caller has turned (`decision-r08-f32.md` §10's envelope, R08.T6.f's
 * smoke), its end where it leaves its sphere's shell or meets its ground.
 *
 * @param splitShift - Steps moved from the side after the ray's lowest point to the side before it
 *   (negative: back), each side keeping one: the split the kernel's `f32` may take where the
 *   twin's share stands at its rounding (R08.T6.f's smoke, {@link marchSplit}); 0 for the twin's own.
 */
export function oblateSkyViewMarchTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  ray: OblateSkyViewRay,
  samples: number,
  splitShift = 0,
): Rgb {
  return oblateSkyViewMarcher(planet, view, samples, splitShift)(ray);
}

/**
 * {@link rayPieces} with `shift` steps moved from the side after t* to the side before it, each side
 * that has length keeping one (see {@link oblateSkyViewMarchTwin}).
 */
function shiftedRayPieces(
  ray: TwinSegment,
  samples: number,
  edgesM: ReadonlyArray<number>,
  geometry: MarchGeometry,
  shift: number,
): MarchPieces {
  if (shift === 0) {
    return rayPieces(ray, samples, edgesM, geometry);
  }
  const o = ray.originM;
  const d = ray.direction;
  const nearestM = -(o.x * d.x + o.y * d.y + o.z * d.z);
  const split = marchSplit(ray.tStartM, ray.tEndM, nearestM, samples, ray.fromCamera);
  const least = split.beforeM > 0 ? 1 : 0;
  const most = samples - (split.afterM > 0 ? 1 : 0);
  const stepsBefore = Math.min(Math.max(split.stepsBefore + shift, least), most);
  const moved = { ...split, stepsBefore, stepsAfter: samples - stepsBefore };
  return marchPieces(moved, crossingsFromLowestM(ray, moved.lowestM, edgesM, geometry));
}

/** {@link oblateSkyViewMarchTwin} with what every ray of a view shares made once. */
function oblateSkyViewMarcher(
  planet: OblatePlanetTwin,
  view: TwinView,
  samples: number,
  splitShift = 0,
): (ray: OblateSkyViewRay) => Rgb {
  const terms = termsOf(planet.medium);
  const camera = oblateCameraOf(planet, view);
  const edgesM = marchEdgesOf(terms, planet.medium.topHeightM).map(
    (edge) => edge / camera.gravityRatio,
  );
  const pressureBarOf = pressureBarLookup(planet.medium);
  const reader = oblateReaderOf(planet);
  const axes = skyFrameAxes(view);
  const n = Math.max(Math.floor(samples), 2);
  return ({ ray, bottomRadiusM }) => {
    const geometry: MarchGeometry = { kind: "sphere", bottomRadiusM };
    // The sphere's centre in the body-fixed axes: the camera's geodetic height, not its height on
    // the sphere, below the camera, so that each sample's body-fixed place stands where the
    // sphere's march puts it, the camera moved along its normal to its height on the sphere, held
    // 1 m inside the shell (R08.T6.f). From above the air, the place stood the clamp higher, and
    // each lookup's far points with it: 84 km on a Saturn-class figure from 1,000 km.
    const reachM = bottomRadiusM + view.heightM;
    const frame: MarchFrame = {
      originM: {
        x: view.cameraM.x - reachM * axes.z.x,
        y: view.cameraM.y - reachM * axes.z.y,
        z: view.cameraM.z - reachM * axes.z.z,
      },
      ...axes,
    };
    const marcher = new Marcher(
      terms,
      pressureBarOf,
      { kind: "oblate", reader, frame, heights: "geometry" },
      geometry,
      ray,
    );
    const pieces = shiftedRayPieces(ray, n, edgesM, geometry, splitShift);
    for (const { tM, dtM } of marchPiecesSteps(pieces)) {
      marcher.step(tM, dtM);
    }
    const [r, g, b] = marcher.luminance;
    return [r, g, b];
  };
}

/**
 * The sky-view table's twin on an oblate body: {@link oblateSkyViewTexelsTwin}'s every texel,
 * alpha 1.
 *
 * @throws RangeError as {@link skyViewTwin}.
 */
export function oblateSkyViewTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  size: TableSize,
  samples: number,
): TwinTable {
  const texelAt = oblateSkyViewTexelsTwin(planet, view, size, samples);
  const { widthTexels: w, heightTexels: h } = size;
  const texels = new Float64Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      texels.set([...texelAt(x, y), 1], (y * w + x) * 4);
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

/**
 * The oblate sky view read in a direction (R08.T6.e): {@link skyViewReadTwin}'s read with the
 * horizon split on the osculating sphere R_α of the direction's own azimuth at the camera, and the
 * atmosphere's height the medium's top over the camera's s, as {@link oblateSkyViewRayTwin}
 * writes it.
 *
 * @param direction - A unit direction, body-fixed.
 */
export function oblateSkyViewReadTwin(
  texelAt: (x: number, y: number) => Rgb,
  size: TableSize,
  planet: OblatePlanetTwin,
  view: TwinView,
  direction: Vec3,
): Rgb {
  const camera = oblateCameraOf(planet, view);
  const n = view.normal;
  const d = direction;
  const mu = d.x * n.x + d.y * n.y + d.z * n.z;
  const flat = { x: d.x - mu * n.x, y: d.y - mu * n.y, z: d.z - mu * n.z };
  const flatLength = Math.hypot(flat.x, flat.y, flat.z);
  const horizontal = flatLength > 0 ? scale(flat, 1 / flatLength) : view.skyAxis;
  const bottom = curvatureAlongM(planet.body.figure, camera.latitudeRad, n, horizontal);
  return skyViewReadOn(texelAt, size, bottom, camera.atmosphereM, view, direction);
}

/**
 * The aerial-perspective volume's twin on an oblate body (plan R08, Design note 17, _Per-view
 * marches_; R08.T6.e): {@link aerialPerspectiveTwin}'s froxels, each ray marched on its own
 * osculating sphere R_α at the camera, the medium at each sample's h\*, its s at its own latitude,
 * and the tables through the slices and bands.
 *
 * @throws RangeError as {@link aerialPerspectiveTwin}.
 */
export function oblateAerialPerspectiveTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  size: VolumeSize,
  samplesPerSlice: number,
  reachM: number,
): TwinVolume {
  const terms = termsOf(planet.medium);
  const pressureBarOf = pressureBarLookup(planet.medium);
  const reader = oblateReaderOf(planet);
  const camera = oblateCameraOf(planet, view);
  const { widthTexels: w, heightTexels: h, slices } = size;
  const perSlice = Math.max(Math.floor(samplesPerSlice), 1);
  const dtM = reachM / (slices * perSlice);
  const n = view.normal;
  const texels = new Float64Array(w * h * slices * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const direction = unit(unnormalisedRay(view, unitOfTexel(x, w), unitOfTexel(y, h)));
      const bottom = curvatureAlongM(planet.body.figure, camera.latitudeRad, n, direction);
      const lift = bottom + Math.max(view.heightM, 0);
      const geometry: MarchGeometry = { kind: "sphere", bottomRadiusM: bottom };
      const frame: MarchFrame = {
        ...BODY_FRAME,
        originM: {
          x: view.cameraM.x - n.x * lift,
          y: view.cameraM.y - n.y * lift,
          z: view.cameraM.z - n.z * lift,
        },
      };
      const ray: TwinRay = {
        originM: { x: n.x * lift, y: n.y * lift, z: n.z * lift },
        direction,
        sun: view.sun,
        sources: view.sources,
        tStartM: 0,
        tEndM: reachM,
        fromCamera: true,
      };
      const marcher = new Marcher(
        terms,
        pressureBarOf,
        { kind: "oblate", reader, frame, heights: "geometry" },
        geometry,
        ray,
      );
      for (let slice = 0; slice < slices; slice += 1) {
        for (let step = 0; step < perSlice; step += 1) {
          marcher.step((slice * perSlice + step + 0.5) * dtM, dtM);
        }
        const t = marcher.throughput;
        texels.set([...marcher.luminance, (t[0] + t[1] + t[2]) / 3], ((slice * h + y) * w + x) * 4);
      }
    }
  }
  return { widthTexels: w, heightTexels: h, slices, texels };
}

/** 1 ÷ s at the equator and at the pole: the oblate ray march's shells' scales (Design note 17). */
function shellScalesOf(planet: OblatePlanetTwin): readonly [number, number] {
  const gravity = planet.body.gravity;
  return [1 / gravityRatio(gravity, 0), 1 / gravityRatio(gravity, Math.PI / 2)];
}

/**
 * A ray from `originM` along the unit `direction` clipped as the oblate ray march clips it
 * (R08.T6.e): {@link rayMarchRayAlongTwin}'s, its top the quadric through h\* = `topHeightM` at the
 * equator and the poles, (a + H ÷ s_e, c + H ÷ s_p), as R08.T12.d's tracer takes it.
 */
export function oblateRayMarchRayAlongTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  originM: Vec3,
  direction: Vec3,
  tSurfaceM: number,
): RayMarchRay | null {
  const topM = planet.medium.topHeightM;
  const [equatorial, polar] = shellScalesOf(planet);
  const a = view.figure.equatorialRadiusM;
  const c = view.figure.polarRadiusM;
  const [shellNear, shellFar] = spheroidHits(
    originM,
    direction,
    a + topM * equatorial,
    c + topM * polar,
  );
  const [groundNear] = spheroidHits(originM, direction, a, c);
  const tStartM = Math.max(shellNear, 0);
  const hitsGround = groundNear > 0;
  const tEndM = Math.min(hitsGround ? Math.min(shellFar, groundNear) : shellFar, tSurfaceM);
  if (shellFar <= 0 || tEndM <= tStartM) {
    return null;
  }
  return {
    ray: {
      originM,
      direction,
      sun: view.sun,
      sources: view.sources,
      tStartM,
      tEndM,
      fromCamera: shellNear <= 0,
    },
    bareGroundM:
      !Number.isFinite(tSurfaceM) && hitsGround && groundNear <= tEndM ? groundNear : null,
  };
}

/**
 * The oblate ray-march texel's ray at `u`, `v` of the target, where the scene's reversed-Z depth is
 * `depth` (R08.T6.e): {@link rayMarchRayTwin}'s, the camera above the atmosphere where its h\* is,
 * clipped by {@link oblateRayMarchRayAlongTwin}.
 */
export function oblateRayMarchRayTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  u: number,
  v: number,
  depth: number,
  reachM: number,
): RayMarchRay | null {
  const camera = oblateCameraOf(planet, view);
  const aboveTop = camera.gravityRatio * view.heightM >= planet.medium.topHeightM;
  const unnormalised = unnormalisedRay(view, u, v);
  const lengthM = Math.sqrt(unnormalised.x ** 2 + unnormalised.y ** 2 + unnormalised.z ** 2);
  const tSurfaceM = depth > 0 ? (view.nearM / depth) * lengthM : Number.POSITIVE_INFINITY;
  if (!(aboveTop || (depth > 0 && tSurfaceM > reachM))) {
    return null;
  }
  return oblateRayMarchRayAlongTwin(planet, view, view.cameraM, unit(unnormalised), tSurfaceM);
}

/** What every oblate ray-march texel of a view reads (R08.T6.e). */
function oblateRayMarchContext(
  planet: OblatePlanetTwin,
  view: TwinView,
  settings: RayMarchSettings,
): RayMarchContext {
  const terms = termsOf(planet.medium);
  const [equatorialShellScale, polarShellScale] = shellScalesOf(planet);
  return {
    medium: planet.medium,
    terms,
    edgesM: marchEdgesOf(terms, planet.medium.topHeightM),
    pressureBarOf: pressureBarLookup(planet.medium),
    reads: {
      kind: "oblate",
      reader: oblateReaderOf(planet),
      frame: BODY_FRAME,
      heights: "geodetic",
    },
    geometry: {
      kind: "gravitySpheroid",
      figure: view.figure,
      equatorialShellScale,
      polarShellScale,
    },
    view,
    samples: Math.max(Math.floor(settings.rayMarchSamples), 2),
  };
}

/**
 * One oblate ray-march texel's twin along a clipped ray (R08.T6.e): {@link rayMarchTexelTwin}'s, on
 * R05's spheroid shells through h\* at the equator and the poles, the medium at each sample's h\* of
 * its geodetic height, its s at its own latitude, and the tables through the slices and bands; the
 * bare ground lit through the sliced lookup from its own latitude.
 *
 * @throws RangeError as {@link rayMarchTexelTwin}.
 */
export function oblateRayMarchTexelTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  settings: RayMarchSettings,
  marched: RayMarchRay | null,
): readonly [number, number, number, number] {
  const out = new Float64Array(4);
  rayMarchTexelInto(oblateRayMarchContext(planet, view, settings), marched, out);
  return [out[0] ?? Number.NaN, out[1] ?? Number.NaN, out[2] ?? Number.NaN, out[3] ?? Number.NaN];
}

/**
 * The oblate ray-march target's twin (R08.T6.e): {@link rayMarchTwin}'s texels, each by
 * {@link oblateRayMarchRayTwin} and {@link oblateRayMarchTexelTwin}.
 *
 * @throws RangeError as {@link rayMarchTwin}.
 */
export function oblateRayMarchTwin(
  planet: OblatePlanetTwin,
  view: TwinView,
  settings: RayMarchSettings,
  depthAt: (xPx: number, yPx: number) => number,
): TwinTable {
  const context = oblateRayMarchContext(planet, view, settings);
  const fullW = view.viewport.widthPx;
  const fullH = view.viewport.heightPx;
  const w = Math.ceil(fullW * settings.rayMarchScale);
  const h = Math.ceil(fullH * settings.rayMarchScale);
  const texels = new Float64Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const u = (x + 0.5) / w;
      const v = (y + 0.5) / h;
      const depth = depthAt(
        Math.min(Math.floor(u * fullW), fullW - 1),
        Math.min(Math.floor(v * fullH), fullH - 1),
      );
      const marched = oblateRayMarchRayTwin(
        planet,
        view,
        u,
        v,
        depth,
        settings.aerialPerspectiveReachM,
      );
      rayMarchTexelInto(context, marched, texels.subarray((y * w + x) * 4, (y * w + x) * 4 + 4));
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

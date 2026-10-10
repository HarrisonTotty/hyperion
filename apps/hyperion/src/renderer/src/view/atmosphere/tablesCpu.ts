/**
 * The CPU twin of the atmosphere's kernels in `f64` (plan R08, R08.T6.a): Hillaire's per-planet
 * transmittance and multiple-scattering tables, the per-frame sky view and aerial perspective, and
 * the per-pixel ray march, over a medium of any number of terms, as R05's kernels compute them.
 *
 * @remarks
 * Each twin follows its kernel's WGSL step for step, at the kernel's sizes and step counts unless
 * a caller asks for others: the same parameterisations, the same midpoints and placed steps
 * (`marchSteps.ts`, R05.T12.e), the same bilinear reads of the tables with their texel centres at
 * (i + ½) ÷ size, and the same in-scattering per step, throughput · S · Δt · g(σ_t Δt) with
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
import {
  type AtmosphereCamera,
  atmosphereInputs,
  type SpheroidFigure,
  type SunState,
  type TableSizes,
  type VolumeSize,
} from "./hillaire";
import { marchSplit, marchStep, type MarchStep } from "./marchSteps";
import {
  type AtmosphereMedium,
  checkNoAbsorbers,
  checkTabulatedTops,
  type DensityProfile,
  densityAt,
  extinction,
  type PhaseFunction,
  phaseAt,
} from "./medium";
import {
  intersectsGround,
  localRadiusM,
  maxDistanceM,
  type Shell,
  transmittanceRMuToUv,
  transmittanceUvToRMu,
} from "./opticalDepth";
import { SKY_SPECTRAL_TO_LUMINANCE, SUN_SPECTRAL_TO_LUMINANCE } from "./solar";
import {
  MULTI_SCATTERING_SAMPLES,
  MULTI_SCATTERING_SIZE,
  type TableSize,
  TRANSMITTANCE_SAMPLES,
  TRANSMITTANCE_SIZE,
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

/** One term as the twin reads it: its profile, its coefficients at unit density and its phase. */
interface TwinTerm {
  readonly density: DensityProfile;
  readonly scattering: Rgb;
  readonly extinction: Rgb;
  readonly phase: PhaseFunction;
}

/**
 * The medium's terms with their extinctions, checked.
 *
 * @throws RangeError for a tabulated density whose last level lies below the medium's top with a
 *   density other than 0: it would hold that density up to the top (the `tabulated` rule), as a
 *   column does with its last level's 10⁻⁷ of the ground's (R08.T3.a), so a medium built on it
 *   takes a top no higher than its last level; and, as `checkNoAbsorbers`, for an absorber term,
 *   whose curves the twin reads only from R08.T6.c.
 */
function termsOf(medium: AtmosphereMedium): readonly TwinTerm[] {
  checkTabulatedTops(medium);
  checkNoAbsorbers(medium);
  return medium.terms.map((term) => ({
    density: term.density,
    scattering: term.scattering,
    extinction: extinction(term),
    phase: term.phase,
  }));
}

/**
 * The medium at a height into `out`: the scattering in 0–2 and the extinction in 3–5, m⁻¹, and,
 * given each term's phase per channel for the ray, the scattering towards the eye in 6–8,
 * m⁻¹ sr⁻¹ (`medium.wgsl`'s `mediumAt` and `source.wgsl`'s `sampleMediumAt`).
 */
function sampleInto(
  terms: readonly TwinTerm[],
  phases: Float64Array | null,
  heightM: number,
  out: Float64Array,
): void {
  out.fill(0);
  for (const [k, term] of terms.entries()) {
    const d = densityAt(term.density, heightM);
    for (const c of CHANNELS) {
      const scattered = term.scattering[c] * d;
      out[c] = (out[c] ?? 0) + scattered;
      out[3 + c] = (out[3 + c] ?? 0) + term.extinction[c] * d;
      if (phases !== null) {
        out[6 + c] = (out[6 + c] ?? 0) + scattered * (phases[k * 3 + c] ?? Number.NaN);
      }
    }
  }
}

/** Each term's phase per channel for one cosine, term k's at 3k–3k + 2. */
function phasesAt(terms: readonly TwinTerm[], cosTheta: number): Float64Array {
  const phases = new Float64Array(terms.length * 3);
  for (const [k, term] of terms.entries()) {
    phases.set(phaseAt(term.phase, cosTheta), k * 3);
  }
  return phases;
}

/** Bilinear read of a table's RGB, texel centres at (i + ½) ÷ size, clamped: `common.wgsl`'s. */
function bilinearInto(table: TwinTable, u: number, v: number, out: TwinRgb): void {
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
  const at = (x: number, y: number, c: number): number => texels[(y * w + x) * 4 + c] ?? Number.NaN;
  for (const c of CHANNELS) {
    const a = at(x0, y0, c) * (1 - fx) + at(x1, y0, c) * fx;
    const b = at(x0, y1, c) * (1 - fx) + at(x1, y1, c) * fx;
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

/** The optical depth along a ray by the transmittance kernel's midpoint rule, into `out`. */
function kernelOpticalDepthInto(
  terms: readonly TwinTerm[],
  shell: Shell,
  rM: number,
  mu: number,
  samples: number,
  out: TwinRgb,
  scratch: Float64Array,
): void {
  const n = Math.max(Math.floor(samples), 1);
  const dt = maxDistanceM(shell, rM, mu) / n;
  out.fill(0);
  for (let i = 0; i < n; i += 1) {
    sampleInto(terms, null, localRadiusM(rM, mu, (i + 0.5) * dt) - shell.bottomRadiusM, scratch);
    for (const c of CHANNELS) {
      out[c] += (scratch[3 + c] ?? Number.NaN) * dt;
    }
  }
}

/**
 * The optical depth per channel from radius `rM` along zenith cosine `mu` to the top or the
 * ground, by `transmittance.wgsl`'s rule: `samples` equal steps, each sampled at its midpoint.
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
  const out: TwinRgb = [0, 0, 0];
  const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  kernelOpticalDepthInto(termsOf(medium), shell, rM, mu, samples, out, new Float64Array(9));
  return out;
}

/**
 * The transmittance table's twin: e^(−τ) per channel at each texel's (r, μ) of Bruneton 2017's
 * parameterisation (`transmittanceUvToRMu`), alpha 1, as `transmittance.wgsl` stores it.
 *
 * @param bottomRadiusM - The tables' ground radius, m (`tableRadiusM(figure)` for a body).
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function transmittanceTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  size: TableSize = TRANSMITTANCE_SIZE,
  samples = TRANSMITTANCE_SAMPLES,
): TwinTable {
  const terms = termsOf(medium);
  const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM };
  const { widthTexels: w, heightTexels: h } = size;
  const texels = new Float64Array(w * h * 4);
  const depth: TwinRgb = [0, 0, 0];
  const scratch = new Float64Array(9);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const { rM, mu } = transmittanceUvToRMu(shell, (x + 0.5) / w, (y + 0.5) / h);
      kernelOpticalDepthInto(terms, shell, rM, mu, samples, depth, scratch);
      texels.set(
        [Math.exp(-depth[0]), Math.exp(-depth[1]), Math.exp(-depth[2]), 1],
        (y * w + x) * 4,
      );
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
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
   * `even`: the kernel's equal steps sampled at their midpoints. `placed`: R05.T12.e's steps toward
   * each ray's lowest point, or for the side before it toward the texel's point (`marchSplit` with
   * `fromCamera`), against which the kernel's are measured.
   */
  readonly placement: "even" | "placed";
  /**
   * The step factor g(x) = (1 − e^(−x)) ÷ x: {@link exactStepFactor} for the twin, or an `f32`
   * emulation of a kernel's form, through which a test measures what `f32` does to the table.
   */
  readonly stepFactor: (x: number) => number;
}

/**
 * The multiple-scattering kernel's own: Hillaire 2020's 32² (his Table 2, at 20 steps a ray) with
 * R05.T12.b's 32 even steps a ray.
 */
export const MULTI_SCATTERING_KERNEL_OPTIONS: MultiScatteringOptions = {
  size: MULTI_SCATTERING_SIZE,
  samples: MULTI_SCATTERING_SAMPLES,
  placement: "even",
  stepFactor: exactStepFactor,
};

/** What a multiple-scattering texel reads: the medium, its shell and the transmittance table. */
interface MultiScatteringInputs {
  readonly terms: readonly TwinTerm[];
  readonly shell: Shell;
  readonly groundAlbedo: Rgb;
  readonly transmittance: TwinTable;
  readonly options: MultiScatteringOptions;
}

/** The steps of one multiple-scattering ray of length `tMaxM` from radius `rM` at cosine `mu`. */
function multiScatteringSteps(
  options: MultiScatteringOptions,
  rM: number,
  mu: number,
  tMaxM: number,
): MarchStep[] {
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
  }
  return steps;
}

/**
 * One direction's integral (`multiScattering.wgsl`'s `integrate`): the luminance it brings per
 * unit illuminance into 0–2 (single scattering under the isotropic phase, and the ground's
 * Lambertian bounce) and f_ms's integrand into 3–5.
 */
function integrateDirectionInto(
  inputs: MultiScatteringInputs,
  rM: number,
  dir: Vec3,
  sun: Vec3,
  out: Float64Array,
  scratch: Float64Array,
  toSun: TwinRgb,
): void {
  const { terms, shell, transmittance, options } = inputs;
  const mu = dir.z;
  const tMax = maxDistanceM(shell, rM, mu);
  out.fill(0);
  const throughput: TwinRgb = [1, 1, 1];
  for (const { tM, dtM } of multiScatteringSteps(options, rM, mu, tMax)) {
    const px = tM * dir.x;
    const py = tM * dir.y;
    const pz = rM + tM * dir.z;
    const rP = Math.sqrt(px * px + py * py + pz * pz);
    const muSun = (sun.x * px + sun.y * py + sun.z * pz) / rP;
    sampleInto(terms, null, rP - shell.bottomRadiusM, scratch);
    const lit = intersectsGround(shell, rP, muSun) ? 0 : 1;
    const [u, v] = transmittanceRMuToUv(shell, rP, muSun);
    bilinearInto(transmittance, u, v, toSun);
    for (const c of CHANNELS) {
      const scattering = scratch[c] ?? Number.NaN;
      const stepDepth = (scratch[3 + c] ?? Number.NaN) * dtM;
      const factor = dtM * options.stepFactor(stepDepth);
      const source = (lit * toSun[c] * scattering) / (4 * Math.PI);
      out[c] = (out[c] ?? 0) + throughput[c] * source * factor;
      out[3 + c] = (out[3 + c] ?? 0) + throughput[c] * scattering * factor;
      throughput[c] *= Math.exp(-stepDepth);
    }
  }
  if (intersectsGround(shell, rM, mu)) {
    const gx = tMax * dir.x;
    const gy = tMax * dir.y;
    const gz = rM + tMax * dir.z;
    const muSun = (sun.x * gx + sun.y * gy + sun.z * gz) / Math.sqrt(gx * gx + gy * gy + gz * gz);
    const [u, v] = transmittanceRMuToUv(shell, shell.bottomRadiusM, muSun);
    bilinearInto(transmittance, u, v, toSun);
    for (const c of CHANNELS) {
      out[c] =
        (out[c] ?? 0) +
        (toSun[c] * throughput[c] * Math.max(muSun, 0) * inputs.groundAlbedo[c]) / Math.PI;
    }
  }
}

/**
 * One multiple-scattering texel's value, per unit illuminance, into `out`: the second order's
 * mean over the 64 directions times F_ms = 1 ÷ (1 − f_ms), every further order as a geometric
 * series (Hillaire 2020, eqs. 9 and 10).
 */
function multiScatteringTexelInto(
  inputs: MultiScatteringInputs,
  x: number,
  y: number,
  out: TwinRgb,
): void {
  const { widthTexels: w, heightTexels: h } = inputs.options.size;
  const { bottomRadiusM: bottom, topRadiusM: top } = inputs.shell;
  const unitX = Math.min(Math.max(((x + 0.5) / w - 0.5 / w) * (w / (w - 1)), 0), 1);
  const unitY = Math.min(Math.max(((y + 0.5) / h - 0.5 / h) * (h / (h - 1)), 0), 1);
  const rM =
    bottom +
    MULTI_SCATTERING_GROUND_OFFSET_M +
    unitY * (top - bottom - MULTI_SCATTERING_GROUND_OFFSET_M);
  const muSun = unitX * 2 - 1;
  const sun = { x: 0, y: Math.sqrt(Math.max(1 - muSun * muSun, 0)), z: muSun };
  const sum = new Float64Array(6);
  const one = new Float64Array(6);
  const scratch = new Float64Array(9);
  const toSun: TwinRgb = [0, 0, 0];
  for (let z = 0; z < MULTI_SCATTERING_DIRECTIONS; z += 1) {
    const a = (0.5 + Math.floor(z / SQRT_DIRECTIONS)) / SQRT_DIRECTIONS;
    const b = (0.5 + (z % SQRT_DIRECTIONS)) / SQRT_DIRECTIONS;
    const theta = 2 * Math.PI * a;
    const cosPhi = 1 - 2 * b;
    const sinPhi = Math.sqrt(Math.max(1 - cosPhi * cosPhi, 0));
    const dir = { x: Math.cos(theta) * sinPhi, y: Math.sin(theta) * sinPhi, z: cosPhi };
    integrateDirectionInto(inputs, rM, dir, sun, one, scratch, toSun);
    for (let i = 0; i < 6; i += 1) {
      sum[i] = (sum[i] ?? 0) + (one[i] ?? Number.NaN);
    }
  }
  for (const c of CHANNELS) {
    const secondOrder = (sum[c] ?? Number.NaN) / MULTI_SCATTERING_DIRECTIONS;
    const fMs = (sum[3 + c] ?? Number.NaN) / MULTI_SCATTERING_DIRECTIONS;
    out[c] = secondOrder / (1 - fMs);
  }
}

/**
 * The multiple-scattering table's twin: at each texel's height and sun zenith cosine (sebh's
 * mapping, its ground {@link MULTI_SCATTERING_GROUND_OFFSET_M} up), the luminance that every order
 * past the first adds per unit illuminance under an isotropic phase, alpha 1, as
 * `multiScattering.wgsl` stores it. It reads `transmittance` as the kernel reads its table.
 *
 * @param bottomRadiusM - The tables' ground radius, m, the transmittance table's own.
 * @param texels - The texels to compute, each [x, y]; every texel by default. Texels not listed
 *   are left 0.
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks), or from `marchSplit` for placed
 *   steps of fewer than 2.
 */
export function multiScatteringTwin(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  transmittance: TwinTable,
  options: MultiScatteringOptions = MULTI_SCATTERING_KERNEL_OPTIONS,
  texels?: ReadonlyArray<readonly [number, number]>,
): TwinTable {
  const inputs: MultiScatteringInputs = {
    terms: termsOf(medium),
    shell: { bottomRadiusM, topRadiusM: bottomRadiusM + medium.topHeightM },
    groundAlbedo: medium.groundAlbedo,
    transmittance,
    options,
  };
  const { widthTexels: w, heightTexels: h } = options.size;
  const out = new Float64Array(w * h * 4);
  const value: TwinRgb = [0, 0, 0];
  const which =
    texels ?? Array.from({ length: w * h }, (_, i) => [i % w, Math.floor(i / w)] as const);
  for (const [x, y] of which) {
    multiScatteringTexelInto(inputs, x, y, value);
    out.set([value[0], value[1], value[2], 1], (y * w + x) * 4);
  }
  return { widthTexels: w, heightTexels: h, texels: out };
}

// --- The per-planet tables and their reads -------------------------------------------------------

/** A planet's two per-planet tables in `f64`, at the kernels' sizes and steps. */
export interface PlanetTwin {
  readonly medium: AtmosphereMedium;
  /** The tables' ground radius, m. */
  readonly bottomRadiusM: number;
  readonly transmittance: TwinTable;
  readonly multiScattering: TwinTable;
}

/**
 * Both per-planet tables for a medium, as `AtmosphereTables` builds them: transmittance at 256 × 64
 * with 256 steps, then multiple scattering at 32² with 32 even steps reading it.
 *
 * @param bottomRadiusM - The tables' ground radius, m (`tableRadiusM(figure)` for a body).
 * @throws RangeError for a medium with a tabulated term that ends below its top at a density
 *   other than 0 (the module's remarks).
 */
export function planetTwin(medium: AtmosphereMedium, bottomRadiusM: number): PlanetTwin {
  const transmittance = transmittanceTwin(medium, bottomRadiusM);
  return {
    medium,
    bottomRadiusM,
    transmittance,
    multiScattering: multiScatteringTwin(medium, bottomRadiusM, transmittance),
  };
}

/**
 * What a march reads the per-planet tables through, at a height above the datum: the twin's own
 * tables ({@link tableSources}), or a test's stand-in.
 */
export interface TwinSources {
  /** The tables' ground radius, m: the sphere the sun's ground shadow is tested on. */
  readonly bottomRadiusM: number;
  /** The transmittance from a height to the top along the sun's zenith cosine, into `out`. */
  readonly sunTransmittance: (heightM: number, muSun: number, out: TwinRgb) => void;
  /** The multiple-scattering source per unit illuminance at a height, into `out`. */
  readonly multiScattering: (heightM: number, muSun: number, out: TwinRgb) => void;
}

/**
 * A planet's tables read as the per-frame kernels read them: `source.wgsl`'s `tableTransmittance`
 * (the height clamped into the shell) and `tableMultiScattering` (sebh's mapping, its sub-texel
 * remap), each bilinear.
 */
export function tableSources(planet: PlanetTwin): TwinSources {
  const bottom = planet.bottomRadiusM;
  const atmosphereM = planet.medium.topHeightM;
  const shell = { bottomRadiusM: bottom, topRadiusM: bottom + atmosphereM };
  const table = planet.multiScattering;
  const w = table.widthTexels;
  const h = table.heightTexels;
  return {
    bottomRadiusM: bottom,
    sunTransmittance: (heightM, muSun, out) => {
      const rM = bottom + Math.min(Math.max(heightM, 0), atmosphereM);
      const [u, v] = transmittanceRMuToUv(shell, rM, muSun);
      bilinearInto(planet.transmittance, u, v, out);
    },
    multiScattering: (heightM, muSun, out) => {
      const v =
        (Math.max(heightM, 0) - MULTI_SCATTERING_GROUND_OFFSET_M) /
        (atmosphereM - MULTI_SCATTERING_GROUND_OFFSET_M);
      const unitX = Math.min(Math.max(muSun * 0.5 + 0.5, 0), 1);
      const unitY = Math.min(Math.max(v, 0), 1);
      bilinearInto(
        table,
        (unitX + 0.5 / w) * (w / (w + 1)),
        (unitY + 0.5 / h) * (h / (h + 1)),
        out,
      );
    },
  };
}

// --- The per-frame marches -----------------------------------------------------------------------

/** One ray of a per-frame march: its origin from the centre of what it is marched about. */
export interface TwinRay {
  readonly originM: Vec3;
  /** The unit direction of the ray. */
  readonly direction: Vec3;
  /** The unit direction to the sun. */
  readonly sun: Vec3;
  /** The segment marched, m along the ray. */
  readonly tStartM: number;
  readonly tEndM: number;
  /** Whether the segment starts at a camera inside the atmosphere (`marchSplit`'s `fromCamera`). */
  readonly fromCamera: boolean;
}

/**
 * What a march takes a sample's height and sun cosine against: a sphere about the centre (the
 * sky view's and aerial perspective's, of the camera's Gaussian radius √(MN)), or the datum
 * spheroid (the ray march's, the height along the radius and the normal of the similar spheroid
 * through the sample, as `rayMarch.wgsl`'s `heightAbove` and `normalAt`).
 */
export type MarchGeometry =
  | { readonly kind: "sphere"; readonly bottomRadiusM: number }
  | { readonly kind: "spheroid"; readonly figure: SpheroidFigure };

/** A march's result: the in-scattered radiance per unit sun illuminance and the transmittance. */
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
 * Whether the ground of a sphere of radius `bottomRadiusM` hides the sun from a height above it at
 * the sun's zenith cosine: `common.wgsl`'s `shellIntersectsGround` at r = R + max(h, 0), as
 * `source.wgsl`'s `sourceAt` tests it.
 */
function groundShadows(bottomRadiusM: number, heightM: number, muSun: number): boolean {
  const rM = bottomRadiusM + Math.max(heightM, 0);
  return muSun < 0 && rM * rM * (muSun * muSun - 1) + bottomRadiusM * bottomRadiusM >= 0;
}

/** A per-frame march in progress: the accumulated radiance and throughput along one ray. */
class Marcher {
  readonly luminance: TwinRgb = [0, 0, 0];
  readonly throughput: TwinRgb = [1, 1, 1];
  readonly #terms: readonly TwinTerm[];
  readonly #phases: Float64Array;
  readonly #sources: TwinSources;
  readonly #geometry: MarchGeometry;
  readonly #ray: TwinRay;
  readonly #local = new Float64Array(9);
  readonly #toSun: TwinRgb = [0, 0, 0];
  readonly #multiple: TwinRgb = [0, 0, 0];

  constructor(
    terms: readonly TwinTerm[],
    sources: TwinSources,
    geometry: MarchGeometry,
    ray: TwinRay,
  ) {
    this.#terms = terms;
    this.#sources = sources;
    this.#geometry = geometry;
    this.#ray = ray;
    const d = ray.direction;
    const s = ray.sun;
    this.#phases = phasesAt(terms, d.x * s.x + d.y * s.y + d.z * s.z);
  }

  /**
   * One step at distance `tM` along the ray, `dtM` long: the medium there, its scattering source
   * (`source.wgsl`'s `sourceAt`: single scattering of the sun's light shadowed by the ground on
   * the tables' sphere, plus multiple scattering) and the step's in-scattering.
   */
  step(tM: number, dtM: number): void {
    const { originM: o, direction: d, sun: s } = this.#ray;
    const px = o.x + tM * d.x;
    const py = o.y + tM * d.y;
    const pz = o.z + tM * d.z;
    let heightM: number;
    let muSun: number;
    const geometry = this.#geometry;
    switch (geometry.kind) {
      case "sphere": {
        const rP = Math.sqrt(px * px + py * py + pz * pz);
        heightM = rP - geometry.bottomRadiusM;
        muSun = (s.x * px + s.y * py + s.z * pz) / rP;
        break;
      }
      case "spheroid": {
        const a2 = geometry.figure.equatorialRadiusM ** 2;
        const c2 = geometry.figure.polarRadiusM ** 2;
        const rP = Math.sqrt(px * px + py * py + pz * pz);
        const surfaceM =
          1 / Math.sqrt((px * px + py * py) / (rP * rP * a2) + (pz * pz) / (rP * rP * c2));
        heightM = rP - surfaceM;
        const nx = px / a2;
        const ny = py / a2;
        const nz = pz / c2;
        muSun = (s.x * nx + s.y * ny + s.z * nz) / Math.sqrt(nx * nx + ny * ny + nz * nz);
        break;
      }
    }
    const local = this.#local;
    sampleInto(this.#terms, this.#phases, heightM, local);
    const sources = this.#sources;
    const lit = groundShadows(sources.bottomRadiusM, heightM, muSun) ? 0 : 1;
    sources.sunTransmittance(heightM, muSun, this.#toSun);
    sources.multiScattering(heightM, muSun, this.#multiple);
    for (const c of CHANNELS) {
      const source =
        lit * this.#toSun[c] * (local[6 + c] ?? Number.NaN) +
        this.#multiple[c] * (local[c] ?? Number.NaN);
      const stepDepth = (local[3 + c] ?? Number.NaN) * dtM;
      this.luminance[c] += this.throughput[c] * source * dtM * exactStepFactor(stepDepth);
      this.throughput[c] *= Math.exp(-stepDepth);
    }
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
  const marcher = new Marcher(termsOf(medium), sources, geometry, ray);
  for (const { tM, dtM } of steps) {
    marcher.step(tM, dtM);
  }
  return { luminance: [...marcher.luminance], transmittance: [...marcher.throughput] };
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
  /** The unit direction to the sun. */
  readonly sun: Vec3;
  /** The ray basis: right × tan(fov_x ÷ 2), up × tan(fov_y ÷ 2), and forward. */
  readonly right: Vec3;
  readonly upRay: Vec3;
  readonly forward: Vec3;
  readonly figure: SpheroidFigure;
  /** The projection's near plane, for distances from depth. */
  readonly nearM: number;
  readonly viewport: ViewSize;
  /** The sun's spectral-to-luminance factors over the sky's, for the ground the march sees. */
  readonly sunOverSky: Rgb;
}

/** A frame's view from its camera and sun, as `HillaireAtmosphere.drawFrame` fills its uniform. */
export function twinView(
  camera: AtmosphereCamera,
  sun: SunState,
  figure: SpheroidFigure,
  nearM: number,
): TwinView {
  const inputs = atmosphereInputs(camera, figure);
  const tanX = Math.tan(camera.fovXRad / 2);
  const tanY = tanX / (camera.viewport.widthPx / camera.viewport.heightPx);
  return {
    cameraM: camera.positionM,
    heightM: inputs.heightM,
    normal: inputs.normal,
    cameraRadiusM: inputs.radiusM - inputs.heightM,
    sun: sun.directionBodyFixed,
    right: scale(rotate(camera.orientation, { x: 1, y: 0, z: 0 }), tanX),
    upRay: scale(rotate(camera.orientation, { x: 0, y: 1, z: 0 }), tanY),
    forward: rotate(camera.orientation, { x: 0, y: 0, z: -1 }),
    figure,
    nearM,
    viewport: camera.viewport,
    sunOverSky: [
      SUN_SPECTRAL_TO_LUMINANCE[0] / SKY_SPECTRAL_TO_LUMINANCE[0],
      SUN_SPECTRAL_TO_LUMINANCE[1] / SKY_SPECTRAL_TO_LUMINANCE[1],
      SUN_SPECTRAL_TO_LUMINANCE[2] / SKY_SPECTRAL_TO_LUMINANCE[2],
    ],
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
 * The ray of the sky-view texel (x, y), as `skyView.wgsl` marches it: sebh's parameterisation
 * (`view.wgsl`'s `skyViewUvToParams`) on the camera's own sphere, the camera kept 1 m inside the
 * shell, in the frame whose z is the datum's normal and whose x points to the sun's azimuth.
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
  const heightM = Math.min(Math.max(view.heightM, 1), atmosphereM - 1);
  const rM = bottom + heightM;
  const { widthTexels: w, heightTexels: h } = size;
  const unitX = Math.min(Math.max(((x + 0.5) / w - 0.5 / w) * (w / (w - 1)), 0), 1);
  const unitY = Math.min(Math.max(((y + 0.5) / h - 0.5 / h) * (h / (h - 1)), 0), 1);
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
  const lightViewCos = -(unitX * unitX * 2 - 1);
  const viewZenithSin = Math.sqrt(Math.max(1 - viewZenithCos * viewZenithCos, 0));
  const n = view.normal;
  const s = view.sun;
  const muSunCamera = n.x * s.x + n.y * s.y + n.z * s.z;
  const shell = { bottomRadiusM: bottom, topRadiusM: bottom + atmosphereM };
  return {
    originM: { x: 0, y: 0, z: rM },
    direction: {
      x: viewZenithSin * lightViewCos,
      y: viewZenithSin * Math.sqrt(Math.max(1 - lightViewCos * lightViewCos, 0)),
      z: viewZenithCos,
    },
    sun: { x: Math.sqrt(Math.max(1 - muSunCamera * muSunCamera, 0)), y: 0, z: muSunCamera },
    tStartM: 0,
    tEndM: maxDistanceM(shell, rM, viewZenithCos),
    fromCamera: true,
  };
}

/**
 * The sky-view table's twin: each texel's ray marched at `samples` placed steps (at least 2) on
 * the camera's own sphere, reading the planet's tables at each sample's height, alpha 1.
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
  const terms = termsOf(planet.medium);
  const sources = tableSources(planet);
  const geometry: MarchGeometry = { kind: "sphere", bottomRadiusM: view.cameraRadiusM };
  const n = Math.max(Math.floor(samples), 2);
  const { widthTexels: w, heightTexels: h } = size;
  const texels = new Float64Array(w * h * 4);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const ray = skyViewRayTwin(planet.medium, view, x, y, size);
      const marcher = new Marcher(terms, sources, geometry, ray);
      for (const { tM, dtM } of placedSteps(ray, n)) {
        marcher.step(tM, dtM);
      }
      texels.set([...marcher.luminance, 1], (y * w + x) * 4);
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

/**
 * The aerial-perspective volume's twin: for each froxel column, the running in-scattered radiance
 * per unit sun illuminance (rgb) and the mean transmittance (a) at each slice's far face, out to
 * `reachM` in slices of equal depth with `samplesPerSlice` even steps each (at least 1), on the
 * camera's own sphere, as `aerialPerspective.wgsl` stores them.
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
      const direction = unit(unnormalisedRay(view, (x + 0.5) / w, (y + 0.5) / h));
      const ray: TwinRay = {
        originM,
        direction,
        sun: view.sun,
        tStartM: 0,
        tEndM: reachM,
        fromCamera: true,
      };
      const marcher = new Marcher(terms, sources, geometry, ray);
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
  const direction = unit(unnormalised);
  const o = view.cameraM;
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
    ray: { originM: o, direction, sun: view.sun, tStartM, tEndM, fromCamera: shellNear <= 0 },
    bareGroundM: depth === 0 && hitsGround && groundNear <= tEndM ? groundNear : null,
  };
}

/** The ray march's settings: `TableSizes`' scale, steps and reach. */
export type RayMarchSettings = Pick<
  TableSizes,
  "rayMarchScale" | "rayMarchSamples" | "aerialPerspectiveReachM"
>;

/**
 * The ray-march target's twin at the setting's scale of the viewport: each texel's in-scattered
 * radiance per unit sun illuminance (rgb) and mean transmittance (a), with the bare ground seen
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
  const medium = planet.medium;
  const terms = termsOf(medium);
  const sources = tableSources(planet);
  const geometry: MarchGeometry = { kind: "spheroid", figure: view.figure };
  const fullW = view.viewport.widthPx;
  const fullH = view.viewport.heightPx;
  const w = Math.ceil(fullW * settings.rayMarchScale);
  const h = Math.ceil(fullH * settings.rayMarchScale);
  const n = Math.max(Math.floor(settings.rayMarchSamples), 2);
  const texels = new Float64Array(w * h * 4);
  const toSun: TwinRgb = [0, 0, 0];
  const a2 = view.figure.equatorialRadiusM ** 2;
  const c2 = view.figure.polarRadiusM ** 2;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const u = (x + 0.5) / w;
      const v = (y + 0.5) / h;
      const depth = depthAt(
        Math.min(Math.floor(u * fullW), fullW - 1),
        Math.min(Math.floor(v * fullH), fullH - 1),
      );
      const at = (y * w + x) * 4;
      const marched = rayMarchRayTwin(medium, view, u, v, depth, settings.aerialPerspectiveReachM);
      if (marched === null) {
        texels.set([0, 0, 0, 1], at);
        continue;
      }
      const { ray, bareGroundM } = marched;
      const marcher = new Marcher(terms, sources, geometry, ray);
      for (const { tM, dtM } of placedSteps(ray, n)) {
        marcher.step(tM, dtM);
      }
      const luminance = marcher.luminance;
      const t = marcher.throughput;
      let meanTransmittance = (t[0] + t[1] + t[2]) / 3;
      if (bareGroundM !== null) {
        const o = ray.originM;
        const d = ray.direction;
        const s = ray.sun;
        const nx = (o.x + bareGroundM * d.x) / a2;
        const ny = (o.y + bareGroundM * d.y) / a2;
        const nz = (o.z + bareGroundM * d.z) / c2;
        const muSun = (s.x * nx + s.y * ny + s.z * nz) / Math.sqrt(nx * nx + ny * ny + nz * nz);
        sources.sunTransmittance(0, muSun, toSun);
        for (const c of CHANNELS) {
          luminance[c] +=
            (t[c] * toSun[c] * Math.max(muSun, 0) * medium.groundAlbedo[c] * view.sunOverSky[c]) /
            Math.PI;
        }
        meanTransmittance = 0;
      }
      texels.set([...luminance, meanTransmittance], at);
    }
  }
  return { widthTexels: w, heightTexels: h, texels };
}

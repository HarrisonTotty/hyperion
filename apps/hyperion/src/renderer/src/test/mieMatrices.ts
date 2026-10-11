/**
 * The Mie-matrix writer of the reference tracer's sphere benchmarks (plan R08, R08.T12.e): the
 * scattering matrices of Kokhanovsky et al. 2010's aerosol and cloud and of IPRT Phase A's spheres
 * (A3, A5), from the client's Mie over each case's stated size distribution, written onto the
 * `tabulated` phase that the tracer reads (`crates/hyperion-fit/src/atmosphere/case.rs`).
 *
 * @remarks
 * **The matrix.** Every sphere's matrix is R08.T5.b's {@link sphereScatteringMatrix} of R08.T5.a's
 * {@link mieSphere}, so that the benchmarks check the client's elements, b₂'s sign among them
 * (R08's Risks, "Deviations in T5.b, as built"). A case's matrix is their mean weighted by each
 * node's number weight times its scattering cross-section, x² Q_sca at one wavelength, so that it
 * keeps the normalisation ∫ a₁ dΩ ÷ 4π = 1; its single-scattering albedo and asymmetry parameter
 * are the same sums' (the "source's mean cosine from T5's Mie g", not the table's).
 *
 * **The sums over size** follow each benchmark's own definition (`decision-r08-licences.md` row
 * 6c: the matrices are recomputed from the stated distributions, never copied):
 *
 * - Kokhanovsky et al. 2010 (JQSRT 111, 1931, §3, eq. (10)): log-normal radii, integrated from
 *   0.005 µm to a_end. Their matrices came from Mishchenko's `spher.f`, whose quadrature is not
 *   stated, so the integral is converged here by the composite Gauss–Legendre rule of R08.T5.b
 *   (order {@link SIZE_PANEL_ORDER} in ln r, panels no wider than
 *   {@link SIZE_PANEL_SIZE_PARAMETER} in size parameter and {@link SIZE_PANEL_LN_RADIUS} in ln r,
 *   halved `level` times). Non-absorbing spheres' resonances make it converge slowly: the measured
 *   changes are in {@link SPHERE_CASES}' comments. The rule is restated here rather than called, so
 *   that a retuning of T5.b's bake (its panel widths or levels) cannot move the blessed benchmark
 *   tables unseen.
 * - IPRT Phase A (Emde et al. 2015, JQSRT 164, 8; arXiv:1901.01813): its matrices were computed by
 *   libRadtran's Mie tool, whose parameters its optical-property files record (`waso.mie.cdf`,
 *   `watercloud.mie.cdf`: the radius parameter, `n_r_max` and `dx_max`). The writer repeats that
 *   tool's sampling of the size distribution as a stated method, not as code
 *   (`decision-r08-cmf-licence.md` §6). The method is F. Evans's from SHDOM's Mie tables, which
 *   step by Δx = max(0.01, 0.03 √x), and which libRadtran credits and gives a single parameter.
 *   libRadtran 2.0.6's `src/mie.c` (GPL-2.0-or-later) was read for it alone, and nothing of it is
 *   copied: radii from 0.02 r to n_r_max r, where r is the file's radius parameter, in
 *   steps of Δx = max(dx_max, dx_max √x) in size parameter, the last radius the last at or below
 *   n_r_max r, and the trapezoid rule in r over them. A3's distribution is cut there while its
 *   integrand is still large, and the grid's last radius sets its forward peak, so the exact
 *   integral to n_r_max r would not be IPRT's matrix (g 0.5544 against IPRT's 0.5498); and for
 *   A5's water, whose resonances no feasible rule resolves, the composite rule above stays 0.3% of
 *   a₁ from IPRT's matrix at 50,240 nodes, where libRadtran's sum reproduces it to 2 × 10⁻⁴.
 *
 * **The table** is even in u = √(Θ ÷ π), from exactly 0 to exactly 1, with each element divided
 * by 4π (sr⁻¹) and then by the table's own integral over the sphere read linearly in u, the
 * tracer's rule (R08's Risks, "Deviations in T12.c, as built": a pointwise table of a forward
 * peak integrates to more than 1). Values are rounded to {@link SIGNIFICANT_DIGITS} significant
 * digits, which moves the integral by under 10⁻⁹ and keeps |b₁|, |b₂|, |a₃| ≤ a₁ within the
 * tracer's 10⁻⁹ allowance.
 */

import { gaussLegendre } from "../view/lighting/quadrature";
import { type ComplexIndex, mieSphere } from "../view/atmosphere/mie";
import {
  SIZE_PANEL_LN_RADIUS,
  SIZE_PANEL_ORDER,
  SIZE_PANEL_SIZE_PARAMETER,
  type ScatteringMatrix,
  sphereScatteringMatrix,
} from "../view/atmosphere/sizeDistribution";

/** The significant digits each table value is written to. */
export const SIGNIFICANT_DIGITS = 10;

/**
 * A distribution of sphere radii, as a number density per unit radius up to a constant.
 *
 * - `logNormal`: n(r) ∝ r⁻¹ exp(−½ (ln(r ÷ r_m) ÷ s)²), with mode radius r_m (the median, the
 *   peak in ln r) and s the standard deviation of ln r (Kokhanovsky et al.'s a₀ and s;
 *   libRadtran's, which takes its stated radius as r_m, with s = ln α, α its geometric standard
 *   deviation).
 * - `gamma`: n(r) ∝ r^α e^(−(α + 3) r ÷ r_eff), libRadtran's form, which is Hansen and Travis
 *   1974's standard gamma distribution (Space Sci. Rev. 16, 527, eq. (2.56)) with effective
 *   variance 1 ÷ (α + 3).
 */
export type RadiusDistribution =
  | {
      readonly kind: "logNormal";
      readonly modeRadiusUm: number;
      readonly lnSigma: number;
    }
  | {
      readonly kind: "gamma";
      readonly effectiveRadiusUm: number;
      readonly alpha: number;
    };

/**
 * How the sums over radius are taken (module remarks).
 *
 * - `gaussLegendre`: R08.T5.b's composite rule in ln r over [`fromUm`, `toUm`], its panels halved
 *   `level` times.
 * - `libRadtran`: libRadtran's grid and trapezoid, `radiusUm` being its radius parameter (a
 *   log-normal's mode radius, a gamma distribution's effective radius).
 */
export type RadiusRule =
  | {
      readonly kind: "gaussLegendre";
      readonly fromUm: number;
      readonly toUm: number;
      readonly level: number;
    }
  | {
      readonly kind: "libRadtran";
      readonly radiusUm: number;
      readonly nRMax: number;
      readonly dxMax: number;
    };

/** One benchmark's spheres: what its source states, and the table written for it. */
export interface SphereCase {
  /** The file's name, and the benchmark's key in the tracer's tests. */
  readonly name: string;
  /** Where the case is defined. */
  readonly source: string;
  /** The vacuum wavelength, nm. */
  readonly wavelengthNm: number;
  /** The spheres' index relative to the medium around them, k ≥ 0 absorbing. */
  readonly index: ComplexIndex;
  readonly distribution: RadiusDistribution;
  readonly rule: RadiusRule;
  /** The table's entries, even in u. */
  readonly entries: number;
}

/** A sum over radius: each node's radius and number weight n(r) dr, up to a constant. */
export interface RadiusNodes {
  readonly radiiUm: Float64Array;
  readonly numberWeights: Float64Array;
}

/** A case's matrix and its bulk optics. */
export interface SphereCaseOptics {
  /** The matrix at the table's cosines, normalised as {@link ScatteringMatrix} is. */
  readonly matrix: ScatteringMatrix;
  /** ⟨σ_sca⟩ ÷ ⟨σ_ext⟩. */
  readonly singleScatteringAlbedo: number;
  /** The mean cosine of scattering, ⟨σ_sca g⟩ ÷ ⟨σ_sca⟩. */
  readonly asymmetry: number;
  /** The number of radii summed. */
  readonly nodes: number;
}

/** The case format's `tabulated` phase at one wavelength (`case.rs`). */
export interface TabulatedPhaseJson {
  readonly kind: "tabulated";
  readonly u: readonly number[];
  readonly a1: readonly (readonly number[])[];
  readonly asymmetry: readonly number[];
  readonly matrix: {
    readonly a2: readonly (readonly number[])[];
    readonly a3: readonly (readonly number[])[];
    readonly a4: readonly (readonly number[])[];
    readonly b1: readonly (readonly number[])[];
    readonly b2: readonly (readonly number[])[];
  };
}

/** The ln r of n(r) r, the number weight per unit ln r, up to a constant. */
function logNumberPerLnRadius(distribution: RadiusDistribution, radiusUm: number): number {
  let logNumber: number;
  switch (distribution.kind) {
    case "logNormal": {
      const z = Math.log(radiusUm / distribution.modeRadiusUm) / distribution.lnSigma;
      logNumber = -0.5 * z * z;
      break;
    }
    case "gamma": {
      const { alpha, effectiveRadiusUm } = distribution;
      logNumber = (alpha + 1) * Math.log(radiusUm) - ((alpha + 3) * radiusUm) / effectiveRadiusUm;
      break;
    }
  }
  return logNumber;
}

const PANEL_RULE = gaussLegendre(SIZE_PANEL_ORDER);

/** The composite Gauss–Legendre rule's nodes (module remarks). */
function gaussLegendreNodes(
  distribution: RadiusDistribution,
  rule: Extract<RadiusRule, { kind: "gaussLegendre" }>,
  wavenumberPerUm: number,
): RadiusNodes {
  const dx = SIZE_PANEL_SIZE_PARAMETER / 2 ** rule.level;
  const dy = SIZE_PANEL_LN_RADIUS / 2 ** rule.level;
  const high = Math.log(rule.toUm);
  const ends = [Math.log(rule.fromUm)];
  for (let y = Math.log(rule.fromUm); y < high;) {
    const x = wavenumberPerUm * Math.exp(y);
    y = Math.min(high, y + Math.min(dy, Math.log1p(dx / x)));
    ends.push(y);
  }
  const count = (ends.length - 1) * SIZE_PANEL_ORDER;
  const radiiUm = new Float64Array(count);
  const logWeights = new Float64Array(count);
  let peak = -Infinity;
  for (let p = 0; p + 1 < ends.length; p += 1) {
    const from = ends[p] ?? Number.NaN;
    const half = 0.5 * ((ends[p + 1] ?? Number.NaN) - from);
    for (let i = 0; i < SIZE_PANEL_ORDER; i += 1) {
      const node = p * SIZE_PANEL_ORDER + i;
      const radius = Math.exp(from + half * (1 + (PANEL_RULE.x[i] ?? Number.NaN)));
      radiiUm[node] = radius;
      const logWeight =
        Math.log(half * (PANEL_RULE.w[i] ?? Number.NaN)) +
        logNumberPerLnRadius(distribution, radius);
      logWeights[node] = logWeight;
      peak = Math.max(peak, logWeight);
    }
  }
  return { radiiUm, numberWeights: logWeights.map((l) => Math.exp(l - peak)) };
}

/** libRadtran's grid and trapezoid in r (module remarks). */
function libRadtranNodes(
  distribution: RadiusDistribution,
  rule: Extract<RadiusRule, { kind: "libRadtran" }>,
  wavelengthUm: number,
): RadiusNodes {
  const radii: number[] = [];
  const largest = rule.nRMax * rule.radiusUm;
  for (let radius = 0.02 * rule.radiusUm; radius <= largest;) {
    radii.push(radius);
    const x = (2 * Math.PI * radius) / wavelengthUm;
    const dx = Math.max(rule.dxMax, rule.dxMax * Math.sqrt(x));
    radius += (dx * wavelengthUm) / (2 * Math.PI);
  }
  const count = radii.length;
  if (count < 2) {
    throw new RangeError(`libRadtran's grid from ${rule.radiusUm} µm has fewer than two radii`);
  }
  const logWeights = new Float64Array(count);
  let peak = -Infinity;
  for (let i = 0; i < count; i += 1) {
    const radius = radii[i] ?? Number.NaN;
    const below = radii[i - 1] ?? radius;
    const above = radii[i + 1] ?? radius;
    // n(r) dr of the trapezoid: half the span to the neighbours, and n(r) = (r n(r)) ÷ r.
    const logWeight =
      Math.log(0.5 * (above - below)) +
      logNumberPerLnRadius(distribution, radius) -
      Math.log(radius);
    logWeights[i] = logWeight;
    peak = Math.max(peak, logWeight);
  }
  return {
    radiiUm: Float64Array.from(radii),
    numberWeights: logWeights.map((l) => Math.exp(l - peak)),
  };
}

/** Whether a rule's parameter is finite and > 0. */
function positive(value: number): boolean {
  return value > 0 && Number.isFinite(value);
}

/**
 * The radii a case's sums run over, with their number weights (module remarks).
 *
 * @throws RangeError for a rule outside its range: a Gauss–Legendre interval that is not finite,
 *   positive and ascending, or a level that is not an integer ≥ 0; a libRadtran radius parameter,
 *   `n_r_max` or `dx_max` that is not finite and > 0; or a libRadtran grid of fewer than two radii.
 */
export function radiusNodes(sphereCase: SphereCase): RadiusNodes {
  const wavelengthUm = sphereCase.wavelengthNm / 1000;
  const { rule } = sphereCase;
  let nodes: RadiusNodes;
  switch (rule.kind) {
    case "gaussLegendre":
      if (!(positive(rule.fromUm) && positive(rule.toUm) && rule.fromUm < rule.toUm)) {
        throw new RangeError(`the interval ${rule.fromUm}–${rule.toUm} µm must ascend from > 0`);
      }
      if (!(Number.isInteger(rule.level) && rule.level >= 0)) {
        throw new RangeError(`level ${rule.level} must be an integer ≥ 0`);
      }
      nodes = gaussLegendreNodes(sphereCase.distribution, rule, (2 * Math.PI) / wavelengthUm);
      break;
    case "libRadtran":
      if (!(positive(rule.radiusUm) && positive(rule.nRMax) && positive(rule.dxMax))) {
        throw new RangeError(
          `libRadtran's radius ${rule.radiusUm} µm, n_r_max ${rule.nRMax} and dx_max ${rule.dxMax} must be finite and > 0`,
        );
      }
      nodes = libRadtranNodes(sphereCase.distribution, rule, wavelengthUm);
      break;
  }
  return nodes;
}

/**
 * A case's matrix at the cosines `mu`, with its single-scattering albedo and asymmetry, from the
 * client's Mie (module remarks).
 *
 * @throws RangeError as {@link mieSphere} does, for a radius outside its size parameters.
 */
export function sphereCaseOptics(sphereCase: SphereCase, mu: Float64Array): SphereCaseOptics {
  const { radiiUm, numberWeights } = radiusNodes(sphereCase);
  const wavenumberPerUm = (2 * Math.PI * 1000) / sphereCase.wavelengthNm;
  const length = mu.length;
  const sums = {
    a1: new Float64Array(length),
    a2: new Float64Array(length),
    a3: new Float64Array(length),
    a4: new Float64Array(length),
    b1: new Float64Array(length),
    b2: new Float64Array(length),
  };
  let scattering = 0;
  let extinction = 0;
  let scatteringCosine = 0;
  for (let i = 0; i < radiiUm.length; i += 1) {
    const x = wavenumberPerUm * (radiiUm[i] ?? Number.NaN);
    const result = mieSphere(x, sphereCase.index, mu);
    const sphere = sphereScatteringMatrix(result, x, mu);
    // The node's share of the scattering, n(r) dr times its cross-section ∝ x² Q_sca.
    const weight = (numberWeights[i] ?? Number.NaN) * x * x;
    const share = weight * result.qSca;
    scattering += share;
    extinction += weight * result.qExt;
    scatteringCosine += share * result.asymmetry;
    for (const element of ["a1", "a2", "a3", "a4", "b1", "b2"] as const) {
      const sum = sums[element];
      const values = sphere[element];
      for (let j = 0; j < length; j += 1) {
        sum[j] = (sum[j] ?? 0) + share * (values[j] ?? Number.NaN);
      }
    }
  }
  const scale = (values: Float64Array): Float64Array => values.map((v) => v / scattering);
  return {
    matrix: {
      mu: mu.slice(),
      // Mie is sampled at the cosines asked, so no scattering lies in an unresolved peak.
      forwardPeak: 0,
      a1: scale(sums.a1),
      a2: scale(sums.a2),
      a3: scale(sums.a3),
      a4: scale(sums.a4),
      b1: scale(sums.b1),
      b2: scale(sums.b2),
    },
    singleScatteringAlbedo: scattering / extinction,
    asymmetry: scatteringCosine / scattering,
    nodes: radiiUm.length,
  };
}

/** `entries` values of u = √(Θ ÷ π) even from exactly 0 to exactly 1. */
export function evenU(entries: number): Float64Array {
  if (!(Number.isInteger(entries) && entries >= 2)) {
    throw new RangeError(`a table has an integer number of entries ≥ 2, not ${entries}`);
  }
  return Float64Array.from({ length: entries }, (_, i) => i / (entries - 1));
}

/** The cosines cos(π u²) of a table's scattering angles. */
export function cosinesOf(u: Float64Array): Float64Array {
  return u.map((x) => Math.cos(Math.PI * x * x));
}

const INTERVAL_RULE = gaussLegendre(16);

/**
 * A table's integral over the sphere and its mean cosine, each element read linearly in u between
 * entries: the tracer's rule (`optics.rs`, `TabulatedPhase::new`), with dΩ = 4π² u sin(πu²) du
 * and a 16-point Gauss–Legendre rule on each interval.
 */
export function linearInUMoments(
  u: ArrayLike<number>,
  values: ArrayLike<number>,
): { readonly integral: number; readonly meanCosine: number } {
  let integral = 0;
  let firstMoment = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const from = u[i] ?? Number.NaN;
    const to = u[i + 1] ?? Number.NaN;
    const low = values[i] ?? Number.NaN;
    const high = values[i + 1] ?? Number.NaN;
    const half = 0.5 * (to - from);
    for (let k = 0; k < INTERVAL_RULE.x.length; k += 1) {
      const t = 0.5 * (1 + (INTERVAL_RULE.x[k] ?? Number.NaN));
      const x = from + 2 * half * t;
      const value = low + (high - low) * t;
      const measure = 4 * Math.PI * Math.PI * x * Math.sin(Math.PI * x * x);
      const w = half * (INTERVAL_RULE.w[k] ?? Number.NaN) * value * measure;
      integral += w;
      firstMoment += w * Math.cos(Math.PI * x * x);
    }
  }
  return { integral, meanCosine: firstMoment / integral };
}

/**
 * The integral over the sphere of a matrix's a₁ ÷ 4π at the entries `u`, read linearly in u: 1 for
 * a table that holds Mie's normalisation exactly, and more for a forward peak sampled at its
 * entries (R08's Risks, "Deviations in T12.c, as built").
 *
 * @throws RangeError if `matrix` is not given at `u`'s entries, or holds a forward delta
 *   ({@link ScatteringMatrix.forwardPeak}), which the case format's table cannot carry.
 */
export function tableIntegral(matrix: ScatteringMatrix, u: Float64Array): number {
  if (matrix.mu.length !== u.length) {
    throw new RangeError(
      `a matrix of ${matrix.mu.length} cosines is not a table of ${u.length} entries`,
    );
  }
  if (matrix.forwardPeak !== 0) {
    throw new RangeError(
      `a matrix with a forward delta of ${matrix.forwardPeak} has no table in the case format`,
    );
  }
  return linearInUMoments(
    u,
    matrix.a1.map((v) => v / (4 * Math.PI)),
  ).integral;
}

/**
 * A table entry as written: an element of a matrix normalised as {@link ScatteringMatrix} is, in
 * sr⁻¹ and divided by the table's {@link tableIntegral}, to {@link SIGNIFICANT_DIGITS} digits.
 */
export function tableValue(element: number, integral: number): number {
  return Number((element / (4 * Math.PI * integral)).toPrecision(SIGNIFICANT_DIGITS));
}

/**
 * The case format's `tabulated` phase of `matrix` at the entries `u` (its cosines cos(π u²)), with
 * `asymmetry` the source's mean cosine (module remarks).
 *
 * @throws RangeError if `matrix` is not given at `u`'s entries.
 */
export function tabulatedPhase(
  matrix: ScatteringMatrix,
  u: Float64Array,
  asymmetry: number,
): TabulatedPhaseJson {
  const integral = tableIntegral(matrix, u);
  const row = (values: Float64Array): number[][] => [
    Array.from(values, (v) => tableValue(v, integral)),
  ];
  return {
    kind: "tabulated",
    u: Array.from(u),
    a1: row(matrix.a1),
    asymmetry: [asymmetry],
    matrix: {
      a2: row(matrix.a2),
      a3: row(matrix.a3),
      a4: row(matrix.a4),
      b1: row(matrix.b1),
      b2: row(matrix.b2),
    },
  };
}

/** A committed file of the writer: one case's definition, bulk optics and table. */
export interface SphereMatrixFile {
  readonly description: string;
  readonly name: string;
  readonly source: string;
  readonly wavelengthNm: number;
  readonly index: ComplexIndex;
  readonly distribution: RadiusDistribution;
  readonly rule: RadiusRule;
  readonly nodes: number;
  readonly singleScatteringAlbedo: number;
  readonly asymmetry: number;
  /** The table's {@link tableIntegral} before it was divided out. */
  readonly linearInUIntegral: number;
  readonly phase: TabulatedPhaseJson;
}

/** The text that heads every file, saying what wrote it. */
export const SPHERE_MATRIX_DESCRIPTION =
  "Written by apps/hyperion/src/renderer/src/test/mieMatrices.test.ts (plan R08, R08.T12.e) from the client's Mie; read by crates/hyperion-fit/src/atmosphere/benchmarks.rs. Do not edit by hand.";

/** A case's file, as the writer computes it. */
export function sphereMatrixFile(sphereCase: SphereCase): SphereMatrixFile {
  const u = evenU(sphereCase.entries);
  const optics = sphereCaseOptics(sphereCase, cosinesOf(u));
  return {
    description: SPHERE_MATRIX_DESCRIPTION,
    name: sphereCase.name,
    source: sphereCase.source,
    wavelengthNm: sphereCase.wavelengthNm,
    index: sphereCase.index,
    distribution: sphereCase.distribution,
    rule: sphereCase.rule,
    nodes: optics.nodes,
    singleScatteringAlbedo: optics.singleScatteringAlbedo,
    asymmetry: optics.asymmetry,
    linearInUIntegral: tableIntegral(optics.matrix, u),
    phase: tabulatedPhase(optics.matrix, u, optics.asymmetry),
  };
}

/**
 * The entries a quick check recomputes: every `stride`-th from the first, and the last.
 *
 * @remarks
 * Mie's amplitudes at one cosine do not depend on the others asked for, and the sums over size
 * run in the same order whatever the cosines, so these entries come out exactly as the whole
 * table's do.
 */
export function sampledEntries(entries: number, stride: number): number[] {
  const chosen: number[] = [];
  for (let i = 0; i < entries; i += stride) {
    chosen.push(i);
  }
  if (chosen.at(-1) !== entries - 1) {
    chosen.push(entries - 1);
  }
  return chosen;
}

/** Kokhanovsky et al. 2010's statement of their aerosol and cloud (§3, eq. (10)). */
const KOKHANOVSKY_2010 =
  "A. A. Kokhanovsky et al., J. Quant. Spectrosc. Radiat. Transfer 111 (2010) 1931-1946, section 3 and eq. (10)";

/** IPRT Phase A's statement of its spheres, and the optical-property files that record the run. */
const IPRT_PHASE_A =
  "C. Emde et al., J. Quant. Spectrosc. Radiat. Transfer 164 (2015) 8-36 (arXiv:1901.01813), section 3.2; libRadtran Mie tool parameters from IPRT's";

/** The benchmarks' spheres, in the order the tracer's tests read them. */
export const SPHERE_CASES: readonly SphereCase[] = [
  {
    name: "kokhanovsky-aerosol",
    source: KOKHANOVSKY_2010,
    wavelengthNm: 412,
    index: { n: 1.385, k: 0 },
    distribution: { kind: "logNormal", modeRadiusUm: 0.3, lnSigma: 0.92 },
    // 60,080 nodes; a₁ moves by at most 6.6 × 10⁻⁴ of itself between levels 5 and 6, and g by
    // 4 × 10⁻⁶ (0.792762 here, their 0.79275).
    rule: { kind: "gaussLegendre", fromUm: 0.005, toUm: 30, level: 5 },
    entries: 1025,
  },
  {
    name: "kokhanovsky-cloud",
    source: KOKHANOVSKY_2010,
    wavelengthNm: 412,
    index: { n: 1.339, k: 0 },
    distribution: { kind: "logNormal", modeRadiusUm: 5, lnSigma: 0.4 },
    // 49,184 nodes; a₁ moved by up to 9 × 10⁻³ of itself near 150°–176° between levels 2 and 3, and
    // g by 5 × 10⁻⁵ (0.861060 here, their 0.86114).
    rule: { kind: "gaussLegendre", fromUm: 0.005, toUm: 100, level: 3 },
    entries: 1025,
  },
  {
    name: "iprt-a3",
    source: `${IPRT_PHASE_A} waso.mie.cdf`,
    // 350 nm, as Emde et al. §3.2.3 and both of IPRT's files give it; the wiki's A3 page says 500.
    wavelengthNm: 350,
    index: { n: 1.422, k: 2.649e-3 },
    // σ_g 2.24, Emde et al.'s: `waso.mie.cdf` records 2.0, but only 2.24 reproduces its matrix.
    // Cut at 5 r_mod as IPRT's run was, g 0.550 against 0.703 to 10 r_mod: this is IPRT's
    // benchmark aerosol, not a physical model of OPAC's water-soluble aerosol.
    distribution: { kind: "logNormal", modeRadiusUm: 0.0262, lnSigma: Math.log(2.24) },
    rule: { kind: "libRadtran", radiusUm: 0.0262, nRMax: 5, dxMax: 0.03 },
    entries: 513,
  },
  {
    name: "iprt-a5",
    source: `${IPRT_PHASE_A} watercloud.mie.cdf`,
    wavelengthNm: 800,
    index: { n: 1.3258698, k: 1.250319e-7 },
    // Emde et al.'s "width of 0.1" is v_eff = 1 ÷ (α + 3); `watercloud.mie.cdf` records α 7.
    distribution: { kind: "gamma", effectiveRadiusUm: 10, alpha: 7 },
    rule: { kind: "libRadtran", radiusUm: 10, nRMax: 5, dxMax: 0.003 },
    entries: 1025,
  },
];

/**
 * The forward lobe's higher orders (plan R08, R08.T7; `science-r08-mmf.md` §3): near each source,
 * light scattered more than once within a term's forward lobe stays near the source's direction,
 * which the first-order term and Hillaire's isotropic multiple scattering both miss. At a 5° sun on
 * Mars those orders were estimated at 47–81% of the radiance 3–12° from the Sun; in the twin's Mars
 * sky they are 36–66% (`forwardLobe.test.ts`).
 *
 * @remarks
 * **The model** (the small-angle approximation: W. H. Wells, "Theory of small angle scattering",
 * AGARD Lecture Series 61, 1973, a reference from memory; E. P. Zege, A. P. Ivanov and I. L. Katsev,
 * _Image Transfer Through a Scattering Medium_, Springer, 1991, doi:10.1007/978-3-642-75286-5;
 * neither re-read here: the identity below is derived, and the tests hold it to a direct numerical
 * convolution). Along a
 * slant scattering depth x = ωτ_s of a term whose forward part (to {@link FORWARD_LOBE_RAD}) is p,
 * the orders n ≥ 2 add F e^(−τ_s) Σₙ xⁿ ÷ n! p^{*n}(θ), p^{*n} the n-fold self-convolution of p on
 * the plane of small angles. A Hankel transform, p̂(k) = 2π ∫ p(θ) J₀(kθ) θ dθ, turns the series into
 * e^(x p̂) − 1 − x p̂.
 *
 * **How the kernels carry it: a per-sample effective phase** (this task's choice). Light reaching a
 * sample from a source has crossed the lobe's scattering depth x between the source and it, so it
 * arrives as the direct beam e^(−τ) and its own forward orders about it, the forward-diffuse light
 * e^(−τ) D, D = Σ_{n≥1} xⁿ ÷ n! p^{*n}, whose total is e^(−τ) (e^(x P) − 1), P = p̂(0) the forward
 * part's share. Scattered once more at the sample, by the whole phase p_w (the whole table, on the
 * same plane), its phase is p plus E(x, θ) = D * p_w, whose transform is
 * p̂_w (e^(x p̂) − 1). Within the lobe that is the forward part's own series: integrated along a view
 * path through a homogeneous slab, ∫₀ˣ E dx′ is Σ_{n≥2} xⁿ ÷ n! p^{*n} but for the last
 * scattering's wide part. Beyond the lobe's spread E tends to (e^(x P) − 1) p(θ): the
 * forward-diffuse light scattered out of the lobe as the direct beam is, which is the beam that
 * δ-M's scaling leaves (Wiscombe 1977, J. Atmos. Sci. 34, 1408), e^(−τ + x P), scattered by p. So
 * the table is tabulated at every entry ({@link FORWARD_ORDERS_ENTRIES}). Each term with a
 * tabulated phase carries a table of Ẽ = E e^(−x P)
 * by (x, u = √(θ ÷ π)) per channel, which the kernels
 * read per sample at the lobes' x, taken as the stored optical depth to the top along the source
 * times the lobe terms' share of the sample's extinction (exact for well-mixed terms; a layered
 * one is an approximation, recorded), and multiply by e^(x P) and the source's transmittance. With
 * several lobe terms each takes their sum's x, which is exact for terms of one shape (two halves of
 * a term are the term) and an approximation for mixed shapes, recorded. A term whose forward part
 * is no lobe ({@link FORWARD_LOBE_MIN_SHARE}) takes none.
 *
 * **Not counted twice.** Hillaire's multiple-scattering table is built with each such term's
 * in-lobe scattering truncated, as the thick bakes' delta-M does: its scattering and extinction
 * less P σ_s, and the source's light at each sample raised by e^(x P), the forward-diffuse light that
 * the truncation leaves in the beam (`multiScattering.wgsl`, `tablesCpu.ts`).
 *
 * The forward part is the table's resolved lobe; a phase table's forward delta is direct light and
 * is out of the kernels' medium already (`tables.ts`'s `withoutForwardDelta`).
 */

import { gaussLegendre } from "../lighting/quadrature";
import type { Rgb } from "../photometry/toneCurve";
import { FORWARD_PEAK_GRID_ENTRIES, FORWARD_PEAK_U } from "./forwardPeak";
import type { PhaseTable } from "./medium";

/** The forward lobe's reach: 30°, `science-r08-mmf.md` §3's forward part. */
export const FORWARD_LOBE_RAD = Math.PI / 6;

/**
 * The least forward share, in the brightest channel, at which a tabulated phase's forward part is a
 * lobe whose orders are drawn: 0.2, three times an isotropic phase's 0.067 within 30°. A Rayleigh
 * shape holds 0.094 there and is spread by Hillaire's isotropic term as it is; the Mars mode holds
 * over 0.3. Below it a term takes no orders and no truncation (R08.T7's choice, recorded).
 */
export const FORWARD_LOBE_MIN_SHARE = 0.2;

/** Whether a lobe is drawn: its brightest channel's share at least {@link FORWARD_LOBE_MIN_SHARE}. */
export function lobeIsDrawn(lobe: ForwardLobe): boolean {
  return Math.max(...lobe.share) >= FORWARD_LOBE_MIN_SHARE;
}

/**
 * The phase-table entries the forward part takes, 0 to 104: u_k = k ÷ 255, θ_k = π u_k², the
 * last at 29.94°, under {@link FORWARD_LOBE_RAD}; beyond it the part is 0.
 */
export const FORWARD_LOBE_ENTRIES = 105;

/**
 * The phase-table entries the effective phase is tabulated at: all 256, to 180°. Its forward-diffuse
 * light scattered once more by the whole phase reaches every angle; within the lobe it is the
 * forward part's orders, and far from it (1 − e^(−x P)) p(θ) smoothed over the diffuse light's
 * spread. Tabulated to 59.8° alone, the switch to the unsmoothed form there stepped by 8% at a slant
 * depth of 4.5 on the Mars mode, so the table takes every entry.
 */
export const FORWARD_ORDERS_ENTRIES = 256;

/**
 * The phase-table entries of the last scattering's phase p_w on the plane: all 256, the whole
 * phase. The plane's geometry stands in for the sphere's far from the source, where the
 * forward-diffuse light it is convolved with lies within a few tens of degrees of the source.
 */
export const FORWARD_WIDE_ENTRIES = 256;

/** The lobe table's nodes in x: x_j = X_MAX (j ÷ 47)², dense where x is small. */
export const FORWARD_LOBE_X_NODES = 48;

/** The lobe table's largest x, a scattering depth; past it the table holds its last node. */
export const FORWARD_LOBE_X_MAX = 64;

/** The x of node j of {@link FORWARD_LOBE_X_NODES}. */
export function forwardLobeX(j: number): number {
  return FORWARD_LOBE_X_MAX * (j / (FORWARD_LOBE_X_NODES - 1)) ** 2;
}

/**
 * J₀(x), the Bessel function of the first kind of order 0: Press et al., _Numerical Recipes in
 * C_, 2nd ed. (1992), §6.5, `bessj0`, rational approximations below 8 and Hankel's asymptotic form
 * above, after Hart et al., _Computer Approximations_ (1968); about 10⁻⁸ absolute. A test holds it
 * to the integral (1 ÷ π) ∫₀^π cos(x sin t) dt.
 */
export function besselJ0(x: number): number {
  const ax = Math.abs(x);
  if (ax < 8) {
    const y = x * x;
    const a =
      57_568_490_574.0 +
      y *
        (-13_362_590_354.0 +
          y * (651_619_640.7 + y * (-11_214_424.18 + y * (77_392.330_17 + y * -184.905_245_6))));
    const b =
      57_568_490_411.0 +
      y * (1_029_532_985.0 + y * (9_494_680.718 + y * (59_272.648_53 + y * (267.853_271_2 + y))));
    return a / b;
  }
  const z = 8 / ax;
  const y = z * z;
  const xx = ax - 0.785_398_164;
  const p =
    1 +
    y *
      (-0.109_862_862_7e-2 +
        y * (0.273_451_040_7e-4 + y * (-0.207_337_063_9e-5 + y * 0.209_388_721_1e-6)));
  const q =
    -0.156_249_999_5e-1 +
    y *
      (0.143_048_876_5e-3 +
        y * (-0.691_114_765_1e-5 + y * (0.762_109_516_1e-6 - y * 0.934_935_152e-7)));
  return Math.sqrt(0.636_619_772 / ax) * (Math.cos(xx) * p - z * Math.sin(xx) * q);
}

/** The Gauss–Legendre rule on each u interval of the forward part's transform (order 8). */
const LOBE_GL = gaussLegendre(8);

/**
 * The k grid of the transforms, rad⁻¹: steps of 1/8 to 64, then 1/2 to 2,048, which resolves J₀(kθ)
 * to 30° (a period of 12 rad⁻¹ there) and reaches features of the lobe to about 0.03°, past a
 * resolved lobe's narrowest (the delta inside 0.044° is out of the kernels' medium).
 */
const LOBE_K: Float64Array = (() => {
  const ks: number[] = [];
  for (let k = 0; k < 64; k += 0.125) {
    ks.push(k);
  }
  for (let k = 64; k <= 2_048; k += 0.5) {
    ks.push(k);
  }
  return Float64Array.from(ks);
})();

/** The trapezoid weights of {@link LOBE_K} for ∫ f(k) k dk, the transform's measure. */
const LOBE_K_WEIGHT: Float64Array = LOBE_K.map((k, i) => {
  const before = i > 0 ? k - (LOBE_K[i - 1] ?? 0) : 0;
  const after = i + 1 < LOBE_K.length ? (LOBE_K[i + 1] ?? 0) - k : 0;
  return 0.5 * (before + after) * k;
});

/** Three channels' transforms at each of {@link LOBE_K}. */
type Transforms = readonly [Float64Array, Float64Array, Float64Array];

/**
 * A phase's three channels on the plane as p(θ), linear in u between the table's entries, and
 * their transforms p̂ at each of {@link LOBE_K}, 2π ∫ p(θ) J₀(kθ) θ dθ = 4π³ ∫ p(u) J₀(kπu²) u³ du,
 * by Gauss–Legendre on each interval: `forward` of entries 0 to {@link FORWARD_LOBE_ENTRIES} − 1
 * (the forward part, 0 beyond), and `whole` of entries 0 to `entries` − 1. Each J₀ is taken once
 * for every channel and both.
 */
function lobeTransforms(
  u: Float64Array,
  values: readonly [Float64Array, Float64Array, Float64Array],
  entries: number,
): { readonly forward: Transforms; readonly whole: Transforms } {
  const n = LOBE_K.length;
  const forward: Transforms = [new Float64Array(n), new Float64Array(n), new Float64Array(n)];
  const whole: Transforms = [new Float64Array(n), new Float64Array(n), new Float64Array(n)];
  const [r, g, b] = values;
  for (let i = 0; i + 1 < entries; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const half = 0.5 * (u1 - u0);
    const inForward = i + 1 < FORWARD_LOBE_ENTRIES;
    for (let q = 0; q < LOBE_GL.x.length; q += 1) {
      const t = 0.5 * (1 + (LOBE_GL.x[q] ?? 0));
      const uu = u0 + t * (u1 - u0);
      const base = half * (LOBE_GL.w[q] ?? 0) * uu ** 3;
      const at = (channel: Float64Array): number =>
        base * ((channel[i] ?? Number.NaN) * (1 - t) + (channel[i + 1] ?? Number.NaN) * t);
      const wr = at(r);
      const wg = at(g);
      const wb = at(b);
      const theta = Math.PI * uu * uu;
      for (let j = 0; j < n; j += 1) {
        const bessel = besselJ0((LOBE_K[j] ?? 0) * theta);
        whole[0][j] = (whole[0][j] ?? 0) + wr * bessel;
        whole[1][j] = (whole[1][j] ?? 0) + wg * bessel;
        whole[2][j] = (whole[2][j] ?? 0) + wb * bessel;
        if (inForward) {
          forward[0][j] = (forward[0][j] ?? 0) + wr * bessel;
          forward[1][j] = (forward[1][j] ?? 0) + wg * bessel;
          forward[2][j] = (forward[2][j] ?? 0) + wb * bessel;
        }
      }
    }
  }
  const factor = 4 * Math.PI ** 3;
  for (const channel of [...forward, ...whole]) {
    for (let j = 0; j < n; j += 1) {
      channel[j] = (channel[j] ?? 0) * factor;
    }
  }
  return { forward, whole };
}

/** A term's forward lobe as the kernels take it, per channel. */
export interface ForwardLobe {
  /** P = p̂(0), each channel's forward share on the plane of small angles. */
  readonly share: Rgb;
  /**
   * Ẽ(x_j, θ_k) = E e^(−x P) per channel, sr⁻¹: node j of {@link FORWARD_LOBE_X_NODES} in x, entry
   * k of the phase table's u in θ, {@link FORWARD_ORDERS_ENTRIES} of them; at [j][c][k].
   */
  readonly table: ReadonlyArray<readonly [Float64Array, Float64Array, Float64Array]>;
}

/**
 * Inverts a transform onto the table's forward entries: (1 ÷ 2π) ∫ f(k) J₀(kθ_k) k dk at each θ_k,
 * the J₀ rows given.
 */
function inverted(f: Float64Array, rows: ReadonlyArray<Float64Array>): Float64Array {
  return Float64Array.from(rows, (row) => {
    let sum = 0;
    for (let j = 0; j < f.length; j += 1) {
      sum += (f[j] ?? 0) * (row[j] ?? 0) * (LOBE_K_WEIGHT[j] ?? 0);
    }
    return sum / (2 * Math.PI);
  });
}

/** The rows of {@link besselRows} on the kernels' grid, made at the first lobe. */
let gridRows: Float64Array[] | null = null;

/**
 * J₀(k θ_k) at every k of {@link LOBE_K}, one row an entry of `u` the orders take: the kernels'
 * grid's, the one a lobe stands on (`checkOnGrid`), made once.
 */
function besselRows(u: Float64Array): Float64Array[] {
  gridRows ??= Array.from({ length: FORWARD_ORDERS_ENTRIES }, (_, k) => {
    const theta = Math.PI * (u[k] ?? Number.NaN) ** 2;
    return LOBE_K.map((kk) => besselJ0(kk * theta));
  });
  return gridRows;
}

/**
 * A tabulated phase's forward lobe ({@link ForwardLobe}): each channel's forward part's transform
 * p̂, P = p̂(0), the whole phase's p̂_w ({@link FORWARD_WIDE_ENTRIES}), and
 * Ẽ(x, θ) = (1 ÷ 2π) ∫ p̂_w (e^(x p̂) − 1) e^(−x P) J₀(kθ) k dk at each x node and entry, every
 * exponent taken as x (p̂ − P) so that nothing overflows.
 *
 * @throws RangeError for a table off the kernels' 256-entry grid, whose entries the lobe stands on.
 */
export function forwardLobeOf(table: PhaseTable): ForwardLobe {
  checkOnGrid(table);
  const rows = besselRows(table.u);
  const { forward: transforms, whole: wide } = lobeTransforms(
    table.u,
    table.values,
    FORWARD_WIDE_ENTRIES,
  );
  const share: Rgb = [transforms[0][0] ?? 0, transforms[1][0] ?? 0, transforms[2][0] ?? 0];
  const nodes = Array.from({ length: FORWARD_LOBE_X_NODES }, (_, j) => {
    const x = forwardLobeX(j);
    const [r, g, b] = transforms.map((pHat, c) => {
      const p = share[c] ?? 0;
      const last = wide[c] ?? pHat;
      const f = pHat.map(
        (v, k) => (last[k] ?? Number.NaN) * (Math.exp(x * (v - p)) - Math.exp(-x * p)),
      );
      return inverted(f, rows);
    });
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table has three channels");
    }
    return [r, g, b] as const;
  });
  return { share, table: nodes };
}

/**
 * Refuses a table off the kernels' grid, u_k = k ÷ 255 for k = 0 to 255, at whose entries the lobe's
 * readers take it (`forwardLobeAt`, `source.wgsl`'s `lobeOrdersAt`).
 */
function checkOnGrid(table: PhaseTable): void {
  const u = table.u;
  const even = u.length === FORWARD_PEAK_GRID_ENTRIES && FORWARD_PEAK_U.every((x, k) => u[k] === x);
  if (!even) {
    throw new RangeError(
      `a forward lobe stands on the kernels' ${FORWARD_PEAK_GRID_ENTRIES} even entries, not a table of ${u.length}`,
    );
  }
}

/** Each table's lobe, made once: tables are immutable values. */
const LOBES = new WeakMap<PhaseTable, ForwardLobe>();

/**
 * The latest lobes by their tables' values, for a table equal to one already made but another
 * object: a structured clone in a twin worker, or `withoutForwardDelta`'s at each `setMedium`.
 */
const LOBES_BY_VALUES = new Map<string, ForwardLobe>();

/** How many lobes {@link LOBES_BY_VALUES} keeps, the oldest dropped first. */
const LOBES_BY_VALUES_KEPT = 16;

/** A table's values as a key, every channel's entries in order. */
function valuesKey(table: PhaseTable): string {
  return table.values.map((channel) => channel.join(",")).join(";");
}

/**
 * {@link forwardLobeOf}, kept per table and by its values, since the packer, every twin and every
 * frame's medium read the same table's lobe (about half a second a table to make, at a load of 5).
 *
 * @throws RangeError as {@link forwardLobeOf}.
 */
export function forwardLobeOfTable(table: PhaseTable): ForwardLobe {
  let lobe = LOBES.get(table);
  if (lobe === undefined) {
    checkOnGrid(table);
    const key = valuesKey(table);
    lobe = LOBES_BY_VALUES.get(key);
    if (lobe === undefined) {
      lobe = forwardLobeOf(table);
      LOBES_BY_VALUES.set(key, lobe);
      const oldest = LOBES_BY_VALUES.keys().next();
      if (LOBES_BY_VALUES.size > LOBES_BY_VALUES_KEPT && oldest.done !== true) {
        LOBES_BY_VALUES.delete(oldest.value);
      }
    }
    LOBES.set(table, lobe);
  }
  return lobe;
}

/**
 * The added orders of a homogeneous slab at scattering depth x, Σ_{n≥2} xⁿ ÷ n! p^{*n}(θ), of the
 * forward part alone, at the table's {@link FORWARD_ORDERS_ENTRIES} entries, per channel, by the
 * same transform: the series R08.T7's test holds against a direct convolution, and which ∫₀ˣ E dx′
 * through the kernels' table gives within the lobe, but for the last scattering's wide part.
 *
 * @throws RangeError as {@link forwardLobeOf}.
 */
export function slabOrders(
  table: PhaseTable,
  x: number,
): readonly [Float64Array, Float64Array, Float64Array] {
  checkOnGrid(table);
  const rows = besselRows(table.u);
  const { forward } = lobeTransforms(table.u, table.values, FORWARD_LOBE_ENTRIES);
  const [r, g, b] = forward.map((pHat) => {
    return inverted(
      pHat.map((v) => Math.expm1(x * v) - x * v),
      rows,
    );
  });
  if (r === undefined || g === undefined || b === undefined) {
    throw new Error("a phase table has three channels");
  }
  return [r, g, b];
}

/**
 * Ẽ at a scattering depth x and an entry's place `entry` (fractional, in table entries), channel
 * c: bilinear in the x nodes' index and the entries, as the kernels read it; beyond the orders'
 * entries (1 − e^(−x P)) `phase`, the forward-diffuse light scattered by the phase itself.
 *
 * @param phase - The term's phase at the angle, channel c, sr⁻¹: what it takes beyond the table.
 */
export function forwardLobeAt(
  lobe: ForwardLobe,
  c: 0 | 1 | 2,
  x: number,
  entry: number,
  phase: number,
): number {
  if (!(entry < FORWARD_ORDERS_ENTRIES - 1)) {
    return -Math.expm1(-Math.max(x, 0) * lobe.share[c]) * phase;
  }
  const last = FORWARD_LOBE_X_NODES - 1;
  const at = Math.min(Math.sqrt(Math.max(x, 0) / FORWARD_LOBE_X_MAX) * last, last);
  const j = Math.min(Math.floor(at), last - 1);
  const fj = at - j;
  const k = Math.max(0, Math.floor(entry));
  const fk = entry - k;
  const read = (jj: number): number => {
    const row = lobe.table[jj]?.[c];
    const a = row?.[k] ?? 0;
    const b = row?.[k + 1] ?? 0;
    return a + (b - a) * fk;
  };
  return read(j) * (1 - fj) + read(j + 1) * fj;
}

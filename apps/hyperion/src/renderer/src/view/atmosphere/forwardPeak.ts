/**
 * A phase table's forward delta (plan R08, Design note 6): the unresolved diffraction peak of a
 * large particle, which R08.T5.c lays on a table's first {@link FORWARD_PEAK_ENTRIES} entries
 * (`aerosol.ts`'s `aerosolPhaseTable`), and which R08.T7 takes out of the medium the per-frame
 * kernels read (`tables.ts`'s `withoutForwardDelta`).
 *
 * @remarks
 * The delta is direct light (decided 2026-10-10, `decision-r08-aerosol-labels.md` item 3). Within
 * 0.044° of the source, no sky-view texel, froxel or march pixel can sample it without aliasing,
 * and a star whose disc is smaller (a Sun-like star beyond about 6 au) would leave it in the sky.
 * Taken out of the kernels' table and extinction, which is exact for a forward delta (Potter 1970,
 * J. Atmos. Sci. 27, 943), its light stays in the shared transmittance: the star's image, its
 * glare, the sunlight at each scattering point and on the ground.
 *
 * Here, and not in `aerosol.ts`, so that the render thread's bundle, which `tables.ts` is in, does
 * not take the aerosol optics' modules and tables.
 */

import { gaussLegendre } from "../lighting/quadrature";

/**
 * The table entries an unresolved forward peak is laid on: a triangle in u over the first 4
 * intervals, θ < 0.044°, inside both phase files' first angle beyond 0° (0.05° for Yang et al.,
 * 0.08° for TAMUdust2020), so it adds nothing where the files have values.
 */
export const FORWARD_PEAK_ENTRIES = 4;

/**
 * The phase tables' grid the delta is laid on: 256 entries even in u = √(θ ÷ π), `tables.ts`'s
 * `PHASE_TABLE_ENTRIES` and `sizeDistribution.ts`'s `PHASE_TABLE_SIZE` (a test holds the three
 * equal).
 */
export const FORWARD_PEAK_GRID_ENTRIES = 256;

/** The grid's u, i ÷ 255 at entry i, `sizeDistribution.ts`'s `PHASE_TABLE_U`. */
export const FORWARD_PEAK_U: Float64Array = Float64Array.from(
  { length: FORWARD_PEAK_GRID_ENTRIES },
  (_, i) => i / (FORWARD_PEAK_GRID_ENTRIES - 1),
);

/** The forward peak's shape on {@link FORWARD_PEAK_U}: 1 − k ÷ {@link FORWARD_PEAK_ENTRIES}, then 0. */
export const FORWARD_PEAK_SHAPE: Float64Array = FORWARD_PEAK_U.map((_, k) =>
  Math.max(0, 1 - k / FORWARD_PEAK_ENTRIES),
);

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
export function phaseTableMoment(
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

/** The delta's shape's integral over the sphere as the table is read. */
export const FORWARD_PEAK_INTEGRAL = phaseTableMoment(FORWARD_PEAK_U, FORWARD_PEAK_SHAPE, () => 1);

/** μ_δ, the delta's mean cosine as the table is read, about 1 − 8.3 × 10⁻⁸. */
export const FORWARD_PEAK_COSINE =
  phaseTableMoment(FORWARD_PEAK_U, FORWARD_PEAK_SHAPE, (mu) => mu) / FORWARD_PEAK_INTEGRAL;

/**
 * The delta's entry k for a share `share` of a channel's scattering, sr⁻¹: the shape there scaled
 * so that its integral as read is the share, as `aerosolPhaseTable` lays it.
 */
export function forwardPeakEntry(share: number, k: number): number {
  return (share / FORWARD_PEAK_INTEGRAL) * (FORWARD_PEAK_SHAPE[k] ?? 0);
}

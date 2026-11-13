/**
 * R09's blend of the two months nearest a time, the client's twin (plan R11, R11.T8.b; Design note
 * 19): the surface crate's `month_blend_of` (`field/months.rs`), to the bit, so that the sea's wind
 * turns with the seasons as the climate does, without the module on the main thread.
 *
 * @remarks
 * A year has 1 month or 12, the twelve equal spans of the seasonal orbit's eccentric anomaly E from
 * periapsis, whose edges are the mean anomalies Mₖ = k · 30° − e sin(k · 30°) (Kepler's equation,
 * Murray and Dermott 1999, _Solar System Dynamics_, eq. 2.52). The weight is linear in mean anomaly,
 * that is in time, between the two months' centres, each its span's midpoint, so a monthly field
 * turns without a step at a month's edge and faster near periapsis on an eccentric orbit. Every step
 * is one of the four operators or the exact remainder (`%` is C's `fmod`), on the crate's own
 * constants (π ÷ 6 and √3 ÷ 2 as `core::f64::consts::FRAC_PI_6` and the crate's literal hold them,
 * the first a bit above `Math.PI / 6`), so the twin and the module agree to the bit: the module's
 * `monthBlend` export and the crate's `months.golden` hold it so (`monthBlend.test.ts`,
 * `oceanMap.wasm.test.ts`).
 */

/** A time's place between two months: `from` with weight 1 − `weight`, `to` with `weight`. */
export interface MonthBlend {
  /** The earlier month, from 0, whose centre is at or before the time. */
  readonly from: number;
  /** The later month, from 0: the next after `from`, month 0 after month 11; 0 in a one-month year. */
  readonly to: number;
  /** The later month's weight, 0 to 1: 0 at `from`'s centre. */
  readonly weight: number;
}

/** The months of a seasonal year. */
const MONTHS = 12;

/** 2π, exact as 2 × π. */
const TAU = 2 * Math.PI;

/**
 * π ÷ 6 correctly rounded, Rust's `core::f64::consts::FRAC_PI_6` (0x3fe0c152382d7366), one unit in
 * the last place above `Math.PI / 6`.
 */
export const FRAC_PI_6 = 0.5235987755982989;

/** sin 60° = √3 ÷ 2, the crate's `HALF_ROOT_3`. */
const HALF_ROOT_3 = 0.8660254037844386;

/** sin(k · 30°) for k from 0 to 12, exact where the sine is 0, ±½ or ±1 (the crate's `EDGE_SINES`). */
const EDGE_SINES: ReadonlyArray<number> = [
  0,
  0.5,
  HALF_ROOT_3,
  1,
  HALF_ROOT_3,
  0.5,
  0,
  -0.5,
  -HALF_ROOT_3,
  -1,
  -HALF_ROOT_3,
  -0.5,
  0,
];

/**
 * The mean anomalies of the twelve months' edges, radians from periapsis, edge 12 at 2π (the
 * crate's `month_edges`); `null` unless 0 ≤ e < 1.
 */
export function monthEdges(seasonEccentricity: number): ReadonlyArray<number> | null {
  const e = seasonEccentricity;
  if (!(e >= 0 && e < 1)) {
    return null;
  }
  const edges: number[] = [];
  for (let k = 0; k < MONTHS; k += 1) {
    edges.push(k * FRAC_PI_6 - e * (EDGE_SINES[k] ?? 0));
  }
  edges.push(TAU);
  return edges;
}

/** `m` reduced to [0, 2π): its exact remainder modulo 2π, lifted by 2π if negative, +0 for −0. */
function reduce(m: number): number {
  const turn = m % TAU;
  if (turn < 0) {
    const lifted = turn + TAU;
    return lifted < TAU ? lifted : 0;
  }
  // −0 + 0 is +0.
  return turn + 0;
}

/** Months `from` and `to` blended at `at`, linearly between their centres `start` and `end`. */
function blend(from: number, to: number, start: number, end: number, at: number): MonthBlend {
  return { from, to, weight: (at - start) / (end - start) };
}

/**
 * The blend of the two months nearest the time at the seasonal orbit's mean anomaly `meanAnomalyRad`
 * (radians, any finite angle, 0 at periapsis) in a year of `months` months on an orbit of
 * eccentricity `seasonEccentricity`: the crate's `month_blend_of`, bit for bit; month 0 alone in a
 * one-month year.
 *
 * @returns The blend, or `null` for a count of months other than 1 or 12, an eccentricity outside
 *   0 ≤ e < 1, or a mean anomaly that is not finite, as the crate refuses them.
 */
export function monthBlend(
  months: number,
  seasonEccentricity: number,
  meanAnomalyRad: number,
): MonthBlend | null {
  if (!(months === 1 || months === MONTHS) || !Number.isFinite(meanAnomalyRad)) {
    return null;
  }
  const edges = monthEdges(seasonEccentricity);
  if (edges === null) {
    return null;
  }
  const m = reduce(meanAnomalyRad);
  if (months === 1) {
    return { from: 0, to: 0, weight: 0 };
  }
  const centres: number[] = [];
  for (let k = 0; k < MONTHS; k += 1) {
    centres.push(((edges[k] ?? 0) + (edges[k + 1] ?? 0)) / 2);
  }
  // Month 11's centre loses 2π exactly (Sterbenz), as the crate's.
  const lastBeforePeriapsis = (centres[MONTHS - 1] ?? 0) - TAU;
  const first = centres[0] ?? 0;
  for (let k = MONTHS - 1; k >= 0; k -= 1) {
    const centre = centres[k] ?? 0;
    if (centre <= m) {
      return k === MONTHS - 1
        ? blend(11, 0, lastBeforePeriapsis, first, m - TAU)
        : blend(k, k + 1, centre, centres[k + 1] ?? 0, m);
    }
  }
  return blend(11, 0, lastBeforePeriapsis, first, m);
}

/**
 * A perf run's warm-up verdict and its trend screen (plan R12, R12.T4.b; Design note 7).
 *
 * @remarks
 * The warm-up lasts at least {@link WARM_UP_MIN_S}, 4.3 τ of the UHD 620's RAPL PL1 window
 * (τ = 28 s), so that the PL1 average has converged to about 1.4%, and then until a sliding
 * {@link WINDOW_S} window changes from the one before by less than {@link GPU_CLOCK_TOLERANCE} in
 * median GPU frequency and {@link PACKAGE_TOLERANCE_DEGC} in package temperature, for at most
 * {@link WARM_UP_MAX_S}, after which the record says `NOT STEADY` (Kalibera and Jones, ISMM 2013,
 * doi:10.1145/2464157.2464160, judge warm-up from the sequence rather than assume a count). The
 * renderer's frame intervals are judged beside them, their median within the GPU clock's
 * tolerance, so that a machine whose clock and temperature are unread (a pod, a Mac) is still judged
 * on its frames (a lean, 2026-11-17). A reading neither window holds is not judged.
 *
 * The trend screen flags repetitions whose per-repetition mean frame times are all in one order:
 * at five, Mann–Kendall's |S| = 10, whose exact two-sided p is 2/120 = 1/60 ≈ 0.017 under
 * exchangeability (Mann, Econometrica 13, 245–259, 1945; Kendall, Rank Correlation Methods, the
 * exact null distribution of S). It is a coarse drift screen with low power, not a proof that the
 * repetitions are independent.
 */

/** The warm-up's least length, s: two minutes, 4.3 τ of PL1's 28 s window (Design note 7). */
export const WARM_UP_MIN_S = 120;

/** The warm-up's most, s: ten minutes, after which it is `NOT STEADY` (Design note 7). */
export const WARM_UP_MAX_S = 600;

/** The sliding window's length, s (Design note 7). */
export const WINDOW_S = 30;

/** The largest change in median GPU frequency between two windows that is steady, a fraction. */
export const GPU_CLOCK_TOLERANCE = 0.05;

/** The largest change in median package temperature between two windows that is steady, °C. */
export const PACKAGE_TOLERANCE_DEGC = 2;

/** The largest change in median frame interval between two windows that is steady, a fraction. */
export const INTERVAL_TOLERANCE = 0.05;

/** A value at a time, ms on the runner's clock. */
export interface Timed {
  readonly atMs: number;
  readonly value: number;
}

/** What the warm-up is judged on, each series timestamped on the runner's clock. */
export interface WarmUpSeries {
  /** The GPU's clock, MHz (R12.T2's samplers); empty where unread. */
  readonly gpuMHz: ReadonlyArray<Timed>;
  /** The package's temperature, °C; empty where unread. */
  readonly packageDegC: ReadonlyArray<Timed>;
  /** The renderer's frame intervals, ms, each at the time the runner took it. */
  readonly intervalsMs: ReadonlyArray<Timed>;
}

/** Where the warm-up stands: still warming, steady, or `NOT STEADY` at its limit. */
export type WarmUpVerdict =
  | { readonly kind: "warming"; readonly elapsedS: number }
  | { readonly kind: "steady"; readonly elapsedS: number }
  | { readonly kind: "not-steady"; readonly elapsedS: number; readonly why: string };

/** The median of `values`, or `null` for none. */
export function median(values: ReadonlyArray<number>): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = values.toSorted((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? (sorted[mid] ?? null)
    : ((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2;
}

/** The median of the values of `series` in `[fromMs, toMs)`. */
function windowMedian(series: ReadonlyArray<Timed>, fromMs: number, toMs: number): number | null {
  return median(series.filter(({ atMs }) => atMs >= fromMs && atMs < toMs).map((s) => s.value));
}

/**
 * Why the window ending at `nowMs` is not steady against the one before, or `null` where every
 * reading both windows hold is within its tolerance.
 */
export function unsteadyReason(series: WarmUpSeries, nowMs: number): string | null {
  const windowMs = WINDOW_S * 1000;
  const reasons: string[] = [];
  const relative = (
    name: string,
    values: ReadonlyArray<Timed>,
    tolerance: number,
    unit: string,
  ): void => {
    const before = windowMedian(values, nowMs - 2 * windowMs, nowMs - windowMs);
    const last = windowMedian(values, nowMs - windowMs, nowMs + 1);
    if (before !== null && last !== null && before > 0) {
      const change = Math.abs(last - before) / before;
      if (!(change < tolerance)) {
        reasons.push(
          `${name} ${before.toFixed(1)} to ${last.toFixed(1)} ${unit}, ${(change * 100).toFixed(1)}%`,
        );
      }
    }
  };
  relative("median GPU clock", series.gpuMHz, GPU_CLOCK_TOLERANCE, "MHz");
  relative("median frame interval", series.intervalsMs, INTERVAL_TOLERANCE, "ms");
  const before = windowMedian(series.packageDegC, nowMs - 2 * windowMs, nowMs - windowMs);
  const last = windowMedian(series.packageDegC, nowMs - windowMs, nowMs + 1);
  if (before !== null && last !== null && !(Math.abs(last - before) < PACKAGE_TOLERANCE_DEGC)) {
    reasons.push(`package ${before.toFixed(1)} to ${last.toFixed(1)} °C`);
  }
  return reasons.length === 0 ? null : reasons.join("; ");
}

/**
 * The warm-up's verdict at `nowMs`, begun at `startMs`: warming before {@link WARM_UP_MIN_S},
 * steady once the last window holds against the one before, `NOT STEADY` at {@link WARM_UP_MAX_S}.
 */
export function judgeWarmUp(series: WarmUpSeries, startMs: number, nowMs: number): WarmUpVerdict {
  const elapsedS = (nowMs - startMs) / 1000;
  if (elapsedS < WARM_UP_MIN_S) {
    return { kind: "warming", elapsedS };
  }
  const why = unsteadyReason(series, nowMs);
  if (why === null) {
    return { kind: "steady", elapsedS };
  }
  return elapsedS >= WARM_UP_MAX_S
    ? { kind: "not-steady", elapsedS, why }
    : { kind: "warming", elapsedS };
}

/** The trend screen's finding: the repetitions' order and Mann–Kendall's S. */
export interface TrendFlag {
  readonly direction: "rising" | "falling";
  /** Σ sign(xⱼ − xᵢ) over every pair i < j. */
  readonly s: number;
}

/** The fewest repetitions the screen reads: Design note 7's five. */
export const TREND_MIN_REPETITIONS = 5;

/** Mann–Kendall's S of `values`: Σ sign(xⱼ − xᵢ) over every pair i < j. */
export function mannKendallS(values: ReadonlyArray<number>): number {
  let s = 0;
  for (let i = 0; i < values.length; i += 1) {
    for (let j = i + 1; j < values.length; j += 1) {
      s += Math.sign((values[j] ?? 0) - (values[i] ?? 0));
    }
  }
  return s;
}

/**
 * The trend screen over the repetitions' mean frame times, ms: flagged where every pair is in one
 * order, |S| = n(n − 1) ÷ 2 (10 at five, all four successive differences of one sign); `null`
 * otherwise, or below {@link TREND_MIN_REPETITIONS}.
 */
export function trendOf(meansMs: ReadonlyArray<number>): TrendFlag | null {
  const n = meansMs.length;
  if (n < TREND_MIN_REPETITIONS) {
    return null;
  }
  const s = mannKendallS(meansMs);
  const all = (n * (n - 1)) / 2;
  return Math.abs(s) === all ? { direction: s > 0 ? "rising" : "falling", s } : null;
}

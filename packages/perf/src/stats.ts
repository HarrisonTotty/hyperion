/**
 * The arithmetic over a record's repetitions: medians, ranges, the trend screen and the throttle
 * marks (plan R12, Design note 7).
 */

/** The median of the values, the mean of the middle two for an even count; `null` for none. */
export function median(values: ReadonlyArray<number>): number | null {
  if (values.length === 0) return null;
  const sorted = values.toSorted((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const upper = sorted[mid];
  if (upper === undefined) return null;
  if (sorted.length % 2 === 1) return upper;
  const lower = sorted[mid - 1];
  return lower === undefined ? upper : (lower + upper) / 2;
}

/** A metric's least and greatest values over the repetitions. */
export interface ValueRange {
  readonly min: number;
  readonly max: number;
}

/** The range of the values; `null` for none. */
export function rangeOf(values: ReadonlyArray<number>): ValueRange | null {
  if (values.length === 0) return null;
  return { min: Math.min(...values), max: Math.max(...values) };
}

/** The values, or `null` if any is missing: a metric is compared only where every repetition has it. */
export function complete(values: ReadonlyArray<number | null>): ReadonlyArray<number> | null {
  const out: number[] = [];
  for (const value of values) {
    if (value === null) return null;
    out.push(value);
  }
  return out;
}

/**
 * The repetitions the trend screen needs: at five, a monotone sequence is Mann–Kendall's |S| = 10,
 * p = 2/120 ≈ 0.017 two-sided under exchangeability (Mann 1945; Kendall, _Rank Correlation
 * Methods_), the smallest p the exact test attains there; fewer cannot reach it.
 */
export const TREND_MIN_REPETITIONS = 5;

/**
 * Design note 7's trend screen: every successive difference of the per-repetition means of one
 * sign, none zero, over at least {@link TREND_MIN_REPETITIONS} repetitions. A coarse drift screen
 * of low power, not a proof of independence.
 */
export function monotoneTrend(values: ReadonlyArray<number>): boolean {
  if (values.length < TREND_MIN_REPETITIONS) return false;
  let sign = 0;
  for (let i = 1; i < values.length; i += 1) {
    const current = values[i];
    const previous = values[i - 1];
    if (current === undefined || previous === undefined) return false;
    const step = Math.sign(current - previous);
    if (step === 0 || (sign !== 0 && step !== sign)) return false;
    sign = step;
  }
  return true;
}

/**
 * How far a repetition's throttle fraction may sit from the scene's median before it is marked, as
 * a fraction: 20 percentage points (Design note 7; provisional until R12.T8.d).
 */
export const THROTTLE_MARK_MARGIN = 0.2;

/**
 * The 1-based indices of the repetitions whose throttle fraction differs from the median of all of
 * them by more than {@link THROTTLE_MARK_MARGIN}; a repetition without one is not marked.
 */
export function throttleMarks(fractions: ReadonlyArray<number | null>): ReadonlyArray<number> {
  const known = fractions.filter((f): f is number => f !== null);
  const middle = median(known);
  if (middle === null) return [];
  const marked: number[] = [];
  fractions.forEach((f, i) => {
    if (f !== null && Math.abs(f - middle) > THROTTLE_MARK_MARGIN) marked.push(i + 1);
  });
  return marked;
}

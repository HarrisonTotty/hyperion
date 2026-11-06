/**
 * The display's vsync period as a loop's animation frames measure it, and the rule by which a view
 * at its design rate draws on a whole number of vsyncs (plan R07, T19; R05 Design note 21; the
 * owner's frame rate, research-frame-pacing-120hz §3).
 *
 * @remarks
 * The design's rates are 60 and 30 Hz on any display. Counting animation frames gives them only
 * at 60 Hz: on a 120 Hz panel a 60 Hz view drew at 120 and a 30 Hz one at 60. By time, a view at
 * `rateHz` draws on every k-th vsync of the measured period P, k the most whole vsyncs whose rate
 * stays within 5% of `rateHz` or above it ({@link vsyncsPerDraw}): 1 and 2 at 60 or 59.94 Hz,
 * today's frames; 2 and 4 at 120 Hz; 2 and 5 at 144 Hz (72 and 28.8 Hz). It draws in the first
 * callback once (k − ½)·P has passed since its last draw ({@link drawsNow}): every k-th callback on
 * time, and the next callback after a late frame, which counting frames held back a vsync more.
 *
 * A view asked only in the draws of another, every m vsyncs (the instruments in the primary's
 * frames), draws on a whole number of those: k a multiple of m by the same 5% rule, and once
 * (k − m/2)·P has passed. Its own k alone would fall between them: 5 at 144 Hz under a primary's 2,
 * which it could only meet at 6 (24 Hz); on the primary's grid it is 4 (36 Hz).
 *
 * Pure and import-free, so that the main process's results read the same rule as the renderer's
 * loop (`tsconfig.node.json` takes this file, as it does `pinnedMath.ts`).
 */

/** The animation-frame intervals the period is measured over: the last 120, 2 s at 60 Hz. */
export const VSYNC_WINDOW_FRAMES = 120;

/** The intervals seen before the measured period is used; until then {@link DEFAULT_VSYNC_MS}. */
export const VSYNC_SEEN_FRAMES = 30;

/** The period taken until {@link VSYNC_SEEN_FRAMES} intervals are seen, ms: a 60 Hz display's. */
export const DEFAULT_VSYNC_MS = 1000 / 60;

/**
 * The share of the window's intervals at or below the period taken: its 10th percentile, so that
 * callbacks that skipped a vsync (a GPU-bound stretch) leave it at the panel's period while a
 * tenth of them still come on time.
 */
export const VSYNC_PERCENTILE = 0.1;

/**
 * How far above its design rate a view may draw to keep a whole number of vsyncs: 5%, so that a
 * 59.94 Hz display's period still gives a 60 Hz view every vsync (1.05 × 16.67 / 16.68 ≥ 1), and a
 * 144 Hz one gives it every second (72 Hz), never every third (48 Hz).
 */
export const RATE_MARGIN = 1.05;

/**
 * The vsync period `intervalsMs` measure, ms: the {@link VSYNC_PERCENTILE} (nearest rank) of the
 * last {@link VSYNC_WINDOW_FRAMES} positive ones, or `null` with fewer than
 * {@link VSYNC_SEEN_FRAMES} of them.
 *
 * @param intervalsMs - Intervals between consecutive animation frames, oldest first; a zero or a
 *   non-finite one (a first frame's, a recorder's gap) is left out.
 */
export function vsyncPeriodOf(intervalsMs: ReadonlyArray<number>): number | null {
  const seen = intervalsMs
    .slice(-VSYNC_WINDOW_FRAMES)
    .filter((ms) => Number.isFinite(ms) && ms > 0);
  if (seen.length < VSYNC_SEEN_FRAMES) {
    return null;
  }
  const sorted = seen.toSorted((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(VSYNC_PERCENTILE * sorted.length) - 1)] ?? null;
}

/**
 * The vsync period a loop's animation frames measure (research-frame-pacing-120hz §3): the
 * {@link VSYNC_PERCENTILE} of the last {@link VSYNC_WINDOW_FRAMES} intervals between the callbacks
 * it is told of, and {@link DEFAULT_VSYNC_MS} until {@link VSYNC_SEEN_FRAMES} have been seen.
 *
 * @remarks
 * One a loop, told of every callback whether or not a view draws in it. An overestimate, while
 * every frame is GPU-bound, only draws views in more callbacks, and corrects within about a dozen
 * light ones.
 */
export class VsyncClock {
  readonly #intervalsMs: number[] = [];
  #lastMs: number | null = null;
  #periodMs = DEFAULT_VSYNC_MS;

  /** Notes an animation frame's callback at its timestamp, ms. */
  note(nowMs: number): void {
    const last = this.#lastMs;
    this.#lastMs = nowMs;
    if (last === null || !(nowMs > last)) {
      return;
    }
    this.#intervalsMs.push(nowMs - last);
    if (this.#intervalsMs.length > VSYNC_WINDOW_FRAMES) {
      this.#intervalsMs.shift();
    }
    this.#periodMs = vsyncPeriodOf(this.#intervalsMs) ?? DEFAULT_VSYNC_MS;
  }

  /** The measured vsync period, ms. */
  get periodMs(): number {
    return this.#periodMs;
  }
}

/**
 * The vsyncs between a view's draws at `rateHz` on a display of vsync period `periodMs`:
 * k = m × max(1, ⌊{@link RATE_MARGIN} × (1000 ÷ rateHz) ÷ (m × periodMs)⌋), m `withinVsyncs`.
 *
 * @param withinVsyncs - The vsyncs between the callbacks it is asked in: 1, every callback, by
 *   default; another view's own k where it is asked only in that view's draws.
 */
export function vsyncsPerDraw(rateHz: number, periodMs: number, withinVsyncs = 1): number {
  const m = Math.max(1, withinVsyncs);
  return m * Math.max(1, Math.floor((RATE_MARGIN * (1000 / rateHz)) / (m * periodMs)));
}

/**
 * Whether a view at `rateHz` draws in this callback: once (k − m/2)·P has passed since its last
 * draw, k {@link vsyncsPerDraw}, P `periodMs` and m `withinVsyncs`, so (k − ½)·P for a view asked
 * in every callback.
 *
 * @param sinceLastDrawMs - The time since the view last drew, ms; `Infinity` before its first draw.
 * @param withinVsyncs - As {@link vsyncsPerDraw}'s.
 */
export function drawsNow(
  sinceLastDrawMs: number,
  rateHz: number,
  periodMs: number,
  withinVsyncs = 1,
): boolean {
  const m = Math.max(1, withinVsyncs);
  return sinceLastDrawMs >= (vsyncsPerDraw(rateHz, periodMs, m) - m / 2) * periodMs;
}

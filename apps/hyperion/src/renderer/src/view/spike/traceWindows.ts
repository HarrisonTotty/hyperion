/**
 * Where the descent spike's trace is stopped and started again (plan R05, T14.e;
 * decision-r05-trace-windows.md): the boundaries of its windows, a pure function of the profile's
 * segments.
 *
 * @remarks
 * Chromium's tracing service crashed reading out a whole descent's trace at its stop, so the trace
 * is taken in windows of script time, each written to its own file. A window holds at most
 * {@link TRACE_WINDOW_MAX_S}, apart from the one that holds the busy stretch
 * ({@link TRACE_BUSY_SEGMENTS}) unbroken, which holds at most {@link TRACE_BUSY_WINDOW_MAX_S}. No
 * boundary lies within {@link TRACE_CHANGE_CLEARANCE_S} of a segment change, or inside the busy
 * stretch widened by that clearance, so that the stall at a boundary falls where little streams.
 * Boundaries are placed greedily from the script's start, each as late as allowed. For today's
 * profile they fall at 120, 240, 360, 480, 600, 720, 840, 950 and 1,190 s.
 */

import type { SpikeSegmentSpan } from "../../../../preload/api";

/** The longest window of script time, s, outside the busy stretch. */
export const TRACE_WINDOW_MAX_S = 120;

/** The longest window, s, for the one that holds the busy stretch with its clearances. */
export const TRACE_BUSY_WINDOW_MAX_S = 240;

/** How far every boundary stays from a segment change, s. */
export const TRACE_CHANGE_CLEARANCE_S = 10;

/**
 * How long after a window's start resolves its boundary's frames are still left out of every
 * per-frame figure, s: the start's own pause.
 *
 * @remarks
 * The report carries it (`traceGuardS`), and the main process's merge applies it, so this is its one
 * source. Provisional until T14.f's hidden runs confirm it.
 */
export const TRACE_BOUNDARY_GUARD_S = 1;

/**
 * The segments of the busy stretch, which one window holds unbroken: the approach and flare, the
 * low fast pass, the slowdown and the vertical descent, where streaming peaks (Design note 21: a
 * failure near the ground must not be averaged away).
 */
export const TRACE_BUSY_SEGMENTS: ReadonlyArray<string> = [
  "approach and flare",
  "low fast pass",
  "slowdown",
  "vertical descent",
];

/** A stretch of script time, s. */
interface Stretch {
  readonly fromS: number;
  readonly toS: number;
}

/** The busy segments' stretch, from the first's start to the last's end, or `null` for none. */
function busyStretchOf(spans: ReadonlyArray<SpikeSegmentSpan>): Stretch | null {
  const busy = spans.filter(({ name }) => TRACE_BUSY_SEGMENTS.includes(name));
  if (busy.length === 0) {
    return null;
  }
  return {
    fromS: Math.min(...busy.map(({ startS }) => startS)),
    toS: Math.max(...busy.map(({ endS }) => endS)),
  };
}

/**
 * The script times at which the trace is stopped and started again, ascending, s.
 *
 * @param segmentSpans - The descent's segments in order (`DescentProfile.segmentSpans()`); the
 * script runs from 0 to the last one's end.
 * @returns One fewer than the windows: none for a script that fits in one window.
 * @throws Error if the busy stretch with its clearances is longer than
 * {@link TRACE_BUSY_WINDOW_MAX_S}, or if segment changes leave no allowed boundary within a
 * window's reach.
 */
export function traceBoundaries(segmentSpans: ReadonlyArray<SpikeSegmentSpan>): number[] {
  const endS = Math.max(0, ...segmentSpans.map((span) => span.endS));
  const changesS = segmentSpans.slice(1).map(({ startS }) => startS);
  const busy = busyStretchOf(segmentSpans);
  const clearance = TRACE_CHANGE_CLEARANCE_S;
  const widened: Stretch | null =
    busy === null ? null : { fromS: busy.fromS - clearance, toS: busy.toS + clearance };
  if (widened !== null && widened.toS - widened.fromS > TRACE_BUSY_WINDOW_MAX_S) {
    throw new Error(
      `the busy stretch with its clearances is ${widened.toS - widened.fromS} s, more than the ${TRACE_BUSY_WINDOW_MAX_S} s one window may hold`,
    );
  }
  const allowed = (t: number): boolean =>
    changesS.every((c) => Math.abs(t - c) >= clearance) &&
    (widened === null || t <= widened.fromS || t >= widened.toS);
  // Each forbidden stretch is open, so the latest allowed time at or before a limit is the limit
  // itself or the lower edge of a forbidden stretch.
  const edgesS = [
    ...changesS.map((c) => c - clearance),
    ...(widened === null ? [] : [widened.fromS]),
  ];
  const latestAllowed = (afterS: number, limitS: number, atLeastS: number): number | null => {
    const candidates = [limitS, ...edgesS].filter(
      (t) => t > afterS && t <= limitS && t >= atLeastS && allowed(t),
    );
    return candidates.length === 0 ? null : Math.max(...candidates);
  };
  const boundaries: number[] = [];
  let fromS = 0;
  for (;;) {
    // The last window may be the busy one, holding the busy segments to the script's end.
    const lastHoldsBusy = busy !== null && fromS <= busy.fromS && endS > busy.fromS;
    if (endS - fromS <= (lastHoldsBusy ? TRACE_BUSY_WINDOW_MAX_S : TRACE_WINDOW_MAX_S)) {
      return boundaries;
    }
    // A window that cannot end within the usual length holds the busy stretch whole.
    const next =
      latestAllowed(fromS, fromS + TRACE_WINDOW_MAX_S, fromS) ??
      (busy !== null && fromS <= busy.fromS
        ? latestAllowed(fromS, fromS + TRACE_BUSY_WINDOW_MAX_S, busy.toS)
        : null);
    if (next === null) {
      throw new Error(`no trace boundary can be placed after ${fromS} s`);
    }
    boundaries.push(next);
    fromS = next;
  }
}

/**
 * Where the descent spike's trace is stopped and started again (plan R05, T14.e and T14.g;
 * decision-r05-trace-windows.md and decision-r05-trace-windows-2.md): the boundaries of its
 * windows, a pure function of the profile's segments.
 *
 * @remarks
 * Chromium's tracing service crashed reading out a whole descent's trace at its stop, so the trace
 * is taken in windows of script time, each written to its own file. A window holds at most
 * {@link TRACE_WINDOW_MAX_S}, apart from the last, which holds the busy stretch
 * ({@link TRACE_BUSY_SEGMENTS}) unbroken from its start widened by the clearance to the script's
 * end, at most {@link TRACE_BUSY_WINDOW_MAX_S}: a stop's gap grows with the bytes since the
 * previous one, so no boundary follows the busiest stretch, and the last stop, after the script,
 * leaves nothing out. No boundary lies within {@link TRACE_CHANGE_CLEARANCE_S} of a segment
 * change, so that the stall at a boundary falls where little streams. Boundaries are placed
 * greedily from the script's start, each as late as allowed. For today's profile they fall at 120,
 * 240, 360, 480, 600, 720, 840 and 950 s, and the last window runs from 950 s to the end at
 * 1,230 s.
 */

import type { SpikeSegmentSpan } from "../../../../preload/api";

/** The longest window of script time, s, other than the last. */
export const TRACE_WINDOW_MAX_S = 120;

/**
 * The longest window, s, for the last, which holds the busy stretch from its start widened by the
 * clearance to the script's end.
 */
export const TRACE_BUSY_WINDOW_MAX_S = 300;

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
 * The segments of the busy stretch, which the last window holds unbroken to the script's end: the
 * approach and flare, the low fast pass, the slowdown and the vertical descent, where streaming
 * peaks (Design note 21: a failure near the ground must not be averaged away).
 */
export const TRACE_BUSY_SEGMENTS: ReadonlyArray<string> = [
  "approach and flare",
  "low fast pass",
  "slowdown",
  "vertical descent",
];

/** Where the busy stretch begins, the first busy segment's start, s, or `null` for none. */
function busyStartOf(spans: ReadonlyArray<SpikeSegmentSpan>): number | null {
  const busy = spans.filter(({ name }) => TRACE_BUSY_SEGMENTS.includes(name));
  return busy.length === 0 ? null : Math.min(...busy.map(({ startS }) => startS));
}

/**
 * The script times at which the trace is stopped and started again, ascending, s.
 *
 * @param segmentSpans - The descent's segments in order (`DescentProfile.segmentSpans()`); the
 * script runs from 0 to the last one's end.
 * @returns One fewer than the windows: none for a script that fits in one window. With a busy
 * stretch, none lies after its start widened by the clearance.
 * @throws Error if the last window, from the latest boundary allowed before the widened busy
 * stretch to the script's end, would be longer than {@link TRACE_BUSY_WINDOW_MAX_S}, or if
 * segment changes leave no allowed boundary within a window's reach.
 */
export function traceBoundaries(segmentSpans: ReadonlyArray<SpikeSegmentSpan>): number[] {
  const endS = Math.max(0, ...segmentSpans.map((span) => span.endS));
  const changesS = segmentSpans.slice(1).map(({ startS }) => startS);
  const busyFromS = busyStartOf(segmentSpans);
  const busy = busyFromS !== null;
  const clearance = TRACE_CHANGE_CLEARANCE_S;
  // No boundary lies after this: the busy stretch's start less the clearance, or the script's end
  // without a busy stretch.
  const lastFromS = busyFromS === null ? endS : busyFromS - clearance;
  if (busy && endS - Math.max(0, lastFromS) > TRACE_BUSY_WINDOW_MAX_S) {
    throw new Error(
      `the last window, from the busy stretch's start less its clearance to the script's end, is ${endS - Math.max(0, lastFromS)} s, more than the ${TRACE_BUSY_WINDOW_MAX_S} s one window may hold`,
    );
  }
  const allowed = (t: number): boolean =>
    changesS.every((c) => Math.abs(t - c) >= clearance) && t <= lastFromS;
  // Each forbidden stretch is open, so the latest allowed time at or before a limit is the limit
  // itself or the lower edge of a forbidden stretch.
  const edgesS = [...changesS.map((c) => c - clearance), lastFromS];
  const latestAllowed = (afterS: number, limitS: number): number | null => {
    const candidates = [limitS, ...edgesS].filter((t) => t > afterS && t <= limitS && allowed(t));
    return candidates.length === 0 ? null : Math.max(...candidates);
  };
  const lastMaxS = busy ? TRACE_BUSY_WINDOW_MAX_S : TRACE_WINDOW_MAX_S;
  const boundaries: number[] = [];
  let fromS = 0;
  for (;;) {
    // The rest fits in the last window; with a busy stretch, the rest always holds it.
    if (endS - fromS <= lastMaxS) {
      return boundaries;
    }
    const next = latestAllowed(fromS, fromS + TRACE_WINDOW_MAX_S);
    if (next === null) {
      throw new Error(
        busy
          ? `no trace boundary can be placed after ${fromS} s, and the last window from there to the script's end, ${endS - fromS} s, is more than the ${TRACE_BUSY_WINDOW_MAX_S} s one window may hold`
          : `no trace boundary can be placed after ${fromS} s`,
      );
    }
    boundaries.push(next);
    fromS = next;
  }
}

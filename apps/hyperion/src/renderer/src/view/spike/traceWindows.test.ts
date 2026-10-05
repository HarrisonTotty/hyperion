import { describe, expect, it } from "vitest";

import type { SpikeSegmentSpan } from "../../../../preload/api";
import { recordProfile } from "./demandRecord";
import {
  TRACE_BUSY_SEGMENTS,
  TRACE_BUSY_WINDOW_MAX_S,
  TRACE_CHANGE_CLEARANCE_S,
  TRACE_WINDOW_MAX_S,
  traceBoundaries,
} from "./traceWindows";

/** Today's descent: 0, 60, 960, 1,080, 1,110, 1,170, 1,180 and 1,230 s. */
const TODAY = recordProfile().segmentSpans();

/** Segments of the given durations, s, named as given. */
function spansOf(segments: ReadonlyArray<readonly [string, number]>): SpikeSegmentSpan[] {
  let startS = 0;
  return segments.map(([name, durationS]) => {
    const span = { name, startS, endS: startS + durationS };
    startS += durationS;
    return span;
  });
}

describe("the trace's boundaries", () => {
  it("fall at the ruling's times for today's profile", () => {
    expect(traceBoundaries(TODAY)).toEqual([120, 240, 360, 480, 600, 720, 840, 950]);
  });

  it("keep every window within its length, the last holding the busy segments to the end", () => {
    const boundaries = traceBoundaries(TODAY);
    const endS = TODAY.at(-1)?.endS ?? 0;
    const edges = [0, ...boundaries, endS];
    const busy = TODAY.filter(({ name }) => TRACE_BUSY_SEGMENTS.includes(name));
    const busyFromS = Math.min(...busy.map(({ startS }) => startS));
    for (let k = 1; k < edges.length - 1; k += 1) {
      expect((edges[k] ?? 0) - (edges[k - 1] ?? 0)).toBeLessThanOrEqual(TRACE_WINDOW_MAX_S);
    }
    // The last window: from the busy stretch's start less the clearance, 950 s, to 1,230 s.
    const lastFromS = boundaries.at(-1) ?? 0;
    expect(lastFromS).toBe(busyFromS - TRACE_CHANGE_CLEARANCE_S);
    expect(endS - lastFromS).toBe(280);
    expect(endS - lastFromS).toBeLessThanOrEqual(TRACE_BUSY_WINDOW_MAX_S);
    for (const boundary of boundaries) {
      for (const { startS } of TODAY.slice(1)) {
        expect(Math.abs(boundary - startS)).toBeGreaterThanOrEqual(TRACE_CHANGE_CLEARANCE_S);
      }
    }
  });

  it("place none after the busy stretch's start, however long the segments after it", () => {
    const spans = spansOf([
      ["orbit coast", 60],
      ["descent arc", 300],
      ["approach and flare", 100],
      ["low fast pass", 30],
      ["hover and touchdown", 150],
    ]);
    // The busy stretch starts at 360 s; the last window holds 350 s to the end at 640 s.
    expect(traceBoundaries(spans)).toEqual([120, 240, 350]);
  });

  it("throw for a profile whose last window would exceed 300 s", () => {
    const long = spansOf([
      ["orbit coast", 60],
      ["approach and flare", 250],
      ["low fast pass", 40],
      ["hover and touchdown", 30],
    ]);
    expect(TRACE_BUSY_WINDOW_MAX_S).toBe(300);
    // From 50 s, the approach's start less the clearance, to the end at 380 s.
    expect(() => traceBoundaries(long)).toThrow(
      /the last window, .* is 330 s, more than the 300 s/,
    );
  });

  it("throw when a change's clearance moves the last boundary too early for the last window", () => {
    // The busy stretch starts at 60 s, but 50 s is within 10 s of the change at 55 s, so the last
    // boundary allowed is 45 s, 303 s before the end.
    const spans = spansOf([
      ["orbit coast", 55],
      ["descent arc", 5],
      ["approach and flare", 250],
      ["hover and touchdown", 38],
    ]);
    expect(() => traceBoundaries(spans)).toThrow(
      /no trace boundary can be placed after 45 s, and the last window from there to the script's end, 303 s/,
    );
  });

  it("are none for a script that fits in one window", () => {
    expect(traceBoundaries(spansOf([["orbit coast", 100]]))).toEqual([]);
    expect(traceBoundaries([])).toEqual([]);
  });

  it("move earlier rather than fall within a change's clearance", () => {
    // A change at 125 s: 120 s is 5 s from it, so the boundary falls at 115 s.
    expect(
      traceBoundaries(
        spansOf([
          ["descent arc", 125],
          ["orbit coast", 100],
        ]),
      ),
    ).toEqual([115]);
  });
});

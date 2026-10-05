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
    expect(traceBoundaries(TODAY)).toEqual([120, 240, 360, 480, 600, 720, 840, 950, 1190]);
  });

  it("keep every window within its length and every boundary clear of every change", () => {
    const boundaries = traceBoundaries(TODAY);
    const endS = TODAY.at(-1)?.endS ?? 0;
    const edges = [0, ...boundaries, endS];
    const busy = TODAY.filter(({ name }) => TRACE_BUSY_SEGMENTS.includes(name));
    const busyFromS = Math.min(...busy.map(({ startS }) => startS));
    const busyToS = Math.max(...busy.map(({ endS: e }) => e));
    let busyWindows = 0;
    for (let k = 1; k < edges.length; k += 1) {
      const fromS = edges[k - 1] ?? 0;
      const toS = edges[k] ?? 0;
      const holdsBusy = fromS <= busyFromS && toS >= busyToS;
      busyWindows += holdsBusy ? 1 : 0;
      expect(toS - fromS).toBeLessThanOrEqual(
        holdsBusy ? TRACE_BUSY_WINDOW_MAX_S : TRACE_WINDOW_MAX_S,
      );
    }
    expect(busyWindows).toBe(1);
    for (const boundary of boundaries) {
      for (const { startS } of TODAY.slice(1)) {
        expect(Math.abs(boundary - startS)).toBeGreaterThanOrEqual(TRACE_CHANGE_CLEARANCE_S);
      }
    }
  });

  it("throws for a busy stretch that with its clearances exceeds one window", () => {
    const long = spansOf([
      ["orbit coast", 60],
      ["approach and flare", 150],
      ["low fast pass", 40],
      ["slowdown", 40],
      ["hover and touchdown", 30],
    ]);
    expect(() => traceBoundaries(long)).toThrow(/busy stretch with its clearances is 250 s/);
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

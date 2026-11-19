/**
 * A resolve's GPU time from its passes' timestamp spans, and each pass's own share of it (lane
 * fix-mac-gpu-time).
 *
 * @remarks
 * On a serial timer (SwiftShader, the RTX 3080, the UHD 620) each pass begins at or after the end
 * of the one before it, and the union of the spans is their sum, to the nanosecond. On Metal they
 * overlap: the owner's M5 stamps the atmosphere's composite from before the march it reads has
 * ended, so its span holds that wait (the 2026-11-08 Mac batch: in all 70 runs the composite reads
 * the passes encoded before it plus about 0.5 ms), and a sum counted the march twice, 24.3 ms of
 * GPU in a frame drawn every 16.7 ms.
 *
 * A span is skipped, taking no time, where its end precedes its begin (a reset of the timestamp
 * counter, which the still's unit tally in `photoreal/renderer.ts` also leaves unmeasured), where
 * it ends at its begin (an empty pass, or one either of whose stamps was never written), or where
 * either end is not finite.
 */

import type { PassTimes } from "./types";

/** What the spans are read from: a pass's two stamps, ns from its resolve's earliest. */
export type PassSpan = Pick<PassTimes["passes"][number], "beginNs" | "endNs">;

/**
 * Each pass's own time, ns, in the passes' order: in its resolve's order of end stamps (the
 * passes' order between equal ends), the part of its span that no pass ending before it covers;
 * so each instant goes to the pass, of those whose spans hold it, that ends first. 0 for a
 * skipped span.
 *
 * @remarks
 * A pass that waits on another is stamped from the wait's start, so the end of its span is its
 * own work and the start is the other's: the composite whose span holds the march's dispatches
 * keeps only what runs after the last of them ends. Where no span holds another, this is each
 * pass's end less the later of its begin and the previous pass's end. The own times sum to the
 * union of the spans ({@link passUnionNs}), so the breakdown's total is the frame's; on a serial
 * timer each is its span, `endNs − beginNs`.
 */
export function passOwnNs(passes: ReadonlyArray<PassSpan>): number[] {
  const own = passes.map(() => 0);
  const byEnd = passes
    .flatMap((pass, index) => (counted(pass) ? [{ pass, index }] : []))
    .toSorted((a, b) => a.pass.endNs - b.pass.endNs);
  // The union of the spans taken so far, as disjoint intervals in order of begin.
  const covered: Array<{ beginNs: number; endNs: number }> = [];
  for (const { pass, index } of byEnd) {
    let coveredNs = 0;
    for (const interval of covered) {
      coveredNs += Math.max(
        0,
        Math.min(interval.endNs, pass.endNs) - Math.max(interval.beginNs, pass.beginNs),
      );
    }
    own[index] = pass.endNs - pass.beginNs - coveredNs;
    addInterval(covered, pass.beginNs, pass.endNs);
  }
  return own;
}

/**
 * The time, ns, in which any of a resolve's passes ran: the union of their spans, each skipped
 * span taking none; the sum of their {@link passOwnNs}.
 */
export function passUnionNs(passes: ReadonlyArray<PassSpan>): number {
  const byBegin = passes.filter(counted).toSorted((a, b) => a.beginNs - b.beginNs);
  let unionNs = 0;
  let reachedNs = Number.NEGATIVE_INFINITY;
  for (const { beginNs, endNs } of byBegin) {
    unionNs += Math.max(0, endNs - Math.max(beginNs, reachedNs));
    reachedNs = Math.max(reachedNs, endNs);
  }
  return unionNs;
}

/** Whether a span takes time: both ends finite, its end after its begin. */
function counted({ beginNs, endNs }: PassSpan): boolean {
  return Number.isFinite(beginNs) && Number.isFinite(endNs) && endNs > beginNs;
}

/** Unites `[beginNs, endNs)` with `covered`, disjoint intervals in order of begin, in place. */
function addInterval(
  covered: Array<{ beginNs: number; endNs: number }>,
  beginNs: number,
  endNs: number,
): void {
  let merged = { beginNs, endNs };
  const kept = covered.filter((interval) => {
    if (interval.endNs < merged.beginNs || interval.beginNs > merged.endNs) {
      return true;
    }
    merged = {
      beginNs: Math.min(merged.beginNs, interval.beginNs),
      endNs: Math.max(merged.endNs, interval.endNs),
    };
    return false;
  });
  const at = kept.findIndex((interval) => interval.beginNs > merged.beginNs);
  kept.splice(at === -1 ? kept.length : at, 0, merged);
  covered.splice(0, covered.length, ...kept);
}

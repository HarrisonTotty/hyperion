import { describe, expect, it } from "vitest";

import type { SpikeSizesPass, SpikeSizesReport } from "../preload/api";
import { sizesReportOf } from "./fixtures/sizesReport";
import { checkSchema, isSpikeSizesReport, sizesReportProblems } from "./sizesSchema";

/** The fixture's block, a test's change made to a copy of it. */
function changed(change: (block: SpikeSizesReport) => unknown): unknown {
  return change(sizesReportOf());
}

describe("the measured sizes' check (R10.T14.b)", () => {
  it("passes a well-formed block of one pass, of two, and of a pass with its stand-in", () => {
    expect([
      sizesReportProblems(sizesReportOf()),
      sizesReportProblems(
        sizesReportOf({
          samples: 5,
          passes: [
            { cache: "lit", views: 1 },
            { cache: "wireframe", views: 1 },
          ],
        }),
      ),
      sizesReportProblems(
        sizesReportOf({
          samples: 5,
          passes: [
            { cache: "lit", views: 2 },
            { cache: "wireframe", views: 0 },
          ],
        }),
      ),
    ]).toEqual([[], [], []]);
    expect(isSpikeSizesReport(sizesReportOf())).toBe(true);
  });

  it("passes a block with no cascades allocated, and why, and one whose cascades differ from the spec, flagged", () => {
    const block = sizesReportOf();
    expect([
      sizesReportProblems(noCascades(block, "the low setting has no shadow cascades")),
      sizesReportProblems(withCascades(block, { specBytes: 1_024, differsFromSpec: true })),
    ]).toEqual([[], []]);
  });

  it.each<[string, (pass: SpikeSizesPass) => unknown, string]>([
    [
      "a stand-in's samples",
      (pass) => ({ ...pass, series: sizesReportOf().passes[0]?.series ?? null }),
      "sizes.passes[1].series is given for a pass that describes no view",
    ],
    [
      "a stand-in with a rule",
      (pass) => ({ ...pass, rule: sizesReportOf().passes[0]?.rule ?? null }),
      "sizes.passes[1].rule is null exactly where the pass describes no view",
    ],
    [
      "a stand-in not marked so",
      (pass) => ({ ...pass, notSized: null }),
      "sizes.passes[1].notSized is given exactly where the pass describes no view",
    ],
  ])("refuses %s", (_, change, problem) => {
    const block = sizesReportOf({
      samples: 3,
      passes: [
        { cache: "lit", views: 2 },
        { cache: "wireframe", views: 0 },
      ],
    });
    const [sized, standIn] = block.passes;
    if (sized === undefined || standIn === undefined) {
      throw new Error("the block holds a pass and its stand-in");
    }
    expect(sizesReportProblems({ ...block, passes: [sized, change(standIn)] })).toContain(problem);
  });

  it("refuses a sized pass without its samples", () => {
    const value = withFirstPass(sizesReportOf(), (pass) => ({ ...pass, series: null }));
    expect(sizesReportProblems(value)).toContain(
      "sizes.passes[0].series is null for a pass that describes a view",
    );
  });

  it.each<[string, (block: SpikeSizesReport) => unknown, string]>([
    ["a block that is no object", () => "sizes", "sizes is not an object"],
    [
      "a field the block does not have",
      (b) => ({ ...b, extra: 1 }),
      "sizes.extra is not a field of the block",
    ],
    ["a field missing", ({ readouts: _readouts, ...rest }) => rest, "sizes.readouts is missing"],
    [
      "a count that is not whole",
      (b) => ({ ...b, readouts: { ...b.readouts, publishes: 1.5 } }),
      "sizes.readouts.publishes is not a whole count",
    ],
    [
      "a figure that is not finite",
      (b) => ({ ...b, memory: { ...b.memory, workerFieldBytes: { bytes: 1, atS: Number.NaN } } }),
      "sizes.memory.workerFieldBytes.atS is not a finite number",
    ],
    [
      "a choice that is not one",
      (b) => ({ ...b, secondView: { ...b.secondView, style: "lit" } }),
      "sizes.secondView.style is not one of photorealistic, wireframe",
    ],
    [
      "a figure with both a value and a reason",
      (b) => ({ ...b, sampledLowestM: { value: -1, reason: "both" } }),
      "sizes.sampledLowestM has both a value and a reason",
    ],
    [
      "a figure missing without a reason",
      (b) => ({ ...b, sampledLowestM: { value: null, reason: "" } }),
      "sizes.sampledLowestM is null without a reason",
    ],
    [
      "a column shorter than the samples",
      (b) =>
        withFirstPass(b, (pass) => ({
          ...pass,
          series: { ...pass.series, forcedResident: { samples: [1], gaps: [] } },
        })),
      "sizes.passes[0].series.forcedResident has 1 samples, not the pass's 3",
    ],
    [
      "a missing sample outside a gap",
      (b) =>
        withFirstPass(b, (pass) => ({
          ...pass,
          series: { ...pass.series, usedSlots: { samples: [1, -1, 3], gaps: [] } },
        })),
      "sizes.passes[0].series.usedSlots.samples[1] is -1 outside a gap, or not -1 within one",
    ],
    [
      "a pass of two views without its unions",
      (b) => withFirstPass(b, (pass) => ({ ...pass, series: { ...pass.series, union: null } })),
      "sizes.passes[0].series.union is given exactly where the pass describes other than one view",
    ],
    [
      "a segment index past the segments",
      (b) =>
        withFirstPass(b, (pass) => ({
          ...pass,
          series: {
            ...pass.series,
            segmentRuns: [
              { segment: 0, from: 0 },
              { segment: 13, from: 1 },
            ],
          },
        })),
      "sizes.passes[0].series.segmentRuns indexes past the block's 13 segments",
    ],
    [
      "segment runs out of order",
      (b) =>
        withFirstPass(b, (pass) => ({
          ...pass,
          series: {
            ...pass.series,
            segmentRuns: [
              { segment: 0, from: 1 },
              { segment: 1, from: 0 },
            ],
          },
        })),
      "sizes.passes[0].series.segmentRuns is not runs of the samples from the first, in order",
    ],
    [
      "skipped seconds out of order",
      (b) =>
        withFirstPass(b, (pass) => ({
          ...pass,
          series: { ...pass.series, seconds: { first: 5, skipped: [7, 6] } },
        })),
      "sizes.passes[0].series.seconds.skipped is not ascending after the first second",
    ],
    [
      "cascades of another source",
      (b) => withCascades(b, { source: "computed" }),
      "sizes.memory.cascades.source is not one of engine, none",
    ],
    [
      "no cascades allocated, without a reason",
      (b) => noCascades(b, null),
      "sizes.memory.cascades.reason is given exactly where none was allocated",
    ],
    [
      "cascades that are not the engine's shadows peak",
      (b) => withCascades(b, { bytes: 1_024 }),
      "sizes.memory.cascades.bytes is the engine's shadows peak, above 0 exactly where they were allocated",
    ],
    [
      "cascades unlike the spec's not flagged",
      (b) => withCascades(b, { specBytes: 1_024 }),
      "sizes.memory.cascades.differsFromSpec is set exactly where the allocated bytes differ from the spec's",
    ],
    [
      "an engine peak above the sum of its categories' peaks",
      (b) => ({
        ...b,
        memory: { ...b.memory, engine: { ...b.memory.engine, peak: { bytes: 2 ** 40, atS: 1 } } },
      }),
      "sizes.memory.engine.peak.bytes lies between its largest category's peak and their sum",
    ],
    [
      "an engine peak below its largest category's",
      (b) => ({
        ...b,
        memory: { ...b.memory, engine: { ...b.memory.engine, peak: { bytes: 1, atS: null } } },
      }),
      "sizes.memory.engine.peak.bytes lies between its largest category's peak and their sum",
    ],
    [
      "a view's peaks for another number of views",
      (b) =>
        withFirstPass(b, (pass) => ({
          ...pass,
          counts: { ...pass.counts, allRound: { ...pass.counts.allRound, views: [] } },
        })),
      "sizes.passes[0].counts.allRound has 0 views' peaks, not 2",
    ],
  ])("refuses %s", (_, change, problem) => {
    const value = changed(change);
    expect(sizesReportProblems(value)).toContain(problem);
    expect(isSpikeSizesReport(value)).toBe(false);
  });

  it("names a long list's first malformed item alone", () => {
    const problems: string[] = [];
    checkSchema([1, -2, -3, 4], { list: "count" }, "x", problems);
    expect(problems).toEqual(["x[1] is not a whole count"]);
  });
});

/** `block` with its cascades' fields in `change` changed. */
function withCascades(block: SpikeSizesReport, change: Readonly<Record<string, unknown>>): unknown {
  return {
    ...block,
    memory: { ...block.memory, cascades: { ...block.memory.cascades, ...change } },
  };
}

/** `block` with no cascades allocated: none under the engine's `shadows`, for `reason`. */
function noCascades(block: SpikeSizesReport, reason: string | null): SpikeSizesReport {
  const { memory } = block;
  const shadows = memory.engine.categories.find(({ category }) => category === "shadows");
  return {
    ...block,
    memory: {
      ...memory,
      cascades: { ...memory.cascades, bytes: 0, source: "none", reason, differsFromSpec: false },
      engine: {
        ...memory.engine,
        categories: memory.engine.categories.filter(({ category }) => category !== "shadows"),
        peak: { ...memory.engine.peak, bytes: memory.engine.peak.bytes - (shadows?.bytes ?? 0) },
      },
    },
  };
}

/** `block` with its first pass changed by `change`. */
function withFirstPass(
  block: SpikeSizesReport,
  change: (pass: SpikeSizesReport["passes"][number]) => unknown,
): unknown {
  const [first, ...rest] = block.passes;
  return first === undefined ? block : { ...block, passes: [change(first), ...rest] };
}

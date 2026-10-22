import { describe, expect, it } from "vitest";

import type {
  HandoverCheckRecord,
  HandoverComparisonRecord,
  HandoverLookRecord,
  HandoverTexelRecord,
} from "../preload/api";
import { HANDOVER_RECORD } from "./fixtures/handoverRecord";
import {
  buildHandoverCheckResults,
  HANDOVER_BOUND_STOPS,
  type HandoverCheckInput,
  handoverCheckMarkdown,
  handoverFailures,
  stopsBetween,
  texelFigures,
  writeHandoverResults,
} from "./handoverCheckResults";
import { measured, type ResultsFiles } from "./results";

function input(
  record: HandoverCheckRecord = HANDOVER_RECORD,
  overrides: Partial<Omit<HandoverCheckInput, "record">> = {},
): HandoverCheckInput {
  return {
    run: {
      startedAt: new Date("2026-10-21T09:00:00Z"),
      machine: {
        name: "effect",
        cpu: "AMD Ryzen 7 3700X",
        logicalCores: 16,
        memoryBytes: 32e9,
        governor: measured("schedutil"),
        loadAverage: [3.1, 3.4, 3.6],
        gpu: measured({ vendorId: 4318, deviceId: 8710, driverVersion: null, description: null }),
      },
      versions: { app: "0.1.0", electron: "44", chromium: "152", node: "24", v8: "15" },
      platform: "linux",
      launchMode: "vulkan",
      setting: "high",
      smoke: false,
      fixture: "terrain-test",
      switches: [],
      shown: false,
      window: { widthDip: 1926, heightDip: 1086 },
      captures: measured("target/views-check/terrain/captures-x"),
    },
    record,
    gpuProcessExits: 0,
    consoleErrors: [],
    ...overrides,
  };
}

/** The record with look `index` changed by `change`. */
function withLook(
  index: number,
  change: (look: HandoverLookRecord) => HandoverLookRecord,
): HandoverCheckInput {
  return input({
    ...HANDOVER_RECORD,
    looks: HANDOVER_RECORD.looks.map((look, i) => (i === index ? change(look) : look)),
  });
}

/** The record with look `index`'s first comparison changed by `change`. */
function withComparison(
  index: number,
  change: (comparison: HandoverComparisonRecord) => HandoverComparisonRecord,
): HandoverCheckInput {
  return withLook(index, (look) => ({
    ...look,
    comparisons: look.comparisons.map((each, i) => (i === 0 ? change(each) : each)),
  }));
}

/** A comparison with one more cell. */
function withTexel(texel: HandoverTexelRecord) {
  return (comparison: HandoverComparisonRecord): HandoverComparisonRecord => ({
    ...comparison,
    texels: [...comparison.texels, texel],
  });
}

/** The failures of the results of `given`, as one line. */
function failuresOf(given: HandoverCheckInput): string {
  return handoverFailures(buildHandoverCheckResults(given));
}

describe("the hand-over check's results", () => {
  it("pass a record within the bound everywhere", () => {
    const results = buildHandoverCheckResults(input());
    expect([results.overall, handoverFailures(results)]).toEqual(["pass", ""]);
  });

  it("keep every cell with a shared pixel, judged or not, with its change", () => {
    const comparison = buildHandoverCheckResults(input()).looks[0]?.comparisons[0];
    expect(comparison?.texels.map((texel) => [texel.texel, texel.judged])).toEqual([
      [19_200, true],
      [19_201, true],
      [19_265, false],
    ]);
  });

  it("judge no cell of a single shared pixel, however it changed", () => {
    // The fixture's third cell doubles over its one pixel.
    const comparison = buildHandoverCheckResults(input()).looks[0]?.comparisons[0];
    expect([comparison?.texels[2]?.log2, comparison?.texelsOver]).toEqual([1, 0]);
  });

  it("fail a judged look with a cell over 1/3 stop", () => {
    const results = buildHandoverCheckResults(
      withComparison(
        1,
        withTexel({ texel: 20_000, sharedPixels: 2, meanBefore: 0.2, meanAfter: 0.27 }),
      ),
    );
    expect([results.overall, results.looks[1]?.comparisons[0]?.texelsOver]).toEqual(["fail", 1]);
  });

  it("count a cell dark on one side among those over, the worst", () => {
    const comparison = buildHandoverCheckResults(
      withComparison(
        0,
        withTexel({ texel: 20_000, sharedPixels: 3, meanBefore: 0.2, meanAfter: 0 }),
      ),
    ).looks[0]?.comparisons[0];
    expect([comparison?.darkTexels, comparison?.texelsOver, comparison?.worst?.dark]).toEqual([
      1,
      1,
      "after",
    ]);
  });

  it("record the terminator's cells over 1/3 stop without judging them", () => {
    const results = buildHandoverCheckResults(
      withComparison(
        2,
        withTexel({ texel: 20_000, sharedPixels: 4, meanBefore: 0.01, meanAfter: 0.02 }),
      ),
    );
    const comparison = results.looks[2]?.comparisons[0];
    expect([results.overall, comparison?.verdict, comparison?.texelsOver]).toEqual([
      "pass",
      "recorded",
      1,
    ]);
  });

  it("judge a look by its name, not by the record's word", () => {
    // The terminator's record claims to be judged; its name says it is recorded.
    const results = buildHandoverCheckResults(
      withLook(2, (look) => ({
        ...look,
        judged: true,
        comparisons: look.comparisons.map((each, i) =>
          i === 0
            ? withTexel({ texel: 20_000, sharedPixels: 4, meanBefore: 0.01, meanAfter: 0.02 })(each)
            : each,
        ),
      })),
    );
    expect(results.looks[2]?.comparisons[0]?.verdict).toBe("recorded");
  });

  it.each([
    ["body", "own", "the body over its own pixels changed by \\+0\\.379"],
    ["body", "shared", "the body over its shared pixels changed by \\+0\\.379"],
    ["survey", "own", "the survey over its own pixels changed by \\+0\\.379"],
    ["survey", "shared", "the survey over its shared pixels changed by \\+0\\.379"],
  ] as const)(
    "fail a judged look whose %s's mean over its %s pixels moved by more than 1/3 stop",
    (part, set, reason) => {
      const given = withComparison(0, (comparison) => ({
        ...comparison,
        [part]: {
          ...comparison[part],
          [set]: { ...comparison[part][set], meanAfter: comparison[part][set].meanBefore * 1.3 },
        },
      }));
      expect(failuresOf(given)).toMatch(new RegExp(reason, "u"));
    },
  );

  it("fail a judged look's mean over no pixel", () => {
    const given = withComparison(0, (comparison) => ({
      ...comparison,
      body: { ...comparison.body, shared: { pixels: 0, meanBefore: 0, meanAfter: 0 } },
    }));
    expect(failuresOf(given)).toMatch(/the body over its shared pixels: no pixel/u);
  });

  it("fail a judged look with no cell of 2 shared pixels to judge", () => {
    const given = withComparison(0, (comparison) => ({
      ...comparison,
      texels: comparison.texels.map((texel) => ({ ...texel, sharedPixels: 1 })),
    }));
    expect(failuresOf(given)).toMatch(/no cell of 2 shared pixels or more to judge/u);
  });

  it("fail a look whose exposure was not locked", () => {
    const given = withLook(0, (look) => ({
      ...look,
      exposure: { lockedEv100: null, lockedReading: null, endReading: null, held: true },
    }));
    expect(failuresOf(given)).toBe("handover-30: the exposure was not locked");
  });

  it("fail a look whose exposure's lock did not hold", () => {
    const given = withLook(0, (look) => ({
      ...look,
      exposure: { ...look.exposure, endReading: "EV100 14.0 AUTO", held: false },
    }));
    expect(failuresOf(given)).toMatch(/lock did not hold \(it read EV100 14\.0 AUTO/u);
  });

  it("fail the terminator's look when a switch was not compared", () => {
    const given = withLook(2, (look) => ({ ...look, comparisons: look.comparisons.slice(0, 2) }));
    expect(failuresOf(given)).toBe("handover-terminator: terrain-to-disc was not compared");
  });

  it("fail a frame whose body pixels mostly missed the datum (the camera model)", () => {
    const given = withComparison(0, (comparison) => ({
      ...comparison,
      after: { ...comparison.after, bodyMisses: 30_000 },
    }));
    expect(failuresOf(given)).toMatch(/after: 30000 of 44000 body pixels missed the datum/u);
  });

  it("fail a look whose cells' centres did not map back to their cells", () => {
    const given = withLook(0, (look) => ({ ...look, mappingMismatches: 2 }));
    expect(failuresOf(given)).toBe(
      "handover-30: 2 surveyed cells' centres did not map back to their cells",
    );
  });

  it("fail a look that held no surveyed cell", () => {
    const given = withLook(0, (look) => ({ ...look, surveyedCells: 0 }));
    expect(failuresOf(given)).toBe("handover-30: no surveyed cell was held");
  });

  it("fail a look with a harness failure", () => {
    const given = withLook(1, (look) => ({ ...look, failures: ["a read-back was refused"] }));
    expect(failuresOf(given)).toBe("handover-90: a read-back was refused");
  });

  it("fail a run whose looks did not all run", () => {
    const given = input({ ...HANDOVER_RECORD, looks: HANDOVER_RECORD.looks.slice(0, 2) });
    expect(failuresOf(given)).toBe(
      "the looks ran handover-30, handover-90, where handover-30, handover-90, handover-terminator are asked",
    );
  });

  it("fail a run that met an engine fault", () => {
    const given = input({
      ...HANDOVER_RECORD,
      faults: [{ atMs: 1, kind: "device-lost", reason: "unknown", line: "device-lost: gone" }],
    });
    expect(failuresOf(given)).toBe("a fault: device-lost: gone");
  });

  it("fail a run in which the GPU process exited", () => {
    expect(failuresOf(input(HANDOVER_RECORD, { gpuProcessExits: 1 }))).toBe(
      "1 exits of the GPU process",
    );
  });

  it("record the console's errors without judging them", () => {
    const results = buildHandoverCheckResults(
      input(HANDOVER_RECORD, { consoleErrors: ["one", "two"] }),
    );
    expect([results.overall, results.findings.consoleErrors]).toEqual([
      "pass",
      { count: 2, first: ["one", "two"] },
    ]);
  });

  it("take a change in stops as log₂ of the ratio, none where a side is dark", () => {
    expect([stopsBetween(0.2, 0.4), stopsBetween(0, 0.4), stopsBetween(0.2, 0)]).toEqual([
      1,
      null,
      null,
    ]);
  });

  it("read a cell dark on both sides as unchanged", () => {
    const both = texelFigures({ texel: 1, sharedPixels: 2, meanBefore: 0, meanAfter: 0 });
    expect([both.dark, both.log2, both.over]).toEqual(["both", null, false]);
  });

  it("hold a cell to a third of a stop either way", () => {
    const within = texelFigures({ texel: 1, sharedPixels: 2, meanBefore: 1, meanAfter: 1.25 });
    const beyond = texelFigures({ texel: 2, sharedPixels: 2, meanBefore: 1.27, meanAfter: 1 });
    expect([HANDOVER_BOUND_STOPS, within.over, beyond.over]).toEqual([1 / 3, false, true]);
  });
});

describe("the hand-over check's summary", () => {
  /** The summary of a run with one cell over the bound at 90°. */
  function summary(): string {
    return handoverCheckMarkdown(
      buildHandoverCheckResults(
        withComparison(
          1,
          withTexel({ texel: 20_000, sharedPixels: 2, meanBefore: 0.2, meanAfter: 0.3 }),
        ),
      ),
    );
  }

  it("is titled for its machine, setting and date", () => {
    expect(summary().split("\n")[0]).toBe("# Hand-over check: effect, high, 2026-10-21");
  });

  it("lists the cells over 1/3 stop with their means and change", () => {
    expect(summary()).toContain(
      "| handover-90 | disc-to-figure | 20000 | 2 | 0.2000 | 0.3000 | +0.585 |",
    );
  });

  it("states the hand-over range", () => {
    expect(summary()).toContain("the hand-over range 15419 km");
  });
});

/** Files held in a map, by path. */
function inMemory(): { readonly files: ResultsFiles; readonly written: Map<string, string> } {
  const written = new Map<string, string>();
  return {
    written,
    files: {
      mkdir: () => Promise.resolve(),
      exists: (path) => Promise.resolve(written.has(path)),
      writeFile: (path, text) => {
        written.set(path, text);
        return Promise.resolve();
      },
    },
  };
}

describe("the hand-over check's files", () => {
  it("are named for their date, machine and setting, a smoke's and a repeat's apart", async () => {
    const { files } = inMemory();
    const results = buildHandoverCheckResults(input());
    const first = await writeHandoverResults("/out", results, files);
    const second = await writeHandoverResults("/out", results, files);
    const smoke = await writeHandoverResults(
      "/out",
      buildHandoverCheckResults({ ...input(), run: { ...input().run, smoke: true } }),
      files,
    );
    expect([first.json, second.json, smoke.markdown]).toEqual([
      "/out/2026-10-21-effect-high-handover.json",
      "/out/2026-10-21-effect-high-handover-2.json",
      "/out/2026-10-21-effect-high-handover-smoke.md",
    ]);
  });

  it("hold the results as JSON of their schema", async () => {
    const { files, written } = inMemory();
    const { json } = await writeHandoverResults("/out", buildHandoverCheckResults(input()), files);
    expect(JSON.parse(written.get(json) ?? "null")).toMatchObject({
      schema: "hyperion.views-check.handover-results",
      version: 1,
      overall: "pass",
      rangeM: 1.5419e7,
    });
  });

  it("are named -handover-weathered for a weathered run, its run block and summary naming its field (R10.T1.c)", async () => {
    const { files, written } = inMemory();
    const weathered = (smoke: boolean): HandoverCheckInput => ({
      ...input(),
      run: { ...input().run, smoke, fixture: "terrain-test-weathered" },
    });
    const full = await writeHandoverResults(
      "/out",
      buildHandoverCheckResults(weathered(false)),
      files,
    );
    const smoke = await writeHandoverResults(
      "/out",
      buildHandoverCheckResults(weathered(true)),
      files,
    );
    expect([full.json, smoke.markdown]).toEqual([
      "/out/2026-10-21-effect-high-handover-weathered.json",
      "/out/2026-10-21-effect-high-handover-weathered-smoke.md",
    ]);
    expect(JSON.parse(written.get(full.json) ?? "null")).toMatchObject({
      run: { smoke: false, fixture: "terrain-test-weathered" },
    });
    expect(written.get(full.markdown)).toContain(
      "- **Field:** `terrain-test-weathered`, TEST MOON weathered (`terrain_test_weathered.golden`), served in TERRAIN TEST's place",
    );
  });

  it("keep TERRAIN TEST's run block, and its summary, as before the option (R10.T1.c)", () => {
    const results = buildHandoverCheckResults(input());
    expect([
      Object.keys(results.run),
      handoverCheckMarkdown(results).includes("**Field:**"),
    ]).toEqual([
      [
        "startedAt",
        "machine",
        "versions",
        "platform",
        "launchMode",
        "setting",
        "smoke",
        "switches",
        "shown",
        "window",
        "devicePixelRatio",
        "captures",
        "provisional",
      ],
      false,
    ]);
  });
});

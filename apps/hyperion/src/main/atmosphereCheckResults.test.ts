import { describe, expect, it } from "vitest";

import type { AtmosphereCheckCapture, AtmosphereCheckRecord } from "../preload/api";
import {
  ATMOSPHERE_CHECK_SCHEMA,
  type AtmosphereCheckInput,
  atmosphereCaptureFailures,
  atmosphereCheckMarkdown,
  atmosphereFailures,
  atmosphereFaultKind,
  buildAtmosphereCheckResults,
  rangeText,
  writeAtmosphereResults,
} from "./atmosphereCheckResults";
import { ATMOSPHERE_RECORD } from "./fixtures/atmosphereRecord";
import { measured, missing, type ResultsFiles } from "./results";

/** The atmosphere's fault line for TEST MARS (the renderer's `atmosphereNotAvailableLine`). */
const NOT_AVAILABLE =
  "ATMOSPHERE NOT AVAILABLE: computation failed for TEST MARS, reopen VIEW to retry";

function input(
  record: AtmosphereCheckRecord = ATMOSPHERE_RECORD,
  overrides: Partial<Omit<AtmosphereCheckInput, "record">> = {},
): AtmosphereCheckInput {
  return {
    run: {
      startedAt: new Date("2026-10-26T09:00:00Z"),
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
      switches: [],
      shown: false,
      window: { widthDip: 1926, heightDip: 1086 },
      captures: measured("target/views-check/atmosphere/captures-x"),
    },
    record,
    gpuProcessExits: 0,
    consoleErrors: [],
    ...overrides,
  };
}

/** The record with capture `index`'s fault lines set to `faults`. */
function withFaults(index: number, faults: ReadonlyArray<string>): AtmosphereCheckRecord {
  return {
    ...ATMOSPHERE_RECORD,
    captures: ATMOSPHERE_RECORD.captures.map((capture, i): AtmosphereCheckCapture =>
      i === index ? { ...capture, faults } : capture,
    ),
  };
}

describe("the atmosphere run's pass rule", () => {
  it("passes a run whose captures were taken and saved, with no fault line", () => {
    expect(atmosphereCaptureFailures(ATMOSPHERE_RECORD, true)).toEqual([]);
  });

  it("fails a run that took no capture", () => {
    expect(atmosphereCaptureFailures({ ...ATMOSPHERE_RECORD, captures: [] }, true)).toEqual([
      "no capture was taken",
    ]);
  });

  it("fails a run whose captures the main process did not save", () => {
    expect(atmosphereCaptureFailures(ATMOSPHERE_RECORD, false)).toEqual([
      "the main process saved no capture",
    ]);
  });

  it("fails a capture that showed the atmosphere's fault, naming it and the line", () => {
    expect(atmosphereCaptureFailures(withFaults(2, [NOT_AVAILABLE]), true)).toEqual([
      `fixture-test-mars: atmosphere fault: ${NOT_AVAILABLE}`,
    ]);
  });

  it("fails a capture that showed a graphics fault, as a graphics fault", () => {
    const line = "GRAPHICS FAULT: device lost, reopen VIEW to retry";
    expect(atmosphereCaptureFailures(withFaults(4, [line]), true)).toEqual([
      `moonlit-night: graphics fault: ${line}`,
    ]);
  });

  it("classes the atmosphere's two lines as its own and every other as graphics", () => {
    expect(
      [
        NOT_AVAILABLE,
        "ATMOSPHERE DATA INVALID: optical depth not finite for TEST EARTH, reopen VIEW to retry",
        "TERRAIN NOT AVAILABLE: terrain computation failed, reopen VIEW to retry",
        "ATMOSPHERE: PENDING",
      ].map(atmosphereFaultKind),
    ).toEqual(["atmosphere", "atmosphere", "graphics", "graphics"]);
  });
});

describe("the atmosphere run's results", () => {
  it("pass the fixture's record", () => {
    const results = buildAtmosphereCheckResults(input());
    expect([results.schema, results.version, results.overall, atmosphereFailures(results)]).toEqual(
      [ATMOSPHERE_CHECK_SCHEMA, 1, "pass", ""],
    );
  });

  it("keep the run block, as the hand-over check's, without a field", () => {
    const results = buildAtmosphereCheckResults(input());
    expect([Object.keys(results.run), results.run.startedAt, results.run.provisional]).toEqual([
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
      "2026-10-26T09:00:00.000Z",
      "provisional: load average 3.10 at the start; a shared machine; a hidden run: no presentation",
    ]);
  });

  it("fail on a capture's fault line, classing it and judging that capture alone", () => {
    const results = buildAtmosphereCheckResults(input(withFaults(2, [NOT_AVAILABLE])));
    expect([
      results.overall,
      results.captures.map((capture) => capture.verdict),
      results.captures[2]?.faults,
      results.findings.captures.atmosphereFaults,
      results.findings.captures.graphicsFaults,
      atmosphereFailures(results),
    ]).toEqual([
      "fail",
      ["pass", "pass", "fail", "pass", "pass"],
      [{ kind: "atmosphere", line: NOT_AVAILABLE }],
      1,
      0,
      `fixture-test-mars: atmosphere fault: ${NOT_AVAILABLE}`,
    ]);
  });

  it("fail a run whose captures were not saved, the run block saying why", () => {
    const results = buildAtmosphereCheckResults({
      ...input(),
      run: { ...input().run, captures: missing("no capture was saved") },
    });
    expect([results.overall, results.run.captures.reason]).toEqual([
      "fail",
      "no capture was saved",
    ]);
  });

  it("join the reasons into one line for the session's end", () => {
    const results = buildAtmosphereCheckResults({
      ...input({ ...ATMOSPHERE_RECORD, captures: [] }),
      run: { ...input().run, captures: missing("no capture directory") },
    });
    expect(atmosphereFailures(results)).toBe(
      "no capture was taken; the main process saved no capture",
    );
  });

  it("record the engine's faults, the GPU process's exits and the console's errors without judging them", () => {
    const results = buildAtmosphereCheckResults(
      input(
        { ...ATMOSPHERE_RECORD, faults: ["device-lost: unknown"] },
        { gpuProcessExits: 1, consoleErrors: ["one", "two"] },
      ),
    );
    expect([results.overall, results.findings.faults, results.findings.consoleErrors]).toEqual([
      "pass",
      { record: ["device-lost: unknown"], gpuProcessExits: 1 },
      { count: 2, first: ["one", "two"] },
    ]);
  });

  it("note a smoke run's times as the harness's proof", () => {
    const results = buildAtmosphereCheckResults({
      ...input(),
      run: { ...input().run, smoke: true },
    });
    expect(results.run.provisional).toContain("a smoke run");
  });
});

describe("the atmosphere run's summary", () => {
  /** The summary of a run whose TEST MARS capture showed the atmosphere's fault. */
  function summary(): string {
    return atmosphereCheckMarkdown(
      buildAtmosphereCheckResults(input(withFaults(2, [NOT_AVAILABLE]))),
    );
  }

  it("is titled for its machine, setting and date", () => {
    expect(summary().split("\n")[0]).toBe("# Atmosphere flight: effect, high, 2026-10-26");
  });

  it("states the overall, the media's readiness, the captures' directory and the faults", () => {
    const text = summary();
    for (const row of [
      "- **Overall:** fail",
      "- **Media ready:** 4.2 s after the scene's choice",
      "- **Captures (not committed):** `target/views-check/atmosphere/captures-x`",
      "- **Faults:** fault lines 1 atmosphere, 0 graphics; recorded, not judged: 0 engine faults, 0 GPU-process exits, console errors 0",
    ]) {
      expect(text).toContain(row);
    }
  });

  it("lists each capture with its camera, range, phase, exposure, frames and statements", () => {
    const text = summary();
    expect(text).toContain(
      "| fixture-test-earth | TEST EARTH | FREE · RATE 1.00 Mm/s | 25.0 Mm | 60.0° | EV100 12.0 AUTO | 898 | 16.7 / 18.2 | 4.10 / 5.30 | PHOTOREALISTIC · TARGET TEST EARTH | — |",
    );
    expect(text).toContain(
      "| moonlit-night | — | SEAT | — | — | EV100 -1.0 AUTO | 1078 | 16.7 / 18.2 | 4.10 / 5.30 | PHOTOREALISTIC · NIGHT SIDE | — |",
    );
    expect(text).toContain(`| atmosphere: ${NOT_AVAILABLE} |`);
  });

  it("lists the reasons, or none", () => {
    expect(summary()).toContain(
      `## Reasons\n\n- fixture-test-mars: atmosphere fault: ${NOT_AVAILABLE}\n`,
    );
    expect(atmosphereCheckMarkdown(buildAtmosphereCheckResults(input()))).toContain(
      "## Reasons\n\nNone.\n",
    );
  });

  it("writes a range under 10 Mm in km and one from there in Mm", () => {
    expect([rangeText(6.371e6), rangeText(2.5e7), rangeText(null)]).toEqual([
      "6371 km",
      "25.0 Mm",
      "—",
    ]);
  });

  it("escapes a pipe in a cell", () => {
    const record = withFaults(0, ["GRAPHICS FAULT: a | b"]);
    expect(atmosphereCheckMarkdown(buildAtmosphereCheckResults(input(record)))).toContain(
      "| graphics: GRAPHICS FAULT: a \\| b |",
    );
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

describe("the atmosphere run's files", () => {
  it("are named for their date, machine and setting, a smoke's and a repeat's apart", async () => {
    const { files } = inMemory();
    const results = buildAtmosphereCheckResults(input());
    const first = await writeAtmosphereResults("/out", results, files);
    const second = await writeAtmosphereResults("/out", results, files);
    const third = await writeAtmosphereResults("/out", results, files);
    const smoke = await writeAtmosphereResults(
      "/out",
      buildAtmosphereCheckResults({ ...input(), run: { ...input().run, smoke: true } }),
      files,
    );
    expect([first.json, first.markdown, second.json, third.json, smoke.markdown]).toEqual([
      "/out/2026-10-26-effect-high-atmosphere.json",
      "/out/2026-10-26-effect-high-atmosphere.md",
      "/out/2026-10-26-effect-high-atmosphere-2.json",
      "/out/2026-10-26-effect-high-atmosphere-3.json",
      "/out/2026-10-26-effect-high-atmosphere-smoke.md",
    ]);
  });

  it("hold the results as JSON of their schema, and the summary beside them", async () => {
    const { files, written } = inMemory();
    const { json, markdown } = await writeAtmosphereResults(
      "/out",
      buildAtmosphereCheckResults(input()),
      files,
    );
    expect(JSON.parse(written.get(json) ?? "null")).toMatchObject({
      schema: "hyperion.views-check.atmosphere-results",
      version: 1,
      overall: "pass",
      mediaReadyMs: 4200,
    });
    expect(written.get(markdown)?.split("\n")[0]).toBe(
      "# Atmosphere flight: effect, high, 2026-10-26",
    );
  });
});

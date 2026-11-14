import { readdirSync, readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { foldSpikeResult } from "./fold.ts";
import { PerfRecordError, validatePerfRecord } from "./record.ts";

/**
 * R05's results file of 2026-10-07 at the low setting (`docs/measurements/descent-spike/`, version
 * 6, shown, on the RTX 3080), its long series cut short and its machine renamed.
 */
function fixture(): Record<string, unknown> {
  const text = readFileSync(new URL("../fixtures/descent-spike-v6.json", import.meta.url), "utf8");
  const value: unknown = JSON.parse(text);
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("not an object");
  return { ...value };
}

function runOf(file: Record<string, unknown>): Record<string, unknown> {
  const run = file["run"];
  if (typeof run !== "object" || run === null || Array.isArray(run)) throw new Error("no run");
  return { ...run };
}

describe("foldSpikeResult", () => {
  it("folds R05's results file into a valid record of one repetition", () => {
    const record = foldSpikeResult(fixture());
    expect(validatePerfRecord(record)).toEqual(record);
    expect(record).toMatchObject({
      id: "2026-10-07-rtx3080-desktop-cockpit-descent-1",
      source: "descent-spike",
      commit: null,
      generatorVersion: null,
      scene: { id: "cockpit-descent", version: 0, seed: "7" },
      setting: { quality: "low" },
      harness: {
        repetitions: 1,
        timer: "full",
        gpuTiming: true,
        display: "physical",
        pacing: "every-second-vsync",
      },
      machine: { name: "rtx3080-desktop", gpu: { vendorId: 0x10de, deviceId: 0x2206 } },
    });
    expect(record.harness.vsyncPeriodMs).toBeCloseTo(16.667, 3);
    expect(record.runs).toHaveLength(1);
  });

  it("carries the frames, the pass rows and the passes R05 timed", () => {
    const [run] = foldSpikeResult(fixture()).runs;
    expect(run?.frames.source).toBe("presentation");
    expect(run?.frames.count).toBeGreaterThan(30_000);
    expect(run?.segments.map((s) => s.segment)).toEqual([
      "orbit coast",
      "descent arc",
      "approach and flare",
      "low fast pass",
      "slowdown",
      "vertical descent",
      "hover and touchdown",
    ]);
    expect(run?.rows.map((r) => r.row)).toEqual(["terrain", "atmosphere-frame"]);
    expect(run?.passes.map((p) => p.key)).toContain("primary:terrain");
    expect(run?.memory.gpu?.source).toBe("nvidia-smi");
    expect(run?.streaming).toHaveLength(7);
  });

  it("reads version 9's measured vsync and a hidden run as virtual", () => {
    const file = fixture();
    const run = runOf(file);
    const record = foldSpikeResult({
      ...file,
      version: 9,
      run: {
        ...run,
        shown: false,
        pacing: { framePacing: "display", measuredVsyncMs: { value: 16.68, reason: null } },
      },
    });
    expect(record.harness.vsyncPeriodMs).toBe(16.68);
    expect(record.harness.display).toBe("virtual");
  });

  it("keys the wireframe second view's pass to the instrument", () => {
    const record = foldSpikeResult({
      ...fixture(),
      sizes: {
        secondView: { style: "wireframe", viewports: [{ widthPx: 275, heightPx: 140, fromS: 0 }] },
      },
    });
    expect(record.views.map((v) => `${v.role}:${v.style}`)).toEqual([
      "primary:photorealistic",
      "instrument:wireframe",
    ]);
    expect(record.runs[0]?.passes.map((p) => p.key)).toContain("instrument:view:wireframe");
  });

  it("leaves out a clock sample the sampler missed", () => {
    const file = fixture();
    const gpu = file["gpu"];
    if (typeof gpu !== "object" || gpu === null) throw new Error("no gpu");
    const clocks = {
      value: {
        source: "nvidia-smi",
        graphicsMHz: { value: { samples: [900, -1, 1100, 1000] }, reason: null },
      },
      reason: null,
    };
    const record = foldSpikeResult({ ...file, gpu: { ...gpu, clocks } });
    expect(record.runs[0]?.throttle).toMatchObject({ gpuMinMHz: 900, gpuMedianMHz: 1000 });
  });

  it("folds every results file R05 committed", () => {
    const dir = new URL("../../../docs/measurements/descent-spike/", import.meta.url);
    const folded = readdirSync(dir)
      .filter((name) => name.endsWith(".json"))
      .map((name): unknown => JSON.parse(readFileSync(new URL(name, dir), "utf8")))
      .filter(
        (file) =>
          typeof file === "object" &&
          file !== null &&
          "schema" in file &&
          file.schema === "hyperion.descent-spike.results",
      )
      .map((file) => foldSpikeResult(file).source);
    expect(folded.length).toBeGreaterThanOrEqual(12);
    expect(new Set(folded)).toEqual(new Set(["descent-spike"]));
  });

  it("refuses a file that is not R05's results or of a version it does not read", () => {
    expect(() =>
      foldSpikeResult({ ...fixture(), schema: "hyperion.descent-spike.demand" }),
    ).toThrow(PerfRecordError);
    expect(() => foldSpikeResult({ ...fixture(), version: 5 })).toThrow(/spike:version/);
  });
});

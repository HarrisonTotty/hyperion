import { describe, expect, it, vi } from "vitest";

import type { DescentSpikeReport, SpikeReportPart } from "../preload/api";
import {
  measured,
  type PartialResultsFiles,
  type RunDescription,
  validateResults,
} from "./results";
import {
  mergeReportPart,
  PARTIAL_TRACE_REASON,
  partialsSink,
  runWithTerrain,
  SpikePartials,
} from "./spikePartials";

const RUN: RunDescription = {
  startedAt: new Date("2026-11-03T18:00:00Z"),
  machine: {
    name: "mac",
    cpu: "Apple M5",
    logicalCores: 10,
    memoryBytes: 16 * 1024 ** 3,
    governor: measured("none"),
    loadAverage: [1.2, 1.1, 1],
    gpu: measured({ vendorId: 0x10_6b, deviceId: 1, driverVersion: null, description: null }),
  },
  versions: { app: "0.1.0", electron: "45.1.0", chromium: "156", node: "24", v8: "15" },
  platform: "darwin",
  launchMode: "default",
  setting: "high",
  seed: "1",
  world: "test-planet",
  style: null,
  options: { setting: "high", seed: "1" },
  switches: [],
  shown: false,
  displayHz: null,
  nvidiaBaselineBytes: null,
};

/** Ten frames a second of script over two segments, 10 s each, from `fromS` up to `toS`. */
function report(fromS: number, toS: number, labels = ["terrain"]): DescentSpikeReport {
  const scriptTimesS = Array.from(
    { length: Math.round((toS - fromS) * 10) },
    (_, i) => fromS + i / 10,
  );
  return {
    scriptStartMs: 1000,
    traceWindows: [],
    traceGuardS: 1,
    warmupS: 2,
    segments: [
      { name: "orbit coast", startS: 0, endS: 10 },
      { name: "descent arc", startS: 10, endS: 20 },
    ],
    levels: [{ level: 0, epsilonM: 9000, k: 4.1 }],
    timer: "quantized",
    untimedPasses: 0,
    frames: {
      scriptTimesS,
      rafIntervalsMs: scriptTimesS.map(() => 100),
      ourCodeMs: scriptTimesS.map(() => 4),
      callbackStartsMs: scriptTimesS.map((s) => 1000 + s * 1000),
      missingResolves: scriptTimesS.map(() => 0),
      inFlightResolves: scriptTimesS.map(() => 0),
      passes: labels.map((label) => ({
        label,
        row: label === "terrain" ? ("terrain" as const) : ("other" as const),
        gpuMs: scriptTimesS.map(() => 2),
      })),
    },
    streaming: [],
    uploadBytes: 0,
    latePipelines: [],
    adapterPeakBytes: 0,
    canvas: { widthPx: 1920, heightPx: 1080 },
  };
}

/** The part of the frames from `fromS` up to `toS`, after `fromFrame` frames, complete up to `toS`. */
function part(fromFrame: number, fromS: number, toS: number, labels?: string[]): SpikeReportPart {
  return { fromFrame, throughS: toS, report: report(fromS, toS, labels) };
}

/** Files held by path, which a removal deletes. */
function memoryFiles(): PartialResultsFiles & { readonly written: Map<string, string> } {
  const written = new Map<string, string>();
  return {
    written,
    mkdir: () => Promise.resolve(),
    exists: (path) => Promise.resolve(written.has(path)),
    writeFile: (path, text) => {
      written.set(path, text);
      return Promise.resolve();
    },
    remove: (path) => {
      written.delete(path);
      return Promise.resolve();
    },
  };
}

describe("a spike report's parts (R14's Mac measurements)", () => {
  it("put together continue each series, a label new to a part null before it and one it lacks null in it", () => {
    const merged = mergeReportPart(
      mergeReportPart(null, part(0, 0, 0.2)),
      part(2, 0.2, 0.4, ["atmosphere view"]),
    );
    expect([merged.frames.scriptTimesS, merged.frames.passes]).toEqual([
      [0, 0.1, 0.2, 0.30000000000000004],
      [
        { label: "terrain", row: "terrain", gpuMs: [2, 2, null, null] },
        { label: "atmosphere view", row: "other", gpuMs: [null, null, 2, 2] },
      ],
    ]);
  });

  it("refuse a part that does not begin where the frames so far end", () => {
    expect(() => mergeReportPart(mergeReportPart(null, part(0, 0, 1)), part(9, 1, 2))).toThrow(
      /begins at frame 9, after 10 frames/u,
    );
  });

  it("give the run the terrain variant the renderer drew with", () => {
    expect(
      runWithTerrain(RUN, {
        ...report(0, 1),
        terrain: { vertexPath: "baked-offsets", normals: "mesh" },
      }).options,
    ).toEqual({
      setting: "high",
      seed: "1",
      terrainVertexPath: "baked-offsets",
      terrainNormals: "mesh",
    });
  });
});

describe("a spike run's finished segments (R14's Mac measurements)", () => {
  it("are written each time another segment has ended, under the run's name and -partial, then removed", async () => {
    const files = memoryFiles();
    const partials = new SpikePartials(
      { outDir: "/out", name: "2026-11-03-mac-high", run: RUN },
      files,
    );
    const written = [
      await partials.add(part(0, 0, 8)),
      await partials.add(part(80, 8, 12)),
      await partials.add(part(120, 12, 16)),
    ];
    expect(written).toEqual([null, "/out/2026-11-03-mac-high-partial.json", null]);
    const results: unknown = JSON.parse(
      files.written.get("/out/2026-11-03-mac-high-partial.json") ?? "null",
    );
    expect([validateResults(results), results]).toEqual([
      [],
      expect.objectContaining({
        run: expect.objectContaining({
          trace: { value: null, reason: PARTIAL_TRACE_REASON },
        }),
      }),
    ]);
    expect(await partials.add(part(160, 16, 20))).toBe("/out/2026-11-03-mac-high-partial.json");
    await partials.remove();
    expect([...files.written.keys()]).toEqual([]);
  });

  it("are written in turn through a sink, each failure logged and the next part taken", async () => {
    const log = vi.fn<(line: string) => void>();
    const add = vi
      .fn<SpikePartials["add"]>()
      .mockRejectedValueOnce(new Error("the disk is full"))
      .mockResolvedValue("/out/a-partial.json");
    const remove = vi.fn<SpikePartials["remove"]>(() => Promise.resolve());
    const sink = partialsSink({ add, remove }, log);
    sink.add(part(0, 0, 10));
    sink.add(part(100, 10, 20));
    await sink.close(true);
    expect([log.mock.calls, add.mock.calls.length, remove.mock.calls.length]).toEqual([
      [
        ["descent spike: the finished segments were not written: the disk is full"],
        ["descent spike: the segments finished so far in /out/a-partial.json"],
      ],
      2,
      1,
    ]);
  });

  it("are kept where the run's results were not written", async () => {
    const remove = vi.fn<SpikePartials["remove"]>(() => Promise.resolve());
    const sink = partialsSink({ add: () => Promise.resolve(null), remove }, () => undefined);
    await sink.close(false);
    expect(remove).not.toHaveBeenCalled();
  });
});

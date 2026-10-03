import { describe, expect, it, vi } from "vitest";

import type { SpikeLaunch } from "../preload/api";
import { DEFAULT_SPIKE_SEED } from "../preload/spikeLaunch";
import { measured, type RunDescription, validateResults } from "./results";
import { SpikeSession, type SpikeSessionDeps } from "./spikeSession";
import { smallReport } from "./fixtures/spikeReport";

const LAUNCH: SpikeLaunch = {
  setting: "low",
  seed: DEFAULT_SPIKE_SEED,
  smoke: false,
  out: "/out",
  workers: null,
  vertexPath: null,
  normals: null,
  ridged: "off",
  dawnSafety: "on",
  capture: "/capture",
};

const RUN: RunDescription = {
  startedAt: new Date("2026-10-03T18:00:00Z"),
  machine: {
    name: "devbox",
    cpu: "AMD Ryzen 7 3700X 8-Core Processor",
    logicalCores: 16,
    memoryBytes: 32 * 1024 ** 3,
    governor: measured("schedutil"),
    loadAverage: [0.4, 0.6, 0.8],
    gpu: measured({ vendorId: 0x10de, deviceId: 0x2206, driverVersion: null, description: null }),
  },
  versions: { app: "0.1.0", electron: "44.4.3", chromium: "152", node: "24", v8: "15" },
  platform: "linux",
  launchMode: "vulkan",
  setting: "low",
  seed: DEFAULT_SPIKE_SEED,
  options: { setting: "low", seed: DEFAULT_SPIKE_SEED },
  switches: [],
  shown: false,
  displayHz: null,
  nvidiaBaselineBytes: null,
};

/** A session over fakes, with what it wrote and how it ended. */
function sessionOf(reduce: SpikeSessionDeps["reduce"] = () => Promise.reject(new Error("bad"))) {
  const written = new Map<string, string | Uint8Array>();
  const removed: string[] = [];
  const exits: number[] = [];
  const memoryStart = vi.fn<() => void>();
  const deps: SpikeSessionDeps = {
    launch: LAUNCH,
    describe: () => Promise.resolve(RUN),
    trace: { start: () => Promise.resolve(), stop: (path) => Promise.resolve(path) },
    tracePath: "/profile/spike-trace.json",
    reduce,
    memory: { start: memoryStart, stop: () => Promise.resolve([]) },
    outDir: "/out",
    exit: (code) => {
      exits.push(code);
    },
    log: () => undefined,
    files: {
      mkdir: () => Promise.resolve(),
      writeFile: (path, data) => {
        written.set(path, data);
        return Promise.resolve();
      },
      rm: (path) => {
        removed.push(path);
        return Promise.resolve();
      },
      results: {
        mkdir: () => Promise.resolve(),
        exists: (path) => Promise.resolve(written.has(path)),
        writeFile: (path, text) => {
          written.set(path, text);
          return Promise.resolve();
        },
      },
    },
  };
  return { session: new SpikeSession(deps), memoryStart, written, removed, exits };
}

describe("a spike run's session", () => {
  it("measures between start and stop, removes the trace, and writes a valid results file", async () => {
    const { session, memoryStart, written, removed } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    expect(memoryStart).toHaveBeenCalledOnce();
    ops.rendererMemory(1234);
    expect(session.rendererPrivateBytes()).toBe(1234);
    await ops.stopMeasuring();
    expect(removed).toEqual(["/profile/spike-trace.json"]);
    const paths = await ops.writeResults(smallReport());
    expect(paths.json).toBe("/out/2026-10-03-devbox-low.json");
    const text = written.get(paths.json);
    expect(typeof text).toBe("string");
    const results: unknown = JSON.parse(String(text));
    expect(validateResults(results)).toEqual([]);
    // A trace that could not be reduced is a missing figure with its reason, not a failed run.
    expect(String(text)).toContain("the trace could not be reduced: bad");
  });

  it("measures again after a stop, but not twice at once", async () => {
    const { session } = sessionOf();
    const ops = session.operations();
    await ops.startMeasuring();
    await ops.stopMeasuring();
    await expect(ops.startMeasuring()).resolves.toBeUndefined();
    await expect(ops.startMeasuring()).rejects.toThrow(/already/);
  });

  it("refuses to stop before starting, and to write before measuring", async () => {
    const { session } = sessionOf();
    const ops = session.operations();
    await expect(ops.stopMeasuring()).rejects.toThrow(/not measuring/);
    await expect(ops.writeResults(smallReport())).rejects.toThrow(/before it started/);
  });

  it("writes a capture's two files into --capture's directory", async () => {
    const { session, written } = sessionOf();
    const bin = new Uint8Array([1, 2, 3]);
    await expect(session.operations().writeCapture({ json: "{}", bin })).resolves.toBe("/capture");
    expect(written.get("/capture/capture.json")).toBe("{}");
    expect(written.get("/capture/capture.bin")).toBe(bin);
  });

  it("ends the app with the run's status", () => {
    const { session, exits } = sessionOf();
    expect(session.ended).toBe(false);
    session.operations().end(1, "no patch baked");
    session.operations().end(0, null);
    expect(exits).toEqual([1, 0]);
    expect(session.ended).toBe(true);
  });
});

import type { TraceConfig } from "electron";
import { describe, expect, it } from "vitest";

import {
  CDP_READ_BYTES,
  CDP_TRACE_COMMANDS,
  cdpStartParams,
  CdpTracing,
  type CdpTracingDeps,
} from "./cdpTracing";
import { chunkOf, FAKE_STREAM, FakeDebugger, memoryWriter } from "./fixtures/debugger";
import {
  SPIKE_GPU_CATEGORY,
  SPIKE_PROFILER_CATEGORY,
  SPIKE_TRACE_CATEGORIES,
  SpikeTrace,
  spikeTraceConfig,
} from "./spike";

/** A transport over a fake debugger, its files in memory, its clock at 0 unless a test moves it. */
function tracingOf(clock: () => number = () => 0): {
  readonly fake: FakeDebugger;
  readonly tracing: CdpTracing;
  readonly logs: string[];
  readonly writer: ReturnType<typeof memoryWriter>;
} {
  const fake = new FakeDebugger();
  const logs: string[] = [];
  const writer = memoryWriter();
  const deps: CdpTracingDeps = {
    log: (line) => {
      logs.push(line);
    },
    nowMs: clock,
    openFile: writer.openFile,
  };
  return { fake, tracing: new CdpTracing(fake, deps), logs, writer };
}

/** The CDP record mode `cdpStartParams` gives `config`. */
function modeOf(config: TraceConfig): unknown {
  const traceConfig: unknown = cdpStartParams(config)["traceConfig"];
  return typeof traceConfig === "object" && traceConfig !== null
    ? Reflect.get(traceConfig, "recordMode")
    : undefined;
}

/** Lets the transport's awaited calls run. */
function settle(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

describe("the spike's trace over CDP", () => {
  it("sends only Tracing.start, Tracing.end, IO.read and IO.close, attaching once and detaching once", async () => {
    const { fake, tracing, logs } = tracingOf();
    const trace = new SpikeTrace(tracing);
    await trace.start();
    await trace.cycle("/p/spike-trace-0.pftrace", () => undefined);
    fake.chunks = [chunkOf([7], true)];
    await trace.stop("/p/spike-trace-1.pftrace");
    trace.close();
    expect(CDP_TRACE_COMMANDS).toEqual(["Tracing.start", "Tracing.end", "IO.read", "IO.close"]);
    expect(fake.calls).toEqual([
      "attach 1.3",
      "Tracing.start",
      "Tracing.end",
      "IO.read",
      "IO.read",
      "IO.close",
      "Tracing.start",
      "Tracing.end",
      "IO.read",
      "IO.close",
      "detach",
    ]);
    const commands: ReadonlyArray<string> = CDP_TRACE_COMMANDS;
    expect(fake.sent.every(({ method }) => commands.includes(method))).toBe(true);
    // Its own detach is not taken for the session's loss.
    expect(logs.some((line) => line.includes("detached"))).toBe(false);
  });

  it("starts a timed window with exactly the ruled parameters", async () => {
    const { fake, tracing } = tracingOf();
    await new SpikeTrace(tracing).start();
    expect(fake.sent[0]).toEqual({
      method: "Tracing.start",
      params: {
        traceConfig: {
          recordMode: "recordUntilFull",
          traceBufferSizeInKb: 786_432,
          includedCategories: [...SPIKE_TRACE_CATEGORIES],
          excludedCategories: ["*"],
        },
        transferMode: "ReturnAsStream",
        streamFormat: "proto",
        streamCompression: "none",
        bufferUsageReportingInterval: 1000,
      },
    });
  });

  it("starts a profiled window with the profiled buffer and categories", async () => {
    const { fake, tracing } = tracingOf();
    await new SpikeTrace(tracing, { profiled: true }).start();
    expect(fake.sent[0]?.params).toEqual({
      ...cdpStartParams(spikeTraceConfig()),
      traceConfig: {
        recordMode: "recordUntilFull",
        traceBufferSizeInKb: 1_572_864,
        includedCategories: [
          ...SPIKE_TRACE_CATEGORIES,
          SPIKE_GPU_CATEGORY,
          SPIKE_PROFILER_CATEGORY,
        ],
        excludedCategories: ["*"],
      },
    });
  });

  it("names each of Chromium's recording modes as CDP does", () => {
    expect(modeOf({})).toBe("recordUntilFull");
    expect(modeOf({ recording_mode: "record-continuously" })).toBe("recordContinuously");
    expect(modeOf({ recording_mode: "record-as-much-as-possible" })).toBe("recordAsMuchAsPossible");
    expect(modeOf({ recording_mode: "trace-to-console" })).toBe("echoToConsole");
  });

  it("waits for tracingComplete, then writes the stream's chunks in order until its end", async () => {
    const { fake, tracing, writer } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.holdComplete = true;
    fake.chunks = [chunkOf([1, 2, 3], false), chunkOf([], false), chunkOf([4, 5], true)];
    const stopping = tracing.stop("/p/w.pftrace");
    await settle();
    expect(fake.calls.at(-1)).toBe("Tracing.end");
    expect(writer.files.size).toBe(0);
    fake.complete();
    await expect(stopping).resolves.toEqual({ lostData: false, bufferPercent: null });
    expect(writer.files.get("/p/w.pftrace")).toEqual([1, 2, 3, 4, 5]);
    expect(writer.closed).toEqual(["/p/w.pftrace"]);
    expect(fake.sent.slice(2)).toEqual([
      { method: "IO.read", params: { handle: FAKE_STREAM, size: CDP_READ_BYTES } },
      { method: "IO.read", params: { handle: FAKE_STREAM, size: CDP_READ_BYTES } },
      { method: "IO.read", params: { handle: FAKE_STREAM, size: CDP_READ_BYTES } },
      { method: "IO.close", params: { handle: FAKE_STREAM } },
    ]);
    expect(CDP_READ_BYTES).toBe(8 * 1024 * 1024);
  });

  it("tells a window whose stop says Chromium lost data", async () => {
    const { fake, tracing, logs } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.dataLoss = true;
    await expect(tracing.stop("/p/w.pftrace")).resolves.toMatchObject({ lostData: true });
    expect(logs.at(-1)).toMatch(/, data lost$/);
  });

  it("gives the window's last percentFull as a percentage: 0.095 is 9.5 %", async () => {
    const { fake, tracing } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.bufferUsage(0.05);
    fake.bufferUsage(0.095);
    const { bufferPercent } = await tracing.stop("/p/a.pftrace");
    expect(bufferPercent).toBeCloseTo(9.5, 12);
    // The next window has no reading until Chromium reports one.
    await tracing.start(spikeTraceConfig());
    await expect(tracing.stop("/p/b.pftrace")).resolves.toEqual({
      lostData: false,
      bufferPercent: null,
    });
  });

  it("logs both phases of each stop: until the trace's end, and the stream's read", async () => {
    const times = [0, 1500, 2230];
    const { tracing, logs } = tracingOf(() => times.shift() ?? 0);
    await tracing.start(spikeTraceConfig());
    await tracing.stop("/p/a.pftrace");
    expect(logs).toEqual([
      "descent spike: trace window 1 stopped: complete after 1500 ms, 6 B read in 730 ms",
    ]);
  });

  it("ends the trace, not the run, on a detach it did not ask for", async () => {
    const { fake, tracing, logs } = tracingOf();
    await tracing.start(spikeTraceConfig());
    const before = fake.calls.length;
    fake.lose("Render process gone.");
    const reason = "the trace's debugger session detached: Render process gone.";
    expect(logs).toEqual([`descent spike: ${reason}`]);
    await expect(tracing.stop("/p/a.pftrace")).rejects.toThrow(reason);
    await expect(tracing.start(spikeTraceConfig())).rejects.toThrow(reason);
    tracing.close();
    // No command, and no detach, after the session ended.
    expect(fake.calls.length).toBe(before);
  });

  it("fails a stop whose session detaches while it waits for the trace's end", async () => {
    const { fake, tracing, writer } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.holdComplete = true;
    const stopping = tracing.stop("/p/a.pftrace");
    await settle();
    fake.lose("target closed");
    await expect(stopping).rejects.toThrow("the trace's debugger session detached: target closed");
    expect(writer.files.size).toBe(0);
  });

  it("fails a stop still waiting for the trace's end when it is closed", async () => {
    const { fake, tracing } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.holdComplete = true;
    const stopping = tracing.stop("/p/a.pftrace");
    await settle();
    tracing.close();
    await expect(stopping).rejects.toThrow("the trace's debugger session is closed");
  });

  it("keeps a read's error over the file's when both fail, and logs the file's", async () => {
    const fake = new FakeDebugger();
    const logs: string[] = [];
    const tracing = new CdpTracing(fake, {
      log: (line) => {
        logs.push(line);
      },
      nowMs: () => 0,
      openFile: () =>
        Promise.resolve({
          write: () => Promise.resolve(),
          close: () => Promise.reject(new Error("EIO")),
        }),
    });
    await tracing.start(spikeTraceConfig());
    fake.refuse.set("IO.read", new Error("Invalid stream handle"));
    await expect(tracing.stop("/p/a.pftrace")).rejects.toThrow("Invalid stream handle");
    expect(logs).toContain("descent spike: a trace window's file did not close: EIO");
  });

  it("closes the window's file and the stream when a read fails", async () => {
    const { fake, tracing, writer } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.refuse.set("IO.read", new Error("Invalid stream handle"));
    await expect(tracing.stop("/p/a.pftrace")).rejects.toThrow("Invalid stream handle");
    expect(writer.closed).toEqual(["/p/a.pftrace"]);
    expect(fake.calls.at(-1)).toBe("IO.close");
  });

  it("keeps a whole window whose stream would not close, and logs it", async () => {
    const { fake, tracing, logs, writer } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.refuse.set("IO.close", new Error("Invalid stream handle"));
    await expect(tracing.stop("/p/a.pftrace")).resolves.toMatchObject({ lostData: false });
    expect(writer.files.get("/p/a.pftrace")).toEqual([1, 2, 3, 4, 5, 6]);
    expect(logs).toContain(
      "descent spike: the trace's stream was not closed: Invalid stream handle",
    );
  });

  it("fails a stop whose end returns no stream", async () => {
    const { fake, tracing } = tracingOf();
    await tracing.start(spikeTraceConfig());
    fake.holdComplete = true;
    const stopping = tracing.stop("/p/a.pftrace");
    await settle();
    fake.emit("message", {}, "Tracing.tracingComplete", { dataLossOccurred: false });
    await expect(stopping).rejects.toThrow("Chromium returned no stream for the trace");
  });

  it("still refuses a start while recording, and starts Chromium's recording once", async () => {
    const { fake, tracing } = tracingOf();
    const trace = new SpikeTrace(tracing);
    await trace.start();
    await expect(trace.start()).rejects.toThrow("already recording");
    expect(fake.calls.filter((call) => call === "Tracing.start")).toHaveLength(1);
  });

  it("refuses to start once closed, and never attaches again", async () => {
    const { fake, tracing } = tracingOf();
    await tracing.start(spikeTraceConfig());
    await tracing.stop("/p/a.pftrace");
    tracing.close();
    await expect(tracing.start(spikeTraceConfig())).rejects.toThrow(
      "the trace's debugger session is closed",
    );
    expect(fake.calls.filter((call) => call.startsWith("attach"))).toHaveLength(1);
    expect(fake.listenerCount("message") + fake.listenerCount("detach")).toBe(0);
  });

  it("refuses a start it cannot attach for, and stops listening", async () => {
    const { fake, tracing } = tracingOf();
    fake.failAttach = new Error("Another debugger is already attached to this target");
    await expect(tracing.start(spikeTraceConfig())).rejects.toThrow(
      "the trace's debugger session did not attach: Another debugger is already attached",
    );
    expect(fake.listenerCount("message") + fake.listenerCount("detach")).toBe(0);
    expect(fake.calls).toEqual([]);
  });
});

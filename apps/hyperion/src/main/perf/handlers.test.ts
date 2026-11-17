import { describe, expect, it } from "vitest";

import type { PerfRepetitionResult } from "../../preload/api";
import {
  PERF_CHANNELS,
  PerfCallRefused,
  type PerfOperations,
  readRepetitionResult,
  registerPerfHandlers,
} from "./handlers";

/** The page a call comes from, as the handlers' `isSender` reads it. */
interface FakeEvent {
  readonly own: boolean;
}

/** The handlers over stand-in operations, each call recorded. */
function harness() {
  const handlers = new Map<string, (event: FakeEvent, ...args: unknown[]) => Promise<unknown>>();
  const calls: Array<readonly [string, ...unknown[]]> = [];
  const operations: PerfOperations = {
    config: () => Promise.resolve("config"),
    describe: (description) => {
      calls.push(["describe", description]);
      return Promise.resolve();
    },
    warmUpSample: (sample) => {
      calls.push(["warmUpSample", sample]);
      return Promise.resolve({ kind: "warming", elapsedS: 1 });
    },
    beginRepetition: (n) => {
      calls.push(["beginRepetition", n]);
      return Promise.resolve({ serverUrl: "ws://127.0.0.1:1/ws" });
    },
    endRepetition: (n, result) => {
      calls.push(["endRepetition", n, result]);
      return Promise.resolve();
    },
    finish: (end) => {
      calls.push(["finish", end]);
      return Promise.resolve();
    },
  };
  registerPerfHandlers<FakeEvent>({
    handle: (channel, listener) => handlers.set(channel, listener),
    isSender: (event) => event.own,
    operations,
  });
  const call = (channel: string, ...args: unknown[]): Promise<unknown> => {
    const handler = handlers.get(channel);
    if (handler === undefined) {
      throw new Error(`no handler for ${channel}`);
    }
    return handler({ own: true }, ...args);
  };
  return { handlers, calls, call };
}

const RESULT: PerfRepetitionResult = {
  spanS: 120,
  timeOriginMs: 1_800_000_000_000,
  frames: { intervalsMs: [16.7], callbackStartsMs: [10], callbackMs: [3], scriptTimesS: [0] },
  segments: [{ name: "sweep", startS: 0, endS: 120 }],
  passes: [{ key: "primary:sky", row: "stars", frames: 1, p50Ms: 1, p95Ms: 1, p99Ms: 1 }],
  rows: [{ row: "stars", frames: 1, meanMs: 1, p50Ms: 1, p95Ms: 1, p99Ms: 1 }],
  timedPassesPerFrameMs: 1,
  timer: "full",
  memory: {
    itemPeakBytes: { "star-cubemap": 100 },
    tallyPeakBytes: 100,
    workerFieldBytes: [],
    workerWasmBytes: [],
  },
  releases: [],
  streaming: [],
  latePipelines: 0,
  heightPointUs: null,
  internalScale: [{ min: 1, median: 1 }],
  arrival: null,
};

describe("perf mode's handlers", () => {
  it("handle every channel of the preload's", () => {
    const { handlers } = harness();
    expect([...handlers.keys()].toSorted()).toEqual(Object.values(PERF_CHANNELS).toSorted());
  });

  it("refuse a call from any page but the run window's", async () => {
    const { handlers } = harness();
    const config = handlers.get(PERF_CHANNELS.config);
    await expect(config?.({ own: false })).rejects.toBeInstanceOf(PerfCallRefused);
  });

  it("hand the runner the arguments they read, and refuse one of no shape", async () => {
    const { call, calls } = harness();
    await call(PERF_CHANNELS.endRepetition, 1, RESULT);
    expect(calls).toEqual([["endRepetition", 1, RESULT]]);
    await expect(call(PERF_CHANNELS.endRepetition, 0, RESULT)).rejects.toBeInstanceOf(
      PerfCallRefused,
    );
    await expect(
      call(PERF_CHANNELS.endRepetition, 1, { ...RESULT, timer: "approximate" }),
    ).rejects.toThrow(/timer/);
    await expect(call(PERF_CHANNELS.warmUpSample, { intervalsMs: [-1] })).rejects.toBeInstanceOf(
      PerfCallRefused,
    );
    await expect(
      call(PERF_CHANNELS.finish, { status: "failed", reason: "" }),
    ).rejects.toBeInstanceOf(PerfCallRefused);
    await call(PERF_CHANNELS.finish, { status: "failed", reason: "GRAPHICS FAULT" });
    expect(calls.at(-1)).toEqual(["finish", { status: "failed", reason: "GRAPHICS FAULT" }]);
  });

  it("refuse a pass of no row or a memory item of no row", () => {
    expect(() =>
      readRepetitionResult({ ...RESULT, passes: [{ ...RESULT.passes[0], row: "sky" }] }, ""),
    ).toThrow(/a budget row/);
    expect(() =>
      readRepetitionResult(
        { ...RESULT, memory: { ...RESULT.memory, itemPeakBytes: { "sky-cube": 1 } } },
        "",
      ),
    ).toThrow(/a memory item/);
  });
});

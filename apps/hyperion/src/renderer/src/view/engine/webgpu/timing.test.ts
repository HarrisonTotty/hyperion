import { describe, expect, it, vi } from "vitest";

import type { BufferSpec } from "../memory";
import type { BufferHandle, PassTimes } from "../types";
import { PASSES_PER_FRAME, PassTimer, type TimerHost, TIMING_FRAMES_IN_FLIGHT } from "./timing";

/**
 * An engine's creation path that records its query sets and buffers, and hands out buffers
 * holding `stamps` once mapped; a mapping resolves at once, when the test calls `settle`
 * (`held`), or never (`failing`, which rejects with `failure`'s error).
 */
function timerDevice(
  stamps: ReadonlyArray<bigint>,
  mapping: "at-once" | "held" | "failing" = "at-once",
  failure: () => unknown = () => new Error("the mapping failed"),
): TimerHost & {
  readonly querySets: GPUQuerySetDescriptor[];
  readonly buffers: BufferSpec[];
  /** Resolves every mapping waiting. */
  settle(): void;
} {
  const querySets: GPUQuerySetDescriptor[] = [];
  const buffers: BufferSpec[] = [];
  const gpuBuffers = new Map<BufferHandle, GPUBuffer>();
  const waiting: Array<() => void> = [];
  return {
    querySets,
    buffers,
    settle: () => {
      for (const resolve of waiting.splice(0)) {
        resolve();
      }
    },
    createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet {
      querySets.push(descriptor);
      return {
        label: descriptor.label ?? "",
        type: descriptor.type,
        count: descriptor.count,
        destroy: () => undefined,
      };
    },
    createBuffer(spec: BufferSpec): BufferHandle {
      buffers.push(spec);
      const handle: BufferHandle = { kind: "buffer", name: spec.name, bytes: spec.bytes };
      gpuBuffers.set(handle, {
        label: spec.name,
        size: spec.bytes,
        usage: spec.usage,
        mapState: "unmapped",
        mapAsync: () => {
          if (mapping === "held") {
            return new Promise((resolve) => {
              waiting.push(() => {
                resolve(undefined);
              });
            });
          }
          return mapping === "failing" ? Promise.reject(failure()) : Promise.resolve(undefined);
        },
        getMappedRange: () => new BigUint64Array(stamps).buffer,
        unmap: () => undefined,
        destroy: () => undefined,
      });
      return handle;
    },
    gpuBufferOf(handle: BufferHandle): GPUBuffer {
      const buffer = gpuBuffers.get(handle);
      if (buffer === undefined) {
        throw new Error(`no buffer ${handle.name}`);
      }
      return buffer;
    },
  };
}

/** A mapping's failure as WebGPU's validation gives it. */
function invalidBuffer(): DOMException {
  return new DOMException("the buffer is invalid", "OperationError");
}

/** A mapping cut short by its device's loss, as WebGPU's is. */
function deviceLost(): DOMException {
  return new DOMException("the device was lost", "AbortError");
}

/** An encoder that records the resolve and the copy. */
function recordingEncoder(): Pick<GPUCommandEncoder, "resolveQuerySet" | "copyBufferToBuffer"> & {
  readonly calls: string[];
} {
  const calls: string[] = [];
  return {
    calls,
    resolveQuerySet: (_set: GPUQuerySet, first: number, count: number) => {
      calls.push(`resolve ${first}..${first + count}`);
      return undefined;
    },
    copyBufferToBuffer: () => {
      calls.push("copy");
      return undefined;
    },
  };
}

describe("the pass timer", () => {
  it("allocates two queries a labelled pass, in order", () => {
    const timer = new PassTimer(timerDevice([]), "quantized");
    const compute = timer.writesFor("histogram");
    const cockpit = timer.writesFor("cockpit");
    expect(compute).toMatchObject({ beginningOfPassWriteIndex: 0, endOfPassWriteIndex: 1 });
    expect(cockpit).toMatchObject({ beginningOfPassWriteIndex: 2, endOfPassWriteIndex: 3 });
    expect(timer.pending).toEqual([
      { label: "histogram", bracketed: false, begin: 0, end: 1 },
      { label: "cockpit", bracketed: false, begin: 2, end: 3 },
    ]);
  });

  it("resolves the frame's queries and reports each pass by label, in nanoseconds", async () => {
    const timer = new PassTimer(timerDevice([100n, 350n, 1_000n, 4_000n]), "full");
    const listener = vi.fn<(times: PassTimes) => void>();
    timer.listen(listener);
    timer.writesFor("histogram");
    timer.writesFor("cockpit");
    const encoder = recordingEncoder();
    timer.resolve(encoder)?.();
    expect(encoder.calls).toEqual(["resolve 0..4", "copy"]);
    await vi.waitFor(() => {
      expect(listener).toHaveBeenCalledExactlyOnceWith({
        frame: 1,
        timer: "full",
        passes: [
          { label: "histogram", ns: 250, bracketed: false },
          { label: "cockpit", ns: 3_000, bracketed: false },
        ],
      });
    });
    expect(timer.pending).toEqual([]);
  });

  it("makes no query set and never reports without timestamp-query", () => {
    const device = timerDevice([]);
    const timer = new PassTimer(device, "absent");
    const listener = vi.fn<(times: PassTimes) => void>();
    timer.listen(listener);
    expect(timer.writesFor("histogram")).toBeUndefined();
    expect(timer.resolve(recordingEncoder())).toBeNull();
    expect(device.querySets).toEqual([]);
    expect(listener).not.toHaveBeenCalled();
  });

  it("resolves nothing when no pass was timed", () => {
    const timer = new PassTimer(timerDevice([]), "quantized");
    expect(timer.resolve(recordingEncoder())).toBeNull();
  });
});

describe("the pass timer's capacity", () => {
  it("times 64 passes a frame and warns once of the rest", () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const timer = new PassTimer(timerDevice([]), "quantized");
    for (let pass = 0; pass < PASSES_PER_FRAME + 2; pass += 1) {
      timer.writesFor(`pass ${pass}`);
    }
    expect(timer.pending).toHaveLength(PASSES_PER_FRAME);
    expect(console.warn).toHaveBeenCalledOnce();
  });

  it("starts each frame's queries again at 0", () => {
    const timer = new PassTimer(timerDevice([0n, 1n]), "quantized");
    timer.writesFor("first");
    timer.resolve(recordingEncoder());
    expect(timer.writesFor("second")?.beginningOfPassWriteIndex).toBe(0);
  });

  it("forgets passes rolled back, whose encoding threw before they were submitted", () => {
    const timer = new PassTimer(timerDevice([]), "quantized");
    timer.writesFor("frame");
    const mark = timer.mark();
    timer.writesFor("dispatch that threw");
    timer.rollBack(mark);
    expect(timer.pending.map(({ label }) => label)).toEqual(["frame"]);
    expect(timer.writesFor("next")?.beginningOfPassWriteIndex).toBe(2);
  });

  it("reports nothing once disposed", async () => {
    const timer = new PassTimer(timerDevice([0n, 10n]), "quantized");
    const listener = vi.fn<(times: PassTimes) => void>();
    timer.listen(listener);
    timer.writesFor("pass");
    const read = timer.resolve(recordingEncoder());
    timer.dispose();
    read?.();
    await Promise.resolve();
    await Promise.resolve();
    expect(listener).not.toHaveBeenCalled();
    expect(timer.writesFor("later")).toBeUndefined();
  });
});

describe("the pass timer's buffers", () => {
  it("are made through the engine's creation path and reused frame after frame", async () => {
    const device = timerDevice([0n, 10n]);
    const timer = new PassTimer(device, "quantized");
    const listener = vi.fn<(times: PassTimes) => void>();
    timer.listen(listener);
    for (let frame = 1; frame <= 5; frame += 1) {
      timer.writesFor("cockpit");
      timer.resolve(recordingEncoder())?.();
      // Each frame waits for the one before it to be read, as frames a few apart do.
      // oxlint-disable-next-line no-await-in-loop
      await vi.waitFor(() => {
        expect(listener).toHaveBeenCalledTimes(frame);
      });
    }
    expect(device.buffers.map(({ name }) => name)).toEqual([
      "pass times resolved 1",
      "pass times readback 1",
    ]);
    expect(device.querySets).toHaveLength(1);
  });

  it("drop a frame's times, with one warning, while every pair is in flight", () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const device = timerDevice([0n, 10n], "held");
    const timer = new PassTimer(device, "quantized");
    for (let frame = 0; frame < TIMING_FRAMES_IN_FLIGHT; frame += 1) {
      timer.writesFor("cockpit");
      timer.resolve(recordingEncoder())?.();
    }
    timer.writesFor("cockpit");
    expect(timer.resolve(recordingEncoder())).toBeNull();
    expect(timer.pending).toEqual([]);
    expect(device.buffers).toHaveLength(TIMING_FRAMES_IN_FLIGHT * 2);
    expect(console.warn).toHaveBeenCalledOnce();
  });

  it("number a dropped resolve, so that it reads as a gap (R07.T19)", () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const device = timerDevice([0n, 10n], "held");
    const timer = new PassTimer(device, "quantized");
    for (let frame = 0; frame <= TIMING_FRAMES_IN_FLIGHT; frame += 1) {
      timer.writesFor("cockpit");
      timer.resolve(recordingEncoder())?.();
    }
    expect(timer.frame).toBe(TIMING_FRAMES_IN_FLIGHT + 1);
  });

  it("are reused after a read that failed", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const device = timerDevice([0n, 10n], "failing");
    const timer = new PassTimer(device, "quantized");
    for (let frame = 1; frame <= TIMING_FRAMES_IN_FLIGHT + 2; frame += 1) {
      timer.writesFor("cockpit");
      timer.resolve(recordingEncoder())?.();
      // Each frame waits for the one before it to fail, as frames a few apart do; polled each
      // millisecond, since there are as many frames as pairs.
      // oxlint-disable-next-line no-await-in-loop
      await vi.waitFor(
        () => {
          expect(timer.failedReads).toBe(frame);
        },
        { interval: 1 },
      );
    }
    expect(device.buffers).toHaveLength(2);
  });
});

describe("the pass timer's failed reads", () => {
  it("drop their resolves' times, with one warning naming the error, never an error", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const timer = new PassTimer(timerDevice([0n, 10n], "failing", invalidBuffer), "quantized");
    const listener = vi.fn<(times: PassTimes) => void>();
    timer.listen(listener);
    for (let frame = 0; frame < 3; frame += 1) {
      timer.writesFor("cockpit");
      timer.resolve(recordingEncoder())?.();
    }
    await vi.waitFor(() => {
      expect(timer.failedReads).toBe(3);
    });
    expect(console.warn).toHaveBeenCalledExactlyOnceWith(
      "a resolve's pass times could not be read, and are dropped, as are any later read's that fails: OperationError: the buffer is invalid",
    );
    expect([listener.mock.calls, vi.mocked(console.error).mock.calls]).toEqual([[], []]);
  });

  it("are no failure, and no warning, where the device's loss cut them short", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const device = timerDevice([0n, 10n], "failing", deviceLost);
    const timer = new PassTimer(device, "quantized");
    timer.writesFor("cockpit");
    timer.resolve(recordingEncoder())?.();
    await macrotask();
    // Its pair is back: the next resolve reuses it.
    timer.writesFor("cockpit");
    timer.resolve(recordingEncoder())?.();
    await macrotask();
    expect([timer.failedReads, device.buffers.length, vi.mocked(console.warn).mock.calls]).toEqual([
      0,
      2,
      [],
    ]);
  });

  it("are not made once the device is lost: each resolve is dropped, keeping its number", () => {
    const device = timerDevice([0n, 10n], "held");
    const timer = new PassTimer(device, "quantized");
    timer.writesFor("before");
    expect(timer.resolve(recordingEncoder())).not.toBeNull();
    timer.deviceLost();
    timer.writesFor("after");
    const encoder = recordingEncoder();
    expect([timer.resolve(encoder), encoder.calls, timer.pending, timer.frame]).toEqual([
      null,
      [],
      [],
      2,
    ]);
  });

  it("leave a listener's own failure an error, its times having been read", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const timer = new PassTimer(timerDevice([0n, 10n]), "quantized");
    timer.listen(() => {
      throw new TypeError("the listener broke");
    });
    timer.writesFor("cockpit");
    timer.resolve(recordingEncoder())?.();
    await vi.waitFor(() => {
      expect(console.error).toHaveBeenCalledExactlyOnceWith(
        "a listener of the pass times failed: TypeError: the listener broke",
      );
    });
    expect(timer.failedReads).toBe(0);
  });
});

/** Runs every microtask queued before it: a read's mapping, its report or failure and its end. */
function macrotask(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

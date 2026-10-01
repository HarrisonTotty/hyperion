import { describe, expect, it, vi } from "vitest";

import type { PassTimes } from "../types";
import { PASSES_PER_FRAME, PassTimer, type TimerDevice } from "./timing";

/** A device that records its query sets and hands out buffers holding `stamps` once mapped. */
function timerDevice(stamps: ReadonlyArray<bigint>): TimerDevice & {
  readonly querySets: GPUQuerySetDescriptor[];
} {
  const querySets: GPUQuerySetDescriptor[] = [];
  return {
    querySets,
    createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet {
      querySets.push(descriptor);
      return {
        label: descriptor.label ?? "",
        type: descriptor.type,
        count: descriptor.count,
        destroy: () => undefined,
      };
    },
    createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer {
      return {
        label: descriptor.label ?? "",
        size: descriptor.size,
        usage: descriptor.usage,
        mapState: "unmapped",
        mapAsync: () => Promise.resolve(undefined),
        getMappedRange: () => new BigUint64Array(stamps).buffer,
        unmap: () => undefined,
        destroy: () => undefined,
      };
    },
  };
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

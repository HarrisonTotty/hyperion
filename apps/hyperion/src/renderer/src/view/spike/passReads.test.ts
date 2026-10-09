import { afterEach, describe, expect, it, vi } from "vitest";

import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { MAP_MODE } from "../engine/gpuFlags";
import { PASS_TIMES_STAGING_LABEL, PassReads, trackPassReads } from "./passReads";

/** A fake GPU's device, tracked for `reads`. */
async function trackedOn(reads: PassReads): Promise<GPUDevice> {
  const adapter = await new FakeGpu([
    new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["timestamp-query"] }),
  ]).requestAdapter();
  if (adapter === null) {
    throw new Error("no adapter");
  }
  return trackPassReads(await adapter.requestDevice(), reads);
}

describe("the pass-time reads", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("number a read by the resolve numbered as it begins, in flight until it ends", () => {
    const counter = { n: 3 };
    const reads = new PassReads(() => counter.n);
    expect(reads.began()).toBe(3);
    counter.n = 4;
    reads.began();
    expect(reads.inFlight()).toEqual([3, 4]);
    reads.ended(3);
    expect(reads.inFlight()).toEqual([4]);
  });

  it("settle at once with no read in flight in the range", async () => {
    const reads = new PassReads(() => 7);
    reads.began();
    // Resolve 7 lies outside (0, 6] and outside (7, 9].
    await expect(reads.settled(0, 6, 1000)).resolves.toBe(true);
    await expect(reads.settled(7, 9, 1000)).resolves.toBe(true);
  });

  it("wait for every read in the range to end, and leave no timer behind", async () => {
    vi.useFakeTimers();
    const counter = { n: 1 };
    const reads = new PassReads(() => counter.n);
    reads.began();
    counter.n = 2;
    reads.began();
    const outcome: { settled: boolean | null } = { settled: null };
    void reads.settled(0, 2, 1000).then((settled) => {
      outcome.settled = settled;
      return settled;
    });
    reads.ended(1);
    await vi.advanceTimersByTimeAsync(0);
    expect(outcome.settled).toBeNull();
    reads.ended(2);
    await vi.advanceTimersByTimeAsync(0);
    expect(outcome.settled).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("give up after the bound with a read still in flight", async () => {
    vi.useFakeTimers();
    const reads = new PassReads(() => 5);
    reads.began();
    const outcome: { settled: boolean | null } = { settled: null };
    void reads.settled(4, 5, 1000).then((settled) => {
      outcome.settled = settled;
      return settled;
    });
    await vi.advanceTimersByTimeAsync(999);
    expect(outcome.settled).toBeNull();
    await vi.advanceTimersByTimeAsync(1);
    expect(outcome.settled).toBe(false);
    expect(reads.inFlight()).toEqual([5]);
  });
});

describe("a tracked device", () => {
  it("tells when a staging buffer of the timer's begins a read, and when its mapping fails", async () => {
    const counter = { n: 1 };
    const reads = new PassReads(() => counter.n);
    const tracked = await trackedOn(reads);
    const staging = tracked.createBuffer({ label: "pass times readback 1", size: 16, usage: 0 });
    await staging.mapAsync(MAP_MODE.READ);
    // A mapping that succeeds ends with its report, not here.
    expect(reads.inFlight()).toEqual([1]);
    counter.n = 2;
    staging.destroy();
    await expect(staging.mapAsync(MAP_MODE.READ)).rejects.toThrow("destroyed");
    expect(reads.inFlight()).toEqual([1]);
  });

  it("leaves every other buffer's mapping alone", async () => {
    const reads = new PassReads(() => 1);
    const tracked = await trackedOn(reads);
    const other = tracked.createBuffer({ label: "view readback", size: 16, usage: 0 });
    await other.mapAsync(MAP_MODE.READ);
    expect(reads.inFlight()).toEqual([]);
  });

  it("takes the timer's staging buffers by their label, not its resolve buffers", () => {
    expect(
      ["pass times readback 1", "pass times readback 135", "pass times resolved 1"].map((label) =>
        PASS_TIMES_STAGING_LABEL.test(label),
      ),
    ).toEqual([true, true, false]);
  });
});

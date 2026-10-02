import { describe, expect, it, vi } from "vitest";

import { FakeAdapter, type FakeDevice, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import type { GraphicsFault } from "../status";
import { logUncapturedErrors, watchDeviceLoss } from "./deviceLoss";

async function device(): Promise<FakeDevice> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const made = adapter.devices.at(0);
  if (made === undefined) {
    throw new Error("the fake adapter made no device");
  }
  return made;
}

describe("the device-loss watch", () => {
  it("reports a loss with its reason", async () => {
    const gpu = await device();
    const reports: GraphicsFault[] = [];
    watchDeviceLoss(
      gpu,
      () => false,
      (fault) => reports.push(fault),
    );
    gpu.loseDevice("unknown", "the GPU process crashed");
    await vi.waitFor(() => {
      expect(reports).toEqual([
        { kind: "device-lost", reason: "unknown", message: "the GPU process crashed" },
      ]);
    });
  });

  it("does not report the loss that follows the engine's own disposal", async () => {
    const gpu = await device();
    const report = vi.fn<(fault: GraphicsFault) => void>();
    let disposed = false;
    watchDeviceLoss(gpu, () => disposed, report);
    disposed = true;
    gpu.destroy();
    await gpu.lost;
    await Promise.resolve();
    expect(report).not.toHaveBeenCalled();
  });

  it("does not report a loss after the watch has ended", async () => {
    const gpu = await device();
    const report = vi.fn<(fault: GraphicsFault) => void>();
    const stop = watchDeviceLoss(gpu, () => false, report);
    stop();
    gpu.loseDevice();
    await gpu.lost;
    await Promise.resolve();
    expect(report).not.toHaveBeenCalled();
  });
});

describe("the uncaptured-error log", () => {
  it("logs an error's message until it is removed", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const target = new EventTarget();
    const stop = logUncapturedErrors(target);
    target.dispatchEvent(
      Object.assign(new Event("uncapturederror"), { error: { message: "bad bind group" } }),
    );
    stop();
    target.dispatchEvent(Object.assign(new Event("uncapturederror"), { error: { message: "x" } }));
    expect(console.error).toHaveBeenCalledExactlyOnceWith(
      "the GPU device raised an uncaptured error: bad bind group",
    );
  });
});

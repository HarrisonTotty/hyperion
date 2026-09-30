import { describe, expect, it } from "vitest";

import { CRASH_LOOP_WINDOW_MS, crashLoopDecision, type GpuProcessEvent } from "./crashLoop";

function crash(atMs: number, reason = "crashed"): GpuProcessEvent {
  return { kind: "gone", atMs, reason, exitCode: 139 };
}

function status(atMs: number, vulkan: string, webgpu = "enabled"): GpuProcessEvent {
  return { kind: "status", atMs, vulkan, webgpu };
}

describe("the crash-loop policy", () => {
  it("does not relaunch after two crashes within the window", () => {
    expect(crashLoopDecision([crash(0), crash(4_000)], "vulkan")).toBe("none");
  });

  it("relaunches after three crashes within the window", () => {
    expect(crashLoopDecision([crash(0), crash(4_000), crash(8_000)], "vulkan")).toBe(
      "relaunch-safe",
    );
  });

  it("does not relaunch after three crashes spread over more than the window", () => {
    const history = [crash(0), crash(CRASH_LOOP_WINDOW_MS / 2), crash(CRASH_LOOP_WINDOW_MS + 1)];
    expect(crashLoopDecision(history, "vulkan")).toBe("none");
  });

  it("never counts a clean exit", () => {
    const history = [crash(0, "clean-exit"), crash(1_000, "clean-exit"), crash(2_000)];
    expect(crashLoopDecision(history, "vulkan")).toBe("none");
  });

  it("relaunches when Vulkan leaves enabled_on after a status that held it", () => {
    const history = [status(0, "enabled_on"), status(5_000, "enabled_readback")];
    expect(crashLoopDecision(history, "vulkan")).toBe("relaunch-safe");
  });

  it("relaunches when WebGPU leaves enabled after a status that held it", () => {
    const history = [status(0, "enabled_on", "enabled"), status(5_000, "enabled_on", "disabled")];
    expect(crashLoopDecision(history, "vulkan")).toBe("relaunch-safe");
  });

  it("does not relaunch on a first status that never held them", () => {
    const history = [status(0, "disabled_off", "unavailable_off")];
    expect(crashLoopDecision(history, "vulkan")).toBe("none");
  });

  it("never relaunches in safe or default mode", () => {
    const history = [
      crash(0),
      crash(1_000),
      crash(2_000),
      status(3_000, "enabled_on"),
      status(4_000, "disabled_off", "disabled"),
    ];
    expect(crashLoopDecision(history, "safe")).toBe("none");
    expect(crashLoopDecision(history, "default")).toBe("none");
  });

  it("never relaunches on a Windows-like status in default mode", () => {
    const history = [status(0, "disabled_off"), crash(1_000), crash(2_000), crash(3_000)];
    expect(crashLoopDecision(history, "default")).toBe("none");
  });

  it("depends on the events and not on their arrival order", () => {
    const events = [
      status(0, "enabled_on"),
      status(1_000, "enabled_on"),
      status(1_000, "disabled_off"),
      crash(2_000),
      crash(3_000),
    ];
    const reversed = events.toReversed();
    expect(crashLoopDecision(reversed, "vulkan")).toBe(crashLoopDecision(events, "vulkan"));
    const sameInstant = [status(1_000, "disabled_off"), status(1_000, "enabled_on")];
    expect(crashLoopDecision(sameInstant, "vulkan")).toBe("none");
    expect(crashLoopDecision(sameInstant.toReversed(), "vulkan")).toBe("none");
  });
});

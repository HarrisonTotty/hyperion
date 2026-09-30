import { describe, expect, it } from "vitest";

import {
  graphicsArguments,
  graphicsLaunchFromArgv,
  readGpuProcessGoneReport,
} from "./graphicsLaunch";

const RENDERER_ARGV: readonly string[] = ["/opt/hyperion/hyperion", "--type=renderer"];

describe("the graphics launch hand-off", () => {
  it("is safe on Linux only with the switch", () => {
    const argv = [...RENDERER_ARGV, ...graphicsArguments("safe", false)];
    expect(graphicsLaunchFromArgv(argv, "linux").launchMode).toBe("safe");
    expect(graphicsLaunchFromArgv(RENDERER_ARGV, "linux").launchMode).toBe("vulkan");
  });

  it("is default off Linux whatever the switch", () => {
    const argv = [...RENDERER_ARGV, ...graphicsArguments("safe", true)];
    expect(graphicsLaunchFromArgv(argv, "win32").launchMode).toBe("default");
    expect(graphicsLaunchFromArgv(RENDERER_ARGV, "darwin").launchMode).toBe("default");
  });

  it("reports timing only with its switch", () => {
    const timed = [...RENDERER_ARGV, ...graphicsArguments("vulkan", true)];
    expect(graphicsLaunchFromArgv(timed, "linux").gpuTiming).toBe(true);
    expect(graphicsLaunchFromArgv(RENDERER_ARGV, "linux").gpuTiming).toBe(false);
  });

  it("carries nothing for a plain Vulkan launch", () => {
    expect(graphicsArguments("vulkan", false)).toEqual([]);
  });
});

describe("reading a crash report", () => {
  it("reads a well-formed report", () => {
    expect(readGpuProcessGoneReport({ reason: "killed", count: 2 })).toEqual({
      reason: "killed",
      count: 2,
    });
  });

  it("refuses anything else", () => {
    for (const message of [undefined, null, "killed", { reason: "killed" }, { count: 1.5 }]) {
      expect(readGpuProcessGoneReport(message)).toBeUndefined();
    }
  });
});

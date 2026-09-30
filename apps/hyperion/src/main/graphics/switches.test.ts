import { describe, expect, it } from "vitest";

import {
  applyGraphicsSwitches,
  type ChromiumSwitch,
  type GraphicsLaunchMode,
  graphicsSwitches,
  launchModeOf,
  mergeSwitchValue,
} from "./switches";

const PLATFORMS: ReadonlyArray<NodeJS.Platform> = ["linux", "win32", "darwin", "freebsd"];
const MODES: ReadonlyArray<GraphicsLaunchMode> = ["default", "vulkan", "safe"];

const VULKAN_SET: ReadonlyArray<ChromiumSwitch> = [
  { name: "use-angle", value: "vulkan" },
  { name: "enable-features", value: "Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
  { name: "enable-dawn-features", value: "enable_subgroups_intel_gen9" },
];

/** A command line that records what is appended, as Chromium's keeps the last copy of a switch. */
class FakeCommandLine {
  readonly appended: Array<{ readonly name: string; readonly value: string | undefined }> = [];
  readonly #values = new Map<string, string>();

  constructor(initial: Readonly<Record<string, string>>) {
    for (const [name, value] of Object.entries(initial)) {
      this.#values.set(name, value);
    }
  }

  appendSwitch(name: string, value?: string): void {
    this.appended.push({ name, value });
    this.#values.set(name, value ?? "");
  }

  getSwitchValue(name: string): string {
    return this.#values.get(name) ?? "";
  }
}

describe("the graphics switches", () => {
  it("forces the Vulkan path on Linux, exactly and in order", () => {
    expect(graphicsSwitches({ platform: "linux", mode: "vulkan", gpuTiming: false })).toEqual(
      VULKAN_SET,
    );
  });

  it("gives nothing in safe and default modes, with or without timing", () => {
    for (const platform of PLATFORMS) {
      for (const mode of ["safe", "default"] as const) {
        for (const gpuTiming of [false, true]) {
          expect(graphicsSwitches({ platform, mode, gpuTiming })).toEqual([]);
        }
      }
    }
  });

  it("gives nothing off Linux, whatever the mode", () => {
    for (const mode of MODES) {
      expect(graphicsSwitches({ platform: "win32", mode, gpuTiming: true })).toEqual([]);
      expect(graphicsSwitches({ platform: "darwin", mode, gpuTiming: true })).toEqual([]);
    }
  });

  it("never yields the unsafe flag or the adapter override", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES) {
        for (const gpuTiming of [false, true]) {
          const names = graphicsSwitches({ platform, mode, gpuTiming }).map(({ name }) => name);
          expect(names).not.toContain("enable-unsafe-webgpu");
          expect(names).not.toContain("use-webgpu-adapter");
        }
      }
    }
  });

  it("adds only the Dawn quantization toggle for timing", () => {
    expect(graphicsSwitches({ platform: "linux", mode: "vulkan", gpuTiming: true })).toEqual([
      ...VULKAN_SET,
      { name: "disable-dawn-features", value: "timestamp_quantization" },
    ]);
  });
});

describe("the launch mode", () => {
  it("is default off Linux whatever the switch", () => {
    for (const platform of ["win32", "darwin", "freebsd"] as const) {
      expect(launchModeOf(platform, false)).toBe("default");
      expect(launchModeOf(platform, true)).toBe("default");
    }
  });

  it("is safe on Linux only with the switch", () => {
    expect(launchModeOf("linux", true)).toBe("safe");
    expect(launchModeOf("linux", false)).toBe("vulkan");
  });
});

describe("merging a list switch", () => {
  it("keeps the existing values first and drops duplicates", () => {
    expect(mergeSwitchValue("A,B", ["B", "C"])).toBe("A,B,C");
  });

  it("gives the added list when nothing exists", () => {
    expect(mergeSwitchValue("", ["B", "C"])).toBe("B,C");
  });

  it("drops duplicates within the added list", () => {
    expect(mergeSwitchValue("A", ["C", "C", "A"])).toBe("A,C");
  });
});

describe("applying the switches", () => {
  it("appends one merged enable-features over the value already there", () => {
    const commandLine = new FakeCommandLine({ "enable-features": "Foo" });
    applyGraphicsSwitches(commandLine, VULKAN_SET);
    const features = commandLine.appended.filter(({ name }) => name === "enable-features");
    expect(features).toEqual([
      { name: "enable-features", value: "Foo,Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
    ]);
  });

  it("appends a single-valued switch as it is", () => {
    const commandLine = new FakeCommandLine({ "use-angle": "gl" });
    applyGraphicsSwitches(commandLine, VULKAN_SET);
    expect(commandLine.appended).toContainEqual({ name: "use-angle", value: "vulkan" });
  });

  it("appends a bare flag without a value", () => {
    const commandLine = new FakeCommandLine({});
    applyGraphicsSwitches(commandLine, [{ name: "some-flag" }]);
    expect(commandLine.appended).toEqual([{ name: "some-flag", value: undefined }]);
  });
});

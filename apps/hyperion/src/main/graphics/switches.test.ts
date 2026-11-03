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
const MODES: ReadonlyArray<GraphicsLaunchMode> = ["default", "vulkan", "safe", "software"];

const VULKAN_SET: ReadonlyArray<ChromiumSwitch> = [
  { name: "use-angle", value: "vulkan" },
  { name: "enable-features", value: "Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
  { name: "enable-dawn-features", value: "enable_subgroups_intel_gen9" },
];

/** The Dawn toggle that lifts timestamp quantization. */
const TIMING_TOGGLE: ChromiumSwitch = {
  name: "disable-dawn-features",
  value: "timestamp_quantization",
};

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

  it("gives nothing in safe mode, which draws no WebGPU, with or without timing", () => {
    for (const platform of PLATFORMS) {
      for (const gpuTiming of [false, true]) {
        expect(graphicsSwitches({ platform, mode: "safe", gpuTiming })).toEqual([]);
      }
    }
  });

  it("gives nothing in default and software modes without timing", () => {
    for (const platform of PLATFORMS) {
      for (const mode of ["software", "default"] as const) {
        expect(graphicsSwitches({ platform, mode, gpuTiming: false })).toEqual([]);
      }
    }
  });

  it("forces no Vulkan off Linux, whatever the mode", () => {
    for (const platform of ["win32", "darwin", "freebsd"] as const) {
      for (const mode of MODES) {
        for (const gpuTiming of [false, true]) {
          const names = graphicsSwitches({ platform, mode, gpuTiming }).map(({ name }) => name);
          expect(names.filter((name) => name !== "disable-dawn-features")).toEqual([]);
        }
      }
    }
  });

  it("pins each launch's switches on darwin, linux and win32, the timing toggle on every backend (R14's Mac measurements)", () => {
    const launches = (["darwin", "linux", "win32"] as const).flatMap((platform) =>
      // Each platform's own modes (`launchModeOf`): default off Linux; vulkan, software, safe on it.
      (platform === "linux"
        ? (["vulkan", "software", "safe"] as const)
        : (["default"] as const)
      ).flatMap((mode) =>
        [false, true].map((gpuTiming) => [
          `${platform} ${mode}${gpuTiming ? " timed" : ""}`,
          graphicsSwitches({ platform, mode, gpuTiming }),
        ]),
      ),
    );
    expect(Object.fromEntries(launches)).toEqual({
      "darwin default": [],
      "darwin default timed": [TIMING_TOGGLE],
      "linux vulkan": VULKAN_SET,
      "linux vulkan timed": [...VULKAN_SET, TIMING_TOGGLE],
      "linux software": [],
      "linux software timed": [TIMING_TOGGLE],
      "linux safe": [],
      "linux safe timed": [],
      "win32 default": [],
      "win32 default timed": [TIMING_TOGGLE],
    });
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
      TIMING_TOGGLE,
    ]);
  });
});

describe("the launch mode", () => {
  const SWITCHES = [false, true].flatMap((safe) =>
    [false, true].map((software) => ({ safe, software })),
  );

  it("is default off Linux whatever the switches", () => {
    for (const platform of ["win32", "darwin", "freebsd"] as const) {
      for (const switches of SWITCHES) {
        expect(launchModeOf(platform, switches)).toBe("default");
      }
    }
  });

  it("is safe on Linux only with the switch", () => {
    expect(launchModeOf("linux", { safe: true, software: false })).toBe("safe");
    expect(launchModeOf("linux", { safe: false, software: false })).toBe("vulkan");
  });

  it("is software on Linux only with its switch (R01.T16)", () => {
    expect(launchModeOf("linux", { safe: false, software: true })).toBe("software");
  });

  it("is safe with both switches, so that a relaunched crash loop never draws", () => {
    expect(launchModeOf("linux", { safe: true, software: true })).toBe("safe");
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

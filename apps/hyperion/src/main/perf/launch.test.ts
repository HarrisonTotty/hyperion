import { describe, expect, it } from "vitest";

import type { GraphicsLaunchMode } from "../../preload/api";
import { graphicsSwitches } from "../graphics/switches";
import {
  machineSlugOf,
  perfGpuTiming,
  perfSwitches,
  perfSwitchesInForce,
  perfWindowSize,
} from "./launch";

const PLATFORMS: ReadonlyArray<NodeJS.Platform> = ["linux", "darwin", "win32"];
const MODES: ReadonlyArray<GraphicsLaunchMode> = ["default", "vulkan", "safe", "software"];

/** Every switch of a list as Chromium's command line writes it. */
function written(switches: ReadonlyArray<{ readonly name: string; readonly value?: string }>) {
  return switches.map(({ name, value }) => (value === undefined ? name : `${name}=${value}`));
}

describe("perf mode's switches", () => {
  it("are R01's with the timing toggle, and nothing more", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES) {
        expect(perfSwitches({ platform, mode, control: false })).toEqual(
          graphicsSwitches({ platform, mode, gpuTiming: true }),
        );
      }
    }
  });

  it("lift the timestamps' quantization wherever WebGPU draws", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES.filter((each) => each !== "safe")) {
        expect(written(perfSwitches({ platform, mode, control: false }))).toContain(
          "disable-dawn-features=timestamp_quantization",
        );
        expect(perfGpuTiming({ mode, control: false })).toBe(true);
      }
    }
  });

  it("never take the developer features or unsafe WebGPU", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES) {
        for (const control of [false, true]) {
          const names = perfSwitches({ platform, mode, control }).map(({ name }) => name);
          expect(names).not.toContain("enable-webgpu-developer-features");
          expect(names).not.toContain("enable-unsafe-webgpu");
        }
      }
    }
  });

  it("leave the quantization on a control run", () => {
    for (const platform of PLATFORMS) {
      for (const mode of MODES) {
        const control = perfSwitches({ platform, mode, control: true });
        expect(control).toEqual(graphicsSwitches({ platform, mode, gpuTiming: false }));
        expect(written(control).join(" ")).not.toContain("timestamp_quantization");
        expect(perfGpuTiming({ mode, control: true })).toBe(false);
      }
    }
  });
});

describe("the switches in force, for the record", () => {
  it("are the recipe's before the entry script, then perf mode's own", () => {
    const argv = [
      "/repo/apps/hyperion/node_modules/electron/dist/electron",
      "--ozone-platform=x11",
      "/repo/apps/hyperion/out/main/index.js",
      "--perf-run",
      "cockpit-descent",
      "--perf-reps",
      "5",
      "--user-data-dir=/repo/target/perf-run/profile.abc",
    ];
    expect(
      perfSwitchesInForce(argv, false, [
        { name: "use-angle", value: "vulkan" },
        { name: "disable-gpu-vsync" },
      ]),
    ).toEqual(["--ozone-platform=x11", "--use-angle=vulkan", "--disable-gpu-vsync"]);
  });

  it("keep the software mode's SwiftShader switches, unsafe WebGPU among them, less the profile", () => {
    const argv = [
      "electron",
      "--enable-unsafe-webgpu",
      "--use-webgpu-adapter=swiftshader",
      "--ozone-platform=headless",
      "--user-data-dir=/tmp/profile",
      "--hyperion-graphics-software",
      "out/main/index.js",
      "--perf-run",
      "nuclear-disc-sky",
    ];
    expect(perfSwitchesInForce(argv, false, [])).toEqual([
      "--enable-unsafe-webgpu",
      "--use-webgpu-adapter=swiftshader",
      "--ozone-platform=headless",
      "--hyperion-graphics-software",
    ]);
  });

  it("are perf mode's alone on a packaged launch, whose own options have no entry before them", () => {
    expect(
      perfSwitchesInForce(["hyperion", "--perf-run", "cockpit-descent"], true, [
        { name: "use-angle", value: "vulkan" },
      ]),
    ).toEqual(["--use-angle=vulkan"]);
  });
});

describe("a perf run's window", () => {
  it("is the station's 1080p on both settings", () => {
    for (const setting of ["high", "low"] as const) {
      expect(perfWindowSize({ scene: "station-wireframe-descent", setting })).toEqual({
        width: 1920,
        height: 1080,
      });
    }
  });

  it("is every other scene's 1080p at high and 720p at low", () => {
    expect(perfWindowSize({ scene: "cockpit-descent", setting: "high" })).toEqual({
      width: 1920,
      height: 1080,
    });
    expect(perfWindowSize({ scene: "cockpit-descent", setting: "low" })).toEqual({
      width: 1280,
      height: 720,
    });
  });
});

describe("the machine's slug", () => {
  it("names the README's machines by their GPU or, for the Mac, its CPU", () => {
    expect(
      machineSlugOf({ vendorId: 0x10de, deviceId: 0x2206 }, "linux", "AMD Ryzen 7 3700X"),
    ).toBe("rtx3080");
    expect(machineSlugOf({ vendorId: 0x8086, deviceId: 0x3ea0 }, "linux", "i7-8665U")).toBe(
      "uhd620",
    );
    expect(machineSlugOf({ vendorId: 0x106b, deviceId: 0 }, "darwin", "Apple M5")).toBe("m5");
  });

  it("names SwiftShader, and any other GPU by its IDs, never by the host", () => {
    expect(machineSlugOf({ vendorId: 0x1ae0, deviceId: 0xc0de }, "linux", "AMD EPYC 7571")).toBe(
      "swiftshader",
    );
    expect(machineSlugOf({ vendorId: 0x1002, deviceId: 0x73bf }, "linux", "x")).toBe(
      "gpu-1002-73bf",
    );
    expect(machineSlugOf(null, "win32", "x")).toBe("unknown-win32");
    // The software mode's SwiftShader, whose IDs Chromium gave as 0 in a pod's run.
    expect(machineSlugOf({ vendorId: 0, deviceId: 0 }, "linux", "AMD EPYC 7571", true)).toBe(
      "swiftshader",
    );
  });
});

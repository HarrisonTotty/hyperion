import { describe, expect, it } from "vitest";

import type { GraphicsLaunchMode } from "../../preload/api";
import { graphicsSwitches } from "../graphics/switches";
import { perfGpuTiming, perfSwitches } from "./launch";

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

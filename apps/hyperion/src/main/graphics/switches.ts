/**
 * The Chromium switches that give the client a hardware WebGPU adapter on Linux.
 *
 * @remarks
 * The switches are data: {@link graphicsSwitches} says which, and {@link applyGraphicsSwitches}
 * puts them on the command line before `ready`, merged into any value already there. Researched
 * 2026-09-29 by probe on Electron 44.4.3 on the UHD 620 (Mesa 26.2.3 ANV, Xorg): an appended
 * `enable-features` or `enable-dawn-features` replaces any value already on the command line, last
 * writer wins, so a list switch is read, merged and appended once (R01 Design note 2). The unsafe
 * WebGPU flag and the adapter override are the smoke harness's alone and never appear here.
 */

import type { CommandLine } from "electron";

import type { GraphicsLaunchMode } from "../../preload/api";

export type { GraphicsLaunchMode } from "../../preload/api";
export { GPU_TIMING_SWITCH, launchModeOf, SAFE_MODE_SWITCH } from "../../preload/graphicsLaunch";

/** One Chromium switch; `value` absent for a bare flag. */
export interface ChromiumSwitch {
  readonly name: string;
  readonly value?: string;
}

/** What decides a launch's switches. */
export interface GraphicsLaunchOptions {
  readonly platform: NodeJS.Platform;
  readonly mode: GraphicsLaunchMode;
  /** Measurement only: lifts Dawn's timestamp quantization (R01 Design note 4). */
  readonly gpuTiming: boolean;
}

/**
 * The switches whose value is a comma-separated list, which Chromium reads from the last copy on
 * the command line only, so that ours are merged into any value already there.
 */
const LIST_SWITCHES: ReadonlySet<string> = new Set([
  "enable-features",
  "disable-features",
  "enable-dawn-features",
  "disable-dawn-features",
]);

/**
 * The forced Vulkan path: ANGLE on Vulkan, Chromium's Vulkan features, and the Dawn toggle that
 * exposes subgroups on Intel Gen9 (R01 Design note 2).
 */
const VULKAN_SWITCHES: ReadonlyArray<ChromiumSwitch> = [
  { name: "use-angle", value: "vulkan" },
  { name: "enable-features", value: "Vulkan,VulkanFromANGLE,DefaultANGLEVulkan" },
  { name: "enable-dawn-features", value: "enable_subgroups_intel_gen9" },
];

/** The one Dawn toggle that lifts timestamp quantization and adds no feature or adapter. */
const GPU_TIMING_DAWN_SWITCH: ChromiumSwitch = {
  name: "disable-dawn-features",
  value: "timestamp_quantization",
};

/**
 * The switches for a launch.
 *
 * @returns The Vulkan set, plus the timing toggle when `gpuTiming` is on, in `vulkan` mode on
 * Linux; nothing in `safe` mode (no Vulkan, no Dawn toggle and no timing, since nothing draws with
 * WebGPU there) and nothing in `default` mode, or on any other platform, which needs none.
 */
export function graphicsSwitches(options: GraphicsLaunchOptions): ReadonlyArray<ChromiumSwitch> {
  const forced = options.platform === "linux" && options.mode === "vulkan";
  if (!forced) {
    return [];
  }
  return options.gpuTiming ? [...VULKAN_SWITCHES, GPU_TIMING_DAWN_SWITCH] : VULKAN_SWITCHES;
}

/**
 * The union of a comma-separated list and more items, existing values first, without duplicates.
 *
 * @param existing - The switch's value as the command line holds it; empty when it holds none.
 */
export function mergeSwitchValue(existing: string, added: ReadonlyArray<string>): string {
  const items = [...existing.split(","), ...added].map((item) => item.trim());
  return [...new Set(items.filter((item) => item.length > 0))].join(",");
}

/**
 * Puts `switches` on the command line, merging each list switch into the value already there.
 *
 * @remarks
 * Call before `app` is ready: Chromium reads its switches when the GPU process starts.
 */
export function applyGraphicsSwitches(
  commandLine: Pick<CommandLine, "appendSwitch" | "getSwitchValue">,
  switches: ReadonlyArray<ChromiumSwitch>,
): void {
  for (const { name, value } of switches) {
    if (value === undefined) {
      commandLine.appendSwitch(name);
    } else if (LIST_SWITCHES.has(name)) {
      commandLine.appendSwitch(
        name,
        mergeSwitchValue(commandLine.getSwitchValue(name), value.split(",")),
      );
    } else {
      commandLine.appendSwitch(name, value);
    }
  }
}

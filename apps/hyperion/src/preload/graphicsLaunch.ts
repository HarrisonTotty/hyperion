/**
 * How this launch runs the GPU, shared by the main process, which decides it, and the preload,
 * which reports it to the renderer.
 *
 * @remarks
 * A sandboxed preload cannot read the main process's command line, so the main process passes the
 * two graphics switches on in the window's `additionalArguments`, as it passes the server URL, and
 * the preload reads them back out of its own `argv`. Like `serverUrl.ts`, this module imports
 * nothing at run time, so it is safe in either bundle.
 */

import type { GraphicsLaunchMode } from "./api";

/**
 * The switch that makes a launch the declared safe mode: no Vulkan, no WebGPU, the DOM and Canvas
 * 2D consoles only.
 *
 * @remarks
 * Written without its leading dashes, as `app.commandLine.hasSwitch` takes it. The main process
 * adds it itself when it relaunches out of a GPU-process crash loop, so a launch is in safe mode at
 * most once (R01 Design notes 5 and 6).
 */
export const SAFE_MODE_SWITCH = "hyperion-graphics-safe";

/**
 * The measurement-only switch that lifts Dawn's 65,536 ns timestamp quantization.
 *
 * @remarks
 * Written without its leading dashes. Only the performance runs pass it; shipping launches keep the
 * quantization (R01 Design note 4).
 */
export const GPU_TIMING_SWITCH = "hyperion-gpu-timing";

/**
 * The launch's mode.
 *
 * @param platform - `process.platform`.
 * @param safeSwitch - Whether {@link SAFE_MODE_SWITCH} is on the command line.
 * @returns `default` off Linux whatever the switch, since no other platform takes switches;
 * otherwise `safe` with the switch and `vulkan` without it.
 */
export function launchModeOf(platform: NodeJS.Platform, safeSwitch: boolean): GraphicsLaunchMode {
  if (platform !== "linux") {
    return "default";
  }
  return safeSwitch ? "safe" : "vulkan";
}

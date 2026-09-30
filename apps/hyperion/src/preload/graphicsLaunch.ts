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

import type { GpuProcessGoneReport, GraphicsLaunchMode } from "./api";

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

/** The one IPC channel that carries GPU-process crashes from the main process to the preload. */
export const GPU_PROCESS_GONE_CHANNEL = "hyperion:gpu-process-gone";

/**
 * The renderer arguments that carry the launch's graphics set-up, for
 * `webPreferences.additionalArguments`.
 */
export function graphicsArguments(mode: GraphicsLaunchMode, gpuTiming: boolean): string[] {
  const args: string[] = [];
  if (mode === "safe") {
    args.push(`--${SAFE_MODE_SWITCH}`);
  }
  if (gpuTiming) {
    args.push(`--${GPU_TIMING_SWITCH}`);
  }
  return args;
}

/** The launch's graphics set-up, as the preload reads it back. */
export interface GraphicsLaunch {
  readonly launchMode: GraphicsLaunchMode;
  /** Whether timestamps are unquantized: only ever true in `vulkan` mode. */
  readonly gpuTiming: boolean;
}

/**
 * The graphics set-up carried by `argv`, the renderer's arguments.
 *
 * @param platform - `process.platform`: the mode is `default` off Linux whatever the arguments.
 */
export function graphicsLaunchFromArgv(
  argv: readonly string[],
  platform: NodeJS.Platform,
): GraphicsLaunch {
  const launchMode = launchModeOf(platform, argv.includes(`--${SAFE_MODE_SWITCH}`));
  // Only the forced path carries the timing toggle, so no other mode has full timestamps.
  const gpuTiming = launchMode === "vulkan" && argv.includes(`--${GPU_TIMING_SWITCH}`);
  return { launchMode, gpuTiming };
}

/**
 * A crash report received over {@link GPU_PROCESS_GONE_CHANNEL}, or `undefined` when the message
 * is not one.
 *
 * @param message - The IPC message's payload, untrusted until narrowed.
 */
export function readGpuProcessGoneReport(message: unknown): GpuProcessGoneReport | undefined {
  if (typeof message !== "object" || message === null) {
    return undefined;
  }
  const reason: unknown = Reflect.get(message, "reason");
  const count: unknown = Reflect.get(message, "count");
  if (typeof reason !== "string" || typeof count !== "number" || !Number.isInteger(count)) {
    return undefined;
  }
  return { reason, count };
}

/** The part of `ipcRenderer` a crash subscription uses. */
export interface GpuProcessGoneSource {
  on(channel: string, listener: (event: unknown, message: unknown) => void): unknown;
  removeListener(channel: string, listener: (event: unknown, message: unknown) => void): unknown;
}

/**
 * Registers `listener` for the crash reports arriving on {@link GPU_PROCESS_GONE_CHANNEL}, passing on
 * only well-formed ones.
 *
 * @param source - `ipcRenderer`.
 * @returns The listener's removal.
 */
export function subscribeGpuProcessGone(
  source: GpuProcessGoneSource,
  listener: (report: GpuProcessGoneReport) => void,
): () => void {
  const handler = (_event: unknown, message: unknown): void => {
    const report = readGpuProcessGoneReport(message);
    if (report !== undefined) {
      listener(report);
    }
  };
  source.on(GPU_PROCESS_GONE_CHANNEL, handler);
  return () => {
    source.removeListener(GPU_PROCESS_GONE_CHANNEL, handler);
  };
}

/**
 * When a run of GPU-process crashes becomes a relaunch into the declared safe mode.
 *
 * @remarks
 * Researched 2026-09-29 by probe (SIGKILL of the GPU process every 4 s on the UHD 620): each crash
 * fires `child-process-gone` with `type: "GPU"`, then `gpu-info-update`; after three, the feature
 * status's `vulkan` leaves `enabled_on` and `requestAdapter()` returns null; after six, GPU
 * compositing goes to software. The policy watches both the count and the feature status, so it
 * holds if Chromium's own thresholds differ from what the probe saw (R01 Design note 6). Pure: the
 * monitor feeds it events and acts on its answer.
 */

import type { GraphicsLaunchMode } from "../../preload/api";

/** One thing the main process saw of the GPU process, stamped with its time. */
export type GpuProcessEvent =
  | {
      readonly kind: "gone";
      /** Milliseconds on a monotonic clock. */
      readonly atMs: number;
      /** Electron's `Details.reason`, such as `crashed`, `killed` or `clean-exit`. */
      readonly reason: string;
      readonly exitCode: number;
    }
  | {
      readonly kind: "status";
      /** Milliseconds on a monotonic clock. */
      readonly atMs: number;
      /** The feature status's `vulkan` entry, empty when it was absent. */
      readonly vulkan: string;
      /** The feature status's `webgpu` entry, empty when it was absent. */
      readonly webgpu: string;
    };

/** What the monitor does next. */
export type CrashLoopDecision = "none" | "relaunch-safe";

/** GPU-process exits within {@link CRASH_LOOP_WINDOW_MS} that make a crash loop. */
export const CRASH_LOOP_COUNT = 3;

/** The window in which {@link CRASH_LOOP_COUNT} exits make a crash loop: five minutes. */
export const CRASH_LOOP_WINDOW_MS = 300_000;

/** The feature-status values the probe read while Chromium held the forced path. */
const VULKAN_HELD = "enabled_on";
const WEBGPU_HELD = "enabled";

function crashTimesMs(history: ReadonlyArray<GpuProcessEvent>): number[] {
  const times: number[] = [];
  for (const event of history) {
    if (event.kind === "gone" && event.reason !== "clean-exit") {
      times.push(event.atMs);
    }
  }
  return times.toSorted((a, b) => a - b);
}

function crashesMakeALoop(history: ReadonlyArray<GpuProcessEvent>): boolean {
  const times = crashTimesMs(history);
  for (let first = 0; first + CRASH_LOOP_COUNT - 1 < times.length; first += 1) {
    const start = times[first];
    const end = times[first + CRASH_LOOP_COUNT - 1];
    if (start !== undefined && end !== undefined && end - start <= CRASH_LOOP_WINDOW_MS) {
      return true;
    }
  }
  return false;
}

/**
 * Whether a status event shows a value no longer held after an earlier status event held it.
 *
 * @remarks
 * "Earlier" is strictly earlier in time, so that two events stamped alike decide nothing between
 * them and the answer does not depend on the order in which they arrived.
 */
function statusDropped(
  history: ReadonlyArray<GpuProcessEvent>,
  held: (event: GpuProcessEvent & { readonly kind: "status" }) => boolean,
): boolean {
  let firstHeldAtMs: number | undefined;
  for (const event of history) {
    if (event.kind === "status" && held(event)) {
      firstHeldAtMs = Math.min(firstHeldAtMs ?? event.atMs, event.atMs);
    }
  }
  if (firstHeldAtMs === undefined) {
    return false;
  }
  const since = firstHeldAtMs;
  return history.some((event) => event.kind === "status" && !held(event) && event.atMs > since);
}

/**
 * Whether the history is a crash loop that calls for the one relaunch into safe mode.
 *
 * @returns `relaunch-safe` in `vulkan` mode when {@link CRASH_LOOP_COUNT} exits other than
 * `clean-exit` fall within {@link CRASH_LOOP_WINDOW_MS}, or when the feature status shows `vulkan`
 * leaving `enabled_on` or `webgpu` leaving `enabled` after an earlier status held it; `none`
 * otherwise. Always `none` in `default` and `safe` modes: safe mode never relaunches, which makes
 * the relaunch happen once, and every platform but Linux leaves a crash loop to Chromium's own
 * fallback (R01 Design note 6).
 */
export function crashLoopDecision(
  history: ReadonlyArray<GpuProcessEvent>,
  mode: GraphicsLaunchMode,
): CrashLoopDecision {
  if (mode !== "vulkan") {
    return "none";
  }
  const loop =
    crashesMakeALoop(history) ||
    statusDropped(history, (event) => event.vulkan === VULKAN_HELD) ||
    statusDropped(history, (event) => event.webgpu === WEBGPU_HELD);
  return loop ? "relaunch-safe" : "none";
}

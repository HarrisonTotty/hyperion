/**
 * Watches the GPU process, reports its crashes to every window and relaunches once into the
 * declared safe mode when they become a loop, or, in a views check, ends the launch.
 *
 * @remarks
 * The probe of 2026-09-29 saw each crash fire `child-process-gone` with `type: "GPU"`, then
 * `gpu-info-update`, and nothing reach `render-process-gone`; the feature status is read only after
 * `gpu-info-update`, since at `ready` it is stale (R01 Design notes 2 and 6).
 *
 * A views check never relaunches (plan R08, R08.T10.g): its script takes this first process's exit
 * for the run's and then kills its process group, the relaunch with it, so a relaunch's exit 0
 * would pass a run that wrote no results. It ends the launch with exit 1 instead, and a line naming
 * the crashes counted, and the feature status where it dropped short of the loop's count.
 */

import type { GpuProcessGoneReport, GraphicsLaunchMode } from "../../preload/api";
import { GPU_PROCESS_GONE_CHANNEL } from "../../preload/graphicsLaunch";
import { CRASH_LOOP_COUNT, crashLoopDecision, type GpuProcessEvent } from "./crashLoop";
import { readFeatureStatus } from "./featureStatus";
import { SAFE_MODE_SWITCH } from "./switches";

/** The part of Electron's `child-process-gone` details the monitor reads. */
export interface ChildProcessGoneDetails {
  readonly type: string;
  readonly reason: string;
  readonly exitCode: number;
}

type ChildProcessGoneListener = (event: unknown, details: ChildProcessGoneDetails) => void;

/** The part of Electron's `app` the monitor uses. */
export interface MonitoredApp {
  on(event: "child-process-gone", listener: ChildProcessGoneListener): unknown;
  on(event: "gpu-info-update", listener: () => void): unknown;
  removeListener(event: "child-process-gone", listener: ChildProcessGoneListener): unknown;
  removeListener(event: "gpu-info-update", listener: () => void): unknown;
  getGPUFeatureStatus(): unknown;
  relaunch(options: { args: string[] }): void;
  exit(exitCode: number): void;
}

/** The part of a `BrowserWindow` the monitor sends to. */
export interface MonitoredWindow {
  readonly webContents: {
    send(channel: string, report: GpuProcessGoneReport): void;
  };
}

/** What the monitor watches and acts through. */
export interface GpuProcessMonitorOptions {
  readonly app: MonitoredApp;
  /** Every window open now, each of which is told of a crash. */
  readonly windows: () => ReadonlyArray<MonitoredWindow>;
  readonly mode: GraphicsLaunchMode;
  /** `process.argv.slice(1)`: Electron supplies the executable itself. */
  readonly args: readonly string[];
  /** A monotonic clock, in milliseconds. */
  readonly nowMs: () => number;
  /**
   * A views check's log, which a crash loop's line is written to as the launch ends with exit 1;
   * `null` for every other launch, which relaunches into safe mode on a loop instead.
   */
  readonly viewsCheckLog: ((line: string) => void) | null;
}

/** Keeps the GPU process's history for this launch and acts on the crash-loop policy. */
export class GpuProcessMonitor {
  readonly #options: GpuProcessMonitorOptions;
  readonly #history: GpuProcessEvent[] = [];
  #crashes = 0;
  /** Whether a loop has ended this launch, relaunched or not. */
  #ended = false;

  readonly #onChildProcessGone = (_event: unknown, details: ChildProcessGoneDetails): void => {
    if (details.type !== "GPU") {
      return;
    }
    this.#record({
      kind: "gone",
      atMs: this.#options.nowMs(),
      reason: details.reason,
      exitCode: details.exitCode,
    });
    if (details.reason !== "clean-exit") {
      this.#crashes += 1;
      const report: GpuProcessGoneReport = { reason: details.reason, count: this.#crashes };
      for (const window of this.#options.windows()) {
        window.webContents.send(GPU_PROCESS_GONE_CHANNEL, report);
      }
    }
    this.#decide();
  };

  readonly #onGpuInfoUpdate = (): void => {
    const { vulkan, webgpu } = readFeatureStatus(this.#options.app.getGPUFeatureStatus());
    this.#record({
      kind: "status",
      atMs: this.#options.nowMs(),
      vulkan: vulkan ?? "",
      webgpu: webgpu ?? "",
    });
    this.#decide();
  };

  /** Starts watching; {@link GpuProcessMonitor.dispose} stops. */
  constructor(options: GpuProcessMonitorOptions) {
    this.#options = options;
    options.app.on("child-process-gone", this.#onChildProcessGone);
    options.app.on("gpu-info-update", this.#onGpuInfoUpdate);
  }

  /** The events seen so far, oldest first. */
  get history(): ReadonlyArray<GpuProcessEvent> {
    return this.#history;
  }

  /** Stops watching. */
  dispose(): void {
    this.#options.app.removeListener("child-process-gone", this.#onChildProcessGone);
    this.#options.app.removeListener("gpu-info-update", this.#onGpuInfoUpdate);
  }

  #record(event: GpuProcessEvent): void {
    this.#history.push(event);
  }

  #decide(): void {
    if (this.#ended) {
      return;
    }
    switch (crashLoopDecision(this.#history, this.#options.mode)) {
      case "none":
        return;
      case "relaunch-safe": {
        this.#ended = true;
        const log = this.#options.viewsCheckLog;
        if (log !== null) {
          // Short of the loop's count of crashes, the feature status dropping made the loop.
          log(
            this.#crashes >= CRASH_LOOP_COUNT
              ? `views check: FAIL the GPU process crashed ${String(this.#crashes)} times`
              : `views check: FAIL the GPU's feature status dropped Vulkan or WebGPU (the GPU process crashed ${String(this.#crashes)} times)`,
          );
          this.#options.app.exit(1);
          return;
        }
        this.#options.app.relaunch({ args: [...this.#options.args, `--${SAFE_MODE_SWITCH}`] });
        this.#options.app.exit(0);
        return;
      }
    }
  }
}

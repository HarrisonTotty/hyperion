/**
 * Watches the GPU process, reports its crashes to every window and relaunches once into the
 * declared safe mode when they become a loop.
 *
 * @remarks
 * The probe of 2026-09-29 saw each crash fire `child-process-gone` with `type: "GPU"`, then
 * `gpu-info-update`, and nothing reach `render-process-gone`; the feature status is read only after
 * `gpu-info-update`, since at `ready` it is stale (R01 Design notes 2 and 6).
 */

import type { GpuProcessGoneReport, GraphicsLaunchMode } from "../../preload/api";
import { GPU_PROCESS_GONE_CHANNEL } from "../../preload/graphicsLaunch";
import { crashLoopDecision, type GpuProcessEvent } from "./crashLoop";
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
}

/** Keeps the GPU process's history for this launch and acts on the crash-loop policy. */
export class GpuProcessMonitor {
  readonly #options: GpuProcessMonitorOptions;
  readonly #history: GpuProcessEvent[] = [];
  #crashes = 0;
  #relaunched = false;

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
    if (this.#relaunched) {
      return;
    }
    switch (crashLoopDecision(this.#history, this.#options.mode)) {
      case "none":
        return;
      case "relaunch-safe":
        this.#relaunched = true;
        this.#options.app.relaunch({ args: [...this.#options.args, `--${SAFE_MODE_SWITCH}`] });
        this.#options.app.exit(0);
        return;
    }
  }
}

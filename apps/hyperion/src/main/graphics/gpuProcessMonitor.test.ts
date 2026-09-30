import { EventEmitter } from "node:events";

import { describe, expect, it } from "vitest";

import type { GpuProcessGoneReport, GraphicsLaunchMode } from "../../preload/api";
import { GPU_PROCESS_GONE_CHANNEL } from "../../preload/graphicsLaunch";
import {
  type ChildProcessGoneDetails,
  GpuProcessMonitor,
  type MonitoredApp,
  type MonitoredWindow,
} from "./gpuProcessMonitor";

/** An `app` that the test drives: it emits the two events and records relaunches and exits. */
class FakeApp extends EventEmitter implements MonitoredApp {
  featureStatus: unknown = { vulkan: "enabled_on", webgpu: "enabled" };
  readonly relaunches: Array<readonly string[]> = [];
  readonly exits: number[] = [];

  getGPUFeatureStatus(): unknown {
    return this.featureStatus;
  }

  relaunch(options: { args: string[] }): void {
    this.relaunches.push(options.args);
  }

  exit(exitCode: number): void {
    this.exits.push(exitCode);
  }

  gone(type: string, reason = "killed"): void {
    const details: ChildProcessGoneDetails = { type, reason, exitCode: 9 };
    this.emit("child-process-gone", {}, details);
  }

  infoUpdate(): void {
    this.emit("gpu-info-update");
  }

  get watchers(): number {
    return this.listenerCount("child-process-gone") + this.listenerCount("gpu-info-update");
  }
}

class FakeWindow implements MonitoredWindow {
  readonly sent: Array<{ readonly channel: string; readonly report: GpuProcessGoneReport }> = [];
  readonly webContents = {
    send: (channel: string, report: GpuProcessGoneReport): void => {
      this.sent.push({ channel, report });
    },
  };
}

const ARGS: readonly string[] = ["/opt/hyperion/resources/app", "--port", "9000"];

function monitor(
  mode: GraphicsLaunchMode,
  windows: ReadonlyArray<FakeWindow> = [],
): { app: FakeApp; monitor: GpuProcessMonitor; tick: (ms: number) => void } {
  const app = new FakeApp();
  let nowMs = 0;
  const watched = new GpuProcessMonitor({
    app,
    windows: () => windows,
    mode,
    args: ARGS,
    nowMs: () => nowMs,
  });
  return {
    app,
    monitor: watched,
    tick: (ms) => {
      nowMs += ms;
    },
  };
}

describe("the GPU-process monitor", () => {
  it("sends each GPU crash to every window on the one channel, with its count", () => {
    const windows = [new FakeWindow(), new FakeWindow()];
    const { app } = monitor("vulkan", windows);
    app.gone("GPU", "crashed");
    app.gone("GPU", "killed");
    for (const window of windows) {
      expect(window.sent).toEqual([
        { channel: GPU_PROCESS_GONE_CHANNEL, report: { reason: "crashed", count: 1 } },
        { channel: GPU_PROCESS_GONE_CHANNEL, report: { reason: "killed", count: 2 } },
      ]);
    }
  });

  it("ignores every process but the GPU's", () => {
    const window = new FakeWindow();
    const { app, monitor: watched } = monitor("vulkan", [window]);
    app.gone("Utility");
    app.gone("Utility");
    app.gone("Utility");
    expect(window.sent).toEqual([]);
    expect(watched.history).toEqual([]);
    expect(app.relaunches).toEqual([]);
  });

  it("relaunches once into safe mode after three crashes, with no executable path", () => {
    const { app, tick } = monitor("vulkan");
    for (let crash = 0; crash < 4; crash += 1) {
      app.gone("GPU");
      app.infoUpdate();
      tick(4_000);
    }
    expect(app.relaunches).toEqual([[...ARGS, "--hyperion-graphics-safe"]]);
    expect(app.exits).toEqual([0]);
  });

  it("relaunches when the feature status drops Vulkan", () => {
    const { app, tick } = monitor("vulkan");
    app.infoUpdate();
    tick(1_000);
    app.featureStatus = { vulkan: "disabled_off", webgpu: "enabled" };
    app.infoUpdate();
    expect(app.relaunches).toHaveLength(1);
  });

  it("never relaunches in safe mode", () => {
    const window = new FakeWindow();
    const { app, tick } = monitor("safe", [window]);
    for (let crash = 0; crash < 4; crash += 1) {
      app.gone("GPU");
      tick(1_000);
    }
    expect(app.relaunches).toEqual([]);
    expect(window.sent).toHaveLength(4);
  });

  it("stops listening once disposed", () => {
    const { app, monitor: watched } = monitor("vulkan");
    watched.dispose();
    expect(app.watchers).toBe(0);
  });
});

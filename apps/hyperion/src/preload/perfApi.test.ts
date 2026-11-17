import { describe, expect, it } from "vitest";

import { PERF_CHANNELS } from "../main/perf/handlers";
import type { PerfLaunch } from "./api";
import { PERF_CHANNEL_NAMES, perfApi, perfMember, readPerfConfig } from "./perfApi";
import { perfLaunchFromArgv, perfSwitch, readPerfLaunch } from "./perfLaunch";

const LAUNCH: PerfLaunch = {
  scene: "cockpit-descent",
  setting: "low",
  out: null,
  repetitions: 5,
  control: false,
  force: false,
};

/** An `invoke` that answers nothing. */
function invoke(): Promise<unknown> {
  return Promise.resolve(undefined);
}

describe("perf mode's preload", () => {
  it("calls the main process's channels", () => {
    expect(PERF_CHANNEL_NAMES).toEqual(PERF_CHANNELS);
  });

  it("carries the launch in the renderer's argv, and only on a perf launch", () => {
    expect(perfLaunchFromArgv(["electron", perfSwitch(LAUNCH)])).toEqual(LAUNCH);
    expect(perfLaunchFromArgv(["electron"])).toBeNull();
    expect(perfMember([perfSwitch(LAUNCH)], { invoke }).perf).toBeDefined();
    expect(perfMember([], { invoke })).toEqual({});
  });

  it("refuses a switch that holds no launch", () => {
    expect(() => perfLaunchFromArgv(["--hyperion-perf-run=%7B"])).toThrow(/not JSON/);
    expect(readPerfLaunch({ ...LAUNCH, scene: "cockpit" })).toBeNull();
    expect(readPerfLaunch({ ...LAUNCH, repetitions: 0 })).toBeNull();
  });

  it("narrows the main process's answers at the bridge", async () => {
    const config = {
      launch: LAUNCH,
      switches: ["--use-angle=vulkan"],
      gpuTiming: true,
      launchMode: "vulkan",
      hidden: false,
      serverUrl: "ws://127.0.0.1:40001/ws",
      arrival: { universe: "3504954ff4feabf4", system: "4200acb20000000d", body: null },
    };
    expect(readPerfConfig(config)).toEqual(config);
    expect(readPerfConfig({ ...config, launchMode: "metal" })).toBeNull();
    const calls: unknown[][] = [];
    const api = perfApi({
      invoke: (channel, ...args) => {
        calls.push([channel, ...args]);
        switch (channel) {
          case PERF_CHANNEL_NAMES.config:
            return Promise.resolve(config);
          case PERF_CHANNEL_NAMES.warmUpSample:
            return Promise.resolve({ kind: "steady", elapsedS: 150 });
          case PERF_CHANNEL_NAMES.beginRepetition:
            return Promise.resolve({
              serverUrl: "ws://127.0.0.1:40002/ws",
              arrival: config.arrival,
            });
          default:
            return Promise.resolve(undefined);
        }
      },
    });
    await expect(api.config()).resolves.toEqual(config);
    await expect(api.warmUpSample({ intervalsMs: [33.4] })).resolves.toEqual({
      kind: "steady",
      elapsedS: 150,
    });
    await expect(api.beginRepetition(1)).resolves.toEqual({
      serverUrl: "ws://127.0.0.1:40002/ws",
      arrival: config.arrival,
    });
    await api.finish({ status: "complete", notes: "" });
    expect(calls.map(([channel]) => channel)).toEqual([
      PERF_CHANNEL_NAMES.config,
      PERF_CHANNEL_NAMES.warmUpSample,
      PERF_CHANNEL_NAMES.beginRepetition,
      PERF_CHANNEL_NAMES.finish,
    ]);
    const odd = perfApi({ invoke: () => Promise.resolve({ kind: "maybe" }) });
    await expect(odd.warmUpSample({ intervalsMs: [] })).rejects.toThrow(/not of its shape/);
  });
});

import { describe, expect, it, vi } from "vitest";

import { SPIKE_CHANNELS } from "../main/spike";
import { SPIKE_CHANNEL_NAMES, spikeApi, spikeMember } from "./spikeApi";
import { DEFAULT_SPIKE_SEED, type SpikeLaunch, spikeSwitch } from "./spikeLaunch";

const LAUNCH: SpikeLaunch = {
  setting: "high",
  seed: DEFAULT_SPIKE_SEED,
  smoke: true,
  out: null,
  workers: null,
  vertexPath: null,
  normals: null,
  ridged: "off",
  dawnSafety: "on",
  capture: null,
};

describe("the preload's spike functions", () => {
  it("are absent on an ordinary launch, and present on a spike launch", () => {
    const deps = { invoke: () => Promise.resolve(), privateKib: () => Promise.resolve(0) };
    expect(spikeMember(["--hyperion-server-url=ws://127.0.0.1:7878/ws"], deps)).toEqual({});
    expect(spikeMember([spikeSwitch(LAUNCH)], deps).spike?.launch).toEqual(LAUNCH);
  });

  it("use the main process's channels", () => {
    expect(SPIKE_CHANNEL_NAMES).toEqual(SPIKE_CHANNELS);
  });

  it("send each operation on its own channel", async () => {
    const invoke = vi.fn<(channel: string, ...args: unknown[]) => Promise<unknown>>((channel) =>
      Promise.resolve(
        channel === SPIKE_CHANNEL_NAMES.writeResults ? { json: "a", markdown: "b" } : "/dir",
      ),
    );
    const api = spikeApi(LAUNCH, { invoke, privateKib: () => Promise.resolve(2.5) });
    await api.startTrace();
    await api.sampleMemory();
    await api.end({ status: "pass" });
    expect(invoke.mock.calls).toEqual([
      [SPIKE_CHANNEL_NAMES.startTrace],
      [SPIKE_CHANNEL_NAMES.memory, 2560],
      [SPIKE_CHANNEL_NAMES.end, { status: "pass" }],
    ]);
    expect(api.launch).toBe(LAUNCH);
  });

  it("refuses an answer of the wrong shape", async () => {
    const api = spikeApi(LAUNCH, {
      invoke: () => Promise.resolve(undefined),
      privateKib: () => Promise.resolve(0),
    });
    await expect(api.writeCapture({ json: "{}", bin: new Uint8Array() })).rejects.toThrow(
      /capture/,
    );
  });
});

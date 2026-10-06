import { describe, expect, it } from "vitest";

import {
  DEFAULT_SPIKE_SEED,
  readSpikeLaunch,
  type SpikeLaunch,
  spikeLaunchFromArgv,
  spikeSwitch,
} from "./spikeLaunch";

const LAUNCH: SpikeLaunch = {
  setting: "low",
  seed: "18446744073709551615",
  smoke: true,
  out: "/data/out dir",
  workers: 3,
  vertexPath: "face-differences",
  normals: "mesh",
  ridged: "on",
  dawnSafety: "off",
  capture: "/data/capture=1",
  traceProfile: "on",
};

describe("the spike's switch", () => {
  it("carries the options to the preload unchanged", () => {
    expect(spikeLaunchFromArgv(["--type=renderer", spikeSwitch(LAUNCH), "--x"])).toEqual(LAUNCH);
  });

  it("is absent on an ordinary launch", () => {
    expect(spikeLaunchFromArgv(["--type=renderer", "--hyperion-server-url=ws://a/ws"])).toBeNull();
  });

  it("refuses a switch that holds no options", () => {
    expect(() => spikeLaunchFromArgv(["--hyperion-descent-spike=%7B"])).toThrow(/not JSON/);
    expect(() => spikeLaunchFromArgv(["--hyperion-descent-spike=%7B%7D"])).toThrow(/options/);
  });
});

describe("readSpikeLaunch", () => {
  it("reads valid options", () => {
    expect(readSpikeLaunch({ ...LAUNCH, seed: DEFAULT_SPIKE_SEED })).toEqual({
      ...LAUNCH,
      seed: DEFAULT_SPIKE_SEED,
    });
  });

  it.each([
    ["setting", "medium"],
    ["seed", "18446744073709551616"],
    ["seed", 7],
    ["smoke", "yes"],
    ["out", ""],
    ["workers", 0],
    ["workers", 1.5],
    ["vertexPath", "offsets"],
    ["normals", "triple"],
    ["ridged", true],
    ["dawnSafety", null],
    ["capture", 3],
    ["traceProfile", "yes"],
    ["traceProfile", null],
  ])("refuses %s = %j", (field, value) => {
    expect(readSpikeLaunch({ ...LAUNCH, [field]: value })).toBeNull();
  });

  it("refuses options with a field missing", () => {
    const { traceProfile: _, ...partial } = LAUNCH;
    expect(readSpikeLaunch(partial)).toBeNull();
    expect(readSpikeLaunch(null)).toBeNull();
  });
});

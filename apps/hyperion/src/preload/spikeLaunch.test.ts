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
  world: "terrain-test",
  style: "wireframe",
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
    ["world", "earth"],
    ["world", null],
    ["style", "lit"],
    ["style", null],
  ])("refuses %s = %j", (field, value) => {
    expect(readSpikeLaunch({ ...LAUNCH, [field]: value })).toBeNull();
  });

  it("reads the terrain world in each style, and the test planet in none", () => {
    for (const style of ["photorealistic", "wireframe"] as const) {
      expect(readSpikeLaunch({ ...LAUNCH, world: "terrain-test", style })).toEqual({
        ...LAUNCH,
        world: "terrain-test",
        style,
      });
    }
    expect(readSpikeLaunch({ ...LAUNCH, world: "test-planet", style: null })).toEqual({
      ...LAUNCH,
      world: "test-planet",
      style: null,
    });
  });

  it.each(["photorealistic", "wireframe"] as const)(
    "refuses the test planet with a style, %s: its main view is SPIKE LIT",
    (style) => {
      expect(readSpikeLaunch({ ...LAUNCH, world: "test-planet", style })).toBeNull();
    },
  );

  it("refuses options with a field missing", () => {
    const { traceProfile: _, ...partial } = LAUNCH;
    expect(readSpikeLaunch(partial)).toBeNull();
    const { world: _world, ...worldless } = LAUNCH;
    expect(readSpikeLaunch(worldless)).toBeNull();
    const { style: _style, ...styleless } = LAUNCH;
    expect(readSpikeLaunch(styleless)).toBeNull();
    expect(readSpikeLaunch(null)).toBeNull();
  });
});

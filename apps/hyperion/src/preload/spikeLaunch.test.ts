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

  it("carries the measured sizes on to the preload", () => {
    const measured: SpikeLaunch = { ...LAUNCH, sizes: "on" };
    expect(spikeLaunchFromArgv([spikeSwitch(measured)])).toEqual(measured);
  });

  it("is absent on an ordinary launch", () => {
    expect(spikeLaunchFromArgv(["--type=renderer", "--hyperion-server-url=ws://a/ws"])).toBeNull();
  });

  it("refuses a switch that holds no options", () => {
    expect(() => spikeLaunchFromArgv(["--hyperion-descent-spike=%7B"])).toThrow(/not JSON/);
    expect(() => spikeLaunchFromArgv(["--hyperion-descent-spike=%7B%7D"])).toThrow(/options/);
  });
});

/** A served world's launch (R10.T15.b): its universe and body, place-ship's placement's. */
const SERVED: SpikeLaunch = {
  ...LAUNCH,
  ridged: "off",
  vertexPath: null,
  normals: null,
  world: "served",
  served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023.0100" },
};

describe("a served world's launch (R10.T15.b)", () => {
  it("carries its universe and body to the preload", () => {
    expect(spikeLaunchFromArgv([spikeSwitch(SERVED)])).toEqual(SERVED);
    expect(readSpikeLaunch({ ...SERVED, sizes: "on" })).toEqual({ ...SERVED, sizes: "on" });
  });

  it.each([
    [{ served: undefined }],
    [{ served: { universe: "3504954ff4feabf4" } }],
    [{ served: { universe: "3504954FF4FEABF4", body: "61ffd967fe000023.0100" } }],
    [{ served: { universe: "3504954ff4feabf4", body: "61ffd967fe000023" } }],
    [{ style: null }],
  ])("refuses a served launch with %j", (change) => {
    expect(readSpikeLaunch({ ...SERVED, ...change })).toBeNull();
  });

  it("refuses a served world's names on any other world", () => {
    expect(readSpikeLaunch({ ...LAUNCH, served: SERVED.served })).toBeNull();
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
    ["sizes", "yes"],
    ["sizes", null],
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

  it("reads the measured sizes on over the terrain world", () => {
    expect(readSpikeLaunch({ ...LAUNCH, sizes: "on" })).toEqual({ ...LAUNCH, sizes: "on" });
  });

  it("reads the measured sizes off as absent, the launch as it was before the option", () => {
    expect(readSpikeLaunch({ ...LAUNCH, sizes: "off" })).toEqual(LAUNCH);
    expect(readSpikeLaunch({ ...LAUNCH, sizes: "off" })).not.toHaveProperty("sizes");
  });

  it("reads the horizon map off in the photorealistic style, the A/B's (R10.T15.b)", () => {
    const lit = { ...LAUNCH, style: "photorealistic" } as const;
    expect(readSpikeLaunch({ ...lit, horizon: "off" })).toEqual({ ...lit, horizon: "off" });
  });

  it("reads the horizon map on as absent, the launch as it was before the option", () => {
    expect(readSpikeLaunch({ ...LAUNCH, horizon: "on" })).toEqual(LAUNCH);
  });

  it.each([
    ["over the test planet, whose bakes make none", { world: "test-planet", style: null }],
    ["in the wireframe, whose bakes skip it already", { style: "wireframe" }],
    ["of another word", { style: "photorealistic", horizon: "skipped" }],
  ] as const)("refuses the horizon map off %s", (_why, change) => {
    expect(readSpikeLaunch({ ...LAUNCH, horizon: "off", ...change })).toBeNull();
  });

  it("refuses the measured sizes over the test planet, whose terrain is no session's", () => {
    expect(
      readSpikeLaunch({ ...LAUNCH, world: "test-planet", style: null, sizes: "on" }),
    ).toBeNull();
  });

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

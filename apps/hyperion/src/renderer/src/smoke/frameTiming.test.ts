import { describe, expect, it } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../test/fakeGpu";
import { FakeRenderEngine } from "../test/fakeRenderEngine";
import { timeAtmosphereFrame } from "./frameTiming";
import { Checks } from "./harness";

describe("the frame timing on a device without timestamp-query (R08.T11)", () => {
  it("fails the emitting ground's timings, R08.T18.a's (O), with the reason, as the full run names it", async () => {
    const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
    const checks = new Checks();
    await timeAtmosphereFrame(new FakeRenderEngine(device), checks);
    expect(checks.list).toContainEqual({
      name: "R08.T11 the frame without its air, its ground emitting's timings, high, the measured Earth",
      pass: false,
      detail: "no timestamp-query, so no pass times",
    });
  });
});

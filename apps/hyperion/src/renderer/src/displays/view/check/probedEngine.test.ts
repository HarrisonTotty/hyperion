import { describe, expect, it, type Mock, vi } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { FakeRenderEngine } from "../../../test/fakeRenderEngine";
import type { DepthLayerPass } from "../../../view/engine/types";
import { type EngineProbe, ProbedEngine } from "./probedEngine";

/** A probe whose renders and submissions a test reads. */
interface RecordingProbe extends EngineProbe {
  readonly rendered: Mock<EngineProbe["rendered"]>;
  readonly submitted: Mock<EngineProbe["submitted"]>;
}

/** A probe whose every member is a mock. */
function recordingProbe(): RecordingProbe {
  return {
    rendered: vi.fn<EngineProbe["rendered"]>(),
    sized: vi.fn<EngineProbe["sized"]>(),
    submitted: vi.fn<EngineProbe["submitted"]>(),
    madeTarget: vi.fn<EngineProbe["madeTarget"]>(),
    preparing: vi.fn<EngineProbe["preparing"]>(),
  };
}

const PASS: DepthLayerPass = {
  label: "view:shadow 0",
  layer: 0,
  viewRotation: new Float32Array(16),
  projection: new Float32Array(16),
  draws: [],
};

describe("a probed engine's depth layers (R10.T9.b)", () => {
  it("forward the render, and report its main-thread time as the owner's shadows, with no resolves", async () => {
    const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
    const inner = new FakeRenderEngine(device);
    inner.passTimesFrame = 7;
    const probe = recordingProbe();
    let clockMs = 100;
    const engine = new ProbedEngine(inner, probe, () => {
      clockMs += 1.5;
      return clockMs;
    });
    const layers = engine.createDepthLayers({
      name: "cascades",
      sizePx: 64,
      layers: 4,
      category: "shadows",
    });
    layers.render("view", [PASS]);
    expect({
      forwarded: inner.depthLayers[0]?.renders,
      depth: layers.depth === inner.depthLayers[0]?.depth,
      rendered: probe.rendered.mock.calls,
      submitted: probe.submitted.mock.calls.length,
    }).toEqual({
      forwarded: [{ owner: "view", passes: [PASS] }],
      depth: true,
      rendered: [["view:shadows", 7, 7, 1.5]],
      submitted: 0,
    });
  });
});

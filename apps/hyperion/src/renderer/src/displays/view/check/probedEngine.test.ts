import { describe, expect, it, type Mock, vi } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { FakeRenderEngine } from "../../../test/fakeRenderEngine";
import { COMPOSITE_MATERIAL, SKY_VIEW_KERNEL } from "../../../view/atmosphere/hillaire";
import type { DepthLayerPass, WgslMaterialSpec } from "../../../view/engine/types";
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
    dispatched: vi.fn<EngineProbe["dispatched"]>(),
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

describe("a probed engine's composite probe (the owner's priority, 2026-11-01)", () => {
  /** The WGSL each material was made with through `engine`, by name, in order. */
  async function madeWith(
    probe: "sky-off" | "transmittance-off" | "per-pixel" | "surface-grey" | null,
  ): Promise<ReadonlyArray<readonly [string, string]>> {
    const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
    const inner = new FakeRenderEngine(device);
    // The fake makes an async material through its `createMaterial`, which sees both.
    const made = vi.spyOn(inner, "createMaterial");
    const engine = new ProbedEngine(inner, recordingProbe(), () => 0, probe);
    const other: WgslMaterialSpec = { ...COMPOSITE_MATERIAL, name: SKY_VIEW_KERNEL.name };
    await engine.createMaterialAsync(COMPOSITE_MATERIAL, ["rgba16float"]);
    engine.createMaterial(other);
    return made.mock.calls.map(([spec]) => [
      spec.name,
      /const COMPOSITE_PROBE : u32 = (\d)u;/u.exec(spec.fragmentWgsl)?.[1] ?? "none",
    ]);
  }

  it("makes the composite as drawn without a probe", async () => {
    expect(await madeWith(null)).toEqual([
      ["atmosphere composite", "0"],
      [SKY_VIEW_KERNEL.name, "0"],
    ]);
  });

  it.each([
    ["sky-off", "1"],
    ["transmittance-off", "2"],
    ["per-pixel", "3"],
    ["surface-grey", "4"],
  ] as const)("makes the composite alone under %s", async (probe, value) => {
    expect(await madeWith(probe)).toEqual([
      ["atmosphere composite", value],
      [SKY_VIEW_KERNEL.name, "0"],
    ]);
  });
});

import { describe, expect, it } from "vitest";

import type { AllocationEvent } from "../view/engine/memory";
import type { DepthLayerPass, WgslMaterialSpec } from "../view/engine/types";
import { countingRenderEngine } from "./countingRenderEngine";

/** The cascades' set: four 2,048² layers under `shadows`. */
const CASCADES = {
  name: "terrain shadow cascades",
  sizePx: 2048,
  layers: 4,
  category: "shadows",
} as const;

/** An empty pass into `layer`. */
function pass(layer: number): DepthLayerPass {
  return {
    label: `view:shadow ${layer}`,
    layer,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: [],
  };
}

describe("the counting engine's depth layers (R10.T9.b)", () => {
  it("count the cascades' 67,108,864 B under shadows, and release them at disposal", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const target = engine.createDepthLayers(CASCADES);
    target.dispose();
    target.dispose();
    expect(events).toEqual([
      { kind: "created", name: CASCADES.name, bytes: 67_108_864, category: "shadows" },
      { kind: "destroyed", name: CASCADES.name, bytes: 67_108_864, category: "shadows" },
    ]);
  });

  it("record each render with its owner, and count its passes", async () => {
    const engine = await countingRenderEngine();
    const target = engine.createDepthLayers(CASCADES);
    target.render("view", [pass(0), pass(1), pass(2), pass(3)]);
    expect({
      renders: engine.depthLayerRenders.map(({ owner, passes }) => [owner, passes.length]),
      targets: engine.counts.depthLayerTargets,
      passes: engine.counts.depthLayerPasses,
    }).toEqual({ renders: [["view", 4]], targets: 1, passes: 4 });
  });

  it("refuse the passes the engine refuses", async () => {
    const engine = await countingRenderEngine();
    const target = engine.createDepthLayers(CASCADES);
    expect(() => {
      target.render("view", [pass(4)]);
    }).toThrow(/which has layers 0 to 3/u);
  });

  it("draw nothing once a modelled loss took them", async () => {
    const engine = await countingRenderEngine();
    const target = engine.createDepthLayers(CASCADES);
    engine.loseHandles();
    target.render("view", [pass(0)]);
    expect(engine.depthLayerRenders).toEqual([]);
  });
});

describe("the counting engine's unclippedDepth (R10.T9.b)", () => {
  const CASTER: WgslMaterialSpec = {
    name: "terrain caster",
    displayName: "TERRAIN SHADOWS",
    vertexWgsl: "",
    fragmentWgsl: "",
    uniforms: [],
    samplers: [],
    cullMode: "none",
    depthWrite: true,
    colourWrites: false,
    blend: "none",
    unclippedDepth: true,
  };

  it("is refused on its fake device, granted no optional features and so no depth-clip-control, as a throw and as a rejection", async () => {
    const engine = await countingRenderEngine();
    expect(() => engine.createMaterial(CASTER)).toThrow(/depth-clip-control/u);
    await expect(engine.createMaterialAsync(CASTER, ["depth-only"])).rejects.toThrow(
      /depth-clip-control/u,
    );
  });
});

describe("the counting engine's kernels (R01.T15)", () => {
  const TOY = { name: "toy", reference: "", subgroup: null, readback: "bit-exact" } as const;

  it("records every kernel asked, and releases each handle once", async () => {
    const engine = await countingRenderEngine();
    const kernel = engine.createCompute(TOY);
    await engine.createComputeAsync(TOY);
    expect(engine.kernelPairs).toEqual([TOY, TOY]);
    engine.releaseCompute(kernel);
    expect(() => {
      engine.releaseCompute(kernel);
    }).toThrow(/kernel toy was not made by this engine, or was released/u);
  });

  it("drops the release of a kernel a modelled loss took, once", async () => {
    const engine = await countingRenderEngine();
    const kernel = engine.createCompute(TOY);
    engine.loseHandles();
    expect(() => {
      engine.releaseCompute(kernel);
    }).not.toThrow();
    expect(() => {
      engine.releaseCompute(kernel);
    }).toThrow(/was released/u);
  });
});

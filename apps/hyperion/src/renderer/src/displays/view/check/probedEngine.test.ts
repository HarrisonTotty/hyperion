import { describe, expect, it, type Mock, vi } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { FakeRenderEngine } from "../../../test/fakeRenderEngine";
import {
  COMPOSITE_MATERIAL,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  SKY_VIEW_KERNEL,
} from "../../../view/atmosphere/hillaire";
import { MARCH_CENSUS_KERNEL } from "../../../view/atmosphere/marchProbe";
import type {
  ComputeBindings,
  DepthLayerPass,
  RenderEngine,
  TextureHandle,
  WgslMaterialSpec,
} from "../../../view/engine/types";
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

/** A march's bindings over an area, as `HillaireAtmosphere.drawFrame` gives them. */
function marchBindings(engine: RenderEngine): ComputeBindings {
  const texture: TextureHandle = { kind: "texture", name: "t" };
  return {
    uniforms: {
      medium: new Float32Array(16),
      view: new Float32Array(4),
      marchArea: new Float32Array(8),
    },
    buffers: {
      marchTiles: engine.createBuffer({ name: "tiles", bytes: 4, usage: 0, category: "other" }),
    },
    sampled: { sceneDepth: texture, oblateBody: texture },
    storage: {},
  };
}

describe("a probed engine's march probe and census (plan R08, R08.T8.d)", () => {
  /** A probed engine over a fake whose dispatches, writes and read-backs a test reads. */
  async function probed(marchProbe: "centre" | null = null): Promise<{
    readonly engine: ProbedEngine;
    readonly inner: FakeRenderEngine;
    readonly dispatched: Mock<RenderEngine["dispatch"]>;
    readonly made: Mock<RenderEngine["createCompute"]>;
  }> {
    const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
    const inner = new FakeRenderEngine(device);
    // The fake refuses dispatches, writes and read-backs; its interface's members take them here.
    const asEngine: RenderEngine = inner;
    const dispatched = vi.fn<RenderEngine["dispatch"]>();
    vi.spyOn(asEngine, "dispatch").mockImplementation(dispatched);
    vi.spyOn(asEngine, "writeBuffer").mockImplementation(() => undefined);
    vi.spyOn(asEngine, "readBuffer").mockImplementation(() => {
      const words = new Uint32Array(17);
      words.set([5, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 1, 6, 1, 1, 17, 64]);
      return Promise.resolve(words.buffer);
    });
    const made = vi.spyOn(inner, "createCompute");
    const engine = new ProbedEngine(inner, recordingProbe(), () => 0, null, marchProbe);
    return { engine, inner, dispatched, made };
  }

  it("makes the march's kernels with no footprint under `centre`, and every other as it is", async () => {
    const { engine, made } = await probed("centre");
    for (const pair of [
      RAY_MARCH_KERNEL,
      RAY_MARCH_CENTRE_KERNEL,
      RAY_MARCH_FILL_KERNEL,
      SKY_VIEW_KERNEL,
    ]) {
      engine.createCompute(pair);
    }
    await engine.createComputeAsync(RAY_MARCH_SLOTS_KERNEL);
    expect(
      made.mock.calls.map(([pair]) => [pair.name, pair.reference.includes("let above = false;")]),
    ).toEqual([
      [RAY_MARCH_KERNEL.name, true],
      [RAY_MARCH_CENTRE_KERNEL.name, true],
      [RAY_MARCH_FILL_KERNEL.name, false],
      [SKY_VIEW_KERNEL.name, false],
      [RAY_MARCH_SLOTS_KERNEL.name, true],
    ]);
    const plain = await probed();
    plain.engine.createCompute(RAY_MARCH_KERNEL);
    expect(plain.made.mock.calls[0]?.[0]).toBe(RAY_MARCH_KERNEL);
  });

  it("takes a census beside the next march over an area alone, once, and releases what it made", async () => {
    const { engine, inner, dispatched } = await probed();
    const census = engine.marchCensus(new AbortController().signal);
    const bindings = marchBindings(engine);
    const sky = engine.createCompute(SKY_VIEW_KERNEL);
    const centre = engine.createCompute(RAY_MARCH_CENTRE_KERNEL);
    const march = engine.createCompute(RAY_MARCH_KERNEL);
    engine.dispatch(sky, bindings, [1, 1, 1], "atmosphere view");
    engine.dispatch(centre, bindings, [2, 3, 1], "atmosphere view");
    engine.dispatch(march, bindings, [2, 3, 1], "atmosphere view");
    engine.dispatch(march, bindings, [2, 3, 1], "atmosphere view");
    expect(await census).toEqual({
      kernel: RAY_MARCH_KERNEL.name,
      texelsByNodes: [5, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1],
      wholeMeans: 1,
      centresInAir: 6,
      tiles: 1,
      limbTiles: 1,
      rays: 17,
      laneRays: 64,
    });
    expect({
      dispatched: dispatched.mock.calls.map(([kernel, , workgroups, pass]) => [
        kernel.name,
        workgroups,
        pass,
      ]),
      counted: Object.keys(dispatched.mock.calls[3]?.[1].uniforms ?? {}),
      released: inner.released.map((handle) => handle.name),
    }).toEqual({
      dispatched: [
        [SKY_VIEW_KERNEL.name, [1, 1, 1], "atmosphere view"],
        [RAY_MARCH_CENTRE_KERNEL.name, [2, 3, 1], "atmosphere view"],
        [RAY_MARCH_KERNEL.name, [2, 3, 1], "atmosphere view"],
        [MARCH_CENSUS_KERNEL.name, [2, 3, 1], MARCH_CENSUS_KERNEL.name],
        [RAY_MARCH_KERNEL.name, [2, 3, 1], "atmosphere view"],
      ],
      counted: ["medium", "view", "marchArea"],
      released: [MARCH_CENSUS_KERNEL.name, MARCH_CENSUS_KERNEL.name],
    });
  });

  it("takes no census its ask's signal withdrew", async () => {
    const { engine, dispatched } = await probed();
    const stop = new AbortController();
    void engine.marchCensus(stop.signal);
    stop.abort();
    engine.dispatch(engine.createCompute(RAY_MARCH_KERNEL), marchBindings(engine), [1, 1, 1]);
    expect(dispatched.mock.calls.map(([kernel]) => kernel.name)).toEqual([RAY_MARCH_KERNEL.name]);
  });
});

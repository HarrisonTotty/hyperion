import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { BODY_DISC_CELLS_KERNEL, BODY_DISC_MATERIALS } from "../bodies/draw";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  ComputeBindings,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  TextureHandle,
} from "../engine/types";
import { COMPOSITE_MATERIAL } from "./hillaire";
import { SourceTableEngine } from "./sourceTableEngine";
import {
  airlessKernel,
  airlessMaterial,
  createAirlessTables,
  isAirlessTable,
  ONE_BAND_SOURCE_TABLE_NAME,
  SOURCE_TABLE_NAME,
  tableOnlyKernel,
  tableOnlyMaterial,
} from "./surfaceLighting";

/** A sources' table of `name`, made through `engine`. */
function tableOf(engine: SourceTableEngine, name: string): TextureHandle {
  return engine.createTexture({
    name,
    size: [4, 4, 1],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING,
    category: "atmosphere-tables",
  });
}

/** A draw of `material` with `table` bound as its sources' table. */
function drawOf(material: MaterialHandle, mesh: MeshHandle, table: TextureHandle): DrawItem {
  return {
    mesh,
    material,
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: { atmosphereSourceTable: table },
  };
}

/** A wrapped counting engine, a scene target, a mesh and the disc's interior made through it. */
async function setUp(): Promise<{
  readonly inner: Awaited<ReturnType<typeof countingRenderEngine>>;
  readonly engine: SourceTableEngine;
  readonly material: MaterialHandle;
  readonly mesh: MeshHandle;
  readonly render: (draw: DrawItem) => string | undefined;
}> {
  const inner = await countingRenderEngine();
  const engine = new SourceTableEngine(inner);
  const target = engine.createRenderTarget({
    name: "scene",
    size: { widthPx: 4, heightPx: 4 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const mesh = engine.createMesh({
    name: "quad",
    positions: new Float32Array(9),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
  const material = engine.createMaterial(BODY_DISC_MATERIALS.interior);
  const render = (draw: DrawItem): string | undefined => {
    target.render({
      label: "bodies",
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [draw],
      postProcesses: [],
    });
    return inner.targetFrames.at(-1)?.draws[0]?.material.name;
  };
  return { inner, engine, material, mesh, render };
}

describe("the lit passes' table-only twins (R08.T9.b's lever)", () => {
  it("makes a twin of a material or kernel that binds a sources' table, and of nothing else", () => {
    const twin = tableOnlyMaterial(BODY_DISC_MATERIALS.interior);
    const kernel = tableOnlyKernel(BODY_DISC_CELLS_KERNEL);
    expect({
      twin: twin?.name,
      set: twin?.fragmentWgsl.includes("const SOURCE_TABLE_ONLY : bool = true;"),
      unset: twin?.fragmentWgsl.includes("const SOURCE_TABLE_ONLY : bool = false;"),
      kernel: kernel?.name,
      composite: tableOnlyMaterial(COMPOSITE_MATERIAL),
      again: twin === null ? "none" : tableOnlyMaterial(twin),
    }).toEqual({
      twin: "bodies:disc (table only)",
      set: true,
      unset: false,
      kernel: "BODY DISC CELLS (table only)",
      composite: null,
      again: null,
    });
  });

  it("draws a body of one band by its twin once the twin is prepared there, and every other draw as written", async () => {
    const { engine, material, mesh, render } = await setUp();
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    const several = tableOf(engine, SOURCE_TABLE_NAME);
    // The first draw begins the twin's making and preparing, and draws as written.
    const first = render(drawOf(material, mesh, oneBand));
    await engine.twinsSettled();
    expect({
      first,
      twinned: render(drawOf(material, mesh, oneBand)),
      several: render(drawOf(material, mesh, several)),
      swaps: engine.swaps,
    }).toEqual({
      first: "bodies:disc",
      twinned: "bodies:disc (table only)",
      several: "bodies:disc",
      swaps: 1,
    });
  });

  it("renders a chain of its targets in one submission by their twins, as each render would (plan R14)", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    render(drawOf(material, mesh, oneBand));
    await engine.twinsSettled();
    const second = engine.createRenderTarget({
      name: "scene 2",
      size: { widthPx: 4, heightPx: 4 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    const before = inner.targetFrames.length;
    const frame = {
      label: "bodies",
      viewRotation: new Float32Array(16),
      projection: new Float32Array(16),
      draws: [drawOf(material, mesh, oneBand)],
      postProcesses: [],
    };
    // A target of the scene's format, where the twin is prepared already.
    second.render(frame);
    engine.renderTargets([{ target: second, frame }]);
    expect(inner.targetFrames.slice(before).map((drawn) => drawn.draws[0]?.material.name)).toEqual([
      "bodies:disc (table only)",
      "bodies:disc (table only)",
    ]);
  });

  it("dispatches a kernel's twin where a one-band table is bound, from the dispatch after it is made", async () => {
    const { inner, engine } = await setUp();
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const bindings = {
      uniforms: {},
      buffers: {},
      sampled: { atmosphere_source_table: oneBand },
      storage: {},
    };
    engine.dispatch(kernel, bindings, [1, 1, 1]);
    await engine.twinsSettled();
    engine.dispatch(kernel, bindings, [1, 1, 1]);
    engine.dispatch(kernel, { ...bindings, sampled: {} }, [1, 1, 1]);
    expect(inner.dispatched.map((d) => d.kernel)).toEqual([
      "BODY DISC CELLS",
      "BODY DISC CELLS (table only)",
      "BODY DISC CELLS",
    ]);
  });

  it("releases a twin with its material", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const before = inner.live.materials;
    render(drawOf(material, mesh, tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME)));
    await engine.twinsSettled();
    const made = inner.live.materials;
    engine.releaseMaterial(material);
    expect([before, made, inner.live.materials]).toEqual([before, before + 1, before - 1]);
  });

  it("makes a twin in a task after the frame that first draws or dispatches it, never in that frame (perf-long-tasks-4)", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const kernelsMade = vi.spyOn(inner, "createComputeAsync");
    const before = inner.live.materials;
    render(drawOf(material, mesh, oneBand));
    engine.dispatch(
      kernel,
      { uniforms: {}, buffers: {}, sampled: { atmosphere_source_table: oneBand }, storage: {} },
      [1, 1, 1],
    );
    expect([inner.live.materials, kernelsMade.mock.calls.length]).toEqual([before, 0]);
    await engine.twinsSettled();
    expect([inner.live.materials, kernelsMade.mock.calls.length]).toEqual([before + 1, 1]);
  });

  it("makes no twin of a material or kernel let go before its task (perf-long-tasks-4)", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const kernelsMade = vi.spyOn(inner, "createComputeAsync");
    const before = inner.live.materials;
    render(drawOf(material, mesh, oneBand));
    engine.dispatch(
      kernel,
      { uniforms: {}, buffers: {}, sampled: { atmosphere_source_table: oneBand }, storage: {} },
      [1, 1, 1],
    );
    engine.releaseMaterial(material);
    engine.releaseCompute(kernel);
    await engine.twinsSettled();
    expect([inner.live.materials, kernelsMade.mock.calls.length]).toEqual([before - 1, 0]);
  });
});

/** A draw of `material` with `body` bound as its body's table and its sources' table. */
function bodyDrawOf(material: MaterialHandle, mesh: MeshHandle, body: TextureHandle): DrawItem {
  return {
    mesh,
    material,
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: { atmosphereBody: body, atmosphereSourceTable: body },
  };
}

/** A dispatch's bindings with `body` as its body's table and `table` as its sources' table. */
function sampled(body: TextureHandle, table: TextureHandle): ComputeBindings {
  return {
    uniforms: {},
    buffers: {},
    sampled: { atmosphere_body: body, atmosphere_source_table: table },
    storage: {},
  };
}

describe("the lit passes' airless twins (plan R07, perf-bodies-pass)", () => {
  it("makes a twin of a material or kernel that binds a body's tables, and of nothing else", () => {
    const twin = airlessMaterial(BODY_DISC_MATERIALS.interior);
    const kernel = airlessKernel(BODY_DISC_CELLS_KERNEL);
    expect({
      twin: twin?.name,
      set: twin?.fragmentWgsl.includes("const AIRLESS_ONLY : bool = true;"),
      unset: twin?.fragmentWgsl.includes("const AIRLESS_ONLY : bool = false;"),
      // The table-only constant is left as written.
      tableOnly: twin?.fragmentWgsl.includes("const SOURCE_TABLE_ONLY : bool = false;"),
      kernel: kernel?.name,
      composite: airlessMaterial(COMPOSITE_MATERIAL),
      again: twin === null ? "none" : airlessMaterial(twin),
    }).toEqual({
      twin: "bodies:disc (airless)",
      set: true,
      unset: false,
      tableOnly: true,
      kernel: "BODY DISC CELLS (airless)",
      composite: null,
      again: null,
    });
  });

  it("knows the airless tables' texture alone", async () => {
    const { engine } = await setUp();
    const airless = createAirlessTables(engine, "bodies:no atmosphere");
    expect({
      body: isAirlessTable(airless.body),
      every: isAirlessTable(airless.transmittance),
      table: isAirlessTable(tableOf(engine, SOURCE_TABLE_NAME)),
      none: isAirlessTable(undefined),
    }).toEqual({ body: true, every: true, table: false, none: false });
  });

  it("draws an airless body by its twin once the twin is prepared there, and a body with air as written", async () => {
    const { engine, material, mesh, render } = await setUp();
    const airless = createAirlessTables(engine, "bodies:no atmosphere");
    const air = tableOf(engine, SOURCE_TABLE_NAME);
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    // The first draw begins the twin's making and preparing, and draws as written.
    const first = render(bodyDrawOf(material, mesh, airless.body));
    await engine.twinsSettled();
    expect({
      first,
      twinned: render(bodyDrawOf(material, mesh, airless.body)),
      air: render(bodyDrawOf(material, mesh, air)),
      // A body of one band keeps its table-only twin, made at its own first draw.
      oneBandFirst: render(drawOf(material, mesh, oneBand)),
    }).toEqual({
      first: "bodies:disc",
      twinned: "bodies:disc (airless)",
      air: "bodies:disc",
      oneBandFirst: "bodies:disc",
    });
    await engine.twinsSettled();
    expect({
      oneBand: render(drawOf(material, mesh, oneBand)),
      airless: render(bodyDrawOf(material, mesh, airless.body)),
      swaps: engine.swaps,
    }).toEqual({
      oneBand: "bodies:disc (table only)",
      airless: "bodies:disc (airless)",
      swaps: 3,
    });
  });

  it("dispatches a kernel's airless twin where the airless tables are bound, from the dispatch after it is made", async () => {
    const { inner, engine } = await setUp();
    const airless = createAirlessTables(engine, "bodies:no atmosphere");
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const bindings = {
      uniforms: {},
      buffers: {},
      sampled: { atmosphere_body: airless.body, atmosphere_source_table: airless.body },
      storage: {},
    };
    engine.dispatch(kernel, bindings, [1, 1, 1]);
    await engine.twinsSettled();
    engine.dispatch(kernel, bindings, [1, 1, 1]);
    engine.dispatch(kernel, { ...bindings, sampled: {} }, [1, 1, 1]);
    expect(inner.dispatched.map((d) => d.kernel)).toEqual([
      "BODY DISC CELLS",
      "BODY DISC CELLS (airless)",
      "BODY DISC CELLS",
    ]);
  });

  it("releases a kernel's twins with it, one made after its release at once", async () => {
    const { inner, engine } = await setUp();
    const released: string[] = [];
    const release = inner.releaseCompute.bind(inner);
    inner.releaseCompute = (kernel) => {
      released.push(kernel.name);
      release(kernel);
    };
    const airless = createAirlessTables(engine, "bodies:no atmosphere");
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    engine.dispatch(kernel, sampled(airless.body, airless.body), [1, 1, 1]);
    engine.dispatch(kernel, sampled(oneBand, oneBand), [1, 1, 1]);
    await engine.twinsSettled();
    engine.releaseCompute(kernel);
    const withIt = [...released];
    // A twin still being made as its kernel goes, in its task (perf-long-tasks-4), is released
    // once it is made: here its kernel goes as its making begins.
    const late = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const make = inner.createComputeAsync.bind(inner);
    inner.createComputeAsync = (pair) => {
      engine.releaseCompute(late);
      return make(pair);
    };
    engine.dispatch(late, sampled(airless.body, airless.body), [1, 1, 1]);
    await engine.twinsSettled();
    expect({ withIt, late: released.slice(withIt.length) }).toEqual({
      withIt: ["BODY DISC CELLS (table only)", "BODY DISC CELLS (airless)", "BODY DISC CELLS"],
      late: ["BODY DISC CELLS", "BODY DISC CELLS (airless)"],
    });
  });

  it("releases both twins with their material", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const before = inner.live.materials;
    render(bodyDrawOf(material, mesh, createAirlessTables(engine, "bodies:no atmosphere").body));
    render(drawOf(material, mesh, tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME)));
    await engine.twinsSettled();
    const made = inner.live.materials;
    engine.releaseMaterial(material);
    expect([before, made, inner.live.materials]).toEqual([before, before + 2, before - 1]);
  });

  it("makes an airless twin in a task after the frame that first draws or dispatches it, never in that frame (perf-long-tasks-4)", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const airless = createAirlessTables(engine, "bodies:no atmosphere");
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const kernelsMade = vi.spyOn(inner, "createComputeAsync");
    const before = inner.live.materials;
    render(bodyDrawOf(material, mesh, airless.body));
    engine.dispatch(
      kernel,
      { uniforms: {}, buffers: {}, sampled: { atmosphere_body: airless.body }, storage: {} },
      [1, 1, 1],
    );
    expect([inner.live.materials, kernelsMade.mock.calls.length]).toEqual([before, 0]);
    await engine.twinsSettled();
    expect([inner.live.materials, kernelsMade.mock.calls.length]).toEqual([before + 1, 1]);
  });

  it("makes no airless twin of a material or kernel let go before its task (perf-long-tasks-4)", async () => {
    const { inner, engine, material, mesh, render } = await setUp();
    const airless = createAirlessTables(engine, "bodies:no atmosphere");
    const kernel = engine.createCompute(BODY_DISC_CELLS_KERNEL);
    const kernelsMade = vi.spyOn(inner, "createComputeAsync");
    const before = inner.live.materials;
    render(bodyDrawOf(material, mesh, airless.body));
    engine.dispatch(
      kernel,
      { uniforms: {}, buffers: {}, sampled: { atmosphere_body: airless.body }, storage: {} },
      [1, 1, 1],
    );
    engine.releaseMaterial(material);
    engine.releaseCompute(kernel);
    await engine.twinsSettled();
    expect([inner.live.materials, kernelsMade.mock.calls.length]).toEqual([before - 1, 0]);
  });

  it("makes both twins again for a material made again, as its callers make it after a restore", async () => {
    const { engine, mesh, render } = await setUp();
    const airless = createAirlessTables(engine, "bodies:no atmosphere").body;
    const oneBand = tableOf(engine, ONE_BAND_SOURCE_TABLE_NAME);
    const drawnByTwins = async (material: MaterialHandle): Promise<Array<string | undefined>> => {
      render(bodyDrawOf(material, mesh, airless));
      render(drawOf(material, mesh, oneBand));
      await engine.twinsSettled();
      return [render(bodyDrawOf(material, mesh, airless)), render(drawOf(material, mesh, oneBand))];
    };
    const first = engine.createMaterial(BODY_DISC_MATERIALS.interior);
    const before = await drawnByTwins(first);
    engine.releaseMaterial(first);
    const again = await drawnByTwins(engine.createMaterial(BODY_DISC_MATERIALS.interior));
    expect([before, again]).toEqual([
      ["bodies:disc (airless)", "bodies:disc (table only)"],
      ["bodies:disc (airless)", "bodies:disc (table only)"],
    ]);
  });
});

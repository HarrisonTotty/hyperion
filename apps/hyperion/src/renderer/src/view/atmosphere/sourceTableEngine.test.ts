import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { BODY_DISC_CELLS_KERNEL, BODY_DISC_MATERIALS } from "../bodies/draw";
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { DrawItem, MaterialHandle, MeshHandle, TextureHandle } from "../engine/types";
import { COMPOSITE_MATERIAL } from "./hillaire";
import { SourceTableEngine } from "./sourceTableEngine";
import {
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
});

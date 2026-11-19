import { describe, expect, it } from "vitest";

import { FakeAdapter, SWIFTSHADER_INFO } from "../test/fakeGpu";
import { FakeRenderEngine } from "../test/fakeRenderEngine";
import { WGSL_CATALOGUE } from "../view/engine/catalogue";
import type { MaterialHandle, WgslMaterialSpec } from "../view/engine/types";
import { QUALITY_SETTINGS } from "../view/quality/qualitySetting";
import { sessionMaterials, terrainMaterialOf } from "../view/terrain/materialSet";
import { causesOf, isCatalogued, type Refusal, watchedEngine } from "./ladder";
import { flatSpec } from "./harness";

/** A device's limits that fit either setting's own vertex path (`fittedTerrainLayout`). */
const ROOMY = {
  maxStorageBufferBindingSize: 2 ** 31 - 4,
  maxBufferSize: 2 ** 32,
  maxTextureDimension2D: 16_384,
} as const;

describe("the ladder's catalogue check (plan R12, R12.T7.a)", () => {
  it("finds a catalogued entry by its spec's name", () => {
    const [first] = WGSL_CATALOGUE;
    expect(first === undefined ? false : isCatalogued(first.spec.name)).toBe(true);
  });

  it("finds a terrain material's still form as its material's", () => {
    expect(isCatalogued("lit-still", new Set(["lit"]))).toBe(true);
    expect(isCatalogued("lit-still", new Set(["lit-still"]))).toBe(true);
  });

  it("finds no name the catalogue does not hold, still or not", () => {
    expect(isCatalogued("not catalogued", new Set(["lit"]))).toBe(false);
    expect(isCatalogued("not catalogued-still", new Set(["lit"]))).toBe(false);
    expect(isCatalogued("-still", new Set(["lit"]))).toBe(false);
  });

  it.each(QUALITY_SETTINGS)(
    "holds every material the terrain draws with on %s, still forms as their materials",
    (setting) => {
      const keys = sessionMaterials(setting, ROOMY);
      expect(keys.length).toBeGreaterThan(0);
      const names = keys.map(
        (key) =>
          terrainMaterialOf(key.kind, key.path, { depthClipControl: true }, key.still === true).spec
            .name,
      );
      expect(names.filter((name) => !isCatalogued(name))).toEqual([]);
    },
  );
});

/** The refusal a device gives, with the engine's own error over it, as `drawing.ts` throws it. */
const REFUSED = new Error("the pipeline of material refused could not be made", {
  cause: new Error("Depth stencil state is not present"),
});

/** An engine whose device refuses every material's pipelines, as SwiftShader refused the sea's. */
class RefusingEngine extends FakeRenderEngine {
  override createMaterialAsync(): Promise<MaterialHandle> {
    return Promise.reject(REFUSED);
  }
}

/** A watched refusing engine, and what it records. */
async function watched(): Promise<{
  readonly engine: ReturnType<typeof watchedEngine>;
  readonly refusals: Refusal[];
}> {
  const device = await new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] }).requestDevice();
  const refusals: Refusal[] = [];
  return { engine: watchedEngine(new RefusingEngine(device), refusals), refusals };
}

describe("the ladder's watch for a refused make (plan R12, R12.T7.a)", () => {
  const spec: WgslMaterialSpec = flatSpec("refused");

  it("passes the refusal on to the make's caller as it came", async () => {
    const { engine } = await watched();
    await expect(engine.createMaterialAsync(spec, ["rgba16float"])).rejects.toBe(REFUSED);
  });

  it("records the refusal under the made spec's name, with its causes", async () => {
    const { engine, refusals } = await watched();
    await engine.createMaterialAsync(spec, ["rgba16float"]).catch(() => undefined);
    expect(refusals).toEqual([
      {
        name: "refused",
        why: "Error: the pipeline of material refused could not be made <- Error: Depth stencil state is not present",
      },
    ]);
  });

  it("leaves a make it does not watch working, on the engine itself", async () => {
    const { engine, refusals } = await watched();
    expect(engine.createMaterial(spec)).toEqual({ kind: "material", name: "refused" });
    expect(engine.capabilities.subgroups).toBe(false);
    expect(refusals).toEqual([]);
  });

  it("reads a cause that is no error as its text", () => {
    expect(causesOf(new Error("refused", { cause: "lost" }))).toBe("Error: refused <- lost");
  });
});

import { describe, expect, it } from "vitest";

import { FakeTexture } from "../../../test/fakeGpu";
import type { TextureSpec } from "../memory";
import type { TextureHandle } from "../types";
import { chainSteps, pipelineKey, viewDimensionBinds } from "./drawing";
import { Intermediates } from "./intermediates";

describe("a post-process chain", () => {
  it("writes the output directly with one pass", () => {
    expect(chainSteps(1)).toEqual([{ input: 0, output: "output" }]);
  });

  it("ping-pongs between the intermediates, the last pass writing the output", () => {
    expect(chainSteps(3)).toEqual([
      { input: 0, output: 1 },
      { input: 1, output: 0 },
      { input: 0, output: "output" },
    ]);
  });
});

describe("a pipeline's key", () => {
  it("differs by mesh layout, colour format and depth", () => {
    const keys = new Set([
      pipelineKey("a", "rgba16float", true),
      pipelineKey("b", "rgba16float", true),
      pipelineKey("a", "bgra8unorm-srgb", true),
      pipelineKey("a", "rgba16float", false),
    ]);
    expect(keys.size).toBe(4);
  });
});

describe("a chain's intermediates", () => {
  it("are made at a size, kept while it holds, and remade, the old destroyed, when it changes", () => {
    const made: TextureSpec[] = [];
    const destroyed: string[] = [];
    const textures = new Map<TextureHandle, FakeTexture>();
    const intermediates = new Intermediates(
      {
        createTexture: (spec) => {
          made.push(spec);
          const handle: TextureHandle = { kind: "texture", name: `${spec.name} #${made.length}` };
          textures.set(
            handle,
            new FakeTexture({ size: spec.size, format: spec.format, usage: spec.usage }),
          );
          return handle;
        },
        gpuTextureOf: (handle) => {
          const texture = textures.get(handle);
          if (texture === undefined) {
            throw new Error(`no texture ${handle.name}`);
          }
          return texture;
        },
        destroyTexture: (handle) => {
          destroyed.push(handle.name);
        },
      },
      "cockpit",
      "render-targets",
    );
    const first = intermediates.at({ widthPx: 4, heightPx: 2 });
    expect(intermediates.at({ widthPx: 4, heightPx: 2 })).toEqual(first);
    expect(made.map(({ name, format, size }) => [name, format, size])).toEqual([
      ["cockpit:post-process 0", "rgba16float", [4, 2]],
      ["cockpit:post-process 1", "rgba16float", [4, 2]],
    ]);
    intermediates.at({ widthPx: 8, heightPx: 2 });
    expect(destroyed).toEqual(["cockpit:post-process 0 #1", "cockpit:post-process 1 #2"]);
    expect(made).toHaveLength(4);
    intermediates.release();
    expect(destroyed).toHaveLength(4);
  });
});

describe("a texture binding's view dimension", () => {
  it("binds a single-layer 2D texture where an array is declared, and nothing else across kinds", () => {
    expect(viewDimensionBinds("2d", "2d")).toBe(true);
    expect(viewDimensionBinds("2d-array", "2d-array")).toBe(true);
    expect(viewDimensionBinds("2d-array", "2d")).toBe(true);
    expect(viewDimensionBinds("2d", "2d-array")).toBe(false);
    expect(viewDimensionBinds("cube", "2d")).toBe(false);
    expect(viewDimensionBinds("3d", "2d-array")).toBe(false);
  });
});

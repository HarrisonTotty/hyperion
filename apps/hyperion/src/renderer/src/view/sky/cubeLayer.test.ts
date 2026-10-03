import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { CUBE_DISPLAY_MATERIAL, CUBE_HDR_MATERIAL, SkyCubeLayer } from "./cubeLayer";

const BAKED = {
  cube: { kind: "texture", name: "sky cube: view" },
  peak: { kind: "buffer", name: "sky cube: view peak", bytes: 4 },
  faceSizePx: 8,
  path: "gpu",
} as const;

describe("SkyCubeLayer", () => {
  it("draws the cube with its peak in the variant asked for, at the frame's exposure", async () => {
    const engine = await countingRenderEngine();
    const layer = new SkyCubeLayer(engine);
    const display = layer.draw(BAKED, "display", 0.25);
    const hdr = layer.draw(BAKED, "hdr", 0.25);
    expect([
      display.material.name,
      hdr.material.name,
      display.textures["stars"]?.name,
      display.storageBuffers?.["peak"]?.name,
      [...(display.uniforms["exposure"] ?? [])],
    ]).toEqual([
      CUBE_DISPLAY_MATERIAL.name,
      CUBE_HDR_MATERIAL.name,
      "sky cube: view",
      "sky cube: view peak",
      [0.25, 0, 0, 0],
    ]);
  });

  it("makes its materials again after a device loss's restore", async () => {
    const engine = await countingRenderEngine();
    const layer = new SkyCubeLayer(engine);
    const before = engine.counts.materials;
    engine.restore();
    expect(engine.counts.materials - before).toBe(2);
    layer.dispose();
  });

  it("tones the display variant by agxSprite and leaves the HDR variant linear", () => {
    expect(CUBE_DISPLAY_MATERIAL.fragmentWgsl).toContain("return agxSprite(rgb);");
    expect(CUBE_HDR_MATERIAL.fragmentWgsl).not.toContain("return agxSprite(rgb);");
  });
});

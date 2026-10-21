import { describe, expect, it } from "vitest";

import { CASCADE_BYTES, CASCADE_TEXTURE } from "../terrain/shadows/cascades";
import {
  assertDepthLayerPasses,
  assertDepthLayersFit,
  assertMaterialSupported,
  depthLayersTexture,
} from "./depthLayers";
import { TEXTURE_USAGE } from "./gpuFlags";
import { textureBytes } from "./memory";
import {
  type DepthLayerPass,
  DepthSelfSample,
  type DrawItem,
  type TextureHandle,
  type WgslMaterialSpec,
} from "./types";

/** The cascades' set: four 2,048² layers under `shadows` (R10.T9.a's spec). */
const CASCADES = {
  name: "terrain shadow cascades",
  sizePx: 2048,
  layers: 4,
  category: "shadows",
} as const;

const DEPTH: TextureHandle = { kind: "texture", name: "terrain shadow cascades" };
const OTHER: TextureHandle = { kind: "texture", name: "albedo" };

/** A pass into `layer` drawing one draw that samples `textures`. */
function pass(
  layer: number,
  textures: Readonly<Record<string, TextureHandle>> = {},
): DepthLayerPass {
  const draw: DrawItem = {
    mesh: { kind: "mesh", name: "patch" },
    material: { kind: "material", name: "caster" },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures,
  };
  return {
    label: `shadow cascade ${layer}`,
    layer,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: [draw],
  };
}

const TARGET = { name: "terrain shadow cascades", layers: 4, depth: DEPTH };

describe("a depth-layer target's texture", () => {
  it("is the cascades' spec: four 2,048² depth32float layers, 67,108,864 B under shadows", () => {
    const spec = depthLayersTexture(CASCADES);
    expect(spec).toEqual({ ...CASCADE_TEXTURE, category: "shadows" });
    expect(textureBytes(spec)).toBe(CASCADE_BYTES);
  });

  it("is attached and sampled, and nothing else", () => {
    expect(depthLayersTexture(CASCADES).usage).toBe(
      TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.TEXTURE_BINDING,
    );
  });
});

describe("a depth-layer target's fit", () => {
  const limits = { maxTextureDimension2D: 8192 };

  it("takes the cascades' set", () => {
    expect(() => {
      assertDepthLayersFit(CASCADES, limits);
    }).not.toThrow();
  });

  it("refuses a size beyond the device's, or not whole, naming the target", () => {
    for (const sizePx of [0, 8193, 1.5]) {
      expect(() => {
        assertDepthLayersFit({ ...CASCADES, sizePx }, limits);
      }).toThrow(
        /depth layers terrain shadow cascades of .* px a side: the device takes 1 to 8192/u,
      );
    }
  });

  it("refuses no layers, or more than 256", () => {
    for (const layers of [0, 257, 2.5]) {
      expect(() => {
        assertDepthLayersFit({ ...CASCADES, layers }, limits);
      }).toThrow(/the device takes 1 to 256/u);
    }
  });
});

describe("a depth-layer render's passes", () => {
  it("take each layer once, in any order", () => {
    expect(() => {
      assertDepthLayerPasses(TARGET, [pass(3), pass(0), pass(1)]);
    }).not.toThrow();
  });

  it("refuse a layer outside the target, naming it", () => {
    for (const layer of [-1, 4, 0.5]) {
      expect(() => {
        assertDepthLayerPasses(TARGET, [pass(layer)]);
      }).toThrow(/draws into layer .* of terrain shadow cascades, which has layers 0 to 3/u);
    }
  });

  it("refuse a layer named twice, whose second clear would lose the first's draws", () => {
    expect(() => {
      assertDepthLayerPasses(TARGET, [pass(1), pass(2), pass(1)]);
    }).toThrow("pass shadow cascade 1 draws into layer 1 of terrain shadow cascades again");
  });

  it("refuse a draw that samples the target's own depth", () => {
    expect(() => {
      assertDepthLayerPasses(TARGET, [pass(0, { albedo: OTHER }), pass(1, { cascades: DEPTH })]);
    }).toThrow(DepthSelfSample);
  });
});

describe("a material's unclippedDepth", () => {
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

  it("is refused without depth-clip-control, naming the material and the feature", () => {
    expect(() => {
      assertMaterialSupported(CASTER, { depthClipControl: false });
    }).toThrow(
      "material terrain caster asks for unclippedDepth, which needs depth-clip-control, which the device lacks",
    );
  });

  it("is taken with depth-clip-control, and a material without it anywhere", () => {
    expect(() => {
      assertMaterialSupported(CASTER, { depthClipControl: true });
      assertMaterialSupported({ ...CASTER, unclippedDepth: false }, { depthClipControl: false });
    }).not.toThrow();
  });
});

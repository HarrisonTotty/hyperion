import { describe, expect, it } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import { TEXTURE_USAGE } from "../gpuFlags";
import type { AllocationEvent } from "../memory";
import { summariseAdapter } from "../platform";
import {
  ColourSelfSample,
  DepthSelfSample,
  type DrawItem,
  type RenderTargetSpec,
  type TextureHandle,
} from "../types";
import { ResourceRegistry } from "./resources";
import {
  assertNoColourSelfSample,
  assertNoDepthSelfSample,
  assertTargetFits,
  colourSpec,
  depthSpec,
} from "./target";

const SPEC: RenderTargetSpec = {
  name: "hdr",
  size: { widthPx: 64, heightPx: 32 },
  format: "rgba16float",
  mips: 3,
  depth: true,
  category: "render-targets",
};

describe("a target's textures", () => {
  it("are raised as render-targets allocations, with their bytes", async () => {
    const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
    const device = await adapter.requestDevice();
    const events: AllocationEvent[] = [];
    const resources = new ResourceRegistry(device, (event) => events.push(event));
    const colour = resources.createTexture(colourSpec(SPEC, SPEC.size));
    resources.createTexture(depthSpec(SPEC, SPEC.size));
    resources.destroyTexture(colour);
    expect(events).toEqual([
      {
        kind: "created",
        name: "hdr:colour",
        bytes: (64 * 32 + 32 * 16 + 16 * 8) * 8,
        category: "render-targets",
      },
      { kind: "created", name: "hdr:depth", bytes: 64 * 32 * 4, category: "render-targets" },
      {
        kind: "destroyed",
        name: "hdr:colour",
        bytes: (64 * 32 + 32 * 16 + 16 * 8) * 8,
        category: "render-targets",
      },
    ]);
  });

  it("give the depth COPY_SRC and TEXTURE_BINDING, so it can be read and sampled", () => {
    const depth = depthSpec(SPEC, SPEC.size);
    expect(depth.format).toBe("depth32float");
    expect(depth.usage & TEXTURE_USAGE.COPY_SRC).not.toBe(0);
    expect(depth.usage & TEXTURE_USAGE.TEXTURE_BINDING).not.toBe(0);
  });

  it("give the colour its sampled mips", () => {
    expect(colourSpec(SPEC, SPEC.size)).toMatchObject({ format: "rgba16float", mips: 3 });
  });
});

describe("a target's fit", () => {
  const caps = summariseAdapter(
    new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
  ).capabilities;

  it("refuses rg11b10ufloat on a device that cannot render to it", () => {
    expect(() => {
      assertTargetFits({ ...SPEC, format: "rg11b10ufloat" }, SPEC.size, caps);
    }).toThrow(/rg11b10ufloat/u);
  });

  it("refuses more mips than the size has", () => {
    expect(() => {
      assertTargetFits({ ...SPEC, mips: 3 }, { widthPx: 2, heightPx: 2 }, caps);
    }).toThrow(/asks for 3 mips; 2 × 2 has 2/u);
  });

  it("accepts the full chain", () => {
    expect(() => {
      assertTargetFits({ ...SPEC, mips: 7 }, SPEC.size, caps);
    }).not.toThrow();
  });
});

/** A full-screen draw of the aerial material binding `textures`. */
function draw(textures: Readonly<Record<string, TextureHandle>>): DrawItem {
  return {
    mesh: { kind: "mesh", name: "fullscreen" },
    material: { kind: "material", name: "aerial" },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures,
  };
}

describe("a draw sampling a target's depth", () => {
  const depth: TextureHandle = Object.freeze({ kind: "texture", name: "terrain:depth" });
  const other: TextureHandle = Object.freeze({ kind: "texture", name: "lut" });

  it("is refused into the same target, naming the target and the material", () => {
    const attempt = (): void => {
      assertNoDepthSelfSample("terrain", depth, [draw({ lut: other }), draw({ scene: depth })]);
    };
    expect(attempt).toThrow(DepthSelfSample);
    expect(attempt).toThrow("material aerial samples the depth of terrain");
  });

  it("passes into another target, and where the target has no depth", () => {
    expect(() => {
      assertNoDepthSelfSample("sky", null, [draw({ scene: depth })]);
    }).not.toThrow();
    expect(() => {
      assertNoDepthSelfSample("terrain", depth, [draw({ lut: other })]);
    }).not.toThrow();
  });
});

describe("a draw or a post-process sampling a target's colour", () => {
  const colour: TextureHandle = Object.freeze({ kind: "texture", name: "bloom:colour" });
  const other: TextureHandle = Object.freeze({ kind: "texture", name: "lut" });
  const bloom = { kind: "post-process", name: "bloom" } as const;

  it("is refused in a draw into the same target, naming the target and the material", () => {
    const attempt = (): void => {
      assertNoColourSelfSample("bloom", colour, {
        draws: [draw({ lut: other }), draw({ scene: colour })],
        postProcesses: [],
      });
    };
    expect(attempt).toThrow(ColourSelfSample);
    expect(attempt).toThrow("material aerial samples the colour of bloom");
  });

  it("is refused in a post-process of the same target, naming it", () => {
    expect(() => {
      assertNoColourSelfSample("bloom", colour, {
        draws: [],
        postProcesses: [{ postProcess: bloom, uniforms: {}, textures: { level: colour } }],
      });
    }).toThrow("post-process bloom samples the colour of bloom");
  });

  it("passes where the sampling pass draws into an intermediate, not the target", () => {
    const blur = { kind: "post-process", name: "blur" } as const;
    expect(() => {
      assertNoColourSelfSample("bloom", colour, {
        draws: [draw({ previous: colour })],
        postProcesses: [
          { postProcess: blur, uniforms: {}, textures: { previous: colour } },
          { postProcess: bloom, uniforms: {} },
        ],
      });
    }).not.toThrow();
  });

  it("passes when nothing samples it", () => {
    expect(() => {
      assertNoColourSelfSample("bloom", colour, {
        draws: [draw({ lut: other })],
        postProcesses: [{ postProcess: bloom, uniforms: {} }],
      });
    }).not.toThrow();
  });
});

import { describe, expect, it, vi } from "vitest";

import { FakeBuffer } from "../../../test/fakeGpu";
import type { DrawItem } from "../types";
import { BLEND_STATES } from "./materials";
import {
  attributeLocations,
  floatVertexFormat,
  packStruct,
  partitionDraws,
  rawDepthBias,
  type RawDraw,
  RawPipelines,
  rawPipelineDescriptor,
  usedBindings,
  structLayout,
} from "./rawPass";

const COMPILED = `
struct LeftOver {
  viewRotation : mat4x4f,
  clipProjection : mat4x4f,
  offsetFromCameraM : vec3f,
  tint : vec4<f32>,
  count : u32,
};
struct VertexInputs {
  @builtin(vertex_index) vertexIndex : u32,
  @builtin(instance_index) instanceIndex : u32,
@location(0) position : vec3f,
@location(1) segmentEnd : vec4f,
};`;

const DRAW: DrawItem = {
  mesh: { kind: "mesh", name: "m" },
  material: { kind: "material", name: "l" },
  offsetFromCameraM: new Float32Array(3),
  uniforms: {},
  textures: {},
};

describe("a frame's draws", () => {
  it("send an indirect draw to the raw pass and never to Babylon's", () => {
    const indirect = {
      ...DRAW,
      indirect: { buffer: { kind: "buffer", name: "a", bytes: 16 }, offsetBytes: 0 },
    } as const;
    const { babylon, raw } = partitionDraws([DRAW, indirect, DRAW]);
    expect(babylon).toEqual([DRAW, DRAW]);
    expect(raw).toEqual([indirect]);
  });
});

describe("Babylon's leftover block", () => {
  it("is laid out by WGSL's uniform rules", () => {
    expect(structLayout(COMPILED, "LeftOver")).toEqual({
      members: [
        { name: "viewRotation", type: "mat4x4f", offsetBytes: 0, sizeBytes: 64 },
        { name: "clipProjection", type: "mat4x4f", offsetBytes: 64, sizeBytes: 64 },
        { name: "offsetFromCameraM", type: "vec3f", offsetBytes: 128, sizeBytes: 12 },
        { name: "tint", type: "vec4f", offsetBytes: 144, sizeBytes: 16 },
        { name: "count", type: "u32", offsetBytes: 160, sizeBytes: 4 },
      ],
      sizeBytes: 176,
    });
  });

  it("is packed from values by name, an unsigned member as an integer", () => {
    const layout = structLayout(COMPILED, "LeftOver");
    const bytes = packStruct(
      layout,
      new Map([
        ["offsetFromCameraM", new Float32Array([1, 2, 3])],
        ["count", new Float32Array([7])],
      ]),
    );
    expect([...new Float32Array(bytes, 128, 3)]).toEqual([1, 2, 3]);
    expect(new Uint32Array(bytes, 160, 1)[0]).toBe(7);
  });

  it("is empty when the code declares none", () => {
    expect(structLayout("fn main() {}", "LeftOver")).toEqual({ members: [], sizeBytes: 0 });
  });
});

describe("a raw draw's vertex inputs", () => {
  it("are found at the locations the compiled code gives them", () => {
    expect([...attributeLocations(COMPILED)]).toEqual([
      ["position", 0],
      ["segmentEnd", 1],
    ]);
  });

  it("take a float format by their components", () => {
    expect(floatVertexFormat(3)).toBe("float32x3");
    expect(() => floatVertexFormat(5)).toThrow(/5/u);
  });
});

describe("a raw pipeline's depth bias", () => {
  it("is negated, since away from the camera is smaller under reversed depth", () => {
    expect(rawDepthBias({ constant: 4, slopeScale: 1.5 }, "triangle-list")).toEqual({
      depthBias: -4,
      depthBiasSlopeScale: -1.5,
    });
  });

  it("is zero for lines, which take none", () => {
    expect(rawDepthBias({ constant: 4, slopeScale: 1.5 }, "line-list")).toEqual({
      depthBias: 0,
      depthBiasSlopeScale: 0,
    });
  });
});

describe("the bindings a raw draw binds", () => {
  it("are those its code uses, not a texture's unused automatic sampler", () => {
    const fragment = `
@group(1) @binding(0) var lut : texture_2d<f32>;
@group(1) @binding(1) var lutSampler : sampler;
@fragment fn main() -> @location(0) vec4f { return textureLoad(lut, vec2i(0, 0), 0); }`;
    expect([...usedBindings(["lut", "lutSampler"], "", fragment)]).toEqual(["lut"]);
  });
});

/** A raw draw of lines with one vertex buffer. */
const RAW: RawDraw = {
  name: "lines",
  vertexWgsl: "vertex",
  fragmentWgsl: "fragment",
  topology: "line-list",
  vertexBuffers: [
    {
      location: 0,
      buffer: new FakeBuffer({ size: 36, usage: 0 }),
      offsetBytes: 0,
      strideBytes: 12,
      format: "float32x3",
      instanced: false,
    },
  ],
  index: null,
  resources: new Map(),
  state: {
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: BLEND_STATES.additive,
    depthBias: { constant: 2, slopeScale: 1 },
  },
  indirect: { buffer: new FakeBuffer({ size: 16, usage: 0 }), offsetBytes: 0 },
};

const MODULE: GPUShaderModule = {
  label: "m",
  getCompilationInfo: () => Promise.reject(new Error("unused")),
};

describe("a raw pipeline", () => {
  it("loads Babylon's depth, tests greater-or-equal and keeps the material's state", () => {
    const descriptor = rawPipelineDescriptor(RAW, MODULE, MODULE, "rgba16float", true);
    expect(descriptor.depthStencil).toEqual({
      format: "depth32float",
      depthWriteEnabled: false,
      depthCompare: "greater-equal",
      depthBias: 0,
      depthBiasSlopeScale: 0,
    });
    expect(descriptor.fragment?.targets).toEqual([
      { format: "rgba16float", blend: BLEND_STATES.additive, writeMask: 0xf },
    ]);
    expect(descriptor.primitive).toEqual({
      topology: "line-list",
      cullMode: "none",
      frontFace: "ccw",
    });
  });

  it("is made once, asynchronously, and the draw waits for it", async () => {
    const made: GPURenderPipelineDescriptor[] = [];
    const pipeline: GPURenderPipeline = {
      label: "made",
      getBindGroupLayout: () => ({ label: "" }),
    };
    const pipelines = new RawPipelines({
      createShaderModule: () => MODULE,
      createRenderPipelineAsync: (descriptor: GPURenderPipelineDescriptor) => {
        made.push(descriptor);
        return Promise.resolve(pipeline);
      },
    });
    expect(pipelines.pipelineFor(RAW, "rgba16float", true)).toBeUndefined();
    await vi.waitFor(() => {
      expect(pipelines.pipelineFor(RAW, "rgba16float", true)).toBe(pipeline);
    });
    expect(made).toHaveLength(1);
  });
});

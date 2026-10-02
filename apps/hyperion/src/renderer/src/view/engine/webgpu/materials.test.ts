import { describe, expect, it } from "vitest";

import { POST_PROCESS_BINDINGS, type WgslMaterialSpec, type WgslPostProcessSpec } from "../types";
import {
  BLEND_STATES,
  depthBiasOf,
  materialBindings,
  materialPipelineDescriptor,
  postProcessBindings,
  postProcessPipelineDescriptor,
  resourceLayoutEntries,
  samplerDescriptor,
} from "./materials";

const SPEC: WgslMaterialSpec = {
  name: "hull",
  displayName: "TEST MATERIAL",
  vertexWgsl: "vertex",
  fragmentWgsl: "fragment",
  uniforms: [{ name: "tint", type: "vec4f" }],
  samplers: [{ name: "lutSampler", filter: "linear", address: "clamp-to-edge", binding: 1 }],
  textures: [
    { name: "lut", binding: 0, viewDimension: "3d" },
    { name: "sceneDepth", binding: 3, sampleType: "depth" },
  ],
  storageBuffers: [{ name: "instances", binding: 2 }],
  cullMode: "back",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
};

/** Stand-ins for the GPU objects a descriptor only carries. */
const MODULE: GPUShaderModule = {
  label: "module",
  getCompilationInfo: () => Promise.resolve({ messages: [] }),
};
const LAYOUT: GPUPipelineLayout = { label: "layout" };
const BUFFERS: ReadonlyArray<GPUVertexBufferLayout> = [
  { arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] },
];

describe("a material's pipeline", () => {
  it("tests reversed depth greater-or-equal, with the material's own state", () => {
    const descriptor = materialPipelineDescriptor(
      { ...SPEC, depthWrite: false, colourWrites: false, blend: "additive" },
      { vertex: MODULE, fragment: MODULE },
      LAYOUT,
      BUFFERS,
      "triangle-list",
      "rgba16float",
      true,
    );
    expect(descriptor.layout).toBe(LAYOUT);
    expect(descriptor.vertex.entryPoint).toBe("vertexMain");
    expect(descriptor.fragment?.entryPoint).toBe("fragmentMain");
    expect(descriptor.primitive).toEqual({
      topology: "triangle-list",
      cullMode: "back",
      frontFace: "ccw",
    });
    expect(descriptor.depthStencil).toEqual({
      format: "depth32float",
      depthWriteEnabled: false,
      depthCompare: "greater-equal",
      depthBias: 0,
      depthBiasSlopeScale: 0,
    });
    expect([...(descriptor.fragment?.targets ?? [])]).toEqual([
      { format: "rgba16float", blend: BLEND_STATES.additive, writeMask: 0 },
    ]);
  });

  it("has no depth state into a target without depth", () => {
    const descriptor = materialPipelineDescriptor(
      SPEC,
      { vertex: MODULE, fragment: MODULE },
      LAYOUT,
      BUFFERS,
      "line-list",
      "rgba8unorm",
      false,
    );
    expect(descriptor.depthStencil).toBeUndefined();
  });

  it("negates a bias away from the camera, and gives lines none", () => {
    const bias = { constant: 4, slopeScale: 1.5 };
    expect(depthBiasOf(bias, "triangle-list")).toEqual({
      depthBias: -4,
      depthBiasSlopeScale: -1.5,
    });
    expect(depthBiasOf(bias, "line-list")).toEqual({ depthBias: 0, depthBiasSlopeScale: 0 });
  });
});

describe("a material's resources", () => {
  it("are laid out at their declared bindings, in both stages", () => {
    expect(resourceLayoutEntries(materialBindings(SPEC))).toEqual([
      { binding: 0, visibility: 3, texture: { sampleType: "float", viewDimension: "3d" } },
      { binding: 1, visibility: 3, sampler: { type: "filtering" } },
      { binding: 2, visibility: 3, buffer: { type: "read-only-storage" } },
      { binding: 3, visibility: 3, texture: { sampleType: "depth", viewDimension: "2d" } },
    ]);
  });

  it("refuse two resources at one binding, naming the material", () => {
    expect(() =>
      resourceLayoutEntries(
        materialBindings({ ...SPEC, storageBuffers: [{ name: "instances", binding: 1 }] }),
      ),
    ).toThrow("material hull declares @group(2) @binding(1) twice");
  });

  it("take a nearest sampler as non-filtering, and filter between mips too", () => {
    const nearest = { name: "s", filter: "nearest", address: "repeat", binding: 0 } as const;
    expect(
      resourceLayoutEntries({ owner: "m", textures: [], samplers: [nearest], storageBuffers: [] }),
    ).toEqual([{ binding: 0, visibility: 3, sampler: { type: "non-filtering" } }]);
    expect(samplerDescriptor(nearest)).toMatchObject({
      magFilter: "nearest",
      minFilter: "nearest",
      mipmapFilter: "nearest",
      addressModeU: "repeat",
      addressModeW: "repeat",
    });
  });
});

describe("a post-process", () => {
  const POST: WgslPostProcessSpec = {
    name: "tonemap",
    displayName: "TEST TONE MAP",
    fragmentWgsl: "fragment",
    uniforms: [],
    textures: [{ name: "bloom", binding: 2 }],
  };

  it("reads its input colour at the fixed bindings, then its own textures", () => {
    const bindings = postProcessBindings(POST);
    expect(bindings.textures.map(({ name, binding }) => [name, binding])).toEqual([
      ["hdr-colour", POST_PROCESS_BINDINGS["hdr-colour"]],
      ["bloom", 2],
    ]);
    expect(bindings.samplers.map(({ binding }) => binding)).toEqual([
      POST_PROCESS_BINDINGS["hdr-colour-sampler"],
    ]);
  });

  it("is a full-screen triangle list with no depth", () => {
    const descriptor = postProcessPipelineDescriptor(
      POST,
      MODULE,
      MODULE,
      LAYOUT,
      "bgra8unorm-srgb",
    );
    expect(descriptor.primitive).toEqual({ topology: "triangle-list", cullMode: "none" });
    expect(descriptor.depthStencil).toBeUndefined();
    expect(descriptor.fragment?.entryPoint).toBe("fragmentMain");
  });
});

describe("the blend table", () => {
  it("keeps the destination alpha in every blending mode: (zero, one)", () => {
    for (const mode of ["additive", "premultiplied"] as const) {
      expect(BLEND_STATES[mode]?.alpha).toEqual({
        srcFactor: "zero",
        dstFactor: "one",
        operation: "add",
      });
    }
    expect(BLEND_STATES.none).toBeUndefined();
  });

  it("adds source-alpha-weighted colour, or composites premultiplied colour", () => {
    expect(BLEND_STATES.additive?.color).toEqual({
      srcFactor: "src-alpha",
      dstFactor: "one",
      operation: "add",
    });
    expect(BLEND_STATES.premultiplied?.color).toEqual({
      srcFactor: "one",
      dstFactor: "one-minus-src-alpha",
      operation: "add",
    });
  });
});

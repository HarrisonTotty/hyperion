import { describe, expect, it } from "vitest";

import { FakeAdapter, FakeBuffer, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import type { KernelPair } from "../kernels";
import { summariseAdapter } from "../platform";
import {
  type BoundResource,
  kernelBindGroupEntries,
  kernelBindings,
  prepareKernel,
} from "./compute";

const PAIR: KernelPair = {
  name: "sum",
  reference: [
    "@group(0) @binding(0) var<storage, read> values : array<u32>;",
    "@group(0) @binding(1) var<storage, read_write> total : array<u32>;",
    "@group(1)@binding(0) var<uniform> params : Params;",
    "// @group(2) @binding(0) var ghost : texture_2d<f32>;",
    "@group(2) @binding(1) var table : texture_3d<f32>;",
    "@group(2) @binding(2) var target : texture_storage_2d<rgba16float, write>;",
    "@group(2) @binding(3) var linear : sampler;",
    "@group(2) @binding(4) var lookup : texture_storage_2d<r32float, read>;",
  ].join("\n"),
  subgroup: "enable subgroups;\n@group(0) @binding(0) var<storage, read> values : array<u32>;",
  readback: "bit-exact",
};

function capabilities(
  features: ReadonlyArray<GPUFeatureName>,
): ReturnType<typeof summariseAdapter>["capabilities"] {
  return summariseAdapter(new FakeAdapter({ info: INTEL_UHD_620_INFO, features })).capabilities;
}

describe("a kernel's bindings", () => {
  it("are read by name from the source's declarations", () => {
    const bindings = kernelBindings(PAIR.reference);
    expect(Object.fromEntries(bindings)).toStrictEqual({
      values: { group: 0, binding: 0, kind: "storage", writable: false, viewDimension: undefined },
      total: { group: 0, binding: 1, kind: "storage", writable: true, viewDimension: undefined },
      params: { group: 1, binding: 0, kind: "uniform", writable: false, viewDimension: undefined },
      table: { group: 2, binding: 1, kind: "texture", writable: false, viewDimension: "3d" },
      target: {
        group: 2,
        binding: 2,
        kind: "storage-texture",
        writable: true,
        viewDimension: "2d",
      },
      linear: { group: 2, binding: 3, kind: "sampler", writable: false, viewDimension: undefined },
      lookup: {
        group: 2,
        binding: 4,
        kind: "storage-texture",
        writable: false,
        viewDimension: "2d",
      },
    });
  });

  it("report each texture's declared view dimension, an array's as 2d-array (R08.T0)", () => {
    const declared = [
      ["a", "texture_1d<f32>", "1d"],
      ["b", "texture_storage_1d<r32float, write>", "1d"],
      ["c", "texture_2d<f32>", "2d"],
      ["d", "texture_depth_2d", "2d"],
      ["e", "texture_multisampled_2d<f32>", "2d"],
      ["f", "texture_depth_multisampled_2d", "2d"],
      ["g", "texture_storage_2d<rgba16float, write>", "2d"],
      ["h", "texture_2d_array<f32>", "2d-array"],
      ["i", "texture_depth_2d_array", "2d-array"],
      ["j", "texture_storage_2d_array<rgba16float, write>", "2d-array"],
      ["k", "texture_3d<u32>", "3d"],
      ["l", "texture_storage_3d<rgba16float, read_write>", "3d"],
      ["m", "texture_cube<f32>", "cube"],
      ["n", "texture_depth_cube", "cube"],
      ["o", "texture_cube_array<f32>", "cube-array"],
      ["p", "texture_depth_cube_array", "cube-array"],
      ["q", "sampler_comparison", undefined],
    ] as const;
    const bindings = kernelBindings(
      declared
        .map(([name, type], index) => `@group(0) @binding(${index}) var ${name} : ${type};`)
        .join("\n"),
    );
    expect(
      Object.fromEntries([...bindings].map(([name, b]) => [name, b.viewDimension])),
    ).toStrictEqual(Object.fromEntries(declared.map(([name, , dimension]) => [name, dimension])));
  });
});

describe("a dispatch's bind groups", () => {
  const kernel = { pair: PAIR, bindings: kernelBindings(PAIR.reference) };
  const buffer: BoundResource = { kind: "buffer", buffer: new FakeBuffer({ size: 4, usage: 0 }) };

  it("refuse a declared binding given nothing, naming it", () => {
    expect(() => kernelBindGroupEntries(kernel, new Map([["values", buffer]]))).toThrow(
      /sum's binding total/u,
    );
  });

  it("refuse a resource for a name the kernel does not declare", () => {
    expect(() => kernelBindGroupEntries(kernel, new Map([["ghost", buffer]]))).toThrow(
      /declares no binding ghost/u,
    );
  });

  it("gather each group's entries in group order", () => {
    const small = {
      pair: PAIR,
      bindings: kernelBindings(
        "@group(1) @binding(0) var<uniform> b : vec4f;\n@group(0) @binding(3) var<storage, read> a : array<u32>;",
      ),
    };
    const groups = kernelBindGroupEntries(
      small,
      new Map([
        ["a", buffer],
        ["b", buffer],
      ]),
    );
    expect([...groups.keys()]).toEqual([0, 1]);
    expect(groups.get(0)?.map(({ binding }) => binding)).toEqual([3]);
  });
});

describe("a kernel's variant", () => {
  it("is the subgroup twin only when the device has subgroups", () => {
    expect(prepareKernel(PAIR, capabilities(["subgroups"])).path).toBe("subgroup");
    expect(prepareKernel(PAIR, capabilities([])).path).toBe("reference");
  });

  it("is refused when it enables both f16 and subgroups", () => {
    const both = { ...PAIR, subgroup: "enable f16, subgroups;" };
    expect(() => prepareKernel(both, capabilities(["subgroups"]))).toThrow(/sum/u);
  });
});

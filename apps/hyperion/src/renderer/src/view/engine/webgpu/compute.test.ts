import { describe, expect, it } from "vitest";

import {
  FakeAdapter,
  FakeBuffer,
  type FakeDevice,
  INTEL_UHD_620_INFO,
} from "../../../test/fakeGpu";
import type { KernelPair } from "../kernels";
import { summariseAdapter } from "../platform";
import {
  type BoundResource,
  createKernel,
  kernelBindGroupEntries,
  kernelBindings,
  kernelKey,
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

describe("a kernel's key (R01.T15)", () => {
  const spec = prepareKernel(PAIR, capabilities([]));
  const key = kernelKey(PAIR, spec);

  it("is the same for a kernel of the same name, sources and readback in another object", () => {
    const copy: KernelPair = { ...PAIR, reference: [PAIR.reference].join("") };
    expect(kernelKey(copy, prepareKernel(copy, capabilities([])))).toBe(key);
  });

  it("differs where the name, either source or the readback class differs alone", () => {
    const others: ReadonlyArray<KernelPair> = [
      { ...PAIR, name: "sum again" },
      { ...PAIR, reference: `${PAIR.reference}\n` },
      { ...PAIR, subgroup: null },
      { ...PAIR, readback: "presentation-only" },
    ];
    const keys = others.map((other) => kernelKey(other, prepareKernel(other, capabilities([]))));
    expect(new Set([key, ...keys]).size).toBe(1 + others.length);
  });

  it("differs where either source differs alone on the subgroup path too, the compiled one or the other", () => {
    const subgroups = capabilities(["subgroups"]);
    const pairs: ReadonlyArray<KernelPair> = [
      PAIR,
      { ...PAIR, reference: `${PAIR.reference}\n` },
      { ...PAIR, subgroup: `${PAIR.subgroup ?? ""}\n` },
    ];
    const keys = pairs.map((pair) => kernelKey(pair, prepareKernel(pair, subgroups)));
    expect(new Set(keys).size).toBe(3);
  });

  it("differs where the module's code or label alone differs, for a spec not of the pair's sources", () => {
    const keys = [
      kernelKey(PAIR, { ...spec, module: { ...spec.module, code: `${PAIR.reference}\n` } }),
      kernelKey(PAIR, { ...spec, module: { ...spec.module, label: "other" } }),
    ];
    expect(new Set([key, ...keys]).size).toBe(3);
  });

  it("differs where the entry point alone differs, or one constant's value or name", () => {
    const keys = [
      kernelKey(PAIR, { ...spec, entryPoint: "other" }),
      kernelKey(PAIR, { ...spec, constants: { count: 1 } }),
      kernelKey(PAIR, { ...spec, constants: { count: 2 } }),
      kernelKey(PAIR, { ...spec, constants: { total: 1 } }),
    ];
    expect(new Set([key, ...keys]).size).toBe(5);
  });

  it("holds an override of -0 apart from one of 0, and takes constants in any order", () => {
    expect(kernelKey(PAIR, { ...spec, constants: { scale: -0 } })).not.toBe(
      kernelKey(PAIR, { ...spec, constants: { scale: 0 } }),
    );
    expect(kernelKey(PAIR, { ...spec, constants: { a: 1, b: 2 } })).toBe(
      kernelKey(PAIR, { ...spec, constants: { b: 2, a: 1 } }),
    );
  });
});

/** A fake device with no optional features. */
async function fakeDevice(): Promise<FakeDevice> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const made = adapter.devices.at(0);
  if (made === undefined) {
    throw new Error("the fake adapter made no device");
  }
  return made;
}

describe("a kernel's pipeline (R01.T15)", () => {
  it("is compiled from its spec: `main` under `auto`, with no constants where it has none", async () => {
    const gpu = await fakeDevice();
    createKernel(gpu, PAIR, prepareKernel(PAIR, capabilities([])));
    const compute = gpu.computePipelines.at(0)?.descriptor.compute;
    expect([gpu.computePipelines.at(0)?.descriptor.layout, compute?.entryPoint]).toEqual([
      "auto",
      "main",
    ]);
    expect(compute !== undefined && "constants" in compute).toBe(false);
  });

  it("takes the spec's entry point and constants", async () => {
    const gpu = await fakeDevice();
    const spec = { ...prepareKernel(PAIR, capabilities([])), entryPoint: "other" };
    createKernel(gpu, PAIR, { ...spec, constants: { count: 3 } });
    const compute = gpu.computePipelines.at(0)?.descriptor.compute;
    expect([compute?.entryPoint, compute?.constants]).toEqual(["other", { count: 3 }]);
  });
});

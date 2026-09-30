import { describe, expect, it } from "vitest";

import { assertNoF16Subgroups, highamBound, type KernelPair, selectKernel } from "./kernels";
import type { GpuCapabilities } from "./platform";

const CAPABILITIES: GpuCapabilities = {
  subgroups: false,
  shaderF16: false,
  timestampQuery: false,
  float32Filterable: false,
  float32Blendable: false,
  rg11b10Renderable: false,
  depthClipControl: false,
  maxTextureDimension2D: 8192,
  subgroupMinSize: null,
};

const WITH_SUBGROUPS: GpuCapabilities = { ...CAPABILITIES, subgroups: true, subgroupMinSize: 4 };

const PAIR: KernelPair = {
  name: "toy-sum",
  reference: "// reference",
  subgroup: "enable subgroups;\n// subgroup",
  readback: "bit-exact",
};

const REFERENCE_ONLY: KernelPair = { ...PAIR, subgroup: null };

describe("kernel selection", () => {
  it("takes the subgroup variant with the feature", () => {
    expect(selectKernel(PAIR, WITH_SUBGROUPS)).toEqual({ path: "subgroup", wgsl: PAIR.subgroup });
  });

  it("takes the reference without the feature", () => {
    expect(selectKernel(PAIR, CAPABILITIES)).toEqual({ path: "reference", wgsl: PAIR.reference });
  });

  it("always takes the reference of a pair without a subgroup variant", () => {
    for (const capabilities of [CAPABILITIES, WITH_SUBGROUPS]) {
      expect(selectKernel(REFERENCE_ONLY, capabilities)).toEqual({
        path: "reference",
        wgsl: REFERENCE_ONLY.reference,
      });
    }
  });
});

describe("the f16 and subgroups guard", () => {
  it("throws with the kernel's name for a module enabling both", () => {
    expect(() => {
      assertNoF16Subgroups("mixed", "enable f16;\nenable subgroups;\n");
    }).toThrow("mixed");
  });

  it("throws for both in one directive", () => {
    expect(() => {
      assertNoF16Subgroups("mixed", "enable f16, subgroups;\n");
    }).toThrow("mixed");
  });

  it("passes a module enabling one of them", () => {
    expect(() => {
      assertNoF16Subgroups("f16-only", "enable f16;\n");
      assertNoF16Subgroups("subgroups-only", "enable subgroups;\n// f16 in a comment\n");
    }).not.toThrow();
  });
});

describe("the Higham bound", () => {
  it("is γ(n − 1) · Σ|x| with u = 2⁻²⁴", () => {
    // n = 1025: (n − 1)u = 1024 · 2⁻²⁴ = 2⁻¹⁴, and γ = 2⁻¹⁴ ÷ (1 − 2⁻¹⁴) = 1 ÷ 16383.
    expect(highamBound(1025, 16_383)).toBeCloseTo(1, 12);
  });

  it("is zero for a single term", () => {
    expect(highamBound(1, 5)).toBe(0);
  });

  it("refuses a count that is not a positive integer", () => {
    expect(() => highamBound(0, 1)).toThrow("at least one term");
    expect(() => highamBound(2.5, 1)).toThrow("at least one term");
  });
});

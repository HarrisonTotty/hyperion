import { describe, expect, it } from "vitest";

import { assertNoF16Subgroups, highamBound, type KernelPair, selectKernel } from "./kernels";
import type { GpuCapabilities } from "./platform";
import {
  SUM_F32,
  SUM_F32_COUNT,
  SUM_U32,
  SUM_U32_COUNT,
  SUM_U32_RAGGED,
  SUM_U32_RAGGED_COUNT,
  SUM_U32_RAGGED_WORKGROUP,
  sumF32Inputs,
  sumF64,
  sumU32Inputs,
  wrappingSumU32,
} from "./twins";

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
  maxStorageBufferBindingSize: 134_217_728,
  maxBufferSize: 268_435_456,
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

  it("throws for both on one line", () => {
    expect(() => {
      assertNoF16Subgroups("same-line", "enable f16; enable subgroups;\n");
    }).toThrow("same-line");
  });

  it("throws for both after a requires directive and with a comment inside one", () => {
    expect(() => {
      assertNoF16Subgroups(
        "commented",
        "requires readonly_and_readwrite_storage_textures; enable subgroups;\nenable /* c */ f16;\n",
      );
    }).toThrow("commented");
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

  it("refuses a count at which the bound fails", () => {
    expect(() => highamBound(2 ** 24 + 1, 1)).toThrow("does not hold");
  });

  it("refuses a count that is not a positive integer", () => {
    expect(() => highamBound(0, 1)).toThrow("at least one term");
    expect(() => highamBound(2.5, 1)).toThrow("at least one term");
  });
});

describe("the subgroup twins' CPU references", () => {
  it("wrap a u32 sum modulo 2³², as Rust's wrapping_add does", () => {
    expect(wrappingSumU32([0xffff_ffff, 2])).toBe(1);
    expect(wrappingSumU32([])).toBe(0);
  });

  it("give inputs that wrap the u32 sum past 2³² more than once", () => {
    const inputs = sumU32Inputs(SUM_U32_COUNT);
    const exact = inputs.reduce((sum, value) => sum + value, 0);
    expect(exact).toBeGreaterThan(2 * 2 ** 32);
    expect(wrappingSumU32(inputs)).toBe(exact % 2 ** 32);
  });

  it("leave the ragged count and workgroup indivisible by any subgroup size", () => {
    for (const size of [4, 8, 16, 32, 64, 128]) {
      expect(SUM_U32_RAGGED_WORKGROUP % size).not.toBe(0);
      expect(SUM_U32_RAGGED_COUNT % size).not.toBe(0);
    }
  });

  it("give finite, normal f32 inputs, and their f64 sum and absolute sum", () => {
    const inputs = sumF32Inputs(SUM_F32_COUNT);
    expect(inputs.every((value) => value >= 1 && value < 2)).toBe(true);
    expect(sumF64([1.5, -0.25])).toEqual({ sum: 1.25, sumAbs: 1.75 });
  });

  it("declare which pair may reach the CPU, and read subgroup_size", () => {
    expect([SUM_U32.readback, SUM_U32_RAGGED.readback, SUM_F32.readback]).toEqual([
      "bit-exact",
      "bit-exact",
      "presentation-only",
    ]);
    expect(SUM_U32.subgroup).toContain("@builtin(subgroup_size)");
    expect(() => {
      for (const pair of [SUM_U32, SUM_U32_RAGGED, SUM_F32]) {
        assertNoF16Subgroups(pair.name, pair.subgroup ?? "");
      }
    }).not.toThrow();
  });
});

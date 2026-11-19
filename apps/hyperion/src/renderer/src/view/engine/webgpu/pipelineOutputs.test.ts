import { describe, expect, it } from "vitest";

import { OCEAN_DEPTH_COPY_MATERIAL, OCEAN_NEAR_PASS_MATERIAL } from "../../ocean/oceanNearPass";
import { OCEAN_FAR_PASS_MATERIAL } from "../../ocean/oceanPass";
import { TONEMAP_MATERIAL } from "../../post/tonemap";
import { materialOutputs, pipelineOutputs, writesFragDepth } from "./pipelineOutputs";

describe("a material's depth output (plan R12, R12.T7.a's ladder)", () => {
  it("reads frag_depth in a fragment's outputs, spaces and all", () => {
    expect(writesFragDepth({ fragmentWgsl: "struct O { @builtin(frag_depth) d : f32 }" })).toBe(
      true,
    );
    expect(writesFragDepth({ fragmentWgsl: "@builtin( frag_depth ) d : f32" })).toBe(true);
  });

  it("reads none in a comment, or where only another builtin is written", () => {
    expect(
      writesFragDepth({ fragmentWgsl: "// @builtin(frag_depth)\n/* @builtin(frag_depth) */" }),
    ).toBe(false);
    expect(writesFragDepth({ fragmentWgsl: "@builtin(position) p : vec4f" })).toBe(false);
  });

  it("finds R11's seas writing it, and the tone curve and the depth copy not", () => {
    expect(
      [
        OCEAN_FAR_PASS_MATERIAL,
        OCEAN_NEAR_PASS_MATERIAL,
        OCEAN_DEPTH_COPY_MATERIAL,
        TONEMAP_MATERIAL,
      ].map(writesFragDepth),
    ).toEqual([true, true, false, false]);
  });

  it("makes a bare colour format's form with depth alone for a material writing it", () => {
    expect(materialOutputs(OCEAN_FAR_PASS_MATERIAL, ["rgba16float"], "bgra8unorm")).toEqual([
      ["rgba16float", true],
    ]);
  });

  it("makes both forms of a bare colour format for a material that writes no depth", () => {
    expect(materialOutputs(TONEMAP_MATERIAL, ["rgba16float"], "bgra8unorm")).toEqual(
      pipelineOutputs("rgba16float", "bgra8unorm"),
    );
  });

  it("refuses a target without depth alone for a material writing it, naming both", () => {
    expect(() =>
      materialOutputs(
        OCEAN_FAR_PASS_MATERIAL,
        [{ colour: "rgba16float", depth: false }],
        "bgra8unorm",
      ),
    ).toThrow(
      "material ocean far pass writes frag_depth, which a target of rgba16float without depth cannot take",
    );
  });
});

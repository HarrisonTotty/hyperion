import { describe, expect, it } from "vitest";

import { ID_WRITE_MATERIAL } from "../idTarget";
import type { WgslMaterialSpec } from "../types";
import {
  assertFragmentOutputFits,
  type FragmentOutput,
  formatTexels,
  fragmentOutputAt,
  outputTypeName,
  outputTypeOf,
} from "./fragmentOutput";

/** A material's fields the check reads, writing `vec4f` unblended. */
const WRITER: Pick<WgslMaterialSpec, "name" | "fragmentWgsl" | "blend" | "colourWrites"> = {
  name: "writer",
  fragmentWgsl: "@fragment fn fragmentMain() -> @location(0) vec4f { return vec4f(1.0); }",
  blend: "none",
  colourWrites: true,
};

/** `WRITER` with no output at all. */
const SILENT: typeof WRITER = {
  ...WRITER,
  fragmentWgsl: "@fragment fn fragmentMain() { discard; }",
};

/** `WRITER` writing `type` at `@location(0)`. */
function writing(type: string): typeof WRITER {
  return {
    ...WRITER,
    fragmentWgsl: `@fragment fn fragmentMain() -> @location(0) ${type} { return ${type}(); }`,
  };
}

/** What `fragmentMain` of `wgsl` writes at `@location(0)`. */
function outputOf(wgsl: string): FragmentOutput {
  return fragmentOutputAt(wgsl, "fragmentMain", 0);
}

describe("a fragment's output read from its WGSL (plan R14, R14.T2.b's finding)", () => {
  it("reads an attributed return", () => {
    expect(outputOf("@fragment fn fragmentMain() -> @location(0) u32 { return 0u; }")).toEqual({
      kind: "written",
      type: { scalar: "u32", components: 1 },
    });
  });

  it("reads past the inputs' own attributes and parentheses", () => {
    expect(
      outputOf(
        "@fragment fn fragmentMain(@builtin(position) at : vec4f, @location(0) @interpolate(flat) id : u32) -> @location(0) vec4<f32> { return at; }",
      ),
    ).toEqual({ kind: "written", type: { scalar: "f32", components: 4 } });
  });

  it("reads a returned struct's member at the location, beside its depth", () => {
    expect(
      outputOf(
        [
          "struct Out {",
          "  @builtin(frag_depth) depth : f32,",
          "  // @location(0) colour : vec4f,",
          "  @location(0) ids : vec2u,",
          "}",
          "@fragment fn fragmentMain() -> Out { var out : Out; return out; }",
        ].join("\n"),
      ),
    ).toEqual({ kind: "written", type: { scalar: "u32", components: 2 } });
  });

  it("reads a location written as a typed literal", () => {
    expect([
      outputOf("@fragment fn fragmentMain() -> @location(0u) vec4f { return vec4f(); }"),
      outputOf(
        "struct Out { @location(0i) colour : vec4f }\n@fragment fn fragmentMain() -> Out { return Out(); }",
      ),
    ]).toEqual([
      { kind: "written", type: { scalar: "f32", components: 4 } },
      { kind: "written", type: { scalar: "f32", components: 4 } },
    ]);
  });

  it("leaves unread a location that is no literal, which may be the one asked for", () => {
    expect(
      [
        "const COLOUR = 0;\n@fragment fn fragmentMain() -> @location(COLOUR) vec4f { return vec4f(); }",
        "const COLOUR = 0;\nstruct Out { @location(COLOUR) colour : vec4f }\n@fragment fn fragmentMain() -> Out { return Out(); }",
      ].map((wgsl) => outputOf(wgsl).kind),
    ).toEqual(["unread", "unread"]);
  });

  it("follows an alias", () => {
    expect(
      outputOf("alias Id = u32;\n@fragment fn fragmentMain() -> @location(0) Id { return 0u; }"),
    ).toEqual({ kind: "written", type: { scalar: "u32", components: 1 } });
  });

  it("reads no output from an entry point that returns nothing, or only depth", () => {
    expect([
      outputOf("@fragment fn fragmentMain(@builtin(position) at : vec4f) { discard; }").kind,
      outputOf("@fragment fn fragmentMain() -> @builtin(frag_depth) f32 { return 0.5; }").kind,
      outputOf(
        "struct D { @builtin(frag_depth) depth : f32 }\n@fragment fn fragmentMain() -> D { return D(0.5); }",
      ).kind,
    ]).toEqual(["none", "none", "none"]);
  });

  it("leaves unread a source with no such entry point", () => {
    expect(
      [
        "",
        "@fragment fn main() -> @location(0) vec4f { return vec4f(1.0); }",
        "// @fragment fn fragmentMain() -> @location(0) vec4f { }",
      ].map((wgsl) => outputOf(wgsl).kind),
    ).toEqual(["unread", "unread", "unread"]);
  });

  it("reads WGSL's scalar and vector types, and no other", () => {
    expect(
      [
        "f32",
        "f16",
        "i32",
        "u32",
        "vec3f",
        "vec2h",
        "vec4i",
        "vec4<u32>",
        "mat4x4f",
        "vec4<bool>",
      ].map(outputTypeOf),
    ).toEqual([
      { scalar: "f32", components: 1 },
      { scalar: "f16", components: 1 },
      { scalar: "i32", components: 1 },
      { scalar: "u32", components: 1 },
      { scalar: "f32", components: 3 },
      { scalar: "f16", components: 2 },
      { scalar: "i32", components: 4 },
      { scalar: "u32", components: 4 },
      undefined,
      undefined,
    ]);
  });
});

describe("a fragment output's type as WGSL writes it", () => {
  it("names a scalar by itself and a vector by its shorthand", () => {
    expect(
      (["u32", "vec4f", "vec2h", "vec3i", "vec4<u32>"] as const).map((type) => {
        const read = outputTypeOf(type);
        return read === undefined ? undefined : outputTypeName(read);
      }),
    ).toEqual(["u32", "vec4f", "vec2h", "vec3i", "vec4u"]);
  });
});

describe("a colour format's texels (WebGPU §26.1)", () => {
  it("reads each format's sample type and components from its name", () => {
    expect(
      (
        [
          "r32uint",
          "rgba16float",
          "rg11b10ufloat",
          "rgba8unorm",
          "bgra8unorm-srgb",
          "rgb10a2uint",
          "rg16sint",
        ] as const
      ).map((format) => [format, formatTexels(format)]),
    ).toEqual([
      ["r32uint", { sampleType: "uint", components: 1 }],
      ["rgba16float", { sampleType: "float", components: 4 }],
      ["rg11b10ufloat", { sampleType: "float", components: 3 }],
      ["rgba8unorm", { sampleType: "float", components: 4 }],
      ["bgra8unorm-srgb", { sampleType: "float", components: 4 }],
      ["rgb10a2uint", { sampleType: "uint", components: 4 }],
      ["rg16sint", { sampleType: "sint", components: 2 }],
    ]);
  });
});

describe("a material's output checked against its target's format", () => {
  it("refuses a vec4f output into an r32uint target, naming both types", () => {
    expect(() => {
      assertFragmentOutputFits(WRITER, "fragmentMain", "r32uint");
    }).toThrow("material writer writes vec4f at @location(0), but r32uint takes u32");
  });

  it("refuses a u32 output into an rgba16float target, naming both types", () => {
    expect(() => {
      assertFragmentOutputFits(ID_WRITE_MATERIAL, "fragmentMain", "rgba16float");
    }).toThrow(
      `material ${ID_WRITE_MATERIAL.name} writes u32 at @location(0), but rgba16float takes vec4f`,
    );
  });

  it("takes a u32 output into r32uint and a vec4f into rgba16float", () => {
    expect(() => {
      assertFragmentOutputFits(ID_WRITE_MATERIAL, "fragmentMain", "r32uint");
      assertFragmentOutputFits(WRITER, "fragmentMain", "rgba16float");
    }).not.toThrow();
  });

  it("takes more components than the format holds, as WebGPU does", () => {
    expect(() => {
      assertFragmentOutputFits(writing("vec4u"), "fragmentMain", "r32uint");
      assertFragmentOutputFits(WRITER, "fragmentMain", "rg11b10ufloat");
    }).not.toThrow();
  });

  it("refuses fewer components than the format holds", () => {
    expect(() => {
      assertFragmentOutputFits(writing("vec3f"), "fragmentMain", "rgba8unorm");
    }).toThrow("material writer writes vec3f at @location(0), but rgba8unorm takes vec4f");
  });

  it("refuses a signed output into an unsigned format", () => {
    expect(() => {
      assertFragmentOutputFits(writing("i32"), "fragmentMain", "r32uint");
    }).toThrow("material writer writes i32 at @location(0), but r32uint takes u32");
  });

  it("refuses a blend from an output with no alpha", () => {
    expect(() => {
      assertFragmentOutputFits(
        { ...writing("vec3f"), blend: "additive" },
        "fragmentMain",
        "rg11b10ufloat",
      );
    }).toThrow("material writer blends additive into rg11b10ufloat from vec3f, which has no alpha");
  });

  it("refuses colour writes with no output at @location(0)", () => {
    expect(() => {
      assertFragmentOutputFits(SILENT, "fragmentMain", "rgba16float");
    }).toThrow(
      "material writer writes colour into rgba16float, but its fragmentMain has no @location(0) output",
    );
  });

  it("takes no output from a material that writes no colour", () => {
    expect(() => {
      assertFragmentOutputFits({ ...SILENT, colourWrites: false }, "fragmentMain", "rgba16float");
    }).not.toThrow();
  });

  it("leaves a source it cannot read to the device", () => {
    expect(() => {
      assertFragmentOutputFits({ ...WRITER, fragmentWgsl: "" }, "fragmentMain", "r32uint");
    }).not.toThrow();
  });
});

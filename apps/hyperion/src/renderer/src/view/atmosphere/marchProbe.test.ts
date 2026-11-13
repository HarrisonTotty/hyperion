import { describe, expect, it } from "vitest";

import type { BufferHandle, ComputeBindings, TextureHandle } from "../engine/types";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import {
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SLOTS_THICK_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  SKY_VIEW_KERNEL,
} from "./hillaire";
import {
  MARCH_CENSUS_BUFFER,
  MARCH_CENSUS_KERNEL,
  MARCH_CENSUS_NODES,
  marchCensusBindingsOf,
  marchCensusOf,
  marchProbedKernel,
} from "./marchProbe";

const TEXTURE: TextureHandle = { kind: "texture", name: "t" };
const CENSUS: BufferHandle = { kind: "buffer", name: "census", bytes: MARCH_CENSUS_BUFFER.bytes };

/** A march's bindings over an area, as `HillaireAtmosphere.drawFrame` gives them. */
const MARCH: ComputeBindings = {
  uniforms: {
    medium: new Float32Array(16),
    view: new Float32Array(4),
    marchArea: new Float32Array(8),
    layerEdges: new Float32Array(4),
  },
  buffers: {},
  sampled: { sceneDepth: TEXTURE, oblateBody: TEXTURE, transmittance: TEXTURE },
  storage: {},
};

describe("the march's probe (plan R08, R08.T8.d and R08.T8.e)", () => {
  it("holds every march kernel's footprint test once, and no other kernel's", () => {
    const computes = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec] : [],
    );
    const probed = computes.filter(
      (pair) => marchProbedKernel(pair, "centre").reference !== pair.reference,
    );
    expect(probed.map((pair) => pair.name)).toContain(RAY_MARCH_KERNEL.name);
    expect(probed.map((pair) => pair.name)).toContain(RAY_MARCH_CENTRE_KERNEL.name);
    expect(probed.every((pair) => pair.name.startsWith("atmosphere ray march"))).toBe(true);
    for (const pair of [RAY_MARCH_FILL_KERNEL, SKY_VIEW_KERNEL, MARCH_CENSUS_KERNEL]) {
      expect(marchProbedKernel(pair, "centre")).toBe(pair);
    }
  });

  it("marches the centre ray alone, the kernel's name kept", () => {
    const probed = marchProbedKernel(RAY_MARCH_KERNEL, "centre");
    expect([probed.name, probed.reference.includes("let above = false;")]).toEqual([
      RAY_MARCH_KERNEL.name,
      true,
    ]);
  });

  it.each([
    ["depth-zero", "SourceLight(1.0, vec3f(1.0), vec3f(0.0));"],
    ["one-source", "return min(u32(view.sky.x), 1u);"],
    ["no-lobes", "let lobes = false;"],
  ] as const)(
    "rewrites the step's sources of every march that steps under %s, and no other kernel (R08.T8.e)",
    (probe, line) => {
      const computes = WGSL_CATALOGUE.flatMap((entry) =>
        entry.kind === "compute" ? [entry.spec] : [],
      );
      const stepping = computes.filter(
        (pair) =>
          pair.name.startsWith("atmosphere ray march") &&
          pair.reference.includes("fn rayMarchSourcesAt("),
      );
      expect(stepping.map((pair) => pair.name)).toEqual(
        expect.arrayContaining([RAY_MARCH_KERNEL.name, RAY_MARCH_SLOTS_KERNEL.name]),
      );
      for (const pair of stepping) {
        const probed = marchProbedKernel(pair, probe);
        expect([probed.name, probed.reference.includes(line)]).toEqual([pair.name, true]);
      }
      for (const pair of computes.filter((each) => !stepping.includes(each))) {
        expect(marchProbedKernel(pair, probe)).toBe(pair);
      }
    },
  );

  it("compiles the per-step march's lookup out under depth-zero, as before the table", () => {
    for (const pair of [RAY_MARCH_THICK_KERNEL, RAY_MARCH_SLOTS_THICK_KERNEL]) {
      const probed = marchProbedKernel(pair, "depth-zero").reference;
      expect([
        probed.includes("let light = sourceLightAt(body, start, bodyDir, slot);"),
        probed.includes("slicedSlotDepthsOf(sliced, body, slot, lobes)"),
        probed.includes("let light = SourceLight(1.0, vec3f(1.0), vec3f(0.0));"),
      ]).toEqual([false, false, true]);
    }
  });

  it("compiles both of each step's ways out of both reads under depth-zero, keeping the ray's own", () => {
    for (const pair of [RAY_MARCH_KERNEL, RAY_MARCH_SLOTS_KERNEL]) {
      const probed = marchProbedKernel(pair, "depth-zero").reference;
      expect([
        probed.includes("for (var fromTable = tabled; fromTable; fromTable = false) {"),
        probed.includes("for (var direct = !tabled; direct; direct = false) {"),
        probed.split("if (false) {").length - pair.reference.split("if (false) {").length,
        probed.includes("fn rayMarchGroundLightAt("),
        probed.includes("return -log(sourceLightAt(body, start, d, 0u).toSun.g);"),
      ]).toEqual([false, false, 2, true, true]);
    }
  });

  it("refuses a march whose footprint test it could not tell apart", () => {
    const twice = { ...RAY_MARCH_KERNEL, reference: RAY_MARCH_KERNEL.reference.repeat(2) };
    expect(() => marchProbedKernel(twice, "centre")).toThrow(/more than one place/u);
  });
});

describe("the march's far-limb census (plan R08, R08.T8.d)", () => {
  it("reads the census's words: the texels by nodes, then its counts", () => {
    const words = Uint32Array.from({ length: MARCH_CENSUS_BUFFER.bytes / 4 }, (_, i) => i);
    expect(marchCensusOf("atmosphere ray march", words)).toEqual({
      kernel: "atmosphere ray march",
      texelsByNodes: Array.from({ length: MARCH_CENSUS_NODES + 1 }, (_, i) => i),
      wholeMeans: 11,
      centresInAir: 12,
      tiles: 13,
      limbTiles: 14,
      rays: 15,
      laneRays: 16,
    });
    expect(() => marchCensusOf("atmosphere ray march", words.subarray(1))).toThrow(RangeError);
  });

  it("is taken beside a march's footprint over an area alone, with its own five bindings", () => {
    const bound = marchCensusBindingsOf(RAY_MARCH_KERNEL.name, MARCH);
    expect(bound?.(CENSUS)).toEqual({
      uniforms: {
        medium: MARCH.uniforms["medium"],
        view: MARCH.uniforms["view"],
        marchArea: MARCH.uniforms["marchArea"],
      },
      buffers: { census: CENSUS },
      sampled: { sceneDepth: TEXTURE, oblateBody: TEXTURE },
      storage: {},
    });
    expect([
      marchCensusBindingsOf(RAY_MARCH_CENTRE_KERNEL.name, MARCH),
      marchCensusBindingsOf(SKY_VIEW_KERNEL.name, MARCH),
      marchCensusBindingsOf(RAY_MARCH_KERNEL.name, { ...MARCH, sampled: { sceneDepth: TEXTURE } }),
    ]).toEqual([null, null, null]);
  });
});

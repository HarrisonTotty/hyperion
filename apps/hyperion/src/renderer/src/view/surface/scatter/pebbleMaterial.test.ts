import { describe, expect, it } from "vitest";

import { PARTICLE_INSTANCE_BYTES, sphereParticleTemplates } from "../../clusters/particles";
import { terrainLitMaterialSpec } from "../../terrain/terrainMaterial";
import { PEBBLE_AXES } from "./pebbleLaw";
import {
  PEBBLE_BINDINGS,
  PEBBLE_MATERIALS,
  pebbleTemplates,
  SCATTER_PATCHES_BUFFER,
} from "./pebbleMaterial";

/** WebGPU's default `maxStorageBuffersPerShaderStage`, which the engine never raises. */
const STORAGE_PER_STAGE = 8;

describe("the pebbles' material", () => {
  it("has one vertex and one fragment stage, the lit terrain's renamed into functions", () => {
    for (const spec of Object.values(PEBBLE_MATERIALS)) {
      expect(spec.vertexWgsl.match(/@vertex/gu)).toHaveLength(1);
      expect(spec.vertexWgsl.match(/@fragment/gu)).toHaveLength(1);
      expect(spec.vertexWgsl).toContain("fn lit_vertex(grid : vec3f, instance : u32)");
      expect(spec.vertexWgsl).toContain("fn lit_fragment(in : LitVertexOut)");
      expect(spec.vertexWgsl).toContain("fn vertexMain(@location(0) local : vec3f");
      expect(spec.vertexWgsl).toContain("  clusterBase : u32,\n}");
      expect(spec.uniforms.at(-1)).toEqual({ name: "clusterBase", type: "u32" });
    }
  });

  it("binds the lit draw's buffers its stages read, within WebGPU's eight a stage", () => {
    for (const [path, spec] of Object.entries(PEBBLE_MATERIALS)) {
      const storage = spec.storageBuffers ?? [];
      for (const stage of ["vertex", "fragment"] as const) {
        expect(storage.filter((b) => b.stages?.includes(stage) ?? true).length).toBeLessThanOrEqual(
          STORAGE_PER_STAGE,
        );
      }
      const bindings = storage.map((b) => b.binding);
      expect(new Set(bindings).size).toBe(bindings.length);
      expect(storage.map((b) => b.name)).toEqual(
        expect.arrayContaining([
          "heights",
          "slots",
          "particleInstances",
          "particleLists",
          SCATTER_PATCHES_BUFFER,
          "classLayers",
          "terrainLights",
          "horizonMap",
          "surveyMask",
        ]),
      );
      expect(storage.find((b) => b.name === "particleInstances")?.binding).toBe(
        PEBBLE_BINDINGS.instances,
      );
      // The lit material's textures and samplers, so that the lit draw's bind them.
      const lit = terrainLitMaterialSpec(
        path === "baked-offsets" ? "baked-offsets" : "face-differences",
      );
      expect(spec.textures).toEqual(lit.textures);
      expect(spec.samplers).toEqual(lit.samplers);
      expect(spec.depthWrite).toBe(true);
      expect(spec.cullMode).toBe("back");
    }
  });

  it("reads the particle path's record at its 32 bytes", () => {
    const source = PEBBLE_MATERIALS["face-differences"].vertexWgsl;
    expect(PARTICLE_INSTANCE_BYTES).toBe(32);
    expect(source).toContain(
      "struct PebbleRecord {\n  centre : vec3f,\n  radius : f32,\n  rotation : vec2u,\n  colour : u32,\n  tag : u32,\n}",
    );
  });

  it("draws by the sphere templates, their bounds and errors grown by the largest semi-axis", () => {
    const sphere = sphereParticleTemplates();
    const pebble = pebbleTemplates();
    const largest = Math.max(...PEBBLE_AXES);
    expect(pebble.boundsScale).toBeCloseTo(sphere.boundsScale * largest, 12);
    pebble.errorFractions.forEach((e, k) => {
      expect(e).toBeCloseTo((sphere.errorFractions[k] ?? 0) * largest, 12);
      expect(pebble.templates[k]?.errorScale).toBeCloseTo(e, 12);
    });
    expect(pebble.mesh.positions).toEqual(sphere.mesh.positions);
    // Coarsest first: the error falls template by template.
    for (let k = 1; k < pebble.errorFractions.length; k += 1) {
      expect(pebble.errorFractions[k]).toBeLessThan(pebble.errorFractions[k - 1] ?? 0);
    }
  });
});

/**
 * Decorative scatter's material and templates (plan R11, R11.T6.a): `pebbleDraw.wgsl` over the lit
 * terrain's source, for each vertex path, and the particle templates a pebble is drawn by.
 *
 * @remarks
 * - **The source** is the lit terrain material's (`terrainLitMaterialSpec`): its `Draw` with a
 *   bucket's `clusterBase` after its fields (`withClusterBase`, as the cluster forms take it), its
 *   own vertex and fragment stages renamed into functions no entry point calls, and
 *   `pebbleDraw.wgsl`'s stages appended, so that a pebble is shaded by `shade_terrain` itself, with
 *   its ground's laws, horizon map, cascades, air and planetshine, and its meter class as the
 *   terrain's (R07's `METER_CLASS` in the alpha; Design note 1's lit-pass rule).
 * - **Its bindings** are the lit draw's that its stages read, so that a pebble's draw is the lit
 *   terrain draw's with the particle path's buffers and the patches' records beside: the vertex
 *   stage the heights and slot records, the instances, the lists and the patches (five of WebGPU's
 *   eight storage buffers a stage); the fragment stage the class layers, the lights, the horizon
 *   maps and the survey masks (four), and every texture and sampler of the lit material.
 * - **The templates** ({@link pebbleTemplates}) are the particle path's area-matched sphere
 *   templates (`sphereParticleTemplates`), which the vertex stage scales to the pebble's ellipsoid
 *   (`PEBBLE_AXES`): their error fractions and bounds grow by the largest semi-axis, so that the
 *   instance cull's template and bounds hold for the ellipsoid, and the record's radius stays the
 *   law's D ÷ 2.
 */

import { type ParticleTemplates, sphereParticleTemplates } from "../../clusters/particles";
import type { StorageBufferSpec, WgslMaterialSpec } from "../../engine/types";
import type { TerrainVertexPath } from "../../quality/qualitySetting";
import { TERRAIN_BINDINGS } from "../../terrain/gpu/material";
import {
  TERRAIN_LIT_BINDINGS,
  terrainLitMaterialSpec,
  withClusterBase,
} from "../../terrain/terrainMaterial";
import pebbleDrawWgsl from "./pebbleDraw.wgsl?raw";
import { PEBBLE_AXES } from "./pebbleLaw";

/** `terrainLit.wgsl`'s vertex stage's head, which the pebbles' source renames. */
const LIT_VERTEX_HEAD =
  "@vertex\nfn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> LitVertexOut {";

/** `terrainLitShade.wgsl`'s fragment stage's head, which the pebbles' source renames. */
const LIT_FRAGMENT_HEAD = "@fragment\nfn fragmentMain(in : LitVertexOut) -> @location(0) vec4f {";

/**
 * The `@group(2)` bindings `pebbleDraw.wgsl` declares past the lit material's: the particle path's
 * instances and lists (`PARTICLE_STORAGE_BUFFERS`' names) and the patches' records.
 */
export const PEBBLE_BINDINGS = { instances: 25, lists: 26, patches: 27 } as const;

/** The `DrawItem.storageBuffers` name of the patches' records. */
export const SCATTER_PATCHES_BUFFER = "scatterPatches";

/** `source` with `head` renamed to `renamed` once. */
function renamedOnce(source: string, head: string, renamed: string, what: string): string {
  const parts = source.split(head);
  if (parts.length !== 2) {
    throw new Error(
      `the pebbles' source needs the lit terrain's ${what} once, found ${String(parts.length - 1)}`,
    );
  }
  return parts.join(renamed);
}

/** The console names of the two paths' pebble materials. */
const DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "PEBBLES OFFSETS",
  "face-differences": "PEBBLES",
};

/**
 * The pebbles' material of a vertex path (see the module's remarks): opaque, depth written, back
 * faces culled, drawn with {@link pebbleTemplates}' mesh from the particle path's buckets.
 *
 * @throws Error unless the lit source holds its `Draw`, vertex and fragment stages once each.
 */
export function pebbleMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const lit = terrainLitMaterialSpec(path);
  const renamed = renamedOnce(
    renamedOnce(
      withClusterBase(lit.fragmentWgsl),
      LIT_VERTEX_HEAD,
      "fn lit_vertex(grid : vec3f, instance : u32) -> LitVertexOut {",
      "vertex stage",
    ),
    LIT_FRAGMENT_HEAD,
    "fn lit_fragment(in : LitVertexOut) -> vec4f {",
    "fragment stage",
  );
  const source = `${renamed}\n${pebbleDrawWgsl}`;
  const vertex = ["vertex"] as const;
  const fragment = ["fragment"] as const;
  const b = TERRAIN_LIT_BINDINGS;
  const storageBuffers: StorageBufferSpec[] = [
    { name: "heights", binding: TERRAIN_BINDINGS.heights, stages: vertex },
    { name: "slots", binding: TERRAIN_BINDINGS.slots, stages: vertex },
    { name: "particleInstances", binding: PEBBLE_BINDINGS.instances, stages: vertex },
    { name: "particleLists", binding: PEBBLE_BINDINGS.lists, stages: vertex },
    { name: SCATTER_PATCHES_BUFFER, binding: PEBBLE_BINDINGS.patches, stages: vertex },
    { name: "classLayers", binding: b.classLayers, stages: fragment },
    { name: "terrainLights", binding: b.terrainLights, stages: fragment },
    { name: "horizonMap", binding: b.horizonMap, stages: fragment },
    { name: "surveyMask", binding: b.surveyMask, stages: fragment },
  ];
  return {
    ...lit,
    name: `scatter-pebbles-${path}`,
    displayName: DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [...lit.uniforms, { name: "clusterBase", type: "u32" }],
    storageBuffers,
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
  };
}

/** Both paths' pebble materials, as the catalogue holds them. */
export const PEBBLE_MATERIALS: Readonly<Record<TerrainVertexPath, WgslMaterialSpec>> = {
  "face-differences": pebbleMaterialSpec("face-differences"),
  "baked-offsets": pebbleMaterialSpec("baked-offsets"),
};

/**
 * The templates a pebble is drawn by (see the module's remarks): the sphere templates, their mesh
 * as it stands (the vertex stage scales it), their extents' inner by the least semi-axis and outer
 * by the largest, their error fractions and bounds by the largest.
 */
export function pebbleTemplates(): ParticleTemplates {
  const sphere = sphereParticleTemplates();
  const largest = Math.max(...PEBBLE_AXES);
  const least = Math.min(...PEBBLE_AXES);
  return {
    templates: sphere.templates.map((t, k) => ({
      ...t,
      errorScale: (sphere.errorFractions[k] ?? t.errorScale) * largest,
    })),
    extents: sphere.extents.map(([inner, outer]) => [inner * least, outer * largest] as const),
    errorFractions: sphere.errorFractions.map((e) => e * largest),
    boundsScale: sphere.boundsScale * largest,
    mesh: { ...sphere.mesh, name: "pebble templates" },
  };
}

/**
 * The authoritative rocks' materials (plan R11, T4.b; plan R14, R14.T5.c): their lit draw on the
 * cluster path (`rock.wgsl`), lit as the terrain under each rock is, and their wireframe draw
 * (`rockWire.wgsl`), depth-writing geometry stroked at its silhouette; both over `rockPage.wgsl`'s
 * decode of the rocks' pages.
 *
 * @remarks
 * - **The lit draw** composes the lit terrain's source (`terrainMaterial.ts`' plain form, its
 *   `Draw` with `clusterBase` after the cascades' fields), its vertex and fragment stages made
 *   functions, then the rocks' decode and their own stages. It binds what the lit terrain's
 *   fragment stage binds, the slots' records and the survey mask beside them (the class sets and
 *   the horizon read them), and the rocks' four buffers past the terrain's
 *   ({@link ROCK_BINDINGS}): six storage buffers in the vertex stage and five in the fragment
 *   stage against WebGPU's eight; its textures are the lit terrain's.
 * - **The wireframe's draw** composes `frame.wgsl`, the decode and its own stages, its `Draw` the
 *   wireframe ground's with `clusterBase` after it, so that a draw takes the ground's uniforms.
 * - Both draw with the mesh shape's template (`clusters/templates.ts`' `meshClusterMesh`), cull
 *   back faces and write depth.
 */

import type { StorageBufferSpec, WgslMaterialSpec } from "../../engine/types";
import frameWgsl from "../../shaders/frame.wgsl?raw";
import {
  TERRAIN_LIT_BINDINGS,
  terrainLitMaterialSpec,
  withClusterBase,
  withLitStagesAsFunctions,
} from "../../terrain/terrainMaterial";
import rockWgsl from "./rock.wgsl?raw";
import rockPageWgsl from "./rockPage.wgsl?raw";
import rockWireWgsl from "./rockWire.wgsl?raw";

/** The rocks' `@group(2)` bindings past the terrain's (`rockPage.wgsl`). */
export const ROCK_BINDINGS = {
  /** The bucket lists of the rocks' cull, at the terrain's own binding. */
  clusterLists: TERRAIN_LIT_BINDINGS.clusterLists,
  /** The rocks' page pool's pages, word by word (`clusters/pages.ts`' `pages`). */
  rockPages: 25,
  /** The pool's records, eight words a record, 128 a slot (`pages`' `records`). */
  rockPageRecords: 26,
  /** The view's rock instances (`rockDraw.ts`' ROCK_INSTANCE_BYTES). */
  rockInstances: 27,
  /** The view's rock records (`rockPages.ts`' ROCK_RECORD_WORDS). */
  rockRecords: 28,
} as const;

/** The rocks' own storage, read by the vertex stage. */
const ROCK_STORAGE: ReadonlyArray<StorageBufferSpec> = [
  { name: "clusterLists", binding: ROCK_BINDINGS.clusterLists, stages: ["vertex"] },
  { name: "rockPages", binding: ROCK_BINDINGS.rockPages, stages: ["vertex"] },
  { name: "rockPageRecords", binding: ROCK_BINDINGS.rockPageRecords, stages: ["vertex"] },
  { name: "rockInstances", binding: ROCK_BINDINGS.rockInstances, stages: ["vertex"] },
  { name: "rockRecords", binding: ROCK_BINDINGS.rockRecords, stages: ["vertex"] },
];

/** The lit draw's material name, its `MaterialSet` key. */
export const ROCK_LIT_MATERIAL = "rock-lit";

/** The wireframe's draw's material name. */
export const ROCK_WIRE_MATERIAL = "rock-wire";

/** The lit draw's material (see the module's remarks). */
export function rockLitMaterialSpec(): WgslMaterialSpec {
  const plain = terrainLitMaterialSpec("face-differences");
  const source =
    withClusterBase(withLitStagesAsFunctions(plain.fragmentWgsl)) + rockPageWgsl + rockWgsl;
  const b = TERRAIN_LIT_BINDINGS;
  const fragment = ["fragment"] as const;
  return {
    ...plain,
    name: ROCK_LIT_MATERIAL,
    displayName: "ROCKS",
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [...plain.uniforms, { name: "clusterBase", type: "u32" }],
    storageBuffers: [
      { name: "slots", binding: b.slots, stages: ["vertex", "fragment"] },
      { name: "classLayers", binding: b.classLayers, stages: fragment },
      { name: "terrainLights", binding: b.terrainLights, stages: fragment },
      { name: "horizonMap", binding: b.horizonMap, stages: fragment },
      { name: "surveyMask", binding: b.surveyMask, stages: fragment },
      ...ROCK_STORAGE,
    ],
    cullMode: "back",
    depthWrite: true,
  };
}

/** The wireframe's draw's material (see the module's remarks). */
export function rockWireMaterialSpec(): WgslMaterialSpec {
  const source = frameWgsl + rockPageWgsl + rockWireWgsl;
  return {
    name: ROCK_WIRE_MATERIAL,
    displayName: "ROCKS WIREFRAME",
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      { name: "bodyRotation", type: "mat4x4f" },
      { name: "atlas", type: "vec4f" },
      { name: "contour", type: "vec4f" },
      { name: "contourWidthsPx", type: "vec4f" },
      { name: "graticule", type: "vec4f" },
      { name: "muted", type: "vec4f" },
      { name: "text", type: "vec4f" },
      { name: "casing", type: "vec4f" },
      { name: "occluderSlopePx", type: "f32" },
      { name: "clusterBase", type: "u32" },
    ],
    samplers: [],
    textures: [],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
    storageBuffers: ROCK_STORAGE,
  };
}

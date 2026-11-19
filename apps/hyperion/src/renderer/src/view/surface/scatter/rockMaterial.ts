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
 * - **By the visibility buffer** (plan R14, R14.T5.e; Design note 9): the ID raster
 *   (`rockId.wgsl`), the lit draw's vertex function and an ID a pixel into `r32uint`, its `Draw`
 *   the lit draw's with `visibleFirst` after `clusterBase`; and the resolve (`rockResolve.wgsl`), a
 *   full-screen draw shading each of the rocks' pixels once by the lit draw's fragment function,
 *   its lights the view's resolve frame's rows, as terrain's resolve reads them. The resolve's
 *   fragment stage binds the lit fragment's four buffers but the lights and the rocks' four, eight
 *   against WebGPU's eight, and the lit textures with the IDs and the resolve frame.
 */

import { VISIBILITY_IDS_TEXTURE } from "../../clusters/resolve";
import resolveBarycentricsWgsl from "../../clusters/resolveBarycentrics.wgsl?raw";
import { VISIBLE_FIRST_UNIFORM } from "../../clusters/visibility";
import type { StorageBufferSpec, WgslMaterialSpec } from "../../engine/types";
import frameWgsl from "../../shaders/frame.wgsl?raw";
import {
  RESOLVE_FRAME_TEXTURE,
  TERRAIN_LIT_BINDINGS,
  terrainLitMaterialSpec,
  withClusterBase,
  withLitStagesAsFunctions,
} from "../../terrain/terrainMaterial";
import rockWgsl from "./rock.wgsl?raw";
import rockForwardWgsl from "./rockForward.wgsl?raw";
import rockIdWgsl from "./rockId.wgsl?raw";
import rockPageWgsl from "./rockPage.wgsl?raw";
import rockResolveWgsl from "./rockResolve.wgsl?raw";
import rockWireWgsl from "./rockWire.wgsl?raw";

/** The rocks' `@group(2)` bindings past the terrain's (`rockPage.wgsl`). */
export const ROCK_BINDINGS = {
  /** The bucket lists of the rocks' cull, at the terrain's own binding. */
  clusterLists: TERRAIN_LIT_BINDINGS.clusterLists,
  /**
   * The rocks' page pool's pages, word by word (`clusters/pages.ts`' `pages`), each cluster's
   * template read from its page's own records.
   */
  rockPages: 25,
  /** The view's rock instances (`rockDraw.ts`' ROCK_INSTANCE_BYTES). */
  rockInstances: 27,
  /** The view's rock records (`rockPages.ts`' ROCK_RECORD_WORDS). */
  rockRecords: 28,
  /** The resolve's view of the IDs and its resolve frame (`rockResolve.wgsl`). */
  visibilityIds: 29,
  resolveFrame: 30,
} as const;

/** The rocks' own storage, read by the vertex stage. */
const ROCK_STORAGE: ReadonlyArray<StorageBufferSpec> = [
  { name: "clusterLists", binding: ROCK_BINDINGS.clusterLists, stages: ["vertex"] },
  { name: "rockPages", binding: ROCK_BINDINGS.rockPages, stages: ["vertex"] },
  { name: "rockInstances", binding: ROCK_BINDINGS.rockInstances, stages: ["vertex"] },
  { name: "rockRecords", binding: ROCK_BINDINGS.rockRecords, stages: ["vertex"] },
];

/** The lit draw's material name, its `MaterialSet` key. */
export const ROCK_LIT_MATERIAL = "rock-lit";

/** The wireframe's draw's material name. */
export const ROCK_WIRE_MATERIAL = "rock-wire";

/** The ID raster's material name (plan R14, R14.T5.e). */
export const ROCK_ID_MATERIAL = "rock-id";

/** The resolve's material name (plan R14, R14.T5.e). */
export const ROCK_RESOLVE_MATERIAL = "rock-resolve";

/** The lit `Draw`'s end, which `terrainMaterial.ts`' `withClusterBase` extends. */
const CLUSTER_BASE_END = "  clusterBase : u32,\n}";

/** The lit library under every rock draw but the wireframe's: its stages functions. */
function litLibrary(): { readonly plain: WgslMaterialSpec; readonly source: string } {
  const plain = terrainLitMaterialSpec("face-differences");
  return { plain, source: withClusterBase(withLitStagesAsFunctions(plain.fragmentWgsl)) };
}

/**
 * `source` with `from` replaced by `to`, which it holds exactly once.
 *
 * @throws Error naming the form where it does not.
 */
function once(form: string, source: string, from: string, to: string): string {
  const parts = source.split(from);
  if (parts.length !== 2) {
    throw new Error(
      `${form} needs ${JSON.stringify(from)} once, found ${String(parts.length - 1)}`,
    );
  }
  return parts.join(to);
}

/** The lit draw's material (see the module's remarks). */
export function rockLitMaterialSpec(): WgslMaterialSpec {
  const { plain, source: library } = litLibrary();
  const source = library + rockPageWgsl + rockWgsl + rockForwardWgsl;
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

/**
 * The ID raster's material (see the module's remarks): into `r32uint`, the scene's depth tested
 * and written, its storage the vertex stage's alone (the slots' records and the rocks' four).
 */
export function rockIdMaterialSpec(): WgslMaterialSpec {
  const { plain, source: library } = litLibrary();
  const source =
    once(
      "the rocks' ID raster",
      library,
      CLUSTER_BASE_END,
      "  clusterBase : u32,\n  // The first index of the rocks' range of the view's index space (`clusters/visibility.ts`).\n  visibleFirst : u32,\n}",
    ) +
    rockPageWgsl +
    rockWgsl +
    rockIdWgsl;
  return {
    ...plain,
    name: ROCK_ID_MATERIAL,
    displayName: "ROCK IDS",
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      ...plain.uniforms,
      { name: "clusterBase", type: "u32" },
      { name: VISIBLE_FIRST_UNIFORM, type: "u32" },
    ],
    samplers: [],
    textures: [],
    storageBuffers: [
      { name: "slots", binding: TERRAIN_LIT_BINDINGS.slots, stages: ["vertex"] },
      ...ROCK_STORAGE,
    ],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
  };
}

/**
 * The resolve's material (see the module's remarks): a full-screen draw at reversed depth's near
 * end that writes no depth, its `Draw` the lit draw's with `resolveLists` (`clusters/resolve.ts`'
 * `resolveUniforms`) before `clusterBase`, which it leaves 0.
 */
export function rockResolveMaterialSpec(): WgslMaterialSpec {
  const plain = terrainLitMaterialSpec("face-differences");
  const form = "the rocks' resolve";
  let shaded = once(
    form,
    plain.fragmentWgsl,
    "  cascadeViewAxis : vec4f,\n}",
    "  cascadeViewAxis : vec4f,\n  // The resolve's lists (`clusters/resolve.ts`' `resolveUniforms`).\n  resolveLists : vec4f,\n}",
  );
  shaded = once(
    form,
    shaded,
    "@group(2) @binding(8) var<storage, read> terrain_lights : array<vec4f>;",
    "// The lights are the resolve frame's rows (`rockResolve.wgsl`).",
  );
  shaded = once(
    form,
    shaded,
    "  return terrain_lights[i];",
    "  return textureLoad(rock_resolve_frame, vec2u(i, 0u), 0);",
  );
  const library = once(
    form,
    withLitStagesAsFunctions(shaded),
    "  resolveLists : vec4f,\n}",
    "  resolveLists : vec4f,\n  // 0: an ID's index is its entry's in the rocks' whole lists.\n  clusterBase : u32,\n}",
  );
  const source = `${library}${rockPageWgsl}${rockWgsl}\n${resolveBarycentricsWgsl}\n${rockResolveWgsl}`;
  const b = TERRAIN_LIT_BINDINGS;
  const fragment = ["fragment"] as const;
  return {
    ...plain,
    name: ROCK_RESOLVE_MATERIAL,
    displayName: "ROCK RESOLVE",
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      ...plain.uniforms,
      { name: "resolveLists", type: "vec4f" },
      { name: "clusterBase", type: "u32" },
    ],
    textures: [
      ...(plain.textures ?? []),
      { name: VISIBILITY_IDS_TEXTURE, binding: ROCK_BINDINGS.visibilityIds, sampleType: "uint" },
      {
        name: RESOLVE_FRAME_TEXTURE,
        binding: ROCK_BINDINGS.resolveFrame,
        sampleType: "unfilterable-float",
      },
    ],
    storageBuffers: [
      { name: "slots", binding: b.slots, stages: fragment },
      { name: "classLayers", binding: b.classLayers, stages: fragment },
      { name: "horizonMap", binding: b.horizonMap, stages: fragment },
      { name: "surveyMask", binding: b.surveyMask, stages: fragment },
      ...ROCK_STORAGE.map((buffer) => ({ ...buffer, stages: fragment })),
    ],
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "none",
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

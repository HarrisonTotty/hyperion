/**
 * The terrain pass's materials (plan R05, R05.T11.b, Design notes 4 and 17): one per vertex path,
 * composed from `frame.wgsl`, `patchVertex.wgsl` (the `FaceDifferences` arithmetic, which R07's
 * smooth figure shares), `terrainPatch.wgsl` (the records and the vertex and normal arithmetic,
 * which R10's lit material shares), `terrain.wgsl` and the path's own source, registered in
 * `WGSL_CATALOGUE`.
 *
 * @remarks
 * The two paths differ only in how a vertex's offset from its patch origin is formed; the
 * `BakedOffsets` material also binds the offsets buffer. Both draw the shared mesh of
 * `resources.ts` with one instance per drawn patch, the `Draw` uniform's `offsetFromCameraM` zero,
 * into an `rgba16float` target with a reversed-Z `depth32float`.
 */

import type { StorageBufferSpec, WgslMaterialSpec } from "../../engine/types";
import type { TerrainVertexPath } from "../../quality/qualitySetting";
import frameWgsl from "../../shaders/frame.wgsl?raw";
import patchVertexWgsl from "../shaders/patchVertex.wgsl?raw";
import bakedOffsetsWgsl from "../shaders/terrainBakedOffsets.wgsl?raw";
import faceDifferencesWgsl from "../shaders/terrainFaceDifferences.wgsl?raw";
import terrainPatchWgsl from "../shaders/terrainPatch.wgsl?raw";
import terrainWgsl from "../shaders/terrain.wgsl?raw";

/** The `@group(2)` bindings `terrainPatch.wgsl` and the paths declare. */
export const TERRAIN_BINDINGS = {
  heights: 0,
  slots: 1,
  instances: 2,
  contacts: 3,
  normals: 4,
  offsets: 5,
} as const;

/** The pass's label in `PassTimes`, the `terrain` row of the spike's metrics (Design note 18). */
export const TERRAIN_PASS_LABEL = "terrain";

/**
 * The label in `PassTimes` of a view's shadow cascades' caster passes on the high setting (plan
 * R10, R10.T9.b; decision-r10-t9b-design D7): one label for the four, whose entries are summed,
 * submitted before the view's bodies pass and timed in its resolve.
 */
export const TERRAIN_SHADOW_PASS_LABEL = "terrain shadows";

/** Each vertex path's source, which defines `ownOffset` and `morphOffset`. */
export const PATH_SOURCES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": bakedOffsetsWgsl,
  "face-differences": faceDifferencesWgsl,
};

const DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "TERRAIN OFFSETS",
  "face-differences": "TERRAIN",
};

/**
 * The storage buffers of `terrainPatch.wgsl` and the path's source, which only the vertex stage
 * reads: the heights, slot records, instances and contacts, and on `baked-offsets` the offsets
 * (R10's decision-r10-t10b-r09-t2e §5).
 */
export function terrainVertexStorage(path: TerrainVertexPath): StorageBufferSpec[] {
  const vertex = ["vertex"] as const;
  const storageBuffers: StorageBufferSpec[] = [
    { name: "heights", binding: TERRAIN_BINDINGS.heights, stages: vertex },
    { name: "slots", binding: TERRAIN_BINDINGS.slots, stages: vertex },
    { name: "instances", binding: TERRAIN_BINDINGS.instances, stages: vertex },
    { name: "contacts", binding: TERRAIN_BINDINGS.contacts, stages: vertex },
  ];
  if (path === "baked-offsets") {
    storageBuffers.push({ name: "offsets", binding: TERRAIN_BINDINGS.offsets, stages: vertex });
  }
  return storageBuffers;
}

/** The terrain material of a vertex path. */
export function terrainMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const source = frameWgsl + patchVertexWgsl + terrainPatchWgsl + terrainWgsl + PATH_SOURCES[path];
  const storageBuffers = terrainVertexStorage(path);
  return {
    name: `terrain-${path}`,
    displayName: DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      { name: "bodyRotation", type: "mat4x4f" },
      { name: "sunDirection", type: "vec4f" },
      { name: "sunRadiance", type: "vec4f" },
      { name: "atlas", type: "vec4f" },
    ],
    samplers: [],
    textures: [{ name: "normals", binding: TERRAIN_BINDINGS.normals, viewDimension: "2d-array" }],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
    storageBuffers,
  };
}

/** `terrainPatch.wgsl`'s declaration that its pipeline mixes an instance's heights (R10.T16.i). */
export const TERRAIN_MIXES_ON = "const TERRAIN_MIXES : bool = true;";

/** The still form's declaration of it ({@link stillMaterialSpec}). */
export const TERRAIN_MIXES_OFF = "const TERRAIN_MIXES : bool = false;";

/**
 * A terrain material's still form (plan R10, R10.T16.i; the lane's settled frame): its source with
 * `TERRAIN_MIXES` declared false, so that its pipeline holds no rebake's, rise's, refine's or
 * coarsen's mix, which on SwiftShader every vertex paid for whether or not it mixed. A frame draws
 * it where no instance it draws mixes (`fadeWord`'s bits 8 to 19 all 0), and draws what the
 * material draws there, bit for bit. Its name is the material's, marked still; its display name the
 * material's, since it draws the same.
 *
 * @throws Error where the material's source does not declare `TERRAIN_MIXES` exactly once.
 */
export function stillMaterialSpec(spec: WgslMaterialSpec): WgslMaterialSpec {
  const still = (source: string): string => {
    const parts = source.split(TERRAIN_MIXES_ON);
    if (parts.length !== 2) {
      throw new Error(`terrain material ${spec.name} does not declare TERRAIN_MIXES once`);
    }
    return parts.join(TERRAIN_MIXES_OFF);
  };
  return {
    ...spec,
    name: `${spec.name}-still`,
    vertexWgsl: still(spec.vertexWgsl),
    fragmentWgsl: still(spec.fragmentWgsl),
  };
}

/** Both paths' materials, for the catalogue. */
export const TERRAIN_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainMaterialSpec("baked-offsets"),
  terrainMaterialSpec("face-differences"),
];

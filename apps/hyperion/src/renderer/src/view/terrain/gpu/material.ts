/**
 * The terrain pass's materials (plan R05, R05.T11.b, Design notes 4 and 17): one per vertex path,
 * composed from `frame.wgsl`, `patchVertex.wgsl` (the `FaceDifferences` arithmetic, which R07's
 * smooth figure shares), `terrain.wgsl` and the path's own source, registered in `WGSL_CATALOGUE`.
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
import terrainWgsl from "../shaders/terrain.wgsl?raw";

/** The `@group(2)` bindings `terrain.wgsl` and the paths declare. */
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

const PATH_SOURCES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": bakedOffsetsWgsl,
  "face-differences": faceDifferencesWgsl,
};

const DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "TERRAIN OFFSETS",
  "face-differences": "TERRAIN",
};

/** The terrain material of a vertex path. */
export function terrainMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const source = frameWgsl + patchVertexWgsl + terrainWgsl + PATH_SOURCES[path];
  const storageBuffers: StorageBufferSpec[] = [
    { name: "heights", binding: TERRAIN_BINDINGS.heights },
    { name: "slots", binding: TERRAIN_BINDINGS.slots },
    { name: "instances", binding: TERRAIN_BINDINGS.instances },
    { name: "contacts", binding: TERRAIN_BINDINGS.contacts },
  ];
  if (path === "baked-offsets") {
    storageBuffers.push({ name: "offsets", binding: TERRAIN_BINDINGS.offsets });
  }
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

/** Both paths' materials, for the catalogue. */
export const TERRAIN_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainMaterialSpec("baked-offsets"),
  terrainMaterialSpec("face-differences"),
];

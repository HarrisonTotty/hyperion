/**
 * A still's tiles' accumulation into its assembled image (plan R11, R11.T10.e.2):
 * `stillAccumulate.wgsl`'s two materials, one a tile's first sample's, with no blend, and one for
 * each after it, additive.
 */

import type { WgslMaterialSpec } from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import accumulateWgsl from "./stillAccumulate.wgsl?raw";

/** The source both materials compile: the frame, then the accumulation. */
const SOURCE = frameWgsl + accumulateWgsl;

/**
 * A tile's first sample into the still's assembled image: its texels at the sample's weight, with
 * no blend, its meter class in alpha. The one the WGSL catalogue compiles.
 */
export const STILL_TILE_FIRST_MATERIAL: WgslMaterialSpec = {
  name: "still tile",
  displayName: "STILL TILES",
  vertexWgsl: SOURCE,
  fragmentWgsl: SOURCE,
  uniforms: [
    { name: "weight", type: "f32" },
    { name: "first", type: "f32" },
    { name: "origin", type: "vec4f" },
  ],
  samplers: [],
  textures: [{ name: "tile", binding: 0 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** Each later sample of a tile, added at its weight over the image, which keeps its alpha. */
export const STILL_TILE_ADD_MATERIAL: WgslMaterialSpec = {
  ...STILL_TILE_FIRST_MATERIAL,
  name: "still tile added",
  displayName: "STILL TILES ADDED",
  blend: "additive",
};

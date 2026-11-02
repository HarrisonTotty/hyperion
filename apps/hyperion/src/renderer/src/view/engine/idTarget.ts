/**
 * The engine's ID target's own forms (plan R14, R14.T2.b; an ask of R01): a material that writes
 * an ID into an `r32uint` target, tested against and writing the depth that target draws with
 * (`RenderTargetSpec.depthOf`), and one that reads an ID target as a `texture_2d<u32>` by
 * `textureLoad` (a binding of `sampleType` `uint`).
 *
 * @remarks
 * They are what T2.c's ID raster and T2.d's resolve take: the first writes `draw.id` at every
 * covered pixel its depth test passes, unblended; the second reads the ID under each pixel and
 * writes it as a grey, id ÷ 255, black where none. An ID's uniform is a `Float32Array` that the
 * engine packs as the `u32` it declares, so IDs are exact to 2²⁴ and {@link ID_NONE} is the clear's
 * alone; T2.c's ID raster writes its own. Nothing draws them yet but the smoke page's ID frame, and
 * `WGSL_CATALOGUE` compiles each for its target.
 */

import frameWgsl from "../shaders/frame.wgsl?raw";
import type { WgslMaterialSpec } from "./types";

/**
 * An ID target's value where no draw wrote: `0xffff_ffff`, reserved from the index space (plan R14,
 * Design note 9), which each frame's pass clears to.
 */
export const ID_NONE = 0xffff_ffff;

/** The ID writer's source: a mesh's positions placed as the flat materials place them. */
const ID_WRITE_WGSL = `${frameWgsl}
struct Draw {
  offsetFromCameraM : vec3f,
  // The ID written at every pixel the draw covers and its depth test passes.
  id : u32,
}
@group(1) @binding(0) var<uniform> draw : Draw;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return frame.clipProjection * frame.viewRotation * vec4f(position + draw.offsetFromCameraM, 1.0);
}

@fragment fn fragmentMain() -> @location(0) u32 {
  return draw.id;
}
`;

/** The ID reader's source: the ID under each pixel, by `textureLoad`, as a grey; none black. */
const ID_READ_WGSL = `${frameWgsl}
struct Draw {
  offsetFromCameraM : vec3f,
}
@group(1) @binding(0) var<uniform> draw : Draw;

// The ID target, read whole texels at a time: an r32uint is never filtered.
@group(2) @binding(0) var ids : texture_2d<u32>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return frame.clipProjection * frame.viewRotation * vec4f(position + draw.offsetFromCameraM, 1.0);
}

@fragment fn fragmentMain(@builtin(position) at : vec4f) -> @location(0) vec4f {
  let id = textureLoad(ids, vec2i(at.xy), 0).r;
  // None, 0xffff_ffff, is black, as no ID is.
  let grey = select(f32(id) / 255.0, 0.0, id == 0xffffffffu);
  return vec4f(vec3f(grey), 1.0);
}
`;

/**
 * The ID writer (see the module's remarks): into an `r32uint` target, `draw.id` at each pixel its
 * depth test passes, its depth written; never blended.
 */
export const ID_WRITE_MATERIAL: WgslMaterialSpec = {
  name: "engine:idWrite",
  displayName: "ID TARGET WRITE",
  vertexWgsl: ID_WRITE_WGSL,
  fragmentWgsl: ID_WRITE_WGSL,
  uniforms: [{ name: "id", type: "u32" }],
  samplers: [],
  cullMode: "none",
  depthWrite: true,
  colourWrites: true,
  blend: "none",
};

/**
 * The ID reader (see the module's remarks): an ID target bound as `ids`, `sampleType` `uint`,
 * each pixel's ID written as a grey; no depth written.
 */
export const ID_READ_MATERIAL: WgslMaterialSpec = {
  name: "engine:idRead",
  displayName: "ID TARGET READ",
  vertexWgsl: ID_READ_WGSL,
  fragmentWgsl: ID_READ_WGSL,
  uniforms: [],
  samplers: [],
  textures: [{ name: "ids", binding: 0, sampleType: "uint" }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

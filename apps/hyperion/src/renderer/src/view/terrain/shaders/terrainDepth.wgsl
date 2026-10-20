// A view's lit terrain as depth alone, for the symbology's pass over the photorealistic image
// (plan R10, R10.T16.c): drawn into the canvas's depth before the overlay's lines, so that nearer
// ground hides a survey edge, an orbit or a graticule behind it as the image does (the
// r10-shader handoff's "For R10.T16", item a). The lit material's vertices, skirts and fins
// (terrainLit.wgsl's `vertexMain`) and its survey discard (`survey_kept`), so that the depth is
// the ground the image draws: the terrain's surveyed cells and the skirts that join them to the
// reference surface, whose own depth over the image is the body's occluder sphere's. Each
// fragment's depth is pushed away from the camera as the occluders' are (R07.T16.d;
// `occluder.wgsl`): `occluderSlopePx` pixels of the depth's screen slope, its magnitude, and
// TERRAIN_DEPTH_FRACTION of the depth, so that a cased mark on the ground (the survey edge on its
// rim) stays in front of the ground it lies on, at every slope, rather than z-fighting with it
// (R10.T10.c's deferred "The mark sits at the ground's own depth"). No colour is written: the
// material's colour writes are off. Composed after terrainLit.wgsl, in place of
// terrainLitShade.wgsl, before the vertex path's source (`terrainMaterial.ts`).

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The lit material's: the camera less the body's centre, body-fixed, metres, w 1 ÷ a; unread.
  cameraBodyFixedM : vec4f,
  // The lit material's normals atlas; unread here.
  atlas : vec4f,
  // The slope term, px: the overlay's draw list's `occluderSlopePx`, which follows the strokes'
  // scale, as the body and hull occluders take it.
  occluderSlopePx : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// The constant push, a fraction of the depth: 2^-16, as the occluders' (`occluder.wgsl`'s
// DEPTH_FRACTION, Design note 5 of R02's 128 units at the larger of depth32float's units).
const TERRAIN_DEPTH_FRACTION : f32 = 0.0000152587890625;

struct TerrainDepth {
  // Unwritten: the material's colour writes are off.
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@fragment
fn fragmentMain(in : LitVertexOut) -> TerrainDepth {
  // The rasterised depth and its screen slope, per pixel, taken before the discard so that the
  // derivatives stand in uniform control flow; reversed-Z depth is affine in screen space across a
  // triangle, so the fine derivatives are exact.
  let depth = in.clip.z;
  let slope = length(vec2f(dpdxFine(depth), dpdyFine(depth)));
  if (!survey_kept(in)) {
    discard;
  }
  var out : TerrainDepth;
  out.colour = vec4f(0.0);
  // Away is smaller under reversed-Z.
  out.depth = clamp(depth * (1.0 - TERRAIN_DEPTH_FRACTION) - draw.occluderSlopePx * slope, 0.0, 1.0);
  return out;
}

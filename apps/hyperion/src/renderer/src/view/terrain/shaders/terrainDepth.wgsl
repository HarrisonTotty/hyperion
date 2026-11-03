// A view's lit terrain as depth alone, for the symbology's pass over the photorealistic image
// (plan R10, R10.T16.c): drawn into the canvas's depth before the overlay's lines, so that nearer
// ground hides a survey edge, an orbit or a graticule behind it as the image does (the
// r10-shader handoff's "For R10.T16", item a). The lit material's vertices, skirts and fins
// (terrainLit.wgsl's `vertexMain`) and its survey discard (`survey_kept`), so that the depth is
// the ground the image draws: the terrain's surveyed cells and the skirts that join them to the
// reference surface, whose own depth over the image is the body's occluder sphere's. Each
// fragment's depth is pushed away from the camera by TERRAIN_DEPTH_FRACTION of itself alone
// (R10.T16.c's grazing fix): the occluders' slope term (R07.T16.d; `occluder.wgsl`), which it took
// until then, grew as the cotangent of the grazing angle, to some 15 km 287 km out from 10 km up,
// and let an orbit, a graticule or a survey edge that far behind a ridge show through it. The mark
// on the ground, the survey edge on its rim, pulls its own depth towards the camera instead
// (`lineStroke.wgsl`'s `line_depth`), so that it stays in front of the ground it lies on rather
// than z-fighting with it (R10.T10.c's deferred "The mark sits at the ground's own depth"). No
// colour is written: the
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
  // The overlay's draw list's `occluderSlopePx`, px; unread since R10.T16.c's grazing fix, which
  // pulls the survey edge's depth towards the camera rather than pushing the terrain's away.
  occluderSlopePx : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// The overlay's depth keeps the stride-1 surface (plan R14, R14.T3.c; `terrainStrides.wgsl`).
const STRIDE_MORPH : bool = false;

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
  if (!survey_kept(in)) {
    discard;
  }
  var out : TerrainDepth;
  out.colour = vec4f(0.0);
  // Away is smaller under reversed-Z.
  out.depth = clamp(in.clip.z * (1.0 - TERRAIN_DEPTH_FRACTION), 0.0, 1.0);
  return out;
}

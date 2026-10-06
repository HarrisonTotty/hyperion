// The hulls' depth-only occluder faces (plan R02, Design note 5): two-sided, colour writes off,
// depth written from the fragment and pushed away from the camera as the body occluder's is
// (R07.T16.d; decision-r07-t16a, item 1). The push is `occluderSlopePx` pixels of the depth's
// screen slope, its magnitude, so that a cased edge's whole coverage stays in front of its own
// faces on every slope, and DEPTH_FRACTION of the depth, Design note 5's constant. No hardware
// depth bias is set: it is pipeline state, which cannot follow the strokes' scale, and backends
// may scale the slope's larger component instead of its magnitude. Composed after frame.wgsl.

struct Draw {
  // The craft's reference point from the camera, m (Design notes 2 and 22).
  offsetFromCameraM: vec3f,
  // The hull's first triangle in `corners`.
  firstTriangle: f32,
  // The slope term, px: the draw list's `occluderSlopePx`, which follows the strokes' scale.
  occluderSlopePx: f32,
}

@group(1) @binding(0) var<uniform> draw: Draw;

// One per corner, three per triangle: m from the craft's reference point, then 0.
@group(2) @binding(0) var<storage, read> corners: array<vec4f>;

// The constant push, a fraction of the depth: 2^-16, Design note 5's 128 units at the larger of
// depth32float's units (2^-23 of the depth), the same on every backend (`drawList.ts`'s
// HULL_OCCLUDER_DEPTH_FRACTION).
const DEPTH_FRACTION = 0.0000152587890625;

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> @builtin(position) vec4f {
  // corner.x is the corner's index in its triangle: 0, 1 or 2.
  let p = corners[(u32(draw.firstTriangle) + instance) * 3u + u32(corner.x)].xyz;
  return frame.clipProjection * frame.viewRotation * vec4f(draw.offsetFromCameraM + p, 1.0);
}

struct FaceDepth {
  @location(0) colour: vec4f,
  @builtin(frag_depth) depth: f32,
}

@fragment
fn fragmentMain(@builtin(position) position: vec4f) -> FaceDepth {
  // The rasterised depth and its screen slope, per pixel: reversed-Z depth is affine in screen
  // space across a plane, so the fine derivatives of a face's depth are exact.
  let depth = position.z;
  let slope = length(vec2f(dpdxFine(depth), dpdyFine(depth)));
  var out: FaceDepth;
  out.colour = vec4f(0.0);
  // Away is smaller under reversed-Z.
  out.depth = clamp(depth * (1.0 - DEPTH_FRACTION) - draw.occluderSlopePx * slope, 0.0, 1.0);
  return out;
}

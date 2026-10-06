// The hulls' depth-only occluder faces (plan R02, Design note 5): two-sided, colour writes off,
// depth written and pushed away from the camera by the pass's depth bias, which the material
// carries as `depthBiasAway { constant: 128, slopeScale: 3 }` and never sets on a line pass: the
// slope term covers a cased edge's half-width and its fringe over a photorealistic image
// (R07.T16.a). Composed after frame.wgsl.

struct Draw {
  // The craft's reference point from the camera, m (Design notes 2 and 22).
  offsetFromCameraM: vec3f,
  // The hull's first triangle in `corners`.
  firstTriangle: f32,
}

@group(1) @binding(0) var<uniform> draw: Draw;

// One per corner, three per triangle: m from the craft's reference point, then 0.
@group(2) @binding(0) var<storage, read> corners: array<vec4f>;

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> @builtin(position) vec4f {
  // corner.x is the corner's index in its triangle: 0, 1 or 2.
  let p = corners[(u32(draw.firstTriangle) + instance) * 3u + u32(corner.x)].xyz;
  return frame.clipProjection * frame.viewRotation * vec4f(draw.offsetFromCameraM + p, 1.0);
}

@fragment
fn fragmentMain() -> @location(0) vec4f {
  return vec4f(0.0);
}

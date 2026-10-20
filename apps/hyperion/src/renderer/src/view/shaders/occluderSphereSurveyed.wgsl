// A body's occluder sphere over its surveyed terrain in the wireframe (plan R10, R10.T11; Design
// note 12): occluderSphere.wgsl's depth, discarded wherever the body's coverage mask holds the cell
// beneath the hit surveyed (wireCoverage.wgsl), so that the terrain's own depth stands there and the
// sphere hides no surveyed valley below the datum. One body a draw, its sphere `firstSphere` of the
// frame's spheres. Composed after frame.wgsl, occluderSphereHit.wgsl, cubeTexel.wgsl and
// wireCoverage.wgsl. Colour writes off, depth written, compared greater-equal.

struct Draw {
  // Unused: the sphere carries its own centre.
  offsetFromCameraM: vec3f,
  // The slope term's scale, px, as occluderSphere.wgsl's.
  occluderSlopePx: f32,
  // The body's sphere's index among the frame's spheres.
  firstSphere: f32,
  // The body's body-fixed axes into the camera frame's, as `mat4x4f` (its rotation).
  coverageAxes: mat4x4f,
  // x the body's equatorial radius over its polar, a ÷ c; y, z, w unused.
  coverageStretch: vec4f,
}

@group(1) @binding(0) var<uniform> draw: Draw;

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> SphereVarying {
  return sphere_vertex(corner, u32(draw.firstSphere) + instance);
}

@fragment
fn fragmentMain(v: SphereVarying) -> SphereDepth {
  let hit = sphere_hit(v);
  // The hit from the body's centre, back from view space to the camera frame's axes.
  let fromCentre = (transpose(frame.viewRotation) * vec4f(hit.hitM - hit.centreM, 0.0)).xyz;
  if (!hit.found || coverage_surveyed(fromCentre)) {
    discard;
  }
  return sphere_depth(hit, draw.occluderSlopePx);
}

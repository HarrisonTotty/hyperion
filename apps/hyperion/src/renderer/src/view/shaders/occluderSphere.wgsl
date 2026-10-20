// The bodies' depth-only occluder spheres (plan R02, Design note 5), exact rather than tessellated:
// each sphere is a screen rectangle the CPU bounds, and each fragment intersects its ray with the
// sphere and writes that depth (occluderSphereHit.wgsl). Composed after frame.wgsl and
// occluderSphereHit.wgsl. Colour writes off, depth written, compared greater-equal.

struct Draw {
  // Unused: each sphere carries its own centre.
  offsetFromCameraM: vec3f,
  // The slope term's scale, px: the draw list's `occluderSlopePx`, half the widest cased stroke
  // over a body (the heavy stroke with a casing each side, at the strokes' scale) and the
  // one-pixel fringe, w_max / 2 + 1, rounded up: 3 at a scale of 1, 5 at 2 (R07.T16.d).
  occluderSlopePx: f32,
}

@group(1) @binding(0) var<uniform> draw: Draw;

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> SphereVarying {
  return sphere_vertex(corner, instance);
}

@fragment
fn fragmentMain(v: SphereVarying) -> SphereDepth {
  let hit = sphere_hit(v);
  if (!hit.found) {
    discard;
  }
  // Pushed away by the slope over a cased stroke's half-width and its fringe, as the hull faces
  // are (Design note 5).
  return sphere_depth(hit, draw.occluderSlopePx);
}

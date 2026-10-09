// The bodies' depth-only occluder spheres (plan R02, Design note 5), exact rather than tessellated:
// each sphere is a screen rectangle the CPU bounds, and each fragment intersects its ray with the
// sphere and writes that depth. Composed after frame.wgsl. Colour writes off, depth written,
// compared greater-equal.

struct Draw {
  // Unused: each sphere carries its own centre.
  offsetFromCameraM: vec3f,
  // The slope term's scale, px: the draw list's `occluderSlopePx`, half the widest cased stroke
  // over a body (the heavy stroke with a casing each side, at the strokes' scale) and the
  // one-pixel fringe, w_max / 2 + 1, rounded up: 3 at a scale of 1, 5 at 2 (R07.T16.d).
  occluderSlopePx: f32,
}

@group(1) @binding(0) var<uniform> draw: Draw;

// Three per sphere: its screen rectangle, px (left, top, right, bottom); its centre from the
// camera, m, along the camera frame's axes, and its radius, m (`occluderRadius`); its altitude, the
// centre's distance less the radius, m, differenced in f64 on the CPU, then 0, 0, 0.
@group(2) @binding(0) var<storage, read> spheres: array<vec4f>;

struct SphereVarying {
  @builtin(position) position: vec4f,
  @location(0) @interpolate(flat) sphere: u32,
}

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> SphereVarying {
  let rect = spheres[instance * 3u];
  let px = mix(rect.xy, rect.zw, corner.xy);
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  var out: SphereVarying;
  out.position = vec4f(ndc, 0.0, 1.0);
  out.sphere = instance;
  return out;
}

struct SphereDepth {
  @location(0) colour: vec4f,
  @builtin(frag_depth) depth: f32,
}

@fragment
fn fragmentMain(v: SphereVarying) -> SphereDepth {
  let centreAndRadius = spheres[v.sphere * 3u + 1u];
  let altitudeM = spheres[v.sphere * 3u + 2u].x;
  let radiusM = centreAndRadius.w;
  // The ray through this pixel's centre, in view space (looking down -z).
  let ndc = vec2f(
    v.position.x / frame.viewport.x * 2.0 - 1.0,
    1.0 - v.position.y / frame.viewport.y * 2.0,
  );
  let ray = normalize(vec3f(
    ndc.x / frame.clipProjection[0][0],
    ndc.y / frame.clipProjection[1][1],
    -1.0,
  ));
  let centre = (frame.viewRotation * vec4f(centreAndRadius.xyz, 0.0)).xyz;
  let along = dot(centre, ray);
  let perp = centre - along * ray;
  let h2 = radiusM * radiusM - dot(perp, perp);
  if (along <= 0.0 || h2 < 0.0 || altitudeM <= 0.0) {
    discard;
  }
  // The near intersection, t = (D^2 - r^2) / (b + sqrt(h^2)), with D^2 - r^2 = altitude * (D + r):
  // free of the cancellation of b - sqrt(h^2) close to the body.
  let t = altitudeM * (length(centre) + radiusM) / (along + sqrt(h2));
  let n = frame.clipProjection[3][2];
  // Reversed-Z, infinite far: depth = n / (-z) (Design note 4).
  let depth = n / (t * -ray.z);
  // The depth's slope across the screen, per pixel, from the tangent plane at the hit point: on
  // it depth = n (N . u) / (N . P) with u = (x_ndc / s, y_ndc / (s a), -1), linear in the view.
  let hit = t * ray;
  let normal = (hit - centre) / radiusM;
  let facing = abs(dot(normal, hit));
  let slopeX = n * abs(normal.x) * 2.0 / (frame.viewport.x * frame.clipProjection[0][0] * facing);
  let slopeY = n * abs(normal.y) * 2.0 / (frame.viewport.y * frame.clipProjection[1][1] * facing);
  var out: SphereDepth;
  out.colour = vec4f(0.0);
  // Pushed away by the slope over a cased stroke's half-width and its fringe, as the hull faces
  // are (Design note 5), so that the whole width of the body's own graticule stays in front:
  // the 4e-6 margin alone holds only the stroke's centreline. The slope's magnitude, not its larger
  // component, which falls up to sqrt(2) short where the gradient runs diagonally across the screen.
  out.depth = clamp(depth - draw.occluderSlopePx * length(vec2f(slopeX, slopeY)), 0.0, 1.0);
  return out;
}

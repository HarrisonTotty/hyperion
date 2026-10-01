// The bodies' depth-only occluder spheres (plan R02, Design note 5), exact rather than tessellated:
// each sphere is a screen rectangle the CPU bounds, and each fragment intersects its ray with the
// sphere and writes that depth. Composed after frame.wgsl. Colour writes off, depth written,
// compared greater-equal.

struct Draw {
  // Unused: each sphere carries its own centre.
  offsetFromCameraM: vec3f,
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
  // The near intersection, t = (D^2 - r^2) / (b + sqrt(h^2)), with D^2 - r^2 = altitude * (D + r): free of
  // the cancellation of b - sqrt(h^2) close to the body.
  let t = altitudeM * (length(centre) + radiusM) / (along + sqrt(h2));
  var out: SphereDepth;
  out.colour = vec4f(0.0);
  // Reversed-Z, infinite far: depth = n / (-z) (Design note 4).
  out.depth = min(frame.clipProjection[3][2] / (t * -ray.z), 1.0);
  return out;
}

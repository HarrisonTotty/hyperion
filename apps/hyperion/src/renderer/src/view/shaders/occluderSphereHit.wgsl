// The bodies' occluder spheres' geometry (plan R02, Design note 5; a library since R10.T11, shared
// by occluderSphere.wgsl and occluderSphereSurveyed.wgsl): each sphere a screen rectangle the CPU
// bounds, and each fragment's ray intersected with the sphere, exact rather than tessellated. A
// spheroid's (R02.T19) is intersected in its stretched space, L v = v + (v . q) q along its pole,
// where it is a sphere: the ray's parameter is the same in both spaces.
// Composed after frame.wgsl, before an occluder's own `Draw` and entry points.

// Three per sphere: its screen rectangle, px (left, top, right, bottom); its centre from the
// camera, m, along the camera frame's axes, and its radius, m (`occluderRadius`, a spheroid's
// stretched one); its altitude, the centre's distance less the radius, m, differenced in f64 on
// the CPU (a spheroid's in its stretched space), then its axis q, its pole times sqrt(a / c - 1)
// along the camera frame's axes, 0, 0, 0 for a sphere (`spheroidAxis`).
@group(2) @binding(0) var<storage, read> spheres: array<vec4f>;

// The rows a sphere takes in `spheres`, as `packWireframe` writes them (`OCCLUDER_SPHERE_ROWS`).
const SPHERE_ROWS: u32 = 3u;

struct SphereVarying {
  @builtin(position) position: vec4f,
  @location(0) @interpolate(flat) sphere: u32,
}

// Corner `corner` of sphere `sphere`'s screen rectangle.
fn sphere_vertex(corner: vec3f, sphere: u32) -> SphereVarying {
  let rect = spheres[sphere * SPHERE_ROWS];
  let px = mix(rect.xy, rect.zw, corner.xy);
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  var out: SphereVarying;
  out.position = vec4f(ndc, 0.0, 1.0);
  out.sphere = sphere;
  return out;
}

// Where a fragment's ray meets its sphere.
struct SphereHit {
  // Whether it meets it, in front of the camera, from outside.
  found: bool,
  // The near intersection's depth, reversed-Z.
  depth: f32,
  // The depth's slope across the screen, per pixel, from the tangent plane at the hit.
  slope: f32,
  // The hit, and the sphere's centre, in view space, m.
  hitM: vec3f,
  centreM: vec3f,
}

fn sphere_hit(v: SphereVarying) -> SphereHit {
  let centreAndRadius = spheres[v.sphere * SPHERE_ROWS + 1u];
  let altitudeAndAxis = spheres[v.sphere * SPHERE_ROWS + 2u];
  let altitudeM = altitudeAndAxis.x;
  let radiusM = centreAndRadius.w;
  // A spheroid's axis in view space, and whether there is one: a sphere takes the sphere's own
  // sums, unchanged.
  let axis = (frame.viewRotation * vec4f(altitudeAndAxis.yzw, 0.0)).xyz;
  let oblate = dot(altitudeAndAxis.yzw, altitudeAndAxis.yzw) > 0.0;
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
  // The ray and the centre in the stretched space, the ray's direction there and its length, by
  // which a distance along it there is divided back.
  let stretchedRay = ray + dot(ray, axis) * axis;
  let rayScale = select(1.0, length(stretchedRay), oblate);
  let direction = select(ray, stretchedRay / rayScale, oblate);
  let stretchedCentre = select(centre, centre + dot(centre, axis) * axis, oblate);
  let along = dot(stretchedCentre, direction);
  let perp = stretchedCentre - along * direction;
  let h2 = radiusM * radiusM - dot(perp, perp);
  var out: SphereHit;
  out.found = !(along <= 0.0 || h2 < 0.0 || altitudeM <= 0.0);
  out.centreM = centre;
  if (!out.found) {
    return out;
  }
  // The near intersection, t = (D^2 - r^2) / (b + sqrt(h^2)), with D^2 - r^2 = altitude * (D + r):
  // free of the cancellation of b - sqrt(h^2) close to the body.
  let t = altitudeM * (length(stretchedCentre) + radiusM) / (along + sqrt(h2)) / rayScale;
  let n = frame.clipProjection[3][2];
  // Reversed-Z, infinite far: depth = n / (-z) (Design note 4).
  out.depth = n / (t * -ray.z);
  // The depth's slope across the screen, per pixel, from the tangent plane at the hit point: on
  // it depth = n (N . u) / (N . P) with u = (x_ndc / s, y_ndc / (s a), -1), linear in the view.
  let hit = t * ray;
  out.hitM = hit;
  // The outward normal: a sphere's (hit - c) / r, a spheroid's along L^2 (hit - c), the gradient of
  // |L (x - c)|^2, L^2 v = v + (s + 1)(v . q) q with s + 1 = 2 + |q|^2.
  let fromCentre = hit - centre;
  let normal = select(
    fromCentre / radiusM,
    normalize(fromCentre + (2.0 + dot(axis, axis)) * dot(fromCentre, axis) * axis),
    oblate,
  );
  let facing = abs(dot(normal, hit));
  let slopeX = n * abs(normal.x) * 2.0 / (frame.viewport.x * frame.clipProjection[0][0] * facing);
  let slopeY = n * abs(normal.y) * 2.0 / (frame.viewport.y * frame.clipProjection[1][1] * facing);
  // The slope's magnitude, not its larger component, which falls up to sqrt(2) short where the
  // gradient runs diagonally across the screen.
  out.slope = length(vec2f(slopeX, slopeY));
  return out;
}

struct SphereDepth {
  @location(0) colour: vec4f,
  @builtin(frag_depth) depth: f32,
}

// A hit's depth pushed away by `slopePx` pixels of its slope, so that the whole width of the body's
// own graticule stays in front: the 4e-6 margin alone holds only the stroke's centreline.
fn sphere_depth(hit: SphereHit, slopePx: f32) -> SphereDepth {
  var out: SphereDepth;
  out.colour = vec4f(0.0);
  out.depth = clamp(hit.depth - slopePx * hit.slope, 0.0, 1.0);
  return out;
}

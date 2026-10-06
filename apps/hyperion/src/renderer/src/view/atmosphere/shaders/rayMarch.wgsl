// The per-pixel ray march (plan R05, Design note 16; Hillaire 2020, section 5.4): the light
// scattered into a pixel's ray per unit sun illuminance (rgb) and the ray's mean transmittance (a),
// where the tables do not serve: the sky seen from above the atmosphere, and surfaces beyond the
// aerial-perspective volume's reach. Elsewhere it writes nothing the composite reads. At
// `view.output.z` of the output's resolution (1 high, 0.5 low, with the composite's depth-aware
// upsample). Follows `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl`.
//
// Ported from Bevy 0.19.1's `raymarch_atmosphere` (functions.wgsl, MIT; the notice is in
// common.wgsl). Changes (Design note 16): the ray is clipped against the datum spheroid's shells,
// a + top and c + top for the atmosphere and a and c for the ground, not spheres; each sample's
// height is its height above the spheroid along the radius and its sun cosine is taken against the
// spheroid's normal, and the tables are read at that height; the ground's bounce reads the sun's
// transmittance at the ground (Bevy reads it at radius 0); each step is sampled at its midpoint.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<uniform> view : AtmosphereView;
@group(0) @binding(2) var transmittance : texture_2d<f32>;
@group(0) @binding(3) var multiScattering : texture_2d<f32>;
@group(0) @binding(4) var sceneDepth : texture_depth_2d;
@group(0) @binding(5) var rayMarchOut : texture_storage_2d<rgba16float, write>;

// The near and far distances along a ray to a spheroid of radii (a, a, c), or (-1, -1) if missed:
// a sphere's in space scaled by a ÷ c along z.
fn spheroidHits(origin : vec3f, dir : vec3f, a : f32, c : f32) -> vec2f {
  let s = vec3f(1.0, 1.0, a / c);
  let o = origin * s;
  let d = dir * s;
  let qa = dot(d, d);
  let qb = dot(o, d);
  let qc = dot(o, o) - a * a;
  let disc = qb * qb - qa * qc;
  if (disc < 0.0) {
    return vec2f(-1.0, -1.0);
  }
  let root = sqrt(disc);
  return vec2f((-qb - root) / qa, (-qb + root) / qa);
}

// A point's height above the spheroid along its radius, m.
fn heightAbove(p : vec3f, a : f32, c : f32) -> f32 {
  let r = length(p);
  let d = p / r;
  let surface = inverseSqrt((d.x * d.x + d.y * d.y) / (a * a) + d.z * d.z / (c * c));
  return r - surface;
}

// The spheroid's outward normal at the ellipsoid through `p` similar to the datum.
fn normalAt(p : vec3f, a : f32, c : f32) -> vec3f {
  return normalize(vec3f(p.x / (a * a), p.y / (a * a), p.z / (c * c)));
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  // The target may be larger than this frame's output (it grows and is never remade smaller).
  let size = vec2u(ceil(view.output.xy * view.output.z));
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size);
  let full = vec2u(view.output.xy);
  let depthTexel = min(vec2u(uv * vec2f(full)), full - 1u);
  let depth = textureLoad(sceneDepth, depthTexel, 0);
  let topM = medium.topRadiusM - medium.bottomRadiusM;
  let aboveTop = view.camera.w >= topM;
  let reach = view.tables.y;
  var tSurface = 3.4e38;
  if (depth > 0.0) {
    tSurface = distanceAtDepth(view, uv, depth);
  }
  let needed = aboveTop || (depth > 0.0 && tSurface > reach);
  if (!needed) {
    textureStore(rayMarchOut, id.xy, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  let dir = rayThrough(view, uv);
  let origin = view.camera.xyz;
  let a = view.figure.x;
  let c = view.figure.y;
  let shell = spheroidHits(origin, dir, a + topM, c + topM);
  let ground = spheroidHits(origin, dir, a, c);
  let tStart = max(shell.x, 0.0);
  var tEnd = shell.y;
  let hitsGround = ground.x > 0.0;
  if (hitsGround) {
    tEnd = min(tEnd, ground.x);
  }
  tEnd = min(tEnd, tSurface);
  if (shell.y <= 0.0 || tEnd <= tStart) {
    textureStore(rayMarchOut, id.xy, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  let samples = max(u32(view.figure.w), 1u);
  let dt = (tEnd - tStart) / f32(samples);
  let cosTheta = dot(dir, view.sun.xyz);
  var luminance = vec3f(0.0);
  var throughput = vec3f(1.0);
  for (var i = 0u; i < samples; i++) {
    let p = origin + (tStart + (f32(i) + 0.5) * dt) * dir;
    let heightM = heightAbove(p, a, c);
    let muSun = dot(view.sun.xyz, normalAt(p, a, c));
    let local = mediumAt(medium.bottomRadiusM + heightM);
    let extinction = max(local.extinction, vec3f(1e-12));
    let stepTransmittance = exp(-local.extinction * dt);
    let source = sourceAt(transmittance, multiScattering, view.tables.x, heightM, muSun, cosTheta);
    luminance += throughput * (source - source * stepTransmittance) / extinction;
    throughput *= stepTransmittance;
  }
  // The bare ground under the atmosphere, seen from above it where nothing is drawn.
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    let p = origin + ground.x * dir;
    let muSun = dot(view.sun.xyz, normalAt(p, a, c));
    let toSun = tableTransmittance(transmittance, view.tables.x, 0.0, muSun);
    // Sunlight on the grey ground keeps the sun's spectrum: the sun's factors, not the sky's.
    luminance += throughput * toSun * max(muSun, 0.0) * medium.groundAlbedo.rgb / PI_VIEW
      * view.sunOverSky.rgb;
  }
  // A sky pixel whose ray meets the ground sees nothing of space behind it: no sun's disc there.
  var meanTransmittance = (throughput.r + throughput.g + throughput.b) / 3.0;
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    meanTransmittance = 0.0;
  }
  textureStore(rayMarchOut, id.xy, vec4f(luminance, meanTransmittance));
}

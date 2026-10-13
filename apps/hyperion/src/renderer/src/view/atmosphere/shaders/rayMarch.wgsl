// The per-pixel ray march (plan R05, Design note 16; Hillaire 2020, section 5.4): the light
// scattered into a pixel's ray, summed over the view's drawn sources and weighted by each one's
// illuminance times the frame's scale (plan R08, R08.T7) (rgb), and the ray's mean transmittance (a),
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
// transmittance at the ground (Bevy reads it at radius 0); the steps are placed toward the ray's
// lowest point, the point nearest the body's centre, or, for the camera side of a two-sided ray
// from inside the atmosphere, toward the camera (`marchSplit` in common.wgsl, R05.T12.e), where
// Bevy spaces them evenly, and each is sampled at its midpoint; each term's density is evaluated
// once a sample; a step's in-scattering is taken without f32's cancellation (`stepFactor` in
// common.wgsl). The placement takes the centre alone: it needs no spheroid.
//
// Changes (plan R08, R08.T6.g, Design note 18): no height, root or discriminant is formed from the
// camera's f32 position as a difference of radii, which in f32 near 6.4e6 m carry 0.25 m (2 m near
// 6e7 m). Each shell is a quadric along the ray, Q(t) = q_a t^2 + 2 q_b t + q_c in space scaled to
// a sphere, whose constant q_c comes formed in f64 (`view.exact.xy`); the camera's position enters
// only in products with unit directions, where its rounding moves a height by about 6e-8 t. The
// ray meets each shell at the roots that do not cancel, and a sample's height above the spheroid
// along its radius is |p| q / (sqrt(1 + q)(sqrt(1 + q) + 1)), q = Q / a^2. Beyond h0 = a / 2, Q is
// taken about the ray's closest approach t*, q_a (t - t*)^2 + Q*, with Q* = -disc / q_a from
// Lagrange's identity and t - t* carried from the steps' own offsets (`marchStep`), where the
// constant's own error would grow as h0 (`decision-r08-f32.md` section 4.2).
//
// Changes (plan R08, R08.T6.c, Design notes 5 and 8): the ray's own path takes its absorbers by
// the path's column, under the sun's curves of growth (`viewAbsorberStep` in source.wgsl), and
// the sun's transmittance, the bare ground's included, is the table's optical depth with them.
//
// Changes (plan R08, R08.T6.i; `decision-r08-t6d-findings.md` section 1): where the medium has
// layer edges, each side of the ray's split is cut into pieces where the ray crosses them
// (`MarchWalk` in common.wgsl), at the same count; a medium with none takes the steps as before.
// Each edge e's shell is the quadric (a + e, c + e), as the top's is (a + H, c + H)
// (`marchWalkCrossShell`).
//
// Changes (plan R08, R08.T7, Design note 7): one march sums every drawn source, each light slot's
// share carried on its own path (`ViewMarch` in source.wgsl), alpha slot 0's mean transmittance;
// the bare ground is lit by every source through its own slot's throughput.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d<f32>;
@group(0) @binding(6) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(7) var sceneDepth : texture_depth_2d;
@group(0) @binding(8) var rayMarchOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var absorberColumns : texture_2d<f32>;
@group(0) @binding(10) var absorberCurves : texture_2d<f32>;
@group(0) @binding(11) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(12) var absorberPressureColumns : texture_2d<f32>;
@group(0) @binding(13) var lastAbsorberPressureColumn : texture_2d<f32>;
@group(0) @binding(14) var lobeTables : texture_2d_array<f32>;

// A ray's quadric about a shell of equatorial radius `radius`, in space scaled by
// S = diag(1, 1, k) to a sphere of that radius: Q(t) = |S (P + t d)|^2 - radius^2 = qa t^2 +
// 2 qb t + qc, its constant qc formed in f64. `disc` is qb^2 - qa qc without its cancellation: as
// written near the body, where qc is small, and from Lagrange's identity, qa radius^2 - |S P x S d|^2,
// far from it (`far`), where qb^2 and qa qc nearly cancel.
struct RayQuadric {
  qa : f32,
  qb : f32,
  qc : f32,
  disc : f32,
}

fn rayQuadric(origin : vec3f, dir : vec3f, k : f32, radius : f32, qc : f32, far : bool) -> RayQuadric {
  let scale = vec3f(1.0, 1.0, k);
  let o = origin * scale;
  let d = dir * scale;
  let qa = dot(d, d);
  let qb = dot(o, d);
  var disc : f32;
  if (far) {
    let c = cross(o, d);
    disc = qa * (radius * radius) - dot(c, c);
  } else {
    disc = qb * qb - qa * qc;
  }
  return RayQuadric(qa, qb, qc, disc);
}

// The near and far distances along the ray to the quadric's surface, or (-1, -1) if missed: its
// roots s / qa and qc / s, s = -(qb + sign(qb) sqrt(disc)), neither of which cancels.
fn quadricHits(q : RayQuadric) -> vec2f {
  if (q.disc < 0.0) {
    return vec2f(-1.0, -1.0);
  }
  let root = sqrt(q.disc);
  let s = -(q.qb + select(-root, root, q.qb >= 0.0));
  if (s == 0.0) {
    return vec2f(0.0, 0.0);
  }
  let first = s / q.qa;
  let second = q.qc / s;
  return vec2f(min(first, second), max(first, second));
}

// A point's height above the spheroid along its radius, from its distance from the centre and
// q = Q / a^2 (the ground's quadric there): |p| - |p| / sqrt(1 + q), the difference taken as
// |p| q / (sqrt(1 + q)(sqrt(1 + q) + 1)).
fn radialHeight(r : f32, q : f32) -> f32 {
  let root = sqrt(max(1.0 + q, 1e-12));
  return r * q / (root * (root + 1.0));
}

// The spheroid's outward normal at the ellipsoid through `p` similar to the datum.
fn normalAt(p : vec3f, a : f32, c : f32) -> vec3f {
  return normalize(vec3f(p.x / (a * a), p.y / (a * a), p.z / (c * c)));
}

// A pixel's ray and what its samples' heights are formed from (Design note 18): the camera's
// position and the ray's direction, the datum's radii, whether the ground's quadric is taken about
// the closest approach (`far`), that quadric, Q* = -disc / qa there, t_low - t* (the split's lowest
// point from the quadric's own closest approach), a^2, and the sources' directions with their
// scattering angles (R08.T7).
struct MarchRay {
  origin : vec3f,
  dir : vec3f,
  a : f32,
  c : f32,
  far : bool,
  groundQ : RayQuadric,
  qStar : f32,
  lowestFromStar : f32,
  a2 : f32,
  sources : array<vec3f, MAX_SKY_SOURCES>,
  angles : array<ScatteringAngle, MAX_SKY_SOURCES>,
}

// One step of the march, at its midpoint: its height above the spheroid along the radius from the
// ground's quadric, about t* beyond h0 = a / 2 and from the camera within it, then the medium
// there, its sources and their in-scattering.
fn rayMarchStep(march : ptr<function, ViewMarch>, ray : MarchRay, stepAt : MarchStep) {
  let p = ray.origin + stepAt.tM * ray.dir;
  let groundQ = ray.groundQ;
  var bigQ : f32;
  if (ray.far) {
    let fromStar = stepAt.fromLowestM + ray.lowestFromStar;
    bigQ = groundQ.qa * fromStar * fromStar + ray.qStar;
  } else {
    bigQ = stepAt.tM * (groundQ.qa * stepAt.tM + 2.0 * groundQ.qb) + groundQ.qc;
  }
  let heightM = radialHeight(length(p), bigQ / ray.a2);
  let local = sampleMediumAt(heightM);
  let up = normalAt(p, ray.a, ray.c);
  let shares = viewSourcesAt(heightM, up, ray.sources, ray.angles, local);
  viewMarchStep(march, local, shares, stepAt.dtM);
}

// Adds to the walk where the ray crosses the shell of the march edge `edgeM` above the datum (plan
// R08, R08.T6.i): the quadric (a + e, c + e), as the top's shell (a + H, c + H) stands for the top,
// in space scaled by S_e = diag(1, 1, k_e), k_e = (a + e) / (c + e), crossed at most twice, each
// crossing as its signed distance from t_low, the split's lowest point. Within h0 = a / 2 the
// crossings are the quadric's roots (`quadricHits`), its constant |S_e P|^2 - (a + e)^2 formed
// from the ground's q_g, which comes formed in f64, as q_g + (k_e - k)(k_e + k) P_z^2 - e (2a + e),
// with k_e - k = e (c - a) / (c (c + e)): no difference of large values (Design note 18). Beyond it
// they stand about the quadric's own closest approach t*_e, at -/+ sqrt(disc) / qa with disc from
// Lagrange's identity, t*_e taken from the ground's t* as t_o - t* is (`nearestFromStar`, with
// k_e), so that each lies from t_low without a difference of large distances, as the steps do.
fn marchWalkCrossShell(
  walk : ptr<function, MarchWalk>,
  ray : MarchRay,
  nearestFromStar : f32,
  acrossZ : f32,
  edgeM : f32,
) {
  let a = ray.a;
  let c = ray.c;
  let kE = (a + edgeM) / (c + edgeM);
  if (ray.far) {
    let q = rayQuadric(ray.origin, ray.dir, kE, a + edgeM, 0.0, true);
    if (q.disc < 0.0) {
      return;
    }
    let halfChord = sqrt(q.disc) / q.qa;
    let edgeNearestFromStar = (kE - 1.0) * (kE + 1.0) * ray.dir.z * acrossZ / q.qa;
    // t*_e from t_low: (t*_e - t_o) + (t_o - t*) - (t_low - t*).
    let centre = (nearestFromStar - edgeNearestFromStar) - ray.lowestFromStar;
    marchWalkCross(walk, centre - halfChord);
    marchWalkCross(walk, centre + halfChord);
    return;
  }
  let k = a / c;
  let kGap = edgeM * (c - a) / (c * (c + edgeM));
  let z2 = ray.origin.z * ray.origin.z;
  let qc = (view.exact.x + kGap * (kE + k) * z2) - edgeM * (2.0 * a + edgeM);
  let q = rayQuadric(ray.origin, ray.dir, kE, a + edgeM, qc, false);
  if (q.disc < 0.0) {
    return;
  }
  let hits = quadricHits(q);
  let lowestM = (*walk).split.lowestM;
  marchWalkCross(walk, hits.x - lowestM);
  marchWalkCross(walk, hits.y - lowestM);
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
  let topM = medium.topHeightM;
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
  let k = a / c;
  // Beyond half the equatorial radius, the ground's quadric is taken about the closest approach.
  let far = view.camera.w > 0.5 * a;
  let shellQ = rayQuadric(origin, dir, (a + topM) / (c + topM), a + topM, view.exact.y, far);
  let groundQ = rayQuadric(origin, dir, k, a, view.exact.x, far);
  let shell = quadricHits(shellQ);
  let ground = quadricHits(groundQ);
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
  let samples = max(u32(view.figure.w), 2u);
  let nearest = -dot(origin, dir);
  let split = marchSplit(tStart, tEnd, nearest, samples, shell.x <= 0.0);
  // The closest-approach form's offsets: t - t* = (t - t_low) + (t_low - t*), t_low the split's
  // lowest point. Unclamped, t_low is t_o, the unscaled closest approach the split is placed about,
  // and t_o - t* = (k^2 - 1) d_z [d x (P x d)]_z / qa, 0 on a sphere. Clamped at the ground's near
  // root, t_low - t* is that root's own -sqrt(disc) / qa, with no difference of large distances,
  // so that Q is 0 where the segment meets the ground. Clamped elsewhere (a surface drawn before
  // t_o, or a segment's start), t_low - t_o is formed as a difference, which rounds to about
  // eps |P|, the depth's or the shell root's own precision there.
  let qStar = -groundQ.disc / groundQ.qa;
  let acrossZ = cross(dir, cross(origin, dir)).z;
  let nearestFromStar = (k - 1.0) * (k + 1.0) * dir.z * acrossZ / groundQ.qa;
  var lowestFromStar = (split.lowestM - nearest) + nearestFromStar;
  if (hitsGround && split.lowestM == ground.x) {
    lowestFromStar = -sqrt(max(groundQ.disc, 0.0)) / groundQ.qa;
  }
  let count = skySourceCount(view);
  var sources : array<vec3f, MAX_SKY_SOURCES>;
  for (var k = 0u; k < count; k++) {
    sources[k] = view.sources[k].direction.xyz;
  }
  let angles = sourceAngles(dir, sources, count);
  let ray = MarchRay(origin, dir, a, c, far, groundQ, qStar, lowestFromStar, a * a, sources, angles);
  var march = viewMarchStart();
  // A medium with no layer edge takes R05.T12.e's steps as they were, and declares no walk; the
  // edges are the same for every pixel, so the branch is uniform (R08.T6.i).
  let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);
  if (edgeCount == 0u) {
    for (var i = 0u; i < samples; i++) {
      rayMarchStep(&march, ray, marchStep(split, i));
    }
  } else {
    var walk : MarchWalk;
    marchWalkStart(&walk, split);
    for (var e = 0u; e < edgeCount; e++) {
      let edgeM = layerEdges.middles[e / 4u][e % 4u];
      marchWalkCrossShell(&walk, ray, nearestFromStar, acrossZ, edgeM);
    }
    marchWalkReady(&walk);
    for (var i = 0u; i < samples; i++) {
      rayMarchStep(&march, ray, marchWalkStep(&walk, i));
    }
  }
  var luminance = march.luminance;
  // The bare ground under the atmosphere, seen from above it where nothing is drawn: each source's
  // light on it, weighted as the sky is, through its own slot's throughput (R08.T7).
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    let p = origin + ground.x * dir;
    let normal = normalAt(p, a, c);
    for (var k = 0u; k < count; k++) {
      let source = view.sources[k];
      let slot = skySlotOf(source);
      let muSun = dot(source.direction.xyz, normal);
      let toSun = tableTransmittance(transmittance, view.tables.x, 0.0, muSun, slot);
      luminance += march.throughput[slot] * source.weight.rgb * toSun * max(muSun, 0.0)
        * medium.groundAlbedo.rgb / PI_VIEW;
    }
  }
  // A sky pixel whose ray meets the ground sees nothing of space behind it: no sun's disc there.
  var meanTransmittance = viewMeanTransmittance(march);
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    meanTransmittance = 0.0;
  }
  textureStore(rayMarchOut, id.xy, vec4f(luminance, meanTransmittance));
}

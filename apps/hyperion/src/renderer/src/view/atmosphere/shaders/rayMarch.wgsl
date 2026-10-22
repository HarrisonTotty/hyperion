// The per-pixel ray march (plan R05, Design note 16): the light scattered into a pixel's ray,
// summed over the view's drawn sources and weighted by each one's illuminance times the frame's
// scale (plan R08, R08.T7) (rgb), and the ray's mean transmittance (a), where the tables do not
// serve: the sky seen from above the atmosphere, which Hillaire 2020 (section 7) also marches on
// screen, and surfaces beyond the aerial-perspective volume's reach (32 km, his section 5.4), past
// which nothing of his marches. Elsewhere it writes nothing the composite reads. At
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
// from inside the atmosphere to a drawn surface, toward the camera (`marchSplit` in common.wgsl,
// R05.T12.e; every two-sided sky segment under the sun-aware warp since R08.T8, below), where
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
//
// Changes (plan R08, R08.T6.f, Design note 17, _Per-view marches_): the shells stand through h*: the
// top and each layer edge e are the quadrics (a + e / s_e, c + e / s_p), as R08.T12.d's tracer takes
// them, in place of (a + e, c + e). Each sample is read at its height on the datum along the
// similar spheroid's normal, the root of the exact quadratic from its ground quadric
// (`oblate.wgsl`'s datumPointOf), in place of its radial height, which departs from it by
// h (1 / cos psi - 1) (0.5% of h on a Saturn-class figure at 45°), since h* = s h is geodetic; its
// normal is the datum's at that height's foot. The medium is read at h* = s h, s at the sample's
// latitude, and the sources through the sliced lookup and the bands (`source.wgsl`); the bare
// ground is lit through the lookup from its own place. As `tablesCpu.ts`'s oblateRayMarchTwin.
//
// Changes (plan R08, R08.T8): the march also draws every pixel from inside the air once the camera's
// h* reaches the setting's march band (`view.exact.w` above 0, `hillaire.ts`'s marchWeightOf), where
// the composite blends it with the sky view and the aerial-perspective volume, as it draws every
// pixel from above the top. Every two-sided sky segment it marches (nothing drawn along it), from
// inside the air or outside it, takes the sun-aware warp by the first source (`marchSkyWarp`,
// `marchEvenStep`, `marchWalkEvenStep`): even where the Sun is ahead or across the ray, R05's
// one-sided placement from the segment's start where it is behind. From inside, R05.T12.e's gate
// failed both placements on its sky families from the band's cameras (about the lowest point five
// twilight rays with the Sun behind the camera, 17.6% at worst; toward the camera one, 8.12%), and
// the placement about the lowest point from outside failed the night-side and terminator limb (the
// Sun 90-105 degrees from the zenith at the tangent point; 12.19%), where even steps hold them
// (4.32% and 4.85%) and keep the march one rule on both sides of the top, which it would otherwise
// cross with a step of 13-36%. Even steps in turn drew the sky under the horizontal at the camera's
// last light (the Sun just over the camera's visible horizon, behind) 45-124 codes off under a seam
// at the horizontal, which the warp's placement from the start holds within 1.7 codes on high and
// 10.8 on low. One-sided segments and every drawn surface keep R05.T12.e's placement, a drawn
// surface's camera side from inside toward the camera (`decision-r08-t8-leans.md` sections 2, 4 and
// 5.4; plan R08's Risks, "Deviations in T8.a, as built").
//
// Changes (plan R08, R08.T8.c): from above the top, a texel whose footprint meets the shell takes
// the march's mean over its footprint across the limb, blended in by its span (`farLimb.wgsl`;
// `decision-r08-t8-leans.md` section 6.1): from about one scale height a texel one ray cannot stand
// for its footprint, and the limb broke into specks and gaps that flickered as the body moved by a
// fraction of a pixel. A ray's march is `marchRay`, once for the centre and once a node.
//
// Changes (plan R08, R08.T8.d; `decision-r08-t8-leans.md` section 8): no step holds an array or
// takes one by value (source.wgsl's header): `MarchRay` holds neither the sources nor their angles,
// each step reading them from the view (`viewSourcesAt`), and the ground's bounce takes its slot's
// throughput through `viewThroughputOf`; the arithmetic unchanged.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d_array<f32>;
@group(0) @binding(6) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(7) var sceneDepth : texture_depth_2d;
@group(0) @binding(8) var rayMarchOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(10) var absorberCurves : texture_2d<f32>;
@group(0) @binding(11) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(12) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(13) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(0) @binding(14) var lobeTables : texture_2d_array<f32>;
@group(0) @binding(15) var oblateBody : texture_2d<f32>;

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

// A pixel's ray and what its samples' heights are formed from (Design note 18): the camera's
// position and the ray's direction, the datum's radii, 1 / s at the equator and at the pole (the
// shells' scales through h*, R08.T6.f), whether the ground's quadric is taken about the closest
// approach (`far`), that quadric, Q* = -disc / qa there, t_low - t* (the split's lowest point from
// the quadric's own closest approach). The sources' directions and their scattering angles are
// formed at each step from the view (`viewSourcesAt`, R08.T8.d), never held here.
struct MarchRay {
  origin : vec3f,
  dir : vec3f,
  a : f32,
  c : f32,
  equatorialShellScale : f32,
  polarShellScale : f32,
  far : bool,
  groundQ : RayQuadric,
  qStar : f32,
  lowestFromStar : f32,
}

// One step of the march, at its midpoint: its ground quadric, about t* beyond h0 = a / 2 and from
// the camera within it, its datum point (its height and the datum's normal there) and s at its
// latitude, then the medium at h* = s h, its sources through the sliced lookup and their
// in-scattering (R08.T6.f).
fn rayMarchStep(
  march : ptr<function, ViewMarch>,
  ray : MarchRay,
  body : OblateBody,
  stepAt : MarchStep,
) {
  let p = ray.origin + stepAt.tM * ray.dir;
  let groundQ = ray.groundQ;
  var bigQ : f32;
  if (ray.far) {
    let fromStar = stepAt.fromLowestM + ray.lowestFromStar;
    bigQ = groundQ.qa * fromStar * fromStar + ray.qStar;
  } else {
    bigQ = stepAt.tM * (groundQ.qa * stepAt.tM + 2.0 * groundQ.qb) + groundQ.qc;
  }
  let datum = datumPointOf(body, p, bigQ);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let local = sampleMediumAt(s * datum.heightM);
  let start = sampleStartOf(body, p, bigQ, datum.heightM, datum.normal, datum.normal, s, local);
  let shares = viewSourcesAt(body, start, datum.normal, ray.dir, local);
  viewMarchStep(march, local, shares, stepAt.dtM);
}

// Step `i` of a split segment as the ray march steps it (R08.T8): a two-sided sky segment's under
// the sun-aware warp `warp`, every other segment's R05.T12.e's.
fn rayMarchStepOf(split : MarchSplit, i : u32, evenSky : bool, warp : f32) -> MarchStep {
  if (evenSky) {
    return marchEvenStep(split, i, warp);
  }
  return marchStep(split, i);
}

// Step `i` of a segment cut at the medium's layer edges as the ray march steps it (R08.T8): a
// two-sided sky segment's warped walk (made ready by `marchWalkEvenReady`, which holds its warp),
// every other segment's R08.T6.i walk (`marchWalkReady`).
fn rayMarchWalkStepOf(walk : ptr<function, MarchWalk>, i : u32, evenSky : bool) -> MarchStep {
  if (evenSky) {
    return marchWalkEvenStep(walk, i);
  }
  return marchWalkStep(walk, i);
}

// Adds to the walk where the ray crosses the shell of the march edge `edgeM` above the datum in h*
// (plan R08, R08.T6.i and R08.T6.f): the quadric (a + e_a, c + e_c), e_a = e / s_e and e_c = e / s_p,
// as the top's shell (a + H / s_e, c + H / s_p) stands for the top, in space scaled by
// S_e = diag(1, 1, k_e), k_e = (a + e_a) / (c + e_c), crossed at most twice, each crossing as its
// signed distance from t_low, the split's lowest point. Within h0 = a / 2 the crossings are the
// quadric's roots (`quadricHits`), its constant |S_e P|^2 - (a + e_a)^2 formed from the ground's
// q_g, which comes formed in f64, as q_g + (k_e - k)(k_e + k) P_z^2 - e_a (2a + e_a), with
// k_e - k = (c e_a - a e_c) / (c (c + e_c)): no difference of large values (Design note 18). Beyond
// it they stand about the quadric's own closest approach t*_e, at -/+ sqrt(disc) / qa with disc
// from Lagrange's identity, t*_e taken from the ground's t* as t_o - t* is (`nearestFromStar`, with
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
  let edgeA = edgeM * ray.equatorialShellScale;
  let edgeC = edgeM * ray.polarShellScale;
  let kE = (a + edgeA) / (c + edgeC);
  if (ray.far) {
    let q = rayQuadric(ray.origin, ray.dir, kE, a + edgeA, 0.0, true);
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
  let kGap = (c * edgeA - a * edgeC) / (c * (c + edgeC));
  let z2 = ray.origin.z * ray.origin.z;
  let qc = (view.exact.x + kGap * (kE + k) * z2) - edgeA * (2.0 * a + edgeA);
  let q = rayQuadric(ray.origin, ray.dir, kE, a + edgeA, qc, false);
  if (q.disc < 0.0) {
    return;
  }
  let hits = quadricHits(q);
  let lowestM = (*walk).split.lowestM;
  marchWalkCross(walk, hits.x - lowestM);
  marchWalkCross(walk, hits.y - lowestM);
}

// One ray of the march, through `uv` of the output, the scene's reversed-Z depth under the texel
// `depth` (0 where nothing is drawn): its in-scatter (rgb) and mean transmittance (a), (0, 0, 0, 1)
// where it misses the air. The texel's centre ray, and each node of its far-limb footprint
// (R08.T8.c), each at the scene's depth under its own point.
fn marchRay(uv : vec2f, depth : f32) -> vec4f {
  let topM = medium.topHeightM;
  let body = oblateBodyOf(oblateBody);
  var tSurface = 3.4e38;
  if (depth > 0.0) {
    tSurface = distanceAtDepth(view, uv, depth);
  }
  let dir = rayThrough(view, uv);
  let origin = view.camera.xyz;
  let a = view.figure.x;
  let c = view.figure.y;
  let k = a / c;
  // Beyond half the equatorial radius, the ground's quadric is taken about the closest approach.
  let far = view.camera.w > 0.5 * a;
  // The top's shell through h* = H at the equator and the poles (R08.T6.f).
  let topA = topM * body.scales.z;
  let topC = topM * body.scales.w;
  let shellQ = rayQuadric(origin, dir, (a + topA) / (c + topC), a + topA, view.exact.y, far);
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
    return vec4f(0.0, 0.0, 0.0, 1.0);
  }
  let samples = max(u32(view.figure.w), 2u);
  let nearest = -dot(origin, dir);
  // Every two-sided sky segment, from inside the air or outside it, under the sun-aware warp
  // (R08.T8); a drawn surface's camera side from inside toward the camera, and every other segment's
  // sides about its lowest point, R05.T12.e's.
  let split = marchSplit(tStart, tEnd, nearest, samples, shell.x <= 0.0 && depth > 0.0);
  let evenSky = depth == 0.0 && split.beforeM > 0.0 && split.afterM > 0.0;
  let count = viewSourceCount();
  // The warp a, once a pixel: from the march's first source, 0 where no source is drawn.
  var warp = 0.0;
  if (evenSky && count > 0u) {
    warp = marchSkyWarp(dir, view.sources[0].direction.xyz);
  }
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
  let ray = MarchRay(
    origin, dir, a, c, body.scales.z, body.scales.w, far, groundQ, qStar, lowestFromStar
  );
  var march = viewMarchStart();
  // A medium with no layer edge takes R05.T12.e's steps as they were, and declares no walk; the
  // edges are the same for every pixel, so the branch is uniform (R08.T6.i).
  let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);
  // One loop a path, each step chosen in it (R08.T8), so that the step's work is compiled twice,
  // not once a placement and path.
  if (edgeCount == 0u) {
    for (var i = 0u; i < samples; i++) {
      rayMarchStep(&march, ray, body, rayMarchStepOf(split, i, evenSky, warp));
    }
  } else {
    var walk : MarchWalk;
    marchWalkStart(&walk, split);
    for (var e = 0u; e < edgeCount; e++) {
      let edgeM = layerEdges.middles[e / 4u][e % 4u];
      marchWalkCrossShell(&walk, ray, nearestFromStar, acrossZ, edgeM);
    }
    if (evenSky) {
      marchWalkEvenReady(&walk, warp);
    } else {
      marchWalkReady(&walk);
    }
    for (var i = 0u; i < samples; i++) {
      rayMarchStep(&march, ray, body, rayMarchWalkStepOf(&walk, i, evenSky));
    }
  }
  var luminance = march.luminance;
  // The bare ground under the atmosphere, seen from above it where nothing is drawn: each source's
  // light on it through the sliced lookup from its own place (R08.T6.f), weighted as the sky is,
  // through its own slot's throughput (R08.T7), and the source's cosine against the datum's normal
  // there.
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    let p = origin + ground.x * dir;
    let datum = datumPointOf(body, p, 0.0);
    let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
    let local = sampleMediumAt(s * datum.heightM);
    let start = sampleStartOf(body, p, 0.0, datum.heightM, datum.normal, datum.normal, s, local);
    for (var k = 0u; k < count; k++) {
      let source = view.sources[k];
      let slot = skySlotOf(source);
      let muSun = dot(source.direction.xyz, datum.normal);
      let light = sourceLightAt(body, start, source.direction.xyz, slot);
      luminance += viewThroughputOf(march, slot) * source.weight.rgb * light.toSun
        * max(muSun, 0.0) * medium.groundAlbedo.rgb / PI_VIEW;
    }
  }
  // A sky pixel whose ray meets the ground sees nothing of space behind it: no sun's disc there.
  var meanTransmittance = viewMeanTransmittance(march);
  if (depth == 0.0 && hitsGround && ground.x <= tEnd) {
    meanTransmittance = 0.0;
  }
  return vec4f(luminance, meanTransmittance);
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
  let reach = view.tables.y;
  var tSurface = 3.4e38;
  if (depth > 0.0) {
    tSurface = distanceAtDepth(view, uv, depth);
  }
  // Every pixel once the camera's h* reaches the march band, whose weight is 1 at and above the top
  // (R08.T8); below it, the surfaces beyond the aerial-perspective volume's reach.
  let needed = view.exact.w > 0.0 || (depth > 0.0 && tSurface > reach);
  if (!needed) {
    textureStore(rayMarchOut, id.xy, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  // From above the top, a texel whose footprint meets the shell takes its mean across the limb,
  // blended in by its span (R08.T8.c, `farLimb.wgsl`); the camera's h* against the top is the same
  // for every texel, so the branch is uniform.
  var footprint = farLimbNone();
  if (view.camera.w * view.curvature.z >= medium.topHeightM) {
    footprint = farLimbFootprintOf(
      farLimbRaysOf(view), oblateBodyOf(oblateBody), oblateBody, uv, vec2f(size)
    );
  }
  // The centre ray first, then each node where the footprint blends, through one call of the march,
  // so that its registers are its steps' alone, not a second march's beside a held footprint
  // (R08.T8.d); each node formed as it is marched (`farLimbNodeOf`).
  let nodes = select(0u, footprint.count, footprint.blend > 0.0);
  var marched = vec4f(0.0);
  var mean = vec4f(0.0, 0.0, 0.0, footprint.spaceShare);
  for (var j = 0u; j <= nodes; j++) {
    var rayUv = uv;
    var rayDepth = depth;
    var weight = 0.0;
    if (j > 0u) {
      let node = farLimbNodeOf(&footprint, j - 1u);
      // Each node at the scene's depth under its own point, so that a drawn surface's edge across
      // the footprint cuts only the nodes on it.
      let nodeUv = clamp(node.xy, vec2f(0.0), vec2f(1.0));
      let nodeTexel = min(vec2u(nodeUv * vec2f(full)), full - 1u);
      rayUv = node.xy;
      rayDepth = textureLoad(sceneDepth, nodeTexel, 0);
      weight = node.z;
    }
    let one = marchRay(rayUv, rayDepth);
    if (j == 0u) {
      marched = one;
    } else {
      mean += weight * one;
    }
  }
  if (footprint.blend > 0.0) {
    if (footprint.blend >= 1.0) {
      marched = mean;
    } else {
      marched = mix(marched, mean, footprint.blend);
    }
  }
  textureStore(rayMarchOut, id.xy, marched);
}

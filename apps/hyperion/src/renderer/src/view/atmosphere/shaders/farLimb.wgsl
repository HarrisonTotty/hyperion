// The far limb's footprint (plan R08, R08.T8.c; `decision-r08-t8-leans.md` section 6.1). From above
// the top the composite takes the march alone, one ray a texel, and from about one scale height a
// texel that ray cannot stand for the texel's footprint: the limb breaks into specks and gaps that
// flicker as the body moves by a fraction of a pixel. A texel whose footprint meets the shell takes
// the mean over its footprint across the limb, blended in by its span: the ray march's in-scatter
// and mean transmittance, and the composite's per-channel transmittance to space, alike. As
// `tablesCpu.ts`'s `farLimbFootprintTwin`. Follows `common.wgsl`, `view.wgsl` and `oblate.wgsl`.
//
// A ray's tangent height h is its closest approach's height over the datum, taken in space scaled
// to its sphere, S = diag(1, 1, a / c): |S o x S d| / |S d| - a. Across the texel h is taken as
// linear, its gradient (g_x, g_y) from h at the midpoints of the texel's edges, so that its span
// over the texel is |g_x| + |g_y|; the top's cut is the same over the top's shell, the quadric
// (a + H / s_e, c + H / s_p). s is that span over the local scale height at the tangent point,
// the body's table's (`oblateScaleHeightAt`) at h* (h at the centre ray's, held to the ground,
// times s(phi) at its latitude) over s(phi), the table holding it in h*, and the weight w_L = clamp((s - 0.6) / 0.4, 0, 1), continuous in the
// camera, so that nothing pops and no hysteresis is needed. The footprint is projected on the
// screen direction in which h rises, a trapezoid (a box where the limb lies along a texel's side), cut
// where h meets the datum's edge and the top: the ground piece takes 2 Gauss-Legendre nodes, the
// shell piece 8 (the ceiling at every span: 2 under one local scale height put the Saturn-class
// figure's 1 H pixel from 1e8 m at 10.9% of its converged footprint, where 8 hold it at 5.0%), and
// space none, each piece weighted by its share and each node by the projection's density there.
// Texels whose footprint lies wholly on the face, wholly in space, or whose closest approach lies
// behind the camera, and every texel with w_L 0, keep the centre ray. Each node is marched at the
// scene's depth under its own point. h is a difference of radii, which Design note 18 keeps out of
// the march's heights; here it only weighs and cuts the footprint, and f32 rounds it by about
// eps |o| (0.4 m from 400 km, 60 m from 1e9 m, against a texel's 1.4 and 600 km), inside the
// note's 3e-7 max(t, h, 1 km), which moves a cut by 5e-4 of a texel at most.

const FAR_LIMB_BLEND_FROM : f32 = 0.6;
const FAR_LIMB_BLEND_WIDTH : f32 = 0.4;
// The ground piece's nodes and the shell piece's, at most.
const FAR_LIMB_NODES : u32 = 10u;
// The projection's narrower width under which it is taken as a box, texels.
const FAR_LIMB_BOX_WIDTH : f32 = 1e-4;

// Gauss-Legendre nodes and weights on [0, 1] (Abramowitz and Stegun 1964, Table 25.4): x and w.
const FAR_LIMB_GAUSS_2 = array<vec2f, 2>(
  vec2f(0.211324865405187, 0.5),
  vec2f(0.788675134594813, 0.5),
);
const FAR_LIMB_GAUSS_8 = array<vec2f, 8>(
  vec2f(0.019855071751232, 0.050614268145188),
  vec2f(0.101666761293187, 0.111190517226687),
  vec2f(0.237233795041836, 0.156853322938944),
  vec2f(0.408282678752175, 0.181341891689181),
  vec2f(0.591717321247825, 0.181341891689181),
  vec2f(0.762766204958164, 0.156853322938944),
  vec2f(0.898333238706813, 0.111190517226687),
  vec2f(0.980144928248768, 0.050614268145188),
);

// A texel's footprint: w_L (0 for the centre ray alone), the share above the top, and its nodes,
// each the output's coordinates of its ray (xy) and its weight (z).
struct FarLimbFootprint {
  blend : f32,
  spaceShare : f32,
  count : u32,
  nodes : array<vec3f, FAR_LIMB_NODES>,
}

// The height of a line's closest approach over the spheroid of radii (a, a, c), m (x), negative
// where the line meets it, and how far along the unit `dir` from `origin` that approach lies in
// the scaled space (y), positive ahead.
fn farLimbApproach(origin : vec3f, dir : vec3f, a : f32, c : f32) -> vec2f {
  let scale = vec3f(1.0, 1.0, a / c);
  let o = origin * scale;
  let d = dir * scale;
  let qa = dot(d, d);
  let across = cross(o, d);
  return vec2f(sqrt(dot(across, across) / qa) - a, -dot(o, d) / qa);
}

// The tangent height over the datum of the ray through `uv`.
fn farLimbHeightAt(v : AtmosphereView, uv : vec2f) -> f32 {
  return farLimbApproach(v.camera.xyz, rayThrough(v, uv), v.figure.x, v.figure.y).x;
}

// The footprint's projection on the unit screen direction (gx, gy): a sum of uniforms of widths
// |gx| and |gy| texels, hi the wider and lo the narrower.
struct FarLimbProjection {
  hi : f32,
  lo : f32,
  halfWidth : f32,
}

fn farLimbProjectionOf(g : vec2f) -> FarLimbProjection {
  let ax = abs(g.x);
  let ay = abs(g.y);
  let hi = max(ax, ay);
  let lo = min(ax, ay);
  if (lo < FAR_LIMB_BOX_WIDTH) {
    return FarLimbProjection(hi, 0.0, 0.5 * hi);
  }
  return FarLimbProjection(hi, lo, 0.5 * (hi + lo));
}

// The projection's density at xi texels from the centre.
fn farLimbDensity(p : FarLimbProjection, xi : f32) -> f32 {
  if (p.lo == 0.0) {
    return select(0.0, 1.0 / p.hi, abs(xi) <= p.halfWidth);
  }
  return min(max(p.halfWidth - abs(xi), 0.0) / p.lo, 1.0) / p.hi;
}

// The projection's share below t <= 0 on a trapezoid.
fn farLimbShareUnder(p : FarLimbProjection, t : f32) -> f32 {
  let plateau = 0.5 * (p.hi - p.lo);
  if (t <= -plateau) {
    let rise = t + p.halfWidth;
    return rise * rise / (2.0 * p.hi * p.lo);
  }
  return p.lo / (2.0 * p.hi) + (t + plateau) / p.hi;
}

// The projection's share below xi texels from the centre.
fn farLimbShareBelow(p : FarLimbProjection, xi : f32) -> f32 {
  if (p.lo == 0.0) {
    return clamp((xi + p.halfWidth) / p.hi, 0.0, 1.0);
  }
  let t = clamp(xi, -p.halfWidth, p.halfWidth);
  if (t <= 0.0) {
    return farLimbShareUnder(p, t);
  }
  return 1.0 - farLimbShareUnder(p, -t);
}

// Adds a piece [lowXi, highXi] of the footprint, texels along the unit `g` from the centre `uv`, to
// the footprint's nodes: `count` Gauss-Legendre nodes (2 or 8), each weighted by the projection's
// density there, the piece's weights scaled to its share.
fn farLimbAddPiece(
  footprint : ptr<function, FarLimbFootprint>,
  p : FarLimbProjection,
  lowXi : f32,
  highXi : f32,
  count : u32,
  uv : vec2f,
  g : vec2f,
  size : vec2f,
) {
  let share = farLimbShareBelow(p, highXi) - farLimbShareBelow(p, lowXi);
  if (!(highXi > lowXi) || !(share > 0.0)) {
    return;
  }
  var rule2 = FAR_LIMB_GAUSS_2;
  var rule8 = FAR_LIMB_GAUSS_8;
  var raw : array<f32, 8>;
  var total = 0.0;
  for (var i = 0u; i < count; i++) {
    var node = rule8[i];
    if (count == 2u) {
      node = rule2[i];
    }
    let xi = lowXi + node.x * (highXi - lowXi);
    raw[i] = node.y * farLimbDensity(p, xi);
    total += raw[i];
  }
  for (var i = 0u; i < count; i++) {
    var node = rule8[i];
    if (count == 2u) {
      node = rule2[i];
    }
    let xi = lowXi + node.x * (highXi - lowXi);
    var weight = share * node.y;
    if (total > 0.0) {
      weight = share * raw[i] / total;
    }
    (*footprint).nodes[(*footprint).count] = vec3f(uv + xi * g / size, weight);
    (*footprint).count += 1u;
  }
}

// The footprint of the texel whose centre is `uv` in a target of `size` texels, the camera above
// the top (the caller's test): w_L 0 where the texel keeps its centre ray.
fn farLimbFootprintOf(
  v : AtmosphereView,
  body : OblateBody,
  table : texture_2d<f32>,
  uv : vec2f,
  size : vec2f,
) -> FarLimbFootprint {
  var footprint : FarLimbFootprint;
  footprint.blend = 0.0;
  footprint.spaceShare = 0.0;
  footprint.count = 0u;
  let origin = v.camera.xyz;
  let a = v.figure.x;
  let c = v.figure.y;
  let topM = body.counts.w;
  let dir = rayThrough(v, uv);
  let centre = farLimbApproach(origin, dir, a, c);
  if (!(centre.y > 0.0)) {
    return footprint;
  }
  let halfTexel = 0.5 / size;
  let gx = farLimbHeightAt(v, uv + vec2f(halfTexel.x, 0.0))
    - farLimbHeightAt(v, uv - vec2f(halfTexel.x, 0.0));
  let gy = farLimbHeightAt(v, uv + vec2f(0.0, halfTexel.y))
    - farLimbHeightAt(v, uv - vec2f(0.0, halfTexel.y));
  let spanM = abs(gx) + abs(gy);
  let topHeightM = farLimbApproach(
    origin, dir, a + topM * body.scales.z, c + topM * body.scales.w
  ).x;
  if (!(centre.x + 0.5 * spanM > 0.0) || !(topHeightM - 0.5 * spanM < 0.0) || !(spanM > 0.0)) {
    return footprint;
  }
  // s at the tangent point's latitude, from its place's geocentric sine.
  let tangent = origin + centre.y * dir;
  let sin2 = tangent.z * tangent.z / dot(tangent, tangent);
  let s = gravityRatioAt(body, sin2);
  let scaleM = oblateScaleHeightAt(body, table, s * max(centre.x, 0.0));
  // The table's H is in h*, s times the geometric height's: the span over H / s.
  let blend = clamp((spanM * s / scaleM - FAR_LIMB_BLEND_FROM) / FAR_LIMB_BLEND_WIDTH, 0.0, 1.0);
  if (!(blend > 0.0)) {
    return footprint;
  }
  let gLength = length(vec2f(gx, gy));
  let g = vec2f(gx, gy) / gLength;
  let p = farLimbProjectionOf(g);
  // h rises along g by gLength a texel: under the datum's edge below xi_G, over the top above xi_T.
  let groundXi = clamp(-centre.x / gLength, -p.halfWidth, p.halfWidth);
  let topXi = clamp(-topHeightM / gLength, groundXi, p.halfWidth);
  footprint.blend = blend;
  footprint.spaceShare = 1.0 - farLimbShareBelow(p, topXi);
  farLimbAddPiece(&footprint, p, -p.halfWidth, groundXi, 2u, uv, g, size);
  farLimbAddPiece(&footprint, p, groundXi, topXi, 8u, uv, g, size);
  return footprint;
}

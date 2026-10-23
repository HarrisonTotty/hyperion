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
// shell piece 8 (2 under one local scale height put the Saturn-class figure's 1 H pixel from 1e8 m
// at 10.9% of its converged footprint, where 8 hold it at 5.0%) but where its local scale height is
// held to the room left to the top (top - h* <= 1.01 H at the centre's tangent point), where it
// takes 2 up to 2 H tall and 4 up to 4 H (since R08.T8.e, lever 4 of `decision-r08-t8-leans.md`
// section 11; `tablesCpu.ts`'s FAR_LIMB_SCHEDULE), and space none, each piece weighted by its share
// and each node by the projection's density there.
// Texels whose footprint lies wholly on the face, wholly in space, or whose closest approach lies
// behind the camera, and every texel with w_L 0, keep the centre ray. Each node is marched at the
// scene's depth under its own point. h is a difference of radii, which Design note 18 keeps out of
// the march's heights; here it only weighs and cuts the footprint, and f32 rounds it by about
// eps |o| (0.4 m from 400 km, 60 m from 1e9 m, against a texel's 1.4 and 600 km), inside the
// note's 3e-7 max(t, h, 1 km), which moves a cut by 5e-4 of a texel at most.

const FAR_LIMB_BLEND_FROM : f32 = 0.6;
const FAR_LIMB_BLEND_WIDTH : f32 = 0.4;
// The projection's narrower width under which it is taken as a box, texels.
const FAR_LIMB_BOX_WIDTH : f32 = 1e-4;

// Gauss-Legendre nodes and weights on [0, 1] (Abramowitz and Stegun 1964, Table 25.4): x and w.
const FAR_LIMB_GAUSS_2 = array<vec2f, 2>(
  vec2f(0.211324865405187, 0.5),
  vec2f(0.788675134594813, 0.5),
);
const FAR_LIMB_GAUSS_4 = array<vec2f, 4>(
  vec2f(0.069431844202974, 0.173927422568727),
  vec2f(0.330009478207572, 0.326072577431273),
  vec2f(0.669990521792428, 0.326072577431273),
  vec2f(0.930568155797026, 0.173927422568727),
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

// The footprint's projection on the unit screen direction (gx, gy): a sum of uniforms of widths
// |gx| and |gy| texels, hi the wider and lo the narrower.
struct FarLimbProjection {
  hi : f32,
  lo : f32,
  halfWidth : f32,
}

// The shell piece's schedule (R08.T8.e, lever 4; `tablesCpu.ts`'s FAR_LIMB_ROOM_SHARE,
// FAR_LIMB_TWO_NODE_SCALE_HEIGHTS and FAR_LIMB_FOUR_NODE_SCALE_HEIGHTS): the room left to the top,
// in local scale heights, under which H is held to it, and the piece's tallest extents, in local
// scale heights, for 2 and 4 nodes there.
const FAR_LIMB_ROOM_SHARE : f32 = 1.01;
const FAR_LIMB_TWO_NODE_SCALE_HEIGHTS : f32 = 2.0;
const FAR_LIMB_FOUR_NODE_SCALE_HEIGHTS : f32 = 4.0;

// Node i of the `count`-node rule above, 2, 4 or 8 (`farLimbAddPiece` before R08.T8.d held the rules
// in arrays it indexed by a loop variable, which Tint copies whole into each invocation): each read
// by a constant index.
fn farLimbGaussNode(count : u32, i : u32) -> vec2f {
  if (count == 2u) {
    if (i == 0u) {
      return FAR_LIMB_GAUSS_2[0];
    }
    return FAR_LIMB_GAUSS_2[1];
  }
  if (count == 4u) {
    switch (i) {
      case 0u: {
        return FAR_LIMB_GAUSS_4[0];
      }
      case 1u: {
        return FAR_LIMB_GAUSS_4[1];
      }
      case 2u: {
        return FAR_LIMB_GAUSS_4[2];
      }
      default: {
        return FAR_LIMB_GAUSS_4[3];
      }
    }
  }
  switch (i) {
    case 0u: {
      return FAR_LIMB_GAUSS_8[0];
    }
    case 1u: {
      return FAR_LIMB_GAUSS_8[1];
    }
    case 2u: {
      return FAR_LIMB_GAUSS_8[2];
    }
    case 3u: {
      return FAR_LIMB_GAUSS_8[3];
    }
    case 4u: {
      return FAR_LIMB_GAUSS_8[4];
    }
    case 5u: {
      return FAR_LIMB_GAUSS_8[5];
    }
    case 6u: {
      return FAR_LIMB_GAUSS_8[6];
    }
    default: {
      return FAR_LIMB_GAUSS_8[7];
    }
  }
}

// A piece [lowXi, highXi] of a footprint, texels along its screen direction from its centre: its
// nodes' count (0 for a piece left out), its share of the footprint, and its nodes' densities
// summed, from which each node's weight is formed when asked (`farLimbNodeOf`).
struct FarLimbPiece {
  lowXi : f32,
  highXi : f32,
  count : u32,
  share : f32,
  total : f32,
}

// A texel's footprint: w_L (0 for the centre ray alone), the share above the top, its nodes' count,
// and what forms each node (`farLimbNodeOf`): the projection, the centre `uv`, the unit screen
// direction `g` in which h rises, the target's size, and the ground piece and the shell piece, in
// that order. No node is held (R08.T8.d): an array of them, held across the march of each, kept
// the ray march's registers from its steps.
struct FarLimbFootprint {
  blend : f32,
  spaceShare : f32,
  count : u32,
  projection : FarLimbProjection,
  uv : vec2f,
  g : vec2f,
  size : vec2f,
  ground : FarLimbPiece,
  shell : FarLimbPiece,
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

// What a footprint takes from the view: the camera, its ray basis and the datum's radii, so that no
// footprint helper takes the whole view, with its sources' array, by value (R08.T8.d).
struct FarLimbRays {
  camera : vec3f,
  right : vec3f,
  upRay : vec3f,
  forward : vec3f,
  a : f32,
  c : f32,
}

fn farLimbRaysOf(v : AtmosphereView) -> FarLimbRays {
  return FarLimbRays(v.camera.xyz, v.right.xyz, v.upRay.xyz, v.forward.xyz, v.figure.x, v.figure.y);
}

// The unit ray through `uv`, as `view.wgsl`'s rayThrough forms it.
fn farLimbRayThrough(rays : FarLimbRays, uv : vec2f) -> vec3f {
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  return normalize(rays.forward + ndc.x * rays.right + ndc.y * rays.upRay);
}

// The tangent height over the datum of the ray through `uv`.
fn farLimbHeightAt(rays : FarLimbRays, uv : vec2f) -> f32 {
  return farLimbApproach(rays.camera, farLimbRayThrough(rays, uv), rays.a, rays.c).x;
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

// A piece [lowXi, highXi] of the footprint: `count` Gauss-Legendre nodes (2, 4 or 8), each weighted by
// the projection's density there, the piece's weights scaled to its share; no nodes, and so no
// piece, where it is empty or holds no share.
fn farLimbPieceOf(p : FarLimbProjection, lowXi : f32, highXi : f32, count : u32) -> FarLimbPiece {
  let share = farLimbShareBelow(p, highXi) - farLimbShareBelow(p, lowXi);
  if (!(highXi > lowXi) || !(share > 0.0)) {
    return FarLimbPiece(lowXi, highXi, 0u, share, 0.0);
  }
  var total = 0.0;
  for (var i = 0u; i < count; i++) {
    let node = farLimbGaussNode(count, i);
    let xi = lowXi + node.x * (highXi - lowXi);
    let raw = node.y * farLimbDensity(p, xi);
    total += raw;
  }
  return FarLimbPiece(lowXi, highXi, count, share, total);
}

// A footprint with none: the centre ray alone.
fn farLimbNone() -> FarLimbFootprint {
  var footprint : FarLimbFootprint;
  footprint.blend = 0.0;
  footprint.spaceShare = 0.0;
  footprint.count = 0u;
  return footprint;
}

// Node n of the footprint, n under its count, the ground piece's first: the output's coordinates of
// its ray (xy) and its weight (z), as `farLimbAddPiece` formed and held it before R08.T8.d.
fn farLimbNodeOf(footprint : ptr<function, FarLimbFootprint>, n : u32) -> vec3f {
  var piece = (*footprint).ground;
  var i = n;
  if (n >= piece.count) {
    i = n - piece.count;
    piece = (*footprint).shell;
  }
  let node = farLimbGaussNode(piece.count, i);
  let xi = piece.lowXi + node.x * (piece.highXi - piece.lowXi);
  var weight = piece.share * node.y;
  if (piece.total > 0.0) {
    let raw = node.y * farLimbDensity((*footprint).projection, xi);
    weight = piece.share * raw / piece.total;
  }
  return vec3f((*footprint).uv + xi * (*footprint).g / (*footprint).size, weight);
}

// The footprint of the texel whose centre is `uv` in a target of `size` texels, the camera above
// the top (the caller's test): w_L 0 where the texel keeps its centre ray.
fn farLimbFootprintOf(
  rays : FarLimbRays,
  body : OblateBody,
  table : texture_2d<f32>,
  uv : vec2f,
  size : vec2f,
) -> FarLimbFootprint {
  var footprint = farLimbNone();
  let origin = rays.camera;
  let a = rays.a;
  let c = rays.c;
  let topM = body.counts.w;
  let dir = farLimbRayThrough(rays, uv);
  let centre = farLimbApproach(origin, dir, a, c);
  if (!(centre.y > 0.0)) {
    return footprint;
  }
  let topHeightM = farLimbApproach(
    origin, dir, a + topM * body.scales.z, c + topM * body.scales.w
  ).x;
  // A bound on the texel's span from the centre ray alone (R08.T8.d), so that a texel that cannot
  // reach the shell keeps its centre ray without forming its edges' four rays. Along the straight
  // path between two unnormalised rays w = forward + x right + y upRay, |w| >= 1 (|forward| = 1,
  // right and upRay perpendicular to it), so the scaled unit ray turns by at most k |Δw|, k = a ÷ c
  // (a >= c: no prolate figure is drawn), and the tangent height moves by at most |S o| k |Δw|;
  // across the texel's edges' midpoints |Δw| is 2 |right| ÷ size.x and 2 |upRay| ÷ size.y.
  // Widened by 1% and by 10⁻⁵ |S o| for f32's rounding of the span itself (a few 10⁻⁶ |S o| at
  // WGSL's bounds, about 5 × 10⁻⁷ typically), so that it passes every texel the span's own test
  // below passes.
  let scaledOrigin = origin * vec3f(1.0, 1.0, a / c);
  let reachM = length(scaledOrigin) * (a / c) * 2.0
    * (length(rays.right) / size.x + length(rays.upRay) / size.y);
  let boundM = reachM * 1.01 + 1e-5 * length(scaledOrigin);
  if (!(centre.x + 0.5 * boundM > 0.0) || !(topHeightM - 0.5 * boundM < 0.0)) {
    return footprint;
  }
  let halfTexel = 0.5 / size;
  let gx = farLimbHeightAt(rays, uv + vec2f(halfTexel.x, 0.0))
    - farLimbHeightAt(rays, uv - vec2f(halfTexel.x, 0.0));
  let gy = farLimbHeightAt(rays, uv + vec2f(0.0, halfTexel.y))
    - farLimbHeightAt(rays, uv - vec2f(0.0, halfTexel.y));
  let spanM = abs(gx) + abs(gy);
  if (!(centre.x + 0.5 * spanM > 0.0) || !(topHeightM - 0.5 * spanM < 0.0) || !(spanM > 0.0)) {
    return footprint;
  }
  // s at the tangent point's latitude, from its place's geocentric sine.
  let tangent = origin + centre.y * dir;
  let sin2 = tangent.z * tangent.z / dot(tangent, tangent);
  let s = gravityRatioAt(body, sin2);
  let hStar = s * max(centre.x, 0.0);
  let scaleM = oblateScaleHeightAt(body, table, hStar);
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
  footprint.projection = p;
  footprint.uv = uv;
  footprint.g = g;
  footprint.size = size;
  footprint.ground = farLimbPieceOf(p, -p.halfWidth, groundXi, 2u);
  // The shell piece's nodes (R08.T8.e, lever 4): 8, but where H is held to the room left to the top
  // 2 up to 2 H tall and 4 up to 4 H; a NaN takes 8, as the twin's comparisons give it.
  let shellExtent = (topXi - groundXi) * gLength / scaleM;
  var shellCount = 8u;
  if (topM - hStar <= FAR_LIMB_ROOM_SHARE * scaleM) {
    if (shellExtent <= FAR_LIMB_TWO_NODE_SCALE_HEIGHTS) {
      shellCount = 2u;
    } else if (shellExtent <= FAR_LIMB_FOUR_NODE_SCALE_HEIGHTS) {
      shellCount = 4u;
    }
  }
  footprint.shell = farLimbPieceOf(p, groundXi, topXi, shellCount);
  footprint.count = footprint.ground.count + footprint.shell.count;
  return footprint;
}

// The terrain's shadows on itself from its horizon maps (plan R10, R10.T8.b; Design note 10;
// decision-r10-t8a §1 and §2): the sun's grid azimuth at a texel, the horizon there from the
// patch's map blended to its parent's by the morph factor, and that horizon as R07's
// `sphere_irradiance` takes its `horizon` argument. A library composed after frame.wgsl,
// patchVertex.wgsl (`SlotRecord`, `stToUv`), R08's surface lighting (preciseTrig.wgsl's
// `preciseAtan2`) and terrainPatch.wgsl (`VERTICES_PER_SIDE`), before terrainLit.wgsl, which calls
// it. The TypeScript twins are `shadows/gridAzimuth.ts` (the azimuth), `shadows/horizonLookup.ts`
// (the lookup and the blend) and `shadows/litFraction.ts` (the horizon term).
//
// The angles go through `preciseAtan2`, never WGSL's `atan2`, whose 4,096 ULP is about 1 mrad near
// ±π, some five of the map's steps between knots 10° apart, nor `asin` or `acos` (6.8 × 10⁻⁵
// absolute); R07's `sphere_irradiance`, which the horizon enters, kept the built-ins (R10's Risks,
// "Deferred findings (R10.T8.b)"; R08 Design note 18; decision-r10-t8a §2) until R07.T6.d gave it
// the precise forms too.

// Every slot's horizon map, HORIZON_TEXELS `vec4u` a slot (`slotLayout.ts`' `horizon-map`, the
// bake's layout, `hyperion_surface::bake::horizon`): the near field's 65 × 65 vertices row by row,
// then the far field's 9 × 9 points, point (a, b) over vertex (8a, 8b). Word w of a texel holds
// azimuth 2w in its low half and 2w + 1 in its high half, each e ÷ 90° as `pack2x16snorm` packs it;
// azimuth k looks along the patch grid's step `AZIMUTH_STEPS[k]`, at k × 45° of grid azimuth. Read
// by the fragment stage alone (decision-r10-t10b-r09-t2e §5).
@group(2) @binding(16) var<storage, read> horizon_map : array<vec4u>;

// A slot's texels: 65 × 65 near, then 9 × 9 far.
const HORIZON_TEXELS : u32 = 4306u;
const HORIZON_FAR_TEXEL : u32 = 4225u;
const HORIZON_FAR_SIDE : u32 = 9u;
// `uniforms.ts`' NO_PARENT_SLOT: no parent's map to blend to (a root, a parent not resident, or
// one whose slot holds no map).
const HORIZON_NO_PARENT : u32 = 0xffffffffu;
// 45°, an azimuth's step.
const HORIZON_EIGHTH_TURN : f32 = 0.785398163397448;
// The share of the star's direction, squared, under which its part across the ground is taken as
// none, so that the azimuth is any (0): |t| ≤ 10⁻⁶ |s|, some eight roundings of f32, where the star
// stands within 0.2″ of the zenith and above every horizon.
const HORIZON_ZENITH_SHARE : f32 = 1e-12;

// A face's centre axis and its u and v axes (`hyperion_surface::cube::face_uv_to_xyz`: the point
// (u, v) is centre + u U + v V).
struct HorizonFace {
  centre : vec3f,
  u : vec3f,
  v : vec3f,
}

fn horizon_face(face : u32) -> HorizonFace {
  switch face {
    case 0u: {
      return HorizonFace(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), vec3f(0.0, 0.0, 1.0));
    }
    case 1u: {
      return HorizonFace(vec3f(0.0, 1.0, 0.0), vec3f(-1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0));
    }
    case 2u: {
      return HorizonFace(vec3f(0.0, 0.0, 1.0), vec3f(-1.0, 0.0, 0.0), vec3f(0.0, -1.0, 0.0));
    }
    case 3u: {
      return HorizonFace(vec3f(-1.0, 0.0, 0.0), vec3f(0.0, 0.0, -1.0), vec3f(0.0, -1.0, 0.0));
    }
    case 4u: {
      return HorizonFace(vec3f(0.0, -1.0, 0.0), vec3f(0.0, 0.0, -1.0), vec3f(1.0, 0.0, 0.0));
    }
    default: {
      return HorizonFace(vec3f(0.0, 0.0, -1.0), vec3f(0.0, 1.0, 0.0), vec3f(1.0, 0.0, 0.0));
    }
  }
}

// The face of a slot record, from its centre axis, ±1 along one body-fixed axis: x, y, z for faces
// 0 to 2, and 3 more for the negative ones.
fn horizon_face_of(rec : SlotRecord) -> u32 {
  let a = rec.axisA;
  let axis = select(select(2u, 1u, abs(a.y) > 0.5), 0u, abs(a.x) > 0.5);
  return axis + select(0u, 3u, a.x + a.y + a.z < 0.0);
}

// Vertex (x, y)'s cell coordinates (s, t) on its patch's face, from the record's centre (s₀, t₀)
// and its step, linear across a quad, so that a fragment's are its own: to f32's spacing, which
// past level 18 is coarser than a vertex step and moves the azimuth by under 10⁻⁷ rad.
fn horizon_face_st(rec : SlotRecord, x : u32, y : u32) -> vec2f {
  return vec2f(rec.s0, rec.t0) + (vec2f(f32(x), f32(y)) - vec2f(32.0)) * rec.step;
}

// du/ds of S2's quadratic warp at s: 8s ÷ 3 for s at least ½, 8 (1 − s) ÷ 3 below.
fn horizon_warp_rate(s : f32) -> f32 {
  return select(8.0 * (1.0 - s) / 3.0, 8.0 * s / 3.0, s >= 0.5);
}

// The grid azimuth of the unit direction `towards` at cell coordinates `st` of `face`, in eighths
// of a turn, [0, 8], as `hyperion_surface::bake::horizon::grid_azimuth` takes it on the patch's own
// face (never the face the direction falls on, which a skirt's or an edge's fragment may lie just
// past): with `towards`' part across the ground a ∂p/∂x + b ∂p/∂y on S2's grid tangents
// ∂p/∂x ∝ (U − p (p · U)) du/ds and ∂p/∂y ∝ (V − p (p · V)) dv/dt, the angle atan2(b, a), through
// `preciseAtan2`'s half-plane form with the sign of b; 0 where the star is at the zenith or nadir.
fn horizon_grid_azimuth(face : u32, st : vec2f, towards : vec3f) -> f32 {
  let axes = horizon_face(face);
  let p = normalize(axes.centre + stToUv(st.x) * axes.u + stToUv(st.y) * axes.v);
  let across_ground = towards - p * dot(towards, p);
  if (dot(across_ground, across_ground) <= HORIZON_ZENITH_SHARE * dot(towards, towards)) {
    return 0.0;
  }
  let gx = (axes.u - p * dot(p, axes.u)) * horizon_warp_rate(st.x);
  let gy = (axes.v - p * dot(p, axes.v)) * horizon_warp_rate(st.y);
  let xx = dot(gx, gx);
  let xy = dot(gx, gy);
  let yy = dot(gy, gy);
  let wx = dot(gx, across_ground);
  let wy = dot(gy, across_ground);
  // The solve of [xx xy; xy yy] (a, b) = (wx, wy), times its positive determinant.
  let a = yy * wx - xy * wy;
  let b = xx * wy - xy * wx;
  if (!(max(abs(a), abs(b)) > 0.0)) {
    return 0.0;
  }
  let half_turn = preciseAtan2(abs(b), a);
  let phi = select(half_turn, 2.0 * TRIG_PI - half_turn, b < 0.0);
  return phi / HORIZON_EIGHTH_TURN;
}

// The two azimuths about a grid azimuth and its fraction of the way from the first to the second.
struct HorizonKnots {
  k0 : u32,
  k1 : u32,
  between : f32,
}

// The knots of a grid azimuth of `eighths` eighths of a turn, [0, 8]: 8 is 0.
fn horizon_knots(eighths : f32) -> HorizonKnots {
  let whole = floor(eighths);
  let k0 = u32(whole) % 8u;
  return HorizonKnots(k0, (k0 + 1u) % 8u, eighths - whole);
}

// Azimuth k's elevation in a texel's words, radians: `unpack2x16snorm`'s max(c ÷ 32,767, −1) of
// its code, × 90°.
fn horizon_code(words : vec4u, k : u32) -> f32 {
  let pair = unpack2x16snorm(words[k >> 1u]);
  return select(pair.x, pair.y, (k & 1u) == 1u) * TRIG_HALF_PI;
}

// a + (b − a) f, `hyperion_surface::bake::horizon`'s `lerp`.
fn horizon_lerp(a : f32, b : f32, f : f32) -> f32 {
  return a + (b - a) * f;
}

// A texel's horizon towards `knots`: linear in azimuth between the two about it.
fn horizon_texel(texel : u32, knots : HorizonKnots) -> f32 {
  let words = horizon_map[texel];
  return horizon_lerp(horizon_code(words, knots.k0), horizon_code(words, knots.k1), knots.between);
}

// The near field of `slot` at `p`, its patch's vertex coordinates (x, y) in [0, 64]: bilinear
// between the four vertices about p.
fn horizon_near(slot : u32, p : vec2f, knots : HorizonKnots) -> f32 {
  let base = min(vec2u(max(floor(p), vec2f(0.0))), vec2u(VERTICES_PER_SIDE - 2u));
  let f = clamp(p - vec2f(base), vec2f(0.0), vec2f(1.0));
  let first = slot * HORIZON_TEXELS + base.y * VERTICES_PER_SIDE + base.x;
  let e00 = horizon_texel(first, knots);
  let e10 = horizon_texel(first + 1u, knots);
  let e01 = horizon_texel(first + VERTICES_PER_SIDE, knots);
  let e11 = horizon_texel(first + VERTICES_PER_SIDE + 1u, knots);
  return horizon_lerp(horizon_lerp(e00, e10, f.x), horizon_lerp(e01, e11, f.x), f.y);
}

// The far field of `slot` at `p`: bilinear at p ÷ 8 between the four points about it.
fn horizon_far(slot : u32, p : vec2f, knots : HorizonKnots) -> f32 {
  let q = p * 0.125;
  let base = min(vec2u(max(floor(q), vec2f(0.0))), vec2u(HORIZON_FAR_SIDE - 2u));
  let f = clamp(q - vec2f(base), vec2f(0.0), vec2f(1.0));
  let first = slot * HORIZON_TEXELS + HORIZON_FAR_TEXEL + base.y * HORIZON_FAR_SIDE + base.x;
  let e00 = horizon_texel(first, knots);
  let e10 = horizon_texel(first + 1u, knots);
  let e01 = horizon_texel(first + HORIZON_FAR_SIDE, knots);
  let e11 = horizon_texel(first + HORIZON_FAR_SIDE + 1u, knots);
  return horizon_lerp(horizon_lerp(e00, e10, f.x), horizon_lerp(e01, e11, f.x), f.y);
}

// `slot`'s map at `p` towards `knots`: the larger of its near and far fields
// (`HorizonMap::elevation_at` at a vertex).
fn horizon_elevation(slot : u32, p : vec2f, knots : HorizonKnots) -> f32 {
  return max(horizon_near(slot, p, knots), horizon_far(slot, p, knots));
}

// The horizon at `p` of `slot`, blended to its parent's map by the morph factor `morph`, as the
// geometry is (decision-r10-t8a §1): mix(own, parent's, k), the parent's read at its own vertex
// coordinates ((q_x 64 + x) ÷ 2, (q_y 64 + y) ÷ 2), q the patch's quadrant in it, and only where
// k > 0; the own map where `parent.x` is HORIZON_NO_PARENT. `parent` is the instance's parent slot
// and quadrant (bit 0 q_x, bit 1 q_y).
fn horizon_blended(slot : u32, parent : vec2u, p : vec2f, knots : HorizonKnots, morph : f32) -> f32 {
  let own = horizon_elevation(slot, p, knots);
  if (parent.x == HORIZON_NO_PARENT || !(morph > 0.0)) {
    return own;
  }
  let quadrant = vec2f(f32(parent.y & 1u), f32((parent.y >> 1u) & 1u));
  let at = (quadrant * 64.0 + p) * 0.5;
  return mix(own, horizon_elevation(parent.x, at, knots), morph);
}

// `litFraction.ts`' MAX_STAR_DISTANCE_RADII: a star's distance over its radius is taken at most
// 10¹⁸ (ρ of 10⁻¹⁸ rad), where R07's term is a hard step (R10's Risks, "Deviations in T8.b, as
// built"). R07's term formed H² until R07.T6.d, ∞ × 0 for a point star; it forms none now, and the
// cap keeps its 1 ÷ H finite and inside WGSL's division bound, a point star's d ÷ 0 being no value
// WGSL defines (its finite-math assumption, §15.7.2).
const HORIZON_MAX_STAR_DISTANCE_RADII : f32 = 1e18;

// H for R07's `sphere_irradiance`: the star's `distance` over its `radius`, in one unit, at most
// HORIZON_MAX_STAR_DISTANCE_RADII, so that a point star (radius 0) shades as the hard step.
fn terrain_star_distance_radii(distance : f32, radius : f32) -> f32 {
  return min(distance / radius, HORIZON_MAX_STAR_DISTANCE_RADII);
}

// The local horizon towards a star as R07's `sphere_irradiance(h, phi, horizon)` takes it, radians:
// an elevation above the element's own tangent plane in the star's direction, its cone about the
// normal (`lighting/oracle.ts`). The map's horizon e is against the datum's horizontal, so it is
// carried into the element's frame by the star's elevation above the datum less its elevation above
// the element: e − (e★ − (π ÷ 2 − φ)), so that the disc is cut exactly where the star's centre
// stands at e above the datum (Design note 10's x = (e★ − e) ÷ ρ in R07's visible fraction), and
// the element's own plane where it hides more (R07 takes a negative horizon as its plane): exact in
// the share of the disc cut off; the light through it is weighed about n, not ν, an error of at
// most (2 ÷ 3π) ρ sin φ (1 − cos β) of face-on, β the angle at the star between the great circles
// to n and ν (decision-r10-t8b §1). On the
// smooth figure, n = ν, it is e itself. e★ is π ÷ 2 less the star's angle from ν, through
// `preciseAtan2` (R08 Design note 18).
//
// `slot`, `parent`, `face`, `st`, `patch_uv` and `morph` are the texel's (its slot, its parent's
// slot and quadrant, its face, its cell coordinates, its place in its patch ÷ 64, its morph
// factor); `nu` the datum's unit normal, `towards` the unit direction to the star and `phi` its
// angle from the terrain's normal, all body-fixed.
fn terrain_horizon(
  slot : u32,
  parent : vec2u,
  face : u32,
  st : vec2f,
  patch_uv : vec2f,
  morph : f32,
  nu : vec3f,
  towards : vec3f,
  phi : f32,
) -> f32 {
  let knots = horizon_knots(horizon_grid_azimuth(face, st, towards));
  let e = horizon_blended(slot, parent, patch_uv * 64.0, knots, morph);
  // In `angle_between`'s form and operand order, so that with n = ν the two angles are one.
  let star = TRIG_HALF_PI - preciseAtan2(length(cross(nu, towards)), dot(nu, towards));
  return element_horizon(e, star, phi);
}

// The horizon `e` against the datum's horizontal carried into the element's frame towards a star
// at elevation `star` above the datum and angle `phi` from the element's normal, radians:
// e − (e★ − (π ÷ 2 − φ)) (`litFraction.ts`' `elementHorizonRad`; R10's "Questions for main
// (R10.T8.b)").
fn element_horizon(e : f32, star : f32, phi : f32) -> f32 {
  return e - (star - (TRIG_HALF_PI - phi));
}

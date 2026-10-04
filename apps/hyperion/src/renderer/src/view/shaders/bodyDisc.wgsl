// The disc regime of a lit body (plan R07, T8.a and T8.b; Design notes 2, 6, 10, 19 and 24): one
// screen rectangle the CPU bounds, each fragment's rays intersected with the body's spheroid and
// shaded by `body_brdf`'s law under the horizon, eclipse, ring-shadow and atmosphere terms of
// `litBody.wgsl`. The law is the disc's surface's (`DiscSurface`): one law, or under R10's class
// map each class's law by its weight at the hit and the uniform law by what the weights leave.
// Composed after frame.wgsl and litBody.wgsl. The TypeScript twin is `view/bodies/discShading.ts`.
//
// Every length reaches the GPU already divided: the body's centre as a unit direction with its
// radii over its distance D, and every light and occluder relative to the body's centre over its
// equatorial radius a, so that no large number meets another in f32 (Design note 19).
//
// A disc is two draws of this source, which the CPU orders together in the painter's sequence:
// `edgePass` 0 shades the pixels the body covers wholly, opaque, writing the meter class in alpha
// (Design note 10); `edgePass` 1 the limb's partly covered pixels, premultiplied by their coverage
// over what is beneath, whose class they keep (every blend keeps the destination's alpha). No depth
// is written: the painter's order by power stands for it (Design note 2).

struct Draw {
  // Unused: each disc carries its own centre.
  offsetFromCameraM : vec3f,
  // The disc's index in `discs`.
  disc : u32,
  // 0 for the wholly covered pixels, 1 for the limb's.
  edgePass : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// DISC_ROWS rows per disc (`discShading.ts`' `packDiscRecords`), along the galactic axes:
//   0  the screen rectangle, px (left, top, right, bottom);
//   1  the centre's unit direction from the camera; w, a ÷ D;
//   2  the pole's unit direction; w, c ÷ a;
//   3  samples per axis in a wholly covered pixel and in a limb pixel; the lights; the occluders;
//   4  law 0's A per channel (r, g, b), scaled for the figure; w, its L: the uniform law, or a
//      class map's `elsewhere`;
//   5  law 0's row of `phase_factor_table`; the annuli per channel; exposure ÷ π; the class map's
//      classes, 0 for a uniform surface;
//   6 + 8j, light j: the star's unit direction from the body's centre, w its distance ÷ a; its
//      illuminance face-on per channel (lx), w its radius ÷ a; then its annuli (`DiscAnnuli`'s outer
//      edges and fluxes) for r, g and b, two rows each;
//   22 + k, occluder k: its centre from the body's centre ÷ a; w, its radius ÷ a;
//   24, 25  a class map's body-fixed x and y axes (z their cross product);
//   26 + k, class k (law k + 1): its A per channel, scaled; w, its L;
//   42 to 45  the classes' rows of `phase_factor_table`, four a row.
@group(2) @binding(0) var<storage, read> discs : array<vec4f>;

// The phase factors of the frame's laws, one row each (`litBody.wgsl`).
@group(2) @binding(1) var phase_factor_table : texture_2d<f32>;

// The disc's class map (`discSurface.ts`): N × N texels per face of R05's cube sphere, class k's
// weight in channel k mod 4 of layer face + 6 ⌊k ÷ 4⌋, read unfiltered; any one texture where the
// surface is uniform, unread.
@group(2) @binding(2) var class_weights : texture_2d_array<f32>;

const DISC_ROWS : u32 = 46u;
const FIRST_LIGHT_ROW : u32 = 6u;
const LIGHT_ROWS : u32 = 8u;
const FIRST_OCCLUDER_ROW : u32 = 22u;
const AXES_ROW : u32 = 24u;
const FIRST_CLASS_ROW : u32 = 26u;
const FIRST_CLASS_TABLE_ROW : u32 = 42u;
const MAX_DISC_LIGHTS : u32 = 2u;
const MAX_DISC_OCCLUDERS : u32 = 2u;
// `MAX_DISC_CLASSES`, and the laws a disc shades with: law 0 and one per class.
const MAX_DISC_CLASSES : u32 = 16u;
const SURFACE_LAWS : u32 = 17u;

// `METER_CLASS.litBody`, `unlitBody` and `other` (`view/post/meter.ts`).
const METER_LIT_BODY : f32 = 2.0;
const METER_UNLIT_BODY : f32 = 3.0;
const METER_OTHER : f32 = 1.0;

// `HALF_FLOAT_MAX`: the largest value the rgba16float target holds.
const HDR_MAX : f32 = 65504.0;

// A sample is lit directly where its star's horizon and eclipse terms exceed this share of the
// face-on irradiance (`LIT_IRRADIANCE`), so that f32 and f64 agree at the terminator's edge.
const LIT_IRRADIANCE : f32 = 1e-5;

fn disc_row(i : u32) -> vec4f {
  return discs[draw.disc * DISC_ROWS + i];
}

// A direction along the galactic axes in the view's axes.
fn to_view(v : vec3f) -> vec3f {
  return (frame.viewRotation * vec4f(v, 0.0)).xyz;
}

// The ray through a point of the view, px, in view space (looking down −z), unit.
fn view_ray(px : vec2f) -> vec3f {
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  return normalize(vec3f(ndc.x / frame.clipProjection[0][0], ndc.y / frame.clipProjection[1][1], -1.0));
}

// The body as one fragment sees it, in view space and units of D.
struct Body {
  centre : vec3f,
  pole : vec3f,
  // a ÷ c, which scales the polar axis to make the spheroid a sphere of radius a ÷ D.
  stretch : f32,
  radius : f32,
  // The scaled centre, M u.
  scaled_centre : vec3f,
  // A class map's body-fixed axes; 0 for a uniform surface.
  axis_x : vec3f,
  axis_y : vec3f,
  axis_z : vec3f,
}

fn stretch_along(body : Body, x : vec3f) -> vec3f {
  return x + (body.stretch - 1.0) * dot(x, body.pole) * body.pole;
}

fn body_of() -> Body {
  let centre_row = disc_row(1u);
  let pole_row = disc_row(2u);
  var body : Body;
  body.centre = to_view(centre_row.xyz);
  body.pole = normalize(to_view(pole_row.xyz));
  body.stretch = 1.0 / pole_row.w;
  body.radius = centre_row.w;
  body.scaled_centre = stretch_along(body, body.centre);
  body.axis_x = to_view(disc_row(AXES_ROW).xyz);
  body.axis_y = to_view(disc_row(AXES_ROW + 1u).xyz);
  body.axis_z = cross(body.axis_x, body.axis_y);
  return body;
}

// S2's quadratic warp from a face coordinate u ∈ [−1, 1] to a cell coordinate s ∈ [0, 1]
// (`uvToSt`, R05's cube sphere).
fn uv_to_st(u : f32) -> f32 {
  if (u >= 0.0) {
    return 0.5 * sqrt(1.0 + 3.0 * u);
  }
  return 1.0 - 0.5 * sqrt(1.0 - 3.0 * u);
}

// A cell coordinate as a texel index of n.
fn texel_index(s : f32, n : u32) -> u32 {
  return u32(clamp(floor(s * f32(n)), 0.0, f32(n - 1u)));
}

// The class map's texel of a body-fixed direction: (i, j, face), R05's face of the largest
// |component| (ties to the lowest face: +x 0, +y 1, +z 2, −x 3, −y 4, −z 5) and its (u, v) there
// (`xyzToFaceUv`), warped and scaled by n (`classMapTexelOf`).
fn class_map_texel(p : vec3f, n : u32) -> vec3u {
  let faces = vec3u(select(3u, 0u, p.x > 0.0), select(4u, 1u, p.y > 0.0), select(5u, 2u, p.z > 0.0));
  let size = abs(p);
  var face = faces.x;
  var largest = size.x;
  for (var axis = 1u; axis < 3u; axis = axis + 1u) {
    if (size[axis] > largest || (size[axis] == largest && faces[axis] < face)) {
      face = faces[axis];
      largest = size[axis];
    }
  }
  var uv : vec2f;
  switch face {
    case 0u: { uv = vec2f(p.y / p.x, p.z / p.x); }
    case 1u: { uv = vec2f(-p.x / p.y, p.z / p.y); }
    case 2u: { uv = vec2f(-p.x / p.z, -p.y / p.z); }
    case 3u: { uv = vec2f(p.z / p.x, p.y / p.x); }
    case 4u: { uv = vec2f(p.z / p.y, -p.x / p.y); }
    default: { uv = vec2f(-p.y / p.z, -p.x / p.z); }
  }
  return vec3u(texel_index(uv_to_st(uv.x), n), texel_index(uv_to_st(uv.y), n), face);
}

// The share of each of the disc's laws at the point `q` (from the body's centre ÷ a): law 0's
// alone on a uniform surface; under a class map, each class's weight at the texel the point's
// cube-sphere direction falls in (R05's spheroid point of the unit direction d is M d, so d is q
// stretched along the pole), scaled down to sum to 1 where it sums to more, and law 0 (`elsewhere`)
// what they leave (`surfaceShares`).
fn surface_shares(body : Body, q : vec3f, classes : u32) -> array<f32, SURFACE_LAWS> {
  var shares : array<f32, SURFACE_LAWS>;
  shares[0] = 1.0;
  if (classes == 0u) {
    return shares;
  }
  let d = normalize(stretch_along(body, q));
  let p = vec3f(dot(d, body.axis_x), dot(d, body.axis_y), dot(d, body.axis_z));
  let texel = class_map_texel(p, textureDimensions(class_weights).x);
  let count = min(classes, MAX_DISC_CLASSES);
  var total = 0.0;
  for (var group = 0u; group * 4u < count; group = group + 1u) {
    let weights = textureLoad(class_weights, texel.xy, texel.z + 6u * group, 0);
    for (var c = 0u; c < min(4u, count - group * 4u); c = c + 1u) {
      shares[group * 4u + c + 1u] = weights[c];
      total = total + weights[c];
    }
  }
  if (total > 1.0) {
    for (var k = 1u; k <= count; k = k + 1u) {
      shares[k] = shares[k] / total;
    }
  }
  shares[0] = select(1.0 - total, 0.0, total >= 1.0);
  return shares;
}

// Law m of the disc: 0 from rows 4 and 5, class m − 1 from its rows.
fn surface_law(m : u32) -> LunarLambert {
  var law : LunarLambert;
  law.s = vec3f(1.0);
  if (m == 0u) {
    let row = disc_row(4u);
    law.a = row.xyz;
    law.l = row.w;
    law.table_row = u32(disc_row(5u).x);
    return law;
  }
  let k = m - 1u;
  let row = disc_row(FIRST_CLASS_ROW + k);
  law.a = row.xyz;
  law.l = row.w;
  law.table_row = u32(disc_row(FIRST_CLASS_TABLE_ROW + k / 4u)[k % 4u]);
  return law;
}

// Where a ray meets the spheroid: the point from the body's centre ÷ a in xyz, w 1; w 0 for a miss.
// In the scaled space, with r̂ the unit scaled ray and u′ the scaled centre, the ray passes u′ at
// the perpendicular p⊥ = (r̂ × u′) × r̂ − u′'s part across it − and the near hit is
// q′ = −p⊥ − √((a ÷ D)² − |r̂ × u′|²) r̂ from the centre: cross products and a difference of small
// squares, never b² − rr · power, which cancels to 7% in f32 for a body 10⁻³ rad across.
fn hit_spheroid(body : Body, ray : vec3f) -> vec4f {
  let scaled = normalize(stretch_along(body, ray));
  if (dot(scaled, body.scaled_centre) <= 0.0) {
    return vec4f(0.0);
  }
  let across = cross(scaled, body.scaled_centre);
  let h2 = body.radius * body.radius - dot(across, across);
  if (h2 < 0.0) {
    return vec4f(0.0);
  }
  let scaled_hit = -cross(across, scaled) - sqrt(h2) * scaled;
  // Back through M⁻¹ x = x + (c ÷ a − 1)(x · p) p, in units of a.
  let hit = scaled_hit + (1.0 / body.stretch - 1.0) * dot(scaled_hit, body.pole) * body.pole;
  return vec4f(hit / body.radius, 1.0);
}

// One annulus set of light j and channel c (0 r, 1 g, 2 b).
fn light_annuli(j : u32, c : u32, count : u32) -> DiscAnnuli {
  let first = FIRST_LIGHT_ROW + j * LIGHT_ROWS + 2u + 2u * c;
  var annuli : DiscAnnuli;
  annuli.outer = disc_row(first);
  annuli.flux = disc_row(first + 1u);
  annuli.count = count;
  return annuli;
}

// The light a sample reflects towards the camera, and whether any star lights it directly.
struct Shaded {
  radiance : vec3f,
  lit : bool,
}

// The radiance of the point `q` (from the body's centre ÷ a) seen along `ray`, pre-exposed.
fn shade(body : Body, q : vec3f, ray : vec3f) -> Shaded {
  let stretch2 = body.stretch * body.stretch;
  let normal = normalize(q + (stretch2 - 1.0) * dot(q, body.pole) * body.pole);
  let mu = dot(normal, -ray);
  let counts = disc_row(3u);
  let misc = disc_row(5u);
  let annulus_count = u32(misc.y);
  let laws = min(u32(misc.w), MAX_DISC_CLASSES) + 1u;
  var out : Shaded;
  out.radiance = vec3f(0.0);
  out.lit = false;
  if (mu <= 0.0) {
    return out;
  }
  var shares = surface_shares(body, q, laws - 1u);
  // The surface's mean A, for R08's sky light on the Lambert share's flat term.
  var mean_a = vec3f(0.0);
  for (var m = 0u; m < laws; m = m + 1u) {
    if (shares[m] > 0.0) {
      mean_a = mean_a + shares[m] * surface_law(m).a;
    }
  }
  let lights = min(u32(counts.z), MAX_DISC_LIGHTS);
  let occluders = min(u32(counts.w), MAX_DISC_OCCLUDERS);
  var first_mu0 = 0.0;
  for (var j = 0u; j < lights; j = j + 1u) {
    let place = disc_row(FIRST_LIGHT_ROW + j * LIGHT_ROWS);
    let light = disc_row(FIRST_LIGHT_ROW + j * LIGHT_ROWS + 1u);
    let to_star = to_view(place.xyz) * place.w - q;
    let distance = length(to_star);
    let towards = to_star / distance;
    let mu0 = dot(normal, towards);
    if (j == 0u) {
      first_mu0 = mu0;
    }
    // The horizon term: the star's whole disc over the element's tangent plane (Design note 6).
    let horizon = sphere_irradiance(distance / light.w, acos(clamp(mu0, -1.0, 1.0)), 0.0);
    if (horizon <= 0.0) {
      continue;
    }
    let alpha = acos(clamp(dot(towards, -ray), -1.0, 1.0));
    let star_radius = asin(min(1.0, light.w / distance));
    // `body_brdf` with the disc's μ₀ factor replaced by the horizon term, which equals μ₀ wherever
    // the whole star is up and lights the soft band past the terminator; the Lommel–Seeliger
    // term's μ₀ + μ floored at the star's angular radius, so that it stays bounded in that band.
    // Each of the surface's laws by its share.
    let ls_denominator = max(max(mu0, 0.0) + mu, star_radius);
    var reflectance = vec3f(0.0);
    for (var m = 0u; m < laws; m = m + 1u) {
      let share = shares[m];
      if (!(share > 0.0)) {
        continue;
      }
      let law = surface_law(m);
      let disc_term = law.l * 2.0 * horizon / ls_denominator + (1.0 - law.l) * horizon;
      reflectance = reflectance + share * law.a * phase_factor(law.table_row, alpha) * disc_term;
    }
    // The eclipse term, each occluder's hidden fraction added (`eclipseVisible`).
    var visible = vec3f(1.0);
    for (var k = 0u; k < occluders; k = k + 1u) {
      let occluder = disc_row(FIRST_OCCLUDER_ROW + k);
      let to_occluder = to_view(occluder.xyz) - q;
      let occluder_distance = length(to_occluder);
      // An occluder beyond the star hides nothing of it; a point inside one sees none of it.
      if (occluder_distance >= distance) {
        continue;
      }
      if (occluder_distance <= occluder.w) {
        visible = vec3f(0.0);
        break;
      }
      let occluder_radius = asin(occluder.w / occluder_distance);
      // atan2 of the cross and dot products keeps small separations, where acos loses √ε.
      let along = to_occluder / occluder_distance;
      let separation = atan2(length(cross(along, towards)), dot(along, towards));
      for (var c = 0u; c < 3u; c = c + 1u) {
        let kept = eclipse_visible(
          star_radius,
          light_annuli(j, c, annulus_count),
          occluder_radius,
          separation,
        );
        visible[c] = visible[c] - (1.0 - kept);
      }
    }
    visible = max(visible, vec3f(0.0));
    let transmitted = atmosphere_sun_transmittance(0.0, mu0, 0.0, 0.0);
    let ring = ring_shadow_on_body(q, towards);
    out.radiance = out.radiance + light.xyz * misc.z * reflectance * visible * transmitted * ring;
    // Lit directly: the horizon and eclipse terms leave more than LIT_IRRADIANCE of face-on.
    out.lit = out.lit || (horizon > LIT_IRRADIANCE && any(visible > vec3f(LIT_IRRADIANCE)));
  }
  // R08's sky light, 0 until it lands, on the Lambert share's flat term.
  out.radiance = out.radiance
    + mean_a * atmosphere_sky_irradiance(0.0, first_mu0, 0.0) * misc.z;
  return out;
}

struct DiscVarying {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) corner : vec3f) -> DiscVarying {
  let rect = disc_row(0u);
  let px = mix(rect.xy, rect.zw, corner.xy);
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  var out : DiscVarying;
  out.position = vec4f(ndc, 0.0, 1.0);
  return out;
}

// A limb pixel's corners within this many pixels inside the limb still count as on it
// (`LIMB_OVERLAP_PX`): a pixel at the threshold is drawn by both draws, never by neither.
const LIMB_OVERLAP_PX : f32 = 1e-3;
// The step of the limb angle's gradient at a sample, px.
const GRADIENT_STEP_PX : f32 = 0.015625;
// A pixel whose centre is further than this outside the limb, px, holds none of the body.
const OUTSIDE_PX : f32 = 0.75;
// A cell whose inner side is deeper than this many cell widths inside the limb is shaded once.
const NEAR_LIMB_CELLS : f32 = 2.0;

// Three-point Gauss–Legendre on [0, 1], exact to degree 5.
const GAUSS_NODES = array<f32, 3>(0.1127016653792583, 0.5, 0.8872983346207417);
const GAUSS_WEIGHTS = array<f32, 3>(0.2777777777777778, 0.4444444444444444, 0.2777777777777778);

// The signed angle of a ray from the limb in the scaled space, rad: positive outside.
fn limb_angle(body : Body, ray : vec3f) -> f32 {
  let scaled = stretch_along(body, ray);
  let along = dot(scaled, body.scaled_centre);
  let across = length(cross(scaled, body.scaled_centre));
  return atan2(across, along) - asin(body.radius / length(body.scaled_centre));
}

fn angle_at(body : Body, px : vec2f) -> f32 {
  return limb_angle(body, view_ray(px));
}

// One cell's light (weighted), its covered share, and its shaded points and lit ones.
struct CellSum {
  radiance : vec3f,
  coverage : f32,
  lit : u32,
  shaded : u32,
}

fn add_sample(sum : ptr<function, CellSum>, body : Body, px : vec2f, weight : f32) {
  let ray = view_ray(px);
  let hit = hit_spheroid(body, ray);
  if (hit.w == 0.0) {
    return;
  }
  let shaded = shade(body, hit.xyz, ray);
  (*sum).radiance = (*sum).radiance + weight * shaded.radiance;
  (*sum).lit = (*sum).lit + select(0u, 1u, shaded.lit);
  (*sum).shaded = (*sum).shaded + 1u;
}

// A cell of side h = 1 ÷ n about `px`: deep inside, its centre's light; near the limb, the
// integral over its profile across the limb's local normal (a trapezoid, the convolution of boxes
// of widths h|n_x| and h|n_y|) inside the limb, by three Gauss points in u = √δ in each linear
// piece, exact where the light goes as A + B√δ + Cδ with the depth δ (`discShading.ts`).
fn cell_sum(body : Body, px : vec2f, n : u32) -> CellSum {
  var sum : CellSum;
  sum.radiance = vec3f(0.0);
  sum.coverage = 0.0;
  sum.lit = 0u;
  sum.shaded = 0u;
  let angle = angle_at(body, px);
  let sx = (angle_at(body, px + vec2f(GRADIENT_STEP_PX, 0.0)) - angle) / GRADIENT_STEP_PX;
  let sy = (angle_at(body, px + vec2f(0.0, GRADIENT_STEP_PX)) - angle) / GRADIENT_STEP_PX;
  let slope = length(vec2f(sx, sy));
  let h = 1.0 / f32(n);
  if (!(slope > 0.0)) {
    add_sample(&sum, body, px, 1.0);
    sum.coverage = 1.0;
    return sum;
  }
  let normal = vec2f(sx, sy) / slope;
  let half_extent = h * (abs(normal.x) + abs(normal.y)) * 0.5;
  let depth = -angle / slope;
  if (depth - half_extent > NEAR_LIMB_CELLS * h) {
    add_sample(&sum, body, px, 1.0);
    sum.coverage = 1.0;
    return sum;
  }
  let a = h * max(abs(normal.x), abs(normal.y));
  let b = h * min(abs(normal.x), abs(normal.y));
  let top = h * h / a;
  let outer = (a + b) * 0.5;
  let inner = (a - b) * 0.5;
  // The profile's pieces [t0, t1, p(t0), p(t1)]: rising, flat, falling.
  var pieces = array<vec4f, 3>(
    vec4f(-outer, -inner, 0.0, top),
    vec4f(-inner, inner, top, top),
    vec4f(inner, outer, top, 0.0),
  );
  var covered = 0.0;
  for (var k = 0u; k < 3u; k = k + 1u) {
    let piece = pieces[k];
    let t0 = piece.x;
    let t1 = piece.y;
    if (!(t1 > t0) || t0 >= depth) {
      continue;
    }
    let end = min(t1, depth);
    let p_start = piece.z;
    let p_end = piece.z + (piece.w - piece.z) * (end - t0) / (t1 - t0);
    covered = covered + (end - t0) * (p_start + p_end) * 0.5;
    let ua = sqrt(max(depth - end, 0.0));
    let ub = sqrt(max(depth - t0, 0.0));
    for (var g = 0u; g < 3u; g = g + 1u) {
      let u = ua + GAUSS_NODES[g] * (ub - ua);
      let t = depth - u * u;
      let profile = piece.z + (piece.w - piece.z) * (t - t0) / (t1 - t0);
      let weight = GAUSS_WEIGHTS[g] * (ub - ua) * 2.0 * u * profile / (h * h);
      add_sample(&sum, body, px + t * normal, weight);
    }
  }
  sum.coverage = min(1.0, covered / (h * h));
  return sum;
}

// A pixel's cells summed: the light and the coverage as means over its n × n cells.
fn pixel_sum(body : Body, centre_px : vec2f, n : u32) -> CellSum {
  var sum : CellSum;
  sum.radiance = vec3f(0.0);
  sum.coverage = 0.0;
  sum.lit = 0u;
  sum.shaded = 0u;
  for (var i = 0u; i < n; i = i + 1u) {
    for (var j = 0u; j < n; j = j + 1u) {
      let offset = (vec2f(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
      let one = cell_sum(body, centre_px + offset, n);
      sum.radiance = sum.radiance + one.radiance;
      sum.coverage = sum.coverage + one.coverage;
      sum.lit = sum.lit + one.lit;
      sum.shaded = sum.shaded + one.shaded;
    }
  }
  let cells = f32(n * n);
  sum.radiance = sum.radiance / cells;
  sum.coverage = sum.coverage / cells;
  return sum;
}

@fragment
fn fragmentMain(v : DiscVarying) -> @location(0) vec4f {
  let body = body_of();
  let counts = disc_row(3u);
  let pixel = floor(v.position.xy);
  if (dot(body.centre, view_ray(pixel + 0.5)) <= 0.0) {
    discard;
  }
  let c00 = angle_at(body, pixel);
  let c10 = angle_at(body, pixel + vec2f(1.0, 0.0));
  let c01 = angle_at(body, pixel + vec2f(0.0, 1.0));
  let c11 = angle_at(body, pixel + vec2f(1.0, 1.0));
  let gradient = length(vec2f(c10 + c11 - c00 - c01, c01 + c11 - c00 - c10) * 0.5);
  let centre = (c00 + c10 + c01 + c11) * 0.25;
  if (!(gradient > 0.0) || centre / gradient > OUTSIDE_PX) {
    discard;
  }
  let corners = vec4f(c00, c10, c01, c11);
  // Convex: all four corners inside means the body covers the pixel wholly.
  let interior = all(corners < vec4f(0.0));
  let limb = !all(corners / gradient < vec4f(-LIMB_OVERLAP_PX));
  if (draw.edgePass == 0u) {
    if (!interior) {
      discard;
    }
    let sum = pixel_sum(body, v.position.xy, u32(counts.x));
    let light = select(vec3f(0.0), sum.radiance / sum.coverage, sum.coverage > 0.0);
    // The pure-pixel class: lit where every shaded point is, unlit where none is, `other` where
    // they are mixed or no star lights the body.
    var meter = METER_OTHER;
    if (u32(counts.z) > 0u && sum.shaded > 0u) {
      if (sum.lit == sum.shaded) {
        meter = METER_LIT_BODY;
      } else if (sum.lit == 0u) {
        meter = METER_UNLIT_BODY;
      }
    }
    return vec4f(min(light, vec3f(HDR_MAX)), meter);
  }
  if (!limb) {
    discard;
  }
  let sum = pixel_sum(body, v.position.xy, u32(counts.y));
  if (sum.coverage <= 0.0) {
    discard;
  }
  // Premultiplied by the coverage over what is beneath, whose class it keeps.
  return vec4f(min(sum.radiance, vec3f(HDR_MAX)), min(sum.coverage, 1.0));
}

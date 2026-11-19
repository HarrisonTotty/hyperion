// The cloud volume's density (plan R11, R11.T7.c; Design note 10): the cloud map's column at a place
// (`cloudMap.wgsl`), refined to the sample's own footprint by the field's finer octaves, laid
// through the deck by its kind's profile and eroded by the shape noises; the twin of
// `clouds/cloudShape.ts` (whose remarks give the model, and whose `columnFromMap` the refinement)
// and of `clouds/cloudMap.ts`' reading. Its includer declares, at its own group and bindings:
//   - `cloud_map : texture_2d<f32>`, the map, and `cloud_map_sampler : sampler`, linear and
//     `clamp-to-edge`;
//   - `cloud_layer : array<vec4f>`, read-only storage, the body's field's rows (`cloudField.wgsl`'s
//     layout), and `cloud_field_noise : texture_3d<f32>`, the field's noise tile (`cloudNoise.ts`);
//   - `cloud_shape : array<vec4f>`, read-only storage, CLOUD_SHAPE_ROWS rows (`cloudVolumePass.ts`'
//     `packCloudShape`);
//   - `cloud_base_noise : texture_3d<f32>` and `cloud_detail_noise : texture_3d<f32>`, the 128³ and
//     32³ noises, and `cloud_curl_noise : texture_2d<f32>`, the 128² curl potential, each `r8unorm`
//     (`cloudShapeNoise.ts`), with `cloud_noise_repeat : sampler`, linear and `repeat`.

// The shape's rows: 0 x 1 ÷ the base tile, y 1 ÷ the detail tile, z 1 ÷ the curl tile, each m⁻¹,
// w κ; 1 the base noise's drift in its tiles, w its time-rate share; 2 the detail's, w its share; 3
// x and y the base's and the detail's features for their fades, m, z 1 where the deck may be
// cumuliform, w a cumulus cell's feature, m; 4 to 24 the normalisation G at w_b = i ÷ 8, w_d = j ÷ 8,
// at 9 i + j, four a row (`cloudShape.ts`' `normalisationTable`).
const CLOUD_SHAPE_ROW_TILES : u32 = 0u;
const CLOUD_SHAPE_ROW_BASE : u32 = 1u;
const CLOUD_SHAPE_ROW_DETAIL : u32 = 2u;
const CLOUD_SHAPE_ROW_FEATURES : u32 = 3u;
const CLOUD_SHAPE_ROW_TABLE : u32 = 4u;
const CLOUD_SHAPE_ROWS : u32 = 25u;
const CLOUD_SHAPE_TABLE_STEPS : u32 = 9u;

// `cloudShape.ts`' BASE_EROSION, DETAIL_EROSION, EDGE_SHARPNESS, WISP_SHARE, CURL_GAIN and
// CURL_REACH, and the curl noise's side.
const CLOUD_BASE_EROSION : f32 = 0.6;
const CLOUD_DETAIL_EROSION : f32 = 0.4;
const CLOUD_EDGE_SHARPNESS : f32 = 3.0;
const CLOUD_WISP_SHARE : f32 = 0.2;
const CLOUD_CURL_GAIN : f32 = 16.0;
const CLOUD_CURL_REACH : f32 = 0.3;
const CLOUD_CURL_SIZE : f32 = 128.0;

// A frame's map (`cloudMap.ts`' `cloudMapUniforms`, its `Draw` members in order).
struct CloudMapPlane {
  anchor : vec4f,
  east : vec4f,
  north : vec4f,
  origins01 : vec4f,
  origins23 : vec4f,
  origins4 : vec4f,
}

// A column as the map holds it: x f, y the mean τ, z η_top, w s.
fn cloud_shape_row(i : u32) -> vec4f {
  return cloud_shape[i];
}

// A place's coordinates on the map's plane, m, and 1 in z where it is projected, else 0
// (`cloudMap.ts`' `onMapPlane`).
fn cloud_map_plane_at(m : CloudMapPlane, p : vec3f) -> vec3f {
  let r = length(p);
  let c = dot(p, m.anchor.xyz) / r;
  if (!(c > 0.05)) {
    return vec3f(0.0);
  }
  let k = m.anchor.w / (c * r);
  return vec3f(dot(p, m.east.xyz) * k, dot(p, m.north.xyz) * k, 1.0);
}

fn cloud_map_origin(m : CloudMapPlane, level : u32) -> vec2f {
  switch level {
    case 0u: { return m.origins01.xy; }
    case 1u: { return m.origins01.zw; }
    case 2u: { return m.origins23.xy; }
    case 3u: { return m.origins23.zw; }
    default: { return m.origins4.xy; }
  }
}

// The continuous level a read takes (`cloudMap.ts`' `mapLevelAt`); −1 beyond the last.
fn cloud_map_level_at(m : CloudMapPlane, plane : vec2f, footprint : f32) -> f32 {
  let side = m.origins4.z;
  let half0 = 0.5 * side * m.east.w;
  let centre = m.origins01.xy + half0;
  let d = abs(plane - centre);
  let r0 = max(d.x, d.y) / half0;
  let last = m.north.w - 1.0;
  if (r0 / exp2(2.0 * last) > 1.0 - 1.0 / side) {
    return -1.0;
  }
  let by_footprint = 0.5 * log2(max(footprint, 1e-3) / m.east.w);
  let by_extent = 0.5 * log2(max(r0, 1e-9) / m.origins4.w);
  return clamp(max(by_footprint, by_extent), 0.0, last);
}

// One level of the map at a place on the plane, bilinear between its texel centres, held at its edge.
fn cloud_map_read_level(m : CloudMapPlane, level : u32, plane : vec2f) -> vec4f {
  let side = m.origins4.z;
  let size = m.east.w * exp2(2.0 * f32(level));
  let t = clamp((plane - cloud_map_origin(m, level)) / size - 0.5, vec2f(0.0), vec2f(side - 1.0));
  let uv = (t + 0.5) / side;
  let at = vec2f(uv.x, (f32(level) + uv.y) / m.north.w);
  return textureSampleLevel(cloud_map, cloud_map_sampler, at, 0.0);
}

// A read of the map at a place (`CloudMapTwin.readAt`): its texel (x the resolved sum, y z_c, z and w
// the wind eastward and northward), blended between its two nearest levels, their texels' sizes,
// m, and the coarser's share; `valid` false beyond the map.
struct CloudMapRead {
  texel : vec4f,
  fine_m : f32,
  coarse_m : f32,
  blend : f32,
  valid : bool,
}

fn cloud_map_read(m : CloudMapPlane, p : vec3f, footprint : f32) -> CloudMapRead {
  let plane = cloud_map_plane_at(m, p);
  if (plane.z < 0.5) {
    return CloudMapRead(vec4f(0.0), 1.0, 1.0, 0.0, false);
  }
  let level = cloud_map_level_at(m, plane.xy, footprint);
  if (level < 0.0) {
    return CloudMapRead(vec4f(0.0), 1.0, 1.0, 0.0, false);
  }
  let k0 = u32(floor(level));
  let k1 = min(k0 + 1u, u32(m.north.w) - 1u);
  let fine = cloud_map_read_level(m, k0, plane.xy);
  let fine_m = m.east.w * exp2(2.0 * f32(k0));
  let coarse_m = m.east.w * exp2(2.0 * f32(k1));
  if (k1 == k0) {
    return CloudMapRead(fine, fine_m, coarse_m, 0.0, true);
  }
  let blend = level - f32(k0);
  return CloudMapRead(mix(fine, cloud_map_read_level(m, k1, plane.xy), blend), fine_m, coarse_m, blend, true);
}

// The field's rows (`cloudField.wgsl`'s CLOUD_ROW_*), under names of their own so that this library
// composes beside it.
const CLOUD_FINE_ROW_SHELL : u32 = 0u;
const CLOUD_FINE_ROW_OPTICS : u32 = 1u;
const CLOUD_FINE_ROW_FLOW : u32 = 2u;
const CLOUD_FINE_ROW_NORM : u32 = 3u;
const CLOUD_FINE_ROW_FADES : u32 = 4u;
const CLOUD_FINE_ROW_ROTATION : u32 = 7u;
const CLOUD_FINE_ROW_OFFSETS : u32 = 10u;
const CLOUD_FINE_RATIO : f32 = 0.7;
const CLOUD_FINE_LEAST_SD : f32 = 1e-3;
const CLOUD_FINE_LEAST_SHARE : f32 = 1e-6;
const CLOUD_FINE_NOISE_STD : f32 = 0.974;
const CLOUD_FINE_NOISE_RANGE : f32 = 4.0;
const CLOUD_FINE_PI : f32 = 3.14159265358979;
// `cloudShape.ts`' EXCESS_FULL, TOP_EXPONENT, STRATIFORM_LEAST_TOP, CUMULUS_COVER, STRATUS_COVER
// and LEAST_TOP_SHARE, and its least scale an excess is divided by.
const CLOUD_EXCESS_FULL : f32 = 2.0;
const CLOUD_TOP_EXPONENT : f32 = 0.6;
const CLOUD_STRATIFORM_LEAST_TOP : f32 = 0.7;
const CLOUD_CUMULUS_COVER : f32 = 0.5;
const CLOUD_STRATUS_COVER : f32 = 0.85;
const CLOUD_LEAST_TOP_SHARE : f32 = 0.02;
const CLOUD_LEAST_EXCESS_SCALE : f32 = 1e-9;

fn cloud_fine_row(i : u32) -> vec4f {
  return cloud_layer[i];
}

// w_k at a footprint (`cloudDensity.ts`' `octaveWeightAt`).
fn cloud_fine_weight(k : u32, footprint : f32) -> f32 {
  let optics = cloud_fine_row(CLOUD_FINE_ROW_OPTICS);
  if (k >= u32(optics.z)) {
    return 0.0;
  }
  let cell = optics.w * exp2(-f32(k));
  let reach = clamp(log2(cell / (2.0 * footprint)), 0.0, 1.0);
  return reach * cloud_fine_row(CLOUD_FINE_ROW_FADES + k / 4u)[k % 4u];
}

// sin and cos as (sin, cos), the angle first taken into [−π, π] (`cloudField.wgsl`'s `cloud_sin_cos`).
fn cloud_fine_sin_cos(angle : f32) -> vec2f {
  let reduced = angle - 2.0 * CLOUD_FINE_PI * round(angle / (2.0 * CLOUD_FINE_PI));
  let magnitude = preciseSinCos(min(abs(reduced), CLOUD_FINE_PI));
  return vec2f(select(magnitude.x, -magnitude.x, reduced < 0.0), magnitude.y);
}

// `cloudField.wgsl`'s `cloud_advected`.
fn cloud_fine_advected(p : vec3f, wind : vec2f, advection_s : f32) -> vec3f {
  let rho = length(p.xy);
  var q = p;
  if (rho > 0.0) {
    let sc = cloud_fine_sin_cos(-wind.x * advection_s / rho);
    q = vec3f(sc.y * p.x - sc.x * p.y, sc.x * p.x + sc.y * p.y, p.z);
  }
  let r = length(q);
  if (wind.y == 0.0 || !(rho > 0.0) || !(r > 0.0)) {
    return q;
  }
  let sc = cloud_fine_sin_cos(wind.y * advection_s / r);
  let scale = (sc.y * rho + sc.x * q.z) / rho;
  return vec3f(q.xy * scale, sc.y * q.z - sc.x * rho);
}

// `cloudField.wgsl`'s `cloud_octave_turn` and `cloud_body_turn`.
fn cloud_fine_octave_turn(c : vec3f) -> vec3f {
  return vec3f(
    dot(vec3f(0.0, 0.8, 0.6), c),
    dot(vec3f(-0.8, 0.36, -0.48), c),
    dot(vec3f(-0.6, -0.48, 0.64), c),
  );
}

fn cloud_fine_body_turn(p : vec3f) -> vec3f {
  return vec3f(
    dot(cloud_fine_row(CLOUD_FINE_ROW_ROTATION).xyz, p),
    dot(cloud_fine_row(CLOUD_FINE_ROW_ROTATION + 1u).xyz, p),
    dot(cloud_fine_row(CLOUD_FINE_ROW_ROTATION + 2u).xyz, p),
  );
}

fn cloud_fine_noise_at(c : vec3f) -> f32 {
  let stored = textureSampleLevel(cloud_field_noise, cloud_noise_repeat, c, 0.0).r;
  return (stored * 2.0 * CLOUD_FINE_NOISE_RANGE - CLOUD_FINE_NOISE_RANGE) / CLOUD_FINE_NOISE_STD;
}

// Q(x) and φ(x) (`cloudField.wgsl`'s).
fn cloud_fine_tail(x : f32) -> f32 {
  let z = abs(x) * 0.70710678;
  let t = 1.0 / (1.0 + 0.5 * z);
  let poly = -z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418
    + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587
    + t * (-0.82215223 + t * 0.17087277))))))));
  let erfc = t * exp(poly);
  return select(1.0 - 0.5 * erfc, 0.5 * erfc, x >= 0.0);
}

fn cloud_fine_density(x : f32) -> f32 {
  return exp(-0.5 * x * x) * 0.39894228;
}

// `cloudShape.ts`' `deckTopShare`.
fn cloud_top_share(excess : f32, coverage : f32, cumuliform : bool) -> f32 {
  let thickness = pow(max(clamp(excess / CLOUD_EXCESS_FULL, 0.0, 1.0), 1e-9), CLOUD_TOP_EXPONENT);
  let sheet = CLOUD_STRATIFORM_LEAST_TOP + (1.0 - CLOUD_STRATIFORM_LEAST_TOP) * thickness;
  var cumulus = 0.0;
  if (cumuliform) {
    cumulus = clamp((CLOUD_STRATUS_COVER - coverage) / (CLOUD_STRATUS_COVER - CLOUD_CUMULUS_COVER), 0.0, 1.0);
  }
  return max(sheet + cumulus * (thickness - sheet), CLOUD_LEAST_TOP_SHARE);
}

// A column (`cloudShape.ts`' CloudColumn): the covered share f, the mean τ, η_top, s, the covered
// part's mean excess ē and the coverage's share C.
struct CloudColumn {
  fraction : f32,
  mean_tau : f32,
  top : f32,
  sd : f32,
  excess : f32,
  cover : f32,
}

// A sample's column at a place on the layer's shell and a footprint (`cloudShape.ts`'
// `columnFromMap`): the map's resolved sum, with, where `refine` holds, each octave whose weight at
// the footprint differs from the map's blended weight added at the place; clear beyond the map.
fn cloud_column_from_map(m : CloudMapPlane, p : vec3f, footprint : f32, refine : bool) -> CloudColumn {
  let read = cloud_map_read(m, p, footprint);
  if (!read.valid) {
    return CloudColumn(0.0, 0.0, 1.0, 0.0, 0.0, 0.0);
  }
  let optics = cloud_fine_row(CLOUD_FINE_ROW_OPTICS);
  let n = min(u32(optics.z), 12u);
  var refine_lo = n;
  var refine_hi = 0u;
  if (refine) {
    for (var k = 0u; k < n; k = k + 1u) {
      let fine = cloud_fine_weight(k, read.fine_m);
      let mapped = fine + read.blend * (cloud_fine_weight(k, read.coarse_m) - fine);
      if (cloud_fine_weight(k, footprint) != mapped) {
        refine_lo = min(refine_lo, k);
        refine_hi = k + 1u;
      }
    }
  }
  var sum = read.texel.x;
  var variance = 0.0;
  var amplitude = 1.0;
  let flow = cloud_fine_row(CLOUD_FINE_ROW_FLOW);
  let norm = 1.0 / length(flow.zw);
  let tile = 1.0 / (8.0 * optics.w);
  var c0 = vec3f(0.0);
  var c1 = vec3f(0.0);
  if (refine_hi > refine_lo) {
    c0 = cloud_fine_body_turn(cloud_fine_advected(p, read.texel.zw, flow.x)) * tile;
    c1 = cloud_fine_body_turn(cloud_fine_advected(p, read.texel.zw, flow.y)) * tile;
  }
  for (var k = 0u; k < n; k = k + 1u) {
    let fine = cloud_fine_weight(k, read.fine_m);
    let mapped = fine + read.blend * (cloud_fine_weight(k, read.coarse_m) - fine);
    var own = mapped;
    if (refine) {
      own = cloud_fine_weight(k, footprint);
    }
    if (k >= refine_lo && k < refine_hi) {
      let n0 = cloud_fine_noise_at(c0 + cloud_fine_row(CLOUD_FINE_ROW_OFFSETS + 2u * k).xyz);
      let n1 = cloud_fine_noise_at(c1 + cloud_fine_row(CLOUD_FINE_ROW_OFFSETS + 2u * k + 1u).xyz);
      sum = sum + (own - mapped) * amplitude * (flow.z * n0 + flow.w * n1) * norm;
    }
    variance = variance + (own * amplitude) * (own * amplitude);
    if (refine_hi > refine_lo) {
      c0 = cloud_fine_octave_turn(c0) * 2.0;
      c1 = cloud_fine_octave_turn(c1) * 2.0;
    }
    amplitude = amplitude * CLOUD_FINE_RATIO;
  }
  let threshold = read.texel.y;
  let cover = cloud_fine_tail(threshold);
  let scale = cover / (cloud_fine_density(threshold) - threshold * cover);
  let inverse_total = cloud_fine_row(CLOUD_FINE_ROW_NORM).x;
  let zr = sum * inverse_total;
  let s = max(sqrt(max(1.0 - variance * inverse_total * inverse_total, 0.0)), CLOUD_FINE_LEAST_SD);
  let d = (threshold - zr) / s;
  let q = cloud_fine_tail(d);
  let mean_tau_bar = cloud_fine_row(CLOUD_FINE_ROW_SHELL).w;
  let mean_tau = max(mean_tau_bar * scale * s * (cloud_fine_density(d) - d * q), 0.0);
  let in_cloud = mean_tau / max(q, CLOUD_FINE_LEAST_SHARE);
  let excess = in_cloud / max(mean_tau_bar * scale, CLOUD_LEAST_EXCESS_SCALE);
  let cumuliform = cloud_shape_row(CLOUD_SHAPE_ROW_FEATURES).z > 0.5;
  return CloudColumn(q, mean_tau, cloud_top_share(excess, cover, cumuliform), s, excess, cover);
}

// `cloudShape.ts`' `domeTopShare`.
fn cloud_dome_top(excess : f32) -> f32 {
  return max(pow(max(clamp(excess / CLOUD_EXCESS_FULL, 0.0, 1.0), 1e-9), CLOUD_TOP_EXPONENT), CLOUD_LEAST_TOP_SHARE);
}

// `cloudShape.ts`' `cumulusShare`.
fn cloud_cumulus_share(cover : f32) -> f32 {
  if (!(cloud_shape_row(CLOUD_SHAPE_ROW_FEATURES).z > 0.5)) {
    return 0.0;
  }
  return clamp((CLOUD_STRATUS_COVER - cover) / (CLOUD_STRATUS_COVER - CLOUD_CUMULUS_COVER), 0.0, 1.0);
}

// The least share a column's cells are formed at (`cloudShape.ts`' CELL_LEAST_SHARE), the cells'
// region's footprint, m, their mesoscale share, their least in-cloud τ over τ̄, the second reading's
// scale and offset of their noise, the tall cell's share and the rise's saturation
// (`cloudShape.ts`' CELL_REGION_M, CELL_MESOSCALE, CELL_LEAST_TAU_SHARE, CELL_SECOND_SCALE,
// CELL_SECOND_OFFSET, CELL_TALL_SHARE and CELL_SATURATION).
const CLOUD_CELL_LEAST_SHARE : f32 = 1e-6;
const CLOUD_CELL_REGION_M : f32 = 40000.0;
const CLOUD_CELL_MESOSCALE : f32 = 0.5;
const CLOUD_CELL_LEAST_TAU_SHARE : f32 = 0.6;
const CLOUD_CELL_SECOND_SCALE : f32 = 1.7320508;
const CLOUD_CELL_SECOND_OFFSET : vec3f = vec3f(0.371, 0.613, 0.129);
const CLOUD_CELL_TALL_SHARE : f32 = 1.5;
const CLOUD_CELL_SATURATION : f32 = 0.25;

// The cells' uniform noise at a place on the layer's shell (`cloudShape.ts`' `cellNoiseAt`).
fn cloud_cell_noise_at(p : vec3f) -> f32 {
  let first = cloud_base_at(p);
  let k2 = cloud_shape_row(CLOUD_SHAPE_ROW_TILES).x / CLOUD_CELL_SECOND_SCALE;
  let turned = vec3f(
    dot(vec3f(0.0, 0.8, 0.6), p),
    dot(vec3f(-0.8, 0.36, -0.48), p),
    dot(vec3f(-0.6, -0.48, 0.64), p),
  );
  let second = textureSampleLevel(cloud_base_noise, cloud_noise_repeat, turned * k2 + CLOUD_CELL_SECOND_OFFSET, 0.0).r;
  // The sum of two uniforms back to a uniform, by the triangular distribution's CDF.
  let t = clamp(first + second, 0.0, 2.0);
  return select(1.0 - 0.5 * (2.0 - t) * (2.0 - t), 0.5 * t * t, t <= 1.0);
}

// `cloudShape.ts`' `cellRise`, `cellTopShare` and `cellRiseMean`.
fn cloud_cell_rise(du : f32) -> f32 {
  let below = 1.0 - clamp(du / CLOUD_CELL_SATURATION, 0.0, 1.0);
  return 1.0 - below * below;
}

fn cloud_cell_top(mean_excess : f32, du : f32) -> f32 {
  return max(cloud_dome_top(mean_excess * CLOUD_CELL_TALL_SHARE) * cloud_cell_rise(du), CLOUD_LEAST_TOP_SHARE);
}

fn cloud_cell_rise_mean(f_in : f32) -> f32 {
  let f = clamp(f_in, CLOUD_CELL_LEAST_SHARE, 1.0);
  let s = CLOUD_CELL_SATURATION;
  return select(1.0 - s / (3.0 * f), f / s - f * f / (3.0 * s * s), f <= s);
}

// A column's cumulus cell at a place (`cloudShape.ts`' `cellOf`): x its in-cloud τ, y its excess, z
// its top share, from the region's column, the local one's share and the cells' noise there.
fn cloud_cell_of(region : CloudColumn, local_fraction : f32, u : f32) -> vec3f {
  if (!(region.mean_tau > 0.0)) {
    return vec3f(0.0, 0.0, CLOUD_LEAST_TOP_SHARE);
  }
  let lo = CLOUD_CELL_LEAST_SHARE;
  let f_region = clamp(region.fraction, lo, 1.0);
  let f_cover = f_region + CLOUD_CELL_MESOSCALE * (local_fraction - f_region);
  let in_cloud = region.mean_tau / f_region;
  let cell_mean = max(in_cloud, CLOUD_CELL_LEAST_TAU_SHARE * cloud_fine_row(CLOUD_FINE_ROW_SHELL).w);
  let f = clamp(f_cover * in_cloud / cell_mean, lo, 1.0);
  let du = max(u - (1.0 - f), 0.0);
  var share = 0.0;
  if (du > 0.0) {
    share = cloud_cell_rise(du) / cloud_cell_rise_mean(f);
  }
  let excess = region.excess * cell_mean / in_cloud;
  return vec3f(cell_mean * share, excess * share, cloud_cell_top(excess, du));
}

// G at (w_b, w_d), bilinear in the table (`cloudShape.ts`' `normalisationAt`).
fn cloud_normalisation(wb : f32, wd : f32) -> f32 {
  let n = CLOUD_SHAPE_TABLE_STEPS;
  let x = clamp(wb, 0.0, 1.0) * f32(n - 1u);
  let y = clamp(wd, 0.0, 1.0) * f32(n - 1u);
  let i0 = min(u32(x), n - 2u);
  let j0 = min(u32(y), n - 2u);
  let fx = x - f32(i0);
  let fy = y - f32(j0);
  let a = i0 * n + j0;
  let b = (i0 + 1u) * n + j0;
  let g00 = cloud_shape_row(CLOUD_SHAPE_ROW_TABLE + a / 4u)[a % 4u];
  let g01 = cloud_shape_row(CLOUD_SHAPE_ROW_TABLE + (a + 1u) / 4u)[(a + 1u) % 4u];
  let g10 = cloud_shape_row(CLOUD_SHAPE_ROW_TABLE + b / 4u)[b % 4u];
  let g11 = cloud_shape_row(CLOUD_SHAPE_ROW_TABLE + (b + 1u) / 4u)[(b + 1u) % 4u];
  let top = g00 + fy * (g01 - g00);
  let bottom = g10 + fy * (g11 - g10);
  return top + fx * (bottom - top);
}

// The eroded envelope (`cloudShape.ts`' `erosion`).
fn cloud_erosion(envelope : f32, nb : f32, nd : f32, wb : f32, wd : f32) -> f32 {
  let lo = wb * CLOUD_BASE_EROSION * (1.0 - nb);
  let shaped = clamp((envelope - lo) / (1.0 - lo), 0.0, 1.0);
  let lo_detail = wd * CLOUD_DETAIL_EROSION * nd;
  let frayed = clamp((shaped - lo_detail) / (1.0 - lo_detail), 0.0, 1.0);
  return min(frayed * CLOUD_EDGE_SHARPNESS, 1.0);
}

// u^κ, held at u ≥ 10⁻⁶, 1 at κ = 0.
fn cloud_powered(u : f32, kappa : f32) -> f32 {
  if (kappa == 0.0) {
    return 1.0;
  }
  return pow(max(u, 1e-6), kappa);
}

// σ(550) per metre at a sample (`cloudShape.ts`' `densityPerM`): `u` its place in its column of
// top share `top` (after the flattening), `nb` and `nd` the noises (the detail as stored), `wb` and
// `wd` their shares before the flattening `flat`.
fn cloud_density(
  column_tau : f32, u : f32, top : f32, depth_m : f32, nb : f32, nd : f32, wb_in : f32, wd_in : f32,
  flat : f32,
) -> f32 {
  if (!(u >= 0.0 && u < 1.0) || !(column_tau > 0.0)) {
    return 0.0;
  }
  let kappa = cloud_shape_row(CLOUD_SHAPE_ROW_TILES).w;
  let profile = (1.0 + kappa) * cloud_powered(u, kappa);
  let shaped = profile + flat * (1.0 - profile);
  let wb = wb_in * (1.0 - flat);
  let wd = wd_in * (1.0 - flat);
  let turn = clamp(u / CLOUD_WISP_SHARE, 0.0, 1.0);
  let g = cloud_erosion(1.0 - u, nb, nd + turn * (1.0 - 2.0 * nd), wb, wd);
  let eroded = g + flat * (1.0 - g);
  let mean = cloud_normalisation(wb, wd);
  let normalised = mean + flat * (1.0 - mean);
  return column_tau * shaped * eroded / (depth_m * top * normalised);
}

// The share of a column's τ above a sample by its profile (`cloudShape.ts`' `shareAbove`).
fn cloud_share_above(u_in : f32, flat : f32) -> f32 {
  let u = clamp(u_in, 0.0, 1.0);
  let kappa = cloud_shape_row(CLOUD_SHAPE_ROW_TILES).w;
  let below = u * cloud_powered(u, kappa);
  return 1.0 - (below + flat * (u - below));
}

// A noise's share at a footprint (`cloudShape.ts`' `noiseShare`).
fn cloud_noise_share(feature_m : f32, footprint : f32) -> f32 {
  return clamp(log2(feature_m / (2.0 * max(footprint, 1e-3))), 0.0, 1.0);
}

// The base noise at a body-fixed place, m (`cloudShape.ts`' `baseNoiseAt`).
fn cloud_base_at(p : vec3f) -> f32 {
  let row = cloud_shape_row(CLOUD_SHAPE_ROW_BASE);
  let u = p * cloud_shape_row(CLOUD_SHAPE_ROW_TILES).x + row.xyz;
  return textureSampleLevel(cloud_base_noise, cloud_noise_repeat, u, 0.0).r;
}

// The detail noise at a body-fixed place, its coordinates moved by the curl in the plane of `east`
// and `north`, the curl read at `plane`, m (`cloudShape.ts`' `detailNoiseAt`).
fn cloud_detail_at(p : vec3f, plane : vec2f, east : vec3f, north : vec3f) -> f32 {
  let tiles = cloud_shape_row(CLOUD_SHAPE_ROW_TILES);
  let c = plane * tiles.z;
  let h = 1.0 / CLOUD_CURL_SIZE;
  let centre = textureSampleLevel(cloud_curl_noise, cloud_noise_repeat, c, 0.0).r;
  let along_x = textureSampleLevel(cloud_curl_noise, cloud_noise_repeat, c + vec2f(h, 0.0), 0.0).r - centre;
  let along_y = textureSampleLevel(cloud_curl_noise, cloud_noise_repeat, c + vec2f(0.0, h), 0.0).r - centre;
  let moved = CLOUD_CURL_GAIN * CLOUD_CURL_REACH * vec2f(along_y, -along_x);
  let row = cloud_shape_row(CLOUD_SHAPE_ROW_DETAIL);
  let u = p * tiles.y + row.xyz + moved.x * east + moved.y * north;
  return textureSampleLevel(cloud_detail_noise, cloud_noise_repeat, u, 0.0).r;
}

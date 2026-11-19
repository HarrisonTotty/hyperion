// The cloud volume's density (plan R11, R11.T7.c; Design note 10): the cloud map's column at a place
// (`cloudMap.wgsl`), laid through the deck by its kind's profile and eroded by the shape noises, the
// twin of `clouds/cloudShape.ts` (whose remarks give the model) and of `clouds/cloudMap.ts`'
// reading. Its includer declares, at its own group and bindings:
//   - `cloud_map : texture_2d<f32>`, the map, and `cloud_map_sampler : sampler`, linear and
//     `clamp-to-edge`;
//   - `cloud_shape : array<vec4f>`, read-only storage, CLOUD_SHAPE_ROWS rows (`cloudVolumePass.ts`'
//     `packCloudShape`);
//   - `cloud_base_noise : texture_3d<f32>` and `cloud_detail_noise : texture_3d<f32>`, the 128³ and
//     32³ noises, and `cloud_curl_noise : texture_2d<f32>`, the 128² curl potential, each `r8unorm`
//     (`cloudShapeNoise.ts`), with `cloud_noise_repeat : sampler`, linear and `repeat`.

// The shape's rows: 0 x 1 ÷ the base tile, y 1 ÷ the detail tile, z 1 ÷ the curl tile, each m⁻¹,
// w κ; 1 the base noise's drift in its tiles, w its time-rate share; 2 the detail's, w its share; 3
// x and y the base's and the detail's features for their fades, m, z 1 where the deck may be
// cumuliform, w unused; 4 to 24 the normalisation G at w_b = i ÷ 8, w_d = j ÷ 8, at 9 i + j, four a
// row (`cloudShape.ts`' `normalisationTable`).
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

// The column at a body-fixed place and footprint, m (`CloudMapTwin.columnAt`): clear beyond the map.
fn cloud_map_column(m : CloudMapPlane, p : vec3f, footprint : f32) -> vec4f {
  let plane = cloud_map_plane_at(m, p);
  if (plane.z < 0.5) {
    return vec4f(0.0, 0.0, 1.0, 0.0);
  }
  let level = cloud_map_level_at(m, plane.xy, footprint);
  if (level < 0.0) {
    return vec4f(0.0, 0.0, 1.0, 0.0);
  }
  let k0 = u32(floor(level));
  let k1 = min(k0 + 1u, u32(m.north.w) - 1u);
  let fine = cloud_map_read_level(m, k0, plane.xy);
  if (k1 == k0) {
    return fine;
  }
  return mix(fine, cloud_map_read_level(m, k1, plane.xy), level - f32(k0));
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

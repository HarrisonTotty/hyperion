// The `ocean` pass's shared bindings and helpers (plan R11, R11.T8.b and T8.c): the body's ocean
// map, R08's surface lighting's nine tables and the eclipse rows, at `@group(2)` bindings 0 to 10,
// which the far sea (oceanPass.wgsl) and the near sea (oceanNear.wgsl) both bind; R07's meter
// classes; and the eclipse term, litBody.wgsl's `eclipse_visible` copied as `ocean_eclipse_visible`
// since that library needs the phase table, which neither pass binds. Composed after frame.wgsl,
// R08's surface lighting (`SURFACE_LIGHTING_WGSL`), shaders/cubeTexel.wgsl and oceanFar.wgsl.

@group(2) @binding(0) var ocean_map : texture_2d_array<f32>;
// R08's surface lighting's nine tables, each by textureLoad.
@group(2) @binding(1) var atmosphere_body : texture_2d<f32>;
@group(2) @binding(2) var atmosphere_transmittance : texture_2d_array<f32>;
@group(2) @binding(3) var atmosphere_absorber_columns : texture_2d_array<f32>;
@group(2) @binding(4) var atmosphere_absorber_pressure_columns : texture_2d_array<f32>;
@group(2) @binding(5) var atmosphere_last_absorber_pressure_column : texture_2d_array<f32>;
@group(2) @binding(6) var atmosphere_absorber_curves : texture_2d<f32>;
@group(2) @binding(7) var atmosphere_irradiance : texture_2d_array<f32>;
@group(2) @binding(8) var atmosphere_below_deck : texture_2d_array<f32>;
@group(2) @binding(9) var atmosphere_source_table : texture_2d_array<f32>;
// The eclipse rows (`packOceanEclipse`): row 0 the lights, the occluders, the annuli a channel and
// the datum's equatorial radius a, m; for light j, from row 1 + 8 j, its direction from the body's
// centre and its distance ÷ a, its radius ÷ a in w, then per channel its annuli's outer edges and
// fluxes; from row 17 each occluder's centre ÷ a and radius ÷ a, body-fixed.
@group(2) @binding(10) var<storage, read> ocean_eclipse : array<vec4f>;

// `METER_CLASS.litBody` and `unlitBody` (`view/post/meter.ts`).
const OCEAN_METER_LIT_BODY : f32 = 2.0;
const OCEAN_METER_UNLIT_BODY : f32 = 3.0;
// The share of the brightest source's light above which a pixel is a lit body's (OCEAN_LIT_SHARE).
const OCEAN_LIT_SHARE : f32 = 1e-6;
// `HALF_FLOAT_MAX`: the largest value the rgba16float target holds.
const OCEAN_HDR_MAX : f32 = 65504.0;

// The eclipse rows' layout (`oceanPass.ts`' OCEAN_ECLIPSE_*).
const OCEAN_ECLIPSE_LIGHT_ROWS : u32 = 8u;
const OCEAN_ECLIPSE_FIRST_OCCLUDER : u32 = 17u;

// The area of overlap of two circles of radii r and k whose centres are z apart (litBody.wgsl's
// `circle_overlap_area`, its twin `annuli.ts`' `circleOverlapArea`).
fn ocean_circle_overlap(r : f32, k : f32, z : f32) -> f32 {
  if (r <= 0.0 || k <= 0.0 || z >= r + k) {
    return 0.0;
  }
  if (z <= abs(r - k)) {
    let m = min(r, k);
    return OCEAN_PI * m * m;
  }
  let d1 = (z * z + r * r - k * k) / (2.0 * z);
  let d2 = z - d1;
  let s1 = r * r * acos(clamp(d1 / r, -1.0, 1.0)) - d1 * sqrt(max(r * r - d1 * d1, 0.0));
  let s2 = k * k * acos(clamp(d2 / k, -1.0, 1.0)) - d2 * sqrt(max(k * k - d2 * d2, 0.0));
  return s1 + s2;
}

// The fraction of light j's channel c left visible by one occluder, the two as angular radii and the
// angle between their centres (litBody.wgsl's `eclipse_visible`, its twin `annulusVisibleFraction`).
fn ocean_eclipse_visible(
  j : u32,
  c : u32,
  star_radius : f32,
  occluder_radius : f32,
  separation : f32,
) -> f32 {
  let ratio = occluder_radius / star_radius;
  let z = separation / star_radius;
  if (z >= 1.0 + ratio) {
    return 1.0;
  }
  let base = 1u + j * OCEAN_ECLIPSE_LIGHT_ROWS + 2u + 2u * c;
  let outer = ocean_eclipse[base];
  let flux = ocean_eclipse[base + 1u];
  var hidden = 0.0;
  var inner_overlap = 0.0;
  var inner_edge = 0.0;
  for (var a = 0u; a < min(u32(ocean_eclipse[0].z), 4u); a = a + 1u) {
    let outer_edge = outer[a];
    let outer_overlap = ocean_circle_overlap(outer_edge, ratio, z);
    let area = OCEAN_PI * (outer_edge * outer_edge - inner_edge * inner_edge);
    hidden = hidden + flux[a] * (outer_overlap - inner_overlap) / area;
    inner_overlap = outer_overlap;
    inner_edge = outer_edge;
  }
  return max(0.0, 1.0 - hidden);
}

// The share of source j's light per channel that reaches the point `point` (from the body's centre,
// body-fixed, m) past the record's occluders; 1 for a source the record does not hold.
fn ocean_source_visible(j : u32, point : vec3f) -> vec3f {
  let header = ocean_eclipse[0];
  if (j >= u32(header.x)) {
    return vec3f(1.0);
  }
  let q = point / header.w;
  let light = ocean_eclipse[1u + j * OCEAN_ECLIPSE_LIGHT_ROWS];
  let star_radius_over_a = ocean_eclipse[2u + j * OCEAN_ECLIPSE_LIGHT_ROWS].w;
  let to_star = light.xyz * light.w - q;
  let star_distance = length(to_star);
  let towards = to_star / star_distance;
  let star_radius = ocean_angular_radius(star_radius_over_a, star_distance);
  var visible = vec3f(1.0);
  for (var k = 0u; k < u32(header.y); k = k + 1u) {
    let occluder = ocean_eclipse[OCEAN_ECLIPSE_FIRST_OCCLUDER + k];
    let to_occluder = occluder.xyz - q;
    let occluder_distance = length(to_occluder);
    if (occluder_distance >= star_distance) {
      continue;
    }
    if (occluder_distance <= occluder.w) {
      return vec3f(0.0);
    }
    let occluder_radius = ocean_angular_radius(occluder.w, occluder_distance);
    let along = to_occluder / occluder_distance;
    let separation = ocean_atan2(length(cross(along, towards)), dot(along, towards));
    for (var c = 0u; c < 3u; c = c + 1u) {
      visible[c] = visible[c] - (1.0 - ocean_eclipse_visible(j, c, star_radius, occluder_radius, separation));
    }
  }
  return max(visible, vec3f(0.0));
}

// The angular radius of a sphere of `radius` at `distance` from its centre, in one unit: asin(r ÷ d).
fn ocean_angular_radius(radius : f32, distance : f32) -> f32 {
  return ocean_atan2(radius, sqrt(max(distance * distance - radius * radius, 0.0)));
}

// atan2(y, x) by preciseTrig.wgsl's `preciseAtan2`, 0 at the origin.
fn ocean_atan2(y : f32, x : f32) -> f32 {
  if (!(abs(y) > 0.0 || abs(x) > 0.0)) {
    return 0.0;
  }
  let angle = preciseAtan2(abs(y), x);
  return select(angle, -angle, y < 0.0);
}

// A source's azimuth from local north towards east at a point of outward normal `normal`, rad.
fn ocean_azimuth(normal : vec3f, d : vec3f) -> f32 {
  let east = ocean_east(normal);
  let north = cross(normal, east);
  return ocean_atan2(dot(d, east), dot(d, north));
}

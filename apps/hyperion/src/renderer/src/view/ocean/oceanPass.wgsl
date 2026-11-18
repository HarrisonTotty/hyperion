// The `ocean` pass's far sea (plan R11, R11.T8.b; Design notes 11 and 12): one draw a body whose sea
// the frame draws, a full-screen triangle, each pixel's ray met with the body's sea through
// oceanFar.wgsl's `ocean_far_hit` (the sea-level spheroid where the ocean map holds open water, a
// closed basin's raised one where its texel says so) and lit there by each of the body's two
// brightest sources through R08's surface lighting (`atmosphere_sun_transmittance` and
// `atmosphere_sky_irradiance` at the sea's height above the datum, its geodetic latitude and the
// source's azimuth from local north, R08 Design note 17), each source's light through
// `ocean_far_light`, the glint capped at the bloom threshold, which the frame's glare source carries
// past (`glint.ts`). Each source's light, its sky's with it, is dimmed by the eclipses of the bodies
// between it and the sea (R07's eclipse term, litBody.wgsl's `eclipse_visible`, copied here as
// `ocean_eclipse_visible` since that library needs the phase table, which this pass does not bind),
// read from the body's disc record (`ocean_eclipse`, `oceanPass.ts`' `packOceanEclipse`).
//
// Drawn after the opaque passes and before R08's composite, opaque: the sea's light holds its bed's
// seen through the water, so it replaces what lies under it, and it writes its depth (frag_depth,
// reversed-Z, tested against the scene's), so that the composite lays the air over the sea and not
// over the bed beneath it (a lean, 2026-11-18). Where the body's figure is drawn as a mesh on its
// datum, the sea's depth is the nearer of its own and the datum's along the ray, so that a sea level
// below the datum is not hidden by the figure it stands in for. Its alpha is R07's meter class: a
// lit body's where the brightest source is above the sea's horizon, an unlit one's elsewhere.
//
// The ray is the pass's own: the pixel's through the frame's projection (a still's tile and its
// sub-pixel offset included), in the view's axes, turned into the body-fixed ones, as
// occluderSphereHit.wgsl's is formed.
//
// Composed after frame.wgsl, R08's surface lighting (`SURFACE_LIGHTING_WGSL`), shaders/cubeTexel.wgsl
// and oceanFar.wgsl.

struct Draw {
  offsetFromCameraM : vec3f,
  // xyz: the camera from the body's centre, body-fixed, m; w: 1 where the body's figure is drawn as
  // a mesh on its datum, else 0.
  camera : vec4f,
  // The view's x, y and z axes in the body-fixed ones.
  viewX : vec4f,
  viewY : vec4f,
  viewZ : vec4f,
  // x, y: the datum's equatorial and polar radii, m; z: the sea level above it, m; w: the sources
  // drawn, 0 to 2.
  datum : vec4f,
  // Each source: xyz its unit direction, body-fixed; w its light slot.
  source0 : vec4f,
  // Its illuminance face-on per channel times the frame's pre-exposure, lx.
  sourceLight0 : vec4f,
  source1 : vec4f,
  sourceLight1 : vec4f,
  // oceanFar.wgsl's `OceanFar`, row by row (`packOceanFar`): its light and cap the pre-exposed ones.
  farFigure : vec4f,
  farSun : vec4f,
  farSunLux : vec4f,
  farSkyLux : vec4f,
  farIndex : vec4f,
  farAttenuation : vec4f,
  farDeep : vec4f,
  farBed : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
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

struct OceanVertex {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> OceanVertex {
  // A full-screen triangle; each fragment's depth is its own hit's.
  return OceanVertex(vec4f(position.xy, 1.0, 1.0));
}

struct OceanOut {
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

// The draw's `OceanFar`.
fn ocean_pass_far() -> OceanFar {
  return OceanFar(
    draw.farFigure,
    draw.farSun,
    draw.farSunLux,
    draw.farSkyLux,
    draw.farIndex,
    draw.farAttenuation,
    draw.farDeep,
    draw.farBed,
  );
}

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

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> OceanOut {
  // The pixel's ray in the view's axes, through the frame's projection, off-axis terms included.
  let ndc = vec2f(
    position.x * frame.viewport.z * 2.0 - 1.0,
    1.0 - position.y * frame.viewport.w * 2.0,
  );
  let viewRay = vec3f(
    (ndc.x + frame.clipProjection[2][0]) / frame.clipProjection[0][0],
    (ndc.y + frame.clipProjection[2][1]) / frame.clipProjection[1][1],
    -1.0,
  );
  let rayLength = length(viewRay);
  let d = normalize(viewRay.x * draw.viewX.xyz + viewRay.y * draw.viewY.xyz + viewRay.z * draw.viewZ.xyz);
  let o = draw.camera.xyz;
  var far = ocean_pass_far();
  let hit = ocean_far_hit(ocean_map, far, o, d);
  if (!hit.found) {
    discard;
  }
  let height = draw.datum.z + hit.texel.level;
  let latitude = ocean_atan2(hit.normal.z, length(hit.normal.xy));
  var radiance = vec3f(0.0);
  var lit = false;
  let sources = min(u32(draw.datum.w), 2u);
  for (var j = 0u; j < sources; j = j + 1u) {
    var direction = draw.source0;
    var light = draw.sourceLight0.xyz;
    if (j == 1u) {
      direction = draw.source1;
      light = draw.sourceLight1.xyz;
    }
    let s = direction.xyz;
    let slot = u32(direction.w);
    let mu0 = dot(hit.normal, s);
    // Its sky shares its eclipse, as an umbra's air does (terrainLit.wgsl's reading).
    let visible = ocean_source_visible(j, hit.point);
    let transmitted = visible * atmosphere_sun_transmittance(height, mu0, latitude, ocean_azimuth(hit.normal, s), slot);
    let sky = visible * atmosphere_sky_irradiance(height, mu0, latitude, slot);
    far.sun = vec4f(s, draw.farSun.w);
    radiance = radiance + ocean_far_light(far, OceanLight(light * transmitted, light * sky), -d, hit.normal, hit.texel, hit.t);
    if (j == 0u) {
      lit = mu0 > 0.0 && any(transmitted > vec3f(OCEAN_LIT_SHARE));
    }
  }
  // The nearer of the sea's and the datum's along the ray where the figure stands on the datum.
  var depthT = hit.t;
  if (draw.camera.w > 0.5) {
    let datum = ocean_spheroid_hit(o, d, draw.datum.x, draw.datum.y);
    if (datum.found) {
      depthT = min(depthT, datum.t);
    }
  }
  var out : OceanOut;
  // Reversed-Z, infinite far: n ÷ (−z), −z = t ÷ |viewRay| (occluderSphereHit.wgsl's).
  out.depth = clamp(frame.clipProjection[3][2] * rayLength / depthT, 0.0, 1.0);
  out.colour = vec4f(
    min(radiance, vec3f(OCEAN_HDR_MAX)),
    select(OCEAN_METER_UNLIT_BODY, OCEAN_METER_LIT_BODY, lit),
  );
  return out;
}

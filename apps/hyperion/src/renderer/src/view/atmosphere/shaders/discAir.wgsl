// The disc's air beyond the 10⁹ m boundary (plan R08, R08.T16.b; Design notes 13 and 17;
// `discTable.ts`, whose `discTableAt` is the read's twin, and `view/bodies/discShading.ts`, the
// step's): composed only into the disc kernels' forms for a body drawn by its reflectance table
// (`discReflectance.ts`' `withDiscAir`), after bodyDisc.wgsl, whose `shade` hands this file its
// sample's light as its last step. The table is at group 2's binding 15, a 2D array of one layer a
// latitude band and light slot, `DISC_TABLE_WIDTH` texels a row (a view cosine and azimuth each) and
// a row a sun cosine, read by `textureLoad`: rgb the air's π L ÷ E, alpha the march's mean
// transmittance. A record drawn by these forms carries its air block past the records, one row from
// the row its row 46's z names (`packDiscAir`): x the table's light slots a band, y its sun's floor
// μ_f, z and w 0. The bands' s and the latitude's are the body's table's (`oblate.wgsl`), bound for
// the surface lighting at binding 6.
@group(2) @binding(15) var disc_reflectance : texture_2d_array<f32>;

// `discTable.ts`' grid: 16 view cosines (v ÷ 15)², 8 sun rows μ_f (1 − s ÷ 8)² below the horizontal
// and 24 at (k ÷ 23)² from it up, 13 azimuths π k ÷ 12.
const DISC_VIEW_COSINES : u32 = 16u;
const DISC_SUN_ROWS_BELOW : u32 = 8u;
const DISC_SUN_ROWS_ABOVE : u32 = 24u;
const DISC_SUN_COSINES : u32 = 32u;
const DISC_AZIMUTHS : u32 = 13u;

// A view cosine's place among the nodes (`discViewPlace`).
fn disc_view_place(mu : f32) -> f32 {
  return f32(DISC_VIEW_COSINES - 1u) * sqrt(clamp(mu, 0.0, 1.0));
}

// A sun cosine's place among the rows under the floor `sun_floor` (`discSunPlace`).
fn disc_sun_place(mu : f32, sun_floor : f32) -> f32 {
  if (mu < 0.0) {
    return f32(DISC_SUN_ROWS_BELOW) * max(1.0 - sqrt(min(mu / sun_floor, 1.0)), 0.0);
  }
  return f32(DISC_SUN_ROWS_BELOW) + f32(DISC_SUN_ROWS_ABOVE - 1u) * sqrt(min(mu, 1.0));
}

// Δφ between the horizontal parts of `v` and `l` about the unit `n`, in [0, π]; 0 where either is
// vertical (`discAzimuthOf`), by `preciseAtan2` of their cross and dot products.
fn disc_air_azimuth(n : vec3f, v : vec3f, l : vec3f) -> f32 {
  let hv = v - dot(n, v) * n;
  let hl = l - dot(n, l) * n;
  let along = dot(hv, hl);
  let across = length(cross(hv, hl));
  if (!(across > 0.0) && !(abs(along) > 0.0)) {
    return 0.0;
  }
  return preciseAtan2(across, along);
}

// The node below a place on an axis of `n` nodes, and the share to the next, held at the ends.
fn disc_air_cell(place : f32, n : u32) -> vec2f {
  let lower = clamp(floor(place), 0.0, f32(n - 2u));
  return vec2f(lower, clamp(place - lower, 0.0, 1.0));
}

// One layer at `place` (the sun's row, the view's node, the azimuth's), linear in each between the
// nodes about it (`discTable.ts`' `addLayer`).
fn disc_air_layer(layer : u32, place : vec3f) -> vec4f {
  let s = disc_air_cell(place.x, DISC_SUN_COSINES);
  let v = disc_air_cell(place.y, DISC_VIEW_COSINES);
  let k = disc_air_cell(place.z, DISC_AZIMUTHS);
  var out = vec4f(0.0);
  for (var corner = 0u; corner < 8u; corner = corner + 1u) {
    let ds = corner & 1u;
    let dv = (corner >> 1u) & 1u;
    let dk = corner >> 2u;
    let w = select(1.0 - s.y, s.y, ds == 1u) * select(1.0 - v.y, v.y, dv == 1u)
      * select(1.0 - k.y, k.y, dk == 1u);
    let texel = vec2u(
      (u32(k.x) + dk) * DISC_VIEW_COSINES + u32(v.x) + dv,
      u32(s.x) + ds,
    );
    out = out + w * textureLoad(disc_reflectance, texel, layer, 0);
  }
  return out;
}

// The table at s and light slot `slot` (held to its `slots`), its bands read as `bandTableAt` reads
// the multiple scattering's: linear in ln s between the two about s, the nearer beyond them, one
// band's alone (`discTableAt`).
fn disc_air_at(air : OblateBody, s : f32, slot : u32, slots : u32, place : vec3f) -> vec4f {
  let k = min(slot, slots - 1u);
  let last = oblateBandCount(air) - 1u;
  if (last == 0u) {
    return disc_air_layer(k, place);
  }
  let ls = log(s);
  if (!(ls > oblateBand(atmosphere_body, 0u).y)) {
    return disc_air_layer(k, place);
  }
  if (!(ls < oblateBand(atmosphere_body, last).y)) {
    return disc_air_layer(last * slots + k, place);
  }
  var lower = 0u;
  while (lower + 1u < last && oblateBand(atmosphere_body, lower + 1u).y <= ls) {
    lower++;
  }
  let l0 = oblateBand(atmosphere_body, lower).y;
  let w = (ls - l0) / (oblateBand(atmosphere_body, lower + 1u).y - l0);
  return disc_air_layer(lower * slots + k, place) * (1.0 - w)
    + disc_air_layer((lower + 1u) * slots + k, place) * w;
}

// The air's own light (R08.T18.b; `discThermalAt`), pi L_th over the table's scale: its thermal
// layers past its bands' light slots, one a band, at the top sun row and azimuth 0, the bands read as
// `disc_air_at` reads them.
fn disc_air_thermal_at(air : OblateBody, s : f32, slots : u32, view_place : f32) -> vec3f {
  let bands = oblateBandCount(air);
  let base = bands * slots;
  let last = bands - 1u;
  let place = vec3f(f32(DISC_SUN_COSINES - 1u), view_place, 0.0);
  if (last == 0u) {
    return disc_air_layer(base, place).rgb;
  }
  let ls = log(s);
  if (!(ls > oblateBand(atmosphere_body, 0u).y)) {
    return disc_air_layer(base, place).rgb;
  }
  if (!(ls < oblateBand(atmosphere_body, last).y)) {
    return disc_air_layer(base + last, place).rgb;
  }
  var lower = 0u;
  while (lower + 1u < last && oblateBand(atmosphere_body, lower + 1u).y <= ls) {
    lower++;
  }
  let l0 = oblateBand(atmosphere_body, lower).y;
  let w = (ls - l0) / (oblateBand(atmosphere_body, lower + 1u).y - l0);
  return disc_air_layer(base + lower, place).rgb * (1.0 - w)
    + disc_air_layer(base + lower + 1u, place).rgb * w;
}

// A star's light at `q` towards it through the eclipse's occluders, per channel: `shade`'s term
// (`eclipseVisible`), each occluder's hidden fraction taken off.
fn disc_air_visible(q : vec3f, j : u32, towards : vec3f, distance : f32, star_radius : f32) -> vec3f {
  let counts = disc_row(3u);
  let annulus_count = u32(disc_row(5u).y);
  let occluders = min(u32(counts.w), MAX_DISC_OCCLUDERS);
  var visible = vec3f(1.0);
  for (var k = 0u; k < occluders; k = k + 1u) {
    let occluder = disc_row(FIRST_OCCLUDER_ROW + k);
    let to_occluder = to_view(occluder.xyz) - q;
    let occluder_distance = length(to_occluder);
    if (occluder_distance >= distance) {
      continue;
    }
    if (occluder_distance <= occluder.w) {
      return vec3f(0.0);
    }
    let occluder_radius = asin(occluder.w / occluder_distance);
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
  return max(visible, vec3f(0.0));
}

// The sample at `q` seen along `ray` beyond the boundary (`discShading.ts`' twin): its ground's
// light `ground`, `shade`'s, dimmed by the march's mean transmittance along the view (the table's
// alpha at light slot 0, as the composite dims a drawn surface), and each light's air at its cosines
// and azimuth there: a star's under its eclipse and the ring's shadow, as the sky's light is, at its
// light slot; a planetshine source's at slot 0, by the inverse square from the body's centre, as its
// sky term. A light under the table's floor lights no air.
fn disc_air(body : Body, q : vec3f, ray : vec3f, ground : vec3f) -> vec3f {
  let block = discs[u32(disc_row(SECONDARY_COUNT_ROW).z)];
  let slots = max(u32(block.x), 1u);
  let sun_floor = block.y;
  let air = oblateBodyOf(atmosphere_body);
  let stretch2 = body.stretch * body.stretch;
  let normal = normalize(q + (stretch2 - 1.0) * dot(q, body.pole) * body.pole);
  let to_camera = -ray;
  let sin_latitude = clamp(dot(normal, body.pole), -1.0, 1.0);
  let s = gravityRatioAt(air, sin_latitude * sin_latitude);
  let view_place = disc_view_place(dot(normal, to_camera));
  let through = disc_air_at(air, s, 0u, slots, vec3f(f32(DISC_SUN_COSINES - 1u), view_place, 0.0));
  var out = ground * through.a;
  let counts = disc_row(3u);
  let misc = disc_row(5u);
  let lights = min(u32(counts.z), MAX_DISC_LIGHTS);
  for (var j = 0u; j < lights; j = j + 1u) {
    let place = disc_row(FIRST_LIGHT_ROW + j * LIGHT_ROWS);
    let light = disc_row(FIRST_LIGHT_ROW + j * LIGHT_ROWS + 1u);
    let to_star = to_view(place.xyz) * place.w - q;
    let distance = length(to_star);
    let towards = to_star / distance;
    let mu0 = dot(normal, towards);
    if (mu0 < sun_floor) {
      continue;
    }
    let star_radius = asin(min(1.0, light.w / distance));
    let visible = disc_air_visible(q, j, towards, distance, star_radius);
    let ring = ring_shadow_on_body(q, towards);
    let lit = disc_air_at(air, s, j, slots, vec3f(
      disc_sun_place(mu0, sun_floor), view_place,
      f32(DISC_AZIMUTHS - 1u) * disc_air_azimuth(normal, to_camera, towards) / TRIG_PI
    ));
    out = out + light.xyz * misc.z * lit.rgb * visible * ring;
  }
  let secondaries = min(u32(disc_row(SECONDARY_COUNT_ROW).x), MAX_DISC_SECONDARIES);
  for (var j = 0u; j < secondaries; j = j + 1u) {
    let place = disc_row(FIRST_SECONDARY_ROW + j * SECONDARY_ROWS);
    let source = disc_row(FIRST_SECONDARY_ROW + j * SECONDARY_ROWS + 1u);
    let to_source = to_view(place.xyz) * place.w - q;
    let distance = length(to_source);
    let towards = to_source / distance;
    let mu0 = dot(normal, towards);
    if (mu0 < sun_floor) {
      continue;
    }
    let near = place.w / distance;
    let lit = disc_air_at(air, s, 0u, slots, vec3f(
      disc_sun_place(mu0, sun_floor), view_place,
      f32(DISC_AZIMUTHS - 1u) * disc_air_azimuth(normal, to_camera, towards) / TRIG_PI
    ));
    out = out + source.xyz * (near * near) * misc.z * lit.rgb;
  }
  // A self-luminous medium's own light (R08.T18.b): its thermal layers times their scale, the
  // block's z, 0 for a table without them.
  if (block.z > 0.0) {
    out = out + disc_air_thermal_at(air, s, slots, view_place) * block.z * misc.z;
  }
  return out;
}

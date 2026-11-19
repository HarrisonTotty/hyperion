// The cloud volume's light (plan R11, R11.T7.c; Design note 10): the high setting's march through a
// condensate's deck, its two-lobe phase, Wrenninge et al. 2013's multiple-scattering octaves and
// their correction to δ-Eddington. The twin of `clouds/cloudVolume.ts`, its f64 reference, whose
// remarks give the model, and of `clouds/cloudMarch.ts`' `marchCloudRay`, whose remarks give the
// march (`cloud_march`). Composed after `cloudShape.wgsl` and `cloudOptics.wgsl`; its includer
// declares, besides `cloudShape.wgsl`'s bindings:
//   - `cloud_octave_correction : texture_2d<f32>`, the condensate's K(τ, μ₀) on the octaves'
//     multiple scattering, `r32float`, CLOUD_CORRECTION_TAUS × CLOUD_CORRECTION_MUS, read by
//     `textureLoad` (`cloudVolume.ts`' `octaveCorrectionOf`).

// `cloudVolume.ts`' CLOUD_OCTAVES, BACKWARD_LOBE_ASYMMETRY and FORWARD_LOBE_SHARE: octave i's
// extinction, on the sun's path and the view's, a^i; its scattering b^i ≤ a^i; its phase's c^i.
const CLOUD_OCTAVE_COUNT : u32 = 4u;
const CLOUD_OCTAVE_EXTINCTION : f32 = 0.1102;
const CLOUD_OCTAVE_SCATTERING : f32 = 0.1102;
const CLOUD_OCTAVE_PHASE : f32 = 0.0;
const CLOUD_BACKWARD_LOBE : f32 = -0.3;
const CLOUD_FORWARD_SHARE : f32 = 0.5;
// The correction table's shape (`cloudVolume.ts`' CORRECTION_*).
const CLOUD_CORRECTION_TAUS : u32 = 32u;
const CLOUD_CORRECTION_MUS : u32 = 16u;
const CLOUD_CORRECTION_TAU_LEAST : f32 = 0.05;
const CLOUD_CORRECTION_TAU_MOST : f32 = 10000.0;
const CLOUD_CORRECTION_LEAST_MU : f32 = 0.02;
// Below this σ_e Δ a step's (1 − e^(−x)) ÷ x takes its series, which f32's 1 − e^(−x) would lose:
// at 0.03 the series' error is under 10⁻⁸, the closed form's under 10⁻⁵ with WGSL's `exp`.
const CLOUD_ESCAPE_SERIES : f32 = 0.03;
// The least zenith cosine the sun is taken at inside the deck, and the view's in a column.
const CLOUD_VOLUME_LEAST_MU : f32 = 0.02;
const CLOUD_LEAST_VIEW_MU : f32 = 1e-6;
const CLOUD_VOLUME_PI : f32 = 3.14159265358979;

// Henyey and Greenstein's phase function, sr⁻¹.
fn cloud_hg(cos_theta : f32, g : f32) -> f32 {
  let d = 1.0 + g * g - 2.0 * g * cos_theta;
  return (1.0 - g * g) / (4.0 * CLOUD_VOLUME_PI * d * sqrt(d));
}

// The two lobes of asymmetry g, each lobe's asymmetry scaled by `scale` (`twoLobePhase`).
fn cloud_two_lobe(cos_theta : f32, g : f32, scale : f32) -> f32 {
  let forward = g + CLOUD_FORWARD_SHARE * (1.0 - g);
  let w = (g - CLOUD_BACKWARD_LOBE) / (forward - CLOUD_BACKWARD_LOBE);
  return w * cloud_hg(cos_theta, scale * forward)
    + (1.0 - w) * cloud_hg(cos_theta, scale * CLOUD_BACKWARD_LOBE);
}

// (1 − e^(−x)) ÷ x for x ≥ 0 (`cloudVolume.ts`' `escape`).
fn cloud_escape(x : f32) -> f32 {
  if (x > CLOUD_ESCAPE_SERIES) {
    return (1.0 - exp(-x)) / x;
  }
  return 1.0 - x * (0.5 - x * (1.0 / 6.0 - x / 24.0));
}

// K, on the octaves' multiple scattering, at a column's τ and the sun's μ₀: bilinear in ln τ and μ₀ between the table's texels, held at
// its edges (`octaveCorrectionAt`).
fn cloud_octave_correction_at(tau : f32, mu0 : f32) -> f32 {
  let u = log(clamp(tau, CLOUD_CORRECTION_TAU_LEAST, CLOUD_CORRECTION_TAU_MOST) / CLOUD_CORRECTION_TAU_LEAST)
    / log(CLOUD_CORRECTION_TAU_MOST / CLOUD_CORRECTION_TAU_LEAST);
  let v = (clamp(mu0, CLOUD_CORRECTION_LEAST_MU, 1.0) - CLOUD_CORRECTION_LEAST_MU) / (1.0 - CLOUD_CORRECTION_LEAST_MU);
  let x = u * f32(CLOUD_CORRECTION_TAUS - 1u);
  let y = v * f32(CLOUD_CORRECTION_MUS - 1u);
  let i0 = min(u32(x), CLOUD_CORRECTION_TAUS - 2u);
  let j0 = min(u32(y), CLOUD_CORRECTION_MUS - 2u);
  let fx = x - f32(i0);
  let fy = y - f32(j0);
  let k00 = textureLoad(cloud_octave_correction, vec2u(i0, j0), 0).r;
  let k10 = textureLoad(cloud_octave_correction, vec2u(i0 + 1u, j0), 0).r;
  let k01 = textureLoad(cloud_octave_correction, vec2u(i0, j0 + 1u), 0).r;
  let k11 = textureLoad(cloud_octave_correction, vec2u(i0 + 1u, j0 + 1u), 0).r;
  let top = k00 + fx * (k10 - k00);
  let bottom = k01 + fx * (k11 - k01);
  return top + fy * (bottom - top);
}

// A homogeneous column's radiance towards a view above it by its octaves, per unit of the sun's
// irradiance face-on, sr⁻¹ (`marchColumn`): x the first octave's, y the rest's, uncorrected; `steps`
// equal steps of vertical τ from the top.
fn cloud_column_march(
  tau : f32, mu0 : f32, mu_view : f32, cos_theta : f32, g : f32, co_albedo : f32, steps : u32,
) -> vec2f {
  let dt = tau / f32(steps);
  let view_mu = max(mu_view, CLOUD_LEAST_VIEW_MU);
  let albedo = 1.0 - co_albedo;
  var light = vec2f(0.0);
  var a = 1.0;
  var b = 1.0;
  var c = 1.0;
  for (var i = 0u; i < CLOUD_OCTAVE_COUNT; i = i + 1u) {
    let phase = cloud_two_lobe(cos_theta, g, c);
    // The view's path takes the medium's own extinction, the octaves' a^i the sun's alone.
    let view_step = dt / view_mu;
    let step_light = b * albedo * phase * (dt / view_mu) * cloud_escape(view_step);
    var radiance = 0.0;
    var transmittance = 1.0;
    for (var k = 0u; k < steps; k = k + 1u) {
      let sun = exp(-a * (f32(k) + 0.5) * dt / mu0);
      radiance = radiance + transmittance * step_light * sun;
      transmittance = transmittance * exp(-view_step);
    }
    if (i == 0u) {
      light.x = radiance;
    } else {
      light.y = light.y + radiance;
    }
    a = a * CLOUD_OCTAVE_EXTINCTION;
    b = b * CLOUD_OCTAVE_SCATTERING;
    c = c * CLOUD_OCTAVE_PHASE;
  }
  return light;
}

// The ray's distances where it meets the spheroid |S x| = r, S = diag(1, 1, k), as (near, far);
// both negative where it misses.
fn cloud_spheroid_hits(o : vec3f, d : vec3f, k : f32, r : f32) -> vec2f {
  let so = vec3f(o.xy, o.z * k);
  let sd = vec3f(d.xy, d.z * k);
  let a = dot(sd, sd);
  let b = dot(so, sd);
  let c = dot(so, so) - r * r;
  let disc = b * b - a * c;
  if (disc < 0.0) {
    return vec2f(-1.0);
  }
  let root = sqrt(disc);
  return vec2f((-b - root) / a, (-b + root) / a);
}

// The ray's first stretch through the deck between `base_m` and `top_m` above the datum `datum`
// (`cloudMarch.ts`' `deckStretch`); (−1, −1) where it has none.
fn cloud_deck_stretch(o : vec3f, d : vec3f, k : f32, datum : f32, base_m : f32, top_m : f32) -> vec2f {
  let outer = cloud_spheroid_hits(o, d, k, datum + top_m);
  if (outer.y <= 0.0) {
    return vec2f(-1.0);
  }
  let inner = cloud_spheroid_hits(o, d, k, datum + base_m);
  var t0 = max(outer.x, 0.0);
  var t1 = outer.y;
  if (inner.x > t0) {
    t1 = inner.x;
  } else if (inner.x < 0.0 && inner.y > t0) {
    t0 = inner.y;
  }
  if (!(t1 > t0)) {
    return vec2f(-1.0);
  }
  return vec2f(t0, t1);
}

// `cloudMarch.ts`' LIGHT_REACH_DEPTHS, LIGHT_LEAST_MU, LEAST_TRANSMITTANCE, SHARE_SD_RANGE and
// LEAST_SHARE.
const CLOUD_LIGHT_REACH_DEPTHS : f32 = 3.0;
const CLOUD_LIGHT_LEAST_MU : f32 = 0.33333333;
const CLOUD_MARCH_LEAST_VIEW : f32 = 3e-3;
const CLOUD_SHARE_SD_LEAST : f32 = 0.1;
const CLOUD_SHARE_SD_MOST : f32 = 0.5;
const CLOUD_MARCH_LEAST_SHARE : f32 = 1e-4;
// `cloudMarch.ts`' SKY_MU, the two-stream diffusivity's cosine.
const CLOUD_MARCH_SKY_COSINE : f32 = 0.6;

// What a march reads of its deck and its condensate.
struct CloudMarchDeck {
  // The deck's base and top above the datum, its depth, and the datum's equatorial radius, m.
  base_m : f32,
  top_m : f32,
  depth_m : f32,
  datum : f32,
  // k, the deck's spheroids' a ÷ c.
  k : f32,
  // g and 1 − ϖ₀.
  asymmetry : f32,
  co_albedo : f32,
  // φ, the flattening towards the layer's slab.
  flat : f32,
  // a_s, the layer's shell's equatorial radius, m, on which the map's columns stand.
  shell_m : f32,
}

// A march's levers (`cloudMarch.ts`' CloudMarchLevers): the view's samples, each march towards
// the source's, and 1 where the detail noise is read.
struct CloudMarchLevers {
  view_steps : u32,
  light_steps : u32,
  detail : f32,
}

// A march's result (`cloudMarch.ts`' CloudMarch): each source's radiance per unit of its
// irradiance face-on, sr⁻¹, the sky's per unit of its irradiance ÷ π, the view's transmittance and
// the distance its opacity is weighed at, each before the share, and the share.
struct CloudMarchResult {
  sun : vec2f,
  sky : f32,
  transmittance : f32,
  mean_t : f32,
  share : f32,
}

// A sample's density, its column's in-cloud τ over the share and the share of that above it
// (`cloudMarch.ts`' SampleDensity).
struct CloudSampleDensity {
  sigma : f32,
  column_tau : f32,
  above : f32,
  // The region's column the cells read, and whether they read one.
  region : CloudColumn,
  has_region : bool,
}

fn cloud_height_of(x : vec3f, deck : CloudMarchDeck) -> f32 {
  return length(vec3f(x.xy, x.z * deck.k)) - deck.datum;
}

// The column above or below a place at a footprint (`cloudMarch.ts`' `columnAt`): read at the
// place's direction on the layer's shell, refined to the footprint where `refine` holds.
fn cloud_column_at(m : CloudMapPlane, deck : CloudMarchDeck, x : vec3f, footprint : f32, refine : bool) -> CloudColumn {
  let p = x * (deck.shell_m / length(vec3f(x.xy, x.z * deck.k)));
  return cloud_column_from_map(m, p, footprint, refine);
}

// The field's footprint a column is read at: a liquid deck's at least half a cell's
// (`cloudMarch.ts`' `fieldFootprintM`).
fn cloud_field_footprint(footprint : f32) -> f32 {
  let features = cloud_shape_row(CLOUD_SHAPE_ROW_FEATURES);
  return select(footprint, max(footprint, 0.5 * features.w), features.z > 0.5);
}

// The density at a place and footprint (`cloudMarch.ts`' `densityAt`): its column as a sheet,
// mixed with its cumulus cells by their share, the cells' region `region_in` where `has_region`.
fn cloud_density_at(
  m : CloudMapPlane, deck : CloudMarchDeck, x : vec3f, footprint : f32, share : f32, detail : bool,
  refine : bool, region_in : CloudColumn, has_region : bool,
) -> CloudSampleDensity {
  let column = cloud_column_at(m, deck, x, cloud_field_footprint(footprint), refine);
  if (!(column.mean_tau > 0.0)) {
    return CloudSampleDensity(0.0, 0.0, 0.0, region_in, has_region);
  }
  let height = cloud_height_of(x, deck);
  let features = cloud_shape_row(CLOUD_SHAPE_ROW_FEATURES);
  let base_rate = cloud_shape_row(CLOUD_SHAPE_ROW_BASE).w;
  let wb = cloud_noise_share(features.x, footprint) * base_rate;
  var wd = 0.0;
  if (detail) {
    wd = cloud_noise_share(features.y, footprint) * cloud_shape_row(CLOUD_SHAPE_ROW_DETAIL).w;
  }
  let w_cell = cloud_cumulus_share(column.cover) * cloud_noise_share(features.w, footprint) * base_rate
    * (1.0 - deck.flat);
  var nb = 0.5;
  if (wb > 0.0) {
    nb = cloud_base_at(x);
  }
  var nd = 0.5;
  if (wd > 0.0) {
    let plane = cloud_map_plane_at(m, x);
    nd = cloud_detail_at(x, plane.xy, m.east.xyz, m.north.xyz);
  }
  let sheet_top = column.top + deck.flat * (1.0 - column.top);
  let sheet_u = (height - deck.base_m) / (deck.depth_m * sheet_top);
  let sheet_tau = column.mean_tau / share;
  let sheet = cloud_density(sheet_tau, sheet_u, sheet_top, deck.depth_m, nb, nd, wb, wd, deck.flat);
  let sheet_above = cloud_share_above(sheet_u, deck.flat);
  if (!(w_cell > 0.0)) {
    return CloudSampleDensity(sheet, sheet_tau, sheet_above, region_in, has_region);
  }
  // The cell's pattern is the cells' noise on the layer's shell above or below the sample, its
  // cover and τ the region's.
  let on_shell = x * (deck.shell_m / length(vec3f(x.xy, x.z * deck.k)));
  var region = region_in;
  if (!has_region) {
    region = cloud_column_at(m, deck, x, max(footprint, CLOUD_CELL_REGION_M), false);
  }
  let cell = cloud_cell_of(
    region, column.fraction, cloud_cell_noise_at(on_shell), cloud_cell_height_noise_at(on_shell)
  );
  let cell_top = cell.z + deck.flat * (1.0 - cell.z);
  let cell_u = (height - deck.base_m) / (deck.depth_m * cell_top);
  let cell_tau = cell.x / share;
  let cell_sigma = cloud_density(cell_tau, cell_u, cell_top, deck.depth_m, nb, nd, wb, wd, deck.flat);
  return CloudSampleDensity(
    sheet + w_cell * (cell_sigma - sheet),
    sheet_tau + w_cell * (cell_tau - sheet_tau),
    sheet_above + w_cell * (cloud_share_above(cell_u, deck.flat) - sheet_above),
    region,
    true,
  );
}

// The sun's optical depth to a sample by the march towards it (`cloudMarch.ts`' `sunDepth`).
fn cloud_sun_depth(
  m : CloudMapPlane, deck : CloudMarchDeck, x : vec3f, height : f32, mu_sun : f32, sun : vec3f,
  footprint : f32, share : f32, steps : u32, region : CloudColumn, has_region : bool,
) -> f32 {
  let reach = min(
    max(deck.top_m - height, 0.0) / max(mu_sun, CLOUD_LIGHT_LEAST_MU),
    CLOUD_LIGHT_REACH_DEPTHS * deck.depth_m,
  );
  let n = f32(steps);
  var tau = 0.0;
  for (var j = 0u; j < steps; j = j + 1u) {
    let f = (f32(j) + 0.5) / n;
    let step = reach * (2.0 * f32(j) + 1.0) / (n * n);
    tau = tau
      + cloud_density_at(m, deck, x + reach * f * f * sun, footprint, share, false, false, region, has_region).sigma * step;
  }
  return tau;
}

// The pixel's footprint at a ray's entry into the deck, m (`cloudMarch.ts`' `entryFootprintM`).
fn cloud_entry_footprint(deck : CloudMarchDeck, o : vec3f, d : vec3f, t0 : f32, footprint_per_m : f32) -> f32 {
  let x0 = o + t0 * d;
  let normal = normalize(vec3f(x0.xy, x0.z * deck.k * deck.k));
  let mu_view = max(abs(dot(d, normal)), CLOUD_VOLUME_LEAST_MU);
  return max(footprint_per_m * t0 / mu_view, 1.0);
}

// The column at a ray's entry into the deck (`cloudMarch.ts`' `marchEntry`).
fn cloud_march_entry(m : CloudMapPlane, deck : CloudMarchDeck, o : vec3f, d : vec3f, t0 : f32, footprint_per_m : f32) -> CloudColumn {
  let footprint = cloud_entry_footprint(deck, o, d, t0, footprint_per_m);
  return cloud_column_at(m, deck, o + t0 * d, cloud_field_footprint(footprint), true);
}

// The conservative two-stream's diffuse reflectance (`cloudMarch.ts`' `diffuseReflectance`).
fn cloud_diffuse_reflectance(tau : f32, g : f32) -> f32 {
  let x = 0.75 * (1.0 - g) * max(tau, 0.0);
  return x / (1.0 + x);
}

// A view ray's march between `t0` and `t1` (`cloudMarch.ts`' `marchCloudRay`), lit by `sun0`,
// marched towards, and `sun1`, plane-parallel, where `suns` is 2; `entry` the column at its entry.
fn cloud_march(
  m : CloudMapPlane, deck : CloudMarchDeck, levers : CloudMarchLevers, o : vec3f, d : vec3f,
  t0 : f32, t1 : f32, sun0 : vec3f, sun1 : vec3f, suns : u32, footprint_per_m : f32, jitter : f32,
  entry : CloudColumn,
) -> CloudMarchResult {
  // The layer's share by the field's unresolved weight or the cells' (`cloudMarch.ts`' `rayShare`).
  let entry_footprint = cloud_entry_footprint(deck, o, d, t0, footprint_per_m);
  let cells = cloud_cumulus_share(entry.cover)
    * (1.0 - cloud_noise_share(cloud_shape_row(CLOUD_SHAPE_ROW_FEATURES).w, entry_footprint));
  let w = max(smoothstep(CLOUD_SHARE_SD_LEAST, CLOUD_SHARE_SD_MOST, entry.sd), cells);
  let weight_share = w + deck.flat * (1.0 - w);
  let share = 1.0 - weight_share * (1.0 - entry.fraction);
  var result = CloudMarchResult(vec2f(0.0), 0.0, 1.0, t0, share);
  if (!(share >= CLOUD_MARCH_LEAST_SHARE) || !(t1 > t0)) {
    return result;
  }
  // δ-Eddington's R and T_diff at the sky's cosine of the entry's column, which the flattening
  // mixes the sky's light to.
  var entry_slab = vec2f(0.0);
  let entry_tau = entry.mean_tau / share;
  if (deck.flat > 0.0 && entry_tau > 0.0) {
    let slab = cloud_delta_eddington(entry_tau, deck.asymmetry, deck.co_albedo, CLOUD_MARCH_SKY_COSINE);
    entry_slab = vec2f(slab.reflectance, slab.diffuse);
  }
  let length_m = t1 - t0;
  let p = 1.0 + clamp(log2(max(length_m / (8.0 * deck.depth_m), 1.0)) / 3.0, 0.0, 1.0);
  let n = f32(levers.view_steps);
  let albedo = 1.0 - deck.co_albedo;
  let cos0 = dot(sun0, d);
  let cos1 = dot(sun1, d);
  var light = vec2f(0.0);
  var sky = 0.0;
  var view = 1.0;
  var weighed = 0.0;
  var weight = 0.0;
  for (var s = 0u; s < levers.view_steps; s = s + 1u) {
    let v = (f32(s) + jitter) / n;
    let t = t0 + length_m * pow(max(v, 1e-9), p);
    let dt = length_m * p * pow(max(v, 0.5 / n), p - 1.0) / n;
    let x = o + t * d;
    let footprint = max(footprint_per_m * t, 1.0);
    let sample = cloud_density_at(
      m, deck, x, footprint, share, levers.detail > 0.5, true, CloudColumn(0.0, 0.0, 1.0, 0.0, 0.0, 0.0), false
    );
    if (!(sample.sigma > 0.0)) {
      continue;
    }
    let column_tau = sample.column_tau;
    let height = cloud_height_of(x, deck);
    let normal = normalize(vec3f(x.xy, x.z * deck.k * deck.k));
    let view_step = sample.sigma * dt;
    let scattered = view * sample.sigma * dt * cloud_escape(view_step);
    let horizon = -sqrt(2.0 * max(height, 0.0) / deck.datum);
    for (var j = 0u; j < suns; j = j + 1u) {
      var sun = sun0;
      var cos_theta = cos0;
      if (j == 1u) {
        sun = sun1;
        cos_theta = cos1;
      }
      let raw_mu = dot(normal, sun);
      if (raw_mu < horizon) {
        continue;
      }
      let mu_sun = max(raw_mu, CLOUD_VOLUME_LEAST_MU);
      let plane_parallel = column_tau * sample.above / mu_sun;
      var marched = plane_parallel;
      if (j == 0u && deck.flat < 1.0) {
        marched = cloud_sun_depth(
          m, deck, x, height, raw_mu, sun, footprint, share, levers.light_steps, sample.region, sample.has_region
        );
      }
      let tau_sun = marched + deck.flat * (plane_parallel - marched);
      let multiple = cloud_octave_correction_at(column_tau, mu_sun);
      var a = 1.0;
      var b = 1.0;
      var c = 1.0;
      var source = 0.0;
      for (var i = 0u; i < CLOUD_OCTAVE_COUNT; i = i + 1u) {
        let octave_weight = select(multiple, 1.0, i == 0u);
        source = source + octave_weight * b * albedo * cloud_two_lobe(cos_theta, deck.asymmetry, c) * exp(-a * tau_sun);
        a = a * CLOUD_OCTAVE_EXTINCTION;
        b = b * CLOUD_OCTAVE_SCATTERING;
        c = c * CLOUD_OCTAVE_PHASE;
      }
      if (j == 0u) {
        light.x = light.x + scattered * source;
      } else {
        light.y = light.y + scattered * source;
      }
    }
    let reflect = cloud_diffuse_reflectance(column_tau, deck.asymmetry);
    let slab_r = reflect + deck.flat * (entry_slab.x - reflect);
    let slab_t = 1.0 - reflect + deck.flat * (entry_slab.y - (1.0 - reflect));
    let opacity = 1.0 - exp(-view_step);
    sky = sky + view * opacity * (slab_r + sample.above * (slab_t - slab_r));
    weighed = weighed + view * opacity * t;
    weight = weight + view * opacity;
    view = view * exp(-view_step);
    if (view < CLOUD_MARCH_LEAST_VIEW) {
      break;
    }
  }
  result.sun = light;
  result.sky = sky;
  // The background through the clouds, the δ-scaled share (`cloudOptics.wgsl`).
  result.transmittance = cloud_background_transmittance(view, deck.asymmetry);
  if (weight > 0.0) {
    result.mean_t = weighed / weight;
  }
  return result;
}

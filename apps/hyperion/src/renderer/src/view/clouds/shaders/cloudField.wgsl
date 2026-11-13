// The clouds' one optical-depth field (plan R11, R11.T7.b; Design notes 8 and 10): at a place on the
// layer and a footprint, the share the clouds cover and their in-cloud τ(550), read by the layer's
// pass (`cloudLayer.wgsl`), the terrain's cloud light (`cloudShadow.wgsl`) and the volume (T7.c).
// The twin of `clouds/cloudDensity.ts`' `cloudFieldAt`, its f64 reference, whose remarks give the
// model: octaves of the noise tile summed to a standard normal z, the coverage met by the threshold
// z_c = Φ⁻¹(1 − C), the in-cloud τ linear in z − z_c, and the footprint's unresolved octaves taken
// as the expectation over them, so that the field filters itself.
//
// Composed after `view/shaders/preciseTrig.wgsl` (its angles go through `preciseAtan2` and
// `preciseSinCos`, never WGSL's trigonometry, signed by `cloud_atan2` and `cloud_sin_cos`). Its includer declares, at its own group and
// bindings:
//   - `cloud_layer : array<vec4f>`, read-only storage, the body's CLOUD_LAYER_ROWS rows
//     (`cloudLayer.ts`' `packCloudLayer`);
//   - `cloud_coverage : texture_2d<f32>`, the coverage texture, `rgba16float`, its texels z_c, N,
//     the wind eastward and northward, m s⁻¹ (`cloudDensity.ts`' `coverageTexels`);
//   - `cloud_noise : texture_3d<f32>`, the noise tile, `r8unorm` (`cloudNoise.ts`);
//   - `cloud_sampler : sampler`, linear and `repeat`.

// The body's rows: 0 the shell (x a and y c of the layer's spheroid, m; z its height above the
// datum, m; w τ̄); 1 the optics (x g, y 1 − ϖ₀, z the octaves, w ℓ₀, m); 2 the flow map (x and y each
// phase's advection time, s; z and w their weights); 3 x 1 ÷ √(Σρ²ᵏ) over the octaves, y the datum's
// equatorial radius a, m, which turns a place ÷ a into metres (`cloudShadow.wgsl`); 4 to 6 the
// octaves' time-rate fades, four a row; 7 to 9 the body's rotation, a row each; then octave k's
// phase i's offset, in tiles, at 10 + 2k + i.
const CLOUD_ROW_SHELL : u32 = 0u;
const CLOUD_ROW_OPTICS : u32 = 1u;
const CLOUD_ROW_FLOW : u32 = 2u;
const CLOUD_ROW_NORM : u32 = 3u;
const CLOUD_ROW_FADES : u32 = 4u;
const CLOUD_ROW_ROTATION : u32 = 7u;
const CLOUD_ROW_OFFSETS : u32 = 10u;
const CLOUD_LAYER_ROWS : u32 = 34u;

// `cloudDensity.ts`' constants.
const CLOUD_OCTAVES_MAX : u32 = 12u;
const CLOUD_OCTAVE_RATIO : f32 = 0.7;
const CLOUD_LEAST_UNRESOLVED_SD : f32 = 1e-3;
const CLOUD_LEAST_SHARE : f32 = 1e-6;
// `cloudNoise.ts`' CLOUD_NOISE_STD and CLOUD_NOISE_RANGE.
const CLOUD_NOISE_STD : f32 = 0.974;
const CLOUD_NOISE_RANGE : f32 = 4.0;
const CLOUD_PI : f32 = 3.14159265358979;

// The field at a footprint: the share covered, the mean τ over the footprint, the in-cloud τ, and
// z_r, the resolved structure, and s, the unresolved standard deviation (`cloudDensity.ts`'
// CloudSample).
struct CloudSample {
  fraction : f32,
  mean_tau : f32,
  in_cloud_tau : f32,
  resolved : f32,
  unresolved_sd : f32,
}

fn cloud_row(i : u32) -> vec4f {
  return cloud_layer[i];
}

// sin and cos of an angle, as (sin, cos), the angle first taken into [−π, π] by whole turns.
fn cloud_sin_cos(angle : f32) -> vec2f {
  let reduced = angle - 2.0 * CLOUD_PI * round(angle / (2.0 * CLOUD_PI));
  let magnitude = preciseSinCos(min(abs(reduced), CLOUD_PI));
  return vec2f(select(magnitude.x, -magnitude.x, reduced < 0.0), magnitude.y);
}

// atan2(y, x) in [−π, π], by `preciseAtan2`, which takes y ≥ 0 alone; 0 where both are 0.
fn cloud_atan2(y : f32, x : f32) -> f32 {
  if (!(abs(y) > 0.0 || abs(x) > 0.0)) {
    return 0.0;
  }
  let angle = preciseAtan2(abs(y), x);
  return select(angle, -angle, y < 0.0);
}

// `cloudDensity.ts`' OCTAVE_ROTATION, by rows.
fn cloud_octave_turn(c : vec3f) -> vec3f {
  return vec3f(
    dot(vec3f(0.0, 0.8, 0.6), c),
    dot(vec3f(-0.8, 0.36, -0.48), c),
    dot(vec3f(-0.6, -0.48, 0.64), c),
  );
}

// The body's rotation of the octaves' coordinates.
fn cloud_body_turn(p : vec3f) -> vec3f {
  return vec3f(
    dot(cloud_row(CLOUD_ROW_ROTATION).xyz, p),
    dot(cloud_row(CLOUD_ROW_ROTATION + 1u).xyz, p),
    dot(cloud_row(CLOUD_ROW_ROTATION + 2u).xyz, p),
  );
}

// Where the structure at `p` came from `advection_s` earlier under the wind there (east, north),
// `cloudDensity.ts`' `advected`: turned about the pole, then along the meridian.
fn cloud_advected(p : vec3f, wind : vec2f, advection_s : f32) -> vec3f {
  let rho = length(p.xy);
  var q = p;
  if (rho > 0.0) {
    let sc = cloud_sin_cos(-wind.x * advection_s / rho);
    q = vec3f(sc.y * p.x - sc.x * p.y, sc.x * p.x + sc.y * p.y, p.z);
  }
  let r = length(q);
  if (wind.y == 0.0 || !(rho > 0.0) || !(r > 0.0)) {
    return q;
  }
  let sc = cloud_sin_cos(wind.y * advection_s / r);
  let scale = (sc.y * rho + sc.x * q.z) / rho;
  return vec3f(q.xy * scale, sc.y * q.z - sc.x * rho);
}

// The coverage texture at a place: bilinear, `repeat` around a parallel, held at the outer rows'
// centres towards the poles (`cloudDensity.ts`' `sampleCoverage`).
fn cloud_coverage_at(p : vec3f) -> vec4f {
  let size = vec2f(textureDimensions(cloud_coverage));
  let latitude = cloud_atan2(p.z, length(p.xy));
  let longitude = cloud_atan2(p.y, p.x);
  let u = (longitude + CLOUD_PI) / (2.0 * CLOUD_PI);
  let v = clamp((0.5 * CLOUD_PI - latitude) / CLOUD_PI, 0.5 / size.y, 1.0 - 0.5 / size.y);
  return textureSampleLevel(cloud_coverage, cloud_sampler, vec2f(u, v), 0.0);
}

// Q(x), the standard normal's upper tail, by Numerical Recipes' erfc (`cloudDensity.ts`' normalTail).
fn cloud_normal_tail(x : f32) -> f32 {
  let z = abs(x) * 0.70710678;
  let t = 1.0 / (1.0 + 0.5 * z);
  let poly = -z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418
    + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587
    + t * (-0.82215223 + t * 0.17087277))))))));
  let erfc = t * exp(poly);
  return select(1.0 - 0.5 * erfc, 0.5 * erfc, x >= 0.0);
}

// φ(x), the standard normal density.
fn cloud_normal_density(x : f32) -> f32 {
  return exp(-0.5 * x * x) * 0.39894228;
}

// One octave's noise at its coordinates, of unit variance.
fn cloud_noise_at(c : vec3f) -> f32 {
  let stored = textureSampleLevel(cloud_noise, cloud_sampler, c, 0.0).r;
  return (stored * 2.0 * CLOUD_NOISE_RANGE - CLOUD_NOISE_RANGE) / CLOUD_NOISE_STD;
}

// The field at `p`, body-fixed metres on the layer, over a footprint of `footprint_m`, drawing at
// most `octave_cap` octaves (the rest unresolved: a cheaper reader's choice that keeps the coverage).
fn cloud_field_at(p : vec3f, footprint_m : f32, octave_cap : u32) -> CloudSample {
  let cell = cloud_coverage_at(p);
  let flow = cloud_row(CLOUD_ROW_FLOW);
  let optics = cloud_row(CLOUD_ROW_OPTICS);
  let octaves = min(min(u32(optics.z), octave_cap), CLOUD_OCTAVES_MAX);
  let norm = 1.0 / length(flow.zw);
  let tile = 1.0 / (8.0 * optics.w);
  var c0 = cloud_body_turn(cloud_advected(p, cell.zw, flow.x)) * tile;
  var c1 = cloud_body_turn(cloud_advected(p, cell.zw, flow.y)) * tile;
  var resolved = 0.0;
  var resolved_variance = 0.0;
  var amplitude = 1.0;
  var cell_m = optics.w;
  for (var k = 0u; k < octaves; k = k + 1u) {
    let reach = clamp(log2(cell_m / (2.0 * footprint_m)), 0.0, 1.0);
    let weight = reach * cloud_row(CLOUD_ROW_FADES + k / 4u)[k % 4u];
    if (!(weight > 0.0)) {
      break;
    }
    let n0 = cloud_noise_at(c0 + cloud_row(CLOUD_ROW_OFFSETS + 2u * k).xyz);
    let n1 = cloud_noise_at(c1 + cloud_row(CLOUD_ROW_OFFSETS + 2u * k + 1u).xyz);
    let n = (flow.z * n0 + flow.w * n1) * norm;
    resolved = resolved + weight * amplitude * n;
    resolved_variance = resolved_variance + (weight * amplitude) * (weight * amplitude);
    c0 = cloud_octave_turn(c0) * 2.0;
    c1 = cloud_octave_turn(c1) * 2.0;
    amplitude = amplitude * CLOUD_OCTAVE_RATIO;
    cell_m = cell_m * 0.5;
  }
  let inverse_total = cloud_row(CLOUD_ROW_NORM).x;
  let zr = resolved * inverse_total;
  let unresolved = max(1.0 - resolved_variance * inverse_total * inverse_total, 0.0);
  let s = max(sqrt(unresolved), CLOUD_LEAST_UNRESOLVED_SD);
  let d = (cell.x - zr) / s;
  let q = cloud_normal_tail(d);
  let mean_tau = max(cloud_row(CLOUD_ROW_SHELL).w * cell.y * s * (cloud_normal_density(d) - d * q), 0.0);
  return CloudSample(q, mean_tau, mean_tau / max(q, CLOUD_LEAST_SHARE), zr, s);
}

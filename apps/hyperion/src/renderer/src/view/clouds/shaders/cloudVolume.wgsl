// The cloud volume's light (plan R11, R11.T7.c; Design note 10): the high setting's march through a
// condensate's deck, its two-lobe phase, Wrenninge et al. 2013's multiple-scattering octaves and
// their correction to δ-Eddington. The twin of `clouds/cloudVolume.ts`, its f64 reference, whose
// remarks give the model. Composed after `view/shaders/preciseTrig.wgsl`, `cloudField.wgsl` and
// `cloudOptics.wgsl`; its includer declares, besides `cloudField.wgsl`'s bindings:
//   - `cloud_octave_correction : texture_2d<f32>`, the condensate's K(τ, μ₀) on the octaves'
//     multiple scattering, `r32float`, CLOUD_CORRECTION_TAUS × CLOUD_CORRECTION_MUS, read by
//     `textureLoad` (`cloudVolume.ts`' `octaveCorrectionOf`).
//
// A column's τ is the field's mean τ over its footprint (`cloud_field_at`), read on the layer's shell
// above the sample so that every height of a column reads the same, and spread uniformly through the
// deck's depth: the layer is the volume's independent-pixel approximation. The sun's optical depth
// from a sample to the deck's top is the column's share above it over the sun's local zenith cosine,
// a plane-parallel column's (lean, 2026-11-13); a sample whose sun is below its horizon takes none.
// R08's transmittance to the deck is its caller's.

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
    let view_step = a * dt / view_mu;
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

// The march of a view ray from `o` along unit `d`, body-fixed m, through the deck between `base_m`
// and `top_m` above the datum, lit by the source along unit `sun`: x the radiance towards the camera
// per unit of the source's irradiance face-on, sr⁻¹; y the view's transmittance through the deck.
// Its footprint grows as `footprint_per_m` of the distance, at least 1 m. The deck's surfaces are
// spheroids of the layer's flattening, of semi-axes a + h and (a + h) ÷ k for the layer's k = a_s ÷
// c_s; the ray's first stretch through the deck is marched.
fn cloud_volume_march(
  o : vec3f, d : vec3f, sun : vec3f, base_m : f32, top_m : f32, steps : u32, footprint_per_m : f32,
) -> vec2f {
  let shell = cloud_row(CLOUD_ROW_SHELL);
  let optics = cloud_row(CLOUD_ROW_OPTICS);
  let datum = cloud_row(CLOUD_ROW_NORM).y;
  let k = shell.x / shell.y;
  let outer = cloud_spheroid_hits(o, d, k, datum + top_m);
  if (outer.y <= 0.0) {
    return vec2f(0.0, 1.0);
  }
  let inner = cloud_spheroid_hits(o, d, k, datum + base_m);
  var t0 = max(outer.x, 0.0);
  var t1 = outer.y;
  if (inner.x > t0) {
    // From above: the stretch ends where the ray meets the deck's base.
    t1 = inner.x;
  } else if (inner.x < 0.0 && inner.y > t0) {
    // From beneath the base: the deck begins where the ray leaves it.
    t0 = inner.y;
  }
  return cloud_volume_march_between(o, d, sun, base_m, top_m, t0, t1, steps, footprint_per_m, k, shell, optics, datum);
}

fn cloud_volume_march_between(
  o : vec3f, d : vec3f, sun : vec3f, base_m : f32, top_m : f32, t0 : f32, t1 : f32, steps : u32,
  footprint_per_m : f32, k : f32, shell : vec4f, optics : vec4f, datum : f32,
) -> vec2f {
  let depth = max(top_m - base_m, 1.0);
  let dt = (t1 - t0) / f32(steps);
  let cos_theta = dot(sun, d);
  let albedo = 1.0 - optics.y;
  var view = array<f32, 4>(1.0, 1.0, 1.0, 1.0);
  var radiance = 0.0;
  for (var s = 0u; s < steps; s = s + 1u) {
    let t = t0 + (f32(s) + 0.5) * dt;
    let x = o + t * d;
    let sx = vec3f(x.xy, x.z * k);
    let scaled = length(sx);
    let height = scaled - datum;
    // The column above the sample, read on the layer's shell.
    let on_shell = x * (shell.x / scaled);
    let field = cloud_field_at(on_shell, max(footprint_per_m * t, 1.0), CLOUD_OCTAVES_MAX);
    let column = field.mean_tau;
    let sigma = column / depth;
    let normal = normalize(vec3f(x.xy, x.z * k * k));
    let raw_mu = dot(normal, sun);
    // Below its horizon a sample takes no sun.
    let lit = raw_mu >= -sqrt(2.0 * max(height, 0.0) / datum);
    let mu_sun = max(raw_mu, CLOUD_VOLUME_LEAST_MU);
    let above = column * clamp((top_m - height) / depth, 0.0, 1.0);
    let multiple = cloud_octave_correction_at(column, mu_sun);
    var a = 1.0;
    var b = 1.0;
    var c = 1.0;
    for (var i = 0u; i < CLOUD_OCTAVE_COUNT; i = i + 1u) {
      let view_step = a * sigma * dt;
      let weight = select(multiple, 1.0, i == 0u);
      var source = 0.0;
      if (lit) {
        source = weight * b * albedo * cloud_two_lobe(cos_theta, optics.x, c) * exp(-a * above / mu_sun);
      }
      radiance = radiance + view[i] * source * sigma * dt * cloud_escape(view_step);
      view[i] = view[i] * exp(-view_step);
      a = a * CLOUD_OCTAVE_EXTINCTION;
      b = b * CLOUD_OCTAVE_SCATTERING;
      c = c * CLOUD_OCTAVE_PHASE;
    }
  }
  return vec2f(radiance, view[0]);
}

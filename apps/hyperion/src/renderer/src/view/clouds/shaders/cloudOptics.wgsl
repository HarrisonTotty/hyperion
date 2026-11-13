// A cloud layer's optics in f32 (plan R11, R11.T7.b; Design note 10): the δ-Eddington reflectance
// and the direct and diffuse transmittance of a plane-parallel layer lit by a direct beam, the
// shaders' twin of `clouds/cloudOptics.ts`' `deltaEddington`, which is its f64 reference. Declares
// no binding. With T7.a.1's notes for f32 (plan R11, T7.b):
//   - only decaying exponentials: e^(−kτ′), e^(−2kτ′) as (e^(−kτ′))², e^(−τ′ ÷ μ₀), and the direct
//     transmittance e^(−τ ÷ μ₀) with the unscaled τ, the diffuse T − T_dir;
//   - the co-albedo 1 − ϖ₀ passed, not ϖ₀, and 1 − ϖ′ formed as (1 − ϖ₀) ÷ (1 − ϖ₀ f);
//   - the conservative solution at a scaled co-albedo 1 − ϖ′ ≤ CLOUD_CONSERVATIVE_CO_ALBEDO, 10⁻⁶,
//     re-derived for f32: the general solution's error grows as ε₃₂ ÷ k, ε₃₂ = 2⁻²³ (k ≈ 1.3 × 10⁻³
//     at 1 − ϖ′ = 10⁻⁶ and g 0.85), and the absorptance the conservative side drops there is at most
//     3.3 × 10⁻⁵ at τ ≤ 50, so the two errors meet near it. Emulated in correctly rounded f32 over
//     1 − ϖ₀ 10⁻⁹ to 0.5, g 0 to 0.9, μ₀ 0.02 to 1 and τ 0.01 to 50, this form is within 8.0 × 10⁻⁵ of
//     the f64 one in R and 4.3 × 10⁻⁵ in T (2026-11-02); WGSL's own `exp`, bounded to 3 + 2|x| ULP,
//     adds to that, which the smoke harness bounds;
//   - the removable singularity at kμ₀ = 1 stepped around, R and T − e^(−τ′ ÷ μ₀) interpolated
//     linearly in μ₀ across CLOUD_SINGULAR_HALF_WIDTH, 5 × 10⁻³, near ε₃₂^(1/3), as the f64 form does
//     across 10⁻⁵;
//   - the diffuse transmittance clamped to ≥ 0, since a thin layer leaves it at round-off.
// Meador and Weaver 1980 (the coefficients and both solutions) and Joseph, Wiscombe and Weinman 1976
// (the δ-scaling), as `cloudOptics.ts` cites them.

const CLOUD_CONSERVATIVE_CO_ALBEDO : f32 = 1e-6;
const CLOUD_SINGULAR_HALF_WIDTH : f32 = 5e-3;

// R, T_dir and T_diff, each a fraction of the beam's flux on a horizontal surface, μ₀ π F₀.
struct CloudSlab {
  reflectance : f32,
  direct : f32,
  diffuse : f32,
}

// The general solution's scattered part at one μ₀ (`cloudOptics.ts`' closedForm): x R, y T less the
// scaled direct beam e^(−τ′ ÷ μ₀). `w` is ϖ′, `gs` g′ and `t` τ′.
fn cloud_scattered(t : f32, w : f32, gs : f32, mu0 : f32, k : f32) -> vec2f {
  let g1 = (7.0 - w * (4.0 + 3.0 * gs)) / 4.0;
  let g2 = -(1.0 - w * (4.0 - 3.0 * gs)) / 4.0;
  let g3 = (2.0 - 3.0 * gs * mu0) / 4.0;
  let g4 = 1.0 - g3;
  let a1 = g1 * g4 + g2 * g3;
  let a2 = g1 * g3 + g2 * g4;
  let km = k * mu0;
  let eK = exp(-k * t);
  let e2K = eK * eK;
  let eSun = exp(-t / mu0);
  let scale = w / ((1.0 - km * km) * (k + g1 + (k - g1) * e2K));
  let r = scale * ((1.0 - km) * (a2 + k * g3) - (1.0 + km) * (a2 - k * g3) * e2K
    - 2.0 * k * (g3 - a2 * mu0) * eSun * eK);
  let s = -scale * ((1.0 + km) * (a1 + k * g4) * eSun - (1.0 - km) * (a1 - k * g4) * e2K * eSun
    - 2.0 * k * (g4 + a1 * mu0) * eK);
  return vec2f(r, s);
}

// The δ-Eddington layer of optical depth `tau`, asymmetry `g` in [0, 1) and co-albedo 1 − ϖ₀
// `co_albedo` under a beam at zenith cosine `mu0` in (0, 1].
fn cloud_delta_eddington(tau : f32, g : f32, co_albedo : f32, mu0 : f32) -> CloudSlab {
  let f = g * g;
  let remaining = 1.0 - f + co_albedo * f;
  let t = remaining * tau;
  let cs = co_albedo / remaining;
  let gs = g / (1.0 + g);
  let direct = exp(-tau / mu0);
  var reflectance : f32;
  var total : f32;
  if (cs <= CLOUD_CONSERVATIVE_CO_ALBEDO) {
    let g1 = 3.0 * (1.0 - gs) / 4.0;
    let g3 = (2.0 - 3.0 * gs * mu0) / 4.0;
    reflectance = (g1 * t + (g3 - g1 * mu0) * (1.0 - exp(-t / mu0))) / (1.0 + g1 * t);
    total = 1.0 - reflectance;
  } else {
    let w = 1.0 - cs;
    let k = sqrt(3.0 * cs * (1.0 - w * gs));
    var part : vec2f;
    if (abs(1.0 - k * mu0) >= CLOUD_SINGULAR_HALF_WIDTH) {
      part = cloud_scattered(t, w, gs, mu0, k);
    } else {
      let below = (1.0 - CLOUD_SINGULAR_HALF_WIDTH) / k;
      let above = (1.0 + CLOUD_SINGULAR_HALF_WIDTH) / k;
      let lower = cloud_scattered(t, w, gs, below, k);
      let upper = cloud_scattered(t, w, gs, above, k);
      part = mix(lower, upper, (mu0 - below) / (above - below));
    }
    reflectance = part.x;
    total = exp(-t / mu0) + part.y;
  }
  return CloudSlab(clamp(reflectance, 0.0, 1.0), direct, max(total - direct, 0.0));
}

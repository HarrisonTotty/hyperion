// The shading of a lit body's surface (plan R07, Design notes 5, 6 and 24): the body BRDF, a
// lunar-Lambert disc term times a phase factor f read from a table, as reflectance I/F; the horizon
// and eclipse terms of a lit point; and the hooks R08 and R11 fill, as stubs.
//
// A library composed by concatenation: it declares no binding of its own. Its includer declares
// `phase_factor_table : texture_2d<f32>` at the includer's own group and binding (an
// `rgba32float` texture read with textureLoad, so unfilterable-float): one row per tabulated law,
// PHASE_TABLE_SAMPLES texels from 0 to π every 0.5°, f in r, g and b (alpha unused). The
// TypeScript twin is `view/appearance/brdf.ts`, which reads the same table the same way.

const LIT_PI : f32 = 3.14159265358979;

// The phase-factor table's samples and step, rad: 0° to 180° every 0.5° (`law.ts`).
const PHASE_TABLE_SAMPLES : u32 = 361u;
const PHASE_TABLE_STEP_RAD : f32 = 0.00872664625997165;

// A lunar-Lambert law, which a caller may build per texel (R10): the albedo scale A per channel,
// the Lommel–Seeliger share L, the exponents s per channel that its table row was made with, and
// `table_row`, the row of `phase_factor_table` holding its f (one row per tabulated law: template,
// L and s). The shader reads f from the row; s rides along for callers that rebuild rows.
struct LunarLambert {
  a : vec3f,
  l : f32,
  s : vec3f,
  table_row : u32,
}

// f per channel at phase α, by linear interpolation between the row's two nearest samples.
fn phase_factor(row : u32, alpha : f32) -> vec3f {
  let x = clamp(alpha, 0.0, LIT_PI) / PHASE_TABLE_STEP_RAD;
  let i = min(u32(floor(x)), PHASE_TABLE_SAMPLES - 2u);
  let t = x - f32(i);
  let below = textureLoad(phase_factor_table, vec2u(i, row), 0).rgb;
  let above = textureLoad(phase_factor_table, vec2u(i + 1u, row), 0).rgb;
  return below + t * (above - below);
}

// I/F = A · f(α) · [L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀]; dark where μ₀ ≤ 0 or μ ≤ 0.
fn body_brdf(law : LunarLambert, mu0 : f32, mu : f32, alpha : f32) -> vec3f {
  if (mu0 <= 0.0 || mu <= 0.0) {
    return vec3f(0.0);
  }
  let disc = law.l * 2.0 * mu0 / (mu0 + mu) + (1.0 - law.l) * mu0;
  return law.a * phase_factor(law.table_row, alpha) * disc;
}

// The horizon term (Design note 6): the irradiance from a uniform sphere of H = d ÷ R★ on an
// element whose normal is φ from the sphere's centre, over that of the sphere face-on, above a local
// horizon of elevation `horizon` towards the star (0 on the smooth figure; R10's on terrain). The
// TypeScript twin is `view/lighting/sphereIrradiance.ts`.
fn howell_view_factor(h : f32, phi_in : f32) -> f32 {
  let phi = min(abs(phi_in), LIT_PI);
  let h2 = h * h;
  let edge = acos(1.0 / h);
  if (phi <= edge) {
    return cos(phi) / h2;
  }
  if (phi >= LIT_PI - edge) {
    return 0.0;
  }
  let x = sqrt(h2 - 1.0);
  let y = -x / tan(phi);
  let root = sqrt(max(0.0, 1.0 - y * y));
  return (cos(phi) * acos(clamp(y, -1.0, 1.0)) - x * sin(phi) * root) / (LIT_PI * h2)
    + atan(sin(phi) * root / x) / LIT_PI;
}

fn disc_visible_fraction(h : f32, phi : f32) -> f32 {
  let rho = asin(1.0 / h);
  let x = clamp((0.5 * LIT_PI - phi) / rho, -1.0, 1.0);
  return (acos(-x) + x * sqrt(1.0 - x * x)) / LIT_PI;
}

fn sphere_irradiance(h : f32, phi : f32, horizon : f32) -> f32 {
  let eta = max(horizon, 0.0);
  if (eta == 0.0) {
    return h * h * howell_view_factor(h, phi);
  }
  let tilted = phi + eta;
  let direct = cos(eta) * h * h * howell_view_factor(h, tilted);
  let tangential = sin(eta) * sin(tilted) * disc_visible_fraction(h, tilted);
  return max(0.0, direct + tangential);
}

// The exact area of the overlap of circles of radii r and k whose centres are z apart.
fn circle_overlap_area(r : f32, k : f32, z : f32) -> f32 {
  if (r <= 0.0 || k <= 0.0 || z >= r + k) {
    return 0.0;
  }
  if (z <= abs(r - k)) {
    let small = min(r, k);
    return LIT_PI * small * small;
  }
  let d1 = (z * z + r * r - k * k) / (2.0 * z);
  let d2 = z - d1;
  let s1 = r * r * acos(clamp(d1 / r, -1.0, 1.0)) - d1 * sqrt(max(r * r - d1 * d1, 0.0));
  let s2 = k * k * acos(clamp(d2 / k, -1.0, 1.0)) - d2 * sqrt(max(k * k - d2 * d2, 0.0));
  return s1 + s2;
}

fn power2_moment(c : f32, alpha : f32, mu : f32) -> f32 {
  return (1.0 - c) * mu * mu * 0.5 + c * pow(mu, alpha + 2.0) / (alpha + 2.0);
}

// The eclipse term (Design note 6): the fraction of a power-2 disc's flux (I(μ)/I(1) =
// 1 − c(1 − μ^α)) left visible by one occluder, both given as angular radii and the angle between
// their centres, by `annuli` annuli uniform in μ, each eclipsed by the difference of two exact
// overlaps. The TypeScript twin is `view/lighting/annuli.ts`'s `annulusVisibleFraction`.
fn eclipse_visible(
  star_radius : f32,
  limb_c : f32,
  limb_alpha : f32,
  annuli : u32,
  occluder_radius : f32,
  separation : f32,
) -> f32 {
  let ratio = occluder_radius / star_radius;
  let z = separation / star_radius;
  if (z >= 1.0 + ratio) {
    return 1.0;
  }
  let total = power2_moment(limb_c, limb_alpha, 1.0);
  let k = f32(annuli);
  var hidden = 0.0;
  var inner_overlap = 0.0;
  var inner_edge = 0.0;
  var mu_inner = 1.0;
  for (var j = 1u; j <= annuli; j = j + 1u) {
    let mu_outer = 1.0 - f32(j) / k;
    let outer_edge = sqrt(1.0 - mu_outer * mu_outer);
    let outer_overlap = circle_overlap_area(outer_edge, ratio, z);
    let flux = (power2_moment(limb_c, limb_alpha, mu_inner) - power2_moment(limb_c, limb_alpha, mu_outer)) / total;
    let area = LIT_PI * (outer_edge * outer_edge - inner_edge * inner_edge);
    hidden = hidden + flux * (outer_overlap - inner_overlap) / area;
    inner_overlap = outer_overlap;
    inner_edge = outer_edge;
    mu_inner = mu_outer;
  }
  return max(0.0, 1.0 - hidden);
}

// R11's hook (its Design note 14): the ring's shadow on the body, 1 until `ringShadow.wgsl`
// replaces this stub, and not called where R11's `ringShadowOnBody` setting is off.
fn ring_shadow_on_body(p : vec3f, sun : vec3f) -> vec3f {
  return vec3f(1.0);
}

// R08's hooks (R08.T9.b, its Design note 17), with their full signatures: each sun's
// transmittance through the atmosphere, 1, and the sky's irradiance, 0, until
// `surfaceLighting.wgsl` replaces them; a body without an atmosphere keeps these.
fn atmosphere_sun_transmittance(
  altitude_m : f32,
  mu_sun : f32,
  latitude_rad : f32,
  sun_azimuth_rad : f32,
) -> vec3f {
  return vec3f(1.0);
}

fn atmosphere_sky_irradiance(altitude_m : f32, mu_sun : f32, latitude_rad : f32) -> vec3f {
  return vec3f(0.0);
}

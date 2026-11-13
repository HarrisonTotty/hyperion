// The shading of a lit body's surface (plan R07, Design notes 5, 6, 7 and 24): the body BRDF, a
// lunar-Lambert disc term times a phase factor f read from a table, as reflectance I/F; the horizon
// and eclipse terms of a lit point; planetshine's irradiance; and R11's hook, as a stub (R08's are
// `surfaceLighting.wgsl`'s since plan R08's R08.T9.b).
//
// A library composed by concatenation: it declares no binding of its own. Its includer declares
// `phase_factor_table : texture_2d<f32>` at the includer's own group and binding (an
// `rgba32float` texture read with textureLoad, so unfilterable-float): one row per tabulated law,
// PHASE_TABLE_SAMPLES texels from 0 to π every 0.5°, f in r, g and b and the law's L(α) in a (plan
// R10, R10.T10.b; decision-r07-t8b), L(0) at every sample for a law without a curve. The
// TypeScript twin is `view/appearance/brdf.ts`, which reads the same table the same way.

const LIT_PI : f32 = 3.14159265358979;

// The phase-factor table's samples and step, rad: 0° to 180° every 0.5° (`law.ts`).
const PHASE_TABLE_SAMPLES : u32 = 361u;
const PHASE_TABLE_STEP_RAD : f32 = 0.00872664625997165;

// A lunar-Lambert law: the albedo scale A per channel, its Lommel–Seeliger share at opposition
// L(0), the exponents s per channel that its table row was made with, and `table_row`, the row of
// `phase_factor_table` holding its f and L(α) (one row per tabulated law: template, s and L(α)).
// The shader reads f and L(α) from the row; `l` is L(0), for p, A and the figure's albedo scale,
// and s rides along for callers that rebuild rows. A texel of several classes (R10) reflects the
// weighted sum of its classes' `body_brdf`, each class keeping its own LunarLambert and row, never
// one law built from weighted parameters (decision-r07-t8b).
struct LunarLambert {
  a : vec3f,
  l : f32,
  s : vec3f,
  table_row : u32,
}

// The row at phase α, f per channel in rgb and L(α) in a, by linear interpolation between its two
// nearest samples.
fn phase_row(row : u32, alpha : f32) -> vec4f {
  let x = clamp(alpha, 0.0, LIT_PI) / PHASE_TABLE_STEP_RAD;
  let i = min(u32(floor(x)), PHASE_TABLE_SAMPLES - 2u);
  let t = x - f32(i);
  let below = textureLoad(phase_factor_table, vec2u(i, row), 0);
  let above = textureLoad(phase_factor_table, vec2u(i + 1u, row), 0);
  return below + t * (above - below);
}

// f per channel at phase α.
fn phase_factor(row : u32, alpha : f32) -> vec3f {
  return phase_row(row, alpha).rgb;
}

// I/F = A · f(α) · [L(α) · 2μ₀ ÷ (μ₀ + μ) + (1 − L(α)) μ₀], f and L(α) from the law's row; dark
// where μ₀ ≤ 0 or μ ≤ 0.
fn body_brdf(law : LunarLambert, mu0 : f32, mu : f32, alpha : f32) -> vec3f {
  if (mu0 <= 0.0 || mu <= 0.0) {
    return vec3f(0.0);
  }
  let row = phase_row(law.table_row, alpha);
  let disc = row.a * 2.0 * mu0 / (mu0 + mu) + (1.0 - row.a) * mu0;
  return law.a * row.rgb * disc;
}

// The horizon term (Design note 6): the irradiance from a uniform sphere of H = d ÷ R★ on an
// element whose normal is φ from the sphere's centre, over that of the sphere face-on, above a local
// horizon of elevation `horizon` towards the star (0 on the smooth figure; R10's on terrain). The
// TypeScript twin is `view/lighting/sphereIrradiance.ts`.
//
// It calls none of WGSL's `sin`, `cos`, `tan`, `asin`, `acos` or `atan` (R07.T6.d; R08 Design note
// 18), and takes `preciseTrig.wgsl`'s forms, which every includer composes with R08's surface
// lighting. Howell's partial branch is rewritten so that nothing in it cancels that matters of
// face-on: as written, the terms of H² atan z − X sin φ √(1 − Y²) are each about H ÷ π of face-on
// and cancel to about 2 ÷ (3H²) of themselves at 90° (1.4 × 10⁻⁵ at 1 au), so that a relative
// error ε in `atan` arrived as (H ÷ π) ε of face-on, 68 ε at 1 au, and Mesa's `nir_atan`
// (ε = −2.07 × 10⁻⁵, the UHD 620's driver on Linux) took 1.4 × 10⁻³ of face-on off the soft
// terminator and every terrain shadow edge (decision-r10-t8b §2). No H² is formed, so the term is
// finite at every finite H and, where the implementation keeps infinities, at H = ∞ (ρ = 0), the
// hard step.

// (atan z − z) ÷ z³ for z ≥ 0, −1/3 at 0: three halvings, (atan z − z) = 2 (atan t − t) −
// z³ ÷ (1 + √(1 + z²))² with t = z ÷ (1 + √(1 + z²)), take any z to t ≤ tan(π ÷ 16) = 0.199, since
// atan t = atan z ÷ 8 < π ÷ 16, and there the series −1/3 + t²/5 − t⁴/7 + t⁶/9 − t⁸/11 truncates at
// under t¹⁰/13 = 7.5 × 10⁻⁹, 2.3 × 10⁻⁸ of itself. Per halving, B(z) = (2 B(t) − p) ÷ p³ with
// p = 1 + √(1 + z²): both terms negative, so nothing cancels, and a relative error in B(t) reaches
// B(z) at under a quarter of itself.
fn atan_tail(z : f32) -> f32 {
  let p0 = 1.0 + sqrt(1.0 + z * z);
  let t1 = z / p0;
  let p1 = 1.0 + sqrt(1.0 + t1 * t1);
  let t2 = t1 / p1;
  let p2 = 1.0 + sqrt(1.0 + t2 * t2);
  let t3 = t2 / p2;
  let t = t3 * t3;
  var b = -1.0 / 3.0 + t * (1.0 / 5.0 + t * (-1.0 / 7.0 + t * (1.0 / 9.0 + t * (-1.0 / 11.0))));
  b = (2.0 * b - p2) / (p2 * p2 * p2);
  b = (2.0 * b - p1) / (p1 * p1 * p1);
  return (2.0 * b - p0) / (p0 * p0 * p0);
}

// H² F, Howell's view factor over the sphere's face-on: cos φ with the whole disc up (cos φ ≥ q,
// q = 1 ÷ H = sin ρ), 0 with none, and between them (Howell's partial branch, X = √(H² − 1),
// Y = −X cot φ, z = sin φ √(1 − Y²) ÷ X)
//
//   π H² F = cos φ arccos Y − X sin φ √(1 − Y²) + H² atan z = cos φ arccos Y + z + H² (atan z − z),
//
// an identity, as X sin φ √(1 − Y²) = X² z = (H² − 1) z. In the band's coordinate c = H cos φ,
// |c| < 1, with cos ρ = √(1 − q²): Y = −c cos ρ ÷ sin φ, √(1 − Y²) = √(1 − c²) ÷ sin φ, so arccos Y
// = atan2(√(1 − c²), −c cos ρ) (sin φ > 0 in the band), and z = q u with u = √(1 − c²) ÷ cos ρ, so
//
//   H² F = (q ÷ π) [c arccos Y + u (1 + u² B(q u))],   B(z) = (atan z − z) ÷ z³ (`atan_tail`).
//
// The bracket's terms are of order one, and 1 + u² B = (atan z − z cos² ρ) ÷ (z sin² ρ) lies in
// (0, 1), least at c = 0: 2/3 for H ≫ 1, 0.65 at H = 3, 0.19 at H = 1.01, where H itself is
// coarse in f32. Only at the band's far edge, c → −1, do the bracket's two parts cancel, to order
// (1 − c²)^(5/2) from (1 − c²)^(1/2), where the term is under 3 × 10⁻⁸ of face-on off and the
// clamp at 0 takes its rounding.
fn howell_irradiance(h : f32, phi_in : f32) -> f32 {
  let phi = min(abs(phi_in), LIT_PI);
  let q = 1.0 / h;
  let cos_phi = preciseSinCos(phi).y;
  if (cos_phi >= q) {
    return cos_phi;
  }
  if (cos_phi <= -q) {
    return 0.0;
  }
  let c = clamp(cos_phi * h, -1.0, 1.0);
  let cos_rho = sqrt((1.0 - q) * (1.0 + q));
  let chord = sqrt((1.0 - c) * (1.0 + c));
  let u = chord / cos_rho;
  let arc = preciseAtan2(chord, -c * cos_rho);
  return q / LIT_PI * (c * arc + u * (1.0 + u * u * atan_tail(q * u)));
}

// The share of the disc above a plane at `phi` from its centre, as a flat disc of angular radius
// ρ = asin(1 ÷ H) cut by a line at x = (π ÷ 2 − φ) ÷ ρ: (arccos(−x) + x √(1 − x²)) ÷ π, and for
// ρ = 0 the step, ½ on the plane.
fn disc_visible_fraction(h : f32, phi : f32) -> f32 {
  let q = 1.0 / h;
  let rho = preciseAtan2(q, sqrt((1.0 - q) * (1.0 + q)));
  let offset = 0.5 * LIT_PI - phi;
  if (abs(offset) >= rho) {
    return select(select(0.5, 0.0, offset < 0.0), 1.0, offset > 0.0);
  }
  let x = clamp(offset / rho, -1.0, 1.0);
  let root = sqrt((1.0 - x) * (1.0 + x));
  return (preciseAtan2(root, -x) + x * root) / LIT_PI;
}

// The horizon term above a local horizon η = max(horizon, 0): Howell's form at φ, clamped at 0,
// where η = 0; otherwise the disc cut at φ + η and weighed by the element's own normal,
// cos η H² F(φ + η) + sin η sin(φ + η) × the share above the cut (`sphereIrradianceFactor`).
fn sphere_irradiance(h : f32, phi : f32, horizon : f32) -> f32 {
  let eta = max(horizon, 0.0);
  if (eta == 0.0) {
    return max(0.0, howell_irradiance(h, phi));
  }
  // Past π the disc is wholly cut off, by either term, whatever η is.
  let tilted = clamp(phi + eta, 0.0, LIT_PI);
  let eta_sin_cos = preciseSinCos(min(eta, LIT_PI));
  let direct = eta_sin_cos.y * howell_irradiance(h, tilted);
  let tangential = eta_sin_cos.x * preciseSinCos(tilted).x * disc_visible_fraction(h, tilted);
  return max(0.0, direct + tangential);
}

// `body_brdf`'s disc term under an extended source (a star, or a neighbour's planetshine), at the
// share `l`, the law's L(α) from its row: μ₀ replaced by the source's irradiance factor h, which
// equals μ₀ wherever the whole source is up and lights the soft band past the terminator, and the
// Lommel–Seeliger term's μ₀ + μ floored at the source's angular radius, so that it stays bounded at
// the limb in that band. The TypeScript twin is `view/bodies/discShading.ts`' `surfaceReflectance`.
fn lit_disc_term(l : f32, h : f32, mu0 : f32, mu : f32, source_radius : f32) -> f32 {
  return l * 2.0 * h / max(max(mu0, 0.0) + mu, source_radius) + (1.0 - l) * h;
}

// Planetshine (Design note 7): the factor on a lit neighbour's illuminance face-on at the body's
// centre that reaches a lit point. The neighbour is a uniform sphere through `sphere_irradiance`,
// its face-on value carried from the centre to the point by the inverse square, and it is never
// shadow-tested against a third body. `to_source` runs from the point to the neighbour's centre;
// it, `source_radius` and `centre_distance` share one unit of length; `normal` is the point's unit
// normal. The angle from the normal is `preciseAtan2` of the cross and dot products, never WGSL's
// `acos` (R07.T6.d). The TypeScript twin is `view/lighting/planetshine.ts`' `planetshineIrradiance`.
fn planetshine_irradiance(
  to_source : vec3f,
  source_radius : f32,
  centre_distance : f32,
  normal : vec3f,
  horizon : f32,
) -> f32 {
  let distance = length(to_source);
  let phi = preciseAtan2(length(cross(normal, to_source)), dot(normal, to_source));
  let near = centre_distance / distance;
  return sphere_irradiance(distance / source_radius, phi, horizon) * near * near;
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

// A star disc's annuli for one channel (`annuli.ts`'s `AnnulusSet`, placed on the CPU per star
// and channel by equal concentric error, decision-r07-dn6): `outer[j]` the outer radius of annulus
// j in stellar radii (the first's inner radius is 0, the last's outer 1), `flux[j]` its exact share
// of the disc's flux, `count` the number in use, at most four.
struct DiscAnnuli {
  outer : vec4f,
  flux : vec4f,
  count : u32,
}

// The eclipse term (Design note 6): the fraction of the disc's flux left visible by one occluder,
// the two given as angular radii and the angle between their centres; each uniform annulus is
// eclipsed by the difference of two exact overlaps. The TypeScript twin is `view/lighting/
// annuli.ts`'s `annulusVisibleFraction`.
fn eclipse_visible(
  star_radius : f32,
  annuli : DiscAnnuli,
  occluder_radius : f32,
  separation : f32,
) -> f32 {
  let ratio = occluder_radius / star_radius;
  let z = separation / star_radius;
  if (z >= 1.0 + ratio) {
    return 1.0;
  }
  var hidden = 0.0;
  var inner_overlap = 0.0;
  var inner_edge = 0.0;
  for (var j = 0u; j < min(annuli.count, 4u); j = j + 1u) {
    let outer_edge = annuli.outer[j];
    let outer_overlap = circle_overlap_area(outer_edge, ratio, z);
    let area = LIT_PI * (outer_edge * outer_edge - inner_edge * inner_edge);
    hidden = hidden + annuli.flux[j] * (outer_overlap - inner_overlap) / area;
    inner_overlap = outer_overlap;
    inner_edge = outer_edge;
  }
  return max(0.0, 1.0 - hidden);
}

// A lit point's frame for R08's lookups (plan R08, R08.T9.b; Design note 17): its geodetic latitude,
// rad, from its spheroid normal against the pole, and its north and east, unit and horizontal, in
// the axes of the normal and the pole (the view's on R07's disc, body-fixed on R10's terrain). At
// a pole, where every azimuth is the same, east is any horizontal direction. Shared by
// `bodyDisc.wgsl` and R10's `terrainLit.wgsl`, it calls `preciseTrig.wgsl`'s `preciseAtan2`, which
// every includer composes with R08's surface lighting.
struct LocalFrame {
  latitude : f32,
  north : vec3f,
  east : vec3f,
}

// Below this cosine of its latitude a point is taken as at a pole.
const AT_POLE : f32 = 1e-6;

// The frame at the point of unit spheroid normal `normal` on a body of unit pole `pole`: sin φ is
// n · p and cos φ |p × n|, φ by `preciseAtan2` (R08's `preciseTrig.wgsl`), east p × n ÷ cos φ and
// north n × east.
fn local_frame(normal : vec3f, pole : vec3f) -> LocalFrame {
  let sin_latitude = clamp(dot(normal, pole), -1.0, 1.0);
  let towards_east = cross(pole, normal);
  let cos_latitude = length(towards_east);
  var east : vec3f;
  if (cos_latitude > AT_POLE) {
    east = towards_east / cos_latitude;
  } else {
    let other = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(normal.x) > 0.9);
    east = normalize(cross(other, normal));
  }
  let latitude = preciseAtan2(abs(sin_latitude), cos_latitude);
  return LocalFrame(select(latitude, -latitude, sin_latitude < 0.0), cross(normal, east), east);
}

// A unit direction's azimuth at the frame, rad, from north towards east, in [−π, π]; 0 at the
// zenith or the nadir.
fn local_azimuth(frame : LocalFrame, d : vec3f) -> f32 {
  let towards_north = dot(d, frame.north);
  let towards_east = dot(d, frame.east);
  if (!(towards_north * towards_north + towards_east * towards_east > 0.0)) {
    return 0.0;
  }
  let azimuth = preciseAtan2(abs(towards_east), towards_north);
  return select(azimuth, -azimuth, towards_east < 0.0);
}

// R11's hook (its Design note 14): the ring's shadow on the body, 1 until `ringShadow.wgsl`
// replaces this stub, and not called where R11's `ringShadowOnBody` setting is off.
fn ring_shadow_on_body(p : vec3f, sun : vec3f) -> vec3f {
  return vec3f(1.0);
}

// R08's hooks, each source's transmittance through the atmosphere and the sky's irradiance, were
// stubs of 1 and 0 here until plan R08's R08.T9.b: `atmosphere_sun_transmittance`,
// `atmosphere_sky_irradiance` and `atmosphere_source_transmittance` are now
// `view/atmosphere/shaders/surfaceLighting.wgsl`'s, which every module that includes this one
// joins (WGSL resolves every name in a module), with the tables it reads; a body without an
// atmosphere binds one-texel defaults, and reads 1 and 0 as the stubs did.

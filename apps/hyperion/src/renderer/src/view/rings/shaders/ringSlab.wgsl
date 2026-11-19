// A ring as an optical-depth slab (plan R11, R11.T9.c; Design notes 13 to 15 and 18): the annulus
// in the ring plane, each fragment the view ray's crossing of the plane, lit on whichever face the
// star lights by the classical single-scattering layer times the shadowing table's f_e plus its
// multiple scattering, under the planet's shadow (R07's eclipse term with the planet the occluder),
// and seen through by exp(−τ ÷ μ). The TypeScript twin is `view/rings/ringSlab.ts`'
// `ringSlabFragment`; the photometry's `f64` forms are `ringPhotometry.ts`'.
//
// Composed after frame.wgsl, preciseTrig.wgsl, whose `preciseAtan2` it calls, and the slab's
// constants (`ringSlab.ts`' `RING_SLAB_CONSTANTS_WGSL`). Two materials draw it,
// `ringSlab.ts`' `RING_SLAB_MATERIALS`: `attached`, into a target that holds the scene's depth,
// which its fragments test and do not write; `sampled`, into one of the air targets, which hold no
// depth, its fragments reading the scene's depth themselves (`scene_depth_nearer`). No depth bias:
// the plane is drawn at its own depth, the triangles lying in it.
//
// Every length reaches the GPU already divided by the planet's equatorial radius a and relative to
// its centre (R07 Design note 19), but the vertices', which are metres from the camera so that the
// depth they write is the scene's. RING_ROWS rows per ring (`ringSlab.ts`' `packRingSlabRecords`),
// along the galactic axes:
//   0  the planet's centre from the camera ÷ a; w, the ring plane's offset n · centre ÷ a, the
//      camera's height below the plane, formed in f64 on the CPU;
//   1  the plane's unit normal n; w, the inner edge's radius ÷ a;
//   2  the planet's pole; w, c ÷ a;
//   3  the outer edge's radius ÷ a; the lights; the occluders; exposure ÷ π;
//   4  the ring's kind, 0 massive and 1 dusty; the annuli per channel; the scale of the profile's
//      optical depths, its densest band's τ; the pixel's angle its particles are instanced at (rad,
//      R11.T9.d), 0 where none is;
//   5 + 8j, light j: the star's unit direction from the planet's centre, w its distance ÷ a; its
//      illuminance face-on per channel (lx), w its radius ÷ a; its annuli for r, g and b, two rows
//      each, as a disc record's (`bodyDisc.wgsl`);
//   21 + k, occluder k: a disc body's centre from the planet's centre ÷ a; w, its radius ÷ a.

struct Draw {
  // The planet's centre from the camera, m, about which the vertices are placed.
  offsetFromCameraM : vec3f,
  // The ring's record in `rings`.
  ring : u32,
  // The fragments the draw keeps (`ringSlab.ts`' `RingPart`): 0 every one, 1 those beyond the near
  // side of the planet's air shell along the view ray, 2 the others.
  part : u32,
  // The air shell's radius ÷ a, read where `part` is not 0.
  shellOverA : f32,
  // a, m.
  radiusM : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@group(2) @binding(0) var<storage, read> rings : array<vec4f>;

// The ring's radial profile (`ringSlab.ts`' `ringProfileTexels`): RING_PROFILE_TEXELS texels across
// the annulus, inner edge to outer, and 3 × RING_PROFILE_LEVELS rows in three blocks, a row a
// level, each texel of level l the bands of the 2^l texels about it as two components: block 0 the
// first's ϖ₀ per channel and τ ÷ the record's scale (row 4's z), block 1 the second's, block 2 the
// second's share and the two surge half-widths, rad. A row a level, so that one bilinear fetch a
// block both filters along the radius and blends the pixel's footprint between two levels, as a
// mip chain's trilinear fetch would.
@group(2) @binding(1) var ring_profile : texture_2d<f32>;

// The shadowing table (`shadowingLookup.ts`): v along x, μ_eff along y, τ_phot along z, the lit
// face's (f_e, I₂, I₃) layers then the unlit face's (I₂, I₃); and the orders' ratios, μ_eff along x
// and τ_phot along y, (lit q_e, lit q_o, unlit q_e, unlit q_o).
@group(2) @binding(2) var ring_shadowing : texture_3d<f32>;
@group(2) @binding(3) var ring_ratios : texture_2d<f32>;
@group(2) @binding(4) var ring_sampler : sampler;

const RING_PI : f32 = 3.14159265358979;

fn ring_row(i : u32) -> vec4f {
  return rings[draw.ring * RING_ROWS + i];
}

// A direction along the galactic axes in the view's.
fn ring_to_view(v : vec3f) -> vec3f {
  return (frame.viewRotation * vec4f(v, 0.0)).xyz;
}

// The ray through a point of the output, px, in view space (looking down −z), unit: the
// projection's third column holds a still's tile's off-axis terms (plan R11, R11.T10.e.2), so that
// a tile takes the still's ray; 0 for every view's frame, whose ray is as it was.
fn ring_view_ray(px : vec2f) -> vec3f {
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  return normalize(vec3f(
    (ndc.x + frame.clipProjection[2][0]) / frame.clipProjection[0][0],
    (ndc.y + frame.clipProjection[2][1]) / frame.clipProjection[1][1],
    -1.0,
  ));
}

// Two unit vectors that span the plane of unit `n` (`ringSlab.ts`' `planeAxes`).
fn ring_plane_axes(n : vec3f) -> mat2x3f {
  let a = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(n.x) > 0.9);
  let e1 = normalize(cross(n, a));
  return mat2x3f(e1, cross(n, e1));
}

struct RingVarying {
  @builtin(position) position : vec4f,
}

// One corner of the annulus's mesh (`ringSlab.ts`' `ringSlabMesh`): x and y its direction in the
// plane's axes, the outer polygon's already out by 1 ÷ cos(π ÷ N), so that it holds the outer edge;
// z 0 on the inner polygon, which the inner edge holds, and 1 on the outer. Each polygon stands off
// its edge by RING_MESH_MARGIN of its radius or two pixels at the corner's distance, the larger, so
// that the pixels just outside an edge are drawn and its coverage falls off over a pixel at any
// size, the far annulus's included.
@vertex
fn vertexMain(@location(0) corner : vec3f) -> RingVarying {
  let head = ring_row(1u);
  let axes = ring_plane_axes(head.xyz);
  let outer = corner.z > 0.5;
  let radius = select(head.w, ring_row(3u).x, outer) * draw.radiusM;
  let direction = corner.x * axes[0] + corner.y * axes[1];
  let pixel = 2.0 / (frame.clipProjection[0][0] * frame.viewport.x);
  let reach = max(radius * RING_MESH_MARGIN, 2.0 * pixel * length(draw.offsetFromCameraM + direction * radius));
  let grown = select(max(radius - reach, 0.0), radius + reach, outer);
  var out : RingVarying;
  out.position = frame.clipProjection * (frame.viewRotation * vec4f(draw.offsetFromCameraM + direction * grown, 1.0));
  return out;
}

// The nearer root of the unit ray against a sphere of radius `radius` about `centre`, both from the
// camera, or −1 where it misses: the closest approach by the cross product, which keeps a far
// sphere's grazing rays where |c|² − (c · d)² would cancel.
struct SphereHits {
  near : f32,
  far : f32,
  hit : bool,
}

fn ring_sphere_hits(ray : vec3f, centre : vec3f, radius : f32) -> SphereHits {
  let along = dot(centre, ray);
  let across = cross(centre, ray);
  let h2 = radius * radius - dot(across, across);
  var out : SphereHits;
  out.hit = h2 >= 0.0;
  let half = sqrt(max(h2, 0.0));
  out.near = along - half;
  out.far = along + half;
  return out;
}

// p stretched along the pole by a ÷ c about the planet's centre, which makes its spheroid the
// sphere of radius 1.
fn ring_stretched(p : vec3f, pole : vec3f, stretch : f32) -> vec3f {
  return p + (stretch - 1.0) * dot(p, pole) * pole;
}

// The share of the optical depth instanced where the pixel's footprint is `footprint` m (R11.T9.d;
// `ringSplit.ts`' `instancedShare`): ln(s_max ÷ s_pix) ÷ ln(s_max ÷ s_min), s_pix half the
// footprint, within [0, 1]; 0 where the footprint is not positive, no particle being instanced.
fn ring_instanced_share(footprint : f32) -> f32 {
  if (!(footprint > 0.0)) {
    return 0.0;
  }
  let share = log(RING_PARTICLE_RADIUS_MAX / (0.5 * footprint)) / log(RING_PARTICLE_RADIUS_MAX / RING_PARTICLE_RADIUS_MIN);
  return clamp(share, 0.0, 1.0);
}

// 1 − e^(−x), its series where x is small, so that a thin layer keeps its digits.
fn ring_one_minus_exp(x : f32) -> f32 {
  if (x < 1e-3) {
    return x * (1.0 - x * (0.5 - x / 6.0));
  }
  return 1.0 - exp(-x);
}

// The particles' phase at phase angle `alpha`, normalised over 4π: Salo and French's power law
// c_n (π − α)^n for a massive ring, the two-term Henyey–Greenstein for a dusty one
// (`ringPhotometry.ts`' `powerLawPhase`, `dustyPhase`).
fn ring_hg(g : f32, cos_theta : f32) -> f32 {
  let g2 = g * g;
  let d = 1.0 + g2 - 2.0 * g * cos_theta;
  return (1.0 - g2) / (d * sqrt(d));
}

fn ring_phase(alpha : f32, dusty : bool) -> f32 {
  if (dusty) {
    let cos_theta = -cos(alpha);
    return RING_DUSTY_FORWARD_WEIGHT * ring_hg(RING_DUSTY_FORWARD_G, cos_theta)
      + (1.0 - RING_DUSTY_FORWARD_WEIGHT) * ring_hg(-RING_DUSTY_BACKWARD_G, cos_theta);
  }
  // Held off 0, where pow's exp2(n log2 0) has no defined value.
  return RING_POWER_LAW_NORMALISATION * pow(max(RING_PI - alpha, 1e-6), RING_POWER_LAW_EXPONENT);
}

// The particles' own opposition surge (`ringSlab.ts`' `surgeAt`): 1 + (A(τ) − 1) e^(−α ln 2 ÷ w),
// A(τ) = 1 + (A_sparse − 1) ÷ (1 + τ), w the half-width, rad; 1 for a dusty ring.
fn ring_surge(alpha : f32, tau : f32, half_width : f32, dusty : bool) -> f32 {
  if (dusty || !(half_width > 0.0)) {
    return 1.0;
  }
  let amplitude = (RING_SPARSE_SURGE_AMPLITUDE - 1.0) / (1.0 + max(tau, 0.0));
  return 1.0 + amplitude * exp(-max(alpha, 0.0) * 0.693147180559945 / half_width);
}

// The shadowing table's fractional node indices (`shadowingLookup.ts`' `tauIndex`, `muIndex`,
// `phaseIndex`), each clamped to its axis.
fn ring_tau_index(tau : f32) -> f32 {
  return min(f32(RING_TAU_NODES - 1u) * log(1.0 + max(tau, 0.0) / RING_TAU_SCALE) / log(1.0 + RING_TAU_MAX / RING_TAU_SCALE), f32(RING_TAU_NODES - 1u));
}

fn ring_mu_index(mu_eff : f32) -> f32 {
  return f32(RING_MU_NODES - 1u) * sqrt(clamp((mu_eff - RING_MU_MIN) / (1.0 - RING_MU_MIN), 0.0, 1.0));
}

fn ring_phase_index(v : f32) -> f32 {
  return f32(RING_PHASE_NODES - 1u) * log(1.0 + clamp(v, 0.0, 1.0) / RING_PHASE_SCALE) / log(1.0 + 1.0 / RING_PHASE_SCALE);
}

// The quantities at (τ_phot, μ_eff, v) on face `face` (0 lit, 1 unlit): one trilinear fetch, each
// index at its node's texel centre, the τ index within the face's layers.
fn shadowing_at(face : u32, tau : f32, mu_eff : f32, v : f32) -> vec4f {
  let coord = vec3f(
    (ring_phase_index(v) + 0.5) / f32(RING_PHASE_NODES),
    (ring_mu_index(mu_eff) + 0.5) / f32(RING_MU_NODES),
    (ring_tau_index(tau) + 0.5 + f32(face * RING_TAU_NODES)) / f32(2u * RING_TAU_NODES),
  );
  return textureSampleLevel(ring_shadowing, ring_sampler, coord, 0.0);
}

fn ratios_at(tau : f32, mu_eff : f32) -> vec4f {
  let coord = vec2f(
    (ring_mu_index(mu_eff) + 0.5) / f32(RING_MU_NODES),
    (ring_tau_index(tau) + 0.5) / f32(RING_TAU_NODES),
  );
  return textureSampleLevel(ring_ratios, ring_sampler, coord, 0.0);
}

// A star disc's annuli for one channel, as litBody.wgsl's `DiscAnnuli` holds them (`annuli.ts`'
// `AnnulusSet`): each annulus's outer radius in stellar radii and its exact share of the flux.
struct RingAnnuli {
  outer : vec4f,
  flux : vec4f,
  count : u32,
}

// One light's annuli for channel c, as `bodyDisc.wgsl`'s `light_annuli` reads a disc record's.
fn ring_light_annuli(j : u32, c : u32, count : u32) -> RingAnnuli {
  let first = RING_FIRST_LIGHT_ROW + j * RING_LIGHT_ROWS + 2u + 2u * c;
  var annuli : RingAnnuli;
  annuli.outer = ring_row(first);
  annuli.flux = ring_row(first + 1u);
  annuli.count = count;
  return annuli;
}

// The area of a circle of radius `radius` beyond a chord `depth` in from its edge: ρ² (θ − sin θ
// cos θ), θ the chord's half-angle from the centre, formed from the depth so that a large circle's
// thin segment keeps its digits, θ − sin θ cos θ by its series, (2/3) θ³ − (2/15) θ⁵, where θ is
// small.
fn ring_segment(radius : f32, depth : f32) -> f32 {
  let h = clamp(depth, 0.0, 2.0 * radius);
  let s = sqrt(h * (2.0 * radius - h));
  let c = radius - h;
  let theta = preciseAtan2(s, c);
  if (theta < 1e-2) {
    let t2 = theta * theta;
    return radius * radius * theta * t2 * (2.0 / 3.0 - t2 * (2.0 / 15.0));
  }
  return radius * radius * theta - s * c;
}

// The exact overlap of circles of radii r and k whose centres are z apart, `gap` = z − k given
// apart: R07's `circle_overlap_area` (litBody.wgsl) in a form that stays well-conditioned for an
// occluder far larger than the star, as the planet is seen from its ring (k ÷ r up to some 10³),
// where z² − k² and acos near 1 cancel every digit of f32 and the penumbra's light goes astray.
fn ring_overlap_area(r : f32, k : f32, z : f32, gap : f32) -> f32 {
  if (r <= 0.0 || k <= 0.0 || gap >= r) {
    return 0.0;
  }
  if (z <= abs(r - k)) {
    let small = min(r, k);
    return RING_PI * small * small;
  }
  // The chord's distance from the small circle's centre towards the other, (z² + r² − k²) ÷ 2z.
  let d1 = (gap * (z + k) + r * r) / (2.0 * z);
  return ring_segment(r, r - d1) + ring_segment(k, d1 - gap);
}

// The eclipse term (R07 Design note 6; litBody.wgsl's `eclipse_visible`) by `ring_overlap_area`:
// the share of the disc's flux the occluder leaves visible, the two given as angular radii and
// the angle between their centres, `gap` that angle less the occluder's radius.
fn ring_eclipse_visible(star_radius : f32, annuli : RingAnnuli, occluder_radius : f32, separation : f32, gap : f32) -> f32 {
  let k = occluder_radius / star_radius;
  let z = separation / star_radius;
  let g = gap / star_radius;
  if (g >= 1.0) {
    return 1.0;
  }
  var hidden = 0.0;
  var inner_overlap = 0.0;
  var inner_edge = 0.0;
  for (var j = 0u; j < min(annuli.count, 4u); j = j + 1u) {
    let outer_edge = annuli.outer[j];
    let outer_overlap = ring_overlap_area(outer_edge, k, z, g);
    let area = RING_PI * (outer_edge * outer_edge - inner_edge * inner_edge);
    hidden = hidden + annuli.flux[j] * (outer_overlap - inner_overlap) / area;
    inner_overlap = outer_overlap;
    inner_edge = outer_edge;
  }
  return clamp(1.0 - hidden, 0.0, 1.0);
}

// The share of light j's flux per channel the planet leaves visible at the ring point `x` (from the
// planet's centre ÷ a, view axes), `star` the star's place likewise (`ringShadows.ts`'
// `planetShadowOnRing`): in the frame stretched by a ÷ c along the pole, where the planet is the
// unit sphere, its angular radius and its separation from the star's centre as seen from the point,
// each by `preciseAtan2`, whose bound is f32's where WGSL's atan2 allows 4,096 ULP, a third of the
// Sun's radius at 10 au against the planet's half a radian.
fn ring_planet_visible(x : vec3f, star : vec3f, star_radius_over_a : f32, j : u32, annuli : u32, pole : vec3f, stretch : f32) -> vec3f {
  let p = ring_stretched(x, pole, stretch);
  let to_star = ring_stretched(star, pole, stretch) - p;
  let star_distance = length(to_star);
  let planet_distance = length(p);
  if (planet_distance <= 1.0) {
    return vec3f(0.0);
  }
  if (planet_distance >= star_distance) {
    return vec3f(1.0);
  }
  let star_radius = asin(min(star_radius_over_a / star_distance, 1.0));
  let planet_radius = preciseAtan2(1.0, sqrt((planet_distance - 1.0) * (planet_distance + 1.0)));
  let along = -p / planet_distance;
  let towards = to_star / star_distance;
  let separation = preciseAtan2(length(cross(along, towards)), dot(along, towards));
  let gap = separation - planet_radius;
  if (gap >= star_radius) {
    return vec3f(1.0);
  }
  var visible = vec3f(1.0);
  for (var c = 0u; c < 3u; c = c + 1u) {
    visible[c] = ring_eclipse_visible(star_radius, ring_light_annuli(j, c, annuli), planet_radius, separation, gap);
  }
  return visible;
}

// The light one fragment of the ring sends towards the camera, pre-exposed, and the share of the
// light behind it it lets through.
struct RingShaded {
  radiance : vec3f,
  transmittance : f32,
}

// One component of the footprint's profile (`ringSlab.ts`' `ProfileComponent`): its particles'
// ϖ₀ per channel, its τ and its surge's half-width, rad.
struct RingComponent {
  albedo : vec3f,
  tau : f32,
  half_width : f32,
}

// One component's I/F at a light's geometry: the lit face I/F = ϖ₀ P μ₀ ÷ (4(μ + μ₀)) [1 −
// e^(−τ(1/μ + 1/μ₀))] f_e + ϖ₀² I₂ ÷ (1 − ϖ₀² q_e) + ϖ₀³ I₃ ÷ (1 − ϖ₀² q_o), the unlit face the
// transmitted beam single-scattered plus its own orders (Design notes 14 and 15;
// `shadowingTable.ts`' **What it holds**), P the particles' phase `particles` times their own
// surge.
fn ring_component_if(part : RingComponent, lit : bool, mu : f32, mu0 : f32, alpha : f32, particles : f32, dusty : bool) -> vec3f {
  let tau = max(part.tau, 0.0);
  let albedo = part.albedo;
  let w2 = albedo * albedo;
  let phase = particles * ring_surge(alpha, tau, part.half_width, dusty);
  let mu_eff = 2.0 * mu * mu0 / (mu + mu0);
  // The phase's range at B = B′ = asin μ_eff, which the table's v spans; never 0, at μ_eff = 1.
  let range = max(RING_PI - 2.0 * asin(min(mu_eff, 1.0)), 1e-6);
  let q = ratios_at(tau, mu_eff);
  if (lit) {
    let table = shadowing_at(0u, tau, mu_eff, alpha / range);
    let geometry = mu0 / (mu + mu0);
    let single = albedo * (phase * geometry / 4.0 * ring_one_minus_exp(tau * (1.0 / mu + 1.0 / mu0)) * table.x);
    let multiple = w2 * (table.y * geometry) / (1.0 - w2 * q.x)
      + w2 * albedo * (table.z * geometry) / (1.0 - w2 * q.y);
    return single + multiple;
  }
  let table = shadowing_at(1u, tau, mu_eff, (RING_PI - alpha) / range);
  var single = 0.0;
  if (abs(mu - mu0) > RING_UNLIT_NEAR * max(mu, mu0)) {
    single = phase * mu0 / (4.0 * (mu - mu0)) * (exp(-tau / mu) - exp(-tau / mu0));
  } else {
    // ϖ₀ P τ e^(−τ/μ₀) E(z) ÷ 4μ, E(z) = expm1(z) ÷ z at z = τ (μ − μ₀) ÷ (μ μ₀), small here.
    let z = tau * (mu - mu0) / (mu * mu0);
    single = phase * tau * exp(-tau / mu0) / (4.0 * mu) * (1.0 + z / 2.0 + z * z / 6.0);
  }
  let multiple = w2 * (table.x * mu0) / (1.0 - w2 * q.z)
    + w2 * albedo * (table.y * mu0) / (1.0 - w2 * q.w);
  return albedo * single + multiple;
}

// The slab at the ring point `x` (from the planet's centre ÷ a, view axes), seen along `ray`, its
// footprint two components, the second of share `share`: each light's I/F per component times its
// face-on illuminance and the planet's shadow, and the light each lets through, mixed by length,
// as the pixel's area average is (`ringSlab.ts`' `ringProfileTexels`).
fn ring_shade(x : vec3f, ray : vec3f, n : vec3f, first : RingComponent, second : RingComponent, share : f32) -> RingShaded {
  let pole = normalize(ring_to_view(ring_row(2u).xyz));
  let stretch = 1.0 / ring_row(2u).w;
  let counts = ring_row(3u);
  let kind = ring_row(4u);
  let dusty = kind.x > 0.5;
  let annuli = u32(kind.y);
  let den = dot(n, ray);
  let mu = abs(den);
  var out : RingShaded;
  out.radiance = vec3f(0.0);
  out.transmittance = (1.0 - share) * exp(-max(first.tau, 0.0) / mu) + share * exp(-max(second.tau, 0.0) / mu);
  for (var j = 0u; j < min(u32(counts.y), RING_MAX_LIGHTS); j = j + 1u) {
    let place = ring_row(RING_FIRST_LIGHT_ROW + j * RING_LIGHT_ROWS);
    let light = ring_row(RING_FIRST_LIGHT_ROW + j * RING_LIGHT_ROWS + 1u);
    let star = ring_to_view(place.xyz) * place.w;
    let to_star = star - x;
    let towards = normalize(to_star);
    let s_n = dot(towards, n);
    let mu0 = abs(s_n);
    if (!(mu0 > 0.0)) {
      continue;
    }
    let visible = ring_planet_visible(x, star, light.w, j, annuli, pole, stretch);
    if (!any(visible > vec3f(0.0))) {
      continue;
    }
    // The phase angle between the star and the camera as seen from the point, by `preciseAtan2`,
    // which keeps the surge's tenths of a degree where acos near 1 would not.
    let alpha = preciseAtan2(length(cross(towards, -ray)), dot(towards, -ray));
    let particles = ring_phase(alpha, dusty);
    // The star and the camera on one side of the plane: the lit face.
    let lit = s_n * den < 0.0;
    var reflectance = (1.0 - share) * ring_component_if(first, lit, mu, mu0, alpha, particles, dusty);
    if (share > 0.0) {
      reflectance = reflectance + share * ring_component_if(second, lit, mu, mu0, alpha, particles, dusty);
    }
    out.radiance = out.radiance + reflectance * light.xyz * visible * counts.w;
  }
  return out;
}

@fragment
fn fragmentMain(v : RingVarying) -> @location(0) vec4f {
  let ray = ring_view_ray(v.position.xy);
  let centre_row = ring_row(0u);
  let head = ring_row(1u);
  let centre = ring_to_view(centre_row.xyz);
  let n = ring_to_view(head.xyz);
  let den = dot(n, ray);
  // The crossing of the plane n · p = h along the ray, t its distance ÷ a, and its point from the
  // planet's centre: t held finite, so that a ray along the plane, whose point lies off the annulus,
  // leaves its quad's derivatives finite too.
  let along_plane = select(den, RING_PARALLEL_COSINE, abs(den) < RING_PARALLEL_COSINE);
  let t = clamp(centre_row.w / along_plane, -RING_FARTHEST, RING_FARTHEST);
  let x = t * ray - centre;
  let r = length(x);
  // The pixel's footprint across the radius ÷ a, formed here, in uniform control flow, for the
  // profile's level and the edges' coverage.
  let footprint = max(abs(dpdx(r)), abs(dpdy(r)));
  let inner = head.w;
  let outer = ring_row(3u).x;
  let span = outer - inner;
  let level = clamp(log2(max(footprint / span * f32(RING_PROFILE_TEXELS), 1.0)), 0.0, f32(RING_PROFILE_LEVELS - 1u));
  let u = clamp((r - inner) / span, 0.0, 1.0);
  let rows = f32(3u * RING_PROFILE_LEVELS);
  let levels = f32(RING_PROFILE_LEVELS);
  let first_texel = textureSampleLevel(ring_profile, ring_sampler, vec2f(u, (level + 0.5) / rows), 0.0);
  let second_texel = textureSampleLevel(ring_profile, ring_sampler, vec2f(u, (levels + level + 0.5) / rows), 0.0);
  let mix_texel = textureSampleLevel(ring_profile, ring_sampler, vec2f(u, (2.0 * levels + level + 0.5) / rows), 0.0);
  // The share of the optical depth the ring's particles close to take from the slab (R11.T9.d;
  // `ringSplit.ts`' `instancedShare`), at the crossing's own footprint; none where row 4's w is 0.
  let kept = 1.0 - ring_instanced_share(t * draw.radiusM * ring_row(4u).w);
  let tau_scale = ring_row(4u).z * kept;
  let first = RingComponent(first_texel.rgb, first_texel.a * tau_scale, mix_texel.g);
  let second = RingComponent(second_texel.rgb, second_texel.a * tau_scale, mix_texel.b);
  // The edges' coverage of the pixel, so that the inner and outer edges fall where R02's ellipses
  // do, at coverage one half.
  let edge = max(footprint, 1e-30);
  let coverage = clamp((r - inner) / edge + 0.5, 0.0, 1.0) * clamp((outer - r) / edge + 0.5, 0.0, 1.0);
  var keep = t > 0.0 && coverage > 0.0 && !scene_depth_nearer(v.position);
  // Behind the planet: the ray meets its spheroid nearer than the plane (stretched, the unit sphere).
  let pole = normalize(ring_to_view(ring_row(2u).xyz));
  let stretch = 1.0 / ring_row(2u).w;
  let scaled_ray = ring_stretched(ray, pole, stretch);
  let scale = length(scaled_ray);
  let planet = ring_sphere_hits(scaled_ray / scale, ring_stretched(centre, pole, stretch), 1.0);
  keep = keep && !(planet.hit && planet.far > 0.0 && planet.near / scale < t);
  // Behind a disc body nearer than the plane, which writes no depth (R07 Design note 2).
  let counts = ring_row(3u);
  for (var k = 0u; k < min(u32(counts.z), RING_MAX_OCCLUDERS); k = k + 1u) {
    let occluder = ring_row(RING_FIRST_OCCLUDER_ROW + k);
    let hits = ring_sphere_hits(ray, centre + ring_to_view(occluder.xyz), occluder.w);
    keep = keep && !(hits.hit && hits.far > 0.0 && hits.near < t);
  }
  // The draw's part: beyond the near side of the planet's air shell, which its composite lays over
  // what lies behind it, or the rest, which is drawn over the composite.
  if (draw.part != 0u) {
    let shell = ring_sphere_hits(ray, centre, draw.shellOverA);
    let beyond = shell.hit && shell.far > 0.0 && t > max(shell.near, 0.0);
    keep = keep && (beyond == (draw.part == 1u));
  }
  var out = vec4f(0.0);
  // The light only where the fragment is kept, in a loop of at most one pass, not an `if`, as
  // `bodyDiscDraw.wgsl` forms its own: a discarded fragment runs on as a helper.
  for (var shading = keep; shading; shading = false) {
    let shaded = ring_shade(x, ray, n, first, second, clamp(mix_texel.r, 0.0, 1.0));
    out = coverage * vec4f(min(shaded.radiance, vec3f(RING_HDR_MAX)), 1.0 - shaded.transmittance);
  }
  if (!keep) {
    discard;
  }
  return out;
}

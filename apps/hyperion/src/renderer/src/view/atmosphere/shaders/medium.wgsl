// The atmosphere's medium, read in place (plan R05, Design note 16; plan R08, R08.T6.b): each term's
// density at a height, the medium at a height, and the shell's forms (`common.wgsl`) on the
// medium's own shell. Prepended, after `common.wgsl`, to every kernel, each of which declares the
// medium's shell, its terms and their density tables as `var<uniform> medium : Medium;`,
// `var<storage, read> terms : array<Term, MAX_TERMS>;` and `var densityTables :
// texture_2d_array<f32>;` in group 0. The composite declares none and does not take this.
//
// The helpers read `medium` and `terms` where they are rather than taking them by value. A 416 B
// `Medium` passed by value and indexed by a loop variable (`terms[i]`) is, most likely, copied into
// each invocation's own memory (Tint keeps a value indexed that way in a function variable), and
// the ray march did that three times a sample: 6 to 70 ms a frame on high (plan R05's Risks,
// "R05.T14, the high run's pass timer and atmosphere, diagnosed"). Every height is formed from a
// height, never as a radius less the ground's (plan R08, Design note 18, R08.T6.g), and R05's
// radius forms (`mediumAt(r)`, `distanceToTop`, `distanceToBottom`, `intersectsGround`,
// `maxDistance` and the tables' (r, mu) mappings) are gone.
//
// Ported from Bevy 0.19.1's functions.wgsl and bruneton_functions.wgsl (MIT) and Bruneton's
// precomputed_atmospheric_scattering (BSD-3-Clause), with Bruneton 2017's texel mapping on every
// table (plan R08, Design note 19); the multiple-scattering table's 10 m ground offset is a
// deliberate change from sebh's code (common.wgsl). Bevy's and Bruneton's notices and the changes
// are in common.wgsl, sebh's notice in multiScattering.wgsl.

// A tabulated profile's relative density at a height h >= 0 above the ground, from layer `layer`
// of `densityTables`, whose n levels stand at h_k = top (k / (n - 1))^2: linear in h between the
// two levels about h, and the last level's beyond the top (`tables.ts`'s `resampledDensity`,
// whose levels the CPU twin reads by `medium.ts`'s `tabulated` rule).
fn tabulatedDensityOf(layer : u32, topM : f32, h : f32) -> f32 {
  let n = textureDimensions(densityTables).x;
  let last = f32(n - 1u);
  let k = min(u32(sqrt(clamp(h / topM, 0.0, 1.0)) * last), n - 2u);
  let below = f32(k) / last;
  let above = f32(k + 1u) / last;
  let h0 = topM * below * below;
  let h1 = topM * above * above;
  let f = clamp((h - h0) / (h1 - h0), 0.0, 1.0);
  let d0 = textureLoad(densityTables, vec2u(k, 0u), layer, 0).r;
  let d1 = textureLoad(densityTables, vec2u(k + 1u, 0u), layer, 0).r;
  return mix(d0, d1, f);
}

// Term i's relative density at a height above the ground, its profile `profile` (Term.profile in
// common.wgsl); a negative height is the ground's.
fn densityOf(i : u32, profile : vec4f, heightM : f32) -> f32 {
  let h = max(heightM, 0.0);
  if (profile.x < 0.5) {
    return exp(-h / profile.y);
  }
  if (profile.x < 1.5) {
    let rising = (h - profile.y) / (profile.z - profile.y);
    let falling = (profile.w - h) / (profile.w - profile.z);
    return max(min(rising, falling), 0.0);
  }
  return tabulatedDensityOf(i, profile.y, h);
}

// The medium at a height above the ground.
fn mediumAtHeight(h : f32) -> MediumSample {
  var scattering = vec3f(0.0);
  var extinction = vec3f(0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    let d = densityOf(i, term.profile, h);
    scattering += term.scattering.rgb * d;
    extinction += (term.scattering.rgb + term.absorption.rgb) * d;
  }
  return MediumSample(scattering, extinction);
}

// r^2 - bottom^2 for a point `heightM` above the medium's ground (`shellAboveGround`).
fn aboveGroundOf(heightM : f32) -> f32 {
  return shellAboveGround(medium.bottomRadiusM, heightM);
}

// A ray's length inside the atmosphere from `heightM` above the ground at zenith cosine mu, to the
// ground or to the top (`shellMaxDistance`).
fn maxDistanceFromHeight(heightM : f32, mu : f32) -> f32 {
  return shellMaxDistance(medium.bottomRadiusM, medium.topHeightM, heightM, mu);
}

// The height at distance t along a ray from `heightM` at zenith cosine mu, its radius there `rT`
// (`shellHeightAlong`).
fn heightAlong(heightM : f32, mu : f32, t : f32, rT : f32) -> f32 {
  return shellHeightAlong(medium.bottomRadiusM, heightM, mu, t, rT);
}

// A transmittance texel's point and ray: its height, its zenith cosine, and rho = sqrt(r^2 -
// bottom^2), the distance to the horizon, from which its own heights are formed.
struct TransmittanceTexel {
  heightM : f32,
  mu : f32,
  rho : f32,
}

// Bruneton 2017's GetRMuFromTransmittanceTextureUv (precomputed_atmospheric_scattering,
// atmosphere/functions.glsl) after its texel mapping, from the unit range (x_mu, x_r) to the
// texel's point and ray, in heights (plan R08, Design note 18): rho = H_rho x_r, h = rho^2 /
// (sqrt(rho^2 + bottom^2) + bottom), and mu = (top^2 - r^2 - d^2) / (2 r d), with top^2 - r^2
// formed from h as `transmittanceUnitAt` forms it (`shellBelowTop`), so that the read at the
// texel's own (h, mu) returns the texel. From the nominal H_rho^2 (1 - x_r)(1 + x_r) it would not
// at the top row, where h stands f32's 8 mm below the top while that form gives 0, and the first
// texel's ray turned from the zenith to the horizontal, 0.036 of a texel off on a Saturn-radius
// sphere (R08.T6.g).
fn transmittanceTexelOf(unit : vec2f) -> TransmittanceTexel {
  let bottom = medium.bottomRadiusM;
  let topM = medium.topHeightM;
  let bigH = sqrt(topM * (2.0 * bottom + topM));
  let rho = bigH * unit.y;
  let heightM = rho * rho / (sqrt(rho * rho + bottom * bottom) + bottom);
  let dMin = topM - heightM;
  let dMax = rho + bigH;
  let d = dMin + unit.x * (dMax - dMin);
  let belowTop = shellBelowTop(bottom, topM, heightM);
  var mu = 1.0;
  if (d != 0.0) {
    mu = (belowTop - d * d) / (2.0 * (bottom + heightM) * d);
  }
  return TransmittanceTexel(heightM, clamp(mu, -1.0, 1.0), rho);
}

// The atmosphere's medium, read in place (plan R05, Design note 16; plan R08, R08.T6.b): each term's
// density at a height, the medium at a radius, and the shell's distances and the tables' (r, mu)
// mappings on the medium's own shell. Prepended, after `common.wgsl`, to every kernel, each of which
// declares the medium's shell, its terms and their density tables as
// `var<uniform> medium : Medium;`, `var<storage, read> terms : array<Term, MAX_TERMS>;` and
// `var densityTables : texture_2d_array<f32>;` in group 0. The composite declares none and does
// not take this.
//
// The helpers read `medium` and `terms` where they are rather than taking them by value. A 416 B
// `Medium` passed by value and indexed by a loop variable (`terms[i]`) is, most likely, copied into
// each invocation's own memory (Tint keeps a value indexed that way in a function variable), and
// the ray march did that three times a sample: 6 to 70 ms a frame on high (plan R05's Risks,
// "R05.T14, the high run's pass timer and atmosphere, diagnosed"). The arithmetic is common.wgsl's
// as it was, with the `tabulated` profile added (R08.T6.b).
//
// Ported from Bevy 0.19.1's functions.wgsl and bruneton_functions.wgsl (MIT) and Bruneton's
// precomputed_atmospheric_scattering (BSD-3-Clause), with sebh's sub-texel remap for the
// multiple-scattering table (MIT); its 10 m ground offset is a deliberate change from sebh's code
// (common.wgsl). Bevy's and Bruneton's notices and the changes are in common.wgsl, sebh's notice in
// multiScattering.wgsl.

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

// The medium at a radius from the centre.
fn mediumAt(r : f32) -> MediumSample {
  return mediumAtHeight(r - medium.bottomRadiusM);
}

// The medium at a height above the ground, for a caller that forms the height without the
// cancellation of r - bottom (`transmittance.wgsl`).
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

fn distanceToTop(r : f32, mu : f32) -> f32 {
  let discriminant = max(r * r * (mu * mu - 1.0) + medium.topRadiusM * medium.topRadiusM, 0.0);
  return max(-r * mu + sqrt(discriminant), 0.0);
}

fn distanceToBottom(r : f32, mu : f32) -> f32 {
  let discriminant = max(r * r * (mu * mu - 1.0) + medium.bottomRadiusM * medium.bottomRadiusM, 0.0);
  return max(-r * mu - sqrt(discriminant), 0.0);
}

fn intersectsGround(r : f32, mu : f32) -> bool {
  return shellIntersectsGround(medium.bottomRadiusM, r, mu);
}

// The ray's length inside the atmosphere, to the ground or to the top.
fn maxDistance(r : f32, mu : f32) -> f32 {
  if (intersectsGround(r, mu)) {
    return distanceToBottom(r, mu);
  }
  return distanceToTop(r, mu);
}

// r^2 - bottom^2 for a point `heightM` above the ground, h (2 bottom + h), which keeps its
// precision near the ground where the difference of squares loses the 0.5 m of an f32 radius near
// 6.4e6 m (plan R08, R08.T6.b).
fn aboveGroundOf(heightM : f32) -> f32 {
  return heightM * (2.0 * medium.bottomRadiusM + heightM);
}

// `maxDistance` from a ray's start `heightM` above the ground, r = bottom + heightM, without f32's
// cancellation (plan R08, R08.T6.b): the ground at aboveGround / (-r mu + sqrt((r mu)^2 -
// aboveGround)), the top at belowTop / (r mu + root) upward and -r mu + root otherwise, with
// root = sqrt((r mu)^2 + belowTop) and belowTop = top^2 - r^2 = (top - r)(top + r).
fn maxDistanceFromHeight(heightM : f32, mu : f32) -> f32 {
  let r = medium.bottomRadiusM + heightM;
  let rMu = r * mu;
  let aboveGround = aboveGroundOf(heightM);
  if (mu < 0.0 && rMu * rMu >= aboveGround) {
    return aboveGround / (-rMu + sqrt(rMu * rMu - aboveGround));
  }
  let belowTop = (medium.topRadiusM - medium.bottomRadiusM - heightM) * (medium.topRadiusM + r);
  let root = sqrt(max(rMu * rMu + belowTop, 0.0));
  return max(select(-rMu + root, belowTop / (rMu + root), mu > 0.0), 0.0);
}

// The height above the ground at distance t along a ray from `heightM` at zenith cosine mu, its
// radius there `rT`: (t^2 + 2 r mu t + aboveGround) / (rT + bottom), the difference rT - bottom
// without f32's cancellation (plan R08, R08.T6.b).
fn heightAlong(heightM : f32, mu : f32, t : f32, rT : f32) -> f32 {
  let rMu = (medium.bottomRadiusM + heightM) * mu;
  return (t * t + 2.0 * rMu * t + aboveGroundOf(heightM)) / (rT + medium.bottomRadiusM);
}

// Bruneton 2017's GetRMuFromTransmittanceTextureUv (precomputed_atmospheric_scattering,
// atmosphere/functions.glsl), without its sub-texel remap, as Bevy and sebh take it: (u, v) in
// [0, 1]^2 to (r, mu); u spans the distance to the top between its least (straight up) and its
// greatest (to the horizon), v the distance to the horizon, rho / H, which is Bruneton and Neyret
// 2008's (section 4).
fn transmittanceUvToRMu(uv : vec2f) -> vec2f {
  let bigH = sqrt(medium.topRadiusM * medium.topRadiusM - medium.bottomRadiusM * medium.bottomRadiusM);
  let rho = bigH * uv.y;
  let r = sqrt(rho * rho + medium.bottomRadiusM * medium.bottomRadiusM);
  let dMin = medium.topRadiusM - r;
  let dMax = rho + bigH;
  let d = dMin + uv.x * (dMax - dMin);
  var mu = 1.0;
  if (d != 0.0) {
    mu = (bigH * bigH - rho * rho - d * d) / (2.0 * r * d);
  }
  return vec2f(r, clamp(mu, -1.0, 1.0));
}

fn transmittanceRMuToUv(r : f32, mu : f32) -> vec2f {
  return shellRMuToUv(medium.bottomRadiusM, medium.topRadiusM, r, mu);
}

// The multiple-scattering table's (u, v) to (r, mu_sun): u the sun's zenith cosine, v the height.
fn multiScatteringUvToRMu(uv : vec2f, size : vec2f) -> vec2f {
  let unit = clamp(subUvsToUnit(uv, size), vec2f(0.0), vec2f(1.0));
  let r = medium.bottomRadiusM + GROUND_OFFSET_M
    + unit.y * (medium.topRadiusM - medium.bottomRadiusM - GROUND_OFFSET_M);
  return vec2f(r, unit.x * 2.0 - 1.0);
}

fn multiScatteringRMuToUv(r : f32, muSun : f32, size : vec2f) -> vec2f {
  return shellMultiScatteringRMuToUv(medium.bottomRadiusM, medium.topRadiusM, r, muSun, size);
}

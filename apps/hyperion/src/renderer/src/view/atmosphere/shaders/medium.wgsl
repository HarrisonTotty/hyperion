// The atmosphere's medium, read in place (plan R05, Design note 16): the medium at a radius, and the
// shell's distances and the tables' (r, mu) mappings on the medium's own shell. Prepended, after
// `common.wgsl`, to every kernel, each of which declares the medium as
// `@group(0) @binding(0) var<uniform> medium : Medium;`. The composite declares none and does not
// take this.
//
// The helpers read `medium` where it is rather than taking a `Medium` by value. A 416 B `Medium`
// passed by value and indexed by a loop variable (`terms[i]`) is, most likely, copied into each
// invocation's own memory (Tint keeps a value indexed that way in a function variable), and the
// ray march did that three times a sample: 6 to 70 ms a frame on high (plan R05's Risks, "R05.T14,
// the high run's pass timer and atmosphere, diagnosed"). The arithmetic is common.wgsl's as it was.
//
// Ported from Bevy 0.19.1's functions.wgsl and bruneton_functions.wgsl (MIT) and Bruneton's
// precomputed_atmospheric_scattering (BSD-3-Clause), with sebh's multiple-scattering mapping (MIT).
// Bevy's and Bruneton's notices and the changes are in common.wgsl, sebh's notice in
// multiScattering.wgsl.

fn mediumAt(r : f32) -> MediumSample {
  let h = r - medium.bottomRadiusM;
  var scattering = vec3f(0.0);
  var extinction = vec3f(0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = medium.terms[i];
    let d = densityOf(term.profile, h);
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

// Bruneton and Neyret 2008, section 4: (u, v) in [0, 1]^2 to (r, mu); u spans the distance to the
// top between its least (straight up) and its greatest (to the horizon), v the distance to the
// horizon.
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

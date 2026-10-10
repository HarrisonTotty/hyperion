// The transmittance table (plan R05, Design note 16; Hillaire 2020, section 4): for each (r, mu)
// of Bruneton's parameterisation, the transmittance from radius r along zenith cosine mu to the
// top of the atmosphere or the ground, per channel. Follows `common.wgsl` and `medium.wgsl`; reads
// the medium's terms and their density tables (R08.T6.b).
//
// Ported from Bevy 0.19.1's crates/bevy_pbr/src/atmosphere/transmittance_lut.wgsl (MIT or
// Apache-2.0; the notice is in common.wgsl), checked against sebh's UnrealEngineSkyAtmosphere
// (MIT, Copyright (c) 2020 Epic Games, Inc.). Changes: the medium's terms are evaluated directly,
// and the optical depth is the midpoint rule over equal steps with `medium.samples` of them, where
// Bevy samples at 0.3 of each step and drops the last step's remainder, which a downward ray to the
// ground, densest at its end, feels. Each step's height, and a ray's distance to the ground, are
// taken from the texel's rho = sqrt(r^2 - bottom^2) without f32's cancellation (plan R08, R08.T6.b):
// h = (t^2 + 2 r mu t + rho^2) / (r(t) + bottom) for r(t) - bottom, and
// rho^2 / (-r mu + sqrt((r mu)^2 - rho^2)) for -r mu - sqrt(r^2 (mu^2 - 1) + bottom^2). Formed as
// differences, they lose the 0.5 m an f32 radius near 6.4e6 m carries, 4e-4 of the aerosol's
// 1.2 km scale height, and so about 3e-3 of a grazing ray's transmittance at an optical depth near
// 10 from the ground, which the CPU twin (`tablesCpu.ts`) does not.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var transmittanceOut : texture_storage_2d<rgba16float, write>;

// The optical depth from radius r along zenith cosine mu to the top or the ground, rho the texel's
// sqrt(r^2 - bottom^2).
fn opticalDepth(r : f32, mu : f32, rho : f32) -> vec3f {
  let samples = max(u32(medium.samples), 1u);
  let rho2 = rho * rho;
  let rMu = r * mu;
  var tMax = distanceToTop(r, mu);
  if (mu < 0.0 && rMu * rMu >= rho2) {
    tMax = rho2 / (-rMu + sqrt(rMu * rMu - rho2));
  }
  let dt = tMax / f32(samples);
  var depth = vec3f(0.0);
  for (var i = 0u; i < samples; i++) {
    let t = (f32(i) + 0.5) * dt;
    let heightM = (t * t + 2.0 * rMu * t + rho2) / (localR(r, mu, t) + medium.bottomRadiusM);
    depth += mediumAtHeight(heightM).extinction * dt;
  }
  return depth;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(transmittanceOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(size);
  let rMu = transmittanceUvToRMu(uv);
  // rho = H v, as transmittanceUvToRMu forms it.
  let bigH = sqrt(medium.topRadiusM * medium.topRadiusM - medium.bottomRadiusM * medium.bottomRadiusM);
  let rho = bigH * uv.y;
  textureStore(transmittanceOut, id.xy, vec4f(exp(-opticalDepth(rMu.x, rMu.y, rho)), 1.0));
}

// The transmittance table (plan R05, Design note 16; Hillaire 2020, section 4): for each (r, mu)
// of Bruneton's parameterisation, the transmittance from radius r along zenith cosine mu to the
// top of the atmosphere, per channel. Follows `common.wgsl` and `medium.wgsl`; reads
// the medium's terms and their density tables (R08.T6.b).
//
// Ported from Bevy 0.19.1's crates/bevy_pbr/src/atmosphere/transmittance_lut.wgsl (MIT or
// Apache-2.0; the notice is in common.wgsl), checked against sebh's UnrealEngineSkyAtmosphere
// (MIT, Copyright (c) 2020 Epic Games, Inc.). Changes: the medium's terms are evaluated directly,
// and the optical depth is the midpoint rule over equal steps with `medium.samples` of them, where
// Bevy samples at 0.3 of each step and drops the last step's remainder. Each texel's point is taken
// from its rho = sqrt(r^2 - bottom^2) as a height (`transmittanceTexelOf`, plan R08, Design note
// 18), and each step's height as h = (t^2 + 2 r mu t + rho^2) / (r(t) + bottom), without f32's
// cancellation (R08.T6.b, R08.T6.g). Formed as differences of radii, they lost the 0.5 m spacing
// (0.25 m rounding) of an f32 radius near 6.4e6 m, 4e-4 of the aerosol's 1.2 km scale height, and so about 3e-3 of a
// grazing ray's transmittance at an optical depth near 10 from the ground, which the CPU twin
// (`tablesCpu.ts`) does not. The texels stand at Bruneton 2017's texel mapping on both axes
// (`unitFromTexelUv`, plan R08, Design note 19), so that row 0 is the ground and the last column
// the ray tangent to it; R05, as Bevy and sebh, took none, and the reads' clamp held the outer
// half texels. Every ray is integrated to the top, as Bruneton's
// ComputeTransmittanceToTopAtmosphereBoundary does (R08.T6.g): the table's rays, x_mu <= 1, never
// pass below the ground's tangent, and the tangent ray itself, the last column, would by rounding
// "meet the ground" at its tangent point and hold the transmittance of that short path, 0.30 in
// place of 9e-5 beside it on Earth's second row. R05's branch to the ground never ran at its texel
// centres, which never reached the tangent.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var transmittanceOut : texture_storage_2d<rgba16float, write>;

// The optical depth from a texel's point along its zenith cosine to the top.
fn opticalDepth(texel : TransmittanceTexel) -> vec3f {
  let samples = max(u32(medium.samples), 1u);
  let r = medium.bottomRadiusM + texel.heightM;
  let mu = texel.mu;
  let rho2 = texel.rho * texel.rho;
  let rMu = r * mu;
  let tMax = shellDistanceToTop(medium.bottomRadiusM, medium.topHeightM, texel.heightM, mu);
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
  let sizeF = vec2f(size);
  let unit = clamp(unitFromTexelUv((vec2f(id.xy) + 0.5) / sizeF, sizeF), vec2f(0.0), vec2f(1.0));
  let texel = transmittanceTexelOf(unit);
  textureStore(transmittanceOut, id.xy, vec4f(exp(-opticalDepth(texel)), 1.0));
}

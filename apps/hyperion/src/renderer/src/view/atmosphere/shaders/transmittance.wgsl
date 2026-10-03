// The transmittance table (plan R05, Design note 16; Hillaire 2020, section 4): for each (r, mu)
// of Bruneton's parameterisation, the transmittance from radius r along zenith cosine mu to the
// top of the atmosphere or the ground, per channel. Follows `common.wgsl`.
//
// Ported from Bevy 0.19.1's crates/bevy_pbr/src/atmosphere/transmittance_lut.wgsl (MIT or
// Apache-2.0; the notice is in common.wgsl), checked against sebh's UnrealEngineSkyAtmosphere
// (MIT, Copyright (c) 2020 Epic Games, Inc.). Changes: the medium's terms are evaluated directly,
// and the optical depth is the midpoint rule over equal steps with `medium.samples` of them, where
// Bevy samples at 0.3 of each step and drops the last step's remainder, which a downward ray to the
// ground, densest at its end, feels.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var transmittanceOut : texture_storage_2d<rgba16float, write>;

fn opticalDepth(r : f32, mu : f32) -> vec3f {
  let samples = max(u32(medium.samples), 1u);
  let tMax = maxDistance(medium, r, mu);
  let dt = tMax / f32(samples);
  var depth = vec3f(0.0);
  for (var i = 0u; i < samples; i++) {
    let t = (f32(i) + 0.5) * dt;
    depth += mediumAt(medium, localR(r, mu, t)).extinction * dt;
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
  let rMu = transmittanceUvToRMu(medium, uv);
  textureStore(transmittanceOut, id.xy, vec4f(exp(-opticalDepth(rMu.x, rMu.y)), 1.0));
}

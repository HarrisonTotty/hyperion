// The multiple-scattering table (plan R05, Design note 16; Hillaire 2020, section 5.5, equations
// 5 to 10): for each height and sun zenith cosine, the luminance that every order of scattering
// past the first adds, per unit illuminance, under an isotropic phase function. Follows
// `common.wgsl`; reads the transmittance table.
//
// Ported from Bevy 0.19.1's crates/bevy_pbr/src/atmosphere/multiscattering_lut.wgsl (MIT or
// Apache-2.0; the notice is in common.wgsl) and checked against sebh's NewMultiScattCS and
// IntegrateScatteredLuminance (UnrealEngineSkyAtmosphere, Resources/RenderSkyRayMarching.hlsl,
// MIT, Copyright (c) 2020 Epic Games, Inc.). Where the two differ, sebh's reference is followed:
// the sun's direction is (0, sqrt(1 - mu^2), mu) with z up (Bevy normalises (0, mu, -1), which is
// not at zenith cosine mu); the ground's bounce is Lambertian, albedo / pi, with the sun's
// transmittance read at the ground's radius (Bevy reads it at radius 0 and omits the 1 / pi); the
// 64 directions are sebh's 8 x 8 stratified sphere, with its reduction. Each step is sampled at its
// midpoint. sebh's code is used under its licence (https://github.com/sebh/UnrealEngineSkyAtmosphere,
// master, fetched 2026-10-02):
//
//   MIT License
//
//   Copyright (c) 2020 Epic Games, Inc.
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//   and associated documentation files (the "Software"), to deal in the Software without
//   restriction, including without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//   Software is furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be included in all copies or
//   substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
//   BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//   NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//   DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var transmittance : texture_2d<f32>;
@group(0) @binding(2) var multiScatteringOut : texture_storage_2d<rgba16float, write>;

const PI : f32 = 3.14159265358979;
const DIRECTIONS : u32 = 64u;
const SQRT_DIRECTIONS : u32 = 8u;

var<workgroup> secondOrder : array<vec3f, DIRECTIONS>;
var<workgroup> transfer : array<vec3f, DIRECTIONS>;

fn transmittanceToSun(r : f32, muSun : f32) -> vec3f {
  return bilinear(transmittance, transmittanceRMuToUv(medium, r, muSun)).rgb;
}

struct Integrated {
  // The luminance this direction brings, per unit illuminance: single scattering under the
  // isotropic phase, and the ground's bounce.
  luminance : vec3f,
  // f_ms's integrand: the scattering along the ray, as if each point received unit luminance.
  transfer : vec3f,
}

fn integrate(r : f32, dir : vec3f, sun : vec3f) -> Integrated {
  let origin = vec3f(0.0, 0.0, r);
  let mu = dir.z;
  let hitsGround = intersectsGround(medium, r, mu);
  let tMax = maxDistance(medium, r, mu);
  let samples = max(u32(medium.samples), 1u);
  let dt = tMax / f32(samples);
  var luminance = vec3f(0.0);
  var transferSum = vec3f(0.0);
  var throughput = vec3f(1.0);
  for (var i = 0u; i < samples; i++) {
    let p = origin + ((f32(i) + 0.5) * dt) * dir;
    let rP = length(p);
    let muSun = dot(sun, p / rP);
    let local = mediumAt(medium, rP);
    let extinction = max(local.extinction, vec3f(1e-12));
    let stepTransmittance = exp(-local.extinction * dt);
    let lit = select(1.0, 0.0, intersectsGround(medium, rP, muSun));
    let source = lit * transmittanceToSun(rP, muSun) * local.scattering / (4.0 * PI);
    luminance += throughput * (source - source * stepTransmittance) / extinction;
    transferSum += throughput * (local.scattering - local.scattering * stepTransmittance) / extinction;
    throughput *= stepTransmittance;
  }
  if (hitsGround) {
    let ground = origin + tMax * dir;
    let up = normalize(ground);
    let muSun = dot(sun, up);
    luminance += transmittanceToSun(medium.bottomRadiusM, muSun) * throughput
      * max(muSun, 0.0) * medium.groundAlbedo.rgb / PI;
  }
  return Integrated(luminance, transferSum);
}

@compute @workgroup_size(1, 1, 64)
fn main(
  @builtin(global_invocation_id) id : vec3u,
  @builtin(local_invocation_id) lid : vec3u,
) {
  let size = vec2f(textureDimensions(multiScatteringOut));
  let uv = (vec2f(id.xy) + 0.5) / size;
  let rMu = multiScatteringUvToRMu(medium, uv, size);
  let muSun = rMu.y;
  let sun = vec3f(0.0, sqrt(max(1.0 - muSun * muSun, 0.0)), muSun);

  // sebh's stratified directions: uniform in azimuth and in the cosine of the polar angle.
  let a = (0.5 + f32(lid.z / SQRT_DIRECTIONS)) / f32(SQRT_DIRECTIONS);
  let b = (0.5 + f32(lid.z % SQRT_DIRECTIONS)) / f32(SQRT_DIRECTIONS);
  let theta = 2.0 * PI * a;
  let cosPhi = 1.0 - 2.0 * b;
  let sinPhi = sqrt(max(1.0 - cosPhi * cosPhi, 0.0));
  let dir = vec3f(cos(theta) * sinPhi, sin(theta) * sinPhi, cosPhi);

  let result = integrate(rMu.x, dir, sun);
  secondOrder[lid.z] = result.luminance;
  transfer[lid.z] = result.transfer;
  workgroupBarrier();
  for (var step = DIRECTIONS / 2u; step > 0u; step >>= 1u) {
    if (lid.z < step) {
      secondOrder[lid.z] += secondOrder[lid.z + step];
      transfer[lid.z] += transfer[lid.z + step];
    }
    workgroupBarrier();
  }
  if (lid.z != 0u) {
    return;
  }
  // Each direction stands for 4 pi / 64 sr, and the isotropic phase is 1 / (4 pi): the mean.
  let secondOrderLuminance = secondOrder[0] / f32(DIRECTIONS);
  let fMs = transfer[0] / f32(DIRECTIONS);
  // Equation 10: every further order as a geometric series.
  textureStore(multiScatteringOut, id.xy, vec4f(secondOrderLuminance / (1.0 - fMs), 1.0));
}

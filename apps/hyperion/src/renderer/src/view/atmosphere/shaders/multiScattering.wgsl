// The multiple-scattering table (plan R05, Design note 16; Hillaire 2020, section 5.5, equations
// 5 to 10): for each height and sun zenith cosine, the luminance that every order of scattering
// past the first adds, per unit illuminance, under an isotropic phase function, whatever the terms'
// own phase functions: Hillaire's isotropic table for `thin` worlds (plan R08, R08.T6.b). Follows
// `common.wgsl` and `medium.wgsl`; reads the medium's terms, their density tables and the
// transmittance table, as optical depth with its absorber columns, and one light slot's curves of
// growth (`curveSlot`, 0 in R08.T6.c), which the sun's transmittance and each direction's own path
// take (plan R08, Design notes 5 and 8, R08.T6.c).
//
// Ported from Bevy 0.19.1's crates/bevy_pbr/src/atmosphere/multiscattering_lut.wgsl (MIT or
// Apache-2.0; the notice is in common.wgsl) and checked against sebh's NewMultiScattCS and
// IntegrateScatteredLuminance (UnrealEngineSkyAtmosphere, Resources/RenderSkyRayMarching.hlsl, MIT,
// Copyright (c) 2020 Epic Games, Inc.). Where the two differ, sebh's reference is followed: the
// sun's direction is (0, sqrt(1 - mu^2), mu) with z up (Bevy normalises (0, mu, -1), which is not
// at zenith cosine mu); the ground's bounce is Lambertian, albedo / pi, with the sun's
// transmittance read at the ground's radius (Bevy reads it at radius 0 and omits the 1 / pi); the
// 64 directions are sebh's 8 x 8 stratified sphere, with its reduction. Each ray's steps are placed
// by R05.T12.e's rule (`marchSplit` with `fromCamera`, plan R08, R08.T6.d), where sebh and Bevy
// take even ones, each sampled at its midpoint, and its in-scattering is taken without f32's
// cancellation (`stepFactor` in common.wgsl, R05.T12.e's step factor, taken here in R08.T6.a).
// Hillaire's 1 - f_ms is accumulated, not formed (R08.T6.d; `decision-r08-t6b-floor.md` section
// 9.3): in nearly conservative air f_ms comes within f32's spacing of 1, so that one less it
// rounded to 0, and a Venus-class table was NaN where the sun was down. Each texel stands at its
// height, 10 m + v (H - 10 m), and the ray's length, each sample's height, the ground's shadow and
// the sun's transmittance are taken from heights, never as a radius less the ground's (plan R08,
// Design note 18, R08.T6.g); the transmittance table is read through Bruneton 2017's texel mapping
// (Design note 19), whose write here is sebh's own, unchanged. sebh's code is used under its
// licence (https://github.com/sebh/UnrealEngineSkyAtmosphere, master, fetched 2026-10-02):
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
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var transmittance : texture_2d<f32>;
@group(0) @binding(4) var multiScatteringOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var absorberColumns : texture_2d<f32>;
@group(0) @binding(6) var absorberCurves : texture_2d<f32>;

const PI : f32 = 3.14159265358979;
const DIRECTIONS : u32 = 64u;
const SQRT_DIRECTIONS : u32 = 8u;

// cos and sin of the eight azimuths 2 pi (k + 1/2) / 8, k = 0..7, sebh's stratification: cos(pi / 8)
// and sin(pi / 8) with their signs in turn, as constants (plan R08, R08.T6.b). f32's cos and sin of
// 2 pi a, which WGSL bounds only to 2^-11 absolute and only within [-pi, pi], moved a night texel
// at the terminator by up to 1.6e-2 against the CPU twin: there a direction's error of 1e-5
// moves the texel by 3e-3 of itself.
fn azimuthOf(k : u32) -> vec2f {
  let c = 0.92387953251128674;
  let s = 0.38268343236508978;
  var azimuths = array<vec2f, 8>(
    vec2f(c, s),
    vec2f(s, c),
    vec2f(-s, c),
    vec2f(-c, s),
    vec2f(-c, -s),
    vec2f(-s, -c),
    vec2f(s, -c),
    vec2f(c, -s),
  );
  return azimuths[k];
}

var<workgroup> secondOrder : array<vec3f, DIRECTIONS>;
var<workgroup> escape : array<vec3f, DIRECTIONS>;

// The sun's transmittance from a height above the ground, read on the table's sphere: its stored
// optical depth and absorber columns, with light slot `curveSlot()`'s curves of growth,
// exponentiated in f32 (R08.T6.c). The one table R08.T6.c builds is shared by every sun and built
// with slot 0's curves, the body's brightest star's (plan R08's Risks, "Which star's absorber
// curve the shared tables are built with"); R08.T7 builds one a light slot through `curveSlot`.
fn transmittanceToSun(heightM : f32, muSun : f32) -> vec3f {
  let h = clamp(heightM, 0.0, medium.topHeightM);
  let unit = transmittanceUnitAt(medium.bottomRadiusM, medium.topHeightM, h, muSun);
  let count = absorberCount();
  let slot = curveSlot();
  return transmittanceThroughTables(
    transmittance, absorberColumns, absorberCurves, count, slot, unit
  );
}

struct Integrated {
  // The luminance this direction brings, per unit illuminance: single scattering under the
  // isotropic phase, and the ground's bounce.
  luminance : vec3f,
  // This direction's 1 - f_ms (plan R08, R08.T6.d; `decision-r08-t6b-floor.md` section 9.3): the
  // throughput at the ray's end, the light that leaves the medium, plus the light the medium
  // absorbs along it, sum throughput (sigma_a dt + the step's absorber depth) stepFactor(d).
  // Since sum throughput (1 - e^-d) telescopes to 1 - throughput at the end, it is Hillaire's
  // 1 - f_ms exactly, positive by construction, and never formed as one less a sum near 1.
  escape : vec3f,
}

fn integrate(heightM : f32, dir : vec3f, sun : vec3f) -> Integrated {
  let bottom = medium.bottomRadiusM;
  let origin = vec3f(0.0, 0.0, bottom + heightM);
  let mu = dir.z;
  let hitsGround = groundHitFromHeight(bottom, heightM, mu);
  let tMax = maxDistanceFromHeight(heightM, mu);
  let samples = max(u32(medium.samples), 2u);
  // The steps placed toward the ray's lowest point and, before it, toward the texel's point
  // (`marchSplit` with `fromCamera`, R05.T12.e's rule; plan R08, R08.T6.d), where R05 took even
  // steps: they erred by up to 10.5% in the table, and placed steps by 2.6% (R08.T6.a).
  let split = marchSplit(0.0, tMax, -(bottom + heightM) * mu, samples, true);
  var luminance = vec3f(0.0);
  var escaped = vec3f(0.0);
  var throughput = vec3f(1.0);
  var path = absorberPathStart();
  for (var i = 0u; i < samples; i++) {
    let stepAt = marchStep(split, i);
    let t = stepAt.tM;
    let dt = stepAt.dtM;
    let p = origin + t * dir;
    let rP = length(p);
    let muSun = dot(sun, p / rP);
    let sampleHeightM = heightAlong(heightM, mu, t, rP);
    let local = mediumAtHeight(sampleHeightM);
    // The direction's own path takes its absorbers by its column, under the sun's curves (R08.T6.c).
    let next = absorberStep(absorberCurves, absorberCount(), curveSlot(), path, local.columns, dt);
    let stepAbsorbed = next.depth - path.depth;
    let stepDepth = local.extinction * dt + stepAbsorbed;
    path = next;
    let stepTransmittance = exp(-stepDepth);
    let lit = select(1.0, 0.0, groundHitFromHeight(bottom, max(sampleHeightM, 0.0), muSun));
    let source = lit * transmittanceToSun(sampleHeightM, muSun) * local.scattering / (4.0 * PI);
    let factor = stepFactor(stepDepth);
    luminance += throughput * source * dt * factor;
    escaped += throughput * (local.absorption * dt + stepAbsorbed) * factor;
    throughput *= stepTransmittance;
  }
  if (hitsGround) {
    let ground = origin + tMax * dir;
    let up = normalize(ground);
    let muSun = dot(sun, up);
    luminance += transmittanceToSun(0.0, muSun) * throughput
      * max(muSun, 0.0) * medium.groundAlbedo.rgb / PI;
  }
  return Integrated(luminance, escaped + throughput);
}

@compute @workgroup_size(1, 1, 64)
fn main(
  @builtin(global_invocation_id) id : vec3u,
  @builtin(local_invocation_id) lid : vec3u,
) {
  let size = vec2f(textureDimensions(multiScatteringOut));
  // The texel's unit range (Bruneton's mapping, sebh's write): x the sun's zenith cosine, y the
  // height from GROUND_OFFSET_M to the top.
  let unit = clamp(unitFromTexelUv((vec2f(id.xy) + 0.5) / size, size), vec2f(0.0), vec2f(1.0));
  let heightM = GROUND_OFFSET_M + unit.y * (medium.topHeightM - GROUND_OFFSET_M);
  let muSun = unit.x * 2.0 - 1.0;
  let sun = vec3f(0.0, sqrt(max(1.0 - muSun * muSun, 0.0)), muSun);

  // sebh's stratified directions: uniform in azimuth and in the cosine of the polar angle.
  let azimuth = azimuthOf(lid.z / SQRT_DIRECTIONS);
  let b = (0.5 + f32(lid.z % SQRT_DIRECTIONS)) / f32(SQRT_DIRECTIONS);
  let cosPhi = 1.0 - 2.0 * b;
  let sinPhi = sqrt(max(1.0 - cosPhi * cosPhi, 0.0));
  let dir = vec3f(azimuth.x * sinPhi, azimuth.y * sinPhi, cosPhi);

  let result = integrate(heightM, dir, sun);
  secondOrder[lid.z] = result.luminance;
  escape[lid.z] = result.escape;
  workgroupBarrier();
  for (var step = DIRECTIONS / 2u; step > 0u; step >>= 1u) {
    if (lid.z < step) {
      secondOrder[lid.z] += secondOrder[lid.z + step];
      escape[lid.z] += escape[lid.z + step];
    }
    workgroupBarrier();
  }
  if (lid.z != 0u) {
    return;
  }
  // Each direction stands for 4 pi / 64 sr, and the isotropic phase is 1 / (4 pi): the mean.
  let secondOrderLuminance = secondOrder[0] / f32(DIRECTIONS);
  let meanEscape = escape[0] / f32(DIRECTIONS);
  // Equations 9 and 10: every further order as a geometric series, L2 / (1 - f_ms), with 1 - f_ms
  // the mean escape (R08.T6.d). Where it is 0 in f32, every direction crossing some 100 optical
  // depths with no absorption, the texel keeps L2 alone, the second order, a lower bound of the
  // term; alpha counts its channels so stored, 0 to 3, for the smoke. No epsilon and no cap: a value past the half float's largest is the smoke's to refuse
  // (`decision-r08-t6b-floor.md` section 9.3).
  let alone = meanEscape <= vec3f(0.0);
  let stored = select(secondOrderLuminance / meanEscape, secondOrderLuminance, alone);
  let aloneCount = dot(select(vec3f(0.0), vec3f(1.0), alone), vec3f(1.0));
  textureStore(multiScatteringOut, id.xy, vec4f(stored, aloneCount));
}

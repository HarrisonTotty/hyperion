// The atmosphere's shared WGSL (plan R05, Design note 16): the medium as a list of terms, the
// spherical shell's geometry and the tables' parameterisations on a shell of given radii. Prepended
// to every kernel and to the composite (`tables.ts`, `hillaire.ts`). The helpers that read a
// kernel's `medium` uniform are in `medium.wgsl` and `source.wgsl`, which the composite does not
// take.
//
// Ported from Bevy 0.19.1's atmosphere (crates/bevy_pbr/src/atmosphere/functions.wgsl and
// bruneton_functions.wgsl, https://github.com/bevyengine/bevy, tag v0.19.1, fetched 2026-10-02),
// dual-licensed MIT or Apache-2.0, used here under its LICENSE-MIT, which names no holder (the Bevy
// contributors):
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
//
// The transmittance parameterisation and the shell intersections are Bevy's port of Bruneton's
// (precomputed_atmospheric_scattering, atmosphere/functions.glsl), under Bruneton's licence:
//
//   Copyright (c) 2017 Eric Bruneton. All rights reserved.
//   Copyright (c) 2008 INRIA. All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without modification, are permitted
//   provided that the following conditions are met:
//   1. Redistributions of source code must retain the above copyright notice, this list of
//      conditions and the following disclaimer.
//   2. Redistributions in binary form must reproduce the above copyright notice, this list of
//      conditions and the following disclaimer in the documentation and/or other materials
//      provided with the distribution.
//   3. Neither the name of the copyright holders nor the names of its contributors may be used to
//      endorse or promote products derived from this software without specific prior written
//      permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
//   IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY
//   AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
//   CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//   CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//   THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
//   OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//   POSSIBILITY OF SUCH DAMAGE.
//
// Changes: Bevy's medium density and scattering tables are replaced by the terms themselves,
// evaluated per sample from `Medium` (each term's profile, scattering and absorption); the
// functions take the shell's radii from `Medium`, read from the kernel's uniform in place
// (`medium.wgsl`); names follow the project's WGSL style. The multiple-scattering table's mapping
// (GROUND_OFFSET_M, the sub-texel remap) follows sebh's UnrealEngineSkyAtmosphere (MIT, Copyright
// (c) 2020 Epic Games, Inc.; the full notice is in multiScattering.wgsl).

// The most terms a medium may have; `tables.ts` holds the same number.
const MAX_TERMS: u32 = 8u;

// One term: per-channel coefficients at unit density, m^-1, and its density profile:
// profile.x 0 for exponential (y the scale height, m), 1 for a tent (y bottom, z peak, w top, m).
// The phase: scattering.w 0 none, 1 Rayleigh, 2 Cornette–Shanks with its g in absorption.w.
struct Term {
  scattering : vec4f,
  absorption : vec4f,
  profile : vec4f,
}

// The medium on a spherical shell, metres from the centre.
struct Medium {
  bottomRadiusM : f32,
  topRadiusM : f32,
  termCount : f32,
  samples : f32,
  groundAlbedo : vec4f,
  terms : array<Term, MAX_TERMS>,
}

struct MediumSample {
  scattering : vec3f,
  extinction : vec3f,
}

fn densityOf(profile : vec4f, heightM : f32) -> f32 {
  let h = max(heightM, 0.0);
  if (profile.x < 0.5) {
    return exp(-h / profile.y);
  }
  let rising = (h - profile.y) / (profile.z - profile.y);
  let falling = (profile.w - h) / (profile.w - profile.z);
  return max(min(rising, falling), 0.0);
}

// A march step's in-scattering factor g(x) = (1 - e^-x) / x per channel, x = sigma_t dt the step's
// optical depth: a step of constant medium adds throughput * S * dt * g(x), the integral that
// sebh's IntegrateScatteredLuminance takes as (S - S e^-x) / sigma_t (R05.T12.e). Below x = 0.01 it
// is the series 1 - x (1/2 - x/6), whose truncation, x^3 / 24, is under 4.2e-8 there; from 0.01 up,
// (1 - exp(-x)) / x, within 1.8e-5 just above the switch (WGSL's exp bound) and falling as 1 / x.
// In f32 the old form errs by up to about 9e-8 / x of itself with a correctly rounded exp, and
// 2.4e-7 / x within WGSL's bound: at x = 1e-6, 1.3% and up to 18% (decision-r05-high-atmosphere.md,
// addendum A). On SwiftShader a converged march of 1,024 steps lost up to 15% that way, and in lane
// C's f32 model a 90 km tangent ray lost 13%.
fn stepFactor(x : vec3f) -> vec3f {
  return select((1.0 - exp(-x)) / x, 1.0 - x * (0.5 - x / 6.0), x < vec3f(0.01));
}

// The radius at distance t along a ray from radius r at zenith cosine mu.
fn localR(r : f32, mu : f32, t : f32) -> f32 {
  return sqrt(max(t * t + 2.0 * r * mu * t + r * r, 0.0));
}

// Whether a ray from radius r at zenith cosine mu meets a sphere of radius `bottom`.
fn shellIntersectsGround(bottom : f32, r : f32, mu : f32) -> bool {
  return mu < 0.0 && r * r * (mu * mu - 1.0) + bottom * bottom >= 0.0;
}

// The transmittance table's (u, v) for (r, mu) on a shell of the given radii.
fn shellRMuToUv(bottom : f32, top : f32, r : f32, mu : f32) -> vec2f {
  let bigH = sqrt(top * top - bottom * bottom);
  let rho = sqrt(max(r * r - bottom * bottom, 0.0));
  let discriminant = max(r * r * (mu * mu - 1.0) + top * top, 0.0);
  let d = max(-r * mu + sqrt(discriminant), 0.0);
  let dMin = top - r;
  let dMax = rho + bigH;
  return vec2f((d - dMin) / (dMax - dMin), rho / bigH);
}

// The multiple-scattering table's lowest radius sits this far above the ground, m, so that a sun
// on the horizon is not shadowed by the ground it stands on (sebh's PLANET_RADIUS_OFFSET).
const GROUND_OFFSET_M: f32 = 10.0;

// sebh's sub-texel remapping: texel centres to [0, 1] and back, so that the table's edge texels
// hold its range's ends.
fn subUvsToUnit(uv : vec2f, size : vec2f) -> vec2f {
  return (uv - 0.5 / size) * (size / (size - 1.0));
}

fn unitToSubUvs(unit : vec2f, size : vec2f) -> vec2f {
  return (unit + 0.5 / size) * (size / (size + 1.0));
}

// The multiple-scattering table's (u, v) for (r, mu_sun) on a shell of the given radii.
fn shellMultiScatteringRMuToUv(bottom : f32, top : f32, r : f32, muSun : f32, size : vec2f) -> vec2f {
  let v = (r - bottom - GROUND_OFFSET_M) / (top - bottom - GROUND_OFFSET_M);
  return unitToSubUvs(clamp(vec2f(muSun * 0.5 + 0.5, v), vec2f(0.0), vec2f(1.0)), size);
}

// Bilinear read of a table whose texel centres sit at (i + 0.5) / size, clamped at its edges:
// compute passes bind no sampler, so the filter is written out.
fn bilinear(table : texture_2d<f32>, uv : vec2f) -> vec4f {
  let size = vec2f(textureDimensions(table));
  let p = clamp(uv * size - 0.5, vec2f(0.0), size - 1.0);
  let p0 = vec2u(floor(p));
  let p1 = min(p0 + 1u, vec2u(size) - 1u);
  let f = p - floor(p);
  let a = mix(textureLoad(table, p0, 0), textureLoad(table, vec2u(p1.x, p0.y), 0), f.x);
  let b = mix(textureLoad(table, vec2u(p0.x, p1.y), 0), textureLoad(table, p1, 0), f.x);
  return mix(a, b, f.y);
}

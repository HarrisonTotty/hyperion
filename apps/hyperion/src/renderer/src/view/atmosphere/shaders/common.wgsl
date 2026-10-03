// The atmosphere's shared WGSL (plan R05, Design note 16): the medium as a list of terms, the
// spherical shell's geometry and the transmittance table's (r, mu) parameterisation. Prepended to
// each kernel by `tables.ts`.
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
// functions take the shell's radii from `Medium`; names follow the project's WGSL style. The
// multiple-scattering table's mapping (GROUND_OFFSET_M, the sub-texel remap) follows sebh's
// UnrealEngineSkyAtmosphere (MIT, Copyright (c) 2020 Epic Games, Inc.; the full notice is in
// multiScattering.wgsl).

// The most terms a medium may have; `tables.ts` holds the same number.
const MAX_TERMS: u32 = 8u;

// One term: per-channel coefficients at unit density, m^-1, and its density profile:
// profile.x 0 for exponential (y the scale height, m), 1 for a tent (y bottom, z peak, w top, m).
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

fn mediumAt(m : Medium, r : f32) -> MediumSample {
  let h = r - m.bottomRadiusM;
  var scattering = vec3f(0.0);
  var extinction = vec3f(0.0);
  let count = min(u32(m.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = m.terms[i];
    let d = densityOf(term.profile, h);
    scattering += term.scattering.rgb * d;
    extinction += (term.scattering.rgb + term.absorption.rgb) * d;
  }
  return MediumSample(scattering, extinction);
}

// The radius at distance t along a ray from radius r at zenith cosine mu.
fn localR(r : f32, mu : f32, t : f32) -> f32 {
  return sqrt(max(t * t + 2.0 * r * mu * t + r * r, 0.0));
}

fn distanceToTop(m : Medium, r : f32, mu : f32) -> f32 {
  let discriminant = max(r * r * (mu * mu - 1.0) + m.topRadiusM * m.topRadiusM, 0.0);
  return max(-r * mu + sqrt(discriminant), 0.0);
}

fn distanceToBottom(m : Medium, r : f32, mu : f32) -> f32 {
  let discriminant = max(r * r * (mu * mu - 1.0) + m.bottomRadiusM * m.bottomRadiusM, 0.0);
  return max(-r * mu - sqrt(discriminant), 0.0);
}

fn intersectsGround(m : Medium, r : f32, mu : f32) -> bool {
  return mu < 0.0 && r * r * (mu * mu - 1.0) + m.bottomRadiusM * m.bottomRadiusM >= 0.0;
}

// The ray's length inside the atmosphere, to the ground or to the top.
fn maxDistance(m : Medium, r : f32, mu : f32) -> f32 {
  if (intersectsGround(m, r, mu)) {
    return distanceToBottom(m, r, mu);
  }
  return distanceToTop(m, r, mu);
}

// Bruneton and Neyret 2008, section 4: (u, v) in [0, 1]^2 to (r, mu); u spans the distance to the
// top between its least (straight up) and its greatest (to the horizon), v the distance to the
// horizon.
fn transmittanceUvToRMu(m : Medium, uv : vec2f) -> vec2f {
  let bigH = sqrt(m.topRadiusM * m.topRadiusM - m.bottomRadiusM * m.bottomRadiusM);
  let rho = bigH * uv.y;
  let r = sqrt(rho * rho + m.bottomRadiusM * m.bottomRadiusM);
  let dMin = m.topRadiusM - r;
  let dMax = rho + bigH;
  let d = dMin + uv.x * (dMax - dMin);
  var mu = 1.0;
  if (d != 0.0) {
    mu = (bigH * bigH - rho * rho - d * d) / (2.0 * r * d);
  }
  return vec2f(r, clamp(mu, -1.0, 1.0));
}

fn transmittanceRMuToUv(m : Medium, r : f32, mu : f32) -> vec2f {
  let bigH = sqrt(m.topRadiusM * m.topRadiusM - m.bottomRadiusM * m.bottomRadiusM);
  let rho = sqrt(max(r * r - m.bottomRadiusM * m.bottomRadiusM, 0.0));
  let d = distanceToTop(m, r, mu);
  let dMin = m.topRadiusM - r;
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

// The multiple-scattering table's (u, v) to (r, mu_sun): u the sun's zenith cosine, v the height.
fn multiScatteringUvToRMu(m : Medium, uv : vec2f, size : vec2f) -> vec2f {
  let unit = clamp(subUvsToUnit(uv, size), vec2f(0.0), vec2f(1.0));
  let r = m.bottomRadiusM + GROUND_OFFSET_M + unit.y * (m.topRadiusM - m.bottomRadiusM - GROUND_OFFSET_M);
  return vec2f(r, unit.x * 2.0 - 1.0);
}

fn multiScatteringRMuToUv(m : Medium, r : f32, muSun : f32, size : vec2f) -> vec2f {
  let v = (r - m.bottomRadiusM - GROUND_OFFSET_M) / (m.topRadiusM - m.bottomRadiusM - GROUND_OFFSET_M);
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

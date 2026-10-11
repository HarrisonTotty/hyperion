// The atmosphere's shared WGSL (plan R05, Design note 16): the medium as a list of terms, the
// spherical shell's geometry from a height and the tables' parameterisations on a sphere of given
// radius (plan R08, Design notes 18 and 19). Prepended, after `view/shaders/preciseTrig.wgsl`, to
// every kernel and to the composite (`tables.ts`, `hillaire.ts`). The helpers that read a kernel's
// `medium` uniform are in `medium.wgsl` and `source.wgsl`, which the composite does not take.
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
// evaluated per sample (each term's profile, scattering and absorption, from the kernel's `terms`
// storage buffer, R08.T6.b); the functions take the shell from the kernel's `medium` uniform, read
// in place (`medium.wgsl`); names follow the project's WGSL style. The shell's forms take a point's
// height, never a difference of radii (plan R08, Design note 18, R08.T6.g): in f32 a radius near
// 6.4e6 m carries 0.25 m of rounding, and r^2 - R^2 near the ground loses most of its digits. Every
// table maps its unit ranges to its texels by Bruneton's exact pair (`unitFromTexelUv`,
// `texelUvFromUnit`; plan R08, Design note 19, R08.T6.g), in place of sebh's sub-texel remap, whose
// read did not invert its write, and of the transmittance table's none. The multiple-scattering
// table's ground offset is a deliberate change from sebh's code (GROUND_OFFSET_M, below).

// The most terms a medium may have; `tables.ts` holds the same number and refuses more.
const MAX_TERMS: u32 = 8u;

// One term (plan R08, R08.T6.b): per-channel coefficients at unit density, m^-1, in the rgb of
// `scattering` and `absorption`, its density profile and its phase function. The profile:
// profile.x 0 for exponential (y the scale height, m), 1 for a tent (y bottom, z peak, w top, m),
// 2 for tabulated (y the top of its table, m, read from the term's layer of `densityTables`,
// `medium.wgsl`). The phase: phase.w 0 none, 1 Rayleigh with its depolarisation ratio per channel
// in xyz (R08.T3.c), 2 Cornette–Shanks with its g in x, 3 tabulated per channel (the term's layer
// of `phaseTables`, `source.wgsl`). A term's layer in either table is its index in `terms`.
struct Term {
  scattering : vec4f,
  absorption : vec4f,
  profile : vec4f,
  phase : vec4f,
}

// The medium's shell, its term count, a kernel's steps along each ray and the ground's albedo:
// R05's `Medium` uniform without its terms, which travel in a storage buffer of MAX_TERMS terms
// (R08.T6.b). The shell is its ground's radius and the atmosphere's height above it, formed in
// f64 (R08.T6.g), so that no kernel forms the height as the top's radius less the ground's.
struct Medium {
  bottomRadiusM : f32,
  topHeightM : f32,
  termCount : f32,
  samples : f32,
  groundAlbedo : vec4f,
}

struct MediumSample {
  scattering : vec3f,
  extinction : vec3f,
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

// The radius at distance t along a ray from radius r at zenith cosine mu: a magnitude, which
// carries f32's relative precision, so the forms below take it, never a difference of it.
fn localR(r : f32, mu : f32, t : f32) -> f32 {
  return sqrt(max(t * t + 2.0 * r * mu * t + r * r, 0.0));
}

// --- The shell from a height (plan R08, Design note 18) -------------------------------------------
// A sphere of radius `bottom`, the atmosphere `topHeightM` above it, and a point `heightM` above
// the sphere at r = bottom + heightM, a ray from it at zenith cosine mu.

// r^2 - bottom^2 = h (2 bottom + h).
fn shellAboveGround(bottom : f32, heightM : f32) -> f32 {
  return heightM * (2.0 * bottom + heightM);
}

// top^2 - r^2 = (H - h)(2 bottom + H + h).
fn shellBelowTop(bottom : f32, topHeightM : f32, heightM : f32) -> f32 {
  return (topHeightM - heightM) * (2.0 * bottom + topHeightM + heightM);
}

// Whether the ray meets the sphere: mu < 0 and (r mu)^2 >= r^2 - bottom^2. Also the ground's
// shadow at a point, for the sun's zenith cosine there.
fn groundHitFromHeight(bottom : f32, heightM : f32, mu : f32) -> bool {
  let rMu = (bottom + heightM) * mu;
  return mu < 0.0 && rMu * rMu >= shellAboveGround(bottom, heightM);
}

// The distance to the sphere along a ray that meets it (`groundHitFromHeight`): the near root
// A / (-r mu + sqrt((r mu)^2 - A)), A = r^2 - bottom^2, which does not cancel.
fn shellDistanceToGround(bottom : f32, heightM : f32, mu : f32) -> f32 {
  let rMu = (bottom + heightM) * mu;
  let aboveGround = shellAboveGround(bottom, heightM);
  return aboveGround / (-rMu + sqrt(max(rMu * rMu - aboveGround, 0.0)));
}

// The distance to the top: B / (r mu + sqrt((r mu)^2 + B)) upward and -r mu + sqrt((r mu)^2 + B)
// otherwise, B = top^2 - r^2, each the root that does not cancel; 0 upward from above the top.
// Every caller takes h at or below H.
fn shellDistanceToTop(bottom : f32, topHeightM : f32, heightM : f32, mu : f32) -> f32 {
  let rMu = (bottom + heightM) * mu;
  let belowTop = shellBelowTop(bottom, topHeightM, heightM);
  let root = sqrt(max(rMu * rMu + belowTop, 0.0));
  return max(select(-rMu + root, belowTop / (rMu + root), mu > 0.0), 0.0);
}

// The ray's length inside the atmosphere, to the ground or to the top.
fn shellMaxDistance(bottom : f32, topHeightM : f32, heightM : f32, mu : f32) -> f32 {
  if (groundHitFromHeight(bottom, heightM, mu)) {
    return shellDistanceToGround(bottom, heightM, mu);
  }
  return shellDistanceToTop(bottom, topHeightM, heightM, mu);
}

// The height at distance t along the ray, rT its radius there (`localR`):
// (t^2 + 2 r mu t + (r^2 - bottom^2)) / (rT + bottom) = rT - bottom without the difference.
fn shellHeightAlong(bottom : f32, heightM : f32, mu : f32, t : f32, rT : f32) -> f32 {
  let rMu = (bottom + heightM) * mu;
  return (t * t + 2.0 * rMu * t + shellAboveGround(bottom, heightM)) / (rT + bottom);
}

// The transmittance table's unit range (x_mu, x_r) at a point and zenith cosine: Bruneton 2017's
// GetTransmittanceTextureUvFromRMu (precomputed_atmospheric_scattering, atmosphere/functions.glsl)
// before its texel mapping. x_mu = (d - d_min) / (d_max - d_min), d the distance to the top along
// mu, d_min = H - h and d_max = rho + H_rho; x_r = rho / H_rho, which is Bruneton and Neyret 2008's
// (section 4), with rho = sqrt(h (2 bottom + h)) and H_rho = sqrt(H (2 bottom + H)) the distances
// to the horizon from the point and from the top.
fn transmittanceUnitAt(bottom : f32, topHeightM : f32, heightM : f32, mu : f32) -> vec2f {
  let bigH = sqrt(topHeightM * (2.0 * bottom + topHeightM));
  let rho = sqrt(max(shellAboveGround(bottom, heightM), 0.0));
  let d = shellDistanceToTop(bottom, topHeightM, heightM, mu);
  let dMin = topHeightM - heightM;
  let dMax = rho + bigH;
  return vec2f((d - dMin) / (dMax - dMin), rho / bigH);
}

// The multiple-scattering table's lowest row sits this far above the ground, m, so that a sun on
// the horizon is not shadowed by the ground it stands on. A deliberate change from sebh's code,
// which has the same aim (PLANET_RADIUS_OFFSET) and takes its 0.01 km off the height range as here,
// but also adds the 0.01 to the unitless row coordinate when NewMultiScattCS writes a row, so that
// row 0 sits about 1 km up on Earth, and applies no offset when GetMultipleScattering reads one.
// Here the 10 m is a length on the write (`multiScattering.wgsl`) and on the read
// (`multiScatteringUnitAt`) alike, so that the two agree.
const GROUND_OFFSET_M: f32 = 10.0;

// The multiple-scattering table's unit range at a height and the sun's zenith cosine: x the sun's
// cosine, mu_s / 2 + 1/2, and y the height, (h - 10 m) / (H - 10 m), from the height itself.
fn multiScatteringUnitAt(topHeightM : f32, heightM : f32, muSun : f32) -> vec2f {
  let y = (heightM - GROUND_OFFSET_M) / (topHeightM - GROUND_OFFSET_M);
  return clamp(vec2f(muSun * 0.5 + 0.5, y), vec2f(0.0), vec2f(1.0));
}

// Bruneton 2017's texel mapping (precomputed_atmospheric_scattering, functions.glsl:
// GetUnitRangeFromTextureCoord and GetTextureCoordFromUnitRange; plan R08, Design note 19): a unit
// range's 0 and 1 stand at the centres of an axis's first and last texels, and each function
// inverts the other exactly. Every axis it maps has at least 2 texels.
fn unitFromTexelUv(uv : vec2f, size : vec2f) -> vec2f {
  return (uv * size - 0.5) / (size - 1.0);
}

fn texelUvFromUnit(unit : vec2f, size : vec2f) -> vec2f {
  return (unit * (size - 1.0) + 0.5) / size;
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

// A table's bilinear read at a unit range, clamped to [0, 1], through Bruneton's texel mapping.
fn tableAtUnit(table : texture_2d<f32>, unit : vec2f) -> vec4f {
  let size = vec2f(textureDimensions(table));
  return bilinear(table, texelUvFromUnit(clamp(unit, vec2f(0.0), vec2f(1.0)), size));
}

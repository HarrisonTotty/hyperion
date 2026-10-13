// The atmosphere's shared WGSL (plan R05, Design note 16): the medium as a list of terms, the
// spherical shell's geometry from a height, the tables' parameterisations on a sphere of given
// radius (plan R08, Design notes 18 and 19) and the steps of a march (R05.T12.e). Prepended, after
// `view/shaders/preciseTrig.wgsl`, to every kernel and to the composite (`tables.ts`,
// `hillaire.ts`). The helpers that read a kernel's `medium` uniform are in `medium.wgsl` and
// `source.wgsl`, which the composite does not take.
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
// table's ground offset is a deliberate change from sebh's code (GROUND_OFFSET_M, below). The
// transmittance table holds optical depth and the absorbers' columns, not Bevy's transmittance,
// and each light's curves of growth are added on read (plan R08, Design notes 5 and 8, R08.T6.c).

// The most terms a medium may have; `tables.ts` holds the same number and refuses more.
const MAX_TERMS: u32 = 8u;

// The most absorber slots a medium has (plan R08, Design note 8, R08.T6.c): the transmittance
// table's alpha and the absorber-column table's four channels, `tables.ts`'s ABSORBER_SLOTS and
// `absorbers.ts`'s ABSORBER_COLUMNS. A light's curves take this many rows of `absorberCurves`.
const ABSORBER_SLOTS: u32 = 5u;

// One term (plan R08, R08.T6.b): per-channel coefficients at unit density, m^-1, in the rgb of
// `scattering` and `absorption`, its density profile and its phase function. The profile:
// profile.x 0 for exponential (y the scale height, m), 1 for a tent (y bottom, z peak, w top, m),
// 2 for tabulated (y the top of its table, m, read from the term's layer of `densityTables`,
// `medium.wgsl`). The phase: phase.w 0 none, 1 Rayleigh with its depolarisation ratio per channel
// in xyz (R08.T3.c), 2 Cornette–Shanks with its g in x, 3 tabulated per channel (the term's layer
// of `phaseTables`, `source.wgsl`). A term's layer in either table is its index in `terms`. An
// absorber (R08.T6.c, Design notes 5 and 8), whose rgb coefficients are 0 since it absorbs by its
// curves of growth, carries its slot in absorption.w, 1 to ABSORBER_SLOTS for slots 0 to 4 and 0
// for every other term, and in scattering.w n / U, m^-1: its number density at relative density
// 1 over its vertical column, so that the density times it, integrated along a path, is the
// path's column over the vertical one, u / U.
struct Term {
  scattering : vec4f,
  absorption : vec4f,
  profile : vec4f,
  phase : vec4f,
}

// The medium's shell, its term count, a kernel's steps along each ray, the ground's albedo and its
// absorber slots: R05's `Medium` uniform without its terms, which travel in a storage buffer of
// MAX_TERMS terms (R08.T6.b). The shell is its ground's radius and the atmosphere's height above
// it, formed in f64 (R08.T6.g), so that no kernel forms the height as the top's radius less the
// ground's. absorbers.x is how many of its terms are absorbers, 0 to ABSORBER_SLOTS, and
// absorbers.y the light slot whose curves of growth a per-planet table is built with, its
// `curveSlot` (R08.T6.c; 0 until R08.T7 builds one multiple-scattering table a light slot,
// `decision-r08-t6b-floor.md` section 8); z and w are unused.
struct Medium {
  bottomRadiusM : f32,
  topHeightM : f32,
  termCount : f32,
  samples : f32,
  groundAlbedo : vec4f,
  absorbers : vec4f,
}

// A path's absorber columns, each slot's u / U, the column crossed over the absorber's own vertical
// column (plan R08, Design notes 5 and 8, R08.T6.c): slot 0 in `first`, which the transmittance
// table keeps in its alpha, and slots 1 to 4 in `more`, which the absorber-column table keeps in
// its rgba. As a medium's sample, the same per metre.
struct AbsorberColumns {
  first : f32,
  more : vec4f,
}

// The medium at a height: its scattering, its extinction and its absorption, m^-1, and its
// absorbers' columns per metre. The absorption is the terms' own absorption coefficients summed,
// never the extinction less the scattering, so that it is never negative by rounding (plan R08,
// R08.T6.d; `decision-r08-t6b-floor.md` section 9.3).
struct MediumSample {
  scattering : vec3f,
  extinction : vec3f,
  absorption : vec3f,
  columns : AbsorberColumns,
}

// A term's absorber columns per metre at relative density d: its n / U times d in its slot, and
// nothing for a term that is not an absorber.
fn termColumns(term : Term, d : f32) -> AbsorberColumns {
  let slot = term.absorption.w;
  let column = term.scattering.w * d;
  let more = abs(vec4f(slot) - vec4f(2.0, 3.0, 4.0, 5.0)) < vec4f(0.5);
  return AbsorberColumns(
    select(0.0, column, abs(slot - 1.0) < 0.5),
    select(vec4f(0.0), vec4f(column), more),
  );
}

// a + k b, slot by slot.
fn addColumns(a : AbsorberColumns, b : AbsorberColumns, k : f32) -> AbsorberColumns {
  return AbsorberColumns(a.first + k * b.first, a.more + k * b.more);
}

// --- Curves of growth (plan R08, Design notes 5 and 8, R08.T6.c) ----------------------------------
// A light's curves of growth stand in `absorberCurves` (rgba32float, `tables.ts`): row
// slot * ABSORBER_SLOTS + k is light slot `slot`'s curve for absorber slot k, its texel j the
// optical depth -ln T_c per channel (rgb) at node j, u / U = 10^(-3 + j / 40), the grid of
// `absorbers.ts`'s ABSORBER_CURVE_DECADES and ABSORBER_CURVE_NODES_PER_DECADE. `tables.ts` holds
// the same constants and refuses a curve on another grid.

// ln(u / U) at node 0: ln 10^-3.
const CURVE_FIRST_LN : f32 = -6.907755278982137;
// The nodes per unit of ln u: 40 / ln 10.
const CURVE_NODES_PER_LN : f32 = 17.37177927613007;
// The least transmittance a curve holds, 10^-6 (`absorbers.ts`'s ABSORBER_TRANSMITTANCE_FLOOR), as
// an optical depth: -ln 10^-6.
const CURVE_DEPTH_MAX : f32 = 13.815510557964274;

// -ln T_c per channel through a path's column `x`, as u / U, from row `row` of `curves`: the read
// rule of `absorbers.ts`'s absorberTransmittance. Linear in ln u on a log-log scale between the
// nodes about it, linear between them where either is 0; proportional to u below the first node,
// the linear regime; the last interval's power law beyond the last node; clamped to
// [0, -ln 10^-6] as the curve is. 0 for no column.
fn curveDepth(curves : texture_2d<f32>, row : u32, x : f32) -> vec3f {
  if (x <= 0.0) {
    return vec3f(0.0);
  }
  let nodes = textureDimensions(curves).x;
  // The column's place on the grid, in nodes from node 0.
  let node = (log(x) - CURVE_FIRST_LN) * CURVE_NODES_PER_LN;
  var depth : vec3f;
  if (node <= 0.0) {
    // exp(ln u - ln u_0) = (u / U) / 10^-3.
    depth = textureLoad(curves, vec2u(0u, row), 0).rgb * (x * 1000.0);
  } else {
    let k = min(u32(node), nodes - 2u);
    let t = node - f32(k);
    let a = textureLoad(curves, vec2u(k, row), 0).rgb;
    let b = textureLoad(curves, vec2u(k + 1u, row), 0).rgb;
    let logLog = exp(log(a) + (log(b) - log(a)) * t);
    depth = select(a + (b - a) * t, logLog, (a > vec3f(0.0)) & (b > vec3f(0.0)));
  }
  return clamp(depth, vec3f(0.0), vec3f(CURVE_DEPTH_MAX));
}

// The absorbers' optical depth per channel through a path's columns under light slot `slot`'s
// curves, summed over the medium's `count` absorber slots.
fn absorberDepth(
  curves : texture_2d<f32>,
  count : u32,
  slot : u32,
  columns : AbsorberColumns,
) -> vec3f {
  var depth = vec3f(0.0);
  if (count == 0u) {
    return depth;
  }
  let base = slot * ABSORBER_SLOTS;
  depth += curveDepth(curves, base, columns.first);
  for (var k = 1u; k < min(count, ABSORBER_SLOTS); k++) {
    depth += curveDepth(curves, base + k, columns.more[k - 1u]);
  }
  return depth;
}

// A march's absorber path: the columns crossed from the ray's start, and the optical depth the
// light's curves give them. A curve of growth is not exponential in u, so a step's share is not
// its own column's depth but how much it deepens the whole path's: Design note 5's rule, the curve
// applied to the path's column.
struct AbsorberPath {
  columns : AbsorberColumns,
  depth : vec3f,
}

fn absorberPathStart() -> AbsorberPath {
  return AbsorberPath(AbsorberColumns(0.0, vec4f(0.0)), vec3f(0.0));
}

// The path one step longer: its columns grown by `local`, per metre, over `dtM`, and its depth
// under light slot `slot`'s curves there. The step's optical depth is the new depth less the
// path's, never negative since every curve is non-decreasing in u. A medium with no absorber keeps
// the path as it was.
fn absorberStep(
  curves : texture_2d<f32>,
  count : u32,
  slot : u32,
  path : AbsorberPath,
  local : AbsorberColumns,
  dtM : f32,
) -> AbsorberPath {
  if (count == 0u) {
    return path;
  }
  let columns = addColumns(path.columns, local, dtM);
  return AbsorberPath(columns, absorberDepth(curves, count, slot, columns));
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

// Where the line of a ray from `heightM` at zenith cosine mu crosses the sphere `edgeM` above the
// ground: its two signed distances, nearer first, which a caller keeps where they lie ahead of the
// ray, or -1 for both where the line misses that sphere or starts on it tangent to it (plan R08,
// R08.T6.d; the layer edges of `decision-r08-t6b-floor.md` section 9.2). They are the roots of
// t^2 + 2 r mu t + C = 0 with C = r^2 - (bottom + e)^2 = (h - e)(2 bottom + h + e), formed from
// heights, and taken as q = -(r mu + sign(r mu) sqrt((r mu)^2 - C)) and C / q, neither of which
// cancels (Press et al., Numerical Recipes, 3rd ed., section 5.6), never as a difference of radii.
fn shellCrossings(bottom : f32, heightM : f32, mu : f32, edgeM : f32) -> vec2f {
  let rMu = (bottom + heightM) * mu;
  let c = (heightM - edgeM) * (2.0 * bottom + heightM + edgeM);
  let disc = rMu * rMu - c;
  if (disc < 0.0) {
    return vec2f(-1.0);
  }
  let q = -(rMu + select(-1.0, 1.0, rMu >= 0.0) * sqrt(disc));
  if (q == 0.0) {
    return vec2f(-1.0);
  }
  let other = c / q;
  return vec2f(min(q, other), max(q, other));
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

// The transmittance at the transmittance table's unit range (`transmittanceUnitAt`) for light slot
// `slot` (plan R08, Design notes 5 and 8, R08.T6.c): the stored optical depth (rgb) and slot 0's
// column (alpha) read bilinearly, the absorber-column table's slots 1 to 4 likewise where the
// medium has them, the light's curves of growth added to the optical depth, and the sum
// exponentiated in f32, so that no light's transmittance is ever stored in rgba16float on its way
// to a lit point or the disc. The read sites keep R08.T6.g's height forms and its texel mapping.
fn transmittanceThroughTables(
  depthTable : texture_2d<f32>,
  columnTable : texture_2d<f32>,
  curves : texture_2d<f32>,
  count : u32,
  slot : u32,
  unit : vec2f,
) -> vec3f {
  let stored = tableAtUnit(depthTable, unit);
  var columns = AbsorberColumns(stored.a, vec4f(0.0));
  if (count > 1u) {
    columns.more = tableAtUnit(columnTable, unit);
  }
  return exp(-(stored.rgb + absorberDepth(curves, count, slot, columns)));
}

// --- The steps of a march (R05.T12.e; moved here from `source.wgsl` in plan R08, R08.T6.d, so that
// the multiple-scattering kernel, which does not take `source.wgsl`, places its steps by them) ---

// A ray's segment [tStart, tEnd] split at its lowest point t*, the point nearest the centre of the
// sphere the ray is marched about, with the steps each side takes (R05.T12.e, with addendum A of
// decision-r05-high-atmosphere.md; `marchSteps.ts` is its TypeScript twin). Even steps
// under-sample the dense air at that point (the camera, the ground or a limb's tangent point),
// whose aerosol has a 1.2 km scale height (plan R05, Risks, "The per-frame marches under-sample the
// dense air"), so each side is placed quadratically toward t*. The camera side of a two-sided
// segment that starts at a camera inside the atmosphere is placed toward the camera instead: where
// the camera's air is dense, on a ray that falls to a lowest point below the camera and climbs out,
// the view's own attenuation puts that side's light at the camera end. From 60-100 km it is not,
// and the high setting's 75 sky-view steps carry those rays (addendum B).
struct MarchSplit {
  startM : f32,
  // t* = clamp(-o.d, tStart, tEnd), m.
  lowestM : f32,
  // The lengths of the segment before and after t*, m.
  beforeM : f32,
  afterM : f32,
  stepsBefore : u32,
  stepsAfter : u32,
  // Whether the side before t* is placed toward the segment's start, the camera.
  beforeTowardStart : bool,
}

// Splits the segment at `nearestM`, the distance -o.d to the ray's closest approach to the centre.
// A two-sided segment shares its steps in proportion to the square root of each side's length,
// which gives each side the same smallest step L / n^2, at the end it is placed toward (t*, or the
// camera for a camera side), at least one a side; a one-sided segment gives all its steps to its
// side. `fromCamera`: the segment starts at a camera inside the atmosphere.
// `samples` is at least 2.
fn marchSplit(
  tStartM : f32,
  tEndM : f32,
  nearestM : f32,
  samples : u32,
  fromCamera : bool,
) -> MarchSplit {
  let lowest = clamp(nearestM, tStartM, tEndM);
  let before = lowest - tStartM;
  let after = tEndM - lowest;
  let rootBefore = sqrt(before);
  let total = rootBefore + sqrt(after);
  let share = select(0.0, floor(f32(samples) * rootBefore / total + 0.5), total > 0.0);
  let least = select(0u, 1u, before > 0.0);
  let most = samples - select(0u, 1u, after > 0.0);
  let stepsBefore = min(max(u32(share), least), most);
  let towardStart = fromCamera && before > 0.0 && after > 0.0;
  let stepsAfter = samples - stepsBefore;
  return MarchSplit(tStartM, lowest, before, after, stepsBefore, stepsAfter, towardStart);
}

// One step of a march: its midpoint's distance along the ray, its length, and its midpoint's
// signed distance from the split's lowest point t*, formed without the difference of the two
// distances (plan R08, Design note 18: the ray march's closest-approach form), m.
struct MarchStep {
  tM : f32,
  dtM : f32,
  fromLowestM : f32,
}

// Step `i` of a split segment, counted from tStart. A side of length L and n steps placed toward
// an end a has its boundaries at a -/+ L (k / n)^2, k = 0..n, so its step k from a has its midpoint
// at a -/+ L (k^2 + k + 1/2) / n^2 and the length L (2k + 1) / n^2.
fn marchStep(split : MarchSplit, i : u32) -> MarchStep {
  if (i < split.stepsBefore) {
    let n = f32(split.stepsBefore);
    let unitM = split.beforeM / (n * n);
    if (split.beforeTowardStart) {
      let k = f32(i);
      let fromStart = unitM * (k * k + k + 0.5);
      return MarchStep(split.startM + fromStart, unitM * (2.0 * k + 1.0), fromStart - split.beforeM);
    }
    let k = f32(split.stepsBefore - 1u - i);
    let fromLowest = unitM * (k * k + k + 0.5);
    return MarchStep(split.lowestM - fromLowest, unitM * (2.0 * k + 1.0), -fromLowest);
  }
  let n = f32(split.stepsAfter);
  let k = f32(i - split.stepsBefore);
  let unitM = split.afterM / (n * n);
  let fromLowest = unitM * (k * k + k + 0.5);
  return MarchStep(split.lowestM + fromLowest, unitM * (2.0 * k + 1.0), fromLowest);
}

// The atmosphere's shared WGSL (plan R05, Design note 16): the medium as a list of terms, the
// spherical shell's geometry from a height, the tables' parameterisations on a sphere of given
// radius (plan R08, Design notes 18 and 19), the steps of a march (R05.T12.e) and their cut at the
// medium's layer edges (`MarchWalk`, plan R08, R08.T6.i). Prepended, after
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
// `decision-r08-t6b-floor.md` section 8); z is 1 where `densityTables` holds the column's pressure
// profile at the layer past the terms' (R08.T4.d; `medium.wgsl`'s `pressureBarAt`), 0 otherwise;
// w is the medium's line slots, a bitmask of the absorber slots whose curves have pressure rows
// (bit k for slot k; `pressureChannelOf`).
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

// The medium at a height: its scattering, its extinction and its absorption, m^-1, its absorbers'
// columns per metre, and the same times p / ABSORBER_PRESSURE_REFERENCE_PA, their pressure-weighted
// columns per metre (R08.T4.d). The absorption is the terms' own absorption coefficients summed,
// never the extinction less the scattering, so that it is never negative by rounding (plan R08,
// R08.T6.d; `decision-r08-t6b-floor.md` section 9.3).
struct MediumSample {
  scattering : vec3f,
  extinction : vec3f,
  absorption : vec3f,
  columns : AbsorberColumns,
  pressureColumns : AbsorberColumns,
}

// The pressure the pressure-weighted columns are taken over, Pa (R08.T4.d): a path's
// Curtis-Godson pressure (Curtis 1952, QJRMS 78, 638; Godson 1953, QJRMS 79, 367) is
// p-bar = ABSORBER_PRESSURE_REFERENCE_PA * w / x, w its pressure-weighted column and x its column,
// both over the absorber's vertical column. A Venus-class grazing chord's w, about 3e3, stays under
// rgba16float's 65504; a high, thin path's is subnormal or 0 (`medium.ts`'s
// ABSORBER_PRESSURE_REFERENCE_PA, held equal by a test).
const ABSORBER_PRESSURE_REFERENCE_PA : f32 = 100000.0;

// The least p-bar a path is read at, Pa, far under any curve's first row, so that a
// pressure-weighted column stored as 0 reads at that row rather than at log(0) (`medium.ts`'s
// ABSORBER_PRESSURE_FLOOR_PA, held equal by a test).
const ABSORBER_PRESSURE_FLOOR_PA : f32 = 0.001;

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

// Slot or pressure channel k of a set of columns: `first` for 0, `more` for 1 to 4.
fn columnAt(c : AbsorberColumns, k : u32) -> f32 {
  if (k == 0u) {
    return c.first;
  }
  return c.more[k - 1u];
}

// The pressure channel of absorber slot k among the line slots `lines`, the bitmask of the slots
// whose curves have pressure rows (R08.T4.d; `decision-r08-t4d-kdist-chain.md` section 15): its
// rank among them, in slot order, or -1 for a slot without lines, which reads no pressure. The
// channels stand in AbsorberColumns as the slots do: 0 in `first`, 1 to 4 in `more`.
fn pressureChannelOf(lines : u32, k : u32) -> i32 {
  if (((lines >> k) & 1u) == 0u) {
    return -1;
  }
  return i32(countOneBits(lines & ((1u << k) - 1u)));
}

// A sample's pressure-weighted columns per metre by channel: each line slot's column per metre
// times p / ABSORBER_PRESSURE_REFERENCE_PA (`pressureBar`) in its channel, 0 in the rest.
fn pressureColumnsOf(columns : AbsorberColumns, lines : u32, pressureBar : f32) -> AbsorberColumns {
  var out = AbsorberColumns(0.0, vec4f(0.0));
  for (var k = 0u; k < ABSORBER_SLOTS; k++) {
    let c = pressureChannelOf(lines, k);
    if (c == 0) {
      out.first = columnAt(columns, k) * pressureBar;
    } else if (c > 0) {
      out.more[u32(c) - 1u] = columnAt(columns, k) * pressureBar;
    }
  }
  return out;
}

// --- Curves of growth (plan R08, Design notes 5 and 8, R08.T6.c; pressure rows, R08.T4.d) --------
// A light's curves of growth stand in `absorberCurves` (rgba32float, `tables.ts`'s
// packAbsorberCurves), each a block of pressure rows, one row for an absorber without lines:
// - row 0 is a header: its texel k, k < ABSORBER_SLOTS, absorber slot k's first row within a
//   light's block (r), its rows (g) and its read's top, ln(p / 1 Pa) (b); its texel ABSORBER_SLOTS
//   the rows one light's block takes (r);
// - light slot s's curve for absorber slot k at pressure row q is row 1 + s * block + first + q,
//   its texel j the optical depth -ln T_c per channel (rgb) at node j, u / U = 10^(-3 + j / 40),
//   the grid of `absorbers.ts`'s ABSORBER_CURVE_DECADES and ABSORBER_CURVE_NODES_PER_DECADE, and
//   its alpha the row's ln(p / 1 Pa).
// `tables.ts` holds the same constants and refuses a curve on another grid.

// ln(u / U) at node 0: ln 10^-3.
const CURVE_FIRST_LN : f32 = -6.907755278982137;
// The nodes per unit of ln u: 40 / ln 10.
const CURVE_NODES_PER_LN : f32 = 17.37177927613007;
// The least transmittance a curve holds, 10^-6 (`absorbers.ts`'s ABSORBER_TRANSMITTANCE_FLOOR), as
// an optical depth: -ln 10^-6.
const CURVE_DEPTH_MAX : f32 = 13.815510557964274;

// -ln T_c per channel through a path's column `x`, as u / U, from row `row` of `curves`, unclamped:
// `absorbers.ts`'s depthAt. Linear in ln u on a log-log scale between the nodes about it, linear
// between them where either is 0; proportional to u below the first node, the linear regime; the
// last interval's power law beyond the last node.
fn curveRowDepth(curves : texture_2d<f32>, row : u32, x : f32) -> vec3f {
  let nodes = textureDimensions(curves).x;
  // The column's place on the grid, in nodes from node 0.
  let node = (log(x) - CURVE_FIRST_LN) * CURVE_NODES_PER_LN;
  if (node <= 0.0) {
    // exp(ln u - ln u_0) = (u / U) / 10^-3.
    return textureLoad(curves, vec2u(0u, row), 0).rgb * (x * 1000.0);
  }
  let k = min(u32(node), nodes - 2u);
  let t = node - f32(k);
  let a = textureLoad(curves, vec2u(k, row), 0).rgb;
  let b = textureLoad(curves, vec2u(k + 1u, row), 0).rgb;
  let logLog = exp(log(a) + (log(b) - log(a)) * t);
  return select(a + (b - a) * t, logLog, (a > vec3f(0.0)) & (b > vec3f(0.0)));
}

// A row's ln(p / 1 Pa), in its first texel's alpha.
fn curveRowLogPressure(curves : texture_2d<f32>, row : u32) -> f32 {
  return textureLoad(curves, vec2u(0u, row), 0).a;
}

// -ln T_c per channel through a path's column `x`, as u / U, at its absorber-weighted pressure
// ln(p-bar / 1 Pa) `lnP`, from light slot `light`'s curve for absorber slot `k`: the read rule of
// `absorbers.ts`'s absorberTransmittance (its depthAtPressure, `decision-r08-t4d-kdist-chain.md`
// ruling 3 and section 8). Each row's depth (`curveRowDepth`), then 4-point Lagrange in ln p-bar
// on the depth over the four rows nearest, its weights from the rows' own ln p-bar, unclamped
// between them; linear between the two rows about p-bar on a curve of under four rows; p-bar held
// at the first row below and at the read's top above; then clamped to [0, -ln 10^-6] as the curve
// is. One row, for an absorber without lines, is read alone. 0 for no column.
fn curveDepth(curves : texture_2d<f32>, light : u32, k : u32, x : f32, lnP : f32) -> vec3f {
  if (x <= 0.0) {
    return vec3f(0.0);
  }
  let header = textureLoad(curves, vec2u(k, 0u), 0);
  let block = u32(textureLoad(curves, vec2u(ABSORBER_SLOTS, 0u), 0).r);
  let first = 1u + light * block + u32(header.r);
  let rows = u32(header.g);
  var depth : vec3f;
  if (rows <= 1u) {
    depth = curveRowDepth(curves, first, x);
  } else {
    let lowest = curveRowLogPressure(curves, first);
    let top = min(curveRowLogPressure(curves, first + rows - 1u), header.b);
    let y = clamp(lnP, lowest, top);
    var r = 0u;
    while (r < rows - 2u && curveRowLogPressure(curves, first + r + 1u) < y) {
      r++;
    }
    if (rows < 4u) {
      let lo = curveRowLogPressure(curves, first + r);
      let hi = curveRowLogPressure(curves, first + r + 1u);
      let a = curveRowDepth(curves, first + r, x);
      let b = curveRowDepth(curves, first + r + 1u, x);
      depth = a + (b - a) * ((y - lo) / (hi - lo));
    } else {
      let s0 = min(max(r, 1u) - 1u, rows - 4u);
      var ys : array<f32, 4>;
      for (var m = 0u; m < 4u; m++) {
        ys[m] = curveRowLogPressure(curves, first + s0 + m);
      }
      depth = vec3f(0.0);
      for (var m = 0u; m < 4u; m++) {
        var w = 1.0;
        for (var n = 0u; n < 4u; n++) {
          if (n != m) {
            w *= (y - ys[n]) / (ys[m] - ys[n]);
          }
        }
        depth += w * curveRowDepth(curves, first + s0 + m, x);
      }
    }
  }
  return clamp(depth, vec3f(0.0), vec3f(CURVE_DEPTH_MAX));
}

// ln(p-bar / 1 Pa) of a path's slot from its column `x` and pressure-weighted column `w`, both
// over the absorber's vertical column (ABSORBER_PRESSURE_REFERENCE_PA), p-bar at least
// ABSORBER_PRESSURE_FLOOR_PA; read only where x > 0.
fn pathLogPressure(x : f32, w : f32) -> f32 {
  return log(max(ABSORBER_PRESSURE_REFERENCE_PA * w / x, ABSORBER_PRESSURE_FLOOR_PA));
}

// The absorbers' optical depth per channel through a path's columns, slot by slot, and its
// pressure-weighted columns, channel by channel among the line slots `lines`
// (`pressureChannelOf`), under light slot `slot`'s curves, summed over the medium's `count`
// absorber slots. A slot without lines reads no pressure.
fn absorberDepth(
  curves : texture_2d<f32>,
  count : u32,
  slot : u32,
  lines : u32,
  columns : AbsorberColumns,
  pressureColumns : AbsorberColumns,
) -> vec3f {
  var depth = vec3f(0.0);
  for (var k = 0u; k < min(count, ABSORBER_SLOTS); k++) {
    let x = columnAt(columns, k);
    let c = pressureChannelOf(lines, k);
    var lnP = 0.0;
    if (c >= 0) {
      lnP = pathLogPressure(x, columnAt(pressureColumns, u32(c)));
    }
    depth += curveDepth(curves, slot, k, x, lnP);
  }
  return depth;
}

// A march's absorber path: the columns crossed from the ray's start, and the optical depth the
// light's curves give them. A curve of growth is not exponential in u, so a step's share is not
// its own column's depth but how much it deepens the whole path's: Design note 5's rule, the curve
// applied to the path's column.
struct AbsorberPath {
  columns : AbsorberColumns,
  pressureColumns : AbsorberColumns,
  depth : vec3f,
}

fn absorberPathStart() -> AbsorberPath {
  let none = AbsorberColumns(0.0, vec4f(0.0));
  return AbsorberPath(none, none, vec3f(0.0));
}

// The path one step longer: its columns and pressure-weighted columns grown by `local` and
// `localPressureColumns`, per metre, over `dtM`, and its depth under light slot `slot`'s curves
// there, the line slots `lines` reading their channels' pressures. The step's optical depth is the
// new depth less the path's, never negative for a curve without pressure rows, which is
// non-decreasing in u; one with lines may fall a little where the path's p-bar moves (R08.T4.d's
// record). A medium with no absorber keeps the path as it was.
fn absorberStep(
  curves : texture_2d<f32>,
  count : u32,
  slot : u32,
  lines : u32,
  path : AbsorberPath,
  local : AbsorberColumns,
  localPressureColumns : AbsorberColumns,
  dtM : f32,
) -> AbsorberPath {
  if (count == 0u) {
    return path;
  }
  let columns = addColumns(path.columns, local, dtM);
  let pressureColumns = addColumns(path.pressureColumns, localPressureColumns, dtM);
  return AbsorberPath(
    columns, pressureColumns, absorberDepth(curves, count, slot, lines, columns, pressureColumns)
  );
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
// medium has them, and the line slots' pressure-weighted columns beside them by pressure channel
// (R08.T4.d, `pressureChannelOf` of `lines`: channels 0 to 3 in `pressureColumnTable`'s rgba,
// channel 4 in `lastPressureColumnTable`'s r), the light's curves of growth added to the optical
// depth at each slot's p-bar, and the sum exponentiated in f32, so
// that no light's transmittance is ever stored in rgba16float on its way to a lit point or the
// disc. The read sites keep R08.T6.g's height forms and its texel mapping.
fn transmittanceThroughTables(
  depthTable : texture_2d<f32>,
  columnTable : texture_2d<f32>,
  pressureColumnTable : texture_2d<f32>,
  lastPressureColumnTable : texture_2d<f32>,
  curves : texture_2d<f32>,
  count : u32,
  slot : u32,
  lines : u32,
  unit : vec2f,
) -> vec3f {
  let stored = tableAtUnit(depthTable, unit);
  var columns = AbsorberColumns(stored.a, vec4f(0.0));
  var pressureColumns = AbsorberColumns(0.0, vec4f(0.0));
  if (count > 0u) {
    let firstFour = tableAtUnit(pressureColumnTable, unit);
    pressureColumns = AbsorberColumns(
      firstFour.x,
      vec4f(firstFour.yzw, tableAtUnit(lastPressureColumnTable, unit).x),
    );
  }
  if (count > 1u) {
    columns.more = tableAtUnit(columnTable, unit);
  }
  return exp(-(stored.rgb + absorberDepth(curves, count, slot, lines, columns, pressureColumns)));
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

// --- The layer edges (plan R08, R08.T6.d and R08.T6.i) -------------------------------------------

// The most edges the per-frame marches cut at: one a ramp, two a term (R08.T6.i).
const MARCH_EDGES_MAX : u32 = 2u * MAX_TERMS;

// The medium's layer edges, heights above the ground, m (`tables.ts`'s packLayerEdges; moved here
// from `transmittance.wgsl` in plan R08's R08.T6.i, since the sky view, the ray march and the
// multiple-scattering kernel read it too). `heights`, the transmittance kernel's (R08.T6.d): term
// i's in heights[i], where its density table leaves 0 and where each ramp ends, -1 where unused.
// `middles`, the per-frame marches' (R08.T6.i): each ramp as one edge at its middle height, at
// most two a term, those strictly inside the atmosphere, the first `count.x` of MARCH_EDGES_MAX in
// order and -1 past them. A ramp of 10-300 m inside a step of tens of kilometres errs by about its
// half-width's column, so the marches cut at its middle alone (`decision-r08-t6d-findings.md`
// section 1.3). `count.yzw` are unused.
struct LayerEdges {
  heights : array<vec4f, MAX_TERMS>,
  middles : array<vec4f, MARCH_EDGES_MAX / 4u>,
  count : vec4f,
}

// --- The steps of a march cut at layer edges (plan R08, R08.T6.i; `decision-r08-t6d-findings.md`
// section 1) ------------------------------------------------------------------------------------
// R05.T12.e's placed steps take a layer's edge to first order: a step whose midpoint falls just
// below a deck's or a slab's sharp base misses up to half a step of its densest air, and one just
// above counts that density over the whole step (T6.d's slab Earth missed by 21.3% at the limb).
// So each side of R05.T12.e's split is cut into pieces where the ray crosses an edge, and each
// piece is placed by R05.T12.e's rule on its own, at the same counts: no sample is added.

// The most places a segment crosses the marches' edges: each edge's shell twice.
const MARCH_CROSSINGS_MAX : u32 = 2u * MARCH_EDGES_MAX;

// A split segment cut into pieces at its crossings, and the walk of its steps through them
// (`marchSteps.ts`'s marchPieces and marchPiecesSteps are its twin). Each side of the split is cut
// at the crossings inside it. Its steps are shared among its pieces in proportion to the square
// root of each piece's length, which gives each piece the same smallest step at the end it is
// placed toward, as R05.T12.e shares a segment's between its sides, at least one a piece; each
// piece's count rounds its end's share of the rest, so that they sum to the side's, as
// `transmittance.wgsl`'s rayToTop rounds its pieces'. Each piece is placed quadratically toward
// its end nearer t*, which for a layer is its dense edge and for the air t*; on a camera side
// placed toward the camera, toward its end nearer the camera. A side that crosses no edge, or has
// more pieces than steps, keeps R05.T12.e's placement (`marchStep`), so that a segment that crosses
// no edge is R05.T12.e's to the bit.
struct MarchWalk {
  split : MarchSplit,
  // Each crossing's signed distance from t*, m, ascending, each once: those before t* first.
  fromLowestM : array<f32, MARCH_CROSSINGS_MAX>,
  crossings : u32,
  crossingsBefore : u32,
  // Each piece's steps, the side before t*'s from 0 and the side after it's from `firstAfter`
  // (exact integers in f32); while the walk is made ready, each piece's root length.
  shares : array<f32, MARCH_CROSSINGS_MAX + 2u>,
  firstAfter : u32,
  // Whether each side is cut into pieces.
  piecedBefore : bool,
  piecedAfter : bool,
  // Where the walk stands: its piece and the steps it has taken there, and the piece's count, unit
  // L / n^2, the end it is placed toward from t*, and whether that is its lower end
  // (`marchWalkEnter`).
  piece : u32,
  taken : u32,
  pieceSteps : f32,
  unitM : f32,
  anchorM : f32,
  towardLow : bool,
}

// A walk over `split` with no crossings yet.
fn marchWalkStart(walk : ptr<function, MarchWalk>, split : MarchSplit) {
  (*walk).split = split;
  (*walk).crossings = 0u;
}

// Adds a crossing at signed distance `fromLowestM` from t*, kept, in order, where it lies strictly
// inside either side: neither at the segment's ends nor at t* itself.
fn marchWalkCross(walk : ptr<function, MarchWalk>, fromLowestM : f32) {
  let beforeM = (*walk).split.beforeM;
  let afterM = (*walk).split.afterM;
  let inside = (fromLowestM > -beforeM && fromLowestM < 0.0)
    || (fromLowestM > 0.0 && fromLowestM < afterM);
  let count = (*walk).crossings;
  if (!inside || count >= MARCH_CROSSINGS_MAX) {
    return;
  }
  var at = count;
  while (at > 0u && (*walk).fromLowestM[at - 1u] > fromLowestM) {
    (*walk).fromLowestM[at] = (*walk).fromLowestM[at - 1u];
    at--;
  }
  (*walk).fromLowestM[at] = fromLowestM;
  (*walk).crossings = count + 1u;
}

// One side's shares: its `steps` among the pieces between `startM` and `endM` from t* that the
// walk's crossings `first` to `first + count` cut, piece p's into shares[at + p]. One a piece, and
// the rest by the pieces' root lengths, rounded at each piece's end. False, leaving R05.T12.e's
// placement, for a side that crosses no edge or has more pieces than steps.
fn marchSideShares(
  walk : ptr<function, MarchWalk>,
  first : u32,
  count : u32,
  startM : f32,
  endM : f32,
  steps : u32,
  at : u32,
) -> bool {
  let pieces = count + 1u;
  if (count == 0u || pieces > steps) {
    return false;
  }
  var total = 0.0;
  var pieceStartM = startM;
  for (var p = 0u; p < pieces; p++) {
    var pieceEndM = endM;
    if (p < count) {
      pieceEndM = (*walk).fromLowestM[first + p];
    }
    let root = sqrt(pieceEndM - pieceStartM);
    (*walk).shares[at + p] = root;
    total += root;
    pieceStartM = pieceEndM;
  }
  let rest = steps - pieces;
  var spent = 0u;
  var cumulative = 0.0;
  for (var p = 0u; p < pieces; p++) {
    cumulative += (*walk).shares[at + p];
    var share = rest;
    if (p + 1u < pieces) {
      share = u32(floor(f32(rest) * cumulative / total + 0.5));
    }
    (*walk).shares[at + p] = f32(1u + share - spent);
    spent = share;
  }
  return true;
}

// Makes the walk ready once its crossings are in: each distance once, and each side's shares.
fn marchWalkReady(walk : ptr<function, MarchWalk>) {
  var n = 0u;
  for (var j = 0u; j < (*walk).crossings; j++) {
    let t = (*walk).fromLowestM[j];
    if (n == 0u || t > (*walk).fromLowestM[n - 1u]) {
      (*walk).fromLowestM[n] = t;
      n++;
    }
  }
  (*walk).crossings = n;
  var before = 0u;
  while (before < n && (*walk).fromLowestM[before] < 0.0) {
    before++;
  }
  let split = (*walk).split;
  (*walk).crossingsBefore = before;
  (*walk).firstAfter = before + 1u;
  (*walk).piecedBefore = marchSideShares(
    walk, 0u, before, -split.beforeM, 0.0, split.stepsBefore, 0u
  );
  (*walk).piecedAfter = marchSideShares(
    walk, before, n - before, 0.0, split.afterM, split.stepsAfter, before + 1u
  );
  // The first piece: the side before t*'s first where it is cut, entered now; otherwise the side
  // after it's, which its first step enters (`marchWalkStep`).
  (*walk).piece = 0u;
  (*walk).taken = 0u;
  if ((*walk).piecedBefore) {
    marchWalkEnter(walk, 0u, true);
  }
}

// Enters piece `piece` of the walk, on the side before t* or after it, and holds what its steps
// share: its count n, its unit L / n^2, and its end a from t*, the one it is placed toward, nearer
// t* or, on a camera side placed toward the camera, nearer the camera. The side before t* runs from
// -beforeM to 0 and the side after it from 0 to afterM, each cut at its crossings.
fn marchWalkEnter(walk : ptr<function, MarchWalk>, piece : u32, before : bool) {
  let split = (*walk).split;
  var lowM : f32;
  var highM : f32;
  if (before) {
    lowM = -split.beforeM;
    if (piece > 0u) {
      lowM = (*walk).fromLowestM[piece - 1u];
    }
    highM = 0.0;
    if (piece < (*walk).crossingsBefore) {
      highM = (*walk).fromLowestM[piece];
    }
  } else {
    let at = (*walk).crossingsBefore + piece - (*walk).firstAfter;
    lowM = 0.0;
    if (at > (*walk).crossingsBefore) {
      lowM = (*walk).fromLowestM[at - 1u];
    }
    highM = split.afterM;
    if (at < (*walk).crossings) {
      highM = (*walk).fromLowestM[at];
    }
  }
  let n = (*walk).shares[piece];
  let towardLow = !before || split.beforeTowardStart;
  (*walk).piece = piece;
  (*walk).taken = 0u;
  (*walk).pieceSteps = n;
  (*walk).unitM = (highM - lowM) / (n * n);
  (*walk).towardLow = towardLow;
  (*walk).anchorM = select(highM, lowM, towardLow);
}

// Step `i` of the walk's segment, counted from tStart, the steps taken in order from 0: on a side
// not cut into pieces, R05.T12.e's `marchStep`; on one that is, step k of its piece of n steps
// placed toward its end a, at a -/+ L (k^2 + k + 1/2) / n^2 and L (2k + 1) / n^2 long, as
// `marchStep` places a side, its offset from t* formed from the end's. Per step, as `marchStep`, a
// compare and the closed form; the piece's own terms are formed once, as the walk enters it.
fn marchWalkStep(walk : ptr<function, MarchWalk>, i : u32) -> MarchStep {
  let split = (*walk).split;
  let before = i < split.stepsBefore;
  if ((before && !(*walk).piecedBefore) || (!before && !(*walk).piecedAfter)) {
    return marchStep(split, i);
  }
  if (!before && (*walk).piece < (*walk).firstAfter) {
    marchWalkEnter(walk, (*walk).firstAfter, false);
  } else if (f32((*walk).taken) >= (*walk).pieceSteps) {
    marchWalkEnter(walk, (*walk).piece + 1u, before);
  }
  let j = f32((*walk).taken);
  (*walk).taken++;
  let unitM = (*walk).unitM;
  var k = j;
  var fromLowest : f32;
  if ((*walk).towardLow) {
    fromLowest = (*walk).anchorM + unitM * (k * k + k + 0.5);
  } else {
    k = (*walk).pieceSteps - 1.0 - j;
    fromLowest = (*walk).anchorM - unitM * (k * k + k + 0.5);
  }
  return MarchStep(split.lowestM + fromLowest, unitM * (2.0 * k + 1.0), fromLowest);
}

// Adds to the walk where a ray from `heightM` above a sphere of radius `bottom`, at zenith cosine
// mu, crosses the sphere `edgeM` above it (`shellCrossings`), as its signed distances from t*: the
// sky view's and the multiple-scattering kernel's crossings, whose segments start at the ray's
// origin. Each kernel reads its edges from its own `layerEdges` uniform, one at a time, never the
// struct by value (plan R08, R08.T6.b: a value indexed by a loop variable is copied into each
// invocation's memory).
fn marchWalkCrossSphere(
  walk : ptr<function, MarchWalk>,
  bottom : f32,
  heightM : f32,
  mu : f32,
  edgeM : f32,
) {
  let lowestM = (*walk).split.lowestM;
  let roots = shellCrossings(bottom, heightM, mu, edgeM);
  if (roots.x > 0.0) {
    marchWalkCross(walk, roots.x - lowestM);
  }
  if (roots.y > 0.0) {
    marchWalkCross(walk, roots.y - lowestM);
  }
}

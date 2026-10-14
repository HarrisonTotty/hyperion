// The transmittance table (plan R05, Design note 16; Hillaire 2020, section 5.2): for each (r, mu)
// of Bruneton's parameterisation, what light loses from radius r along zenith cosine mu to the top
// of the atmosphere, kept as optical depth (plan R08, Design note 8, R08.T6.c): rgb the optical
// depth per channel of every term but the absorbers, alpha absorber slot 0's column u / U, and the
// absorber-column table's rgba slots 1 to 4's. Each light's curves of growth turn the columns into
// optical depth on read (Design note 5; `common.wgsl`'s `transmittanceThroughLayer`). Stored as
// T, a slant path through a Venus-class atmosphere fell below rgba16float's least value; stored as
// tau, the half float's step is at most 2^-10 of tau, and the transmittance's relative error at
// most 2^-11 tau (4.9e-4 tau), within Design note 8's 1e-3 tau. Each stored value is held to the
// half float's largest, 65504 (IEEE 754-2008 binary16), so that a deeper path stores a finite
// value whose e^-tau is 0 in f32, never an infinity whose bilinear read at the table's edge,
// inf * 0, is NaN. Follows `common.wgsl` and `medium.wgsl`; reads the medium's terms and their
// density tables (R08.T6.b). Since R08.T6.f each build writes one layer of 2D array tables, a κ
// slice's on the sphere κ_k R_ref, or a latitude band's own in its gravity-scaled frame, every
// coefficient over s_b (`Medium.build`; Design note 17).
//
// Ported from Bevy 0.19.1's crates/bevy_pbr/src/atmosphere/transmittance_lut.wgsl (MIT or
// Apache-2.0; the notice is in common.wgsl), checked against sebh's UnrealEngineSkyAtmosphere
// (MIT, Copyright (c) 2020 Epic Games, Inc.). Changes: the medium's terms are evaluated directly,
// and the optical depth is the midpoint rule over `medium.samples` steps, where Bevy samples at 0.3
// of each step and drops the last step's remainder; the steps are equal on each piece of the ray
// between the places it crosses a term's layer edges, since plan R08's R08.T6.d (`rayToTop`), and
// were equal along the whole ray before. Each texel's point is taken
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
// Each table is a 2D array of one layer a κ slice (plan R08, R08.T6.f; Design note 17), or a
// latitude band's own one-layer table, the build writing layer `buildTransmittanceLayer()`.
@group(0) @binding(3) var transmittanceOut : texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(4) var absorberColumnsOut : texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(5) var<uniform> layerEdges : LayerEdges;
// The line slots' pressure-weighted columns by pressure channel (R08.T4.d; `common.wgsl`'s
// `pressureChannelOf`): channels 0 to 3 in rgba, channel 4 in the second table's r, each
// (p / ABSORBER_PRESSURE_REFERENCE_PA) (u / U) summed along the ray.
@group(0) @binding(6) var absorberPressureColumnsOut : texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(7) var lastAbsorberPressureColumnOut : texture_storage_2d_array<rgba16float, write>;

// The pressure channels one table holds, in its rgba (`tables.ts`'s PRESSURE_CHANNELS_A_TABLE).
const PRESSURE_CHANNELS_A_TABLE : u32 = 4u;

// rgba16float's largest finite value (IEEE 754-2008 binary16), to which every stored value is held.
const HALF_MAX : f32 = 65504.0;

// The terms' layer edges are `layerEdges.heights` (`common.wgsl`'s LayerEdges, R08.T6.d): term i's
// in heights[i], where its density table leaves 0 and where each ramp ends, -1 where unused.

// The most places a ray may cross the terms' layer edges: four edges a term, each crossed at most
// twice.
const MAX_CROSSINGS : u32 = 8u * MAX_TERMS;

// The fewest steps a piece of a ray between two crossings takes (`tables.ts`'s
// TRANSMITTANCE_STEPS_A_PIECE).
const STEPS_A_PIECE : u32 = 4u;

// What the table keeps of a ray from a texel's point to the top: the optical depth of every term
// but the absorbers, each absorber slot's column over its vertical one (R08.T6.c), and each line
// slot's pressure-weighted column by pressure channel (R08.T4.d), summed in the same inner loop.
struct RayToTop {
  depth : vec3f,
  columns : AbsorberColumns,
  pressureColumns : AbsorberColumns,
}

// The optical depth and the absorber columns from a texel's point along its zenith cosine to the
// top. The ray is split where it crosses the terms' layer edges (`shellCrossings`), so that no
// ramp of a layer, up to one column interval (about 115 m on a 250 K column), falls inside a step
// of up to 8.9 km, where the midpoint rule errs
// to first order (R08.T6.d; `decision-r08-t6b-floor.md` section 9.2). The `medium.samples` steps
// are spread over the pieces in proportion to their length, STEPS_A_PIECE a piece at least, by
// rounding each piece's end's share of the rest (so that they sum to the rest exactly), each piece
// at its own steps' midpoints. A ray that crosses no edge is R05's one piece of equal steps.
fn rayToTop(texel : TransmittanceTexel) -> RayToTop {
  let samples = max(u32(medium.samples), 1u);
  let bottom = medium.bottomRadiusM;
  let r = bottom + texel.heightM;
  let mu = texel.mu;
  let rho2 = texel.rho * texel.rho;
  let rMu = r * mu;
  let tMax = shellDistanceToTop(bottom, medium.topHeightM, texel.heightM, mu);
  // The crossings within (0, tMax), ascending, each distance once.
  var ends : array<f32, MAX_CROSSINGS + 1u>;
  var crossings = 0u;
  let termCount = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < termCount; i++) {
    let edges = layerEdges.heights[i];
    for (var k = 0u; k < 4u; k++) {
      let edgeM = edges[k];
      if (edgeM <= 0.0 || edgeM >= medium.topHeightM) {
        continue;
      }
      let roots = shellCrossings(bottom, texel.heightM, mu, edgeM);
      for (var j = 0u; j < 2u; j++) {
        let t = roots[j];
        if (t <= 0.0 || t >= tMax) {
          continue;
        }
        var at = crossings;
        while (at > 0u && ends[at - 1u] > t) {
          ends[at] = ends[at - 1u];
          at--;
        }
        ends[at] = t;
        crossings++;
      }
    }
  }
  // The pieces' ends, each distance once, the ray's own end last.
  var pieces = 0u;
  for (var j = 0u; j < crossings; j++) {
    if (pieces == 0u || ends[j] > ends[pieces - 1u]) {
      ends[pieces] = ends[j];
      pieces++;
    }
  }
  ends[pieces] = tMax;
  pieces++;
  let least = min(STEPS_A_PIECE, samples / pieces);
  let rest = samples - least * pieces;
  var depth = vec3f(0.0);
  var columns = AbsorberColumns(0.0, vec4f(0.0));
  var pressureColumns = AbsorberColumns(0.0, vec4f(0.0));
  var start = 0.0;
  var spent = 0u;
  for (var p = 0u; p < pieces; p++) {
    let end = ends[p];
    var share = rest;
    if (p + 1u < pieces) {
      share = u32(floor(f32(rest) * (end / tMax) + 0.5));
    }
    let steps = least + share - spent;
    spent = share;
    if (steps > 0u) {
      let dt = (end - start) / f32(steps);
      for (var i = 0u; i < steps; i++) {
        let t = start + (f32(i) + 0.5) * dt;
        let heightM = (t * t + 2.0 * rMu * t + rho2) / (localR(r, mu, t) + bottom);
        let local = mediumAtHeight(heightM);
        depth += local.extinction * dt;
        columns = addColumns(columns, local.columns, dt);
        pressureColumns = addColumns(pressureColumns, local.pressureColumns, dt);
      }
    }
    start = end;
  }
  return RayToTop(depth, columns, pressureColumns);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(transmittanceOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let sizeF = vec2f(size);
  let unit = clamp(unitFromTexelUv((vec2f(id.xy) + 0.5) / sizeF, sizeF), vec2f(0.0), vec2f(1.0));
  let ray = rayToTop(transmittanceTexelOf(unit));
  let layer = buildTransmittanceLayer();
  let stored = min(vec4f(ray.depth, ray.columns.first), vec4f(HALF_MAX));
  textureStore(transmittanceOut, id.xy, layer, stored);
  textureStore(absorberColumnsOut, id.xy, layer, min(ray.columns.more, vec4f(HALF_MAX)));
  let firstFour = vec4f(ray.pressureColumns.first, ray.pressureColumns.more.xyz);
  textureStore(absorberPressureColumnsOut, id.xy, layer, min(firstFour, vec4f(HALF_MAX)));
  // A fifth pressure channel, and its table at the transmittance's size, only for five line slots
  // (`tables.ts`'s lastPressureColumnSize); the table is otherwise one texel a layer, never read.
  if (countOneBits(lineSlots()) > PRESSURE_CHANNELS_A_TABLE) {
    let last = min(ray.pressureColumns.more.w, HALF_MAX);
    textureStore(lastAbsorberPressureColumnOut, id.xy, layer, vec4f(last, 0.0, 0.0, 0.0));
  }
}

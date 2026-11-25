// Decorative scatter's placement on the GPU (plan R11, R11.T6.a; Design notes 2, 3, 4 and 7): the
// pebbles below 0.4 m across of every drawn patch near enough to show them, from an integer lattice
// hash, the rocks' size–frequency law below the split, the patch's baked class weights and its
// slope, falling off with distance by pixel footprint, written into the particle path's instances
// (`clusters/particles.ts`' record) and counted for its instance cull and its indirect draws. Its
// reference is `pebbleLaw.ts` and `pebblePlace.ts`' `placePebblesCpu`, whose module remarks hold the
// design; its `f32` arithmetic is the device's, so the kernel is `presentation-only`.
//
// Composed after `fold.ts`' `folded_group`, `decoration.wgsl` (its lattice hash), `patchVertex.wgsl`
// (the slot record and its `FaceDifferences` arithmetic), the path's prelude and source (the
// vertex path's `ownOffset` and `morphOffset`, `pebblePlace.ts`' `pebbleKernel`) and
// `scatterCull.wgsl`.
//
// A workgroup an entry, folded: an entry is a block of up to 16 × 16 cells of one octave's lattice
// over one patch, its 64 threads each taking every 64th cell of it. A cell draws its Poisson count
// of proposals from word 0 of its hash, and proposal j its six words from 1 + 6 j: its place, its
// diameter on the proposals' law C ÷ D², its keep, its yaw and its tint (`pebbleLaw.ts`). A
// proposal is kept where its keep is below its ground's share of the law (the palette's slots'
// laws by their class shares, `pebble_acceptance`), times the slope's filter and the density, and
// drawn where its fade by pixel footprint rounds to a byte above 0 and it meets no body's box: at
// its whole radius, its fade's byte in its tag above its patch's index (`pebblePlace.ts`'
// `pebbleTag`), by which its draw takes it over to its ground's shading.

struct PebbleParams {
  // The rows of the body's rotation, body-fixed axes into the passes' frame (w unused).
  rotation0 : vec4f,
  rotation1 : vec4f,
  rotation2 : vec4f,
  // x the view's px per rad; y the scatter density; z and w the slope filter's cosines, full and
  // none (`PEBBLE_REPOSE_DEG`).
  view : vec4f,
  // x the entries, y the instances' capacity, z the body's decoration seed, w the bodies' boxes.
  counts : vec4u,
}

// One block of an octave's cells over a patch: the patch's index, the octave, its cells' levels
// below the patch (cells along its side 2^levels) and the block, 16 × 16 cells, row by row.
struct PebbleEntry {
  patchIndex : u32,
  octave : u32,
  cellLevels : u32,
  block : u32,
}

// A patch the scatter draws on (`pebblePlace.ts`' `SCATTER_PATCH_BYTES`), which the pebbles' draw
// reads by an instance's tag (`pebbleDraw.wgsl`): its origin less the camera, m, in the passes'
// frame, and its slot; its morph's start and end, m; its parent's slot and its quadrant there (the
// terrain's instance's); its key (face, level, i, j); its class sets (the terrain instance's
// `cells`).
struct ScatterPatch {
  originM : vec3f,
  slot : u32,
  morphStartM : f32,
  morphEndM : f32,
  parentSlot : u32,
  parentQuadrant : u32,
  key : vec4u,
  cells : vec4u,
}

const PEBBLE_GROUP : u32 = 64u;
const PEBBLE_BLOCK_SIDE : u32 = 16u;
const PEBBLE_BLOCK_CELLS : u32 = 256u;
// `pebbleLaw.ts`' PEBBLE_SALT, PEBBLE_WORDS_EACH, PEBBLE_MAX_PER_CELL and PEBBLE_FADE_PX.
const PEBBLE_SALT : u32 = 16u;
const PEBBLE_WORDS_EACH : u32 = 6u;
const PEBBLE_MAX_PER_CELL : u32 = 48u;
const PEBBLE_FADE_PX : f32 = 2.0;
// `pebblePlace.ts`' PEBBLE_TAG_PATCH_BITS.
const PEBBLE_TAG_PATCH_BITS : u32 = 24u;
// `pebbleLaw.ts`' PEBBLE_AXES: the ellipsoid's semi-axes over its radius.
const PEBBLE_AXES : vec3f = vec3f(1.1892071150027210, 0.8408964152537145, 0.75);
// Golombek and Rapp 1997's q(k) = 1.79 + 0.152 ÷ k per metre, and the law's largest k
// (`pebbleLaw.ts`' Q_CONSTANT_PER_M, Q_INVERSE_PER_M and MAX_ROCK_ABUNDANCE).
const Q_CONSTANT_PER_M : f32 = 1.79;
const Q_INVERSE_PER_M : f32 = 0.152;
const MAX_ROCK_ABUNDANCE : f32 = 0.4;
// `terrainLit.wgsl`'s palette slots and its class layers' words (`slotLayout.ts`).
const PEBBLE_CLASSES : u32 = 8u;
const PEBBLE_LAW_WORD : u32 = 8450u;
const PEBBLE_LAYER_WORDS : u32 = 13507u;
// A slot law's word that holds its rock abundance k (`terrainMaterial.ts`' `packClassLaws`).
const PEBBLE_ABUNDANCE_WORD : u32 = 7u;
const PEBBLE_TAU : f32 = 6.283185307179586;
const PEBBLE_QUANT : f32 = 4095.0;

@group(0) @binding(0) var<uniform> params : PebbleParams;
// Each octave's diameters, m, low then high, and its proposals per square metre.
@group(0) @binding(1) var<uniform> octaves : array<vec4f, 5>;
@group(0) @binding(2) var<uniform> bodies : array<ScatterBox, 8>;
@group(0) @binding(3) var<storage, read> entries : array<PebbleEntry>;
@group(0) @binding(4) var<storage, read> patches : array<ScatterPatch>;
@group(0) @binding(5) var<storage, read> slots : array<SlotRecord>;
@group(0) @binding(6) var<storage, read> class_layers : array<u32>;
@group(0) @binding(8) var<storage, read_write> instances : array<u32>;
@group(0) @binding(9) var<storage, read_write> instance_count : array<atomic<u32>>;

// Word `n` of the cell of hash `cell` (`pebbleLaw.ts`' `pebbleWord`).
fn pebble_word(cell : u32, n : u32) -> u32 {
  return decoration_mix(cell ^ decoration_mix(n));
}

// A word's top 24 bits as a number in [0, 1) (`pebbleUnit`).
fn pebble_unit(word : u32) -> f32 {
  return f32(word >> 8u) / 16777216.0;
}

// The Poisson count of `mean` at the uniform `u`, at most PEBBLE_MAX_PER_CELL (`poissonCount`).
fn pebble_count(mean : f32, u : f32) -> u32 {
  var term = exp(-mean);
  var total = term;
  var n = 0u;
  loop {
    if (u < total || n >= PEBBLE_MAX_PER_CELL) {
      break;
    }
    n = n + 1u;
    term = term * mean / f32(n);
    total = total + term;
  }
  return n;
}

// The diameter at the uniform `u` of an octave `range` (low, high), below high (`pebbleDiameter`).
fn pebble_diameter(range : vec4f, u : f32) -> f32 {
  let low = range.x;
  let high = range.y;
  let d = low * high / (high - u * (high - low));
  return min(d, bitcast<f32>(bitcast<u32>(high) - 1u));
}

// The two words of vertex (x, y) of `slot` as eight class weights in [0, 1].
fn pebble_weights(slot : u32, x : u32, y : u32, high : bool) -> vec4f {
  let at = slot * PEBBLE_LAYER_WORDS + 2u * (y * VERTICES_PER_SIDE + x) + select(0u, 1u, high);
  return unpack4x8unorm(class_layers[at]);
}

// The share of a proposal of diameter `d` kept at the place `at` (quads) of `slot`
// (`pebbleAcceptance` of `terrainShares`): each named slot's weight, interpolated bilinearly as
// `terrain_shares` takes it and scaled down where they sum past 1, times its law's
// (1.79 k + 0.152) e^(−q(k) d) over the proposals' (1.79 k_max + 0.152).
fn pebble_acceptance(slot : u32, at : vec2f, d : f32) -> f32 {
  let base = min(vec2u(floor(max(at, vec2f(0.0)))), vec2u(63u));
  let f = clamp(at - vec2f(base), vec2f(0.0), vec2f(1.0));
  var total = 0.0;
  var keep = 0.0;
  for (var half = 0u; half < 2u; half = half + 1u) {
    let high = half == 1u;
    let w = mix(
      mix(pebble_weights(slot, base.x, base.y, high), pebble_weights(slot, base.x + 1u, base.y, high), f.x),
      mix(pebble_weights(slot, base.x, base.y + 1u, high), pebble_weights(slot, base.x + 1u, base.y + 1u, high), f.x),
      f.y,
    );
    for (var c = 0u; c < 4u; c = c + 1u) {
      let law = slot * PEBBLE_LAYER_WORDS + PEBBLE_LAW_WORD + 8u * (4u * half + c);
      if (!(bitcast<f32>(class_layers[law + 4u]) >= 0.0)) {
        continue;
      }
      total = total + w[c];
      let k = min(bitcast<f32>(class_layers[law + PEBBLE_ABUNDANCE_WORD]), MAX_ROCK_ABUNDANCE);
      if (k > 0.0 && w[c] > 0.0) {
        let q = Q_CONSTANT_PER_M + Q_INVERSE_PER_M / k;
        keep = keep + w[c] * (Q_CONSTANT_PER_M * k + Q_INVERSE_PER_M) * exp(-q * d);
      }
    }
  }
  let top = Q_CONSTANT_PER_M * MAX_ROCK_ABUNDANCE + Q_INVERSE_PER_M;
  return clamp(keep / (top * max(total, 1.0)), 0.0, 1.0);
}

// The slope's filter at `cos_slope` (`slopeFilter`).
fn pebble_slope_filter(cos_slope : f32) -> f32 {
  return clamp((cos_slope - params.view.w) / (params.view.z - params.view.w), 0.0, 1.0);
}

// The datum's area of the quad holding the place `at` (quads) of the patch of `rec`, m²: the
// parallelogram of its corner's edges, M (d − d₀) at each, as `pebblePlace.ts`' `quadAreaM2`.
fn pebble_quad_area(rec : SlotRecord, at : vec2f) -> f32 {
  let q = min(vec2u(floor(max(at, vec2f(0.0)))), vec2u(63u));
  let m00 = rec.scale * directionDifference(rec, q.x, q.y);
  let m10 = rec.scale * directionDifference(rec, q.x + 1u, q.y);
  let m01 = rec.scale * directionDifference(rec, q.x, q.y + 1u);
  return length(cross(m10 - m00, m01 - m00));
}

// The unit quaternion (x, y, z, w) turning a pebble's axes onto the passes' frame: +z onto `up` by
// the shortest arc, after a turn of `yaw` about +z (`pebblePlace.ts`' `pebbleRotation`).
fn pebble_rotation(up : vec3f, yaw : f32) -> vec4f {
  var arc = vec4f(1.0, 0.0, 0.0, 0.0);
  if (1.0 + up.z > 1e-6) {
    arc = normalize(vec4f(-up.y, up.x, 0.0, 1.0 + up.z));
  }
  let turn = vec4f(0.0, 0.0, sin(0.5 * yaw), cos(0.5 * yaw));
  return vec4f(
    arc.w * turn.xyz + turn.w * arc.xyz + cross(arc.xyz, turn.xyz),
    arc.w * turn.w - dot(arc.xyz, turn.xyz),
  );
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(num_workgroups) groups : vec3u, @builtin(local_invocation_index) local : u32) {
  let n = folded_group(group, groups);
  if (n >= params.counts.x) {
    return;
  }
  let entry = entries[n];
  let tile = patches[entry.patchIndex];
  let rec = slots[tile.slot];
  let range = octaves[entry.octave];
  let side = 1u << entry.cellLevels;
  let blocks = max(side / PEBBLE_BLOCK_SIDE, 1u);
  let corner = vec2u(entry.block % blocks, entry.block / blocks) * PEBBLE_BLOCK_SIDE;
  let quads_each = 64.0 / f32(side);
  let rotation = mat3x3f(params.rotation0.xyz, params.rotation1.xyz, params.rotation2.xyz);
  let level = tile.key.y + entry.cellLevels;
  let prefix = decoration_prefix(params.counts.z, PEBBLE_SALT + entry.octave, tile.key.x, level);
  for (var c = local; c < PEBBLE_BLOCK_CELLS; c = c + PEBBLE_GROUP) {
    let cell = corner + vec2u(c % PEBBLE_BLOCK_SIDE, c / PEBBLE_BLOCK_SIDE);
    if (cell.x >= side || cell.y >= side) {
      continue;
    }
    let hash = decoration_point(prefix, tile.key.z * side + cell.x, tile.key.w * side + cell.y);
    let centre_at = (vec2f(cell) + vec2f(0.5)) * quads_each;
    let mean = range.z * pebble_quad_area(rec, centre_at) * quads_each * quads_each;
    let count = pebble_count(mean, pebble_unit(pebble_word(hash, 0u)));
    for (var j = 0u; j < count; j = j + 1u) {
      let w0 = 1u + PEBBLE_WORDS_EACH * j;
      let at = (vec2f(cell) + vec2f(pebble_unit(pebble_word(hash, w0)), pebble_unit(pebble_word(hash, w0 + 1u)))) * quads_each;
      let d = pebble_diameter(range, pebble_unit(pebble_word(hash, w0 + 2u)));
      let keep = pebble_unit(pebble_word(hash, w0 + 3u));
      // The ground's share first, the cheapest refusal.
      let share = pebble_acceptance(tile.slot, at, d) * params.view.y;
      if (!(keep < share)) {
        continue;
      }
      // The triangle of the patch's mesh holding the place (`terrainPatch.wgsl`'s `surfacePointAt`).
      let q = min(vec2u(floor(at)), vec2u(63u));
      let fq = at - vec2f(q);
      let mid = select(q + vec2u(0u, 1u), q + vec2u(1u, 0u), fq.x >= fq.y);
      let far = q + vec2u(1u);
      let b0 = 1.0 - max(fq.x, fq.y);
      let b1 = abs(fq.x - fq.y);
      let b2 = min(fq.x, fq.y);
      let a0 = ownOffset(tile.slot, rec, q.x, q.y);
      let a1 = ownOffset(tile.slot, rec, mid.x, mid.y);
      let a2 = ownOffset(tile.slot, rec, far.x, far.y);
      let own = b0 * a0 + b1 * a1 + b2 * a2;
      let morphed =
        b0 * morphOffset(tile.slot, rec, q.x, q.y) + b1 * morphOffset(tile.slot, rec, mid.x, mid.y) + b2 * morphOffset(tile.slot, rec, far.x, far.y);
      let distance_own = length(tile.originM + rotation * own);
      var k = 0.0;
      if (tile.morphEndM > tile.morphStartM) {
        k = clamp((distance_own - tile.morphStartM) / (tile.morphEndM - tile.morphStartM), 0.0, 1.0);
      }
      let ground = mix(own, morphed, k);
      let centre = tile.originM + rotation * ground;
      let fade = decoration_fade(d / PEBBLE_FADE_PX, length(centre), params.view.x);
      // `pebbleFadeByte`'s ⌊255 fade + ½⌋, not WGSL's `round`, which rounds a half to even.
      let fade_byte = u32(floor(clamp(fade, 0.0, 1.0) * 255.0 + 0.5));
      if (fade_byte == 0u) {
        continue;
      }
      let datum = normalize(rec.nu0 + normalDifference(rec, directionDifferenceAt(rec, at)));
      var normal = normalize(cross(a1 - a0, a2 - a0));
      if (dot(normal, datum) < 0.0) {
        normal = -normal;
      }
      if (!(keep < share * pebble_slope_filter(dot(normal, datum)))) {
        continue;
      }
      let radius = 0.5 * d;
      var met = false;
      for (var b = 0u; b < min(params.counts.w, 8u); b = b + 1u) {
        if (scatter_box_meets(bodies[b], centre, radius * PEBBLE_AXES.x)) {
          met = true;
          break;
        }
      }
      if (met) {
        continue;
      }
      let rotation_q = pebble_rotation(rotation * normal, PEBBLE_TAU * pebble_unit(pebble_word(hash, w0 + 4u)));
      let uv = clamp(at / 64.0, vec2f(0.0), vec2f(1.0));
      let colour = u32(round(uv.x * PEBBLE_QUANT)) | (u32(round(uv.y * PEBBLE_QUANT)) << 12u) |
        ((pebble_word(hash, w0 + 5u) >> 24u) << 24u);
      let index = atomicAdd(&instance_count[0], 1u);
      if (index >= params.counts.y) {
        continue;
      }
      let base = index * 8u;
      instances[base] = bitcast<u32>(centre.x);
      instances[base + 1u] = bitcast<u32>(centre.y);
      instances[base + 2u] = bitcast<u32>(centre.z);
      instances[base + 3u] = bitcast<u32>(radius);
      instances[base + 4u] = pack2x16snorm(rotation_q.xy);
      instances[base + 5u] = pack2x16snorm(rotation_q.zw);
      instances[base + 6u] = colour;
      instances[base + 7u] = entry.patchIndex | (fade_byte << PEBBLE_TAG_PATCH_BITS);
    }
  }
}

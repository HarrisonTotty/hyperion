// The instance path's test field (plan R14, R14.T5.d; `clusters/instanceScene.ts`): `counts.x`
// particles placed on the GPU as R11's placements will place theirs, a thread an instance, folded
// (`folded_group`, composed ahead of this source), and their count written for the instance cull.
// Each instance draws its numbers from its index alone (`pcg`), so that a field is the same whatever
// the dispatch's order: a direction uniform in the view's widened rectangle of tangents, a distance
// log-uniform between `ranges.x` and `ranges.y` (as many an octave of distance, as a placement by
// pixel footprint would have), a radius log-uniform between `ranges.z` and `ranges.w`, a uniform
// rotation (Shoemake 1992, "Uniform random rotations", Graphics Gems III), and a grey of rock
// albedo, 0.15 to 0.45, a little warm. Its `f32` arithmetic is the device's: the kernel is
// `presentation-only`, and a check reads its instances back.

struct PlaceParams {
  // The view's axes in the passes' frame (the view looks down −z), and the tangents of the
  // field's half-width and half-height.
  viewX : vec4f,
  viewY : vec4f,
  viewZ : vec4f,
  // Distance least and most, radius least and most, m.
  ranges : vec4f,
  // The instances placed, the path's capacity, the field's seed.
  counts : vec4u,
}

const PLACE_GROUP : u32 = 64u;
const TAU : f32 = 6.283185307179586;

@group(0) @binding(0) var<uniform> params : PlaceParams;
@group(0) @binding(1) var<storage, read_write> instances : array<u32>;
@group(0) @binding(2) var<storage, read_write> instance_count : array<u32>;

// The PCG hash's output of `v` (Jarzynski and Olano 2020, "Hash Functions for GPU Rendering", JCGT
// 9(3), its supplement's §18 `pcg`, after O'Neill 2014's PCG).
fn pcg(v : u32) -> u32 {
  let state = v * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return (word >> 22u) ^ word;
}

// A number in [0, 1) from a hash's top 24 bits.
fn unit_of(h : u32) -> f32 {
  return f32(h >> 8u) / 16777216.0;
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(num_workgroups) groups : vec3u, @builtin(local_invocation_index) local : u32) {
  let count = min(params.counts.x, params.counts.y);
  let k = folded_group(group, groups) * PLACE_GROUP + local;
  if (k == 0u) {
    instance_count[0] = count;
  }
  if (k >= count) {
    return;
  }
  var h = pcg(k ^ pcg(params.counts.z));
  var u : array<f32, 8>;
  for (var i = 0u; i < 8u; i = i + 1u) {
    h = pcg(h);
    u[i] = unit_of(h);
  }
  let tangent = vec2f((2.0 * u[0] - 1.0) * params.viewX.w, (2.0 * u[1] - 1.0) * params.viewY.w);
  let along = normalize(vec3f(tangent, -1.0));
  let away = params.ranges.x * pow(params.ranges.y / params.ranges.x, u[2]);
  let p = along * away;
  let centre = params.viewX.xyz * p.x + params.viewY.xyz * p.y + params.viewZ.xyz * p.z;
  let radius = params.ranges.z * pow(params.ranges.w / params.ranges.z, u[3]);
  let s = sqrt(1.0 - u[4]);
  let t = sqrt(u[4]);
  let q = vec4f(s * sin(TAU * u[5]), s * cos(TAU * u[5]), t * sin(TAU * u[6]), t * cos(TAU * u[6]));
  let grey = 0.15 + 0.3 * u[7];
  let base = k * 8u;
  instances[base] = bitcast<u32>(centre.x);
  instances[base + 1u] = bitcast<u32>(centre.y);
  instances[base + 2u] = bitcast<u32>(centre.z);
  instances[base + 3u] = bitcast<u32>(radius);
  instances[base + 4u] = pack2x16snorm(q.xy);
  instances[base + 5u] = pack2x16snorm(q.zw);
  instances[base + 6u] = pack4x8unorm(vec4f(grey * 1.05, grey, grey * 0.9, 1.0));
  instances[base + 7u] = k;
}

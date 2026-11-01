// The cluster path's cull (plan R14, R14.T1.b; the research `research-terrain-gpu-driven` §5): one
// dispatch a view-frame, a workgroup a consumer instance and a thread a cluster of its page, each
// cluster tested against every pass its instance may enter, given the coarsest template within the
// pass's tolerance, and appended to that template's bucket of the pass (`clusters/buckets.ts`).
// The TypeScript twin is `clusters/cull.ts`' `cullTwin`.
//
// The appends are `atomicAdd`s, so a bucket's order varies from run to run: only depth-only passes
// draw from it, whose result is each texel's nearest depth whatever the order (plan R14's design
// note on compaction). The kernel is `presentation-only`; its counts, integer sums, reach the CPU
// only through `clusters/stats.ts`' `bit-exact` copy.
//
// - _Instances_: the consumer's own records, `counts.y` words each, its first four the origin less
//   the camera (`vec3f`, in the frame the passes' axes are in) and the page; beside them, one
//   `vec2u` an instance: the passes it may enter, a bit a pass (the consumer's own patch-level test,
//   terrain's `castsInto` in `f64`), and its error unit, m, as `f32` bits (terrain's: its level's
//   largest vertex spacing).
// - _A cascade's box_ (kind 1): the cluster's sphere against the light-space box of `castsInto`
//   (`terrainPass.ts`), |v·x_L| ≤ r + ρ, |v·y_L| ≤ r + ρ, v·z_L ≥ −(far + ρ), each widened by
//   ROUNDING_FRACTION of the camera-relative lengths and ROUNDING_M, the `f32` arithmetic's
//   allowance, so that no cluster the `f64` test keeps is refused. A cluster outside x or y draws no
//   texel of the layer, and one past the far plane none either: clipped there, or, under
//   `unclippedDepth`, clamped to the cleared depth, which the comparison leaves as it is. The side
//   towards the light is the instance's own test's: a cluster cut there would change a pancaked
//   caster's texels.
// - _The stride_: the coarsest s = 2^l, l up to the pass's cap, with s × the unit within the pass's
//   tolerance, in `f32` as the twin forms it; s = 1 where none is.
// - _The bucket_: the record's shape (its flags' bits 0–3) × STRIDE_LEVELS + l, from the pass's
//   first bucket; the entry, (instance << 8) | cluster, at its template's list offset in the pass
//   plus the place the append took. An append past a template's capacity is dropped: none is, the
//   lists holding every cluster of every instance once a pass.

struct ClusterRecord {
  centre : vec3f,
  radius : f32,
  errors : vec2u,
  templateRef : u32,
  flags : u32,
}

struct CullPass {
  header : vec4u,
  scalars : vec4f,
  axisX : vec4f,
  axisY : vec4f,
  axisZ : vec4f,
  centre : vec4f,
  receivers : vec4f,
  covered : vec4f,
}

struct CullParams {
  rotation0 : vec4f,
  rotation1 : vec4f,
  rotation2 : vec4f,
  counts : vec4u,
  viewAxis : vec3f,
  templates : u32,
  listOffsetsLow : vec4u,
  listOffsetsHigh : vec4u,
  capacitiesLow : vec4u,
  capacitiesHigh : vec4u,
}

// `cullPass.ts`' MAX_CULL_PASSES, a pass's kinds and the strides' levels.
const MAX_CULL_PASSES : u32 = 8u;
const PASS_CASCADE : u32 = 1u;
const STRIDE_LEVELS : u32 = 4u;
// The threads of a workgroup, each taking every 64th cluster of its instance's page.
const CULL_GROUP : u32 = 64u;
// The `f32` allowance: per metre of the camera-relative lengths a test forms (some sixteen ulps,
// 2⁻²⁴ each), and a metre.
const ROUNDING_FRACTION : f32 = 1e-6;
const ROUNDING_M : f32 = 1.0;

@group(0) @binding(0) var<uniform> params : CullParams;
@group(0) @binding(1) var<uniform> passes : array<CullPass, 8>;
@group(0) @binding(2) var<storage, read> instance_words : array<u32>;
@group(0) @binding(3) var<storage, read> cull_inputs : array<vec2u>;
@group(0) @binding(4) var<storage, read> records : array<ClusterRecord>;
@group(0) @binding(5) var<storage, read_write> lists : array<u32>;
@group(0) @binding(6) var<storage, read_write> args : array<atomic<u32>>;

// A record's centre in the instances' frame, turned from its page's axes.
fn page_to_instance(v : vec3f) -> vec3f {
  return params.rotation0.xyz * v.x + params.rotation1.xyz * v.y + params.rotation2.xyz * v.z;
}

// Whether the sphere at `centre`, camera-relative, of radius `radius` may draw into cascade box
// `pass` (see the module's head).
fn cascade_keeps(cull_pass : CullPass, centre : vec3f, radius : f32) -> bool {
  let v = vec3f(dot(centre, cull_pass.axisX.xyz), dot(centre, cull_pass.axisY.xyz), dot(centre, cull_pass.axisZ.xyz)) - cull_pass.centre.xyz;
  let slack = ROUNDING_FRACTION * (length(centre) + length(cull_pass.centre.xyz) + cull_pass.scalars.y) + ROUNDING_M;
  let reach = cull_pass.scalars.y + radius + slack;
  return abs(v.x) <= reach && abs(v.y) <= reach && v.z >= -(cull_pass.scalars.z + radius + slack);
}

// The coarsest stride's level, up to the pass's cap, whose 2^l × `unit` is within its tolerance.
fn stride_level(cull_pass : CullPass, unit : f32) -> u32 {
  var level = 0u;
  for (var l = 1u; l <= cull_pass.header.y && l < STRIDE_LEVELS; l = l + 1u) {
    if (f32(1u << l) * unit <= cull_pass.scalars.x) {
      level = l;
    }
  }
  return level;
}

fn template_offset(t : u32) -> u32 {
  if (t < 4u) {
    return params.listOffsetsLow[t];
  }
  return params.listOffsetsHigh[t - 4u];
}

fn template_capacity(t : u32) -> u32 {
  if (t < 4u) {
    return params.capacitiesLow[t];
  }
  return params.capacitiesHigh[t - 4u];
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(local_invocation_id) local : vec3u) {
  let instance = group.x;
  if (instance >= params.counts.x) {
    return;
  }
  let base = instance * params.counts.y;
  let origin = vec3f(
    bitcast<f32>(instance_words[base]),
    bitcast<f32>(instance_words[base + 1u]),
    bitcast<f32>(instance_words[base + 2u]),
  );
  let page = instance_words[base + 3u];
  let input = cull_inputs[instance];
  let unit = bitcast<f32>(input.y);
  let clusters = params.counts.z;
  let pass_count = min(params.counts.w, MAX_CULL_PASSES);
  for (var c = local.x; c < clusters; c = c + CULL_GROUP) {
    let record = records[page * clusters + c];
    let centre = origin + page_to_instance(record.centre);
    let shape = record.flags & 0xfu;
    for (var p = 0u; p < pass_count; p = p + 1u) {
      if ((input.x & (1u << p)) == 0u) {
        continue;
      }
      let cull_pass = passes[p];
      if (cull_pass.header.x != PASS_CASCADE || !cascade_keeps(cull_pass, centre, record.radius)) {
        continue;
      }
      let t = shape * STRIDE_LEVELS + stride_level(cull_pass, unit);
      if (t >= params.templates) {
        continue;
      }
      let at = atomicAdd(&args[(cull_pass.header.z + t) * 5u + 1u], 1u);
      if (at < template_capacity(t)) {
        lists[cull_pass.header.w + template_offset(t) + at] = (instance << 8u) | c;
      }
    }
  }
}

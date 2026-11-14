// The instance cull (plan R14, R14.T5.d): an instance's bounds tested against every pass of the
// frame before any cluster of it, a thread an instance. Its TypeScript twin is
// `clusters/instanceCull.ts`' `instanceCullTwin`. Composed after `fold.ts`' `folded_group` and the
// cluster cull's pass tests (`cull.ts`' CULL_PASS_TESTS_WGSL): the same `CullPass`, the same
// allowances and `pass_keeps`, so that an instance's sphere is kept as a cluster's would be.
//
// - _Instances_: `counts.y` words each, their first three the centre less the camera (`vec3f`, in
//   the frame the passes' axes are in) and word `counts.z` the radius of the sphere its templates
//   draw, m, its bounds that radius times `boundsScale` (`particles.ts`' `boundsScale`, the
//   templates' largest vertex over the radius); a negative radius is no instance. The live ones are the first `instance_count[0]` (a producer's
//   GPU-written count, R11's placement), at most the path's capacity, `counts.x`. Every instance
//   may enter each pass whose bit `passMask` sets.
// - _The fold_: the dispatch's workgroups fold over two dimensions past 65,535 (`fold.ts`), each
//   taking the 64 instances from its index's; a workgroup wholly past the capacity does nothing.
// - _The template_ (a particle's: `particles.ts`), for each pass that keeps the sphere: the
//   coarsest of `templates` (coarsest first) whose error fraction times the radius is within the
//   pass's tolerance at the bounds, τ max(|S| − ρ, 0) for a camera pass, ρ the bounds' radius and τ
//   its tolerance in rad as the DAG cut takes it, and its texel, m, for a cascade; the finest where
//   none is.
// - _The append_: the instance's index into the (pass, template) bucket of `clusters/buckets.ts`,
//   at its template's list offset in the pass plus the place it took: aggregated in the workgroup,
//   one global `atomicAdd` a (pass, template) a workgroup, since one an instance contends hardest
//   on Apple's GPUs (plan R14, the cull's Provides). The order is not reproducible: the kernel is
//   `presentation-only`. An append past a template's capacity is dropped: none is, each template's
//   list holding every instance once a pass.

struct InstanceCullParams {
  counts : vec4u,
  viewAxis : vec3f,
  templates : u32,
  errors : vec4f,
  listOffsets : vec4u,
  capacities : vec4u,
  passMask : u32,
  boundsScale : f32,
  spare0 : u32,
  spare1 : u32,
}

// The pass tests' absolute allowance, m (`instanceCull.ts`' INSTANCE_ROUNDING_M): a millimetre, since
// an instance's centre is the `f32` its producer wrote, narrowed from nothing, so that its tests'
// rounding is the allowance's fraction of their lengths alone.
const ROUNDING_M : f32 = 0.001;
// `particles.ts`' MAX_PARTICLE_TEMPLATES, and the workgroup's counters: one a (pass, template).
const MAX_TEMPLATES : u32 = 4u;
const GROUP_COUNTERS : u32 = 32u;
// A pass that does not keep the instance.
const NO_TEMPLATE : u32 = 0xffffffffu;
// The threads of a workgroup, an instance each (`instanceCull.ts`' INSTANCE_CULL_WORKGROUP).
const INSTANCE_GROUP : u32 = 64u;

@group(0) @binding(0) var<uniform> params : InstanceCullParams;
@group(0) @binding(1) var<uniform> passes : array<CullPass, 8>;
@group(0) @binding(2) var<storage, read> instance_words : array<u32>;
@group(0) @binding(3) var<storage, read> instance_count : array<u32>;
@group(0) @binding(4) var<storage, read_write> lists : array<u32>;
@group(0) @binding(5) var<storage, read_write> args : array<atomic<u32>>;

var<workgroup> group_counts : array<atomic<u32>, 32>;
var<workgroup> group_bases : array<u32, 32>;

// The coarsest template of the instance's sphere of `radius` within the pass's tolerance at its
// bounds of radius `bounds` (see the module's head), the finest where none is.
fn instance_template(cull_pass : CullPass, centre : vec3f, radius : f32, bounds : f32) -> u32 {
  var reach = cull_pass.scalars.x;
  if (cull_pass.header.x != PASS_CASCADE) {
    reach = cull_pass.scalars.x * group_distance(centre, bounds);
  }
  let last = min(params.templates, MAX_TEMPLATES) - 1u;
  for (var t = 0u; t < last; t = t + 1u) {
    if (params.errors[t] * radius <= reach) {
      return t;
    }
  }
  return last;
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(num_workgroups) groups : vec3u, @builtin(local_invocation_index) local : u32) {
  let first = folded_group(group, groups) * INSTANCE_GROUP;
  if (first >= params.counts.x || params.templates == 0u) {
    return;
  }
  if (local < GROUP_COUNTERS) {
    atomicStore(&group_counts[local], 0u);
  }
  workgroupBarrier();
  let instance = first + local;
  let live = instance < min(instance_count[0], params.counts.x);
  var centre = vec3f(0.0);
  var radius = -1.0;
  if (live) {
    let base = instance * params.counts.y;
    centre = vec3f(
      bitcast<f32>(instance_words[base]),
      bitcast<f32>(instance_words[base + 1u]),
      bitcast<f32>(instance_words[base + 2u]),
    );
    radius = bitcast<f32>(instance_words[base + params.counts.z]);
  }
  let pass_count = min(params.counts.w, MAX_CULL_PASSES);
  var kept : array<u32, 8>;
  var slots : array<u32, 8>;
  for (var p = 0u; p < pass_count; p = p + 1u) {
    kept[p] = NO_TEMPLATE;
    let cull_pass = passes[p];
    let bounds = radius * params.boundsScale;
    if (radius >= 0.0 && (params.passMask & (1u << p)) != 0u && pass_keeps(cull_pass, centre, bounds)) {
      let t = instance_template(cull_pass, centre, radius, bounds);
      kept[p] = t;
      slots[p] = atomicAdd(&group_counts[p * MAX_TEMPLATES + t], 1u);
    }
  }
  workgroupBarrier();
  if (local < pass_count * MAX_TEMPLATES) {
    let p = local / MAX_TEMPLATES;
    let t = local % MAX_TEMPLATES;
    let n = atomicLoad(&group_counts[local]);
    group_bases[local] = 0u;
    if (n > 0u && t < params.templates) {
      group_bases[local] = atomicAdd(&args[(passes[p].header.z + t) * 5u + 1u], n);
    }
  }
  workgroupBarrier();
  for (var p = 0u; p < pass_count; p = p + 1u) {
    let t = kept[p];
    if (t == NO_TEMPLATE) {
      continue;
    }
    let at = group_bases[p * MAX_TEMPLATES + t] + slots[p];
    if (at < params.capacities[t]) {
      lists[passes[p].header.w + params.listOffsets[t] + at] = instance;
    }
  }
}

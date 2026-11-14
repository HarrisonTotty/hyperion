// The instance cull above the cluster cull (plan R14, R14.T5.d): a mesh consumer's instances' bounds
// tested against every pass before any cluster of them, a thread an instance, and the instances any
// pass keeps compacted, their records copied whole, for the cluster cull's dispatch over them alone
// (`consumer.ts`' `ClusterConsumerPath.cullInstances`). Its twin is `instanceCull.ts`'
// `instanceCompactTwin`. Composed after `fold.ts`' `folded_group` and the cluster cull's pass tests
// (`cull.ts`' CULL_PASS_TESTS_WGSL), as `instanceCull.wgsl` is, whose instances and fold it shares.
//
// - _Instances_: the consumer's own records, `counts.y` words each, its first three the origin less
//   the camera (the cluster cull's) and word `counts.z` its bounding sphere's radius about that
//   origin, m, holding every cluster of every page it may draw; a negative radius is no instance.
//   The live ones are the first `instance_count[0]`, at most `counts.x`. Each may enter the passes
//   whose bits `mask` sets.
// - _The survivors_: an instance any pass keeps takes the next place k of `survivors[0]` (aggregated
//   in the workgroup: one global `atomicAdd` a workgroup), its record copied to place k of
//   `compacted`, and its cull input, the passes that keep it and an error unit of 0 (a DAG record's
//   cut reads none), to place k of `compacted_inputs`, the cluster cull's `cull_inputs`. The order
//   is not reproducible: the kernel is `presentation-only`, and only appended passes draw its
//   instances' clusters until a stable compaction orders them.

struct InstanceCompactParams {
  counts : vec4u,
  viewAxis : vec3f,
  mask : u32,
}

// The pass tests' absolute allowance, m (`instanceCull.ts`' INSTANCE_ROUNDING_M): a millimetre, since
// an instance's centre is the `f32` its producer wrote, narrowed from nothing, so that its tests'
// rounding is the allowance's fraction of their lengths alone.
const ROUNDING_M : f32 = 0.001;
const INSTANCE_GROUP : u32 = 64u;

@group(0) @binding(0) var<uniform> params : InstanceCompactParams;
@group(0) @binding(1) var<uniform> passes : array<CullPass, 8>;
@group(0) @binding(2) var<storage, read> instance_words : array<u32>;
@group(0) @binding(3) var<storage, read> instance_count : array<u32>;
@group(0) @binding(4) var<storage, read_write> compacted : array<u32>;
@group(0) @binding(5) var<storage, read_write> compacted_inputs : array<vec2u>;
@group(0) @binding(6) var<storage, read_write> survivors : array<atomic<u32>>;

var<workgroup> group_count : atomic<u32>;
var<workgroup> group_base : u32;

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(num_workgroups) groups : vec3u, @builtin(local_invocation_index) local : u32) {
  let first = folded_group(group, groups) * INSTANCE_GROUP;
  if (first >= params.counts.x) {
    return;
  }
  if (local == 0u) {
    atomicStore(&group_count, 0u);
  }
  workgroupBarrier();
  let instance = first + local;
  let base = instance * params.counts.y;
  var bits = 0u;
  if (instance < min(instance_count[0], params.counts.x)) {
    let centre = vec3f(
      bitcast<f32>(instance_words[base]),
      bitcast<f32>(instance_words[base + 1u]),
      bitcast<f32>(instance_words[base + 2u]),
    );
    let radius = bitcast<f32>(instance_words[base + params.counts.z]);
    let pass_count = min(params.counts.w, MAX_CULL_PASSES);
    for (var p = 0u; p < pass_count; p = p + 1u) {
      if (radius >= 0.0 && (params.mask & (1u << p)) != 0u && pass_keeps(passes[p], centre, radius)) {
        bits = bits | (1u << p);
      }
    }
  }
  var slot = 0u;
  if (bits != 0u) {
    slot = atomicAdd(&group_count, 1u);
  }
  workgroupBarrier();
  if (local == 0u) {
    let n = atomicLoad(&group_count);
    group_base = 0u;
    if (n > 0u) {
      group_base = atomicAdd(&survivors[0], n);
    }
  }
  workgroupBarrier();
  if (bits == 0u) {
    return;
  }
  let at = group_base + slot;
  for (var w = 0u; w < params.counts.y; w = w + 1u) {
    compacted[at * params.counts.y + w] = instance_words[base + w];
  }
  compacted_inputs[at] = vec2u(bits, 0u);
}

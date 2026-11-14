// The survivors' fold (plan R14, R14.T5.d; `clusters/fold.ts`): after the instance cull above the
// cluster cull (`instanceCompact.wgsl`), the cluster cull's indirect dispatch over its survivors,
// a workgroup a survivor, folded over two dimensions past 65,535, as `fold.ts`' `foldedWorkgroups`
// folds the same count, integer arithmetic: y = ⌈n ÷ 65,535⌉ rows of x = ⌈n ÷ y⌉, (0, 1, 1) for
// none. The x · y − n places past the survivors, fewer than y, have their cull inputs zeroed, so that
// the cluster cull's workgroups there enter no pass. One workgroup. Its twin is `fold.ts`'
// `foldedWorkgroups`.

struct InstanceFoldParams {
  // The survivors' capacity, and the compacted inputs' places, which hold the fold's tail.
  counts : vec4u,
}

const MAX_WORKGROUPS_PER_DIMENSION : u32 = 65535u;
const FOLD_GROUP : u32 = 64u;

@group(0) @binding(0) var<uniform> params : InstanceFoldParams;
// The survivors' count, then the cluster cull's indirect workgroups (x, y, z).
@group(0) @binding(1) var<storage, read_write> survivors : array<u32>;
@group(0) @binding(2) var<storage, read_write> compacted_inputs : array<vec2u>;

// The fold of `n` workgroups (`fold.ts`' `foldedWorkgroups`).
fn fold_of(n : u32) -> vec3u {
  let y = max((n + MAX_WORKGROUPS_PER_DIMENSION - 1u) / MAX_WORKGROUPS_PER_DIMENSION, 1u);
  return vec3u((n + y - 1u) / y, y, 1u);
}

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_index) local : u32) {
  let n = min(survivors[0], params.counts.x);
  let fold = fold_of(n);
  let end = min(fold.x * fold.y, params.counts.y);
  for (var k = n + local; k < end; k = k + FOLD_GROUP) {
    compacted_inputs[k] = vec2u(0u);
  }
  if (local == 0u) {
    survivors[1] = fold.x;
    survivors[2] = fold.y;
    survivors[3] = fold.z;
  }
}

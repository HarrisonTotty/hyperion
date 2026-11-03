// The far limb's node pass (plan R08, R08.T8.d; `hillaire.ts`'s `LimbRays` `packed`): each node ray
// the packed entry point (`rayMarchPacked.wgsl`) wrote to the limb's list, one an invocation, so
// that a tile's lanes no longer wait on its slowest texel's nodes; each marched by the one
// `marchRay` call site at the scene's depth under its own point, as the footprint's entry point
// marches it, its march written back beside it for the resolve (`limbResolve.wgsl`). Dispatched
// indirectly, over the rays the dispatch's count took (`limbDispatch.wgsl`). Appended last to make
// `hillaire.ts`'s RAY_MARCH_LIMB_KERNEL and RAY_MARCH_SLOTS_LIMB_KERNEL, after `limbList.wgsl`,
// `rayMarch.wgsl` without its two targets (`withoutMarchTargets`) and the source's read.

@group(0) @binding(20) var<storage, read> limbCounts : LimbCounts;
@group(0) @binding(22) var<storage, read_write> limbRays : array<LimbRay>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let i = id.x;
  if (i >= limbCounts.taken.y) {
    return;
  }
  let uv = limbRays[i].uv;
  // As the footprint's entry point takes a node's depth.
  let full = vec2u(view.output.xy);
  let nodeUv = clamp(uv, vec2f(0.0), vec2f(1.0));
  let nodeTexel = min(vec2u(nodeUv * vec2f(full)), full - 1u);
  let one = marchRay(uv, textureLoad(sceneDepth, nodeTexel, 0));
  limbRays[i].light = one.light;
  limbRays[i].transmittance = one.transmittance;
}

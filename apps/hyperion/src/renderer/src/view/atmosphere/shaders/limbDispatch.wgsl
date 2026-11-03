// The far limb's dispatches (plan R08, R08.T8.d; `hillaire.ts`'s `LimbRays` `packed`): once the
// packed entry point has filled the lists (`limbList.wgsl`), the texels and node rays they took,
// each count held to its list's room, and the node pass's and the resolve's workgroups for their
// indirect dispatches, x, y and z each (`limbNodes.wgsl`, `limbResolve.wgsl`). One invocation.
// Follows `limbList.wgsl` alone.

@group(0) @binding(0) var<storage, read_write> limbCounts : LimbCounts;
@group(0) @binding(1) var<storage, read_write> limbDispatch : array<u32, 6>;
@group(0) @binding(2) var<storage, read> limbTexels : array<LimbTexel>;
@group(0) @binding(3) var<storage, read> limbRays : array<LimbRay>;

@compute @workgroup_size(1, 1, 1)
fn main() {
  let texels = min(limbCounts.reserved.x, arrayLength(&limbTexels));
  let rays = min(limbCounts.reserved.y, arrayLength(&limbRays));
  limbCounts.taken = vec2u(texels, rays);
  limbDispatch[0] = (rays + LIMB_WORKGROUP - 1u) / LIMB_WORKGROUP;
  limbDispatch[1] = 1u;
  limbDispatch[2] = 1u;
  limbDispatch[3] = (texels + LIMB_WORKGROUP - 1u) / LIMB_WORKGROUP;
  limbDispatch[4] = 1u;
  limbDispatch[5] = 1u;
}

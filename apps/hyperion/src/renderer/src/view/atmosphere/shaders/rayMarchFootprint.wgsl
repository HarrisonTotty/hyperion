// The ray march's entry point over the far limb's footprint (plan R08, R08.T8.c to R08.T8.e; since
// R08.T11.d, `decision-r08-t8-leans.md` section 11, Ruling 4, the tile cull's second): each texel of
// a tile the centre's entry point (`rayMarchCentre.wgsl`) classed as one whose footprint may blend,
// marched as the one kernel marched every texel before. Under the reference path (`marchArea.bound.x`
// 0) it marches every texel of the area, the whole frame, and reads no tile's kind. Appended last
// to make `hillaire.ts`'s RAY_MARCH_KERNEL and RAY_MARCH_SLOTS_KERNEL, after `marchArea.wgsl`,
// `rayMarch.wgsl` and the source's read.

@group(0) @binding(17) var<uniform> marchArea : MarchArea;
@group(0) @binding(18) var<storage, read> marchTiles : array<u32>;

@compute @workgroup_size(8, 8, 1)
fn main(
  @builtin(workgroup_id) group : vec3u,
  @builtin(local_invocation_id) local : vec3u,
) {
  // The tile cull (R08.T11.d): any tile but the footprint's is the centre's entry point's, which
  // classed it and was dispatched first.
  if (marchArea.bound.x > 0.0 && marchTiles[marchTileIndexOf(marchArea, group.xy)] != MARCH_TILE_FOOTPRINT) {
    return;
  }
  let texel = marchTexelOf(marchArea, group.xy, local.xy);
  // The target may be larger than this frame's output (it grows and is never remade smaller).
  let size = vec2u(ceil(view.output.xy * view.output.z));
  if (texel.x >= size.x || texel.y >= size.y) {
    return;
  }
  let uv = (vec2f(texel) + 0.5) / vec2f(size);
  let full = vec2u(view.output.xy);
  let depthTexel = min(vec2u(uv * vec2f(full)), full - 1u);
  let depth = textureLoad(sceneDepth, depthTexel, 0);
  let reach = view.tables.y;
  var tSurface = 3.4e38;
  if (depth > 0.0) {
    tSurface = distanceAtDepth(view, uv, depth);
  }
  // Every pixel once the camera's h* reaches the march band, whose weight is 1 at and above the top
  // (R08.T8); below it, the surfaces beyond the aerial-perspective volume's reach.
  let needed = view.exact.w > 0.0 || (depth > 0.0 && tSurface > reach);
  if (!needed) {
    textureStore(rayMarchOut, texel, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  // From above the top, a texel whose footprint meets the shell takes its mean across the limb,
  // blended in by its span (R08.T8.c, `farLimb.wgsl`); the camera's h* against the top is the same
  // for every texel, so the branch is uniform.
  let above = view.camera.w * view.curvature.z >= medium.topHeightM;
  // Ray j is the centre ray (j = 0) or node j − 1, each through one call of the march (R08.T8.d).
  // One call site is a constraint, not a style: SwiftShader inlines every call, and with the
  // march's steps emitted twice in one entry point its GPU process segfaults (a write to 0x18 in
  // `libvk_swiftshader.so`, R08.T8.d). The footprint is formed again for each ray, from the texel's
  // centre alone, and gives the count and w_L at the first and the ray's own node (`farLimbNodeOf`)
  // after; so none of it is held across a march, only the count, w_L and the sums (R08.T8.e, E1′).
  // Where w_L is 1 the mean reads none of the centre ray, so the loop starts at the first node (E4).
  var nodes = 0u;
  var blend = 0.0;
  var marched = vec4f(0.0);
  var mean = vec4f(0.0);
  for (var j = 0u; j <= nodes; j++) {
    var rayUv = uv;
    var rayDepth = depth;
    var weight = 0.0;
    if (above) {
      var footprint = farLimbFootprintOf(
        farLimbRaysOf(view), oblateBodyOf(oblateBody), oblateBody, uv, vec2f(size)
      );
      if (j == 0u) {
        blend = footprint.blend;
        nodes = select(0u, footprint.count, footprint.blend > 0.0);
        mean = vec4f(0.0, 0.0, 0.0, footprint.spaceShare);
        if (blend >= 1.0) {
          j = 1u;
        }
      }
      if (j > 0u) {
        let node = farLimbNodeOf(&footprint, j - 1u);
        // Each node at the scene's depth under its own point, so that a drawn surface's edge across
        // the footprint cuts only the nodes on it.
        let nodeUv = clamp(node.xy, vec2f(0.0), vec2f(1.0));
        let nodeTexel = min(vec2u(nodeUv * vec2f(full)), full - 1u);
        rayUv = node.xy;
        rayDepth = textureLoad(sceneDepth, nodeTexel, 0);
        weight = node.z;
      }
    }
    let one = marchRay(rayUv, rayDepth);
    if (j == 0u) {
      marched = one;
    } else {
      mean += weight * one;
    }
  }
  if (blend > 0.0) {
    if (blend >= 1.0) {
      marched = mean;
    } else {
      marched = mix(marched, mean, blend);
    }
  }
  textureStore(rayMarchOut, texel, marched);
}

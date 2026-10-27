// The ray march's entry point at the centre ray (plan R08, R08.T11.d; `decision-r08-t8-leans.md`
// section 11, Ruling 4, the tile cull's first): dispatched first over the air's area
// (`marchArea.ts`), it classes each tile of 8 x 8 texels by a conservative form of the far limb's
// footprint's own test (`farLimb.wgsl`'s farLimbFootprintOf), writes the tile's kind for the
// footprint's entry point (`rayMarchFootprint.wgsl`), dispatched after it, and takes the tiles no
// footprint can blend in: (0, 0, 0, 1) on a tile every ray misses, and the centre ray, as the
// footprint's entry point marches a texel of w_L 0, elsewhere. It holds none of the footprint
// across its march, which cost the one kernel 0.42-0.86 ms from orbit (QW2 and QW3's Kn - K0, plan
// R08's "Deviations in T8.e, as built"); on the RTX 3080 the two dispatches cost 0.4-1.0 ms more
// than the one kernel from 400 km and 1e7 m and saved 0.4-1.3 ms farther, so the cull is a cover of
// its own (`hillaire.ts`'s `MarchCover` `tiles`), off for a view. Appended last to make `hillaire.ts`'s
// RAY_MARCH_CENTRE_KERNEL and RAY_MARCH_SLOTS_CENTRE_KERNEL, after `marchArea.wgsl`,
// `rayMarch.wgsl` and the source's read.

@group(0) @binding(17) var<uniform> marchArea : MarchArea;
@group(0) @binding(18) var<storage, read_write> marchTiles : array<u32>;

// The texels about a tile's centre that its texels' rays reach, per axis: their centres within 3.5,
// their footprints' nodes within 0.71 of a texel of those, and their spans' rays at the texels'
// edges' midpoints within 0.5.
const MARCH_TILE_REACH_TEXELS : f32 = 4.5;

// The kind of the tile at `tile` on the target's grid of `size` texels (`marchArea.wgsl`'s kinds),
// from the line through its centre. Along the straight path between two unnormalised rays
// w = forward + x right + y upRay, |w| >= 1, the scaled unit ray turns by at most k |Δw|, k the
// larger of a shell's axis ratio and its inverse, and so a line's tangent height over the datum or
// the top's shell moves by at most |o| k^2 |Δw| (`farLimbFootprintOf`'s bound, |S o| <= k |o|); over
// the tile's reach |Δw| is at most 2 E (|right| ÷ size.x + |upRay| ÷ size.y). That bound, widened by
// 1% and by `marchArea.bound.y` (`marchArea.ts`'s widening, about 160 times f32's rounding of a
// tangent height or of the march's discriminant), holds every texel's tangent heights, and each
// footprint's span, which is at most their change across the texel. So where the top's tangent
// height less it is above 0, every ray misses the shell and no footprint reaches the top; where the
// datum's plus it is at most 0, or the camera is inside the top, no footprint blends
// (`farLimbFootprintOf` needs the span to reach above the datum and below the top, from above the
// top). The top is the larger of the medium's and the body table's, the march's and the
// footprint's.
fn marchTileKindOf(tile : vec2u, size : vec2f) -> u32 {
  let body = oblateBodyOf(oblateBody);
  let origin = view.camera.xyz;
  let a = view.figure.x;
  let c = view.figure.y;
  let topM = max(medium.topHeightM, body.counts.w);
  let topA = a + topM * body.scales.z;
  let topC = c + topM * body.scales.w;
  let centreUv = (vec2f(tile * MARCH_TILE_TEXELS) + 0.5 * f32(MARCH_TILE_TEXELS)) / size;
  let dir = farLimbRayThrough(farLimbRaysOf(view), centreUv);
  let groundM = farLimbApproach(origin, dir, a, c).x;
  let topHeightM = farLimbApproach(origin, dir, topA, topC).x;
  let k = max(max(a / c, c / a), max(topA / topC, topC / topA));
  let reachM = length(origin) * k * k * 2.0 * MARCH_TILE_REACH_TEXELS
    * (length(view.right.xyz) / size.x + length(view.upRay.xyz) / size.y);
  let boundM = 1.01 * reachM + marchArea.bound.y;
  if (topHeightM - boundM > 0.0) {
    return MARCH_TILE_MISS;
  }
  // As the footprint's entry point takes it, so that both read one `above`.
  let above = view.camera.w * view.curvature.z >= medium.topHeightM;
  if (above && groundM + boundM > 0.0) {
    return MARCH_TILE_FOOTPRINT;
  }
  return MARCH_TILE_CENTRE;
}

@compute @workgroup_size(8, 8, 1)
fn main(
  @builtin(workgroup_id) group : vec3u,
  @builtin(local_invocation_id) local : vec3u,
  @builtin(local_invocation_index) lane : u32,
) {
  // The target may be larger than this frame's output (it grows and is never remade smaller).
  let size = vec2u(ceil(view.output.xy * view.output.z));
  // Every invocation of the workgroup forms the same kind from the same values; the first writes
  // it, once a tile and before the footprint's entry point reads it.
  let kind = marchTileKindOf(vec2u(marchArea.tiles.xy) + group.xy, vec2f(size));
  if (lane == 0u) {
    marchTiles[marchTileIndexOf(marchArea, group.xy)] = kind;
  }
  if (kind == MARCH_TILE_FOOTPRINT) {
    return;
  }
  let texel = marchTexelOf(marchArea, group.xy, local.xy);
  if (texel.x >= size.x || texel.y >= size.y) {
    return;
  }
  if (kind == MARCH_TILE_MISS) {
    textureStore(rayMarchOut, texel, vec4f(0.0, 0.0, 0.0, 1.0));
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
  // As the footprint's entry point: every pixel once the camera's h* reaches the march band, and
  // below it the surfaces beyond the aerial-perspective volume's reach (R08.T8).
  let needed = view.exact.w > 0.0 || (depth > 0.0 && tSurface > reach);
  if (!needed) {
    textureStore(rayMarchOut, texel, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  // The centre ray alone, as the footprint's entry point marches a texel of w_L 0.
  textureStore(rayMarchOut, texel, marchRay(uv, depth));
}

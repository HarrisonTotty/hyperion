// The ray march's entry point over the far limb's footprint with its node rays packed (plan R08,
// R08.T8.d; `hillaire.ts`'s `LimbRays` `packed`): `rayMarchFootprint.wgsl`'s, but a texel whose
// footprint is blended in takes an entry of the limb's lists (`limbList.wgsl`) and writes its
// footprint's node rays there, marching its centre ray alone (none at w_L 1), which it holds for
// the resolve (`limbResolve.wgsl`) in place of storing the texel; the node pass marches the nodes
// one a thread (`limbNodes.wgsl`). A texel whose nodes find no room in the lists is marched as the
// footprint's entry point marches it, with its nodes, and stored. Every other texel is the
// footprint's entry point's, line for line, so that each texel's bits are its. Appended last to
// make `hillaire.ts`'s RAY_MARCH_PACKED_KERNEL and RAY_MARCH_SLOTS_PACKED_KERNEL, after
// `limbList.wgsl`, `marchArea.wgsl`, `rayMarch.wgsl` and the source's read.

@group(0) @binding(17) var<uniform> marchArea : MarchArea;
@group(0) @binding(18) var<storage, read> marchTiles : array<u32>;
@group(0) @binding(20) var<storage, read_write> limbCounts : LimbReservations;
@group(0) @binding(21) var<storage, read_write> limbTexels : array<LimbTexel>;
@group(0) @binding(22) var<storage, read_write> limbRays : array<LimbRay>;

// The lists' counts as the packed entry point reserves them (`limbList.wgsl`'s LimbCounts).
struct LimbReservations {
  texels : atomic<u32>,
  rays : atomic<u32>,
  taken : vec2u,
}

// The entry the texel `texel` takes for `nodes` node rays, or LIMB_REFUSED where the lists have no
// room for it; an entry reserved whose rays find none is marked refused.
fn limbEntryOf(texel : vec2u, nodes : u32) -> u32 {
  let entry = atomicAdd(&limbCounts.texels, 1u);
  if (entry >= arrayLength(&limbTexels)) {
    return LIMB_REFUSED;
  }
  let first = atomicAdd(&limbCounts.rays, nodes);
  if (first + nodes > arrayLength(&limbRays) || first + nodes < first) {
    limbTexels[entry].texel = LIMB_REFUSED;
    return LIMB_REFUSED;
  }
  limbTexels[entry].texel = texel.x | (texel.y << 16u);
  limbTexels[entry].first = first;
  limbTexels[entry].nodes = nodes;
  return entry;
}

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
    storeMarched(texel, marchMissed());
    return;
  }
  // As the footprint's entry point (`rayMarchFootprint.wgsl`): ray j is the centre ray (j = 0) or
  // node j − 1, through one call of the march; but a texel that takes an entry writes its nodes
  // there and marches its centre ray alone, none at w_L 1.
  let above = view.camera.w * view.curvature.z >= medium.topHeightM;
  var nodes = 0u;
  var blend = 0.0;
  var entry = LIMB_REFUSED;
  var marched = vec4f(0.0);
  var mean = vec4f(0.0);
  var marchedRgb = vec3f(0.0);
  var meanRgb = vec3f(0.0);
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
        meanRgb = vec3f(footprint.spaceShare);
        if (nodes > 0u) {
          entry = limbEntryOf(texel, nodes);
        }
        if (entry != LIMB_REFUSED) {
          let first = limbTexels[entry].first;
          for (var n = 0u; n < nodes; n++) {
            let node = farLimbNodeOf(&footprint, n);
            limbRays[first + n].uv = node.xy;
            limbRays[first + n].weight = node.z;
          }
          limbTexels[entry].blend = blend;
          limbTexels[entry].spaceShare = footprint.spaceShare;
          nodes = 0u;
          // Where w_L is 1 the mean reads none of the centre ray (E4): nothing is marched here.
          if (blend >= 1.0) {
            break;
          }
        } else if (blend >= 1.0) {
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
      marched = one.light;
      marchedRgb = one.transmittance;
    } else {
      mean += weight * one.light;
      meanRgb += weight * one.transmittance;
    }
  }
  // A texel with an entry is the resolve's to store, its centre ray held for the blend.
  if (entry != LIMB_REFUSED) {
    limbTexels[entry].centre = marched;
    limbTexels[entry].centreTransmittance = marchedRgb;
    return;
  }
  if (blend > 0.0) {
    if (blend >= 1.0) {
      marched = mean;
      marchedRgb = meanRgb;
    } else {
      marched = mix(marched, mean, blend);
      marchedRgb = mix(marchedRgb, meanRgb, blend);
    }
  }
  storeMarched(texel, MarchedRay(marched, marchedRgb));
}

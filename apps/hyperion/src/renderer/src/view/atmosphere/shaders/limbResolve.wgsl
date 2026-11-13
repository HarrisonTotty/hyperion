// The far limb's resolve (plan R08, R08.T8.d; `hillaire.ts`'s `LimbRays` `packed`): each limb
// texel of the list (`limbList.wgsl`), its node rays' marches summed in node order onto its
// footprint's share above the top and blended with its centre ray by its w_L, as the footprint's
// entry point sums and blends them (`rayMarchFootprint.wgsl`), and stored into both of the march's
// targets (`rayMarch.wgsl`'s storeMarched). A texel its entry marks refused was marched and stored
// whole by the packed entry point. Dispatched indirectly, over the texels the dispatch's count took
// (`limbDispatch.wgsl`). Follows `limbList.wgsl` alone.

@group(0) @binding(0) var<storage, read> limbCounts : LimbCounts;
@group(0) @binding(1) var<storage, read> limbTexels : array<LimbTexel>;
@group(0) @binding(2) var<storage, read> limbRays : array<LimbRay>;
@group(0) @binding(3) var rayMarchOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var rayMarchTransmittanceOut : texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let i = id.x;
  if (i >= limbCounts.taken.x) {
    return;
  }
  let entry = limbTexels[i];
  if (entry.texel == LIMB_REFUSED) {
    return;
  }
  var mean = vec4f(0.0, 0.0, 0.0, entry.spaceShare);
  var meanRgb = vec3f(entry.spaceShare);
  for (var n = 0u; n < entry.nodes; n++) {
    let ray = limbRays[entry.first + n];
    mean += ray.weight * ray.light;
    meanRgb += ray.weight * ray.transmittance;
  }
  var marched = entry.centre;
  var marchedRgb = entry.centreTransmittance;
  if (entry.blend >= 1.0) {
    marched = mean;
    marchedRgb = meanRgb;
  } else {
    marched = mix(marched, mean, entry.blend);
    marchedRgb = mix(marchedRgb, meanRgb, entry.blend);
  }
  let texel = vec2u(entry.texel & 0xffffu, entry.texel >> 16u);
  textureStore(rayMarchOut, texel, marched);
  textureStore(rayMarchTransmittanceOut, texel, vec4f(marchedRgb, marched.a));
}

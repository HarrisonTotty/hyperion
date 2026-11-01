// The ray march beneath a cloud deck (plan R08, R08.T15.b.2; `thick/deckRead.ts`, "Below the
// deck"; `tablesCpu.ts`'s belowDeckMarchTexelTwin): each texel of the air's area as
// `rayMarchFootprint.wgsl` addresses it, its surface beyond the aerial-perspective volume's reach
// taking the path's light to it and T's mean (`belowDeckView.wgsl`'s belowDeckPathTo), every other
// texel (0, 0, 0, 1), as the footprint's march leaves a texel the composite does not read: beneath
// a deck the camera's h* lies under the march band, where the composite takes the sky view and the
// volume. Follows `common.wgsl`, `view.wgsl`, `oblate.wgsl`, `marchArea.wgsl`, `belowDeck.wgsl` and
// `belowDeckView.wgsl`; it reads no medium.

@group(0) @binding(0) var<uniform> view : AtmosphereView;
@group(0) @binding(1) var oblateBody : texture_2d<f32>;
@group(0) @binding(2) var belowDeckTable : texture_2d_array<f32>;
@group(0) @binding(3) var sceneDepth : texture_depth_2d;
@group(0) @binding(4) var rayMarchOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var<uniform> marchArea : MarchArea;

@compute @workgroup_size(8, 8, 1)
fn main(
  @builtin(workgroup_id) group : vec3u,
  @builtin(local_invocation_id) local : vec3u,
) {
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
  if (!(depth > 0.0)) {
    textureStore(rayMarchOut, texel, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  let tSurface = distanceAtDepth(view, uv, depth);
  if (!(tSurface > view.tables.y)) {
    textureStore(rayMarchOut, texel, vec4f(0.0, 0.0, 0.0, 1.0));
    return;
  }
  let body = oblateBodyOf(oblateBody);
  let dir = rayThrough(view, uv);
  let fromM = belowDeckCameraHeightStar();
  let atCamera = belowDeckSkyAt(body, view.up.xyz, dir, fromM);
  textureStore(rayMarchOut, texel, belowDeckPathTo(body, dir, atCamera, fromM, tSurface));
}

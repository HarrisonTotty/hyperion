// The aerial-perspective volume beneath a cloud deck (plan R08, R08.T15.b.2; `thick/deckRead.ts`,
// "Below the deck"; `tablesCpu.ts`'s belowDeckAerialPerspectiveTwin): for each froxel column's ray,
// as `aerialPerspective.wgsl` lays it, at each slice's far face t, the path's light summed over the
// view's sources (rgb) and T's mean over the channels (a), `belowDeckView.wgsl`'s belowDeckPathTo,
// and T per channel beside it in the second volume (rgb, with the mean in a; plan R08, R08.T9.c).
// Follows `common.wgsl`, `view.wgsl`, `oblate.wgsl`, `belowDeck.wgsl` and `belowDeckView.wgsl`; it
// reads no medium.

@group(0) @binding(0) var<uniform> view : AtmosphereView;
@group(0) @binding(1) var oblateBody : texture_2d<f32>;
@group(0) @binding(2) var belowDeckTable : texture_2d_array<f32>;
@group(0) @binding(3) var aerialOut : texture_storage_3d<rgba16float, write>;
@group(0) @binding(4) var aerialTransmittanceOut : texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(aerialOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let body = oblateBodyOf(oblateBody);
  let sizeXy = vec2f(size.xy);
  let dir = rayThrough(view, unitFromTexelUv((vec2f(id.xy) + 0.5) / sizeXy, sizeXy));
  let fromM = belowDeckCameraHeightStar();
  let atCamera = belowDeckSkyAt(body, view.up.xyz, dir, fromM);
  let slices = size.z;
  let reach = view.tables.y;
  for (var slice = 0u; slice < slices; slice++) {
    let t = f32(slice + 1u) * reach / f32(slices);
    let path = belowDeckPathTo(body, dir, atCamera, fromM, t);
    textureStore(aerialOut, vec3u(id.xy, slice), path.light);
    textureStore(aerialTransmittanceOut, vec3u(id.xy, slice), vec4f(path.transmittance, path.light.a));
  }
}

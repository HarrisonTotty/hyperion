// The sky-view table beneath a cloud deck (plan R08, R08.T15.b.2; `thick/deckRead.ts`, "Below the
// deck"; `tablesCpu.ts`'s belowDeckSkyViewTexelsTwin): each texel's direction laid as `skyView.wgsl`
// lays it, on the column's sphere at the camera's height held as the sky view's is, and the table
// below the deck read along it at the camera's h* for each of the view's sources, in place of a
// march. Follows `common.wgsl`, `view.wgsl`, `oblate.wgsl`, `belowDeck.wgsl` and `belowDeckView.wgsl`;
// it reads no medium.

@group(0) @binding(0) var<uniform> view : AtmosphereView;
@group(0) @binding(1) var oblateBody : texture_2d<f32>;
@group(0) @binding(2) var belowDeckTable : texture_2d_array<f32>;
@group(0) @binding(3) var skyViewOut : texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(skyViewOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let sizeF = vec2f(size);
  let body = oblateBodyOf(oblateBody);
  let frame = skyFrame(view);
  let uv = (vec2f(id.xy) + 0.5) / sizeF;
  let azimuth = skyViewAzimuthOf(uv, sizeF);
  let horizontal = azimuth.x * frame.x + azimuth.y * frame.y;
  let heightM = skyViewHeight(view.camera.w, view.curvature.w);
  let column = skyViewColumnOf(
    view.curvature.x, view.curvature.y, frame.z, frame.x, horizontal, heightM
  );
  let params = skyViewUvToParams(uv, sizeF, column.y);
  let dir = vec3f(params.y * params.z, params.y * params.w, params.x);
  let bodyDir = dir.x * frame.x + dir.y * frame.y + dir.z * frame.z;
  let light = belowDeckSkyAt(body, frame.z, bodyDir, belowDeckCameraHeightStar());
  textureStore(skyViewOut, id.xy, vec4f(light, 1.0));
}

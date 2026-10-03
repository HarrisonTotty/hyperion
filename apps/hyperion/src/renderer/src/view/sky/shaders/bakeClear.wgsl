// Zeros a bake's scratch face before its splat, which adds to what the face holds (plan R06,
// T13.g; R01's splat loads its target).

struct Params {
  size : vec4u,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var face : texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= params.size.x || id.y >= params.size.x) {
    return;
  }
  textureStore(face, vec2i(id.xy), vec4f(0.0));
}

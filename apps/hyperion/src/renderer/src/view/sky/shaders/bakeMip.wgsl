// One mip step of a baked face (plan R06, T13.g): each texel the sum of its 2 × 2 children's flux
// and solid angle, or 3 × 3 at a 3,072² face's last step (`mips.ts`'s `mipStep`). Level 0 is the
// splat's own target, flux and a count, so the first step takes its children's solid angles from
// the buffer; every later level carries its solid angle in alpha.

struct Params {
  // The parent level's side, texels; the step, 2 or 3; 1 when the children are level 0; 0.
  size : vec4u,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var children : texture_storage_2d<rgba32float, read>;
@group(0) @binding(2) var parent : texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var<storage, read> omega : array<f32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = params.size.x;
  let step = params.size.y;
  let fromSplat = params.size.z == 1u;
  if (id.x >= size || id.y >= size) {
    return;
  }
  let childSize = size * step;
  var sum = vec4f(0.0);
  for (var dy = 0u; dy < step; dy += 1u) {
    for (var dx = 0u; dx < step; dx += 1u) {
      let at = id.xy * step + vec2u(dx, dy);
      let child = textureLoad(children, vec2i(at));
      // The buffer is read whatever the level, so that the binding is always used.
      let solidAngle = omega[min(at.y * childSize + at.x, arrayLength(&omega) - 1u)];
      sum += vec4f(child.rgb, select(child.a, solidAngle, fromSplat));
    }
  }
  textureStore(parent, vec2i(id.xy), sum);
}

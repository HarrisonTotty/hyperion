// One mip step of a baked face, or of a tile of one (plan R06, T13.g): each texel the sum of its
// 2 × 2 children's flux and solid angle, or 3 × 3 at a 3,072² face's last step (`mips.ts`'s
// `mipStep`). Level 0 is the splat's own target, flux and a count, so the first step takes its
// children's solid angles from the buffer; every later level carries its solid angle in alpha. A
// tile's step writes its parents where they lie in the parent texture. Composed before
// bakeOmega.wgsl.

struct Params {
  // The parents' side, texels; the step, 2 or 3; 1 when the children are level 0; 0.
  size : vec4u,
  // The children's origin on the face, texels, where they are level 0 (else 0, 0); the parents'
  // origin in the parent texture, texels.
  origin : vec4u,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var children : texture_storage_2d<rgba32float, read>;
@group(0) @binding(2) var parent : texture_storage_2d<rgba32float, write>;
// The face's solid angles, its lower triangle (bakeOmega.wgsl).
@group(0) @binding(3) var<storage, read> omega : array<f32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = params.size.x;
  let step = params.size.y;
  let fromSplat = params.size.z == 1u;
  if (id.x >= size || id.y >= size) {
    return;
  }
  var sum = vec4f(0.0);
  for (var dy = 0u; dy < step; dy += 1u) {
    for (var dx = 0u; dx < step; dx += 1u) {
      let at = id.xy * step + vec2u(dx, dy);
      let child = textureLoad(children, vec2i(at));
      // The buffer is read whatever the level, so that the binding is always used.
      let solidAngle = solidAngleAt(params.origin.xy + at);
      sum += vec4f(child.rgb, select(child.a, solidAngle, fromSplat));
    }
  }
  textureStore(parent, vec2i(params.origin.zw + id.xy), sum);
}

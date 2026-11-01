// A baked face's level, or a tile of one, packed to rgb9e5ufloat (plan R06, T13.g): each texel's
// luminance, its flux over its solid angle (the buffer's at level 0, the splat's target; alpha's
// above), scaled by the cube's power of two from the brightest texel, packed by bakeCommon.wgsl's
// packer into rows padded to 256 bytes for `copyBufferToTexture`. Composed after bakeCommon.wgsl
// and before bakeOmega.wgsl.

struct Params {
  // The level's or the tile's side, texels; a padded row's length, texels; 1 at level 0; 0.
  size : vec4u,
  // The tile's origin on the face at level 0, texels, where the solid angles are read (else 0, 0);
  // 0, 0.
  origin : vec4u,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var level : texture_storage_2d<rgba32float, read>;
@group(0) @binding(2) var<storage, read> peak : array<u32, 1>;
// The face's solid angles, its lower triangle (bakeOmega.wgsl).
@group(0) @binding(3) var<storage, read> omega : array<f32>;
@group(0) @binding(4) var<storage, read_write> packed : array<u32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = params.size.x;
  if (id.x >= size || id.y >= size) {
    return;
  }
  let texel = textureLoad(level, vec2i(id.xy));
  let solidAngle = select(texel.a, solidAngleAt(params.origin.xy + id.xy), params.size.z == 1u);
  let k = scaleExponent(bitcast<f32>(peak[0]));
  // Every texel has a positive solid angle, so the luminance is finite.
  let luminance = ldexp(texel.rgb / solidAngle, vec3i(k));
  packed[id.y * params.size.y + id.x] = packRgb9e5(luminance);
}

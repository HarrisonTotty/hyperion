// Bloom's upsample (plan R07, T14.b, Design note 12): U_m = w_m D_m + c up(U_{m+1}), the 3 x 3
// tent at 0 and +-1 coarse texels, weights (1, 2, 1) / 4 on each axis, each tap a bilinear sample
// made of four `textureLoad`s, clamped at the edges. `c` is the coarsest level's own weight on the
// first pass, which reads that level's downsample directly, and 1 after. It matches its CPU twin,
// `bloomUpTent` and `bloomChain` in `bloom.ts`, sample for sample.
//
// Drawn as a material on a full-screen triangle into level m's up target. The Frame
// (`frame.wgsl`) is concatenated ahead of this source.

struct Draw {
  offsetFromCameraM : vec3f,
  // w_m, this level's weight in the kernel.
  levelWeight : f32,
  // c, the coarse input's weight: the coarsest level's w on the first pass, 1 after.
  coarseWeight : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var level : texture_2d<f32>;
@group(2) @binding(1) var coarse : texture_2d<f32>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

fn coarseTexel(at : vec2i, size : vec2i) -> vec3f {
  return textureLoad(coarse, clamp(at, vec2i(0), size - 1), 0).rgb;
}

// A bilinear sample of the coarse level at continuous texel coordinates, clamped.
fn coarseBilinear(x : f32, y : f32, size : vec2i) -> vec3f {
  let u = x - 0.5;
  let v = y - 0.5;
  let i0 = i32(floor(u));
  let j0 = i32(floor(v));
  let fu = u - floor(u);
  let fv = v - floor(v);
  let a = coarseTexel(vec2i(i0, j0), size);
  let b = coarseTexel(vec2i(i0 + 1, j0), size);
  let c = coarseTexel(vec2i(i0, j0 + 1), size);
  let d = coarseTexel(vec2i(i0 + 1, j0 + 1), size);
  return (1.0 - fv) * ((1.0 - fu) * a + fu * b) + fv * ((1.0 - fu) * c + fu * d);
}

@fragment fn fragmentMain(@builtin(position) fragment : vec4f) -> @location(0) vec4f {
  let size = vec2i(textureDimensions(coarse));
  let centre = fragment.xy * vec2f(size) * frame.viewport.zw;
  let offsets = array<f32, 3>(-1.0, 0.0, 1.0);
  let tent = array<f32, 3>(0.25, 0.5, 0.25);
  var up = vec3f(0.0);
  for (var j = 0; j < 3; j++) {
    for (var i = 0; i < 3; i++) {
      up += tent[i] * tent[j] * coarseBilinear(centre.x + offsets[i], centre.y + offsets[j], size);
    }
  }
  let here = textureLoad(level, vec2i(fragment.xy), 0).rgb;
  return vec4f(draw.levelWeight * here + draw.coarseWeight * up, 1.0);
}

// Bloom's downsample (plan R07, T14.b, Design note 12): Jimenez 2014's 13 taps without the Karis
// average, from one mip level into the next, the first pass taking only the light above the
// threshold. Every tap is a bilinear sample made of four `textureLoad`s, clamped at the edges, so
// that the threshold applies per texel before any filtering (it is not linear) and the pass
// matches its CPU twin, `bloomDown` in `bloom.ts`, sample for sample.
//
// Drawn as a material on a full-screen triangle into the next level's target: `vertexMain` passes
// the mesh's clip-space corners through, and `fragmentMain` reads its texel from the fragment's
// position. The Frame (`frame.wgsl`) is concatenated ahead of this source.

struct Draw {
  offsetFromCameraM : vec3f,
  // The threshold in the source's units; 0 after the first pass, where every value is excess.
  threshold : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var source : texture_2d<f32>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

// The light above the threshold, per channel: L - min(L, T).
fn excess(value : vec3f) -> vec3f {
  return value - min(value, vec3f(draw.threshold));
}

fn texel(at : vec2i, size : vec2i) -> vec3f {
  return excess(textureLoad(source, clamp(at, vec2i(0), size - 1), 0).rgb);
}

// A bilinear sample at continuous texel coordinates (texel n's centre at n + 0.5), clamped.
fn bilinear(x : f32, y : f32, size : vec2i) -> vec3f {
  let u = x - 0.5;
  let v = y - 0.5;
  let i0 = i32(floor(u));
  let j0 = i32(floor(v));
  let fu = u - floor(u);
  let fv = v - floor(v);
  let a = texel(vec2i(i0, j0), size);
  let b = texel(vec2i(i0 + 1, j0), size);
  let c = texel(vec2i(i0, j0 + 1), size);
  let d = texel(vec2i(i0 + 1, j0 + 1), size);
  return (1.0 - fv) * ((1.0 - fu) * a + fu * b) + fv * ((1.0 - fu) * c + fu * d);
}

@fragment fn fragmentMain(@builtin(position) fragment : vec4f) -> @location(0) vec4f {
  let size = vec2i(textureDimensions(source));
  let scale = vec2f(size) * frame.viewport.zw;
  let centre = fragment.xy * scale;
  let outer = array<f32, 3>(-2.0, 0.0, 2.0);
  let tent = array<f32, 3>(0.25, 0.5, 0.25);
  var sum = vec3f(0.0);
  for (var j = 0; j < 3; j++) {
    for (var i = 0; i < 3; i++) {
      sum += 0.5 * tent[i] * tent[j] * bilinear(centre.x + outer[i], centre.y + outer[j], size);
    }
  }
  sum += 0.125 * bilinear(centre.x - 1.0, centre.y - 1.0, size);
  sum += 0.125 * bilinear(centre.x + 1.0, centre.y - 1.0, size);
  sum += 0.125 * bilinear(centre.x - 1.0, centre.y + 1.0, size);
  sum += 0.125 * bilinear(centre.x + 1.0, centre.y + 1.0, size);
  return vec4f(sum, 1.0);
}

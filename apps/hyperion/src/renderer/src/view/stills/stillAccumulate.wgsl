// A still's sample of one tile added to the still's assembled image (plan R11, R11.T10.e.2): a
// full-screen triangle over the image, scissored to the tile's rect, each pixel the tile's texel at
// the pixel less the tile's origin, times the sample's weight. A tile's first sample is drawn with
// no blend, its meter class in alpha (R07's `METER_CLASS`, which the bloom chain reads); each after
// it additively, at alpha 1, so that R01's additive blend keeps the image's alpha. Composed after
// frame.wgsl.

struct Draw {
  // Unused: the triangle is given in clip space.
  offsetFromCameraM : vec3f,
  // The sample's weight in the still, 1 ÷ n.
  weight : f32,
  // 1 for a tile's first sample, which writes its meter class; 0 for each after it.
  first : f32,
  // The tile's origin in the image, px, then 0 and 0.
  origin : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var tile : texture_2d<f32>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

@fragment fn fragmentMain(@builtin(position) fragment : vec4f) -> @location(0) vec4f {
  let texel = textureLoad(tile, vec2i(floor(fragment.xy - draw.origin.xy)), 0);
  return vec4f(draw.weight * texel.rgb, select(1.0, texel.a, draw.first > 0.5));
}

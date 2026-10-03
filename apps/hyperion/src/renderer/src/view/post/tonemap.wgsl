// The photorealistic style's last pass (plan R07, T15, Design notes 9, 12 and 13): the view's HDR
// colour upscaled from its internal resolution, the bloom chain's light above the display's range
// gathered back, each glare source's veil in closed form, the exposure, R02's AgX (`agx`, Filament's
// port, included unchanged from `toneCurve.wgsl`), the sRGB encoding, and a static blue-noise TPDF
// dither of +-1 LSB in the encoded domain. Drawn as a material on a full-screen triangle into the
// canvas through its own non-sRGB view (`FrameSubmission.encoding` "in-pass"), so that the dither
// is applied after the encoding. Its TypeScript twin is `tonemapTexel` in `tonemap.ts`.
//
// Concatenated after `frame.wgsl`, `toneCurve.wgsl` and `glare.wgsl`.

struct Draw {
  offsetFromCameraM : vec3f,
  // The exposure scale over the target's pre-exposure: a target value times this is AgX's input.
  exposure : f32,
  // The bloom threshold in the target's units.
  threshold : f32,
  // w0, the kernel's weight of level 0 (0 on the low setting).
  levelZeroWeight : f32,
  // U1's weight: 1, or the coarsest level's when that is level 1.
  levelOneWeight : f32,
  // The glare sources in `glareSources`.
  sourceCount : u32,
  glarePoisson0 : vec4f,
  glarePoisson1 : vec4f,
  glarePoisson2 : vec4f,
  glareBroad : vec4f,
  glareMisc : vec4f,
  // 1 to dither, 0 not to (the harness's twin check).
  dither : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var hdrColour : texture_2d<f32>;
@group(2) @binding(1) var hdrSampler : sampler;
@group(2) @binding(2) var bloomUp : texture_2d<f32>;
@group(2) @binding(3) var blueNoise : texture_2d<f32>;
@group(2) @binding(4) var<storage, read> glareSources : array<GlareSourceGpu>;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

// The sRGB encoding of a display-linear value (IEC 61966-2-1).
fn srgb_encode(linear : vec3f) -> vec3f {
  let low = linear * 12.92;
  let high = 1.055 * pow(linear, vec3f(1.0 / 2.4)) - 0.055;
  return select(high, low, linear <= vec3f(0.0031308));
}

// A triangular value in [-1, 1] from a uniform one in [0, 1], by the inverse of the triangular
// distribution's cumulative function, which keeps the tile's blue spectrum.
fn tpdf(u : f32) -> f32 {
  let v = clamp(u, 0.0, 1.0);
  return select(1.0 - sqrt(2.0 * (1.0 - v)), sqrt(2.0 * v) - 1.0, v < 0.5);
}

@fragment fn fragmentMain(@builtin(position) fragment : vec4f) -> @location(0) vec4f {
  let outputSize = frame.viewport.xy;
  let uv = fragment.xy * frame.viewport.zw;
  let hdr = textureSampleLevel(hdrColour, hdrSampler, uv, 0.0).rgb;
  var light = min(hdr, vec3f(draw.threshold));
  light += draw.levelZeroWeight * bloom_excess(hdr, draw.threshold);
  light += draw.levelOneWeight * bloom_tent(bloomUp, fragment.xy, outputSize);
  let terms = GlareTerms(
    draw.glarePoisson0,
    draw.glarePoisson1,
    draw.glarePoisson2,
    draw.glareBroad,
    draw.glareMisc,
  );
  let pixel = glare_pixel_direction(fragment.xy, frame.viewport, frame.clipProjection);
  for (var i = 0u; i < draw.sourceCount; i++) {
    var source = glareSources[i];
    source.direction = normalize((frame.viewRotation * vec4f(source.direction, 0.0)).xyz);
    light += glare_veil(source, pixel, terms);
  }
  // agxSprite, the curve less its floor, as the wireframe's sprites write it: an isolated star on
  // black is identical in both styles before the dither.
  let encoded = srgb_encode(agxSprite(light * draw.exposure));
  let tile = vec2u(textureDimensions(blueNoise));
  let at = vec2u(fragment.xy) % tile;
  // One tile, read at three offsets, so that the channels' noise is decorrelated.
  let noise = vec3f(
    tpdf(textureLoad(blueNoise, at, 0).r),
    tpdf(textureLoad(blueNoise, (at + tile / 3u) % tile, 0).r),
    tpdf(textureLoad(blueNoise, (at + 2u * tile / 3u) % tile, 0).r),
  );
  return vec4f(clamp(encoded + draw.dither * noise / 255.0, vec3f(0.0), vec3f(1.0)), 1.0);
}

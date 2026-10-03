// The spike's lit view's display pass (plan R05, T11.c, Design notes 17 and 26): R02's AgX (`agx`,
// from toneCurve.wgsl) over the pre-exposed linear colour the terrain pass drew, sampled bilinearly
// so that the low setting's 720p render is presented upscaled, on a full-screen triangle given in
// clip space. The view's `-srgb` view encodes the output.
// Concatenated after frame.wgsl and toneCurve.wgsl.

struct Draw {
  offsetFromCameraM : vec3f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@group(2) @binding(0) var hdrColour : texture_2d<f32>;
@group(2) @binding(1) var hdrSampler : sampler;

@vertex
fn vertexMain(@location(0) clip : vec3f) -> @builtin(position) vec4f {
  return vec4f(clip.xy, 0.0, 1.0);
}

@fragment
fn fragmentMain(@builtin(position) pixel : vec4f) -> @location(0) vec4f {
  // The output's pixel centre as a fraction of the output, which the render covers whole.
  let light = textureSample(hdrColour, hdrSampler, pixel.xy * frame.viewport.zw).rgb;
  return vec4f(agx(light), 1.0);
}

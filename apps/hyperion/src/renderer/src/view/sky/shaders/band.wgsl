// The sky's band (plan R06, Design note 15, T13.d): a full-screen draw that samples the band cube
// in each pixel's direction, bilinearly, and writes its pre-exposed linear luminance into the HDR
// scene target, drawn first, at infinity. Composed after frame.wgsl.
//
// The cube is on the galactic axes, as the view's frames are, so a pixel's direction is the view
// ray rotated back by the transpose of `frame.viewRotation`. Alpha 1: R01's additive blend keeps
// the destination's alpha, R07's meter class.

struct Draw {
  // Unused: the band is at infinity.
  offsetFromCameraM: vec3f,
  // The pre-exposure scale (R02's `exposureScale`), then 0, 0, 0.
  exposure: vec4f,
}

@group(1) @binding(0) var<uniform> draw: Draw;

// The band's luminance per channel, cd/m², with the culled stars' light, on a cube.
@group(2) @binding(0) var band: texture_cube<f32>;
@group(2) @binding(1) var bandSampler: sampler;

struct BandVarying {
  @builtin(position) position: vec4f,
  @location(0) ndc: vec2f,
}

@vertex
fn vertexMain(@location(0) position: vec3f) -> BandVarying {
  // A full-screen triangle given in clip space; depth 0, at infinity under reversed-Z.
  var out: BandVarying;
  out.position = vec4f(position.xy, 0.0, 1.0);
  out.ndc = position.xy;
  return out;
}

@fragment
fn fragmentMain(v: BandVarying) -> @location(0) vec4f {
  // The view ray through the pixel, right-handed, looking along −z.
  let ray = normalize(vec3f(
    v.ndc.x / frame.clipProjection[0][0],
    v.ndc.y / frame.clipProjection[1][1],
    -1.0,
  ));
  let rotation = mat3x3f(
    frame.viewRotation[0].xyz,
    frame.viewRotation[1].xyz,
    frame.viewRotation[2].xyz,
  );
  let galactic = transpose(rotation) * ray;
  let luminance = textureSample(band, bandSampler, galactic).rgb;
  return vec4f(luminance * draw.exposure.x, 1.0);
}

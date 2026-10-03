// The baked star cube's draw (plan R06, Design note 21, T13.g): a full-screen draw at infinity that
// samples the cube in each pixel's direction, trilinearly, divides out the bake's power of two, and
// pre-exposes the luminance. Composed after frame.wgsl and bakeCommon.wgsl, and then either
// toneCurve.wgsl and `CUBE_DISPLAY_TONE` (the wireframe's display variant, toned per pixel by
// R02's `agxSprite` as its sprites are) or `CUBE_HDR_TONE` (linear, into the HDR scene target).
// Additive with alpha 1: R01's blend keeps the destination's alpha, R07's meter class.

struct Draw {
  // Unused: the cube is at infinity.
  offsetFromCameraM : vec3f,
  // The pre-exposure scale, then 0, 0, 0.
  exposure : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@group(2) @binding(0) var stars : texture_cube<f32>;
@group(2) @binding(1) var starsSampler : sampler;
// The brightest level-0 luminance's f32 bits, from which the bake's power of two follows.
@group(2) @binding(2) var<storage, read> peak : array<u32, 1>;

struct CubeVarying {
  @builtin(position) position : vec4f,
  @location(0) ndc : vec2f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> CubeVarying {
  var out : CubeVarying;
  out.position = vec4f(position.xy, 0.0, 1.0);
  out.ndc = position.xy;
  return out;
}

@fragment
fn fragmentMain(v : CubeVarying) -> @location(0) vec4f {
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
  let k = scaleExponent(bitcast<f32>(peak[0]));
  let luminance = ldexp(textureSample(stars, starsSampler, galactic).rgb, vec3i(-k));
  return vec4f(cubeTone(luminance * draw.exposure.x), 1.0);
}

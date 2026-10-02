// The stars as sprites (plan R02, Design notes 10 and 12): a 7 x 7 px quad about each star's pixel,
// each pixel lit by the star's pre-exposed colour times the pixel-integrated Gaussian point-spread
// weight at the star's sub-pixel position, toned per sprite by `agxSprite` and added in linear
// light through the canvas's sRGB view. Depth-tested at infinity (depth 0), so that an occluder
// hides it; no depth write. Composed after frame.wgsl and toneCurve.wgsl.

struct Draw {
  // Unused: each sprite carries its own position.
  offsetFromCameraM: vec3f,
}

@group(1) @binding(0) var<uniform> draw: Draw;

// Two per sprite: its position, px from the view's top left with its sub-pixel part, then 0, 0;
// its pre-exposed linear Rec. 709 colour per unit of point-spread weight, then 0.
@group(2) @binding(0) var<storage, read> sprites: array<vec4f>;

// The point-spread function's standard deviation, px: a full width at half maximum of 1.5 px
// (`PSF_SIGMA_PX`, Design note 10).
const PSF_SIGMA_PX = 0.64;

// The quad's width, px (`PSF_QUAD_PX`), whose tails beyond hold under 3e-6 of the light.
const PSF_QUAD_PX = 7.0;

// The error function, Abramowitz and Stegun 7.1.26: absolute error under 1.5e-7, below an f32's
// resolution of the weights that matter.
fn erfApprox(x: f32) -> f32 {
  let s = sign(x);
  let a = abs(x);
  let t = 1.0 / (1.0 + 0.3275911 * a);
  let poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027
    + t * 1.061405429))));
  return s * (1.0 - poly * exp(-a * a));
}

// The share of a unit Gaussian's light, centred at `centre`, falling in the pixel [p - 0.5,
// p + 0.5] about the pixel centre p.
fn pixelWeight(p: f32, centre: f32) -> f32 {
  let k = 1.0 / (PSF_SIGMA_PX * sqrt(2.0));
  return 0.5 * (erfApprox((p + 0.5 - centre) * k) - erfApprox((p - 0.5 - centre) * k));
}

struct SpriteVarying {
  @builtin(position) position: vec4f,
  @location(0) @interpolate(flat) sprite: u32,
}

@vertex
fn vertexMain(
  @location(0) corner: vec3f,
  @builtin(instance_index) instance: u32,
) -> SpriteVarying {
  let at = sprites[instance * 2u].xy;
  // The star's own pixel and three on each side.
  let first = floor(at) - vec2f(floor(PSF_QUAD_PX * 0.5));
  let px = first + corner.xy * PSF_QUAD_PX;
  let ndc = vec2f(px.x / frame.viewport.x * 2.0 - 1.0, 1.0 - px.y / frame.viewport.y * 2.0);
  var out: SpriteVarying;
  // Depth 0: at infinity under reversed-Z.
  out.position = vec4f(ndc, 0.0, 1.0);
  out.sprite = instance;
  return out;
}

@fragment
fn fragmentMain(v: SpriteVarying) -> @location(0) vec4f {
  let at = sprites[v.sprite * 2u].xy;
  let rgb = sprites[v.sprite * 2u + 1u].xyz;
  let weight = pixelWeight(v.position.x, at.x) * pixelWeight(v.position.y, at.y);
  // Alpha 1: R01's additive blend scales the colour by the source alpha and keeps the
  // destination's alpha (R07's meter class) whatever this writes.
  return vec4f(agxSprite(rgb * weight), 1.0);
}

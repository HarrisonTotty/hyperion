// A host star's limb-darkened disc (plan R06, Design note 16, T13.e): a full-screen draw at
// infinity that lights the pixels whose view ray falls within the disc's angular radius, by the
// power-2 law I(μ) = I(1) (1 − c (1 − μ^α)) per channel, pre-exposed and clamped at a half
// float's 65,504 (the energy above it goes to R07's glare pass), into the HDR scene target. It
// writes R07's meter class `hostDisc`, 0, into the alpha, with no blend, so that the meter leaves
// the disc out. Composed after frame.wgsl.

struct Draw {
  // Unused: the disc is at infinity.
  offsetFromCameraM: vec3f,
  // The unit direction to the disc's centre on the galactic axes, then sin ρ.
  axis: vec4f,
  // I(1) per linear Rec. 709 channel, cd/m², then 0.
  central: vec4f,
  // The law's c per channel, then 0.
  limbC: vec4f,
  // The law's α per channel, then 0.
  limbAlpha: vec4f,
  // The pre-exposure scale, then 0, 0, 0.
  exposure: vec4f,
}

@group(1) @binding(0) var<uniform> draw: Draw;

// A half float's largest value: what a pre-exposed texel is clamped at.
const HALF_MAX = 65504.0;

// R07's `METER_CLASS.hostDisc`.
const METER_CLASS_HOST_DISC = 0.0;

struct DiscVarying {
  @builtin(position) position: vec4f,
  @location(0) ndc: vec2f,
}

@vertex
fn vertexMain(@location(0) position: vec3f) -> DiscVarying {
  // A full-screen triangle given in clip space; depth 0, at infinity under reversed-Z, so that a
  // body in front hides the disc.
  var out: DiscVarying;
  out.position = vec4f(position.xy, 0.0, 1.0);
  out.ndc = position.xy;
  return out;
}

@fragment
fn fragmentMain(v: DiscVarying) -> @location(0) vec4f {
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
  let axis = draw.axis.xyz;
  let sinRho = draw.axis.w;
  // sin θ from the cross product, which keeps its precision for the Sun's 0.27° where 1 − cos θ
  // would not in f32.
  let sinTheta = length(cross(galactic, axis));
  if (dot(galactic, axis) <= 0.0 || sinTheta >= sinRho) {
    discard;
  }
  let q = sinTheta / sinRho;
  let mu = sqrt(max(0.0, 1.0 - q * q));
  let law = vec3f(1.0) - draw.limbC.xyz * (vec3f(1.0) - pow(vec3f(max(mu, 1e-6)), draw.limbAlpha.xyz));
  let luminance = draw.central.xyz * law * draw.exposure.x;
  return vec4f(min(luminance, vec3f(HALF_MAX)), METER_CLASS_HOST_DISC);
}

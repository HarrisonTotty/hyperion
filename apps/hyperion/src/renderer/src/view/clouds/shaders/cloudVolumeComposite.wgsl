// The cloud volume's composite (plan R11, R11.T7.c): its resolved clouds (`cloudVolumeResolve.wgsl`),
// at a quarter of the view's pixels, laid over the body's air at the view's own, into the target the
// body's composite drew, blended premultiplied so that the meter class beneath stays, as the
// two-dimensional layer is (`cloudLayer.wgsl`). The resolved light is read bilinear; a pixel whose
// scene stands nearer than the ray's entry into the deck takes none, so that ground in front of
// the clouds keeps its edge at the view's own resolution. Writes no depth.

struct Draw {
  offsetFromCameraM : vec3f,
  // xyz: the camera from the body's centre, body-fixed, m.
  camera : vec4f,
  // The ray basis, body-fixed: right × tan(fovX ÷ 2), up × tan(fovY ÷ 2), forward.
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  // x: the deck's a ÷ c, k; y, z: |S o|² − r², S = diag(1, 1, k), at its top's and its base's
  // radii, formed in f64; w: the near plane, m.
  deck : vec4f,
  // x, y: the output's size, px.
  view : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneDepth : texture_depth_2d;
@group(2) @binding(1) var cloud_resolved : texture_2d<f32>;
@group(2) @binding(2) var cloud_resolved_sampler : sampler;

// The share below which a pixel draws nothing (`cloudLayer.wgsl`'s CLOUD_LEAST_DRAWN).
const CLOUD_COMPOSITE_LEAST_DRAWN : f32 = 1e-4;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

// The distance along unit `d` from `o` at which the ray enters the deck: at its top from above, at
// its base from below, 0 within it; -1 where it never does.
fn cloud_deck_entry(o : vec3f, d : vec3f) -> f32 {
  let k = draw.deck.x;
  let so = vec3f(o.xy, o.z * k);
  let sd = vec3f(d.xy, d.z * k);
  let a = dot(sd, sd);
  let b = dot(so, sd);
  let top = draw.deck.y;
  let base = draw.deck.z;
  if (top > 0.0) {
    let disc = b * b - a * top;
    if (disc < 0.0 || b >= 0.0) {
      return -1.0;
    }
    return top / (-b + sqrt(disc));
  }
  if (base < 0.0) {
    let disc = b * b - a * base;
    return (-b + sqrt(max(disc, 0.0))) / a;
  }
  return 0.0;
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let uv = position.xy / draw.view.xy;
  let clouds = textureSampleLevel(cloud_resolved, cloud_resolved_sampler, uv, 0.0);
  if (clouds.a < CLOUD_COMPOSITE_LEAST_DRAWN) {
    discard;
  }
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let unnormalised = draw.forward.xyz + ndc.x * draw.right.xyz + ndc.y * draw.upRay.xyz;
  let d = normalize(unnormalised);
  let entry = cloud_deck_entry(draw.camera.xyz, d);
  // The scene's depth stands in front of the deck: reversed-Z, 0 at infinity.
  let depth = textureLoad(sceneDepth, vec2i(position.xy), 0);
  if (entry < 0.0 || (depth > 0.0 && draw.deck.w / depth * length(unnormalised) <= entry)) {
    discard;
  }
  return clouds;
}

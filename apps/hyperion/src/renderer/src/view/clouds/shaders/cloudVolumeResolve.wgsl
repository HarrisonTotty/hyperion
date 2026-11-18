// The cloud volume's temporal resolve (plan R11, R11.T7.c; Design note 10's quarter resolution with
// temporal reprojection): each pixel of the march's target (`cloudVolumeMarch.wgsl`) blended with
// the last frame's resolved clouds where the same cloud stood then, into the other of the volume's
// two histories, which the composite reads (`cloudVolumeComposite.wgsl`).
//
// The cloud a pixel sees is taken to stand where its ray meets the layer's shell, as the layer
// draws it (`cloudLayer.wgsl`), or far along the ray where the ray misses it; that place is carried
// into the last frame's camera, `moved` its place less the last camera's, formed in f64, and the
// last frame's resolved light read there, bilinear, then held within the range of this frame's
// march over the pixel's 3 × 3 neighbours, so that a cloud the reprojection mistakes leaves no
// trail. The blend keeps `weight` of this frame's march (`cloudVolumePass.ts`' TEMPORAL_WEIGHT);
// where no history holds (the first frame, a cut, a new body or size) the march stands alone.
// The twin of the reprojection is `cloudVolumePass.ts`' `reprojectedUv`.

struct Draw {
  offsetFromCameraM : vec3f,
  // xyz: the camera from the body's centre, body-fixed, m; w: 1 where a history holds, else 0.
  camera : vec4f,
  // This frame's ray basis, body-fixed: right × tan(fovX ÷ 2), up × tan(fovY ÷ 2), forward.
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  // x: |S o|² − a_s², S = diag(1, 1, a_s ÷ c_s), formed in f64; y: a_s ÷ c_s; z: unused; w: the
  // weight of this frame's march.
  shell : vec4f,
  // xyz: this camera's place less the last frame's, body-fixed, m, formed in f64.
  moved : vec4f,
  // The last frame's camera basis: right ÷ tan(fovX ÷ 2), up ÷ tan(fovY ÷ 2), forward, body-fixed.
  lastRight : vec4f,
  lastUp : vec4f,
  lastForward : vec4f,
  // x, y: the target's size, px.
  view : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var cloud_march : texture_2d<f32>;
@group(2) @binding(1) var cloud_history : texture_2d<f32>;
@group(2) @binding(2) var cloud_history_sampler : sampler;

// How far along a ray that misses the layer's shell its cloud is taken to stand, m.
const CLOUD_RESOLVE_FAR_M : f32 = 1e9;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let size = vec2i(draw.view.xy);
  let p = vec2i(position.xy);
  let now = textureLoad(cloud_march, p, 0);
  if (draw.camera.w < 0.5) {
    return now;
  }
  var low = now;
  var high = now;
  for (var dy = -1; dy <= 1; dy = dy + 1) {
    for (var dx = -1; dx <= 1; dx = dx + 1) {
      let q = clamp(p + vec2i(dx, dy), vec2i(0), size - vec2i(1));
      let neighbour = textureLoad(cloud_march, q, 0);
      low = min(low, neighbour);
      high = max(high, neighbour);
    }
  }
  let uv = position.xy / draw.view.xy;
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let d = normalize(draw.forward.xyz + ndc.x * draw.right.xyz + ndc.y * draw.upRay.xyz);
  // The ray's crossing of the layer's shell (`cloudLayer.wgsl`'s solve).
  let o = draw.camera.xyz;
  let k = draw.shell.y;
  let so = vec3f(o.xy, o.z * k);
  let sd = vec3f(d.xy, d.z * k);
  let a = dot(sd, sd);
  let b = dot(so, sd);
  let c = draw.shell.x;
  let disc = b * b - a * c;
  var t = CLOUD_RESOLVE_FAR_M;
  if (disc >= 0.0) {
    let root = sqrt(disc);
    if (c > 0.0 && b < 0.0) {
      t = c / (-b + root);
    } else if (c <= 0.0) {
      t = (-b + root) / a;
    }
  }
  // The place from the last frame's camera, into its view.
  let v = draw.moved.xyz + t * d;
  let z = dot(v, draw.lastForward.xyz);
  if (!(z > 0.0)) {
    return now;
  }
  let last = vec2f(dot(v, draw.lastRight.xyz), dot(v, draw.lastUp.xyz)) / z;
  if (any(abs(last) > vec2f(1.0))) {
    return now;
  }
  let at = vec2f(0.5 + 0.5 * last.x, 0.5 - 0.5 * last.y);
  let held = clamp(textureSampleLevel(cloud_history, cloud_history_sampler, at, 0.0), low, high);
  return mix(held, now, draw.shell.w);
}

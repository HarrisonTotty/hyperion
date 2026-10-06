// The atmosphere's composite (plan R05, Design note 16): one full-screen draw over the view that
// reads the terrain target's colour and reversed-Z depth and lays the atmosphere over them: the sky
// from the sky-view table (or the ray march above the atmosphere) with the sun's disc where depth
// is at the far plane; the surface through the aerial-perspective volume within its reach and the
// ray march beyond it. Its output is pre-exposed luminance, the sun's disc clamped to rgba16float's
// largest value after pre-exposure. Follows `common.wgsl` (for the shell's geometry and
// `bilinear`) and `view.wgsl`, without the medium's chunks (`medium.wgsl`, `source.wgsl`), since it
// declares no medium; drawn with no depth test, as a full-screen draw into another target (the
// terrain target's depth may not be sampled by a draw into itself).
//
// Ported from Bevy 0.19.1's render_sky.wgsl (MIT; the notice is in common.wgsl). Changes: the
// transmittance over the aerial-perspective volume is its stored mean, sebh's, rather than Bevy's
// dual-source blend; the half-resolution ray march is upsampled by depth; the sun's disc is
// clamped.

struct Draw {
  offsetFromCameraM : vec3f,
  camera : vec4f,
  up : vec4f,
  sun : vec4f,
  skyScale : vec4f,
  sunDisc : vec4f,
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  tables : vec4f,
  figure : vec4f,
  output : vec4f,
  sunOverSky : vec4f,
  // x: the tables' top radius; y: the sky view's ground, the camera's Gaussian radius sqrt(MN);
  // z: the atmosphere's height, m; w: unused.
  shell : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneColour : texture_2d<f32>;
@group(2) @binding(1) var sceneDepth : texture_depth_2d;
@group(2) @binding(2) var skyView : texture_2d<f32>;
@group(2) @binding(3) var aerial : texture_3d<f32>;
@group(2) @binding(4) var rayMarch : texture_2d<f32>;
@group(2) @binding(5) var transmittance : texture_2d<f32>;
@group(2) @binding(6) var linearClamp : sampler;

const HALF_MAX : f32 = 65504.0;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  // A full-screen triangle given in clip space; z at the near plane, drawn with no depth test.
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

fn viewOf() -> AtmosphereView {
  return AtmosphereView(
    draw.camera,
    draw.up,
    draw.sun,
    draw.skyScale,
    draw.sunDisc,
    draw.right,
    draw.upRay,
    draw.forward,
    draw.tables,
    draw.figure,
    draw.output,
    draw.sunOverSky,
  );
}

// The transmittance from the camera along `dir` to space, per channel, on the tables' sphere.
fn transmittanceToSpace(v : AtmosphereView, dir : vec3f) -> vec3f {
  let bottom = v.tables.x;
  let top = draw.shell.x;
  let mu = dot(dir, v.up.xyz);
  let heightM = clamp(v.camera.w, 0.0, top - bottom);
  let r = bottom + heightM;
  if (groundHitFromHeight(bottom, heightM, mu)) {
    return vec3f(0.0);
  }
  return bilinear(transmittance, shellRMuToUv(bottom, top, r, mu)).rgb;
}

// The ray march's texel for this pixel: at full resolution its own; at half, of the four nearest,
// the one whose source pixel's depth is closest to this pixel's.
fn rayMarched(pixel : vec2u, depth : f32) -> vec4f {
  let scale = draw.output.z;
  if (scale >= 1.0) {
    return textureLoad(rayMarch, pixel, 0);
  }
  // The target may be larger than this frame's output; the kernel filled only its share.
  let size = vec2u(ceil(draw.output.xy * scale));
  let full = vec2u(draw.output.xy);
  let base = vec2i(vec2f(pixel) * scale - 0.5);
  var best = vec4f(0.0, 0.0, 0.0, 1.0);
  var bestError = 3.4e38;
  for (var j = 0; j < 2; j++) {
    for (var i = 0; i < 2; i++) {
      let t = vec2u(clamp(base + vec2i(i, j), vec2i(0), vec2i(size) - 1));
      let source = min(vec2u((vec2f(t) + 0.5) / scale), full - 1u);
      let error = abs(textureLoad(sceneDepth, source, 0) - depth);
      if (error < bestError) {
        bestError = error;
        best = textureLoad(rayMarch, t, 0);
      }
    }
  }
  return best;
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let v = viewOf();
  let pixel = vec2u(position.xy);
  let uv = position.xy / v.output.xy;
  let depth = textureLoad(sceneDepth, pixel, 0);
  let colour = textureLoad(sceneColour, pixel, 0).rgb;
  let dir = rayThrough(v, uv);
  let exposure = v.skyScale.w;
  let topM = draw.shell.z;
  let aboveTop = v.camera.w >= topM;

  if (depth == 0.0) {
    var inscattered : vec3f;
    var toSpace : vec3f;
    if (aboveTop) {
      let marched = rayMarched(pixel, depth);
      inscattered = marched.rgb;
      toSpace = vec3f(marched.a);
    } else {
      let bottom = draw.shell.y;
      let heightM = skyViewHeight(v.camera.w, topM);
      let up = v.up.xyz;
      let viewZenithCos = dot(dir, up);
      let frame = skyFrame(v);
      let horizontal = vec2f(dot(dir, frame.x), dot(dir, frame.y));
      var lightViewCos = 1.0;
      if (dot(horizontal, horizontal) > 1e-12) {
        lightViewCos = normalize(horizontal).x;
      }
      let ground = groundHitFromHeight(bottom, heightM, viewZenithCos);
      let size = vec2f(textureDimensions(skyView));
      let skyUv = skyViewParamsToUv(ground, viewZenithCos, lightViewCos, size, heightM, bottom);
      inscattered = textureSampleLevel(skyView, linearClamp, skyUv, 0.0).rgb;
      toSpace = transmittanceToSpace(v, dir);
    }
    var luminance = inscattered * v.skyScale.rgb * exposure;
    // The sun's disc, with its transmittance, clamped after pre-exposure.
    let cosToSun = dot(dir, v.sun.xyz);
    if (cosToSun >= cos(v.sun.w)) {
      luminance += min(v.sunDisc.rgb * toSpace * exposure, vec3f(HALF_MAX));
    }
    return vec4f(min(luminance, vec3f(HALF_MAX)), 1.0);
  }

  let distance = distanceAtDepth(v, uv, depth);
  let reach = v.tables.y;
  if (distance <= reach && !aboveTop) {
    let slices = v.tables.z;
    let w = clamp(distance / reach - 0.5 / slices, 0.0, 1.0);
    let sampled = textureSampleLevel(aerial, linearClamp, vec3f(uv, w), 0.0);
    // Within the first slice, fade from the camera, where nothing has scattered yet.
    let fade = clamp(distance / (reach / slices), 0.0, 1.0);
    let inscattered = sampled.rgb * fade;
    let meanTransmittance = mix(1.0, sampled.a, fade);
    let luminance = colour * meanTransmittance + inscattered * v.skyScale.rgb * exposure;
    return vec4f(min(luminance, vec3f(HALF_MAX)), 1.0);
  }
  let marched = rayMarched(pixel, depth);
  let luminance = colour * marched.a + marched.rgb * v.skyScale.rgb * exposure;
  return vec4f(min(luminance, vec3f(HALF_MAX)), 1.0);
}

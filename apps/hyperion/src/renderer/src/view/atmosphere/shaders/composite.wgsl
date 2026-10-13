// The atmosphere's composite (plan R05, Design note 16): one full-screen draw over the view that
// reads the terrain target's colour and reversed-Z depth and lays the atmosphere over them: the sky
// from the sky-view table (or the ray march above the atmosphere) with the sun's disc where depth
// is at the far plane; the surface through the aerial-perspective volume within its reach and the
// ray march beyond it. Its output is pre-exposed luminance, the sun's disc clamped to rgba16float's
// largest value after pre-exposure. Follows `view/shaders/preciseTrig.wgsl`, `common.wgsl` (for the
// shell's geometry, the texel mapping and `bilinear`) and `view.wgsl`, without the medium's chunks
// (`medium.wgsl`, `source.wgsl`), since it declares no medium; drawn with no depth test, as a
// full-screen draw into another target (the terrain target's depth may not be sampled by a draw
// into itself).
//
// Ported from Bevy 0.19.1's render_sky.wgsl (MIT; the notice is in common.wgsl). Changes: the
// transmittance over the aerial-perspective volume is its stored mean, sebh's, rather than Bevy's
// dual-source blend; the half-resolution ray march is upsampled by depth; the sun's disc is
// clamped. Changes (plan R08, R08.T6.g): the transmittance to space and the sky view's ground are
// taken from the camera's height (Design note 18); the sky view is read at the ray's zenith,
// preciseAtan2(|d x up|, d . up), and its azimuth's half-angle sine, the chord, so that the read
// inverts the kernel's write; the sun's disc is |d x s| <= sin(rho) with d . s > 0, sin(rho) formed
// in f64, in place of cos(theta) >= cos(rho), which f32 cannot resolve for a disc under 3.5e-4 rad
// (the Sun beyond about 13 au) and whose builtin cos WGSL bounds only to 2^-11; every table is read
// through Bruneton's texel mapping (Design note 19), the aerial volume's screen axes included.
// Changes (plan R08, R08.T6.c, Design notes 5 and 8): the transmittance to space is the table's
// optical depth with the sun's curves of growth, exponentiated here.

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
  exact : vec4f,
  // x: the atmosphere's height H above the ground, m, formed in f64; y: how many of the medium's
  // terms are absorbers, 0 to ABSORBER_SLOTS (R08.T6.c); z, w: unused.
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
@group(2) @binding(7) var absorberColumns : texture_2d<f32>;
@group(2) @binding(8) var absorberCurves : texture_2d<f32>;

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
    draw.exact,
  );
}

// The sun's transmittance from the camera along `dir` to space, per channel, on the tables'
// sphere: the table's optical depth and absorber columns, with the sun's curves of growth (light
// slot 0), exponentiated in f32 (`transmittanceThroughTables`, R08.T6.c), so that the disc's light
// is never a stored half float.
fn transmittanceToSpace(v : AtmosphereView, dir : vec3f) -> vec3f {
  let bottom = v.tables.x;
  let topM = draw.shell.x;
  let mu = dot(dir, v.up.xyz);
  let heightM = clamp(v.camera.w, 0.0, topM);
  if (groundHitFromHeight(bottom, heightM, mu)) {
    return vec3f(0.0);
  }
  let unit = transmittanceUnitAt(bottom, topM, heightM, mu);
  let count = min(u32(draw.shell.y), ABSORBER_SLOTS);
  return transmittanceThroughTables(
    transmittance, absorberColumns, absorberCurves, count, 0u, unit
  );
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
  let topM = draw.shell.x;
  let aboveTop = v.camera.w >= topM;

  if (depth == 0.0) {
    var inscattered : vec3f;
    var toSpace : vec3f;
    if (aboveTop) {
      let marched = rayMarched(pixel, depth);
      inscattered = marched.rgb;
      toSpace = vec3f(marched.a);
    } else {
      // The sky view's sphere: the camera's Gaussian radius, and its height there.
      let bottom = v.up.w;
      let heightM = skyViewHeight(v.camera.w, topM);
      let up = v.up.xyz;
      let viewZenithCos = dot(dir, up);
      let viewZenith = preciseAtan2(length(cross(dir, up)), viewZenithCos);
      let frame = skyFrame(v);
      let horizontal = vec2f(dot(dir, frame.x), dot(dir, frame.y));
      var azimuthHalfSine = 0.0;
      if (dot(horizontal, horizontal) > 1e-12) {
        azimuthHalfSine = 0.5 * length(normalize(horizontal) - vec2f(1.0, 0.0));
      }
      let ground = groundHitFromHeight(bottom, heightM, viewZenithCos);
      let size = vec2f(textureDimensions(skyView));
      let skyUv = skyViewParamsToUv(ground, viewZenith, azimuthHalfSine, size, v.exact.z);
      inscattered = textureSampleLevel(skyView, linearClamp, skyUv, 0.0).rgb;
      toSpace = transmittanceToSpace(v, dir);
    }
    var luminance = inscattered * v.skyScale.rgb * exposure;
    // The sun's disc, with its transmittance, clamped after pre-exposure: |d x s| <= sin(rho).
    let cosToSun = dot(dir, v.sun.xyz);
    if (cosToSun > 0.0 && length(cross(dir, v.sun.xyz)) <= v.exact.w) {
      luminance += min(v.sunDisc.rgb * toSpace * exposure, vec3f(HALF_MAX));
    }
    return vec4f(min(luminance, vec3f(HALF_MAX)), 1.0);
  }

  let distance = distanceAtDepth(v, uv, depth);
  let reach = v.tables.y;
  if (distance <= reach && !aboveTop) {
    let slices = v.tables.z;
    // The slice axis is Bruneton's mapping already: slice k holds the march to (k + 1) reach /
    // slices, so the unit (slices distance / reach - 1) / (slices - 1) reads at this w. The
    // screen axes take it here, the froxels standing at (i, j) / (size - 1) of the screen.
    let w = clamp(distance / reach - 0.5 / slices, 0.0, 1.0);
    let aerialSize = vec2f(textureDimensions(aerial).xy);
    let screen = texelUvFromUnit(uv, aerialSize);
    let sampled = textureSampleLevel(aerial, linearClamp, vec3f(screen, w), 0.0);
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

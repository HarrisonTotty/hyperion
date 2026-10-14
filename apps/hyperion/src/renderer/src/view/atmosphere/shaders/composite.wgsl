// The atmosphere's composite (plan R05, Design note 16): one full-screen draw over the view that
// reads the terrain target's colour and reversed-Z depth and lays the atmosphere over them: the sky
// from the sky-view table (or the ray march above the atmosphere) with the sun's disc where depth
// is at the far plane; the surface through the aerial-perspective volume within its reach and the
// ray march beyond it. Its output is pre-exposed luminance, the sun's disc clamped to rgba16float's
// largest value after pre-exposure. Follows `view/shaders/preciseTrig.wgsl`, `common.wgsl` (for the
// shell's geometry, the texel mapping and `bilinear`), `view.wgsl` and `oblate.wgsl` (R08.T6.f),
// without the medium's chunks
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
// optical depth with the sun's curves of growth, exponentiated here. Changes (plan R08, R08.T9.a):
// the volume's read moved, unchanged, to `aerialRead.wgsl`, which follows `view.wgsl` here and
// which other readers of the volume take.
// Changes (plan R08, R08.T7, Design note 7): the sky and the surfaces take the per-view tables
// summed over the drawn sources, which hold pre-exposed luminance (`view.sky.w` the factor, 1); a
// sky pixel's scene colour, R06's sprites, band, cube and host discs drawn beneath, is multiplied by
// the transmittance from the camera along the pixel's ray, inside a source's disc that source's own
// (its light slot's curves) and elsewhere light slot 0's; each source the composite draws a disc
// for adds it through its own transmittance; the sky view is read at the ray's local azimuth; and
// the output's alpha is the scene's, R07's METER_CLASS, which the pass keeps.
// Changes (plan R08, R08.T6.f, Design note 17): it follows `oblate.wgsl` too. The sky view is read on
// the sphere of the ray's own azimuth at the camera, R_alpha, with its own horizon split
// (`skyViewColumnOf`), the camera's height on it held under the medium's top over the camera's s;
// the transmittance to space is the sliced lookup's from the camera along the ray, the camera's
// local scale height formed in f64 (`view.exact.z`); and the camera is above the air where its h*
// is above the top.

struct Draw {
  offsetFromCameraM : vec3f,
  camera : vec4f,
  up : vec4f,
  sky : vec4f,
  skyAxis : vec4f,
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  tables : vec4f,
  figure : vec4f,
  output : vec4f,
  exact : vec4f,
  curvature : vec4f,
  // The sources, `AtmosphereView.sources` member by member (R08.T7).
  sourceDirection0 : vec4f,
  sourceWeight0 : vec4f,
  sourceDisc0 : vec4f,
  sourceDirection1 : vec4f,
  sourceWeight1 : vec4f,
  sourceDisc1 : vec4f,
  sourceDirection2 : vec4f,
  sourceWeight2 : vec4f,
  sourceDisc2 : vec4f,
  sourceDirection3 : vec4f,
  sourceWeight3 : vec4f,
  sourceDisc3 : vec4f,
  sourceDirection4 : vec4f,
  sourceWeight4 : vec4f,
  sourceDisc4 : vec4f,
  sourceDirection5 : vec4f,
  sourceWeight5 : vec4f,
  sourceDisc5 : vec4f,
  // x: the atmosphere's height H above the ground, m, formed in f64; y: how many of the medium's
  // terms are absorbers, 0 to ABSORBER_SLOTS (R08.T6.c); z: the medium's line slots, the bitmask
  // `common.wgsl`'s `pressureChannelOf` reads (R08.T4.d); w: unused.
  shell : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneColour : texture_2d<f32>;
@group(2) @binding(1) var sceneDepth : texture_depth_2d;
@group(2) @binding(2) var skyView : texture_2d<f32>;
@group(2) @binding(3) var aerial : texture_3d<f32>;
@group(2) @binding(4) var rayMarch : texture_2d<f32>;
@group(2) @binding(5) var transmittance : texture_2d_array<f32>;
@group(2) @binding(6) var linearClamp : sampler;
@group(2) @binding(7) var absorberColumns : texture_2d_array<f32>;
@group(2) @binding(8) var absorberCurves : texture_2d<f32>;
@group(2) @binding(9) var absorberPressureColumns : texture_2d_array<f32>;
@group(2) @binding(10) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(2) @binding(11) var oblateBody : texture_2d<f32>;

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
    draw.sky,
    draw.skyAxis,
    draw.right,
    draw.upRay,
    draw.forward,
    draw.tables,
    draw.figure,
    draw.output,
    draw.exact,
    draw.curvature,
    array<SkySource, MAX_SKY_SOURCES>(
      SkySource(draw.sourceDirection0, draw.sourceWeight0, draw.sourceDisc0),
      SkySource(draw.sourceDirection1, draw.sourceWeight1, draw.sourceDisc1),
      SkySource(draw.sourceDirection2, draw.sourceWeight2, draw.sourceDisc2),
      SkySource(draw.sourceDirection3, draw.sourceWeight3, draw.sourceDisc3),
      SkySource(draw.sourceDirection4, draw.sourceWeight4, draw.sourceDisc4),
      SkySource(draw.sourceDirection5, draw.sourceWeight5, draw.sourceDisc5),
    ),
  );
}

// A light's transmittance from the camera along `dir` to space, per channel: the sliced lookup's
// from the camera (R08.T6.f; `oblate.wgsl`), the slices' optical depth, absorber columns and
// pressure-weighted columns over s, with light slot `slot`'s curves of growth (R08.T6.c, R08.T4.d;
// each light's own since R08.T7), exponentiated in f32, so that a disc's light is never a stored
// half float; 0 where the ground hides space.
fn transmittanceToSpace(v : AtmosphereView, body : OblateBody, dir : vec3f, slot : u32) -> vec3f {
  let n = v.up.xyz;
  let start = LookupStart(
    v.camera.xyz, oblateScaled(body, v.camera.xyz), v.exact.x, v.camera.w, n, n.z, length(n.xy),
    v.curvature.z, v.camera.w * v.curvature.z, v.exact.z
  );
  let ray = slicedRayOf(body, oblateBody, start, dir);
  if (ray.shadowed) {
    return vec3f(0.0);
  }
  let count = min(u32(draw.shell.y), ABSORBER_SLOTS);
  let stored = slicedStoredOf(
    ray, body, oblateBody, transmittance, absorberColumns, absorberPressureColumns,
    lastAbsorberPressureColumn, count
  );
  let absorbed = absorberDepth(
    absorberCurves, count, slot, u32(draw.shell.z), stored.columns, stored.pressureColumns
  );
  return exp(-(stored.depth + absorbed));
}

// Whether a ray falls within a source's disc: |d x s| <= sin(rho) with d . s > 0, sin(rho) formed
// in f64 (Design note 18), and no disc where it is 0.
fn withinDisc(dir : vec3f, source : SkySource) -> bool {
  let s = source.direction.xyz;
  let sinRadius = source.direction.w;
  return sinRadius > 0.0 && dot(dir, s) > 0.0 && length(cross(dir, s)) <= sinRadius;
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
  let scene = textureLoad(sceneColour, pixel, 0);
  let colour = scene.rgb;
  // R07's METER_CLASS, kept: the pass writes the scene's own alpha (R08.T7).
  let meterClass = scene.a;
  let dir = rayThrough(v, uv);
  let factor = v.sky.w;
  let topM = draw.shell.x;
  let body = oblateBodyOf(oblateBody);
  let aboveTop = v.camera.w * v.curvature.z >= topM;
  let count = skySourceCount(v);

  if (depth == 0.0) {
    // The source whose disc the ray falls in, the last such, whose light slot dims what lies behind
    // it; slot 0's elsewhere.
    var slotHere = 0u;
    for (var k = 0u; k < count; k++) {
      if (withinDisc(dir, v.sources[k])) {
        slotHere = skySlotOf(v.sources[k]);
      }
    }
    var inscattered : vec3f;
    var behind : vec3f;
    var marchedAlpha = 1.0;
    if (aboveTop) {
      let marched = rayMarched(pixel, depth);
      inscattered = marched.rgb;
      marchedAlpha = marched.a;
      behind = vec3f(marched.a);
    } else {
      // The sky view's sphere at the ray's own azimuth, R_alpha, its horizon split, and the
      // camera's height on it (R08.T6.f).
      let heightM = skyViewHeight(v.camera.w, v.curvature.w);
      let up = v.up.xyz;
      let frame = skyFrame(v);
      let column = skyViewColumnOf(v.curvature.x, v.curvature.y, up, frame.x, dir, heightM);
      let viewZenithCos = dot(dir, up);
      let viewZenith = preciseAtan2(length(cross(dir, up)), viewZenithCos);
      let azimuthUnit = skyViewAzimuthUnit(dot(dir, frame.x), dot(dir, frame.y));
      let ground = groundHitFromHeight(column.x, heightM, viewZenithCos);
      let size = vec2f(textureDimensions(skyView));
      let skyUv = skyViewParamsToUv(ground, viewZenith, azimuthUnit, size, column.y);
      inscattered = textureSampleLevel(skyView, linearClamp, skyUv, 0.0).rgb;
      behind = transmittanceToSpace(v, body, dir, slotHere);
    }
    var luminance = colour * behind + inscattered * factor;
    // Each disc the composite draws, through its own light's transmittance, clamped after
    // pre-exposure.
    for (var k = 0u; k < count; k++) {
      let source = v.sources[k];
      if (withinDisc(dir, source) && dot(source.disc.rgb, vec3f(1.0)) > 0.0) {
        var toSpace = vec3f(marchedAlpha);
        if (!aboveTop) {
          toSpace = transmittanceToSpace(v, body, dir, skySlotOf(source));
        }
        luminance += min(source.disc.rgb * toSpace * factor, vec3f(HALF_MAX));
      }
    }
    return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
  }

  let distance = distanceAtDepth(v, uv, depth);
  let reach = v.tables.y;
  // Within the volume's reach, the volume, through the read every reader of it takes
  // (`aerialRead.wgsl`, R08.T9.a); beyond it, and from above the air, the ray march.
  if (distance <= reach && !aboveTop) {
    let aerialAt = aerialPerspectiveAt(aerial, linearClamp, uv, distance, reach);
    let luminance = colour * aerialAt.a + aerialAt.rgb * factor;
    return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
  }
  let marched = rayMarched(pixel, depth);
  let luminance = colour * marched.a + marched.rgb * factor;
  return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
}

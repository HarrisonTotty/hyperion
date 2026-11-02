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
// Changes (plan R08, R08.T8): the switch to the ray march is by the camera's h*, with a blend band
// (`TableSizes.marchBand`): below it the sky view and the aerial-perspective volume serve as before;
// from its upper edge up the march serves every pixel, the sky's and the surfaces' within the
// volume's reach, as from above the top; between them each is blended linearly with the march by
// the march's share, `view.exact.w`, formed in f64. Inside the air what lies behind the sky, and each
// disc, keeps the camera's own transmittance to space.
// Changes (plan R08, R08.T8.b): from above the top what lies behind the sky, and each disc, takes
// the transmittance to space per channel, the sliced lookup's from the ray's entry into the top's
// shell (`transmittanceFromAbove`), in place of the march's mean over the channels, which dimmed a
// sun seen through the limb grey where the air reddens it, and stepped in colour at the top. Low's
// half-resolution march is read from the nearest of the texels whose depths tie (`rayMarched`).
// Changes (plan R08, R08.T8.c): from above the top, that transmittance takes the mean over the
// pixel's far-limb footprint, blended in by its span, as the march's in-scatter does
// (`farLimb.wgsl`; `decision-r08-t8-leans.md` section 6.1), so that a star, or the Sun's disc in
// an eclipse seen from afar, is not cut out row by row where the centre ray grazes the base of the
// air.
// Changes (plan R08, R08.T11.d; `decision-r08-t11-frame.md` section 2): the march is dispatched over
// the air's area alone (`marchArea.ts`), and a pixel whose texel, or at half resolution any of its
// four candidates, lies outside it reads no air: the march's (0, 0, 0, 1), and from above the top a
// transmittance to space of 1, each what it read there, since outside the area every ray misses the
// top's shell, widened, and no footprint blends (`marchArea.ts`). It reads neither the march's
// target, which the march left as it was there, nor the footprint.
// Changes (the owner's priority, 2026-11-01): the transmittance to space moved to
// `compositeTransmittance.wgsl`. Where the march runs below the output's resolution, from above the
// top, a sky pixel takes what lies behind it from the march's resolution (`behindMarch`, light slot
// 0's across a pixel's footprint at each texel's centre, as ln tau, `compositeBehind.wgsl`), its
// four candidates interpolated (`behindAtMarch`) where all are sky and either thin or dim behind;
// elsewhere, and within a source's disc, it is formed per pixel as before (`AtmosphereSettings`'
// `compositeTransmittance`, whose `per-pixel` is the way back).
// Changes (plan R08, R08.T9.c): a drawn surface seen through the air is dimmed by its transmittance
// per channel, the aerial-perspective volume's second texture (`aerialTransmittanceAt`) or the ray
// march's second target, in place of the channels' mean, R05's grey, which left the ground 13% too
// dim in red and 23% too bright in blue through 5 km from 2 m on the measured Earth, and through the
// limb from orbit grey where the air reddens it (`AtmosphereSettings`' `surfaceTransmittance`,
// `levers.y`, whose `grey` is the way back).

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
  // The march's ground and sky rules (R08.T8.e), which the composite carries and does not read.
  marchGround : vec4f,
  marchSky : vec4f,
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
  // `common.wgsl`'s `pressureChannelOf` reads (R08.T4.d); w: from a camera beneath a cloud deck, the
  // kernels' vertical optical depth to it in the green channel (R08.T15.b.2), and 0 elsewhere.
  shell : vec4f,
  // The ray march's area, its texels [x, z) x [y, w) of its target (R08.T11.d; `marchArea.ts`'s
  // marchAreaTexels): every texel the march wrote this frame.
  marchArea : vec4f,
  // The composite's levers (the owner's priority, 2026-11-01; `settings.ts`'s AtmosphereSettings):
  // x 1 where what lies behind the sky from above the top is read at the march's resolution
  // (`behindMarch`, which `compositeBehind.wgsl` wrote over the march's area this frame), 0 where
  // it is formed per pixel; y 1 where a drawn surface is dimmed by its transmittance per channel
  // (R08.T9.c), 0 where by the channels' mean.
  levers : vec4f,
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
// The transmittance to space from above the top at the march's resolution, light slot 0's, as each
// channel's ln tau, tau = -ln T, at each sky texel of the march's area (`compositeBehind.wgsl`);
// read only where `levers.x` is 1.
@group(2) @binding(12) var behindMarch : texture_2d<f32>;
// The transmittance per channel beside the volume and the march (R08.T9.c): the volume's second
// texture and the march's second target, rgb, each with the mean in a.
@group(2) @binding(13) var aerialTransmittance : texture_3d<f32>;
@group(2) @binding(14) var rayMarchTransmittance : texture_2d<f32>;

const HALF_MAX : f32 = 65504.0;

// The views check's probe of the composite's cost (the owner's priority, 2026-11-01;
// `hillaire.ts`'s compositeProbedMaterial): 0, the composite as drawn, on every launch but an
// inspection profile's `--composite-probe`, whose material sets 1, every sky pixel keeping the
// scene's light (no sky branch); 2, the sky drawn with no transmittance across from above, 1 in its
// place (neither the far-limb footprint nor the sliced lookups it takes); 3, that transmittance
// formed per pixel whatever `levers.x`, the way back's composite; or 4, every drawn surface dimmed by
// the channels' mean whatever `levers.y`, the grey way back's (R08.T9.c). A constant, so that the
// composite every player draws compiles none of them.
const COMPOSITE_PROBE : u32 = 0u;

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
    draw.marchGround,
    draw.marchSky,
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

// What lies beyond the air along `dir` from a camera beneath a cloud deck (plan R08, R08.T15.b.2;
// `tablesCpu.ts`'s belowDeckBehindTwin): e^(-tau / mu) in every channel, tau the kernels' vertical
// optical depth from the top to the camera in the green channel (`draw.shell.w`, formed in f64), mu
// the ray's cosine against the zenith, the plane-parallel beam's; 0 at and below the horizontal.
fn belowDeckBehind(v : AtmosphereView, dir : vec3f) -> vec3f {
  let mu = dot(dir, v.up.xyz);
  if (!(mu > 0.0)) {
    return vec3f(0.0);
  }
  return vec3f(exp(-draw.shell.w / mu));
}

// Whether a ray falls within a source's disc: |d x s| <= sin(rho) with d . s > 0, sin(rho) formed
// in f64 (Design note 18), and no disc where it is 0.
fn withinDisc(dir : vec3f, source : SkySource) -> bool {
  let s = source.direction.xyz;
  let sinRadius = source.direction.w;
  return sinRadius > 0.0 && dot(dir, s) > 0.0 && length(cross(dir, s)) <= sinRadius;
}

// The ray march's texel for this pixel: at full resolution its own; at half, of the four nearest,
// the one whose source pixel's depth is closest to this pixel's, and of those that tie, the one
// whose centre is nearest the pixel's (R08.T8.b, `decision-r08-t8-leans.md` §6.2). Without the
// tie-break every sky and limb pixel, whose four depths are all 0, took the first candidate, so the
// half-resolution layer stood one output pixel down and right of the scene.
fn rayMarched(pixel : vec2u, depth : f32) -> vec4f {
  let at = rayMarchTexelOf(pixel, depth);
  if (!at.found) {
    return vec4f(0.0, 0.0, 0.0, 1.0);
  }
  return textureLoad(rayMarch, at.texel, 0);
}

// The ray march's texel `rayMarched` reads for a pixel (R08.T9.c, which reads the second target
// there too): `found` false where no candidate's depth compares (a NaN), which reads (0, 0, 0, 1).
struct MarchTexel {
  texel : vec2u,
  found : bool,
}

// `rayMarched`'s choice of texel: at full resolution the pixel's own; at half, of the four nearest,
// the one whose source pixel's depth is closest to this pixel's, ties to the nearest centre.
fn rayMarchTexelOf(pixel : vec2u, depth : f32) -> MarchTexel {
  let scale = draw.output.z;
  if (scale >= 1.0) {
    return MarchTexel(pixel, true);
  }
  // The target may be larger than this frame's output; the kernel filled only its share.
  let size = vec2u(ceil(draw.output.xy * scale));
  let full = vec2u(draw.output.xy);
  let base = vec2i(vec2f(pixel) * scale - 0.5);
  let centre = vec2f(pixel) + 0.5;
  var best = MarchTexel(vec2u(0u), false);
  var bestError = 3.4e38;
  var bestDistance = 3.4e38;
  for (var j = 0; j < 2; j++) {
    for (var i = 0; i < 2; i++) {
      let t = vec2u(clamp(base + vec2i(i, j), vec2i(0), vec2i(size) - 1));
      let source = min(vec2u((vec2f(t) + 0.5) / scale), full - 1u);
      let error = abs(textureLoad(sceneDepth, source, 0) - depth);
      // A tie of depths goes to the texel nearest the pixel's centre, the one the full-resolution
      // scene lines up with: the squared distance between their centres in output pixels.
      let offset = (vec2f(t) + 0.5) / scale - centre;
      let distance = dot(offset, offset);
      if (error < bestError || (error == bestError && distance < bestDistance)) {
        bestError = error;
        bestDistance = distance;
        best = MarchTexel(t, true);
      }
    }
  }
  return best;
}

// The most optical depth, in any channel at any of a pixel's four candidates, under which
// `behindAtMarch` stands for the pixel's own whatever lies behind. T = e^-tau moves by e^(tau
// delta) for a relative error delta of tau, so where tau is large the read's error grows by tau:
// through the limb's dense rows from 400 km it stood up to 0.9 stops apart under a light 30 times
// the air's behind them.
const BEHIND_MOST_DEPTH : f32 = 2.0;

// Past `BEHIND_MOST_DEPTH`, the share of the light scattered in front of a pixel, in each channel,
// that what lies behind it may bring through the candidates' most transmittance for the read to
// stand: with colour T <= 0.1 S, T off by a factor of up to 2.6 moves the pixel's light by under
// 1/3 stop, (1 + 0.26) / (1 + 0.1) = 2^0.2.
const BEHIND_THROUGH_SHARE : f32 = 0.1;

// What lies behind a sky pixel, read from the march's resolution (the owner's priority,
// 2026-11-01).
struct MarchBehind {
  behind : vec3f,
  // Whether the read stands for the pixel's own (`behindAtMarch`); per pixel where not.
  found : bool,
}

// What lies behind a sky pixel, its scene's light `colour` and the light `scattered` in front of
// it, from the march's resolution (`behindMarch`, each channel's ln tau, tau = -ln T its optical
// depth): the four texels around the pixel's centre, `rayMarched`'s candidates, interpolated
// bilinearly, where each is a sky texel (`compositeBehind.wgsl` wrote those alone) and either their
// tau is at most `BEHIND_MOST_DEPTH` or what lies behind is dim against the light in front
// (`BEHIND_THROUGH_SHARE`). ln tau is linear in the tangent height through an exponential air, and
// a tangent ray's height is near linear across the screen, where T read linearly, or a nearest
// texel, stood stops off the pixel's own across the limb. Where a candidate is not sky, the pixel
// lies within a texel of a drawn edge, the body's face among them, whose datum cuts the pixel's own
// far-limb footprint (`farLimb.wgsl`'s ground piece) where no texel's can stand for it: those stood
// up to 5 stops apart, and `found` is false, as it is past the depth with a bright light behind.
fn behindAtMarch(pixel : vec2u, colour : vec3f, scattered : vec3f) -> MarchBehind {
  let scale = draw.output.z;
  let size = vec2i(ceil(draw.output.xy * scale));
  let full = vec2u(draw.output.xy);
  let at = (vec2f(pixel) + 0.5) * scale - 0.5;
  let base = floor(at);
  let f = at - base;
  var sum = vec3f(0.0);
  var most = vec3f(-3.4e38);
  var least = vec3f(3.4e38);
  for (var j = 0; j < 2; j++) {
    for (var i = 0; i < 2; i++) {
      let t = vec2u(clamp(vec2i(base) + vec2i(i, j), vec2i(0), size - 1));
      let source = min(vec2u((vec2f(t) + 0.5) / scale), full - 1u);
      if (textureLoad(sceneDepth, source, 0) != 0.0) {
        return MarchBehind(vec3f(1.0), false);
      }
      let lnDepth = textureLoad(behindMarch, t, 0).rgb;
      most = max(most, lnDepth);
      least = min(least, lnDepth);
      sum += select(1.0 - f.x, f.x, i == 1) * select(1.0 - f.y, f.y, j == 1) * lnDepth;
    }
  }
  let thin = all(most <= vec3f(log(BEHIND_MOST_DEPTH)));
  let dim = all(colour * exp(-exp(least)) <= BEHIND_THROUGH_SHARE * scattered);
  if (!(thin || dim)) {
    return MarchBehind(vec3f(1.0), false);
  }
  return MarchBehind(exp(-exp(sum)), true);
}

// Whether the ray march wrote what this pixel reads (R08.T11.d): its texel at full resolution, at
// half each of the four `rayMarched` chooses from, within the march's area.
fn marchedHere(pixel : vec2u) -> bool {
  let scale = draw.output.z;
  var low = vec2i(pixel);
  var high = low;
  if (scale < 1.0) {
    let size = vec2i(ceil(draw.output.xy * scale));
    let base = vec2i(vec2f(pixel) * scale - 0.5);
    low = clamp(base, vec2i(0), size - 1);
    high = clamp(base + vec2i(1), vec2i(0), size - 1);
  }
  let area = draw.marchArea;
  return all(vec2f(low) >= area.xy) && all(vec2f(high) < area.zw);
}

// The ray march's texel for this pixel (`rayMarched`) where the march wrote it, and outside the
// area what it would have written there, its ray missing the air (R08.T11.d): the first target's
// alone, for the sky.
fn marchedLightAt(pixel : vec2u, depth : f32, here : bool) -> vec4f {
  if (here) {
    return rayMarched(pixel, depth);
  }
  return vec4f(0.0, 0.0, 0.0, 1.0);
}

// The ray march's two targets at a pixel's texel: the first's in-scatter and mean transmittance,
// and the second's transmittance per channel (R08.T9.c).
struct Marched {
  light : vec4f,
  transmittance : vec3f,
}

// `marchedLightAt` with the second target's texel beside the first's (R08.T9.c), for a surface.
fn marchedAt(pixel : vec2u, depth : f32, here : bool) -> Marched {
  if (here) {
    let at = rayMarchTexelOf(pixel, depth);
    if (at.found) {
      return Marched(
        textureLoad(rayMarch, at.texel, 0),
        textureLoad(rayMarchTransmittance, at.texel, 0).rgb,
      );
    }
  }
  return Marched(vec4f(0.0, 0.0, 0.0, 1.0), vec3f(1.0));
}

// What a drawn surface's light passes through the march's air: its transmittance per channel where
// `channels` (R08.T9.c), else the channels' mean.
fn marchedThrough(marched : Marched, channels : bool) -> vec3f {
  if (channels) {
    return marched.transmittance;
  }
  return vec3f(marched.light.a);
}

// Whether the composite draws source `direction`'s disc, `disc` its light, along `dir`: the full
// path's `withinDisc` and its light's test, read from the draw's own members.
fn discAlong(dir : vec3f, direction : vec4f, disc : vec4f) -> bool {
  let sinRadius = direction.w;
  return sinRadius > 0.0 && dot(dir, direction.xyz) > 0.0 &&
    length(cross(dir, direction.xyz)) <= sinRadius && dot(disc.rgb, vec3f(1.0)) > 0.0;
}

// Whether any of the frame's `count` sources has its disc drawn along `dir`.
fn anyDiscAlong(dir : vec3f, count : u32) -> bool {
  return (count > 0u && discAlong(dir, draw.sourceDirection0, draw.sourceDisc0)) ||
    (count > 1u && discAlong(dir, draw.sourceDirection1, draw.sourceDisc1)) ||
    (count > 2u && discAlong(dir, draw.sourceDirection2, draw.sourceDisc2)) ||
    (count > 3u && discAlong(dir, draw.sourceDirection3, draw.sourceDisc3)) ||
    (count > 4u && discAlong(dir, draw.sourceDirection4, draw.sourceDisc4)) ||
    (count > 5u && discAlong(dir, draw.sourceDirection5, draw.sourceDisc5));
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let pixel = vec2u(position.xy);
  // From above the air's top, outside the march's area, a pixel reads no air (R08.T11.d), and with
  // no disc drawn on it keeps the scene's light: the full path's own sum, colour × 1 + 0 × factor,
  // formed here before the view and the body are read (R08.T16.a.2's giant a few pixels across).
  if (draw.camera.w * draw.curvature.z >= draw.shell.x && draw.exact.w >= 1.0 && !marchedHere(pixel)) {
    let uv = position.xy / draw.output.xy;
    let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let dir = normalize(draw.forward.xyz + ndc.x * draw.right.xyz + ndc.y * draw.upRay.xyz);
    if (!anyDiscAlong(dir, min(u32(draw.sky.x), MAX_SKY_SOURCES))) {
      let scene = textureLoad(sceneColour, pixel, 0);
      let meterClass = scene.a;
      let luminance = scene.rgb * vec3f(1.0) + vec3f(0.0) * draw.sky.w;
      return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
    }
  }
  let v = viewOf();
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
  // The ray march's share of the frame (R08.T8): 0 below the march band, 1 from its upper edge up,
  // the same for every pixel, so each branch on it is uniform.
  let marchWeight = v.exact.w;
  let count = skySourceCount(v);
  // Whether the pixel lies in the march's area (R08.T11.d); outside it, it reads no air.
  let here = marchedHere(pixel);

  if (depth == 0.0) {
    if (COMPOSITE_PROBE == 1u) {
      return vec4f(min(colour, vec3f(HALF_MAX)), meterClass);
    }
    // The source whose disc the ray falls in, the last such, whose light slot dims what lies behind
    // it; slot 0's elsewhere.
    var slotHere = 0u;
    var inDisc = false;
    for (var k = 0u; k < count; k++) {
      if (withinDisc(dir, v.sources[k])) {
        slotHere = skySlotOf(v.sources[k]);
        inDisc = true;
      }
    }
    var inscattered : vec3f;
    var behind : vec3f;
    // The pixel's far-limb footprint from above the top (R08.T8.c), w_L 0 elsewhere.
    var footprint = farLimbNone();
    if (aboveTop) {
      // What lies behind takes its transmittance to space per channel from the ray's entry into the
      // air (R08.T8.b), not the march's mean over the channels, and across the pixel's footprint
      // (R08.T8.c); outside the march's area, where the ray misses the air, 1 (R08.T11.d).
      behind = vec3f(1.0);
      inscattered = vec3f(0.0);
      if (here) {
        inscattered = rayMarched(pixel, depth).rgb;
        if (COMPOSITE_PROBE != 2u) {
          // From the march's resolution where the frame formed it and the read stands for the
          // pixel's own (the owner's priority, 2026-11-01; `behindAtMarch`): neither the footprint
          // nor a lookup is formed for the pixel. Per pixel elsewhere, as before, and within a
          // source's disc, whose light slot and own transmittance the disc takes. Each is a loop of
          // at most one pass, not an `if`: on a GPU the two are the same, a branch that no thread
          // of a group takes is skipped, but SwiftShader, the checks' software adapter, runs both
          // sides of a branch under its lanes' mask whatever they take, and a loop only while a
          // lane is in it; as an `if` the pod's composite paid the per-pixel term at every pixel
          // (468 ms against 225 ms with it compiled out, at low from orbit).
          var fromMarch = MarchBehind(vec3f(1.0), false);
          let reads = draw.levers.x > 0.5 && COMPOSITE_PROBE != 3u && !inDisc;
          for (var read = reads; read; read = false) {
            fromMarch = behindAtMarch(pixel, colour, inscattered * factor);
          }
          behind = fromMarch.behind;
          for (var perPixel = !fromMarch.found; perPixel; perPixel = false) {
            footprint = farLimbFootprintOf(farLimbRaysOf(v), body, oblateBody, uv, v.output.xy);
            behind = transmittanceAcrossFromAbove(v, body, &footprint, dir, slotHere);
          }
        }
      }
    } else {
      // Inside the air: from the march band's upper edge up the march's sky, below its lower edge
      // the sky view's, and between them the two blended by the march's share (R08.T8). What lies
      // behind takes the camera's own transmittance to space either way.
      if (marchWeight < 1.0) {
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
      }
      if (marchWeight > 0.0) {
        let marched = marchedLightAt(pixel, depth, here);
        if (marchWeight < 1.0) {
          inscattered = mix(inscattered, marched.rgb, marchWeight);
        } else {
          inscattered = marched.rgb;
        }
      }
      // Beneath a deck (`shell.w` above 0, R08.T15.b.2), through the deck's column.
      if (draw.shell.w > 0.0) {
        behind = belowDeckBehind(v, dir);
      } else {
        behind = transmittanceToSpace(v, body, dir, slotHere);
      }
    }
    var luminance = colour * behind + inscattered * factor;
    // Each disc the composite draws, through its own light's transmittance, clamped after
    // pre-exposure.
    for (var k = 0u; k < count; k++) {
      let source = v.sources[k];
      if (withinDisc(dir, source) && dot(source.disc.rgb, vec3f(1.0)) > 0.0) {
        var toSpace : vec3f;
        if (aboveTop) {
          toSpace = vec3f(1.0);
          if (here && COMPOSITE_PROBE != 2u) {
            toSpace = transmittanceAcrossFromAbove(v, body, &footprint, dir, skySlotOf(source));
          }
        } else if (draw.shell.w > 0.0) {
          toSpace = belowDeckBehind(v, dir);
        } else {
          toSpace = transmittanceToSpace(v, body, dir, skySlotOf(source));
        }
        luminance += min(source.disc.rgb * toSpace * factor, vec3f(HALF_MAX));
      }
    }
    return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
  }

  let distance = distanceAtDepth(v, uv, depth);
  let reach = v.tables.y;
  // The surface's light through the air per channel (R08.T9.c), or by the channels' mean, the way
  // back; the same for every pixel, so each branch on it is uniform.
  let channels = draw.levers.y > 0.5 && COMPOSITE_PROBE != 4u;
  // Within the volume's reach, the volume, through the read every reader of it takes
  // (`aerialRead.wgsl`, R08.T9.a), blended with the ray march by its share in the march band
  // (R08.T8); beyond it, and from the band's upper edge up (above the air included), the ray march.
  if (distance <= reach && marchWeight < 1.0) {
    let aerialAt = aerialPerspectiveAt(aerial, linearClamp, uv, distance, reach);
    var through = vec3f(aerialAt.a);
    if (channels) {
      through = aerialTransmittanceAt(aerialTransmittance, linearClamp, uv, distance, reach);
    }
    var luminance = colour * through + aerialAt.rgb * factor;
    if (marchWeight > 0.0) {
      let marched = marchedAt(pixel, depth, here);
      luminance = mix(
        luminance,
        colour * marchedThrough(marched, channels) + marched.light.rgb * factor,
        marchWeight,
      );
    }
    return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
  }
  let marched = marchedAt(pixel, depth, here);
  let luminance = colour * marchedThrough(marched, channels) + marched.light.rgb * factor;
  return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
}

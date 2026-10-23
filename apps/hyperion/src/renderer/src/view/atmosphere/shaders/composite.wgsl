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

// The sliced lookup's start at the camera (R08.T6.f; `oblate.wgsl`): its place, its ground quadric
// and local scale height formed in f64 (`view.exact`), its height, the datum's normal under it, and
// s and h* there.
fn cameraLookupStart(v : AtmosphereView, body : OblateBody) -> LookupStart {
  let n = v.up.xyz;
  return LookupStart(
    v.camera.xyz, oblateScaled(body, v.camera.xyz), v.exact.x, v.camera.w, n, n.z, length(n.xy),
    v.curvature.z, v.camera.w * v.curvature.z, v.exact.z
  );
}

// A light's transmittance to space along `dir` from the lookup's `start`, per channel: the sliced
// lookup's (R08.T6.f; `oblate.wgsl`), the slices' optical depth, absorber columns and
// pressure-weighted columns over s, with light slot `slot`'s curves of growth (R08.T6.c, R08.T4.d;
// each light's own since R08.T7), exponentiated in f32, so that a disc's light is never a stored
// half float; 0 where the ground hides space.
fn transmittanceFrom(body : OblateBody, start : LookupStart, dir : vec3f, slot : u32) -> vec3f {
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

// A light's transmittance from the camera along `dir` to space, per channel
// (`transmittanceFrom` at the camera's start): inside the air.
fn transmittanceToSpace(v : AtmosphereView, body : OblateBody, dir : vec3f, slot : u32) -> vec3f {
  return transmittanceFrom(body, cameraLookupStart(v, body), dir, slot);
}

// The distance along the unit `dir` from a camera above the top to the ray's entry into the top's
// shell, the quadric (a + H / s_e, c + H / s_p) the ray march clips the air by (R08.T6.f), 0 where
// the camera lies within it, and -1 where the ray misses it or it lies behind (R08.T8.b). As
// `rayMarch.wgsl`'s `rayQuadric` and `quadricHits` take it: in space scaled to a sphere, its
// constant formed in f64 (`view.exact.y`), its discriminant from Lagrange's identity beyond
// h0 = a / 2, and its roots in the forms that do not cancel.
fn topEntryOf(v : AtmosphereView, body : OblateBody, dir : vec3f) -> f32 {
  let a = v.figure.x;
  let topA = draw.shell.x * body.scales.z;
  let radius = a + topA;
  let scale = vec3f(1.0, 1.0, radius / (v.figure.y + draw.shell.x * body.scales.w));
  let o = v.camera.xyz * scale;
  let d = dir * scale;
  let qa = dot(d, d);
  let qb = dot(o, d);
  let qc = v.exact.y;
  var disc = qb * qb - qa * qc;
  if (v.camera.w > 0.5 * a) {
    let across = cross(o, d);
    disc = qa * (radius * radius) - dot(across, across);
  }
  if (disc < 0.0) {
    return -1.0;
  }
  let root = sqrt(disc);
  let s = -(qb + select(-root, root, qb >= 0.0));
  if (s == 0.0) {
    return -1.0;
  }
  let first = s / qa;
  let second = qc / s;
  if (max(first, second) <= 0.0) {
    return -1.0;
  }
  return max(min(first, second), 0.0);
}

// A light's transmittance to space along `dir` from a camera above the top, per channel (R08.T8.b):
// 1 where the ray misses the air; the sliced lookup's from the camera where it lies within the
// top's shell; and otherwise from the ray's entry into it, the camera's ray carrying nothing
// before it, so that its transmittance to space is the table's from the entry to the top
// (Bruneton 2017, precomputed_atmospheric_scattering's `functions.glsl`: `GetSkyRadiance` moves a
// viewer in space to the top boundary along the view ray, 1 where the ray misses the air and 0
// where it meets the ground, and `ComputeTransmittanceToTopAtmosphereBoundary` defines the table).
// The entry's ground quadric Q = |S p|^2 - a^2 is the shell's, H_e (2a + H_e) + (k - k_t)(k + k_t)
// z^2, with H_e = H / s_e, H_p = H / s_p, k_t = (a + H_e) / (c + H_p) and k - k_t =
// (a H_p - c H_e) / (c (c + H_p)), which no difference of radii forms (Design note 18); its
// height and normal are its datum point's, s and h* there, and, on a body of several bands, the
// local scale height the body's table holds at that h*, since the composite declares no medium.
fn transmittanceFromAbove(v : AtmosphereView, body : OblateBody, dir : vec3f, slot : u32) -> vec3f {
  let entryM = topEntryOf(v, body, dir);
  if (entryM < 0.0) {
    return vec3f(1.0);
  }
  if (entryM == 0.0) {
    return transmittanceToSpace(v, body, dir, slot);
  }
  let a = v.figure.x;
  let c = v.figure.y;
  let topA = draw.shell.x * body.scales.z;
  let topC = draw.shell.x * body.scales.w;
  let k = body.scales.x;
  let kTop = (a + topA) / (c + topC);
  let kGap = (a * topC - c * topA) / (c * (c + topC));
  let p = v.camera.xyz + entryM * dir;
  let quadricM2 = topA * (2.0 * a + topA) + kGap * (k + kTop) * p.z * p.z;
  let datum = datumPointOf(body, p, quadricM2);
  let s = gravityRatioAt(body, datum.normal.z * datum.normal.z);
  let z = s * datum.heightM;
  var scaleM = 0.0;
  if (oblateBandCount(body) > 1u) {
    scaleM = oblateScaleHeightAt(body, oblateBody, z);
  }
  let start = LookupStart(
    p, oblateScaled(body, p), quadricM2, datum.heightM, datum.normal, datum.normal.z,
    length(datum.normal.xy), s, z, scaleM
  );
  return transmittanceFrom(body, start, dir, slot);
}

// A light's transmittance to space from a camera above the top across the pixel's far-limb
// footprint (R08.T8.c): `transmittanceFromAbove` along the pixel's ray `dir`, blended by w_L with
// its mean over the footprint's nodes, the share above the top counted as 1, what lies there.
fn transmittanceAcrossFromAbove(
  v : AtmosphereView,
  body : OblateBody,
  footprint : ptr<function, FarLimbFootprint>,
  dir : vec3f,
  slot : u32,
) -> vec3f {
  let centre = transmittanceFromAbove(v, body, dir, slot);
  let blend = (*footprint).blend;
  if (!(blend > 0.0)) {
    return centre;
  }
  var mean = vec3f((*footprint).spaceShare);
  for (var i = 0u; i < (*footprint).count; i++) {
    let node = farLimbNodeOf(footprint, i);
    mean += node.z * transmittanceFromAbove(v, body, rayThrough(v, node.xy), slot);
  }
  if (blend >= 1.0) {
    return mean;
  }
  return mix(centre, mean, blend);
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
  let scale = draw.output.z;
  if (scale >= 1.0) {
    return textureLoad(rayMarch, pixel, 0);
  }
  // The target may be larger than this frame's output; the kernel filled only its share.
  let size = vec2u(ceil(draw.output.xy * scale));
  let full = vec2u(draw.output.xy);
  let base = vec2i(vec2f(pixel) * scale - 0.5);
  let centre = vec2f(pixel) + 0.5;
  var best = vec4f(0.0, 0.0, 0.0, 1.0);
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
  // The ray march's share of the frame (R08.T8): 0 below the march band, 1 from its upper edge up,
  // the same for every pixel, so each branch on it is uniform.
  let marchWeight = v.exact.w;
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
    // The pixel's far-limb footprint from above the top (R08.T8.c), w_L 0 elsewhere.
    var footprint = farLimbNone();
    if (aboveTop) {
      // What lies behind takes its transmittance to space per channel from the ray's entry into the
      // air (R08.T8.b), not the march's mean over the channels, and across the pixel's footprint
      // (R08.T8.c).
      footprint = farLimbFootprintOf(farLimbRaysOf(v), body, oblateBody, uv, v.output.xy);
      inscattered = rayMarched(pixel, depth).rgb;
      behind = transmittanceAcrossFromAbove(v, body, &footprint, dir, slotHere);
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
        let marched = rayMarched(pixel, depth);
        if (marchWeight < 1.0) {
          inscattered = mix(inscattered, marched.rgb, marchWeight);
        } else {
          inscattered = marched.rgb;
        }
      }
      behind = transmittanceToSpace(v, body, dir, slotHere);
    }
    var luminance = colour * behind + inscattered * factor;
    // Each disc the composite draws, through its own light's transmittance, clamped after
    // pre-exposure.
    for (var k = 0u; k < count; k++) {
      let source = v.sources[k];
      if (withinDisc(dir, source) && dot(source.disc.rgb, vec3f(1.0)) > 0.0) {
        var toSpace : vec3f;
        if (aboveTop) {
          toSpace = transmittanceAcrossFromAbove(v, body, &footprint, dir, skySlotOf(source));
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
  // Within the volume's reach, the volume, through the read every reader of it takes
  // (`aerialRead.wgsl`, R08.T9.a), blended with the ray march by its share in the march band
  // (R08.T8); beyond it, and from the band's upper edge up (above the air included), the ray march.
  if (distance <= reach && marchWeight < 1.0) {
    let aerialAt = aerialPerspectiveAt(aerial, linearClamp, uv, distance, reach);
    var luminance = colour * aerialAt.a + aerialAt.rgb * factor;
    if (marchWeight > 0.0) {
      let marched = rayMarched(pixel, depth);
      luminance = mix(luminance, colour * marched.a + marched.rgb * factor, marchWeight);
    }
    return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
  }
  let marched = rayMarched(pixel, depth);
  let luminance = colour * marched.a + marched.rgb * factor;
  return vec4f(min(luminance, vec3f(HALF_MAX)), meterClass);
}

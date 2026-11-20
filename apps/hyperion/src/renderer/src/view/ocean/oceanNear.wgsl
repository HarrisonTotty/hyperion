// The near sea (plan R11, R11.T8.c; Design notes 11, 12 and 19): below 20 km over the sea, a grid of
// screen points, each one's ray through the frame's projection met with the sea-level spheroid in
// the camera's stretched frame (`ocean_near_hit`), displaced by the sea's Gerstner set, each
// component faded by the footprint of a grid cell there, its phases laid on the sea at its wind's
// anchor (`packOceanWaves`); each fragment shaded with the set's normal at its Lagrangian point, its
// components faded by a pixel's footprint, the BRDF keeping the slopes the drawn components leave,
// Fresnel at the wave's facet, the sky reflected, the water's own light over its thickness and the
// bed through it, refracted on the high setting, and the whitecaps; its CPU twin, in f64, is
// ocean/oceanNear.ts, whose remarks give each figure's source.
//
// The water's thickness is the bed's distance along the pixel's ray less the sea's, from the scene's
// depth copied before the pass (`ocean_scene_depth`, its reversed-Z bits in an r32uint), within
// OCEAN_NEAR_DEPTH_MAX of the camera, its vertical share by the ray's cosine to the sea's up; the
// texel's bathymetry beyond, or where the scene drew no bed. A bed within OCEAN_NEAR_CULL of the
// sea's distance is culled: the sea is drawn over it with no water at its depth, the bed's, which
// no precision orders; a bed nearer than that hides the sea, which is discarded there.
//
// At the shore (R11.T8.d, shoreline.wgsl): every amplitude limited by the water's depth, at a
// vertex's pixel as at a fragment's; the surf zone's foam by that depth and the bed's slope, which
// the scene's depth gives at three pixels, with the whitecaps; and the depth fade to the bed over
// the pixel's reach along that slope, the sea's alpha.
//
// Drawn into the scene target, premultiplied by that alpha, keeping the meter class beneath, and
// writing its depth. Composed after frame.wgsl, R08's surface lighting, shaders/cubeTexel.wgsl,
// oceanFar.wgsl, oceanShared.wgsl and shoreline.wgsl.

struct Draw {
  offsetFromCameraM : vec3f,
  // xyz: the camera from the body's centre, body-fixed, m; w: 1 with refraction (the high setting).
  camera : vec4f,
  // The view's x, y and z axes in the body-fixed ones.
  viewX : vec4f,
  viewY : vec4f,
  viewZ : vec4f,
  // The stretched frame (`oceanNearFrame`): Û′, the camera's direction stretched along the pole by
  // a ÷ c, w |o′|, its distance there, m.
  nearUp : vec4f,
  // The anchor's wind frame (`nearSeaAnchor`): along the wind, w the camera's height above the sea, m.
  nearUpwind : vec4f,
  // Across the wind, w C₀ = |o′|² − a², m², formed in f64.
  nearAcross : vec4f,
  // x: the grid's cell's angle, rad; y: the stretch a ÷ c.
  nearGrid : vec4f,
  // x, y: the datum's equatorial and polar radii, m; z: the sea level above it, m; w: the sources
  // drawn, 0 to 2.
  datum : vec4f,
  // Each source: xyz its unit direction, body-fixed; w its light slot.
  source0 : vec4f,
  // Its illuminance face-on per channel times the frame's pre-exposure, lx; w its angular radius,
  // rad, which the cloudy form's clouds take (`oceanClouds.ts`).
  sourceLight0 : vec4f,
  source1 : vec4f,
  sourceLight1 : vec4f,
  // oceanFar.wgsl's `OceanFar`, row by row (`packOceanFar`): its light and cap the pre-exposed ones.
  farFigure : vec4f,
  farSun : vec4f,
  farSunLux : vec4f,
  farSkyLux : vec4f,
  farIndex : vec4f,
  farAttenuation : vec4f,
  farDeep : vec4f,
  farBed : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
// The waves (`packOceanWaves`): a header (components, the set's H_rms, m, the peak's deep-water
// wavelength L₀, m), then each component's (k_x, k_y, A, φ − ω t + k · ξ) and (Q, λ, cos², 0) in the
// anchor's wind frame.
@group(2) @binding(11) var<storage, read> ocean_waves : array<vec4f>;
// The scene's depth before the pass, its reversed-Z f32 bits.
@group(2) @binding(12) var ocean_scene_depth : texture_2d<u32>;
// The scene's colour before the pass, pre-exposed: the bed's light for refraction; a one-texel
// stand-in without it.
@group(2) @binding(13) var ocean_scene_colour : texture_2d<f32>;

// `oceanNear.ts`' OCEAN_DEPTH_THICKNESS_MAX_M, OCEAN_LAYER_CULL_SHARE and the footprint's least
// view cosine (`oceanFar.ts`' FOOTPRINT_MIN_COSINE).
const OCEAN_NEAR_DEPTH_MAX : f32 = 1e5;
const OCEAN_NEAR_CULL : f32 = 1e-6;
const OCEAN_NEAR_MIN_COSINE : f32 = 0.01;
const OCEAN_NEAR_LEAST_REFRACTED : f32 = 1e-3;

// Where a ray from the camera meets the sea-level spheroid (`oceanNearHit`).
struct OceanNearHit {
  t : f32,
  // The point from the camera, body-fixed, m.
  point : vec3f,
  // The spheroid's outward normal there.
  up : vec3f,
  // Its Lagrangian coordinates in the anchor's wind frame, camera-relative, m.
  lagrangian : vec2f,
}

fn ocean_near_hit(direction : vec3f) -> OceanNearHit {
  let k = draw.nearGrid.y;
  let up = draw.nearUp.xyz;
  let distance = draw.nearUp.w;
  let excess = draw.nearAcross.w;
  let d = vec3f(direction.xy, direction.z * k);
  let a2 = dot(d, d);
  let b = distance * dot(up, d);
  let disc = b * b - a2 * excess;
  var out : OceanNearHit;
  if (b < 0.0 && disc >= 0.0) {
    out.t = excess / (-b + sqrt(disc));
    out.point = out.t * direction;
  } else {
    // The tangent from the camera at this azimuth, in the stretched space.
    let w = normalize(d);
    let horizontal = w - up * dot(up, w);
    let along = select(normalize(cross(up, vec3f(0.0, 0.0, 1.0))), normalize(horizontal), length(horizontal) > 1e-12);
    let reach = sqrt(max(excess, 0.0));
    let tangent = up * (-reach / distance) + along * (draw.farFigure.x / distance);
    out.point = vec3f(reach * tangent.xy, reach * tangent.z / k);
    out.t = length(out.point);
  }
  let world = draw.camera.xyz + out.point;
  let a = draw.farFigure.x;
  let c = draw.farFigure.y;
  out.up = normalize(vec3f(world.xy / (a * a), world.z / (c * c)));
  out.lagrangian = vec2f(dot(out.point, draw.nearUpwind.xyz), dot(out.point, draw.nearAcross.xyz));
  return out;
}

// A component's share drawn at a footprint (`componentFade`): Bruneton et al. 2010's smoothstep from
// 1.0 to 2.5 footprints a wavelength (OCEAN_FADE_BAND).
fn ocean_near_fade(wavelength : f32, footprint : f32) -> f32 {
  if (!(footprint > 0.0)) {
    return 1.0;
  }
  let x = clamp((wavelength / footprint - 1.0) / 1.5, 0.0, 1.0);
  return x * x * (3.0 - 2.0 * x);
}

// The surface at a Lagrangian point, its components faded by a footprint (`oceanNearSurface`).
struct OceanNearSurface {
  // Along the wind, across it and up, m.
  displacement : vec3f,
  // The unit normal in the wind's frame.
  normal : vec3f,
  // The drawn components' resolved variance, along the wind and across it.
  resolved : vec2f,
}

fn ocean_near_surface(lagrangian : vec2f, footprint : f32, amplitude_scale : f32) -> OceanNearSurface {
  var displacement = vec3f(0.0);
  var xx = 0.0;
  var yy = 0.0;
  var xy = 0.0;
  var zx = 0.0;
  var zy = 0.0;
  var resolved = vec2f(0.0);
  let count = min(u32(ocean_waves[0].x), 32u);
  for (var i = 0u; i < count; i = i + 1u) {
    let wave = ocean_waves[1u + 2u * i];
    let shape = ocean_waves[2u + 2u * i];
    let fade = ocean_near_fade(shape.y, footprint);
    if (fade == 0.0) {
      continue;
    }
    let a = fade * amplitude_scale * wave.z;
    let k = length(wave.xy);
    let u = wave.xy / k;
    let theta = dot(wave.xy, lagrangian) + wave.w;
    let s = sin(theta);
    let c = cos(theta);
    displacement = displacement + vec3f(-shape.x * a * u * s, a * c);
    let qakc = shape.x * a * k * c;
    xx = xx + qakc * u.x * u.x;
    yy = yy + qakc * u.y * u.y;
    xy = xy + qakc * u.x * u.y;
    zx = zx + a * k * u.x * s;
    zy = zy + a * k * u.y * s;
    let ka = k * a;
    let v = 1.0 - sqrt(max(1.0 - ka * ka, 0.0));
    resolved = resolved + v * vec2f(shape.z, 1.0 - shape.z);
  }
  var out : OceanNearSurface;
  out.displacement = displacement;
  out.normal = normalize(cross(vec3f(1.0 - xx, -xy, -zx), vec3f(-xy, 1.0 - yy, -zy)));
  out.resolved = resolved;
  return out;
}

// The wind's frame at a point of local up `up`: the anchor's wind laid tangent there.
fn ocean_near_along(up : vec3f) -> vec3f {
  return normalize(draw.nearUpwind.xyz - up * dot(draw.nearUpwind.xyz, up));
}

// What a near fragment's water is made of (`OceanNearPoint`'s thickness and bed).
struct OceanNearWater {
  // The vertical depth, m.
  depth : f32,
  // The bed's light in air where the refracted ray meets it, pre-exposed; `has_bed` false for the
  // far sea's albedo.
  bed : vec3f,
  has_bed : bool,
}

// The near sea's light at a point under `light`, pre-exposed (`oceanNearLight`).
fn ocean_near_light(
  far : OceanFar,
  light : OceanLight,
  view : vec3f,
  up : vec3f,
  normal_in : vec3f,
  texel : OceanTexel,
  resolved : vec2f,
  footprint : f32,
  water : OceanNearWater,
  shore_foam : f32,
) -> vec3f {
  let sun = far.sun.xyz;
  // A normal facing away from the camera reflected towards it (`facingNormal`).
  let facing = dot(normal_in, view);
  let normal = select(normal_in, normalize(normal_in - 2.0 * facing * view), facing < 0.0);
  let east = ocean_east(up);
  let north = cross(up, east);
  let wind_vector = east * texel.wind.x + north * texel.wind.y;
  let wind10 = length(wind_vector);
  let along = select(east, wind_vector / wind10, wind10 > 0.0);
  let tangent = along - normal * dot(along, normal);
  let upwind = select(normalize(cross(normal, vec3f(0.0, 0.0, 1.0))), normalize(tangent), length(tangent) > 1e-9);
  let spread = footprint / far.figure.x;
  let variance = max(ocean_slope_variance(wind10, far) - resolved, vec2f(OCEAN_SLOPE_FLOOR))
    + vec2f(spread * spread / 12.0);
  let index = far.index.xyz;
  let glint = light.sunLux * ocean_glint_factor(view, sun, normal, upwind, variance, index);
  let mu_v = max(dot(normal, view), 0.0);
  let mu_s = max(dot(normal, sun), 0.0);
  let fresnel_view = ocean_fresnel3(index, mu_v);
  let fresnel_sun = ocean_fresnel3(index, mu_s);
  let sky = fresnel_view * light.skyLux / OCEAN_PI;
  let down = light.sunLux * max(dot(up, sun), 0.0) + light.skyLux;
  var water_light = vec3f(0.0);
  if (far.attenuation.w > 0.5) {
    let kd = far.attenuation.xyz;
    let mu_st = ocean_refracted_cosine(index, mu_s);
    let mu_vt = ocean_refracted_cosine(index, mu_v);
    let entering = light.sunLux * mu_s * (vec3f(1.0) - fresnel_sun) + light.skyLux * (1.0 - far.deep.w);
    // One path for the column and the bed: down and up the refracted rays with the bed's light,
    // twice the sun's way with the far sea's albedo.
    var through = exp(-2.0 * kd * water.depth / mu_st);
    if (water.has_bed) {
      through = exp(-kd * water.depth * (vec3f(1.0) / mu_st + vec3f(1.0) / max(mu_vt, vec3f(OCEAN_NEAR_LEAST_REFRACTED))));
    }
    let column = far.deep.xyz * (vec3f(1.0) - through);
    water_light = (vec3f(1.0) - fresnel_view) * column * entering / (index * index * OCEAN_UPWELLING_Q);
    if (water.has_bed) {
      water_light = water_light + water.bed * (vec3f(1.0) - fresnel_sun) * (vec3f(1.0) - fresnel_view)
        / (index * index) * through;
    } else {
      water_light = water_light + (vec3f(1.0) - fresnel_view) * far.bed.xyz * through * entering
        / (index * index * OCEAN_UPWELLING_Q);
    }
  }
  // The wind's whitecaps and the shore's foam, as independent shares of the surface, on the setting
  // that draws whitecaps.
  let shore = select(0.0, shore_foam, far.figure.w > 0.5);
  let whitecaps = 1.0 - (1.0 - ocean_whitecaps(wind10, far)) * (1.0 - shore);
  let foam = OCEAN_WHITECAP_ALBEDO * down / OCEAN_PI;
  return (1.0 - whitecaps) * (min(glint, vec3f(far.sun.w)) + sky + water_light) + whitecaps * foam;
}

// The water's vertical depth along the ray at `pixel` (`oceanThickness`): from the scene's depth
// within OCEAN_NEAR_DEPTH_MAX, else the bathymetry; x the depth, m, y 1 where the scene's depth gave
// it, z 1 where the cull takes the bed, w 1 where the bed is nearer than the sea (the sea hidden).
fn ocean_near_water_depth(
  pixel : vec2i,
  ray_length : f32,
  sea_distance : f32,
  view_cosine : f32,
  bathymetry : f32,
) -> vec4f {
  let size = vec2i(textureDimensions(ocean_scene_depth));
  let bed_depth = bitcast<f32>(textureLoad(ocean_scene_depth, clamp(pixel, vec2i(0), size - vec2i(1)), 0).x);
  if (!(bed_depth > 0.0) || sea_distance > OCEAN_NEAR_DEPTH_MAX) {
    return vec4f(max(bathymetry, 0.0), 0.0, 0.0, 0.0);
  }
  let bed_distance = frame.clipProjection[3][2] * ray_length / bed_depth;
  let separation = bed_distance - sea_distance;
  let cull = OCEAN_NEAR_CULL * sea_distance;
  if (separation < -cull) {
    return vec4f(0.0, 1.0, 0.0, 1.0);
  }
  if (separation <= cull) {
    return vec4f(0.0, 1.0, 1.0, 0.0);
  }
  return vec4f(separation * view_cosine, 1.0, 0.0, 0.0);
}

// The bed's point at `pixel` from the camera, body-fixed, m, by the scene's depth; w 0 where none.
fn ocean_near_bed_point(pixel : vec2i) -> vec4f {
  let size = vec2i(textureDimensions(ocean_scene_depth));
  let at = clamp(pixel, vec2i(0), size - vec2i(1));
  let depth = bitcast<f32>(textureLoad(ocean_scene_depth, at, 0).x);
  if (!(depth > 0.0)) {
    return vec4f(0.0);
  }
  let ndc = vec2f(
    (f32(at.x) + 0.5) / f32(size.x) * 2.0 - 1.0,
    1.0 - (f32(at.y) + 0.5) / f32(size.y) * 2.0,
  );
  let ray = ocean_near_view_ray(ndc);
  let d = normalize(ray.x * draw.viewX.xyz + ray.y * draw.viewY.xyz + ray.z * draw.viewZ.xyz);
  return vec4f(d * (frame.clipProjection[3][2] * length(ray) / depth), 1.0);
}

// The bed's slope against `up`, from its points at `pixel` and the next across and down, or back
// at the frame's last column and row: xyz the unit horizontal direction downhill, w tan β; 0 where
// any is undrawn or the bed is level.
fn ocean_near_bed_slope(pixel : vec2i, up : vec3f) -> vec4f {
  let size = vec2i(textureDimensions(ocean_scene_depth));
  let step = select(vec2i(1), vec2i(-1), pixel + vec2i(1) >= size);
  let p0 = ocean_near_bed_point(pixel);
  let px = ocean_near_bed_point(pixel + vec2i(step.x, 0));
  let py = ocean_near_bed_point(pixel + vec2i(0, step.y));
  if (p0.w * px.w * py.w < 0.5) {
    return vec4f(0.0);
  }
  let n = cross(px.xyz - p0.xyz, py.xyz - p0.xyz);
  if (!(length(n) > 0.0)) {
    return vec4f(0.0);
  }
  let c = dot(normalize(n), up);
  // The normal leans downhill; facing down, it is turned up first.
  let tilt = normalize(n) * sign(c) - up * abs(c);
  if (!(length(tilt) > 1e-6)) {
    return vec4f(0.0);
  }
  return vec4f(normalize(tilt), sqrt(max(1.0 - c * c, 0.0)) / max(abs(c), 1e-3));
}

// The draw's `OceanFar`.
fn ocean_near_far() -> OceanFar {
  return OceanFar(
    draw.farFigure,
    draw.farSun,
    draw.farSunLux,
    draw.farSkyLux,
    draw.farIndex,
    draw.farAttenuation,
    draw.farDeep,
    draw.farBed,
  );
}

// The view-space ray through the frame's projection at `ndc`, (x, y, −1).
fn ocean_near_view_ray(ndc : vec2f) -> vec3f {
  return vec3f(
    (ndc.x + frame.clipProjection[2][0]) / frame.clipProjection[0][0],
    (ndc.y + frame.clipProjection[2][1]) / frame.clipProjection[1][1],
    -1.0,
  );
}

// A point from the camera, body-fixed, m, in clip space.
fn ocean_near_clip(point : vec3f) -> vec4f {
  let p = vec3f(dot(point, draw.viewX.xyz), dot(point, draw.viewY.xyz), dot(point, draw.viewZ.xyz));
  return frame.clipProjection * vec4f(p, 1.0);
}

struct OceanNearVarying {
  @builtin(position) position : vec4f,
  // The Lagrangian point, m, in the wind's frame at the nadir.
  @location(0) lagrangian : vec2f,
  // The displaced point from the camera, body-fixed, m.
  @location(1) from_camera : vec3f,
  // The local sphere's up at the mean point.
  @location(2) up : vec3f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> OceanNearVarying {
  let ray = ocean_near_view_ray(position.xy);
  let d = normalize(ray.x * draw.viewX.xyz + ray.y * draw.viewY.xyz + ray.z * draw.viewZ.xyz);
  let hit = ocean_near_hit(d);
  // The displacement filtered by a grid cell's footprint, which its vertices sample (Bruneton's eq. 2).
  let cell = hit.t * draw.nearGrid.x / sqrt(max(dot(-normalize(hit.point), hit.up), OCEAN_NEAR_MIN_COSINE));
  // The shore's depth limit at the vertex's own pixel (`shoreAmplitudeScale`).
  let size = vec2f(textureDimensions(ocean_scene_depth));
  let pixel = vec2i(vec2f((position.x + 1.0) * 0.5, (1.0 - position.y) * 0.5) * size);
  let world = draw.camera.xyz + hit.point;
  let water = ocean_near_water_depth(
    pixel, length(ray), hit.t, abs(dot(normalize(hit.point), hit.up)), ocean_texel(ocean_map, world).depth,
  );
  let scale = shore_amplitude_scale(water.x, ocean_waves[0].y);
  let surface = ocean_near_surface(hit.lagrangian, cell, scale);
  let along = ocean_near_along(hit.up);
  let across = cross(hit.up, along);
  // The border's vertices, z 1, are a skirt the waves leave on the mean surface outside the view,
  // which the next ring's displaced vertices cannot pull inside it (`oceanGridMesh`).
  let shift = select(surface.displacement, vec3f(0.0), position.z > 0.5);
  let p = hit.point + along * shift.x + across * shift.y + hit.up * shift.z;
  var out : OceanNearVarying;
  out.position = ocean_near_clip(p);
  out.lagrangian = hit.lagrangian;
  out.from_camera = p;
  out.up = hit.up;
  return out;
}

struct OceanNearOut {
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@fragment
fn fragmentMain(v : OceanNearVarying) -> OceanNearOut {
  let p = v.from_camera;
  let sea_distance = length(p);
  let view = -p / sea_distance;
  let world = draw.camera.xyz + p;
  let texel = ocean_texel(ocean_map, world);
  // The sea's open water at the sea level alone: a closed basin is the far pass's.
  if (!texel.open || texel.level != 0.0) {
    discard;
  }
  let up = normalize(v.up);
  let footprint = sea_distance * draw.farFigure.z / sqrt(max(dot(view, up), OCEAN_NEAR_MIN_COSINE));
  // The water at this pixel, from the scene's depth before the pass.
  let pixel = vec2i(v.position.xy);
  let ndc = vec2f(
    v.position.x * frame.viewport.z * 2.0 - 1.0,
    1.0 - v.position.y * frame.viewport.w * 2.0,
  );
  let ray_length = length(ocean_near_view_ray(ndc));
  let depth_here = ocean_near_water_depth(pixel, ray_length, sea_distance, abs(dot(view, up)), texel.depth);
  if (depth_here.w > 0.5) {
    discard;
  }
  var out : OceanNearOut;
  out.depth = select(v.position.z, bitcast<f32>(textureLoad(ocean_scene_depth, pixel, 0).x), depth_here.z > 0.5);
  var water : OceanNearWater;
  water.depth = depth_here.x;
  water.has_bed = false;
  // The shore: the waves limited by the depth, its foam by the depth and the bed's slope.
  let rms_height = ocean_waves[0].y;
  let slope = select(vec4f(0.0), ocean_near_bed_slope(pixel, up), depth_here.y > 0.5);
  let scale = shore_amplitude_scale(water.depth, rms_height);
  let shore_foam = shore_foam_share(water.depth, slope.w, rms_height, ocean_waves[0].z);
  // The fade over the pixel's reach along the slope, from the view's horizontal direction.
  let view_cosine = dot(view, up);
  let view_flat = view - up * view_cosine;
  let slope_along_view = select(0.0, abs(dot(normalize(view_flat), slope.xyz)), length(view_flat) > 1e-6);
  let reach = shore_pixel_reach(
    sea_distance, draw.farFigure.z, view_cosine, slope_along_view, OCEAN_NEAR_MIN_COSINE,
  );
  let fade = select(1.0, shore_fade(water.depth, reach, slope.w), depth_here.y > 0.5);
  let surface = ocean_near_surface(v.lagrangian, footprint, scale);
  let along = ocean_near_along(up);
  let across = cross(up, along);
  let normal = normalize(along * surface.normal.x + across * surface.normal.y + up * surface.normal.z);
  if (depth_here.y > 0.5) {
    // Refraction, on the high setting: the bed where the ray refracted at the wave meets it.
    if (draw.camera.w > 0.5) {
      let refracted = refract(-view, normal, 1.0 / draw.farIndex.y);
      let down = max(-dot(refracted, up), OCEAN_NEAR_LEAST_REFRACTED);
      let bed_point = p + refracted * (water.depth / down);
      let clip = ocean_near_clip(bed_point);
      let size = vec2i(textureDimensions(ocean_scene_colour));
      var there = pixel;
      if (clip.w > 0.0) {
        let at = vec2f((clip.x / clip.w + 1.0) * 0.5, (1.0 - clip.y / clip.w) * 0.5) * vec2f(size);
        there = clamp(vec2i(at), vec2i(0), size - vec2i(1));
      }
      // A refracted ray that lands on ground nearer than the sea takes the pixel's own bed.
      let there_depth = bitcast<f32>(textureLoad(ocean_scene_depth, there, 0).x);
      if (there_depth > v.position.z) {
        there = pixel;
      }
      water.bed = textureLoad(ocean_scene_colour, there, 0).rgb;
      water.has_bed = true;
    }
  }
  var far = ocean_near_far();
  let height = draw.datum.z + surface.displacement.z;
  let latitude = ocean_atan2(up.z, length(up.xy));
  var radiance = vec3f(0.0);
  let sources = min(u32(draw.datum.w), 2u);
  for (var j = 0u; j < sources; j = j + 1u) {
    var direction = draw.source0;
    var light = draw.sourceLight0.xyz;
    if (j == 1u) {
      direction = draw.source1;
      light = draw.sourceLight1.xyz;
    }
    let s = direction.xyz;
    let slot = u32(direction.w);
    let mu0 = dot(up, s);
    let visible = ocean_source_visible(j, world);
    let transmitted = visible * atmosphere_sun_transmittance(height, mu0, latitude, ocean_azimuth(up, s), slot);
    let sky = visible * atmosphere_sky_irradiance(height, mu0, latitude, slot);
    far.sun = vec4f(s, draw.farSun.w);
    // The bed's own light counted once, with the first source.
    var water_j = water;
    if (j > 0u && water.has_bed) {
      water_j.bed = vec3f(0.0);
    }
    radiance = radiance + ocean_near_light(
      far, OceanLight(light * transmitted, light * sky), view, up, normal, texel, surface.resolved,
      footprint, water_j, shore_foam,
    );
  }
  // Premultiplied by the depth fade: the bed shows through the thinnest water (`shoreFade`).
  out.colour = vec4f(min(radiance, vec3f(OCEAN_HDR_MAX)) * fade, fade);
  return out;
}

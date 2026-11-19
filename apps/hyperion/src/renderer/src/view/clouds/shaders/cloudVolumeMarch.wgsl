// The cloud volume's march (plan R11, R11.T7.c; Design notes 10 and 18): the high setting's clouds of
// one body at a quarter of the view's pixels, into the volume's own target, which its temporal
// resolve (`cloudVolumeResolve.wgsl`) accumulates and its composite (`cloudVolumeComposite.wgsl`)
// lays over the body's air, as the two-dimensional layer is laid (`cloudLayer.wgsl`).
//
// Each pixel's ray marches the deck's first stretch, from above, below or within it
// (`cloudVolume.wgsl`'s `cloud_deck_stretch`), cut where the scene's depth stands nearer, through the
// three-dimensional density (`cloud_march`; `cloudMarch.ts`' remarks), its samples offset by the
// blue noise at the pixel moved by the frame's jitter, so that the resolve's frames together sample
// the stretch evenly. The march's light is per unit of each source's irradiance face-on and of the
// sky's; each source lights the clouds through R08's transmittance to the place the ray's opacity
// is weighed at, at its geodetic height and latitude and the source's azimuth (R08 Design note 17),
// and the sky through R08's irradiance there. The air in front of that place is the layer's: its
// transmittance R08's ratio and its light (1 − T_f) J (`cloudLayer.wgsl`). The ray is drawn in its
// share of the pixel's footprint (`cloud_march`'s), so the target holds, premultiplied,
// T_f C + α (1 − T_f) J and α = share (1 − T_view).
//
// Composed after frame.wgsl, R08's surface lighting (`SURFACE_LIGHTING_WGSL`), `cloudShape.wgsl`,
// `cloudOptics.wgsl` and `cloudVolume.wgsl`.

struct Draw {
  offsetFromCameraM : vec3f,
  // The layer's members (`cloudLayer.wgsl`'s Draw), its view and pixel angle the march target's.
  camera : vec4f,
  cameraNormal : vec4f,
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  shell : vec4f,
  view : vec4f,
  source0 : vec4f,
  sourceLight0 : vec4f,
  source1 : vec4f,
  sourceLight1 : vec4f,
  // x: the view's samples; y: each march towards the source's; z: the frame's jitter in [0, 1);
  // w: 1 where the detail noise is read.
  levers : vec4f,
  // x, y: the deck's base and top above the datum, m; z, w: the scene depth's size, px.
  deck : vec4f,
  // The cloud map's plane (`cloudMap.ts`' `cloudMapUniforms`).
  mapAnchor : vec4f,
  mapEast : vec4f,
  mapNorth : vec4f,
  mapOrigins01 : vec4f,
  mapOrigins23 : vec4f,
  mapOrigins4 : vec4f,
  // x: φ, the flattening towards the layer's slab.
  march : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneDepth : texture_depth_2d;
@group(2) @binding(1) var cloud_map : texture_2d<f32>;
@group(2) @binding(2) var cloud_map_sampler : sampler;
@group(2) @binding(3) var<storage, read> cloud_layer : array<vec4f>;
@group(2) @binding(4) var<storage, read> cloud_shape : array<vec4f>;
// R08's surface lighting's nine tables, each by textureLoad.
@group(2) @binding(5) var atmosphere_body : texture_2d<f32>;
@group(2) @binding(6) var atmosphere_transmittance : texture_2d_array<f32>;
@group(2) @binding(7) var atmosphere_absorber_columns : texture_2d_array<f32>;
@group(2) @binding(8) var atmosphere_absorber_pressure_columns : texture_2d_array<f32>;
@group(2) @binding(9) var atmosphere_last_absorber_pressure_column : texture_2d_array<f32>;
@group(2) @binding(10) var atmosphere_absorber_curves : texture_2d<f32>;
@group(2) @binding(11) var atmosphere_irradiance : texture_2d_array<f32>;
@group(2) @binding(12) var atmosphere_below_deck : texture_2d_array<f32>;
@group(2) @binding(13) var atmosphere_source_table : texture_2d_array<f32>;
// The condensate's correction of its octaves (`cloudVolume.ts`' `octaveCorrectionOf`), `r32float`.
@group(2) @binding(14) var cloud_octave_correction : texture_2d<f32>;
// The view's blue-noise tile (`post/blueNoise.ts`), `r16float`, by textureLoad.
@group(2) @binding(15) var blue_noise : texture_2d<f32>;
// The world's shape noises (`cloudShapeNoise.ts`) and their sampler, linear and `repeat`.
@group(2) @binding(16) var cloud_base_noise : texture_3d<f32>;
@group(2) @binding(17) var cloud_detail_noise : texture_3d<f32>;
@group(2) @binding(18) var cloud_curl_noise : texture_2d<f32>;
@group(2) @binding(19) var cloud_noise_repeat : sampler;

// The layer's rows the march reads (`cloudField.wgsl`'s CLOUD_ROW_SHELL, CLOUD_ROW_OPTICS and
// CLOUD_ROW_NORM): x a_s and y c_s; x g and y 1 − ϖ₀; y the datum's equatorial radius a, m.
const CLOUD_MARCH_ROW_SHELL : u32 = 0u;
const CLOUD_MARCH_ROW_OPTICS : u32 = 1u;
const CLOUD_MARCH_ROW_NORM : u32 = 3u;
// The layer's least cosine and least transmittance (`cloudLayer.wgsl`'s).
const CLOUD_MARCH_LEAST_MU : f32 = 0.02;
const CLOUD_MARCH_LEAST_TRANSMITTANCE : f32 = 1e-6;
// The least opacity a pixel draws (`cloudLayer.wgsl`'s CLOUD_LEAST_SHARE).
const CLOUD_MARCH_LEAST_ALPHA : f32 = 1e-6;
const CLOUD_MARCH_PI : f32 = 3.14159265358979;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

// atan2(y, x) in [−π, π], by `preciseAtan2` (`cloudField.wgsl`'s `cloud_atan2`).
fn cloud_march_atan2(y : f32, x : f32) -> f32 {
  if (!(abs(y) > 0.0 || abs(x) > 0.0)) {
    return 0.0;
  }
  let angle = preciseAtan2(abs(y), x);
  return select(angle, -angle, y < 0.0);
}

// A place's local frame (`cloudLayer.wgsl`'s `cloud_place`).
struct CloudMarchPlace {
  normal : vec3f,
  north : vec3f,
  east : vec3f,
  latitude : f32,
}

fn cloud_march_place(normal : vec3f) -> CloudMarchPlace {
  var east = vec3f(-normal.y, normal.x, 0.0);
  let length_east = length(east);
  east = select(vec3f(0.0, 1.0, 0.0), east / length_east, length_east > 1e-6);
  let north = cross(normal, east);
  let latitude = cloud_march_atan2(normal.z, length(normal.xy));
  return CloudMarchPlace(normal, north, east, latitude);
}

fn cloud_march_azimuth(place : CloudMarchPlace, d : vec3f) -> f32 {
  return cloud_march_atan2(dot(d, place.east), dot(d, place.north));
}

// Rayleigh's phase function, per steradian.
fn cloud_march_rayleigh(cosine : f32) -> f32 {
  return 0.0596831 * (1.0 + cosine * cosine);
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let uv = position.xy / draw.view.xy;
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let unnormalised = draw.forward.xyz + ndc.x * draw.right.xyz + ndc.y * draw.upRay.xyz;
  let d = normalize(unnormalised);
  let o = draw.camera.xyz;
  let shell = cloud_layer[CLOUD_MARCH_ROW_SHELL];
  let optics = cloud_layer[CLOUD_MARCH_ROW_OPTICS];
  let datum = cloud_layer[CLOUD_MARCH_ROW_NORM].y;
  let k = shell.x / shell.y;
  let base_m = draw.deck.x;
  let top_m = draw.deck.y;
  let stretch = cloud_deck_stretch(o, d, k, datum, base_m, top_m);
  if (stretch.y <= 0.0) {
    return vec4f(0.0);
  }
  let t0 = stretch.x;
  var t1 = stretch.y;
  // The scene in front of the deck's far end: reversed-Z, 0 at infinity, at the pixel's centre.
  let full = vec2f(draw.deck.zw);
  let at = clamp(vec2i(uv * full), vec2i(0), vec2i(full) - vec2i(1));
  let depth = textureLoad(sceneDepth, at, 0);
  if (depth > 0.0) {
    t1 = min(t1, draw.shell.z / depth * length(unnormalised));
  }
  if (!(t1 > t0)) {
    return vec4f(0.0);
  }
  let noise_size = vec2u(textureDimensions(blue_noise));
  let noise = textureLoad(blue_noise, vec2u(position.xy) % noise_size, 0).r;
  let jitter = fract(noise + draw.levers.z);
  let footprint_per_m = draw.shell.w;
  let m = CloudMapPlane(
    draw.mapAnchor, draw.mapEast, draw.mapNorth, draw.mapOrigins01, draw.mapOrigins23, draw.mapOrigins4
  );
  // The column at the entry, over the pixel's footprint there, which sets the ray's share.
  let entry = cloud_march_entry(m, o, d, t0, k, footprint_per_m);
  let deck = CloudMarchDeck(
    base_m, top_m, max(top_m - base_m, 1.0), datum, k, optics.x, optics.y, draw.march.x
  );
  let levers = CloudMarchLevers(max(u32(draw.levers.x), 1u), max(u32(draw.levers.y), 1u), draw.levers.w);
  let sources = min(u32(draw.view.w), 2u);
  let march = cloud_march(
    m, deck, levers, o, d, t0, t1, draw.source0.xyz, draw.source1.xyz, sources, footprint_per_m,
    jitter, entry
  );
  let alpha = march.share * (1.0 - march.transmittance);
  if (alpha < CLOUD_MARCH_LEAST_ALPHA) {
    return vec4f(0.0);
  }
  // The place the ray's opacity is weighed at: where its sources' light, the sky's and the air in
  // front are taken.
  let x = o + march.mean_t * d;
  let height = length(vec3f(x.xy, x.z * k)) - datum;
  let normal = normalize(vec3f(x.xy, x.z * k * k));
  let place = cloud_march_place(normal);
  var radiance = vec3f(0.0);
  var source_function = vec3f(0.0);
  for (var j = 0u; j < sources; j = j + 1u) {
    var direction = draw.source0;
    var light = draw.sourceLight0.xyz;
    var sun = march.sun.x;
    if (j == 1u) {
      direction = draw.source1;
      light = draw.sourceLight1.xyz;
      sun = march.sun.y;
    }
    let s = direction.xyz;
    let slot = u32(direction.w);
    let mu0 = dot(normal, s);
    let transmitted = atmosphere_sun_transmittance(height, mu0, place.latitude, cloud_march_azimuth(place, s), slot);
    let sky_light = atmosphere_sky_irradiance(height, mu0, place.latitude, slot);
    radiance = radiance + light * (transmitted * sun + sky_light * (march.sky / CLOUD_MARCH_PI));
    source_function = source_function
      + light * (transmitted * cloud_march_rayleigh(dot(s, d)) + sky_light * 0.15915494);
  }
  radiance = march.share * radiance;
  // The air in front of that place (`cloudLayer.wgsl`'s).
  let above = dot(d, normal) < 0.0;
  var front = vec3f(1.0);
  if (above) {
    let up = -d;
    front = atmosphere_sun_transmittance(height, dot(normal, up), place.latitude, cloud_march_azimuth(place, up), 0u);
    if (draw.camera.w < draw.view.z) {
      let camera_place = cloud_march_place(draw.cameraNormal.xyz);
      let at_camera = atmosphere_sun_transmittance(
        draw.camera.w, dot(camera_place.normal, up), draw.cameraNormal.w,
        cloud_march_azimuth(camera_place, up), 0u
      );
      front = front / max(at_camera, vec3f(CLOUD_MARCH_LEAST_TRANSMITTANCE));
    }
  } else {
    let camera_place = cloud_march_place(draw.cameraNormal.xyz);
    let from_camera = atmosphere_sun_transmittance(
      draw.camera.w, dot(camera_place.normal, d), draw.cameraNormal.w,
      cloud_march_azimuth(camera_place, d), 0u
    );
    let from_entry = atmosphere_sun_transmittance(height, dot(normal, d), place.latitude, cloud_march_azimuth(place, d), 0u);
    front = from_camera / max(from_entry, vec3f(CLOUD_MARCH_LEAST_TRANSMITTANCE));
  }
  front = clamp(front, vec3f(0.0), vec3f(1.0));
  return vec4f(front * radiance + alpha * (1.0 - front) * source_function, alpha);
}

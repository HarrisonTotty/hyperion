// The cloud volume's march (plan R11, R11.T7.c; Design notes 10 and 18): the high setting's clouds of
// one body at a quarter of the view's pixels, into the volume's own target, which its temporal
// resolve (`cloudVolumeResolve.wgsl`) accumulates and its composite (`cloudVolumeComposite.wgsl`)
// lays over the body's air, as the two-dimensional layer is laid (`cloudLayer.wgsl`).
//
// Each pixel's ray marches the deck's first stretch between its base and its top
// (`cloudVolume.wgsl`'s `cloud_volume_march_between`), cut where the scene's depth stands nearer,
// its steps offset by the blue noise at the pixel moved by the frame's jitter, so that the resolve's
// frames together sample the stretch evenly. Each source lights it through R08's transmittance to
// the deck's entry, at its geodetic height and latitude and the source's azimuth (R08 Design note
// 17). The march draws the covered part of the pixel's footprint alone, each column its in-cloud
// τ, and weighs its light and opacity by the covered share f the field gives over the entry, so
// that the volume is the layer's independent-pixel form (Design note 10), the same at a coarse
// footprint as at a fine one. The sky's light, which the march does not carry, takes δ-Eddington's
// at the diffusivity's cosine of the covered column, held to the march's opacity of it. The air in front of the entry is the layer's:
// its transmittance R08's ratio and its light (1 − T_f) J (`cloudLayer.wgsl`). The cloud is seen
// from above where the ray enters it going down.
// So the target holds, premultiplied, T_f C + α (1 − T_f) J and α = f (1 − T_view).
//
// Composed after frame.wgsl, R08's surface lighting (`SURFACE_LIGHTING_WGSL`), `cloudField.wgsl`,
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
  // x: the octaves the field draws at most; y: the march's steps; z: the frame's jitter in [0, 1).
  levers : vec4f,
  // x, y: the deck's base and top above the datum, m; z, w: the scene depth's size, px.
  deck : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneDepth : texture_depth_2d;
@group(2) @binding(1) var cloud_coverage : texture_2d<f32>;
@group(2) @binding(2) var cloud_noise : texture_3d<f32>;
@group(2) @binding(3) var cloud_sampler : sampler;
@group(2) @binding(4) var<storage, read> cloud_layer : array<vec4f>;
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

// The layer's least cosine, sky cosine and least transmittance (`cloudLayer.wgsl`'s).
const CLOUD_MARCH_LEAST_MU : f32 = 0.02;
const CLOUD_MARCH_SKY_MU : f32 = 0.6;
const CLOUD_MARCH_LEAST_TRANSMITTANCE : f32 = 1e-6;
// The share below which a pixel draws nothing (`cloudLayer.wgsl`'s CLOUD_LEAST_DRAWN).
const CLOUD_MARCH_LEAST_DRAWN : f32 = 1e-4;
// The least opacity of the slab's column the sky's light is held to the march's by, and the most
// that holding may raise it.
const CLOUD_MARCH_LEAST_OPACITY : f32 = 1e-4;
const CLOUD_MARCH_MOST_SKY_RATIO : f32 = 4.0;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
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
  let latitude = cloud_atan2(normal.z, length(normal.xy));
  return CloudMarchPlace(normal, north, east, latitude);
}

fn cloud_march_azimuth(place : CloudMarchPlace, d : vec3f) -> f32 {
  return cloud_atan2(dot(d, place.east), dot(d, place.north));
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
  let shell = cloud_row(CLOUD_ROW_SHELL);
  let optics = cloud_row(CLOUD_ROW_OPTICS);
  let datum = cloud_row(CLOUD_ROW_NORM).y;
  let k = shell.x / shell.y;
  let base_m = draw.deck.x;
  let top_m = draw.deck.y;
  // The deck's first stretch along the ray, as `cloud_volume_march` takes it.
  let outer = cloud_spheroid_hits(o, d, k, datum + top_m);
  if (outer.y <= 0.0) {
    return vec4f(0.0);
  }
  let inner = cloud_spheroid_hits(o, d, k, datum + base_m);
  var t0 = max(outer.x, 0.0);
  var t1 = outer.y;
  if (inner.x > t0) {
    t1 = inner.x;
  } else if (inner.x < 0.0 && inner.y > t0) {
    t0 = inner.y;
  }
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
  let steps = max(u32(draw.levers.y), 1u);
  let dt = (t1 - t0) / f32(steps);
  let noise_size = vec2u(textureDimensions(blue_noise));
  let noise = textureLoad(blue_noise, vec2u(position.xy) % noise_size, 0).r;
  let shift = (fract(noise + draw.levers.z) - 0.5) * dt;
  let footprint_per_m = draw.shell.w;
  // The deck's entry: where its sources' light and the air in front are taken.
  let x0 = o + t0 * d;
  let sx0 = vec3f(x0.xy, x0.z * k);
  let height = length(sx0) - datum;
  let normal = normalize(vec3f(x0.xy, x0.z * k * k));
  let place = cloud_march_place(normal);
  // The covered share of the pixel's footprint and its in-cloud τ, the layer's at the column over
  // the entry: the march draws the covered part alone, weighed by its share, the layer's
  // independent-pixel form (Design note 10).
  let mu_view = max(abs(dot(d, normal)), CLOUD_MARCH_LEAST_MU);
  let column = cloud_field_at(
    x0 * (shell.x / length(sx0)), max(footprint_per_m * t0 / mu_view, 1.0), u32(draw.levers.x)
  );
  let share = column.fraction;
  if (share < CLOUD_MARCH_LEAST_DRAWN) {
    return vec4f(0.0);
  }
  var radiance = vec3f(0.0);
  var sky = vec3f(0.0);
  var source_function = vec3f(0.0);
  var view = 1.0;
  let sources = min(u32(draw.view.w), 2u);
  for (var j = 0u; j < sources; j = j + 1u) {
    var direction = draw.source0;
    var light = draw.sourceLight0.xyz;
    if (j == 1u) {
      direction = draw.source1;
      light = draw.sourceLight1.xyz;
    }
    let s = direction.xyz;
    let slot = u32(direction.w);
    let mu0 = dot(normal, s);
    let transmitted = atmosphere_sun_transmittance(height, mu0, place.latitude, cloud_march_azimuth(place, s), slot);
    let sky_light = atmosphere_sky_irradiance(height, mu0, place.latitude, slot);
    let march = cloud_volume_march_between(
      o, d, s, base_m, top_m, t0 + shift, t1 + shift, steps, footprint_per_m, k, shell, optics, datum, share
    );
    radiance = radiance + light * transmitted * march.x;
    sky = sky + light * sky_light;
    view = march.y;
    source_function = source_function
      + light * (transmitted * cloud_march_rayleigh(dot(s, d)) + sky_light * 0.15915494);
  }
  if (sources == 0u) {
    view = cloud_volume_march_between(
      o, d, normal, base_m, top_m, t0 + shift, t1 + shift, steps, footprint_per_m, k, shell, optics, datum, share
    ).y;
  }
  // The covered part's opacity, then the footprint's.
  let covered = 1.0 - view;
  let alpha = share * covered;
  if (alpha < CLOUD_LEAST_SHARE) {
    return vec4f(0.0);
  }
  // The sky's light: δ-Eddington's of the covered part's column at the diffusivity's cosine, held to
  // the march's opacity of it, which the scene's depth may cut.
  let sky_slab = cloud_delta_eddington(column.in_cloud_tau, optics.x, optics.y, CLOUD_MARCH_SKY_MU);
  let above = dot(d, normal) < 0.0;
  let sky_share = select(sky_slab.diffuse, sky_slab.reflectance, above);
  let column_opacity = 1.0 - exp(-column.in_cloud_tau / mu_view);
  let held = min(covered / max(column_opacity, CLOUD_MARCH_LEAST_OPACITY), CLOUD_MARCH_MOST_SKY_RATIO);
  radiance = share * (radiance + sky * (sky_share * held * 0.31830989));
  // The air in front of the entry (`cloudLayer.wgsl`'s).
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

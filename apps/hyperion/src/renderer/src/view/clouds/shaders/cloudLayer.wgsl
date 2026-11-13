// The two-dimensional cloud layer (plan R11, R11.T7.b; Design notes 8, 10 and 18): the low setting's
// clouds, and the high setting's where its frame-rate floor steps the volume down. One full-screen
// draw a body, after the body's atmosphere composite, into the target that composite wrote, blended
// premultiplied so that the meter class beneath stays (R07 Design note 10; R01 Design note 21). The
// layer is a shell on the spheroid of semi-axes a + h and c + h, h the condensate's height above
// the datum; each pixel's ray meets it once, from above or from below, where the scene's depth does
// not stand nearer; it writes no depth.
//
// At the hit, the field (`cloudField.wgsl`) gives the footprint's covered share f and in-cloud τ.
// The covered share reflects or transmits each source by δ-Eddington (`cloudOptics.wgsl`) at the
// source's zenith cosine against the shell's normal, lit through R08's surface lighting at the shell's
// geodetic height and latitude and the source's azimuth from local north (`surfaceLighting.wgsl`'s
// `atmosphere_sun_transmittance` and `atmosphere_sky_irradiance`, R08 Design note 17), its radiance
// that flux taken as Lambertian, π⁻¹ of it a steradian (lean, 2026-11-02): from above,
// R(μ₀) μ₀ E T_sun + R(0.6) E_sky, the sky's light at 0.6, the diffusivity's cosine; from below,
// T_diff(μ₀) μ₀ E T_sun + T_diff(0.6) E_sky, the sky's unscattered light being the background's,
// seen through the cloud. The background is seen through it by e^(−τ ÷ μ_v) of the
// covered share, μ_v the view ray's cosine against the normal.
//
// The air between the camera and the hit (lean, 2026-11-02): its transmittance T_f is R08's, the
// ratio of the transmittances to space from the lower of the two ends and from the upper along the
// upward direction between them, the camera's own taken as 1 above the air; its light, which the
// composite laid over the whole ray, is (1 − T_f) J, J the single-scattering source of each source,
// its light times the transmittance to the hit and Rayleigh's phase function at their angle, with the
// sky's mean intensity for the multiple scattering, E_sky ÷ 2π for an isotropic upper hemisphere of
// irradiance E_sky. So the blend is
// out = T_c dst + (1 − T_c) S_f + T_f C, T_c the background's share, C the cloud's light.
//
// Composed after frame.wgsl, R08's surface lighting (`SURFACE_LIGHTING_WGSL`: preciseTrig.wgsl,
// common.wgsl, oblate.wgsl, belowDeck.wgsl, surfaceLighting.wgsl), `cloudField.wgsl` and
// `cloudOptics.wgsl`.

struct Draw {
  offsetFromCameraM : vec3f,
  // xyz: the camera from the body's centre, body-fixed, m; w: its geodetic height above the datum.
  camera : vec4f,
  // xyz: the datum's outward normal under the camera; w: its geodetic latitude, rad.
  cameraNormal : vec4f,
  // The camera's ray basis, body-fixed: right × tan(fovX ÷ 2), up × tan(fovY ÷ 2), forward.
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  // x: |S o|² − a_s², S = diag(1, 1, a_s ÷ c_s), o the camera, formed in f64; y: a_s ÷ c_s; z: the
  // near plane, m; w: the angle a pixel subtends at the centre of the view, rad.
  shell : vec4f,
  // x, y: the output's size, px; z: the air's top above the datum, m; w: the sources drawn, 0 to 2.
  view : vec4f,
  // Each source: xyz its unit direction, body-fixed; w its light slot.
  source0 : vec4f,
  // Its illuminance face-on per channel times the frame's pre-exposure, lx.
  sourceLight0 : vec4f,
  source1 : vec4f,
  sourceLight1 : vec4f,
  // x: the octaves the layer draws at most.
  levers : vec4f,
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

// The least zenith cosine δ-Eddington is taken at, for a source at or below the shell's horizon.
const CLOUD_LEAST_MU : f32 = 0.02;
// The zenith cosine of the sky's light on the layer: 0.6, the two-stream diffusivity's 1 ÷ 1.66.
const CLOUD_SKY_MU : f32 = 0.6;
// The least transmittance the air's ratio divides by.
const CLOUD_LEAST_TRANSMITTANCE : f32 = 1e-6;
// The share below which a pixel draws nothing.
const CLOUD_LEAST_DRAWN : f32 = 1e-4;

struct VertexOut {
  @builtin(position) position : vec4f,
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> VertexOut {
  // A full-screen triangle in clip space, drawn with no depth test.
  return VertexOut(vec4f(position.xy, 1.0, 1.0));
}

// A place's local frame: its outward normal, north and east, and its geodetic latitude.
struct CloudPlace {
  normal : vec3f,
  north : vec3f,
  east : vec3f,
  latitude : f32,
}

fn cloud_place(normal : vec3f) -> CloudPlace {
  var east = vec3f(-normal.y, normal.x, 0.0);
  let length_east = length(east);
  east = select(vec3f(0.0, 1.0, 0.0), east / length_east, length_east > 1e-6);
  let north = cross(normal, east);
  let latitude = cloud_atan2(normal.z, length(normal.xy));
  return CloudPlace(normal, north, east, latitude);
}

// A direction's azimuth from local north towards east, rad.
fn cloud_azimuth(place : CloudPlace, d : vec3f) -> f32 {
  return cloud_atan2(dot(d, place.east), dot(d, place.north));
}

// Rayleigh's phase function at the cosine of the scattering angle, per steradian.
fn cloud_rayleigh_phase(cosine : f32) -> f32 {
  return 0.0596831 * (1.0 + cosine * cosine);
}

@fragment
fn fragmentMain(@builtin(position) position : vec4f) -> @location(0) vec4f {
  let uv = position.xy / draw.view.xy;
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let unnormalised = draw.forward.xyz + ndc.x * draw.right.xyz + ndc.y * draw.upRay.xyz;
  let d = normalize(unnormalised);
  let o = draw.camera.xyz;
  let k = draw.shell.y;
  let so = vec3f(o.xy, o.z * k);
  let sd = vec3f(d.xy, d.z * k);
  let a = dot(sd, sd);
  let b = dot(so, sd);
  let c = draw.shell.x;
  let disc = b * b - a * c;
  if (disc < 0.0) {
    discard;
  }
  let root = sqrt(disc);
  let above = c > 0.0;
  var t : f32;
  if (above) {
    if (b >= 0.0) {
      discard;
    }
    t = c / (-b + root);
  } else {
    t = (-b + root) / a;
  }
  // The scene's depth stands in front: reversed-Z, 0 at infinity.
  let depth = textureLoad(sceneDepth, vec2i(position.xy), 0);
  if (depth > 0.0 && t >= draw.shell.z / depth * length(unnormalised)) {
    discard;
  }
  let p = o + t * d;
  let shell = cloud_row(CLOUD_ROW_SHELL);
  let normal = normalize(vec3f(p.xy, p.z * k * k));
  let mu_view = max(abs(dot(d, normal)), CLOUD_LEAST_MU);
  let footprint = max(t * draw.shell.w / mu_view, 1.0);
  let field = cloud_field_at(p, footprint, u32(draw.levers.x));
  if (field.fraction < CLOUD_LEAST_DRAWN) {
    discard;
  }
  let tau = field.in_cloud_tau;
  let optics = cloud_row(CLOUD_ROW_OPTICS);
  let place = cloud_place(normal);
  let height = shell.z;
  let sky_slab = cloud_delta_eddington(tau, optics.x, optics.y, CLOUD_SKY_MU);
  var radiance = vec3f(0.0);
  var source_function = vec3f(0.0);
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
    let transmitted = atmosphere_sun_transmittance(height, mu0, place.latitude, cloud_azimuth(place, s), slot);
    let sky = atmosphere_sky_irradiance(height, mu0, place.latitude, slot);
    let slab = cloud_delta_eddington(tau, optics.x, optics.y, max(mu0, CLOUD_LEAST_MU));
    let lit = max(mu0, 0.0) * transmitted;
    if (above) {
      radiance = radiance + light * (slab.reflectance * lit + sky_slab.reflectance * sky);
    } else {
      radiance = radiance + light * (slab.diffuse * lit + sky_slab.diffuse * sky);
    }
    source_function = source_function
      + light * (transmitted * cloud_rayleigh_phase(dot(s, d)) + sky * 0.15915494);
  }
  radiance = radiance * 0.31830989;
  // The air in front of the hit: the transmittances to space from the lower end and the upper along
  // the upward direction between them.
  var front = vec3f(1.0);
  if (above) {
    let up = -d;
    front = atmosphere_sun_transmittance(height, dot(normal, up), place.latitude, cloud_azimuth(place, up), 0u);
    if (draw.camera.w < draw.view.z) {
      let camera_place = cloud_place(draw.cameraNormal.xyz);
      let at_camera = atmosphere_sun_transmittance(
        draw.camera.w, dot(camera_place.normal, up), draw.cameraNormal.w,
        cloud_azimuth(camera_place, up), 0u
      );
      front = front / max(at_camera, vec3f(CLOUD_LEAST_TRANSMITTANCE));
    }
  } else {
    let camera_place = cloud_place(draw.cameraNormal.xyz);
    let from_camera = atmosphere_sun_transmittance(
      draw.camera.w, dot(camera_place.normal, d), draw.cameraNormal.w,
      cloud_azimuth(camera_place, d), 0u
    );
    let from_hit = atmosphere_sun_transmittance(height, dot(normal, d), place.latitude, cloud_azimuth(place, d), 0u);
    front = from_camera / max(from_hit, vec3f(CLOUD_LEAST_TRANSMITTANCE));
  }
  front = clamp(front, vec3f(0.0), vec3f(1.0));
  let alpha = field.fraction * (1.0 - exp(-tau / mu_view));
  let coloured = front * field.fraction * radiance + alpha * (1.0 - front) * source_function;
  return vec4f(coloured, alpha);
}

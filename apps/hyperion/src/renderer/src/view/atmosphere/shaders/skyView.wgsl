// The sky-view table (plan R05, Design note 16; Hillaire 2020, section 5.2): the sky's luminance
// per unit sun illuminance around the camera, by view zenith and azimuth from the sun, rebuilt
// every frame. Follows `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl`.
//
// Ported from Bevy 0.19.1's sky_view_lut.wgsl (MIT; the notice is in common.wgsl), with sebh's
// SkyViewLutCS parameterisation and sample counts (MIT; the notice is in multiScattering.wgsl):
// the table is symmetric about the sun's vertical plane, so its azimuth spans [0, pi]. The sphere
// is the camera's own, of its Gaussian radius sqrt(MN) (Design note 16): `medium`'s ground is at
// that radius and the camera at its height above it. No ground bounce, as sebh's.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<uniform> view : AtmosphereView;
@group(0) @binding(2) var transmittance : texture_2d<f32>;
@group(0) @binding(3) var multiScattering : texture_2d<f32>;
@group(0) @binding(4) var skyViewOut : texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(skyViewOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let sizeF = vec2f(size);
  let bottom = medium.bottomRadiusM;
  let top = medium.topRadiusM;
  // The sky-view table serves cameras inside the atmosphere; above it the composite ray-marches.
  let heightM = skyViewHeight(view.camera.w, top - bottom);
  let r = bottom + heightM;
  let params = skyViewUvToParams((vec2f(id.xy) + 0.5) / sizeF, sizeF, heightM, bottom);
  let viewZenithCos = params.x;
  let lightViewCos = params.y;
  let viewZenithSin = sqrt(max(1.0 - viewZenithCos * viewZenithCos, 0.0));
  let dir = vec3f(
    viewZenithSin * lightViewCos,
    viewZenithSin * sqrt(max(1.0 - lightViewCos * lightViewCos, 0.0)),
    viewZenithCos,
  );
  let muSunCamera = dot(view.sun.xyz, view.up.xyz);
  let sun = vec3f(sqrt(max(1.0 - muSunCamera * muSunCamera, 0.0)), 0.0, muSunCamera);
  let cosTheta = dot(dir, sun);

  let origin = vec3f(0.0, 0.0, r);
  let tMax = maxDistance(r, viewZenithCos);
  let samples = max(u32(view.output.w), 1u);
  let dt = tMax / f32(samples);
  var luminance = vec3f(0.0);
  var throughput = vec3f(1.0);
  for (var i = 0u; i < samples; i++) {
    let p = origin + ((f32(i) + 0.5) * dt) * dir;
    let rP = length(p);
    let sampleHeightM = rP - bottom;
    let muSun = dot(sun, p / rP);
    let local = mediumAt(rP);
    let extinction = max(local.extinction, vec3f(1e-12));
    let stepTransmittance = exp(-local.extinction * dt);
    let source = sourceAt(
      transmittance,
      multiScattering,
      view.tables.x,
      sampleHeightM,
      muSun,
      cosTheta,
    );
    luminance += throughput * (source - source * stepTransmittance) / extinction;
    throughput *= stepTransmittance;
  }
  textureStore(skyViewOut, id.xy, vec4f(luminance, 1.0));
}

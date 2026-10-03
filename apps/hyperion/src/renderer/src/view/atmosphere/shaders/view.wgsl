// The atmosphere's per-frame WGSL (plan R05, R05.T12.c, Design note 16): the view's uniform and the
// single- and multiple-scattering source at a point, read from the per-planet tables. Prepended,
// after `common.wgsl`, to the sky-view, aerial-perspective and ray-march kernels and the composite.
//
// Ported from Bevy 0.19.1's functions.wgsl (`sample_local_inscattering`, MIT; the notice is in
// common.wgsl) and checked against sebh's IntegrateScatteredLuminance (MIT; the notice is in
// multiScattering.wgsl). Changes: the phase functions are the medium's own terms'; the tables are
// read on their own sphere, at the sample's height above the datum (Design note 16).

const PI_VIEW : f32 = 3.14159265358979;

// The view, in the body-fixed frame (z along the pole), metres.
struct AtmosphereView {
  // xyz: the camera's position from the body's centre; w: its geodetic height above the datum.
  camera : vec4f,
  // xyz: the datum's outward normal under the camera; w: the camera's Gaussian radius sqrt(MN).
  up : vec4f,
  // xyz: the unit direction to the sun; w: the sun's angular radius, rad.
  sun : vec4f,
  // rgb: a table radiance's factor to pre-exposed luminance (Rec. 709); w: the exposure scale.
  skyScale : vec4f,
  // rgb: the sun disc's pre-exposed luminance at the top of the atmosphere; w: unused.
  sunDisc : vec4f,
  // The camera's ray basis in the body-fixed frame: right x tan(fovX / 2), up x tan(fovY / 2),
  // forward.
  right : vec4f,
  upRay : vec4f,
  forward : vec4f,
  // x: the tables' ground radius; y: the aerial-perspective reach, m; z: its slices; w: its
  // samples a slice.
  tables : vec4f,
  // x, y: the datum's equatorial and polar radii; z: the near plane, m; w: the ray march's samples.
  figure : vec4f,
  // x, y: the output's size in pixels; z: the ray march's scale (1 or 0.5); w: the sky-view
  // samples.
  output : vec4f,
  // rgb: the sun's spectral-to-luminance factors over the sky's, for sunlight the grey ground
  // reflects (Bruneton 2017, model.cc, GetSunAndSkyIlluminance); w: unused.
  sunOverSky : vec4f,
}

// The camera's height on the sky view's sphere: kept 1 m clear of the ground and the top, where
// the table's horizon split degenerates. The sky-view kernel and the composite both take it.
fn skyViewHeight(heightM : f32, atmosphereM : f32) -> f32 {
  return clamp(heightM, 1.0, atmosphereM - 1.0);
}

// Whether a ray from height h above a sphere of radius `bottom`, at zenith cosine mu, meets it:
// r^2 mu^2 >= r^2 - bottom^2, with r^2 - bottom^2 formed as h (2 bottom + h), which keeps its
// precision near the ground where the difference of squares would cancel in f32.
fn groundHitFromHeight(bottom : f32, heightM : f32, mu : f32) -> bool {
  let r = bottom + heightM;
  return mu < 0.0 && r * r * mu * mu >= heightM * (2.0 * bottom + heightM);
}

// The phase a term scatters with towards the eye, for the cosine between the view direction and
// the direction to the sun. Term.scattering.w: 0 none, 1 Rayleigh, 2 Cornette–Shanks with its g in
// Term.absorption.w.
fn phaseOf(term : Term, cosTheta : f32) -> f32 {
  let kind = term.scattering.w;
  if (kind < 0.5) {
    return 0.0;
  }
  if (kind < 1.5) {
    return 3.0 / (16.0 * PI_VIEW) * (1.0 + cosTheta * cosTheta);
  }
  let g = term.absorption.w;
  let k = 3.0 / (8.0 * PI_VIEW) * (1.0 - g * g) / (2.0 + g * g);
  return k * (1.0 + cosTheta * cosTheta) / pow(max(1.0 + g * g - 2.0 * g * cosTheta, 1e-6), 1.5);
}

// Each term's scattering times its phase, summed, at a height above the datum.
fn phasedScatteringAt(m : Medium, heightM : f32, cosTheta : f32) -> vec3f {
  var sum = vec3f(0.0);
  let count = min(u32(m.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = m.terms[i];
    sum += term.scattering.rgb * densityOf(term.profile, heightM) * phaseOf(term, cosTheta);
  }
  return sum;
}

// The transmittance from a height above the datum along a zenith cosine to the top, read from the
// table on the tables' own sphere.
fn tableTransmittance(
  table : texture_2d<f32>,
  m : Medium,
  tableBottomM : f32,
  heightM : f32,
  mu : f32,
) -> vec3f {
  let top = tableBottomM + (m.topRadiusM - m.bottomRadiusM);
  let r = tableBottomM + clamp(heightM, 0.0, top - tableBottomM);
  return bilinear(table, shellRMuToUv(tableBottomM, top, r, mu)).rgb;
}

// The multiple-scattering source per unit illuminance at a height, for the sun's zenith cosine.
fn tableMultiScattering(
  table : texture_2d<f32>,
  m : Medium,
  tableBottomM : f32,
  heightM : f32,
  muSun : f32,
) -> vec3f {
  let top = tableBottomM + (m.topRadiusM - m.bottomRadiusM);
  let size = vec2f(textureDimensions(table));
  let r = tableBottomM + max(heightM, 0.0);
  return bilinear(table, shellMultiScatteringRMuToUv(tableBottomM, top, r, muSun, size)).rgb;
}

// The scattering source at a point per unit sun illuminance, per unit length: single scattering
// of the sun's light, shadowed by the ground, plus multiple scattering (Hillaire 2020, equations 3
// and 11).
fn sourceAt(
  m : Medium,
  transmittance : texture_2d<f32>,
  multiScattering : texture_2d<f32>,
  tableBottomM : f32,
  heightM : f32,
  muSun : f32,
  cosTheta : f32,
) -> vec3f {
  let rTable = tableBottomM + max(heightM, 0.0);
  let lit = select(1.0, 0.0, shellIntersectsGround(tableBottomM, rTable, muSun));
  let toSun = tableTransmittance(transmittance, m, tableBottomM, heightM, muSun);
  let single = lit * toSun * phasedScatteringAt(m, heightM, cosTheta);
  let local = mediumAt(m, m.bottomRadiusM + heightM);
  let multiple = tableMultiScattering(multiScattering, m, tableBottomM, heightM, muSun);
  return single + multiple * local.scattering;
}

// The ray through a point of the output, (0, 0) at its top left, in the body-fixed frame.
fn rayThrough(v : AtmosphereView, uv : vec2f) -> vec3f {
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  return normalize(v.forward.xyz + ndc.x * v.right.xyz + ndc.y * v.upRay.xyz);
}

// The distance along a pixel's ray to the surface at a reversed-Z depth, m: the view-space depth
// near ÷ depth, over the cosine between the ray and the forward axis.
fn distanceAtDepth(v : AtmosphereView, uv : vec2f, depth : f32) -> f32 {
  let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let unnormalised = v.forward.xyz + ndc.x * v.right.xyz + ndc.y * v.upRay.xyz;
  return v.figure.z / depth * length(unnormalised);
}

// The frame of the camera's sky: z the datum's normal, x towards the sun's azimuth, y completing
// it; the sky-view table is symmetric about the sun's vertical plane (sebh's parameterisation).
struct SkyFrame {
  x : vec3f,
  y : vec3f,
  z : vec3f,
}

fn skyFrame(v : AtmosphereView) -> SkyFrame {
  let z = v.up.xyz;
  var x = v.sun.xyz - dot(v.sun.xyz, z) * z;
  if (dot(x, x) < 1e-12) {
    // The sun at the zenith or the nadir: any horizontal axis will do.
    x = select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), abs(z.x) > 0.9);
    x = x - dot(x, z) * z;
  }
  x = normalize(x);
  return SkyFrame(x, cross(z, x), z);
}

// sebh's sky-view parameterisation (RenderSkyCommon.hlsl, SkyViewLutParamsToUv and its inverse):
// v splits at the horizon, each half with a square-root compression towards it; u is the azimuth
// from the sun's, compressed towards the sun. The camera stands `heightM` above a sphere of
// radius `bottom`, its own (Design note 16).
fn skyViewUvToParams(uv : vec2f, size : vec2f, heightM : f32, bottom : f32) -> vec2f {
  let unit = clamp(subUvsToUnit(uv, size), vec2f(0.0), vec2f(1.0));
  let r = bottom + heightM;
  let vHorizon = sqrt(max(heightM * (2.0 * bottom + heightM), 0.0));
  let beta = acos(clamp(vHorizon / r, -1.0, 1.0));
  let zenithHorizon = PI_VIEW - beta;
  var viewZenithCos : f32;
  if (unit.y < 0.5) {
    var c = 1.0 - 2.0 * unit.y;
    c = 1.0 - c * c;
    viewZenithCos = cos(zenithHorizon * c);
  } else {
    let c = unit.y * 2.0 - 1.0;
    viewZenithCos = cos(zenithHorizon + beta * c * c);
  }
  let lightViewCos = -(unit.x * unit.x * 2.0 - 1.0);
  return vec2f(viewZenithCos, lightViewCos);
}

fn skyViewParamsToUv(
  intersectsGroundView : bool,
  viewZenithCos : f32,
  lightViewCos : f32,
  size : vec2f,
  heightM : f32,
  bottom : f32,
) -> vec2f {
  let r = bottom + heightM;
  let vHorizon = sqrt(max(heightM * (2.0 * bottom + heightM), 0.0));
  let beta = acos(clamp(vHorizon / r, -1.0, 1.0));
  let zenithHorizon = PI_VIEW - beta;
  let zenith = acos(clamp(viewZenithCos, -1.0, 1.0));
  var v : f32;
  if (!intersectsGroundView) {
    let c = 1.0 - sqrt(max(1.0 - zenith / zenithHorizon, 0.0));
    v = c * 0.5;
  } else {
    let c = sqrt(max((zenith - zenithHorizon) / max(beta, 1e-6), 0.0));
    v = c * 0.5 + 0.5;
  }
  let u = sqrt(max(-lightViewCos * 0.5 + 0.5, 0.0));
  return unitToSubUvs(vec2f(u, v), size);
}

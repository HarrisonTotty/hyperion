// The atmosphere's per-frame WGSL (plan R05, R05.T12.c, Design note 16): the view's uniform, the
// camera's rays and the sky-view table's parameterisation. Prepended, after `common.wgsl`, to the
// composite and, after `medium.wgsl`, to the sky-view, aerial-perspective and ray-march kernels,
// which then take the scattering source (`source.wgsl`). It reads no medium. The sky-view
// parameterisation is sebh's (MIT; the notice is in multiScattering.wgsl), its angles taken by the
// precise forms of `view/shaders/preciseTrig.wgsl` and its texels by Bruneton's mapping (plan R08,
// Design notes 18 and 19).

const PI_VIEW : f32 = 3.14159265358979;

// The sources the sky's kernels hold (plan R08, R08.T7; `settings.ts`'s MAX_SKY_SOURCES): four
// stars and two planetshine sources (Design note 7).
const MAX_SKY_SOURCES : u32 = 6u;

// The light slots a march tracks a path for (`settings.ts`'s MAX_SKY_LIGHT_SLOTS): each star's
// own absorber curves; a planetshine source takes slot 0's.
const MAX_SKY_LIGHT_SLOTS : u32 = 4u;

// One of the sky's sources (plan R08, R08.T7; `hillaire.ts`'s FrameSource), formed in f64 and
// narrowed once.
struct SkySource {
  // xyz: the unit direction to the source, body-fixed; w: sin of its angular radius, formed in f64,
  // 0 for none (Design note 18's disc test |d x s| <= sin(rho) with d . s > 0).
  direction : vec4f,
  // rgb: its illuminance per channel times the frame's scale, the weight of its light in the
  // per-view tables; w: its light slot, as a float: the path, absorber curves and
  // multiple-scattering layer its light takes, 0 for every source of a medium with no absorber.
  weight : vec4f,
  // rgb: its disc's luminance at the top of the air times the frame's scale, its illuminance over
  // its solid angle, 0 where the composite does not draw it; w: unused.
  disc : vec4f,
}

// The view, in the body-fixed frame (z along the pole), metres.
struct AtmosphereView {
  // xyz: the camera's position from the body's centre; w: its geodetic height above the datum.
  camera : vec4f,
  // xyz: the datum's outward normal under the camera; w: the camera's Gaussian radius sqrt(MN).
  up : vec4f,
  // x: how many sources are drawn, 0 to MAX_SKY_SOURCES; y: the light slots the marches track a
  // path for, 1 to MAX_SKY_LIGHT_SLOTS; z: the multiple-scattering table's layers; w: the
  // composite's factor from the per-view tables to pre-exposed luminance, 1 while they hold
  // pre-exposed luminance (R08.T7; `decision-r08-t6b-floor.md` section 6).
  sky : vec4f,
  // xyz: the sky view's x axis, the first source's horizontal direction, formed in f64 (R08.T7):
  // the table's local azimuth is measured from it towards up x it.
  skyAxis : vec4f,
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
  // What f32 cannot form here, formed in f64 by `HillaireAtmosphere` and narrowed once (plan R08,
  // Design note 18): x and y, the ray march's ground and top-shell quadric constants,
  // q_g = |S P|^2 - a^2 and q_t = |S_t P|^2 - (a + H / s_e)^2, P the camera's position, S = diag(1,
  // 1, a / c) and S_t = diag(1, 1, (a + H / s_e) / (c + H / s_p)), the top the quadric through h* = H
  // at the equator and the poles (R08.T6.f; Design note 17); z, the camera's local scale height in
  // h*, which the composite's sliced lookup takes from the camera (`oblate.wgsl`'s LookupStart),
  // from the medium's own column (R08.T6.f); w, unused. The sky view's beta, z before R08.T6.f, is
  // each column's own since (`oblate.wgsl`'s skyViewColumnOf).
  exact : vec4f,
  // The camera on an oblate body (plan R08, R08.T6.f; Design note 17), formed in f64: x and y its
  // principal radii of curvature M and N, from which each sky-view column's sphere R_alpha is
  // taken; z s = g(phi) / g_ref at its latitude; w the sky view's atmosphere, the medium's top over
  // that s.
  curvature : vec4f,
  // The drawn sources, the first `sky.x` of them (R08.T7).
  sources : array<SkySource, MAX_SKY_SOURCES>,
}

// How many sources the view draws.
fn skySourceCount(v : AtmosphereView) -> u32 {
  return min(u32(v.sky.x), MAX_SKY_SOURCES);
}

// How many light slots the view's marches track a path for, at least 1.
fn skyPathCount(v : AtmosphereView) -> u32 {
  return clamp(u32(v.sky.y), 1u, MAX_SKY_LIGHT_SLOTS);
}

// A source's light slot: its path, its absorber curves and its multiple-scattering layer.
fn skySlotOf(source : SkySource) -> u32 {
  return min(u32(source.weight.w), MAX_SKY_LIGHT_SLOTS - 1u);
}

// The camera's height on the sky view's spheres: kept 1 m clear of the ground and the top, where
// the table's horizon split degenerates. The sky-view kernel and the composite both take it, the
// atmosphere's height `curvature.w` (R08.T6.f), and `hillaire.ts`'s `skyViewHeight` is its twin.
fn skyViewHeight(heightM : f32, atmosphereM : f32) -> f32 {
  return clamp(heightM, 1.0, atmosphereM - 1.0);
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

// The frame of the camera's sky: z the datum's normal, x the view's sky axis (the first source's
// azimuth, formed in f64), y = z x x. The sky-view table is indexed by local azimuth about x over
// the whole circle (Design note 7, R08.T7), since its sources stand at their own azimuths.
struct SkyFrame {
  x : vec3f,
  y : vec3f,
  z : vec3f,
}

fn skyFrame(v : AtmosphereView) -> SkyFrame {
  let z = v.up.xyz;
  let x = v.skyAxis.xyz;
  return SkyFrame(x, cross(z, x), z);
}

// sebh's sky-view zenith parameterisation (RenderSkyCommon.hlsl, SkyViewLutParamsToUv and its
// inverse): v splits at the horizon, each half with a square-root compression towards it. beta is
// the angle from the nadir to the horizon (sebh's GroundToHorizonCos is its cosine), so that the
// horizon's zenith angle is pi - beta: beta = acos(vHorizon / r) = atan2(bottom, vHorizon), each
// column's own on its sphere R_alpha since R08.T6.f (`oblate.wgsl`'s skyViewColumnOf; formed in f64
// as `AtmosphereView.exact.z` before), and the zenith's sine and cosine are taken by
// `preciseSinCos`. The azimuth is local since R08.T7 (Design note 7), in place of sebh's
// sun-relative u = sin(phi / 2) over [0, pi]: phi = 2 pi u from the sky axis over the whole circle,
// its sine and cosine by `preciseSinCos` on [0, pi] with the circle's other half by symmetry, so
// that the first and last columns both stand at the axis. The texel's (u, v) is Bruneton's mapping
// of its centre (`unitFromTexelUv`), which is sebh's write. Its ray's direction in the sky frame:
// x the zenith's cosine, y its sine, z the azimuth's cosine, w its sine.
fn skyViewUvToParams(uv : vec2f, size : vec2f, beta : f32) -> vec4f {
  let unit = clamp(unitFromTexelUv(uv, size), vec2f(0.0), vec2f(1.0));
  let zenithHorizon = PI_VIEW - beta;
  let azimuthCosSin = skyViewAzimuthOf(uv, size);
  var zenith : f32;
  if (unit.y < 0.5) {
    var c = 1.0 - 2.0 * unit.y;
    c = 1.0 - c * c;
    zenith = zenithHorizon * c;
  } else {
    let c = unit.y * 2.0 - 1.0;
    zenith = zenithHorizon + beta * c * c;
  }
  let zenithSinCos = preciseSinCos(zenith);
  return vec4f(zenithSinCos.y, zenithSinCos.x, azimuthCosSin);
}

// A texel's local azimuth's cosine and sine, `skyViewUvToParams`'s z and w: phi = 2 pi u from the
// sky axis, by `preciseSinCos` on [0, pi] and the circle's other half by symmetry. The sky view's
// column sphere is taken at it (R08.T6.f).
fn skyViewAzimuthOf(uv : vec2f, size : vec2f) -> vec2f {
  let unit = clamp(unitFromTexelUv(uv, size), vec2f(0.0), vec2f(1.0));
  let phi = 2.0 * PI_VIEW * unit.x;
  var azimuthSinCos : vec2f;
  if (phi <= PI_VIEW) {
    azimuthSinCos = preciseSinCos(phi);
  } else {
    let mirrored = preciseSinCos(2.0 * PI_VIEW - phi);
    azimuthSinCos = vec2f(-mirrored.x, mirrored.y);
  }
  return vec2f(azimuthSinCos.y, azimuthSinCos.x);
}

// A direction's local azimuth about the sky frame as the table's unit range, phi / (2 pi) in
// [0, 1]: `preciseAtan2(|y|, x)` on [0, pi], and 2 pi less it where y < 0, the components the
// direction's own in the frame. A direction at the zenith or the nadir takes 0.
fn skyViewAzimuthUnit(x : f32, y : f32) -> f32 {
  if (x == 0.0 && y == 0.0) {
    return 0.0;
  }
  let a = preciseAtan2(abs(y), x);
  return select(a, 2.0 * PI_VIEW - a, y < 0.0) / (2.0 * PI_VIEW);
}

// The read: a ray's zenith angle, `preciseAtan2(|d x up|, d . up)`, and its local azimuth's unit
// range (`skyViewAzimuthUnit`), to the table's coordinates through Bruneton's mapping
// (`texelUvFromUnit`), so that a texel read at its own ray returns itself.
fn skyViewParamsToUv(
  intersectsGroundView : bool,
  viewZenith : f32,
  azimuthUnit : f32,
  size : vec2f,
  beta : f32,
) -> vec2f {
  let zenithHorizon = PI_VIEW - beta;
  var v : f32;
  if (!intersectsGroundView) {
    let c = 1.0 - sqrt(max(1.0 - viewZenith / zenithHorizon, 0.0));
    v = c * 0.5;
  } else {
    let c = sqrt(max((viewZenith - zenithHorizon) / max(beta, 1e-6), 0.0));
    v = c * 0.5 + 0.5;
  }
  let unit = clamp(vec2f(azimuthUnit, v), vec2f(0.0), vec2f(1.0));
  return texelUvFromUnit(unit, size);
}

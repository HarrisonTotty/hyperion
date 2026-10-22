// The sky-view table (plan R05, Design note 16; Hillaire 2020, section 5.3): the sky's luminance
// around the camera, summed over the view's drawn sources and weighted by each one's illuminance
// times the frame's scale (plan R08, R08.T7), by view zenith and local azimuth, rebuilt every
// frame. Follows `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl`.
//
// Ported from Bevy 0.19.1's sky_view_lut.wgsl (MIT; the notice is in common.wgsl), with sebh's
// SkyViewLutCS zenith parameterisation (MIT; the notice is in multiScattering.wgsl). sebh's table is
// symmetric about the sun's vertical plane, its azimuth over [0, pi] from the sun's; since R08.T7
// its azimuth is local, over the whole circle from the first source's azimuth, so that every
// source's light shares the one table (Design note 7, `view.wgsl`'s `skyViewUvToParams`). The
// camera's own, of its Gaussian radius sqrt(MN) (Design note 16): `medium`'s ground is at that
// radius and the camera at its height above it. No ground bounce, as sebh's. Changes (R05.T12.e):
// the steps are placed toward each ray's lowest point, or, for the camera side of a two-sided ray,
// toward the camera (`marchSplit` in common.wgsl), where Bevy spaces them evenly, and each is
// sampled at its midpoint; their count is the setting's (75 on high, addendum B, where Hillaire
// 2020's Table 2 has 30); each term's density is evaluated once a sample; a step's in-scattering
// is taken without f32's cancellation (`stepFactor` in common.wgsl). Changes (plan R08, R08.T6.b):
// the ray's length and each sample's height are formed from the camera's height without f32's
// cancellation (`maxDistanceFromHeight`, `heightAlong` in medium.wgsl), where |p| - bottom and the
// distance to the ground lose the 0.5 m of an f32 radius near 6.4e6 m: from a camera 2 m up they
// moved the table by up to 17% against the CPU twin, and 5.4% after. The rest, near the zenith
// and at the limb, was the builtin trigonometry's, which `skyViewUvToParams` now avoids
// (view.wgsl). Changes (plan R08, R08.T6.g, Design notes 18 and 19): the horizon's angle comes
// formed in f64 (`view.exact.z`), the ray's direction from precise sines and cosines, the sun's
// shadow and the tables' coordinates from heights (`sourceAt`), and the texels stand at
// Bruneton's mapping, which is sebh's write. Changes (plan R08, R08.T6.c, Design notes 5 and 8):
// the view's own path takes its absorbers by the path's column, under the sun's curves of growth
// (`viewAbsorberStep` in source.wgsl), and the sun's transmittance is the table's optical depth
// with them. Changes (plan R08, R08.T6.i; `decision-r08-t6d-findings.md` section 1): where the
// medium has layer edges, each side of the ray's split is cut into pieces where the ray crosses
// them (`MarchWalk` in common.wgsl), at the same count; a medium with none takes the steps as
// before. Changes (plan R08, R08.T7, Design note 7): one march sums every drawn source, each
// light slot's share carried on its own path (`ViewMarch` in source.wgsl). Changes (plan R08,
// R08.T6.f, Design note 17, _Per-view marches_): each column's rays are marched on the osculating
// sphere R_α of the column's azimuth at the camera, its atmosphere the medium's top over the
// camera's s, its horizon split β its own (`oblate.wgsl`'s skyViewColumnOf), and its layer edges at
// e over the camera's s; each sample's medium at its h* = s h, s at the latitude of the sample's
// own place on the datum, and its sources through the sliced lookup and the bands
// (`source.wgsl`), as `tablesCpu.ts`'s oblateSkyViewTexelsTwin marches them. Changes (plan R08,
// R08.T8.d; `decision-r08-t8-leans.md` section 8): no step holds an array or takes one by value
// (source.wgsl's header): `SkyRay` holds neither the sources nor their angles, each step taking
// them from the view into the sky frame (`skySourcesAt`); the arithmetic unchanged.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d_array<f32>;
@group(0) @binding(6) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(7) var skyViewOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(8) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(9) var absorberCurves : texture_2d<f32>;
@group(0) @binding(10) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(11) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(12) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(0) @binding(13) var lobeTables : texture_2d_array<f32>;
@group(0) @binding(14) var oblateBody : texture_2d<f32>;

// A texel's ray: its column's sphere R_α, the camera's height on it and its point there, the ray's
// direction in the sky frame and body-fixed, with S of the latter, the camera's place on the sphere,
// body-fixed, with its S and its ground quadric, and the ray's zenith cosine. The sources'
// directions and their scattering angles are formed at each step from the view (`skySourcesAt`,
// R08.T8.d), never held here.
struct SkyRay {
  bottomM : f32,
  heightM : f32,
  zenithCos : f32,
  origin : vec3f,
  dir : vec3f,
  bodyDir : vec3f,
  scaledBodyDir : vec3f,
  cameraPlace : vec3f,
  cameraScaled : vec3f,
  cameraQuadric : f32,
}

// One step of the march, at its midpoint: its height on the column's sphere and the normal there,
// its body-fixed place, the camera's place on the sphere plus t along the ray, whose datum point
// gives s; the medium at h* = s h, its sources and their in-scattering.
fn skyViewStep(
  march : ptr<function, ViewMarch>,
  ray : SkyRay,
  body : OblateBody,
  frame : SkyFrame,
  stepAt : MarchStep,
  lobes : bool,
) {
  let t = stepAt.tM;
  let p = ray.origin + t * ray.dir;
  let rP = length(p);
  let sampleHeightM = shellHeightAlong(ray.bottomM, ray.heightM, ray.zenithCos, t, rP);
  let up = p / rP;
  let normal = up.x * frame.x + up.y * frame.y + up.z * frame.z;
  let place = ray.cameraPlace + t * ray.bodyDir;
  let quadric = quadricAlong(ray.cameraQuadric, ray.cameraScaled, ray.scaledBodyDir, t);
  // The place's latitude, where the body's lookups or its s take one.
  var latitudeNormal = normal;
  var s = body.gravity.w;
  if (oblateNeedsLatitude(body)) {
    latitudeNormal = datumPointOf(body, place, quadric).normal;
    s = gravityRatioAt(body, latitudeNormal.z * latitudeNormal.z);
  }
  let local = sampleMediumAt(s * sampleHeightM);
  let start = sampleStartOf(body, place, quadric, sampleHeightM, normal, latitudeNormal, s, local);
  let shares = skySourcesAt(body, start, up, ray.dir, frame, lobes, local);
  viewMarchStep(march, local, shares, stepAt.dtM);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(skyViewOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let sizeF = vec2f(size);
  let body = oblateBodyOf(oblateBody);
  let frame = skyFrame(view);
  // The column's azimuth, as the zenith's parameterisation takes it, and its sphere: R_α at the
  // camera along it, the atmosphere the medium's top over the camera's s (`view.curvature`).
  let uv = (vec2f(id.xy) + 0.5) / sizeF;
  let azimuth = skyViewAzimuthOf(uv, sizeF);
  let horizontal = azimuth.x * frame.x + azimuth.y * frame.y;
  let atmosphereM = view.curvature.w;
  // The sky-view table serves cameras inside the atmosphere; above it the composite ray-marches.
  let heightM = skyViewHeight(view.camera.w, atmosphereM);
  let column = skyViewColumnOf(
    view.curvature.x, view.curvature.y, frame.z, frame.x, horizontal, heightM
  );
  let bottom = column.x;
  let r = bottom + heightM;
  let params = skyViewUvToParams(uv, sizeF, column.y);
  let viewZenithCos = params.x;
  let viewZenithSin = params.y;
  let dir = vec3f(viewZenithSin * params.z, viewZenithSin * params.w, viewZenithCos);
  let bodyDir = dir.x * frame.x + dir.y * frame.y + dir.z * frame.z;

  let origin = vec3f(0.0, 0.0, r);
  let tMax = shellMaxDistance(bottom, atmosphereM, heightM, viewZenithCos);
  let samples = max(u32(view.output.w), 2u);
  // The ray's closest approach to the sphere's centre is at -o.d = -r cos(view zenith).
  // The camera is inside the atmosphere: the sky view serves no other.
  let split = marchSplit(0.0, tMax, -r * viewZenithCos, samples, true);
  // The camera's place where the sphere puts it: moved along its normal from its geodetic height
  // to its height on the sphere (R08.T6.f), the two the same but where the shell holds it (1 m off
  // the ground and the top); its quadric from q_g by the same move, which keeps its digits.
  let lift = heightM - view.camera.w;
  let cameraPlace = view.camera.xyz + lift * frame.z;
  let scaledNormal = oblateScaled(body, frame.z);
  let cameraQuadric = view.exact.x
    + lift * (2.0 * dot(oblateScaled(body, view.camera.xyz), scaledNormal)
      + lift * dot(scaledNormal, scaledNormal));
  let ray = SkyRay(
    bottom, heightM, viewZenithCos, origin, dir, bodyDir, oblateScaled(body, bodyDir), cameraPlace,
    oblateScaled(body, cameraPlace), cameraQuadric
  );
  var march = viewMarchStart();
  let lobes = hasForwardLobes();
  // A medium with no layer edge takes R05.T12.e's steps as they were, and declares no walk; the
  // edges are the same for every texel, so the branch is uniform (R08.T6.i). They stand at e over
  // the camera's s on the column's sphere (R08.T6.f).
  let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);
  if (edgeCount == 0u) {
    for (var i = 0u; i < samples; i++) {
      skyViewStep(&march, ray, body, frame, marchStep(split, i), lobes);
    }
  } else {
    var walk : MarchWalk;
    marchWalkStart(&walk, split);
    let inverseS = 1.0 / view.curvature.z;
    for (var e = 0u; e < edgeCount; e++) {
      let edgeM = layerEdges.middles[e / 4u][e % 4u] * inverseS;
      marchWalkCrossSphere(&walk, bottom, heightM, viewZenithCos, edgeM);
    }
    marchWalkReady(&walk);
    for (var i = 0u; i < samples; i++) {
      skyViewStep(&march, ray, body, frame, marchWalkStep(&walk, i), lobes);
    }
  }
  textureStore(skyViewOut, id.xy, vec4f(march.luminance, 1.0));
}

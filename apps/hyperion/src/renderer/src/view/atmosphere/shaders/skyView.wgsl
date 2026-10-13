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
// light slot's share carried on its own path (`ViewMarch` in source.wgsl).

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d<f32>;
@group(0) @binding(6) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(7) var skyViewOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(8) var absorberColumns : texture_2d<f32>;
@group(0) @binding(9) var absorberCurves : texture_2d<f32>;
@group(0) @binding(10) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(11) var absorberPressureColumns : texture_2d<f32>;
@group(0) @binding(12) var lastAbsorberPressureColumn : texture_2d<f32>;
@group(0) @binding(13) var lobeTables : texture_2d_array<f32>;

// A texel's ray: the camera's height on the sky view's sphere and its point there, the ray's
// direction and its zenith cosine, and the sources' directions in the sky frame with their
// scattering angles.
struct SkyRay {
  heightM : f32,
  zenithCos : f32,
  origin : vec3f,
  dir : vec3f,
  sources : array<vec3f, MAX_SKY_SOURCES>,
  angles : array<ScatteringAngle, MAX_SKY_SOURCES>,
}

// One step of the march, at its midpoint: the medium there, its sources and their in-scattering.
fn skyViewStep(march : ptr<function, ViewMarch>, ray : SkyRay, stepAt : MarchStep) {
  let p = ray.origin + stepAt.tM * ray.dir;
  let rP = length(p);
  let sampleHeightM = heightAlong(ray.heightM, ray.zenithCos, stepAt.tM, rP);
  let local = sampleMediumAt(sampleHeightM);
  let shares = viewSourcesAt(sampleHeightM, p / rP, ray.sources, ray.angles, local);
  viewMarchStep(march, local, shares, stepAt.dtM);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(skyViewOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let sizeF = vec2f(size);
  // The sky-view table serves cameras inside the atmosphere; above it the composite ray-marches.
  let heightM = skyViewHeight(view.camera.w, medium.topHeightM);
  let r = medium.bottomRadiusM + heightM;
  let params = skyViewUvToParams((vec2f(id.xy) + 0.5) / sizeF, sizeF, view.exact.z);
  let viewZenithCos = params.x;
  let viewZenithSin = params.y;
  let dir = vec3f(viewZenithSin * params.z, viewZenithSin * params.w, viewZenithCos);
  // Each source in the sky frame: its components along the sky axis, up x the axis and the normal.
  let frame = skyFrame(view);
  let count = skySourceCount(view);
  var sources : array<vec3f, MAX_SKY_SOURCES>;
  for (var k = 0u; k < count; k++) {
    let s = view.sources[k].direction.xyz;
    sources[k] = vec3f(dot(s, frame.x), dot(s, frame.y), dot(s, frame.z));
  }
  let angles = sourceAngles(dir, sources, count);

  let origin = vec3f(0.0, 0.0, r);
  let tMax = maxDistanceFromHeight(heightM, viewZenithCos);
  let samples = max(u32(view.output.w), 2u);
  // The ray's closest approach to the sphere's centre is at -o.d = -r cos(view zenith).
  // The camera is inside the atmosphere: the sky view serves no other.
  let split = marchSplit(0.0, tMax, -r * viewZenithCos, samples, true);
  let ray = SkyRay(heightM, viewZenithCos, origin, dir, sources, angles);
  var march = viewMarchStart();
  // A medium with no layer edge takes R05.T12.e's steps as they were, and declares no walk; the
  // edges are the same for every texel, so the branch is uniform (R08.T6.i).
  let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);
  if (edgeCount == 0u) {
    for (var i = 0u; i < samples; i++) {
      skyViewStep(&march, ray, marchStep(split, i));
    }
  } else {
    var walk : MarchWalk;
    marchWalkStart(&walk, split);
    for (var e = 0u; e < edgeCount; e++) {
      let edgeM = layerEdges.middles[e / 4u][e % 4u];
      marchWalkCrossSphere(&walk, medium.bottomRadiusM, heightM, viewZenithCos, edgeM);
    }
    marchWalkReady(&walk);
    for (var i = 0u; i < samples; i++) {
      skyViewStep(&march, ray, marchWalkStep(&walk, i));
    }
  }
  textureStore(skyViewOut, id.xy, vec4f(march.luminance, 1.0));
}

// The sky-view table (plan R05, Design note 16; Hillaire 2020, section 5.2): the sky's luminance
// per unit sun illuminance around the camera, by view zenith and azimuth from the sun, rebuilt
// every frame. Follows `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl`.
//
// Ported from Bevy 0.19.1's sky_view_lut.wgsl (MIT; the notice is in common.wgsl), with sebh's
// SkyViewLutCS parameterisation (MIT; the notice is in multiScattering.wgsl): the table is
// symmetric about the sun's vertical plane, so its azimuth spans [0, pi]. The sphere is the
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
// before.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d<f32>;
@group(0) @binding(6) var multiScattering : texture_2d<f32>;
@group(0) @binding(7) var skyViewOut : texture_storage_2d<rgba16float, write>;
@group(0) @binding(8) var absorberColumns : texture_2d<f32>;
@group(0) @binding(9) var absorberCurves : texture_2d<f32>;
@group(0) @binding(10) var<uniform> layerEdges : LayerEdges;
@group(0) @binding(11) var absorberPressureColumns : texture_2d<f32>;
@group(0) @binding(12) var lastAbsorberPressureColumn : texture_2d<f32>;

// A texel's ray: the camera's height on the sky view's sphere and its point there, the ray's
// direction and its zenith cosine, the sun in the sky frame, and the scattering angle.
struct SkyRay {
  heightM : f32,
  zenithCos : f32,
  origin : vec3f,
  dir : vec3f,
  sun : vec3f,
  angle : ScatteringAngle,
}

// A march along a texel's ray so far: the light it has gathered, its throughput and its own
// absorber path.
struct SkyMarch {
  luminance : vec3f,
  throughput : vec3f,
  path : AbsorberPath,
}

// One step of the march, at its midpoint: the medium there, its source and its in-scattering.
fn skyViewStep(march : ptr<function, SkyMarch>, ray : SkyRay, stepAt : MarchStep) {
  let dt = stepAt.dtM;
  let p = ray.origin + stepAt.tM * ray.dir;
  let rP = length(p);
  let sampleHeightM = heightAlong(ray.heightM, ray.zenithCos, stepAt.tM, rP);
  let muSun = dot(ray.sun, p / rP);
  let local = sampleMediumAt(sampleHeightM, ray.angle);
  let next = viewAbsorberStep((*march).path, local, dt);
  let stepDepth = local.extinction * dt + (next.depth - (*march).path.depth);
  (*march).path = next;
  let stepTransmittance = exp(-stepDepth);
  let source = sourceAt(
    transmittance,
    multiScattering,
    view.tables.x,
    sampleHeightM,
    muSun,
    local,
  );
  (*march).luminance += (*march).throughput * source * dt * stepFactor(stepDepth);
  (*march).throughput *= stepTransmittance;
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
  // The sun in the sky frame: its zenith's cosine and sine, the latter |s x up| (Design note 18).
  let muSunCamera = dot(view.sun.xyz, view.up.xyz);
  let sun = vec3f(length(cross(view.sun.xyz, view.up.xyz)), 0.0, muSunCamera);
  let angle = scatteringAngle(dir, sun);

  let origin = vec3f(0.0, 0.0, r);
  let tMax = maxDistanceFromHeight(heightM, viewZenithCos);
  let samples = max(u32(view.output.w), 2u);
  // The ray's closest approach to the sphere's centre is at -o.d = -r cos(view zenith).
  // The camera is inside the atmosphere: the sky view serves no other.
  let split = marchSplit(0.0, tMax, -r * viewZenithCos, samples, true);
  let ray = SkyRay(heightM, viewZenithCos, origin, dir, sun, angle);
  var march = SkyMarch(vec3f(0.0), vec3f(1.0), absorberPathStart());
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

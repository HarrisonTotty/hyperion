// The aerial-perspective volume (plan R05, Design note 16; Hillaire 2020, section 5.4): for each
// froxel of the camera's frustum, the light scattered into the ray between the camera and the
// froxel's far face, summed over the view's drawn sources and weighted by each one's illuminance
// times the frame's scale (plan R08, R08.T7) (rgb), and the ray's mean transmittance (a), in
// `view.tables.z` slices of equal depth out to `view.tables.y` (32 km, Hillaire 2020's reach),
// rebuilt every frame. Follows `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl`.
//
// Ported from Bevy 0.19.1's aerial_view_lut.wgsl (MIT; the notice is in common.wgsl), marching once
// through the slices and storing each slice's running integral. Changes: the integral is stored
// linear, with sebh's mean transmittance in alpha, rather than as Bevy's logarithm; each step is
// sampled at its midpoint; the sphere is the camera's own (Design note 16); each term's density is
// evaluated once a sample, and a step's in-scattering is taken without f32's cancellation
// (`stepFactor` in common.wgsl; R05.T12.e). The slices keep their equal depths, so the steps are
// even. Changes (plan R08, R08.T6.g): each sample's height comes from the camera's height and the
// ray's zenith cosine by the sphere's identity (`heightAlong`), never as its radius less the
// ground's, the origin carrying directions only (Design note 18); froxel (i, j) marches the ray
// through the screen at (i, j) / (size - 1), Bruneton's texel mapping (Design note 19), so that the
// composite's read of the screen's edges falls on the outer froxels' centres. The slice axis is
// already the pair's: slice k holds the march at (k + 1) reach / slices, which the composite reads
// at distance / reach - 1/2 / slices. Changes (plan R08, R08.T6.c): the view's own path takes its
// absorbers by the path's column, under the sun's curves of growth (`viewAbsorberStep`), so that
// the mean transmittance in alpha carries them. Changes (plan R08, R08.T7, Design note 7): one
// march sums every drawn source, each light slot's share carried on its own path (`ViewMarch` in
// source.wgsl), and alpha is slot 0's mean transmittance. Changes (plan R08, R08.T6.f, Design note
// 17, _Per-view marches_): each froxel's ray is marched on its own osculating sphere R_α at the
// camera, each sample's medium at its h* = s h, s at the latitude of the sample's own place on the
// datum, and its sources through the sliced lookup and the bands (`source.wgsl`), as
// `tablesCpu.ts`'s oblateAerialPerspectiveTwin marches them. Changes (plan R08, R08.T8.d;
// `decision-r08-t8-leans.md` section 8): no step holds an array or takes one by value
// (source.wgsl's header): the sources and their angles are formed at each step from the view
// (`viewSourcesAt`), not held a froxel; the arithmetic unchanged.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<storage, read> terms : array<Term, MAX_TERMS>;
@group(0) @binding(2) var densityTables : texture_2d_array<f32>;
@group(0) @binding(3) var phaseTables : texture_2d_array<f32>;
@group(0) @binding(4) var<uniform> view : AtmosphereView;
@group(0) @binding(5) var transmittance : texture_2d_array<f32>;
@group(0) @binding(6) var multiScattering : texture_2d_array<f32>;
@group(0) @binding(7) var aerialOut : texture_storage_3d<rgba16float, write>;
@group(0) @binding(8) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(9) var absorberCurves : texture_2d<f32>;
@group(0) @binding(10) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(11) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(0) @binding(12) var lobeTables : texture_2d_array<f32>;
@group(0) @binding(13) var oblateBody : texture_2d<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(aerialOut);
  if (id.x >= size.x || id.y >= size.y) {
    return;
  }
  let body = oblateBodyOf(oblateBody);
  let sizeXy = vec2f(size.xy);
  let dir = rayThrough(view, unitFromTexelUv((vec2f(id.xy) + 0.5) / sizeXy, sizeXy));
  let n = view.up.xyz;
  // The froxel's own sphere: R_α at the camera along its ray (R08.T6.f), centred so that its datum
  // normal is the sphere's; the origin carries the samples' directions, and their heights come
  // from the camera's.
  let bottom = cameraCurvatureAlong(view.curvature.x, view.curvature.y, n, dir);
  let cameraHeightM = max(view.camera.w, 0.0);
  let origin = n * (bottom + cameraHeightM);
  let mu = dot(dir, n);
  let cameraScaled = oblateScaled(body, view.camera.xyz);
  let scaledDir = oblateScaled(body, dir);
  let slices = size.z;
  let perSlice = max(u32(view.tables.w), 1u);
  let dt = view.tables.y / f32(slices * perSlice);
  var march = viewMarchStart();
  let lobes = hasForwardLobes();
  for (var slice = 0u; slice < slices; slice++) {
    for (var step = 0u; step < perSlice; step++) {
      let t = (f32(slice * perSlice + step) + 0.5) * dt;
      let p = origin + t * dir;
      let rP = length(p);
      let heightM = shellHeightAlong(bottom, cameraHeightM, mu, t, rP);
      let up = p / rP;
      // The sample's body-fixed place, the camera's plus t along the ray, and its datum point.
      let place = view.camera.xyz + t * dir;
      let quadric = quadricAlong(view.exact.x, cameraScaled, scaledDir, t);
      // The place's latitude, where the body's lookups or its s take one.
      var latitudeNormal = up;
      var s = body.gravity.w;
      if (oblateNeedsLatitude(body)) {
        latitudeNormal = datumPointOf(body, place, quadric).normal;
        s = gravityRatioAt(body, latitudeNormal.z * latitudeNormal.z);
      }
      let local = sampleMediumAt(s * heightM);
      let start = sampleStartOf(body, place, quadric, heightM, up, latitudeNormal, s, local);
      let shares = viewSourcesAt(body, start, up, dir, lobes, local);
      viewMarchStep(&march, local, shares, dt);
    }
    textureStore(aerialOut, vec3u(id.xy, slice), vec4f(march.luminance, viewMeanTransmittance(&march)));
  }
}

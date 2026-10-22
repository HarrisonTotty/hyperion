// The atmosphere's scattering source (plan R05, R05.T12.c, Design note 16): the single- and
// multiple-scattering source at a sample, read from the per-planet tables; the steps of the
// per-frame marches (R05.T12.e) are `common.wgsl`'s since plan R08's R08.T6.d, and the medium at a
// sample and each term's phase towards a ray `scattering.wgsl`'s since R08.T9.b, which the
// sky-irradiance kernel takes too. Prepended, after `common.wgsl`, `medium.wgsl`, `view.wgsl`,
// `oblate.wgsl` and `scattering.wgsl`, to the sky-view, aerial-perspective and ray-march kernels,
// each of which also declares the phase tables as
// `var phaseTables : texture_2d_array<f32>;`, the transmittance table, the absorber-column table
// and the lights' curves of growth as `var transmittance : texture_2d_array<f32>;`,
// `var absorberColumns : texture_2d_array<f32>;` and `var absorberCurves : texture_2d<f32>;`
// (R08.T6.c), the pressure-weighted columns as
// `var absorberPressureColumns : texture_2d_array<f32>;` and
// `var lastAbsorberPressureColumn : texture_2d_array<f32>;` (R08.T4.d), each a 2D array of one
// layer a κ slice (R08.T6.f), the multiple-scattering table as
// `var multiScattering : texture_2d_array<f32>;`, one layer a band and light slot (R08.T6.f), the
// forward-lobe tables as `var lobeTables : texture_2d_array<f32>;` (R08.T7) and the body's table as
// `var oblateBody : texture_2d<f32>;` (R08.T6.f), in group 0; like `medium.wgsl`, it reads the
// kernel's `medium`, `terms` and tables in place, so the composite does not take it.
//
// The source is ported from Bevy 0.19.1's functions.wgsl (`sample_local_inscattering`, MIT; the
// notice is in common.wgsl) and checked against sebh's IntegrateScatteredLuminance (MIT; the
// notice is in multiScattering.wgsl). Changes: the phase functions are the medium's own terms',
// per channel, a tabulated one among them (R08.T6.b); each term's density is evaluated once a
// sample (R05.T12.e); the tables are read on their own sphere, at the sample's height above the
// datum (Design note 16), from heights alone and through Bruneton's texel mapping (plan R08,
// Design notes 18 and 19, R08.T6.g); the transmittance is read as optical depth with the sun's
// curves of growth added, and each march's own path takes its absorbers by the path's column
// (plan R08, Design notes 5 and 8, R08.T6.c). Added here (R05.T12.e): the steps' placement
// (`marchSplit`, `marchStep`), which R08.T6.d moved to `common.wgsl` for the multiple-scattering
// kernel. Changes (plan R08, R08.T7, Design note 7): the source is summed over the view's drawn
// sources in one march, each weighted by its illuminance times the frame's scale, each with its
// own phase, ground shadow, transmittance and multiple-scattering layer; and each light slot's
// share of the in-scattered light is carried along the view's own path under that slot's absorber
// curves, a path a slot (`ViewMarch`), slot 0's throughput the views' mean transmittance. Changes
// (plan R08, R08.T6.f, Design note 17): the medium is read at the sample's gravity-scaled height
// h* = s h, s at the sample's own latitude; each source's transmittance through the sliced lookup
// from the sample along the source (`oblate.wgsl`), its stored optical depth over s; the multiple
// scattering from the latitude bands about the sample's s; the ground's shadow the lookup's.
// Changes (plan R08, R08.T8.d; `decision-r08-t8-leans.md` section 8): no step of the three
// per-frame marches holds an array or takes one by value. Tint keeps an array value indexed by a
// loop variable in a function-scope variable of its own, stored whole at each call (the SPIR-V's
// OpVariable and whole-array OpStore at the callee's entry), and a function-scope array indexed by
// a variable lives in the invocation's local memory: the sources' directions and angles passed by
// value to each step made the ray march's step about 2.6 times its cost on the RTX 3080, and the
// slots' and terms' arrays the rest of its excess, as R05.T14's `Medium` by value had. So each
// light slot's share and march, and each term's density, is a member of its own, selected by a
// switch (`ViewShares`, `ViewMarch`, and scattering.wgsl's `TermDensities`); each source's
// direction is read from the view at each step and its scattering angle formed there, as the
// kernels formed both once a pixel before; the arithmetic is unchanged, and the read-backs with it,
// to the bit.

// A march's absorber path one step `dtM` on, through the sample `local` (`absorberStep`): the
// view's own path under light slot `slot`'s curves, since what it carries is that light's
// (R08.T6.c; a path a slot since R08.T7), its lines at the path's pressure (R08.T4.d).
fn viewAbsorberStep(path : AbsorberPath, local : SampleMedium, dtM : f32, slot : u32) -> AbsorberPath {
  return absorberStep(
    absorberCurves, absorberCount(), slot, lineSlots(), path, local.columns, local.pressureColumns,
    dtM
  );
}

// One source's light at a sample through the sliced lookup (plan R08, Design note 17; R08.T6.f):
// whether the ground leaves it lit, its transmittance to the sample under light slot `slot`'s
// curves of growth, exponentiated in f32 (R08.T6.c), and the stored optical depth along it, the
// forward lobes' read (R08.T7), each divided by s.
struct SourceLight {
  lit : f32,
  toSun : vec3f,
  depth : vec3f,
}

// The light of the source along the unit body-fixed `d` at the lookup's start (`oblate.wgsl`'s
// `slicedRayOf` and `slicedStoredOf` through the kernel's slices; `tablesCpu.ts`'s
// OblateReader.sunInto). Each kernel that takes this declares `oblateBody`, `transmittance`,
// `absorberColumns`, `absorberPressureColumns`, `lastAbsorberPressureColumn` (2D arrays, one layer
// a slice) and `absorberCurves`.
fn sourceLightAt(body : OblateBody, start : LookupStart, d : vec3f, slot : u32) -> SourceLight {
  let ray = slicedRayOf(body, oblateBody, start, d);
  let count = absorberCount();
  let stored = slicedStoredOf(
    ray, body, oblateBody, transmittance, absorberColumns, absorberPressureColumns,
    lastAbsorberPressureColumn, count
  );
  let absorbed = absorberDepth(
    absorberCurves, count, slot, lineSlots(), stored.columns, stored.pressureColumns
  );
  return SourceLight(select(1.0, 0.0, ray.shadowed), exp(-(stored.depth + absorbed)), stored.depth);
}

// The lookup's start at a sample of a per-frame march (`oblate.wgsl`'s LookupStart): the sample's
// body-fixed place and its ground quadric, the march's own height and unit normal there
// (body-fixed), the unit geodetic normal of the place's datum point, whose z is the sine of its
// latitude, s there (`gravityRatio`), h* = s h, and, on a body of several bands, the local scale
// height at h* from the medium there, `local`.
fn sampleStartOf(
  body : OblateBody,
  place : vec3f,
  quadricM2 : f32,
  heightM : f32,
  normal : vec3f,
  latitudeNormal : vec3f,
  gravityRatio : f32,
  local : SampleMedium,
) -> LookupStart {
  let z = gravityRatio * heightM;
  // The local scale height places the fit's second point, which only a body of several bands
  // takes (`slicedRayOf`): one band reads the start's own sphere, and one slice and band the slice.
  var scaleM = 0.0;
  if (oblateBandCount(body) > 1u) {
    scaleM = localScaleHeightOf(body, oblateBody, transmittance, z, local.extinction.g);
  }
  return LookupStart(
    place, oblateScaled(body, place), quadricM2, heightM, normal, latitudeNormal.z,
    length(latitudeNormal.xy), gravityRatio, z, scaleM
  );
}

// One source's scattering source at a sample per unit of its illuminance, per unit length: single
// scattering of its light (`light`), shadowed by the ground, plus multiple scattering from the
// bands about the sample's s at its h* (Hillaire 2020, equations 3 and 11; R08.T6.f). `local` is the
// medium at the sample's h* (`sampleMediumAt`), `phased` its scattering towards the eye for the
// source (`phasedScatteringOf`).
fn sourceAt(
  body : OblateBody,
  start : LookupStart,
  light : SourceLight,
  muSun : f32,
  slot : u32,
  local : SampleMedium,
  phased : vec3f,
) -> vec3f {
  let single = light.lit * light.toSun * phased;
  let multiple = bandMultiScatteringAt(
    body, oblateBody, multiScattering, start.scaledHeightM, start.gravityRatio, muSun, slot
  );
  return single + multiple * local.scattering;
}

// The view's sources at a sample summed by light slot, a member a slot (MAX_SKY_LIGHT_SLOTS of
// them), so that no step indexes an array (R08.T8.d).
struct ViewShares {
  slot0 : vec3f,
  slot1 : vec3f,
  slot2 : vec3f,
  slot3 : vec3f,
}

// `shares` with a source's light `one` times its weight `weight` added into slot `slot`'s, at most
// MAX_SKY_LIGHT_SLOTS - 1: the multiply and its add in one statement, as `shares[slot] += weight *
// one` had them before R08.T8.d, so that a compiler that fuses the two fuses them alike.
fn viewShareAdd(shares : ViewShares, slot : u32, weight : vec3f, one : vec3f) -> ViewShares {
  var out = shares;
  switch (slot) {
    case 0u: {
      out.slot0 += weight * one;
    }
    case 1u: {
      out.slot1 += weight * one;
    }
    case 2u: {
      out.slot2 += weight * one;
    }
    default: {
      out.slot3 += weight * one;
    }
  }
  return out;
}

// Slot `slot`'s share, at most MAX_SKY_LIGHT_SLOTS - 1.
fn viewShareOf(shares : ViewShares, slot : u32) -> vec3f {
  switch (slot) {
    case 0u: {
      return shares.slot0;
    }
    case 1u: {
      return shares.slot1;
    }
    case 2u: {
      return shares.slot2;
    }
    default: {
      return shares.slot3;
    }
  }
}

// The kernel's drawn sources and the light slots its marches track a path for, read from its own
// `view` in place, never through the whole view by value (`view.wgsl`'s skySourceCount and
// skyPathCount, R08.T8.d).
fn viewSourceCount() -> u32 {
  return min(u32(view.sky.x), MAX_SKY_SOURCES);
}

fn viewPathCount() -> u32 {
  return clamp(u32(view.sky.y), 1u, MAX_SKY_LIGHT_SLOTS);
}

// One source's light at a sample towards the eye per unit of its weight, its `sourceAt` for the
// ray along `ray`: `dir` the source's direction and `up` the unit normal the sample's zenith cosine
// is taken against, all three in the march's frame, `bodyDir` its direction body-fixed, which the
// lookups take from the sample's start (R08.T6.f), and `slot` its light slot; its scattering angle
// formed here from `ray` and `dir` at each step (R08.T8.d), as the kernels formed it once a pixel
// before.
fn sourceInScatteringAt(
  body : OblateBody,
  start : LookupStart,
  up : vec3f,
  ray : vec3f,
  dir : vec3f,
  bodyDir : vec3f,
  slot : u32,
  lobes : bool,
  local : SampleMedium,
) -> vec3f {
  let muSun = dot(dir, up);
  let light = sourceLightAt(body, start, bodyDir, slot);
  let angle = scatteringAngle(ray, dir);
  var phased = phasedScatteringOf(local, angle);
  if (lobes) {
    // The forward lobes' higher orders, at the stored optical depth along the source (R08.T7).
    phased += lobeOrdersOf(local, angle, light.depth);
  }
  return sourceAt(body, start, light, muSun, slot, local, phased);
}

// The view's sources at a sample, summed by light slot, for a march in the body-fixed frame (the
// ray march's, the aerial perspective's): each source's `sourceInScatteringAt` times its weight
// added into its slot's share, so that each share is carried along the view's path under its own
// curves (`viewMarchStep`).
fn viewSourcesAt(
  body : OblateBody,
  start : LookupStart,
  up : vec3f,
  ray : vec3f,
  local : SampleMedium,
) -> ViewShares {
  var shares = ViewShares(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
  let count = viewSourceCount();
  let lobes = hasForwardLobes();
  for (var k = 0u; k < count; k++) {
    let source = view.sources[k];
    let slot = skySlotOf(source);
    let d = source.direction.xyz;
    let one = sourceInScatteringAt(body, start, up, ray, d, d, slot, lobes, local);
    shares = viewShareAdd(shares, slot, source.weight.rgb, one);
  }
  return shares;
}

// The same for a march in the sky frame `frame` (the sky view's), `up` and `ray` in that frame:
// each source's direction taken into it, its components along the frame's axes.
fn skySourcesAt(
  body : OblateBody,
  start : LookupStart,
  up : vec3f,
  ray : vec3f,
  frame : SkyFrame,
  local : SampleMedium,
) -> ViewShares {
  var shares = ViewShares(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
  let count = viewSourceCount();
  let lobes = hasForwardLobes();
  for (var k = 0u; k < count; k++) {
    let source = view.sources[k];
    let slot = skySlotOf(source);
    let s = source.direction.xyz;
    let d = vec3f(dot(s, frame.x), dot(s, frame.y), dot(s, frame.z));
    let one = sourceInScatteringAt(body, start, up, ray, d, s, slot, lobes, local);
    shares = viewShareAdd(shares, slot, source.weight.rgb, one);
  }
  return shares;
}

// A march along a ray so far: the light it has gathered, and per light slot its throughput and its
// own absorber path (R08.T7), slot 0's throughput the views' mean transmittance; a member a slot
// (MAX_SKY_LIGHT_SLOTS of them), so that no step indexes an array (R08.T8.d).
struct ViewMarch {
  luminance : vec3f,
  throughput0 : vec3f,
  throughput1 : vec3f,
  throughput2 : vec3f,
  throughput3 : vec3f,
  path0 : AbsorberPath,
  path1 : AbsorberPath,
  path2 : AbsorberPath,
  path3 : AbsorberPath,
}

fn viewMarchStart() -> ViewMarch {
  let path = absorberPathStart();
  let one = vec3f(1.0);
  return ViewMarch(vec3f(0.0), one, one, one, one, path, path, path, path);
}

// Slot `slot`'s throughput, at most MAX_SKY_LIGHT_SLOTS - 1.
fn viewThroughputOf(march : ViewMarch, slot : u32) -> vec3f {
  switch (slot) {
    case 0u: {
      return march.throughput0;
    }
    case 1u: {
      return march.throughput1;
    }
    case 2u: {
      return march.throughput2;
    }
    default: {
      return march.throughput3;
    }
  }
}

// Slot `slot`'s absorber path, at most MAX_SKY_LIGHT_SLOTS - 1.
fn viewPathOf(march : ViewMarch, slot : u32) -> AbsorberPath {
  switch (slot) {
    case 0u: {
      return march.path0;
    }
    case 1u: {
      return march.path1;
    }
    case 2u: {
      return march.path2;
    }
    default: {
      return march.path3;
    }
  }
}

// `march` with slot `slot`'s throughput and path set, at most MAX_SKY_LIGHT_SLOTS - 1.
fn withViewSlot(
  march : ViewMarch,
  slot : u32,
  throughput : vec3f,
  path : AbsorberPath,
) -> ViewMarch {
  var out = march;
  switch (slot) {
    case 0u: {
      out.throughput0 = throughput;
      out.path0 = path;
    }
    case 1u: {
      out.throughput1 = throughput;
      out.path1 = path;
    }
    case 2u: {
      out.throughput2 = throughput;
      out.path2 = path;
    }
    default: {
      out.throughput3 = throughput;
      out.path3 = path;
    }
  }
  return out;
}

// One step `dtM` of a march through the sample `local`, with each light slot's share of the
// source there (`viewSourcesAt`, `skySourcesAt`): each slot's path one step on under its curves,
// its share's in-scattering through its own throughput, without f32's cancellation (`stepFactor`),
// and its throughput carried on.
fn viewMarchStep(
  march : ptr<function, ViewMarch>,
  local : SampleMedium,
  shares : ViewShares,
  dtM : f32,
) {
  let paths = viewPathCount();
  var m = *march;
  for (var p = 0u; p < paths; p++) {
    let path = viewPathOf(m, p);
    let throughput = viewThroughputOf(m, p);
    let next = viewAbsorberStep(path, local, dtM, p);
    let stepDepth = local.extinction * dtM + (next.depth - path.depth);
    m.luminance += throughput * viewShareOf(shares, p) * dtM * stepFactor(stepDepth);
    m = withViewSlot(m, p, throughput * exp(-stepDepth), next);
  }
  *march = m;
}

// The views' mean transmittance of a march, slot 0's throughput averaged over the channels.
fn viewMeanTransmittance(march : ViewMarch) -> f32 {
  let t = march.throughput0;
  return (t.r + t.g + t.b) / 3.0;
}

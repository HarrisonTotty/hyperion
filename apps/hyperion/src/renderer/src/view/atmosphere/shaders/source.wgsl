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
// per-frame marches takes or returns an array by value, nor holds one but slots 1 to 3 of the
// march, indexed only where the view tracks them. Tint keeps an array value indexed by a
// loop variable in a function-scope variable of its own, stored whole at each call (the SPIR-V's
// OpVariable and whole-array OpStore at the callee's entry), and a function-scope array indexed by
// a variable lives in the invocation's local memory: the sources' directions and angles passed by
// value to each step made the ray march's step about 2.6 times its cost on the RTX 3080, as
// R05.T14's `Medium` by value had. So each light slot's share and each term's density is a member
// of its own, selected by a switch (`ViewShares`, scattering.wgsl's `TermDensities`); the march's
// slot 0 is a member of its own and the slots only absorbing media lit by several stars step stand
// apart, held by pointer (`ViewMarch`), which keeps the registers of the march that every view
// takes its steps' own; each source's direction is read from the view at each step and its
// scattering angle formed there, as the kernels formed both once a pixel before; and the forward
// lobes' test is the kernel's, taken once a ray (or texel). The arithmetic is unchanged:
// read-backs are byte-identical on SwiftShader, and on the RTX 3080 only its fused multiply-adds,
// which follow the code's shape, move a half-float ulp in a few values (plan R08's Risks,
// "Deviations in T8.d, as built"). Changes (plan R08, R08.T8.e; E2 and E3 of
// `decision-r08-t8-leans.md` section 10): the view's paths share their columns and each
// absorber's read of the curves (`viewMarchStep`, `common.wgsl`'s `curveRowsOf`), each slot loading
// only its own curve's texels, and a curve's pressure row is found by bisection; the arithmetic
// unchanged.

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
// them), selected by a switch, so that no step holds them in an array (R08.T8.d).
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
// `view` in place, never through the whole view by value (as `view.wgsl`'s skySourceCount reads a
// view's count, R08.T8.d).
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
  lobes : bool,
  local : SampleMedium,
) -> ViewShares {
  var shares = ViewShares(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
  let count = viewSourceCount();
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
  lobes : bool,
  local : SampleMedium,
) -> ViewShares {
  var shares = ViewShares(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
  let count = viewSourceCount();
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
// own absorber path (R08.T7), slot 0's throughput the views' mean transmittance. Slot 0, which
// every march steps, is a member of its own; slots 1 to MAX_SKY_LIGHT_SLOTS - 1, which only a
// medium with absorbers lit by several stars' lights steps, stand apart in arrays, which a march
// holds by pointer and its steps index only where the view has those slots (R08.T8.d). Every slot's
// path crosses the same columns, so slot 0's path holds them, and the other slots hold only their
// own depths under their curves (R08.T8.e, E3 of `decision-r08-t8-leans.md` section 10).
struct ViewMarch {
  luminance : vec3f,
  throughput0 : vec3f,
  path0 : AbsorberPath,
  throughputs : array<vec3f, MAX_SKY_LIGHT_SLOTS - 1u>,
  depths : array<vec3f, MAX_SKY_LIGHT_SLOTS - 1u>,
}

fn viewMarchStart() -> ViewMarch {
  var march : ViewMarch;
  march.luminance = vec3f(0.0);
  march.throughput0 = vec3f(1.0);
  march.path0 = absorberPathStart();
  for (var p = 0u; p < MAX_SKY_LIGHT_SLOTS - 1u; p++) {
    march.throughputs[p] = vec3f(1.0);
    march.depths[p] = vec3f(0.0);
  }
  return march;
}

// Slot `slot`'s throughput, at most MAX_SKY_LIGHT_SLOTS - 1.
fn viewThroughputOf(march : ptr<function, ViewMarch>, slot : u32) -> vec3f {
  if (slot == 0u) {
    return (*march).throughput0;
  }
  return (*march).throughputs[slot - 1u];
}

// Slot `slot`'s path depth among slots 1 to 3's, `depth1` to `depth3`, selected so that no step
// holds them in an array (R08.T8.d).
fn viewSlotDepthOf(depth1 : vec3f, depth2 : vec3f, depth3 : vec3f, slot : u32) -> vec3f {
  switch (slot) {
    case 1u: {
      return depth1;
    }
    case 2u: {
      return depth2;
    }
    default: {
      return depth3;
    }
  }
}

// One step `dtM` of a march through the sample `local`, with each light slot's share of the
// source there (`viewSourcesAt`, `skySourcesAt`): each slot's path one step on under its curves,
// its share's in-scattering through its own throughput, without f32's cancellation (`stepFactor`),
// and its throughput carried on, slot 0 first. The paths' columns, each absorber's place on the
// curves' grid, its pressure row and its weights are the same for every slot, so they are formed
// once a step, and each slot loads only its own curve's texels (`curveNodeOf`, `curveRowsOf`, `curveDepthFrom`;
// R08.T8.e, E3): the arithmetic each slot's depth takes is `absorberStep`'s under its own curves.
fn viewMarchStep(
  march : ptr<function, ViewMarch>,
  local : SampleMedium,
  shares : ViewShares,
  dtM : f32,
) {
  let paths = viewPathCount();
  let count = absorberCount();
  let path0 = (*march).path0;
  let throughput0 = (*march).throughput0;
  // The paths one step on: as `absorberStep` makes them, every slot's from the same columns.
  var next0 = path0;
  var depth1 = vec3f(0.0);
  var depth2 = vec3f(0.0);
  var depth3 = vec3f(0.0);
  if (count > 0u) {
    let lines = lineSlots();
    next0.columns = addColumns(path0.columns, local.columns, dtM);
    next0.pressureColumns = addColumns(path0.pressureColumns, local.pressureColumns, dtM);
    var depth0 = vec3f(0.0);
    for (var k = 0u; k < min(count, ABSORBER_SLOTS); k++) {
      let x = columnAt(next0.columns, k);
      let c = pressureChannelOf(lines, k);
      var lnP = 0.0;
      if (c >= 0) {
        lnP = pathLogPressure(x, columnAt(next0.pressureColumns, u32(c)));
      }
      // `curveDepth` for each slot, its two light-independent parts formed once: a slot's depth
      // takes nothing where the column is 0, as `curveDepth`'s 0 adds nothing.
      if (x > 0.0) {
        let place = curveNodeOf(absorberCurves, x);
        let read = curveRowsOf(absorberCurves, k, lnP);
        // One call of a slot's read in a loop, so that SwiftShader, which inlines every call, holds
        // one copy of it (R08.T8.e).
        for (var p = 0u; p < paths; p++) {
          let one = curveDepthFrom(absorberCurves, p, x, place, read);
          switch (p) {
            case 0u: {
              depth0 += one;
            }
            case 1u: {
              depth1 += one;
            }
            case 2u: {
              depth2 += one;
            }
            default: {
              depth3 += one;
            }
          }
        }
      }
    }
    next0.depth = depth0;
  }
  let stepDepth0 = local.extinction * dtM + (next0.depth - path0.depth);
  (*march).path0 = next0;
  (*march).luminance += throughput0 * shares.slot0 * dtM * stepFactor(stepDepth0);
  (*march).throughput0 = throughput0 * exp(-stepDepth0);
  for (var p = 1u; p < paths; p++) {
    let before = (*march).depths[p - 1u];
    // With no absorber a slot's path keeps its depth, as `absorberStep` keeps the path.
    var after = before;
    if (count > 0u) {
      after = viewSlotDepthOf(depth1, depth2, depth3, p);
    }
    let throughput = (*march).throughputs[p - 1u];
    let stepDepth = local.extinction * dtM + (after - before);
    (*march).depths[p - 1u] = after;
    (*march).luminance += throughput * viewShareOf(shares, p) * dtM * stepFactor(stepDepth);
    (*march).throughputs[p - 1u] = throughput * exp(-stepDepth);
  }
}

// The views' mean transmittance of a march, slot 0's throughput averaged over the channels.
fn viewMeanTransmittance(march : ptr<function, ViewMarch>) -> f32 {
  let t = (*march).throughput0;
  return (t.r + t.g + t.b) / 3.0;
}

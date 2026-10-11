// The atmosphere's scattering source (plan R05, R05.T12.c, Design note 16): the medium at a sample,
// the single- and multiple-scattering source there, read from the per-planet tables, and the steps
// of the per-frame marches (R05.T12.e). Prepended, after `common.wgsl`, `medium.wgsl` and
// `view.wgsl`, to the sky-view, aerial-perspective and ray-march kernels, each of which also
// declares the phase tables as `var phaseTables : texture_2d_array<f32>;`, and the absorber-column
// table and the lights' curves of growth as `var absorberColumns : texture_2d<f32>;` and
// `var absorberCurves : texture_2d<f32>;` (R08.T6.c), in group 0; like `medium.wgsl`, it reads the
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
// (`marchSplit`, `marchStep`).

// The angle a ray scatters the sun's light through towards the eye: its cosine, which the closed
// forms take; u = sqrt(theta / pi), the phase tables' coordinate (`medium.ts`'s `PhaseTable`); and
// 1 - cos(theta) as half the squared chord, |d - s|^2 / 2 (plan R08, Design note 18, R08.T6.g),
// which keeps its precision near the forward peak, where 1 - d . s loses it in f32: 1 - cos(theta)
// falls below f32's resolution at 1 for theta under about 3.5e-4 rad. theta is atan2(|d x s|,
// d . s), for the same reason in place of acos(d . s), taken by `preciseAtan2`
// (`view/shaders/preciseTrig.wgsl`), since WGSL bounds the builtin atan2 only to 4096 ULP.
struct ScatteringAngle {
  cosTheta : f32,
  u : f32,
  oneMinusCos : f32,
}

// The scattering angle between a ray's unit direction and the unit direction to the sun.
fn scatteringAngle(dir : vec3f, sun : vec3f) -> ScatteringAngle {
  let cosTheta = dot(dir, sun);
  let theta = preciseAtan2(length(cross(dir, sun)), cosTheta);
  let chord = dir - sun;
  return ScatteringAngle(cosTheta, sqrt(theta / PI_VIEW), 0.5 * dot(chord, chord));
}

// Term i's phase per channel towards the eye, sr^-1 (`medium.ts`'s `phaseAt`). Term.phase.w: 0
// none; 1 Rayleigh, with each channel's depolarisation ratio rho in Term.phase.xyz, Chandrasekhar's
// 3 / (16 pi) ((1 + 3 gamma) + (1 - gamma) cos^2) / (1 + 2 gamma), gamma = rho / (2 - rho), which at
// rho = 0 is R05's 3 / (16 pi) (1 + cos^2) (R08.T3.c); 2 Cornette–Shanks with its g in
// Term.phase.x, its denominator 1 + g^2 - 2 g cos(theta) taken as (1 - g)^2 + 2 g (1 - cos(theta))
// (R08.T6.g), since in f32 the first form loses up to about 0.4% of the forward peak at g = 0.99,
// where (1 - g)^2 is 1e-4, and raised to 3/2 as x sqrt(x); 3 tabulated: layer i of `phaseTables`,
// its entries even in u from 0 to 1, read linear in u between the two about it.
fn phaseOf(i : u32, term : Term, angle : ScatteringAngle) -> vec3f {
  let kind = term.phase.w;
  if (kind < 0.5) {
    return vec3f(0.0);
  }
  let cosTheta = angle.cosTheta;
  if (kind < 1.5) {
    let gamma = term.phase.xyz / (2.0 - term.phase.xyz);
    let shape = 1.0 + 3.0 * gamma + (1.0 - gamma) * (cosTheta * cosTheta);
    return 3.0 / (16.0 * PI_VIEW) * shape / (1.0 + 2.0 * gamma);
  }
  if (kind < 2.5) {
    let g = term.phase.x;
    let k = 3.0 / (8.0 * PI_VIEW) * (1.0 - g * g) / (2.0 + g * g);
    let denominator = max((1.0 - g) * (1.0 - g) + 2.0 * g * angle.oneMinusCos, 1e-6);
    let peak = denominator * sqrt(denominator);
    return vec3f(k * (1.0 + cosTheta * cosTheta) / peak);
  }
  let n = textureDimensions(phaseTables).x;
  let x = clamp(angle.u, 0.0, 1.0) * f32(n - 1u);
  let k = min(u32(x), n - 2u);
  let below = textureLoad(phaseTables, vec2u(k, 0u), i, 0).rgb;
  let above = textureLoad(phaseTables, vec2u(k + 1u, 0u), i, 0).rgb;
  return mix(below, above, x - f32(k));
}

// The medium at a sample, each term's density evaluated once (R05.T12.e): the scattering and the
// extinction, m^-1, the scattering towards the eye, each term's scattering times its phase,
// per channel, m^-1 sr^-1, and the absorbers' columns per metre (R08.T6.c).
struct SampleMedium {
  scattering : vec3f,
  extinction : vec3f,
  phasedScattering : vec3f,
  columns : AbsorberColumns,
}

// The medium at a height above the datum, for the angle between the view direction and the
// direction to the sun.
fn sampleMediumAt(heightM : f32, angle : ScatteringAngle) -> SampleMedium {
  var scattering = vec3f(0.0);
  var extinction = vec3f(0.0);
  var phased = vec3f(0.0);
  var columns = AbsorberColumns(0.0, vec4f(0.0));
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    let d = densityOf(i, term.profile, heightM);
    let scattered = term.scattering.rgb * d;
    scattering += scattered;
    extinction += (term.scattering.rgb + term.absorption.rgb) * d;
    phased += scattered * phaseOf(i, term, angle);
    columns = addColumns(columns, termColumns(term, d), 1.0);
  }
  return SampleMedium(scattering, extinction, phased, columns);
}

// The sun's transmittance from a height above the datum along a zenith cosine to the top, read from
// the table on the tables' own sphere, of ground radius `tableBottomM` and the medium's height:
// its stored optical depth and absorber columns, with the sun's curves of growth (light slot 0,
// the one sun the kernels draw until R08.T7), exponentiated in f32 (`transmittanceThroughTables`,
// R08.T6.c). Each kernel that takes this declares `absorberColumns` and `absorberCurves`.
fn tableTransmittance(table : texture_2d<f32>, tableBottomM : f32, heightM : f32, mu : f32) -> vec3f {
  let h = clamp(heightM, 0.0, medium.topHeightM);
  let unit = transmittanceUnitAt(tableBottomM, medium.topHeightM, h, mu);
  return transmittanceThroughTables(
    table, absorberColumns, absorberCurves, absorberCount(), 0u, unit
  );
}

// A march's absorber path one step `dtM` on, through the sample `local` (`absorberStep`): the
// view's own path, under the sun's curves, since what it carries is the sun's light (R08.T6.c).
fn viewAbsorberStep(path : AbsorberPath, local : SampleMedium, dtM : f32) -> AbsorberPath {
  return absorberStep(absorberCurves, absorberCount(), 0u, path, local.columns, dtM);
}

// The multiple-scattering source per unit illuminance at a height, for the sun's zenith cosine.
fn tableMultiScattering(table : texture_2d<f32>, heightM : f32, muSun : f32) -> vec3f {
  return tableAtUnit(table, multiScatteringUnitAt(medium.topHeightM, max(heightM, 0.0), muSun)).rgb;
}

// The scattering source at a point per unit sun illuminance, per unit length: single scattering
// of the sun's light, shadowed by the ground, plus multiple scattering (Hillaire 2020, equations 3
// and 11). `local` is the medium at the point's height, `sampleMediumAt(heightM, angle)`.
fn sourceAt(
  transmittance : texture_2d<f32>,
  multiScattering : texture_2d<f32>,
  tableBottomM : f32,
  heightM : f32,
  muSun : f32,
  local : SampleMedium,
) -> vec3f {
  let lit = select(1.0, 0.0, groundHitFromHeight(tableBottomM, max(heightM, 0.0), muSun));
  let toSun = tableTransmittance(transmittance, tableBottomM, heightM, muSun);
  let single = lit * toSun * local.phasedScattering;
  let multiple = tableMultiScattering(multiScattering, heightM, muSun);
  return single + multiple * local.scattering;
}

// A ray's segment [tStart, tEnd] split at its lowest point t*, the point nearest the centre of the
// sphere the ray is marched about, with the steps each side takes (R05.T12.e, with addendum A of
// decision-r05-high-atmosphere.md; `marchSteps.ts` is its TypeScript twin). Even steps
// under-sample the dense air at that point (the camera, the ground or a limb's tangent point),
// whose aerosol has a 1.2 km scale height (plan R05, Risks, "The per-frame marches under-sample the
// dense air"), so each side is placed quadratically toward t*. The camera side of a two-sided
// segment that starts at a camera inside the atmosphere is placed toward the camera instead: where
// the camera's air is dense, on a ray that falls to a lowest point below the camera and climbs out,
// the view's own attenuation puts that side's light at the camera end. From 60-100 km it is not,
// and the high setting's 75 sky-view steps carry those rays (addendum B).
struct MarchSplit {
  startM : f32,
  // t* = clamp(-o.d, tStart, tEnd), m.
  lowestM : f32,
  // The lengths of the segment before and after t*, m.
  beforeM : f32,
  afterM : f32,
  stepsBefore : u32,
  stepsAfter : u32,
  // Whether the side before t* is placed toward the segment's start, the camera.
  beforeTowardStart : bool,
}

// Splits the segment at `nearestM`, the distance -o.d to the ray's closest approach to the centre.
// A two-sided segment shares its steps in proportion to the square root of each side's length,
// which gives each side the same smallest step L / n^2, at the end it is placed toward (t*, or the
// camera for a camera side), at least one a side; a one-sided segment gives all its steps to its
// side. `fromCamera`: the segment starts at a camera inside the atmosphere.
// `samples` is at least 2.
fn marchSplit(
  tStartM : f32,
  tEndM : f32,
  nearestM : f32,
  samples : u32,
  fromCamera : bool,
) -> MarchSplit {
  let lowest = clamp(nearestM, tStartM, tEndM);
  let before = lowest - tStartM;
  let after = tEndM - lowest;
  let rootBefore = sqrt(before);
  let total = rootBefore + sqrt(after);
  let share = select(0.0, floor(f32(samples) * rootBefore / total + 0.5), total > 0.0);
  let least = select(0u, 1u, before > 0.0);
  let most = samples - select(0u, 1u, after > 0.0);
  let stepsBefore = min(max(u32(share), least), most);
  let towardStart = fromCamera && before > 0.0 && after > 0.0;
  let stepsAfter = samples - stepsBefore;
  return MarchSplit(tStartM, lowest, before, after, stepsBefore, stepsAfter, towardStart);
}

// One step of a march: its midpoint's distance along the ray, its length, and its midpoint's
// signed distance from the split's lowest point t*, formed without the difference of the two
// distances (plan R08, Design note 18: the ray march's closest-approach form), m.
struct MarchStep {
  tM : f32,
  dtM : f32,
  fromLowestM : f32,
}

// Step `i` of a split segment, counted from tStart. A side of length L and n steps placed toward
// an end a has its boundaries at a -/+ L (k / n)^2, k = 0..n, so its step k from a has its midpoint
// at a -/+ L (k^2 + k + 1/2) / n^2 and the length L (2k + 1) / n^2.
fn marchStep(split : MarchSplit, i : u32) -> MarchStep {
  if (i < split.stepsBefore) {
    let n = f32(split.stepsBefore);
    let unitM = split.beforeM / (n * n);
    if (split.beforeTowardStart) {
      let k = f32(i);
      let fromStart = unitM * (k * k + k + 0.5);
      return MarchStep(split.startM + fromStart, unitM * (2.0 * k + 1.0), fromStart - split.beforeM);
    }
    let k = f32(split.stepsBefore - 1u - i);
    let fromLowest = unitM * (k * k + k + 0.5);
    return MarchStep(split.lowestM - fromLowest, unitM * (2.0 * k + 1.0), -fromLowest);
  }
  let n = f32(split.stepsAfter);
  let k = f32(i - split.stepsBefore);
  let unitM = split.afterM / (n * n);
  let fromLowest = unitM * (k * k + k + 0.5);
  return MarchStep(split.lowestM + fromLowest, unitM * (2.0 * k + 1.0), fromLowest);
}

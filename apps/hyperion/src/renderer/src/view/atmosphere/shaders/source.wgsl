// The atmosphere's scattering source (plan R05, R05.T12.c, Design note 16): the medium at a sample,
// and the single- and multiple-scattering source there, read from the per-planet tables; the steps
// of the per-frame marches (R05.T12.e) are `common.wgsl`'s since plan R08's R08.T6.d. Prepended,
// after `common.wgsl`, `medium.wgsl` and `view.wgsl`, to the sky-view, aerial-perspective and
// ray-march kernels, each of which also declares the phase tables as
// `var phaseTables : texture_2d_array<f32>;`, and the absorber-column table and the lights' curves
// of growth as `var absorberColumns : texture_2d<f32>;` and `var absorberCurves : texture_2d<f32>;`
// (R08.T6.c), and the pressure-weighted columns as
// `var absorberPressureColumns : texture_2d<f32>;` and
// `var lastAbsorberPressureColumn : texture_2d<f32>;` (R08.T4.d), and the forward-lobe tables as
// `var lobeTables : texture_2d_array<f32>;` (R08.T7), in group 0; like `medium.wgsl`, it reads the
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
// curves, a path a slot (`ViewMarch`), slot 0's throughput the views' mean transmittance.

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
// extinction, m^-1, the absorbers' columns per metre (R08.T6.c), the same times the pressure
// ratio there (R08.T4.d), and each term's density, from which each source's scattering towards
// the eye is formed (`phasedScatteringOf`, R08.T7).
struct SampleMedium {
  scattering : vec3f,
  extinction : vec3f,
  columns : AbsorberColumns,
  pressureColumns : AbsorberColumns,
  densities : array<f32, MAX_TERMS>,
}

// The medium at a height above the datum.
fn sampleMediumAt(heightM : f32) -> SampleMedium {
  var scattering = vec3f(0.0);
  var extinction = vec3f(0.0);
  var columns = AbsorberColumns(0.0, vec4f(0.0));
  var densities : array<f32, MAX_TERMS>;
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    let d = densityOf(i, term.profile, heightM);
    densities[i] = d;
    scattering += term.scattering.rgb * d;
    extinction += (term.scattering.rgb + term.absorption.rgb) * d;
    columns = addColumns(columns, termColumns(term, d), 1.0);
  }
  let pressureColumns = pressureColumnsOf(columns, lineSlots(), pressureBarAt(heightM));
  return SampleMedium(scattering, extinction, columns, pressureColumns, densities);
}

// The scattering towards the eye from one source at a sample, m^-1 sr^-1: each term's scattering
// times its phase per channel for the angle between the ray and the source, in the terms' order.
fn phasedScatteringOf(local : SampleMedium, angle : ScatteringAngle) -> vec3f {
  var phased = vec3f(0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    let scattered = term.scattering.rgb * local.densities[i];
    phased += scattered * phaseOf(i, term, angle);
  }
  return phased;
}

// The phase-table entries a tabulated term's forward-lobe orders stand on, u_k = k / 255 to 180°,
// the lobe tables' x nodes and their largest x (`forwardLobe.ts`'s FORWARD_ORDERS_ENTRIES,
// FORWARD_LOBE_X_NODES and FORWARD_LOBE_X_MAX; R08.T7).
const LOBE_ORDERS_ENTRIES : u32 = 256u;
const LOBE_X_NODES : u32 = 48u;
const LOBE_X_MAX : f32 = 64.0;

// Whether a term has a forward lobe whose higher orders the marches add (R08.T7): a tabulated
// phase whose lobe is drawn, its share P in phase.xyz above 0.
fn hasLobe(term : Term) -> bool {
  return term.phase.w > 2.5 && any(term.phase.xyz > vec3f(0.0));
}

// Whether any of the medium's terms has one: the same for every texel, so the branch it guards is
// uniform.
fn hasForwardLobes() -> bool {
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    if (hasLobe(terms[i])) {
      return true;
    }
  }
  return false;
}

// Term i's forward-lobe effective phase, E~(x, theta) per channel, sr^-1 (`forwardLobe.ts`): layer i
// of `lobeTables`, linear in u between the phase table's entries about it, as `phaseOf` reads a
// tabulated phase, and linear in the index of the x nodes x_j = X_MAX (j / 47)^2 about each
// channel's x; past the orders' entries (1 - e^(-x P)) times the term's phase at the angle, the
// forward-diffuse light scattered by the phase itself (`forwardLobe.ts`'s `forwardLobeAt`).
fn lobeOrdersAt(i : u32, u : f32, x : vec3f, share : vec3f, phase : vec3f) -> vec3f {
  let at = clamp(u, 0.0, 1.0) * 255.0;
  if (at >= f32(LOBE_ORDERS_ENTRIES - 1u)) {
    return (1.0 - exp(-max(x, vec3f(0.0)) * share)) * phase;
  }
  let k = u32(at);
  let fk = at - f32(k);
  let last = f32(LOBE_X_NODES - 1u);
  var out = vec3f(0.0);
  for (var c = 0u; c < 3u; c++) {
    let node = min(sqrt(max(x[c], 0.0) / LOBE_X_MAX) * last, last);
    let j = min(u32(node), LOBE_X_NODES - 2u);
    let fj = node - f32(j);
    let a = mix(
      textureLoad(lobeTables, vec2u(k, j), i, 0)[c],
      textureLoad(lobeTables, vec2u(k + 1u, j), i, 0)[c],
      fk,
    );
    let b = mix(
      textureLoad(lobeTables, vec2u(k, j + 1u), i, 0)[c],
      textureLoad(lobeTables, vec2u(k + 1u, j + 1u), i, 0)[c],
      fk,
    );
    out[c] = mix(a, b, fj);
  }
  return out;
}

// The lobe terms' forward-lobe higher orders towards the eye from one source at a sample, m^-1
// sr^-1 (R08.T7; `forwardLobe.ts`): each such term's scattering times e^(x P) E~(x, theta), P its
// lobe's share (its phase.xyz) and x the lobe terms' scattering depth, their share of the sample's
// extinction times `depth`, the stored optical depth to the top along the source; the exponent
// held under 80.
fn lobeOrdersOf(local : SampleMedium, angle : ScatteringAngle, depth : vec3f) -> vec3f {
  let count = min(u32(medium.termCount), MAX_TERMS);
  var lobeScattering = vec3f(0.0);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    if (hasLobe(term)) {
      lobeScattering += term.scattering.rgb * local.densities[i];
    }
  }
  let x = depth * lobeScattering / max(local.extinction, vec3f(1e-30));
  var orders = vec3f(0.0);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    if (!hasLobe(term)) {
      continue;
    }
    let scattered = term.scattering.rgb * local.densities[i];
    let boost = exp(min(x * term.phase.xyz, vec3f(80.0)));
    let phase = phaseOf(i, term, angle);
    orders += scattered * boost * lobeOrdersAt(i, angle.u, x, term.phase.xyz, phase);
  }
  return orders;
}

// A source's transmittance from a height above the datum along a zenith cosine to the top, read
// from the table on the tables' own sphere, of ground radius `tableBottomM` and the medium's
// height: its stored optical depth and absorber columns, with light slot `slot`'s curves of growth,
// exponentiated in f32 (`transmittanceThroughTables`, R08.T6.c; each source's own slot since
// R08.T7). Each kernel that takes this declares `absorberColumns`, `absorberCurves`,
// `absorberPressureColumns` and `lastAbsorberPressureColumn` (R08.T4.d).
fn tableTransmittance(
  table : texture_2d<f32>,
  tableBottomM : f32,
  heightM : f32,
  mu : f32,
  slot : u32,
) -> vec3f {
  let h = clamp(heightM, 0.0, medium.topHeightM);
  let unit = transmittanceUnitAt(tableBottomM, medium.topHeightM, h, mu);
  return transmittanceThroughTables(
    table, absorberColumns, absorberPressureColumns, lastAbsorberPressureColumn, absorberCurves,
    absorberCount(), slot, lineSlots(), unit
  );
}

// A march's absorber path one step `dtM` on, through the sample `local` (`absorberStep`): the
// view's own path under light slot `slot`'s curves, since what it carries is that light's
// (R08.T6.c; a path a slot since R08.T7), its lines at the path's pressure (R08.T4.d).
fn viewAbsorberStep(path : AbsorberPath, local : SampleMedium, dtM : f32, slot : u32) -> AbsorberPath {
  return absorberStep(
    absorberCurves, absorberCount(), slot, lineSlots(), path, local.columns, local.pressureColumns,
    dtM
  );
}

// The multiple-scattering source per unit illuminance at a height, for the source's zenith cosine,
// from its light slot's layer (R08.T7), or the one layer of a medium with no absorber.
fn tableMultiScattering(
  table : texture_2d_array<f32>,
  heightM : f32,
  muSun : f32,
  slot : u32,
) -> vec3f {
  let unit = multiScatteringUnitAt(medium.topHeightM, max(heightM, 0.0), muSun);
  return tableAtUnitLayer(table, slot, unit).rgb;
}

// One source's scattering source at a point per unit of its illuminance, per unit length: single
// scattering of its light, shadowed by the ground, plus multiple scattering (Hillaire 2020,
// equations 3 and 11). `local` is the medium at the point's height (`sampleMediumAt`), `phased`
// its scattering towards the eye for the source (`phasedScatteringOf`).
fn sourceAt(
  transmittance : texture_2d<f32>,
  multiScattering : texture_2d_array<f32>,
  tableBottomM : f32,
  heightM : f32,
  muSun : f32,
  slot : u32,
  local : SampleMedium,
  phased : vec3f,
) -> vec3f {
  let lit = select(1.0, 0.0, groundHitFromHeight(tableBottomM, max(heightM, 0.0), muSun));
  let toSun = tableTransmittance(transmittance, tableBottomM, heightM, muSun, slot);
  let single = lit * toSun * phased;
  let multiple = tableMultiScattering(multiScattering, heightM, muSun, slot);
  return single + multiple * local.scattering;
}

// The scattering angle between a ray and each of the view's sources, constant along a straight
// ray, `dirs` the sources' directions in the frame the ray's is given in.
fn sourceAngles(
  dir : vec3f,
  dirs : array<vec3f, MAX_SKY_SOURCES>,
  count : u32,
) -> array<ScatteringAngle, MAX_SKY_SOURCES> {
  var angles : array<ScatteringAngle, MAX_SKY_SOURCES>;
  for (var k = 0u; k < count; k++) {
    angles[k] = scatteringAngle(dir, dirs[k]);
  }
  return angles;
}

// The view's sources at a sample, summed by light slot: each source's `sourceAt` times its weight,
// added into its slot's share, so that each share is carried along the view's path under its own
// curves (`viewMarchStep`). `dirs` are the sources' directions and `up` the normal the sample's
// heights and zenith cosines are taken against, both in one frame.
fn viewSourcesAt(
  heightM : f32,
  up : vec3f,
  dirs : array<vec3f, MAX_SKY_SOURCES>,
  angles : array<ScatteringAngle, MAX_SKY_SOURCES>,
  local : SampleMedium,
) -> array<vec3f, MAX_SKY_LIGHT_SLOTS> {
  var shares : array<vec3f, MAX_SKY_LIGHT_SLOTS>;
  let count = skySourceCount(view);
  let lobes = hasForwardLobes();
  for (var k = 0u; k < count; k++) {
    let source = view.sources[k];
    let slot = skySlotOf(source);
    let muSun = dot(dirs[k], up);
    var phased = phasedScatteringOf(local, angles[k]);
    if (lobes) {
      // The forward lobes' higher orders, at the stored optical depth along the source (R08.T7).
      let h = clamp(heightM, 0.0, medium.topHeightM);
      let unit = transmittanceUnitAt(view.tables.x, medium.topHeightM, h, muSun);
      phased += lobeOrdersOf(local, angles[k], tableAtUnit(transmittance, unit).rgb);
    }
    let one = sourceAt(
      transmittance, multiScattering, view.tables.x, heightM, muSun, slot, local, phased
    );
    shares[slot] += source.weight.rgb * one;
  }
  return shares;
}

// A march along a ray so far: the light it has gathered, and per light slot its throughput and its
// own absorber path (R08.T7), slot 0's throughput the views' mean transmittance.
struct ViewMarch {
  luminance : vec3f,
  throughput : array<vec3f, MAX_SKY_LIGHT_SLOTS>,
  paths : array<AbsorberPath, MAX_SKY_LIGHT_SLOTS>,
}

fn viewMarchStart() -> ViewMarch {
  var march : ViewMarch;
  march.luminance = vec3f(0.0);
  for (var p = 0u; p < MAX_SKY_LIGHT_SLOTS; p++) {
    march.throughput[p] = vec3f(1.0);
    march.paths[p] = absorberPathStart();
  }
  return march;
}

// One step `dtM` of a march through the sample `local`, with each light slot's share of the
// source there (`viewSourcesAt`): each slot's path one step on under its curves, its share's
// in-scattering through its own throughput, without f32's cancellation (`stepFactor`), and its
// throughput carried on.
fn viewMarchStep(
  march : ptr<function, ViewMarch>,
  local : SampleMedium,
  shares : array<vec3f, MAX_SKY_LIGHT_SLOTS>,
  dtM : f32,
) {
  let paths = skyPathCount(view);
  for (var p = 0u; p < paths; p++) {
    let next = viewAbsorberStep((*march).paths[p], local, dtM, p);
    let stepDepth = local.extinction * dtM + (next.depth - (*march).paths[p].depth);
    (*march).paths[p] = next;
    (*march).luminance += (*march).throughput[p] * shares[p] * dtM * stepFactor(stepDepth);
    (*march).throughput[p] *= exp(-stepDepth);
  }
}

// The views' mean transmittance of a march, slot 0's throughput averaged over the channels.
fn viewMeanTransmittance(march : ViewMarch) -> f32 {
  let t = march.throughput[0];
  return (t.r + t.g + t.b) / 3.0;
}

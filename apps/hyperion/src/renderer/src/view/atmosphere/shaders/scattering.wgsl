// The medium at a sample and its scattering towards a ray (plan R05, R05.T12.c; plan R08, R08.T6.b,
// R08.T7 and R08.T9.b): each term's density, the sample's scattering, extinction and absorber
// columns, and each term's phase for the angle between the ray and a source, the tabulated terms'
// forward lobes' higher orders among them. Moved out of `source.wgsl` by R08.T9.b, unchanged but
// for the constant pi (`preciseTrig.wgsl`'s TRIG_PI, the same f32 as `view.wgsl`'s PI_VIEW), so
// that the sky-irradiance kernel (`irradiance.wgsl`), which declares no view, takes them as the
// per-frame kernels do. Prepended, after `view/shaders/preciseTrig.wgsl`, `common.wgsl` and
// `medium.wgsl`, to the sky-view, aerial-perspective, ray-march and irradiance kernels, each of
// which also declares the phase tables as `var phaseTables : texture_2d_array<f32>;` and the
// forward-lobe tables as `var lobeTables : texture_2d_array<f32>;` (R08.T7) in group 0; it reads
// the kernel's `medium`, `terms` and tables in place.
//
// Ported from Bevy 0.19.1's functions.wgsl (`sample_local_inscattering`, MIT; the notice is in
// common.wgsl) and checked against sebh's IntegrateScatteredLuminance (MIT; the notice is in
// multiScattering.wgsl). Changes: the phase functions are the medium's own terms', per channel, a
// tabulated one among them (R08.T6.b); each term's density is evaluated once a sample (R05.T12.e);
// the scattering angle is taken without f32's cancellation near the forward peak (R08.T6.g); the
// densities are held a member a term (`TermDensities`), selected by a switch, not in an array
// (R08.T8.d; source.wgsl's header).

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
  return ScatteringAngle(cosTheta, sqrt(theta / TRIG_PI), 0.5 * dot(chord, chord));
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
    return 3.0 / (16.0 * TRIG_PI) * shape / (1.0 + 2.0 * gamma);
  }
  if (kind < 2.5) {
    let g = term.phase.x;
    let k = 3.0 / (8.0 * TRIG_PI) * (1.0 - g * g) / (2.0 + g * g);
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

// Each term's density at a sample, a member a term (MAX_TERMS of them), so that no step indexes an
// array (R08.T8.d).
struct TermDensities {
  d0 : f32,
  d1 : f32,
  d2 : f32,
  d3 : f32,
  d4 : f32,
  d5 : f32,
  d6 : f32,
  d7 : f32,
}

// Term `i`'s density, `i` under MAX_TERMS.
fn termDensity(densities : TermDensities, i : u32) -> f32 {
  switch (i) {
    case 0u: {
      return densities.d0;
    }
    case 1u: {
      return densities.d1;
    }
    case 2u: {
      return densities.d2;
    }
    case 3u: {
      return densities.d3;
    }
    case 4u: {
      return densities.d4;
    }
    case 5u: {
      return densities.d5;
    }
    case 6u: {
      return densities.d6;
    }
    default: {
      return densities.d7;
    }
  }
}

// `densities` with term `i`'s set to `d`.
fn withTermDensity(densities : TermDensities, i : u32, d : f32) -> TermDensities {
  var out = densities;
  switch (i) {
    case 0u: {
      out.d0 = d;
    }
    case 1u: {
      out.d1 = d;
    }
    case 2u: {
      out.d2 = d;
    }
    case 3u: {
      out.d3 = d;
    }
    case 4u: {
      out.d4 = d;
    }
    case 5u: {
      out.d5 = d;
    }
    case 6u: {
      out.d6 = d;
    }
    default: {
      out.d7 = d;
    }
  }
  return out;
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
  densities : TermDensities,
}

// The medium at a height above the datum.
fn sampleMediumAt(heightM : f32) -> SampleMedium {
  var scattering = vec3f(0.0);
  var extinction = vec3f(0.0);
  var columns = AbsorberColumns(0.0, vec4f(0.0));
  var densities = TermDensities(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    let d = densityOf(i, term.profile, heightM);
    densities = withTermDensity(densities, i, d);
    scattering += term.scattering.rgb * d;
    extinction += (term.scattering.rgb + term.absorption.rgb) * d;
    columns = addColumns(columns, termColumns(term, d), 1.0);
  }
  // A medium with no absorber has no column to weight: its pressure-weighted columns are the zeros
  // `pressureColumnsOf` would give (R08.T8.d).
  var pressureColumns = AbsorberColumns(0.0, vec4f(0.0));
  if (absorberCount() > 0u) {
    pressureColumns = pressureColumnsOf(columns, lineSlots(), pressureBarAt(heightM));
  }
  return SampleMedium(scattering, extinction, columns, pressureColumns, densities);
}

// The scattering towards the eye from one source at a sample, m^-1 sr^-1: each term's scattering
// times its phase per channel for the angle between the ray and the source, in the terms' order.
fn phasedScatteringOf(local : SampleMedium, angle : ScatteringAngle) -> vec3f {
  var phased = vec3f(0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    let scattered = term.scattering.rgb * termDensity(local.densities, i);
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
      lobeScattering += term.scattering.rgb * termDensity(local.densities, i);
    }
  }
  let x = depth * lobeScattering / max(local.extinction, vec3f(1e-30));
  var orders = vec3f(0.0);
  for (var i = 0u; i < count; i++) {
    let term = terms[i];
    if (!hasLobe(term)) {
      continue;
    }
    let scattered = term.scattering.rgb * termDensity(local.densities, i);
    let boost = exp(min(x * term.phase.xyz, vec3f(80.0)));
    let phase = phaseOf(i, term, angle);
    orders += scattered * boost * lobeOrdersAt(i, angle.u, x, term.phase.xyz, phase);
  }
  return orders;
}

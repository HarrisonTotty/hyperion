// The atmosphere's scattering source (plan R05, R05.T12.c, Design note 16): the single- and
// multiple-scattering source at a point, read from the per-planet tables. Prepended, after
// `common.wgsl`, `medium.wgsl` and `view.wgsl`, to the sky-view, aerial-perspective and ray-march
// kernels; like `medium.wgsl`, it reads the kernel's `medium` uniform in place, so the composite
// does not take it.
//
// Ported from Bevy 0.19.1's functions.wgsl (`sample_local_inscattering`, MIT; the notice is in
// common.wgsl) and checked against sebh's IntegrateScatteredLuminance (MIT; the notice is in
// multiScattering.wgsl). Changes: the phase functions are the medium's own terms'; the tables are
// read on their own sphere, at the sample's height above the datum (Design note 16).

// The phase a term scatters with towards the eye, for the cosine between the view direction and
// the direction to the sun. Term.scattering.w: 0 none, 1 Rayleigh, 2 Cornette–Shanks with its g in
// Term.absorption.w.
fn phaseOf(term : Term, cosTheta : f32) -> f32 {
  let kind = term.scattering.w;
  if (kind < 0.5) {
    return 0.0;
  }
  if (kind < 1.5) {
    return 3.0 / (16.0 * PI_VIEW) * (1.0 + cosTheta * cosTheta);
  }
  let g = term.absorption.w;
  let k = 3.0 / (8.0 * PI_VIEW) * (1.0 - g * g) / (2.0 + g * g);
  return k * (1.0 + cosTheta * cosTheta) / pow(max(1.0 + g * g - 2.0 * g * cosTheta, 1e-6), 1.5);
}

// Each term's scattering times its phase, summed, at a height above the datum.
fn phasedScatteringAt(heightM : f32, cosTheta : f32) -> vec3f {
  var sum = vec3f(0.0);
  let count = min(u32(medium.termCount), MAX_TERMS);
  for (var i = 0u; i < count; i++) {
    let term = medium.terms[i];
    sum += term.scattering.rgb * densityOf(term.profile, heightM) * phaseOf(term, cosTheta);
  }
  return sum;
}

// The transmittance from a height above the datum along a zenith cosine to the top, read from the
// table on the tables' own sphere.
fn tableTransmittance(table : texture_2d<f32>, tableBottomM : f32, heightM : f32, mu : f32) -> vec3f {
  let top = tableBottomM + (medium.topRadiusM - medium.bottomRadiusM);
  let r = tableBottomM + clamp(heightM, 0.0, top - tableBottomM);
  return bilinear(table, shellRMuToUv(tableBottomM, top, r, mu)).rgb;
}

// The multiple-scattering source per unit illuminance at a height, for the sun's zenith cosine.
fn tableMultiScattering(
  table : texture_2d<f32>,
  tableBottomM : f32,
  heightM : f32,
  muSun : f32,
) -> vec3f {
  let top = tableBottomM + (medium.topRadiusM - medium.bottomRadiusM);
  let size = vec2f(textureDimensions(table));
  let r = tableBottomM + max(heightM, 0.0);
  return bilinear(table, shellMultiScatteringRMuToUv(tableBottomM, top, r, muSun, size)).rgb;
}

// The scattering source at a point per unit sun illuminance, per unit length: single scattering
// of the sun's light, shadowed by the ground, plus multiple scattering (Hillaire 2020, equations 3
// and 11).
fn sourceAt(
  transmittance : texture_2d<f32>,
  multiScattering : texture_2d<f32>,
  tableBottomM : f32,
  heightM : f32,
  muSun : f32,
  cosTheta : f32,
) -> vec3f {
  let rTable = tableBottomM + max(heightM, 0.0);
  let lit = select(1.0, 0.0, shellIntersectsGround(tableBottomM, rTable, muSun));
  let toSun = tableTransmittance(transmittance, tableBottomM, heightM, muSun);
  let single = lit * toSun * phasedScatteringAt(heightM, cosTheta);
  let local = mediumAt(medium.bottomRadiusM + heightM);
  let multiple = tableMultiScattering(multiScattering, tableBottomM, heightM, muSun);
  return single + multiple * local.scattering;
}

// The thick table's read over a body's latitude bands, for the per-frame kernels (plan R08,
// R08.T14.c and R08.T14.f; `thick/read.ts`): appended after `thickRead.wgsl` by `hillaire.ts`'s
// withThickRead, on `oblate.wgsl`'s body and bands.

// J0, G and G2 of light slot `slot` at the bands about s, as the multiple scattering's bands are read
// (`oblate.wgsl`'s bandTableAt): linear in ln s between the two bands about s, the nearer band
// beyond them, one band's alone. Two reads of a layer at most, so that the kernels inline
// thickLayerAt twice.
fn thickBandsAt(
  body : OblateBody,
  row : ThickRow,
  s : f32,
  muSun : f32,
  muTravel : f32,
  slot : u32,
) -> ThickSource {
  let layers = thickSlotCount();
  let k = min(slot, layers - 1u);
  let last = oblateBandCount(body) - 1u;
  var lower = 0u;
  var w = 0.0;
  if (last > 0u) {
    let ls = log(s);
    if (!(ls > oblateBand(oblateBody, 0u).y)) {
      lower = 0u;
    } else if (!(ls < oblateBand(oblateBody, last).y)) {
      lower = last;
    } else {
      while (lower + 1u < last && oblateBand(oblateBody, lower + 1u).y <= ls) {
        lower++;
      }
      let l0 = oblateBand(oblateBody, lower).y;
      w = (ls - l0) / (oblateBand(oblateBody, lower + 1u).y - l0);
    }
  }
  let near = thickLayerAt(lower * layers + k, row, muSun, muTravel);
  if (w == 0.0) {
    return near;
  }
  return thickMix(near, thickLayerAt((lower + 1u) * layers + k, row, muSun, muTravel), w);
}

// One source's multiple scattering per unit of its illuminance and of scattering, sr^-1, for the
// view along the unit `ray` at a sample whose unit zenith is `up`, the source along `dir`, all three
// in the march's frame: on the bake's table the analytic term (`oblate.wgsl`'s bandMultiScatteringAt)
// at its share and the table's J, held at 0, at the rest; beside the correction alone, the analytic
// term whole and the correction at the table's share, the sum held at 0 (`thick/read.ts`'s
// thickMultipleInto).
fn thickMultipleAt(
  body : OblateBody,
  start : LookupStart,
  muSun : f32,
  slot : u32,
  ray : vec3f,
  dir : vec3f,
  up : vec3f,
) -> vec3f {
  let analytic = thickAnalyticShare(muSun);
  let correction = thickCorrectionOnly();
  let weight = select(analytic, 1.0, correction);
  var multiple = vec3f(0.0);
  if (weight > 0.0) {
    multiple = weight * bandMultiScatteringAt(
      body, oblateBody, multiScattering, start.scaledHeightM, start.gravityRatio, muSun, slot
    );
  }
  if (analytic < 1.0) {
    let row = thickRowAt(max(start.scaledHeightM, 0.0));
    let muView = dot(ray, up);
    let horizontal = dot(ray, dir) - muView * muSun;
    let harmonic = 2.0 * horizontal * horizontal - (1.0 - muView * muView) * (1.0 - muSun * muSun);
    let source = thickBandsAt(body, row, start.gravityRatio, muSun, -muView, slot);
    let j = source.level + source.slope * horizontal + source.curve * harmonic;
    if (correction) {
      multiple = max(multiple + (1.0 - analytic) * j, vec3f(0.0));
    } else {
      multiple += (1.0 - analytic) * max(j, vec3f(0.0));
    }
  }
  return multiple;
}

// `source.wgsl`'s sourceAt with the table's multiple scattering: single scattering of the source's
// light, shadowed by the ground, plus thickMultipleAt times the sample's scattering.
fn thickSourceAt(
  body : OblateBody,
  start : LookupStart,
  light : SourceLight,
  muSun : f32,
  slot : u32,
  local : SampleMedium,
  phased : vec3f,
  ray : vec3f,
  dir : vec3f,
  up : vec3f,
) -> vec3f {
  let single = light.lit * light.toSun * phased;
  return single + thickMultipleAt(body, start, muSun, slot, ray, dir, up) * local.scattering;
}

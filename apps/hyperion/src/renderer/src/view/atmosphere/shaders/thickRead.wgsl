// The thick multiple-scattering table's read (plan R08, R08.T14.c and R08.T14.f; Design notes 9 and
// 10; `thick/read.ts`, its packing and CPU twin): J_ms from the bake with the polarisation
// correction summed into it, in place of Hillaire's sigma_s Psi_ms p_u, or the correction alone
// beside it, for a body whose tables hold one. Appended to the sky-view, aerial-perspective and
// march kernels by `hillaire.ts`'s withThickRead, which points their sources' multiple scattering
// here and leaves every other line as it is.
//
// The table, `rgba32uint`, one layer a band and light slot (band b's slot k at b x slots + k): 16
// texels across, one a travel cosine mu_v = (v + 1/2) / 8 - 1, and rows a x 32 + s, altitude a of 33
// and sun cosine mu0 = (s + 1/2) / 32, then the info row. A texel holds six halves in its first three
// words: J0's red and green, J0's blue and G's red, G's green and blue, each word's first half its
// low one; and G2 in its fourth, three 9-bit two's-complement mantissas (red in bits 0 to 8) and
// their shared exponent e in bits 27 to 31, each m 2^(e - 24). J0, G and G2 are E0, E1 /
// (sin theta_v sin theta0) and E2 / (sin theta_v sin theta0)^2 divided by the kernels' scattering at
// the altitude, so that the read, J0 + G h + G2 (2 h^2 - (1 - mu_v^2)(1 - mu0^2)) with
// h = v.s - (v.n)(s.n), is per steradian and per unit of scattering, as Psi_ms is. Layer 0's info row
// holds the altitudes, h* above the ground in m, as f32 bits in words 0 to 32, the light slots a band
// in word 33, and the table's kind in word 34: 0 the bake's (`thick`), 1 the correction alone
// (`correction`), a thin world's.
@group(0) @binding(24) var thickTable : texture_2d_array<u32>;

const THICK_ALTITUDES : u32 = 33u;
const THICK_SUN_COSINES : u32 = 32u;
const THICK_TRAVEL_COSINES : u32 = 16u;
const THICK_INFO_ROW : u32 = 1056u;
const THICK_KIND_WORD : u32 = 34u;
const THICK_CURVE_BITS : u32 = 9u;
const THICK_CURVE_EXPONENT_BIAS : i32 = 24;

// Word `i` of layer 0's info row.
fn thickInfoWord(i : u32) -> u32 {
  return textureLoad(thickTable, vec2u(i / 4u, THICK_INFO_ROW), 0u, 0)[i % 4u];
}

// The table's altitude `a`, h* above the ground, m.
fn thickAltitude(a : u32) -> f32 {
  return bitcast<f32>(thickInfoWord(a));
}

// The light slots a band.
fn thickSlotCount() -> u32 {
  return max(thickInfoWord(THICK_ALTITUDES), 1u);
}

// Whether the table holds the polarisation correction alone, a thin world's (R08.T14.f): Psi_ms then
// stays whole and the correction is added to it.
fn thickCorrectionOnly() -> bool {
  return thickInfoWord(THICK_KIND_WORD) != 0u;
}

// The analytic term's share of a source's multiple scattering at its zenith cosine `muSun` on the
// bake's table: all of it at and below the horizontal, where the table has no beam, none from the
// table's first mu0 cell's upper edge, 1/32, and linear across that cell; the table takes the rest.
fn thickAnalyticShare(muSun : f32) -> f32 {
  return 1.0 - clamp(muSun * f32(THICK_SUN_COSINES), 0.0, 1.0);
}

// The analytic term's weight, and the forward lobes' higher orders': its share on the bake's table,
// since the table's E holds the aureole scattered once more that the orders' tail adds (their
// in-lobe part, which E leaves out, is left out on the table's side, `thick/read.ts`); whole beside
// the correction alone.
fn thickAnalyticWeight(muSun : f32) -> f32 {
  return select(thickAnalyticShare(muSun), 1.0, thickCorrectionOnly());
}

// Where h* lies among the altitudes: the row below it, at most 31, and its share to the next, in
// [0, 1], by five halvings of 0 to 32.
struct ThickRow {
  lower : u32,
  t : f32,
}

fn thickRowAt(scaledHeightM : f32) -> ThickRow {
  var lower = 0u;
  var upper = THICK_ALTITUDES - 1u;
  for (var i = 0u; i < 5u; i++) {
    let middle = (lower + upper) / 2u;
    if (thickAltitude(middle) <= scaledHeightM) {
      lower = middle;
    } else {
      upper = middle;
    }
  }
  let below = thickAltitude(lower);
  let above = thickAltitude(lower + 1u);
  return ThickRow(lower, clamp((scaledHeightM - below) / (above - below), 0.0, 1.0));
}

// J0, G and G2 per channel.
struct ThickSource {
  level : vec3f,
  slope : vec3f,
  curve : vec3f,
}

// G2 of a texel's fourth word, exactly: each 9-bit mantissa sign-extended, times 2^(e - 24).
fn thickCurveOf(word : u32) -> vec3f {
  let bits = bitcast<i32>(word);
  let mantissas = vec3f(
    f32(extractBits(bits, 0u, THICK_CURVE_BITS)),
    f32(extractBits(bits, THICK_CURVE_BITS, THICK_CURVE_BITS)),
    f32(extractBits(bits, 2u * THICK_CURVE_BITS, THICK_CURVE_BITS)),
  );
  return ldexp(mantissas, vec3i(i32(word >> (3u * THICK_CURVE_BITS)) - THICK_CURVE_EXPONENT_BIAS));
}

// Texel (a, s, v) of `layer`, its six halves and G2.
fn thickTexel(layer : u32, a : u32, s : u32, v : u32) -> ThickSource {
  let w = textureLoad(thickTable, vec2u(v, a * THICK_SUN_COSINES + s), layer, 0);
  let x = unpack2x16float(w.x);
  let y = unpack2x16float(w.y);
  let z = unpack2x16float(w.z);
  return ThickSource(vec3f(x.x, x.y, y.x), vec3f(y.y, z.x, z.y), thickCurveOf(w.w));
}

fn thickMix(a : ThickSource, b : ThickSource, t : f32) -> ThickSource {
  return ThickSource(
    a.level * (1.0 - t) + b.level * t,
    a.slope * (1.0 - t) + b.slope * t,
    a.curve * (1.0 - t) + b.curve * t,
  );
}

// One layer's J0, G and G2 at `row`, sun cosine `muSun` and travel cosine `muTravel` (positive upward:
// for a view along v, -(v.n)): linear in each between the cells' centres about it, held at the
// table's edges (`thick/read.ts`'s addThickLayer).
fn thickLayerAt(layer : u32, row : ThickRow, muSun : f32, muTravel : f32) -> ThickSource {
  let x = clamp(muSun * f32(THICK_SUN_COSINES) - 0.5, 0.0, f32(THICK_SUN_COSINES - 1u));
  let s0 = min(u32(x), THICK_SUN_COSINES - 2u);
  let fs = x - f32(s0);
  let y = clamp(
    (muTravel + 1.0) * f32(THICK_TRAVEL_COSINES / 2u) - 0.5, 0.0, f32(THICK_TRAVEL_COSINES - 1u)
  );
  let v0 = min(u32(y), THICK_TRAVEL_COSINES - 2u);
  let fv = y - f32(v0);
  let a0 = row.lower;
  let a1 = a0 + 1u;
  let lower = thickMix(
    thickMix(thickTexel(layer, a0, s0, v0), thickTexel(layer, a0, s0, v0 + 1u), fv),
    thickMix(thickTexel(layer, a0, s0 + 1u, v0), thickTexel(layer, a0, s0 + 1u, v0 + 1u), fv),
    fs,
  );
  let upper = thickMix(
    thickMix(thickTexel(layer, a1, s0, v0), thickTexel(layer, a1, s0, v0 + 1u), fv),
    thickMix(thickTexel(layer, a1, s0 + 1u, v0), thickTexel(layer, a1, s0 + 1u, v0 + 1u), fv),
    fs,
  );
  return thickMix(lower, upper, row.t);
}

// J0, G and G2 of light slot `slot` at the bands about s, as the multiple scattering's bands are read
// (`oblate.wgsl`'s bandTableAt): linear in ln s between the two bands about s, the nearer band
// beyond them, one band's alone.
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
  if (last == 0u) {
    return thickLayerAt(k, row, muSun, muTravel);
  }
  let ls = log(s);
  if (!(ls > oblateBand(oblateBody, 0u).y)) {
    return thickLayerAt(k, row, muSun, muTravel);
  }
  if (!(ls < oblateBand(oblateBody, last).y)) {
    return thickLayerAt(last * layers + k, row, muSun, muTravel);
  }
  var lower = 0u;
  while (lower + 1u < last && oblateBand(oblateBody, lower + 1u).y <= ls) {
    lower++;
  }
  let l0 = oblateBand(oblateBody, lower).y;
  let w = (ls - l0) / (oblateBand(oblateBody, lower + 1u).y - l0);
  return thickMix(
    thickLayerAt(lower * layers + k, row, muSun, muTravel),
    thickLayerAt((lower + 1u) * layers + k, row, muSun, muTravel),
    w,
  );
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

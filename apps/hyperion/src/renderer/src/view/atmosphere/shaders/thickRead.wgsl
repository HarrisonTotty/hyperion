// The thick multiple-scattering table's read (plan R08, R08.T14.c and R08.T14.f; Design notes 9 and
// 10; `thick/read.ts`, its packing and CPU twin): J_ms from the bake with the polarisation
// correction summed into it, in place of Hillaire's sigma_s Psi_ms p_u, or the correction alone
// beside it, for a body whose tables hold one. This file holds the table and one layer's read;
// `thickReadBands.wgsl` the march's read over the bands. Appended with it to the sky-view,
// aerial-perspective and march kernels by `hillaire.ts`'s withThickRead, which points their sources'
// multiple scattering there and leaves every other line as it is, and with `irradianceThick.wgsl` to
// the sky-irradiance kernel by `tables.ts`'s IRRADIANCE_THICK_KERNEL (R08.T14.d.4).
//
// The table, `rgba32uint`, one layer a band and light slot (band b's slot k at b x slots + k): 16
// texels across, one a travel cosine mu_v = (v + 1/2) / 8 - 1, and rows a x 46 + s, altitude a of 33
// and sun cosine s of 46, then the info row. The sun cosines are 14 rows below the horizontal,
// mu0 = -(13.5 - s) / 64 for s = 0 to 13, to the cell edge -7/32, and 32 above it,
// mu0 = (s - 13.5) / 32 for s = 14 to 45 (R08.T14.d.2). A texel holds six halves in its first three
// words: J0's red and green, J0's blue and G's red, G's green and blue, each word's first half its
// low one; and G2 in its fourth, three 9-bit two's-complement mantissas (red in bits 0 to 8) and
// their shared exponent e in bits 27 to 31, each m 2^(e - 24). J0, G and G2 are J^0,
// J^1 / (sin theta_v sin theta0) and J^2 / (sin theta_v sin theta0)^2, J^m the source's m-th
// azimuthal coefficient, m = 0 to 2, per steradian and per unit of the kernels' scattering
// sigma_s,k at the altitude (the bake's J with the correction's summed in, or the correction's
// alone), so that the read, J0 + G h + G2 (2 h^2 - (1 - mu_v^2)(1 - mu0^2)) with
// h = v.s - (v.n)(s.n), is per steradian and per unit of scattering, as Psi_ms is. Layer 0's info row
// holds the altitudes, h* above the ground in m, as f32 bits in words 0 to 32, the light slots a band
// in word 33, and the table's kind in word 34: 0 the bake's (`thick`), 1 the correction alone
// (`correction`), a thin world's.
@group(0) @binding(24) var thickTable : texture_2d_array<u32>;

const THICK_ALTITUDES : u32 = 33u;
const THICK_SUN_COSINES : u32 = 46u;
const THICK_SUN_COSINES_BELOW : u32 = 14u;
const THICK_SUN_ROWS_A_UNIT_BELOW : f32 = 64.0;
const THICK_SUN_ROWS_A_UNIT_ABOVE : f32 = 32.0;
const THICK_TRAVEL_COSINES : u32 = 16u;
const THICK_INFO_ROW : u32 = 1518u;
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
// bake's table: all of it below the rows' edge, -7/32, none from -6/32 up, and linear across that
// cell, 1 - clamp(32 mu0 + 7, 0, 1); the table takes the rest.
fn thickAnalyticShare(muSun : f32) -> f32 {
  return 1.0 - clamp(muSun * 32.0 + 7.0, 0.0, 1.0);
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

// The step in mu0 between two rows below mu0 = 1/64, at share `t` to `b`: in each channel where both
// rows hold J0 > 0, J0 = J0_a^(1 - t) J0_b^t, by log2 and exp2, with G and G2 linear as their ratios
// to J0; linear in all three where either does not (`thick/read.ts`'s addGeometric).
fn thickGeometricMix(a : ThickSource, b : ThickSource, t : f32) -> ThickSource {
  let linear = thickMix(a, b, t);
  let both = (a.level > vec3f(0.0)) & (b.level > vec3f(0.0));
  let la = select(vec3f(1.0), a.level, both);
  let lb = select(vec3f(1.0), b.level, both);
  let level = exp2((1.0 - t) * log2(la) + t * log2(lb));
  return ThickSource(
    select(linear.level, level, both),
    select(linear.slope, level * ((1.0 - t) * a.slope / la + t * b.slope / lb), both),
    select(linear.curve, level * ((1.0 - t) * a.curve / la + t * b.curve / lb), both),
  );
}

// Where the sun cosine `muSun` lies among the table's rows, as a row and its share to the next, held
// in [0, 45] (`thick/read.ts`'s thickSunPlace): 14 + 32 mu0 - 1/2 from the first row above the
// horizontal, 1/64, up; 13 + (mu0 + 1/128) / (3/128) across the cell from the last row below it,
// -1/128; and 13 + 64 (mu0 + 1/128) below that.
fn thickSunPlace(muSun : f32) -> f32 {
  let firstAbove = 0.5 / THICK_SUN_ROWS_A_UNIT_ABOVE;
  let lastBelow = -0.5 / THICK_SUN_ROWS_A_UNIT_BELOW;
  let below = f32(THICK_SUN_COSINES_BELOW - 1u);
  if (muSun >= firstAbove) {
    return min(
      f32(THICK_SUN_COSINES_BELOW) + muSun * THICK_SUN_ROWS_A_UNIT_ABOVE - 0.5,
      f32(THICK_SUN_COSINES - 1u),
    );
  }
  if (muSun >= lastBelow) {
    return below + (muSun - lastBelow) / (firstAbove - lastBelow);
  }
  return max(below + (muSun - lastBelow) * THICK_SUN_ROWS_A_UNIT_BELOW, 0.0);
}

// The step in mu0 between two rows at share `t` to `b`: geometric below mu0 = 1/64 (`below`,
// thickGeometricMix), linear from it up.
fn thickSunMix(a : ThickSource, b : ThickSource, t : f32, below : bool) -> ThickSource {
  if (below) {
    return thickGeometricMix(a, b, t);
  }
  return thickMix(a, b, t);
}

// One layer's J0, G and G2 at `row`, sun cosine `muSun` and travel cosine `muTravel` (positive upward:
// for a view along v, -(v.n)): linear in each between the cells' centres about it, held at the
// table's edges, but below mu0 = 1/64 geometric in mu0 at each altitude and travel cosine about it
// (thickSunMix), then linear in the two (`thick/read.ts`'s addThickLayer). Its eight texels are
// loaded once whichever the step.
fn thickLayerAt(layer : u32, row : ThickRow, muSun : f32, muTravel : f32) -> ThickSource {
  let x = thickSunPlace(muSun);
  let s0 = min(u32(x), THICK_SUN_COSINES - 2u);
  let fs = x - f32(s0);
  let below = s0 < THICK_SUN_COSINES_BELOW;
  let y = clamp(
    (muTravel + 1.0) * f32(THICK_TRAVEL_COSINES / 2u) - 0.5, 0.0, f32(THICK_TRAVEL_COSINES - 1u)
  );
  let v0 = min(u32(y), THICK_TRAVEL_COSINES - 2u);
  let fv = y - f32(v0);
  let a0 = row.lower;
  let a1 = a0 + 1u;
  let lower = thickMix(
    thickSunMix(thickTexel(layer, a0, s0, v0), thickTexel(layer, a0, s0 + 1u, v0), fs, below),
    thickSunMix(
      thickTexel(layer, a0, s0, v0 + 1u), thickTexel(layer, a0, s0 + 1u, v0 + 1u), fs, below
    ),
    fv,
  );
  let upper = thickMix(
    thickSunMix(thickTexel(layer, a1, s0, v0), thickTexel(layer, a1, s0 + 1u, v0), fs, below),
    thickSunMix(
      thickTexel(layer, a1, s0, v0 + 1u), thickTexel(layer, a1, s0 + 1u, v0 + 1u), fs, below
    ),
    fv,
  );
  return thickMix(lower, upper, row.t);
}

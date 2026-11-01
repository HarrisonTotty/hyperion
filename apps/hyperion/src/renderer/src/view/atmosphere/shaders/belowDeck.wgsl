// The table below a cloud deck's read (plan R08, R08.T15.b.2; Design note 9; `thick/deckRead.ts`,
// "Below the deck", its packing and CPU twin): beneath a deck over a surface, the whole column's
// plane-parallel field at the deck's 17 altitudes from the ground to the split, read in place of
// every table of the upper medium by the sky view, aerial perspective and march kernels beneath a
// deck, and by the surface's lighting there (`surfaceLighting.wgsl`). Composed after `common.wgsl`
// and `oblate.wgsl`; it declares no binding, its callers passing the table and the body's table.
//
// The table, `rgba32float`, one layer a band and light slot (band b's slot k at b x slots + k): 50
// texels across, row a x 32 + s for altitude a and sun cosine (s + 1/2) / 32, holding at texel
// v x 3 + m mode m's I^m / (sin^m theta_v sin^m theta0) at travel cosine (v + 1/2) / 8 - 1,
// m = 0 ... 2, at texel 48 the downward irradiance the kernels' beam leaves out and at 49 the
// kernels' beam's transmission from the top, red, green and blue in the first three values; then the
// info rows, 544, at texel a the kernels' vertical optical depth from the top per channel and the
// altitude's h* in the fourth, and 545, at texel a their extinction per unit of h*. A one-texel stub
// holds no table. Each read is
// linear between the cells' centres in h*, mu0 and mu, held at the table's edges, and in ln s
// between the bands about the point's s, the nearer band beyond them; under the first sun row
// (1/64) the field and the sky's irradiance are scaled by mu0 / (1/64), and nothing lights the
// ground below the horizontal. The radiance sums its modes with the Chebyshev factors c^m of the
// view's and the source's horizontal parts (c^0 = 1, c^1 = h, c^2 = 2 h^2 - (1 - mu^2)(1 - mu0^2)),
// held at 0; the beam is geometric in 1 / mu0 and then in h* where both values of a step hold light,
// else linear, the edge row's plane-parallel power beyond the rows and above a row of 0.

const BELOW_DECK_ALTITUDES : u32 = 17u;
const BELOW_DECK_SUN_COSINES : u32 = 32u;
const BELOW_DECK_TRAVEL_COSINES : u32 = 16u;
const BELOW_DECK_MODES : u32 = 3u;
const BELOW_DECK_IRRADIANCE_TEXEL : u32 = 48u;
const BELOW_DECK_DIRECT_TEXEL : u32 = 49u;
const BELOW_DECK_TABLE_WIDTH : u32 = 50u;
const BELOW_DECK_INFO_ROW : u32 = 544u;
const BELOW_DECK_EXTINCTION_ROW : u32 = 545u;

// Texel (x, y) of `layer`: its four values.
fn belowDeckTexel(table : texture_2d_array<f32>, layer : u32, x : u32, y : u32) -> vec4f {
  return textureLoad(table, vec2u(x, y), layer, 0);
}

// Whether `table` holds a deck's table below it: a one-texel stub holds none.
fn belowDeckHeld(table : texture_2d_array<f32>) -> bool {
  return textureDimensions(table).x >= BELOW_DECK_TABLE_WIDTH;
}

// The split's h* of a table below a deck: layer 0's last altitude; 0 where none is held.
fn belowDeckSplitOf(table : texture_2d_array<f32>) -> f32 {
  if (!belowDeckHeld(table)) {
    return 0.0;
  }
  return belowDeckTexel(table, 0u, BELOW_DECK_ALTITUDES - 1u, BELOW_DECK_INFO_ROW).w;
}

// A place among a table's rows: the row below and its share to the next.
struct BelowDeckPlace {
  i0 : u32,
  f : f32,
}

// Where `heightStarM` lies among a layer's altitudes, held at the ground and at the split.
fn belowDeckAltitudePlace(
  table : texture_2d_array<f32>,
  layer : u32,
  heightStarM : f32,
) -> BelowDeckPlace {
  var lower = 0u;
  while (
    lower + 2u < BELOW_DECK_ALTITUDES
      && belowDeckTexel(table, layer, lower + 1u, BELOW_DECK_INFO_ROW).w <= heightStarM
  ) {
    lower++;
  }
  let h0 = belowDeckTexel(table, layer, lower, BELOW_DECK_INFO_ROW).w;
  let h1 = belowDeckTexel(table, layer, lower + 1u, BELOW_DECK_INFO_ROW).w;
  return BelowDeckPlace(lower, clamp((heightStarM - h0) / (h1 - h0), 0.0, 1.0));
}

// Where `muSun` lies among the sun cosines, x = 32 mu0 - 1/2 held in [0, 31].
fn belowDeckSunPlace(muSun : f32) -> BelowDeckPlace {
  let x = clamp(muSun * f32(BELOW_DECK_SUN_COSINES) - 0.5, 0.0, f32(BELOW_DECK_SUN_COSINES - 1u));
  let s0 = min(u32(x), BELOW_DECK_SUN_COSINES - 2u);
  return BelowDeckPlace(s0, x - f32(s0));
}

// The diffuse field's scale at a low sun: mu0 / (1/64) under the first row, 0 at and below the
// horizontal.
fn belowDeckLowSunScale(muSun : f32) -> f32 {
  return clamp(muSun * 2.0 * f32(BELOW_DECK_SUN_COSINES), 0.0, 1.0);
}

// Where `muTravel` lies among the travel cosines, y = 8 (mu + 1) - 1/2 held in [0, 15].
fn belowDeckTravelPlace(muTravel : f32) -> BelowDeckPlace {
  let half = f32(BELOW_DECK_TRAVEL_COSINES / 2u);
  let y = clamp((muTravel + 1.0) * half - 0.5, 0.0, f32(BELOW_DECK_TRAVEL_COSINES - 1u));
  let v0 = min(u32(y), BELOW_DECK_TRAVEL_COSINES - 2u);
  return BelowDeckPlace(v0, y - f32(v0));
}

// The band layers about the gravity ratio `s` for light slot `slot`, the nearest slot held where the
// table has fewer: the lower's layer, the upper's and the upper's weight, linear in ln s between the
// bands about s and the nearer band beyond them (`thickReadBands.wgsl`'s thickBandsAt).
struct BelowDeckLayers {
  lower : u32,
  upper : u32,
  w : f32,
}

fn belowDeckLayersAt(
  table : texture_2d_array<f32>,
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  s : f32,
  slot : u32,
) -> BelowDeckLayers {
  let bands = oblateBandCount(body);
  let slotCount = max(textureNumLayers(table) / bands, 1u);
  let k = min(slot, slotCount - 1u);
  let last = bands - 1u;
  var lower = 0u;
  var w = 0.0;
  if (last > 0u) {
    let ls = log(s);
    if (!(ls > oblateBand(bodyTable, 0u).y)) {
      lower = 0u;
    } else if (!(ls < oblateBand(bodyTable, last).y)) {
      lower = last;
    } else {
      while (lower + 1u < last && oblateBand(bodyTable, lower + 1u).y <= ls) {
        lower++;
      }
      let l0 = oblateBand(bodyTable, lower).y;
      w = (ls - l0) / (oblateBand(bodyTable, lower + 1u).y - l0);
    }
  }
  return BelowDeckLayers(lower * slotCount + k, min(lower + 1u, last) * slotCount + k, w);
}

// One layer's sum_m I^m c^m at a point, trilinear over the cells' centres about it.
fn belowDeckLayerRadiance(
  table : texture_2d_array<f32>,
  layer : u32,
  heightStarM : f32,
  sun : BelowDeckPlace,
  travel : BelowDeckPlace,
  c1 : f32,
  c2 : f32,
) -> vec3f {
  let altitude = belowDeckAltitudePlace(table, layer, heightStarM);
  var sum = vec3f(0.0);
  for (var corner = 0u; corner < 8u; corner++) {
    let da = corner / 4u;
    let ds = (corner / 2u) % 2u;
    let dv = corner % 2u;
    let w = select(1.0 - altitude.f, altitude.f, da == 1u) * select(1.0 - sun.f, sun.f, ds == 1u)
      * select(1.0 - travel.f, travel.f, dv == 1u);
    let row = (altitude.i0 + da) * BELOW_DECK_SUN_COSINES + sun.i0 + ds;
    let x0 = (travel.i0 + dv) * BELOW_DECK_MODES;
    sum += w * (belowDeckTexel(table, layer, x0, row).xyz
      + c1 * belowDeckTexel(table, layer, x0 + 1u, row).xyz
      + c2 * belowDeckTexel(table, layer, x0 + 2u, row).xyz);
  }
  return sum;
}

// The diffuse radiance beneath a deck for one source, per unit of the source's weight, sr^-1 per
// unit of its irradiance at the top (`thick/deckRead.ts`'s belowDeckRadianceInto): at h*
// `heightStarM`, the source's cosine `muSun` against the zenith there, the light's travel cosine
// `muTravel`, -(v.n), and `horizontal` = v.s - (v.n)(s.n).
fn belowDeckRadianceAt(
  table : texture_2d_array<f32>,
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  s : f32,
  heightStarM : f32,
  muSun : f32,
  muTravel : f32,
  horizontal : f32,
  slot : u32,
) -> vec3f {
  let scale = belowDeckLowSunScale(muSun);
  if (!(scale > 0.0)) {
    return vec3f(0.0);
  }
  let crossed = (1.0 - muTravel * muTravel) * (1.0 - muSun * muSun);
  let c2 = 2.0 * horizontal * horizontal - crossed;
  let sun = belowDeckSunPlace(muSun);
  let travel = belowDeckTravelPlace(muTravel);
  let layers = belowDeckLayersAt(table, body, bodyTable, s, slot);
  var sum = belowDeckLayerRadiance(table, layers.lower, heightStarM, sun, travel, horizontal, c2);
  if (layers.w > 0.0) {
    let upper = belowDeckLayerRadiance(table, layers.upper, heightStarM, sun, travel, horizontal, c2);
    sum = sum * (1.0 - layers.w) + upper * layers.w;
  }
  return max(scale * sum, vec3f(0.0));
}

// The light on a horizontal surface beneath a deck for one source, per unit of its irradiance normal
// to its beam at the top: the sky's irradiance and the kernels' beam's transmission.
struct BelowDeckLight {
  sky : vec3f,
  sun : vec3f,
}

// One step of the beam's read between two values: geometric at share `f` where both hold light,
// else linear at share `linear`.
fn belowDeckBeamStep(t0 : f32, t1 : f32, f : f32, linear : f32) -> f32 {
  if (t0 > 0.0 && t1 > 0.0) {
    return exp((1.0 - f) * log(t0) + f * log(t1));
  }
  return (1.0 - linear) * t0 + linear * t1;
}

// One layer's light at a point (`thick/deckRead.ts`'s belowDeckLightInto), mu0 above 0.
fn belowDeckLayerLight(
  table : texture_2d_array<f32>,
  layer : u32,
  heightStarM : f32,
  muSun : f32,
) -> BelowDeckLight {
  let altitude = belowDeckAltitudePlace(table, layer, heightStarM);
  let place = belowDeckSunPlace(muSun);
  let scale = belowDeckLowSunScale(muSun);
  let first = 0.5 / f32(BELOW_DECK_SUN_COSINES);
  let last = (f32(BELOW_DECK_SUN_COSINES) - 0.5) / f32(BELOW_DECK_SUN_COSINES);
  let beyond = muSun < first || muSun > last;
  let mu0 = (f32(place.i0) + 0.5) / f32(BELOW_DECK_SUN_COSINES);
  let mu1 = (f32(place.i0) + 1.5) / f32(BELOW_DECK_SUN_COSINES);
  let power = select(last, first, muSun < first) / muSun;
  let fs = (1.0 / muSun - 1.0 / mu0) / (1.0 / mu1 - 1.0 / mu0);
  var sky = vec3f(0.0);
  // The beam by altitude row, per channel.
  var rows = array<vec3f, 2>(vec3f(0.0), vec3f(0.0));
  for (var da = 0u; da < 2u; da++) {
    let wa = select(1.0 - altitude.f, altitude.f, da == 1u);
    let lowerRow = (altitude.i0 + da) * BELOW_DECK_SUN_COSINES + place.i0;
    sky += wa * (1.0 - place.f) * belowDeckTexel(table, layer, BELOW_DECK_IRRADIANCE_TEXEL, lowerRow).xyz
      + wa * place.f * belowDeckTexel(table, layer, BELOW_DECK_IRRADIANCE_TEXEL, lowerRow + 1u).xyz;
    let lower = belowDeckTexel(table, layer, BELOW_DECK_DIRECT_TEXEL, lowerRow).xyz;
    let upper = belowDeckTexel(table, layer, BELOW_DECK_DIRECT_TEXEL, lowerRow + 1u).xyz;
    var beam = vec3f(0.0);
    for (var c = 0u; c < 3u; c++) {
      if (beyond) {
        let held = select(upper[c], lower[c], muSun < first);
        beam[c] = select(held, pow(held, power), held > 0.0);
      } else if (lower[c] == 0.0 && upper[c] > 0.0) {
        // A lower row that the f32 holds as 0 under a lit one: the lit row's plane-parallel power.
        beam[c] = pow(upper[c], mu1 / muSun);
      } else {
        beam[c] = belowDeckBeamStep(lower[c], upper[c], fs, place.f);
      }
    }
    rows[da] = beam;
  }
  var sun = vec3f(0.0);
  for (var c = 0u; c < 3u; c++) {
    sun[c] = belowDeckBeamStep(rows[0][c], rows[1][c], altitude.f, altitude.f);
  }
  return BelowDeckLight(scale * sky, sun);
}

// The light on a horizontal surface beneath a deck for one source (`thick/deckRead.ts`'s
// belowDeckLightInto): none at and below the horizontal.
fn belowDeckLightAt(
  table : texture_2d_array<f32>,
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  s : f32,
  heightStarM : f32,
  muSun : f32,
  slot : u32,
) -> BelowDeckLight {
  if (!(muSun > 0.0)) {
    return BelowDeckLight(vec3f(0.0), vec3f(0.0));
  }
  let layers = belowDeckLayersAt(table, body, bodyTable, s, slot);
  var light = belowDeckLayerLight(table, layers.lower, heightStarM, muSun);
  if (layers.w > 0.0) {
    let upper = belowDeckLayerLight(table, layers.upper, heightStarM, muSun);
    light = BelowDeckLight(
      light.sky * (1.0 - layers.w) + upper.sky * layers.w,
      light.sun * (1.0 - layers.w) + upper.sun * layers.w,
    );
  }
  return light;
}

// One layer's vertical optical depth from the top at h*, linear between the altitudes.
fn belowDeckLayerDepth(table : texture_2d_array<f32>, layer : u32, heightStarM : f32) -> vec3f {
  let altitude = belowDeckAltitudePlace(table, layer, heightStarM);
  return (1.0 - altitude.f) * belowDeckTexel(table, layer, altitude.i0, BELOW_DECK_INFO_ROW).xyz
    + altitude.f * belowDeckTexel(table, layer, altitude.i0 + 1u, BELOW_DECK_INFO_ROW).xyz;
}

// The kernels' vertical optical depth from the top to h* beneath a deck, per channel
// (`thick/deckRead.ts`'s belowDeckOpticalDepthInto).
fn belowDeckDepthAt(
  table : texture_2d_array<f32>,
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  s : f32,
  heightStarM : f32,
) -> vec3f {
  let layers = belowDeckLayersAt(table, body, bodyTable, s, 0u);
  var depth = belowDeckLayerDepth(table, layers.lower, heightStarM);
  if (layers.w > 0.0) {
    depth = depth * (1.0 - layers.w) + belowDeckLayerDepth(table, layers.upper, heightStarM) * layers.w;
  }
  return depth;
}

// The integral of sigma over [x0, x1] inside an altitude interval [h0, h1] whose extinction runs
// from s0 to s1, geometric in h where both are above 0, else linear (`thick/deckRead.ts`'s
// intervalDepth).
fn belowDeckIntervalDepth(h0 : f32, h1 : f32, s0 : f32, s1 : f32, x0 : f32, x1 : f32) -> f32 {
  let span = h1 - h0;
  let f0 = (x0 - h0) / span;
  let f1 = (x1 - h0) / span;
  if (s0 > 0.0 && s1 > 0.0) {
    let lr = log(s1 / s0);
    if (abs(lr) > 1e-6) {
      return s0 * span * (exp(lr * f1) - exp(lr * f0)) / lr;
    }
  }
  return span * (s0 * (f1 - f0) + 0.5 * (s1 - s0) * (f1 * f1 - f0 * f0));
}

// The extinction at a share `f` of an interval from s0 to s1: geometric where both are above 0.
fn belowDeckExtinctionAt(s0 : f32, s1 : f32, f : f32) -> f32 {
  if (s0 > 0.0 && s1 > 0.0) {
    return s0 * exp(f * log(s1 / s0));
  }
  return (1.0 - f) * s0 + f * s1;
}

// One layer's mean of the kernels' extinction sigma* (the second info row) between h* `lowM` and
// `highM`, read geometrically between the altitudes; sigma*(lowM) where they lie within a
// millimetre.
fn belowDeckLayerExtinction(
  table : texture_2d_array<f32>,
  layer : u32,
  lowM : f32,
  highM : f32,
) -> vec3f {
  var sigma = vec3f(0.0);
  if (!(highM - lowM > 1e-3)) {
    let place = belowDeckAltitudePlace(table, layer, lowM);
    let lower = belowDeckTexel(table, layer, place.i0, BELOW_DECK_EXTINCTION_ROW).xyz;
    let upper = belowDeckTexel(table, layer, place.i0 + 1u, BELOW_DECK_EXTINCTION_ROW).xyz;
    for (var c = 0u; c < 3u; c++) {
      sigma[c] = belowDeckExtinctionAt(lower[c], upper[c], place.f);
    }
    return sigma;
  }
  for (var a = 0u; a + 1u < BELOW_DECK_ALTITUDES; a++) {
    let h0 = belowDeckTexel(table, layer, a, BELOW_DECK_INFO_ROW).w;
    let h1 = belowDeckTexel(table, layer, a + 1u, BELOW_DECK_INFO_ROW).w;
    let x0 = max(lowM, h0);
    let x1 = min(highM, h1);
    if (!(x1 > x0)) {
      continue;
    }
    let lower = belowDeckTexel(table, layer, a, BELOW_DECK_EXTINCTION_ROW).xyz;
    let upper = belowDeckTexel(table, layer, a + 1u, BELOW_DECK_EXTINCTION_ROW).xyz;
    for (var c = 0u; c < 3u; c++) {
      sigma[c] += belowDeckIntervalDepth(h0, h1, lower[c], upper[c], x0, x1);
    }
  }
  return sigma / (highM - lowM);
}

// The optical depth of a straight path of `distanceM` beneath a deck from h* `fromM` to h* `toM`
// (`thick/deckRead.ts`'s belowDeckPathDepthInto): d s times the mean of sigma* between the two.
fn belowDeckPathDepth(
  table : texture_2d_array<f32>,
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  s : f32,
  fromM : f32,
  toM : f32,
  distanceM : f32,
) -> vec3f {
  let lowM = min(fromM, toM);
  let highM = max(fromM, toM);
  let layers = belowDeckLayersAt(table, body, bodyTable, s, 0u);
  var sigma = belowDeckLayerExtinction(table, layers.lower, lowM, highM);
  if (layers.w > 0.0) {
    sigma = sigma * (1.0 - layers.w)
      + belowDeckLayerExtinction(table, layers.upper, lowM, highM) * layers.w;
  }
  return distanceM * s * sigma;
}

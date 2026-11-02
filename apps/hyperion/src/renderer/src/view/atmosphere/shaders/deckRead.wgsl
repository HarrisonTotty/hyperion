// The cloud deck's read (plan R08, R08.T15.a.2; Design notes 9 and 17; `thick/deckRead.ts`, its
// packing and CPU twin): at a ray's hit on the deck, the upper medium's ground, each source's light
// reflected by the deck in place of the bare ground's Lambertian term. Appended after
// `thickRead.wgsl` to the ray march's thick kernels by `hillaire.ts`'s withDeckRead, which points
// the march's bare-ground term here and leaves every other line as it is, so that a body with no
// deck keeps its kernels to the byte.
//
// The table, `rgba32uint`, one layer a band and light slot (band b's slot k at b x slots + k), each
// texel's first three words three f32 values' bits (red, green, blue): 208 texels across, eight cells
// of 26 a row, cell v (the upward view cosine mu_v = (v + 1/2) / 8) of row s (the thick table's sun
// cosines, its 46 rows) at texels v x 26 to v x 26 + 25: the eight modes of rho - rho_ss, the
// multiple-scattering part of the deck's reflectance, then the eight of beta rho, the same weighted
// by the bake's own beam at the deck, then the eight of D, the deck's sky-lit radiance (0 past its
// modes), each stored over sin^m theta_v sin^m theta0 at the cell's centre; then G and G_beta, the
// exact single scattering's geometry factor and its beam's. After the 46 rows the phase p(Theta) per
// channel, sr^-1, its 256 entries even in u = sqrt(theta / pi), entry i at texel (i mod 208,
// 46 + i / 208). Per unit of the source's weight the deck sends up
// (max(mu0, 0) / pi)(sum_m beta rho^m c^m + G_beta p(Theta)) + w(mu0) sum_m D^m c^m, held at 0, with
// c^m = sin^m theta_v sin^m theta0 cos(m dphi) by Chebyshev's recurrence from the horizontal parts'
// dot product h = v.s - (v.n)(s.n) (c^0 = 1, c^1 = h, c^(m+1) = 2 h c^m - (1 - mu_v^2)(1 - mu0^2)
// c^(m-1)), cos Theta = s.v, and w(mu0) = clamp(32 mu0 + 7, 0, 1), the thick table's share. A source
// past the table's light slots reads rho - rho_ss and G times the kernels' own transmittance in place
// of beta rho and G_beta, and the last slot's D, as the thick read takes the last slot's J. Every
// texel's fourth word is 0 but each layer's first texel's, 1 where the deck lies over an envelope's
// floor (`thick/deckRead.ts`'s DECK_FLOOR_WORD), which no kernel reads since R08.T16.a.2's shares by
// the light each piece returns (`rayMarch.wgsl`'s rayMarchDepthShares).
@group(0) @binding(25) var deckTable : texture_2d_array<u32>;

const DECK_TABLE_MODES : u32 = 8u;
const DECK_TABLE_VIEW_COSINES : u32 = 8u;
const DECK_TABLE_PHASE_ENTRIES : u32 = 256u;
const DECK_TEXELS_A_CELL : u32 = 26u;
const DECK_CELL_REFLECTANCE : u32 = 0u;
const DECK_CELL_BEAM : u32 = 8u;
const DECK_CELL_DIFFUSE : u32 = 16u;
const DECK_CELL_SINGLE : u32 = 24u;
const DECK_CELL_SINGLE_BEAM : u32 = 25u;
const DECK_TABLE_WIDTH : u32 = 208u;
const DECK_PHASE_ROW : u32 = 46u;
// Texel (x, y) of `layer`: its three f32 values.
fn deckTexel(layer : u32, x : u32, y : u32) -> vec3f {
  return bitcast<vec3f>(textureLoad(deckTable, vec2u(x, y), layer, 0).xyz);
}

// One layer's sums at a point: the beam's and the sky-lit radiance's.
struct DeckLight {
  beam : vec3f,
  sky : vec3f,
}

// One layer's beam and sky-lit series at a point (`thick/deckRead.ts`'s addDeckLayer): the beam's
// sum_m X^m c^m + G p(Theta) of the beam's set (beta rho and G_beta where `carried`, else rho - rho_ss
// and G), its rows held at the first above the horizontal, and the sky-lit sum_m D^m c^m, each
// bilinear over the cells' centres about the point; `phase` is u = sqrt(Theta / pi).
fn deckLayerAt(
  layer : u32,
  muSun : f32,
  muTravel : f32,
  horizontal : f32,
  crossed : f32,
  phase : f32,
  carried : bool,
) -> DeckLight {
  let place = thickSunPlace(muSun);
  let litPlace = max(place, f32(THICK_SUN_COSINES_BELOW));
  let lit0 = min(u32(litPlace), THICK_SUN_COSINES - 2u);
  let litShare = litPlace - f32(lit0);
  let all0 = min(u32(place), THICK_SUN_COSINES - 2u);
  let allShare = place - f32(all0);
  let y = clamp(
    muTravel * f32(DECK_TABLE_VIEW_COSINES) - 0.5, 0.0, f32(DECK_TABLE_VIEW_COSINES - 1u)
  );
  let v0 = min(u32(y), DECK_TABLE_VIEW_COSINES - 2u);
  let fv = y - f32(v0);
  let series = select(DECK_CELL_REFLECTANCE, DECK_CELL_BEAM, carried);
  let single = select(DECK_CELL_SINGLE, DECK_CELL_SINGLE_BEAM, carried);
  let u = phase * f32(DECK_TABLE_PHASE_ENTRIES - 1u);
  let i0 = min(u32(u), DECK_TABLE_PHASE_ENTRIES - 2u);
  let fu = u - f32(i0);
  let i1 = i0 + 1u;
  let p = deckTexel(layer, i0 % DECK_TABLE_WIDTH, DECK_PHASE_ROW + i0 / DECK_TABLE_WIDTH) * (1.0 - fu)
    + deckTexel(layer, i1 % DECK_TABLE_WIDTH, DECK_PHASE_ROW + i1 / DECK_TABLE_WIDTH) * fu;
  var beam = vec3f(0.0);
  var sky = vec3f(0.0);
  for (var corner = 0u; corner < 4u; corner++) {
    let ds = corner / 2u;
    let dv = corner % 2u;
    let wv = select(1.0 - fv, fv, dv == 1u);
    let beamWeight = select(1.0 - litShare, litShare, ds == 1u) * wv;
    let skyWeight = select(1.0 - allShare, allShare, ds == 1u) * wv;
    let x0 = (v0 + dv) * DECK_TEXELS_A_CELL;
    let beamRow = lit0 + ds;
    let skyRow = all0 + ds;
    var previous = 0.0;
    var current = 1.0;
    for (var m = 0u; m < DECK_TABLE_MODES; m++) {
      beam += beamWeight * current * deckTexel(layer, x0 + series + m, beamRow);
      sky += skyWeight * current * deckTexel(layer, x0 + DECK_CELL_DIFFUSE + m, skyRow);
      var next = horizontal;
      if (m > 0u) {
        next = 2.0 * horizontal * current - crossed * previous;
      }
      previous = current;
      current = next;
    }
    beam += beamWeight * deckTexel(layer, x0 + single, beamRow) * p;
  }
  return DeckLight(beam, sky);
}

// The deck's light at a ray's hit on it for one source, per unit of the source's weight, sr^-1 per
// unit of its irradiance (`thick/deckRead.ts`'s deckGroundInto): `n` the deck's unit normal at the
// hit, `dir` the ray's unit direction, `sun` the source's, `s` the gravity ratio at the hit, `slot`
// the source's light slot and `toSun` the kernels' own transmittance to the hit from it. The bands
// about s are read as `thickRead.wgsl`'s thickBandsAt reads them, linear in ln s, the nearer band
// beyond them, two reads of a layer at most.
fn deckGroundAt(
  body : OblateBody,
  s : f32,
  n : vec3f,
  dir : vec3f,
  sun : vec3f,
  slot : u32,
  toSun : vec3f,
) -> vec3f {
  let muSun = dot(sun, n);
  let muView = dot(dir, n);
  let muTravel = -muView;
  let horizontal = dot(dir, sun) - muView * muSun;
  let crossed = (1.0 - muTravel * muTravel) * (1.0 - muSun * muSun);
  let phase = sqrt(acos(clamp(dot(sun, dir), -1.0, 1.0)) / PI_VIEW);
  let slots = thickSlotCount();
  let carried = slot < slots;
  let k = min(slot, slots - 1u);
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
  var light = deckLayerAt(lower * slots + k, muSun, muTravel, horizontal, crossed, phase, carried);
  if (w > 0.0) {
    let upper = deckLayerAt(
      (lower + 1u) * slots + k, muSun, muTravel, horizontal, crossed, phase, carried
    );
    light = DeckLight(
      light.beam * (1.0 - w) + upper.beam * w, light.sky * (1.0 - w) + upper.sky * w
    );
  }
  let direct = max(muSun, 0.0) / PI_VIEW;
  let share = clamp(muSun * 32.0 + 7.0, 0.0, 1.0);
  let transmitted = select(toSun, vec3f(1.0), carried);
  return max(direct * transmitted * light.beam + share * light.sky, vec3f(0.0));
}

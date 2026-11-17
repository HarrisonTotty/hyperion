// A cloud deck's thermal light (plan R08, R08.T15.b.2; `thermalSource.ts`'s DeckThermal, its
// packing and CPU reads): appended to the deck's marches' thermal forms and to the kernels beneath a
// deck by `hillaire.ts`, after the binding of `thermalSource`. Its reads search nothing, so that the
// marches compile on SwiftShader, whose GPU process crashed on them with `thermalSource.wgsl`'s
// search of the altitudes, and again with a search of the bands and a loop over two (2026-11-02):
// the section holds one band, a body of one band's, or none.
//
// The section follows `thermalSource.wgsl`'s bands: its header (x the upper medium's top above the
// split, m; y the rows; z 1, or 0 for a deck without its thermal light; w the table below's
// altitudes, 0 for an envelope), then the band's rows of J_th evenly in h* from the split to the
// top, cd m^-2 per unit of the kernels' scattering, its glow at its top, cd m^-2, and its field
// below, cd m^-2, rgb, each at the 16 travel cosines mu = (v + 1/2) / 8 - 1.

const THERMAL_DECK_TRAVEL_COSINES : u32 = 16u;

// The section's header, at the end of the bands of `thermalSource.wgsl`'s scattered light.
fn thermalDeckBase() -> u32 {
  let header = thermalSource[0];
  return 1u + u32(header.y) + u32(header.z) * u32(header.w) * (1u + THERMAL_DECK_TRAVEL_COSINES);
}

// One row's value of the part `offset` rows of 16 into the band, at row `row`, linear between the
// travel cosines about `muTravel`, held at the edges.
fn thermalDeckRow(offset : u32, row : u32, muTravel : f32) -> vec3f {
  let half = f32(THERMAL_DECK_TRAVEL_COSINES / 2u);
  let y = clamp((muTravel + 1.0) * half - 0.5, 0.0, f32(THERMAL_DECK_TRAVEL_COSINES - 1u));
  let v0 = min(u32(floor(y)), THERMAL_DECK_TRAVEL_COSINES - 2u);
  let g = y - f32(v0);
  let at = thermalDeckBase() + 1u + (offset + row) * THERMAL_DECK_TRAVEL_COSINES + v0;
  return thermalSource[at].rgb * (1.0 - g) + thermalSource[at + 1u].rgb * g;
}

// The deck's J_th at h* = `heightM` above the split for the travel cosine `muTravel`, cd m^-2 per
// unit of the kernels' scattering (`thermalSource.ts`'s deckScatteringInto): its rows evenly in h*
// from the split to the top, linear between them, held at the ends; 0 without it.
fn thermalDeckScatteringAt(heightM : f32, muTravel : f32) -> vec3f {
  let header = thermalSource[thermalDeckBase()];
  if (u32(header.z) == 0u) {
    return vec3f(0.0);
  }
  let last = u32(header.y) - 1u;
  let place = clamp(heightM / header.x, 0.0, 1.0) * f32(last);
  let r0 = min(u32(floor(place)), last - 1u);
  let f = place - f32(r0);
  return thermalDeckRow(0u, r0, muTravel) * (1.0 - f) + thermalDeckRow(0u, r0 + 1u, muTravel) * f;
}

// The deck's glow at its top for the light's travel cosine `muTravel`, cd m^-2
// (`thermalSource.ts`'s deckGlowInto); 0 without it.
fn thermalDeckGlowAt(muTravel : f32) -> vec3f {
  let header = thermalSource[thermalDeckBase()];
  if (u32(header.z) == 0u) {
    return vec3f(0.0);
  }
  return thermalDeckRow(u32(header.y), 0u, muTravel);
}

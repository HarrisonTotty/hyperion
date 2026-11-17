// The thermal field below a cloud deck over a surface (plan R08, R08.T15.b.2; `thermalSource.ts`'s
// deckBelowInto): appended after `thermalDeck.wgsl` to the kernels beneath a deck by `hillaire.ts`'s
// withBelowDeckThermal. Its rows are the table below's altitudes, found as that table's own reads
// find them (`belowDeck.wgsl`'s belowDeckAltitudePlace, over its first layer's).

// The field at h* = `heightStarM` above the ground and the light's travel cosine `muTravel`,
// cd m^-2; 0 for a section with no field below.
fn belowDeckThermalAt(heightStarM : f32, muTravel : f32) -> vec3f {
  let header = thermalSource[thermalDeckBase()];
  if (u32(header.z) == 0u || u32(header.w) == 0u) {
    return vec3f(0.0);
  }
  let place = belowDeckAltitudePlace(belowDeckTable, 0u, heightStarM);
  let offset = u32(header.y) + 1u;
  return thermalDeckRow(offset, place.i0, muTravel) * (1.0 - place.f)
    + thermalDeckRow(offset, place.i0 + 1u, muTravel) * place.f;
}

// 1 - e^(-tau) per channel: by its series under 0.045, where f32's 1 - e^(-tau) keeps few figures.
// There the series' remainder, tau^3 / 24 of it, meets the difference's error under WGSL's bound
// on exp, 3 ULP, both 4e-6.
fn belowDeckOneLess(tau : vec3f) -> vec3f {
  let series = tau * (1.0 - 0.5 * tau * (1.0 - tau / 3.0));
  return select(1.0 - exp(-tau), series, tau < vec3f(0.045));
}

// A path's thermal light beneath the deck from h* `fromM` to h* `toM` along the light's travel
// cosine `muTravel`, its optical depth `depth` per channel (`tablesCpu.ts`'s belowDeckPathTwin):
// I(h_camera) - T I(h_t) as (I(h_camera) - I(h_t)) + (1 - T) I(h_t), the difference formed from
// the rows' height weights, so that f32 keeps its figures where the two heights lie close; 0 for a
// section with no field below.
fn belowDeckThermalPath(fromM : f32, toM : f32, muTravel : f32, depth : vec3f) -> vec3f {
  let header = thermalSource[thermalDeckBase()];
  if (u32(header.z) == 0u || u32(header.w) == 0u) {
    return vec3f(0.0);
  }
  let offset = u32(header.y) + 1u;
  let camera = belowDeckAltitudePlace(belowDeckTable, 0u, fromM);
  let far = belowDeckAltitudePlace(belowDeckTable, 0u, toM);
  // Each height's field less the camera's lower row's.
  let b = camera.i0;
  let rb = thermalDeckRow(offset, b, muTravel);
  let rb1 = thermalDeckRow(offset, b + 1u, muTravel);
  let atCamera = camera.f * (rb1 - rb);
  var atTarget = vec3f(0.0);
  if (far.i0 == b) {
    atTarget = far.f * (rb1 - rb);
  } else if (far.i0 + 1u == b) {
    atTarget = (far.f - 1.0) * (rb - thermalDeckRow(offset, b - 1u, muTravel));
  } else if (far.i0 == b + 1u) {
    atTarget = (rb1 - rb) + far.f * (thermalDeckRow(offset, b + 2u, muTravel) - rb1);
  } else {
    atTarget = thermalDeckRow(offset, far.i0, muTravel) * (1.0 - far.f)
      + thermalDeckRow(offset, far.i0 + 1u, muTravel) * far.f - rb;
  }
  return (atCamera - atTarget) + belowDeckOneLess(depth) * (rb + atTarget);
}

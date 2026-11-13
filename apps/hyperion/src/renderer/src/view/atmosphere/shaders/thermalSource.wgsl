// The medium's own light (plan R08, R08.T18.b; `thermalSource.ts`, its packing and CPU twin):
// appended to the sky-view, aerial-perspective and march kernels by `hillaire.ts`'s
// withThermalSource, which gives each sample's medium its black body B~ at its h* times the frame's
// pre-exposure scale (`view.marchSky.w`), each step the emission of the depth it absorbs, and each
// step's slot 0 the scattered thermal light of a thick bake. Every other line is the kernel's own.
//
// The buffer, vec4f by vec4f: the header (x the medium's top, m; y the heights B~ is held at, evenly
// from 0 to the top; z the bands of scattered thermal light, 0 for none; w the altitudes a band);
// B~ at each height, cd m^-2, rgb; then each band's altitudes, h* above the ground in m, in x,
// followed by its J_th, cd m^-2 per unit of the kernels' scattering, altitude by altitude and travel
// cosine by travel cosine, mu = (v + 1/2) / 8 - 1 for v = 0 to 15, as the thick table's.
@group(0) @binding(25) var<storage, read> thermalSource : array<vec4f>;

const THERMAL_TRAVEL_COSINES : u32 = 16u;

// B~ at h* = `heightM`, cd m^-2: linear between the table's heights, held at its ends
// (`thermalPlanckInto`).
fn thermalPlanckAt(heightM : f32) -> vec3f {
  let header = thermalSource[0];
  let last = u32(header.y) - 1u;
  let place = clamp(heightM / header.x, 0.0, 1.0) * f32(last);
  let lower = min(u32(floor(place)), last - 1u);
  let f = place - f32(lower);
  let a = thermalSource[1u + lower].rgb;
  let b = thermalSource[2u + lower].rgb;
  return a + (b - a) * f;
}

// One band's J_th at h* = `heightM` for the travel cosine `muTravel`: linear in h between the
// altitudes about it, found by bisection, and in the travel cosine between the nodes about it, held
// at the edges (`scatteringLayerInto`).
fn thermalScatteringLayer(band : u32, heightM : f32, muTravel : f32) -> vec3f {
  let header = thermalSource[0];
  let altitudes = u32(header.w);
  let base = 1u + u32(header.y) + band * altitudes * (1u + THERMAL_TRAVEL_COSINES);
  let last = altitudes - 1u;
  var lo = 0u;
  var f = 0.0;
  if (!(heightM > thermalSource[base].x)) {
    lo = 0u;
  } else if (!(heightM < thermalSource[base + last].x)) {
    lo = last - 1u;
    f = 1.0;
  } else {
    var hi = last;
    loop {
      if (hi - lo <= 1u) {
        break;
      }
      let mid = (lo + hi) / 2u;
      if (thermalSource[base + mid].x <= heightM) {
        lo = mid;
      } else {
        hi = mid;
      }
    }
    let h0 = thermalSource[base + lo].x;
    f = (heightM - h0) / (thermalSource[base + lo + 1u].x - h0);
  }
  let place = clamp(
    (muTravel + 1.0) * f32(THERMAL_TRAVEL_COSINES / 2u) - 0.5,
    0.0,
    f32(THERMAL_TRAVEL_COSINES - 1u),
  );
  let v0 = min(u32(floor(place)), THERMAL_TRAVEL_COSINES - 2u);
  let g = place - f32(v0);
  let row0 = base + altitudes + lo * THERMAL_TRAVEL_COSINES + v0;
  let row1 = row0 + THERMAL_TRAVEL_COSINES;
  let low = thermalSource[row0].rgb * (1.0 - g) + thermalSource[row0 + 1u].rgb * g;
  let high = thermalSource[row1].rgb * (1.0 - g) + thermalSource[row1 + 1u].rgb * g;
  return low * (1.0 - f) + high * f;
}

// J_th at s and h* = `heightM` for the travel cosine `muTravel`, cd m^-2 per unit of scattering: 0
// without scattered thermal light, its bands linear in ln s between the two about s and the nearer
// beyond them, as the thick table's (`thermalScatteringInto`).
fn thermalScatteringAt(s : f32, heightM : f32, muTravel : f32) -> vec3f {
  let count = u32(thermalSource[0].z);
  if (count == 0u) {
    return vec3f(0.0);
  }
  let last = count - 1u;
  if (last == 0u) {
    return thermalScatteringLayer(0u, heightM, muTravel);
  }
  let ls = log(s);
  if (!(ls > oblateBand(oblateBody, 0u).y)) {
    return thermalScatteringLayer(0u, heightM, muTravel);
  }
  if (!(ls < oblateBand(oblateBody, last).y)) {
    return thermalScatteringLayer(last, heightM, muTravel);
  }
  var lower = 0u;
  while (lower + 1u < last && oblateBand(oblateBody, lower + 1u).y <= ls) {
    lower++;
  }
  let l0 = oblateBand(oblateBody, lower).y;
  let w = (ls - l0) / (oblateBand(oblateBody, lower + 1u).y - l0);
  return thermalScatteringLayer(lower, heightM, muTravel) * (1.0 - w)
    + thermalScatteringLayer(lower + 1u, heightM, muTravel) * w;
}

// A step's shares with the scattered thermal light in slot 0's, the views' path: sigma_s,k J_th at
// the sample's s and h* for the light travelling at `muTravel` to the eye, times the frame's scale.
fn thermalSharesOf(
  shares : ViewShares,
  start : LookupStart,
  local : SampleMedium,
  muTravel : f32,
) -> ViewShares {
  var out = shares;
  out.slot0 += local.scattering
    * thermalScatteringAt(start.gravityRatio, start.scaledHeightM, muTravel)
    * view.marchSky.w;
  return out;
}

// The ray march's read of each step's sources from the sources' table (plan R08, R08.T8.e's table;
// `AtmosphereSettings.marchSourceTransmittance`, `table`): each source's optical depth from the
// step to space under its light slot's curves, by the step's h* and the source's zenith cosine,
// from R08.T9.b's sources' table (`sourceTransmittance.wgsl`, one layer a latitude band and light
// slot), and, for a medium with forward lobes, the stored optical depth of every term but the
// absorbers, the lobes' read, from the table's lobes' layers after them, in place of the sliced
// lookup a source a step (`source.wgsl`'s sourceInScatteringAt, the shared read's, and
// `rayMarchSlots.wgsl`'s rayMarchSourcesAt). Appended to every thin march by `hillaire.ts`'
// withMarchSourceTable, which rewrites each read's step to take it; the thick, deck and thermal
// marches are made without it, each step's sources per step.
//
// Where the table is read: at every step of every entry point, the centre ray's, the footprint's
// and the limb's nodes alike, wherever a sources' table is bound. A body of one band takes every
// source from it; one of several bands takes a source within MARCH_TABLE_HORIZON_COSINE of the
// horizon of the reference sphere by the sliced lookup, as the lit passes do (`surfaceLighting.wgsl`'s
// SOURCE_TABLE_HORIZON_COSINE): the table's rays stand at one azimuth. Through the table's one-texel
// stub every step takes the sliced lookup, as before the lever (`per-step`). The ground's shadow is
// the reference sphere's, as the lit passes' read of the table takes it. The ray's count
// (`rayMarchDepthAlong`), the pieces' ends (`rayMarchPieceAt`) and the bare ground's light
// (`rayMarchGroundLightAt`), once a ray, keep the sliced lookup.
//
// A table-only twin (`sourceTableEngine.ts`, as R08.T9.b's lit modules' twins) sets
// SOURCE_TABLE_ONLY, so that the sliced lookup a step is compiled out of it, its arithmetic else the
// march's own: the frame dispatches it where the bound table is a body of one band's.

@group(0) @binding(23) var atmosphere_source_table : texture_2d_array<f32>;

// Whether this march is its table-only twin (`sourceTableEngine.ts`): false as written; true in the
// twin dispatched where the bound sources' table is a body of one band's, whose every step's source
// that table holds.
const SOURCE_TABLE_ONLY : bool = false;

// Within this of the horizon's zenith cosine a body of several bands takes a step's source by the
// sliced lookup, though its table is bound: `surfaceLighting.wgsl`'s SOURCE_TABLE_HORIZON_COSINE
// (lean, 2026-11-03), the same rule for the march as for the lit passes.
const MARCH_TABLE_HORIZON_COSINE : f32 = 0.1;

// Whether a step at h* `scaledHeightM` takes a source at zenith cosine `mu` from the bound sources'
// table: in the table-only twin always; through the one-texel stub never; on a body of one band
// always; on one of several unless it lies within MARCH_TABLE_HORIZON_COSINE of the horizon of the
// reference sphere R_ref.
fn marchTableTakes(body : OblateBody, scaledHeightM : f32, mu : f32) -> bool {
  if (SOURCE_TABLE_ONLY) {
    return true;
  }
  if (textureDimensions(atmosphere_source_table).x <= 1u) {
    return false;
  }
  if (oblateBandCount(body) == 1u) {
    return true;
  }
  let bottom = body.figure.z;
  let z = max(scaledHeightM, 0.0);
  let horizon = -sqrt(shellAboveGround(bottom, z)) / (bottom + z);
  return mu - horizon >= MARCH_TABLE_HORIZON_COSINE;
}

// The table's lobes' layers at `unit`: band b's at B L + b, after the B L layers of the bands' light
// slots (`tables.ts`'s sourceTransmittance), linear in ln s between the bands about `s`, as
// `oblate.wgsl`'s bandTableAt takes a light slot's.
fn marchLobesDepthAt(body : OblateBody, unit : vec2f, s : f32) -> vec3f {
  let bands = oblateBandCount(body);
  let first = bands * oblateLayersPerBand(body);
  let last = bands - 1u;
  if (last == 0u) {
    return tableAtUnitLayer(atmosphere_source_table, first, unit).rgb;
  }
  let ls = log(s);
  if (!(ls > oblateBand(oblateBody, 0u).y)) {
    return tableAtUnitLayer(atmosphere_source_table, first, unit).rgb;
  }
  if (!(ls < oblateBand(oblateBody, last).y)) {
    return tableAtUnitLayer(atmosphere_source_table, first + last, unit).rgb;
  }
  var lower = 0u;
  while (lower + 1u < last && oblateBand(oblateBody, lower + 1u).y <= ls) {
    lower++;
  }
  let l0 = oblateBand(oblateBody, lower).y;
  let w = (ls - l0) / (oblateBand(oblateBody, lower + 1u).y - l0);
  let below = tableAtUnitLayer(atmosphere_source_table, first + lower, unit).rgb;
  let above = tableAtUnitLayer(atmosphere_source_table, first + lower + 1u, unit).rgb;
  return below * (1.0 - w) + above * w;
}

// A step's source of light slot `slot` at zenith cosine `mu` from the bound sources' table
// (`source.wgsl`'s SourceLight): lit where the reference sphere's ground leaves it so, its
// transmittance the table's optical depth at the step's h* (held to [0, top]) and `mu` on the
// reference sphere's axes, linear in ln s between the bands about the step's s, exponentiated in
// f32, as the lit passes read it (`surfaceLighting.wgsl`'s atmosphereTabledTransmittance); and,
// where the medium has forward lobes (`lobes`), the lobes' stored depth from the lobes' layers.
fn marchTabledLightAt(
  body : OblateBody,
  start : LookupStart,
  mu : f32,
  slot : u32,
  lobes : bool,
) -> SourceLight {
  let bottom = body.figure.z;
  let top = body.counts.w;
  let z = clamp(start.scaledHeightM, 0.0, top);
  let m = clamp(mu, -1.0, 1.0);
  let unit = transmittanceUnitAt(bottom, top, z, m);
  let depth = bandTableAt(
    body, oblateBody, atmosphere_source_table, unit, start.gravityRatio, slot
  );
  // A loop of at most one pass, not an `if`, as the composite's reads are (`composite.wgsl`):
  // SwiftShader runs both sides of a branch under its lanes' mask.
  var stored = vec3f(0.0);
  for (var read = lobes; read; read = false) {
    stored = marchLobesDepthAt(body, unit, start.gravityRatio);
  }
  return SourceLight(select(1.0, 0.0, groundHitFromHeight(bottom, z, m)), exp(-depth), stored);
}

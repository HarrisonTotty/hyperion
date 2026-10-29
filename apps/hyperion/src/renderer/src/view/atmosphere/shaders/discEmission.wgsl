// The ground's thermal emission on R07's disc (plan R08, R08.T18.a; `discReflectance.ts`, whose
// `discEmissionAt` is its twin): composed only into the disc kernels' emissive forms
// (`withDiscEmission`), after bodyDisc.wgsl, whose `shade` calls `disc_emission` as its last step,
// and thermal.wgsl. A record drawn by these forms carries its emission block past the records,
// EMISSION_ROWS rows from the row its row 46's y names (`packDiscEmission`):
//   0  the brightest star's unit direction from the body's centre, along the galactic axes, zero
//      where none lights it; w the frame's exposure scale;
//   1  the temperature field's profile (`thermal.ts`' `packTemperatureField`);
//   2  x the field's floor, K; y the melt's least temperature, K, 0 with no melt; z the classes
//      whose form is `Melt`, bit k for class k; w 0.
// A block's first row is past every record, so 0 names none, and such a record emits nothing.
const EMISSION_ROWS : u32 = 3u;

// The light a sample emits, pre-exposed: the field's temperature at its datum normal `normal`, from
// the star's cosine and the latitude's sine about the disc's pole; its laws by their shares at the
// sample (`surface`, `laws` of them) split into the ground's and the `Melt` classes', each through
// `thermal_radiance` with its laws' A summed by their shares, the melt's at no less than its least
// temperature; times the exposure scale.
fn disc_emission(body : Body, normal : vec3f, surface : SurfaceAt, laws : u32) -> vec3f {
  let first = u32(disc_row(SECONDARY_COUNT_ROW).y);
  if (first == 0u) {
    return vec3f(0.0);
  }
  let star = discs[first];
  let profile = discs[first + 1u];
  let floors = discs[first + 2u];
  let t = thermal_field_temperature(
    profile,
    floors.x,
    dot(normal, to_view(star.xyz)),
    dot(normal, body.pole),
  );
  let melt_mask = u32(floors.z);
  var ground_a = vec3f(0.0);
  var ground = 0.0;
  var melt_a = vec3f(0.0);
  var melt = 0.0;
  for (var m = 0u; m < laws; m = m + 1u) {
    let share = surface.shares[m];
    if (!(share > 0.0)) {
      continue;
    }
    if (m > 0u && ((melt_mask >> (m - 1u)) & 1u) == 1u) {
      melt_a = melt_a + share * surface.albedo[m];
      melt = melt + share;
    } else {
      ground_a = ground_a + share * surface.albedo[m];
      ground = ground + share;
    }
  }
  var out = vec3f(0.0);
  if (ground > 0.0) {
    out = out + thermal_radiance(ground_a, ground, t);
  }
  if (melt > 0.0) {
    out = out + thermal_radiance(melt_a, melt, max(t, floors.y));
  }
  return out * star.w;
}

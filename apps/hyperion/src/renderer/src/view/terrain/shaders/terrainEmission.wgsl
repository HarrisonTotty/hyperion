// The ground's thermal emission on R10's lit terrain (plan R10, R10.T10.b; plan R08, R08.T18.a;
// decision-composition §1.9): composed only into the lit terrain's emissive forms
// (`terrainMaterial.ts`' `withTerrainEmission`), after terrainLit.wgsl, whose `shade_terrain` adds
// `terrain_emission` to each texel's light as its last step, and R08's thermal.wgsl, the function
// R07's disc shares (`discEmission.wgsl`). Its twin is `discReflectance.ts`' `discEmissionAt`, over
// the texel's shares and A with the patch's `Melt` slots as the disc's `Melt` classes.
//
// A body drawn by these forms carries its emission block in `terrain_lights` past the lights,
// EMISSION_ROWS rows from FIRST_TERRAIN_EMISSION_ROW (`packTerrainLighting`), body-fixed:
//   0  the brightest star's unit direction from the body's centre, zero where none lights it; w the
//      frame's exposure scale;
//   1  the temperature field's profile (`thermal.ts`' `packTemperatureField`);
//   2  x the field's floor, K; y the melt's least temperature, K, 0 with no melt; z the disc's
//      `Melt` classes' mask, unread here (the terrain's are its slots' word 6); w 0.
// A palette slot whose form is `Melt` has 1 in its laws' word 6 (`packClassLaws`), unread by the
// plain forms; its share emits at no less than the melt's least temperature.
const FIRST_TERRAIN_EMISSION_ROW : u32 = 25u;
const TERRAIN_EMISSION_ROWS : u32 = 3u;

// Whether palette slot k of `slot` has the `Melt` form: its laws' word 6.
fn class_melt(slot : u32, k : u32) -> bool {
  let at = slot * CLASS_LAYER_WORDS + CLASS_LAW_WORD + 8u * k;
  return bitcast<f32>(class_layers[at + 6u]) > 0.5;
}

// The light a texel emits, pre-exposed: the field's temperature at its datum normal `nu`, from the
// star's cosine and the latitude's sine about the body's pole; its laws by their shares split into
// the ground's and the `Melt` slots', each through `thermal_luminance` with its laws' A (`albedos`,
// `terrain_albedos`') summed by their shares, the melt's at no less than its least temperature;
// times the exposure scale.
fn terrain_emission(
  nu : vec3f,
  shares : array<f32, TERRAIN_LAWS>,
  albedos : array<vec3f, TERRAIN_LAWS>,
  slot : u32,
) -> vec3f {
  let star = terrain_row(FIRST_TERRAIN_EMISSION_ROW);
  let profile = terrain_row(FIRST_TERRAIN_EMISSION_ROW + 1u);
  let floors = terrain_row(FIRST_TERRAIN_EMISSION_ROW + 2u);
  let t = thermal_field_temperature(profile, floors.x, dot(nu, star.xyz), dot(nu, BODY_POLE));
  var ground_a = vec3f(0.0);
  var ground = 0.0;
  var melt_a = vec3f(0.0);
  var melt = 0.0;
  for (var m = 0u; m < TERRAIN_LAWS; m = m + 1u) {
    let share = shares[m];
    if (!(share > 0.0)) {
      continue;
    }
    if (m > 0u && class_melt(slot, m - 1u)) {
      melt_a = melt_a + share * albedos[m];
      melt = melt + share;
    } else {
      ground_a = ground_a + share * albedos[m];
      ground = ground + share;
    }
  }
  var out = vec3f(0.0);
  if (ground > 0.0) {
    out = out + thermal_luminance(ground_a, ground, t);
  }
  if (melt > 0.0) {
    out = out + thermal_luminance(melt_a, melt, max(t, floors.y));
  }
  return out * star.w;
}

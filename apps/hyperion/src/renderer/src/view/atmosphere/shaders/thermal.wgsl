// The ground's thermal emission, the shared function (plan R08, R08.T18.a; decision-composition
// §1.9): `thermal.ts`' `thermalLuminance` and `fieldTemperatureK` in f32, operation for operation,
// which R07's disc (`discEmission.wgsl`) and R10's terrain (`terrainEmission.wgsl`) call in their
// emissive forms alone. Composed after its tables (`thermal.ts`' THERMAL_WGSL: THERMAL_BINS,
// THERMAL_EXPONENT_MAX, THERMAL_BIN_EXPONENT_K, THERMAL_BIN_LUMINANCE and THERMAL_GROUND_RULE),
// with no binding of its own.

// The luminance per render channel, cd m⁻² before the exposure, that a share `share` of a surface
// emits at `temperature` K, its laws' A summed by their shares in `albedo_sum`: over the 15 bake
// bins, Planck's 1 ÷ (e^x − 1) as e^(−x) ÷ (1 − e^(−x)), none past THERMAL_EXPONENT_MAX, times the
// bin's emissivity by Kirchhoff, share − A_b held to [0, share] with A_b the ground rule of the
// summed A, times its luminance per channel.
fn thermal_luminance(albedo_sum : vec3f, share : f32, temperature : f32) -> vec3f {
  var out = vec3f(0.0);
  for (var b = 0u; b < THERMAL_BINS; b = b + 1u) {
    let x = THERMAL_BIN_EXPONENT_K[b] / temperature;
    if (!(x <= THERMAL_EXPONENT_MAX)) {
      continue;
    }
    let e = exp(-x);
    let planck = e / (1.0 - e);
    let emissivity = clamp(share - dot(THERMAL_GROUND_RULE[b], albedo_sum), 0.0, share);
    out = out + THERMAL_BIN_LUMINANCE[b] * (emissivity * planck);
  }
  return out;
}

// A temperature field's temperature at a point, K, before the melt's (`thermal.ts`'
// `packTemperatureField`): `profile.x` 1 for a locked world's eyeball, y T_n⁴ and z T_ss⁴ − T_n⁴;
// 2 for a zonal profile, y T_s, z (T(0°) − T(90°)) ÷ 3 and w (3/4) of the day–night contrast; held to
// `floor_k`. `cos_substellar` is the cosine of the point's angle from the brightest star's
// direction, `sin_latitude` the sine of its latitude about the pole.
fn thermal_field_temperature(
  profile : vec4f,
  floor_k : f32,
  cos_substellar : f32,
  sin_latitude : f32,
) -> f32 {
  var t : f32;
  if (profile.x < 1.5) {
    t = sqrt(sqrt(profile.y + profile.z * max(cos_substellar, 0.0)));
  } else {
    t = profile.y - profile.z * (3.0 * sin_latitude * sin_latitude - 1.0) + profile.w * cos_substellar;
  }
  return max(t, floor_k);
}

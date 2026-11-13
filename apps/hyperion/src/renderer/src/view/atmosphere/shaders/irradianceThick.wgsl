// The sky-irradiance table's read of the thick table (plan R08, R08.T14.d.4; R08.T14.d's
// precondition 3): on a body whose tables hold one, the sky's irradiance gathers the multiple
// scattering the sky is drawn with, so that the ground is lit by the light it is seen under.
// Appended after `thickRead.wgsl` to `irradiance.wgsl` by `tables.ts`'s IRRADIANCE_THICK_KERNEL,
// which points the build's multiple scattering and its forward lobes' higher orders here and leaves
// every other line as it is; its CPU twin is `tablesCpu.ts`'s irradianceTwin with its thick layer.

// The thick table's layer the build reads (`Medium.build.w`): band b's light slot k at
// b x slots + k, the table's last slot where it holds fewer (`tablesCpu.ts`'s thickIrradianceLayer).
fn buildThickLayer() -> u32 {
  return u32(medium.build.w);
}

// The multiple scattering per unit of illuminance and of scattering, sr^-1, at a step of the build's
// march along the unit `dir` from the texel's point, the step's unit zenith `up` and the source along
// `sun`: as `thickReadBands.wgsl`'s thickMultipleAt forms it, on the build's one layer, `psi` the
// band's own Psi_ms at the step. On the bake's table Psi_ms at the analytic term's share and the
// table's J, held at 0, at the rest; beside the correction alone, Psi_ms whole and the correction at
// the table's share, the sum held at 0.
fn thickIrradianceMultiple(
  psi : vec3f,
  scaledHeightM : f32,
  muSun : f32,
  dir : vec3f,
  up : vec3f,
  sun : vec3f,
) -> vec3f {
  let analytic = thickAnalyticShare(muSun);
  let correction = thickCorrectionOnly();
  var multiple = select(analytic, 1.0, correction) * psi;
  if (analytic < 1.0) {
    let muView = dot(dir, up);
    let horizontal = dot(dir, sun) - muView * muSun;
    let harmonic = 2.0 * horizontal * horizontal - (1.0 - muView * muView) * (1.0 - muSun * muSun);
    let source = thickLayerAt(buildThickLayer(), thickRowAt(scaledHeightM), muSun, -muView);
    let j = source.level + source.slope * horizontal + source.curve * harmonic;
    if (correction) {
      multiple = max(multiple + (1.0 - analytic) * j, vec3f(0.0));
    } else {
      multiple += (1.0 - analytic) * max(j, vec3f(0.0));
    }
  }
  return multiple;
}

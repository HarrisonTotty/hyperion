// The decked air's departure's sky (plan R08, fix-r08-decked-air-route; `rayMarch.wgsl`'s header):
// the sky's irradiance at a drawn surface's end of a thick read's ray march, from the table the lit
// passes read it from (`surfaceLighting.wgsl`'s `atmosphere_sky_irradiance`, `irradiance.wgsl`;
// on a body whose tables hold a thick table its build gathers the bake's J, `irradianceThick.wgsl`),
// so that the ground's light the march takes from its in-scatter, S_bar, is lit as the drawn
// ground is. Appended after the thick read to the ray march's thick forms alone by `hillaire.ts`'s
// withThickRead, which points the march's `skyLit` here; the sky view's and the aerial
// perspective's thick forms declare no table of it, and a deck's march drops it (withDeckRead).

// The sky's irradiance table: one layer a latitude band and light slot, band b's slot k at
// b x (the layers a band) + k, per unit of the source's illuminance (`tables.ts`'s irradiance).
@group(0) @binding(26) var marchIrradiance : texture_2d_array<f32>;

// The sky's irradiance on a horizontal surface at the lookup's start, per unit of the source's
// illuminance, for a source of light slot `slot` at zenith cosine `muSun` against the datum's normal
// there: `surfaceLighting.wgsl`'s read at the start's h* (its s times its height above the datum)
// and s, in x_r = sqrt(h* / H) and mu_s / 2 + 1/2, between the bands about s.
fn thickDepartureSkyAt(body : OblateBody, start : LookupStart, muSun : f32, slot : u32) -> vec3f {
  let unit = vec2f(muSun * 0.5 + 0.5, sqrt(max(start.scaledHeightM, 0.0) / body.counts.w));
  return bandTableAt(
    body, oblateBody, marchIrradiance, clamp(unit, vec2f(0.0), vec2f(1.0)), start.gravityRatio,
    slot
  );
}

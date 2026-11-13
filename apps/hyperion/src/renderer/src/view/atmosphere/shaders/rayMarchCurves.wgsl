// The ray march's read of a source's light by the curves of growth (plan R08, R08.T6.c and R08.T7;
// R08.T8.e, lever 5 of `decision-r08-t8-leans.md` sections 10 and 13): the shared transmittance
// tables' stored optical depth and absorber columns through the sliced lookup, each light slot's
// curves applied on read (`source.wgsl`'s sourceLightAt), as every per-frame kernel reads it. Appended
// after `rayMarch.wgsl` to make `hillaire.ts`'s RAY_MARCH_KERNEL, the march of every body that keeps
// the shared read (`tables.ts`'s slotDepthsRefusal); `rayMarchSlots.wgsl` makes the other kernel from
// the same `rayMarch.wgsl`. It declares nothing.

// The view's sources at a step, summed by light slot (`rayMarch.wgsl`'s rayMarchStep): the per-frame
// kernels' own (`source.wgsl`'s viewSourcesAt).
fn rayMarchSourcesAt(
  body : OblateBody,
  start : LookupStart,
  up : vec3f,
  ray : vec3f,
  lobes : bool,
  local : SampleMedium,
) -> ViewShares {
  return viewSourcesAt(body, start, up, ray, lobes, local);
}

// The transmittance of light slot `slot`'s source along the unit body-fixed `d` to the bare ground at
// the lookup's start (`rayMarch.wgsl`'s bounce): `sourceLightAt`'s.
fn rayMarchGroundLightAt(body : OblateBody, start : LookupStart, d : vec3f, slot : u32) -> vec3f {
  return sourceLightAt(body, start, d, slot).toSun;
}

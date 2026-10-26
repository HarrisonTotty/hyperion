// The ray march's read of a source's light from its light slot's layer (plan R08, R08.T8.e, lever 5;
// `decision-r08-t8-leans.md` sections 10 and 13): each source's optical depth to the top in one read
// of its slot's layer of `tables.ts`'s slot layers (`slotDepths.wgsl`, the shared table's stored depth
// with that slot's curves applied at each texel), through the sliced lookup and divided by s, in place
// of the stored depth and absorber columns read and the curves applied on read (`source.wgsl`'s
// sourceLightAt). R05's read was one read of stored optical depth a source a step; R08 split the
// absorbers out because their curves depend on the star (Design note 5), and this restores one read
// a source a step at the source's light slot. Appended after `rayMarch.wgsl` to make `hillaire.ts`'s
// RAY_MARCH_SLOTS_KERNEL, chosen for a body whose tables hold the layers (an absorber, one latitude
// band, its stars on at most MAX_BODY_LIGHTS slots, its per-planet tables within the 2 MB row with
// them), when its pipeline is made; `rayMarchCurves.wgsl` makes the other kernel. It declares the
// layers, which no other kernel binds.
//
// Where the read is taken: each source's light at each step, and the bare ground's light. Lever 3's
// count (`rayMarch.wgsl`'s rayMarchDepthAlong) keeps the curves' read, a read a ray, so that no count
// moves and the absorber tables stay bound to this kernel's entry point, which the engine's automatic
// layouts need; the forward lobes keep the shared table's stored depth, extinction alone, read at the
// same slices and weights. Between texels the layer interpolates -ln T_c(u) where the curves' read
// interpolates u, a difference of second order in a texel's change of u, nil in the linear regime;
// and a layer's depth can be divided by s only after its curves, which takes s^(n - 1) of today's
// absorber depth, n = d ln D / d ln u, at most 1% of it on a body of one band (section 13, point 1).
//
// SwiftShader inlines every call, and its GPU process crashed on more copies of a read's code
// (R08.T8.d, R08.T8.e): the step's loop calls the sliced ray and the slot read once each, and the
// read returns a matrix, no struct.

@group(0) @binding(16) var slotDepths : texture_2d_array<f32>;

// A sliced ray's read of light slot `slot`'s layer (`tablesCpu.ts`'s OblateReader slot read):
// column 0 the slot's optical depth to the top, column 1 the shared table's stored optical depth of
// every term but the absorbers, the forward lobes' depth, where `withExtinction` and 0 elsewhere;
// each slice read at its own cosine from h* held in the shell, weighted as `slicedStoredOf` weights
// it, and the sums divided by s. Slice k's slot p is layer k L + p, L the body's light slots
// (`oblateLayersPerBand`).
fn slicedSlotDepthsOf(
  ray : SlicedRay,
  body : OblateBody,
  slot : u32,
  withExtinction : bool,
) -> mat2x3f {
  let top = body.counts.w;
  let height = clamp(ray.scaledHeightM, 0.0, top);
  let slots = oblateLayersPerBand(body);
  let p = min(slot, slots - 1u);
  var total = vec3f(0.0);
  var extinction = vec3f(0.0);
  if (ray.weight < 1.0) {
    let k = ray.lowerSlice;
    let unit = transmittanceUnitAt(oblateSlice(oblateBody, k).x, top, height, ray.lowerCosine);
    let w = 1.0 - ray.weight;
    total = tableAtUnitLayer(slotDepths, k * slots + p, unit).rgb * w;
    if (withExtinction) {
      extinction = tableAtUnitLayer(transmittance, k, unit).rgb * w;
    }
  }
  if (ray.weight > 0.0) {
    let k = ray.lowerSlice + 1u;
    let unit = transmittanceUnitAt(oblateSlice(oblateBody, k).x, top, height, ray.upperCosine);
    let w = ray.weight;
    total = total + tableAtUnitLayer(slotDepths, k * slots + p, unit).rgb * w;
    if (withExtinction) {
      extinction = extinction + tableAtUnitLayer(transmittance, k, unit).rgb * w;
    }
  }
  let inverse = 1.0 / ray.gravityRatio;
  return mat2x3f(total * inverse, extinction * inverse);
}

// The view's sources at a step, summed by light slot (`rayMarch.wgsl`'s rayMarchStep):
// `source.wgsl`'s viewSourcesAt and sourceInScatteringAt, each source's light from its slot's layer,
// its sliced ray and its read each at one call site in the loop.
fn rayMarchSourcesAt(
  body : OblateBody,
  start : LookupStart,
  up : vec3f,
  ray : vec3f,
  lobes : bool,
  local : SampleMedium,
) -> ViewShares {
  var shares = ViewShares(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
  let count = viewSourceCount();
  for (var k = 0u; k < count; k++) {
    let source = view.sources[k];
    let slot = skySlotOf(source);
    let dir = source.direction.xyz;
    let sliced = slicedRayOf(body, oblateBody, start, dir);
    let read = slicedSlotDepthsOf(sliced, body, slot, lobes);
    let light = SourceLight(select(1.0, 0.0, sliced.shadowed), exp(-read[0]), read[1]);
    let muSun = dot(dir, up);
    let angle = scatteringAngle(ray, dir);
    var phased = phasedScatteringOf(local, angle);
    if (lobes) {
      // The forward lobes' higher orders, at the stored optical depth along the source (R08.T7).
      phased += lobeOrdersOf(local, angle, light.depth);
    }
    let one = sourceAt(body, start, light, muSun, slot, local, phased);
    shares = viewShareAdd(shares, slot, source.weight.rgb, one);
  }
  return shares;
}

// The transmittance of light slot `slot`'s source along the unit body-fixed `d` to the bare ground at
// the lookup's start (`rayMarch.wgsl`'s bounce): one read of its slot's layer, as the air above it
// takes it (section 13, point 4).
fn rayMarchGroundLightAt(body : OblateBody, start : LookupStart, d : vec3f, slot : u32) -> vec3f {
  let sliced = slicedRayOf(body, oblateBody, start, d);
  return exp(-slicedSlotDepthsOf(sliced, body, slot, false)[0]);
}

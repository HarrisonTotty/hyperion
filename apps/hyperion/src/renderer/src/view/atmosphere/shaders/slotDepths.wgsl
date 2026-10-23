// Each light slot's optical depth to the top (plan R08, R08.T8.e, lever 5; `decision-r08-t8-leans.md`
// sections 10 and 13): one layer a kappa slice and light slot, slice k's slot p at layer k L + p, L the
// light slots the body's stars take (`tables.ts`'s multiScatteringLayersOf). Each texel is the shared
// transmittance table's stored optical depth at that texel (rgb) plus slot p's curves of growth at the
// texel's own stored absorber columns and pressure-weighted columns, each curve clamped to
// [0, -ln 1e-6] as every read clamps it (`common.wgsl`'s absorberDepth), the sum held to the half
// float's largest, as the transmittance kernel holds every stored value. So at a texel's own (h, mu)
// the layer is today's read of that slot's light, exactly, and the ray march reads a source's light
// from it in one read (`rayMarchSlots.wgsl`). Built only under an absorber, for a body of one latitude
// band whose stars take at most MAX_BODY_LIGHTS slots and whose per-planet tables stay within the
// 2 MB row with the layers (`tables.ts`'s slotDepthsRefusal), after the transmittance slices and
// before the multiple scattering, from the stored tables, in the stored build and the smoke's
// kernel arm alike. Follows `view/shaders/preciseTrig.wgsl` and `common.wgsl`; it takes no
// `medium.wgsl`, whose reads of the terms it has no use for, and reads the medium's absorber slots
// and line slots from the uniform itself.
//
// The reads take the texel itself (textureLoad, no filter), with `sliceStoredAt`'s guards
// (`oblate.wgsl`): the pressure-weighted columns only under an absorber, the further slots' columns
// only for two or more, and the fifth pressure channel's table at its own size, one texel unless the
// medium has five line slots, as the bilinear read's clamp holds it.

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var transmittance : texture_2d_array<f32>;
@group(0) @binding(2) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(3) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(4) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(0) @binding(5) var absorberCurves : texture_2d<f32>;
@group(0) @binding(6) var slotDepthsOut : texture_storage_2d_array<rgba16float, write>;

// rgba16float's largest finite value (IEEE 754-2008 binary16), to which the sum is held, as
// `transmittance.wgsl`'s HALF_MAX holds every stored value.
const SLOT_DEPTH_MAX : f32 = 65504.0;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = textureDimensions(slotDepthsOut);
  let layers = textureNumLayers(slotDepthsOut);
  if (id.x >= size.x || id.y >= size.y || id.z >= layers) {
    return;
  }
  // One layer of the shared tables a slice; the light slots a slice takes are the layers over them.
  let slices = max(textureNumLayers(transmittance), 1u);
  let slots = max(layers / slices, 1u);
  let slice = id.z / slots;
  let slot = id.z % slots;
  let texel = id.xy;
  let count = min(u32(medium.absorbers.x), ABSORBER_SLOTS);
  let lines = u32(medium.absorbers.w);
  let stored = textureLoad(transmittance, texel, slice, 0);
  var columns = AbsorberColumns(stored.a, vec4f(0.0));
  var pressureColumns = AbsorberColumns(0.0, vec4f(0.0));
  if (count > 0u) {
    let firstFour = textureLoad(absorberPressureColumns, texel, slice, 0);
    let lastTexel = min(texel, textureDimensions(lastAbsorberPressureColumn) - 1u);
    let last = textureLoad(lastAbsorberPressureColumn, lastTexel, slice, 0).x;
    pressureColumns = AbsorberColumns(firstFour.x, vec4f(firstFour.yzw, last));
  }
  if (count > 1u) {
    columns.more = textureLoad(absorberColumns, texel, slice, 0);
  }
  let absorbed = absorberDepth(absorberCurves, count, slot, lines, columns, pressureColumns);
  let total = min(stored.rgb + absorbed, vec3f(SLOT_DEPTH_MAX));
  textureStore(slotDepthsOut, texel, id.z, vec4f(total, 0.0));
}

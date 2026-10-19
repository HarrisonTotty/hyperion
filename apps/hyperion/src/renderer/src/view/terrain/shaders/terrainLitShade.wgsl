// The lit terrain material's own `Draw` and fragment stage (plan R10, R10.T10.b; split from
// terrainLit.wgsl by R10.T16.c, so that the terrain's depth for the symbology's pass,
// terrainDepth.wgsl, takes the same vertices and survey discard with a `Draw` and fragment of its
// own). Composed after terrainLit.wgsl, before the vertex path's source (`terrainMaterial.ts`).

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The camera less the body's centre, body-fixed, metres; w, 1 ÷ a, a the equatorial radius.
  cameraBodyFixedM : vec4f,
  // The normals atlas: tiles across a layer, tiles a layer, a tile's stored texels, samples a side.
  atlas : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@fragment
fn fragmentMain(in : LitVertexOut) -> @location(0) vec4f {
  if (!survey_kept(in)) {
    discard;
  }
  // One call of the shading: a compiler that inlines every call (SwiftShader's) takes one copy.
  let reference = in.kind != LIT_GRID;
  let normal = select(patchNormalAt(in.slot, in.patchUv), normalize(in.datumNormal), reference);
  let shaded = shade_terrain(in, normal, reference);
  var meter = METER_OTHER;
  if (u32(terrain_row(0u).x) > 0u) {
    meter = select(METER_UNLIT_BODY, METER_LIT_BODY, shaded.lit);
  }
  return vec4f(min(shaded.radiance, vec3f(HALF_FLOAT_MAX)), meter);
}

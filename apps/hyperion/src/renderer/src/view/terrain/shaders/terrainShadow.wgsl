// A view's lit terrain as a caster into one of the high setting's shadow cascades, depth alone
// (plan R10, R10.T9.b; Design note 11; decision-r10-t9b-design D4): the lit material's vertices,
// skirts and fins (terrainLit.wgsl's `vertexMain`, unchanged), placed by the pass's `Frame`, which a
// cascade's pass fills with its light rotation and reversed orthographic projection (`cascades.ts`'
// `Cascade.viewRotation` and `projection`), from the view's own instance records, whose origins
// and morph factors are the eye's; so each patch stores the surface the view draws, a patch at
// k = 1 its parent's. Its survey discard (`survey_kept`) keeps the terrain's surveyed cells and the
// skirts and fins that join them to the reference surface, as the image draws them. Composed after
// terrainLit.wgsl, in place of terrainLitShade.wgsl, before the vertex path's source
// (`terrainMaterial.ts`' `terrainShadowMaterialSpec`).
//
// Drawn into a `depth-only` target (the engine's `DepthLayerTarget`), so its fragment stage writes
// no output; it is kept for the discard. The depth is the rasteriser's, never `frag_depth`, pushed
// away from the light by the pipeline's slope-scaled bias (`CASCADE_DEPTH_BIAS`, as
// `depthBiasAway`), with culling off so that a patch casts from either face, and, where the
// adapter offers `depth-clip-control`, `unclippedDepth`, which pancakes a caster nearer the sun than
// the cascade's near plane onto it.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The lit material's: the camera less the body's centre, body-fixed, metres, w 1 ÷ a; unread.
  cameraBodyFixedM : vec4f,
  // The lit material's normals atlas; unread here.
  atlas : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@fragment
fn fragmentMain(in : LitVertexOut) {
  if (!survey_kept(in)) {
    discard;
  }
}

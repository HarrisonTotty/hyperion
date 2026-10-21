// The lit terrain material's own `Draw` and fragment stage (plan R10, R10.T10.b; split from
// terrainLit.wgsl by R10.T16.c, so that the terrain's depth for the symbology's pass,
// terrainDepth.wgsl, and the shadow cascades' caster, terrainShadow.wgsl (R10.T9.b), take the same
// vertices and survey discard with a `Draw` and fragment of their own). Composed after
// terrainLit.wgsl and before the cascades' receiver (`shadows/cascadeShadow.wgsl`, R10.T9.b), which
// reads this `Draw`'s cascade fields, then the vertex path's source (`terrainMaterial.ts`).
//
// The cascades' lit fraction c′ (R10.T9.b; Design note 11; decision-r10-t9b-design D5(b) and (c))
// is formed here, once a fragment, and handed to `shade_terrain`, whose star loop takes it on star 0
// beside the horizon map: from the fragment's place, its geometric normal and star 0's direction,
// each turned from the body-fixed axes into the non-rotating ones by `bodyRotation`. The geometric
// normal comes from the fine screen derivatives of the place, taken at the top, before the discard,
// where they stand in uniform control flow. A reference fragment (a skirt's wall or a fin below its
// lip) takes c′ = 1, as it takes no horizon map, so that it matches R07's reference surface beneath
// it, which has no cascades; so a craft's shadow stops at the survey's edge. A skirt's lip, which
// shades as the grid at its edge vertex (R10.T8.c), is received there too: lifted along the datum's
// normal to its edge vertex's height, with the datum's normal for its own, a wall's.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The camera less the body's centre, body-fixed, metres; w, 1 ÷ a, a the equatorial radius.
  cameraBodyFixedM : vec4f,
  // The normals atlas: tiles across a layer, tiles a layer, a tile's stored texels, samples a side.
  atlas : vec4f,
  // Each cascade's `lightFromCamera` (`cascades.ts`' `Cascade`), nearest first: a camera-relative
  // place in the body's non-rotating axes to the cascade's clip x, y and depth. Identity at w = 0.
  cascadeLight0 : mat4x4f,
  cascadeLight1 : mat4x4f,
  cascadeLight2 : mat4x4f,
  cascadeLight3 : mat4x4f,
  // C₁ … C₄, each cascade's far end along the view axis, metres.
  cascadeFarM : vec4f,
  // Where each cascade starts to blend into the next, the last into lit, metres along the view axis.
  cascadeFadeStartM : vec4f,
  // Each cascade's normal offset, metres: NORMAL_OFFSET_TEXELS of its texels.
  cascadeNormalOffsetM : vec4f,
  // The view axis, the camera's forward, in the body's non-rotating axes; w, the cascades' weight
  // (R10.T9.a's `cascadeWeight`), 0 where no cascades were drawn.
  cascadeViewAxis : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@fragment
fn fragmentMain(in : LitVertexOut) -> @location(0) vec4f {
  let datum = normalize(in.datumNormal);
  let geometric = cascade_geometric_normal(
    dpdxFine(in.fromCameraBf), dpdyFine(in.fromCameraBf), datum
  );
  if (!survey_kept(in)) {
    discard;
  }
  // One call of the shading: a compiler that inlines every call (SwiftShader's) takes one copy.
  // The grid's normal morphs with its height, to the parent's tile by k, and a skirt's lip shades
  // as the grid at its edge vertex (R10.T8.c).
  let reference = in.kind != LIT_GRID && !skirt_lip(in);
  // A branch, not a `select`, so that a skirt's or a fin's fragment reads no tile and no cascade.
  var normal = datum;
  var cascade_lit = 1.0;
  if (!reference) {
    normal = shadingNormalAt(in.slot, in.parent, in.patchUv, in.morph);
    var place = in.fromCameraBf;
    var receiver_normal = geometric;
    if (in.kind != LIT_GRID) {
      place = place + datum * (in.gridHeightM - in.heightM);
      receiver_normal = datum;
    }
    let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
    // Star 0's unit direction from the body's centre (`packTerrainLighting`), the brightest: its
    // centre's, for the normal offset's N·L, not the top edge of its disc that the cascades are
    // fitted to (decision-r10-t9b-penumbra); not normalised again, so that a frame without a star,
    // whose row is zeros and whose weight is 0, forms no NaN.
    let sun = terrain_row(FIRST_TERRAIN_LIGHT_ROW).xyz;
    cascade_lit = cascade_lit_fraction(rotation * place, rotation * receiver_normal, rotation * sun);
  }
  let shaded = shade_terrain(in, normal, reference, cascade_lit);
  var meter = METER_OTHER;
  if (u32(terrain_row(0u).x) > 0u) {
    meter = select(METER_UNLIT_BODY, METER_LIT_BODY, shaded.lit);
  }
  return vec4f(min(shaded.radiance, vec3f(HALF_FLOAT_MAX)), meter);
}

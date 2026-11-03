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
//
// Near a hand-over that fades the terrain's own detail (plan R10, R10.T15.b.2; `handover.ts`'
// `handoverDetailShare`), the lights' row 2's z, the share, is below 1: the texel is lit at a place
// mixed by the share from the disc's, the point where its ray meets the datum (`disc_place`), with
// the datum's normal and height there, its shading normal mixed from the disc's n′ there
// (`disc_shading_normal`, the disc's normal map bound at 24), and `shade_terrain` scales the
// horizon map's elevations by it, so that at 0 the terrain's pixel is lit as the disc lights the
// same pixel, its relief standing in its geometry alone. The cascades' receiver keeps the texel's
// own place and geometric normal.

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

// The lit surface takes the stride morph (plan R14, R14.T3.c; `terrainStrides.wgsl`).
const STRIDE_MORPH : bool = true;

// The disc's normal map (plan R10, R10.T10.g; `bodies/discNormals.ts`, bound as `bodyDisc.wgsl`'s
// `surface_normals` is): six layers, one a face, of (N + 2) × (N + 2) `rg16float` texels, face
// texel (i, j) at (i + 1, j + 1), each the octahedral pair of a body-fixed unit normal, a gutter of
// the neighbouring faces' edge texels about it, and the pair (2, 2) where it holds none. Read only
// where the lights' row 2's z, the share of the terrain's own detail (R10.T15.b.2), is below 1,
// which a draw gives only with its disc's map bound; any 2D array texture of floats otherwise.
@group(2) @binding(24) var disc_normals : texture_2d_array<f32>;

// A normal-map texel's first coordinate above this holds no normal (`discNormals.ts`'
// NO_NORMAL_ABOVE).
const DISC_NO_NORMAL_ABOVE : f32 = 1.5;

// The decoded normal of the disc's normal-map texel `texel` (the gutter's at −1 and N) of `face`'s
// layer in xyz, w 1; or w 0 where it holds none (`bodyDisc.wgsl`'s `stored_normal`).
fn disc_stored_normal(texel : vec2i, face : u32) -> vec4f {
  let pair = textureLoad(disc_normals, vec2u(texel + vec2i(1)), face, 0).xy;
  if (abs(pair.x) > DISC_NO_NORMAL_ABOVE) {
    return vec4f(0.0);
  }
  return vec4f(decodeOctahedral(pair), 1.0);
}

// The disc's shading normal n′ at cell coordinates `st` of `face`, body-fixed, as `bodyDisc.wgsl`'s
// `shading_normal` reconstructs it at the same place (R10.T10.g; `discNormals.ts`'
// `normalMapNormalAt`): `datum` where the texel holding the place holds none; otherwise the map's
// survey-masked bilinear reconstruction on the decoded vectors, each of the four texels whose
// centres are nearest, the gutter's at a face's edge, those holding a normal alone, by its bilinear
// weight, summed and renormalised (the holding texel's own where the sum vanishes). The terrain's
// (face, st) is the cube sphere's own, as the disc's cube direction of its hit is.
fn disc_shading_normal(face : u32, st : vec2f, datum : vec3f) -> vec3f {
  let n = textureDimensions(disc_normals).x - 2u;
  let last = f32(n - 1u);
  let st_n = st * f32(n);
  let own = disc_stored_normal(vec2i(clamp(floor(st_n), vec2f(0.0), vec2f(last))), face);
  if (own.w == 0.0) {
    return datum;
  }
  let corner_at = st_n - 0.5;
  let first = vec2i(clamp(floor(corner_at), vec2f(-1.0), vec2f(last)));
  let f = corner_at - vec2f(first);
  var sum = vec3f(0.0);
  for (var corner = 0u; corner < 4u; corner = corner + 1u) {
    let step = vec2i(i32(corner & 1u), i32(corner >> 1u));
    let one = disc_stored_normal(first + step, face);
    if (one.w == 0.0) {
      continue;
    }
    let b = select(1.0 - f.x, f.x, step.x == 1) * select(1.0 - f.y, f.y, step.y == 1);
    sum = sum + b * one.xyz;
  }
  let size = length(sum);
  return normalize(select(own.xyz, sum / size, size > 0.0));
}

// S2's quadratic warp from a face coordinate u ∈ [−1, 1] to a cell coordinate s ∈ [0, 1], the
// inverse of `stToUv` (`cubeTexel.wgsl`'s `uv_to_st`, not composed with the terrain).
fn disc_uv_to_st(u : f32) -> f32 {
  if (u >= 0.0) {
    return 0.5 * sqrt(1.0 + 3.0 * u);
  }
  return 1.0 - 0.5 * sqrt(1.0 - 3.0 * u);
}

// The least cosine between a texel's ray and its datum's normal at which `disc_place` steps along
// the ray, 0.1 (an emission angle of 84.3°): the step is at most ten times the texel's height.
const DISC_PLACE_MIN_DOWN : f32 = 0.1;

// The disc's point along a texel's ray, as the disc draws the same pixel: its place ÷ a and its
// datum's normal, body-fixed, and its cell coordinates on the texel's face.
struct DiscPlace {
  place : vec3f,
  datum : vec3f,
  face : u32,
  st : vec2f,
}

// Where the ray through the texel `in`, lit at `own`, meets the datum (R10.T15.b.2): the texel's
// foot on the datum stepped along the ground by the ray's part across it to the foot's tangent plane,
// h ÷ (−r · ν) along the ray r, so that a texel above the datum steps away from the camera and one
// below it towards it; the datum's normal there ν + the step, normalised, and its cell coordinates
// the texel's cube direction stepped likewise, on the sphere of a's radius, an error of the second
// order in the step (a few kilometres beside the body's radius) and of the first in its flattening.
fn disc_place(in : LitVertexOut, own : ShadePlace) -> DiscPlace {
  let ray = normalize(in.fromCameraBf);
  let h = own.heightM * draw.cameraBodyFixedM.w;
  let down = max(-dot(ray, own.datum), DISC_PLACE_MIN_DOWN);
  let along = ray * (h / down);
  let step = along - own.datum * dot(along, own.datum);
  let axes = horizon_face(in.horizonFace);
  let moved =
    normalize(axes.centre + stToUv(in.horizonSt.x) * axes.u + stToUv(in.horizonSt.y) * axes.v) + step;
  let k = dot(moved, axes.centre);
  var out : DiscPlace;
  out.place = own.place - own.datum * h + step;
  out.datum = normalize(own.datum + step);
  out.face = in.horizonFace;
  out.st = vec2f(disc_uv_to_st(dot(moved, axes.u) / k), disc_uv_to_st(dot(moved, axes.v) / k));
  return out;
}

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
  var at = own_shade_place(in);
  if (!reference) {
    normal = shadingNormalAt(in.slot, in.parent, in.patchUv, in.morph);
    if (TERRAIN_MIXES && in.rebakeFade > 0.0) {
      // A rebake's normal fades from the older bake's tile (R10.T16.i), or from the datum's where
      // the reference surface stood; a refine's or a coarsen's takes the parent's tile by the morph
      // factor already, and fades from the datum's where the parent held the ground unsurveyed.
      var older = datum;
      if (in.rebakeSlot == PARENT_MIX_SLOT) {
        if (parent_drawn_at(in.parent, in.patchUv)) {
          older = normal;
        }
      } else if (older_drawn_at(in.rebakeSlot, in.patchUv)) {
        older = shadingNormalAt(in.rebakeSlot, in.parent, in.patchUv, in.morph);
      }
      normal = normalize(mix(normal, older, in.rebakeFade));
    }
    // Near a hand-over that fades the terrain's own detail (R10.T15.b.2), its place, datum, height
    // and normal mixed from the disc's by the share, the disc's at 0; at 1, wherever no fade stands,
    // its own, bit for bit.
    let share = terrain_row(2u).z;
    if (share < 1.0) {
      let disc = disc_place(in, at);
      normal = normalize(mix(disc_shading_normal(disc.face, disc.st, disc.datum), normal, share));
      at = ShadePlace(
        mix(disc.place, at.place, share), normalize(mix(disc.datum, at.datum, share)), share * at.heightM
      );
    }
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
  let shaded = shade_terrain(in, normal, reference, cascade_lit, at);
  var meter = METER_OTHER;
  if (u32(terrain_row(0u).x) > 0u) {
    meter = select(METER_UNLIT_BODY, METER_LIT_BODY, shaded.lit);
  }
  return vec4f(min(shaded.radiance, vec3f(HALF_FLOAT_MAX)), meter);
}

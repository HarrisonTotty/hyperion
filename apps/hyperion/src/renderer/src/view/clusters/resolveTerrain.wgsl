// The visibility buffer's resolve of terrain (plan R14, R14.T2.d; Design note 9;
// `clusters/resolve.ts`): one full-screen draw into the scene target that, at each pixel holding a
// terrain ID, refetches its triangle's three vertices through the lit vertex function the raster
// placed them by (`lit_vertex`, so that the morph, the holds, the contacts' hold, the fades and the
// stride morph agree with it), forms the pixel's perspective-correct barycentrics from their clip
// positions snapped to the rasteriser's sub-pixel grid, interpolates the lit fragment's varyings by them, its flat ones the first corner's as
// the rasteriser takes them, and shades the pixel once by the lit fragment (`lit_fragment`), its
// geometric normal the triangle's face from its edges. A pixel with no ID, or another consumer's,
// is discarded; the draw writes no depth, the raster's standing. Composed after the interior form's
// source, its lit vertex stage the function `lit_vertex`, its fragment stage the function
// `lit_fragment` over the face's edges, its lights and contacts read from the view's resolve frame
// (`resolve_frame` below) in place of their storage buffers, terrainCluster.wgsl's decode and the
// resolves' barycentrics (`resolveBarycentrics.wgsl`; `terrainMaterial.ts`'
// `terrainResolveMaterialSpec`). The TypeScript twins are `resolve.ts`' `visibleTemplate`,
// `templateTriangle` and `clipBarycentrics`.

// The view's IDs (`clusters/visibility.ts`' `VisibilityBuffer`), read a whole texel at a time.
@group(2) @binding(25) var visibility_ids : texture_2d<u32>;

// The view's resolve frame (`gpu/resources.ts`' `resolveFrame`), one row of texels: its lights'
// rows (`packTerrainLighting`), then its contacts' count, then two texels a contact, its centre and
// held radius, then its ramp, so that the resolve reads them with no storage binding of their own.
@group(2) @binding(26) var resolve_frame : texture_2d<f32>;

// `clusters/visibility.ts`' VISIBILITY_TRIANGLE_BITS and ID_NONE, `resolve.ts`' RESOLVE_TEMPLATES,
// and `gpu/resources.ts`' RESOLVE_CONTACTS_TEXEL, the lights' rows.
const VISIBILITY_TRIANGLE_BITS : u32 = 7u;
const VISIBILITY_NONE : u32 = 0xffffffffu;
const RESOLVE_TEMPLATES : u32 = 8u;
const RESOLVE_CONTACTS_TEXEL : u32 = 28u;

// The frame's contacts, as terrainPatch.wgsl's `contacts` holds them.
fn resolve_contact_count() -> u32 {
  return u32(textureLoad(resolve_frame, vec2u(RESOLVE_CONTACTS_TEXEL, 0u), 0).x);
}

fn resolve_contact(i : u32) -> Contact {
  let a = textureLoad(resolve_frame, vec2u(RESOLVE_CONTACTS_TEXEL + 1u + 2u * i, 0u), 0);
  let b = textureLoad(resolve_frame, vec2u(RESOLVE_CONTACTS_TEXEL + 2u + 2u * i, 0u), 0);
  return Contact(a.xyz, a.w, b.x, b.y, b.z, b.w);
}

// Opaque template t's first entry in its pass (`resolve.ts`' `resolveUniforms`).
fn resolve_template_offset(t : u32) -> u32 {
  if (t < 4u) {
    return u32(draw.resolveTemplates0[t]);
  }
  return u32(draw.resolveTemplates1[t - 4u]);
}

// The opaque template of the entry at `local` of terrain's lists: the last whose first entry in its
// pass is at most the entry's (`resolve.ts`' `visibleTemplate`).
fn resolve_template(local : u32) -> u32 {
  let at = local % u32(draw.resolveLists.x);
  var which = 0u;
  for (var t = 1u; t < RESOLVE_TEMPLATES; t += 1u) {
    if (resolve_template_offset(t) <= at) {
      which = t;
    }
  }
  return which;
}

// The corners (lx, ly, code) of `triangle` of opaque template `which`, in its index order: blocks at
// strides 1 to 8, then strips (`resolve.ts`' `templateTriangle`).
fn resolve_corners(which : u32, triangle : u32) -> array<vec3f, 3> {
  let s = 1u << (which % 4u);
  let quad = triangle / 2u;
  let second = (triangle % 2u) == 1u;
  if (which < 4u) {
    let across = CLUSTER_QUADS / s;
    let x = f32((quad % across) * s);
    let y = f32((quad / across) * s);
    let w = f32(s);
    if (second) {
      return array<vec3f, 3>(vec3f(x, y, 0.0), vec3f(x + w, y + w, 0.0), vec3f(x, y + w, 0.0));
    }
    return array<vec3f, 3>(vec3f(x, y, 0.0), vec3f(x + w, y, 0.0), vec3f(x + w, y + w, 0.0));
  }
  let k = f32(quad * s);
  let next = k + f32(s);
  if (second) {
    return array<vec3f, 3>(vec3f(next, 0.0, 1.0), vec3f(next, 0.0, 0.0), vec3f(k, 0.0, 0.0));
  }
  return array<vec3f, 3>(vec3f(k, 0.0, 1.0), vec3f(next, 0.0, 1.0), vec3f(k, 0.0, 0.0));
}

@vertex
fn vertexMain(@location(0) position : vec3f) -> @builtin(position) @invariant vec4f {
  // At reversed depth's near end, so that every pixel passes the depth test; it writes none.
  return vec4f(position.xy, 1.0, 1.0);
}

@fragment
fn fragmentMain(@builtin(position) pixel : vec4f) -> @location(0) vec4f {
  let id = textureLoad(visibility_ids, vec2u(pixel.xy), 0).x;
  let visible = id >> VISIBILITY_TRIANGLE_BITS;
  let first = u32(draw.resolveLists.y);
  let kept = !(id == VISIBILITY_NONE || visible < first || visible - first >= u32(draw.resolveLists.z));
  // WGSL's `discard` demotes the invocation to a helper, which runs on to the stage's end (Tint
  // emulates it so on Metal and Direct3D, with a flag that discards at the end): a pixel of no ID
  // of these lists would refetch a triangle and shade in full. So it is shaded only where kept, in
  // a loop of at most one pass, not an `if`, as the composite's reads are (`composite.wgsl`):
  // SwiftShader, the checks' adapter, runs both sides of a branch under its lanes' mask, and a loop
  // only while a lane is in it. The pixels kept draw what they drew, bit for bit.
  var colour = vec4f(0.0);
  for (var shade = kept; shade; shade = false) {
    let local = visible - first;
    let item = cluster_lists[local];
    let instance = item >> 8u;
    let cluster = item & 0xffu;
    let sub = instances[instance].sub.x;
    // A `var`, so that the loop below may index it at run time.
    var corners = resolve_corners(resolve_template(local), id & ((1u << VISIBILITY_TRIANGLE_BITS) - 1u));
    // One call of the vertex function in a loop, so that a compiler that inlines every call (SwiftShader's)
    // takes one copy of it.
    var at : array<LitVertexOut, 3>;
    for (var k = 0u; k < 3u; k += 1u) {
      at[k] = lit_vertex(terrain_cluster_grid(cluster, corners[k], sub), instance);
    }
    let size = vec2f(textureDimensions(visibility_ids));
    let ndc = vec2f(2.0 * pixel.x / size.x - 1.0, 1.0 - 2.0 * pixel.y / size.y);
    // The corners snapped as the rasteriser snapped them, so that the weights are its own.
    let b = resolve_barycentrics(
      resolve_snapped(at[0].clip, size), resolve_snapped(at[1].clip, size), resolve_snapped(at[2].clip, size), ndc
    );
    // The flat varyings the first corner's, the provoking vertex's; the rest interpolated.
    var in = at[0];
    in.clip = pixel;
    in.patchUv = b.x * at[0].patchUv + b.y * at[1].patchUv + b.z * at[2].patchUv;
    in.fromCameraBf = b.x * at[0].fromCameraBf + b.y * at[1].fromCameraBf + b.z * at[2].fromCameraBf;
    in.datumNormal = b.x * at[0].datumNormal + b.y * at[1].datumNormal + b.z * at[2].datumNormal;
    in.heightM = b.x * at[0].heightM + b.y * at[1].heightM + b.z * at[2].heightM;
    in.horizonSt = b.x * at[0].horizonSt + b.y * at[1].horizonSt + b.z * at[2].horizonSt;
    in.morph = b.x * at[0].morph + b.y * at[1].morph + b.z * at[2].morph;
    in.gridHeightM = b.x * at[0].gridHeightM + b.y * at[1].gridHeightM + b.z * at[2].gridHeightM;
    // The geometric normal is the face's, from its edges (`cascade_geometric_normal` orients it);
    // the patch's quad from the ratio of its cell coordinates' and its place's spans across them
    // (lane fix-jagged-biome-edges), as the lit fragment takes it from their rates.
    let face_uv = abs(at[1].patchUv - at[0].patchUv) + abs(at[2].patchUv - at[0].patchUv);
    let face_st = abs(at[1].horizonSt - at[0].horizonSt) + abs(at[2].horizonSt - at[0].horizonSt);
    colour = lit_fragment(
      in, at[1].fromCameraBf - at[0].fromCameraBf, at[2].fromCameraBf - at[0].fromCameraBf, face_uv, face_st
    );
  }
  if (!kept) {
    discard;
  }
  return colour;
}

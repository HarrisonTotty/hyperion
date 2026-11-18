// The visibility buffer's resolve of the authoritative rocks (plan R14, R14.T5.e; Design note 9;
// `clusters/resolve.ts`): one full-screen draw into the scene target that, at each pixel holding an
// ID of the rocks' range of the view's index space, refetches its triangle's three corners through
// the forward draw's vertex function (`rock_vertex`, so that every place and varying is that
// draw's), forms the pixel's perspective-correct barycentrics from their clip positions snapped to
// the rasteriser's sub-pixel grid (`resolveBarycentrics.wgsl`), interpolates the forward fragment's
// varyings by them, its flat ones the first corner's, and shades the pixel once by the forward
// fragment's function (`rock_fragment`), its geometric normal the triangle's face from its edges. A
// pixel of no ID, or of another range's, is discarded; the draw writes no depth, the raster's
// standing. Composed after the lit terrain's source, its `Draw` with `resolveLists` and
// `clusterBase` (0: an ID's index is its entry's in the rocks' whole lists), its lights read from
// the view's resolve frame (`rock_resolve_frame` below) in place of their storage buffer, then
// `rockPage.wgsl`, `rock.wgsl` and the barycentrics (`rockMaterial.ts`' `rockResolveMaterialSpec`).

// The view's IDs (`clusters/visibility.ts`' `VisibilityBuffer`), past the rocks' own bindings.
@group(2) @binding(29) var rock_visibility_ids : texture_2d<u32>;

// The view's resolve frame (`gpu/resources.ts`' `resolveFrame`): its lights' rows first.
@group(2) @binding(30) var rock_resolve_frame : texture_2d<f32>;

// `clusters/visibility.ts`' VISIBILITY_TRIANGLE_BITS and ID_NONE.
const ROCK_ID_TRIANGLE_BITS : u32 = 7u;
const ROCK_ID_NONE : u32 = 0xffffffffu;

@vertex
fn vertexMain(@location(0) position : vec3f) -> @builtin(position) @invariant vec4f {
  // At reversed depth's near end, so that every pixel passes the depth test; it writes none.
  return vec4f(position.xy, 1.0, 1.0);
}

@fragment
fn fragmentMain(@builtin(position) pixel : vec4f) -> @location(0) vec4f {
  let id = textureLoad(rock_visibility_ids, vec2u(pixel.xy), 0).x;
  let visible = id >> ROCK_ID_TRIANGLE_BITS;
  let first = u32(draw.resolveLists.y);
  let kept = !(id == ROCK_ID_NONE || visible < first || visible - first >= u32(draw.resolveLists.z));
  // Shaded only where kept, in a loop of at most one pass rather than an `if`, as terrain's resolve
  // does (`resolveTerrain.wgsl`): a discarded invocation runs on as a helper, and SwiftShader runs
  // both sides of a branch under its lanes' mask.
  var colour = vec4f(0.0);
  for (var shade = kept; shade; shade = false) {
    let entry = visible - first;
    let triangle = f32(id & ((1u << ROCK_ID_TRIANGLE_BITS) - 1u));
    // One call of the vertex function in a loop, so that a compiler that inlines every call takes
    // one copy of it.
    var at : array<RockVertexOut, 3>;
    for (var k = 0u; k < 3u; k += 1u) {
      at[k] = rock_vertex(vec3f(f32(k), triangle, 0.0), entry);
    }
    let size = vec2f(textureDimensions(rock_visibility_ids));
    let ndc = vec2f(2.0 * pixel.x / size.x - 1.0, 1.0 - 2.0 * pixel.y / size.y);
    // The corners snapped as the rasteriser snapped them, so that the weights are its own.
    let b = resolve_barycentrics(
      resolve_snapped(at[0].clip, size), resolve_snapped(at[1].clip, size), resolve_snapped(at[2].clip, size), ndc
    );
    // The flat varyings the first corner's, the provoking vertex's; the rest interpolated.
    var in = at[0];
    in.clip = pixel;
    in.fromCameraBf = b.x * at[0].fromCameraBf + b.y * at[1].fromCameraBf + b.z * at[2].fromCameraBf;
    in.normal = b.x * at[0].normal + b.y * at[1].normal + b.z * at[2].normal;
    in.heightM = b.x * at[0].heightM + b.y * at[1].heightM + b.z * at[2].heightM;
    let geometric = cascade_geometric_normal(
      at[1].fromCameraBf - at[0].fromCameraBf, at[2].fromCameraBf - at[0].fromCameraBf, normalize(in.datum)
    );
    colour = rock_fragment(in, geometric);
  }
  if (!kept) {
    discard;
  }
  return colour;
}

// The authoritative rocks' ID raster (plan R14, R14.T5.e; Design note 9; `clusters/visibility.ts`):
// the rocks' lit buckets drawn into the view's visibility target with the scene's depth, each vertex
// placed by `rock_vertex` as the forward draw places it (`@invariant`), so that the raster covers
// and keeps the forward draw's pixels, and each pixel given the ID of its triangle: its entry's
// index in the rocks' range of the view's index space, `visibleFirst` plus the bucket's base plus
// its instance, above the triangle of its cluster, which the mesh template's vertex carries in its
// y (`clusters/templates.ts`' `meshTemplate`), so that no de-indexed mesh and no `primitive_index`
// is needed. The vertex stage hands the fragment stage the clip position and one flat `u32`.

// `clusters/visibility.ts`' VISIBILITY_TRIANGLE_BITS.
const ROCK_VISIBILITY_TRIANGLE_BITS : u32 = 7u;

struct RockIdVertex {
  @builtin(position) @invariant clip : vec4f,
  @location(0) @interpolate(flat) word : u32,
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> RockIdVertex {
  let at = rock_vertex(local, entry);
  var out : RockIdVertex;
  out.clip = at.clip;
  out.word = ((draw.visibleFirst + draw.clusterBase + entry) << ROCK_VISIBILITY_TRIANGLE_BITS) | u32(local.y);
  return out;
}

@fragment
fn fragmentMain(in : RockIdVertex) -> @location(0) u32 {
  return in.word;
}

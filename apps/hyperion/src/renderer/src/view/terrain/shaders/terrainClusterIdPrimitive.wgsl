// The ID raster's entry points on the `primitive_index` path (plan R14, R14.T2.c;
// `terrainClusterId.wgsl`), on a device with the `primitive-index` feature, whose source enables
// `primitive_index` first: the consumer's own cluster mesh, the vertex stage handing on the
// visible-cluster index, and the fragment stage forming the ID from the triangle the rasteriser
// names, counted from 0 for each instance, a bucket draw's instance being one cluster.

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> TerrainIdVertex {
  let at = terrain_id_place(local, entry);
  var out : TerrainIdVertex;
  out.clip = at.clip;
  out.word = at.visible;
  return out;
}

@fragment
fn fragmentMain(in : TerrainIdVertex, @builtin(primitive_index) triangle : u32) -> @location(0) u32 {
  return (in.word << VISIBILITY_TRIANGLE_BITS) | triangle;
}

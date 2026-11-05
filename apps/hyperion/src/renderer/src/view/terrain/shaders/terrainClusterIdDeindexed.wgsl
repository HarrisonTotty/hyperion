// The ID raster's entry points on the de-indexed path (plan R14, R14.T2.c; `terrainClusterId.wgsl`):
// a de-indexed cluster mesh (`clusters/visibility.ts`' `deindexedClusterMesh`), whose `position`'s z
// carries each vertex's triangle of its template above its lit-mesh code, so that the vertex stage
// writes the ID and the fragment stage writes it as it is.

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> TerrainIdVertex {
  let at = terrain_id_place(local, entry);
  var out : TerrainIdVertex;
  out.clip = at.clip;
  out.word = (at.visible << VISIBILITY_TRIANGLE_BITS) | (at.packed / DEINDEXED_TRIANGLE_STEP);
  return out;
}

@fragment
fn fragmentMain(in : TerrainIdVertex) -> @location(0) u32 {
  return in.word;
}

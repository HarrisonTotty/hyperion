// The wireframe's interior patches on the cluster path (plan R14, R14.T4.c): its vertex stage over
// a bucket's list and the terrain cluster mesh's templates, which hands `wire_vertex`, the
// wireframe's own vertex stage (terrainWire.wgsl's, as `terrainWire.ts`' `withWireClusterVertex`
// names it), exactly the lit mesh's vertex and the interior record, so that each is placed, its
// contours' coordinates and its graticule's with it, as the lit mesh's is by the instanced draw.
// Composed after terrainCluster.wgsl's decode (`terrain_cluster_grid` and `cluster_lists`, its own
// entry point taken out) and a `Draw` that declares `clusterBase`, a bucket's list base.

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> WireVertexOut {
  let item = cluster_lists[draw.clusterBase + entry];
  return wire_vertex(terrain_cluster_grid(item & 0xffu, local), item >> 8u);
}
